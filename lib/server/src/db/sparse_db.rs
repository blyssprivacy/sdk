use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Write};
use std::sync::RwLockReadGuard;
use std::sync::{Arc, RwLock};

use super::aligned_memory::AlignedMemory64;

pub unsafe fn bytes_to_u64(bytes: &[u8]) -> &[u64] {
    std::slice::from_raw_parts(
        bytes.as_ptr() as *const u64,
        bytes.len() / std::mem::size_of::<u64>(),
    )
}

type Row = (bool, Option<AlignedMemory64>);
type CacheItem = (Option<usize>, AlignedMemory64);

const CHUNK_SIZE: usize = 256 * 1024; // 256 KiB
pub struct SparseDb {
    path: String,
    item_size: usize,
    // active_item_ids: Arc<Vec<RwLock<Row>>>,
    active_rows: Arc<RwLock<HashSet<usize>>>,
    // fixed size buffer of 64-byte aligned items, usize the current item held in the buffer or None if the buffer is free for use.
    // allocated once and overwritten frequently, to avoid repeated allocations.
    item_cache: Arc<Vec<RwLock<CacheItem>>>,
    thread_pool: rayon::ThreadPool,
    cache_misses: std::sync::atomic::AtomicUsize,
}
impl SparseDb {
    pub fn new(
        path: Option<String>,
        item_size: usize,
        num_items: usize,
        prefetch_window: Option<usize>,
    ) -> Self {
        // let mut item_ids = Vec::with_capacity(num_items);
        // for i in 0..num_items {
        //     item_ids.push(RwLock::new((false, None)));
        // }
        let prefetch_window = prefetch_window.unwrap_or(32);
        let mut item_cache = Vec::with_capacity(prefetch_window);
        for i in 0..prefetch_window {
            item_cache.push(RwLock::new((
                None,
                AlignedMemory64::new(item_size / std::mem::size_of::<u64>()),
            )));
        }

        let dbstore = path.unwrap_or_else(|| String::from("/mnt/flashpir/0"));
        if !std::path::Path::new(&dbstore).exists() {
            panic!(
                "DB storage path {} does not exist on the filesystem",
                dbstore
            );
        }

        Self {
            // if path is None, use "/tmp/sparsedb"
            path: dbstore,
            item_size,
            // active_item_ids: Arc::new(item_ids),
            active_rows: Arc::new(RwLock::new(HashSet::new())),
            item_cache: Arc::new(item_cache),
            // cache_freelist: Arc::new(RwLock::new(cache_freelist)),
            thread_pool: rayon::ThreadPoolBuilder::new()
                .num_threads(8)
                .build()
                .unwrap(),
            cache_misses: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn idx_2_fpath(&self, id: usize, chunk: usize) -> String {
        let file_path = format!("{}/{}-{}", self.path, id, chunk);
        file_path
    }

    fn id_2_cache_pos(&self, id: usize) -> usize {
        id % self.item_cache.len()
    }

    // blocking read into existing cache item
    fn _prefetch(file_path: String, id: usize, item: &mut CacheItem) {
        let mut f = File::open(file_path).unwrap();
        f.read_exact(item.1.as_mut_bytes()).unwrap();
        item.0 = Some(id);
    }

    pub fn prefetch_item(&self, id: usize) {
        // if id not in active_rows, return
        {
            let active_rows = self.active_rows.read().unwrap();
            if !active_rows.contains(&id) {
                return;
            }
        }

        let file_path = self.idx_2_fpath(id, 0);
        let item_cache_ref = self.item_cache.clone();
        let cache_pos = self.id_2_cache_pos(id);

        self.thread_pool.spawn_fifo(move || {
            let mut item_guard = item_cache_ref[cache_pos].write().unwrap();
            let item = &mut *item_guard;
            SparseDb::_prefetch(file_path, id, item);
        });
    }

    pub fn get_active_ids(&self) -> Vec<usize> {
        let set = self.active_rows.read().unwrap();
        set.iter().cloned().collect()
    }

    pub fn get_item(&self, id: usize) -> Option<RwLockReadGuard<CacheItem>> {
        // check if id is active
        {
            let active_rows = self.active_rows.read().unwrap();
            if !active_rows.contains(&id) {
                return None;
            }
        }

        let cache_pos = self.id_2_cache_pos(id);
        // try serving from cache
        {
            let item = self.item_cache[cache_pos].read().unwrap();
            if item.0.is_some() && item.0.unwrap() == id {
                return Some(item);
            }
        }

        // if missed, synchronously fetch the item into cache
        self.cache_misses
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let file_path = self.idx_2_fpath(id, 0);
        {
            let mut item_guard = self.item_cache[cache_pos].write().unwrap();
            let item = &mut *item_guard;
            SparseDb::_prefetch(file_path, id, item);
        }

        // // acquire read lock on the newly written item, and return the guard to consumer
        // let item = self.item_cache[cache_pos].read().unwrap();
        // // if item was evicted, recurse and retry
        // if item.0.is_none() || item.0.unwrap() != id {
        //     drop(item);
        //     println!("WARN: thrashing on item {}", id);
        // }
        // Some(item)

        // retry serving the item from cache
        // todo: track thrashing?
        self.get_item(id)
    }

    pub fn delete(&self, idx: usize) {
        // remove from valid set
        {
            let mut active_rows = self.active_rows.write().unwrap();
            active_rows.remove(&idx);
        }
        // (no need to explicitly evict from cache, we'll just let it be overwritten)

        // delete from filesystem
        let file_path = self.idx_2_fpath(idx, 0);
        std::fs::remove_file(file_path).ok();
    }

    pub fn upsert(&self, idx: usize, data: &[u64]) {
        // race to write in filesystem
        // Simultaneous upserts to the same idx are not allowed
        // Interspersed reads and writes to SparseDB are not allowed
        let mut file = File::create(self.idx_2_fpath(idx, 0)).unwrap();
        let byte_slice = unsafe {
            std::slice::from_raw_parts(
                data.as_ptr() as *const u8,
                data.len() * std::mem::size_of::<u64>(),
            )
        };
        file.write_all(byte_slice).unwrap();
        // write_all returns when the new file is visible to other processes, but may not yet be fully commited to nonvolatile storage (i.e. can't survive power loss).
        // sync_all absolutely ensures durability, but will limit write speed per disk to ~800MB/s.
        // file.sync_all().unwrap();

        // add to valid set
        {
            let mut active_rows = self.active_rows.write().unwrap();
            active_rows.insert(idx);
        }
    }

    // benchmarking tools, not needed for correctness
    pub fn current_size(&self) -> usize {
        let num_active_rows = self.active_rows.read().unwrap().len();
        num_active_rows * self.item_size
    }

    pub fn pop_cache_misses(&self) -> usize {
        self.cache_misses
            .swap(0, std::sync::atomic::Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use rand::{Rng, SeedableRng};
    use rayon::prelude::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};

    use super::*;
    use std::time::Instant;

    #[test]
    fn benchmark_sparse_db() {
        let n = 16_000; // number of items
        let item_size = 256 * 1024; // 256KiB
        println!(
            "Benchmarking {} MiB SparseDb ({} items @ {} KiB each)",
            n * item_size / 1024 / 1024,
            n,
            item_size / 1024
        );

        let mut rng = rand::rngs::SmallRng::seed_from_u64(17 as u64);
        let mut data = vec![0u64; item_size / std::mem::size_of::<u64>()];
        for item in data.iter_mut() {
            *item = rng.gen();
        }

        // Initialize SparseDb
        const PREFETCH_WINDOW: usize = 32;
        let db = SparseDb::new(None, item_size, n, Some(PREFETCH_WINDOW));

        // Insert N items
        let start = Instant::now();
        (0..n).into_par_iter().with_max_len(8).for_each(|i| {
            db.upsert(i, &data);
        });
        let duration = start.elapsed();
        let write_bw = (n as f64 * item_size as f64) / (duration.as_secs_f64() * 1e6);
        println!(
            "Write: {} MiB/s ({} ms)",
            write_bw as u64,
            duration.as_millis()
        );

        // dump kernel page cache
        std::process::Command::new("sudo")
            .arg("sh")
            .arg("-c")
            .arg("echo 3 > /proc/sys/vm/drop_caches")
            .output()
            .unwrap();

        // Sequentially read and measure bandwidth
        let start = Instant::now();
        for i in 0..PREFETCH_WINDOW - 1 {
            db.prefetch_item(i);
        }
        let mut sum: u64 = 0;
        for i in 0..n {
            db.prefetch_item(i + PREFETCH_WINDOW - 1);
            {
                let item = db.get_item(i).unwrap();
                let b = item.1.as_slice();
                for value in b.iter() {
                    sum += *value;
                }
            }
            // db.release_item(i);
        }
        core::hint::black_box(sum);
        let duration = start.elapsed();
        let read_bw = (n as f64 * item_size as f64) / (duration.as_secs_f64() * 1e6);
        println!(
            "Read: {} MiB/s ({} ms)",
            read_bw as u64,
            duration.as_millis()
        );
    }
}

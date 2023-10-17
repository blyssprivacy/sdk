use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::prelude::PermissionsExt;
use std::sync::RwLockReadGuard;
use std::sync::{Arc, RwLock};

use crossbeam_utils::CachePadded;

use super::aligned_memory::AlignedMemory64;
use super::sorted_vec::SortedVec;

pub unsafe fn bytes_to_u64(bytes: &[u8]) -> &[u64] {
    std::slice::from_raw_parts(
        bytes.as_ptr() as *const u64,
        bytes.len() / std::mem::size_of::<u64>(),
    )
}
pub struct CacheItem {
    id: Option<usize>,
    pub data: AlignedMemory64,
}
pub struct SparseDb {
    paths: Vec<String>,
    item_size: usize,
    // current, sorted set of non-sparse items
    active_item_ids: Arc<RwLock<SortedVec>>,
    // fixed size buffer of 64-byte aligned items, usize the current item held in the buffer or None if the buffer is free for use.
    // allocated once and overwritten frequently, to avoid repeated allocations.
    item_cache: Arc<Vec<CachePadded<RwLock<CacheItem>>>>,
    thread_pool: rayon::ThreadPool,
    cache_misses: std::sync::atomic::AtomicUsize,
}
impl SparseDb {
    pub fn new(
        uuid: Option<String>,
        path: Option<String>,
        item_size: usize,
        _num_items: usize,
        prefetch_window: Option<usize>,
    ) -> Self {
        let uuid = uuid.unwrap_or_else(|| String::from("dev"));
        let dbpath = path.unwrap_or_else(|| String::from("/mnt/flashpir"));
        let dbroot = std::path::Path::new(&dbpath);
        if !dbroot.exists() {
            panic!(
                "DB storage path {} does not exist on the filesystem",
                dbroot.to_str().unwrap()
            );
        }

        // subdirs of the form //dbroot/diskN will be treated as independent storage devices;
        // SparseDb will distribute reads & writes in round-robin order over devices
        let mut storage_paths: Vec<_> = std::fs::read_dir(dbroot)
            .unwrap()
            .filter_map(|res| {
                let path = res.unwrap().path();
                if path.is_dir() {
                    let dir_name = path.file_name().unwrap().to_str().unwrap();
                    if dir_name.starts_with("disk") && dir_name[4..].parse::<u32>().is_ok() {
                        Some(path)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        storage_paths.sort();

        // if no subdirs are found, use dbroot directly as the only storage device
        if storage_paths.is_empty() {
            storage_paths.push(dbroot.to_path_buf());
        }

        // create unique subdirectories for this db instance
        for storage_path in &mut storage_paths {
            let new_path = storage_path.join(&uuid);
            std::fs::create_dir_all(&new_path).unwrap();
            std::fs::set_permissions(&new_path, std::fs::Permissions::from_mode(0o777)).unwrap();
            *storage_path = new_path;
        }

        let paths: Vec<String> = storage_paths
            .iter()
            .map(|p| p.to_str().unwrap().to_string())
            .collect();

        println!("Using storage devices: {:?}", paths);
        assert!(paths.len() > 0);
        let n_ssds = paths.len();

        // setup read scratchpad ("cache")
        let prefetch_window = prefetch_window.unwrap_or(32);
        let mut item_cache = Vec::with_capacity(prefetch_window);
        for _ in 0..prefetch_window {
            item_cache.push(CachePadded::new(RwLock::new(CacheItem {
                id: None,
                data: AlignedMemory64::new(item_size / std::mem::size_of::<u64>()),
            })));
        }
        println!(
            "Using {} MiB of memory for SparseDb item cache",
            item_cache.len() * item_size / 1024 / 1024
        );
        // at least 8 threads, at most 4 threads per SSD, and at most PREFETCH_WINDOW threads total
        let n_prefetchers = std::cmp::max(std::cmp::min(4 * n_ssds, prefetch_window), 8);

        Self {
            paths: paths,
            item_size,
            active_item_ids: Arc::new(RwLock::new(SortedVec::new())),
            item_cache: Arc::new(item_cache),
            thread_pool: rayon::ThreadPoolBuilder::new()
                .num_threads(n_prefetchers)
                .build()
                .unwrap(),
            cache_misses: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn id_2_fpath(&self, id: usize) -> String {
        let device_idx = id % self.paths.len();
        let path = self.paths[device_idx].clone();

        let file_path = format!("{}/{}", path, id);
        file_path
    }

    fn id_2_cache_pos(&self, id: usize) -> usize {
        id % self.item_cache.len()
    }

    // blocking read into existing cache item
    fn _prefetch(file_path: String, id: usize, item: &mut CacheItem) {
        let mut f = File::open(file_path).unwrap();
        f.read_exact(item.data.as_mut_bytes()).unwrap();
        item.id = Some(id);
    }

    fn prefetch_item(&self, id: usize) {
        // if id is not active, return
        {
            let active_item_ids = self.active_item_ids.read().unwrap();
            if !active_item_ids.contains(id) {
                return;
            }
        }

        let file_path = self.id_2_fpath(id);
        let item_cache_ref = self.item_cache.clone();
        let cache_pos = self.id_2_cache_pos(id);

        self.thread_pool.spawn_fifo(move || {
            let mut item_guard = item_cache_ref[cache_pos].write().unwrap();
            let item = &mut *item_guard;
            SparseDb::_prefetch(file_path, id, item);
        });
    }

    pub fn prefill(&self) {
        // Fetch items from the beginning of the active set to fill the prefetch window.
        // Nonblocking.
        let n = self.item_cache.len();
        let ids_ref = self.active_item_ids.read().unwrap();
        let ids = ids_ref.as_vec();
        for i in 0..std::cmp::min(n, ids.len()) {
            let id = ids[i];
            self.prefetch_item(id);
        }
    }

    pub fn get_active_ids(&self) -> Vec<usize> {
        let set = self.active_item_ids.read().unwrap();
        set.as_vec().clone()
    }

    fn _get(&self, id: usize, attempts: usize) -> Option<RwLockReadGuard<CacheItem>> {
        // check if id is active
        {
            let active_item_ids = self.active_item_ids.read().unwrap();
            if !active_item_ids.contains(id) {
                return None;
            }

            // Start prefetching a new active item to fill the end of the prefetch window
            // Fetching into the end of the window maximizes time to prefetch, and minimizes risk of evicting the item we're about to read
            let ordered_idx_of_current_item = active_item_ids.index_of(id).unwrap();
            let idx_to_prefetch = ordered_idx_of_current_item + self.item_cache.len() - 1;
            let id_to_prefetch = active_item_ids.as_vec().get(idx_to_prefetch).cloned();

            if let Some(id_to_prefetch) = id_to_prefetch {
                self.prefetch_item(id_to_prefetch);
            }
        }

        let cache_pos = self.id_2_cache_pos(id);
        // try serving from cache, without waiting for locks
        {
            if let Ok(item) = self.item_cache[cache_pos].try_read() {
                if let Some(item_id) = item.id {
                    if item_id == id {
                        return Some(item);
                    }
                }
            }
        }

        // if the correct item was not immediately ready, count as miss
        self.cache_misses
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // however, a prefetcher may be working on it, and thus holding the corresponding lock.
        // Wait for the lock to see if our data arrives.
        {
            let item = self.item_cache[cache_pos].read().unwrap();
            if let Some(item_id) = item.id {
                if item_id == id {
                    return Some(item);
                }
            }
        }

        // if the data is still not in cache, we must fill it ourselves
        let file_path = self.id_2_fpath(id);
        {
            let mut item_guard = self.item_cache[cache_pos].write().unwrap();
            let item = &mut *item_guard;
            SparseDb::_prefetch(file_path, id, item);
        }

        // after fill, recurse to retry serving the item from cache, up to a limit
        if attempts > 3 {
            panic!("FATAL: item {} exceeded retry limit for fill attempts", id);
        }

        if attempts > 0 {
            println!(
                "WARNING: item {} was evicted immediately after fill. Retrying ({} attempts)",
                id, attempts
            );
        }
        self._get(id, attempts + 1)
    }

    pub fn get_item(&self, id: usize) -> Option<RwLockReadGuard<CacheItem>> {
        self._get(id, 0)
    }

    pub fn delete(&self, id: usize) {
        // remove from valid set
        {
            let mut active_item_ids = self.active_item_ids.write().unwrap();
            active_item_ids.remove(id);
        }
        // (no need to explicitly evict from cache, just let it be overwritten)

        // race to delete from filesystem
        let file_path = self.id_2_fpath(id);
        std::fs::remove_file(file_path).ok();
    }

    pub fn upsert(&self, id: usize, data: &[u64]) {
        // race to write in filesystem
        // Simultaneous upserts to the same item id are not allowed
        // Interspersed reads and writes to SparseDB are not allowed
        let mut file = File::create(self.id_2_fpath(id)).unwrap();
        let byte_slice = unsafe {
            std::slice::from_raw_parts(
                data.as_ptr() as *const u8,
                data.len() * std::mem::size_of::<u64>(),
            )
        };
        file.write_all(byte_slice).unwrap();
        // write_all returns when the new file is visible to other processes, but may not yet be fully commited to nonvolatile storage (i.e. can't survive power loss).
        // sync_all ensures durability, but will limit write speed per disk to ~800MB/s.
        // file.sync_all().unwrap();

        // add to valid set
        {
            let mut active_item_ids = self.active_item_ids.write().unwrap();
            active_item_ids.insert(id);
        }
    }

    // benchmarking tools
    pub fn current_count(&self) -> usize {
        self.active_item_ids.read().unwrap().as_vec().len()
    }

    pub fn current_size(&self) -> usize {
        self.current_count() * self.item_size
    }

    pub fn pop_cache_misses(&self) -> usize {
        self.cache_misses
            .swap(0, std::sync::atomic::Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use rand::{seq::index::sample, Rng, SeedableRng};
    use rayon::prelude::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};

    use super::*;
    use std::time::Instant;

    #[test]
    fn benchmark_sparse_db() {
        let n = 16_000; // number of items
        let item_size = 1024 * 1024;
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
        const PREFETCH_WINDOW: usize = 500;
        let db = SparseDb::new(None, None, item_size, n, Some(PREFETCH_WINDOW));

        // let item_ids = (0..n).collect::<Vec<usize>>();
        const DB_DENSITY: f64 = 0.5;
        let item_ids =
            sample(&mut rng, (n as f64 / 1.0_f64.min(DB_DENSITY)) as usize, n).into_vec();
        let n_range = *item_ids.iter().max().unwrap_or(&0);

        // Insert N items
        let start = Instant::now();
        item_ids.into_par_iter().with_max_len(32).for_each(|i| {
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
        db.prefill();
        for i in 0..n_range {
            {
                let item = db.get_item(i);
                if item.is_none() {
                    continue;
                }
                let item = item.unwrap();
                let b = item.data.as_slice();
                core::hint::black_box(b);
            }
        }
        let duration = start.elapsed();
        let read_bw = (n as f64 * item_size as f64) / (duration.as_secs_f64() * 1e6);
        println!(
            "Read: {} MiB/s ({} ms)",
            read_bw as u64,
            duration.as_millis()
        );

        let misses = db.pop_cache_misses();
        let miss_rate = misses as f64 / n as f64;
        println!("Cache misses: {} ({:.2}%)", misses, miss_rate * 100.0);
    }
}

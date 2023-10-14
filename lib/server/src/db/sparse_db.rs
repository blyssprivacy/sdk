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

const CHUNK_SIZE: usize = 256 * 1024; // 256 KiB
pub struct SparseDb {
    path: String,
    item_size: usize,
    // indices of nonsparse rows, sorted.
    active_item_ids: Arc<Vec<RwLock<Row>>>,
    active_rows: Arc<RwLock<HashSet<usize>>>,
    // item_cache: RwLock<HashMap<usize, Vec<u8>>>,
    thread_pool: rayon::ThreadPool,
}
impl SparseDb {
    pub fn new(path: Option<String>, item_size: usize, num_items: usize) -> Self {
        let mut item_ids = Vec::with_capacity(num_items);
        for i in 0..num_items {
            item_ids.push(RwLock::new((false, None)));
        }

        Self {
            // if path is None, use "/tmp/sparsedb"
            path: path.unwrap_or_else(|| String::from("/mnt/nvme2")),
            item_size,
            active_item_ids: Arc::new(item_ids),
            active_rows: Arc::new(RwLock::new(HashSet::new())),
            thread_pool: rayon::ThreadPoolBuilder::new()
                .num_threads(8)
                .build()
                .unwrap(),
        }
    }

    fn idx_2_fpath(&self, id: usize, chunk: usize) -> String {
        let file_path = format!("{}/{}-{}", self.path, id, chunk);
        file_path
    }

    // blocks until data is ready
    // TODO: make zero copy, read data directly into item_cache?
    // careful to avoid locking item_cache, must support parallel prefetching
    fn _prefetch(item_size: usize, file_path: String) -> AlignedMemory64 {
        let mut f = File::open(file_path).unwrap();
        let mut data = AlignedMemory64::new(item_size / std::mem::size_of::<u64>());
        f.read_exact(data.as_mut_bytes()).unwrap();
        data
    }

    pub fn prefetch_item(&self, id: usize) {
        if id >= self.active_item_ids.len() {
            return;
        }

        {
            let item = self.active_item_ids[id].read().unwrap();
            let (present, _) = *item;
            if !present {
                return;
            }
        }

        let file_path = self.idx_2_fpath(id, 0);
        let item_ids_ref = self.active_item_ids.clone();
        let item_size = self.item_size;

        self.thread_pool.spawn_fifo(move || {
            let data = SparseDb::_prefetch(item_size, file_path);
            let mut item = item_ids_ref[id].write().unwrap();
            *item = (true, Some(data));
        });
    }

    pub fn release_item(&self, id: usize) {
        let mut item = self.active_item_ids[id].write().unwrap();
        *item = (true, None);
    }

    pub fn current_size(&self) -> usize {
        let num_active_rows = self.active_rows.read().unwrap().len();
        num_active_rows * self.item_size
    }

    pub fn get_active_ids(&self) -> Vec<usize> {
        let set = self.active_rows.read().unwrap();
        set.iter().cloned().collect()
    }

    pub fn get_item(&self, id: usize) -> Option<RwLockReadGuard<Row>> {
        let item = self.active_item_ids[id].read().unwrap();
        let (present, _) = *item;

        if !present {
            return None;
        }

        if item.1.is_some() {
            println!("hit  {}", id);
            Some(item)
        } else {
            println!("miss {}", id);
            // drop read lock
            drop(item);

            // item not in cache, read from disk
            let file_path = self.idx_2_fpath(id, 0);
            let data = SparseDb::_prefetch(self.item_size, file_path);

            // acquire write lock
            let mut item = self.active_item_ids[id].write().unwrap();

            // insert into cache
            *item = (true, Some(data));

            let bytes = item.1.as_ref().unwrap();

            drop(item);
            let item = self.active_item_ids[id].read().unwrap();

            Some(item)
        }
    }

    pub fn delete(&self, idx: usize) {
        {
            let mut active_rows = self.active_rows.write().unwrap();
            active_rows.remove(&idx);
        }

        let mut item = self.active_item_ids[idx].write().unwrap();

        let file_path = self.idx_2_fpath(idx, 0);
        std::fs::remove_file(file_path).ok();

        *item = (false, None);
    }

    pub fn upsert(&self, idx: usize, data: &[u64]) {
        {
            let mut active_rows = self.active_rows.write().unwrap();
            active_rows.insert(idx);
        }

        let mut item = self.active_item_ids[idx].write().unwrap();

        // race to write to filesystem
        let mut file = File::create(self.idx_2_fpath(idx, 0)).unwrap();
        let byte_slice = unsafe {
            std::slice::from_raw_parts(
                data.as_ptr() as *const u8,
                data.len() * std::mem::size_of::<u64>(),
            )
        };
        file.write_all(byte_slice).unwrap();
        file.sync_all().unwrap();
        drop(file);

        *item = (true, None);
    }
}

#[cfg(test)]
mod tests {
    use rand::{Rng, SeedableRng};
    use rayon::prelude::{IntoParallelIterator, ParallelIterator};

    use super::*;
    use std::time::Instant;

    #[test]
    fn benchmark_sparse_db() {
        let n = 8000; // number of items
        let item_size = 1024 * 1024; // 256KiB
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
        let PATH: String = String::from("/scratch/sparsedb_bench");
        let db = SparseDb::new(Some(PATH), item_size, n);

        // Insert N items
        let start = Instant::now();
        (0..n).into_par_iter().for_each(|i| {
            db.upsert(i, &data);
        });
        let duration = start.elapsed();
        let write_bw = (n as f64 * item_size as f64) / (duration.as_secs_f64() * 1e6);
        println!(
            "Write: {} MiB/s ({} ms)",
            write_bw as u64,
            duration.as_millis()
        );

        // wait, counting down to stdout every second
        const WAIT: usize = 1;
        print!("Waiting {} seconds: ", WAIT);
        std::io::stdout().flush().unwrap();
        for _ in 0..WAIT {
            std::thread::sleep(std::time::Duration::from_secs(1));
            print!(".");
            std::io::stdout().flush().unwrap();
        }
        println!("");

        // Sequentially read and measure bandwidth
        let start = Instant::now();
        // prefetch W items in advance
        const PREFETCH_OFFSET: usize = 32;
        const W: usize = 8;
        for i in PREFETCH_OFFSET..(PREFETCH_OFFSET + W) {
            db.prefetch_item(i);
        }
        let mut sum: u64 = 0;
        for i in 0..n {
            db.prefetch_item(PREFETCH_OFFSET + i + W);
            {
                let item = db.get_item(i).unwrap();
                let b = item.1.as_ref().unwrap().as_slice();
                for value in b.iter() {
                    sum += *value;
                }
            }
            db.release_item(i);
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

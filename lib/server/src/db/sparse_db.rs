use std::{
    fs::{File, OpenOptions},
    io::Write,
    sync::{Arc, RwLock},
};

use memmapix::{Advice, Mmap, MmapOptions};

pub struct SparseDb {
    path: String,
    item_size: usize,
    // indices of nonsparse rows, sorted.
    // Wrapped in refcell to allow for interior mutability.
    active_item_ids: RwLock<Vec<(usize, Arc<Mmap>)>>,
    // Methods that modify active_item_ids must acquire this lock.
    // Once modification of the Vec is done, the lock can be dropped, so the actual disk IO is parallel.
    // Callers must never make simultaneous modifications to the same row (they would race in the filesystem)
    // TODO: use RwLock instead of Mutex ?
    // writer_lock: std::sync::Arc<std::sync::Mutex<()>>,
    thread_pool: rayon::ThreadPool,
}
impl SparseDb {
    pub fn new(path: Option<String>, item_size: usize) -> Self {
        Self {
            // if path is None, use "/tmp/sparsedb"
            path: path.unwrap_or_else(|| String::from("/scratch/sparsedb")),
            item_size,
            active_item_ids: RwLock::new(Vec::new()),
            thread_pool: rayon::ThreadPoolBuilder::new()
                .num_threads(8)
                .build()
                .unwrap(),
        }
    }

    fn idx_2_fpath(&self, idx: usize) -> String {
        let file_path = format!("{}/{}", self.path, idx);
        file_path
    }

    // blocks until data is paged in
    fn _prefetch(item: &Mmap) {
        let _ = item.advise(Advice::Sequential);
        let _ = item.advise(Advice::WillNeed);
    }

    pub fn prefetch_item(&self, id: usize) {
        let item_ids = self.active_item_ids.read().unwrap();
        let (found, pos) = SparseDb::index_of_item(&item_ids, id);
        if !found {
            return;
        }
        let mmap = Arc::clone(&item_ids[pos].1);

        self.thread_pool.spawn_fifo(move || {
            SparseDb::_prefetch(&mmap);
        });
    }

    fn index_of_item<T>(map: &Vec<(usize, T)>, idx: usize) -> (bool, usize) {
        // binary search to find the index of idx in a vec of key-value tuples.
        // if idx is not found, return the index where it should be inserted.
        if map.is_empty() {
            return (false, 0);
        }

        let pp = map.partition_point(|x| x.0 < idx);

        if pp == map.len() {
            return (false, pp);
        }

        if map[pp].0 == idx {
            (true, pp)
        } else {
            (false, pp)
        }
    }

    pub fn current_size(&self) -> usize {
        let item_ids = self.active_item_ids.read().unwrap();
        item_ids.len() * self.item_size
    }

    pub fn get_active_ids(&self) -> Vec<usize> {
        let item_ids = self.active_item_ids.read().unwrap();
        item_ids.iter().map(|x| x.0).collect()
    }

    pub fn get_item(&self, idx: usize) -> Option<&[u64]> {
        let item_ids = self.active_item_ids.read().unwrap();
        let (found, pos) = SparseDb::index_of_item(&item_ids, idx);
        if !found {
            return None;
        }

        let mmap = &item_ids[pos].1;

        let bytes = mmap.as_ref();
        assert_eq!(
            bytes.as_ptr() as usize % 32,
            0,
            "Row base address not aligned to 32 bytes"
        );

        assert_eq!(
            bytes.len() % std::mem::size_of::<u64>(),
            0,
            "Row size not divisible by 8 bytes; malformed u64's"
        );
        unsafe {
            Some(std::slice::from_raw_parts(
                bytes.as_ptr() as *const u64,
                bytes.len() / std::mem::size_of::<u64>(),
            ))
        }
    }

    fn invalidate_item(&self, idx: usize) {
        // atomically remove item from active_item_ids
        let mut item_ids = self.active_item_ids.write().unwrap();
        let (found, pos) = SparseDb::index_of_item(&item_ids, idx);
        if !found {
            return;
        }
        (*item_ids).remove(pos);
    }

    pub fn delete(&self, idx: usize) {
        self.invalidate_item(idx);
        // FS delete is unprotected by locks, races considered acceptable
        let file_path = self.idx_2_fpath(idx);
        std::fs::remove_file(file_path).ok();
    }

    pub fn upsert(&self, idx: usize, data: &[u64]) {
        // invalidate existing data to avoid torn writes
        self.invalidate_item(idx);

        // race to write to filesystem
        let file_path = self.idx_2_fpath(idx);
        let mut file = File::create(self.idx_2_fpath(idx)).unwrap();
        let byte_slice = unsafe {
            std::slice::from_raw_parts(
                data.as_ptr() as *const u8,
                data.len() * std::mem::size_of::<u64>(),
            )
        };
        file.write_all(byte_slice).unwrap();
        file.sync_all().unwrap();
        drop(file);

        // after writing, remap the file
        let ro_file = File::open(file_path).unwrap();
        let mmap = Arc::new(unsafe { MmapOptions::new().map(&ro_file).unwrap() });

        // acquire lock, then load new mmap into active_item_ids
        let mut item_ids = self.active_item_ids.write().unwrap();
        let (_, pos) = SparseDb::index_of_item(&item_ids, idx);
        (*item_ids).insert(pos, (idx, Arc::clone(&mmap)));
    }
}
unsafe impl Send for SparseDb {}
unsafe impl Sync for SparseDb {}

#[cfg(test)]
mod tests {
    use rand::{Rng, SeedableRng};
    use rayon::prelude::{IntoParallelIterator, ParallelIterator};

    use super::*;
    use std::time::Instant;

    #[test]
    fn benchmark_sparse_db() {
        let n = 16000; // number of items
        let item_size = 1024 * 1024; // 256KiB
        let mut rng = rand::rngs::SmallRng::seed_from_u64(17 as u64);
        let mut data = vec![0u64; item_size / std::mem::size_of::<u64>()];
        for item in data.iter_mut() {
            *item = rng.gen();
        }

        // Initialize SparseDb
        let PATH: String = String::from("/scratch/sparsedb_bench");
        let db = SparseDb::new(Some(PATH), item_size);

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

        // Sequentially read and measure bandwidth
        let start = Instant::now();
        // prefetch W items in advance
        const W: usize = 8;
        for i in 1..W {
            db.prefetch_item(i);
        }
        for i in 0..n {
            db.prefetch_item(i + W);
            let b = db.get_item(i);
            core::hint::black_box(b.unwrap()[0]);
        }
        let duration = start.elapsed();
        let read_bw = (n as f64 * item_size as f64) / (duration.as_secs_f64() * 1e6);
        println!(
            "Read: {} MiB/s ({} ms)",
            read_bw as u64,
            duration.as_millis()
        );
    }
}

use std::{
    fs::{File, OpenOptions},
    io::Write,
    sync::RwLock,
};

use memmapix::{Advice, Mmap, MmapOptions};

pub struct SparseDb {
    path: String,
    item_size: usize,
    // indices of nonsparse rows, sorted.
    // Wrapped in refcell to allow for interior mutability.
    active_item_ids: RwLock<Vec<(usize, Mmap)>>,
    // Methods that modify active_item_ids must acquire this lock.
    // Once modification of the Vec is done, the lock can be dropped, so the actual disk IO is parallel.
    // Callers must never make simultaneous modifications to the same row (they would race in the filesystem)
    // TODO: use RwLock instead of Mutex ?
    // writer_lock: std::sync::Arc<std::sync::Mutex<()>>,
}
impl SparseDb {
    pub fn new(path: Option<String>, item_size: usize) -> Self {
        Self {
            // if path is None, use "/tmp/sparsedb"
            path: path.unwrap_or_else(|| String::from("/scratch/sparsedb")),
            item_size,
            active_item_ids: RwLock::new(Vec::new()),
        }
    }

    fn idx_2_fpath(&self, idx: usize) -> String {
        let file_path = format!("{}/{}", self.path, idx);
        file_path
    }

    fn read_file(&self, idx: usize) -> File {
        let file_path = self.idx_2_fpath(idx);
        File::open(file_path).unwrap()
    }

    fn write_file(&self, idx: usize) -> File {
        let file_path = self.idx_2_fpath(idx);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&file_path)
            .unwrap();
        file.set_len(self.item_size as u64).unwrap();
        file
    }

    pub fn prefetch_item(&self, id: usize) {
        let item_ids = self.active_item_ids.read().unwrap();

        let (found, pos) = SparseDb::index_of_item(&item_ids, id);
        if !found {
            return;
        }

        let mmap = &item_ids[pos].1;
        let _ = mmap.advise(Advice::Sequential);
        let _ = mmap.advise(Advice::WillNeed);
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
        let mut file = File::create(self.idx_2_fpath(idx)).unwrap();
        let byte_slice = unsafe {
            std::slice::from_raw_parts(
                data.as_ptr() as *const u8,
                data.len() * std::mem::size_of::<u64>(),
            )
        };
        file.write_all(byte_slice).unwrap();
        file.sync_all().unwrap();

        // after writing, remap the file
        let ro_file = self.read_file(idx);
        let mmap = unsafe { MmapOptions::new().map(&ro_file).unwrap() };

        // load read-only mmap into active_item_ids
        let mut item_ids = self.active_item_ids.write().unwrap();
        let (_, pos) = SparseDb::index_of_item(&item_ids, idx);
        (*item_ids).insert(pos, (idx, mmap));
    }
}
unsafe impl Send for SparseDb {}
unsafe impl Sync for SparseDb {}

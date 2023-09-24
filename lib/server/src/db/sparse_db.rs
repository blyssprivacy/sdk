use std::{
    cell::RefCell,
    fs::{File, OpenOptions},
};

use memmapix::{Advice, Mmap, MmapMut, MmapOptions};

pub struct SparseDb {
    path: String,
    item_size: usize,
    // indices of nonsparse rows, sorted.
    // Wrapped in refcell to allow for interior mutability.
    active_item_ids: RefCell<Vec<usize>>,
    // Methods that modify active_item_ids must acquire this lock.
    // Once modification of the Vec is done, the lock can be dropped, so the actual disk IO is parallel.
    // Callers must never make simultaneous modifications to the same row (they would race in the filesystem)
    // TODO: use RwLock instead of Mutex ?
    writer_lock: std::sync::Arc<std::sync::Mutex<()>>,
}
impl SparseDb {
    pub fn new(path: Option<String>, item_size: usize) -> Self {
        Self {
            // if path is None, use "/tmp/sparsedb"
            path: path.unwrap_or_else(|| String::from("/tmp/sparsedb")),
            item_size,
            active_item_ids: RefCell::new(Vec::new()),
            writer_lock: std::sync::Arc::new(std::sync::Mutex::new(())),
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

    fn prefetch_item(&self, idx: usize) {
        // TODO: implement using madvise
        let _idx = idx;
    }

    fn index_of_item(&self, idx: usize) -> (bool, usize) {
        // binary search to find the index of idx in active_item_ids.
        // if idx is not found, return the index where it should be inserted.
        let item_ids = self.active_item_ids.borrow();

        if item_ids.is_empty() {
            return (false, 0);
        }

        let pp = item_ids.partition_point(|&x| x < idx);

        if pp == item_ids.len() {
            return (false, pp);
        }

        if item_ids[pp] == idx {
            (true, pp)
        } else {
            (false, pp)
        }
    }

    pub fn get_active_ids(&self) -> Vec<usize> {
        self.active_item_ids.borrow().clone()
    }

    pub fn get_item(&self, idx: usize) -> Option<Mmap> {
        let (found, pos) = self.index_of_item(idx);
        if !found {
            return None;
        }
        let file = self.read_file(idx);
        let mmap = unsafe { MmapOptions::new().populate().map(&file).unwrap() };

        // prefetch the next nonsparse item id
        let ids = self.active_item_ids.borrow();
        if pos < ids.len() - 1 {
            let next = ids.get(pos + 1);
            if let Some(next) = next {
                self.prefetch_item(*next);
            }
        }

        Some(mmap)
    }

    // static
    pub fn mmap_to_slice(mmap: &Mmap) -> &[u64] {
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
            std::slice::from_raw_parts(
                bytes.as_ptr() as *const u64,
                bytes.len() / std::mem::size_of::<u64>(),
            )
        }
    }

    pub fn delete(&self, idx: usize) {
        let writer_lock = self.writer_lock.clone();
        let _lock = writer_lock.lock().unwrap();

        let (found, pos) = self.index_of_item(idx);
        if !found {
            return;
        }
        // modification to shared state done, races in filesytem are ok.
        drop(_lock);

        let file_path = self.idx_2_fpath(idx);
        std::fs::remove_file(file_path).ok();
        self.active_item_ids.borrow_mut().remove(pos);
    }

    pub fn upsert(&self, idx: usize, data: &[u64]) {
        let writer_lock = self.writer_lock.clone();
        let _lock = writer_lock.lock().unwrap();
        let (found, pos) = self.index_of_item(idx);
        if !found {
            self.active_item_ids.borrow_mut().insert(pos, idx);
        }
        drop(_lock);

        let file = self.write_file(idx);
        let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
        let bytes = mmap.as_mut();
        if bytes.len() >= data.len() * std::mem::size_of::<u64>() {
            let u64_slice = unsafe {
                std::slice::from_raw_parts_mut(bytes.as_mut_ptr() as *mut u64, data.len())
            };
            u64_slice.copy_from_slice(data);
            mmap.flush().unwrap();
        }
    }
}
unsafe impl Send for SparseDb {}
unsafe impl Sync for SparseDb {}

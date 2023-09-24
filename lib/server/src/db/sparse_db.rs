use std::fs::{File, OpenOptions};

use memmapix::{Mmap, MmapMut};

pub struct SparseDb {
    path: String,
    // indices of nonzero rows, sorted
    pub active_item_ids: Vec<usize>,
    item_size: usize,
}
impl SparseDb {
    pub fn new(path: Option<String>, item_size: usize) -> Self {
        Self {
            // if path is None, use "/tmp/sparsedb"
            path: path.unwrap_or_else(|| String::from("/tmp/sparsedb")),
            active_item_ids: Vec::new(),
            item_size,
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

    pub fn get_item(&self, idx: usize) -> Option<Mmap> {
        let (found, _) = self.index_of_item(idx);
        if !found {
            return None;
        }
        let file = self.read_file(idx);
        let mmap = unsafe { Mmap::map(&file).unwrap() };
        Some(mmap)
    }

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

    fn index_of_item(&self, idx: usize) -> (bool, usize) {
        if self.active_item_ids.is_empty() {
            return (false, 0);
        }

        let pp = self.active_item_ids.partition_point(|&x| x < idx);

        if pp == self.active_item_ids.len() {
            return (false, pp);
        }

        if self.active_item_ids[pp] == idx {
            (true, pp)
        } else {
            (false, pp)
        }
    }

    pub fn delete(&mut self, idx: usize) {
        let (found, pp) = self.index_of_item(idx);
        if !found {
            return;
        }

        let file_path = self.idx_2_fpath(idx);
        std::fs::remove_file(file_path).ok();
        self.active_item_ids.remove(pp);
    }

    pub fn upsert(&mut self, idx: usize, data: &[u64]) {
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

        let (found, pp) = self.index_of_item(idx);
        if !found {
            self.active_item_ids.insert(pp, idx);
        }
    }
}

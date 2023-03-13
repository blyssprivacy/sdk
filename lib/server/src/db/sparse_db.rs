use std::collections::HashMap;

use super::aligned_memory::AlignedMemory64;

pub struct SparseDb {
    // series of polynomials
    pub data: Vec<AlignedMemory64>,

    // db_idx to data vector index
    pub db_idx_to_vec_idx: HashMap<usize, usize>,
}
impl SparseDb {
    pub fn new() -> SparseDb {
        SparseDb {
            data: Vec::new(),
            db_idx_to_vec_idx: HashMap::new(),
        }
    }

    pub fn get_idx(&self, idx: usize) -> Option<&usize> {
        self.db_idx_to_vec_idx.get(&idx)
    }

    pub fn add(&mut self, idx: usize, data: &[u64]) {
        let mut new_poly = AlignedMemory64::new(data.len());
        new_poly.as_mut_slice().copy_from_slice(data);
        self.data.push(new_poly);
        self.db_idx_to_vec_idx.insert(idx, self.data.len() - 1);
    }

    fn update_impl(&mut self, vec_idx: usize, data: &[u64]) {
        self.data[vec_idx].as_mut_slice().copy_from_slice(data);
    }

    pub fn update(&mut self, idx: usize, data: &[u64]) {
        let vec_idx = self.get_idx(idx).unwrap();
        self.update_impl(*vec_idx, data);
    }

    pub fn upsert(&mut self, idx: usize, data: &[u64]) {
        let opt_vec_idx = self.get_idx(idx);
        if let Some(vec_idx) = opt_vec_idx {
            self.update_impl(*vec_idx, data);
        } else {
            self.add(idx, data);
        }
    }
}

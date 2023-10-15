pub struct SortedVec {
    data: Vec<usize>,
}

impl SortedVec {
    // Create a new, empty SortedVec
    pub fn new() -> Self {
        SortedVec { data: Vec::new() }
    }

    // Insert a new element, maintaining sorted order
    pub fn insert(&mut self, value: usize) {
        let insert_idx = match self.data.binary_search(&value) {
            Ok(idx) | Err(idx) => idx,
        };
        self.data.insert(insert_idx, value);
    }

    pub fn remove(&mut self, value: usize) {
        match self.data.binary_search(&value) {
            Ok(idx) => {
                self.data.remove(idx);
            }
            Err(_) => {}
        }
    }

    // Check if the SortedVec contains a value
    pub fn contains(&self, value: usize) -> bool {
        self.data.binary_search(&value).is_ok()
    }

    pub fn index_of(&self, value: usize) -> Option<usize> {
        self.data.binary_search(&value).ok()
    }

    pub fn as_vec(&self) -> &Vec<usize> {
        &self.data
    }
}

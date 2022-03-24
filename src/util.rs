use crate::params::*;

pub fn calc_index(indices: &[usize], lengths: &[usize]) -> usize {
    let mut idx = 0usize;
    let mut prod = 1usize;
    for i in (0..indices.len()).rev() {
        idx += indices[i] * prod;
        prod *= lengths[i];
    }
    idx
}

pub fn get_test_params() -> Params {
    Params::init(
        2048, 
        &vec![268369921u64, 249561089u64], 
        6.4,
        2,
        56,
        56,
        56,
        56,
        true
    )
}
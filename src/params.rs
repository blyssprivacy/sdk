use crate::{arith::*, ntt::*};

pub struct Params {
    pub poly_len: usize,
    pub poly_len_log2: usize,
    pub ntt_tables: Vec<Vec<Vec<u64>>>,
    pub crt_count: usize,
    pub moduli: Vec<u64>,
}

impl Params {
    pub fn num_words(&self) -> usize {
        self.poly_len * self.crt_count
    }
    pub fn get_ntt_forward_table(&self, i: usize) -> &[u64] {
        self.ntt_tables[i][0].as_slice()
    }
    pub fn get_ntt_forward_prime_table(&self, i: usize) -> &[u64] {
        self.ntt_tables[i][1].as_slice()
    }

    pub fn init(poly_len: usize, moduli: Vec<u64>) -> Self {
        let poly_len_log2 = log2(poly_len as u64) as usize;
        let crt_count = moduli.len();
        let ntt_tables = build_ntt_tables(poly_len, moduli.as_slice());
        Self {
            poly_len,
            poly_len_log2,
            ntt_tables,
            crt_count,
            moduli,
        }
    }
}

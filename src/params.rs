use crate::{arith::*, ntt::*};

pub struct Params {    
    pub poly_len: usize,
    pub poly_len_log2: usize,
    pub ntt_tables: Vec<Vec<Vec<u64>>>,    
    
    pub crt_count: usize,
    pub moduli: Vec<u64>,
    pub modulus: u64,

    pub noise_width: f64,

    pub n: usize,

    pub t_conv: usize,
    pub t_exp_left: usize,
    pub t_exp_right: usize,
    pub t_gsw: usize,
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
    pub fn get_ntt_inverse_table(&self, i: usize) -> &[u64] {
        self.ntt_tables[i][2].as_slice()
    }
    pub fn get_ntt_inverse_prime_table(&self, i: usize) -> &[u64] {
        self.ntt_tables[i][3].as_slice()
    }

    pub fn get_sk_gsw(&self) -> (usize, usize) {
        (self.n, 1)
    }
    pub fn get_sk_reg(&self) -> (usize, usize) {
        (1, 1)
    }

    pub fn m_conv(&self) -> usize {
        2 * self.t_conv;
    }

    pub fn init(
        poly_len: usize,
        moduli: &Vec<u64>,
        noise_width: f64,
        n: usize,
        t_conv: usize,
        t_exp_left: usize,
        t_exp_right: usize,
        t_gsw: usize,
    ) -> Self {
        let poly_len_log2 = log2(poly_len as u64) as usize;
        let crt_count = moduli.len();
        let ntt_tables = build_ntt_tables(poly_len, moduli.as_slice());
        let mut modulus = 1;
        for m in moduli {
            modulus *= m;
        }
        Self {
            poly_len,
            poly_len_log2,
            ntt_tables,
            crt_count,
            moduli: moduli.clone(),
            modulus,
            noise_width,
            n,
            t_conv,
            t_exp_left,
            t_exp_right,
            t_gsw,
        }
    }
}

use crate::{arith::*, ntt::*, number_theory::*, poly::*};

pub static Q2_VALUES: [u64; 37] = [
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    12289,
    12289,
    61441,
    65537,
    65537,
    520193,
    786433,
    786433,
    3604481,
    7340033,
    16515073,
    33292289,
    67043329,
    132120577,
    268369921,
    469762049,
    1073479681,
    2013265921,
    4293918721,
    8588886017,
    17175674881,
    34359214081,
    68718428161,
];

#[derive(Debug, PartialEq)]
pub struct Params {
    pub poly_len: usize,
    pub poly_len_log2: usize,
    pub ntt_tables: Vec<Vec<Vec<u64>>>,
    pub scratch: Vec<u64>,

    pub crt_count: usize,
    pub moduli: Vec<u64>,
    pub modulus: u64,
    pub modulus_log2: u64,
    pub noise_width: f64,

    pub n: usize,
    pub pt_modulus: u64,
    pub q2_bits: u64,
    pub t_conv: usize,
    pub t_exp_left: usize,
    pub t_exp_right: usize,
    pub t_gsw: usize,

    pub expand_queries: bool,
    pub db_dim_1: usize,
    pub db_dim_2: usize,
    pub instances: usize,
    pub db_item_size: usize,
}

impl Params {
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

    pub fn get_v_neg1(&self) -> Vec<PolyMatrixNTT> {
        let mut v_neg1 = Vec::new();
        for i in 0..self.poly_len_log2 {
            let idx = self.poly_len - (1 << i);
            let mut ng1 = PolyMatrixRaw::zero(&self, 1, 1);
            ng1.data[idx] = 1;
            v_neg1.push((-&ng1).ntt());
        }
        v_neg1
    }

    pub fn get_sk_gsw(&self) -> (usize, usize) {
        (self.n, 1)
    }
    pub fn get_sk_reg(&self) -> (usize, usize) {
        (1, 1)
    }

    pub fn m_conv(&self) -> usize {
        self.t_conv
    }

    pub fn crt_compose_1(&self, x: u64) -> u64 {
        assert_eq!(self.crt_count, 1);
        x
    }

    pub fn crt_compose_2(&self, x: u64, y: u64) -> u64 {
        assert_eq!(self.crt_count, 2);

        let a = self.moduli[0];
        let b = self.moduli[1];
        let a_inv_mod_b = invert_uint_mod(a, b).unwrap();
        let b_inv_mod_a = invert_uint_mod(b, a).unwrap();
        let mut val = (x as u128) * (b_inv_mod_a as u128) * (b as u128);
        val += (y as u128) * (a_inv_mod_b as u128) * (a as u128);
        (val % (self.modulus as u128)) as u64 // FIXME: use barrett
    }

    pub fn crt_compose(&self, a: &[u64], idx: usize) -> u64 {
        if self.crt_count == 1 {
            self.crt_compose_1(a[idx])
        } else {
            self.crt_compose_2(a[idx], a[idx + self.poly_len])
        }
    }

    pub fn init(
        poly_len: usize,
        moduli: &Vec<u64>,
        noise_width: f64,
        n: usize,
        pt_modulus: u64,
        q2_bits: u64,
        t_conv: usize,
        t_exp_left: usize,
        t_exp_right: usize,
        t_gsw: usize,
        expand_queries: bool,
        db_dim_1: usize,
        db_dim_2: usize,
        instances: usize,
        db_item_size: usize,
    ) -> Self {
        let poly_len_log2 = log2(poly_len as u64) as usize;
        let crt_count = moduli.len();
        let ntt_tables = build_ntt_tables(poly_len, moduli.as_slice());
        let scratch = vec![0u64; crt_count * poly_len];
        let mut modulus = 1;
        for m in moduli {
            modulus *= m;
        }
        let modulus_log2 = log2_ceil(modulus);
        Self {
            poly_len,
            poly_len_log2,
            ntt_tables,
            scratch,
            crt_count,
            moduli: moduli.clone(),
            modulus,
            modulus_log2,
            noise_width,
            n,
            pt_modulus,
            q2_bits,
            t_conv,
            t_exp_left,
            t_exp_right,
            t_gsw,
            expand_queries,
            db_dim_1,
            db_dim_2,
            instances,
            db_item_size,
        }
    }
}

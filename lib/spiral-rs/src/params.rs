use std::mem::size_of;

use crate::{arith::*, client::SEED_LENGTH, ntt::*, number_theory::*, poly::*};

pub const MAX_MODULI: usize = 4;

pub static MIN_Q2_BITS: u64 = 14;
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

#[derive(Debug, PartialEq, Clone)]
pub struct Params {
    pub poly_len: usize,
    pub poly_len_log2: usize,
    pub ntt_tables: Vec<Vec<Vec<u64>>>,
    pub scratch: Vec<u64>,

    pub crt_count: usize,
    pub barrett_cr_0: [u64; MAX_MODULI],
    pub barrett_cr_1: [u64; MAX_MODULI],
    pub barrett_cr_0_modulus: u64,
    pub barrett_cr_1_modulus: u64,
    pub mod0_inv_mod1: u64,
    pub mod1_inv_mod0: u64,
    pub moduli: [u64; MAX_MODULI],
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

    pub fn num_expanded(&self) -> usize {
        1 << self.db_dim_1
    }

    pub fn num_items(&self) -> usize {
        (1 << self.db_dim_1) * (1 << self.db_dim_2)
    }

    pub fn item_size(&self) -> usize {
        let logp = log2(self.pt_modulus) as usize;
        self.instances * self.n * self.n * self.poly_len * logp / 8
    }

    pub fn g(&self) -> usize {
        let num_bits_to_gen = self.t_gsw * self.db_dim_2 + self.num_expanded();
        log2_ceil_usize(num_bits_to_gen)
    }

    pub fn stop_round(&self) -> usize {
        log2_ceil_usize(self.t_gsw * self.db_dim_2)
    }

    pub fn factor_on_first_dim(&self) -> usize {
        if self.db_dim_2 == 0 {
            1
        } else {
            2
        }
    }

    pub fn setup_bytes(&self) -> usize {
        let mut sz_polys = 0;

        let packing_sz = ((self.n + 1) - 1) * self.t_conv;
        sz_polys += self.n * packing_sz;

        if self.expand_queries {
            let expansion_left_sz = self.g() * self.t_exp_left;
            let expansion_right_sz = (self.stop_round() + 1) * self.t_exp_right;
            let conversion_sz = 2 * self.t_conv;

            sz_polys += expansion_left_sz + expansion_right_sz + conversion_sz;
        }

        let sz_bytes = sz_polys * self.poly_len * size_of::<u64>();
        SEED_LENGTH + sz_bytes
    }

    pub fn query_bytes(&self) -> usize {
        let sz_polys;

        if self.expand_queries {
            sz_polys = 1;
        } else {
            let first_dimension_sz = self.num_expanded();
            let further_dimension_sz = self.db_dim_2 * (2 * self.t_gsw);
            sz_polys = first_dimension_sz + further_dimension_sz;
        }

        let sz_bytes = sz_polys * self.poly_len * size_of::<u64>();
        SEED_LENGTH + sz_bytes
    }

    pub fn query_v_buf_bytes(&self) -> usize {
        self.num_expanded() * self.poly_len * size_of::<u64>()
    }

    pub fn bytes_per_chunk(&self) -> usize {
        let trials = self.n * self.n;
        let chunks = self.instances * trials;
        let bytes_per_chunk = f64::ceil(self.db_item_size as f64 / chunks as f64) as usize;
        bytes_per_chunk
    }

    pub fn modp_words_per_chunk(&self) -> usize {
        let bytes_per_chunk = self.bytes_per_chunk();
        let logp = log2(self.pt_modulus);
        let modp_words_per_chunk = f64::ceil((bytes_per_chunk * 8) as f64 / logp as f64) as usize;
        modp_words_per_chunk
    }

    pub fn crt_compose_1(&self, x: u64) -> u64 {
        assert_eq!(self.crt_count, 1);
        x
    }

    pub fn crt_compose_2(&self, x: u64, y: u64) -> u64 {
        assert_eq!(self.crt_count, 2);

        let mut val = (x as u128) * (self.mod1_inv_mod0 as u128);
        val += (y as u128) * (self.mod0_inv_mod1 as u128);

        barrett_reduction_u128(self, val)
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
        moduli: &[u64],
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
        assert!(q2_bits >= MIN_Q2_BITS);

        let poly_len_log2 = log2(poly_len as u64) as usize;
        let crt_count = moduli.len();
        assert!(crt_count <= MAX_MODULI);
        let mut moduli_array = [0; MAX_MODULI];
        for i in 0..crt_count {
            moduli_array[i] = moduli[i];
        }
        let ntt_tables = build_ntt_tables(poly_len, moduli);
        let scratch = vec![0u64; crt_count * poly_len];
        let mut modulus = 1;
        for m in moduli {
            modulus *= m;
        }
        let modulus_log2 = log2_ceil(modulus);
        let (barrett_cr_0, barrett_cr_1) = get_barrett(moduli);
        let (barrett_cr_0_modulus, barrett_cr_1_modulus) = get_barrett_crs(modulus);
        let mut mod0_inv_mod1 = 0;
        let mut mod1_inv_mod0 = 0;
        if crt_count == 2 {
            mod0_inv_mod1 = moduli[0] * invert_uint_mod(moduli[0], moduli[1]).unwrap();
            mod1_inv_mod0 = moduli[1] * invert_uint_mod(moduli[1], moduli[0]).unwrap();
        }
        Self {
            poly_len,
            poly_len_log2,
            ntt_tables,
            scratch,
            crt_count,
            barrett_cr_0,
            barrett_cr_1,
            barrett_cr_0_modulus,
            barrett_cr_1_modulus,
            mod0_inv_mod1,
            mod1_inv_mod0,
            moduli: moduli_array,
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

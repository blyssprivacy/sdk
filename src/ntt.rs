use std::usize;

use crate::{number_theory::*, params::*, poly::*};
use rand::Rng;

pub fn build_ntt_tables(poly_len: usize, moduli: &[u64]) -> Vec<Vec<Vec<u64>>> {
    let mut v: Vec<Vec<Vec<u64>>> = Vec::new();
    for coeff_mod in 0..moduli.len() {
        let modulus = moduli[coeff_mod];
        let root = get_minimal_primitive_root(2 * poly_len as u64, modulus).unwrap();
        let inv_root = invert_uint_mod(root, modulus);
    }
    v
}

pub fn ntt_forward(params: Params, out: &mut PolyMatrixRaw, inp: &PolyMatrixRaw) {
    for coeff_mod in 0..params.crt_count {
        let mut n = params.poly_len;

        for mm in 0..params.poly_len_log2 {
            let m = 1 << mm;
            let t = n >> (mm + 1);

            for i in 0..m {
                let w = params.get_ntt_forward_table(coeff_mod);
                let wprime = params.get_ntt_forward_prime_table(coeff_mod);
            }
        }
    }
}

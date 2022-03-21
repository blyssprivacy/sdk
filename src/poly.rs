use std::arch::x86_64::*;
use std::ops::Mul;

use crate::{arith::*, params::*, util::calc_index};

pub trait PolyMatrix<'a> {
    fn is_ntt(&self) -> bool;
    fn get_rows(&self) -> usize;
    fn get_cols(&self) -> usize;
    fn get_params(&self) -> &Params;
    fn zero(params: &'a Params, rows: usize, cols: usize) -> Self;
    fn random(params: &'a Params, rows: usize, cols: usize, rng: &mut dyn Iterator<Item=u64>) -> Self;
    fn as_slice(&self) -> &[u64];
    fn as_mut_slice(&mut self) -> &mut [u64];
    fn zero_out(&mut self) {
        for item in self.as_mut_slice() {
            *item = 0;
        }
    }
    fn get_poly(&self, row: usize, col: usize) -> &[u64] {
        let num_words = self.get_params().num_words();
        let start = (row * self.get_cols() + col) * num_words;
        &self.as_slice()[start..start + num_words]
    }
    fn get_poly_mut(&mut self, row: usize, col: usize) -> &mut [u64] {
        let num_words = self.get_params().num_words();
        let start = (row * self.get_cols() + col) * num_words;
        &mut self.as_mut_slice()[start..start + num_words]
    }
}

pub struct PolyMatrixRaw<'a> {
    params: &'a Params,
    rows: usize,
    cols: usize,
    data: Vec<u64>,
}

pub struct PolyMatrixNTT<'a> {
    params: &'a Params,
    rows: usize,
    cols: usize,
    data: Vec<u64>,
}

impl<'a> PolyMatrix<'a> for PolyMatrixRaw<'a> {
    fn is_ntt(&self) -> bool {
        false
    }
    fn get_rows(&self) -> usize {
        self.rows
    }
    fn get_cols(&self) -> usize {
        self.cols
    }
    fn get_params(&self) -> &Params {
        &self.params
    }
    fn as_slice(&self) -> &[u64] {
        self.data.as_slice()
    }
    fn as_mut_slice(&mut self) -> &mut [u64] {
        self.data.as_mut_slice()
    }
    fn zero(params: &'a Params, rows: usize, cols: usize) -> PolyMatrixRaw<'a> {
        let num_coeffs = rows * cols * params.poly_len;
        let data: Vec<u64> = vec![0; num_coeffs];
        PolyMatrixRaw {
            params,
            rows,
            cols,
            data,
        }
    }
    fn random(params: &'a Params, rows: usize, cols: usize, rng: &mut dyn Iterator<Item=u64>) -> Self {
        let mut out = PolyMatrixRaw::zero(params, rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                for i in 0..params.poly_len {
                    let val: u64 = rng.next().unwrap();
                    out.get_poly_mut(r, c)[i] = val % params.modulus;
                }
            }
        }
        out
    }
}

impl<'a> PolyMatrix<'a> for PolyMatrixNTT<'a> {
    fn is_ntt(&self) -> bool {
        true
    }
    fn get_rows(&self) -> usize {
        self.rows
    }
    fn get_cols(&self) -> usize {
        self.cols
    }
    fn get_params(&self) -> &Params {
        &self.params
    }
    fn as_slice(&self) -> &[u64] {
        self.data.as_slice()
    }
    fn as_mut_slice(&mut self) -> &mut [u64] {
        self.data.as_mut_slice()
    }
    fn zero(params: &'a Params, rows: usize, cols: usize) -> PolyMatrixNTT<'a> {
        let num_coeffs = rows * cols * params.poly_len * params.crt_count;
        let data: Vec<u64> = vec![0; num_coeffs];
        PolyMatrixNTT {
            params,
            rows,
            cols,
            data,
        }
    }
    fn random(params: &'a Params, rows: usize, cols: usize, rng: &mut dyn Iterator<Item=u64>) -> Self {
        let mut out = PolyMatrixNTT::zero(params, rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                for i in 0..params.crt_count {
                    for j in 0..params.poly_len {
                        let idx = calc_index(&[i, j], &[params.crt_count, params.poly_len]);
                        let val: u64 = rng.next().unwrap();
                        out.get_poly_mut(r, c)[idx] = val % params.moduli[i];
                    }
                }
            }
        }
        out
    }
}

pub fn multiply_poly(params: &Params, res: &mut [u64], a: &[u64], b: &[u64]) {
    for c in 0..params.crt_count {
        for i in 0..params.poly_len {
            res[i] = multiply_modular(params, a[i], b[i], c);
        }
    }
}

pub fn multiply_add_poly(params: &Params, res: &mut [u64], a: &[u64], b: &[u64]) {
    for c in 0..params.crt_count {
        for i in 0..params.poly_len {
            res[i] = multiply_add_modular(params, a[i], b[i], res[i], c);
        }
    }
}

pub fn multiply_add_poly_avx(params: &Params, res: &mut [u64], a: &[u64], b: &[u64]) {
    for c in 0..params.crt_count {
        for i in (0..params.poly_len).step_by(4) {
            unsafe {
                let p_x = &a[c*params.poly_len + i] as *const u64;
                let p_y = &b[c*params.poly_len + i] as *const u64;
                let p_z = &mut res[c*params.poly_len + i] as *mut u64;
                let x = _mm256_loadu_si256(p_x as *const __m256i);
                let y = _mm256_loadu_si256(p_y as *const __m256i);
                let z = _mm256_loadu_si256(p_z as *const __m256i);

                let product = _mm256_mul_epu32(x, y);
                let out = _mm256_add_epi64(z, product);
                
                _mm256_storeu_si256(p_z as *mut __m256i, out);
            }
        }
    }
}

pub fn modular_reduce(params: &Params, res: &mut [u64]) {
    for c in 0..params.crt_count {
        for i in 0..params.poly_len {
            res[c*params.poly_len + i] %= params.moduli[c];
        }
    }
}

#[cfg(not(target_feature = "avx2"))]
pub fn multiply(res: &mut PolyMatrixNTT, a: &PolyMatrixNTT, b: &PolyMatrixNTT) {
    assert!(a.cols == b.rows);

    for i in 0..a.rows {
        for j in 0..b.cols {
            for z in 0..res.params.poly_len {
                res.get_poly_mut(i, j)[z] = 0;
            }
            for k in 0..a.cols {
                let params = res.params;
                let res_poly = res.get_poly_mut(i, j);
                let pol1 = a.get_poly(i, k);
                let pol2 = b.get_poly(k, j);
                multiply_add_poly(params, res_poly, pol1, pol2);
            }
        }
    }
}

#[cfg(target_feature = "avx2")]
pub fn multiply(res: &mut PolyMatrixNTT, a: &PolyMatrixNTT, b: &PolyMatrixNTT) {
    assert!(a.cols == b.rows);

    let params = res.params;
    for i in 0..a.rows {
        for j in 0..b.cols {
            for z in 0..res.params.poly_len {
                res.get_poly_mut(i, j)[z] = 0;
            }
            let res_poly = res.get_poly_mut(i, j);
            for k in 0..a.cols {
                let pol1 = a.get_poly(i, k);
                let pol2 = b.get_poly(k, j);
                multiply_add_poly_avx(params, res_poly, pol1, pol2);
            }
            modular_reduce(params, res_poly);
        }
    }
}

impl<'a> Mul for PolyMatrixNTT<'a> {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let mut out = PolyMatrixNTT::zero(self.params, self.rows, rhs.cols);
        multiply(&mut out, &self, &rhs);
        out
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn get_params() -> Params {
        Params::init(2048, &vec![268369921u64, 249561089u64])
    }

    fn assert_all_zero(a: &[u64]) {
        for i in a {
            assert_eq!(*i, 0);
        }
    }

    #[test]
    fn sets_all_zeros() {
        let params = get_params();
        let m1 = PolyMatrixNTT::zero(&params, 2, 1);
        assert_all_zero(m1.as_slice());
    }

    #[test]
    fn multiply_correctness() {
        let params = get_params();
        let m1 = PolyMatrixNTT::zero(&params, 2, 1);
        let m2 = PolyMatrixNTT::zero(&params, 3, 2);
        let m3 = m2 * m1;
        assert_all_zero(m3.as_slice());
    }
}

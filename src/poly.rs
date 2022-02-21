use crate::{arith::*, params::*};

pub trait PolyMatrix<'a> {
    fn is_ntt(&self) -> bool;
    fn get_rows(&self) -> usize;
    fn get_cols(&self) -> usize;
    fn get_params(&self) -> &Params;
    fn zero(params: &'a Params, rows: usize, cols: usize) -> Self;
    fn as_slice(&self) -> &[u64];
    fn as_mut_slice(&mut self) -> &mut [u64];
    fn zero_out(&mut self) {
        for item in self.as_mut_slice() {
            *item = 0;
        }
    }
    fn get_poly(&self, row: usize, col: usize) -> &[u64] {
        let params = self.get_params();
        let start = (row * self.get_cols() + col) * params.poly_len;
        &self.as_slice()[start..start + params.poly_len]
    }
    fn get_poly_mut(&mut self, row: usize, col: usize) -> &mut [u64] {
        let poly_len = self.get_params().poly_len;
        let start = (row * self.get_cols() + col) * poly_len;
        &mut self.as_mut_slice()[start..start + poly_len]
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

#[cfg(test)]
mod test {
    use super::*;

    fn get_params() -> Params {
        Params::init(2048, vec![7, 31])
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
        let mut m3 = PolyMatrixNTT::zero(&params, 3, 1);
        multiply(&mut m3, &m2, &m1);
        assert_all_zero(m3.as_slice());
    }
}

//! Contraction / Expansion
//!
//! Allows representation of a single, large true value using several underlying database elements.
//!
//! Specifically, we represent each true element with `delta` database elements in Z_'mod',
//! mapped from \[0, mod\] to \[-mod/2, mod/2\].
//!
//! To contract, do the opposite.

use crate::arith::arith::{centered_to_raw, raw_to_centered, reconstruct_from_base_p};

use super::matrix::Matrix;

/// Parameters for contracting.
pub struct ContractParams {
    /// The modulus to use.
    pub modulus: u32,

    /// The number of values mod `modulus` to map to a single true value.
    pub delta: usize,
}

pub trait Contractable {
    /// Contracts the matrix, taking groups of `delta` input value,
    /// each in the range \[-mod/2, mod/2\], and reconstructing a single, larger
    /// value in the range \[0, mod ^ delta)
    fn contract(&self, contract_params: &ContractParams) -> Self;

    /// Expands the matrix, taking large input values and splitting them
    /// into `delta` elements, each in the range \[-mod/2, mod/2\].
    fn expand(&self, contract_params: &ContractParams) -> Self;
}

impl Contractable for Matrix {
    fn contract(&self, contract_params: &ContractParams) -> Self {
        let (modulus, delta) = (contract_params.modulus, contract_params.delta);
        let mut out = Matrix::new(self.rows / delta, self.cols);

        for i in 0..out.rows {
            for j in 0..out.cols {
                let mut vals = Vec::new();
                for f in 0..delta {
                    let new_val = self[i * delta + f][j];
                    vals.push(centered_to_raw(new_val, modulus) as u64);
                }
                out.data[i * self.cols + j] +=
                    (reconstruct_from_base_p(modulus as u64, &vals)) as u32
            }
        }

        out
    }

    fn expand(&self, contract_params: &ContractParams) -> Self {
        let (modulus, delta) = (contract_params.modulus, contract_params.delta);
        let mut out = Matrix::new(self.rows * delta, self.cols);

        for i in 0..self.rows {
            for j in 0..self.cols {
                let mut val = self[i][j];
                for f in 0..delta {
                    let new_val = val % modulus;
                    out.data[(i * delta + f) * self.cols + j] = raw_to_centered(new_val, modulus);
                    val /= modulus;
                }
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use crate::matrix::matrix::Matrix;

    use super::{ContractParams, Contractable};

    #[test]
    fn squish_unsquish_are_inverses() {
        let contract_params = ContractParams {
            modulus: 552,
            delta: 4,
        };
        let m = Matrix::random(8, 35);
        let me = m.expand(&contract_params);
        let guess1 = me.contract(&contract_params);
        let me2 = guess1.expand(&contract_params);
        let guess2 = me2.contract(&contract_params);

        assert_eq!(guess1, m);
        assert_eq!(guess2, m);
    }
}

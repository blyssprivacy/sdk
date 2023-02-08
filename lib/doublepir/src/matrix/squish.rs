//! Squishing / Unsquishing
//!
//! Allows representation of several, small true values with a single underlying database value.

use super::matrix::Matrix;

/// Parameters for squishing.
///
/// `basis` * `delta` must be less than or equal to 32.
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct SquishParams {
    /// The number of bits per true value.
    pub basis: u64,

    /// The number of true values to group into a single underlying database value.
    pub delta: usize,
}

impl SquishParams {
    pub fn default() -> SquishParams {
        SquishParams {
            basis: 10,
            delta: 3,
        }
    }

    pub fn zero() -> SquishParams {
        SquishParams { basis: 0, delta: 0 }
    }

    fn validate(&self) {
        assert!(self.basis < 32);
        assert!(self.delta < 32);
        assert!(self.basis * (self.delta as u64) <= 32);
    }
}

pub trait Squishable {
    /// Squishes the matrix, representing each group of `delta` consecutive input values,
    /// each of `basis` bits, as a single output value.
    ///
    /// Requires that each input value is at most 'basis' bits.
    fn squish(&self, squish_params: &SquishParams) -> Self;

    /// Unsquishes the matrix, taking each input value and splitting it into 'delta'
    /// consecutive values, each of 'basis' bits.
    ///
    /// `orig_cols` is the original number of columns in the matrix, before it was squished.
    fn unsquish(&self, squish_params: &SquishParams, orig_cols: usize) -> Self;
}

impl Squishable for Matrix {
    fn squish(&self, squish_params: &SquishParams) -> Self {
        squish_params.validate();
        let (delta, basis) = (squish_params.delta, squish_params.basis);
        let mut out = Matrix::new(self.rows, (self.cols + delta - 1) / delta);

        for i in 0..out.rows {
            for j in 0..out.cols {
                for k in 0..delta {
                    if delta * j + k < self.cols {
                        let val = self[i][delta * j + k];
                        out.data[i * out.cols + j] += val << ((k as u64) * basis)
                    }
                }
            }
        }

        out
    }

    fn unsquish(&self, squish_params: &SquishParams, orig_cols: usize) -> Self {
        squish_params.validate();
        let (delta, basis) = (squish_params.delta, squish_params.basis);
        assert!(orig_cols <= self.cols * delta);

        let mut out = Matrix::new(self.rows, orig_cols);
        let mask = (1 << basis) - 1;

        for i in 0..self.rows {
            for j in 0..self.cols {
                for k in 0..delta {
                    if j * delta + k < orig_cols {
                        out.data[i * out.cols + j * delta + k] =
                            ((self[i][j]) >> ((k as u64) * basis)) & mask;
                    }
                }
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use crate::matrix::matrix::Matrix;

    use super::{SquishParams, Squishable};

    #[test]
    fn squish_unsquish_are_inverses() {
        let squish_params = SquishParams {
            basis: 10,
            delta: 3,
        };
        let mut m = Matrix::random(10, 35);
        m %= 1 << squish_params.basis;
        let ms = m.squish(&squish_params);
        let guess1 = ms.unsquish(&squish_params, m.cols);
        let ms2 = guess1.squish(&squish_params);
        let guess2 = ms2.unsquish(&squish_params, m.cols);

        assert_eq!(guess1, m);
        assert_eq!(guess2, m);
    }
}

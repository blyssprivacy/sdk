use std::{future::Future, slice};

use rand::{
    distributions::{Standard, Uniform},
    Rng, SeedableRng,
};
use rand_chacha::ChaCha20Rng;

use crate::{debug, util::*};

use super::{derive_with_aes, gauss_sample};

pub type Seed = <ChaCha20Rng as SeedableRng>::Seed;

/// !! INSERCURE !!
/// Replaces all calls for randomness with fixed data.
/// Very useful for debugging, but completely negates all security guarantees!
/// Do NOT enable in production.
pub const DETERMINISTIC: bool = false;

/// Computes full checksums when calling `Matrix.checksum()`.
/// Very useful for debugging, but incurs significant runtime cost!
/// Do not enable in production.
pub const COMPUTE_FULL_CHECKSUMS: bool = false;

/// A matrix of [`u32`] values, supporting most basic matrix operations.
///
/// # Examples
///
/// ```
/// # use doublepir_rs::matrix::Matrix;
/// let mut m1 = Matrix::new(3, 5);
/// m1[0][3] = 7;
///
/// let m2 = Matrix::random(5, 2);
/// let m3 = &m1 * &m2;
///
/// assert_eq!(m3.rows, m1.rows);
/// assert_eq!(m3.cols, m2.cols);
///
/// let m4 = Matrix::random(5, 2);
/// let m5 = m2 + m4;
/// ```
///
/// Each `Matrix` owns its own data, as a vector.
/// `clone()` is a deep copy.
#[derive(PartialEq, Debug)]
pub struct Matrix {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<u32>,
}

impl Matrix {
    /// Construct a new Matrix filled with zeros.
    pub fn new(rows: usize, cols: usize) -> Self {
        Matrix {
            rows,
            cols,
            data: vec![0u32; rows * cols],
        }
    }

    /// Construct a new Matrix filled with uniformly random data.
    pub fn random(rows: usize, cols: usize) -> Self {
        if DETERMINISTIC {
            return Self::derive_from_seed(rows, cols, SEED_ZERO_SHORT);
        }

        let mut rng = rand::thread_rng();
        Matrix {
            rows,
            cols,
            data: (0..rows * cols).map(|_| rng.sample(Standard)).collect(),
        }
    }

    /// Construct a new Matrix filled with random data, in the range [0, modulus).
    /// Faster than computing a random matrix and then computing the modulo.
    pub fn random_mod(rows: usize, cols: usize, modulus: u32) -> Self {
        if DETERMINISTIC {
            return Self::derive_from_seed(rows, cols, SEED_ZERO_SHORT) % modulus;
        }

        let mut rng = rand::thread_rng();
        let dist = Uniform::new(0, modulus);
        Matrix {
            rows,
            cols,
            data: (0..rows * cols).map(|_| rng.sample(dist)).collect(),
        }
    }

    /// Construct a new Matrix filled with random data, in the range [0, 2^logmod).
    /// Faster than computing a random matrix and then computing the modulo.
    pub fn random_logmod(rows: usize, cols: usize, logmod: u32) -> Self {
        assert!(logmod <= 32);
        if logmod == 32 {
            Self::random(rows, cols)
        } else {
            Self::random_mod(rows, cols, 1 << logmod)
        }
    }

    /// Construct a new Matrix filled with data sampled from a Gaussian with
    /// sigma = 6.4. Negative values are represented in the twos-complement
    /// form. For example, a value of -7 would be 2^32 - 7.
    pub fn gaussian(rows: usize, cols: usize) -> Self {
        if DETERMINISTIC {
            return Self::new(rows, cols);
        }

        Matrix {
            rows,
            cols,
            data: (0..rows * cols).map(|_| gauss_sample() as u32).collect(),
        }
    }

    /// Construct a new Matrix filled with data derived deterministically from
    /// the given seed, using ChaCha20. The seed must be 32 bytes.
    ///
    /// This should only be used to construct public, pseudorandom
    /// data, not anything private (like secret keys or queries).
    pub fn derive_from_seed(rows: usize, cols: usize, seed: [u8; 16]) -> Self {
        let mut data = vec![0u32; rows * cols];
        let data_u8 = unsafe {
            let ptr = data.as_mut_ptr() as *mut u8;
            let slice: &mut [u8] = slice::from_raw_parts_mut(ptr, data.len() * 4);
            slice
        };
        derive_with_aes(seed, data_u8);

        Matrix { rows, cols, data }
    }

    pub async fn derive_from_fn_seed<T, Fut>(
        rows: usize,
        cols: usize,
        seed: [u8; 16],
        derive: fn(&[u8; 16], u64, &mut [u8]) -> Fut,
    ) -> Self
    where
        Fut: Future<Output = T>,
        T: Sized,
    {
        let mut data = vec![0u32; rows * cols];
        let data_u8 = unsafe {
            let ptr = data.as_mut_ptr() as *mut u8;
            let slice: &mut [u8] = slice::from_raw_parts_mut(ptr, data.len() * 4);
            slice
        };

        derive(&seed, 0, data_u8).await;

        Matrix { rows, cols, data }
    }

    /// Get the slice of the data that the matrix contains.
    pub fn slc(&self) -> &[u32] {
        &self.data[0..self.rows * self.cols]
    }

    /// Get the mutable slice of the data that the matrix contains.
    pub fn mut_slc(&mut self) -> &mut [u32] {
        &mut self.data[0..self.rows * self.cols]
    }

    /// Apply the function to all values in the matrix.
    pub fn apply(&mut self, f: impl Fn(u32) -> u32) {
        for i in 0..self.rows * self.cols {
            self.data[i] = f(self.data[i]);
        }
    }

    /// Get a checksum of the values in the matrix.
    ///
    /// This is a simple XOR of all of the [`u32`] values.
    /// Useful for debugging.
    pub fn checksum(&self) -> u32 {
        if !COMPUTE_FULL_CHECKSUMS {
            return 0;
        }

        let mut c = 0;
        for i in 0..self.rows * self.cols {
            c ^= self.data[i];
        }
        c
    }

    /// Print a checksum with the given message.
    pub fn print_checksum(&self, msg: &str) {
        debug!("{}: {}", msg, self.checksum());
    }

    /// Print the dimensions of this matrix, along with the given message.
    pub fn print_dims(&self, msg: &str) {
        println!("{}: ({} x {})", msg, self.rows, self.cols);
    }
}

impl Clone for Matrix {
    fn clone(&self) -> Self {
        Self {
            rows: self.rows.clone(),
            cols: self.cols.clone(),
            data: self.data.clone(),
        }
    }
}

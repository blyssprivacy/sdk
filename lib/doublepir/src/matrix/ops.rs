use std::ops::{Add, AddAssign, Mul, MulAssign, Rem, RemAssign, Sub, SubAssign};

use super::Matrix;

// Addition

fn raw_mat_add(a: &[u32], b: &[u32], c: &mut [u32]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), c.len());

    for i in 0..a.len() {
        c[i] = a[i].wrapping_add(b[i]);
    }
}

fn raw_mat_add_assign(a: &mut [u32], b: &[u32]) {
    assert_eq!(a.len(), b.len());

    for i in 0..a.len() {
        a[i] = a[i].wrapping_add(b[i]);
    }
}

fn raw_mat_add_const(a: &[u32], b: u32, c: &mut [u32]) {
    assert_eq!(a.len(), c.len());

    for i in 0..a.len() {
        c[i] = a[i].wrapping_add(b);
    }
}

fn raw_mat_add_assign_const(a: &mut [u32], b: u32) {
    for i in 0..a.len() {
        a[i] = a[i].wrapping_add(b);
    }
}

impl Add for Matrix {
    type Output = Matrix;

    fn add(self, rhs: Self) -> Self::Output {
        assert_eq!(self.rows, rhs.rows);
        assert_eq!(self.cols, rhs.cols);

        let mut out = Matrix::new(self.rows, self.cols);
        raw_mat_add(self.slc(), rhs.slc(), out.mut_slc());
        out
    }
}

impl AddAssign for Matrix {
    fn add_assign(&mut self, rhs: Self) {
        assert_eq!(self.rows, rhs.rows);
        assert_eq!(self.cols, rhs.cols);

        raw_mat_add_assign(self.mut_slc(), rhs.slc());
    }
}

impl Add<u32> for Matrix {
    type Output = Matrix;

    fn add(self, rhs: u32) -> Self::Output {
        let mut out = Matrix::new(self.rows, self.cols);
        raw_mat_add_const(self.slc(), rhs, out.mut_slc());
        out
    }
}

impl AddAssign<u32> for Matrix {
    fn add_assign(&mut self, rhs: u32) {
        raw_mat_add_assign_const(self.mut_slc(), rhs);
    }
}

// Subtraction

fn raw_mat_sub(a: &[u32], b: &[u32], c: &mut [u32]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), c.len());

    for i in 0..a.len() {
        c[i] = a[i].wrapping_sub(b[i]);
    }
}

fn raw_mat_sub_assign(a: &mut [u32], b: &[u32]) {
    assert_eq!(a.len(), b.len());

    for i in 0..a.len() {
        a[i] = a[i].wrapping_sub(b[i]);
    }
}

fn raw_mat_sub_const(a: &[u32], b: u32, c: &mut [u32]) {
    assert_eq!(a.len(), c.len());

    for i in 0..a.len() {
        c[i] = a[i].wrapping_sub(b);
    }
}

fn raw_mat_sub_assign_const(a: &mut [u32], b: u32) {
    for i in 0..a.len() {
        a[i] = a[i].wrapping_sub(b);
    }
}

impl Sub for Matrix {
    type Output = Matrix;

    fn sub(self, rhs: Self) -> Self::Output {
        assert_eq!(self.rows, rhs.rows);
        assert_eq!(self.cols, rhs.cols);

        let mut out = Matrix::new(self.rows, self.cols);
        raw_mat_sub(self.slc(), rhs.slc(), out.mut_slc());
        out
    }
}

impl SubAssign for Matrix {
    fn sub_assign(&mut self, rhs: Self) {
        assert_eq!(self.rows, rhs.rows);
        assert_eq!(self.cols, rhs.cols);

        raw_mat_sub_assign(self.mut_slc(), rhs.slc());
    }
}

impl Sub<u32> for Matrix {
    type Output = Matrix;

    fn sub(self, rhs: u32) -> Self::Output {
        let mut out = Matrix::new(self.rows, self.cols);
        raw_mat_sub_const(self.slc(), rhs, out.mut_slc());
        out
    }
}

impl SubAssign<u32> for Matrix {
    fn sub_assign(&mut self, rhs: u32) {
        raw_mat_sub_assign_const(self.mut_slc(), rhs);
    }
}

// Multiplication

fn raw_mul_const(a: &[u32], b: u32, c: &mut [u32]) {
    assert_eq!(a.len(), c.len());

    for i in 0..a.len() {
        c[i] = a[i] * b;
    }
}

fn raw_mul_assign_const(a: &mut [u32], b: u32) {
    for i in 0..a.len() {
        a[i] *= b;
    }
}

fn raw_rem_assign_const(a: &mut [u32], b: u32) {
    for i in 0..a.len() {
        a[i] %= b;
    }
}

fn raw_mat_mul_add(
    a: &[u32],
    b: &[u32],
    c: &mut [u32],
    a_rows: usize,
    a_cols: usize,
    b_cols: usize,
) {
    assert_eq!(a.len(), a_rows * a_cols);
    assert_eq!(b.len(), a_cols * b_cols);
    assert_eq!(c.len(), a_rows * b_cols);

    for i in 0..a_rows {
        for k in 0..a_cols {
            for j in 0..b_cols {
                // c[b_cols * i + j] += a[a_cols * i + k] * b[b_cols * k + j];
                // c[b_cols * i + j] += a[a_cols * i + k].wrapping_mul(b[b_cols * k + j]);
                c[b_cols * i + j] = c[b_cols * i + j]
                    .wrapping_add(a[a_cols * i + k].wrapping_mul(b[b_cols * k + j]));
            }
        }
    }
}

// We purposely do not implement:
//      impl Mul for Matrix
// to discourage unnecessary copies.

impl Mul for &Matrix {
    type Output = Matrix;

    fn mul(self, rhs: Self) -> Self::Output {
        assert_eq!(self.cols, rhs.rows);

        let mut out = Matrix::new(self.rows, rhs.cols);
        raw_mat_mul_add(
            self.slc(),
            rhs.slc(),
            out.mut_slc(),
            self.rows,
            self.cols,
            rhs.cols,
        );
        out
    }
}

impl Mul<u32> for Matrix {
    type Output = Matrix;

    fn mul(self, rhs: u32) -> Self::Output {
        let mut out = Matrix::new(self.rows, self.cols);
        raw_mul_const(self.slc(), rhs, out.mut_slc());
        out
    }
}

impl MulAssign<u32> for Matrix {
    fn mul_assign(&mut self, rhs: u32) {
        raw_mul_assign_const(self.mut_slc(), rhs);
    }
}

// Modulo ("remainder", or "%")

impl Rem<u32> for Matrix {
    type Output = Matrix;

    fn rem(self, rhs: u32) -> Self::Output {
        let mut out = self.clone();
        out %= rhs;
        out
    }
}

impl RemAssign<u32> for Matrix {
    fn rem_assign(&mut self, rhs: u32) {
        raw_rem_assign_const(self.mut_slc(), rhs);
    }
}

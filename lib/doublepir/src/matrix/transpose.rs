use super::matrix::Matrix;

pub trait Transpose {
    /// Transposes the matrix in-place.
    fn transpose(&mut self);
}

impl Transpose for Matrix {
    fn transpose(&mut self) {
        let mut out = Matrix::new(self.cols, self.rows);
        for i in 0..self.rows {
            for j in 0..self.cols {
                out[j][i] = self[i][j];
            }
        }

        self.rows = out.rows;
        self.cols = out.cols;
        self.data = out.data;
    }
}

#[cfg(test)]
mod tests {
    use crate::matrix::{matrix::Matrix, transpose::Transpose};

    #[test]
    fn transpose_is_inverse_of_itself() {
        let m = Matrix::random(10, 35);
        let mut m2 = m.clone();
        m2.transpose();
        let mut m3 = m2.clone();
        m3.transpose();

        assert_eq!(m, m3);
    }
}

use std::ops::{Index, IndexMut, Range};

use super::Matrix;

pub struct MatrixRef<'a> {
    pub rows: usize,
    pub cols: usize,
    pub data: &'a [u32],
}

impl<'a> MatrixRef<'a> {
    pub fn to_owned_matrix(&self) -> Matrix {
        Matrix {
            rows: self.rows,
            cols: self.cols,
            data: self.data.to_owned(),
        }
    }

    pub fn rows(&self, start_row: usize, num_rows: usize) -> MatrixRef {
        let start = start_row * self.cols;
        let end = start + num_rows * self.cols;
        MatrixRef {
            rows: num_rows,
            cols: self.cols,
            data: &self.data[start..end],
        }
    }
}

impl Matrix {
    pub fn column(&self, col: usize) -> Matrix {
        let mut out = Matrix::new(self.rows, 1);
        for i in 0..self.rows {
            out[i][0] = self[i][col];
        }
        out
    }

    pub fn rows(&self, start_row: usize, num_rows: usize) -> MatrixRef {
        let start = start_row * self.cols;
        let end = start + num_rows * self.cols;
        MatrixRef {
            rows: num_rows,
            cols: self.cols,
            data: &self.data[start..end],
        }
    }

    pub fn drop_last_rows(&mut self, num_rows_to_drop: usize) {
        assert!(self.rows > num_rows_to_drop);

        self.rows -= num_rows_to_drop;
        self.data.truncate(self.rows * self.cols);
    }

    pub fn append_zeros(&mut self, n: usize) {
        self.concat(&Matrix::new(n, 1));
    }

    pub fn concat(&mut self, other: &Matrix) {
        if self.rows == 0 && self.cols == 0 {
            self.rows = other.rows;
            self.cols = other.cols;
            self.data = other.data.clone();
            return;
        }

        assert_eq!(self.cols, other.cols);

        self.rows += other.rows;
        self.data.extend(&other.data);
    }

    pub fn concat_ref(&mut self, other: &MatrixRef) {
        assert_eq!(self.cols, other.cols);

        self.rows += other.rows;
        self.data.extend(other.data);
    }

    pub fn concat_cols(&mut self, n: usize) {
        if n == 1 {
            return;
        }

        assert_eq!(self.cols % n, 0);

        let mut out = Matrix::new(self.rows * n, self.cols / n);
        for i in 0..self.rows {
            for j in 0..self.cols {
                let col = j / n;
                let row = i + self.rows * (j % n);
                out[row][col] = self[i][j];
            }
        }

        self.rows = out.rows;
        self.cols = out.cols;
        self.data = out.data;
    }

    pub fn map(&mut self, f: fn(u32) -> u32) {
        for i in 0..self.rows * self.cols {
            self.data[i] = f(self.data[i]);
        }
    }

    pub fn as_matrix_ref(&self) -> MatrixRef {
        MatrixRef {
            rows: self.rows,
            cols: self.cols,
            data: self.data.as_ref(),
        }
    }

    pub fn transpose_expand_concat_cols_squish(
        &mut self,
        modulus: u64,
        delta: usize,
        concat: usize,
        basis: u64,
        d: usize,
    ) {
        let mut out = Matrix::new(self.cols * delta * concat, (self.rows / concat + d - 1) / d);

        for j in 0..self.rows {
            for i in 0..self.cols {
                let mut val = self.data[i + j * self.cols] as u64;
                for f in 0..delta {
                    let new_val = val % modulus;
                    let r = (i * delta + f) + self.cols * delta * (j % concat);
                    let c = j / concat;
                    out.data[r * out.cols + c / d] += (new_val << (basis * (c % d) as u64)) as u32;
                    val /= modulus;
                }
            }
        }

        self.rows = out.rows;
        self.cols = out.cols;
        self.data = out.data;
    }
}

impl Index<usize> for Matrix {
    type Output = [u32];

    fn index(&self, index: usize) -> &Self::Output {
        let start = index * self.cols;
        &self.data[start..start + self.cols]
    }
}

impl IndexMut<usize> for Matrix {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        let start = index * self.cols;
        &mut self.data[start..start + self.cols]
    }
}

impl Index<Range<usize>> for Matrix {
    type Output = [u32];

    fn index(&self, index: Range<usize>) -> &Self::Output {
        &self.data[index.start * self.cols..index.end * self.cols]
    }
}

impl IndexMut<Range<usize>> for Matrix {
    fn index_mut(&mut self, index: Range<usize>) -> &mut Self::Output {
        &mut self.data[index.start * self.cols..index.end * self.cols]
    }
}

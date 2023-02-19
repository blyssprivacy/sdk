use std::time::Instant;

use crate::info;

use super::{Matrix, MatrixRef};
use rayon::prelude::*;

// Hard-coded, to allow for compiler optimizations:
const COMPRESSION: usize = 3;
const BASIS: u32 = 10;
const BASIS2: u32 = BASIS * 2;
const MASK: u32 = (1 << BASIS) - 1;

fn raw_mat_mul_vec_packed(out: &mut [u32], a: &[u32], b: &[u32], a_rows: usize, a_cols: usize) {
    let start = Instant::now();
    let (mut db, mut db2, mut db3, mut db4, mut db5, mut db6, mut db7, mut db8);
    let (mut val, mut val2, mut val3, mut val4, mut val5, mut val6, mut val7, mut val8);
    let (mut tmp, mut tmp2, mut tmp3, mut tmp4, mut tmp5, mut tmp6, mut tmp7, mut tmp8);

    let mut index = 0usize;
    let mut index2;

    for i in (0..a_rows).step_by(8) {
        tmp = 0;
        tmp2 = 0;
        tmp3 = 0;
        tmp4 = 0;
        tmp5 = 0;
        tmp6 = 0;
        tmp7 = 0;
        tmp8 = 0;

        index2 = 0;
        for _ in 0..a_cols {
            db = a[index];
            db2 = a[index + 1 * a_cols];
            db3 = a[index + 2 * a_cols];
            db4 = a[index + 3 * a_cols];
            db5 = a[index + 4 * a_cols];
            db6 = a[index + 5 * a_cols];
            db7 = a[index + 6 * a_cols];
            db8 = a[index + 7 * a_cols];

            val = db & MASK;
            val2 = db2 & MASK;
            val3 = db3 & MASK;
            val4 = db4 & MASK;
            val5 = db5 & MASK;
            val6 = db6 & MASK;
            val7 = db7 & MASK;
            val8 = db8 & MASK;
            tmp += val * b[index2];
            tmp2 += val2 * b[index2];
            tmp3 += val3 * b[index2];
            tmp4 += val4 * b[index2];
            tmp5 += val5 * b[index2];
            tmp6 += val6 * b[index2];
            tmp7 += val7 * b[index2];
            tmp8 += val8 * b[index2];
            index2 += 1;

            val = (db >> BASIS) & MASK;
            val2 = (db2 >> BASIS) & MASK;
            val3 = (db3 >> BASIS) & MASK;
            val4 = (db4 >> BASIS) & MASK;
            val5 = (db5 >> BASIS) & MASK;
            val6 = (db6 >> BASIS) & MASK;
            val7 = (db7 >> BASIS) & MASK;
            val8 = (db8 >> BASIS) & MASK;
            tmp += val * b[index2];
            tmp2 += val2 * b[index2];
            tmp3 += val3 * b[index2];
            tmp4 += val4 * b[index2];
            tmp5 += val5 * b[index2];
            tmp6 += val6 * b[index2];
            tmp7 += val7 * b[index2];
            tmp8 += val8 * b[index2];
            index2 += 1;

            val = (db >> BASIS2) & MASK;
            val2 = (db2 >> BASIS2) & MASK;
            val3 = (db3 >> BASIS2) & MASK;
            val4 = (db4 >> BASIS2) & MASK;
            val5 = (db5 >> BASIS2) & MASK;
            val6 = (db6 >> BASIS2) & MASK;
            val7 = (db7 >> BASIS2) & MASK;
            val8 = (db8 >> BASIS2) & MASK;
            tmp += val * b[index2];
            tmp2 += val2 * b[index2];
            tmp3 += val3 * b[index2];
            tmp4 += val4 * b[index2];
            tmp5 += val5 * b[index2];
            tmp6 += val6 * b[index2];
            tmp7 += val7 * b[index2];
            tmp8 += val8 * b[index2];
            index2 += 1;
            index += 1;
        }
        out[i] += tmp;
        out[i + 1] += tmp2;
        out[i + 2] += tmp3;
        out[i + 3] += tmp4;
        out[i + 4] += tmp5;
        out[i + 5] += tmp6;
        out[i + 6] += tmp7;
        out[i + 7] += tmp8;
        index += a_cols * 7;
    }
    info!(
        "raw_mat_mul_vec_packed took {} us",
        start.elapsed().as_micros()
    );
}

const MIN_THREAD_SIZE: usize = 16;
const THREADS_TO_USE: usize = 2;

pub fn matrix_mul_vec_packed(
    a: &MatrixRef,
    b: &MatrixRef,
    basis: u64,
    compression: usize,
) -> Matrix {
    let start = Instant::now();
    assert_eq!(
        a.cols * compression,
        b.rows,
        "a.cols {} compression {} b.rows {}",
        a.cols,
        compression,
        b.rows
    );
    assert_eq!(b.cols, 1);
    assert_eq!(basis, 10);
    assert_eq!(compression, 3);

    let mut out = Matrix::new(a.rows + 8, 1);

    let a_rows_rounded_down = (a.rows / 8) * 8;
    if a_rows_rounded_down >= MIN_THREAD_SIZE {
        // println!("using {} threads", rayon::current_num_threads());
        let start = Instant::now();
        let chunk_size = a_rows_rounded_down / THREADS_TO_USE; //rayon::current_num_threads();
        let chunk_size = (chunk_size + 8 - 1 / 8) * 8;
        let out_data_chunks = (&mut out.data[0..a_rows_rounded_down]).par_chunks_mut(chunk_size);
        let a_data_chunks = a.data.par_chunks(chunk_size * a.cols);
        let work_chunks = out_data_chunks.zip(a_data_chunks);
        work_chunks.for_each(|(out_data, a_data)| {
            raw_mat_mul_vec_packed(out_data, a_data, &b.data, out_data.len(), a.cols);
        });
        println!("threaded mul took {} us", start.elapsed().as_micros());
    } else {
        let start = Instant::now();
        raw_mat_mul_vec_packed(&mut out.data, &a.data, &b.data, a_rows_rounded_down, a.cols);
        println!("non-threaded mul took {} us", start.elapsed().as_micros());
    }

    if a_rows_rounded_down < a.rows {
        let diff = a.rows - a_rows_rounded_down;
        let mut tmp = vec![0u32; 8 * a.cols];
        (&mut tmp[..diff * a.cols]).copy_from_slice(&a.data[a_rows_rounded_down * a.cols..]);
        raw_mat_mul_vec_packed(
            &mut out.data[a_rows_rounded_down..],
            &tmp,
            &b.data,
            8,
            a.cols,
        );
    }

    out.drop_last_rows(8);
    info!(
        "matrix_mul_vec_packed took {} us",
        start.elapsed().as_micros()
    );

    out
}

fn raw_matrix_mul_transposed_packed(
    out: &mut [u32],
    a: &[u32],
    b: &[u32],
    a_rows: usize,
    a_cols: usize,
    b_rows: usize,
    b_cols: usize,
) {
    let (mut val, mut tmp, mut db);
    let (mut tmp2, mut tmp3, mut tmp4, mut tmp5, mut tmp6, mut tmp7, mut tmp8);
    let (mut val2, mut val3);
    let (mut ind1, mut ind2);

    if a_rows > a_cols {
        // when the database rows are long
        ind1 = 0;
        for i in 0..a_rows {
            for k in 0..a_cols {
                db = a[ind1];
                ind1 += 1;
                val = db & MASK;
                val2 = (db >> BASIS) & MASK;
                val3 = (db >> BASIS2) & MASK;
                for j in 0..b_rows {
                    out[b_rows * i + j] += val * b[k * COMPRESSION + j * b_cols];
                    out[b_rows * i + j] += val2 * b[k * COMPRESSION + j * b_cols + 1];
                    out[b_rows * i + j] += val3 * b[k * COMPRESSION + j * b_cols + 2];
                }
            }
        }
    } else {
        // when the database rows are short
        for j in (0..b_rows).step_by(8) {
            ind1 = 0;
            for i in 0..a_rows {
                tmp = 0;
                tmp2 = 0;
                tmp3 = 0;
                tmp4 = 0;
                tmp5 = 0;
                tmp6 = 0;
                tmp7 = 0;
                tmp8 = 0;
                ind2 = 0;
                for _ in 0..a_cols {
                    db = a[ind1];
                    ind1 += 1;
                    for m in 0..(COMPRESSION as u32) {
                        val = (db >> (m * BASIS)) & MASK;
                        tmp += val * b[ind2 + (j + 0) * b_cols];
                        tmp2 += val * b[ind2 + (j + 1) * b_cols];
                        tmp3 += val * b[ind2 + (j + 2) * b_cols];
                        tmp4 += val * b[ind2 + (j + 3) * b_cols];
                        tmp5 += val * b[ind2 + (j + 4) * b_cols];
                        tmp6 += val * b[ind2 + (j + 5) * b_cols];
                        tmp7 += val * b[ind2 + (j + 6) * b_cols];
                        tmp8 += val * b[ind2 + (j + 7) * b_cols];
                        ind2 += 1;
                    }
                }
                out[b_rows * i + j + 0] = tmp;
                out[b_rows * i + j + 1] = tmp2;
                out[b_rows * i + j + 2] = tmp3;
                out[b_rows * i + j + 3] = tmp4;
                out[b_rows * i + j + 4] = tmp5;
                out[b_rows * i + j + 5] = tmp6;
                out[b_rows * i + j + 6] = tmp7;
                out[b_rows * i + j + 7] = tmp8;
            }
        }
    }
}

pub fn matrix_mul_transposed_packed(
    a: &MatrixRef,
    b: &MatrixRef,
    basis: u64,
    compression: usize,
) -> Matrix {
    assert_eq!(basis, 10);
    assert_eq!(compression, 3);

    let mut out = Matrix::new(a.rows, b.rows);
    // println!(
    //     "matrix_mul_transposed_packed: ({} x {}), ({} x {})",
    //     a.rows, a.cols, b.rows, b.cols
    // );
    raw_matrix_mul_transposed_packed(
        &mut out.data,
        &a.data,
        &b.data,
        a.rows,
        a.cols,
        b.rows,
        b.cols,
    );
    out
}

#[cfg(target_feature = "avx2")]
use std::arch::x86_64::*;

use spiral_rs::arith::*;
use spiral_rs::params::*;
use spiral_rs::poly::*;

use crate::db::sparse_db::*;

use rayon::prelude::{IndexedParallelIterator, IntoParallelRefMutIterator, ParallelIterator};

pub const MAX_SUMMED: usize = 1 << 6;
pub const PACKED_OFFSET_2: i32 = 32;

fn reduce_poly_4(params: &Params, out_slice: &mut PolyMatrixNTT, z: usize, poly_len: usize) {
    for z_in in 0..4 {
        out_slice.data[z + z_in] = barrett_coeff_u64(params, out_slice.data[z + z_in], 0);
        out_slice.data[poly_len + z + z_in] =
            barrett_coeff_u64(params, out_slice.data[poly_len + z + z_in], 1);
        out_slice.data[2 * poly_len + z + z_in] =
            barrett_coeff_u64(params, out_slice.data[2 * poly_len + z + z_in], 0);
        out_slice.data[3 * poly_len + z + z_in] =
            barrett_coeff_u64(params, out_slice.data[3 * poly_len + z + z_in], 1);
    }
}

unsafe fn compute_single_out_poly(
    params: &Params,
    query: &[u64],
    db_row: &[u64],
    j: usize,
    it: usize,
    out_slice: &mut PolyMatrixNTT,
    reduce: bool,
) {
    let poly_len = params.poly_len;

    let b_poly = db_row.get_unchecked(it * poly_len) as *const u64;
    for z in (0..poly_len).step_by(4) {
        let v_a1 = query.get_unchecked((j * 2) * poly_len + z) as *const u64;
        let v_a2 = query.get_unchecked((j * 2 + 1) * poly_len + z) as *const u64;
        let v_b = b_poly.add(z);

        let a1 = _mm256_load_si256(v_a1 as *const __m256i);
        let a2 = _mm256_load_si256(v_a2 as *const __m256i);
        let b = _mm256_load_si256(v_b as *const __m256i);

        let a1_lo = a1;
        let a1_hi = _mm256_srli_epi64(a1, PACKED_OFFSET_2);
        let a2_lo = a2;
        let a2_hi = _mm256_srli_epi64(a2, PACKED_OFFSET_2);
        let b_lo = b;
        let b_hi = _mm256_srli_epi64(b, PACKED_OFFSET_2);

        let c1_lo_loc = ((&mut out_slice.data[z]) as *mut u64) as *mut __m256i;
        let c1_hi_loc = ((&mut out_slice.data[poly_len + z]) as *mut u64) as *mut __m256i;
        let c2_lo_loc = ((&mut out_slice.data[2 * poly_len + z]) as *mut u64) as *mut __m256i;
        let c2_hi_loc = ((&mut out_slice.data[3 * poly_len + z]) as *mut u64) as *mut __m256i;

        let mut c1_lo = _mm256_load_si256(c1_lo_loc);
        let mut c1_hi = _mm256_load_si256(c1_hi_loc);
        let mut c2_lo = _mm256_load_si256(c2_lo_loc);
        let mut c2_hi = _mm256_load_si256(c2_hi_loc);

        c1_lo = _mm256_add_epi64(c1_lo, _mm256_mul_epu32(a1_lo, b_lo));
        c1_hi = _mm256_add_epi64(c1_hi, _mm256_mul_epu32(a1_hi, b_hi));
        c2_lo = _mm256_add_epi64(c2_lo, _mm256_mul_epu32(a2_lo, b_lo));
        c2_hi = _mm256_add_epi64(c2_hi, _mm256_mul_epu32(a2_hi, b_hi));

        _mm256_store_si256(c1_lo_loc, c1_lo);
        _mm256_store_si256(c1_hi_loc, c1_hi);
        _mm256_store_si256(c2_lo_loc, c2_lo);
        _mm256_store_si256(c2_hi_loc, c2_hi);

        if reduce {
            reduce_poly_4(params, out_slice, z, poly_len);
        }
    }
}

#[cfg(target_feature = "avx2")]
pub fn multiply_reg_by_sparsedb(
    out: &mut Vec<PolyMatrixNTT>,
    sparse_db: &SparseDb,
    query: &[u64],
    params: &Params,
    dim0: usize,
    num_per: usize,
    inst_trials: usize,
) {
    //    db:  (num_per, dim0) -> (inst_trials, poly_len)
    // query:  [dim0, ct_rows, poly_len]
    //   out:  [inst_trials, num_per, PolyMatrixNTT]

    let poly_len = params.poly_len;
    let crt_count = params.crt_count;
    assert_eq!(crt_count, 2);

    let mut adds = 0;

    for j in 0..dim0 {
        for i in 0..num_per {
            let full_idx = j * num_per + i;
            let db_row = sparse_db.get_item(full_idx);
            if db_row.is_none() {
                continue;
            }
            let db_row = db_row.unwrap();

            let reduce = adds >= MAX_SUMMED;
            out.par_iter_mut()
                .skip(i)
                .step_by(num_per)
                .enumerate()
                .for_each(|(it, out_slice)| unsafe {
                    compute_single_out_poly(params, query, db_row, j, it, out_slice, reduce);
                });
        }
        adds += 1;
        if adds >= MAX_SUMMED {
            adds = 0;
        }
    }

    for i in 0..(num_per * inst_trials) {
        let out_slice = &mut out[i];
        for z in (0..poly_len).step_by(4) {
            reduce_poly_4(params, out_slice, z, poly_len);
        }
    }
}

#[cfg(not(target_feature = "avx2"))]
pub fn multiply_reg_by_sparse_database(
    out: &mut Vec<PolyMatrixNTT>,
    db: &SparseDb,
    query: &[u64],
    params: &Params,
    dim0: usize,
    num_per: usize,
    db_idx: usize,
) {
    //    db:  [inst_trials, num_per, dim0, poly_len]
    // query:  [dim0, ct_rows, poly_len]

    let poly_len = params.poly_len;
    let crt_count = params.crt_count;
    assert_eq!(crt_count, 2);

    let lo_mask = (1 << PACKED_OFFSET_2) - 1;

    for j in 0..dim0 {
        let mut adds = 0;

        for i in 0..num_per {
            let (part_0, part_1) = out[i].data.as_mut_slice().split_at_mut(2 * poly_len);
            let (out_0, out_1) = part_0.split_at_mut(poly_len);
            let (out_2, out_3) = part_1.split_at_mut(poly_len);

            let full_idx = db_idx * (dim0 * num_per) + j * num_per + i;
            let result = db.get_idx(full_idx);
            if result.is_none() {
                continue;
            }
            let real_idx = *result.unwrap();

            let b_poly = db.data[real_idx].as_slice();

            for z in 0..poly_len {
                let a1 = query[(j * 2) * poly_len + z];
                let a2 = query[(j * 2 + 1) * poly_len + z];
                let b = b_poly[z];

                let a1_lo = (a1 & lo_mask) as u32;
                let a1_hi = (a1 >> PACKED_OFFSET_2) as u32;
                let a2_lo = (a2 & lo_mask) as u32;
                let a2_hi = (a2 >> PACKED_OFFSET_2) as u32;
                let b_lo = (b & lo_mask) as u32;
                let b_hi = (b >> PACKED_OFFSET_2) as u32;

                let c1_lo_loc = &mut out_0[z];
                let c1_hi_loc = &mut out_1[z];
                let c2_lo_loc = &mut out_2[z];
                let c2_hi_loc = &mut out_3[z];

                *c1_lo_loc += (a1_lo as u64) * (b_lo as u64);
                *c1_hi_loc += (a1_hi as u64) * (b_hi as u64);
                *c2_lo_loc += (a2_lo as u64) * (b_lo as u64);
                *c2_hi_loc += (a2_hi as u64) * (b_hi as u64);

                if adds >= MAX_SUMMED {
                    *c1_lo_loc = barrett_coeff_u64(params, *c1_lo_loc, 0);
                    *c1_hi_loc = barrett_coeff_u64(params, *c1_hi_loc, 1);
                    *c2_lo_loc = barrett_coeff_u64(params, *c2_lo_loc, 0);
                    *c2_hi_loc = barrett_coeff_u64(params, *c2_hi_loc, 1);
                }
            }

            adds += 1;

            if adds >= MAX_SUMMED {
                adds = 0;
            }
        }
    }

    for i in 0..num_per {
        for z in 0..poly_len {
            out[i].data[z] = barrett_coeff_u64(params, out[i].data[z], 0);
            out[i].data[poly_len + z] = barrett_coeff_u64(params, out[i].data[poly_len + z], 1);
            out[i].data[2 * poly_len + z] =
                barrett_coeff_u64(params, out[i].data[2 * poly_len + z], 0);
            out[i].data[3 * poly_len + z] =
                barrett_coeff_u64(params, out[i].data[3 * poly_len + z], 1);
        }
    }
}

#[cfg(not(target_feature = "avx2"))]
pub fn multiply_reg_by_database(
    out: &mut Vec<PolyMatrixNTT>,
    db: &[u64],
    v_firstdim: &[u64],
    params: &Params,
    dim0: usize,
    num_per: usize,
) {
    let ct_rows = 2;
    let ct_cols = 1;
    let pt_rows = 1;
    let pt_cols = 1;

    for z in 0..params.poly_len {
        let idx_a_base = z * (ct_cols * dim0 * ct_rows);
        let mut idx_b_base = z * (num_per * pt_cols * dim0 * pt_rows);

        for i in 0..num_per {
            for c in 0..pt_cols {
                let mut sums_out_n0_0 = 0u128;
                let mut sums_out_n0_1 = 0u128;
                let mut sums_out_n1_0 = 0u128;
                let mut sums_out_n1_1 = 0u128;

                for jm in 0..(dim0 * pt_rows) {
                    let b = db[idx_b_base];
                    idx_b_base += 1;

                    let v_a0 = v_firstdim[idx_a_base + jm * ct_rows];
                    let v_a1 = v_firstdim[idx_a_base + jm * ct_rows + 1];

                    let b_lo = b as u32;
                    let b_hi = (b >> 32) as u32;

                    let v_a0_lo = v_a0 as u32;
                    let v_a0_hi = (v_a0 >> 32) as u32;

                    let v_a1_lo = v_a1 as u32;
                    let v_a1_hi = (v_a1 >> 32) as u32;

                    // do n0
                    sums_out_n0_0 += ((v_a0_lo as u64) * (b_lo as u64)) as u128;
                    sums_out_n0_1 += ((v_a1_lo as u64) * (b_lo as u64)) as u128;

                    // do n1
                    sums_out_n1_0 += ((v_a0_hi as u64) * (b_hi as u64)) as u128;
                    sums_out_n1_1 += ((v_a1_hi as u64) * (b_hi as u64)) as u128;
                }

                // output n0
                let (crt_count, poly_len) = (params.crt_count, params.poly_len);
                let mut n = 0;
                let mut idx_c = c * (crt_count * poly_len) + n * (poly_len) + z;
                out[i].data[idx_c] = (sums_out_n0_0 % (params.moduli[0] as u128)) as u64;
                idx_c += pt_cols * crt_count * poly_len;
                out[i].data[idx_c] = (sums_out_n0_1 % (params.moduli[0] as u128)) as u64;

                // output n1
                n = 1;
                idx_c = c * (crt_count * poly_len) + n * (poly_len) + z;
                out[i].data[idx_c] = (sums_out_n1_0 % (params.moduli[1] as u128)) as u64;
                idx_c += pt_cols * crt_count * poly_len;
                out[i].data[idx_c] = (sums_out_n1_1 % (params.moduli[1] as u128)) as u64;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::time::Instant;

    use super::*;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha20Rng;
    use spiral_rs::util;

    use crate::db::aligned_memory::*;

    fn get_params() -> Params {
        let cfg = r#"
            {'n': 4,
            'nu_1': 9,
            'nu_2': 5,
            'p': 256,
            'q2_bits': 20,
            't_gsw': 9,
            't_conv': 4,
            't_exp_left': 8,
            't_exp_right': 28,
            'instances': 2,
            'db_item_size': 65536 }
        "#;
        util::params_from_json(&cfg.replace("'", "\""))
    }

    #[test]
    fn multiply_reg_by_sparse_database_perf() {
        let params = get_params();

        let mut rng = ChaCha20Rng::from_entropy();

        let dim0 = 1 << params.db_dim_1;
        let num_per = 1 << params.db_dim_2;

        let v_reg_sz = dim0 * 2 * params.poly_len;
        let rand_v_reg = PolyMatrixRaw::random(&params, dim0 * 2, 1);
        let mut v_reg_reoriented = AlignedMemory64::new(v_reg_sz);
        v_reg_reoriented
            .as_mut_slice()
            .copy_from_slice(rand_v_reg.as_slice());

        let mut out = Vec::with_capacity(num_per);
        for _ in 0..dim0 {
            out.push(PolyMatrixNTT::zero(&params, 2, 1));
        }

        let inst_trials = params.instances * params.n * params.n;
        let db_row_size = params.poly_len * inst_trials * std::mem::size_of::<u64>();
        let db = SparseDb::new(None, db_row_size);
        let total_idx_sz = params.instances * params.n * params.n * dim0 * num_per;
        println!("total_idx_sz: {}", total_idx_sz);
        let mut data = vec![0u64; params.poly_len];
        let mut insertion_time_sum: u64 = 0;
        const N_INSERTIONS: usize = 100;
        for _ in 0..N_INSERTIONS {
            let rand_idx = rng.gen::<usize>() % total_idx_sz;
            let mut db_item = PolyMatrixRaw::random(&params, 1, 1);
            for z in 0..params.poly_len {
                db_item.data[z] &= 255;
            }
            let start = Instant::now();
            // db_item.reduce_mod(params.pt_modulus);
            for z in 0..params.poly_len {
                db_item.data[z] = recenter_mod(db_item.data[z], params.pt_modulus, params.modulus);
            }
            let db_item_ntt = db_item.ntt();

            for z in 0..params.poly_len {
                data[z] = db_item_ntt.data[z]
                    | (db_item_ntt.data[params.poly_len + z] << PACKED_OFFSET_2);
            }
            db.upsert(rand_idx, data.as_slice());
            insertion_time_sum += start.elapsed().as_micros() as u64;
        }
        println!(
            "Avg insertion time: {:.0} us",
            insertion_time_sum as f64 / N_INSERTIONS as f64
        );

        let start = Instant::now();
        multiply_reg_by_sparsedb(
            &mut out,
            &db,
            v_reg_reoriented.as_slice(),
            &params,
            dim0,
            num_per,
            inst_trials,
        );
        println!("Mul took {} us", start.elapsed().as_micros())
    }
}

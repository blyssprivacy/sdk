#[cfg(target_feature = "avx2")]
use std::arch::x86_64::*;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::time::Instant;

use crate::aligned_memory::*;
use crate::arith::*;
use crate::client::PublicParameters;
use crate::client::Query;
use crate::gadget::*;
use crate::params::*;
use crate::poly::*;
use crate::util::*;

use rayon::prelude::*;

pub fn coefficient_expansion(
    v: &mut Vec<PolyMatrixNTT>,
    g: usize,
    stop_round: usize,
    params: &Params,
    v_w_left: &Vec<PolyMatrixNTT>,
    v_w_right: &Vec<PolyMatrixNTT>,
    v_neg1: &Vec<PolyMatrixNTT>,
    max_bits_to_gen_right: usize,
) {
    let poly_len = params.poly_len;

    for r in 0..g {
        let num_in = 1 << r;
        let num_out = 2 * num_in;

        let t = (poly_len / (1 << r)) + 1;

        let neg1 = &v_neg1[r];

        let action_expand = |(i, v_i): (usize, &mut PolyMatrixNTT)| {
            if (stop_round > 0 && r > stop_round && (i % 2) == 1)
                || (stop_round > 0
                    && r == stop_round
                    && (i % 2) == 1
                    && (i / 2) >= max_bits_to_gen_right)
            {
                return;
            }

            let mut ct = PolyMatrixRaw::zero(params, 2, 1);
            let mut ct_auto = PolyMatrixRaw::zero(params, 2, 1);
            let mut ct_auto_1 = PolyMatrixRaw::zero(params, 1, 1);
            let mut ct_auto_1_ntt = PolyMatrixNTT::zero(params, 1, 1);
            let mut w_times_ginv_ct = PolyMatrixNTT::zero(params, 2, 1);

            let mut ginv_ct_left = PolyMatrixRaw::zero(params, params.t_exp_left, 1);
            let mut ginv_ct_left_ntt = PolyMatrixNTT::zero(params, params.t_exp_left, 1);
            let mut ginv_ct_right = PolyMatrixRaw::zero(params, params.t_exp_right, 1);
            let mut ginv_ct_right_ntt = PolyMatrixNTT::zero(params, params.t_exp_right, 1);

            let (w, _gadget_dim, gi_ct, gi_ct_ntt) = match i % 2 {
                0 => (
                    &v_w_left[r],
                    params.t_exp_left,
                    &mut ginv_ct_left,
                    &mut ginv_ct_left_ntt,
                ),
                1 | _ => (
                    &v_w_right[r],
                    params.t_exp_right,
                    &mut ginv_ct_right,
                    &mut ginv_ct_right_ntt,
                ),
            };

            // if i < num_in {
            //     let (src, dest) = v.split_at_mut(num_in);
            //     scalar_multiply(&mut dest[i], neg1, &src[i]);
            // }

            from_ntt(&mut ct, &v_i);
            automorph(&mut ct_auto, &ct, t);
            gadget_invert_rdim(gi_ct, &ct_auto, 1);
            to_ntt_no_reduce(gi_ct_ntt, &gi_ct);
            ct_auto_1
                .data
                .as_mut_slice()
                .copy_from_slice(ct_auto.get_poly(1, 0));
            to_ntt(&mut ct_auto_1_ntt, &ct_auto_1);
            multiply(&mut w_times_ginv_ct, w, &gi_ct_ntt);

            let mut idx = 0;
            for j in 0..2 {
                for n in 0..params.crt_count {
                    for z in 0..poly_len {
                        let sum = (*v_i).data[idx]
                            + w_times_ginv_ct.data[idx]
                            + j * ct_auto_1_ntt.data[n * poly_len + z];
                        (*v_i).data[idx] = barrett_coeff_u64(params, sum, n);
                        idx += 1;
                    }
                }
            }
        };

        let (src, dest) = v.split_at_mut(num_in);
        src.par_iter_mut()
            .zip(dest.par_iter_mut())
            .for_each(|(s, d)| {
                scalar_multiply(d, neg1, s);
            });

        v[0..num_in]
            .par_iter_mut()
            .enumerate()
            .for_each(action_expand);
        v[num_in..num_out]
            .par_iter_mut()
            .enumerate()
            .for_each(action_expand);
    }
}

pub fn regev_to_gsw<'a>(
    v_gsw: &mut Vec<PolyMatrixNTT<'a>>,
    v_inp: &Vec<PolyMatrixNTT<'a>>,
    v: &PolyMatrixNTT<'a>,
    params: &'a Params,
    idx_factor: usize,
    idx_offset: usize,
) {
    assert!(v.rows == 2);
    assert!(v.cols == 2 * params.t_conv);

    v_gsw.par_iter_mut().enumerate().for_each(|(i, ct)| {
        let mut ginv_c_inp = PolyMatrixRaw::zero(params, 2 * params.t_conv, 1);
        let mut ginv_c_inp_ntt = PolyMatrixNTT::zero(params, 2 * params.t_conv, 1);
        let mut tmp_ct_raw = PolyMatrixRaw::zero(params, 2, 1);
        let mut tmp_ct = PolyMatrixNTT::zero(params, 2, 1);

        for j in 0..params.t_gsw {
            let idx_ct = i * params.t_gsw + j;
            let idx_inp = idx_factor * (idx_ct) + idx_offset;
            ct.copy_into(&v_inp[idx_inp], 0, 2 * j + 1);
            from_ntt(&mut tmp_ct_raw, &v_inp[idx_inp]);
            gadget_invert(&mut ginv_c_inp, &tmp_ct_raw);
            to_ntt(&mut ginv_c_inp_ntt, &ginv_c_inp);
            multiply(&mut tmp_ct, v, &ginv_c_inp_ntt);
            ct.copy_into(&tmp_ct, 0, 2 * j);
        }
    });
}

pub const MAX_SUMMED: usize = 1 << 6;
pub const PACKED_OFFSET_2: i32 = 32;

#[cfg(target_feature = "avx2")]
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

    assert!(dim0 * ct_rows >= MAX_SUMMED);

    let mut sums_out_n0_u64 = AlignedMemory64::new(4);
    let mut sums_out_n2_u64 = AlignedMemory64::new(4);

    for z in 0..params.poly_len {
        let idx_a_base = z * (ct_cols * dim0 * ct_rows);
        let mut idx_b_base = z * (num_per * pt_cols * dim0 * pt_rows);

        for i in 0..num_per {
            for c in 0..pt_cols {
                let inner_limit = MAX_SUMMED;
                let outer_limit = dim0 * ct_rows / inner_limit;

                let mut sums_out_n0_u64_acc = [0u64, 0, 0, 0];
                let mut sums_out_n2_u64_acc = [0u64, 0, 0, 0];

                for o_jm in 0..outer_limit {
                    unsafe {
                        let mut sums_out_n0 = _mm256_setzero_si256();
                        let mut sums_out_n2 = _mm256_setzero_si256();

                        for i_jm in 0..inner_limit / 4 {
                            let jm = o_jm * inner_limit + (4 * i_jm);

                            let b_inp_1 = *db.get_unchecked(idx_b_base) as i64;
                            idx_b_base += 1;
                            let b_inp_2 = *db.get_unchecked(idx_b_base) as i64;
                            idx_b_base += 1;
                            let b = _mm256_set_epi64x(b_inp_2, b_inp_2, b_inp_1, b_inp_1);

                            let v_a = v_firstdim.get_unchecked(idx_a_base + jm) as *const u64;

                            let a = _mm256_load_si256(v_a as *const __m256i);
                            let a_lo = a;
                            let a_hi_hi = _mm256_srli_epi64(a, PACKED_OFFSET_2);
                            let b_lo = b;
                            let b_hi_hi = _mm256_srli_epi64(b, PACKED_OFFSET_2);

                            sums_out_n0 =
                                _mm256_add_epi64(sums_out_n0, _mm256_mul_epu32(a_lo, b_lo));
                            sums_out_n2 =
                                _mm256_add_epi64(sums_out_n2, _mm256_mul_epu32(a_hi_hi, b_hi_hi));
                        }

                        // reduce here, otherwise we will overflow

                        _mm256_store_si256(
                            sums_out_n0_u64.as_mut_ptr() as *mut __m256i,
                            sums_out_n0,
                        );
                        _mm256_store_si256(
                            sums_out_n2_u64.as_mut_ptr() as *mut __m256i,
                            sums_out_n2,
                        );

                        for idx in 0..4 {
                            let val = sums_out_n0_u64[idx];
                            sums_out_n0_u64_acc[idx] =
                                barrett_coeff_u64(params, val + sums_out_n0_u64_acc[idx], 0);
                        }
                        for idx in 0..4 {
                            let val = sums_out_n2_u64[idx];
                            sums_out_n2_u64_acc[idx] =
                                barrett_coeff_u64(params, val + sums_out_n2_u64_acc[idx], 1);
                        }
                    }
                }

                for idx in 0..4 {
                    sums_out_n0_u64_acc[idx] =
                        barrett_coeff_u64(params, sums_out_n0_u64_acc[idx], 0);
                    sums_out_n2_u64_acc[idx] =
                        barrett_coeff_u64(params, sums_out_n2_u64_acc[idx], 1);
                }

                // output n0
                let (crt_count, poly_len) = (params.crt_count, params.poly_len);
                let mut n = 0;
                let mut idx_c = c * (crt_count * poly_len) + n * (poly_len) + z;
                out[i].data[idx_c] =
                    barrett_coeff_u64(params, sums_out_n0_u64_acc[0] + sums_out_n0_u64_acc[2], 0);
                idx_c += pt_cols * crt_count * poly_len;
                out[i].data[idx_c] =
                    barrett_coeff_u64(params, sums_out_n0_u64_acc[1] + sums_out_n0_u64_acc[3], 0);

                // output n1
                n = 1;
                idx_c = c * (crt_count * poly_len) + n * (poly_len) + z;
                out[i].data[idx_c] =
                    barrett_coeff_u64(params, sums_out_n2_u64_acc[0] + sums_out_n2_u64_acc[2], 1);
                idx_c += pt_cols * crt_count * poly_len;
                out[i].data[idx_c] =
                    barrett_coeff_u64(params, sums_out_n2_u64_acc[1] + sums_out_n2_u64_acc[3], 1);
            }
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

pub fn generate_random_db_and_get_item<'a>(
    params: &'a Params,
    item_idx: usize,
) -> (PolyMatrixRaw<'a>, AlignedMemory64) {
    let mut rng = get_seeded_rng();

    let instances = params.instances;
    let trials = params.n * params.n;
    let dim0 = 1 << params.db_dim_1;
    let num_per = 1 << params.db_dim_2;
    let num_items = dim0 * num_per;
    let db_size_words = instances * trials * num_items * params.poly_len;
    let mut v = AlignedMemory64::new(db_size_words);

    let mut item = PolyMatrixRaw::zero(params, params.n, params.n);

    for instance in 0..instances {
        println!("Instance {:?}", instance);
        for trial in 0..trials {
            println!("Trial {:?}", trial);
            for i in 0..num_items {
                let ii = i % num_per;
                let j = i / num_per;

                let mut db_item = PolyMatrixRaw::random_rng(params, 1, 1, &mut rng);
                db_item.reduce_mod(params.pt_modulus);

                if i == item_idx && instance == 0 {
                    item.copy_into(&db_item, trial / params.n, trial % params.n);
                }

                for z in 0..params.poly_len {
                    db_item.data[z] =
                        recenter_mod(db_item.data[z], params.pt_modulus, params.modulus);
                }

                let db_item_ntt = db_item.ntt();
                for z in 0..params.poly_len {
                    let idx_dst = calc_index(
                        &[instance, trial, z, ii, j],
                        &[instances, trials, params.poly_len, num_per, dim0],
                    );

                    v[idx_dst] = db_item_ntt.data[z]
                        | (db_item_ntt.data[params.poly_len + z] << PACKED_OFFSET_2);
                }
            }
        }
    }
    (item, v)
}

pub fn load_item_from_file<'a>(
    params: &'a Params,
    file: &mut File,
    instance: usize,
    trial: usize,
    item_idx: usize,
) -> PolyMatrixRaw<'a> {
    let db_item_size = params.db_item_size;
    let instances = params.instances;
    let trials = params.n * params.n;

    let chunks = instances * trials;
    let bytes_per_chunk = f64::ceil(db_item_size as f64 / chunks as f64) as usize;
    let logp = f64::ceil(f64::log2(params.pt_modulus as f64)) as usize;
    let modp_words_per_chunk = f64::ceil((bytes_per_chunk * 8) as f64 / logp as f64) as usize;
    assert!(modp_words_per_chunk <= params.poly_len);

    let idx_item_in_file = item_idx * db_item_size;
    let idx_chunk = instance * trials + trial;
    let idx_poly_in_file = idx_item_in_file + idx_chunk * bytes_per_chunk;

    let mut out = PolyMatrixRaw::zero(params, 1, 1);

    let seek_result = file.seek(SeekFrom::Start(idx_poly_in_file as u64));
    if seek_result.is_err() {
        return out;
    }
    let mut data = vec![0u8; 2 * bytes_per_chunk];
    let bytes_read = file
        .read(&mut data.as_mut_slice()[0..bytes_per_chunk])
        .unwrap();

    let modp_words_read = f64::ceil((bytes_read * 8) as f64 / logp as f64) as usize;
    assert!(modp_words_read <= params.poly_len);

    for i in 0..modp_words_read {
        out.data[i] = read_arbitrary_bits(&data, i * logp, logp);
        assert!(out.data[i] <= params.pt_modulus);
    }

    out
}

pub fn load_db_from_file(params: &Params, file: &mut File) -> AlignedMemory64 {
    let instances = params.instances;
    let trials = params.n * params.n;
    let dim0 = 1 << params.db_dim_1;
    let num_per = 1 << params.db_dim_2;
    let num_items = dim0 * num_per;
    let db_size_words = instances * trials * num_items * params.poly_len;
    let mut v = AlignedMemory64::new(db_size_words);

    for instance in 0..instances {
        println!("Instance {:?}", instance);
        for trial in 0..trials {
            println!("Trial {:?}", trial);
            for i in 0..num_items {
                if i % 8192 == 0 {
                    println!("item {:?}", i);
                }
                let ii = i % num_per;
                let j = i / num_per;

                let mut db_item = load_item_from_file(params, file, instance, trial, i);
                // db_item.reduce_mod(params.pt_modulus);

                for z in 0..params.poly_len {
                    db_item.data[z] =
                        recenter_mod(db_item.data[z], params.pt_modulus, params.modulus);
                }

                let db_item_ntt = db_item.ntt();
                for z in 0..params.poly_len {
                    let idx_dst = calc_index(
                        &[instance, trial, z, ii, j],
                        &[instances, trials, params.poly_len, num_per, dim0],
                    );

                    v[idx_dst] = db_item_ntt.data[z]
                        | (db_item_ntt.data[params.poly_len + z] << PACKED_OFFSET_2);
                }
            }
        }
    }
    v
}

pub fn load_file_unsafe(data: &mut [u64], file: &mut File) {
    let data_as_u8_mut = unsafe { data.align_to_mut::<u8>().1 };
    file.read_exact(data_as_u8_mut).unwrap();
}

pub fn load_file(data: &mut [u64], file: &mut File) {
    let mut reader = BufReader::with_capacity(1 << 24, file);
    let mut buf = [0u8; 8];
    for i in 0..data.len() {
        reader.read(&mut buf).unwrap();
        data[i] = u64::from_ne_bytes(buf);
    }
}

pub fn load_preprocessed_db_from_file(params: &Params, file: &mut File) -> AlignedMemory64 {
    let instances = params.instances;
    let trials = params.n * params.n;
    let dim0 = 1 << params.db_dim_1;
    let num_per = 1 << params.db_dim_2;
    let num_items = dim0 * num_per;
    let db_size_words = instances * trials * num_items * params.poly_len;
    let mut v = AlignedMemory64::new(db_size_words);
    let v_mut_slice = v.as_mut_slice();

    let now = Instant::now();
    load_file(v_mut_slice, file);
    println!("Done loading ({} ms).", now.elapsed().as_millis());

    v
}

pub fn fold_ciphertexts(
    params: &Params,
    v_cts: &mut Vec<PolyMatrixRaw>,
    v_folding: &Vec<PolyMatrixNTT>,
    v_folding_neg: &Vec<PolyMatrixNTT>,
) {
    let further_dims = log2(v_cts.len() as u64) as usize;
    let ell = v_folding[0].cols / 2;
    let mut ginv_c = PolyMatrixRaw::zero(&params, 2 * ell, 1);
    let mut ginv_c_ntt = PolyMatrixNTT::zero(&params, 2 * ell, 1);
    let mut prod = PolyMatrixNTT::zero(&params, 2, 1);
    let mut sum = PolyMatrixNTT::zero(&params, 2, 1);

    let mut num_per = v_cts.len();
    for cur_dim in 0..further_dims {
        num_per = num_per / 2;
        for i in 0..num_per {
            gadget_invert(&mut ginv_c, &v_cts[i]);
            to_ntt(&mut ginv_c_ntt, &ginv_c);
            multiply(
                &mut prod,
                &v_folding_neg[further_dims - 1 - cur_dim],
                &ginv_c_ntt,
            );
            gadget_invert(&mut ginv_c, &v_cts[num_per + i]);
            to_ntt(&mut ginv_c_ntt, &ginv_c);
            multiply(
                &mut sum,
                &v_folding[further_dims - 1 - cur_dim],
                &ginv_c_ntt,
            );
            add_into(&mut sum, &prod);
            from_ntt(&mut v_cts[i], &sum);
        }
    }
}

pub fn pack<'a>(
    params: &'a Params,
    v_ct: &Vec<PolyMatrixRaw>,
    v_w: &Vec<PolyMatrixNTT>,
) -> PolyMatrixNTT<'a> {
    assert!(v_ct.len() >= params.n * params.n);
    assert!(v_w.len() == params.n);
    assert!(v_ct[0].rows == 2);
    assert!(v_ct[0].cols == 1);
    assert!(v_w[0].rows == (params.n + 1));
    assert!(v_w[0].cols == params.t_conv);

    let mut result = PolyMatrixNTT::zero(params, params.n + 1, params.n);

    let mut ginv = PolyMatrixRaw::zero(params, params.t_conv, 1);
    let mut ginv_nttd = PolyMatrixNTT::zero(params, params.t_conv, 1);
    let mut prod = PolyMatrixNTT::zero(params, params.n + 1, 1);
    let mut ct_1 = PolyMatrixRaw::zero(params, 1, 1);
    let mut ct_2 = PolyMatrixRaw::zero(params, 1, 1);
    let mut ct_2_ntt = PolyMatrixNTT::zero(params, 1, 1);

    for c in 0..params.n {
        let mut v_int = PolyMatrixNTT::zero(&params, params.n + 1, 1);
        for r in 0..params.n {
            let w = &v_w[r];
            let ct = &v_ct[r * params.n + c];
            ct_1.get_poly_mut(0, 0).copy_from_slice(ct.get_poly(0, 0));
            ct_2.get_poly_mut(0, 0).copy_from_slice(ct.get_poly(1, 0));
            to_ntt(&mut ct_2_ntt, &ct_2);
            gadget_invert(&mut ginv, &ct_1);
            to_ntt(&mut ginv_nttd, &ginv);
            multiply(&mut prod, &w, &ginv_nttd);
            add_into_at(&mut v_int, &ct_2_ntt, 1 + r, 0);
            add_into(&mut v_int, &prod);
        }
        result.copy_into(&v_int, 0, c);
    }

    result
}

pub fn encode(params: &Params, v_packed_ct: &Vec<PolyMatrixRaw>) -> Vec<u8> {
    let q1 = 4 * params.pt_modulus;
    let q1_bits = log2_ceil(q1) as usize;
    let q2 = Q2_VALUES[params.q2_bits as usize];
    let q2_bits = params.q2_bits as usize;

    let num_bits = params.instances
        * ((q2_bits * params.n * params.poly_len)
            + (q1_bits * params.n * params.n * params.poly_len));
    let round_to = 64;
    let num_bytes_rounded_up = ((num_bits + round_to - 1) / round_to) * round_to / 8;

    let mut result = vec![0u8; num_bytes_rounded_up];
    let mut bit_offs = 0;
    for instance in 0..params.instances {
        let packed_ct = &v_packed_ct[instance];

        let mut first_row = packed_ct.submatrix(0, 0, 1, packed_ct.cols);
        let mut rest_rows = packed_ct.submatrix(1, 0, packed_ct.rows - 1, packed_ct.cols);
        first_row.apply_func(|x| rescale(x, params.modulus, q2));
        rest_rows.apply_func(|x| rescale(x, params.modulus, q1));

        let data = result.as_mut_slice();
        for i in 0..params.n * params.poly_len {
            write_arbitrary_bits(data, first_row.data[i], bit_offs, q2_bits);
            bit_offs += q2_bits;
        }
        for i in 0..params.n * params.n * params.poly_len {
            write_arbitrary_bits(data, rest_rows.data[i], bit_offs, q1_bits);
            bit_offs += q1_bits;
        }
    }
    result
}

pub fn get_v_folding_neg<'a>(
    params: &'a Params,
    v_folding: &Vec<PolyMatrixNTT<'a>>,
) -> Vec<PolyMatrixNTT<'a>> {
    let gadget_ntt = build_gadget(params, 2, 2 * params.t_gsw).ntt(); // TODO: make this better

    let v_folding_neg = (0..params.db_dim_2)
        .into_par_iter()
        .map(|i| {
            let mut ct_gsw_inv = PolyMatrixRaw::zero(params, 2, 2 * params.t_gsw);
            invert(&mut ct_gsw_inv, &v_folding[i].raw());
            let mut ct_gsw_neg = PolyMatrixNTT::zero(params, 2, 2 * params.t_gsw);
            add(&mut ct_gsw_neg, &gadget_ntt, &ct_gsw_inv.ntt());
            ct_gsw_neg
        })
        .collect();

    v_folding_neg
}

pub fn expand_query<'a>(
    params: &'a Params,
    public_params: &PublicParameters<'a>,
    query: &Query<'a>,
) -> (AlignedMemory64, Vec<PolyMatrixNTT<'a>>) {
    let dim0 = 1 << params.db_dim_1;
    let further_dims = params.db_dim_2;

    let mut v_reg_reoriented;
    let mut v_folding;

    let num_bits_to_gen = params.t_gsw * further_dims + dim0;
    let g = log2_ceil_usize(num_bits_to_gen);
    let right_expanded = params.t_gsw * further_dims;
    let stop_round = log2_ceil_usize(right_expanded);

    let mut v = Vec::new();
    for _ in 0..(1 << g) {
        v.push(PolyMatrixNTT::zero(params, 2, 1));
    }
    v[0].copy_into(&query.ct.as_ref().unwrap().ntt(), 0, 0);

    let v_conversion = &public_params.v_conversion.as_ref().unwrap()[0];
    let v_w_left = public_params.v_expansion_left.as_ref().unwrap();
    let v_w_right = public_params.v_expansion_right.as_ref().unwrap();
    let v_neg1 = params.get_v_neg1();

    coefficient_expansion(
        &mut v,
        g,
        stop_round,
        params,
        &v_w_left,
        &v_w_right,
        &v_neg1,
        params.t_gsw * params.db_dim_2,
    );

    let mut v_reg_inp = Vec::with_capacity(dim0);
    for i in 0..dim0 {
        v_reg_inp.push(v[2 * i].clone());
    }
    let mut v_gsw_inp = Vec::with_capacity(right_expanded);
    for i in 0..right_expanded {
        v_gsw_inp.push(v[2 * i + 1].clone());
    }

    let v_reg_sz = dim0 * 2 * params.poly_len;
    v_reg_reoriented = AlignedMemory64::new(v_reg_sz);
    reorient_reg_ciphertexts(params, v_reg_reoriented.as_mut_slice(), &v_reg_inp);

    v_folding = Vec::new();
    for _ in 0..params.db_dim_2 {
        v_folding.push(PolyMatrixNTT::zero(params, 2, 2 * params.t_gsw));
    }

    regev_to_gsw(&mut v_folding, &v_gsw_inp, &v_conversion, params, 1, 0);

    (v_reg_reoriented, v_folding)
}

pub fn process_query(
    params: &Params,
    public_params: &PublicParameters,
    query: &Query,
    db: &[u64],
) -> Vec<u8> {
    let dim0 = 1 << params.db_dim_1;
    let num_per = 1 << params.db_dim_2;
    let db_slice_sz = dim0 * num_per * params.poly_len;

    let v_packing = public_params.v_packing.as_ref();

    let mut v_reg_reoriented;
    let v_folding;
    if params.expand_queries {
        (v_reg_reoriented, v_folding) = expand_query(params, public_params, query);
    } else {
        v_reg_reoriented = AlignedMemory64::new(query.v_buf.as_ref().unwrap().len());
        v_reg_reoriented
            .as_mut_slice()
            .copy_from_slice(query.v_buf.as_ref().unwrap());

        v_folding = query
            .v_ct
            .as_ref()
            .unwrap()
            .iter()
            .map(|x| x.ntt())
            .collect();
    }
    let v_folding_neg = get_v_folding_neg(params, &v_folding);

    let v_packed_ct = (0..params.instances)
        .into_par_iter()
        .map(|instance| {
            let mut intermediate = Vec::with_capacity(num_per);
            let mut intermediate_raw = Vec::with_capacity(num_per);
            for _ in 0..num_per {
                intermediate.push(PolyMatrixNTT::zero(params, 2, 1));
                intermediate_raw.push(PolyMatrixRaw::zero(params, 2, 1));
            }

            let mut v_ct = Vec::new();

            for trial in 0..(params.n * params.n) {
                let idx = (instance * (params.n * params.n) + trial) * db_slice_sz;
                let cur_db = &db[idx..(idx + db_slice_sz)];

                multiply_reg_by_database(
                    &mut intermediate,
                    cur_db,
                    v_reg_reoriented.as_slice(),
                    params,
                    dim0,
                    num_per,
                );

                for i in 0..intermediate.len() {
                    from_ntt(&mut intermediate_raw[i], &intermediate[i]);
                }

                fold_ciphertexts(params, &mut intermediate_raw, &v_folding, &v_folding_neg);

                v_ct.push(intermediate_raw[0].clone());
            }

            let packed_ct = pack(params, &v_ct, &v_packing);

            packed_ct.raw()
        })
        .collect();

    encode(params, &v_packed_ct)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::client::*;
    use rand::{prelude::SmallRng, Rng};

    const TEST_PREPROCESSED_DB_PATH: &'static str = "/home/samir/wiki/enwiki-20220320.dbp";

    fn get_params() -> Params {
        get_fast_expansion_testing_params()
    }

    fn dec_reg<'a>(
        params: &'a Params,
        ct: &PolyMatrixNTT<'a>,
        client: &mut Client<'a, SmallRng>,
        scale_k: u64,
    ) -> u64 {
        let dec = client.decrypt_matrix_reg(ct).raw();
        let mut val = dec.data[0] as i64;
        if val >= (params.modulus / 2) as i64 {
            val -= params.modulus as i64;
        }
        let val_rounded = f64::round(val as f64 / scale_k as f64) as i64;
        if val_rounded == 0 {
            0
        } else {
            1
        }
    }

    fn dec_gsw<'a>(
        params: &'a Params,
        ct: &PolyMatrixNTT<'a>,
        client: &mut Client<'a, SmallRng>,
    ) -> u64 {
        let dec = client.decrypt_matrix_reg(ct).raw();
        let idx = 2 * (params.t_gsw - 1) * params.poly_len + params.poly_len; // this offset should encode a large value
        let mut val = dec.data[idx] as i64;
        if val >= (params.modulus / 2) as i64 {
            val -= params.modulus as i64;
        }
        if i64::abs(val) < (1i64 << 10) {
            0
        } else {
            1
        }
    }

    #[test]
    fn coefficient_expansion_is_correct() {
        let params = get_params();
        let v_neg1 = params.get_v_neg1();
        let mut seeded_rng = get_seeded_rng();
        let mut client = Client::init(&params, &mut seeded_rng);
        let public_params = client.generate_keys();

        let mut v = Vec::new();
        for _ in 0..(1 << (params.db_dim_1 + 1)) {
            v.push(PolyMatrixNTT::zero(&params, 2, 1));
        }

        let target = 7;
        let scale_k = params.modulus / params.pt_modulus;
        let mut sigma = PolyMatrixRaw::zero(&params, 1, 1);
        sigma.data[target] = scale_k;
        v[0] = client.encrypt_matrix_reg(&sigma.ntt());
        let test_ct = client.encrypt_matrix_reg(&sigma.ntt());

        let v_w_left = public_params.v_expansion_left.unwrap();
        let v_w_right = public_params.v_expansion_right.unwrap();
        coefficient_expansion(
            &mut v,
            params.g(),
            params.stop_round(),
            &params,
            &v_w_left,
            &v_w_right,
            &v_neg1,
            params.t_gsw * params.db_dim_2,
        );

        assert_eq!(dec_reg(&params, &test_ct, &mut client, scale_k), 0);

        for i in 0..v.len() {
            if i == target {
                assert_eq!(dec_reg(&params, &v[i], &mut client, scale_k), 1);
            } else {
                assert_eq!(dec_reg(&params, &v[i], &mut client, scale_k), 0);
            }
        }
    }

    #[test]
    fn regev_to_gsw_is_correct() {
        let mut params = get_params();
        params.db_dim_2 = 1;
        let mut seeded_rng = get_seeded_rng();
        let mut client = Client::init(&params, &mut seeded_rng);
        let public_params = client.generate_keys();

        let mut enc_constant = |val| {
            let mut sigma = PolyMatrixRaw::zero(&params, 1, 1);
            sigma.data[0] = val;
            client.encrypt_matrix_reg(&sigma.ntt())
        };

        let v = &public_params.v_conversion.unwrap()[0];

        let bits_per = get_bits_per(&params, params.t_gsw);
        let mut v_inp_1 = Vec::new();
        let mut v_inp_0 = Vec::new();
        for i in 0..params.t_gsw {
            let val = 1u64 << (bits_per * i);
            v_inp_1.push(enc_constant(val));
            v_inp_0.push(enc_constant(0));
        }

        let mut v_gsw = Vec::new();
        v_gsw.push(PolyMatrixNTT::zero(&params, 2, 2 * params.t_gsw));

        regev_to_gsw(&mut v_gsw, &v_inp_1, v, &params, 1, 0);

        assert_eq!(dec_gsw(&params, &v_gsw[0], &mut client), 1);

        regev_to_gsw(&mut v_gsw, &v_inp_0, v, &params, 1, 0);

        assert_eq!(dec_gsw(&params, &v_gsw[0], &mut client), 0);
    }

    #[test]
    fn multiply_reg_by_database_is_correct() {
        let params = get_params();
        let mut seeded_rng = get_seeded_rng();

        let dim0 = 1 << params.db_dim_1;
        let num_per = 1 << params.db_dim_2;
        let scale_k = params.modulus / params.pt_modulus;

        let target_idx = seeded_rng.gen::<usize>() % (dim0 * num_per);
        let target_idx_dim0 = target_idx / num_per;
        let target_idx_num_per = target_idx % num_per;

        let mut client = Client::init(&params, &mut seeded_rng);
        _ = client.generate_keys();

        let (corr_item, db) = generate_random_db_and_get_item(&params, target_idx);

        let mut v_reg = Vec::new();
        for i in 0..dim0 {
            let val = if i == target_idx_dim0 { scale_k } else { 0 };
            let sigma = PolyMatrixRaw::single_value(&params, val).ntt();
            v_reg.push(client.encrypt_matrix_reg(&sigma));
        }

        let v_reg_sz = dim0 * 2 * params.poly_len;
        let mut v_reg_reoriented = AlignedMemory64::new(v_reg_sz);
        reorient_reg_ciphertexts(&params, v_reg_reoriented.as_mut_slice(), &v_reg);

        let mut out = Vec::with_capacity(num_per);
        for _ in 0..dim0 {
            out.push(PolyMatrixNTT::zero(&params, 2, 1));
        }
        multiply_reg_by_database(
            &mut out,
            db.as_slice(),
            v_reg_reoriented.as_slice(),
            &params,
            dim0,
            num_per,
        );

        // decrypt
        let dec = client.decrypt_matrix_reg(&out[target_idx_num_per]).raw();
        let mut dec_rescaled = PolyMatrixRaw::zero(&params, 1, 1);
        for z in 0..params.poly_len {
            dec_rescaled.data[z] = rescale(dec.data[z], params.modulus, params.pt_modulus);
        }

        for z in 0..params.poly_len {
            // println!("{:?} {:?}", dec_rescaled.data[z], corr_item.data[z]);
            assert_eq!(dec_rescaled.data[z], corr_item.data[z]);
        }
    }

    #[test]
    fn fold_ciphertexts_is_correct() {
        let params = get_params();
        let mut seeded_rng = get_seeded_rng();

        let dim0 = 1 << params.db_dim_1;
        let num_per = 1 << params.db_dim_2;
        let scale_k = params.modulus / params.pt_modulus;

        let target_idx = seeded_rng.gen::<usize>() % (dim0 * num_per);
        let target_idx_num_per = target_idx % num_per;

        let mut client = Client::init(&params, &mut seeded_rng);
        _ = client.generate_keys();

        let mut v_reg = Vec::new();
        for i in 0..num_per {
            let val = if i == target_idx_num_per { scale_k } else { 0 };
            let sigma = PolyMatrixRaw::single_value(&params, val).ntt();
            v_reg.push(client.encrypt_matrix_reg(&sigma));
        }

        let mut v_reg_raw = Vec::new();
        for i in 0..num_per {
            v_reg_raw.push(v_reg[i].raw());
        }

        let bits_per = get_bits_per(&params, params.t_gsw);
        let mut v_folding = Vec::new();
        for i in 0..params.db_dim_2 {
            let bit = ((target_idx_num_per as u64) & (1 << (i as u64))) >> (i as u64);
            let mut ct_gsw = PolyMatrixNTT::zero(&params, 2, 2 * params.t_gsw);

            for j in 0..params.t_gsw {
                let value = (1u64 << (bits_per * j)) * bit;
                let sigma = PolyMatrixRaw::single_value(&params, value);
                let sigma_ntt = to_ntt_alloc(&sigma);
                let ct = client.encrypt_matrix_reg(&sigma_ntt);
                ct_gsw.copy_into(&ct, 0, 2 * j + 1);
                let prod = &to_ntt_alloc(client.get_sk_reg()) * &sigma_ntt;
                let ct = &client.encrypt_matrix_reg(&prod);
                ct_gsw.copy_into(&ct, 0, 2 * j);
            }

            v_folding.push(ct_gsw);
        }

        let gadget_ntt = build_gadget(&params, 2, 2 * params.t_gsw).ntt();
        let mut v_folding_neg = Vec::new();
        let mut ct_gsw_inv = PolyMatrixRaw::zero(&params, 2, 2 * params.t_gsw);
        for i in 0..params.db_dim_2 {
            invert(&mut ct_gsw_inv, &v_folding[i].raw());
            let mut ct_gsw_neg = PolyMatrixNTT::zero(&params, 2, 2 * params.t_gsw);
            add(&mut ct_gsw_neg, &gadget_ntt, &ct_gsw_inv.ntt());
            v_folding_neg.push(ct_gsw_neg);
        }

        fold_ciphertexts(&params, &mut v_reg_raw, &v_folding, &v_folding_neg);

        // decrypt
        assert_eq!(
            dec_reg(&params, &v_reg_raw[0].ntt(), &mut client, scale_k),
            1
        );
    }

    fn full_protocol_is_correct_for_params(params: &Params) {
        let mut seeded_rng = get_seeded_rng();

        let target_idx = seeded_rng.gen::<usize>() % (params.db_dim_1 + params.db_dim_2);

        let mut client = Client::init(params, &mut seeded_rng);

        let public_params = client.generate_keys();
        let query = client.generate_query(target_idx);

        let (corr_item, db) = generate_random_db_and_get_item(params, target_idx);

        let response = process_query(params, &public_params, &query, db.as_slice());

        let result = client.decode_response(response.as_slice());

        let p_bits = log2_ceil(params.pt_modulus) as usize;
        let corr_result = corr_item.to_vec(p_bits, params.modp_words_per_chunk());

        assert_eq!(result.len(), corr_result.len());

        for z in 0..corr_result.len() {
            assert_eq!(result[z], corr_result[z], "at {:?}", z);
        }
    }

    fn full_protocol_is_correct_for_params_real_db(params: &Params) {
        let mut seeded_rng = get_seeded_rng();

        let target_idx = seeded_rng.gen::<usize>() % (params.db_dim_1 + params.db_dim_2);

        let mut client = Client::init(params, &mut seeded_rng);

        let public_params = client.generate_keys();
        let query = client.generate_query(target_idx);

        let mut file = File::open(TEST_PREPROCESSED_DB_PATH).unwrap();

        let db = load_preprocessed_db_from_file(params, &mut file);

        let response = process_query(params, &public_params, &query, db.as_slice());

        let result = client.decode_response(response.as_slice());

        let corr_result = vec![0x42, 0x5a, 0x68];

        for z in 0..corr_result.len() {
            assert_eq!(result[z], corr_result[z]);
        }
    }

    #[test]
    fn full_protocol_is_correct() {
        full_protocol_is_correct_for_params(&get_params());
    }

    #[test]
    #[ignore]
    fn larger_full_protocol_is_correct() {
        let cfg_expand = r#"
            {
            'n': 2,
            'nu_1': 10,
            'nu_2': 6,
            'p': 512,
            'q2_bits': 21,
            's_e': 85.83255142749422,
            't_gsw': 10,
            't_conv': 4,
            't_exp_left': 16,
            't_exp_right': 56,
            'instances': 1,
            'db_item_size': 9000 }
        "#;
        let cfg = cfg_expand;
        let cfg = cfg.replace("'", "\"");
        let params = params_from_json(&cfg);

        full_protocol_is_correct_for_params(&params);
        full_protocol_is_correct_for_params_real_db(&params);
    }

    // #[test]
    // fn full_protocol_is_correct_20_256() {
    //     full_protocol_is_correct_for_params(&params_from_json(&CFG_20_256.replace("'", "\"")));
    // }

    // #[test]
    // fn full_protocol_is_correct_16_100000() {
    //     full_protocol_is_correct_for_params(&params_from_json(&CFG_16_100000.replace("'", "\"")));
    // }

    #[test]
    #[ignore]
    fn full_protocol_is_correct_real_db_16_100000() {
        full_protocol_is_correct_for_params_real_db(&params_from_json(
            &CFG_16_100000.replace("'", "\""),
        ));
    }
}

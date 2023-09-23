use std::collections::HashMap;
use std::collections::HashSet;

use crate::db::aligned_memory::*;
use spiral_rs::arith::*;
use spiral_rs::client::*;
use spiral_rs::gadget::*;
use spiral_rs::params::*;
use spiral_rs::poly::*;

use rayon::prelude::*;

pub fn get_v_folding_neg<'a>(
    params: &'a Params,
    v_folding: &Vec<PolyMatrixNTT<'a>>,
) -> Vec<PolyMatrixNTT<'a>> {
    let gadget_ntt = build_gadget(params, 2, 2 * params.t_gsw).ntt(); // TODO: make this better

    let v_folding_neg = (0..params.db_dim_2)
        .into_iter()
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

pub fn coefficient_expansion(
    v: &mut Vec<PolyMatrixNTT>,
    g: usize,
    stop_round: usize,
    params: &Params,
    v_w_left: &Vec<PolyMatrixNTT>,
    v_w_right: &Vec<PolyMatrixNTT>,
    v_neg1: &Vec<PolyMatrixNTT>,
    max_bits_to_gen_right: usize,
    indices: Option<&HashSet<(usize, usize)>>,
) {
    let poly_len = params.poly_len;

    let empty_set = HashSet::new();
    let indices_set = indices.unwrap_or(&empty_set);

    for r in 0..g {
        let num_in = 1 << r;
        let num_out = 2 * num_in;

        let t = (poly_len / (1 << r)) + 1;

        let neg1 = &v_neg1[r];

        let action_expand = |(i, v_i, j): (usize, &mut PolyMatrixNTT, usize)| {
            if (stop_round > 0 && r > stop_round && (i % 2) == 1)
                || (stop_round > 0
                    && r == stop_round
                    && (i % 2) == 1
                    && (i / 2) >= max_bits_to_gen_right)
            {
                return;
            }

            let out_idx = j * num_in + i;
            if indices.is_some() && !indices_set.contains(&(r, out_idx)) {
                // println!("skipping: r: {} out_idx: {}", r, out_idx);
                return;
            }
            // println!("not skipping: r: {} out_idx: {}", r, out_idx);

            let mut ct = PolyMatrixRaw::zero(params, 2, 1);
            let mut ct_auto = PolyMatrixRaw::zero(params, 2, 1);
            let mut ct_auto_1 = PolyMatrixRaw::zero(params, 1, 1);
            let mut ct_auto_1_ntt = PolyMatrixNTT::zero(params, 1, 1);
            let mut w_times_ginv_ct = PolyMatrixNTT::zero(params, 2, 1);

            let mut ginv_ct_left = PolyMatrixRaw::zero(params, params.t_exp_left, 1);
            let mut ginv_ct_left_ntt = PolyMatrixNTT::zero(params, params.t_exp_left, 1);
            let mut ginv_ct_right = PolyMatrixRaw::zero(params, params.t_exp_right, 1);
            let mut ginv_ct_right_ntt = PolyMatrixNTT::zero(params, params.t_exp_right, 1);

            let (w, _gadget_dim, gi_ct, gi_ct_ntt) = match (r != 0) && (i % 2 == 0) {
                true => (
                    &v_w_left[r],
                    params.t_exp_left,
                    &mut ginv_ct_left,
                    &mut ginv_ct_left_ntt,
                ),
                false => (
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
            .map(|x| (x.0, x.1, 0))
            .for_each(action_expand);
        v[num_in..num_out]
            .par_iter_mut()
            .enumerate()
            .map(|x| (x.0, x.1, 1))
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

pub fn reorient_reg_ciphertexts(params: &Params, out: &mut [u64], v_reg: &Vec<PolyMatrixNTT>) {
    // query:  [dim0, ct_rows, poly_len]

    let poly_len = params.poly_len;
    let crt_count = params.crt_count;

    assert_eq!(crt_count, 2);
    assert!(log2(params.moduli[0]) <= 32);

    let num_reg_expanded = 1 << params.db_dim_1;
    let ct_rows = v_reg[0].rows;
    let ct_cols = v_reg[0].cols;

    assert_eq!(ct_rows, 2);
    assert_eq!(ct_cols, 1);

    for j in 0..num_reg_expanded {
        for r in 0..ct_rows {
            for m in 0..ct_cols {
                for z in 0..params.poly_len {
                    let idx_a_in =
                        r * (ct_cols * crt_count * poly_len) + m * (crt_count * poly_len);
                    let idx_a_out = j * (ct_rows * poly_len) + r * (poly_len) + z;
                    let val1 = v_reg[j].data[idx_a_in + z] % params.moduli[0];
                    let val2 = v_reg[j].data[idx_a_in + params.poly_len + z] % params.moduli[1];

                    out[idx_a_out] = val1 | (val2 << 32);
                }
            }
        }
    }
}

pub fn to_per_round_set(params: &Params, indices: &HashSet<usize>) -> HashSet<(usize, usize)> {
    let mut to_do = HashSet::<(usize, usize)>::new();
    // print!("{}: ", params.g() - 1);
    // let further_dims_threshold = params.t_gsw * params.db_dim_2;
    for i in 0..(1 << (params.g())) {
        if (i % 2 == 0 && indices.contains(&(i / 2))) || (i % 2 == 1) {
            // print!("1 ");
            to_do.insert((params.g() - 1, i));
        } else {
            // print!("0 ");
        }
    }
    // for i in indices {
    //     to_do.insert((params.g() - 1, 2 * i));
    // }
    // for i in 0..(1 << (params.g() - 1)) {
    //     to_do.insert((params.g() - 1, 2 * i + 1));
    // }
    // print!("\n");

    for r in (0..params.g() - 1).rev() {
        // print!("{}: ", r);
        for i in 0..(1 << (r + 1)) {
            let left = to_do.contains(&(r + 1, i));
            let right = to_do.contains(&(r + 1, i + (1 << (r + 1))));
            if left || right {
                // print!("1 ");
                to_do.insert((r, i));
            } else {
                // print!("0 ");
            }
        }
        // print!("\n");
    }
    to_do
}

pub fn expand_query<'a>(
    params: &'a Params,
    public_params: &PublicParameters<'a>,
    query: &Query<'a>,
    indices: Option<&Vec<usize>>,
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
    let v_w_right = public_params.v_expansion_right.as_ref().unwrap_or(v_w_left);
    let v_neg1 = params.get_v_neg1();

    let mut v_reg_inp = Vec::with_capacity(dim0);
    let mut v_gsw_inp = Vec::with_capacity(right_expanded);

    let to_do;
    let mut indices_to_do = None;
    if let Some(inds) = indices {
        // print!("inds: ");
        // for i in inds.keys() {
        //     if *i < params.num_items() {
        //         print!("{} ", *i)
        //     }
        // }
        // print!("\n");

        let mut set_dim0 = HashSet::new();
        for i in inds {
            if *i < params.num_items() {
                set_dim0.insert(*i / (1 << params.db_dim_2));
            }
        }

        // print!("set_dim0: ");
        // for i in &set_dim0 {
        //     print!("{} ", *i)
        // }
        // print!("\n");

        to_do = to_per_round_set(params, &set_dim0);
        indices_to_do = Some(&to_do);
    }

    if further_dims > 0 {
        coefficient_expansion(
            &mut v,
            g,
            stop_round,
            params,
            &v_w_left,
            &v_w_right,
            &v_neg1,
            params.t_gsw * params.db_dim_2,
            indices_to_do,
        );

        for i in 0..dim0 {
            v_reg_inp.push(v[2 * i].clone());
        }
        for i in 0..right_expanded {
            v_gsw_inp.push(v[2 * i + 1].clone());
        }
    } else {
        coefficient_expansion(
            &mut v,
            g,
            0,
            params,
            &v_w_left,
            &v_w_left,
            &v_neg1,
            0,
            indices_to_do,
        );
        for i in 0..dim0 {
            v_reg_inp.push(v[i].clone());
        }
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

#[cfg(test)]
mod test {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha20Rng;
    use spiral_rs::util;

    fn get_params() -> Params {
        util::get_fast_expansion_testing_params()
    }

    fn dec_reg<'a>(
        params: &'a Params,
        ct: &PolyMatrixNTT<'a>,
        client: &mut Client<'a>,
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

    fn dec_gsw<'a>(params: &'a Params, ct: &PolyMatrixNTT<'a>, client: &mut Client<'a>) -> u64 {
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
        let mut rng = ChaCha20Rng::from_entropy();
        let mut rng_pub = ChaCha20Rng::from_entropy();
        let mut client = Client::init(&params);
        let public_params = client.generate_keys();

        let mut v = Vec::new();
        for _ in 0..(1 << (params.db_dim_1 + 1)) {
            v.push(PolyMatrixNTT::zero(&params, 2, 1));
        }

        let indices_set = HashSet::new();
        // indices_set.insert(1);
        // indices_set.insert(7);
        // indices_set.insert(4);
        let to_do = to_per_round_set(&params, &indices_set);

        let target = 4;
        let scale_k = params.modulus / params.pt_modulus;
        let mut sigma = PolyMatrixRaw::zero(&params, 1, 1);
        sigma.data[target] = scale_k;
        v[0] = client.encrypt_matrix_reg(&sigma.ntt(), &mut rng, &mut rng_pub);
        let test_ct = client.encrypt_matrix_reg(&sigma.ntt(), &mut rng, &mut rng_pub);

        let v_w_left = public_params.v_expansion_left.unwrap();
        let v_w_right = public_params.v_expansion_right.unwrap_or(v_w_left.clone());
        coefficient_expansion(
            &mut v,
            params.g(),
            params.stop_round(),
            &params,
            &v_w_left,
            &v_w_right,
            &v_neg1,
            params.t_gsw * params.db_dim_2,
            Some(&to_do),
        );

        assert_eq!(dec_reg(&params, &test_ct, &mut client, scale_k), 0);

        for i in 0..v.len() {
            let val = dec_reg(&params, &v[i], &mut client, scale_k);
            // println!("{} ? {}", i, val);
            if i == target {
                assert_eq!(val, 1);
            } else {
                assert_eq!(val, 0);
            }
        }
    }

    #[test]
    fn regev_to_gsw_is_correct() {
        let mut params = get_params();
        params.db_dim_2 = 1;
        let mut rng = ChaCha20Rng::from_entropy();
        let mut rng_pub = ChaCha20Rng::from_entropy();
        let mut client = Client::init(&params);
        let public_params = client.generate_keys();

        let mut enc_constant = |val| {
            let mut sigma = PolyMatrixRaw::zero(&params, 1, 1);
            sigma.data[0] = val;
            client.encrypt_matrix_reg(&sigma.ntt(), &mut rng, &mut rng_pub)
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
}

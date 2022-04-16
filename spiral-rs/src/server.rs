use crate::arith::*;
use crate::gadget::*;
use crate::params::*;
use crate::poly::*;

pub fn coefficient_expansion(
    v: &mut Vec<PolyMatrixNTT>,
    g: usize,
    stopround: usize,
    params: &Params,
    v_w_left: &Vec<PolyMatrixNTT>,
    v_w_right: &Vec<PolyMatrixNTT>,
    v_neg1: &Vec<PolyMatrixNTT>,
    max_bits_to_gen_right: usize,
) {
    let poly_len = params.poly_len;

    let mut ct = PolyMatrixRaw::zero(params, 2, 1);
    let mut ct_auto = PolyMatrixRaw::zero(params, 2, 1);
    let mut ct_auto_1 = PolyMatrixRaw::zero(params, 1, 1);
    let mut ct_auto_1_ntt = PolyMatrixNTT::zero(params, 1, 1);
    let mut ginv_ct_left = PolyMatrixRaw::zero(params, params.t_exp_left, 1);
    let mut ginv_ct_left_ntt = PolyMatrixNTT::zero(params, params.t_exp_left, 1);
    let mut ginv_ct_right = PolyMatrixRaw::zero(params, params.t_exp_right, 1);
    let mut ginv_ct_right_ntt = PolyMatrixNTT::zero(params, params.t_exp_right, 1);
    let mut w_times_ginv_ct = PolyMatrixNTT::zero(params, 2, 1);

    for r in 0..g {
        let num_in = 1 << r;
        let num_out = 2 * num_in;

        let t = (poly_len / (1 << r)) + 1;

        let neg1 = &v_neg1[r];

        for i in 0..num_out {
            if stopround > 0 && i % 2 == 1 && r > stopround
                || (r == stopround && i / 2 >= max_bits_to_gen_right)
            {
                continue;
            }

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

            if i < num_in {
                let (src, dest) = v.split_at_mut(num_in);
                scalar_multiply(&mut dest[i], neg1, &src[i]);
            }

            from_ntt(&mut ct, &v[i]);
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
                        let sum = v[i].data[idx]
                            + w_times_ginv_ct.data[idx]
                            + j * ct_auto_1_ntt.data[n * poly_len + z];
                        v[i].data[idx] = barrett_coeff_u64(params, sum, n);
                        idx += 1;
                    }
                }
            }
        }
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

    let mut ginv_c_inp = PolyMatrixRaw::zero(params, 2 * params.t_conv, 1);
    let mut ginv_c_inp_ntt = PolyMatrixNTT::zero(params, 2 * params.t_conv, 1);
    let mut tmp_ct_raw = PolyMatrixRaw::zero(params, 2, 1);
    let mut tmp_ct = PolyMatrixNTT::zero(params, 2, 1);

    for i in 0..params.db_dim_2 {
        let ct = &mut v_gsw[i];
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
    }
}

#[cfg(test)]
mod test {
    use rand::prelude::StdRng;

    use crate::{client::*, util::*};

    use super::*;

    fn get_params() -> Params {
        let mut params = get_expansion_testing_params();
        params.db_dim_1 = 6;
        params.db_dim_2 = 2;
        params.t_exp_right = 8;
        params
    }

    fn dec_reg<'a>(
        params: &'a Params,
        ct: &PolyMatrixNTT<'a>,
        client: &mut Client<'a, StdRng>,
        scale_k: u64,
    ) -> u64 {
        let dec = client.decrypt_matrix_reg(ct).raw();
        let mut val = dec.data[0] as i64;
        if val >= (params.modulus / 2) as i64 {
            val -= params.modulus as i64;
        }
        let val_rounded = f64::round(val as f64 / scale_k as f64) as i64;
        println!("{:?} {:?}", val, val_rounded);
        if val_rounded == 0 {
            0
        } else {
            1
        }
    }

    fn dec_gsw<'a>(
        params: &'a Params,
        ct: &PolyMatrixNTT<'a>,
        client: &mut Client<'a, StdRng>,
    ) -> u64 {
        let dec = client.decrypt_matrix_reg(ct).raw();
        let idx = (params.t_gsw - 1) * params.poly_len + params.poly_len; // this offset should encode a large value
        let mut val = dec.data[idx] as i64;
        if val >= (params.modulus / 2) as i64 {
            val -= params.modulus as i64;
        }
        if val < 100 {
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
        for _ in 0..params.poly_len {
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
            client.g,
            client.stop_round,
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
}

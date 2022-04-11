use crate::arith;
use crate::gadget::gadget_invert;
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

            let (w, gadget_dim) = match i % 2 {
                0 => (&v_w_left[r], params.t_exp_left),
                1 | _ => (&v_w_right[r], params.t_exp_right),
            };

            if i < num_in {
                let (src, dest) = v.split_at_mut(num_in);
                scalar_multiply(&mut dest[i], neg1, &src[i]);
            }

            let ct = from_ntt_alloc(&v[i]);
            let ct_auto = automorph_alloc(&ct, t);
            let ct_auto_0 = ct_auto.submatrix(0, 0, 1, 1);
            let ct_auto_1_ntt = ct_auto.submatrix(1, 0, 1, 1).ntt();
            let ginv_ct = gadget_invert(gadget_dim, &ct_auto_0);
            let ginv_ct_ntt = ginv_ct.ntt();
            let w_times_ginv_ct = w * &ginv_ct_ntt;

            let mut idx = 0;
            for j in 0..2 {
                for n in 0..params.crt_count {
                    for z in 0..poly_len {
                        let sum = v[i].data[idx]
                            + w_times_ginv_ct.data[idx]
                            + j * ct_auto_1_ntt.data[n * poly_len + z];
                        v[i].data[idx] = arith::modular_reduce(params, sum, n);
                        idx += 1;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{client::*, util::*};

    use super::*;

    fn get_params() -> Params {
        get_short_keygen_params()
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
        let scale_k = params.modulus / params.pt_modulus;
        let mut sigma = PolyMatrixRaw::zero(&params, 1, 1);
        sigma.data[7] = scale_k;
        v[0] = client.encrypt_matrix_reg(&sigma.ntt());

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
    }
}

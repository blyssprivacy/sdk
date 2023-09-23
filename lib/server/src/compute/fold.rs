use spiral_rs::arith::*;
use spiral_rs::gadget::*;
use spiral_rs::params::*;
use spiral_rs::poly::*;

fn is_all_zeros(v: &[u64]) -> bool {
    for i in 0..v.len() {
        if v[i] != 0 {
            return false;
        }
    }
    true
}

pub fn fold_ciphertexts(
    params: &Params,
    v_cts: &mut [PolyMatrixRaw],
    v_folding: &[PolyMatrixNTT],
    v_folding_neg: &[PolyMatrixNTT],
) {
    if v_cts.len() == 1 {
        return;
    }

    let further_dims = log2(v_cts.len() as u64) as usize;
    let ell = v_folding[0].cols / 2;

    let mut num_per = v_cts.len();
    for cur_dim in 0..further_dims {
        num_per = num_per / 2;
        for i in 0..num_per {
            let mut ginv_c = PolyMatrixRaw::zero(&params, 2 * ell, 1);
            let mut ginv_c_ntt = PolyMatrixNTT::zero(&params, 2 * ell, 1);
            let mut prod = PolyMatrixNTT::zero(&params, 2, 1);
            let mut sum = PolyMatrixNTT::zero(&params, 2, 1);

            // crucial for correctness
            if is_all_zeros(v_cts[i].data.as_slice()) {
                let (p0, p1) = v_cts.split_at_mut(num_per);
                p0[i].copy_into(&p1[i], 0, 0);
                continue;
            } else if is_all_zeros(v_cts[i + num_per].data.as_slice()) {
                continue;
            }

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

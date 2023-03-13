use spiral_rs::arith::*;
use spiral_rs::gadget::*;
use spiral_rs::params::*;
use spiral_rs::poly::*;

pub fn sub_poly_raw(params: &Params, res: &mut [u64], a: &[u64]) {
    for i in 0..params.poly_len {
        res[i] = barrett_u64(params, res[i] + (params.modulus - a[i]))
    }
}

fn sub_raw(res: &mut PolyMatrixRaw, a: &PolyMatrixRaw) {
    assert!(res.rows == a.rows);
    assert!(res.cols == a.cols);

    let params = res.params;
    for i in 0..res.rows {
        for j in 0..res.cols {
            let res_poly = res.get_poly_mut(i, j);
            let pol2 = a.get_poly(i, j);
            sub_poly_raw(params, res_poly, pol2);
        }
    }
}

pub fn add_poly_raw(params: &Params, res: &mut [u64], a: &[u64]) {
    for i in 0..params.poly_len {
        res[i] = barrett_u64(params, res[i] + a[i])
    }
}

fn add_raw(res: &mut PolyMatrixRaw, a: &PolyMatrixRaw) {
    assert!(res.rows == a.rows);
    assert!(res.cols == a.cols);

    let params = res.params;
    for i in 0..res.rows {
        for j in 0..res.cols {
            let res_poly = res.get_poly_mut(i, j);
            let pol2 = a.get_poly(i, j);
            add_poly_raw(params, res_poly, pol2);
        }
    }
}

pub fn fold_ciphertexts(
    params: &Params,
    v_cts: &mut Vec<PolyMatrixRaw>,
    v_folding: &Vec<PolyMatrixNTT>,
) {
    if v_cts.len() == 1 {
        return;
    }

    let further_dims = log2(v_cts.len() as u64) as usize;
    let ell = v_folding[0].cols / 2;
    let mut ginv_c = PolyMatrixRaw::zero(&params, 2 * ell, 1);
    let mut ginv_c_ntt = PolyMatrixNTT::zero(&params, 2 * ell, 1);
    let mut prod = PolyMatrixNTT::zero(&params, 2, 1);
    let mut result = PolyMatrixRaw::zero(&params, 2, 1);
    let mut difference = PolyMatrixRaw::zero(&params, 2, 1);

    let mut num_per = v_cts.len();
    for cur_dim in 0..further_dims {
        num_per = num_per / 2;
        for i in 0..num_per {
            difference.copy_into(&v_cts[num_per + i], 0, 0);
            sub_raw(&mut difference, &v_cts[i]);
            gadget_invert(&mut ginv_c, &difference);
            to_ntt(&mut ginv_c_ntt, &ginv_c);
            multiply(
                &mut prod,
                &v_folding[further_dims - 1 - cur_dim],
                &ginv_c_ntt,
            );
            from_ntt(&mut result, &prod);
            add_raw(&mut v_cts[i], &result);
        }
    }
}

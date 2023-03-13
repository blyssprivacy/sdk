use spiral_rs::gadget::*;
use spiral_rs::params::*;
use spiral_rs::poly::*;

pub fn pack<'a>(
    params: &'a Params,
    v_ct: &[PolyMatrixRaw],
    v_w: &[PolyMatrixNTT],
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

use spiral_rs::gadget::*;
use spiral_rs::params::*;
use spiral_rs::poly::*;

pub fn pack<'a>(
    params: &'a Params,
    v_ct: &[PolyMatrixRaw],
    v_w: &[PolyMatrixNTT],
) -> PolyMatrixNTT<'a> {
    assert!(v_ct.len() >= params.n * params.n);
    assert!(v_w.len() == 2);
    assert!(v_ct[0].rows == 2);
    assert!(v_ct[0].cols == 1);
    assert!(v_w[0].rows == (params.n + 1));
    assert!(v_w[0].cols == params.t_conv);

    let w_key = &v_w[0];
    let w_shift = &v_w[1];

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
            let ct = &v_ct[r * params.n + c];
            ct_1.get_poly_mut(0, 0).copy_from_slice(ct.get_poly(0, 0));
            ct_2.get_poly_mut(0, 0).copy_from_slice(ct.get_poly(1, 0));
            to_ntt(&mut ct_2_ntt, &ct_2);
            gadget_invert(&mut ginv, &ct_1);
            to_ntt(&mut ginv_nttd, &ginv);
            multiply(&mut prod, &w_key, &ginv_nttd);
            add_into_at(&mut prod, &ct_2_ntt, 1, 0);

            // shift until correct position
            for _ in 0..r {
                let prod_ct_1 = prod.submatrix(0, 0, 1, 1);
                let prod_ct_rest = prod.submatrix(1, 0, prod.rows - 1, 1);

                let ginv = gadget_invert_alloc(params.t_conv, &prod_ct_1.raw());
                let shifted_part_1 = w_shift * &ginv.ntt();
                let shifted_part_2 = shift_rows_by_one(&prod_ct_rest).pad_top(1);
                prod = &shifted_part_1 + &shifted_part_2;
            }

            add_into(&mut v_int, &prod);
        }
        result.copy_into(&v_int, 0, c);
    }

    result
}

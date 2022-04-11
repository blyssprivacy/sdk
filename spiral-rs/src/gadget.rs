use crate::{params::*, poly::*};

pub fn get_bits_per(params: &Params, dim: usize) -> usize {
    let modulus_log2 = params.modulus_log2;
    if dim as u64 == modulus_log2 {
        return 1;
    }
    ((modulus_log2 as f64) / (dim as f64)).floor() as usize + 1
}

pub fn build_gadget(params: &Params, rows: usize, cols: usize) -> PolyMatrixRaw {
    let mut g = PolyMatrixRaw::zero(params, rows, cols);
    let nx = g.rows;
    let m = g.cols;

    assert_eq!(m % nx, 0);

    let num_elems = m / nx;
    let params = g.params;
    let bits_per = get_bits_per(params, num_elems);

    for i in 0..nx {
        for j in 0..num_elems {
            if bits_per * j >= 64 {
                continue;
            }
            let poly = g.get_poly_mut(i, i + j * nx);
            poly[0] = 1u64 << (bits_per * j);
        }
    }
    g
}

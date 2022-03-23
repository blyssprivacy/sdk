use std::collections::HashMap;

use crate::{poly::*, params::*, discrete_gaussian::*};

pub struct PublicParameters<'a> {
    v_packing: Vec<PolyMatrixNTT<'a>>,            // Ws
    v_expansion_left: Vec<PolyMatrixNTT<'a>>,
    v_expansion_right: Vec<PolyMatrixNTT<'a>>,
    v_conversion: PolyMatrixNTT<'a>,              // V
}

impl<'a> PublicParameters<'a> {
    fn init(params: &'a Params) -> Self {
        PublicParameters { 
            v_packing: Vec::new(), 
            v_expansion_left: Vec::new(), 
            v_expansion_right: Vec::new(), 
            v_conversion: PolyMatrixNTT::zero(params, 2, 2 * params.m_conv()) 
        }
    }
}

pub struct Client<'a> {
    params: &'a Params,
    sk_gsw: PolyMatrixRaw<'a>,
    sk_reg: PolyMatrixRaw<'a>,
    sk_gsw_full: PolyMatrixRaw<'a>,
    sk_reg_full: PolyMatrixRaw<'a>,
}

fn matrix_with_identity<'a> (p: &PolyMatrixRaw<'a>) -> PolyMatrixRaw<'a> {
    assert_eq!(p.cols, 1);
    let mut r = PolyMatrixRaw::zero(p.params, p.rows, p.rows + 1);
    r.copy_into(p, 0, 0);
    r.copy_into(&PolyMatrixRaw::identity(p.params, p.rows, p.rows), 0, 1);
    r
}

impl<'a> Client<'a> {
    fn init(params: &'a Params) -> Self {
        let sk_gsw_dims = params.get_sk_gsw();
        let sk_reg_dims = params.get_sk_reg();
        let sk_gsw = PolyMatrixRaw::zero(params, sk_gsw_dims.0, sk_gsw_dims.1);
        let sk_reg = PolyMatrixRaw::zero(params, sk_reg_dims.0, sk_reg_dims.1);
        let sk_gsw_full = matrix_with_identity(&sk_gsw);
        let sk_reg_full = matrix_with_identity(&sk_reg);
        Self {
            params,
            sk_gsw,
            sk_reg,
            sk_gsw_full,
            sk_reg_full,
        }
    }
    fn generate_keys(&mut self) -> PublicParameters {
        let params = self.params;
        let mut dg = DiscreteGaussian::init(params);
        dg.sample_matrix(&mut self.sk_gsw);
        dg.sample_matrix(&mut self.sk_reg);
        self.sk_gsw_full = matrix_with_identity(&self.sk_gsw);
        self.sk_reg_full = matrix_with_identity(&self.sk_reg);
        let sk_reg_ntt = to_ntt()
        let pp = PublicParameters::init(params);
        
        // For packing
        for i in 0..params.n {
            MatPoly scaled = from_scalar_multiply(sk_reg_ntt, )
        }

        pp
    }
    // fn generate_query(&self) -> Query<'a, Params>;
    
}
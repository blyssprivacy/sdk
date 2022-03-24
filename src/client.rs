use std::collections::HashMap;

use crate::{poly::*, params::*, discrete_gaussian::*, gadget::*};

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
    dg: DiscreteGaussian,
}

fn matrix_with_identity<'a> (p: &PolyMatrixRaw<'a>) -> PolyMatrixRaw<'a> {
    assert_eq!(p.cols, 1);
    let mut r = PolyMatrixRaw::zero(p.params, p.rows, p.rows + 1);
    r.copy_into(p, 0, 0);
    r.copy_into(&PolyMatrixRaw::identity(p.params, p.rows, p.rows), 0, 1);
    r
}

impl<'a> Client<'a> {
    pub fn init(params: &'a Params) -> Self {
        let sk_gsw_dims = params.get_sk_gsw();
        let sk_reg_dims = params.get_sk_reg();
        let sk_gsw = PolyMatrixRaw::zero(params, sk_gsw_dims.0, sk_gsw_dims.1);
        let sk_reg = PolyMatrixRaw::zero(params, sk_reg_dims.0, sk_reg_dims.1);
        let sk_gsw_full = matrix_with_identity(&sk_gsw);
        let sk_reg_full = matrix_with_identity(&sk_reg);
        let dg = DiscreteGaussian::init(params);
        Self {
            params,
            sk_gsw,
            sk_reg,
            sk_gsw_full,
            sk_reg_full,
            dg,
        }
    }

    fn get_fresh_gsw_public_key(&mut self, m: usize) -> PolyMatrixRaw<'a> {
        let params = self.params;
        let n = params.n;

        let a = PolyMatrixRaw::random(params, 1, m);
        let e = PolyMatrixRaw::noise(params, n, m, &mut self.dg);
        let a_inv = -&a;
        let b_p = &self.sk_gsw.ntt() * &a.ntt();
        let b = &e.ntt() + &b_p;
        let p = stack(&a_inv, &b.raw());
        p
    }

    fn encrypt_matrix_gsw(&mut self, ag: PolyMatrixNTT<'a>) -> PolyMatrixNTT<'a> {
        let params = self.params;
        let mx = ag.cols;
        let p = self.get_fresh_gsw_public_key(mx);
        let res = &(p.ntt()) + &(ag.pad_top(1));
        res
    }

    pub fn generate_keys(&mut self) -> PublicParameters {
        let params = self.params;
        self.dg.sample_matrix(&mut self.sk_gsw);
        self.dg.sample_matrix(&mut self.sk_reg);
        self.sk_gsw_full = matrix_with_identity(&self.sk_gsw);
        self.sk_reg_full = matrix_with_identity(&self.sk_reg);
        let sk_reg_ntt = to_ntt_alloc(&self.sk_reg);
        let m_conv = params.m_conv();

        let mut pp = PublicParameters::init(params);
        
        // Params for packing
        let gadget_conv = build_gadget(params, 1, m_conv);
        let gadget_conv_ntt = to_ntt_alloc(&gadget_conv);
        for i in 0..params.n {
            let scaled = scalar_multiply_alloc(&sk_reg_ntt, &gadget_conv_ntt);
            let mut ag = PolyMatrixNTT::zero(params, params.n, m_conv);
            ag.copy_into(&scaled, i, 0);
            let w = self.encrypt_matrix_gsw(ag);
            pp.v_packing.push(w);
        }

        // Params for expansion

        // Params for converison


        pp
    }
    // fn generate_query(&self) -> Query<'a, Params>;
    
}
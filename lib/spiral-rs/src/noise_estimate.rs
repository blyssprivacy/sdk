use std::f64::consts::{E, PI};

use crate::{
    client::HAMMING_WEIGHT,
    params::{Params, Q2_VALUES},
};

// This a simplified subset of a Params instance
pub struct Paramset {
    pub n: usize,
    pub d: usize,
    pub p: u64,
    pub q: u64,
    pub sigma: f64,
    pub t_conv: usize,
    pub t_exp_left: usize,
    pub t_exp_right: usize,
    pub t_gsw: usize,
    pub db_dim_1: usize,
    pub db_dim_2: usize,
    pub expand_queries: bool,
}

pub fn extract_paramset(params: &Params) -> Paramset {
    Paramset {
        n: params.n,
        d: params.poly_len,
        p: params.pt_modulus,
        q: params.modulus,
        sigma: params.noise_width,
        t_conv: params.t_conv,
        t_exp_left: params.t_exp_left,
        t_exp_right: params.t_exp_right,
        t_gsw: params.t_gsw,
        db_dim_1: params.db_dim_1,
        db_dim_2: params.db_dim_2,
        expand_queries: params.expand_queries,
    }
}

fn get_base(t: usize, q: u64) -> f64 {
    // f64::ceil((q as f64).powf(1. / t as f64))
    let q_f = q as f64;
    let t_f = t as f64;
    let q_bits = f64::ceil(f64::log2(q_f));
    2f64.powf((q_bits / t_f).ceil())
}

fn gadget_exp_factor(s: &Paramset, t: usize, z: f64) -> f64 {
    (t * s.d) as f64 * s.sigma.powi(2) * z.powi(2) / 4f64
}

pub fn get_noise_from_paramset(s: &Paramset) -> f64 {
    let nu1 = s.db_dim_1 as i32;
    let nu2 = s.db_dim_2 as i32;

    let n_used = 1;

    let z_gsw = get_base(s.t_gsw, s.q);
    let m_gsw = (n_used + 1) * s.t_gsw;
    let z_conv = get_base(s.t_conv, s.q);
    let z_exp_left = get_base(s.t_exp_left, s.q);
    let z_exp_right = get_base(s.t_exp_right, s.q);

    let num_exp_reg = s.db_dim_1 + 1;

    let mut sigma_reg_2 = s.sigma.powi(2);
    let mut sigma_gsw_2 = s.sigma.powi(2);

    if s.expand_queries {
        sigma_reg_2 = 4f64.powf(num_exp_reg as f64)
            * s.sigma.powi(2)
            // * (1.0 + ((s.d * s.t_exp_left) as f64 * z_exp_left.powi(2) / 3.));
            * (1.0 + ((s.t_exp_left) as f64 * z_exp_left.powi(2) / 3.));
        // NB: above, we exclude a factor of s.d; this is bad according to the paper, but
        //     in practice, it seems to model the noise accurately

        let num_exp_gsw = f64::ceil(f64::log2((s.t_gsw as f64) * (nu2 as f64))) as i32 + 1;
        sigma_gsw_2 = 4f64.powi(num_exp_gsw)
            * s.sigma.powi(2)
            * (1.0 + ((s.t_exp_right) as f64 * z_exp_right.powi(2) / 3.));
        sigma_gsw_2 = sigma_gsw_2 * 2. * (HAMMING_WEIGHT as f64)
            + 2. * gadget_exp_factor(s, s.t_conv, z_conv);
    }

    let sigma_0_2 = (2f64.powi(nu1))
        * (n_used as f64)
        * (s.d as f64)
        * ((s.p as f64) / 2.).powi(2)
        * (sigma_reg_2);
    let sigma_rest =
        (nu2 as f64) * (s.d as f64) * (m_gsw as f64) * z_gsw.powi(2) / 2. * (sigma_gsw_2);
    let sigma_r_2 = sigma_0_2 + sigma_rest;

    let sigma_packing_2 = ((s.d * s.n * s.t_conv) as f64) * s.sigma.powi(2) * z_conv.powi(2) / 4.;

    sigma_r_2 + sigma_packing_2
}

pub fn get_p_err(s: &Paramset, s_e: f64, q_prime: u64) -> f64 {
    let p_f = s.p as f64;
    let q_prime_f = q_prime as f64;
    let q_f = s.q as f64;

    let q_mod_p = 1;
    let modswitch_adj = (1. / 8.) * ((4. * p_f) * (q_mod_p as f64) / q_f);
    let thresh = (1. / 4.) - modswitch_adj;
    assert!((thresh > 0.) && (thresh < (1. / 4.)));

    let s_round_2 = s.sigma.powi(2) * (s.d as f64) / 4.;
    let numer = -PI * thresh.powi(2);
    let denom = s_e * (p_f / q_f).powi(2) + (s_round_2) * (p_f / q_prime_f).powi(2);

    let p_single_err_log = f64::ln(2.) + (numer / denom);
    let p_err_log = p_single_err_log + f64::ln((s.n * s.n * s.d) as f64);
    let p_err = p_err_log * f64::log(E, 2.);
    p_err
}

pub trait NoiseEstimator {
    fn estimate_noise(&self) -> f64;
    fn estimate_log2_err_prob(&self) -> f64;
}

impl NoiseEstimator for Params {
    fn estimate_noise(&self) -> f64 {
        get_noise_from_paramset(&extract_paramset(self))
    }

    fn estimate_log2_err_prob(&self) -> f64 {
        let q2 = Q2_VALUES[self.q2_bits as usize];
        let paramset = extract_paramset(self);
        let s_e = self.estimate_noise();
        get_p_err(&paramset, s_e, q2)
    }
}

#[cfg(test)]
mod test {
    use crate::util::*;

    use super::*;

    #[test]
    fn get_noise_from_paramset_correct() {
        let cfg_expand = r#"
        {
            "n": 2,
            "nu_1": 9,
            "nu_2": 5,
            "p": 256,
            "q2_bits": 22,
            "t_gsw": 7,
            "t_conv": 3,
            "t_exp_left": 5,
            "t_exp_right": 5,
            "instances": 4,
            "db_item_size": 32768
        }
        "#;
        let params = params_from_json(cfg_expand);
        let s_e = params.estimate_noise();
        let p_err = params.estimate_log2_err_prob();
        let noise_log2 = f64::log2(s_e);
        println!("noise_log2: {}", noise_log2);
        println!("p_err: {}", p_err);
        println!("setup bytes: {}", params.setup_bytes());
        // assert!(noise_log2 < 87.0);
        assert!(p_err <= -40.0);
    }
}

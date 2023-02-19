use crate::{arith::arith::round_raw, matrix::ContractParams};

use super::PARAMS_STORE;

#[derive(Debug, Clone, Copy)]
pub struct Params {
    pub n: usize,   // LWE secret dimension
    pub sigma: f64, // LWE error distribution stddev

    pub l: usize, // DB height
    pub m: usize, // DB width

    pub logq: u64, // (logarithm of) ciphertext modulus
    pub p: u64,    // plaintext modulus
}

impl Params {
    pub fn ext_delta(&self) -> u64 {
        (1 << self.logq) / self.p
    }

    pub fn delta(&self) -> u64 {
        ((self.logq as f64) / (self.p as f64).log2()).ceil() as u64
    }

    pub fn round(&self, x: u64) -> u64 {
        round_raw(x, self.p, self.ext_delta())
    }

    pub fn to_string(&self) -> String {
        format!(
            "{},{},{},{},{},{}",
            self.n, self.sigma, self.l, self.m, self.logq, self.p
        )
    }

    pub fn from_string(inp_str: &str) -> Self {
        let mut inp = inp_str.split(",");
        let n = inp.next().unwrap().parse().unwrap();
        let sigma = inp.next().unwrap().parse().unwrap();
        let l = inp.next().unwrap().parse().unwrap();
        let m = inp.next().unwrap().parse().unwrap();
        let logq = inp.next().unwrap().parse().unwrap();
        let p = inp.next().unwrap().parse().unwrap();
        Self {
            n,
            sigma,
            l,
            m,
            logq,
            p,
        }
    }

    pub fn zero() -> Self {
        Params {
            n: 0,
            l: 0,
            m: 0,
            logq: 0,
            sigma: 0.0,
            p: 0,
        }
    }

    /// Choose parameters, using the input constraints. The free variables are `sigma` and `p`.
    ///
    /// Takes in the number of entries (`n`), the log of the ciphertext modulus (`logq`),
    /// the dimensions of the database (`l` x `m`), and the maximum number of samples for
    /// LWE parameter selection (`max_samples`).
    pub fn pick(n: usize, logq: u64, l: usize, m: usize, max_samples: usize) -> Self {
        let mut params = Params {
            n,
            l,
            m,
            logq,
            sigma: 0.0,
            p: 0,
        };

        for param_set in PARAMS_STORE {
            let (logn, logm, logq, sigma, _, _, p_doublepir) = param_set;

            if (params.n == (1 << logn)) && (max_samples <= (1 << logm)) && (params.logq == (logq))
            {
                params.sigma = sigma;

                params.p = p_doublepir;

                if sigma == 0.0 || params.p == 0 {
                    panic!("Params invalid!");
                }

                // Hack to make some rounding stuff work!
                if params.p == 552 {
                    params.p = 512;
                }

                return params;
            }
        }

        panic!("No suitable params known!");
    }

    pub fn get_contract_params(&self) -> ContractParams {
        ContractParams {
            modulus: self.p as u32,
            delta: self.delta() as usize,
        }
    }
}

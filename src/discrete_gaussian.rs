use rand::{thread_rng, Rng, rngs::ThreadRng};
use rand::distributions::OpenClosed01;

use crate::params::*;
use crate::poly::*;
use std::f64::consts::PI;

pub const NUM_WIDTHS: usize = 8;

pub struct DiscreteGaussian {
    max_val: i64,
    table: Vec<f64>,
    rng: ThreadRng
}

impl DiscreteGaussian {
    pub fn init(params: &Params) -> Self {
        let max_val = (params.noise_width * (NUM_WIDTHS as f64)).ceil() as i64;
        let mut table = vec![0f64; 0];
        let mut sum_p = 0f64;
        table.push(0f64);
        for i in -max_val..max_val+1 {
            let p_val = f64::exp(-PI * f64::powi(i as f64, 2) / f64::powi(params.noise_width, 2));
            table.push(p_val);
            sum_p += p_val;
        }
        for i in 0..table.len() {
            table[i] /= sum_p;
        }
        table.push(1.0);
    
        Self {
            max_val,
            table,
            rng: thread_rng()
        }
    }

    // FIXME: this is not necessarily constant-time w/ optimization
    pub fn sample(&mut self) -> i64 {
        let val: f64 = self.rng.sample(OpenClosed01);
        let mut found = 0i64;
        for i in 0..self.table.len()-1 {
           let bit1: i64 = (val <= self.table[i]) as i64;
           let bit2: i64 = (val > self.table[i+1]) as i64;
            found += bit1 * bit2 * (i as i64);
        }
        found -= self.max_val;
        found
    }

    pub fn sample_matrix(&mut self, p: &mut PolyMatrixRaw) {
        let modulus = p.get_params().modulus;
        for r in 0..p.rows {
            for c in 0..p.cols {
                let poly = p.get_poly_mut(r, c);
                for z in 0..poly.len() {
                    let mut s = self.sample();
                    s += modulus as i64;
                    s %= modulus as i64; // FIXME: not constant time
                    poly[z] = s as u64;
                }
            }
        }
    }
}
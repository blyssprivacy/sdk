mod arith;
mod ntt;
mod number_theory;
mod params;
mod poly;

use crate::params::*;
use crate::poly::*;

fn main() {
    println!("Hello, world!");
    let params = Params::init(2048, vec![7, 31]);
    let m1 = poly::PolyMatrixNTT::zero(&params, 2, 1);
    println!("{}", m1.is_ntt());
    let m2 = poly::PolyMatrixNTT::zero(&params, 3, 2);
    let mut m3 = poly::PolyMatrixNTT::zero(&params, 3, 1);
    println!("{}", m1.is_ntt());
    multiply(&mut m3, &m2, &m1);
    println!("{}", m3.is_ntt());
}

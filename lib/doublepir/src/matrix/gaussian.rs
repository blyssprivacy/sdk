use rand::{thread_rng, Rng};

/// Sample from a gaussian with standard deviation 6.4
pub fn gauss_sample() -> i64 {
    let mut rng = thread_rng();

    // TODO: should ideally use something more robust here
    let val: f64 = rng.sample::<f64, _>(rand_distr::StandardNormal) * 6.4f64;
    val.round() as i64
}

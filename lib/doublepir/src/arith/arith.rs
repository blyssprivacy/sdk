/// Returns the element whose base-p decomposition is given by the values in vals
pub fn reconstruct_from_base_p(p: u64, vals: &[u64]) -> u64 {
    let mut res = 0;
    let mut coeff = 1;
    for (i, v) in vals.iter().enumerate() {
        res += coeff * v;
        if i < vals.len() - 1 {
            coeff *= p;
        }
    }

    res
}

/// Returns the i-th elem in the representation of m in base p.
pub fn base_p(p: u64, mut m: u64, i: u64) -> u64 {
    for _ in 0..i {
        m = m / p
    }
    return m % p;
}

/// Maps value from [-modulus/2, modulus/2] to [0, p].
pub fn centered_to_raw(val: u32, modulus: u32) -> u32 {
    let modulus_u64 = modulus as u64;
    (((val as u64) + (modulus_u64 / 2)) as u32) % modulus
}

/// Maps value from [0, modulus] to [-modulus/2, modulus/2].
pub fn raw_to_centered(val: u32, modulus: u32) -> u32 {
    val.wrapping_sub(modulus / 2)
}

/// Recovers the mod-p value from a noisy value by dividing by ext_delta
/// and rounding.
pub fn round_raw(x: u64, p: u64, ext_delta: u64) -> u64 {
    let v = (x + ext_delta / 2) / ext_delta;
    return v % p;
}

#[cfg(test)]
mod tests {
    use rand::{thread_rng, Rng};

    use super::*;

    #[test]
    fn reconstruct_from_base_p_is_correct() {
        let mut val = [1, 0, 0, 1, 1, 0, 0, 0, 0];
        val.reverse();
        assert_eq!(reconstruct_from_base_p(2, &val), 0b100110000);
    }

    #[test]
    fn base_p_is_correct() {
        assert_eq!(base_p(2, 0b10000000, 0), 0);
        assert_eq!(base_p(2, 0b10000000, 6), 0);
        assert_eq!(base_p(2, 0b10000000, 7), 1);
        assert_eq!(base_p(2, 0b10010000, 4), 1);
        assert_eq!(base_p(17, 0, 0), 0);
        assert_eq!(base_p(17, 0, 6), 0);
        assert_eq!(base_p(17, 0, 100), 0);
    }

    #[test]
    fn reconstruct_from_base_p_and_base_p_are_inverses() {
        let mut rng = thread_rng();
        let p = 12289;
        let v1 = rng.gen::<u64>();
        let num_entries = (2.0f64.powi(64)).log(p as f64).ceil() as u64; // log_p (2^64)

        let mut vals = Vec::new();
        for i in 0..num_entries {
            vals.push(base_p(p as u64, v1 as u64, i as u64));
        }

        let recovered = reconstruct_from_base_p(p as u64, &vals);

        assert_eq!(recovered, v1);
    }

    #[test]
    fn centered_to_raw_and_raw_to_centered_are_inverses() {
        let mut rng = thread_rng();
        let p = rng.gen::<u32>();
        let v1 = rng.gen::<u32>() % p;

        for val in [v1, 0, p - 1, p / 2, p / 2 - 1, p / 2 + 1] {
            assert_eq!(
                centered_to_raw(raw_to_centered(val, p), p),
                val,
                "failed for {} % {}, from raw",
                val,
                p
            );
            assert_eq!(
                raw_to_centered(centered_to_raw(raw_to_centered(val, p), p), p),
                raw_to_centered(val, p),
                "failed for {} % {}, from centered",
                val,
                p
            );
        }
    }
}

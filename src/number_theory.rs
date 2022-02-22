use crate::arith::*;
use rand::Rng;

const ATTEMPT_MAX: usize = 100;

pub fn is_primitive_root(root: u64, degree: u64, modulus: u64) -> bool {
    if root == 0 {
        return false;
    }

    exponentiate_uint_mod(root, degree >> 1, modulus) == modulus - 1
}

pub fn get_primitive_root(degree: u64, modulus: u64) -> Option<u64> {
    assert!(modulus > 1);
    assert!(degree >= 2);
    let size_entire_group = modulus - 1;
    let size_quotient_group = size_entire_group / degree;
    if size_entire_group - size_quotient_group * degree != 0 {
        return None;
    }

    let mut root = 0u64;
    for trial in 0..ATTEMPT_MAX {
        let mut rng = rand::thread_rng();
        let r1: u64 = rng.gen();
        let r2: u64 = rng.gen();
        let r3 = ((r1 << 32) | r2) % modulus;
        root = exponentiate_uint_mod(r3, size_quotient_group, modulus);
        if is_primitive_root(root, degree, modulus) {
            break;
        }
        if trial == ATTEMPT_MAX - 1 {
            return None;
        }
    }

    Some(root)
}

pub fn get_minimal_primitive_root(degree: u64, modulus: u64) -> Option<u64> {
    let mut root = get_primitive_root(degree, modulus)?;
    let generator_sq = multiply_uint_mod(root, root, modulus);
    let mut current_generator = root;

    for _ in 0..degree {
        if current_generator < root {
            root = current_generator;
        }

        current_generator = multiply_uint_mod(current_generator, generator_sq, modulus);
    }

    Some(root)
}

pub fn extended_gcd(mut x: u64, mut y: u64) -> (u64, i64, i64) {
    assert!(x != 0);
    assert!(y != 0);

    let mut prev_a = 1;
    let mut a = 0;
    let mut prev_b = 0;
    let mut b = 1;

    while y != 0 {
        let q: i64 = (x / y) as i64;
        let mut temp = (x % y) as i64;
        x = y;
        y = temp as u64;

        temp = a;
        a = prev_a - (q * a);
        prev_a = temp;

        temp = b;
        b = prev_b - (q * b);
        prev_b = temp;
    }

    (x, prev_a, prev_b)
}

pub fn invert_uint_mod(value: u64, modulus: u64) -> Option<u64> {
    if value == 0 {
        return None;
    }
    let gcd_tuple = extended_gcd(value, modulus);
    if gcd_tuple.0 != 1 {
        return None;
    } else if gcd_tuple.1 < 0 {
        return Some((gcd_tuple.1 as u64).overflowing_add(modulus).0);
    } else {
        return Some(gcd_tuple.1 as u64);
    }
}

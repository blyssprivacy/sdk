use crate::params::*;
use std::mem;

pub fn multiply_uint_mod(a: u64, b: u64, modulus: u64) -> u64 {
    (((a as u128) * (b as u128)) % (modulus as u128)) as u64
}

pub const fn log2(a: u64) -> u64 {
    std::mem::size_of::<u64>() as u64 * 8 - a.leading_zeros() as u64 - 1
}

pub fn multiply_modular(params: &Params, a: u64, b: u64, c: usize) -> u64 {
    (a * b) % params.moduli[c]
}

pub fn multiply_add_modular(params: &Params, a: u64, b: u64, x: u64, c: usize) -> u64 {
    (a * b + x) % params.moduli[c]
}

pub fn exponentiate_uint_mod(operand: u64, mut exponent: u64, modulus: u64) -> u64 {
    if exponent == 0 {
        return 1;
    }

    if exponent == 1 {
        return operand;
    }

    let mut power = operand;
    let mut product;
    let mut intermediate = 1u64;

    loop {
        if (exponent % 2) == 1 {
            product = multiply_uint_mod(power, intermediate, modulus);
            mem::swap(&mut product, &mut intermediate);
        }
        exponent >>= 1;
        if exponent == 0 {
            break;
        }
        product = multiply_uint_mod(power, power, modulus);
        mem::swap(&mut product, &mut power);
    }
    intermediate
}

pub fn reverse_bits(x: u64, bit_count: usize) -> u64 {
    if bit_count == 0 {
        return 0;
    }

    let r = x.reverse_bits();
    r >> (mem::size_of::<u64>() * 8 - bit_count)
}

pub fn div2_uint_mod(operand: u64, modulus: u64) -> u64 {
    if operand & 1 == 1 {
        let res = operand.overflowing_add(modulus);
        if res.1 {
            return (res.0 >> 1) | (1u64 << 63);
        } else {
            return res.0 >> 1;
        }
    } else {
        return operand >> 1;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn div2_uint_mod_correct() {
        assert_eq!(div2_uint_mod(3, 7), 5);
    }
}

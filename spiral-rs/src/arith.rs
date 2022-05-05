use crate::params::*;
use std::mem;
use std::slice;

pub fn multiply_uint_mod(a: u64, b: u64, modulus: u64) -> u64 {
    (((a as u128) * (b as u128)) % (modulus as u128)) as u64
}

pub const fn log2(a: u64) -> u64 {
    std::mem::size_of::<u64>() as u64 * 8 - a.leading_zeros() as u64 - 1
}

pub fn log2_ceil(a: u64) -> u64 {
    f64::ceil(f64::log2(a as f64)) as u64
}

pub fn log2_ceil_usize(a: usize) -> usize {
    f64::ceil(f64::log2(a as f64)) as usize
}

pub fn multiply_modular(params: &Params, a: u64, b: u64, c: usize) -> u64 {
    barrett_coeff_u64(params, a * b, c)
}

pub fn multiply_add_modular(params: &Params, a: u64, b: u64, x: u64, c: usize) -> u64 {
    barrett_coeff_u64(params, a * b + x, c)
}

pub fn add_modular(params: &Params, a: u64, b: u64, c: usize) -> u64 {
    barrett_coeff_u64(params, a + b, c)
}

pub fn invert_modular(params: &Params, a: u64, c: usize) -> u64 {
    params.moduli[c] - a
}

pub fn modular_reduce(params: &Params, x: u64, c: usize) -> u64 {
    barrett_coeff_u64(params, x, c)
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

pub fn recenter(val: u64, from_modulus: u64, to_modulus: u64) -> u64 {
    assert!(from_modulus >= to_modulus);

    let from_modulus_i64 = from_modulus as i64;
    let to_modulus_i64 = to_modulus as i64;

    let mut a_val = val as i64;
    if val >= from_modulus / 2 {
        a_val -= from_modulus_i64;
    }
    a_val = a_val + (from_modulus_i64 / to_modulus_i64) * to_modulus_i64 + 2 * to_modulus_i64;
    a_val %= to_modulus_i64;
    a_val as u64
}

pub fn get_barrett_crs(modulus: u64) -> (u64, u64) {
    let numerator = [0, 0, 1];
    let (_, quotient) = divide_uint192_inplace(numerator, modulus);

    (quotient[0], quotient[1])
}

pub fn get_barrett(moduli: &[u64]) -> ([u64; MAX_MODULI], [u64; MAX_MODULI]) {
    let mut cr0 = [0u64; MAX_MODULI];
    let mut cr1 = [0u64; MAX_MODULI];
    for i in 0..moduli.len() {
        (cr0[i], cr1[i]) = get_barrett_crs(moduli[i]);
    }
    (cr0, cr1)
}

pub fn barrett_raw_u64(input: u64, const_ratio_1: u64, modulus: u64) -> u64 {
    let tmp = (((input as u128) * (const_ratio_1 as u128)) >> 64) as u64;

    // Barrett subtraction
    let res = input - tmp * modulus;

    // One more subtraction is enough
    if res >= modulus {
        res - modulus
    } else {
        res
    }
}

pub fn barrett_u64(params: &Params, val: u64) -> u64 {
    barrett_raw_u64(val, params.barrett_cr_1_modulus, params.modulus)
}

pub fn barrett_coeff_u64(params: &Params, val: u64, n: usize) -> u64 {
    barrett_raw_u64(val, params.barrett_cr_1[n], params.moduli[n])
}

fn split(x: u128) -> (u64, u64) {
    let lo = x & ((1u128 << 64) - 1);
    let hi = x >> 64;
    (lo as u64, hi as u64)
}

fn mul_u128(a: u64, b: u64) -> (u64, u64) {
    let prod = (a as u128) * (b as u128);
    split(prod)
}

fn add_u64(op1: u64, op2: u64, out: &mut u64) -> u64 {
    match op1.checked_add(op2) {
        Some(x) => {
            *out = x;
            0
        }
        None => 1,
    }
}

fn barrett_raw_u128(val: u128, cr0: u64, cr1: u64, modulus: u64) -> u64 {
    let (zx, zy) = split(val);

    let mut tmp1 = 0;
    let mut tmp3;
    let mut carry;
    let (_, prody) = mul_u128(zx, cr0);
    carry = prody;
    let (mut tmp2x, mut tmp2y) = mul_u128(zx, cr1);
    tmp3 = tmp2y + add_u64(tmp2x, carry, &mut tmp1);
    (tmp2x, tmp2y) = mul_u128(zy, cr0);
    carry = tmp2y + add_u64(tmp1, tmp2x, &mut tmp1);
    tmp1 = zy * cr1 + tmp3 + carry;
    tmp3 = zx.wrapping_sub(tmp1.wrapping_mul(modulus));

    tmp3

    // uint64_t zx = val & (((__uint128_t)1 << 64) - 1);
    // uint64_t zy = val >> 64;

    // uint64_t tmp1, tmp3, carry;
    // ulonglong2_h prod = umul64wide(zx, const_ratio_0);
    // carry = prod.y;
    // ulonglong2_h tmp2 = umul64wide(zx, const_ratio_1);
    // tmp3 = tmp2.y + cpu_add_u64(tmp2.x, carry, &tmp1);
    // tmp2 = umul64wide(zy, const_ratio_0);
    // carry = tmp2.y + cpu_add_u64(tmp1, tmp2.x, &tmp1);
    // tmp1 = zy * const_ratio_1 + tmp3 + carry;
    // tmp3 = zx - tmp1 * modulus;

    // return tmp3;
}

fn barrett_reduction_u128_raw(modulus: u64, cr0: u64, cr1: u64, val: u128) -> u64 {
    let mut reduced_val = barrett_raw_u128(val, cr0, cr1, modulus);
    reduced_val -= (modulus) * ((reduced_val >= modulus) as u64);
    reduced_val
}

pub fn barrett_reduction_u128(params: &Params, val: u128) -> u64 {
    let modulus = params.modulus;
    let cr0 = params.barrett_cr_0_modulus;
    let cr1 = params.barrett_cr_1_modulus;
    barrett_reduction_u128_raw(modulus, cr0, cr1, val)
}

// Following code is ported from SEAL (github.com/microsoft/SEAL)

pub fn get_significant_bit_count(val: &[u64]) -> usize {
    for i in (0..val.len()).rev() {
        for j in (0..64).rev() {
            if (val[i] & (1u64 << j)) != 0 {
                return i * 64 + j + 1;
            }
        }
    }
    0
}

fn divide_round_up(num: usize, denom: usize) -> usize {
    (num + (denom - 1)) / denom
}

const BITS_PER_U64: usize = u64::BITS as usize;

fn left_shift_uint192(operand: [u64; 3], shift_amount: usize) -> [u64; 3] {
    let mut result = [0u64; 3];
    if (shift_amount & (BITS_PER_U64 << 1)) != 0 {
        result[2] = operand[0];
        result[1] = 0;
        result[0] = 0;
    } else if (shift_amount & BITS_PER_U64) != 0 {
        result[2] = operand[1];
        result[1] = operand[0];
        result[0] = 0;
    } else {
        result[2] = operand[2];
        result[1] = operand[1];
        result[0] = operand[0];
    }

    let bit_shift_amount = shift_amount & (BITS_PER_U64 - 1);

    if bit_shift_amount != 0 {
        let neg_bit_shift_amount = BITS_PER_U64 - bit_shift_amount;

        result[2] = (result[2] << bit_shift_amount) | (result[1] >> neg_bit_shift_amount);
        result[1] = (result[1] << bit_shift_amount) | (result[0] >> neg_bit_shift_amount);
        result[0] = result[0] << bit_shift_amount;
    }

    result
}

fn right_shift_uint192(operand: [u64; 3], shift_amount: usize) -> [u64; 3] {
    let mut result = [0u64; 3];

    if (shift_amount & (BITS_PER_U64 << 1)) != 0 {
        result[0] = operand[2];
        result[1] = 0;
        result[2] = 0;
    } else if (shift_amount & BITS_PER_U64) != 0 {
        result[0] = operand[1];
        result[1] = operand[2];
        result[2] = 0;
    } else {
        result[2] = operand[2];
        result[1] = operand[1];
        result[0] = operand[0];
    }

    let bit_shift_amount = shift_amount & (BITS_PER_U64 - 1);

    if bit_shift_amount != 0 {
        let neg_bit_shift_amount = BITS_PER_U64 - bit_shift_amount;

        result[0] = (result[0] >> bit_shift_amount) | (result[1] << neg_bit_shift_amount);
        result[1] = (result[1] >> bit_shift_amount) | (result[2] << neg_bit_shift_amount);
        result[2] = result[2] >> bit_shift_amount;
    }

    result
}

fn add_uint64(operand1: u64, operand2: u64, result: &mut u64) -> u8 {
    *result = operand1.wrapping_add(operand2);
    (*result < operand1) as u8
}

fn add_uint64_carry(operand1: u64, operand2: u64, carry: u8, result: &mut u64) -> u8 {
    let operand1 = operand1.wrapping_add(operand2);
    *result = operand1.wrapping_add(carry as u64);
    ((operand1 < operand2) || (!operand1 < (carry as u64))) as u8
}

fn sub_uint64(operand1: u64, operand2: u64, result: &mut u64) -> u8 {
    *result = operand1.wrapping_sub(operand2);
    (operand2 > operand1) as u8
}

fn sub_uint64_borrow(operand1: u64, operand2: u64, borrow: u8, result: &mut u64) -> u8 {
    let diff = operand1.wrapping_sub(operand2);
    *result = diff.wrapping_sub((borrow != 0) as u64);
    ((diff > operand1) || (diff < (borrow as u64))) as u8
}

pub fn sub_uint(operand1: &[u64], operand2: &[u64], uint64_count: usize, result: &mut [u64]) -> u8 {
    let mut borrow = sub_uint64(operand1[0], operand2[0], &mut result[0]);

    for i in 0..uint64_count - 1 {
        let mut temp_result = 0u64;
        borrow = sub_uint64_borrow(operand1[1 + i], operand2[1 + i], borrow, &mut temp_result);
        result[1 + i] = temp_result;
    }

    borrow
}

pub fn add_uint(operand1: &[u64], operand2: &[u64], uint64_count: usize, result: &mut [u64]) -> u8 {
    let mut carry = add_uint64(operand1[0], operand2[0], &mut result[0]);

    for i in 0..uint64_count - 1 {
        let mut temp_result = 0u64;
        carry = add_uint64_carry(operand1[1 + i], operand2[1 + i], carry, &mut temp_result);
        result[1 + i] = temp_result;
    }

    carry
}

pub fn divide_uint192_inplace(mut numerator: [u64; 3], denominator: u64) -> ([u64; 3], [u64; 3]) {
    let mut numerator_bits = get_significant_bit_count(&numerator);
    let mut denominator_bits = get_significant_bit_count(slice::from_ref(&denominator));

    let mut quotient = [0u64; 3];

    if numerator_bits < denominator_bits {
        return (numerator, quotient);
    }

    let uint64_count = divide_round_up(numerator_bits, BITS_PER_U64);

    if uint64_count == 1 {
        quotient[0] = numerator[0] / denominator;
        numerator[0] -= quotient[0] * denominator;
        return (numerator, quotient);
    }

    let mut shifted_denominator = [0u64; 3];
    shifted_denominator[0] = denominator;

    let mut difference = [0u64; 3];

    let denominator_shift = numerator_bits - denominator_bits;

    let shifted_denominator = left_shift_uint192(shifted_denominator, denominator_shift);
    denominator_bits += denominator_shift;

    let mut remaining_shifts = denominator_shift;
    while numerator_bits == denominator_bits {
        if (sub_uint(
            &numerator,
            &shifted_denominator,
            uint64_count,
            &mut difference,
        )) != 0
        {
            if remaining_shifts == 0 {
                break;
            }

            add_uint(
                &difference.clone(),
                &numerator,
                uint64_count,
                &mut difference,
            );

            quotient = left_shift_uint192(quotient, 1);
            remaining_shifts -= 1;
        }

        quotient[0] |= 1;

        numerator_bits = get_significant_bit_count(&difference);
        let mut numerator_shift = denominator_bits - numerator_bits;
        if numerator_shift > remaining_shifts {
            numerator_shift = remaining_shifts;
        }

        if numerator_bits > 0 {
            numerator = left_shift_uint192(difference, numerator_shift);
            numerator_bits += numerator_shift;
        } else {
            for w in 0..uint64_count {
                numerator[w] = 0;
            }
        }

        quotient = left_shift_uint192(quotient, numerator_shift);
        remaining_shifts -= numerator_shift;
    }

    if numerator_bits > 0 {
        numerator = right_shift_uint192(numerator, denominator_shift);
    }

    (numerator, quotient)
}

pub fn recenter_mod(val: u64, small_modulus: u64, large_modulus: u64) -> u64 {
    assert!(val < small_modulus);
    let mut val_i64 = val as i64;
    let small_modulus_i64 = small_modulus as i64;
    let large_modulus_i64 = large_modulus as i64;
    if val_i64 > small_modulus_i64 / 2 {
        val_i64 -= small_modulus_i64;
    }
    if val_i64 < 0 {
        val_i64 += large_modulus_i64;
    }
    val_i64 as u64
}

pub fn rescale(a: u64, inp_mod: u64, out_mod: u64) -> u64 {
    let inp_mod_i64 = inp_mod as i64;
    let out_mod_i128 = out_mod as i128;
    let mut inp_val = (a % inp_mod) as i64;
    if inp_val >= (inp_mod_i64 / 2) {
        inp_val -= inp_mod_i64;
    }
    let sign: i64 = if inp_val >= 0 { 1 } else { -1 };
    let val = (inp_val as i128) * (out_mod as i128);
    let mut result = (val + (sign * (inp_mod_i64 / 2)) as i128) / (inp_mod as i128);
    result = (result + ((inp_mod / out_mod) * out_mod) as i128 + (2 * out_mod_i128)) % out_mod_i128;

    assert!(result >= 0);

    ((result + out_mod_i128) % out_mod_i128) as u64
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::util::get_seeded_rng;
    use rand::Rng;

    fn combine(lo: u64, hi: u64) -> u128 {
        (lo as u128) & ((hi as u128) << 64)
    }

    #[test]
    fn div2_uint_mod_correct() {
        assert_eq!(div2_uint_mod(3, 7), 5);
    }

    #[test]
    fn divide_uint192_inplace_correct() {
        assert_eq!(
            divide_uint192_inplace([35, 0, 0], 7),
            ([0, 0, 0], [5, 0, 0])
        );
        assert_eq!(
            divide_uint192_inplace([0x10101010, 0x2B2B2B2B, 0xF1F1F1F1], 0x1000),
            (
                [0x10, 0, 0],
                [0xB2B0000000010101, 0x1F1000000002B2B2, 0xF1F1F]
            )
        );
    }

    #[test]
    fn get_barrett_crs_correct() {
        assert_eq!(
            get_barrett_crs(268369921u64),
            (16144578669088582089u64, 68736257792u64)
        );
        assert_eq!(
            get_barrett_crs(249561089u64),
            (10966983149909726427u64, 73916747789u64)
        );
        assert_eq!(
            get_barrett_crs(66974689739603969u64),
            (7906011006380390721u64, 275u64)
        );
    }

    #[test]
    fn barrett_reduction_u128_raw_correct() {
        let modulus = 66974689739603969u64;
        let modulus_u128 = modulus as u128;
        let exec = |val| {
            barrett_reduction_u128_raw(66974689739603969u64, 7906011006380390721u64, 275u64, val)
        };
        assert_eq!(exec(modulus_u128), 0);
        assert_eq!(exec(modulus_u128 + 1), 1);
        assert_eq!(exec(modulus_u128 * 7 + 5), 5);

        let mut rng = get_seeded_rng();
        for _ in 0..100 {
            let val = combine(rng.gen(), rng.gen());
            assert_eq!(exec(val), (val % modulus_u128) as u64);
        }
    }

    #[test]
    fn barrett_raw_u64_correct() {
        let modulus = 66974689739603969u64;
        let cr1 = 275u64;

        let mut rng = get_seeded_rng();
        for _ in 0..100 {
            let val = rng.gen();
            assert_eq!(barrett_raw_u64(val, cr1, modulus), val % modulus);
        }
    }
}

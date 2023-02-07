#[cfg(target_feature = "avx2")]
use std::arch::x86_64::*;

use crate::{arith::*, number_theory::*, params::*};

pub fn powers_of_primitive_root(root: u64, modulus: u64, poly_len_log2: usize) -> Vec<u64> {
    let poly_len = 1usize << poly_len_log2;
    let mut root_powers = vec![0u64; poly_len];
    let mut power = root;
    for i in 1..poly_len {
        let idx = reverse_bits(i as u64, poly_len_log2) as usize;
        root_powers[idx] = power;
        power = multiply_uint_mod(power, root, modulus);
    }
    root_powers[0] = 1;
    root_powers
}

pub fn scale_powers_u64(modulus: u64, poly_len: usize, inp: &[u64]) -> Vec<u64> {
    let mut scaled_powers = vec![0; poly_len];
    for i in 0..poly_len {
        let wide_val = (inp[i] as u128) << 64u128;
        let quotient = wide_val / (modulus as u128);
        scaled_powers[i] = quotient as u64;
    }
    scaled_powers
}

pub fn scale_powers_u32(modulus: u32, poly_len: usize, inp: &[u64]) -> Vec<u64> {
    let mut scaled_powers = vec![0; poly_len];
    for i in 0..poly_len {
        let wide_val = inp[i] << 32;
        let quotient = wide_val / (modulus as u64);
        scaled_powers[i] = (quotient as u32) as u64;
    }
    scaled_powers
}

pub fn build_ntt_tables(poly_len: usize, moduli: &[u64]) -> Vec<Vec<Vec<u64>>> {
    let poly_len_log2 = log2(poly_len as u64) as usize;
    let mut output: Vec<Vec<Vec<u64>>> = vec![Vec::new(); moduli.len()];
    for coeff_mod in 0..moduli.len() {
        let modulus = moduli[coeff_mod];
        let modulus_as_u32 = modulus.try_into().unwrap();
        let root = get_minimal_primitive_root(2 * poly_len as u64, modulus).unwrap();
        let inv_root = invert_uint_mod(root, modulus).unwrap();

        let root_powers = powers_of_primitive_root(root, modulus, poly_len_log2);
        let scaled_root_powers = scale_powers_u32(modulus_as_u32, poly_len, root_powers.as_slice());
        let mut inv_root_powers = powers_of_primitive_root(inv_root, modulus, poly_len_log2);
        for i in 0..poly_len {
            inv_root_powers[i] = div2_uint_mod(inv_root_powers[i], modulus);
        }
        let scaled_inv_root_powers =
            scale_powers_u32(modulus_as_u32, poly_len, inv_root_powers.as_slice());

        output[coeff_mod] = vec![
            root_powers,
            scaled_root_powers,
            inv_root_powers,
            scaled_inv_root_powers,
        ];
    }
    output
}

#[cfg(not(target_feature = "avx2"))]
pub fn ntt_forward(params: &Params, operand_overall: &mut [u64]) {
    let log_n = params.poly_len_log2;
    let n = 1 << log_n;

    for coeff_mod in 0..params.crt_count {
        let operand = &mut operand_overall[coeff_mod * n..coeff_mod * n + n];

        let forward_table = params.get_ntt_forward_table(coeff_mod);
        let forward_table_prime = params.get_ntt_forward_prime_table(coeff_mod);
        let modulus_small = params.moduli[coeff_mod] as u32;
        let two_times_modulus_small: u32 = 2 * modulus_small;

        for mm in 0..log_n {
            let m = 1 << mm;
            let t = n >> (mm + 1);

            let mut it = operand.chunks_exact_mut(2 * t);

            for i in 0..m {
                let w = forward_table[m + i];
                let w_prime = forward_table_prime[m + i];

                let op = it.next().unwrap();

                for j in 0..t {
                    let x: u32 = op[j] as u32;
                    let y: u32 = op[t + j] as u32;

                    let curr_x: u32 =
                        x - (two_times_modulus_small * ((x >= two_times_modulus_small) as u32));
                    let q_tmp: u64 = ((y as u64) * (w_prime as u64)) >> 32u64;
                    let q_new = w * (y as u64) - q_tmp * (modulus_small as u64);

                    op[j] = curr_x as u64 + q_new;
                    op[t + j] = curr_x as u64 + ((two_times_modulus_small as u64) - q_new);
                }
            }
        }

        for i in 0..n {
            operand[i] -= ((operand[i] >= two_times_modulus_small as u64) as u64)
                * two_times_modulus_small as u64;
            operand[i] -= ((operand[i] >= modulus_small as u64) as u64) * modulus_small as u64;
        }
    }
}

#[cfg(target_feature = "avx2")]
pub fn ntt_forward(params: &Params, operand_overall: &mut [u64]) {
    let log_n = params.poly_len_log2;
    let n = 1 << log_n;

    for coeff_mod in 0..params.crt_count {
        let operand = &mut operand_overall[coeff_mod * n..coeff_mod * n + n];

        let forward_table = params.get_ntt_forward_table(coeff_mod);
        let forward_table_prime = params.get_ntt_forward_prime_table(coeff_mod);
        let modulus_small = params.moduli[coeff_mod] as u32;
        let two_times_modulus_small: u32 = 2 * modulus_small;

        for mm in 0..log_n {
            let m = 1 << mm;
            let t = n >> (mm + 1);

            let mut it = operand.chunks_exact_mut(2 * t);

            for i in 0..m {
                let w = forward_table[m + i];
                let w_prime = forward_table_prime[m + i];

                let op = it.next().unwrap();

                if t < 4 {
                    for j in 0..t {
                        let x: u32 = op[j] as u32;
                        let y: u32 = op[t + j] as u32;

                        let curr_x: u32 =
                            x - (two_times_modulus_small * ((x >= two_times_modulus_small) as u32));
                        let q_tmp: u64 = ((y as u64) * (w_prime as u64)) >> 32u64;
                        let q_new = w * (y as u64) - q_tmp * (modulus_small as u64);

                        op[j] = curr_x as u64 + q_new;
                        op[t + j] = curr_x as u64 + ((two_times_modulus_small as u64) - q_new);
                    }
                } else {
                    unsafe {
                        for j in (0..t).step_by(4) {
                            // Use AVX2 here
                            let p_x = &mut op[j] as *mut u64;
                            let p_y = &mut op[j + t] as *mut u64;
                            let x = _mm256_load_si256(p_x as *const __m256i);
                            let y = _mm256_load_si256(p_y as *const __m256i);

                            let cmp_val = _mm256_set1_epi64x(two_times_modulus_small as i64);
                            let gt_mask = _mm256_cmpgt_epi64(x, cmp_val);

                            let to_subtract = _mm256_and_si256(gt_mask, cmp_val);
                            let curr_x = _mm256_sub_epi64(x, to_subtract);

                            // uint32_t q_val = ((y) * (uint64_t)(Wprime)) >> 32;
                            let w_prime_vec = _mm256_set1_epi64x(w_prime as i64);
                            let product = _mm256_mul_epu32(y, w_prime_vec);
                            let q_val = _mm256_srli_epi64(product, 32);

                            // q_val = W * y - q_val * modulus_small;
                            let w_vec = _mm256_set1_epi64x(w as i64);
                            let w_times_y = _mm256_mul_epu32(y, w_vec);
                            let modulus_small_vec = _mm256_set1_epi64x(modulus_small as i64);
                            let q_scaled = _mm256_mul_epu32(q_val, modulus_small_vec);
                            let q_final = _mm256_sub_epi64(w_times_y, q_scaled);

                            let new_x = _mm256_add_epi64(curr_x, q_final);
                            let q_final_inverted = _mm256_sub_epi64(cmp_val, q_final);
                            let new_y = _mm256_add_epi64(curr_x, q_final_inverted);

                            _mm256_store_si256(p_x as *mut __m256i, new_x);
                            _mm256_store_si256(p_y as *mut __m256i, new_y);
                        }
                    }
                }
            }
        }

        for i in (0..n).step_by(4) {
            unsafe {
                let p_x = &mut operand[i] as *mut u64;

                let cmp_val1 = _mm256_set1_epi64x(two_times_modulus_small as i64);
                let mut x = _mm256_load_si256(p_x as *const __m256i);
                let mut gt_mask = _mm256_cmpgt_epi64(x, cmp_val1);
                let mut to_subtract = _mm256_and_si256(gt_mask, cmp_val1);
                x = _mm256_sub_epi64(x, to_subtract);

                let cmp_val2 = _mm256_set1_epi64x(modulus_small as i64);
                gt_mask = _mm256_cmpgt_epi64(x, cmp_val2);
                to_subtract = _mm256_and_si256(gt_mask, cmp_val2);
                x = _mm256_sub_epi64(x, to_subtract);
                _mm256_store_si256(p_x as *mut __m256i, x);
            }
        }
    }
}

#[cfg(not(target_feature = "avx2"))]
pub fn ntt_inverse(params: &Params, operand_overall: &mut [u64]) {
    for coeff_mod in 0..params.crt_count {
        let n = params.poly_len;

        let operand = &mut operand_overall[coeff_mod * n..coeff_mod * n + n];

        let inverse_table = params.get_ntt_inverse_table(coeff_mod);
        let inverse_table_prime = params.get_ntt_inverse_prime_table(coeff_mod);
        let modulus = params.moduli[coeff_mod];
        let two_times_modulus: u64 = 2 * modulus;

        for mm in (0..params.poly_len_log2).rev() {
            let h = 1 << mm;
            let t = n >> (mm + 1);

            let mut it = operand.chunks_exact_mut(2 * t);

            for i in 0..h {
                let w = inverse_table[h + i];
                let w_prime = inverse_table_prime[h + i];

                let op = it.next().unwrap();

                for j in 0..t {
                    let x = op[j];
                    let y = op[t + j];

                    let t_tmp = two_times_modulus - y + x;
                    let curr_x = x + y - (two_times_modulus * (((x << 1) >= t_tmp) as u64));
                    let h_tmp = (t_tmp * w_prime) >> 32;

                    let res_x = (curr_x + (modulus * ((t_tmp & 1) as u64))) >> 1;
                    let res_y = w * t_tmp - h_tmp * modulus;

                    op[j] = res_x;
                    op[t + j] = res_y;
                }
            }
        }

        for i in 0..n {
            operand[i] -= ((operand[i] >= two_times_modulus) as u64) * two_times_modulus;
            operand[i] -= ((operand[i] >= modulus) as u64) * modulus;
        }
    }
}

#[cfg(target_feature = "avx2")]
pub fn ntt_inverse(params: &Params, operand_overall: &mut [u64]) {
    for coeff_mod in 0..params.crt_count {
        let n = params.poly_len;

        let operand = &mut operand_overall[coeff_mod * n..coeff_mod * n + n];

        let inverse_table = params.get_ntt_inverse_table(coeff_mod);
        let inverse_table_prime = params.get_ntt_inverse_prime_table(coeff_mod);
        let modulus = params.moduli[coeff_mod];
        let two_times_modulus: u64 = 2 * modulus;
        for mm in (0..params.poly_len_log2).rev() {
            let h = 1 << mm;
            let t = n >> (mm + 1);

            let mut it = operand.chunks_exact_mut(2 * t);

            for i in 0..h {
                let w = inverse_table[h + i];
                let w_prime = inverse_table_prime[h + i];

                let op = it.next().unwrap();

                if t < 4 {
                    for j in 0..t {
                        let x = op[j];
                        let y = op[t + j];

                        let t_tmp = two_times_modulus - y + x;
                        let curr_x = x + y - (two_times_modulus * (((x << 1) >= t_tmp) as u64));
                        let h_tmp = (t_tmp * w_prime) >> 32;

                        let res_x = (curr_x + (modulus * ((t_tmp & 1) as u64))) >> 1;
                        let res_y = w * t_tmp - h_tmp * modulus;

                        op[j] = res_x;
                        op[t + j] = res_y;
                    }
                } else {
                    unsafe {
                        for j in (0..t).step_by(4) {
                            // Use AVX2 here
                            let p_x = &mut op[j] as *mut u64;
                            let p_y = &mut op[j + t] as *mut u64;
                            let x = _mm256_load_si256(p_x as *const __m256i);
                            let y = _mm256_load_si256(p_y as *const __m256i);

                            let modulus_vec = _mm256_set1_epi64x(modulus as i64);
                            let two_times_modulus_vec =
                                _mm256_set1_epi64x(two_times_modulus as i64);
                            let mut t_tmp = _mm256_set1_epi64x(two_times_modulus as i64);
                            t_tmp = _mm256_sub_epi64(t_tmp, y);
                            t_tmp = _mm256_add_epi64(t_tmp, x);
                            let gt_mask = _mm256_cmpgt_epi64(_mm256_slli_epi64(x, 1), t_tmp);
                            let to_subtract = _mm256_and_si256(gt_mask, two_times_modulus_vec);
                            let mut curr_x = _mm256_add_epi64(x, y);
                            curr_x = _mm256_sub_epi64(curr_x, to_subtract);

                            let w_prime_vec = _mm256_set1_epi64x(w_prime as i64);
                            let mut h_tmp = _mm256_mul_epu32(t_tmp, w_prime_vec);
                            h_tmp = _mm256_srli_epi64(h_tmp, 32);

                            let and_mask = _mm256_set_epi64x(1, 1, 1, 1);
                            let eq_mask =
                                _mm256_cmpeq_epi64(_mm256_and_si256(t_tmp, and_mask), and_mask);
                            let to_add = _mm256_and_si256(eq_mask, modulus_vec);

                            let new_x = _mm256_srli_epi64(_mm256_add_epi64(curr_x, to_add), 1);

                            let w_vec = _mm256_set1_epi64x(w as i64);
                            let w_times_t_tmp = _mm256_mul_epu32(t_tmp, w_vec);
                            let h_tmp_times_modulus = _mm256_mul_epu32(h_tmp, modulus_vec);
                            let new_y = _mm256_sub_epi64(w_times_t_tmp, h_tmp_times_modulus);

                            _mm256_store_si256(p_x as *mut __m256i, new_x);
                            _mm256_store_si256(p_y as *mut __m256i, new_y);
                        }
                    }
                }
            }
        }

        for i in 0..n {
            operand[i] -= ((operand[i] >= two_times_modulus) as u64) * two_times_modulus;
            operand[i] -= ((operand[i] >= modulus) as u64) * modulus;
        }

        // for i in (0..n).step_by(4) {
        //     unsafe {
        //         let p_x = &mut operand[i] as *mut u64;

        //         let cmp_val1 = _mm256_set1_epi64x(two_times_modulus as i64);
        //         let mut x = _mm256_load_si256(p_x as *const __m256i);
        //         let mut gt_mask = _mm256_cmpgt_epi64(x, cmp_val1);
        //         let mut to_subtract = _mm256_and_si256(gt_mask, cmp_val1);
        //         x = _mm256_sub_epi64(x, to_subtract);

        //         let cmp_val2 = _mm256_set1_epi64x(modulus as i64);
        //         gt_mask = _mm256_cmpgt_epi64(x, cmp_val2);
        //         to_subtract = _mm256_and_si256(gt_mask, cmp_val2);
        //         x = _mm256_sub_epi64(x, to_subtract);
        //         _mm256_store_si256(p_x as *mut __m256i, x);
        //     }
        // }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{aligned_memory::AlignedMemory64, util::*};
    use rand::Rng;

    fn get_params() -> Params {
        get_test_params()
    }

    const REF_VAL: u64 = 519370102;

    #[test]
    fn build_ntt_tables_correct() {
        let moduli = [268369921u64, 249561089u64];
        let poly_len = 2048usize;
        let res = build_ntt_tables(poly_len, moduli.as_slice());
        assert_eq!(res.len(), 2);
        assert_eq!(res[0].len(), 4);
        assert_eq!(res[0][0].len(), poly_len);
        assert_eq!(res[0][2][0], 134184961u64);
        assert_eq!(res[0][2][1], 96647580u64);
        let mut x1 = 0u64;
        for i in 0..res.len() {
            for j in 0..res[0].len() {
                for k in 0..res[0][0].len() {
                    x1 ^= res[i][j][k];
                }
            }
        }
        assert_eq!(x1, REF_VAL);
    }

    #[test]
    fn ntt_forward_correct() {
        let params = get_params();
        let mut v1 = AlignedMemory64::new(2 * 2048);
        v1[0] = 100;
        v1[2048] = 100;
        ntt_forward(&params, v1.as_mut_slice());
        assert_eq!(v1[50], 100);
        assert_eq!(v1[2048 + 50], 100);
    }

    #[test]
    fn ntt_inverse_correct() {
        let params = get_params();
        let mut v1 = AlignedMemory64::new(2 * 2048);
        for i in 0..v1.len() {
            v1[i] = 100;
        }
        ntt_inverse(&params, v1.as_mut_slice());
        assert_eq!(v1[0], 100);
        assert_eq!(v1[2048], 100);
        assert_eq!(v1[50], 0);
        assert_eq!(v1[2048 + 50], 0);
    }

    #[test]
    fn ntt_correct() {
        let params = get_params();
        let mut v1 = AlignedMemory64::new(params.crt_count * params.poly_len);
        let mut rng = rand::thread_rng();
        for i in 0..params.crt_count {
            for j in 0..params.poly_len {
                let idx = calc_index(&[i, j], &[params.crt_count, params.poly_len]);
                let val: u64 = rng.gen();
                v1[idx] = val % params.moduli[i];
            }
        }
        let mut v2 = v1.clone();
        ntt_forward(&params, v2.as_mut_slice());
        ntt_inverse(&params, v2.as_mut_slice());
        for i in 0..params.crt_count * params.poly_len {
            assert_eq!(v1[i], v2[i]);
        }
    }

    #[test]
    fn calc_index_correct() {
        assert_eq!(calc_index(&[2, 3, 4], &[10, 10, 100]), 2304);
        assert_eq!(calc_index(&[2, 3, 4], &[3, 5, 7]), 95);
    }
}

use crate::{
    arith::*,
    number_theory::*,
    params::*,
    poly::*,
    util::*,
};

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
        let mut scaled_inv_root_powers =
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

pub fn ntt_forward(params: &Params, operand_overall: &mut [u64]) {
    let log_n = params.poly_len_log2;
    let n = 1 << log_n;

    for coeff_mod in 0..params.crt_count {
        let operand = &mut operand_overall[coeff_mod*n..coeff_mod*n+n];

        let forward_table = params.get_ntt_forward_table(coeff_mod);
        let forward_table_prime = params.get_ntt_forward_prime_table(coeff_mod);
        let modulus_small = params.moduli[coeff_mod] as u32;
        let two_times_modulus_small: u32 = 2 * modulus_small;

        for mm in 0..log_n {
            let m = 1 << mm;
            let t = n >> (mm + 1);

            let mut it = operand.chunks_exact_mut(2 * t);

            for i in 0..m {
                let w = forward_table[m+i];
                let w_prime = forward_table_prime[m+i];
                
                let op = it.next().unwrap();

                for j in 0..t {
                    let x: u32 = op[j] as u32;
                    let y: u32 = op[t + j] as u32;

                    let currX: u32 = x - (two_times_modulus_small * ((x >= two_times_modulus_small) as u32));

                    let Q: u64 = ((y as u64) * (w_prime as u64)) >> 32u64;

                    let new_Q = w * (y as u64) - Q * (modulus_small as u64);

                    op[j] = currX as u64 + new_Q;
                    op[t + j] = currX as u64 +  ((two_times_modulus_small as u64) - new_Q);
                }
            }
        }

        for i in 0..n {
            operand[i] -= ((operand[i] >= two_times_modulus_small as u64) as u64) * two_times_modulus_small as u64;
            operand[i] -= ((operand[i] >= modulus_small as u64) as u64) * modulus_small as u64;
        }
    }
}

pub fn ntt_inverse(params: &Params, operand_overall: &mut [u64]) {
    for coeff_mod in 0..params.crt_count {
        let mut n = params.poly_len;

        let operand = &mut operand_overall[coeff_mod*n..coeff_mod*n+n];

        let inverse_table = params.get_ntt_inverse_table(coeff_mod);
        let inverse_table_prime = params.get_ntt_inverse_prime_table(coeff_mod);
        let modulus = params.moduli[coeff_mod];
        let two_times_modulus: u64 = 2 * modulus;

        for mm in (0..params.poly_len_log2).rev() {
            let h = 1 << mm;
            let t = n >> (mm + 1);

            for i in 0..h {
                let w = inverse_table[h+i];
                let w_prime = inverse_table_prime[h+i];

                for j in 0..t {
                    let x = operand[2 * i * t + j];
                    let y = operand[2 * i * t + t + j];
                    
                    let T = two_times_modulus - y + x;
                    let currU = x + y - (two_times_modulus * (((x << 1) >= T) as u64));

                    let resX= (currU + (modulus * ((T & 1) as u64))) >> 1;
                    let H = (T * w_prime) >> 32;

                    operand[2 * i * t + j] = resX;
                    operand[2 * i * t + t + j] = w * T - H * modulus;
                }
            }
        }

        for i in 0..n {
            operand[i] -= ((operand[i] >= two_times_modulus) as u64) * two_times_modulus;
            operand[i] -= ((operand[i] >= modulus) as u64) * modulus;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rand::Rng;

    fn get_params() -> Params {
        Params::init(2048, vec![268369921u64, 249561089u64])
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
        let mut v1 = vec![0; 2*2048];
        v1[0] = 100;
        v1[2048] = 100;
        ntt_forward(&params, v1.as_mut_slice());
        assert_eq!(v1[50], 100);
        assert_eq!(v1[2048 + 50], 100);
    }

    #[test]
    fn ntt_correct() {
        let params = get_params();
        let mut v1 = vec![0; params.crt_count * params.poly_len];
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
        for i in 0..params.crt_count*params.poly_len {
            assert_eq!(v1[i], v2[i]);
        }
    }

    #[test]
    fn calc_index_correct() {
        assert_eq!(calc_index(&[2, 3, 4], &[10, 10, 100]), 2304);
        assert_eq!(calc_index(&[2, 3, 4], &[3, 5, 7]), 95);
    }
}
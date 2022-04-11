use crate::params::*;
use serde_json::Value;

pub fn calc_index(indices: &[usize], lengths: &[usize]) -> usize {
    let mut idx = 0usize;
    let mut prod = 1usize;
    for i in (0..indices.len()).rev() {
        idx += indices[i] * prod;
        prod *= lengths[i];
    }
    idx
}

pub fn get_test_params() -> Params {
    Params::init(
        2048,
        &vec![268369921u64, 249561089u64],
        6.4,
        2,
        256,
        20,
        4,
        8,
        56,
        8,
        true,
        9,
        6,
        1,
        2048,
    )
}

pub const fn get_empty_params() -> Params {
    Params {
        poly_len: 0,
        poly_len_log2: 0,
        ntt_tables: Vec::new(),
        scratch: Vec::new(),
        crt_count: 0,
        moduli: Vec::new(),
        modulus: 0,
        modulus_log2: 0,
        noise_width: 0f64,
        n: 0,
        pt_modulus: 0,
        q2_bits: 0,
        t_conv: 0,
        t_exp_left: 0,
        t_exp_right: 0,
        t_gsw: 0,
        expand_queries: false,
        db_dim_1: 0,
        db_dim_2: 0,
        instances: 0,
        db_item_size: 0,
    }
}

pub fn params_from_json(cfg: &str) -> Params {
    let v: Value = serde_json::from_str(cfg).unwrap();
    let n = v["n"].as_u64().unwrap() as usize;
    let db_dim_1 = v["nu_1"].as_u64().unwrap() as usize;
    let db_dim_2 = v["nu_2"].as_u64().unwrap() as usize;
    let instances = v["instances"].as_u64().unwrap_or(1) as usize;
    let db_item_size = v["db_item_size"].as_u64().unwrap_or(1) as usize;
    let p = v["p"].as_u64().unwrap();
    let q2_bits = v["q_prime_bits"].as_u64().unwrap();
    let t_gsw = v["t_GSW"].as_u64().unwrap() as usize;
    let t_conv = v["t_conv"].as_u64().unwrap() as usize;
    let t_exp_left = v["t_exp"].as_u64().unwrap() as usize;
    let t_exp_right = v["t_exp_right"].as_u64().unwrap() as usize;
    let do_expansion = v.get("kinda_direct_upload").is_none();
    Params::init(
        2048,
        &vec![268369921u64, 249561089u64],
        6.4,
        n,
        p,
        q2_bits,
        t_conv,
        t_exp_left,
        t_exp_right,
        t_gsw,
        do_expansion,
        db_dim_1,
        db_dim_2,
        instances,
        db_item_size,
    )
}

pub fn read_arbitrary_bits(data: &[u8], bit_offs: usize, num_bits: usize) -> u64 {
    let word_off = bit_offs / 64;
    let bit_off_within_word = bit_offs % 64;
    if (bit_off_within_word + num_bits) <= 64 {
        let idx = word_off * 8;
        let val = u64::from_ne_bytes(data[idx..idx + 8].try_into().unwrap());
        (val >> bit_off_within_word) & ((1u64 << num_bits) - 1)
    } else {
        let idx = word_off * 8;
        let val = u128::from_ne_bytes(data[idx..idx + 16].try_into().unwrap());
        ((val >> bit_off_within_word) & ((1u128 << num_bits) - 1)) as u64
    }
}

pub fn write_arbitrary_bits(data: &mut [u8], mut val: u64, bit_offs: usize, num_bits: usize) {
    let word_off = bit_offs / 64;
    let bit_off_within_word = bit_offs % 64;
    val = val & ((1u64 << num_bits) - 1);
    if (bit_off_within_word + num_bits) <= 64 {
        let idx = word_off * 8;
        let mut cur_val = u64::from_ne_bytes(data[idx..idx + 8].try_into().unwrap());
        cur_val &= !(((1u64 << num_bits) - 1) << bit_off_within_word);
        cur_val |= val << bit_off_within_word;
        data[idx..idx + 8].copy_from_slice(&u64::to_ne_bytes(cur_val));
    } else {
        let idx = word_off * 8;
        let mut cur_val = u128::from_ne_bytes(data[idx..idx + 16].try_into().unwrap());
        let mask = !(((1u128 << num_bits) - 1) << bit_off_within_word);
        cur_val &= mask;
        cur_val |= (val as u128) << bit_off_within_word;
        data[idx..idx + 16].copy_from_slice(&u128::to_ne_bytes(cur_val));
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn params_from_json_correct() {
        let cfg = r#"
            {'n': 2,
            'nu_1': 9,
            'nu_2': 6,
            'p': 256,
            'q_prime_bits': 20,
            's_e': 87.62938774292914,
            't_GSW': 8,
            't_conv': 4,
            't_exp': 8,
            't_exp_right': 56,
            'instances': 1,
            'db_item_size': 2048 }
        "#;
        let cfg = cfg.replace("'", "\"");
        let b = params_from_json(&cfg);
        let c = Params::init(
            2048,
            &vec![268369921u64, 249561089u64],
            6.4,
            2,
            256,
            20,
            4,
            8,
            56,
            8,
            true,
            9,
            6,
            1,
            2048,
        );
        assert_eq!(b, c);
    }

    #[test]
    fn test_read_write_arbitrary_bits() {
        let len = 4096;
        let num_bits = 9;
        let mut data = vec![0u8; len];
        let scaled_len = len * 8 / num_bits - 64;
        let mut bit_offs = 0;
        let get_from = |i: usize| -> u64 { ((i * 7 + 13) % (1 << num_bits)) as u64 };
        for i in 0..scaled_len {
            write_arbitrary_bits(data.as_mut_slice(), get_from(i), bit_offs, num_bits);
            bit_offs += num_bits;
        }
        bit_offs = 0;
        for i in 0..scaled_len {
            let val = read_arbitrary_bits(data.as_slice(), bit_offs, num_bits);
            assert_eq!(val, get_from(i));
            bit_offs += num_bits;
        }
    }
}

use crate::{arith::*, client::Seed, params::*, poly::*};
use rand::{prelude::SmallRng, thread_rng, Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde_json::Value;
use std::fs;

pub const CFG_20_256: &'static str = r#"
        {'n': 2,
        'nu_1': 9,
        'nu_2': 6,
        'p': 256,
        'q2_bits': 20,
        's_e': 87.62938774292914,
        't_gsw': 8,
        't_conv': 4,
        't_exp_left': 8,
        't_exp_right': 56,
        'instances': 1,
        'db_item_size': 8192 }
    "#;
pub const CFG_16_100000: &'static str = r#"
        {'n': 2,
        'nu_1': 10,
        'nu_2': 6,
        'p': 512,
        'q2_bits': 21,
        's_e': 85.83255142749422,
        't_gsw': 10,
        't_conv': 4,
        't_exp_left': 16,
        't_exp_right': 56,
        'instances': 11,
        'db_item_size': 100000 }
    "#;

pub fn calc_index(indices: &[usize], lengths: &[usize]) -> usize {
    let mut idx = 0usize;
    let mut prod = 1usize;
    for i in (0..indices.len()).rev() {
        idx += indices[i] * prod;
        prod *= lengths[i];
    }
    idx
}

pub fn decompose_index(indices: &mut [usize], index: usize, lengths: &[usize]) {
    let mut cur = index;
    let mut prod = 1usize;
    for i in 1..lengths.len() {
        prod *= lengths[i];
    }

    for i in 0..lengths.len() {
        let val = cur / prod;
        cur -= val * prod;
        indices[i] = val;
        if i < lengths.len() - 1 {
            prod /= lengths[i + 1];
        }
    }
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
        0,
    )
}

pub fn get_short_keygen_params() -> Params {
    Params::init(
        2048,
        &vec![268369921u64, 249561089u64],
        6.4,
        2,
        256,
        20,
        4,
        4,
        4,
        4,
        true,
        9,
        6,
        1,
        2048,
        0,
    )
}

pub fn get_expansion_testing_params() -> Params {
    let cfg = r#"
        {'n': 2,
        'nu_1': 9,
        'nu_2': 6,
        'p': 256,
        'q2_bits': 20,
        't_gsw': 8,
        't_conv': 4,
        't_exp_left': 8,
        't_exp_right': 56,
        'instances': 1,
        'db_item_size': 8192 }
    "#;
    params_from_json(&cfg.replace("'", "\""))
}

pub fn get_fast_expansion_testing_params() -> Params {
    let cfg = r#"
        {'n': 2,
        'nu_1': 6,
        'nu_2': 2,
        'p': 256,
        'q2_bits': 20,
        't_gsw': 8,
        't_conv': 4,
        't_exp_left': 8,
        't_exp_right': 8,
        'instances': 1,
        'db_item_size': 8192 }
    "#;
    params_from_json(&cfg.replace("'", "\""))
}

pub fn get_no_expansion_testing_params() -> Params {
    let cfg = r#"
        {'direct_upload': 1,
        'n': 5,
        'nu_1': 6,
        'nu_2': 3,
        'p': 65536,
        'q2_bits': 27,
        't_gsw': 3,
        't_conv': 56,
        't_exp_left': 56,
        't_exp_right': 56}
    "#;
    params_from_json(&cfg.replace("'", "\""))
}

pub fn get_seed() -> u64 {
    thread_rng().gen::<u64>()
}

pub fn get_seeded_rng() -> SmallRng {
    SmallRng::seed_from_u64(get_seed())
}

pub fn get_chacha_seed() -> Seed {
    thread_rng().gen::<[u8; 32]>()
}

pub fn get_chacha_rng() -> ChaCha20Rng {
    ChaCha20Rng::from_seed(get_chacha_seed())
}

pub fn get_static_seed() -> u64 {
    0x123456789
}

pub fn get_chacha_static_seed() -> Seed {
    [
        0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8, 0x9, 0xa, 0xb, 0xc, 0xd, 0xe, 0xf, 0x0, 0x1,
        0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8, 0x9, 0xa, 0xb, 0xc, 0xd, 0xe, 0xf,
    ]
}

pub fn get_static_seeded_rng() -> SmallRng {
    SmallRng::seed_from_u64(get_static_seed())
}

pub const fn get_empty_params() -> Params {
    Params {
        poly_len: 0,
        poly_len_log2: 0,
        ntt_tables: Vec::new(),
        scratch: Vec::new(),
        crt_count: 0,
        barrett_cr_0_modulus: 0,
        barrett_cr_1_modulus: 0,
        barrett_cr_0: [0u64; MAX_MODULI],
        barrett_cr_1: [0u64; MAX_MODULI],
        mod0_inv_mod1: 0,
        mod1_inv_mod0: 0,
        moduli: [0u64; MAX_MODULI],
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
        version: 0,
    }
}

pub fn params_from_json(cfg: &str) -> Params {
    let v: Value = serde_json::from_str(cfg).unwrap();
    params_from_json_obj(&v)
}

pub fn params_from_json_obj(v: &Value) -> Params {
    let n = v["n"].as_u64().unwrap() as usize;
    let db_dim_1 = v["nu_1"].as_u64().unwrap() as usize;
    let db_dim_2 = v["nu_2"].as_u64().unwrap() as usize;
    let instances = v["instances"].as_u64().unwrap_or(1) as usize;
    let p = v["p"].as_u64().unwrap();
    let q2_bits = u64::max(v["q2_bits"].as_u64().unwrap(), MIN_Q2_BITS);
    let t_gsw = v["t_gsw"].as_u64().unwrap() as usize;
    let t_conv = v["t_conv"].as_u64().unwrap() as usize;
    let t_exp_left = v["t_exp_left"].as_u64().unwrap() as usize;
    let t_exp_right = v["t_exp_right"].as_u64().unwrap() as usize;
    let do_expansion = v.get("direct_upload").is_none();

    let mut db_item_size = v["db_item_size"].as_u64().unwrap_or(0) as usize;
    if db_item_size == 0 {
        db_item_size = instances * n * n;
        db_item_size = db_item_size * 2048 * log2_ceil(p) as usize / 8;
    }

    let version = v["version"].as_u64().unwrap_or(0) as usize;

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
        version,
    )
}

static ALL_PARAMS_STORE_FNAME: &str = "../params_store.json";

pub fn get_params_from_store(target_num_log2: usize, item_size: usize) -> Params {
    let params_store_str = fs::read_to_string(ALL_PARAMS_STORE_FNAME).unwrap();
    let v: Value = serde_json::from_str(&params_store_str).unwrap();
    let nearest_target_num = target_num_log2;
    let nearest_item_size = 1 << usize::max(log2_ceil_usize(item_size), 8);
    println!(
        "Starting with parameters for 2^{} x {} bytes...",
        nearest_target_num, nearest_item_size
    );
    let target = v
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_object().unwrap())
        .filter(|x| x.get("target_num").unwrap().as_u64().unwrap() == (nearest_target_num as u64))
        .filter(|x| x.get("item_size").unwrap().as_u64().unwrap() == (nearest_item_size as u64))
        .map(|x| x.get("params").unwrap())
        .next()
        .unwrap();
    params_from_json_obj(target)
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

pub fn reorient_reg_ciphertexts(params: &Params, out: &mut [u64], v_reg: &Vec<PolyMatrixNTT>) {
    let poly_len = params.poly_len;
    let crt_count = params.crt_count;

    assert_eq!(crt_count, 2);
    assert!(log2(params.moduli[0]) <= 32);

    let num_reg_expanded = 1 << params.db_dim_1;
    let ct_rows = v_reg[0].rows;
    let ct_cols = v_reg[0].cols;

    assert_eq!(ct_rows, 2);
    assert_eq!(ct_cols, 1);

    for j in 0..num_reg_expanded {
        for r in 0..ct_rows {
            for m in 0..ct_cols {
                for z in 0..params.poly_len {
                    let idx_a_in =
                        r * (ct_cols * crt_count * poly_len) + m * (crt_count * poly_len);
                    let idx_a_out = z * (num_reg_expanded * ct_cols * ct_rows)
                        + j * (ct_cols * ct_rows)
                        + m * (ct_rows)
                        + r;
                    let val1 = v_reg[j].data[idx_a_in + z] % params.moduli[0];
                    let val2 = v_reg[j].data[idx_a_in + params.poly_len + z] % params.moduli[1];

                    out[idx_a_out] = val1 | (val2 << 32);
                }
            }
        }
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
            'q2_bits': 20,
            's_e': 87.62938774292914,
            't_gsw': 8,
            't_conv': 4,
            't_exp_left': 8,
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
            0,
        );
        assert_eq!(b, c);
    }

    #[test]
    fn test_decompose_calc_correct() {
        let lengths = [5, 4, 3];
        let indices = [2, 1, 2];
        let idx = calc_index(&indices, &lengths);
        let mut gues_indices = [0, 0, 0];
        decompose_index(&mut gues_indices, idx, &lengths);
        assert_eq!(indices, gues_indices);
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

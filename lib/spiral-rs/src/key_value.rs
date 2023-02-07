use crate::params::Params;
use sha2::{Digest, Sha256};

const VARINT_MAX_BYTES: usize = 8;
const MAX_VARINT_BITS: u64 = 63;

pub fn varint_decode(data: &[u8]) -> (usize, usize) {
    let mut shift = 0u64;
    let mut result = 0u64;
    let mut j = 0;

    while shift < MAX_VARINT_BITS {
        let i = data[j] as u64;
        j += 1;
        result |= (i & 0x7f) << shift;
        shift += 7;
        if i & 0x80 == 0 {
            break;
        }
    }

    (result as usize, j)
}

pub fn row_from_key(params: &Params, key: &str) -> usize {
    let num_items = params.num_items();
    let buckets_log2 = (num_items as f64).log2().ceil() as usize;

    let hash = Sha256::digest(key.as_bytes());

    // let idx = read_arbitrary_bits(&hash, 0, buckets_log2) as usize;
    let mut idx = 0;
    for i in 0..buckets_log2 {
        let cond = hash[i / 8] & (1 << (7 - (i % 8)));
        if cond != 0 {
            idx += 1 << (buckets_log2 - i - 1);
        }
    }
    idx
}

pub fn extract_result_impl(key: &str, result: &[u8]) -> Result<Vec<u8>, &'static str> {
    let hash_bytes = result[0] as usize;
    let hash = Sha256::digest(key.as_bytes());
    let target = &hash[(hash.len() - hash_bytes)..];
    let mut i = 1;
    while i < result.len() {
        // read key
        let key_hash = &result[i..i + hash_bytes];
        i += hash_bytes;

        // read len
        let (value_len, value_len_len) = varint_decode(&result[i..i + VARINT_MAX_BYTES]);
        i += value_len_len;

        // read value
        let value = &result[i..i + value_len];
        i += value_len;

        if key_hash == target {
            return Ok(value.to_vec());
        }
    }

    Err("key not found")
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::util::*;

    fn get_params() -> Params {
        params_from_json(
            r#"{
            "n": 4,
            "nu_1": 9,
            "nu_2": 5,
            "p": 256,
            "q2_bits": 20,
            "t_gsw": 8,
            "t_conv": 4,
            "t_exp_left": 8,
            "t_exp_right": 56,
            "instances": 2,
            "db_item_size": 65536
        }"#,
        )
    }

    #[test]
    fn row_from_key_is_correct() {
        let params = get_params();
        assert_eq!(row_from_key(&params, "CA"), 4825);
        assert_eq!(row_from_key(&params, "OR"), 8359);
    }
}

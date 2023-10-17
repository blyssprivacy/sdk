use std::{collections::HashMap, io::Read};

use bzip2::{read::BzEncoder, Compression};
use sha2::{Digest, Sha256};
use spiral_rs::params::Params;

use super::{loading::update_item_raw, sparse_db::SparseDb};

pub fn row_from_key(num_items: usize, key: &str) -> usize {
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

pub fn hash_key(key: &str, key_hash_bytes: usize) -> Vec<u8> {
    let hash = Sha256::digest(key.as_bytes());
    (&hash)[hash.len() - key_hash_bytes..].to_vec()
}

pub fn varint_encode(mut number: u64) -> Vec<u8> {
    let mut buf = Vec::new();
    loop {
        let to_write = (number & 0x7F) as u8;
        number >>= 7;
        if number != 0 {
            buf.push(to_write | 0x80);
        } else {
            buf.push(to_write);
            break;
        }
    }
    buf
}

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

const DEFAULT_KEY_HASH_BYTES: u8 = 8;

pub fn update_row(row: &mut Vec<u8>, key: &str, value: &[u8]) {
    if row.len() == 0 {
        row.push(DEFAULT_KEY_HASH_BYTES);
    }

    let key_hash_bytes = row[0] as usize;

    let mut target_key_hash = hash_key(key, key_hash_bytes);

    let mut i = 1;
    let mut found_start = false;
    let mut found_end = false;
    let mut start = 0;
    let mut end = 0;
    while i < row.len() {
        // read key
        let key_hash = &row[i..i + key_hash_bytes];
        i += key_hash_bytes;

        if key_hash == target_key_hash {
            found_start = true;
            start = i;
        }

        // read len
        let (value_len, value_len_len) = varint_decode(&row[i..i + VARINT_MAX_BYTES]);
        i += value_len_len;

        // read value
        i += value_len as usize;

        if key_hash == target_key_hash {
            found_end = true;
            end = i;
        }
    }

    if found_start {
        assert!(found_end);
    }

    let mut new_value = value.to_vec();

    if value.len() == 0 {
        // deleting this key, so also delete the key hash
        assert!(found_start);
        start -= key_hash_bytes;
    } else {
        new_value = varint_encode(value.len() as u64);
        new_value.append(&mut value.to_vec());
    }

    if found_start {
        row.splice(start..end, new_value);
    } else {
        row.append(&mut target_key_hash);
        row.append(&mut new_value);
    }
}

pub fn unwrap_kv_pairs(data: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut kv_pairs = Vec::new();

    // Parse the data as a JSON object
    if let Ok(json_data) = serde_json::from_slice::<HashMap<String, String>>(data) {
        for (key, base64_value) in json_data.iter() {
            // Decode the Base64-encoded value
            if let Ok(decoded_value) = base64::decode(base64_value) {
                kv_pairs.push((key.clone(), decoded_value));
            }
        }
    }
    // print KV pairs
    println!("kv_pairs: {:?}", kv_pairs);

    kv_pairs
}

pub fn update_database(
    params: &Params,
    kv_pairs: &[(&str, &[u8])],
    rows: &mut [Vec<u8>],
    db: &mut SparseDb,
) {
    let mut row_id_to_keys = HashMap::new();
    let mut keys_to_values = HashMap::new();
    for (k, v) in kv_pairs {
        keys_to_values.insert(*k, *v);
        let row_id = row_from_key(rows.len(), k);
        if !row_id_to_keys.contains_key(&row_id) {
            row_id_to_keys.insert(row_id, Vec::new());
        }
        row_id_to_keys.get_mut(&row_id).unwrap().push(*k);
    }

    let mut row_ids_to_update = row_id_to_keys.keys().collect::<Vec<_>>();
    row_ids_to_update.sort();

    for row_id in row_ids_to_update {
        let row_data = &mut rows[*row_id];

        let keys_to_update = row_id_to_keys.get(row_id).unwrap();
        for key in keys_to_update {
            let value = keys_to_values.get(key).unwrap();
            update_row(row_data, *key, *value);
        }

        let mut compressor = BzEncoder::new(row_data.as_slice(), Compression::best());
        let mut compressed = Vec::new();
        compressor.read_to_end(&mut compressed).unwrap();

        update_item_raw(params, *row_id, &compressed, db).unwrap();
    }
}

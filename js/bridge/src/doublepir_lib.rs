use doublepir_rs::{doublepir::*, pir::PirClient};
use serde_json::{self, Value};
use std::convert::TryInto;
use wasm_bindgen::prelude::*;

use sha2::{Digest, Sha256};

fn row_from_key(num_entries: usize, key: &str) -> usize {
    let buckets_log2 = (num_entries as f64).log2().ceil() as usize;

    let hash = Sha256::digest(key.as_bytes());

    let mut idx = 0;
    for i in 0..buckets_log2 {
        let cond = hash[i / 8] & (1 << (7 - (i % 8)));
        if cond != 0 {
            idx += 1 << (buckets_log2 - i - 1);
        }
    }
    idx
}

fn extract_result_impl(result: &[u8]) -> bool {
    let val = u64::from_ne_bytes(result.try_into().unwrap());
    val != 0
}

// Container class for a static lifetime DoublePirClient
// Avoids a lifetime in the return signature of bound Rust functions
#[wasm_bindgen]
pub struct DoublePIRApiClient {
    client: &'static mut DoublePirClient,
    index: usize,
    state: Vec<u8>,
}

#[wasm_bindgen]
impl DoublePIRApiClient {
    pub fn initialize_client(json_params: Option<String>) -> DoublePIRApiClient {
        let param_str = json_params.unwrap();
        let v: Value = serde_json::from_str(&param_str).unwrap();

        let num_entries = v["num_entries"].as_u64().unwrap() as usize;
        let bits_per_entry = v["bits_per_entry"].as_u64().unwrap() as usize;

        let raw_client = DoublePirClient::new(num_entries, bits_per_entry);
        let client = Box::leak(Box::new(raw_client));

        DoublePIRApiClient {
            client,
            index: 0,
            state: Vec::new(),
        }
    }

    pub fn generate_query(&mut self, idx_target: usize) -> Box<[u8]> {
        self.index = idx_target;
        let (query, state) = self.client.generate_query(idx_target);
        self.state = state;
        return query.into_boxed_slice();
    }

    pub fn load_hint(&mut self, hint: Box<[u8]>) {
        self.client.load_hint(&hint);
    }

    pub fn decode_response(&self, data: Box<[u8]>) -> Box<[u8]> {
        self.client
            .decode_response(&data, self.index, &self.state)
            .into_boxed_slice()
    }

    pub fn get_row(&self, key: &str) -> u32 {
        row_from_key(self.client.num_entries(), key) as u32
    }

    pub fn extract_result(&self, result: &[u8]) -> bool {
        extract_result_impl(result)
    }
}

use doublepir_rs::{
    database::DbInfo,
    doublepir::*,
    matrix::Matrix,
    matrix::SquishParams,
    params::Params,
    pir::PirClient,
    serializer::Serialize,
    util::{SEEDS, SEEDS_SHORT},
};
use js_sys::Promise;
use serde_json::{self, Value};
use std::{
    convert::TryInto,
    fmt::Write,
    time::{Duration, Instant},
};
use wasm_bindgen::prelude::*;

use sha1::{Digest, Sha1};
use sha2::Sha256;
use wasm_bindgen_futures::JsFuture;
use web_sys::console;

fn row_from_key(num_entries: u64, key: &str) -> u64 {
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

fn top_be_bits(data: &[u8], bits: usize) -> u64 {
    let mut idx = 0;
    for i in 0..bits {
        let cond = data[i / 8] & (1 << (7 - (i % 8)));
        if cond != 0 {
            idx += 1 << (bits - i - 1);
        }
    }
    idx
}

fn get_bloom_indices(val: &str, k: usize, log2m: usize) -> Vec<u64> {
    let mut out = Vec::new();
    for k_i in 0..k {
        let val_to_hash = format!("{}", k_i) + val;
        let hash = Sha1::digest(val_to_hash);
        let inp_idx = top_be_bits(&hash, log2m);
        let idx = (inp_idx / 8) * 8 + (7 - (inp_idx % 8));
        out.push(idx);
    }
    out
}

fn bytes_to_hex_upper(data: &[u8]) -> String {
    static CHARS: &'static [u8] = b"0123456789ABCDEF";
    let mut s = String::with_capacity(data.as_ref().len() * 2);

    for &byte in data.iter() {
        s.write_char(CHARS[(byte >> 4) as usize].into()).unwrap();
        s.write_char(CHARS[(byte & 0xf) as usize].into()).unwrap();
    }

    s
}

fn get_key_bloom_indices(key: &str, k: usize, log2m: usize) -> Vec<u64> {
    let hash = Sha1::digest(key);
    let key_str = bytes_to_hex_upper(&hash);

    get_bloom_indices(&key_str, k, log2m)
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
    index: u64,
    state: Vec<u8>,
    indices: Vec<u64>,
    states: Vec<Vec<u8>>,
    query_plan: Vec<Option<(u64, u64)>>,
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window)]
    fn aes_derive_fast_1(ctr: u64, dst: *mut u8, len: u32) -> Promise;

    #[wasm_bindgen(js_namespace = window)]
    fn aes_derive_fast_2(ctr: u64, dst: *mut u8, len: u32) -> Promise;
}

fn derive_fast(seed: &[u8; 16], ctr: u64, dst: &mut [u8]) -> JsFuture {
    if SEEDS_SHORT[0][0] == seed[0] {
        console::log_1(&format!("{:?} {:?}", dst.as_mut_ptr(), dst.len()).into());
        JsFuture::from(aes_derive_fast_1(ctr, dst.as_mut_ptr(), dst.len() as u32))
    } else {
        JsFuture::from(aes_derive_fast_2(ctr, dst.as_mut_ptr(), dst.len() as u32))
    }
}

#[wasm_bindgen]
impl DoublePIRApiClient {
    pub async fn initialize_client(json_params: Option<String>) -> DoublePIRApiClient {
        console_error_panic_hook::set_once();

        let param_str = json_params.unwrap();
        let v: Value = serde_json::from_str(&param_str).unwrap();

        let num_entries = v["num_entries"].as_str().unwrap().parse::<u64>().unwrap();
        let bits_per_entry = v["bits_per_entry"].as_u64().unwrap() as usize;

        let raw_client = DoublePirClient::with_params_derive_fast(
            &Params::from_string("1024,6.4,92681,92683,32,464"),
            &DbInfo {
                num_entries,
                bits_per_entry: bits_per_entry as u64,
                packing: 8,
                ne: 1,
                x: 1,
                p: 464,
                logq: 32,
                squish_params: SquishParams::default(),
                orig_cols: 92683,
            },
            derive_fast,
        )
        .await;
        let client = Box::leak(Box::new(raw_client));

        DoublePIRApiClient {
            client,
            index: 0,
            state: Vec::new(),
            indices: Vec::new(),
            states: Vec::new(),
            query_plan: Vec::new(),
        }
    }

    pub fn generate_query(&mut self, idx_target: u64) -> Box<[u8]> {
        Vec::new().into_boxed_slice()
    }

    pub fn generate_query_batch(&mut self, indices: Vec<u64>) -> Box<[u8]> {
        self.indices = indices.clone();
        console::log_1(&format!("sending: {:?}", indices).into());
        let (queries, client_states, query_plan) = self.client.generate_query_batch(&indices);
        self.states = client_states;
        self.query_plan = query_plan;
        queries.serialize().into_boxed_slice()
    }

    pub fn load_hint(&mut self, hint: Box<[u8]>) {
        self.client.load_hint(&hint);
    }

    pub fn decode_response(&self, data: Box<[u8]>) -> Box<[u8]> {
        self.client
            .decode_response(&data, self.index, &self.state)
            .into_boxed_slice()
    }

    pub fn decode_response_batch(&self, data: Box<[u8]>) -> Vec<i32> {
        let mut out = Vec::<i32>::new();
        for (batch_idx, client_state) in self.states.iter().enumerate() {
            let planned_query = self.query_plan[batch_idx];
            if planned_query.is_none() {
                out.push(-1);
                println!("could not get query (batch: {})", batch_idx);
                continue;
            }
            let planned_query = planned_query.unwrap();
            let index_to_query_in_batch = planned_query.1;

            let result = u64::from_ne_bytes(
                self.client
                    .decode_response_impl(&data, index_to_query_in_batch, batch_idx, &client_state)
                    .as_slice()
                    .try_into()
                    .unwrap(),
            );

            out.push(result as i32);
        }
        out
    }

    pub fn get_row(&self, key: &str) -> u64 {
        row_from_key(self.client.num_entries(), key) as u64
    }

    pub fn get_bloom_indices(&self, key: &str, k: usize, log2m: usize) -> Vec<u64> {
        get_key_bloom_indices(key, k, log2m)
    }

    pub fn extract_result(&self, result: &[u8]) -> bool {
        extract_result_impl(result)
    }
}

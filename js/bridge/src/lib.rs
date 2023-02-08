use spiral_rs::client::*;
use spiral_rs::params::Params;
use spiral_rs::util::*;

use std::convert::TryInto;
use wasm_bindgen::prelude::*;

pub mod doublepir_lib;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[allow(unused_macros)]
macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

pub struct ApiClientObj<'a> {
    pub params: &'a Params,
    pub c: Client<'a>,
}

// Container class for a static lifetime ApiClientObj
// Avoids a lifetime in the return signature of bound Rust functions
#[wasm_bindgen]
pub struct ApiClient {
    client: &'static mut ApiClientObj<'static>,
}

#[wasm_bindgen]
pub fn initialize_client(json_params: Option<String>) -> ApiClient {
    let mut cfg = DEFAULT_PARAMS.to_owned();
    if json_params.is_some() {
        cfg = json_params.unwrap();
    }

    let params = Box::leak(Box::new(params_from_json(&cfg)));
    let client = Box::leak(Box::new(ApiClientObj {
        params,
        c: Client::init(params),
    }));

    ApiClient { client }
}

#[wasm_bindgen]
pub fn generate_keys(
    c: &mut ApiClient,
    seed: Box<[u8]>,
    generate_pub_params: bool,
) -> Option<Box<[u8]>> {
    let seed_val = (*seed).try_into().unwrap();
    let result = c
        .client
        .c
        .generate_keys_optional(seed_val, generate_pub_params)?
        .into_boxed_slice();
    Some(result)
}

#[wasm_bindgen]
pub fn generate_query(c: &mut ApiClient, id: &str, idx_target: usize) -> Box<[u8]> {
    c.client
        .c
        .generate_full_query(id, idx_target)
        .into_boxed_slice()
}

#[wasm_bindgen]
pub fn decode_response(c: &mut ApiClient, data: Box<[u8]>) -> Box<[u8]> {
    c.client.c.decode_response(&*data).into_boxed_slice()
}

#[wasm_bindgen]
pub fn get_row(c: &mut ApiClient, key: &str) -> u32 {
    spiral_rs::key_value::row_from_key(c.client.params, key) as u32
}

#[wasm_bindgen]
pub fn extract_result(_c: &mut ApiClient, key: &str, result: &[u8]) -> Option<Vec<u8>> {
    spiral_rs::key_value::extract_result_impl(key, result).ok()
}

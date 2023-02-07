use pyo3::prelude::*;

use spiral_rs::client::*;
use spiral_rs::key_value::*;
use spiral_rs::params::Params;
use spiral_rs::util::*;

use std::convert::TryInto;

pub struct ApiClientObj<'a> {
    pub params: &'a Params,
    pub c: Client<'a>,
}

// Container class for a static lifetime ApiClientObj
// Avoids a lifetime in the return signature of bound Rust functions
#[pyclass]
pub struct ApiClient {
    client: &'static mut ApiClientObj<'static>,
}

#[pyfunction]
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

#[pyfunction]
pub fn generate_keys(
    c: &mut ApiClient,
    seed: Vec<u8>,
    generate_pub_params: bool,
) -> Option<Vec<u8>> {
    let seed_val = (*seed).try_into().unwrap();
    Some(
        c.client
            .c
            .generate_keys_optional(seed_val, generate_pub_params)?,
    )
}

#[pyfunction]
pub fn generate_query(c: &mut ApiClient, id: &str, idx_target: usize) -> Vec<u8> {
    c.client.c.generate_full_query(id, idx_target)
}

#[pyfunction]
pub fn decode_response(c: &mut ApiClient, data: Vec<u8>) -> Vec<u8> {
    c.client.c.decode_response(&*data)
}

#[pyfunction]
pub fn get_row(c: &mut ApiClient, key: &str) -> u32 {
    row_from_key(c.client.params, key) as u32
}

#[pyfunction]
pub fn extract_result(_c: &mut ApiClient, key: &str, result: &[u8]) -> Option<Vec<u8>> {
    extract_result_impl(key, result).ok()
}

#[pymodule]
fn blyss(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(initialize_client, m)?)?;
    m.add_function(wrap_pyfunction!(generate_keys, m)?)?;
    m.add_function(wrap_pyfunction!(generate_query, m)?)?;
    m.add_function(wrap_pyfunction!(decode_response, m)?)?;
    m.add_function(wrap_pyfunction!(get_row, m)?)?;
    m.add_function(wrap_pyfunction!(extract_result, m)?)?;
    m.add_class::<ApiClient>()?;
    Ok(())
}

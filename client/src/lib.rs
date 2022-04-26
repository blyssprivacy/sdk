mod utils;
use rand::{rngs::ThreadRng, thread_rng};
use spiral_rs::{client::*, discrete_gaussian::*, params::*, util::*};
use wasm_bindgen::prelude::*;

const UUID_V4_LEN: usize = 36;

// console_log! macro
// #[wasm_bindgen]
// extern "C" {
//     #[wasm_bindgen(js_namespace = console)]
//     fn log(s: &str);
// }
// macro_rules! console_log {
//     ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
// }

// Container class for a static lifetime Client
// Avoids a lifetime in the return signature of bound Rust functions
#[wasm_bindgen]
pub struct WrappedClient {
    client: Client<'static, ThreadRng>,
}

// Unsafe global with a static lifetime
// Accessed unsafely only once, at load / setup
static mut PARAMS: Params = get_empty_params();
static mut RNG: Option<ThreadRng> = None;

// Very simply test to ensure random generation is not obviously biased.
fn dg_seems_okay() {
    let params = get_test_params();
    let mut rng = thread_rng();
    let mut dg = DiscreteGaussian::init(&params, &mut rng);
    let mut v = Vec::new();
    let trials = 10000;
    let mut sum = 0;
    for _ in 0..trials {
        let val = dg.sample();
        v.push(val);
        sum += val;
    }
    let mean = sum as f64 / trials as f64;
    let std_dev = params.noise_width / f64::sqrt(2f64 * std::f64::consts::PI);
    let std_dev_of_mean = std_dev / f64::sqrt(trials as f64);
    assert!(f64::abs(mean) < std_dev_of_mean * 5f64);
}

// Initializes a client; can optionally take in a set of parameters
#[wasm_bindgen]
pub fn initialize(json_params: Option<String>) -> WrappedClient {
    dg_seems_okay();
    // spiral_rs::ntt::test::ntt_correct();
    let cfg = r#"
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
    let mut cfg = cfg.replace("'", "\"");
    if json_params.is_some() {
        cfg = json_params.unwrap();
    }
    let client;

    // this minimal unsafe operation is need to initialize state
    unsafe {
        PARAMS = params_from_json(&cfg);
        RNG = Some(thread_rng());
        client = Client::init(&PARAMS, RNG.as_mut().unwrap());
    }

    WrappedClient { client }
}

#[wasm_bindgen]
pub fn generate_public_parameters(c: &mut WrappedClient) -> Box<[u8]> {
    c.client.generate_keys().serialize().into_boxed_slice()
}

#[wasm_bindgen]
pub fn generate_query(c: &mut WrappedClient, id: &str, idx_target: usize) -> Box<[u8]> {
    assert_eq!(id.len(), UUID_V4_LEN);
    let query = c.client.generate_query(idx_target);
    let mut query_buf = query.serialize();
    let mut full_query_buf = id.as_bytes().to_vec();
    full_query_buf.append(&mut query_buf);
    full_query_buf.into_boxed_slice()
}

#[wasm_bindgen]
pub fn decode_response(c: &mut WrappedClient, data: Box<[u8]>) -> Box<[u8]> {
    c.client.decode_response(&*data).into_boxed_slice()
}

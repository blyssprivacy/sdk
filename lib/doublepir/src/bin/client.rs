use base64::{engine::general_purpose, Engine};
use std::{
    borrow::Cow,
    collections::HashMap,
    env,
    fmt::Write,
    time::{Duration, Instant},
};

use doublepir_rs::{doublepir::*, serializer::Serialize};
use reqwest::blocking::multipart;
use sha1::{Digest, Sha1};

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
        println!("idx: {} ({}, {})", idx, idx / 8, idx % 8);
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

fn get_password_bloom_indices(password: &str, k: usize, log2m: usize) -> Vec<u64> {
    let hash = Sha1::digest(password);
    let key_str = bytes_to_hex_upper(&hash);

    println!("key_str: {}", key_str);

    get_bloom_indices(&key_str, k, log2m)
}

fn post_data_to_server(data: Vec<u8>, server_url: &str) -> Vec<u8> {
    let http_client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(360))
        .build()
        .unwrap();
    let resp = http_client
        .post(server_url)
        .body(Vec::<u8>::new())
        .send()
        .unwrap();
    let result: serde_json::Value = resp.json().unwrap();
    println!("got {:?}", result);
    let post_url = result.get("url").unwrap().as_str().unwrap();
    let uuid = result.get("uuid").unwrap().as_str().unwrap();
    let fields = result.get("fields").unwrap().as_object().unwrap();
    let mut form = multipart::Form::new();
    for (field, value) in fields.iter() {
        form = form.text(
            Cow::Owned(field.clone()),
            Cow::Owned(value.as_str().unwrap().clone().to_owned()),
        );
    }
    let form = form.part("file", multipart::Part::bytes(data));
    let resp = http_client.post(post_url).multipart(form).send().unwrap();
    println!("{:?}", resp.status());
    resp.error_for_status().unwrap();

    let mut hmap = HashMap::<String, String>::new();
    hmap.insert("uuid".to_string(), uuid.to_string());
    let resp = http_client.post(server_url).json(&hmap).send().unwrap();

    let answer = resp.bytes().unwrap().to_vec();
    println!("got answer len: {}", answer.len());
    answer
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let num_entries: u64 = args[1].parse().unwrap();
    let bits_per_entry: u64 = args[2].parse().unwrap();
    let data_file_name: String = args[3].parse().unwrap();
    let server_url: String = args[4].parse().unwrap();
    let password: String = args[5].parse().unwrap();
    let k: usize = args[6].parse().unwrap();
    assert_eq!(bits_per_entry, 1);

    // let rng = thread_rng();
    // let num_queries = 2;
    // let indices_to_query: Vec<usize> = rng
    //     .sample_iter::<usize, _>(Standard)
    //     .take(num_queries)
    //     .map(|x| x % num_entries)
    //     .collect();
    let indices_to_query =
        get_password_bloom_indices(&password, k, f64::log2(num_entries as f64).floor() as usize);
    println!("Querying {:?}", indices_to_query);

    let params = DoublePirClient::params_from_file(&format!("{}.params", data_file_name));
    let dbinfo = DoublePirClient::dbinfo_from_file(&format!("{}.dbinfo", data_file_name));
    let mut client = DoublePirClient::with_params(&params, &dbinfo);
    println!(
        "Loaded. Params: {:?} {:?}",
        *client.params_ref(),
        *client.dbinfo_ref()
    );

    // Client loads the hint
    client.load_hint_from_file(&format!("{}.hint", data_file_name));

    // Client generate the queries
    let (queries, client_states, query_plan) = client.generate_query_batch(&indices_to_query);

    // Server processes the queries
    let query_bytes = queries.serialize();
    println!("query raw size: {}", query_bytes.len());

    let start = Instant::now();
    // let answer = server.answer(&query_bytes);
    // let http_client = reqwest::blocking::Client::builder()
    //     .timeout(Duration::from_secs(360))
    //     .build()
    //     .unwrap();
    // let resp = http_client
    //     .post(&server_url)
    //     .body(query_bytes)
    //     .send()
    //     .unwrap();
    // let answer = resp.bytes().unwrap().to_vec();
    let answer = post_data_to_server(query_bytes, &server_url);
    println!("Answer took {} us", start.elapsed().as_micros());

    println!("Answer len: {}", answer.len());
    // println!("Answer len: {:?}", std::str::from_utf8(&answer).unwrap());

    if answer.len() < 1000 {
        println!("{:?}", answer);
        println!("{:?}", String::from_utf8(answer.clone()).unwrap());
    }

    let answer_data = if server_url.starts_with("http://localhost") {
        answer
    } else {
        general_purpose::STANDARD.decode(&answer).unwrap()
    };

    // Client decodes the answer
    for (batch_idx, client_state) in client_states.iter().enumerate() {
        let planned_query = query_plan[batch_idx];
        if planned_query.is_none() {
            println!("could not get query (batch: {})", batch_idx);
            continue;
        }
        let planned_query = planned_query.unwrap();
        let index_to_query = planned_query.0;
        let index_to_query_in_batch = planned_query.1;

        println!("retrieved query {} (batch: {})", index_to_query, batch_idx);

        let result = u64::from_ne_bytes(
            client
                .decode_response_impl(
                    &answer_data,
                    index_to_query_in_batch,
                    batch_idx,
                    &client_state,
                )
                .as_slice()
                .try_into()
                .unwrap(),
        );

        let corr_result = 1; //(index_to_query % 2) as u64;

        // Check if correct
        println!("Got {}, expected {}", result, corr_result);
        assert_eq!(result, corr_result);
    }
}

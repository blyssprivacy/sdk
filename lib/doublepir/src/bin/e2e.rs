use std::{env, ops::AddAssign, time::Instant};

use doublepir_rs::{
    doublepir::*,
    matrix::Matrix,
    pir::PirServer,
    serializer::{DeserializeSlice, Serialize},
    util::checksum_u32,
};
// use rand::thread_rng;

pub fn round_to_multiple(x: u64, delta: u64) -> u64 {
    let v = (x + delta / 2) / delta;
    return v * delta;
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let num_entries: u64 = args[1].parse().unwrap();
    let bits_per_entry: u64 = args[2].parse().unwrap();
    let data_file_name: String = args[3].parse().unwrap();
    assert_eq!(bits_per_entry, 1);

    // let mut rng = thread_rng();
    // let index_to_query = rng.gen::<usize>() % num_entries;
    let indices_to_query = vec![
        42589995014,
        6766736080,
        42461265606,
        29863406032,
        59769687430,
        33912068661,
        37794679486,
        43925156523,
    ];
    let num_chunks = indices_to_query.len();
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

    let query_bytes = queries.serialize();

    // Server processes the queries
    let mut server = DoublePirServer::new(num_entries, bits_per_entry as usize);
    println!("{:?} {:?}", *server.params_ref(), *server.dbinfo_ref());
    server.restore_from_files(&data_file_name, true, true);

    let start = Instant::now();
    let mut responses = Vec::new();
    for chunk_idx in 1..num_chunks + 1 {
        let db_rows = server.db_ref().num_rows();
        let db_cols = server.db_ref().num_cols();
        let total_sz_bytes = db_rows * db_cols * 4;
        let batch_sz = db_rows / num_chunks;
        let batch_sz_bytes = batch_sz * db_cols * 4;

        let start = (chunk_idx - 1) * batch_sz_bytes;
        let mut end = start + batch_sz_bytes;

        if chunk_idx == num_chunks {
            end = total_sz_bytes;
        }

        println!("at chunk {} getting slice: {}-{}", chunk_idx, start, end);

        let data_slc_u8 = &server.db_ref().raw_data[start..end];
        let data_slc = unsafe {
            let ptr = data_slc_u8.as_ptr() as *const u32;
            let slice: &[u32] = std::slice::from_raw_parts(ptr, data_slc_u8.len() / 4);
            slice
        };

        println!("checksum u32 {}", checksum_u32(data_slc));

        let response = server.answer_inline(&query_bytes, data_slc, Some(chunk_idx - 1));
        responses.push(response);
    }

    let mut full_response = Vec::new();
    for chunk_idx in 0..num_chunks {
        let response = Vec::<Matrix>::deserialize(&responses[chunk_idx]);
        if chunk_idx == 0 {
            full_response.extend(response.into_iter());
        } else {
            for resp_idx in 0..response.len() {
                if resp_idx % 2 == 1 {
                    continue;
                }
                full_response[resp_idx].add_assign(response[resp_idx].clone());
            }
        }
    }

    println!("Answer took {} us", start.elapsed().as_micros());

    let answer_bytes = full_response.serialize();

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
                    &answer_bytes,
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

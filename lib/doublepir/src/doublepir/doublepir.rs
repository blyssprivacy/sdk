use std::time::Instant;

use crate::{database::*, info, matrix::*, params::Params, serializer::State, util::*};

// Ratio between first-level DB and second-level DB
const COMP_RATIO: usize = 64;

/// Fixed scheme value for the log of the ciphertext modulus.
pub const LOGQ: u64 = 32;

/// Fixed scheme value for the LWE dimension.
pub const SEC_PARAM: usize = 1usize << 10;

/// Maximum considered plaintext modulus
const MAX_SEARCH_P: u64 = 1 << 20;

pub fn pick_params(num_entries: usize, d: u64, n: usize, logq: u64) -> Params {
    let mut good_p = Params::zero();
    let mut found = false;

    let mut mod_p = 2;

    // Iteratively refine p and DB dims, until find tight values
    while mod_p < MAX_SEARCH_P {
        let (l, m) = approx_database_dims(num_entries, d, mod_p, COMP_RATIO * n);

        let p = Params::pick(n, logq, l, m, usize::max(l, m));
        if p.p < mod_p {
            if !found {
                panic!("Error; should not happen")
            }
            info!("params: {:?}", good_p);
            return good_p;
        }

        good_p = p;
        found = true;

        mod_p += 1;
    }

    panic!("Could not find params");
}

/// Returns shared_state
pub fn init(info: &DbInfo, params: &Params) -> State {
    let a_1 = Matrix::derive_from_seed(params.m, params.n, SEEDS[0]);
    let a_2 = Matrix::derive_from_seed(params.l / (info.x as usize), params.n, SEEDS[1]);

    vec![a_1, a_2]
}

/// Returns (server_state, hint)
pub fn setup(db: &mut Db, shared: &State, params: &Params) -> (State, State) {
    let a_1 = shared[0].clone();
    let a_2 = shared[1].clone();

    let start = Instant::now();
    let mut h_1 = &db.data * &a_1; // (l, m) * (m, n) = (l, n)
    println!("mul took {} us", start.elapsed().as_micros());
    h_1.transpose(); // (n, l)
    h_1 = h_1.expand(&params.get_contract_params());
    h_1.concat_cols(db.info.x as usize); // (n * delta * x, l / x)

    let h_2 = &h_1 * &a_2;

    // pack the database more tightly, because the online computation is memory-bound
    db.data += (params.p / 2) as u32;
    db.squish();

    h_1 += (params.p / 2) as u32;
    h_1 = h_1.squish(&SquishParams::default());

    let mut a_2_copy = a_2.clone(); // deep copy whole matrix
    if a_2_copy.rows % 3 != 0 {
        a_2_copy.concat(&Matrix::new(3 - (a_2_copy.rows % 3), a_2_copy.cols))
    }
    a_2_copy.transpose();

    h_1.print_checksum("H1");
    a_2_copy.print_checksum("A2_copy");
    h_2.print_checksum("H2");
    db.data.print_checksum("DB.data");

    (vec![h_1, a_2_copy], vec![h_2])
}

/// Returns (client_state, query)
pub fn query(i: usize, shared: &State, params: &Params, info: &DbInfo) -> (State, State) {
    let mut idx_to_query = i;
    if info.packing > 0 {
        idx_to_query = idx_to_query / info.packing;
    }
    let i1 = (idx_to_query / params.m) * (info.ne / info.x) as usize;
    let i2 = idx_to_query % params.m;

    println!("{} -> {},{}", i, i1, i2);

    let a_1 = shared[0].clone();
    a_1.print_dims("a_1");
    let a_2 = shared[1].clone();

    let secret1 = Matrix::random_logmod(params.n, 1, params.logq as u32);
    let err1 = Matrix::gaussian(params.m, 1);
    let mut query1 = &a_1 * &secret1;
    query1 += err1;
    query1.data[i2] += params.ext_delta() as u32;

    let squishing = info.squish_params.delta;
    if params.m % squishing != 0 {
        query1.append_zeros(squishing - (params.m % squishing))
    }

    query1.print_checksum("query1");

    let mut state = vec![secret1];
    let mut msg = vec![query1];

    for j in 0..info.ne / info.x {
        // let secret2 = Matrix::random_logmod(params.n, 1, params.logq as u32);

        // Use error distribution secret instead of uniform
        let secret2 = Matrix::gaussian(params.n, 1);
        secret2.print_dims("secret2");
        let err2 = Matrix::gaussian(params.l / info.x as usize, 1);
        let mut query2 = &a_2 * &secret2;
        query2 += err2;
        query2.print_dims("query2");
        query2.data[i1 + j as usize] += params.ext_delta() as u32;

        if (params.l / info.x as usize) % squishing != 0 {
            query2.append_zeros(squishing - ((params.l / info.x as usize) % squishing));
        }

        query2.print_checksum("query2");

        state.push(secret2);
        msg.push(query2);
    }

    (state, msg)
}

/// Returns answer.
pub fn answer(
    db: &Db,
    queries: &[State],
    server: &State,
    _shared: &State,
    params: &Params,
    raw_data: Option<&[u32]>,
    chunk_idx: Option<usize>,
) -> State {
    let h_1 = &server[0];
    let a_2_transpose = &server[1];

    let mut a_1 = Matrix::new(0, 0);
    let num_queries = queries.len();
    println!("got num_queries: {}", num_queries);
    let mut batch_sz = db.num_rows() / num_queries;

    let mut last = 0;
    // selects a column from each batch of rows
    for (batch, q) in queries.iter().enumerate() {
        if batch == num_queries - 1 {
            batch_sz = db.num_rows() - last;
        }

        let mut start_row = last;
        if let Some(cur_chunk_idx) = chunk_idx {
            start_row = 0;

            if batch != cur_chunk_idx {
                last += batch_sz;
                a_1.concat(&Matrix::new(batch_sz, 1));
                continue;
            }
        }

        let q_1 = q[0].as_matrix_ref();

        let mut mat_ref = db.get_mat_ref();
        if let Some(x) = raw_data {
            mat_ref = MatrixRef {
                rows: db.num_rows(),
                cols: db.num_cols(),
                data: x,
            };

            // For debugging:
            // let slc = mat_ref.rows(start_row, batch_sz).data;
            // println!(
            //     "checksum on slice #{}: {} ",
            //     chunk_idx.unwrap(),
            //     checksum_u32(slc)
            // );
        }
        println!("db rows: {} cols: {}", db.num_rows(), db.num_cols());
        println!("getting rows start: {} num: {}", start_row, batch_sz);
        let a = matrix_mul_vec_packed(
            &mat_ref.rows(start_row, batch_sz),
            &q_1,
            db.info.squish_params.basis,
            db.info.squish_params.delta,
        );
        // a.print_checksum("a");
        // q_1.to_owned_matrix().print_checksum("q1");
        // db.data
        //     .rows(last, batch_sz)
        //     .to_owned_matrix()
        //     .print_checksum("db.data.rows");
        a_1.concat(&a);
        last += batch_sz
    }

    a_1.print_checksum("a1");

    a_1.print_dims("before taeaccas");
    a_1.transpose_expand_concat_cols_squish(params.p, params.delta() as usize, db.info.x, 10, 3);
    a_1.print_dims("after taeaccas");
    a_1.print_checksum("a1 (#2)");

    let mut msg = vec![matrix_mul_transposed_packed(
        &a_1.as_matrix_ref(),
        &a_2_transpose.as_matrix_ref(),
        10,
        3,
    )];
    msg[0].print_checksum("h1");

    for q in queries {
        for j in 0..(db.info.ne / db.info.x) as usize {
            let q_2 = &q[1 + j].as_matrix_ref();
            let a_2 = matrix_mul_vec_packed(&h_1.as_matrix_ref(), &q_2, 10, 3);
            a_1.print_dims("a_1 just before");
            q_2.to_owned_matrix().print_dims("q_2 just before");
            let h_2 = matrix_mul_vec_packed(&a_1.as_matrix_ref(), &q_2, 10, 3);

            a_2.print_checksum("a_2");
            h_2.print_checksum("h_2");

            msg.push(a_2);
            msg.push(h_2);
        }
    }
    println!("sending result msg.len(): {}", msg.len());

    msg
}

pub fn recover(
    i: usize,
    batch_index: usize,
    offline: &State,
    query: &State,
    answer: &State,
    shared: &State,
    client: &State,
    params: &Params,
    info: &DbInfo,
) -> u64 {
    info!("============== BEGIN RECOVERY");
    let h_2 = offline[0].clone();
    let mut h1 = answer[0].clone(); // deep copy whole matrix
    let secret1 = client[0].clone();

    h_2.print_checksum("H2");
    h1.print_checksum("h1");
    secret1.print_checksum("secret1");

    let ratio = params.p / 2;
    let mut val1 = 0u64;
    for j in 0..params.m {
        val1 += ratio * (query[0][j][0] as u64);
    }
    val1 %= 1 << params.logq;
    val1 = (1 << params.logq) - val1;

    info!("val1: {}", val1);

    let mut val2 = 0u64;
    for j in 0..params.l / info.x {
        val2 += ratio * (query[1][j][0] as u64);
    }
    val2 %= 1 << params.logq;
    val2 = (1 << params.logq) - val2;

    info!("val2: {}", val2);

    let a_2 = shared[1].clone();
    if (a_2.cols != params.n) || (h1.cols != params.n) {
        panic!("Should not happen!");
    }

    for j1 in 0..params.n {
        let mut val3 = 0u64;
        for j2 in 0..a_2.rows {
            val3 += ratio * (a_2[j2][j1] as u64);
        }
        val3 %= 1 << params.logq;
        val3 = (1 << params.logq) - val3;

        let v = val3 as u32;
        // info!("v: {}", v);
        for k in 0..h1.rows {
            h1.data[k * h1.cols + j1] += v;
        }
    }
    h1.print_checksum("h1 (#2)");

    let offset = (info.ne / info.x * 2) * batch_index; // for batching
    let mut vals = Vec::new();
    for i in 0..info.ne / info.x {
        let a2 = &answer[1 + 2 * i + offset].as_matrix_ref();
        let mut h2 = answer[2 + 2 * i + offset].clone();
        let secret2 = client[1 + i].clone();
        h2 += val2 as u32;

        for j in 0..info.x {
            let delta = params.delta() as usize;
            let mut state = a2
                .rows(j * params.n * delta, params.n * delta)
                .to_owned_matrix();
            state += val2 as u32;
            state.concat_ref(&h2.rows(j * delta, delta));

            state.print_checksum("state");

            let mut hint = h_2
                .rows(j * params.n * delta, params.n * delta)
                .to_owned_matrix();
            hint.concat_ref(&h1.rows(j * delta, delta));
            hint.print_checksum("hint");

            let interm = &hint * &secret2;
            state.print_checksum("state (#1.7)");
            state -= interm;
            state.print_checksum("state (#1.8)");
            println!(" (pre)  state.data[0] = {}", state.data[0]);
            state.apply(|x: u32| params.round(x as u64) as u32);
            println!("(post)  state.data[0] = {}", state.data[0]);
            state.print_checksum("state (#1.9)");
            state = state.contract(&params.get_contract_params());
            state.print_checksum("state (#2)");

            let mut noised = state.data[params.n] as u64 + val1;
            for l in 0..params.n {
                noised -= (secret1.data[l] * state.data[l]) as u64;
                noised = noised % (1 << params.logq);
            }
            println!("noised: {}", noised);
            vals.push(params.round(noised));
        }
    }

    return Db::reconstruct_elem(vals, i, info);
}

#[cfg(test)]
mod tests {
    use std::{ops::AddAssign, time::Instant};

    use rand::{distributions::Standard, thread_rng, Rng};

    use super::*;

    #[test]
    #[ignore]
    fn simple_end_to_end_test() {
        let num_entries = 1 << 24;
        let bits_per_entry = 1;
        let item_max = 1u64 << bits_per_entry;

        let mut rng = thread_rng();
        let index_to_query = rng.gen::<usize>() % num_entries;
        println!("index_to_query {}", index_to_query);
        let rng_iter = thread_rng().sample_iter(rand::distributions::Standard);
        let vals_iter = rng_iter.map(|x: u8| x % (item_max as u8)).take(num_entries);
        let corr_val = rng.gen::<u8>() % (item_max as u8);
        let vals_iter_fixed_point = FixedPointIter::new(vals_iter, index_to_query, corr_val);

        let params = pick_params(num_entries, bits_per_entry, SEC_PARAM, LOGQ);
        println!("params: {:?}", params);
        let mut db = Db::with_data(num_entries, bits_per_entry, &params, vals_iter_fixed_point);
        println!("info: {:?}", db.info);
        assert_eq!(db.get_elem(index_to_query) as u8, corr_val);

        let shared_state = init(&db.info, &params);
        let (server_state, hint) = setup(&mut db, &shared_state, &params);
        let (client_state, query) = query(index_to_query, &shared_state, &params, &db.info);
        let query_clone = query.clone();
        let start = Instant::now();
        let answer = answer(
            &db,
            &vec![query],
            &server_state,
            &shared_state,
            &params,
            None,
            None,
        );
        println!("Answer took: {} us", start.elapsed().as_micros());
        let result = recover(
            index_to_query,
            0,
            &hint,
            &query_clone,
            &answer,
            &shared_state,
            &client_state,
            &params,
            &db.info,
        );

        println!("result {}", result);

        assert_eq!(result as u8, corr_val);
    }

    #[test]
    #[ignore]
    fn batched_end_to_end_test() {
        let num_entries = 1 << 24;
        let bits_per_entry = 1;
        let item_max = 1u64 << bits_per_entry;

        let batch_sz = 14 * 65536 * 9;

        let mut index_to_query_1: usize = thread_rng().gen::<usize>() % batch_sz;
        let mut index_to_query_2: usize = (index_to_query_1 + batch_sz) % num_entries;
        if index_to_query_2 < index_to_query_1 {
            (index_to_query_1, index_to_query_2) = (index_to_query_2, index_to_query_1);
        }
        let indices_to_query = vec![index_to_query_1, index_to_query_2];
        println!("index_to_query_1 {}", index_to_query_1);
        println!("index_to_query_2 {}", index_to_query_2);
        let vals_iter = thread_rng()
            .sample_iter(Standard)
            .map(|x: u8| x % (item_max as u8))
            .take(num_entries);
        let corr_val = thread_rng().gen::<u8>() % (item_max as u8);
        let vals_iter_fixed_point = FixedPointIter::new(vals_iter, index_to_query_1, corr_val);
        let vals_iter_fixed_point =
            FixedPointIter::new(vals_iter_fixed_point, index_to_query_2, corr_val);

        let params = pick_params(num_entries, bits_per_entry, SEC_PARAM, LOGQ);
        println!("params: {:?}", params);
        let mut db = Db::with_data(num_entries, bits_per_entry, &params, vals_iter_fixed_point);

        let shared_state = init(&db.info, &params);
        let (server_state, hint) = setup(&mut db, &shared_state, &params);
        let (client_state_1, query_val_1) =
            query(index_to_query_1, &shared_state, &params, &db.info);
        let (client_state_2, query_val_2) =
            query(index_to_query_2, &shared_state, &params, &db.info);

        println!("info: {:?}", db.info);

        assert_eq!(db.get_elem(index_to_query_1) as u8, corr_val);
        assert_eq!(db.get_elem(index_to_query_2) as u8, corr_val);

        let queries = vec![query_val_1, query_val_2];
        let client_states = vec![client_state_1, client_state_2];

        let start = Instant::now();
        let full_response = answer(
            &db,
            &queries,
            &server_state,
            &shared_state,
            &params,
            Some(&db.data.data),
            None,
        );

        println!("Answer took: {} us", start.elapsed().as_micros());

        for chunk_idx in 0..queries.len() {
            let result = recover(
                indices_to_query[chunk_idx],
                chunk_idx,
                &hint,
                &queries[chunk_idx],
                &full_response,
                &shared_state,
                &client_states[chunk_idx],
                &params,
                &db.info,
            );

            println!("got {}, expected {}", result, corr_val);

            assert_eq!(result as u8, corr_val);
        }
    }

    #[test]
    #[ignore]
    fn chunked_end_to_end_test() {
        let num_entries = 1 << 24;
        let bits_per_entry = 1;
        let item_max = 1u64 << bits_per_entry;

        let batch_sz = 14 * 65536 * 9;

        let mut index_to_query_1: usize = thread_rng().gen::<usize>() % batch_sz;
        let mut index_to_query_2: usize = (index_to_query_1 + batch_sz) % num_entries;
        if index_to_query_2 < index_to_query_1 {
            (index_to_query_1, index_to_query_2) = (index_to_query_2, index_to_query_1);
        }
        let indices_to_query = vec![index_to_query_1, index_to_query_2];
        println!("index_to_query_1 {}", index_to_query_1);
        println!("index_to_query_2 {}", index_to_query_2);
        let vals_iter = thread_rng()
            .sample_iter(Standard)
            .map(|x: u8| x % (item_max as u8))
            .take(num_entries);
        let corr_val = thread_rng().gen::<u8>() % (item_max as u8);
        let vals_iter_fixed_point = FixedPointIter::new(vals_iter, index_to_query_1, corr_val);
        let vals_iter_fixed_point =
            FixedPointIter::new(vals_iter_fixed_point, index_to_query_2, corr_val);

        let params = pick_params(num_entries, bits_per_entry, SEC_PARAM, LOGQ);
        println!("params: {:?}", params);
        let mut db = Db::with_data(num_entries, bits_per_entry, &params, vals_iter_fixed_point);

        let shared_state = init(&db.info, &params);
        let (server_state, hint) = setup(&mut db, &shared_state, &params);
        let (client_state_1, query_val_1) =
            query(index_to_query_1, &shared_state, &params, &db.info);
        let (client_state_2, query_val_2) =
            query(index_to_query_2, &shared_state, &params, &db.info);

        println!("info: {:?}", db.info);

        assert_eq!(db.get_elem(index_to_query_1) as u8, corr_val);
        assert_eq!(db.get_elem(index_to_query_2) as u8, corr_val);

        let queries = vec![query_val_1, query_val_2];
        let client_states = vec![client_state_1, client_state_2];

        let mut full_response = Vec::new();
        let num_chunks = 2;

        // last batch may be *bigger*, not smaller...
        let batch_sz = db.num_rows() / num_chunks;
        let (most, last) = db
            .data
            .data
            .as_slice()
            .split_at(batch_sz * db.num_cols() * (num_chunks - 1));
        let data_slcs = most
            .chunks(batch_sz * db.num_cols())
            .chain(std::iter::once(last));

        let start = Instant::now();
        for (chunk_idx, data_slc) in data_slcs.enumerate() {
            println!("{}: data_slc.len() = {}", chunk_idx, data_slc.len());
            let response = answer(
                &db,
                &queries,
                &server_state,
                &shared_state,
                &params,
                Some(data_slc),
                Some(chunk_idx),
            );
            assert_eq!(response.len(), 1 + 2 * num_chunks);

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
            println!("============");
        }
        assert_eq!(full_response.len(), 5);
        println!("Answers took: {} us", start.elapsed().as_micros());

        for chunk_idx in 0..num_chunks {
            let result = recover(
                indices_to_query[chunk_idx],
                chunk_idx,
                &hint,
                &queries[chunk_idx],
                &full_response,
                &shared_state,
                &client_states[chunk_idx],
                &params,
                &db.info,
            );

            println!("got {}, expected {}", result, corr_val);

            assert_eq!(result as u8, corr_val);
        }
    }
}

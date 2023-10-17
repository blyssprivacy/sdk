use std::time::Instant;

use spiral_rs::arith::*;
use spiral_rs::client::PublicParameters;
use spiral_rs::client::Query;
use spiral_rs::params::*;
use spiral_rs::poly::*;

use rayon::prelude::*;
use spiral_rs::util::write_arbitrary_bits;

use crate::compute::dot_product::*;
use crate::compute::fold::*;
use crate::compute::pack::*;
use crate::compute::query_expansion::*;
use crate::db::aligned_memory::*;
use crate::db::sparse_db::SparseDb;

fn prepare_query<'a>(
    params: &'a Params,
    public_params: &'a PublicParameters,
    query: &'a Query,
    indices: Option<&[usize]>,
) -> (AlignedMemory64, Vec<PolyMatrixNTT<'a>>) {
    let mut v_reg_reoriented;
    let v_folding;
    if params.expand_queries {
        (v_reg_reoriented, v_folding) = expand_query(params, public_params, query, indices);
    } else {
        v_reg_reoriented = AlignedMemory64::new(query.v_buf.as_ref().unwrap().len());
        v_reg_reoriented
            .as_mut_slice()
            .copy_from_slice(query.v_buf.as_ref().unwrap());

        v_folding = query
            .v_ct
            .as_ref()
            .unwrap()
            .iter()
            .map(|x| x.ntt())
            .collect();
    }

    (v_reg_reoriented, v_folding)
}

pub fn process_query(
    params: &Params,
    public_params: &PublicParameters,
    query: &Query,
    db: &SparseDb,
) -> Vec<u8> {
    let v_packing = public_params.v_packing.as_ref();
    let (v_reg_reoriented, v_folding) =
        prepare_query(params, public_params, query, Some(&db.get_active_ids()));
    let v_folding_neg = get_v_folding_neg(params, &v_folding);

    let trials = params.n * params.n;
    let inst_trials = params.instances * trials;
    let num_per = 1 << params.db_dim_2;
    let n_results = inst_trials * num_per;
    let mut intermediate = vec![PolyMatrixNTT::zero(params, 2, 1); n_results];

    let stamp = Instant::now();
    db.prefill();
    multiply_reg_by_sparsedb(&mut intermediate, db, v_reg_reoriented.as_slice(), params);
    let mul_time = stamp.elapsed().as_micros();
    let mb_per_sec = db.current_size() as f64 / mul_time as f64;
    println!(
        "mul:  {} us ({:.0} MB @ {:.0} MB/s)",
        mul_time,
        db.current_size() as f64 / 1e6,
        mb_per_sec
    );

    let misses = db.pop_cache_misses();
    let miss_rate = misses as f64 / db.current_count() as f64;
    println!("Cache misses: {} ({:.2}%)", misses, miss_rate * 100.0);

    let stamp = Instant::now();
    let v_cts: Vec<_> = intermediate
        .par_chunks(num_per)
        .map(|chunk| {
            let mut intermediate_raw: Vec<PolyMatrixRaw> =
                chunk.par_iter().map(|item| item.raw()).collect();
            fold_ciphertexts(params, &mut intermediate_raw, &v_folding, &v_folding_neg);
            intermediate_raw[0].clone()
        })
        .collect();
    println!("fold: {} us", stamp.elapsed().as_micros());

    let stamp = Instant::now();
    let v_packed_ct = v_cts
        .par_chunks_exact(trials)
        .map(|chunk: &[PolyMatrixRaw]| {
            let packed_ct = pack(params, chunk, &v_packing);
            packed_ct.raw()
        })
        .collect();
    println!("pack: {} us", stamp.elapsed().as_micros());

    encode(params, &v_packed_ct)
}

pub fn encode(params: &Params, v_packed_ct: &Vec<PolyMatrixRaw>) -> Vec<u8> {
    let q1 = 4 * params.pt_modulus;
    let q1_bits = log2_ceil(q1) as usize;
    let q2 = Q2_VALUES[params.q2_bits as usize];
    let q2_bits = params.q2_bits as usize;

    let num_bits = params.instances
        * ((q2_bits * params.n * params.poly_len)
            + (q1_bits * params.n * params.n * params.poly_len));
    let round_to = 64;
    let num_bytes_rounded_up = ((num_bits + round_to - 1) / round_to) * round_to / 8;

    let mut result = vec![0u8; num_bytes_rounded_up];
    let mut bit_offs = 0;
    for instance in 0..params.instances {
        let packed_ct = &v_packed_ct[instance];

        let mut first_row = packed_ct.submatrix(0, 0, 1, packed_ct.cols);
        let mut rest_rows = packed_ct.submatrix(1, 0, packed_ct.rows - 1, packed_ct.cols);
        first_row.apply_func(|x| rescale(x, params.modulus, q2));
        rest_rows.apply_func(|x| rescale(x, params.modulus, q1));

        let data = result.as_mut_slice();
        for i in 0..params.n * params.poly_len {
            write_arbitrary_bits(data, first_row.data[i], bit_offs, q2_bits);
            bit_offs += q2_bits;
        }
        for i in 0..params.n * params.n * params.poly_len {
            write_arbitrary_bits(data, rest_rows.data[i], bit_offs, q1_bits);
            bit_offs += q1_bits;
        }
    }
    result
}

#[cfg(test)]
mod test {
    use super::*;
    use rand::Rng;
    use spiral_rs::client::*;
    use spiral_rs::util;
    use std::fs::File;
    use std::io::Write;
    use std::os::fd::FromRawFd;
    use std::time::Instant;

    use crate::db::loading::*;

    fn get_params() -> Params {
        let cfg = r#"
        {
            "n": 2,
            "nu_1": 9,
            "nu_2": 5,
            "p": 256,
            "q2_bits": 22,
            "t_gsw": 7,
            "t_conv": 3,
            "t_exp_left": 5,
            "t_exp_right": 5,
            "instances": 4,
            "db_item_size": 32768
        }
        "#;
        util::params_from_json(&cfg)
    }

    fn setup_full_protocol_test(
        params: &Params,
    ) -> (Client, PublicParameters, Query, SparseDb, PolyMatrixRaw) {
        let mut seeded_rng = util::get_seeded_rng();

        let target_idx = seeded_rng.gen::<usize>() % (1 << (params.db_dim_1 + params.db_dim_2));
        println!("targeting index {}", target_idx);

        let mut client = Client::init(&params);

        let public_params = client.generate_keys();
        let query = client.generate_query(target_idx);

        let stamp = Instant::now();
        let dummy_items = params.num_items();
        let (corr_db_item, db) =
            generate_fake_sparse_db_and_get_item(params, target_idx, dummy_items);
        println!(
            "generated {} items in {} ms",
            dummy_items,
            stamp.elapsed().as_millis()
        );

        return (client, public_params, query, db, corr_db_item);
    }

    fn full_protocol_is_correct_for_params(params: &Params) {
        let (client, public_params, query, db, corr_db_item) = setup_full_protocol_test(params);

        let mut response: Vec<u8> = Vec::new();

        let stamp = Instant::now();
        response = process_query(params, &public_params, &query, &db);
        println!("processing took {} us", stamp.elapsed().as_micros());

        println!(
            "pub params: {} bytes ({} actual) \nresponse: {} bytes",
            params.setup_bytes(),
            public_params.serialize().len(),
            response.len()
        );

        // decode and verify
        let result = client.decode_response(response.as_slice());

        let p_bits = log2_ceil(params.pt_modulus) as usize;
        let corr_result = corr_db_item.to_vec(p_bits, params.modp_words_per_chunk());

        assert_eq!(result.len(), corr_result.len());

        for z in 0..corr_result.len() {
            assert_eq!(result[z], corr_result[z], "at {:?}", z);
        }
    }

    #[test]
    fn full_protocol_is_correct() {
        full_protocol_is_correct_for_params(&get_params());
    }

    #[test]
    fn bench_mul() {
        let params = get_params();
        let (_, public_params, query, db, _) = setup_full_protocol_test(&params);
        let (v_reg_reoriented, _) =
            prepare_query(&params, &public_params, &query, Some(&db.get_active_ids()));

        let trials = params.n * params.n;
        let inst_trials = params.instances * trials;
        let num_per = 1 << params.db_dim_2;
        let n_results = inst_trials * num_per;
        let mut intermediate = vec![PolyMatrixNTT::zero(&params, 2, 1); n_results];

        let perf_fd: Option<File> = std::env::var("PERF_CTL_FD")
            .ok()
            .and_then(|fd| fd.parse().ok())
            .map(|raw_fd| unsafe { std::fs::File::from_raw_fd(raw_fd) });
        if let Some(mut perf_fd) = perf_fd.as_ref() {
            writeln!(perf_fd, "enable").unwrap();
            println!("Started perf collection.");
        }

        const N_RUNS: usize = 1;
        let timings: Vec<_> = (0..N_RUNS)
            .map(|_| {
                let stamp = Instant::now();
                db.prefill();
                multiply_reg_by_sparsedb(
                    &mut intermediate,
                    &db,
                    v_reg_reoriented.as_slice(),
                    &params,
                );
                core::hint::black_box(&intermediate);
                stamp.elapsed().as_micros()
            })
            .collect();

        if let Some(mut perf_fd) = perf_fd.as_ref() {
            writeln!(perf_fd, "disable").unwrap();
            println!("Stopped perf collection.");
        }

        let dbsize = db.current_size() as f64;
        let slowest = *timings.iter().max().unwrap();
        let avg = timings.iter().sum::<u128>() / N_RUNS as u128;

        println!(
            "slowest: {} us ({} MB/s)",
            slowest,
            (dbsize / slowest as f64) as u64
        );
        println!("avg: {} us, ({} MB/s)", avg, (dbsize / avg as f64) as u64);
    }
}

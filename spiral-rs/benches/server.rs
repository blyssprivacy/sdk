use criterion::measurement::WallTime;
use criterion::BenchmarkGroup;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pprof::criterion::{Output, PProfProfiler};

use rand::Rng;
use spiral_rs::aligned_memory::AlignedMemory64;
use spiral_rs::client::*;
use spiral_rs::params::*;
use spiral_rs::poly::*;
use spiral_rs::server::*;
use spiral_rs::util::*;
use std::time::Duration;

pub fn generate_random_incorrect_db(params: &Params) -> AlignedMemory64 {
    let instances = params.instances;
    let trials = params.n * params.n;
    let dim0 = 1 << params.db_dim_1;
    let num_per = 1 << params.db_dim_2;
    let num_items = dim0 * num_per;
    let db_size_words = instances * trials * num_items * params.poly_len;
    let mut out = AlignedMemory64::new(db_size_words);
    let out_mut_slice = out.as_mut_slice();
    let mut rng = get_seeded_rng();
    for i in 0..out_mut_slice.len() {
        out_mut_slice[i] = rng.gen();
    }
    out
}

fn test_full_processing(group: &mut BenchmarkGroup<WallTime>) {
    // let names = ["server_processing_20_256", "server_processing_16_100000"];
    // let cfgs = [CFG_20_256, CFG_16_100000];
    let names = ["server_processing_16_100000"];
    let cfgs = [CFG_16_100000];

    for i in 0..names.len() {
        let name = names[i];
        let cfg = cfgs[i];
        let params = params_from_json(&cfg.replace("'", "\""));
        let mut seeded_rng = get_seeded_rng();

        let target_idx = seeded_rng.gen::<usize>() % (params.db_dim_1 + params.db_dim_2);

        let mut client = Client::init(&params);
        let public_params = client.generate_keys();
        let query = client.generate_query(target_idx);

        println!("Generating database...");
        let db = generate_random_incorrect_db(&params);
        println!("Done generating database.");

        group.bench_function(name, |b| {
            b.iter(|| {
                black_box(process_query(
                    black_box(&params),
                    black_box(&public_params),
                    black_box(&query),
                    black_box(db.as_slice()),
                ));
            });
        });
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("server");
    group
        .sample_size(10)
        .measurement_time(Duration::from_secs(30));

    let params = get_expansion_testing_params();
    let v_neg1 = params.get_v_neg1();
    let mut rng = get_chacha_rng();
    let mut rng_pub = get_chacha_rng();
    let mut client = Client::init(&params);
    let public_params = client.generate_keys();

    let mut v = Vec::new();
    for _ in 0..params.poly_len {
        v.push(PolyMatrixNTT::zero(&params, 2, 1));
    }
    let scale_k = params.modulus / params.pt_modulus;
    let mut sigma = PolyMatrixRaw::zero(&params, 1, 1);
    sigma.data[7] = scale_k;
    v[0] = client.encrypt_matrix_reg(&sigma.ntt(), &mut rng, &mut rng_pub);

    let v_w_left = public_params.v_expansion_left.unwrap();
    let v_w_right = public_params.v_expansion_right.unwrap();

    // note: the benchmark on AVX2 is 545ms for the c++ impl
    group.bench_function("coefficient_expansion", |b| {
        b.iter(|| {
            coefficient_expansion(
                black_box(&mut v),
                black_box(params.g()),
                black_box(params.stop_round()),
                black_box(&params),
                black_box(&v_w_left),
                black_box(&v_w_right),
                black_box(&v_neg1),
                black_box(params.t_gsw * params.db_dim_2),
            )
        });
    });

    let mut seeded_rng = get_seeded_rng();
    let trials = params.n * params.n;
    let dim0 = 1 << params.db_dim_1;
    let num_per = 1 << params.db_dim_2;
    let num_items = dim0 * num_per;
    let db_size_words = trials * num_items * params.poly_len;
    let mut db = vec![0u64; db_size_words];
    for i in 0..db_size_words {
        db[i] = seeded_rng.gen();
    }
    println!("{} polys in db", trials * num_items);

    let v_reg_sz = dim0 * 2 * params.poly_len;
    let mut v_reg_reoriented = AlignedMemory64::new(v_reg_sz);
    for i in 0..v_reg_sz {
        v_reg_reoriented[i] = seeded_rng.gen();
    }
    let mut out = Vec::with_capacity(num_per);
    for _ in 0..dim0 {
        out.push(PolyMatrixNTT::zero(&params, 2, 1));
    }

    // note: the benchmark on AVX2 is 45ms for the c++ impl
    group.bench_function("first_dimension_processing", |b| {
        b.iter(|| {
            multiply_reg_by_database(
                black_box(&mut out),
                black_box(db.as_slice()),
                black_box(v_reg_reoriented.as_slice()),
                black_box(&params),
                black_box(dim0),
                black_box(num_per),
            )
        });
    });

    // full server processing benchmark

    test_full_processing(&mut group);

    group.finish();
}

// criterion_group!(benches, criterion_benchmark);
criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets = criterion_benchmark
}
criterion_main!(benches);

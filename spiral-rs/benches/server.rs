use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pprof::criterion::{Output, PProfProfiler};

use spiral_rs::client::*;
use spiral_rs::poly::*;
use spiral_rs::server::*;
use spiral_rs::util::*;
use std::time::Duration;

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("sample-size");
    group
        .sample_size(10)
        .measurement_time(Duration::from_secs(30));

    let params = get_expansion_testing_params();
    let v_neg1 = params.get_v_neg1();
    let mut seeded_rng = get_seeded_rng();
    let mut client = Client::init(&params, &mut seeded_rng);
    let public_params = client.generate_keys();

    let mut v = Vec::new();
    for _ in 0..params.poly_len {
        v.push(PolyMatrixNTT::zero(&params, 2, 1));
    }
    let scale_k = params.modulus / params.pt_modulus;
    let mut sigma = PolyMatrixRaw::zero(&params, 1, 1);
    sigma.data[7] = scale_k;
    v[0] = client.encrypt_matrix_reg(&sigma.ntt());

    let v_w_left = public_params.v_expansion_left.unwrap();
    let v_w_right = public_params.v_expansion_right.unwrap();

    // note: the benchmark on AVX2 is 545ms for the c++ impl
    group.bench_function("coeff exp", |b| {
        b.iter(|| {
            coefficient_expansion(
                black_box(&mut v),
                black_box(client.g),
                black_box(client.stop_round),
                black_box(&params),
                black_box(&v_w_left),
                black_box(&v_w_right),
                black_box(&v_neg1),
                black_box(params.t_gsw * params.db_dim_2),
            )
        });
    });
    group.finish();
}

// criterion_group!(benches, criterion_benchmark);
criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets = criterion_benchmark
}
criterion_main!(benches);

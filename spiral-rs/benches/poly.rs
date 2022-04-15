use criterion::{black_box, criterion_group, criterion_main, Criterion};
use spiral_rs::poly::*;
use spiral_rs::util::*;

fn criterion_benchmark(c: &mut Criterion) {
    let params = get_test_params();
    let mut m1 = PolyMatrixRaw::random(&params, 10, 10);
    let mut m2 = PolyMatrixNTT::random(&params, 10, 10);
    let m3 = PolyMatrixNTT::random(&params, 10, 10);
    let mut m4 = PolyMatrixNTT::random(&params, 10, 10);

    // c.bench_function("nttf_noreduce 2048", |b| {
    //     b.iter(|| to_ntt_no_reduce(black_box(&mut m2), black_box(&m1)))
    // });

    c.bench_function("multiply", |b| {
        b.iter(|| multiply(black_box(&mut m4), black_box(&m2), black_box(&m3)))
    });

    c.bench_function("nttf_full 2048", |b| {
        b.iter(|| to_ntt(black_box(&mut m2), black_box(&m1)))
    });

    c.bench_function("ntti_full 2048", |b| {
        b.iter(|| from_ntt(black_box(&mut m1), black_box(&m2)))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

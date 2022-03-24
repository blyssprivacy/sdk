use spiral_rs::poly::*;
use spiral_rs::util::*;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn criterion_benchmark(c: &mut Criterion) {
    let params = get_test_params();
    let m1 = PolyMatrixNTT::random(&params, 2, 1);
    let m2 = PolyMatrixNTT::random(&params, 3, 2);
    let mut m3 = PolyMatrixNTT::zero(&params, 2, 2);
    c.bench_function("nttf 2048", |b| b.iter(|| multiply(black_box(&mut m3), black_box(&m1), black_box(&m2))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

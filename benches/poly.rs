use spiral_rs::poly::*;
use spiral_rs::params::*;
use spiral_rs::util::*;
use rand::Rng;
use rand::distributions::Standard;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn criterion_benchmark(c: &mut Criterion) {
    let params = Params::init(2048, &vec![268369921u64, 249561089u64]);
    let mut rng = rand::thread_rng();
    let mut iter = rng.sample_iter(&Standard);
    let m1 = PolyMatrixNTT::random(&params, 2, 1, &mut iter);
    let m2 = PolyMatrixNTT::random(&params, 3, 2, &mut iter);
    let mut m3 = PolyMatrixNTT::zero(&params, 2, 2);
    c.bench_function("nttf 2048", |b| b.iter(|| multiply(black_box(&mut m3), black_box(&m1), black_box(&m2))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

use spiral_rs::ntt::*;
use spiral_rs::params::*;
use spiral_rs::util::*;
use rand::Rng;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn criterion_benchmark(c: &mut Criterion) {
    let params = Params::init(2048, &vec![268369921u64, 249561089u64]);
    let mut v1 = vec![0; params.crt_count * params.poly_len];
    let mut rng = rand::thread_rng();
    for i in 0..params.crt_count {
        for j in 0..params.poly_len {
            let idx = calc_index(&[i, j], &[params.crt_count, params.poly_len]);
            let val: u64 = rng.gen();
            v1[idx] = val % params.moduli[i];
        }
    }
    c.bench_function("nttf 2048", |b| b.iter(|| ntt_forward(black_box(&params), black_box(&mut v1))));
    c.bench_function("ntti 2048", |b| b.iter(|| ntt_inverse(black_box(&params), black_box(&mut v1))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::Rng;
use spiral_rs::aligned_memory::*;
use spiral_rs::ntt::*;
use spiral_rs::util::*;

fn criterion_benchmark(c: &mut Criterion) {
    let params = get_test_params();
    let mut v1 = AlignedMemory64::new(params.crt_count * params.poly_len);
    let mut rng = rand::thread_rng();
    for i in 0..params.crt_count {
        for j in 0..params.poly_len {
            let idx = calc_index(&[i, j], &[params.crt_count, params.poly_len]);
            let val: u64 = rng.gen();
            v1[idx] = val % params.moduli[i];
        }
    }
    c.bench_function("nttf 2048", |b| {
        b.iter(|| ntt_forward(black_box(&params), black_box(v1.as_mut_slice())))
    });
    c.bench_function("ntti 2048", |b| {
        b.iter(|| ntt_inverse(black_box(&params), black_box(v1.as_mut_slice())))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

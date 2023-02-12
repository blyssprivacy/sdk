use std::future::Future;

use aes::cipher::{KeyIvInit, StreamCipher};

use super::Matrix;

type Aes128Ctr64BE = ctr::Ctr64BE<aes::Aes128>;

const DERIVE_CHUNK_SIZE: usize = 65536;

pub fn derive_with_aes(key: [u8; 16], out: &mut [u8]) {
    for (i, chunk) in out.chunks_mut(DERIVE_CHUNK_SIZE).enumerate() {
        derive_with_aes_at(key, i as u32, chunk);
    }
}

pub fn derive_with_aes_at(key: [u8; 16], i: u32, out: &mut [u8]) {
    let mut iv = [0u8; 16];
    (&mut iv[0..8]).copy_from_slice(&(i as u64).to_be_bytes());
    let mut cipher = Aes128Ctr64BE::new(&key.into(), &iv.into());
    cipher.apply_keystream(out);
}

/// Computes the multiplication of the matrix derived using derive(derive_idx, _, _)
/// by matrix `b`, producing a matrix.
///
/// The derived matrix is taken to be (m x n) and the input matrix is (n x ?).
pub async fn matrix_mul_derive_fn<T, Fut>(
    m: usize,
    n: usize,
    b: &Matrix,
    derive: fn(u32, u32, &mut [u8]) -> Fut,
    derive_idx: u32,
) -> Matrix
where
    Fut: Future<Output = T>,
    T: Sized,
{
    assert_eq!(b.rows, n);
    assert_eq!((DERIVE_CHUNK_SIZE / 4) % n, 0);

    let row_chunks = (DERIVE_CHUNK_SIZE / 4) / n;
    assert!(m >= row_chunks);

    let mut out = Matrix::new(m, b.cols);
    for m_i in (0..m).step_by(row_chunks) {
        let mut rows_in_cur_chunk = row_chunks.min(m - m_i);

        let mut partial_a = Matrix::new(rows_in_cur_chunk, n);
        let partial_bytes = unsafe { partial_a.raw_bytes_mut() };
        // assert!(partial_bytes.len() == DERIVE_CHUNK_SIZE || ... );
        derive(derive_idx, (m_i / row_chunks) as u32, partial_bytes).await;

        let partial_result = &partial_a * b;
        (&mut out.data[m_i * out.cols..(m_i + rows_in_cur_chunk) * out.cols])
            .copy_from_slice(&partial_result.data)
    }

    out
}

#[cfg(test)]
mod tests {
    use std::future;

    use crate::util::SEEDS_SHORT;

    use super::*;

    #[test]
    fn derive_with_aes_is_correct() {
        let mut data = vec![0u8; 265 * 65536];
        derive_with_aes(SEEDS_SHORT[0], &mut data);
        println!("{:?}", &data[0..32]);
        println!("{:?}", &data[258 * 65536..258 * 65536 + 32]);
        assert_eq!(data[0], 247);
        assert_eq!(data[16], 196);
        assert_eq!(data[258 * 65536], 63);

        let mut data = vec![0u8; 265 * 65536];
        derive_with_aes(SEEDS_SHORT[1], &mut data);
        println!("{:?}", &data[0..32]);
        println!("{:?}", &data[258 * 65536..258 * 65536 + 32]);
        assert_eq!(data[0], 132);
        assert_eq!(data[258 * 65536], 254);
    }

    #[tokio::test]
    async fn matrix_mul_derive_fn_is_correct() {
        let m = 256;
        let n = 1024;
        let c = 8;
        let b = Matrix::random(n, c);

        let a_gold = Matrix::derive_from_seed(m, n, SEEDS_SHORT[0]);

        let derive_fn = |s: u32, i: u32, out: &mut [u8]| {
            future::ready(derive_with_aes_at(SEEDS_SHORT[(s as usize) - 1], i, out))
        };

        let result = matrix_mul_derive_fn(m, n, &b, derive_fn, 1).await;

        let gold = &a_gold * &b;

        assert_eq!(result, gold);
    }
}
// #[cfg(feature = "web-sys")]
// pub fn derive_with_aes(seed: Seed, out: &mut [u8]) {
//     let full_key: &[u8; 32] = &seed.into(); //[0x42; 16];
//     let mut key = [0u8; 16];
//     key.copy_from_slice(&full_key[..16]);

//     let iv = [0x24; 16];

//     // let mut counter = js_sys::Uint8Array::new(&16.into());
//     // counter.copy_from(&iv);

//     // let mut params = js_sys::Object::new();
//     // js_sys::Reflect::set(&params, &"name".into(), &"AES-CTR".into());
//     // js_sys::Reflect::set(&params, &"counter".into(), &"AES-CTR".into());
//     // js_sys::Reflect::set(&params, &"length".into(), &64.into());
//     // {
//     //     name: "AES-CTR",
//     //     counter,
//     //     length: 64,
//     //   }
//     // let mut cipher = Aes128Ctr64BE::new(&key.into(), &iv.into());

//     let window = web_sys::window().unwrap();
//     let func = window.get("deriveSeed").unwrap().;

//     for chunk in out.chunks_mut(65536) {
//         SubtleCrypto::encrypt_with_object_and_u8_array(, key, data);
//     }
// }

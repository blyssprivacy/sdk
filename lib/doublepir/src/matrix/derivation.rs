use super::Seed;
use aes::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};

type Aes128Ctr64LE = ctr::Ctr64LE<aes::Aes128>;

pub fn derive_with_aes(key: [u8; 16], out: &mut [u8]) {
    for (i, chunk) in out.chunks_mut(65536).enumerate() {
        let mut iv = [0u8; 16];
        (&mut iv[0..8]).copy_from_slice(&(i as u64).to_be_bytes());
        let mut cipher = Aes128Ctr64LE::new(&key.into(), &iv.into());
        cipher.apply_keystream(chunk);
    }
}
#[cfg(test)]
mod tests {
    use rand::{thread_rng, Rng};

    use crate::util::SEEDS_SHORT;

    use super::*;

    #[test]
    fn derive_with_aes_is_correct() {
        let mut data = vec![0u8; 265 * 65536];
        derive_with_aes(SEEDS_SHORT[0], &mut data);
        println!("{:?}", &data[0..32]);
        println!("{:?}", &data[258 * 65536..258 * 65536 + 32]);
        assert_eq!(data[0], 247);
        assert_eq!(data[258 * 65536], 63);
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
//     // let mut cipher = Aes128Ctr64LE::new(&key.into(), &iv.into());

//     let window = web_sys::window().unwrap();
//     let func = window.get("deriveSeed").unwrap().;

//     for chunk in out.chunks_mut(65536) {
//         SubtleCrypto::encrypt_with_object_and_u8_array(, key, data);
//     }
// }

// Not the nicest way to import WASM, but allows Webpack
// to bundle everything into a single JS file
import initWasm, {
  ApiClient,
  DoublePIRApiClient,
  decode_response,
  extract_result,
  generate_keys,
  generate_query,
  get_row,
  initialize_client
} from '../../dist/lib/lib';
import wasmData from '../../dist/lib/lib_bg.wasm';

(async function () {
  (window as any).wasmInit = await initWasm(wasmData);
})();

const key1 = new Uint8Array([
  0x9c, 0x22, 0x77, 0x85, 0x45, 0xac, 0x22, 0x97, 0x41, 0x90, 0x8e, 0x65, 0x2d,
  0x33, 0x3a, 0x0f
]); // first 16 bytes of SHA256(blyss1)])

const key2 = new Uint8Array([
  0x5f, 0xff, 0xc4, 0x82, 0xc7, 0x2a, 0x85, 0x4a, 0x10, 0x35, 0x9e, 0x9f, 0xa2,
  0xf5, 0xe0, 0x7f
]); // first 16 bytes of SHA256(blyss2)])

async function aes_derive_fast(
  keyIdx: number,
  _ctr: bigint,
  dst: number,
  len: number
) {
  const wasmInit = (window as any).wasmInit;
  console.log('here.....');
  console.log(wasmInit.memory);
  console.log(dst, len);

  const rawKey = keyIdx == 1 ? key1 : key2;
  const key = await crypto.subtle.importKey(
    'raw',
    rawKey.buffer,
    'AES-CTR',
    false,
    ['encrypt', 'decrypt']
  );

  const chunkSize = 65536;
  const data = new Uint8Array(chunkSize);

  const numChunks = Math.floor((len + chunkSize - 1) / chunkSize);
  for (let i = 0; i < numChunks; i++) {
    // console.log(i);
    const counter = new Uint8Array(16);
    const dv = new DataView(counter.buffer);
    dv.setBigUint64(0, BigInt(i), false);
    const val = await window.crypto.subtle.encrypt(
      {
        name: 'AES-CTR',
        counter,
        length: 64
      },
      key,
      data
    );
    const start = i * chunkSize;
    let outLen = chunkSize;
    if (i === numChunks - 1) {
      console.log('last one');
      outLen = len - start;
    }
    const outRound = new Uint8Array(
      wasmInit.memory.buffer,
      dst + start,
      outLen
    );
    outRound.set(new Uint8Array(val.slice(0, outLen)));
  }

  console.log(`idx: ${keyIdx}`);
  console.log(new Uint8Array(wasmInit.memory.buffer, dst, 128));
  console.log(new Uint8Array(wasmInit.memory.buffer, dst + 258 * 65536, 128));
  // console.log('yay');
}

(window as any).aes_derive_fast_1 = async function (
  ctr: bigint,
  dst: number,
  len: number
) {
  return await aes_derive_fast(1, ctr, dst, len);
};

(window as any).aes_derive_fast_2 = async function (
  ctr: bigint,
  dst: number,
  len: number
) {
  return await aes_derive_fast(2, ctr, dst, len);
};

export {
  ApiClient,
  DoublePIRApiClient,
  decode_response,
  extract_result,
  generate_keys,
  generate_query,
  get_row,
  initialize_client
};

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

if (typeof crypto === 'undefined')
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  (globalThis as any).crypto = (require('node:crypto') as any).webcrypto;

(async function () {
  let imported: any = initWasm;
  if (typeof imported !== 'function') imported = function () {};
  const result = await imported(wasmData);
  if (typeof window !== 'undefined') {
    (window as any).wasmInit = result;
  }
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
  chunkIdx: number,
  dst: number,
  len: number
) {
  const wasmInit = (window as any).wasmInit;
  const rawKey = keyIdx == 1 ? key1 : key2;
  const key = await crypto.subtle.importKey(
    'raw',
    rawKey.buffer,
    'AES-CTR',
    false,
    ['encrypt', 'decrypt']
  );

  const data = new Uint8Array(len);

  const counter = new Uint8Array(16);
  const dv = new DataView(counter.buffer);
  dv.setBigUint64(0, BigInt(chunkIdx), false);
  const val = await window.crypto.subtle.encrypt(
    {
      name: 'AES-CTR',
      counter,
      length: 64 // bits for counter
    },
    key,
    data
  );
  const outRound = new Uint8Array(wasmInit.memory.buffer, dst, len);
  outRound.set(new Uint8Array(val.slice(0, len)));
}

let windowObj: any;
if (typeof window !== 'undefined') {
  // none
  windowObj = window;
} else {
  (global.window as any) = {};
  windowObj = global.window;
}

windowObj.aes_derive_fast_1 = async function (
  ctr: number,
  dst: number,
  len: number
) {
  return await aes_derive_fast(1, ctr, dst, len);
};

windowObj.aes_derive_fast_2 = async function (
  ctr: number,
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

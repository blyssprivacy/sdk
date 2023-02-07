// Not the nicest way to import WASM, but allows Webpack
// to bundle everything into a single JS file
import initWasm, {
  ApiClient,
  decode_response,
  extract_result,
  generate_keys,
  generate_query,
  get_row,
  initialize_client
} from '../../dist/lib/lib';
import wasmData from '../../dist/lib/lib_bg.wasm';
initWasm(wasmData);

export {
  ApiClient,
  decode_response,
  extract_result,
  generate_keys,
  generate_query,
  get_row,
  initialize_client
};

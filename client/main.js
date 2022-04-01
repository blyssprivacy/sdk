// Use ES module import syntax to import functionality from the module
// that we have compiled.
//
// Note that the `default` import is an initialization function which
// will "boot" the module and make it ready to use. Currently browsers
// don't support natively imported WebAssembly as an ES module, but
// eventually the manual initialization won't be required!
import init, { 
    initialize,
    generate_public_parameters,
    generate_query,
    decode_response
} from './pkg/client.js';

const API_URL = "https://spiralwiki.com:8088";
const SETUP_URL = "/setup";
const QUERY_URL = "/query";

async function postData(url = '', data = {}, json = false) {
    const response = await fetch(url, {
      method: 'POST',
      mode: 'cors',
      cache: 'no-cache',
      credentials: 'omit',
      headers: { 'Content-Type': 'application/octet-stream' },
      redirect: 'follow',
      referrerPolicy: 'no-referrer',
      body: data
    });
    if (json) {
        return response.json();
    } else {
        let data = await response.arrayBuffer();
        return new Uint8Array(data);
    }
  }

const api = {
    setup: async (data) => postData(API_URL + SETUP_URL, data, true),
    query: async (data) => postData(API_URL + QUERY_URL, data, false)
}
async function run() {
    // First up we need to actually load the wasm file, so we use the
    // default export to inform it where the wasm file is located on the
    // server, and then we wait on the returned promise to wait for the
    // wasm to be loaded.
    //
    // It may look like this: `await init('./pkg/without_a_bundler_bg.wasm');`,
    // but there is also a handy default inside `init` function, which uses
    // `import.meta` to locate the wasm file relatively to js file.
    //
    // Note that instead of a string you can also pass in any of the
    // following things:
    //
    // * `WebAssembly.Module`
    //
    // * `ArrayBuffer`
    //
    // * `Response`
    //
    // * `Promise` which returns any of the above, e.g. `fetch("./path/to/wasm")`
    //
    // This gives you complete control over how the module is loaded
    // and compiled.
    //
    // Also note that the promise, when resolved, yields the wasm module's
    // exports which is the same as importing the `*_bg` module in other
    // modes
    await init();

    let make_query_btn = document.getElementById("make_query");
    let output_area = document.getElementById("output");

    let has_set_up = false;
    let id = "";
    let client = null;

    make_query_btn.onclick = async () => {
        make_query_btn.disabled = true;

        if (!has_set_up) {
            console.log("Initializing...");
            client = initialize();
            console.log("done");
            console.log("Generating public parameters...");
            let publicParameters = generate_public_parameters(client);
            console.log(`done (${publicParameters.length} bytes)`);
            console.log("Sending public parameters...");
            let setup_resp = await api.setup(publicParameters);
            console.log("sent.");
            console.log(setup_resp);
            id = setup_resp["id"];
            has_set_up = true;
        }

        let targetIdx = parseInt(document.getElementById("query_idx").value, 10);
        if (targetIdx === NaN) targetIdx = 7;

        console.log("Generating query...");
        let query = generate_query(client, id, targetIdx);
        console.log(`done (${query.length} bytes)`);

        console.log("Sending query...");
        let response = await api.query(query);
        console.log("sent.");

        console.log(`done, got (${response.length} bytes)`);
        console.log(response);

        console.log("Decoding result...");
        let result = decode_response(client, response)
        console.log("done.")
        console.log("Final result:")
        console.log(result);

        output_area.innerHTML = result.map (b => b.toString(10)).join(" ");

        make_query_btn.disabled = false;
    }
}
run();
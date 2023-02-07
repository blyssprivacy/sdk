# Blyss SDK

The [Blyss SDK](https://blyss.dev) lets you use homomorphic encryption to make private retrievals from Blyss buckets.

Get an API key by [signing up](https://blyss.dev/auth/sign-up). Detailed documentation is at [docs.blyss.dev](https://docs.blyss.dev).

> **Warning**
> The SDK has not yet been audited or reviewed, and the public Blyss service is still in beta. Data stored in Blyss should not be considered durable. [Contact us](mailto:founders@blyss.dev) for access to a production-ready service.

> _Looking for Spiral?_
> Our new name is Blyss. The core Rust cryptographic library that this repo started as is now in `lib/spiral-rs`.

## Quick start

You can quickly try using the SDK without downloading anything. The example code shows how to use Blyss buckets to perform private contact intersection.

1. Get an API key by [signing up](https://blyss.dev/auth/sign-up).
2. Open [this CodeSandbox](https://codesandbox.io/s/blyss-contact-intersection-example-7qr6r5) and enter your API key where it says `<YOUR API KEY HERE>`. This lets you try using the SDK in your browser.
3. Try adding users to the service using the "Add a user" button. As you add more users, the service will _privately_ intersect each new users's contacts and the already existing users.
   Every user's list of contacts stays completely private using homomorphic encryption: it never leaves their device unencrypted.

If you prefer a simpler example using vanilla JS, check out [this CodePen](https://codepen.io/blyssprivacy/pen/qByMJwr?editors=0010&layout=left).

## Examples

The `examples/` directory has several examples of how to use the Blyss SDK. Running the examples requires [an API key](https://blyss.dev/auth/sign-up).

### Browser

The browser example shows how to quickly start using the Blyss SDK from vanilla JavaScript. The `blyss-bundle.min.js` build output is a single-file JS bundle that binds the library to `window.blyss`. Including this is a fast way to get started, especially if you prefer to use vanilla JS.

1. Edit `examples/browser-simple/main.js` to use your API key.
2. Run a local HTTP server (we suggest [serve](https://github.com/vercel/serve)) in the repo root.
3. Go to `http://localhost:3000/examples/browser-simple/` in a browser.

### React

The React example shows how to use the Blyss SDK in modern client-side JS. It also implements a more complicated application: private contact intersection.

1. Enter `examples/react-complex`, and run `npm install`.
2. Edit `src/App.tsx` to use your API key.
3. Run `npm run start`.

### Node

The Node.js example shows how to use the Blyss SDK in server-side JS. Node 19+ and ESM support is required.

1. Enter `examples/node`, and run `npm install`.
2. Edit `main.ts` to use your API key.
3. Run `ts-node main.ts`.

### Python

The Blyss SDK for Python is still in development. To build the Python library:

1. Enter `python/` and run `python -m venv .env` (on macOS, use `python3 -m venv .env`)
2. Run `source .env/bin/activate`
3. Run `pip install maturin`
4. Run `maturin develop`

This will install the SDK locally as `blyss`. You can now import `blyss` in scripts you run from this virtual environment. To run the Python example:

1. Enter `examples/python`
2. Run `python main.py`

## Documentation

All documentation is available at [docs.blyss.dev](https://docs.blyss.dev). You can generate the docs by running `npm start` in `docs/`.

## Contributing

Please feel free to open issues and pull requests! For bugs, try to provide as much context as possible. We are also always open to documentation contributions.

## Building

Steps to building the SDK:

1. Install Node with [nvm](https://github.com/nvm-sh/nvm#installing-and-updating), Rust with [rustup](https://rustup.rs/), and [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/).
2. Run `npm install`.
3. Run `npm run build`.

This will completely build the SDK, including the core Rust library in `lib/spiral-rs`.

The SDK is structured as:

1. `lib/spiral-rs/`, a Rust crate containing the core cryptographic implementation of the [Spiral PIR scheme](https://eprint.iacr.org/2022/368.pdf).
2. `js/`, the TypeScript code that implements the user-facing Blyss SDK.
   - `js/bridge/`, a Rust "bridge" crate that exposes key functionality from `spiral-rs` to the TypeScript code.
3. `python/`, the Python version of the SDK (still in development)
   - `python/src/lib.rs`, another Rust "bridge" crate that exposes key functionality from `spiral-rs` to the Python code.

## License

MIT (see LICENSE.md)

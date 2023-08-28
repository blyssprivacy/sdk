<h1 align="center">
  <img height="75" src="docs/static/img/logotype-light.svg" alt="blyss">
</h1>
<p align="center">
  <p align="center">Open-source SDK for accessing data privately using homomorphic encryption.</p>
</p>

<h4 align="center">
  <a href="https://docs.blyss.dev">Docs</a> &nbsp; | &nbsp;
  <a href="https://blyss.dev">Website</a> &nbsp; | &nbsp;
  <a href="mailto:founders@blyss.dev">Contact</a>
</h4>

<h4 align="center">
  <a href="https://github.com/blyssprivacy/sdk/blob/main/LICENSE">
    <img src="https://img.shields.io/npm/l/@blyss/sdk?color=blue" alt="The Blyss SDK is released under the MIT license." />
  </a>
  <a href="https://www.npmjs.com/package/@blyss/sdk">
    <img src="https://img.shields.io/npm/v/@blyss/sdk?color=brightgreen" alt="Blyss SDK on NPM" />
  </a>
  <br/>
  <a href="https://signal.group/#CjQKIAVLMoW2pGtd58Ha1tVGtXTv7Z01YV3aA1VmTtX0sj1mEhC07vIrWB7aq9KOw5f2GQsw">
    <img src="https://img.shields.io/badge/chat%20on%20Signal--blue?style=social" alt="Chat on Signal" />
  </a>
  <a href="https://twitter.com/blyssdev">
    <img src="https://img.shields.io/twitter/follow/blyssdev?label=%40blyssdev&style=social" alt="Blyss Twitter" />
  </a>
  <a href="https://github.com/blyssprivacy/sdk">
    <img src="https://img.shields.io/github/stars/blyssprivacy/sdk?style=social" alt="Blyss GitHub stars" />
  </a>
</h4>

The [Blyss SDK](https://blyss.dev) lets you use homomorphic encryption to [retrieve information privately](https://blintzbase.com/posts/pir-and-fhe-from-scratch/). With it, you can build new kinds of privacy-preserving services, like [private password breach checking](https://playground.blyss.dev/passwords/), [private nameserver resolution](https://sprl.it/), and even [private Wikipedia](https://spiralwiki.com/).

You can get an API key by [signing up](https://blyss.dev), or run a server locally. Detailed documentation is at [docs.blyss.dev](https://docs.blyss.dev).

> **Warning**
> The SDK has not yet been security reviewed, and the public Blyss service is still in beta. [Contact us](mailto:founders@blyss.dev) for access to a production-ready service.

## Quick start (cloud)

You can quickly try using the SDK with our managed service without downloading anything.

1. Get an API key by [signing up here](https://blyss.dev).
2. Open [this StackBlitz](https://stackblitz.com/edit/blyss-private-contact-intersection) and enter your API key where it says `<YOUR API KEY HERE>`.
3. Try adding users to the service using the "Add a user" button. As you add more users, the service will _privately_ intersect each new users's contacts and the already existing users.
   Every user's list of contacts stays completely private using homomorphic encryption: it never leaves their device unencrypted.

We also have [a simpler example using vanilla JS](https://codepen.io/blyssprivacy/pen/qByMJwr?editors=0010&layout=left).

## Quick start (local)

You can also use the Blyss SDK completely locally. 

1. Clone this repo with `git clone git@github.com:blyssprivacy/sdk.git`.
2. Run the server by entering `lib/server` and running `cargo run --release`. The server will run on `localhost:8008` by default.
3. Run the client by entering `examples/node-local` and running `npx ts-node main.ts`. This will perform some writes and then a private read to your bucket.

## Install

### JavaScript / Node
To use the Blyss SDK in an existing TypeScript project, install it with `npm install @blyss/sdk`. Then, import the client with `import { Client } from '@blyss/sdk';`. If you're using SDK in Node, and prefer not to use ESM, you can instead import it as `const blyss = require('@blyss/sdk/node')`.

### Python
#### From PyPI:
`pip install --upgrade blyss`

#### From repo:
1. `cd python` from repo root.
2. `pip install --upgrade .`

## Examples

The `examples/` directory has several examples of how to use the Blyss SDK. Running the examples requires [an API key](https://blyss.dev).

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

The Node.js example shows how to use the Blyss SDK in server-side JS. Node 18+ is required.

1. Enter `examples/node`, and run `npm install`.
2. Edit `main.ts` to use your API key.
3. Run `ts-node main.ts`.

### Python

1. Install blyss.
2. Enter `examples/python`.
3. Run `python main.py`.

## Documentation

All documentation is available at [docs.blyss.dev](https://docs.blyss.dev). You can generate the docs by running `npm start` in `docs/`.

## Contributing

Please feel free to open issues and pull requests! For bugs, try to provide as much context as possible. We are also always open to documentation contributions.

## Building from source

### JavaScript / Node

1. Install Node with [nvm](https://github.com/nvm-sh/nvm#installing-and-updating), Rust with [rustup](https://rustup.rs/), and [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/).
2. Run `npm install`.
3. Run `npm run build`.

This will build the complete SDK, including the core Rust libraries in `lib/spiral-rs` and `lib/doublepir`

### Python

Requires python 3.8+.
1. Run `pip install .` in `python`.

## Repository Map

The Blyss SDK is structured as:

1. `lib/`,
   - `lib/server/`, a Rust project containing the open-source Blyss server.
   - `lib/spiral-rs/`, a Rust crate containing the core cryptographic implementation of the [Spiral PIR scheme](https://eprint.iacr.org/2022/368).
   - `lib/doublepir/`, a Rust crate containing the core cryptographic implementation of the [DoublePIR scheme](https://eprint.iacr.org/2022/949).
2. `js/`, the TypeScript code that implements the user-facing Blyss SDK.
   - `js/bridge/`, a Rust "bridge" crate that exposes key functionality from `spiral-rs` and `doublepir` to the TypeScript code.
3. `python/`, the Python version of the SDK.
   - `python/src/lib.rs`, another Rust "bridge" crate that exposes key functionality from `spiral-rs` to the Python code.

## License

MIT (see LICENSE.md)

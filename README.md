# Spiral: Fast, High-Rate Single-Server PIR via FHE Composition

This is an implementation of our paper "Spiral: Fast, High-Rate Single-Server PIR via FHE Composition", available [here](https://eprint.iacr.org/2022/368.pdf). 

> **WARNING**: This is research-quality code; it has not been checked for side-channel leakage or basic logical or memory safety issues. Do not use this in production.

## Building

- In `spiral-rs/spiral-rs`:
    - To build the library `spiral-rs`, run `cargo build --release`.
    - To run the library tests, run `cargo test`.
    - To build the server, run `cargo build --release --bin server --features server`.
    - To preprocess a database, run `cargo build --release --bin preprocess_db`.
    - To run the server, run `target/release/server dbfile.dbp` with the preprocessed database file `dbfile.dbp`
- In `spiral-rs/client`:
    - To build the client for our Wikipedia demo, run `wasm-pack build --target web --out-dir static/pkg`

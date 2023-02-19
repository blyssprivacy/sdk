# DoublePIR in Rust

This is a Rust implementation of the DoublePIR PIR scheme from the paper "One Server for the Price of Two: Simple and Fast Single-Server Private Information Retrieval", available at [https://eprint.iacr.org/2022/949.pdf](https://eprint.iacr.org/2022/949.pdf). Many thanks to the orginal authors for open-sourcing their implementation, available at [https://github.com/ahenzinger/simplepir](https://github.com/ahenzinger/simplepir).

The aim of this implementation is to build a robust, high-quality library with minimal dependencies, a simple interface, and state-of-the-art performance. To those ends, we are building:
  - complete documentation
  - complete test coverage
  - easy-to-run benchmarks
  - eventually, a generic specification for PIR schemes

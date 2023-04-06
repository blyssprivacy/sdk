//! Rust client for [Blyss](https://blyss.dev).
//!
//! Provides interfaces for privately reading data from Blyss buckets.
//! Also exposes a higher-level API for fetching Merkle proofs.
//!
//! Documentation for the Blyss service is at [docs.blyss.dev](https://docs.blyss.dev).

/// Low level functionality for accessing Blyss buckets.
pub mod api;

/// High level functionality for fetching Merkle proofs from Blyss buckets.
pub mod proof;

/// Error types for Blyss.
pub mod error;

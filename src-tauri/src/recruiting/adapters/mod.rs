//! Data-source adapters for the recruiting module.
//!
//! Each adapter wraps one external source (Exa, and later Hunter / GitHub /
//! X — see roadmap phases S1–S3) with a consistent shape: take a query +
//! API key, return a typed response or a typed error. Adapters never read
//! the Keychain themselves; the command layer fetches the key and passes
//! it in. That separation keeps adapters unit-testable without OS-level
//! Keychain access.

pub mod exa;

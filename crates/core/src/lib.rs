//! Rahasyakosh core: crypto, vault format, item model, generator and TOTP.
//!
//! No I/O, no clock, no ambient RNG: callers inject randomness (`rand_core::CryptoRng`)
//! and timestamps, which keeps this crate deterministic in tests and WASM-safe.
#![forbid(unsafe_code)]

pub mod crypto;
pub mod error;
pub mod generator;
pub mod item;
pub mod totp;
pub mod vault;
pub mod wordlist;

pub use error::{Error, Result};

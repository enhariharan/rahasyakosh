//! The portable vault *bundle*: a vault written out as one JSON file, `{ "header": …, "records": […] }`.
//!
//! It is what `rk export -f vault` writes and `rk migrate` reads, and the format of the golden vault in
//! `spec/vectors/`. Everything in it is encrypted, but the file may come from anywhere, so parsing must be total:
//! any input gives a `Bundle` or an error, never a panic.
use crate::{Error, Result};
use rahasyakosh_core::vault::{Record, VaultHeader};
use serde::{Deserialize, Serialize};

/// Largest bundle we will parse.
pub const MAX_BUNDLE_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bundle {
    pub header: VaultHeader,
    pub records: Vec<Record>,
}

impl Bundle {
    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_BUNDLE_BYTES {
            return Err(Error::Bundle("file is too large".into()));
        }
        serde_json::from_slice(bytes).map_err(|e| Error::Bundle(e.to_string()))
    }

    pub fn to_vec(&self) -> Result<Vec<u8>> {
        serde_json::to_vec_pretty(self).map_err(|e| Error::Bundle(e.to_string()))
    }
}

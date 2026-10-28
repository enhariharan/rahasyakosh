//! Operations on an unlocked vault (M2; the session state machine and auto-lock arrive with M3).
//!
//! [`Vault`] ties the pieces together: it unlocks with the master password, keeps the decrypted items and a
//! search index in memory, and writes every change through a [`rahasyakosh_store::Store`] as an encrypted record.
//! Front-ends (the CLI, later the desktop app) use this and never touch keys or records themselves.
//!
//! Like `core`, it reads neither the clock nor a random generator: callers pass the time and an RNG.
#![forbid(unsafe_code)]

#[cfg(not(target_family = "wasm"))]
mod highwater;
mod index;
#[cfg(not(target_family = "wasm"))]
pub mod profile_files;
mod profiles;
#[cfg(not(target_family = "wasm"))]
mod secretkeys;
mod session;
mod vault;

#[cfg(not(target_family = "wasm"))]
pub use highwater::HighWater;
pub use profiles::{
    new_profile_id, Profile, ProfileError, ProfileList, DEFAULT_FULL_NAME, DEFAULT_USERNAME,
};
#[cfg(not(target_family = "wasm"))]
pub use secretkeys::SecretKeyDir;
pub use session::{
    backoff_secs, should_clear, Language, Locked, Session, Settings, ThemeMode, Tick, ToolbarStyle,
    UnlockError, UnlockHints, Unlocked,
};
pub use vault::AUTO_PURGE_LIMIT;
pub use vault::{Config, Error, Result, Scope, Skipped, UnlockFailed, Vault, Warning};

//! Operations on an unlocked vault (M2; the session state machine and auto-lock arrive with M3).
//!
//! [`Vault`] ties the pieces together: it unlocks with the master password, keeps the decrypted items and a
//! search index in memory, and writes every change through a [`rahasyakosh_store::Store`] as an encrypted record.
//! Front-ends (the CLI, later the desktop app) use this and never touch keys or records themselves.
//!
//! Like `core`, it reads neither the clock nor a random generator: callers pass the time and an RNG.
#![forbid(unsafe_code)]

mod index;
mod profiles;
mod session;
mod vault;

pub use profiles::{
    new_profile_id, Profile, ProfileError, ProfileList, DEFAULT_FULL_NAME, DEFAULT_USERNAME,
};
pub use session::{
    backoff_secs, should_clear, Language, Locked, Session, Settings, ThemeMode, Tick, ToolbarStyle,
    UnlockError, Unlocked,
};
pub use vault::{Config, Error, Result, Scope, Skipped, UnlockFailed, Vault};

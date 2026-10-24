//! The things that differ from one operating system to another.
//!
//! The rest of the app calls the small, system-neutral functions in this file. Each system has its own file
//! (`linux.rs`, `other.rs`) with the work that only makes sense there; add a file here for a system that needs its own
//! tweak, rather than putting `cfg` checks into the app.
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as os;

#[cfg(not(target_os = "linux"))]
mod other;
#[cfg(not(target_os = "linux"))]
use other as os;

/// Set (by the system-specific code) when a screen reader has started since the last check.
static SCREEN_READER_STARTED: AtomicBool = AtomicBool::new(false);

/// Whether developer messages should be printed to the console. Switched on by starting the app with
/// `RAHASYAKOSH_DEBUG=1` (any value except empty or `0`).
pub fn debug() -> bool {
    std::env::var_os("RAHASYAKOSH_DEBUG").is_some_and(|v| !v.is_empty() && v != "0")
}

/// Start watching for system events the app has to react to. Call once, on the main thread, before the window opens.
pub fn init() {
    os::watch_screen_reader(|| SCREEN_READER_STARTED.store(true, Ordering::SeqCst));
}

/// Whether a screen reader started since this was last called (the answer is then reset).
///
/// A screen reader started after the app only notices the window once keyboard focus moves, so the app moves it.
pub fn screen_reader_started() -> bool {
    SCREEN_READER_STARTED.swap(false, Ordering::SeqCst)
}

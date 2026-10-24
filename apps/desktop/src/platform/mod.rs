//! The things that differ from one operating system to another.
//!
//! The rest of the app calls the small, system-neutral functions in this file. Each system has its own file
//! (`linux.rs`, `other.rs`) with the work that only makes sense there; add a file here for a system that needs its own
//! tweak, rather than putting `cfg` checks into the app.
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as os;

#[cfg(not(target_os = "linux"))]
mod other;
#[cfg(not(target_os = "linux"))]
use other as os;

/// The latest unreported screen-reader change, written by the system-specific code: 0 none, 1 started, 2 stopped.
static SCREEN_READER_CHANGE: AtomicU8 = AtomicU8::new(0);

/// Whether a screen reader is running now: asked once at start-up, then kept up to date from the changes.
static SCREEN_READER_ON: AtomicBool = AtomicBool::new(false);

/// JavaScript to run in the page shortly after the window gets keyboard focus back while a screen reader runs, if this
/// system's web view needs help telling the screen reader which control has focus.
pub fn script_after_window_focus() -> Option<&'static str> {
    os::SCRIPT_AFTER_WINDOW_FOCUS
}

/// Whether developer messages should be printed to the console. Switched on by starting the app with
/// `RAHASYAKOSH_DEBUG=1` (any value except empty or `0`).
pub fn debug() -> bool {
    std::env::var_os("RAHASYAKOSH_DEBUG").is_some_and(|v| !v.is_empty() && v != "0")
}

/// Start watching for system events the app has to react to. Call once, on the main thread, before the window opens.
pub fn init() {
    SCREEN_READER_ON.store(os::screen_reader_enabled_now(), Ordering::SeqCst);
    if debug() {
        eprintln!(
            "Screen reader at start: {}",
            if screen_reader_active() { "on" } else { "off" }
        );
    }
    os::watch_screen_reader(|enabled| {
        SCREEN_READER_ON.store(enabled, Ordering::SeqCst);
        SCREEN_READER_CHANGE.store(if enabled { 1 } else { 2 }, Ordering::SeqCst)
    });
}

/// Whether a screen reader is running right now.
pub fn screen_reader_active() -> bool {
    SCREEN_READER_ON.load(Ordering::SeqCst)
}

/// Whether a screen reader started (`Some(true)`) or stopped (`Some(false)`) since this was last called.
/// The answer is then reset; if it changed several times in between, the latest change is the one reported.
pub fn screen_reader_change() -> Option<bool> {
    match SCREEN_READER_CHANGE.swap(0, Ordering::SeqCst) {
        1 => Some(true),
        2 => Some(false),
        _ => None,
    }
}

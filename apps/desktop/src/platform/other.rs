//! Systems without their own tweaks yet (macOS, Windows).
//!
//! Nothing is known to be needed here. If a screen reader (VoiceOver, NVDA, Narrator) turns out to miss a window it
//! was started before, do the equivalent of `linux.rs` in a file of its own.

/// Nothing to watch on this system.
pub fn watch_screen_reader(_on_change: impl Fn(bool) + 'static) {}

/// Not known on this system, so assume no screen reader.
pub fn screen_reader_enabled_now() -> bool {
    false
}

/// Nothing known to be needed when the window gets focus back.
pub const SCRIPT_AFTER_WINDOW_FOCUS: Option<&str> = None;

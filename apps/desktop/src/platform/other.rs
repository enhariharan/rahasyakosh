//! Systems without their own tweaks yet (macOS, Windows).
//!
//! Nothing is known to be needed here. If a screen reader (VoiceOver, NVDA, Narrator) turns out to miss a window it
//! was started before, do the equivalent of `linux.rs` in a file of its own.

/// Nothing to watch on this system.
pub fn watch_screen_reader(_on_start: impl Fn() + 'static) {}

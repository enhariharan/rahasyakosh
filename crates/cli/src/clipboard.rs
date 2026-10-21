//! Copy a secret to the desktop clipboard and clear it again after a delay (US-SEC-01).
//!
//! The clipboard itself is `rahasyakosh-clipboard`. This module adds what a one-shot command needs: a small detached helper
//! process that outlives the command, waits, and clears the clipboard only if it still holds our value.
use anyhow::{Context, Result};
use rahasyakosh_clipboard::Backend;
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use zeroize::Zeroizing;

/// Copy `value`, and unless `clear_after` is 0, start a detached helper that clears it afterwards.
pub fn copy_and_schedule_clear(value: &str, clear_after: u64) -> Result<()> {
    let backend = Backend::detect()?;
    backend.copy(value)?;
    if clear_after == 0 {
        return Ok(());
    }
    if let Err(e) = spawn_clearer(value, clear_after) {
        // Never leave a secret on the clipboard that nothing is going to clear.
        let _ = backend.clear();
        return Err(
            e.context("could not start the helper that clears the clipboard; copy cancelled")
        );
    }
    Ok(())
}

/// Re-run this program as a detached helper (own process group, so Ctrl-C in the terminal and closing it do
/// not kill it) and hand it the value on stdin.
fn spawn_clearer(value: &str, after: u64) -> Result<()> {
    let mut child = Command::new(std::env::current_exe()?)
        .args(["clipboard-clear", "--after", &after.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .context("starting the clipboard helper")?;
    child
        .stdin
        .take()
        .expect("piped")
        .write_all(value.as_bytes())?;
    Ok(())
}

/// The helper: wait, then clear the clipboard only if it still holds the value we put there.
pub fn run_clearer(after: u64) -> Result<()> {
    let mut value = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut value)?;
    let value = Zeroizing::new(value);
    std::thread::sleep(std::time::Duration::from_secs(after));
    Backend::detect()?.clear_if_unchanged(&value)?;
    Ok(())
}

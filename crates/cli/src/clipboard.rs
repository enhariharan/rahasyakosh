//! Copy a secret to the desktop clipboard and clear it again after a delay (US-SEC-01).
//!
//! We use the desktop's own clipboard tools (`wl-copy`/`wl-paste` on Wayland, `xclip` or `xsel` on X11)
//! instead of linking a clipboard library: it adds no dependencies to a program that handles secrets, and on
//! Linux a clipboard is served by a process that must stay alive, which these tools handle for us.
//!
//! The secret is passed to tools and to the helper through standard input, never on a command line, where
//! other users could read it from the process list.
use anyhow::{bail, Context, Result};
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Wayland,
    Xclip,
    Xsel,
}

fn on_path(tool: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(tool).is_file()))
        .unwrap_or(false)
}

fn env_set(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|v| !v.is_empty())
}

impl Backend {
    pub fn detect() -> Result<Self> {
        if env_set("WAYLAND_DISPLAY") && on_path("wl-copy") && on_path("wl-paste") {
            return Ok(Self::Wayland);
        }
        if env_set("DISPLAY") {
            if on_path("xclip") {
                return Ok(Self::Xclip);
            }
            if on_path("xsel") {
                return Ok(Self::Xsel);
            }
        }
        bail!(
            "no clipboard tool found. Install one: `sudo apt install wl-clipboard` (Wayland) or \
             `sudo apt install xclip` (X11)"
        )
    }

    fn copy_cmd(self) -> Command {
        match self {
            Self::Wayland => cmd("wl-copy", &["--type", "text/plain"]),
            Self::Xclip => cmd("xclip", &["-selection", "clipboard", "-in"]),
            Self::Xsel => cmd("xsel", &["--clipboard", "--input"]),
        }
    }

    fn read_cmd(self) -> Command {
        match self {
            Self::Wayland => cmd("wl-paste", &["--no-newline", "--type", "text/plain"]),
            Self::Xclip => cmd("xclip", &["-selection", "clipboard", "-out"]),
            Self::Xsel => cmd("xsel", &["--clipboard", "--output"]),
        }
    }

    /// Put `value` on the clipboard.
    pub fn copy(self, value: &str) -> Result<()> {
        let mut child = self
            .copy_cmd()
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("starting the clipboard tool")?;
        child
            .stdin
            .take()
            .expect("piped")
            .write_all(value.as_bytes())?;
        // The tool forks into the background to keep serving the clipboard, so waiting here is short.
        if !child.wait()?.success() {
            bail!("the clipboard tool failed");
        }
        Ok(())
    }

    /// Current clipboard text, or `None` if it is empty or holds something that is not text.
    pub fn read(self) -> Result<Option<Zeroizing<String>>> {
        let out = self
            .read_cmd()
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .context("starting the clipboard tool")?;
        if !out.status.success() {
            return Ok(None);
        }
        Ok(String::from_utf8(out.stdout).ok().map(Zeroizing::new))
    }

    pub fn clear(self) -> Result<()> {
        match self {
            Self::Wayland => run_quiet(cmd("wl-copy", &["--clear"])),
            Self::Xsel => run_quiet(cmd("xsel", &["--clipboard", "--clear"])),
            Self::Xclip => self.copy(""),
        }
    }
}

fn cmd(program: &str, args: &[&str]) -> Command {
    let mut c = Command::new(program);
    c.args(args);
    c
}

fn run_quiet(mut c: Command) -> Result<()> {
    let ok = c
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("starting the clipboard tool")?
        .success();
    if !ok {
        bail!("the clipboard tool failed");
    }
    Ok(())
}

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
        .spawn()?;
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
    let backend = Backend::detect()?;
    if backend.read()?.is_some_and(|now| *now == *value) {
        backend.clear()?;
    }
    Ok(())
}

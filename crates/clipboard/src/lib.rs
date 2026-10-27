//! Talking to the desktop clipboard (US-SEC-01).
//!
//! We use the desktop's own clipboard tools (`wl-copy`/`wl-paste` on Wayland, `xclip` or `xsel` on X11) instead of linking a
//! clipboard library: it adds no dependencies to a program that handles secrets, and on Linux a clipboard is served by a
//! process that must stay alive, which these tools handle for us.
//!
//! The secret is passed to the tools on standard input, never on a command line, where other users could read it from the
//! process list.
use std::io::Write;
use std::process::{Command, Stdio};
use zeroize::Zeroizing;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "no clipboard tool found. Install one: `sudo apt install wl-clipboard` (Wayland) or `sudo apt install xclip` (X11)"
    )]
    NoTool,
    #[error("the clipboard tool could not be run: {0}")]
    Io(#[from] std::io::Error),
    #[error("the clipboard tool failed")]
    Failed,
}

pub type Result<T> = std::result::Result<T, Error>;

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

/// Variables that may hold the master password or the Secret Key. A helper that stays alive to serve the clipboard
/// would keep them in its environment, where the same user and root can read them (audit finding P4-1).
pub const SECRET_ENV: [&str; 2] = ["RK_PASSWORD", "RK_SECRET_KEY"];

fn cmd(program: &str, args: &[&str]) -> Command {
    let mut c = Command::new(program);
    c.args(args);
    for name in SECRET_ENV {
        c.env_remove(name);
    }
    c
}

fn run_quiet(mut c: Command) -> Result<()> {
    let ok = c
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .success();
    if ok {
        Ok(())
    } else {
        Err(Error::Failed)
    }
}

impl Backend {
    /// The clipboard tool this desktop session can use.
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
        Err(Error::NoTool)
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
            .spawn()?;
        child
            .stdin
            .take()
            .expect("piped")
            .write_all(value.as_bytes())?;
        // The tool forks into the background to keep serving the clipboard, so waiting here is short.
        if child.wait()?.success() {
            Ok(())
        } else {
            Err(Error::Failed)
        }
    }

    /// Current clipboard text, or `None` if it is empty or holds something that is not text.
    pub fn read(self) -> Result<Option<Zeroizing<String>>> {
        let out = self
            .read_cmd()
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()?;
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

    /// Clear the clipboard only if it still holds exactly `value`; never destroy something else the user copied.
    /// Returns whether it was cleared.
    pub fn clear_if_unchanged(self, value: &str) -> Result<bool> {
        if self.read()?.is_some_and(|now| *now == value) {
            self.clear()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

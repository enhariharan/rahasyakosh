//! End-to-end tests that drive the real `rk` binary against a throwaway vault.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const PW: &str = "correct horse battery staple wobble";

/// A vault file that is removed when the test ends.
struct Vault(PathBuf);

impl Vault {
    fn new(name: &str) -> Self {
        let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.rkv"));
        let _ = std::fs::remove_file(&path);
        let v = Vault(path);
        assert!(v.rk(&["init"], "").status.success());
        v
    }

    fn rk(&self, args: &[&str], stdin: &str) -> Output {
        run(&self.0, args, stdin)
    }

    fn ok(&self, args: &[&str], stdin: &str) -> String {
        let out = self.rk(args, stdin);
        assert!(
            out.status.success(),
            "rk {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    fn err(&self, args: &[&str]) -> String {
        let out = self.rk(args, "");
        assert!(!out.status.success(), "rk {args:?} should have failed");
        String::from_utf8_lossy(&out.stderr).into_owned()
    }

    fn revs(&self) -> Vec<u64> {
        let v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&self.0).unwrap()).unwrap();
        v["records"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["rev"].as_u64().unwrap())
            .collect()
    }
}

impl Drop for Vault {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn run(vault: &Path, args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rk"))
        .args(args)
        .env("RK_VAULT", vault)
        .env("RK_PASSWORD", PW)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn edit_changes_only_the_given_fields() {
    let v = Vault::new("edit_only_given");
    v.ok(
        &["add", "GitHub", "-u", "alice", "--password-stdin"],
        "old-pw\n",
    );
    v.ok(&["edit", "github", "--username", "bob"], "");
    assert_eq!(v.ok(&["get", "github", "-f", "username"], "").trim(), "bob");
    assert_eq!(v.ok(&["get", "github"], "").trim(), "old-pw");
}

#[test]
fn edit_title_notes_and_revision() {
    let v = Vault::new("edit_title_notes");
    v.ok(&["add", "Bank", "--password-stdin"], "pw\n");
    assert_eq!(v.revs(), [1]);
    v.ok(
        &["edit", "bank", "--title", "My Bank", "--notes", "branch 42"],
        "",
    );
    assert_eq!(v.revs(), [2]);
    assert_eq!(v.ok(&["list"], ""), "My Bank\t\n");
    assert_eq!(
        v.ok(&["get", "my bank", "-f", "notes"], "").trim(),
        "branch 42"
    );
    v.ok(&["edit", "my bank", "--notes", "again"], "");
    assert_eq!(v.revs(), [3]);
}

#[test]
fn edit_password_from_stdin_and_generated() {
    let v = Vault::new("edit_password");
    v.ok(&["add", "Site", "--password-stdin"], "first\n");
    v.ok(&["edit", "site", "--password-stdin"], "second\n");
    assert_eq!(v.ok(&["get", "site"], "").trim(), "second");

    let printed = v.ok(&["edit", "site", "--generate"], "");
    let stored = v.ok(&["get", "site"], "");
    assert_eq!(printed.trim(), stored.trim());
    assert_eq!(printed.trim().chars().count(), 20);
    assert_ne!(printed.trim(), "second");
}

#[test]
fn password_options_are_mutually_exclusive() {
    let v = Vault::new("edit_exclusive");
    v.ok(&["add", "Site", "--password-stdin"], "pw\n");
    v.err(&["edit", "site", "--generate", "--password-stdin"]);
}

#[test]
fn edit_with_nothing_to_change_is_an_error() {
    let v = Vault::new("edit_nothing");
    v.ok(&["add", "Site", "--password-stdin"], "pw\n");
    assert!(v.err(&["edit", "site"]).contains("nothing to change"));
    assert_eq!(v.revs(), [1]);
}

#[test]
fn edit_totp_set_validate_and_remove() {
    let v = Vault::new("edit_totp");
    v.ok(&["add", "Site", "--password-stdin"], "pw\n");

    // An invalid secret is rejected and nothing is stored.
    v.err(&["edit", "site", "--totp", "not base32!"]);
    assert_eq!(v.revs(), [1]);
    v.err(&["totp", "site"]);

    v.ok(&["edit", "site", "--totp", "JBSWY3DPEHPK3PXP"], "");
    let code = v.ok(&["totp", "site"], "");
    assert!(code
        .split('\t')
        .next()
        .unwrap()
        .chars()
        .all(|c| c.is_ascii_digit()));

    v.ok(&["edit", "site", "--remove-totp"], "");
    v.err(&["totp", "site"]);
    v.err(&[
        "edit",
        "site",
        "--totp",
        "JBSWY3DPEHPK3PXP",
        "--remove-totp",
    ]);
}

#[test]
fn edit_url_and_tags() {
    let v = Vault::new("edit_tags");
    v.ok(
        &[
            "add",
            "Site",
            "--url",
            "https://old.test",
            "--password-stdin",
        ],
        "pw\n",
    );
    v.ok(&["edit", "site", "--tag", "work", "--tag", "Dev"], "");
    assert!(v.ok(&["list", "work"], "").contains("Site"));
    // Tags are added once and matched case-insensitively.
    v.ok(&["edit", "site", "--tag", "WORK", "--untag", "dev"], "");
    assert!(v.ok(&["list", "work"], "").contains("Site"));
    assert!(v.ok(&["list", "dev"], "").trim().is_empty());

    assert!(v.ok(&["list", "old.test"], "").contains("Site"));
    v.ok(&["edit", "site", "--url", "https://new.test"], "");
    assert!(v.ok(&["list", "old.test"], "").trim().is_empty());
    assert!(v.ok(&["list", "new.test"], "").contains("Site"));
    v.ok(&["edit", "site", "--url", ""], "");
    assert!(v.ok(&["list", "new.test"], "").trim().is_empty());
}

#[test]
fn edit_unknown_ambiguous_or_trashed_items_fail() {
    let v = Vault::new("edit_errors");
    v.ok(&["add", "Alpha one", "--password-stdin"], "pw\n");
    v.ok(&["add", "Alpha two", "--password-stdin"], "pw\n");
    assert!(v
        .err(&["edit", "zzz", "--notes", "x"])
        .contains("no item matches"));
    assert!(v
        .err(&["edit", "alpha", "--notes", "x"])
        .contains("ambiguous"));
    v.ok(&["rm", "alpha one"], "");
    assert!(v
        .err(&["edit", "alpha one", "--notes", "x"])
        .contains("no item matches"));
}

#[test]
fn wrong_master_password_cannot_edit() {
    let v = Vault::new("edit_wrong_pw");
    v.ok(&["add", "Site", "--password-stdin"], "pw\n");
    let out = Command::new(env!("CARGO_BIN_EXE_rk"))
        .args(["edit", "site", "--notes", "x"])
        .env("RK_VAULT", &v.0)
        .env("RK_PASSWORD", "not the password")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(v.revs(), [1]);
}

/// The committed golden vault (see spec/vectors/README.md) must open with the real CLI.
#[test]
fn cli_opens_the_golden_vault() {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors/golden-v1.rkv");
    let out = Command::new(env!("CARGO_BIN_EXE_rk"))
        .args(["list"])
        .env("RK_VAULT", golden)
        .env("RK_PASSWORD", "रहस्यकोश-golden-1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("GitHub\talice@example.com"));
    assert!(text.contains("बैंक (Bank)\tme"));
    assert!(!text.contains("Old site"), "trashed items are not listed");
}

#[test]
fn trash_lists_and_restore_brings_items_back() {
    let v = Vault::new("trash_restore");
    v.ok(
        &["add", "GitHub", "-u", "alice", "--password-stdin"],
        "gh-pw\n",
    );
    v.ok(
        &["add", "Bank", "-u", "me", "--password-stdin"],
        "bank-pw\n",
    );
    assert_eq!(v.ok(&["trash"], ""), "");

    v.ok(&["rm", "bank"], "");
    assert_eq!(v.ok(&["list"], "").trim(), "GitHub\talice");
    assert_eq!(v.ok(&["trash"], ""), "Bank\tme\tdeleted today\n");
    v.err(&["get", "bank"]);

    v.ok(&["restore", "bank"], "");
    assert_eq!(v.ok(&["trash"], ""), "");
    assert_eq!(v.ok(&["get", "bank"], "").trim(), "bank-pw");
    // Delete (rev 2), restore (rev 3), and the untouched item stays at 1.
    let mut revs = v.revs();
    revs.sort();
    assert_eq!(revs, [1, 3]);
}

#[test]
fn restore_only_sees_the_trash() {
    let v = Vault::new("restore_errors");
    v.ok(&["add", "Site", "--password-stdin"], "pw\n");
    assert!(v.err(&["restore", "site"]).contains("nothing in the trash"));
    assert!(v.err(&["restore", "zzz"]).contains("nothing in the trash"));
}

#[test]
fn trashed_items_can_be_purged_and_removing_twice_explains() {
    let v = Vault::new("purge_trashed");
    v.ok(&["add", "Old", "--password-stdin"], "pw\n");
    v.ok(&["rm", "old"], "");
    // A second plain `rm` tells you where the item is instead of "no item matches".
    assert!(v.err(&["rm", "old"]).contains("already in the trash"));
    v.ok(&["rm", "old", "--purge"], "");
    assert_eq!(v.ok(&["trash"], ""), "");
    assert!(v.revs().is_empty());
}

#[test]
fn purge_can_also_remove_a_live_item_and_prefers_it_over_a_trashed_namesake() {
    let v = Vault::new("purge_prefers_live");
    v.ok(&["add", "Dup", "-u", "first", "--password-stdin"], "one\n");
    v.ok(&["rm", "dup"], "");
    v.ok(&["add", "Dup", "-u", "second", "--password-stdin"], "two\n");
    v.ok(&["rm", "dup", "--purge"], "");
    // The live "second" item went; the trashed "first" one is still recoverable.
    assert_eq!(v.ok(&["trash"], ""), "Dup\tfirst\tdeleted today\n");
    assert_eq!(v.ok(&["list"], ""), "");
}

/// A stand-in for `wl-copy`/`wl-paste` that keeps the "clipboard" in a file, so clipboard behaviour can be
/// tested without a desktop. The real `rk` code path (tool detection, stdin hand-off, the detached helper) runs.
struct FakeClipboard {
    bin: PathBuf,
    clip: PathBuf,
}

impl FakeClipboard {
    fn new(name: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("clip-{name}"));
        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let clip = root.join("clipboard");
        std::fs::write(&clip, "").unwrap();
        let script = |file: &str, body: &str| {
            let p = bin.join(file);
            std::fs::write(&p, format!("#!/bin/sh\nf=\"{}\"\n{body}\n", clip.display())).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        };
        script(
            "wl-copy",
            "if [ \"$1\" = \"--clear\" ]; then : > \"$f\"; exit 0; fi\n/bin/cat > \"$f\"",
        );
        script("wl-paste", "[ -s \"$f\" ] || exit 1\n/bin/cat \"$f\"");
        FakeClipboard { bin, clip }
    }

    fn contents(&self) -> String {
        std::fs::read_to_string(&self.clip).unwrap()
    }

    fn set(&self, text: &str) {
        std::fs::write(&self.clip, text).unwrap();
    }

    /// Wait up to ~6 seconds for the clipboard to become empty.
    fn wait_until_empty(&self) -> bool {
        for _ in 0..60 {
            if self.contents().is_empty() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        false
    }

    fn rk(&self, v: &Vault, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_rk"))
            .args(args)
            .env_clear()
            .env("PATH", &self.bin)
            .env("WAYLAND_DISPLAY", "fake-0")
            .env("RK_VAULT", &v.0)
            .env("RK_PASSWORD", PW)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
}

#[test]
fn copy_puts_the_secret_on_the_clipboard_and_keeps_it_off_the_screen() {
    let v = Vault::new("clip_copy");
    v.ok(
        &["add", "Site", "-u", "me", "--password-stdin"],
        "s3cret-pw\n",
    );
    let clip = FakeClipboard::new("copy");
    let out = clip.rk(&v, &["get", "site", "--copy", "--clear-after", "0"]);
    assert!(out.status.success());
    assert!(out.stdout.is_empty(), "the secret must not be printed");
    assert!(!String::from_utf8_lossy(&out.stderr).contains("s3cret-pw"));
    assert_eq!(clip.contents(), "s3cret-pw");
}

#[test]
fn clipboard_is_cleared_after_the_delay() {
    let v = Vault::new("clip_clear");
    v.ok(&["add", "Site", "--password-stdin"], "s3cret-pw\n");
    let clip = FakeClipboard::new("clear");
    assert!(clip
        .rk(&v, &["get", "site", "-c", "--clear-after", "1"])
        .status
        .success());
    assert_eq!(clip.contents(), "s3cret-pw");
    assert!(
        clip.wait_until_empty(),
        "clipboard should have been cleared"
    );
}

#[test]
fn clipboard_is_not_cleared_if_the_user_copied_something_else() {
    let v = Vault::new("clip_replaced");
    v.ok(&["add", "Site", "--password-stdin"], "s3cret-pw\n");
    let clip = FakeClipboard::new("replaced");
    assert!(clip
        .rk(&v, &["get", "site", "-c", "--clear-after", "2"])
        .status
        .success());
    clip.set("something the user copied");
    std::thread::sleep(std::time::Duration::from_secs(4));
    assert_eq!(clip.contents(), "something the user copied");
}

#[test]
fn zero_means_never_clear() {
    let v = Vault::new("clip_never");
    v.ok(&["add", "Site", "--password-stdin"], "s3cret-pw\n");
    let clip = FakeClipboard::new("never");
    let out = clip.rk(&v, &["get", "site", "-c", "--clear-after", "0"]);
    assert!(String::from_utf8_lossy(&out.stderr).contains("stay on the clipboard"));
    std::thread::sleep(std::time::Duration::from_secs(2));
    assert_eq!(clip.contents(), "s3cret-pw");
}

#[test]
fn copy_works_for_other_fields_and_for_totp() {
    let v = Vault::new("clip_fields");
    v.ok(
        &[
            "add",
            "Site",
            "-u",
            "alice",
            "--password-stdin",
            "--totp",
            "JBSWY3DPEHPK3PXP",
        ],
        "pw\n",
    );
    let clip = FakeClipboard::new("fields");
    assert!(clip
        .rk(
            &v,
            &["get", "site", "-f", "username", "-c", "--clear-after", "0"]
        )
        .status
        .success());
    assert_eq!(clip.contents(), "alice");
    let out = clip.rk(&v, &["totp", "site", "-c", "--clear-after", "0"]);
    assert!(out.status.success() && out.stdout.is_empty());
    let code = clip.contents();
    assert!(
        code.len() == 6 && code.chars().all(|c| c.is_ascii_digit()),
        "{code}"
    );
}

#[test]
fn without_a_clipboard_tool_it_fails_helpfully_and_prints_nothing() {
    let v = Vault::new("clip_missing");
    v.ok(&["add", "Site", "--password-stdin"], "s3cret-pw\n");
    let empty = Path::new(env!("CARGO_TARGET_TMPDIR")).join("clip-no-tools");
    std::fs::create_dir_all(&empty).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rk"))
        .args(["get", "site", "--copy"])
        .env_clear()
        .env("PATH", &empty)
        .env("WAYLAND_DISPLAY", "fake-0")
        .env("RK_VAULT", &v.0)
        .env("RK_PASSWORD", PW)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        out.stdout.is_empty(),
        "must not fall back to printing the secret"
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("wl-clipboard") && err.contains("xclip"),
        "{err}"
    );
    assert!(!err.contains("s3cret-pw"));
}

#[test]
fn clear_after_requires_copy() {
    let v = Vault::new("clip_requires");
    v.ok(&["add", "Site", "--password-stdin"], "pw\n");
    v.err(&["get", "site", "--clear-after", "5"]);
}

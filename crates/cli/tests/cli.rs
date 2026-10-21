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

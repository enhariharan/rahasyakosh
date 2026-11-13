//! End-to-end tests that drive the real `rk` binary against a throwaway vault.
use rahasyakosh_store::{SqliteStore, Store};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const PW: &str = "correct horse battery staple wobble";

/// A vault file that is removed when the test ends.
struct Vault(PathBuf);

impl Vault {
    fn new(name: &str) -> Self {
        let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.db"));
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

    /// Revision numbers of the stored records, read straight from the database.
    fn revs(&self) -> Vec<u64> {
        let store = SqliteStore::open(&self.0).unwrap();
        store
            .list()
            .unwrap()
            .iter()
            .filter(|m| !m.id.is_nil()) // the sealed manifest is not an item
            .map(|m| m.rev)
            .collect()
    }
}

impl Drop for Vault {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Where the test binary's Secret Keys are kept (never the real user's folder).
fn data_home() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("xdg-data")
}

fn run(vault: &Path, args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rk"))
        .args(args)
        .env("RK_VAULT", vault)
        .env("XDG_STATE_HOME", env!("CARGO_TARGET_TMPDIR"))
        .env("XDG_DATA_HOME", data_home())
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
        .env("XDG_STATE_HOME", env!("CARGO_TARGET_TMPDIR"))
        .env("RK_PASSWORD", "not the password")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(v.revs(), [1]);
}

/// The committed golden vault (see spec/vectors/README.md) is a JSON bundle. It must load into the real CLI.
#[test]
fn cli_migrates_and_opens_the_golden_vault() {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors/golden-v1.rkv");
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join("golden-migrated.db");
    let _ = std::fs::remove_file(&target);
    let pw = "रहस्यकोश-golden-1";
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_rk"))
            .args(args)
            .env("RK_VAULT", &target)
            .env("XDG_STATE_HOME", env!("CARGO_TARGET_TMPDIR"))
            .env("RK_PASSWORD", pw)
            .output()
            .unwrap()
    };
    let out = run(&["migrate", golden.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("Migrated 5 item(s)"));

    let out = run(&["list"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("GitHub\talice@example.com"));
    assert!(text.contains("बैंक (Bank)\tme"));
    assert!(!text.contains("Old site"), "trashed items are not listed");
    let _ = std::fs::remove_file(&target);
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
            .env("XDG_STATE_HOME", env!("CARGO_TARGET_TMPDIR"))
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
        .env("XDG_STATE_HOME", env!("CARGO_TARGET_TMPDIR"))
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

// ---- new in M2: favourites, history, migration, auto-purge, friendly errors ------------------------------

#[test]
fn favourites_are_listed_first_and_marked() {
    let v = Vault::new("favourites");
    for t in ["Alpha", "Bravo", "Charlie"] {
        v.ok(&["add", t, "--password-stdin"], "pw\n");
    }
    assert_eq!(v.ok(&["list"], ""), "Alpha\t\nBravo\t\nCharlie\t\n");
    v.ok(&["edit", "charlie", "--favourite"], "");
    assert_eq!(v.ok(&["list"], ""), "Charlie\t\t★\nAlpha\t\nBravo\t\n");
    v.ok(&["edit", "charlie", "--no-favourite"], "");
    assert_eq!(v.ok(&["list"], ""), "Alpha\t\nBravo\t\nCharlie\t\n");
    v.err(&["edit", "charlie", "--favourite", "--no-favourite"]);
}

#[test]
fn history_lists_versions_without_showing_secrets_and_revert_restores() {
    let v = Vault::new("history");
    v.ok(
        &["add", "Site", "-u", "alice", "--password-stdin"],
        "first-secret\n",
    );
    v.ok(&["edit", "site", "--password-stdin"], "second-secret\n");
    v.ok(
        &["edit", "site", "--username", "bob", "--notes", "hello"],
        "",
    );

    let out = v.rk(&["history", "site"], "");
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        !text.contains("secret"),
        "history must not print passwords: {text}"
    );
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert!(
        lines[0].starts_with("0\treplaced today\t")
            && lines[0].contains("username")
            && lines[0].contains("notes"),
        "{text}"
    );
    assert!(
        lines[1].starts_with("1\treplaced today\t") && lines[1].contains("password"),
        "{text}"
    );

    // Go back to the very first password; the present content is kept too.
    v.ok(&["revert", "site", "1"], "");
    assert_eq!(v.ok(&["get", "site"], "").trim(), "first-secret");
    assert_eq!(v.ok(&["get", "site", "-f", "username"], "").trim(), "alice");
    assert_eq!(v.ok(&["history", "site"], "").lines().count(), 3);
    v.err(&["revert", "site", "9"]);
}

#[test]
fn editing_to_the_same_values_reports_no_change_and_writes_nothing() {
    let v = Vault::new("nochange");
    v.ok(&["add", "Site", "-u", "alice", "--password-stdin"], "pw\n");
    let out = v.rk(&["edit", "site", "--username", "alice"], "");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("No changes"));
    assert_eq!(v.revs(), [1]);
    assert_eq!(v.ok(&["history", "site"], ""), "");
}

#[test]
fn migrate_loads_an_old_vault_file_and_refuses_bad_input() {
    let v = Vault::new("migrate-src");
    v.ok(&["add", "Site", "-u", "alice", "--password-stdin"], "pw\n");
    // `export -f vault` writes the portable JSON bundle.
    let bundle = Path::new(env!("CARGO_TARGET_TMPDIR")).join("migrate-src.bundle.rkv");
    let _ = std::fs::remove_file(&bundle);
    v.ok(
        &["export", "-f", "vault", "-o", bundle.to_str().unwrap()],
        "",
    );

    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join("migrate-dst.db");
    let _ = std::fs::remove_file(&target);
    let run = |args: &[&str], pw: &str, vault: &Path| {
        Command::new(env!("CARGO_BIN_EXE_rk"))
            .args(args)
            .env("RK_VAULT", vault)
            .env("XDG_STATE_HOME", env!("CARGO_TARGET_TMPDIR"))
            .env("RK_PASSWORD", pw)
            .output()
            .unwrap()
    };

    // Wrong password: nothing is left behind.
    let out = run(
        &["migrate", bundle.to_str().unwrap()],
        "not the password",
        &target,
    );
    assert!(!out.status.success());
    assert!(
        !target.exists(),
        "a failed migration must not leave a vault file"
    );

    let out = run(&["migrate", bundle.to_str().unwrap()], PW, &target);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = run(&["get", "site"], PW, &target);
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "pw");

    // It will not overwrite an existing vault.
    assert!(!run(&["migrate", bundle.to_str().unwrap()], PW, &target)
        .status
        .success());
    let _ = std::fs::remove_file(&target);
    let _ = std::fs::remove_file(&bundle);
}

#[test]
fn old_format_missing_and_foreign_files_get_helpful_errors() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let run = |p: &Path| {
        Command::new(env!("CARGO_BIN_EXE_rk"))
            .args(["list"])
            .env("RK_VAULT", p)
            .env("RK_PASSWORD", PW)
            .output()
            .unwrap()
    };
    let err = |o: Output| String::from_utf8_lossy(&o.stderr).into_owned();

    let old = dir.join("errors-old.rkv");
    std::fs::write(&old, "{\n  \"header\": {}, \"records\": []\n}").unwrap();
    let e = err(run(&old));
    assert!(
        e.contains("old vault file format") && e.contains("rk migrate"),
        "{e}"
    );

    let foreign = dir.join("errors-foreign.bin");
    std::fs::write(&foreign, b"\x7fELF this is not a vault").unwrap();
    assert!(err(run(&foreign)).contains("not a Rahasyakosh vault"));

    let missing = dir.join("errors-missing.db");
    let _ = std::fs::remove_file(&missing);
    let e = err(run(&missing));
    assert!(e.contains("no vault at") && e.contains("rk init"), "{e}");
    assert!(
        !missing.exists(),
        "a failed open must not create a vault file"
    );
    let _ = std::fs::remove_file(&old);
    let _ = std::fs::remove_file(&foreign);
}

#[test]
fn items_in_the_trash_for_30_days_are_deleted_on_the_next_unlock() {
    use rahasyakosh_app::{Config, Vault as AppVault};
    use rahasyakosh_core::crypto::KdfParams;
    use rahasyakosh_core::item::Item;
    use rand::SeedableRng;

    // Build a vault whose trashed items have different ages, with explicit timestamps.
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("expiry.db");
    let _ = std::fs::remove_file(&path);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let day = 86_400;
    let kdf = KdfParams {
        m_kib: KdfParams::MIN_M_KIB,
        t: KdfParams::MIN_T,
        p: 1,
    };
    let mut rng = rand::rngs::StdRng::seed_from_u64(1);
    let mut av = AppVault::create(
        SqliteStore::open(&path).unwrap(),
        PW,
        kdf,
        Config::default(),
        &mut rng,
    )
    .unwrap();
    let ids: Vec<_> = (1..=3u128)
        .map(|n| {
            let id = rahasyakosh_store::Uuid::from_u128(n);
            av.add(
                Item::login(&format!("Item{n}"), "u", "p", now, id),
                &mut rng,
            )
            .unwrap();
            id
        })
        .collect();
    av.trash(ids[0], now - 31 * day, &mut rng).unwrap(); // expired
    av.trash(ids[1], now - 29 * day, &mut rng).unwrap(); // not yet
    drop(av);

    let out = Command::new(env!("CARGO_BIN_EXE_rk"))
        .args(["trash"])
        .env("RK_VAULT", &path)
        .env("RK_PASSWORD", PW)
        .output()
        .unwrap();
    assert!(out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Deleted 1 item(s)") && stderr.contains("30 days"),
        "{stderr}"
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "Item2\tu\tdeleted 29 days ago\n"
    );
    let _ = std::fs::remove_file(&path);
}

// ---- Secret Key (audit finding P1-3) -------------------------------------------------------------------

fn run_with(vault: &Path, args: &[&str], extra_env: &[(&str, &str)], data: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rk"))
        .args(args)
        .env("RK_VAULT", vault)
        .env("XDG_STATE_HOME", env!("CARGO_TARGET_TMPDIR"))
        .env("XDG_DATA_HOME", data)
        .env("RK_PASSWORD", PW)
        .envs(extra_env.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap()
}

#[test]
fn a_secret_key_vault_needs_the_key_and_this_device_keeps_it() {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("secret_key_cli.db");
    let _ = std::fs::remove_file(&path);
    let data = Path::new(env!("CARGO_TARGET_TMPDIR")).join("sk-cli-data");
    let _ = std::fs::remove_dir_all(&data);

    let init = run_with(&path, &["init", "--secret-key"], &[], &data);
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    let err = String::from_utf8_lossy(&init.stderr).into_owned();
    let key = err
        .split_whitespace()
        .find(|w| w.starts_with("RK1-"))
        .expect("the key is shown once")
        .to_string();

    // On this device the key is found by itself.
    let out = run_with(
        &path,
        &["add", "Bank", "-u", "me", "--generate"],
        &[],
        &data,
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!String::from_utf8_lossy(&out.stdout).contains(&key));

    // Another device (an empty key folder) with the vault file alone: refused, no way to even try passwords.
    let other = Path::new(env!("CARGO_TARGET_TMPDIR")).join("sk-cli-other");
    let _ = std::fs::remove_dir_all(&other);
    let out = run_with(&path, &["list"], &[], &other);
    assert!(!out.status.success());
    // With the key typed (here from the environment) it opens, and the device keeps it.
    let out = run_with(&path, &["list"], &[("RK_SECRET_KEY", &key)], &other);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("Bank"));
    let out = run_with(&path, &["list"], &[], &other);
    assert!(
        out.status.success(),
        "kept on the device after the first use"
    );

    // A mistyped key is reported as such, not as a wrong password.
    let third = Path::new(env!("CARGO_TARGET_TMPDIR")).join("sk-cli-third");
    let _ = std::fs::remove_dir_all(&third);
    let bad = format!("{}A", &key[..key.len() - 1]);
    let out = run_with(&path, &["list"], &[("RK_SECRET_KEY", &bad)], &third);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("typing mistake"));

    // It can be removed again.
    let out = run_with(&path, &["secret-key", "disable"], &[], &data);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = run_with(&path, &["list"], &[], &third);
    assert!(
        out.status.success(),
        "the master password alone opens it again"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn an_unusually_costly_vault_is_announced_before_unlocking() {
    use rahasyakosh_app::{Config, Vault};
    use rahasyakosh_core::crypto::KdfParams;
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("costly.db");
    let _ = std::fs::remove_file(&path);
    let kdf = KdfParams {
        m_kib: 140 * 1024,
        t: 2,
        p: 1,
    };
    Vault::create(
        SqliteStore::open(&path).unwrap(),
        PW,
        kdf,
        Config::default(),
        &mut rand::rng(),
    )
    .unwrap();
    let out = run(&path, &["list"], "");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("asks for 140 MiB"), "{err}");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_short_generated_master_passphrase_is_refused() {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("short_passphrase.db");
    let _ = std::fs::remove_file(&path);
    let data = Path::new(env!("CARGO_TARGET_TMPDIR")).join("short-passphrase-data");

    let out = run_with(&path, &["init", "--passphrase", "--words", "3"], &[], &data);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("at least 5 words"), "{err}");
    assert!(!path.exists(), "no vault should have been made");
}

#[cfg(unix)]
#[test]
fn the_default_folder_of_an_older_install_is_made_private() {
    use std::os::unix::fs::PermissionsExt;
    let home = Path::new(env!("CARGO_TARGET_TMPDIR")).join("old-install-home");
    let _ = std::fs::remove_dir_all(&home);
    let dir = home.join(".local/share/rahasyakosh");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("profiles.json"), b"{}").unwrap();
    for (p, m) in [(&dir, 0o775), (&dir.join("profiles.json"), 0o664)] {
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(m)).unwrap();
    }

    let out = Command::new(env!("CARGO_BIN_EXE_rk"))
        .arg("list")
        .env("HOME", &home)
        .env_remove("RK_VAULT")
        .env("RK_PASSWORD", PW)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!out.status.success(), "there is no vault there yet");

    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&dir.join("profiles.json")), 0o600);
}

#[cfg(unix)]
#[test]
fn an_export_can_never_overwrite_the_live_vault() {
    let v = Vault::new("export_over_vault");
    v.ok(&["add", "Site", "-u", "alice", "--password-stdin"], "pw\n");
    let before = std::fs::read(&v.0).unwrap();
    let dir = v.0.parent().unwrap();
    let name = v.0.file_name().unwrap().to_str().unwrap();

    let link = dir.join("export_over_vault.link");
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(&v.0, &link).unwrap();
    let dotdot = dir.join("..").join(dir.file_name().unwrap()).join(name);
    let spellings = [
        v.0.display().to_string(),
        link.display().to_string(),
        dotdot.display().to_string(),
    ];
    for target in &spellings {
        for format in ["vault", "csv"] {
            let err = v.err(&["export", "-f", format, "-o", target, "--force"]);
            assert!(
                err.contains("is the vault itself") || err.contains("SQLite database"),
                "{format} to {target}: {err}"
            );
        }
    }
    // Nothing changed, and the vault still works.
    assert_eq!(std::fs::read(&v.0).unwrap(), before);
    assert!(v.ok(&["list"], "").contains("Site"));

    // Another SQLite database is not overwritten either, wherever it lives.
    let other = dir.join("export_over_vault.other.db");
    std::fs::copy(&v.0, &other).unwrap();
    let err = v.err(&[
        "export",
        "-f",
        "vault",
        "-o",
        other.to_str().unwrap(),
        "--force",
    ]);
    assert!(err.contains("SQLite database"), "{err}");
}

#[test]
fn escape_sequences_in_stored_text_are_shown_spelled_out_not_sent_to_the_terminal() {
    let v = Vault::new("escape_sequences");
    let csv = Path::new(env!("CARGO_TARGET_TMPDIR")).join("escape_sequences.csv");
    let evil = "\x1b]52;c;aGk=\x07\x1b[2J";
    std::fs::write(
        &csv,
        format!("name,url,username,password\n\"{evil}\",https://e.example/\x1b[31m,\"bob\x1b[1m\",secret\n"),
    )
    .unwrap();
    let csv = csv.to_str().unwrap();
    let no_raw_escapes = |label: &str, out: &Output| {
        let text = String::from_utf8_lossy(&out.stdout).into_owned()
            + &String::from_utf8_lossy(&out.stderr);
        assert!(
            !text.contains('\x1b') && !text.contains('\x07'),
            "{label}: {text:?}"
        );
        text
    };

    let dry = v.rk(&["import", csv, "--dry-run"], "");
    assert!(dry.status.success());
    let text = no_raw_escapes("dry run", &dry);
    assert!(text.contains("\\x1b]52;c;aGk=\\x07\\x1b[2J"), "{text:?}");

    assert!(v.rk(&["import", csv, "--yes"], "").status.success());
    let list = v.rk(&["list"], "");
    no_raw_escapes("list", &list);
    let first = String::from_utf8_lossy(&list.stdout)
        .split('\t')
        .next()
        .unwrap()
        .to_string();
    assert!(first.contains("\\x1b"), "{first}");

    // An error that quotes titles is cleaned as well.
    let get = v.rk(&["get", "no-such-item-\x1b[2J"], "");
    no_raw_escapes("error", &get);

    // Into a pipe the stored value comes out exactly as it is.
    let raw = v.rk(&["get", "52", "-f", "username"], "");
    assert!(
        raw.status.success(),
        "{}",
        String::from_utf8_lossy(&raw.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&raw.stdout), "bob\x1b[1m\n");

    v.ok(&["rm", &first.replace("\\x1b", "")[..4]], "");
    no_raw_escapes("trash", &v.rk(&["trash"], ""));
}

#[test]
fn the_clipboard_helpers_do_not_inherit_the_master_password() {
    use std::os::unix::fs::PermissionsExt;
    let v = Vault::new("clip_env");
    v.ok(&["add", "Site", "--password-stdin"], "s3cret-pw\n");
    let clip = FakeClipboard::new("env");
    // A `wl-copy` that also records the environment it was started with, every time it runs.
    let log = clip.bin.parent().unwrap().join("env.log");
    let _ = std::fs::remove_file(&log);
    let p = clip.bin.join("wl-copy");
    std::fs::write(
        &p,
        format!(
            "#!/bin/sh\nf=\"{}\"\n/usr/bin/env >> \"{}\"\nif [ \"$1\" = \"--clear\" ]; then : > \"$f\"; exit 0; fi\n/bin/cat > \"$f\"\n",
            clip.clip.display(),
            log.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_rk"))
        .args(["get", "site", "-c", "--clear-after", "1"])
        .env_clear()
        .env("PATH", &clip.bin)
        .env("WAYLAND_DISPLAY", "fake-0")
        .env("RK_VAULT", &v.0)
        .env("XDG_STATE_HOME", env!("CARGO_TARGET_TMPDIR"))
        .env("RK_PASSWORD", PW)
        .env("RK_SECRET_KEY", "RK1-NOT-REALLY-A-KEY")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(clip.wait_until_empty(), "the helper should have cleared it");
    let seen = std::fs::read_to_string(&log).unwrap();
    assert!(
        seen.contains("WAYLAND_DISPLAY"),
        "the tool did run: {seen:?}"
    );
    assert!(
        seen.matches("WAYLAND_DISPLAY").count() >= 2,
        "both the copy and the clear were recorded"
    );
    assert!(
        !seen.contains("RK_PASSWORD") && !seen.contains(PW),
        "{seen}"
    );
    assert!(!seen.contains("RK_SECRET_KEY"), "{seen}");
}

/// A data folder with a profile list naming two vaults (the vault files themselves are made by the tests).
fn profile_folder(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("profiles-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("profiles")).unwrap();
    let mut list = rahasyakosh_app::ProfileList::new();
    list.add(
        uuid::Uuid::from_u128(1),
        "Default user",
        "defaultuser",
        "vault.db",
    )
    .unwrap();
    list.add(
        uuid::Uuid::from_u128(2),
        "Asha Rao",
        "Asha",
        "profiles/asha.db",
    )
    .unwrap();
    rahasyakosh_app::profile_files::save_list(&dir, &list).unwrap();
    dir
}

#[test]
fn profile_picks_the_vault_listed_for_that_username() {
    let dir = profile_folder("pick");
    let (main, asha) = (dir.join("vault.db"), dir.join("profiles/asha.db"));
    assert!(run(&main, &["init"], "").status.success());
    assert!(run(&asha, &["init"], "").status.success());
    assert!(run(
        &asha,
        &["add", "OnlyAsha", "-u", "a", "--password-stdin"],
        "pw-1\n"
    )
    .status
    .success());

    // RK_VAULT names the main vault; --profile (any case) switches to Asha's.
    let out = run(&main, &["--profile", "ASHA", "list"], "");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("OnlyAsha"));
    let out = run(&main, &["list"], "");
    assert!(!String::from_utf8_lossy(&out.stdout).contains("OnlyAsha"));
}

#[test]
fn profiles_lists_usernames_and_unknown_or_missing_lists_are_explained() {
    let dir = profile_folder("list");
    let main = dir.join("vault.db");
    let out = run(&main, &["profiles"], "");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("defaultuser\tDefault user"), "{text}");
    assert!(text.contains("Asha\tAsha Rao"), "{text}");

    let out = run(&main, &["--profile", "nobody", "list"], "");
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("nobody") && err.contains("defaultuser, Asha"),
        "{err}"
    );

    let empty = Path::new(env!("CARGO_TARGET_TMPDIR")).join("no-profile-list/vault.db");
    std::fs::create_dir_all(empty.parent().unwrap()).unwrap();
    let _ = std::fs::remove_file(empty.parent().unwrap().join("profiles.json"));
    let out = run(&empty, &["--profile", "x", "list"], "");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no profile list"));
}

#[test]
fn import_reads_a_bitwarden_json_export() {
    let v = Vault::new("import-bitwarden-json");
    let file = v.0.with_extension("json");
    std::fs::write(
        &file,
        r#"{"encrypted":false,"folders":[],"items":[
            {"type":1,"name":"Forum","login":{"username":"carol","password":"pw-2","uris":[{"uri":"https://forum.example"}]}},
            {"type":2,"name":"A note"}]}"#,
    )
    .unwrap();
    let out = v.rk(&["import", file.to_str().unwrap(), "--yes"], "");
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(out.status.success(), "{err}");
    assert!(err.contains("Detected format: bitwarden"), "{err}");
    assert!(v.ok(&["list"], "").contains("Forum"));
    assert!(!v.ok(&["list"], "").contains("A note"));
    let _ = std::fs::remove_file(&file);
}

// ---- shell completions (issue #3) -----------------------------------------------------------------------

/// `rk completions <shell>` prints a completion script for the shell named, and nothing else.
#[test]
fn completions_print_a_non_empty_script_for_each_shell() {
    for shell in ["bash", "zsh", "fish"] {
        let out = Command::new(env!("CARGO_BIN_EXE_rk"))
            .args(["completions", shell])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "rk completions {shell} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let script = String::from_utf8(out.stdout).unwrap();
        assert!(
            !script.trim().is_empty(),
            "the {shell} completion script must not be empty"
        );
        assert!(
            script.contains("rk"),
            "the {shell} completion script must complete the rk command"
        );
        // Nothing but the script: no prompt, no vault, no diagnostics.
        assert!(
            out.stderr.is_empty(),
            "rk completions {shell} wrote to stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// An unsupported shell is rejected instead of printing something half-right.
#[test]
fn completions_rejects_an_unsupported_shell() {
    let out = Command::new(env!("CARGO_BIN_EXE_rk"))
        .args(["completions", "tcsh"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("tcsh"), "{err}");
}

# Security audit test checklist (findings in `docs/reports/`, plan in `docs/13-security-audit.md`)

A manual pass that re-checks every finding of the 2026-10-26 security audit, using the finding IDs (P1-1 … P7-6). It has two uses:
- **Before a fix:** the check should *fail* (the problem is reproduced). Write down what you saw.
- **After a fix:** the check should *pass*. Keep the result as proof that the fix works.

Most checks are also worth turning into automated tests when the fix is written. This list is for the parts that need a person, a real desktop session or a look at the machine. Allow about 90 minutes for a full pass; sections are independent.

> **Never use a real vault.** Everything below uses a throwaway folder. Real passwords must not be typed into a build that is being tested.

## Setup
- [ ] Note the commit (`git rev-parse --short HEAD`), the date, the session type (`echo $XDG_SESSION_TYPE`) and the Ubuntu version.
- [ ] Build the CLI and the desktop app (`cargo build`, and `install-local.sh` in `apps/desktop`).
- [ ] Make a scratch folder and a throwaway vault. In every command below `rk` means the CLI with this vault:
      `mkdir -p /tmp/rk-audit && cd /tmp/rk-audit && export RK_VAULT=/tmp/rk-audit/v.db RK_PASSWORD='audit-test-passphrase'`
      Then `rk init`, and `rk add` two or three items. Unset `RK_PASSWORD` for tests that must prompt.
- [ ] For the desktop app use a separate home: `XDG_DATA_HOME=/tmp/rk-audit/data XDG_CONFIG_HOME=/tmp/rk-audit/cfg rahasyakosh`
- [ ] Use a result mark in the Result column: `✔` secure, `✘` problem reproduced, `?` not sure. Every `✘` after a fix becomes an issue.

## 1. Crypto core (Phase 1)

### P1-1 Unicode normalisation of the master password
| Step | Expected after fix | Result |
|---|---|---|
| Create a vault whose master password is `café` typed with the single character `é` (U+00E9). Unlock it with `cafe` followed by a combining accent (U+0301). | Unlocks. Fixed 2026-10-27 (`crates/core/tests/password_normalisation.rs`). | |
| Repeat with a Devanagari password that has a nukta, once composed and once decomposed (for example `क़` U+0958 vs `क` + `़` U+0915 U+093C). | Both forms unlock. | |
| Take a vault created **before** the fix with a non-ASCII password. | Still unlocks (raw bytes tried first) and is upgraded or flagged. | |
| Type the same password on another keyboard layout or input method (for example Indic input method vs copy and paste). | Unlocks either way. | |
| The strength meter and the password generator show the same result for both forms. | Same bits. | |

### P1-2 Tampered header cannot force a huge Argon2
- [ ] Edit the vault header so `kdf` asks for `m = 1 GiB, t = 20` (use a hex editor on a copy; a throwaway test helper is fine). Try to unlock and time it with `time`.
- [ ] Expected after fix: refused at once with a clear message, or a prompt showing the cost *before* deriving. Pass: well under 1 s and no more than the new ceiling of memory (watch with `/usr/bin/time -v`).
- [ ] Desktop: with a vault whose header asks for more than twice the default (`t` ≥ 7), the unlock screen shows the note about memory and passes above the Unlock button, before any password is typed. A normal vault shows no note. (Automated 2026-10-27; look at it once on screen.)
- [ ] A normal vault still unlocks in the usual time. The desktop slider for the Argon2 cost still reaches only values the unlock accepts (create a vault at the highest setting and unlock it).

### P1-3 Design document matches the code
- [ ] Read `docs/04-security-and-crypto.md`. Every claim is true of the current build, or is marked "planned": Secret Key, `verifier`, header `magic`, `mlock`.
- [ ] The book (`docs/book`) says what protects a stolen vault file today (master password and Argon2 cost, plus the Secret Key if enabled).
- [ ] Fixed 2026-10-27 for the Secret Key (`crates/core/tests/secret_key.rs`, `crates/cli/tests/cli.rs`): `rk init --secret-key` shows a key once; the vault file alone does not open; a new device asks for the key. `verifier`, a header `magic` field and `mlock` are documented as not built.
- [ ] Desktop: with a Secret Key vault and no key saved on this device (move `~/.local/share/rahasyakosh/secret-keys` aside), the unlock screen shows a Secret Key box. A wrong or misspelt key gives "not a valid Secret Key" and does not start the wrong-password delay; the right key opens the vault and the next unlock needs only the password. (Automated 2026-10-27; look at it once on screen, including with Orca.)
- [ ] Desktop: create a new profile with **Also protect this vault with a Secret Key** ticked. After Create, a Secret Key screen shows `RK1-...` once; OK stays disabled until "I have written it down" is ticked; Copy works and the clipboard clears. Lock and unlock: only the password is asked. Move the `secret-keys` folder aside: the Secret Key box appears. Check with Orca that the checkbox, its hint, the key and the confirm box are read in a sensible order. (Not run on screen yet.)

### P1-4 Argon2 working memory
- [ ] `core/Cargo.toml` enables the `zeroize` feature for `argon2`, and `cargo build --target wasm32-unknown-unknown -p rahasyakosh-core` (or the CI wasm job) still passes. Checked 2026-10-27.

### P1-5 No test-only switch in release code
- [ ] `grep -rn "cfg!(test)" crates/` finds no check that changes security behaviour. `cargo build --release` of every crate, with `cargo tree -e features`, shows no test-only feature turned on. Checked 2026-10-27 (`cfg!(test)` removed; `INSECURE_TEST` removed).

### P1-6 Old secrets are wiped
- [ ] Run the desktop app under `gdb` or use a core dump of a throwaway process: add an item with the sentinel password `SENTINEL-OLD-PW`, edit it to `SENTINEL-NEW-PW`, lock, then search the process memory (`gcore`, then `strings core | grep SENTINEL`). Expected after fix: the old value is not found in the `rk` process. (For the web view see P6-3.) Code changed 2026-10-27; the memory check itself has not been run yet.

### P1-7 Item size limit
- [ ] `rk add` with a note of 2 MiB (`head -c 2000000 /dev/zero | tr '\0' a`). Expected: refused with a clear message, and the vault still unlocks afterwards.
- [ ] An item just under the limit is saved, and the vault unlocks and lists it.
- [ ] Import a CSV with one huge cell: refused or truncated with a message, never a vault that cannot be reopened.

(Automated 2026-10-27: `crates/app/tests/item_size.rs`. The limit is 4 MiB, so use a 5 MB note for the manual check.)

### P1-8 Short generated master passphrase
- [ ] `rk init --passphrase --words 3` (on a scratch vault). Expected: refused, or the entropy is shown and confirmation is required. The default (6 words) works without a prompt. Decision 2026-10-27: refused below 5 words; automated in `crates/cli/tests/cli.rs`.

### P1-9 Generator and TOTP
- [ ] Generate 10 000 passwords of length 20 with all classes: every one has at least one character of each class, and the character counts look even (no digit appears twice as often as another). `cargo test -p rahasyakosh-core` passes the TOTP RFC 6238 vectors.

## 2. Storage and integrity (Phase 2)
Use the directory store or SQLite vault as noted. Always copy the vault first (`cp -a`) so you can restore it.

### P2-1 Rollback and deletion
- [ ] Add item A (rev 1). Copy the vault folder. Edit A so its password changes (rev 2). Put the old copy of A's record back. Unlock.
      Fixed 2026-10-27 (`crates/app/tests/integrity.rs`). Expected: a warning ("this item went back to an older version"); the old password is **not** silently shown. (Today: the old version appears.)
- [ ] Delete one record file by hand and unlock. Expected: a warning that an item is missing.
- [ ] Restore the whole vault from a backup that is older than this device's last state: a warning is shown, and the user can accept it knowingly.
- [ ] On a second device or fresh profile that has never seen the vault, unlock works without a false alarm.

### P2-2 Tombstones
- [ ] Write a fake tombstone for an existing item id with rev `18446744073709551615` (`<id>.rkt` containing `{"rev":18446744073709551615}`). Unlock. Expected: the fake tombstone is rejected or reported; the item is still shown; the id can still be edited.
- [ ] A genuine delete (move to trash, then delete forever) still works and stays deleted after a restart.

### P2-3 One bad record
- [ ] Put a file `<random-uuid>.rkr` containing `{}` into the vault folder. Unlock. Expected: the vault opens, the bad record is listed as "could not be read", the other items are all there. (Today: "vault storage is corrupted".)
- [ ] A **wrong password** still fails with the normal message and shows no item names.

### P2-4 Password change and the WAL
- [ ] Change the master password, then run `strings v.db-wal | grep -c wrapped_vk` (or look for two headers). Expected after fix: the WAL is checkpointed and holds one header only.
- [ ] The password-change screen explains what a change does and does not protect against; a "rotate the vault key" option exists or is listed as planned.

### P2-5 Hardening
- [ ] Make a symlink `<id>.rkr -> /dev/zero` in a directory store. Unlock. Expected: skipped or refused at once, no memory growth (watch with `top`).
- [ ] Open a SQLite file with a trigger or view added with the `sqlite3` tool. Expected: the app does not run it, and does not crash.
- [ ] `ls -ld` on the vault's parent folder created by the app is `drwx------` (0700).
- [ ] Automated 2026-10-27 (`crates/store/tests`): the `/dev/zero` link, the planted trigger, the planted temporary name and the create race (header is linked in, not overwritten).

## 3. Session and lock (Phase 3)

### P3-1 Trash retention of 0
- [ ] Put an item in the trash. Write `"trash_retention_days": 0` into `settings.json` and unlock. Expected: the item is **still in the trash**; the setting is raised to 1 (decided 2026-10-27). Automated.
- [ ] Purging many items at once asks for confirmation: not implemented; the 1-day floor is the fix.

### P3-2 settings.json
- [ ] `ls -l ~/.config/rahasyakosh/settings.json` is `-rw-------`. Write `garbage` into it and start the app: the user is told the file could not be read, defaults are used, and the broken file is kept as `settings.json.damaged` (the previous good file is `settings.json.bak`). Automated 2026-10-27; look once at the on-screen notice.
- [ ] Kill the app while saving settings (change a setting, `kill -9` immediately, repeat 20 times): the file is never empty or half-written.

### P3-3 Clock changes
- [ ] Set the idle lock to 1 minute. Move the system clock back 1 hour (`sudo date -s`), wait 2 minutes. Expected after fix: the app locks. Restore the clock afterwards (`sudo timedatectl set-ntp true`).
- [ ] Suspend the laptop for longer than the idle time and resume: the app is locked on resume.

### P3-4 Throttle
- [ ] Enter a wrong password 5 times: the waiting time grows each time.
- [ ] Quit and restart the app: the wait is still there (if persistence is built) or the limit is documented as "keyboard only".
- [ ] In the export and delete-profile dialogs, enter a wrong password 5 times: the same waiting time applies. (Today: no limit.)

### P3-5 Idle tick
- [ ] While the vault is open, `peek` data disappears within one second of the idle time (watch the UI at the lock time).

## 4. Clipboard and CLI (Phase 4)

### P4-1 `RK_PASSWORD` in helper processes
- [ ] `RK_PASSWORD=... rk get <item> --copy`, then immediately `for p in $(pgrep -f 'wl-copy|xclip|xsel|rk'); do tr '\0' '\n' < /proc/$p/environ | grep -c RK_PASSWORD; done`. Expected after fix: `0` for every helper.

### P4-2 `export -f vault --force`
- [ ] `rk export -f vault -o v.db --force` (target = the live vault, by name, by `./v.db`, by absolute path, and through a symlink). Expected: refused every time, and `rk list` still works. (Today: the vault is replaced by a JSON bundle.)
- [ ] The desktop Settings export to the vault file's own path is refused too (it refuses any existing file). Automated 2026-10-27.

### P4-3 Secrets in the command line
- [ ] Run `rk add ... --totp JBSWY3DPEHPK3PXP` and in another terminal `ps aux | grep JBSW`. Expected after fix: a warning for use on the command line, and a `--totp-stdin` (or prompt) route that does not show in `ps`. Check `~/.bash_history` too.

### P4-4 Clipboard clearing
- [ ] Copy a password, wait for the delay, paste elsewhere: empty. Kill the helper (`pkill -f 'rk .*clear'`) before the delay: the message at copy time did not promise more than "should clear".
- [ ] Copy two different passwords within 5 seconds: the second is still cleared at its own time, and the first clear does not remove it early.
- [ ] With a clipboard manager running (GNOME clipboard history, `cliphist`, Klipper): does the password stay in its history? Record yes/no; the docs should say so.

### P4-5 Clipboard tools through PATH
- [ ] Put a fake `wl-copy` script that writes its input to a file first in `PATH`. Copy a password. Expected after fix: the fake is not run (system tools only), or a verbose mode shows which path was used. Remove the fake afterwards.

### P4-6 `rk passwd` with `RK_PASSWORD`
- [ ] With `RK_PASSWORD` set, `rk passwd`. Expected after fix: refused with an explanation. (Today: "Master password changed" while nothing changed.)

### P4-7 Plaintext export file
- [ ] `touch out.csv && chmod 644 out.csv`, then `rk export -f csv -o out.csv --force`. `ls -l out.csv` is `-rw-------`. Repeat with `out.csv` as a symlink to another file: the other file is not overwritten.

### P4-8 Read-only commands
- [ ] Put an item in the trash and age it past the retention (edit the stored time in a test vault). `rk list` does not delete it, or says clearly that it did.

## 5. Import and export (Phase 5)

### P5-1 CSV import speed
- [ ] Generate a CSV with 40 000 distinct rows and run `time rk import big.csv --dry-run`. Expected after fix: a few seconds at most, and doubling the rows about doubles the time (not four times). (Today: 26 s in a debug build.)
- [ ] The desktop import of the same file does not freeze the window for long (progress display not built; the library test takes 0.25 s). Automated in `crates/transfer/tests/large_import.rs`.

### P5-2 Terminal escape sequences
- [ ] Import a CSV whose title is `ESC ] 52 ; c ; aGk= BEL ESC [ 2 J` (`printf '\033]52;c;aGk=\a\033[2J'`). Run `rk import f.csv --dry-run`, then import it and run `rk list`, `rk trash`, `rk history`.
      Expected after fix: the characters are shown as visible escapes (for example `\x1b`), the screen is not cleared, and the clipboard is not changed (`wl-paste`).
- [ ] `rk get` into a pipe (`| xxd`) still writes the value unchanged. Both automated 2026-10-27; run the first once in a real terminal and look at the screen and `wl-paste`.

### P5-3 History in the JSON export
- [ ] Change a password with `rk edit`, export JSON, and search for the old password. Expected after fix: absent, unless `--with-history` is given, or the warning says the file holds old passwords.

### P5-4 CSV formulas
- [ ] Export an item titled `=HYPERLINK("http://example.com","x")` as CSV and open it in LibreOffice. The export warning mentions that spreadsheets run formulas, or the safe option is available.

### P5-5 Reading huge files
- [ ] `rk import /dev/zero` and `rk import` of a 300 MiB file: refused quickly, memory stays low.

### P5-6, P5-7 Memory and labels
- [ ] After an import the sentinel check from P1-6 finds no imported password in `rk`'s memory (partial fix is acceptable; record the result).
- [ ] A custom field name containing a newline does not create a fake block in the text export.

### P5-8 … P5-11 Fuzzing
- [ ] `cargo +nightly fuzz list` shows the new targets (`Settings::from_json`, profile index, text/JSON export, `--map`). `csv_import` is given an `existing` set. A `bundle` target also tries to unlock the vault.
- [ ] Every crash input found is added to a `regressions` folder and replayed in the normal test run (`cargo test -p fuzzcheck`).

## 6. Desktop UI (Phase 6)

### P6-1 Core dumps and ptrace
- [ ] With the desktop app and the CLI running: `grep -E 'Max core|dumpable' /proc/$(pgrep rahasyakosh-desktop)/limits` shows a core limit of `0`, and `cat /proc/$(pgrep rahasyakosh-desktop)/status | grep -i dump` or `prctl` shows not dumpable.
- [ ] Make the app crash on purpose (a debug build with a forced panic, or `kill -SEGV`). `coredumpctl list` and `/var/crash` show no new dump for it.
- [ ] `gdb -p <pid>` as the same user is refused (when `ptrace_scope` is 1 or higher).
- [ ] `docs/04` says what is and is not done about `mlock` and swap.

### P6-2 Folder and file permissions
- [ ] On a fresh install (empty `XDG_DATA_HOME` and `XDG_CONFIG_HOME`), then on an install created by the **old** build:
      `stat -c '%a %n' ~/.local/share/rahasyakosh ~/.local/share/rahasyakosh/profiles ~/.local/share/rahasyakosh/profiles.json ~/.config/rahasyakosh ~/.config/rahasyakosh/settings.json ~/.local/share/rahasyakosh/profiles/*`
      Expected: folders `700`, files `600`, including after the first start of the new build over old modes.
- [ ] Automated 2026-10-27 (`private_fs` tests, desktop `profiles` test, CLI test); still look once at a real old install.
- [ ] As another local user (`sudo -u nobody ls ~hariharan/.local/share/rahasyakosh`): permission denied.

### P6-3 Typed secrets in memory
- [ ] Type the sentinel `SENTINEL-MASTER-PW` into the unlock box and submit (wrong password, so the app stays open), then dump the memory of the desktop process and its `WebKitWebProcess` and search for the sentinel. Record where it is found; it is expected in the web process (outside the guarantee, written in `docs/04`) but not in the main process after the fix.
- [ ] The unlock box, the new-password boxes, the export password box and the delete-profile box are all emptied after use (also after an error).

### P6-4 Edit form
- [ ] Open an item to edit: the password box is masked, with a show/hide button, and it hides again after the reveal timeout. Spell check is off in the secret and TOTP boxes (no red underline on the password).

### P6-5 Web view files
- [ ] Type the sentinels `SENTINEL-MASTER`, `SENTINEL-PW` and `SENTINEL-NOTE` into the master, password and notes boxes, save an item, quit, then:
      `grep -rl SENTINEL ~/.local/share/rahasyakosh-desktop ~/.cache/rahasyakosh-desktop ~/.local/share/rahasyakosh /tmp 2>/dev/null` (the vault itself is encrypted, so it is not a hit). Expected: no hit.
- [ ] If anything is found, the web view uses a private/ephemeral data folder after the fix.

### P6-6 Accessibility tree
- [ ] With `accerciser` (or `busctl` on the a11y bus) look at a revealed password, the TOTP code and the edit form: record what is visible. The book says so. The TOTP code is hidden behind the reveal step if that was decided.

### P6-7 Switching profile and the throttle
- [ ] Two profiles A and B. Enter a wrong password for A four times, switch to B, switch back to A. Expected after fix: A's wait is still counted.

### P6-8 Release build
- [ ] In the installed release build right-click on the window: no "Inspect Element" entry. `F12` and `Ctrl+Shift+I` open nothing.
- [ ] There is a content security policy (or the decision not to have one is written down).

## 7. Supply chain and release (Phase 7)

### P7-1 Accessibility bus
- [ ] Turn the bus off (`gsettings set org.gnome.desktop.interface toolkit-accessibility false`; check `busctl --user get-property org.a11y.Bus /org/a11y/bus org.a11y.Status IsEnabled`). Start the app from the app grid. Expected after fix: the bus is **not** switched on silently (it asks once, or only does so when a screen reader is running). `install-local.sh --no-a11y-bus` exists and its message says what the setting does.

### P7-2 Pinned actions
- [ ] `grep -rn "uses:" .github/workflows` shows third-party actions pinned to full commit SHAs with a tag comment; `ci.yml` has `permissions: contents: read`; Dependabot is configured for actions.

### P7-3 Locked and signed builds
- [ ] Every cargo command in `ci.yml` uses `--locked`; there is a `rust-toolchain.toml`.
- [ ] A release run publishes SHA-256 sums, a signature (`cosign verify-blob` succeeds) and an SBOM. Download the artifact and check the sum yourself.

### P7-4 Release profile
- [ ] `grep -n -A8 'profile.release' Cargo.toml apps/desktop/Cargo.toml` shows `overflow-checks = true` in both, and the book states the panic setting for each.

### P7-5 Deny policy and advisories
- [ ] `cargo deny check` and `cargo audit` are clean in the root workspace and in `apps/desktop`. Every ignore in `deny.toml` still matches a crate (no stale entries).

### P7-6 Install script
- [ ] Install with the home folder in a path that contains a space and a `&` (a test user is enough). The `.desktop` entry and the launcher still start the app. `--uninstall` leaves the vault alone.

## 8. Regression checks (always)
- [ ] `cargo test --workspace`, `cargo clippy --workspace -- -D warnings` and the CI wasm job pass.
- [ ] `cargo audit` and `cargo deny check` pass.
- [ ] Create a vault, add items, lock, unlock, edit, delete, restore from trash, export and import it again: nothing is lost.
- [ ] A vault created by the **previous release** still opens (format compatibility), and a vault made by the new build is rejected cleanly (not corrupted) by an old build.
- [ ] The golden test vectors in `core` still pass unchanged.

## Recording results
Fill in the Result column and paste the tables at the bottom of this file under a dated heading, with the commit, session type and Ubuntu version.
Every `✘` or `?` becomes an issue labelled `security` that names the finding ID, the steps and what was seen. When a finding is fixed, update its Status in the latest report in `docs/reports/` and re-run the matching section here.

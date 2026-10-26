# 13 — Security audit plan and findings

Audit started 2026-10-26. Status key: `todo`, `in progress`, `done`.

## Automated checks (2026-10-26)
- `cargo audit` and `cargo deny check`: clean (advisories, bans, licenses, sources).
- `#![forbid(unsafe_code)]` in `core`, `app`, `store`. The other crates have no `unsafe` in their `.rs` files (dependencies not checked).
- No network code. Attack surface: local files, clipboard subprocess, CLI args, import/parse paths.

## Early findings (leads, to verify)
1. **Rollback and deletion not prevented in `core`.** Record AAD binds `rev`, but an attacker with write access can restore an older valid record or delete records. The design doc promises a local high-water mark; check whether `store`/`app` implements it.
2. **Design doc and code disagree.** `04-security-and-crypto.md` describes a Secret Key (2SKD), a `verifier` blob, a header `magic` and mlock. `core/src/vault.rs` has none of them.
3. **Unauthenticated header drives the KDF.** `unlock` runs Argon2 before anything is authenticated; `KdfParams::validate` allows up to 1 GiB and t=20. Local DoS from a tampered file. Consider a lower cap.
4. **Argon2 `zeroize` feature appears off** (`default-features = false, features = ["alloc"]`), so the working memory is probably not wiped. Confirm against the crate's features.
5. **`panic = "abort"` in release plus about 177 `unwrap`/`expect`/`panic!` in `src`.** The count includes inline test modules. Abort skips `Drop`, so zeroization does not run. Triage which are reachable from hostile input.
6. **`check_kdf` uses `cfg!(test)` to skip validation** (`core/src/vault.rs`). Safe today, fragile. Prefer an explicit parameter.

The primitives, parameters, nonce handling, AAD design and known-answer tests look sound.

## Phases
| # | Phase | Scope | Main questions | Status |
|---|---|---|---|---|
| 1 | Crypto core | `core/{crypto,vault,item,totp,generator}.rs` | Secrets escaping `Zeroizing`? `Item` zeroization incl. history? Generator modulo bias? TOTP handling? | partly read |
| 2 | Storage and integrity | `store/{sqlite,directory}.rs`, `app/vault.rs` | Rollback protection? Atomic writes, file permissions (0600)? SQL injection, path traversal? Plaintext left by tombstones? | done |
| 3 | Session and lock | `app/session.rs` | Auto-lock, key dropped on lock, brute-force throttling, profile index leaks | done |
| 4 | Clipboard and CLI | `clipboard`, `cli` | Clipboard readers, `current_exe()` re-spawn hijack, secrets in argv/env/history/TTY echo, `println!` of passwords | done |
| 5 | Import and export | `transfer`, fuzz targets | CSV/formula injection, plaintext export without re-auth, hostile input, fuzz coverage gaps | done |
| 6 | Desktop UI | `apps/desktop` | Non-zeroized `String`s, screenshot/Wayland leakage, clipboard clearing | done |
| 7 | Supply chain and release | CI, `deny.toml`, packaging | Action pinning, reproducibility, install script | done |

## Approach
Work through the phases in order. Report each finding with file and line, and a severity. Write a test or PoC where a finding is checkable. Fix nothing until the findings have been reviewed.

## Findings log
## Phase 2 — storage and integrity (2026-10-26)
Files read: `store/{lib,directory,sqlite}.rs`, `app/vault.rs`. Four findings were reproduced with a temporary test (kept outside the repo, not committed); the rest are from reading the code.

### P2-1 High — Rollback, deletion and resurrection are undetected (confirmed)
`docs/04` lists "rollback detection (HLC + local high-water mark)" as a mitigation for a malicious sync server. Nothing implements it: a search for high-water/rollback finds nothing. The record AAD binds `rev`, but an older record is still a valid record.
PoC: edit an item to rev 2, restore the old `.rkr` file, unlock. Unlock succeeds and the item is at rev 1 with the old content. Deleting a record file is equally invisible.
Impact: a sync server or anyone with write access can silently revert a changed password to the old one, or remove items.
Fix: keep per-vault state outside the synced folder (item id to highest rev seen, and a record count) and refuse or warn on regression. Also consider one sealed manifest record (id to rev map plus a counter, authenticated under the vault key) so deletion is detectable on a fresh device.

### P2-2 High — Tombstones are unauthenticated (confirmed)
`.rkt` files and the `tombstones` table hold a plain `{"rev": N}`. Anyone who can write the store can hide any item.
PoC: write `<id>.rkt` with rev `u64::MAX`. Unlock succeeds, the item is gone from the list with no warning, and writing that id again fails with `stale revision ... already at 18446744073709551615`. The id stays unusable.
Fix: seal tombstones under the vault key with the id and rev in the AAD, and treat an unsealed or unverifiable tombstone as corrupt. Bound how far a rev may jump.

### P2-3 Medium — One bad record makes the whole vault unopenable (confirmed)
`Vault::unlock_inner` uses `?` on every record, so one undecryptable or malformed record fails the entire unlock.
PoC: drop `<uuid>.rkr` containing `{}` into the directory. Unlock fails with `vault storage is corrupted`.
Impact: a sync peer or a sync conflict can lock the user out of everything until they delete the file by hand.
Fix: skip and report records that fail (a quarantine list the UI shows), and open the rest. A wrong password must still fail closed, which is already decided by the header before any record is read.

### P2-4 Medium — Changing the password does not change the vault key, and the old header stays on disk (confirmed for the WAL part)
`change_password` re-wraps the same VK. Someone who has an old header (a backup, sync history) and the old password can still open every later record. There is no VK rotation, although `docs/04` mentions one.
PoC: after a password change, the SQLite `-wal` file still holds both header copies (old and new). `secure_delete` does not cover WAL frames.
Fix: offer VK rotation (re-encrypt all items) and say in the UI that a password change does not protect against a leaked old password plus an old copy of the vault. Checkpoint with `wal_checkpoint(TRUNCATE)` after `set_header`.

### P2-5 Low — Hardening (from reading, not reproduced)
- **Hostile SQLite file.** `SqliteStore::open` does not set `trusted_schema=OFF` or defensive mode, and `user_version` is trusted. A vault file received from someone else can carry triggers or views. The SQLite parser has a fuzz target, which is good.
- **`DirectoryStore::read` follows symlinks.** `fs::metadata` follows a symlink and reports a small length for a device such as `/dev/zero`, and `fs::read` then reads without limit. The temporary name `.{name}.tmp` is fixed and opened without `O_NOFOLLOW`. Only matters if another party can plant symlinks in the synced folder. Use `symlink_metadata`, reject non-regular files, and read with `take(MAX_FILE)`.
- **`DirectoryStore::create`** checks `exists()` and then writes, which is a race (low impact).
- **Query building with `format!`** in `stored_rev(table)` and `SqliteStore::pragma(name)`. Both are safe today (literals and tests only), but `pragma` is public. Make them private or use an enum.
- **`SqliteStore::open`** creates parent directories with the default umask. The files are 0600, so the data is protected, but listing the directory is not.

### What held up
- Writes are rejected unless the rev is newer, in both backends. The conformance suite checks the same rules for each, which protects against two programs overwriting each other.
- `BEGIN IMMEDIATE` is used, so check-then-write cannot race.
- SQLite batches are atomic. Directory batches are not, and that is documented, and each file is written via tmp + fsync + rename.
- SQLite file and sidecars are 0600 (confirmed for `-wal` and `-shm`); directory store files are 0600 and the directory 0700.
- Parameters are bound (no string-built values); stores never see keys or plaintext.
- Migrations refuse a database newer than the program.

## Phase 3 — session and lock (2026-10-26)
Files read: `app/session.rs`, and how `apps/desktop/src/{model,lib}.rs` drive it. Findings come from reading the code; none are reproduced yet. The profile index was not part of `session.rs`; it is left to Phase 6.

### P3-1 Medium — A settings value of 0 days for the trash deletes every trashed item at the next unlock (from reading)
`Settings::validated` only caps `trash_retention_days` (max 3650), with no minimum. `purge_expired` computes `keep_for = 0`, so `now - d >= 0` holds for every trashed item, and `Session::unlock` runs it right after each successful unlock. `settings.json` is a plain file, so a hand edit, a bad import or a malicious write of `"trash_retention_days": 0` permanently deletes the trash with no prompt and leaves tombstones.
Fix: set a minimum of 1 day (or treat 0 as "never purge"). Ask in the UI before purging more than a few items.

### P3-2 Low — `settings.json` is written without care (from reading)
`save_settings` uses `fs::write` with the default umask (likely 0644), is not atomic, and ignores errors. `from_json` turns any parse error into the defaults, so a half-written file silently resets the user's choices. No secrets are in it, but anyone who can write it can switch off idle lock, clipboard clearing and reveal timeout (`0` means off for all three), which is the same class as P3-1.
Fix: write via tmp + rename with mode 0600. Keep the previous file as `.bak` and tell the user when the file could not be read.

### P3-3 Low — Idle lock and throttle trust the wall clock (from reading)
`now()` is `SystemTime` in both front-ends. Setting the clock back stops the idle lock from firing (`now - last_activity` stays small) and lengthens the unlock wait. Setting it forward locks early, which is harmless. The wall clock does handle suspend correctly (the lock fires on resume), so a plain `Instant` is not a better answer: on Linux `CLOCK_MONOTONIC` stops during suspend.
Fix: take `max(wall elapsed, boottime elapsed)`. Needs root or a clock-setting right, so the impact is low.

### P3-4 Low — The unlock throttle is in memory only, and the re-auth check has none (from reading)
`failures` and `next_attempt_at` live in `Session`. Restarting the app, or opening a profile (`open_profile` builds a new `Session`), resets them. `Model::check_password` (export, delete profile) runs a full Argon2 check without counting failures, so someone at an unlocked screen can guess without limit.
The doc comment already says the throttle only slows someone at the keyboard. Say so in `docs/04` as well, and route `check_password` through the same counter. Consider a persisted counter only if the threat is a person at the keyboard; an attacker with the file is not slowed by it.

### P3-5 Info — `peek()` does not check the idle time
`peek` gives read access until the next `tick`, which is at most one second away. If the UI thread blocks (for example on Argon2 during unlock of another profile), the window is longer. Acceptable; noted so the one-second tick stays a requirement.

### What held up
- `lock()` drops the `Vault` (keys, items, index) and clears every revealed field; `revealed` holds only item ids and field names, never values.
- Idle lock applies on every user action (`vault(now)` ticks first) as well as on the timer. Activity from the future is capped (`last_input.min(t)`).
- The pending clipboard secret is `Zeroizing`, is returned on lock so the caller clears it, and is cleared only if the clipboard still holds exactly that value (`should_clear`).
- Lockout is checked before the password is tried, and only a real decryption failure counts as a wrong password (a corrupt file does not). Backoff arithmetic is capped (shift at most 20, wait at most 300 s) and cannot overflow.
- Key-derivation settings are clamped to the library's floor, so a settings file cannot weaken a new vault.
- The CLI does not use `Session`; it asks for the password per command. Phase 4 covers that.

## Phase 4 — clipboard and CLI (2026-10-26)
Files read: `clipboard/src/lib.rs`, `cli/src/{main,clipboard,file}.rs`. P4-2 was reproduced with a throwaway vault in the session scratchpad (nothing in the repo changed); the rest are from reading the code.

### P4-1 Medium — `RK_PASSWORD` outlives the command inside the clipboard helpers (from reading)
`read_password` takes the master password from `$RK_PASSWORD`. The variable is readable by the same user and root through `/proc/<pid>/environ`, and every child inherits it. `spawn_clearer` (`cli/src/clipboard.rs`) and the `wl-copy`/`xclip`/`xsel` spawns (`clipboard/src/lib.rs`) never clear the environment. So the detached helper holds the master password in its environment for the whole delay, and `xclip`/`wl-copy`, which stay alive to serve the clipboard, hold it until the clipboard is replaced.
Fix: `env_remove("RK_PASSWORD")` (better `env_clear` plus the few variables the tools need: `PATH`, `DISPLAY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`) on every spawn. Longer term, offer `--password-file`/a file descriptor and describe `RK_PASSWORD` as scripting-only in the help.

### P4-2 Medium — `rk export -f vault --force` can overwrite the live vault (confirmed)
The guard `out == path` compares path text. With the vault at `./v.db` and `--output v.db`, the check passes, `--force` skips the exists check, and `file::save` renames the JSON bundle over the open SQLite vault.
PoC: `rk --vault ./v.db export -f vault -o v.db --force` exits 0; the next `rk --vault v.db list` says the file is "in the old vault file format". The data is not lost (the bundle holds the same records, and `rk migrate` can read it), but the vault is unusable until the user works that out, and a `-wal` file may be left beside it.
Fix: compare with `same_file` (device and inode, or `canonicalize` of the parent plus file name) and refuse when the target is an existing SQLite vault. The same applies to the desktop export.

### P4-3 Low — Secrets in the command line (from reading)
`rk add --totp <secret>` and `rk edit --totp <secret>` take the TOTP seed as an argument, so it appears in `ps`, `/proc/<pid>/cmdline` and shell history. `--notes` has the same problem. Passwords do not: `--password` is only a switch for a prompt, and `--password-stdin` exists.
Fix: add `--totp-stdin` (or prompt) and warn when `--totp` is used with a value; same for notes.

### P4-4 Low — The clipboard clear is best effort and the messages promise more (from reading)
- The helper prints nothing (stdout and stderr are null). If it is killed (logout, `pkill`, out of memory) or the tool is missing at clear time, the secret stays and the user was told "the clipboard clears in 20s".
- Two copies of the same value in a row: the first helper clears the clipboard early, which shortens the second.
- Clipboard managers (GNOME, KDE Klipper, `cliphist`) keep their own history; clearing the selection does not touch it. No "sensitive" hint is set.
- The helper reads the secret with `read_to_string` into a plain `String` first, so growth can leave copies in freed memory.
Fix: say "should clear" and mention clipboard managers in the docs and in `docs/04`; let the helper write a failure to a log or to `$XDG_RUNTIME_DIR`; read stdin into a `Zeroizing<Vec<u8>>`.

### P4-5 Low — Clipboard tools are found through `PATH` (from reading)
`on_path` checks `is_file` only (not that it is executable) and the command runs by bare name. A writable directory early in `PATH` (for example `~/.local/bin`) lets another program of the same user plant a `wl-copy` that receives every copied secret on stdin. The re-spawn through `std::env::current_exe()` is not a hijack risk by itself: it uses `/proc/self/exe`, and replacing that binary already gives the attacker everything.
Fix: look the tools up in `/usr/bin` and `/bin` first, or only accept tools owned by root; show the resolved path in a verbose mode.

### P4-6 Low — `rk passwd` with `RK_PASSWORD` set (from reading)
`new_master` reads the new password through the same `read_password`, so with the variable set the "new" password equals the current one: the command reports "Master password changed" and changes nothing (and the repeat prompt is skipped). Not a leak, but it is a silent no-op that looks like success.
Fix: refuse `rk passwd` when `RK_PASSWORD` is set, or read the new password from another source.

### P4-7 Low — Plaintext export file handling (from reading)
`write_private` opens with mode 0600 only when it creates the file. With `--force` on an existing file it keeps that file's old mode (possibly 0644), and follows a symlink at the target. Without `--force`, `create_new` is correct. Fix: for `--force`, write a new temp file (0600) in the same directory and rename it over the target, as `file::save` does.

### P4-8 Info — Read-only commands delete data
`open_vault` calls `purge_expired` with the default 30-day retention for every command, so `rk list` can permanently delete old trash, and uses CLI defaults instead of the desktop settings. Consider doing this only in commands that already write, and tell the user (it already prints a line).

### What held up
- Secrets go to the clipboard tools on standard input, never as arguments; the tools' arguments are fixed strings.
- The master and item passwords are read with echo off (`rpassword`, from the terminal) and held in `Zeroizing`. Plaintext exports ask for the password again on the terminal even when `RK_PASSWORD` is set, and print a warning first.
- Files for exports are created `create_new` + 0600; vault bundles are written to a temp file, fsynced and renamed, 0600.
- The clipboard is cleared only when it still holds exactly the copied value, and a copy is cancelled and cleared if the helper cannot start.
- `get` prints only the requested field; `import` prints title, user and URL, never passwords; error messages do not include secrets.
- No `unsafe` is needed; the `cli` and `clipboard` crates have no `forbid(unsafe_code)` attribute, so add it to make that a rule.

## Phase 5 — import and export (2026-10-26)
Files read: `transfer/src/{lib,bundle}.rs`, the import and export commands in `cli/src/main.rs`, `fuzzcheck/src/lib.rs`, `fuzz/`, `.github/workflows/fuzz.yml`. P5-1, P5-2 and P5-3 were reproduced with the debug `rk` binary and files in the session scratchpad (nothing in the repo changed). The timings are from a debug build.

### P5-1 Medium — CSV import time grows with the square of the row count (confirmed)
For every new row, `import_csv` scans the whole `seen` set (`seen.iter().any(...)`) to count "similar" logins. Cost is O(rows²).
PoC: `rk import big.csv --dry-run` on a file of distinct rows took 5.2 s for 20 000 rows and 25.7 s for 40 000 rows (about 5× for 2×). The 50 MiB cap allows roughly 500 000 short rows, so a hostile or merely large file can hang the CLI and the desktop import for a very long time, with no progress shown.
Fix: index `seen` by (title, username, url) as well, for example `HashMap<(String, String, String), HashSet<String /* password */>>`, so the check is one lookup. Add a large-file timing test.

### P5-2 Medium — Imported text can carry terminal escape sequences (confirmed)
`rk import --dry-run` (and the confirmation listing) and `rk list` print titles, usernames and URLs with `println!` unchanged. A CSV row whose title holds `ESC ] 52 ; c ; … BEL` or `ESC [ 2 J` reaches the terminal as written; the output shows the raw bytes `033 ] 5 2 ; c ; a G k = \a 033 [ 2 J`. Terminals that allow OSC 52 let such a sequence write to the clipboard, and others can rewrite what is on screen, so the user may confirm an import that does not look like what is in the file. The same applies to items that arrive some other way (a shared or synced vault).
Fix: print every untrusted string through one function that replaces control characters (including `ESC`, `BEL` and C1 codes) with `�` or `\x1b` form, in `list`, `trash`, `import`, `history` and error messages. `get` should keep writing values raw to a pipe, but escape when stdout is a terminal.

### P5-3 Low — The JSON export includes the history of old passwords (confirmed)
`export_json` serialises each `Item`, and `Item.history` is part of it. After `rk edit H --password-stdin`, `rk export -f json` contains the old password as well as the new one. "Complete" is the documented intent, but the warning only talks about "every password in it". It also means a user who changed a leaked password still exports the leaked one.
Fix: say so in the export warning and the book, or leave history out unless asked (`--with-history`). The `txt` and `csv` exports do not include it.

### P5-4 Low — CSV export writes cells verbatim (from reading, documented in the code)
A title, note or username beginning with `=`, `+`, `-` or `@` becomes a formula when the file is opened in a spreadsheet (for example `=HYPERLINK(...)` or a DDE call in old versions). The doc comment tells the user to use a text editor, but the CLI and desktop message do not repeat it. Imported items are the likely source of such values. Fix: add the warning to the export message, or offer `--spreadsheet-safe` which prefixes `'` (not default: it breaks round trips into other managers).

### P5-5 Low — Whole files are read before the size limit is checked (from reading)
`rk import` and `file::load` call `std::fs::read` and only then compare the length with `MAX_IMPORT_BYTES` (50 MiB) or `MAX_BUNDLE_BYTES` (256 MiB). A path such as `/dev/zero`, or a very large file, exhausts memory first. A mistake rather than an attack, since the user names the file. Fix: `File::open` + `take(MAX + 1)` + `read_to_end`, and reject non-regular files.

### P5-6 Low — Imported data is not zeroised once parsed (from reading)
The input bytes are `Zeroizing`, but the `csv` reader's buffers and the `StringRecord`s are plain memory, and `Item::login(...)` copies each value. Passwords from a CSV therefore remain in freed memory after the import. The file on disk is the bigger exposure (the tool already says to delete it). Note for the memory-hygiene work in Phase 6.

### P5-7 Info — Text export labels (from reading)
`export_txt` indents multi-line values, so a note cannot forge a new block, but custom field names are written without escaping and can contain newlines. Cosmetic: the file is for people to read.

### Fuzz coverage gaps
Targets today: `vault_header`, `record`, `payload`, `item_json`, `csv_import`, `totp`, `bundle`, `directory_store`, `sqlite_store`, `strength`. Seeds are replayed in the normal test run (`fuzzcheck/tests/replay.rs`) and the weekly workflow runs all ten.
- **P5-8 `csv_import` never passes `existing`** items, and libFuzzer's default input size (4 KiB) cannot show P5-1. Add an `existing` set derived from the input and a timing assertion on a larger generated file.
- **P5-9 `bundle` stops at the store.** It never calls `Vault::unlock` on the records, which is where P2-3 (one bad record blocks the vault) lives. Add an unlock check using the cheap key settings.
- **P5-10 Not fuzzed:** `Settings::from_json` (reads a user-writable file; see P3-1), the profile index file parser, `export_txt`/`export_json` with arbitrary items, and the CLI's `--map` and query parsing. Add small targets.
- **P5-11** Only `item_json` has a `regressions` directory; keep every future crash input there.

### What held up
- Plaintext exports (csv, json, txt) ask for the master password again, warn first, and are returned as `Zeroizing` strings; encrypted bundle export needs no re-auth because it stays encrypted.
- `import_csv` and `Bundle::from_slice` are total (error or value, never a panic) and are fuzzed, with a round trip: what we export, we can import. Passwords, usernames and notes are kept exactly as in the file (not trimmed); warnings never contain secrets (tested).
- Both size limits exist (50 MiB CSV, 256 MiB bundle); `serde_json` limits nesting depth.
- `rk migrate` imports into a new file and proves it opens with the password, and removes the half-built file on failure. The CLI never imports straight into an existing vault without confirmation (`--yes` skips it, `--dry-run` stops before).
- Trashed items are left out of every export.

## Phase 6 — desktop UI (2026-10-26)
Files read: `apps/desktop/src/{lib,model,profiles}.rs`, `platform/linux.rs`, `crates/app/src/profiles.rs`, packaging. P6-2 and P6-5's directory listing were checked on this machine's real data folder (file modes and names only; no contents were read). The rest is from reading the code. No GUI session was driven, so P6-5 and P6-8 still need a run.

### P6-1 Medium — No process hardening: core dumps, ptrace, swap (from reading; also lead 2)
Nothing in the repository calls `prctl`, `setrlimit`, `mlock` or `madvise`, although `docs/04` promises locked memory. A crash (the release profile uses `panic = "abort"`) can write a core file or an Ubuntu crash report with the vault key and decrypted items in it, and the pages can be swapped out. Same-user processes can attach with `ptrace` where `ptrace_scope` allows it.
Fix: at start-up set `RLIMIT_CORE` to 0 and `PR_SET_DUMPABLE` to 0, through a safe wrapper (`rustix` has them) so the crates can keep `forbid(unsafe_code)`; `mlock` needs `unsafe`, so put it in one small audited module or change the design doc to say it is not done.

### P6-2 Medium — The data and config folders are readable by other users (confirmed on this machine)
`~/.local/share/rahasyakosh` is mode 775, `profiles/` 775, `profiles.json` 664, and `~/.config/rahasyakosh/settings.json` 664 (vault files and their `-wal`/`-shm` are 600). `profiles::save`, `save_settings` and `create_dir_all` use the default umask. Any other local user can list the folders and read `profiles.json`, which holds every profile's real full name and username, and can see how many vaults exist.
Fix: create the folders with mode 0700 and write both files 0600 via a temp file and rename, and tighten existing installs at start-up (`chmod` when the mode is wider). This also settles P2-5 (parent directories) and P3-2.

### P6-3 Medium — Typed secrets pass through memory that is never wiped (from reading)
The master password and every other typed secret live in several unwiped places: the `<input>` inside the WebKit web process, the event messages Dioxus sends on each keystroke (the whole value each time), and the `String`s in the signals `pw`, `pw2`, `new_pw`, `new_pw2`, `export_pw`, `del_pw` and the edit-form fields. `submit()` ends with `pw.set(String::new())`, which drops the old `String` without wiping it. Revealed values are copied into `FieldView` and into page text. `Form` does wipe itself, but only its own copy.
This cannot be fully fixed in a web view. Reduce it: keep the typed values in `Zeroizing<String>`, take and wipe them after use, clear the input element after every submit (done only for the unlock box), and say plainly in `docs/04` that the web process is outside the wipe guarantee.

### P6-4 Low — The edit form shows the password in clear (from reading)
`f-pw` in the add/edit form is a normal text input, not `type=password`, and has no reveal timeout. The password stays visible for as long as the form is open (screen sharing, shoulder surfing, accessibility tools). The detail pane hides secrets until revealed, so this is inconsistent.
Fix: mask by default with a show/hide button that follows `reveal_secs`. Also set `spellcheck="false"` on the secret and TOTP boxes.

### P6-5 Low — The web view keeps its own files on disk (partly checked)
`~/.local/share/rahasyakosh-desktop` (`storage`, `mediakeys`, `hsts-storage.sqlite`) and `~/.cache/rahasyakosh-desktop` (`CacheStorage`, `WebKitCache`) exist. The page is local and the code uses no browser storage, so little should be there, but this was not verified.
To check: type a recognisable sentinel into the master, password and notes boxes, quit, and search both folders. If anything is found, switch to an ephemeral/private web-view data folder (`with_incognito`, or a data directory under `$XDG_RUNTIME_DIR`).

### P6-6 Low — Anything that reads the accessibility tree sees revealed values (from reading, by design)
Revealed secrets, the live two-factor code and the edit form values are page text, so any program using AT-SPI (a screen reader, but also any program a user has allowed) can read them. The two-factor code of the selected item is shown without a reveal step. This is a trade-off for screen reader users (the project's stated priority), not a bug. Document it, and consider hiding the code behind the same reveal timeout.

### P6-7 Low — Switching profile resets the unlock throttle (from reading; adds to P3-4)
Choosing a profile in the picker calls `open_profile`, which builds a new `Session`. Someone at the keyboard can go from profile A to B and back to A and the counter for A starts again at zero. Keep the failure counters in the app, keyed by profile ID, for as long as the program runs (and see P3-4 for persistence).

### P6-8 Info — To check in a release build
Whether right-click gives "Inspect Element" or the web inspector can be opened (Dioxus enables developer tools in debug builds; confirm they are off in the release build and the packaged binary), and whether a content-security policy is set. The code has no `dangerous_inner_html`, no links and no external requests, so the practical risk is small.

### What held up
- **No injection surface.** Item text is rendered through Dioxus (escaped). There is no `dangerous_inner_html` and no `href`/navigation, so a hostile title or note cannot run script or open a page. The `eval` calls interpolate only enum-derived constants and static element IDs.
- **Profile paths.** `vault_file` rejects any `file` in `profiles.json` that is absolute or contains `..`, with a test, so a tampered list cannot make "delete profile" remove files outside the data folder. Deleting a profile and exporting both ask for the master password.
- **Lock clears the screen.** `forget_screen` drops the form, selection, query and revealed fields, and the clipboard secret is cleared on lock and on close.
- **No secrets in announcements.** Screen reader messages use field labels, never values; `Debug` for `Form` and `Item` is redacted.
- **Export** refuses to replace a file, creates it 0600, and re-checks the password (but see P3-4 for the missing throttle).
- The profile index is documented as holding no secrets; it does hold real names, which is why P6-2 matters.

## Phase 7 — supply chain and release (2026-10-26)
Files read: `.github/workflows/{ci,fuzz}.yml`, `deny.toml`, `apps/desktop/deny.toml`, both `[profile.release]` sections, `apps/desktop/install-local.sh`, `packaging/`, `SECURITY.md`, `docs/12-publishing-ubuntu.md`. `cargo deny check` and `cargo audit` were run in `apps/desktop` (the root workspace was run on 2026-10-26 earlier). The latest release tags were looked up with `gh`.

### P7-1 Medium — The installer switches the desktop accessibility bus on for the whole session (from reading)
The launcher written by `install-local.sh` sets `org.a11y.Status IsEnabled` to true at every start when it is off. This is done so Orca works (the project's priority), but it is a system-wide change made by a password manager. With the bus on, any program in the login session can use AT-SPI to read other programs' text and watch key events (the same interface a screen reader uses), so it widens the keystroke-observation surface for everything the user types, including master passwords in other programs.
Fix: do not change the setting silently. Ask once (or only do it when a screen reader is running, which the app already detects), say what it does in the book and the install message, and offer `--no-a11y-bus`. Cross-reference P6-6.

### P7-2 Low — GitHub Actions are pinned by tag, not by commit (from reading)
All actions use moving tags: `actions/checkout@v7`, `actions/upload-artifact@v7`, `Swatinem/rust-cache@v2`, `EmbarkStudios/cargo-deny-action@v2`, `taiki-e/install-action@v2`, `bytecodealliance/actions/wasmtime/setup@v1`, and `dtolnay/rust-toolchain@stable`/`@nightly` (a branch name). The tags exist (the latest are v7.0.1, v7.0.2, v2.1.1, v2.87.31, v2.9.3). A compromised third-party action runs with the repository's token and the build tools. The repository default token is read-only, which limits the harm; `ci.yml` has no `permissions:` block (`fuzz.yml` does).
Fix: pin third-party actions to full commit SHAs (keep the tag in a comment, let Dependabot update them), add `permissions: contents: read` to `ci.yml`, and pin the tools installed on the fly (`cargo-audit`, `cargo-deny`, `cargo-fuzz`, `wasmtime` is already pinned).

### P7-3 Low — Builds are not locked or reproducible yet (from reading)
The root jobs run `cargo clippy/test` without `--locked` (the desktop job uses it), the toolchain is "latest stable", and there is no release workflow, checksum, signature or SBOM yet (all listed as to-do in `docs/12`). Installing from source with `install-local.sh` uses `--locked`, which is good.
Fix: add `--locked` to all root jobs, pin the toolchain in `rust-toolchain.toml`, and build releases in a workflow that publishes SHA-256 sums and a signature (`cosign` keyless), with `cargo auditable` or an SBOM, as `docs/12` plans.

### P7-4 Low — Release profile differs between the two workspaces (from reading)
The root `[profile.release]` sets `panic = "abort"`, which applies to `rk`. `apps/desktop` is its own workspace with `lto = "thin"` and `strip` only, so the desktop app unwinds on panic and does run `Drop` (and zeroization) on the way out. This changes lead 5: the abort/`Drop` concern is about the CLI, not the desktop app. Neither profile sets `overflow-checks = true`; for code that does size and time arithmetic on untrusted values that is cheap insurance.
Fix: state the choice in the book, and enable `overflow-checks` for release in both.

### P7-5 Info — Deny policy and advisory exceptions
- Root policy is strict (no OpenSSL, `ring` banned, wildcards denied, unknown registries and git sources denied) and passes. The desktop policy allows OpenSSL only through `native-tls`, each exception has a written reason, and `cargo deny check` passes there too.
- `RUSTSEC-2026-0097` (`rand` 0.7.3 unsound with a custom logger) is an accepted warning. Its stated reason holds: the desktop dependency tree has no `tracing-subscriber` or `dioxus-logger`. `cargo deny` reports that ignore as "no crate matched", so the entry may be stale for deny while `cargo audit` still shows it. Check and remove stale ignores whenever dependencies change.
- `cargo audit` in the desktop reports 5 allowed warnings (unmaintained or unsound crates inside the GUI toolkit), all listed in `deny.toml`.

### P7-6 Info — Install script (from reading)
It touches only `~/.local`, builds with `--locked`, installs with explicit modes, and `--uninstall` leaves the vault alone. Minor: the `.desktop` file is made with `sed` and `|` as delimiter, which breaks for a home directory containing `|` or `&`; the launcher embeds `$bin` in double quotes, which is fine for normal paths. Both are cosmetic.

### What held up
- Workflows use `pull_request`, not `pull_request_target`, so forks cannot reach secrets; the fuzz workflow runs with `contents: read`; no secrets are used anywhere in CI.
- `Cargo.lock` is committed for both workspaces; the desktop job builds `--locked`.
- `SECURITY.md` gives a private reporting channel, a response time and a scope, and states the project is unaudited.
- The plan in `docs/12` (checksums, signing, sandbox testing with Orca and the clipboard before a snap) covers the right topics.

---

# Summary of the audit (2026-10-26)

All seven phases are done. Nothing has been fixed, as agreed. Severity is the auditor's judgement for a local-first app whose main risks are a copied vault file and a hostile sync folder. "Confirmed" means reproduced; the rest is from reading code.

| ID | Sev | Finding | Status |
|---|---|---|---|
| P2-1 | High | Rollback and deletion of records undetected (no high-water mark) | confirmed |
| P2-2 | High | Tombstones unauthenticated; any rev hides an item and blocks its id | confirmed |
| P2-3 | Medium | One bad record makes the whole vault unopenable | confirmed |
| P2-4 | Medium | Password change keeps the vault key; old header stays in the WAL | confirmed (WAL) |
| P1* | Medium | Unauthenticated header drives Argon2 (up to 1 GiB, t=20); design doc and code disagree (Secret Key, verifier, mlock) | leads, phase 1 still partly read |
| P6-1 | Medium | No core-dump, ptrace or mlock hardening | reading |
| P6-2 | Medium | Data and config folders and `profiles.json` readable by other users | confirmed |
| P6-3 | Medium | Typed secrets pass through unwiped memory (web view, signals) | reading |
| P4-1 | Medium | `RK_PASSWORD` stays in the clipboard helpers' environment | reading |
| P4-2 | Medium | `export -f vault --force` can overwrite the live vault | confirmed |
| P5-1 | Medium | CSV import is quadratic (40k rows took 26 s in a debug build) | confirmed |
| P5-2 | Medium | Terminal escape sequences from imported text are printed raw | confirmed |
| P3-1 | Medium | `trash_retention_days = 0` purges all trash at the next unlock | reading |
| P7-1 | Medium | Installer turns the accessibility bus on session-wide | reading |
| P2-5, P3-2..5, P4-3..8, P5-3..11, P6-4..8, P7-2..6 | Low/Info | Hardening and documentation items | see sections |

Not yet done: **Phase 1** is marked "partly read" in the table (crypto core: `item`, `totp`, `generator`, Argon2 `zeroize` feature, `cfg!(test)` in `check_kdf`). It should be finished before the findings are reviewed, because the early leads 3, 4 and 6 depend on it.

Suggested order for the fixes, once reviewed: (1) rollback and authenticated tombstones, with a sealed manifest (P2-1, P2-2, P2-3); (2) file modes and the folder mode on every platform path (P6-2, P3-2, P2-5); (3) the small, local bugs with tests (P4-2, P3-1, P5-1, P5-2, P4-1); (4) process hardening and memory hygiene (P6-1, P6-3); (5) CI pinning and release signing (P7-2, P7-3).

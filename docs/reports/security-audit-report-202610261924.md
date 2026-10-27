<a id="top"></a>
# Security audit report — 2026-10-26

| | |
|---|---|
| **Report written** | 2026-10-26 19:24 |
| **Code audited** | `main` at commit `7890072` (the audit was done on the code as of 2026-10-26; no code was changed during it) |
| **Audited by** | Claude (AI assistant), reviewed by the project owner. This is not an independent third-party audit. |
| **Plan, scope and method** | [`../13-security-audit.md`](../13-security-audit.md) |
| **Manual re-test list** | [`../tests/security-audit-test-checklist.md`](../tests/security-audit-test-checklist.md) |

How to read the findings:
- Any word you do not know is explained in the [Glossary](../book/src/glossary.md).
- **Issue** (the first item of each finding in section 4) says in plain English what the problem is, and explains any technical word it uses. The items after it give the details for people who want to check the work.
- **Severity** is the auditor's judgement for a local-first app whose main risks are a copied vault file and a hostile sync folder: High, Medium, Low, Info.
- **How found:** *Confirmed* = reproduced with a test or command; *Measured* = reproduced and timed; *From reading* = found by reading the code, not run; *Checked* = a suspicion that was looked at and judged.
- **Status:** `open` = not fixed. The report records the audit as it stood on 2026-10-26; fixes are noted in this column as they land (with the date and the test), and re-run the matching section of the test checklist.

## Contents

- [1. Summary](#s1)
  - [Findings that matter most](#findings-that-matter-most)
  - [Suggested order for the fixes (once the findings are reviewed)](#suggested-order-for-the-fixes-once-the-findings-are-reviewed)
- [2. Automated checks (2026-10-26)](#s2)
- [3. Early leads and what became of them](#s3)
- [4. Findings by phase](#s4)
  - [Phase 1 — Crypto core](#phase-1)
    - [P1-1 — The master password is used as raw bytes (Medium)](#p1-1)
    - [P1-2 — A tampered header makes unlock run a huge Argon2 before anything is authenticated (Medium)](#p1-2)
    - [P1-3 — `docs/04` promises things the code does not do (Medium)](#p1-3)
    - [P1-4 — The Argon2 `zeroize` feature is off (Low)](#p1-4)
    - [P1-5 — `check_kdf` skips validation with `cfg!(test)` (Low)](#p1-5)
    - [P1-6 — Old secrets are dropped without being wiped (Low)](#p1-6)
    - [P1-7 — No limit on the size of an item (Low)](#p1-7)
    - [P1-8 — A generated master passphrase can be very short (Low)](#p1-8)
    - [P1-9 — Generator and TOTP details (Info)](#p1-9)
  - [Phase 2 — Storage and integrity](#phase-2)
    - [P2-1 — Rollback, deletion and resurrection are undetected (High)](#p2-1)
    - [P2-2 — Tombstones are unauthenticated (High)](#p2-2)
    - [P2-3 — One bad record makes the whole vault unopenable (Medium)](#p2-3)
    - [P2-4 — Changing the password does not change the vault key, and the old header stays on disk (Medium)](#p2-4)
    - [P2-5 — Hardening gaps in the stores (Low)](#p2-5)
  - [Phase 3 — Session and lock](#phase-3)
    - [P3-1 — A settings value of 0 days for the trash deletes every trashed item at the next unlock (Medium)](#p3-1)
    - [P3-2 — `settings.json` is written without care (Low)](#p3-2)
    - [P3-3 — Idle lock and throttle trust the wall clock (Low)](#p3-3)
    - [P3-4 — The unlock throttle is in memory only, and the re-auth check has none (Low)](#p3-4)
    - [P3-5 — `peek()` does not check the idle time (Info)](#p3-5)
  - [Phase 4 — Clipboard and CLI](#phase-4)
    - [P4-1 — `RK_PASSWORD` outlives the command inside the clipboard helpers (Medium)](#p4-1)
    - [P4-2 — `rk export -f vault --force` can overwrite the live vault (Medium)](#p4-2)
    - [P4-3 — Secrets in the command line (Low)](#p4-3)
    - [P4-4 — The clipboard clear is best effort and the messages promise more (Low)](#p4-4)
    - [P4-5 — Clipboard tools are found through `PATH` (Low)](#p4-5)
    - [P4-6 — `rk passwd` with `RK_PASSWORD` set (Low)](#p4-6)
    - [P4-7 — Plaintext export file handling (Low)](#p4-7)
    - [P4-8 — Read-only commands delete data (Info)](#p4-8)
  - [Phase 5 — Import and export](#phase-5)
    - [P5-1 — CSV import time grows with the square of the row count (Medium)](#p5-1)
    - [P5-2 — Imported text can carry terminal escape sequences (Medium)](#p5-2)
    - [P5-3 — The JSON export includes the history of old passwords (Low)](#p5-3)
    - [P5-4 — CSV export writes cells verbatim (Low)](#p5-4)
    - [P5-5 — Whole files are read before the size limit is checked (Low)](#p5-5)
    - [P5-6 — Imported data is not zeroised once parsed (Low)](#p5-6)
    - [P5-7 — Text export labels (Info)](#p5-7)
    - [P5-8 — The `csv_import` fuzz target never passes `existing` items (Low)](#p5-8)
    - [P5-9 — The `bundle` fuzz target stops at the store (Low)](#p5-9)
    - [P5-10 — Several readers of untrusted input are not fuzzed (Low)](#p5-10)
    - [P5-11 — Only `item_json` has a `regressions` directory (Low)](#p5-11)
  - [Phase 6 — Desktop UI](#phase-6)
    - [P6-1 — No process hardening: core dumps, ptrace, swap (Medium)](#p6-1)
    - [P6-2 — The data and config folders are readable by other users (Medium)](#p6-2)
    - [P6-3 — Typed secrets pass through memory that is never wiped (Medium)](#p6-3)
    - [P6-4 — The edit form shows the password in clear (Low)](#p6-4)
    - [P6-5 — The web view keeps its own files on disk (Low)](#p6-5)
    - [P6-6 — Anything that reads the accessibility tree sees revealed values (Low)](#p6-6)
    - [P6-7 — Switching profile resets the unlock throttle (Low)](#p6-7)
    - [P6-8 — To check in a release build (Info)](#p6-8)
  - [Phase 7 — Supply chain and release](#phase-7)
    - [P7-1 — The installer switches the desktop accessibility bus on for the whole session (Medium)](#p7-1)
    - [P7-2 — GitHub Actions are pinned by tag, not by commit (Low)](#p7-2)
    - [P7-3 — Builds are not locked or reproducible yet (Low)](#p7-3)
    - [P7-4 — Release profile differs between the two workspaces (Low)](#p7-4)
    - [P7-5 — Deny policy and advisory exceptions (Info)](#p7-5)
    - [P7-6 — Install script details (Info)](#p7-6)
- [5. Not covered, and what to do next](#s5)

<a id="s1"></a>
## 1. Summary
All seven phases are done (Phase 1 was finished last, on the same day). 52 findings were recorded.

| Severity | Count | Findings |
|---|---|---|
| High | 2 | [P2-1](#p2-1), [P2-2](#p2-2) |
| Medium | 14 | [P1-1](#p1-1), [P1-2](#p1-2), [P1-3](#p1-3), [P2-3](#p2-3), [P2-4](#p2-4), [P3-1](#p3-1), [P4-1](#p4-1), [P4-2](#p4-2), [P5-1](#p5-1), [P5-2](#p5-2), [P6-1](#p6-1), [P6-2](#p6-2), [P6-3](#p6-3), [P7-1](#p7-1) |
| Low | 29 | [P1-4](#p1-4) to [P1-8](#p1-8), [P2-5](#p2-5), [P3-2](#p3-2) to [P3-4](#p3-4), [P4-3](#p4-3) to [P4-7](#p4-7), [P5-3](#p5-3) to [P5-6](#p5-6), [P5-8](#p5-8) to [P5-11](#p5-11), [P6-4](#p6-4) to [P6-7](#p6-7), [P7-2](#p7-2) to [P7-4](#p7-4) |
| Info | 7 | [P1-9](#p1-9), [P3-5](#p3-5), [P4-8](#p4-8), [P5-7](#p5-7), [P6-8](#p6-8), [P7-5](#p7-5), [P7-6](#p7-6) |

<a id="findings-that-matter-most"></a>
### Findings that matter most
| ID | Severity | Finding | How found | Status |
|---|---|---|---|---|
| [P2-1](#p2-1) | High | Rollback and deletion of records are undetected (no high-water mark) | Confirmed | fixed 2026-10-27 |
| [P2-2](#p2-2) | High | Tombstones are unauthenticated; any rev hides an item and blocks its id | Confirmed | fixed 2026-10-27 |
| [P2-3](#p2-3) | Medium | One bad record makes the whole vault unopenable | Confirmed | fixed |
| [P2-4](#p2-4) | Medium | Password change keeps the vault key; the old header stays in the WAL | Confirmed (WAL) | open |
| [P1-1](#p1-1) | Medium | The master password is not Unicode-normalised before Argon2 (lock-out risk across keyboards and platforms; relevant to the Indic-language users) | From reading | fixed 2026-10-27 |
| [P1-2](#p1-2) | Medium | The unauthenticated header drives Argon2: one unlock attempt with the allowed maximum took 10.8 s and 1 GiB | Measured | fixed 2026-10-27 |
| [P1-3](#p1-3) | Medium | The design doc (`docs/04`) and the code disagree: no Secret Key, verifier blob, header magic or `mlock` | Confirmed | fixed in part 2026-10-27 |
| [P6-1](#p6-1) | Medium | No core-dump, ptrace or `mlock` hardening | From reading | open |
| [P6-2](#p6-2) | Medium | Data and config folders and `profiles.json` are readable by other users | Confirmed | open |
| [P6-3](#p6-3) | Medium | Typed secrets pass through memory that is never wiped (web view, signals) | From reading | open |
| [P4-1](#p4-1) | Medium | `RK_PASSWORD` stays in the clipboard helpers' environment | From reading | open |
| [P4-2](#p4-2) | Medium | `export -f vault --force` can overwrite the live vault | Confirmed | open |
| [P5-1](#p5-1) | Medium | CSV import is quadratic (40 000 rows took 26 s in a debug build) | Confirmed | open |
| [P5-2](#p5-2) | Medium | Terminal escape sequences from imported text are printed raw | Confirmed | open |
| [P3-1](#p3-1) | Medium | `trash_retention_days = 0` purges all trash at the next unlock | From reading | open |
| [P7-1](#p7-1) | Medium | The installer turns the accessibility bus on session-wide | From reading | open |

<a id="suggested-order-for-the-fixes-once-the-findings-are-reviewed"></a>
### Suggested order for the fixes (once the findings are reviewed)
1. Rollback and authenticated tombstones, with a sealed manifest ([P2-1](#p2-1), [P2-2](#p2-2), [P2-3](#p2-3)), together with the Argon2 ceiling ([P1-2](#p1-2)).
2. File modes and the folder mode on every platform path ([P6-2](#p6-2), [P3-2](#p3-2), [P2-5](#p2-5)).
3. The small, local bugs, each with a test ([P4-2](#p4-2), [P3-1](#p3-1), [P5-1](#p5-1), [P5-2](#p5-2), [P4-1](#p4-1), and the quick Low items [P1-4](#p1-4), [P1-7](#p1-7), [P1-8](#p1-8)).
4. Process hardening and memory hygiene ([P6-1](#p6-1), [P6-3](#p6-3), [P1-6](#p1-6)).
5. CI pinning and release signing ([P7-2](#p7-2), [P7-3](#p7-3)).
6. Decisions needed first: Unicode normalisation ([P1-1](#p1-1), changes the key for existing non-ASCII passwords, so it needs a header format flag) and whether to build the Secret Key or mark it as planned ([P1-3](#p1-3)).

[Top](#top)

<a id="s2"></a>
## 2. Automated checks (2026-10-26)
| Check | Result |
|---|---|
| `cargo audit` and `cargo deny check` | Clean (advisories, bans, licenses, sources) |
| `#![forbid(unsafe_code)]` | Present in `core`, `app`, `store`. The other crates have no `unsafe` in their `.rs` files (dependencies not checked). |
| Network code | None. Attack surface: local files, clipboard subprocess, CLI arguments, import and parse paths. |

[Top](#top)

<a id="s3"></a>
## 3. Early leads and what became of them
| # | Lead | Outcome | Finding |
|---|---|---|---|
| 1 | Rollback and deletion not prevented in `core`; the design doc promises a local high-water mark | Confirmed: nothing implements it | [P2-1](#p2-1) |
| 2 | Design doc and code disagree (Secret Key, `verifier`, header `magic`, `mlock`) | Confirmed | [P1-3](#p1-3), [P6-1](#p6-1) |
| 3 | Unauthenticated header drives the KDF (up to 1 GiB, t = 20) | Confirmed and measured (10.8 s) | [P1-2](#p1-2) |
| 4 | Argon2 `zeroize` feature appears off | Confirmed | [P1-4](#p1-4) |
| 5 | `panic = "abort"` in release plus about 177 `unwrap`/`expect`/`panic!` in `src` (abort skips `Drop`, so zeroization does not run) | Narrowed: only the CLI aborts; the desktop app unwinds and does run `Drop`. Reachability of each panic from hostile input is not triaged yet. | [P7-4](#p7-4) |
| 6 | `check_kdf` uses `cfg!(test)` to skip validation | Checked: safe today, fragile | [P1-5](#p1-5) |

The primitives, parameters, nonce handling, AAD design and known-answer tests looked sound from the first read.

[Top](#top)

<a id="s4"></a>
## 4. Findings by phase


<a id="phase-1"></a>
### Phase 1 — Crypto core
Files read: all of `core/src/{crypto,vault,item,totp,generator}.rs`, the `argon2` 0.6.0 source in the cargo registry, and `core/Cargo.toml`. [P1-2](#p1-2) was measured with a temporary test that was deleted afterwards (nothing in the repo changed).


<a id="p1-1"></a>
#### P1-1 — The master password is used as raw bytes (Medium)

- **Issue:** The same password can be stored as different bytes inside the computer, depending on how it was typed. For example, é can be one character, or the letter e followed by a separate accent mark; many Hindi, Tamil, Telugu, Malayalam and Urdu letters can be written in more than one way too. The app feeds those raw bytes into its key-making step, so a password you typed correctly on another keyboard or another system can be rejected as "wrong". The fix is *normalisation*: converting text to one agreed standard form (NFKC, a Unicode standard) before use.
- **Severity:** Medium
- **How found:** From reading
- **Evidence:** `derive_kek` passes `password.as_bytes()` to Argon2. The same visible password can be different bytes: `é` as one code point or `e` plus a combining accent, and many Devanagari, Tamil, Telugu, Malayalam and Urdu sequences (composed or decomposed, depending on the input method and platform). Nothing in the repo normalises text.
- **Impact:** A password typed correctly on another keyboard, platform or future port gives "wrong password". The app offers seven languages, so non-Latin master passwords are a stated audience.
- **Recommended fix:** Normalise to NFKC before deriving keys. Because this changes the key for existing non-ASCII passwords, use a format flag: new vaults record `"pw_norm": "nfkc"` in the authenticated header, and unlock of older vaults tries the raw bytes first, then the normalised form. Add golden vectors for both. The strength meter and the generator use the same form.
- **Status:** fixed, 2026-10-27: new vaults and every password change record `"pw_norm":"nfkc"` in the authenticated header and key the vault from the NFKC form (`PwNorm`, `VaultHeader::unlock`). A vault from before the change tries the typed bytes first, then the NFKC and NFKD forms, and moves to the new form on its next password change. Tests `crates/core/tests/password_normalisation.rs`, including a golden header written by the earlier code. Known limit: an older vault whose password mixed composed and decomposed characters opens only with exactly those bytes. The strength meter only reads the typed text and needs no change.

[Top](#top)

<a id="p1-2"></a>
#### P1-2 — A tampered header makes unlock run a huge Argon2 before anything is authenticated (Medium)

- **Issue:** A vault file says in its header how much work (memory and time) the *Argon2* step must do when you unlock it. Argon2 is the deliberately slow, memory-hungry function that turns your master password into the encryption key, so that guessing passwords is expensive. The header is read before it can be checked for tampering, so someone who can edit the file can ask for an enormous amount of work (1 GiB of memory, about 11 seconds) and make every unlock attempt slow or crash a small machine. It cannot reveal your data; it only wastes your time and memory.
- **Severity:** Medium
- **How found:** Measured
- **Evidence:** `unlock` calls `check_kdf` and then `derive_kek` first; the header (including `kdf`) is authenticated only by the decryption that follows. `KdfParams::validate` allows `m_kib` up to 1 GiB, `t` up to 20, `p` up to 16. One derivation with `m = 1 GiB, t = 20, p = 1` took 10.8 s and 1 GiB (debug profile, optimised dependencies).
- **Impact:** Anyone who can write the vault file (sync peer, shared folder) makes each unlock attempt cost that, or fail with out-of-memory on a small machine. It reveals nothing: the header is bound as AAD, so a changed header never decrypts.
- **Recommended fix:** Lower the accepted ceiling to what the app can itself produce (the desktop slider tops out well below 1 GiB; for example 256 MiB and t ≤ 10), keep the floor, and show the parameters before an expensive derivation ("this vault asks for 1 GiB; continue?"). Change `Settings::validated` and the validator together.
- **Status:** fixed 2026-10-27. 2026-10-26: the ceiling is now 256 MiB and 10 passes (`KdfParams::MAX_*`, test `crates/core/tests/hostile_header.rs`). 2026-10-27: `rk` now says what an unusually costly vault asks for (`KdfParams::is_unusually_costly`, more than twice the default) before it asks for the password, and lets someone at the terminal cancel (test `an_unusually_costly_vault_is_announced_before_unlocking`). The desktop app shows the same note above the password box (`Session::unlock_hints`, `Model::costly_unlock_notice`; tests `unlock_hints.rs` and `a_costly_vault_is_announced_on_the_unlock_screen_and_a_normal_one_is_not`). It cannot ask "continue?" first, because the unlock button is the choice: pressing it is the continue, and not pressing it is the cancel. Fixed.

[Top](#top)

<a id="p1-3"></a>
#### P1-3 — `docs/04` promises things the code does not do (Medium)

- **Issue:** The design document (`docs/04`) describes protections that the code does not have yet: a *Secret Key* (a second long random secret, kept apart from your master password, that must be combined with it to open the vault), a *verifier* (a small check value used to test a password), a *magic* field (a fixed marker at the start of the file that says what kind of file it is) and *`mlock`* (a request to the operating system never to copy the program's memory to disk). A reader would trust a stronger design than the one that exists. Today, a stolen vault file is protected only by your master password and the cost of Argon2.
- **Severity:** Medium
- **How found:** Confirmed
- **Evidence:** The design doc describes a Secret Key (two-secret key derivation), a `verifier` blob, a header `magic` field and `mlock`ed memory. `VaultHeader` has `format_version`, `vault_id`, `kdf`, `salt`, `wrapped_vk` only; `MAGIC` is only mixed into the AAD; there is no Secret Key and no memory locking.
- **Impact:** A reader will believe a stolen vault file is protected by a high-entropy second secret. Today it is protected by the master password and the Argon2 cost alone.
- **Recommended fix:** Either mark these as planned and say what protects the vault today, or build them. The Secret Key matters most, because it changes what a weak master password costs. The wrong-password test is "AEAD open fails", which is fine but is also what the throttle in `Session::unlock` relies on (see P3).
- **Status:** fixed in part, 2026-10-27: the Secret Key is built and opt-in (`SecretKey`, header `secret_key`, KEK label `rk/kek/v2`; `rk init --secret-key`, `rk secret-key enable|show|disable`; the key is kept on the device in a 0600 file apart from the vault). Tests `crates/core/tests/secret_key.rs`, `crates/app/tests/secret_key.rs`, `crates/cli/tests/cli.rs`. `docs/04` now says there is no `verifier` (a failed unwrap does that job) and that `magic` is part of the AAD, and that `mlock` is not built (see P6-1). 2026-10-27 (later): the desktop unlock screen asks for a Secret Key when the vault needs one and the device has none, and keeps a key that worked on the device (`Session::unlock_hints`, `Model::unlock_with_key`; tests `a_key_that_worked_is_kept_on_the_device_...` and `a_secret_key_can_be_typed_on_the_unlock_screen`). Not done: creating a vault with a Secret Key, or adding one, from the desktop app (use `rk`); a Secret Key is not required by default.

[Top](#top)

<a id="p1-4"></a>
#### P1-4 — The Argon2 `zeroize` feature is off (Low)

- **Issue:** Argon2 (the slow password-to-key function) has an optional setting that wipes its temporary working values when it finishes. It is switched off here. One of those leftover values is a quick fingerprint of your password and salt; if someone obtains a copy of the program's memory while it is there, they can test password guesses against it far faster than against the full Argon2. The risk is small, because anyone who can read that memory can find your keys directly anyway.
- **Severity:** Low
- **How found:** Confirmed
- **Evidence:** `core/Cargo.toml`: `argon2 = { default-features = false, features = ["alloc"] }`. So `initial_hash` (line 389) and the `blockhash` temporaries (lines 570-573) are not wiped. The large memory matrix is not wiped even with the feature on.
- **Impact:** `initial_hash` is a plain Blake2b of password and salt; a memory image that contains it lets someone test guesses with one cheap hash instead of a full Argon2. Low, because an attacker who can read this memory also finds the key and the password buffers ([P6-3](#p6-3)).
- **Recommended fix:** Enable the `zeroize` feature (check it still builds for `wasm32`) and state the limit in `docs/04`.
- **Status:** fixed 2026-10-27: `argon2` is built with its `zeroize` feature (the wasm builds still pass). The large memory matrix is still not wiped; `docs/04` says so.

[Top](#top)

<a id="p1-5"></a>
#### P1-5 — `check_kdf` skips validation with `cfg!(test)` (Low)

- **Issue:** To keep unit tests fast, the code skips a safety check on the Argon2 settings when it is compiled for the library's own tests (`cfg!(test)` is a compile-time switch for "this is a test build"). Real builds still run the check, so nothing is wrong today. The risk is that a future build change could turn the check off by accident without anyone noticing.
- **Severity:** Low
- **How found:** Checked
- **Evidence:** `cfg!(test)` is true only when `rahasyakosh-core` itself is compiled for its own unit tests. Other crates' tests, integration tests, the fuzz crate and every release build validate normally.
- **Impact:** Safe today. Fragile: a core unit test can never reach the validation path through `unlock`, and a later build change (a test-only feature, `--cfg test`) could silently switch validation off.
- **Recommended fix:** Pass the check as an argument (`unlock_with(policy)`), or put the cheap parameters behind a `test-kdf` cargo feature that `deny` and CI keep out of releases.
- **Status:** fixed 2026-10-27: `check_kdf` always validates. The core unit tests now use the cheapest parameters the real validation accepts (`CHEAP`), and the test-only `KdfParams::INSECURE_TEST` is gone.

[Top](#top)

<a id="p1-6"></a>
#### P1-6 — Old secrets are dropped without being wiped (Low)

- **Issue:** The app is careful to *zeroise* secrets (overwrite them with zeros) when it is done with them. But in several places it replaces or copies a secret without wiping the old copy first, so the old password stays in the computer's memory until something else happens to overwrite it. For example, after you change a password, the previous one can linger in freed memory.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** `Item`, `Field` and `ItemVersion` are `ZeroizeOnDrop` (history included). But `set_field` (`f.value = value.into()`), `revert_to` for `title` and `notes`, and the UI `Form`/signal copies drop the old `String` as plain memory. `UnlockedVault::unlock` copies the key into a stack array before wrapping it in `Zeroizing`; the generator builds the password in a plain `Vec<char>`; `base32_decode` fills a plain `Vec<u8>`.
- **Impact:** After a password edit, the previous password stays in freed heap until overwritten.
- **Recommended fix:** Wipe before replacing (`f.value.zeroize()` first), build intermediate buffers in `Zeroizing`, construct keys directly inside `Zeroizing`.
- **Status:** fixed in part, 2026-10-27: `set_field` and `revert_to` wipe the text they replace; the vault key is built in place inside `Zeroizing` (no stack copy); the generator's character buffer and `base32_decode`'s buffer are `Zeroizing` (also on the error path). Not done: the UI `Form`/signal copies and the web view, which are tracked under [P6-3](#p6-3).

[Top](#top)

<a id="p1-7"></a>
#### P1-7 — No limit on the size of an item (Low)

- **Issue:** There is no maximum size for an item. You can save a huge note, or import a spreadsheet with one enormous cell, and the app accepts it. Later, the storage code refuses to read any file over 64 MiB, so that one oversized item can make the vault fail to open.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** Nothing in `core` or `app` caps an item. `pad` stores the length as `u32` (`data.len() as u32` would wrap above 4 GiB), and `DirectoryStore` refuses to read any file over 64 MiB (`MAX_FILE`).
- **Impact:** A too-large item (a huge note, an import of one huge row) is written without complaint and then makes the directory store unreadable at the next unlock (same all-or-nothing failure as [P2-3](#p2-3)).
- **Recommended fix:** Reject items above a fixed size (for example 1 MiB of plaintext) in `Vault::commit`, and make the store limit and the item limit agree.
- **Status:** fixed 2026-10-27: `seal_item` refuses an item above `MAX_ITEM_BYTES` (4 MiB serialised, history included; chosen so a record stays well under the stores' 64 MiB file limit) with `Error::ItemTooLarge`. A bulk import with one oversized row adds nothing and says why. Tests `crates/app/tests/item_size.rs`.

[Top](#top)

<a id="p1-8"></a>
#### P1-8 — A generated master passphrase can be very short (Low)

- **Issue:** `rk init --passphrase` makes a master passphrase from random words. If you ask for only 3 words, it makes one with about 39 bits of randomness (*bits of entropy* measure how many guesses an attacker would need; each bit doubles the count) and does not warn you, although it warns about weak typed passwords. 39 bits is weak for a master password. The default of 6 words is fine.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** `rk init --passphrase --words 3` generates about 39 bits (EFF large list, 12.9 bits a word) and skips the strength check that typed passwords go through. The default is 6 words (about 77 bits).
- **Impact:** A weak master passphrase that the tool itself produced and approved.
- **Recommended fix:** Require at least 5 or 6 words for a master passphrase, or show the bits and ask for confirmation.
- **Status:** fixed 2026-10-27: `rk init --passphrase` and `rk passwd --passphrase` refuse fewer than 5 words (about 64 bits). Test `a_short_generated_master_passphrase_is_refused`. `rk gen` is unchanged: a site password may be short on purpose.

[Top](#top)

<a id="p1-9"></a>
#### P1-9 — Generator and TOTP details (Info)

- **Issue:** This is a note, not a problem. The password generator and the two-factor code maker (*TOTP*, the 6-digit codes that change every 30 seconds) were checked. The generator is fair (no character is favoured) and the codes match the official test values. One small, harmless effect: for very short generated passwords the real randomness is slightly lower than the figure shown.
- **Severity:** Info
- **How found:** From reading
- **Evidence:** `uniform` is correct rejection sampling (`zone = u32::MAX - (2^32 mod n)`), so no modulo bias. The generator draws one character per selected class and fills the rest from the union before shuffling: slightly less entropy than "uniform over all strings that contain every class", negligible at length 20, visible below about 6 characters (`entropy_bits` is documented as an upper bound). TOTP matches RFC 6238 (Appendix B vectors for SHA-1, SHA-256, SHA-512), checks digits 6 to 8 and period > 0, secret is `ZeroizeOnDrop`, `otpauth://hotp` is refused.
- **Impact:** None to act on.
- **Recommended fix:** Note only.
- **Status:** n/a

[Top](#top)

**What held up (core)**
- Primitives: Argon2id, HKDF-SHA256, XChaCha20-Poly1305 from RustCrypto, with the RFC 5869 and XChaCha draft vectors in the tests. `random_bytes` takes an injected `CryptoRng`; nonces are 192-bit random, so reuse is not a practical risk.
- Key hierarchy: master key, then a KEK by HKDF with a domain label, which unwraps the vault key; every item has its own key (`HKDF(vk, "rk/item/v1" ‖ id)`).
- AAD: the header binds version, vault id, KDF parameters and salt (a changed header never opens); records bind magic, vault id, item id, revision and record version, and `open_record` checks the id inside the payload against the record id. Moving a record to another vault, item id or revision fails.
- `unpad` is bounds-checked and safe on 32-bit and WebAssembly; padding to 64 bytes hides exact lengths; all key types are `Zeroizing`.
- Search (`Item::matches`) never looks at concealed fields or notes; `Debug` for `Item`, `Field` and `ItemVersion` redacts concealed values, and a fuzz target asserts it.

[Top](#top)

<a id="phase-2"></a>
### Phase 2 — Storage and integrity
Files read: `store/{lib,directory,sqlite}.rs`, `app/vault.rs`. Four findings were reproduced with a temporary test (kept outside the repo, not committed); the rest are from reading the code.


<a id="p2-1"></a>
#### P2-1 — Rollback, deletion and resurrection are undetected (High)

- **Issue:** The vault is stored as many separate encrypted records, one per item. The app checks that each record is genuine, but not that it is the *latest* one, or that no record has been removed. So anyone who can write to the vault folder (a sync service, a shared drive, a person with access to it) can swap a record for an older copy, which silently brings back a password you already changed, or delete a record and the item just vanishes. This is called a *rollback*. The design document claims this is detected; it is not.
- **Severity:** High
- **How found:** Confirmed
- **Evidence:** `docs/04` lists "rollback detection (HLC + local high-water mark)" as a mitigation; nothing implements it. The record AAD binds `rev`, but an older record is still a valid record. PoC: edit an item to rev 2, restore the old `.rkr` file, unlock. It succeeds and shows rev 1 with the old content. Deleting a record file is equally invisible.
- **Impact:** A sync server or anyone with write access can silently revert a changed password to the old one, or remove items.
- **Recommended fix:** Keep per-vault state outside the synced folder (item id to highest rev seen, and a record count) and refuse or warn on regression. Consider one sealed manifest record (id-to-rev map plus a counter, authenticated under the vault key) so deletion is detectable on a fresh device.
- **Status:** fixed, 2026-10-27: every write also seals a *manifest* (item id to newest revision, plus a write counter) under the vault key. Unlock compares the store with it and reports a `RolledBack`, `Missing` or `Resurrected` item (`Vault::warnings()`); a device-local file (`HighWater`, `$XDG_STATE_HOME/rahasyakosh/seen.json`) catches a whole-vault rollback (`OlderThanSeen`) and a removed manifest. Tests `crates/app/tests/integrity.rs`. Cost: every write also seals the manifest, whose size grows with the item count (about 0.3 s per edit at 10,000 items in a debug build, 56 ms before). Limits: the warnings inform, they do not restore the newer version; a vault from before this change is trusted as it stands until its first write (trust on first use); a device that never opened the vault has no high-water mark; two devices writing the same vault would disagree (sync is not built yet).

[Top](#top)

<a id="p2-2"></a>
#### P2-2 — Tombstones are unauthenticated (High)

- **Issue:** When you delete an item, the app leaves a small marker file called a *tombstone*, so that sync tools know the item was deleted on purpose. The marker is plain text with no protection, so anyone who can write to the folder can create one for any item, saying it was deleted at an absurdly high revision number. The item then disappears from the list without a warning, and the app can never save that item again.
- **Severity:** High
- **How found:** Confirmed
- **Evidence:** `.rkt` files and the `tombstones` table hold a plain `{"rev": N}`. PoC: write `<id>.rkt` with rev `u64::MAX`. Unlock succeeds, the item is gone from the list with no warning, and writing that id again fails with `stale revision ... already at 18446744073709551615`.
- **Impact:** Anyone who can write the store can hide any item and make its id permanently unusable.
- **Recommended fix:** Seal tombstones under the vault key with the id and rev in the AAD; treat an unsealed or unverifiable tombstone as corrupt. Bound how far a rev may jump.
- **Status:** fixed, 2026-10-27: a tombstone counts only if the sealed manifest vouches for it (`gone`). Any other is reported as `ForgedTombstone` and removed (`Store::discard_tombstone`), so the item shows again and its id can be written. A purge seals the manifest first. Tests `crates/app/tests/integrity.rs`, conformance suite. Not done: the tombstone file itself is still unsealed, which only matters once sync trusts tombstones from other devices.

[Top](#top)

<a id="p2-3"></a>
#### P2-3 — One bad record makes the whole vault unopenable (Medium)

- **Issue:** Opening the vault used to be all-or-nothing: if even one stored record was damaged or unreadable (a half-synced file, a sync conflict, or a file someone planted), the whole vault refused to open until the user found and deleted that file by hand. This has since been fixed: the bad record is now skipped and reported, and the rest opens.
- **Severity:** Medium
- **How found:** Confirmed
- **Evidence:** `Vault::unlock_inner` uses `?` on every record. PoC: drop a `<uuid>.rkr` containing `{}` into the directory; unlock fails with `vault storage is corrupted`.
- **Impact:** A sync peer or a sync conflict can lock the user out of everything until they delete the file by hand.
- **Recommended fix:** Skip and report records that fail (a quarantine list the UI shows) and open the rest. A wrong password must still fail closed; that is decided by the header before any record is read.
- **Status:** fixed, 2026-10-26: `Vault::unlock` skips unreadable and unopenable records and lists them in `Vault::unreadable()`; the CLI prints their ids and the desktop app shows a notice (tests `crates/app/tests/hostile_store.rs`, `crates/store/tests/directory.rs`). A skipped record is also listed as missing by the manifest check (P2-1).

[Top](#top)

<a id="p2-4"></a>
#### P2-4 — Changing the password does not change the vault key, and the old header stays on disk (Medium)

- **Issue:** Your master password protects a *vault key*, and that vault key encrypts everything. Changing the master password only re-wraps the same vault key with the new password; the vault key itself stays the same. So if someone kept an old copy of the vault (a backup, sync history) and knows the old password, they can still open every later item. Also, the database's write-ahead log (a scratch file SQLite keeps for crash safety) can still hold a copy of the old header after a change.
- **Severity:** Medium
- **How found:** Confirmed (WAL part)
- **Evidence:** `change_password` re-wraps the same VK; there is no VK rotation, although `docs/04` mentions one. After a password change the SQLite `-wal` file still holds both header copies; `secure_delete` does not cover WAL frames.
- **Impact:** Someone with an old header (a backup, sync history) and the old password can still open every later record.
- **Recommended fix:** Offer VK rotation (re-encrypt all items) and say in the UI that a password change does not protect against a leaked old password plus an old copy of the vault. Checkpoint with `wal_checkpoint(TRUNCATE)` after `set_header`.
- **Status:** open

[Top](#top)

<a id="p2-5"></a>
#### P2-5 — Hardening gaps in the stores (Low)

- **Issue:** A group of small weaknesses in how the storage code touches files and the database: it does not turn off some database features that a vault file from a stranger could abuse (such as triggers, small programs stored inside the file), it follows *symlinks* (shortcut files that point elsewhere, so a planted one could point at a never-ending device such as `/dev/zero`), it checks that a file does not exist and then creates it in two steps (a race, where another program can slip in between), it builds some database commands from text, and it creates folders that other users can list. Each is minor on its own.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** (a) `SqliteStore::open` does not set `trusted_schema=OFF` or defensive mode, and trusts `user_version`. (b) `DirectoryStore::read` follows symlinks: `fs::metadata` reports a small length for a device such as `/dev/zero`, then `fs::read` reads without limit; the temp name `.{name}.tmp` is fixed and opened without `O_NOFOLLOW`. (c) `DirectoryStore::create` checks `exists()` then writes (a race). (d) `stored_rev(table)` and the public `SqliteStore::pragma(name)` build queries with `format!` (safe today: literals only). (e) `SqliteStore::open` creates parent directories with the default umask.
- **Impact:** A vault file received from someone else can carry triggers or views; a planted symlink in a synced folder can exhaust memory; directory listing is not private. All low impact.
- **Recommended fix:** (a) Set `trusted_schema=OFF` and defensive mode. (b) Use `symlink_metadata`, reject non-regular files, read with `take(MAX_FILE)`. (c) Create with `create_new`. (d) Make them private or use an enum. (e) Create folders with mode 0700 (see [P6-2](#p6-2)).
- **Status:** open

[Top](#top)

**What held up (storage)**
- Writes are rejected unless the rev is newer, in both backends; the conformance suite checks the same rules for each, which protects against two programs overwriting each other.
- `BEGIN IMMEDIATE` is used, so check-then-write cannot race.
- SQLite batches are atomic. Directory batches are not (documented), and each file is written via tmp + fsync + rename.
- SQLite file and sidecars are 0600 (confirmed for `-wal` and `-shm`); directory store files are 0600 and the directory 0700.
- Parameters are bound (no string-built values); stores never see keys or plaintext.
- Migrations refuse a database newer than the program.

[Top](#top)

<a id="phase-3"></a>
### Phase 3 — Session and lock
Files read: `app/session.rs`, and how `apps/desktop/src/{model,lib}.rs` drive it. Findings come from reading the code; none are reproduced yet. The profile index was left to Phase 6.


<a id="p3-1"></a>
#### P3-1 — A settings value of 0 days for the trash deletes every trashed item at the next unlock (Medium)

- **Issue:** The setting *trash retention days* says how long deleted items stay in the trash before being erased for good. It has an upper limit but no lower limit, and 0 means "erase immediately". The app erases expired trash right after every unlock, so if the plain settings file is set to 0 (by mistake, by a bad import or on purpose), all trashed items are permanently deleted at the next unlock, with no question asked.
- **Severity:** Medium
- **How found:** From reading
- **Evidence:** `Settings::validated` only caps `trash_retention_days` (max 3650), with no minimum. `purge_expired` computes `keep_for = 0`, so `now - d >= 0` holds for every trashed item, and `Session::unlock` runs it right after each successful unlock.
- **Impact:** A hand edit, a bad import or a malicious write of `"trash_retention_days": 0` in the plain `settings.json` permanently deletes the trash with no prompt and leaves tombstones.
- **Recommended fix:** Set a minimum of 1 day (or treat 0 as "never purge"). Ask in the UI before purging more than a few items.
- **Status:** open

[Top](#top)

<a id="p3-2"></a>
#### P3-2 — `settings.json` is written without care (Low)

- **Issue:** The settings file (`settings.json`) is saved carelessly: not in a way that survives a crash half-way, with permissions that let other users read it, and any error is ignored. If the file is damaged, the app quietly falls back to defaults. Anyone who can edit the file can also turn off the idle lock, the clipboard clearing and the reveal timeout (0 means off for all three).
- **Severity:** Low
- **How found:** From reading
- **Evidence:** `save_settings` uses `fs::write` with the default umask (likely 0644), is not atomic and ignores errors. `from_json` turns any parse error into the defaults.
- **Impact:** A half-written file silently resets the user's choices. Anyone who can write it can switch off idle lock, clipboard clearing and reveal timeout (`0` means off for all three), the same class as [P3-1](#p3-1).
- **Recommended fix:** Write via tmp + rename with mode 0600. Keep the previous file as `.bak` and tell the user when the file could not be read.
- **Status:** open

[Top](#top)

<a id="p3-3"></a>
#### P3-3 — Idle lock and throttle trust the wall clock (Low)

- **Issue:** The auto-lock and the wait after wrong passwords depend on the computer's clock. If the clock is set back, the idle timer never fires and the wait gets longer. This needs administrator rights, so the real risk is low. (A "monotonic" clock, one that only ever counts forward, is no better here because on Linux it stops while the computer is asleep.)
- **Severity:** Low
- **How found:** From reading
- **Evidence:** `now()` is `SystemTime` in both front-ends. A plain `Instant` is no better: on Linux `CLOCK_MONOTONIC` stops during suspend.
- **Impact:** Setting the clock back stops the idle lock from firing and lengthens the unlock wait. Setting it forward only locks early. Needs root or a clock-setting right, so the impact is low.
- **Recommended fix:** Take `max(wall elapsed, boottime elapsed)`.
- **Status:** open

[Top](#top)

<a id="p3-4"></a>
#### P3-4 — The unlock throttle is in memory only, and the re-auth check has none (Low)

- **Issue:** After a few wrong master passwords, the app makes you wait before trying again (*throttling*). That counter lives only in memory, so closing and reopening the app, or switching profile, resets it. The export and delete-profile dialogs also check the master password but do not count failures, so someone sitting at an unlocked screen could keep guessing there. Someone with a copy of the vault file is not slowed at all, because the throttle is in the app, not in the file.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** `failures` and `next_attempt_at` live in `Session`; restarting the app or opening a profile (`open_profile` builds a new `Session`) resets them. `Model::check_password` (export, delete profile) runs a full Argon2 check without counting failures.
- **Impact:** Someone at an unlocked screen can guess the master password without limit through the export and delete dialogs. An attacker with the file is not slowed by the throttle at all.
- **Recommended fix:** Say so in `docs/04`; route `check_password` through the same counter. Persist the counter only if the threat is a person at the keyboard.
- **Status:** open

[Top](#top)

<a id="p3-5"></a>
#### P3-5 — `peek()` does not check the idle time (Info)

- **Issue:** A note, not a problem. The "peek" function (a quick read of the unlocked vault) does not check how long you have been idle; it relies on a timer that ticks every second. If the screen is frozen for a while (for example while another profile is being unlocked), the lock may come a little late. This is acceptable.
- **Severity:** Info
- **How found:** From reading
- **Evidence:** `peek` gives read access until the next `tick`, at most one second away. If the UI thread blocks (for example on Argon2 during the unlock of another profile) the window is longer.
- **Impact:** Acceptable.
- **Recommended fix:** Keep the one-second tick as a requirement.
- **Status:** n/a

[Top](#top)

**What held up (session)**
- `lock()` drops the `Vault` (keys, items, index) and clears every revealed field; `revealed` holds only item ids and field names, never values.
- Idle lock applies on every user action (`vault(now)` ticks first) as well as on the timer. Activity from the future is capped (`last_input.min(t)`).
- The pending clipboard secret is `Zeroizing`, is returned on lock so the caller clears it, and is cleared only if the clipboard still holds exactly that value (`should_clear`).
- Lockout is checked before the password is tried, and only a real decryption failure counts as a wrong password (a corrupt file does not). Backoff arithmetic is capped (shift at most 20, wait at most 300 s) and cannot overflow.
- Key-derivation settings are clamped to the library's floor, so a settings file cannot weaken a new vault.
- The CLI does not use `Session`; it asks for the password per command (Phase 4).

[Top](#top)

<a id="phase-4"></a>
### Phase 4 — Clipboard and CLI
Files read: `clipboard/src/lib.rs`, `cli/src/{main,clipboard,file}.rs`. [P4-2](#p4-2) was reproduced with a throwaway vault (nothing in the repo changed); the rest are from reading the code.


<a id="p4-1"></a>
#### P4-1 — `RK_PASSWORD` outlives the command inside the clipboard helpers (Medium)

- **Issue:** For scripts, the command-line tool can read the master password from an *environment variable* (a named setting that every program started from it inherits), called `RK_PASSWORD`. When the tool starts a helper program to copy something to the clipboard or to clear it later, that helper inherits the variable too and keeps the master password in its environment for as long as it runs, up to the whole clipboard delay. The same user, or an administrator, can read it from `/proc` (a view of every running program's details).
- **Severity:** Medium
- **How found:** From reading
- **Evidence:** `read_password` takes the master password from `$RK_PASSWORD`. `spawn_clearer` (`cli/src/clipboard.rs`) and the `wl-copy`/`xclip`/`xsel` spawns (`clipboard/src/lib.rs`) never clear the environment, so every child inherits it.
- **Impact:** The detached helper holds the master password in its environment for the whole delay, and `xclip`/`wl-copy`, which stay alive to serve the clipboard, hold it until the clipboard is replaced. The same user and root can read it from `/proc/<pid>/environ`.
- **Recommended fix:** `env_remove("RK_PASSWORD")` (better `env_clear` plus `PATH`, `DISPLAY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`) on every spawn. Longer term offer `--password-file` or a file descriptor and describe `RK_PASSWORD` as scripting-only.
- **Status:** open

[Top](#top)

<a id="p4-2"></a>
#### P4-2 — `rk export -f vault --force` can overwrite the live vault (Medium)

- **Issue:** `rk export -f vault --force` is meant to write a backup copy. If you give it the live vault file as the output name, it overwrites the live vault with the backup format. The safety check compares the *text* of the two paths, so a different spelling of the same file (such as `./v.db` and `v.db`) gets past it. The vault is then unreadable until you run the `rk migrate` command to convert it.
- **Severity:** Medium
- **How found:** Confirmed
- **Evidence:** The guard `out == path` compares path text. PoC: `rk --vault ./v.db export -f vault -o v.db --force` exits 0 and `file::save` renames the JSON bundle over the open SQLite vault; the next `rk --vault v.db list` says the file is "in the old vault file format".
- **Impact:** The vault is unusable until the user works out `rk migrate` (the bundle holds the same records), and a `-wal` file may be left beside it. The desktop export has the same risk.
- **Recommended fix:** Compare with `same_file` (device and inode, or `canonicalize` of the parent plus file name) and refuse when the target is an existing SQLite vault.
- **Status:** open

[Top](#top)

<a id="p4-3"></a>
#### P4-3 — Secrets in the command line (Low)

- **Issue:** The command-line tool accepts the two-factor secret (the *TOTP seed*, which makes the 6-digit codes) and notes as part of the command typed on the line. Anything typed on a command line can be seen by other programs through `ps` (a list of running programs and their arguments) and is stored in the shell history file.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** `rk add --totp <secret>` and `rk edit --totp <secret>` take the TOTP seed as an argument; `--notes` the same. Passwords do not (`--password` is a switch, `--password-stdin` exists).
- **Impact:** The seed appears in `ps`, `/proc/<pid>/cmdline` and shell history.
- **Recommended fix:** Add `--totp-stdin` (or prompt); warn when `--totp` is given a value; same for notes.
- **Status:** open

[Top](#top)

<a id="p4-4"></a>
#### P4-4 — The clipboard clear is best effort and the messages promise more (Low)

- **Issue:** After you copy a password, the app says the clipboard will be cleared in 20 seconds. That is a best effort, not a promise: the small helper that does the clearing can be killed, the clipboard tool may be gone, two copies in a row can shorten the second wait, and *clipboard managers* (tools that remember everything you copied) keep their own history that the app cannot clear.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** (a) The helper prints nothing; if it is killed or the tool is missing at clear time the secret stays. (b) Two copies of the same value in a row: the first helper clears early, shortening the second. (c) Clipboard managers (GNOME, Klipper, `cliphist`) keep their own history and no "sensitive" hint is set. (d) The helper reads the secret with `read_to_string` into a plain `String`.
- **Impact:** The user was told "the clipboard clears in 20s" when that may not happen; the secret can also live on in a clipboard history or in freed memory.
- **Recommended fix:** Say "should clear" and mention clipboard managers in the docs and `docs/04`; let the helper write failures to a log or `$XDG_RUNTIME_DIR`; read stdin into a `Zeroizing<Vec<u8>>`.
- **Status:** open

[Top](#top)

<a id="p4-5"></a>
#### P4-5 — Clipboard tools are found through `PATH` (Low)

- **Issue:** To copy to the clipboard, the app runs tools such as `wl-copy` by bare name and finds them by searching the folders listed in `PATH` (the list of places where the system looks for programs). If an early folder in that list is writable by the user (such as `~/.local/bin`), another program running as the same user can put a fake `wl-copy` there and receive every password you copy.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** `on_path` checks `is_file` only (not executable) and the command runs by bare name. The re-spawn through `std::env::current_exe()` is not a hijack risk by itself: it uses `/proc/self/exe`, and replacing that binary already gives the attacker everything.
- **Impact:** A writable directory early in `PATH` (for example `~/.local/bin`) lets another program of the same user plant a `wl-copy` that receives every copied secret on stdin.
- **Recommended fix:** Look the tools up in `/usr/bin` and `/bin` first, or only accept tools owned by root; show the resolved path in a verbose mode.
- **Status:** open

[Top](#top)

<a id="p4-6"></a>
#### P4-6 — `rk passwd` with `RK_PASSWORD` set (Low)

- **Issue:** `rk passwd` changes the master password. If the `RK_PASSWORD` environment variable (see P4-1) is set, the tool reads the "new" password from the same variable, so the new password equals the old one, and the "type it again" prompt is skipped. The command says "Master password changed" but changed nothing. It is misleading, but nothing leaks.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** `new_master` reads the new password through the same `read_password`, so with the variable set the "new" password equals the current one and the repeat prompt is skipped.
- **Impact:** The command reports "Master password changed" and changes nothing: a silent no-op that looks like success. Not a leak.
- **Recommended fix:** Refuse `rk passwd` when `RK_PASSWORD` is set, or read the new password from another source.
- **Status:** open

[Top](#top)

<a id="p4-7"></a>
#### P4-7 — Plaintext export file handling (Low)

- **Issue:** When you export your passwords as plain text, the file is meant to be private (readable only by you). With `--force` on a file that already exists, the tool keeps that file's old permissions, which may let others read it, and it follows a *symlink* (a shortcut file) at the target, so the export can be written to a different file than the one you named.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** `write_private` opens with mode 0600 only when it creates the file. With `--force` on an existing file it keeps that file's old mode and follows a symlink at the target. Without `--force`, `create_new` is correct.
- **Impact:** A plaintext export can end up world-readable, or be written through a symlink to another file.
- **Recommended fix:** For `--force`, write a new temp file (0600) in the same directory and rename it over the target, as `file::save` does.
- **Status:** open

[Top](#top)

<a id="p4-8"></a>
#### P4-8 — Read-only commands delete data (Info)

- **Issue:** A note. Every command, even `rk list` that only shows items, quietly erases trash items older than 30 days first. The 30 days is a default for the command line and may not match what you set in the desktop app. It does print a message when it does so.
- **Severity:** Info
- **How found:** From reading
- **Evidence:** `open_vault` calls `purge_expired` with the default 30-day retention for every command, using CLI defaults instead of the desktop settings.
- **Impact:** `rk list` can permanently delete old trash. It already prints a line.
- **Recommended fix:** Do it only in commands that already write, and tell the user.
- **Status:** open

[Top](#top)

**What held up (clipboard and CLI)**
- Secrets go to the clipboard tools on standard input, never as arguments; the tools' arguments are fixed strings.
- The master and item passwords are read with echo off (`rpassword`, from the terminal) and held in `Zeroizing`. Plaintext exports ask for the password again on the terminal even when `RK_PASSWORD` is set, and print a warning first.
- Files for exports are created `create_new` + 0600; vault bundles are written to a temp file, fsynced and renamed, 0600.
- The clipboard is cleared only when it still holds exactly the copied value, and a copy is cancelled and cleared if the helper cannot start.
- `get` prints only the requested field; `import` prints title, user and URL, never passwords; error messages do not include secrets.
- No `unsafe` is needed; the `cli` and `clipboard` crates have no `forbid(unsafe_code)` attribute, so add it to make that a rule.

[Top](#top)

<a id="phase-5"></a>
### Phase 5 — Import and export
Files read: `transfer/src/{lib,bundle}.rs`, the import and export commands in `cli/src/main.rs`, `fuzzcheck/src/lib.rs`, `fuzz/`, `.github/workflows/fuzz.yml`. [P5-1](#p5-1), [P5-2](#p5-2) and [P5-3](#p5-3) were reproduced with the debug `rk` binary and files in a scratch folder (nothing in the repo changed). Timings are from a debug build.


<a id="p5-1"></a>
#### P5-1 — CSV import time grows with the square of the row count (Medium)

- **Issue:** Importing a CSV (a spreadsheet saved as plain text) compares each new row with all rows seen so far to find similar logins. That makes the work grow with the *square* of the row count: 20 000 rows took about 5 seconds, and 40 000 about 26 seconds. The size cap allows about 500 000 rows, so a large or hostile file can freeze the import for a very long time with no progress shown.
- **Severity:** Medium
- **How found:** Confirmed
- **Evidence:** For every new row `import_csv` scans the whole `seen` set (`seen.iter().any(...)`) to count similar logins: O(rows²). `rk import big.csv --dry-run`: 5.2 s for 20 000 rows, 25.7 s for 40 000.
- **Impact:** The 50 MiB cap allows about 500 000 short rows, so a hostile or merely large file can hang the CLI and the desktop import for a very long time, with no progress shown.
- **Recommended fix:** Index `seen` by (title, username, url), for example `HashMap<(String, String, String), HashSet<String>>` of passwords, so the check is one lookup. Add a large-file timing test.
- **Status:** open

[Top](#top)

<a id="p5-2"></a>
#### P5-2 — Imported text can carry terminal escape sequences (Medium)

- **Issue:** Imported text (titles, usernames, web addresses) is printed to the terminal exactly as it is. A terminal obeys hidden *escape sequences*, special character codes, such as "clear the screen" or, in some terminals, "put this text on the clipboard" (the code called OSC 52). So a crafted CSV could make the import preview look different from what is really in the file, or change your clipboard. The same applies to items that arrive through sync or sharing.
- **Severity:** Medium
- **How found:** Confirmed
- **Evidence:** `rk import --dry-run`, the confirmation listing and `rk list` print titles, usernames and URLs with `println!` unchanged. A CSV title holding `ESC ] 52 ; c ; … BEL` or `ESC [ 2 J` reaches the terminal as written.
- **Impact:** Terminals that allow OSC 52 let it write to the clipboard; others let it rewrite the screen, so the user may confirm an import that does not look like what is in the file. Items arriving by sync or sharing behave the same.
- **Recommended fix:** Print every untrusted string through one function that replaces control characters (`ESC`, `BEL`, C1 codes) in `list`, `trash`, `import`, `history` and error messages. `get` keeps writing raw to a pipe but escapes when stdout is a terminal.
- **Status:** open

[Top](#top)

<a id="p5-3"></a>
#### P5-3 — The JSON export includes the history of old passwords (Low)

- **Issue:** The JSON export includes each item's *history*, which holds the old passwords you replaced. The warning only says the file contains "every password". If you changed a leaked password, the export still carries the leaked one.
- **Severity:** Low
- **How found:** Confirmed
- **Evidence:** `export_json` serialises each `Item`, history included. After `rk edit H --password-stdin`, `rk export -f json` contains the old password as well as the new one. The `txt` and `csv` exports do not.
- **Impact:** "Complete" is the documented intent, but the warning only says "every password in it". A user who changed a leaked password still exports the leaked one.
- **Recommended fix:** Say so in the export warning and the book, or leave history out unless asked (`--with-history`).
- **Status:** open

[Top](#top)

<a id="p5-4"></a>
#### P5-4 — CSV export writes cells verbatim (Low)

- **Issue:** In the CSV export, a cell that starts with `=`, `+`, `-` or `@` is treated by spreadsheet programs as a *formula*, a small program, such as one that opens a web link. If an imported item holds such text, opening your export in a spreadsheet can run it. The code comment mentions this, but the messages shown to the user do not.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** A title, note or username beginning with `=`, `+`, `-` or `@` becomes a formula when opened in a spreadsheet (for example `=HYPERLINK(...)`, or a DDE call in old versions). The doc comment warns, but the CLI and desktop messages do not.
- **Impact:** Imported items are the likely source of such values; opening the export in a spreadsheet can run attacker formulas.
- **Recommended fix:** Add the warning to the export message, or offer `--spreadsheet-safe` which prefixes `'` (not default: it breaks round trips into other managers).
- **Status:** open

[Top](#top)

<a id="p5-5"></a>
#### P5-5 — Whole files are read before the size limit is checked (Low)

- **Issue:** When you import a file, the app first reads the whole file into memory and only then checks whether it is over the size limit (50 MiB for CSV, 256 MiB for a bundle). A giant file, or a special file such as `/dev/zero` that never ends, can use up all memory before the check runs. This would normally be a mistake by the user, not an attack, because the user names the file.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** `rk import` and `file::load` call `std::fs::read` and only then compare with `MAX_IMPORT_BYTES` (50 MiB) or `MAX_BUNDLE_BYTES` (256 MiB).
- **Impact:** `/dev/zero` or a very large file exhausts memory first. A mistake rather than an attack, since the user names the file.
- **Recommended fix:** `File::open` + `take(MAX + 1)` + `read_to_end`, and reject non-regular files.
- **Status:** open

[Top](#top)

<a id="p5-6"></a>
#### P5-6 — Imported data is not zeroised once parsed (Low)

- **Issue:** The raw imported file is wiped from memory after use, but the parsed copies made by the CSV reader are not. Passwords from an import can stay in freed memory afterwards. The file on disk is the bigger exposure; the tool already tells you to delete it.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** The input bytes are `Zeroizing`, but the `csv` reader's buffers and `StringRecord`s are plain memory, and `Item::login(...)` copies each value.
- **Impact:** Passwords from a CSV remain in freed memory after the import. The file on disk is the bigger exposure (the tool already says to delete it).
- **Recommended fix:** Include in the memory-hygiene work ([P6-3](#p6-3), [P1-6](#p1-6)).
- **Status:** open

[Top](#top)

<a id="p5-7"></a>
#### P5-7 — Text export labels (Info)

- **Issue:** A note. In the plain-text export, long multi-line values are indented so a note cannot pretend to start a new item. But a custom field's *name* is written as it is, and could contain a line break. The effect is cosmetic, because the file is meant for people to read.
- **Severity:** Info
- **How found:** From reading
- **Evidence:** `export_txt` indents multi-line values so a note cannot forge a new block, but custom field names are written without escaping and can contain newlines.
- **Impact:** Cosmetic: the file is for people to read.
- **Recommended fix:** Escape or reject newlines in field names.
- **Status:** open

[Top](#top)

<a id="p5-8"></a>
#### P5-8 — The `csv_import` fuzz target never passes `existing` items (Low)

- **Issue:** *Fuzzing* means feeding a program huge numbers of random or damaged inputs to see if it breaks. The fuzz test for CSV import never supplies any existing items, so it never reaches the slow part described in P5-1. Its small default input size (4 KiB) is also too small to show a slowdown.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** libFuzzer's default input size (4 KiB) also cannot show [P5-1](#p5-1).
- **Impact:** The quadratic behaviour in [P5-1](#p5-1) cannot be found by fuzzing.
- **Recommended fix:** Derive an `existing` set from the input and add a timing assertion on a larger generated file.
- **Status:** open

[Top](#top)

<a id="p5-9"></a>
#### P5-9 — The `bundle` fuzz target stops at the store (Low)

- **Issue:** The fuzz test for vault bundle files checks that reading them is safe, but it never goes on to unlock the vault from them. The unlock step is where a bad record used to block everything (P2-3), so fuzzing could not have found it.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** It never calls `Vault::unlock` on the records, which is where [P2-3](#p2-3) lives.
- **Impact:** A malformed record that blocks unlock is not found by fuzzing.
- **Recommended fix:** Add an unlock check using the cheap key settings.
- **Status:** open

[Top](#top)

<a id="p5-10"></a>
#### P5-10 — Several readers of untrusted input are not fuzzed (Low)

- **Issue:** Several places that read untrusted input have no fuzz test: the settings file, the profile list file, the text and JSON exports, and the command-line query and column-mapping options. A bug in any of them would go unnoticed.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** Not fuzzed: `Settings::from_json` (a user-writable file; see [P3-1](#p3-1)), the profile index file parser, `export_txt`/`export_json` with arbitrary items, the CLI's `--map` and query parsing.
- **Impact:** Bugs in these parsers go unnoticed.
- **Recommended fix:** Add small targets.
- **Status:** open

[Top](#top)

<a id="p5-11"></a>
#### P5-11 — Only `item_json` has a `regressions` directory (Low)

- **Issue:** A *regression* is a bug that comes back after it was fixed. Only one fuzz test has a folder for saving inputs that once crashed the program (a `regressions` directory). For the others, a fixed crash could return and nobody would notice.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** Other targets have no place for saved crash inputs.
- **Impact:** A fixed crash can come back unnoticed.
- **Recommended fix:** Keep every future crash input in a `regressions` directory and replay it in the normal test run.
- **Status:** open

[Top](#top)

Fuzz targets today: `vault_header`, `record`, `payload`, `item_json`, `csv_import`, `totp`, `bundle`, `directory_store`, `sqlite_store`, `strength`. Seeds are replayed in the normal test run (`fuzzcheck/tests/replay.rs`) and the weekly workflow runs all ten.

**What held up (import and export)**
- Plaintext exports (csv, json, txt) ask for the master password again, warn first, and are returned as `Zeroizing` strings; encrypted bundle export needs no re-auth because it stays encrypted.
- `import_csv` and `Bundle::from_slice` are total (error or value, never a panic) and are fuzzed, with a round trip: what we export, we can import. Passwords, usernames and notes are kept exactly as in the file; warnings never contain secrets (tested).
- Both size limits exist (50 MiB CSV, 256 MiB bundle); `serde_json` limits nesting depth.
- `rk migrate` imports into a new file, proves it opens with the password, and removes the half-built file on failure. The CLI never imports straight into an existing vault without confirmation (`--yes` skips it, `--dry-run` stops before).
- Trashed items are left out of every export.

[Top](#top)

<a id="phase-6"></a>
### Phase 6 — Desktop UI
Files read: `apps/desktop/src/{lib,model,profiles}.rs`, `platform/linux.rs`, `crates/app/src/profiles.rs`, packaging. [P6-2](#p6-2) and [P6-5](#p6-5)'s directory listing were checked on the auditor's real data folder (file modes and names only; no contents were read). The rest is from reading the code. No GUI session was driven, so [P6-5](#p6-5) and [P6-8](#p6-8) still need a run.


<a id="p6-1"></a>
#### P6-1 — No process hardening: core dumps, ptrace, swap (Medium)

- **Issue:** The app does not use any of the operating system's tools to limit what can be extracted from a running program. If the program crashes, the system may write a *core file* (a snapshot of its memory) or send a crash report that contains the vault key and your decrypted items. Memory can also be copied to the *swap* (the disk space used when RAM is full), and other programs of the same user may be allowed to attach to it with `ptrace` (a debugging feature that can read another program's memory). The design document promises locked memory; there is none.
- **Severity:** Medium
- **How found:** From reading
- **Evidence:** Nothing in the repo calls `prctl`, `setrlimit`, `mlock` or `madvise`, although `docs/04` promises locked memory.
- **Impact:** A crash (the CLI release profile uses `panic = "abort"`) can write a core file or an Ubuntu crash report with the vault key and decrypted items in it; pages can be swapped out; same-user processes can attach with `ptrace` where `ptrace_scope` allows.
- **Recommended fix:** At start-up set `RLIMIT_CORE` to 0 and `PR_SET_DUMPABLE` to 0 through a safe wrapper (`rustix`) so the crates keep `forbid(unsafe_code)`. `mlock` needs `unsafe`: put it in one small audited module, or change the design doc to say it is not done.
- **Status:** open

[Top](#top)

<a id="p6-2"></a>
#### P6-2 — The data and config folders are readable by other users (Medium)

- **Issue:** The folders where the app keeps its data and settings can be listed by other users on the same computer, and the profile list and settings files can be read by them. The vault files themselves are private. The profile list holds each profile's real full name and username, and shows how many vaults exist.
- **Severity:** Medium
- **How found:** Confirmed
- **Evidence:** `~/.local/share/rahasyakosh` is mode 775, `profiles/` 775, `profiles.json` 664, `~/.config/rahasyakosh/settings.json` 664 (vault files and `-wal`/`-shm` are 600). `profiles::save`, `save_settings` and `create_dir_all` use the default umask.
- **Impact:** Any other local user can list the folders, read `profiles.json` (every profile's real full name and username) and see how many vaults exist.
- **Recommended fix:** Create folders with mode 0700, write both files 0600 via temp file and rename, and tighten existing installs at start-up. This also settles [P2-5](#p2-5) (parent directories) and [P3-2](#p3-2).
- **Status:** open

[Top](#top)

<a id="p6-3"></a>
#### P6-3 — Typed secrets pass through memory that is never wiped (Medium)

- **Issue:** Whatever you type into a password box passes through places the app cannot wipe: the web view (the built-in browser that draws the window), the messages sent between the window and the program on each key press, and ordinary text variables that are dropped without being overwritten. So the master password and other typed secrets stay in memory after use. This cannot be fully fixed while the window is a web view.
- **Severity:** Medium
- **How found:** From reading
- **Evidence:** Unwiped places: the `<input>` inside the WebKit web process; the event messages Dioxus sends on each keystroke (the whole value each time); the `String`s in the signals `pw`, `pw2`, `new_pw`, `new_pw2`, `export_pw`, `del_pw` and the edit-form fields. `submit()` ends with `pw.set(String::new())`, which drops the old `String` without wiping it. Revealed values are copied into `FieldView` and page text. `Form` wipes only its own copy.
- **Impact:** The master password and other typed secrets stay in memory after use. This cannot be fully fixed in a web view.
- **Recommended fix:** Keep typed values in `Zeroizing<String>`, take and wipe them after use, clear the input element after every submit (done only for the unlock box), and say plainly in `docs/04` that the web process is outside the wipe guarantee.
- **Status:** open

[Top](#top)

<a id="p6-4"></a>
#### P6-4 — The edit form shows the password in clear (Low)

- **Issue:** In the form for adding or editing an item, the password field shows the password as plain text, not as dots, and it never hides again while the form is open. The item detail page hides secrets until you ask, so the form is inconsistent. Anyone looking at your screen, or at a screen share, can read it.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** `f-pw` in the add/edit form is a normal text input, not `type=password`, with no reveal timeout. The detail pane hides secrets until revealed, so this is inconsistent.
- **Impact:** The password stays visible as long as the form is open (screen sharing, shoulder surfing, accessibility tools).
- **Recommended fix:** Mask by default with a show/hide button that follows `reveal_secs`. Set `spellcheck="false"` on the secret and TOTP boxes.
- **Status:** open

[Top](#top)

<a id="p6-5"></a>
#### P6-5 — The web view keeps its own files on disk (Low)

- **Issue:** The built-in web view (the browser engine that draws the window) creates its own folders of cache and storage on disk. The app does not use browser storage, so little should be there, but nobody has checked whether any text you typed ends up in those files.
- **Severity:** Low
- **How found:** Checked (partly)
- **Evidence:** `~/.local/share/rahasyakosh-desktop` (`storage`, `mediakeys`, `hsts-storage.sqlite`) and `~/.cache/rahasyakosh-desktop` (`CacheStorage`, `WebKitCache`) exist. The page is local and the code uses no browser storage, so little should be there, but contents were not verified.
- **Impact:** Possible leftovers of typed text on disk.
- **Recommended fix:** Type a sentinel into the master, password and notes boxes, quit, and search both folders. If anything is found, use an ephemeral web-view data folder (`with_incognito`, or a directory under `$XDG_RUNTIME_DIR`).
- **Status:** open

[Top](#top)

<a id="p6-6"></a>
#### P6-6 — Anything that reads the accessibility tree sees revealed values (Low)

- **Issue:** Screen readers such as Orca, and any other program allowed to use the *accessibility tree* (the structured description of the window that assistive technology reads), can read everything shown on screen. That includes revealed passwords and the current two-factor code. This is a deliberate trade-off to make the app usable by blind people, so it is documented rather than "fixed".
- **Severity:** Low
- **How found:** From reading (by design)
- **Evidence:** Revealed secrets, the live two-factor code and the edit form values are page text; the code of the selected item is shown without a reveal step.
- **Impact:** Any program using AT-SPI (a screen reader, but also any allowed program) can read them. This is a trade-off for screen-reader users, the project's stated priority, not a bug.
- **Recommended fix:** Document it, and consider hiding the code behind the same reveal timeout.
- **Status:** open

[Top](#top)

<a id="p6-7"></a>
#### P6-7 — Switching profile resets the unlock throttle (Low)

- **Issue:** After wrong passwords, the unlock wait grows (see P3-4). Choosing another profile builds a new session, which resets that counter. Someone at the keyboard can try profile A, switch to B and back, and A's counter starts again at zero.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** Choosing a profile calls `open_profile`, which builds a new `Session` (adds to [P3-4](#p3-4)).
- **Impact:** Someone at the keyboard can go from profile A to B and back to A and A's counter starts at zero.
- **Recommended fix:** Keep failure counters in the app, keyed by profile ID, for as long as the program runs.
- **Status:** open

[Top](#top)

<a id="p6-8"></a>
#### P6-8 — To check in a release build (Info)

- **Issue:** A note, and a to-do. Developer tools (the "Inspect Element" panel) are on in debug builds of the toolkit the desktop app uses. The audit found nothing to worry about in the code, but nobody has yet checked that they are really off in the release build.
- **Severity:** Info
- **How found:** Not run
- **Evidence:** Dioxus enables developer tools in debug builds. The code has no `dangerous_inner_html`, no links and no external requests.
- **Impact:** Practical risk is small.
- **Recommended fix:** Confirm right-click gives no "Inspect Element" and the inspector cannot be opened in the release build and the packaged binary; confirm whether a content-security policy is set.
- **Status:** open

[Top](#top)

**What held up (desktop UI)**
- **No injection surface.** Item text is rendered through Dioxus (escaped); no `dangerous_inner_html`, no `href`/navigation, so a hostile title or note cannot run script or open a page. The `eval` calls interpolate only enum-derived constants and static element IDs.
- **Profile paths.** `vault_file` rejects any `file` in `profiles.json` that is absolute or contains `..`, with a test, so a tampered list cannot make "delete profile" remove files outside the data folder. Deleting a profile and exporting both ask for the master password.
- **Lock clears the screen.** `forget_screen` drops the form, selection, query and revealed fields, and the clipboard secret is cleared on lock and on close.
- **No secrets in announcements.** Screen reader messages use field labels, never values; `Debug` for `Form` and `Item` is redacted.
- **Export** refuses to replace a file, creates it 0600, and re-checks the password (but see [P3-4](#p3-4) for the missing throttle).
- The profile index is documented as holding no secrets; it does hold real names, which is why [P6-2](#p6-2) matters.

[Top](#top)

<a id="phase-7"></a>
### Phase 7 — Supply chain and release
Files read: `.github/workflows/{ci,fuzz}.yml`, `deny.toml`, `apps/desktop/deny.toml`, both `[profile.release]` sections, `apps/desktop/install-local.sh`, `packaging/`, `SECURITY.md`, `docs/12-publishing-ubuntu.md`. `cargo deny check` and `cargo audit` were run in `apps/desktop`. The latest release tags were looked up with `gh`.


<a id="p7-1"></a>
#### P7-1 — The installer switches the desktop accessibility bus on for the whole session (Medium)

- **Issue:** To make the Orca screen reader work, the installed launcher switches on the desktop's *accessibility bus* (the system through which programs talk to screen readers) every time the app starts, if it was off. This changes a setting for the whole login session, not just this app. With the bus on, any program in the session can read other programs' text and watch key presses, which includes passwords you type elsewhere.
- **Severity:** Medium
- **How found:** From reading
- **Evidence:** The launcher written by `install-local.sh` sets `org.a11y.Status IsEnabled` to true at every start when it is off, so that Orca works (the project's priority).
- **Impact:** A system-wide change made by a password manager. With the bus on, any program in the login session can use AT-SPI to read other programs' text and watch key events, widening the keystroke-observation surface for everything the user types, including master passwords in other programs.
- **Recommended fix:** Do not change the setting silently: ask once (or only when a screen reader is running, which the app already detects), say what it does in the book and install message, and offer `--no-a11y-bus`. See [P6-6](#p6-6).
- **Status:** open

[Top](#top)

<a id="p7-2"></a>
#### P7-2 — GitHub Actions are pinned by tag, not by commit (Low)

- **Issue:** The project's automatic build and test service (*GitHub Actions*, "CI") uses ready-made helper steps from other people. They are named by a *tag* (like `v2`) that the owner can move to different code at any time, rather than by an exact *commit* (a fixed snapshot). If one of those helpers were hijacked, it would run inside the project's build. The default access it gets is read-only, which limits the damage.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** Moving tags: `actions/checkout@v7`, `actions/upload-artifact@v7`, `Swatinem/rust-cache@v2`, `EmbarkStudios/cargo-deny-action@v2`, `taiki-e/install-action@v2`, `bytecodealliance/actions/wasmtime/setup@v1`, and `dtolnay/rust-toolchain@stable`/`@nightly` (a branch name). `ci.yml` has no `permissions:` block (`fuzz.yml` does). The default token is read-only, which limits the harm.
- **Impact:** A compromised third-party action runs with the repository's token and the build tools.
- **Recommended fix:** Pin third-party actions to full commit SHAs (keep the tag in a comment, let Dependabot update them), add `permissions: contents: read` to `ci.yml`, pin tools installed on the fly (`cargo-audit`, `cargo-deny`, `cargo-fuzz`; `wasmtime` is already pinned).
- **Status:** open

[Top](#top)

<a id="p7-3"></a>
#### P7-3 — Builds are not locked or reproducible yet (Low)

- **Issue:** Someone who downloads the app cannot yet check that it is the genuine one. The builds do not all insist on the exact library versions in the lock file (`--locked`), the compiler is "whatever is latest", and there is no release process that publishes a *checksum* (a fingerprint of the file), a *signature* (proof of who made it) or an *SBOM* (a list of everything inside the app). This is on the to-do list in `docs/12`.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** The root jobs run `cargo clippy/test` without `--locked` (the desktop job uses it); the toolchain is "latest stable"; there is no release workflow, checksum, signature or SBOM yet (all listed as to-do in `docs/12`). `install-local.sh` uses `--locked`.
- **Impact:** Users cannot check that a download is the genuine, untampered build.
- **Recommended fix:** Add `--locked` to all root jobs, pin the toolchain in `rust-toolchain.toml`, and build releases in a workflow that publishes SHA-256 sums and a signature (`cosign` keyless), with `cargo auditable` or an SBOM, as `docs/12` plans.
- **Status:** open

[Top](#top)

<a id="p7-4"></a>
#### P7-4 — Release profile differs between the two workspaces (Low)

- **Issue:** The command-line tool and the desktop app are built with different settings. The command-line tool stops the program immediately on a crash (`panic = "abort"`), which skips the code that wipes secrets from memory. The desktop app does run that cleanup. Neither turns on *overflow checks*, which catch arithmetic that goes past the largest number a variable can hold.
- **Severity:** Low
- **How found:** From reading
- **Evidence:** The root `[profile.release]` sets `panic = "abort"` (applies to `rk`). `apps/desktop` has `lto = "thin"` and `strip` only, so it unwinds on panic and does run `Drop` (and zeroization). Neither profile sets `overflow-checks = true`.
- **Impact:** The abort/`Drop` concern (lead 5) is about the CLI, not the desktop app. Missing overflow checks are a risk for size and time arithmetic on untrusted values.
- **Recommended fix:** State the choice in the book, and enable `overflow-checks` for release in both.
- **Status:** open

[Top](#top)

<a id="p7-5"></a>
#### P7-5 — Deny policy and advisory exceptions (Info)

- **Issue:** A note. The project checks its dependencies (other people's code) with `cargo deny` (a rules checker) and `cargo audit` (a check against known security warnings). Both pass. A few warnings are accepted, each with a written reason. One of them may be out of date and should be removed when no longer needed.
- **Severity:** Info
- **How found:** From reading
- **Evidence:** Root policy is strict (no OpenSSL, `ring` banned, wildcards denied, unknown registries and git sources denied) and passes. The desktop policy allows OpenSSL only through `native-tls`, each exception has a written reason, and it passes. `RUSTSEC-2026-0097` (`rand` 0.7.3 unsound with a custom logger) is an accepted warning whose reason holds (no `tracing-subscriber` or `dioxus-logger` in the tree); `cargo deny` reports that ignore as "no crate matched", so it may be stale for deny while `cargo audit` still shows it. `cargo audit` in the desktop reports 5 allowed warnings (unmaintained or unsound crates inside the GUI toolkit), all listed in `deny.toml`.
- **Impact:** None now.
- **Recommended fix:** Check and remove stale ignores whenever dependencies change.
- **Status:** open

[Top](#top)

<a id="p7-6"></a>
#### P7-6 — Install script details (Info)

- **Issue:** A note. The install script only changes files under `~/.local` and does not touch your vault when uninstalling. It does build the launcher file with text substitution that breaks if your home folder name contains a `|` or `&`. The effect is cosmetic.
- **Severity:** Info
- **How found:** From reading
- **Evidence:** It touches only `~/.local`, builds with `--locked`, installs with explicit modes, and `--uninstall` leaves the vault alone. The `.desktop` file is made with `sed` using the pipe character as delimiter, which breaks for a home directory containing it or `&`; the launcher embeds `$bin` in double quotes.
- **Impact:** Cosmetic.
- **Recommended fix:** Escape the substituted value.
- **Status:** open

[Top](#top)

**What held up (supply chain and release)**
- Workflows use `pull_request`, not `pull_request_target`, so forks cannot reach secrets; the fuzz workflow runs with `contents: read`; no secrets are used anywhere in CI.
- `Cargo.lock` is committed for both workspaces; the desktop job builds `--locked`.
- `SECURITY.md` gives a private reporting channel, a response time and a scope, and states the project is unaudited.
- The plan in `docs/12` (checksums, signing, sandbox testing with Orca and the clipboard before a snap) covers the right topics.

[Top](#top)

<a id="s5"></a>
## 5. Not covered, and what to do next
- Out of scope (see the plan): the code of third-party dependencies, the operating system, WebKit and the desktop environment, hardware attacks, a machine that already runs malware as the user, and clients that do not exist yet.
- Not run: [P6-5](#p6-5) and [P6-8](#p6-8) need a GUI session; most "from reading" findings have a manual check in the test checklist.
- Recommended before any claim of being "audited": an outside review of the crypto design and of the sync and rollback design.

[Top](#top)


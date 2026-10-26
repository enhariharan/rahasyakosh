# 13 — Security audit plan

Audit started 2026-10-26. Status key: `todo`, `in progress`, `done`.

## Purpose
Rahasyakosh is a password manager for the user's passwords. A flaw in it costs the user far more than a flaw in most other programs. This audit is a structured, written look at the whole project to:
- Find those flaws **before** the app is published and relied on
- Leave a record of what was checked, what was found and what held up.
It does not prove the app is secure. However, it lowers the chance that obvious and subtle mistakes ship.

### Why is this audit necessary
- The app's promise is that a copied vault file, a hostile sync folder or a hurried user does not lose their passwords. That promise must be actually tested.
- `docs/04` may describe more than the implemented code (Secret Key, `verifier`, `mlock`, rollback detection). It must be compared and assured that users are not told something untrue (P1-3).
- Readers of this project may be newcomers. Intended users may not be technical. They cannot judge the security by themselves, so the project has to.
- `SECURITY.md` says the project has not been audited. This plan is the first step towards closing that gap.
- Fixing is cheaper now than after release.

### What is audited
- A review of the source, build and release setup against the threats in `docs/04-security-and-crypto.md`.
- It covers seven areas detailed in `Phases` below - the crypto core, storage and integrity, session and lock, clipboard and CLI, import and export, the desktop UI, and the supply chain and release.
  - For each area it asks the questions listed in the Phases table.
  - It rates every finding (High, Medium, Low, Info) and says whether it was reproduced ("confirmed") or found by reading code ("from reading").
  - The output is a dated report in [`reports/`](reports/): findings P1-1 … P7-6, a summary table and a suggested fix order.
  - A matching manual test list is in [`tests/security-audit-test-checklist.md`](tests/security-audit-test-checklist.md).
- **Out of scope:** the code of third-party dependencies (only checked with `cargo audit` and `cargo deny`), the operating system, WebKit and the desktop environment, hardware attacks, a machine that already has malware running as the user, and the web, Android and iOS clients that do not exist yet. The Rust compiler and the RustCrypto libraries are trusted.

### When is the audit conducted
- Started after the main features (vault, profiles, import/export, desktop app, accessibility) existed
- Finished before the first public release described in `docs/12-publishing-ubuntu.md`.
- The audit scope is a snapshot of the code at that date (the report names the commit).
- It must be **repeated in part** when something it covers changes: the vault format, the key hierarchy, the store, the clipboard code, import/export, the installer or the CI.
- Each fix is re-checked with the matching section of the test checklist.
- Findings are reviewed first. Fixes are reviewed before release.

### Where is the audit conducted
- In this repository: the Rust crates `core`, `store`, `app`, `clipboard`, `cli`, `transfer`, the fuzz crates, the desktop app in `apps/desktop`, the GitHub Actions workflows, `deny.toml`, `packaging/` and `apps/desktop/install-local.sh`.
- On the developer's machine (Ubuntu, Wayland): running the CLI against throwaway vaults, checking real file modes of the data folders (names and modes only, never contents), and timing the attacks that could be measured.

### Who conducts the audit
- **Done by:** Claude (an AI assistant), working in Claude Code at the request of the project owner. This is a review by the assistant that also helped write the code, so it can share the code's blind spots. It is not an independent third-party audit.
- **Reviewed and decided by:** the project owner, who decides which findings to accept and in what order to fix them.
- **For:** the maintainers (to fix), future contributors (to know the rules), and users who want to know what has been checked.
- **Next:** an outside review of the crypto design and the sync/rollback design is recommended before any claim of being "audited".

### How is the audit conducted
1. **Automated checks first:** `cargo audit`, `cargo deny check`, a search for `unsafe`, and a search for network code.
2. **Early leads:** a list of suspicions from the first read, each to be confirmed or dropped.
3. **Phases in order**, from the inside out (crypto core, then storage, session, clipboard and CLI, import/export, UI, supply chain). Every file in scope was read, with file and line references.
4. **Reproduce where possible:** a temporary test or a command on a throwaway vault, kept outside the repository so the audit changed no code. Findings that could not be reproduced are marked "from reading".
5. **Rate and record:** severity, impact, a fix suggestion, and a note of what held up so good decisions are not undone later.
6. **Summarise and order:** the summary table and a suggested fix order.
7. **Verify later:** every finding has a manual check in the test checklist; the fix for each is expected to add an automated test, and the finding's status is updated here once the check passes.

## Phases
| # | Phase | Scope | Main question | Impact on failure | Status |
|---|---|---|---|---|---|
| 1 | Crypto core | `core/{crypto,vault,item,totp,generator}.rs` | Do any secrets leave their `Zeroizing` wrapper, so that they are not wiped from memory? | Old passwords and keys stay in memory after use, where a memory dump, swap file or crash report can expose them. | done |
| 1 | Crypto core | `core/{crypto,vault,item,totp,generator}.rs` | Is every `Item` wiped when it is dropped, including its history of old values? | Passwords the user has already changed, including leaked ones, survive in memory and can be recovered. | done |
| 1 | Crypto core | `core/{crypto,vault,item,totp,generator}.rs` | Does the password generator favour some characters over others (modulo bias)? | Generated passwords are less random than they look, so they are easier to guess. | done |
| 1 | Crypto core | `core/{crypto,vault,item,totp,generator}.rs` | Is the TOTP code handled correctly and are its secrets kept safe? | Two-factor codes come out wrong, so the user cannot log in, or the TOTP secret leaks and the second factor is lost. | done |
| 2 | Storage and integrity | `store/{sqlite,directory}.rs`, `app/vault.rs` | Can someone roll the vault back to an older version, or delete records, without the app noticing? | A sync server or anyone with write access can silently restore an old password or remove items, and the user never finds out. | done |
| 2 | Storage and integrity | `store/{sqlite,directory}.rs`, `app/vault.rs` | Are files written atomically (temp file, then rename) and with owner-only permissions (0600)? | A crash or power cut can leave a half-written, unreadable vault, and other users on the machine can read the files. | done |
| 2 | Storage and integrity | `store/{sqlite,directory}.rs`, `app/vault.rs` | Can a crafted value cause SQL injection, or a crafted file name escape the vault folder (path traversal)? | An attacker-supplied value or name can change or delete data, or touch files outside the vault folder. | done |
| 2 | Storage and integrity | `store/{sqlite,directory}.rs`, `app/vault.rs` | Does a deleted item (tombstone) leave readable plaintext behind? | Items the user deleted can still be read from disk by someone who copies the vault. | done |
| 3 | Session and lock | `app/session.rs` | Does the vault lock by itself after the idle time, and on suspend? | An unattended, unlocked vault stays open to anyone at the keyboard. | done |
| 3 | Session and lock | `app/session.rs` | Is the key dropped from memory, and are revealed values cleared, when the vault locks? | The vault looks locked but the key and revealed passwords are still in memory, so locking gives false safety. | done |
| 3 | Session and lock | `app/session.rs` | Is guessing the master password slowed down after wrong attempts (brute-force throttling)? | Someone at the keyboard can try many passwords quickly. | done |
| 3 | Session and lock | `app/session.rs` | Does the profile index leak anything it should not? | Other users or programs can learn names, usernames or how many vaults exist. | done |
| 4 | Clipboard and CLI | `clipboard`, `cli` | Can other programs read the clipboard, and is the secret cleared from it after the delay? | A copied password stays on the clipboard and any program, or a clipboard history tool, can read it. | done |
| 4 | Clipboard and CLI | `clipboard`, `cli` | Can another program replace the helper that the app starts again (`current_exe()` re-spawn hijack) or the clipboard tools it finds through `PATH`? | A planted program receives every copied secret. | done |
| 4 | Clipboard and CLI | `clipboard`, `cli` | Do secrets appear in command-line arguments, environment variables, shell history, or on screen while typing (TTY echo)? | Passwords can be read from `ps`, `/proc`, shell history or over the user's shoulder. | done |
| 4 | Clipboard and CLI | `clipboard`, `cli` | Does any command print a password with `println!` when it should not? | A password lands in the terminal, scrollback or a log file in clear text. | done |
| 5 | Import and export | `transfer`, fuzz targets | Can an imported file inject spreadsheet formulas or terminal escape sequences? | Opening an export in a spreadsheet runs attacker formulas, or imported text rewrites the terminal screen or the clipboard. | done |
| 5 | Import and export | `transfer`, fuzz targets | Does a plaintext export ask for the master password again before writing? | Anyone at an unlocked screen can dump every password to a file. | done |
| 5 | Import and export | `transfer`, fuzz targets | Does hostile or very large input (a huge file, a malformed CSV or bundle) crash the app or make it hang? | The app freezes, runs out of memory or crashes, and the user may lose work or be unable to import. | done |
| 5 | Import and export | `transfer`, fuzz targets | Do the fuzz tests cover all the places that read untrusted input? | Bugs in parsers go unnoticed until someone finds them first. | done |
| 6 | Desktop UI | `apps/desktop` | Are typed secrets, such as the master password, wiped from memory (non-zeroized `String`s), and where does the web view keep copies? | The master password stays in memory, including the web view's, after it was typed. | done |
| 6 | Desktop UI | `apps/desktop` | Can a screenshot, screen sharing, Wayland or the accessibility tree reveal secrets that should be hidden? | Passwords or two-factor codes are exposed to other programs or viewers without the user's knowledge. | done |
| 6 | Desktop UI | `apps/desktop` | Is the clipboard cleared when the app locks or closes? | A copied password stays available after the vault is locked. | done |
| 6 | Desktop UI | `apps/desktop` | Does the web view write files to disk, or leave developer tools open in a release build? | Secrets are left in browser files on disk, or an attacker can inspect the running app. | done |
| 7 | Supply chain and release | CI, `deny.toml`, packaging | Are the GitHub Actions pinned to exact commits, and does CI run with the least permissions it needs? | A compromised third-party action can run in the build and tamper with releases. | done |
| 7 | Supply chain and release | CI, `deny.toml`, packaging | Can the same source be built again to give the same result (locked dependencies, pinned toolchain), and are releases checksummed and signed? | Users cannot check that a download is the genuine, untampered build. | done |
| 7 | Supply chain and release | CI, `deny.toml`, packaging | Do `cargo audit` and `cargo deny` pass, and is every exception explained? | Known-vulnerable or unwanted dependencies slip into the app. | done |
| 7 | Supply chain and release | CI, `deny.toml`, packaging | Does the install script change only what it should, and nothing else on the user's system? | The installer changes system settings without the user's knowledge, which can weaken other software. | done |

## Approach
Work through the phases in order. Report each finding with file and line, and a severity. Write a test or PoC where a finding is checkable. Fix nothing until the findings have been reviewed.

## Reports
The findings, the summary and the suggested fix order are in dated report files, one per audit run, in [`reports/`](reports/):
- [Security audit report, 2026-10-26 19:24](reports/security-audit-report-202610261924.md): all seven phases, 52 findings (2 High, 14 Medium, 29 Low, 7 Info), the automated checks and the early leads.

A new run of the audit (or a re-audit of a changed area) gets a new report named `security-audit-report-yyyymmddhhmm.md`. Reports are not edited afterwards, except to update the **Status** of a finding when it is fixed.

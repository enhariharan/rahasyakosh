# A tour of the repository

```
rahasyakosh/
├── Cargo.toml            workspace definition + shared version/licence
├── deny.toml             rules for cargo-deny (licences, bans, advisories)
├── SECURITY.md           how to report a vulnerability
├── CONTRIBUTING.md       how to contribute
├── CODE_OF_CONDUCT.md    how we treat each other
├── .github/workflows/    CI: runs all checks on every push and PR
├── docs/                 design documents (01–06) and this book (docs/book)
├── apps/
    │   └── desktop/      the Dioxus desktop app (its own workspace; needs WebKitGTK to build)
    └── crates/
    ├── core/             rahasyakosh-core: all security-relevant logic
    ├── clipboard/        rahasyakosh-clipboard: copy to the desktop clipboard and clear it (shared by rk and the app)
    ├── store/            rahasyakosh-store: saves encrypted records (SQLite, directory and memory backends)
    ├── app/              rahasyakosh-app: the unlocked vault (items, trash, history, search)
    ├── fuzzcheck/        rahasyakosh-fuzzcheck: robustness checks for every parser, used by tests and by `fuzz/`
    ├── strength/         rahasyakosh-strength: advisory password strength meter (zxcvbn)
    ├── transfer/         rahasyakosh-transfer: CSV import and CSV/JSON export (pure conversion)
    └── cli/              rk: the command-line front-end
```

## `crates/core` — the library that matters

| File | Responsibility |
|---|---|
| `crypto.rs` | Thin wrappers over Argon2id, HKDF and XChaCha20-Poly1305: `seal`, `open`, `derive_master_key` |
| `vault.rs` | Header, key wrapping, unlocking, changing password, encrypting/decrypting records |
| `item.rs` | The decrypted item model: title, fields, kind (login, note, …) |
| `generator.rs` | Random passwords, passphrases, entropy estimate |
| `wordlist.rs` | The bundled EFF passphrase word list |
| `totp.rs` | Time-based one-time codes |
| `error.rs` | The `Error` type; messages here must never leak secrets |
| `lib.rs` | Module list and `#![forbid(unsafe_code)]` |

Rules: **no file access, no network, no clock, no ambient randomness.** Everything is passed in.
That is what makes it testable and portable to WebAssembly.

## `apps/desktop` — the desktop app

A separate Cargo workspace (it needs WebKitGTK and other system libraries that the rest of the project does not). `src/model.rs` holds everything the screens do, tested without a window;
`src/lib.rs` draws it with Dioxus. See [The desktop app](desktop-app.md).

## `crates/clipboard` — the clipboard

Wraps the desktop's clipboard tools (`wl-copy`/`wl-paste`, `xclip`, `xsel`) and implements "clear it, but only if it is still our value". Used by `rk` and by the desktop app.

## `crates/store` and `crates/app` — storage, vault operations and the session

`store` saves and loads encrypted records and never has a key; `app` holds the keys and the decrypted items and offers one tested set of operations to every
front-end, and the **session** (locking, auto-lock, reveal and clipboard bookkeeping). See [Where your data lives](storage.md) and [Sessions](sessions.md).

## `crates/fuzzcheck` and `fuzz/` — attacking our own parsers

`fuzzcheck` holds ten functions that must survive any input; `fuzz/` is the `cargo fuzz` project that drives them with a fuzzer, with `fuzz/corpus/` (seeds) and
`fuzz/regressions/` (every past crash). See [Fuzzing](fuzzing.md).

## `crates/strength` — the strength meter

One file, `lib.rs`, wrapping the `zxcvbn` library. It is a separate crate because it needs the clock and,
on the web, JavaScript, neither of which `core` may use. See
[Passwords and passphrases](passwords-and-passphrases.md#the-strength-meter).

## `crates/transfer` — import and export

Converts between CSV/JSON text and items. No file or terminal access. See
[Importing and exporting](import-export.md).

## `crates/cli` — the `rk` program

`clipboard.rs` copies secrets with the desktop's clipboard tools and clears them again. `main.rs` defines the commands (`init`, `add`, `get`, `list`, `edit`, `history`, `revert`, `totp`, `rm`, `trash`, `restore`, `gen`, `passwd`, `import`, `export`, `migrate`) using
`clap`. `file.rs` reads and writes the portable JSON *bundle* used by `rk export -f vault` and `rk migrate`.
Everything else goes through `rahasyakosh-app`.

## CI: the safety net

On every push, GitHub Actions (`.github/workflows/ci.yml`) runs:

| Check | Catches |
|---|---|
| `cargo fmt --check` | inconsistent formatting |
| `cargo clippy -D warnings` | common mistakes and suspicious code |
| `cargo test` | broken behaviour |
| wasm32 build of core, store, app, strength and transfer | accidental use of OS-only features |
| `cargo deny` | bad licences, banned crates, yanked versions |
| `cargo audit` | dependencies with known vulnerabilities |

## The design documents

`docs/01`…`06` cover vision, user stories (IDs like `US-GEN-02`), architecture, the security design,
UX and the roadmap. Quote story IDs in commits, PRs and tests so every change traces to a reason.

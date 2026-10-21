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
└── crates/
    ├── core/             rahasyakosh-core: all security-relevant logic
    └── cli/              rk: the command-line front-end
```

## `crates/core` — the library that matters

| File | Responsibility |
|---|---|
| `crypto.rs` | Thin wrappers over Argon2id, HKDF and XChaCha20-Poly1305: `seal`, `open`, `derive_master_key` |
| `vault.rs` | Header, key wrapping, unlocking, changing password, encrypting/decrypting records |
| `item.rs` | The decrypted item model: title, fields, kind (login, note, …) |
| `generator.rs` | Random passwords, passphrases, entropy estimate |
| `totp.rs` | Time-based one-time codes |
| `error.rs` | The `Error` type; messages here must never leak secrets |
| `lib.rs` | Module list and `#![forbid(unsafe_code)]` |

Rules: **no file access, no network, no clock, no ambient randomness.** Everything is passed in.
That is what makes it testable and portable to WebAssembly.

## `crates/cli` — the `rk` program

`main.rs` defines the commands (`init`, `add`, `get`, `list`, `totp`, `rm`, `gen`, `passwd`) using
`clap`. `file.rs` reads and writes the vault file. It is deliberately simple; a proper storage
layer (SQLite and directory backends) arrives in milestone M2.

## CI: the safety net

On every push, GitHub Actions (`.github/workflows/ci.yml`) runs:

| Check | Catches |
|---|---|
| `cargo fmt --check` | inconsistent formatting |
| `cargo clippy -D warnings` | common mistakes and suspicious code |
| `cargo test` | broken behaviour |
| wasm32 build of core | accidental use of OS-only features |
| `cargo deny` | bad licences, banned crates, yanked versions |
| `cargo audit` | dependencies with known vulnerabilities |

## The design documents

`docs/01`…`06` cover vision, user stories (IDs like `US-GEN-02`), architecture, the security design,
UX and the roadmap. Quote story IDs in commits, PRs and tests so every change traces to a reason.

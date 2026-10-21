# 06 — Roadmap & Implementation Plan

| Milestone | Scope | Stories | Exit criteria | Status | Remarks |
|-----------|-------|---------|---------------|--------|---------|
| **M0 Foundations** | Workspace, MIT licence, CI, `deny.toml`, SECURITY.md, CONTRIBUTING.md, CODE_OF_CONDUCT.md | US-OSS-01, US-OSS-02 | CI green on Linux + wasm32 build | done | CI, `deny.toml`, SECURITY.md, CONTRIBUTING.md and CODE_OF_CONDUCT.md in place |
| **M1 Core** | `core`: KDF, key hierarchy, AEAD records, vault header, item model, generator, strength, TOTP; test vectors | US-VLT-01, US-VLT-02, US-VLT-03, US-VLT-04, US-ITM-01, US-GEN-01, US-GEN-02, US-GEN-03, US-TOT-01, US-TOT-02 | All KATs pass; round-trip property tests; wasm32 builds | done | Stories, KATs (HKDF, XChaCha20, Argon2id), golden vault (native + WASM in CI) and property tests (round-trip, bit-flip, tamper; proptest) done. US-VLT-04: key wiping on drop done; sessions, idle timeout and manual lock deferred to M3 |
| **M1b CLI** | `rk` CLI on file backend | US-CLI-01, US-IMP-01 (CSV), US-IMP-03 | Usable daily by authors | done | `rk` has init, add, get (with `--copy`), list, edit, totp, rm, trash, restore, gen, passwd, import, export. No `lock`: the CLI is stateless; sessions and `lock` come with M3 (US-VLT-04) |
| **M2 Store** | Store trait, SQLite + dir backends, trash, history, search index | US-ITM-02, US-ITM-03, US-ITM-04, US-ITM-05, US-ITM-08 | Migration tests; 10k items perf | in-progress | Design in `docs/07-store-design.md` (approved). Done: `store` crate with trait, memory, directory and SQLite backends, conformance suite, migration framework with a schema-v1 fixture. `Item.history` (schema 2, backward compatible) and the `app` crate (`Vault<S>`: unlock, add/edit, trash, expiry, history/revert, search) done. Next: CLI on the new stack, `rk migrate`, performance tests |
| **M3 App + Desktop** | `app` session crate, Dioxus desktop, auto-lock, clipboard clear | US-SEC-01, US-SEC-02, US-SEC-03, US-SEC-04, US-DSK-01 | Alpha release (Linux/macOS/Win) | to-do | Not started |
| **M4 Sync** | HLC merge, folder transport, device pairing, reference server | US-SYN-01, US-SYN-02, US-SYN-03, US-SYN-04, US-SYN-05 | Chaos tests: no data loss under concurrent edits | to-do | Not started |
| **M5 Web + Extension** | WASM API, IndexedDB store, PWA, MV3 extension autofill | US-WEB-01, US-EXT-01 | Beta | to-do | Not started |
| **M6 Android** | Android app, Keystore biometrics, AutofillService | US-AND-01, US-AND-02 | Beta on F-Droid/Play | to-do | Not started |
| **M7 Hardening → 1.0** | Health/breach check, importers, i18n, a11y audit, external security audit | US-GEN-04, US-GEN-05, US-IMP-02, US-A11Y-01, US-I18N-01 | 1.0 | to-do | Not started |
| Later | iOS, sharing, passkeys, SSH agent, attachments | | | |  |

## Testing strategy
- Unit + KAT (RFC 6238, Argon2 reference vectors, XChaCha20-Poly1305 vectors).
- Property tests: encrypt→decrypt identity, tamper → error, any bit-flip in header/record rejected.
- Fuzzing: header/record parsers, importers.
- Cross-platform golden vault in `spec/vectors/` opened by native and WASM tests in CI.
- UI: component snapshot tests; e2e with WebDriver for web/extension; manual a11y passes.

## Definition of done (per story)
Acceptance criteria met · tests · docs updated · clippy/fmt clean · no new `unsafe` without review ·
security-relevant changes reviewed by a second maintainer.

## Progress
- [x] M0: workspace, MIT licence, CI (fmt, clippy, tests, wasm32 build, cargo-deny, cargo-audit), `deny.toml`, SECURITY.md, CONTRIBUTING.md, CODE_OF_CONDUCT.md
- [x] M1 core: KDF, key hierarchy, records, items, generator, TOTP (13 tests; wasm32 builds)
- [x] M1b CLI `rk`: init, add, get, list, totp, rm, gen, passwd
- [x] US-GEN-02: EFF large wordlist bundled in core; `rk gen --passphrase`
- [x] US-GEN-03: strength meter (zxcvbn, separate `rahasyakosh-strength` crate); enforced for master passwords (US-VLT-01); `rk init|passwd --passphrase`
- [x] US-IMP-01 (CSV: generic/Chrome/Firefox, column mapping, preview, dedupe) and US-IMP-03 (CSV/JSON plaintext with warning + re-auth, encrypted vault copy): `rk import`, `rk export`
- [x] Known-answer tests for HKDF, XChaCha20-Poly1305, Argon2id; golden vault `spec/vectors/golden-v1.*` opened by native and WASM tests (CI `wasm-test` job)
- [x] `rk edit` (title, username, URL, notes, password via prompt/stdin/generate, TOTP, tags); CLI end-to-end tests. `lock` deferred to M3 by decision: the CLI is stateless
- [x] Property tests (`crates/core/tests/properties.rs`, `crates/transfer/tests/properties.rs`); mutation-checked. They found and fixed an importer bug (whitespace stripped from usernames/passwords)
- [x] `rk get|totp --copy` with auto-clear (US-SEC-01 for the CLI; Wayland/X11 tools), `rk trash`, `rk restore`, `rk rm --purge` reaches trashed items
- [x] M2 work packages 1-2: `rahasyakosh-store` crate (`Store` trait, memory and directory backends, shared conformance suite, crash-recovery tests)
- [x] M2 work package 3: SQLite backend (`sqlite` feature; WAL, `synchronous=FULL`, `secure_delete`, 0600 files), migrations with rollback, schema-v1 fixture
- [x] M2 work package 4: `Item.history` (`ITEM_SCHEMA` 2; newest first, capped at 10, revert keeps the past); schema-1 payloads and the golden vault still load
- [x] M2 work package 5: `rahasyakosh-app` (`Vault<S>`) tested on memory, directory and SQLite stores, with a random-operation model test and a two-session race test
- [ ] Next: M2 work packages 6-7 (CLI switch + `rk migrate`/`history`/`revert`, performance tests, book chapter). Fuzzing of header/record parsers and importers is still open (see Testing strategy)

## Original order
1. Workspace + `rahasyakosh-core` (M1) with tests.
2. `rk` CLI on a single-file vault (M1b) as the first usable vertical slice.
3. WASM build check of `core`.

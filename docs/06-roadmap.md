# 06 — Roadmap & Implementation Plan

| Milestone | Scope | Stories | Exit criteria |
|-----------|-------|---------|---------------|
| **M0 Foundations** | Workspace, MIT licence, CI, `deny.toml`, SECURITY.md, CONTRIBUTING.md, CODE_OF_CONDUCT.md | US-OSS-01, US-OSS-02 | CI green on Linux + wasm32 build |
| **M1 Core** | `core`: KDF, key hierarchy, AEAD records, vault header, item model, generator, strength, TOTP; test vectors | US-VLT-01, US-VLT-02, US-VLT-03, US-VLT-04, US-ITM-01, US-GEN-01, US-GEN-02, US-GEN-03, US-TOT-01, US-TOT-02 | All KATs pass; round-trip property tests; wasm32 builds |
| **M1b CLI** | `rk` CLI on file backend | US-CLI-01, US-IMP-01 (CSV), US-IMP-03 | Usable daily by authors |
| **M2 Store** | Store trait, SQLite + dir backends, trash, history, search index | US-ITM-02, US-ITM-03, US-ITM-04, US-ITM-05, US-ITM-08 | Migration tests; 10k items perf |
| **M3 App + Desktop** | `app` session crate, Dioxus desktop, auto-lock, clipboard clear | US-SEC-01, US-SEC-02, US-SEC-03, US-SEC-04, US-DSK-01 | Alpha release (Linux/macOS/Win) |
| **M4 Sync** | HLC merge, folder transport, device pairing, reference server | US-SYN-01, US-SYN-02, US-SYN-03, US-SYN-04, US-SYN-05 | Chaos tests: no data loss under concurrent edits |
| **M5 Web + Extension** | WASM API, IndexedDB store, PWA, MV3 extension autofill | US-WEB-01, US-EXT-01 | Beta |
| **M6 Android** | Android app, Keystore biometrics, AutofillService | US-AND-01, US-AND-02 | Beta on F-Droid/Play |
| **M7 Hardening → 1.0** | Health/breach check, importers, i18n, a11y audit, external security audit | US-GEN-04, US-GEN-05, US-IMP-02, US-A11Y-01, US-I18N-01 | 1.0 |
| Later | iOS, sharing, passkeys, SSH agent, attachments | | |

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
- [ ] Next: CSV import/export (US-IMP-01, US-IMP-03), then M2 store

## Original order
1. Workspace + `rahasyakosh-core` (M1) with tests.
2. `rk` CLI on a single-file vault (M1b) as the first usable vertical slice.
3. WASM build check of `core`.

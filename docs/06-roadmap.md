# 06 — Roadmap & Implementation Plan

| Milestone | Scope | Stories | Exit criteria |
|-----------|-------|---------|---------------|
| **M0 Foundations** | Workspace, MIT licence, CI, `deny.toml`, SECURITY.md | OSS-01/02 | CI green on Linux + wasm32 build |
| **M1 Core** | `core`: KDF, key hierarchy, AEAD records, vault header, item model, generator, strength, TOTP; test vectors | VLT-01..04, ITM-01, GEN-01..03, TOT-01/02 | All KATs pass; round-trip property tests; wasm32 builds |
| **M1b CLI** | `rk` CLI on file backend | CLI-01, IMP-01(CSV), IMP-03 | Usable daily by authors |
| **M2 Store** | Store trait, SQLite + dir backends, trash, history, search index | ITM-02..05, ITM-08 | Migration tests; 10k items perf |
| **M3 App + Desktop** | `app` session crate, Dioxus desktop, auto-lock, clipboard clear | SEC-*, DSK-01 | Alpha release (Linux/macOS/Win) |
| **M4 Sync** | HLC merge, folder transport, device pairing, reference server | SYN-* | Chaos tests: no data loss under concurrent edits |
| **M5 Web + Extension** | WASM API, IndexedDB store, PWA, MV3 extension autofill | WEB-01, EXT-01 | Beta |
| **M6 Android** | Android app, Keystore biometrics, AutofillService | AND-* | Beta on F-Droid/Play |
| **M7 Hardening → 1.0** | Health/breach check, importers, i18n, a11y audit, external security audit | GEN-04/05, IMP-02, A11Y, I18N | 1.0 |
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
- [x] M0 partial: workspace, MIT licence (CI, deny.toml, SECURITY.md still to do)
- [x] M1 core: KDF, key hierarchy, records, items, generator, TOTP (13 tests; wasm32 builds)
- [x] M1b CLI `rk`: init, add, get, list, totp, rm, gen, passwd
- [ ] Next: CI + deny.toml, EFF wordlist + passphrase CLI, strength meter, CSV import/export, then M2 store

## Original order
1. Workspace + `rahasyakosh-core` (M1) with tests.
2. `rk` CLI on a single-file vault (M1b) as the first usable vertical slice.
3. WASM build check of `core`.

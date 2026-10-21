# 01 — Vision & Scope

## Vision
A password manager you can fully audit, self-host, and trust: one Rust core, identical
behaviour on every platform, zero-knowledge by construction, no account required.

## Principles
1. **Zero knowledge.** Plaintext and keys never leave the device. Any server/sync target only sees ciphertext.
2. **Local-first.** Fully functional offline; sync is optional and additive.
3. **One core, many shells.** All security-relevant logic lives in `rahasyakosh-core`; UIs are thin.
4. **Boring crypto.** Only audited RustCrypto primitives; no custom constructions.
5. **No lock-in.** Documented vault format, full import/export.
6. **Small attack surface.** Minimal dependencies, `#![forbid(unsafe_code)]` in core, `cargo-deny`/`cargo-audit` in CI.

## Personas
| Persona | Needs |
|---------|-------|
| **Asha, everyday user** | Save/autofill logins on phone + laptop, strong password suggestions, simple recovery |
| **Dev, power user** | CLI, TOTP, SSH keys/notes, scriptability, self-hosted sync |
| **Meera, family organiser** | Share a few logins with family safely (post-v1) |
| **Auditor / contributor** | Readable spec, reproducible builds, test vectors |

## In scope (v1.0)
Vault CRUD (logins, secure notes, TOTP, cards, identities), password/passphrase generator,
strength meter, auto-lock, clipboard clearing, search/tags/favourites, import/export,
file-based E2EE sync, desktop + Android + browser app, browser-extension autofill, CLI.

## Non-goals (v1.0)
Hosted SaaS offering, enterprise SSO/SCIM, passkey *provider* on all platforms (tracked as stretch),
hardware-token-only vaults, closed-source components.

## Success metrics
- Unlock-to-autofill < 1 s on mid-range Android.
- WASM core bundle < 600 KB gzipped.
- Zero high/critical findings in an external audit before 1.0 tag.
- Same encrypted vault opens bit-identically on all platforms (cross-platform test vectors).

## Licence
MIT. All dependencies must be MIT/Apache-2.0/BSD/ISC-compatible (enforced by `cargo-deny`).

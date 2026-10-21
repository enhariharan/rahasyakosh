# 03 — Architecture

## 1. Layered view

```
 ┌────────────┬────────────┬────────────┬─────────────┬─────────────────┐
 │  CLI       │ Desktop    │ Android    │ Web (PWA)   │ Browser ext.    │   shells (thin)
 │ (clap)     │ (Dioxus)   │ (Dioxus/   │ (Dioxus     │ (WASM + JS      │
 │            │            │  UniFFI)   │  web)       │  content script)│
 ├────────────┴────────────┴────────────┴─────────────┴─────────────────┤
 │ rahasyakosh-app   — session state machine, use-cases, view-models    │
 ├──────────────────────────────────────────────────────────────────────┤
 │ rahasyakosh-sync  — merge engine, sync transports (folder, server)   │
 ├──────────────────────────────────────────────────────────────────────┤
 │ rahasyakosh-core  — crypto, vault model, format, generator, TOTP     │  no I/O, wasm-safe,
 │                     (pure, deterministic given RNG; no unsafe)       │  forbid(unsafe)
 ├──────────────────────────────────────────────────────────────────────┤
 │ rahasyakosh-store — `Store` trait + backends: fs, sqlite, IndexedDB  │
 └──────────────────────────────────────────────────────────────────────┘
```

Rules:
* Dependencies only point downward.
* `core` has no filesystem, network, clock or RNG access of its own
  * They are injected (`Rng`, `Clock` traits) so it is testable and WASM-safe.

## 2. Workspace layout

```
rahasyakosh/
├─ Cargo.toml                (workspace)
├─ crates/
│  ├─ core/        rahasyakosh-core
│  ├─ store/       rahasyakosh-store      (M2)
│  ├─ sync/        rahasyakosh-sync       (M4)
│  ├─ app/         rahasyakosh-app        (M3)
│  ├─ cli/         rahasyakosh-cli        (binary `rk`)
│  ├─ wasm/        rahasyakosh-wasm       (wasm-bindgen API for web + extension)
│  ├─ ffi/         rahasyakosh-ffi        (UniFFI for Kotlin/Swift, if needed)
│  └─ ui/          rahasyakosh-ui         (Dioxus components shared by desktop/web/mobile)
├─ apps/
│  ├─ desktop/
│  ├─ web/
│  ├─ android/
│  └─ extension/
├─ docs/
└─ spec/  (format spec + test vectors)
```

## 3. Key decisions (ADRs)

**ADR-1 Rust core shared everywhere.** One implementation of crypto/format; shells call it
(directly, via wasm-bindgen, or UniFFI). *Alternatives rejected:* per-platform crypto (divergence risk).

**ADR-2 UI: Dioxus for desktop + web; Android via Dioxus-mobile with UniFFI + Kotlin as the
fallback.** Gives one Rust UI codebase (RSX, signals). Risk: Android autofill/accessibility
services must be native Kotlin regardless, so `ffi` crate is required for US-AND-02. Decision
re-evaluated at M5 spike. *Alternatives:* Tauri (web UI in JS—less Rust, but mature mobile),
Slint (good embedded, smaller ecosystem), egui (poor a11y).

**ADR-3 Crypto: RustCrypto only** — `argon2`, `chacha20poly1305` (XChaCha20-Poly1305), `hkdf`,
`sha2`, `x25519-dalek`/`ed25519-dalek`, `zeroize`, `getrandom` (with `js` feature on wasm).
Rationale: pure Rust, compiles to WASM, audited. No OpenSSL/ring.

**ADR-4 Storage: item-granular encrypted records behind a `Store` trait.**
Each item is an independent AEAD blob keyed by UUIDv7 id. Enables incremental sync and avoids
rewriting a monolith. Backends: single-file container for CLI/desktop (SQLite, bundled) in M2;
IndexedDB on web; plain directory of `<id>.rkr` files for folder-sync mode. Metadata that
would leak (titles, URLs) is inside the ciphertext; only id, version, updated-at (HLC) and size are plaintext.

**ADR-5 Sync: state-based per-item LWW with Hybrid Logical Clocks**, tombstones, and
conflict-preserving history (losing version retained as history entry). No server-side logic
needed → any dumb blob store works (folder, S3, WebDAV). A reference server (axum) is optional.
CRDT for fields deferred (P2) – per-item granularity is sufficient for a password vault.

**ADR-6 Browser extension: MV3, WASM core in the service worker/offscreen document;
content script only detects forms and requests credentials by origin match.** Native-messaging
bridge to the desktop app is P2.

**ADR-7 Errors/logging:** `thiserror` in libs, `anyhow` in binaries; `tracing` with a
redaction layer; secrets are wrapped in `Secret<T>` whose `Debug` prints `[REDACTED]`.

**ADR-8 MSRV & features:** stable Rust, edition 2021→2024 when stable across targets; `core`
has `std` feature on by default; `alloc`-only possible later.

## 4. Core domain model

```
Vault { id, header: VaultHeader }
Item  { id: Uuid, kind: Login|Note|Card|Identity|Totp, title, fields[], tags[], urls[],
        favourite, created_at, updated_at, history[], deleted_at? }
Field { name, value: Secret|Text, kind: Text|Concealed|Url|Totp|Email }
```
`ItemPayload` (what is encrypted) is CBOR-/JSON-serialised (serde); JSON chosen first for
debuggability, versioned with `schema: u16`.

## 5. Runtime/session model (`app` crate)

State machine: `Uninitialised → Locked → Unlocked{keys, index} → Locked`.
All commands (`add_item`, `search`, `generate`, …) are methods available only on `Unlocked`
(type-state or runtime guard), so a locked vault cannot leak. Index is in-memory, rebuilt on unlock.

## 6. Platform specifics

| Platform | Secure storage for biometric key | Notes |
|----------|----------------------------------|-------|
| Linux | Secret Service (libsecret) via `keyring` | Wayland clipboard needs `wl-clipboard` crate |
| macOS | Keychain + LAContext | notarised build |
| Windows | Credential Manager + Windows Hello | |
| Android | Keystore (AES-GCM key, biometric-bound) | `FLAG_SECURE`, AutofillService (Kotlin) |
| Web | Not available; master password every session (optionally WebAuthn PRF) | CSP strict, no 3rd-party scripts |
| iOS | Keychain/Secure Enclave | later |

## 7. Build, quality gates
- `cargo fmt`, `clippy -D warnings`, `cargo test --workspace`, `cargo deny`, `cargo audit`.
- `wasm32-unknown-unknown` build of `core` and `wasm` on every PR with a size budget (<600 KB gz).
- Property tests (`proptest`) for format round-trip; fuzz targets (`cargo-fuzz`) for parsers.
- Known-answer tests for each primitive; RFC 6238 TOTP vectors; spec vectors in `spec/`.
- Reproducible release builds; SBOM (`cargo-cyclonedx`); signed tags.

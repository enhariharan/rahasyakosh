# 04 — Security & Crypto Design

## 1. Threat model

| Adversary | In scope? | Mitigation |
|-----------|-----------|------------|
| Thief with vault file / stolen cloud blob | Yes | Argon2id + XChaCha20-Poly1305; optional Secret Key (built, opt-in: `rk init --secret-key`, `rk secret-key enable`) |
| Malicious/compromised sync server | Yes | E2EE; integrity via AEAD with id/version bound as AAD; rollback detection (a sealed manifest of newest revisions, plus a local high-water mark of the highest write seen; see docs/book storage.md) |
| Network attacker | Yes | TLS + E2EE; no secrets in requests (HIBP uses k-anonymity) |
| Shoulder surfing / screen capture | Partly | Masked fields, FLAG_SECURE, clipboard auto-clear |
| Malware running as user, while unlocked | **No** (documented) | Minimise key lifetime, auto-lock |
| Cold-boot / memory forensics on locked device | Partly | zeroize on lock. `mlock` is **not built** (needs `unsafe`; see audit finding P6-1) |
| Malicious web page (extension) | Yes | Strict origin matching (eTLD+1 via PSL), no autofill in iframes cross-origin by default, user gesture for fill |
| Supply-chain | Yes | cargo-deny/audit, pinned lockfile, minimal deps, reproducible builds |

## 2. Key hierarchy

```
master password ──Argon2id(salt, m,t,p)──► MK (32 B)  ──┐
                                                         ├─HKDF-SHA256("rk/kek/v1")──► KEK
secret key (optional, 128 b, local) ─────────────────────┘     (2SKD: HKDF salt=secret key; "rk/kek/v2" when used)

The master password is first converted to NFKC (header `pw_norm`). With a Secret Key the header has `secret_key: true`
(authenticated), the KEK label is `rk/kek/v2`, and the Secret Key is the HKDF salt. The key is 128 random bits written
as `RK1-` plus eight groups of four base32 characters (the last 32 bits are a checksum). It is **not** in the vault;
it is kept in a 0600 file under `~/.local/share/rahasyakosh/secret-keys/<vault id>.key` and on the user's printed sheet.

VK (32 B random, per vault)  ──XChaCha20-Poly1305 under KEK──► wrapped_vk (in header)
VK ──HKDF("rk/item/v1" ‖ item_id)──► item key (per item)   (so each item uses a unique key)
VK ──HKDF("rk/auth/v1")──► sync auth token key             (never the master password)
```
- Changing the master password re-wraps VK only.
- Rotating VK (on suspected compromise) re-encrypts all items (explicit, slow operation).
- Header integrity: the header is bound as AAD of wrapped_vk. There is **no** separate `verifier` blob: a wrong password is detected when the wrapped vault key fails to decrypt (an AEAD failure), which is equally strong and has one thing less to keep consistent. The `magic` bytes are mixed into the AAD of the header and the records, not stored as a header field.

## 3. Primitives & parameters
| Purpose | Primitive | Params |
|---------|-----------|--------|
| KDF | Argon2id v1.3 | default m=64 MiB, t=3, p=1 (desktop); mobile m=32 MiB, t=3; stored in header, floor enforced (m≥19 MiB, t≥2) |
| AEAD | XChaCha20-Poly1305 | 24-byte random nonce per encryption (safe to randomise) |
| KDF expand | HKDF-SHA256 | domain-separated `info` strings, versioned |
| RNG | OS CSPRNG via `getrandom` | injected `Rng` trait; tests use seeded ChaCha |
| Signatures/sharing (P2) | Ed25519 / X25519 | |
| TOTP | HMAC-SHA1/256/512 | RFC 6238 |
| Hash for HIBP | SHA-1 | only first 5 hex chars leave the device |

## 4. Vault container format v1 (spec summary; full spec in `spec/format-v1.md`)

```
Header (plaintext, CBOR/JSON, canonical):
  magic "RKV1", format_version u16, vault_id uuid,
  kdf { alg:"argon2id", m_kib, t, p, salt[16] }, secret_key bool (omitted when false), pw_norm "nfkc" (omitted in older vaults),
  wrapped_vk { nonce[24], ct[48] }   (no verifier; see section 2)
Record (one per item):
  record_version u16, item_id uuid, rev u64 (HLC), nonce[24], ciphertext
  AAD = magic ‖ vault_id ‖ item_id ‖ rev ‖ record_version
Payload (decrypted): JSON ItemPayload { schema, kind, title, fields, … } — padded to 64 B buckets
```
Binding `item_id` and `rev` in AAD prevents record swapping/replay between items.
Padding hides exact lengths coarsely.

## 5. Secret handling rules (enforced by review + lint)
1. All secret-bearing types derive `Zeroize + ZeroizeOnDrop`; no `Clone` unless needed.
2. `Debug`/`Display` for secrets redact. No secret ever passes through `format!`/`tracing`.
3. Constant-time comparison (`subtle`) for MACs/verifiers.
4. No secret-dependent branching in our code beyond library primitives.
5. `#![forbid(unsafe_code)]` in `core`; `unsafe` in shells (mlock, FFI) isolated and reviewed (none exists yet).
6. Clipboard writes tag as sensitive where OS supports (Android `EXTRA_IS_SENSITIVE`, macOS concealed type).
7. Plaintext export requires re-auth and warns.

## 6. Autofill safety (extension & Android)
- Match on registrable domain (Public Suffix List); HTTPS required unless explicit override.
- Never autofill on page load without user gesture (default); no filling hidden fields.
- Credentials are delivered to the content script only for the matched origin, one item at a time.

## 7. Disclosure & audit
`SECURITY.md` with private contact; target an independent audit before 1.0; publish format spec & test vectors.

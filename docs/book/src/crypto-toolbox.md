# The cryptography toolbox

The project uses a handful of standard tools. Each does one job. We use implementations from the
[RustCrypto](https://github.com/RustCrypto) family: written in pure Rust and widely reviewed. We do
**not** write our own cryptography, and neither should you.

| Job | Tool | In the code |
|---|---|---|
| Turn the master password into a key | Argon2id | `crypto::derive_master_key` |
| Make several keys from one key | HKDF-SHA256 | `crypto::hkdf_expand` |
| Encrypt and authenticate data | XChaCha20-Poly1305 | `crypto::seal`, `crypto::open` |
| Two-factor codes | HMAC (SHA-1/256/512) | `totp.rs` |
| Unpredictable bytes | OS CSPRNG | injected as `rng` |

## Argon2id — "make guessing passwords expensive"

Humans choose weak passwords, and an attacker holding a stolen vault can try billions of guesses
offline. Argon2id defends by making each guess cost real time **and memory**. With our defaults
(64 MiB of memory, 3 passes), one guess takes a noticeable fraction of a second on a normal
computer, and cannot be sped up cheaply with graphics cards or custom chips because of the memory
requirement.

The settings (`m_kib`, `t`, `p`) are stored in the vault header, so they can be strengthened later.
When *opening* a vault we also enforce a **minimum** (`KdfParams::MIN_M_KIB`, `MIN_T`), so an
attacker cannot trick us into accepting a weak, fast setting (a *downgrade attack*), and a
**maximum**, so a hostile file cannot ask for 100 GB of memory.

Why "id"? Argon2 comes in variants. Argon2id is the one recommended for password hashing.

## HKDF — "one key in, many independent keys out"

We need several keys (one for wrapping the vault key, one for each item…). HKDF derives many keys
from one secret. Each derivation includes a label such as `"rk/kek/v1"` or `"rk/item/v1" + item_id`.
Different labels yield unrelated keys, so a mistake with one key cannot expose another. Labels
are versioned (`v1`) so they can evolve.

## XChaCha20-Poly1305 — "lock the box and add a tamper seal"

This is the AEAD that protects data. *XChaCha20* scrambles the bytes; *Poly1305* produces the tag
that proves nothing was altered. The "X" gives a 24-byte nonce, which is large enough that we can
simply choose a fresh random one for every encryption and never worry about repeats.

`crypto::seal` returns the nonce plus ciphertext. `crypto::open` returns the plaintext or a single
generic error. It never says *why* it failed.

## HMAC — "a keyed fingerprint"

Used inside TOTP (see [One-time codes](totp.md)). It mixes a secret key with a message, using a
hash function, to produce a code only the key-holder can compute.

## Things we deliberately avoid

- **Home-made algorithms or "clever" combinations.** Rule one of cryptography.
- **OpenSSL and `ring`.** Banned in `deny.toml` to keep the build pure Rust and portable to WebAssembly.
- **Old or broken algorithms** (MD5, DES, ECB mode, RC4). SHA-1 appears *only* because TOTP and the
  breach-check protocol require it; it is not used to protect your data.

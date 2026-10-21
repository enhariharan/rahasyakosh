# Test vectors

Files here are a **compatibility contract**. Every future version of `rahasyakosh-core` must open them,
on every platform (native and WebAssembly), and third-party implementations can use them to check theirs.

## Golden vault, format version 1

| File | Contents |
|---|---|
| `golden-v1.rkv` | A vault as the portable JSON *bundle*: `{ "header": …, "records": […] }` (what `rk export -f vault` writes and `rk migrate` loads) |
| `golden-v1.expected.json` | The five items inside it, decrypted |

- **Master password:** `रहस्यकोश-golden-1` (UTF-8; includes non-ASCII on purpose)
- **KDF:** Argon2id, m = 65536 KiB, t = 3, p = 1 (`KdfParams::DESKTOP`)
- **Items:** a login with TOTP and tags, a note with emoji and Devanagari text, a login edited once
  (revision 2) with quotes and a backslash in its password, a login in the trash, and a long note that
  spans several padding buckets.
- **Deterministic:** generated from a fixed seed, ids and timestamps by
  `cargo run -p rahasyakosh-core --example golden -- spec/vectors`.
- The vault holds only made-up data.

## Rules

1. **Never edit these files.** `crates/core/tests/golden.rs` pins their SHA-256 and fails if they change.
2. **Do not regenerate them to make a test pass.** A failing test means you broke compatibility with
   existing vaults. Fix the code.
3. When the format version changes, **add** `golden-v2.*` and keep `golden-v1.*` (old vaults must still open).

## Primitive known-answer tests

These live in `crates/core/src/crypto.rs` (module `kat`): HKDF-SHA256 (RFC 5869), XChaCha20-Poly1305
(draft-irtf-cfrg-xchacha-03, A.3.1), Argon2id (RFC 9106 §5.3), plus Argon2id outputs computed
independently with OpenSSL 3.5 and Python `cryptography` 46. TOTP vectors (RFC 6238) are in `totp.rs`.

## Schema fixtures (SQLite store)

| File | Contents |
|---|---|
| `schema-v1.sqlite` | A database as written by schema version 1: sample header, three records, one tombstone |

`crates/store/tests/sqlite.rs` copies every `schema-v*.sqlite` here, opens it with the current code and checks that it upgrades and
still holds the same data. When a new schema version is released, generate its fixture once with
`cargo run -p rahasyakosh-store --features sqlite --example gen_schema_fixture -- spec/vectors` (it refuses to overwrite) and keep all
older ones. The same rule applies as for the golden vault: never edit or regenerate a released fixture.

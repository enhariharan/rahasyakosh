# Testing with known answers

Ordinary tests check that code does what *we think* is right. In cryptography that is not enough:
if we misunderstand an algorithm, our code and our test can be wrong in the same way and both pass.
So this project also tests against answers that come from **outside** the code.

## Known-answer tests (KATs)

A **known-answer test** feeds fixed inputs to a primitive and checks the output against a value
published by someone else. They live in `crates/core/src/crypto.rs`, module `kat`:

| Primitive | Where the expected answer comes from |
|---|---|
| HKDF-SHA256 | RFC 5869, Appendix A (test cases 1 and 3) |
| XChaCha20-Poly1305 | draft-irtf-cfrg-xchacha-03, Appendix A.3.1 |
| Argon2id (the library) | RFC 9106, section 5.3 |
| Argon2id (our wrapper) | Computed independently, see below |
| TOTP | RFC 6238, Appendix B (in `totp.rs`) |

These tests catch the quiet failures: a dependency upgrade that changes behaviour, a wrong parameter
(for example memory in the wrong unit), or a refactor that subtly alters what is computed.

### Why OpenSSL and Python appear in the history

The RFC 9106 Argon2 vector uses two extra inputs (a "secret" and "associated data") that our wrapper
does not use. So it cannot test *our* function, only the library underneath it.

To test our own wrapper at our own settings we needed correct answers from somewhere other than our
code. We asked two unrelated implementations, **OpenSSL** and Python's **`cryptography`** package, for
the Argon2id result at the same inputs. They share no code with each other or with the Rust crate.
When independent programs agree, the shared answer is very likely right, and that value is
written into the test. **Nothing depends on OpenSSL or Python at run time.** They were used once, to
produce the expected numbers. The comment above those tests records which versions were used.

The general rule: *the expected value must never be produced by the code under test.*

## Golden files

A **golden file** is a saved, known-good output kept in the repository. Tests compare the code against
it. "Golden" means it is the reference that everything else is measured against.

Our golden file is a complete small vault:

| File | What it is |
|---|---|
| `spec/vectors/golden-v1.rkv` | A real encrypted vault in the CLI's file layout |
| `spec/vectors/golden-v1.expected.json` | What should be inside once decrypted |
| `spec/vectors/README.md` | Its master password, settings and rules |

The test in `crates/core/tests/golden.rs` opens the vault with the known password and checks that
the five items match the expected file exactly. It also checks that wrong passwords, flipped bits,
swapped records and weakened settings are all rejected.

### Why this matters so much for a password manager

Imagine a developer improves the code and, without noticing, changes how records are encrypted.
Their new tests pass, because new code reads what new code writes. But every vault already on
users' disks is now unreadable. For a password manager that is catastrophic: the user's only copy
of their passwords is gone. The golden vault is a stand-in for "every vault anyone has ever made".
If the current code can still open it, old vaults still open.

### The rules

1. **Never edit a golden file.** A checksum in the test fails if you do.
2. **Never regenerate one to make a failing test pass.** A failure means compatibility broke. Fix the
   code, not the test.
3. **When the format changes on purpose,** add a *new* golden file (`golden-v2`) and keep the old one.
   Old vaults must continue to open.
4. It is opened on **both native and WebAssembly** in CI (`wasm-test` job): the same file, the same
   expected result, on every platform we support.
5. It is **deterministic**: made from a fixed seed, fixed ids and fixed timestamps, by
   `cargo run -p rahasyakosh-core --example golden -- spec/vectors`. Re-running it gives identical bytes.
6. It contains only made-up data, and non-ASCII text on purpose (Devanagari, an emoji, quotes and a
   backslash) so that character-handling bugs show up.

### Other kinds of "golden" you may meet

Elsewhere in software, golden files are often *snapshots* of program output (for example, a
rendered screen) that are updated when the output intentionally changes. Ours is stricter: it is
never updated, because other people's data depends on it.

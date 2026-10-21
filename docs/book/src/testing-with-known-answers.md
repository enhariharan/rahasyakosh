# Testing with known answers and properties

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
| `spec/vectors/golden-v1.rkv` | A real encrypted vault, in the portable *bundle* layout (a JSON file; `rk migrate` loads it) |
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

## Property tests

A known-answer test checks one input. A **property test** states a rule that must hold for *every* input,
then lets the computer try hundreds of generated inputs, including odd ones a person would never think of:
empty strings, quotes, newlines, emoji, enormous numbers. We use the `proptest` library. If a rule fails,
it automatically **shrinks** the failing input to the smallest one that still fails, which makes the bug
easy to see.

They live in `crates/core/tests/properties.rs` and `crates/transfer/tests/properties.rs`. The rules include:

| Rule | In plain English |
|---|---|
| seal then open is the identity | Encrypting then decrypting gives back exactly what you put in |
| any single bit flip is rejected | Change one bit of a ciphertext, nonce, record or vault header, and opening fails |
| wrong key, wrong context, resized data | All rejected |
| a record cannot be moved or replayed | Changing its item id, revision or version makes it fail |
| only the right password unlocks | And changing the password keeps items readable while revoking the old one |
| generated passwords match their spec | Right length, every chosen character class present, none of the excluded ones |
| TOTP codes have the right shape | Right number of digits, and unchanged within one time step |
| parsers never panic | Feed the CSV importer and the TOTP parsers arbitrary junk; they may refuse it, but must not crash |
| export then import preserves every value | Nothing is trimmed, altered or lost on the way |

They run only on native targets, because `proptest` does not build for WebAssembly. The known-answer
tests and golden vault cover WebAssembly.

### A real bug they found

The export-then-import property failed at once on a username that was just a tab character. The importer
trimmed spaces from every CSV cell, so it quietly turned `"\t"` into `""`. The same would have happened
to a password with a leading or trailing space. Changing a stored secret during import is exactly the sort
of silent bug that is miserable to track down later. The fix is to tidy only titles, URLs, TOTP
secrets and tags, and leave usernames, passwords and notes exactly as they are. A plain unit test
(`secrets_are_not_trimmed`) now pins it. This is the usual pattern: let a property find the bug, then
keep a small ordinary test so the failure is obvious next time.

## Checking that the tests can fail

A test that always passes is worse than none. After writing the security properties we deliberately **broke the
code** in small ways ("mutations") and checked that the tests noticed:

| Deliberate breakage | Caught? |
|---|---|
| `open` ignores the associated data | Yes, by seven tests |
| Record associated data ignores the revision number | Yes |
| Header associated data ignores the vault id | Yes |
| Padding bucket changed from 64 to 63 | Yes |
| Header associated data ignores the salt | **No** |
| Record associated data ignores the item id | **No** |
| Header or record associated data ignores the version number | **No** |

The last three are not gaps in the tests. They are **redundant protections**. The salt already
changes the key that Argon2 derives. The item id already changes the per-item key. The version
numbers are checked by an explicit "is this a version I understand?" test before anything else happens.
So removing the extra binding does not make any attack succeed, and no test of behaviour can tell the difference.
That is deliberate defence in depth: if one layer has a flaw, the other still holds.

One consequence for future maintainers: **when a second format version exists, add tests for
version downgrade attacks.** Until then, version binding in the associated data cannot be exercised.

# Writing code that handles secrets

Most security bugs are not broken mathematics but ordinary mistakes around the maths. These are
the habits this repository enforces. Full list: section 5 of `docs/04-security-and-crypto.md`.

## 1. Wipe secrets when done
Types holding secrets use the `zeroize` crate (`Zeroizing<…>`, `ZeroizeOnDrop`) so memory is
overwritten when the value goes away. A plain `String` password may leave copies behind. Don't
casually `.clone()` a secret; each clone is another copy to wipe.

## 2. Never print secrets
`Debug` and `Display` for secret types must redact. Never put a secret in `format!`, `println!`,
error messages or logs. Error messages are an especially common leak.

## 3. Compare secrets in constant time
Use `subtle`'s constant-time equality. A normal `==` can return faster the earlier the
mismatch occurs. Over many measurements that timing leaks the secret.

## 4. Fail the same way every time
Wrong password, corrupt data and tampered header all produce the *same* generic error. Different
messages would tell an attacker which guess was "closer".

## 5. No `unsafe` in core
`rahasyakosh-core` has `#![forbid(unsafe_code)]`. Rust's safety guarantees are a major reason for
choosing it. `unsafe` code (for example, locking memory pages) belongs only in the thin outer
layers, isolated and reviewed.

## 6. No hidden inputs: inject randomness and time
The core never calls the system random generator or the clock. (This is why the strength meter, whose
library does read the clock, is a separate crate.) The caller passes in an `rng` and a
timestamp. Benefits: tests can use a fixed seed and reproduce results exactly, and the same code
compiles to WebAssembly where "the system" differs.

## 7. Treat file contents as hostile
A vault file may come from an attacker. Validate sizes and parameters before using them (see the
Argon2 bounds), and prefer errors over panics.

## 8. Keep dependencies few and vetted
Every dependency is code we trust with your secrets. New crates must pass `cargo deny` (licences,
advisories, banned crates) and `cargo audit` (known vulnerabilities) in CI.

## 9. Test with known answers
Cryptography is unforgiving. We test against **published test vectors** (e.g. RFC 6238 for TOTP)
so we know we match the standard (see [Testing with known answers](testing-with-known-answers.md)), plus **property tests**: "encrypt then decrypt returns the input",
"changing any bit makes decryption fail".

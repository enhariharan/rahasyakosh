# Glossary

**AAD** — Associated data: unencrypted context covered by the authentication tag.
**AEAD** — Authenticated encryption with associated data; encrypts and detects tampering.
**Argon2id** — A deliberately slow, memory-hungry password-hashing/KDF algorithm.
**Base32** — Text encoding using A–Z and 2–7; used for TOTP secrets.
**Bit** — A single 0/1. A 256-bit key has 2²⁵⁶ possible values.
**Ciphertext** — Encrypted data.
**Constant-time** — Taking the same time regardless of secret values.
**CSPRNG** — Cryptographically secure pseudo-random number generator.
**Downgrade attack** — Tricking software into using weaker settings.
**EFF wordlist** — The Electronic Frontier Foundation's 7,776-word list for dice-based passphrases.
**Entropy** — A measure of unpredictability, in bits.
**Golden file** — A saved known-good output kept in the repo; tests check the code still reproduces or opens it. Ours is never edited.
**HKDF** — A function that derives several independent keys from one secret.
**HMAC** — A keyed hash used to authenticate messages.
**CSV injection** — A spreadsheet treating a cell starting with `=`, `+`, `-` or `@` as a formula.
**KDF** — Key derivation function.
**KEK** — Key-encrypting key; protects (wraps) another key.
**Key** — A secret value used to encrypt or decrypt.
**MAC** — Message authentication code.
**Known-answer test (KAT)** — A test that checks output against a value published or computed by an independent source.
**Locked / unlocked** — Whether the vault key is erased from memory (locked) or held there for use (unlocked).
**Master password** — The one password the user remembers.
**Modulo bias** — Unevenness introduced by using `%` on random numbers.
**Nonce** — A number that must be used only once per key.
**Passphrase** — A password made of random words.
**Plaintext** — Readable, unencrypted data.
**Poly1305** — The authentication part of XChaCha20-Poly1305.
**Property test** — A test that checks a rule holds for many generated inputs.
**Rejection sampling** — Discarding random draws that would introduce bias.
**Re-authentication** — Asking for the master password again before a dangerous action.
**Salt** — Random public data mixed into password hashing.
**Strength score** — A 0–4 rating of how guessable a password is (from zxcvbn).
**Session** — A long-lived unlocked state that remembers the key for a while. Planned for M3; the CLI has none.
**Test vector** — A published input/output pair used to verify an implementation.
**Threat model** — Statement of who we defend against and who we don't.
**TOTP** — Time-based one-time password (RFC 6238).
**Vault** — The encrypted file holding all items.
**VK** — Vault key: the random key that (via per-item keys) encrypts items.
**Wrapped key** — A key encrypted under another key.
**XChaCha20-Poly1305** — The AEAD cipher used here, with a 24-byte nonce.
**Zero knowledge** — The service provider cannot read your data.
**Zeroize** — Overwrite secret memory with zeros.

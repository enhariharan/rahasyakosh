# Security words you will meet

You will see these words in code, comments and reviews. Skim now, return when needed.

## Plaintext and ciphertext
**Plaintext** is readable data, such as your password `hunter2`. **Ciphertext** is the same data
after **encryption**: random-looking bytes. **Decryption** turns it back, but only if you have the
right **key**.

## Key
A key is a secret number (here, 32 random bytes = 256 bits) that locks and unlocks data. The
algorithm is public; secrecy lives entirely in the key. This is called *Kerckhoffs's principle*,
and it is why we use well-known, published algorithms and never invent our own.

## Bits and "bits of security"
A 128-bit secret has 2¹²⁸ possibilities. If every possibility is equally likely, guessing it means
trying about half of them. "N bits of **entropy**" means "as hard to guess as an N-bit random
number". Each extra bit doubles the work. 70+ bits is out of reach for any attacker today when
guesses are slow; 128+ bits is out of reach for everyone.

## Random, and *cryptographically* random
Ordinary "random" functions (as used in games) are predictable if you know the seed. A **CSPRNG**
(cryptographically secure pseudo-random number generator) is fed unpredictable data from the
operating system and cannot be predicted. **Every secret in this project comes from a CSPRNG.**

## Hash
A **hash function** turns any input into a short fixed-size fingerprint. The same input always
gives the same fingerprint; you cannot go backwards from the fingerprint; changing one bit of the
input changes the fingerprint completely. SHA-256 is a hash function.

## Salt
A random value, stored in the open, that is mixed in when hashing a password. Two users with the
same password then get different results, which stops attackers from using precomputed tables.

## KDF (key derivation function)
A function that turns a **weak** secret (a human-chosen password) into a **strong** key. Good ones
are deliberately slow and memory-hungry so that an attacker's guesses are expensive. We use
**Argon2id**.

## MAC and authentication
Encryption alone hides data but does not stop someone *changing* it. **Authentication** detects
tampering. A **MAC** (message authentication code) is a keyed checksum: only someone with the key
can produce a valid one.

## AEAD
**Authenticated Encryption with Associated Data** does both jobs at once: it encrypts, and it
attaches a tag that fails verification if the ciphertext (or the associated data) was altered.
We use **XChaCha20-Poly1305**. If decryption fails, you cannot tell whether the key was wrong or
the data was damaged; this is intentional.

## Nonce
"Number used once". A value that must never repeat for the same key. Reusing a nonce can destroy
the security of the cipher. XChaCha20 uses 24-byte nonces, long enough to pick at random safely.

## AAD (associated data)
Extra data that is *not* encrypted but *is* covered by the authentication tag. We use it to bind a
ciphertext to its context (for example "this record belongs to item #7 in vault X"), so an attacker
cannot cut-and-paste records around.

## Constant-time
Code whose running time does not depend on secret values. If comparing two secrets stops at the
first differing byte, the time taken leaks information. Secret comparisons here use the `subtle` crate.

## Zeroize
Overwriting secret memory with zeros when finished, so it does not linger for malware or a memory
dump to find.

## Threat model
A written answer to "who are we defending against, and who are we *not*?" See
[What we defend against](threat-model.md).

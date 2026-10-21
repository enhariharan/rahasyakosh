# One-time codes (TOTP)

Many sites ask for a six-digit code from an app that changes every 30 seconds. This is **TOTP**
(time-based one-time password), standardised in RFC 6238. Rahasyakosh can store the secret and
show the live code (`crates/core/src/totp.rs`).

## How it works

When you enable two-factor authentication, the site gives you a **shared secret** (usually shown
as a QR code or a text in base32, like `JBSWY3DPEHPK3PXP`). Both the site and your app keep it.

Every 30 seconds both sides compute:

1. `counter = floor(unix_time / 30)`: a number that increments each period.
2. `HMAC(secret, counter)`: a keyed fingerprint (see [HMAC](crypto-toolbox.md)).
3. Take a few bytes from the result and reduce them to six digits ("dynamic truncation", from
   RFC 4226's HOTP).

Because both sides know the secret and the time, they get the same digits without communicating.
An attacker who sees one code cannot predict the next, nor learn the secret.

## Details worth knowing

- The code accepts the common `otpauth://totp/...` link format, SHA-1/256/512, 6–8 digits and
  30/60 second periods. The default (SHA-1, 6 digits, 30 s) is what most sites use.
- SHA-1 is acceptable *here*: HMAC-SHA-1 does not suffer from the weaknesses that broke plain SHA-1.
- The `Totp` type holds the secret and is wiped on drop.
- The code takes the current time as an argument rather than reading the clock. That keeps the
  library deterministic in tests, which use the official RFC test vectors.

## A design trade-off

Keeping the TOTP secret in the same vault as the password means someone who breaks into the vault
gets both factors. Many people accept that for convenience; some do not. This is a *product*
decision, not a bug, and the documentation should be honest about it.

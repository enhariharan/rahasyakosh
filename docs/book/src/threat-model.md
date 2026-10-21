# What we defend against

A **threat model** states honestly who the adversaries are and what we promise. A product that
claims to stop everything stops nothing.

## We defend against

| Attacker | How we respond |
|---|---|
| Someone who steals the vault file (laptop theft, cloud-folder leak) | Everything is encrypted. They must guess the master password, and Argon2id makes each guess slow and costly. |
| A malicious or hacked sync server | It only ever holds ciphertext. Tampering is detected by AEAD tags. Replaying old data is detected by revision tracking. |
| Someone watching the network | Data is encrypted before it leaves the device; transport also uses TLS. |
| Someone who edits the file to weaken it | Header fields are authenticated; Argon2 settings have enforced minimums. |
| Malicious entries in the supply chain | Few dependencies, pinned lockfile, `cargo deny` and `cargo audit` in CI. |

## We only partly defend against
- Someone looking over your shoulder or taking a screenshot (masked fields, clipboard auto-clear).
- Memory forensics on a locked device (we wipe keys on lock, but cannot beat every technique).

## We do **not** defend against
- **Malware on your device while the vault is unlocked.** If an attacker can already run code as
  you, they can read the screen, log keystrokes and read memory. No password manager can fix a
  compromised device.
- **A weak master password.** Argon2 slows guessing but cannot save "password123".
- **You being tricked.** Phishing remains the biggest real-world risk; autofill that matches the
  website's real domain is one mitigation.

## Why write this down?
Because every design decision traces back to it. When someone proposes a feature, ask: *which
attacker does this help against, and which new attack surface does it add?* If you cannot answer,
the feature is not ready.

# Security Policy

Rahasyakosh is a password manager. **It has not been independently audited yet — do not store real secrets.**
An external audit is planned before 1.0 (see `docs/04-security-and-crypto.md`).

## Reporting a vulnerability

Please report privately; do not open a public issue.

- Use GitHub's [private vulnerability reporting](https://github.com/enhariharan/rahasyakosh/security/advisories/new).

Include affected version/commit, reproduction steps and impact. We aim to acknowledge within 7 days
and to agree a fix and disclosure timeline with you; we credit reporters unless asked not to.

## Scope

In scope: `rahasyakosh-core` (crypto, vault format, key handling), the `rk` CLI, and the vault file format.
Out of scope: attacks requiring a compromised or malicious host (keyloggers, root malware) and weak master passwords.

## Supported versions

Only the latest commit on `main` is supported until 1.0.

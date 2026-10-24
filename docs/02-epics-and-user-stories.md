# 02 — Epics & User Stories

Priority: **P0** = MVP (M1–M3), **P1** = v1.0, **P2** = post-1.0.
IDs are stable; reference them in commits and tests (e.g. `US-VLT-02`).

## E1 — Vault & Master Credential (P0)
| ID | Story | Acceptance criteria |
|----|-------|---------------------|
| US-VLT-01 | As a user I create a new vault protected by a master password. | Argon2id-derived key; vault file created; password strength shown; refuses empty/very weak (<zxcvbn 2) unless overridden. |
| US-VLT-02 | I unlock an existing vault with my master password. | Wrong password gives generic error, no timing oracle on which part failed; KDF params read from header. |
| US-VLT-03 | I change my master password. | Only the wrapped vault key is re-encrypted; items untouched; old password stops working. |
| US-VLT-04 | I lock the vault manually or automatically. | Keys zeroized; idle timeout configurable; lock on app background / system sleep (platform permitting). Key wiping on drop is in core; sessions, idle timeout and manual lock belong to the M3 `app` session crate (the stateless CLI has nothing to lock). |
| US-VLT-05 | I get a Secret Key / recovery kit (optional) for extra entropy and recovery. (P1) | 128-bit secret key combined with password (2SKD); printable Emergency Kit. |
| US-VLT-06 | I can have multiple vaults. (P1) | Independent keys per vault; switcher in UI. |

## E2 — Item Management (P0)
| ID | Story | Acceptance criteria |
|----|-------|---------------------|
| US-ITM-01 | I add a login (title, username, password, URLs, notes). | Item encrypted at rest; timestamps set. |
| US-ITM-02 | I view, edit and delete items. | Edits keep history (last N versions); delete goes to trash. |
| US-ITM-03 | I restore or permanently delete from trash. | Trash auto-purges after 30 days (configurable). |
| US-ITM-04 | I search by title, username, URL, tag. | Search runs on decrypted in-memory index only; <50 ms for 5k items. |
| US-ITM-05 | I organise with tags and favourites. | Tags many-to-many; favourites pinned. |
| US-ITM-06 | I store secure notes, cards, identities, and custom fields. (P1) | Typed schemas; custom fields can be marked concealed. |
| US-ITM-07 | I attach small files. (P2) | Chunked, individually encrypted; size cap. |
| US-ITM-08 | I view item history and restore a prior version. (P1) | Diff view; restore creates a new version. |

## E3 — Generator & Health (P0/P1)
| ID | Story | Acceptance criteria |
|----|-------|---------------------|
| US-GEN-01 | I generate random passwords with length/charset rules. (P0) | CSPRNG, unbiased sampling, exclude-ambiguous option. |
| US-GEN-02 | I generate passphrases (diceware wordlist). (P0) | EFF large wordlist, separator & capitalisation options. |
| US-GEN-03 | I see a strength estimate. (P0) | zxcvbn-style score + crack-time text. |
| US-GEN-04 | I see weak / reused / old passwords. (P1) | Local analysis only. |
| US-GEN-05 | I check for breaches. (P1) | HIBP k-anonymity range API (5-char SHA-1 prefix); opt-in; never sends full hash. |

## E4 — TOTP & Second Factors (P0/P1)
| ID | Story | Acceptance criteria |
|----|-------|---------------------|
| US-TOT-01 | I add a TOTP secret to an item (manual, `otpauth://` URI, QR scan). (P0) | RFC 6238 SHA-1/256/512, 6–8 digits, 30/60 s; RFC test vectors pass. |
| US-TOT-02 | I see the live code with countdown and copy it. (P0) | Refreshes on boundary. |
| US-TOT-03 | I store recovery codes in an item. (P1) | Concealed field. |
| US-TOT-04 | I store passkeys. (P2) | WebAuthn provider APIs per platform. |

## E5 — Clipboard & Secret Hygiene (P0)
| ID | Story | Acceptance criteria |
|----|-------|---------------------|
| US-SEC-01 | Copied secrets clear automatically. | Default 20 s; only clears if clipboard still holds our value. |
| US-SEC-02 | Secrets hidden by default; reveal on demand. | Masked fields; screenshots blocked on Android (`FLAG_SECURE`). |
| US-SEC-03 | Memory is wiped on lock. | `zeroize` on all key/secret types; no secrets in logs. |
| US-SEC-04 | Brute-force friction. | KDF cost tunable; optional lockout/backoff after failed biometric. |

## E6 — Sync & Backup (P1)
| ID | Story | Acceptance criteria |
|----|-------|---------------------|
| US-SYN-01 | I export an encrypted backup file and restore it. (P0) | Format = vault file; restore verifies integrity. |
| US-SYN-02 | I sync via a folder (Syncthing/Dropbox/iCloud) with safe merge. (P1) | Per-item records, conflict-free merge (LWW by HLC), no data loss on concurrent edits (both versions kept in history). |
| US-SYN-03 | I sync via a self-hosted server. (P1) | Server stores ciphertext only; auth by derived token, not password. |
| US-SYN-04 | I add a new device by QR/code pairing. (P1) | Key transferred via PAKE/ephemeral X25519; no password retyping of secret key. |
| US-SYN-05 | I see sync status and conflicts. (P1) | Non-blocking UI indicator. |

## E7 — Import / Export (P0/P1)
| ID | Story | Acceptance criteria |
|----|-------|---------------------|
| US-IMP-01 | Import from CSV (generic, Chrome, Firefox). (P0) | Column mapping, preview, dedupe. |
| US-IMP-02 | Import Bitwarden JSON, KeePass KDBX, 1Password 1pux. (P1) | Mapped to native types; unsupported fields reported. |
| US-IMP-03 | Export CSV/JSON (plaintext, with warning) and encrypted. (P0) | Re-auth required; warning modal. |

## E8 — Platforms & Shells
| ID | Story | Priority |
|----|-------|----------|
| US-CLI-01 | CLI: `init, add, get, list, edit, rm, gen, totp, passwd, import, export` with stdin/pipe support. The CLI is **stateless** (each command unlocks, acts and exits, so keys never outlive it); a `lock` command and unlock sessions arrive with the M3 session crate (US-VLT-04). | P0 |
| US-DSK-01 | Desktop app (Linux/macOS/Windows) with tray, global quick-search shortcut. | P0 |
| US-DSK-02 | OS keychain/biometric unlock (libsecret, Keychain/Touch ID, Windows Hello). | P1 |
| US-WEB-01 | Web app (WASM) with IndexedDB storage, installable PWA. | P1 |
| US-EXT-01 | Browser extension (Chrome/Firefox MV3): detect login forms, autofill, save-on-submit prompt. | P1 |
| US-AND-01 | Android app with biometric unlock. | P1 |
| US-AND-02 | Android Autofill Service integration. | P1 |
| US-IOS-01 | iOS app + AutoFill Credential Provider. | P2 |
| US-SSH-01 | SSH agent mode serving keys from vault. | P2 |

## E9 — Sharing (P2)
| ID | Story | Acceptance criteria |
|----|-------|---------------------|
| US-SHR-01 | Share an item/vault with another user (public-key envelope). | |
| US-SHR-02 | Emergency access with time delay. | |

## E10 — Settings, Accessibility, i18n (P1)
| ID | Story | Acceptance criteria |
|----|-------|---------------------|
| US-SET-01 | Theme (light/dark/system), density, auto-lock, clipboard timeout, KDF strength. | |
| US-A11Y-01 | Full keyboard navigation, screen-reader labels, WCAG 2.2 AA contrast. | Linux desktop app: labels, contrast test, Orca pass and 200% zoom pass (2026-10-24, `docs/08-a11y-test-checklist.md`). Open: keyboard shortcut checklist (`docs/09`), macOS/Windows screen readers, an external audit |
| US-I18N-01 | Localisation via Fluent; initial: English, हिन्दी, தமிழ். | Desktop app done with built-in string tables instead of Fluent: English, हिन्दी, संस्कृतम्, தமிழ், తెలుగు, മലയാളം, اردو (right to left). Translations need native review |

## E11 — Project Health (P0)
| ID | Story | Acceptance criteria |
|----|-------|---------------------|
| US-OSS-01 | MIT licence, CONTRIBUTING, SECURITY.md (disclosure), CODE_OF_CONDUCT. | |
| US-OSS-02 | CI: fmt, clippy, tests, cargo-deny, cargo-audit, WASM size gate. | |
| US-OSS-03 | Published test vectors & format spec for third-party implementations. | |

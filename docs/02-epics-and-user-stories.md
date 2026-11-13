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
| US-IMP-01 | Import from CSV (generic, Chrome, Firefox). (P0) | Column mapping, preview, dedupe. Available in the CLI (`rk import`) and in the desktop app (**Settings → Import passwords**, with a file chooser and a drop-down per column when the names are not recognised). **Verified 2026-11-13.** |
| US-IMP-02 | Import Bitwarden JSON, KeePass KDBX, 1Password 1pux. (P1) | Mapped to native types; unsupported fields reported. **Partly done: Bitwarden JSON (unencrypted exports; logins, folders as tags; other entry types reported in a warning), and Bitwarden, KeePassXC and 1Password CSV, in the CLI and the desktop app. KeePass `.kdbx` and 1Password `.1pux` are not read yet.** **Verified 2026-11-13.** |
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
| US-A11Y-01 | Full keyboard navigation, screen-reader labels, WCAG 2.2 AA contrast. | Linux desktop app: labels, contrast test, Orca pass and 200% zoom pass (2026-10-24, `docs/tests/a11y-test-checklist.md`). Keyboard shortcut checklist passes (`docs/tests/shortcuts-test-checklist.md`, including other layouts). **Verified 2026-11-13.** Open: macOS/Windows screen readers, an external audit |
| US-I18N-01 | Localisation via Fluent; initial: English, हिन्दी, தமிழ். | Desktop app done with built-in string tables instead of Fluent: English, हिन्दी, संस्कृतम्, தமிழ், తెలుగు, മലയാളം, اردو (right to left). Translations need native review |

## E11 — Project Health (P0)
| ID | Story | Acceptance criteria |
|----|-------|---------------------|
| US-OSS-01 | MIT licence, CONTRIBUTING, SECURITY.md (disclosure), CODE_OF_CONDUCT. | |
| US-OSS-02 | CI: fmt, clippy, tests, cargo-deny, cargo-audit, WASM size gate. | |
| US-OSS-03 | Published test vectors & format spec for third-party implementations. | |

## E12 — Export to a Text File (P0)
| ID | Story | Acceptance criteria |
|----|-------|---------------------|
| US-EXP-01 | I export my saved logins to a plain text file I can read. (P0) | Desktop **Settings → Export passwords** (text) and `rk export -f txt`. Warning shown first; master password asked again; every live login with all its fields, sorted by title; trashed items left out; file created owner-only (`0600`); an existing file is never replaced; the result is announced to screen readers. **Done** for the Linux desktop app and the CLI (2026-10-24). |
| US-EXP-02 | I export to CSV and JSON from Settings as well as plain text. (P1) | The Settings export section offers a format choice (text, CSV, JSON), reusing `transfer::export_csv` / `export_json`. Same safety rules as US-EXP-01 (warning, master password again, owner-only file, never overwrites). **Done** for the Linux desktop app (2026-10-25); the CLI already supported both. |

## E13 — User Profiles (P1)
A **user profile** groups the passwords of one particular user, so one installation can hold the passwords of several users. A profile has a **unique user ID**, a **full name** and a **unique username**; more attributes are expected later, so the profile is a record that can grow, not three loose settings.

| ID | Story | Acceptance criteria |
|----|-------|---------------------|
| US-PRF-01 | The app stores user profiles, each with a unique user ID, a full name and a unique username. (P1) | The ID is generated and never changes. Usernames are unique across profiles (compared ignoring case); a duplicate is refused with a clear message. The profile format allows new attributes later without breaking old data (versioned, as items are). **Done (Linux desktop, 2026-10-24): `rahasyakosh_app::ProfileList`, stored as versioned `profiles.json`.** **Verified 2026-11-13.** |
| US-PRF-02 | At first startup there is a default profile, and I can change its name and username (optional). (P1) | Created with full name **"Default user"** and username **"defaultuser"**. The first-run flow offers both values for editing, pre-filled; leaving them unchanged is allowed and loses nothing. They can be changed later. **Done (Linux desktop): fields on the create-password page, pre-filled; an existing vault becomes the default profile in place.** **Verified 2026-11-13.** |
| US-PRF-03 | I add more profiles. (P1) | A new profile starts with no passwords. Full name and username are required; the user ID is generated. **Done (Linux desktop): **Add a profile** on the unlock screen.** **Verified 2026-11-13.** |
| US-PRF-04 | Every password belongs to exactly one profile, and I switch between profiles. (P1) | The list, search, trash, export and generator health checks only show the active profile. Switching is an explicit action and locks or hides the other profile's secrets. **Done in the desktop app: each profile is its own vault, picked on the unlock screen; the app starts on the profile used last. Done in the CLI: `rk --profile USERNAME` and `rk profiles`.** **Verified 2026-11-13.** |
| US-PRF-05 | I edit or delete a profile. (P1) | Editing the name or username keeps the user ID. Deleting a profile asks for confirmation, states how many passwords go with it, and moves them to the trash. The last remaining profile cannot be deleted. **Done in the desktop app with a changed rule: deleting removes the profile's vault file for good (typed master password; no trash for profiles).** **Verified 2026-11-13.** |

**Decision (2026-10-24, from the product owner):** every profile has **its own master password**. At startup the user **selects a profile and types that profile's master password**. So a profile is a separate vault with its own key: one user cannot read another's passwords, and this overlaps US-VLT-06 (multiple vaults). Consequences:
- Each profile's passwords live in their own encrypted vault file; the profile's ID, full name and username are kept in a small **profile list** beside them. The list is not secret, because the picker must show it before anything is unlocked.
- The unlock screen gains a profile picker; "switching profile" means locking and picking another one (US-PRF-04).
- Deleting a profile deletes its vault, so it needs a typed confirmation, not only a click (US-PRF-05).
- A forgotten master password for one profile loses only that profile's data (see the recovery key, US-VLT-04).

**Agreed answers (2026-10-24):**
1. **Existing vault:** it becomes the default profile in place (same file, same master password, ID generated on first run). Nothing is re-encrypted.
2. **Does the username sign in?** No. The user picks a profile from the list; the username is its unique label, shown in the picker and used by `rk --profile <username>`.
3. **Where do the first-run fields go?** On the create-master-password page, above the password boxes, pre-filled with "Default user" and "defaultuser" and marked optional.
4. **Name edits for the default profile later:** Settings → Profile.

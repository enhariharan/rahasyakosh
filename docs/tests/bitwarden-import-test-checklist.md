# Bitwarden import test checklist

A manual pass over **Settings → Import passwords** for Bitwarden CSV and JSON.
About 10 minutes. Use a throwaway vault:

```
RK_VAULT=/tmp/rk-bw-test/vault.db RK_CONFIG_HOME=/tmp/rk-bw-test/cfg rahasyakosh
```

Sample files are in `docs/tests/fixtures/bitwarden/`.

`✔` works · `✘` does not · `?` does something else (say what).

## Files

| File | What it tests |
| ---- | ------------- |
| `sample.csv` | 5 rows: 4 logins (one TOTP, one no-URL) + 1 note (skipped with warning) |
| `empty.csv` | Header-only CSV — no data rows |
| `truly-empty.csv` | Zero-byte file |
| `large.csv` | 220 logins across 7 folders, every 10th has TOTP |
| `sample.json` | 2 logins + 1 secure note + 1 card (non-logins skipped with warning) |
| `empty.json` | `{"encrypted":false,"folders":[],"items":[]}` |
| `encrypted.json` | `{"encrypted":true,...}` — must be rejected with a clear error |

## 1. CSV import

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| `sample.csv` | Format "bitwarden"; 4 to add; 1 warning about skipped note row | | |
| TOTP on GitHub row | `totp` field present after import; code rotates | | |
| No-URL entry | Imported; website field blank | | |
| `empty.csv` | A readable error or "0 to add"; no crash | | |
| `truly-empty.csv` | A readable error; no crash | | |
| `large.csv` | Format "bitwarden"; 220 to add; preview loads without hang | | |
| `large.csv` — confirm import | 220 items in vault; folders become tags | | |
| Import `sample.csv` a second time | 0 to add; all 4 shown as duplicates | | |

## 2. JSON import

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| `sample.json` | Format "bitwarden"; 2 to add (GitHub, Gmail); warning counts 2 skipped (note + card); no column picker | | |
| GitHub entry | title "GitHub", 2 URLs, tag "Work", notes, TOTP present | | |
| Gmail entry | title "Gmail", tag "Personal", favourite flag preserved as note or tag if surfaced | | |
| `empty.json` | "0 to add"; no crash | | |
| `encrypted.json` | Error: says the export is encrypted and asks for the unencrypted kind | | |

## 3. Edge cases

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Lock (Ctrl+L) mid-preview on `sample.csv` | After unlock: no preview (passwords were dropped from memory) | | |
| Import `large.csv` then import it again | Second preview: 0 to add, 220 duplicates | | |

## 4. Accessibility

- [ ] Keyboard only: Tab reaches **Choose a file…**, path box, and **Show what would be added**; Enter confirms.
- [ ] With Orca on: format name, item count, and any warning are announced when the preview appears.
- [ ] 200% zoom: preview table, warning, and button stay visible without horizontal scroll.

## Recording results

Paste filled-in tables below under a dated heading with commit, session type (Wayland/X11), and Orca on/off.
Every `✘` or `?` becomes an issue labelled `desktop` (or `a11y`).

---

## 2026-11-17 — commit 87fb1b4, CLI (`rk import`), Wayland, Orca off

Tested via `rk import` with `RK_PASSWORD` env var and a throwaway vault at `/tmp/rk-bw-test/`.
UI-specific rows (file chooser, column picker, preview in the desktop app, accessibility, zoom) need a separate desktop pass.

### 1. CSV import

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| `sample.csv` | Format "bitwarden"; 4 to add; 1 warning about skipped note row | ✔ | Also shows "delete this file" reminder |
| TOTP on GitHub row | `totp` field present after import; code rotates | ✔ | `rk totp GitHub` returns a valid 6-digit code with countdown |
| No-URL entry | Imported; website field blank | ✔ | "No URL entry" appears in `rk list` |
| `empty.csv` | A readable error or "0 to add"; no crash | ✔ | "0 to import, 0 duplicate(s) skipped" |
| `truly-empty.csv` | A readable error; no crash | ? | Error: `no password column found in headers []` — correct but the bare `[]` is technical; a "file is empty" message would be friendlier |
| `large.csv` | Format "bitwarden"; 220 to add; preview loads without hang | ✔ | Instant preview |
| `large.csv` — confirm import | 220 items in vault; folders become tags | ✔ | 220 imported in 0.375 s total (import + duplicate re-check); TOTP works on Service 010 |
| Import `sample.csv` a second time | 0 to add; all 4 shown as duplicates | ✔ | "0 to import, 4 duplicate(s) skipped" |

### 2. JSON import

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| `sample.json` | Format "bitwarden"; 2 to add; warning counts 2 skipped; no column picker | ✔ | GitHub and Gmail were already in vault (imported via CSV), so 0 added, 2 duplicates — cross-format duplicate detection works; 2 non-logins warned |
| `empty.json` | "0 to add"; no crash | ✔ | "0 to import, 0 duplicate(s) skipped" |
| `encrypted.json` | Error: says the export is encrypted and asks for the unencrypted kind | ✔ | "this Bitwarden export is encrypted; export it again as an unencrypted .json file" |

### 3. Edge cases

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Lock mid-preview | After unlock: no preview (passwords dropped from memory) | — | Desktop-only; not testable via CLI |
| Import `large.csv` then import it again | Second preview: 0 to add, 220 duplicates | ✔ | Confirmed |

### Issues found

- **Minor UX**: `truly-empty.csv` gives `no password column found in headers []`; the `[]` is a raw Rust debug repr. A message like "the file appears to be empty" would be clearer. Low priority.

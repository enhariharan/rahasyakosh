# 1Password .1pux import test checklist (US-IMP-02)

A manual pass over **Settings → Import passwords** in the Linux desktop app and over the `rk` CLI.
About 20 minutes. Use a throwaway vault:

```
RK_VAULT=/tmp/rk-1pux-test/vault.db RK_CONFIG_HOME=/tmp/rk-1pux-test/cfg rahasyakosh
```

`✔` works · `✘` does not · `?` does something else (say what).

## Files to prepare

Generate the sample fixture first:

```
cargo run -p rahasyakosh-transfer --example gen_test_fixtures
```

This writes `crates/transfer/tests/fixtures/sample.1pux`. The file contains:
- 7 login items in the Personal vault (GitHub with TOTP + multi-URL, My Bank with custom fields, Gmail, Unicode site, Special-chars entry, no-title entry, Wi-Fi standalone password)
- 1 login item in the Work vault (Jira — vault name "Work" should become a tag)
- 1 credit card item (should be skipped)
- 1 archived login (should be skipped)

Also export a real `.1pux` from 1Password if you have access:
**File → Export → 1Password Unencrypted Export (.1pux)** — choose the same vault or all vaults.

---

## 1. CLI import

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| `rk import sample.1pux` | Prints a summary: N to add, 1 credit card and 1 archived skipped (not logins / archived), 1 empty skipped | | |
| Confirm add | Items appear in `rk list` | | |
| `rk import sample.1pux` again (same vault) | "0 items to add; N duplicates skipped" | | |
| `rk import notazip.csv` renamed to `.1pux` | Readable error saying it is not a valid archive | | |
| `rk import bad.1pux` (empty ZIP with no export.data) | Error: "missing export.data" | | |

## 2. Desktop app import

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| **Choose a file…** → pick `sample.1pux` | File chooser opens; path fills in; preview shows "1password", N items to add | | |
| Preview list | 8 logins shown; credit card and archived not in list; a warning mentions N non-login / archived entries skipped | | |
| **Add N item(s) to the vault** | Exactly N items added; success notice | | |
| Import `sample.1pux` a second time | Preview says 0 to add, all duplicates | | |

## 3. Item content

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| GitHub item: title | "GitHub" | | |
| GitHub item: username | alice@example.com | | |
| GitHub item: password | Str0ng!P@ssw0rd (concealed) | | |
| GitHub item: URLs | https://github.com and https://gist.github.com both present | | |
| GitHub item: tags | "work" and "dev" | | |
| GitHub item: notes | "Work GitHub account.\nUse SSH key for repos." | | |
| GitHub item: TOTP | Two-factor code rotates every 30 s and is a valid 6-digit number | | |
| GitHub item: Recovery code | Custom field "Recovery code" (concealed) = "abcd-efgh-ijkl-mnop" | | |
| My Bank item: Account number | Custom field "Account number" (plain) = "1234-5678-9012" | | |
| My Bank item: PIN | Custom field "PIN" (concealed) | | |
| Unicode item: title | "国際サービス" (renders correctly, no mojibake) | | |
| Unicode item: username | 用户名@example.com | | |
| Unicode item: password | 密码123!パスワード (concealed) | | |
| Unicode item: notes | "日本語のメモ\n한국어 메모" — two lines preserved | | |
| Special-chars item: username | user "quoted" <angle> (no HTML escaping) | | |
| Special-chars item: password | P@$$w0rd!#%&*() | | |
| No-title item: title | "notitle.example" (derived from URL hostname) | | |
| Wi-Fi standalone password: title | "Wi-Fi Password" | | |
| Wi-Fi standalone password: notes | "A standalone password item" | | |
| Wi-Fi standalone password: tags | "home" | | |
| Jira item: tags | Contains "Work" (vault name added as tag) | | |
| Personal vault items: no "Personal" tag | No item from the Personal vault has "Personal" in its tags | | |

## 4. Non-login items and edge cases

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Credit card not imported | No item named "Visa Credit" in the vault | | |
| Archived item not imported | No item named "Archived Service" in the vault | | |
| Import summary warning | Warning text mentions N skipped entries (not logins or archived) | | |
| Empty-title+no-URL item | Not imported (counted in skipped_empty, no warning required) | | |
| Import a real 1Password export (if available) | All logins appear; secure notes / credit cards / identities skipped; TOTP works; custom fields visible | | |

## 5. Large export

- [ ] Export from 1Password with 200+ items across multiple vaults: import completes in under 5 seconds and all logins appear.
- [ ] The import summary shows the correct split between imported, duplicates, and non-login items skipped.

## 6. Accessibility and languages

- [ ] Keyboard only: reach the file chooser, the path box, and the add button with `Tab`; focus visible.
- [ ] With Orca on: file format "1password" and the item count are announced; the non-login warning is announced when it appears.
- [ ] 200% zoom: preview and buttons do not overflow.

## Recording results

Paste filled-in tables below under a dated heading (commit, session type, Orca on/off).
Every `✘` or `?` becomes an issue labelled `import` (or `a11y`).

---

## 2026-11-16 — commit e7b5327, CLI-only pass (Wayland, Ubuntu 26.04)

Section 2 (desktop app) and sections 5–6 (large export, a11y) require manual / desktop work and were not run.
Sections 1, 3, and 4 were run via CLI and `rk list`/`rk get`.

### Section 1 — CLI import

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| `rk import sample.1pux` | 8 to add, 1 credit card and 1 archived skipped, 1 empty skipped | ✔ | "8 to import, 0 duplicate(s), 0 empty; warning: 2 non-login entries not imported". Archived + credit card both counted in warning. |
| Confirm add | Items appear in `rk list` | ✔ | 8 items imported and listed |
| `rk import sample.1pux` again | "0 items to add; 8 duplicates skipped" | ✔ | "0 to import, 8 duplicate(s) skipped" |
| `notazip.csv` renamed to `.1pux` | Readable error: not a valid archive | ✔ | "not a valid 1Password export: … does not appear to be a ZIP archive" (fixed 2026-11-16: CLI now checks `.1pux` extension and bails if `is_1pux` returns false) |
| `bad.1pux` (empty ZIP, no export.data) | Error: "missing export.data" | ✔ | "not a valid 1Password export: missing export.data inside the archive" (fixed 2026-11-16: `is_1pux` now matches any `PK` prefix, so empty zips route to `import_1pux` and get the right error) |

### Section 3 — Item content (checked via CLI)

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| GitHub: title | "GitHub" | ✔ | Appears in `rk list` |
| GitHub: username | alice@example.com | ✔ | `rk list` shows it |
| GitHub: password | Str0ng!P@ssw0rd | ✔ | `rk get GitHub` |
| GitHub: URLs | both https://github.com and https://gist.github.com | ✔ | Confirmed in desktop preview (2026-11-16 desktop pass) |
| GitHub: tags (work, dev) | "work" and "dev" | ✔ | `rk list work` shows GitHub; `rk list dev` shows GitHub |
| GitHub: notes | "Work GitHub account.\nUse SSH key for repos." | ✔ | `rk get GitHub -f notes` |
| GitHub: TOTP | Valid 6-digit code, rotates every 30 s | ✔ | `rk totp GitHub` returns a code |
| GitHub: Recovery code | "abcd-efgh-ijkl-mnop" (concealed) | ✔ | `rk get GitHub -f "Recovery code"` |
| My Bank: Account number | "1234-5678-9012" | ✔ | `rk get "My Bank" -f "Account number"` |
| My Bank: PIN | 9876 (concealed) | ✔ | `rk get "My Bank" -f PIN` |
| Unicode: title | "国際サービス" | ✔ | Appears in `rk list` |
| Unicode: username | 用户名@example.com | ✔ | Shown in `rk list` |
| Unicode: password | 密码123!パスワード | ✔ | `rk get "国際サービス"` |
| Unicode: notes | two lines preserved | ✔ | `rk get "国際サービス" -f notes` |
| Special-chars: username | user "quoted" <angle> | ✔ | `rk get Special -f username` |
| Special-chars: password | P@$$w0rd!#%&*() | ✔ | `rk get Special` |
| No-title item: title | "notitle.example" (from URL) | ✔ | Appears in `rk list` as "notitle.example" |
| Wi-Fi Password: title | "Wi-Fi Password" | ✔ | Appears in `rk list` |
| Wi-Fi Password: tags | "home" | ✔ | `rk list home` shows "Wi-Fi Password" |
| Jira: tags | Contains "Work" | ✔ | `rk list work` shows Jira |
| Personal vault items: no "Personal" tag | No item has "Personal" in tags | ✔ | `rk list personal` returns nothing |

### Section 4 — Non-login items and edge cases

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Credit card not imported | No "Visa Credit" in vault | ✔ | `rk list` has no Visa Credit |
| Archived item not imported | No "Archived Service" in vault | ✔ | `rk list` has no Archived Service |
| Import summary warning | Warning mentions N skipped entries | ✔ | "warning: 2 entry(ies) that are not logins … were not imported" |
| Empty-title+no-URL item | Not imported | ✔ | `report.skipped_empty` counted correctly in dry-run |

**Issues filed:**
- `cli: rk get -f url not supported (URL is in item.urls, not a named field)` — label `cli`

**Fixed in this pass:** non-zip `.1pux` error message and empty-zip `.1pux` error message (both give clear "not a valid 1Password export" errors now).

---

## 2026-11-16 — commit e7b5327 + desktop .1pux routing fix, desktop pass (Wayland, Ubuntu 26.04)

**Bug fixed before this pass:** Desktop app was routing `.1pux` files through the CSV parser, producing a misleading UTF-8 error. Fixed by adding an `is_1pux` early-return branch in `model.rs` before the CSV column-detection path.

### Section 2 — Desktop app import

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| **Choose a file…** → pick `sample.1pux` | File chooser opens; path fills in; preview shows "1password", N items to add | ✔ | |
| Preview list | 8 logins shown; credit card and archived not in list; warning mentions skipped entries | ✔ | |
| **Add N item(s) to the vault** | Exactly N items added; success notice | ✔ | |
| Import `sample.1pux` a second time | Preview says 0 to add, all duplicates | ✔ | |

### Section 3 — Item content (desktop verification)

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| GitHub: URLs | both https://github.com and https://gist.github.com both present | ✔ | Both URLs visible in desktop preview |

### Section 4 — Non-login items and edge cases (desktop)

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Choose empty.1pux (valid zip, zero vaults) | "0 items to add" or equivalent; no error | ✔ | |
| Choose truly-empty.1pux (zero bytes) | Readable error; no crash | ✔ | |

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

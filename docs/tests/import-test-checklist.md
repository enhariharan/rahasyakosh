# Import screen test checklist (US-IMP-01, US-IMP-02)

A manual pass over **Settings → Import passwords** in the Linux desktop app. About 15 minutes. Use a throwaway vault
(`RK_VAULT=/tmp/rk-test/vault.db RK_CONFIG_HOME=/tmp/rk-test/cfg rahasyakosh`) so your real data is not touched. Run it with the
screen reader off, then repeat sections 2 and 3 with Orca on.

`✔` works · `✘` does not · `?` does something else (say what).

## Files to prepare
Put these in a folder such as `/tmp/rk-import/`:
- `chrome.csv`: `name,url,username,password,note` and two rows.
- `odd.csv`: `Site,Who,Secret,Misc` and two rows (no recognisable password column).
- `bitwarden.csv`: Bitwarden's columns, one `login` row with a `login_totp`, and one `note` row.
- `keepass.csv`: `"Group","Title","Username","Password","URL","Notes","TOTP"` and one row.
- `bitwarden.json`: an unencrypted export with one login, one secure note and one card.
- `encrypted.json`: `{"encrypted":true,"data":"x"}`.
- `empty.txt`: an empty file.

## 1. Choosing the file
| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| **Choose a file…** | The system file chooser opens | | |
| Choose `chrome.csv` | The path box fills in and the preview appears at once (format "chrome", 2 to add) | | |
| Type a path in the box instead and press **Show what would be added** | The same preview | | |
| Type `~/` plus a file name | The home folder is expanded | | |
| Choose `empty.txt` | A readable error; no preview | | |
| Choose a folder path by typing it | A readable error | | |

## 2. Formats
| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| `bitwarden.csv` | Format "bitwarden"; the note row is not added; a warning says 1 entry is not a login; the TOTP is kept | | |
| `keepass.csv` | Format "keepass"; the group shows up as a tag after importing | | |
| `bitwarden.json` | Format "bitwarden"; only the login is listed; a warning counts the 2 other entries; no column picker is shown | | |
| `encrypted.json` | An error that says the export is encrypted and asks for the unencrypted kind | | |
| Import the same file twice | The second preview says 0 to add and all rows are duplicates | | |

## 3. Choosing columns
| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| `odd.csv` | No list of items; a message says no password column was found; a column drop-down is shown for each kind of information, each with the file's headers and **(none)** | | |
| Choose Site for title, Who for username, Secret for password, then **Read the file again with these columns** | The preview lists the rows with the right titles and usernames | | |
| Change only the password drop-down on `chrome.csv` to another column | The preview reflects it (the first 50 rows show titles, usernames and websites; never passwords) | | |
| Set the website drop-down to **(none)** | The rows come back without websites | | |
| Cancel | The preview and the column pickers disappear | | |

## 4. Confirming
- [ ] **Add N item(s) to the vault** adds exactly N items; the notice says to delete the file because it holds passwords in plain text.
- [ ] Lock (Ctrl+L) while a preview is showing: after unlocking there is no preview (the passwords were dropped).
- [ ] Imported items open with the right title, username, website, notes, tags and (when present) a working two-factor code.

## 5. Accessibility and languages
- [ ] Keyboard only: reach **Choose a file…**, the path box, every column drop-down and both buttons with `Tab`; focus is visible; the file chooser opens with the keyboard.
- [ ] With Orca on: each drop-down is announced with its label (Title, Website, Username, Password, Notes, Two-factor code, Tags); the "no password column" message is announced when it appears; the summary line is announced.
- [ ] 200% zoom: the drop-downs and buttons do not overflow.
- [ ] Switch the language (Hindi, Tamil, Urdu...): the import labels and messages are translated and Urdu reads right to left. Native speakers: note any wrong or odd wording (the translations were written by the assistant and need review).

## Recording results
Paste the filled-in tables at the bottom of this file under a dated heading, with the commit, session type and Orca on or off.
Every `✘` or `?` becomes an issue labelled `desktop` (or `a11y`).

# Importing and exporting

Moving your logins in and out is a promise the project makes: **no lock-in** (principle 5 in
`docs/01-vision-and-scope.md`). It is also one of the most dangerous features, because an export
is, by definition, your whole vault *without* its protection. The code is in `crates/transfer`
(`rahasyakosh-transfer`), and the user-facing commands are in the CLI.

## Why a separate crate?

The CSV library is extra code we do not need in `rahasyakosh-core`, which should stay small and
focused on cryptography. The transfer crate only does *conversion*: bytes in, items out, and the
reverse. It never opens files, prompts, reads the clock or touches keys. The CLI does that part.

## Importing (US-IMP-01)

```
rk import passwords.csv --dry-run         # preview, change nothing
rk import passwords.csv                   # preview, ask "Import N item(s)? [y/N]"
rk import passwords.csv --yes             # no question
rk import odd.csv --map password=pwd --map title=site
```

**Column recognition.** Browsers and managers each name their columns differently. We match the
header names (ignoring case) against a table of aliases, for example `name`, `title` and `account` all
mean "title". This covers the exports of:

| Source | Typical columns |
|---|---|
| Chrome / Edge | `name, url, username, password, note` |
| Firefox | `url, username, password, httpRealm, formActionOrigin, guid, …` (no title; we use the website's name) |
| Generic and our own export | `title, url, username, password, notes, totp, tags` |

If a file uses names we do not know, `--map FIELD=COLUMN` points a field at a column. If no password
column can be found at all, the import stops with a message rather than guessing.

**Preview.** Before anything is stored you see each item's title, username and URL. **Passwords are
never printed.** Warnings are written without secrets too (a test checks this).

**De-duplication.** Importing the same file twice must not double your vault. A row is skipped as a duplicate
when its title, username, URL **and password** all match an existing item or an earlier row. If only the
password differs, it is kept (it may be a changed password you want to see) and you get a warning.
Items in the trash do not count.

**Bad data.** Empty rows are skipped; a TOTP secret that cannot be decoded is dropped with a warning
instead of failing the whole import; files over 50 MiB are refused.

## Exporting (US-IMP-03)

```
rk export -o backup.csv                   # plaintext CSV
rk export -f json -o backup.json          # plaintext JSON, includes every field
rk export -f txt -o passwords.txt         # plaintext, readable by a person, includes every field
rk export -f vault -o backup.rkv          # encrypted copy of the vault
```

### Plaintext exports: three safety rules

1. **A loud warning** is printed first.
2. **Re-authentication.** The master password is asked for *again* and checked against the vault,
   even though you already unlocked it. The reasoning: a vault that stays unlocked on screen,
   or a session an attacker walks up to, should not be able to dump everything with one command.
   The prompt needs a real terminal, so a script cannot bypass it, and `RK_PASSWORD` is deliberately
   not used for this step.
3. **Careful file handling.** The output file is created with owner-only permissions (`0600`) and the
   command **refuses to overwrite** an existing file unless you pass `--force`.

Trashed items are not exported.

### Which format?

| Format | Contains | Use it for |
|---|---|---|
| `json` | Everything (all fields, tags, timestamps) | A complete backup you can read |
| `txt` | Every field of every login, written out as readable text, sorted by title | Reading or printing your passwords yourself. The desktop app's **Settings → Export** writes this format |
| `csv` | Logins only: title, URL, username, password, notes, TOTP, tags | Moving to another manager |
| `vault` | An **encrypted** copy of the vault file | Backups. Safe to store anywhere; unlocks only with your master password |

For ordinary backups, prefer `vault`. It is still encrypted, so it needs none of the safety rules above.

### The text export (US-EXP-01)

`txt` is for people, not programs: a short warning at the top, then one block per login (title, username,
password, websites, two-factor secret, any other fields, tags, notes), sorted by title and separated by lines.
It follows the same three safety rules. The desktop app's **Settings → Export passwords to a text file**
asks for the master password again, suggests `~/rahasyakosh-export-<date>.txt`, refuses to replace an existing
file and creates the file readable by you only. `export_txt` in `crates/transfer` makes the text and does no
file work.

### A trap: spreadsheets

Opening a CSV in Excel or LibreOffice can treat a cell that starts with `=`, `+`, `-` or `@` as a
*formula* ("CSV injection"). We write values exactly as stored, because changing them would corrupt
passwords that legitimately start with those characters. So **open exports in a text editor**, and delete
the file when finished. Note that deleting a file is not guaranteed to erase it from an SSD.

## What to remember when changing this code

- Never print or log a password; check that warnings and previews stay secret-free.
- Keep plaintext in `Zeroizing` containers where possible. (Limit: the CSV library's internal buffers
  are not wiped, which is another reason the output file is the real exposure.)
- The importer is a parser of untrusted input: bound sizes, never panic on malformed rows, test with
  odd inputs (byte-order marks, embedded newlines and quotes, ragged rows).

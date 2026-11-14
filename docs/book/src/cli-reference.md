# CLI reference: `rk`

`rk` is the command-line tool for Rahasyakosh. It is **stateless**: every invocation
unlocks the vault, does one thing and exits, so your keys never linger in memory between
commands (US-CLI-01).

> **New to `rk`?** The [Quick start](quick-start.md) walks you through installing it and
> doing everyday tasks with worked examples. This page is a reference for the full set of
> options once you are up and running.

---

## Global options

These options can be placed before any subcommand.

| Option | Environment variable | Meaning |
|---|---|---|
| `--vault FILE` | `RK_VAULT` | Vault file to use. Default: `~/.local/share/rahasyakosh/vault.db` |
| `--profile USERNAME` | `RK_PROFILE` | Open the vault that belongs to this profile (made in the desktop app). `rk profiles` lists what is available |

---

## Vault setup

### `rk init`

Create a new, empty vault at the vault path.

```
rk init                   # choose your own master password
rk init --passphrase      # generate a random passphrase (recommended)
rk init --passphrase -w 8 # eight words instead of the default six
rk init --secret-key      # also lock the vault with a randomly-generated Secret Key
```

A generated passphrase is shown exactly once; write it on paper, then type it again when asked.
`rk` refuses passphrases shorter than five words (about 39 bits of entropy) and passwords the
strength meter rates below "fair" unless you add `--allow-weak`.

**There is no recovery.** A lost master password means a locked vault, forever.

| Option | Meaning |
|---|---|
| `--passphrase` / `-p` | Generate a random passphrase from the EFF large wordlist |
| `--words N` / `-w N` | Number of words (at least 5; requires `--passphrase`) |
| `--allow-weak` | Accept a master password rated below "fair" (not recommended) |
| `--secret-key` | Add a Secret Key: a second secret needed alongside the master password on each new device |

### `rk secret-key enable|show|disable`

Manage the vault's Secret Key — a second secret generated once and stored on each device that
uses the vault. It makes an offline brute-force attack much harder by adding randomness the
attacker cannot guess.

```
rk secret-key enable    # add a Secret Key; shown once — print it and keep it somewhere safe
rk secret-key show      # show the Secret Key again (asks for the master password)
rk secret-key disable   # remove the Secret Key; master password alone opens the vault again
```

The key is stored automatically in a per-device folder (`~/.local/share/rahasyakosh/keys/`)
so you are not asked for it on every command. You will be asked when you use a new device.
Set `RK_SECRET_KEY` to pass it without a prompt (useful in scripts).

### `rk passwd`

Change the master password. The items are not re-encrypted; only the key-wrapping changes.

```
rk passwd                # choose a new master password
rk passwd --passphrase   # replace it with a generated passphrase
```

### `rk profiles`

List the profiles (usernames and display names) that have a vault in the same folder as `--vault`.
Profiles are made in the desktop app.

---

## Adding and changing items

### `rk add TITLE`

Add a login.

```
rk add GitHub -u alice --url github.com
rk add GitHub -u alice --url github.com -g          # generate a password (printed once)
echo 'hunter2' | rk add GitHub -u alice --password-stdin
rk add GitHub -u alice --totp JBSWY3DPEHPK3PXP      # store a TOTP two-factor secret
rk add GitHub -u alice --totp "otpauth://totp/..."   # or an otpauth:// URI
```

| Option | Meaning |
|---|---|
| `--username` / `-u` | Username (default: empty) |
| `--url` | Website URL |
| `--generate` / `-g` | Generate a random 20-character password and print it once |
| `--password-stdin` | Read the password from standard input (one line; no prompt) |
| `--totp SECRET` | Base32 TOTP secret or `otpauth://` URI |

### `rk edit QUERY`

Change one or more fields of the best-matching item. Only the options you pass are changed.

```
rk edit github --username bob
rk edit github --password                     # prompt for a new password
rk edit github -g                             # replace it with a generated one (printed once)
rk edit github --url https://github.com
rk edit github --notes "work account"
rk edit github --tag work --tag dev           # add tags (repeatable)
rk edit github --untag dev                    # remove a tag (repeatable)
rk edit github --totp JBSWY3DPEHPK3PXP
rk edit github --remove-totp
rk edit github --favourite                    # pin it; listed first and marked ★
rk edit github --no-favourite
```

| Option | Meaning |
|---|---|
| `--title` | New title |
| `--username` / `-u` | New username |
| `--url URL` | Replace the URLs with this one; pass an empty string (`""`) to clear them |
| `--notes TEXT` | Notes (plain text) |
| `--password` | Prompt for a new password |
| `--password-stdin` | Read the new password from stdin |
| `--generate` / `-g` | Generate a new password (printed once) |
| `--totp SECRET` | Set the TOTP secret |
| `--remove-totp` | Remove the TOTP secret |
| `--tag TAG` | Add a tag (repeatable) |
| `--untag TAG` | Remove a tag (repeatable) |
| `--favourite` | Pin the item (shown first, marked ★) |
| `--no-favourite` | Unpin it |

---

## Finding and reading items

**How queries work.** `rk` matches your text against each item's title, URL, tags and
non-secret fields. A title that *equals* your text (case-insensitive) wins outright. Otherwise
the text may appear anywhere in those fields; if more than one item matches, `rk` stops and
lists them so you can try a longer name.

### `rk list [TEXT]`

List all items, or the items that match TEXT. Each line shows the title, username and a ★ for
favourites. Favourites are shown first.

```
rk list
rk list github
rk list work          # any item tagged "work", or whose title/URL contains "work"
```

### `rk get QUERY`

Print the value of a field (password by default), or copy it to the clipboard.

```
rk get github                           # print the password
rk get github -f username               # print the username
rk get github -f notes
rk get github --copy                    # copy the password; clipboard clears in 20 s
rk get github --copy --clear-after 60   # clear after 60 s instead
rk get github --copy --clear-after 0    # never clear
```

| Option | Meaning |
|---|---|
| `--field FIELD` / `-f FIELD` | Field to show (default: `password`). Use `notes` for the notes field |
| `--copy` / `-c` | Copy to the clipboard and clear it after a delay instead of printing |
| `--clear-after SECS` | Seconds before the clipboard is cleared (default: 20; 0 = never; requires `--copy`) |

> **Terminal safety.** When printing to a terminal, control characters in the stored value are
> written spelled out (e.g. `\x1b`) so that a crafted value cannot drive the terminal. When piped
> to a file or another program, the value is written exactly as stored.

### `rk totp QUERY`

Print the current TOTP code for a two-factor secret, or copy it.

```
rk totp github                      # print the code and how many seconds it has left
rk totp github --copy               # copy it; clip clears in 20 s
```

---

## Deleting and recovering items

### `rk rm QUERY`

Move an item to the trash. It is hidden from `rk list` and `rk get` but can be recovered.

```
rk rm github
rk rm github --purge    # delete permanently, including from the trash
```

### `rk trash`

List the trashed items, with how long ago each was deleted.

```
rk trash
rk trash --purge-expired    # permanently delete items past their 30-day retention (asks first)
```

Items in the trash are deleted automatically 30 days after they were deleted, the next time you
use `rk`. The retention period is set in the desktop app's settings (minimum: 1 day).

### `rk restore QUERY`

Bring an item back from the trash.

```
rk restore github
```

---

## Item history

Every time you change or delete an item, the previous version is kept. `rk history` shows the
list (fields that changed are named, never their values). `rk revert` makes an earlier version
current, while also keeping the version it replaced.

```
rk history github          # list earlier versions: when replaced, which parts changed
rk revert github 1         # make version 1 current again
```

The number of versions kept per item is set in the desktop app's settings (default: 10).

---

## Generating passwords

### `rk gen`

Generate a password or passphrase, without touching any vault.

```
rk gen                              # 20 random characters
rk gen -l 32                        # 32 characters
rk gen --no-symbols                 # letters and digits only
rk gen --avoid-ambiguous            # skip look-alikes: O 0 I l 1
rk gen --passphrase                 # six random EFF words, e.g. "correct-horse-battery-staple-moon-river"
rk gen -p -w 7                      # seven words
rk gen -p -s . -c                   # words separated by ".", each capitalised
```

| Option | Meaning |
|---|---|
| `--length N` / `-l N` | Character count (default: 20) |
| `--no-symbols` | Letters and digits only |
| `--avoid-ambiguous` | Exclude characters that look like each other: `O`, `0`, `I`, `l`, `1` |
| `--passphrase` / `-p` | Generate a passphrase from the EFF large wordlist instead |
| `--words N` / `-w N` | Number of words (default: 6; requires `--passphrase`) |
| `--separator S` / `-s S` | Separator between words (default: `-`; requires `--passphrase`) |
| `--capitalize` / `-c` | Capitalise the first letter of each word (requires `--passphrase`) |

---

## Importing and exporting

See [Importing and exporting](import-export.md) for a thorough explanation. The quick forms:

```
rk import passwords.csv --dry-run         # preview without changing anything
rk import passwords.csv                   # preview and confirm
rk import passwords.csv -y                # no confirmation prompt
rk import odd.csv --map password=pwd      # tell rk which column holds the password

rk export -o backup.csv                   # plaintext CSV
rk export -f json -o backup.json          # plaintext JSON (every field)
rk export -f txt -o passwords.txt         # plaintext, readable, every field
rk export -f vault -o backup.rkv          # encrypted copy (safe to store anywhere)
```

Plaintext exports warn loudly and ask for the master password again before writing anything.

---

## Migrating an old vault

Old versions of Rahasyakosh stored the vault as a JSON file (`vault.rkv`). `rk migrate` reads
it into a new SQLite vault. Your master password does not change; the old file is left untouched.

```
rk migrate old-vault.rkv --vault new-vault.db
```

---

## Shell completions

`rk completions` prints a completion script for your shell. Source it once and pressing Tab
will complete subcommands and flags (US-CLI-01).

```
# bash
rk completions bash >> ~/.bash_completion

# zsh (put the file in a directory on your $fpath, e.g. ~/.zfunc)
rk completions zsh > ~/.zfunc/_rk
# then, once: echo 'fpath=(~/.zfunc $fpath)' >> ~/.zshrc && echo 'autoload -U compinit && compinit' >> ~/.zshrc

# fish
rk completions fish > ~/.config/fish/completions/rk.fish
```

After sourcing the script, start a new shell session (or `source` the file) and Tab completion
is active.

---

## Environment variables

| Variable | Meaning |
|---|---|
| `RK_VAULT` | Vault file path. Same as `--vault FILE` |
| `RK_PROFILE` | Profile username. Same as `--profile USERNAME` |
| `RK_PASSWORD` | Master password, for scripts. **Do not set this in your shell configuration**: it keeps your master password in every process's environment |
| `RK_SECRET_KEY` | Secret Key (`RK1-XXXX-…`), for scripts. Same caution as `RK_PASSWORD` |

---

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Error (message printed to standard error) |

---

## Scripting tips

`rk` is designed to be used in scripts as well as interactively.

**Read the password from an environment variable (non-interactive unlock):**
```bash
export RK_PASSWORD="my master password"
rk get github -f username    # no prompt
```

**Copy a field to a variable:**
```bash
pw=$(RK_PASSWORD="$MASTER" rk get github)
```

**Pipe a new password in:**
```bash
openssl rand -base64 24 | rk add "Generated" -u svc --password-stdin
```

**Check whether a title already exists:**
```bash
if rk list "github" | grep -qi "^github"; then
  echo "found"
fi
```

The exit code is 0 on success and 1 on any error. Standard output carries the result; standard
error carries all messages (`Copied`, `Added`, `Error: …`), so a pipe to another program sees
only the value.

**`RK_PASSWORD` in scripts.** The variable is read once per command; there is no persistent
session, so it cannot be used to unlock a session that other processes share. Each command
unlocks independently with the same key derivation cost as an interactive unlock.

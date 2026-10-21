# Quick start: using `rk`

This chapter is for using the command-line tool `rk` to store passwords. The rest of the book explains why it works the way it does.

> Unsure what a word means (vault, passphrase, trash)? See the [Glossary](glossary.md).

> **Not audited yet.** Nobody independent has reviewed this software. Read [Before you store real passwords](#before-you-store-real-passwords) first.

## Install

You need a Rust toolchain (<https://rustup.rs>). From the repository:

```
cargo install --path crates/cli      # installs `rk` into ~/.cargo/bin (make sure that is on your PATH)
rk --version
```

For the clipboard commands, your desktop needs one clipboard tool: `sudo apt install wl-clipboard` on Wayland (the Ubuntu default,
usually already present) or `sudo apt install xclip` on X11.

To update later, `git pull` and run the same `cargo install` command again.

> Prefer a window to a terminal? There is a desktop app that uses the same vault: see [The desktop app](desktop-app.md).

## Create your vault

```
rk init --passphrase
```

This creates a vault and generates a **master passphrase** of six random words. It is shown **once**: write it on paper, then type it again when
asked. If you prefer to choose your own master password, run `rk init`; it shows a strength rating and refuses anything rated below
"fair" unless you add `--allow-weak`.

**There is no recovery.** If you lose the master passphrase, nobody can open your vault, including the authors.

The vault is one file, `~/.local/share/rahasyakosh/vault.db`, readable only by you. Use another file with `--vault FILE`, or set `RK_VAULT`.
Every command asks for the master passphrase and takes a fraction of a second to check it.

## Everyday use

### Add a login

```
rk add GitHub -u alice --url github.com            # asks for the password (hidden)
rk add GitHub -u alice --url github.com -g         # generates a strong password and prints it once
echo 'the-password' | rk add GitHub -u alice --password-stdin
rk add GitHub -u alice --totp JBSWY3DPEHPK3PXP     # also store a two-factor secret
```

### Find and read

```
rk list                  # everything, sorted by title
rk list work             # items whose title, URL, tag or non-secret field contains "work"
rk get github --copy     # copy the password to the clipboard; it is cleared after 20 seconds
rk get github -f username --copy
rk get github            # print the password (it stays in your terminal scrollback)
rk get github -f notes
rk totp github --copy    # the current two-factor code
```

**How a name is matched:** a title that equals your text (ignoring case) wins. Otherwise the text may appear in the title, URL, tags or any non-secret field.
If several items match, `rk` stops and lists them instead of guessing; use a longer name.

### Change an item

Only the options you give are changed.

```
rk edit github --username bob
rk edit github --password                # prompts for a new password
rk edit github -g                        # replaces it with a generated password and prints it once
rk edit github --url https://github.com --notes "work account"
rk edit github --tag work --tag dev      # add tags; --untag removes
rk edit github --totp JBSWY3DPEHPK3PXP   # or --remove-totp
```

### Delete, and get things back

```
rk rm github             # moves to the trash (hidden from `rk list`)
rk trash                 # what is in the trash, and how long ago it was deleted
rk restore github        # bring it back
rk rm github --purge     # delete for good (also works on trashed items)
```

Items stay in the trash for 30 days, then are deleted automatically the next time you use `rk`.

### History and favourites

```
rk history github          # earlier versions: which parts changed (never the secret values)
rk revert github 1         # make version 1 current again (the present content is kept too)
rk edit github --favourite # pin it: favourites are listed first, marked with ★ ( --no-favourite removes)
```

### Passwords and passphrases

```
rk gen                              # 20 random characters
rk gen -l 32 --no-symbols           # longer, letters and digits only
rk gen --avoid-ambiguous            # no look-alike characters (O 0 I l 1)
rk gen --passphrase                 # six random words, e.g. scorebook-exit-floss-crevice-google-ouch
rk gen -p -w 7 -s . -c              # seven words, "." between, capitalised
rk passwd                           # change the master password (items are not re-encrypted)
rk passwd --passphrase              # ... to a new generated passphrase
```

## Backups and moving data

```
rk export -f vault -o ~/backup.rkv       # an encrypted copy: safe to store anywhere
rk export -f json -o all.json            # PLAINTEXT, asks for your master password again
rk export -o logins.csv                  # PLAINTEXT CSV
rk import chrome-passwords.csv --dry-run # preview what an import would add (passwords are never shown)
rk import chrome-passwords.csv           # ask, then import; duplicates are skipped
```

Plaintext exports contain every password. Delete them as soon as you are done, and read [Importing and exporting](import-export.md).
**Make an encrypted backup regularly and keep it somewhere other than this computer.** To restore a backup, load it into a new vault with `rk migrate backup.rkv --vault new-vault.db`.

## Command cheat sheet

| Command | What it does |
|---|---|
| `rk init [--passphrase]` | Create a vault |
| `rk add TITLE [-u USER] [--url URL] [-g] [--password-stdin] [--totp SECRET]` | Add a login |
| `rk list [TEXT]` | List or search items |
| `rk get TEXT [-f FIELD] [-c] [--clear-after SECS]` | Show or copy a field (default: password) |
| `rk totp TEXT [-c]` | Current two-factor code |
| `rk edit TEXT [options]` | Change an item |
| `rk rm TEXT [--purge]` | Trash an item, or delete it for good |
| `rk trash` / `rk restore TEXT` | List the trash / bring an item back |
| `rk history TEXT` / `rk revert TEXT N` | Earlier versions / go back to version N |
| `rk migrate FILE` | Load an old-format vault or a backup bundle into a new vault |
| `rk gen [--passphrase]` | Generate a password or passphrase |
| `rk passwd [--passphrase]` | Change the master password |
| `rk import FILE [--map F=COL] [--dry-run] [-y]` | Import CSV (Chrome, Firefox or generic) |
| `rk export [-f csv\|json\|vault] [-o FILE] [--force]` | Export |

Add `--help` to any command. Every command also takes `--vault FILE`.

## Before you store real passwords

- **It has not been audited.** The cryptography is standard and carefully tested (see [Testing with known answers and properties](testing-with-known-answers.md)), but no independent expert has reviewed
  it. Until then, keep another password manager as your main copy.
- **A lost master passphrase cannot be recovered.**
- **`rk get` without `--copy` prints the password**, which then sits in your terminal scrollback. Prefer `--copy`.
- **Do not set `RK_PASSWORD` in your shell configuration.** It exists for scripts and tests, and would keep your master password in your environment.
- **Back up** with `rk export -f vault`. Restoring a backup is only possible if you still know the master passphrase it was made with.
- **If you used an earlier version** your vault is an old-format file (`vault.rkv`). Convert it once:
  `rk migrate ~/.local/share/rahasyakosh/vault.rkv --vault ~/.local/share/rahasyakosh/vault.db`. Your passwords and master password stay the same, and the old file
  is left untouched; keep it until you have checked the new vault. `rk` tells you when it meets an old file.

## When something goes wrong

| Message | Meaning |
|---|---|
| `decryption failed: wrong password or corrupted data` | Wrong master password, or the file was damaged. The program deliberately does not say which |
| `master password is too weak` | Choose a longer one, use `--passphrase`, or accept the risk with `--allow-weak` |
| `'x' is ambiguous: A, B` | More than one item matches; use a longer name |
| `no item matches 'x'` | Nothing live matches. Check `rk trash`; `rk restore` brings an item back |
| `... is in the old vault file format` | Run `rk migrate` as the message says |
| `no vault at ...` | Create one with `rk init`, or check `--vault` / `RK_VAULT` |
| `no clipboard tool found` | Install `wl-clipboard` (Wayland) or `xclip` (X11) |
| `nothing to change` | You ran `rk edit` without saying what to change |
| `... already exists` | `rk init` will not overwrite a vault. Use `--vault` for a different file |

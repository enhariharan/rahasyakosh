# Privacy and data storage

Rahasyakosh is designed so that your secrets never leave your device. This page
says exactly what is stored, where, and what the app can and cannot do.

## No network, no telemetry, no accounts

- Rahasyakosh **never connects to the internet**. There is no sync server, no
  update check, no crash reporter, no analytics endpoint.
- It **does not require an account** of any kind. No email address, no
  registration, no licence key.
- It **sends nothing** about you or your vault to Anthropic, the author, or
  anyone else.

You can verify this yourself: `ss -tnp` or `nethogs` will show no outgoing
connections while the app is running.

You will see one **local** connection to `127.0.0.1` (the loopback address,
which never leaves your computer). The app's window is a small web view, and
the program sends screen updates to it over this connection. It is protected by
a secret key that changes on every start. This is also why the snap asks for the
`network-bind` permission: that lets the app open a socket on your own computer.
The snap does **not** have the `network` permission, so it cannot reach the
internet.

## What is stored and where

All Rahasyakosh data lives under one folder:

```
~/.local/share/rahasyakosh/
```

| Path | Contents | Encrypted? |
|------|----------|------------|
| `profiles.json` | Profile list: each profile's internal ID, **full name** and **username** | No |
| `<uuid>/vault.db` | All items, history, trash | Yes (see below) |
| `secret-keys/<uuid>.key` | Your secret key, if you use one | Yes (key itself is a secret) |

Desktop display preferences (colour scheme, language, auto-lock time) are stored
in `~/.config/rahasyakosh/`. They contain no vault data.

**If you installed the snap**, the same files live under the snap's own folder
instead, `~/snap/rahasyakosh/common/`: the vault folder is
`~/snap/rahasyakosh/common/.local/share/rahasyakosh/` and the preferences are in
`~/snap/rahasyakosh/common/.config/rahasyakosh/`. A snap install and a `.deb`
install therefore have separate vaults; move items between them with export and
import. The `common` folder is shared by every snap revision, so going back to an
older version with `snap revert` does not roll your vault back. Removing the snap
deletes this folder (snapd keeps an automatic snapshot for 31 days), so make a
backup first.

**`profiles.json` is not encrypted.** The names and usernames you chose for your
profiles are readable by anyone with access to your home directory. Avoid using
real-name information there if that matters to you.

## What is encrypted

Everything inside `vault.db` is encrypted with AES-256-GCM, keyed from your
master password (and secret key if you use one). That includes:

- item titles, usernames, passwords, URLs, notes, tags
- one-time code seeds (TOTP secrets)
- item history and timestamps
- trash contents

The database stores only the **size** (rounded to 64-byte blocks) and
**revision number** of each record in the clear, plus the locked vault header.

For the full cryptographic details see [How a vault works](how-a-vault-works.md)
and [The cryptography toolbox](crypto-toolbox.md).

## The clipboard

When you copy a password from the desktop app or with `rk get --copy`, the app
clears the clipboard after a delay (20 seconds by default, configurable in
Settings).

**What clearing cannot do:**

- **Clipboard managers and history tools** (GNOME's GPaste, KDE Klipper,
  third-party apps) may capture a copy before the app clears it. Disable
  clipboard history in those tools if you use a password manager.
- **Wayland compositors** may keep a clipboard history that the app cannot reach
  with the standard `wl-copy` / `wl-paste` protocol. The app clears what it
  can; compositor-level history is outside its control.
- On **X11** the clipboard is owned by the process that wrote it. Clearing works
  as long as the helper process (`rk clipboard-clear`) stays alive. If the
  helper is killed before the timeout, the clipboard is not cleared.

The app never prints passwords to standard output. The only way a password
leaves the vault is through the clipboard or through an explicit export.

## Exports

The desktop app can export your vault to a plain-text file, CSV, or JSON. These
files are **not encrypted**. The app asks for your master password again before
exporting and writes the file to wherever you choose. Delete export files when
you are done with them; they contain all your passwords in the clear.

## WebKit local storage

The desktop app is built with WebKitGTK. WebKit may create its own storage
files for caching and local state under:

```
~/.local/share/rahasyakosh/webview/
```

Rahasyakosh does not use JavaScript local storage or cookies for vault data.
The WebKit storage, if present, holds only UI state (scroll position, etc.).

## The secret key

If you add a secret key to your vault, the key is stored in
`secret-keys/<uuid>.key`. This file is readable only by you (`0600`). The key
is a random 128-bit value; losing it means losing access to the vault on any
device that does not already have the key file. See
[Backing up and recovering your vault](backups.md).

## Shared computers

On a multi-user system, vault files and settings are created with mode `0600`
(files) and `0700` (directories), so other users cannot read or even list them.
The app repairs incorrect permissions on startup.

If you use a shared computer, lock the vault when you step away (`Ctrl+L`) and
consider enabling the auto-lock timer in Settings.

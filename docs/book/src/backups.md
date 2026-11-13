# Backing up and recovering your vault

Rahasyakosh does not sync or upload your data anywhere. That means **you** are
responsible for backups. This chapter explains what to back up, how to restore
it, and what cannot be recovered if you lose it.

## What to back up

Everything Rahasyakosh stores lives in one folder:

```
~/.local/share/rahasyakosh/
```

That folder contains:

| Path | What it is |
|------|------------|
| `profiles.json` | Your profile list: names and usernames (not encrypted) |
| `<uuid>/vault.db` | Your vault: all items, history and trash, encrypted |
| `secret-keys/<uuid>.key` | Your secret key, if you chose one (see below) |

Back up the **entire folder**. Everything is either encrypted or non-secret, so
you can store the backup anywhere you would store a file.

> **Desktop-app settings** (`~/.config/rahasyakosh/`) hold display preferences
> only, not vault data. You do not need to back them up.

## How often to back up

Back up after any session where you added or changed something important. A
simple approach: copy the folder to an external drive or a cloud service after
each use.

If you use a folder-sync tool (Syncthing, Dropbox, rclone), point it at
`~/.local/share/rahasyakosh/`. Sync is not built into the app yet, but the
folder is safe to sync: each file is either encrypted or only holds
non-sensitive names.

## Restoring from a backup

Copy the folder back to `~/.local/share/rahasyakosh/` on the new machine, then
open the app or run `rk`. The vault opens with your master password as before.

If you use a **secret key** (see below), the key file must be present too. If
you used a different device for the backup, copy
`secret-keys/<uuid>.key` from the backup into the same path on the new machine.

## Migrating to a new computer

1. Copy `~/.local/share/rahasyakosh/` from the old computer to the same path on
   the new one.
2. Install Rahasyakosh on the new computer (`sudo apt install ./rahasyakosh_*.deb`).
3. Open the app. Your profiles appear and your vault unlocks with your master
   password.

## The secret key

If you added a secret key to your vault, two things are needed to unlock it:

1. Your master password (you remember it)
2. Your secret key (the app loads it from `secret-keys/<uuid>.key`)

On the same device, the key file is loaded automatically and you are only asked
for the password. On a **new device** the key file is not there yet, so the app
asks for both.

**Back up the key sheet.** When you first created the secret key, the app showed
a key sheet with the key text. If you printed or saved it, that is your recovery
copy. Store it somewhere safe and separate from your vault backup (the whole
point is that the two secrets live apart).

If you have the key text, you can enter it when the new device asks. The app
then saves it to `secret-keys/<uuid>.key` so you are not asked again.

## What cannot be recovered

| Lost thing | Can it be recovered? |
|------------|---------------------|
| Master password | **No.** Rahasyakosh uses one-way key derivation (Argon2). There is no reset, no recovery email, and no backdoor. The vault cannot be opened without it. |
| Secret key (no backup) | **No.** If you lose both the key file and the key sheet, the vault cannot be opened. |
| Vault file | **No**, without a backup. The vault exists only on your device. |

This is a deliberate design: only you can open your vault. The flip side is that
only you can lose access to it. Keep a backup.

## Checking a backup

Open a file manager and confirm the folder exists and is non-empty. For extra
confidence, copy the backup to a test machine (or a different path with
`RK_VAULT` and `RK_CONFIG_HOME` set to point at it) and verify the vault opens:

```sh
RK_VAULT=/path/to/backup/uuid/vault.db \
RK_CONFIG_HOME=/path/to/backup \
rk list
```

If `rk list` shows your items, the backup is good.

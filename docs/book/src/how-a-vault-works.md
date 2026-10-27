# How a vault works

This chapter follows your data from the master password to the file on disk. The authoritative
specification is `docs/04-security-and-crypto.md`; the code is in `crates/core/src/vault.rs`.

## The key hierarchy

Instead of encrypting everything directly with a key derived from your password, we use layers:

```
master password
      │  Argon2id (+ random salt)
      ▼
master key ──HKDF "rk/kek/v1"──► KEK  (key-encrypting key)
                                  │ unlocks
                                  ▼
                       wrapped vault key (stored in header)
                                  │ decrypts to
                                  ▼
                         VK  (random 32-byte vault key)
                                  │ HKDF "rk/item/v1" + item id
                                  ▼
                         one key per item
```

**Why the extra layer?** The vault key (VK) is random and never changes. The password-derived key
(KEK) only *wraps* it, like a safe containing the real key. When you change your master password,
we re-wrap the small vault key under a new KEK. We do **not** need to re-encrypt every item. That
is `UnlockedVault::change_password`.

**Why is the password "normalised"?** The same word can be stored as different bytes. An `é` can be one
character, or an `e` followed by a separate accent mark, and many Hindi, Tamil, Telugu, Malayalam and Urdu letters can be built in more
than one way too. Which one you get depends on the keyboard or input method. Before the password is turned into a key, the program
converts it to one standard form (called NFKC), so the same password you can see always opens the vault. The header records this
(`pw_norm`). Vaults made earlier still open: the program tries your typed bytes first, then the standard forms.

**Why a key per item?** If something ever went wrong with one item's key, only that item is
affected. It also means a record is meaningless without its own id.

## What is stored on disk

A vault (a SQLite database file, see [Where your data lives](storage.md)) has two parts:

1. **Header** (readable, not secret): format version, a vault id, the Argon2 settings, the salt,
   and the **wrapped vault key**. None of this helps an attacker without the password.
2. **Records**: one per item. Each has an item id, a revision number, a nonce, and the
   **ciphertext**. The item's title, username, password, notes and everything else are *inside*
   the ciphertext.

The same two parts, written out as one JSON file, form the portable *bundle* that `rk export -f vault` produces and `rk migrate` reads.

Before encryption, each item is padded to a multiple of 64 bytes so that file sizes do not
reveal, say, how long your password is.

The database is written safely: changes go through a log and a transaction (so a crash cannot leave half a vault) and the file is readable only by you (`0600`).

## Unlocking

1. Read the header; check the Argon2 settings are within allowed bounds.
2. Run Argon2id on the password with the stored salt. Derive the KEK.
3. Try to decrypt the wrapped vault key. If the password is wrong, authentication fails and we
   return one generic error — no hints.
4. Hold the vault key in memory (`UnlockedVault`) until it is dropped, at which point it is wiped (see
   [Locked and unlocked](#locked-and-unlocked)).

## Locked and unlocked

Your vault file is always encrypted on disk. **Unlocked** means a program has used your master password to
work out the vault key and holds it in memory, so it can decrypt items without asking again. **Locked**
means that key has been erased from memory; only the encrypted file remains, and getting back in needs the
master password. Locking matters because a key in memory can be used by anything able to read that memory
(malware, or someone at your open laptop). The shorter the key exists, the smaller that window. Apps lock
after idle time, on sleep, or when you press a button.

**The command-line tool never stays unlocked.** Every `rk` command asks for the master password, unlocks,
does its job and exits, and the key is wiped as the process ends (`UnlockedVault` zeroizes on drop). That is
the safest design, which is why there is no `rk lock`: there is nothing to lock. The cost is typing the
password each time.

A convenient "unlock once, work for a while" mode needs a long-lived **session** (now built for the desktop app: see [Sessions](sessions.md)) that holds the key and
forgets it after a timeout, much as `ssh-agent` does for SSH keys. That is new security-sensitive code with its
own risks (who may talk to it, how long it lives, how it wipes), and the desktop app needs the same thing. So we
build it once, in the M3 session crate, rather than twice. See story US-VLT-04.

## Authenticated context: why the AAD matters

Every encryption includes **associated data** that is checked but not hidden:

- The header's AAD covers the format version, vault id, Argon2 settings and salt. If anyone edits
  those fields (for example to weaken the settings), unwrapping the key fails.
- A record's AAD covers the vault id, item id, revision and record version. If an attacker swaps
  two records, or replays an old version of one, decryption fails.

This is a powerful trick: **the context is part of the lock.**

## The Secret Key (optional)

A master password is chosen by a person, so it can be weak. A **Secret Key** is 128 random bits made by the program, written like
`RK1-ABCD-EFGH-...` (eight groups of four letters and digits; the last part is a checksum, so a typing mistake is caught at once instead of
looking like a wrong password). It is mixed into the key that locks the vault key, so opening the vault needs **both** the master
password and the Secret Key.

The point: someone who steals only your vault file cannot even start guessing master passwords, because they would have to guess a
128-bit random value first. The Secret Key is **not** inside the vault file. Each computer keeps its own copy in a private file in
`~/.local/share/rahasyakosh/secret-keys/`, and you keep a printed copy for a new device.

- Make a vault with one: `rk init --secret-key`. Add it to an existing vault: `rk secret-key enable`. Show it again: `rk secret-key show`.
  Remove it: `rk secret-key disable`.
- On a new computer, `rk` asks for it once (or reads `RK_SECRET_KEY`) and then keeps it on that computer.
- **If you lose the Secret Key and every device that has it, the vault cannot be opened, even with the right master password.**
  That is the price of the protection. Print it and keep it somewhere safe.
- The desktop app uses the key kept on the computer, but cannot yet take a typed one; on a new computer, run `rk list` once with
  `RK_SECRET_KEY` set.

## Two things this design does not do

- It cannot protect you from malware running as you while the vault is unlocked. See
  [What we defend against](threat-model.md).
- It cannot rescue a weak master password. Argon2 makes guessing expensive, not impossible.
  The optional **Secret Key** helps: extra random entropy that is kept on your devices and not in the vault file (see below).

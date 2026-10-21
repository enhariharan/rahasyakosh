# Where your data lives: the store

This chapter explains how a vault is kept on disk, how deleting, history and search work, and how the program protects against crashes and
mistakes. It is the heart of milestone M2. (New words are in the [Glossary](glossary.md).)

## Three layers

```
  rk (the command)        decides what to do: "add this", "search that"
        │
  rahasyakosh-app         the unlocked vault: keeps decrypted items in memory, search, trash, history
        │      ╲
  rahasyakosh-store        saves and loads encrypted records        rahasyakosh-core: keys, encryption, item model
```

The key idea: **the store never has a key.** It saves and loads scrambled records and cannot read them. A bug in the store can lose or
corrupt data, which we test hard against, but it cannot leak a password. The `app` layer holds the keys, decrypts, and gives the front-ends
(the command line now, the desktop app later) one tested set of operations.

## What a store keeps

Per vault, a store keeps:

- the **header** (the readable, non-secret settings and the locked vault key),
- one **record** per item (scrambled), with its id and a **revision** number,
- a **tombstone** for each item that was deleted for good (just its id and the revision it was deleted at).

A thief who copies the storage learns how many items there are, their random ids, revision counts and rough sizes (rounded up to 64-byte
groups). Titles, usernames, URLs, notes, timestamps and whether an item is in the trash are all inside the scrambled part.

## Three backends

A **backend** is one way of keeping those things. They all follow one interface, so the rest of the program does not care which is used.

| Backend | What it is | Used for |
|---|---|---|
| **SQLite** | One database file, `vault.db` | The default for `rk` and the future desktop app |
| **Directory** | A folder with one small file per item | Folder-sync tools (Syncthing, Dropbox) later, in milestone M4 |
| **Memory** | Nothing on disk | Tests, and later the web app |

### Why SQLite

SQLite is a small, very widely used database that lives in one file and is famous for not losing data. We use the copy bundled inside our
program, so you do not need to install anything. We configure it for safety rather than speed:

- **Write-ahead log.** Changes are written to a log first, so a crash or power cut can never leave the database half-written.
- **Full sync.** A write is not reported as done until the operating system confirms it reached the disk.
- **Secure delete.** When a record is deleted, its old bytes are overwritten with zeros instead of being left in the file's free space. A test
  proves this: it stores a recognisable pattern, deletes the record, and searches the file for the pattern.
- **Private files.** The database and its helper files are readable only by you (mode `0600`).
- **One transaction per change.** "Check that nobody changed this item, then write" happens as one indivisible step.

## Two terminals at once: revisions

Every record has a **revision number** that goes up with each change. A store accepts a write only if its revision is higher than the one stored. So if two
programs edit the same item at the same time, the second one is told "someone else changed this" instead of silently erasing the first one's work. In
the CLI that appears as an error; run the command again and you will see the newer version.

## Trash, deleting and auto-delete

`rk rm` marks an item as deleted *inside* its scrambled data and keeps it; that is the **trash**. `rk restore` undoes it. `rk rm --purge` removes the record for
good and leaves a **tombstone**, a tiny marker so that, once syncing exists, other devices can learn the item was deleted and not bring it back.

Items left in the trash for **30 days** are deleted automatically. This happens whenever the vault is unlocked, and not inside the store, for a plain reason: the
time an item was trashed is part of the scrambled data, so only the unlocked program can read it.

## History

Each time an item's content changes, its previous content is saved inside the item, up to 10 earlier versions, newest first.

```
rk history github          # see the versions (what changed, never the secret values)
rk revert github 1         # make version 1 current again
```

Reverting never rewrites the past: the content you leave is saved first, so you can always undo a revert. Old passwords in history are protected exactly like
the current one: same key, same scrambled record. Purging an item removes its history with it. Because an item's history lives *inside* the item, one item is still one
record, which keeps saving and (later) syncing simple.

## Search

When you unlock, the program decrypts every item once and builds a search index **in memory only**; nothing searchable is ever written to disk. You can search for several words
(`rk list work github`), and every word must match something in the item's title, URL, tags or non-secret fields. Passwords, other concealed fields and notes are
**never** searched, so nothing secret can be found by trial searching. The index is wiped, like the keys, when the program exits.

Searching is a plain scan, which is fast enough: see the numbers below.

## Upgrading the storage format safely: migrations

Programs change. Sometimes a new version needs a different table layout. A **migration** upgrades an existing database. The rules:

1. The database records its **schema version** (SQLite's `user_version`).
2. On opening, the program runs every upgrade step from the stored version up to the current one, **each inside a transaction**. If a step fails, the database is left exactly as it was.
3. A database from a **newer** program is refused outright ("upgrade rk") rather than guessed at or damaged.
4. For every released schema version a **fixture** (a sample database, `spec/vectors/schema-v1.sqlite`) is kept forever. A test upgrades each fixture and checks that the data is intact. Like the golden vault, a fixture is never edited.

### Your old vault file

Until now `rk` kept a vault as one JSON file. That format still exists as the portable **bundle**: it is what `rk export -f vault` writes. To move an old vault
(or restore a backup) into the new storage:

```
rk migrate ~/.local/share/rahasyakosh/vault.rkv --vault ~/.local/share/rahasyakosh/vault.db
```

It copies the scrambled records as they are (nothing is decrypted or re-encrypted, and your master password stays the same), then proves the copy opens with your
password. If anything fails, the new file is removed. It never changes or deletes the old one, so keep it until you are happy.

## How fast is it?

Measured in the tests (a normal laptop, unoptimised test build, deliberately cheap key settings so the figures show the storage, not the password hashing):

| What | Result | Target |
|---|---|---|
| Unlock a vault of **10,000** items (decrypt all + build the index) | about 0.8 s | usable |
| Search **5,000** items | 2 to 5 ms | under 50 ms |
| List all 10,000 items | about 5 ms | |
| Change one item in a 10,000-item vault | 0.1 to 0.2 s (mostly waiting for the disk to confirm) | does not grow with vault size |

The real password check (Argon2) adds a fixed ~0.15 s on top of unlocking.

## How it is tested

- **One suite, every backend.** A shared "conformance" test suite (about a dozen scenarios: stale writes, tombstones, reopening, binary data, a
  megabyte value, 300 records) must pass on every backend, so they all behave identically. We broke the memory backend on purpose three ways to prove the suite notices.
- **Crashes.** The directory backend has tests that simulate the program dying between the two steps of a write or a purge.
- **Corruption.** Garbage, truncated, oversized and mislabelled files produce clear errors, never a crash.
- **A model.** A property test applies random sequences of add, rename, trash, restore, purge and revert to the real vault and to a trivial model and demands they always agree, including after locking and unlocking.
- **Races.** Two sessions on one database, and four threads writing at once, lose no updates.
- **Mutation checks.** We changed the trash expiry by one day, made search match any word instead of all, made the index include passwords, stopped edits saving history, and stopped purges leaving tombstones. The tests caught every one.

## What it does not do yet

- **No syncing.** Two devices cannot share a vault yet (milestone M4). The directory backend and tombstones are groundwork.
- **The directory backend is not all-or-nothing across files.** A crash in the middle of a large batch can leave some records written and some not (each file is always complete).
- **SQLite is not built for WebAssembly.** The browser version will use a different backend (IndexedDB, milestone M5).
- **The CLI has no sessions.** It unlocks, acts and exits each time. See [How a vault works](how-a-vault-works.md).

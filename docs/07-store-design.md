# 07 — M2 Store Design

Status: **implemented** (decisions in section 10 accepted as recommended; all seven work packages done). Implements M2 of the roadmap (ADR-4 in `03-architecture.md` already fixes the broad shape).
Stories: US-ITM-02, US-ITM-03, US-ITM-04, US-ITM-05, US-ITM-08.

## 1. Goals and non-goals

**Goals**
- Replace the single JSON file (rewritten whole on every change) with item-granular storage behind a `Store` trait.
- Backends: **SQLite** (default for CLI/desktop), **directory** (one file per item, for folder sync), **memory** (tests, later web).
- Trash with restore and auto-purge, item history with restore, tags and favourites, in-memory search.
- Schema migrations with tests. 10,000 items stay usable.

**Non-goals (later milestones)**
- Sync, hybrid logical clocks, merge (M4). We only leave room for them (section 8).
- IndexedDB backend (M5). The trait must not prevent it.
- Session timers, auto-lock, clipboard (M3).

## 2. Layering

```
 rk (CLI)  ─────────────►  rahasyakosh-app   Vault<S>: unlocked operations, history, trash, search index
                                │   uses                    │ uses
                                ▼                           ▼
                          rahasyakosh-core            rahasyakosh-store
                     crypto, header, records,         Store trait + backends
                     Item model (no I/O)              (knows nothing about keys)
```

- **`store` never sees a key or plaintext.** It saves and loads the header and opaque encrypted `Record`s. A bug or leak there
  cannot expose secrets, and the same backends serve every shell.
- **`core` stays free of I/O**, as today.
- **`app`** is the crate the roadmap calls the session crate. M2 starts it with the *operations* (add, edit, trash, history,
  search) and M3 adds the session state machine, auto-lock and clipboard on top.

New crates: `rahasyakosh-store`, `rahasyakosh-app`. The SQLite backend sits behind a cargo feature `sqlite`, so the trait,
memory and directory backends still build for `wasm32-unknown-unknown`; SQLite (C code) does not.

## 3. The `Store` trait

Stores hold the vault header and a set of records, plus tombstones for purged items. It is deliberately dumb.

```rust
pub trait Store {
    fn create(&mut self, header: &VaultHeader) -> Result<()>;   // fails if a vault already exists
    fn header(&self) -> Result<VaultHeader>;
    fn set_header(&mut self, header: &VaultHeader) -> Result<()>; // change of master password

    fn get(&self, id: Uuid) -> Result<Option<Record>>;
    fn list(&self) -> Result<Vec<RecordMeta>>;                  // id, rev, size: no ciphertext, cheap
    fn all(&self) -> Result<Vec<Record>>;                       // used once on unlock
    fn put_many(&mut self, records: &[Record]) -> Result<()>;   // atomic where the backend can (SQLite: one transaction)
    fn purge(&mut self, id: Uuid, rev: u64) -> Result<()>;      // remove the record, keep a tombstone
    fn tombstones(&self) -> Result<Vec<Tombstone>>;
}
```

- `put` of a record whose `rev` is not higher than the stored one is refused (`StaleRevision`). Two processes editing the same item
  cannot silently overwrite each other; the loser gets an error and retries.
- A shared **conformance suite** (`store/tests/conformance.rs`, generic over `S: Store`) runs the same tests against every
  backend: create/read/update/purge, stale revision, header change, list vs all, tombstones, reopen from disk, and corrupted-file handling.

### Plaintext metadata (what a thief of the storage can see)

Per record: `id` (random UUID), `rev`, `record_version`, ciphertext length (padded to 64-byte buckets). Per vault: header (already
public), record count, tombstone ids and revisions. Titles, URLs, tags, timestamps and the trash flag stay inside the ciphertext.

## 4. Backends

### SQLite (`sqlite` feature, crate `rusqlite` with bundled SQLite)
```
PRAGMA user_version = <schema version>      -- migrations key off this
meta(key TEXT PRIMARY KEY, value BLOB)      -- 'header' = header JSON
records(id BLOB(16) PRIMARY KEY, rev INTEGER NOT NULL, record_version INTEGER NOT NULL,
        nonce BLOB NOT NULL, ct BLOB NOT NULL)
tombstones(id BLOB(16) PRIMARY KEY, rev INTEGER NOT NULL)
```
- WAL mode, `foreign_keys` on, file created `0600`, new parent folders `0700` (`private_fs::create_private_dir`). `private_fs` also has `write_private_atomic` (temp file 0600 + fsync + rename) and `tighten_tree` (repair old modes under a folder the program owns).
- **`PRAGMA secure_delete = ON`** so purged ciphertext is zeroed out of free pages rather than lingering in the file.
- Every write is one transaction. A second `rk` process waits on the SQLite lock (busy timeout) instead of corrupting anything.

### Directory (for folder sync, M4)
```
<dir>/header.json
<dir>/<uuid>.rkr        one JSON Record per item
<dir>/<uuid>.rkt        tombstone
```
Each file is written to a temp name, flushed and renamed, so a sync tool never uploads a half-written record. No locking; the
stale-revision check is best-effort here and real conflict handling arrives with M4.

### Memory
`BTreeMap`s. Used by tests and as the model other backends are checked against.

## 5. Migrations

- The schema version lives in `PRAGMA user_version`. `SqliteStore::open` runs the ordered list of migration steps from the
  stored version up to current inside **one transaction**; a failed step leaves the file untouched.
- Opening a database with a *newer* version than the code knows is refused ("upgrade rk"), never downgraded.
- **Migration tests:** a fixture database for every released schema version is committed under `spec/vectors/` (like the golden
  vault) and a test migrates each to current and checks the data. The fixtures are never regenerated.
- **Legacy file:** `rk migrate <old.rkv>` imports the current single-JSON-file vault into a new SQLite store (records are copied as they
  are; nothing is decrypted or re-encrypted).

## 6. Vault operations (`rahasyakosh-app`)

`Vault<S: Store>` is what the CLI and later the apps use. It exists only in the unlocked state: `Vault::unlock(store, password)`
derives the keys, decrypts every record once and builds the index; dropping it wipes the keys and index.

| Operation | Behaviour |
|---|---|
| `add`, `edit` | Seal with `rev + 1`; the previous version is pushed onto the item's history first |
| `trash` / `restore` | Set / clear `deleted_at` inside the payload (a normal edit, so it is also recorded in history) |
| `purge` | `Store::purge`, tombstone left behind |
| auto-purge | (`rk trash` already lists trashed items; nothing purges them automatically yet.) On unlock, items in the trash longer than 30 days (configurable) are purged. **The store cannot do this itself because `deleted_at` is encrypted**, so it is the vault's job |
| `history`, `revert` | List stored versions; `revert` makes a *new* version with the old content (never rewrites the past) |
| tags, favourites | Part of the payload; tags are listed from the index; favourites sort first |
| `search` | In-memory, see below |

### History (US-ITM-02, US-ITM-08)
History lives **inside the item's payload**: `Item.history: Vec<ItemVersion>`, newest first, capped at N (default 10). This matches
the domain model in `03-architecture.md` section 4 and keeps one item = one atomic record, which is also what sync wants. Cost: a record is
up to N+1 times larger; with 64-byte padding that is acceptable for logins.
`history` is `#[serde(default)]`, so old payloads (including the golden vault) still load; new writes use `ITEM_SCHEMA = 2`.
Passwords in history are as protected as the current one (same key, same ciphertext). Purging an item removes its history with it.

### Search (US-ITM-04)
Built on unlock from decrypted items, matching **title, username, URLs and tags** (concealed fields and notes are not searched).
For each item the index keeps lower-cased copies of those strings, in `Zeroizing` buffers, and a query is a linear scan with
`contains`. Linear is enough at this size (5,000 items is well under the 50 ms target, to be proved by a test) and has no
index structure to keep consistent or to leak. Updated incrementally on every operation; wiped on drop.

## 7. CLI changes

- Default vault becomes `~/.local/share/rahasyakosh/vault.db` (SQLite). `--vault` / `RK_VAULT` unchanged.
- New: `rk migrate`, `rk history <item>`, `rk revert <item> <version>`, `rk edit --favourite`. (`rk trash`, `rk restore` and `rk rm --purge` already exist and keep their behaviour.)
- `rk export -f vault` keeps writing the **portable JSON bundle** (header + records), so backups stay human-inspectable and the format
  in `spec/vectors/` stays the interchange format. `rk migrate <file>` loads such a bundle (or an old-format vault) into a new vault; it replaced the separate `import-vault` idea because it is the same operation.
- Still stateless: no sessions, so no `lock` (decided earlier).

## 8. Leaving room for sync (M4)

- Tombstones are in the schema from day one, so deletions can propagate later without a migration.
- `rev: u64` is a plain counter today. ADR-5 wants a hybrid logical clock (HLC) in plaintext. Adding it changes the record format
  (`record_version` 2, new golden vector, a read-old/write-new rule). **Decision needed**, see section 10.

## 9. Testing and performance

- Conformance suite over memory, directory and SQLite; property test: a random operation sequence applied to a backend and to the
  memory model must give identical state; reopen-after-every-step.
- Corruption: truncated and garbage files, wrong schema version, record with a bad id, all return errors, never panic.
- Migration tests with fixture databases (section 5).
- Performance (CI, optimised dependencies): create 10,000 items; unlock + decrypt + index in a budget measured and written into the test
  (target well under 2 s excluding the one Argon2 call); search under 50 ms at 5,000 items; add/edit one item in constant time.
- WebAssembly: store trait + memory + directory backends compile for `wasm32-unknown-unknown`; the `sqlite` feature is excluded in CI.

## 10. Decisions needed before building

1. **Plaintext HLC in the record format now or in M4?** *Recommendation:* decide now to **not** change the format in M2; add
   `record_version = 2` with the HLC in M4, with a migration and a `golden-v2`. Rationale: the format is already frozen by the golden
   vault and nothing needs the clock before sync. Risk: every M2 vault gets one more migration later (cheap: re-seal each record).
2. **Keep a JSON file in any role?** *Recommendation:* yes, only as the **portable bundle** for backup/export/import (section 7), not as a
   working store.
3. **Start the `app` crate in M2?** *Recommendation:* yes (section 2). The alternative, putting these operations in the CLI, would mean
   rewriting them for the desktop app.
4. **History inside the payload vs. separate version records?** *Recommendation:* inside the payload (section 6). Separate records scale
   better for very large histories, which we do not need, but complicate sync and the store trait.

## 10b. Profiles (E13)

Decided 2026-10-24: **one vault file and one master password per profile.** Nothing in the store, core or record formats changed. `rahasyakosh_app::ProfileList` (pure, tested) holds each profile's ID, full name, unique username (case-insensitive) and vault file name; the front-end saves it as versioned JSON (`profiles.json`, written through a temporary file). The list is **plaintext** because the picker needs it before unlocking; it holds no secret. An existing single vault is adopted in place as the default profile, so no vault is touched or re-encrypted. A profile's file name is checked so it cannot point outside the data folder.

## 11. Work packages (in order)

1. `store` crate: trait, errors, memory backend, conformance suite.
2. Directory backend.
3. SQLite backend, migration framework, fixture database.
4. `core`: `Item.history` and `ITEM_SCHEMA = 2` (backward compatible), golden vault still opens.
5. `app` crate: `Vault<S>` unlock, CRUD, trash/restore/purge/auto-purge, history/revert, tags/favourites, search index.
6. CLI on the new stack; `rk migrate` and the new commands; update CLI end-to-end tests.
7. 10k/5k performance tests; property test against the memory model; book chapter; roadmap.

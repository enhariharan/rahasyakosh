//! SQLite backend (feature `sqlite`): the default working store for CLI and desktop.
//!
//! ```text
//! PRAGMA user_version        schema version; migrations key off it
//! meta(key, value)           'header' = vault header (JSON)
//! records(id, rev, record_version, nonce, ct)
//! tombstones(id, rev)
//! ```
//! Revisions are `u64` in Rust but SQLite integers are signed, so they are stored bit-for-bit as `i64` and are only
//! ever compared in Rust, never in SQL.
//!
//! Settings: write-ahead log, `synchronous = FULL` (a committed write survives power loss), and
//! `secure_delete = ON` so purged ciphertext is overwritten with zeros rather than left in free pages.
//! The file (and its `-wal`/`-shm` companions) is private to the owner.
use crate::{
    check_batch, Error, Readable, Record, RecordMeta, Result, Store, Tombstone, Unreadable, Uuid,
    VaultHeader,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

/// One schema step. Runs inside a transaction that also bumps `user_version`.
pub type Migration = fn(&Transaction) -> rusqlite::Result<()>;

/// Every schema version, in order. `MIGRATIONS[n]` upgrades a database at version `n` to `n + 1`.
/// **Never edit or reorder a released entry**; append a new one, add a fixture database for the old version
/// to `spec/vectors/`, and extend the migration test.
pub const MIGRATIONS: &[Migration] = &[v1_initial_schema];

/// The schema version this program writes.
pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

fn v1_initial_schema(tx: &Transaction) -> rusqlite::Result<()> {
    tx.execute_batch(
        "CREATE TABLE meta (
             key   TEXT PRIMARY KEY,
             value BLOB NOT NULL
         ) WITHOUT ROWID;
         CREATE TABLE records (
             id             BLOB PRIMARY KEY CHECK (length(id) = 16),
             rev            INTEGER NOT NULL,
             record_version INTEGER NOT NULL,
             nonce          BLOB NOT NULL CHECK (length(nonce) = 24),
             ct             BLOB NOT NULL
         ) WITHOUT ROWID;
         CREATE TABLE tombstones (
             id  BLOB PRIMARY KEY CHECK (length(id) = 16),
             rev INTEGER NOT NULL
         ) WITHOUT ROWID;",
    )
}

pub struct SqliteStore {
    conn: Connection,
    path: Option<std::path::PathBuf>,
}

impl SqliteStore {
    /// Open (creating if necessary) the database at `path` and bring its schema up to date.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        create_private_file(path)?;
        let conn = Connection::open(path).map_err(db)?;
        Self::init(conn, MIGRATIONS, Some(path))
    }

    /// A private in-memory database (tests).
    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory().map_err(db)?, MIGRATIONS, None)
    }

    /// Like [`open`](Self::open) with a custom list of migrations. For testing the migration machinery.
    #[doc(hidden)]
    pub fn open_with_migrations(path: impl AsRef<Path>, steps: &[Migration]) -> Result<Self> {
        let path = path.as_ref();
        create_private_file(path)?;
        Self::init(Connection::open(path).map_err(db)?, steps, Some(path))
    }

    fn init(mut conn: Connection, steps: &[Migration], path: Option<&Path>) -> Result<Self> {
        conn.busy_timeout(Duration::from_secs(5)).map_err(db)?;
        conn.pragma_update(None, "foreign_keys", "ON").map_err(db)?;
        conn.pragma_update(None, "secure_delete", "ON")
            .map_err(db)?;
        conn.pragma_update(None, "synchronous", "FULL")
            .map_err(db)?;
        if path.is_some() {
            conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get::<_, String>(0))
                .map_err(db)?;
        }
        migrate(&mut conn, steps)?;
        Ok(Self {
            conn,
            path: path.map(Path::to_path_buf),
        })
    }

    /// Where the database file is, or `None` for an in-memory database.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Value of a connection setting such as `secure_delete`. For tests.
    #[doc(hidden)]
    pub fn pragma(&self, name: &str) -> Result<String> {
        self.conn
            .query_row(&format!("PRAGMA {name}"), [], |r| {
                r.get::<_, rusqlite::types::Value>(0)
            })
            .map(|v| match v {
                rusqlite::types::Value::Integer(i) => i.to_string(),
                rusqlite::types::Value::Text(t) => t,
                other => format!("{other:?}"),
            })
            .map_err(db)
    }

    /// The schema version stored in the database.
    pub fn schema_version(&self) -> Result<u32> {
        user_version(&self.conn)
    }

    fn tx(&mut self) -> Result<Transaction<'_>> {
        // IMMEDIATE takes the write lock up front, so "check the revision, then write" cannot race another process.
        self.conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)
    }
}

/// Bring `conn` from its stored version up to `steps.len()`. Each step and the version bump commit together;
/// a failing step leaves the database exactly as it was. A database from the future is refused, never touched.
fn migrate(conn: &mut Connection, steps: &[Migration]) -> Result<()> {
    let supported = steps.len() as u32;
    let found = user_version(conn)?;
    if found > supported {
        return Err(Error::TooNew { found, supported });
    }
    for (n, step) in steps.iter().enumerate().skip(found as usize) {
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        step(&tx).map_err(db)?;
        tx.pragma_update(None, "user_version", n as u32 + 1)
            .map_err(db)?;
        tx.commit().map_err(db)?;
    }
    Ok(())
}

fn user_version(conn: &Connection) -> Result<u32> {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(db)
}

/// Create the file owner-only before SQLite does, so it never exists with looser permissions.
fn create_private_file(path: &Path) -> Result<()> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(false);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    opts.open(path).map(drop).map_err(io)
}

fn io(e: std::io::Error) -> Error {
    Error::Io(e.to_string())
}

fn db(e: rusqlite::Error) -> Error {
    use rusqlite::ErrorCode::{DatabaseCorrupt, NotADatabase};
    match e.sqlite_error_code() {
        Some(NotADatabase | DatabaseCorrupt) => Error::Corrupt(e.to_string()),
        _ => Error::Io(e.to_string()),
    }
}

fn to_db_rev(rev: u64) -> i64 {
    rev as i64
}

fn from_db_rev(rev: i64) -> u64 {
    rev as u64
}

fn uuid_from(bytes: &[u8]) -> Result<Uuid> {
    Uuid::from_slice(bytes).map_err(|_| Error::Corrupt("bad item id".into()))
}

fn record_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<RawRecord> {
    Ok(RawRecord {
        id: r.get(0)?,
        rev: r.get(1)?,
        record_version: r.get(2)?,
        nonce: r.get(3)?,
        ct: r.get(4)?,
    })
}

struct RawRecord {
    id: Vec<u8>,
    rev: i64,
    record_version: u16,
    nonce: Vec<u8>,
    ct: Vec<u8>,
}

impl RawRecord {
    fn into_record(self) -> Result<Record> {
        Ok(Record {
            record_version: self.record_version,
            item_id: uuid_from(&self.id)?,
            rev: from_db_rev(self.rev),
            nonce: self
                .nonce
                .try_into()
                .map_err(|_| Error::Corrupt("bad nonce length".into()))?,
            ct: self.ct,
        })
    }
}

const SELECT_RECORD: &str = "SELECT id, rev, record_version, nonce, ct FROM records";

fn read_header(conn: &Connection) -> Result<Option<VaultHeader>> {
    let bytes: Option<Vec<u8>> = conn
        .query_row("SELECT value FROM meta WHERE key = 'header'", [], |r| {
            r.get(0)
        })
        .optional()
        .map_err(db)?;
    bytes
        .map(|b| serde_json::from_slice(&b).map_err(|e| Error::Corrupt(format!("header: {e}"))))
        .transpose()
}

fn write_header(conn: &Connection, header: &VaultHeader) -> Result<()> {
    let json = serde_json::to_vec(header).map_err(|e| Error::Io(e.to_string()))?;
    conn.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES ('header', ?1)",
        params![json],
    )
    .map_err(db)?;
    Ok(())
}

fn stored_rev(conn: &Connection, table: &str, id: Uuid) -> Result<Option<u64>> {
    conn.query_row(
        &format!("SELECT rev FROM {table} WHERE id = ?1"),
        params![id.as_bytes().as_slice()],
        |r| r.get::<_, i64>(0),
    )
    .optional()
    .map(|r| r.map(from_db_rev))
    .map_err(db)
}

impl Store for SqliteStore {
    fn create(&mut self, header: &VaultHeader) -> Result<()> {
        let tx = self.tx()?;
        if read_header(&tx)?.is_some() {
            return Err(Error::AlreadyExists);
        }
        write_header(&tx, header)?;
        tx.commit().map_err(db)
    }

    fn header(&self) -> Result<VaultHeader> {
        read_header(&self.conn)?.ok_or(Error::NotInitialised)
    }

    fn set_header(&mut self, header: &VaultHeader) -> Result<()> {
        let tx = self.tx()?;
        let current = read_header(&tx)?.ok_or(Error::NotInitialised)?;
        if current.vault_id != header.vault_id {
            return Err(Error::WrongVault);
        }
        write_header(&tx, header)?;
        tx.commit().map_err(db)
    }

    fn get(&self, id: Uuid) -> Result<Option<Record>> {
        self.conn
            .query_row(
                &format!("{SELECT_RECORD} WHERE id = ?1"),
                params![id.as_bytes().as_slice()],
                record_from_row,
            )
            .optional()
            .map_err(db)?
            .map(RawRecord::into_record)
            .transpose()
    }

    fn list(&self) -> Result<Vec<RecordMeta>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, rev, length(ct) FROM records ORDER BY id")
            .map_err(db)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, Vec<u8>>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })
            .map_err(db)?;
        rows.map(|row| {
            let (id, rev, size) = row.map_err(db)?;
            Ok(RecordMeta {
                id: uuid_from(&id)?,
                rev: from_db_rev(rev),
                size: size as usize,
            })
        })
        .collect()
    }

    fn all(&self) -> Result<Vec<Record>> {
        let mut stmt = self
            .conn
            .prepare(&format!("{SELECT_RECORD} ORDER BY id"))
            .map_err(db)?;
        let rows = stmt.query_map([], record_from_row).map_err(db)?;
        rows.map(|row| row.map_err(db)?.into_record()).collect()
    }

    fn all_readable(&self) -> Result<Readable> {
        let mut stmt = self
            .conn
            .prepare(&format!("{SELECT_RECORD} ORDER BY id"))
            .map_err(db)?;
        let rows = stmt.query_map([], record_from_row).map_err(db)?;
        let mut out = Readable::default();
        for row in rows {
            let raw = row.map_err(db)?;
            let id = Uuid::from_slice(&raw.id).ok();
            match raw.into_record() {
                Ok(r) => out.records.push(r),
                Err(Error::Corrupt(reason)) => out.unreadable.push(Unreadable {
                    id,
                    source: "records".into(),
                    reason,
                }),
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    fn put_many(&mut self, records: &[Record]) -> Result<()> {
        let tx = self.tx()?;
        read_header(&tx)?.ok_or(Error::NotInitialised)?;
        let mut current = BTreeMap::new();
        for r in records {
            current.insert(
                r.item_id,
                (
                    stored_rev(&tx, "records", r.item_id)?,
                    stored_rev(&tx, "tombstones", r.item_id)?,
                ),
            );
        }
        check_batch(records, |id| {
            current.get(&id).copied().unwrap_or((None, None))
        })?;
        for r in records {
            let id = r.item_id.as_bytes().as_slice();
            tx.execute("DELETE FROM tombstones WHERE id = ?1", params![id])
                .map_err(db)?;
            tx.execute(
                "INSERT OR REPLACE INTO records (id, rev, record_version, nonce, ct) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, to_db_rev(r.rev), r.record_version, r.nonce.as_slice(), r.ct],
            )
            .map_err(db)?;
        }
        tx.commit().map_err(db)
    }

    fn purge(&mut self, id: Uuid, rev: u64) -> Result<()> {
        let tx = self.tx()?;
        read_header(&tx)?.ok_or(Error::NotInitialised)?;
        if let Some(stored) = stored_rev(&tx, "records", id)? {
            if rev <= stored {
                return Err(Error::StaleRevision {
                    id,
                    stored,
                    offered: rev,
                });
            }
        }
        let rev = rev.max(stored_rev(&tx, "tombstones", id)?.unwrap_or(0));
        let key = id.as_bytes().as_slice();
        tx.execute("DELETE FROM records WHERE id = ?1", params![key])
            .map_err(db)?;
        tx.execute(
            "INSERT OR REPLACE INTO tombstones (id, rev) VALUES (?1, ?2)",
            params![key, to_db_rev(rev)],
        )
        .map_err(db)?;
        tx.commit().map_err(db)
    }

    fn tombstones(&self) -> Result<Vec<Tombstone>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, rev FROM tombstones ORDER BY id")
            .map_err(db)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, i64>(1)?)))
            .map_err(db)?;
        rows.map(|row| {
            let (id, rev) = row.map_err(db)?;
            Ok(Tombstone {
                id: uuid_from(&id)?,
                rev: from_db_rev(rev),
            })
        })
        .collect()
    }
}

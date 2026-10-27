//! Storage for vaults (M2, see `docs/07-store-design.md`).
//!
//! A store keeps the vault header, the encrypted [`Record`]s and tombstones for purged items. It never sees a
//! key or any plaintext, so a bug here cannot leak secrets, and every shell can share the same backends.
#![forbid(unsafe_code)]

pub mod conformance;
mod directory;
mod memory;
#[cfg(feature = "sqlite")]
mod sqlite;

pub use directory::DirectoryStore;
pub use memory::MemoryStore;
pub use rahasyakosh_core::vault::{Record, VaultHeader};
#[cfg(feature = "sqlite")]
pub use sqlite::{Migration, SqliteStore, MIGRATIONS, SCHEMA_VERSION};
pub use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("a vault already exists here")]
    AlreadyExists,
    #[error("no vault has been created here")]
    NotInitialised,
    #[error("header belongs to a different vault")]
    WrongVault,
    /// A write was not newer than what is stored (or than the item's tombstone).
    #[error("stale revision for {id}: offered {offered}, already at {stored}")]
    StaleRevision { id: Uuid, stored: u64, offered: u64 },
    #[error("the same item appears twice in one batch")]
    DuplicateInBatch,
    /// The stored data cannot be read: truncated, garbled or inconsistent.
    #[error("vault storage is corrupted: {0}")]
    Corrupt(String),
    /// The database was written by a newer version of this program.
    #[error("storage format version {found} is newer than this program understands ({supported}); upgrade rk")]
    TooNew { found: u32, supported: u32 },
    #[error("storage error: {0}")]
    Io(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// What can be known about a record without reading its ciphertext.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordMeta {
    pub id: Uuid,
    pub rev: u64,
    /// Ciphertext length in bytes (already padded to 64-byte buckets).
    pub size: usize,
}

/// A stored record or tombstone that could not be read (garbled, truncated, or a sync conflict copy). It is
/// reported to the user and skipped, so that one bad file cannot lock them out of everything else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreadable {
    /// The item it belongs to, when that is known (for a directory store, from the file name).
    pub id: Option<Uuid>,
    /// Where it was found: a file name, or `records` for a database row.
    pub source: String,
    pub reason: String,
}

/// The result of reading everything that can be read: see [`Store::all_readable`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Readable {
    pub records: Vec<Record>,
    pub unreadable: Vec<Unreadable>,
}

/// Marks an item as permanently removed, so the removal can later be propagated by sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tombstone {
    pub id: Uuid,
    pub rev: u64,
}

/// Persistence for one vault.
///
/// Write rules, identical for every backend (the conformance suite checks them):
/// - A record is accepted only if its `rev` is greater than both the stored record's and the item's tombstone's.
///   Otherwise the whole call fails with [`Error::StaleRevision`] and nothing is written.
/// - Writing a record newer than a tombstone removes the tombstone.
/// - Lists come back sorted by id.
pub trait Store {
    /// Create the vault. Fails with [`Error::AlreadyExists`] if one is already there.
    fn create(&mut self, header: &VaultHeader) -> Result<()>;

    fn header(&self) -> Result<VaultHeader>;

    /// Replace the header (a master password change). The vault id must not change.
    fn set_header(&mut self, header: &VaultHeader) -> Result<()>;

    fn get(&self, id: Uuid) -> Result<Option<Record>>;

    /// Cheap listing: no ciphertext is read.
    fn list(&self) -> Result<Vec<RecordMeta>>;

    /// Every record. Used once, when the vault is unlocked.
    fn all(&self) -> Result<Vec<Record>>;

    /// Like [`all`](Self::all), but a record that cannot be parsed is reported in `unreadable` instead of failing
    /// the whole call. Problems with the store itself (I/O errors, a damaged database) still fail.
    /// The default is for backends that cannot have a bad record on its own.
    fn all_readable(&self) -> Result<Readable> {
        Ok(Readable {
            records: self.all()?,
            unreadable: vec![],
        })
    }

    /// Write records. All are validated first; where the backend can, the writes are one atomic step.
    fn put_many(&mut self, records: &[Record]) -> Result<()>;

    fn put(&mut self, record: &Record) -> Result<()> {
        self.put_many(std::slice::from_ref(record))
    }

    /// Remove an item for good and leave a tombstone at `rev`, which must be newer than the stored record.
    /// Purging an item that is already gone is not an error.
    fn purge(&mut self, id: Uuid, rev: u64) -> Result<()>;

    fn tombstones(&self) -> Result<Vec<Tombstone>>;
}

/// Shared by backends: checks a batch against the current state without writing anything.
/// `current(id)` returns the stored `(record rev, tombstone rev)` of an item.
pub(crate) fn check_batch(
    records: &[Record],
    mut current: impl FnMut(Uuid) -> (Option<u64>, Option<u64>),
) -> Result<()> {
    let mut seen = std::collections::BTreeSet::new();
    for r in records {
        if !seen.insert(r.item_id) {
            return Err(Error::DuplicateInBatch);
        }
        let (rec, tomb) = current(r.item_id);
        let stored = rec.into_iter().chain(tomb).max();
        if let Some(stored) = stored {
            if r.rev <= stored {
                return Err(Error::StaleRevision {
                    id: r.item_id,
                    stored,
                    offered: r.rev,
                });
            }
        }
    }
    Ok(())
}

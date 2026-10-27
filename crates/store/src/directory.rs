//! One file per item in a directory, for folder-sync tools (Syncthing, Dropbox, iCloud, ...).
//!
//! ```text
//! <dir>/header.json      vault header
//! <dir>/<uuid>.rkr       one Record (JSON)
//! <dir>/<uuid>.rkt       tombstone: {"rev": N}
//! ```
//! Every file is written under a temporary name, flushed and renamed, so a sync tool never picks up a
//! half-written record. A batch is **not** atomic across files; if the program dies mid-batch, some records are
//! written and some are not, and each file on its own is always complete. Files with other names (a sync tool's
//! conflict copies, temporary files) are ignored.
use crate::{
    check_batch, Error, Readable, Record, RecordMeta, Result, Store, Tombstone, Unreadable, Uuid,
    VaultHeader,
};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// A file larger than this is treated as corrupt rather than read into memory.
const MAX_FILE: u64 = 64 * 1024 * 1024;
const HEADER: &str = "header.json";

#[derive(Debug, Clone)]
pub struct DirectoryStore {
    dir: PathBuf,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct TombstoneFile {
    rev: u64,
}

impl DirectoryStore {
    /// A handle on `dir`. Nothing is read or created until [`Store::create`] or a read.
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn path(&self) -> &Path {
        &self.dir
    }

    fn file(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    fn record_name(id: Uuid) -> String {
        format!("{id}.rkr")
    }

    fn tombstone_name(id: Uuid) -> String {
        format!("{id}.rkt")
    }

    /// Whole file contents, or `None` if it does not exist.
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let path = self.file(name);
        let meta = match fs::metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io(e)),
        };
        if meta.len() > MAX_FILE {
            return Err(Error::Corrupt(format!("{name} is implausibly large")));
        }
        match fs::read(&path) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io(e)),
        }
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<()> {
        let tmp = self.file(&format!(".{name}.tmp"));
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
        let mut f = opts.open(&tmp).map_err(io)?;
        f.write_all(bytes).map_err(io)?;
        f.sync_all().map_err(io)?;
        drop(f);
        fs::rename(&tmp, self.file(name)).map_err(io)?;
        // Make the rename itself durable. Best effort: not every filesystem allows syncing a directory.
        if let Ok(d) = fs::File::open(&self.dir) {
            let _ = d.sync_all();
        }
        Ok(())
    }

    fn remove(&self, name: &str) -> Result<()> {
        match fs::remove_file(self.file(name)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io(e)),
        }
    }

    fn require_vault(&self) -> Result<VaultHeader> {
        self.header()
    }

    fn read_record_file(&self, id: Uuid) -> Result<Option<Record>> {
        let Some(bytes) = self.read(&Self::record_name(id))? else {
            return Ok(None);
        };
        let r: Record = serde_json::from_slice(&bytes)
            .map_err(|e| Error::Corrupt(format!("{}: {e}", Self::record_name(id))))?;
        if r.item_id != id {
            return Err(Error::Corrupt(format!(
                "{} holds a record for a different item",
                Self::record_name(id)
            )));
        }
        Ok(Some(r))
    }

    fn read_tombstone_file(&self, id: Uuid) -> Result<Option<u64>> {
        let Some(bytes) = self.read(&Self::tombstone_name(id))? else {
            return Ok(None);
        };
        serde_json::from_slice::<TombstoneFile>(&bytes)
            .map(|t| Some(t.rev))
            .map_err(|e| Error::Corrupt(format!("{}: {e}", Self::tombstone_name(id))))
    }

    /// Ids of all files with the given extension whose stem is a UUID, sorted.
    fn ids_with_extension(&self, ext: &str) -> Result<Vec<Uuid>> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(io(e)),
        };
        let mut ids = Vec::new();
        for entry in entries {
            let path = entry.map_err(io)?.path();
            if path.extension().and_then(|e| e.to_str()) != Some(ext) {
                continue;
            }
            if let Some(id) = path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                ids.push(id);
            }
        }
        ids.sort();
        Ok(ids)
    }

    fn all_tombstones(&self) -> Result<BTreeMap<Uuid, u64>> {
        let mut out = BTreeMap::new();
        for id in self.ids_with_extension("rkt")? {
            if let Some(rev) = self.read_tombstone_file(id)? {
                out.insert(id, rev);
            }
        }
        Ok(out)
    }

    /// Live records, sorted by id. A record at or below its tombstone's revision counts as purged: this covers a
    /// crash between writing a tombstone and removing the record file.
    fn live_records(&self) -> Result<Vec<Record>> {
        let tombs = self.all_tombstones()?;
        let mut out = Vec::new();
        for id in self.ids_with_extension("rkr")? {
            if let Some(r) = self.read_record_file(id)? {
                if tombs.get(&id).is_none_or(|t| r.rev > *t) {
                    out.push(r);
                }
            }
        }
        Ok(out)
    }
}

fn io(e: std::io::Error) -> Error {
    Error::Io(e.to_string())
}

impl Store for DirectoryStore {
    fn create(&mut self, header: &VaultHeader) -> Result<()> {
        if self.file(HEADER).exists() {
            return Err(Error::AlreadyExists);
        }
        fs::create_dir_all(&self.dir).map_err(io)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.dir, fs::Permissions::from_mode(0o700)).map_err(io)?;
        }
        self.write(
            HEADER,
            &serde_json::to_vec_pretty(header).map_err(|e| Error::Io(e.to_string()))?,
        )
    }

    fn header(&self) -> Result<VaultHeader> {
        let bytes = self.read(HEADER)?.ok_or(Error::NotInitialised)?;
        serde_json::from_slice(&bytes).map_err(|e| Error::Corrupt(format!("{HEADER}: {e}")))
    }

    fn set_header(&mut self, header: &VaultHeader) -> Result<()> {
        if self.require_vault()?.vault_id != header.vault_id {
            return Err(Error::WrongVault);
        }
        self.write(
            HEADER,
            &serde_json::to_vec_pretty(header).map_err(|e| Error::Io(e.to_string()))?,
        )
    }

    fn get(&self, id: Uuid) -> Result<Option<Record>> {
        let Some(r) = self.read_record_file(id)? else {
            return Ok(None);
        };
        let tomb = self.read_tombstone_file(id)?;
        Ok(tomb.is_none_or(|t| r.rev > t).then_some(r))
    }

    fn list(&self) -> Result<Vec<RecordMeta>> {
        Ok(self
            .live_records()?
            .iter()
            .map(|r| RecordMeta {
                id: r.item_id,
                rev: r.rev,
                size: r.ct.len(),
            })
            .collect())
    }

    fn all(&self) -> Result<Vec<Record>> {
        self.live_records()
    }

    fn all_readable(&self) -> Result<Readable> {
        self.require_vault()?;
        let mut out = Readable::default();
        // A tombstone that cannot be read is reported and ignored: the item it names stays visible.
        let mut tombs = BTreeMap::new();
        for id in self.ids_with_extension("rkt")? {
            match self.read_tombstone_file(id) {
                Ok(Some(rev)) => {
                    tombs.insert(id, rev);
                }
                Ok(None) => {}
                Err(Error::Corrupt(reason)) => out.unreadable.push(Unreadable {
                    id: Some(id),
                    source: Self::tombstone_name(id),
                    reason,
                }),
                Err(e) => return Err(e),
            }
        }
        for id in self.ids_with_extension("rkr")? {
            match self.read_record_file(id) {
                Ok(Some(r)) => {
                    if tombs.get(&id).is_none_or(|t| r.rev > *t) {
                        out.records.push(r);
                    }
                }
                Ok(None) => {}
                Err(Error::Corrupt(reason)) => out.unreadable.push(Unreadable {
                    id: Some(id),
                    source: Self::record_name(id),
                    reason,
                }),
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    fn put_many(&mut self, records: &[Record]) -> Result<()> {
        self.require_vault()?;
        let mut lookup_error = None;
        check_batch(records, |id| {
            let rec = self.read_record_file(id).map(|r| r.map(|r| r.rev));
            let tomb = self.read_tombstone_file(id);
            match (rec, tomb) {
                (Ok(r), Ok(t)) => (r, t),
                (Err(e), _) | (_, Err(e)) => {
                    lookup_error.get_or_insert(e);
                    (None, None)
                }
            }
        })?;
        if let Some(e) = lookup_error {
            return Err(e);
        }
        for r in records {
            let bytes = serde_json::to_vec(r).map_err(|e| Error::Io(e.to_string()))?;
            self.write(&Self::record_name(r.item_id), &bytes)?;
            self.remove(&Self::tombstone_name(r.item_id))?;
        }
        Ok(())
    }

    fn purge(&mut self, id: Uuid, rev: u64) -> Result<()> {
        self.require_vault()?;
        if let Some(stored) = self.read_record_file(id)?.map(|r| r.rev) {
            if rev <= stored {
                return Err(Error::StaleRevision {
                    id,
                    stored,
                    offered: rev,
                });
            }
        }
        let rev = rev.max(self.read_tombstone_file(id)?.unwrap_or(0));
        let bytes =
            serde_json::to_vec(&TombstoneFile { rev }).map_err(|e| Error::Io(e.to_string()))?;
        // Tombstone first: a crash before the record is removed leaves a record that reads as purged.
        self.write(&Self::tombstone_name(id), &bytes)?;
        self.remove(&Self::record_name(id))
    }

    fn tombstones(&self) -> Result<Vec<Tombstone>> {
        let records: BTreeMap<Uuid, u64> = self
            .ids_with_extension("rkr")?
            .into_iter()
            .filter_map(|id| {
                self.read_record_file(id)
                    .ok()
                    .flatten()
                    .map(|r| (id, r.rev))
            })
            .collect();
        Ok(self
            .all_tombstones()?
            .into_iter()
            // A tombstone that a newer record has superseded (crash between the two steps of a write) is stale.
            .filter(|(id, rev)| records.get(id).is_none_or(|r| r <= rev))
            .map(|(id, rev)| Tombstone { id, rev })
            .collect())
    }
}

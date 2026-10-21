use crate::{check_batch, Error, Record, RecordMeta, Result, Store, Tombstone, VaultHeader};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Keeps everything in memory. Used by tests, and as the reference the other backends are checked against.
#[derive(Debug, Default, Clone)]
pub struct MemoryStore {
    header: Option<VaultHeader>,
    records: BTreeMap<Uuid, Record>,
    tombstones: BTreeMap<Uuid, u64>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Store for MemoryStore {
    fn create(&mut self, header: &VaultHeader) -> Result<()> {
        if self.header.is_some() {
            return Err(Error::AlreadyExists);
        }
        self.header = Some(header.clone());
        Ok(())
    }

    fn header(&self) -> Result<VaultHeader> {
        self.header.clone().ok_or(Error::NotInitialised)
    }

    fn set_header(&mut self, header: &VaultHeader) -> Result<()> {
        let current = self.header.as_ref().ok_or(Error::NotInitialised)?;
        if current.vault_id != header.vault_id {
            return Err(Error::WrongVault);
        }
        self.header = Some(header.clone());
        Ok(())
    }

    fn get(&self, id: Uuid) -> Result<Option<Record>> {
        Ok(self.records.get(&id).cloned())
    }

    fn list(&self) -> Result<Vec<RecordMeta>> {
        Ok(self
            .records
            .values()
            .map(|r| RecordMeta {
                id: r.item_id,
                rev: r.rev,
                size: r.ct.len(),
            })
            .collect())
    }

    fn all(&self) -> Result<Vec<Record>> {
        Ok(self.records.values().cloned().collect())
    }

    fn put_many(&mut self, records: &[Record]) -> Result<()> {
        self.header.as_ref().ok_or(Error::NotInitialised)?;
        check_batch(records, |id| {
            (
                self.records.get(&id).map(|r| r.rev),
                self.tombstones.get(&id).copied(),
            )
        })?;
        for r in records {
            self.tombstones.remove(&r.item_id);
            self.records.insert(r.item_id, r.clone());
        }
        Ok(())
    }

    fn purge(&mut self, id: Uuid, rev: u64) -> Result<()> {
        self.header.as_ref().ok_or(Error::NotInitialised)?;
        if let Some(stored) = self.records.get(&id).map(|r| r.rev) {
            if rev <= stored {
                return Err(Error::StaleRevision {
                    id,
                    stored,
                    offered: rev,
                });
            }
        }
        self.records.remove(&id);
        let t = self.tombstones.entry(id).or_insert(rev);
        *t = (*t).max(rev);
        Ok(())
    }

    fn tombstones(&self) -> Result<Vec<Tombstone>> {
        Ok(self
            .tombstones
            .iter()
            .map(|(&id, &rev)| Tombstone { id, rev })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conformance::{self, Fixture};

    struct Mem;
    impl Fixture for Mem {
        type S = MemoryStore;
        fn fresh(&self) -> MemoryStore {
            MemoryStore::new()
        }
        fn reopen(&self, s: MemoryStore) -> MemoryStore {
            s
        }
    }

    #[test]
    fn memory_store_passes_the_conformance_suite() {
        conformance::run(&Mem);
    }
}

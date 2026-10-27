use crate::index::SearchIndex;
use rahasyakosh_core::crypto::KdfParams;
use rahasyakosh_core::item::{Item, ItemVersion, DEFAULT_HISTORY_LIMIT};
use rahasyakosh_core::secret_key::SecretKey;
use rahasyakosh_core::vault::{Manifest, UnlockedVault, MANIFEST_ID};
use rahasyakosh_store::{Record, Store, Unreadable};
use std::collections::BTreeMap;
use uuid::Uuid;

const SECONDS_PER_DAY: i64 = 86_400;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Store(#[from] rahasyakosh_store::Error),
    #[error(transparent)]
    Core(#[from] rahasyakosh_core::Error),
    #[error("no such item")]
    NotFound(Uuid),
    #[error("no item matches '{0}'")]
    NoMatch(String),
    #[error("'{query}' is ambiguous: {}", titles.join(", "))]
    Ambiguous { query: String, titles: Vec<String> },
    #[error("{0}")]
    State(&'static str),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Tunable behaviour. The defaults follow US-ITM-02 and US-ITM-03.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// Earlier versions kept per item.
    pub history_limit: usize,
    /// Days an item stays in the trash before [`Vault::purge_expired`] deletes it.
    pub trash_retention_days: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            history_limit: DEFAULT_HISTORY_LIMIT,
            trash_retention_days: 30,
        }
    }
}

/// Which items a lookup may return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Live,
    Trashed,
    /// Live items first, then trashed ones.
    Any,
}

/// A stored item that could not be opened at unlock and was left out (see [`Vault::unreadable`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// The item it belongs to, when known.
    pub id: Option<Uuid>,
    /// Where it was found: a file name or `records`.
    pub source: String,
    pub reason: String,
}

impl From<Unreadable> for Skipped {
    fn from(u: Unreadable) -> Self {
        Self {
            id: u.id,
            source: u.source,
            reason: u.reason,
        }
    }
}

/// Something the vault's sealed manifest says is wrong with what the store holds (audit findings P2-1 and P2-2).
/// None of these stops the vault opening. Show them to the user: they mean a sync problem, a restored backup or
/// someone changing the folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    /// The manifest lists this item but the store has no readable record for it.
    Missing { id: Uuid, expected: u64 },
    /// The store holds an older revision of this item than was last written.
    RolledBack { id: Uuid, expected: u64, found: u64 },
    /// An item that was deleted for good is back in the store. It is left out.
    Resurrected { id: Uuid },
    /// A tombstone that the manifest does not vouch for was removed; it would have hidden the item.
    ForgedTombstone { id: Uuid },
    /// The manifest could not be opened, so nothing could be checked.
    ManifestUnreadable { reason: String },
    /// This device had seen the vault with a manifest, but there is none now.
    ManifestMissing,
    /// The whole vault is older than this device last saw it (a restored backup, or a rolled-back sync folder).
    OlderThanSeen { seen: u64, found: u64 },
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing { id, .. } => write!(f, "item {id} is missing from the vault folder"),
            Self::RolledBack { id, expected, found } => write!(
                f,
                "item {id} is at an older version ({found}) than the last one saved ({expected})"
            ),
            Self::Resurrected { id } => write!(f, "item {id} was deleted but its file is back; ignored"),
            Self::ForgedTombstone { id } => write!(f, "a deletion mark for item {id} was not genuine; removed"),
            Self::ManifestUnreadable { .. } => write!(f, "the vault's check list could not be read"),
            Self::ManifestMissing => write!(f, "the vault's check list is missing"),
            Self::OlderThanSeen { seen, found } => write!(
                f,
                "this vault is older than when this device last opened it (write {found}, was {seen})"
            ),
        }
    }
}

struct Entry {
    item: Item,
    rev: u64,
}

/// A failed unlock, with the store returned so the caller can try again.
#[derive(Debug)]
pub struct UnlockFailed<S> {
    pub error: Error,
    pub store: S,
}

/// An unlocked vault: keys, decrypted items and search index, all in memory, backed by a [`Store`].
/// Dropping it wipes the keys, the items and the index.
pub struct Vault<S: Store> {
    store: S,
    keys: UnlockedVault,
    entries: BTreeMap<Uuid, Entry>,
    index: SearchIndex,
    config: Config,
    skipped: Vec<Skipped>,
    manifest: Manifest,
    warnings: Vec<Warning>,
    #[cfg(not(target_family = "wasm"))]
    highwater: Option<crate::HighWater>,
}

/// What `unlock_inner` read.
struct Opened {
    keys: UnlockedVault,
    entries: BTreeMap<Uuid, Entry>,
    index: SearchIndex,
    skipped: Vec<Skipped>,
    manifest: Manifest,
    warnings: Vec<Warning>,
    forged: Vec<Uuid>,
}

impl<S: Store> Vault<S> {
    /// Create a new vault in an empty `store`.
    pub fn create(
        mut store: S,
        password: &str,
        kdf: KdfParams,
        config: Config,
        rng: &mut impl rand_core::CryptoRng,
    ) -> Result<Self> {
        let keys = UnlockedVault::create(password, kdf, rng)?;
        store.create(keys.header())?;
        Ok(Self {
            store,
            keys,
            entries: BTreeMap::new(),
            index: SearchIndex::default(),
            config,
            skipped: vec![],
            manifest: Manifest::default(),
            warnings: vec![],
            #[cfg(not(target_family = "wasm"))]
            highwater: None,
        })
    }

    /// Unlock an existing vault: check the password, decrypt every item once and build the search index.
    pub fn unlock(store: S, password: &str, config: Config) -> Result<Self> {
        Self::try_unlock(store, password, config).map_err(|f| f.error)
    }

    /// Like [`unlock`](Self::unlock), but a failure (typically a wrong password) hands the store back so the caller can try again.
    pub fn try_unlock(
        store: S,
        password: &str,
        config: Config,
    ) -> std::result::Result<Self, UnlockFailed<S>> {
        Self::try_unlock_checked(store, password, config, None)
    }

    /// Like [`try_unlock`](Self::try_unlock), also telling the vault the highest write number ([`seq`](Self::seq))
    /// this device has seen, kept outside the synced folder. A vault below it was rolled back as a whole.
    pub fn try_unlock_checked(
        store: S,
        password: &str,
        config: Config,
        last_seen: Option<u64>,
    ) -> std::result::Result<Self, UnlockFailed<S>> {
        Self::try_unlock_full(store, password, None, config, last_seen)
    }

    /// The general form: also takes the vault's Secret Key, for a vault that has one (see
    /// [`set_secret_key`](Self::set_secret_key)). Without it such a vault fails with
    /// `rahasyakosh_core::Error::SecretKeyRequired`.
    pub fn try_unlock_full(
        mut store: S,
        password: &str,
        secret_key: Option<&SecretKey>,
        config: Config,
        last_seen: Option<u64>,
    ) -> std::result::Result<Self, UnlockFailed<S>> {
        match Self::unlock_inner(&store, password, secret_key, last_seen) {
            Ok(o) => {
                // Forged tombstones are removed so they cannot hide the item or block writes to it. If this
                // fails, the vault still opens; the item stays hidden and the warning stays.
                for id in &o.forged {
                    let _ = store.discard_tombstone(*id);
                }
                let mut v = Self {
                    store,
                    keys: o.keys,
                    entries: o.entries,
                    index: o.index,
                    config,
                    skipped: o.skipped,
                    manifest: o.manifest,
                    warnings: o.warnings,
                    #[cfg(not(target_family = "wasm"))]
                    highwater: None,
                };
                if !o.forged.is_empty() {
                    v.reload_after_discard();
                }
                Ok(v)
            }
            Err(error) => Err(UnlockFailed { error, store }),
        }
    }

    /// After forged tombstones were removed, records they hid are readable again: load them.
    fn reload_after_discard(&mut self) {
        let Ok(readable) = self.store.all_readable() else {
            return;
        };
        for record in readable.records {
            if record.item_id == MANIFEST_ID || self.entries.contains_key(&record.item_id) {
                continue;
            }
            if self
                .manifest
                .gone
                .get(&record.item_id)
                .is_some_and(|g| record.rev <= *g)
            {
                continue;
            }
            if let Ok(item) = self.keys.open_record(&record) {
                self.index.set(&item);
                self.entries.insert(
                    record.item_id,
                    Entry {
                        item,
                        rev: record.rev,
                    },
                );
            }
        }
    }

    fn unlock_inner(
        store: &S,
        password: &str,
        secret_key: Option<&SecretKey>,
        last_seen: Option<u64>,
    ) -> Result<Opened> {
        // The header decides the password first, so a wrong password still fails closed. After that, one bad
        // record (a sync conflict, a damaged file, a hostile file) is skipped and reported, not fatal.
        let keys = store.header()?.unlock_with(password, secret_key)?;
        let readable = store.all_readable()?;
        let mut skipped: Vec<Skipped> =
            readable.unreadable.into_iter().map(Skipped::from).collect();
        let mut warnings = vec![];

        // The manifest, if there is one, is the authority on what should be here.
        let mut manifest = None;
        let mut unreadable_seq = None;
        let mut records = vec![];
        for record in readable.records {
            if record.item_id == MANIFEST_ID {
                match keys.open_manifest(&record) {
                    Ok(m) => manifest = Some(m),
                    Err(e) => {
                        // Keep its revision: the next manifest must be newer, or the store refuses it.
                        unreadable_seq = Some(record.rev);
                        warnings.push(Warning::ManifestUnreadable {
                            reason: e.to_string(),
                        });
                    }
                }
            } else {
                records.push(record);
            }
        }
        let have_manifest = manifest.is_some();
        let manifest_unreadable = unreadable_seq.is_some();
        let mut manifest = manifest.unwrap_or_default();
        manifest.seq = manifest.seq.max(unreadable_seq.unwrap_or(0));
        if !have_manifest && !manifest_unreadable && last_seen.is_some_and(|s| s > 0) {
            warnings.push(Warning::ManifestMissing);
        }
        if let Some(seen) = last_seen {
            if have_manifest && manifest.seq < seen {
                warnings.push(Warning::OlderThanSeen {
                    seen,
                    found: manifest.seq,
                });
            }
        }

        // Tombstones the manifest does not vouch for. (An old vault with no manifest is trusted as it stands,
        // and the manifest written on its first change records them.)
        let mut forged = vec![];
        if have_manifest {
            for t in store.tombstones()? {
                if manifest.gone.get(&t.id).is_none_or(|g| t.rev > *g) {
                    forged.push(t.id);
                    warnings.push(Warning::ForgedTombstone { id: t.id });
                }
            }
        } else {
            for t in store.tombstones()? {
                manifest.gone.insert(t.id, t.rev);
            }
        }

        let mut entries = BTreeMap::new();
        let mut index = SearchIndex::default();
        for record in records {
            if have_manifest
                && manifest
                    .gone
                    .get(&record.item_id)
                    .is_some_and(|g| record.rev <= *g)
            {
                warnings.push(Warning::Resurrected { id: record.item_id });
                continue;
            }
            let item = match keys.open_record(&record) {
                Ok(item) => item,
                Err(e) => {
                    skipped.push(Skipped {
                        id: Some(record.item_id),
                        source: "records".into(),
                        reason: e.to_string(),
                    });
                    continue;
                }
            };
            if have_manifest {
                if let Some(&expected) = manifest.live.get(&record.item_id) {
                    if record.rev < expected {
                        warnings.push(Warning::RolledBack {
                            id: record.item_id,
                            expected,
                            found: record.rev,
                        });
                    }
                }
            } else {
                manifest.live.insert(record.item_id, record.rev);
            }
            index.set(&item);
            entries.insert(
                record.item_id,
                Entry {
                    item,
                    rev: record.rev,
                },
            );
        }
        if have_manifest {
            // Records that are listed but cannot be shown. One that was skipped is already reported.
            for (&id, &expected) in &manifest.live {
                let known = entries.contains_key(&id)
                    || skipped.iter().any(|s| s.id == Some(id))
                    || forged.contains(&id);
                if !known {
                    warnings.push(Warning::Missing { id, expected });
                }
            }
        }
        Ok(Opened {
            keys,
            entries,
            index,
            skipped,
            manifest,
            warnings,
            forged,
        })
    }

    /// Problems found at unlock by comparing the store with the sealed manifest. See [`Warning`].
    pub fn warnings(&self) -> &[Warning] {
        &self.warnings
    }

    /// Remember, in `highwater`, the highest write number this device sees from now on, and record the current one.
    /// Saving is best effort.
    #[cfg(not(target_family = "wasm"))]
    pub fn track(&mut self, highwater: crate::HighWater) {
        self.highwater = Some(highwater);
        self.note_seq();
    }

    #[cfg(not(target_family = "wasm"))]
    fn note_seq(&mut self) {
        let id = self.keys.header().vault_id;
        let seq = self.manifest.seq;
        if let Some(h) = &mut self.highwater {
            let _ = h.record(id, seq);
        }
    }

    #[cfg(target_family = "wasm")]
    fn note_seq(&mut self) {}

    /// Whether this vault is also locked with a Secret Key.
    pub fn has_secret_key(&self) -> bool {
        self.keys.header().secret_key
    }

    /// Lock the vault with a new Secret Key as well as the master password, and return the key to show the user once.
    /// `password` is checked first, so a typing mistake cannot lock the vault with the wrong one.
    pub fn set_secret_key(
        &mut self,
        password: &str,
        rng: &mut impl rand_core::CryptoRng,
    ) -> Result<SecretKey> {
        self.check_password(password)?;
        let sk = self.keys.set_secret_key(password, rng)?;
        self.store.set_header(self.keys.header())?;
        Ok(sk)
    }

    /// Go back to the master password alone.
    pub fn clear_secret_key(
        &mut self,
        password: &str,
        rng: &mut impl rand_core::CryptoRng,
    ) -> Result<()> {
        self.check_password(password)?;
        self.keys.clear_secret_key(password, rng)?;
        self.store.set_header(self.keys.header())?;
        Ok(())
    }

    /// The Secret Key this vault was opened with.
    pub fn secret_key(&self) -> Option<&SecretKey> {
        self.keys.secret_key()
    }

    fn check_password(&self, password: &str) -> Result<()> {
        self.store
            .header()?
            .unlock_with(password, self.keys.secret_key())?;
        Ok(())
    }

    /// The id of this vault.
    pub fn vault_id(&self) -> Uuid {
        self.keys.header().vault_id
    }

    /// How many writes this vault has ever had. A device should remember the highest it has seen, and pass it to
    /// [`try_unlock_checked`](Self::try_unlock_checked) next time.
    pub fn seq(&self) -> u64 {
        self.manifest.seq
    }

    /// Stored items that could not be opened at unlock and are not in this vault. Show them to the user: they are
    /// usually a sync conflict or a damaged file, and may also be someone tampering with the folder. They are
    /// left in the store untouched.
    pub fn unreadable(&self) -> &[Skipped] {
        &self.skipped
    }

    /// Lock: wipe keys and items, and hand the store back.
    pub fn into_store(self) -> S {
        self.store
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    /// Change behaviour such as the history limit and trash retention for the open vault.
    pub fn set_config(&mut self, config: Config) {
        self.config = config;
    }

    pub fn config(&self) -> Config {
        self.config
    }

    // ---- reading ----------------------------------------------------------------------------------

    pub fn get(&self, id: Uuid) -> Option<&Item> {
        self.entries.get(&id).map(|e| &e.item)
    }

    /// The revision currently stored for an item.
    pub fn revision(&self, id: Uuid) -> Option<u64> {
        self.entries.get(&id).map(|e| e.rev)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn sorted(&self, ids: impl Iterator<Item = Uuid>, scope: Scope) -> Vec<&Item> {
        let mut items: Vec<&Item> = ids
            .filter_map(|id| self.get(id))
            .filter(|i| match scope {
                Scope::Live => i.deleted_at.is_none(),
                Scope::Trashed => i.deleted_at.is_some(),
                Scope::Any => true,
            })
            .collect();
        // Live before trashed, favourites first, then by title.
        items.sort_by_cached_key(|i| {
            (
                i.deleted_at.is_some(),
                !i.favourite,
                i.title.to_lowercase(),
                i.id,
            )
        });
        items
    }

    /// Items in `scope`, favourites first, then by title.
    pub fn items(&self, scope: Scope) -> Vec<&Item> {
        self.sorted(self.entries.keys().copied(), scope)
    }

    /// Search live items. Every whitespace-separated term must appear in the title, a URL, a tag or a
    /// non-secret field. An empty query lists everything.
    pub fn search(&self, query: &str) -> Vec<&Item> {
        self.search_in(query, Scope::Live)
    }

    pub fn search_in(&self, query: &str, scope: Scope) -> Vec<&Item> {
        self.sorted(self.index.matching(query).into_iter(), scope)
    }

    /// The single item a user means by `query`: an exact title (ignoring case) wins, otherwise there must be exactly one search hit.
    pub fn resolve(&self, query: &str, scope: Scope) -> Result<&Item> {
        let pool = self.items(scope);
        if let Some(i) = pool.iter().find(|i| i.title.eq_ignore_ascii_case(query)) {
            return Ok(i);
        }
        let hits = self.search_in(query, scope);
        match hits.as_slice() {
            [] => Err(Error::NoMatch(query.to_string())),
            [one] => Ok(one),
            many => Err(Error::Ambiguous {
                query: query.to_string(),
                titles: many
                    .iter()
                    .map(|i| {
                        if i.deleted_at.is_some() {
                            format!("{} (in trash)", i.title)
                        } else {
                            i.title.clone()
                        }
                    })
                    .collect(),
            }),
        }
    }

    /// Every tag in use on live items, with the number of items carrying it, sorted by name.
    pub fn tags(&self) -> Vec<(String, usize)> {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for e in self
            .entries
            .values()
            .filter(|e| e.item.deleted_at.is_none())
        {
            for t in &e.item.tags {
                *counts.entry(t.clone()).or_default() += 1;
            }
        }
        counts.into_iter().collect()
    }

    /// Earlier versions of an item, newest first.
    pub fn history(&self, id: Uuid) -> Result<&[ItemVersion]> {
        Ok(&self
            .entries
            .get(&id)
            .ok_or(Error::NotFound(id))?
            .item
            .history)
    }

    // ---- writing ----------------------------------------------------------------------------------

    /// Seal `items` as new records at revision 1 and store them in one step.
    pub fn add_many(
        &mut self,
        items: Vec<Item>,
        rng: &mut impl rand_core::CryptoRng,
    ) -> Result<()> {
        let mut seen = std::collections::BTreeSet::new();
        for i in &items {
            if self.entries.contains_key(&i.id) || !seen.insert(i.id) {
                return Err(Error::State("an item with this id already exists"));
            }
        }
        let mut manifest = self.next_manifest()?;
        let mut records = items
            .iter()
            .map(|i| self.keys.seal_item(i, 1, rng))
            .collect::<std::result::Result<Vec<Record>, _>>()?;
        for i in &items {
            manifest.live.insert(i.id, 1);
        }
        // The manifest goes last: a crash part-way leaves records newer than it, which is harmless.
        records.push(self.keys.seal_manifest(&manifest, rng)?);
        self.store.put_many(&records)?;
        self.manifest = manifest;
        self.note_seq();
        for item in items {
            self.index.set(&item);
            self.entries.insert(item.id, Entry { item, rev: 1 });
        }
        Ok(())
    }

    pub fn add(&mut self, item: Item, rng: &mut impl rand_core::CryptoRng) -> Result<Uuid> {
        let id = item.id;
        self.add_many(vec![item], rng)?;
        Ok(id)
    }

    /// Replace an item with `new`, already prepared by the caller. On success memory matches the store;
    /// on failure nothing has changed.
    fn commit(&mut self, new: Item, rng: &mut impl rand_core::CryptoRng) -> Result<()> {
        let entry = self.entries.get(&new.id).ok_or(Error::NotFound(new.id))?;
        let rev = entry.rev + 1;
        let record = self.keys.seal_item(&new, rev, rng)?;
        let mut manifest = self.next_manifest()?;
        manifest.live.insert(new.id, rev);
        let sealed = self.keys.seal_manifest(&manifest, rng)?;
        self.store.put_many(&[record, sealed])?;
        self.manifest = manifest;
        self.note_seq();
        self.index.set(&new);
        self.entries.insert(new.id, Entry { item: new, rev });
        Ok(())
    }

    /// Change an item with `change`. If the content ends up different, the old content is saved to its history
    /// (capped at `history_limit`). Returns whether anything changed; if not, nothing is written.
    pub fn edit(
        &mut self,
        id: Uuid,
        now: i64,
        rng: &mut impl rand_core::CryptoRng,
        change: impl FnOnce(&mut Item),
    ) -> Result<bool> {
        let old = &self.entries.get(&id).ok_or(Error::NotFound(id))?.item;
        let mut work = old.clone();
        work.push_history(now, self.config.history_limit);
        change(&mut work);
        if work.id != id {
            return Err(Error::State("an item's id cannot be changed"));
        }
        let content_changed = !old.same_content_as(&work.snapshot(now));
        if !content_changed {
            // Nothing to remember; keep the history as it was.
            work.history = old.history.clone();
            if work.deleted_at == old.deleted_at {
                return Ok(false);
            }
        }
        work.updated_at = now;
        self.commit(work, rng)?;
        Ok(true)
    }

    /// Move an item to the trash. Its content and history are untouched.
    pub fn trash(&mut self, id: Uuid, now: i64, rng: &mut impl rand_core::CryptoRng) -> Result<()> {
        let mut item = self.get(id).ok_or(Error::NotFound(id))?.clone();
        if item.deleted_at.is_some() {
            return Err(Error::State("already in the trash"));
        }
        item.deleted_at = Some(now);
        item.updated_at = now;
        self.commit(item, rng)
    }

    pub fn restore(
        &mut self,
        id: Uuid,
        now: i64,
        rng: &mut impl rand_core::CryptoRng,
    ) -> Result<()> {
        let mut item = self.get(id).ok_or(Error::NotFound(id))?.clone();
        if item.deleted_at.is_none() {
            return Err(Error::State("not in the trash"));
        }
        item.deleted_at = None;
        item.updated_at = now;
        self.commit(item, rng)
    }

    /// The manifest as it will be after one more write.
    fn next_manifest(&self) -> Result<Manifest> {
        let mut m = self.manifest.clone();
        m.seq = m
            .seq
            .checked_add(1)
            .ok_or(Error::State("the vault's write counter is used up"))?;
        Ok(m)
    }

    /// Delete an item for good (live or trashed). Leaves a tombstone in the store.
    pub fn purge(&mut self, id: Uuid, rng: &mut impl rand_core::CryptoRng) -> Result<()> {
        let rev = self.entries.get(&id).ok_or(Error::NotFound(id))?.rev;
        // The manifest first, so a crash leaves a record that reads as deleted, never a tombstone nobody vouches for.
        let mut manifest = self.next_manifest()?;
        manifest.live.remove(&id);
        manifest.gone.insert(id, rev + 1);
        self.store.put(&self.keys.seal_manifest(&manifest, rng)?)?;
        self.manifest = manifest;
        self.note_seq();
        self.store.purge(id, rev + 1)?;
        self.index.remove(id);
        self.entries.remove(&id);
        Ok(())
    }

    /// Delete items that have been in the trash for at least `trash_retention_days`. Returns how many.
    pub fn purge_expired(
        &mut self,
        now: i64,
        rng: &mut impl rand_core::CryptoRng,
    ) -> Result<usize> {
        // At least one day, whatever the config says: 0 would delete everything in the trash at once.
        let keep_for = i64::from(self.config.trash_retention_days.max(1)) * SECONDS_PER_DAY;
        let expired: Vec<Uuid> = self
            .entries
            .values()
            .filter(|e| e.item.deleted_at.is_some_and(|d| now - d >= keep_for))
            .map(|e| e.item.id)
            .collect();
        for id in &expired {
            self.purge(*id, rng)?;
        }
        Ok(expired.len())
    }

    /// Make an earlier version current again. The present content is saved to history first.
    pub fn revert(
        &mut self,
        id: Uuid,
        version: usize,
        now: i64,
        rng: &mut impl rand_core::CryptoRng,
    ) -> Result<()> {
        let mut item = self.get(id).ok_or(Error::NotFound(id))?.clone();
        item.revert_to(version, now, self.config.history_limit)?;
        self.commit(item, rng)
    }

    /// Change the master password. Items are not re-encrypted; only the stored header changes.
    pub fn change_password(
        &mut self,
        new_password: &str,
        kdf: KdfParams,
        rng: &mut impl rand_core::CryptoRng,
    ) -> Result<()> {
        self.keys.change_password(new_password, kdf, rng)?;
        self.store.set_header(self.keys.header())?;
        Ok(())
    }
}

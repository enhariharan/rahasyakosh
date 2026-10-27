use crate::index::SearchIndex;
use rahasyakosh_core::crypto::KdfParams;
use rahasyakosh_core::item::{Item, ItemVersion, DEFAULT_HISTORY_LIMIT};
use rahasyakosh_core::vault::UnlockedVault;
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
        match Self::unlock_inner(&store, password) {
            Ok((keys, entries, index, skipped)) => Ok(Self {
                store,
                keys,
                entries,
                index,
                config,
                skipped,
            }),
            Err(error) => Err(UnlockFailed { error, store }),
        }
    }

    #[allow(clippy::type_complexity)]
    fn unlock_inner(
        store: &S,
        password: &str,
    ) -> Result<(
        UnlockedVault,
        BTreeMap<Uuid, Entry>,
        SearchIndex,
        Vec<Skipped>,
    )> {
        // The header decides the password first, so a wrong password still fails closed. After that, one bad
        // record (a sync conflict, a damaged file, a hostile file) is skipped and reported, not fatal.
        let keys = store.header()?.unlock(password)?;
        let mut entries = BTreeMap::new();
        let mut index = SearchIndex::default();
        let readable = store.all_readable()?;
        let mut skipped: Vec<Skipped> =
            readable.unreadable.into_iter().map(Skipped::from).collect();
        for record in readable.records {
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
            index.set(&item);
            entries.insert(
                record.item_id,
                Entry {
                    item,
                    rev: record.rev,
                },
            );
        }
        Ok((keys, entries, index, skipped))
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
        let records = items
            .iter()
            .map(|i| self.keys.seal_item(i, 1, rng))
            .collect::<std::result::Result<Vec<Record>, _>>()?;
        self.store.put_many(&records)?;
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
        self.store.put(&record)?;
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

    /// Delete an item for good (live or trashed). Leaves a tombstone in the store.
    pub fn purge(&mut self, id: Uuid) -> Result<()> {
        let rev = self.entries.get(&id).ok_or(Error::NotFound(id))?.rev;
        self.store.purge(id, rev + 1)?;
        self.index.remove(id);
        self.entries.remove(&id);
        Ok(())
    }

    /// Delete items that have been in the trash for at least `trash_retention_days`. Returns how many.
    pub fn purge_expired(&mut self, now: i64) -> Result<usize> {
        let keep_for = i64::from(self.config.trash_retention_days) * SECONDS_PER_DAY;
        let expired: Vec<Uuid> = self
            .entries
            .values()
            .filter(|e| e.item.deleted_at.is_some_and(|d| now - d >= keep_for))
            .map(|e| e.item.id)
            .collect();
        for id in &expired {
            self.purge(*id)?;
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

//! In-memory search index over decrypted items (US-ITM-04).
//!
//! Plain substring matching over lower-cased copies of the title, URLs, tags and non-secret field values.
//! A linear scan is fast enough at the sizes we target (see the performance tests) and has no index structure that
//! could drift out of sync or leak. Concealed fields and notes are never indexed. The copies are `Zeroizing`.
use rahasyakosh_core::item::Item;
use std::collections::BTreeMap;
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Default)]
pub(crate) struct SearchIndex {
    entries: BTreeMap<Uuid, Vec<Zeroizing<String>>>,
}

impl SearchIndex {
    pub fn set(&mut self, item: &Item) {
        let mut texts = vec![Zeroizing::new(item.title.to_lowercase())];
        texts.extend(item.urls.iter().map(|u| Zeroizing::new(u.to_lowercase())));
        texts.extend(item.tags.iter().map(|t| Zeroizing::new(t.to_lowercase())));
        texts.extend(
            item.fields
                .iter()
                .filter(|f| !f.concealed)
                .map(|f| Zeroizing::new(f.value.to_lowercase())),
        );
        self.entries.insert(item.id, texts);
    }

    pub fn remove(&mut self, id: Uuid) {
        self.entries.remove(&id);
    }

    /// Ids of items where every whitespace-separated term of `query` occurs in at least one indexed text.
    /// An empty query matches everything.
    pub fn matching(&self, query: &str) -> Vec<Uuid> {
        let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        self.entries
            .iter()
            .filter(|(_, texts)| {
                terms
                    .iter()
                    .all(|t| texts.iter().any(|s| s.contains(t.as_str())))
            })
            .map(|(id, _)| *id)
            .collect()
    }
}

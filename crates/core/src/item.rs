//! Decrypted item model. This is the payload that gets encrypted into a vault record.
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Payload schema written today. Readers accept every earlier schema: new fields are optional with defaults.
/// Schema 2 added [`Item::history`].
pub const ITEM_SCHEMA: u16 = 2;

/// How many earlier versions an item keeps unless told otherwise (US-ITM-02).
pub const DEFAULT_HISTORY_LIMIT: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Login,
    Note,
    Card,
    Identity,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Field {
    pub name: String,
    pub value: String,
    /// Concealed fields are masked in UIs and never printed by default.
    pub concealed: bool,
}

impl std::fmt::Debug for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let v = if self.concealed {
            "[REDACTED]"
        } else {
            self.value.as_str()
        };
        f.debug_struct("Field")
            .field("name", &self.name)
            .field("value", &v)
            .finish()
    }
}

impl Field {
    pub fn text(name: &str, value: &str) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            concealed: false,
        }
    }
    pub fn secret(name: &str, value: &str) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            concealed: true,
        }
    }
}

/// An earlier state of an item's content. History lives inside the item, so one item stays one encrypted record
/// and old passwords are protected exactly like the current one.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct ItemVersion {
    /// When this content stopped being current (the time of the edit that replaced it).
    #[zeroize(skip)]
    pub replaced_at: i64,
    pub title: String,
    pub fields: Vec<Field>,
    pub urls: Vec<String>,
    pub tags: Vec<String>,
    pub notes: String,
    #[zeroize(skip)]
    pub favourite: bool,
}

impl std::fmt::Debug for ItemVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ItemVersion")
            .field("replaced_at", &self.replaced_at)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Item {
    pub schema: u16,
    #[zeroize(skip)]
    pub id: Uuid,
    #[zeroize(skip)]
    pub kind: ItemKind,
    pub title: String,
    pub fields: Vec<Field>,
    pub urls: Vec<String>,
    pub tags: Vec<String>,
    pub notes: String,
    #[zeroize(skip)]
    pub favourite: bool,
    #[zeroize(skip)]
    pub created_at: i64,
    #[zeroize(skip)]
    pub updated_at: i64,
    /// Set when moved to trash.
    #[zeroize(skip)]
    pub deleted_at: Option<i64>,
    /// Earlier versions of the content, newest first. Absent in schema-1 payloads.
    #[serde(default)]
    pub history: Vec<ItemVersion>,
}

impl std::fmt::Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Item")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("title", &self.title)
            .field("fields", &self.fields)
            .finish_non_exhaustive()
    }
}

impl Item {
    pub fn new(kind: ItemKind, title: &str, now: i64, id: Uuid) -> Self {
        Self {
            schema: ITEM_SCHEMA,
            id,
            kind,
            title: title.into(),
            fields: vec![],
            urls: vec![],
            tags: vec![],
            notes: String::new(),
            favourite: false,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            history: vec![],
        }
    }

    pub fn login(title: &str, username: &str, password: &str, now: i64, id: Uuid) -> Self {
        let mut i = Self::new(ItemKind::Login, title, now, id);
        i.fields.push(Field::text("username", username));
        i.fields.push(Field::secret("password", password));
        i
    }

    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case(name))
    }

    /// The current content as a history entry, stamped with the time it is being replaced.
    pub fn snapshot(&self, replaced_at: i64) -> ItemVersion {
        ItemVersion {
            replaced_at,
            title: self.title.clone(),
            fields: self.fields.clone(),
            urls: self.urls.clone(),
            tags: self.tags.clone(),
            notes: self.notes.clone(),
            favourite: self.favourite,
        }
    }

    /// Does the current content equal that of `v`? (Ignores timestamps, trash state and history itself.)
    pub fn same_content_as(&self, v: &ItemVersion) -> bool {
        self.title == v.title
            && self.fields == v.fields
            && self.urls == v.urls
            && self.tags == v.tags
            && self.notes == v.notes
            && self.favourite == v.favourite
    }

    /// Remember the current content before it is changed. Keeps at most `limit` versions, newest first.
    /// Does nothing if the content is identical to the newest entry already.
    pub fn push_history(&mut self, replaced_at: i64, limit: usize) {
        if limit == 0 {
            self.history.clear();
            return;
        }
        if self
            .history
            .first()
            .is_some_and(|v| self.same_content_as(v))
        {
            return;
        }
        self.history.insert(0, self.snapshot(replaced_at));
        self.history.truncate(limit);
    }

    /// Make the content of `history[index]` current again. The present content is saved to history first, so
    /// nothing is lost and the past is never rewritten.
    pub fn revert_to(&mut self, index: usize, now: i64, limit: usize) -> Result<(), crate::Error> {
        let old = self.history.get(index).cloned().ok_or_else(|| {
            crate::Error::InvalidArgument(format!("no version {index} in this item's history"))
        })?;
        self.push_history(now, limit);
        // The text being replaced may be a secret; wipe it rather than just drop it.
        self.title.zeroize();
        self.notes.zeroize();
        self.title = old.title.clone();
        self.fields = old.fields.clone();
        self.urls = old.urls.clone();
        self.tags = old.tags.clone();
        self.notes = old.notes.clone();
        self.favourite = old.favourite;
        self.updated_at = now;
        Ok(())
    }

    /// Set a field's value, keeping its position if it exists, otherwise appending it.
    pub fn set_field(&mut self, name: &str, value: &str, concealed: bool) {
        match self
            .fields
            .iter_mut()
            .find(|f| f.name.eq_ignore_ascii_case(name))
        {
            Some(f) => {
                // Wipe the old value before it is replaced (a plain assignment frees it unwiped).
                f.value.zeroize();
                f.value = value.into();
                f.concealed = concealed;
            }
            None => self.fields.push(Field {
                name: name.into(),
                value: value.into(),
                concealed,
            }),
        }
    }

    /// Remove a field by name (case-insensitive). Returns whether one was removed.
    pub fn remove_field(&mut self, name: &str) -> bool {
        let before = self.fields.len();
        self.fields.retain(|f| !f.name.eq_ignore_ascii_case(name));
        self.fields.len() != before
    }

    /// Case-insensitive match on title, URLs, tags and non-concealed fields.
    pub fn matches(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.title.to_lowercase().contains(&q)
            || self.urls.iter().any(|u| u.to_lowercase().contains(&q))
            || self.tags.iter().any(|t| t.to_lowercase().contains(&q))
            || self
                .fields
                .iter()
                .any(|f| !f.concealed && f.value.to_lowercase().contains(&q))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn login() -> Item {
        Item::login("Site", "alice", "pw", 1, Uuid::from_u128(1))
    }

    #[test]
    fn set_field_replaces_in_place_and_appends() {
        let mut i = login();
        i.set_field("PASSWORD", "new", true);
        assert_eq!(i.fields.len(), 2);
        assert_eq!(i.fields[1].value, "new");
        i.set_field("totp", "JBSWY3DPEHPK3PXP", true);
        assert_eq!(i.fields.len(), 3);
        assert!(i.field("totp").unwrap().concealed);
    }

    #[test]
    fn remove_field_reports_whether_it_removed() {
        let mut i = login();
        assert!(i.remove_field("Username"));
        assert!(!i.remove_field("username"));
        assert!(i.field("username").is_none());
    }

    fn edited(i: &Item, new_password: &str, at: i64) -> Item {
        let mut e = i.clone();
        e.push_history(at, 3);
        e.set_field("password", new_password, true);
        e.updated_at = at;
        e
    }

    #[test]
    fn history_is_newest_first_and_capped() {
        let mut i = login();
        for (n, pw) in ["two", "three", "four", "five"].into_iter().enumerate() {
            i = edited(&i, pw, 10 + n as i64);
        }
        assert_eq!(i.field("password").unwrap().value, "five");
        assert_eq!(i.history.len(), 3, "capped at the limit");
        let kept: Vec<_> = i
            .history
            .iter()
            .map(|v| v.fields[1].value.as_str())
            .collect();
        assert_eq!(
            kept,
            ["four", "three", "two"],
            "newest first; the oldest ('pw') was dropped"
        );
        assert_eq!(i.history[0].replaced_at, 13);
    }

    #[test]
    fn unchanged_content_does_not_add_history() {
        let mut i = login();
        i.push_history(5, 10);
        assert_eq!(i.history.len(), 1);
        i.push_history(6, 10); // content still equals the newest entry
        assert_eq!(i.history.len(), 1);
        i.set_field("password", "changed", true);
        i.push_history(7, 10);
        assert_eq!(i.history.len(), 2);
    }

    #[test]
    fn limit_zero_keeps_no_history() {
        let mut i = edited(&login(), "x", 5);
        i.push_history(6, 0);
        assert!(i.history.is_empty());
    }

    #[test]
    fn revert_restores_content_and_keeps_the_past() {
        let v1 = login();
        let mut v3 = edited(&edited(&v1, "two", 10), "three", 20);
        assert_eq!(v3.history.len(), 2);
        // history[1] is the original content ("pw"); revert to it.
        v3.revert_to(1, 30, 10).unwrap();
        assert_eq!(v3.field("password").unwrap().value, "pw");
        assert_eq!(v3.updated_at, 30);
        // The "three" content we just left is now the newest history entry, and nothing was lost.
        assert_eq!(v3.history[0].fields[1].value, "three");
        assert_eq!(v3.history.len(), 3);
        assert!(v3.revert_to(9, 31, 10).is_err());
    }

    #[test]
    fn schema_one_payloads_without_history_still_load() {
        let json = r#"{"schema":1,"id":"01900000-0000-7000-8000-000000000001","kind":"login","title":"Old",
            "fields":[],"urls":[],"tags":[],"notes":"","favourite":false,"created_at":1,"updated_at":1,"deleted_at":null}"#;
        let i: Item = serde_json::from_str(json).unwrap();
        assert_eq!(i.schema, 1);
        assert!(i.history.is_empty());
    }

    #[test]
    fn history_round_trips_through_json_and_hides_content_in_debug() {
        let i = edited(&login(), "secret-new", 9);
        let back: Item = serde_json::from_str(&serde_json::to_string(&i).unwrap()).unwrap();
        assert_eq!(back, i);
        let shown = format!("{:?}", i.history[0]);
        assert!(!shown.contains("pw") && !shown.contains("Site"), "{shown}");
    }
}

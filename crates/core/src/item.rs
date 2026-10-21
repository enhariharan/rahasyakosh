//! Decrypted item model. This is the payload that gets encrypted into a vault record.
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub const ITEM_SCHEMA: u16 = 1;

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
        let v = if self.concealed { "[REDACTED]" } else { self.value.as_str() };
        f.debug_struct("Field").field("name", &self.name).field("value", &v).finish()
    }
}

impl Field {
    pub fn text(name: &str, value: &str) -> Self {
        Self { name: name.into(), value: value.into(), concealed: false }
    }
    pub fn secret(name: &str, value: &str) -> Self {
        Self { name: name.into(), value: value.into(), concealed: true }
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
        }
    }

    pub fn login(title: &str, username: &str, password: &str, now: i64, id: Uuid) -> Self {
        let mut i = Self::new(ItemKind::Login, title, now, id);
        i.fields.push(Field::text("username", username));
        i.fields.push(Field::secret("password", password));
        i
    }

    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name.eq_ignore_ascii_case(name))
    }

    /// Case-insensitive match on title, URLs, tags and non-concealed fields.
    pub fn matches(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.title.to_lowercase().contains(&q)
            || self.urls.iter().any(|u| u.to_lowercase().contains(&q))
            || self.tags.iter().any(|t| t.to_lowercase().contains(&q))
            || self.fields.iter().any(|f| !f.concealed && f.value.to_lowercase().contains(&q))
    }
}

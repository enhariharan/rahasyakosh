//! Everything the screens do, with no GUI in it. The Dioxus layer in `main.rs` only draws this and forwards clicks, so the
//! behaviour (locking, hiding, copying, forms, trash, settings) is tested here without a window.
use rahasyakosh_app::{Locked, Scope, Session, Settings, UnlockError};
use rahasyakosh_core::generator::{self, PasswordSpec};
use rahasyakosh_core::item::{Field, Item, ItemVersion};
use rahasyakosh_core::totp::Totp;
use rahasyakosh_store::{Store, Uuid};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// The clipboard, as the model needs it. The real one wraps the desktop's tools; tests use a fake.
pub trait Clipboard {
    fn copy(&mut self, text: &str) -> Result<(), String>;
    /// Clear the clipboard only if it still holds exactly `text`. Returns whether it did.
    fn clear_if_unchanged(&mut self, text: &str) -> Result<bool, String>;
}

pub struct SystemClipboard;

impl Clipboard for SystemClipboard {
    fn copy(&mut self, text: &str) -> Result<(), String> {
        rahasyakosh_clipboard::Backend::detect()
            .and_then(|b| b.copy(text))
            .map_err(|e| e.to_string())
    }

    fn clear_if_unchanged(&mut self, text: &str) -> Result<bool, String> {
        rahasyakosh_clipboard::Backend::detect()
            .and_then(|b| b.clear_if_unchanged(text))
            .map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeKind {
    Info,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub text: String,
    pub kind: NoticeKind,
    until: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    Items,
    Trash,
    Settings,
    /// The add/edit form is open (its content is in `Model::form`).
    Form,
}

/// One line in the item list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemRow {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub favourite: bool,
}

/// A field as the detail pane shows it. A secret's value is present only while revealed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldView {
    pub name: String,
    pub label: String,
    pub secret: bool,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TotpView {
    pub code: String,
    pub seconds_left: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRow {
    pub index: usize,
    pub when: String,
    pub changed: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detail {
    pub id: Uuid,
    pub title: String,
    pub favourite: bool,
    pub trashed: bool,
    pub fields: Vec<FieldView>,
    pub urls: Vec<String>,
    pub tags: Vec<String>,
    pub totp: Option<TotpView>,
    pub history: Vec<HistoryRow>,
}

/// The add/edit form. It holds secrets, so it wipes itself when dropped.
#[derive(Clone, Default, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct Form {
    #[zeroize(skip)]
    pub id: Option<Uuid>,
    pub title: String,
    pub username: String,
    pub password: String,
    pub url: String,
    pub totp: String,
    pub notes: String,
    /// Comma-separated.
    pub tags: String,
    #[zeroize(skip)]
    pub favourite: bool,
}

impl std::fmt::Debug for Form {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Form")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl Form {
    pub fn from_item(item: &Item) -> Self {
        let value = |n: &str| item.field(n).map(|f| f.value.clone()).unwrap_or_default();
        Self {
            id: Some(item.id),
            title: item.title.clone(),
            username: value("username"),
            password: value("password"),
            url: item.urls.first().cloned().unwrap_or_default(),
            totp: value("totp"),
            notes: item.notes.clone(),
            tags: item.tags.join(", "),
            favourite: item.favourite,
        }
    }

    /// Why this form cannot be saved, if it cannot.
    pub fn problem(&self) -> Option<String> {
        if self.title.trim().is_empty() {
            return Some("Give the item a title.".into());
        }
        let t = self.totp.trim();
        if !t.is_empty() {
            let ok = if t.starts_with("otpauth://") {
                Totp::from_uri(t).is_ok()
            } else {
                Totp::from_base32(t).is_ok()
            };
            if !ok {
                return Some(
                    "The two-factor secret is not valid (expected base32 or an otpauth:// link)."
                        .into(),
                );
            }
        }
        None
    }

    /// Write the form's values into `item`.
    pub fn apply(&self, item: &mut Item) {
        item.title = self.title.trim().to_string();
        item.set_field("username", self.username.trim(), false);
        item.set_field("password", &self.password, true);
        let url = self.url.trim();
        item.urls = if url.is_empty() {
            vec![]
        } else {
            vec![url.to_string()]
        };
        let totp = self.totp.trim();
        if totp.is_empty() {
            item.remove_field("totp");
        } else {
            item.set_field("totp", totp, true);
        }
        item.notes = self.notes.clone();
        let mut tags: Vec<String> = Vec::new();
        for t in self
            .tags
            .split(',')
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            if !tags.iter().any(|x| x.eq_ignore_ascii_case(t)) {
                tags.push(t.to_string());
            }
        }
        item.tags = tags;
        item.favourite = self.favourite;
    }
}

/// Check the first-run choice and return the master password to create the vault with.
///
/// Either the generated `passphrase` (which the user must have confirmed they wrote down) or their own password
/// (typed twice, and strong enough).
pub fn new_master_password(
    own: bool,
    passphrase: &str,
    written_down: bool,
    password: &str,
    repeat: &str,
) -> Result<String, String> {
    if !own {
        return if written_down {
            Ok(passphrase.to_string())
        } else {
            Err("Tick the box to confirm you have written the passphrase down.".into())
        };
    }
    if password != repeat {
        return Err("The two passwords do not match.".into());
    }
    let s = rahasyakosh_strength::estimate(password, &[]);
    if s.score < rahasyakosh_strength::Strength::MIN_MASTER_SCORE {
        return Err(
            "That password is too weak. Choose a longer one, or use the generated passphrase."
                .into(),
        );
    }
    Ok(password.to_string())
}

/// "today", "1 day ago", "5 days ago".
pub fn days_ago(then: i64, now: i64) -> String {
    match (now - then).max(0) / 86_400 {
        0 => "today".into(),
        1 => "1 day ago".into(),
        n => format!("{n} days ago"),
    }
}

/// Which parts differ between two versions, never their values.
pub fn changed_parts(a: &ItemVersion, b: &ItemVersion) -> String {
    let mut parts: Vec<String> = Vec::new();
    if a.title != b.title {
        parts.push("title".into());
    }
    if a.urls != b.urls {
        parts.push("url".into());
    }
    if a.tags != b.tags {
        parts.push("tags".into());
    }
    if a.notes != b.notes {
        parts.push("notes".into());
    }
    if a.favourite != b.favourite {
        parts.push("favourite".into());
    }
    let mut names: Vec<&str> = a
        .fields
        .iter()
        .chain(&b.fields)
        .map(|f| f.name.as_str())
        .collect();
    names.sort_unstable();
    names.dedup();
    for name in names {
        let find = |v: &ItemVersion| {
            v.fields
                .iter()
                .find(|f| f.name == name)
                .map(|f| f.value.clone())
        };
        if find(a) != find(b) {
            parts.push(name.to_string());
        }
    }
    if parts.is_empty() {
        "no visible change".into()
    } else {
        parts.join(", ")
    }
}

/// Persists settings; `None` in tests.
pub type SaveSettings = Box<dyn FnMut(&Settings)>;

pub struct Model<S: Store, C: Clipboard> {
    pub session: Session<S>,
    clipboard: C,
    save_settings: Option<SaveSettings>,
    pub view: View,
    pub query: String,
    pub selected: Option<Uuid>,
    pub form: Option<Form>,
    pub notice: Option<Notice>,
    /// Shown on the unlock screen after a wrong password.
    pub unlock_error: Option<String>,
}

impl<S: Store, C: Clipboard> Model<S, C> {
    pub fn new(session: Session<S>, clipboard: C, save_settings: Option<SaveSettings>) -> Self {
        Self {
            session,
            clipboard,
            save_settings,
            view: View::Items,
            query: String::new(),
            selected: None,
            form: None,
            notice: None,
            unlock_error: None,
        }
    }

    pub fn is_unlocked(&self) -> bool {
        self.session.is_unlocked()
    }

    fn info(&mut self, text: impl Into<String>, now: i64) {
        self.notice = Some(Notice {
            text: text.into(),
            kind: NoticeKind::Info,
            until: now + 4,
        });
    }

    fn error(&mut self, text: impl Into<String>, now: i64) {
        self.notice = Some(Notice {
            text: text.into(),
            kind: NoticeKind::Error,
            until: now + 8,
        });
    }

    /// Everything that should be forgotten when the vault locks.
    fn forget_screen(&mut self) {
        self.view = View::Items;
        self.query.clear();
        self.selected = None;
        self.form = None;
    }

    // ---- locking --------------------------------------------------------------------------------------

    pub fn unlock(&mut self, password: &str, now: i64) {
        match self.session.unlock(password, now) {
            Ok(u) => {
                self.unlock_error = None;
                if u.purged > 0 {
                    self.info(
                        format!(
                            "Deleted {} item(s) that had been in the trash too long.",
                            u.purged
                        ),
                        now,
                    );
                }
            }
            // The unlock screen shows the live countdown, so the messages do not repeat the wait.
            Err(UnlockError::WrongPassword { .. }) => {
                self.unlock_error = Some("Wrong password.".into())
            }
            Err(UnlockError::LockedOut { .. }) => {
                self.unlock_error = Some("Too many wrong passwords.".into());
            }
            Err(e) => self.unlock_error = Some(e.to_string()),
        }
    }

    /// Lock now, wiping everything, and clear the clipboard if it still holds a secret we copied.
    pub fn lock(&mut self) {
        let pending = self.session.lock();
        self.forget_screen();
        if let Some(v) = pending {
            let _ = self.clipboard.clear_if_unchanged(&v);
        }
    }

    /// Call about once a second with the current time. Applies the inactivity lock, hides revealed secrets
    /// on time, clears a copied secret when due, and expires notices.
    pub fn tick(&mut self, now: i64) {
        let t = self.session.tick(now);
        if t.locked {
            self.forget_screen();
            if let Some(v) = t.clear_now {
                let _ = self.clipboard.clear_if_unchanged(&v);
            }
        }
        if let Some(v) = self.session.clipboard_due(now) {
            let _ = self.clipboard.clear_if_unchanged(&v);
        }
        if self.notice.as_ref().is_some_and(|n| now >= n.until) {
            self.notice = None;
        }
    }

    /// The user did something (clicked, typed, moved the pointer).
    pub fn activity(&mut self, now: i64) {
        self.session.touch(now);
    }

    /// The window lost focus.
    pub fn window_blurred(&mut self) {
        if self.session.settings().lock_on_blur && self.is_unlocked() {
            self.lock();
        }
    }

    // ---- lists ---------------------------------------------------------------------------------------

    pub fn rows(&self) -> Vec<ItemRow> {
        let Some(v) = self.session.peek() else {
            return vec![];
        };
        let to_row = |i: &Item| ItemRow {
            id: i.id,
            title: i.title.clone(),
            subtitle: i
                .field("username")
                .map(|f| f.value.clone())
                .unwrap_or_default(),
            favourite: i.favourite,
        };
        match self.view {
            View::Trash => v.items(Scope::Trashed).into_iter().map(to_row).collect(),
            _ => v
                .search_in(&self.query, Scope::Live)
                .into_iter()
                .map(to_row)
                .collect(),
        }
    }

    pub fn trash_count(&self) -> usize {
        self.session
            .peek()
            .map_or(0, |v| v.items(Scope::Trashed).len())
    }

    pub fn show(&mut self, view: View, now: i64) {
        self.activity(now);
        self.session.hide_all();
        self.view = view;
        self.selected = None;
        self.form = None;
    }

    pub fn select(&mut self, id: Uuid, now: i64) {
        self.activity(now);
        if self.selected != Some(id) {
            self.session.hide_all();
        }
        self.selected = Some(id);
    }

    // ---- the detail pane -----------------------------------------------------------------------------

    pub fn detail(&self, now: i64) -> Option<Detail> {
        let id = self.selected?;
        let item = self.session.peek()?.get(id)?;
        let reveal = |name: &str| self.session.is_revealed(id, name, now);
        let mut fields = Vec::new();
        for f in &item.fields {
            if f.name == "totp" {
                continue; // shown as a live code, never as the seed
            }
            fields.push(FieldView {
                name: f.name.clone(),
                label: label(&f.name),
                secret: f.concealed,
                value: (!f.concealed || reveal(&f.name)).then(|| f.value.clone()),
            });
        }
        if !item.notes.is_empty() {
            fields.push(FieldView {
                name: "notes".into(),
                label: "Notes".into(),
                secret: true,
                value: reveal("notes").then(|| item.notes.clone()),
            });
        }
        let totp = item.field("totp").and_then(|f| {
            let t = if f.value.starts_with("otpauth://") {
                Totp::from_uri(&f.value).ok()?
            } else {
                Totp::from_base32(&f.value).ok()?
            };
            let ts = now.max(0) as u64;
            Some(TotpView {
                code: t.code_at(ts),
                seconds_left: t.seconds_remaining(ts),
            })
        });
        let mut newer = item.snapshot(now);
        let mut history = Vec::new();
        for (i, v) in item.history.iter().enumerate() {
            history.push(HistoryRow {
                index: i,
                when: days_ago(v.replaced_at, now),
                changed: changed_parts(v, &newer),
            });
            newer = v.clone();
        }
        Some(Detail {
            id,
            title: item.title.clone(),
            favourite: item.favourite,
            trashed: item.deleted_at.is_some(),
            fields,
            urls: item.urls.clone(),
            tags: item.tags.clone(),
            totp,
            history,
        })
    }

    pub fn reveal(&mut self, field: &str, now: i64) {
        self.activity(now);
        if let Some(id) = self.selected {
            let _ = self.session.reveal(id, field, now);
        }
    }

    pub fn hide(&mut self, field: &str, now: i64) {
        self.activity(now);
        if let Some(id) = self.selected {
            self.session.hide(id, field);
        }
    }

    /// Copy a field (or `"totp"` for the current code) to the clipboard and schedule it to be cleared.
    pub fn copy(&mut self, field: &str, now: i64) {
        self.activity(now);
        let Some(id) = self.selected else { return };
        let value = {
            let Some(item) = self.session.peek().and_then(|v| v.get(id)) else {
                return;
            };
            match field {
                "notes" => Some(item.notes.clone()),
                "totp" => item.field("totp").and_then(|f| {
                    let t = if f.value.starts_with("otpauth://") {
                        Totp::from_uri(&f.value).ok()?
                    } else {
                        Totp::from_base32(&f.value).ok()?
                    };
                    Some(t.code_at(now.max(0) as u64))
                }),
                "url" => item.urls.first().cloned(),
                name => item.field(name).map(|f| f.value.clone()),
            }
        };
        let Some(value) = value else { return };
        match self.clipboard.copy(&value) {
            Ok(()) => {
                let secs = self.session.settings().clipboard_clear_secs;
                self.session.copied(&value, now);
                if secs == 0 {
                    self.info(
                        "Copied. It stays on the clipboard until you replace it.",
                        now,
                    );
                } else {
                    self.info(format!("Copied. The clipboard clears in {secs} s."), now);
                }
            }
            Err(e) => self.error(e, now),
        }
    }

    // ---- editing ---------------------------------------------------------------------------------------

    pub fn start_new(&mut self, now: i64) {
        self.activity(now);
        self.view = View::Form;
        self.form = Some(Form::default());
        self.selected = None;
    }

    pub fn start_edit(&mut self, now: i64) {
        self.activity(now);
        let form = self
            .selected
            .and_then(|id| self.session.peek()?.get(id))
            .map(Form::from_item);
        if let Some(f) = form {
            self.view = View::Form;
            self.form = Some(f);
        }
    }

    pub fn cancel_form(&mut self, now: i64) {
        self.activity(now);
        self.form = None;
        self.view = View::Items;
    }

    /// Fill the form's password with a new random one.
    pub fn generate_password(&mut self, length: usize) {
        if let Some(f) = &mut self.form {
            let spec = PasswordSpec {
                length,
                ..Default::default()
            };
            if let Ok(p) = generator::generate_password(&spec, &mut rand::rng()) {
                f.password = p.to_string();
            }
        }
    }

    pub fn save_form(&mut self, now: i64) {
        self.activity(now);
        let Some(form) = self.form.clone() else {
            return;
        };
        if let Some(problem) = form.problem() {
            self.error(problem, now);
            return;
        }
        let mut rng = rand::rng();
        let result: Result<Uuid, String> = (|| {
            let vault = self
                .session
                .vault(now)
                .map_err(|Locked| "The vault is locked.".to_string())?;
            match form.id {
                Some(id) => {
                    vault
                        .edit(id, now, &mut rng, |item| form.apply(item))
                        .map_err(|e| e.to_string())?;
                    Ok(id)
                }
                None => {
                    let mut item = Item::new(
                        rahasyakosh_core::item::ItemKind::Login,
                        "",
                        now,
                        Uuid::now_v7(),
                    );
                    item.fields.push(Field::text("username", ""));
                    form.apply(&mut item);
                    vault.add(item, &mut rng).map_err(|e| e.to_string())
                }
            }
        })();
        match result {
            Ok(id) => {
                self.form = None;
                self.view = View::Items;
                self.selected = Some(id);
                self.info("Saved.", now);
            }
            Err(e) if !self.is_unlocked() => {
                self.forget_screen();
                let _ = e;
            }
            Err(e) => self.error(e, now),
        }
    }

    pub fn toggle_favourite(&mut self, now: i64) {
        if let Some(id) = self.selected {
            let fav = self
                .session
                .peek()
                .and_then(|v| v.get(id))
                .map(|i| i.favourite);
            self.edit_selected(id, now, move |i| i.favourite = !fav.unwrap_or(false));
        }
    }

    fn edit_selected(&mut self, id: Uuid, now: i64, change: impl FnOnce(&mut Item)) {
        self.activity(now);
        let r = match self.session.vault(now) {
            Ok(v) => v
                .edit(id, now, &mut rand::rng(), change)
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Err(Locked) => Err("The vault is locked.".into()),
        };
        if let Err(e) = r {
            self.error(e, now);
        }
    }

    // ---- trash and history ---------------------------------------------------------------------------

    pub fn trash_selected(&mut self, now: i64) {
        self.activity(now);
        let Some(id) = self.selected else { return };
        match self
            .session
            .vault(now)
            .map(|v| v.trash(id, now, &mut rand::rng()))
        {
            Ok(Ok(())) => {
                self.selected = None;
                self.info("Moved to the trash.", now);
            }
            Ok(Err(e)) => self.error(e.to_string(), now),
            Err(Locked) => self.forget_screen(),
        }
    }

    pub fn restore_selected(&mut self, now: i64) {
        self.activity(now);
        let Some(id) = self.selected else { return };
        match self
            .session
            .vault(now)
            .map(|v| v.restore(id, now, &mut rand::rng()))
        {
            Ok(Ok(())) => {
                self.selected = None;
                self.info("Restored.", now);
            }
            Ok(Err(e)) => self.error(e.to_string(), now),
            Err(Locked) => self.forget_screen(),
        }
    }

    pub fn purge_selected(&mut self, now: i64) {
        self.activity(now);
        let Some(id) = self.selected else { return };
        match self.session.vault(now).map(|v| v.purge(id)) {
            Ok(Ok(())) => {
                self.selected = None;
                self.info("Deleted for good.", now);
            }
            Ok(Err(e)) => self.error(e.to_string(), now),
            Err(Locked) => self.forget_screen(),
        }
    }

    pub fn revert_selected(&mut self, version: usize, now: i64) {
        self.activity(now);
        let Some(id) = self.selected else { return };
        match self
            .session
            .vault(now)
            .map(|v| v.revert(id, version, now, &mut rand::rng()))
        {
            Ok(Ok(())) => {
                self.session.hide_all();
                self.info(
                    "Went back to that version. The previous content is kept in the history.",
                    now,
                );
            }
            Ok(Err(e)) => self.error(e.to_string(), now),
            Err(Locked) => self.forget_screen(),
        }
    }

    // ---- settings and the master password ---------------------------------------------------------------

    pub fn apply_settings(&mut self, settings: Settings, now: i64) {
        self.activity(now);
        self.session.set_settings(settings);
        if let Some(save) = &mut self.save_settings {
            save(self.session.settings());
        }
        self.info("Settings saved.", now);
    }

    /// Change the master password. Returns an error message if it is refused.
    pub fn change_master_password(
        &mut self,
        new: &str,
        again: &str,
        now: i64,
    ) -> Result<(), String> {
        self.activity(now);
        if new != again {
            return Err("The two passwords do not match.".into());
        }
        let s = rahasyakosh_strength::estimate(new, &[]);
        if s.score < rahasyakosh_strength::Strength::MIN_MASTER_SCORE {
            return Err(format!(
                "That password is too weak ({}). {}",
                s.label(),
                s.warning
                    .unwrap_or_else(|| "Choose a longer one or use a generated passphrase.".into())
            ));
        }
        let kdf = self.session.settings().kdf_params();
        let vault = self
            .session
            .vault(now)
            .map_err(|Locked| "The vault is locked.".to_string())?;
        vault
            .change_password(new, kdf, &mut rand::rng())
            .map_err(|e| e.to_string())?;
        self.info("Master password changed.", now);
        Ok(())
    }
}

fn label(name: &str) -> String {
    let mut c = name.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

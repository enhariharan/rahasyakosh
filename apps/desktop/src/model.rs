//! Everything the screens do, with no GUI in it. The Dioxus layer in `main.rs` only draws this and forwards clicks, so the
//! behaviour (locking, hiding, copying, forms, trash, settings) is tested here without a window.
use crate::i18n::{t, trf, Msg};
use rahasyakosh_app::{Language, Locked, Scope, Session, Settings, UnlockError};
use rahasyakosh_core::crypto::KdfParams;
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
    /// The item that "Undo" brings back from the trash, when this notice offers it.
    pub undo: Option<Uuid>,
    until: i64,
}

/// What a screen reader should say next. It lives in a region that is always on the page, because
/// a region added at the moment of the message is often not read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Announcement {
    pub text: String,
    /// Errors interrupt (an alert); everything else waits its turn.
    pub urgent: bool,
    count: u32,
}

impl Announcement {
    /// The text for the region. Every message changes it, even a repeat of the last one,
    /// so the reader speaks it again.
    pub fn spoken(&self) -> String {
        if self.count % 2 == 1 {
            format!("{}\u{200b}", self.text)
        } else {
            self.text.clone()
        }
    }
}

/// The file formats Settings can export to. All three hold every password in plain text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportFormat {
    #[default]
    Text,
    Csv,
    Json,
}

impl ExportFormat {
    pub const ALL: [ExportFormat; 3] = [Self::Text, Self::Csv, Self::Json];

    /// The value of the format list's option, and the file name extension.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Text => "txt",
            Self::Csv => "csv",
            Self::Json => "json",
        }
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.extension() == ext)
    }

    pub fn label(self) -> Msg {
        match self {
            Self::Text => Msg::ExportFormatText,
            Self::Csv => Msg::ExportFormatCsv,
            Self::Json => Msg::ExportFormatJson,
        }
    }

    fn render(self, items: &[Item]) -> Result<zeroize::Zeroizing<String>, String> {
        match self {
            Self::Text => Ok(rahasyakosh_transfer::export_txt(items)),
            Self::Csv => rahasyakosh_transfer::export_csv(items).map_err(|e| e.to_string()),
            Self::Json => rahasyakosh_transfer::export_json(items).map_err(|e| e.to_string()),
        }
    }
}

/// `path` with its extension changed to `format`'s, if it ends in one of the export extensions (so choosing another
/// format changes the suggested name but leaves a name the user typed alone).
pub fn with_export_extension(path: &str, format: ExportFormat) -> String {
    match path.rsplit_once('.') {
        Some((stem, ext)) if ExportFormat::from_extension(ext).is_some() => {
            format!("{stem}.{}", format.extension())
        }
        _ => path.to_string(),
    }
}

/// Where the export file is suggested to go: the home folder, named for the day.
pub fn default_export_path(now: i64, format: ExportFormat) -> String {
    let (y, m, d) = civil_date(now.div_euclid(86_400));
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    format!(
        "{home}/rahasyakosh-export-{y:04}-{m:02}-{d:02}.{}",
        format.extension()
    )
}

/// Year, month and day for a number of days since 1970-01-01 (proleptic Gregorian calendar).
fn civil_date(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// Create `path` (which must not exist) readable by its owner only, write `bytes` and flush them to disk.
fn write_new_private_file(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    let mut f = opts.open(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

/// The notice shown when a screen reader starts or stops while the app is open (see `platform`). The window is only
/// picked up by a screen reader that was started late once the app has gone back to the unlock screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReaderNotice {
    /// A screen reader started (true) or stopped (false).
    pub enabled: bool,
    /// The vault was open, so pressing OK locks it. If it was already on the unlock screen, OK just closes the notice.
    pub lock_first: bool,
}

impl ReaderNotice {
    /// The wording of the notice.
    pub fn message(self) -> Msg {
        match (self.enabled, self.lock_first) {
            (true, true) => Msg::ReaderEnabledLock,
            (false, true) => Msg::ReaderDisabledLock,
            (true, false) => Msg::ReaderEnabledUnlock,
            (false, false) => Msg::ReaderDisabledUnlock,
        }
    }
}

/// Why a form cannot be saved: which box is wrong and what to tell the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormError {
    /// The `id` of the input to mark and focus.
    pub field_id: &'static str,
    pub text: String,
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

    /// The `id` of the input `problem` is about.
    pub fn problem_field(&self) -> &'static str {
        if self.title.trim().is_empty() {
            "f-title"
        } else {
            "f-totp"
        }
    }

    /// Why this form cannot be saved, if it cannot.
    pub fn problem(&self, lang: Language) -> Option<String> {
        if self.title.trim().is_empty() {
            return Some(t(lang, Msg::ErrTitle).into());
        }
        let secret = self.totp.trim();
        if !secret.is_empty() {
            let ok = if secret.starts_with("otpauth://") {
                Totp::from_uri(secret).is_ok()
            } else {
                Totp::from_base32(secret).is_ok()
            };
            if !ok {
                return Some(t(lang, Msg::ErrTotp).into());
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
    lang: Language,
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
            Err(t(lang, Msg::ErrTick).into())
        };
    }
    if password != repeat {
        return Err(t(lang, Msg::ErrMismatch).into());
    }
    let s = rahasyakosh_strength::estimate(password, &[]);
    if s.score < rahasyakosh_strength::Strength::MIN_MASTER_SCORE {
        return Err(t(lang, Msg::ErrWeakGenerated).into());
    }
    Ok(password.to_string())
}

/// "today", "1 day ago", "5 days ago".
pub fn days_ago(lang: Language, then: i64, now: i64) -> String {
    match (now - then).max(0) / 86_400 {
        0 => t(lang, Msg::Today).into(),
        1 => t(lang, Msg::OneDayAgo).into(),
        n => trf(lang, Msg::DaysAgo, &[&n]),
    }
}

/// Which parts differ between two versions, never their values.
pub fn changed_parts(lang: Language, a: &ItemVersion, b: &ItemVersion) -> String {
    let mut parts: Vec<String> = Vec::new();
    if a.title != b.title {
        parts.push(t(lang, Msg::PartTitle).into());
    }
    if a.urls != b.urls {
        parts.push(t(lang, Msg::PartUrl).into());
    }
    if a.tags != b.tags {
        parts.push(t(lang, Msg::PartTags).into());
    }
    if a.notes != b.notes {
        parts.push(t(lang, Msg::PartNotes).into());
    }
    if a.favourite != b.favourite {
        parts.push(t(lang, Msg::PartFavourite).into());
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
            parts.push(label_lower(lang, name));
        }
    }
    if parts.is_empty() {
        t(lang, Msg::NoChange).into()
    } else {
        parts.join(", ")
    }
}

/// Persists settings; `None` in tests.
/// Saves the settings; the error says why it could not.
pub type SaveSettings = Box<dyn FnMut(&Settings) -> Result<(), String>>;

pub struct Model<S: Store, C: Clipboard> {
    pub session: Session<S>,
    clipboard: C,
    save_settings: Option<SaveSettings>,
    pub view: View,
    pub query: String,
    pub selected: Option<Uuid>,
    pub form: Option<Form>,
    pub notice: Option<Notice>,
    pub announcement: Announcement,
    /// Set when saving the form failed; shown next to the box that is wrong.
    pub form_error: Option<FormError>,
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
            announcement: Announcement::default(),
            form_error: None,
            unlock_error: None,
        }
    }

    /// The language the settings ask for.
    pub fn lang(&self) -> Language {
        self.session.settings().language
    }

    pub fn is_unlocked(&self) -> bool {
        self.session.is_unlocked()
    }

    /// Have a screen reader say `text`.
    fn say(&mut self, text: impl Into<String>, urgent: bool) {
        self.announcement = Announcement {
            text: text.into(),
            urgent,
            count: self.announcement.count + 1,
        };
    }

    fn info(&mut self, text: impl Into<String>, now: i64) {
        let text = text.into();
        self.say(text.clone(), false);
        self.notice = Some(Notice {
            text,
            kind: NoticeKind::Info,
            undo: None,
            until: now + 4,
        });
    }

    fn error(&mut self, text: impl Into<String>, now: i64) {
        let text = text.into();
        self.say(text.clone(), true);
        self.notice = Some(Notice {
            text,
            kind: NoticeKind::Error,
            undo: None,
            until: now + 8,
        });
    }

    /// Everything that should be forgotten when the vault locks.
    fn forget_screen(&mut self) {
        self.view = View::Items;
        self.query.clear();
        self.selected = None;
        self.form = None;
        self.form_error = None;
        let text = t(self.lang(), Msg::VaultIsLocked);
        self.say(text, false);
    }

    // ---- export ---------------------------------------------------------------------------------------

    /// Write every live login to a new plain file in `format` at `path` (`~/` means the home folder), after checking the master
    /// password again. Returns how many logins were written and where.
    ///
    /// Refuses to replace a file that exists. The file is created readable by its owner only.
    pub fn export(
        &mut self,
        format: ExportFormat,
        password: &str,
        path: &str,
        now: i64,
    ) -> Result<(usize, std::path::PathBuf), String> {
        self.activity(now);
        let lang = self.lang();
        let result = self.try_export(format, password, path, now);
        match &result {
            Ok((count, file)) => {
                let text = trf(lang, Msg::ExportDone, &[count, &file.display()]);
                self.info(text, now);
            }
            Err(e) => self.error(e.clone(), now),
        }
        result
    }

    fn try_export(
        &mut self,
        format: ExportFormat,
        password: &str,
        path: &str,
        now: i64,
    ) -> Result<(usize, std::path::PathBuf), String> {
        let lang = self.lang();
        let path = path.trim();
        if path.is_empty() {
            return Err(t(lang, Msg::ExportEmptyPath).into());
        }
        let file = match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
            (Some(rest), Some(home)) => std::path::Path::new(&home).join(rest),
            _ => std::path::PathBuf::from(path),
        };
        let folder = file
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        if !folder.is_dir() {
            return Err(t(lang, Msg::ExportNoFolder).into());
        }
        if file.exists() {
            return Err(t(lang, Msg::ExportFileExists).into());
        }
        let vault = self
            .session
            .vault(now)
            .map_err(|Locked| t(lang, Msg::VaultIsLocked).to_string())?;
        // The vault is open, but exporting is what a person walking up to an unlocked screen would do: ask again.
        Self::check_password(vault, password, lang)?;
        let items: Vec<Item> = vault.items(Scope::Live).into_iter().cloned().collect();
        if items.is_empty() {
            return Err(t(lang, Msg::ExportNothing).into());
        }
        let text = format.render(&items)?;
        write_new_private_file(&file, text.as_bytes()).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                t(lang, Msg::ExportFileExists).to_string()
            } else {
                trf(lang, Msg::ExportFailed, &[&e])
            }
        })?;
        Ok((items.len(), file))
    }

    /// Checks the master password again against the open vault, for actions that must not be done by whoever
    /// walks up to an unlocked screen (export, deleting a profile).
    fn check_password(
        vault: &rahasyakosh_app::Vault<S>,
        password: &str,
        lang: Language,
    ) -> Result<(), String> {
        vault
            .store()
            .header()
            .map_err(|e| e.to_string())?
            .unlock_with(password, vault.secret_key())
            .map(drop)
            .map_err(|_| t(lang, Msg::WrongPassword).to_string())
    }

    /// Whether the open vault is also locked with a Secret Key.
    pub fn has_secret_key(&self) -> bool {
        self.session.peek().is_some_and(|v| v.has_secret_key())
    }

    /// Lock the open vault with a Secret Key as well, after checking the master password. Returns the key as text and
    /// whether it could be kept on this device, for the one-time sheet. Audit finding P1-3.
    pub fn add_secret_key(
        &mut self,
        password: &str,
        now: i64,
    ) -> Result<(zeroize::Zeroizing<String>, bool), String> {
        self.activity(now);
        let lang = self.lang();
        if self.has_secret_key() {
            return Err(t(lang, Msg::SecretKeyInUse).into());
        }
        let result = self
            .session
            .add_secret_key(password, now, &mut rand::rng())
            .map_err(|e| match e {
                rahasyakosh_app::Error::State(_) => e.to_string(),
                _ => t(lang, Msg::WrongPassword).to_string(),
            });
        if let Err(e) = &result {
            self.error(e.clone(), now);
        }
        result
    }

    /// For deleting a profile: is `password` the master password of the open vault?
    pub fn confirm_password(&mut self, password: &str, now: i64) -> Result<(), String> {
        let lang = self.lang();
        let vault = self
            .session
            .vault(now)
            .map_err(|Locked| t(lang, Msg::VaultIsLocked).to_string())?;
        Self::check_password(vault, password, lang)
    }

    // ---- screen reader started or stopped -------------------------------------------------------------

    /// What to show now that a screen reader started or stopped. `current` is the notice already on screen, if any:
    /// a second change only updates its wording and keeps what OK will do.
    pub fn screen_reader_changed(
        &self,
        enabled: bool,
        current: Option<ReaderNotice>,
    ) -> ReaderNotice {
        ReaderNotice {
            enabled,
            lock_first: current.map_or_else(|| self.is_unlocked(), |c| c.lock_first),
        }
    }

    /// The user pressed OK: lock the vault if the notice said it would, which returns to the unlock screen.
    pub fn acknowledge_reader_notice(&mut self, notice: ReaderNotice) {
        if notice.lock_first {
            self.lock();
        }
    }

    // ---- locking --------------------------------------------------------------------------------------

    /// What to show on the unlock screen before anything is typed (audit finding P1-2).
    pub fn unlock_hints(&self) -> rahasyakosh_app::UnlockHints {
        self.session.unlock_hints()
    }

    /// The costly-vault notice in the current language, if the vault needs one.
    pub fn costly_unlock_notice(&self) -> Option<String> {
        let k = self.unlock_hints().costly_kdf?;
        let d = KdfParams::DESKTOP;
        Some(trf(
            self.lang(),
            Msg::CostlyUnlock,
            &[&(k.m_kib / 1024), &k.t, &(d.m_kib / 1024), &d.t],
        ))
    }

    pub fn unlock(&mut self, password: &str, now: i64) {
        self.unlock_with_key(password, "", now);
    }

    /// Unlock, with a Secret Key the user typed (`""` when they typed none, so the one kept on this device is used).
    pub fn unlock_with_key(&mut self, password: &str, secret_key: &str, now: i64) {
        let lang = self.lang();
        let typed = secret_key.trim();
        if typed.is_empty() {
            self.session.provide_secret_key(None);
        } else {
            match rahasyakosh_core::secret_key::SecretKey::parse(typed) {
                Ok(k) => self.session.provide_secret_key(Some(k)),
                Err(_) => {
                    self.session.provide_secret_key(None);
                    let e = t(lang, Msg::SecretKeyInvalid).to_string();
                    self.unlock_error = Some(e.clone());
                    self.say(e, true);
                    return;
                }
            }
        }
        let result = self.session.unlock(password, now, &mut rand::rng());
        // The session keeps a typed key on the device when it worked; never hold it longer than this call.
        self.session.provide_secret_key(None);
        match result {
            Ok(u) => {
                self.unlock_error = None;
                if u.purged > 0 {
                    self.info(trf(lang, Msg::DeletedOld, &[&u.purged]), now);
                }
                if u.unreadable > 0 {
                    self.info(trf(lang, Msg::SkippedItems, &[&u.unreadable]), now);
                }
                if u.warnings > 0 {
                    self.info(trf(lang, Msg::IntegrityWarnings, &[&u.warnings]), now);
                }
            }
            // The unlock screen shows the live countdown, so the messages do not repeat the wait.
            Err(UnlockError::WrongPassword { .. }) => {
                self.unlock_error = Some(t(lang, Msg::WrongPassword).into())
            }
            Err(UnlockError::LockedOut { .. }) => {
                self.unlock_error = Some(t(lang, Msg::TooManyWrong).into());
            }
            Err(UnlockError::Failed(rahasyakosh_app::Error::Core(
                rahasyakosh_core::Error::SecretKeyRequired,
            ))) => self.unlock_error = Some(t(lang, Msg::SecretKeyNeeded).into()),
            Err(e) => self.unlock_error = Some(e.to_string()),
        }
        if let Some(e) = self.unlock_error.clone() {
            self.say(e, true);
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
        let lang = self.lang();
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
                label: label(lang, &f.name),
                secret: f.concealed,
                value: (!f.concealed || reveal(&f.name)).then(|| f.value.clone()),
            });
        }
        if !item.notes.is_empty() {
            fields.push(FieldView {
                name: "notes".into(),
                label: t(lang, Msg::Notes).into(),
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
                when: days_ago(lang, v.replaced_at, now),
                changed: changed_parts(lang, v, &newer),
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
            let text = trf(self.lang(), Msg::IsShown, &[&self.field_label(field)]);
            self.say(text, false);
        }
    }

    pub fn hide(&mut self, field: &str, now: i64) {
        self.activity(now);
        if let Some(id) = self.selected {
            self.session.hide(id, field);
            let text = trf(self.lang(), Msg::IsHidden, &[&self.field_label(field)]);
            self.say(text, false);
        }
    }

    /// The name of a field as the screen shows it.
    fn field_label(&self, name: &str) -> String {
        let lang = self.lang();
        if name == "notes" {
            t(lang, Msg::Notes).into()
        } else {
            label(lang, name)
        }
    }

    /// Show a hidden field, or hide it again if it is showing.
    pub fn toggle_reveal(&mut self, field: &str, now: i64) {
        if let Some(id) = self.selected {
            if self.session.is_revealed(id, field, now) {
                self.hide(field, now);
            } else {
                self.reveal(field, now);
            }
        }
    }

    /// Whether an item is open in the detail pane (so item shortcuts such as Edit make sense).
    pub fn has_open_item(&self) -> bool {
        self.view == View::Items && self.selected.is_some()
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
                    self.info(t(self.lang(), Msg::CopiedStays), now);
                } else {
                    self.info(trf(self.lang(), Msg::CopiedClears, &[&secs]), now);
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
        self.form_error = None;
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
        if let Some(problem) = form.problem(self.lang()) {
            let field_id = form.problem_field();
            self.form_error = Some(FormError {
                field_id,
                text: problem.clone(),
            });
            self.error(problem, now);
            return;
        }
        self.form_error = None;
        let mut rng = rand::rng();
        let lang = self.lang();
        let result: Result<Uuid, String> = (|| {
            let vault = self
                .session
                .vault(now)
                .map_err(|Locked| t(lang, Msg::VaultIsLocked).to_string())?;
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
                self.info(t(self.lang(), Msg::Saved), now);
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
            Err(Locked) => Err(t(self.lang(), Msg::VaultIsLocked).into()),
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
                self.info(t(self.lang(), Msg::MovedToTrashUndo), now);
                if let Some(n) = &mut self.notice {
                    n.undo = Some(id);
                    n.until = now + 8;
                }
            }
            Ok(Err(e)) => self.error(e.to_string(), now),
            Err(Locked) => self.forget_screen(),
        }
    }

    /// Bring back the item that was just moved to the trash (the "Undo" offered by that notice).
    pub fn undo_trash(&mut self, now: i64) {
        self.activity(now);
        let Some(id) = self.notice.as_ref().and_then(|n| n.undo) else {
            return;
        };
        match self
            .session
            .vault(now)
            .map(|v| v.restore(id, now, &mut rand::rng()))
        {
            Ok(Ok(())) => {
                self.selected = Some(id);
                self.info(t(self.lang(), Msg::Restored), now);
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
                self.info(t(self.lang(), Msg::Restored), now);
            }
            Ok(Err(e)) => self.error(e.to_string(), now),
            Err(Locked) => self.forget_screen(),
        }
    }

    pub fn purge_selected(&mut self, now: i64) {
        self.activity(now);
        let Some(id) = self.selected else { return };
        match self
            .session
            .vault(now)
            .map(|v| v.purge(id, &mut rand::rng()))
        {
            Ok(Ok(())) => {
                self.selected = None;
                self.info(t(self.lang(), Msg::DeletedForGood), now);
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
                self.info(t(self.lang(), Msg::WentBack), now);
            }
            Ok(Err(e)) => self.error(e.to_string(), now),
            Err(Locked) => self.forget_screen(),
        }
    }

    // ---- settings and the master password ---------------------------------------------------------------

    pub fn apply_settings(&mut self, settings: Settings, now: i64) {
        self.activity(now);
        self.session.set_settings(settings);
        let saved = match &mut self.save_settings {
            Some(save) => save(self.session.settings()),
            None => Ok(()),
        };
        match saved {
            Ok(()) => self.info(t(self.lang(), Msg::SettingsSaved), now),
            // They are in force until the app closes, but would be lost: say so.
            Err(e) => self.error(trf(self.lang(), Msg::SettingsNotSaved, &[&e]), now),
        }
    }

    /// Change the master password. Returns an error message if it is refused.
    pub fn change_master_password(
        &mut self,
        new: &str,
        again: &str,
        now: i64,
    ) -> Result<(), String> {
        self.activity(now);
        let lang = self.lang();
        if new != again {
            return Err(t(lang, Msg::ErrMismatch).into());
        }
        let s = rahasyakosh_strength::estimate(new, &[]);
        if s.score < rahasyakosh_strength::Strength::MIN_MASTER_SCORE {
            let advice = s
                .warning
                .clone()
                .unwrap_or_else(|| t(lang, Msg::ChooseLonger).into());
            return Err(trf(
                lang,
                Msg::ErrWeakMaster,
                &[&strength_label(lang, s.score), &advice],
            ));
        }
        let kdf = self.session.settings().kdf_params();
        let vault = self
            .session
            .vault(now)
            .map_err(|Locked| t(lang, Msg::VaultIsLocked).to_string())?;
        vault
            .change_password(new, kdf, &mut rand::rng())
            .map_err(|e| e.to_string())?;
        self.info(t(lang, Msg::MasterChanged), now);
        Ok(())
    }
}

/// The shown name of a field: the translated name for the usual ones, else the stored name with a capital.
fn label(lang: Language, name: &str) -> String {
    match name {
        "username" => return t(lang, Msg::FieldUsername).into(),
        "password" => return t(lang, Msg::FieldPassword).into(),
        _ => {}
    }
    let mut c = name.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// Like [`label`] but for a list of changed parts, which are written in lower case.
fn label_lower(lang: Language, name: &str) -> String {
    match name {
        "username" => t(lang, Msg::FieldUsername).to_lowercase(),
        "password" => t(lang, Msg::FieldPassword).to_lowercase(),
        _ => name.to_string(),
    }
}

/// "very weak" ... "very strong" for a password-strength score of 0 to 4.
pub fn strength_label(lang: Language, score: u8) -> &'static str {
    t(
        lang,
        match score {
            0 => Msg::Strength0,
            1 => Msg::Strength1,
            2 => Msg::Strength2,
            3 => Msg::Strength3,
            _ => Msg::Strength4,
        },
    )
}

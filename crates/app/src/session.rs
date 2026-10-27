//! The session: when a vault is locked or unlocked, and what that implies (US-VLT-04, US-SEC-01..04).
//!
//! A [`Session`] owns a store. While **locked** it holds only the store (no keys, no items). While **unlocked** it
//! also holds a [`Vault`]. It enforces, for any front-end:
//!
//! - **auto-lock** after a period of inactivity, and manual lock; locking drops the vault, which wipes keys, items and
//!   the search index, and forgets every revealed secret;
//! - **brute-force friction** on the unlock screen: after three wrong passwords each further attempt must wait
//!   longer (1 s, 2 s, 4 s … up to 5 minutes). This slows someone at your keyboard; it is not a defence against an
//!   attacker who copies the vault file, which is what the slow key derivation is for;
//! - **secrets hidden by default**: a field is shown only after [`Session::reveal`], and hides again after a timeout;
//! - **copied secrets are cleared**: [`Session::copied`] remembers what was put on the clipboard so the front-end can
//!   clear it after the configured delay, but only if it is still there.
//!
//! Like the rest of this crate it never reads the clock: every method that depends on time takes `now` in seconds.
use crate::vault::{Config, Error, Result, Vault};
use rahasyakosh_core::crypto::KdfParams;
use rahasyakosh_core::item::DEFAULT_HISTORY_LIMIT;
use rahasyakosh_store::{Store, Uuid};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// How a front-end draws its top-bar buttons (icon and text, text only, or icon only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolbarStyle {
    #[default]
    IconAndText,
    TextOnly,
    IconOnly,
}

/// Which colour scheme a front-end uses: always light, always dark, or whatever the system is set to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

/// The language a front-end shows its text in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[default]
    English,
    Hindi,
    Sanskrit,
    Tamil,
    Telugu,
    Malayalam,
    Urdu,
}

impl Language {
    pub const ALL: [Language; 7] = [
        Self::English,
        Self::Hindi,
        Self::Sanskrit,
        Self::Tamil,
        Self::Telugu,
        Self::Malayalam,
        Self::Urdu,
    ];

    /// Position in [`Self::ALL`], used to index translation tables.
    pub fn index(self) -> usize {
        self as usize
    }

    /// BCP 47 language code, for the `lang` attribute.
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Hindi => "hi",
            Self::Sanskrit => "sa",
            Self::Tamil => "ta",
            Self::Telugu => "te",
            Self::Malayalam => "ml",
            Self::Urdu => "ur",
        }
    }

    /// The language's own name for itself, so it can be found whatever language is showing.
    pub fn native_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Hindi => "हिन्दी",
            Self::Sanskrit => "संस्कृतम्",
            Self::Tamil => "தமிழ்",
            Self::Telugu => "తెలుగు",
            Self::Malayalam => "മലയാളം",
            Self::Urdu => "اردو",
        }
    }

    /// Whether the language is written right to left.
    pub fn is_rtl(self) -> bool {
        self == Self::Urdu
    }
}

/// User-adjustable behaviour (US-SET-01). Contains no secrets, so a front-end may keep it in a plain file.
/// Missing fields take their defaults, so older settings files keep working when options are added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Lock after this many idle seconds; 0 = never.
    pub idle_lock_secs: u32,
    /// Clear a copied secret from the clipboard after this many seconds; 0 = never.
    pub clipboard_clear_secs: u32,
    /// Hide a revealed secret again after this many seconds; 0 = until hidden by hand.
    pub reveal_secs: u32,
    /// Lock as soon as the window loses focus.
    pub lock_on_blur: bool,
    /// Days an item stays in the trash (1 to 3650).
    pub trash_retention_days: u32,
    /// Earlier versions kept per item.
    pub history_limit: usize,
    /// Memory the key derivation uses, in MiB, for vaults created or re-keyed from now on (US-SEC-04).
    pub kdf_memory_mib: u32,
    /// Passes of the key derivation.
    pub kdf_passes: u32,
    /// How the desktop top-bar buttons are drawn.
    pub toolbar_style: ToolbarStyle,
    /// Light, dark, or follow the system.
    pub theme: ThemeMode,
    /// The language the text is shown in.
    pub language: Language,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            idle_lock_secs: 300,
            clipboard_clear_secs: 20,
            reveal_secs: 10,
            lock_on_blur: false,
            trash_retention_days: 30,
            history_limit: DEFAULT_HISTORY_LIMIT,
            kdf_memory_mib: KdfParams::DESKTOP.m_kib / 1024,
            kdf_passes: KdfParams::DESKTOP.t,
            toolbar_style: ToolbarStyle::default(),
            theme: ThemeMode::default(),
            language: Language::default(),
        }
    }
}

impl Settings {
    pub const MAX_IDLE_SECS: u32 = 24 * 3600;
    pub const MAX_CLIPBOARD_SECS: u32 = 3600;
    pub const MAX_REVEAL_SECS: u32 = 3600;
    /// Never 0: an item must always get at least a day in the trash, so a bad or forged settings file cannot make the
    /// next unlock empty the trash for good (audit finding P3-1).
    pub const MIN_RETENTION_DAYS: u32 = 1;
    pub const MAX_RETENTION_DAYS: u32 = 3650;
    pub const MAX_HISTORY: usize = 100;

    /// The same settings with every value brought into its allowed range. Key-derivation costs never go below the
    /// library's floor, so a settings file cannot weaken a vault.
    pub fn validated(mut self) -> Self {
        self.idle_lock_secs = self.idle_lock_secs.min(Self::MAX_IDLE_SECS);
        self.clipboard_clear_secs = self.clipboard_clear_secs.min(Self::MAX_CLIPBOARD_SECS);
        self.reveal_secs = self.reveal_secs.min(Self::MAX_REVEAL_SECS);
        self.trash_retention_days = self
            .trash_retention_days
            .clamp(Self::MIN_RETENTION_DAYS, Self::MAX_RETENTION_DAYS);
        self.history_limit = self.history_limit.min(Self::MAX_HISTORY);
        self.kdf_memory_mib = self.kdf_memory_mib.clamp(
            KdfParams::MIN_M_KIB.div_ceil(1024),
            KdfParams::MAX_M_KIB / 1024,
        );
        self.kdf_passes = self.kdf_passes.clamp(KdfParams::MIN_T, KdfParams::MAX_T);
        self
    }

    /// Key-derivation settings for a vault created or re-keyed now.
    pub fn kdf_params(&self) -> KdfParams {
        let s = self.clone().validated();
        KdfParams {
            m_kib: s.kdf_memory_mib * 1024,
            t: s.kdf_passes,
            p: KdfParams::DESKTOP.p,
        }
    }

    pub fn vault_config(&self) -> Config {
        let s = self.clone().validated();
        Config {
            history_limit: s.history_limit,
            trash_retention_days: s.trash_retention_days,
        }
    }

    /// Parse a settings file; anything unreadable gives the defaults rather than an error, and values are clamped.
    pub fn from_json(bytes: &[u8]) -> Self {
        Self::parse(bytes).unwrap_or_default()
    }

    /// Parse a settings file, clamping the values; `None` if it is not readable settings (audit finding P3-2), so
    /// a front-end can tell the user instead of quietly using the defaults.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice::<Self>(bytes)
            .ok()
            .map(Self::validated)
    }

    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(&self.clone().validated()).expect("settings serialise")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UnlockError {
    #[error("the vault is already unlocked")]
    AlreadyUnlocked,
    /// Too many wrong passwords in a row: try again later. The password was not checked.
    #[error("too many wrong passwords; try again in {retry_in_secs} s")]
    LockedOut { retry_in_secs: i64 },
    #[error("wrong password ({failures} in a row); next attempt allowed in {retry_in_secs} s")]
    WrongPassword { failures: u32, retry_in_secs: i64 },
    #[error(transparent)]
    Failed(#[from] Error),
}

/// The vault is locked (or has just locked itself because of inactivity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the vault is locked")]
pub struct Locked;

/// What a front-end should tell the user on the unlock screen, before they type anything.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnlockHints {
    /// The vault's file asks for a clearly more expensive key derivation than this program's default (audit finding
    /// P1-2). The header is read before it is authenticated, so a forged one can ask for this; unlocking will be slow.
    pub costly_kdf: Option<rahasyakosh_core::crypto::KdfParams>,
    /// The vault needs a Secret Key and this device has none saved, so the user has to type it (audit finding P1-3).
    pub needs_secret_key: bool,
}

/// What a successful unlock did besides opening the vault.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unlocked {
    /// Items deleted because they had been in the trash too long.
    pub purged: usize,
    /// Stored items that could not be opened and were left out (see [`Vault::unreadable`]). Tell the user.
    pub unreadable: usize,
    /// Things the vault's sealed manifest says are wrong (see [`Vault::warnings`]): a rolled-back, missing or
    /// forged item. Tell the user.
    pub warnings: usize,
}

struct Active<S: Store> {
    vault: Vault<S>,
    last_activity: i64,
}

enum State<S: Store> {
    Locked(S),
    Unlocked(Box<Active<S>>),
    /// Only while a transition is in progress; never observable.
    Moving,
}

/// A secret copied to the clipboard that should be cleared again.
struct PendingClear {
    value: Zeroizing<String>,
    due: i64,
}

/// Wait before the next unlock attempt allowed after `failures` wrong passwords in a row.
pub fn backoff_secs(failures: u32) -> i64 {
    if failures < 3 {
        0
    } else {
        (1i64 << (failures - 3).min(20)).min(300)
    }
}

pub struct Session<S: Store> {
    state: State<S>,
    settings: Settings,
    failures: u32,
    next_attempt_at: i64,
    revealed: Vec<(Uuid, String, i64)>,
    pending_clear: Option<PendingClear>,
    /// Where this device remembers the newest write it has seen of each vault (see `HighWater`).
    #[cfg(not(target_family = "wasm"))]
    highwater_path: Option<std::path::PathBuf>,
    /// Where this device keeps the Secret Keys of its vaults.
    #[cfg(not(target_family = "wasm"))]
    secret_keys: Option<crate::SecretKeyDir>,
    /// A Secret Key the user typed in, used for the next unlock.
    secret_key: Option<rahasyakosh_core::secret_key::SecretKey>,
}

impl<S: Store> Session<S> {
    /// Look for vault Secret Keys in `dir` (kept apart from the vault file). See [`crate::SecretKeyDir`].
    #[cfg(not(target_family = "wasm"))]
    #[must_use]
    pub fn with_secret_keys(mut self, dir: crate::SecretKeyDir) -> Self {
        self.secret_keys = Some(dir);
        self
    }

    /// Lock the open vault with a new Secret Key as well, and keep the key on this device. Returns the key as text
    /// (to show the user once) and whether it could be kept on the device; when it could not, the user has to type
    /// it at every unlock. The master password is checked first. Audit finding P1-3.
    pub fn add_secret_key(
        &mut self,
        password: &str,
        now: i64,
        rng: &mut impl rand_core::CryptoRng,
    ) -> std::result::Result<(Zeroizing<String>, bool), Error> {
        let vault = self
            .vault(now)
            .map_err(|_| Error::State("the vault is locked"))?;
        let key = vault.set_secret_key(password, rng)?;
        #[cfg(not(target_family = "wasm"))]
        let kept = self
            .secret_keys
            .as_ref()
            .zip(self.peek().map(Vault::vault_id))
            .is_some_and(|(dir, id)| dir.save(id, &key).is_ok());
        #[cfg(target_family = "wasm")]
        let kept = false;
        Ok((key.to_text(), kept))
    }

    /// What to tell the user about the locked vault before they unlock it. Empty when the vault is open.
    pub fn unlock_hints(&self) -> UnlockHints {
        let State::Locked(store) = &self.state else {
            return UnlockHints::default();
        };
        let Ok(header) = store.header() else {
            return UnlockHints::default();
        };
        #[cfg(not(target_family = "wasm"))]
        let on_device = self
            .secret_keys
            .as_ref()
            .is_some_and(|d| d.load(header.vault_id).is_some());
        #[cfg(target_family = "wasm")]
        let on_device = false;
        UnlockHints {
            costly_kdf: header.kdf.is_unusually_costly().then_some(header.kdf),
            needs_secret_key: header.secret_key && self.secret_key.is_none() && !on_device,
        }
    }

    /// Use this Secret Key (typed by the user) for the next unlock, in preference to the one kept on the device.
    pub fn provide_secret_key(&mut self, key: Option<rahasyakosh_core::secret_key::SecretKey>) {
        self.secret_key = key;
    }

    /// Remember, in the file at `path` (outside the vault folder), the newest write seen of each vault, and warn
    /// at unlock when a vault is older than that. Audit finding P2-1.
    #[cfg(not(target_family = "wasm"))]
    #[must_use]
    pub fn with_highwater(mut self, path: std::path::PathBuf) -> Self {
        self.highwater_path = Some(path);
        self
    }

    /// A locked session over an existing vault.
    pub fn new(store: S, settings: Settings) -> Self {
        Self {
            state: State::Locked(store),
            settings: settings.validated(),
            failures: 0,
            next_attempt_at: 0,
            revealed: Vec::new(),
            pending_clear: None,
            #[cfg(not(target_family = "wasm"))]
            highwater_path: None,
            #[cfg(not(target_family = "wasm"))]
            secret_keys: None,
            secret_key: None,
        }
    }

    /// Create a new vault in an empty `store` and start with it unlocked.
    pub fn create(
        store: S,
        password: &str,
        settings: Settings,
        now: i64,
        rng: &mut impl rand_core::CryptoRng,
    ) -> Result<Self> {
        let settings = settings.validated();
        let vault = Vault::create(
            store,
            password,
            settings.kdf_params(),
            settings.vault_config(),
            rng,
        )?;
        Ok(Self {
            state: State::Unlocked(Box::new(Active {
                vault,
                last_activity: now,
            })),
            settings,
            failures: 0,
            next_attempt_at: 0,
            revealed: Vec::new(),
            pending_clear: None,
            #[cfg(not(target_family = "wasm"))]
            highwater_path: None,
            #[cfg(not(target_family = "wasm"))]
            secret_keys: None,
            secret_key: None,
        })
    }

    pub fn is_unlocked(&self) -> bool {
        matches!(self.state, State::Unlocked(_))
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Change settings (values are clamped). Applies to the open vault immediately.
    pub fn set_settings(&mut self, settings: Settings) {
        self.settings = settings.validated();
        if let State::Unlocked(a) = &mut self.state {
            a.vault.set_config(self.settings.vault_config());
        }
    }

    pub fn failed_attempts(&self) -> u32 {
        self.failures
    }

    /// Seconds until another unlock attempt is allowed (0 = now).
    pub fn retry_in(&self, now: i64) -> i64 {
        (self.next_attempt_at - now).max(0)
    }

    // ---- locking ------------------------------------------------------------------------------------

    /// Try to unlock with `password`.
    pub fn unlock(
        &mut self,
        password: &str,
        now: i64,
        rng: &mut impl rand_core::CryptoRng,
    ) -> std::result::Result<Unlocked, UnlockError> {
        if self.is_unlocked() {
            return Err(UnlockError::AlreadyUnlocked);
        }
        let wait = self.retry_in(now);
        if wait > 0 {
            return Err(UnlockError::LockedOut {
                retry_in_secs: wait,
            });
        }
        let State::Locked(store) = std::mem::replace(&mut self.state, State::Moving) else {
            unreachable!("checked above");
        };
        #[cfg(not(target_family = "wasm"))]
        let mut highwater = self.highwater_path.as_ref().map(crate::HighWater::at);
        #[cfg(not(target_family = "wasm"))]
        let last_seen = highwater.as_ref().and_then(|h| {
            let id = store.header().ok()?.vault_id;
            h.last_seen(id)
        });
        #[cfg(target_family = "wasm")]
        let last_seen = None;
        let secret_key = self.secret_key.clone().or_else(|| {
            #[cfg(not(target_family = "wasm"))]
            {
                let header = store.header().ok().filter(|h| h.secret_key)?;
                self.secret_keys.as_ref()?.load(header.vault_id)
            }
            #[cfg(target_family = "wasm")]
            None
        });
        match Vault::try_unlock_full(
            store,
            password,
            secret_key.as_ref(),
            self.settings.vault_config(),
            last_seen,
        ) {
            Ok(mut vault) => {
                #[cfg(not(target_family = "wasm"))]
                if let Some(h) = highwater.take() {
                    vault.track(h);
                }
                let purged = match vault.purge_expired(now, rng) {
                    Ok(n) => n,
                    Err(e) => {
                        self.state = State::Locked(vault.into_store());
                        return Err(e.into());
                    }
                };
                self.failures = 0;
                self.next_attempt_at = 0;
                // A key the user typed worked: keep it on this device so they are not asked again.
                #[cfg(not(target_family = "wasm"))]
                if let (Some(k), Some(dir), Ok(h)) =
                    (&self.secret_key, &self.secret_keys, vault.store().header())
                {
                    let _ = dir.save(h.vault_id, k);
                }
                self.secret_key = None;
                let unreadable = vault.unreadable().len();
                let warnings = vault.warnings().len();
                self.state = State::Unlocked(Box::new(Active {
                    vault,
                    last_activity: now,
                }));
                Ok(Unlocked {
                    purged,
                    unreadable,
                    warnings,
                })
            }
            Err(failed) => {
                self.state = State::Locked(failed.store);
                match failed.error {
                    Error::Core(rahasyakosh_core::Error::Decrypt) => {
                        self.failures += 1;
                        let wait = backoff_secs(self.failures);
                        self.next_attempt_at = now + wait;
                        Err(UnlockError::WrongPassword {
                            failures: self.failures,
                            retry_in_secs: wait,
                        })
                    }
                    other => Err(other.into()),
                }
            }
        }
    }

    /// Lock now. Keys, decrypted items, the search index and every revealed secret are dropped. Returns the secret
    /// still waiting to be cleared from the clipboard, if any, so the caller can clear it straight away.
    pub fn lock(&mut self) -> Option<Zeroizing<String>> {
        if let State::Unlocked(_) = self.state {
            let State::Unlocked(active) = std::mem::replace(&mut self.state, State::Moving) else {
                unreachable!()
            };
            self.state = State::Locked(active.vault.into_store());
        }
        self.revealed.clear();
        self.pending_clear.take().map(|p| p.value)
    }

    /// Apply the inactivity timeout and expire revealed secrets. Call this about once a second.
    /// Returns the clipboard value to clear if the session just locked itself.
    pub fn tick(&mut self, now: i64) -> Tick {
        let mut locked = false;
        let mut clear_now = None;
        if let State::Unlocked(a) = &self.state {
            let idle = self.settings.idle_lock_secs;
            if idle > 0 && now - a.last_activity >= i64::from(idle) {
                clear_now = self.lock();
                locked = true;
            }
        }
        self.revealed
            .retain(|(_, _, until)| *until == 0 || now < *until);
        Tick { locked, clear_now }
    }

    /// Record user activity (a click, a key press), postponing the inactivity lock.
    pub fn touch(&mut self, now: i64) {
        if let State::Unlocked(a) = &mut self.state {
            a.last_activity = now;
        }
    }

    /// Seconds left before the inactivity lock, or `None` if locked or the timeout is off.
    pub fn seconds_until_lock(&self, now: i64) -> Option<i64> {
        match &self.state {
            State::Unlocked(a) if self.settings.idle_lock_secs > 0 => {
                Some((a.last_activity + i64::from(self.settings.idle_lock_secs) - now).max(0))
            }
            _ => None,
        }
    }

    // ---- the vault -----------------------------------------------------------------------------------

    /// Read-only access that does **not** count as activity (rendering a screen is not a user action).
    pub fn peek(&self) -> Option<&Vault<S>> {
        match &self.state {
            State::Unlocked(a) => Some(&a.vault),
            _ => None,
        }
    }

    /// Access for a user action: counts as activity. If the inactivity timeout has already passed, the session locks instead.
    pub fn vault(&mut self, now: i64) -> std::result::Result<&mut Vault<S>, Locked> {
        if self.tick(now).locked {
            return Err(Locked);
        }
        match &mut self.state {
            State::Unlocked(a) => {
                a.last_activity = now;
                Ok(&mut a.vault)
            }
            _ => Err(Locked),
        }
    }

    // ---- secrets on screen and on the clipboard -------------------------------------------------------

    /// Show a secret field. It hides again after `reveal_secs` (or when [`hide`](Self::hide) / lock is called).
    pub fn reveal(&mut self, item: Uuid, field: &str, now: i64) -> std::result::Result<(), Locked> {
        if !self.is_unlocked() {
            return Err(Locked);
        }
        self.revealed
            .retain(|(i, f, _)| !(*i == item && f == field));
        let until = if self.settings.reveal_secs == 0 {
            0
        } else {
            now + i64::from(self.settings.reveal_secs)
        };
        self.revealed.push((item, field.to_string(), until));
        Ok(())
    }

    pub fn hide(&mut self, item: Uuid, field: &str) {
        self.revealed
            .retain(|(i, f, _)| !(*i == item && f == field));
    }

    pub fn hide_all(&mut self) {
        self.revealed.clear();
    }

    pub fn is_revealed(&self, item: Uuid, field: &str, now: i64) -> bool {
        self.revealed
            .iter()
            .any(|(i, f, until)| *i == item && f == field && (*until == 0 || now < *until))
    }

    /// Note that `value` was just put on the clipboard. Returns when it should be cleared (`None` = never).
    pub fn copied(&mut self, value: &str, now: i64) -> Option<i64> {
        if self.settings.clipboard_clear_secs == 0 {
            self.pending_clear = None;
            return None;
        }
        let due = now + i64::from(self.settings.clipboard_clear_secs);
        self.pending_clear = Some(PendingClear {
            value: Zeroizing::new(value.to_string()),
            due,
        });
        Some(due)
    }

    /// The copied secret, once its time is up. The caller clears the clipboard only if it still holds exactly this
    /// value (see [`should_clear`]). Returns each secret once.
    pub fn clipboard_due(&mut self, now: i64) -> Option<Zeroizing<String>> {
        if self.pending_clear.as_ref().is_some_and(|p| now >= p.due) {
            self.pending_clear.take().map(|p| p.value)
        } else {
            None
        }
    }

    pub fn has_pending_clipboard(&self) -> bool {
        self.pending_clear.is_some()
    }
}

/// What [`Session::tick`] did.
#[derive(Debug, PartialEq, Eq)]
pub struct Tick {
    /// The session locked itself because of inactivity.
    pub locked: bool,
    /// A copied secret that should be cleared from the clipboard right now (because of that lock).
    pub clear_now: Option<Zeroizing<String>>,
}

/// Clear the clipboard only if it still holds the secret we put there; never destroy something else the user copied.
pub fn should_clear(pending: &str, current: Option<&str>) -> bool {
    current == Some(pending)
}

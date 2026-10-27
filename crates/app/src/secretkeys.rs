//! Where this device keeps the Secret Keys of its vaults (audit finding P1-3).
//!
//! A Secret Key is kept **apart from the vault file**: in a private file in the user's data folder, one per vault id,
//! readable by the owner only. Copying the vault folder, or syncing it, does not copy the key. The user also keeps a
//! printed recovery sheet for a new device.
use rahasyakosh_core::secret_key::SecretKey;
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SecretKeyDir {
    dir: PathBuf,
}

impl SecretKeyDir {
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// `$XDG_DATA_HOME/rahasyakosh/secret-keys`, else `~/.local/share/rahasyakosh/secret-keys`.
    pub fn default_dir() -> Option<Self> {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME").map(|h| Path::new(&h).join(".local").join("share"))
            })?;
        Some(Self::at(base.join("rahasyakosh").join("secret-keys")))
    }

    fn file(&self, vault: Uuid) -> PathBuf {
        self.dir.join(format!("{vault}.key"))
    }

    /// The key kept for `vault`, if there is a readable, valid one.
    pub fn load(&self, vault: Uuid) -> Option<SecretKey> {
        let text = zeroize::Zeroizing::new(std::fs::read_to_string(self.file(vault)).ok()?);
        SecretKey::parse(&text).ok()
    }

    pub fn save(&self, vault: Uuid, key: &SecretKey) -> std::io::Result<()> {
        let mut dir = std::fs::DirBuilder::new();
        dir.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut dir, 0o700);
        dir.create(&self.dir)?;
        let path = self.file(vault);
        let tmp = path.with_extension("key.tmp");
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
        let mut f = opts.open(&tmp)?;
        f.write_all(key.to_text().as_bytes())?;
        f.sync_all()?;
        std::fs::rename(&tmp, &path)
    }

    pub fn remove(&self, vault: Uuid) -> std::io::Result<()> {
        match std::fs::remove_file(self.file(vault)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

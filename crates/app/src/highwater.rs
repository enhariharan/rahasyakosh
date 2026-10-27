//! What this device has seen of each vault, kept outside the vault folder (audit finding P2-1).
//!
//! The sealed manifest catches a removed or swapped record. It cannot catch the whole vault, manifest included,
//! being replaced by an older copy, because an older copy is perfectly genuine. Only a device that remembers
//! "I have seen write number N" can tell. So each device keeps the highest write number it has seen per vault in
//! a small private file that is **not** in the synced folder, and warns when a vault shows a lower one.
//!
//! The file holds only vault ids and counters. Losing it loses the protection, not any data.
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct HighWater {
    path: PathBuf,
    seen: BTreeMap<Uuid, u64>,
}

impl HighWater {
    /// Read the file at `path`. A missing or damaged file starts empty: this is a safety net, not a gate.
    pub fn at(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let seen = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<BTreeMap<Uuid, u64>>(&b).ok())
            .unwrap_or_default();
        Self { path, seen }
    }

    /// Where the file lives by default: `$XDG_STATE_HOME/rahasyakosh/seen.json`, else `~/.local/state/...`.
    pub fn default_path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME").map(|h| Path::new(&h).join(".local").join("state"))
            })?;
        Some(base.join("rahasyakosh").join("seen.json"))
    }

    pub fn last_seen(&self, vault: Uuid) -> Option<u64> {
        self.seen.get(&vault).copied()
    }

    /// Remember `seq` for `vault` if it is higher than what was known. Best effort: a failure to save is returned
    /// but never stops the vault working.
    pub fn record(&mut self, vault: Uuid, seq: u64) -> std::io::Result<()> {
        let e = self.seen.entry(vault).or_insert(0);
        if seq <= *e {
            return Ok(());
        }
        *e = seq;
        self.save()
    }

    fn save(&self) -> std::io::Result<()> {
        let dir = self.path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir)?;
        let tmp = self.path.with_extension("json.tmp");
        let bytes = serde_json::to_vec(&self.seen).map_err(std::io::Error::other)?;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
        let mut f = opts.open(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
        std::fs::rename(&tmp, &self.path)
    }
}

//! On-disk single-file vault: `{ header, records[] }` as JSON. Replaced by `rahasyakosh-store` in M2.
use anyhow::{Context, Result};
use rahasyakosh_core::vault::{Record, VaultHeader};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};

#[derive(Serialize, Deserialize)]
pub struct VaultFile {
    pub header: VaultHeader,
    pub records: Vec<Record>,
}

impl VaultFile {
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)?;
        serde_json::from_slice(&bytes).context("vault file is corrupted")
    }

    /// Atomic write (temp file + fsync + rename), mode 0600 on Unix.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("rkv.tmp");
        {
            let mut opts = fs::OpenOptions::new();
            opts.write(true).create(true).truncate(true);
            #[cfg(unix)]
            std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
            let mut f = opts.open(&tmp)?;
            f.write_all(&serde_json::to_vec_pretty(self)?)?;
            f.sync_all()?;
        }
        fs::rename(&tmp, path)?;
        Ok(())
    }
}

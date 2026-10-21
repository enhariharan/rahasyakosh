//! Reading and writing the portable vault bundle (a JSON file) on disk.
use anyhow::{Context, Result};
use rahasyakosh_transfer::Bundle;
use std::{fs, io::Write, path::Path};

pub fn load(path: &Path) -> Result<Bundle> {
    let bytes = fs::read(path)?;
    Bundle::from_slice(&bytes).context("vault file is corrupted")
}

/// Atomic write (temp file + fsync + rename), mode 0600 on Unix.
pub fn save(bundle: &Bundle, path: &Path) -> Result<()> {
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
        f.write_all(&bundle.to_vec()?)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

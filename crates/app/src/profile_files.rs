//! Where the profile list and the profiles' vault files live on disk (E13). The rules about names and usernames
//! are in [`ProfileList`]; this file only reads and writes, and is shared by the desktop app and the `rk` command.
use crate::profiles::{Profile, ProfileList};
use rahasyakosh_store::private_fs;
use std::path::{Component, Path, PathBuf};

/// The file holding the profile list, inside the data folder.
pub fn list_path(dir: &Path) -> PathBuf {
    dir.join("profiles.json")
}

/// The vault file of `profile`. `None` if the list names a path that leaves the data folder.
pub fn vault_file(dir: &Path, profile: &Profile) -> Option<PathBuf> {
    let rel = Path::new(&profile.file);
    rel.components()
        .all(|c| matches!(c, Component::Normal(_)))
        .then(|| dir.join(rel))
}

/// Reads the saved profile list. `Ok(None)` if there is none yet; an error if it exists but cannot be read.
pub fn read_list(dir: &Path) -> Result<Option<ProfileList>, String> {
    match std::fs::read(list_path(dir)) {
        Ok(bytes) => ProfileList::from_json(&bytes)
            .map(Some)
            .ok_or_else(|| format!("{} is damaged.", list_path(dir).display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Cannot read {}: {e}", list_path(dir).display())),
    }
}

/// Writes the list through a temporary file so a crash cannot leave half of it.
pub fn save_list(dir: &Path, list: &ProfileList) -> Result<(), String> {
    let write = || -> std::io::Result<()> {
        private_fs::create_private_dir(dir)?;
        private_fs::write_private_atomic(&list_path(dir), &list.to_json())
    };
    write().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rk-appprofiles-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_missing_list_is_none_a_saved_one_reads_back_and_a_bad_one_is_an_error() {
        let dir = scratch("read");
        assert_eq!(read_list(&dir), Ok(None));
        let mut list = ProfileList::new();
        list.add(Uuid::from_u128(1), "A", "a", "v.db").unwrap();
        save_list(&dir, &list).unwrap();
        assert_eq!(read_list(&dir), Ok(Some(list)));
        std::fs::write(list_path(&dir), b"garbage").unwrap();
        assert!(read_list(&dir).unwrap_err().contains("damaged"));
        assert_eq!(std::fs::read(list_path(&dir)).unwrap(), b"garbage");
    }

    #[test]
    fn a_vault_path_cannot_leave_the_data_folder() {
        let dir = scratch("escape");
        let mut list = ProfileList::new();
        for (i, file) in ["../x.db", "/etc/x.db", "profiles/a.db"]
            .into_iter()
            .enumerate()
        {
            list.add(Uuid::from_u128(i as u128 + 1), "N", &format!("u{i}"), file)
                .unwrap();
        }
        let got: Vec<_> = list.all().iter().map(|p| vault_file(&dir, p)).collect();
        assert_eq!(got, [None, None, Some(dir.join("profiles/a.db"))]);
    }
}

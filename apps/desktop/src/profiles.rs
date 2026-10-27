//! Where the profile list and the profiles' vault files live on disk (E13). The rules about names and usernames
//! are in `rahasyakosh_app::ProfileList`; this file only reads and writes.
use rahasyakosh_app::{new_profile_id, Profile, ProfileList, DEFAULT_FULL_NAME, DEFAULT_USERNAME};
use rahasyakosh_store::{private_fs, Uuid};
use std::path::{Component, Path, PathBuf};

fn list_path(dir: &Path) -> PathBuf {
    dir.join("profiles.json")
}

/// The vault file of `profile`. `None` if the list names a path that leaves the data folder.
pub fn vault_file(dir: &Path, profile: &Profile) -> Option<PathBuf> {
    let rel = Path::new(&profile.file);
    rel.components()
        .all(|c| matches!(c, Component::Normal(_)))
        .then(|| dir.join(rel))
}

/// The vault file name for a new profile (the first one keeps the old single-vault name).
pub fn new_vault_file(first: Option<&str>, id: Uuid) -> String {
    first.map_or_else(|| format!("profiles/{id}.db"), str::to_string)
}

/// Reads the profile list. When there is none yet but a vault file already exists (made before profiles
/// existed), that vault becomes the default profile, in place: same file, same master password.
pub fn load(dir: &Path, legacy_vault: &Path) -> Result<ProfileList, String> {
    match std::fs::read(list_path(dir)) {
        Ok(bytes) => ProfileList::from_json(&bytes)
            .ok_or_else(|| format!("{} is damaged.", list_path(dir).display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let has_vault = std::fs::metadata(legacy_vault).is_ok_and(|m| m.len() > 0);
            let mut list = ProfileList::new();
            if has_vault {
                let file = legacy_vault
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("vault.db");
                let id = new_profile_id(&mut rand::rng());
                list.add(id, DEFAULT_FULL_NAME, DEFAULT_USERNAME, file)
                    .map_err(|e| e.to_string())?;
                save(dir, &list)?;
            }
            Ok(list)
        }
        Err(e) => Err(format!("Cannot read {}: {e}", list_path(dir).display())),
    }
}

/// Writes the list through a temporary file so a crash cannot leave half of it.
pub fn save(dir: &Path, list: &ProfileList) -> Result<(), String> {
    let write = || -> std::io::Result<()> {
        private_fs::create_private_dir(dir)?;
        private_fs::write_private_atomic(&list_path(dir), &list.to_json())
    };
    write().map_err(|e| e.to_string())
}

/// Makes sure the folder for a new vault file exists.
pub fn prepare_folder(vault: &Path) {
    if let Some(parent) = vault.parent() {
        let _ = private_fs::create_private_dir(parent);
    }
}

/// Deletes a vault file and the SQLite side files next to it.
pub fn delete_vault(vault: &Path) {
    let name = vault.as_os_str().to_owned();
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut p = name.clone();
        p.push(suffix);
        let _ = std::fs::remove_file(PathBuf::from(p));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rk-profiles-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn no_vault_and_no_list_means_first_run() {
        let dir = scratch("first");
        assert!(load(&dir, &dir.join("vault.db")).unwrap().is_empty());
        assert!(!list_path(&dir).exists());
    }

    #[test]
    fn an_existing_vault_becomes_the_default_profile_in_place() {
        let dir = scratch("legacy");
        let vault = dir.join("vault.db");
        std::fs::write(&vault, b"SQLite format 3\0 pretend").unwrap();
        let list = load(&dir, &vault).unwrap();
        assert_eq!(list.len(), 1);
        let p = &list.all()[0];
        assert_eq!(
            (p.full_name.as_str(), p.username.as_str()),
            ("Default user", "defaultuser")
        );
        assert_eq!(vault_file(&dir, p), Some(vault.clone()));
        assert_eq!(std::fs::read(&vault).unwrap(), b"SQLite format 3\0 pretend");
        // Read back from the saved list: the same ID, not a new one.
        assert_eq!(load(&dir, &vault).unwrap(), list);
    }

    #[test]
    fn a_damaged_list_is_reported_not_replaced() {
        let dir = scratch("damaged");
        std::fs::write(list_path(&dir), b"garbage").unwrap();
        assert!(load(&dir, &dir.join("vault.db"))
            .unwrap_err()
            .contains("damaged"));
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

    #[cfg(unix)]
    #[test]
    fn the_list_and_the_folders_made_for_it_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        let root = scratch("modes");
        let data = root.join("share/rahasyakosh");
        save(&data, &ProfileList::new()).unwrap();
        prepare_folder(&data.join("profiles/a.db"));
        assert_eq!(mode(&data), 0o700);
        assert_eq!(mode(&data.join("profiles")), 0o700);
        assert_eq!(mode(&list_path(&data)), 0o600);
        let leftovers: Vec<_> = std::fs::read_dir(&data).unwrap().collect();
        assert_eq!(leftovers.len(), 2, "the list and profiles/: {leftovers:?}");
    }

    #[test]
    fn deleting_a_vault_removes_its_side_files() {
        let dir = scratch("delete");
        let v = dir.join("a.db");
        for s in ["", "-wal", "-shm"] {
            std::fs::write(dir.join(format!("a.db{s}")), b"x").unwrap();
        }
        delete_vault(&v);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    }
}

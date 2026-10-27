//! Reading and writing `settings.json` with care (audit finding P3-2): private, atomic, the previous file kept, and a
//! damaged file kept and reported instead of silently replaced by the defaults.
use crate::i18n::{trf, Msg};
use rahasyakosh_app::{Language, Settings};
use rahasyakosh_store::private_fs;
use std::path::{Path, PathBuf};

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Looks at the settings file once at start-up. A file that exists but is not readable settings is kept as
/// `settings.json.damaged` (private, atomic) and the defaults then apply until the user saves. Returns the message
/// to show the user, if any. A missing file is normal (first run) and says nothing.
pub fn check(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    if Settings::parse(&bytes).is_some() {
        return None;
    }
    let kept = with_suffix(path, ".damaged");
    let _ = private_fs::write_private_atomic(&kept, &bytes);
    Some(trf(
        Language::English,
        Msg::SettingsUnreadable,
        &[&kept.display()],
    ))
}

/// Saves the settings. The readable file they replace is kept as `settings.json.bak` first, so one bad save is not
/// final.
pub fn save(path: &Path, s: &Settings) -> Result<(), String> {
    let write = || -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            private_fs::create_private_dir(dir)?;
        }
        if let Ok(old) = std::fs::read(path) {
            if Settings::parse(&old).is_some() {
                private_fs::write_private_atomic(&with_suffix(path, ".bak"), &old)?;
            }
        }
        private_fs::write_private_atomic(path, &s.to_json())
    };
    write().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rk-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d.join("rahasyakosh/settings.json")
    }

    #[test]
    fn a_missing_file_is_normal() {
        assert_eq!(check(&scratch("missing")), None);
    }

    #[test]
    fn saving_keeps_the_previous_file_and_reads_back() {
        let path = scratch("save");
        let a = Settings {
            idle_lock_secs: 120,
            ..Settings::default()
        };
        let b = Settings {
            idle_lock_secs: 240,
            ..Settings::default()
        };
        save(&path, &a).unwrap();
        assert!(!with_suffix(&path, ".bak").exists(), "nothing to keep yet");
        save(&path, &b).unwrap();
        assert_eq!(Settings::parse(&std::fs::read(&path).unwrap()), Some(b));
        let bak = std::fs::read(with_suffix(&path, ".bak")).unwrap();
        assert_eq!(Settings::parse(&bak), Some(a));
        assert_eq!(check(&path), None);
    }

    #[test]
    fn a_damaged_file_is_kept_and_reported_and_does_not_replace_the_good_backup() {
        let path = scratch("damaged");
        save(&path, &Settings::default()).unwrap();
        save(&path, &Settings::default()).unwrap();
        std::fs::write(&path, b"garbage").unwrap();
        let msg = check(&path).expect("told");
        assert!(msg.contains("settings.json.damaged"), "{msg}");
        assert_eq!(
            std::fs::read(with_suffix(&path, ".damaged")).unwrap(),
            b"garbage"
        );
        // Saving over the damaged file does not turn the damaged text into the backup.
        save(&path, &Settings::default()).unwrap();
        let bak = std::fs::read(with_suffix(&path, ".bak")).unwrap();
        assert!(Settings::parse(&bak).is_some());
        assert_eq!(check(&path), None);
    }

    #[cfg(unix)]
    #[test]
    fn the_files_and_the_folder_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let path = scratch("modes");
        save(&path, &Settings::default()).unwrap();
        save(&path, &Settings::default()).unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(&with_suffix(&path, ".bak")), 0o600);
        assert_eq!(mode(path.parent().unwrap()), 0o700);
    }

    #[test]
    fn a_failed_save_says_why() {
        let path = scratch("fail");
        // A file where the folder should be.
        std::fs::create_dir_all(path.parent().unwrap().parent().unwrap()).unwrap();
        std::fs::write(path.parent().unwrap(), b"x").unwrap();
        assert!(save(&path, &Settings::default()).is_err());
    }
}

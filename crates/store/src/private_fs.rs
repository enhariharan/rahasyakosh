//! Files and folders only their owner can use (audit findings P6-2, P3-2, P2-5).
//!
//! The vault file itself was always private. The folders around it, the profile list and the settings file were
//! made with the default umask, so other users on the same computer could list and read them. These helpers are
//! the one way the programs create such things. On systems without Unix permissions they do the same work without
//! the mode.
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Create `dir` and any missing parents, each new folder readable only by its owner (mode 0700). A folder that
/// already exists is left as it is: it may be the user's own (`/tmp`, a shared folder), and only
/// [`tighten_tree`] should change those, on folders the program owns.
pub fn create_private_dir(dir: &Path) -> io::Result<()> {
    let mut b = fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut b, 0o700);
    b.create(dir)
}

/// Write `bytes` to `path` so that a crash leaves either the old file or the new one, never half: a temporary file
/// next to it, created 0600 (a leftover or planted file of that name is removed first, never written through),
/// flushed, then renamed over `path`.
pub fn write_private_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = temp_name(path);
    match fs::remove_file(&tmp) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    let mut f = opts.open(&tmp)?;
    let written = f.write_all(bytes).and_then(|()| f.sync_all());
    drop(f);
    if let Err(e) = written.and_then(|()| fs::rename(&tmp, path)) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

fn temp_name(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
    path.with_file_name(format!(".{}.tmp", name.as_deref().unwrap_or("file")))
}

/// Are `a` and `b` the same file on disk, however they are spelled (a relative path, a symlink, a hard link)? Both
/// must exist; two paths to nothing are not "the same file". Audit finding P4-2.
pub fn is_same_file(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (fs::metadata(a), fs::metadata(b)) {
            (Ok(x), Ok(y)) => x.dev() == y.dev() && x.ino() == y.ino(),
            _ => false,
        }
    }
    #[cfg(not(unix))]
    {
        match (fs::canonicalize(a), fs::canonicalize(b)) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

/// Does `path` start with the SQLite file signature? Whatever it is called, that is somebody's database (possibly a
/// vault), and an export must not replace it.
pub fn is_sqlite_database(path: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 15];
    fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut head))
        .is_ok()
        && &head == b"SQLite format 3"
}

/// Make `root` and everything under it owner-only: folders 0700, files 0600. Used at start-up on the folders the
/// program owns, to repair installs made before these rules. Symlinks are neither followed nor changed. Returns how
/// many entries were changed. Missing paths are fine (nothing to do).
pub fn tighten_tree(root: &Path) -> io::Result<usize> {
    #[cfg(not(unix))]
    {
        let _ = root;
        Ok(0)
    }
    #[cfg(unix)]
    {
        fn go(p: &Path, depth: u32, changed: &mut usize) -> io::Result<()> {
            use std::os::unix::fs::PermissionsExt;
            let meta = match fs::symlink_metadata(p) {
                Ok(m) => m,
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(e) => return Err(e),
            };
            let want = if meta.is_dir() {
                0o700
            } else if meta.is_file() {
                0o600
            } else {
                return Ok(());
            };
            if meta.permissions().mode() & 0o777 != want {
                fs::set_permissions(p, fs::Permissions::from_mode(want))?;
                *changed += 1;
            }
            if meta.is_dir() && depth < 4 {
                for entry in fs::read_dir(p)? {
                    go(&entry?.path(), depth + 1, changed)?;
                }
            }
            Ok(())
        }
        let mut changed = 0;
        go(root, 0, &mut changed)?;
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("rk-private-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        p
    }

    #[cfg(unix)]
    fn mode(p: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(p).unwrap().permissions().mode() & 0o777
    }

    #[cfg(unix)]
    #[test]
    fn new_folders_are_private_and_old_ones_are_left_alone() {
        use std::os::unix::fs::PermissionsExt;
        let root = scratch("dirs");
        fs::create_dir_all(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        create_private_dir(&root.join("a/b")).unwrap();
        assert_eq!(mode(&root.join("a")), 0o700);
        assert_eq!(mode(&root.join("a/b")), 0o700);
        assert_eq!(
            mode(&root),
            0o755,
            "an existing folder is not ours to change"
        );
        create_private_dir(&root.join("a/b")).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn an_atomic_write_is_private_replaces_the_file_and_leaves_no_temp_file() {
        let root = scratch("atomic");
        fs::create_dir_all(&root).unwrap();
        let f = root.join("x.json");
        write_private_atomic(&f, b"one").unwrap();
        write_private_atomic(&f, b"two").unwrap();
        assert_eq!(fs::read(&f).unwrap(), b"two");
        assert_eq!(mode(&f), 0o600);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn a_planted_temp_file_or_link_is_not_written_through() {
        let root = scratch("planted");
        fs::create_dir_all(&root).unwrap();
        let victim = root.join("victim");
        fs::write(&victim, b"keep").unwrap();
        std::os::unix::fs::symlink(&victim, root.join(".x.json.tmp")).unwrap();
        write_private_atomic(&root.join("x.json"), b"new").unwrap();
        assert_eq!(fs::read(&victim).unwrap(), b"keep");
        assert_eq!(fs::read(root.join("x.json")).unwrap(), b"new");
    }

    #[cfg(unix)]
    #[test]
    fn the_same_file_is_recognised_under_any_spelling() {
        let root = scratch("same");
        fs::create_dir_all(&root).unwrap();
        let a = root.join("v.db");
        fs::write(&a, b"SQLite format 3\0rest").unwrap();
        std::os::unix::fs::symlink(&a, root.join("link.db")).unwrap();
        fs::hard_link(&a, root.join("hard.db")).unwrap();
        fs::write(root.join("other.db"), b"x").unwrap();
        assert!(is_same_file(
            &a,
            &root
                .join("../")
                .join(root.file_name().unwrap())
                .join("v.db")
        ));
        assert!(is_same_file(&a, &root.join("link.db")));
        assert!(is_same_file(&a, &root.join("hard.db")));
        assert!(!is_same_file(&a, &root.join("other.db")));
        assert!(!is_same_file(&a, &root.join("missing")));
        assert!(!is_same_file(&root.join("missing"), &root.join("missing")));
        assert!(is_sqlite_database(&a));
        assert!(!is_sqlite_database(&root.join("other.db")));
        assert!(!is_sqlite_database(&root.join("missing")));
    }

    #[cfg(unix)]
    #[test]
    fn tightening_repairs_modes_without_following_links() {
        use std::os::unix::fs::PermissionsExt;
        let root = scratch("tighten");
        fs::create_dir_all(root.join("profiles")).unwrap();
        fs::write(root.join("profiles.json"), b"{}").unwrap();
        fs::write(root.join("profiles/a.db"), b"x").unwrap();
        let outside = scratch("tighten-outside");
        fs::write(&outside, b"y").unwrap();
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o644)).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
        for p in [&root, &root.join("profiles")] {
            fs::set_permissions(p, fs::Permissions::from_mode(0o775)).unwrap();
        }
        for p in [root.join("profiles.json"), root.join("profiles/a.db")] {
            fs::set_permissions(p, fs::Permissions::from_mode(0o664)).unwrap();
        }
        assert_eq!(tighten_tree(&root).unwrap(), 4);
        assert_eq!(mode(&root), 0o700);
        assert_eq!(mode(&root.join("profiles")), 0o700);
        assert_eq!(mode(&root.join("profiles.json")), 0o600);
        assert_eq!(mode(&root.join("profiles/a.db")), 0o600);
        assert_eq!(mode(&outside), 0o644, "a link's target is not ours");
        assert_eq!(tighten_tree(&root).unwrap(), 0);
        assert_eq!(tighten_tree(&root.join("missing")).unwrap(), 0);
    }
}

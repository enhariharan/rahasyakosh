//! Where the program keeps its own files.
//!
//! Inside a snap, snapd points `$HOME` and the XDG folders at a per-revision copy (`~/snap/<name>/<rev>/`). A
//! `snap revert` brings back the older revision's copy, which would roll back the vault, its secret keys and the
//! high-water mark of P2-1 together. So inside a snap the program's files go under `$SNAP_USER_COMMON`
//! (`~/snap/<name>/common/`), which every revision shares and a revert leaves alone. Outside a snap nothing
//! changes: the usual XDG variable, else the folder under `$HOME`.
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// An absolute path from an environment value; an empty or relative value counts as unset.
fn absolute(v: Option<OsString>) -> Option<PathBuf> {
    v.map(PathBuf::from).filter(|p| p.is_absolute())
}

/// The snap's per-user folder shared by all revisions, when running inside a snap.
pub fn snap_common_dir() -> Option<PathBuf> {
    absolute(std::env::var_os("SNAP_USER_COMMON"))
}

/// The base folder for one kind of file: `<snap common>/<sub>` inside a snap, else the XDG folder, else
/// `<home>/<sub>`.
fn base(
    snap: Option<OsString>,
    xdg: Option<OsString>,
    home: Option<OsString>,
    sub: &[&str],
) -> Option<PathBuf> {
    let under = |root: &Path| sub.iter().fold(root.to_path_buf(), |p, s| p.join(s));
    absolute(snap)
        .map(|s| under(&s))
        .or_else(|| absolute(xdg))
        .or_else(|| home.map(|h| under(Path::new(&h))))
}

/// [`base`] read from the environment, with `xdg_var` as the XDG variable for this kind of file.
pub(crate) fn base_from_env(xdg_var: &str, sub: &[&str]) -> Option<PathBuf> {
    base(
        std::env::var_os("SNAP_USER_COMMON"),
        std::env::var_os(xdg_var),
        std::env::var_os("HOME"),
        sub,
    )
}

#[cfg(test)]
mod tests {
    use super::base;
    use std::path::PathBuf;

    fn os(s: &str) -> Option<std::ffi::OsString> {
        Some(s.into())
    }
    const SHARE: &[&str] = &[".local", "share"];

    #[test]
    fn snap_common_wins_over_xdg_and_home() {
        assert_eq!(
            base(
                os("/s/common"),
                os("/s/x1/.local/share"),
                os("/s/x1"),
                SHARE
            ),
            Some(PathBuf::from("/s/common/.local/share"))
        );
    }

    #[test]
    fn outside_a_snap_xdg_then_home() {
        assert_eq!(
            base(None, os("/xdg"), os("/h"), SHARE),
            Some(PathBuf::from("/xdg"))
        );
        assert_eq!(
            base(None, None, os("/h"), SHARE),
            Some(PathBuf::from("/h/.local/share"))
        );
        assert_eq!(base(None, None, None, SHARE), None);
    }

    #[test]
    fn empty_or_relative_values_are_ignored() {
        assert_eq!(
            base(os(""), os("rel"), os("/h"), SHARE),
            Some(PathBuf::from("/h/.local/share"))
        );
    }
}

//! The clipboard backend against stand-in `wl-copy`/`wl-paste` scripts that keep the "clipboard" in a file.
//! One test only, because it changes the process-wide environment.
#![cfg(unix)]
use rahasyakosh_clipboard::{Backend, Error};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn install_fakes(dir: &Path, clip: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let script = |name: &str, body: &str| {
        let p = dir.join(name);
        std::fs::write(&p, format!("#!/bin/sh\nf=\"{}\"\n{body}\n", clip.display())).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    };
    script(
        "wl-copy",
        "if [ \"$1\" = \"--clear\" ]; then : > \"$f\"; exit 0; fi\n/bin/cat > \"$f\"",
    );
    script("wl-paste", "[ -s \"$f\" ] || exit 1\n/bin/cat \"$f\"");
}

#[test]
fn copy_read_clear_and_clear_only_if_unchanged() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("clipboard-crate");
    let _ = std::fs::remove_dir_all(&root);
    let (bin, clip) = (root.join("bin"), root.join("clipboard"));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&clip, "").unwrap();

    // No tools on the PATH: a clear error that says what to install.
    std::fs::create_dir_all(&bin).unwrap();
    std::env::set_var("PATH", &bin);
    std::env::set_var("WAYLAND_DISPLAY", "fake-0");
    std::env::remove_var("DISPLAY");
    let e = Backend::detect().unwrap_err();
    assert!(matches!(e, Error::NoTool));
    assert!(e.to_string().contains("wl-clipboard") && e.to_string().contains("xclip"));

    install_fakes(&bin, &clip);
    let b = Backend::detect().unwrap();
    assert_eq!(b, Backend::Wayland);

    assert!(
        b.read().unwrap().is_none(),
        "an empty clipboard reads as None"
    );
    b.copy("s3cret-pw").unwrap();
    assert_eq!(&*b.read().unwrap().unwrap(), "s3cret-pw");

    // Unchanged: cleared.
    assert!(b.clear_if_unchanged("s3cret-pw").unwrap());
    assert!(b.read().unwrap().is_none());

    // The user copied something else meanwhile: left alone.
    b.copy("something else").unwrap();
    assert!(!b.clear_if_unchanged("s3cret-pw").unwrap());
    assert_eq!(&*b.read().unwrap().unwrap(), "something else");

    b.clear().unwrap();
    assert!(b.read().unwrap().is_none());
    // Unicode and awkward bytes survive.
    b.copy("pässwörd रहस्य 🔐 \"q\" \\ $x").unwrap();
    assert_eq!(&*b.read().unwrap().unwrap(), "pässwörd रहस्य 🔐 \"q\" \\ $x");
}

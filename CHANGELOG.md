# Changelog

All notable changes to Rahasyakosh are written here, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows
[Semantic Versioning](https://semver.org/) once it has a first release.

**Alpha release — the app has not been independently audited. Do not store real secrets.**

## [Unreleased]

### Fixed
- **Release list in software centres:** the AppStream metadata listed only the first alpha, as version `0.1.0` dated 2026-11-12, so app stores would show it as the newest release (`0.1.0` ranks above every `0.1.0` alpha). It now lists alpha.10 and alpha.1, with pre-release versions written `0.1.0~alpha10`, which rank in the right order.

## [0.1.0-alpha.10] — 2026-11-19

### Fixed
- **Release .deb version and name:** the GitHub release `.deb` was always `rahasyakosh_0.1.0-1_amd64.deb`, with version `0.1.0-1` taken from `Cargo.toml`. apt ranks that above every PPA alpha and even the PPA's final 0.1.0, so anyone who installed it would never get PPA upgrades. The package version now comes from the tag (`0.1.0~alpha10-1`, the PPA's scheme) and the file is named after the tag (`rahasyakosh_0.1.0-alpha.10_amd64.deb`). If you installed the alpha.9 `.deb` from GitHub, remove it once (`sudo apt remove rahasyakosh`) before installing a newer one or using the PPA.
- **Release checksums:** `SHA256SUMS.txt` listed build paths (`target/release/rk`), so `sha256sum --check` failed on the downloaded files, and it left out the `.deb`. It now lists bare file names, including the `.deb`; check with `sha256sum --check --ignore-missing SHA256SUMS.txt`.
- **AUR package:** the PKGBUILD downloaded a `.deb` name no release had. It now uses the tag-based name, which releases have from v0.1.0-alpha.10 on.

## [0.1.0-alpha.9] — 2026-11-19

### Fixed
- **PPA uploads:** Launchpad's upload server answered `550 Requested action not taken: internal server error` partway through the alpha.7 and alpha.8 uploads, which stopped the rest. noble has alpha.7 and alpha.8; resolute and stonking got neither. The workflow now retries each series up to 3 times and carries on with the others if one still fails, then lists what was not uploaded. No app changes since alpha.6.

## [0.1.0-alpha.8] — 2026-11-19

### Changed
- **PPA series:** the PPA now gets builds for the Ubuntu releases Launchpad still supports: noble (24.04 LTS), resolute (26.04 LTS) and stonking (26.10). oracular (24.10), plucky (25.04) and questing (25.10) have reached end of life and Launchpad refuses uploads to them; the alpha.7 upload stopped at plucky, so resolute never got alpha.7. No app changes since alpha.6.

## [0.1.0-alpha.7] — 2026-11-19

### Fixed
- **PPA uploads:** the alpha.6 PPA run stopped before uploading anything (`unmet build dependencies: build-essential`), because the runner image does not include `build-essential`. The workflow now installs it. No app changes since alpha.6.

## [0.1.0-alpha.6] — 2026-11-19

### Added
- **Packaging for Arch:** an AUR package (`aur/rahasyakosh-bin`), with a checklist in `docs/deploy/`.

### Fixed
- **PPA workflow:** a YAML parse error that stopped `ppa.yml` from running at all (the alpha.5 PPA runs failed at once).
- **Desktop app on a Wayland desktop with no X server:** the app no longer quits at start-up with `cannot open display` / `readPIDFromPeer`. Dioxus moves WebKit's page renderer to X11 on Wayland desktops; the app now skips that move when no X server can be reached and keeps the renderer on Wayland. Nothing changes where an X server is available (the .deb, PPA and snap on GNOME, which provides XWayland).
- **PPA uploads:** the `apps/desktop/debian/` packaging folder the PPA workflow copies on each release tag is now in the repository.

## [0.1.0-alpha.5] — 2026-11-19

### Added
- **PPA for all supported Ubuntu series:** install with `sudo add-apt-repository ppa:enhariharan/rahasyakosh && sudo apt install rahasyakosh`. Covers noble (24.04 LTS), oracular (24.10), plucky (25.04), questing (25.10), and resolute (26.04 LTS).
- **Automated PPA uploads:** a new `ppa.yml` workflow builds and uploads signed source packages to the PPA for all five Ubuntu series on every release tag.

### Fixed
- **`rk profile use <username>` now affects the CLI:** subsequent `rk` commands (without `--profile` or `RK_PROFILE`) now operate on the last-used profile's vault, matching the desktop app's behaviour.
- A `.1pux` file that is not a valid ZIP archive now gives a clear error ("not a valid .1pux archive") in the desktop app instead of silently falling through to the CSV parser.
- An empty ZIP file given a `.1pux` extension now gives a clear "missing export.data" error instead of a confusing CSV-column error.

### How to verify
Downloads are built and signed by GitHub Actions. To verify a binary:
```
# Check the SHA-256 checksum
sha256sum --check SHA256SUMS.txt

# Verify GitHub Actions build provenance (requires the gh CLI)
gh attestation verify rk --repo enhariharan/rahasyakosh
gh attestation verify rahasyakosh-desktop --repo enhariharan/rahasyakosh
```

### Known issues
- macOS and Windows are not yet supported.
- The app has not been independently audited. **Do not store real secrets until an audit is complete.**
- Deleting a profile has no trash; it removes the vault file permanently.

## [0.1.0-alpha.4] — 2026-11-17

### Added
- **KeePass `.kdbx` import (US-IMP-02):** import a KeePass 4 database directly from the desktop import screen or with `rk import file.kdbx`. The database password is prompted; `RK_KDBX_PASSWORD` can supply it non-interactively.
- **1Password `.1pux` import (US-IMP-02):** import a 1Password unencrypted export from the desktop or with `rk import file.1pux`. Login and Password items, TOTP fields, vault names as tags, and URL/notes are all carried over.
- **`rk profile use <username>`:** sets which profile the desktop app opens by default (CLI behaviour fixed in alpha.5).

### Fixed
- `rk import` correctly detects KDBX magic bytes and `.1pux` ZIP headers regardless of the file name.
- An import file that is completely empty now shows a user-readable hint rather than a bare I/O error.
- Export warning text and note field labels sharpened.

### How to verify
Downloads are built and signed by GitHub Actions. To verify a binary:
```
# Check the SHA-256 checksum
sha256sum --check SHA256SUMS.txt

# Verify GitHub Actions build provenance (requires the gh CLI)
gh attestation verify rk --repo enhariharan/rahasyakosh
gh attestation verify rahasyakosh-desktop --repo enhariharan/rahasyakosh
```

### Known issues
- macOS and Windows are not yet supported.
- The app has not been independently audited. **Do not store real secrets until an audit is complete.**
- Deleting a profile has no trash; it removes the vault file permanently.

## [0.1.0-alpha.3] — 2026-11-14

### Added
- **Shell completions:** `rk completions bash|zsh|fish` generates shell completion scripts. See the [CLI reference](docs/book/src/cli-reference.md) for install instructions.
- **About page (Ctrl+4):** shows the version, a short description, and a link to the source repository.
- **`rahasyakosh.rk` snap command:** the `rk` CLI is now accessible inside the snap as `rahasyakosh.rk`.
- Banner image added to Snap Store listing.

### Fixed
- Shell completions work on bash, zsh and fish (manual checklist passed).

## [0.1.0-alpha.2] — 2026-11-14

### Fixed
- Orca (screen reader) no longer misreads the password field on initial load and after in-app navigation in the snap.
- The setup master-password fields now retry the AT-SPI blur/focus handshake until the accessibility service is ready, so Orca reliably echoes key presses on the first try.
- The password field in the item form now has a show/hide toggle, accessible by keyboard and screen reader.

## [0.1.0-alpha.1] — 2026-11-13

### Added
- **Profiles (desktop):** several people can share one installation, each with their own vault and master password. The app starts on the profile used last.
- **`RK_CONFIG_HOME`** sets the folder of the desktop settings file on any system; it takes priority over `XDG_CONFIG_HOME`.
- **`rk --profile USERNAME`** (or `RK_PROFILE`) opens a profile's vault from the command line; `rk profiles` lists them.
- **Import:** Bitwarden JSON exports (unencrypted), and Bitwarden, KeePassXC and 1Password CSV files are recognised. The desktop import screen has a file chooser and a column picker.
- **Export** to a plain text file, CSV or JSON in the desktop app, with the master password asked again.
- Desktop app for Linux (Dioxus and WebKitGTK): unlock, list and search, reveal and copy, two-factor codes, forms, trash, history, settings, seven languages, screen-reader support.
- Command-line tool `rk` and a local SQLite vault with history, trash and a 10 000-item performance test.
- Secret Key (a second secret kept apart from the vault file), addable to an existing vault from the desktop app.
- Fuzz targets for every parser of untrusted input, replayed in normal CI, and a weekly fuzz job.

### Changed
- A profile whose vault file is missing no longer blocks the other profiles: the problem is shown under the profile picker.
- Two profiles with the same full name are told apart in the list by their username.
- Deleting a profile is announced to a screen reader.
- An import file with no recognisable password column now asks which column to use instead of failing.

### Fixed
- Audit findings P1-8, P3-1, P4-1, P5-1 and P5-2 (a short generated master passphrase, a trash retention of 0 days, `RK_PASSWORD` in the clipboard helpers' environment, CSV import time growing with the square of the rows, and terminal escape sequences in stored text). See `docs/13-security-audit.md`.

### Not yet
- KeePass `.kdbx` and 1Password `.1pux` import; macOS and Windows; sync; an external security audit.

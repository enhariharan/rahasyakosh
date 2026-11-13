# Changelog

All notable changes to Rahasyakosh are written here, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows
[Semantic Versioning](https://semver.org/) once it has a first release.

**Alpha release — the app has not been independently audited. Do not store real secrets.**

## [Unreleased]

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

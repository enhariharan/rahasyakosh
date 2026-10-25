# Publishing Rahasyakosh as an open-source app for Ubuntu

Guidelines, not a record of work done. Status today: MIT licence, CI (build, tests, clippy, `cargo deny`, audit), a fuzz job, and a local installer (`apps/desktop/install-local.sh`). There is **no release process, no installable package, and no security audit yet**.

## 0. Before you publish anything
A password manager earns trust slowly. Do these first:
- [ ] **Be honest in the README.** Keep "not audited, do not store real secrets" until an independent review is done. Label early releases *alpha*.
- [ ] **Security audit** of `crates/core` (crypto, vault format) by an outside reviewer, or at least a public call for review. Publish the threat model (`docs/04-security-and-crypto.md`).
- [ ] **Fix the known rough edges** in [`11-next-steps.md`](11-next-steps.md) (profile start-up, missing vault file).
- [ ] **Data safety:** a vault-format change after release must migrate old vaults. Freeze the format only after the golden-vault tests pass; keep them in CI.
- [ ] **Backups and recovery:** document how users back up the vault folder, and say clearly that a lost master password cannot be recovered.
- [ ] Run all manual checklists in `docs/tests/` on a clean Ubuntu install (LTS 24.04 and the latest release), Wayland and X11, Orca on and off.

## 1. Repository hygiene (mostly present)
- [x] `LICENSE` (MIT), `CODE_OF_CONDUCT.md`, `CONTRIBUTING.md`, `SECURITY.md`.
- [ ] `SECURITY.md`: give a private reporting route (GitHub private vulnerability reporting, plus an email) and a response time.
- [ ] `CHANGELOG.md` (Keep a Changelog format) and Semantic Versioning. Tag releases `v0.1.0-alpha.1` and so on.
- [ ] Issue templates (bug, feature, accessibility) and a pull request template.
- [ ] Set the same version in every `Cargo.toml` (`apps/desktop` has its own workspace) and in the book.
- [ ] Keep `Cargo.lock` committed; build with `--locked`.
- [ ] Decide the name check: confirm "Rahasyakosh" is free as a project and package name (apt, snap, flathub, GitHub topics).

## 2. Reproducible, verifiable releases
- [ ] A GitHub Actions workflow that runs on a version tag: build on `ubuntu-22.04` (older glibc runs on more systems) with `cargo build --release --locked`, then upload the artifacts.
- [ ] Publish **SHA-256 checksums** and **sign** them (GPG key published on the project page, or `cosign`/sigstore keyless signing). Give users the exact verify commands.
- [ ] Generate a **software bill of materials** (`cargo cyclonedx`) and attach it; keep `cargo deny` and `cargo audit` green in the release job.
- [ ] Use GitHub **artifact attestations** (build provenance) so users can check a file came from your workflow.
- [ ] Never build releases from a developer laptop.

## 3. Packaging for Ubuntu: pick channels in this order
The desktop app needs WebKitGTK 4.1 (`libwebkit2gtk-4.1-0`), GTK 3 and libsoup 3; build dependencies are listed in `install-local.sh`. Ubuntu 22.04 and later ship these.

1. **`.deb` attached to GitHub Releases** (easiest first step)
   - Use `cargo-deb` (add `[package.metadata.deb]` to `apps/desktop/Cargo.toml`) or `cargo-dpkg`.
   - Install `rahasyakosh-desktop` to `/usr/bin`, the launcher script, `rahasyakosh.desktop` to `/usr/share/applications`, the icon to `/usr/share/icons/hicolor/scalable/apps`, and an AppStream file (see below).
   - Declare runtime `Depends:` (webkit, gtk3, libxdo, libayatana-appindicator) and set `Section: utils`, a proper description and the licence.
   - The launcher that turns on the accessibility service (see `install-local.sh`) must ship in the package, since screen-reader support depends on it.
   - Test: `sudo apt install ./rahasyakosh_*.deb` on a clean VM, then remove it with `apt remove` and confirm the vault in `~/.local/share/rahasyakosh/` is untouched.
2. **A PPA on Launchpad** so users get updates through `apt`
   - Needs a Launchpad account, a signed GPG key, and a Debian source package (`debian/` folder). Launchpad builds offline, so vendor the Rust crates (`cargo vendor`) or build a binary-only package.
   - Good for Ubuntu users, but more work to keep up.
3. **Snap** (Snap Store, default on Ubuntu)
   - `snapcraft.yaml` with the `gnome` extension; confinement `strict` is preferred. Check what a sandbox does to a password manager: access to the vault folder in the home directory, the clipboard, and the accessibility bus (Orca). Test with a screen reader inside the snap before publishing.
   - Register the name early in the Snap Store; ask for the right interfaces in the forum if needed.
4. **Flatpak / Flathub** (works on Ubuntu too, popular with privacy-minded users)
   - Needs a reproducible manifest, AppStream metadata and a review by the Flathub team. Same sandbox questions as for the snap.
5. **Official Ubuntu/Debian archive**: long term. Needs a Debian packaging sponsor and all Rust dependencies packaged individually. Do not plan for it in the alpha stage.

Avoid a plain `curl | sh` installer.

## 4. Desktop integration files
- [ ] **AppStream metadata** (`io.github.enhariharan.Rahasyakosh.metainfo.xml`): name, summary, description, screenshots, licence, release list, content rating, categories. Validate with `appstreamcli validate`. Needed by Software Centre, Flathub and the Snap Store.
- [ ] `rahasyakosh.desktop`: check with `desktop-file-validate`; categories `Utility;Security;`.
- [ ] Icons at several sizes (scalable SVG is present) and 2–4 screenshots in `docs/book/src/images/`.
- [ ] Choose the reverse-DNS app ID once and keep it (`io.github.enhariharan.Rahasyakosh` is the natural one).

## 5. Quality bar for each release
- [ ] CI green on the tagged commit (fmt, clippy `-D warnings`, tests, deny, audit, fuzz job recent).
- [ ] Manual checklists in `docs/tests/` run on the release candidate; results pasted in each file.
- [ ] Upgrade test: install the previous release, create a vault with two profiles, upgrade, confirm everything still unlocks.
- [ ] Clean-VM install test for each package format, on Ubuntu LTS and the newest release.
- [ ] Accessibility: Orca pass and 200% zoom pass on the release candidate (`docs/tests/a11y-test-checklist.md`).
- [ ] Release notes: what changed, known issues, and how to verify downloads.

## 6. Privacy and trust statements
- [ ] State plainly: no network access, no telemetry, no accounts. If that is true, ship it as a promise and keep a test or CI check that the app opens no network connections.
- [ ] Say what is stored where: vault files, `profiles.json` (names and usernames are **not encrypted**), settings, WebKit's own files.
- [ ] Say what the clipboard clearing can and cannot do (clipboard managers and Wayland compositors may keep history).

## 7. Community and project health
- [ ] Enable GitHub Discussions, Dependabot (cargo and Actions) and private vulnerability reporting.
- [ ] A short roadmap issue (macOS, Windows, sync, CLI profiles) so people know what is planned.
- [ ] Label good first issues; invite translators for the six non-English languages (native review is still outstanding).
- [ ] Announce when ready: r/rust, r/Ubuntu, r/selfhosted, Hacker News "Show HN", Ubuntu Discourse, the Rust users forum. Lead with the honest status (alpha, unaudited) and the accessibility work.

## 8. Suggested path
1. Finish the unlock checklist and the profile rough edges.
2. Add `CHANGELOG.md`, issue templates and the release workflow with checksums and signing.
3. Ship **v0.1.0-alpha.1** as a `.deb` on GitHub Releases, clearly marked unaudited.
4. Add AppStream metadata, then a snap (and/or Flatpak) once sandbox behaviour with Orca and the vault folder is tested.
5. Seek an independent review of the crypto, then drop the "do not store real secrets" warning only after that.
6. Consider a PPA once there is regular demand for updates through `apt`.

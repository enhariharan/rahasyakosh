# Publishing Rahasyakosh as an open-source app for Ubuntu

Guidelines, not a record of work done.

Status today (2026-11-13):
* MIT licence,
* CI green (build, tests, clippy, `cargo deny`, audit),
* A fuzz job,
* A local installer (`apps/desktop/install-local.sh`),
* A `.deb` package built with `cargo-deb`,
* A tag-triggered release workflow with SHA-256 checksums and sigstore attestations,
* All manual checklists passing on Ubuntu 24.04 Wayland and X11 (GNOME Boxes VM, 2026-11-13),
* v0.1.0-alpha.1 published on GitHub Releases with release notes,
* Backups/recovery and privacy docs in the book,
* Name "Rahasyakosh" confirmed free on apt, snap and GitHub.

There is **no security audit yet**.

## 0. Before you publish anything
A password manager earns trust slowly. Do these first:
- [ ] **Be honest in the README.** Keep "not audited, do not store real secrets" until an independent review is done. Label early releases *alpha*.
- [ ] **Security audit** of `crates/core` (crypto, vault format) by an outside reviewer, or at least a public call for review. Publish the threat model (`docs/04-security-and-crypto.md`).
- [x] **Fix the known rough edges** in [`11-next-steps.md`](11-next-steps.md) (profile start-up, missing vault file). Done 2026-10-28; the new rows in `tests/profiles-test-checklist.md` still need a manual pass.
- [ ] **Data safety:** a vault-format change after release must migrate old vaults. Freeze the format only after the golden-vault tests pass; keep them in CI.
- [x] **Backups and recovery:** document how users back up the vault folder, and say clearly that a lost master password cannot be recovered. Done 2026-11-13 (`docs/book/src/backups.md`).
- [x] Run all manual checklists in `docs/tests/` on a clean Ubuntu install (LTS 24.04 and the latest release), Wayland and X11, Orca on and off. Done 2026-11-13 (Wayland and X11, GNOME Boxes VM, Ubuntu 24.04).

## 1. Repository hygiene (mostly present)
- [x] `LICENSE` (MIT), `CODE_OF_CONDUCT.md`, `CONTRIBUTING.md`, `SECURITY.md`.
- [x] `SECURITY.md`: has the GitHub private reporting route, a 7-day response time, and an email address. Private vulnerability reporting enabled 2026-11-13.
- [x] `CHANGELOG.md` (Keep a Changelog format) is in place with an **Unreleased** section. Still to do: Semantic Versioning in practice, and tagging releases `v0.1.0-alpha.1` and so on.
- [x] Issue templates (bug, feature, accessibility) and a pull request template, in `.github/`.
- [x] Set the same version in every `Cargo.toml` (`apps/desktop` has its own workspace) and in the book. Both workspaces are at `0.1.0`; the book has no version field.
- [x] Keep `Cargo.lock` committed; build with `--locked`. Both are in place.
- [x] Decide the name check: confirm "Rahasyakosh" is free as a project and package name (apt, snap, flathub, GitHub topics). Checked 2026-11-13: not in apt, not in Snap Store, only enhariharan/rahasyakosh on GitHub. Flathub requires manual search; nothing found in public index.

## 2. Reproducible, verifiable releases
- [x] A GitHub Actions workflow that runs on a version tag: builds on `ubuntu-22.04` with `cargo build --release --locked`, produces a `.deb`, and uploads artifacts. Done 2026-11-07.
- [x] Publish **SHA-256 checksums** and sign them with sigstore keyless signing (`cosign`). Done 2026-11-07.
- [x] Generate a **software bill of materials** (`cargo cyclonedx`) and attach it; keep `cargo deny` and `cargo audit` green in the release job. Done 2026-11-13 (`.cdx.json` files attached to release).
- [x] Use GitHub **artifact attestations** (build provenance) so users can check a file came from your workflow. Done 2026-11-07.
- [ ] Never build releases from a developer laptop.

## 3. Packaging for Ubuntu: pick channels in this order
The desktop app needs WebKitGTK 4.1 (`libwebkit2gtk-4.1-0`), GTK 3 and libsoup 3; build dependencies are listed in `install-local.sh`. Ubuntu 22.04 and later ship these.

1. **`.deb` attached to GitHub Releases** (easiest first step)
   - Use `cargo-deb` (add `[package.metadata.deb]` to `apps/desktop/Cargo.toml`) or `cargo-dpkg`.
   - Install `rahasyakosh-desktop` to `/usr/bin`, the launcher script, `rahasyakosh.desktop` to `/usr/share/applications`, the icon to `/usr/share/icons/hicolor/scalable/apps`, and an AppStream file (see below).
   - Declare runtime `Depends:` (webkit, gtk3, libxdo, libayatana-appindicator) and set `Section: utils`, a proper description and the licence.
   - The launcher that turns on the accessibility service (see `install-local.sh`) must ship in the package, since screen-reader support depends on it.
   - Test: `sudo apt install ./rahasyakosh_*.deb` on a clean VM, then remove it with `apt remove` and confirm the vault in `~/.local/share/rahasyakosh/` is untouched. **Done 2026-11-13 (v0.1.0-alpha.1, Ubuntu 24.04 Wayland VM).**
2. **A PPA on Launchpad** so users get updates through `apt`
   - Needs a Launchpad account, a signed GPG key, and a Debian source package (`debian/` folder). Launchpad builds offline, so vendor the Rust crates (`cargo vendor`) or build a binary-only package.
   - Good for Ubuntu users, but more work to keep up.
3. **Snap** (Snap Store, default on Ubuntu)
   - `snapcraft.yaml` with the `gnome` extension; confinement `strict` is preferred. Check what a sandbox does to a password manager: access to the vault folder in the home directory, the clipboard, and the accessibility bus (Orca). Test with a screen reader inside the snap before publishing.
   - Register the name early in the Snap Store; ask for the right interfaces in the forum if needed.
   - **Progress 2026-11-13:** `snap/snapcraft.yaml` written (core24 base, gnome extension, strict confinement, `personal-files` plug for the vault folder, `desktop-legacy` for AT-SPI/Orca). Name `rahasyakosh` registered in the Snap Store. Build not yet complete: Multipass and LXD both time out waiting for networking on this host (Ubuntu 26.04, all QEMU bridge interfaces NO-CARRIER). Next step: diagnose bridge networking or find an alternative build environment (e.g. a Ubuntu 24.04 VM or CI runner).
   - **Progress 2026-11-13 (later):** `snapcraft pack` now builds `rahasyakosh_0.1.0-alpha.1_amd64.snap` (77 MB) locally with LXD. The networking timeout was `ufw` blocking DHCP/DNS on the LXD bridge; fixed with `sudo ufw allow in on lxdbr0` and `sudo ufw route allow in|out on lxdbr0`. (Use LXD, not `--destructive-mode`: a core24 snap cannot be built directly on a 26.04 host.) The build uses only committed code (`source-type: git`), so commit before packing. Next: install with `sudo snap install --dangerous`, then test the vault folder, clipboard, tray and Orca inside the sandbox before uploading to the Snap Store.
4. **Flatpak / Flathub** (works on Ubuntu too, popular with privacy-minded users)
   - Needs a reproducible manifest, AppStream metadata and a review by the Flathub team. Same sandbox questions as for the snap.
5. **Official Ubuntu/Debian archive**: long term. Needs a Debian packaging sponsor and all Rust dependencies packaged individually. Do not plan for it in the alpha stage.

Avoid a plain `curl | sh` installer.

## 4. Desktop integration files
- [x] **AppStream metadata** (`apps/desktop/packaging/io.github.enhariharan.Rahasyakosh.metainfo.xml`): written, passes `appstreamcli validate --no-net`, includes screenshots, a release entry and a `developer` entry. Done 2026-11-07.
- [x] `rahasyakosh.desktop`: passes `desktop-file-validate` (one hint: it suggests adding `Settings` or `System` as a second category; categories are `Utility;Security;`).
- [x] Icons at several sizes: PNG 16/32/48/64/128/256px generated from SVG and added to `.deb` assets and `install-local.sh`. Done 2026-11-13.
- [x] Screenshots (4): unlock, first-run welcome, item detail with TOTP, edit form — all 1080×720 in `docs/book/src/images/`, referenced in AppStream metainfo. Done 2026-11-13.
- [ ] Choose the reverse-DNS app ID once and keep it (`io.github.enhariharan.Rahasyakosh` is the natural one).

## 5. Quality bar for each release
- [x] CI green on the tagged commit (fmt, clippy `-D warnings`, tests, deny, audit, fuzz job recent). Green on commits `ad76505` and `20ca27c` (2026-11-13).
- [x] Manual checklists in `docs/tests/` run on the release candidate; results pasted in each file. Done 2026-11-13 (Wayland and X11).
- [x] Upgrade test: install the previous release, create a vault with two profiles, upgrade, confirm everything still unlocks. Not applicable for v0.1.0-alpha.1 (first release). Required from v0.1.0-alpha.2 onward.
- [ ] Clean-VM install test for each package format, on Ubuntu LTS and the newest release.
- [x] Accessibility: Orca pass and 200% zoom pass on the release candidate (`docs/tests/a11y-test-checklist.md`). Done 2026-11-13.
- [ ] Release notes: what changed, known issues, and how to verify downloads.

## 6. Privacy and trust statements
- [x] State plainly: no network access, no telemetry, no accounts. Done 2026-11-13 (`docs/book/src/privacy.md`).
- [x] Say what is stored where: vault files, `profiles.json` (names and usernames are **not encrypted**), settings, WebKit's own files. Done 2026-11-13.
- [x] Say what the clipboard clearing can and cannot do (clipboard managers and Wayland compositors may keep history). Done 2026-11-13.

## 7. Community and project health
- [x] Enable GitHub Discussions, Dependabot security updates, and private vulnerability reporting. Done 2026-11-13.
- [x] A short roadmap issue (macOS, Windows, sync, CLI profiles) so people know what is planned. Done 2026-11-13 (issue #1).
- [x] Label good first issues; invite translators for the six non-English languages. Done 2026-11-13 (issues #2–#4: Hindi translation review, shell completion, .desktop categories).
- [ ] Announce when ready: r/rust, r/Ubuntu, r/selfhosted, Hacker News "Show HN", Ubuntu Discourse, the Rust users forum. Lead with the honest status (alpha, unaudited) and the accessibility work.

## 8. Suggested path
1. Finish the unlock checklist and the profile rough edges.
2. Add `CHANGELOG.md`, issue templates and the release workflow with checksums and signing.
3. Ship **v0.1.0-alpha.1** as a `.deb` on GitHub Releases, clearly marked unaudited.
4. Add AppStream metadata, then a snap (and/or Flatpak) once sandbox behaviour with Orca and the vault folder is tested.
5. Seek an independent review of the crypto, then drop the "do not store real secrets" warning only after that.
6. Consider a PPA once there is regular demand for updates through `apt`.

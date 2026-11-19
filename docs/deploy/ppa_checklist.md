# Publishing Rahasyakosh to a Launchpad PPA

A PPA (Personal Package Archive) lets Ubuntu users install and update Rahasyakosh with
`sudo apt install rahasyakosh` without manually downloading `.deb` files. Launchpad builds
the package on clean Ubuntu VMs, so a successful PPA upload also satisfies the clean-VM
`.deb` install test for each Ubuntu series.

Status: not started (planned after `v0.1.0-alpha.4` `.deb` clean-VM test passes).

---

## Prerequisites

- [ ] A [Launchpad account](https://launchpad.net/) at `launchpad.net/~<your-id>`.
- [ ] A GPG key uploaded to the Ubuntu keyserver and linked to your Launchpad account:
  ```
  gpg --gen-key                              # if you don't have one
  gpg --keyserver keyserver.ubuntu.com --send-keys <KEY-ID>
  ```
  Then add the key fingerprint under **OpenPGP keys** on your Launchpad profile.
- [ ] `dput`, `devscripts`, `debhelper`, `dh-cargo` installed locally:
  ```
  sudo apt install dput devscripts debhelper dh-cargo
  ```
- [ ] The `.deb` clean-VM install test passed (`snap_store_checklist.md` section 5).

---

## 1. Register the PPA

1. Go to `launchpad.net/~<your-id>` → **Create a new PPA**.
2. Name it `rahasyakosh` (URL will be `ppa:enhariharan/rahasyakosh`).
3. Set the display name to "Rahasyakosh password manager" and mark it **public**.

---

## 2. Create the Debian source package

Launchpad builds from a **source package** (`.dsc` + `.orig.tar.gz` + `.debian.tar.xz`),
not a binary `.deb`. Two options:

### Option A: Binary-only package (simpler, recommended for alpha)
Compile the binary locally, wrap it in a minimal `debian/` folder, and upload a
source-less package. Launchpad accepts this for PPAs (unlike the main archive).

Steps:
1. Build the release binary:
   ```
   cd apps/desktop
   cargo build --release --locked
   ```
2. Create `apps/desktop/debian/` with the files below.
3. Build and sign the source package:
   ```
   cd apps/desktop
   dpkg-buildpackage -S -sa -k<KEY-ID>
   ```
4. Upload:
   ```
   dput ppa:enhariharan/rahasyakosh ../rahasyakosh_*.changes
   ```

### Option B: Vendored source (more Debian-idiomatic, larger upload)
1. `cargo vendor` to bundle all Rust dependencies (~150 MB).
2. Create `.cargo/config.toml` pointing at the vendor directory.
3. Follow the same `debian/` structure below; Launchpad builds everything from source.

---

## 3. `debian/` folder structure

Create these files under `apps/desktop/debian/`:

### `debian/control`
```
Source: rahasyakosh
Section: utils
Priority: optional
Maintainer: Hariharan Narayanan <enhariharan@gmail.com>
Build-Depends: debhelper-compat (= 13)
Standards-Version: 4.6.2
Homepage: https://github.com/enhariharan/rahasyakosh
Rules-Requires-Root: no

Package: rahasyakosh
Architecture: amd64
Depends: ${shlibs:Depends}, ${misc:Depends},
 libwebkit2gtk-4.1-0,
 libgtk-3-0,
 libxdo3,
 libayatana-appindicator3-1,
 libsoup-3.0-0
Description: Local-first, end-to-end-encrypted password manager
 Rahasyakosh keeps your logins and two-factor codes in an encrypted
 vault on your own computer. No accounts, no telemetry, no network.
 .
 Each person sharing the computer can have a profile with their own
 vault and master password. Passwords can be imported from browsers
 and other password managers (Bitwarden, KeePassXC, 1Password).
 .
 Full screen-reader support via AT-SPI/Orca.
 .
 Early alpha. Not independently audited — do not store real secrets
 until an audit is complete.
```

### `debian/changelog`
```
rahasyakosh (0.1.0~alpha4-1) noble; urgency=low

  * Initial PPA release.

 -- Hariharan Narayanan <enhariharan@gmail.com>  Wed, 18 Nov 2026 00:00:00 +0530
```
Update the series (`noble` = 24.04 LTS, `resolute` = 26.04 LTS,
`stonking` = 26.10; only series Launchpad still supports, see
`ppa_release_checklist.md`) and date for each upload. Use `dch` to edit it:
```
dch -i   # increment and open in $EDITOR
```

### `debian/rules`
```makefile
#!/usr/bin/make -f
%:
	dh $@

override_dh_auto_build:
	# Binary-only: binary is pre-built; nothing to compile here.

override_dh_auto_install:
	install -Dm755 target/release/rahasyakosh-desktop \
	    debian/rahasyakosh/usr/bin/rahasyakosh-desktop
	install -Dm755 packaging/rahasyakosh-launcher \
	    debian/rahasyakosh/usr/bin/rahasyakosh
	install -Dm644 packaging/rahasyakosh-system.desktop \
	    debian/rahasyakosh/usr/share/applications/rahasyakosh.desktop
	install -Dm644 packaging/rahasyakosh-chest.svg \
	    debian/rahasyakosh/usr/share/icons/hicolor/scalable/apps/rahasyakosh.svg
	install -Dm644 packaging/io.github.enhariharan.Rahasyakosh.metainfo.xml \
	    debian/rahasyakosh/usr/share/metainfo/io.github.enhariharan.Rahasyakosh.metainfo.xml
```

### `debian/compat`
```
13
```

---

## 4. Test the package locally before uploading

```bash
# Build a local .deb to smoke-test before hitting Launchpad
cd apps/desktop
dpkg-buildpackage -b -us -uc

# Install and test on a local VM
sudo apt install ../rahasyakosh_*.deb
rahasyakosh
sudo apt remove rahasyakosh
```

---

## 5. Upload to Launchpad and monitor the build

```bash
dput ppa:enhariharan/rahasyakosh ../rahasyakosh_*.changes
```

- Launchpad emails you when the build starts and when it succeeds or fails.
- Build log is at `launchpad.net/~enhariharan/+archive/ubuntu/rahasyakosh/+builds`.
- Builds for multiple series: repeat the upload with a different series in
  `debian/changelog` for each of `noble` (24.04), `oracular` (24.10), `questing` (25.10).

---

## 6. Verify the PPA install on a clean VM

For each Ubuntu series:
- [ ] `sudo add-apt-repository ppa:enhariharan/rahasyakosh && sudo apt update && sudo apt install rahasyakosh`
- [ ] App launches, vault creates, clipboard clears, Orca reads the window.
- [ ] `sudo apt remove rahasyakosh` leaves `~/.local/share/rahasyakosh/` untouched.
- [ ] `sudo apt upgrade` picks up a subsequent release without manual steps.

---

## 7. Announce the PPA

Add to README under the install options:
```
sudo add-apt-repository ppa:enhariharan/rahasyakosh
sudo apt install rahasyakosh
```

Update the Snap Store listing description to also mention the PPA.

---

## 8. Keeping the PPA current

For each new release:
1. Build the new binary locally.
2. Run `dch -v <new-version>-1` to update `debian/changelog`.
3. `dpkg-buildpackage -S -sa -k<KEY-ID>` and `dput` for each series.
4. Monitor the Launchpad build.

Consider automating this with a GHA workflow that triggers on `v*.*.*` tags,
mirrors the existing `release.yml` and `snap.yml` patterns.

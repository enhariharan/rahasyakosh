# AUR (Arch User Repository) Checklist

Package: `rahasyakosh-bin` — installs pre-built binaries from GitHub Releases.
Files live in `aur/rahasyakosh-bin/` in this repo.

---

## One-time setup

- [ ] Create an account at `https://aur.archlinux.org/` (requires a valid email).
- [ ] Add your SSH public key to your AUR profile under **My Account → SSH Public Key**.
- [ ] Clone the (empty) AUR package repo:
  ```bash
  git clone ssh://aur@aur.archlinux.org/rahasyakosh-bin.git ~/aur-rahasyakosh-bin
  ```
- [ ] Copy the initial files in and push:
  ```bash
  cp aur/rahasyakosh-bin/PKGBUILD ~/aur-rahasyakosh-bin/
  cp aur/rahasyakosh-bin/.SRCINFO ~/aur-rahasyakosh-bin/
  cd ~/aur-rahasyakosh-bin
  git add PKGBUILD .SRCINFO
  git commit -m "Initial release: rahasyakosh-bin 0.1.0.alpha5"
  git push
  ```

## Verify the package builds on Arch

On an Arch Linux machine or in a clean `archlinux` Docker container:
```bash
cd ~/aur-rahasyakosh-bin
makepkg -si
rahasyakosh    # desktop app
rk --version   # CLI
```

---

## Per-release update

For each new release (e.g. `v0.1.0-alpha.10`):

1. Get the SHA256 of the new `.deb` and `rk` binary. From v0.1.0-alpha.10 on, the
   `.deb` is named after the tag (`rahasyakosh_0.1.0-alpha.10_amd64.deb`) and the
   release's `SHA256SUMS.txt` lists both files, so you can read the hashes from it:
   ```bash
   gh release download v0.1.0-alpha.10 --pattern "*.deb" --pattern "rk" --pattern SHA256SUMS.txt -D ~/aur-new/
   cd ~/aur-new && sha256sum --check --ignore-missing SHA256SUMS.txt && cat SHA256SUMS.txt
   ```
   Releases up to alpha.9 named the file `rahasyakosh_0.1.0-1_amd64.deb`, which the
   PKGBUILD no longer looks for.

2. Update `aur/rahasyakosh-bin/PKGBUILD`:
   - `pkgver=0.1.0.alpha10`
   - `pkgrel=1`
   - `sha256sums_x86_64` with the new hashes

3. Regenerate `.SRCINFO`:
   ```bash
   cd aur/rahasyakosh-bin
   makepkg --printsrcinfo > .SRCINFO
   ```

4. Push to AUR:
   ```bash
   cp PKGBUILD .SRCINFO ~/aur-rahasyakosh-bin/
   cd ~/aur-rahasyakosh-bin
   git add PKGBUILD .SRCINFO
   git commit -m "Update to 0.1.0.alpha10"
   git push
   ```

---

## Automation (future)

The `release.yml` workflow can be extended to:
1. Compute SHA256 of release assets
2. Update `PKGBUILD` and `.SRCINFO` in this repo
3. Push to `aur@aur.archlinux.org` via a deploy key stored in GitHub secrets

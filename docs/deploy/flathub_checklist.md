# Flathub Checklist

Package: `io.github.enhariharan.Rahasyakosh`
Manifest: `flatpak/io.github.enhariharan.Rahasyakosh.yml`

Flathub requires a source build (no pre-built binaries). The manifest uses
the GNOME runtime (which bundles WebKitGTK) and the rust-stable SDK extension.

---

## One-time setup

### 1. Install tools
```bash
sudo apt install flatpak-builder
flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install flathub org.gnome.Platform//47 org.gnome.Sdk//47
flatpak install flathub org.freedesktop.Sdk.Extension.rust-stable//24.08
pip3 install aiohttp toml  # for flatpak-cargo-generator.py
```

### 2. Generate Cargo sources
Flathub builds are offline — all crates must be declared in the manifest.
```bash
# Download the generator (from flatpak/flatpak-builder-tools on GitHub)
curl -LO https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py

python3 flatpak-cargo-generator.py \
  apps/desktop/Cargo.lock \
  -o flatpak/generated-sources.json
```
Commit `generated-sources.json` alongside the manifest.

### 3. Test the build locally
```bash
flatpak-builder --force-clean --sandbox \
  /tmp/rk-flatpak-build \
  flatpak/io.github.enhariharan.Rahasyakosh.yml

flatpak-builder --run \
  /tmp/rk-flatpak-build \
  flatpak/io.github.enhariharan.Rahasyakosh.yml \
  rahasyakosh
```

### 4. Submit to Flathub
1. Fork `https://github.com/flathub/flathub`
2. Create branch `new-pr/io.github.enhariharan.Rahasyakosh`
3. Add directory `io.github.enhariharan.Rahasyakosh/` with:
   - `io.github.enhariharan.Rahasyakosh.yml` (the manifest)
   - `generated-sources.json`
4. Open a PR to `flathub/flathub` — the bot will run a CI build.
5. A Flathub team member reviews and merges. Expect 1–4 weeks.

---

## Per-release update

For each new release (e.g. `v0.1.0-alpha.6`):

1. Update the manifest tag and commit hash:
   ```yaml
   tag: v0.1.0-alpha.6
   commit: <full git commit hash>
   ```

2. Regenerate cargo sources (crates change between releases):
   ```bash
   python3 flatpak-cargo-generator.py \
     apps/desktop/Cargo.lock \
     -o flatpak/generated-sources.json
   ```

3. Test locally (step 3 above), then push to your Flathub fork.
   Flathub pulls from their own repo — open a PR to `flathub/flathub`.

---

## Sandbox policy notes

The app needs these `finish-args` (already in the manifest):

| Flag | Reason |
|---|---|
| `--share=network` | Dioxus Desktop binds a local WebSocket between the Rust process and the WebKit webview; without this the app panics |
| `--filesystem=home/.local/share/rahasyakosh:create` | Vault storage |
| `--filesystem=home/.config/rahasyakosh:create` | Settings |
| `--filesystem=home/.local/state/rahasyakosh:create` | State (last-used profile, etc.) |
| `--talk-name=org.a11y.Bus` | AT-SPI / Orca screen reader |
| `--env=GIO_USE_PROXY_RESOLVER=dummy` | Prevents blank white screen in WebKit inside Flatpak sandbox |

Flathub reviewers may ask to tighten `--share=network` to `--share=localhost`
(loopback only). Try `--share=localhost` first in local testing; if the
Dioxus WebSocket works, prefer it over full network access.

---

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Blank white screen | WebKit proxy resolver | Add `--env=GIO_USE_PROXY_RESOLVER=dummy` |
| App panics on startup | Dioxus WebSocket blocked | Ensure `--share=network` or `--share=localhost` |
| `cargo: command not found` | rust-stable extension not on PATH | Add `--build-option append-path` for the extension |
| Build fails: missing crate | `generated-sources.json` out of date | Re-run `flatpak-cargo-generator.py` |
| Flathub CI fails: metainfo error | AppStream validation failure | Run `appstream-util validate` on the `.metainfo.xml` |

# Flathub Checklist

Package: `io.github.enhariharan.Rahasyakosh`
Manifest: `flatpak/io.github.enhariharan.Rahasyakosh.yml`

Flathub requires a source build (no pre-built binaries). The manifest uses
the GNOME runtime (which bundles WebKitGTK) and the rust-stable SDK extension.
It builds a tagged release from GitHub (`tag:` and `commit:` in the manifest),
as Flathub will, so a local build tests that release, not your working copy.

---

## One-time setup

### 1. Install tools
```bash
sudo apt install flatpak-builder
flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install flathub org.gnome.Platform//47 org.gnome.Sdk//47
flatpak install flathub org.freedesktop.Sdk.Extension.rust-stable//24.08
pip3 install aiohttp tomlkit  # for flatpak-cargo-generator.py
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
Run these from the repository root.
```bash
flatpak-builder --force-clean --sandbox --delete-build-dirs \
  ~/.cache/rk-flatpak-build \
  flatpak/io.github.enhariharan.Rahasyakosh.yml

flatpak-builder --run \
  ~/.cache/rk-flatpak-build \
  flatpak/io.github.enhariharan.Rahasyakosh.yml \
  rahasyakosh
```
The build folder (`~/.cache/rk-flatpak-build`) must be on the same filesystem as
the folder you run the command from, because flatpak-builder keeps its cache in
`.flatpak-builder/` there and links files across. Don't use `/tmp`: on many
systems it is a RAM disk (tmpfs), and the build stops with `The state dir
(…/.flatpak-builder) is not on the same filesystem as the target dir (/tmp)`.
`--delete-build-dirs` removes each module's build folder afterwards; the
desktop app's build folder is over 2 GB.

#### Test uncommitted changes
The manifest fetches the tagged commit, so it never sees local edits. To build
your working copy instead, copy the manifest to `flatpak/local.yml` (git
ignores that name) and, in the `rahasyakosh` module only, replace the
`type: git` source (its `url`, `tag` and `commit` lines) with:
```yaml
      - type: dir
        path: ..
        skip:
          - target
          - apps/desktop/target
```
Keep the copy in `flatpak/` so `path: ..` and `generated-sources.json` still
resolve, then run step 3 with `flatpak/local.yml`. Never submit this copy.

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

For each new release (e.g. `v0.1.0-alpha.10`), after the tag is pushed:

1. Update the `rahasyakosh` module's tag and commit in the manifest. The commit
   is the full hash the tag points at:
   ```bash
   git rev-parse 'v0.1.0-alpha.10^{commit}'
   ```
   ```yaml
   tag: v0.1.0-alpha.10
   commit: <that full hash>
   ```

2. Regenerate cargo sources if `apps/desktop/Cargo.lock` changed since the
   previous release (this prints nothing when it hasn't):
   ```bash
   git diff --stat v0.1.0-alpha.9 v0.1.0-alpha.10 -- apps/desktop/Cargo.lock
   python3 flatpak-cargo-generator.py \
     apps/desktop/Cargo.lock \
     -o flatpak/generated-sources.json
   ```

3. Test locally (step 3 above). Before the app is accepted, update the files in
   your `flathub/flathub` pull request; once it is accepted, Flathub creates a
   repository for the app (`flathub/io.github.enhariharan.Rahasyakosh`), and
   later releases are pull requests there.

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
| `WebKitWebProcess: cannot open display:` then `readPIDFromPeer: Unexpected short read` | Dioxus sets `GDK_BACKEND=x11` for WebKit's renderer process, but on a Wayland desktop Flatpak hides X11 (`fallback-x11`), so the renderer finds no display and quits | Already handled in code: `platform::renderer_via_x11()` turns the switch off when `DISPLAY` is unset. Don't set `GDK_BACKEND` in the manifest or launcher; Dioxus overrides it anyway, and forcing one backend breaks hosts that only have the other |
| `The state dir (…/.flatpak-builder) is not on the same filesystem as the target dir (/tmp)` | `/tmp` is a RAM disk (tmpfs) and the build cache is on disk | Use a build folder on the same disk, e.g. `~/.cache/rk-flatpak-build` (step 3) |
| `cargo: command not found` | rust-stable extension not on PATH | Add `--build-option append-path` for the extension |
| Build fails: missing crate | `generated-sources.json` out of date | Re-run `flatpak-cargo-generator.py` |
| Flathub CI fails: metainfo error | AppStream validation failure | Run `appstream-util validate` on the `.metainfo.xml` |

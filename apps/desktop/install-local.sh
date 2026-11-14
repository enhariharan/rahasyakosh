#!/usr/bin/env bash
# Build the desktop app and install it for the current user only (nothing outside ~/.local is touched):
#   ~/.local/bin/rahasyakosh-desktop, a small launcher (~/.local/bin/rahasyakosh), an app-grid entry and an icon.
# Needs the build libraries once:  sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev libayatana-appindicator3-dev libsoup-3.0-dev
# Remove it again with:  ./install-local.sh --uninstall
set -euo pipefail
cd "$(dirname "$0")"
bin="$HOME/.local/bin/rahasyakosh-desktop"
launcher="$HOME/.local/bin/rahasyakosh"
desktop="$HOME/.local/share/applications/rahasyakosh.desktop"
icon="$HOME/.local/share/icons/hicolor/scalable/apps/rahasyakosh.svg"
icon_dir="$HOME/.local/share/icons/hicolor"

if [[ "${1:-}" == "--uninstall" ]]; then
  rm -f "$bin" "$launcher" "$desktop" "$icon"
  for size in 16 32 48 64 128 256 512; do
    rm -f "$icon_dir/${size}x${size}/apps/rahasyakosh.png"
  done
  echo "Removed the app. Your vault (~/.local/share/rahasyakosh/) and settings (~/.config/rahasyakosh/) were not touched."
  echo "The web view's own files remain and are safe to delete: ~/.local/share/rahasyakosh-desktop ~/.cache/rahasyakosh-desktop"
  exit 0
fi

cargo build --release --locked
mkdir -p "$(dirname "$bin")" "$(dirname "$desktop")" "$(dirname "$icon")"
install -m 755 target/release/rahasyakosh-desktop "$bin"
install -m 644 packaging/rahasyakosh-chest.svg "$icon"
for size in 16 32 48 64 128 256 512; do
  mkdir -p "$icon_dir/${size}x${size}/apps"
  install -m 644 "packaging/icons/rahasyakosh-${size}.png" "$icon_dir/${size}x${size}/apps/rahasyakosh.png"
done
# The launcher switches the desktop's accessibility service on before the app starts. An app that starts while it is
# off may never connect to a screen reader turned on later (typed characters are read out, alerts stay silent).
# The switch lasts for the login session, so it has to be done at every launch, not once at install time.
cat > "$launcher" <<LAUNCHER
#!/usr/bin/env bash
if command -v busctl >/dev/null 2>&1 \\
  && [[ "\$(busctl --user get-property org.a11y.Bus /org/a11y/bus org.a11y.Status IsEnabled 2>/dev/null)" == "b false" ]]; then
  busctl --user set-property org.a11y.Bus /org/a11y/bus org.a11y.Status IsEnabled b true >/dev/null 2>&1 || true
fi
exec "$bin" "\$@"
LAUNCHER
chmod 755 "$launcher"
sed "s|@EXEC@|$launcher|" packaging/rahasyakosh.desktop > "$desktop"
chmod 644 "$desktop"
# A stale per-user icon cache hides newly installed icons from the app grid, so drop it (it is rebuilt on demand).
rm -f "$HOME/.local/share/icons/hicolor/icon-theme.cache"
command -v update-desktop-database >/dev/null && update-desktop-database "$HOME/.local/share/applications" || true
echo "Installed. Find 'Rahasyakosh' in your app grid, or run: $launcher"

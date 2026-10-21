#!/usr/bin/env bash
# Build the desktop app and install it for the current user only (nothing outside ~/.local is touched):
#   ~/.local/bin/rahasyakosh-desktop, a launcher entry in the app grid, and an icon.
# Needs the build libraries once:  sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev libayatana-appindicator3-dev libsoup-3.0-dev
# Remove it again with:  ./install-local.sh --uninstall
set -euo pipefail
cd "$(dirname "$0")"
bin="$HOME/.local/bin/rahasyakosh-desktop"
desktop="$HOME/.local/share/applications/rahasyakosh.desktop"
icon="$HOME/.local/share/icons/hicolor/scalable/apps/rahasyakosh.svg"

if [[ "${1:-}" == "--uninstall" ]]; then
  rm -f "$bin" "$desktop" "$icon"
  echo "Removed the app. Your vault (~/.local/share/rahasyakosh/) and settings (~/.config/rahasyakosh/) were not touched."
  echo "The web view's own files remain and are safe to delete: ~/.local/share/rahasyakosh-desktop ~/.cache/rahasyakosh-desktop"
  exit 0
fi

cargo build --release --locked
mkdir -p "$(dirname "$bin")" "$(dirname "$desktop")" "$(dirname "$icon")"
install -m 755 target/release/rahasyakosh-desktop "$bin"
install -m 644 packaging/rahasyakosh.svg "$icon"
sed "s|@EXEC@|$bin|" packaging/rahasyakosh.desktop > "$desktop"
chmod 644 "$desktop"
command -v update-desktop-database >/dev/null && update-desktop-database "$HOME/.local/share/applications" || true
echo "Installed. Find 'Rahasyakosh' in your app grid, or run: $bin"

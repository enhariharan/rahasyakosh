#!/usr/bin/env bash
# Remove the locally installed app AND all vault and settings data.
# FOR LOCAL DEVELOPMENT ONLY — this permanently deletes the vault.
set -euo pipefail

bin="$HOME/.local/bin/rahasyakosh-desktop"
launcher="$HOME/.local/bin/rahasyakosh"
desktop="$HOME/.local/share/applications/rahasyakosh.desktop"
icon="$HOME/.local/share/icons/hicolor/scalable/apps/rahasyakosh.svg"
icon_dir="$HOME/.local/share/icons/hicolor"

echo "This will permanently delete:"
echo "  App binaries and desktop entry"
echo "  Vault and profiles:  ~/.local/share/rahasyakosh/"
echo "  Settings:            ~/.config/rahasyakosh/"
echo "  WebKit cache:        ~/.local/share/rahasyakosh-desktop/  ~/.cache/rahasyakosh-desktop/"
echo ""
read -r -p "Type YES to continue: " confirm
[[ "$confirm" == "YES" ]] || { echo "Aborted."; exit 1; }

# App binaries and desktop integration
rm -f "$bin" "$launcher" "$desktop" "$icon"
for size in 16 32 48 64 128 256 512; do
  rm -f "$icon_dir/${size}x${size}/apps/rahasyakosh.png"
done
rm -f "$HOME/.local/share/icons/hicolor/icon-theme.cache"
command -v update-desktop-database >/dev/null && update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true

# Vault, profiles and settings
rm -rf "$HOME/.local/share/rahasyakosh/"
rm -rf "$HOME/.config/rahasyakosh/"

# WebKit's own files (session storage, IndexedDB, disk cache)
rm -rf "$HOME/.local/share/rahasyakosh-desktop/"
rm -rf "$HOME/.cache/rahasyakosh-desktop/"

echo "Done. Everything removed."

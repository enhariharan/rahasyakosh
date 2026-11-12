#!/usr/bin/env bash
# Set up a clean Ubuntu 24.04 VM for running the Rahasyakosh manual checklists.
#
# Usage (inside the VM):
#   bash vm-setup.sh                      # build from source
#   bash vm-setup.sh --deb ~/rahasyakosh_*.deb  # install a pre-built .deb
#
# What this does:
#   1. Installs system libraries and Orca
#   2. Installs Rust (if not already present)
#   3. Clones the repo and builds with install-local.sh  (or installs the .deb)
#   4. Creates a throwaway vault folder and a launch helper
#   5. Prints the session-type check and checklist launch commands
set -euo pipefail

DEB_PATH=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --deb) DEB_PATH="$2"; shift 2 ;;
    *) echo "Unknown argument: $1"; exit 1 ;;
  esac
done

REPO_URL="https://github.com/enhariharan/rahasyakosh"
REPO_DIR="$HOME/rahasyakosh"
TEST_DIR="/tmp/rk-test"
LAUNCH_SCRIPT="$HOME/.local/bin/rk-test"

# ── 1. System packages ────────────────────────────────────────────────────────
echo "==> Installing system packages..."
sudo apt-get update -q
sudo apt-get install -y \
  orca at-spi2-core \
  libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev \
  libayatana-appindicator3-dev libsoup-3.0-dev \
  git curl build-essential

# ── 2. Rust ───────────────────────────────────────────────────────────────────
if ! command -v cargo &>/dev/null; then
  echo "==> Installing Rust..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
  export PATH="$HOME/.cargo/bin:$PATH"
else
  echo "==> Rust already installed ($(cargo --version))"
  export PATH="$HOME/.cargo/bin:$PATH"
fi

# ── 3. App install ────────────────────────────────────────────────────────────
if [[ -n "$DEB_PATH" ]]; then
  echo "==> Installing .deb: $DEB_PATH"
  sudo apt-get install -y "$DEB_PATH"
else
  echo "==> Cloning / updating repo..."
  if [[ -d "$REPO_DIR/.git" ]]; then
    git -C "$REPO_DIR" pull --ff-only
  else
    git clone "$REPO_URL" "$REPO_DIR"
  fi
  echo "==> Building and installing (this takes a few minutes)..."
  cd "$REPO_DIR/apps/desktop"
  ./install-local.sh
fi

# ── 4. Throwaway vault ────────────────────────────────────────────────────────
echo "==> Creating throwaway test folder: $TEST_DIR"
mkdir -p "$TEST_DIR"

mkdir -p "$(dirname "$LAUNCH_SCRIPT")"
cat > "$LAUNCH_SCRIPT" <<LAUNCHER
#!/usr/bin/env bash
# Launch Rahasyakosh against a throwaway vault so your real data is not touched.
export RK_VAULT=$TEST_DIR/vault.db
export RK_CONFIG_HOME=$TEST_DIR/cfg
exec rahasyakosh "\$@"
LAUNCHER
chmod +x "$LAUNCH_SCRIPT"

# ── 5. Summary ────────────────────────────────────────────────────────────────
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo " Setup complete."
echo ""
echo " Session type (run this now):"
echo "   echo \$XDG_SESSION_TYPE   # expect: wayland"
echo ""
echo " Launch with throwaway vault:"
echo "   $LAUNCH_SCRIPT"
echo ""
echo " To reset the throwaway vault between checklist runs:"
echo "   rm -rf $TEST_DIR && mkdir -p $TEST_DIR"
echo ""
echo " Checklists to run (Wayland first, then X11):"
if [[ -n "$DEB_PATH" ]] || [[ ! -d "$REPO_DIR" ]]; then
  echo "   Clone or copy the repo to find them under docs/tests/"
else
  echo "   $REPO_DIR/docs/tests/unlock-screen-test-checklist.md"
  echo "   $REPO_DIR/docs/tests/profiles-test-checklist.md"
  echo "   $REPO_DIR/docs/tests/import-test-checklist.md"
  echo "   $REPO_DIR/docs/tests/a11y-test-checklist.md"
  echo "   $REPO_DIR/docs/tests/shortcuts-test-checklist.md"
fi
echo ""
echo " To switch to X11: log out, click ⚙ at the login screen, pick 'Ubuntu on Xorg'."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

#!/usr/bin/env bash
# Build a .deb package for manual testing on a clean VM.
# Run from the repository root or from scripts/.
# Output: apps/desktop/target/debian/rahasyakosh_<version>_amd64.deb
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> Building desktop binary (release, locked)..."
cd apps/desktop
cargo build --release --locked

echo "==> Packaging as .deb..."
cargo deb --no-build

deb=$(ls -1t target/debian/rahasyakosh_*.deb | head -1)
echo ""
echo "Built: $(pwd)/$deb"
echo ""
echo "Copy to the VM, then inside the VM run:"
echo "  sudo apt install ./$deb"
echo ""
echo "Or copy the vm-setup.sh script to the VM and run it to build from source."

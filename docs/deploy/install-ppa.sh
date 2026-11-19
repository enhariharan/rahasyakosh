#!/usr/bin/env bash
# Install Rahasyakosh from the Launchpad PPA.
set -euo pipefail

sudo add-apt-repository ppa:enhariharan/rahasyakosh
sudo apt update
sudo apt install rahasyakosh

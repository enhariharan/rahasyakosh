# Rahasyakosh (रहस्यकोश)

Open-source (MIT), local-first, end-to-end-encrypted password manager in Rust.

**Design docs:** [`docs/`](docs/README.md).

**How to use it:** [quick start](docs/book/src/quick-start.md) for the command line, or the [desktop app](docs/book/src/desktop-app.md) (Linux).

**Status:** M0–M2 done; M3 in progress (session and a Linux desktop app done). **Not yet audited — do not store real secrets.**

```
cargo test --workspace
cargo run -p rahasyakosh-cli -- init      # binary name: rk
rk add GitHub -u alice --url github.com -g
rk get github --copy     # Copied to clipboard, cleared after 20s
```

## Desktop app (Linux)

Install the build libraries once:

```
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev \
                 libayatana-appindicator3-dev libsoup-3.0-dev
```

Build and run directly (no install):

```
cargo build --release --manifest-path apps/desktop/Cargo.toml
./apps/desktop/target/release/rahasyakosh-desktop
```

Or install for the current user (adds an app-grid entry and a `rahasyakosh` launcher):

```
./apps/desktop/install-local.sh           # installs to ~/.local/bin/
rahasyakosh                               # run it
./apps/desktop/install-local.sh --uninstall   # remove it
```

## Read the book

`docs/book/` is a plain-English guide to how Rahasyakosh works (security and cryptography included), written for newcomers and future
maintainers. It has a quick start and a glossary. It is an [mdBook](https://rust-lang.github.io/mdBook/):

```
cargo install mdbook               # once
mdbook serve docs/book --open      # read it in your browser (reloads when you edit)
mdbook build docs/book             # or build static HTML into target/book/ (open target/book/index.html)
```

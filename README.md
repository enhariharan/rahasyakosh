# Rahasyakosh (रहस्यकोश)

Open-source (MIT), local-first, end-to-end-encrypted password manager in Rust.

**Design docs:** [`docs/`](docs/README.md).

**How to use it:** [quick start](docs/book/src/quick-start.md).

**Status:** M0–M2 done (storage: SQLite, trash, history, search). **Not yet audited — do not store real secrets.**

```
cargo test --workspace
cargo run -p rahasyakosh-cli -- init      # binary name: rk
rk add GitHub -u alice --url github.com -g
rk get github --copy     # Copied to clipboard, cleared after 20s
```

## Read the book

`docs/book/` is a plain-English guide to how Rahasyakosh works (security and cryptography included), written for newcomers and future
maintainers. It has a quick start and a glossary. It is an [mdBook](https://rust-lang.github.io/mdBook/):

```
cargo install mdbook               # once
mdbook serve docs/book --open      # read it in your browser (reloads when you edit)
mdbook build docs/book             # or build static HTML into target/book/ (open target/book/index.html)
```

# Rahasyakosh (रहस्यकोश)

Open-source (MIT), local-first, end-to-end-encrypted password manager in Rust.
Design docs: [`docs/`](docs/README.md).

**Status:** M1 (core) + M1b (CLI) done. **Not yet audited — do not store real secrets.**

```
cargo test --workspace
cargo run -p rahasyakosh-cli -- init      # binary name: rk
rk add GitHub -u alice --url github.com -g
rk get github --copy     # to the clipboard, cleared after 20 s
```

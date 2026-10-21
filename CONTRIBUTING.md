# Contributing to Rahasyakosh

Thanks for helping. Rahasyakosh is a password manager, so correctness and security come before speed of delivery.

## Before you start
- Read `docs/` — especially `03-architecture.md` and `04-security-and-crypto.md`.
- Work is tracked as user stories (`US-XXX-NN`, see `docs/02-epics-and-user-stories.md`). Reference the story ID in
  commits, PRs and tests. For anything not covered by a story, open an issue first.
- **Security vulnerabilities:** do not open an issue or PR; follow `SECURITY.md`.

## Development
```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p rahasyakosh-core -p rahasyakosh-store -p rahasyakosh-strength -p rahasyakosh-transfer --target wasm32-unknown-unknown
cargo deny check
# optional: run core's tests (incl. the golden vault) under WebAssembly; needs wasmtime installed
rustup target add wasm32-wasip1
CARGO_TARGET_WASM32_WASIP1_RUNNER=wasmtime cargo test -p rahasyakosh-core -p rahasyakosh-store -p rahasyakosh-transfer --target wasm32-wasip1
```
CI runs the same checks. Minimum supported Rust version is set in `Cargo.toml` (`rust-version`).

## Rules for changes
- `rahasyakosh-core` does no I/O and must stay WASM-safe; inject RNG and clock.
- No new `unsafe` without discussion. No logging of secrets. Secret types must `zeroize` on drop.
- Do not add crypto primitives or dependencies casually; new dependencies must pass `cargo deny`.
- Do not roll your own crypto; use the vetted crates listed in the architecture doc.

## Definition of done
Acceptance criteria met · tests · docs updated · clippy/fmt clean · no new `unsafe` without review ·
security-relevant changes reviewed by a second maintainer (see `docs/06-roadmap.md`).

## Pull requests
- Keep PRs focused; one story or fix per PR.
- Write clear commit messages (imperative mood, story ID where relevant).
- By contributing you agree your work is licensed under the project's MIT licence.

All participation is subject to the [Code of Conduct](CODE_OF_CONDUCT.md).

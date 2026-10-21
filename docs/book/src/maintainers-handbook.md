# Maintainer's handbook

## Set-up

```
git clone git@github.com:enhariharan/rahasyakosh.git
cd rahasyakosh
rustup target add wasm32-unknown-unknown
cargo install cargo-deny cargo-audit mdbook
cargo test --workspace
```

## The pre-push checklist

```
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build -p rahasyakosh-core -p rahasyakosh-store -p rahasyakosh-app -p rahasyakosh-strength -p rahasyakosh-transfer --target wasm32-unknown-unknown
cargo deny --all-features check
cargo audit
```
CI also runs core's tests inside a WebAssembly runtime (wasmtime). To do the same locally, see `CONTRIBUTING.md`.
CI runs the same things; running them locally saves a round trip.

## Making a change

1. Find or create the user story (`docs/02-epics-and-user-stories.md`). Note its ID.
2. Write the test first where practical; for crypto, add a known-answer or property test.
3. Implement, keeping to the rules in [Writing code that handles secrets](safe-coding.md).
4. Update the docs, including this book if behaviour or explanations change.
5. Mention the story ID in the commit message. Open a PR; changes touching crypto or secret handling
   need a second reviewer.

## Never do these

- Invent your own cryptographic algorithm or protocol.
- Add a dependency without checking it with `cargo deny` and asking whether you truly need it.
- Print, log or put secrets in error messages.
- Reuse a nonce, or use a non-cryptographic random generator for anything secret.
- Compare secrets with `==`.
- Weaken the Argon2 minimums to make a test faster. Use the test-only parameters instead.
- Change the on-disk format without bumping the format version and adding a migration path.
- Add `unsafe` to `rahasyakosh-core`.

## Changing the vault format

Old vaults must still open. Increase `FORMAT_VERSION`, keep reading old versions, and add a test
that opens a saved old vault. Labels like `"rk/kek/v1"` exist so keys can be re-derived under a
new label without clashing.

## Reading list to grow into the role

Start with [Further reading](further-reading.md). A sensible order: this book → `docs/04` →
read `crypto.rs` and `vault.rs` end to end → read the tests → read one RFC (6238 is short).

## When in doubt

Ask. In security work, "I'm not sure this is safe" is a valuable thing to say in a review.
Report vulnerabilities privately through `SECURITY.md`, never in a public issue.

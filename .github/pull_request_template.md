## What and why
<!-- One or two sentences. Link the user story (for example US-IMP-02) or issue. -->

## Checklist
- [ ] Tests added or changed, and `cargo test --workspace` passes (and `cargo test` in `apps/desktop` if the app changed)
- [ ] `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings` are clean
- [ ] Docs updated in plain English (the book in `docs/book/src`, the glossary for new terms, the story status)
- [ ] `CHANGELOG.md` has a line under **Unreleased** for anything a user would notice
- [ ] No new `unsafe`; no secret is logged, printed or put in an error message
- [ ] New text in the desktop app has all seven languages (native review is welcome)
- [ ] Anything that parses untrusted input is covered by a fuzz target

# Rahasyakosh (रहस्यकोश) — Documentation

An open-source (MIT), local-first, end-to-end-encrypted password manager written in Rust.
Targets: Linux, macOS, Windows, Android, iOS (later), Browser (WASM + extension).

| # | Document | Purpose |
|---|----------|---------|
| 01 | [Vision & Scope](01-vision-and-scope.md) | Goals, non-goals, personas, principles |
| 02 | [Epics & User Stories](02-epics-and-user-stories.md) | Backlog with priorities and acceptance criteria |
| 03 | [Architecture](03-architecture.md) | Crate layout, platform strategy, storage, sync, ADRs |
| 04 | [Security & Crypto Design](04-security-and-crypto.md) | Threat model, key hierarchy, vault format |
| 05 | [UX Design](05-ux-design.md) | Information architecture, flows, screens, accessibility |
| 06 | [Roadmap & Implementation Plan](06-roadmap.md) | Milestones, testing, CI, release |
| 07 | [M2 Store Design](07-store-design.md) | Store trait, SQLite/directory backends, trash, history, search (draft) |
| 11 | [Next steps](11-next-steps.md) | Notes for resuming work: queue, decisions needed, known limits |
| 12 | [Publishing for Ubuntu](12-publishing-ubuntu.md) | Guidelines for releasing and packaging as an open-source Ubuntu app |
| — | [The Rahasyakosh Book](book/src/introduction.md) | Plain-English guide to security and cryptography for newcomers and maintainers (mdBook: `mdbook serve docs/book --open`) |

Status: **draft v0.1** — decisions marked *ADR* in 03 are open to revision until M2.

Manual test checklists are in [`tests/`](tests/): [accessibility](tests/a11y-test-checklist.md), [shortcuts](tests/shortcuts-test-checklist.md), [profiles](tests/profiles-test-checklist.md), [unlock screen](tests/unlock-screen-test-checklist.md) and [security audit](tests/security-audit-test-checklist.md).
Audit reports are in [`reports/`](reports/); the audit plan is [13-security-audit.md](13-security-audit.md).

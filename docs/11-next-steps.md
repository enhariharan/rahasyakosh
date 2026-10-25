# Next steps (notes for resuming)

Written 2026-10-25. State at the end of the day: `main` is at `9fbbd38`, CI green, nothing uncommitted. Profiles (E13) and export to text, CSV and JSON (E12) are done in the Linux desktop app.

## First thing tomorrow
1. **Run the unlock screen checklist**: [`tests/unlock-screen-test-checklist.md`](tests/unlock-screen-test-checklist.md) (about 15 min). The floating **Add a profile** button and Ctrl+Shift+N have been compiled and unit-tested but never seen in a running window. Paste results at the bottom of that file, and raise an issue for each `✘` or `?`.

## Work queue (recommended order)
2. **Profile rough edges** (small, fix before any release):
   - The app always starts on the first profile; remember the last one used.
   - A profile whose vault file is missing shows an error screen that blocks every other profile until restart; show the error and let the user pick another profile.
   - Duplicate full names give two identical drop-down entries. **Decision needed**: (a) refuse duplicate full names, or (b) show the username only when names clash.
   - No spoken (screen-reader) confirmation after deleting a profile.
   - Unused message `ProfileDeleted` in `apps/desktop/src/i18n.rs` (use it for the point above, or remove it).
3. **`rk --profile <username>`** in the CLI. US-PRF-04 is done in the desktop app only; the CLI still sees one vault. Reuse `ProfileList` from `crates/app/src/profiles.rs`.
4. **Desktop import screen.** Export has three formats in Settings; import is CLI-only (`rk import`).
5. **Native-speaker review of translations** (Hindi, Sanskrit, Tamil, Telugu, Malayalam, Urdu). The strings were written by the assistant. Needs a human reviewer, not code.
6. **Publishing to Ubuntu**: see [`12-publishing-ubuntu.md`](12-publishing-ubuntu.md). Items 2 and 1 should come before the first public release.

## Open items from earlier (see `06-roadmap.md`)
- macOS and Windows clipboard and packaging.
- System tray and global quick-search shortcut (not possible on Wayland without a portal).
- Orca / screen-reader check on other desktops and readers.
- Sync (M4) and an external security audit (the app is **not audited**; the README says not to store real secrets).

## Known limitations (told to the user, still true)
- Deleting a profile has no trash; it removes the vault file for good.
- Deleting `profiles.json` re-adopts only `vault.db` as "Default user"; other profiles' files stay on disk, unlisted.
- Synthetic clicks are ignored by WebKitGTK, so UI screens cannot be driven by script; they are verified by the model-layer tests plus manual checklists in `docs/tests/`.

## Reminders (from working agreements)
- Commit and push only when asked, as separate commits for code and docs; watch CI after pushing.
- Docs are plain English; update the book and glossary with each feature.

# Next steps (notes for resuming)

Written 2026-10-25, updated 2026-10-28. Profiles (E13) and export to text, CSV and JSON (E12) are done in the Linux desktop app. On 2026-10-28 the profile rough edges, `rk --profile`, and the import polish below were done (uncommitted at the time of writing; check `git log`).

## First thing tomorrow
1. ~~**Run the unlock screen checklist**~~ Done (see `b0b32d1` and `5180cc1`).

## Work queue (recommended order)
2. ~~**Profile rough edges**~~ Done 2026-10-28:
   - The app starts on the profile used last (`last_used` in `profiles.json`).
   - A profile whose vault file is missing or empty shows the problem under the profile picker; the other profiles stay reachable.
   - Duplicate full names: **decided (b)**, the list shows "Full name (username)" for the profiles that clash. Refusing duplicates would have broken lists that already exist.
   - Deleting a profile is announced to a screen reader ("Profile … was deleted."); `ProfileDeleted` is now used.
3. ~~**`rk --profile <username>`**~~ Done: `rk --profile USERNAME` (or `RK_PROFILE`) and `rk profiles`. The profile list is read from the folder of the vault file. The CLI cannot create profiles; the desktop app does.
4. ~~Desktop import screen.~~ Built, and polished 2026-10-28: a file chooser, a column picker (also when no password column is found), Bitwarden JSON, and recognition of Bitwarden, KeePassXC and 1Password CSV files. ~~Still open from US-IMP-02: KeePass `.kdbx` and 1Password `.1pux`.~~ Done (desktop wired 2026-11-15, CLI `rk import` wired 2026-11-16, error-message bugs fixed 2026-11-16).
5. ~~**Native-speaker review of translations**~~ Done 2026-11-10: Hindi, Sanskrit, Tamil, Telugu, Malayalam, Urdu reviewed by a human reviewer.
6. ~~**Publishing to Ubuntu**~~ Done through v0.1.0-alpha.4: snap on `edge` channel, GHA auto-publish workflow, Snap Store listing with icon/screenshots/metadata, sigstore signing.

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

# 05 — UX Design

## 1. UX principles
1. **Safe by default, fast by habit.** The common path (unlock → find → copy/fill) takes ≤3 interactions.
2. **Secrets are never on screen unless asked.**
3. **Same mental model everywhere**; layout adapts (3-pane desktop → 2-pane tablet → stack on phone).
4. **Plain language** for crypto concepts; no jargon in primary flows.
5. **Accessible**: keyboard-first on desktop, WCAG 2.2 AA, screen-reader labelled, scalable text.

## 2. Information architecture

```
Locked screen
└─ Unlocked
   ├─ Sidebar: All items · Favourites · Types (Login, Note, Card, ID) · Tags · Trash · Health
   ├─ Item list (search at top, sort, quick-copy actions)
   ├─ Item detail (view ⇄ edit)
   ├─ Generator (modal / sheet / standalone)
   └─ Settings: Security · Sync · Appearance · Import/Export · About
```

## 3. Key flows

### F1 First run / create vault
Welcome → "Create new vault" or "Open existing / import" → choose master password
(live strength meter, passphrase suggestion button, "this can't be recovered" notice with explicit
checkbox) → optional Secret Key & Emergency Kit (skippable, re-offerable) → vault ready, empty-state
with "Add first login" and "Import".

### F2 Unlock
Single password field, focus on open. Biometric button when enrolled. Failed attempt: shake +
"Incorrect password" (no hints). Show KDF progress spinner (≤1 s target).

### F3 Find & use a credential (desktop)
Global shortcut (default Ctrl/Cmd+Shift+Space) → quick-search palette → Enter copies password
(toast "Copied – clears in 20 s" with countdown) · Tab to username · ⌘U copies username · ⌘T copies TOTP.

### F4 Add login
"+" → type picker (default Login) → Title, Username, Password (generate button inline), URL,
TOTP (scan/paste), Notes, Tags. Save disabled until title or username present. Unsaved-changes guard.

### F5 Autofill (extension / Android)
Focus login field → inline key icon → popup lists matching items (domain-matched first) →
choose → fills. After submit on a new/changed credential: non-modal "Save to Rahasyakosh?" bar.
If locked: unlock prompt inline, then continue fill.

### F6 Generator
Mode toggle (Random / Passphrase), length slider with live preview, toggles (A-Z, a-z, 0-9,
symbols, avoid ambiguous), regenerate, copy, "Use this password". Strength bar below.

### F7 Sync setup
Settings → Sync → pick target (Folder / Server) → pair device (QR on first device, scan on second)
→ status chip in header: ● synced / ⟳ syncing / ⚠ conflict (tap to review; losing version restorable).

### F8 Import / Export
Choose source → file → preview table with mapping & duplicates → confirm → summary report.
Export: re-enter master password → strong warning for plaintext.

## 4. Screen wireframes (desktop, unlocked)

```
┌──────────────────────────────────────────────────────────────────────────┐
│ ☰  Rahasyakosh        🔍 Search (Ctrl+K)             ● synced   ⚙  🔒    │
├────────────┬───────────────────────────┬─────────────────────────────────┤
│ All items  │ ★ GitHub        alice     │  GitHub                [Edit]   │
│ Favourites │   Gmail         a@x.com   │  ───────────────────────────    │
│ ─ Types ─  │   Bank          alice01   │  Username   alice        [copy] │
│ Logins     │   Wi-Fi home    —         │  Password   ••••••••••   [👁][copy]│
│ Notes      │   …                       │  One-time   482 913  ◔ 18s [copy]│
│ Cards      │                           │  Website    github.com   [open] │
│ ─ Tags ─   │                           │  Notes      …                   │
│ work       │                           │  Updated 2 days ago · History   │
│ Trash      │ [+ New]                   │                                  │
└────────────┴───────────────────────────┴─────────────────────────────────┘
```
Mobile: bottom tab bar (Vault · Generator · Health · Settings); list → detail push navigation;
floating "+" button; swipe actions (copy/favourite).

## 5. Visual design
- Calm, neutral palette; one accent colour (teal-indigo). Light/dark/system. Tokens in `ui/theme`.
- Typography: system UI font; monospace with distinguishable 0/O, 1/l/I for passwords; digits grouped in TOTP.
- Colour-coded strength (never colour only—also text/icon).
- Motion: ≤150 ms, honours reduced-motion.
- Icons: Lucide set (ISC). Favicons fetched only if user opts in (privacy), otherwise letter avatars.

## 6. Components (shared in `rahasyakosh-ui`)
`SecretField` (masked/reveal/copy), `StrengthMeter`, `TotpBadge`, `ItemRow`, `TagChip`,
`SearchPalette`, `ConfirmDialog`, `Toast`, `SyncChip`, `EmptyState`.

## 7. Accessibility checklist
Logical focus order; visible focus ring; every icon button has a label; reveal state announced;
TOTP countdown not announced every second (announce on request); contrast ≥4.5:1; 44 px touch targets;
no time-limited actions without extension (clipboard timer can be changed).

## 8. Microcopy examples
- Master password: "This password unlocks everything. We can't reset it for you — there is no server that knows it."
- Copy toast: "Password copied. Clears in 20 s."
- Plaintext export: "This file will contain your passwords unencrypted. Anyone who opens it can read them."

## 9. Usability validation
Hallway tests of F1, F3, F4, F5 with 5 users per milestone; success = task done unaided, first-time
create-vault <3 min, find-and-copy <10 s. Track qualitative issues in GitHub.

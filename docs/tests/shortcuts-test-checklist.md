# Keyboard shortcut test checklist (US-A11Y-01, US-DSK-01)

A manual pass over every keyboard shortcut in the Linux desktop app. It takes about 15 minutes.
Run it with the screen reader **off** first (so you know the shortcut itself works), then repeat the
rows that failed with Orca **on** (Orca can take some keys for itself).

## Setup
- [ ] Throwaway vault with at least: one item with username, password, website and a TOTP secret; one pinned item; one item in the trash.
- [ ] Install the current build (`install-local.sh`), quit any older copy, and note the commit.
- [ ] Note the session type (`echo $XDG_SESSION_TYPE`), the keyboard layout (Settings → Keyboard), and whether Orca is on.
- [ ] Use the keyboard only for the test itself. Click only to put focus where a row says "focus in …".

## Where focus is matters
Shortcuts are caught by the app window. If a shortcut only fails in some places, that is the bug, so
the main table below is run from **three starting points**. Mark each cell:

- **List**: focus on an item row in the list (click a row, or `Tab` to it).
- **Search**: focus in the search box.
- **Blank**: you clicked an empty part of the window, so no control has focus.

`✔` works · `✘` does nothing · `?` does something else (say what).

**Cause of the first failures (2026-10-24):** keys pressed while nothing in the window has focus go to the page itself, outside the app's own key handler. That happens right after unlocking (the password box disappears with focus in it) and after clicking a list row (the list is redrawn). Clicking a top-bar tab put focus back inside, so shortcuts started working. The app now also catches keys sent to the page itself. The "Blank" column is the case that failed; also check:
- [ ] Right after unlocking, with no click: `Ctrl+2`, `Ctrl+3`, `Ctrl+K`, `Ctrl+N` work.
- [ ] Click an item in the list, then press `Ctrl+3`: it works without clicking a tab first.
- [ ] After pressing a shortcut that redraws the screen (for example `Ctrl+D`), the next shortcut still works.

## 1. Moving around (unlocked, no item needed)

| Shortcut | Expected | List | Search | Blank | Notes |
| -------- | -------- | :--: | :----: | :---: | ----- |
| `Ctrl+1` | Items page opens (the list) | | | | |
| `Ctrl+2` | Trash page opens | | | | |
| `Ctrl+3` | Settings page opens | | | | |
| `Ctrl+K` | Cursor jumps to the search box | | | | |
| `Ctrl+N` | New-item form opens, title field focused | | | | |
| `Ctrl+L` | Vault locks, unlock screen shown | | | | |
| `Esc` in the new-item form | Form closes, back to the list, nothing saved | | | | |
| `Esc` on Settings | Back to the list | | | | |
| `Esc` on Trash | Back to the list | | | | |

## 2. Acting on the open item
Open an item first (click a row or press `Enter` on it). Then:

| Shortcut | Expected | List | Search | Blank | Notes |
| -------- | -------- | :--: | :----: | :---: | ----- |
| `Ctrl+P` | Pin / unpin (the star appears or goes) | | | | |
| `Ctrl+E` | Edit form opens with the item's values | | | | |
| `Ctrl+D` | Item moves to the trash | | | | |
| `Ctrl+Shift+U` | Username copied (a "copied" message shows) | | | | |
| `Ctrl+Shift+C` | Password copied; clipboard clears after the set time | | | | |
| `Ctrl+Shift+W` | Website copied | | | | |
| `Ctrl+Shift+T` | One-time code copied (item with TOTP only) | | | | |
| `Ctrl+Shift+S` | Password shown; press again to hide | | | | |
| `Ctrl+Z` (within 8 s of `Ctrl+D`) | The trashed item comes back and is selected | | | | |

To see what was copied, paste somewhere harmless (`wl-paste` in a terminal).

## 3. Shortcuts that must do nothing
- [ ] With **no item open**, `Ctrl+P`, `Ctrl+E`, `Ctrl+D` and the `Ctrl+Shift` copy/show keys do nothing and show no error.
- [ ] With the vault **locked**, none of the shortcuts above work (try `Ctrl+N`, `Ctrl+2`, `Ctrl+K`).
- [ ] `Ctrl+Shift+P`, `Ctrl+Shift+E` and `Ctrl+Shift+D` do nothing (only the unshifted keys are used).
- [ ] Typing the same letters in a text field without Ctrl (for example the letters p, e, d, n, l, k in the search box) types them and triggers nothing.

## 4. Conflicts and side effects
- [ ] In the search box, `Ctrl+K` does not insert a character, and `Ctrl+A`, `Ctrl+C`, `Ctrl+V`, `Ctrl+X` still select, copy, paste and cut text.
- [ ] `Ctrl+D` does not open a browser bookmark or any system dialog.
- [ ] `Ctrl+Shift+C` and `Ctrl+Shift+S` do not open developer tools or a save dialog.
- [ ] `Ctrl+L` locks the vault (it must not move focus to a browser-style address bar).
- [ ] Holding a shortcut down does not repeat the action many times (`Ctrl+D` should trash one item, not several).
- [ ] The app does not lock on a shortcut press by accident: use `Ctrl+3`, wait, and check the idle timer restarted.

## 5. The hints in the app
- [ ] Hovering a top-bar button or item-page button shows its shortcut in the tooltip, for example "Lock (Ctrl+L)".
- [ ] The shortcut shown matches the one that works. Write down any mismatch.
- [ ] With Orca on, focusing a button announces its shortcut (for example "Control+L").
- [ ] With the display setting on **icons only**, and again on **text only**, the shortcuts still work.

## 6. Other layouts (optional)
Add a second keyboard layout (Settings → Keyboard → Input Sources) and repeat sections 1 and 2.
Shortcuts are matched on the letter or digit produced, so layouts such as French (digits need Shift)
or Dvorak may behave differently. Write down the layout and what failed.

## Recording results
Paste the filled tables into the pull request or the release notes, with the setup details (commit,
session type, layout, Orca on or off).

Every `✘` or `?` becomes an issue labelled `a11y` (or `desktop`). For each one note: the focus
starting point, whether Orca was on, and anything that appeared on screen. When every row passes on
the current build, update US-DSK-01 and US-A11Y-01 in `docs/02-epics-and-user-stories.md`.

** Test report - 2026-10-24 (Linux desktop app, Ubuntu, Wayland) **

| Section                               | Pass / fail | Notes                                                       |
| ------------------------------------- | ----------- | ----------------------------------------------------------- |
| 1 Moving around                       | Pass        | From a list row, the search box and with nothing focused    |
| 2 Acting on the open item             | Pass        | All three starting points                                   |
| 3 Shortcuts that must do nothing      | Pass        |                                                             |
| 4 Conflicts and side effects          | Pass        |                                                             |
| 5 The hints in the app                | Pass        |                                                             |
| 6 Other keyboard layouts (optional)   | Pass        |                                                             |

Earlier failures (`Ctrl+K`, `Ctrl+2`, `Ctrl+3` doing nothing) came from keys pressed with nothing focused not reaching the app's key handler; fixed by catching keys sent to the page itself.


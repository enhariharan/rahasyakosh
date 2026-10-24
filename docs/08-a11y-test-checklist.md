# 08 — Screen-reader and zoom test checklist (US-A11Y-01)

A manual pass for the Linux desktop app with the Orca screen reader, plus a 200% zoom check.
It takes about 20 minutes. Run it before each release and after big UI changes.

## Setup

- [ ] Use a throwaway vault with a few items (one with a TOTP secret, one pinned, one in the trash). Never test with a real vault.
- [ ] Install the current build (`install-local.sh`) and quit any older copy.
- [ ] **Start Orca before the app.** An app started before Orca may not be connected to it (typed characters are echoed, alerts are silent). Section 0b covers the other order.
- [ ] Turn Orca on with `Super+Alt+S` (or Settings → Accessibility → Screen Reader). Turn the speech rate up if it is slow.
- [ ] Note the session type (Wayland or X11; run `echo $XDG_SESSION_TYPE`), the Orca version (`orca --version`) and the app commit.
- [ ] Use the keyboard only. Do not touch the mouse. Keys: `Tab` / `Shift+Tab` move, `Enter` / `Space` activate, `Esc` closes.

## 0. Does Orca see the app at all?

- [ ] Orca announces the window title when the app opens or gains focus.
- [ ] Tabbing announces something for every stop. Silence means the accessibility tree is not exposed (record it as a blocker, then repeat once under X11).

## 0b. Orca started after the app (known issue)
Open the app first, then turn Orca on, then unlock.
- [ ] Record what happens (typed characters echoed? alerts silent?). On 2026-10-24 this was reproduced: with Orca started *after* the app, typed passphrase characters were announced and the wrong-password alert was not. Starting Orca first works. Treat it as a limitation of how the app connects to the accessibility service until it is fixed, and say so in the release notes.
- Already ruled out on the test machine: `gsettings get org.gnome.desktop.interface toolkit-accessibility` is `true`. `busctl --user get-property org.a11y.Bus /org/a11y/bus org.a11y.Status IsEnabled` is `false` with Orca off and `true` with Orca on. So the app starts while the accessibility service is off and does not attach when it comes on. No fix is known for the app itself; restart the app after starting Orca. `install-local.sh` now installs a launcher (`~/.local/bin/rahasyakosh`, used by the app-grid entry) that switches the service on before the app starts. Test that: with Orca off, open the app from the app grid, then turn Orca on, then unlock. Running `rahasyakosh-desktop` directly does not do this.

**What the experiments found (2026-10-24):** a reload does not help. What helps is a real change of keyboard focus *from one control to another*: a mouse click on Unlock moves focus to the button and back to the password box, and after that Orca reads the box correctly. Blurring and refocusing the same box was not enough. Pressing Enter does not move focus, so Orca kept echoing.

The app now listens for the desktop's "screen reader started or stopped" message (`apps/desktop/src/platform/linux.rs`) and shows a dialog (see below). Nudging the focus a few times worked once but was replaced by this.

- [ ] **On the unlock screen:** with Orca off, start the app, then start Orca. A dialog "Screen reader was enabled. Unlock the app to continue using it." appears with OK focused. Press Enter (or OK, or Esc): the dialog closes and the cursor is in the password box. Type a wrong passphrase: no echo, and the alert is spoken.
- [ ] **With the vault open:** unlock first, then start Orca. The dialog says the app will go back to the lock screen. OK locks the vault and shows the unlock screen; unlock and check the item list is read correctly.
- [ ] Quit Orca: the dialog appears again, worded "disabled".
- [ ] While the dialog is up, Tab stays on OK and shortcuts such as Ctrl+N do nothing.
- [ ] Run once as `RAHASYAKOSH_DEBUG=1 rahasyakosh-desktop` and check "Screen reader enabled" / "disabled" appear in the terminal.
- [ ] Open the app from the app-grid launcher with Orca off, then start Orca: note whether the dialog still appears (it should, because the launcher only switches on the general accessibility flag).

## 1. Welcome and create-passphrase pages

- [ ] The logo is read as the app name, once (not "image" or a file name).
- [ ] The tagline and description are read in order when you read the page (`Orca+Down` arrow, or flat review).
- [ ] The generated passphrase is readable word by word, and the one-time warning is read before or with it.
- [ ] The "I have written this down" tick box is announced as a check box with its state (checked / not checked).
- [ ] "Create vault" is announced as disabled until the tick is set, and nothing is lost when you tab past it.
- [ ] The copy button has a name ("Copy" or similar), not just "button".
- [ ] Focus starts on a sensible control, and moving to the second page puts focus at its top.

## 2. Unlock screen

- [ ] The password field is announced as "Enter master password" (or the translated text) plus "password edit", and typed characters are NOT read back.
- [ ] A wrong password is announced (error text read, or focus returned with the message).
- [ ] **After Alt+Tab (bug found 2026-10-24):** with Orca running, type a wrong passphrase but do NOT press Enter, Alt+Tab to another app and back, then type more. Expected: Orca says "Enter master password, password text" again when you come back, **typing works**, the characters are not echoed, and a wrong-password alert is spoken after Enter. The half-typed text stays in the box.
  *Cause, recorded with an AT-SPI event listener:* when the window comes back, WebKitGTK reports focus only for the web view as a whole, not for the password box, so Orca treated the web view as focused and echoed every key. The app now takes focus off the focused control and gives it straight back (in one step) 0.15 s after the window returns, while a screen reader runs; that makes WebKitGTK report the password box. An earlier attempt that moved focus to the Unlock button and back stopped typing working (it can leave Orca in browse mode, where letter keys are navigation commands): always check you can type after returning.
- [ ] **Alt+Tab with the vault open and Orca running:** the app locks when you leave it, even if "lock when the window loses focus" is off. Come back, unlock, and check the item list is read correctly.
- [ ] **Alt+Tab with Orca off:** nothing changes (the half-typed passphrase is still there, and the vault stays open unless the setting is on).
- [ ] The backoff countdown is not announced every second (it should be read once, or not spam).
- [ ] After unlock, focus lands on the item list or the search box, not on nothing.

## 3. Item list

- [ ] Search: the field has a name; as you type, the number of results (or the change) is reachable without a flood of speech.
- [ ] Each row reads as: title, username, and flags such as pinned / favourite. No "unlabelled" rows.
- [ ] `Up` / `Down` or `Tab` reaches every row. `Enter` opens the item.
- [ ] The floating "New item" bar is reachable by `Tab`, is announced with its shortcut (`Ctrl+N`), and does not cover the last row in a way that hides it from the reader.
- [ ] `Ctrl+N` works from the list.

## 4. Item page

- [ ] Each icon button is read by its name, with its shortcut where one exists: pin, edit, trash, copy, show.
- [ ] Nothing extra is read for the icons themselves. *What this means:* the small pictures inside the buttons (pin, edit, trash, copy, show) are decoration, because each button already has a text name. We mark them `aria-hidden` so Orca skips them. You should hear only the button name ("Copy password, button"), never "image", "graphic" or "svg". There is nothing to look at: just listen while tabbing through the buttons.
- [ ] **Reveal**: pressing Show reads "shown" / the state change, and the password is NOT read aloud on its own. Reading it is the user's choice.
- [ ] Hide again: the state change is announced and the password is no longer exposed.
- [ ] **Copy**: something confirms the copy ("Copied" or equivalent) without exposing the secret.
- [ ] The TOTP code: reachable, not re-announced every 30 seconds by itself, and the time left is available on demand.
- [ ] History and trash actions have names. Moving to trash is announced ("Moved to the trash. Press Ctrl+Z to undo.") and `Ctrl+Z` within 8 seconds brings the item back ("Restored."). There is no confirmation dialog by design.

## 5. Forms (new / edit item)

- [ ] Every field has a label read before its type ("Title, edit", "Password, password edit").
- [ ] Required fields and errors are announced, and focus goes to the first error.
- [ ] The password generator: *How to test:* in the new-item form, tab to the Generate button (it is next to the Password field) and press it. The button has a name ("Generate random password" or similar). The field then holds a new 20-character password. Orca should tell you the field changed, or you should be able to move back to the field and hear (or, with Show, read) that it now has a value. It is a single button: there are no length, word or symbol controls to test yet.
- [ ] The strength meter: *How to test:* type `abc` in the Password field (new-item form, or the create-passphrase page on first run). A line such as "Strength: Very weak, cracked in …" appears below the field. Orca should read it, or you should reach it with the review keys, without looking at the screen. Then type a long random password and check that the wording changes ("Strong"). If the meter is only a coloured bar with no words, or is not announced when it changes, this item fails.
- [ ] Save and Cancel are reachable at the end of the tab order. `Esc` cancels.

## 6. Settings

- [ ] Each setting (auto-lock time, colour scheme, icons or text, language) has a name and its current value is read.
- [ ] Changing the language: the app name, headings and window title change, and Orca keeps working (set the Orca language voice to match if you test Hindi, Tamil, etc.).
- [ ] **Urdu (right-to-left)**: the tab order follows the visual order, and `Left` / `Right` hints (if any) are not reversed wrongly.
- [ ] Colour scheme: light, dark and system all keep the focus ring visible.

## 7. Locking

- [ ] Manual lock and auto-lock: the user is told the vault locked (announcement or focus moved to the unlock screen).
- [ ] Nothing from the vault is still readable in review mode after locking.

## 8. Focus and keyboard (no screen reader needed)

- [ ] A visible focus ring is on every control, in both light and dark.
- [ ] No keyboard trap: you can always leave a dialog or field with `Tab` or `Esc`.
- [ ] The tab order matches the visual order on every page.

## 9. 200% zoom

Set the webview zoom to 200% (`Ctrl` + `+` until it reads 200%, or set Settings → Accessibility → Large Text and a 2× display scale). Check each page: welcome, create-passphrase, unlock, list, item, forms, settings.

- [ ] No text is cut off or overlaps.
- [ ] No horizontal scrolling is needed to use a page.
- [ ] Buttons and the floating "New item" bar stay reachable and do not cover content.
- [ ] Dialogs fit the window, or scroll inside it.
- [ ] Repeat once with the window at its smallest allowed size.

## Recording results

Copy the table below into the pull request or the release notes.

| Area                | Pass / fail | Notes (what Orca said, what you expected) |
| ------------------- | ----------- | ----------------------------------------- |
| 0 Orca sees the app |             |                                           |
| 1 Welcome / create  |             |                                           |
| 2 Unlock            |             |                                           |
| 3 List              |             |                                           |
| 4 Item page         |             |                                           |
| 5 Forms             |             |                                           |
| 6 Settings / RTL    |             |                                           |
| 7 Locking           |             |                                           |
| 8 Focus / keyboard  |             |                                           |
| 9 200% zoom         |             |                                           |

Any failure becomes an issue labelled `a11y`. When every area passes on the current build, update US-A11Y-01 in `docs/02-epics-and-user-stories.md` and the roadmap row for M3.

** Test report - 2026-10-24 09:31 **

| Area                | Pass / fail | Notes (what Orca said, what you expected)                                                                                                                                          |
| ------------------- | ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0 Orca sees the app | Pass        |                                                                                                                                                                                    |
| 1 Welcome / create  | Pass        |                                                                                                                                                                                    |
| 2 Unlock            | Fail        | (1) "Enter master password" is not announced; (2) "Password edit" is not announced; (3) CRITICAL - Typed passphrase characters are read back; (4) Wrong password is not announced; |
| 3 List              | Pass        |                                                                                                                                                                                    |
| 4 Item page         | Fail        | (1) State change is not announced; (2) What are the decorative icons? (3) No confirmation is asked by the app before thrashing. It is also not announced.                          |
| 5 Forms             | Fail        | (1) Required fields and errors are not announced, and focus does not go to the first error. (2) How to test password generator controls? (3) How to test strength meter?           |
| 6 Settings / RTL    | Pass        |                                                                                                                                                                                    |
| 7 Locking           | Fail        | Not announced.                                                                                                                                                                     |
| 8 Focus / keyboard  | Fail        | 'Esc' does not work. 'Tab' works as expected.                                                                                                                                      |
| 9 200% zoom         | Fail        | (1) Search bar is very very small in size. Even the cursor is also not visible; (2) "New Item" covers content;                                                                     |

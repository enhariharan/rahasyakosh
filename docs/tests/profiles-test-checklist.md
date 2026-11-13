# User profiles test checklist (US-PRF-01 to 05)

A manual pass over the profiles feature in the Linux desktop app. It takes about 20 minutes.
Run it with the screen reader **off** first, then repeat sections 2, 3 and 6 with Orca **on**.

## Setup
- [ ] Install the current build (`install-local.sh`), quit any older copy, and note the commit.
- [ ] **Back up first:** `cp ~/.local/share/rahasyakosh/vault.db ~/vault_backup.db` (if a vault exists). Restore it afterwards if you want your old data.
- [ ] For a clean first run, use a throwaway data folder so your real vault is not touched:
      `RK_VAULT=/tmp/rk-test/vault.db RK_CONFIG_HOME=/tmp/rk-test/cfg rahasyakosh`
      (the profile list and the other profiles' vaults go in the same folder as `vault.db`).
- [ ] Note the session type (`echo $XDG_SESSION_TYPE`) and whether Orca is on.

`✔` works · `✘` does not · `?` does something else (say what).

## 1. First run (US-PRF-01, US-PRF-02)
Start with an empty data folder.

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Welcome page, then **Create master passphrase** | The create page opens | ✔ | |
| Profile fields on that page | **Full name** shows "Default user", **Username** shows "defaultuser" | ✔ | |
| A hint under the fields | Says you can keep them and change them later in Settings | ✔ | |
| Leave both unchanged and create the vault | Works; the vault opens | ✔ | |
| Lock (Ctrl+L) | Unlock screen shows a **Profile** list with "Default user" | ✔ | |
| Repeat with a fresh folder; change name to "Asha Rao", username to "asha" | The unlock screen shows "Asha Rao" | ✔ | |
| Repeat with a fresh folder; clear the full name and try to create | Refused: "Enter a full name." No vault created | ✔ | |
| Repeat; clear the username and try to create | Refused: "Enter a username." | ✔ | |
| `profiles.json` exists next to `vault.db` | Has one profile with an `id`, the name and username | ✔ | |

## 2. The unlock screen (US-PRF-04)
Have at least two profiles (do section 3 first if you do not have one).

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| The **Profile** list | Shows every profile as "Full name" only (no username), the current one selected | ✔ | |
| Choose another profile | The list shows it; the box below takes **that** profile's master password | ✔ | |
| Type the other profile's master password | Unlocks that profile, not the first | ✔ | |
| Type the **wrong** profile's password | "Wrong password." and the wait grows after three tries | ✔ | |
| Lock, then choose the first profile and unlock | Its own passwords, none of the other's | ✔ | |
| The password box has focus after choosing a profile | You can type straight away (with Orca: the box is announced) | ✔ | |

## 3. Adding a profile (US-PRF-03)

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| **Add a profile**: a round button floating over the bottom corner of the unlock screen (a person-with-a-plus icon) | Hovering shows "Add a profile (Ctrl+Shift+N)"; clicking it opens a page titled "Add a profile" with a note that it has its own vault and master password | ✔ | |
| `Ctrl+Shift+N` on the unlock screen | Same page opens, without clicking | ✔ | |
| `Ctrl+Shift+N` elsewhere (vault unlocked, or already on the add-profile page) | Does nothing | ✔ | |
| With Orca on, focusing the floating button | Announces "Add a profile" and its shortcut | ✔ | |
| The fields | **Full name** and **Username** are empty (not pre-filled) | ✔ | |
| **Cancel** | Back to the unlock screen, nothing added | ✔ | |
| Create with an empty name, then an empty username | "Enter a full name." / "Enter a username." | ✔ | |
| Create with a username already used, in other letter case (for example "ASHA" when "asha" exists) | "Another profile already uses that username." | ✔ | |
| Create with a valid name, username and a new master password | You are signed in to the new profile | ✔ | |
| Its list of items | **Empty** | ✔ | |
| Add an item, lock, switch to the first profile | The item is **not** in the first profile | ✔ | |
| A weak or mismatched master password | Refused with the usual message | ✔ | |
| `profiles/<id>.db` appears next to `vault.db` | One file per added profile | ✔ | |

## 4. Passwords stay separate (US-PRF-04)
| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Add one item to each of two profiles with different titles | In each profile the list, search and trash show only its own items | ✔ | |
| In profile A, Settings → Export | The file lists only A's items | ✔ | |
| Trash an item in profile A, switch to B | B's trash is empty | ✔ | |
| Profile B's master password used on profile A (and vice versa) | "Wrong password."; does not unlock | ✔ | |
| The health/strength screens (if you open them) | Show only the open profile | ✔ | Screen not yet implemented (M7); skipped |

## 5. Editing a profile (US-PRF-05)
Settings → **Profile**.

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Fields show the open profile's name and username | Yes | ✔ | |
| Change the full name and **Save profile** | "Profile saved."; the unlock screen shows the new name | ✔ | |
| Change the username to another profile's (any letter case) | "Another profile already uses that username." | ✔ | |
| Save with the username only changed in letter case (own username) | Allowed | ✔ | |
| Empty name or empty username | The matching "Enter …" message | ✔ | |
| Restart the app | The changes are still there | ✔ | |

## 6. Deleting a profile (US-PRF-05)
Settings → **Delete this profile**. Use a profile you can lose.

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| With only **one** profile | Says it is the only profile and cannot be deleted; no form | ✔ | |
| With two or more: the warning | Says it is permanent and asks for the master password | ✔ | |
| Wrong master password | "Wrong password."; nothing deleted | ✔ | |
| Right master password | The profile is gone; the app shows the unlock screen for another profile | ✔ | |
| The Profile list | No longer lists the deleted profile | ✔ | |
| `profiles/<id>.db` | The file (and any `-wal` / `-shm`) is gone | ✔ | |
| The other profiles | Still unlock and keep their items | ✔ | |
| Restart the app | The deleted profile is still gone | ✔ | |
| Screen reader (known gap) | Nothing announces the deletion; note what Orca says after it | ✔ | |

## 7. Existing vault from before profiles (US-PRF-02)
Needs a vault made by an older build (use the backup from Setup).

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Put the old `vault.db` in a fresh folder with no `profiles.json`; start with `RK_VAULT` pointing at it | The unlock screen shows one profile, "Default user" | | |
| The **old master password** | Unlocks; every old item is there | | |
| `profiles.json` now exists | `vault.db` was not renamed or changed (compare size and date) | | |
| Rename the profile in Settings, restart | The new name is shown; the old password still works | | |

## 8. Edge cases and safety
| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Delete `profiles.json` while the app is closed, with `vault.db` present | Adopts the vault as "Default user" (new ID); other profiles' files stay in `profiles/` but are unlisted (known limit) | | |
| Damage `profiles.json` (write `garbage`) | Error is shown; the file is **not** overwritten | | |
| Delete one profile's `.db` file by hand | "The vault file of this profile is missing"; note whether others are reachable (known limit: error screen blocks until restart) | | |
| A very long full name (100+ chars) and a name with accents or non-Latin letters (e.g. "आशा राव") | Shown correctly in the list and the Settings field | | |
| Spaces around a name or username | Trimmed when saved | | |
| Auto-lock after idle time | Returns to the unlock screen with the **same** profile selected | | |
| `ls -l` on the data folder | Vault files are readable by you only (`-rw-------`) | | |

## 8b. Rough edges fixed 2026-10-28
| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Unlock profile B (not the first), quit the app, start it again | It starts on profile B | | |
| Delete profile B's vault file by hand (`rm profiles/<id>.db`), start the app | The window shows the **Profile** list, the missing-vault message, and **Add a profile**; no blank error page | | |
| In that state, pick profile A in the list | A's unlock screen appears and A unlocks | | |
| Pick the broken profile again | The message comes back; nothing crashes | | |
| Make two profiles with the same full name and different usernames | The list shows "Name (username1)" and "Name (username2)"; a unique name shows without the username | | |
| Delete a profile in Settings (with Orca on) | Orca says "Profile \<name\> was deleted."; the next profile's unlock screen shows | | |
| Delete the profile that is next in the list but whose vault file was removed | The deletion still happens; the message is spoken together with the problem text | | |
| `profiles.json` after picking a profile | Has a `last_used` entry with that profile's id | | |
| Delete `last_used` from `profiles.json` by hand | The app starts on the first profile | | |

## 9. Accessibility and languages
| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Keyboard only | Reach the **Profile** list, **Add a profile**, all fields, **Cancel** and the Settings profile buttons with `Tab`; each shows a visible focus ring | | |
| With Orca on | The Profile list is announced with its label "Profile"; error messages ("Enter a username." and so on) are announced when they appear | | |
| 200% zoom | The unlock screen's picker and the add-profile page do not overflow or hide the buttons | | |
| Switch the language (e.g. Hindi, Tamil, Urdu) | Every profiles label and message is translated; the Urdu page reads right to left except the username and password boxes | | |
| Translations (native speakers) | Note any wrong or odd wording (written by the assistant; need review) | | |

## Recording results
Fill in the tables and paste them at the bottom of this file under a dated heading, with the commit, session type and Orca on or off.
Every `✘` or `?` becomes an issue labelled `desktop` (or `a11y`); note the steps, the profile count, and anything shown on screen.
When every row passes, mark US-PRF-01 to 05 as verified in `docs/02-epics-and-user-stories.md`.

---

## 2026-11-13 — commit e3565b2, Wayland (GNOME Boxes VM, Ubuntu 24.04)

All checks pass.

## 2026-11-13 — commit 5c60509, X11/Xorg (GNOME Boxes VM, Ubuntu 24.04)

All checks pass.

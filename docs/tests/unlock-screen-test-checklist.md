# Unlock screen test checklist (US-VLT-02, US-VLT-04, US-PRF-04, US-A11Y-01)

A manual pass over the unlock screen of the Linux desktop app: the profile list, the master password box, the floating **Add a profile** button and the wait after wrong passwords. It takes about 15 minutes.
Run it with the screen reader **off** first, then repeat sections 1, 3 and 6 with Orca **on**.

## Setup
- [ ] Install the current build (`install-local.sh`), quit any older copy, and note the commit.
- [ ] Use a throwaway data folder so your real vault is not touched:
      `RK_VAULT=/tmp/rk-test/vault.db RK_CONFIG_HOME=/tmp/rk-test/cfg rahasyakosh`
- [ ] Create **two** profiles with different master passwords (for example "Asha Rao" and "Default user"), add one item to each, then lock with `Ctrl+L`. Also keep a run with only **one** profile for section 1.
- [ ] Note the session type (`echo $XDG_SESSION_TYPE`) and whether Orca is on.

`✔` works · `✘` does not · `?` does something else (say what).

## 1. Layout
| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| The **Profile** label and list | Sit directly above the password box, with the same left edge and the same width | ✔ | |
| The list entries | Show the full name only, no username | ✔ | |
| The password box | Has a label, and has focus when the screen appears (you can type at once) | ✔ | |
| With only one profile | The list still shows that one profile; unlocking works | ✔ | |
| The floating round button | Sits over the bottom corner (right; left in Urdu), does not scroll away, does not cover the password box or the unlock button, at normal size or at 200% zoom | ✔ | |
| Window narrowed to about 400 px | Nothing is cut off; the button still shows | ✔ | |
| Light and dark desktop theme | Text, list, box and button are readable in both | ✔ | |

## 2. Choosing a profile
| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Open the list with the mouse and pick the other profile | The list shows it; the password box is cleared and ready to type | ✔ | |
| Same with the keyboard (`Tab` to the list, arrow keys) | Works; the choice is kept when you `Tab` on | ✔ | |
| Type the chosen profile's master password and press `Enter` | Unlocks **that** profile, with only its items | ✔ | |
| Type the other profile's password for the chosen profile | "Wrong password."; stays on the unlock screen | ✔ | |
| Lock again (`Ctrl+L`) | The unlock screen comes back; type the first profile's password to confirm it opens the first | ✔ | |
| Auto-lock after the idle time | Returns to the unlock screen with the same profile selected | ✔ | |

## 3. Master password box and wrong passwords
| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Typing | Shows dots, never the letters | ✔ | |
| Press `Enter` on an empty box | Refused with a message; nothing happens to the vault | ✔ | |
| One or two wrong passwords | "Wrong password." each time, no wait | ✔ | |
| Third wrong password in a row | The app makes you wait (about 1 s), then about 2 s, 4 s and so on after each further failure | ✔ | |
| During the wait | The button is not usable or the app says to wait; a right password is not accepted until the wait ends | ✔ | |
| After the wait, the right password | Unlocks | ✔ | |
| After a good unlock and a new lock | The wrong-password count starts again | ✔ | |
| Paste into the box (`Ctrl+V`) | Works | ✔ | |
| Password with accents or non-Latin letters, if you have one | Unlocks when typed correctly | ✔ | |

## 4. The floating **Add a profile** button
| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Look | Round, a person with a plus sign, in the accent colour | ✔ | |
| Hover | Tooltip "Add a profile (Ctrl+Shift+N)" | ✔ | |
| Click it | Opens the page "Add a profile" with empty **Full name** and **Username** | ✔ | |
| `Ctrl+Shift+N` on the unlock screen, focus in the password box | Opens the same page | ✔ | |
| `Ctrl+Shift+N` right after the window opens, nothing focused | Opens the same page | ✔ | |
| `Ctrl+Shift+N` on the add-profile page | Does nothing | ✔ | |
| `Ctrl+Shift+N` with the vault unlocked | Does nothing (`Ctrl+N` is still **New item**) | ✔ | |
| **Cancel** on the add-profile page | Back to the unlock screen, no profile added, list unchanged | ✔ | |
| Keyboard `Tab` order | List, then password box, then the buttons, then the floating button; each shows a visible focus ring | ✔ | |
| `Enter` or `Space` on the focused floating button | Opens the add-profile page | ✔ | |

## 5. Errors on this screen
| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Delete one profile's `.db` file, choose that profile and unlock | "The vault file of this profile is missing"; others still reachable (known limit: error screen blocks until restart) | ✔ | |
| Damage `profiles.json` (write `garbage`) and start the app | An error is shown; the file is **not** overwritten | ✔ | |
| An error message on this screen | Shown in text, not only by colour | ✔ | |

## 6. Accessibility (Orca on)
| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| On arrival | Orca announces the password box by its label, not just "window" | ✔ | |
| The **Profile** list | Announced with its label and the selected full name | ✔ | Announced when explicitly focused (Tab), not on arrival — expected: password box has initial focus |
| After choosing another profile | Orca announces the choice; the password box can be typed in straight away | ✔ | |
| "Wrong password." | Announced when it appears | ✔ | |
| The floating button | Announced as "Add a profile" with its shortcut | ✔ | |
| While typing the master password | Orca does not read out the characters | ✔ | |
| Alt+Tab away and back | The focused box is announced again; typed keys are not echoed | ✔ | |
| 200% zoom | Nothing overlaps or is hidden; list, box and buttons can all be reached | ✔ | |
| Contrast | Floating button, list and messages look fine in both themes | ✔ | |

## 7. Languages
| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Switch language (e.g. Hindi, Tamil, Urdu) and lock | Labels, messages and button tooltip are translated | ✔ | |
| Urdu layout | Reads right to left; button in the left corner; password box excepted | ✔ | Translations were written by the assistant and need native-speaker review |

## Recording results
Fill in the tables and paste them at the bottom of this file under a dated heading, with the commit, session type and Orca on or off.
Every `✘` or `?` becomes an issue labelled `desktop` (or `a11y`); note the steps, the profile count, and anything shown on screen.

---

## 2026-11-07 — commit c04a0f4, Wayland, Orca on for sections 1/3/6

All checks pass. Notes:
- Section 6, Profile list announcement: announced when explicitly focused (Tab), not on arrival. Expected — password box holds initial focus. Not a defect.

## 2026-11-12 — commit 2b55bd3, Wayland (GNOME Boxes VM, Ubuntu 24.04)

All checks pass.

## 2026-11-13 — commit 5c60509, X11/Xorg (GNOME Boxes VM, Ubuntu 24.04)

All checks pass.

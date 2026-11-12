# First-start test checklist (US-PRF-01, US-PRF-02, US-SEC-01)

A manual pass over the first-start flow of the Linux desktop app: the welcome screen, the create-passphrase page (generated and own-password paths), the secret-key option, and the key sheet. It takes about 20 minutes.
Run it with the screen reader **off** first, then repeat sections 2 and 5 with Orca **on**.

## Setup
- [ ] Install the current build (`install-local.sh`), quit any older copy, and note the commit.
- [ ] Use a **clean** throwaway folder with no vault and no config:
      `rm -rf /tmp/rk-test && mkdir /tmp/rk-test && rk-test`
      (or: `RK_VAULT=/tmp/rk-test/vault.db RK_CONFIG_HOME=/tmp/rk-test/cfg rahasyakosh`)
- [ ] Note the session type (`echo $XDG_SESSION_TYPE`) and whether Orca is on.
- [ ] To reset between runs: `rm -rf /tmp/rk-test && mkdir /tmp/rk-test`

`✔` works · `✘` does not · `?` does something else (say what).

## 1. Welcome screen
| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Screen shown on first start | App logo, title, tagline (three lines about what it stores), language picker, one primary button | | |
| Primary button label | "Create master passphrase" (or equivalent in selected language) | | |
| Language picker on this screen | Changes the UI language; the button and tagline update immediately | | |
| Clicking the button | Moves to the create-passphrase page | | |

## 2. Create-passphrase page — generated passphrase (default path)
| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| **Full name** field | Pre-filled with "Default user"; editable | | |
| **Username** field | Pre-filled with "defaultuser"; editable | | |
| Hint under the username | Tells you these can be kept and changed later in Settings | | |
| Generated passphrase | Shown in a readable box; multiple words, not a password | | |
| "Shown once" warning | Present near the passphrase | | |
| Copy button | Copies the passphrase; icon changes to a checkmark briefly | | |
| "Generate another" button | Replaces the passphrase with a new one; "Written down" checkbox unchecks | | |
| "Written down" checkbox | Unchecked by default; "Create vault" button is disabled until it is checked | | |
| Check "Written down", then click "Create vault" | Vault opens; welcome page is gone | | |
| Lock (`Ctrl+L`) | Unlock screen shows a **Profile** list with "Default user" | | |

## 3. Create-passphrase page — validation
Reset the throwaway folder before each sub-case.

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Clear **Full name**, click "Create vault" | Refused: "Enter a full name." No vault created | | |
| Clear **Username**, click "Create vault" | Refused: "Enter a username." No vault created | | |
| Enter a custom full name ("Asha Rao") and username ("asha"), create | Unlock screen shows "Asha Rao" | | |
| `profiles.json` next to `vault.db` | Has one entry with an `id`, the name and the username | | |
| Vault location line at the bottom of the page | Shows the actual folder path | | |

## 4. Create-passphrase page — own-password path
Reset the throwaway folder.

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| "Choose own password" link | Hides the generated passphrase section; shows two password inputs | | |
| Typing in the first password box | A strength meter appears below | | |
| "Use generated" link | Switches back to the generated passphrase; generated passphrase is new | | |
| Switch to own, type a password, leave "Repeat it" empty, click "Create vault" | Refused (passwords must match); no vault created | | |
| Type matching passwords in both fields, click "Create vault" | Vault opens | | |
| Lock and unlock with that password | Works | | |

## 5. Secret key option
Reset the throwaway folder.

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Secret key checkbox | Unchecked by default; a hint explains what it is | | |
| Tick "Also generate a secret key", check "Written down", click "Create vault" | A key-sheet screen appears before the vault opens | | |
| Key sheet | Shows the secret key text; has a copy button | | |
| Copy button on the key sheet | Copies the key; icon changes briefly | | |
| "I have saved it" checkbox or button | Must be confirmed before you can continue | | |
| Continue | Vault opens | | |
| Lock and unlock | The app unlocks with the passphrase alone — the key is saved on this device so it is loaded automatically (no prompt). The key sheet said "saved to this device." | | |
| Lock and unlock on a **different device** (or after deleting the saved key file) | The app asks for both the passphrase and the secret key | | |
| Repeat without ticking the secret key option | No key sheet is shown; vault opens directly with passphrase only | | |

## 6. Accessibility (Orca on)
Restart with a clean folder.

| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Welcome screen on arrival | Orca announces the window and the focused element | | |
| "Create master passphrase" button | Announced by its label when focused | | |
| **Full name** field on the create page | Announced with its label; focus lands there on page load | | |
| **Username** field | Announced with its label | | |
| Generated passphrase | Readable by Orca when focused | | |
| Validation errors ("Enter a full name.") | Announced when they appear (role="alert") | | |
| "Written down" checkbox | Announced as a checkbox with its label | | |
| "Create vault" button when disabled | Announced as disabled | | |
| After the vault opens | Orca announces the main content area or the first focusable item | | |

## 7. Languages
| Check | Expected | Result | Notes |
| ----- | -------- | :----: | ----- |
| Switch to Hindi (or Tamil, Urdu) on the welcome screen, then create | All labels on the create page are in that language | | |
| Urdu layout on the create page | Right-to-left; username field stays LTR (dir="ltr") | | |

## Recording results
Fill in the tables and paste them at the bottom of this file under a dated heading, with the commit, session type and Orca on or off.
Every `✘` or `?` becomes an issue labelled `desktop`; note the exact steps and any on-screen message.

---

# Sessions: locking, hiding and clearing

The command-line tool unlocks, acts and exits, so keys never outlive a command. A desktop app is different: you open it once and use it for a while.
That makes it far more convenient, and far more dangerous, because the vault key now sits in memory for as long as the window is open. The **session**
(`crates/app/src/session.rs`) is the part of the program that decides *how long*, and what must happen when that time is up. It is the same code for every future front-end.

## The states

```
   ┌────────┐   correct password    ┌──────────┐
   │ Locked │ ────────────────────► │ Unlocked │
   │        │ ◄──────────────────── │          │
   └────────┘  lock button, or idle └──────────┘
```

- **Locked** holds only the *store*, the scrambled file. No keys, no decrypted items, no search index.
- **Unlocked** also holds the open vault. Locking throws the vault away, and the keys, items and index are overwritten with zeros as they go
  (see [Writing code that handles secrets](safe-coding.md)).

## Auto-lock (US-VLT-04)

The session locks itself after a period with no activity: 5 minutes by default, adjustable, or never. Two details matter:

- **Drawing the screen is not activity.** The window redraws every second to show the countdown; if that counted, the vault would never lock. Only a real action (a click, a key, a copy) resets the timer.
- **Using the vault after the time is up locks it instead.** Even if no timer happened to fire, the next action first checks the clock and, finding the time is up, refuses and locks.

An optional setting also locks as soon as the window loses focus.

## Friction on the unlock screen (US-SEC-04)

After three wrong passwords in a row, each further attempt must wait longer: 1 s, 2 s, 4 s, 8 s … up to 5 minutes. During the wait the password is **not even checked**, so guessing
faster does not help.

Be clear about what this protects. It slows someone *at your keyboard* guessing your password in the app. It does **nothing** against an attacker who copies the vault file
and guesses offline with their own computer: they never go through the app. That is the job of the deliberately slow key derivation (Argon2id), whose cost you can raise in
the settings (the *KDF memory* and *passes*, applied to a vault created or re-keyed from then on). Only a wrong password counts: other errors, such as a damaged file, do not feed the counter.

## Hidden by default (US-SEC-02)

Passwords, two-factor secrets and other concealed fields are shown as dots. **Reveal** shows one field for 10 seconds (adjustable, or until you hide it), then hides it again.
Locking forgets everything that was revealed, so unlocking again starts with everything hidden.

## The clipboard (US-SEC-01)

When you copy a password the session remembers *what* it copied and *when* it should be cleared (20 seconds by default). When the time comes, the app looks at the clipboard and clears it **only if
it still holds exactly that value**. If you copied something else in the meantime, your text is left alone. If the session locks first (manually or from inactivity), the copied secret is cleared at once.
This is the same rule the command-line tool follows; both use one shared clipboard crate.

## Settings (US-SET-01)

The settings hold no secrets, so they live in a plain file (`~/.config/rahasyakosh/settings.json`), though only you can read it. It is saved safely (a temporary file, then a rename, so a crash cannot leave half a file) and the previous version is kept as `settings.json.bak`. If the file cannot be read, the app tells you, uses the defaults and keeps the damaged file as `settings.json.damaged` instead of silently overwriting it. A settings file that anyone could edit could switch off the idle lock, so the file is not readable or writable by other users.

| Setting | Default | Meaning |
|---|---|---|
| Auto-lock | 5 minutes | Idle time before locking; 0 = never |
| Clear clipboard after | 20 s | 0 = never |
| Hide revealed secrets after | 10 s | 0 = until hidden by hand |
| Lock when the window loses focus | off | |
| Keep deleted items for | 30 days | Then they are deleted for good when you next unlock |
| History per item | 10 versions | |
| Key derivation memory / passes | 64 MiB / 3 | Higher is slower to guess offline, and slower to unlock |
| Colour scheme | same as the system | Light, dark or system |
| Language | English | English, Hindi, Sanskrit, Tamil, Telugu, Malayalam or Urdu (US-I18N-01) |

A settings file is treated as untrusted: unreadable content gives the defaults, missing options take defaults (so an old file keeps working when options are added), and every number is clamped to a sane range.
In particular, the key derivation can never be set below the floor the library enforces, so editing the file cannot weaken a vault.

## How it is tested

The session never reads the clock; every method takes the time as an argument. That makes it testable without waiting: a test can say "five minutes later" in one line. The 23 tests cover the idle boundary (locks at exactly the
timeout, not a second later), the backoff schedule and its cap, the refusal during a wait, a wrong-then-right password, reveal expiry and the wipe on lock, the clipboard rules (due once, replaced by a newer copy, handed back on lock),
and settings clamping. A property test runs random sequences of unlock, lock, wait and activity against a simple model of the state machine. As with the other crates, we then broke the code on purpose (idle lock one second late, no backoff, ignoring the lockout wait, keeping
revealed secrets after a lock, clearing a clipboard that had been replaced, not clamping the key-derivation floor) and checked that each break was caught.

## What it cannot do

- **It cannot stop the operating system from copying memory.** While unlocked, the key is in RAM, and the operating system may write memory to swap space or a crash dump.
  Locking wipes our copy, not the system's.
- **It cannot hide the screen from a screenshot or a camera**, and it cannot stop other programs you run from reading the clipboard during the 20 seconds.
- **It does not yet hear about system sleep or screen lock.** Auto-lock works from inactivity; reacting to the operating system's own lock event is planned.

# The clipboard and the trash

Two everyday features, and the security thinking behind each.

## Copying a password (US-SEC-01)

```
rk get github --copy                    # copy the password; it is cleared after 20 seconds
rk get github -f username --copy        # any field
rk totp github --copy                   # the current two-factor code
rk get github --copy --clear-after 60   # a longer wait; 0 means never clear
```

### Why not just print it?

Anything printed in a terminal stays in its scrollback, can be captured by screen recording, and can be seen
over your shoulder. Copying avoids all that: nothing is printed, you paste the password where you need it.

### Why it must be cleared

The clipboard is shared with every program you run, and some desktops keep a history of everything copied. A
password left there is available to anything that asks for it until you copy something else. So after a delay
(20 seconds by default) `rk` clears it.

Two details matter:

- **It only clears what it put there.** Before clearing, the helper reads the clipboard and compares it with the
  secret. If you have copied something else in the meantime, your new text is left alone. Clearing someone's
  unrelated work would be a bug.
- **Something has to stay alive to do it.** `rk get` returns straight away, so it starts a small helper process
  (`rk clipboard-clear`, a hidden command) that waits and then clears. The helper gets its own process group, so closing the terminal or
  pressing Ctrl-C does not stop it. If the helper cannot be started, the copy is **cancelled** and the clipboard
  emptied, rather than leaving a secret that nothing will clear.

### How it talks to the clipboard

Linux has no single clipboard API. On **Wayland** (the default on current Ubuntu) it uses the `wl-copy` and
`wl-paste` tools from the `wl-clipboard` package; on **X11** it uses `xclip` or `xsel`. If none is installed, `rk`
says which package to install and does **not** fall back to printing the password.

Using the desktop's own tools instead of a clipboard library means no extra code in a program that handles
secrets, and these tools already know how to keep a Linux clipboard alive. The secret reaches them through
**standard input**, never as a command-line argument, because arguments of running programs are visible to
other users in the process list.

### Limits you should know about

- A clipboard-history program (some desktops and add-ons have one) may keep a copy that `rk` cannot clear.
- Between copying and clearing, any program you run can read the clipboard. Twenty seconds is a compromise.
- The X11 paths (`xclip`, `xsel`) are written but have only been tested with a stand-in, not a real X11 session.
- macOS and Windows are not supported yet; the desktop app (M3) will use each platform's own clipboard API.

### How it is tested

The tests put fake `wl-copy` and `wl-paste` scripts on the `PATH`, backed by a file, so the real `rk` code (tool
detection, the stdin hand-off, the detached helper) runs without a desktop. They check that the secret is not
printed, is cleared after the delay, is *not* cleared if replaced, stays when the delay is 0, and that a missing tool
produces a helpful error and prints nothing.

## The trash

`rk rm github` does not destroy anything. It marks the item as deleted (inside the encrypted data) so you can change your mind.

```
rk rm github            # move to the trash; it disappears from `rk list`
rk trash                # see what is in the trash and how long ago it was deleted
rk restore github       # bring it back
rk rm github --purge    # delete it forever (works on trashed items too)
```

A few behaviours worth knowing:

- **Restore and purge look at the right set.** `restore` only considers trashed items. `rm --purge` considers both,
  preferring a live item over a trashed one with the same name.
- **A second `rk rm` explains itself** ("already in the trash") instead of claiming nothing matches.
- **The trash empties itself after 30 days.** Whenever you unlock the vault, items that have been in the trash for 30 days or more are deleted for good
  (US-ITM-03). It happens at unlock because the time of deletion is encrypted too, so only the unlocked program can read it. See
  [Where your data lives](storage.md).
- **Purging is permanent.** The record is removed from the vault file; copies in backups you made earlier still contain it.

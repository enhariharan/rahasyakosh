# Profile switching test checklist (US-PRF-01)

Manual smoke test for `rk profile use <username>` and `rk profile list`.
Requires the desktop app to have already created at least two profiles, or
the test vault setup below.

---

## Setup

Profiles are made in the desktop app. To test without it, create two profiles
manually (requires a development build):

```bash
# In a throwaway data directory
export RK_VAULT=/tmp/rk-profile-test/alice.db
mkdir -p /tmp/rk-profile-test

rk init --passphrase          # note the passphrase; this is Alice's vault
# Edit profiles.json by hand to add both profiles, OR use the desktop app
```

The simplest way is to open the desktop app, create two profiles ("alice" and
"bob"), then confirm `rk profiles` lists both.

---

## Automated tests

```
cargo test -p rahasyakosh-cli profile
```

Expected: `profile_use_sets_default_and_persists` and
`profile_use_unknown_username_gives_clear_error` both pass.

---

## 1 — List profiles

```bash
rk profiles         # original command — must still work
rk profile list     # new alias
```

- [ ] Both commands print the same output: one line per profile, tab-separated `username<TAB>display name`
- [ ] No error output

---

## 2 — Set the default profile

```bash
# Check which profile is currently the default (it is listed first after `rk profiles`)
rk profiles

# Switch to the second profile
rk profile use bob

# Confirm the default changed
rk profiles
```

- [ ] `rk profile use bob` prints: `Default profile set to "bob" (…)`
- [ ] Re-running `rk profiles` shows bob listed first (or the shell prompt reflects the change)
- [ ] `rk list` (no `--profile` flag) now opens bob's vault, not alice's

---

## 3 — Default persists across shell sessions

```bash
rk profile use bob
# Open a new terminal
rk list
```

- [ ] A fresh shell, without any `--profile` flag, opens bob's vault

---

## 4 — Override the default with --profile

```bash
rk profile use bob
rk --profile alice list
```

- [ ] `--profile alice` opens alice's vault even though bob is the default

---

## 5 — Unknown username gives a clear error

```bash
rk profile use nobody
```

- [ ] Exit code 1
- [ ] Error message names the unknown username and lists the known ones
- [ ] `profiles.json` is NOT modified (run `rk profiles` to confirm the default is unchanged)

---

## 6 — No profiles.json (single-vault install)

On a machine with no desktop app and no `profiles.json`:

```bash
rk profile use alice
```

- [ ] Error message explains there is no profile list and suggests `--vault`
- [ ] No crash or panic

---

## Recording results

| Check | Pass / fail | Notes |
|-------|-------------|-------|
| Automated tests | | |
| 1 List profiles | | |
| 2 Set default | | |
| 3 Persists across sessions | | |
| 4 --profile overrides default | | |
| 5 Unknown username error | | |
| 6 No profiles.json error | | |

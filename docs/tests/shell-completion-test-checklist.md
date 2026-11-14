# Shell completion test checklist (US-CLI-01, issue #3)

Verify that `rk completions <shell>` produces a working completion script for each supported shell.
Run the unit test first, then work through each shell you have installed.
The final snap check gates closing issue #3.

## Setup
- [ ] Build a fresh debug binary: `cargo build -p rahasyakosh-cli`
- [ ] Set an alias for the test: `alias rk="$(pwd)/target/debug/rk"`
- [ ] Note the commit (`git rev-parse --short HEAD`) and your login shell (`echo $SHELL`).

---

## 1 — Unit test

```
cargo test -p rahasyakosh-cli completions
```

- [ ] `completions_bash_is_nonempty` passes with no warnings.

---

## 2 — Bash

```bash
rk completions bash > /tmp/rk-completion.bash
source /tmp/rk-completion.bash
```

Press Tab after each command below (do **not** press Enter):

- [ ] `rk <Tab>` — lists all subcommands (`add`, `get`, `list`, `rm`, …)
- [ ] `rk get --<Tab>` — lists `--field`, `--copy`, `--clear-after` and `--help`
- [ ] `rk completions <Tab>` — offers `bash`, `zsh`, `fish`
- [ ] No error output appears on any of the above

---

## 3 — Zsh

```zsh
mkdir -p ~/.zfunc
rk completions zsh > ~/.zfunc/_rk

# If not already in ~/.zshrc, add once:
# fpath=(~/.zfunc $fpath)
# autoload -U compinit && compinit

# Reload in the current session:
autoload -U compinit && compinit -u
```

- [ ] `rk <Tab>` — lists all subcommands
- [ ] `rk get --<Tab>` — lists flags
- [ ] `rk completions <Tab>` — offers `bash`, `zsh`, `fish`
- [ ] No error output

---

## 4 — Fish (if installed)

```fish
rk completions fish > ~/.config/fish/completions/rk.fish
# Start a new fish session, then:
```

- [ ] `rk <Tab>` — lists all subcommands
- [ ] `rk get --<Tab>` — lists flags
- [ ] No error output

---

## 5 — Snap binary (do after the snap store revision containing this feature is published)

```bash
unalias rk 2>/dev/null || true   # use snap rk, not the dev build
rk completions bash > /tmp/rk-snap-completion.bash
source /tmp/rk-snap-completion.bash
rk <Tab>
rk get --<Tab>
```

- [ ] Completions work from the snap-installed binary
- [ ] `rk completions bash` prints output (non-empty) with no error

Close issue #3 once all boxes above are ticked and the snap revision is live.

---

## Recording results

Paste a summary below with: commit, OS, shell version, any failures.

| Shell | Pass / fail | Notes |
|-------|-------------|-------|
| Bash  |             |       |
| Zsh   |             |       |
| Fish  |             |       |
| Snap  |             |       |

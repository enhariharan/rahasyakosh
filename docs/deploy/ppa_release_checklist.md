# PPA Release Checklist

For every new release, the PPA workflow (`ppa.yml`) uploads signed source
packages automatically when a `v*.*.*` tag is pushed. This checklist covers
the manual steps that still apply and the one-time setup that must be done
before the first automated run.

---

## One-time setup (already done for this repo)

- [x] Launchpad account at `launchpad.net/~enhariharan`
- [x] PPA created: `ppa:enhariharan/rahasyakosh`
- [x] GPG key `0CA81C7A013EC6CD508B6B823AA5402D46C7F229` uploaded to `keyserver.ubuntu.com` and confirmed on Launchpad
- [x] GitHub secrets `PPA_GPG_KEY` and `PPA_GPG_PASSPHRASE` added to the repository
- [x] `apps/desktop/debian/` folder with `control`, `rules`, `changelog`, `source/format`
- [x] `ppa.yml` workflow committed; covers noble, oracular, plucky, questing, resolute

---

## Per-release steps

### 1. Push the release tag
```bash
git tag v0.1.0-alpha.5
git push origin v0.1.0-alpha.5
```
This triggers `ppa.yml` automatically.

### 2. Monitor the workflow
```bash
gh run list --workflow=ppa.yml --limit 3
gh run view <run-id> --log-failed
```
Or watch the build queue at:
`https://launchpad.net/~enhariharan/+archive/ubuntu/rahasyakosh/+builds`

### 3. Verify publishing
Check each series is published (status should change from `Pending` to `Published`):
```bash
for SERIES in noble oracular plucky questing resolute; do
  curl -s "https://api.launchpad.net/1.0/~enhariharan/+archive/ubuntu/rahasyakosh?\
ws.op=getPublishedBinaries&distro_arch_series=\
https://api.launchpad.net/1.0/ubuntu/${SERIES}/amd64" \
  | python3 -c "import sys,json; e=json.load(sys.stdin)['entries'][0]; \
    print('$SERIES:', e['binary_package_version'], e['status'])"
done
```

### 4. Smoke-test the install
On a clean VM or container for each series:
```bash
sudo add-apt-repository ppa:enhariharan/rahasyakosh
sudo apt update
sudo apt install rahasyakosh
rahasyakosh
sudo apt remove rahasyakosh
```

---

## Adding a new Ubuntu series

When a new Ubuntu series is released:
1. Add it to the `for SERIES in ...` list in `.github/workflows/ppa.yml`
2. Push any commit — the workflow only fires on tags, so the change takes
   effect on the next release tag
3. Optionally back-fill the new series manually using the script in `docs/deploy/`

---

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Workflow YAML parse error | `<<HEREDOC` treated as YAML merge key | Use `printf` instead (already fixed) |
| `gpg: signing failed` | Passphrase wrong or key not imported | Check `PPA_GPG_PASSPHRASE` secret; re-import key |
| `dput` rejected: duplicate version | Same version already in PPA for that series | Bump Debian revision (`1~noble1` → `2~noble1`) |
| PPA shows build but `apt` returns 404 | Launchpad publishing queue lag | Wait 20–30 min; check `Pending` → `Published` via API |
| Build fails on Launchpad | Missing runtime lib in Build-Depends | Add lib to `debian/control` Build-Depends, or override `dh_shlibdeps` |

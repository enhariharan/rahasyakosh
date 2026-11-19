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
- [x] `ppa.yml` workflow committed; covers noble (24.04 LTS), resolute (26.04 LTS), stonking (26.10)

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
for SERIES in noble resolute stonking; do
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

## Removing an end-of-life Ubuntu series

Ubuntu's in-between releases (for example 25.10) are supported for nine months;
the long-term ones (xx.04 in even years) for five years. Once a series reaches
its end of life, Launchpad marks it `Obsolete` and no longer accepts uploads
for it, so keeping it in the list only produces a failed upload on every
release. Before each release tag, list the series Launchpad still supports:
```bash
curl -s https://api.launchpad.net/devel/ubuntu/series | python3 -c "import sys,json; \
  [print(e['name'], e['version'], e['status']) for e in json.load(sys.stdin)['entries'] \
   if e['status'] in ('Supported', 'Current Stable Release')]"
```
and remove any series from the `for SERIES in ...` list in `ppa.yml` that is not
in it. Leave out series whose glibc is older than the one the runner builds
against (the binary would not start there): for now, jammy (22.04) and
everything older, which Launchpad still lists because of extended support.

---

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Workflow YAML parse error | `<<HEREDOC` treated as YAML merge key | Use `printf` instead (already fixed) |
| `gpg: signing failed` | Passphrase wrong or key not imported | Check `PPA_GPG_PASSPHRASE` secret; re-import key |
| `dput` rejected: duplicate version | Same version already in PPA for that series | Bump Debian revision (`1~noble1` → `2~noble1`) |
| `550 Requested action not taken: internal server error` from `dput` | Launchpad's FTP upload server failing, intermittently; it hit the 2nd or 3rd upload in a row for alpha.7 and alpha.8 (see Launchpad bug 2043679 for an earlier outage) | The workflow retries each series 3 times (pausing 1 and 2 min) and carries on with the others. A series that still fails is listed at the end of the job. It cannot be re-uploaded for the same version (see the next row); the next release tag uploads it. If it keeps happening, switch `dput` to SFTP (needs an SSH key on Launchpad and as a GitHub secret) |
| Re-running a failed PPA job, or a hand upload, is rejected: `.orig.tar.gz` "already exists ... different contents" | Every run rebuilds the binary, so the orig tarball differs from the one Launchpad already holds for that version | Don't re-run for a version that reached Launchpad for any series; fix and tag the next version instead |
| Upload rejected for one series by e-mail | Series is end-of-life (`Obsolete` on Launchpad) | Remove it from `ppa.yml`; see "Removing an end-of-life Ubuntu series" |
| `unmet build dependencies: build-essential:native` | Runner image lacks `build-essential` | Install it in the "System libraries" step (already fixed) |
| PPA shows build but `apt` returns 404 | Launchpad publishing queue lag | Wait 20–30 min; check `Pending` → `Published` via API |
| Build fails on Launchpad | Missing runtime lib in Build-Depends | Add lib to `debian/control` Build-Depends, or override `dh_shlibdeps` |

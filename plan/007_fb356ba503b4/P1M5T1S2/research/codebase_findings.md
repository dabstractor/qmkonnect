# Codebase & External Findings — P1.M5.T1.S2 (Homebrew tap + Scoop bucket CI jobs)

Verified this session by reading the actual files. Every fact below is grep/read-confirmed.

## 1. The file being edited + existing job inventory

`.github/workflows/release.yml` (11.6 KB, 5 jobs today):
- `macos` — builds DMG, uploads artifact `macos-dmg` → `QMKonnect-<ver>-macos.dmg`.
- `windows` — builds Inno exe, uploads artifact `windows-exe` → `QMKonnect-<ver>-windows-x64.exe`.
- `linux-binary` — `qmkonnect-<ver>-linux-x86_64.tar.gz`.
- `arch` — `.pkg.tar.zst`.
- `publish` — `needs: [macos, windows, linux-binary, arch]`, `if: github.event_name == 'push'`,
  `permissions: contents: write`, uses `softprops/action-gh-release@v2` to create the GitHub Release
  and attach ALL four artifact globs. **This is the dependency** — its assets (DMG + exe) must be live
  before the download step.
- (P1.M5.T1.S1, in parallel) appends an `aur` job after `publish`.

Top-level: `permissions: contents: read` (line ~24). My two jobs push to EXTERNAL repos over SSH
deploy keys — they do NOT use the GITHUB_TOKEN for the push → they need NO `permissions` escalation.
Mirror the aur job: NO `permissions:` line on the new jobs (inherit top-level `contents: read`).

The trigger is `on: push: tags: - 'v*'` → every `push` event here is a `v<ver>` tag push. So
`github.ref_name` (= `GITHUB_REF_NAME`) is always `v<ver>`, and `${GITHUB_REF_NAME#v}` is the bare
version. This is the cleanest version source for build-less jobs (no Rust toolchain needed). All
other jobs use `cargo metadata | jq`, but they BUILD Rust; these two jobs don't.

## 2. The two INPUT scripts (P1.M2.T1.S2 / P1.M3.T1.S2, both Complete) — CONTRACTS

### `packaging/homebrew/update-cask.sh` (bash)
- Usage: `./update-cask.sh <version> [<sha256>]`. `--help` prints the header.
- **With 2 args**: uses the given SHA256 → **skips its own download**. Patches `version` + `sha256`
  stanzas in `$SCRIPT_DIR/Casks/qmkonnect.rb` (BSD+GNU-sed portable).
- **With 1 arg**: downloads `https://github.com/dabstractor/qmkonnect/releases/download/v<ver>/QMKonnect-<ver>-macos.dmg`,
  hashes with `shasum -a 256` (falls back to `sha256sum`).
- Rejects a leading `v` in the version arg. Validates SHA256 = 64 lowercase hex.
- Best-effort `brew audit --cask --new-cask` only if `brew` on PATH (NOT on ubuntu → skipped).
- **PURE local file update — does NOT push.** Its header documents the CI flow verbatim:
  "CI loads it into ssh-agent, then: git clone git@github.com:dabstractor/homebrew-qmkonnect.git,
  run this script, cp Casks/qmkonnect.rb into the clone, commit, push."
- The contract says the JOB computes the SHA256 (download DMG + `sha256sum`) and passes it as arg 2.
  → exactly ONE download total. update-cask.sh then patches `packaging/homebrew/Casks/qmkonnect.rb`
  in the SOURCE checkout. The job cp's that into the tap clone.

### `packaging/scoop/update-manifest.ps1` (PowerShell, cross-platform pwsh)
- Usage: `./update-manifest.ps1 -Version <ver> [-Sha256 <sha>]`. `-Help` prints header.
- **With -Sha256**: skips its own download. Regex-patches top-level `version` + concrete
  `architecture.64bit.url` + `architecture.64bit.hash` in `$PSScriptRoot/qmkonnect.json`,
  LEAVING the `autoupdate.$version` URL template untouched. Re-parses JSON to validate.
- **Without -Sha256**: downloads
  `https://github.com/dabstractor/qmkonnect/releases/download/v<ver>/QMKonnect-<ver>-windows-x64.exe`,
  `Get-FileHash -Algorithm SHA256`.
- Rejects leading `v`. Validates SHA256 = 64 lowercase hex.
- Best-effort `scoop checkver` only if `scoop` on PATH (NOT on ubuntu → skipped). The script's own
  always-on check is the JSON re-parse + Select-String confirmation.
- **PURE local file update — does NOT push.** Header documents CI flow verbatim:
  "git clone git@github.com:dabstractor/scoop-qmkonnect.git, run this script, cp qmkonnect.json into
  the clone as bucket/qmkonnect.json, commit, push."
- pwsh 7+ is PREINSTALLED on ubuntu-latest → `shell: pwsh` works; the script's author explicitly
  made it cross-platform ("PowerShell 5.1 (Windows) or 7+ (pwsh, cross-platform — GitHub Actions
  ubuntu/windows)").

### Neither script sets a git identity
The clone/commit/push is done by the CI JOB (not the scripts). → the JOB must run
`git config user.email/user.name` in the tap/bucket clone before `git commit` (else "Author identity
unknown"). (Contrast: the AUR `publish.sh` does its OWN commit inside the script and still needs the
job to pre-set an identity. Here the job owns the commit entirely.)

## 3. External repo layouts (from tap-README.md / bucket-README.md — authoritative)

| | Homebrew tap | Scoop bucket |
|---|---|---|
| Repo | `dabstractor/homebrew-qmkonnect` | `dabstractor/scoop-qmkonnect` |
| SSH clone | `git@github.com:dabstractor/homebrew-qmkonnect.git` | `git@github.com:dabstractor/scoop-qmkonnect.git` |
| File in repo | `Casks/qmkonnect.rb` | `bucket/qmkonnect.json` |
| Source-repo file | `packaging/homebrew/Casks/qmkonnect.rb` | `packaging/scoop/qmkonnect.json` |
| Secret | `HOMEBREW_TAP_DEPLOY_KEY` | `SCOOP_BUCKET_DEPLOY_KEY` |
| Auth | GitHub deploy key (SSH, **write** access) | GitHub deploy key (SSH, **write** access) |

Both READMEs state the deploy key needs "Allow write access" checked when adding the public half.
Public half → repo Settings → Deploy keys. Private half → Actions secret in `dabstractor/qmkonnect`.

## 4. CRITICAL EXTERNAL CORRECTION — the SSH-agent action

The bucket-README (P1.M3.T1.S2) references `webfactory/agents/github-ssh-agent@v0.9.0`.
**That reference is WRONG** — it would fail CI ("unable to resolve action `webfactory/agents/...`").
- Correct repo: **`webfactory/ssh-agent`** (confirmed via GitHub Marketplace +
  `api.github.com/repos/webfactory/ssh-agent`).
- Reference: `webfactory/ssh-agent@v0.9.0` (stable, proven, tens of millions of uses) or
  `webfactory/ssh-agent@v0.10.0` (latest — "Upgrade to node-24"; functionally identical for our use).
- The action loads the key into `ssh-agent` AND **adds `github.com` to `~/.ssh/known_hosts`
  by default** (built-in GitHub host-key list). → `git clone git@github.com:...` works
  non-interactively with no extra ssh-keyscan step. Input: `ssh-private-key: ${{ secrets.<KEY> }}`.
- This is the canonical GitHub-deploy-key solution; cleaner than the raw `~/.ssh`+`ssh-keyscan`
  setup the aur job uses (aur pushes to a NON-GitHub host, aur.archlinux.org, where webfactory's
  built-in github.com known_hosts don't help — that's why the aur job goes raw).

## 5. Why `needs: [publish]` (not the build jobs)

The download step fetches `QMKonnect-<ver>-macos.dmg` / `-windows-x64.exe` from the GITHUB RELEASE
(`github.com/dabstractor/qmkonnect/releases/download/v<ver>/...`). Those assets exist only AFTER the
`publish` job attaches them. `workflow_dispatch` dry-runs don't publish → the download 404s → hence
`if: github.event_name == 'push'` (same gate as `publish` and the aur job). Don't depend on
`[macos]`/`[windows]` directly — those upload workflow ARTIFACTS, not RELEASE assets.

## 6. Version extraction — recommend `github.ref_name`, not cargo metadata

These jobs build NO Rust. Installing `dtolnay/rust-toolchain@stable` just to read a version is
~30s of waste. On a `v*` tag push, `GITHUB_REF_NAME` is the tag (`v0.2.8`); `${GITHUB_REF_NAME#v}`
is the bare version (`0.2.8`) both scripts want. cargo-release cuts the tag from the Cargo.toml
version, so this is transitively the Cargo.toml source-of-truth. (All build jobs use `cargo metadata
| jq` because they already have the toolchain; here it's pure overhead.) Use ref_name; note
`cargo metadata | jq` as the consistency alternative.

## 7. Parallel coordination with P1.M5.T1.S1 (aur job) — zero logical conflict

Both PRPs append jobs to release.yml after `publish`. They are INDEPENDENT siblings (each
`needs: [publish]`; none depends on another). Append order among aur/homebrew-tap/scoop-bucket is
functionally irrelevant. Safe placement: append both at the very END of the file. If the aur job is
already present when this lands, append after it; if not, append after `publish`. Either is correct.

## 8. Scope boundary (forbidden)

- Edit ONLY `.github/workflows/release.yml`. No Rust, no Cargo.toml, no packaging/ (the two scripts
  are INPUT, unmodified), no docs/*, no PRD.md/tasks.json/prd_snapshot.md.
- Do NOT add Winget/Nix/asdf jobs (P1.M5.T2.* — separate work items).
- Do NOT modify `update-cask.sh` / `update-manifest.ps1`.
- The two secrets (`HOMEBREW_TAP_DEPLOY_KEY`, `SCOOP_BUCKET_DEPLOY_KEY`) are DOCUMENTED inline
  (Mode A ride-along comment blocks), not created by the workflow.
# homebrew-qmkonnect — Homebrew tap for QMKonnect

Custom Homebrew tap for [QMKonnect](https://github.com/dabstractor/qmkonnect),
distributing the macOS **Cask** until the released DMG is Developer-ID-signed +
notarized and graduates to the official [`Homebrew/homebrew-cask`](https://github.com/Homebrew/homebrew-cask)
repo (PRD §12).

## What this is

This repository is a Homebrew **tap** — a git repo named `homebrew-<name>` that
holds [`Casks/qmkonnect.rb`](Casks/qmkonnect.rb). The QMKonnect `.app` ships as a
universal `.dmg` (one cask covers Apple Silicon + Intel); this tap is the macOS
community channel alongside the primary DMG download (PRD §4 F15, §5). Homebrew is
inherently **per-user**, so no system-wide install or `sudo` is involved.

Per PRD §12 / `architecture/external_deps.md` §2: *"Homebrew ships via a custom
tap until notarization qualifies it for the official cask."* This repo is that
custom tap.

## Install

```bash
# The tap ALIAS is mulletware/qmkonnect (the app's bundle id is io.mulletware.*)
# but the tap REPO lives in the dabstractor org — so the explicit URL is REQUIRED.
# Without it `brew` would look for github.com/mulletware/homebrew-qmkonnect (wrong repo).
brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect
brew install --cask qmkonnect

# If the build is unnotarized and Gatekeeper blocks the first launch:
brew install --cask --no-quarantine qmkonnect
```

Upgrading is automatic — the cask's `livecheck` (`strategy :header`) probes the
`releases/latest` page for new `v<version>` tags:

```bash
brew upgrade --cask qmkonnect
```

## What it installs

| Path | What |
|---|---|
| `/Applications/QMKonnect.app` | The menu-bar/tray app (universal binary) |
| `~/Library/Application Support/QMKonnect/config.toml` | Per-user device filter (VID/PID, debounce, poll interval) |
| `~/Library/Application Support/QMKonnect/rules.toml` | Per-user callback-name → command rules |

The app **auto-starts at login** via `SMAppService` (default on). On first launch
it requests **Screen Recording** permission (needed to read window *titles*; the
app runs without it but sends only app names).

## Uninstall

```bash
brew uninstall --cask qmkonnect

# Also remove per-user config/data (the cask's `zap` stanza):
brew uninstall --cask --zap qmkonnect
```

## For maintainers — updating the cask

The cask ships with `version "0.2.8"` and a `sha256 :no_check` placeholder. Each
tagged release patches both fields and pushes to this tap. To validate a cask
locally:

```bash
# From a clone of THIS tap repo:
ruby -c Casks/qmkonnect.rb                        # syntax check (any host with ruby)
brew audit --cask --new-cask Casks/qmkonnect.rb   # DSL / stanza-order (macOS or Linuxbrew)
# --strict + the real sha256 are only provable here AFTER CI fills the hash from a published release.
```

The mechanical update is driven from the **source repo** by
[`packaging/homebrew/update-cask.sh`](https://github.com/dabstractor/qmkonnect/blob/master/packaging/homebrew/update-cask.sh):
it downloads the release DMG, computes its SHA256 (`shasum -a 256` / `sha256sum`),
patches the cask's `version` + `sha256` lines (BSD/GNU-sed portable), and runs a
best-effort `brew audit --cask --new-cask`.

```bash
# From a clone of dabstractor/qmkonnect:
./packaging/homebrew/update-cask.sh 0.2.8                                   # download + hash + patch + audit
./packaging/homebrew/update-cask.sh 0.2.8 <precomputed-sha256>              # skip the download
```

## CI publishing (deploy key)

New releases are pushed to this tap **automatically** by the QMKonnect release
workflow (wired in P1.M5.T1.S2). It authenticates to this repo with a GitHub
**deploy key** (SSH, write access):

1. Generate an SSH key pair.
2. Add the **public** half to this tap repo: *Settings → Deploy keys* (check
   "Allow write access").
3. Store the **private** half as the `HOMEBREW_TAP_DEPLOY_KEY` Actions secret in
   [`dabstractor/qmkonnect`](https://github.com/dabstractor/qmkonnect).

On a tag, CI loads that key into `ssh-agent`, clones
`git@github.com:dabstractor/homebrew-qmkonnect.git`, runs `update-cask.sh`,
copies the patched `Casks/qmkonnect.rb` in, commits, and pushes. This mirrors the
AUR SSH-key model (see `architecture/external_deps.md` §"CI Publishing Strategy"
and `packaging/linux/aur/publish.sh`).

## Path to the official cask

Once the released DMG is Developer-ID-signed + notarized (PRD §12), this cask
graduates to the official `Homebrew/homebrew-cask` repo. The `version` / `url` /
`sha256` / `livecheck` stanzas carry over unchanged — only the distribution
channel changes (and the `--no-quarantine` workaround is no longer needed, since
a notarized build passes Gatekeeper natively).

## See also

- **Source repo:** <https://github.com/dabstractor/qmkonnect>
- **Install docs:** [`docs/installation.md`](https://github.com/dabstractor/qmkonnect/blob/master/docs/installation.md) (macOS section)
- **macOS build scripts:** [`packaging/macos/`](https://github.com/dabstractor/qmkonnect/tree/master/packaging/macos) (`clean.sh` / `build.sh` / `install.sh`)
- **Cask Cookbook (stanza reference):** <https://docs.brew.sh/Cask-Cookbook>
# Research Notes — P1.M4.T1.S1: asdf plugin scripts (list-all / download / install + utils.bash)

## What this task is
Create the **asdf plugin scripts** that download + install QMKonnect from its
GitHub Releases, plus a shared `lib/utils.bash` and the plugin repo README. The
SAME scripts serve **mise** (mise is asdf-compatible: it runs the plugin's
`bin/*` scripts unchanged — `architecture/external_deps.md` §6-7). This is the
P1.M4 cross-platform-version-manager channel of F15 (PRD §4).

## Ground-truth facts (all verified from source this session)

| Fact | Value | Source |
|---|---|---|
| GitHub org / repo | `dabstractor/qmkonnect` | `git remote get-url origin` |
| Version (bare) | `0.2.8` | `Cargo.toml` (`version = "0.2.8"`); tag is `v0.2.8` |
| macOS asset | `QMKonnect-{ver}-macos.dmg` (universal2: arm64+x86_64) | `release.yml:84` (`mv QMKonnect.dmg …`) |
| Windows asset | `QMKonnect-{ver}-windows-x64.exe` (Inno INSTALLER) | `release.yml:130` |
| Linux asset | `qmkonnect-{ver}-linux-x86_64.tar.gz` | `release.yml:178` |
| Linux tarball layout | top-level `qmkonnect-{ver}-linux-x86_64/` containing `qmkonnect`, `qmkonnect-hid-id`, `udev/69-qmkonnect-rawhid.rules`, `systemd/qmkonnect.service.template` | `release.yml:172-178` + `aur/PKGBUILD` |
| Release URL pattern | `https://github.com/dabstractor/qmkonnect/releases/download/v{ver}/{asset}` | AUR PKGBUILD + Scoop + Homebrew cask (all identical) |
| Releases API | `https://api.github.com/repos/dabstractor/qmkonnect/releases` → JSON, `"tag_name":"v0.2.8"` | external_deps §"Version Source of Truth" |
| Real SHA256 (0.2.8 Linux tarball) | `86dcaa57254f4b7dd23595ef8e473432c3997df3b6c12621913d215b9ab1b216` | `aur/PKGBUILD` |
| .sha256 sidecar? | **NONE** (Scoop hash is all-zeros placeholder; AUR hardcodes; Homebrew `:no_check`; Winget 64-zero) → the download script must NOT invent a sidecar URL; "validate if sidecar exists" ⇒ probe `${url}.sha256`, validate on 200, skip on 404 | grep across all packaging |
| Two binaries shipped | `qmkonnect` (main) + `qmkonnect-hid-id` (udev helper, `src/bin/hid_id.rs`, `Cargo.toml:132`) | Cargo.toml |
| `packaging/asdf/` exists? | **NO** (greenfield) | `ls packaging/asdf` → ENOENT |
| Tool/plugin name | `qmkonnect` (the contract's mise example `mise plugin add qmkonnet …` is a TYPO — missing 'c'; use `qmkonnect` everywhere) | — |
| Plugin repo | `asdf-qmkonnect` (asdf convention `asdf-{toolname}`), URL `https://github.com/dabstractor/asdf-qmkonnect` | external_deps §7 |

## asdf plugin contract (verified: asdf-vm.com/plugins/create.html + asdf source)

- A plugin = a git repo `asdf-{toolname}` with `bin/list-all`, `bin/download`,
  `bin/install` (+ optional callbacks). asdf runs the **`download`→`install`
  chain**: if `bin/download` exists, asdf runs it first, then `bin/install`.
- **Env vars asdf sets** for the scripts: `ASDF_INSTALL_TYPE=version`,
  `ASDF_INSTALL_VERSION` (the version requested, e.g. `0.2.8`),
  `ASDF_INSTALL_PATH` (dir to INSTALL into), `ASDF_DOWNLOAD_PATH` (dir download
  writes to; install reads from it), `ASDF_CONCURRENCY`, `ASDF_PLUGIN_PATH`.
- **`bin/list-all`**: prints available versions. Classic format = **space-separated
  on one line** (asdf also accepts newline). asdf tokenizes on whitespace.
- **`bin/download`**: download the artifact into `$ASDF_DOWNLOAD_PATH`. Exit 0 =
  success (install then runs); non-zero = failure (install is skipped, asdf errors).
- **`bin/install`**: install into `$ASDF_INSTALL_PATH`. The default
  `list-bin-paths` callback returns `bin` ⇒ **executables must land in
  `$ASDF_INSTALL_PATH/bin/`** (that's what asdf shims). Exit 0 = success.
- **Platform detection**: asdf does NOT pass OS/arch env vars to the plugin; the
  scripts call `uname -s` / `uname -m` themselves (verified in asdf source).
- **`latest` resolution**: if no `bin/latest-stable` callback, asdf resolves
  `latest` from the list-all output (newest). To make `asdf install qmkonnect
  latest` correct WITHOUT a `latest-stable` callback (which is P1.M4.T1.S2's
  territory — plugin repo metadata), **list-all outputs versions ascending
  (oldest→newest)** via a portable numeric sort (`sort -t. -k1,1n -k2,2n -k3,3n`,
  works on BOTH GNU coreutils and BSD sort — NOT `sort -V`, which BSD sort may lack).
- **mise compatibility**: mise auto-detects asdf plugins and runs the same `bin/*`
  scripts → **zero plugin changes** for mise. `mise plugin add qmkonnect
  <url>` + `mise install qmkonnect@<ver>` just work. (external_deps §6.)

## Platform-by-platform install design (the honest framing)

asdf/mise are POSIX/bash version managers → they run on **Linux + macOS**.
Classic asdf does NOT run on Windows; mise-on-Windows uses Git Bash (an edge, not
the target). QMKonnect's three assets have very different natures:

| OS | Asset | asdf/mise install reality |
|---|---|---|
| **Linux x86_64** | `qmkonnect-{ver}-linux-x86_64.tar.gz` (portable binaries + udev + systemd) | **PRIMARY, fully supported.** tar-extract → copy `qmkonnect` + `qmkonnect-hid-id` into `$ASDF_INSTALL_PATH/bin/`. udev rule + systemd template need **root** (`udevadm`, `systemctl --global`) → can't auto-install under asdf's per-user model → stage them in `$ASDF_INSTALL_PATH/share/qmkonnect/` and document the one-time manual setup (mirror the Nix "document manual steps" approach). |
| **macOS** (any arch) | `QMKonnect-{ver}-macos.dmg` (universal2 .app bundle) | **CLI-only.** `hdiutil attach` → copy the raw Mach-O `QMKonnect.app/Contents/MacOS/qmkonnect` → `hdiutil detach`. The raw binary runs CLI flags but **NOT the menu-bar tray** (needs the full `.app` bundle: Info.plist/resources/icon). **Document the caveat** and point to the Homebrew cask / direct DMG for the full app. |
| Windows | `QMKonnect-{ver}-windows-x64.exe` (Inno INSTALLER, not portable) | **Not a real asdf target.** The `.exe` is a setup installer, not a portable binary suitable for `$ASDF_INSTALL_PATH/bin`. The install script **errors with a redirect** to Scoop / Winget / the Inno installer. (The download script still maps `*_NT-*`/`MSYS*`/`MINGW*` → the .exe for completeness, but install refuses.) |

This matches the contract (Linux tar extract + macOS DMG mount/copy + the macOS
CLI-only caveat) and is honest about Windows.

## The macOS DMG extraction idiom (verified)
```bash
mountpoint="$(mktemp -d)"
hdiutil attach -nobrowse -mountpoint "$mountpoint" "$dmg" >/dev/null
cp "$mountpoint/QMKonnect.app/Contents/MacOS/qmkonnect" "$ASDF_INSTALL_PATH/bin/qmkonnect"
hdiutil detach "$mountpoint" >/dev/null
```
`-nobrowse` keeps it out of Finder; `-mountpoint` forces a known path (vs the
default `/Volumes/QMKonnect`). The `.app`'s executable name = app name = `qmkonnect`.

## Linux manual setup (staged files → one-time commands; from qmkonnect.install)
asdf is per-user and can't run root commands. The README documents:
```bash
# (run once; STAGE=$ASDF_INSTALL_PATH/share/qmkonnect after `asdf install`)
sudo install -m644 "$STAGE/69-qmkonnect-rawhid.rules" /usr/lib/udev/rules.d/
sudo install -m755 "$(asdf which qmkonnect-hid-id 2>/dev/null || echo $ASDF_INSTALL_PATH/bin/qmkonnect-hid-id)" /usr/lib/udev/qmkonnect-hid-id
sudo udevadm control --reload-rules && sudo udevadm trigger
install -m644 "$STAGE/qmkonnect.service.template" ~/.config/systemd/user/qmkonnect.service
systemctl --user daemon-reload && systemctl --user enable --now qmkonnect.service
```
(Mirror of `qmkonnect.install` post_install, adapted for per-user asdf paths.)

## Files this task creates (exact, contract-literal)
1. `packaging/asdf/lib/utils.bash` — shared helpers (release identity, platform
   detect, asset-name map, curl download, optional SHA256 sidecar verify,
   version-list from GitHub API).
2. `packaging/asdf/bin/list-all` — prints bare versions ascending (space-separated).
3. `packaging/asdf/bin/download` — maps uname→asset, curls into `$ASDF_DOWNLOAD_PATH`, verifies SHA256 if a sidecar exists.
4. `packaging/asdf/bin/install` — Linux tar-extract / macOS DMG-binary / Windows redirect.
5. `packaging/asdf/README.md` — asdf + mise setup, platform support matrix, macOS
   CLI-only caveat, Linux one-time udev/systemd setup, Windows redirect.
All scripts `#!/usr/bin/env bash`, `set -euo pipefail`, `chmod +x`.

## Cross-task coordination (no conflicts)
- **P1.M3.T2.S2 (parallel, in-flight):** edits ONLY `packaging/winget/submit.ps1`
  + `packaging/winget/README.md`. ZERO overlap with `packaging/asdf/*`. Safe.
- **P1.M4.T1.S2 (downstream):** "asdf plugin repo metadata + mise native backend
  stub" — owns the plugin-repo-metadata files (e.g. a `bin/latest-stable`, the
  mise native backend, repo listing info). S1 creates the 4 core scripts + README;
  S2 adds metadata. S1 must NOT add `bin/latest-stable` (would collide with S2) —
  instead S1's list-all sorts ascending so `latest` resolves correctly without it.
- **P1.M5.T2.S2 (downstream):** "Add Nix flake check + asdf plugin test + asdf
  publish CI steps" — will `asdf plugin test` these scripts + push `packaging/asdf/`
  to the `asdf-qmkonnect` repo on tag. S1's scripts must be `asdf plugin test`-ready.

## Validation on the Linux dev box (what CAN run here)
- `bash -n` syntax-check all 4 scripts (no execution).
- `shellcheck` (if installed) the scripts.
- **End-to-end dry run of list-all** against the real GitHub API:
  `ASDF_PLUGIN_PATH=... bash packaging/asdf/bin/list-all` → must print `… 0.2.8`
  (0.2.8 last = newest). This validates the curl+grep+sed+sort pipeline live.
- **download against the real 0.2.8 release** (Linux asset): set the env vars,
  run bin/download, assert `$ASDF_DOWNLOAD_PATH/qmkonnect-0.2.8-linux-x86_64.tar.gz`
  exists + `tar tzf` lists the expected 4 files + its sha256 =
  `86dcaa57254f4b7dd23595ef8e473432c3997df3b6c12621913d215b9ab1b216`.
- **install against the real tarball**: run bin/install, assert
  `$ASDF_INSTALL_PATH/bin/qmkonnect` + `qmkonnect-hid-id` exist + are executable +
  `qmkonnect --version`/`--help` runs (or `--help` exits 0). Assert the udev/systemd
  files are staged under `$ASDF_INSTALL_PATH/share/qmkonnect/`.
- macOS DMG path + Windows redirect are NOT runnable on Linux → validated by code
  review + documented (the macOS binary-extract logic is standard `hdiutil`).
# scoop-qmkonnect — Scoop bucket for QMKonnect

Custom [Scoop](https://scoop.sh) bucket for
[QMKonnect](https://github.com/dabstractor/qmkonnect), the **Windows community
channel** (see [spec/PRD.md](https://github.com/dabstractor/qmkonnect/blob/master/spec/PRD.md)
§4 F15, §5 — "Windows: Inno `.exe` (primary, no admin) · Scoop · Winget")
alongside the primary direct Inno `.exe` installer.

## What this is

This repository is a Scoop **bucket** — a git repo named `scoop-<name>` that
holds [`bucket/qmkonnect.json`](bucket/qmkonnect.json), following the official
[`ScoopInstaller/BucketTemplate`](https://github.com/ScoopInstaller/BucketTemplate)
layout (`bucket/<app>.json` + a root `README.md`). This is the same shape as a
Homebrew tap's `Casks/` + README.

The manifest downloads the per-tag GitHub-release **Inno installer**
`QMKonnect-<version>-windows-x64.exe` (the `windows` job in the source repo's
[`.github/workflows/release.yml`](https://github.com/dabstractor/qmkonnect/blob/master/.github/workflows/release.yml),
renamed from `QMKonnect-Setup.exe`) and **extracts** it via Scoop's
`innosetup: true` flag — Scoop runs [`innounp`](https://sourceforge.net/projects/innounp/),
it does **not** run the installer. **No Rust toolchain, no `cargo`, no build
dependencies** — the release `.exe` statically links the C runtime (`+crt-static`
in `.cargo/config.toml`) and runs on any clean Windows 10/11 x64 box.

Scoop is **per-user**, so the install needs no admin (it matches the installer's
`PrivilegesRequired=lowest`). The release is **x64-only**
(`ArchitecturesAllowed=x64compatible` in
[`packaging/windows/inno/QMKonnect.iss`](https://github.com/dabstractor/qmkonnect/blob/master/packaging/windows/inno/QMKonnect.iss)),
so the manifest declares a `64bit` architecture block.

The installer is **not code-signed** — fine for Scoop (per
[spec/PRD.md](https://github.com/dabstractor/qmkonnect/blob/master/spec/PRD.md) §12 /
`architecture/external_deps.md` §3: *"Scoop unaffected, they don't enforce
code-signing"*).

## Install

```bash
# Add the bucket (alias MUST carry the explicit URL — `scoop bucket add qmkonnect`
# alone resolves to an implicit user bucket, which is wrong):
scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnect
scoop install qmkonnect
# Update to the latest release (CI keeps the bucket manifest current on each tag):
scoop update qmkonnect
# Uninstall:
scoop uninstall qmkonnect
```

`scoop update qmkonnect` pulls new releases automatically — the manifest's
`checkver`/`autoupdate` blocks detect new GitHub tags, so `scoop update` keeps
the app current with no hand-editing of the manifest.

## What it installs

A concise summary (the **source-repo**
[`packaging/scoop/README.md`](https://github.com/dabstractor/qmkonnect/blob/master/packaging/scoop/README.md)
has the full table):

| Where | What |
|---|---|
| `~\scoop\apps\qmkonnect\current\QMKonnect.exe` | The tray-app binary (extracted from the Inno installer) |
| `~\scoop\apps\qmkonnect\current\Icon.ico`, `…\IconTray-dark.png` | Icon assets (extracted alongside the exe) |
| Start Menu → **QMKonnect** | Scoop-managed shortcut (the manifest's `shortcuts`) |
| `%APPDATA%\QMKonnect\{config.toml,rules.toml}` | Per-user config (app-managed, **not** under the Scoop tree) |

## Differences from the direct Inno installer

**Scoop EXTRACTS the installer via `innounp`; it does not run it.** The Inno
installer's `[Registry]`, `[Icons]`, `[Run]`, and `[Code]` sections therefore do
**not** execute. Relative to double-clicking `QMKonnect-Setup.exe`, the Scoop
install differs in four ways:

- **Autostart is OFF by default.** The Inno installer writes the HKCU `Run`
  value `QMKonnect` (default-on); extraction skips that step.
  → Enable **"Open at Login"** in QMKonnect's tray menu. The app writes the
  *same* HKCU `Run` value itself, keyed to the current exe path, so the toggle
  works correctly from the Scoop apps tree.
- **No Add/Remove-Programs entry.** Manage the app with `scoop uninstall
  qmkonnect` (and `scoop update qmkonnect`), not "Apps & features".
- **Start Menu shortcut has no AppUserModelID.** The Inno installer brands WinRT
  toast notifications as "QMKonnect" (`Mulletware.QMKonnect`); extraction skips
  that step, so toasts render **generically** until a future manifest
  `post_install` sets the AUMID.
- **Install location.** The app lives in the Scoop apps tree
  (`~\scoop\apps\qmkonnect\current\`), not
  `%LOCALAPPDATA%\Programs\QMKonnect\` (the Inno `{app}`).

See the source-repo
[`packaging/scoop/README.md`](https://github.com/dabstractor/qmkonnect/blob/master/packaging/scoop/README.md)
§"Differences from the direct Inno installer" for the full detail.

## For maintainers — updating the manifest

The bucket's `bucket/qmkonnect.json` ships with a `version` and a 64-zero `hash`
placeholder; each tagged release patches `version` + `url` + `hash` and pushes.
The mechanical update is driven from the **source repo** by
[`packaging/scoop/update-manifest.ps1`](https://github.com/dabstractor/qmkonnect/blob/master/packaging/scoop/update-manifest.ps1):
it downloads the release `.exe`, computes its SHA256
(`Get-FileHash -Algorithm SHA256`), patches `version` + `url` + `hash` in
`qmkonnect.json` (leaving the `autoupdate` `$version` template untouched), and
re-validates the JSON.

```powershell
# From a clone of dabstractor/qmkonnect (source repo):
./packaging/scoop/update-manifest.ps1 -Version 0.2.8                           # download + hash + patch + validate
./packaging/scoop/update-manifest.ps1 -Version 0.2.8 -Sha256 <precomputed>     # skip the download
./packaging/scoop/update-manifest.ps1 -Help
```

The script is a **PURE local file update — it does NOT push** to the bucket
(CI does the push).

## CI publishing (deploy key)

New releases are pushed to this bucket **automatically** by the QMKonnect
release workflow (wired in **P1.M5.T1.S2**). It authenticates to this repo with
a GitHub **deploy key** (SSH, write access):

1. Generate an SSH key pair.
2. Add the **public** half to this bucket repo: *Settings → Deploy keys*
   (check "Allow write access").
3. Store the **private** half as the `SCOOP_BUCKET_DEPLOY_KEY` Actions secret in
   [`dabstractor/qmkonnect`](https://github.com/dabstractor/qmkonnect).

On a tag, CI loads that key into `ssh-agent` (e.g.
`webfactory/agents/github-ssh-agent@v0.9.0`), clones
`git@github.com:dabstractor/scoop-qmkonnect.git`, runs
`update-manifest.ps1` against the source checkout, copies the patched
`qmkonnect.json` into the clone as `bucket/qmkonnect.json`, commits, and pushes.
This mirrors the AUR SSH-key model
([`packaging/linux/aur/publish.sh`](https://github.com/dabstractor/qmkonnect/blob/master/packaging/linux/aur/publish.sh))
and the Homebrew deploy-key model
([`packaging/homebrew/tap-README.md`](https://github.com/dabstractor/homebrew-qmkonnect/blob/master/README.md)
§"CI publishing") — see `architecture/external_deps.md` §"CI Publishing Strategy".

## See also

- **Source repo:** <https://github.com/dabstractor/qmkonnect>
- **Install docs:**
  [`docs/installation.md`](https://github.com/dabstractor/qmkonnect/blob/master/docs/installation.md)
  (Windows section)
- **Inno installer:**
  [`packaging/windows/inno/`](https://github.com/dabstractor/qmkonnect/tree/master/packaging/windows/inno)
- **Packaging spec:**
  [`spec/PACKAGING.md`](https://github.com/dabstractor/qmkonnect/blob/master/spec/PACKAGING.md) §3
- **Sibling channels:**
  [`dabstractor/homebrew-qmkonnect`](https://github.com/dabstractor/homebrew-qmkonnect)
  (macOS Cask) ·
  [`packaging/linux/aur/`](https://github.com/dabstractor/qmkonnect/tree/master/packaging/linux/aur)
  (Linux AUR)
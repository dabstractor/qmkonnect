# qmkonnect — Scoop manifest (Windows)

[Scoop](https://scoop.sh) **app manifest** for
[QMKonnect](https://github.com/dabstractor/qmkonnect), the Windows community
channel (see [spec/PRD.md](../../spec/PRD.md) §4 F15, §5 — "Windows: Inno `.exe`
(primary, no admin) · Scoop · Winget") alongside the primary direct Inno `.exe`
installer.

## What this is

`packaging/scoop/qmkonnect.json` is a Scoop **app manifest** — not a formula. It
downloads the per-tag GitHub-release **Inno installer**
`QMKonnect-<version>-windows-x64.exe` (the `windows` job in
[`.github/workflows/release.yml`](../../.github/workflows/release.yml), renamed
from `QMKonnect-Setup.exe`) and **extracts** it via Scoop's `innosetup: true`
flag (Scoop runs [`innounp`](https://sourceforge.net/projects/innounp/) rather
than the installer). **No Rust toolchain, no `cargo`, no build dependencies** —
the release `.exe` statically links the C runtime (`+crt-static` in
`.cargo/config.toml`) and runs on any clean Windows 10/11 x64 box.

Scoop is **per-user**, so the install needs no admin (it matches the installer's
`PrivilegesRequired=lowest`). The release is **x64-only**
(`ArchitecturesAllowed=x64compatible` in
[`packaging/windows/inno/QMKonnect.iss`](../windows/inno/QMKonnect.iss)), so the
manifest declares a `64bit` architecture block.

The installer is **not code-signed** — fine for Scoop (per
[spec/PRD.md](../../spec/PRD.md) §12 /
[architecture/external_deps.md](../../plan/007_fb356ba503b4/architecture/external_deps.md)
§3: "Scoop unaffected, they don't enforce code-signing").

## Install

```bash
# Add the bucket, then install:
scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnect
scoop install qmkonnect
# Update to the latest release:
scoop update qmkonnect
# Uninstall:
scoop uninstall qmkonnect
```

> **Note on `scoop bucket add`:** the alias `qmkonnect` **must** carry the
> explicit URL. `scoop bucket add qmkonnect` *without* a URL resolves to an
> implicit user bucket — wrong. The bucket repo is
> [`dabstractor/scoop-qmkonnect`](https://github.com/dabstractor/scoop-qmkonnect)
> (org `dabstractor`), which is why the URL is required.

`scoop update qmkonnect` pulls new releases automatically — the manifest's
`checkver`/`autoupdate` blocks detect new GitHub tags, so `scoop update` keeps
the app current with no hand-editing of the manifest.

## What it installs

| Where | What |
|---|---|
| `~\scoop\apps\qmkonnect\current\QMKonnect.exe` | The tray-app binary (extracted from the Inno installer) |
| `~\scoop\apps\qmkonnect\current\Icon.ico`, `…\IconTray-dark.png` | Icon assets (extracted alongside the exe) |
| Start Menu → **QMKonnect** | Scoop-managed shortcut (from the manifest's `shortcuts`) |
| `%APPDATA%\QMKonnect\{config.toml,rules.toml}` | Per-user config (app-managed, **not** under the Scoop tree) |

## Differences from the direct Inno installer

**Scoop EXTRACTS the installer via `innounp`; it does not run it.** The Inno
installer's `[Registry]`, `[Icons]`, `[Run]`, and `[Code]` sections therefore do
**not** execute. Relative to double-clicking `QMKonnect-Setup.exe`, the Scoop
install differs in four ways:

- **Autostart is NOT on by default.** The Inno installer writes the HKCU `Run`
  value `QMKonnect` (default-on; see `[Registry]` in
  [`QMKonnect.iss`](../windows/inno/QMKonnect.iss)); extraction skips that step.
  → Enable **"Open at Login"** in QMKonnect's tray menu. The app writes the
  *same* HKCU `Run` value itself, keyed to the current exe path
  ([`src/autostart.rs`](../../src/autostart.rs)), so the toggle works correctly
  from the Scoop apps tree.
- **No Add/Remove-Programs entry.** Manage the app with `scoop uninstall
  qmkonnect` (and `scoop update qmkonnect`), not "Apps & features".
- **Start Menu shortcut has no AppUserModelID.** The Inno installer runs
  `set_aumid.ps1` from `[Code] CurStepChanged` to brand WinRT toast
  notifications as "QMKonnect" (`Mulletware.QMKonnect`, the
  [`APP_AUMID`](../../src/platforms/mod.rs) constant); extraction skips that
  step, so toast notifications (P1.M4) render **generically** until a future
  manifest `post_install` sets the AUMID. (Documented enhancement, out of scope
  here.)
- **Install location.** The app lives in the Scoop apps tree
  (`~\scoop\apps\qmkonnect\current\`), not
  `%LOCALAPPDATA%\Programs\QMKonnect\` (the Inno `{app}`). Per-user config under
  `%APPDATA%\QMKonnect\` is unaffected — it follows the user, not the install
  path.

## Version & hash maintenance

The manifest's `version`, `url`, and `hash` fields are regenerated
mechanically — `scoop checkup` (or the bucket's autoupdate) detects new GitHub
tags via `checkver`, fills `version` + `url` from the `autoupdate` template, and
**computes** the SHA256 from the downloaded file (the release publishes **no**
`.sha256` sidecar — verified: `grep sha256|sidecar .github/workflows/release.yml`
returns nothing). CI (P1.M5.T1.S2) does this automatically on each tag and
pushes the refreshed manifest to the bucket.

The shipped manifest in this directory carries a **64-zero `hash` placeholder**.
CI fills the real SHA256 of the Windows `.exe` before publishing to the bucket.
This is the documented "template; CI fills it" idiom (mirroring the AUR
`PKGBUILD` publish-time hash and the Homebrew cask's `sha256 :no_check` →
CI-patched hash). A zero hash is **safe**: Scoop checks the manifest hash against
the downloaded file's computed hash at install time, so zeros fail that check —
an unfilled manifest can never silently install a tampered binary.

## For maintainers

This directory owns only the **manifest + this source-repo README**. The bucket
repo and the publish script are sibling tasks — **do not create them here**;

- **Bucket repo:** [`dabstractor/scoop-qmkonnect`](https://github.com/dabstractor/scoop-qmkonnect)
  — sibling **P1.M3.T1.S2** owns its README and the `update-manifest.sh` publish
  script (the Scoop analogue of
  [`packaging/homebrew/update-cask.sh`](../homebrew/update-cask.sh) and
  [`packaging/linux/aur/publish.sh`](../linux/aur/publish.sh)).
- **CI publish:** on a tag, the release workflow (P1.M5.T1.S2) clones the bucket
  repo via a **deploy key**, runs the autoupdate to refresh `version`/`url`/
  `hash`, commits, and pushes. This mirrors the AUR SSH-key model
  (P1.M5.T1.S1) and the Homebrew deploy-key model (P1.M5.T1.S2) — see
  [architecture/external_deps.md](../../plan/007_fb356ba503b4/architecture/external_deps.md)
  §"CI Publishing Strategy".
- **`Casks/qmkonnect.rb` is the source of truth for Homebrew; the Scoop
  equivalent here is `qmkonnect.json` in this directory.** The bucket repo
  `dabstractor/scoop-qmkonnect` holds a CI-published copy; edit the file here and
  let CI propagate it.

## Cross-links

- **Source repo:** <https://github.com/dabstractor/qmkonnect>
- **Install docs:** [`docs/installation.md`](../../docs/installation.md)
  (Windows section)
- **Inno installer this manifest consumes:**
  [`packaging/windows/inno/`](../windows/inno/) (`QMKonnect.iss` →
  `QMKonnect-Setup.exe`, renamed to
  `QMKonnect-<version>-windows-x64.exe` for the release)
- **Packaging spec:** [`spec/PACKAGING.md`](../../spec/PACKAGING.md) §3 (Windows
  packaging)
- **Sibling community channels:** [`packaging/homebrew/README.md`](../homebrew/README.md)
  (macOS Cask) and [`packaging/linux/aur/README.md`](../linux/aur/README.md)
  (Linux AUR)
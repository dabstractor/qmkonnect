# qmkonnect — Winget manifest (Windows)

[Winget](https://learn.microsoft.com/windows/package-manager/winget/) (Windows
Package Manager) **manifest** for
[QMKonnect](https://github.com/dabstractor/qmkonnect), the Windows community
channel (see [spec/PRD.md](../../spec/PRD.md) §4 F15, §5 — "Windows: Inno `.exe`
(primary, no admin) · Scoop · Winget") alongside the primary direct Inno `.exe`
installer.

## What this is

`packaging/winget/` holds a Winget **manifest** — three YAML files (`version` +
`defaultLocale` + `installer`) describing the per-tag GitHub-release **Inno
installer** `QMKonnect-<version>-windows-x64.exe` (the `windows` job in
[`.github/workflows/release.yml`](../../.github/workflows/release.yml), renamed
from `QMKonnect-Setup.exe`). `InstallerType: inno` means Winget **runs the
installer** (unlike Scoop, which *extracts* it) with `/SILENT` /
`/VERYSILENT`. **No Rust toolchain, no `cargo`, no build dependencies** — the
release `.exe` statically links the C runtime (`+crt-static` in
`.cargo/config.toml`) and runs on any clean Windows 10/11 x64 box.

Winget installs **per-user** (`Scope: user`; no UAC — matches the installer's
`PrivilegesRequired=lowest` in
[`packaging/windows/inno/QMKonnect.iss`](../windows/inno/QMKonnect.iss)). The
release is **x64-only** (`ArchitecturesAllowed=x64compatible`), so the manifest
declares a single `x64` installer node.

**Multi-file format** (`ManifestVersion 1.6.0`): the single-file `singleton`
type is **deprecated** in the Windows Package Manager Community Repository
(`microsoft/winget-pkgs`), and `merged` does not exist as a `ManifestType` —
so the only non-deprecated, `wingetcreate`-native form for a new submission is
the three-file version + defaultLocale + installer triplet shipped here.

## Install / upgrade / uninstall

```powershell
winget install dabstractor.QMKonnect
# or the short Moniker form (only package with this moniker):
winget install qmkonnect
# Update to the latest release (CI keeps winget-pkgs current on each tag):
winget upgrade dabstractor.QMKonnect
# Uninstall:
winget uninstall dabstractor.QMKonnect
```

## ⚠️ "Unverified publisher" warning (read this)

The Inno `.exe` is **unsigned** ([spec/PRD.md](../../spec/PRD.md) §12 /
[architecture/external_deps.md](../../plan/007_fb356ba503b4/architecture/external_deps.md)
§4: "Winget prompts 'unverified publisher'"), so the **first** `winget install`
(and Windows SmartScreen) shows an **"unverified publisher"** prompt. This is
the expected beta state — a stable code-signing certificate is future work
(PRD §12) — and is **identical** to running the direct, unsigned
`QMKonnect-Setup.exe`. To proceed:

- **Winget policy prompt** → choose *continue / run anyway*.
- **SmartScreen** → *"More info"* → *"Run anyway"*.

([Scoop](../scoop/README.md) is unaffected — it *extracts* via `innounp` rather
than running the installer, so it never trips the publisher check.) If your
organization blocks unsigned Winget packages, use the **direct Inno installer**
([`packaging/windows/inno/`](../windows/inno/)) or
[Scoop](../scoop/README.md) instead.

## What it installs

Because Winget **runs** the Inno installer (not an extract), every `[Registry]`
/ `[Icons]` / `[Run]` / `[Code]` section executes — so the install is identical
to double-clicking `QMKonnect-Setup.exe`:

| Where | What |
|---|---|
| `%LOCALAPPDATA%\Programs\QMKonnect\QMKonnect.exe` | The tray-app binary + `Icon.ico` / `IconTray-*.png` (the Inno `{app}`) |
| Start Menu → **QMKonnect** | Shortcut with AUMID `Mulletware.QMKonnect` (set by `set_aumid.ps1` so WinRT toasts brand correctly) |
| HKCU `Run` value `QMKonnect` | Autostart (default-on — toggle *"Open at Login"* in the tray) |
| Add / Remove Programs → **QMKonnect** | ARP entry (Publisher `Mulletware` / Name `QMKonnect`) — this is how `winget list` / `winget upgrade` correlate the install |
| `%APPDATA%\QMKonnect\{config.toml,rules.toml}` | Per-user config (app-managed) |

## Difference from the direct Inno installer / Scoop

The payload is **identical** — Winget downloads and runs the same Inno
installer, so the install location, shortcuts, autostart value, and ARP entry
match the direct `.exe` exactly. The only difference is that the install is
**Winget-managed**: `winget upgrade dabstractor.QMKonnect` keeps it current
across releases. Versus [Scoop](../scoop/README.md): Scoop *extracts* the
installer (no autostart, no ARP entry, no AUMID); Winget *runs* it (autostart
on, ARP entry present, AUMID set).

## For maintainers — the manifest & CI

The shipped manifest carries `PackageVersion: 0.2.8`, a placeholder
`ReleaseDate: 2025-08-01`, and a **64-zero `InstallerSha256` placeholder**.
These are refreshed automatically on each release (the zero hash **will** fail a
real `winget install` pre-CI — that's expected; an unfilled manifest can never
silently install a tampered binary). Each release is published to
`microsoft/winget-pkgs` by CI (**P1.M3.T2.S2**) via
[`wingetcreate`](https://github.com/microsoft/winget-create):

- **First time (manual, one-time):** a maintainer runs `wingetcreate new`,
  points it at the release `.exe`, fills the metadata from this template, and
  submits the initial PR to `microsoft/winget-pkgs` (folder
  `manifests/d/dabstractor/QMKonnect/<version>/`).
- **Each subsequent release (CI, P1.M3.T2.S2):** the release workflow runs

  ```powershell
  wingetcreate update dabstractor.QMKonnect `
    -u https://github.com/dabstractor/qmkonnect/releases/download/v<version>/QMKonnect-<version>-windows-x64.exe `
    -v <version> -t <WINGET_GITHUB_TOKEN> --submit
  ```

  which refreshes `PackageVersion` + `InstallerUrl` + `InstallerSha256` +
  `ReleaseDate` and opens a PR to `microsoft/winget-pkgs`.

The `WINGET_GITHUB_TOKEN` (a PAT with `public_repo`) is a GitHub Actions secret
in `dabstractor/qmkonnect` — it mirrors the Scoop deploy-key / AUR-SSH model
(see
[architecture/external_deps.md](../../plan/007_fb356ba503b4/architecture/external_deps.md)
§"CI Publishing Strategy"). The workflow job itself is owned by P1.M3.T2.S2;
this task ships only the manifest template + this source-repo doc.

## Version & hash maintenance

`PackageVersion` is the bare Cargo version (`0.2.8`, **no** leading `v`; the
`checkver`/autoupdate source of truth is `Cargo.toml` — see
[architecture/external_deps.md](../../plan/007_fb356ba503b4/architecture/external_deps.md)
§"Version Source of Truth"). The release publishes **no** `.sha256` sidecar
(verified: `grep -E 'sha256|sidecar' .github/workflows/release.yml` returns
nothing), so `InstallerSha256` **must** be computed by `wingetcreate` from the
downloaded `.exe` (the SHA256 of the asset — see
[architecture/external_deps.md](../../plan/007_fb356ba503b4/architecture/external_deps.md)
§"Hashing").

## Validation

On a **Windows** host:

```powershell
wingetcreate validate packaging\winget\
# (optional smoke install — needs the real hash, not the placeholder)
winget install --manifest packaging\winget\dabstractor.QMKonnect.installer.yaml
```

On the **Linux** dev box, only YAML well-formedness can be checked (Winget /
wingetcreate are Windows-only):

```bash
python3 -c "import yaml,sys,glob; [yaml.safe_load(open(f)) for f in glob.glob('packaging/winget/*.yaml')]" \
  && echo "all YAML well-formed"
```

## See also

- **Source repo:** <https://github.com/dabstractor/qmkonnect>
- **Install docs:** [`docs/installation.md`](../../docs/installation.md)
  (Windows section — a Winget row is added in P1.M6.T1.S1)
- **Inno installer this manifest consumes:**
  [`packaging/windows/inno/`](../windows/inno/) (`QMKonnect.iss` →
  `QMKonnect-Setup.exe`, renamed to
  `QMKonnect-<version>-windows-x64.exe` for the release)
- **Packaging spec:** [`spec/PACKAGING.md`](../../spec/PACKAGING.md) §3
  (Windows packaging)
- **Sibling community channels:** [`packaging/scoop/README.md`](../scoop/README.md)
  (Windows Scoop), [`packaging/homebrew/README.md`](../homebrew/README.md)
  (macOS Homebrew), [`packaging/linux/aur/README.md`](../linux/aur/README.md)
  (Linux AUR)
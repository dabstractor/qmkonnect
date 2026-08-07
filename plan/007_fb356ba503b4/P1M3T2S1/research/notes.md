# Research notes — P1.M3.T2.S1 (Winget package manifest)

## Verdict on the contract's "ManifestType: merged" — it does NOT exist

The contract says: "Create ... `ManifestType: merged` (single-file format) OR
separate `*.installer.yaml` + `*.defaultLocale.yaml` + `*.version.yaml` files."

Verified against the authoritative source (`microsoft/winget-pkgs` schema dirs
1.0.0 → 1.12.0, each containing ONLY: `README.md`, `defaultLocale.md`,
`installer.md`, `locale.md`, `singleton.md`, `version.md`):

- **There is no `merged` ManifestType in any schema version.** The valid
  `ManifestType` values are: `version`, `defaultLocale`, `locale`, `installer`,
  `singleton`.
- The single-file format is **`singleton`** (`ManifestType: singleton`).
- **CRITICAL — singleton is DEPRECATED for the community repo.** From the 1.6.0
  `singleton.md`:
  > "The singleton manifest format has been deprecated in the Windows Package
  > Manager Community Repository. The Windows Package Manager 1.6 client still
  > supports singleton manifests."

So neither of the contract's "single-file" options is right for a NEW submission
to `microsoft/winget-pkgs` (where P1.M3.T2.S2 publishes via wingetcreate):
`merged` is invalid; `singleton` is deprecated. The correct, future-proof choice
is the contract's OWN alternative branch — the **multi-file format**
(`version` + `defaultLocale` + `installer`). This is also exactly what
`wingetcreate new`/`update` generates. → This PRP uses MULTI-FILE.

## The three-file multi-file manifest (ManifestVersion 1.6.0)

Authoritative field lists from `microsoft/winget-pkgs/doc/manifest/schema/1.6.0/`
(`version.md`, `defaultLocale.md`, `installer.md`, `singleton.md`).

### File 1 — `<id>.yaml` (VERSION manifest) — minimal, required:
```yaml
PackageIdentifier: dabstractor.QMKonnect
PackageVersion: 0.2.8
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.6.0
```
(note: the version file uses `DefaultLocale`, NOT `PackageLocale`.)

### File 2 — `<id>.locale.<locale>.yaml` (DEFAULTLOCALE manifest)
Required: PackageIdentifier, PackageVersion, PackageLocale, Publisher, PackageName,
License, ShortDescription, ManifestType: defaultLocale, ManifestVersion.
Optional (include): PublisherURL, PublisherSupportURL, PackageURL, LicenseUrl,
Description, Tags, Moniker, ReleaseDate.
```yaml
PackageIdentifier: dabstractor.QMKonnect
PackageVersion: 0.2.8
PackageLocale: en-US
Publisher: Mulletware
PublisherURL: https://github.com/dabstractor/qmkonnect
PublisherSupportURL: https://github.com/dabstractor/qmkonnect/issues
PackageName: QMKonnect
PackageUrl: https://github.com/dabstractor/qmkonnect
License: MIT
LicenseUrl: https://github.com/dabstractor/qmkonnect/blob/master/LICENSE
ShortDescription: Cross-platform window activity notifier for QMK keyboards (Windows tray app)
Tags:
- qmk
- keyboard
- hid
- tray
ReleaseDate: 2025-08-01   # placeholder; wingetcreate/CI sets per release
ManifestType: defaultLocale
ManifestVersion: 1.6.0
```

### File 3 — `<id>.installer.yaml` (INSTALLER manifest)
Required: PackageIdentifier, PackageVersion, Installers[Architecture, InstallerType,
InstallerUrl, InstallerSha256], ManifestType: installer, ManifestVersion.
Installer-level keys may be at ROOT (inherited by all installers) OR per-installer
(per `installer.md` NOTE). With ONE installer, root-level is cleanest + idiomatic
(matches `wingetcreate` output + the WindowsTerminal complex example).
```yaml
PackageIdentifier: dabstractor.QMKonnect
PackageVersion: 0.2.8
InstallerType: inno          # root-level default for all installers
Scope: user                  # per-user (Inno PrivilegesRequired=lowest)
InstallModes:
- interactive
- silent
- silentWithProgress
InstallerSwitches:
  Silent: /VERYSILENT
  SilentWithProgress: /SILENT
UpgradeBehavior: install
ReleaseDate: 2025-08-01      # placeholder; wingetcreate/CI sets per release
Installers:
- Architecture: x64
  InstallerType: inno
  InstallerUrl: https://github.com/dabstractor/qmkonnect/releases/download/v0.2.8/QMKonnect-0.2.8-windows-x64.exe
  InstallerSha256: 0000000000000000000000000000000000000000000000000000000000000000  # placeholder; CI fills
ManifestType: installer
ManifestVersion: 1.6.0
```

## Field-level decisions & gotchas

- **InstallModes must include `silentWithProgress`.** The community repo
  REQUIRES "silent" AND "silent with progress" support
  (`installer.md`/`singleton.md` InstallModes note). The contract listed
  `[interactive, silent]`; that would FAIL community validation. Inno supports
  all three → use `[interactive, silent, silentWithProgress]`. (Documented
  correction to the contract.)
- **InstallerSwitches are technically redundant for `inno`.** `manifest.md`:
  "When Nullsoft or Inno are specified, the client will automatically set the
  silent and silent with progress install behaviors." But the contract
  explicitly requests them and explicit switches are harmless + clearer → KEEP
  `Silent: /VERYSILENT`, `SilentWithProgress: /SILENT` (Inno's documented
  switches, per `jrsoftware.org/ishelp/`).
- **`Scope: user`** matches Inno `PrivilegesRequired=lowest` (per-user, no UAC).
- **UpgradeBehavior: install** — Inno upgrades in place (stable AppId
  `{{FAAE1F7A-...}}`), so "install" (re-run installer over the existing one) is
  correct, NOT "uninstallPrevious".
- **ReleaseDate** format is RFC3339/ISO8601 `YYYY-MM-DD`. Placeholder value;
  `wingetcreate` fills it from the GitHub release's published-at on update.
- **PackageIdentifier casing**: `dabstractor.QMKonnect` (lowercase org, matches
  the contract + every other F15 channel: scoop-qmkonnect, homebrew-qmkonnect,
  AUR under `dabstractor`). winget-pkgs folder would be
  `manifests/d/dabstractor/QMKonnect/<version>/`. The Publisher part is an
  identity token; it need NOT equal the display `Publisher` ("Mulletware"),
  which is correct — ARP correlation is via Publisher+PackageName matching the
  installer's ARP entry (both "Mulletware"/"QMKonnect"), not the id prefix.
- **No `AppsAndFeaturesEntries` needed.** Publisher("Mulletware")+PackageName
  ("QMKonnect") already match the Inno ARP entry (UninstallDisplayName=
  "QMKonnect", AppPublisher="Mulletware"), so winget list/upgrade correlation
  works without it. Keeping the manifest lean per the contract.
- **No `ProductCode`/`PackageFamilyName`/`SignatureSha256`** — those are for
  MSI/MSIX. Inno needs none.
- **`Moniker: qmkonnect`** — lets users `winget install qmkonnect` (short form);
  optional but improves discoverability (only package with this moniker).

## Facts confirmed from the repo

- GitHub org = `dabstractor` (`git remote get-url origin`).
- version = 0.2.8 (Cargo.toml), NO leading v; tag = v0.2.8; asset filename uses
  bare version: `QMKonnect-0.2.8-windows-x64.exe` (release.yml `windows` job:
  `Move-Item Output/QMKonnect-Setup.exe Output/QMKonnect-$version-windows-x64.exe`).
- NO `.sha256` sidecar published (grep release.yml → none) → the manifest's
  `InstallerSha256` MUST be filled by CI/wingetcreate from the downloaded .exe.
- License = MIT (Cargo.toml). LICENSE file = `LICENSE` (no extension) →
  LicenseUrl = `.../blob/master/LICENSE`.
- Inno: MyAppPublisher="Mulletware", MyAppName="QMKonnect", MyAppExeName=
  "QMKonnect.exe", AppId=`{{FAAE1F7A-9DBD-4C2A-B122-A9A73F05D0B3}}`,
  DefaultDirName={localappdata}\Programs\QMKonnect, PrivilegesRequired=lowest,
  HKCU Run autostart "QMKonnect", AUMID Mulletware.QMKonnect (set_aumid.ps1).
- Description (Cargo.toml) = "Cross-platform window activity notifier for QMK
  keyboards". ShortDescription must be ≤100 chars → use the Scoop manifest's
  phrasing "Cross-platform window activity notifier for QMK keyboards (Windows
  tray app)" (≤100 chars).

## CI publishing model (context for the README; the workflow itself = P1.M3.T2.S2)

- external_deps.md §4: "Automated PR to microsoft/winget-pkgs via GitHub Action
  (e.g., `vedantmgoyal9/winget-pkgs-automation`) or wingetcreate".
- The canonical tool is **`wingetcreate`** (`winget install wingetcreate`):
  - `wingetcreate new` → generates the initial multi-file manifest + opens the
    FIRST PR to winget-pkgs (manual, one-time, by a maintainer).
  - `wingetcreate update dabstractor.QMKonnet -u <release-exe-url> -v <version>
    -t <GITHUB_TOKEN> --submit` → CI refreshes version+url+sha256+ReleaseDate and
    submits a PR to winget-pkgs on each tag (P1.M3.T2.S2).
- So THIS task's manifest template is: (a) the reference for the initial manual
  submission, and (b) the human-readable canonical spec. wingetcreate regenerates
  the canonical form in winget-pkgs on update.
- The 64-zero `InstallerSha256` + placeholder `ReleaseDate`/`PackageVersion` are
  INTENTIONAL placeholders (CI/wingetcreate fills them); document this.

## "Unverified publisher" (PRD §12 / external_deps.md §4)

The Inno `.exe` is unsigned → `winget install` (and SmartScreen) prompts
"unverified publisher". The README MUST document this (the contract's DOCS
requirement). It's the expected state for the beta; a stable code-signing cert
is future work (PRD §12). Users bypass it once ("More info" → "Run anyway") or
install via the direct Inno installer / Scoop (no such prompt). This is the same
state as the direct unsigned `.exe`.

## Validation approach (Linux dev box)

- The dev box is Linux; wingetcreate/winget are Windows-only. Validate the
  manifests' STRUCTURE on Linux with:
  1. `python3 -c "import yaml,sys; yaml.safe_load(open(f))"` per file (YAML
     well-formed — python3+yaml is available; the repo's docs tooling uses it).
     Fallback: `python3` absent → grep invariants.
  2. grep the required keys + the exact asset URL + placeholder hash + the
     `dabstractor.QMKonnect` id + `ManifestVersion: 1.6.0` + the three distinct
     `ManifestType:` values.
  3. Cross-file consistency: PackageIdentifier + PackageVersion identical across
     all three; DefaultLocale (file1) == PackageLocale (file2) == en-US.
- DEFERRED to a Windows host: `wingetcreate validate <dir>` +
  `winget validate <manifest>` + a smoke `winget install dabstractor.QMKonnect
  --manifest packaging/winget/` (after CI fills the real hash). Note in report.

## Scope boundaries (siblings)
- P1.M3.T1.S2 (Scoop bucket-README + update-manifest.ps1, parallel): different
  dir (`packaging/scoop/`). No overlap.
- P1.M3.T2.S2 (Winget publishing CI, Planned): consumes this task's manifest
  template + README; writes `.github/workflows/release.yml` winget job. THIS task
  does NOT write CI. Document the wingetcreate flow in the README; do not add
  the workflow.
- P1.M6.T1.S1 (docs/installation.md community channels): will ADD a Winget row
  to docs/installation.md. THIS task creates `packaging/winget/README.md` only;
  do NOT edit docs/installation.md (P1.M6 owns it).
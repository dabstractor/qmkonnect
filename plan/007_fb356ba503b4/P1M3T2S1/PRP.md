# PRP — P1.M3.T2.S1: Create Winget package manifest (multi-file YAML) + README

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). **Pure packaging — no Rust/source/CI change.**
> **Four new files** under `packaging/winget/`: the 3-file Winget manifest (`dabstractor.QMKonnect.yaml`
> version + `dabstractor.QMKonnect.locale.en-US.yaml` defaultLocale + `dabstractor.QMKonnect.installer.yaml`
> installer) + `README.md` (the Winget installation doc). **Scope:** the manifest template + the source-repo
> packaging doc ONLY. The CI publish job is **P1.M3.T2.S2** (Planned).
> **Pattern:** this task is the Winget analogue of the COMPLETED Scoop S1
> (`packaging/scoop/qmkonnect.json` + `packaging/scoop/README.md`) and Homebrew S1
> (`packaging/homebrew/Casks/qmkonnect.rb` + `packaging/homebrew/README.md`) — read both in full before
> writing; they are the manifest + doc template.

---

## ⚠️ CRITICAL CORRECTION TO THE CONTRACT (read first)

The contract says: *"`ManifestType: merged` (single-file format) OR separate `*.installer.yaml` +
`*.defaultLocale.yaml` + `*.version.yaml` files."* Verified against the authoritative schema
(`microsoft/winget-pkgs/doc/manifest/schema/{1.0.0..1.12.0}/`):

1. **There is NO `merged` ManifestType.** The valid `ManifestType` values are `version`, `defaultLocale`,
   `locale`, `installer`, `singleton`. (`merged` does not exist in any schema version.)
2. The real single-file type is **`singleton`** — and it is **DEPRECATED in the winget-pkgs community
   repo** (from `1.6.0/singleton.md`: *"The singleton manifest format has been deprecated in the Windows
   Package Manager Community Repository."*).

Neither single-file option works for a NEW submission to `microsoft/winget-pkgs` (where P1.M3.T2.S2
publishes): `merged` is invalid; `singleton` is deprecated. This PRP therefore uses the contract's OWN
alternative — the **multi-file format** (`version` + `defaultLocale` + `installer`), which is also exactly
what `wingetcreate new`/`update` generates. A second, smaller correction: the contract's
`InstallModes: [interactive, silent]` would FAIL community-repo validation (it REQUIRES `silent` AND
`silentWithProgress`); Inno supports all three, so the manifest uses `[interactive, silent, silentWithProgress]`.

---

## Goal

**Feature Goal**: Stand up the **Winget channel manifest** for QMKonnect (PRD §4 F15; §5 — "Windows: Inno
`.exe` (primary, no admin) · Scoop · Winget"). Deliver a valid, wingetcreate-ready **multi-file manifest**
(version + defaultLocale + installer, `ManifestVersion: 1.6.0`) plus a `README.md` documenting
`winget install dabstractor.QMKonnect`, the "unverified publisher" warning (PRD §12), and the automated
wingetcreate-PR workflow (P1.M3.T2.S2). The manifest points at the per-tag GitHub-release **Inno installer**
`QMKonnet-<version>-windows-x64.exe` (the `windows` job in `.github/workflows/release.yml`, renamed from
`QMKonnect-Setup.exe`), declares `InstallerType: inno` + `Scope: user` + the Inno silent switches, and ships
a 64-zero `InstallerSha256` placeholder that CI/wingetcreate fills per release.

**Deliverable** (4 new files under `packaging/winget/`):
1. `packaging/winget/dabstractor.QMKonnect.yaml` — **version** manifest.
2. `packaging/winget/dabstractor.QMKonnect.locale.en-US.yaml` — **defaultLocale** (en-US) manifest.
3. `packaging/winget/dabstractor.QMKonnect.installer.yaml` — **installer** manifest (the x64 Inno entry).
4. `packaging/winget/README.md` — Winget install/update/uninstall + "unverified publisher" + wingetcreate-PR docs.

**Success Definition**:
- All three YAML files are well-formed YAML (`python3 -c "import yaml; yaml.safe_load(open(f))"` per file
  passes) and carry the contract's exact metadata: `PackageIdentifier: dabstractor.QMKonnect`,
  `PackageVersion: 0.2.8`, `Publisher: Mulletware`, `PackageName: QMKonnect`, `License: MIT`, `Homepage`,
  `ShortDescription`, `Tags: [qmk, keyboard, hid, tray]`, `InstallerType: inno`, `Scope: user`,
  `Architecture: x64`, `InstallerSwitches: { Silent: /VERYSILENT, SilentWithProgress: /SILENT }`,
  `UpgradeBehavior: install`, `ReleaseDate`, `ManifestVersion: 1.6.0`.
- The three `ManifestType` values are distinct and correct: `version` / `defaultLocale` / `installer`.
- `PackageIdentifier` + `PackageVersion` are byte-identical across all three files; `DefaultLocale` (file 1)
  == `PackageLocale` (file 2) == `en-US`.
- `README.md` contains the exact `winget install dabstractor.QMKonnect` command, the "unverified publisher"
  warning + bypass, and the wingetcreate-PR workflow pointer to P1.M3.T2.S2.
- `git diff --stat` shows ONLY the 4 new files under `packaging/winget/`. No Rust/source/Cargo/
  `.github/workflows/*`/other-packaging-dir/docs changes.
- (Windows host, optional/deferred) `wingetcreate validate packaging/winget/` passes; deferred to a Windows
  box (wingetcreate is Windows-only; the dev box is Linux).

## User Persona (if applicable)

**Target User**: a Windows end-user who installs software via **Winget** (Windows Package Manager) and wants
QMKonnect installable/updatable via `winget install`/`winget upgrade` alongside the direct Inno installer.

**Use Case**: `winget install dabstractor.QMKonnect` (or `winget install qmkonnet` via the Moniker) downloads
the Inno `.exe` from the GitHub release and runs it silently (`/SILENT`). `winget upgrade dabstractor.QMKonnect`
pulls each new release (CI keeps winget-pkgs current via wingetcreate).

**User Journey**: (1) `winget search qmkonnect` → finds `dabstractor.QMKonnect`; (2) `winget install
dabstractor.QMKonnect` → SmartScreen/winget prompts "unverified publisher" (unsigned — see README); user
allows; (3) the Inno installer runs per-user (no UAC), places `QMKonnect.exe` + icons in
`%LOCALAPPDATA%\Programs\QMKonnect`, writes HKCU Run autostart, Start Menu shortcut (AUMID
`Mulletware.QMKonnect`); (4) the tray app launches; (5) `winget upgrade` keeps it current.

**Pain Points Addressed**: gives Winget users a native, `winget upgrade`-managed channel. Mirrors the proven
Scoop (S1/S2) + Homebrew (S1/S2) channel shape. F15 (PRD §4) requires a Winget channel.

## Why

- **F15 (PRD §4) requires a Winget channel.** This task ships the manifest + the source-repo doc. CI
  (P1.M3.T2.S2) wires the automated wingetcreate PR to `microsoft/winget-pkgs`. Per external_deps.md §4 /
  PRD §12, Winget prompts "unverified publisher" (the Inno `.exe` is unsigned) — the README documents this
  honestly (same state as the direct unsigned `.exe`; a stable code-signing cert is future work per PRD §12).
- **Multi-file is the only non-deprecated, wingetcreate-native format.** `merged` is invalid; `singleton` is
  deprecated for the community repo; `wingetcreate new`/`update` emits the 3-file form. This PRP uses it so
  the initial manual submission + CI-driven updates are frictionless.
- **Mirrors the proven Scoop/Homebrew channel pattern.** `packaging/scoop/{qmkonnect.json, README.md}` and
  `packaging/homebrew/{Casks/qmkonnect.rb, README.md}` (ALL COMPLETE) are the structural templates. This task
  is the Winget translation: the manifest triplet ← Scoop's `qmkonnect.json`; the README ← Scoop's README.

## What

### Naming Truth (GROUND TRUTH — read before writing)

- `git remote get-url origin` → `git@github.com:dabstractor/qmkonnect.git` ⇒ **GitHub org = `dabstractor`**.
  (The local Linux user is `dustin` in `/home/dustin/...`; that is UNRELATED to the org.)
- Source repo = **`dabstractor/qmkonnect`**. **PackageIdentifier = `dabstractor.QMKonnect`** (lowercase org,
  matches the contract + every other F15 channel: scoop-qmkonnect, homebrew-qmkonnect, AUR). winget-pkgs
  folder would be `manifests/d/dabstractor/QMKonnect/<version>/`.
- The Publisher part of the id (`dabstractor`) is an identity token — it NEED NOT equal the display
  `Publisher` ("Mulletware", from Inno `MyAppPublisher`). ARP correlation is via Publisher+PackageName
  matching the installer's ARP entry (both "Mulletware"/"QMKonnect"), not the id prefix.

### File 1 — `packaging/winget/dabstractor.QMKonnect.yaml` (VERSION manifest)

```yaml
# Winget VERSION manifest for QMKonnect (PRD §4 F15, §5 — "Windows: Inno .exe · Scoop · Winget").
# Multi-file format (version + defaultLocale + installer) — the only non-deprecated form for
# microsoft/winget-pkgs (singleton is deprecated; "merged" does not exist). See packaging/winget/README.md.
# CI (P1.M3.T2.S2 via wingetcreate) updates PackageVersion + DefaultLocale-ReleaseDate + installer
# InstallerSha256/InstallerUrl per release. See plan/007_fb356ba503b4/architecture/external_deps.md §4.
PackageIdentifier: dabstractor.QMKonnect
PackageVersion: 0.2.8
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.6.0
```

### File 2 — `packaging/winget/dabstractor.QMKonnect.locale.en-US.yaml` (DEFAULTLOCALE manifest)

```yaml
# Winget DEFAULTLOCALE (en-US) manifest for QMKonnect. Metadata source: Cargo.toml (version/license/desc)
# + packaging/windows/inno/QMKonnect.iss (Publisher/Name). Tags mirror the F15 channel set.
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
Description: QMKonnect detects the foreground window (app class + title) and notifies a QMK keyboard over Raw HID, so the keymap can switch layers/callbacks on window focus with no reflash. This is the per-user Windows tray-app channel (Inno installer, no admin).
Tags:
- qmk
- keyboard
- hid
- tray
Moniker: qmkonnect
ReleaseDate: 2025-08-01
ManifestType: defaultLocale
ManifestVersion: 1.6.0
```
- **ShortDescription ≤ 100 chars**: "Cross-platform window activity notifier for QMK keyboards (Windows tray app)" = 80 chars ✓ (schema limits strings to 100 before a line break).
- **`ReleaseDate: 2025-08-01`** is a PLACEHOLDER (RFC3339/ISO8601 `YYYY-MM-DD`); `wingetcreate update` sets
  it from the GitHub release's published-at on each tag. Document this in the README.

### File 3 — `packaging/winget/dabstractor.QMKonnect.installer.yaml` (INSTALLER manifest)

```yaml
# Winget INSTALLER manifest for QMKonnect. InstallerType: inno (the shipped Inno Setup installer,
# packaging/windows/inno/QMKonnect.iss → QMKonnect-Setup.exe, renamed by CI to QMKonnect-<ver>-windows-x64.exe).
# Per-user (Inno PrivilegesRequired=lowest → Scope: user, no UAC). Installer-level keys at root are inherited
# by the single x64 installer node (per schema NOTE — matches wingetcreate output + the WindowsTerminal example).
# InstallerSha256 is a 64-zero PLACEHOLDER; CI (P1.M3.T2.S2) fills the real hash from the downloaded .exe.
PackageIdentifier: dabstractor.QMKonnect
PackageVersion: 0.2.8
InstallerType: inno
Scope: user
InstallModes:
- interactive
- silent
- silentWithProgress
InstallerSwitches:
  Silent: /VERYSILENT
  SilentWithProgress: /SILENT
UpgradeBehavior: install
ReleaseDate: 2025-08-01
Installers:
- Architecture: x64
  InstallerType: inno
  InstallerUrl: https://github.com/dabstractor/qmkonnect/releases/download/v0.2.8/QMKonnect-0.2.8-windows-x64.exe
  InstallerSha256: 0000000000000000000000000000000000000000000000000000000000000000
ManifestType: installer
ManifestVersion: 1.6.0
```
- **`InstallModes` includes `silentWithProgress`** (community repo REQUIRES silent + silentWithProgress;
  Inno supports all three). This is a correction to the contract's `[interactive, silent]`.
- **`InstallerSwitches` are explicit** (the contract requests them). For `InstallerType: inno` the winget
  client auto-applies `/VERYSILENT`/`/SILENT` anyway (manifest.md), so these are redundant-but-harmless and
  make the intent self-documenting. (`/VERYSILENT` = fully silent; `/SILENT` = progress, no prompts — per
  `jrsoftware.org/ishelp/`.)
- **`UpgradeBehavior: install`** — the Inno installer upgrades in place via its stable AppId
  (`{{FAAE1F7A-9DBD-4C2A-B122-A9A73F05D0B3}}`), so re-running the installer over the existing one is correct
  (NOT `uninstallPrevious`).
- **No `AppsAndFeaturesEntries`/`ProductCode`/`SignatureSha256`** — Publisher+PackageName already match the
  Inno ARP entry (correlation works); ProductCode/Signature are MSI/MSIX-only. Keeping the manifest lean.

### File 4 — `packaging/winget/README.md` (Mode A docs)

Mirror the structure/tone of `packaging/scoop/README.md` + `packaging/homebrew/README.md`. Sections:
1. **Title + one-line**: `# qmkonnect — Winget manifest (Windows)` — Winget manifest for
   [QMKonnect](https://github.com/dabstractor/qmkonnect), the Windows community channel (PRD §4 F15, §5 —
   "Windows: Inno `.exe` (primary, no admin) · Scoop · Winget") alongside the primary direct Inno `.exe`.
2. **What this is**: a Winget **manifest** (3 YAML files: version + defaultLocale + installer) describing
   the per-tag GitHub-release **Inno installer** `QMKonnect-<version>-windows-x64.exe` (the `windows` job in
   `.github/workflows/release.yml`, renamed from `QMKonnect-Setup.exe`). `InstallerType: inno` → winget runs
   the installer (NOT an extract like Scoop) with `/SILENT`/`/VERYSILENT`. **No Rust toolchain** — the `.exe`
   statically links the CRT (`+crt-static`). Per-user (`Scope: user`, no admin; matches
   `PrivilegesRequired=lowest`). x64-only (`Architecture: x64`). **Multi-file format** (`ManifestVersion
   1.6.0`): the single-file `singleton` is deprecated in the community repo and `merged` does not exist.
3. **Install / upgrade / uninstall** (the EXACT commands):
   ```powershell
   winget install dabstractor.QMKonnect
   # or the short Moniker form (only package with this moniker):
   winget install qmkonnect
   # Update to the latest release (CI keeps winget-pkgs current on each tag):
   winget upgrade dabstractor.QMKonnect
   # Uninstall:
   winget uninstall dabstractor.QMKonnect
   ```
4. **⚠️ "Unverified publisher" warning (read this)** — the Inno `.exe` is **unsigned** (PRD §12), so the
   FIRST `winget install` (and Windows SmartScreen) shows an **"unverified publisher"** prompt. This is the
   expected beta state (a stable code-signing cert is future work, PRD §12) and is IDENTICAL to running the
   direct unsigned `QMKonnect-Setup.exe`. To proceed: winget's policy prompt → continue, or SmartScreen →
   "More info" → "Run anyway". (Scoop is unaffected — it extracts via `innounp` rather than running the
   installer.) If your org blocks unsigned winget packages, use the direct Inno installer or Scoop instead.
5. **What it installs** (table — mirror Scoop's): `%LOCALAPPDATA%\Programs\QMKonnect\QMKonnect.exe` + icons
   (the Inno installer's `{app}`); Start Menu → **QMKonnect** shortcut (AUMID `Mulletware.QMKonnect` so toasts
   brand correctly); HKCU `Run` autostart value `QMKonnect` (default-on — toggle "Open at Login" in the tray);
   `%APPDATA%\QMKonnect\{config.toml,rules.toml}` per-user config. (Unlike Scoop, winget RUNS the installer,
   so autostart IS on by default + there IS an Add/Remove-Programs entry.)
6. **Difference from the direct Inno installer / Scoop**: identical payload (it IS the Inno installer), just
   winget-managed (`winget upgrade` keeps it current). Point to the source-repo scoop README + Inno docs.
7. **For maintainers — the manifest & CI**: the manifest ships with `PackageVersion: 0.2.8`, a placeholder
   `ReleaseDate`, and a 64-zero `InstallerSha256` placeholder. Each release is published to
   `microsoft/winget-pkgs` automatically by CI (P1.M3.T2.S2) via [`wingetcreate`](https://github.com/microsoft/winget-create):
   - **First time (manual, one-time):** a maintainer runs `wingetcreate new`, points it at the release
     `.exe`, fills metadata from this template, and submits the initial PR to `microsoft/winget-pkgs`
     (folder `manifests/d/dabstractor/QMKonnect/<version>/`).
   - **Each subsequent release (CI, P1.M3.T2.S2):** the release workflow runs
     `wingetcreate update dabstractor.QMKonnet -u <release-exe-url> -v <version> -t <WINGET_GITHUB_TOKEN>
     --submit`, which refreshes `PackageVersion` + `InstallerUrl` + `InstallerSha256` + `ReleaseDate` and
     opens a PR to `microsoft/winget-pkgs`.
   The `WINGET_GITHUB_TOKEN` (a PAT with `public_repo`) is an Actions secret in `dabstractor/qmkonnect`
   (mirrors the SCOOP_BUCKET_DEPLOY_KEY / AUR-SSH model — see `architecture/external_deps.md` "CI Publishing
   Strategy").
8. **Validation** (for maintainers): on a Windows host, `wingetcreate validate packaging/winget/` and
   `winget validate packaging/winget/dabstractor.QMKonnect.installer.yaml` (the latter needs the real hash).
   On Linux, `python3 -c "import yaml; yaml.safe_load(open(f))"` per file checks YAML well-formedness.
9. **See also**: source repo; install docs `docs/installation.md` (Windows section); Inno installer
   `packaging/windows/inno/`; packaging spec `spec/PACKAGING.md` §3; sibling channels Scoop
   (`packaging/scoop/`) + Homebrew (`packaging/homebrew/`) + AUR (`packaging/linux/aur/`).

### Success Criteria
- [ ] 3 YAML files exist with the exact content above; a 4th `README.md` exists with sections 1–9.
- [ ] All 3 YAML files parse as well-formed YAML (python3 + yaml, OR grep invariants if python3/yaml absent).
- [ ] The three `ManifestType` values are `version` / `defaultLocale` / `installer`; all three
      `ManifestVersion: 1.6.0`; all three `PackageIdentifier: dabstractor.QMKonnect`; all three
      `PackageVersion: 0.2.8`.
- [ ] README contains `winget install dabstractor.QMKonnect`, the "unverified publisher" warning, and the
      wingetcreate-PR workflow pointer (P1.M3.T2.S2).
- [ ] `git diff --stat` shows ONLY 4 new files under `packaging/winget/`.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior Winget knowledge can create all 4 files verbatim from the "What" section
(the 3 manifests are given in full above; the README is specced section-by-section with exact commands +
warnings), and validate on Linux via `python3 + yaml` (or grep invariants). The Windows-only
`wingetcreate validate` / `winget install --manifest` smoke test is explicitly deferred.

### Documentation & References

```yaml
# MUST READ — the authoritative Winget manifest schema (field names, required/optional, the deprecation note)
- url: https://github.com/microsoft/winget-pkgs/blob/master/doc/manifest/schema/1.6.0/installer.md
  why: the installer-manifest field list + the NOTE that installer-level keys (Scope/InstallModes/
       InstallerSwitches/UpgradeBehavior) may be at root (inherited) OR per-installer. Required installer
       node keys: Architecture, InstallerType, InstallerUrl, InstallerSha256.
  critical: "InstallModes community-repo requirement: MUST support silent AND silentWithProgress. Inno
       supports all three (interactive/silent/silentWithProgress). InstallerSwitches are auto-applied for
       InstallerType: inno but may be stated explicitly (harmless). ReleaseDate is YYYY-MM-DD."
- url: https://github.com/microsoft/winget-pkgs/blob/master/doc/manifest/schema/1.6.0/singleton.md
  why: "states 'singleton has been DEPRECATED in the Windows Package Manager Community Repository' — this is
       WHY this PRP uses multi-file, not singleton/merged. Also documents the defaultLocale fields
       (PackageLocale, Publisher, PackageName, License, ShortDescription, Tags, Moniker, ReleaseDate, LicenseUrl)."
- url: https://github.com/microsoft/winget-cli/blob/master/doc/windows/package-manager/package/manifest.md
  why: the canonical authoring guide: PackageIdentifier format, the version/defaultLocale/installer split,
       "When Inno is specified, the client automatically sets the silent/silentWithProgress behaviors", the
       100-char string limit, Publisher+PackageName should match the ARP entry.

# MUST READ — the COMPLETED in-source precedents (this task is their Winget translation)
- file: packaging/scoop/qmkonnect.json
  why: the closest analog — a channel manifest that downloads the SAME Inno asset, with the SAME metadata
       (homepage/license/description/tags) and a CI-filled hash placeholder. Mirror its metadata values.
  pattern: "version 0.2.8; description 'Cross-platform window activity notifier for QMK keyboards (Windows
       tray app)'; homepage https://github.com/dabstractor/qmkonnect; license MIT; url .../v0.2.8/
       QMKonnect-0.2.8-windows-x64.exe; 64-zero hash placeholder."
- file: packaging/scoop/README.md
  why: the EXACT structural template for packaging/winget/README.md (sections: What this is → Install → What
       it installs → Differences → For maintainers → See also; relative links to ../../spec/PRD.md,
       ../../.github/workflows/release.yml, ../windows/inno/QMKonnect.iss; parenthetical PRD/external_deps
       citations; the unsigned-installer note). Copy the voice.
- file: packaging/homebrew/README.md
  why: the macOS analogue of the README (per-user channel, custom-tap-vs-official caveat = the Winget
       "unverified publisher" caveat). Tone + section skeleton.

# MUST READ — the INPUT facts the manifest encodes (verified this session)
- file: packaging/windows/inno/QMKonnect.iss
  why: confirms the installer metadata + behavior the manifest must mirror. MyAppPublisher "Mulletware"
       (→ Publisher), MyAppName "QMKonnect" (→ PackageName + ARP DisplayName), AppId {{FAAE1F7A-...}}
       (stable in-place upgrade → UpgradeBehavior: install), DefaultDirName {localappdata}\Programs\QMKonnect
       (→ README "What it installs"), PrivilegesRequired=lowest (→ Scope: user), ArchitecturesAllowed=
       x64compatible (→ Architecture: x64), HKCU Run autostart, AUMID Mulletware.QMKonnect.
- file: .github/workflows/release.yml
  why: the `windows` job: version via `cargo metadata | ConvertFrom-Json` (NO leading v); Inno
       `QMKonnect-Setup.exe` renamed to `QMKonnect-<version>-windows-x64.exe` (the asset the manifest's
       InstallerUrl points at). NO `.sha256` sidecar (grep → none) ⇒ InstallerSha256 MUST be CI-filled.
  gotcha: "version has NO leading v; URL path adds the v (.../v0.2.8/...); asset filename uses bare version."
- file: Cargo.toml
  why: metadata source of truth — version 0.2.8, license MIT, description "Cross-platform window activity
       notifier for QMK keyboards", rust-version 1.88. (external_deps.md 'Version Source of Truth'.)

# MUST READ — the architecture decision + CI strategy this implements
- docfile: plan/007_fb356ba503b4/architecture/external_deps.md
  why: §4 Winget — package type (manifest YAML), publication (PR to microsoft/winget-pkgs via wingetcreate),
       per-user, required fields (PackageIdentifier/PackageVersion/Installers/InstallerType: inno/
       InstallerSwitches/SHA256/Publisher+homepage+license), CI (automated wingetcreate PR). §"CI Publishing
       Strategy" + §"Version Source of Truth" + §"Hashing" (Winget: InstallerSha256 in manifest).
  section: "4. Winget (Windows)" + "CI Publishing Strategy" + "Version Source of Truth" + "Hashing"

# MUST READ — PRD context (the feature + platform row this is the channel for)
- url: spec/PRD.md
  why: §4 F15 (community package-manager distribution); §5 platform row "Windows: Inno .exe · Scoop · Winget";
       §12 signing note ("Winget prompts 'unverified publisher'").

# REFERENCE — wingetcreate (the publishing tool P1.M3.T2.S2 wires into CI)
- url: https://github.com/microsoft/winget-create
  why: `wingetcreate new` (initial manifest + first PR), `wingetcreate update <id> -u <url> -v <ver> --submit`
       (CI refresh + PR), `wingetcreate validate`. Confirms multi-file is the generated format + that it sets
       InstallerSha256/ReleaseDate from the release.

# REFERENCE — sibling/contract references
- docfile: plan/007_fb356ba503b4/P1M3T1S1/PRP.md   (the COMPLETED Scoop manifest S1 — structural twin)
  why: the Scoop manifest + source README contract. THIS task is its Winget mirror (JSON→multi-YAML,
       innosetup-extract → InstallerType: inno-run).
- docfile: plan/007_fb356ba503b4/P1M3T2S1/research/notes.md   (this task's research findings)
  why: the `merged`-does-not-exist + singleton-deprecation findings; the field-level decisions; CI model.

# REFERENCE — naming facts (consistent with Scoop/Homebrew/AUR)
- file: src/platforms/mod.rs   (APP_AUMID = "Mulletware.QMKonnect" — the AUMID the Inno installer sets)
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
packaging/
  scoop/
    qmkonnect.json              # <<< manifest precedent (JSON; mirror its metadata values)
    README.md                   # <<< README structural precedent
  homebrew/
    Casks/qmkonnect.rb          # <<< manifest precedent (Ruby)
    README.md                   # <<< README precedent (per-user channel + caveat tone)
  windows/inno/QMKonnect.iss    # installer metadata (Publisher/Name/AppId/scope/autostart/AUMID)
.github/workflows/release.yml   # asset naming + version-from-cargo + NO sha256 sidecar
Cargo.toml                      # version=0.2.8 / license=MIT / description metadata source
LICENSE                         # the file LicenseUrl points at (no extension)
plan/007_fb356ba503b4/
  architecture/external_deps.md # §4 Winget + CI Publishing Strategy
  P1M3T1S1/PRP.md               # Scoop manifest S1 contract (structural twin)
# NEW (this task):  packaging/winget/{3 manifests + README.md}  (directory does not exist yet)
```

### Desired Codebase tree (files this task ADDS)

```bash
packaging/winget/
├── dabstractor.QMKonnect.yaml                     # NEW — version manifest
├── dabstractor.QMKonnect.locale.en-US.yaml        # NEW — defaultLocale (en-US) manifest
├── dabstractor.QMKonnect.installer.yaml           # NEW — installer manifest (x64 Inno)
└── README.md                                      # NEW — Winget install + "unverified publisher" + CI docs
```
(No other files. The CI wingetcreate job = P1.M3.T2.S2; the docs/installation.md Winget row = P1.M6.T1.S1.)

### Known Gotchas of our codebase & Library Quirks

```yaml
# CRITICAL (no "merged" type): ManifestType MUST be one of version | defaultLocale | locale | installer |
#   singleton. "merged" does NOT exist (verified across winget-pkgs schema 1.0–1.12). Do NOT write
#   "ManifestType: merged".

# CRITICAL (singleton is DEPRECATED for the community repo): do NOT use the single-file singleton format for
#   a NEW winget-pkgs submission — use the multi-file (version + defaultLocale + installer) form. wingetcreate
#   also emits multi-file. This is why this PRP uses 3 files, not 1.

# CRITICAL (InstallModes MUST include silentWithProgress): the community repo REQUIRES silent + silentWithProgress.
#   The contract's [interactive, silent] would FAIL validation. Use [interactive, silent, silentWithProgress]
#   (Inno supports all three).

# CRITICAL (org != local user): GitHub org = `dabstractor` (git remote get-url origin). The local Linux user
#   `dustin` (/home/dustin/...) is UNRELATED. PackageIdentifier = dabstractor.QMKonnect (lowercase org).
#   Do NOT write `Dustin.` or `dustin.`.

# CRITICAL (id Publisher part != display Publisher): the id's Publisher part `dabstractor` is an identity
#   token; the display Publisher field is "Mulletware" (Inno MyAppPublisher). They INTENTIONALLY differ —
#   ARP correlation is via Publisher+PackageName matching the installer's ARP entry, not the id prefix. Do NOT
#   set PackageIdentifier to Mulletware.QMKonnect (would diverge from every other F15 channel).

# CRITICAL (NO .sha256 sidecar): the release publishes only the renamed .exe. InstallerSha256 is a 64-zero
#   PLACEHOLDER that CI/wingetcreate fills from the downloaded .exe. Do NOT invent a sidecar URL.

# CRITICAL (version v-prefix): PackageVersion is bare "0.2.8" (NO leading v); the tag is "v0.2.8"; the URL
#   path adds the v (.../v0.2.8/...); the asset filename uses the bare version. Do NOT prepend v to
#   PackageVersion or to the asset name.

# GOTCHA (InstallerSha256 placeholder format): exactly 64 hex chars. Use 64 zeros
#   (0000...0000). wingetcreate overwrites it on update. An all-zero hash will FAIL a real `winget install`
#   (hash mismatch) — that's EXPECTED pre-CI; document it in the README + report.

# GOTCHA (ReleaseDate placeholder): RFC3339/ISO8601 "YYYY-MM-DD". Use "2025-08-01" as a placeholder;
#   wingetcreate update sets the real per-release date. Do NOT use a free-form date.

# GOTCHA (ShortDescription ≤ 100 chars): schema limits strings to 100 chars before a line break. Use the
#   Scoop manifest's phrasing (80 chars). Do NOT paste the full Cargo description there.

# GOTCHA (LicenseUrl): repo LICENSE file has NO extension → https://github.com/dabstractor/qmkonnect/blob/master/LICENSE
#   (not LICENSE.md).

# GOTCHA (Linux dev box): wingetcreate/winget are Windows-only. Validate YAML well-formedness with
#   python3+yaml (or grep invariants); DEFER wingetcreate validate / winget install --manifest to a Windows host.

# GOTCHA (scope): this task does NOT write .github/workflows/* (CI = P1.M3.T2.S2) or docs/installation.md
#   (P1.M6.T1.S1). It creates ONLY the 4 files under packaging/winget/.
```

## Implementation Blueprint

### Data models and structure
No code models. Four static text files (3 YAML manifests + 1 Markdown README). The manifests reference the
existing Inno installer asset + Cargo metadata; they introduce no types/structs.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CREATE packaging/winget/dabstractor.QMKonnect.yaml (VERSION manifest)
  - IMPLEMENT: the exact YAML from "What → File 1" (PackageIdentifier dabstractor.QMKonnect; PackageVersion
    0.2.8; DefaultLocale en-US; ManifestType version; ManifestVersion 1.6.0; the leading # comment block).
  - FOLLOW pattern: microsoft/winget-pkgs 1.6.0 version schema (PackageIdentifier/PackageVersion/DefaultLocale/
    ManifestType/ManifestVersion are the ONLY keys — version is intentionally minimal).
  - NAMING: file dabstractor.QMKonnect.yaml (matches the id; winget-pkgs convention).
  - PLACEMENT: packaging/winget/dabstractor.QMKonnect.yaml.

Task 2: CREATE packaging/winget/dabstractor.QMKonnect.locale.en-US.yaml (DEFAULTLOCALE manifest)
  - IMPLEMENT: the exact YAML from "What → File 2" (PackageLocale en-US; Publisher Mulletware; PublisherURL +
    PublisherSupportURL; PackageName QMKonnect; PackageUrl; License MIT; LicenseUrl .../blob/master/LICENSE;
    ShortDescription ≤100 chars; Description (full); Tags [qmk,keyboard,hid,tray]; Moniker qmkonnect;
    ReleaseDate 2025-08-01 placeholder; ManifestType defaultLocale; ManifestVersion 1.6.0).
  - FOLLOW pattern: 1.6.0 defaultLocale schema + the Scoop manifest's metadata values (homepage/license/desc).
  - NAMING: file dabstractor.QMKonnect.locale.en-US.yaml (winget-pkgs defaultLocale convention:
    <id>.locale.<locale>.yaml).
  - PRESERVE: PackageIdentifier + PackageVersion byte-identical to File 1.

Task 3: CREATE packaging/winget/dabstractor.QMKonnect.installer.yaml (INSTALLER manifest)
  - IMPLEMENT: the exact YAML from "What → File 3" (root: InstallerType inno; Scope user; InstallModes
    [interactive,silent,silentWithProgress]; InstallerSwitches {Silent /VERYSILENT, SilentWithProgress /SILENT};
    UpgradeBehavior install; ReleaseDate placeholder; Installers[Architecture x64, InstallerType inno,
    InstallerUrl .../v0.2.8/QMKonnect-0.2.8-windows-x64.exe, InstallerSha256 64-zero placeholder];
    ManifestType installer; ManifestVersion 1.6.0).
  - FOLLOW pattern: 1.6.0 installer schema; installer-level keys at ROOT (inherited by the single x64 node —
    matches wingetcreate output + the WindowsTerminal complex example).
  - NAMING: file dabstractor.QMKonnect.installer.yaml.
  - PRESERVE: PackageIdentifier + PackageVersion byte-identical to Files 1+2.

Task 4: CREATE packaging/winget/README.md
  - IMPLEMENT: sections 1–9 from "What → File 4". Title `# qmkonnect — Winget manifest (Windows)`.
  - MUST INCLUDE verbatim: `winget install dabstractor.QMKonnect`, `winget install qmkonnect` (Moniker),
    `winget upgrade dabstractor.QMKonnect`, `winget uninstall dabstractor.QMKonnect`; the "unverified
    publisher" warning + the SmartScreen bypass + the Scoop/direct-installer alternatives; the wingetcreate
    maintainer block (`wingetcreate new` first-time; `wingetcreate update dabstractor.QMKonnect -u <url>
    -v <version> -t <WINGET_GITHUB_TOKEN> --submit` per-release CI; P1.M3.T2.S2 pointer); the "What it
    installs" table; the See-also links.
  - FOLLOW pattern: packaging/scoop/README.md (section skeleton, relative links, tone, the unsigned note).
  - PLACEMENT: packaging/winget/README.md.

Task 5: VALIDATE (Linux-safe; no Windows/winget needed)
  - RUN (YAML well-formedness): python3 -c "import yaml,sys; [yaml.safe_load(open(f)) for f in sys.argv[1:]]"
      packaging/winget/*.yaml → expect exit 0 (no exception). If python3/yaml absent, fall back to the grep
      invariants in Validation Level 1.
  - RUN (grep invariants on the 3 manifests): see Validation Level 1 (the keys, the exact asset URL, the
      placeholder hash, the 3 distinct ManifestType values, cross-file id/version/locale consistency).
  - RUN (README content): see Validation Level 3.
  - NOTE: `wingetcreate validate packaging/winget/` + `winget install --manifest` are Windows-only → DEFER
      to a Windows host (note in report).

Task 6: NEVER do these (out of scope / forbidden)
  - DO NOT use "ManifestType: merged" (does not exist) or "ManifestType: singleton" (deprecated for the
      community repo). Use the multi-file version + defaultLocale + installer.
  - DO NOT use InstallModes without silentWithProgress (community-repo validation requires silent +
      silentWithProgress).
  - DO NOT set PackageIdentifier to Mulletware.* or Dustin.* (the org is dabstractor; the display Publisher
      is Mulletware — they intentionally differ).
  - DO NOT prepend `v` to PackageVersion or to the asset filename (version is bare; URL adds the v).
  - DO NOT invent a .sha256 sidecar URL (none exists; InstallerSha256 is CI-filled).
  - DO NOT edit .github/workflows/* (the CI wingetcreate job = P1.M3.T2.S2).
  - DO NOT edit docs/installation.md (the Winget install row = P1.M6.T1.S1).
  - DO NOT change any Rust source / Cargo.toml / other packaging dir.
  - DO NOT edit PRD.md, any tasks.json, or prd_snapshot.md.
```

### Implementation Patterns & Key Details
```yaml
# PATTERN (multi-file manifest triplet — the only non-deprecated form):
#   <id>.yaml                         ManifestType: version        (id + version + DefaultLocale)
#   <id>.locale.<locale>.yaml         ManifestType: defaultLocale  (metadata: Publisher/Name/License/Tags/...)
#   <id>.installer.yaml               ManifestType: installer      (InstallerType/Scope/InstallModes/Switches + Installers[])
#   All three share PackageIdentifier + PackageVersion; version's DefaultLocale == defaultLocale's PackageLocale.

# PATTERN (installer-level keys at ROOT, inherited by the single installer node): the schema NOTE says these
#   may be at root OR per-installer. With ONE x64 installer, root-level is cleanest + matches wingetcreate +
#   the WindowsTerminal complex example. The installer node carries only the per-binary keys
#   (Architecture/InstallerType/InstallerUrl/InstallerSha256).

# PATTERN (Inno silent switches): /VERYSILENT (fully silent) → Silent; /SILENT (progress, no prompts) →
#   SilentWithProgress. These are Inno's documented switches (jrsoftware.org/ishelp). InstallerType: inno
#   auto-applies them, but stating them explicitly is harmless + self-documenting.

# PATTERN (placeholders): PackageVersion "0.2.8", ReleaseDate "2025-08-01", InstallerSha256 64 zeros — ALL
#   refreshed by wingetcreate update on each release (CI P1.M3.T2.S2). The 64-zero hash WILL fail a real
#   `winget install` pre-CI; that is EXPECTED (document in README + report).

# ANTI-PATTERN: don't use singleton/merged. Don't omit silentWithProgress. Don't set the id to Mulletware.*
#   or prepend v to the version. Don't add AppsAndFeaturesEntries/ProductCode/SignatureSha256 (MSI/MSIX-only;
#   Publisher+PackageName already match the Inno ARP entry for correlation).
```

### Integration Points
```yaml
INPUT (release):     QMKonnect-<version>-windows-x64.exe (release.yml windows job, renamed from QMKonnect-Setup.exe)
INPUT (metadata):    Cargo.toml (version=0.2.8, license=MIT, description) + packaging/windows/inno/QMKonnect.iss
                     (Publisher Mulletware, Name QMKonnect, AppId, scope, AUMID)
OUTPUT (this task):  packaging/winget/{3 manifests + README.md}
PUBLISH TARGET:      microsoft/winget-pkgs  folder manifests/d/dabstractor/QMKonnect/<version>/
                     (external repo; wingetcreate opens the PR — initial manual, then CI per release)
CI (P1.M3.T2.S2):    on tag → wingetcreate update dabstractor.QMKonnect -u <release-exe-url> -v <version>
                     -t <WINGET_GITHUB_TOKEN> --submit → PR to winget-pkgs (refreshes PackageVersion +
                     InstallerUrl + InstallerSha256 + ReleaseDate)
DOCS SYNC (P1.M6.T1.S1): docs/installation.md Windows section gets a Winget row (NOT this task)
METADATA SOURCE:     Cargo.toml — single source of truth (external_deps.md 'Version Source of Truth')
PARALLEL (no conflict):
  - P1.M3.T1.S2 (Scoop bucket-README + update-manifest.ps1): different dir (packaging/scoop/). No overlap.
  - P1.M3.T2.S2 (Winget CI): consumes this task's manifest + README; writes the workflow. THIS task documents
    it but does NOT write it.
PLATFORM VALIDATION: Linux box proves the manifests' STRUCTURE via python3+yaml (or grep invariants). The
  live `wingetcreate validate` / `winget install --manifest` smoke test is Windows-only → deferred.
```

## Validation Loop

> Toolchain: the deliverables are 3 YAML files + 1 Markdown doc. `python3` with the `yaml` module checks YAML
> well-formedness on Linux (fallback: grep invariants). `wingetcreate`/`winget` are Windows-only.

### Level 1: Manifest structure invariants (Linux — no Windows/winget needed)
```bash
cd /home/dustin/projects/qmkonnect
D=packaging/winget
# YAML well-formedness (preferred — needs python3+yaml):
if python3 -c "import yaml" 2>/dev/null; then
    python3 -c "import yaml,sys,glob; [yaml.safe_load(open(f)) for f in glob.glob('$D/*.yaml')]" \
        && echo "all YAML well-formed"
fi
# Expected: "all YAML well-formed" (exit 0). A YAMLException means a syntax bug — read + fix.
# (Fallback if python3/yaml absent: rely on the grep invariants below.)

# Required keys + the 3 distinct ManifestType values + ManifestVersion:
grep -Hn '^ManifestType:' $D/*.yaml          # version / defaultLocale / installer (one each)
grep -Hn '^ManifestVersion: 1.6.0' $D/*.yaml # all three
grep -Hn '^PackageIdentifier: dabstractor.QMKonnect' $D/*.yaml   # all three
grep -Hn '^PackageVersion: 0.2.8' $D/*.yaml                       # all three
# Locale consistency: version's DefaultLocale == defaultLocale's PackageLocale:
grep -Hn 'DefaultLocale: en-US' $D/dabstractor.QMKonnect.yaml
grep -Hn 'PackageLocale: en-US' $D/dabstractor.QMKonnect.locale.en-US.yaml
# Installer essentials:
grep -n 'InstallerType: inno\|Scope: user\|Architecture: x64' $D/dabstractor.QMKonnect.installer.yaml
grep -n 'Silent: /VERYSILENT\|SilentWithProgress: /SILENT' $D/dabstractor.QMKonnect.installer.yaml
grep -n 'UpgradeBehavior: install' $D/dabstractor.QMKonnect.installer.yaml
grep -n 'silentWithProgress' $D/dabstractor.QMKonnect.installer.yaml   # MUST be present (community-repo req)
# The exact asset URL + the 64-zero placeholder hash:
grep -n 'releases/download/v0.2.8/QMKonnect-0.2.8-windows-x64.exe' $D/dabstractor.QMKonnect.installer.yaml
grep -nE 'InstallerSha256: 0{64}' $D/dabstractor.QMKonnect.installer.yaml
# Metadata:
grep -n 'Publisher: Mulletware\|PackageName: QMKonnect\|License: MIT\|Moniker: qmkonnect' $D/*.yaml
grep -nA4 '^Tags:' $D/dabstractor.QMKonnect.locale.en-US.yaml   # qmk/keyboard/hid/tray
# Anti-checks (must be ABSENT):
! grep -rn 'ManifestType: merged' $D && echo "no bogus 'merged' type"
! grep -rn 'ManifestType: singleton' $D && echo "no deprecated singleton"
! grep -rn 'InstallModes:\n- interactive\n- silent$' $D  # (silentWithProgress must accompany)
grep -L 'silentWithProgress' $D/dabstractor.QMKonnect.installer.yaml >/dev/null && echo "ERROR: missing silentWithProgress"
# Expected: every grep prints ≥1 hit; the two !grep print their OK lines; the final check prints nothing
# (no ERROR). If "missing silentWithProgress" prints → add it (community-repo validation requires it).
```

### Level 2: Cross-file consistency (Linux)
```bash
cd /home/dustin/projects/qmkonnect
D=packaging/winget
# PackageIdentifier identical across all 3:
ids=$(grep -h '^PackageIdentifier:' $D/*.yaml | sort -u); [ "$(echo "$ids" | wc -l)" -eq 1 ] && echo "id consistent: $ids"
# PackageVersion identical across all 3:
vers=$(grep -h '^PackageVersion:' $D/*.yaml | sort -u); [ "$(echo "$vers" | wc -l)" -eq 1 ] && echo "version consistent: $vers"
# Exactly 3 files, each a distinct ManifestType:
test "$(grep -h '^ManifestType:' $D/*.yaml | sort -u | wc -l)" -eq 3 && echo "3 distinct manifest types"
# Expected: all three echo lines print. Inconsistent id/version or a duplicate/missing ManifestType = a copy bug.
```

### Level 3: README content + scope (Linux)
```bash
cd /home/dustin/projects/qmkonnect
# README has the exact commands + the warning + the CI pointer:
grep -nE 'winget install dabstractor\.QMKonnect|winget install qmkonnect|winget upgrade dabstractor\.QMKonnect|winget uninstall dabstractor\.QMKonnect|unverified publisher|SmartScreen|wingetcreate (new|update)|WINGET_GITHUB_TOKEN|P1\.M3\.T2\.S2|PrivilegesRequired=lowest|Mulletware\.QMKonnect' packaging/winget/README.md
# Expected: hits for install/upgrade/uninstall; the unverified-publisher warning + SmartScreen bypass;
#   the wingetcreate new/update maintainer block; the WINGET_GITHUB_TOKEN secret; the P1.M3.T2.S2 pointer.
# Scope: ONLY 4 new files under packaging/winget/:
git status --short                           # Expected: packaging/winget/* (4 files) only
git diff --stat -- Cargo.toml .github/workflows/release.yml src/ docs/installation.md packaging/scoop packaging/homebrew
                                             # Expected: empty (no edits outside packaging/winget/)
```

### Level 4: Live validation (Windows host — OPTIONAL, deferred)
```powershell
# On a Windows box with wingetcreate + winget, AFTER CI has filled the real InstallerSha256 (or locally
# compute it: (Get-FileHash -Algorithm SHA256 QMKonnet-<ver>-windows-x64.exe).Hash):
cd <clone of dabstractor/qmkonnect>
# 1. Schema/style validation of the manifest triplet:
wingetcreate validate packaging\winget\
#    Expected: "Manifest validation succeeded" (no errors). Fixes any field-name/format mistakes.
# 2. (Optional) smoke install from the local manifest (needs the real hash):
winget install --manifest packaging\winget\dabstractor.QMKonnect.installer.yaml
#    Expected: downloads the .exe, runs the Inno installer (/SILENT), places QMKonnect.exe + icons in
#    %LOCALAPPDATA%\Programs\QMKonnect, Start Menu shortcut, HKCU Run autostart; tray app launches.
# (DEFERRED — wingetcreate/winget are Windows-only; the Linux dev box validates structure only.)
```

## Final Validation Checklist

### Technical Validation
- [ ] All 3 YAML files well-formed (`python3 + yaml`, or grep invariants if absent).
- [ ] `git diff --stat` shows ONLY 4 new files under `packaging/winget/`; nothing under `src/`, `Cargo.toml`,
      `.github/workflows/`, `docs/`, or other `packaging/` dirs changed.

### Feature Validation
- [ ] 3 manifests exist with `ManifestType: version` / `defaultLocale` / `installer` (NOT `merged`/`singleton`).
- [ ] All three carry `ManifestVersion: 1.6.0`, `PackageIdentifier: dabstractor.QMKonnect`, `PackageVersion: 0.2.8`.
- [ ] Installer manifest has `InstallerType: inno`, `Scope: user`, `Architecture: x64`,
      `InstallModes: [interactive, silent, silentWithProgress]`, `InstallerSwitches: {Silent: /VERYSILENT,
      SilentWithProgress: /SILENT}`, `UpgradeBehavior: install`, the exact asset URL, and a 64-zero hash placeholder.
- [ ] defaultLocale manifest has `Publisher: Mulletware`, `PackageName: QMKonnect`, `License: MIT` +
      `LicenseUrl .../blob/master/LICENSE`, `ShortDescription` (≤100 chars), `Tags: [qmk, keyboard, hid, tray]`,
      `Moniker: qmkonnect`, `ReleaseDate` (YYYY-MM-DD placeholder).
- [ ] README contains `winget install dabstractor.QMKonnect`, the "unverified publisher" warning + SmartScreen
      bypass, and the wingetcreate-PR workflow pointer (P1.M3.T2.S2).
- [ ] Locale consistent: version `DefaultLocale: en-US` == defaultLocale `PackageLocale: en-US`.

### Code Quality Validation
- [ ] Metadata matches Cargo.toml + QMKonnect.iss (version/license/desc; Publisher/Name/scope).
- [ ] Relative links in README resolve (`../../spec/PRD.md`, `../../.github/workflows/release.yml`,
      `../windows/inno/QMKonnect.iss`, `../../plan/007_fb356ba503b4/architecture/external_deps.md`).
- [ ] README mirrors the Scoop/Homebrew README tone + section skeleton.

### Documentation & Deployment
- [ ] README documents the placeholder hash + ReleaseDate (CI/wingetcreate fills them) and the WINGET_GITHUB_TOKEN secret.
- [ ] No new env vars / config keys in the app (pure packaging).
- [ ] The Windows-only `wingetcreate validate` / smoke install is noted as deferred in the report.

---

## Anti-Patterns to Avoid
- ❌ Don't use `ManifestType: merged` (doesn't exist) or `ManifestType: singleton` (deprecated for winget-pkgs). Use the multi-file version + defaultLocale + installer.
- ❌ Don't omit `silentWithProgress` from `InstallModes` — the community repo REQUIRES silent + silentWithProgress; the contract's `[interactive, silent]` would fail validation.
- ❌ Don't set `PackageIdentifier` to `Mulletware.*` or `Dustin.*`/`dustin.*` — the org is `dabstractor`; the display `Publisher` "Mulletware" intentionally differs from the id prefix (ARP correlation is via Publisher+PackageName).
- ❌ Don't prepend `v` to `PackageVersion` or to the asset filename — version is bare "0.2.8"; the URL path adds the v; the asset filename uses the bare version.
- ❌ Don't invent a `.sha256` sidecar URL — none is published; `InstallerSha256` is a 64-zero placeholder that CI/wingetcreate fills.
- ❌ Don't add `AppsAndFeaturesEntries`/`ProductCode`/`SignatureSha256` — those are MSI/MSIX-only; Publisher+PackageName already match the Inno ARP entry for winget list/upgrade correlation.
- ❌ Don't write the CI workflow (`.github/workflows/*`) — that's P1.M3.T2.S2. Don't edit `docs/installation.md` — that's P1.M6.T1.S1.
- ❌ Don't set `UpgradeBehavior: uninstallPrevious` — the Inno installer upgrades in place via its stable AppId; use `install`.
- ❌ Don't paste the full Cargo `description` into `ShortDescription` — it must be ≤100 chars.
- ❌ Don't edit Cargo.toml / Rust source / other packaging dirs / PRD.md / tasks.json / prd_snapshot.md.

---

## Confidence Score: 9/10

The 4 files are specced verbatim from the authoritative `microsoft/winget-pkgs` 1.6.0 schema (`version.md`/
`defaultLocale.md`/`installer.md`/`singleton.md`) + the contract, with two documented corrections (`merged` is
invalid + `singleton` is deprecated → use multi-file; `InstallModes` must include `silentWithProgress`). All
metadata is grep-confirmed from `Cargo.toml` (0.2.8/MIT/description), `packaging/windows/inno/QMKonnect.iss`
(Publisher Mulletware, Name QMKonnect, AppId, per-user scope, AUMID), and `.github/workflows/release.yml`
(asset `QMKonnect-<ver>-windows-x64.exe`, no leading-v, no sha256 sidecar). The README mirrors the COMPLETED
`packaging/scoop/README.md` + `packaging/homebrew/README.md`. Linux validates YAML well-formedness + cross-file
consistency via `python3+yaml`/grep; the Windows-only `wingetcreate validate`/smoke install is deferred (and
honestly noted). The 1-point reservation: the initial manual `wingetcreate new` submission to winget-pkgs
(and thus the exact canonical form winget-pkgs stores) happens on a Windows host in P1.M3.T2.S2 — the template
here is structurally complete and schema-valid, but a real submission may receive minor wingetcreate-driven
field reordering/casing (e.g., `PackageUrl` vs `PackageURL`) that the deferred Windows validation catches; the
field NAMES used here are taken verbatim from the 1.6.0 schema docs to minimize that risk.
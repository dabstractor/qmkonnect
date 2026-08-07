# Research findings — P1.M3.T1.S1 (Scoop app manifest for Inno installer)

Verified against the working tree + authoritative Scoop schema this session.

## The release artifact (Windows)
- CI `windows` job (`.github/workflows/release.yml`) builds `cargo build --release`,
  compiles `QMKonnect.iss` via Inno → `Output/QMKonnect-Setup.exe`, then renames to
  **`QMKonnect-<version>-windows-x64.exe`** (the release-asset name).
- Release URL pattern: `https://github.com/dabstractor/qmkonnect/releases/download/v<version>/QMKonnect-<version>-windows-x64.exe`
- **Version has NO leading `v`** (e.g. `0.2.8`); the tag IS `v0.2.8`. Cargo.toml version = `0.2.8`.
- **x64-only** (`ArchitecturesAllowed=x64compatible` in QMKonnect.iss).
- **Per-user, no admin** (`PrivilegesRequired=lowest`).
- **Not code-signed** (PRD §12 / external_deps.md §3: "Scoop unaffected, they don't enforce code-signing").
- **No `.sha256` sidecar is published**: `grep -niE 'sha256|sidecar|\.sha256|hash' release.yml` →
  NO matches. The release publishes only the renamed `.exe`. ⇒ the manifest's `autoupdate`
  must COMPUTE the hash (no `hash.url` sidecar to scrape).

## Inno installer internals (`packaging/windows/inno/QMKonnect.iss`) — what `innosetup:true` does NOT run
- Files copied to `{app}` (= `%LOCALAPPDATA%\Programs\QMKonnect`):
  `qmkonnect.exe` → **renamed `QMKonnect.exe`** (`DestName: {#MyAppExeName}`, `MyAppExeName="QMKonnect.exe"`),
  `Icon.ico`, `IconTray-dark.png`. (Also `set_aumid.ps1` → `{tmp}`, deleteafterinstall.)
- `[Registry]` writes HKCU `Run` value **`QMKonnect`** = `"<app>\QMKonnect.exe"` (default-on autostart).
- `[Icons]` Start Menu shortcut `{userprograms}\QMKonnect`.
- `[Code] CurStepChanged` runs `set_aumid.ps1` to set `System.AppUserModel.ID` =
  **`Mulletware.QMKonnect`** (`APP_AUMID`, src/platforms/mod.rs:138) on the Start Menu .lnk
  (for WinRT toast rendering — P1.M4.T2.S1 work).
- Registers an Add/Remove-Programs uninstall entry (Inno default) + `{app}\unins000.exe`.

### CRITICAL: `innosetup: true` means EXTRACT, not RUN
With `"innosetup": true` (Scoop schema: "True if the installer InnoSetup based"), Scoop uses
**`innounp`** to EXTRACT the `{app}` payload into `~\scoop\apps\qmkonnect\<version>\`. The
installer's CUSTOM LOGIC DOES NOT RUN:
- ❌ No HKCU `Run` autostart value (default-on autostart is LOST under Scoop).
- ❌ No Inno `[Icons]` Start Menu shortcut (Scoop creates its own via the `shortcuts` field).
- ❌ No Add/Remove-Programs uninstall entry (use `scoop uninstall qmkonnect`).
- ❌ No `CurStepChanged` AUMID PowerShell (the Scoop Start Menu .lnk won't carry the AUMID →
  WinRT toast notifications render generically until a future `post_install` sets it; P1.M4 scope).
- ✅ The exe + icon assets ARE extracted alongside each other (same relative layout as the
  installer's `{app}`), so the self-contained Rust tray app runs fine from the Scoop tree.

⇒ The manifest's `shortcuts` is what creates the Start Menu entry; autostart is opt-in via the
app's own tray "Open at Login" toggle (src/autostart.rs, which writes the same HKCU Run value
keyed to the current exe path — works correctly from the Scoop tree).

## Scoop manifest schema (authoritative: ScoopInstaller/Scoop `schema.json`)
- **Required top-level**: `version`, `homepage`, `license`. Plus `url` (required by the `then`
  clause when no architecture-split urls).
- `version` pattern: `^[\w\.\-+_]+$` (so `0.2.8` is valid; NO `v`).
- `license`: SPDX identifier string OR `{identifier, url}`. We have `"MIT"` (Cargo.toml).
- `innosetup`: boolean (no quotes): `"innosetup": true`.
- `hash`: pattern `^([a-fA-F0-9]{64}|(sha1|sha256|...):(...))$`. ⇒ a placeholder MUST be exactly
  64 hex chars. **64 zeros (`000…0`) is schema-valid AND safe**: Scoop checks the manifest hash
  against the downloaded file's computed hash at install; a zero placeholder FAILS that check
  (blocks install) until CI fills the real SHA256. Cannot let a bad/tampered binary through.
- `shortcuts`: array of arrays, each `[name, relative\path.exe, optional args, optional icon]`
  (minItems 2, maxItems 4). ⇒ `[["QMKonnect","QMKonnect.exe"]]`.
- `checkver`: a regex string OR `{ "github": "<repo-uri>", "regex": "<re>" }` (schema `checkver`
  object form). We use `{ "github": "https://github.com/dabstractor/qmkonnect", "regex": "v([\\d.]+)" }`.
- `autoupdate`: object; `url` uses the `$version` variable. If `hash` is OMITTED from autoupdate,
  Scoop COMPUTES it from the download on `scoop checkup`/autoupdate (standard for GitHub-release
  apps with no sidecar). We OMIT `hash` in autoupdate (no sidecar exists).

## Naming facts (consistent with the Homebrew/AUR precedent)
- GitHub org: **`dabstractor`** (source `dabstractor/qmkonnect`; bucket `dabstractor/scoop-qmkonnet`).
- Publisher/brand: **Mulletware** (Cargo authors, Inno `MyAppPublisher`, bundle id `io.mulletware.qmkonnect`).
- Bucket alias + URL: `scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnet`
  (explicit URL; `scoop bucket add qmkonnect` alone resolves to the implicit user bucket — the
  explicit URL matches the contract + external_deps.md §3 exactly).
- AUMID: `Mulletware.QMKonnect`.

## Windows config path (for README accuracy)
- `%APPDATA%\QMKonnect\{config.toml,rules.toml}` primary (src/platforms/windows.rs:491-492);
  `%LOCALAPPDATA%\QMKonnect\` secondary. (NOT under the Scoop tree — per-user, app-managed.)

## Sibling / parallel boundaries (no file overlap)
- **P1.M2.T1.S2** (Homebrew tap, in-flight parallel): owns `packaging/homebrew/{tap-README.md,update-cask.sh}`.
  No overlap with `packaging/scoop/`. Acts as a STRUCTURAL/PATTERN precedent (bucket-repo + deploy-key
  CI model, version v-prefix rule, BSD/GNU portability).
- **P1.M3.T1.S2** (Scoop bucket repo structure + autoupdate config — the real sibling): will own the
  BUCKET repo's README + publish/autoupdate SCRIPT (the `dabstractor/scoop-qmkonnet` repo scaffolding,
  analogous to Homebrew S2's `tap-README.md` + `update-cask.sh`). THIS task (S1) owns ONLY the manifest
  `packaging/scoop/qmkonnect.json` + the SOURCE-REPO packaging doc `packaging/scoop/README.md`. The
  manifest's `checkver`/`autoupdate` BLOCKS are what S2's automation consumes; I do NOT write a publish
  script here (that is S2's `update-manifest.sh`, mirroring `update-cask.sh`).
- **P1.M5.T1.S2** (CI publish jobs): wires the deploy key + pushes the manifest to the bucket on tag.
  This task only DOCUMENTS that flow in the README.

## Deliverables (this task)
1. `packaging/scoop/qmkonnect.json` — the manifest (full JSON given in the PRP).
2. `packaging/scoop/README.md` — source-repo packaging doc (Mode A: rides with the work).

## Validation reality (dev box is Linux)
- `jq . packaging/scoop/qmkonnect.json` → valid JSON (Linux-validatable).
- Schema conformance: validate `qmkonnect.json` against the authoritative Scoop `schema.json`
  (download once, validate with `python -m jsonschema` or `ajv`) — Linux-validatable.
- `scoop install` / `scoop checkver` are Windows-only → deferred to a Windows host (note in report),
  same platform-validation split as the Homebrew/AUR PRPs' target-OS deferrals.

## DRY / scope notes
- Do NOT add a `post_install` autostart/AUMID script in THIS manifest — it reaches into P1.M4
  (toast/AUMID) scope and can't be tested on the Linux box. The contract's field list does not
  include `post_install`. Document the autostart-not-default-on + AUMID deltas in the README's
  "Differences from the direct installer" section + as a documented future enhancement.
- Do NOT write the publish/bucket-update script (that's P1.M3.T1.S2).
- Do NOT add `bin` (a console shim) — a tray app's entry point is the Start Menu `shortcuts` entry,
  matching the contract's field list. (Optional `bin` is harmless but out of the contract's scope.)
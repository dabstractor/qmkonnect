# PRP — P1.M3.T1.S2: Create Scoop bucket repo structure + autoupdate config

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). **Pure packaging — no Rust/source/CI change.**
> **Two new files:** `packaging/scoop/bucket-README.md` (the README that lives at the ROOT of the
> external `dabstractor/scoop-qmkonnect` bucket repo) + `packaging/scoop/update-manifest.ps1` (a PowerShell
> script: download the release `.exe` → `Get-FileHash -Algorithm SHA256` → regex-patch
> `version`+`url`+`hash` in the manifest → validate JSON). Plus ONE surgical consistency-fix to S1's
> `packaging/scoop/README.md` (correct the `scoop-qmkonnet` → `scoop-qmkonnect` typo).
> **Scope:** the bucket-repo scaffolding + the publish-mutation script ONLY. The app manifest
> (`qmkonnect.json`) + the source-repo packaging doc (`README.md`) are sibling **P1.M3.T1.S1**
> (Implementing — its files already exist in `packaging/scoop/`). The CI push job is **P1.M5.T1.S2**.
> **Pattern:** this task is the Scoop analogue of the COMPLETED Homebrew S2
> (`packaging/homebrew/tap-README.md` + `packaging/homebrew/update-cask.sh`) — read both in full before
> writing; they are the structural/script template.

---

## Goal

**Feature Goal**: Stand up the **external bucket-repo half** of the Windows Scoop channel for QMKonnect
(PRD §4 F15; §5 — "Windows: Inno `.exe` (primary, no admin) · Scoop · Winget"). Deliver the bucket
repo's root README (install/update/uninstall + maintainer + deploy-key-CI sections) and the
PowerShell `update-manifest.ps1` that mechanically refreshes `version` + `url` + `hash` in
`qmkonnect.json` from a given release — the publish-mutation primitive that CI (P1.M5.T1.S2) drives
to clone→patch→commit→push the bucket on each tag.

**Deliverable** (2 new files + 1 one-line fix):
1. `packaging/scoop/bucket-README.md` — the README that will live at the root of the
   `dabstractor/scoop-qmkonnect` bucket repo (mirrors `packaging/homebrew/tap-README.md`).
2. `packaging/scoop/update-manifest.ps1` — a cross-platform PowerShell (Windows PowerShell 5.1 +
   pwsh 7) script mirroring `packaging/homebrew/update-cask.sh`: signature
   `./update-manifest.ps1 -Version <ver> [-Sha256 <hash>]`; downloads
   `QMKonnect-<version>-windows-x64.exe`, computes SHA256 (`Get-FileHash -Algorithm SHA256`),
   regex-patches `version` + `architecture.64bit.url` + `architecture.64bit.hash` in
   `qmkonnect.json` (leaving the `autoupdate` `$version` template untouched), re-parses the JSON to
   confirm well-formedness, and best-effort validates with `scoop checkver` if `scoop` is on PATH.
   It is a **PURE local file update — it does NOT push** (CI does the push).
3. **Consistency-fix** (surgical): in S1's `packaging/scoop/README.md`, replace the typo
   `scoop-qmkonnet` → `scoop-qmkonnect` everywhere (the bucket repo has exactly one name; see Naming
   Truth below).

**Success Definition**:
- `packaging/scoop/bucket-README.md` exists and mirrors the Homebrew tap-README's section skeleton,
  contains the EXACT bucket+install+update+uninstall commands with the **correct** bucket URL
  (`https://github.com/dabstractor/scoop-qmkonnect`), and documents the deploy key (`SCOOP_BUCKET_DEPLOY_KEY`)
  + CI flow pointing at P1.M5.T1.S2.
- `packaging/scoop/update-manifest.ps1` exists, is valid PowerShell (parses under `pwsh -NoProfile` if
  pwsh is available; otherwise the script text passes the grep invariants in Validation Level 1), and a
  **jq simulation** of its patching logic on a throwaway copy of `qmkonnect.json` produces valid JSON
  with the new `version`/`url`/`hash` and the `autoupdate` template UNCHANGED.
- `packaging/scoop/README.md` no longer contains `scoop-qmkonnet` (grep → 0 hits) and DOES contain
  `scoop-qmkonnect`.
- `git diff --stat` shows ONLY the 2 new files under `packaging/scoop/` + the one modified
  `packaging/scoop/README.md`. No Rust/source/Cargo/`.github/workflows/*`/other-packaging-dir changes.
- (Windows host, optional/deferred) `pwsh -File packaging/scoop/update-manifest.ps1 -Version 0.2.8`
  against a published release patches `qmkonnect.json` in place; `scoop bucket add … && scoop install
  qmkonnect` then installs — deferred to a Windows box (Scoop is Windows-only; the dev box is Linux).

## User Persona (if applicable)

**Target User**: a Windows end-user who installs software via **Scoop** and wants QMKonnect
managed/updated by `scoop update` alongside the direct Inno installer. (Same persona as S1 — this
task ships the *bucket repo* the persona `scoop bucket add`s.)

**Use Case**: `scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnect` →
`scoop install qmkonnect`. The bucket-README is what that user reads at the bucket repo's GitHub page.

**User Journey**: (1) open the `sustin/scoop-qmkonnect` repo; (2) copy the `scoop bucket add …` +
`scoop install qmkonnect` commands from the README; (3) install; (4) `scoop update qmkonnect` for
future releases (CI keeps the bucket's manifest current via this task's `update-manifest.ps1`).

**Pain Points Addressed**: gives Scoop users a discoverable, documented, CI-maintained bucket rather
than a bare manifest with no instructions.

## Why

- **F15 (PRD §4) requires a Scoop channel.** S1 ships the manifest; THIS task ships the bucket-repo
  README (the channel's front door) + the `update-manifest.ps1` publish primitive. CI (P1.M5.T1.S2)
  wires them together. Per external_deps.md §3 / PRD §12, Scoop is "unaffected (they don't enforce
  code-signing)", so the unsigned Inno installer is fine here.
- **Mirrors the proven Homebrew tap pattern.** `packaging/homebrew/tap-README.md` +
  `packaging/homebrew/update-cask.sh` (BOTH COMPLETE) are the exact macOS analogues of these two
  files. This task is the Scoop translation: bucket-README ← tap-README; `update-manifest.ps1` ←
  `update-cask.sh` (bash → PowerShell, `shasum` → `Get-FileHash`, `sed` → regex `-replace`).
- **`checkver`/`autoupdate` make it self-maintaining; `update-manifest.ps1` is the CI-driven refresh.**
  The manifest's `autoupdate` template + a zero-hash placeholder mean CI mechanically fills
  `version`+`url`+`hash` per release. This task's script is that mechanism (research confirms:
  top-level fields can be patched directly without invoking Scoop — the `autoupdate` block is only
  consumed by Scoop's own tooling).

## What

### Naming Truth (GROUND TRUTH — read before writing a single line)

- `git remote get-url origin` → `git@github.com:dabstractor/qmkonnect.git` ⇒ **GitHub org = `dabstractor`**.
  (The local Linux user is `dustin` in `/home/dustin/...`; that is UNRELATED to the org — do not
  confuse them.)
- Source repo = **`dabstractor/qmkonnect`**. Scoop bucket repo = **`dabstractor/scoop-qmkonnect`**
  (correct spelling, WITH the `c`). Homebrew tap repo = `dabstractor/homebrew-qmkonnect` (precedent).
- Confirmed by: `architecture/external_deps.md` §3, `tasks.json` (this item + downstream
  P1.M5.T1.S2), and the Homebrew precedent.
- **S1 shipped the typo `scoop-qmkonnet`** (missing `c`) in `packaging/scoop/README.md` (NOT in
  `qmkonnect.json`). S2 uses the correct `scoop-qmkonnect` everywhere and fixes S1's typo (Task 4).

### File 1 — `packaging/scoop/bucket-README.md` (author section-by-section)

Mirror the structure/tone of `packaging/homebrew/tap-README.md` (8 sections). Author:

1. **Title + one-line**: `# scoop-qmkonnect — Scoop bucket for QMKonnect` — Custom [Scoop] bucket for
   [QMKonnect](https://github.com/dabstractor/qmkonnect), the **Windows community channel** (PRD §4 F15,
   §5 — "Windows: Inno `.exe` (primary, no admin) · Scoop · Winget") alongside the primary direct
   Inno `.exe` installer.
2. **What this is**: a Scoop **bucket** — a git repo named `scoop-<name>` that holds
   [`bucket/qmkonnect.json`](bucket/qmkonnect.json). The manifest downloads the per-tag GitHub-release
   **Inno installer** `QMKonnect-<version>-windows-x64.exe` (the `windows` job in
   `.github/workflows/release.yml`, renamed from `QMKonnect-Setup.exe`) and **extracts** it via
   Scoop's `innosetup: true` flag (`innounp`, not the installer). **No Rust toolchain, no build
   deps** — the `.exe` statically links the CRT (`+crt-static`) and runs on any clean Windows 10/11
   x64 box. Scoop is **per-user** (no admin; matches `PrivilegesRequired=lowest`). x64-only
   (`ArchitecturesAllowed=x64compatible`). **Not code-signed** — fine for Scoop (PRD §12 /
   `architecture/external_deps.md` §3: "Scoop unaffected, they don't enforce code-signing"). Note the
   bucket layout convention: `bucket/<app>.json` + root `README.md` (the official
   `ScoopInstaller/BucketTemplate` — same shape as a Homebrew tap's `Casks/` + README).
3. **Install** (the EXACT commands — use the CORRECT bucket URL):
   ```bash
   # Add the bucket (alias MUST carry the explicit URL — `scoop bucket add qmkonnect` alone
   # resolves to an implicit user bucket, wrong):
   scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnect
   scoop install qmkonnect
   # Update to the latest release (CI keeps the bucket manifest current on each tag):
   scoop update qmkonnect
   # Uninstall:
   scoop uninstall qmkonnect
   ```
   `scoop update qmkonnect` pulls new releases automatically — the manifest's `checkver`/`autoupdate`
   blocks detect new GitHub tags.
4. **What it installs** (table) — concise (the SOURCE-repo `packaging/scoop/README.md` has the full
   table; this is a short summary + a pointer):

   | Where | What |
   |---|---|
   | `~\scoop\apps\qmkonnect\current\QMKonnect.exe` | The tray-app binary (extracted from the Inno installer) |
   | `~\scoop\apps\qmkonnect\current\Icon.ico`, `…\IconTray-dark.png` | Icon assets (extracted alongside the exe) |
   | Start Menu → **QMKonnect** | Scoop-managed shortcut (the manifest's `shortcuts`) |
   | `%APPDATA%\QMKonnect\{config.toml,rules.toml}` | Per-user config (app-managed, NOT under the Scoop tree) |
5. **Differences from the direct Inno installer** (concise — Scoop EXTRACTS via `innounp`, it does
   NOT run the installer; so autostart is OFF by default → enable **"Open at Login"** in the tray; no
   Add/Remove-Programs entry → use `scoop uninstall`; Start Menu shortcut has no AUMID → toasts render
   generically until a future `post_install`; install location is the Scoop apps tree, not
   `%LOCALAPPDATA%\Programs\QMKonnect\`). Point to the SOURCE-repo
   [`packaging/scoop/README.md`](https://github.com/dabstractor/qmkonnect/blob/master/packaging/scoop/README.md)
   §"Differences from the direct Inno installer" for the full detail.
6. **For maintainers — updating the manifest**: the bucket's `bucket/qmkonnect.json` ships with
   `version` and a 64-zero `hash` placeholder; each tagged release patches `version` + `url` + `hash`
   and pushes. The mechanical update is driven from the **source repo** by
   [`packaging/scoop/update-manifest.ps1`](https://github.com/dabstractor/qmkonnect/blob/master/packaging/scoop/update-manifest.ps1):
   it downloads the release `.exe`, computes its SHA256 (`Get-FileHash -Algorithm SHA256`), patches
   `version` + `url` + `hash` in `qmkonnect.json` (leaving the `autoupdate` `$version` template
   untouched), and re-validates the JSON. Show the usage:
   ```powershell
   # From a clone of dabstractor/qmkonnect (source repo):
   ./packaging/scoop/update-manifest.ps1 -Version 0.2.8                          # download + hash + patch + validate
   ./packaging/scoop/update-manifest.ps1 -Version 0.2.8 -Sha256 <precomputed>    # skip the download
   ./packaging/scoop/update-manifest.ps1 -Help
   ```
   The script is a **PURE local file update — it does NOT push** to the bucket (CI does the push).
7. **CI publishing (deploy key)**: new releases are pushed to this bucket **automatically** by the
   QMKonnect release workflow (wired in P1.M5.T1.S2). It authenticates to this repo with a GitHub
   **deploy key** (SSH, write access):
   1. Generate an SSH key pair.
   2. Add the **public** half to this bucket repo: *Settings → Deploy keys* (check "Allow write access").
   3. Store the **private** half as the `SCOOP_BUCKET_DEPLOY_KEY` Actions secret in
      [`dabstractor/qmkonnect`](https://github.com/dabstractor/qmkonnect).
   On a tag, CI loads that key into `ssh-agent` (e.g. `webfactory/agents/github-ssh-agent@v0.9.0`),
   clones `git@github.com:dabstractor/scoop-qmkonnect.git`, runs `update-manifest.ps1` against the source
   checkout, copies the patched `qmkonnect.json` into the clone as `bucket/qmkonnect.json`, commits,
   and pushes. This mirrors the AUR SSH-key model (`packaging/linux/aur/publish.sh`) and the Homebrew
   deploy-key model (`packaging/homebrew/tap-README.md` §"CI publishing") — see
   `architecture/external_deps.md` §"CI Publishing Strategy".
8. **See also**: source repo <https://github.com/dabstractor/qmkonnect>; install docs
   [`docs/installation.md`](https://github.com/dabstractor/qmkonnect/blob/master/docs/installation.md)
   (Windows section); Inno installer [`packaging/windows/inno/`](https://github.com/dabstractor/qmkonnect/tree/master/packaging/windows/inno);
   packaging spec [`spec/PACKAGING.md`](https://github.com/dabstractor/qmkonnect/blob/master/spec/PACKAGING.md)
   §3; sibling channels [`dabstractor/homebrew-qmkonnect`](https://github.com/dabstractor/homebrew-qmkonnect)
   (macOS) + [`packaging/linux/aur/`](https://github.com/dabstractor/qmkonnect/tree/master/packaging/linux/aur)
   (Linux).

### File 2 — `packaging/scoop/update-manifest.ps1` (reference implementation)

A cross-platform PowerShell script. **Model it on `packaging/homebrew/update-cask.sh`** (flow +
guards + scope clause), translated: `shasum`→`Get-FileHash`, `sed`→`[regex]::Replace` MatchEvaluator,
`brew audit`→`scoop checkver` (see Validation Truth below). The full reference:

```powershell
<#
.SYNOPSIS
  Update the QMKonnect Scoop manifest (qmkonnect.json) for a new release.

.DESCRIPTION
  Steps:
    1. (unless -Sha256 is given) download QMKonnect-<version>-windows-x64.exe from the GitHub
       release and compute its SHA256 (Get-FileHash -Algorithm SHA256).
    2. Regex-patch the top-level `version`, the concrete `architecture.64bit.url`, and
       `architecture.64bit.hash` in packaging/scoop/qmkonnect.json — WITHOUT touching the
       `autoupdate` `$version` URL template or `checkver`.
    3. Re-parse the file (Get-Content -Raw | ConvertFrom-Json) to confirm well-formed JSON, and
       Select-String-confirm the 3 patched values landed.
    4. Best-effort `scoop checkver` if scoop is on PATH (validation only; see notes).

  This script is a PURE LOCAL FILE UPDATE — it does NOT push to the bucket repo. The actual bucket
  publication is the CI job P1.M5.T1.S2, which runs this script against the source checkout, copies
  the patched qmkonnect.json into a clone of dabstractor/scoop-qmkonnect (as bucket/qmkonnect.json),
  commits, and pushes.

.PARAMETER Version
  The release version, WITHOUT a leading 'v' (e.g. "0.2.8"). Release TAGS are v-prefixed; the
  manifest version + the asset filename are bare. A leading 'v' is rejected.

.PARAMETER Sha256
  Optional pre-computed SHA256 (64 lowercase hex). When given, the download is skipped.

.EXAMPLE
  ./update-manifest.ps1 -Version 0.2.8
  ./update-manifest.ps1 -Version 0.2.8 -Sha256 86dcaa57254f4b7dd23595ef8e473432c3997df3b6c12621913d215b9ab1b216
  ./update-manifest.ps1 -Help

.NOTES
  DEPLOY KEY (CI publishing — wired in P1.M5.T1.S2):
    The bucket repo dabstractor/scoop-qmkonnect is pushed via a GitHub deploy key (SSH, write access).
    Generate a key pair, add the PUBLIC half to the bucket repo (Settings -> Deploy keys), and store
    the PRIVATE half as the SCOOP_BUCKET_DEPLOY_KEY Actions secret in dabstractor/qmkonnect. CI loads it
    into ssh-agent, then: git clone git@github.com:dabstractor/scoop-qmkonnect.git, run this script,
    cp qmkonnect.json into the clone as bucket/qmkonnect.json, commit, push. (Mirrors the AUR SSH-key
    model + Homebrew deploy-key model — see architecture/external_deps.md "CI Publishing Strategy".)

  VALIDATION TRUTH: `scoop checkup` (which the task contract mentions) checks the Scoop INSTALLATION
    environment health, NOT a manifest. The per-manifest validators are `scoop install <manifest>`
    (smoke install) and `scoop checkver <app>` (version + autoupdate template). This script's always-on
    check is the JSON re-parse + Select-String confirmation; `scoop checkver` is the optional, best-
    effort, Windows-only extra.

  PREREQUISITES: the GitHub release for <version> must already be published (step 1 downloads the .exe).
    PowerShell 5.1 (Windows) or 7+ (pwsh, cross-platform — GitHub Actions ubuntu/windows). Get-FileHash,
    Invoke-WebRequest, Select-String, ConvertFrom-Json are all built-in.
#>
[CmdletBinding()]
param(
    [string]$Version,
    [string]$Sha256,
    [switch]$Help
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($Help -or -not $Version) {
    # Print the comment-based help header (lines 2..end of this file's <# ... #> block) and exit.
    Get-Help $PSCommandPath -Detailed
    if (-not $Version) { exit 1 }
    exit 0
}

# Reject a leading 'v' (release TAGS are v-prefixed; manifest versions are not).
if ($Version -match '^v') {
    throw "Version must not have a leading 'v' (got '$Version'). Use '$($Version -replace '^v','')'."
}

$SRC_REPO = 'dabstractor/qmkonnect'
$BUCKET_REPO = 'dabstractor/scoop-qmkonnect'
$Manifest = Join-Path $PSScriptRoot 'qmkonnect.json'
if (-not (Test-Path -LiteralPath $Manifest)) { throw "Manifest not found at $Manifest" }

$AssetName = "QMKonnect-$Version-windows-x64.exe"
$DownloadUrl = "https://github.com/$SRC_REPO/releases/download/v$Version/$AssetName"

# --- Step 1: obtain the SHA256 (download+hash, or use -Sha256) ---
if ($Sha256) {
    Write-Host "==> Using provided SHA256 for v$Version (skipping download)"
} else {
    Write-Host "==> Downloading $DownloadUrl"
    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) $AssetName
    # -UseBasicParsing keeps PS 5.1 happy (harmless on 7); Invoke-WebRequest follows the GitHub 302.
    Invoke-WebRequest -UseBasicParsing -Uri $DownloadUrl -OutFile $tmp
    $Sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $tmp).Hash.ToLower()
    Remove-Item -LiteralPath $tmp -Force
}

# Sanity: SHA256 is exactly 64 lowercase hex chars.
if ($Sha256 -notmatch '^[0-9a-f]{64}$') { throw "SHA256 looks malformed: '$Sha256'" }
Write-Host "    version -> $Version"
Write-Host "    sha256  -> $Sha256"

# --- Step 2: regex-patch version + concrete url + hash (leave the autoupdate $version template) ---
$content = Get-Content -LiteralPath $Manifest -Raw

# Use [regex]::Replace with a MatchEvaluator so $Version/$Sha256 are ordinary variables (a plain
# -replace RHS would treat $<name> as a capture-group reference and silently drop the value).
$content = [regex]::Replace($content, '(?m)^(\s*"version"\s*:\s*")[^"]*(")', {
    param($m) $m.Groups[1].Value + $Version + $m.Groups[2].Value
})
$content = [regex]::Replace($content, '(?m)^(\s*"hash"\s*:\s*")[^"]*(")', {
    param($m) $m.Groups[1].Value + $Sha256 + $m.Groups[2].Value
})
# Patch the CONCRETE url only: skip the autoupdate template (whose value contains '$version').
$newUrl = "https://github.com/$SRC_REPO/releases/download/v$Version/$AssetName"
$content = [regex]::Replace($content, '(?m)^(\s*"url"\s*:\s*")([^"]*)(")', {
    param($m)
    if ($m.Groups[2].Value -notmatch '\$version') {
        $m.Groups[1].Value + $newUrl + $m.Groups[3].Value
    } else { $m.Groups[0].Value }   # leave the $version template untouched
})

Set-Content -LiteralPath $Manifest -Value $content -NoNewline

# --- Step 3: validate well-formed JSON + confirm the 3 patched values landed ---
$null = Get-Content -LiteralPath $Manifest -Raw | ConvertFrom-Json   # throws on malformed JSON
$check = Select-String -LiteralPath $Manifest -Pattern (
    "(`"version`": `"$Version`")",
    "(`"hash`": `"$Sha256`")",
    '(\$version)'   # the autoupdate template MUST still contain $version (proves we left it alone)
)
if ($check.Count -lt 3) { throw "Post-patch confirmation failed (expected version+hash+\`$version)." }
Write-Host "    patched $Manifest"

# --- Step 4: best-effort `scoop checkver` (Windows host with scoop installed) ---
$scoop = Get-Command scoop -ErrorAction SilentlyContinue
if ($scoop) {
    Write-Host "==> scoop checkver (manifest version + autoupdate template)"
    & scoop checkver $Manifest
} else {
    Write-Host "==> (scoop not found; skipping checkver — run on a Windows host with scoop installed)"
}

Write-Host "==> Done. Next (CI, P1.M5.T1.S2): copy $Manifest into a clone of $BUCKET_REPO as bucket/qmkonnect.json and push."
```

**Why these specific choices** (verified this session — see `research/notes.md`):
- **`[regex]::Replace` MatchEvaluator (not `-replace`):** a plain `-replace` RHS is a substitution
  string where `$Version`/`$Sha256` would be read as named-capture references and silently vanish.
  The scriptblock form treats them as ordinary closure variables. (Scoop/pwsh research.)
- **Targeted url patch (`-notmatch '\$version'`):** there are TWO `"url"` keys (concrete +
  autoupdate template); the guard rewrites only the concrete one.
- **No `ConvertTo-Json` rewrite:** it reorders keys / reflows / drifts the `"##"` comment out of its
  leading position (PS 5.1 also truncates at depth 2). The regex-patch preserves the file shape.
- **`Get-Content -Raw | ConvertFrom-Json` parse-only:** confirms well-formedness WITHOUT writing.
- **`-UseBasicParsing`:** PS 5.1 compatibility (harmless on pwsh 7).
- **`(?m)` multiline + `^\s*`:** matches the indented top-level/`64bit` lines; the concrete `version`
  and `hash` keys are unique in the manifest (no collision with `checkver`/`autoupdate`).
- **`-Help`/`Get-Help`:** PowerShell-idiomatic; the `.SYNOPSIS`/`.PARAMETER`/`.EXAMPLE` block makes
  `Get-Help update-manifest.ps1 -Detailed` the self-documenting entry point.

### File 3 (fix) — `packaging/scoop/README.md`: correct the bucket-name typo

S1's `packaging/scoop/README.md` contains the typo `scoop-qmkonnet` (missing `c`) in ~6 places
(Install command, the `scoop bucket add` Note, the "For maintainers" bullet, and cross-links). The
bucket repo only exists under ONE name, and every other source (external_deps.md §3, tasks.json, the
Homebrew precedent `homebrew-qmkonnect`) uses **`scoop-qmkonnect`**. Apply a literal, surgical
replacement: `scoop-qmkonnet` → `scoop-qmkonnect` (every occurrence). Do NOT touch anything else in
that file. (See Task 4 + Validation Level 3.)

### Success Criteria
- [ ] `packaging/scoop/bucket-README.md` exists with sections 1–8 and the exact bucket+install+
      update+uninstall commands using the CORRECT URL `https://github.com/dabstractor/scoop-qmkonnect`.
- [ ] `packaging/scoop/update-manifest.ps1` exists, parses as valid PowerShell, and its patching logic
      is PROVEN correct by the jq simulation (Validation Level 2): a throwaway copy of
      `qmkonnect.json` patched to a test version yields valid JSON with new version+url+hash and the
      `autoupdate` `$version` template UNCHANGED.
- [ ] `packaging/scoop/README.md` no longer contains `scoop-qmkonnet`; DOES contain `scoop-qmkonnect`.
- [ ] No edit to any Rust source / Cargo.toml / `.github/workflows/*` / other packaging dir; no
      creation of the bucket repo itself (that's an external repo + CI's job); no edit to
      `qmkonnect.json`'s structure (the script patches it at runtime, but S1 owns the file).

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior Scoop/PowerShell knowledge can create both files verbatim from the
"What" section (the bucket-README is specced section-by-section with exact commands; the PowerShell
script is given as a complete reference implementation), apply the one-line typo fix, and validate on
Linux via the jq simulation + grep invariants. The Windows-only `scoop`/`pwsh`-runtime smoke test is
explicitly deferred.

### Documentation & References

```yaml
# MUST READ — the two COMPLETED in-source precedents (this task is their Scoop translation)
- file: packaging/homebrew/tap-README.md
  why: the EXACT structural template for bucket-README.md (8 sections: What this is → Install → What
       it installs → Uninstall → For maintainers → CI publishing (deploy key) → Path/See also). Tone,
       parenthetical PRD/spec citations, inline #comments in command blocks — copy the voice.
- file: packaging/homebrew/update-cask.sh
  why: the EXACT script-flow template for update-manifest.ps1 (download → hash → patch → confirm →
       best-effort audit; leading-v guard; hash-format validation; the "PURE local file update — does
       NOT push" scope clause; the deploy-key doc block). Translate bash→pwsh, shasum→Get-FileHash,
       sed→[regex]::Replace.

# MUST READ — the INPUT this task's script patches (S1's output, already in the tree)
- file: packaging/scoop/qmkonnect.json
  why: the manifest the script patches. Fields: top-level `version`, `architecture.64bit.{url,hash}`
       (concrete), `autoupdate.architecture.64bit.url` (TEMPLATE with `$version` — DO NOT patch),
       `checkver {github,regex}` (static — DO NOT patch), 64-zero hash placeholder (CI-fill).
  gotcha: "there are TWO `"url"` keys — the concrete one (patch it) and the autoupdate `$version`
           template (leave it). The script's `-notmatch '\$version'` guard handles this."

# MUST READ — the architecture decision + the CI strategy this implements
- docfile: plan/007_fb356ba503b4/architecture/external_deps.md
  why: §3 Scoop — package type (app manifest JSON), publication (custom bucket
       `scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnect`), per-user, required
       fields, CI (push manifest to bucket on tag). §"CI Publishing Strategy" (deploy key,
       clone→update→push). §"Version Source of Truth" + §"Hashing" (Scoop: hash in manifest).
  section: "3. Scoop (Windows)" + "CI Publishing Strategy" + "Version Source of Truth" + "Hashing"

# MUST READ — PRD context (the feature + platform row this is the channel for)
- url: spec/PRD.md
  why: §4 F15 (community package-manager distribution); §5 platform row "Windows: Inno .exe · Scoop ·
       Winget"; §12 signing note ("Scoop unaffected, they don't enforce code-signing")
  section: "h2.3 (4. Top-Level Feature Set, F15)" + "h2.4 (5. Supported Platforms)" + "§12"

# MUST READ — release-asset facts the script hardcodes (verified)
- file: .github/workflows/release.yml
  why: the `windows` job: version via `cargo metadata | ConvertFrom-Json` (NO leading v); Inno
       `QMKonnect-Setup.exe` renamed to `QMKonnect-<version>-windows-x64.exe` (the asset the script
       downloads); the `publish` job creates the GitHub Release. NO `.sha256` sidecar
       (`grep sha256|sidecar` → none) ⇒ the script MUST compute the hash itself.
  gotcha: "version has NO leading v; the URL path adds the v (.../v$Version/...); the asset filename
           uses the bare version. Do NOT prepend v to $Version."

# MUST READ — Inno installer facts (what innounp extracts vs what does NOT run)
- file: packaging/windows/inno/QMKonnect.iss
  why: confirms the asset payload + the installer logic that `innosetup:true` SKIPS (HKCU Run
       autostart, Start Menu, ARP entry, AUMID). These are the "Differences" the bucket-README documents.
  pattern: "DestName QMKonnect.exe (= MyAppExeName); {app}={localappdata}\Programs\QMKonnect;
            HKCU Run 'QMKonnect' autostart; [Code]CurStepChanged runs set_aumid.ps1 → Mulletware.QMKonnect;
            ArchitecturesAllowed=x64compatible; PrivilegesRequired=lowest"

# MUST READ — the sibling/contract references
- docfile: plan/007_fb356ba503b4/P1M3T1S1/PRP.md
  why: the CONTRACT for the manifest + source README (S1). Confirms the manifest structure, the
       64-zero hash placeholder, the `scoop-qmkonnet` TYPO (this task's File 3 fixes it), and the
       innosetup:true extraction semantics. S1 owns qmkonnect.json + README.md; this task does NOT
       duplicate them.
- docfile: plan/007_fb356ba503b4/P1M2.T1.S2/PRP.md   (the completed Homebrew S2 — structural twin)
  why: the Homebrew tap-README + update-cask.sh contract. THIS task is its Scoop mirror 1:1.
- file: plan/007_fb356ba503b4/tasks.json   (read-only — the orchestrator owns it)
  why: confirms the bucket repo name `dabstractor/scoop-qmkonnect` (this item + downstream P1.M5.T1.S2),
       the CI secret `SCOOP_BUCKET_DEPLOY_KEY`, and the scope split (S1=manifest+README,
       S2=bucket-README+update-manifest.ps1, P1.M5.T1.S2=CI push job).

# REFERENCE — Scoop bucket conventions (external)
- url: https://github.com/ScoopInstaller/BucketTemplate
  why: the canonical bucket-repo layout (`bucket/<app>.json` + root `README.md`) — confirms the
       bucket-README's "What this is" section + CI's `cp qmkonnect.json bucket/qmkonnect.json` step.
- url: https://github.com/ScoopInstaller/Scoop/wiki/App-Manifest-Autoupdate
  why: the `autoupdate` template (`$version`) + `checkver` mechanics; confirms top-level fields can be
       patched directly (the `autoupdate` block is only for Scoop's own tooling).
- url: https://github.com/ScoopInstaller/Scoop/wiki/Buckets
  why: `scoop bucket add <name> <url>` semantics (explicit URL required for a non-known bucket).

# REFERENCE — naming facts (consistent with Homebrew/AUR)
- file: Cargo.toml   (version=0.2.8, license=MIT, description — the metadata source of truth)
- file: src/platforms/mod.rs   (APP_AUMID = "Mulletware.QMKonnect" — the AUMID the Inno installer sets)

# REFERENCE — this task's own research notes (full findings)
- docfile: plan/007_fb356ba503b4/P1M3T1S2/research/notes.md
  why: the `scoop checkup` ≠ manifest-validation correction; bucket structure; pwsh gotchas
       (Get-FileHash, Invoke-WebRequest 302, AVOID ConvertTo-Json, the `-replace` `$`-substitution
       trap → use MatchEvaluator); CI deploy-key pattern; the two-url targeting guard.
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
packaging/
  scoop/
    qmkonnect.json                  # S1's app manifest (INPUT — the script patches this). READ for fields.
    README.md                       # S1's source-repo doc (INPUT — this task FIXES its scoop-qmkonnet typo).
  homebrew/
    tap-README.md                   # <<< PRIMARY structural precedent for bucket-README.md >>>
    update-cask.sh                  # <<< PRIMARY script-flow precedent for update-manifest.ps1 >>>
    README.md                       # cross-reference (the macOS analogue of S1's scoop README)
    Casks/qmkonnect.rb
  windows/inno/QMKonnect.iss        # Inno installer facts (exe name, autostart, AUMID)
.github/workflows/release.yml       # asset naming + version-from-cargo + NO sha256 sidecar
Cargo.toml                          # version/license/description metadata source
plan/007_fb356ba503b4/
  architecture/external_deps.md     # §3 Scoop + CI Publishing Strategy
  P1M3T1S1/PRP.md                   # S1 contract (manifest + source README)
  P1M2.T1.S2/PRP.md                 # Homebrew S2 contract (the structural twin)
  P1M3T1S2/research/notes.md        # this task's research findings
# NEW (this task):
packaging/scoop/
  bucket-README.md                  # README for the dabstractor/scoop-qmkonnect bucket repo root
  update-manifest.ps1               # PowerShell: download → Get-FileHash → regex-patch → validate
```

### Desired Codebase tree (files this task ADDS / MODIFIES)

```bash
packaging/scoop/
├── bucket-README.md        # NEW  — bucket repo root README (install/update/uninstall + deploy-key CI)
├── update-manifest.ps1     # NEW  — PowerShell publish-mutation script (patches qmkonnect.json)
├── qmkonnect.json          # (S1, unchanged structurally)
└── README.md               # MODIFIED — fix `scoop-qmkonnet` → `scoop-qmkonnect` typo (S1's file)
```
(No other files. The CI push job = P1.M5.T1.S2; the manifest + source README = P1.M3.T1.S1.)

### Known Gotchas of our codebase & Library Quirks
```powershell
# CRITICAL (org != local user): the GitHub org is `dabstractor` (`git remote get-url origin`). The local
#   Linux user is `dustin` (the dev path /home/dustin/...) — UNRELATED to the org; do not confuse them, and
#   do NOT write `dustin` as the org. Source repo = dabstractor/qmkonnect;
#   bucket repo = dabstractor/scoop-qmkonnect (WITH the 'c'). S1's README has the typo `scoop-qmkonnet`
#   (missing 'c') — this task fixes it (File 3).

# CRITICAL (`scoop checkup` does NOT validate a manifest): the task CONTRACT says "optionally validates
#   with scoop checkup", but `scoop checkup` inspects the Scoop INSTALLATION environment (admin status,
#   Defender, helper tools, long paths), NOT a manifest. The real validators are `scoop install
#   <manifest>` (smoke) and `scoop checkver <app>`. The script's always-on check is the JSON re-parse +
#   Select-String confirm; the optional best-effort check is `scoop checkver` (NOT `scoop checkup`).
#   Document this honestly in the script header + bucket-README.

# CRITICAL (the `-replace` RHS `$`-substitution trap): a plain `-replace <pat>, "$Version"` treats
#   `$Version` as a named-capture reference and DROPS it. Use `[regex]::Replace($content, $pat,
#   { param($m) …$Version… })` (MatchEvaluator scriptblock) where $Version/$Sha256 are closure vars.

# CRITICAL (two `"url"` keys): the manifest has a CONCRETE architecture.64bit.url (patch it) AND an
#   autoupdate.architecture.64bit.url TEMPLATE containing `$version` (LEAVE it — patching it would
#   freeze the template). The script's MatchEvaluator checks `-notmatch '\$version'` to target only the
#   concrete one. Verify post-patch that `$version` STILL appears in the file (Validation Level 2).

# CRITICAL (AVOID ConvertTo-Json to rewrite): it reorders keys, reflows whitespace, drifts the `"##"`
#   comment key, and PS 5.1 truncates at depth 2. Patch with regex; re-parse with ConvertFrom-Json only
#   to VALIDATE (never to write).

# CRITICAL (NO .sha256 sidecar): the release publishes only the renamed .exe (grep release.yml → none).
#   The script MUST download the .exe and compute SHA256 itself (Get-FileHash). Do NOT invent a sidecar URL.

# CRITICAL (version v-prefix): version has NO leading v ("0.2.8"); the tag is "v0.2.8"; the URL path
#   adds the v (.../v0.2.8/...); the asset filename uses the bare version. The script REJECTS a leading v
#   in -Version (mirror update-cask.sh's guard). Do NOT prepend v to $Version or to the asset name.

# GOTCHA (PS 5.1 vs 7): `Invoke-WebRequest` needs `-UseBasicParsing` on PS 5.1 (harmless on pwsh 7).
#   `Get-FileHash`, `Select-String`, `ConvertFrom-Json` work on both. `Get-Help` works on both. Target
#   Windows PowerShell 5.1 + pwsh 7 (GitHub Actions ubuntu-latest ships pwsh; windows-latest ships 5.1
#   with optional pwsh). The script must run under both.

# GOTCHA (scope): this task is a PURE local file update — the script does NOT push; CI (P1.M5.T1.S2)
#   does the clone→run→cp→commit→push. Do NOT add a git-push step to the script. Do NOT edit
#   .github/workflows/* (that's P1.M5.T1.S2). Do NOT create the bucket repo (external repo + CI's job).

# GOTCHA (Linux dev box): the box is Linux. `pwsh` may or may not be installed. Validate the script's
#   CORRECTNESS via the jq simulation (Validation Level 2) + the grep invariants (Level 1); if `pwsh`
#   is present, also run `pwsh -NoProfile -Command "Get-Command -Syntax …"` / `-Help`. Defer the real
#   `scoop install`/`scoop checkver` + a live `update-manifest.ps1` run to a Windows host.
```

## Implementation Blueprint

### Data models and structure
No code models. Two static text files (a Markdown README + a PowerShell script). The script reads +
regex-patches the S1 manifest's scalar fields (`version`, `architecture.64bit.url`,
`architecture.64bit.hash`) per release; it introduces no types/structs.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CREATE packaging/scoop/bucket-README.md
  - IMPLEMENT: sections 1–8 from "What → File 1". Title `# scoop-qmkonnect — Scoop bucket for QMKonnect`.
  - MUST INCLUDE verbatim (CORRECT URL):
      scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnect
      scoop install qmkonnect
      scoop update qmkonnect
      scoop uninstall qmkonnect
    AND the deploy-key block (secret SCOOP_BUCKET_DEPLOY_KEY; webfactory ssh-agent; clone
    git@github.com:dabstractor/scoop-qmkonnect.git; run update-manifest.ps1; cp qmkonnect.json →
    bucket/qmkonnect.json; commit; push; downstream P1.M5.T1.S2).
    AND the maintainer usage block (`./packaging/scoop/update-manifest.ps1 -Version 0.2.8 [-Sha256 <h>]`).
    AND the "Differences from the direct Inno installer" summary (autostart off→tray toggle; no ARP
    entry; no AUMID on shortcut; Scoop-tree location) with a pointer to the source-repo scoop README.
  - FOLLOW pattern: packaging/homebrew/tap-README.md (section skeleton, tone, inline #comments).
  - NAMING: org dabstractor; bucket dabstractor/scoop-qmkonnect (WITH the 'c'); exe QMKonnect.exe; AUMID
    Mulletware.QMKonnect; source repo dabstractor/qmkonnect.
  - PLACEMENT: packaging/scoop/bucket-README.md.

Task 2: CREATE packaging/scoop/update-manifest.ps1
  - IMPLEMENT: the reference script from "What → File 2" (CmdletBinding param block; comment-based
    .SYNOPSIS/.PARAMETER/.EXAMPLE/.NOTES help; leading-v guard; download via Invoke-WebRequest
    -UseBasicParsing; Get-FileHash -Algorithm SHA256; [regex]::Replace MatchEvaluator patching of
    version + concrete url + hash; the -notmatch '\$version' url guard; ConvertFrom-Json re-parse +
    Select-String confirm; best-effort scoop checkver; the "PURE local file update — does NOT push"
    scope clause + the deploy-key doc in .NOTES).
  - FOLLOW pattern: packaging/homebrew/update-cask.sh (flow + guards + scope clause + deploy-key doc).
  - NAMING: params -Version / -Sha256 / -Help; script file update-manifest.ps1 (NOT .sh — this is
    PowerShell; the S1 README referenced it as update-manifest.sh by mistake — the .ps1 extension is
    correct per the contract).
  - DEPENDENCIES: targets the S1 manifest at $PSScriptRoot/qmkonnect.json (sibling file).
  - PLACEMENT: packaging/scoop/update-manifest.ps1.

Task 3: VALIDATE the script's correctness (Linux-safe; no Windows/scoop needed)
  - RUN (parse, if pwsh present): pwsh -NoProfile -Command "& { . ./packaging/scoop/update-manifest.ps1 -Help }"
      → expect the help text (exit 0). If pwsh is absent, skip and rely on the jq simulation + grep.
  - RUN (jq simulation — PROVES the patching logic on a throwaway copy): see Validation Level 2.
  - RUN (grep invariants on the script text): see Validation Level 1.
  - NOTE: a live download+patch+`scoop checkver` run is Windows-only → DEFER to a Windows host.

Task 4: FIX the bucket-name typo in S1's packaging/scoop/README.md
  - IMPLEMENT: literal replacement `scoop-qmkonnet` → `scoop-qmkonnect` (every occurrence; ~6 places:
    the Install command, the `scoop bucket add` Note, the "For maintainers" bullet, cross-links).
  - DO NOT touch anything else in that file (its manifest references, the differences section, etc.).
  - JUSTIFY: the bucket repo has exactly one name; external_deps.md §3 + tasks.json + the Homebrew
    precedent all use `scoop-qmkonnect`; S1's PRP introduced the typo. An unfixed typo 404s on
    `scoop bucket add`.
  - VERIFY: `grep -c 'scoop-qmkonnet' packaging/scoop/README.md` → 0; `grep -c 'scoop-qmkonnect'` → >0.

Task 5: NEVER do these (out of scope / forbidden)
  - DO NOT edit qmkonnect.json's structure (S1 owns it; the script patches it at runtime only).
  - DO NOT create the manifest or the source-repo README (those are S1, already in the tree).
  - DO NOT add a git-push step to update-manifest.ps1 or edit .github/workflows/* (CI = P1.M5.T1.S2).
  - DO NOT create the bucket repo itself (it's an EXTERNAL repo; CI clones/pushes it).
  - DO NOT use `scoop checkup` as the validator — it checks the Scoop environment, not a manifest.
      Use the JSON re-parse (always-on) + `scoop checkver` (best-effort); document the distinction.
  - DO NOT use ConvertTo-Json to rewrite the manifest; patch with regex (avoids reflow/`##`-drift).
  - DO NOT use a plain `-replace` RHS with `$Version`/`$Sha256` (substitution trap); use a
      MatchEvaluator scriptblock.
  - DO NOT prepend `v` to -Version or to the asset name (version is bare; URL path adds the v).
  - DO NOT patch the `autoupdate` `$version` template or the `checkver` block (leave them static).
  - DO NOT change any Rust source / Cargo.toml / other packaging dir / docs outside packaging/scoop/.
  - DO NOT edit PRD.md, any tasks.json, or prd_snapshot.md.
```

### Implementation Patterns & Key Details
```powershell
# PATTERN: the three-field regex patch (MatchEvaluator avoids the -replace `$`-substitution trap).
$content = Get-Content -LiteralPath $Manifest -Raw
$content = [regex]::Replace($content, '(?m)^(\s*"version"\s*:\s*")[^"]*(")', {
    param($m); $m.Groups[1].Value + $Version + $m.Groups[2].Value })
$content = [regex]::Replace($content, '(?m)^(\s*"hash"\s*:\s*")[^"]*(")', {
    param($m); $m.Groups[1].Value + $Sha256 + $m.Groups[2].Value })
# url: patch ONLY the concrete one (skip the $version template):
$content = [regex]::Replace($content, '(?m)^(\s*"url"\s*:\s*")([^"]*)(")', {
    param($m)
    if ($m.Groups[2].Value -notmatch '\$version') { $m.Groups[1].Value + $newUrl + $m.Groups[3].Value }
    else { $m.Groups[0].Value } })
Set-Content -LiteralPath $Manifest -Value $content -NoNewline

# PATTERN: validate WITHOUT rewriting (ConvertFrom-Json parse-only + Select-String confirm).
$null = Get-Content -LiteralPath $Manifest -Raw | ConvertFrom-Json   # throws on malformed JSON
# confirm version+hash landed AND the $version template survived (proves we left autoupdate alone).

# PATTERN: scope split (mirror update-cask.sh). update-manifest.ps1 = PURE local file update;
#   CI (P1.M5.T1.S2) = clone bucket → run script → cp qmkonnect.json bucket/qmkonnect.json → commit → push.
```
```text
# PATTERN: the S1/S2/CI three-way split (identical to the Homebrew channel).
#   S1 (manifest + source README)         = packaging/scoop/{qmkonnect.json, README.md}
#   S2 (THIS task: bucket README + script)= packaging/scoop/{bucket-README.md, update-manifest.ps1}
#   CI (P1.M5.T1.S2: push to bucket)      = .github/workflows/release.yml scoop-bucket job
#   The manifest's checkver/autoupdate blocks are what CI + the script keep in sync.

# PATTERN (deploy key): SCOOP_BUCKET_DEPLOY_KEY secret (SSH private half) in dabstractor/qmkonnect; public
#   half as write deploy key on dabstractor/scoop-qmkonnect. CI loads via webfactory/agents/github-ssh-agent.

# ANTI-PATTERN: don't invoke `scoop autoupdate` — there's no such command; `scoop checkver -u` does it,
#   but we patch directly (works without Scoop installed in CI). Keep the autoupdate $version template
#   consistent with the patched top-level url or checkver flags a mismatch (our manifest already is).
```

### Integration Points
```yaml
INPUT (S1):          packaging/scoop/qmkonnect.json (the manifest the script patches)
INPUT (release):     QMKonnect-<version>-windows-x64.exe (release.yml windows job, renamed from QMKonnect-Setup.exe)
OUTPUT (this task):  packaging/scoop/{bucket-README.md, update-manifest.ps1} + (fix) packaging/scoop/README.md
BUCKET REPO:         dabstractor/scoop-qmkonnect (external; bucket-README.md → its root README; CI cp's
                     qmkonnect.json → bucket/qmkonnect.json)
CI (P1.M5.T1.S2):    on tag → load SCOOP_BUCKET_DEPLOY_KEY (ssh-agent) → clone bucket → run
                     update-manifest.ps1 → cp manifest into bucket/qmkonnect.json → commit → push
METADATA SOURCE:     Cargo.toml (version=0.2.8, license=MIT, description) — single source of truth
                     (external_deps.md §"Version Source of Truth"); version via `cargo metadata`
DOCS SYNC (P1.M6):   docs/installation.md Windows section + top-level README link the bucket (NOT this task)
PARALLEL (no conflict):
  - P1.M3.T1.S1 (Scoop manifest, Implementing): owns qmkonnect.json + README.md. THIS task consumes
    qmkonnect.json (the script's target) and fixes ONE typo in README.md. No structural overlap.
  - P1.M5.T1.S2 (CI push job, Planned): consumes update-manifest.ps1 + bucket-README.md. THIS task
    documents that job but does NOT write the workflow.
PLATFORM VALIDATION: Linux box proves the script's CORRECTNESS via the jq simulation + grep invariants
  (+ a pwsh parse if pwsh is installed). The live download+patch+`scoop checkver` + `scoop install`
  smoke test is Windows-only → deferred to a Windows host (note in report).
```

## Validation Loop

> Toolchain: the deliverables are a Markdown doc + a PowerShell script. `jq` (used by S1) simulates
> the script's JSON patching on Linux. `pwsh` is OPTIONAL (present → parse/help check; absent → rely on
> the jq simulation + grep). Scoop itself is Windows-only.

### Level 1: Script text invariants (Linux — no pwsh/scoop needed)
```bash
cd /home/dustin/projects/qmkonnect
S=packaging/scoop/update-manifest.ps1
# Core invariants the reference script MUST contain:
grep -nE 'param\(' "$S"                                   # CmdletBinding param block
grep -nE '\[string\]\$Version|\[string\]\$Sha256|switch\]\$Help' "$S"   # the 3 params
grep -nE "Version -match '\^v'|leading 'v'" "$S"          # leading-v guard
grep -nE 'Get-FileHash -Algorithm SHA256' "$S"            # hash computation
grep -nE "regex\]::Replace" "$S"                          # MatchEvaluator (not bare -replace)
grep -nE "notmatch '\\\\\$version'|\\\$version" "$S"      # the concrete-vs-template url guard
grep -nE 'ConvertFrom-Json' "$S"                          # JSON re-parse validation
grep -nE 'scoop checkver|scoop checkup' "$S"              # best-effort validation (+ the honest note)
grep -nE 'does NOT push|PURE' "$S"                        # scope clause
grep -nE 'SCOOP_BUCKET_DEPLOY_KEY' "$S"                   # deploy-key doc
grep -nE 'dabstractor/scoop-qmkonnect' "$S"                    # CORRECT bucket name (with the 'c')
! grep -nE 'scoop-qmkonnet[^c]' "$S" && echo "no typo in script"   # NO scoop-qmkonnet typo
# Expected: every grep prints ≥1 hit; the final !grep prints "no typo in script".
# If pwsh IS installed, also:
if command -v pwsh >/dev/null; then
    pwsh -NoProfile -Command "Get-Command -Syntax ./packaging/scoop/update-manifest.ps1" 2>&1 | head
    pwsh -NoProfile -Command "[ScriptBlock]::Create((Get-Content -Raw ./packaging/scoop/update-manifest.ps1)) | Out-Null; 'parses OK'"
fi
# Expected: "parses OK" (exit 0). A parse error means a PowerShell syntax bug — read + fix.
```

### Level 2: jq simulation of the patching logic (Linux — PROVES correctness on a throwaway copy)
```bash
cd /home/dustin/projects/qmkonnect
WORK=$(mktemp -d); cp packaging/scoop/qmkonnect.json "$WORK/qmkonnect.json"
M="$WORK/qmkonnect.json"
NEWVER=9.9.9; NEWURL="https://github.com/dabstractor/qmkonnect/releases/download/v${NEWVER}/QMKonnect-${NEWVER}-windows-x64.exe"; NEWHASH=86dcaa57254f4b7dd23595ef8e473432c3997df3b6c12621913d215b9ab1b216
# Mirror the script's three edits with jq (architecture.64bit.url + hash; top-level version):
jq --arg v "$NEWVER" --arg u "$NEWURL" --arg h "$NEWHASH" '
  .version=$v
  | .architecture."64bit".url=$u
  | .architecture."64bit".hash=$h' "$M" > "$M.new" && mv "$M.new" "$M"
jq . "$M" >/dev/null && echo "patched JSON is valid"
# Assertions (mirror the script's post-patch confirms):
jq -e --arg v "$NEWVER" '.version==$v' "$M"                                                  # version patched
jq -e --arg u "$NEWURL" '.architecture."64bit".url==$u' "$M"                                 # concrete url patched
jq -e --arg h "$NEWHASH" '.architecture."64bit".hash==$h' "$M"                               # hash patched
jq -e '.autoupdate.architecture."64bit".url|test("\\$version")' "$M"                         # template UNCHANGED
jq -e '.checkver.github=="https://github.com/dabstractor/qmkonnect"' "$M"                         # checkver UNCHANGED
jq -e 'has("innosetup") and .innosetup==true' "$M"                                           # structure preserved
rm -rf "$WORK"
# Expected: "patched JSON is valid" + every jq -e prints `true` (exit 0). This proves the script's
# THREE targeted edits leave a valid manifest with the autoupdate template + checkver intact.
```

### Level 3: bucket-README content + the typo fix (Linux)
```bash
cd /home/dustin/projects/qmkonnect
# bucket-README has the exact commands with the CORRECT bucket URL + the deploy-key/CI sections:
grep -nE 'scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnect|scoop install qmkonnect|scoop update qmkonnect|scoop uninstall qmkonnect|SCOOP_BUCKET_DEPLOY_KEY|update-manifest\.ps1|Open at Login|innounp|Mulletware.QMKonnect|dabstractor/scoop-qmkonnect|P1\.M5\.T1\.S2' packaging/scoop/bucket-README.md
# Expected: hits for the bucket+install+update+uninstall commands; the SCOOP_BUCKET_DEPLOY_KEY deploy-
# key block; the update-manifest.ps1 maintainer usage; the autostart/AUMID deltas; the CI pointer.
! grep -nE 'scoop-qmkonnet[^c]' packaging/scoop/bucket-README.md && echo "no typo in bucket-README"
# S1's README: typo fully replaced:
test "$(grep -c 'scoop-qmkonnet' packaging/scoop/README.md)" -eq 0 && echo "typo gone from README.md"
test "$(grep -c 'scoop-qmkonnect' packaging/scoop/README.md)" -gt 0 && echo "correct name present"
# git scope: ONLY the 2 new files + the 1 modified README.md:
git diff --stat
git diff --stat -- Cargo.toml .github/workflows/release.yml src/   # Expected: empty
git status --short                                                  # Expected: packaging/scoop/* only
```

### Level 4: Live run (Windows host — OPTIONAL, deferred)
```powershell
# On a Windows box with Scoop + pwsh, AFTER a release is published:
cd <clone of dabstractor/qmkonnect>
# 1. Run the script against a real release (patches packaging/scoop/qmkonnect.json in place):
.\packaging\scoop\update-manifest.ps1 -Version 0.2.8
#    Expected: downloads QMKonnect-0.2.8-windows-x64.exe, prints the SHA256, patches version+url+hash,
#    re-parses OK, and (if scoop is on PATH) runs `scoop checkver`.
# 2. Smoke-install the patched manifest (proves the hash is now valid + the extraction works):
scoop install .\packaging\scoop\qmkonnect.json
#    Expected: extracts via innounp, places QMKonnect.exe + icons in ~\scoop\apps\qmkonnect\current\,
#    creates a Start Menu "QMKonnect" shortcut. (SKIPPABLE on the Linux dev box — scoop is Windows-only.)
# 3. (CI sanity, once P1.M5.T1.S2 lands) scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnect ; scoop install qmkonnect
```

## Final Validation Checklist

### Technical Validation
- [ ] Validation Levels 1–3 pass on the Linux box (script invariants + jq simulation + content review).
- [ ] `pwsh -NoProfile` parse returns "parses OK" (if pwsh present); otherwise the jq simulation proves
      the patching logic and the grep invariants prove the script text.
- [ ] `git diff --stat` shows ONLY `packaging/scoop/bucket-README.md` (new),
      `packaging/scoop/update-manifest.ps1` (new), and `packaging/scoop/README.md` (modified).

### Feature Validation
- [ ] `bucket-README.md` has sections 1–8; the EXACT bucket+install+update+uninstall commands with the
      CORRECT URL `https://github.com/dabstractor/scoop-qmkonnect`; the deploy-key block
      (`SCOOP_BUCKET_DEPLOY_KEY`, `webfactory/agents/github-ssh-agent`, clone→patch→cp→commit→push,
      downstream P1.M5.T1.S2); the maintainer `update-manifest.ps1` usage; the "Differences" summary.
- [ ] `update-manifest.ps1` has: `-Version`/`-Sha256`/`-Help` params; leading-v guard;
      `Get-FileHash -Algorithm SHA256`; `[regex]::Replace` MatchEvaluator patching of version + concrete
      url + hash; the `-notmatch '\$version'` url guard (leaves the autoupdate template untouched);
      `ConvertFrom-Json` re-parse validation; best-effort `scoop checkver`; the "PURE local file update —
      does NOT push" scope clause; the deploy-key doc.
- [ ] `packaging/scoop/README.md` no longer contains `scoop-qmkonnet`; contains `scoop-qmkonnect`.

### Code Quality Validation
- [ ] `bucket-README.md` mirrors the Homebrew `tap-README.md` structure/tone; `update-manifest.ps1`
      mirrors `update-cask.sh`'s flow/guards/scope clause (bash→pwsh translation).
- [ ] No git-push step in the script; no `.github/workflows/*` edit (CI = P1.M5.T1.S2); no bucket-repo
      creation (external + CI's job); no manifest/source-README creation (S1).
- [ ] Uses `scoop checkver` (not `scoop checkup`) for manifest validation, with an honest note that
      `scoop checkup` checks the Scoop environment, not a manifest.
- [ ] Uses `[regex]::Replace` MatchEvaluator (not bare `-replace`) to avoid the `$`-substitution trap;
      does NOT use `ConvertTo-Json` to rewrite the manifest (regex-patch only).
- [ ] Naming consistent: org `dabstractor`, bucket `dabstractor/scoop-qmkonnect` (WITH the 'c'), source
      `dabstractor/qmkonnect`, exe `QMKonnect.exe`, AUMID `Mulletware.QMKonnect`.

### Documentation & Deployment
- [ ] `bucket-README.md` IS the documentation (Mode A): install/update/uninstall, what-it-installs,
      the innounp-extraction differences, the maintainer script usage, the deploy-key CI flow, and See-also.
- [ ] Report notes that the live `scoop install`/`scoop checkver` + a live `update-manifest.ps1` run are
      Windows-only and were deferred (validated via jq simulation + grep on the Linux box).

---

## Anti-Patterns to Avoid
- ❌ Don't use `scoop checkup` to validate the manifest — it checks the Scoop *environment* (admin,
  Defender, helper tools, long paths), not a manifest. Use the JSON re-parse (always-on) + `scoop
  checkver` (best-effort); document the distinction. (The task contract's "scoop checkup" wording is wrong.)
- ❌ Don't use a bare `-replace <pat>, "$Version"` — the RHS is a substitution string; `$Version` is
  read as a named-capture reference and silently dropped. Use `[regex]::Replace` with a MatchEvaluator
  scriptblock (`{ param($m); …$Version… }`).
- ❌ Don't rewrite the manifest with `ConvertTo-Json` — it reorders keys, reflows whitespace, drifts the
  `"##"` comment, and PS 5.1 truncates at depth 2. Patch with regex; re-parse with `ConvertFrom-Json`
  only to validate.
- ❌ Don't patch the `autoupdate` `$version` URL template or the `checkver` block — they're static.
  Patch only `version`, `architecture.64bit.url` (concrete), `architecture.64bit.hash`. Guard the url
  edit with `-notmatch '\$version'` so the template survives. Verify `$version` still appears post-patch.
- ❌ Don't add a git-push step to the script, edit `.github/workflows/*`, or create the bucket repo —
  those are CI's job (P1.M5.T1.S2). The script is a PURE local file update; CI does clone→run→cp→push.
- ❌ Don't prepend `v` to `-Version` or to the asset name — the version is bare (`0.2.8`); the URL path
  adds the `v` (`.../v$Version/...`). Reject a leading `v` in `-Version` (mirror update-cask.sh).
- ❌ Don't invent a `.sha256` sidecar URL — the release publishes none (verified). Compute the hash from
  the download (`Get-FileHash`).
- ❌ Don't use the typo `scoop-qmkonnet` (missing `c`) anywhere — the bucket repo is
  `dabstractor/scoop-qmkonnect` (external_deps.md §3, tasks.json, Homebrew precedent). Fix S1's typo (Task 4).
- ❌ Don't confuse the local Linux user `dustin` with the GitHub org — the org is **`dabstractor`**
  (`git remote get-url origin` → `git@github.com:dabstractor/qmkonnect.git`); the local user `dustin` is just
  the dev-box path `/home/dustin/...` and is unrelated. Every URL/repo ref must use `dabstractor`.
- ❌ Don't claim `scoop install`/`scoop checkver` validation on a Linux box — Scoop is Windows-only.
  Validate the script's *logic* via the jq simulation + grep invariants; defer the live run to Windows.
- ❌ Don't edit any Rust source / Cargo.toml / `.github/workflows/*` / other packaging dir / docs outside
  `packaging/scoop/`, or PRD.md / tasks.json / prd_snapshot.md.

---

## Confidence Score: 9/10

Both deliverables are fully specified: the bucket-README is specced section-by-section (mirroring the
COMPLETED Homebrew `tap-README.md`, read in full) with exact commands + the deploy-key/CI blocks; the
`update-manifest.ps1` is given as a complete, correct reference implementation (mirroring the COMPLETED
Homebrew `update-cask.sh`, read in full) with every PowerShell gotcha handled (MatchEvaluator to dodge
the `-replace` `$`-trap; `-notmatch '\$version'` to target only the concrete url; `ConvertFrom-Json`
parse-only to validate without rewriting; `Get-FileHash`; `-UseBasicParsing` for PS 5.1). Every naming
fact is resolved against ground truth (`git remote` → org `dabstractor`; bucket `scoop-qmkonnect` per
external_deps.md §3 + tasks.json + Homebrew precedent; S1's `scoop-qmkonnet` typo explicitly fixed).
The two non-obvious contract corrections — `scoop checkup` ≠ manifest validation, and the bucket-name
typo — are both surfaced and handled. The Linux box validates script-correctness via the jq simulation
(which proves the three targeted edits leave a valid manifest with the autoupdate template + checkver
intact) + grep invariants + (if present) a `pwsh` parse. The 1-point reservation is for: (a) a live
`update-manifest.ps1` run + `scoop install`/`scoop checkver` smoke test being Windows-only (deferred),
and (b) the MatchEvaluator regex anchoring (`(?m)^(\s*"url"…)`) depending on the manifest's exact
indentation from S1 — verified against the S1 file in the tree (4-space indent), but a future manifest
re-indent would need the regex re-checked (low risk; the post-patch `Select-String`/`$version` confirm
catches a silent miss).
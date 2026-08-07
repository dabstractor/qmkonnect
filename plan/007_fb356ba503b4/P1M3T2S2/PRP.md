# PRP — P1.M3.T2.S2: Create Winget publishing automation via wingetcreate

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). **Pure packaging — no Rust/source/CI change.**
> **Two deliverables, both under `packaging/winget/`:** (1) `submit.ps1` — a cross-platform PowerShell
> script that downloads a release `.exe`, computes its SHA256, and invokes
> `wingetcreate update dabstractor.QMKonnect --urls "<url>|<hash>" --version <ver>` with EITHER a
> `--submit --token <PAT>` mode (opens the per-release PR to `microsoft/winget-pkgs`) or a local
> `--out <dir>` review mode (no PR); and (2) a **new section appended to** `packaging/winget/README.md`
> (the submission workflow + PAT requirements + the `wingetcreate new` one-time manual first submission
> + the `vedantmgoyal9/winget-releaser@v2` alternative). **Scope:** the publish automation script + the
> submission doc ONLY. The 3 Winget manifest YAMLs are sibling **P1.M3.T2.S1** (Implementing — its 3
> manifests already landed in `packaging/winget/`; its `README.md` has NOT landed yet, so this task
> APPENDS a section to it). The CI job is **P1.M5.T2.S1** (Planned).
> **Pattern:** this task is the Winget analogue of the COMPLETED Scoop S2 (`packaging/scoop/update-manifest.ps1`)
> and Homebrew S2 (`packaging/homebrew/update-cask.sh`) — read both in full before writing; they are the
> script-flow template. **BUT the publication model differs** (see "Critical Architectural Difference"
> below) — read that before writing `submit.ps1`.

---

## ⚠️ CRITICAL ARCHITECTURAL DIFFERENCE FROM SCOOP/HOMEBREW S2 (read first)

Scoop S2 (`update-manifest.ps1`) and Homebrew S2 (`update-cask.sh`) publish to a repo **WE OWN**
(`dabstractor/scoop-qmkonnect`, `dabstractor/homebrew-qmkonnect`) via a CI **deploy-key git push**.
Their scripts are therefore **"PURE local file update — does NOT push"**: they regex-patch the LOCAL
manifest file, and a separate CI job clones the external repo + copies the patched file + commits + pushes.

**Winget is fundamentally different:** the publication target is `microsoft/winget-pkgs` — an EXTERNAL
repo we do NOT own. You cannot `git push` to it. Publication is a **pull request**, and `wingetcreate
update --submit --token` opens that PR **atomically** (it forks `microsoft/winget-pkgs` →
`<token-owner>/winget-pkgs`, pushes a branch, opens `fork → microsoft:main`). Therefore:

1. **`submit.ps1` does NOT regex-patch the local S1 manifest YAMLs.** `wingetcreate update` fetches the
   EXISTING manifest from winget-pkgs and updates `PackageVersion` + `InstallerUrl` + `InstallerSha256`
   from the args. The local S1 YAML triplet is ONLY the metadata template for the one-time manual
   `wingetcreate new` (first submission) — it is NOT an input `submit.ps1` patches. (Verified: S1's
   `dabstractor.QMKonnect.installer.yaml` carries a 64-zero `InstallerSha256` placeholder that wingetcreate
   overwrites on the winget-pkgs copy, not the local copy.)
2. **`submit.ps1` actually OPENS THE PR** (in `-Submit` mode) — it is NOT a pure local patcher. This is
   the intended design per the contract: *"runs `wingetcreate update dabstractor.QMKonnect --version {ver}
   --urls {installer_url}|{sha256} --submit --token {token}` (or generates the manifest locally without
   `--submit` for review)."*
3. **There is NO separate bucket/tap-style README** (unlike Scoop `bucket-README.md` / Homebrew
   `tap-README.md`). The doc deliverable is ONE section **appended** to S1's `packaging/winget/README.md`.

---

## Goal

**Feature Goal**: Stand up the **publishing automation** for the Winget channel (PRD §4 F15; §5 —
"Windows: Inno `.exe` (primary, no admin) · Scoop · Winget"). Deliver `packaging/winget/submit.ps1` —
the per-release primitive that turns a tagged GitHub Release into a winget-pkgs PR via `wingetcreate` —
plus a maintainer-facing "Publishing to microsoft/winget-pkgs" doc section (PAT setup, the one-time
manual first submission, the per-release `submit.ps1`/`winget-releaser` options). This is the Winget
equivalent of the COMPLETED Scoop `update-manifest.ps1` + Homebrew `update-cask.sh`, adapted for the
external-PR publication model. It is **ready for CI integration in P1.M5.T2.S1** (which wires it into
`.github/workflows/release.yml`).

**Deliverable** (1 new file + 1 section appended to a sibling file):
1. `packaging/winget/submit.ps1` — cross-platform PowerShell (Windows PowerShell 5.1 + pwsh 7). Signature
   `./submit.ps1 -Version <ver> [-Sha256 <hash>] [-Submit] [-Token <PAT>] [-OutDir <dir>] [-Help]`.
   Downloads `QMKonnect-<version>-windows-x64.exe`, computes SHA256 (`Get-FileHash -Algorithm SHA256`),
   and invokes `wingetcreate update dabstractor.QMKonnect --urls "<url>|<sha256>" --version <version>`
   with either `--token <PAT> --submit` (open PR) or `--out <OutDir>` (local review, no PR). Builds the
   wingetcreate argv as an **array splat** (so the literal `|` in `URL|HASH` is one process arg, not a
   PowerShell pipeline), redacts the token from its own log line, and degrades to a "would-run" preview
   when `wingetcreate` is absent (so the script is testable on Linux without the tool installed).
2. **Append** a `## Publishing to microsoft/winget-pkgs (for maintainers)` section to
   `packaging/winget/README.md` (S1's file): the classic PAT (`public_repo` → `WINGET_GITHUB_TOKEN`
   secret), the one-time manual `wingetcreate new`, the per-release `submit.ps1` usage (Option A,
   `windows-latest`), the `vedantmgoyal9/winget-releaser@v2` alternative (Option B, `ubuntu-latest`),
   and the versioning truth (bare version, leading-`v` handling).

**Success Definition**:
- `packaging/winget/submit.ps1` exists, parses as valid PowerShell under the installed `pwsh 7.6.2`
  (`[ScriptBlock]::Create((Get-Content -Raw …)) | Out-Null` → exit 0), and `./submit.ps1 -Help` prints
  the comment-based help.
- A **mock-wingetcreate test** (a PATH shim that captures argv; runs on this Linux box) PROVES the
  command construction for BOTH modes: review mode captures `update dabstractor.QMKonnect --urls
  "<exact-url>|<hash>" --version 9.9.9 --out <dir>`; submit mode captures the same plus
  `--token <PAT> --submit`. The `|` survives as ONE argv token (the splat fix); the script's own stdout
  does NOT contain the PAT (redaction works).
- `packaging/winget/README.md` contains the appended section with the exact `wingetcreate new`,
  `submit.ps1 -Submit`, and `winget-releaser@v2` snippets, the `WINGET_GITHUB_TOKEN`/`public_repo`
  PAT block, and the leading-`v` gotcha for the action.
- `git diff --stat` shows ONLY the new `packaging/winget/submit.ps1` + the modified
  `packaging/winget/README.md` (the appended section). No Rust/source/Cargo/`.github/workflows/*`/
  other-packaging-dir changes; the 3 S1 manifest YAMLs are untouched.
- (Windows/CI host, optional/deferred) `winget install Microsoft.WingetCreate` then a real
  `./submit.ps1 -Version <published> -Submit -Token $env:WINGET_GITHUB_TOKEN` opens a PR — deferred to
  the P1.M5.T2.S1 CI host (wingetcreate is Windows-first; the dev box is Linux).

## User Persona (if applicable)

**Target User**: a **QMKonnect maintainer** (or the CI release pipeline) publishing each new release to
the Winget community channel. The end-user persona (`winget install dabstractor.QMKonnect`) is served by
sibling **S1** (the manifest) — THIS task serves the *publisher*.

**Use Case**: after cutting a `v*` tag, run `./packaging/winget/submit.ps1 -Version 0.2.8 -Submit -Token
$env:WINGET_GITHUB_TOKEN` (locally for a manual release, or in the CI `winget` job in P1.M5.T2.S1). It
opens a PR to `microsoft/winget-pkgs`; the winget-pkgs maintainers merge it; `winget upgrade
dabstractor.QMKonnect` then serves the new version to users.

**User Journey**: (1) maintainer reads the README section → creates the classic PAT (`public_repo`) →
stores `WINGET_GITHUB_TOKEN`; (2) one-time: runs `wingetcreate new <url> --token <PAT> --submit`
interactively (fills metadata from S1's manifests) → first PR merged; (3) each release: runs
`submit.ps1 -Submit` (or CI does via P1.M5.T2.S1, OR the `winget-releaser` action) → per-release PR.

**Pain Points Addressed**: gives maintainers a documented, mechanical, CI-ready path to keep the Winget
channel current (currently manual + undocumented). Mirrors the proven Scoop/Homebrew publish-script
shape, adapted for winget-pkgs's PR model.

## Why

- **F15 (PRD §4) requires a Winget channel.** S1 ships the manifest template; THIS task ships the publish
  automation that keeps winget-pkgs current on each tag. CI (P1.M5.T2.S1) wires it into the release
  workflow. Per `architecture/external_deps.md` §4 / PRD §12, Winget prompts "unverified publisher"
  (unsigned Inno `.exe`) — S1's README documents the end-user side; this task documents the *publisher* side.
- **Mirrors the proven Scoop/Homebrew S2 publish-script shape**, adapted for winget-pkgs's external-PR
  model (see "Critical Architectural Difference" above). `packaging/scoop/update-manifest.ps1` and
  `packaging/homebrew/update-cask.sh` (BOTH COMPLETE) are the script-flow templates: download → hash →
  invoke-the-tool → best-effort-validate → scope-clause-in-the-header. The Winget translation:
  `shasum`/`Get-FileHash` stays; `sed`/regex-patch is replaced by delegating to `wingetcreate update`
  (which does the manifest mutation + the PR atomically).
- **Two documented paths (contract requirement):** the primary `wingetcreate` CLI (`submit.ps1`) and the
  alternative `vedantmgoyal9/winget-releaser@v2` action. P1.M5.T2.S1 picks one; this task documents both
  so the choice is informed.

## What

### Naming Truth (GROUND TRUTH — read before writing a single line)

- `git remote get-url origin` → `git@github.com:dabstractor/qmkonnect.git` ⇒ **GitHub org = `dabstractor`**.
  (The local Linux user is `dustin` in `/home/dustin/...`; UNRELATED to the org.)
- Source repo = **`dabstractor/qmkonnect`**. **PackageIdentifier = `dabstractor.QMKonnect`** (matches S1's
  manifests + every other F15 channel: scoop-qmkonnect, homebrew-qmkonnect, AUR).
- The Publisher part of the id (`dabstractor`) is an identity token; the display `Publisher` is
  "Mulletware" (Inno `MyAppPublisher`). They intentionally differ — do NOT change the id.
- Asset: `QMKonnect-<version>-windows-x64.exe` (release.yml renames `QMKonnect-Setup.exe` → this; S1's
  `installer.yaml` `InstallerUrl` confirms). Tags are `v<version>`; `PackageVersion`/asset name are bare.

### File 1 — `packaging/winget/submit.ps1` (reference implementation — author verbatim)

A cross-platform PowerShell script. **Model the flow/guards/header on `packaging/scoop/update-manifest.ps1`**
(download → `Get-FileHash` → leading-`v` guard → hash-format validation → best-effort validate → scope
clause + deploy/secret doc in `.NOTES`), but replace the "regex-patch local file" step with "invoke
`wingetcreate update`" (the Critical Architectural Difference). The full reference:

```powershell
<#
.SYNOPSIS
  Publish a new QMKonnect release to microsoft/winget-pkgs via wingetcreate.

.DESCRIPTION
  Steps:
    1. Resolve the installer URL for <Version>:
       https://github.com/dabstractor/qmkonnect/releases/download/v<Version>/QMKonnect-<Version>-windows-x64.exe
    2. (unless -Sha256 is given) download the .exe and compute its SHA256 (Get-FileHash -Algorithm SHA256).
    3. Invoke `wingetcreate update dabstractor.QMKonnect` with:
           --urls  "<URL>|<SHA256>"      (pipe-delimited; built as ONE array element + splatted, so the
                                          '|' never reaches PowerShell's pipeline parser — see NOTES)
           --version <Version>
       and EITHER:
           (review mode, the default)  --out <OutDir>      -> generates the manifest YAML locally for
                                                              inspection. NO PR. (wingetcreate validate
                                                              is run afterwards as a best-effort check.)
           (-Submit)                   --token <Token> --submit
                                                              -> forks microsoft/winget-pkgs under the
                                                              token's owner, pushes a branch, and opens
                                                              a PR to microsoft/winget-pkgs:main.

  If `wingetcreate` is NOT on PATH, the script prints the exact would-run command (redacted token) and
  stops — so it is testable/previewable on a host without wingetcreate installed (e.g. a Linux dev box).
  In -Submit mode, a missing wingetcreate is a hard error (you cannot open a PR without the tool).

  PREREQUISITE (first release only, manual, interactive): `wingetcreate update` ONLY works AFTER the
    package `dabstractor.QMKonnect` already exists in microsoft/winget-pkgs. The FIRST submission is a
    one-time `wingetcreate new <installerURL> --token <PAT> --submit` (a maintainer fills the metadata
    from packaging/winget/*.yaml — S1's manifest triplet). See packaging/winget/README.md
    "Publishing to microsoft/winget-pkgs". This script is the per-release UPDATER that runs AFTER that
    first PR is merged.

.PARAMETER Version
  Release version WITHOUT a leading 'v' (e.g. "0.2.8"). Tags are v-prefixed; the winget PackageVersion
  and the asset filename are bare. A leading 'v' is rejected.

.PARAMETER Sha256
  Optional pre-computed SHA256 (64 lowercase hex). When given, the download is skipped.

.PARAMETER Submit
  Open a PR to microsoft/winget-pkgs (requires -Token). Omit for review mode (generate the manifest
  locally; no PR). The default is review mode — opening a PR to an external repo is an explicit opt-in.

.PARAMETER Token
  Classic GitHub PAT with the `public_repo` scope, stored as the WINGET_GITHUB_TOKEN Actions secret in
  dabstractor/qmkonnect. REQUIRED when -Submit is given. If -Token is omitted in -Submit mode, the script
  falls back to $env:WINGET_GITHUB_TOKEN; if that is also empty, it errors. Never logged by this script.

.PARAMETER OutDir
  Review-mode output directory for the generated manifest (default: a `winget-out/` folder beside this
  script). Ignored in -Submit mode.

.EXAMPLE
  # Review mode (generate manifest locally for inspection — NO PR; the safe default):
  ./submit.ps1 -Version 0.2.8
  ./submit.ps1 -Version 0.2.8 -Sha256 86dcaa57254f4b7dd23595ef8e473432c3997df3b6c12621913d215b9ab1b216
  # Submit mode (open the PR to winget-pkgs):
  ./submit.ps1 -Version 0.2.8 -Submit -Token $env:WINGET_GITHUB_TOKEN
  ./submit.ps1 -Version 0.2.8 -Sha256 <64-hex> -Submit -Token $env:WINGET_GITHUB_TOKEN
  ./submit.ps1 -Help

.NOTES
  TOKEN: a classic GitHub PAT with the `public_repo` scope, stored as the WINGET_GITHUB_TOKEN Actions
    secret in dabstractor/qmkonnect (create at https://github.com/settings/tokens → classic → check
    public_repo). The default GITHUB_TOKEN is scoped to dabstractor/qmkonnect and CANNOT fork
    microsoft/winget-pkgs — a separate classic PAT is mandatory. wingetcreate auto-creates the fork
    <owner>/winget-pkgs under the token's account and PRs to microsoft/winget-pkgs:main. This script
    NEVER logs the token — it travels only inside the splatted argv to the wingetcreate process; the
    script's own log line shows `--token ***`.

  THE '|' PIPE GOTCHA: `wingetcreate --urls "<URL>|<SHA256>"` uses a literal pipe. In PowerShell a BARE
    '|' is the pipeline operator, so `wingetcreate --urls $url|$hash` would try to pipe the wingetcreate
    process into $hash (CommandNotFoundException). This script avoids the trap entirely by building the
    wingetcreate arguments as an ARRAY and splatting it (`& wingetcreate @wcArgs`): each array element is
    passed to the native process as a separate argv token, so the '|' inside the URL|HASH string is never
    re-parsed by PowerShell. Do NOT replace the splat with a command-string + Invoke-Expression.

  CI INTEGRATION (P1.M5.T2.S1): the release workflow's `winget` job runs on windows-latest, runs
    `winget install Microsoft.WingetCreate`, then invokes this script with -Submit -Token
    $env:WINGET_GITHUB_TOKEN after the GitHub Release publishes. ALTERNATIVELY, P1.M5.T2.S1 may use the
    vedantmgoyal9/winget-releaser@v2 action instead (runs on ubuntu-latest; uses Komac; auto-finds the
    .exe via installers-regex, hashes it, opens the PR). Both paths need the WINGET_GITHUB_TOKEN PAT and
    both require the one-time manual `wingetcreate new` first. See packaging/winget/README.md
    "Publishing to microsoft/winget-pkgs".

  PREREQUISITES: wingetcreate on PATH (Windows: `winget install Microsoft.WingetCreate`). The GitHub
    release for <Version> must already be published (step 2 downloads the .exe). PowerShell 5.1
    (Windows) or 7+ (pwsh, cross-platform — GitHub Actions ubuntu/windows). Get-FileHash,
    Invoke-WebRequest, Get-Command, Get-Help are all built-in.
#>
[CmdletBinding()]
param(
    [string]$Version,
    [string]$Sha256,
    [switch]$Submit,
    [string]$Token,
    [string]$OutDir,
    [switch]$Help
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($Help -or -not $Version) {
    Get-Help $PSCommandPath -Detailed
    if (-not $Version) { exit 1 }
    exit 0
}

# Reject a leading 'v' (release TAGS are v-prefixed; winget versions are not).
if ($Version -match '^v') {
    throw "Version must not have a leading 'v' (got '$Version'). Use '$($Version -replace '^v','')'."
}

# -Submit requires a PAT. Accept it from -Token OR the WINGET_GITHUB_TOKEN env var.
if ($Submit) {
    if (-not $Token) { $Token = $env:WINGET_GITHUB_TOKEN }
    if (-not $Token) {
        throw "-Submit requires a GitHub PAT (classic, 'public_repo' scope). Pass -Token <PAT> or set `$env:WINGET_GITHUB_TOKEN."
    }
}

$PackageIdentifier = 'dabstractor.QMKonnect'
$SrcRepo           = 'dabstractor/qmkonnect'
$AssetName         = "QMKonnect-$Version-windows-x64.exe"
$DownloadUrl       = "https://github.com/$SrcRepo/releases/download/v$Version/$AssetName"

# --- Step 1+2: obtain the SHA256 (download+hash, or use -Sha256) ---
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
Write-Host "    url     -> $DownloadUrl"

# --- Step 3: build the wingetcreate argv as an ARRAY (splat keeps the '|' inside URL|HASH as ONE arg) ---
$UrlHash = "$DownloadUrl|$Sha256"
$wcArgs = @('update', $PackageIdentifier, '--urls', $UrlHash, '--version', $Version)
if ($Submit) {
    $wcArgs += @('--token', $Token, '--submit')
} else {
    if (-not $OutDir) { $OutDir = Join-Path $PSScriptRoot 'winget-out' }
    if (-not (Test-Path -LiteralPath $OutDir)) { New-Item -ItemType Directory -Path $OutDir | Out-Null }
    $wcArgs += @('--out', $OutDir)
}

# A display-only copy of the argv with the token redacted — for the log line (NEVER echo the real token).
$displayArgs = @('update', $PackageIdentifier, '--urls', $UrlHash, '--version', $Version)
if ($Submit) {
    $displayArgs += @('--token', '***', '--submit')
} else {
    $displayArgs += @('--out', $OutDir)
}

# wingetcreate must be on PATH to actually run. If absent, print the would-run command and stop.
$wc = Get-Command wingetcreate -ErrorAction SilentlyContinue
if (-not $wc) {
    Write-Host "==> wingetcreate not found on PATH."
    Write-Host "    Install (Windows / CI windows-latest): winget install Microsoft.WingetCreate"
    Write-Host "    Would-run command:"
    Write-Host "    wingetcreate $($displayArgs -join ' ')"
    if ($Submit) { throw "wingetcreate is required to submit. Install it (`winget install Microsoft.WingetCreate`), then re-run." }
    Write-Host "==> (review preview only — nothing submitted, no manifest written)."
    return
}

# --- Invoke wingetcreate (splat the array so the '|' in $UrlHash is one process arg) ---
Write-Host "==> wingetcreate $($displayArgs -join ' ')"   # redacted; the real token is in $wcArgs only
& wingetcreate @wcArgs
if ($LASTEXITCODE -ne 0) { throw "wingetcreate exited with code $LASTEXITCODE." }

# --- Step 4 (review mode only): best-effort `wingetcreate validate <OutDir>` ---
if (-not $Submit -and $OutDir) {
    Write-Host "==> wingetcreate validate $OutDir"
    & wingetcreate validate $OutDir
    # validate is advisory in review mode — do not hard-fail the whole run on a warning.
}

if ($Submit) {
    Write-Host "==> Done. wingetcreate forked winget-pkgs and opened a PR for $PackageIdentifier v$Version."
    Write-Host "    (The winget-pkgs maintainers review + merge it. `winget upgrade dabstractor.QMKonnect` serves it once merged.)"
} else {
    Write-Host "==> Done. Manifest written to $OutDir (review mode — NOT submitted)."
    Write-Host "    Next: inspect $OutDir, then re-run with -Submit -Token <PAT> to open the PR (or let CI P1.M5.T2.S1 do it)."
}
```

**Why these specific choices** (verified this session — see `research/notes.md`):
- **Array-splat, NOT a command string:** the literal `|` in `<URL>|<SHA256>` is the PowerShell pipeline
  operator if bare. `& wingetcreate @wcArgs` passes each array element as a separate native argv token,
  so the `|` inside the string is never re-parsed. This is THE load-bearing correctness fix (see NOTES).
- **Long flag forms (`--urls`/`--version`/`--token`/`--submit`/`--out`):** confirmed High-confidence from
  the winget-create README + MS Learn + discussion #190 (`wingetcreate update -u {url} -v {version}`,
  `--submit --token <TOKEN>`). Long forms sidestep short-form ambiguity across builds.
- **Token redaction in the log line:** `$displayArgs` mirrors `$wcArgs` with `--token ***`. The real
  `$Token` lives ONLY in `$wcArgs` (the splatted argv). PowerShell's `Write-Host` never sees it. This is
  a security-critical detail — never `Write-Host` the splatted argv directly.
- **Review mode is the default (PR is opt-in via `-Submit`):** opening a PR to an external repo is an
  explicit action; the safe default generates the manifest locally for inspection (`--out`), then a real
  run or CI passes `-Submit`. Mirrors how `update-cask.sh`/`update-manifest.ps1` are safe-by-default.
- **Degrade-to-preview when `wingetcreate` is absent:** lets the script be exercised/previewed on a host
  without the tool (e.g. this Linux dev box) — the would-run command is printed, nothing is submitted or
  written. In `-Submit` mode, absence is a hard error (you cannot open a PR without the tool).
- **`-UseBasicParsing`:** PS 5.1 compatibility (harmless on pwsh 7) — same as `update-manifest.ps1`.
- **`$LASTEXITCODE` check after the native call:** wingetcreate returns non-zero on failure (e.g. invalid
  manifest, network error, PR conflict); surface it instead of printing "Done" on a silent failure.
- **No regex-patch of the local S1 YAML:** `wingetcreate update` fetches the winget-pkgs manifest and
  updates it; the local YAML is untouched (see Critical Architectural Difference).

### File 2 — APPEND a section to `packaging/winget/README.md`

**Relationship to S1's README:** S1 (Implementing) owns the README body (install/upgrade/uninstall,
"unverified publisher", "What it installs", the high-level "For maintainers" pointer). THIS task
**APPENDS** a new `## Publishing to microsoft/winget-pkgs (for maintainers)` section at the END of that
file — the detailed submission-workflow + PAT deep-dive that S1's body only sketches. If S1's `README.md`
is not yet present at implementation time, create the file with just this section (it will merge with
S1's body when both land). Author this section **verbatim**:

````markdown
## Publishing to microsoft/winget-pkgs (for maintainers)

Winget packages do **not** ship from this repo — they live in the community
[`microsoft/winget-pkgs`](https://github.com/microsoft/winget-pkgs) repository, under
`manifests/d/dabstractor/QMKonnect/<version>/`. Each release is published by opening a **pull request**
to that repo; the winget-pkgs maintainers review and merge it. This section documents the two supported
publishing paths and the required GitHub PAT. (The end-user install side — `winget install
dabstractor.QMKonnect`, the "unverified publisher" warning — is covered above.)

### The GitHub PAT (required for both paths)

A **classic** personal access token with the **`public_repo`** scope, stored as the
**`WINGET_GITHUB_TOKEN`** Actions secret in
[`dabstractor/qmkonnect`](https://github.com/dabstractor/qmkonnect). Create it at
<https://github.com/settings/tokens> (Tokens (classic)) → check **`public_repo`**.

Why a separate PAT (not the default `${{ secrets.GITHUB_TOKEN }}`): both `wingetcreate` and the
`winget-releaser` action **auto-fork** `microsoft/winget-pkgs` into a namespace owned by the token
(`<owner>/winget-pkgs`), push a branch to that fork, and open a PR `fork → microsoft:main`. The default
`GITHUB_TOKEN` is scoped to `dabstractor/qmkonnect` only and **cannot** fork or push to an external
repo. **Never log the token** — `submit.ps1` passes it via argv and redacts it (`--token ***`) in its
own log line.

### First release (one-time, manual, interactive)

`wingetcreate update` (and the `winget-releaser` action) only work **after** the package already exists
in winget-pkgs. The very first version must be submitted by hand, once:

```powershell
# On a Windows host (wingetcreate is Windows-first):
winget install Microsoft.WingetCreate   # if not already installed
wingetcreate new `
    https://github.com/dabstractor/qmkonnect/releases/download/v0.2.8/QMKonnect-0.2.8-windows-x64.exe `
    --token $env:WINGET_GITHUB_TOKEN --submit
```

`wingetcreate new` downloads the installer, computes its SHA256, and interactively prompts for the
package metadata. Fill it from the manifest triplet in `packaging/winget/`
(`dabstractor.QMKonnect.{yaml,locale.en-US.yaml,installer.yaml}`) — same `PackageIdentifier`
(`dabstractor.QMKonnect`), `Publisher` (`Mulletware`), `PackageName` (`QMKonnect`), license (`MIT`),
tags (`qmk, keyboard, hid, tray`), `InstallerType: inno`, `Scope: user`, and the
`/VERYSILENT` / `/SILENT` silent switches. It then opens the initial PR; once the winget-pkgs
maintainers merge it, `dabstractor.QMKonnect` is live and **all subsequent releases are automated**.

### Each subsequent release — two options

**Option A — `submit.ps1` (wingetcreate CLI; CI runs on `windows-latest`):**

[`packaging/winget/submit.ps1`](submit.ps1) downloads the release `.exe`, computes its SHA256, and
invokes `wingetcreate update dabstractor.QMKonnect --urls "<url>|<sha256>" --version <ver> --token <PAT>
--submit`, which opens the per-release PR.

```powershell
# Review mode (generate the manifest locally for inspection — NO PR; the safe default):
./packaging/winget/submit.ps1 -Version 0.2.8
# Submit mode (open the PR to winget-pkgs):
./packaging/winget/submit.ps1 -Version 0.2.8 -Submit -Token $env:WINGET_GITHUB_TOKEN
# Skip the download if you already have the hash:
./packaging/winget/submit.ps1 -Version 0.2.8 -Sha256 <64-hex> -Submit -Token $env:WINGET_GITHUB_TOKEN
./packaging/winget/submit.ps1 -Help
```

The CI release workflow wires this in **P1.M5.T2.S1**: a `winget` job on `windows-latest` runs
`winget install Microsoft.WingetCreate` then `submit.ps1 -Submit -Token $env:WINGET_GITHUB_TOKEN` after
the GitHub Release publishes.

**Option B — `vedantmgoyal9/winget-releaser@v2` action (Komac under the hood; CI runs on `ubuntu-latest`):**

The alternative (what P1.M5.T2.S1 may use instead of Option A) is the
[`vedantmgoyal9/winget-releaser@v2`](https://github.com/vedantmgoyal9/winget-releaser) action, which
auto-finds the installer in the GitHub Release via a regex, computes its hash, and opens the PR. Minimal
CI snippet (P1.M5.T2.S1 owns the real workflow file):

```yaml
winget:
  runs-on: ubuntu-latest
  if: github.event_name == 'push'   # tag pushes only (after the GitHub Release publishes)
  steps:
    - uses: vedantmgoyal9/winget-releaser@v2
      with:
        identifier: dabstractor.QMKonnect
        # NOTE: 'version' is INTENTIONALLY OMITTED. The action defaults to the release tag with the
        # leading 'v' stripped (v0.2.8 -> 0.2.8). Do NOT pass `version: ${{ github.event.release.tag_name }}`
        # — that yields 'v0.2.8' VERBATIM (the action's else-branch does not strip it), which winget rejects.
        installers-regex: 'QMKonnect-.*-windows-x64\.exe$'
        token: ${{ secrets.WINGET_GITHUB_TOKEN }}   # classic PAT, public_repo scope
        release-repository: dabstractor/qmkonnect
```

> ⚠️ **Both options fail until the first manual `wingetcreate new` (above) is merged.** They update an
> existing winget-pkgs entry; they cannot create the first one. The `winget-releaser` action pre-flights
> the package's presence and errors with *"Package dabstractor.QMKonnect does not exist in the
> winget-pkgs repository. Please add at least one version of the package before using this action."*

### Versioning truth
`submit.ps1` (Option A) takes a **bare** version (`0.2.8`, no leading `v`) and **rejects** a leading
`v`. Release tags are `v0.2.8`; the winget `PackageVersion` and the asset filename are bare. The
`winget-releaser` action (Option B) strips the leading `v` automatically ONLY when `version` is omitted
(see the NOTE in the snippet above). The version always comes from `Cargo.toml` (the single source of
truth) via `cargo metadata` in CI — see `plan/007_fb356ba503b4/architecture/external_deps.md`
§"Version Source of Truth".
````

### Success Criteria
- [ ] `packaging/winget/submit.ps1` exists, parses under `pwsh` (`[ScriptBlock]::Create(...) | Out-Null`
      → exit 0), and `-Help` prints the comment-based help.
- [ ] The mock-wingetcreate test (Validation Level 2) PROVES the argv for BOTH modes (review + submit):
      the `update dabstractor.QMKonnect --urls "<exact-url>|<hash>" --version 9.9.9 [--out <dir> |
      --token <PAT> --submit]` tokens are captured exactly, the `|` is ONE argv token, and the script's
      stdout does NOT contain the PAT.
- [ ] `packaging/winget/README.md` contains the appended section with the exact `wingetcreate new`,
      `submit.ps1 -Submit`, and `winget-releaser@v2` snippets, the `WINGET_GITHUB_TOKEN`/`public_repo`
      PAT block, and the leading-`v` gotcha.
- [ ] `git diff --stat` shows ONLY the new `packaging/winget/submit.ps1` + the modified
      `packaging/winget/README.md`; the 3 S1 manifest YAMLs are untouched.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior wingetcreate knowledge can create `submit.ps1` verbatim from "What →
File 1" (the full reference script is given) and append the README section verbatim from "What → File 2",
then validate on Linux via the `pwsh` parse + the mock-wingetcreate argv test (Validation Level 2) +
grep invariants (Level 1) + README content (Level 3). The Windows-only live `wingetcreate` run + real
PR submission is explicitly deferred (and honestly noted).

### Documentation & References

```yaml
# MUST READ — the authoritative wingetcreate CLI (the tool submit.ps1 drives)
- url: https://github.com/microsoft/winget-create/blob/main/doc/update.md
  why: the `update` subcommand — confirms `--urls` (URL|SHA256, pipe-delimited), `--version`, `--token`,
       `--submit` (auto-PR to winget-pkgs), `--out` (local generation without submitting). The exact
       flag NAMES this script invokes.
  critical: "the URL|SHA256 pipe is literal; in PowerShell a bare '|' is the pipeline operator → the
       script builds argv as an array + splats it. --submit forks winget-pkgs under the token owner
       and PRs to microsoft:main. update ONLY works for packages already in winget-pkgs (first = `new`)."
- url: https://learn.microsoft.com/en-us/windows/package-manager/winget/create
  why: MS Learn mirror of the wingetcreate usage (new/update/submit) + the URL|HASH format notation.
- url: https://github.com/microsoft/winget-create/discussions/190
  why: confirms the short forms too (`wingetcreate update -u {url} -v {version}` + `--submit --token`).
       The script uses LONG forms to sidestep ambiguity, but this confirms the semantics.

# MUST READ — the authoritative winget-releaser action.yml (Option B; read in full this session)
- url: https://github.com/vedantmgoyal9/winget-releaser/blob/main/action.yml
  why: the EXACT `with:` inputs (identifier/version/installers-regex/token/release-repository/release-tag/
       fork-user/max-versions-to-keep) + the pre-flight "package does not exist" check + that it uses
       Komac (cross-platform → ubuntu-latest). Confirms the version-stripping gotcha: `version` is stripped
       ONLY when omitted; provided verbatim otherwise.
  critical: "OMIT `version` to get bare 0.2.8 from a v0.2.8 tag (the default branch does `-replace '^v'`);
       passing `version: ${{ github.event.release.tag_name }}` yields 'v0.2.8' verbatim → winget rejects."

# MUST READ — the COMPLETED in-source precedents (this task adapts their script flow)
- file: packaging/scoop/update-manifest.ps1
  why: the CLOSEST script-flow twin (PowerShell: param block + comment-based help; leading-v guard;
       download via Invoke-WebRequest -UseBasicParsing; Get-FileHash -Algorithm SHA256; hash-format
       validation; best-effort validator; the scope/secret doc in .NOTES). Copy the voice + guards;
       REPLACE the "regex-patch local file" step with "invoke wingetcreate update".
  pattern: "[CmdletBinding()] param; Set-StrictMode -Version Latest; $ErrorActionPreference='Stop';
       -Help prints Get-Help; leading-v throw; -Sha256 skips download; 64-hex validation; .NOTES documents
       the secret + the scope."
- file: packaging/homebrew/update-cask.sh
  why: the bash twin — same flow (download → hash → patch → confirm → best-effort audit), for tone +
       the "PURE local file update — does NOT push" scope clause (Winget INVERTS this: submit.ps1 DOES
       open the PR in -Submit mode — see Critical Architectural Difference).

# MUST READ — the INPUT facts submit.ps1 encodes (verified this session)
- file: packaging/winget/dabstractor.QMKonnect.installer.yaml   # S1 (landed)
  why: confirms PackageIdentifier `dabstractor.QMKonnect`, InstallerType inno, the exact asset URL
       `.../v0.2.8/QMKonnect-0.2.8-windows-x64.exe`, the 64-zero InstallerSha256 PLACEHOLDER that
       wingetcreate overwrites on the winget-pkgs copy (NOT the local copy submit.ps1 touches).
  gotcha: "submit.ps1 does NOT patch this file — wingetcreate update operates on winget-pkgs directly."
- file: packaging/winget/dabstractor.QMKonnect.yaml              # S1 (landed) — version manifest
  why: confirms PackageVersion 0.2.8 (bare), DefaultLocale en-US, ManifestVersion 1.6.0.
- file: packaging/winget/dabstractor.QMKonnect.locale.en-US.yaml # S1 (landed) — metadata source for `new`
  why: the metadata a maintainer fills into the one-time manual `wingetcreate new` (Publisher Mulletware,
       PackageName QMKonnect, License MIT, Tags, Moniker, silent switches).
- file: .github/workflows/release.yml
  why: the `windows` job: version via `cargo metadata | ConvertFrom-Json` (NO leading v); Inno
       `QMKonnect-Setup.exe` renamed to `QMKonnect-<version>-windows-x64.exe` (the asset submit.ps1
       downloads); the `publish` job creates the GitHub Release (the trigger P1.M5.T2.S1 keys on).
       NO `.sha256` sidecar (grep → none) ⇒ submit.ps1 MUST compute the hash itself.
  gotcha: "version has NO leading v; the URL path adds the v (.../v0.2.8/...); the asset filename uses
       the bare version. Do NOT prepend v to $Version."

# MUST READ — the architecture decision + CI strategy this implements
- docfile: plan/007_fb356ba503b4/architecture/external_deps.md
  why: §4 Winget — publication (PR to microsoft/winget-pkgs via wingetcreate), required fields, CI
       (automated wingetcreate PR or the winget-pkgs-automation/winget-releaser action). §"CI Publishing
       Strategy" (for Winget: "Automated PR to microsoft/winget-pkgs using wingetcreate or the official
       bot"). §"Version Source of Truth" + §"Hashing" (Winget: InstallerSha256 in manifest).
  section: "4. Winget (Windows)" + "CI Publishing Strategy" + "Version Source of Truth" + "Hashing"

# MUST READ — PRD context (the feature + platform row this is the channel for)
- url: spec/PRD.md
  why: §4 F15 (community package-manager distribution); §5 platform row "Windows: Inno .exe · Scoop ·
       Winget"; §12 signing note ("Winget prompts 'unverified publisher'").
  section: "h2.3 (4. Top-Level Feature Set, F15)" + "h2.4 (5. Supported Platforms)" + "§12"

# MUST READ — PAT scope + the winget-pkgs PR/fork model
- url: https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens
  why: confirms the classic-PAT `public_repo` scope covers forking a public repo + opening a PR.
  critical: "the default GITHUB_TOKEN CANNOT fork microsoft/winget-pkgs → a separate classic PAT is
       mandatory for BOTH Option A (submit.ps1) and Option B (winget-releaser)."

# REFERENCE — sibling/contract references
- docfile: plan/007_fb356ba503b4/P1M3T2S1/PRP.md   # S1 contract (the manifest triplet + README this rides on)
  why: confirms the 3 manifest YAMLs' structure (submit.ps1 does NOT patch them but documents them for
       `new`), the PackageIdentifier `dabstractor.QMKonnect`, the 64-zero InstallerSha256 placeholder,
       and that S1's README is the file THIS task appends a section to.
- docfile: plan/007_fb356ba503b4/P1M3T1S2/PRP.md   # the COMPLETED Scoop S2 — structural twin
  why: the Scoop update-manifest.ps1 + bucket-README contract. THIS task is its Winget mirror, with the
       publication model inverted (external PR via wingetcreate, not a deploy-key push to our own repo).
- file: plan/007_fb356ba503b4/tasks.json   # (read-only — the orchestrator owns it)
  why: confirms this item's contract + the downstream P1.M5.T2.S1 (the CI `winget` job that consumes
       submit.ps1 OR the winget-releaser action) + the WINGET_GITHUB_TOKEN secret.

# REFERENCE — this task's own research notes (full findings + source URLs)
- docfile: plan/007_fb356ba503b4/P1M3T2S2/research/notes.md
  why: the wingetcreate flag corroboration; the winget-releaser action.yml findings (incl. the
       version-stripping gotcha); the PAT requirement; the mock-wingetcreate validation strategy.
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
packaging/
  winget/
    dabstractor.QMKonnect.yaml                 # S1 (LANDED) — version manifest (PackageIdentifier/Version source of truth)
    dabstractor.QMKonnect.locale.en-US.yaml    # S1 (LANDED) — metadata for the manual `wingetcreate new`
    dabstractor.QMKonnect.installer.yaml       # S1 (LANDED) — installer manifest (asset URL + 64-zero hash placeholder)
    README.md                                  # S1 (NOT yet landed — THIS task APPENDS a section to it)
  scoop/
    update-manifest.ps1                        # <<< script-flow twin (download → Get-FileHash → invoke tool) >>>
  homebrew/
    update-cask.sh                             # <<< script-flow twin (bash; tone + scope clause) >>>
  windows/inno/QMKonnect.iss                   # installer metadata (Publisher/Name/scope)
.github/workflows/release.yml                  # asset naming + version-from-cargo + NO sha256 sidecar
Cargo.toml                                     # version=0.2.8 (single source of truth)
plan/007_fb356ba503b4/
  architecture/external_deps.md                # §4 Winget + CI Publishing Strategy
  P1M3T2S1/PRP.md                              # S1 contract (the manifests + README this rides on)
  P1M3T1S2/PRP.md                              # Scoop S2 contract (the structural twin)
  P1M3T2S2/research/notes.md                   # this task's research findings
# NEW (this task): packaging/winget/submit.ps1 ; MODIFIED: packaging/winget/README.md (append one section)
```

### Desired Codebase tree (files this task ADDS / MODIFIES)

```bash
packaging/winget/
├── dabstractor.QMKonnect.yaml                 # (S1, unchanged)
├── dabstractor.QMKonnect.locale.en-US.yaml    # (S1, unchanged)
├── dabstractor.QMKonnect.installer.yaml       # (S1, unchanged)
├── submit.ps1                                 # NEW  — wingetcreate-driven publish automation (review + submit)
└── README.md                                  # MODIFIED — APPEND "## Publishing to microsoft/winget-pkgs (for maintainers)"
```
(No other files. The CI `winget` job = P1.M5.T2.S1; docs/installation.md Winget row = P1.M6.T1.S1.)

### Known Gotchas of our codebase & Library Quirks

```yaml
# CRITICAL (publication model INVERTED vs Scoop/Homebrew): the target is microsoft/winget-pkgs (EXTERNAL;
#   we do NOT own it). You cannot git-push; publication is a PR opened by `wingetcreate update --submit`.
#   submit.ps1 does NOT regex-patch the local S1 YAML — wingetcreate update fetches the winget-pkgs
#   manifest and updates it. The local YAML is ONLY the template for the one-time manual `wingetcreate new`.

# CRITICAL (the '|' is the PowerShell pipeline operator): `wingetcreate --urls "<URL>|<HASH>"` uses a
#   literal pipe. Build the wingetcreate argv as an ARRAY and splat it (`& wingetcreate @wcArgs`); each
#   array element is one native argv token, so the '|' inside URL|HASH is never re-parsed. Do NOT build a
#   command string + Invoke-Expression (re-parse breaks on the pipe).

# CRITICAL (never log the PAT): mirror the splatted argv into a $displayArgs copy with `--token ***` for
#   the Write-Host log line. The real $Token lives ONLY in $wcArgs. Never `Write-Host "...$Token..."` or
#   echo the splatted argv directly.

# CRITICAL (first submission is manual): `wingetcreate update` (and winget-releaser) ONLY work AFTER
#   dabstractor.QMKonnect already exists in winget-pkgs. The FIRST version is a one-time, interactive,
#   manual `wingetcreate new <url> --token <PAT> --submit` (metadata filled from S1's manifests). submit.ps1
#   is the per-release UPDATER that runs after that first PR is merged. Document this in the README section.

# CRITICAL (PAT scope): a CLASSIC PAT with `public_repo` scope → WINGET_GITHUB_TOKEN Actions secret. The
#   default GITHUB_TOKEN is scoped to dabstractor/qmkonnect and CANNOT fork microsoft/winget-pkgs.

# CRITICAL (winget-releaser version gotcha — Option B): OMIT the `version` input so the action strips the
#   tag's leading 'v' (v0.2.8 -> 0.2.8). Passing `version: ${{ github.event.release.tag_name }}` yields
#   'v0.2.8' VERBATIM (the action's else-branch does NOT strip) → winget rejects. (Verified in action.yml.)

# CRITICAL (org != local user): GitHub org = `dabstractor` (git remote get-url origin). The local Linux
#   user `dustin` (/home/dustin/...) is UNRELATED. PackageIdentifier = dabstractor.QMKonnect. Do NOT write
#   Dustin./dustin./Mulletware. as the id (the display Publisher "Mulletware" intentionally differs).

# CRITICAL (NO .sha256 sidecar): the release publishes only the renamed .exe. submit.ps1 downloads it and
#   computes SHA256 itself (Get-FileHash). Do NOT invent a sidecar URL.

# CRITICAL (version v-prefix): $Version is bare "0.2.8" (NO leading v); the tag is "v0.2.8"; the URL path
#   adds the v (.../v0.2.8/...); the asset filename uses the bare version. submit.ps1 REJECTS a leading v.

# GOTCHA (long flags only): use --urls/--version/--token/--submit/--out (long forms, High-confidence).
#   Avoid the short forms (-u/-v/-t) — they vary across wingetcreate builds; the long forms are stable.

# GOTCHA (pwsh IS on this Linux box — 7.6.2): VALIDATE the script via pwsh parse + the mock-wingetcreate
#   argv test (Validation Level 2). wingetcreate itself is NOT on the box (Windows-first) → the real run
#   + PR submission is DEFERRED to a windows-latest CI host (P1.M5.T2.S1).

# GOTCHA (scope): this task does NOT write .github/workflows/* (CI = P1.M5.T2.S1) or docs/installation.md
#   (P1.M6.T1.S1). It creates submit.ps1 + appends ONE section to packaging/winget/README.md.
```

## Implementation Blueprint

### Data models and structure
No code models. One static PowerShell script + one Markdown section. The script reads the release
`.exe` URL/version, computes a SHA256, and builds a wingetcreate argv array; it introduces no types.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CREATE packaging/winget/submit.ps1
  - IMPLEMENT: the reference script from "What → File 1" (CmdletBinding param block -Version/-Sha256/
    -Submit/-Token/-OutDir/-Help; comment-based .SYNOPSIS/.PARAMETER/.EXAMPLE/.NOTES help; leading-v
    guard; -Submit → Token-or-$env:WINGET_GITHUB_TOKEN requirement; download via Invoke-WebRequest
    -UseBasicParsing; Get-FileHash -Algorithm SHA256; 64-hex validation; argv ARRAY $wcArgs + $displayArgs
    redacted twin; Get-Command wingetcreate preview-degrade; `& wingetcreate @wcArgs` splat; $LASTEXITCODE
    check; review-mode `wingetcreate validate`; the scope/PAT/pipe-gotcha/CI doc in .NOTES).
  - FOLLOW pattern: packaging/scoop/update-manifest.ps1 (param block + help + guards + Get-FileHash +
    -UseBasicParsing + best-effort validator + .NOTES secret/scope doc). REPLACE the regex-patch step
    with the wingetcreate invocation (the Critical Architectural Difference).
  - NAMING: params -Version / -Sha256 / -Submit / -Token / -OutDir / -Help; script file submit.ps1.
  - DEPENDENCIES: PackageIdentifier dabstractor.QMKonnect + asset URL pattern (from S1's installer.yaml);
    wingetcreate on PATH at RUN time (not author time).
  - PLACEMENT: packaging/winget/submit.ps1.

Task 2: VALIDATE the script (Linux-safe; pwsh 7.6.2 IS installed here)
  - RUN (parse): pwsh -NoProfile -Command "[ScriptBlock]::Create((Get-Content -Raw packaging/winget/submit.ps1)) | Out-Null; 'parses OK'"
      → expect "parses OK" (exit 0).
  - RUN (help): pwsh -NoProfile -File packaging/winget/submit.ps1 -Help → expect the .SYNOPSIS/.PARAMETER block.
  - RUN (mock-wingetcreate argv test — PROVES command construction for BOTH modes): see Validation Level 2.
  - RUN (grep invariants): see Validation Level 1.
  - NOTE: a real download+wingetcreate+PR run is Windows-only → DEFER to a windows-latest host (P1.M5.T2.S1).

Task 3: APPEND the README section (or create the file with it if S1's README is absent)
  - IMPLEMENT: append "## Publishing to microsoft/winget-pkgs (for maintainers)" (the verbatim block from
    "What → File 2") to the END of packaging/winget/README.md. If that file does not yet exist (S1 in
    flight), create it containing just this section (it merges with S1's body when both land).
  - MUST INCLUDE verbatim: the PAT block (classic PAT, public_repo, WINGET_GITHUB_TOKEN, why not
    GITHUB_TOKEN, never-log); the one-time manual `wingetcreate new <url> --token <PAT> --submit` block;
    Option A (submit.ps1 -Version/-Submit usage + the P1.M5.T2.S1 windows-latest pointer); Option B
    (vedantmgoyal9/winget-releaser@v2 snippet with `version` OMITTED + the leading-v NOTE + ubuntu-latest);
    the "both fail until the first new is merged" warning; the versioning-truth paragraph.
  - DO NOT duplicate or rewrite S1's existing README body (install/upgrade/uninstall, "unverified
    publisher", "What it installs", S1's high-level "For maintainers" pointer) — APPEND only.
  - PLACEMENT: packaging/winget/README.md (S1's file; this task appends).

Task 4: NEVER do these (out of scope / forbidden)
  - DO NOT regex-patch / modify the 3 S1 manifest YAMLs (wingetcreate update operates on winget-pkgs, not
      the local files; the local YAML is the template for the manual `new` only).
  - DO NOT build the wingetcreate command as a STRING + Invoke-Expression (the '|' pipe breaks it). Use
      the array-splat (`& wingetcreate @wcArgs`).
  - DO NOT log the PAT — redact to `--token ***` in the display/log argv; keep the real token only in the
      splatted argv.
  - DO NOT edit .github/workflows/* (the CI `winget` job = P1.M5.T2.S1) or docs/installation.md (P1.M6.T1.S1).
  - DO NOT create a separate bucket/tap-style README (unlike Scoop/Homebrew S2) — Winget's target is
      external; append ONE section to packaging/winget/README.md instead.
  - DO NOT use short wingetcreate flags (-u/-v/-t) — use the long forms (--urls/--version/--token/--submit/--out).
  - DO NOT prepend `v` to $Version or to the asset name (version is bare; URL path adds the v).
  - DO NOT invent a .sha256 sidecar URL (none exists; submit.ps1 computes the hash from the downloaded .exe).
  - DO NOT change any Rust source / Cargo.toml / other packaging dir / docs outside packaging/winget/.
  - DO NOT edit PRD.md, any tasks.json, or prd_snapshot.md.
```

### Implementation Patterns & Key Details
```powershell
# PATTERN (the array-splat — THE load-bearing correctness fix for the '|' pipe):
$UrlHash = "$DownloadUrl|$Sha256"                          # ONE string with a literal '|'
$wcArgs  = @('update', $PackageIdentifier, '--urls', $UrlHash, '--version', $Version)
if ($Submit) { $wcArgs += @('--token', $Token, '--submit') }
else        { $wcArgs += @('--out', $OutDir) }
& wingetcreate @wcArgs                                     # each element → one native argv token; '|' survives

# PATTERN (token redaction — NEVER log the PAT):
$displayArgs = @('update', $PackageIdentifier, '--urls', $UrlHash, '--version', $Version)
if ($Submit) { $displayArgs += @('--token', '***', '--submit') }   # '***', NOT $Token
else        { $displayArgs += @('--out', $OutDir) }
Write-Host "==> wingetcreate $($displayArgs -join ' ')"            # safe to log

# PATTERN (degrade-to-preview when wingetcreate is absent → testable on Linux without the tool):
$wc = Get-Command wingetcreate -ErrorAction SilentlyContinue
if (-not $wc) {
    Write-Host "    Would-run command:`n    wingetcreate $($displayArgs -join ' ')"
    if ($Submit) { throw "wingetcreate is required to submit ..." }
    return        # review-mode preview only
}

# PATTERN (the publication model — inverted from Scoop/Homebrew S2):
#   Scoop/Homebrew S2 = regex-patch LOCAL manifest → CI deploy-key git-push to OUR repo.
#   Winget S2         = `wingetcreate update` mutates the WINGET-PKGS manifest + opens the PR atomically.
#   So submit.ps1 OPENS THE PR in -Submit mode (it is NOT a pure local patcher). The local S1 YAML is
#   the template for the one-time manual `wingetcreate new`, NOT an input submit.ps1 patches.
```
```text
# PATTERN: the S1/S2/CI three-way split (Winget variant).
#   S1 (manifest triplet + README body)   = packaging/winget/{3 YAMLs, README.md}
#   S2 (THIS task: publish script + doc)  = packaging/winget/{submit.ps1, README.md += "Publishing" section}
#   CI (P1.M5.T2.S1: the winget job)      = .github/workflows/release.yml winget job (Option A: submit.ps1
#                                            on windows-latest; OR Option B: winget-releaser@v2 on ubuntu-latest)

# ANTI-PATTERN: don't regex-patch the local S1 YAML (wingetcreate update doesn't read it). Don't string-
#   build the wingetcreate command (the '|' pipe). Don't echo the token. Don't pass `version:` to
#   winget-releaser (the leading-v strip only happens when it's omitted). Don't omit the "first release
#   is manual" doc — both options fail without it.
```

### Integration Points
```yaml
INPUT (S1):          packaging/winget/dabstractor.QMKonnect.installer.yaml (PackageIdentifier + asset URL pattern;
                     the 64-zero InstallerSha256 placeholder wingetcreate overwrites on the winget-pkgs copy)
INPUT (release):     QMKonnect-<version>-windows-x64.exe (release.yml windows job, renamed from QMKonnect-Setup.exe)
OUTPUT (this task):  packaging/winget/submit.ps1 + (append) packaging/winget/README.md "Publishing" section
PUBLISH TARGET:      microsoft/winget-pkgs  folder manifests/d/dabstractor/QMKonnect/<version>/
                     (EXTERNAL repo; wingetcreate update --submit opens the PR; first version via manual `new`)
CI (P1.M5.T2.S1):    on tag → either (Option A) windows-latest: winget install Microsoft.WingetCreate then
                     submit.ps1 -Submit -Token $env:WINGET_GITHUB_TOKEN; OR (Option B) ubuntu-latest:
                     vedantmgoyal9/winget-releaser@v2 with identifier/installers-regex/token (version OMITTED).
SECRET:              WINGET_GITHUB_TOKEN — classic PAT, public_repo scope (NOT the default GITHUB_TOKEN).
METADATA SOURCE:     Cargo.toml — single source of truth (external_deps.md §"Version Source of Truth");
                     version via `cargo metadata` (bare, no leading v).
DOCS SYNC (P1.M6):   docs/installation.md Windows section + top-level README link the channel (NOT this task).
PARALLEL (no conflict):
  - P1.M3.T2.S1 (Winget manifest, Implementing): owns the 3 YAMLs + README.md body. THIS task consumes the
    PackageIdentifier/asset-URL facts + APPENDS one section to README.md. submit.ps1 does NOT modify the YAMLs.
  - P1.M5.T2.S1 (CI winget job, Planned): consumes submit.ps1 (Option A) OR the winget-releaser action
    (Option B). THIS task documents both but does NOT write the workflow.
PLATFORM VALIDATION: Linux box (pwsh 7.6.2 installed) proves the script PARSES + command CONSTRUCTION via
  the mock-wingetcreate argv test. The live wingetcreate run + real PR submission is Windows/CI-only → deferred.
```

## Validation Loop

> Toolchain: the deliverables are a PowerShell script + a Markdown section. `pwsh 7.6.2` IS installed on
> this box → parse + help + a MOCK-wingetcreate argv test all run on Linux. `wingetcreate` itself is
> Windows-first (NOT on this box) → the real run/PR is deferred to P1.M5.T2.S1's windows-latest host.

### Level 1: Script text invariants (Linux — no wingetcreate needed)
```bash
cd /home/dustin/projects/qmkonnect
S=packaging/winget/submit.ps1
# Core invariants the reference script MUST contain:
grep -nE 'param\(' "$S"                                            # CmdletBinding param block
grep -nE '\[string\]\$Version|\[switch\]\$Submit|\[string\]\$Token|\[string\]\$Sha256|\[string\]\$OutDir|switch\]\$Help' "$S"
grep -nE "Version -match '\^v'|leading 'v'" "$S"                   # leading-v guard
grep -nE 'Get-FileHash -Algorithm SHA256' "$S"                     # hash computation
grep -nE "dabstractor\.QMKonnect" "$S"                             # PackageIdentifier
grep -nE 'QMKonnect-\$Version-windows-x64\.exe|releases/download/v' "$S"   # asset URL pattern
grep -nE '\$wcArgs\s*=\s*@\(.*update' "$S"                         # argv ARRAY (the splat fix)
grep -nE "urls.*\|.*Sha256|UrlHash.*\|" "$S"                       # the URL|HASH pipe string
grep -nE 'displayArgs.*\*\*\*|token.*\*\*\*' "$S"                  # token REDACTION in the log argv
grep -nE 'wingetcreate @wcArgs|& wingetcreate @wcArgs' "$S"        # the SPLAT invocation (not a string)
grep -nE 'Get-Command wingetcreate' "$S"                           # preview-degrade / presence check
grep -nE 'LASTEXITCODE' "$S"                                       # native-exit check
grep -nE 'public_repo|WINGET_GITHUB_TOKEN' "$S"                    # PAT doc in the header/.NOTES
grep -nE 'wingetcreate new|first release|first submission|PREREQUISITE' "$S"   # the manual-first doc
grep -nE 'vedantmgoyal9/winget-releaser' "$S"                      # the alternative is documented
grep -nE 'winget install Microsoft.WingetCreate' "$S"              # install instructions
# Anti-checks (must be ABSENT — the splat, not a command string; long flags, not short):
! grep -nE 'Invoke-Expression.*wingetcreate|iex.*wingetcreate' "$S" && echo "no IEX command-string (good)"
! grep -nE 'Write-Host.*\$Token' "$S" && echo "no raw-token echo (good)"
# Expected: every grep prints ≥1 hit; the two !grep print their OK lines.
```

### Level 2: pwsh parse + MOCK-wingetcreate argv test (Linux — PROVES command construction)
```bash
cd /home/dustin/projects/qmkonnect
S=packaging/winget/submit.ps1
# (a) Parse check — pwsh 7.6.2 is installed here:
pwsh -NoProfile -Command "[ScriptBlock]::Create((Get-Content -Raw '$S')) | Out-Null; 'parses OK'"
#    Expected: "parses OK" (exit 0). A parse error means a PowerShell syntax bug — read + fix.
# (b) Help check:
pwsh -NoProfile -File "$S" -Help 2>&1 | grep -qi 'SYNOPSIS' && echo "help prints OK"

# (c) MOCK test — a PATH shim captures the exact argv wingetcreate WOULD receive, WITHOUT the real tool.
#     This proves: correct flags, the URL|HASH '|' is ONE argv token, the token is passed (submit mode),
#     AND the script's OWN stdout redacts the token.
WORK=$(mktemp -d); BIN="$WORK/bin"; mkdir -p "$BIN"
CAP="$WORK/captured-args.txt"; : > "$CAP"
cat > "$BIN/wingetcreate" <<EOF
#!/bin/sh
printf '%s\n' "\$@" >> "$CAP"
EOF
chmod +x "$BIN/wingetcreate"

# REVIEW mode (no -Submit): expect --out, NO token, NO --submit.
TESTHASH=86dcaa57254f4b7dd23595ef8e473432c3997df3b6c12621913d215b9ab1b216
OUT=$(PATH="$BIN:$PATH" pwsh -NoProfile -File "$S" -Version 9.9.9 -Sha256 "$TESTHASH" 2>&1) || true
echo "$OUT"
# Assert the captured argv (one token per line):
grep -qx 'update' "$CAP"                                  && echo "ok: update"
grep -qx 'dabstractor.QMKonnect' "$CAP"                   && echo "ok: id"
grep -qx "https://github.com/dabstractor/qmkonnect/releases/download/v9.9.9/QMKonnect-9.9.9-windows-x64.exe|$TESTHASH" "$CAP" \
  && echo "ok: URL|HASH is ONE argv token (the splat fix works)"
grep -qx -- '--urls' "$CAP" && grep -qx -- '--version' "$CAP" && grep -qx '9.9.9' "$CAP"
grep -q -- '--out' "$CAP"                                  && echo "ok: review mode --out"
! grep -q -- '--submit' "$CAP" && echo "ok: review mode has no --submit"
# Reset capture; run SUBMIT mode.
: > "$CAP"
OUT2=$(PATH="$BIN:$PATH" pwsh -NoProfile -File "$S" -Version 9.9.9 -Sha256 "$TESTHASH" -Submit -Token testtok123 2>&1) || true
grep -q -- '--submit' "$CAP"                              && echo "ok: submit mode --submit"
grep -qx 'testtok123' "$CAP"                              && echo "ok: token passed to wingetcreate (argv)"
# SECURITY: the script's OWN stdout must NOT contain the token (display argv redacts to ***):
! (printf '%s' "$OUT2" | grep -q 'testtok123') && echo "ok: token NOT echoed in script stdout (redacted)"
printf '%s' "$OUT2" | grep -q -- '--token \*\*\*' && echo "ok: log line shows --token ***"
rm -rf "$WORK"
# Expected: every 'ok:' line prints. A missing 'ok:' = a real bug in command construction / redaction.
```

### Level 3: README section content + scope (Linux)
```bash
cd /home/dustin/projects/qmkonnect
R=packaging/winget/README.md
# The appended section has the exact submission-workflow + PAT content:
grep -nE 'Publishing to microsoft/winget-pkgs' "$R"
grep -nE 'public_repo|WINGET_GITHUB_TOKEN|classic.*PAT|classic.*personal access token' "$R"
grep -nE 'wingetcreate new' "$R"                                                 # one-time manual first submission
grep -nE 'submit\.ps1.*-Submit|submit\.ps1.*-Version' "$R"                       # Option A usage
grep -nE 'windows-latest' "$R"                                                   # Option A runner
grep -nE 'vedantmgoyal9/winget-releaser@v2' "$R"                                 # Option B action
grep -nE "installers-regex: 'QMKonnect-.*-windows-x64" "$R"                      # the asset regex
grep -nE 'ubuntu-latest' "$R"                                                    # Option B runner
grep -nE "version.*OMITTED|leading 'v'|VERBATIM" "$R"                            # the version-stripping gotcha
grep -nE 'does not exist in the winget-pkgs repository|first.*manual|one-time' "$R"  # the manual-first warning
grep -nE 'P1\.M5\.T2\.S1' "$R"                                                   # CI pointer
# Scope: ONLY the new submit.ps1 + the modified README.md:
git status --short                                              # Expected: packaging/winget/{submit.ps1,README.md}
git diff --stat -- Cargo.toml .github/workflows/release.yml src/ docs/installation.md packaging/scoop packaging/homebrew
                                                                # Expected: empty
# The 3 S1 manifest YAMLs are UNTOUCHED:
git diff --stat -- packaging/winget/dabstractor.QMKonnect.yaml packaging/winget/dabstractor.QMKonnect.locale.en-US.yaml packaging/winget/dabstractor.QMKonnect.installer.yaml
                                                                # Expected: empty
```

### Level 4: Live run (Windows/CI host — OPTIONAL, deferred to P1.M5.T2.S1)
```powershell
# On a windows-latest host (CI) or a Windows dev box, AFTER a release is published AND after the one-time
# manual `wingetcreate new` has been merged into winget-pkgs:
winget install Microsoft.WingetCreate
$env:WINGET_GITHUB_TOKEN = "<classic PAT, public_repo>"
cd <clone of dabstractor/qmkonnect>
# 1. Review mode (generate the manifest locally for inspection — NO PR):
.\packaging\winget\submit.ps1 -Version 0.2.8
#    Expected: downloads QMKonnect-0.2.8-windows-x64.exe, prints the SHA256, runs
#    `wingetcreate update dabstractor.QMKonnect --urls "<url>|<hash>" --version 0.2.8 --out winget-out`,
#    then `wingetcreate validate winget-out`.
# 2. Submit mode (open the PR to microsoft/winget-pkgs):
.\packaging\winget\submit.ps1 -Version 0.2.8 -Submit -Token $env:WINGET_GITHUB_TOKEN
#    Expected: forks microsoft/winget-pkgs (if needed), pushes a branch, opens a PR
#    fork → microsoft:main. The winget-pkgs maintainers review + merge; `winget upgrade
#    dabstractor.QMKonnect` then serves v0.2.8 to users.
# (DEFERRED — wingetcreate is Windows-first; the Linux dev box validates structure + command construction only.)
```

## Final Validation Checklist

### Technical Validation
- [ ] `submit.ps1` parses under `pwsh` ("parses OK"); `-Help` prints the help (Validation Level 2a/2b).
- [ ] The mock-wingetcreate test passes for BOTH modes (every "ok:" line) — proves flags, the `|` is one
      argv token, token passed to wingetcreate, token NOT echoed in stdout.
- [ ] `git diff --stat` shows ONLY `packaging/winget/submit.ps1` (new) + `packaging/winget/README.md`
      (modified); the 3 S1 YAMLs + everything outside `packaging/winget/` unchanged.

### Feature Validation
- [ ] `submit.ps1` exists with the `-Version`/`-Sha256`/`-Submit`/`-Token`/`-OutDir`/`-Help` params, the
      leading-`v` guard, `Get-FileHash -Algorithm SHA256`, the array-splat argv + token-redacted display,
      the `Get-Command wingetcreate` preview-degrade, the `$LASTEXITCODE` check.
- [ ] `submit.ps1` invokes `wingetcreate update dabstractor.QMKonnect --urls "<url>|<hash>" --version <ver>`
      with `--token <PAT> --submit` (submit mode) OR `--out <OutDir>` (review mode).
- [ ] README section documents: the classic PAT (`public_repo` → `WINGET_GITHUB_TOKEN`, why not GITHUB_TOKEN);
      the one-time manual `wingetcreate new`; Option A (`submit.ps1`, windows-latest); Option B
      (`winget-releaser@v2`, ubuntu-latest, `version` OMITTED); the manual-first warning; versioning truth.
- [ ] The leading-`v` gotcha for `winget-releaser` (version omitted → stripped; provided → verbatim) is stated.

### Code Quality Validation
- [ ] Script mirrors `packaging/scoop/update-manifest.ps1` (param block, help, guards, Get-FileHash,
      -UseBasicParsing, best-effort validator, .NOTES secret/scope doc) — with the regex-patch step
      replaced by the wingetcreate invocation (the inverted publication model).
- [ ] No `Invoke-Expression`/command-string for wingetcreate (array-splat only); no raw-token echo;
      long flags only.
- [ ] README section tone + cross-links mirror the sibling channel READMEs.

### Documentation & Deployment
- [ ] README section is appended (not a separate file); it does not duplicate S1's README body.
- [ ] The Windows/CI-only live `wingetcreate` run + PR submission is noted as deferred in the report.
- [ ] No new app env vars / config keys (pure packaging automation).

---

## Anti-Patterns to Avoid
- ❌ Don't regex-patch the local S1 manifest YAMLs — `wingetcreate update` operates on winget-pkgs, not the local files. The local YAML is the template for the one-time manual `wingetcreate new` only. (This is the inverse of Scoop/Homebrew S2, which patch the local manifest.)
- ❌ Don't build the wingetcreate command as a string + `Invoke-Expression` — the literal `|` in `URL|HASH` is the PowerShell pipeline operator. Use an array + splat (`& wingetcreate @wcArgs`).
- ❌ Don't echo the PAT — redact to `--token ***` in the display/log argv; keep the real token only in the splatted argv.
- ❌ Don't omit the "first release is a one-time manual `wingetcreate new`" documentation — both `submit.ps1 -Submit` and `winget-releaser` fail until `dabstractor.QMKonnect` exists in winget-pkgs.
- ❌ Don't pass `version: ${{ github.event.release.tag_name }}` to `winget-releaser` — that yields `v0.2.8` verbatim (the action's else-branch does NOT strip); OMIT `version` so it strips the leading `v`.
- ❌ Don't create a separate bucket/tap-style README (unlike Scoop/Homebrew S2) — Winget's target is external; append ONE section to `packaging/winget/README.md`.
- ❌ Don't use the default `GITHUB_TOKEN` for winget-pkgs PRs — it can't fork an external repo; use a classic PAT with `public_repo` → `WINGET_GITHUB_TOKEN`.
- ❌ Don't use short wingetcreate flags (`-u`/`-v`/`-t`) — use the long forms (`--urls`/`--version`/`--token`/`--submit`/`--out`).
- ❌ Don't prepend `v` to `$Version` or the asset name — version is bare `0.2.8`; the URL path adds the `v`.
- ❌ Don't invent a `.sha256` sidecar URL — none is published; `submit.ps1` computes the hash from the downloaded `.exe`.
- ❌ Don't edit `.github/workflows/*` (CI = P1.M5.T2.S1), `docs/installation.md` (P1.M6.T1.S1), or the 3 S1 YAMLs.
- ❌ Don't edit Cargo.toml / Rust source / other packaging dirs / PRD.md / tasks.json / prd_snapshot.md.

---

## Confidence Score: 9/10

`submit.ps1` is specced as a complete, copy-pasteable reference (download → `Get-FileHash` → array-splat
`wingetcreate update` invocation, with the literal-`|`-pipe trap fixed by the splat and the PAT redacted
in the log line), modeled on the COMPLETED `packaging/scoop/update-manifest.ps1` + `packaging/homebrew/update-cask.sh`
twins. Every wingetcreate flag is corroborated by multiple sources (microsoft/winget-create README + MS
Learn + discussion #190 + the techcommunity/ImageMagick automation examples), and the `winget-releaser`
facts are read directly from its `action.yml` (including the version-stripping gotcha). The command
construction is PROVEN on this Linux box by the mock-wingetcreate argv test (Validation Level 2 — pwsh 7.6.2
is installed here), and the script's preview-degrade makes it exercisable without the tool. The README
section is specced verbatim with the exact `wingetcreate new` / `submit.ps1 -Submit` / `winget-releaser@v2`
snippets + the PAT block + the manual-first warning. The 1-point reservation: the real end-to-end
`wingetcreate update --submit` (fork → branch → PR) and the winget-pkgs maintainers' merge happen only on a
Windows/CI host after the one-time manual `new` — those are honestly deferred to P1.M5.T2.S1; the flag
names + command construction + PAT/runner/versioning decisions here are locked and Linux-validated.
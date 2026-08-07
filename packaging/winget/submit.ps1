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
    installer via installers-regex, hashes it, opens the PR). Both paths need the WINGET_GITHUB_TOKEN PAT and
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
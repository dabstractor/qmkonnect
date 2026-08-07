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
if ($check.Count -lt 3) { throw "Post-patch confirmation failed (expected version+hash+`$version)." }
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
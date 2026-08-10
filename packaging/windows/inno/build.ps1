<#
.SYNOPSIS
    Builds the QMKonnect tray-app installer (Inno Setup) -> Output\QMKonnect-Setup.exe.

.DESCRIPTION
    Reads the version from ..\..\..\Cargo.toml (single source of truth), validates
    that a fresh release exe exists on a TRUSTED path, then runs ISCC.

    Prerequisites:
      * a release build: `cargo build --release`
      * Inno Setup 6: `winget install JRSoftware.InnoSetup`

    Path validation: QMKonnect.iss bakes the exe's source path into the installer
    at COMPILE time (`{#ReleaseDir}`) and re-reads it at install time. If that
    path is on a VM-shared / network / removable volume (e.g. the Z:\ host mount
    on the dev VM), Windows rejects the read with STATUS_UNTRUSTED_MOUNT_POINT
    ("The path cannot be traversed because it contains an untrusted mount point")
    and the install fails. This script refuses to produce such a broken
    installer: when CARGO_TARGET_DIR is unset and the resolved exe is not on the
    system drive, it aborts with instructions. Set CARGO_TARGET_DIR to a
    system-drive path (e.g. C:\qmk-target) in the shell that runs BOTH cargo and
    this script.

    Works from git-bash too:
      powershell -NoProfile -ExecutionPolicy Bypass -File build.ps1
#>
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path

# Version from Cargo.toml (same logic as ../install.ps1).
$Version = '0.0.0'
$cargo = Join-Path $here '..\..\..\Cargo.toml'
if (Test-Path $cargo) {
    $m = Select-String -Path $cargo -Pattern '^\s*version\s*=\s*"([^"]+)"' | Select-Object -First 1
    if ($m) { $Version = $m.Matches.Groups[1].Value }
}

# Locate ISCC: PATH first, then the winget user-scope install location, then
# the machine-scope Program Files locations.
$iscc = (Get-Command iscc.exe -ErrorAction SilentlyContinue).Source
if (-not $iscc) {
    $candidates = @(
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
    )
    foreach ($c in $candidates) { if (Test-Path $c) { $iscc = $c; break } }
}
if (-not $iscc -or -not (Test-Path $iscc)) {
    throw "ISCC.exe not found. Install Inno Setup 6: winget install JRSoftware.InnoSetup"
}

# ---------------------------------------------------------------------------
# Validate the release exe path BEFORE invoking ISCC.
#
# Resolve ReleaseDir exactly as QMKonnect.iss does (the `{#ReleaseDir}` define):
# $env:CARGO_TARGET_DIR\release if set, else ..\..\..\target\release relative to
# this script. ISCC bakes this absolute path into the installer at compile time;
# the installer re-reads it at install time, so it must be on a TRUSTED volume.
# ---------------------------------------------------------------------------
if ($env:CARGO_TARGET_DIR) {
    $releaseDir = Join-Path $env:CARGO_TARGET_DIR 'release'
} else {
    $releaseDir = Join-Path $here '..\..\..\target\release'
}

if (-not (Test-Path $releaseDir)) {
    throw "Release directory not found: $releaseDir`nRun a release build first:  cargo build --release"
}

$exe = Join-Path (Resolve-Path $releaseDir).Path 'qmkonnect.exe'
if (-not (Test-Path $exe)) {
    throw "Release exe not found: $exe`nRun a release build first:  cargo build --release"
}

# Refuse to bake a source path on a non-system drive when the user has NOT
# explicitly opted in via CARGO_TARGET_DIR. A non-system drive here is almost
# always a VM-shared folder (Z:\) / network share / removable volume, which the
# Windows kernel treats as an untrusted mount point.
$exeDrive  = (($exe -split ':')[0]) + ':'
$sysDrive  = if ($env:SystemDrive) { $env:SystemDrive } else { 'C:' }
if (($exeDrive -ne $sysDrive) -and (-not $env:CARGO_TARGET_DIR)) {
    throw @"
Refusing to bake an untrusted-mount source path into the installer.

  Release exe:   $exe   (drive $exeDrive)
  System drive:  $sysDrive

CARGO_TARGET_DIR is unset and the release exe is not on the system drive. On the
dev VM this is the Z:\ host-shared folder: the Inno installer embeds this source
path at compile time and re-reads it during install, where Windows rejects it:
  STATUS_UNTRUSTED_MOUNT_POINT - "The path cannot be traversed because it
  contains an untrusted mount point."

Fix: build into a system-drive directory and re-run this script FROM THE SAME
shell so cargo and ISCC agree on the location:

    `$env:CARGO_TARGET_DIR = "$sysDrive\qmk-target"
    cargo build --release
    powershell -NoProfile -ExecutionPolicy Bypass -File packaging\windows\inno\build.ps1
"@
}

# Warn (don't fail) if a source file is newer than the release exe. This catches
# both "forgot to rebuild" and "cargo built to a different CARGO_TARGET_DIR than
# this script expects" -- the two ways the installer ends up shipping a stale
# binary. Scan only src\**\*.rs + Cargo.toml (the exe's actual inputs; the .iss
# is excluded because installer changes don't need a cargo rebuild; target\ is
# excluded because it is huge).
$staleCandidates = @()
$srcDir = Join-Path $here '..\..\..\src'
if (Test-Path $srcDir) {
    $staleCandidates += Get-ChildItem -Path $srcDir -Recurse -File -Filter '*.rs' -ErrorAction SilentlyContinue
}
$cargoToml = Join-Path $here '..\..\..\Cargo.toml'
if (Test-Path $cargoToml) { $staleCandidates += Get-Item $cargoToml }
if ($staleCandidates.Count -gt 0) {
    $newestSrc = $staleCandidates | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $exeItem   = Get-Item $exe
    if ($newestSrc.LastWriteTime -gt $exeItem.LastWriteTime) {
        Write-Warning @"
Release exe may be stale: $exe
    exe built:     $($exeItem.LastWriteTime)
    newer source:  $($newestSrc.LastWriteTime)  ($($newestSrc.Name))
A source file is newer than the exe. Rebuild before packaging, or this is the
"cargo built to a different CARGO_TARGET_DIR" foot-gun:
    cargo build --release
Proceeding in 3 seconds (Ctrl-C to abort)...
"@
        Start-Sleep -Seconds 3
    }
}

Write-Host "Building QMKonnect-Setup.exe v$Version"
Write-Host "  source exe: $exe"
& $iscc "/DMyAppVersion=$Version" (Join-Path $here 'QMKonnect.iss')
$exit = $LASTEXITCODE
if ($exit -ne 0) { throw "ISCC failed (exit $exit)" }

Write-Host "Built: $(Join-Path $here 'Output\QMKonnect-Setup.exe')" -ForegroundColor Green

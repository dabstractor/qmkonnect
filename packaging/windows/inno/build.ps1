<#
.SYNOPSIS
    Builds the QMKonnect tray-app installer (Inno Setup) -> Output\QMKonnect-Setup.exe.

.DESCRIPTION
    Reads the version from ..\..\..\Cargo.toml (single source of truth) and runs
    ISCC. Prerequisite: a release build (`cargo build --release`) and Inno Setup 6
    (`winget install JRSoftware.InnoSetup`).

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

Write-Host "Building QMKonnect-Setup.exe v$Version"
& $iscc "/DMyAppVersion=$Version" (Join-Path $here 'QMKonnect.iss')
$exit = $LASTEXITCODE
if ($exit -ne 0) { throw "ISCC failed (exit $exit)" }

Write-Host "Built: $(Join-Path $here 'Output\QMKonnect-Setup.exe')" -ForegroundColor Green

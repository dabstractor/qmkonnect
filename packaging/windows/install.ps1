<#
.SYNOPSIS
    Per-user installer for QMKonnect (no administrator / UAC required).

.DESCRIPTION
    A tray app must run in the interactive user session; a Windows Service runs
    in Session 0 and cannot show a tray icon, so we deliberately do NOT install
    a service. Instead this script does a per-user install:

      * copies qmkonnect.exe + its icon assets to
        %LOCALAPPDATA%\Programs\QMKonnect
      * creates a Start Menu shortcut
      * writes a per-user HKCU `Run` value so the tray app auto-launches
        at login (managed by the in-app "Open at Login" toggle too)
      * registers an uninstall entry (Add/Remove Programs -> "QMKonnect")
      * (re)launches the app

    No elevation prompt. Re-running updates an existing install in place.

.PARAMETER ExePath
    Path to the built release qmkonnect.exe. If omitted, the script looks in
    %CARGO_TARGET_DIR%\release\ and .\target\release\.
#>
[CmdletBinding()]
param(
    [string]$ExePath
)

$ErrorActionPreference = 'Stop'
$App = 'QMKonnect'
$Publisher = 'Mulletware'
$Dest = Join-Path $env:LOCALAPPDATA "Programs\$App"

# --- Locate the built exe ----------------------------------------------------
if (-not $ExePath) {
    $candidates = @()
    if ($env:CARGO_TARGET_DIR) { $candidates += Join-Path $env:CARGO_TARGET_DIR 'release\qmkonnect.exe' }
    $candidates += (Join-Path $PSScriptRoot '..\..\target\release\qmkonnect.exe')
    foreach ($c in $candidates) {
        $full = (Resolve-Path $c -ErrorAction SilentlyContinue).Path
        if ($full) { $ExePath = $full; break }
    }
}
if (-not $ExePath -or -not (Test-Path $ExePath)) {
    throw "qmkonnect.exe not found. Build it first ('cargo build --release') or pass -ExePath <path>."
}

# --- Version from Cargo.toml (single source of truth) -----------------------
$Version = '0.0.0'
$cargo = Join-Path $PSScriptRoot '..\..\Cargo.toml'
if (Test-Path $cargo) {
    $m = Select-String -Path $cargo -Pattern '^\s*version\s*=\s*"([^"]+)"' | Select-Object -First 1
    if ($m) { $Version = $m.Matches.Groups[1].Value }
}

# --- Stop any running instance ----------------------------------------------
Get-Process $App -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500

# --- Install files -----------------------------------------------------------
Write-Host "Installing $App to $Dest"
New-Item -ItemType Directory -Force -Path $Dest | Out-Null
Copy-Item $ExePath (Join-Path $Dest "$App.exe") -Force
# Icon assets: load_windows_tray_icon / load_app_icon look beside the exe first.
foreach ($asset in 'IconTray-dark.png', 'Icon.ico') {
    $assetSrc = Join-Path $PSScriptRoot "..\..\packaging\$asset"
    if (Test-Path $assetSrc) { Copy-Item $assetSrc (Join-Path $Dest $asset) -Force }
}

# --- Shortcuts (Start Menu) + per-user autostart (HKCU Run key) ----------
# The Start Menu shortcut is for manual launching. Autostart is a single
# HKCU `Run` registry value (name $App) — the same one the in-app tray
# "Open at Login" toggle manages — so the checkbox and installer never desync.
# (Previously this dropped a .lnk in shell:startup, whose only off-switch was
# Task Manager → Startup; see HANDOFF_WINDOWS_OPEN_AT_LOGIN.md.)
$Wsh = New-Object -ComObject WScript.Shell
$StartMenu = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\$App.lnk"
$ExeDest = Join-Path $Dest "$App.exe"
$IconDest = Join-Path $Dest 'Icon.ico'
$s = $Wsh.CreateShortcut($StartMenu)
$s.TargetPath = $ExeDest
$s.WorkingDirectory = $Dest
if (Test-Path $IconDest) { $s.IconLocation = $IconDest }
$s.Description = 'QMKonnect - window-change notifier for QMK keyboards'
$s.Save()

# Default-on autostart via the Run key (single source of truth shared with the
# tray toggle in src/autostart.rs and the removal in uninstall.ps1).
$RunKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
Set-ItemProperty -Path $RunKey -Name $App -Value $ExeDest

# --- Add/Remove Programs uninstall entry ------------------------------------
# Drop the uninstaller next to the exe so UninstallString always resolves.
$UninstallPs1 = Join-Path $Dest 'uninstall.ps1'
Copy-Item (Join-Path $PSScriptRoot 'uninstall.ps1') $UninstallPs1 -Force

$reg = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$App"
New-Item -Path $reg -Force | Out-Null
Set-ItemProperty $reg 'DisplayName'      $App
Set-ItemProperty $reg 'DisplayVersion'   $Version
Set-ItemProperty $reg 'Publisher'        $Publisher
Set-ItemProperty $reg 'InstallLocation'  $Dest
if (Test-Path $IconDest) { Set-ItemProperty $reg 'DisplayIcon' $IconDest }
Set-ItemProperty $reg 'NoModify'  1 -Type DWord
Set-ItemProperty $reg 'NoRepair'  1 -Type DWord
Set-ItemProperty $reg 'UninstallString' "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$UninstallPs1`""

# --- Launch -----------------------------------------------------------------
Start-Process $ExeDest -WorkingDirectory $Dest

Write-Host ""
Write-Host "Done. $App $Version installed (per-user, no admin)." -ForegroundColor Green
Write-Host "  App:       $ExeDest"
Write-Host "  Autostart: HKCU\...\Run\$App (toggle in the tray: Open at Login)"
Write-Host "  Uninstall: Add/Remove Programs, or: powershell -File `"$UninstallPs1`""

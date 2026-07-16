<#
.SYNOPSIS
    Per-user uninstaller for QMKonnect (companion to install.ps1).
#>
$ErrorActionPreference = 'Continue'
$App = 'QMKonnect'
$Dest = Join-Path $env:LOCALAPPDATA "Programs\$App"

# Stop it.
Get-Process $App -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500

# Shortcuts.
Remove-Item (Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\$App.lnk") -Force -ErrorAction SilentlyContinue

# Per-user autostart (HKCU Run key — written by install.ps1 and the tray
# "Open at Login" toggle). Name must match exactly across all three places.
Remove-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" `
                    -Name $App -ErrorAction SilentlyContinue

# Files.
Remove-Item $Dest -Recurse -Force -ErrorAction SilentlyContinue

# Uninstall registry entry.
Remove-Item "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$App" -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "Uninstalled $App." -ForegroundColor Green

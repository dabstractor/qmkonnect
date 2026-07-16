# Windows Service Installer for QMKonnect

> **Looking for the end-user tray-app installer?** It's in **[`./inno/`](inno/)** (Inno Setup
> → `QMKonnect-Setup.exe`, per-user, no admin, double-click to install). The WiX
> installer below is a **different** product: it installs a Session-0 *service*
> that cannot show a tray icon in your interactive session (see
> [`../../AGENTS.md`](../../AGENTS.md)). Use `inno/` for the tray app; use this
> (WiX) only if you specifically want the headless service.

This directory contains the Windows installer configuration using WiX Toolset. The installer creates a proper Windows service that runs silently in the background without any console windows.

## Prerequisites

1. **WiX Toolset v3.x** - Download from [wixtoolset.org](https://wixtoolset.org/releases/)
   - Or install via Chocolatey: `choco install wixtoolset`
   - Or install via winget: `winget install WiXToolset.WiX`

2. **Rust toolchain** - The application must be built in release mode first

## Building the Installer

### Option 1: PowerShell Script (Recommended)
```powershell
# Build application and create installer
.\build-installer.ps1

# Skip application build (if already built)
.\build-installer.ps1 -SkipBuild

# Show help
.\build-installer.ps1 -Help
```

### Option 2: Batch File
```cmd
build-installer.bat
```

### Option 3: Manual Build
```cmd
# 1. Build the Rust application
cd ..\..
cargo build --release

# 2. Compile WiX source
cd packaging\windows
candle.exe installer.wxs

# 3. Create MSI
light.exe installer.wixobj -ext WixUIExtension -out qmkonnect-Setup.msi
```

## Installer Features

- **Windows Service Installation** - Automatically installs and configures the Windows service
- **Silent Operation** - No console windows or terminal popups
- **Automatic Startup** - Service starts automatically on system boot
- **Professional MSI installer** with standard Windows installer UI
- **Start menu shortcut** - Adds program to Start Menu for manual service management
- **Proper uninstall** - Clean removal including service unregistration
- **Per-machine installation** - Installs for all users
- **Upgrade support** - Handles upgrades from previous versions

## Installation Process

The installer automatically:
1. Copies the executable to Program Files
2. Registers the Windows service
3. Starts the service immediately
4. Configures the service for automatic startup
5. Creates Start Menu shortcuts

## Service Management

After installation, the service can be managed through:

### Windows Services Panel
- Open `services.msc`
- Look for "QMKonnect"
- Right-click for Start/Stop/Restart options

### Command Line
```cmd
# Start service
sc start QMKonnect

# Stop service
sc stop QMKonnect

# Query service status
sc query QMKonnect
```

### Application Commands
```cmd
# Install service manually
qmkonnect.exe --install-service

# Uninstall service manually
qmkonnect.exe --uninstall-service

# Start/stop service manually
qmkonnect.exe --start-service
qmkonnect.exe --stop-service
```

## Files Included

- `installer.wxs` - Main WiX installer configuration with service support
- `license.rtf` - MIT license in RTF format for installer UI
- `build-installer.ps1` - PowerShell build script with enhanced features
- `build-installer.bat` - Batch file build script
- `README.md` - This documentation

## Output

The build process creates `qmkonnect-Setup.msi` which can be distributed to end users.

## User Experience

After installation:
- **No visible windows** - The application runs completely in the background
- **System tray icon** - Appears automatically for user interaction
- **Automatic startup** - Starts with Windows, no user intervention required
- **Silent operation** - All logging goes to Windows Event Log

## Logging and Diagnostics

The service logs events to the Windows Event Log:
- Open Event Viewer
- Navigate to "Applications and Services Logs"
- Look for "QMKonnect" entries

## Troubleshooting

### WiX Not Found
- Install WiX Toolset v3.x from the official website
- Ensure `candle.exe` and `light.exe` are in your PATH
- Restart your command prompt after installation

### Build Errors
- Ensure the Rust application builds successfully with `cargo build --release`
- Check that `target/release/qmkonnect.exe` exists
- Verify all file paths in `installer.wxs` are correct

### Service Issues
- Check Windows Event Log for error messages
- Verify service is installed: `sc query QMKonnect`
- Try manual service commands for debugging
- Ensure installer was run as Administrator

### Icon Issues
- The installer references `../Icon.png` - ensure this file exists
- Icon should be in PNG format for WiX compatibility

## Uninstallation

The installer provides clean uninstallation:
1. Stops the running service
2. Unregisters the Windows service
3. Removes all installed files
4. Cleans up registry entries
5. Removes Start Menu shortcuts

Users can uninstall through:
- Windows Settings > Apps & Features
- Control Panel > Programs and Features
- Right-click on MSI file > Uninstall
@echo off
setlocal enabledelayedexpansion

echo QMKonnect Installer Build Script
echo ==========================================

REM Check if WiX is installed
where candle.exe >nul 2>&1
if %errorlevel% neq 0 (
    echo ERROR: WiX Toolset not found!
    echo Please install WiX Toolset v3.x from https://wixtoolset.org/releases/
    echo Or install via package manager:
    echo   Chocolatey: choco install wixtoolset
    echo   Winget: winget install WiXToolset.WiX
    exit /b 1
)

where light.exe >nul 2>&1
if %errorlevel% neq 0 (
    echo ERROR: WiX Toolset not found!
    echo Please install WiX Toolset v3.x from https://wixtoolset.org/releases/
    exit /b 1
)

echo WiX Toolset found

REM Determine the version from Cargo.toml (single source of truth; MSI Product
REM version mirrors it). Match the line that begins with `version `.
for /f "tokens=2 delims==" %%a in ('findstr /b /c:"version " ..\..\Cargo.toml') do set "_VERRAW=%%a"
if not defined _VERRAW (
    echo ERROR: Could not find a line starting with 'version ' in ..\..\Cargo.toml
    exit /b 1
)
set "VERSION=%_VERRAW:"=%"
set "VERSION=%VERSION: =%"
echo Version: %VERSION%

REM Build Rust application
echo Building Rust application...
pushd ..\..

echo Cleaning previous build...
cargo clean

echo Building in release mode...
cargo build --release
if %errorlevel% neq 0 (
    echo ERROR: Cargo build failed
    popd
    exit /b 1
)

REM Verify executable exists
if not exist "target\release\qmkonnect.exe" (
    echo ERROR: Executable not found
    popd
    exit /b 1
)

echo Rust application built successfully
popd

REM Build installer
echo Building Windows installer...

echo Compiling WiX source...
REM Inject the Cargo version into the .wxs via sentinel replacement.
powershell -NoProfile -Command "(Get-Content installer.wxs) -replace '__VERSION__', '%VERSION%' | Set-Content installer.wxs"
candle.exe installer.wxs -ext WixUtilExtension
if %errorlevel% neq 0 (
    echo ERROR: WiX compilation failed
    exit /b 1
)

echo Creating MSI package...
light.exe installer.wixobj -ext WixUIExtension -ext WixUtilExtension -out "qmkonnect-Setup.msi"
if %errorlevel% neq 0 (
    echo ERROR: MSI creation failed
    exit /b 1
)

REM Clean up intermediate files
del installer.wixobj >nul 2>&1
del *.wixpdb >nul 2>&1

echo.
echo SUCCESS: Installer created successfully: qmkonnect-Setup.msi
echo.
echo Installation Instructions:
echo 1. Run qmkonnect-Setup.msi as Administrator
echo 2. The service will be installed and started automatically
echo 3. System tray icon should appear after installation
echo.
echo Service Management:
echo - View service: services.msc (look for 'QMKonnect')
echo - Manual control: sc start/stop QMKonnect
echo - Logs: Windows Event Viewer ^> Applications and Services Logs

endlocal
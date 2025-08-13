---
layout: default
icon: fas fa-download
order: 2
---

# Installation

QMKonnect provides pre-built binaries for all major platforms. Choose your operating system below for installation instructions.

## Windows

### Option 1: MSI Installer (Recommended)
1. Download the latest `QMK-Window-Notifier-Setup.msi` from [GitHub Releases](https://github.com/dabstractor/qmkonnect/releases)
2. Run the installer as Administrator
3. Follow the installation wizard
4. QMKonnect will be installed to `C:\Program Files\QMKonnect\`

### Option 2: Portable Executable
1. Download the latest `qmkonnect-windows.exe` from [GitHub Releases](https://github.com/dabstractor/qmkonnect/releases)
2. Place the executable in your preferred directory
3. Run directly - no installation required

## macOS

### Option 1: Homebrew (Recommended)
```bash
# Add the tap (coming soon)
brew tap dabstractor/qmkonnect
brew install qmkonnect
```

### Option 2: Direct Download
1. Download the latest `qmkonnect-macos` from [GitHub Releases](https://github.com/dabstractor/qmkonnect/releases)
2. Move to `/usr/local/bin/` or your preferred location
3. Make executable: `chmod +x qmkonnect-macos`

## Linux

### Option 1: Package Managers

#### Ubuntu/Debian (APT)
```bash
# Add repository (coming soon)
curl -fsSL https://github.com/dabstractor/qmkonnect/releases/download/latest/qmkonnect.deb -o qmkonnect.deb
sudo dpkg -i qmkonnect.deb
```

#### Arch Linux (AUR)
```bash
# Using yay
yay -S qmkonnect

# Using paru
paru -S qmkonnect
```

#### Fedora/RHEL (RPM)
```bash
# Download and install RPM (coming soon)
curl -fsSL https://github.com/dabstractor/qmkonnect/releases/download/latest/qmkonnect.rpm -o qmkonnect.rpm
sudo rpm -i qmkonnect.rpm
```

### Option 2: Direct Download
1. Download the latest `qmkonnect-linux` from [GitHub Releases](https://github.com/dabstractor/qmkonnect/releases)
2. Move to `/usr/local/bin/` or your preferred location
3. Make executable: `chmod +x qmkonnect-linux`

## Building from Source

If you prefer to build from source or need to customize the build:

### Prerequisites
- Rust 1.70 or later
- Git

### Build Steps
```bash
# Clone the repository
git clone https://github.com/dabstractor/qmkonnect.git
cd qmkonnect

# Build release version
cargo build --release

# The binary will be in target/release/
```

## Verification

After installation, verify QMKonnect is working:

```bash
# Check version
qmkonnect --version

# Test basic functionality
qmkonnect --help
```

## Next Steps

Once installed, proceed to:
1. **[QMK Integration](qmk-integration)** - Set up your keyboard
2. **[Configuration](configuration)** - Customize the application
3. **[Usage](usage)** - Start using QMKonnect

## Troubleshooting Installation

If you encounter issues during installation, check the [Troubleshooting](troubleshooting) page for common solutions.
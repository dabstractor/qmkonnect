---
layout: default
title: Installation
permalink: /installation/
---

# Installation Guide

QMKonnect has different installation methods for each platform.

## Windows

### MSI Installer (Recommended)

1. Download the latest MSI installer: [QMKonnect.msi](https://github.com/dabstractor/qmkonnect/releases/download/v0.1.0/QMKonnect.msi)
2. Run the installer as Administrator
3. The application will start automatically and be added to Windows startup

The MSI installer:
- Installs to Program Files
- Adds to Windows startup
- Sets up system tray icon
- Can be uninstalled normally

### Build from Source

```bash
git clone https://github.com/dabstractor/qmkonnect.git
cd qmkonnect/packaging/windows
./build-installer.ps1
```

This will create `qmkonnect-Setup.msi` which you can then install.

---

## Linux

### Linux (Hyprland Only)

**Note**: QMKonnect currently only supports Hyprland on Linux. Other window managers are not supported yet. Please contribute support for your window manager!

#### Arch Linux

Install from the AUR or build the package:

```bash
git clone https://github.com/dabstractor/qmkonnect.git
cd qmkonnect/packaging/linux/arch
makepkg -si
systemctl --user enable --now qmkonnect # if you want it to start on hotplug
```

#### Other Linux Distributions

1. Download the release binary: [qmkonnect](https://github.com/dabstractor/qmkonnect/releases/download/v0.1.0/qmkonnect)
2. Install the binary:

```bash
# Make executable and copy to PATH
chmod +x qmkonnect
sudo cp qmkonnect /usr/local/bin/
```

3. Set up systemd service (optional but recommended):

```bash
# Install service file
curl https://raw.githubusercontent.com/dabstractor/qmkonnect/refs/heads/main/packaging/linux/systemd/qmkonnect.service.template | sudo tee /usr/lib/systemd/user/qmkonnect.service

# Enable and start the service
systemctl --user enable --now qmkonnect.service
```

4. Set up udev rules for automatic keyboard detection:

```bash
# Install udev rules
curl https://raw.githubusercontent.com/dabstractor/qmkonnect/refs/heads/main/packaging/linux/udev/99-qmkonnect.rules.template | sudo tee /etc/udev/rules.d/99-qmkonnect.rules.template

# Create config and reload rules
qmkonnect -c
sudo qmkonnect -r
sudo udevadm control --reload && sudo udevadm trigger
```

---

## macOS

### Application Bundle

1. Download QMKonnect.app from the [releases page](https://github.com/dabstractor/qmkonnect/releases/download/v0.1.0/QMKonnect.dmg)
2. Copy QMKonnect.app to your Applications folder
3. Launch QMKonnect from Applications folder

### Build from Source

```bash
git clone https://github.com/dabstractor/qmkonnect.git
cd qmkonnect/packaging/macos
./build.sh
```

Then copy the generated QMKonnect.app to your /Applications folder.

---

## Build from Source (Linux Only)

For Linux users who want to build from source:

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable version)
- Platform dependencies:
  - **Ubuntu/Debian**: `sudo apt install libxdo-dev libudev-dev`
  - **Fedora**: `sudo dnf install libxdo-devel systemd-devel`

### Build Steps

```bash
# Clone the repository
git clone https://github.com/dabstractor/qmkonnect.git
cd qmkonnect

# Build the project
cargo build --release

# The binary will be available at target/release/qmkonnect
```

---

## Verification

After installation, verify QMKonnect is working:

1. **Check if running**:
   - Windows: Look for the system tray icon
   - Linux: `systemctl --user status qmkonnect`
   - macOS: Check Activity Monitor

2. **Test configuration**:
   - Windows: Right-click system tray icon → Settings
   - Linux: `qmkonnect -c` then `qmkonnect -v`
   - macOS: Right-click menu bar icon → Settings

3. **Check logs**:
   - Windows: System tray interface
   - Linux: `journalctl --user -u qmkonnect`
   - macOS: System menu bar interface

---

## Next Steps

After installation:

1. [Set up your QMK firmware]({{ site.baseurl }}/qmk-integration)
2. [Configure your keyboard settings]({{ site.baseurl }}/configuration)
3. [Start using QMKonnect]({{ site.baseurl }}/usage)

---

📖 **[Complete Documentation]({{ site.baseurl }}/llms_full.txt)** - All guides in one comprehensive file

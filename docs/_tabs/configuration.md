---
layout: default
icon: fas fa-cog
order: 3
---

# Configuration Guide

QMKonnect requires only two configuration values: your keyboard's vendor ID and product ID. The configuration method varies by platform.

## Platform-Specific Configuration

### Windows & macOS - GUI Settings

Both Windows and macOS use a settings dialog through the system tray:

1. **Find the system tray icon** (QMKonnect icon in your system tray/menu bar)
2. **Right-click the icon** and select "Settings"
3. **Enter your keyboard IDs**:
   - **Vendor ID**: Your keyboard's vendor ID in hex format (e.g., `feed`)
   - **Product ID**: Your keyboard's product ID in hex format (e.g., `0000`)
4. **Click OK** to save

Settings are saved automatically and work right away - no restart needed.

### Linux - Configuration File

Linux uses a TOML configuration file located at `~/.config/qmk-notifier/config.toml`.

#### Creating the Configuration File

```bash
qmkonnect -c
```

This creates a default configuration file with these contents:

```toml
# QMKonnect Configuration

# Your QMK keyboard's vendor ID (in hex)
vendor_id = 0xfeed

# Your QMK keyboard's product ID (in hex)
product_id = 0x0000

# Add any other configuration options here
```

#### Editing the Configuration

Edit the file with your preferred text editor:

```bash
# Using nano
nano ~/.config/qmk-notifier/config.toml

# Using vim
vim ~/.config/qmk-notifier/config.toml
```

Update the values:
```toml
vendor_id = 0x1234  # Replace with your keyboard's vendor ID
product_id = 0x5678  # Replace with your keyboard's product ID
```

#### Reloading Configuration (Linux Only)

After editing the configuration file, reload it:

```bash
qmkonnect -r
```

This updates the system configuration (udev rules) and reloads the settings.

## Finding Your Keyboard IDs

To configure QMKonnect for your keyboard, you need to find your keyboard's vendor ID and product ID.

### Method 1: QMK Configuration

If you have your QMK configuration, look for these values in your `config.h`:

```c
#define VENDOR_ID    0xFEED
#define PRODUCT_ID   0x0000
```

### Method 2: System Tools

#### Windows
```powershell
# Using PowerShell
Get-WmiObject -Class Win32_USBHub | Where-Object {$_.Name -like "*keyboard*"}

# Or use Device Manager:
# 1. Open Device Manager
# 2. Expand "Keyboards" or "Human Interface Devices"
# 3. Right-click your keyboard → Properties → Details
# 4. Select "Hardware Ids" from dropdown
```

#### Linux
```bash
# List USB devices
lsusb

# More detailed info
lsusb -v | grep -A 5 -B 5 "keyboard\|Keyboard"

# Check hidraw devices
ls -la /dev/hidraw*
cat /sys/class/hidraw/hidraw*/device/uevent
```

#### macOS
```bash
# System Information
system_profiler SPUSBDataType | grep -A 10 -B 10 "keyboard\|Keyboard"

# Or use ioreg
ioreg -p IOUSB | grep -A 10 -B 10 "keyboard\|Keyboard"
```

## Reloading Configuration

After modifying the configuration file, reload it without restarting:

```bash
qmkonnect -r
```

### Linux Additional Steps

On Linux, if you modified udev rules or systemd services, also run:

```bash
# Reload udev rules
sudo udevadm control --reload && sudo udevadm trigger

# Restart systemd service
systemctl --user restart qmkonnect
```

## Configuration Examples

### Basic Configuration (Linux)
```toml
# Minimal configuration - just keyboard IDs
vendor_id = 0xfeed
product_id = 0x0000
```

### Custom Keyboard IDs (Linux)
```toml
# Example with different keyboard IDs
vendor_id = 0x1234
product_id = 0x5678
```

## Validation

To validate your configuration:

```bash
# Test with verbose output to see if keyboard is detected
qmkonnect -v
```

## Troubleshooting Configuration

### Common Issues

1. **Keyboard not detected**:
   - Verify vendor_id and product_id are correct
   - Check if keyboard supports Raw HID
   - Ensure QMK firmware has the notifier module

2. **Permission errors (Linux)**:
   - Add user to appropriate groups: `sudo usermod -a -G input,plugdev $USER`
   - Check udev rules are installed correctly

3. **Configuration not loading**:
   - Verify file path and permissions
   - Check TOML syntax with a validator
   - Use `qmkonnect -v` to see detailed error messages

### Debug Mode

Run with maximum verbosity to diagnose issues:

```bash
qmkonnect -v --debug
```

This will show:
- Configuration file loading
- Keyboard detection attempts
- Window monitoring events
- Communication with keyboard
---
layout: default
title: Configuration
permalink: /configuration/
---

# Configuration Guide

This guide covers configuring QMKonnect to communicate with your keyboard. You only need to provide two values: your keyboard's vendor ID and product ID.

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

After configuration, test that QMKonnect can detect your keyboard:

### Windows & macOS
Check the system tray/menu bar icon - it should show as connected.

### Linux
```bash
# Test with verbose output to see if keyboard is detected
qmkonnect -v
```

If you see "Keyboard detected" messages, you're ready to use QMKonnect.

## Troubleshooting

If your keyboard isn't detected:

1. **Double-check your vendor/product IDs** - they must match exactly
2. **Verify Raw HID is enabled** in your QMK firmware 
3. **Check permissions** (Linux users may need to be in `input` and `plugdev` groups)

For detailed troubleshooting steps, see the [troubleshooting guide]({{ site.baseurl }}/troubleshooting).

---

## QMK Firmware Configuration

Once QMKonnect can detect your keyboard, configure your QMK firmware to respond to window changes.

The qmk-notifier framework provides two main configuration macros:

### Layer Switching with DEFINE_SERIAL_LAYERS

Automatically switch layers based on active windows:

```c
DEFINE_SERIAL_LAYERS({
    // Basic application matching
    { "*calculator*", _NUMPAD },
    { "*chrome*", _BROWSER },
    { "*terminal*", _TERMINAL },
    
    // Specific window title matching
    { WT("*chrome*", "*jitsi*"), _JITSI },
    { WT("alacritty", "terminal"), _TERMINAL },
    
    // Gaming applications
    { "steam_app*", _GAMING },
    { WT("cs2", "Counter-Strike 2"), _GAMING },
});
```

### Custom Commands with DEFINE_SERIAL_COMMANDS

Execute custom functions based on window changes:

```c
DEFINE_SERIAL_COMMANDS({
    // Disable vim mode for specific applications
    { "neovide", &disable_vim_mode },
    { "alacritty", &disable_vim_mode },
    
    // Multiple commands for AI chat interfaces
    { WT("*chrome*", "*claude*"), &vim_insert, &disable_vim_mode },
    { WT("*chrome*", "*chatgpt*"), &vim_insert, &disable_vim_mode },
    
    // Gaming applications
    { WT("steam_app*", "*"), &disable_vim_mode },
});
```

## Framework Elements

### Available Macros

- **`DEFINE_SERIAL_LAYERS`**: Maps window patterns to keyboard layers
- **`DEFINE_SERIAL_COMMANDS`**: Maps window patterns to command functions  
- **`WT(class, title)`**: Helper macro to match both window class and title
- **Wildcard matching**: Use `*` for pattern matching (e.g., `"*chrome*"`)

### Understanding Window Matching

QMKonnect sends window information in this format:
```
{application_class}{GS}{window_title}
```
Where `{GS}` is the Group Separator character (ASCII 0x1D).

Examples:
- VS Code: `code{GS}main.rs - qmkonnect`
- Firefox: `firefox{GS}GitHub - Mozilla Firefox`
- Terminal: `terminal{GS}~/projects/qmkonnect`

## Pattern Matching Examples

```c
// Match any calculator app
{ "*calculator", _NUMPAD }

// Match specific browser with specific site
{ WT("*chrome*", "*jitsi*"), _JITSI }

// Match terminal with specific title
{ WT("alacritty", "terminal"), _TERMINAL }

// Match any Steam game
{ "steam_app*", _GAMING }
```

---

## Next Steps

- [Learn how to use QMKonnect]({{ site.baseurl }}/usage)
- [See real-world examples]({{ site.baseurl }}/examples)

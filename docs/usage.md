---
layout: default
title: Usage
permalink: /usage/
---

# Usage Guide

> **Prerequisite:** This assumes you've already integrated the
> [qmk_notifier](https://github.com/dabstractor/qmk_notifier) module into your
> firmware (see [QMK Integration]({{ site.baseurl }}/qmk-integration)). QMKonnect
> only sends window data — without the firmware module, nothing will switch.

QMKonnect runs in the background, automatically detecting window changes and communicating with your QMK keyboard. Here's how to start, stop, and manage the application.

## Starting QMKonnect

### Windows
- **Automatic startup**: If installed via MSI, starts automatically with Windows
- **Manual start**: Find "QMKonnect" in Start Menu or double-click the desktop shortcut
- **System tray**: Look for the QMKonnect icon in your system tray when running

### Linux  
- **Automatic startup**: `systemctl --user enable qmkonnect` (runs on login)
- **Manual start**: `qmkonnect` or `systemctl --user start qmkonnect`
- **Check status**: `systemctl --user status qmkonnect`

### macOS
- **Manual start**: Launch QMKonnect.app from Applications folder
- **Menu bar**: Look for the QMKonnect icon in your menu bar when running

## Stopping QMKonnect

### Windows
Right-click the system tray icon and select "Quit"

### Linux
```bash
systemctl --user stop qmkonnect
```

### macOS  
Quit from the menu bar icon or application menu

## Auto-Start on Boot

### Windows
**Open at Login** is enabled by default. Toggle it from the system-tray icon → **Open at Login**.
It's backed by the HKCU `Run` key (you can also disable it in Task Manager →
Startup, but the tray toggle is the intended way).

### Linux
```bash
# Enable auto-start on login
systemctl --user enable qmkonnect

# Disable auto-start
systemctl --user disable qmkonnect
```

### macOS
1. System Preferences → Users & Groups → Login Items
2. Add QMKonnect.app to start automatically

## How It Works

Once running, QMKonnect automatically:

1. **Monitors window changes** - detects when you switch between applications
2. **Extracts window information** - gets the application name and window title  
3. **Sends data to your keyboard** - your QMK firmware receives this information
4. **Triggers layer changes** - your keyboard responds based on your configuration

The magic happens in your QMK firmware configuration - QMKonnect just provides the window information your keyboard needs to make intelligent decisions.

## System Integration

### Windows
- **System tray integration**: Right-click the tray icon for settings and status
- **Runs in background**: Minimal resource usage
- **Auto-updates**: Receives automatic updates when available

### Linux  
- **Systemd service**: Integrates with your system's service management
- **Hyprland support**: Currently supports Hyprland window manager only
- **Lightweight**: Designed for minimal system impact

### macOS
- **Menu bar integration**: Access settings and status from the menu bar
- **Accessibility permissions**: Requires one-time setup for window monitoring
- **Native app bundle**: Standard macOS application behavior

## What QMKonnect Enables

With QMKonnect running, your keyboard becomes context-aware:

- **Development environments**: Automatically switch to coding-focused layouts when opening IDEs
- **Gaming**: Enter gaming mode when launching games  
- **Browser work**: Activate browser-specific shortcuts and layers
- **Terminal usage**: Switch to terminal-optimized layouts
- **Media control**: Enable media keys when using music/video applications

The behavior is entirely customized in your QMK firmware - QMKonnect just provides the window information your keyboard needs.

## Status and Monitoring

### Check if QMKonnect is Running

- **Windows**: Look for the QMKonnect icon in your system tray
- **Linux**: `systemctl --user status qmkonnect`
- **macOS**: Look for the QMKonnect icon in your menu bar

### Verify Keyboard Connection

If your layers aren't switching as expected, read the tray/menu-bar icon —
it's three-state:

- **● Device Connected** — a qmk_notifier-capable board is present (you're set).
- **⚠ QMK board found — no qmk_notifier module (flash it)** — a QMK board is
  attached but isn't running qmk_notifier; flash it (see the
  [QMK Integration Guide]({{ site.baseurl }}/qmk-integration)). This is the
  most common cause of "running but nothing happens."
- **○ No Device Connected** — no QMK Raw-HID board detected.

Then:

1. Verify your QMK firmware is properly configured with the qmk_notifier module
2. Test by switching between different applications

For detailed troubleshooting, see the [troubleshooting guide]({{ site.baseurl }}/troubleshooting).

---

## Next Steps

- [See real-world examples]({{ site.baseurl }}/examples)
- [Learn about troubleshooting]({{ site.baseurl }}/troubleshooting)
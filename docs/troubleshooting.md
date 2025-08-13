---
layout: default
title: Troubleshooting
permalink: /troubleshooting/
---

# Troubleshooting Guide

Common issues and solutions for QMKonnect across different platforms.

## Debugging Tools

Before diving into specific issues, familiarize yourself with these debugging tools:

### Linux Command Line Options

```bash
qmkonnect [OPTIONS]

Options:
    -c, --config         Create a default configuration file
    -r, --reload         Reload configuration from file  
    -v, --verbose        Enable verbose logging
    --debug              Maximum verbosity for debugging
    --test-connection    Test keyboard connection
    -h, --help          Show help information
    -V, --version       Show version information
```

### Verbose Logging

Run with verbose logging to see all activity:

```bash
# Linux
qmkonnect -v

# Shows:
# - Window change events
# - Application detection  
# - Data sent to keyboard
# - Connection status
# - Error messages
```

### Debug Mode (Linux)

For detailed troubleshooting:

```bash
qmkonnect --debug

# Shows:
# - Raw window data
# - Filtering decisions
# - Communication protocol details
# - Timing information
```

### View Logs

**Linux (systemd):**
```bash
# View logs
journalctl --user -u qmkonnect -f

# Check service status  
systemctl --user status qmkonnect
```

**Windows & macOS:** Access logs through the system tray/menu bar interface.

## General Issues

### QMKonnect Won't Start

**Symptoms**: App doesn't start or exits right away

**Solutions**:

1. **Check configuration file**:
   ```bash
   qmkonnect -c  # Create default config
   qmkonnect -v  # Run with verbose output
   ```

2. **Check dependencies**:
   - Make sure required libraries are installed
   - Check system compatibility

3. **Run with debug output**:
   ```bash
   qmkonnect --debug
   ```

4. **Check permissions**:
   - Linux: User in `input` and `plugdev` groups
   - macOS: Accessibility permissions granted
   - Windows: Run as Administrator if needed

### Keyboard Not Detected

**Symptoms**: QMKonnect runs but doesn't communicate with keyboard, layers don't switch

**Quick Diagnosis**:
```bash
# Linux - test keyboard connection
qmkonnect --test-connection

# Check if keyboard shows up in verbose mode
qmkonnect -v | grep -i "keyboard\|detect\|connect"
```

**Find Available HID Devices**:
```bash
# Linux:
ls -la /dev/hidraw*
cat /sys/class/hidraw/hidraw*/device/uevent | grep -E "HID_ID|HID_NAME"

# Windows (PowerShell):
Get-WmiObject -Class Win32_USBHub | Where-Object {$_.Name -like "*keyboard*"}

# macOS:
system_profiler SPUSBDataType | grep -A 10 -B 10 -i keyboard
```

**Solutions**:

1. **Verify keyboard configuration**:
   - Check vendor_id and product_id in config
   - Ensure Raw HID is enabled in QMK firmware
   - Confirm qmk-notifier module is included

2. **Check QMK firmware**:
   ```c
   // In rules.mk
   RAW_ENABLE = yes
   
   // In config.h
   #define RAW_USAGE_PAGE 0xFF60
   #define RAW_USAGE_ID   0x61
   ```

3. **Permission issues (Linux)**:
   ```bash
   # Add user to groups
   sudo usermod -a -G input,plugdev $USER
   
   # Install udev rules
   sudo cp packaging/linux/udev/99-qmkonnect.rules.template /etc/udev/rules.d/99-qmkonnect.rules
   sudo udevadm control --reload && sudo udevadm trigger
   ```

### Window Detection Not Working

**Symptoms**: QMKonnect runs but doesn't detect window changes, keyboard doesn't switch layers when changing applications

**Debugging Steps**:

1. **Check verbose output for window events**:
   ```bash
   # Linux 
   qmkonnect -v | grep -i "window\|app\|title"
   
   # You should see messages like:
   # "Window changed: firefox -> Visual Studio Code" 
   # "Sending: code{GS}main.rs - qmkonnect"
   ```

2. **Test window information format**:
   
   QMKonnect sends data in this format: `{application_class}{GS}{window_title}`
   
   Where `{GS}` is Group Separator (ASCII 0x1D). Examples:
   - VS Code: `code{GS}main.rs - qmkonnect`
   - Firefox: `firefox{GS}GitHub - Mozilla Firefox`  
   - Terminal: `terminal{GS}~/projects/qmkonnect`

**Platform-specific solutions**:

#### Windows
```bash
# Check if window hooks are working
qmkonnect -v  # Look for "Window changed" messages
```

#### Linux (Hyprland Only)
```bash
# Check Hyprland integration
echo $HYPRLAND_INSTANCE_SIGNATURE

# Test Hyprland socket manually
socat -u UNIX-CONNECT:/tmp/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket2.sock - | head -10

# Should show window events when you switch applications
```

**Note**: Only Hyprland is supported on Linux. Other window managers are not supported yet.

#### macOS
1. **Grant Accessibility permissions** (required for window monitoring):
   - System Preferences → Security & Privacy → Privacy  
   - Select "Accessibility" from left panel
   - Add QMKonnect to allowed applications

2. **Test window detection**:
   ```bash
   # Run from terminal to see debug output
   ./QMKonnect.app/Contents/MacOS/qmkonnect -v
   ```

## Platform-Specific Issues

### Windows Issues

#### System Tray Icon Missing

**Solutions**:
1. Check if running as tray app:
   ```bash
   qmkonnect --tray-app
   ```

2. Restart Windows Explorer:
   ```powershell
   taskkill /f /im explorer.exe
   start explorer.exe
   ```

3. Check system tray settings:
   - Settings → Personalization → Taskbar
   - Select which icons appear on taskbar

#### Multiple Instances Running

**Symptoms**: Multiple QMKonnect processes in Task Manager

**Solutions**:
1. Kill all instances:
   ```powershell
   taskkill /f /im qmkonnect.exe
   ```

2. Start single instance:
   ```bash
   qmkonnect --tray-app
   ```

3. Check startup folder for duplicates:
   - `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup`

#### Permission Errors

**Solutions**:
1. Run as Administrator
2. Check Windows Defender exclusions
3. Verify antivirus isn't blocking the application

### Linux Issues

#### Systemd Service Fails

**Check service status**:
```bash
systemctl --user status qmkonnect
journalctl --user -u qmkonnect -f
```

**Common fixes**:
1. **Service file issues**:
   ```bash
   # Reinstall service file
   curl https://raw.githubusercontent.com/dabstractor/qmkonnect/main/packaging/linux/systemd/qmkonnect.service.template | tee ~/.config/systemd/user/qmkonnect.service
   
   systemctl --user daemon-reload
   systemctl --user enable --now qmkonnect
   ```

2. **Binary path issues**:
   ```bash
   # Verify binary location
   which qmkonnect
   
   # Update service file if needed
   systemctl --user edit qmkonnect
   ```

#### Hyprland Integration Issues

**Check Hyprland socket**:
```bash
# Verify socket exists
ls -la /tmp/hypr/$HYPRLAND_INSTANCE_SIGNATURE/

# Test socket communication
socat -u UNIX-CONNECT:/tmp/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket2.sock - | head -10
```

**Solutions**:
1. **Socket permission issues**:
   ```bash
   # Check socket permissions
   ls -la /tmp/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket2.sock
   
   # Restart Hyprland if needed
   ```

**Note**: Only Hyprland is supported on Linux. Other window managers are not supported yet. Please contribute support for your window manager!

### macOS Issues

#### Accessibility Permissions

**Grant permissions**:
1. System Preferences → Security & Privacy → Privacy
2. Select "Accessibility" from left panel
3. Click lock to make changes
4. Add QMKonnect to allowed applications

**Verify permissions**:
```bash
# Check current permissions
sqlite3 /Library/Application\ Support/com.apple.TCC/TCC.db "SELECT * FROM access WHERE service='kTCCServiceAccessibility';"
```

#### Application Bundle Issues

**Solutions**:
1. **Rebuild application bundle**:
   ```bash
   cd packaging/macos
   ./build.sh
   ```

2. **Check code signing**:
   ```bash
   codesign -v QMKonnect.app
   ```

3. **Reset application permissions**:
   ```bash
   tccutil reset Accessibility com.yourcompany.qmkonnect
   ```

## Configuration Issues

### Invalid Configuration File

**Symptoms**: Configuration errors on startup

**Solutions**:
1. **Validate TOML syntax**:
   ```bash
   # Use online TOML validator or
   python3 -c "import toml; toml.load('config.toml')"
   ```

2. **Reset to defaults**:
   ```bash
   # Backup current config
   cp ~/.config/qmk-notifier/config.toml ~/.config/qmk-notifier/config.toml.bak
   
   # Create new default config
   qmkonnect -c
   ```

3. **Check file permissions**:
   ```bash
   ls -la ~/.config/qmk-notifier/config.toml
   chmod 644 ~/.config/qmk-notifier/config.toml
   ```

### Wrong Keyboard IDs

**Find correct IDs**:

#### Using QMK Configuration
```c
// Check your QMK config.h
#define VENDOR_ID    0xFEED
#define PRODUCT_ID   0x0000
```

#### Using System Tools
```bash
# Linux
lsusb | grep -i keyboard
cat /sys/class/hidraw/hidraw*/device/uevent | grep -E "HID_ID|HID_NAME"

# macOS
system_profiler SPUSBDataType | grep -A 10 -B 10 -i keyboard

# Windows PowerShell
Get-WmiObject -Class Win32_USBHub | Where-Object {$_.Name -like "*keyboard*"}
```

## Performance Issues

### High CPU Usage

**Diagnosis**:
```bash
# Monitor CPU usage
top -p $(pgrep qmkonnect)

# Check polling interval
qmkonnect -v  # Look for timing information
```

**Solutions**:
1. **Increase polling interval**:
   ```toml
   [window_detection]
   poll_interval = 200  # Increase from default 100ms
   ```

2. **Use application filtering**:
   ```toml
   [window_detection]
   include_apps = ["code", "firefox", "terminal"]  # Only monitor specific apps
   ```

3. **Check for infinite loops**:
   ```bash
   qmkonnect --debug  # Look for repeated events
   ```

### Memory Leaks

**Monitor memory usage**:
```bash
# Linux
ps aux | grep qmkonnect
valgrind --leak-check=full qmkonnect

# macOS
leaks qmkonnect

# Windows
# Use Task Manager or Process Explorer
```

## Communication Issues

### Data Not Reaching Keyboard

**Symptoms**: Window detection works, but keyboard layers don't change

**Debug QMKonnect → Keyboard Communication**:

1. **Verify data is being sent** (Linux):
   ```bash
   # Show what data is being sent to keyboard
   qmkonnect --debug | grep -i "sending\|data"
   
   # Test connection specifically
   qmkonnect --test-connection
   ```

2. **Check QMK firmware side**:
   
   Add debug output to your QMK keymap:
   ```c
   #ifdef CONSOLE_ENABLE
   void qmk_notifier_notify(const char* app_class, const char* window_title) {
       printf("Received: app='%s', title='%s'\n", app_class, window_title);
       // Your layer switching logic here
   }
   #endif
   ```
   
   Then monitor QMK console:
   ```bash
   qmk console
   ```

3. **Verify Raw HID setup**:
   
   Ensure your QMK firmware has:
   ```make
   # In rules.mk
   RAW_ENABLE = yes
   ```
   
   ```c
   // In config.h  
   #define RAW_USAGE_PAGE 0xFF60
   #define RAW_USAGE_ID   0x61
   ```

### Raw HID Issues

**Verify Raw HID setup**:
1. **QMK firmware**:
   ```make
   # In rules.mk
   RAW_ENABLE = yes
   ```

2. **Test Raw HID**:
   ```bash
   # Linux - test hidraw device
   echo "test" > /dev/hidraw0
   
   # Check if device accepts data
   qmkonnect --test-connection
   ```

## Getting Help

### Collecting Debug Information

When reporting issues, include:

1. **System information**:
   ```bash
   # Linux
   uname -a
   lsb_release -a
   
   # macOS
   sw_vers
   
   # Windows
   systeminfo | findstr /B /C:"OS Name" /C:"OS Version"
   ```

2. **QMKonnect version**:
   ```bash
   qmkonnect --version
   ```

3. **Debug output**:
   ```bash
   qmkonnect --debug > debug.log 2>&1
   ```

4. **Configuration file**:
   ```bash
   cat ~/.config/qmk-notifier/config.toml
   ```

### Where to Get Help

- **GitHub Issues**: [https://github.com/dabstractor/qmkonnect/issues](https://github.com/dabstractor/qmkonnect/issues)
- **QMK Discord**: #help channel
- **Documentation**: This site and README files

### Creating Bug Reports

Include:
- Operating system and version
- QMKonnect version
- Steps to reproduce
- Expected vs actual behavior
- Debug logs
- Configuration file (remove sensitive data)

---

## Next Steps

- [Check example setups]({{ site.baseurl }}/examples)
- [Contribute to the project](https://github.com/dabstractor/qmkonnect)
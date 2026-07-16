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
   - Windows: grant the screen-recording permission when prompted (for window titles); HID access needs no special permissions

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

2. **Check QMK firmware** — qmk-notifier must be integrated (it is **required**, not optional):
   ```make
   # In your keymap's rules.mk — this enables Raw HID AND compiles notifier.c:
   include keyboards/handwired/[manufacturer]/[keyboard]/qmk-notifier/rules.mk
   ```
   You do **not** need to set `RAW_USAGE_PAGE` / `RAW_USAGE_ID` — QMK's defaults
   (`0xFF60` / `0x61`) are exactly what qmk-notifier expects. See the
   [QMK Integration Guide]({{ site.baseurl }}/qmk-integration).

3. **Permission issues (Linux)**:
   Default QMK keyboards need no manual permissions setup — the shipped static
   rule grants access to any device exposing the QMK Raw HID signature. If you
   installed from source (not the Arch package), make sure the rule and helper
   are installed:
   ```bash
   sudo install -m755 target/release/qmkonnect-hid-id /usr/lib/udev/qmkonnect-hid-id
   sudo install -m644 packaging/linux/udev/69-qmkonnect-rawhid.rules \
                       /usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules
   sudo udevadm control --reload && sudo udevadm trigger
   # Still no access? Ensure your user is in the `input` group:
   sudo usermod -aG input $USER   # then log out and back in
   ```
   If you set a custom `vendor_id`/`product_id`, also generate the matching
   rule: `sudo qmkonnect -r`.

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

3. Check the autostart entry (HKCU `Run` key):
   - `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\QMKonnect`

#### Installation or Launch Blocked

**Solutions**:
1. SmartScreen "Unknown publisher" → click **More info → Run anyway** (the app is unsigned)
2. Check Windows Defender exclusions / verify antivirus isn't quarantining it
3. The app needs **no Administrator** rights — it's a per-user install

### Linux Issues

#### Broken udev rule corrupts device permissions (VMs / containers fail)

> **Symptoms:** libvirt/QEMU VMs fail to start with errors like
> `Failed to open /dev/null for OFD lock probing: Permission denied` or
> `Could not access KVM kernel module`; `ls -l /dev/null /dev/kvm /dev/fuse`
> shows them as `root:input` mode `0660` instead of `root:root 0666`.

An older QMKonnect build wrote `/etc/udev/rules.d/99-qmkonnect.rules` in a
multi-line form with **no backslash line-continuations**. Because udev treats
every newline as the end of a rule (a trailing comma does **not** continue a
line), the bare-assignment lines matched **every device on the host** and
re-permissioned them to `root:input 0660`. The current build both renders a
correct single-line rule **and** auto-repairs a dangerous legacy rule on
`qmkonnect --reload` / `-r`, so the supported fix is simply:

```bash
sudo qmkonnect -r          # overwrites/purges the bad rule, then reloads udev
```

If `qmkonnect -r` is unavailable, or to recover manually:

```bash
sudo rm /etc/udev/rules.d/99-qmkonnect.rules   # remove the bad rule
sudo udevadm control --reload-rules
sudo udevadm trigger                           # re-apply correct device defaults
# /dev/null, /dev/kvm, /dev/fuse, ... revert to their proper owner/mode.
# Reboot if any node still looks wrong.
```

Then reinstall QMKonnect via the supported path — the static
`packaging/linux/udev/69-qmkonnect-rawhid.rules` (correct: single line, guarded
by `ENV{ID_QMKONNECT}=="1"`) — so default keyboards need no config-driven rule
at all. See `installation.md`.

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

#### Window titles not updating (app name only)

QMKonnect reads window **titles** with `CGWindowListCopyWindowInfo`, which requires **Screen Recording** permission (not Accessibility). Without it, the app still runs and sends the frontmost **app name**, but titles come back empty.

**Grant Screen Recording:**
1. System Settings → Privacy & Security → Screen Recording
2. Enable QMKonnect
3. **Quit & reopen** QMKonnect so the change takes effect

**Why it keeps re-prompting after every rebuild:** local builds are ad-hoc signed, so the app's signature (`cdhash`) changes on each build. macOS keys the Screen-Recording grant to that signature and re-prompts every rebuild — *even though System Settings still shows QMKonnect as granted*. Reset the stale grant to get one clean prompt:

```bash
tccutil reset ScreenCapture io.mulletware.qmkonnect
```

To stop the loop for good, build with a stable identity:

```bash
CODESIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" ./build.sh
```

#### Launching an OLD version after rebuilding

**Symptoms:** you run `build.sh`, but the app that opens is missing your changes or is clearly an older build.

**Cause:** this is a *launch* problem, not a build problem — `build.sh` always compiles current source. macOS LaunchServices remembers every copy of `QMKonnect.app` it has ever seen (old `/Applications` installs, trashed copies, apps left inside a mounted `.dmg`) and can launch one of those **stale** copies instead of your fresh build.

**Fix — clean the old copies before rebuilding:**

```bash
pkill -f "QMKonnect.app"
ls /Volumes | grep -i qmkonnect | while read -r v; do hdiutil detach "/Volumes/$v"; done
rm -rf /Applications/QMKonnect.app ~/.Trash/QMKonnect.app
tccutil reset ScreenCapture io.mulletware.qmkonnect
```

Then rebuild, install, and launch:

```bash
cd packaging/macos && ./build.sh && cp -R QMKonnect.app /Applications/ && cd ../..
open /Applications/QMKonnect.app
```

To see what macOS still has registered (when stale copies linger):

```bash
LSR=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
"$LSR" -dump | grep -i 'path:.*QMKonnect\.app'   # list every registered copy
"$LSR" -u /path/to/stale/QMKonnect.app           # unregister one stale copy
```

See the [installation guide]({{ site.baseurl }}/installation/#macos) for the full build → install → launch sequence.

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

2. **Check QMK firmware side** — make sure qmk-notifier is actually integrated:
   - Your `rules.mk` includes `.../qmk-notifier/rules.mk` (this compiles
     `notifier.c` and enables Raw HID).
   - Your `keymap.c` includes `"qmk-notifier/notifier.h"` and forwards
     `raw_hid_receive()` to `hid_notify()`.
   - See the [QMK Integration Guide]({{ site.baseurl }}/qmk-integration).
   
   To watch what the keyboard receives, add your own `printf` inside a callback
   (there is no built-in `qmk_notifier_notify` callback — the firmware API is the
   `DEFINE_SERIAL_LAYERS` / `DEFINE_SERIAL_COMMANDS` macros):
   ```c
   #ifdef CONSOLE_ENABLE
   void disable_vim(void) {
       printf("qmk-notifier: disable_vim fired\n");
       // ...your real logic...
   }
   #endif
   ```
   
   Then monitor QMK console:
   ```bash
   qmk console
   ```

3. **Verify Raw HID setup** — the qmk-notifier module's `rules.mk` enables this
   for you (`RAW_ENABLE = yes`), so just make sure that module is included:
   ```make
   include keyboards/handwired/[manufacturer]/[keyboard]/qmk-notifier/rules.mk
   ```
   (`RAW_USAGE_PAGE` / `RAW_USAGE_ID` are QMK defaults of `0xFF60` / `0x61` — no
   need to set them unless your firmware deliberately overrides them.)

### Raw HID Issues

**Verify Raw HID setup**:
1. **QMK firmware** — make sure qmk-notifier is integrated (its `rules.mk` sets `RAW_ENABLE = yes` for you):
   ```make
   include keyboards/handwired/[manufacturer]/[keyboard]/qmk-notifier/rules.mk
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
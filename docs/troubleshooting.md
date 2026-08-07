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

> **Read the tray/menu-bar status first — it's three-state.** **● Device
> Connected** means a qmk_notifier-capable board is present. **⚠ QMK board
> found — no qmk_notifier module (flash it)** means a QMK board is attached
> but isn't running qmk_notifier — **flash qmk_notifier** (see
> [QMK Integration]({{ site.baseurl }}/qmk-integration)); this is the most
> common cause of "detected but nothing happens." **○ No Device
> Connected** means no QMK Raw-HID board was found at all.

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
   - Use the **Settings → discovered-device picker** to confirm which board
     QMKonnect sees (each row shows ✓ qmk_notifier-capable or ✗ QMK board,
     no module); set `vendor_id`/`product_id` manually only to disambiguate
     among multiple boards.
   - Ensure Raw HID is enabled in QMK firmware
   - Confirm qmk_notifier module is included

2. **Check QMK firmware** — qmk_notifier must be integrated (it is **required**, not optional):
   ```make
   # In your keymap's rules.mk — this enables Raw HID AND compiles notifier.c:
   include keyboards/handwired/[manufacturer]/[keyboard]/qmk_notifier/rules.mk
   ```
   You do **not** need to set `RAW_USAGE_PAGE` / `RAW_USAGE_ID` — QMK's defaults
   (`0xFF60` / `0x61`) are exactly what qmk_notifier expects. See the
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

#### Linux

QMKonnect auto-selects a window backend at startup. If focus changes don't
switch layers, first see **which backend it chose** and why:

```bash
qmkonnect -v 2>&1 | grep -i "backend\|probing\|selected"
# Expect: 'select_linux_backend: probing …' lines + '… selected' for the winner.
```

Then match your desktop:

- **foreign-toplevel** (KDE Plasma 6, COSMIC, Hyprland, Sway, Niri, wlroots):
  no setup needed; if empty, your compositor may not advertise
  `wlr-foreign-toplevel` — confirm with `qmkonnect -v`.
- **gnome** → needs the `qmkonnect@mulletware` Shell extension (see
  *Linux (GNOME)* below and [Installation → GNOME]({{ site.baseurl }}/installation/)).
- **hyprland** (legacy IPC): verify the socket (commands below).
- **atspi** (best-effort fallback): see *AT-SPI (a11y) backend* under
  [Linux Issues](#linux-issues) — names may be inconsistent; install the GNOME
  extension for reliable detection.
- **x11** (XFCE/MATE/Cinnamon/Budgie/LXQt): needs `xprop`; verify with
  `which xprop`.

**Hyprland (IPC backend) socket diagnostic:**

```bash
# Check Hyprland integration
echo $HYPRLAND_INSTANCE_SIGNATURE

# Test Hyprland socket manually
socat -u UNIX-CONNECT:/tmp/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket2.sock - | head -10

# Should show window events when you switch applications
```

#### Linux (GNOME)

GNOME (Mutter) exposes no client API for the active window, so QMKonnect
detects windows via the **`qmkonnect@mulletware`** Shell extension (see
`docs/installation.md` → GNOME). If focus changes don't switch layers:

1. **Check the backend + extension name in verbose output**:
   ```bash
   qmkonnect -v 2>&1 | grep -i gnome
   # Expect: "gnome: 'io.mulletware.QMKonnect' is owned (extension installed+enabled)"
   #         "→ 'gnome' available, selected"
   # On focus change: "[<ms>] gnome: <app_class> | <title>"
   ```
   If you instead see the GNOME probe `Err` naming the extension, the daemon
   also fires a one-shot first-run notification pointing you here.

2. **Confirm the extension is enabled**:
   ```bash
   gnome-extensions show qmkonnect@mulletware   # State: ENABLED
   ```
   If it shows `DISABLED` or is absent, enable it in the **Extensions** app
   (or `gnome-extensions enable qmkonnect@mulletware`). On a **Wayland**
   session, **log out and back in** the first time you install it so
   `gnome-shell` loads the extension.

3. **Verify the D-Bus name is owned + the method works** (proves the extension
   side, independent of the daemon):
   ```bash
   gdbus call --session --dest io.mulletware.QMKonnect \
     --object-path /io/mulletware/QMKonnect \
     --method io.mulletware.QMKonnect.WindowMonitor.GetActiveWindow
   # → ('<app_class>', '<title>')
   ```

4. **Watch the signal live** while switching focus (should print on every
   change):
   ```bash
   gdbus monitor --session --dest io.mulletware.QMKonnect
   ```

The daemon's GNOME backend auto-selects when the extension's D-Bus name is
owned and re-acquires state if you toggle the extension off and back on (within
~1 s). See `spec/PLATFORMS.md` §8 for the authoritative spec.

> Seeing wrong or empty app names? If `qmkonnect -v` shows the **atspi**
> backend (not **gnome**), the extension isn't installed/enabled and you're on
> the best-effort fallback — see *GNOME shows wrong / inconsistent window
> names* under [Linux Issues](#linux-issues).

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

#### No tray icon on GNOME

Stock GNOME dropped SNI/AppIndicator support, so QMKonnect's tray item is
invisible on a default GNOME session (the daemon still runs fine — device
status, rules, and settings all work). Two options:

1. **Install the *AppIndicator and KStatusNotifierItem Support* extension**
   ([extensions.gnome.org](https://extensions.gnome.org/extension/615/appindicator-support/),
   or your distro's `gnome-shell-extension-appindicator` package). Enable it
   in the **Extensions** app, then **log out and back in** on Wayland.
2. **Run trayless** — device status, rules, and settings all work from the CLI
   (`--list-devices`, `--validate-rules`, `--list-callbacks`); only the
   click-menu is unavailable.

This is **independent** of the window-detection Shell extension — see *Linux
(GNOME)* under [Window Detection](#window-detection-not-working) for that.
See `spec/LINUX.md` §7.4.

#### GNOME shows wrong / inconsistent window names

If `qmkonnect -v` shows the **atspi** backend (not **gnome**) on GNOME, the
Shell extension isn't installed/enabled and you're on the best-effort
fallback → see [*AT-SPI (a11y) backend — best-effort*](#at-spi-a11y-backend-best-effort-requires-enabling-accessibility)
below. Install + enable the `qmkonnect@mulletware` extension ([Installation →
GNOME]({{ site.baseurl }}/installation/)) and restart QMKonnect; it will
auto-select the **gnome** backend and report reliable `WM_CLASS`-based names.

#### AT-SPI (a11y) backend — best-effort + requires enabling accessibility

The **AT-SPI** backend (selected as priority #4, only when the
foreign-toplevel / GNOME / Hyprland backends are all unavailable) is a
**fallback of last resort** for window-focus detection. It tracks the
*focused accessible object* on the desktop's accessibility (a11y) bus and is
the path used most often by GNOME users who installed QMKonnect **without**
the GNOME Shell extension but who have a screen reader or accessibility tool
running. See `spec/PLATFORMS.md` §9 for the full contract and limitations.

**It is off by default.** The a11y bus daemon being up is not enough — apps
only **expose** accessibility when the desktop has Assistive Technology
enabled. To turn it on:

```bash
# GNOME (and most GTK desktops):
gsettings set org.gnome.desktop.interface toolkit-accessibility true
#   or: Settings → Accessibility → enable a screen reader briefly to bring
#   the bus up, then leave it on.
```

The presence check is: `org.a11y.Bus` is owned on the session bus, **or**
`$ATSPI_BUS_ADDRESS` is set (exported by `at-spi-bus-launcher`). Verify with:

```bash
dbus-send --session --dest=org.freedesktop.DBus --print-reply \
          /org/freedesktop/DBus org.freedesktop.DBus.NameHasOwner \
          string:org.a11y.Bus   # → true when the a11y bus is up
```

**Known limitations (it's best-effort, not a primary backend):**

- `app_class` is the focused object's *application readable Name* — **not**
  `WM_CLASS`. For Electron / Chromium / sandboxed apps this may show as
  `chrome`, `python3`, or be empty, so layer rules keyed on the real window
  class will not match reliably.
- `title` is the focused accessible object's name (e.g. a text field or tab),
  **not** the window toplevel title, so it can differ from the titlebar.
- Applications without an accessibility bridge are **invisible** to this
  backend (no focus events are emitted for them).

For reliable GNOME support, **prefer the GNOME Shell extension** (PLATFORMS.md
§8 / `docs/qmk-integration.md`): it reports the real `WM_CLASS` and window
title directly from `global.display.focus_window`. The AT-SPI backend is
intentionally a best-effort safety net, not a replacement.

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
   cp ~/.config/qmkonnect/config.toml ~/.config/qmkonnect/config.toml.bak
   
   # Create new default config
   qmkonnect -c
   ```

3. **Check file permissions**:
   ```bash
   ls -la ~/.config/qmkonnect/config.toml
   chmod 644 ~/.config/qmkonnect/config.toml
   ```

### Wrong Keyboard IDs

> You rarely need to find IDs by hand — the **Settings → discovered-device
> picker** lists connected boards and writes the IDs for you, and
> `qmkonnect --list-devices` prints every board's VID:PID and qmk_notifier
> capability (`kind` column). The methods below are only for the rare
> manual case.

**Find correct IDs**:

#### Using QMK Configuration
```c
// Check your QMK config.h — your board's USB IDs live here:
#define VENDOR_ID    0x????   // your board's USB vendor ID
#define PRODUCT_ID   0x????   // your board's USB product ID
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

## Host Rules Issues

Host-side rules (`rules.toml`) have a few distinct failure modes. If your board's
firmware rules work but host rules don't, check these first. See the
[Configuration Guide]({{ site.baseurl }}/configuration) for the schema and CLI flags.

### Legacy firmware (`proto_ver != 2`) — host rules disabled

**Symptoms**: `rules.toml` is present and valid, but host rules have no effect;
the keyboard still switches layers via its firmware rules.

**Cause**: host rules require firmware that advertises the typed-command
capability (`proto_ver == 2`). With legacy firmware (or while the keyboard is
disconnected) QMKonnect silently falls back to string-only mode and never sends
host commands — your board's `DEFINE_*` rules keep working unchanged.

**Diagnose**:
```bash
qmkonnect --list-callbacks
# Legacy firmware prints exactly:
#   Legacy firmware (no callback support) — host rules will run in string-only mode.
# (exit 0 — this is expected, not an error)
```

**Fix**: flash firmware that defines `DEFINE_HOST_CALLBACKS` (which advertises
`proto_ver == 2`). See the [QMK Integration Guide]({{ site.baseurl }}/qmk-integration).
Until then, nothing is broken — your firmware rules work as before.

### Callback name not found in registry

**Symptoms**: a host callback doesn't fire, or `--validate-rules` warns about an
unknown callback name.

**Cause**: `rules.toml` references a callback **name** that isn't in your
`DEFINE_HOST_CALLBACKS` registry — a typo, a name you haven't registered yet, or
firmware that hasn't been reflashed with the new registry.

**Diagnose**:
```bash
qmkonnect --validate-rules        # unknown names print:  ⚠  unknown callback: {name}
                                  # (a WARNING — exit 0, NOT fatal)
qmkonnect --list-callbacks        # prints the keyboard's real callback name -> id table
```

**Fix**: correct the name in `rules.toml` to match a name printed by
`--list-callbacks`, **or** add it to `DEFINE_HOST_CALLBACKS` and reflash. Names
are case-sensitive strings. See the
[Configuration Guide]({{ site.baseurl }}/configuration) (`--validate-rules` /
`--list-callbacks`) and the
[QMK Integration Guide]({{ site.baseurl }}/qmk-integration) (`DEFINE_HOST_CALLBACKS`).

### `rules.toml` parse error

**Symptoms**: `--validate-rules` exits **non-zero** and prints `rules.toml
invalid: …`; host rules are disabled (string-only fallback) until the file parses.

**Cause**: malformed TOML, a missing required key, or a bad `match` form.

**Diagnose**:
```bash
qmkonnect --validate-rules                  # prints:  rules.toml invalid: {error}  (exit non-zero)
qmkonnect --validate-rules --rules-path ~/rules.draft.toml   # validate a draft elsewhere
```

**Fix**: every `[[rule]]` entry **requires** `match` and at least one of `layer`
/ `enable` / `disable` (an entry setting none of those is an error); `match` is
either a bare string (`"steam_app*"`, class-only) or a **2-element** array
(`["*chrome*", "*youtube*"]` — class and title; 1- or 3-element arrays are
errors); `layer` is **optional** — a **raw QMK layer index** (no reserved range)
when set, and must then be `<` your `layer_state` width (≤31 with
`LAYER_STATE_32BIT`) and `!= 255` (the wire "clear layer" sentinel, which would
silently *clear* the host layer and is rejected). To win in **stack** mode it
must be above your highest board layer; in **replace** mode any valid index
wins. See the
[Configuration Guide]({{ site.baseurl }}/configuration) for the full field table.

At runtime, when `rules.toml` fails to parse during a window focus change,
QMKonnect shows a **one-time desktop notification** (the app dedupes — at most
one per broken state) and then falls back to string-only mode. On **Windows**
this is a **toast** that auto-dismisses after a few seconds and lands in Action
Center (it is no longer a modal dialog you must click away); Linux uses
`notify-send` and macOS uses a Notification Center alert. (On Windows the toast
requires the installed Start Menu shortcut to render — if you launched a dev
build directly the notification may be silent, but the `--validate-rules` error
above is always printed.)

### Device shows connected but rules not applying

**Symptoms**: the keyboard is connected and firmware rules work, but your
`rules.toml` rules have no effect.

**Checklist**:
1. **`rules.toml` present and valid?** `qmkonnect --validate-rules` (a missing
   file prints `No rules.toml found (host rules disabled). Nothing to validate.`
   and exits 0 — host rules are simply off; create it with `qmkonnect -c` or
   hand-write it next to `config.toml`).
2. **Firmware capable?** `qmkonnect --list-callbacks` — the "Legacy firmware …"
   line means host rules are off (see *Legacy firmware* above).
3. **Pattern matches the real window class?** The matcher is class-only for a
   bare `match` string. Check what QMKonnect actually sees:
   ```bash
   qmkonnect -v | grep -i "window\|sending"     # the class\x1Dtitle string sent
   ```
   (or use the tray's "Show Window Information"). That value is exactly what your
   pattern is matched against, so trust it over a native tool — on Hyprland
   QMKonnect uses the window's `initial_class` (which can differ from the `class`
   field `hyprctl` prints), and on X11 it uses the **class** (the second field of
   `xprop WM_CLASS`, not the instance). A `*chrome*` rule won't match a
   class reported as `Google Chrome` — adjust the pattern or use a `[class, title]`
   array.
4. **Edit took effect?** `rules.toml` **is** re-parsed on every window focus
   change — switch windows and back. (Or open it via the tray's **Edit rules**
   item.) If the file failed to parse, a desktop notification fired and host
   rules fell back to string-only; run `qmkonnect --validate-rules` to see the error.
5. **Callback name correct?** `qmkonnect --list-callbacks` (see *Callback name
   not found* above).
6. **Layer index valid?** `layer` is a raw QMK layer index (no reserved range):
   it must be `<` your `layer_state` width (≤31 with `LAYER_STATE_32BIT`) and
   `!= 255` (the wire "clear layer" sentinel). To win in **stack** mode it must
   be above your highest board layer; in **replace** mode any valid index wins.

See the [Configuration Guide]({{ site.baseurl }}/configuration) for the schema and
CLI flags, and the [Examples]({{ site.baseurl }}/examples) for a complete recipe.

### Board rule not firing after enabling host rules (partial migration)

**Symptoms**: after enabling host rules (capable board + a `rules.toml`), a
window that is matched **only** by a rule still defined on the board (in the
firmware's `DEFINE_SERIAL_LAYERS` / `DEFINE_SERIAL_COMMANDS`) stops triggering
that board rule.

**Why**: when host rules are enabled and a window matches **no** host rule,
QMKonnect sends only an `APPLY_HOST_CONTEXT { layer: 0xFF, callbacks: [] }` and
**no** legacy window string (HOST_RULES.md §8(4)). Because no string is sent,
the board never re-evaluates its own `DEFINE_*` rules for that window, so the
previously-active board layer/command persists.

**Fix**: this is the documented behavior — host rules and board rules are not
meant to run side by side for the same window. Finish the migration by moving
that rule into `rules.toml` (the [migration guide](../qmk-integration) tells
you to *remove* rules from `DEFINE_*` once they live on the host), then reload
rules.

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

2. **Check QMK firmware side** — make sure qmk_notifier is actually integrated:
   - Your `rules.mk` includes `.../qmk_notifier/rules.mk` (this compiles
     `notifier.c` and enables Raw HID).
   - Your `keymap.c` includes `"qmk_notifier/notifier.h"` and forwards
     `raw_hid_receive()` to `hid_notify()`.
   - See the [QMK Integration Guide]({{ site.baseurl }}/qmk-integration).
   
   To watch what the keyboard receives, add your own `printf` inside a callback
   (there is no built-in `qmk_notifier_notify` callback — the firmware API is the
   `DEFINE_SERIAL_LAYERS` / `DEFINE_SERIAL_COMMANDS` macros):
   ```c
   #ifdef CONSOLE_ENABLE
   void disable_vim(void) {
       printf("qmk_notifier: disable_vim fired\n");
       // ...your real logic...
   }
   #endif
   ```
   
   Then monitor QMK console:
   ```bash
   qmk console
   ```

3. **Verify Raw HID setup** — the qmk_notifier module's `rules.mk` enables this
   for you (`RAW_ENABLE = yes`), so just make sure that module is included:
   ```make
   include keyboards/handwired/[manufacturer]/[keyboard]/qmk_notifier/rules.mk
   ```
   (`RAW_USAGE_PAGE` / `RAW_USAGE_ID` are QMK defaults of `0xFF60` / `0x61` — no
   need to set them unless your firmware deliberately overrides them.)

### Raw HID Issues

**Verify Raw HID setup**:
1. **QMK firmware** — make sure qmk_notifier is integrated (its `rules.mk` sets `RAW_ENABLE = yes` for you):
   ```make
   include keyboards/handwired/[manufacturer]/[keyboard]/qmk_notifier/rules.mk
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
   cat ~/.config/qmkonnect/config.toml
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
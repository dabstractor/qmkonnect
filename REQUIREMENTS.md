# QMKonnect - Technical Requirements

> **Status: Beta.** QMKonnect is cross-platform and usable today, but some
> platform surfaces are deliberately narrow (notably **Hyprland-only on Linux**)
> and the distributed binaries are **unsigned / ad-hoc signed**. See
> [Beta Status & Future Work](#beta-status--future-work) below.

This document outlines the technical requirements and specifications for the
QMKonnect application — the desktop side of a two-part system. QMKonnect detects
the active window and sends it to your keyboard; the keyboard needs the companion
[**qmk-notifier**](https://github.com/dabstractor/qmk-notifier) firmware module
to react (see the README → *QMK Firmware Setup* and
[docs/qmk-integration.md](docs/qmk-integration.md)).

## Core Requirements

### Cross-Platform Support

- **Windows** 10/11 (64-bit): tray app, runs per-user in the interactive session
- **Linux**: Hyprland only (Wayland); systemd user service + SNI tray
- **macOS**: native menu-bar app bundle

### Window Monitoring

- Real-time detection of foreground-window focus changes
- Capture of application class and window title information
- Filtering of internal/system windows
- Debouncing of rapid window-change bursts (configurable, default 50 ms)

### QMK Integration

- Format window information as `{application_class}{GS}{window_title}` where
  `{GS}` is the Group Separator character (`0x1D`)
- Send the formatted payload to QMK keyboards over Raw HID
- **Auto-discovery** of a single standard QMK keyboard via the QMK Raw HID
  signature (usage page `0xFF60` / usage `0x61`) — no IDs required
- Optional `vendor_id` / `product_id` to disambiguate among multiple QMK
  keyboards, and optional `usage_page` / `usage` for firmware that overrides the
  defaults

### Configuration Management

- User-configurable settings via a TOML configuration file:
  - Linux: `~/.config/qmk-notifier/config.toml`
  - Windows: `%APPDATA%\QMKonnect\config.toml`
  - macOS: `~/Library/Application Support/QMKonnect/config.toml`
  - (Historical Linux path is preserved so existing installs keep working.)
- GUI settings dialog on Windows / macOS; file + CLI on Linux
- Reload configuration without restarting the application (`qmkonnect -r`)

## Windows-Specific Requirements

### Application Model

- **Tray application, not a service**: per-user interactive app with a
  system-tray icon (built on `tray-icon`/`muda`). It must run in the user's own
  session to render the tray icon.
- **Silent Operation**: no console window (`windows_subsystem = "windows"`).
- **Error Logging**: logs to the Windows Event Log (source `"QMKonnect"`); when
  launched from a terminal with `-v`, logs print there instead.

### Singleton Pattern

- **Single Instance**: only one instance can run at a time
- **Detection Method**: named mutex via the `single-instance` crate
- **Graceful Exit**: a second instance detects the first and exits cleanly

### System Tray Integration

- **Tray Icon**: visible icon in the system tray
- **Context Menu**: right-click menu (Settings, Open at Login, Quit, …)
- **Icon Loading**: icon loaded from the install location

### Automatic Startup

- **Startup Method**: HKCU `Run` registry key (`src/autostart.rs`)
- **Default Behavior**: **Open at Login enabled by default**, toggleable from the
  tray (also manageable via Task Manager → Startup)

### Installer

- **Installer Type**: per-user **Inno Setup** installer (`QMKonnect-Setup.exe`)
- **Installation Location**: `%LOCALAPPDATA%\Programs\QMKonnect` (no admin /
  elevation required)
- **Runtime Dependencies**: none — the release binary statically links the C
  runtime (`.cargo/config.toml` ⇒ `+crt-static`), so no Visual C++ Redistributable
  is needed
- **Startup Entry**: enables Open at Login (HKCU `Run`) during install
- **Upgrade Handling**: stop the running app (named-mutex singleton) before
  reinstalling; uninstall cleanly via Add/Remove Programs

### Threading Model

- **Main Thread**: window event hooks must run on the main thread
- **Message Loop**: maintain a proper Windows message loop for event handling
- **Thread Safety**: ensure thread-safe access to shared resources

### Permissions

- No special permissions are required for HID access or foreground-window
  detection.

## Linux-Specific Requirements (Hyprland Only)

> Only Hyprland is supported. Other Wayland compositors and X11 are **not**
> supported in this beta.

### Window Compositor

- **Hyprland**: subscribe to the active-window IPC socket for focus changes
- Optional periodic active-window polling (`poll_interval_ms`, default off)

### Service Integration

- **Systemd Service**: run as a user service (`systemctl --user`)
- **Udev Rules**: a static rule (`69-qmkonnect-rawhid.rules`) + the
  `qmkonnect-hid-id` helper tag any device carrying the QMK Raw HID signature as
  `ID_QMKONNECT=1` and grant access; optionally start/stop the service on
  keyboard hotplug. Per-keyboard rules can be generated with `qmkonnect -r`.

### Tray Integration

- **StatusNotifierItem (SNI)** tray over D-Bus via `ksni`; any SNI-hosting bar
  (Waybar, SwayNC, KDE, GNOME +AppIndicator, …) renders the icon and menu
- **Settings / Notifications**: Settings dialog via `zenity`, notifications via
  `notify-send` (standard on Linux desktops)

### Configuration

- **XDG Compliance**: follow XDG Base Directory Specification
- **Udev Rules**: generate a per-keyboard rule from config (`qmkonnect -r`)

## macOS-Specific Requirements

### Application Bundle

- **App Bundle**: package as a proper `.app` bundle
- **Menu Bar**: menu-bar icon with settings/quit menu
- **Resources**: include necessary resources in the bundle

### Permissions

- **Screen Recording** (not Accessibility) is required to read window **titles**
  via `CGWindowListCopyWindowInfo`. Without it the app still runs and sends the
  frontmost **app name**, but titles come back empty.

### Automatic Startup

- **Startup Method**: `SMAppService` login item (macOS 13 Ventura+), enabled by
  default on first launch; manageable from the menu-bar icon → **Launch at
  Login** or System Settings → General → Login Items

## Performance Requirements

- **Resource Usage**: minimal CPU and memory footprint
- **Startup Time**: fast startup and initialization
- **Responsiveness**: immediate detection of window changes (debounced)

## Error Handling

- **Graceful Degradation**: handle errors without crashing
- **Logging**: platform-appropriate logging (Event Log on Windows, journalctl on
  Linux, stderr/`-v` everywhere)
- **User Feedback**: feedback through appropriate channels (tray/menu-bar icon,
  logs)

## Security Requirements

- **Permissions**: request only necessary permissions
- **Data Handling**: no collection or transmission of sensitive data — window
  metadata is sent only to the locally connected keyboard
- **Installation**: per-user install; **no elevation required**

## Compatibility

- **Windows**: 10 and 11, 64-bit (32-bit and 8.1/8/7 and earlier not supported)
- **Linux**: distributions running **Hyprland** (Wayland)
- **macOS**: recent versions (macOS 13 Ventura+ for SMAppService login items)
- **Toolchain**: Rust 1.88+ (enforced by `rust-version` in `Cargo.toml`)

## Beta Status & Future Work

This is a beta. Current limitations and intended future work:

- **Linux**: only Hyprland is supported; broader Wayland-compositor and X11
  support is planned.
- **Signing / Notarization**: distributed binaries are unsigned (Windows) /
  ad-hoc signed (macOS, not notarized). Releases should be signed with a stable
  Developer ID and notarized — among other things this stops the macOS
  Screen-Recording re-prompt loop on each rebuild.
- **Settings UX**: a richer cross-platform settings UI (today: tray dialog on
  Windows/macOS, `zenity` on Linux, plus the TOML file).
- **Multiple keyboards**: VID/PID disambiguation is supported; richer
  multi-keyboard management is future work.

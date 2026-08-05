# System Context — QMKonnect Bugfix PRD

## Project Overview
QMKonnect is a cross-platform (macOS/Windows/Linux) tray app that monitors the
foreground window and sends `{app_class}{GS}{title}` to a connected QMK keyboard
over HID. Rules in `rules.toml` can map window patterns to firmware callbacks.

## Build & Test Conventions
- **Language**: Rust (Cargo workspace, single binary `qmkonnect`).
- **Test command**: `cargo test --bin qmkonnect -- --test-threads=1`
  (single-threaded due to shared debouncer state).
- **Platform gating**: Platform code lives under `src/platforms/` and is
  `#[cfg(target_os = "...")]` gated. Tests for Windows-specific code only run on
  Windows; Hyprland/X11 tests run on Linux.
- **Feature flags**: `default = ["hyprland", "macos", "linux-tray"]`.
  Hyprland support is the `hyprland` feature (crate `hyprland = "0.4.0-beta.2"`).
- **No CHANGELOG.md** exists; version bumps are in `Cargo.toml` and
  `packaging/windows/inno/QMKonnect.iss` (`#define MyAppVersion`).

## Key Files & Their Roles

| File | Lines | Role |
|------|-------|------|
| `src/platforms/hyprland.rs` | 664 | Hyprland (Wayland) window monitor. Uses `hyprland` crate IPC. |
| `src/platforms/x11.rs` | 184 | X11 window monitor. Shells out to `xprop` for WM_CLASS/_NET_WM_NAME. |
| `src/platforms/windows.rs` | 529 | Windows window monitor. Win32 event hooks + polling fallback. |
| `src/autostart.rs` | 110 | Windows HKCU `Run` key autostart (per-user, `#[cfg(windows)]`). |
| `src/tray.rs` | 3093 | Windows + macOS tray/menu-bar, settings dialogs. |
| `src/linux_tray.rs` | 1315 | Linux SNI tray (zenity-based settings dialog). |
| `src/core/notifier.rs` | 4267 | HID communication, handshake lifecycle, capability detection. |

## Architecture: Notification Flow
1. Platform monitor detects foreground window change.
2. Constructs `WindowInfo { app_class, title }`.
3. Calls `notifier::notify_qmk(&window_info, verbose)`.
4. `notify_qmk` sends `{app_class}{0x1D}{title}` to the QMK board over HID.
5. Rules (`rules.toml`) can override the class→callback mapping.

## Architecture: Handshake Lifecycle (relevant to Bug ID 3)
- `PresenceTracker::tick()` returns `Gain`/`Loss` based on whether the set of
  capable QMK boards changed (plug/unplug events).
- **Gain** → `notifier::perform_handshake(verbose)`: sends `QUERY_INFO`,
  discovers capability + sweeps `QUERY_CALLBACK(i)` to build a `name → id` map
  stored in the global `CALLBACK_NAMES` static.
- **Loss** → `notifier::reset_handshake_state()`: clears `HOST_CAPABLE`,
  `BOARD_HAS_RULES`, `CALLBACK_NAMES` map, and `HAS_HANDSHAKED` guard.
- `perform_handshake` is **idempotent per board boot** (`HAS_HANDSHAKED` guard).
- **Gap (Bug ID 3)**: When the user changes VID/PID in Settings (both boards
  remain plugged in), no plug/unplug event fires, so no Gain/Loss. The stale
  name map from board A persists for board B. Fix: call
  `reset_handshake_state()` + `perform_handshake()` in the save path.

## Architecture: Settings Save Flow (per platform)
- **Windows** (`src/tray.rs` ~L979): Dialog result merged onto `current_config`,
  serialized via `render_config_body`, atomic-written to config path.
- **macOS** (`src/tray.rs` ~L1877): Same pattern, `chosen`/`manual` VID/PID merge.
- **Linux** (`src/linux_tray.rs` `save_and_notify` L718): Calls `write_config()`
  then `apply_device_rule()` (pkexec udev rule).
- **None** of these paths currently reset/re-handshake after a VID/PID change.
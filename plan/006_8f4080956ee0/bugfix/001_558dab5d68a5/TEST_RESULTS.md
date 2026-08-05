# Bug Fix Requirements

## Overview
Four real defects identified, ranging from incorrect data display (Hyprland/X11) to platform-specific configuration and path-handling bugs (multi-board handshake, Windows autostart). The highest-priority items are the Hyprland/X11 identifier mismatches (major) and the unquoted Windows autostart path (major).


## Critical Issues (Must Fix)
Issues that prevent core functionality from working.

None.


## Major Issues (Should Fix)
Issues that significantly impact user experience or functionality.

### Issue 1: Hyprland "Show Window Information" reports `class`, but the keyboard receives `initial_class`
**Severity**: Major
**ID**: 1
**Location**: src/platforms/hyprland.rs:571,577

**Description**:
The dialog in `src/platforms/hyprland.rs` uses `class` to display window info, while the notification paths use `initial_class`. The PRD states `initial_class` is the stable identifier, so the dialog shows a value that can differ from what the firmware matches, causing rules to silently fail if a user copies the wrong value.

**Steps to Reproduce**:
1. Open Settings on Hyprland. 2. Observe window information. 3. Compare `class` from dialog vs `initial_class` sent in notifications for apps where they differ (e.g., some Electron apps).

### Issue 2: X11 `WM_CLASS` parser returns the **instance**, not the **class** (off-by-one)
**Severity**: Major
**ID**: 2
**Location**: src/platforms/x11.rs:68-74

**Description**:
The parser in `src/platforms/x11.rs` incorrectly returns the instance string instead of the class string due to a bug in filtering the `xprop` output. A leading space shifts indices, causing `.get(1)` to return the instance.

**Steps to Reproduce**:
1. Run on X11. 2. Check `xprop -root | grep _NET_ACTIVE_WINDOW` followed by `xprop -id <window_id> | grep WM_CLASS`. 3. Observe the parser returns the instance (e.g., 'firefox') instead of the class (e.g., 'Firefox').

### Issue 3: HKCU `Run` autostart value is written unquoted on Windows
**Severity**: Major
**ID**: 4
**Location**: src/autostart.rs:103, packaging/windows/inno/QMKonnect.iss:103

**Description**:
Both the app (`src/autostart.rs:103`) and the installer write the autostart path without quotes. If the user's path contains spaces (e.g., `C:\Users\John Doe`), the Run key may fail to execute or become a security vulnerability.

**Steps to Reproduce**:
1. Install app on Windows with a username containing a space. 2. Enable autostart. 3. Check Registry `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` for the unquoted path.


## Minor Issues (Nice to Fix)
Small improvements or polish items.

### Issue 1: Settings-dialog VID/PID change does not reset the handshake (multi-capable-board case)
**Severity**: Minor
**ID**: 3
**Location**: src/tray.rs:979/981, src/tray.rs:1877/1878, src/linux_tray.rs (save_and_notify)

**Description**:
When multiple capable boards are present, changing the VID/PID filter in Settings does not reset the handshake state. This causes the app to use the callback name map of the old board (A) for the newly selected board (B), leading to incorrect IDs or dropped commands.

**Steps to Reproduce**:
1. Plug in two capable boards (A and B). 2. Start app; handshake with A. 3. Open Settings, select board B, save. 4. Observe no re-handshake occurs and rules for B use A's name map.

### Issue 2: Windows `should_ignore_window` title-length heuristic uses byte length
**Severity**: Minor
**ID**: 5
**Location**: src/platforms/windows.rs

**Description**:
The check `window_info.title.len() < 2` counts bytes, not characters. A 1-char emoji (4 bytes) is kept, while a 1-char ASCII (1 byte) is ignored. This inconsistent heuristic is not in the PRD and may drop valid windows.

**Steps to Reproduce**:
1. Run on Windows. 2. Open a window with a 1-char ASCII title. 3. Observe it is ignored. 4. Open a window with a 1-char emoji title. 5. Observe it is processed.

## Testing Summary
- Total bugs found: 5
- Critical: 0
- Major: 3
- Minor: 2

## Recommendations
- Fix Finding 1 (Hyprland) and Finding 2 (X11) immediately as they are clear one-liners affecting rule correctness.
- Quote the autostart path in both `autostart.rs` and the Inno installer script to prevent login failures for paths with spaces (Finding 4).
- Investigate and fix the Windows title-length heuristic to use character count or remove it if it's not spec-required (Finding 5).
- Add `reset_handshake_state()` to the settings save path for future per-board support (Finding 3).

# Tray Surfaces — Three-Platform Status Rendering & Settings Dialogs

## No Shared Abstraction
`src/tray.rs` (macOS/Windows via `tray-icon` + `tao`) and `src/linux_tray.rs`
(Linux SNI via `ksni`) are **completely independent** — no shared trait, no shared
module. Each duplicates the status glyph logic, poll loop, `parse_id` hex parser,
and settings dialog. Changes require parity in both.

## Platform Mutual Exclusion
- `tray.rs` is gated `#![cfg(not(all(target_os = "linux", feature = "hyprland")))]`
- `linux_tray.rs` is gated `#[cfg(all(target_os = "linux", feature = "linux-tray"))]`

---

## macOS/Windows Tray (src/tray.rs)

### UserEvent Enum — line 38-48
```rust
enum UserEvent {
    MenuEvent(MenuEvent),
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    DeviceStatus(bool),    // ← line 41: THE EVENT TO CHANGE (bool → DeviceStatus enum)
    #[cfg(target_os = "macos")]
    AutostartSync,
}
```

### Status Text — `device_status_text` — line 660-669
```rust
fn device_status_text(connected: bool) -> String {
    if connected {
        "\u{25CF}  Device Connected".to_string()     // ● BLACK CIRCLE
    } else {
        "\u{25CB}  No Device Connected".to_string()   // ○ WHITE CIRCLE
    }
}
```
**Change target:** rewrite as `fn device_status_text(DeviceStatus) -> String` with
three branches (● Connected / ⚠ No module / ○ Disconnected).

### Status Poll Thread — line 380-406
- **Interval: 3 seconds** (hardcoded)
- Seeds `last` with `startup_device_was_connected()` to avoid spurious first-tick handshake
- On transition (`last != Some(connected)`): runs `handshake_action` → Gain/Loss/None
- Sends `UserEvent::DeviceStatus(connected)` **only on transition**
- **Change target:** call `device_status()` instead of `is_device_connected()` for
  the event payload, but keep `is_device_connected()` for the handshake lifecycle gating.

### Event Loop Arm — line 507-510
```rust
Event::UserEvent(UserEvent::DeviceStatus(connected)) => {
    device_status_i.set_text(device_status_text(connected));
}
```
Only consumer of `DeviceStatus`. Calls `MenuItem::set_text`.

### First-Paint — line 312-319
```rust
let device_status_i = MenuItem::new(
    device_status_text(crate::core::notifier::is_device_connected()),
    false,  // disabled = non-clickable label
    None,
);
```
**Change target:** call `device_status()` instead of `is_device_connected()`.

### Settings Dialog (Windows) — `show_settings_dialog` — line 753-868
- Control IDs: 1001 (VID EDIT), 1002 (PID EDIT), 1003 (OK), 1004 (Cancel)
- `DIALOG_RESULT: Mutex<Option<(Option<u16>, Option<u16>)>>` — line 51
- WndProc at line 1031-1085 handles WM_COMMAND for 1003/1004
- On OK: reads fields → `parse_id_field` → overlay onto Config → `render_config_body` → `atomic_write`
- **Change target:** add LISTBOX above VID/PID, relocate fields under "Advanced" group box, extend DIALOG_RESULT.

### Settings Dialog (macOS) — `show_macos_settings_dialog` — line 1162
- `show_settings_dialog_with_pool` — line 1186: NSAlert + accessory NSView with 2 NSTextFields
- OK = return value 1000, Cancel = 1001
- **Change target:** add NSStackView of device rows in accessory view, "Advanced" toggle.

---

## Linux SNI Tray (src/linux_tray.rs)

### QmkTray Struct — line 66-72
```rust
pub struct QmkTray {
    device_connected: bool,   // ← line 68: THE FIELD TO CHANGE (bool → DeviceStatus)
    dark_mode: bool,
}
```

### Status Line — line 137-161 (inside `fn menu()`)
```rust
let status = if self.device_connected {
    "\u{25CF}  Device Connected"       // ● BLACK CIRCLE
} else {
    "\u{25CB}  No Device Connected"    // ○ WHITE CIRCLE
};
```
Inlined literal — **no function** (unlike tray.rs's `device_status_text`). Parity
test at line 948 (`status_text_uses_parity_glyphs`).

### Icon Dim — line 156-169 (inside `fn icon_pixmap()`)
- Connected → full alpha
- Disconnected → `dim_icon()` scales alpha to ~35% (DIM_ALPHA = 90)
- **Change target:** NoModule needs full-alpha icon (it's present, just not capable).

### Poll Thread — line 259-301
- **Interval: 1 second** (DEVICE_POLL_INTERVAL, vs macOS/Windows 3s)
- Also tracks dark_mode (polls every 10 ticks)
- On transition: `handshake_action` → Gain/Loss/None
- Updates via `poll_handle.update(|t: &mut QmkTray| { t.device_connected = ...; })`
- **Change target:** track previous DeviceStatus, fire one-shot `notify-send` on Disconnected→NoModule.

### Settings Dialog (Linux) — `show_settings_dialog_linux` — line 635-723
- Uses `zenity --forms` with `--add-entry` for VID and PID
- Save: `write_config(vid, pid)` → `render_config_body` → `atomic_write`
- Linux-only: `apply_device_rule(vid, pid)` via `pkexec` for udev
- **Change target:** add `zenity --list` for discovered devices before the `--forms`.

### Notify Pattern — `linux_tray::notify(summary, body)` — line 846-859
```rust
fn notify(summary: &str, body: &str) {
    match Command::new("notify-send")
        .args(["--app-name=QMKonnect", "--icon=input-keyboard", summary, body])
        .status()
    { ... }
}
```
**Note:** `notify-rust` is deliberately avoided (nested tokio runtime panics in
ksni's handler thread, per spec §7.3).

### parse_id — line 830-844
Separate copy of `parse_id_field` from tray.rs:67. Same semantics, slightly different
error string.

---

## Key Parity Requirements

1. **Status glyph text must match** between `tray.rs::device_status_text` and the
   inlined literal in `linux_tray.rs` menu(). The test at `linux_tray.rs:948` enforces this.
2. **parse_id / parse_id_field** duplicate hex validation logic.
3. **Poll intervals differ** (3s macOS/Windows vs 1s Linux) — intentional, no shared constant.
4. **macOS/Windows:** only text changes (no icon dim, no tooltip change).
5. **Linux:** text + icon alpha + tooltip all reflect state.
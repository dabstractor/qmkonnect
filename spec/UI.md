# SPEC — Tray / Menu-Bar UI, Dialogs & Autostart

> Companion to `PRD.md` / `SPEC_ARCHITECTURE.md`. The full user-facing surface:
> tray/menu-bar icon + menu, the Settings dialogs, the "Show Window
> Information" dialogs, the live device-status indicator, and the per-platform
> "Open at Login" autostart. Covers `src/tray.rs` (macOS/Windows),
> `src/linux_tray.rs` (Linux SNI + GTK), and `src/autostart.rs` (Windows).

---

## 1. The Tray Surface (per platform)

| Platform | Crate stack | Where the icon shows | Menu model |
|---|---|---|---|
| **macOS** | `tray-icon` + `tao` + `objc` | Menu bar | muda `Menu` (native Cocoa) |
| **Windows** | `tray-icon` + `tao` + `windows` | System tray | muda `Menu` (native Win32) |
| **Linux** | `ksni` (SNI over `zbus`) | Any SNI-hosting bar (Waybar, SwayNC, KDE, GNOME+AppIndicator) | D-Bus `com.canonical.dbusmenu` serialized tree |

`src/tray.rs` is compiled for `cfg(not(all(target_os="linux", feature="hyprland")))`
— i.e. macOS, Windows, and the non-Hyprland Linux build. The Hyprland/Linux
build uses `src/linux_tray.rs` (feature `linux-tray`, default-on).

### 1.1 Menu layout (macOS / Windows, identical item set modulo labels)

```
About QMKonnect                              ← PredefinedMenuItem::about
●  Device Connected   /  ○ No Device Connected   ← disabled MenuItem (line 2)
─────────────                                ← separator
[Launch at Login  /  Open at Login]          ← CheckMenuItem (macOS / Windows)
Settings                                     ← MenuItem
Edit rules…                                  ← seed rules.toml if absent, then open in system editor (xdg-open / open / start)
─────────────                                ← separator
Show Window Information...                   ← MenuItem (macOS/Windows only)
─────────────                                ← separator
Quit                                         ← MenuItem
```

- **Line 2 (device status):** a **disabled** `MenuItem` whose text is refreshed
  by a background thread (§4). Solid dot `U+25CF` = connected, hollow `U+25CB` =
  absent. Synchronous probe at first paint so the initial state is correct.
- **Autostart toggle:** `CheckMenuItem`. macOS = "Launch at Login"
  (SMAppService); Windows = "Open at Login" (HKCU `Run`). Initial checkmark
  reflects real system state.
- **Show Window Information:** macOS/Windows only (Linux exposes windows via
  `hyprctl` natively, plus the SNI menu surfaces it through a GTK popup).

### 1.2 The Linux SNI menu (`src/linux_tray.rs`)

```
●  Device Connected   /  ○ No Device Connected   ← disabled StandardItem (line 1)
(hidden structural toggle)                   ← visible:false, forces LayoutUpdated redraw
─────────────
Settings…                                    ← zenity --forms (writes config.toml)
Edit rules                                   ← seed rules.toml if absent, then xdg-open
─────────────
Show Window Information                      ← notify-send / native GTK popup
─────────────
Quit                                         ← process::exit(0)
```

The hidden structural item is deliberate: changing the *count* of visible items
forces ksni to emit `LayoutUpdated` (the signal every SNI host honors to redraw
an *open* popup), whereas `ItemsPropertiesUpdated` is ignored by some hosts
(e.g. Quickshell) for open menus.

### 1.3 Icon handling
- **macOS:** monochrome **template** asset `IconTemplate.png` loaded from the
  bundle's `Resources/`; `with_icon_as_template(true)` so macOS tints it to the
  bar. Falls back to a generated 16×16 white square.
- **Windows:** `IconTray-dark.png` beside the exe (installer drops it), zoomed
  ~20% (clamped to headroom) so the glyph renders larger in the fixed tray slot.
- **Linux:** two embedded variants — `IconTray-dark.png` (light outline, for
  dark bars) and `IconTray-light.png` (dark outline, for light bars) — selected
  by querying the `org.freedesktop.appearance.color-scheme` portal (1=dark,
  2=light, 0=no pref→dark). The icon is **dimmed to ~35% alpha** when the device
  is absent (disconnect visible in realtime; `NewIcon` is honored by hosts).

### 1.4 The `EventLoopProxy` pattern (macOS/Windows)
`muda::MenuItem` is `!Send` (`Rc<RefCell<…>>`). Background threads (device-status
poll, deferred autostart register) deliver state to the main thread via
`tao::EventLoopProxy<UserEvent>`:

```rust
enum UserEvent {
    MenuEvent(MenuEvent),
    DeviceStatus(bool),   // macOS/Windows
    AutostartSync,        // macOS — re-sync checkbox after deferred register
}
```

The event-loop arm mutates menu items (the only safe place).

---

## 2. Settings Dialogs

All three write `config.toml` via the **shared** `core::render_config_body`
so the file format is identical everywhere. Config is hot, so a save takes
effect within ~3 s (no restart).

### 2.0 The discovered-device picker (new primary surface)

The primary surface is no longer two raw VID/PID hex fields — it is a **live,
self-populating list of discovered devices** built from `classify_devices()`
(`DEVICE_DISCOVERY.md` §2/§5). The devices name themselves via their HID
descriptors; **there is no curated keyboard database.** Each row shows:

```
✓  Dactyl-Manuform (5x7-1)        0xFEED:0x0000   ← qmk_notifier
✗  Keychron Q1                     0x3434:0x0123   ← QMK board, no module
```

- **One capable board, no VID/PID set** (common case): a read-only
  `Detected: <name>` line; no picker shown. Auto-discovery is already correct.
- **Multiple Tier-1 boards:** the picker appears; selecting a row writes that
  board's VID/PID via `render_config_body` (the disambiguation).
- **`[ Rescan ]`** invalidates the classification cache and re-runs
  `classify_devices` (use after flashing a board with the dialog open).

The legacy VID/PID hex fields move under an **"Advanced / manual override"**
disclosure (§2.1–§2.3) for the rare case of targeting a board not currently on
the bus. Empty/`"auto"` ⇒ `None` ⇒ auto-discovery. Per-platform widget choices
are in `DEVICE_DISCOVERY.md` §5.3.

### 2.1 Windows — native Win32 dialog (`show_settings_dialog`)
- A registered `QMKSettingsDialog` window class, `WS_OVERLAPPED|WS_CAPTION|
  WS_SYSMENU|WS_VISIBLE`, 400×200, centered, `COLOR_3DFACE` background, app icon.
- Controls (ids): `1001` Vendor ID `EDIT`, `1002` Product ID `EDIT`, `1003` OK
  `BUTTON`, `1004` Cancel `BUTTON`; static labels. Fields pre-filled with the
  current 4-digit hex (empty if `None`).
- `settings_dialog_proc`: on OK, `GetDlgItemTextW` both fields,
  `parse_id_field` each (empty/`auto` ⇒ `None`), store `(Option<u16>,
  Option<u16>)` in the shared `DIALOG_RESULT: Mutex`, `DestroyWindow`. On parse
  error, `MessageBoxW`. Modal loop via `GetMessageW`.
- Result written to `config_path` by `render_config_body`. No success dialog
  (the connection is rebuilt per notification).

### 2.2 macOS — `NSAlert` + accessory view (`show_macos_settings_dialog`)
- Wraps in an `NSAutoreleasePool` (background `LSUIElement` apps lack a main
  pool). `NSAlert` with message text showing current `format_id_hex` values
  ("auto" when `None`); OK/Cancel buttons; accessory `NSView` with two
  `NSTextField`s pre-filled.
- On OK (response `1000`), read both fields, `parse_id_field`, write via
  `render_config_body`. Errors via `NSAlert` (critical).

### 2.3 Linux — `zenity --forms` (`show_settings_dialog_linux`)
- `zenity --forms --title=QMK Settings --add-entry="Vendor ID (hex)"
  --add-entry="Product ID (hex)"`, text shows current values. `--ok-label=Copy`
  is *not* used here (that's the window-info dialog).
- Parse the `|`-separated stdout; `parse_id` each (empty/`auto` ⇒ `None`).
- On save: `write_config` then `apply_device_rule(vid,pid)`:
  - Both `None` ⇒ no rule needed (static usage-page rule covers it); best-effort
    `udevadm control --reload-rules` + `udevadm trigger`.
  - At least one `Some` ⇒ render the VID/PID rule, stage under `std::env::temp_dir()`
    (not a predictable `/tmp` name), install via **`pkexec`**
    (`install -m644 …/99-qmkonnect.rules && udevadm … && rm`). If pkexec is
    unavailable/cancelled, surface "Run: `sudo qmkonnect -r`" (which is now
    root-aware — `SPEC_LINUX.md` §4).
- Notifications via `notify-send` (`--app-name=QMKonnect --icon=input-keyboard`) —
  also fires an automatic **"rules.toml invalid"** notification when `rules.toml`
  fails to parse (host rules fall back to string-only — never silent). macOS uses
  `NSUserNotification`/`UNUserNotificationCenter`; Windows a toast — same trigger.

### 2.4 `parse_id_field` / `parse_id` (shared logic)
- Trim; empty **or** literal `"auto"`/`"AUTO"` ⇒ `Ok(None)`.
- Strip optional `0x`/`0X` prefix; `u16::from_str_radix(_, 16)` ⇒ `Ok(Some(v))`.
- Anything else ⇒ `Err`.

---

## 3. "Show Window Information" Dialogs

Purpose: let users discover the **exact** `class`/`title` strings to put in
their `DEFINE_SERIAL_LAYERS` / `DEFINE_SERIAL_COMMANDS`. Each row shows
`<class>  —  <title>` (class only if title empty) and a per-row **Copy** that
copies `"class|title"` (or just `class` when title is empty) — the config-style
form.

### 3.1 Windows — native Win32 (`show_window_info_dialog`)
- Registered `QMKWindowInfoDialog` class; `WS_OVERLAPPED|WS_CAPTION|WS_SYSMENU|
  WS_THICKFRAME|WS_MINIMIZEBOX|WS_MAXIMIZEBOX|WS_VSCROLL`; resizable (min
  480×320, default 760×520); white background; Segoe UI font.
- Layout: fixed bold **header** ("Class (what QMKonnect reports) — Window
  title"), scrollable **rows** (one read-only selectable `EDIT` label + a
  **Copy** `BUTTON` per row, 26 px row height), fixed **footer** tip.
- Scroll: `WM_VSCROLL`/`WM_MOUSEWHEEL` (3 rows/wheel notch)/thumb; an
  `AtomicI32 WININFO_SCROLL_POS` shared with the WndProc; `wininfo_relayout`
  repositions in-view rows and hides off-screen ones.
- Copy: control ids `WI_IDC_COPY_BASE = 6000` + row index; on `WM_COMMAND` with
  `id >= 6000`, look up the row in `WINDOW_INFO_ROWS` and
  `copy_to_clipboard_windows(hwnd, text)` (`CF_UNICODETEXT`, `GlobalAlloc`
  `GMEM_MOVEABLE`, ownership transfers to OS on success — do not `GlobalFree`).
- Debug CLI: `qmkonnect --show-window-info` opens it directly (no tray).

### 3.2 macOS — `NSWindow` + `NSScrollView` (`show_macos_window_info_dialog`)
- `NSAutoreleasePool`; `[NSApp activateIgnoringOtherApps:YES]` (background apps
  must activate or windows can't become key).
- `NSWindow` (`alloc` → `initWithContentRect:styleMask:backing:defer:`),
  `setReleasedWhenClosed:NO`, `center`, titled+closable+miniaturizable.
- `NSScrollView` with an `NSView` document holding one row per app (origin is
  bottom-left, so rows are top-aligned by counting down). Each row: an
  `NSTextField` label (selectable, truncating-tail) + an `NSButton` with an SF
  Symbol (`doc.on.doc`, macOS 11+; `respondsToSelector:`-guarded, else "Copy"
  text). Button `tag = row index`; target `RustWindowInfoCopyTarget` →
  `wi_copy_row:` → `copy_to_pasteboard_macos`.
- Modal via `[NSApp runModalForWindow:]`; the `RustWindowInfoWindowDelegate`
  calls `[NSApp stopModal]` on `windowWillClose:`.

### 3.3 Linux — native GTK popup, zenity fallback (`show_window_info_linux`)
- **Native GTK popup** (preferred): a single owner thread runs `gtk::main()`
  for the process lifetime; requests arrive over an `mpsc` channel polled from
  the main loop. Each request opens a `GtkWindow` (`WindowType::Toplevel`,
  **dialog type-hint** + `set_resizable(false)` + fixed default size → floats on
  every tiling compositor), with a `ScrolledWindow`+`ListBox` (`vexpand`) of
  rows, each a `Label` (end-ellipsized) + a **Copy** `Button` →
  `Clipboard::set_text("class|title")`. 640×760 default.
  - Why not zenity: zenity `--forms` floats but caps the list at ~3–4 rows;
    zenity `--list` is tall but tiles. No single zenity invocation is both.
- **zenity fallback** (`show_window_info_linux_zenity`): `--forms --add-list`
  with `--ok-label=Copy`; height-capped (~3–4 rows) — a hard zenity limitation.
  Select a row → Copy → copies `class|title`. Clipboard via `wl-copy` then
  `xclip`. A `notify-send` notification confirms the copy or reports clipboard
  unavailability.
- Runs on a dedicated thread so ksni's IPC thread stays responsive.

### 3.4 Shared row store
`WINDOW_INFO_ROWS: Mutex<Vec<(String,String)>>` (`tray.rs`, macOS/Windows) —
both the copy-button target and the Win32 WndProc look up the row to copy **by
index**. Only one dialog open at a time, so a single shared slot suffices.

---

## 4. Device-Connection Status Indicator

The tray status line is a **three-state** value derived from `classify_devices()`
(`DEVICE_DISCOVERY.md` §3), refreshed **only on a transition**:

| State | Text | Icon |
|---|---|---|
| **Connected** | `●  Device Connected` (or `●  N Devices Connected`) | solid `U+25CF`, full alpha |
| **No module** | `⚠  QMK board found — no qmk_notifier module (flash it)` | warning glyph |
| **Disconnected** | `○  No Device Connected` | hollow `U+25CB`, ~35% alpha (Linux) |

The "No module" state is the point of the Tier-2 capability probe: a pure-VIA
board (no qmk_notifier firmware) no longer shows a false-green "Connected".

The frequent **Tier-1 presence** poll stays a read-only enumeration
(`is_device_connected()`, pure enumerate, **never opens the device**) on a
background thread. The **Tier-2 classification** that resolves the three states
*does* open each candidate once (shared, non-seize — §R-COEX) on a device
**appearance**, then is TTL-cached, so the hot poll never opens the device.

---

## 5. Tray Lifecycle in the Runners

- **Windows:** `run_tray_app()` → singleton guard → start monitor →
  `tray::setup_tray()` (blocks until Quit).
- **macOS:** monitor on a background thread → `tray::setup_tray()` on main
  (blocks until Quit).
- **Linux/Hyprland:** `linux_tray::spawn()` (ksni, own thread; handle kept
  alive) → `monitor.start()` blocks on the IPC listener.
- **Linux/X11:** monitor on a background thread → if `linux-tray`, park main;
  else `tray::setup_tray()`.

`Quit` (any platform) → `tray_icon.take()` + `ControlFlow::Exit` (macOS/Windows)
or `process::exit(0)` (Linux).

---

## 6. Autostart ("Open at Login" / "Launch at Login")

Default **on** on first run on every platform, with an obvious in-app toggle.
Never fights the user afterwards.

### 6.1 Windows — HKCU `Run` key (`src/autostart.rs`)
- **Single source of truth:** value name `"QMKonnect"` under
  `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`, type
  `REG_SZ`, data = `current_exe()` path. Shared with the installer
  (`install.ps1` / `QMKonnect.iss` / `uninstall.ps1`) — same name everywhere.
- `is_enabled()`: `RegGetValueW(HKCU, SUBKEY, VALUE, RRF_RT_REG_SZ, …)`; presence-based
  (a Task-Manager "Disabled" override under `StartupApproved\Run` is intentionally
  *not* consulted — most apps behave this way).
- `set_enabled(true)`: `RegOpenKeyExW(KEY_SET_VALUE)` → `RegSetValueExW(REG_SZ,
  UTF-16 incl. NUL)` → `RegCloseKey`. `set_enabled(false)`:
  `RegDeleteValueW`.
- Path **self-heals**: written from `current_exe()` at toggle time.
- Tray handler `handle_open_at_login_click`: muda flips the native check on
  click *before* dispatching, so `is_checked()` is already the new desired
  state; persist it, then `set_checked(is_enabled())` to revert visibly on
  failure.

### 6.2 macOS — `SMAppService` (`tray.rs` `mod autostart`)
- Links `ServiceManagement.framework`. `SMAppService.mainApp()` `register`/
  `unregister` (macOS 13+). Status raw values: 0=notRegistered, 1=enabled,
  2=requiresApproval, 3=notFound.
- **First-run default-on** (`autostart_first_run_default_on`): deferred onto the
  main run loop via `dispatch::Queue::main().exec_async` (registration's XPC
  round-trip never blocks the Init callback); gated by a marker file
  `~/Library/Application Support/QMKonnect/.autostart_initialized` so it never
  re-enables after the user turns it off. Signals `UserEvent::AutostartSync` to
  re-sync the checkbox.
- Tray handler `handle_launch_at_login_click`: derives desired state from the
  **real** `is_enabled()` (robust to muda's auto-toggle), performs register/
  unregister, mirrors outcome into the checkmark.

### 6.3 Linux — systemd user service (`SPEC_LINUX.md` §6)
- The static udev rule sets `ENV{SYSTEMD_USER_WANTS}+="qmkonnect.service"` and
  the service is `BindsTo=dev-qmkonnect_device.device` (the symlink the rule
  creates) → the service **starts when the keyboard appears, stops when it
  disappears**. `systemctl --global enable` (from `post_install`) makes it
  start at login.

---

## 7. Activation Policy & Dock Icon (macOS)
- `Info.plist` sets `LSUIElement = true` (launch-time: no Dock icon, no
  CMD-Tab).
- **But** `tao`'s runtime default promotes to Regular in
  `applicationDidFinishLaunching`, overriding `LSUIElement`. So `setup_tray()`
  sets `EventLoopExtMacOS::set_activation_policy(ActivationPolicy::Accessory)`
  **before** `run()` — the only place tao honors it. Accessory apps can still
  surface windows (Settings, Window Info) transiently.

---

*Continue with `SPEC_LINUX.md`.*

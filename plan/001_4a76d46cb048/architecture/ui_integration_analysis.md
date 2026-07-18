# QMKonnect UI / Integration / Packaging — Deep-Dive Analysis

Scope: the cross-platform tray UIs, the platform autostart implementations, the
Linux udev/systemd/hid-id integration, and the macOS/Windows packaging scripts.
All file paths are repo-relative. Severity tags: **[blocker]**, **[major]**,
**[minor]**, **[nit]**, **[info]**.

---

## 1. `src/tray.rs` — macOS / Windows tray (tao + tray-icon + muda)

Read fully (lines 1–2338).

### 1.1 Module gating
- Whole file is gated `#![cfg(not(all(target_os = "linux", feature = "hyprland")))]`
  (line 1). Linux+Hyprland uses `src/linux_tray.rs` instead. There is no
  generic-Linux fallback tray: non-Hyprland Linux builds that are *not* built
  with `feature = "hyprland"` would still compile this file but tao/tray-icon on
  plain X11 is not exercised. **[info]**

### 1.2 UserEvent enum + EventLoopProxy pattern
- `UserEvent` (lines 30–43):
  - `MenuEvent(MenuEvent)` — wraps the muda menu event.
  - `DeviceStatus(bool)` — `cfg(macos|windows)`. Posted from a background poll
    thread; the loop updates the status menu item text.
  - `AutostartSync` — `cfg(macos)`. Posted after the deferred first-run
    SMAppService register so the checkbox re-syncs.
- EventLoop built with `EventLoopBuilder::<UserEvent>::with_user_event().build()`
  (line ~270).
- `MenuEvent::set_event_handler` (lines ~289–295) clones the proxy and forwards
  every `MenuEvent` to the loop as `UserEvent::MenuEvent`. This is the bridge
  from muda's global event channel into tao's per-loop dispatch.
- A background thread (lines ~340–356) spawns for `cfg(macos|windows)`:
  - Polls `crate::core::notifier::is_device_connected()` every 3 s.
  - Posts `DeviceStatus(connected)` **only on a transition** (`last != Some(connected)`),
    avoiding needless D-Bus/event churn.
- `proxy.clone()` is passed into `autostart_first_run_default_on` (macOS) so the
  deferred register can post `AutostartSync`.

### 1.3 Tray menu structure (macOS)
Order (lines ~245–340):
1. **About** (PredefinedMenuItem::about, metadata name "QMKonnect",
   copyright "Copyright Mulletware 2026") — **[nit]** copyright year is a
   hardcoded literal "2026".
2. `sep_about`
3. **Device status** (disabled `MenuItem`, text from `device_status_text`,
   initially probed synchronously).
4. `sep_about` reuse — the same separator object is reused for both the about
   separator and (later) it is *not* reused; see the menu assembly: actually
   `sep_about` is pushed once before the launch toggle, and `sep_wininfo` +
   `sep_before_quit` are separate. Re-check: `sep_about` is created once and
   pushed once. OK.
5. **Launch at Login** (`CheckMenuItem`, macOS only). Placeholder checkmark;
   synced to real SMAppService status in the `Init` handler, then re-synced on
   `AutostartSync`.
6. **Settings** (`MenuItem`).
7. `sep_wininfo`
8. **Show Window Information...** (`MenuItem`).
9. `sep_before_quit`
10. **Quit** (`MenuItem`).

### 1.4 Tray menu structure (Windows)
Order:
1. About
2. sep_about
3. Device status (disabled)
4. Settings
5. **Open at Login** (`CheckMenuItem`, Windows only). Initial checkmark reflects
   `crate::autostart::is_enabled()` directly (real registry state) so first
   paint is correct without an Init sync.
6. sep_wininfo
7. Show Window Information...
8. sep_before_quit
9. Quit

### 1.5 Event handling in the run loop (lines ~358–470)
- `Event::NewEvents(StartCause::Init)`:
  - Builds the tray icon. macOS loads `IconTemplate.png` from the bundle
    (`load_template_icon_from_bundle` → `bundle_resource`), template flag
    `true`. Non-macOS loads `IconTray-dark.png` via `load_windows_tray_icon`
    (with the 20%-zoom scaler), template flag `false`. Falls back to a 16×16
    white default otherwise. **[minor]** on Linux the Windows function returns
    `None` (cfg stub), so non-macOS-non-Windows (i.e. plain Linux using this
    file) would get the white square — but that path is dead because Linux
    builds use `linux_tray.rs`.
  - macOS: `CFRunLoopWakeUp` is called manually so the icon actually shows.
  - macOS: `launch_at_login_i.set_checked(autostart::is_enabled())`, then
    `autostart_first_run_default_on(proxy.clone())`.
- `UserEvent::MenuEvent`:
  - `quit_i` → `tray_icon.take()` then `ControlFlow::Exit`.
  - `settings_i` → `handle_settings_click()`.
  - `window_info_i` (mac/win) → `handle_window_info_click()`.
  - `launch_at_login_i` (mac) → `handle_launch_at_login_click(&launch_at_login_i)`.
  - `open_at_login_i` (win) → `handle_open_at_login_click(&open_at_login_i)`.
- `UserEvent::DeviceStatus(connected)` (mac/win) → `device_status_i.set_text(device_status_text(connected))`.
- `UserEvent::AutostartSync` (mac) → re-sync the checkbox.

### 1.6 Device-status indicator (macOS/Windows)
- `device_status_text(connected)` (lines ~770): solid `U+25CF BLACK CIRCLE` +
  "Device Connected", hollow `U+25CB WHITE CIRCLE` + "No Device Connected".
- The disabled `MenuItem` is line 2 of the menu. Refreshed via the 3 s poll
  thread. This is read-only enumeration (`is_device_connected` never opens the
  device), so it can't disturb the keyboard.

### 1.7 Settings dialog — Windows (`show_settings_dialog`, lines ~600–710)
- Win32 hand-rolled dialog. Registers `WNDCLASSW "QMKSettingsDialog"`, creates a
  400×200 `WS_OVERLAPPED|WS_CAPTION|WS_SYSMENU|WS_VISIBLE` window centered on
  screen. `hbrBackground = COLOR_3DFACE+1` (cast to `HBRUSH((15+1) as isize)`).
- Controls (created in `create_dialog_controls`, lines ~715–840): Vendor ID
  STATIC+EDIT (id 1001), Product ID STATIC+EDIT (id 1002), OK button (1003),
  Cancel button (1004). Edit fields pre-filled with current 4-digit hex (lower
  case) or empty when `None` (auto).
- Window icon: `load_app_icon` tries `Icon.ico` beside exe, then
  `packaging/Icon.ico`, else the system `IDI_INFORMATION`. **[minor]** the
  dialog also calls `LoadIconW(IDI_INFORMATION)` again right after create and
  sets it via `WM_SETICON` — somewhat redundant with the class icon set in
  `load_app_icon`.
- `settings_dialog_proc` (lines ~845–905): `WM_COMMAND` id 1003 (OK) reads both
  edits via `GetDlgItemTextW`, parses via `parse_id_field`, on success stores
  `(Option<u16>, Option<u16>)` into the shared `DIALOG_RESULT` Mutex and
  destroys the window; id 1004 destroys without storing. `WM_DESTROY` →
  `PostQuitMessage(0)`. A modal `GetMessageW` loop runs until WM_QUIT.
- On success, `crate::core::render_config_body(vendor_id, product_id)` is
  written to `config.toml`. Comment notes no restart needed (connection is
  re-established per notification).

### 1.8 Settings dialog — macOS (`show_macos_settings_dialog`, lines ~990–1090)
- Builds an `NSAlert` with `setMessageText "QMK Settings"` and an informative
  text listing current `vendor_id`/`product_id` as 4-digit hex (or "auto" via
  `format_id_hex`). Two `NSTextField`s (vendor at y=0, product at y=30) in an
  `NSView` accessory (200×60). OK/Cancel buttons added (OK returns 1000,
  Cancel 1001).
- `runModal`; on 1000 reads field stringValues, parses, writes config. Errors
  via `show_macos_error_message`.
- **[info]** Runs inside an explicit `NSAutoreleasePool` because LSUIElement
  background apps lack a main pool. Good.
- **[nit]** Field layout uses hardcoded NSRect literals with a local
  `objc_types::{NSPoint,NSSize,NSRect}` C repr — works but is brittle.

### 1.9 Shared state / statics (lines ~46–66)
- `DIALOG_RESULT: Mutex<Option<(Option<u16>, Option<u16>)>>` (Windows) —
  replaces a former `Arc::into_raw + mem::forget` leak tracked as #9.
- `WINDOW_INFO_ROWS: Mutex<Vec<(String,String)>>` (mac/win) — single shared
  slot, assumes only one window-info dialog open at a time. The copy-button
  target objects and the Windows WndProc look up rows by index here.

### 1.10 "Show Window Information..." — Windows (`show_window_info_dialog`,
lines ~1390–1700)
- Full custom Win32 list dialog: class `QMKWindowInfoDialog`,
  `WS_OVERLAPPED|WS_CAPTION|WS_SYSMENU|WS_THICKFRAME|WS_MINIMIZEBOX|WS_MAXIMIZEBOX|WS_VSCROLL`,
  default 760×520, min 480×320 (`WM_GETMINMAXINFO`). White background
  (`WHITE_BRUSH`).
- Layout constants (lines ~1380): margin 14, header h 20, row h 26, label h 22,
  copy button 84×22. Header (bold Segoe UI), footer hint, one read-only
  `WC_EDITW` (ES_READONLY|ES_AUTOHSCROLL|ES_NOHIDESEL) + one `WC_BUTTONW "Copy"`
  per row.
- Scrolling: `WININFO_SCROLL_POS: AtomicI32`. `wininfo_relayout` repositions
  in-view rows and hides off-screen ones; scrollbar set via
  `wininfo_set_scrollbar`. `WM_VSCROLL` (with the `SCROLLBAR_COMMAND` match-arm
  workaround for the no-BitOr windows-crate type) and `WM_MOUSEWHEEL` (3 rows
  per notch) update the offset and relayout. `WM_CTLCOLORSTATIC` paints labels
  black-on-transparent over the white brush.
- Copy: `id >= WI_IDC_COPY_BASE (6000)` → index = id − 6000 →
  `copy_text_for_row` → `copy_to_clipboard_windows` (CF_UNICODETEXT=13,
  `GlobalAlloc`/`GlobalLock`/`SetClipboardData`).
- **[info]** `copy_text_for_row` (lines ~1366) returns `class|title` when title
  is non-empty, else just `class`. Matches the QMK config match syntax.

### 1.11 "Show Window Information..." — macOS
(`show_macos_window_info_dialog` + `_inner`, lines ~1710–2338)
- Builds an `NSWindow` (title+closable+miniaturizable, `setReleasedWhenClosed:NO`)
  with an `NSScrollView` + `NSView` document. One row per app: a borderless
  selectable `NSTextField` (truncating tail) + an `NSButton` Copy (SF Symbol
  "doc.on.doc" where `respondsToSelector:` guards macOS 11+, else text "Copy").
  Rows top-aligned (NSView bottom-left origin counting down).
- Two NSObject subclasses registered once:
  - `RustWindowInfoCopyTarget` with `copyRow:` → extern `wi_copy_row` (reads
    `sender tag`, looks up `WINDOW_INFO_ROWS`, copies via
    `copy_to_pasteboard_macos`).
  - `RustWindowInfoWindowDelegate` with `windowWillClose:` → `wi_window_will_close`
    (`stopModal`).
- Activates the app (`activateIgnoringOtherApps:`) and `makeKeyAndOrderFront:`
  before `runModalForWindow:` so the dialog isn't buried under other apps.
- Cleanup releases window/delegate/target.

### 1.12 macOS autostart (`mod autostart`, lines ~67–135)
- Backed by `SMAppService` (ServiceManagement.framework, macOS 13+). Links the
  framework via `#[link(name="ServiceManagement", kind="framework")]`.
- `STATUS_ENABLED = 1` raw value of `SMAppServiceStatus`
  (0=notRegistered,1=enabled,2=requiresApproval,3=notFound). `is_enabled()`
  returns true only when status==1.
- `enable()` → `[mainAppService registerAndReturnError:]`; `disable()` →
  `unregisterAndReturnError:`. Errors surfaced via `nserror_description`.
- `main_app_service()` returns `mainAppService` or `None` when the class isn't
  present (macOS < 13) — every call degrades to a no-op then.
- `autostart_first_run_default_on` (lines ~137–160): dispatched onto the main
  serial queue via `dispatch::Queue::main().exec_async` so the XPC register
  never blocks the launch-critical Init. Writes a marker file
  `.autostart_initialized` under the config dir *regardless of success* (so an
  unsupported OS / transient failure doesn't retry forever). Posts
  `AutostartSync` afterwards.
- `handle_launch_at_login_click` (lines ~175): derives desired state from the
  *real* system status (not the checkbox), performs the (un)register, then
  mirrors the outcome into the checkmark. Robust to any auto-toggle.

### 1.13 macOS bundle resource resolution
- `bundle_resource(name)` (lines ~1180): `CFBundle::main_bundle().executable_url()`
  → parent → `../Resources/<name>`. All `Option`-propagated so an unbundled
  raw binary doesn't panic.
- `load_template_icon_from_bundle` → `load_icon` (image crate, RGBA decode).

### 1.14 Windows tray icon (`load_windows_tray_icon` + `zoom_in_about_20_percent`,
lines ~660–760)
- Looks for `IconTray-dark.png` beside the exe, then `packaging/IconTray-dark.png`.
- `zoom_in_about_20_percent`: finds opaque bounding box, scales up to min(1.2,
  canvas/content) (so a near-full glyph only enlarges as much as fits),
  Lanczos3 resizes, center-crops back to the original canvas. No clipping.

### 1.15 Gaps / quality issues in tray.rs
- **[major]** Menu is **not rebuilt when the device-status line text changes on
  Windows**: it calls `device_status_i.set_text(...)` which works for muda on
  Windows, but the comment in `linux_tray.rs` notes that on some hosts the open
  menu is a static snapshot. On macOS/Windows this is handled at the toolkit
  level. Confirmed OK for tao/muda.
- **[minor]** `create_default_icon` (lines ~615) is a flat 16×16 white square —
  the comments repeatedly note this is invisible on a light taskbar; only
  Windows actually exercises the PNG loader, but any code path that hits the
  fallback renders nothing useful.
- **[nit]** Copyright year literal "2026" (line ~227).
- **[info]** `set_text` is used to update device status, but the menu was built
  with `append_items(&menu_items)` where `device_status_i` is a `MenuItem` —
  fine, but the menu is owned by `tray_menu.clone()` passed into the icon
  builder; the original `MenuItem`s live on the stack of `setup_tray` for the
  whole `event_loop.run` closure, so `id()` and `set_text` remain valid.

---

## 2. `src/linux_tray.rs` — SNI tray (ksni) + GTK/zenity

Read fully (lines 1–~720). `#![cfg(all(target_os = "linux", feature = "linux-tray"))]`.

### 2.1 Icons & theme handling
- Two embedded PNGs: `TRAY_ICON_DARK_PNG` and `TRAY_ICON_LIGHT_PNG`
  (`include_bytes!("../packaging/IconTray-{dark,light}.png")`), 128×128.
- `QmkTray { device_connected: bool, dark_mode: bool }`. `icon_pixmap()` serves
  the variant matching `dark_mode`, and `dim_icon` (α scaled to ~35%, α=90)
  when the device is absent — visible in the bar in realtime.
- `detect_dark_mode` shells out to `dbus-send` querying the
  `org.freedesktop.portal.Settings` `appearance.color-scheme` (1=dark, 2=light,
  0=no pref). Defaults to **dark** on any failure. `parse_color_scheme` is
  unit-tested.

### 2.2 Tray menu structure (ksni)
Order (`menu()`):
1. Device status (disabled `StandardItem`, `● Device Connected` / `○ No Device
   Connected` — parity glyphs with macOS line 2).
2. **Invisible structural toggle** — present *only when connected*. This is the
   clever bit: changing the item *count* forces ksni to emit `LayoutUpdated`
   (which every SNI host honors to redraw an open popup) instead of only
   `ItemsPropertiesUpdated` (which some hosts, e.g. Quickshell, ignore for open
   menus). Both connect→disconnect and disconnect→connect change the count.
   `visible: false`.
3. Separator
4. **Settings…** → `show_settings_dialog_linux()`.
5. Separator
6. **Show Window Information** → `show_window_info_linux()`.
7. Separator
8. **Quit** → `std::process::exit(0)`.

### 2.3 Polling / ksni update (`spawn`, lines ~250–305)
- `QmkTray::new().assume_sni_available(true).spawn()` — registers and *waits*
  silently rather than hard-failing when no SNI host is present. Trayless run
  is an expected, non-fatal state.
- Background poll thread:
  - `DEVICE_POLL_INTERVAL = 1s` (device presence, re-reads config every call).
  - `COLOR_SCHEME_POLL_EVERY = 10` (~30 s) because each check spawns
    `dbus-send`.
  - Updates the tray only on a transition (`last_device`/`last_dark`).
  - `poll_handle.update(|t| { t.device_connected = …; t.dark_mode = …; })`.

### 2.4 Tooltip
- `tool_tip()` is a live indicator: "…device connected" / "…NO DEVICE
  CONNECTED" (reflected by SNI hosts on hover via `NewToolTip`), unlike the
  open menu which some hosts snapshot.

### 2.5 "Show Window Information" — Linux
- `show_window_info_linux` prefers a native GTK popup
  (`gtk_dialog::sender().send(rows)`); falls back to
  `show_window_info_linux_zenity` if GTK can't init (headless) or the channel
  send fails (recovers the rows via the `SendError`).
- **GTK popup** (`mod gtk_dialog`): a single owner thread (started lazily via
  `OnceLock`) runs `gtk::init()` once and `gtk::main()`. Requests arrive over an
  `mpsc::channel` polled every 50 ms via `glib::timeout_add_local`. Each opens
  an independent `WindowType::Toplevel` with `WindowTypeHint::Dialog`,
  `set_resizable(false)`, `set_default_size(640, 760)`. VBox → help label →
  `ScrolledWindow` (vexpand, min height 420, `PolicyType::Never`/`Automatic`)
  containing a `ListBox` (one row per app: `Label` with `EllipsizeMode::End` +
  `Button "Copy"` that sets `CLIPBOARD` to `class|title`) → Close button.
  Comment explains: dialog type-hint + fixed size is what actually floats a
  native window on Wayland tiling compositors (the X11 Dialog hint is largely
  ignored on Wayland).
- **zenity fallback** (`show_window_info_linux_zenity`): `zenity --forms
  --add-list=` (a real dialog → floats everywhere, but list capped at ~3–4
  rows by a hard zenity limitation). `--ok-label=Copy`. List values are
  `|`-joined after `sanitize_list_value` replaces literal `|` with broken-bar
  `¦` (U+00A6). The chosen display string is mapped back to its row to copy
  `class|title`. Falls back to `notify-send` if zenity is absent.
- Clipboard copy: `copy_to_clipboard` prefers `wl-copy` (wl-clipboard) then
  `xclip -selection clipboard`.

### 2.6 Settings dialog — Linux (`show_settings_dialog_linux`)
- `zenity --forms --add-entry="Vendor ID (hex)" --add-entry="Product ID (hex)"`
  with the current configured values shown in `--text`.
- Output split on `|`; each part parsed by local `parse_id` (tolerates `0x`
  prefix + case + whitespace; empty/"auto" → None).
- `write_config` → `crate::platforms::create_config_dir()` + `config.toml`
  via `crate::core::render_config_body`.
- **`apply_device_rule`** (post-save): both-unset → just `udevadm control
  --reload-rules` + `trigger --subsystem-match=hidraw` (the static usage-page
  rule covers it). At least one set → render via
  `crate::platforms::render_vidpid_rule`, stage to `std::env::temp_dir()`, then
  install privileged via `pkexec sh -c "install -m644 … 99-qmkonnect.rules &&
  udevadm control --reload-rules && udevadm trigger … && rm -f tmp"`. On
  pkexec absence/cancel/failure → notify "Settings saved. Run: sudo qmkonnect
  -r". **[info]** this is the root-aware fallback referenced by #26.
- Notifications via `notify-send` (`--app-name=QMKonnect --icon=input-keyboard`).

### 2.7 Gaps / quality issues in linux_tray.rs
- **[major]** `show_settings_dialog_linux` writes `99-qmkonnect.rules`
  (different number than the static `69-qmkonnect-rawhid.rules`). The
  on-demand VID/PID rule is intentionally numbered *higher* so the static rule
  runs first — consistent with the docstring of the static rule. OK.
- **[minor]** `apply_device_rule`'s pkexec install writes to
  `/etc/udev/rules.d/99-qmkonnect.rules`, but the manual `qmkonnect -r` path
  and the static rule live under `/usr/lib/udev/rules.d/`. Mixing locations is
  correct udev-wise (both are scanned) but is an inconsistent install
  location that could confuse a sysadmin auditing rules.
- **[nit]** `dim_icon` uses `DIM_ALPHA = 90` (~35%) — magic number.
- **[info]** Tests cover glyph parity, initial probe, `parse_id`, color-scheme
  parser, and embedded icon decode (128×128 RGBA). Good coverage.

---

## 3. `src/autostart.rs` — Windows HKCU Run autostart

Read fully. `#![cfg(target_os = "windows")]`.

### 3.1 Single source of truth
- `SUBKEY = "Software\\Microsoft\\Windows\\CurrentVersion\\Run"`,
  `VALUE = "QMKonnect"`. **This exact spelling is the contract** shared with
  `packaging/windows/install.ps1`, `uninstall.ps1`, and the Inno `.iss`
  `[Registry]` section — all four must stay in sync.

### 3.2 Implementation
- `is_enabled()` — presence-based `RegGetValueW(HKEY_CURRENT_USER, SUBKEY,
  VALUE, RRF_RT_REG_SZ, …)`, returns `result.is_ok() && len > 0`. Deliberately
  **does not** consult the Task-Manager "Disabled" override (a 12-byte value
  under `StartupApproved\Run`) — the registry is the truth the checkbox
  reflects.
- `set_enabled(b)` → `enable()`/`disable()`.
  - `enable()`: `RegOpenKeyExW(HKCU, SUBKEY, 0, KEY_SET_VALUE)` →
    `RegSetValueExW(hkey, VALUE, 0, REG_SZ, exe bytes)` where exe = `current_exe`
    resolved at toggle time (self-heals when the install moves).
  - `disable()`: `RegDeleteValueW(hkey, VALUE)` (missing value is not an error
    for the caller's purpose).
- Failures are swallowed; the tray handler re-derives the checkbox from
  `is_enabled()` so a failed write visibly reverts.

### 3.3 Gaps
- **[info]** `current_exe_wide` returns `vec![0]` (just a NUL) on
  `current_exe()` failure — `enable()` would then write a zero-length REG_SZ
  which `is_enabled()` would read as present-but-empty; the `len > 0` guard
  covers the byte count of the NUL terminator only, so this edge would
  self-clear on next toggle. Defensive but harmless.

---

## 4. `src/bin/hid_id.rs` — udev helper binary

Read fully. Pure `std`, no hidapi. Binary name `qmkonnect-hid-id`.

### 4.1 Invocation & flow
- Invoked by the udev rule as `qmkonnect-hid-id %S%p` (hidraw syspath as
  argv[1]); falls back to `$DEVPATH` prefixed with `/sys` if argv[1] absent.
- Reads `<syspath>/device/report_descriptor`. Unreadable → returns 0 with no
  stdout (udev sees no properties).
- If `matches_qmk_signature(bytes)` → `println!("ID_QMKONNECT=1")`.

### 4.2 HID report descriptor parser (`matches_qmk_signature`)
- QMK Raw HID signature: Global Usage Page item = `0xFF60` followed (with
  arbitrary items between) by a Local Usage item = `0x61`.
- Walks the short-item stream: prefix byte → `bSize` (bits 0–1; `3`→4 bytes),
  `bType` (bits 2–3; 1=Global, 2=Local), `bTag` (bits 4–7).
- Long item (prefix `0xFE`): skips `[size][tag][data…]` using the second byte
  as data size.
- `(bType=1, bTag=0)` = Global Usage Page → `current_usage_page = read_le(data)`.
- `(bType=2, bTag=0)` = Local Usage → if `current_usage_page == 0xFF60 && usage
  == 0x61` return true. Comment correctly notes tag 2 would be Usage Maximum
  (`29 61`).
- Bounds-checked at every item; truncation → return false (no panic).
- `read_le`: 0–4 bytes little-endian.

### 4.3 Gaps
- **[info]** Does NOT cover firmware that overrode `RAW_USAGE_PAGE` /
  `RAW_USAGE_ID` — those fall back to the config-driven `99-qmkonnect.rules`
  via `qmkonnect --reload`. Documented inline and in the udev rule header.
- **[info]** Tests are thorough: known signature, items-in-between, wrong page,
  wrong usage, usage without QMK page, truncated/empty descriptor, long-item
  skip, `read_le` LE-ness, absolute-vs-relative `$DEVPATH` joining.

---

## 5. `packaging/linux/udev/69-qmkonnect-rawhid.rules`

Read fully (10 lines + header). The canonical copy (an identical one ships in
`packaging/linux/arch/pkg/qmkonnect/usr/lib/udev/rules.d/`).

Content:
```
SUBSYSTEM=="hidraw", IMPORT{program}="/usr/lib/udev/qmkonnect-hid-id %S%p"
ENV{ID_QMKONNECT}=="1", GROUP="input", MODE="0660", TAG+="uaccess", SYMLINK+="qmkonnect_device", TAG+="systemd", ENV{SYSTEMD_USER_WANTS}+="qmkonnect.service"
```

- `IMPORT{program}` runs the hid-id helper, which sets `ID_QMKONNECT=1` only for
  the QMK Raw HID interface. `%S%p` = the syspath.
- On match: `GROUP=input`, `MODE=0660` (group-readable), `TAG+=uaccess`
  (ConsoleKit/systemd seat grants the active user), `SYMLINK+=qmkonnect_device`
  (creates `/dev/qmkonnect_device` → systemd device unit
  `dev-qmkonnect_device.device`), `TAG+=systemd` + `ENV{SYSTEMD_USER_WANTS}+=
  qmkonnect.service` (systemd starts the user service on hotplug).
- Numbered `69` to run before the optional config-driven `99-qmkonnect.rules`.
- **[info]** Statically identical for every keyboard; never regenerated from
  config. Default QMK keyboards need no `--reload`/sudo.

### Gaps
- **[major]** Hardcoded helper path `/usr/lib/udev/qmkonnect-hid-id`. Distros
  that put udev helpers in `/lib/udev/` (Debian/Ubuntu historically) or
  `/usr/libexec/` would not find it. The Arch PKGBUILD installs to
  `/usr/lib/udev/` so Arch is fine; the manual install docs
  (`docs/installation.md`) also install to `/usr/lib/udev/`, so this is
  consistent *within this project's documented paths*, but is a portability
  caveat for distro packagers.
- **[info]** `ENV{SYSTEMD_USER_WANTS}` requires `systemd-udevd` to be the
  device manager; on non-systemd inits this line is a no-op (the `GROUP`/`MODE`
  still grant access).

---

## 6. `packaging/linux/systemd/qmkonnect.service.template`

Read fully. Effectively a **static** file — the `.template` suffix is vestigial
(see `qmkonnect.install` `post_install`: it just `install -m644 … .service`).

Content:
```ini
[Unit]
Description=QMKonnect - QMK Keyboard Window Notifier
After=graphical-session.target
BindsTo=dev-qmkonnect_device.device
StartLimitBurst=5
StartLimitIntervalSec=60

[Service]
Type=simple
ExecStart=/usr/bin/qmkonnect
Restart=always
RestartSec=5
Environment=RUST_BACKTRACE=1
PrivateTmp=false
ProtectSystem=full
ProtectHome=false
NoNewPrivileges=true
ReadWritePaths=/dev
ReadWritePaths=%t

[Install]
WantedBy=default.target
```

### Integration analysis
- `BindsTo=dev-qmkonnect_device.device` matches the udev rule's
  `SYMLINK+=qmkonnect_device`. On unplug the device unit goes inactive → the
  service stops. On hotplug the udev rule's `SYSTEMD_USER_WANTS` starts the
  service. Consistent.
- `StartLimitBurst=5 / IntervalSec=60` + `Restart=always / RestartSec=5` →
  bounded crash recovery.
- `ReadWritePaths=/dev` + `%t` (the user's `$XDG_RUNTIME_DIR`) — minimal
  writable surface. `ProtectSystem=full` makes `/usr` read-only; the app writes
  config to `~/.config` so `ProtectHome=false` is required. **[info]**
- `WantedBy=default.target` (user session default).

### Gaps
- **[major]** **Binary path mismatch with the manual install docs.**
  `ExecStart=/usr/bin/qmkonnect`, but `docs/installation.md` (step 2 "Other
  Linux Distributions") tells users `sudo cp qmkonnect /usr/local/bin/`. A user
  following the manual path will have the binary at `/usr/local/bin/qmkonnect`
  while the service looks for `/usr/bin/qmkonnect` → the service fails to start
  with "status=203/EXEC". The Arch package and the service agree on `/usr/bin`;
  the manual docs are the outlier.
- **[major]** **`.template` suffix is now vestigial.** `post_install` just
  copies the file verbatim — there is no `%`/`@` substitution left (the comment
  in `qmkonnect.install` explicitly says "The old VID/PID substitution is gone").
  Naming it `.template` misleads readers (and the spec/PACKAGING.md still
  claims it's "instantiated by post_install"). Rename to
  `qmkonnect.service`, or restore actual templating.
- **[minor]** `BindsTo=dev-qmkonnect_device.device` inside a **user** service
  is subtle: the user systemd instance only learns about this device unit
  because the udev rule sets `TAG+=systemd` + `SYSTEMD_USER_WANTS`. If a user
  copies the service to `~/.config/systemd/user/` *without* the udev rule
  installed (the `troubleshooting.md` path), the `BindsTo` device unit may
  never materialize in the user instance → service never starts. The
  `troubleshooting.md` instructions do install the rule separately, but the
  dependency is implicit.
- **[nit]** `Description` still says "- QMK Keyboard Window Notifier" — fine.

---

## 7. macOS packaging — `build.sh`, `clean.sh`, `install.sh`

### 7.1 `build.sh`
- `MACOS_UNIVERSAL=1` → builds `aarch64-apple-darwin` + `x86_64-apple-darwin`
  and `lipo -create`s them into `target/release/qmkonnect`. Default: host arch
  only. CI sets the universal flag so the same `.app` runs on both Apple
  Silicon and Intel.
- Assembles `QMKonnect.app/Contents/{MacOS,Resources}`: copies the binary,
  `Icon.icns`, and `IconTemplate.png` (warns on absence). Generates
  `Info.plist` with `CFBundleIdentifier=io.mulletware.qmkonnect`,
  `CFBundleExecutable=qmkonnect`, `LSUIElement=true` (no Dock icon / CMD-Tab).
- **Code signing**: `codesign --deep --force --sign "$SIGN_IDENTITY"` where
  `SIGN_IDENTITY = ${CODESIGN_IDENTITY:--}`. Ad-hoc (`-`) by default → TCC
  (Screen Recording) re-prompts every rebuild because the cdhash changes; a
  `CODESIGN_IDENTITY` env var enables stable designated-requirement signing.
- Builds `QMKonnect.dmg` (UDZO, with an `/Applications` symlink) via `hdiutil`.
- **[info]** `Info.plist` is generated inline (heredoc) — there is no checked-in
  plist; the only `LSUIElement` source is here.

### 7.2 `clean.sh`
- 5-step reset (run before build+install):
  1. `pkill -f QMKonnect.app`.
  2. Detach any mounted `QMKonnect` DMGs from `/Volumes`.
  3. `lsregister -u` for `/Applications/QMKonnect.app` and
     `~/.Trash/QMKonnect.app` (unregister stale copies from LaunchServices —
     otherwise `open` resurrects a stale build).
  4. `rm -rf` those two app bundles.
  5. `tccutil reset ScreenCapture io.mulletware.qmkonnect` (reset the
     Screen-Recording TCC grant because ad-hoc signing gives a new cdhash).
- **[info]** Deliberately does **not** touch the SMAppService "Launch at Login"
  registration — that entry points at `/Applications/QMKonnect.app` and stays
  valid across reinstalls.

### 7.3 `install.sh`
- Mounts the just-built `packaging/macos/QMKonnect.dmg` (resolved relative to
  the script dir) into a `mktemp -d`, `cp -R` into `/Applications/`, detaches.
- **[info]** Installs from the DMG (not the raw bundle) so the dev loop
  exercises the exact artifact users install.
- Autostart is handled in-app (first-run SMAppService default-on), so nothing
  extra here.

### Gaps (macOS packaging)
- **[major]** `build.sh` `set -e` but the `IconTemplate.png` copy is guarded by
  `2>/dev/null || echo …` — OK. However the `Info.plist` heredoc has no
  `CFBundleShortVersionString` / `CFBundleVersion` keys. macOS notarization
  tools and Software Update rely on these; ad-hoc local builds work, but a
  signed/notarized distribution would be rejected or display version "0".
  **[minor]** `build.sh` does not read the version from `Cargo.toml` (the
  Windows `build.ps1` and `install.ps1` do). The DMG filename and bundle
  version are therefore decoupled from the crate version.

---

## 8. Windows packaging — Inno `.iss` + `build.ps1`

(Note: the task named the file `QMKonnet.iss` — a typo. The actual file is
`packaging/windows/inno/QMKonnect.iss`.)

### 8.1 `QMKonnect.iss` (Inno Setup 6)
- Per-user, **no-admin** installer (`PrivilegesRequired=lowest`,
  `DefaultDirName={localappdata}\Programs\QMKonnect`,
  `DisableDirPage=yes`/`DisableProgramGroupPage=yes`). A tray app must run in
  the interactive session; the comment explicitly contrasts this with the
  WiX MSI that installs a Session-0 *service* (which can't show a tray icon).
- `AppId={{FAAE1F7A-9DBD-4C2A-B122-A9A73F05D0B3}` — stable upgrade identity
  (keeps constant across versions so reinstalls upgrade in place).
- `[Files]`: `qmkonnect.exe` → `{app}\QMKonnect.exe`, `Icon.ico`,
  `IconTray-dark.png` (both beside the exe — matches `load_app_icon` /
  `load_windows_tray_icon` which look beside the exe first).
- `[Icons]`: Start Menu shortcut `{userprograms}\QMKonnect` with icon + comment.
- `[Registry]`: `HKCU\…\Run` value name `"QMKonnect"` → `{app}\QMKonnect.exe`,
  `Flags: uninsdeletevalue`. **Same contract** as `autostart.rs`,
  `install.ps1`, `uninstall.ps1`.
- `[Run]`: postinstall launch with `skipifsilent` (so `/VERYSILENT` CI runs
  don't spawn a tray-less background process).
- `[Code]`: `KillRunningInstance()` runs `taskkill /IM qmkonnect.exe /F /T` in
  both `InitializeSetup` and `InitializeUninstall` — releases the
  single-instance named mutex so the exe can be overwritten. Comment explains
  why the lock exists.

### 8.2 `build.ps1`
- Reads version from `Cargo.toml` (`Select-String '^\s*version\s*=\s*"..."'`,
  first match) — single source of truth, mirroring `install.ps1`.
- Locates ISCC: PATH first, then winget user-scope
  (`%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe`), then machine-scope Program
  Files locations. Throws a helpful `winget install JRSoftware.InnoSetup`
  message if absent.
- Invokes `iscc "/DMyAppVersion=$Version" QMKonnect.iss` →
  `Output\QMKonnect-Setup.exe`. Checks `$LASTEXITCODE`.

### 8.3 `install.ps1` (the per-user PowerShell installer the Inno replicates)
- Copies exe + icon assets to `%LOCALAPPDATA%\Programs\QMKonnect`.
- Start Menu `.lnk` (with icon), HKCU Run value (name `QMKonnect`), Add/Remove
  Programs uninstall entry (`DisplayName/Version/Publisher/InstallLocation/
  DisplayIcon/NoModify/NoRepair/UninstallString`), where `UninstallString`
  points at a copied `uninstall.ps1`.
- Stops any running instance before copy (`Get-Process QMKonnect |
  Stop-Process -Force`).
- **[info]** `ReleaseDir` honors `CARGO_TARGET_DIR` in the `.iss`
  (`#if Len(GetEnv("CARGO_TARGET_DIR")) > 0`), but `install.ps1`'s candidate
  logic also checks `$env:CARGO_TARGET_DIR`. This is the env-var trap called out
  in `AGENTS.md` (a machine-wide `CARGO_TARGET_DIR` redirects output away from
  `.\target\`); both scripts handle it, but the AGENTS.md guidance is to *unset*
  it.

### 8.4 `uninstall.ps1`
- Stops the process, removes the Start Menu `.lnk`, the HKCU Run value (name
  `QMKonnect`), the install dir, and the uninstall registry entry.
- **[info]** `$ErrorActionPreference = 'Continue'` (vs `Stop` elsewhere) —
  tolerates partial-state uninstalls.

### Gaps (Windows packaging)
- **[major]** **Two parallel installers for the same target.** `install.ps1`
  and the Inno `.iss` both install the per-user tray app and both manage the
  same HKCU Run value, but they produce *different* Add/Remove-Programs entries
  and slightly different install layouts. A user who installs via `install.ps1`
  then upgrades via `QMKonnect-Setup.exe` (or vice versa) could end up with
  double entries or a stale `uninstall.ps1` orphaned in the install dir. The
  `.iss` comment claims it "Replicates `../install.ps1` exactly," but the
  uninstall-entry shape differs (`.ps1` writes NoModify/NoRepair; `.iss` uses
  Inno's native ARP). Pick one canonical installer.
- **[minor]** The `.iss` `AppId` GUID is hardcoded; if a future WiX/service
  installer is added, the ARP identities could collide.
- **[nit]** Filename typo risk: the task referred to `QMKonnet.iss`; the actual
  file is `QMKonnect.iss`. Internally consistent in the repo.

---

## 9. Cross-cutting integration map

```
                                  config.toml
                                      ▲
              render_config_body ─────┘ (mac/win/linux Settings dialogs)
                                      │
   is_device_connected (core/notifier) ◄── read every poll / notification
          │
          ├── tray.rs (mac/win): 3s poll thread → UserEvent::DeviceStatus
          │   → device_status_i.set_text("●/○ Device [Not] Connected")
          └── linux_tray.rs (ksni): 1s poll thread → handle.update()

   autostart:
     macOS  : tray.rs::autostart (SMAppService)   — first-run default-on
     Windows: autostart.rs (HKCU Run "QMKonnect") — installer default-on
              contract shared by install.ps1 / uninstall.ps1 / .iss

   Linux hotplug:
     udev 69-…rules → qmkonnect-hid-id (hid_id.rs) sets ID_QMKONNECT=1
       → GROUP/MODE/uaccess + SYMLINK=qmkonnect_device + SYSTEMD_USER_WANTS
     systemd user service BindsTo=dev-qmkonnect_device.device → start/stop
     ExecStart=/usr/bin/qmkonnect
```

---

## 10. Prioritised gaps / quality issues (consolidated)

| Sev | Location | Issue |
|-----|----------|-------|
| major | `qmkonnect.service.template` ExecStart vs `docs/installation.md` step 2 | Manual install copies to `/usr/local/bin/` but service expects `/usr/bin/qmkonnect` → service won't start. |
| major | `qmkonnect.service.template` filename | `.template` suffix is vestigial (no substitution); `post_install` copies it verbatim. Rename or restore templating. |
| major | `packaging/windows/{install.ps1, inno/QMKonnect.iss}` | Two parallel per-user installers managing the same HKCU Run value but different ARP entries — risk of double/stale entries on mixed install paths. |
| major | `69-qmkonnect-rawhid.rules` | Hardcoded `/usr/lib/udev/qmkonnect-hid-id` helper path; distros using `/lib/udev/` or `/usr/libexec/` won't find it (consistent within this project's docs, caveat for packagers). |
| minor | `linux_tray.rs apply_device_rule` | Installs on-demand rule to `/etc/udev/rules.d/99-qmkonnect.rules` while static rule + `qmkonnect -r` use `/usr/lib/udev/rules.d/` — inconsistent locations. |
| minor | `tray.rs create_default_icon` | Flat 16×16 white fallback is invisible on light taskbars; only Windows exercises the real PNG loader. |
| minor | `build.sh` (macOS) | No `CFBundleShortVersionString`/`CFBundleVersion` in generated `Info.plist`; version not read from `Cargo.toml` (unlike Windows scripts). |
| minor | `qmkonnect.service.template` `BindsTo` | User-service `BindsTo=dev-qmkonnect_device.device` is only satisfied if the udev rule (with `TAG+=systemd`) is also installed; implicit cross-file dependency. |
| nit | `tray.rs` about metadata | Copyright year hardcoded "2026". |
| nit | `tray.rs` macOS settings dialog | Hardcoded NSRect layout literals. |
| info | throughout | No bugs found in the EventLoopProxy flow, autostart contracts, hid_id parser, or ksni update/dim logic; tests cover the parser, color-scheme parser, icon decode, and id parsing. |

---

## 11. Start-here pointers for an editor

1. **Tray menu / event handling**: `src/tray.rs:266` (`setup_tray`) → menu
   assembly ~245–340 → run loop ~358–470.
2. **macOS autostart**: `src/tray.rs:67` (`mod autostart`) + first-run default
   `src/tray.rs:137` (`autostart_first_run_default_on`).
3. **Windows autostart**: `src/autostart.rs` (whole file, ~120 lines).
4. **Linux SNI tray + GTK/zenity**: `src/linux_tray.rs:1` (module doc) →
   `spawn` ~250 → settings `show_settings_dialog_linux` ~480.
5. **udev helper**: `src/bin/hid_id.rs:104` (`matches_qmk_signature`).
6. **Linux hotplug wiring**: `packaging/linux/udev/69-qmkonnect-rawhid.rules`
   ↔ `packaging/linux/systemd/qmkonnect.service.template` ↔
   `packaging/linux/arch/qmkonnect.install` (post_install copies the "template").
7. **Windows installer contract**: `packaging/windows/inno/QMKonnect.iss`
   `[Registry]` ↔ `src/autostart.rs` `VALUE` ↔ `install.ps1`/`uninstall.ps1`.

---

## Acceptance

This report is the sole artifact produced; no source files were modified
(review-only / scouting task). All findings carry exact file paths + line
ranges and severity tags. The highest-severity items (`major`) are concrete and
actionable; none are speculative.

# P3.M2.T1.S1 — tray.rs Windows Settings Dialog Verification (codebase recon)

Target: `src/tray.rs` (Rust, `windows` crate v0.52.0). All line numbers below are
exact and verified against the working tree (file is 2431 lines; the dialog block
lives ~779–1109). Every `#[cfg(target_os = "windows")]`-gated item compiles on
Windows. The `core::notifier` device-classification API **already exists in-tree**
(not upstream/in-progress): `classify_devices`, `classification_cache_clear`,
`DeviceKind`, `ClassifiedDevice`, `DeviceFilter`, `configured_filter` are all `pub`.

## §1. `DIALOG_RESULT` static — declaration + EVERY read/write site

**Declaration — `src/tray.rs:53-55`** (gated `#[cfg(target_os = "windows")]`):

```rust
// Shared result slot for the Windows settings dialog, replacing the former
// Arc::into_raw + mem::forget leak (#9).
#[cfg(target_os = "windows")]
static DIALOG_RESULT: std::sync::Mutex<Option<(Option<u16>, Option<u16>)>> =
    std::sync::Mutex::new(None);
```

Three touch sites (all `#[cfg(target_os = "windows")]`):

1. **Reset-before-create — `src/tray.rs:796`** (inside `show_settings_dialog`):
   ```rust
   DIALOG_RESULT.lock().unwrap().take();
   ```
2. **Read-after-message-loop — `src/tray.rs:892`**:
   ```rust
   let result = DIALOG_RESULT.lock().unwrap().take();
   ```
   consumed at `src/tray.rs:894`: `if let Some((vendor_id, product_id)) = result {`
3. **Write in OK arm — `src/tray.rs:1081`** (inside `settings_dialog_proc`, `WM_COMMAND` → `1003`):
   ```rust
   *DIALOG_RESULT.lock().unwrap() = Some((vendor_id, product_id));
   let _ = DestroyWindow(hwnd);
   ```

**Sites that MUST change for `struct DialogResult { chosen, manual }`:**
declaration inner type (54-55); read/desugar (892 + 894 — chosen-first-else-manual);
write (1081 — store `Some(DialogResult{chosen, manual})`). The reset (`.take()` @796)
is type-agnostic — unchanged.

## §2. Save-path overlay — verbatim (`src/tray.rs:891-907`)

```rust
        let result = DIALOG_RESULT.lock().unwrap().take();

        if let Some((vendor_id, product_id)) = result {
            let mut merged = current_config;
            merged.vendor_id = vendor_id;
            merged.product_id = product_id;
            let config_content = crate::core::render_config_body(&merged);
            crate::core::atomic_write(config_path, &config_content)?;
        }
```
Lines 900-905 (`merged.*` + `render_config_body` + `atomic_write`) stay verbatim;
only 892/894 + the two `merged.* =` assignments change (chosen-first-else-manual).

## §3. `create_dialog_controls` — full CreateWindowExW sequence (`src/tray.rs:917-1041`)

Signature (917-924); imports (925-928) — `WC_BUTTONW, WC_EDITW, WC_STATICW` from
`Controls`; `CreateWindowExW, SetDlgItemTextW, WS_CHILD, WS_TABSTOP, WS_VISIBLE`
from `WindowsAndMessaging`. **NONE of `WC_LISTBOX`/`BS_GROUPBOX`/`LB_*`/`LBS_*` are
imported anywhere in tray.rs yet** — they are net-new.

Exact coordinates (`x, y, w, h`):
| Control | ID | x | y | w | h | Line |
|---|---|---|---|---|---|---|
| VID label | — | 20 | 30 | 120 | 20 | 929 |
| VID EDIT | 1001 | 150 | 28 | 100 | 24 | 945 (WS_EX_CLIENTEDGE) |
| PID label | — | 20 | 70 | 120 | 20 | 961 |
| PID EDIT | 1002 | 150 | 68 | 100 | 24 | 977 |
| OK | 1003 | 150 | 110 | 75 | 30 | 993 |
| Cancel | 1004 | 240 | 110 | 75 | 30 | 1009 |

Prefill (`1025-1041`): `to_wide_string(config.vendor_id.map(|v| format!("{:04x}",v)).unwrap_or_default())`
→ `SetDlgItemTextW(hwnd, 1001, PCWSTR(vendor_text.as_ptr()))` (same for 1002).

Existing controls occupy y≈20..140. New LISTBOX + group box + relocated buttons
go below/around; `dialog_height=200` (line 828) MUST be bumped.

## §4. `settings_dialog_proc` (`src/tray.rs:1048-1104`)

Imports (1054-1057): `DefWindowProcW, DestroyWindow, GetDlgItemTextW, MessageBoxW,
PostQuitMessage, MB_ICONERROR, MB_OK, WM_CLOSE, WM_COMMAND, WM_DESTROY`. Note
`GetDlgItem` + `SendMessageW` are NOT imported here yet (they are in the
window-info area, §6) — must be added.

```rust
        WM_COMMAND => {
            let control_id = (wparam.0 & 0xFFFF) as u32;          // :1061 LOWORD
            match control_id {
                1003 => {                                          // OK
                    let mut vendor_buffer = [0u16; 256];
                    let mut product_buffer = [0u16; 256];
                    GetDlgItemTextW(hwnd, 1001, &mut vendor_buffer);
                    GetDlgItemTextW(hwnd, 1002, &mut product_buffer);
                    let vendor_str = String::from_utf16_lossy(&vendor_buffer).trim_end_matches('\0').to_string();
                    let product_str = String::from_utf16_lossy(&product_buffer).trim_end_matches('\0').to_string();
                    match (parse_id_field(&vendor_str), parse_id_field(&product_str)) {
                        (Ok(vendor_id), Ok(product_id)) => {
                            *DIALOG_RESULT.lock().unwrap() = Some((vendor_id, product_id));   // :1081
                            let _ = DestroyWindow(hwnd);
                        }
                        (Err(e), _) | (_, Err(e)) => { /* MessageBoxW */ }
                    }
                }
                1004 => { let _ = DestroyWindow(hwnd); }          // Cancel
                _ => {}
            }
        }
```

`parse_id_field` (shared windows+macos, `tray.rs:70-80`): empty/"auto" ⇒ `Ok(None)`.

## §5. Dialog window itself (`src/tray.rs:827-849`)

`dialog_width = 400` (827), `dialog_height = 200` (828). Styles
`WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE`. Centering (829-830)
derives from the two locals. Window class `QMKSettingsDialog` registered at
806-817 (`lpfnWndProc: Some(settings_dialog_proc)`, `COLOR_3DFACE+1` background).

## §6. `WINDOW_INFO_ROWS` pattern + SendMessageW/GetDlgItem reference

`WINDOW_INFO_ROWS` (`src/tray.rs:80-87`, gated macos+windows):
```rust
static WINDOW_INFO_ROWS: std::sync::Mutex<Vec<(String, String)>> =
    std::sync::Mutex::new(Vec::new());
```
**Pattern to mirror for `PICKER_DEVICES: Mutex<Vec<ClassifiedDevice>>`** (Windows-gated).

`wininfo_move_ctl` (`1796-1804`) — the `GetDlgItem` + `MoveWindow` shape:
```rust
let ctl = GetDlgItem(hwnd, id);
if ctl.0 != 0 { let _ = MoveWindow(ctl, x, y, w, h, true); }
```
`set_font` closure (`1635-1648`) — the canonical `SendMessageW` + `WPARAM`/`LPARAM` shape:
```rust
let _ = SendMessageW(ctl, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
```
This is exactly the shape needed for `LB_ADDSTRING`/`LB_GETCURSEL`/`LB_RESETCONTENT`
(lParam = wide-string ptr for LB_ADDSTRING).

## §7. `core::notifier` API — ALL requested symbols EXIST in-tree (pub + #[allow(dead_code)])

- `pub enum DeviceKind { Capable { proto_ver:u8, feature_flags:u8, callback_count:u8, board_rules_present:bool }, NotQmkNotifier }` — `notifier.rs:816-833` (derives Debug, Clone, PartialEq).
- `pub struct ClassifiedDevice { pub path:String, pub vendor_id:u16, pub product_id:u16, pub product_name:Option<String>, pub usage_page:u16, pub usage:u16, pub kind:DeviceKind }` — `notifier.rs:841-850` (derives Debug, Clone, PartialEq). **vendor_id/product_id are `u16`** ⇒ a pick yields `(u16,u16)`.
- `pub fn classify_devices(verbose: bool) -> Vec<ClassifiedDevice>` — `notifier.rs:1115-1119`.
- `pub fn classification_cache_clear()` — `notifier.rs:916-920`.
- `pub struct DeviceFilter { pub vendor_id:Option<u16>, pub product_id:Option<u16>, pub usage_page:u16, pub usage:u16 }` — `notifier.rs:67-73`.
- `fn configured_filter() -> DeviceFilter` — `notifier.rs:82-94` (**private** — the picker does NOT call it; `classify_devices` encapsulates it).

## §8. `to_wide_string` (`src/tray.rs:1114-1121`)

```rust
fn to_wide_string(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()  // NUL-terminated
}
```
Pass as `windows::core::PCWSTR(vec.as_ptr())`. Literals use `windows::core::w!("...")`.
Runtime `format!`-built strings (listbox rows) MUST use `to_wide_string`.

## Risks / gotchas

1. **chosen vs manual type mismatch** — `ClassifiedDevice.vendor_id/product_id` are `u16`
   (always Some), so a pick yields `(u16,u16)`; typed fields yield `(Option<u16>,Option<u16>)`.
   Lift chosen with `Some(v)`/`Some(p)` in the save path.
2. **No listbox code exists yet** — `WC_LISTBOX`, `BS_GROUPBOX`, all `LB_*`/`LBS_*` must be
   added to the `use` blocks in `create_dialog_controls` (923-928) + `settings_dialog_proc`
   (1054-1057). Verified present in `windows` 0.52: `WC_LISTBOX` in `Win32::UI::Controls`;
   `BS_GROUPBOX`/`LB_*`/`LBS_*` in `Win32::UI::WindowsAndMessaging`.
3. **`dialog_height=200` (828) too short** for a LISTBOX + group box — bump before testing.
4. **Style-combination cast** — `WS_*` are `WINDOW_STYLE(u32)`; `BS_GROUPBOX`/`LBS_*` are `i32`.
   Combine via `WINDOW_STYLE(WS_CHILD.0 | … | LBS_NOTIFY as u32)` — the pattern at `tray.rs:1727`
   (`WINDOW_STYLE(ES_READONLY as u32 | ES_AUTOHSCROLL as u32 | ES_NOHIDESEL as u32)`).
5. **[Rescan] runs HID I/O inline** on the tray thread (cache warm ⇒ ≈0 pings; cold ⇒ ~N×timeout).
   Acceptable for v1 (spec doesn't require a worker); must repopulate synchronously.
6. **`configured_filter` is private** — use `classify_devices` (encapsulates it).
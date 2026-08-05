# Win32 LISTBOX + BS_GROUPBOX Research (external, MS Learn)

Source: Microsoft Learn (verified constant names against the `windows` crate
v0.52.0 at `~/.cargo/registry/.../windows-0.52.0/src/Windows/Win32/UI/...`).

## 1. LISTBOX creation (CreateWindowExW)

- Window class: `WC_LISTBOX` = `windows::core::w!("ListBox")` (lives in
  `windows::Win32::UI::Controls`, NOT `WindowsAndMessaging`). Confirmed present
  in the crate: `pub const WC_LISTBOX: ::windows_core::PCWSTR = w!("ListBox");`.
- Recommended styles (all `i32` raw consts in `WindowsAndMessaging`):
  `WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_VSCROLL | LBS_NOTIFY |
  LBS_HASSTRINGS | LBS_NOINTEGRALHEIGHT`.
  - `LBS_NOTIFY` (=1): sends `WM_COMMAND` on selection change. We read selection
    at OK time, so a live `LBN_SELCHANGE` handler is NOT required for v1 — but
    include `LBS_NOTIFY` for standard behavior.
  - `LBS_HASSTRINGS` (=64): the listbox owns the string memory (default for a
    non-owner-drawn listbox; required for `LB_GETTEXT` to work).
  - `LBS_NOINTEGRALHEIGHT` (=256): show a fractional last row instead of
    snapping to a whole number of rows — important for a fixed-pixel dialog.
  - `WS_VSCROLL` (=0x00200000): vertical scrollbar if items overflow.
- Ex-style: `WS_EX_CLIENTEDGE` (sunken border, matches the existing EDIT boxes
  at `tray.rs:946`).
- `hMenu` param = the control ID as `HMENU(<id>)`. Use ID `1010`.

## 2. Message sequence (SendMessageW)

| Message | wParam | lParam | Returns |
|---|---|---|---|
| `LB_ADDSTRING` (384) | 0 | `PCWSTR` ptr (wide, NUL-terminated) | index of added item, or `LB_ERR` |
| `LB_RESETCONTENT` (388) | 0 | 0 | not meaningful |
| `LB_GETCURSEL` (392) | 0 | 0 | selected index, or `LB_ERR` (−1) if none |
| `LB_SETCURSEL` (390) | index | 0 | not meaningful |
| `LB_GETCOUNT` (395) | 0 | 0 | item count |

- **CRITICAL: `LB_ERR` is −1.** `SendMessageW` returns `LRESULT(isize)`. Cast
  `.0 as i32` and compare `!= LB_ERR` (NOT `!= 0`, because index 0 is a valid
  selection). Guard: `let sel = SendMessageW(lb, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0 as i32; if sel != LB_ERR && sel >= 0 { ... }`.
- All `LB_*` constants are `u32` in the crate; `LB_ERR` is `i32 = -1`.

## 3. WM_COMMAND decode for a LISTBOX

- `LOWORD(wParam)` = control ID, `HIWORD(wParam)` = notification code.
- Existing codebase already extracts the low word: `let control_id = (wparam.0 & 0xFFFF) as u32;` (`tray.rs:1061`).
- For the LISTBOX selection notification, the high word is `LBN_SELCHANGE` (=1).
  Extract: `let code = ((wparam.0 >> 16) & 0xFFFF) as u16;`. **Not required for
  v1** (we read at OK), but documented.
- `LBN_SELCHANGE` fires on click AND keyboard-arrow selection.

## 4. BS_GROUPBOX

- Class `WC_BUTTONW`, style `BS_GROUPBOX` (=7, raw `i32`).
- A group box is **purely visual** — it draws a frame + title and that's it. It
  does NOT intercept clicks on its children. It does **NOT** send `BN_CLICKED`
  (only push/auto buttons do), so it must never be branched on in `WM_COMMAND`.
- Create the group box **BEFORE** the controls it visually contains (labels +
  edits) so those children are higher in z-order and paint correctly on top.
- Give it a control ID anyway (e.g. `1012`) for consistency / future Hide, but
  do not handle it.

## 5. Tab-stop alignment (LBS_USETABSTOPS) vs space-padded strings

- `LBS_USETABSTOPS` + `LB_SETTABSTOPS` (wParam=count, lParam=ptr to `i32` array)
  positions tabs in **dialog-template units** (¼ of the avg char width), which
  require conversion for a pixel-based dialog (ours is pixels).
- **Recommendation: space-padded `format!` strings.** For a 2–4 row list the
  alignment via `format!("{:<22} 0x{:04X}:0x{:04X}  {}", name, vid, pid, glyph)`
  is robust, needs no extra message, and survives font changes acceptably. This
  is what the PRP specifies.

## 6. Sizing (pixels)

- A listbox item height ≈ `TEXTMETRIC.tmHeight + tmExternalLeading` ≈ 16–20 px
  at Segoe UI 9pt. For 4–5 visible rows use `height ≈ 100–120 px` with
  `LBS_NOINTEGRALHEIGHT` so a partial row is acceptable.
- The dialog is created with raw pixels (`CreateWindowExW` x/y/cx/cy are device
  units), so no DLU conversion is needed.

## 7. Style-combination gotcha (windows 0.52 crate — VERIFIED)

`WS_CHILD`/`WS_VISIBLE`/`WS_TABSTOP`/`WS_VSCROLL` are `WINDOW_STYLE` newtypes
(`pub struct WINDOW_STYLE(pub u32)`) and combine via `BitOr`. But `BS_GROUPBOX`,
`LBS_NOTIFY`, `LBS_HASSTRINGS`, `LBS_NOINTEGRALHEIGHT` are raw `i32` consts and
do NOT implement `BitOr<WINDOW_STYLE>`. The codebase pattern (verified at
`tray.rs:1727`: `WINDOW_STYLE(ES_READONLY as u32 | ES_AUTOHSCROLL as u32 | ...)`)
is to cast the raw consts `as u32`, OR them, and wrap in `WINDOW_STYLE(...)`.

```rust
// LISTBOX (1010)
WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | WS_VSCROLL.0
             | LBS_NOTIFY as u32 | LBS_HASSTRINGS as u32 | LBS_NOINTEGRALHEIGHT as u32)

// GROUPBOX (1012)
WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | BS_GROUPBOX as u32)
```

## Sources (Microsoft Learn)

- CreateWindowExW — https://learn.microsoft.com/en-us/windows/win32/winmsg/createwindowexw (hMenu→control ID; pixels)
- LB_ADDSTRING — https://learn.microsoft.com/en-us/windows/win32/controls/lb-addstring
- LB_GETCURSEL — https://learn.microsoft.com/en-us/windows/win32/controls/lb-getcursel
- LB_RESETCONTENT — https://learn.microsoft.com/en-us/windows/win32/controls/lb-resetcontent
- WM_COMMAND — https://learn.microsoft.com/en-us/windows/win32/menurc/wm-command
- LBN_SELCHANGE — https://learn.microsoft.com/en-us/windows/win32/controls/lbn-selchange
- Button Styles (BS_GROUPBOX) — https://learn.microsoft.com/en-us/windows/win32/controls/button-styles
- LB_SETTABSTOPS — https://learn.microsoft.com/en-us/windows/win32/controls/lb-settabstops
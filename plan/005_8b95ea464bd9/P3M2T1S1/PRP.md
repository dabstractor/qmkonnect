# PRP — P3.M2.T1.S1: Windows Win32 picker (LISTBOX) + Advanced group box in src/tray.rs

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This task restructures the **Windows** native
> Settings dialog (`show_settings_dialog` in `src/tray.rs`) so that a live,
> self-populating **LISTBOX** of discovered devices (`classify_devices`) is the
> primary surface, and the legacy VID/PID `EDIT` controls are relocated under an
> **"Advanced / manual override" group box** (disclosure). Selecting a listbox row
> writes that board's VID/PID to `config.toml`; a **[Rescan]** button clears the
> classification cache and re-classifies. Source of truth:
> **`spec/DEVICE_DISCOVERY.md` §5** (the Discovered-Device Picker) +
> **`spec/UI.md` §2.0/§2.1** (the picker as new primary surface + the Win32 dialog
> contract). macOS (`P3.M2.T1.S2`) and Linux (`P3.M2.T1.S3`) pickers are separate
> sibling tasks — this PRP touches **Windows only**.
>
> **CONSUMES (read-only, already in tree — verified):** `classify_devices(verbose)
> -> Vec<ClassifiedDevice>` (`notifier.rs:1116`), `classification_cache_clear()`
> (`notifier.rs:917`), `pub enum DeviceKind { Capable{..}, NotQmkNotifier }`
> (`notifier.rs:816`), `pub struct ClassifiedDevice { path, vendor_id:u16,
> product_id:u16, product_name:Option<String>, usage_page, usage, kind }`
> (`notifier.rs:841`). All four are `pub` and currently `#[allow(dead_code)]`
> because no consumer existed yet — **this task is that consumer** (the `allow`
> attributes become satisfied; leave them, harmless).
>
> **DOES NOT TOUCH:** the write path (P2 DEFER), `classify_devices`/cache logic
> (P3.M1 — Complete), `device_status()` (P1 — Complete), macOS/Linux dialogs
> (P3.M2.T1.S2/S3), CLI flags (P4), `Cargo.toml`, the crate, or any `docs/*.md`
> (Mode A — P4.M2 owns user docs). Single-file change: `src/tray.rs`.

---

## Goal

**Feature Goal**: Restructure `show_settings_dialog` (`src/tray.rs:779`) so the
Windows Settings dialog shows a **LISTBOX** of discovered devices (built from
`classify_devices`) as the primary surface, with the two legacy VID/PID `EDIT`
controls relocated under a `BS_GROUPBOX` "Advanced / manual override" disclosure.
Selecting a listbox row becomes the disambiguation: it writes that board's
`vid`/`pid` into `config.toml` via the shared `render_config_body` renderer. A
**[Rescan]** button invalidates `CLASSIFICATION_CACHE` and re-runs
`classify_devices`. The zero-config case (one capable board, no VID/PID set) is
preserved: the list is hidden and a static `Detected: <name>` line is shown.

**Deliverable** (additive edits to `src/tray.rs`, `#[cfg(target_os = "windows")]`):
1. **`struct DialogResult { chosen: Option<(u16,u16)>, manual: Option<(Option<u16>,Option<u16>)> }`** — replaces the `(Option<u16>, Option<u16>)` tuple inside `DIALOG_RESULT`.
2. **`DIALOG_RESULT: Mutex<Option<DialogResult>>`** (extends the static at line 53).
3. **`static PICKER_DEVICES: Mutex<Vec<ClassifiedDevice>>`** — the listbox's row store (mirrors `WINDOW_INFO_ROWS` @80), keyed by row index → `(vendor_id, product_id)`.
4. **`static DIALOG_OPEN_VIDPID: Mutex<(Option<u16>, Option<u16>)>`** — the dialog-open config's vid/pid, so the proc's [Rescan] arm can re-evaluate the clean-auto case without the `config_path`.
5. **4 new control IDs**: `1010` (LISTBOX), `1011` (Rescan `BUTTON`), `1012` (Advanced `BS_GROUPBOX`), `1013` (header `WC_STATICW`). The existing `1001`/`1002`/`1003`/`1004` IDs are preserved (the two `EDIT`s move under the group box but keep their IDs); window-info's `4001`+ / `5000`+ / `6000`+ ranges are untouched.
6. **`create_dialog_controls`** (`tray.rs:917`) rewritten: taller dialog (420×380), new controls created, the two `EDIT`s + their labels relocated under the group box, then calls `populate_device_picker`.
7. **`fn populate_device_picker(hwnd, devices, open_vid, open_pid)`** — the reusable LISTBOX populator (initial open + [Rescan]): stores `PICKER_DEVICES`, `LB_RESETCONTENT` + `LB_ADDSTRING` per device, sets the header text + listbox/rescan visibility per the three spec cases.
8. **`fn picker_row_text(d: &ClassifiedDevice) -> String`** — `✓/✗` glyph + name + `0xVVVV:0xPPPP` + status label, space-padded.
9. **`settings_dialog_proc`** (`tray.rs:1048`) extended: the OK arm (`1003`) now also reads the listbox selection (`LB_GETCURSEL` → `chosen`) and stores `DialogResult{chosen, manual}`; a new `1011` ([Rescan]) arm clears the cache + re-classifies + repopulates.
10. **Save path** (`tray.rs:891-907`) updated: apply `chosen` first, else `manual`, else leave VID/PID as-is.
11. **Mode-A doc-comment** on `show_settings_dialog` / `create_dialog_controls` citing `spec/DEVICE_DISCOVERY.md` §5 + `spec/UI.md` §2.1.
12. **1 unit test** (`test_picker_row_text_glyphs`) verifying the ✓/✗ row format (the Win32 dialog itself is not unit-testable; the pure string builder is).

**Success Definition**:
- `cargo build --bin qmkonnect` clean (on Windows; no NEW warnings). `cargo build` on Linux/macOS still clean (all new code is `#[cfg(target_os = "windows")]`).
- The dialog, when `classify_devices` returns ≥2 Tier-1 devices OR 1 non-capable board, shows the LISTBOX with one row per device (`✓`/`✗` glyph + name + `0xVID:0xPID` + status); selecting a row + OK writes that board's `(vid,pid)`.
- When `classify_devices` returns exactly one `Capable` board AND the open-time config has no VID/PID, the LISTBOX + [Rescan] are hidden and the header reads `Detected: <name>`.
- When `classify_devices` returns no devices, the LISTBOX + [Rescan] are hidden and the header reads `No QMK keyboards detected...`.
- [Rescan] calls `classification_cache_clear()` + `classify_devices(true)` and repopulates the LISTBOX (visible after flashing a board).
- The Advanced `EDIT` fields still work: typing a hex pair + OK (with no listbox selection) writes the manual VID/PID; empty/`auto` ⇒ `None` ⇒ auto-discovery. `chosen` takes precedence over `manual`.
- `git status` = `src/tray.rs` only.

## User Persona (if applicable)

**Target User**: a Windows user with one or more QMK keyboards who opens
Settings (menu → Settings…) to confirm which board QMKonnect sees or to
disambiguate among several.

**Use Case**: the user has flashed `qmk_notifier` onto one board and has a second
QMK board (VIA/Vial only). They open Settings → the LISTBOX shows both:
`✓ Dactyl 0xFEED:0x0000 ← qmk_notifier` and `✗ Keychron 0x3434:0x0123 ← QMK
board, no module`. They click the capable row → OK → `config.toml` records that
board's VID/PID so notifications narrow to it. If they later flash `qmk_notifier`
onto the second board, they click [Rescan] to refresh the list.

**Pain Points Addressed**: today the Windows dialog is two raw hex fields — the
user must already know their board's VID/PID and type it blind. The picker shows
live `product_name` from the HID descriptor (the device names itself; no curated
database) and makes selection a one-click disambiguation (`spec/UI.md` §2.0).

## Why

- **`DEVICE_DISCOVERY.md` §5.1 mandates the picker as the new primary surface.**
  Raw VID/PID hex entry becomes an "Advanced / manual override" disclosure. This
  task ships the Windows rendering of that picker (§5.3: "Win32 `LISTBOX` …; VID/
  PID fields under a 'Advanced ▸' group box").
- **`UI.md` §2.0/§2.1 specify the Win32 dialog contract** that this dialog
  already implements (`QMKSettingsDialog` window class, `WS_OVERLAPPED|…`,
  control IDs `1001`-`1004`). This task adds the picker + relocates the fields
  while preserving that contract.
- **The zero-config promise must be preserved (§5.1).** The common case — one
  capable board, no VID/PID set — must NOT show a picker; a static `Detected:
  <name>` line is enough. This task implements that branch.
- **Unblocks the three-platform picker parity** (S2 macOS, S3 Linux follow the
  same `DIALOG_RESULT` shape and the same chosen-first-else-manual save path).
- **Consumes the classification API** shipped by P3.M1.T1.S1/S2 (Complete). This
  task is the first real UI consumer; it exercises `classify_devices` +
  `classification_cache_clear` end-to-end on Windows.

## What

Additive `#[cfg(target_os = "windows")]` edits to `src/tray.rs`. No new Cargo
deps (`windows` 0.52 already provides `WC_LISTBOX`, `BS_GROUPBOX`, `LB_*`,
`LBS_*`, `SendMessageW`, `GetDlgItem`, `ShowWindow`, `SW_HIDE`/`SW_SHOW` — all
verified present in the crate + used elsewhere in this file). No macOS/Linux
behavior change (everything is Windows-gated).

### Success Criteria
- [ ] **`struct DialogResult`** with `chosen: Option<(u16,u16)>` + `manual: Option<(Option<u16>,Option<u16>)>`, `#[derive(Clone, Default)]`, declared `#[cfg(target_os = "windows")]` just above the `DIALOG_RESULT` static.
- [ ] **`DIALOG_RESULT: Mutex<Option<DialogResult>>`** — declaration (`tray.rs:54`) extended; reset (`.take()` @796) unchanged; read site (@892/894) destructured as `Some(dr)` with chosen-first-else-manual; write site (@1081) stores `Some(DialogResult{chosen, manual})`.
- [ ] **`PICKER_DEVICES: Mutex<Vec<ClassifiedDevice>>`** + **`DIALOG_OPEN_VIDPID: Mutex<(Option<u16>,Option<u16>)>`** new statics, Windows-gated.
- [ ] **4 new control IDs** `1010`/`1011`/`1012`/`1013` (avoid `1001`-`1004` AND window-info's `4001`-`4013`/`5000`+/`6000`+ ranges).
- [ ] **`create_dialog_controls`** creates: header static (1013), LISTBOX (1010, `WS_EX_CLIENTEDGE`, `LBS_NOTIFY|LBS_HASSTRINGS|LBS_NOINTEGRALHEIGHT|WS_VSCROLL`), [Rescan] button (1011), Advanced group box (1012, `BS_GROUPBOX`), then the relocated VID/PID labels + EDITs (1001/1002) inside the group box, then OK (1003) + Cancel (1004). Prefills 1001/1002 (unchanged). Then calls `populate_device_picker`.
- [ ] **`populate_device_picker(hwnd, devices, open_vid, open_pid)`** stores `PICKER_DEVICES`; `LB_RESETCONTENT` + `LB_ADDSTRING` per device; sets header + visibility per the 3 cases (empty / clean-auto / picker).
- [ ] **`picker_row_text`**: `✓` (`\u{2713}`) for `Capable`, `✗` (`\u{2717}`) for `NotQmkNotifier`; name or `(unnamed)`; `0x{:04X}:0x{:04X}`; status `qmk_notifier` / `QMK board, no module`. Space-padded (`format!("{:<22}", name)`).
- [ ] **OK arm (1003)** reads listbox selection via `GetDlgItem(hwnd,1010)` + `LB_GETCURSEL` (cast `.0 as i32`, compare `!= LB_ERR && >= 0`) → `chosen = Some((vid,pid))` from `PICKER_DEVICES`; reads 1001/1002 → `parse_id_field` each → `manual`. Stores `DialogResult{chosen, manual}`. Parse error ⇒ `MessageBoxW` (unchanged error path).
- [ ] **[Rescan] arm (1011)** calls `classification_cache_clear()` + `classify_devices(true)`, reads `DIALOG_OPEN_VIDPID`, calls `populate_device_picker`.
- [ ] **Save path** applies `chosen` first (`Some((v,p))` ⇒ `merged.vendor_id=Some(v); merged.product_id=Some(p)`), else `manual` (`merged.vendor_id=v; merged.product_id=p`), else leaves `current_config`'s VID/PID. `render_config_body` + `atomic_write` unchanged.
- [ ] **Dialog height bumped** from 200 to ~380 (width 400→420) so the LISTBOX + group box + relocated buttons fit without clipping; centering math (`tray.rs:829-830`) derives from the locals so no other edit.
- [ ] **Mode-A doc-comments** cite `spec/DEVICE_DISCOVERY.md` §5 + `spec/UI.md` §2.1.
- [ ] **`test_picker_row_text_glyphs`** passes: `Capable{..}` ⇒ row starts with `✓` + contains `qmk_notifier`; `NotQmkNotifier` ⇒ `✗` + `QMK board, no module`.
- [ ] `cargo build --bin qmkonnect` clean (Windows). `cargo test --bin qmkonnect -- --test-threads=1` green (existing `test_device_status_text_three_states` + the new test). `git status` = `src/tray.rs` only.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can implement this using only this PRP, because: (a) the exact current `DIALOG_RESULT`/save-path/`create_dialog_controls`/`settings_dialog_proc` code (verbatim, with line numbers) is in `research/tray_dialog_verification.md`; (b) the exact Win32 LISTBOX + group-box API (styles, `LB_*` messages with wParam/lParam, `LB_ERR` sentinel, `WM_COMMAND` decode, style-combination cast pattern at `tray.rs:1727`) is in `research/win32_listbox_research.md`; (c) the `core::notifier` consumer API (`classify_devices`/`classification_cache_clear`/`DeviceKind`/`ClassifiedDevice`) is verified in-tree with signatures; (d) the chosen-first-else-manual save path, the three picker-visibility cases, and the reusable `populate_device_picker` design are fully specified below; (e) the `windows` 0.52 style-combination gotcha (`WINDOW_STYLE(WS_CHILD.0 | … | LBS_NOTIFY as u32)`) is pinned; (f) 12 gotchas are pinned (G1 Windows-only, G2 control-ID ranges, G3 LB_ERR cast, G4 group-box z-order + no WM_COMMAND, G5 style-combination cast, G6 SendMessageW signatures, G7 Rescan blocks UI, G8 to_wide_string + PCWSTR, G9 PICKER_DEVICES mirrors WINDOW_INFO_ROWS, G10 chosen vs manual types, G11 single-dialog static, G12 no macOS/Linux touch).

### Documentation & References

```yaml
# MUST READ — the spec source of truth (the §5 picker UX + §5.3 Windows rendering)
- url: spec/DEVICE_DISCOVERY.md
  why: "§5.1 (the 3 picker cases: clean-auto ⇒ static 'Detected: <name>' + no
        picker; ≥2 Tier-1 boards ⇒ picker; no capable board + ≥1 Tier-1 ⇒ picker
        with ✗); §5.2 (Advanced = the existing two hex fields relocated); §5.3
        (Windows: 'Win32 LISTBOX in the QMKSettingsDialog; VID/PID fields under
        a Advanced ▸ group box'). §3 (the ✓/✗ + 'qmk_notifier'/'QMK board, no
        module' row semantics)."
  section: "## 5. The Discovered-Device Picker (Settings UX) (§5.1-§5.3)"

# MUST READ — the UI spec (the Win32 dialog contract this task extends)
- url: spec/UI.md
  why: "§2.0 (the picker as new primary surface; Advanced disclosure; the
        shared DIALOG_RESULT becomes struct { chosen, manual }); §2.1 (the
        QMKSettingsDialog window class, control IDs 1001-1004, settings_dialog_proc,
        parse_id_field, save via render_config_body — the EXISTING contract this
        task extends); §2.4 (parse_id_field: empty/auto ⇒ None)."
  section: "## 2. Settings Dialogs (§2.0, §2.1, §2.4)"

# MUST READ — the codebase verification (THIS task's exact edit sites, verbatim)
- file: plan/005_8b95ea464bd9/P3M2T1S1/research/tray_dialog_verification.md
  why: "§1 the DIALOG_RESULT static (53-55) + every read/write site (796 reset,
        892/894 read+destructure, 1081 OK-arm write). §2 the save-path overlay
        (891-907, verbatim). §3 create_dialog_controls (917-1041) — all 6 controls
        with exact x/y/w/h + the SetDlgItemTextW prefill. §4 settings_dialog_proc
        (1048-1104) — the control_id decode (1061) + OK/Cancel arms verbatim.
        §5 the dialog window 400×200 (827-849). §6 WINDOW_INFO_ROWS (80-87) as the
        PICKER_DEVICES pattern + the SendMessageW/WM_SETFONT shape (1635-1648) +
        GetDlgItem/MoveWindow (1796-1804). §7 the in-tree notifier API signatures.
        §8 to_wide_string (1114-1121)."

# MUST READ — the Win32 API research (styles, messages, gotchas)
- file: plan/005_8b95ea464bd9/P3M2T1S1/research/win32_listbox_research.md
  why: "§1 LISTBOX styles (LBS_NOTIFY|LBS_HASSTRINGS|LBS_NOINTEGRALHEIGHT|WS_VSCROLL).
        §2 the LB_* message table (wParam/lParam/returns) + LB_ERR=-1 sentinel.
        §3 WM_COMMAND LOWORD/HIWORD decode. §4 BS_GROUPBOX (purely visual; create
        before children; never in WM_COMMAND). §5 space-padded strings over
        LB_SETTABSTOPS. §7 the style-combination cast pattern
        WINDOW_STYLE(WS_CHILD.0 | … | LBS_NOTIFY as u32) — verified at tray.rs:1727."

# MUST READ — the file THIS task edits (every line referenced confirmed by reading)
- file: src/tray.rs
  why: "DIALOG_RESULT @53-55 (the static to extend). show_settings_dialog @779
        (the fn header + the reset @796 + dialog create @827-849 + create_dialog_controls
        call @853 + message loop @885-889 + result read @892 + save @894-907).
        create_dialog_controls @917 (signature + the 6 CreateWindowExW + prefill).
        settings_dialog_proc @1048 (signature + WM_COMMAND @1061 + OK @1063 +
        Cancel @1097). to_wide_string @1114. parse_id_field @70 (shared; empty/
        auto ⇒ Ok(None)). WINDOW_INFO_ROWS @80 (the Mutex<Vec> pattern).
        create_window_info_rows / wininfo_move_ctl @1796 (GetDlgItem+MoveWindow)
        + set_font closure @1635 (SendMessageW+WM_SETFONT+WPARAM+LPARAM shape)."
  pattern: "CreateWindowExW(WINDOW_EX_STYLE, class, title, style, x,y,w,h, hwnd,
            HMENU(id), h_instance, Some(ptr::null())). Wide strings via
            to_wide_string(&format!(...)) then PCWSTR(vec.as_ptr()); literals via w!()."
  gotcha: "G3: SendMessageW LB_GETCURSEL returns LRESULT(isize) — cast .0 as i32,
           compare != LB_ERR (-1), NOT != 0. G5: LBS_*/BS_GROUPBOX are i32; combine
           with WS_* via WINDOW_STYLE(WS_CHILD.0 | … | LBS_NOTIFY as u32) (tray.rs:1727)."

# MUST READ — the consumer API (the classification functions this task calls)
- file: src/core/notifier.rs
  why: "classify_devices(verbose: bool) -> Vec<ClassifiedDevice> @1116 (enumerate
        Tier-1 + per-candidate QUERY_INFO + cache; verbose=true for diagnostic
        eprintln). classification_cache_clear() @917 (drains CLASSIFICATION_CACHE —
        call BEFORE re-classify on Rescan). pub enum DeviceKind { Capable{
        proto_ver,feature_flags,callback_count,board_rules_present}, NotQmkNotifier }
        @816 (the ✓/✗ discriminator). pub struct ClassifiedDevice { path, vendor_id:u16,
        product_id:u16, product_name:Option<String>, usage_page, usage, kind } @841
        (vendor_id/product_id are u16 — always Some — so a pick yields a concrete (u16,u16))."
  pattern: "All four are #[allow(dead_code)] today (no consumer yet); this task is
            the consumer. Call as crate::core::notifier::classify_devices(true) etc."

# Reference — the windows crate (confirmed v0.52.0, feature Win32_UI_WindowsAndMessaging
#   + Win32_UI_Controls + Win32_Foundation already in Cargo.toml)
- url: https://learn.microsoft.com/en-us/windows/win32/controls/lb-getcursel
  why: "LB_GETCURSEL returns the 0-based index of the currently selected item, or
        LB_ERR (-1) if none is selected. This is the single message the OK arm
        needs to read the disambiguation."
- url: https://learn.microsoft.com/en-us/windows/win32/controls/lb-addstring
  why: "LB_ADDSTRING: wParam=0, lParam=lpcwstr (the wide string). Returns the index."
- url: https://learn.microsoft.com/en-us/windows/win32/controls/button-styles
  why: "BS_GROUPBOX (=0x0007): a frame + title; the user CANNOT interact with it;
        it never sends BN_CLICKED. Purely visual grouping."
```

### Current Codebase tree (relevant subset)

```bash
src/
  tray.rs             # macOS + Windows tray + dialogs (2431 lines).
                        # DIALOG_RESULT @53; show_settings_dialog @779;
                        # create_dialog_controls @917; settings_dialog_proc @1048;
                        # to_wide_string @1114; show_macos_settings_dialog @1206;
                        # show_window_info_dialog @1517 + window_info_dialog_proc @1912;
                        # test_device_status_text_three_states @2413.
                        # <-- THIS TASK: Windows-only restructure of show_settings_dialog
                        #     + create_dialog_controls + settings_dialog_proc + DIALOG_RESULT.
  core/
    notifier.rs       # classify_devices @1116; classification_cache_clear @917;
                        # DeviceKind @816; ClassifiedDevice @841 (CONSUMED, not edited)
    mod.rs            # Config @24 (vendor_id/product_id: Option<u16>);
                        # render_config_body @255; atomic_write (CONSUMED, not edited)
spec/
  DEVICE_DISCOVERY.md # §5 = the picker UX source of truth (READ-ONLY)
  UI.md               # §2.0/§2.1/§2.4 = the Win32 dialog contract (READ-ONLY)
Cargo.toml            # windows 0.52.0 (Win32_UI_WindowsAndMessaging + Controls + Foundation) — UNCHANGED
```

### Desired Codebase tree (files this task changes)

```bash
src/
  tray.rs             # MODIFIED (Windows-only, additive):
                        #  + struct DialogResult + extend DIALOG_RESULT
                        #  + static PICKER_DEVICES + static DIALOG_OPEN_VIDPID
                        #  + control IDs 1010/1011/1012/1013 (const block)
                        #  + picker_row_text + populate_device_picker
                        #  + rewrite create_dialog_controls (taller dialog + new controls)
                        #  + extend settings_dialog_proc (OK arm reads listbox; +1011 Rescan arm)
                        #  + update save path (chosen-first-else-manual)
                        #  + test_picker_row_text_glyphs
    # EVERYTHING else unchanged (Cargo.toml, core/*, linux_tray.rs, macOS dialogs, spec/*)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (G1 — Windows-only): ALL new code is #[cfg(target_os = "windows")]. The
//   macOS dialogs (show_macos_settings_dialog @1206) and the shared parse_id_field @70
//   (gated windows+macos) are UNTOUCHED. On Linux/macOS the crate must still build —
//   verify by NOT adding any non-Windows-gated item.
//
// CRITICAL (G2 — control ID ranges): the existing settings dialog uses 1001-1004
//   (VID EDIT, PID EDIT, OK, Cancel). The window-info dialog uses 4001-4013 +
//   5000+ (labels) + 6000+ (copy buttons) — see the comment block at tray.rs:1468-1477.
//   NEW IDs MUST avoid both ranges. This task uses 1010 (LISTBOX), 1011 (Rescan),
//   1012 (group box), 1013 (header static) — safely between 1004 and 4001.
//
// CRITICAL (G3 — LB_ERR sentinel): SendMessageW returns LRESULT(isize). LB_GETCURSEL
//   returns the selected index OR LB_ERR (-1). You MUST cast `.0 as i32` and compare
//   `!= LB_ERR && >= 0`. Comparing `!= 0` is a BUG (index 0 is a valid selection).
//
// CRITICAL (G4 — group box is visual only): BS_GROUPBOX never sends BN_CLICKED and
//   must NEVER be branched on in the WM_COMMAND match. Create it BEFORE the controls
//   it visually contains (VID/PID labels + edits) so those children are higher in
//   z-order and paint on top of the frame.
//
// CRITICAL (G5 — style combination cast): WS_CHILD/WS_VISIBLE/WS_TABSTOP/WS_VSCROLL
//   are WINDOW_STYLE(u32) newtypes (BitOr-able); BS_GROUPBOX/LBS_NOTIFY/LBS_HASSTRINGS/
//   LBS_NOINTEGRALHEIGHT are raw i32 consts and do NOT impl BitOr<WINDOW_STYLE>.
//   The codebase pattern (tray.rs:1727) is to cast `as u32`, OR the raw values, wrap:
//     WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | WS_VSCROLL.0
//                  | LBS_NOTIFY as u32 | LBS_HASSTRINGS as u32 | LBS_NOINTEGRALHEIGHT as u32)
//   For the group box: WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | BS_GROUPBOX as u32).
//
// CRITICAL (G6 — SendMessageW signatures in windows 0.52): SendMessageW(HWND, u32 msg,
//   WPARAM, LPARAM) -> LRESULT. For LB_ADDSTRING the lParam is the wide-string POINTER:
//   LPARAM(text.as_ptr() as isize) where text: Vec<u16> from to_wide_string (NUL-terminated).
//   For LB_GETCURSEL/LB_RESETCONTENT both WPARAM(0)/LPARAM(0). Import WPARAM, LPARAM
//   from windows::Win32::Foundation. LB_* constants are u32 (from WindowsAndMessaging).
//
// GOTCHA (G7 — [Rescan] blocks the UI thread briefly): classify_devices does HID I/O
//   (synchronous hidapi enumerate + per-candidate QUERY_INFO ping). The settings
//   message loop runs on the tray thread; a [Rescan] click runs classify_devices
//   inline and freezes the dialog for the duration. The cache is warm from the
//   handshake (0 pings on a freshly-connected board), so this is usually <50ms; a
//   cold classify with N boards is ~N × (read timeout). Acceptable for v1 (the spec
//   does not require a background worker). Do NOT spawn a thread — the proc must
//   repopulate synchronously so the listbox updates before the call returns. NOTE
//   this in a comment.
//
// GOTCHA (G8 — wide strings): to_wide_string (tray.rs:1114) returns Vec<u16> WITH a
//   NUL terminator. Pass as windows::core::PCWSTR(vec.as_ptr()). For compile-time
//   literals use windows::core::w!("..."). The listbox row text is runtime-built
//   (format!) so it MUST use to_wide_string (w! won't work on a non-literal).
//
// GOTCHA (G9 — PICKER_DEVICES mirrors WINDOW_INFO_ROWS): the Win32 dialog proc is a
//   free `extern "system" fn` with NO way to carry per-call user data except a static.
//   WINDOW_INFO_ROWS @80 (a Mutex<Vec<(String,String)>>) is the established pattern;
//   PICKER_DEVICES: Mutex<Vec<ClassifiedDevice>> mirrors it exactly. Only one settings
//   dialog is open at a time, so a single shared slot is sufficient (same assumption as
//   DIALOG_RESULT + WINDOW_INFO_ROWS).
//
// CRITICAL (G10 — chosen vs manual types): ClassifiedDevice.vendor_id/product_id are
//   u16 (always present), so a listbox pick yields a CONCRETE (u16,u16) — that's why
//   `chosen: Option<(u16,u16)>` differs in type from `manual: Option<(Option<u16>,Option<u16>)>`
//   (the typed hex fields, where blank ⇒ None). In the save path, lift chosen to Option:
//   `merged.vendor_id = Some(v); merged.product_id = Some(p);`. Do NOT confuse the two.
//
// GOTCHA (G11 — the proc needs the open-time config for the clean-auto check): the
//   [Rescan] arm re-evaluates the 3 picker cases, which depend on the open-time config's
//   vid/pid (NOT the live config — the user is mid-edit). The proc has no config_path, so
//   store the open-time (vendor_id, product_id) in DIALOG_OPEN_VIDPID (set in
//   show_settings_dialog before create_dialog_controls). classify_devices reads config
//   internally via configured_filter; do NOT re-read config in the proc.
//
// CRITICAL (G12 — do NOT touch macOS/Linux): the macOS dialog (show_macos_settings_dialog
//   @1206) is a SEPARATE sibling task (P3.M2.T1.S2). The shared parse_id_field @70 is
//   gated windows+macos — leave it. Everything new is #[cfg(target_os = "windows")].
//
// CRATE QUIRK: cargo test --bin qmkonnect -- --test-threads=1 (AGENTS.md; shared
//   MockNotifier globals + DebounceState). The Win32 dialog itself is NOT unit-testable
//   (it spawns a real Win32 message loop); only the pure picker_row_text builder is.
```

## Implementation Blueprint

### Data models and structure

```rust
// ── (1) the extended result slot (replaces the (Option<u16>,Option<u16>) tuple) ──
/// Result of the Windows settings dialog. `chosen` is the listbox selection (a
/// concrete `(vid,pid)` from `ClassifiedDevice`); `manual` is the typed hex pair
/// from the Advanced fields (each `None` ⇒ auto-discovery). The save path applies
/// `chosen` first, else `manual`, else leaves VID/PID as-is (spec/UI.md §2.0).
#[cfg(target_os = "windows")]
#[derive(Clone, Default)]
struct DialogResult {
    chosen: Option<(u16, u16)>,
    manual: Option<(Option<u16>, Option<u16>)>,
}

#[cfg(target_os = "windows")]
static DIALOG_RESULT: std::sync::Mutex<Option<DialogResult>> = std::sync::Mutex::new(None);

// ── (2) the listbox row store (mirrors WINDOW_INFO_ROWS @80) ──
// Populated by populate_device_picker (initial open + [Rescan]); read by the OK
// arm (1003) to map a selected listbox index → (vendor_id, product_id). Only one
// settings dialog is open at a time, so a single shared slot is sufficient.
#[cfg(target_os = "windows")]
static PICKER_DEVICES: std::sync::Mutex<Vec<crate::core::notifier::ClassifiedDevice>> =
    std::sync::Mutex::new(Vec::new());

// ── (3) the dialog-open config's vid/pid (for the [Rescan] arm) ──
// The proc has no config_path; it re-evaluates the clean-auto case from this.
#[cfg(target_os = "windows")]
static DIALOG_OPEN_VIDPID: std::sync::Mutex<(Option<u16>, Option<u16>)> =
    std::sync::Mutex::new((None, None));

// ── (4) new control IDs (avoid 1001-1004 + window-info's 4001+) ──
#[cfg(target_os = "windows")]
const IDC_DEVICE_LIST: i32 = 1010; // LISTBOX (the picker)
#[cfg(target_os = "windows")]
const IDC_RESCAN: i32 = 1011;      // [Rescan] BUTTON
#[cfg(target_os = "windows")]
const IDC_ADVANCED_GROUP: i32 = 1012; // BS_GROUPBOX (visual frame, never in WM_COMMAND)
#[cfg(target_os = "windows")]
const IDC_HEADER: i32 = 1013;      // WC_STATICW (the Detected:/Detected:<name> line)
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: ADD the data model + statics (just above the DIALOG_RESULT line @53)
  - DO: add struct DialogResult + extend DIALOG_RESULT's inner type to Option<DialogResult>;
        add static PICKER_DEVICES + static DIALOG_OPEN_VIDPID + the 4 const control IDs
        (all #[cfg(target_os = "windows")]).
  - GOTCHA G10: chosen is (u16,u16); manual is (Option<u16>,Option<u16>). Default derives
        None for both. G2: IDs 1010-1013 avoid the existing ranges.
  - NOTE: this WILL NOT COMPILE until Task 5 updates the write site (@1081) + the read
        destructure (@894) to the new type — EXPECTED mid-refactor. Fix in Task 4/5.

Task 2: ADD picker_row_text (the pure row-string builder — test target)
  - DO: add (Windows-gated):
        fn picker_row_text(d: &crate::core::notifier::ClassifiedDevice) -> String {
            use crate::core::notifier::DeviceKind;
            let (glyph, status) = match d.kind {
                DeviceKind::Capable { .. } => ("\u{2713}", "qmk_notifier"),        // ✓
                DeviceKind::NotQmkNotifier => ("\u{2717}", "QMK board, no module"), // ✗
            };
            let name = d.product_name.as_deref().unwrap_or("(unnamed)");
            format!("{}  {:<22} 0x{:04X}:0x{:04X}  {}", glyph, name, d.vendor_id, d.product_id, status)
        }
  - GLYPHS: \u{2713} ✓ / \u{2717} ✗ — match spec §5.1/§3 exactly. Space-padded (G5 of
        win32 research: simpler than LB_SETTABSTOPS).

Task 3: ADD populate_device_picker (reusable by initial open + [Rescan])
  - DO: add (Windows-gated):
        fn populate_device_picker(
            hwnd: windows::Win32::Foundation::HWND,
            devices: &[crate::core::notifier::ClassifiedDevice],
            open_vid: Option<u16>,
            open_pid: Option<u16>,
        ) {
            use windows::Win32::UI::WindowsAndMessaging::{
                GetDlgItem, SendMessageW, ShowWindow, LB_ADDSTRING, LB_RESETCONTENT,
                SW_HIDE, SW_SHOW, WPARAM, LPARAM,
            };
            *PICKER_DEVICES.lock().unwrap() = devices.to_vec();
            let lb = GetDlgItem(hwnd, IDC_DEVICE_LIST);
            let _ = SendMessageW(lb, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
            for d in devices {
                let text = to_wide_string(&picker_row_text(d));
                let _ = SendMessageW(lb, LB_ADDSTRING, WPARAM(0), LPARAM(text.as_ptr() as isize));
            }
            // The 3 picker cases (spec/DEVICE_DISCOVERY.md §5.1):
            use crate::core::notifier::DeviceKind;
            let clean_auto = devices.len() == 1
                && matches!(devices[0].kind, DeviceKind::Capable { .. })
                && open_vid.is_none() && open_pid.is_none();
            let (hdr, show_picker) = if devices.is_empty() {
                ("No QMK keyboards detected. Enter IDs manually below.", false)
            } else if clean_auto {
                // The zero-config case: hide the list, show "Detected: <name>".
                (devices[0].product_name.as_deref().unwrap_or("(unnamed)"), false)
                    // header set to "Detected: <name>" below (runtime format)
            } else {
                ("Detected keyboard(s) — choose one:", true)
            };
            // set header (runtime text ⇒ to_wide_string + SetDlgItemTextW)
            // toggle listbox + rescan visibility via ShowWindow(GetDlgItem(...), SW_HIDE/SW_SHOW)
        }
  - FOLLOW: the GetDlgItem + SendMessageW shape from wininfo_move_ctl @1796-1804 + the
        set_font closure @1635-1648 (WPARAM/LPARAM construction).
  - GOTCHA G3: SendMessageW returns LRESULT; for LB_RESETCONTENT/LB_ADDSTRING the return
        is ignored (LB_ADDSTRING returns the index but we don't need it).
  - DETAIL: for the clean-auto case the header is `format!("Detected: {}", name)` — build
        via to_wide_string then SetDlgItemTextW(hwnd, IDC_HEADER, PCWSTR(ptr)). For the
        other cases the header is a literal ⇒ to_wide_string(hdr). Always set the header
        (it always exists). Show/hide BOTH IDC_DEVICE_LIST and IDC_RESCAN together.

Task 4: REWRITE create_dialog_controls (new controls + relocated EDITs)
  - DO: rewrite the body of create_dialog_controls (@917) to:
        (a) bump nothing here (the dialog dimensions are in show_settings_dialog @827-828 —
            change those in Task 6). 
        (b) create controls in this ORDER (z-order matters — G4 group box before its children):
            1. header static (IDC_HEADER=1013): WC_STATICW, WS_CHILD|WS_VISIBLE, x=16,y=14,w=388,h=18.
            2. LISTBOX (IDC_DEVICE_LIST=1010): WC_LISTBOX, WS_EX_CLIENTEDGE,
               WINDOW_STYLE(WS_CHILD.0|WS_VISIBLE.0|WS_TABSTOP.0|WS_VSCROLL.0|LBS_NOTIFY as u32
               |LBS_HASSTRINGS as u32|LBS_NOINTEGRALHEIGHT as u32), x=16,y=36,w=388,h=110.
            3. [Rescan] button (IDC_RESCAN=1011): WC_BUTTONW, WS_CHILD|WS_VISIBLE|WS_TABSTOP,
               x=314,y=152,w=90,h=26.
            4. Advanced group box (IDC_ADVANCED_GROUP=1012): WC_BUTTONW,
               WINDOW_STYLE(WS_CHILD.0|WS_VISIBLE.0|BS_GROUPBOX as u32), x=14,y=188,w=392,h=120.
            5. VID label (no id): WC_STATICW, w!("Vendor ID (hex):"), x=30,y=218,w=130,h=20.
            6. VID EDIT (1001): WC_EDITW, WS_EX_CLIENTEDGE, WS_CHILD|WS_VISIBLE|WS_TABSTOP,
               x=170,y=216,w=110,h=24.  ← relocated under the group box; SAME ID 1001.
            7. PID label (no id): WC_STATICW, w!("Product ID (hex):"), x=30,y=250,w=130,h=20.
            8. PID EDIT (1002): WC_EDITW, ..., x=170,y=248,w=110,h=24.  ← SAME ID 1002.
            9. OK (1003): WC_BUTTONW, w!("OK"), x=230,y=324,w=80,h=30.  ← SAME ID, relocated.
            10. Cancel (1004): WC_BUTTONW, w!("Cancel"), x=318,y=324,w=80,h=30. ← SAME ID, relocated.
        (c) SetDlgItemTextW prefill 1001/1002 — UNCHANGED (the existing block @1025-1041).
        (d) classify + populate:
            let devices = crate::core::notifier::classify_devices(true);
            populate_device_picker(hwnd, &devices, config.vendor_id, config.product_id);
  - IMPORTS: add to the `use windows::Win32::UI::Controls::{...}` line: WC_LISTBOX.
        Add to the `use windows::Win32::UI::WindowsAndMessaging::{...}` line: BS_GROUPBOX,
        LBS_HASSTRINGS, LBS_NOINTEGRALHEIGHT, LBS_NOTIFY, WS_VSCROLL (the WS_* are already
        imported; WINDOW_STYLE too). WINDOW_STYLE is in WindowsAndMessaging.
  - GOTCHA G5: use WINDOW_STYLE(WS_CHILD.0 | ... | LBS_NOTIFY as u32) — the cast pattern.
        G4: group box (step 4) created BEFORE VID/PID labels+edits (steps 5-8). G8: row text
        not needed here (populate_device_picker builds it). The EDIT IDs 1001/1002 are KEPT
        (the OK arm + prefill still reference them).
  - DOC-COMMENT (Mode A): cite spec/DEVICE_DISCOVERY.md §5 + spec/UI.md §2.1.

Task 5: EXTEND settings_dialog_proc (OK arm reads listbox; + [Rescan] arm)
  - DO: in settings_dialog_proc (@1048):
        - add to the `use` line @1054: GetDlgItem, SendMessageW, LB_GETCURSEL, LB_ERR.
        - in the 1003 (OK) arm, BEFORE the existing GetDlgItemTextW/parse_id_field block,
          read the listbox selection → chosen:
            let chosen = {
                let lb = GetDlgItem(hwnd, IDC_DEVICE_LIST);
                let sel = SendMessageW(lb, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0 as i32;
                if sel != LB_ERR && sel >= 0 {
                    PICKER_DEVICES.lock().unwrap().get(sel as usize)
                        .map(|d| (d.vendor_id, d.product_id))
                } else { None }
            };
          then (unchanged) read 1001/1002 → parse_id_field each → (vendor_id, product_id);
          on success store: *DIALOG_RESULT.lock().unwrap() = Some(DialogResult { chosen, manual: Some((vendor_id, product_id)) });
          on parse error ⇒ MessageBoxW (UNCHANGED error block).
        - add a new arm:
            IDC_RESCAN => {
                // [Rescan]: clear the classification cache + re-classify + repopulate.
                // NOTE (G7): runs HID I/O inline on the tray thread; the cache is warm
                // from the handshake so usually <50ms; cold classify is ~N×(read timeout).
                crate::core::notifier::classification_cache_clear();
                let devices = crate::core::notifier::classify_devices(true);
                let (vid, pid) = *DIALOG_OPEN_VIDPID.lock().unwrap();
                populate_device_picker(hwnd, &devices, vid, pid);
            }
  - GOTCHA G3: `sel != LB_ERR && sel >= 0` (LB_ERR=-1; index 0 is valid). G6: SendMessageW
        with WPARAM(0)/LPARAM(0) for LB_GETCURSEL. G11: the [Rescan] arm reads the open-time
        vid/pid from DIALOG_OPEN_VIDPID (NOT the live config).

Task 6: UPDATE the save path + dialog dimensions in show_settings_dialog (@779)
  - DO: 
        (a) bump dimensions (@827-828):
            let dialog_width = 420;
            let dialog_height = 380;
          (the centering math @829-830 derives from these — no other edit).
        (b) store the open-time vid/pid BEFORE create_dialog_controls (@853):
            *DIALOG_OPEN_VIDPID.lock().unwrap() = (current_config.vendor_id, current_config.product_id);
        (c) UPDATE the result read + save (@892-907):
            let result = DIALOG_RESULT.lock().unwrap().take();
            if let Some(dr) = result {
                let mut merged = current_config;
                if let Some((v, p)) = dr.chosen {
                    merged.vendor_id = Some(v);     // G10: lift concrete u16 to Option
                    merged.product_id = Some(p);
                } else if let Some((v, p)) = dr.manual {
                    merged.vendor_id = v;
                    merged.product_id = p;
                }
                let config_content = crate::core::render_config_body(&merged);
                crate::core::atomic_write(config_path, &config_content)?;
            }
          (render_config_body + atomic_write + the comment block are UNCHANGED.)
  - PRECEDENCE: chosen-first-else-manual (spec/UI.md §2.0). When the user clicked OK
        with no listbox selection, chosen=None ⇒ manual applies (the typed fields, which
        were prefilled with the open-time hex). When both listbox + fields present, chosen
        wins (the disambiguation). dr is always Some on OK (Cancel never sets it).

Task 7: ADD test_picker_row_text_glyphs (pure builder — the only unit-testable piece)
  - DO: in the existing `#[cfg(all(test, any(target_os="macos", target_os="windows")))]
        mod tests` (@2411), add (Windows-only inside, but the fn is gated windows; guard
        the test with #[cfg(target_os="windows")] or place it appropriately):
        #[cfg(target_os = "windows")]
        #[test]
        fn test_picker_row_text_glyphs() {
            use crate::core::notifier::{ClassifiedDevice, DeviceKind};
            let capable = ClassifiedDevice {
                path: String::new(), vendor_id: 0xFEED, product_id: 0x0000,
                product_name: Some("Dactyl".into()), usage_page: 0xFF60, usage: 0x61,
                kind: DeviceKind::Capable { proto_ver: 2, feature_flags: 1, callback_count: 0, board_rules_present: false },
            };
            let notqmk = ClassifiedDevice { kind: DeviceKind::NotQmkNotifier,
                vendor_id: 0x3434, product_id: 0x0123, product_name: Some("Keychron".into()),
                ..capable.clone() };
            let cap_row = picker_row_text(&capable);
            let nq_row = picker_row_text(&notqmk);
            assert!(cap_row.starts_with("\u{2713}"), "capable row starts with ✓: {cap_row}");
            assert!(cap_row.contains("0xFEED:0x0000") && cap_row.contains("qmk_notifier"));
            assert!(nq_row.starts_with("\u{2717}"), "notqmk row starts with ✗: {nq_row}");
            assert!(nq_row.contains("0x3434:0x0123") && nq_row.contains("QMK board, no module"));
        }
  - NOTE: ClassifiedDevice must derive Clone (it does — notifier.rs:842). DeviceKind derives
        PartialEq/Clone (notifier.rs:817). The `..capable.clone()` spread needs Clone.

Task 8: VERIFY build + tests (single-threaded, per AGENTS.md)
  - RUN: cargo build --bin qmkonnect   (Windows: clean; Linux/macOS: clean — all new code is
         Windows-gated, so a non-Windows build must still pass).
  - RUN: cargo test --bin qmkonnect -- --test-threads=1   (existing test_device_status_text_three_states
         + the new test_picker_row_text_glyphs pass; the Win32 dialog functions are #[cfg] so
         the test compiles + runs on Windows; on non-Windows the test is cfg'd out).
  - CONFIRM git status shows EXACTLY one file: src/tray.rs.
  - MANUAL (Windows only, per AGENTS.md dev loop): cargo build --release; taskkill /IM
         qmkonnect.exe /F; run the exe; menu → Settings…; verify the 3 picker cases against
         real hardware (≥2 boards, 1 capable, 0 boards); verify [Rescan]; verify Advanced
         manual entry still writes config.toml.
```

### Implementation Patterns & Key Details

```rust
// The LISTBOX CreateWindowExW (Task 4, step 2) — the style-cast pattern (G5):
// CreateWindowExW(
//     windows::Win32::UI::WindowsAndMessaging::WS_EX_CLIENTEDGE,
//     WC_LISTBOX,                                   // from windows::Win32::UI::Controls
//     windows::core::PCWSTR::null(),
//     windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
//         WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | WS_VSCROLL.0
//         | LBS_NOTIFY as u32 | LBS_HASSTRINGS as u32 | LBS_NOINTEGRALHEIGHT as u32,
//     ),
//     16, 36, 388, 110,
//     hwnd,
//     windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_DEVICE_LIST as isize),
//     h_instance,
//     Some(ptr::null()),
// )?;

// The OK-arm selection read (Task 5) — G3 LB_ERR sentinel:
// let lb = GetDlgItem(hwnd, IDC_DEVICE_LIST);
// let sel = SendMessageW(lb, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0 as i32;
// let chosen = if sel != LB_ERR && sel >= 0 {           // NOT != 0 (index 0 valid)
//     PICKER_DEVICES.lock().unwrap().get(sel as usize)
//         .map(|d| (d.vendor_id, d.product_id))
// } else { None };

// The LB_ADDSTRING lParam (Task 3) — G6 wide-string pointer:
// let text = to_wide_string(&picker_row_text(d));      // Vec<u16>, NUL-terminated
// let _ = SendMessageW(lb, LB_ADDSTRING, WPARAM(0), LPARAM(text.as_ptr() as isize));

// The group box (Task 4, step 4) — G4 BS_GROUPBOX, no WM_COMMAND handling:
// CreateWindowExW(
//     WINDOW_EX_STYLE(0), WC_BUTTONW, windows::core::w!("Advanced / manual override"),
//     WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | BS_GROUPBOX as u32),
//     14, 188, 392, 120, hwnd,
//     HMENU(IDC_ADVANCED_GROUP as isize), h_instance, Some(ptr::null()),
// )?;
// NOTE: created BEFORE the VID/PID labels+edits (steps 5-8) for z-order; never branched on.

// NOTE on HMENU: the existing code uses HMENU(1001) etc. (tray.rs:956). HMENU wraps isize.
// The new consts are i32 — cast `as isize` (HMENU(IDC_DEVICE_LIST as isize)). Or keep the
// literal HMENU(1010) inline if you prefer (the consts are for readability).
```

### Integration Points

```yaml
CODE (this task):
  - file: src/tray.rs
    change: "Windows-only additive — struct DialogResult + extended DIALOG_RESULT + 2 new
             statics + 4 control IDs + picker_row_text + populate_device_picker + rewritten
             create_dialog_controls + extended settings_dialog_proc + updated save path + 1 test"
    pattern: "CreateWindowExW control creation mirrors the existing 6 controls @929-1023;
              PICKER_DEVICES mirrors WINDOW_INFO_ROWS @80; GetDlgItem+SendMessageW mirrors
              wininfo_move_ctl @1796 + set_font @1635; the LB_ERR sentinel + style-cast
              pattern verified in research/win32_listbox_research.md."

DEPENDENCIES (this task): NONE new. windows 0.52.0 (Win32_UI_WindowsAndMessaging +
                           Win32_UI_Controls + Win32_Foundation already in Cargo.toml @71-90).
                           No new `use` crate — WC_LISTBOX/BS_GROUPBOX/LB_*/LBS_* all present.

UPSTREAM (consumed read-only):
  - crate::core::notifier::classify_devices(verbose: bool) -> Vec<ClassifiedDevice> (notifier.rs:1116).
  - crate::core::notifier::classification_cache_clear() (notifier.rs:917).
  - crate::core::notifier::DeviceKind { Capable{..}, NotQmkNotifier } (notifier.rs:816).
  - crate::core::notifier::ClassifiedDevice { vendor_id:u16, product_id:u16, product_name:Option<String>, kind:DeviceKind, .. } (notifier.rs:841).
  - crate::core::Config { vendor_id:Option<u16>, product_id:Option<u16>, .. } (mod.rs:24).
  - crate::core::render_config_body + atomic_write (mod.rs:255+) — UNCHANGED.
  - parse_id_field @70 (shared windows+macos; UNCHANGED).

DOWNSTREAM CONSUMERS (later sibling tasks — do NOT implement them here):
  - P3.M2.T1.S2 (macOS NSAlert picker): follows the SAME DialogResult shape + chosen-first-
    else-manual save path. This task establishes the Windows reference; macOS mirrors it.
  - P3.M2.T1.S3 (Linux zenity picker): same.
  - P4.M2.T1.S1 (Mode-A doc sync): will cite this dialog in README/UI docs.

NO OVERLAP:
  - macOS dialog (show_macos_settings_dialog @1206): P3.M2.T1.S2 — UNTOUCHED.
  - Linux dialog (linux_tray.rs): P3.M2.T1.S3 — UNTOUCHED.
  - classify_devices / cache / DeviceStatus (notifier.rs): P3.M1 / P1 — Complete, read-only.
  - window-info dialog (@1517+): separate IDs (4001+), UNTOUCHED.

CONFIG: none (no config schema change — VID/PID stays Option<u16>). ROUTES: none. DATABASE: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles clean on ALL platforms. On Windows the new dialog code compiles;
# on Linux/macOS the #[cfg(target_os="windows")] items are cfg'd out (no change).
# If it fails: most likely a missing import (WC_LISTBOX/BS_GROUPBOX/LB_*/LBS_*/WINDOW_STYLE),
# a wrong SendMessageW arg type (WPARAM/LPARAM), or a type mismatch on DialogResult
# (chosen (u16,u16) vs manual (Option<u16>,Option<u16>)) — READ the error + fix.

# Confirm the deliverables are present (Windows-gated — grep finds them regardless of host):
grep -n 'struct DialogResult' src/tray.rs                       # expect 1
grep -n 'static PICKER_DEVICES' src/tray.rs                     # expect 1
grep -n 'static DIALOG_OPEN_VIDPID' src/tray.rs                 # expect 1
grep -n 'fn populate_device_picker' src/tray.rs                 # expect 1
grep -n 'fn picker_row_text' src/tray.rs                        # expect 1
grep -n 'IDC_DEVICE_LIST\|IDC_RESCAN\|IDC_ADVANCED_GROUP\|IDC_HEADER' src/tray.rs  # expect 4+
grep -c 'LB_GETCURSEL\|LB_ADDSTRING\|LB_RESETCONTENT' src/tray.rs                # expect >=3
grep -c 'classification_cache_clear\|classify_devices' src/tray.rs               # expect >=2 (proc + populate)
# Confirm DIALOG_RESULT is the new type:
grep -n 'static DIALOG_RESULT' src/tray.rs                      # inner type is Option<DialogResult>
```

### Level 2: Unit Tests (Component Validation — the pure builder)

```bash
cd /home/dustin/projects/qmkonnect
# Single-threaded is MANDATORY crate-wide (shared MockNotifier globals + DebounceState, AGENTS.md).
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL green — the existing test_device_status_text_three_states + the new
# test_picker_row_text_glyphs (Windows-only: asserts ✓/✗ glyphs + vid:pid + status labels).
# The Win32 dialog functions (show_settings_dialog/create_dialog_controls/settings_dialog_proc)
# are #[cfg(target_os="windows")] and spawn a real message loop — NOT unit-testable; they are
# covered by the Level-4 manual check on Windows.

cargo test --bin qmkonnect picker_row -- --test-threads=1   # filter to the new test
```

### Level 3: Cross-platform regression (the new code is Windows-only)

```bash
cd /home/dustin/projects/qmkonnect
# On Linux/macOS (the dev box): confirm the Windows-gated additions don't break the build.
cargo build --bin qmkonnect
# Expected: clean — every new item is #[cfg(target_os="windows")], so a non-Windows host
# compiles the rest unchanged. (picker_row_text/picker_row_text_glyphs are Windows-gated too.)

# Confirm the change surface is exactly one file:
git status --short
# Expected: only src/tray.rs modified. NOTHING in Cargo.toml, core/, linux_tray.rs,
# architecture/, docs/, spec/, packaging/.
git diff --stat
# Expected: 1 file: src/tray.rs.
```

### Level 4: Manual dialog testing (Windows only — per AGENTS.md dev loop)

```bash
# The Win32 dialog is a real GUI; it CANNOT be exercised by a unit test. Verify on Windows:
cargo build --release
taskkill /IM qmkonnect.exe /F          # mandatory (single-instance mutex, AGENTS.md)
.\target\release\qmkonnect.exe         # run in your own session, NOT as a service
# Then: tray menu → Settings… Verify against real hardware:
#  CASE A (≥2 Tier-1 boards, or 1 board + 1 VIA board): the LISTBOX shows one row per device
#         with ✓ (capable) / ✗ (no module) glyphs, product_name, 0xVID:0xPID, status label.
#         Click a row → OK → open config.toml → confirm vendor_id/product_id = that board.
#  CASE B (1 capable board, config has no VID/PID): LISTBOX + [Rescan] HIDDEN; header reads
#         "Detected: <name>". OK leaves config on auto (no VID/PID written).
#  CASE C (0 boards): LISTBOX + [Rescan] HIDDEN; header "No QMK keyboards detected...";
#         the Advanced fields still let manual entry.
#  [Rescan]: click it after (un)plugging/flashing a board → listbox repopulates.
#  Advanced override: clear the listbox selection, type a hex pair in VID/PID, OK → that pair
#         is written (manual applies when chosen is None).
#  Precedence: select a listbox row AND type a different pair → chosen wins (the row's VID/PID).
# Expected: all 3 cases render correctly; selection writes the right VID/PID; [Rescan]
#         refreshes; Advanced still works. The dialog is 420×380, not clipped.
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean on the host platform (Windows: full dialog; Linux/macOS: Windows-gated items cfg'd out).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` green (existing `test_device_status_text_three_states` + new `test_picker_row_text_glyphs`).
- [ ] `git status` shows exactly ONE modified file: `src/tray.rs`.

### Feature Validation (contract fidelity)
- [ ] **DIALOG_RESULT** extended to `Mutex<Option<DialogResult>>` with `chosen: Option<(u16,u16)>` + `manual: Option<(Option<u16>,Option<u16>)>`.
- [ ] **4 new control IDs** 1010/1011/1012/1013 (avoid 1001-1004 + window-info's 4001+).
- [ ] **LISTBOX** (1010) created with `LBS_NOTIFY|LBS_HASSTRINGS|LBS_NOINTEGRALHEIGHT|WS_VSCROLL` + `WS_EX_CLIENTEDGE`; style via `WINDOW_STYLE(WS_CHILD.0 | … | LBS_NOTIFY as u32)`.
- [ ] **Group box** (1012) `BS_GROUPBOX`, created before its child labels/edits, NEVER branched in `WM_COMMAND`.
- [ ] **3 picker cases** in `populate_device_picker`: empty ⇒ header "No QMK…"; clean-auto (1 Capable + no VID/PID) ⇒ header "Detected: <name>", list hidden; else ⇒ LISTBOX shown with rows.
- [ ] **picker_row_text**: ✓ (`\u{2713}`)/✗ (`\u{2717}`) glyphs + name + `0x{:04X}:0x{:04X}` + status label, space-padded.
- [ ] **OK arm** reads listbox selection (`LB_GETCURSEL`, `sel != LB_ERR && >= 0`) → `chosen`; reads 1001/1002 → `manual`; stores `DialogResult{chosen, manual}`; parse error ⇒ `MessageBoxW`.
- [ ] **[Rescan] arm** (1011) calls `classification_cache_clear()` + `classify_devices(true)` + `populate_device_picker` (using `DIALOG_OPEN_VIDPID`).
- [ ] **Save path** applies chosen-first-else-manual; `render_config_body` + `atomic_write` unchanged.
- [ ] **Dialog** bumped to 420×380; EDIT IDs 1001/1002 + button IDs 1003/1004 preserved (relocated, not renumbered).

### Code Quality Validation
- [ ] All new code is `#[cfg(target_os = "windows")]` (G1); macOS/Linux dialogs + shared `parse_id_field` untouched (G12).
- [ ] Style-combination uses the `WINDOW_STYLE(WS_CHILD.0 | … | LBS_NOTIFY as u32)` cast pattern (G5, matches `tray.rs:1727`).
- [ ] `LB_GETCURSEL` result cast `.0 as i32`, compared `!= LB_ERR` (G3, NOT `!= 0`).
- [ ] Group box created before children (G4); never in `WM_COMMAND`.
- [ ] `PICKER_DEVICES` mirrors `WINDOW_INFO_ROWS` (G9); `DIALOG_OPEN_VIDPID` set at open, read in [Rescan] (G11).
- [ ] New test prefixed `test_picker_row_text_` (disjoint from `test_device_status_text_*`).
- [ ] No new Cargo deps; no `unsafe` beyond the existing `extern "system" fn` proc; no runnable Rust doctests (binary-only crate).

### Documentation & Deployment
- [ ] Mode-A doc-comment on `show_settings_dialog` / `create_dialog_controls` cites `spec/DEVICE_DISCOVERY.md` §5 + `spec/UI.md` §2.1.
- [ ] No `docs/*.md` or README changes this task (Mode A — P4.M1/P4.M2 own user-facing docs).

---

## Anti-Patterns to Avoid

- ❌ Do NOT add the LISTBOX/group-box code WITHOUT the `#[cfg(target_os="windows")]` gate.
      Every new item must be Windows-gated or the Linux/macOS build breaks (G1).
- ❌ Do NOT compare `LB_GETCURSEL`'s result to `0`. It returns `LB_ERR` (-1) when nothing is
      selected; index 0 is a VALID selection. Cast `.0 as i32` and check `!= LB_ERR && >= 0` (G3).
- ❌ Do NOT write `WS_CHILD | WS_VISIBLE | LBS_NOTIFY`. `WS_*` are `WINDOW_STYLE(u32)` but
      `LBS_*`/`BS_GROUPBOX` are raw `i32` — they don't impl `BitOr<WINDOW_STYLE>`. Use the
      cast pattern `WINDOW_STYLE(WS_CHILD.0 | … | LBS_NOTIFY as u32)` (G5, matches `tray.rs:1727`).
- ❌ Do NOT create the group box AFTER the VID/PID labels/edits. The group box must be created
      FIRST (lower z-order) so its children paint on top (G4).
- ❌ Do NOT branch on the group box ID (`IDC_ADVANCED_GROUP`) in `WM_COMMAND`. A `BS_GROUPBOX`
      never sends `BN_CLICKED`; handling it is dead code (G4).
- ❌ Do NOT re-read the config file in the [Rescan] arm. The user is mid-edit; the clean-auto
      check must use the dialog-OPEN vid/pid from `DIALOG_OPEN_VIDPID` (G11).
- ❌ Do NOT conflate `chosen: (u16,u16)` with `manual: (Option<u16>,Option<u16>)`. A listbox
      pick yields CONCRETE vid/pid (ClassifiedDevice fields are u16); the typed fields yield
      Options (blank ⇒ None). Lift chosen with `Some(v)`/`Some(p)` in the save path (G10).
- ❌ Do NOT touch the macOS dialog (`show_macos_settings_dialog` @1206), the Linux dialog
      (`linux_tray.rs`), or the shared `parse_id_field` @70. They are sibling tasks / shared
      (G12).
- ❌ Do NOT spawn a background thread for [Rescan]. `classify_devices` runs inline on the tray
      thread; the listbox must repopulate synchronously before the proc returns. The cache is
      warm (≈0 pings) so the freeze is brief; cold classify is ~N×(timeout) — acceptable for v1
      (the spec does not require a worker). NOTE it in a comment (G7).
- ❌ Do NOT use `LBS_USETABSTOPS`/`LB_SETTABSTOPS` for column alignment. Tab stops are in
      dialog-template units (need conversion for a pixel dialog); space-padded `format!` strings
      are simpler and robust for 2-4 rows (research §5).
- ❌ Do NOT renumber the EDIT IDs (1001/1002) or the button IDs (1003/1004). They MOVE under
      the group box but keep their IDs — the OK arm + prefill + Cancel arm reference them.
- ❌ Do NOT edit `PRD.md`, any `tasks.json`, `prd_snapshot.md`, `spec/DEVICE_DISCOVERY.md`,
      `spec/UI.md`, `Cargo.toml`, the crate, or any `plan/` file other than this item's own
      `PRP.md` + `research/`.
- ❌ Do NOT run tests multi-threaded — `cargo test --bin qmkonnect -- --test-threads=1`
      (shared MockNotifier globals + DebounceState, AGENTS.md).

---

## Confidence Score: 9/10

This is a well-bounded **single-file, Windows-only UI** task. The two hard parts are
both verified: (1) the `windows` 0.52 **style-combination cast pattern** —
`WINDOW_STYLE(WS_CHILD.0 | … | LBS_NOTIFY as u32)` — is confirmed both in the
crate source (`WS_CHILD: WINDOW_STYLE(u32)` @6133 vs `BS_GROUPBOX: i32` @4010)
and in this very file's existing usage (`tray.rs:1727`:
`WINDOW_STYLE(ES_READONLY as u32 | ES_AUTOHSCROLL as u32 | ES_NOHIDESEL as u32)`);
and (2) the `LB_ERR` sentinel (`SendMessageW(...).0 as i32 != LB_ERR`) is the
one Win32 trap that bites (comparing `!= 0` would treat row 0 as "no selection").
Every referenced line is confirmed: the `DIALOG_RESULT` static + its 3 sites
(53/796/892-907/1081), `create_dialog_controls` (917-1041) with exact coords,
`settings_dialog_proc` (1048-1104) with the control_id decode + OK/Cancel arms,
`to_wide_string` (1114), `WINDOW_INFO_ROWS` (80) as the `PICKER_DEVICES` pattern,
the `GetDlgItem`+`SendMessageW`+`WPARAM`/`LPARAM` shape (1635-1648, 1796-1804),
and the consumer API (`classify_devices` @1116, `classification_cache_clear`
@917, `DeviceKind` @816, `ClassifiedDevice` @841 — all `pub` + in-tree). The
chosen-first-else-manual save path + the 3 picker-visibility cases + the
reusable `populate_device_picker` are fully specified. The 1-point reservation
is for: (a) **the manual Windows GUI verification** (Level 4) — the Win32 dialog
spawns a real message loop and is NOT unit-testable, so the 3 picker cases +
[Rescan] + Advanced override are only verifiable on a Windows box with real
hardware; the implementer must run the AGENTS.md dev loop; (b) the **pixel
layout** (420×380, the exact y-coordinates) is a best-effort design that may need
1-2 px nudges on a real build (clipping/overlap) — non-blocking, cosmetic; and
(c) the **[Rescan] UI freeze** (G7) on a cold classify with many boards —
acceptable for v1 per the spec, but flagged. All three are low-risk and caught
by the manual check. Scope is cleanly bounded from macOS (S2), Linux (S3),
classify_devices (P3.M1), device_status (P1), CLI (P4), and the write path (P2).
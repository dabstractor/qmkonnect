# PRP — P3.M2.T1.S2: macOS NSAlert picker (NSStackView of rows) + Advanced toggle in src/tray.rs

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This task restructures the **macOS** native
> Settings dialog (`show_settings_dialog_with_pool` in `src/tray.rs:1212`) so that
> a live, self-populating **picker** of discovered devices (`classify_devices`) is
> the primary surface inside the `NSAlert` accessory view, and the two legacy
> VID/PID `NSTextField`s are relocated under an **"Advanced / manual override"
> disclosure** toggled by an `NSButton` checkbox. Selecting a radio row writes
> that board's VID/PID to `config.toml` via the shared `render_config_body`.
> Source of truth: **`spec/DEVICE_DISCOVERY.md` §5** (the Discovered-Device
> Picker) + **`spec/UI.md` §2.0/§2.2** (the picker as new primary surface + the
> macOS NSAlert contract). Windows (`P3.M2.T1.S1`) and Linux (`P3.M2.T1.S3`)
> pickers are separate sibling tasks — this PRP touches **macOS only**.
>
> **⚠ LINE-NUMBER NOTE:** the line numbers in this PRP + `research/*.md` were
> captured at research time and the file is GROWING (it was ~2431 lines when the
> architecture doc was written, ~2780 at research time). The parallel sibling
> **P3.M2.T1.S1 is editing `src/tray.rs` RIGHT NOW**, so every `@NNNN` will have
> shifted by the time you implement. **Navigate by FUNCTION NAME + the verbatim
> code signatures** (quoted in `research/tray_dialog_verification_macos.md` §1/§2),
> not by line number. Always `grep -n 'fn show_settings_dialog_with_pool'` to
> find the current site. The verbatim code blocks are the reliable anchors.
>
> **CONSUMES (read-only, already in tree — verified):** `classify_devices(verbose:
> bool) -> Vec<ClassifiedDevice>` (`notifier.rs:1116`), `pub enum DeviceKind {
> Capable{..}, NotQmkNotifier }` (`notifier.rs:816`), `pub struct ClassifiedDevice
> { path, vendor_id:u16, product_id:u16, product_name:Option<String>,
> usage_page, usage, kind }` (`notifier.rs:841`). All `pub` and currently
> `#[allow(dead_code)]` — **this task is a consumer** (the `allow` attributes
> become satisfied; leave them).
>
> **DOES NOT TOUCH:** the write path (P2 DEFER), `classify_devices`/cache logic
> (P3.M1 — Complete), `device_status()` (P1 — Complete), the Windows dialog
> (`show_settings_dialog` — P3.M2.T1.S1, in flight) or its Windows-gated
> `picker_row_text` (mutually-exclusive by `target_os`), the Linux dialog
> (`linux_tray.rs` — P3.M2.T1.S3), CLI flags (P4), `Cargo.toml`, the crate, or any
> `docs/*.md` (Mode A — P4.M2 owns user docs). Single-file change: `src/tray.rs`.
>
> **INTEGRATION SAFETY WITH S1 (parallel, in flight):** S1 adds
> `#[cfg(target_os = "windows")] fn picker_row_text` +
> `#[cfg(target_os = "windows")] fn test_picker_row_text_glyphs`. This task adds
> `#[cfg(target_os = "macos")] fn picker_row_text` +
> `#[cfg(target_os = "macos")] fn test_picker_row_text_glyphs`. They are
> **mutually exclusive by `target_os`** ⇒ exactly one compiles per platform ⇒
> **no symbol collision, no merge conflict on the function name**. Do NOT widen
> the cfg to `any(macos, windows)` (that would collide with S1's on Windows).

---

## Goal

**Feature Goal**: Restructure `show_settings_dialog_with_pool` (`src/tray.rs:1212`)
so the macOS Settings `NSAlert` shows a **picker** of discovered devices (one
`NSButton` radio per `ClassifiedDevice`, titled with the live `product_name` +
`vid:pid` + ✓/✗ glyph) as the primary surface in the accessory view, with the two
legacy VID/PID `NSTextField`s relocated under an **"Advanced / manual override"
disclosure** toggled by an `NSButton` checkbox. Selecting a radio row becomes the
disambiguation: it writes that board's `vid`/`pid` into `config.toml` via the
shared `render_config_body` renderer. The zero-config case (one capable board, no
VID/PID set) is preserved: no picker is shown and a static `Detected: <name>` line
is shown instead.

**Deliverable** (additive `#[cfg(target_os = "macos")]` edits to `src/tray.rs`):
1. **`picker_row_text(d: &ClassifiedDevice) -> String`** (macOS-gated) — the row
   label: `✓`/`✗` glyph + `product_name` (or `(unnamed)`) + `0xVID:0xPID` + status
   (`qmk_notifier` / `QMK board, no module`).
2. **`mac_toggle_advanced`** (`extern "C" fn(&Object, Sel, *mut Object)`) — the
   Advanced checkbox's action: reads the checkbox `state`, flips `setHidden:` on
   the two fields (read from the `ADVANCED_FIELDS` static).
3. **`static ADVANCED_FIELDS: Mutex<[Option<*mut Object>; 2]>`** — carries the two
   field pointers to the extern toggle fn (mirrors `WINDOW_INFO_ROWS` @80; an
   extern fn cannot capture locals).
4. **`RustMacSettingsTarget`** — a registered Obj-C subclass (`NSObject`) with the
   `toggleAdvanced:` method, created once via `ClassDecl` (mirrors
   `RustWindowInfoCopyTarget` @2228). Instantiated per dialog-open as the Advanced
   button's target.
5. **`show_settings_dialog_with_pool`** rewritten: call `classify_devices(true)`,
   pick one of three layout cases (empty / clean-auto / picker), build the radio
   rows (when applicable) + header label + Advanced checkbox + relocated fields in
   a dynamically-sized container `NSView`, set as the accessory view, `runModal`,
   then on OK compute `chosen` (first `NSOnState` radio) and `manual` (two fields)
   with `chosen`-first-else-`manual`-else-as-is save precedence.
6. **Mode-A doc-comment** on `show_settings_dialog_with_pool` citing
   `spec/DEVICE_DISCOVERY.md` §5 + `spec/UI.md` §2.2, and noting the manual-layout
   choice (vs the spec-named `NSStackView`).
7. **1 unit test** `test_picker_row_text_glyphs` (macOS-gated) verifying the ✓/✗
   glyph + vid:pid + status label format (the NSAlert itself is not unit-testable;
   the pure string builder is).

**Success Definition**:
- `cargo build --bin qmkonnect` clean on macOS (no NEW warnings). On Windows/Linux
  the build is unchanged (all new code is `#[cfg(target_os = "macos")]`).
- The dialog, when `classify_devices` returns ≥2 Tier-1 devices OR 1 non-capable
  board, shows one radio row per device (`✓`/`✗` glyph + name + `0xVID:0xPID` +
  status) with a "Detected keyboard(s) — choose one:" header; selecting a row +
  OK writes that board's `(vid,pid)` to `config.toml`.
- When `classify_devices` returns exactly one `Capable` board AND the open-time
  config has no VID/PID, no rows are shown and the header reads `Detected: <name>`.
- When `classify_devices` returns no devices, no rows are shown and the header
  reads `No QMK keyboards detected…`; the Advanced fields are shown by default.
- The Advanced checkbox hides the two `NSTextField`s by default when capable
  boards exist; checking it reveals them for manual hex entry (empty/`auto` ⇒
  `None` ⇒ auto-discovery). `chosen` takes precedence over `manual`.
- `git status` = `src/tray.rs` only.

## User Persona (if applicable)

**Target User**: a macOS user with one or more QMK keyboards who opens Settings
(menu → Settings…) to confirm which board QMKonnect sees or to disambiguate among
several.

**Use Case**: the user has flashed `qmk_notifier` onto one board and has a second
QMK board (VIA/Vial only). They open Settings → the picker shows both:
`✓ Dactyl 0xfeed:0x0000 — qmk_notifier` and `✗ Keychron 0x3434:0x0123 — QMK board,
no module`. They click the capable radio row → OK → `config.toml` records that
board's VID/PID so notifications narrow to it.

**Pain Points Addressed**: today the macOS dialog is two raw hex fields — the user
must already know their board's VID/PID and type it blind. The picker shows live
`product_name` from the HID descriptor (the device names itself; no curated
database) and makes selection a one-click disambiguation (`spec/UI.md` §2.0).

## Why

- **`DEVICE_DISCOVERY.md` §5.1 mandates the picker as the new primary surface.**
  Raw VID/PID hex entry becomes an "Advanced / manual override" disclosure. This
  task ships the macOS rendering of that picker (§5.3: "`NSStackView` of rows in
  the `NSAlert` accessory view; an `NSButton` 'Advanced' toggles the `NSTextField`
  pair").
- **`UI.md` §2.0/§2.2 specify the macOS NSAlert contract** that this dialog
  already implements (NSAutoreleasePool wrapper, message text showing current
  `format_id_hex`, OK/Cancel buttons, accessory `NSView`, `runModal` 1000/1001).
  This task adds the picker + relocates the fields while preserving that contract.
- **The zero-config promise must be preserved (§5.1).** The common case — one
  capable board, no VID/PID set — must NOT show a picker; a static `Detected:
  <name>` line is enough.
- **Consumes the classification API** shipped by P3.M1.T1.S1/S2 (Complete). This
  task is a real UI consumer; it exercises `classify_devices` end-to-end on macOS.
- **Completes the three-platform picker parity** alongside the Windows (S1) and
  Linux (S3) siblings: same `{chosen, manual}` result shape + same
  chosen-first-else-manual save precedence (spec §5.3).

## What

Additive `#[cfg(target_os = "macos")]` edits to `src/tray.rs`. **No new Cargo
deps** (`objc = 0.2.7` @56 already provides `Class::get`, `class!`, `declare::ClassDecl`,
`msg_send!`, `sel!`, `sel_impl!`, `runtime::{Object, Sel, Class, YES, NO}` — all
used elsewhere in this file). No Windows/Linux behavior change.

**Platform-specific deviation, documented (Mode A):** no `[ Rescan ]` button on
macOS. `runModal` BLOCKS the tray thread (unlike Windows' modal `GetMessageW`
loop), so there is no "dialog-open" window during which a board could be flashed
and re-scanned. `classify_devices(true)` is called ONCE before building the
accessory view. The item contract (item description) deliberately omits Rescan.

### Success Criteria
- [ ] **`picker_row_text`** (macOS-gated): `✓` (`\u{2713}`) for `Capable{..}`,
      `✗` (`\u{2717}`) for `NotQmkNotifier`; name or `(unnamed)`;
      `0x{:04X}:0x{:04X}`; status `qmk_notifier` / `QMK board, no module`.
- [ ] **`mac_toggle_advanced`** extern "C" fn reads the Advanced checkbox `state`
      (`isize`); sets `setHidden:` on BOTH fields in `ADVANCED_FIELDS` (`YES` if
      unchecked, `NO` if checked).
- [ ] **`static ADVANCED_FIELDS: Mutex<[Option<*mut Object>; 2]>`** declared
      macOS-gated; populated with `(vendor_field, product_field)` before the
      Advanced button's target/action are wired; read by the toggle fn.
- [ ] **`RustMacSettingsTarget`** registered once (guarded by `Class::get`),
      `NSObject` superclass, `add_method(sel!(toggleAdvanced:), mac_toggle_advanced)`;
      an instance set as the Advanced button's target with action `sel!(toggleAdvanced:)`.
- [ ] **`show_settings_dialog_with_pool`** calls `classify_devices(true)`, picks
      the case, builds header + (optional) radio rows + Advanced checkbox +
      relocated fields, dynamic container height, `setAccessoryView`, `runModal`.
- [ ] **Radio rows**: `class!(NSButton)`, `new`, `setTitle:` `picker_row_text(d)`,
      `setButtonType: 4u64` (NSRadioButton), `setTag:` index, `setFrame:` (manual
      vertical stack), `addSubview:` into the container.
- [ ] **Advanced checkbox**: `class!(NSButton)`, `new`, `setTitle:` "Advanced /
      manual override", `setButtonType: 3u64` (NSSwitchButton), default `state`:
      unchecked + fields `setHidden:YES` when capable boards exist; checked +
      fields visible when no capable boards.
- [ ] **Save path**: on OK (`response == 1000`), compute `chosen` (first radio
      with `state == 1` → `devices[i].(vendor_id,product_id)`), compute `manual`
      (read both fields → `parse_id_field` each). Apply `chosen` first
      (`merged.vendor_id=Some(v); merged.product_id=Some(p)`), else `manual`
      (`merged.vendor_id=v; merged.product_id=p`), else leave `current_config`.
      Then `render_config_body` + `atomic_write` (unchanged). Parse error ⇒
      `show_macos_error_message` (unchanged error path).
- [ ] **Mode-A doc-comment** on `show_settings_dialog_with_pool` cites
      `spec/DEVICE_DISCOVERY.md` §5 + `spec/UI.md` §2.2; notes the manual-layout
      choice + the no-Rescan deviation.
- [ ] **`test_picker_row_text_glyphs`** (macOS-gated) passes: `Capable{..}` ⇒ row
      starts with `✓` + contains `0xFEED:0x0000` + `qmk_notifier`; `NotQmkNotifier`
      ⇒ `✗` + `0x3434:0x0123` + `QMK board, no module`.
- [ ] `cargo build --bin qmkonnect` clean on macOS. `cargo test --bin qmkonnect --
      --test-threads=1` green (existing tests + the new test). `git status` =
      `src/tray.rs` only.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can implement this using only this PRP,
because: (a) the exact current `show_settings_dialog_with_pool` code (verbatim,
with line numbers) is in `research/tray_dialog_verification_macos.md` §1; (b) the
exact `ClassDecl` + `extern "C" fn` target/action pattern (the `RustWindowInfoCopyTarget`/
`wi_copy_row` template @2228) is in §2 of that file; (c) the consumed
`classify_devices`/`DeviceKind`/`ClassifiedDevice` API is verified in-tree with
signatures in §3; (d) the NSButton radio/checkbox constants (`NSRadioButton=4`,
`NSSwitchButton=3`, `NSOnState=1`), `setHidden:`/`setButtonType:`/`state`/`setTitle`/
`setTag`/`setTarget`/`setAction` msg_send shapes, and the NSRadioButton exclusivity +
deprecation note, are pinned in §4; (e) the three picker-visibility cases, the
chosen-first-else-manual save precedence, the dynamic container sizing, and the
no-static-result (locals) design are fully specified below; (f) 10 gotchas are
pinned (G1 macOS-only, G2 objc 0.2.7 not objc2, G3 no Rescan, G4 NSRadioButton
deprecation+exclusivity+safety-net, G5 NSSwitchButton for checkbox, G6 u64 enum
args, G7 no explicit release, G8 ADVANCED_FIELDS static for extern fn, G9 picker_row_text
macOS-gated to avoid S1 collision, G10 chosen vs manual types, G11 container
bottom-up origin).

### Documentation & References

```yaml
# MUST READ — the spec source of truth (the §5 picker UX + §5.3 macOS rendering)
- url: spec/DEVICE_DISCOVERY.md
  why: "§5.1 (the 3 picker cases: clean-auto ⇒ static 'Detected: <name>' + no
        picker; ≥2 Tier-1 boards ⇒ picker; no capable board + ≥1 Tier-1 ⇒ picker
        with ✗). §5.2 (Advanced = the existing two hex fields relocated under a
        disclosure). §5.3 (macOS: 'NSStackView of rows in the NSAlert accessory
        view; an NSButton Advanced toggles the NSTextField pair'; the shared
        DIALOG_RESULT becomes {chosen, manual}). §3 (the ✓/✗ + 'qmk_notifier'/
        'QMK board, no module' row semantics)."
  section: "## 5. The Discovered-Device Picker (Settings UX) (§5.1-§5.3)"

# MUST READ — the UI spec (the macOS NSAlert contract this task extends)
- url: spec/UI.md
  why: "§2.0 (the picker as new primary surface; Advanced disclosure; the shared
        DIALOG_RESULT becomes {chosen, manual}; chosen-first-else-manual save).
        §2.2 (the macOS NSAutoreleasePool wrapper; message text showing current
        format_id_hex; OK/Cancel buttons; accessory NSView; runModal 1000/1001;
        parse_id_field; save via render_config_body — the EXISTING contract this
        task extends). §2.4 (parse_id_field: empty/auto ⇒ None)."
  section: "## 2. Settings Dialogs (§2.0, §2.2, §2.4)"

# MUST READ — the codebase verification (THIS task's exact edit sites, verbatim)
- file: plan/005_8b95ea464bd9/P3M2T1S2/research/tray_dialog_verification_macos.md
  why: "§1 the verbatim current show_settings_dialog_with_pool (@1212) + the entry
        show_macos_settings_dialog (@1188, UNCHANGED) + objc_types (@13-34) +
        the helpers (create_nsstring @1316, nsstring_to_rust_string @1336,
        show_macos_error_message @1354, format_id_hex @61, parse_id_field @70).
        §2 the ClassDecl + extern C fn target/action template (RustWindowInfoCopyTarget
        @2224 + wi_copy_row @2228 + YES/NO usage) — the EXACT pattern for
        RustMacSettingsTarget + mac_toggle_advanced. §3 the in-tree notifier API
        signatures. §4 the NSButton radio/checkbox constants + setHidden/state/
        setButtonType msg_send shapes + the NSRadioButton exclusivity+deprecation
        note + why manual layout beats NSStackView. §5 the 8 locked design
        decisions with rationale."

# MUST READ — the file THIS task edits (every line referenced confirmed by reading)
- file: src/tray.rs
  why: "show_macos_settings_dialog @1188 (entry; pool wrapper; UNCHANGED).
        show_settings_dialog_with_pool @1212 (the fn to restructure; builds the
        NSAlert + 2 NSTextFields + 200x60 container; runModal; on-OK read+save).
        objc_types module @13-34 (NSPoint/NSSize/NSRect, repr(C), Copy+Clone —
        reuse for every new frame). format_id_hex @61 + parse_id_field @70 (shared;
        reused unchanged). create_nsstring @1316 + nsstring_to_rust_string @1336
        + show_macos_error_message @1354 (reused unchanged). The ClassDecl +
        extern C fn template: RustWindowInfoCopyTarget @2224 + wi_copy_row @2228 +
        setTarget/setAction @2381-2382. WINDOW_INFO_ROWS @80 (the Mutex<Vec> static
        pattern ADVANCED_FIELDS mirrors). The window-info NSTextField label pattern
        @2295 (setBezeled:NO/setDrawsBackground:NO/setEditable:NO) if a read-only
        header label is desired."
  pattern: "msg_send![class!(NSButton), new]; setTitle:/setButtonType:/setTag:/
            setTarget:/setAction:/setFrame:/state via msg_send! + sel!. Frames via
            objc_types::NSRect{origin:NSPoint{x,y}, size:NSSize{w,h}}. Strings via
            create_nsstring(&format!(...)) then msg_send![obj, setX: ns]. Read on
            OK via msg_send![field, stringValue] then nsstring_to_rust_string."
  gotcha: "G2 objc 0.2.7 (legacy): use Class::get / class! / msg_send! / sel! /
           declare::ClassDecl — NOT objc2. G4 NSRadioButton=4 is deprecated but
           functional; take-first-NSOnState on OK as the safety net. G6 pass enum
           args as u64 (setButtonType: 4u64). G7 do NOT release the new objects
           (pool + alert retention handle them). G9 picker_row_text MUST be
           #[cfg(target_os = 'macos')] (S1's is windows-gated)."

# MUST READ — the consumer API (the classification functions this task calls)
- file: src/core/notifier.rs
  why: "classify_devices(verbose: bool) -> Vec<ClassifiedDevice> @1116 (enumerate
        Tier-1 + per-candidate QUERY_INFO + cache; verbose=true for diagnostic
        eprintln). pub enum DeviceKind { Capable{..}, NotQmkNotifier } @816 (the
        ✓/✗ discriminator). pub struct ClassifiedDevice { path, vendor_id:u16,
        product_id:u16, product_name:Option<String>, usage_page, usage, kind } @841
        (vendor_id/product_id are u16 — always Some — so a pick yields a concrete
        (u16,u16); product_name may be None ⇒ '(unnamed)')."
  pattern: "All three are #[allow(dead_code)] today (no consumer yet); this task is
            a consumer. Call as crate::core::notifier::classify_devices(true)."

# Reference — AppKit NSButton types + NSControlState (constants pinned in research §4)
- url: https://developer.apple.com/documentation/appkit/nsbutton/setbuttontype(_:)
  why: "setButtonType: takes NSButtonType. NSSwitchButton=3 (checkbox, NOT
        deprecated) for the Advanced toggle; NSRadioButton=4 (deprecated but
        functional) for the device rows. Pass as 3u64/4u64 in this codebase's
        msg_send idiom (matches setLineBreakMode: 3u64 @2303)."
- url: https://developer.apple.com/documentation/appkit/nsswitchbutton
  why: "'NSRadioButton … is used to constrain a selection to a single element from
        several elements' — confirms sibling-radio exclusivity is built-in (no
        NSMatrix needed) and documents the deprecation (acceptable for beta; the
        NSSwitchButton+coordinator future path is a non-goal for v1)."
- url: https://developer.apple.com/documentation/appkit/nscontrol/1428504-state
  why: "state returns NSControlState: NSOnState=1, NSOffState=0. Read via
        `let s: isize = msg_send![btn, state];` — 1 ⇒ selected (the row to pick /
        the checkbox to interpret for setHidden)."
```

### Current Codebase tree (relevant subset)

```bash
src/
  tray.rs             # macOS + Windows tray + dialogs (2431 lines).
                        # objc_types @13-34; format_id_hex @61; parse_id_field @70;
                        # DIALOG_RESULT(windows) @53; WINDOW_INFO_ROWS @80;
                        # show_macos_settings_dialog @1188 (entry; UNCHANGED);
                        # show_settings_dialog_with_pool @1212 (RESTRUCTURE);
                        # create_nsstring @1316; nsstring_to_rust_string @1336;
                        # show_macos_error_message @1354;
                        # RustWindowInfoCopyTarget @2224 + wi_copy_row @2228
                        #   (the ClassDecl + extern C fn TEMPLATE);
                        # tests mod @~2413.
                        # <-- THIS TASK: macOS-only restructure of
                        #     show_settings_dialog_with_pool + new picker_row_text
                        #     + new RustMacSettingsTarget/mac_toggle_advanced +
                        #     static ADVANCED_FIELDS + 1 test.
  core/
    notifier.rs       # classify_devices @1116; DeviceKind @816; ClassifiedDevice @841
                        # (CONSUMED, not edited)
    mod.rs            # Config @24 (vendor_id/product_id: Option<u16>);
                        # render_config_body; atomic_write (CONSUMED, not edited)
spec/
  DEVICE_DISCOVERY.md # §5 = the picker UX source of truth (READ-ONLY)
  UI.md               # §2.0/§2.2/§2.4 = the macOS dialog contract (READ-ONLY)
Cargo.toml            # objc 0.2.7 (@56; macOS feature @115) — UNCHANGED
```

### Desired Codebase tree (files this task changes)

```bash
src/
  tray.rs             # MODIFIED (macOS-only, additive):
                        #  + picker_row_text (macOS-gated)
                        #  + static ADVANCED_FIELDS (macOS-gated)
                        #  + mac_toggle_advanced extern C fn + RustMacSettingsTarget
                        #    registration (macOS-gated)
                        #  + REWRITE show_settings_dialog_with_pool body:
                        #    classify_devices + 3 cases + radio rows + Advanced
                        #    checkbox + relocated fields + dynamic container +
                        #    chosen-first-else-manual save
                        #  + test_picker_row_text_glyphs (macOS-gated)
    # EVERYTHING else unchanged (Cargo.toml, core/*, linux_tray.rs, the Windows
    # dialog, the macOS window-info dialog, spec/*, packaging/*)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (G1 — macOS-only): ALL new code is #[cfg(target_os = "macos")]. The
//   Windows dialog (show_settings_dialog @779) is a SEPARATE sibling task
//   (P3.M2.T1.S1, in flight); the Linux dialog (linux_tray.rs) is P3.M2.T1.S3.
//   The shared parse_id_field @70 is gated windows+macos — leave it. Everything
//   new is macOS-gated so a Windows/Linux build is byte-for-byte unchanged.
//
// CRITICAL (G2 — objc 0.2.7, NOT objc2): this file uses the LEGACY `objc` crate
//   (Cargo.toml:56). Use Class::get("NSAlert"), class!(NSButton), msg_send![...],
//   sel!(toggleAdvanced:), sel_impl!, declare::ClassDecl, runtime::{Object, Sel,
//   Class, YES, NO}. Do NOT use objc2/objc2-foundation APIs (those are separate
//   crates used only for the window-title monitor, NOT the dialogs).
//
// CRITICAL (G3 — NO Rescan on macOS): runModal BLOCKS the tray thread. There is no
//   "dialog-open" window to re-scan within (unlike Windows' GetMessageW loop, where
//   S1 added Rescan). classify_devices(true) is called ONCE before building the
//   accessory view. The item contract deliberately omits Rescan. Document this in
//   the doc-comment (Mode A). Do NOT add a [Rescan] button.
//
// CRITICAL (G4 — NSRadioButton=4 is DEPRECATED but functional): Apple marks it
//   deprecated in favor of NSSwitchButton + a coordinator. It remains fully
//   functional on all current macOS and AppKit does not remove classic primitives.
//   For a beta app this is acceptable (document it). Sibling radio buttons in the
//   SAME superview auto-enforce mutual exclusivity (no NSMatrix needed). SAFETY
//   NET: on OK, take the FIRST radio with state==1 (NSOnState) — imperfect
//   exclusivity can never produce a wrong VID/PID.
//
// CRITICAL (G5 — the Advanced toggle is NSSwitchButton=3, a CHECKBOX, NOT radio):
//   the contract explicitly says "checkbox style". Use setButtonType: 3u64. Its
//   default state: unchecked (state=0) + fields setHidden:YES when capable boards
//   exist; checked (state=1) + fields visible when NO capable boards.
//
// GOTCHA (G6 — pass enum/constant args as u64 in this codebase): the file's idiom
//   is msg_send![obj, setX: 3u64] for integer enum args (see setLineBreakMode: 3u64
//   @2303, setImagePosition: 1u64 @2343). So setButtonType: 4u64 (radio) /
//   3u64 (checkbox). state is READ as isize: `let s: isize = msg_send![btn, state];`.
//
// GOTCHA (G7 — do NOT explicitly release the new objects): the existing settings
//   dialog creates the alert/fields/container via `new` and never calls release —
//   they survive the modal via the NSAutoreleasePool drain + the alert's retention
//   of its accessory view. Follow this convention for the rows + toggle button +
//   target (do NOT add release calls; they would over-release). The target object
//   created via new is retained-but-unowned (same as the window-info target @2260)
//   — a negligible single-object leak per rare dialog open, accepted for beta.
//
// CRITICAL (G8 — the toggle extern fn needs a STATIC for the field pointers): an
//   extern "C" fn cannot capture Rust locals. mac_toggle_advanced reads the two
//   field pointers from `static ADVANCED_FIELDS: Mutex<[Option<*mut Object>; 2]>`
//   (mirrors WINDOW_INFO_ROWS @80 — the established pattern for a free/extern fn
//   that needs shared state). Populate it with (vendor_field, product_field) AFTER
//   creating the fields, BEFORE wiring the toggle button's target/action.
//
// CRITICAL (G9 — picker_row_text MUST be macOS-gated, NOT macos+windows): the
//   parallel sibling S1 adds #[cfg(target_os = "windows")] fn picker_row_text. If
//   THIS task gates it #[cfg(any(macos, windows))], the two definitions collide on
//   Windows (both active). Gate it #[cfg(target_os = "macos")] so exactly one
//   compiles per platform (no collision, no merge conflict). Same for the test
//   test_picker_row_text_glyphs (macOS-gated here vs windows-gated in S1).
//
// CRITICAL (G10 — chosen vs manual types): ClassifiedDevice.vendor_id/product_id
//   are u16 (always present) ⇒ a radio pick yields a CONCRETE (u16,u16) — that is
//   `chosen: Option<(u16,u16)>` (a LOCAL, no struct/static needed on macOS). The
//   typed hex fields yield `(Option<u16>, Option<u16>)` each (blank ⇒ None) — that
//   is `manual`. In the save path, lift chosen: merged.vendor_id=Some(v);
//   merged.product_id=Some(p). Do NOT confuse the two.
//
// GOTCHA (G11 — NSView coordinate origin is BOTTOM-LEFT): setFrame y increases
//   UPWARD. The existing fields are at y=0 (vendor) and y=30 (product) — the
//   BOTTOM of the container. The Advanced checkbox goes ABOVE them (y≈60), the
//   radio rows ABOVE that (y≈88+i*row_h), the header at the TOP. Compute
//   container height = bottom_section + middle_section + rows + header + padding
//   BEFORE setAccessoryView (the accessory view's frame determines the alert's
//   content height).
//
// CRATE QUIRK: cargo test --bin qmkonnect -- --test-threads=1 (AGENTS.md; shared
//   MockNotifier globals + DebounceState). The NSAlert itself is NOT unit-testable
//   (it spawns a real AppKit modal); only the pure picker_row_text builder is.
```

## Implementation Blueprint

### Data models and structure

```rust
// ── (1) the row-label builder (macOS-gated; mirrors S1's windows-gated version) ──
/// One picker row's label: the ✓/✗ capability glyph, the live `product_name`
/// (or `(unnamed)`), the `0xVID:0xPID`, and the status suffix. Built from a
/// [`ClassifiedDevice`] (`spec/DEVICE_DISCOVERY.md` §5.1 / §3). Pure; unit-tested.
//
// G9: macOS-gated so it never collides with the windows-gated `picker_row_text`
// in the parallel sibling P3.M2.T1.S1 (exactly one compiles per target_os).
#[cfg(target_os = "macos")]
fn picker_row_text(d: &crate::core::notifier::ClassifiedDevice) -> String {
    use crate::core::notifier::DeviceKind;
    let (glyph, status) = match d.kind {
        DeviceKind::Capable { .. } => ("\u{2713}", "qmk_notifier"),         // ✓
        DeviceKind::NotQmkNotifier => ("\u{2717}", "QMK board, no module"), // ✗
    };
    let name = d.product_name.as_deref().unwrap_or("(unnamed)");
    format!("{}  {:<22} 0x{:04X}:0x{:04X}  {}", glyph, name, d.vendor_id, d.product_id, status)
}

// ── (2) the field-pointer slot for the Advanced toggle extern fn (G8) ──
// mac_toggle_advanced is an extern "C" fn (registered as toggleAdvanced:) and
// CANNOT capture locals, so it reads the two NSTextField pointers from this
// static (mirrors WINDOW_INFO_ROWS @80). Populated in show_settings_dialog_with_pool
// right after the fields are created. Only one settings dialog is open at a time.
#[cfg(target_os = "macos")]
static ADVANCED_FIELDS: std::sync::Mutex<[Option<*mut objc::runtime::Object>; 2]> =
    std::sync::Mutex::new([None, None]);

// NOTE: NO DIALOG_RESULT static on macOS (G10/G4 of research §5.4). The dialog is
// synchronous (runModal blocks); `chosen: Option<(u16,u16)>` and the two
// parse_id_field results are plain LOCALS computed after runModal returns. Only
// ADVANCED_FIELDS is static (the extern toggle fn cannot see locals).

// ── (3) the Advanced toggle extern fn + its registered class (template: wi_copy_row) ──
#[cfg(target_os = "macos")]
extern "C" fn mac_toggle_advanced(
    _this: &objc::runtime::Object,
    _sel: objc::runtime::Sel,
    sender: *mut objc::runtime::Object,  // the Advanced checkbox NSButton
) {
    use objc::runtime::{YES, NO};
    use objc::{msg_send, sel, sel_impl};
    // The checkbox flips its own state on click before this action fires. Show the
    // fields when checked (state==1/NSOnState), hide when unchecked.
    let state: isize = unsafe { msg_send![sender, state] };
    let hide = if state == 1 { NO } else { YES }; // NSOnState=1
    if let Ok(fields) = ADVANCED_FIELDS.lock() {
        for field_opt in fields.iter() {
            if let Some(field) = field_opt {
                if !(*field).is_null() {
                    let _: () = unsafe { msg_send![*field, setHidden: hide] };
                }
            }
        }
    }
}
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: ADD picker_row_text + ADVANCED_FIELDS + mac_toggle_advanced (macOS-gated)
  - DO: add the three items above (under #[cfg(target_os = "macos")]), placed near
        the existing macOS dialog helpers (e.g. just above show_macos_settings_dialog
        @1188, or alongside the other macOS-gated helpers). picker_row_text first
        (Task 1's test target), then ADVANCED_FIELDS, then mac_toggle_advanced.
  - G9: picker_row_text is macOS-gated (NOT macos+windows) — avoids collision with
        S1's windows-gated version. G8: ADVANCED_FIELDS is the only static (the
        extern fn cannot capture locals). G6: state read as isize; setHidden with
        YES/NO from objc::runtime.
  - NOTE: mac_toggle_advanced references ADVANCED_FIELDS and compiles standalone.
        RustMacSettingsTarget registration is Task 3 (inside the dialog fn).

Task 2: REWRITE the body of show_settings_dialog_with_pool (@1212) — classify + cases
  - DO: keep `current_config = parse_config(...).unwrap_or_default()` (UNCHANGED).
        Then INSERT, after current_config and BEFORE the `unsafe {` alert build:
          let devices = crate::core::notifier::classify_devices(true);
          use crate::core::notifier::DeviceKind;
          let clean_auto = devices.len() == 1
              && matches!(devices[0].kind, DeviceKind::Capable { .. })
              && current_config.vendor_id.is_none()
              && current_config.product_id.is_none();
        Then build the alert + OK/Cancel buttons EXACTLY as today (UNCHANGED).
        KEEP the setMessageText "QMK Settings". UPDATE setInformativeText to:
          - empty: "No QMK keyboards detected. Use Advanced to enter IDs manually.\n\nCurrent — Vendor ID: {fmt} / Product ID: {fmt}"
          - clean-auto: "Detected: {name}. Auto-discovery is active.\n\nVendor ID: {fmt} / Product ID: {fmt}"
          - picker: "Select a detected keyboard below (or use Advanced for manual entry).\n\nVendor ID: {fmt} / Product ID: {fmt}"
        (where {fmt} = format_id_hex(current_config.vendor_id/product_id); {name} =
        devices[0].product_name.as_deref().unwrap_or("(unnamed)")).
        UI.md §2.2 requires the message text show the current format_id_hex — KEEP it.
  - G3: classify_devices(true) is called ONCE here (no Rescan). G4: clean-auto uses
        DeviceKind::Capable match.

Task 3: BUILD the accessory view (header + optional rows + Advanced + relocated fields)
  - DO: inside the existing `unsafe { ... }` block, AFTER the alert + buttons,
        REPLACE the current "two NSTextFields + 200x60 container" block with:
        (a) Register RustMacSettingsTarget ONCE (guarded by Class::get) — mirrors
            RustWindowInfoCopyTarget @2224:
              use objc::{class, declare::ClassDecl};
              if Class::get("RustMacSettingsTarget").is_none() {
                  let superclass = Class::get("NSObject").ok_or("NSObject class not found")?;
                  let mut decl = ClassDecl::new("RustMacSettingsTarget", superclass)
                      .ok_or("failed to declare RustMacSettingsTarget")?;
                  decl.add_method(sel!(toggleAdvanced:),
                      mac_toggle_advanced as extern "C" fn(
                          &objc::runtime::Object, objc::runtime::Sel, *mut objc::runtime::Object));
                  decl.register();
              }
              let target: *mut Object = msg_send![
                  Class::get("RustMacSettingsTarget").ok_or("RustMacSettingsTarget missing")?,
                  new];
        (b) Create the two NSTextFields (vendor_field, product_field) — EXACTLY as
            today (new, setStringValue with format_id_hex, setFrame), but RELOCATE
            their y to the BOTTOM of the (taller) container and widen them. Keep
            vendor at the lower y, product above it. e.g.:
              vendor_field  setFrame origin (0,0)   size (300,22)
              product_field setFrame origin (0,30)  size (300,22)
        (c) Populate ADVANCED_FIELDS: *ADVANCED_FIELDS.lock().unwrap() = [Some(vendor_field), Some(product_field)];
        (d) Create the Advanced checkbox:
              let adv_btn: *mut Object = msg_send![class!(NSButton), new];
              let adv_title = create_nsstring("Advanced / manual override")?;
              let _: () = msg_send![adv_btn, setTitle: adv_title];
              let _: () = msg_send![adv_btn, setButtonType: 3u64];  // NSSwitchButton (G5)
              let _: () = msg_send![adv_btn, setFrame: NSRect{ origin:(0,60), size:(300,22) }];
              let _: () = msg_send![adv_btn, setTarget: target];
              let _: () = msg_send![adv_btn, setAction: sel!(toggleAdvanced:)];
            Default state: if capable boards exist (devices has any Capable) ⇒
              state=0 + both fields setHidden:YES; else (no capable / empty) ⇒
              state=1 + fields visible. Set:
              let show_advanced = !devices.iter().any(|d| matches!(d.kind, DeviceKind::Capable{..}));
              let init_state: isize = if show_advanced { 1 } else { 0 };
              let _: () = msg_send![adv_btn, setState: init_state];
              let hide = if show_advanced { NO } else { YES };
              let _: () = msg_send![vendor_field, setHidden: hide];
              let _: () = msg_send![product_field, setHidden: hide];
        (e) Create the device radio rows (ONLY in the picker case, i.e. NOT clean-auto
            AND NOT empty; else zero rows):
              let mut row_btns: Vec<*mut Object> = Vec::new();
              let row_h: f64 = 22.0;
              let rows_base_y: f64 = 88.0;   // above the Advanced checkbox (y=60+22)
              for (i, d) in devices.iter().enumerate() {
                  let row: *mut Object = msg_send![class!(NSButton), new];
                  let lbl = create_nsstring(&picker_row_text(d))?;
                  let _: () = msg_send![row, setTitle: lbl];
                  let _: () = msg_send![row, setButtonType: 4u64];   // NSRadioButton (G4)
                  let _: () = msg_send![row, setTag: i as isize];
                  let _: () = msg_send![row, setFrame: NSRect{
                      origin:(0.0, rows_base_y + (i as f64)*row_h), size:(360.0, row_h) }];
                  row_btns.push(row);
              }
        (f) Create the header label (NSTextField, read-only) at the TOP:
              let header_text = if devices.is_empty() {
                  "No QMK keyboards detected.".to_string()
              } else if clean_auto {
                  format!("Detected: {}", devices[0].product_name.as_deref().unwrap_or("(unnamed)"))
              } else {
                  "Detected keyboard(s) — choose one:".to_string()
              };
              let header: *mut Object = msg_send![class!(NSTextField), new];
              let h_ns = create_nsstring(&header_text)?;
              let _: () = msg_send![header, setStringValue: h_ns];
              let _: () = msg_send![header, setBezeled: NO];
              let _: () = msg_send![header, setDrawsBackground: NO];
              let _: () = msg_send![header, setEditable: NO];
              let _: () = msg_send![header, setSelectable: YES];
              let rows_count = if devices.is_empty() || clean_auto { 0 } else { devices.len() };
              let header_y = rows_base_y + (rows_count as f64)*row_h + 4.0;
              let _: () = msg_send![header, setFrame: NSRect{ origin:(0.0, header_y), size:(360.0, 18.0) }];
            (Follow the read-only NSTextField label idiom from the window-info dialog
            @2295: setBezeled:NO/setDrawsBackground:NO/setEditable:NO/setSelectable:YES.)
        (g) Build the container NSView with DYNAMIC height (G11 — origin bottom-left):
              let container_height = header_y + 18.0 + 8.0; // header + padding
              let view_class = Class::get("NSView").ok_or("Failed to get NSView class")?;
              let container_view: *mut Object = msg_send![view_class, new];
              let _: () = msg_send![container_view, setFrame: NSRect{
                  origin:(0.0,0.0), size:(360.0, container_height) }];
              // addSubview order does not matter for plain subviews (no z-order need):
              let _: () = msg_send![container_view, addSubview: header];
              for row in &row_btns { let _: () = msg_send![container_view, addSubview: *row]; }
              let _: () = msg_send![container_view, addSubview: adv_btn];
              let _: () = msg_send![container_view, addSubview: vendor_field];
              let _: () = msg_send![container_view, addSubview: product_field];
              let _: () = msg_send![alert, setAccessoryView: container_view];
  - FOLLOW: the exact msg_send shapes from the verbatim current code (research §1)
        + the window-info label idiom @2295. G6: setButtonType 3u64/4u64, state/setState
        isize. G7: no release. G8: populate ADVANCED_FIELDS before setTarget/setAction.
  - DOC-COMMENT (Mode A): cite spec/DEVICE_DISCOVERY.md §5 + spec/UI.md §2.2; note
        the manual-layout choice (vs NSStackView) + the no-Rescan deviation (G3) +
        the NSRadioButton deprecation (G4).

Task 4: UPDATE the on-OK result handling (chosen-first-else-manual)
  - DO: REPLACE the current `if response == 1000 { ... read fields ... save ... }`
        block with:
          if response == 1000 {
              // chosen: first NSOnState radio row → concrete (u16,u16) (G4 safety net)
              let chosen: Option<(u16, u16)> = row_btns.iter().enumerate()
                  .filter_map(|(i, _)| {
                      let s: isize = msg_send![*row_btns[i], state];  // NSOnState=1
                      (s == 1).then(|| (devices[i].vendor_id, devices[i].product_id))
                  }).next();
              // manual: read both fields → parse_id_field each (G10: each Option<u16>)
              let v_ns: *mut Object = msg_send![vendor_field, stringValue];
              let p_ns: *mut Object = msg_send![product_field, stringValue];
              let vendor_str = nsstring_to_rust_string(v_ns)?;
              let product_str = nsstring_to_rust_string(p_ns)?;
              match (parse_id_field(&vendor_str), parse_id_field(&product_str)) {
                  (Ok(vid), Ok(pid)) => {
                      let mut merged = current_config;
                      if let Some((v, p)) = chosen { merged.vendor_id = Some(v); merged.product_id = Some(p); }
                      else { merged.vendor_id = vid; merged.product_id = pid; }
                      let config_content = crate::core::render_config_body(&merged);
                      crate::core::atomic_write(config_path, &config_content)?;
                  }
                  (Err(e), _) | (_, Err(e)) => show_macos_error_message(&format!("Invalid input: {}", e)),
              }
          }
  - NOTE: `row_btns` and `devices` are LOCALS in this same fn (Task 3 built them) —
        no static needed (G10). chosen takes precedence (G4); manual is the typed
        fields (which were prefilled with the open-time hex ⇒ "leave as-is" when the
        user changes nothing + no row picked). render_config_body/atomic_write/error
        path UNCHANGED.

Task 5: ADD test_picker_row_text_glyphs (macOS-gated — the only unit-testable piece)
  - DO: in the existing #[cfg(all(test, any(target_os="macos", target_os="windows")))]
        mod tests (@~2413), add:
          #[cfg(target_os = "macos")]
          #[test]
          fn test_picker_row_text_glyphs() {
              use crate::core::notifier::{ClassifiedDevice, DeviceKind};
              let capable = ClassifiedDevice {
                  path: String::new(), vendor_id: 0xFEED, product_id: 0x0000,
                  product_name: Some("Dactyl".into()), usage_page: 0xFF60, usage: 0x61,
                  kind: DeviceKind::Capable { proto_ver: 2, feature_flags: 1,
                      callback_count: 0, board_rules_present: false },
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
  - G9: macOS-gated (S1's is windows-gated) ⇒ no collision. ClassifiedDevice derives
        Clone (notifier.rs:842); DeviceKind derives PartialEq/Clone (notifier.rs:817).

Task 6: VERIFY build + tests (single-threaded, per AGENTS.md)
  - RUN: cargo build --bin qmkonnect   (macOS: full dialog; Windows/Linux: macOS-gated
        items cfg'd out ⇒ unchanged).
  - RUN: cargo test --bin qmkonnect -- --test-threads=1   (existing tests + the new
        macOS-gated test_picker_row_text_glyphs pass on macOS).
  - CONFIRM git status shows EXACTLY one file: src/tray.rs.
  - MANUAL (macOS only, per AGENTS.md dev loop): cargo test ... ; cd packaging/macos
        && ./clean.sh && ./build.sh && ./install.sh; open /Applications/QMKonnect.app;
        grant Screen-Recording prompt; tray menu → Settings…; verify the 3 picker
        cases against real hardware (≥2 boards, 1 capable, 0 boards); verify the
        Advanced toggle shows/hides the fields; verify manual entry still writes
        config.toml; verify chosen-takes-precedence.
```

### Implementation Patterns & Key Details

```rust
// The radio row (Task 3e) — NSRadioButton=4, title = picker_row_text (G4/G6):
// let row: *mut Object = msg_send![class!(NSButton), new];
// let lbl = create_nsstring(&picker_row_text(d))?;
// let _: () = msg_send![row, setTitle: lbl];
// let _: () = msg_send![row, setButtonType: 4u64];                 // NSRadioButton
// let _: () = msg_send![row, setTag: i as isize];                 // row index
// let _: () = msg_send![row, setFrame: objc_types::NSRect {
//     origin: objc_types::NSPoint { x: 0.0, y: 88.0 + (i as f64)*22.0 },
//     size:   objc_types::NSSize { width: 360.0, height: 22.0 } }];

// The Advanced checkbox (Task 3d) — NSSwitchButton=3 + target/action (G5/G6/G8):
// let adv_btn: *mut Object = msg_send![class!(NSButton), new];
// let _: () = msg_send![adv_btn, setTitle: create_nsstring("Advanced / manual override")?];
// let _: () = msg_send![adv_btn, setButtonType: 3u64];             // NSSwitchButton (checkbox)
// let _: () = msg_send![adv_btn, setTarget: target];               // RustMacSettingsTarget
// let _: () = msg_send![adv_btn, setAction: sel!(toggleAdvanced:)];
// let _: () = msg_send![adv_btn, setState: init_state];            // 1 if no capable boards, else 0

// The toggle extern fn (Task 1) — reads checkbox state, flips setHidden (G8):
// extern "C" fn mac_toggle_advanced(_this, _sel, sender: *mut Object) {
//     let state: isize = unsafe { msg_send![sender, state] };       // NSOnState=1
//     let hide = if state == 1 { NO } else { YES };
//     for f in ADVANCED_FIELDS.lock().unwrap().iter().flatten() {
//         let _: () = unsafe { msg_send![*f, setHidden: hide] };
//     }
// }

// The on-OK chosen read (Task 4) — first NSOnState radio (G4 safety net):
// let chosen = row_btns.iter().enumerate().filter_map(|(i, _)| {
//     let s: isize = msg_send![row_btns[i], state];
//     (s == 1).then(|| (devices[i].vendor_id, devices[i].product_id))
// }).next();

// Container height (Task 3g) — dynamic, origin bottom-left (G11):
// let container_height = header_y + 18.0 + 8.0;
// msg_send![container_view, setFrame: NSRect{ origin:(0,0), size:(360.0, container_height) }];
```

### Integration Points

```yaml
CODE (this task):
  - file: src/tray.rs
    change: "macOS-only additive — picker_row_text + static ADVANCED_FIELDS +
             mac_toggle_advanced extern fn + RustMacSettingsTarget registration +
             REWRITE of show_settings_dialog_with_pool body (classify_devices +
             3 cases + radio rows + Advanced checkbox + relocated fields +
             dynamic container + chosen-first-else-manual save) + 1 test"
    pattern: "msg_send! shapes mirror the verbatim current dialog (research §1);
              the ClassDecl + extern C fn target/action mirrors RustWindowInfoCopyTarget
              @2224 / wi_copy_row @2228; the read-only header label idiom mirrors the
              window-info label @2295; ADVANCED_FIELDS mirrors WINDOW_INFO_ROWS @80."

DEPENDENCIES (this task): NONE new. objc 0.2.7 (@56; macOS feature @115) already
                           provides Class::get/class!/declare::ClassDecl/msg_send!/
                           sel!/sel_impl!/runtime::{Object,Sel,Class,YES,NO}. No new `use`
                           crate — NSButton/NSView/NSTextField/NSAlert all via Class::get/class!.

UPSTREAM (consumed read-only):
  - crate::core::notifier::classify_devices(verbose: bool) -> Vec<ClassifiedDevice> (notifier.rs:1116).
  - crate::core::notifier::DeviceKind { Capable{..}, NotQmkNotifier } (notifier.rs:816).
  - crate::core::notifier::ClassifiedDevice { vendor_id:u16, product_id:u16, product_name:Option<String>, kind:DeviceKind, .. } (notifier.rs:841).
  - crate::core::Config { vendor_id:Option<u16>, product_id:Option<u16>, .. } (mod.rs:24).
  - crate::core::render_config_body + atomic_write (mod.rs) — UNCHANGED.
  - parse_id_field @70 (shared windows+macos; UNCHANGED) + format_id_hex @61 + create_nsstring
    @1316 + nsstring_to_rust_string @1336 + show_macos_error_message @1354 (reused).

DOWNSTREAM / SIBLINGS (do NOT implement them here):
  - P3.M2.T1.S1 (Windows Win32 picker): shares the {chosen, manual} result SEMANTICS +
    chosen-first-else-manual save (Windows needs a static DIALOG_RESULT; macOS uses locals).
    Its windows-gated picker_row_text is mutually exclusive with this task's macOS-gated one.
  - P3.M2.T1.S3 (Linux zenity picker): same result shape.
  - P4.M2.T1.S1 (Mode-A doc sync): will cite this dialog in README/UI docs.

NO OVERLAP:
  - Windows dialog (show_settings_dialog @779): P3.M2.T1.S1 — UNTOUCHED (its windows-gated
    picker_row_text does not collide with this macOS-gated one).
  - Linux dialog (linux_tray.rs): P3.M2.T1.S3 — UNTOUCHED.
  - classify_devices / cache / DeviceStatus (notifier.rs): P3.M1 / P1 — Complete, read-only.
  - macOS window-info dialog (@2186+): separate target class (RustWindowInfoCopyTarget) — UNTOUCHED.

CONFIG: none (no config schema change — VID/PID stays Option<u16>). ROUTES: none. DATABASE: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles clean on ALL platforms. On macOS the new dialog code compiles;
# on Windows/Linux the #[cfg(target_os="macos")] items are cfg'd out (no change).
# If it fails on macOS: most likely a missing objc import (class!/declare::ClassDecl/
# msg_send!/sel!/sel_impl!/runtime::{Object,Sel,Class,YES,NO}), a wrong msg_send arg
# type (use u64 for setButtonType, isize for state/setState, YES/NO for setHidden),
# or a borrow/type issue on row_btns/devices in the on-OK closure — READ + fix.

# Confirm the deliverables are present (macOS-gated — grep finds them regardless of host):
grep -n 'fn picker_row_text' src/tray.rs                       # expect 2 (macOS here + windows S1)
grep -n 'static ADVANCED_FIELDS' src/tray.rs                   # expect 1
grep -n 'fn mac_toggle_advanced' src/tray.rs                   # expect 1
grep -n 'RustMacSettingsTarget' src/tray.rs                    # expect >=2 (decl + get)
grep -n 'toggleAdvanced' src/tray.rs                           # expect >=2 (sel + action)
grep -c 'setButtonType: 4u64\|setButtonType: 3u64' src/tray.rs # expect >=2 (radio rows + checkbox)
grep -c 'classify_devices' src/tray.rs                         # expect >=1 (macOS) + existing windows
grep -n 'chosen' src/tray.rs                                   # expect the macOS on-OK chosen read
```

### Level 2: Unit Tests (Component Validation — the pure builder)

```bash
cd /home/dustin/projects/qmkonnect
# Single-threaded is MANDATORY crate-wide (shared MockNotifier globals + DebounceState, AGENTS.md).
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL green — the existing tests + the new macOS-gated test_picker_row_text_glyphs
# (asserts ✓/✗ glyphs + vid:pid + status labels). The NSAlert functions are
# #[cfg(target_os="macos")] and spawn a real AppKit modal — NOT unit-testable; they are
# covered by the Level-4 manual check on macOS.

cargo test --bin qmkonnect picker_row -- --test-threads=1   # filter to the new test
```

### Level 3: Cross-platform regression (the new code is macOS-only)

```bash
cd /home/dustin/projects/qmkonnect
# On Linux/Windows (or any non-macOS host): confirm the macOS-gated additions don't break the build.
cargo build --bin qmkonnect
# Expected: clean — every new item is #[cfg(target_os = "macos")], so a non-macOS host
# compiles the rest unchanged. (picker_row_text/test_picker_row_text_glyphs are macOS-gated too.)
# NOTE: if the Windows sibling S1 has already merged its windows-gated picker_row_text, a
# Windows build still has exactly ONE picker_row_text (the windows-gated one) — no collision.

# Confirm the change surface is exactly one file:
git status --short
# Expected: only src/tray.rs modified. NOTHING in Cargo.toml, core/, linux_tray.rs,
# architecture/, docs/, spec/, packaging/.
git diff --stat
# Expected: 1 file: src/tray.rs.
```

### Level 4: Manual dialog testing (macOS only — per AGENTS.md dev loop)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1                 # single-threaded (shared debouncer state)
cd packaging/macos && ./clean.sh && ./build.sh && ./install.sh  # clean → build → install to /Applications
open /Applications/QMKonnect.app
# Grant the Screen-Recording prompt (ad-hoc signing → new cdhash → re-prompt). Then: tray menu → Settings…:
#  CASE A (≥2 Tier-1 boards, or 1 capable + 1 VIA board): one radio row per device with ✓ (capable) /
#         ✗ (no module) glyphs, product_name, 0xVID:0xPID, status label. Click the capable radio → OK →
#         open config.toml → confirm vendor_id/product_id = that board's (vid,pid).
#  CASE B (1 capable board, config has no VID/PID): NO radio rows; header reads "Detected: <name>".
#         Advanced checkbox unchecked + fields hidden. OK leaves config on auto (no VID/PID written).
#  CASE C (0 boards): NO radio rows; header "No QMK keyboards detected…"; Advanced checkbox CHECKED +
#         fields shown (so the user can type). Enter a hex pair + OK → that pair is written.
#  Advanced toggle: in CASE A, click "Advanced / manual override" → the two hex fields appear; type a
#         pair + clear any radio selection → OK → that pair is written (manual applies when chosen is None).
#  Precedence: select a radio row AND type a different pair → chosen wins (the row's VID/PID).
#  Clean-auto + manual override: in CASE B, toggle Advanced on, type a pair → OK → the typed pair wins
#         (chosen is None because no row was shown).
# Expected: all 3 cases render correctly; selection writes the right VID/PID; the Advanced toggle
#         shows/hides the fields; manual entry + precedence work. The accessory view is not clipped.
# HEALTH CHECK: if the dialog/menu looks dimmed/unclickable, the main thread is wedged — rebuild
#         (stale binary misleads) then `sample <pid> 2 | grep -i mutex` (healthy = nextEventMatchingMask).
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean on the host platform (macOS: full dialog; Windows/Linux: macOS-gated items cfg'd out).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` green (existing tests + new macOS-gated `test_picker_row_text_glyphs`).
- [ ] `git status` shows exactly ONE modified file: `src/tray.rs`.

### Feature Validation (contract fidelity)
- [ ] `show_settings_dialog_with_pool` calls `classify_devices(true)` once and picks one of the three cases.
- [ ] **Picker case (≥2 boards / 1 non-capable)**: one `NSButton` radio row per device (`setButtonType: 4u64`), titled `picker_row_text(d)` (✓/✗ glyph + name + `0xVID:0xPID` + status); header "Detected keyboard(s) — choose one:".
- [ ] **Clean-auto case (1 Capable + no open VID/PID)**: NO radio rows; header `Detected: <name>`.
- [ ] **Empty case (0 devices)**: NO radio rows; header "No QMK keyboards detected…".
- [ ] **Advanced toggle** (`setButtonType: 3u64` NSSwitchButton): target = `RustMacSettingsTarget`, action `sel!(toggleAdvanced:)`; default unchecked + fields hidden when capable boards exist; checked + fields shown otherwise; `mac_toggle_advanced` flips `setHidden:` on both fields from `ADVANCED_FIELDS`.
- [ ] **Save precedence**: `chosen` (first `state==1` radio → concrete `(u16,u16)`) first, else `manual` (two fields → `parse_id_field` each), else `current_config`; `render_config_body` + `atomic_write`; parse error ⇒ `show_macos_error_message`.
- [ ] **Mode-A doc-comment** cites `spec/DEVICE_DISCOVERY.md` §5 + `spec/UI.md` §2.2; notes manual layout (vs NSStackView) + no-Rescan + NSRadioButton deprecation.

### Code Quality Validation
- [ ] All new code is `#[cfg(target_os = "macos")]` (G1).
- [ ] Uses the legacy `objc = 0.2.7` API (G2) — `Class::get`/`class!`/`msg_send!`/`sel!`/`declare::ClassDecl`/`runtime::{Object,Sel,Class,YES,NO}`.
- [ ] `picker_row_text` + `test_picker_row_text_glyphs` are macOS-gated (G9) — no collision with S1's windows-gated versions.
- [ ] No explicit `release` of new objects (G7); enum args passed as `u64` (G6); state read as `isize` (G6).
- [ ] `ADVANCED_FIELDS` is the ONLY new static (G8); `chosen`/`manual` are locals (no `DIALOG_RESULT` static on macOS).
- [ ] Follows existing msg_send / ClassDecl / window-info-label idioms; no new Cargo deps.

### Documentation & Deployment
- [ ] Code is self-documenting with clear variable/function names.
- [ ] Doc-comment (Mode A) present on `show_settings_dialog_with_pool`.
- [ ] No new environment variables; no config schema change.

---

## Anti-Patterns to Avoid

- ❌ Don't widen `picker_row_text`'s cfg to `any(macos, windows)` — it collides with S1's windows-gated version on Windows builds (G9).
- ❌ Don't add a `[ Rescan ]` button on macOS — `runModal` blocks the thread; there's no dialog-open window to re-scan within (G3).
- ❌ Don't use `objc2`/`objc2-foundation` APIs in the dialog — this file uses the legacy `objc = 0.2.7` crate (G2).
- ❌ Don't pass `setButtonType`/`setLineBreakMode` enum args as raw `i32` — use `u64` (the file's idiom, G6).
- ❌ Don't explicitly `release` the alert/fields/rows/toggle — the pool + alert retention handle them (G7).
- ❌ Don't create a `DIALOG_RESULT` static on macOS — the dialog is synchronous; `chosen`/`manual` are locals (the Windows static is only needed because the Win32 WndProc is a free fn).
- ❌ Don't skip the chosen-first-else-manual precedence — a radio pick must win over the (prefilled) typed fields.
- ❌ Don't catch all exceptions — be specific (parse_id_field `Err` ⇒ `show_macos_error_message`; all other failures propagate as `Err` from the fn).
- ❌ Don't touch the Windows dialog, the macOS window-info dialog, `linux_tray.rs`, `Cargo.toml`, `core/`, or `spec/` — macOS-only single-file change.
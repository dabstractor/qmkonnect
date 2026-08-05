# Research — P3.M2.T1.S2: macOS NSAlert picker + Advanced toggle

Verification of the EXACT current code (read from `src/tray.rs` @ commit under
change) + the external AppKit API surface + the locked design decisions. Every
line number below was confirmed by reading the file.

---

## §1 The macOS settings dialog today (verbatim edit sites)

### Entry — `show_macos_settings_dialog` @1188 (UNCHANGED by this task)
- `#[cfg(target_os = "macos")]`, signature `(config_path: &Path) -> Result<...>`.
- Wraps in `NSAutoreleasePool` (`Class::get("NSAutoreleasePool")`, `msg_send![pool_class, new]`,
  then `msg_send![pool, drain]` at the end). Background `LSUIElement` apps lack a
  main pool — this is the ONLY autorelease scope for the dialog.
- Calls `show_settings_dialog_with_pool(config_path)` between pool new/drain.
- **This task does NOT touch the entry fn** — only `show_settings_dialog_with_pool`.

### Core — `show_settings_dialog_with_pool` @1212 (THE function to restructure)
```rust
#[cfg(target_os = "macos")]
fn show_settings_dialog_with_pool(config_path: &std::path::Path)
    -> Result<(), Box<dyn std::error::Error>> {
    use objc::runtime::{Class, Object};
    use objc::{msg_send, sel, sel_impl};

    let current_config = match crate::core::parse_config(config_path) {
        Ok(config) => config,
        Err(_) => crate::core::Config::default(),
    };

    unsafe {
        // 1. NSAlert
        let alert_class = Class::get("NSAlert").ok_or("...")?;
        let alert: *mut Object = msg_send![alert_class, new];
        let title = create_nsstring("QMK Settings")?;
        let message = create_nsstring(&format!(
            "Current Configuration:\nVendor ID: {}\nProduct ID: {}\n\nEnter new hex values, or leave a field blank for auto-discovery:",
            format_id_hex(current_config.vendor_id),
            format_id_hex(current_config.product_id)
        ))?;
        let _: () = msg_send![alert, setMessageText: title];
        let _: () = msg_send![alert, setInformativeText: message];

        // 2. OK + Cancel buttons
        let ok = create_nsstring("OK")?;
        let cancel = create_nsstring("Cancel")?;
        let _: *mut Object = msg_send![alert, addButtonWithTitle: ok];
        let _: *mut Object = msg_send![alert, addButtonWithTitle: cancel];

        // 3. Two NSTextFields (the CURRENT primary surface — to be relocated under "Advanced")
        let textfield_class = Class::get("NSTextField").ok_or("...")?;
        let vendor_field: *mut Object = msg_send![textfield_class, new];
        let vendor_value = create_nsstring(&format_id_hex(current_config.vendor_id))?;
        let _: () = msg_send![vendor_field, setStringValue: vendor_value];
        let _: () = msg_send![vendor_field, setFrame: objc_types::NSRect {
            origin: objc_types::NSPoint { x: 0.0, y: 0.0 },
            size: objc_types::NSSize { width: 100.0, height: 22.0 }
        }];
        let product_field: *mut Object = msg_send![textfield_class, new];
        let product_value = create_nsstring(&format_id_hex(current_config.product_id))?;
        let _: () = msg_send![product_field, setStringValue: product_value];
        let _: () = msg_send![product_field, setFrame: objc_types::NSRect {
            origin: objc_types::NSPoint { x: 0.0, y: 30.0 },
            size: objc_types::NSSize { width: 100.0, height: 22.0 }
        }];

        // 4. Container NSView 200x60
        let view_class = Class::get("NSView").ok_or("...")?;
        let container_view: *mut Object = msg_send![view_class, new];
        let _: () = msg_send![container_view, setFrame: objc_types::NSRect {
            origin: objc_types::NSPoint { x: 0.0, y: 0.0 },
            size: objc_types::NSSize { width: 200.0, height: 60.0 }
        }];
        let _: () = msg_send![container_view, addSubview: vendor_field];
        let _: () = msg_send![container_view, addSubview: product_field];
        let _: () = msg_send![alert, setAccessoryView: container_view];

        // 5. runModal — 1000 = OK, 1001 = Cancel
        let response: isize = msg_send![alert, runModal];

        if response == 1000 {
            let v_ns: *mut Object = msg_send![vendor_field, stringValue];
            let p_ns: *mut Object = msg_send![product_field, stringValue];
            let vendor_str = nsstring_to_rust_string(v_ns)?;
            let product_str = nsstring_to_rust_string(p_ns)?;
            match (parse_id_field(&vendor_str), parse_id_field(&product_str)) {
                (Ok(vendor_id), Ok(product_id)) => {
                    let mut merged = current_config;
                    merged.vendor_id = vendor_id;
                    merged.product_id = product_id;
                    let config_content = crate::core::render_config_body(&merged);
                    crate::core::atomic_write(config_path, &config_content)?;
                }
                (Err(e), _) | (_, Err(e)) => {
                    show_macos_error_message(&format!("Invalid input: {}", e));
                }
            }
        }
    }
    Ok(())
}
```

**KEY OBSERVATIONS:**
- The dialog is **synchronous**: `runModal` BLOCKS the tray thread until OK/Cancel.
  Everything (build accessory view → runModal → read result → save) happens in ONE
  function scope → **NO static DIALOG_RESULT is needed** (unlike Windows' free
  WndProc). `chosen` and `manual` can be plain locals.
- `runModal` returns `isize`. `1000` = `NSAlertFirstButtonReturn` (OK); `1001` =
  `NSAlertSecondButtonReturn` (Cancel).
- `current_config` is read at dialog-open time (the merge base). Keep this — it is
  the "leave VID/PID as-is" fallback base.
- The two fields are created with `msg_send![textfield_class, new]` and NEVER
  explicitly released — they survive the modal via the pool + the alert's retention
  of the accessory view. **Follow this convention for the new rows + toggle button
  too** (do NOT add explicit `release` calls; they would over-release / complicate).

### Helpers used (all `#[cfg(target_os = "macos")]`, UNCHANGED)
- `create_nsstring(s: &str) -> Result<*mut Object, ...>` @1316 — `NSString
  stringWithUTF8String:`. Reused for ALL runtime-built strings (row titles, header).
- `nsstring_to_rust_string(nsstring: *mut Object) -> Result<String, ...>` @1336 —
  reads `UTF8String`. Reused to read the two fields on OK.
- `show_macos_error_message(message: &str)` @1354 — `NSAlert` critical (style 2).
  Reused for parse errors (UNCHANGED error path).
- `format_id_hex(id: Option<u16>) -> String` @61 — `{:04x}` or `"auto"`. Reused for
  the message text + field prefill.
- `parse_id_field(input: &str) -> Result<Option<u16>, ...>` @70 — shared
  windows+macos; empty/`"auto"` ⇒ `Ok(None)`. Reused for the two fields.

### objc_types module @13-34
```rust
#[cfg(target_os = "macos")]
mod objc_types {
    #[repr(C)] pub struct NSPoint { pub x: f64, pub y: f64 }      // origin
    #[repr(C)] pub struct NSSize { pub width: f64, pub height: f64 }
    #[repr(C)] pub struct NSRect { pub origin: NSPoint, pub size: NSSize }
    // all #[derive(Debug, Copy, Clone)]
}
```
Used via `msg_send![view, setFrame: objc_types::NSRect { origin: NSPoint{x,y}, size: NSSize{width,height} }]`.
**Reuse this for every new NSView/NSButton/NSTextField frame.**

---

## §2 The target/action pattern (template for the Advanced toggle)

`show_macos_window_info_dialog_inner` @2213 registers an Obj-C subclass with a
Rust `extern "C" fn` method — this is the EXACT template for the Advanced
toggle's `toggleAdvanced:` action.

### The extern "C" callback @2228
```rust
extern "C" fn wi_copy_row(
    _this: &objc::runtime::Object,
    _sel: objc::runtime::Sel,
    sender: *mut objc::runtime::Object,
) {
    use objc::{msg_send, sel, sel_impl};
    let idx: isize = unsafe { msg_send![sender, tag] };   // read sender's tag
    let text = WINDOW_INFO_ROWS.lock().unwrap().get(idx as usize)  // read STATIC
        .map(copy_text_for_row);
    if let Some(t) = text { copy_to_pasteboard_macos(&t); }
}
```
- `sender` is the NSButton that was clicked (set as its target via `setTarget:`,
  action via `setAction: sel!(copyRow:)`).
- It reads `sender`'s `tag` (an integer we set per-row) + a **STATIC** for row data
  (an extern fn cannot capture Rust locals).

### The class registration @2224
```rust
use objc::{class, declare::ClassDecl, msg_send, sel, sel_impl};
use objc::runtime::{Class, Object, NO, YES};

if Class::get("RustWindowInfoCopyTarget").is_none() {       // register ONCE
    let superclass = Class::get("NSObject").ok_or("NSObject class not found")?;
    let mut decl = ClassDecl::new("RustWindowInfoCopyTarget", superclass)
        .ok_or("failed to declare RustWindowInfoCopyTarget")?;
    decl.add_method(
        sel!(copyRow:),
        wi_copy_row as extern "C" fn(&Object, objc::runtime::Sel, *mut Object),
    );
    decl.register();
}
let target: *mut Object = msg_send![
    Class::get("RustWindowInfoCopyTarget").ok_or("...")?,
    new
];
// then: msg_send![button, setTarget: target]; msg_send![button, setAction: sel!(copyRow:)];
```

**This task mirrors it as `RustMacSettingsTarget` with `toggleAdvanced:`.** The
toggle action reads the Advanced checkbox's `state` + the two field pointers from
a **STATIC** (`ADVANCED_FIELDS`), and calls `setHidden:` on both.

### YES / NO / BOOL in this codebase
- Imported from `objc::runtime::{YES, NO}` (used @2220, and `setBezeled: NO` /
  `setDrawsBackground: NO` in the window-info label @2300). Pass directly to
  `msg_send![obj, setHidden: YES]` / `... NO`.

---

## §3 The consumed classifier API (P3.M1 — Complete, read-only)

From `src/core/notifier.rs` (all `pub`, all `#[allow(dead_code)]` until a consumer
exists — **this task is a consumer**):
```rust
pub fn classify_devices(verbose: bool) -> Vec<ClassifiedDevice>   // @1116
pub fn classification_cache_clear()                                // @917 (NOT used by macOS)
pub enum DeviceKind {                                              // @816
    Capable { proto_ver: u8, feature_flags: u8, callback_count: u8, board_rules_present: bool },
    NotQmkNotifier,
}
pub struct ClassifiedDevice {                                      // @841
    pub path: String, pub vendor_id: u16, pub product_id: u16,
    pub product_name: Option<String>, pub usage_page: u16, pub usage: u16,
    pub kind: DeviceKind,
}
```
- `vendor_id`/`product_id` are `u16` (always present) ⇒ a row pick yields a
  concrete `(u16, u16)` — that is `chosen`.
- `product_name: Option<String>` — the live HID-descriptor name (may be `None` ⇒
  render `"(unnamed)"`). **No curated database** (spec §5.1).
- `kind` is the ✓/✗ discriminator: `Capable{..}` ⇒ `✓` + "qmk_notifier";
  `NotQmkNotifier` ⇒ `✗` + "QMK board, no module".
- Call `crate::core::notifier::classify_devices(true)` (verbose=true → diagnostic
  eprintln; mirrors the Windows sibling).

---

## §4 External AppKit API research (objc 0.2.7 legacy crate)

### NSButton button-type integers (classic `NSButtonType` enum)
| Constant                  | Int | Use here                               |
|---------------------------|-----|----------------------------------------|
| `NSMomentaryLightButton`  | 0   | —                                      |
| `NSPushOnPushOffButton`   | 1   | —                                      |
| `NSToggleButton`          | 2   | —                                      |
| `NSSwitchButton`          | 3   | **Advanced toggle (checkbox)**         |
| `NSRadioButton`           | 4   | **device-row radios**                  |

- Set via `msg_send![btn, setButtonType: 4u64]` (radio) / `3u64` (checkbox). The
  codebase passes integer enum args as `u64` (see `setLineBreakMode: 3u64` @2303,
  `setImagePosition: 1u64` @2343). **`u64` is the proven arg width here.**
- `NSControlState` values: `NSOnState = 1`, `NSOffState = 0`, `NSMixedState = -1`.
  Read via `let s: isize = msg_send![btn, state];`. A clicked radio/checkbox
  flips its OWN state automatically (no target/action required for the state
  change — the target/action is only for side effects like setHidden).
- **NSRadioButton sibling exclusivity**: AppKit radio buttons (`setButtonType:4`)
  placed as siblings in the SAME superview auto-enforce mutual exclusivity on
  click ("constrain a selection to a single element from several elements" — Apple
  `NSSwitchButton` docs). NSMatrix is NOT required for this classic behavior.
  **DEPRECATION NOTE**: Apple marks `NSRadioButton` deprecated in favor of
  NSSwitchButton + a coordinator, but it remains fully functional on all current
  macOS and AppKit does not remove classic UI primitives. For a beta app this is
  acceptable; the future-proof path (NSSwitchButton checkboxes + a `pickDevice:`
  coordinator on the same target class) is documented below as a non-goal for v1.
  SAFETY NET: on OK we take the FIRST `NSOnState` row regardless, so imperfect
  exclusivity never produces a wrong VID/PID.

### NSButton — set title / target / action / state / frame
```rust
let btn: *mut Object = msg_send![class!(NSButton), new];
let title = create_nsstring(&label)?;
let _: () = msg_send![btn, setTitle: title];
let _: () = msg_send![btn, setButtonType: 4u64];          // radio
let _: () = msg_send![btn, setTag: i as isize];           // row index
let _: () = msg_send![btn, setFrame: objc_types::NSRect { ... }];
let _: () = msg_send![btn, setTarget: target];
let _: () = msg_send![btn, setAction: sel!(toggleAdvanced:)]; // (Advanced btn only)
let s: isize = msg_send![btn, state];                     // 1 = on (read on OK)
```

### setHidden: (the toggle's side effect)
`msg_send![field, setHidden: YES]` / `msg_send![field, setHidden: NO]` — `YES`/`NO`
from `objc::runtime`. The toggle action inverts based on the checkbox `state`.

### NSStackView — the spec-named widget (NOT used; manual layout chosen)
The spec (§5.3) names `NSStackView`. Its API with the legacy crate:
```rust
let stack: *mut Object = msg_send![Class::get("NSStackView")?, new];
let _: () = msg_send![stack, setOrientation: 1u64];   // NSUserInterfaceLayoutOrientationVertical
let _: () = msg_send![stack, addView: row inGravity: 0u64];  // NSStackViewGravityTop
```
**Decision: use MANUAL vertical layout in a plain NSView instead.** Reasons:
1. The existing settings dialog ALREADY uses manual `setFrame` in a plain NSView
   (vendor_field y=0, product_field y=30) — this task extends that exact pattern.
2. NSStackView with the legacy `objc` crate needs Auto Layout constraints
   (`setTranslatesAutoresizingMaskIntoConstraints:NO` + explicit
   constraints via NSLayoutConstraint / Visual Format Language) to size correctly
   inside an NSAlert accessory view; building constraints via raw `msg_send!` is
   fragile and a one-pass failure risk. Manual frames are deterministic.
3. The contract explicitly allows it: "NSStackView **(or vertically-arranged
   NSView)**". The spec §5.3 row SEMANTICS (one row per device, ✓/✗ glyph, vid:pid,
   name, selectable) are fully honored; only the layout container differs.
   **Document this choice in the dialog doc-comment** (Mode A).

---

## §5 Locked design decisions (rationale)

1. **No `[ Rescan ]` button on macOS.** `runModal` BLOCKS the tray thread — there
   is no "dialog open" window during which a board could be flashed and re-scanned
   (unlike Windows' modal `GetMessageW` loop, where S1 added Rescan). The macOS
   contract (item description) deliberately omits Rescan. `classify_devices(true)`
   is called ONCE before building the accessory view. **Deviation from §5.1's
   generic `[ Rescan ]` line — documented as platform-specific (modal runModal).**

2. **Rows = NSButton radio (title = label).** The contract says "NSTextField
   labels" + "Selection writes the chosen vid/pid" — a non-interactive label
   cannot be "selected". NSButton radio titled with the label text IS a labeled,
   selectable row (radio dot + text). This is the robust realization in a modal
   NSAlert. Read `state` on OK; take first `NSOnState`.

3. **Advanced toggle = NSButton checkbox (NSSwitchButton=3)** + a registered
   `RustMacSettingsTarget` class with `toggleAdvanced:` (mirrors `wi_copy_row`).
   Default state: **unchecked + fields HIDDEN** when capable boards exist;
   **checked + fields SHOWN** when no capable boards (so the user can type). The
   `ADVANCED_FIELDS` static carries the two field pointers to the extern fn.

4. **No `DIALOG_RESULT` static on macOS.** Synchronous read-after-runModal ⇒
   `chosen: Option<(u16,u16)>` + the two `parse_id_field` results are locals. The
   "shared DIALOG_RESULT" from spec §5.3 is honored as a SEMANTIC shape
   (`{chosen, manual}`), not a shared static. Only `ADVANCED_FIELDS` is static
   (the extern toggle fn cannot see locals).

5. **`picker_row_text` is macOS-gated** (`#[cfg(target_os = "macos")]`), NOT
   `#[cfg(any(macos, windows))]`. The parallel sibling S1 adds a WINDOWS-gated
   `fn picker_row_text`. They are mutually exclusive by `target_os`, so there is
   **NO symbol collision** (exactly one compiles per platform). Same for the test
   `test_picker_row_text_glyphs` (macOS-gated here vs windows-gated in S1). This
   is the safe integration contract with the in-flight S1.

6. **Three picker cases** (spec §5.1), determined from `devices` + open-time config:
   - `devices.is_empty()` → header "No QMK keyboards detected. Use Advanced to enter IDs manually."; no rows; Advanced shown.
   - clean-auto (`len==1 && Capable && open vid/pid both None`) → header `Detected: <name>`; no rows; Advanced hidden.
   - else (≥2 boards, or 1 non-capable board) → header "Detected keyboard(s) — choose one:"; one radio row per device; Advanced hidden.

7. **Save precedence**: `chosen` first (a radio row ⇒ concrete `(u16,u16)`), else
   `manual` (the two fields ⇒ `Option<u16>` each), else leave `current_config`'s
   VID/PID (the merge base). Mirrors spec §5.3 + S1 exactly.

8. **Memory**: do NOT add explicit `release` for the new alert/fields/rows/toggle.
   Follow the existing settings-dialog convention (pool drain + alert retention).
   The target object created via `new` is retained-but-unowned (same as the
   window-info target @2260) — a negligible single-object leak per rare dialog
   open, accepted for beta (matches existing code).
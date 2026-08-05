# Research — P3.M2.T1.S3: Linux zenity `--list` picker + Advanced `--forms`

> Repo: the **qmkonnect** desktop app (Rust) at `/home/dustin/projects/qmkonnect`.
> This task restructures the **Linux** SNI Settings dialog
> (`show_settings_dialog_linux` in `src/linux_tray.rs`) so a live, self-populating
> **picker** of discovered devices (`classify_devices`) becomes the primary
> surface, and the legacy VID/PID `--forms` entries become the "Advanced / manual
> override" fallback. Source of truth: **`spec/DEVICE_DISCOVERY.md` §5** +
> **`spec/UI.md` §2.0/§2.3/§2.4**. Single-file change: `src/linux_tray.rs`.
>
> **PARALLEL-SAFETY:** the in-flight sibling **P3.M2.T1.S2 edits `src/tray.rs`
> (macOS)** — it does NOT touch `linux_tray.rs`. No task edits `linux_tray.rs`
> concurrently, so **the line numbers below are stable** (no "navigate by name"
> caveat needed, unlike S1/S2). S1 (Windows) is already Complete.

---

## 1. The exact current `show_settings_dialog_linux` (verbatim, @688)

```rust
/// Settings dialog: `zenity --forms` collecting Vendor/Product IDs, validated as
/// hex and written to `config.toml` on save — parity with the macOS/Windows
/// Settings entry.
///
/// The monitor + device-status poll re-read config on every notification / poll,
/// so a save takes effect within ~3 s with no restart needed.
fn show_settings_dialog_linux() {
    // Pre-fill the header with the current configured values so the user knows
    // what they're changing. (zenity --forms entries can't be pre-populated, so
    // the current values are shown in the dialog text.)
    let (cur_vid, cur_pid) = current_config_hex();
    let text = format!(
        "QMK keyboard VID/PID\n\
         Current: vendor_id = 0x{cur_vid}   product_id = 0x{cur_pid}\n\
         Enter new hex values (the 0x prefix is optional):"
    );

    let output = Command::new("zenity")
        .args([
            "--forms",
            "--title=QMK Settings",
            &format!("--text={text}"),
            "--add-entry=Vendor ID (hex)",
            "--add-entry=Product ID (hex)",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

    let out = match output {
        Ok(o) if o.status.success() => o,
        Ok(_) => return, // user cancelled or closed the dialog
        Err(e) => {
            eprintln!("Settings: could not launch zenity: {}", e);
            notify(
                "QMKonnect — Settings unavailable",
                "Install zenity, or edit config.toml directly.",
            );
            return;
        }
    };

    // zenity --forms prints field values separated by '|' on stdout.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut parts = stdout.trim_end_matches('\n').split('|');
    let vid_in = parts.next().unwrap_or_default().trim();
    let pid_in = parts.next().unwrap_or_default().trim();

    let (vid, pid) = match (parse_id(vid_in), parse_id(pid_in)) {
        (Ok(v), Ok(p)) => (v, p),
        _ => {
            notify(
                "QMKonnect — invalid input",
                "Vendor/Product ID must be hex (e.g. feed / 0000), or blank for auto-discovery.",
            );
            return;
        }
    };

    let vid_str = vid
        .map(|v| format!("0x{v:04x}"))
        .unwrap_or_else(|| "auto".to_string());
    let pid_str = pid
        .map(|p| format!("0x{p:04x}"))
        .unwrap_or_else(|| "auto".to_string());

    match write_config(vid, pid) {
        Ok(path) => {
            // Apply the device rule. Both-unset needs no rule (the static
            // usage-page rule covers any 0xFF60/0x61 device); at least one set
            // -> install the on-demand VID/PID fallback rule privileged.
            let outcome = apply_device_rule(vid, pid);
            let detail = match outcome {
                ApplyOutcome::AutoDiscovery => {
                    "Auto-discovery in effect (any standard QMK keyboard).".to_string()
                }
                ApplyOutcome::Applied => "Device rule applied.".to_string(),
                ApplyOutcome::NeedsManual(how) => how,
            };
            notify(
                "QMKonnect — settings saved",
                &format!(
                    "vendor_id = {vid_str}, product_id = {pid_str}\n{detail}\n{}",
                    path.display()
                ),
            );
        }
        Err(e) => {
            eprintln!("Settings: failed to write config: {}", e);
            notify("QMKonnect — could not save", &e.to_string());
        }
    }
}
```

**What stays / what changes:**
- KEEP: the `zenity --forms` invocation + its `'|'`-split parse + `parse_id` each
  + the invalid-input notify + `write_config` → `apply_device_rule` → notify block.
- ADD (before the `--forms`): a `classify_devices(true)` call + a 3-case decision +
  (when the picker case applies) a `zenity --list` invocation whose selection, if
  any, short-circuits straight to `write_config` (skipping the `--forms`).
- REFACTOR: extract the `write_config` → `apply_device_rule` → notify tail into a
  `save_and_notify(vid, pid)` helper so BOTH paths (picker pick / manual --forms)
  share it. Pure extraction; behavior byte-identical.

### Neighbor helpers (all in `src/linux_tray.rs`, UNCHANGED except current_config_hex)

```rust
/// @778 — the outcome enum (applied by apply_device_rule).
enum ApplyOutcome { AutoDiscovery, Applied, NeedsManual(String) }

/// @795 — render+stage+pkexec the VID/PID udev rule. Both None -> no rule; Some -> pkexec.
fn apply_device_rule(vendor_id: Option<u16>, product_id: Option<u16>) -> ApplyOutcome { ... }

/// @838 — current VID/PID as lowercased hex ("feed") or "auto".
fn current_config_hex() -> (String, String) { ... }

/// @859 — overlay VID/PID onto parsed config, render_config_body, atomic_write.
fn write_config(vendor_id: Option<u16>, product_id: Option<u16>)
    -> Result<std::path::PathBuf, Box<dyn std::error::Error>> { ... }

/// @883 — trim; empty/"auto" -> None; 0x-tolerant; u16 radix-16; else Err.
fn parse_id(input: &str) -> Result<Option<u16>, Box<dyn std::error::Error>> { ... }

/// @899 — best-effort notify-send.
fn notify(summary: &str, body: &str) { ... }
```

`current_config_hex` returns **display strings** ("auto"/"feed"). The clean-auto
check needs the **raw `Option<u16>`** values. **ADD** a sibling helper
`current_config_vidpid() -> (Option<u16>, Option<u16>)` and refactor
`current_config_hex` to derive its strings from it (3-line change; keeps the
config-read in one place). Both helpers read the first existing config candidate
via `crate::platforms::get_config_paths()` (the established pattern, see
`write_config` @859 + `current_config_hex` @838).

---

## 2. The consumer API (verified in-tree, `src/core/notifier.rs`) — READ-ONLY

All three are `pub` and currently `#[allow(dead_code)]` (no consumer yet on Linux;
this task is that consumer — the `allow` becomes satisfied, LEAVE it).

```rust
// notifier.rs:1116 — enumerate Tier-1 (0xFF60/0x61) + per-candidate QUERY_INFO +
// 5s-TTL per-path cache. verbose=true -> diagnostic eprintln.
pub fn classify_devices(verbose: bool) -> Vec<ClassifiedDevice>;

// notifier.rs:816 — the ✓/✗ discriminator.
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceKind {
    Capable { proto_ver: u8, feature_flags: u8, callback_count: u8, board_rules_present: bool },
    NotQmkNotifier,
}

// notifier.rs:841 — one enumerated Tier-1 interface. vendor_id/product_id are
// u16 (ALWAYS present) -> a picker pick yields a concrete (u16,u16).
// product_name may be None -> "(unnamed)".
#[derive(Debug, Clone, PartialEq)]
pub struct ClassifiedDevice {
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub product_name: Option<String>,
    pub usage_page: u16,
    pub usage: u16,
    pub kind: DeviceKind,
}

// notifier.rs:917 — drains CLASSIFICATION_CACHE. NOT called on Linux Settings-open
// (classify_devices reads the warm cache; re-open after 5s TTL = fresh probe).
pub fn classification_cache_clear();
```

**Import in `linux_tray.rs`** (module is already `#[cfg(all(target_os="linux",
feature="linux-tray"))]`, so these are implicitly Linux-gated):
`use crate::core::notifier::{classify_devices, ClassifiedDevice, DeviceKind};`
(already imports `DeviceStatus` from the same module @line 5).

---

## 3. zenity `--list` mechanics (pinned from the GNOME man page)

**Man page:** https://commandlinux.com/man-page/man1/zenity/ + https://linux.die.net/man/1/zenity

| Behavior | Detail |
|---|---|
| **Columns + values** | `zenity --list --column="A" --column="B" --column="C" v1A v1B v1C v2A v2B v2C` — values fill columns **left→right, row by row**. Pass each value as a **separate argv element** to `Command::new("zenity").args([...])` (no shell quoting; `✓` glyph + spaces are fine — this codebase already passes `--text=...` with spaces/em-dashes as one arg, and the window-info `--forms` passes `--list-values=a|b|c` similarly). |
| **`--print-column=N`** | "Specify what column to print to standard output. **The default is to return the first column.** 'ALL' may be used to print all columns." → `--print-column=2` prints **only** the VID:PID cell of the selected row. |
| **Exit codes** | OK-with-selection → exit **0** + stdout = the selected cell. Cancel/close → exit **1** (empty stdout). OK-with-no-selection → exit **0** + **empty** stdout (treat as no-selection). ⇒ guard: **`status.success() && !stdout.trim().is_empty()`** = a real pick. |
| **Selection mode** | Plain `--list` (NO `--checklist`) is **single-select**: click a row to highlight, click OK. No `--radiolist` needed. |
| **Sizing** | `--width`/`--height` DO affect a `--list` (it is a real `GtkTreeView`, unlike the height-capped embedded list inside `--forms`). For a handful of device rows `--width=520` keeps the 3 columns readable; height auto-sizes to the row count. |
| **Window type** | `--list` creates a **normal toplevel** window → tiling WMs (Sway/i3/hyprland) **tile** it by default (it does NOT float like `--forms` does). See §6 for the documented tradeoff. |

### The exact picker argv (3 columns: Device / VID:PID / Capability, print col 2)

```rust
// Build argv: the flags first, then the 3 column headers, then N×3 values.
let mut args: Vec<String> = vec![
    "--list".into(),
    "--title=QMK Settings".into(),
    "--text=Select a detected keyboard:".into(),
    "--print-column=2".into(),          // print the VID:PID cell of the selected row
    "--hide-header".into(),             // optional; headers add noise for 1-3 rows
    "--width=520".into(),
    "--column=Device".into(),
    "--column=VID:PID".into(),
    "--column=Capability".into(),
];
for d in &devices {
    let (name, vidpid, cap) = picker_columns(d);
    args.push(name);
    args.push(vidpid);
    args.push(cap);
}
let output = Command::new("zenity")
    .args(args.iter().map(String::as_str))
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .output();
```

**Parse-back** the printed "0xFEED:0x0000" → `(u16, u16)` via a new
`parse_vidpid` helper (splits on `':'`, `parse_id` each half; returns
`Option<(u16,u16)>`). That `(vid,pid)` is `chosen`.

---

## 4. The 3-case decision + the chosen-first-else-manual precedence

The Linux two-dialog model has **no shared struct/static** (unlike Windows'
`DIALOG_RESULT`): the dialogs run sequentially and the result is a plain local.
The precedence is **chosen (from `--list`) → manual (from `--forms`) → no-write
(cancel)**, which is the Linux equivalent of the siblings' chosen-first-else-
manual-else-as-is ("as-is" = "leave config alone" = don't write).

```text
show_settings_dialog_linux():
  devices = classify_devices(true)
  (cur_vid, cur_pid) = current_config_vidpid()           // (Option<u16>, Option<u16>)

  clean_auto = devices.len()==1
      && matches!(devices[0].kind, Capable{..})
      && cur_vid.is_none() && cur_pid.is_none()

  ┌─ CASE A: devices.is_empty() ───────────────────────────────────┐
  │ picker = false. Go straight to the --forms (Advanced) with text │
  │ "No QMK keyboards detected. Enter IDs manually below."         │
  │ chosen=None; manual = whatever the user types (or current).    │
  └─────────────────────────────────────────────────────────────────┘
  ┌─ CASE B: clean_auto (1 capable + no VID/PID) ──────────────────┐
  │ picker = false (ZERO-CONFIG: nothing to choose). Go straight   │
  │ to the --forms (Advanced) with text "Detected: <name>.         │
  │ Auto-discovery is active." chosen=None; manual = current=None. │
  │ ⇒ OK writes None/None (auto) — correct, no spurious VID/PID.   │
  └─────────────────────────────────────────────────────────────────┘
  ┌─ CASE C: picker (≥2 Tier-1, OR 1 non-capable, OR 1 capable but VID/PID set) ┐
  │ Run `zenity --list` (the picker).                                            │
  │  • selection (success + non-empty stdout):                                   │
  │        chosen = parse_vidpid(stdout)  // Some((vid,pid))                     │
  │        save_and_notify(Some(v), Some(p)); return;   // SKIP the --forms      │
  │  • cancel / no-selection:                                                    │
  │        chosen = None; FALL THROUGH to the --forms (Advanced / manual).       │
  └──────────────────────────────────────────────────────────────────────────────┘

  ── THE --forms (Advanced / manual override), reached by A, B, or C-fallthrough ──
  text reflects the case (A: "No QMK keyboards detected…"; B: "Detected: <name>…";
   C-fallthrough: "Manual override — enter hex VID/PID").
  • OK  -> parse_id each -> manual=(vid,pid); save_and_notify(vid,pid); return.
  • Cancel/close -> return (no write).   // existing behavior, preserved
```

**`save_and_notify(vendor_id: Option<u16>, product_id: Option<u16>)`** — extracted
verbatim from the current inline tail (`write_config` → `apply_device_rule` →
notify / error-notify). Both the picker pick and the manual --forms call it.
`apply_device_rule`/pkexec flow is UNCHANGED (contract requirement).

### Why "auto-select" in CASE B does NOT write a VID/PID
The contract says "if exactly one capable device and no VID/PID set, show
'Detected: <name>' and auto-select." Per `DEVICE_DISCOVERY.md` §5.1: *"One capable
board, no VID/PID set (the common case): the header reads `Detected: <name>` and
**no picker is shown. Auto-discovery is already correct; there is nothing to
choose.**"* So "auto-select" = **skip the picker** (the single device is the
implicit selection, but since auto-discovery already targets it, **no VID/PID is
written**). The --forms still opens (so the user CAN manually override), with a
"Detected: <name>. Auto-discovery is active." text. This mirrors the macOS/Windows
"static header + no picker, Advanced fields still present" behavior and preserves
the zero-config promise. Document this clearly in the doc-comment.

---

## 5. New pure helpers (the ONLY unit-testable pieces — zenity itself isn't)

```rust
/// One picker row's three columns: Device name, "0xVID:0xPID", capability glyph+status.
/// Pure; unit-tested. (`spec/DEVICE_DISCOVERY.md` §5.1 / §3.)
fn picker_columns(d: &ClassifiedDevice) -> (String, String, String) {
    let (glyph, status) = match d.kind {
        DeviceKind::Capable { .. } => ("\u{2713}", "qmk_notifier"),         // ✓
        DeviceKind::NotQmkNotifier => ("\u{2717}", "QMK board, no module"), // ✗
    };
    let name = d.product_name.as_deref().unwrap_or("(unnamed)").to_string();
    let vidpid = format!("0x{:04X}:0x{:04X}", d.vendor_id, d.product_id);
    let cap = format!("{glyph} {status}");
    (name, vidpid, cap)
}

/// Parse a zenity --list --print-column=2 selection ("0xFEED:0x0000") back to
/// (vid,pid). Returns None on any malformed input (no colon, non-hex, etc.).
/// Reuses `parse_id` (empty/auto -> None -> None here). Pure; unit-tested.
fn parse_vidpid(s: &str) -> Option<(u16, u16)> {
    let mut it = s.trim().splitn(2, ':');
    let vid = parse_id(it.next()?).ok()??;   // ?? : Result->Option, Option<u16>->u16
    let pid = parse_id(it.next()?).ok()??;
    Some((vid, pid))
}

/// The open-time VID/PID as raw Options (for the clean-auto check). current_config_hex
/// is refactored to derive its display strings from this.
fn current_config_vidpid() -> (Option<u16>, Option<u16>) {
    crate::platforms::get_config_paths()
        .into_iter()
        .find(|p| p.exists())
        .and_then(|p| crate::core::parse_config(&p).ok())
        .map(|cfg| (cfg.vendor_id, cfg.product_id))
        .unwrap_or((None, None))
}

/// Persist VID/PID + apply udev rule + notify. Extracted from the old inline tail
/// so the picker path and the manual --forms path share it. Behavior unchanged.
fn save_and_notify(vendor_id: Option<u16>, product_id: Option<u16>) { /* old tail */ }
```

### Tests to add (in `mod tests` @1006)
- `test_picker_columns`: `Capable{..}` ⇒ vidpid == `"0xFEED:0x0000"` + cap starts
  with `✓` + contains `qmk_notifier`; `NotQmkNotifier` ⇒ vidpid `"0x3434:0x0123"`
  + cap `✗` + `"QMK board, no module"`. (ClassifiedDevice derives Clone — use
  `..capable.clone()` for the notqmk variant.)
- `test_parse_vidpid`: `"0xFEED:0x0000"` → `Some((0xFEED,0))`; `"feed:0x123"` →
  `Some((0xFEED,0x123))`; `""` → None; `"feed"` → None (no colon); `"feed:"` →
  None; `":123"` → None; `"garbage:x"` → None; `"0xFEED:0x0000|extra"` → the
  `splitn(2,..)` leaves pid half as `"0x0000|extra"` which `parse_id` rejects
  (extra chars) → None. (Pin: splitn(2) so a stray `|` in the pid half fails
  safely rather than silently truncating.)

---

## 6. The tiling tradeoff (Mode-A documented deviation)

The window-info dialog (`show_window_info_linux` @383 + the code comment @387-392)
deliberately uses a **native GTK popup** (the `gtk_dialog` module) because:
> *"zenity `--forms` popup floats but caps the list at ~4 rows, and zenity `--list`
> is tall but tiles — neither is acceptable."*

This task uses `zenity --list` for the picker **anyway** because:
1. The **item contract explicitly chooses `--list`** ("for the picker we need
   `zenity --list` … this can work as a separate invocation BEFORE the `--forms`
   dialog"). `spec/DEVICE_DISCOVERY.md` §5.3 lists it as the Linux picker widget
   ("`zenity --list --column …` (the discovered list) + a second `zenity --forms`
   for the Advanced VID/PID; **or** the native GTK popup already used for
   window-info"). The contract picks the first option.
2. The device count is **tiny** (typically 1-3 keyboards), so the height cap of
   `--forms`-embedded-list is irrelevant and the tiling concern is mild (a 3-row
   list tiled in a corner is still usable; a 50-row window list is not — hence the
   different choice for window-info).
3. `--list` gives the **exact selection semantics** needed (single-select → print
   the chosen VID:PID → parse → write). A native GTK popup would reimplement
   that selection + return-value plumbing.

**Document this as a Mode-A note** in the `show_settings_dialog_linux` doc-comment:
the picker uses `zenity --list` (accepting that pure tiling WMs like Sway/i3 will
tile it rather than float it, since `--list` is a normal toplevel); the window-info
dialog avoids this via a native GTK popup, but that tradeoff is unjustified for the
small device list. A future enhancement could route the picker through the existing
`gtk_dialog` infra (out of scope — the contract scopes this to the zenity path).

---

## 7. Locked design decisions (rationale)

| # | Decision | Rationale |
|---|---|---|
| D1 | **Two sequential zenity dialogs** (`--list` THEN `--forms`), NOT combined | `--forms` floats but caps list height at ~4 rows; `--list` is the proper selection widget. Combining is impossible in one zenity call. Two calls is the contract's explicit design. |
| D2 | **`--print-column=2`** (VID:PID column), parse it back | The Device-name column isn't unique (None/duplicate names); VID:PID is the reliable key. Parse with the new `parse_vidpid`. |
| D3 | **No `[Rescan]` button** | The two dialogs are sequential (no "open dialog" window to click a button within, unlike Windows' message loop). `classify_devices(true)` is called once per `show_settings_dialog_linux()` invocation; **re-opening Settings refreshes** (after the 5s cache TTL, the probe re-runs). Mirrors macOS S2 (no Rescan; runModal blocks). |
| D4 | **Picker pick short-circuits the --forms** | Contract: "If the user selects a row, extract the vid/pid and proceed to write_config with those values (skip the manual --forms entry)." So a pick → `save_and_notify` → return; only cancel/no-selection falls through to --forms. |
| D5 | **`classify_devices(true)` reads the warm cache** (no `classification_cache_clear()` on open) | Parity with macOS S2 + Windows S1 initial-open (both call `classify_devices(true)` without clearing). The cache is warm from the status-poll thread + handshake (TTL 5s). Clearing on every open would add a ping-per-open, defeating the cache. |
| D6 | **`save_and_notify` helper** (pure extraction of the old tail) | Removes duplication between the picker path and the manual path; guarantees identical save+apply+notify behavior (incl. the `ApplyOutcome` notify detail). Low risk — byte-identical to the current inline block. |
| D7 | **`current_config_vidpid` helper** + refactor `current_config_hex` | The clean-auto check needs raw `Option<u16>` (the hex strings can't distinguish "auto" reliably for the logic). One config-read helper; `current_config_hex` derives from it. |
| D8 | **CASE B (clean-auto) does NOT write a VID/PID** | Zero-config promise (`DEVICE_DISCOVERY.md` §5.1). "auto-select" = skip the picker (auto-discovery already correct), NOT "write the board's VID/PID". The --forms still opens with a "Detected: <name>" note for manual override. |
| D9 | **Linux has no `DIALOG_RESULT` static** | Unlike Windows (whose proc is a free `extern "system" fn` needing a static) or macOS (whose `runModal` result is read inline), Linux's two zenity `Command::output()` calls are sequential synchronous blocks — the result is a plain local. No shared struct needed. |
| D10 | **Single-file change: `src/linux_tray.rs` only** | No `Cargo.toml` dep (zenity is shelled out; `std::process::Command` already used). No `tray.rs` (S1/S2), no `notifier.rs` (P3.M1 Complete), no `spec/*` (Mode-A owned by P4), no `docs/*` (P4.M2). |

---

## 8. Gotchas (G-series for Linux)

- **G1 (Linux-only module):** `linux_tray.rs` is
  `#![cfg(all(target_os = "linux", feature = "linux-tray"))]`. ALL new code
  inherits this gate. Zero impact on Windows/macOS builds. The new imports
  (`classify_devices`/`ClassifiedDevice`/`DeviceKind`) are from `notifier.rs`
  (cross-platform `pub`) but only referenced inside this gated module.
- **G2 (two sequential dialogs):** `--list` THEN `--forms`, separate invocations.
  Never combine into one zenity call. The picker is a SEPARATE dialog.
- **G3 (no Rescan):** sequential dialogs ⇒ no open-dialog window to click Rescan
  within. Re-open Settings to refresh (D3). Document in the doc-comment (Mode A).
- **G4 (zenity cancel = exit 1):** guard a pick as
  `status.success() && !stdout.trim().is_empty()`. Cancel/close → exit 1 →
  fall through to --forms. OK-with-no-selection → exit 0 + empty stdout → also
  fall through.
- **G5 (`--print-column=2`):** prints the VID:PID cell. Parse with `parse_vidpid`.
  Do NOT print column 1 (Device name — not unique) or column 3 (Capability).
- **G6 (tiling tradeoff):** `--list` is a normal toplevel → tiling WMs tile it
  (unlike `--forms` which floats). This is an accepted, Mode-A-documented tradeoff
  (§6), NOT a bug. The contract explicitly chooses `--list`.
- **G7 (blocking the ksni D-Bus thread):** the existing dialog ALREADY blocks the
  activate-thread (zenity `Command::output()` + `apply_device_rule`/pkexec are
  synchronous). `classify_devices(true)` adds one more short block (reads a warm
  cache ⇒ ~free). Consistent with the existing pattern; do NOT spawn a thread
  (out of scope; the contract scopes this to dialog logic only).
- **G8 (CASE B does NOT write a VID/PID):** "auto-select" = skip the picker, go
  to --forms with "Detected: <name>" text. The user OK-ing with blank/auto fields
  writes None/None (auto-discovery) — correct. NEVER auto-write the single
  capable board's VID/PID (that would break zero-config). Document (D8).
- **G9 (chosen vs manual types):** chosen = `(u16,u16)` from `parse_vidpid`;
  manual = `(Option<u16>,Option<u16>)` from `parse_id` each. `save_and_notify`
  takes `(Option<u16>,Option<u16>)`: chosen → `(Some(v),Some(p))`; manual → `(v,p)`.
- **G10 (save_and_notify extraction):** extract the `write_config` →
  `apply_device_rule` → notify / error-notify tail VERBATIM. Both paths call it.
  `apply_device_rule`/pkexec is UNCHANGED (contract). Pure refactor.
- **G11 (column argv as separate elements):** pass each column value as its own
  `args.push(...)` element (Command::new("zenity").args([...])), NOT a shell
  string. `✓` glyph + spaces are fine (no quoting needed — Rust's Command does
  not go through a shell). The existing `--text=…` with em-dashes proves this.
- **G12 (single-threaded tests, AGENTS.md):** `cargo test --bin qmkonnect --
  --test-threads=1`. The new pure helpers (`picker_columns`, `parse_vidpid`,
  `current_config_vidpid`) are unit-testable; the zenity `Command` invocations
  are NOT (they spawn a real GUI). Cover the helpers, not the shells.
- **G13 (refactor current_config_hex, don't delete it):** it's still used for the
  --forms display text. Derive it from `current_config_vidpid` so there's one
  config-read. Or keep them independent (minimal-risk path: add
  `current_config_vidpid`, leave `current_config_hex` as-is). Both are acceptable;
  the PRP prefers the derive for DRY but allows leaving current_config_hex alone.
- **G14 (the `--text` arg for --forms must reflect the case):** A/B/C-fallthrough
  each get a distinct informative text so the user understands what they're seeing
  ("No QMK keyboards detected…" / "Detected: <name>…" / "Manual override…"). The
  current values (from `current_config_hex`) still appear in all three.
# Research Notes — P1.M1.T1.S3: Linux SNI three-state status + Disconnected→NoModule one-shot notify-send

## Task nature

S3 is the **Linux half** of the three-state device status (F13 headline). S1
delivered `DeviceStatus` + `device_status()` (LANDED in notifier.rs:719/761). S2
renders it on macOS/Windows (parallel). S3 renders it on the Linux SNI tray
(`src/linux_tray.rs`) — which is a RICHER surface than S2: text + ICON DIMMING +
a one-shot `notify-send` on the Disconnected→NoModule transition. The crate
currently compiles (0 errors); S3's edits are the only changes.

## CRITICAL: linux_tray.rs is a richer surface than tray.rs (S2)

S2 (macOS/Windows) is TEXT-ONLY (no icon dim, no tooltip change). S3 (Linux) must:
1. **Menu status line** (3 branches, not 2).
2. **Icon alpha** — full-alpha for Connected AND NoModule (device present);
   dimmed (~35% via `dim_icon`/DIM_ALPHA=90) for Disconnected ONLY.
3. **Tooltip** — derives from state (realtime indicator).
4. **One-shot notify-send** on Disconnected→NoModule (NEW — no S2 equivalent).
5. **Poll thread dual-tracker** (mirror S2: handshake on bool, UI on DeviceStatus).

## S1 contract (LANDED — verified)

`src/core/notifier.rs`:
- `pub enum DeviceStatus { Connected, NoModule, Disconnected }` (line 719) with
  `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`.
- `pub fn device_status() -> DeviceStatus` (line 761) — no-arg, no HID I/O;
  reads `is_device_connected()` + `host_capable()`.
- `fn classify_device_status(present, capable)` (line 772) — private helper.
- `is_device_connected()`, `host_capable()`, `handshake_action()`,
  `perform_handshake()`, `reset_handshake_state()`, `startup_device_was_connected()`
  all UNCHANGED.

## S2 contract (parallel — the strings S3 must match for parity)

The three exact strings (from S2's `device_status_text`):
- Connected: `"\u{25CF}  Device Connected"` (● + 2 spaces)
- NoModule: `"\u{26A0}  QMK board found \u{2014} no qmk_notifier module (flash it)"` (⚠ + 2 spaces, em-dash \u{2014})
- Disconnected: `"\u{25CB}  No Device Connected"` (○ + 2 spaces)

S3 MUST render byte-identical strings (the parity test enforces this). S2 extracts
a `device_status_text(DeviceStatus) -> String` function in tray.rs; S3 should
EXTRACT THE SAME function in linux_tray.rs (currently the text is an INLINED
literal at menu():143-152) so (a) the parity test can assert on real output, not
standalone literals, and (b) the two files' same-named functions are verifiably
identical.

## The 11 `device_connected` sites in linux_tray.rs (verified by grep)

| Line | Site | Change |
|------|------|--------|
| 65 | doc-comment (QmkTray) | cite DEVICE_DISCOVERY.md §3 + one-shot rationale (Mode A) |
| 68 | field `device_connected: bool` | → `device_status: DeviceStatus` |
| 76 | `new()` seed | `is_device_connected()` → `device_status()` |
| 95 | `tool_tip()` description | 3-state (or present/absent) |
| 128 | `icon_pixmap()` dim | `self.device_status != Disconnected` → full alpha |
| 142 | comment | update prose (3 states) |
| 143 | `menu()` status line (inlined `if`) | → `device_status_text(self.device_status)` (extract fn) |
| 170 | `menu()` hidden structural toggle | `self.device_status != Disconnected` (present = Connected OR NoModule) |
| 258 | comment | minor |
| 267 | poll thread `is_device_connected()` | KEEP (bool for handshake) |
| 294 | poll thread `t.device_connected = connected` | → `t.device_status = status` + dual-tracker |
| 953 | test `new_tray_probes_initial_state` | `tray.device_connected` → `tray.device_status` |

## The poll thread (259-301) — dual-tracker design (mirror S2)

Current:
```rust
let mut last_device: Option<bool> = Some(startup_device_was_connected());
let mut last_dark: Option<bool> = None;
loop {
    let connected = is_device_connected();
    let dark = ...;
    if last_device != Some(connected) { handshake_action... }   // handshake (KEEP on bool)
    if last_device != Some(connected) || last_dark != Some(dark) {
        last_device = Some(connected); last_dark = Some(dark);
        poll_handle.update(|t| { t.device_connected = connected; t.dark_mode = dark; });
    }
    sleep(DEVICE_POLL_INTERVAL);
}
```

New (dual-tracker + one-shot notify):
```rust
let mut last_device: Option<bool> = Some(startup_device_was_connected());   // handshake (KEEP)
let mut last_status: Option<DeviceStatus> = Some(device_status());          // NEW: UI + notify
let mut last_dark: Option<bool> = None;
loop {
    let connected = is_device_connected();
    let status = device_status();   // computed AFTER would-be handshake; reads host_capable
    let dark = ...;
    // handshake on Tier-1 presence (UNCHANGED)
    if last_device != Some(connected) {
        match handshake_action(...) { ... }
        last_device = Some(connected);
    }
    // one-shot notify on Disconnected -> NoModule ONLY
    if last_status == Some(DeviceStatus::Disconnected) && status == DeviceStatus::NoModule {
        if !NO_MODULE_NOTIFIED.swap(true, Ordering::SeqCst) {   // fire once per NoModule entry
            notify(SUMMARY, BODY);
        }
    }
    // re-arm when leaving NoModule (so a later re-entry fires again)
    if status != DeviceStatus::NoModule {
        NO_MODULE_NOTIFIED.store(false, Ordering::SeqCst);
    }
    // tray UI on status OR dark transition
    if last_status != Some(status) || last_dark != Some(dark) {
        last_status = Some(status); last_dark = Some(dark);
        poll_handle.update(|t| { t.device_status = status; t.dark_mode = dark; });
    }
    sleep(DEVICE_POLL_INTERVAL);
}
```

KEY: the update condition MUST key on `last_status` (not `last_device`), else the
NoModule→Connected flip (bool stays true) never updates the tray. The handshake
stays keyed on `last_device` (bool) — exactly like S2.

## RULES_INVALID_NOTIFIED idiom (the once-guard model — notifier.rs:299/1161/1168)

```rust
static RULES_INVALID_NOTIFIED: AtomicBool = AtomicBool::new(false);   // :299
// ...on success (re-arm):
RULES_INVALID_NOTIFIED.store(false, Ordering::SeqCst);                 // :1161
// ...on first failure (fire once):
if !RULES_INVALID_NOTIFIED.swap(true, Ordering::SeqCst) {              // :1168
    notify(...);
}
```

S3 mirrors this as `static NO_MODULE_NOTIFIED: AtomicBool` in linux_tray.rs:
- `swap(true, SeqCst)` returns false the first time → fire notify; subsequent
  ticks return true → skip (one-shot per NoModule entry).
- `store(false, SeqCst)` when leaving NoModule → re-arms so a LATER
  Disconnected→NoModule re-entry fires again.

## notify() helper (846-859) — already exists, reuse it

```rust
fn notify(summary: &str, body: &str) {
    Command::new("notify-send").args(["--app-name=QMKonnect","--icon=input-keyboard", summary, body]).status()...
}
```
S3 calls `notify(SUMMARY, BODY)` from the poll thread. notify-rust is DELIBERATELY
avoided (nested tokio runtime panics in ksni's handler thread — spec §7.3). Do NOT
switch to notify-rust.

Per DEVICE_DISCOVERY.md §3: "the same message + a link to docs/qmk-integration.md".
Suggested:
- summary: `"QMK board found \u{2014} no qmk_notifier module"` (or the NoModule status text)
- body: `"This QMK board isn't running the qmk_notifier firmware QMKonnect talks to. Flash it: docs/qmk-integration.md"`

## Tests in linux_tray.rs (run on Linux CI — feature linux-tray default-on)

1. `status_text_uses_parity_glyphs` (948) — UPDATE: assert the three strings from
   the new `device_status_text` fn (3 glyph assertions: \u{25CF}/\u{26A0}/\u{25CB}).
2. `new_tray_probes_initial_state` (953) — UPDATE: `tray.device_connected` →
   `tray.device_status`.
3. `parse_id_handles_prefix_case_and_auto` (960) — UNCHANGED.

## Module-level import safety

linux_tray.rs is gated `#[cfg(all(target_os = "linux", feature = "linux-tray"))]`
(default-on on Linux). A module-level `use crate::core::notifier::DeviceStatus;`
is SAFE here (unlike tray.rs, where it'd be unused on non-Hyprland Linux) — the
whole module compiles or doesn't. Recommend the module-level `use` for terse
match arms. (The file currently fully-qualifies `crate::core::notifier::*`; the
`use` is an additive convenience for the new type only.)

## Sources verified
- S1 PRP + landed notifier.rs (DeviceStatus@719, device_status@761).
- S2 PRP (the three exact strings + dual-tracker pattern to mirror).
- tray_surfaces.md (Linux SNI map: struct@66, status@137, icon dim@156, poll@259, notify@846, DIM_ALPHA@923, parity test@948).
- Actual linux_tray.rs (all 11 device_connected sites + dim_icon@923 + tests@948/953).
- notifier.rs RULES_INVALID_NOTIFIED idiom (299/1161/1168).
- spec/DEVICE_DISCOVERY.md §3 (the three-state table + the one-shot notify requirement).
- spec/UI.md §4 (the three status strings — Mode A doc source).
- `cargo check` = 0 errors currently (S3's edits are the only changes).
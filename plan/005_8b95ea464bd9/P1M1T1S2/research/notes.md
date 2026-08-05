# Research Notes — P1.M1.T1.S2 (Render three-state status in src/tray.rs — macOS/Windows)

## 0. The contract I build on (S1 output — treat as law)

S1 (parallel sibling, "Implementing") adds to `src/core/notifier.rs`:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus { Connected, NoModule, Disconnected }

pub fn device_status() -> DeviceStatus  // reads is_device_connected() + host_capable(); NO HID I/O
```
Accessed from tray.rs as `crate::core::notifier::DeviceStatus` and
`crate::core::notifier::device_status()` (the file fully-qualifies all notifier refs;
there is NO `use crate::core::notifier::*` import — confirm at line 1-13).

`is_device_connected()`, `host_capable()`, `handshake_action()`, `perform_handshake()`,
`reset_handshake_state()`, `startup_device_was_connected()` are all UNCHANGED by S1 and
remain available. S2 only READS them.

## 1. The five change sites in src/tray.rs (exact before → after)

### Site A — `UserEvent` variant (line 41)
```rust
// BEFORE:
#[cfg(any(target_os = "macos", target_os = "windows"))]
DeviceStatus(bool),
// AFTER:
#[cfg(any(target_os = "macos", target_os = "windows"))]
DeviceStatus(crate::core::notifier::DeviceStatus),
```
Fully-qualified type (matches the file's fully-qualified-notifier convention). The cfg
gate is UNCHANGED. (`AutostartSync` and `MenuEvent` variants are untouched.)

### Site B — `device_status_text` (line 660-669) → three branches
```rust
// BEFORE: fn device_status_text(connected: bool) -> String { if connected {...} else {...} }
// AFTER:
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn device_status_text(status: crate::core::notifier::DeviceStatus) -> String {
    use crate::core::notifier::DeviceStatus;   // function-local use → terse match arms, no cfg-import headache
    match status {
        DeviceStatus::Connected => "\u{25CF}  Device Connected".to_string(),            // ●
        DeviceStatus::NoModule => "\u{26A0}  QMK board found \u{2014} no qmk_notifier module (flash it)".to_string(), // ⚠ —
        DeviceStatus::Disconnected => "\u{25CB}  No Device Connected".to_string(),       // ○
    }
}
```
- Glyphs as `\u{}` escapes (matches the existing `\u{25CF}`/`\u{25CB}` style; do NOT paste raw emoji).
- Two spaces after every glyph (matches existing `●  Device Connected`).
- Em-dash `—` in the No-module line is U+2014 (`\u{2014}`); NOT a hyphen-minus.
- The function-local `use crate::core::notifier::DeviceStatus;` is the cleanest way to get terse
  match arms without a module-level cfg-gated import (the function is already cfg-gated, so the
  local use is only in scope where it's needed → no unused-import warning on non-Hyprland Linux).
- Doc-comment MUST cite `spec/UI.md` §4 (Mode A requirement) — see PRP.

### Site C — First-paint MenuItem (line 315-319)
```rust
// BEFORE:
let device_status_i = MenuItem::new(
    device_status_text(crate::core::notifier::is_device_connected()),
    false,  // disabled = non-clickable label
    None,
);
// AFTER (only the argument changes; `false` stays):
let device_status_i = MenuItem::new(
    device_status_text(crate::core::notifier::device_status()),
    false,  // disabled — the "No module" warning glyph stays a disabled label (parity)
    None,
);
```
The `false` (enabled=false ⇒ disabled/non-clickable label) is INTENTIONAL and unchanged.
The "No module" warning must remain a disabled MenuItem (item requirement).

### Site D — Poll thread (lines 384-406): DUAL tracker
This is the subtle one. The handshake lifecycle MUST stay keyed on the Tier-1 presence
BOOL (`is_device_connected()`); the UI event payload becomes the three-state value, sent
on ITS OWN transition. Two independent trackers:
```rust
let mut last: Option<bool> =
    Some(crate::core::notifier::startup_device_was_connected());      // handshake (UNCHANGED)
let mut last_status: Option<crate::core::notifier::DeviceStatus> =
    Some(crate::core::notifier::device_status());                     // NEW: UI event tracker
loop {
    let connected = crate::core::notifier::is_device_connected();
    if last != Some(connected) {
        // ---- handshake block: UNCHANGED (stays keyed on the bool) ----
        match crate::core::notifier::handshake_action(last, connected) {
            crate::core::notifier::HandshakeAction::Gain => { crate::core::notifier::perform_handshake(verbose); }
            crate::core::notifier::HandshakeAction::Loss => { crate::core::notifier::reset_handshake_state(); }
            crate::core::notifier::HandshakeAction::None => {}
        }
        last = Some(connected);
    }
    // ---- UI status: three-state, sent only on transition (NEW) ----
    // Computed AFTER the handshake block so a same-tick Gain+perform_handshake (which may
    // set HOST_CAPABLE ⇒ Connected) is reflected in the payload immediately.
    let status = crate::core::notifier::device_status();
    if last_status != Some(status) {
        let _ = status_proxy.send_event(UserEvent::DeviceStatus(status));
        last_status = Some(status);
    }
    std::thread::sleep(std::time::Duration::from_secs(3));
}
```
Why DUAL (not "send DeviceStatus on the bool transition"):
- The headline F13 transition is **NoModule → Connected**: a board is present (bool stays
  `true`), the runner's startup handshake (or a Gain handshake) sets HOST_CAPABLE=true. The
  BOOL does not change on this transition, so a bool-keyed event would NEVER fire and the UI
  would stay stuck on "No module". Tracking `last_status` independently catches it on the next
  3s poll. This is the whole point of the three-state line.
- The handshake Gain/Loss MUST stay on the bool (item: "do NOT gate [handshake] on the
  three-state value") — gating it on DeviceStatus would, e.g., skip a Loss handshake when the
  board leaves but a NoModule state lingers, or fire spurious handshakes on NoModule↔Connected.
- Seed `last_status = Some(device_status())` so the first tick does NOT emit a redundant event
  (the first-paint at Site C already rendered the correct text). Mirrors today's
  `last = Some(startup_device_was_connected())` no-spurious-first-tick philosophy.

### Site E — Event-loop arm (line 507-510)
```rust
// BEFORE:
Event::UserEvent(UserEvent::DeviceStatus(connected)) => {
    device_status_i.set_text(device_status_text(connected));
}
// AFTER (just rename the binding; the type flows from the variant):
Event::UserEvent(UserEvent::DeviceStatus(status)) => {
    device_status_i.set_text(device_status_text(status));
}
```
The cfg gate on the arm is unchanged. Only the binding name + the (now-inferred) type change.

## 2. The cfg-gating reality (do not get this wrong)

- `src/tray.rs` line 1: `#![cfg(not(all(target_os = "linux", feature = "hyprland")))]` ⇒ the
  WHOLE FILE is skipped on the default Hyprland Linux build (where `linux_tray.rs` is used).
- On non-Hyprland Linux, tray.rs compiles, but every Site-A..E item is further gated
  `#[cfg(any(target_os = "macos", target_os = "windows"))]` ⇒ those items are absent on Linux.
- Therefore a module-level `use crate::core::notifier::DeviceStatus;` would be an UNUSED IMPORT
  on non-Hyprland Linux (all its uses are cfg'd out). AVOID: use the function-local `use` inside
  `device_status_text` (Site B), which is itself cfg-gated. The fully-qualified
  `crate::core::notifier::DeviceStatus` in the UserEvent variant + signature needs no import.
- Consequence for tests (§4): a tray.rs test module must be gated to macOS/Windows too.

## 3. No existing tray.rs test module — add one (cfg-gated)

`grep -n "#\[cfg(test)\]|mod tests|#\[test\]" src/tray.rs` ⇒ ZERO hits. There is no test module.
`device_status_text` is a PURE function (string mapping), so a deterministic no-hardware test is
trivial and valuable (it guards the glyph/text contract that S3's linux_tray.rs parity test will
mirror). Gate the whole mod to macOS/Windows (where the function exists):
```rust
#[cfg(all(test, any(target_os = "macos", target_os = "windows")))]
mod tests {
    use super::device_status_text;
    use crate::core::notifier::DeviceStatus;

    #[test]
    fn test_device_status_text_three_states() {
        assert_eq!(device_status_text(DeviceStatus::Connected),      "\u{25CF}  Device Connected");
        assert_eq!(device_status_text(DeviceStatus::NoModule),       "\u{26A0}  QMK board found \u{2014} no qmk_notifier module (flash it)");
        assert_eq!(device_status_text(DeviceStatus::Disconnected),   "\u{25CB}  No Device Connected");
    }
}
```
On default Linux CI this mod is not compiled (whole file skipped); on non-Hyprland Linux it's
cfg'd out; on macOS/Windows it runs. Deterministic — no hardware, no DebounceState.

## 4. Validation reality

- `cargo build` must succeed on the implementer's platform (macOS per AGENTS.md dev loop, or
  non-Hyprland Linux). `cargo clippy --bin qmkonnect` → no new warnings. `cargo fmt --check` → 0.
- `cargo test --bin qmkonnect -- --test-threads=1` (AGENTS.md: shared global debouncer state):
  existing tests still pass; the new tray.rs test runs ONLY on macOS/Windows.
- Live three-state observation (Disconnected/NoModule/Connected) requires real hardware + the
  AGENTS.md build/install/open loop — NOT a CI gate. The unit test + the S1 resolver tests are
  the deterministic proof of the text mapping; live UX is verified via the dev loop.

## 5. What S2 does NOT touch (boundary discipline)

- `src/core/notifier.rs` (S1's scope — DeviceStatus + device_status() live there; S2 only CALLS them).
- `src/linux_tray.rs` (S3 — the SNI three-state rendering + the Disconnected→NoModule notify-send).
- The handshake lifecycle code (Site D handshake block is byte-unchanged; only the surrounding
  tracker/seed/event-send is added).
- `is_device_connected()` / `host_capable()` / `handshake_action()` / `perform_handshake()` /
  `reset_handshake_state()` / `startup_device_was_connected()` — all read-only.
- `spec/*.md`, `docs/*.md` (Mode A = the device_status_text doc-comment cites UI.md §4; no doc FILES).
- Settings dialogs, window-info dialogs, autostart, icon handling — all unrelated to line-2 status.

## 6. Risk inventory (all low; all caught by build/clippy/the unit test)

1. **Module-level unused import on non-Hyprland Linux** — mitigated by the function-local `use` (§2).
2. **Em-dash vs hyphen** in the No-module string — the test asserts the exact `\u{2014}` string.
3. **Bool-keyed event missing the NoModule→Connected flip** — mitigated by the dual-tracker (§1 Site D).
4. **Spurious first-tick event** — mitigated by seeding `last_status = Some(device_status())`.
5. **Accidentally gating the handshake on DeviceStatus** — the item forbids it; Site D keeps the
   handshake block byte-unchanged on the bool.
6. **Forgetting to update the event-loop arm's binding type** — `connected` → `status`; type inferred.
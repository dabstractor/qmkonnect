# Research findings — P1.M3.T1.S1 (startup_device_state API + seed poll threads)

Verified against the working tree this session. All line numbers are current.

## The defect (Finding #5, handshake_race_research.md)

At startup the runner does `if is_device_connected() { perform_handshake() }`
→ `HAS_HANDSHAKED=true`, `HOST_CAPABLE=true`, `SET_OS` sent once. The poll
threads seed `let mut last: Option<bool> = None`. If the FIRST tick transiently
reads `is_device_connected()==false`, `handshake_action(None, false) == None`
(no Gain, no Loss) → `last` jumps straight to `Some(false)` WITHOUT ever passing
through `Some(true)`, so no `reset_handshake_state()` fires. If the device then
genuinely power-cycles within ~one poll interval, the reconnect `Gain` finds
`HAS_HANDSHAKED` still `true` → `perform_handshake` short-circuits → **`SET_OS`
not re-sent** on the freshly-rebooted board.

## The fix (contract)

Seed each poll thread's `last` from the SAME `is_device_connected()` result the
runner used at startup, captured once into a process-global `OnceLock<bool>`. If
startup was connected (seed `Some(true)`), a transient first-tick `false` now
yields `handshake_action(Some(true), false) == Loss` → `reset_handshake_state()`
→ next tick `true` → `Gain` → `perform_handshake` re-sends `SET_OS` (correct).

## Verified code anchors

### notifier.rs
- Import line 7: `use std::sync::{Arc, Condvar, Mutex};` → add `OnceLock`
  (matches core/mod.rs:9 + linux_tray.rs:500 convention). **OnceLock is stable
  since Rust 1.70; edition = 2021 (Cargo.toml:4). ✓**
- `is_device_connected()` — notifier.rs:216-226 (pure read-only enumeration;
  re-reads config each call). Closes at line 226 `}`. New accessor fns go
  immediately after.
- `static HAS_HANDSHAKED: AtomicBool` — notifier.rs:260 (new `OnceLock` static
  goes right after it, ~line 264, before `RULES_INVALID_NOTIFIED`).
- `reset_handshake_state()` — notifier.rs:649-654 (clears HOST_CAPABLE,
  BOARD_HAS_RULES, CALLBACK_NAMES, HAS_HANDSHAKED).
- `handshake_action(prev, now)` — notifier.rs:689-693:
  - `(Some(true), false) => Loss` ← the ONLY reset trigger (the fix's reliance)
  - `(p, true) if p != Some(true) => Gain` (None→true OR false→true)
  - `_ => None` (no change OR None→false ← the bug path)
- Test module `#[cfg(test)] mod tests` — notifier.rs:1208, `use super::*;`
  (tests call bare `handshake_action`/`HandshakeAction`/`is_device_connected`).
- Existing `test_handshake_action_transitions` — notifier.rs:2036-2042 already
  asserts all 6 transitions incl. `(Some(true), false) == Loss` (:2040).
- Baseline: `cargo test --bin qmkonnect -- --test-threads=1` → **348 passed**
  (this task adds 2 → expect 350).

### Runners — 4 call sites for record_startup_device_state()
Each runner has the identical pattern right before the `if`:
```
        crate::core::notifier::startup_device_probe(self.verbose);
        // If a device is already connected at startup, ...
        if crate::core::notifier::is_device_connected() {
```
- macos.rs:31  (`if` line)
- windows.rs:52 (run_console_mode) AND windows.rs:105 (run_tray_app) — **two
  identical blocks; disambiguate edits by trailing context** (console→
  `ctrlc::set_handler`; tray→`// Start the monitor`).
- linux.rs:31
- runners/mod.rs cfg-gates: `#[cfg(target_os="windows")] mod windows`,
  `#[cfg(target_os="macos")] mod macos`, `#[cfg(target_os="linux")] mod linux`.
  ⇒ on Linux ONLY linux.rs compiles; macos.rs/windows.rs validate on target OS.

### Poll-thread seeds (2 sites)
- tray.rs:385 `let mut last: Option<bool> = None;` — inside
  `#[cfg(any(target_os="macos", target_os="windows"))]` block ⇒ NOT compiled on
  Linux; validates on macOS/Windows builds.
- linux_tray.rs:262 `let mut last_device: Option<bool> = None;` — compiles on
  Linux (linux-tray is a default feature) ⇒ validated on Linux box.

## Contract's line refs vs actual (minor deltas — PRP uses ACTUAL)
- contract "tray.rs:387" → **actual 385**
- contract "linux_tray.rs:263" → **actual 262**
- contract "notifier.rs:689 handshake_action" → ✓ matches
- contract "windows.rs:52-54 & 105-107" → ✓ matches (52, 105)

## OnceLock testing reality
`OnceLock<bool>` is set-once, process-global. Tests run single-threaded in one
process (AGENTS.md). So:
- `startup_device_was_connected()` defaults to `false` via `.unwrap_or(&false)`
  before any record call.
- The FIRST `record_startup_device_state()` call wins; later calls are silent
  no-ops (the `_ = …set(…)` discards the Err).
- On a CI box with no QMK device, `is_device_connected()==false`, so the frozen
  value is `false`. A deterministic test asserts the accessor equals the live
  probe (both false on CI); it cannot pin a fixed value across environments.

## Parallel-item / sibling interactions (no overlap)
- **P1.M2.T2.S2** (parallel): edits tray.rs:878/:1276 + linux_tray.rs:822
  (settings-dialog writes). THIS task edits tray.rs:385 + linux_tray.rs:262
  (poll-thread seeds). Same files, DIFFERENT lines (far apart) → clean merge.
- **P1.M2.T1.S1**: owns atomic_write in core/mod.rs — unrelated file.
- **P1.M3.T2.S1** (sibling, not yet started): will restructure the NOTIFIER
  lock inside `perform_handshake_with` (notifier.rs:388+). THIS task adds a new
  static (~264) + 2 fns (~228) + touches handshake_action's callers, NOT
  perform_handshake_with internals. Low conflict; different regions of notifier.rs.

## DRY note (intentionally rejected)
Could fold record into `startup_device_probe()`, but the contract keeps them
separate: `startup_device_probe` is a read-only VID/PID-typo hint (#16); the new
fn records lifecycle state for the poll threads. Different responsibilities.
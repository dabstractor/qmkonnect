# Research Notes — P1.M1.T1.S1: DeviceStatus (three-state) + device_status() resolver

> Scope: add `DeviceStatus { Connected, NoModule, Disconnected }` + `device_status()`
> to `src/core/notifier.rs`. Pure read of two existing booleans. No UI (S2/S3), no
> per-path ping (P3's classify_devices).

## 1. Validated existing state (confirmed against live code + architecture doc)

| Item | Location | Shape | This subtask |
|---|---|---|---|
| `is_device_connected()` | notifier.rs:216 | pure Tier-1 enumerate via `hidapi::HidApi::new()`; matches usage_page/usage + optional VID/PID; **never opens/sends** | UNCHANGED (read only) |
| `HOST_CAPABLE` | notifier.rs:270 | `static AtomicBool = AtomicBool::new(false)` | UNCHANGED (read via host_capable; tests may set directly) |
| `host_capable()` | notifier.rs:689 | `HOST_CAPABLE.load(Ordering::SeqCst)` | UNCHANGED (read only) |
| `reset_handshake_state()` | notifier.rs:705 | stores HOST_CAPABLE/BOARD_HAS_RULES=false, clears CALLBACK_NAMES, HAS_HANDSHAKED=false | UNCHANGED (used by tests for isolation) |

`HOST_CAPABLE` lifecycle (already maintained by the poll threads — no new work):
- Set `true` by `perform_handshake_with` (line 558) on a capable `Info{proto_ver:2, flags&0x01}` reply.
- Reset `false` on: Timeout (576), non-capable reply (588), device-error (600), and `reset_handshake_state()` (706) on a `HandshakeAction::Loss`.
- Poll threads (`tray.rs:380-406`, `linux_tray.rs:259-301`) already drive `handshake_action` on every transition, so `host_capable()` is correct on each device gain/loss.

## 2. The three-state derivation (verbatim from architecture doc + spec/DEVICE_DISCOVERY.md §3)

| Status | Condition |
|---|---|
| `Disconnected` | `!is_device_connected()` (0 Tier-1 boards) |
| `NoModule` | `is_device_connected() && !host_capable()` (≥1 Tier-1, 0 capable) |
| `Connected` | `is_device_connected() && host_capable()` (≥1 capable) |

`present` dominates: a stale `HOST_CAPABLE=true` can never fabricate `NoModule`/`Connected`
when no board is enumerated. (This is the property the CI integration test asserts.)

**Transient caveat** (architecture doc): right after a Gain, `host_capable()` is false
until `perform_handshake` completes (sub-second); the line may briefly read `NoModule`
before flipping to `Connected`. Acceptable per spec.

## 3. ⚠ The testability constraint (the key design decision)

`device_status()` (no-arg, per contract) reads `is_device_connected()`, which calls
`hidapi::HidApi::new()` and enumerates **real** HID interfaces. In CI there is no
`0xFF60`/`0x61` board ⇒ `is_device_connected() == false` ⇒ `device_status()` can only
naturally return `Disconnected`. There is **no trait seam** to mock hidapi (out of scope).

⇒ To unit-test all **three** derivations deterministically (the contract requires "unit
tests for the three derivations, including the `present && !capable → NoModule` case"),
the pure truth table is split into a **private** helper:

```rust
pub fn device_status() -> DeviceStatus {
    classify_device_status(is_device_connected(), host_capable())
}

fn classify_device_status(present: bool, capable: bool) -> DeviceStatus {
    if !present { DeviceStatus::Disconnected }
    else if capable { DeviceStatus::Connected }
    else { DeviceStatus::NoModule }
}
```

- The **public API** is the no-arg `device_status()` exactly as specified.
- The **private helper** takes the two booleans directly ⇒ all three rows testable without hardware.
- The test module does `use super::*;` so the private helper is in scope.

Test plan (2 tests):
1. `test_classify_device_status_truth_table` — all 3 rows of the helper (false/false→Disconnected,
   false/true→Disconnected, true/false→NoModule, true/true→Connected).
2. `test_device_status_is_disconnected_in_ci_without_hardware` — set `HOST_CAPABLE` both ways
   (`reset_handshake_state()` then `HOST_CAPABLE.store(true,...)`); assert `device_status() ==
   Disconnected` both times (proves the wiring through `is_device_connected()` AND that a stale
   capable flag can't fabricate a false state). Restore via `reset_handshake_state()` for isolation.

`HOST_CAPABLE` is a module-level `static AtomicBool` ⇒ directly settable from the test module.

## 4. Scope boundaries (what is NOT this subtask)

- **No per-path ping / QUERY_INFO in device_status().** Per-candidate Tier-2 classification is P3's
  `classify_devices()` (cache-backed, `ClassifiedDevice`). P1 reuses the handshake's already-set
  `HOST_CAPABLE` flag. Adding I/O here would violate the contract, break the cheap-poll NFR, and
  collide with P3.
- **No per-board count / "N Devices Connected" pluralization.** That's P3. P1's `DeviceStatus` is the
  flat three-state value; P3 layers richness WITHOUT changing these variants.
- **No UI.** S2 (`src/tray.rs` macOS/Windows) and S3 (`src/linux_tray.rs` + the Disconnected→NoModule
  one-shot notify-send, using the once-guard pattern at notifier.rs:299) consume `device_status()`.
- **`is_device_connected()` / `host_capable()` unchanged.** Still used by the write-path/broadcast
  decision, the device-presence snapshot, the picker Tier-1 pass.

## 5. Placement

Status cluster: after `reset_handshake_state()` (line ~710), where `host_capable()` (689) lives.
Group `DeviceStatus` + `device_status()` + `classify_device_status()` together with one blank line
of separation. (NOT next to `is_device_connected()` at 216 — that's the Tier-1 enumerate area, far
from the capability state `device_status()` reads.)

## 6. Derives

`#[derive(Debug, Clone, Copy, PartialEq, Eq)]` — fieldless 3-variant enum. `PartialEq`/`Eq` so S2/S3
(and tests) compare `prev == new` for transition detection. `Copy` is free/idiomatic.

## 7. Validation commands (verified shape)

- `cargo build` — zero warnings (fieldless enum + pure fn).
- `cargo clippy --bin qmkonnect` — no new warnings (pub items don't trip dead_code).
- `cargo fmt --check` — exit 0.
- `cargo test --bin qmkonnect test_classify_device_status_truth_table -- --test-threads=1`
- `cargo test --bin qmkonnect test_device_status_is_disconnected_in_ci_without_hardware -- --test-threads=1`
- `cargo test --bin qmkonnect -- --test-threads=1` — `<N+2> passed; 0 failed`.
- Scope greps: `is_device_connected`/`host_capable` bodies unchanged vs HEAD; `device_status()` body
  has no `HidApi`/`send_command`/`write`/`open`; `src/tray.rs` + `src/linux_tray.rs` unmodified.
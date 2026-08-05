# Architecture Findings — Capability-keyed Discovery Lifecycle Delta (Session 6)

## Executive Summary

This delta is a **specification clarification that was already shipped to code + spec**
during the prior session (commit `d240b27` *"Key handshake lifecycle on capable-board
presence, not Tier-1"*). **There is no new feature work.** The only remaining work is
**verification** — confirm the already-shipped implementation satisfies the v6 spec,
quality gates are green, and there is zero spec/code drift.

All PRD implementation claims have been **independently verified** by direct codebase
inspection during this research phase. Every claim is TRUE.

---

## 1. Verified Implementation Claims

### 1.1 PresenceTracker (capable-keyed, path-set-gated)

| PRD Claim | Location | Verified |
|---|---|---|
| `struct PresenceTracker` | `src/core/notifier.rs:1292` | ✅ `pub struct PresenceTracker { last_paths: Vec<String>, last_capable: bool }` |
| `presence_tick_decision()` (pure core fn) | `src/core/notifier.rs:1251` | ✅ Pure function: `(last_capable, paths_changed, tier1_present, reprobed_capable) -> (HandshakeAction, bool)` |
| `tier1_paths()` | `src/core/notifier.rs:1235` | ✅ `pub fn tier1_paths() -> Vec<String>` |
| `tick()` method | `src/core/notifier.rs:1311` | ✅ Only calls `classify_devices` when `paths != self.last_paths` (path-set-gated) |
| `PresenceTracker::new()` | `src/core/notifier.rs` (impl block) | ✅ Seeds from `tier1_paths()` + `host_capable()` |
| `Default` impl | after struct | ✅ Delegates to `new()` |

**Key behavioral property confirmed:** `tick()` sets `paths_changed = paths != self.last_paths`
and only re-probes capability via `classify_devices(verbose)` inside the
`if paths_changed && tier1_present` branch. On a stable bus, `reprobed` is `None` and
`presence_tick_decision` returns `last_capable` unchanged ⇒ `HandshakeAction::None`.

### 1.2 Tray Poll-Thread Wiring

| Platform | `PresenceTracker::new()` | `presence.tick(verbose)` | `device_status()` read |
|---|---|---|---|
| macOS/Windows (`src/tray.rs`) | ✅ `:439` | ✅ `:452` | ✅ `:446`, `:465` |
| Linux (`src/linux_tray.rs`) | ✅ `:288` | ✅ `:302` | ✅ `:294`, `:312` |

Linux tray field renamed `device_status` confirmed at `linux_tray.rs:85`.

### 1.3 Three-State Device Status

`device_status()` at `src/core/notifier.rs:783` — returns `DeviceStatus` enum
(`Connected` / `NoModule` / `Disconnected`). Both tray poll threads read it on
each transition and update the status menu item. `device_status_text()` helper
in `tray.rs:727` maps enum → user-facing label.

### 1.4 Handshake → Cache Warm-Feed + Scope Guard

| PRD Claim | Location | Verified |
|---|---|---|
| `handshake_warm_eligible(candidate_count)` | `src/core/notifier.rs:1141` | ✅ Returns `candidate_count <= 1` |
| `warm_cache_from_handshake(kind)` | `src/core/notifier.rs:1163` | ✅ Early-returns on `!handshake_warm_eligible(candidates.len())` |
| Called from `perform_handshake_with` | `:567`, `:594`, `:607`, `:625` | ✅ All 4 call sites confirmed (Capable result at 567, NotQmkNotifier at 594/607/625) |

**Scope guard confirmed:** With ≥2 boards, `handshake_warm_eligible` returns false,
`warm_cache_from_handshake` early-returns, and per-path classification is left to
`classify_devices`'s per-candidate vid/pid-narrowed probe.

### 1.5 Proto-v1 Dedup Asymmetry (Documented Caveat)

| Path | Gates on `HAS_HANDSHAKED`? | Location |
|---|---|---|
| `perform_handshake_with()` | ✅ Yes — `if HAS_HANDSHAKED.swap(true, SeqCst)` at `:428` | `src/core/notifier.rs:428` |
| `classify_devices()` | ❌ **Deliberately NOT** — no `HAS_HANDSHAKED` check | `src/core/notifier.rs:1123` |

This is the documented proto-v1 caveat: the picker probe (via `classify_devices`)
sits outside the dedup, so on proto-v1 firmware it can briefly reset the layer per
probe. `classify_devices` body (`:1123-1128`) confirms: `enumerate_candidates` →
`invalidate_absent_cache_entries` → `classify_candidates` — no `HAS_HANDSHAKED` gate.

### 1.6 Trayless Startup Handshake

`src/runners/linux.rs:26-32`:
```rust
// If a device is already connected at startup, run the capability handshake
// now (poll-thread reconnects are handled in linux_tray.rs / tray.rs).
// Completes before the poll thread exists; idempotent via HAS_HANDSHAKED.
if crate::core::notifier::is_device_connected() {
    crate::core::notifier::perform_handshake(self.verbose);
}
```
`BindsTo=dev-qmkonnect_device.device` + `Restart=always` confirmed in the systemd
template (`packaging/linux/systemd/qmkonnect.service.template`).

---

## 2. Verified Test Coverage

The PRD claims "8 targeted tests." Direct grep found **more than 8** PresenceTracker /
warm-feed / classify-related tests:

| Test Name | Line |
|---|---|
| `test_presence_tick_capable_unplug_with_non_capable_remaining_is_loss` | 3012 |
| `test_presence_tick_capable_replug_different_board_is_gain` | 3026 |
| `test_presence_tick_stable_bus_no_reprobe_no_action` | 3037 |
| `test_presence_tick_all_unplugged_forces_loss` | 3052 |
| `test_presence_tick_boot_reprobe_capable_is_none_when_already_capable` | 3069 |
| `test_presence_tick_reprobe_not_capable_after_not_capable_is_none` | 3082 |
| `test_classify_device_status_truth_table` | 3565 |
| `test_classify_reply_info_proto2_capable` | 3765 |
| `test_classify_reply_info_proto2_no_feature_bit_still_capable` | 3786 |
| `test_classify_reply_info_proto1_notqmk` | 3809 |
| `test_classify_candidates_capable` | 3855 |
| `test_classify_candidates_mixed` | 3894 |
| `test_classify_candidates_cache_hit_skips_ping` | 3950 |
| `test_classify_candidates_cache_miss_pings_and_caches` | 3987 |
| `test_classify_candidates_ttl_re_ping` | 4020 |
| `test_classify_devices_smoke_returns_vec` | 4081 |
| `test_handshake_warm_eligible_single_board_only` | 4097 |

---

## 3. Verified Spec Drift (v6 Wording Present)

### `spec/DEVICE_DISCOVERY.md`
- ✅ `:81` — "No **proto-v2 or pure-VIA** board is harmed" (hunk #1)
- ✅ `:87-97` — Proto-v1 caveat block (hunk #1)
- ✅ `:155-164` — Handshake → cache warm-feed scope (hunk #2)
- ✅ `:189` — "**capable-keyed** (not Tier-1-keyed): a `PresenceTracker`..." (hunk #3)

### `spec/LINUX.md`
- ✅ `:211-219` — Trayless (`--no-default-features`) build caveat + `BindsTo`/`Restart` (hunk #4)
- ✅ `:236` — "Poll thread: every **1 s** drive a `PresenceTracker` tick" (hunk #5)
- ✅ `:240` — `device_status` field reference

### `spec/HOST_RULES.md`
- ✅ `:616-627` — R6 expanded with proto-v1 exception detail, references `PresenceTracker` (hunk #6)

---

## 4. Quality Gates

The project's `validate.sh` at `plan/005_8b95ea464bd9/validate.sh` defines 7 phases:
1. `cargo fmt --all --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo build --release` (default + all-targets + no-default-features + hid-id)
4. `cargo test --bin qmkonnet -- --test-threads=1`
5. E2E CLI subcommands
6. Spec invariants
7. hid-id helper against real hardware

Per `AGENTS.md`, the core dev-loop gate is: `cargo test --bin qmkonnect -- --test-threads=1`
(single-threaded because of shared global debouncer state).

---

## 5. Documentation Impact Assessment

- **Mode A (doc-with-work):** NONE. Spec files already at v6 wording, code already carries
  explanatory doc-comments (`PresenceTracker`, `warm_cache_from_handshake`,
  `presence_tick_decision`, proto-v1 picker-caveat comment on `classify_devices`).
- **Mode B (changeset-level docs):** NONE expected. `README.md`, `docs/*.md`,
  `docs/llms_full.txt` were regenerated in commit `293f565` and are not affected by this
  internal correctness/edge-case fix. The final "Sync changeset-level documentation" task
  will verify this and only make changes if drift is found.

---

## 6. Architectural Patterns & Conventions

- **Core/platform separation:** All lifecycle logic lives in `src/core/notifier.rs`
  (platform-independent). Trays are thin poll-loop consumers calling pure core functions.
- **Pure decision functions:** `presence_tick_decision`, `handshake_action`,
  `handshake_warm_eligible` are all pure functions, unit-tested without HID hardware.
- **Global state via atomics:** `HAS_HANDSHAKED: AtomicBool`, `HOST_CAPABLE: AtomicBool`
  — thread-safe without locks.
- **TTL cache pattern:** `CLASSIFICATION_CACHE` with `CLASSIFICATION_TTL` (~5s) keeps
  pings to one-per-appearance in the common case.
- **Single-threaded test discipline:** All tests run with `--test-threads=1` due to
  shared global debouncer state (documented in `AGENTS.md`).
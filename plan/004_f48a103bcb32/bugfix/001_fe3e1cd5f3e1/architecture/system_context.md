# System Context — Bug-Hunt Remediation (plan/004/bugfix/001)

## Project Overview

**QMKonnect** is a cross-platform (Linux/macOS/Windows) menu-bar/tray daemon that
notifies a QMK keyboard of the active window's app class + title over Raw HID.
It supports host-side window rules (rules.toml), firmware capability handshake
(proto v2), debounced send, and hot-reloading configuration.

## Bug-Hunt Findings Status

| # | Finding (PRD §) | Severity | Status | Detail Doc |
|---|----------------|----------|--------|------------|
| 🔴 | Debounce worker panic (h2.0) | CRITICAL | **FIXED** — notifier.rs:863 `match state.pending.take()`; regression test at :2407; 344 tests pass | this doc §1 |
| 1 | Non-atomic config/rules writes (h2.1) | Low | NOT FIXED | `config_writes_research.md` |
| 2 | Config/rules re-read per window change (h2.1) | Low (latency) | NOT FIXED | `config_reread_research.md` |
| 3 | Windows MessageBoxW not toast (h2.1) | Low–Med (spec) | NOT FIXED | `windows_notify_research.md` |
| 4 | Handshake holds NOTIFIER mutex ≤5–8 s (h2.1) | Low–Med | NOT FIXED (known tradeoff) | `handshake_race_research.md` |
| 5 | First-tick device-absent race (h2.1) | Medium | NOT FIXED | `handshake_race_research.md` |

## 1. Critical Debounce Fix (h2.0) — VERIFIED APPLIED

**File:** `src/core/notifier.rs`, `debounce_worker()` (line ~832–896).

**The race:** The worker's inner `wait_timeout` loop releases `STATE` during the
timed wait. `notify_qmk`'s immediate-send ("due") branch acquires `STATE`, sets
`pending = None`, and bumps `last_sent_time`. When the worker wakes with
`now >= target`, `pending` can be `None` → `.unwrap()` panics. With
`panic = "abort"` in release, the service dies.

**Fix (applied):** `state.pending.take()` now uses a `match` with `None => break`
that exits the inner loop (skipping the send) and re-enters the outer wait loop.
No busy-loop (outer loop re-waits on COND). Data correctness preserved (newer
message already sent immediately; superseded pending correctly dropped).

**Key constraint:** `panic = "abort"` (Cargo.toml `[profile.release]`). Any panic
kills the process — mutex poisoning is impossible in release (process is dead
before re-lock). In debug/test builds, tests run `--test-threads=1` so no
cross-thread poisoning. Poison-recovery in the worker is defense-in-depth for
future `unwind` mode only.

## 2. Key Code Locations

### Config & Rules I/O
- `src/core/mod.rs:106` — `parse_config()` → `fs::read_to_string` + `toml::from_str`
- `src/core/mod.rs:157` — `render_config_body()` (pure renderer, no IO)
- `src/core/mod.rs:218` — `create_default_config()` → `fs::write` (NON-ATOMIC)
- `src/core/mod.rs:334` — `create_default_rules()` → `fs::write` (NON-ATOMIC)
- `src/core/rules.rs:210` — `parse_rules()` → `fs::read_to_string` + `toml::from_str`
- `src/tray.rs:878` — Windows settings dialog save → `std::fs::write` (NON-ATOMIC)
- `src/tray.rs:1276` — macOS settings dialog save → `std::fs::write` (NON-ATOMIC)
- `src/linux_tray.rs:822` — Linux settings `write_config()` → `fs::write` (NON-ATOMIC)
- `src/platforms/linux.rs:336` — `write_rule_atomic()` (THE ONLY atomic helper; udev only)

### Notifier & Debounce
- `src/core/notifier.rs:770-813` — `DebounceState` + `STATE`/`COND` statics
- `src/core/notifier.rs:832` — `debounce_worker()` (the critical fix site)
- `src/core/notifier.rs:919` — `notify_qmk()` (immediate + queue paths)
- `src/core/notifier.rs:1013` — `host_context_for_window()` (rules eval + re-read)
- `src/core/notifier.rs:80` — `configured_filter()` (config re-read per call)
- `src/core/mod.rs:89-98` — `configured_debounce_ms()` / `configured_timing()` (config re-read per call)

### Handshake & Device Lifecycle
- `src/core/notifier.rs:388` — `perform_handshake_with()` (holds NOTIFIER lock during sweep)
- `src/core/notifier.rs:402` — `notifier.lock().unwrap()` (NOTIFIER acquired)
- `src/core/notifier.rs:430-481` — `QUERY_CALLBACK` sweep loop (under lock)
- `src/core/notifier.rs:484` — `drop(n)` (NOTIFIER released after sweep)
- `src/core/notifier.rs:260` — `HAS_HANDSHAKED: AtomicBool` (dedup token)
- `src/core/notifier.rs:649` — `reset_handshake_state()`
- `src/core/notifier.rs:689` — `handshake_action(prev, now)` — `(None,false)→None` is the #5 gap
- `src/core/notifier.rs:216` — `is_device_connected()`
- Runners: `src/runners/{linux,macos,windows}.rs` — startup handshake at ~line 31/52
- Poll threads: `src/tray.rs:384-409` (macOS/Windows), `src/linux_tray.rs:261-294` (Linux)

### Notifications
- `src/platforms/mod.rs:126-172` — `notify()` (Linux: notify-send; macOS: osascript; **Windows: MessageBoxW**)
- `src/core/notifier.rs:264` — `RULES_INVALID_NOTIFIED` (dedup flag)
- Only caller: `host_context_for_window()` → "QMKonnect: rules.toml invalid"

### Windows Packaging
- `packaging/windows/inno/QMKonnect.iss` — Inno Setup installer
- Start Menu shortcut at `[Icons]` section (line ~89) — no AUMID property currently

## 3. Architecture Constraints

- **Rust edition 2021**, MSRV 1.88, `panic = "abort"` in release.
- **No new deps philosophy:** prefer std-only solutions. `tempfile` is dev-dep +
  Linux-only dep (not available on macOS/Windows non-test code).
- **Test protocol:** `cargo test --bin qmkonnect -- --test-threads=1` (shared
  global debouncer state). 344 tests currently pass.
- **Platform testing:** macOS app needs `clean.sh && build.sh && install.sh`
  loop; Windows needs `taskkill /IM qmkonnect.exe /F` before each run.
- **Hot-config is intentional:** config.toml/rules.toml are re-read per
  notification cycle by design (PRD §8, ARCHITECTURE.md §10 #4). Any caching
  must preserve the hot-config SLO (~3 s propagation).
- **`notify-rust` is rejected for Linux** (spec/LINUX.md §7.3: nested tokio
  footgun). This is Linux-specific; a Windows toast crate is not precluded.

## 4. Fix Feasibility Summary

| Finding | Approach | New Dep? | Risk | Effort |
|---------|----------|----------|------|--------|
| #1 Atomic writes | std-only `atomic_write(path, content)`: temp file + `fs::rename` (atomic same-dir) | None | Low | 5 call sites |
| #2 Config caching | `Mutex<Option<(SystemTime, T)>>` mtime-keyed cache | None | Low (must keep `test_debounce_ms_is_hot_config` passing) | Medium |
| #3 Windows toast | WinRT toast via `windows` crate features + AUMID; or `tauri-winrt-notification` crate | `Win32_UI_Notification` features or toast crate | Medium (COM interop + AUMID plumbing + Inno changes) | High |
| #4 Handshake mutex | Release NOTIFIER lock per sweep iteration; re-acquire per `QueryCallback` | None | Medium (behavior change; must verify send ordering) | Medium |
| #5 First-tick race | Seed poll thread `last` from startup `is_device_connected()` result | None | Low (one-line change per poll site) | Low |
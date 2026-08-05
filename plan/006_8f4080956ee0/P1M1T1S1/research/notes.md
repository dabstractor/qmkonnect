# Research Notes — P1.M1.T1.S1: Run quality gates + confirm PresenceTracker/warm-feed/classify-mixed tests

> Verification-only subtask. No code changes. Ran the 5 gates + the single-threaded
> test suite during research to embed a verified baseline in the PRP.

## 1. Verified baseline (research run, this session — reproduce this)

| Gate | Command | Result |
|---|---|---|
| (a) | `cargo fmt --all --check` | ✅ exit 0 (clean, no diff) |
| (b) | `cargo clippy --all-targets -- -D warnings` | ✅ exit 0 (`Finished dev profile in 0.14s`, no warnings) |
| (c) | `cargo build --release` | ✅ exit 0 (`Finished release profile [optimized] in 0.08s`) |
| (d) | `cargo build --release --no-default-features` | ✅ exit 0 (10 expected dead_code warnings — §3 below) |
| (e) | `cargo test --bin qmkonnect -- --test-threads=1` | ✅ **389 passed; 0 failed; 0 ignored** (13.58s) |

The 5 named tests (each `... ok`):
- `test_presence_tick_capable_unplug_with_non_capable_remaining_is_loss` (notifier.rs:3012)
- `test_presence_tick_capable_replug_different_board_is_gain` (notifier.rs:3026)
- `test_presence_tick_stable_bus_no_reprobe_no_action` (notifier.rs:3037)
- `test_handshake_warm_eligible_single_board_only` (notifier.rs:4097)
- `test_classify_candidates_mixed` (notifier.rs:3894)

**Verdict: GREEN.** No regressions. (target/ was warm — ~20G cached; cold runs will be slower.)

## 2. The 5 named tests exist at EXACTLY the contract-cited line numbers

`grep -nE` against `src/core/notifier.rs`:
```
3012: fn test_presence_tick_capable_unplug_with_non_capable_remaining_is_loss
3026: fn test_presence_tick_capable_replug_different_board_is_gain
3037: fn test_presence_tick_stable_bus_no_reprobe_no_action
3894: fn test_classify_candidates_mixed
4097: fn test_handshake_warm_eligible_single_board_only
```
All match the contract. (The architecture doc §2 lists 17 PresenceTracker/warm-feed/classify
tests total; these 5 are the headline subset the contract names.)

## 3. Gate (d) trayless-build warnings — EXPECTED, characterized (not a regression)

`--no-default-features` disables `hyprland`/`macos`/`linux-tray`. The tray poll threads are the
ONLY callers of several pure functions; with them feature-gated out, those fns become unreferenced
→ `dead_code` / `never used`. All 10 warnings:

1. `unused variable: verbose` (src/tray.rs:297)
2. `struct PresenceTracker is never constructed`
3. `associated items new and tick are never used` (PresenceTracker)
4. `function presence_tick_decision is never used`
5. `function tier1_paths is never used`
6. `function handshake_action is never used`
7. `enum HandshakeAction is never used`
8. `function reset_handshake_state is never used`
9. `function list_foreground_windows is never used`
10. `function render_config_body is never used`

The gate is BUILD SUCCESS (exit 0), NOT warning-free. Clippy `-D warnings` (gate b) runs on the
DEFAULT feature set (where the tray uses all these), so it is clean. **If a future run shows a
warning outside this list, or the trayless build exits non-zero, THAT is a regression.**

## 4. Build matrix (Cargo.toml [features])

```toml
default = ["hyprland", "macos", "linux-tray"]
hyprland = ["dep:hyprland"]
macos    = ["dep:objc", "dep:core-foundation", "dep:core-graphics", "dep:dispatch"]
linux-tray = ["dep:ksni", "dep:gtk"]
```
- `cargo build --release` → full app with tray (default features). ✅
- `cargo build --release --no-default-features` → trayless minimal service build (spec/LINUX.md
  trayless caveat). ✅ (with the 10 expected dead_code warnings above.)

## 5. The canonical gate sequence (validate.sh, 7 phases — S1 runs phases 1–4)

From `plan/005_8b95ea464bd9/validate.sh`:
1. Phase 1 — `cargo fmt --all --check`
2. Phase 2 — `cargo clippy --all-targets -- -D warnings`
3. Phase 3 — Build (release: default + all-targets + no-default-features + hid-id)
4. Phase 4 — `cargo test --bin qmkonnect -- --test-threads=1` (MUST be single-threaded)
5. Phase 5 — E2E CLI subcommands  ← out of scope for S1
6. Phase 6 — Spec invariants       ← out of scope for S1
7. Phase 7 — hid-id on real hardware ← out of scope for S1 (needs a QMK board)

The contract's 5 gates (a–e) = validate.sh phases 1–4 (phase 3 split into default + trayless).

## 6. Why single-threaded (the load-bearing constraint)

`AGENTS.md` + architecture doc §6: tests share global state — `DebounceState` (the debouncer
`STATE` mutex + `COND` condvar), the mock globals (`MOCK_CALL_COUNT`, `MOCK_RESPONSES`, etc.),
and the `AtomicBool`s (`HOST_CAPABLE`, `HAS_HANDSHAKED`). Parallel execution → races/false failures.
The contract fixes `--test-threads=1`. Never drop it.

## 7. Git context

```
f4315a6 Add task breakdown and architecture research   ← HEAD (plan files only, no code)
d240b27 Key handshake lifecycle on capable-board presence, not Tier-1   ← the shipped delta
332a31d Fix CI/dev-loop quality gates and readme path metadata
```
HEAD is one plan-only commit ahead of `d240b27`; the code being verified is unchanged from the
shipped delta. `git status` for `src/` is clean (Task 1 confirms this before running).

## 8. Report template (the deliverable)

```
GATE (a) fmt:           PASS (exit 0)
GATE (b) clippy:        PASS (exit 0, no warnings)
GATE (c) build --release:                  PASS (exit 0)
GATE (d) build --release --no-default-features:  PASS (exit 0; 10 expected dead_code warnings — listed)
GATE (e) tests:         PASS — 389 passed; 0 failed; 0 ignored (13.58s)

NAMED TESTS (all ok):
  test_presence_tick_capable_unplug_with_non_capable_remaining_is_loss ... ok
  test_presence_tick_capable_replug_different_board_is_gain ... ok
  test_presence_tick_stable_bus_no_reprobe_no_action ... ok
  test_handshake_warm_eligible_single_board_only ... ok
  test_classify_candidates_mixed ... ok

VERDICT: GREEN — no regressions. Capability-keyed lifecycle delta (d240b27) verified gate-clean.
SOURCE CHANGES: none (git status clean).
```
(If any gate diverges from this baseline on an unchanged tree, capture verbatim output + classify
per the PRP's Level-4 failure triage, and report REGRESSION.)
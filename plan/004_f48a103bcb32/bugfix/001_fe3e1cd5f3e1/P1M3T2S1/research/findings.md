# Research findings — P1.M3.T2.S1 (release NOTIFIER per sweep iteration)

## Task
Restructure `perform_handshake_with` (`src/core/notifier.rs:421`) so the callback
sweep (the `for i in 0..sweep_cap` loop) acquires/releases the `NOTIFIER` mutex
**per iteration** instead of holding it for the whole sweep. Closes bug-hunt
Finding #4 (PRD `h2.1` #4).

## Verified current state (this session)

### The lock and its scope
- `static NOTIFIER: Lazy<Arc<Mutex<Box<dyn Notifier>>>>` — `notifier.rs:764`.
- `fn get_notifier() -> Arc<Mutex<Box<dyn Notifier>>>` — `notifier.rs:829` (Arc clone).
- `perform_handshake_with(verbose, opts)` — `notifier.rs:421`.
  - Dedup guard `HAS_HANDSHAKED.swap(true, …)` — `:425`.
  - **Lock acquired** `let n = notifier.lock().unwrap();` — `:434`.
  - `match n.send_command(QueryInfo, &filter)` — `:437` (outer match scrutinee; `n` borrowed only for the call, freed after).
  - Info-capable arm (`:438-445`) → `SetOs` (`:454-458`, uses `n`) → sweep.
  - **Sweep loop** `for i in 0..sweep_cap` — `:449-514`. Deadline check at top
    (`:450-462`). Per-iteration `match n.send_command(QueryCallback(i), &filter)`
    (`:463`). Inside the loop `n` is used ONLY for that one `send_command`.
  - **Post-loop** `drop(n); // release the notifier before the read-only rules validation` — `:515`.
  - Then publishes `CALLBACK_NAMES` (`:516-520`), stores `BOARD_HAS_RULES` + `HOST_CAPABLE` (`:535-536`).
  - The Timeout (`:528`)/non-capable `Ok(other)` (`:539`)/`Err` (`:550`) outer-match arms each `drop(n)` early — **unchanged by this task**.

### Why notifications block (architecture/handshake_race_research.md §Finding #4)
Both window-notification SEND paths lock the SAME `NOTIFIER` mutex to do the HID write:
- `notify_qmk` immediate send — `notifier.rs:919`, lock at `:956-957`.
- `debounce_worker` flush — `notifier.rs:838`, lock at `:888-889`.
`dispatch_window_send` → `notifier.notify/send_command` → `qmk_notifier::run` opens
the HID device **independently each call** (no shared device-handle state between
calls) ⇒ releasing/re-acquiring `NOTIFIER` between iterations is SAFE.

### Constants
- `MAX_HOST_CALLBACKS: u8 = 64` — `notifier.rs:370`.
- `CALLBACK_SWEEP_DEADLINE = Duration::from_secs(5)` — `:379`.
- Worst-case stall today ≈ 5 s sweep + ~1 s overdue callback + ~1 s QueryInfo + ~1 s SetOs ≈ 6–8 s.

## The precise restructure (Info-capable arm only)
1. Right after the `SetOs` block, before the sweep comment/setup: `drop(n);`
   (release the QueryInfo+SetOs lock).
2. Reword the `#4` comment → "secondary bound" (per-iteration release is now primary).
3. Keep deadline check at the TOP of each iteration, but now it runs BEFORE re-locking.
4. Inside the loop, before `match n.send_command(QueryCallback(i)…)`: add
   `let n = notifier.lock().unwrap();` (shadows the dropped outer `n` — legal;
   the outer `n` is moved by the pre-loop `drop`, and is never referenced after).
5. At the end of the loop body (after the `match` close): `drop(n);` (per-iteration release).
6. **Remove** the old post-loop `drop(n);` at `:515` (the guard is now dropped each iteration).

Borrow-checker reasoning: the outer `n` is used by the QueryInfo scrutinee + SetOs,
then dropped before the loop. The per-iteration `n` is loop-scoped. After the loop,
code touches only `CALLBACK_NAMES` (separate mutex) + atomics (`BOARD_HAS_RULES`/
`HOST_CAPABLE`) + `validate_rules_callback_names` — no `n` reference ⇒ compiles.

## Test infra needed (test-only, `#[cfg(test)] mod tests`)
`MockNotifier::send_command` (`:1306`) does NOT sleep today → the sweep runs in
microseconds, too fast for a contending thread to land mid-sweep. Add:
- `static MOCK_SEND_DELAY: Lazy<StdMutex<Option<Duration>>>` (next to `MOCK_SEND_COMMAND_ERRORS`).
- reset it in `reset_global_mock()`.
- `MockNotifier::set_send_delay(Option<Duration>)` setter.
- a `thread::sleep` at the top of `send_command` when `Some` (wall-clock, so CI
  CPU slowdown can't shrink it).

`thread::sleep` / `thread::spawn` are available in the test mod (`use super::*`
globs the module-level `use std::thread;` at `:8`; existing tests already use
`thread::sleep`, e.g. `reset_test_state`).

## Test design (`test_handshake_sweep_releases_lock_between_iterations`)
- Capable board, `n_callbacks = 10`, `per_call_delay = 100 ms` (sweep ≈ 1 s).
- Responses: `Info` + `Ack` (SetOs, ignored but consumed FIFO) + 10× `CallbackName{i, Some("cb_i")}`.
- Reset dedup via `reset_handshake_state()` (matches existing handshake tests).
- Spawn handshake on a worker thread; spin-wait until ≥3 `send_command` calls
  logged (QueryInfo + SetOs + ≥1 QueryCallback ⇒ definitely INSIDE the sweep).
- Main thread contends for `NOTIFIER`; measure `waited` and snapshot
  `calls_when_acquired`.
- **Deterministic assertion**: `calls_when_acquired < 2 + n_callbacks` — the
  contender grabbed the lock WHILE the sweep was still in progress (impossible if
  the lock were held for the whole sweep, since then the contender could only
  acquire after all 2+N calls).
- **Corroborating timing**: `waited < 500 ms` (per-iter ≈ 100–250 ms; full-sweep
  ≈ 800 ms remaining).
- After `h.join()`: `CALLBACK_NAMES` has all 10 entries; `host_capable()` true.
- Clean up: `set_send_delay(None)`.

## Baseline
`cargo test --bin qmkonnect -- --test-threads=1` ⇒ **350 passed; 0 failed**
(P1.M3.T1.S1 already landed). This task adds **1** test → expect **351**.

## Scope boundaries (siblings)
- P1.M3.T1.S1 (Implementing/Complete): added `STARTUP_DEVICE_CONNECTED` +
  `record_startup_device_state`/`startup_device_was_connected` near `:264`/`:228`
  and runner seeds. It does NOT touch `perform_handshake_with` internals ⇒ no
  overlap. `OnceLock` is already imported (`:7`) — we add NO import.
- No Cargo.toml change. No new deps. Pure restructure + 1 test + test infra.
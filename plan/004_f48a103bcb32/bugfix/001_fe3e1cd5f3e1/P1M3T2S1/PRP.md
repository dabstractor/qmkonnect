# PRP — P1.M3.T2.S1: Release `NOTIFIER` lock per sweep iteration in `perform_handshake_with`

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`) — Rust menu-bar/tray daemon.
> **Files edited (1):** `src/core/notifier.rs` (restructure of the Info-capable sweep arm in
> `perform_handshake_with`; 1 new test; 4 small test-infra additions inside `#[cfg(test)]`).
> **Scope:** close bug-hunt **Finding #4** (PRD `h2.1` #4). `perform_handshake_with` runs the
> `QUERY_CALLBACK` sweep while holding the global `NOTIFIER` mutex for its entire duration
> (bounded by `CALLBACK_SWEEP_DEADLINE` = 5 s, plus ~1–3 s of pre-sweep `QueryInfo`/`SetOs`/
> over-deadline `QueryCallback`). During that window **every window notification** (`notify_qmk`'s
> immediate-send arm and `debounce_worker`'s flush — both lock `NOTIFIER` to do the HID write) is
> blocked. Fix: release the `QueryInfo`+`SetOs` lock **before** the sweep and re-acquire `NOTIFIER`
> **per iteration** inside the loop, so a notification send can interleave between any two
> `QueryCallback` iterations. The deadline/cap remain as defense-in-depth.
> **Dependency:** none. Builds on the current `perform_handshake_with` (`notifier.rs:421`).
> `OnceLock` is already imported (`:7`, from P1.M3.T1.S1) — this task adds NO imports.

---

## Goal

**Feature Goal**: Eliminate the up-to-~5–8 s window in which `perform_handshake_with`'s callback
sweep monopolizes the global `NOTIFIER` mutex, blocking all window notifications. After this change
the sweep acquires `NOTIFIER` fresh at the top of each iteration and drops it at the bottom, so a
`notify_qmk` immediate-send or a `debounce_worker` flush can acquire `NOTIFIER` between any two
`QueryCallback(i)` sends. `QueryInfo` + `SetOs` stay under a single lock acquisition (they are quick
and must precede the sweep). The `sweep_start.elapsed() > CALLBACK_SWEEP_DEADLINE` check stays at
the top of each iteration (now evaluated **before** re-locking). `CALLBACK_NAMES` is still published
**atomically after** the sweep; `BOARD_HAS_RULES`/`HOST_CAPABLE` are still set after the sweep.

**Deliverable** (all in `src/core/notifier.rs`):
1. Restructure the Info-capable arm of `perform_handshake_with` (the only arm that sweeps):
   drop the outer `n` guard before the sweep, re-acquire a fresh guard per loop iteration, drop it
   at the end of each iteration, and remove the now-redundant post-loop `drop(n)`.
2. Reword the `#4` comment to note the lock is now released per-iteration (primary mitigation) with
   the cap+deadline as secondary bound.
3. Add 4 small test-only infra pieces (a `MOCK_SEND_DELAY` static, its reset, a `set_send_delay`
   setter, and a `thread::sleep` in `MockNotifier::send_command`) so the sweep has a measurable,
   controllable duration.
4. Add the test `test_handshake_sweep_releases_lock_between_iterations`.

**Success Definition**:
- `cargo test --bin qmkonnect -- --test-threads=1` is green and the count rises **350 → 351**.
- The new test proves a contending thread acquires `NOTIFIER` **while the sweep is still in
  progress** (`calls_when_acquired < 2 + n_callbacks`) — impossible under the old full-sweep hold.
- All pre-existing handshake tests (`test_handshake_capable_populates_state`,
  `test_handshake_legacy_proto_v1_string_only`, `test_handshake_timeout_*`, `test_handshake_dedup_*`,
  `test_handshake_reset_allows_rerun`, …) still pass byte-for-byte in behavior.
- `git diff --stat` shows **only `src/core/notifier.rs`**.

## User Persona (if applicable)

**Target User**: the QMKonnect **notification path** (`notify_qmk` + `debounce_worker`) and,
indirectly, the end user whose window-focus updates must reach the keyboard promptly even while a
(re)connect handshake is mid-flight against a slow/buggy board.

**Use Case**: A capable QMK board is (re)connecting and reports N callbacks. The handshake sweeps
them with `QUERY_CALLBACK(i)`. Concurrently the user switches windows rapidly. Before this fix, the
handshake held `NOTIFIER` for the whole sweep (up to ~5 s on a buggy board), so every window change
in that window stalled. After this fix, each `QUERY_CALLBACK` iteration releases `NOTIFIER`, so
window notifications interleave between iterations.

**User Journey**: (1) Board (re)connects → runner calls `perform_handshake` → `perform_handshake_with`
runs. (2) `QueryInfo` + `SetOs` under one lock, then lock released. (3) Sweep loop: lock →
`QueryCallback(i)` → process → unlock, repeat. (4) A `notify_qmk` call between two iterations
acquires `NOTIFIER`, sends the window update, releases — the sweep resumes on the next iteration.
(5) Sweep done → `CALLBACK_NAMES` published atomically → `HOST_CAPABLE=true`.

**Pain Points Addressed**: the bounded-but-user-visible notification stall (Finding #4) during the
one-per-board-boot handshake. The fix turns a single ~5 s stall into N brief (~tens of ms on a
healthy board) lock holds between which notifications flow freely.

## Why

- **Closes Finding #4 of the bug-hunt report (PRD `h2.1` #4).** `architecture/handshake_race_research.md`
  §Finding #4 documents the race end-to-end and explicitly recommends this design ("A cleaner
  (non-blocking) design would release/re-acquire `NOTIFIER` per sweep iteration"). This task
  implements exactly that recommended fix.
- **Safe by construction.** Each `send_command` opens the HID device independently inside
  `qmk_notifier::run` (no shared device-handle state between calls), so releasing and re-acquiring
  `NOTIFIER` between iterations cannot corrupt a device session. `CALLBACK_NAMES` is published
  atomically after the sweep, and `BOARD_HAS_RULES`/`HOST_CAPABLE` are set after the sweep, so a
  concurrent notification during the sweep observes only the pre-handshake (stale/empty) callback
  map and the pre-handshake capability flags — identical to the state before any handshake ran.
- **No behavior change for a healthy board.** A real keyboard (handful of callbacks, each replying
  in well under a second) completes the sweep in tens of milliseconds either way; the only change is
  that notifications are no longer blocked across that window. The `QueryCallback` ordering is
  unchanged (still sequential `0, 1, 2, …`) — the firmware processes commands FIFO and callback
  indices are positional, so per-iteration re-locking is transparent to the device.

## What

### Code changes — all in `src/core/notifier.rs`

**A. `perform_handshake_with` — Info-capable arm restructure (production logic).**
The change is confined to the Info-capable match arm (the only arm that sweeps). The other three
outer-match arms (`Timeout`, non-capable `Ok(other)`, `Err`) each `drop(n)` early and are **untouched**.
1. Immediately after the `SetOs` block (`if opts.set_os { … }`), add `drop(n);` to release the
   `QueryInfo`+`SetOs` lock before the sweep.
2. Reword the comment above the sweep setup: per-iteration release is now the primary mitigation;
   `MAX_HOST_CALLBACKS` + `CALLBACK_SWEEP_DEADLINE` are now a secondary bound (defense-in-depth).
3. Keep the `sweep_start.elapsed() > CALLBACK_SWEEP_DEADLINE` check at the **top** of each iteration
   (add a one-line note that it now runs before re-locking).
4. Inside the loop, immediately before `match n.send_command(QueryCallback(i), &filter)`, acquire a
   fresh guard: `let n = notifier.lock().unwrap();` (shadows the dropped outer `n`).
5. At the end of the loop body (after the `match` block closes), `drop(n);` to release before the
   next iteration.
6. **Remove** the old post-loop `drop(n); // release the notifier before the read-only rules validation`
   (the guard is now dropped each iteration; the outer `n` is no longer alive there).

**B. Test infra (test-only, inside `#[cfg(test)] mod tests`).**
7. New static `MOCK_SEND_DELAY: Lazy<StdMutex<Option<Duration>>>` (next to `MOCK_SEND_COMMAND_ERRORS`).
8. Reset it in `reset_global_mock()`: `*MOCK_SEND_DELAY.lock().unwrap() = None;`.
9. New `MockNotifier::set_send_delay(delay: Option<Duration>)` setter.
10. In `MockNotifier::send_command`, after pushing to `MOCK_SEND_COMMAND_CALLS`, sleep the configured
    delay when `Some` (`if let Some(d) = *MOCK_SEND_DELAY.lock().unwrap() { thread::sleep(d); }`).

**C. New test.**
11. `test_handshake_sweep_releases_lock_between_iterations` (body in Implementation Tasks Task 6).

### Success Criteria
- [ ] Info-capable arm drops the outer `n` before the sweep; the loop acquires/drops a fresh `n` per iteration.
- [ ] The post-loop `drop(n); // release the notifier before the read-only rules validation` line is GONE.
- [ ] The `Timeout` / non-capable `Ok(other)` / `Err` outer-match arms are byte-identical to before.
- [ ] `QueryCallback` is still sent sequentially `0..sweep_cap` (no reordering).
- [ ] `CALLBACK_NAMES` is still published atomically after the sweep (`local` → `CALLBACK_NAMES`), and `BOARD_HAS_RULES`/`HOST_CAPABLE` are still stored after the sweep.
- [ ] New test infra: `MOCK_SEND_DELAY` static + reset + `set_send_delay` + sleep in `send_command`.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` → **351 passed; 0 failed** (was 350).
- [ ] `git diff --stat` shows only `src/core/notifier.rs`.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can implement every edit from the exact grep-confirmed
anchors below (the sweep loop, the `#4` comment, the `drop(n)` line, the MockNotifier infra), the
verified borrow-checker reasoning (outer `n` dropped pre-loop; per-iteration `n` loop-scoped; no `n`
reference after the loop), the existing handshake-test template (`test_handshake_capable_populates_state`),
and the AGENTS.md single-threaded `cargo test` gate.

### Documentation & References

```yaml
# MUST READ — the bug-hunt analysis that defines this task (the "why" + the recommended fix)
- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/architecture/handshake_race_research.md
  why: "Finding #4 traces the exact stall: the sweep holds NOTIFIER for up to ~5 s (CALLBACK_SWEEP_DEADLINE)
        plus ~1-3 s of pre/over-sweep sends; both window-notification send paths (notify_qmk immediate + 
        debounce_worker flush) need NOTIFIER to do the HID write, so they block for that whole window.
        Each send_command opens the HID device independently (qmk_notifier::run), so there is NO shared
        device-handle state between calls — releasing/re-acquiring NOTIFIER between iterations is safe."
  critical: "the mutex in question is NOTIFIER (notifier.rs:764), NOT the STATE DebounceState mutex and
        NOT the CALLBACK_NAMES mutex. Both send paths already release STATE before taking NOTIFIER
        (notify_qmk dequeues at :927-949 then locks NOTIFIER at :956; the worker dequeues inside STATE
        at :843-881 then locks NOTIFIER at :888), so the contention is purely on NOTIFIER. The
        recommended fix is verbatim what this task implements: 'release/re-acquire NOTIFIER per sweep
        iteration.' CALLBACK_NAMES is published AFTER the sweep, so a concurrent send during the sweep
        sees the pre-handshake (stale/empty) map — acceptable."
  section: "Finding #4 — Handshake holds the NOTIFIER mutex during the sweep"

# MUST READ — PRD context (the defect this fixes)
- url: spec/PRD.md (heading h2.1, finding #4 "Handshake holds the global notifier mutex up to 5 s")
  why: "the PRD-recorded defect; severity Low/Medium (once per board boot, deduped, only at
        (re)connect). 'worth noting' / 'acceptable, but worth noting.' This task upgrades it from
        'tolerated' to 'fixed'."

# MUST READ — the file owning the restructure + the test (exact current state, verified this session)
- file: src/core/notifier.rs
  why: "the ONLY file edited. perform_handshake_with (:421) acquires n at :434; the Info-capable arm
        (:438) does SetOs (:454) then the sweep loop (:449-514) under n, then drop(n) at :515 before
        publishing CALLBACK_NAMES (:516-520). The restructure touches ONLY the Info arm. The MockNotifier
        (:1234+) + reset_global_mock (:1259) + send_command (:1306) get the delay infra. The test goes in
        the existing #[cfg(test)] mod tests (:1240, use super::* at :1241)."
  pattern: "production fns carry `///` doc + `pub fn name(…) { … }`. Tests are `#[test] fn test_subject_scenario()`
        with `reset_test_state(); reset_handshake_state(); set_notifier(Box::new(MockNotifier::new()));`
        preamble (see test_handshake_capable_populates_state :1777). Mock responses are consumed FIFO
        via VecDeque::pop_front; SetOs consumes one response even though its reply is ignored."
  gotcha: "OnceLock is ALREADY imported (:7, from P1.M3.T1.S1) — do NOT touch imports. std::thread is
        imported at module level (:8) and is globbed into the test mod by `use super::*`, so
        thread::sleep / thread::spawn work in tests (existing tests already use them)."

# REFERENCE — the existing capable-handshake test (the template for the new test's response setup)
- file: src/core/notifier.rs
  why: "test_handshake_capable_populates_state (:1777) shows the exact response vector shape: Info +
        Ack(SetOs) + N×CallbackName, and the assertion style (host_capable(), callback_names(),
        get_send_command_calls() ordering). Mirror it."
  pattern: "responses are pushed in CALL ORDER: [Info, Ack(SetOs), CallbackName{0}, CallbackName{1}, …].
        SetOs's reply is ignored by the code (only Err is checked) but the mock still pops it FIFO, so
        you MUST queue an Ack for SetOs or the next QueryCallback gets the wrong response."

# REFERENCE — the sibling task (no overlap; different region of the same file)
- file: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/P1M3T1S1/PRP.md
  why: "P1.M3.T1.S1 (Implementing/Complete) added STARTUP_DEVICE_CONNECTED + 2 pub fns near :228/:264
        + runner seeds. It does NOT touch perform_handshake_with internals. Different region of
        notifier.rs => low merge conflict. It already imported OnceLock (:7) so this task adds NO import."
  critical: "do NOT duplicate P1.M3.T1.S1's work. This task is confined to perform_handshake_with's
        Info-capable sweep arm + the MockNotifier/test block."
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
src/core/notifier.rs   # EDIT ONLY THIS FILE
  - :7    use std::sync::{Arc, Condvar, Mutex, OnceLock};   (OnceLock already present — UNCHANGED)
  - :8    use std::thread;                                   (thread::sleep/spawn — UNCHANGED)
  - :9    use std::time::{Duration, Instant};                (UNCHANGED)
  - :370  const MAX_HOST_CALLBACKS: u8 = 64;                 (UNCHANGED — secondary bound)
  - :379  const CALLBACK_SWEEP_DEADLINE: Duration = Duration::from_secs(5);  (UNCHANGED)
  - :421  pub fn perform_handshake_with(verbose, opts)       ← RESTRUCTURE the Info-capable arm
       - :434   let n = notifier.lock().unwrap();            (outer lock; used by QueryInfo+SetOs)
       - :437   match n.send_command(QueryInfo, &filter)     (outer match)
       - :454   if opts.set_os { n.send_command(SetOs …) }   (under outer lock)
       - :449-514  for i in 0..sweep_cap { … match n.send_command(QueryCallback(i)) … }  ← per-iter lock
       - :515  drop(n); // release the notifier before the read-only rules validation   ← REMOVE
       - :516-520 { publish CALLBACK_NAMES }   (UNCHANGED)
       - :535-536 BOARD_HAS_RULES / HOST_CAPABLE stores      (UNCHANGED)
       - :528/:539/:550  Timeout/non-capable/Err arms each drop(n)   (UNCHANGED)
  - :764  static NOTIFIER: Lazy<Arc<Mutex<Box<dyn Notifier>>>>         (UNCHANGED)
  - :829  fn get_notifier() -> Arc<Mutex<Box<dyn Notifier>>>          (UNCHANGED)
  - :838  fn debounce_worker() { … notifier.lock() at :888-889 … }    (UNCHANGED — the blocked path)
  - :919  pub fn notify_qmk() { … notifier.lock() at :956-957 … }     (UNCHANGED — the blocked path)
  - :1240 #[cfg(test)] mod tests { use super::*; … }                  ← ADD test infra + 1 test
       - :1247-1257  MOCK_* statics                      ← ADD MOCK_SEND_DELAY nearby
       - :1259  fn reset_global_mock()                   ← ADD MOCK_SEND_DELAY reset
       - :1287-1293  MockNotifier::set_mock_* setters    ← ADD set_send_delay
       - :1306-1322  impl Notifier for MockNotifier::send_command   ← ADD the sleep
       - :1777  test_handshake_capable_populates_state   (the template)
Cargo.toml   # DO NOT TOUCH (no new deps; std only)
```

### Desired Codebase tree
No new files, no new modules, no new imports. The 6 in-file changes (A.1–6 restructure +
B.7–10 test infra + C.11 test) in `src/core/notifier.rs` are the entirety of the change.

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (BORROW CHECKER — why the restructure compiles): the outer `let n = notifier.lock()`
//   (:434) is a MutexGuard (not Copy). The Info-capable arm uses it for the QueryInfo scrutinee
//   (:437) and SetOs (:454), then we `drop(n)` before the sweep. After that drop the outer `n` is
//   MOVED — it must NEVER be referenced again. The per-iteration `let n = notifier.lock().unwrap();`
//   inside the loop SHADOWS the moved outer binding in a loop-local scope (legal). After the loop,
//   the code touches only CALLBACK_NAMES (a SEPARATE mutex), BOARD_HAS_RULES/HOST_CAPABLE (atomics),
//   and validate_rules_callback_names — NO `n` reference. That is why the old post-loop `drop(n)`
//   (:515) MUST be removed: `n` no longer names a live guard there.

// CRITICAL (only the Info-capable arm changes): the outer match has 4 arms. The Info arm is the
//   only one that sweeps. The Timeout (:528), non-capable Ok(other) (:539), and Err (:550) arms
//   each drop the outer `n` themselves and NEVER reach the sweep. Leave them byte-identical. The
//   contract: "Preserve all existing Timeout/Err/Info/non-capable arms unchanged (they already
//   drop(n) early)." (The Info arm is the capable one; its early-drop siblings are the other three.)

// CRITICAL (do NOT reorder QueryCallback): still sequential 0,1,2,… — the firmware processes
//   commands FIFO and callback indices are POSITIONAL. Re-acquiring the lock per iteration does not
//   change the send order; do not parallelize or reorder.

// CRITICAL (CALLBACK_NAMES publication is unchanged): `local` is still accumulated in the loop and
//   published ONCE after the loop via `CALLBACK_NAMES.lock().clear()+extend(local)`. Do NOT publish
//   per-iteration — a concurrent notification must see EITHER the old map OR the fully-built new map,
//   never a partial one. The atomic post-sweep publish is what makes a concurrent send safe.

// CRITICAL (BOARD_HAS_RULES/HOST_CAPABLE timing unchanged): both are stored AFTER the sweep (:535-536)
//   and BOARD_HAS_RULES is set BEFORE HOST_CAPABLE (the #5 ordering fix). A concurrent notification
//   during the sweep sees HOST_CAPABLE=false (pre-handshake) → host rules disabled → legacy string
//   path. Do NOT move these stores into the loop.

// GOTCHA (SetOs consumes a mock response): MockNotifier::send_command pops MOCK_RESPONSES FIFO
//   (:1318) regardless of which RunCommand called it. perform_handshake_with ignores SetOs's reply
//   (only Err is checked at :455) but the mock STILL pops one entry. So the test's response vector
//   MUST include an Ack for SetOs between Info and the first CallbackName (see
//   test_handshake_capable_populates_state :1782-1796). Omitting it shifts every CallbackName by one.

// GOTCHA (MOCK_SEND_DELAY is wall-clock thread::sleep): CPU slowdown on a CI box does NOT shrink it
//   (sleep is wall-clock). That is WHY it reliably widens the sweep window for the contending-thread
//   test. Reset it to None at the end of the test (and in reset_global_mock) so it cannot bleed into
//   the other ~350 single-threaded tests.

// GOTCHA (tests MUST run single-threaded): `cargo test --bin qmkonnect -- --test-threads=1`
//   (AGENTS.md — shared global debouncer/mock state). Parallel runs flap.

// GOTCHA (do NOT add imports): OnceLock (:7), thread (:8), Duration/Instant (:9) are all already
//   imported. The test mod globs them via `use super::*` (:1241). No new `use` lines anywhere.
```

## Implementation Blueprint

### Data models and structure
None. No new types, no new production state. The change restructures lock acquisition inside one
match arm of `perform_handshake_with`. The only new state is the test-only `MOCK_SEND_DELAY:
Lazy<StdMutex<Option<Duration>>>`. `CALLBACK_NAMES` / `HOST_CAPABLE` / `BOARD_HAS_RULES` /
`HAS_HANDSHAKED` and the `HandshakeAction` lifecycle are unchanged.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: EDIT src/core/notifier.rs — release the outer lock before the sweep + reword the #4 comment
  Locate the Info-capable arm, immediately after the SetOs block. The current text is:
      "            // Callback sweep → local map (publish after dropping the notifier lock: D2).\n            // #4: bound both the count (`MAX_HOST_CALLBACKS`) and the wall clock\n            // (`CALLBACK_SWEEP_DEADLINE`) so a misbehaving firmware cannot wedge\n            // the global notifier mutex (and every notification behind it).\n            let sweep_start = Instant::now();\n"
  Replace with:
      "            // Release the QueryInfo+SetOs lock BEFORE the sweep. The sweep now re-acquires\n            // NOTIFIER per iteration (#4) so a window-notification send (`notify_qmk`'s\n            // immediate arm or `debounce_worker`'s flush) can acquire it between any two\n            // `QueryCallback` iterations instead of blocking for the whole sweep. Each\n            // `send_command` opens the HID device independently (`qmk_notifier::run`), so\n            // releasing and re-acquiring between iterations is safe — no shared device-handle\n            // state. `CALLBACK_NAMES` is published atomically AFTER the sweep, and\n            // `BOARD_HAS_RULES`/`HOST_CAPABLE` are set after it too, so a concurrent\n            // notification during the sweep only sees the pre-handshake (stale/empty) callback\n            // map — identical to the pre-handshake state.\n            drop(n);\n            // Callback sweep → local map (publish after the sweep: D2).\n            // #4 (secondary bound): `MAX_HOST_CALLBACKS` + `CALLBACK_SWEEP_DEADLINE` still cap a\n            // misbehaving firmware per iteration as defense-in-depth (the primary mitigation is\n            // the per-iteration lock release above).\n            let sweep_start = Instant::now();\n"
  - NOTE: the `drop(n);` line is the new release of the outer guard. It is placed BEFORE
    `let sweep_start` so the lock is free during all the (cheap) sweep setup.
  - PRESERVE: the `if callback_count > MAX_HOST_CALLBACKS { … }` warning block and everything below
    it down to `let mut local: HashMap<String, u8> = HashMap::new();`.

Task 2: EDIT src/core/notifier.rs — note the deadline check now runs before re-locking
  The current loop-top text is:
      "            for i in 0..sweep_cap {\n                if sweep_start.elapsed() > CALLBACK_SWEEP_DEADLINE {\n"
  Replace with:
      "            for i in 0..sweep_cap {\n                // #4: deadline check at the TOP of each iteration, BEFORE re-locking NOTIFIER,\n                // so a per-iteration release actually hands the lock to a waiter before we stop.\n                if sweep_start.elapsed() > CALLBACK_SWEEP_DEADLINE {\n"
  - PRESERVE: the entire deadline `eprintln!` + `break;` body (unchanged).

Task 3: EDIT src/core/notifier.rs — acquire NOTIFIER fresh per iteration (before the QueryCallback match)
  The current match-open line is:
      "                match n.send_command(qmk_notifier::RunCommand::QueryCallback(i), &filter) {\n"
  Replace with:
      "                // Re-acquire NOTIFIER for THIS iteration only — a window notification can now\n                // interleave between any two iterations.\n                let n = notifier.lock().unwrap();\n                match n.send_command(qmk_notifier::RunCommand::QueryCallback(i), &filter) {\n"
  - NOTE: `let n` here SHADOWS the (already-dropped) outer `n` in a loop-local scope. Legal: the
    outer `n` was moved by Task 1's `drop(n)` and is never referenced again after the loop.
  - PRESERVE: the entire match body (the CallbackName/None/other/Err arms) byte-for-byte.

Task 4: EDIT src/core/notifier.rs — drop the per-iteration guard + remove the old post-loop drop
  The current loop-close + old post-loop drop text is:
      "                    Err(e) => {\n                        eprintln!(\"Warning: QUERY_CALLBACK({}) failed: {}\", i, e);\n                    }\n                }\n            }\n            drop(n); // release the notifier before the read-only rules validation\n"
  Replace with:
      "                    Err(e) => {\n                        eprintln!(\"Warning: QUERY_CALLBACK({}) failed: {}\", i, e);\n                    }\n                }\n                drop(n); // release NOTIFIER before the next iteration (per-iteration release)\n            }\n"
  - NOTE: this (a) adds the per-iteration `drop(n);` as the LAST statement of the for-body, and
    (b) DELETES the old `drop(n); // release the notifier before the read-only rules validation`
    that followed the loop (the outer guard no longer exists there). The line that followed the
    deleted drop — `{ let mut names = CALLBACK_NAMES.lock().unwrap(); … }` — is now the first
    statement after the loop. PRESERVE it and everything below (CALLBACK_NAMES publish, the
    #5 BOARD_HAS_RULES-before-HOST_CAPABLE ordering, the verbose log, …).
  - VERIFY after this task: grep for the old comment — it must be GONE:
      grep -n 'release the notifier before the read-only rules validation' src/core/notifier.rs
    → expected: no matches.

Task 5: EDIT src/core/notifier.rs — add the test-only MOCK_SEND_DELAY infra (#[cfg(test)] mod tests)
  Sub-task 5a (static): immediately AFTER the MOCK_SEND_COMMAND_ERRORS static block:
      "    // (instead of consulting MOCK_RESPONSES).\n    static MOCK_SEND_COMMAND_ERRORS: Lazy<StdMutex<VecDeque<String>>> =\n        Lazy::new(|| StdMutex::new(VecDeque::new()));\n"
    Append:
      "    // P1.M3.T2.S1 (#4): optional per-`send_command` artificial delay so the callback sweep\n    // has a measurable, controllable duration. Used by\n    // test_handshake_sweep_releases_lock_between_iterations to prove NOTIFIER is released\n    // between sweep iterations. `None` in production and by default.\n    static MOCK_SEND_DELAY: Lazy<StdMutex<Option<Duration>>> = Lazy::new(|| StdMutex::new(None));\n"
  Sub-task 5b (reset): in reset_global_mock(), the current body ends with:
      "        MOCK_SEND_COMMAND_ERRORS.lock().unwrap().clear();\n    }\n"
    Replace with:
      "        MOCK_SEND_COMMAND_ERRORS.lock().unwrap().clear();\n        *MOCK_SEND_DELAY.lock().unwrap() = None;\n    }\n"
  Sub-task 5c (setter): immediately AFTER the set_mock_send_errors method:
      "        fn set_mock_send_errors(errors: Vec<String>) {\n            MOCK_SEND_COMMAND_ERRORS.lock().unwrap().extend(errors);\n        }\n    }\n"
    Replace with (insert the new setter BEFORE the closing `}` of the impl block):
      "        fn set_mock_send_errors(errors: Vec<String>) {\n            MOCK_SEND_COMMAND_ERRORS.lock().unwrap().extend(errors);\n        }\n\n        /// P1.M3.T2.S1 (#4): inject a per-`send_command` wall-clock delay so the sweep is wide\n        /// enough for a contending thread to acquire NOTIFIER between iterations. Pass `None`\n        /// to disable (the default; production code never sets this).\n        fn set_send_delay(delay: Option<Duration>) {\n            *MOCK_SEND_DELAY.lock().unwrap() = delay;\n        }\n    }\n"
  Sub-task 5d (sleep): in impl Notifier for MockNotifier::send_command, the current head is:
      "            MOCK_SEND_COMMAND_CALLS\n                .lock()\n                .unwrap()\n                .push(command.clone());\n            // LOW-1: if an error is queued, return it (drains one per call).\n"
    Replace with (insert the sleep between the push and the LOW-1 comment):
      "            MOCK_SEND_COMMAND_CALLS\n                .lock()\n                .unwrap()\n                .push(command.clone());\n            // P1.M3.T2.S1 (#4): optional artificial delay to widen the sweep window for the\n            // per-iteration lock-release test (wall-clock sleep, so CI CPU slowdown can't shrink it).\n            if let Some(d) = *MOCK_SEND_DELAY.lock().unwrap() {\n                thread::sleep(d);\n            }\n            // LOW-1: if an error is queued, return it (drains one per call).\n"
  - NOTE: `thread::sleep` resolves because the test mod globs the module-level `use std::thread;`
    (:8) via `use super::*;` (:1241) — existing tests already call thread::sleep (e.g. reset_test_state).
  - PRESERVE: the rest of send_command (the error-drain + MOCK_RESPONSES pop_front + Ack default).

Task 6: ADD the test to src/core/notifier.rs #[cfg(test)] mod tests
  Place it immediately AFTER test_handshake_capable_populates_state (the natural neighbor). Its
  closing `}` is the anchor; append the new test right after it. The test body:
      "    /// #4 / P1.M3.T2.S1: the callback sweep must release NOTIFIER between iterations so a\n    /// window-notification send (`notify_qmk` immediate / `debounce_worker` flush) can acquire\n    /// it between any two `QueryCallback` sends instead of blocking for the whole (up to ~5 s)\n    /// sweep. We configure a capable board with N callbacks and an artificial per-`send_command`\n    /// delay that widens the sweep window, then prove a contending thread can grab NOTIFIER WHILE\n    /// the sweep is still in progress (fewer than 2+N `send_command` calls outstanding) and far\n    /// sooner than the full sweep would take if the lock were held throughout.\n    #[test]\n    fn test_handshake_sweep_releases_lock_between_iterations() {\n        reset_test_state();\n        reset_handshake_state();\n        set_notifier(Box::new(MockNotifier::new()));\n\n        let n_callbacks: u8 = 10;\n        // Per-call delay widens the sweep so a contending thread can land mid-sweep. Total sweep\n        // ≈ 10*100ms = 1s (plus ~200ms pre-sweep QueryInfo+SetOs).\n        let per_call_delay = Duration::from_millis(100);\n        MockNotifier::set_send_delay(Some(per_call_delay));\n\n        let mut responses = vec![qmk_notifier::CommandResponse::Info {\n            proto_ver: 2,\n            feature_flags: 0x01,\n            callback_count: n_callbacks,\n            board_rules_present: false,\n        }];\n        // SetOs consumes one response (its reply is ignored, but the mock still pops FIFO — see\n        // test_handshake_capable_populates_state). Queue an Ack for it.\n        responses.push(qmk_notifier::CommandResponse::Ack { ok: true });\n        for i in 0..n_callbacks {\n            responses.push(qmk_notifier::CommandResponse::CallbackName {\n                index: i,\n                name: Some(format!(\"cb_{}\", i)),\n            });\n        }\n        MockNotifier::set_mock_responses(responses);\n\n        // Run the handshake on a worker thread so the main thread can contend for NOTIFIER while\n        // the sweep is in progress.\n        let h = thread::spawn(move || {\n            perform_handshake(false);\n        });\n\n        // Wait until the handshake is INSIDE the sweep (QueryInfo + SetOs + at least one\n        // QueryCallback have been sent), so the contender provably contends DURING the sweep\n        // rather than before it starts. 2s is ample (the first 3 calls take ~300ms wall).\n        let deadline = Instant::now() + Duration::from_millis(2000);\n        loop {\n            if MockNotifier::get_send_command_calls().len() >= 3 {\n                break;\n            }\n            if Instant::now() >= deadline {\n                panic!(\"handshake never entered the sweep (call count < 3)\");\n            }\n            thread::sleep(Duration::from_millis(5));\n        }\n\n        // Contend for NOTIFIER. With per-iteration release the contender acquires it between two\n        // QueryCallback iterations; with a full-sweep hold it could only acquire after the sweep.\n        let notifier = get_notifier();\n        let contend_start = Instant::now();\n        let _guard = notifier.lock().unwrap();\n        let waited = contend_start.elapsed();\n        let calls_when_acquired = MockNotifier::get_send_command_calls().len();\n        drop(_guard);\n        h.join().unwrap();\n\n        // DETERMINISTIC PROOF: the contender grabbed the lock WHILE the sweep was still in\n        // progress — fewer than 2 (QueryInfo+SetOs) + N callbacks outstanding. If the lock were\n        // held for the whole sweep, the contender could only acquire after all 2+N calls.\n        assert!(\n            calls_when_acquired < 2 + n_callbacks as usize,\n            \"contender acquired NOTIFIER only after {} send_command calls (>= the full sweep \\\n             2+{}); the sweep did NOT release the lock between iterations\",\n            calls_when_acquired,\n            n_callbacks\n        );\n        // Corroborating timing bound: well under the ~800ms a full-sweep hold would still need\n        // from the contention point. Generous to tolerate CI scheduling jitter (per-iter ≈\n        // 100–250ms; full-sweep ≈ 800ms).\n        assert!(\n            waited < Duration::from_millis(500),\n            \"contender blocked {:?} for NOTIFIER; expected to slip in between sweep iterations \\\n             (per-iteration release)\",\n            waited\n        );\n\n        // CALLBACK_NAMES is published atomically AFTER the sweep: all N callbacks present once\n        // perform_handshake returns, and the board is host-capable.\n        assert!(host_capable());\n        let names = callback_names();\n        assert_eq!(\n            names.len(),\n            n_callbacks as usize,\n            \"all {} callbacks must be mapped after the sweep completes\",\n            n_callbacks\n        );\n        for i in 0..n_callbacks {\n            let key = format!(\"cb_{}\", i);\n            assert_eq!(\n                names.get(&key),\n                Some(&i),\n                \"callback {} missing/wrong in CALLBACK_NAMES\",\n                key\n            );\n        }\n\n        // Clean up the delay so it can't bleed into later single-threaded tests.\n        MockNotifier::set_send_delay(None);\n    }\n"
  - FOLLOW pattern: test_handshake_capable_populates_state (:1777) — `reset_test_state();
    reset_handshake_state(); set_notifier(Box::new(MockNotifier::new()));` preamble; responses in
    call order; assertions via host_capable() / callback_names() / get_send_command_calls().
  - COVERAGE: (a) lock is acquirable mid-sweep (count assertion — the deterministic backbone);
    (b) it's acquired quickly vs a full-sweep hold (timing assertion — corroborating); (c) the
    atomic post-sweep CALLBACK_NAMES publication still yields all N entries; (d) host_capable() true.

Task 7: VALIDATE (no edits)
  - cargo build --bin qmkonnect
      # Compiles. The borrow-checker passes because the outer `n` is dropped pre-loop, the per-iter
      # `n` is loop-scoped, and no `n` is referenced after the loop.
  - cargo test --bin qmkonnect -- --test-threads=1
      # Full suite green; count 350 -> 351. --test-threads=1 is REQUIRED (AGENTS.md).
  - git diff --stat     # exactly ONE file: src/core/notifier.rs.
  - grep -n 'release the notifier before the read-only rules validation' src/core/notifier.rs
      # Expected: NO matches (the old post-loop drop comment is gone).
  - grep -n 'drop(n);' src/core/notifier.rs
      # Expected: the pre-loop release (Task 1), the per-iteration release inside the loop (Task 4),
      # and the 3 unchanged early-drops in the Timeout/non-capable/Err outer-match arms. NONE after
      # the sweep loop.

Task 8: NEVER do these (out of scope / forbidden)
  - DO NOT touch the Timeout / non-capable Ok(other) / Err outer-match arms (they drop(n) early and
    never sweep — leave them byte-identical).
  - DO NOT reorder QueryCallback (still sequential 0..sweep_cap — firmware FIFO + positional indices).
  - DO NOT publish CALLBACK_NAMES per-iteration — keep the atomic post-sweep publish.
  - DO NOT move the BOARD_HAS_RULES / HOST_CAPABLE stores (they stay after the sweep, in that order).
  - DO NOT add any `use` import (OnceLock/thread/Duration/Instant all already present at :7-9).
  - DO NOT change MAX_HOST_CALLBACKS or CALLBACK_SWEEP_DEADLINE (they remain the secondary bound).
  - DO NOT edit Cargo.toml, PRD.md, tasks.json, prd_snapshot.md, or any file other than
    src/core/notifier.rs.
  - DO NOT run tests without --test-threads=1 (AGENTS.md — shared global state; parallel runs flap).
  - DO NOT duplicate P1.M3.T1.S1's work (STARTUP_DEVICE_CONNECTED etc.) — different region, no overlap.
```

### Implementation Patterns & Key Details
```rust
// PATTERN: the restructure turns one long-held guard into N short-held guards. The ONLY arm that
// changes is the Info-capable arm:
//
//   let n = notifier.lock().unwrap();          // :434 — outer lock (QueryInfo + SetOs)
//   match n.send_command(QueryInfo, &filter) {
//       Ok(Info { capable }) => {
//           if opts.set_os { n.send_command(SetOs …); }
//           drop(n);                            // ← NEW: release before the sweep
//           let sweep_start = Instant::now();
//           let sweep_cap = …;
//           let mut local = HashMap::new();
//           for i in 0..sweep_cap {
//               if sweep_start.elapsed() > CALLBACK_SWEEP_DEADLINE { break; }
//               let n = notifier.lock().unwrap();   // ← NEW: fresh per iteration
//               match n.send_command(QueryCallback(i), &filter) { … process into `local` … }
//               drop(n);                            // ← NEW: release before next iteration
//           }
//           // (old `drop(n)` here is REMOVED — outer n was dropped pre-loop)
//           { CALLBACK_NAMES.clear()+extend(local) }   // atomic publish
//           BOARD_HAS_RULES.store(…); HOST_CAPABLE.store(true, …);
//       }
//       Ok(Timeout) => { drop(n); … }      // UNCHANGED
//       Ok(other)   => { drop(n); … }      // UNCHANGED
//       Err(e)      => { drop(n); … }      // UNCHANGED
//   }
//
// WHY release before the cheap setup too: putting `drop(n)` ahead of `let sweep_start` means the
//   lock is free during the MAX_HOST_CALLBACKS warning eprintln and the HashMap allocation, slightly
//   widening the inter-iteration window for free. (Harmless — those ops touch no Notifier state.)
//
// WHY shadow `n` inside the loop (not a new name): it keeps the match body bit-for-bit identical
//   (`n.send_command(QueryCallback(i)…)`), so Task 3 only inserts the `let n = …` line above the
//   unchanged match. The shadow is loop-scoped and never escapes.
//
// ANTI-PATTERN: do NOT hold the outer lock across the sweep "to be safe". That is the exact bug
//   this task fixes. The per-iteration release is mandatory.
//
// ANTI-PATTERN: do NOT `mem::replace` or otherwise keep a long-lived guard. A fresh lock() per
//   iteration is the correct, reviewable shape.
//
// TEST PATTERN: the contending-thread test is deterministic via a COUNT assertion, not just timing.
//   `calls_when_acquired < 2 + n_callbacks` is TRUE only if the contender got the lock mid-sweep;
//   under a full-sweep hold the contender could only ever see `calls_when_acquired == 2 + n_callbacks`
//   (the sweep fully done). The spin-wait for >=3 calls guarantees the contender starts contending
//   DURING the sweep (not before it begins), removing the "acquired-before-handshake" false-pass.
```

### Integration Points
```yaml
IMPORTS:
  - NONE. OnceLock (:7), thread (:8), Duration/Instant (:9) all already imported.
DEPENDENCIES:
  - get_notifier() (notifier.rs:829) — unchanged; called once for the outer lock and re-called per
    iteration via the same `notifier` Arc clone (the function-level `let notifier = get_notifier();`
    at :433 is still in scope inside the loop).
  - CALLBACK_NAMES (notifier.rs:253) — unchanged; published atomically after the sweep.
  - HOST_CAPABLE / BOARD_HAS_RULES (notifier.rs:247 / :990) — unchanged; stored after the sweep.
CARGO: none. No Cargo.toml change. std only (thread::sleep, Mutex, Duration, Instant).
PARALLEL / SIBLING (no overlap, clean merges):
  - P1.M3.T1.S1 (Complete): added STARTUP_DEVICE_CONNECTED + 2 pub fns near :228/:264 + runner seeds.
    Does NOT touch perform_handshake_with. Different region of notifier.rs. No merge conflict.
PLATFORM VALIDATION:
  - src/core/notifier.rs is compiled on EVERY OS (it is core, not #[cfg]-gated). So `cargo build` +
    `cargo test --bin qmkonnect -- --test-threads=1` on the Linux dev box fully validates this change
    (production restructure + test infra + the new test). No deferred-to-target-OS caveats — unlike
    the tray/runner edits in sibling tasks, this entire change lives in cross-platform core code.
```

## Validation Loop

> Toolchain: Rust (`cargo`). No ruff/mypy. `cargo build` + `cargo test` are the gates.
> Tests MUST run single-threaded (AGENTS.md — shared global debouncer/mock state).

### Level 1: Syntax & Style (Immediate Feedback)
```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles with zero new warnings/errors. The borrow-checker is the main risk surface:
#   if you see "borrow of moved value: `n`" or "use of moved value: `n`", the post-loop `drop(n)`
#   (Task 4) was not removed, or the per-iteration `n` shadows incorrectly — re-check Task 4.
# If "cannot find function `set_send_delay`" → Task 5c setter not added before the test references it.
```

### Level 2: Unit Test (the new test in isolation)
```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect tests::test_handshake_sweep_releases_lock_between_iterations -- --test-threads=1
# Expected: passes. If it panics "handshake never entered the sweep" → the worker thread was starved
#   (rerun; on a healthy box it always enters within ~300ms). If the count assertion fails with
#   ">= the full sweep 2+10" → the per-iteration release (Tasks 3+4) is not in place. If the timing
#   assertion fails (>500ms) → the lock is still held across iterations (re-check Tasks 1+3+4).
```

### Level 3: Full Suite (Regression — AGENTS.md mandates single-threaded)
```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL tests pass. Count rises 350 -> 351. 0 failed, 0 ignored.
#   Pay special attention to the pre-existing handshake tests:
#     test_handshake_capable_populates_state, test_handshake_legacy_proto_v1_string_only,
#     test_handshake_no_feature_flag_string_only, test_handshake_timeout_string_only,
#     test_handshake_timeout_releases_dedup_token, test_handshake_device_error_releases_dedup_token,
#     test_handshake_legacy_reply_keeps_dedup_token, test_handshake_dedup_idempotent,
#     test_handshake_reset_allows_rerun.
#   They must still pass byte-for-byte (the restructure preserves the QueryInfo/SetOs/sweep order,
#   the dedup token semantics, and the atomic CALLBACK_NAMES publish).
```

### Level 4: Manual device-lifecycle exercise (per AGENTS.md dev loops)
```bash
# The fix is exercised via the platform dev loops (no HID unit harness for the real device). On each
# OS, (re)connect a capable QMK board and confirm the verbose handshake log still sweeps all
# callbacks (CALLBACK_NAMES fully populated) AND that window notifications are NOT visibly stalled
# during the sweep:
#   Linux:   cargo build --bin qmkonnect; run with -v; plug a QMK board; switch windows rapidly
#            during the handshake sweep; confirm the window updates land (previously they'd batch up
#            for up to ~5s on a slow board). Confirm "perform_handshake: complete — capable (N
#            callbacks mapped)" still prints with the full count.
#   macOS:   AGENTS.md macOS loop — packaging/macos clean+build+install; open QMKonnect.app; same
#            plug+sweep+switch sequence.
#   Windows: AGENTS.md Windows loop — cargo build --release; taskkill; .\target\release\qmkonnect.exe;
#            same sequence.
# Expected (all OSes): the handshake still maps all callbacks (the menu/status reflects host-capable)
#   and window-focus updates flow during the sweep instead of stalling. (The unit test pins the
#   per-iteration release deterministically; this is the end-to-end smoke check.)
```

### Level 5: Scope/Build Hygiene
```bash
cd /home/dustin/projects/qmkonnect
git diff --stat                         # Expected: exactly ONE file — src/core/notifier.rs.
git diff Cargo.toml                     # Expected: empty.
grep -n 'release the notifier before the read-only rules validation' src/core/notifier.rs
                                        # Expected: ZERO matches (old post-loop drop comment removed).
grep -n 'MOCK_SEND_DELAY' src/core/notifier.rs
                                        # Expected: 1 static decl + 1 reset (reset_global_mock) +
                                        #   1 setter (set_send_delay) + 1 read (send_command sleep) +
                                        #   2 in the test (set Some / set None) = 6 references.
grep -n 'drop(n);' src/core/notifier.rs
                                        # Expected: pre-loop release + per-iteration release + 3
                                        #   unchanged early-drops (Timeout/non-capable/Err arms).
                                        #   NONE after the sweep loop.
grep -n 'let n = notifier.lock().unwrap();' src/core/notifier.rs
                                        # Expected: 2 — the outer lock (:434) AND the per-iteration
                                        #   lock inside the loop (Task 3).
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` succeeds with no new warnings (borrow-checker is the risk surface).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` → full suite green, count **350 → 351**.
- [ ] `git diff --stat` shows exactly **one** file (`src/core/notifier.rs`); `git diff Cargo.toml` is empty.

### Feature Validation
- [ ] Info-capable arm drops the outer `n` **before** the sweep setup (Task 1 `drop(n);` present before `let sweep_start`).
- [ ] The sweep loop acquires a fresh `let n = notifier.lock().unwrap();` per iteration (Task 3) and drops it at the end of each iteration (Task 4 `drop(n); // release NOTIFIER before the next iteration`).
- [ ] The old post-loop `drop(n); // release the notifier before the read-only rules validation` is **gone** (grep returns nothing).
- [ ] The `Timeout` / non-capable `Ok(other)` / `Err` outer-match arms are byte-identical (they still `drop(n)` early and never sweep).
- [ ] `QueryCallback` is still sent sequentially `0..sweep_cap` (no reordering).
- [ ] `CALLBACK_NAMES` is still published atomically after the sweep; `BOARD_HAS_RULES` is stored before `HOST_CAPABLE` after the sweep (the #5 ordering preserved).
- [ ] New test `test_handshake_sweep_releases_lock_between_iterations` passes; its count assertion (`calls_when_acquired < 2 + n_callbacks`) and timing assertion (`waited < 500ms`) both hold; `CALLBACK_NAMES` has all N entries after `h.join()`; `host_capable()` is true.
- [ ] All pre-existing handshake tests still pass (no behavioral regression).

### Code Quality Validation
- [ ] No new imports added (OnceLock/thread/Duration/Instant already present at `:7-9`).
- [ ] No new dependencies; Cargo.toml untouched.
- [ ] The `#4` comment is reworded to reflect per-iteration release as the primary mitigation, with cap+deadline as secondary bound.
- [ ] `MOCK_SEND_DELAY` is reset in `reset_global_mock()` AND cleared to `None` at the end of the new test (no bleed into other single-threaded tests).
- [ ] The restructure follows the existing code style (doc comments, indentation, `perform_handshake_with`'s structure).

### Documentation & Deployment
- [ ] No user-facing / config / API surface change (internal concurrency improvement — DOCS: none per contract).
- [ ] No new env vars / config keys / CLI flags.
- [ ] The entire change lives in cross-platform core code (`src/core/notifier.rs`) → fully validated on the Linux dev box; no deferred-to-target-OS caveats.

---

## Anti-Patterns to Avoid
- ❌ Don't hold the outer `n` guard across the sweep "for safety" — that is the exact bug (Finding #4) this task fixes. The per-iteration release is mandatory.
- ❌ Don't reorder `QueryCallback` (parallelize, shuffle, etc.) — firmware processes commands FIFO and callback indices are positional; keep sequential `0..sweep_cap`.
- ❌ Don't publish `CALLBACK_NAMES` per-iteration — a concurrent send must see EITHER the old map OR the fully-built new map, never a partial one. Keep the atomic post-sweep publish (`local` → `CALLBACK_NAMES`).
- ❌ Don't move the `BOARD_HAS_RULES` / `HOST_CAPABLE` stores into the loop — they stay after the sweep, with `BOARD_HAS_RULES` before `HOST_CAPABLE` (the #5 ordering).
- ❌ Don't touch the `Timeout` / non-capable `Ok(other)` / `Err` outer-match arms — they `drop(n)` early and never sweep; leave them byte-identical.
- ❌ Don't leave the old post-loop `drop(n); // release the notifier before the read-only rules validation` in place — the outer `n` is dropped before the loop now; that line would be a use-after-move compile error. Remove it (Task 4).
- ❌ Don't add any `use` import — `OnceLock`/`thread`/`Duration`/`Instant` are all already imported (`:7-9`), and the test mod globs them via `use super::*`.
- ❌ Don't change `MAX_HOST_CALLBACKS` (64) or `CALLBACK_SWEEP_DEADLINE` (5s) — they remain the secondary bound (defense-in-depth).
- ❌ Don't run tests without `--test-threads=1` (AGENTS.md — shared global debouncer/mock state; parallel runs flap).
- ❌ Don't forget to reset `MOCK_SEND_DELAY` to `None` (in `reset_global_mock()` and at the end of the new test) — a leftover delay would slow every subsequent single-threaded test's `send_command`.
- ❌ Don't duplicate P1.M3.T1.S1's work (`STARTUP_DEVICE_CONNECTED` etc.) — different region of the same file, no overlap.
- ❌ Don't edit Cargo.toml, PRD.md, tasks.json, prd_snapshot.md, or any file other than `src/core/notifier.rs`.

---

## Confidence Score: 9/10

The change is small and surgical: one match arm's lock scope is restructured (6 in-file edits), plus
4 small test-infra additions and 1 new test — all in `src/core/notifier.rs`, all in cross-platform
core code (no deferred-to-target-OS validation). Every anchor is grep-confirmed current this session
(the `#4` comment at the sweep setup, the loop-top deadline check, the `match n.send_command(QueryCallback(i))`
line, the loop-close + old post-loop `drop(n)` comment, the `MOCK_SEND_COMMAND_ERRORS` static +
`reset_global_mock` + `set_mock_send_errors` + `send_command` head in the test mod, and the
`test_handshake_capable_populates_state` template at `:1777`). The borrow-checker reasoning is
verified: the outer `n` is dropped pre-loop (moved), the per-iteration `n` shadows it in a
loop-local scope, and nothing after the loop references `n`. Baseline `cargo test --bin qmkonnect --
--test-threads=1` is **350 passed** this session → expect **351**. The new test is deterministic
(the `calls_when_acquired < 2 + n_callbacks` count assertion is TRUE only under per-iteration
release and impossible under a full-sweep hold), with a corroborating generous timing bound and a
spin-wait that guarantees the contender contends DURING the sweep. The 1-point reservation: the
timing assertion (`waited < 500ms`) is inherently scheduler-dependent; on a pathologically loaded CI
box the per-iteration acquisition could in principle exceed 500ms. If that ever flakes, widen the
threshold (the count assertion is the real gate and is fully deterministic). The production
correctness rests on the per-iteration release + the unchanged atomic post-sweep publish + the
unchanged `QueryCallback` ordering, all pinned by the test.
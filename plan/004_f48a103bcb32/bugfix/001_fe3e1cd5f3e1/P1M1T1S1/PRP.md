# PRP — P1.M1.T1.S1: Add STATE/COND poison-recovery to debounce_worker and notify_qmk

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). ALL edits in ONE file:
> `src/core/notifier.rs`.
> **Scope:** Defense-in-depth ONLY (debug/test builds). The critical debounce
> panic is **already fixed** (notifier.rs:863 `match state.pending.take()`); this
> subtask adds mutex/condvar **poison recovery** to the 4 production lock/wait
> sites so a poisoned `STATE` no longer cascades into a panic. Release builds use
> `panic = "abort"` (poisoning is impossible there), so this is purely assurance
> for `panic = "unwind"` mode.
> **⚠ The contract's literal test description ("poison the global STATE") is NOT
> viable** — see the prominent note in *What (c)* and the Gotchas. The PRP
> substitutes a faithful, order-independent local-mutex test and explains why.

---

## Goal

**Feature Goal**: Harden the four production mutex/condvar operations in
`debounce_worker()` and `notify_qmk()` (`src/core/notifier.rs`) to recover from a
poisoned `STATE` mutex instead of panicking, and add a regression test proving the
recovery idiom works. The change is **purely the error-recovery path** — no
debouncing logic, timing, or message ordering is touched.

**Deliverable**: `src/core/notifier.rs` with four `.unwrap()` →
`.unwrap_or_else(|e| e.into_inner())` edits (lines 841, 845, 871, 927) plus one
new test `test_debounce_worker_survives_poisoned_state` that verifies the recovery
idiom on a poisoned `Mutex<DebounceState>` and confirms `notify_qmk` still
functions on the (unpoisoned) global `STATE`.

**Success Definition**: `grep -nE 'STATE\.lock\(\)\.unwrap\(\)|COND\.wait.*\.unwrap\(\)'`
on the **production** portion of `notifier.rs` returns **zero** matches for the 4
sites; the new test passes; the full suite is **345 tests** (344 existing + 1 new)
passing under `cargo test --bin qmkonnect -- --test-threads=1`; no debouncing
behavior, timing, or send ordering changes (existing debounce tests still pass).

## User Persona (if applicable)

**Target User**: The QMKonnect daemon itself (reliability hardening) and the
maintainer who may one day flip `panic` to `"unwind"` (or run under a tool that
unwinds). End users see no change.

**Use Case**: Long-running menu-bar/tray daemon. Today a logic panic elsewhere
holding `STATE` would poison it and cascade into every subsequent `notify_qmk`
(also panicking). After this subtask, `notify_qmk`/`debounce_worker` keep serving
windows through a poisoned `STATE`.

**Pain Points Addressed**: Eliminates the cascade-failure mode in debug/test
builds and future-proofs against any `unwind` configuration. (Release already
`abort`s on panic, so poisoning is impossible there — this is belt-and-suspenders.)

## Why

- **The critical race is already fixed** (notifier.rs:863). This subtask closes
  the *secondary* failure mode the bug report flagged: even after the
  `.take().unwrap()` → `match` fix, the surrounding `STATE.lock().unwrap()` /
  `COND.wait(state).unwrap()` / `COND.wait_timeout(...).unwrap().0` calls would
  still panic if `STATE` were ever poisoned by *some other* panic while the lock
  is held — cascading into every later `notify_qmk`. `PoisonError::into_inner()`
  recovers the usable guard, ending the cascade.
- **It is the cheapest possible hardening.** Four mechanical, type-preserving
  edits (`.unwrap()` → `.unwrap_or_else(|e| e.into_inner())`); no logic, no new
  deps, no API surface change. `PoisonError::into_inner()` returns the exact same
  guard type the `Ok` arm would, so every downstream use compiles unchanged.
- **It preserves the "never take down the service" NFR** (PRD §2.1 goal 4 /
  §10) for the unwind configuration, matching the existing graceful-degradation
  philosophy (retry on unplug, app-name-only on missing permission, etc.).

## What

### (a) The four hardening edits (EXACT before → after)

All four are a literal, mechanical, type-preserving substitution. The closure
`|e| e.into_inner()` consumes the `PoisonError` and returns the *same* guard
(`MutexGuard` for `lock`/`wait`; `(MutexGuard, WaitTimeoutResult)` for
`wait_timeout`, of which `.0` takes the guard — identical to the current code).

```rust
// Line 841 — debounce_worker() outer loop:
-            let mut state = STATE.lock().unwrap();
+            let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());

// Line 845 — debounce_worker() "wait until something is queued":
-                state = COND.wait(state).unwrap();
+                state = COND.wait(state).unwrap_or_else(|e| e.into_inner());

// Line 871 — debounce_worker() inner timed wait:
-                    state = COND.wait_timeout(state, target - now).unwrap().0;
+                    state = COND.wait_timeout(state, target - now).unwrap_or_else(|e| e.into_inner()).0;

// Line 927 — notify_qmk():
-        let mut state = STATE.lock().unwrap();
+        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
```

> **No other change.** Line 946 (`COND.notify_one()`) has no `Result`/`.unwrap()`
> — leave it. Do NOT touch `state.pending.take()` (the already-applied `match`),
> the timing math (`target - now`, `interval()`), the send path, or any other
> line. The edits are surgical.

### (b) Type-correctness confirmation (why `.into_inner()` works for all three shapes)

```text
STATE.lock()              -> Result<MutexGuard<'_, DebounceState>,
                                      PoisonError<MutexGuard<'_, DebounceState>>>
   .unwrap_or_else(|e| e.into_inner())  -> MutexGuard<'_, DebounceState>          ✓

COND.wait(state)          -> Result<MutexGuard<'_, DebounceState>,
                                      PoisonError<MutexGuard<'_, DebounceState>>>
   .unwrap_or_else(|e| e.into_inner())  -> MutexGuard<'_, DebounceState>          ✓
   (the rebound `state =` keeps the same type)

COND.wait_timeout(state, dur) -> Result<(MutexGuard, WaitTimeoutResult),
                                        PoisonError<(MutexGuard, WaitTimeoutResult)>>
   .unwrap_or_else(|e| e.into_inner())  -> (MutexGuard, WaitTimeoutResult)        ✓
   .0                                   -> MutexGuard                              ✓
```

`PoisonError::into_inner(self) -> T` is a stable std API; `T` is whatever the
`Ok` variant carries. So each edit yields the identical type the `.unwrap()` did —
no downstream code changes.

### (c) ⚠ The new test — local-mutex approach (the contract's "poison STATE" is not viable)

**Why NOT poison the global `STATE`:** `STATE` is a `static Lazy<Mutex<DebounceState>>`
shared by the entire test binary. `std::sync::Mutex` poison **cannot be cleared on
stable Rust** (no `clear_poison`). The helper `reset_test_state()` (line 1292) —
called at the top of nearly every notifier test — does `STATE.lock().unwrap()`
(line 1297), and the existing regression test asserts `STATE.lock().is_ok()`
(line 2438). Poisoning `STATE` in one test would therefore **permanently** break
every test that runs afterward (the `--test-threads=1` harness serializes tests
but in an order that is **not guaranteed**), making "345 tests pass" impossible.

**The faithful equivalent:** the recovery idiom
(`unwrap_or_else(|e| e.into_inner())`) is **generic over `Mutex<T>`** — it behaves
identically on a local `Mutex<DebounceState>` as on the global `STATE`. So the
test poisons a *local* mutex (proving the idiom recovers), then calls `notify_qmk`
on the (unpoisoned) global `STATE` (proving the 4 edits didn't regress the normal
path, using the existing `MockNotifier` infrastructure).

```rust
/// Defense-in-depth: debounce_worker / notify_qmk recover from a poisoned STATE
/// mutex (debug/test builds only). Release uses `panic = "abort"`, so a panic
/// kills the process before any re-lock and poisoning is impossible there.
///
/// We verify the recovery idiom on a LOCAL `Mutex<DebounceState>` rather than
/// the global `STATE`: std `Mutex` poison cannot be cleared on stable Rust, so
/// poisoning the global `STATE` would permanently contaminate it and break
/// `reset_test_state()` (STATE.lock().unwrap()) plus the `STATE.lock().is_ok()`
/// assertion in `test_debounce_worker_survives_pending_cleared_mid_wait` for
/// every test run afterward. The idiom `unwrap_or_else(|e| e.into_inner())` is
/// generic over Mutex<T>, so a local mutex is a faithful proof that the four
/// hardened production sites (STATE.lock @ worker+notify_qmk, COND.wait,
/// COND.wait_timeout) recover identically. We additionally call notify_qmk on
/// the (unpoisoned) global STATE to confirm the hardening didn't regress the
/// normal path.
#[test]
fn test_debounce_worker_survives_poisoned_state() {
    // --- Part A: the recovery idiom survives a poisoned Mutex<DebounceState>. ---
    let local: Mutex<DebounceState> = Mutex::new(DebounceState {
        last_sent_time: None,
        pending: None,
        verbose: false,
        interval_override: Some(Duration::from_millis(50)),
    });

    // Poison it: lock, then panic under catch_unwind. The guard is dropped during
    // unwinding, which sets the mutex's poison flag (permanent on stable Rust).
    let panic_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = local.lock().unwrap();
        panic!("intentional: poison the mutex");
    }));
    assert!(panic_res.is_err(), "helper must panic to set the poison flag");
    assert!(local.lock().is_err(), "mutex must be poisoned after the panic");

    // Recovery — the EXACT idiom the four hardened production sites use:
    // PoisonError::into_inner() returns the inner guard, usable despite poison.
    {
        let mut guard = local.lock().unwrap_or_else(|e| e.into_inner());
        guard.interval_override = Some(Duration::from_millis(10)); // prove it's usable
        assert_eq!(guard.interval_override, Some(Duration::from_millis(10)));
    }
    // (COND.wait / COND.wait_timeout share the same PoisonError::into_inner shape:
    //  wait         -> PoisonError<MutexGuard>.into_inner()              -> MutexGuard
    //  wait_timeout -> PoisonError<(MutexGuard, WaitTimeoutResult)>.into_inner().0 -> MutexGuard)

    // --- Part B: notify_qmk still works on the (unpoisoned) global STATE ---
    // (confirms the 4-site hardening didn't regress the normal/unpoisoned path).
    reset_test_state();
    set_notifier(Box::new(MockNotifier::new()));
    let res = notify_qmk(&WindowInfo::new("PoisonChk".into(), "t".into()), false);
    assert!(res.is_ok());
    assert!(
        wait_for_count(1, Duration::from_millis(500)),
        "notify_qmk must still flush on an unpoisoned STATE after hardening"
    );
}
```

> **Placement:** inside the existing `#[cfg(test)] mod tests` block, immediately
> after `test_debounce_worker_survives_pending_cleared_mid_wait` (line ~2459). It
> uses the already-imported `use super::*;`, plus `std::panic::catch_unwind` /
> `AssertUnwindSafe` (use the fully-qualified path as shown — no new `use` needed).
> `DebounceState`, `MockNotifier`, `reset_test_state`, `set_notifier`,
> `wait_for_count`, `WindowInfo`, `Mutex`, `Duration` are all already in scope.

### Success Criteria

- [ ] Lines 841, 845, 871, 927 use `.unwrap_or_else(|e| e.into_inner())` (the
      `.0` on 871 preserved); no other production line changed.
- [ ] `test_debounce_worker_survives_poisoned_state` exists, uses a **local**
      `Mutex<DebounceState>`, poisons it via `catch_unwind`, asserts recovery, and
      additionally verifies `notify_qmk` flushes on the unpoisoned global `STATE`.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` → **345 passed; 0 failed**.
- [ ] No debouncing logic/timing/ordering change (the existing debounce tests —
      `test_immediate_send_first_message`, `test_debounce_subsequent_messages`,
      `test_debounce_worker_survives_pending_cleared_mid_wait`, etc. — still pass).
- [ ] No file other than `src/core/notifier.rs` is modified.

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed
> to implement this successfully?"_ — **Yes.** The four exact before→after edits,
> the type-correctness table for all three PoisonError shapes, the full
> ready-to-paste test (with the contamination rationale), the precise grep-based
> validation, and the verified `cargo test` command are all below.

### Documentation & References

```yaml
# MUST READ — the bug-hunt finding (the critical fix is already applied; this subtask is the secondary hardening)
- file: /home/dustin/projects/qmkonnect/plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/architecture/system_context.md
  why: "§1 documents the critical debounce race fix (notifier.rs:863) and the KEY CONSTRAINT: `panic = \"abort\"`
        in [profile.release] means poisoning is impossible in release — this subtask is debug/test-only
        defense-in-depth. §3 confirms the test protocol: cargo test --bin qmkonnect -- --test-threads=1,
        344 tests currently pass."
  section: "1. Critical Debounce Fix (h2.0) — VERIFIED APPLIED" and "3. Architecture Constraints"
  critical: "The critical panic is ALREADY FIXED (don't re-fix it). This subtask hardens the 4 surrounding
             lock/wait sites ONLY. Do NOT change the .take() match at line 863."

# MUST READ — the file being edited (confirm exact current code before editing)
- file: /home/dustin/projects/qmkonnect/src/core/notifier.rs
  why: "Contains STATE (Lazy<Mutex<DebounceState>> @818), COND (Lazy<Condvar> @827), debounce_worker (832-896,
        with the 4 harden sites at 841/845/871), notify_qmk (919-967, harden site at 927, notify_one at 946),
        the test module (MockNotifier @1234, reset_test_state @1292, wait_for_count @1312), and the existing
        regression test test_debounce_worker_survives_pending_cleared_mid_wait @2407 (whose is_ok() assertion
        at 2438 is WHY you must NOT poison the global STATE)."
  pattern: "Lock-acquire style: the codebase uses .lock().unwrap() everywhere (NOT poisoning-aware). The 4
            production sites are the ONLY ones whose recovery is in scope for this subtask."
  gotcha: "There are ~13 total STATE.lock().unwrap() sites; only 5 are production (841, 845, 871, 927, 946).
           The rest (1297, 1353, 1389, 1453, 1526, 1538, 1553, 2412, 2426, 2438, 2445) live in TEST helpers/tests
           and must NOT be hardened (out of scope) — which is exactly why the new test poisons a LOCAL mutex,
           not the global STATE. Poisoning STATE would make every test-site .unwrap() panic."

# REFERENCE — std Mutex poison semantics (why into_inner recovers, why poison is permanent)
- url: https://doc.rust-lang.org/std/sync/struct.PoisonError.html
  why: "PoisonError::into_inner(self) -> T returns the lock regardless of the poison state. lock()/wait()/
        wait_timeout() return LockResult<T> = Result<T, PoisonError<T>>; the Err carries the SAME T, so
        unwrap_or_else(|e| e.into_inner()) yields a usable guard."
  critical: "There is NO stable API to clear a std Mutex's poison flag. Once poisoned, a Mutex stays poisoned
             for the life of the process. This is the decisive reason the new test uses a LOCAL mutex."

# REFERENCE — std Condvar::wait_timeout return type (the .0 extraction)
- url: https://doc.rust-lang.org/std/sync/struct.Condvar.html#method.wait_timeout
  why: "wait_timeout returns LockResult<(MutexGuard, WaitTimeoutResult)>. The PoisonError's into_inner()
        yields the (guard, result) tuple; .0 then takes the guard — matching the existing .unwrap().0 shape."
  section: "wait_timeout"

# REFERENCE — research notes for this subtask (contamination analysis + alternative)
- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/P1M1T1.S1/research/notes.md
  why: "Documents the full STATE.lock() site inventory (prod vs test), the global-STATE contamination proof
        (why poisoning it breaks reset_test_state + the is_ok() assertion), and the global-poison alternative
        with its full requirements (harden ~13 sites + reorder + change is_ok()) — confirming the local-mutex
        test is the only order-independent approach."
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/                       # THIS repo
├── Cargo.toml                   # [profile.release] panic = "abort" (line 123) — poisoning impossible in release
└── src/core/
    └── notifier.rs              # <-- FILE TO EDIT (4 prod sites + 1 new test). STATE @818, COND @827.
```

### Desired Codebase tree with files to be modified

```bash
src/core/
└── notifier.rs   # MODIFIED ONLY — 4 .unwrap()→.unwrap_or_else edits (841/845/871/927) + 1 new #[test] fn.
```

> No new files. `Cargo.toml`, all other source files, and the existing tests are
> untouched (the new test is ADDED, not a replacement).

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL: do NOT poison the global STATE in the new test.
//   std Mutex poison cannot be cleared on stable Rust (no clear_poison). STATE is a process-global static.
//   reset_test_state() (line 1292, called by ~every notifier test) does STATE.lock().unwrap() at 1297; the
//   regression test asserts STATE.lock().is_ok() at 2438. Poisoning STATE would permanently break whichever
//   of those runs afterward — and --test-threads=1 does NOT guarantee run order. The new test poisons a
//   LOCAL Mutex<DebounceState> instead (the recovery idiom is generic over Mutex<T>). See What (c).

// CRITICAL: edit ONLY the 4 production sites (841, 845, 871, 927). NOT the test helpers.
//   There are ~13 STATE.lock().unwrap() sites total; 5 are production (the 4 here + notify_one at 946 which
//   has no unwrap). The rest (1297, 1353, 1389, 1453, 1526, 1538, 1553, 2412, 2426, 2445) are test code —
//   out of scope. Hardening them is scope creep the contract does not authorize, and is unnecessary once
//   the new test avoids poisoning STATE.

// CRITICAL: do NOT re-fix the critical panic at line 863.
//   state.pending.take() already uses `match { Some(pm) => pm, None => break }`. Leave it. This subtask is
//   ONLY the 4 surrounding lock/wait poison-recovery edits.

// CRITICAL: do NOT change ANY debouncing logic, timing, or send ordering.
//   The edits are .unwrap()→.unwrap_or_else(|e| e.into_inner()) on mutex/condvar RESULTS only. Do not touch
//   target - now, interval(), the .take() match, last_sent_time, COND.notify_one/notify_all, or the send path.

// NOTE: PoisonError::into_inner() returns the SAME guard type as Ok — type-preserving.
//   So `let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());` keeps `state: MutexGuard<...>`,
//   and `COND.wait_timeout(...).unwrap_or_else(|e| e.into_inner()).0` still yields the guard. No downstream
//   code changes. Verified in What (b).

// NOTE: catch_unwind requires AssertUnwindSafe because the closure captures the local Mutex by reference and
//   Mutex is not UnwindSafe. Use std::panic::AssertUnwindSafe(|| { ... }) as shown in What (c). This is the
//   standard idiom for deliberately poisoning a mutex in a test.

// NOTE: tests are single-threaded (cargo test --bin qmkonnect -- --test-threads=1) per AGENTS.md — the
//   debounce worker shares global STATE across tests. The local-mutex test does not disturb that global,
//   so it composes safely with the rest of the suite in any order.

// NOTE: release builds abort on panic (Cargo.toml panic="abort"); poisoning literally cannot happen there
//   (the process is dead before any re-lock). The hardening is inert in release — it only matters in
//   debug/test (unwind). Do not add #[cfg(...)] gates; the edits are free at runtime and keep both profiles
//   behaving identically.
```

## Implementation Blueprint

### Data models and structure

No data-model change. `DebounceState` (786), `STATE` (818), `COND` (827),
`PendingMessage`, `HostContext`, etc. are all untouched. This subtask changes four
expressions and adds one test.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CONFIRM the exact current code at the 4 sites
  - READ: src/core/notifier.rs lines 838-875 (debounce_worker: 841 lock, 845 wait, 871 wait_timeout)
          and 924-948 (notify_qmk: 927 lock, 946 notify_one).
  - CONFIRM: each is exactly the `.unwrap()` / `.unwrap().0` form shown in What (a). (The applied
          critical fix at 863 may have shifted surrounding line numbers slightly — match on the TEXT,
          not just the line number: `STATE.lock().unwrap()`, `COND.wait(state).unwrap()`,
          `COND.wait_timeout(state, target - now).unwrap().0`, and the notify_qmk `STATE.lock().unwrap()`.)
  - CONFIRM line 946 is `COND.notify_one();` (no unwrap — leave it).
  - GOAL: anchor the 4 exact edits so they cannot miss.

Task 2: APPLY the 4 hardening edits
  - EDIT line 841:  STATE.lock().unwrap()  ->  STATE.lock().unwrap_or_else(|e| e.into_inner())
  - EDIT line 845:  COND.wait(state).unwrap()  ->  COND.wait(state).unwrap_or_else(|e| e.into_inner())
  - EDIT line 871:  COND.wait_timeout(state, target - now).unwrap().0
                    -> COND.wait_timeout(state, target - now).unwrap_or_else(|e| e.into_inner()).0
  - EDIT line 927:  STATE.lock().unwrap()  ->  STATE.lock().unwrap_or_else(|e| e.into_inner())
  - DO NOT: touch line 863 (.take() match), 946 (notify_one), timing, sends, or anything else.
  - DO NOT: edit any test-helper STATE.lock().unwrap() (lines 1297/1353/1389/1453/1526/1538/1553/2412/
          2426/2445) — they are out of scope and must stay .unwrap() (they rely on STATE being unpoisoned,
          which the local-mutex test preserves).

Task 3: ADD the new test (local-mutex approach)
  - INSERT: test_debounce_worker_survives_poisoned_state (What (c)) inside #[cfg(test)] mod tests,
          immediately after test_debounce_worker_survives_pending_cleared_mid_wait (line ~2459).
  - USE: a LOCAL Mutex<DebounceState> for the poison+recover proof (NOT the global STATE).
  - USE: std::panic::catch_unwind + AssertUnwindSafe to poison; .lock().unwrap_or_else(|e| e.into_inner())
          to recover (the exact production idiom).
  - USE: the existing MockNotifier + reset_test_state + wait_for_count for the Part B notify_qmk check
          (confirms the unpoisoned-normal path still flushes after the 4 edits).
  - DO NOT: poison the global STATE (would contaminate the suite — see Gotchas).

Task 4: VALIDATE
  - RUN: cargo test --bin qmkonnect -- --test-threads=1
  - EXPECT: 345 passed; 0 failed (344 existing + 1 new).
  - RUN: grep -nE 'STATE\.lock\(\)\.unwrap\(\)|COND\.wait\(state\)\.unwrap\(\)|COND\.wait_timeout\(state, target - now\)\.unwrap\(\)\.0' src/core/notifier.rs
  - EXPECT: matches ONLY in the test module (line >= ~1230). The 4 production sites (841/845/871/927) must
          NOT appear with .unwrap(). (See Validation Loop for the precise command.)
```

### Implementation Patterns & Key Details

```rust
// === THE POISON-RECOVERY IDIOM (identical for all 4 sites) ===
// .unwrap()  ->  .unwrap_or_else(|e| e.into_inner())
// PoisonError::into_inner(self) -> T returns the SAME guard the Ok arm carries. Type-preserving, zero
// downstream changes. For wait_timeout, into_inner() yields the (guard, WaitTimeoutResult) tuple, so the
// existing trailing .0 still extracts the guard.

// === WHY LOCAL MUTEX, NOT GLOBAL STATE, IN THE TEST ===
// std Mutex poison is permanent on stable (no clear_poison). STATE is process-global and shared by
// reset_test_state() (STATE.lock().unwrap() @1297) + the is_ok() assertion @2438. Poisoning STATE breaks
// whichever runs next; --test-threads=1 does not guarantee order. The idiom is generic over Mutex<T>, so a
// local Mutex<DebounceState> is a faithful, order-independent proof.

// === WHY catch_unwind + AssertUnwindSafe ===
// To set the poison flag you must panic WHILE holding the lock. catch_unwind stops the unwind at the test
// boundary (so the test process survives). AssertUnwindSafe wraps the closure because it captures the local
// Mutex by ref and Mutex isn't UnwindSafe — the standard deliberate-poison idiom.
```

### Integration Points

```yaml
SOURCE FILES:
  - modify: "src/core/notifier.rs ONLY (4 prod lock/wait sites + 1 new test)"

PUBLIC API SURFACE:
  - unchanged: "notify_qmk signature, debounce_worker, DebounceState, STATE, COND, HostContext, all re-exports"

CARGO / BUILD:
  - none. No new deps. No Cargo.toml change. (panic = \"abort\" stays in [profile.release]; the hardening is
    inert there and active in debug/test unwind mode — no cfg gates needed.)

TEST PROTOCOL:
  - command: "cargo test --bin qmkonnect -- --test-threads=1"
  - expected: "345 passed; 0 failed (single-threaded — shared global debouncer state, per AGENTS.md)"

RELATED (do NOT implement now — out of scope):
  - "Harden the ~9 test-helper STATE.lock().unwrap() sites" — NOT required (the new test avoids poisoning
    STATE, so test helpers stay safe on an unpoisoned mutex). Only revisit if a future test deliberately
    poisons the global STATE (then ALL sites + the is_ok() assertion @2438 must change together).
```

## Validation Loop

> All commands run from the repo root: `/home/dustin/projects/qmkonnect`.

### Level 1: The 4 edits landed and no logic changed (syntax + scope gate)

```bash
cd /home/dustin/projects/qmkonnect

# (a) The 4 production sites are hardened. Build compiles.
cargo build 2>&1 | tee /tmp/build.log
# Expected: "Finished" — zero errors. (unwrap_or_else(|e| e.into_inner()) is type-preserving; if it fails to
# compile, re-check the .0 on the wait_timeout site — into_inner() yields the tuple, THEN .0 takes the guard.)

# (b) Confirm the 4 production .unwrap() forms are GONE (only test-module matches may remain).
grep -nE 'STATE\.lock\(\)\.unwrap\(\)|COND\.wait\(state\)\.unwrap\(\)|wait_timeout\(state, target - now\)\.unwrap\(\)\.0' src/core/notifier.rs
# Expected: every printed line number is >= ~1230 (inside #[cfg(test)] mod tests). NONE at 841/845/871/927.
# (If a production line still prints, that edit was missed — apply it.)

# (c) Confirm the 4 hardened forms ARE present.
grep -nE 'unwrap_or_else\(\|e\| e\.into_inner\(\)\)' src/core/notifier.rs
# Expected: exactly 4 matches at the production sites (841/845/871/927). (No test site should have it — the
# new test calls it on a LOCAL mutex named `local`, so `local.lock().unwrap_or_else(...)` won't match this
# `STATE`/`COND`-anchored grep, but `grep into_inner` will also show the test's local.lock line — that's fine.)

# (d) Lint stays clean (unwrap_or_else with into_inner closure is not a clippy footgun).
cargo clippy --bin qmkonnect 2>&1 | tee /tmp/clippy.log | grep -iE 'warning|error' || echo "clippy clean"
# Expected: no new warnings (the closure is necessary — not replaceable by unwrap_or/unwrap_or_default).
```

### Level 2: The new test + full suite (the real gate)

```bash
cd /home/dustin/projects/qmkonnect

# (a) The new test passes in isolation.
cargo test --bin qmkonnect test_debounce_worker_survives_poisoned_state -- --test-threads=1 --nocapture
# Expected: 1 passed. (Part A: local mutex poisoned + recovered; Part B: notify_qmk flushed -> count 1.)

# (b) The full suite — single-threaded (shared global STATE).
cargo test --bin qmkonnect -- --test-threads=1 2>&1 | tee /tmp/test.log | tail -5
# Expected: "test result: ok. 345 passed; 0 failed; 0 ignored; ...".
# (344 pre-existing + 1 new.) If a DIFFERENT test fails after the new test runs, it means STATE got poisoned
# globally — re-check that the new test uses the LOCAL mutex (not STATE); see Gotchas.

# (c) The existing debounce tests still pass (no behavior/timing/ordering regression).
cargo test --bin qmkonnect test_debounce -- --test-threads=1 --nocapture
cargo test --bin qmkonnect test_immediate_send_first_message -- --test-threads=1 --nocapture
cargo test --bin qmkonnect test_debounce_worker_survives_pending_cleared_mid_wait -- --test-threads=1 --nocapture
# Expected: all pass. (Confirms the 4 edits changed only the error-recovery path, not debounce semantics.)
```

### Level 3: Integration / runtime (sanity — no live-HID change)

```text
NOT REQUIRED for this subtask. The hardening is inert on the happy path (an unpoisoned STATE: lock() returns
Ok, unwrap_or_else never calls its closure). No wire/HID behavior changes. The Level-2 suite IS the proof that
notify_qmk + debounce_worker still flush correctly. (Live-device testing is the AGENTS.md dev loop, not needed
for a mutex-recovery edit.)
```

### Level 4: Defense-in-depth reasoning check (manual)

```text
Confirm the hardening actually closes the cascade (read-only reasoning):

1. Suppose some OTHER panic occurs while STATE is held (e.g. a future bug). The mutex becomes poisoned.
2. Before this subtask: the next notify_qmk did STATE.lock().unwrap() -> PANIC -> cascade. Now it does
   STATE.lock().unwrap_or_else(|e| e.into_inner()) -> returns the (poisoned but usable) guard -> notify_qmk
   proceeds normally. Cascade broken. ☐
3. Same for debounce_worker's 3 sites (841 lock, 845 wait, 871 wait_timeout). ☐
4. Release builds abort on panic before any re-lock, so this path is only reached in debug/test. ☐ (matches
   the architecture doc §1 "defense-in-depth for future unwind mode only".)
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1 (a): `cargo build` compiles (the 4 edits are type-preserving).
- [ ] Level 1 (b): grep shows the 4 production `.unwrap()` forms GONE (only test-module lines remain).
- [ ] Level 1 (c): grep shows exactly 4 `unwrap_or_else(|e| e.into_inner())` at 841/845/871/927.
- [ ] Level 1 (d): `cargo clippy` introduces no new warnings.
- [ ] Level 2 (a): `test_debounce_worker_survives_poisoned_state` passes.
- [ ] Level 2 (b): full suite `345 passed; 0 failed` (`--test-threads=1`).
- [ ] Level 2 (c): existing debounce tests still pass (no regression).

### Feature Validation

- [ ] Lines 841/845/871/927 use `.unwrap_or_else(|e| e.into_inner())` (`.0` preserved on 871).
- [ ] The new test poisons a LOCAL `Mutex<DebounceState>` (NOT the global `STATE`).
- [ ] The new test proves recovery AND verifies `notify_qmk` flushes on the unpoisoned global `STATE`.
- [ ] Line 863 (`.take()` match), 946 (`notify_one`), timing, and sends are unchanged.
- [ ] Only `src/core/notifier.rs` modified.

### Code Quality Validation

- [ ] The 4 edits follow the idiomatic Rust poison-recovery pattern (`unwrap_or_else(|e| e.into_inner())`).
- [ ] The test uses `catch_unwind` + `AssertUnwindSafe` (the standard deliberate-poison idiom).
- [ ] No `#[cfg(...)]` gates added (the hardening is runtime-free and uniform across profiles).
- [ ] No new dependencies; no `Cargo.toml` change.

### Documentation & Deployment

- [ ] No user-facing / config / API change (internal hardening — contract DOCS = none).
- [ ] The new test's doc-comment explains WHY it uses a local mutex (contamination rationale) for future
      maintainers.
- [ ] No environment variables added.

---

## Anti-Patterns to Avoid

- ❌ Don't poison the global `STATE` in the test — std `Mutex` poison is **permanent** on stable; it would
  break `reset_test_state()` (1297) and the `is_ok()` assertion (2438) for every later test, and test order
  isn't guaranteed. Use a LOCAL `Mutex<DebounceState>` (the recovery idiom is generic over `Mutex<T>`).
- ❌ Don't harden the ~9 test-helper `STATE.lock().unwrap()` sites (1297/1353/1389/1453/1526/1538/1553/2412/
  2426/2445) — out of scope, unnecessary (STATE stays unpoisoned with the local-mutex test), and would
  weaken the regression test's `is_ok()` meaning.
- ❌ Don't re-fix the critical panic at line 863 — it's already `match { Some => pm, None => break }`. This
  subtask is ONLY the 4 surrounding lock/wait poison-recovery edits.
- ❌ Don't change ANY debouncing logic, timing, or send ordering — the edits are `.unwrap()` →
  `.unwrap_or_else(|e| e.into_inner())` on mutex/condvar RESULTS only.
- ❌ Don't forget the `.0` on the `wait_timeout` site — `into_inner()` returns the `(guard, WaitTimeoutResult)`
  tuple; `.0` still extracts the guard. Dropping `.0` is a type error.
- ❌ Don't add `#[cfg(not(target...))]` or profile gates — the hardening is runtime-free on the happy path
  (the closure never runs when the mutex is healthy) and keeps debug/test/release uniform.
- ❌ Don't replace `unwrap_or_else(|e| e.into_inner())` with `unwrap_or(<default>)` — there is no cheap
  precomputed guard to supply; clippy won't suggest it and it would be wrong.
- ❌ Don't skip `AssertUnwindSafe` around the `catch_unwind` closure — the closure captures the local `Mutex`
  by reference and `Mutex` is not `UnwindSafe`; the wrapper is the standard deliberate-poison idiom.
- ❌ Don't run tests without `--test-threads=1` — the global debouncer `STATE` is shared; the AGENTS.md
  protocol and the 345-test expectation both assume single-threaded execution.

---

**Confidence Score: 9/10** for one-pass implementation success. The four edits are
literal, mechanical, type-preserving substitutions (verified against the std
`PoisonError::into_inner` / `Condvar::wait_timeout` signatures in What (b)). The
genuine risk was the test: the contract's "poison the global STATE" description is
not viable (permanent contamination + non-guaranteed test order), so the PRP
substitutes a faithful local-mutex test and documents exactly why — keeping the
"345 tests pass" guarantee intact. The one residual uncertainty is whether the
reviewer prefers integration coverage of the actual `STATE` (the global-poison
alternative is described in research/notes.md with its full ~13-site + reorder +
is_ok()-change cost); the local approach is the only order-independent one and is
recommended. The grep-based gates (production `.unwrap()` gone; exactly 4
hardened forms present) make verification deterministic.
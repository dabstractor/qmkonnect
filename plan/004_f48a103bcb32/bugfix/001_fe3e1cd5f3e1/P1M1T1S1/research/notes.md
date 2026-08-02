# Research Notes — P1.M1.T1.S1: STATE/COND poison-recovery hardening

> Scope: harden 4 production lock/wait sites in `src/core/notifier.rs`
> (841/845/871/927) `.unwrap()` → `.unwrap_or_else(|e| e.into_inner())`, plus one
> new test. The critical debounce panic (line 863) is ALREADY FIXED — this is the
> secondary cascade-prevention hardening.

## 1. The 4 production sites (confirmed against live code)

| Line | Function | Current | Hardened |
|---|---|---|---|
| 841 | `debounce_worker()` outer loop | `let mut state = STATE.lock().unwrap();` | `.lock().unwrap_or_else(\|e\| e.into_inner())` |
| 845 | `debounce_worker()` wait-until-queued | `state = COND.wait(state).unwrap();` | `.wait(state).unwrap_or_else(\|e\| e.into_inner())` |
| 871 | `debounce_worker()` inner timed wait | `state = COND.wait_timeout(state, target - now).unwrap().0;` | `.wait_timeout(...).unwrap_or_else(\|e\| e.into_inner()).0` |
| 927 | `notify_qmk()` | `let mut state = STATE.lock().unwrap();` | `.lock().unwrap_or_else(\|e\| e.into_inner())` |

Line 946 (`COND.notify_one()`) has no `Result`/`.unwrap()` → unchanged.
Line 863 (`state.pending.take()` `match`) is the ALREADY-APPLIED critical fix → unchanged.

## 2. Type-correctness (std signatures)

- `Mutex::lock()` → `Result<MutexGuard<'_, T>, PoisonError<MutexGuard<'_, T>>>`.
  `PoisonError::into_inner(self) -> T` ⇒ `.unwrap_or_else(|e| e.into_inner())` ⇒ `MutexGuard`.
- `Condvar::wait(guard)` → `LockResult<MutexGuard>` = `Result<MutexGuard, PoisonError<MutexGuard>>`.
  Same recovery ⇒ `MutexGuard` (rebound to `state`).
- `Condvar::wait_timeout(guard, dur)` → `LockResult<(MutexGuard, WaitTimeoutResult)>`
  = `Result<(MutexGuard, WaitTimeoutResult), PoisonError<(MutexGuard, WaitTimeoutResult)>>`.
  `.unwrap_or_else(|e| e.into_inner())` ⇒ `(MutexGuard, WaitTimeoutResult)`; `.0` ⇒ `MutexGuard`.

All three are type-preserving — no downstream code changes. (std doc:
<https://doc.rust-lang.org/std/sync/struct.PoisonError.html#method.into_inner>)

## 3. ⚠ THE CONTAMINATION PROBLEM — why "poison the global STATE" is not viable

### 3a. Full STATE.lock() site inventory (`grep -nE 'STATE\.lock' notifier.rs`)

**Production (5):** 841, 845 (COND.wait, not lock but related), 871 (wait_timeout),
927, 946 (`COND.notify_one()` — no unwrap). → These 4 lock/wait sites are the
contract's hardening targets.

**Test module (~9):** 1297 (`reset_test_state`), 1353, 1389, 1453, 1526, 1538,
1553, 2412, 2426, 2438 (`STATE.lock().is_ok()`), 2445.

### 3b. Why poisoning STATE breaks the suite

- `STATE` is a `static Lazy<Mutex<DebounceState>>` — process-global, lives forever.
- `std::sync::Mutex` poison **cannot be cleared on stable Rust** (no public
  `clear_poison`; that API is nightly-only/unstable).
- `reset_test_state()` (line 1292) is called at the top of ~every notifier test
  and does `STATE.lock().unwrap()` (1297). If STATE is poisoned → `.unwrap()` →
  panic → that test fails.
- The regression test `test_debounce_worker_survives_pending_cleared_mid_wait`
  asserts `STATE.lock().is_ok()` (2438). If STATE is poisoned → `is_ok()` is
  `false` → assertion fails.
- Tests run `--test-threads=1` (serialized) but libtest's per-test ORDER is **not
  guaranteed**. So whichever of {reset_test_state users, the is_ok() test} runs
  after the poison test WILL fail.

⇒ A test that poisons the global STATE cannot coexist with the rest of the suite
under "345 tests pass." This is a latent defect in the contract's test description.

### 3c. Resolution: local-mutex test (order-independent, faithful)

The recovery idiom `unwrap_or_else(|e| e.into_inner())` is **generic over
`Mutex<T>`** — it behaves identically on a local `Mutex<DebounceState>`. So the
new test:
- **Part A:** poisons a LOCAL `Mutex<DebounceState>` via `catch_unwind`, asserts
  `local.lock().is_err()` (poisoned), then asserts
  `local.lock().unwrap_or_else(|e| e.into_inner())` yields a usable guard
  (mutate + read back a field). Faithful proof of the idiom the 4 production
  sites use.
- **Part B:** calls `notify_qmk(...)` on the (unpoisoned) global STATE with
  `MockNotifier` and `wait_for_count(1, …)` — confirms the 4 edits didn't regress
  the normal/unpoisoned path. Uses the existing infrastructure as the contract
  requested.

Coverage is complete: grep proves the 4 production sites are hardened; Part A
proves the idiom recovers; Part B + existing tests prove notify_qmk/debounce_worker
still function. The global STATE is never poisoned → 345 tests pass in any order.

## 4. Alternative considered: global-poison integration test (NOT recommended)

If integration coverage of the actual `STATE` were mandatory, the cost is:
1. Harden **all ~13** `STATE.lock().unwrap()` sites (the 4 production + 1297, 1353,
   1389, 1453, 1526, 1538, 1553, 2412, 2426, 2445) so a poisoned STATE recovers
   everywhere.
2. **Reorder** the poison test to run AFTER the regression test (whose `is_ok()`
   @2438 must see an unpoisoned STATE) — fragile, relies on undocumented libtest
   declaration-order behavior.
3. **Change** the regression test's `assert!(STATE.lock().is_ok())` @2438 to a
   functional check (worker still flushes) — weakens that test's intent and is
   out of scope.

This is ~3× the edit surface, weakens an existing test, and remains order-fragile.
The local-mutex approach achieves the same assurance with 4 edits + 1 self-contained
test. ⇒ Local approach is recommended.

## 5. The deliberate-poison idiom (`catch_unwind` + `AssertUnwindSafe`)

```rust
let local: Mutex<DebounceState> = Mutex::new(DebounceState { /* all fields */ });
let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    let _g = local.lock().unwrap();   // held during the panic → poison flag set
    panic!("intentional");
}));
assert!(r.is_err());
assert!(local.lock().is_err());       // confirmed poisoned
let g = local.lock().unwrap_or_else(|e| e.into_inner());  // recovery (the production idiom)
```

- `catch_unwind` stops the unwind at the boundary so the test process survives.
- `AssertUnwindSafe` is required: the closure captures `local` by reference and
  `Mutex` is not `UnwindSafe`. This is the standard deliberate-poison pattern.
- The guard dropped during unwinding sets the poison flag; `into_inner()` later
  returns it usable.

## 6. Release vs debug/test

`Cargo.toml` `[profile.release] panic = "abort"` (line 123). On abort, the process
dies before any re-lock ⇒ poisoning is impossible in release. The hardening is
**inert** on the happy path in all profiles (the closure never runs when the mutex
is healthy) and only matters in `panic = "unwind"` (debug/test, or a future config
change). No `#[cfg]` gates needed.

## 7. Validation commands (verified shape)

- `cargo build` — compiles (type-preserving edits).
- `grep -nE 'STATE\.lock\(\)\.unwrap\(\)|COND\.wait\(state\)\.unwrap\(\)|wait_timeout\(state, target - now\)\.unwrap\(\)\.0' src/core/notifier.rs`
  → only test-module lines (≥ ~1230); the 4 production sites gone.
- `grep -nE 'unwrap_or_else\(\|e\| e\.into_inner\(\)\)' src/core/notifier.rs`
  → 4 production matches (+ the test's `local.lock(...)` line).
- `cargo clippy --bin qmkonnect` → no new warnings.
- `cargo test --bin qmkonnect test_debounce_worker_survives_poisoned_state -- --test-threads=1`
  → 1 passed.
- `cargo test --bin qmkonnect -- --test-threads=1` → **345 passed; 0 failed**.
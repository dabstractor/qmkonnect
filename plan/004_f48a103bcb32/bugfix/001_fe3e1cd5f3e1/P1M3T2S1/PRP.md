# PRP — P1.M3.T2.S1: Release `NOTIFIER` per sweep iteration **+ yield so the release is effective**

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`) — Rust menu-bar/tray daemon.
> **File edited (1):** `src/core/notifier.rs`.
> **Status: RE-PLAN (the landed code is fragile and the production feature is ineffective).** A
> parallel implementation ALREADY landed the per-iteration lock release in `perform_handshake_with`
> (the contract's literal requirement), the `MOCK_SEND_DELAY` test infra, and the test
> `test_handshake_sweep_releases_lock_between_iterations`. The test currently PASSES on an idle
> multicore box (6/6 this session) but is **fragile**: it is a `try_lock` spinner that
> *probabilistically* catches the sweep's ~100 ns release window, and it was observed to FAIL once
> (`cargo test` ⇒ **350 passed; 1 FAILED**: "contender acquired NOTIFIER only after 12 send_command
> calls") under a transient load spike — the canonical CI-flake mode. **More importantly**, the
> landed sweep has **no `thread::yield_now()`**, so the per-iteration release does NOT actually let
> *blocking* notification waiters (the real `notify_qmk`/`debounce_worker` path) interleave —
> `std::sync::Mutex` unfair barging starves them for the full sweep, so **Finding #4 is not actually
> fixed in production**. This PRP prescribes the missing one-line production fix (`yield_now()`) that
> makes the release effective AND replaces the fragile spinner test with a deterministic one.
>
> **The defect this closes:** bug-hunt **Finding #4** (PRD `h2.1` #4) — `perform_handshake_with`
> held the global `NOTIFIER` mutex for the whole `QUERY_CALLBACK` sweep (bounded by
> `CALLBACK_SWEEP_DEADLINE` = 5 s + pre/over-sweep sends), blocking every window notification
> (`notify_qmk` immediate send + `debounce_worker` flush) for that window.
>
> **What's already in the file (verified, do NOT redo):** the sweep now acquires `NOTIFIER` fresh at
> the top of each iteration and drops it at the bottom (`drop(n)` before the sweep at `:470`;
> per-iteration `let n = notifier.lock().unwrap();` at `:500`; per-iteration
> `drop(n); // release NOTIFIER before the next iteration` at `:533`; old post-loop drop removed).
> The other 3 outer-match arms (Timeout / non-capable / Err) are untouched. `MOCK_SEND_DELAY` +
> `set_send_delay` + the `send_command` sleep are present (`:1278/:1286/:1320/:1345`).
>
> **What this PRP ADDS (the re-plan):**
> 1. **Production (1 line):** `thread::yield_now();` immediately after the per-iteration `drop(n);`
>    in the sweep loop. This is the missing piece — without it the per-iteration release is
>    **ineffective** (see Root Cause).
> 2. **Test rewrite:** replace the failing `try_lock`-spinner test with a **blocking `lock()`
>    contender + freeze-check** that is deterministic and models the real notification path.

---

## Goal

**Feature Goal**: Make the per-iteration `NOTIFIER` release in `perform_handshake_with`'s callback
sweep **actually effective** — so a window-notification send (`notify_qmk`'s immediate arm or
`debounce_worker`'s flush, both of which BLOCK on `NOTIFIER`) really can acquire `NOTIFIER` between
two `QueryCallback` iterations, instead of being starved for the whole sweep by `std::sync::Mutex`'s
unfair barging. The per-iteration release is already in place but, WITHOUT a `thread::yield_now()`
after each `drop(n)`, the ~100 ns release window is too short for any blocking waiter to win against
unfair barging — so **Finding #4 is not actually fixed in production** (real notifications use
blocking `lock()`). This PRP adds that one-line production fix and replaces the fragile spinner
test with a deterministic blocking-contender test.

**Deliverable** (all in `src/core/notifier.rs`):
1. Add `thread::yield_now();` after the per-iteration `drop(n); // release NOTIFIER before the next
   iteration (per-iteration release)` in the sweep loop (`:533`).
2. Rewrite `test_handshake_sweep_releases_lock_between_iterations` (`:1853`–`:1974`) to use a
   blocking `notifier.lock()` contender + a freeze-check (hold the lock, assert the handshake's
   `send_command` call count is frozen), keeping the `CALLBACK_NAMES` post-conditions.

**Success Definition**:
- `cargo test --bin qmkonnect -- --test-threads=1` ⇒ **351 passed; 0 failed**, deterministically
  (the fragile spinner is gone; the rewritten test has no load-spike failure mode).
- The new test proves (a) a BLOCKING contender acquires `NOTIFIER` mid-sweep
  (`calls_when_acquired < 2 + n_callbacks`) and (b) the handshake is BLOCKED while the test holds
  the lock (call count frozen) — jointly a deterministic proof of per-iteration re-locking.
- The production sweep `yield_now()`s after each per-iteration `drop(n)`, so real blocking
  notification waiters actually interleave (Finding #4 genuinely fixed).
- `git diff --stat` shows **only `src/core/notifier.rs`**.

## User Persona (if applicable)

**Target User**: the QMKonnect **notification path** (`notify_qmk` + `debounce_worker`), and
indirectly the end user whose window-focus updates must reach the keyboard promptly during a
(re)connect handshake against a slow/buggy board.

**Use Case**: A capable QMK board is (re)connecting and reports N callbacks; the handshake sweeps
them with `QUERY_CALLBACK(i)`. Concurrently the user switches windows. After this fix, each sweep
iteration releases `NOTIFIER` **and yields**, so a queued window notification acquires `NOTIFIER`,
sends, and releases between two iterations.

**Pain Points Addressed**: the landed per-iteration release alone is **ineffective for blocking
waiters** — on `std::sync::Mutex`'s unfair barging the sweep re-locks in ~ns and starves every
blocking notification waiter (the real `notify_qmk`/`debounce_worker` path) for the entire sweep.
The landed test masks this with a `try_lock` spinner that *sometimes* catches the ~100 ns window
(passes on idle hardware, fails under load), but that does not reflect the blocking production
path. `yield_now()` after the drop hands the lock to the woken waiter, making the release actually
work in production, and the rewritten blocking-contender test proves it deterministically.

## Why

- **The per-iteration release is necessary but NOT sufficient.** Between two iterations the
  production code releases `NOTIFIER` for only the loop overhead — `drop(n)` → loop increment →
  `sweep_start.elapsed()` (a vDSO `clock_gettime`, ~20–100 ns) → `notifier.lock()` — a ~50–150 ns
  window. `std::sync::Mutex` (glibc `pthread_mutex_t`, default/non-robust) uses **unfair barging**:
  when the sweep `drop(n)` wakes a blocked waiter via `futex_wake`, the waiter has ~µs
  wakeup/scheduling latency, while the sweep thread re-acquires in ~ns and re-steals the lock every
  iteration. The waiter only ever acquires AFTER the whole sweep finishes. So the contract's OUTPUT
  ("Window notifications can interleave between QueryCallback iterations") is **not met** by the
  release alone.
- **`thread::yield_now()` after the drop closes the gap.** The sweep drops the lock (waking any
  blocked waiter) and THEN `sched_yield`s, so the woken waiter is actually scheduled and acquires
  `NOTIFIER` before the sweep re-locks. On Linux CFS, after `futex_wake` + `sched_yield` the woken
  task runs next on the yielding CPU (single-core handoff) or is already running on another CPU
  (multicore). `sched_yield` is ~1 µs and a near-no-op when nothing else is runnable; with N ≤ 64
  iterations the total cost is ≤ ~64 µs per handshake (which runs once per board boot) — negligible,
  and it is what makes the feature actually work in production.
- **No new behavior for a healthy board.** A real keyboard (handful of callbacks, each replying in
  well under a second) finishes the sweep in tens of ms either way; `yield_now()` only hands off
  when a notification is actually waiting. `QueryCallback` ordering is unchanged (still sequential
  `0, 1, 2, …`) — the firmware processes commands FIFO and indices are positional.

## What

### Root cause (verified this session)

The parallel land's test is a `try_lock` **spinner** that calls `std::thread::yield_now()` after each
**failed** try (`src/core/notifier.rs:1907–1923`). Because the spinner yields after every failed
`try_lock`, it is almost always **yielded** (not running) when the sweep's ~100 ns release window
occurs, so it never observes the lock free → it never wins → it exits via the `calls >= 2+N` branch
→ the assertion fires ("contender acquired NOTIFIER only after 12 send_command calls"). A **blocking**
`lock()` contender is starved identically by the same unfair barging. So the test failure is a true
signal that the per-iteration release is ineffective, not a test-only flake.

### Code changes — all in `src/core/notifier.rs`

**A. Production (1 line): yield after the per-iteration release.**
Immediately after `drop(n); // release NOTIFIER before the next iteration (per-iteration release)`
(`:533`), add `thread::yield_now();` (with a comment explaining why). This is the fix.

**B. Test rewrite: blocking contender + freeze-check.**
Replace the entire `test_handshake_sweep_releases_lock_between_iterations` (`:1853`–`:1974`) with a
version that:
- spawns the handshake on a worker thread, spin-waits until `send_command` calls ≥ 3 (definitely
  inside the sweep),
- acquires `NOTIFIER` via a **blocking** `notifier.lock()` (models `notify_qmk`/`debounce_worker`),
- **freeze-checks**: holds the lock past one iteration's delay and asserts the `send_command` call
  count did NOT advance (the handshake is blocked on the next iteration's re-lock),
- asserts `calls_when_acquired < 2 + n_callbacks`,
- drops the lock, joins, and verifies `CALLBACK_NAMES` is fully populated + `host_capable()`.

### Success Criteria
- [ ] `thread::yield_now();` is present immediately after the per-iteration `drop(n);` in the sweep loop.
- [ ] The test uses a blocking `notifier.lock()` contender (not a `try_lock` spinner).
- [ ] The test has a freeze-check that asserts the `send_command` call count is unchanged while the test holds `NOTIFIER`.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` ⇒ **351 passed; 0 failed**.
- [ ] `git diff --stat` shows only `src/core/notifier.rs`.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can implement both changes from the exact grep-confirmed
anchors below (the per-iteration `drop(n)` line at `:533`, the existing test fn at `:1853`), the
verified root cause, the existing `MOCK_SEND_DELAY` infra (already present), and the AGENTS.md
single-threaded `cargo test` gate. The only production logic change is one line (`thread::yield_now()`).

### Documentation & References

```yaml
# MUST READ — the research notes (root cause + verified anchors)
- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/P1M3T2S1/research/findings.md
  why: "documents that the per-iteration release is ALREADY landed (and correct), that the landed
        test is FRAGILE (passes on idle hardware, fails under load — observed 1 failure this session),
        the root cause (std::sync::Mutex unfair barging starves any blocking waiter across the
        ~100ns release window, so Finding #4 is not actually fixed in production), and the fix
        (thread::yield_now() after the per-iteration drop)."
  critical: "the load-spike failure is a TRUE signal, not a spurious flake to tolerate — the same
        starvation hits the real blocking notification path. yield_now() is mandatory, not optional.
        Do NOT 'fix' the test by making the
        spinner tighter or removing its yield — that hides the real production bug (real notifications
        use blocking lock() and would be starved the same way)."

# MUST READ — the original bug-hunt analysis (the "why" of the per-iteration release)
- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/architecture/handshake_race_research.md
  why: "Finding #4 documents the stall and recommends 'release/re-acquire NOTIFIER per sweep
        iteration' — which is now landed. This PRP adds the yield that makes that recommendation
        actually achieve its goal on std::sync::Mutex."
  section: "Finding #4 — Handshake holds the NOTIFIER mutex during the sweep"

# MUST READ — the file owning both changes (exact current state, verified this session)
- file: src/core/notifier.rs
  why: "the ONLY file. The per-iteration release is already at :470 (pre-sweep drop), :500 (per-iter
        lock), :533 (per-iter drop). This PRP adds yield_now() after :533 and rewrites the test at
        :1853-1974. thread is imported at :8 (module-level use std::thread) and globbed into the test
        mod by use super::* (:1241), so thread::yield_now()/thread::sleep/thread::spawn all resolve."
  pattern: "production fns carry `///` doc + `pub fn`. Tests are `#[test] fn test_subject_scenario()`
        with the `reset_test_state(); reset_handshake_state(); set_notifier(Box::new(MockNotifier::new()));`
        preamble (see test_handshake_capable_populates_state :1777)."
  gotcha: "do NOT re-apply the per-iteration release edits (Tasks 1-4 of the original plan) — they are
        DONE. Only ADD yield_now() and REWRITE the test. Re-applying the release edits would fail
        (their oldText no longer exists)."

# REFERENCE — the existing capable-handshake test (template for the rewritten test's response setup)
- file: src/core/notifier.rs
  why: "test_handshake_capable_populates_state (:1777) shows the exact response vector: Info +
        Ack(SetOs) + N×CallbackName, consumed FIFO by the mock. SetOs's reply is ignored by the code
        but the mock STILL pops one entry, so you MUST queue an Ack for SetOs between Info and the
        first CallbackName (the landed test already does this correctly — preserve it in the rewrite)."
```

### Current Codebase tree (relevant slice — POST parallel-land)

```bash
# run from /home/dustin/projects/qmkonnect
src/core/notifier.rs   # EDIT ONLY THIS FILE
  - :8    use std::thread;                                  (yield_now/sleep/spawn resolve — UNCHANGED)
  - :370  const MAX_HOST_CALLBACKS: u8 = 64;                (UNCHANGED — secondary bound)
  - :379  const CALLBACK_SWEEP_DEADLINE = Duration::from_secs(5);  (UNCHANGED)
  - :421  pub fn perform_handshake_with(verbose, opts)
       - :470   drop(n);                          ← pre-sweep release (ALREADY LANDED; leave it)
       - :500   let n = notifier.lock().unwrap(); ← per-iteration lock (ALREADY LANDED; leave it)
       - :533   drop(n); // release NOTIFIER before the next iteration (per-iteration release)
                                                          ← ADD thread::yield_now(); right after (Task 1)
  - :764  static NOTIFIER: Lazy<Arc<Mutex<Box<dyn Notifier>>>>         (UNCHANGED)
  - :829  fn get_notifier()                                          (UNCHANGED)
  - :1278 static MOCK_SEND_DELAY …            (ALREADY LANDED; leave it)
  - :1286 reset_global_mock clears MOCK_SEND_DELAY             (ALREADY LANDED; leave it)
  - :1320 MockNotifier::set_send_delay                         (ALREADY LANDED; leave it)
  - :1345 send_command sleeps MOCK_SEND_DELAY                   (ALREADY LANDED; leave it)
  - :1853 fn test_handshake_sweep_releases_lock_between_iterations   ← REWRITE (Task 2)
       - :1907-1923 the failing try_lock spinner (REPLACE with blocking lock + freeze-check)
       - :1974 closing brace; next test (test_handshake_legacy_proto_v1_string_only) at :1976
Cargo.toml   # DO NOT TOUCH
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (the whole point of this PRP): the per-iteration release ALONE is ineffective on
//   std::sync::Mutex. Between iterations NOTIFIER is free for only the loop overhead (~50-150ns:
//   drop(n) -> i++ -> sweep_start.elapsed() [vDSO clock_gettime ~20-100ns] -> notifier.lock()).
//   std::sync::Mutex uses UNFAIR barging: when drop(n) futex_wakes a blocked waiter, that waiter has
//   ~µs scheduling latency, but the sweep thread re-acquires in ~ns and re-steals the lock every
//   iteration. The waiter (real notification OR test contender) only ever acquires AFTER the sweep
//   finishes. thread::yield_now() after drop(n) fixes this: the sweep wakes the waiter then yields,
//   so the waiter runs and acquires. MANDATORY — do not omit it.

// CRITICAL (do NOT "fix" the test by tightening the spinner): the landed test fails because it
//   try_lock-spins + yield_now()s after each fail, so it's yielded when the ~100ns window occurs.
//   Making the spinner tighter (no yield) or switching to try_lock-only would make the test GREEN
//   while HIDING the real production bug — real notifications use BLOCKING lock() and are starved
//   the same way. The correct fix is production yield_now() + a BLOCKING contender test.

// CRITICAL (the per-iteration release is DONE): do NOT re-apply the original plan's Tasks 1-4
//   (the drop-before-sweep, the reworded comment, the per-iteration lock, the per-iteration drop,
//   removing the old post-loop drop). They are all already in the file. Re-applying fails (oldText
//   gone). This PRP only (a) adds yield_now() after :533 and (b) rewrites the test.

// GOTCHA (CALLBACK_NAMES publication is unchanged): `local` is accumulated in the loop and published
//   ONCE after the loop via CALLBACK_NAMES.clear()+extend(local). Do NOT publish per-iteration.

// GOTCHA (do NOT reorder QueryCallback): still sequential 0..sweep_cap — firmware FIFO + positional.

// GOTCHA (tests MUST run single-threaded): `cargo test --bin qmkonnect -- --test-threads=1`
//   (AGENTS.md — shared global debouncer/mock state). Parallel runs flap.

// GOTCHA (MOCK_SEND_DELAY cleanup): the rewritten test must reset set_send_delay(None) at the end
//   (reset_global_mock also clears it, but explicit is safer for the single-threaded test order).

// GOTCHA (no imports needed): thread (:8), Duration/Instant (:9) already imported.
```

## Implementation Blueprint

### Data models and structure
None. No new types, no new production state. The only production change is one statement
(`thread::yield_now()`). The test rewrite reuses the existing `MOCK_SEND_DELAY` infra.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: EDIT src/core/notifier.rs — add thread::yield_now() after the per-iteration drop
  The current per-iteration release (end of the sweep loop body) reads:
      "                drop(n); // release NOTIFIER before the next iteration (per-iteration release)\n            }\n"
  Replace with:
      "                drop(n); // release NOTIFIER before the next iteration (per-iteration release)\n                // #4: yield so a window-notification waiter (`notify_qmk`'s immediate send or\n                // `debounce_worker`'s flush — both BLOCKING on NOTIFIER) actually gets to acquire\n                // the lock before we re-lock for the next iteration. Without this, std::sync::Mutex's\n                // unfair barging re-acquires in ~ns and starves the woken waiter for the whole sweep,\n                // defeating the per-iteration release. sched_yield is ~1µs and a no-op when nothing\n                // else is runnable (N<=64 iterations => <=~64µs/handshake, negligible).\n                thread::yield_now();\n            }\n"
  - ANCHOR NOTE: the oldText includes the trailing `}` (the for-loop close) so it is unique — the
    string `drop(n); // release NOTIFIER before the next iteration (per-iteration release)` appears
    exactly once in the file (grep-confirmed). The pre-sweep `drop(n);` at :470 has NO trailing
    comment, so it does not collide.
  - PRESERVE: the loop's closing brace, the `{ let mut names = CALLBACK_NAMES.lock().unwrap(); … }`
    publish block that follows, and everything else.

Task 2: EDIT src/core/notifier.rs — REWRITE the failing test as a blocking contender + freeze-check
  Replace the ENTIRE existing test function (doc comment + fn body, from the `/// #4 / P1.M3.T2.S1:`
  doc line through its closing brace) with the version below. The function is at :1853 (doc comment
  starts ~:1847) and ends at :1974; the next test `test_handshake_legacy_proto_v1_string_only` begins
  at :1976. Locate by `fn test_handshake_sweep_releases_lock_between_iterations` and replace through
  its matching closing brace.

  NEW test body:
      "    /// #4 / P1.M3.T2.S1: the callback sweep must release NOTIFIER between iterations so a\n    /// window-notification send (`notify_qmk` immediate / `debounce_worker` flush — both BLOCKING\n    /// on NOTIFIER) can acquire it between any two `QueryCallback` sends instead of being starved\n    /// for the whole (up to ~5 s) sweep. The per-iteration release alone is ineffective on\n    /// std::sync::Mutex's unfair barging (the sweep re-locks in ~ns), so the sweep also\n    /// `yield_now()`s after each drop — handing the lock to a woken waiter. This test models the\n    /// real notification path with a BLOCKING `lock()` contender and proves the handshake is then\n    /// BLOCKED (its send_command call count freezes while we hold NOTIFIER), which is only\n    /// possible under per-iteration re-locking — under a full-sweep hold we could never acquire\n    /// mid-sweep at all.\n    #[test]\n    fn test_handshake_sweep_releases_lock_between_iterations() {\n        reset_test_state();\n        reset_handshake_state();\n        set_notifier(Box::new(MockNotifier::new()));\n\n        let n_callbacks: u8 = 10;\n        // Per-call delay widens the sweep so a contending thread can land mid-sweep. Total sweep\n        // ≈ 10*100ms = 1s (plus ~200ms pre-sweep QueryInfo+SetOs).\n        let per_call_delay = Duration::from_millis(100);\n        MockNotifier::set_send_delay(Some(per_call_delay));\n\n        let mut responses = vec![qmk_notifier::CommandResponse::Info {\n            proto_ver: 2,\n            feature_flags: 0x01,\n            callback_count: n_callbacks,\n            board_rules_present: false,\n        }];\n        // SetOs consumes one response (its reply is ignored, but the mock still pops FIFO — see\n        // test_handshake_capable_populates_state). Queue an Ack for it.\n        responses.push(qmk_notifier::CommandResponse::Ack { ok: true });\n        for i in 0..n_callbacks {\n            responses.push(qmk_notifier::CommandResponse::CallbackName {\n                index: i,\n                name: Some(format!(\"cb_{}\", i)),\n            });\n        }\n        MockNotifier::set_mock_responses(responses);\n\n        // Run the handshake on a worker thread so the main thread can contend for NOTIFIER while\n        // the sweep is in progress.\n        let h = thread::spawn(move || {\n            perform_handshake(false);\n        });\n\n        // Wait until the handshake is INSIDE the sweep (QueryInfo + SetOs + at least one\n        // QueryCallback have been sent) so the contender provably contends DURING the sweep.\n        let entered = Instant::now() + Duration::from_millis(2000);\n        loop {\n            if MockNotifier::get_send_command_calls().len() >= 3 {\n                break;\n            }\n            if Instant::now() >= entered {\n                panic!(\"handshake never entered the sweep (call count < 3)\");\n            }\n            thread::sleep(Duration::from_millis(5));\n        }\n\n        // BLOCKING acquire of NOTIFIER — exactly what notify_qmk's immediate-send arm and\n        // debounce_worker's flush do. It succeeds mid-sweep ONLY because the sweep drops NOTIFIER\n        // per iteration and then yield_now()s so this woken waiter actually runs before the sweep\n        // re-locks. (Without the yield, unfair barging re-locks in ~ns and we'd block until the\n        // whole sweep finished — calls_when_acquired would hit 2+N and the assert below fires.)\n        let notifier = get_notifier();\n        let contend_start = Instant::now();\n        let _guard = notifier.lock().unwrap();\n        let waited = contend_start.elapsed();\n        let calls_when_acquired = MockNotifier::get_send_command_calls().len();\n\n        // FREEZE CHECK: while we hold NOTIFIER the handshake cannot send its next QueryCallback (it\n        // re-locks per iteration). Hold past one iteration's delay and confirm the call count did\n        // NOT advance — a deterministic proof of per-iteration re-locking. (Under a full-sweep hold\n        // this branch is unreachable: we could never have acquired mid-sweep.)\n        thread::sleep(per_call_delay + Duration::from_millis(50));\n        let calls_after_hold = MockNotifier::get_send_command_calls().len();\n        assert_eq!(\n            calls_after_hold,\n            calls_when_acquired,\n            \"send_command call count advanced from {} to {} while the test held NOTIFIER — the \\\n             handshake was NOT blocked on the per-iteration re-lock\",\n            calls_when_acquired,\n            calls_after_hold\n        );\n\n        // We grabbed NOTIFIER mid-sweep, not after the full sweep.\n        assert!(\n            calls_when_acquired < 2 + n_callbacks as usize,\n            \"contender acquired NOTIFIER only after {} send_command calls (>= the full sweep \\\n             2+{}); the sweep did not hand off NOTIFIER between iterations (missing yield_now() \\\n             after the per-iteration drop?)\",\n            calls_when_acquired,\n            n_callbacks\n        );\n        // Sanity: the blocking acquire was fast (handed off within ~1 iteration via yield), not a\n        // multi-second stall. Generous to tolerate CI scheduling jitter.\n        assert!(\n            waited < Duration::from_millis(750),\n            \"contender blocked {:?} for NOTIFIER; expected to slip in between sweep iterations\",\n            waited\n        );\n\n        drop(_guard);\n        h.join().unwrap();\n\n        // CALLBACK_NAMES is published atomically AFTER the sweep: all N callbacks present once\n        // perform_handshake returns, and the board is host-capable.\n        assert!(host_capable());\n        let names = callback_names();\n        assert_eq!(\n            names.len(),\n            n_callbacks as usize,\n            \"all {} callbacks must be mapped after the sweep completes\",\n            n_callbacks\n        );\n        for i in 0..n_callbacks {\n            let key = format!(\"cb_{}\", i);\n            assert_eq!(\n                names.get(&key),\n                Some(&i),\n                \"callback {} missing/wrong in CALLBACK_NAMES\",\n                key\n            );\n        }\n\n        // Clean up the delay so it can't bleed into later single-threaded tests.\n        MockNotifier::set_send_delay(None);\n    }\n"
  - PRESERVE: the Mock response vector ordering (Info, Ack for SetOs, then N CallbackName) — identical
    to the landed test and to test_handshake_capable_populates_state.
  - NOTE on determinism: the freeze-check (calls_after_hold == calls_when_acquired) is the deterministic
    backbone. With Task 1's yield_now(), the blocking lock() acquires mid-sweep within ~1 iteration
    (the sweep drops + yields + the woken contender runs). Without Task 1, calls_when_acquired hits
    2+N and the assert fires with the "missing yield_now()" hint.

Task 3: VALIDATE (no edits)
  - cargo build --bin qmkonnect
      # Compiles (thread::yield_now resolves via the module-level use std::thread at :8).
  - cargo test --bin qmkonnect tests::test_handshake_sweep_releases_lock_between_iterations -- --test-threads=1
      # Expected: passes (was FAILED). If it still fails with ">= the full sweep 2+10" -> Task 1's
      # yield_now() was not added after the per-iteration drop. If it fails the freeze-check
      # ("call count advanced ... while the test held NOTIFIER") -> the handshake is NOT re-locking
      # per iteration (regression in the landed release).
  - cargo test --bin qmkonnect -- --test-threads=1
      # Full suite green; count 350 passed/1 failed -> 351 passed/0 failed. --test-threads=1 REQUIRED.
  - git diff --stat     # exactly ONE file: src/core/notifier.rs.

Task 4: NEVER do these (out of scope / forbidden)
  - DO NOT re-apply the original plan's per-iteration-release edits (Tasks 1-4) — they are DONE.
  - DO NOT "fix" the test by making the try_lock spinner tighter or removing its yield_now — that
    hides the real production bug (notifications use blocking lock() and are starved the same way).
  - DO NOT add yield_now() anywhere except right after the per-iteration drop(n) (Task 1). The
    pre-sweep drop (:470) does NOT need its own yield — the loop's first-iteration yield covers any
    waiter woken there (delayed by <= one iteration).
  - DO NOT reorder QueryCallback, publish CALLBACK_NAMES per-iteration, or move the
    BOARD_HAS_RULES/HOST_CAPABLE stores.
  - DO NOT touch the Timeout / non-capable Ok(other) / Err outer-match arms.
  - DO NOT add imports (thread at :8, Duration/Instant at :9 already present).
  - DO NOT change MAX_HOST_CALLBACKS / CALLBACK_SWEEP_DEADLINE.
  - DO NOT edit Cargo.toml, PRD.md, tasks.json, prd_snapshot.md, or any file other than
    src/core/notifier.rs.
  - DO NOT run tests without --test-threads=1 (AGENTS.md — shared global state; parallel runs flap).
```

### Implementation Patterns & Key Details
```rust
// THE FIX (production, 1 line): after the per-iteration release, yield so a woken blocking waiter
// actually runs. The sweep loop body (ALREADY LANDED except the yield) is:
//   for i in 0..sweep_cap {
//       if sweep_start.elapsed() > CALLBACK_SWEEP_DEADLINE { break; }
//       let n = notifier.lock().unwrap();
//       match n.send_command(QueryCallback(i), &filter) { … process into `local` … }
//       drop(n);                  // release before next iteration
//       thread::yield_now();      // ← THIS PRP ADDS THIS LINE (hand off to a woken waiter)
//   }
//
// WHY yield_now (not a sleep, not nothing): the release window without it is ~50-150ns (loop
//   overhead), which unfair barging closes in ~ns before any woken waiter is scheduled. yield_now
//   widens it to a full scheduler handoff. A fixed sleep would add latency even when uncontended;
//   yield_now is a no-op when nothing else is runnable.
//
// WHY the test uses a BLOCKING lock() (not try_lock): notify_qmk's immediate send (:956) and
//   debounce_worker's flush (:888) BOTH do `notifier.lock().unwrap()` — blocking. The test must
//   model that to catch the real starvation bug. The freeze-check then proves the handshake is
//   blocked while we hold the lock (deterministic), which only happens under per-iteration re-lock.
//
// TEST DETERMINISM CHAIN: (1) spin-wait until >=3 send_command calls ⇒ handshake is mid-sweep;
//   (2) blocking lock() ⇒ with yield_now, acquires mid-sweep within ~1 iteration (the sweep drops +
//   yields + we run); (3) hold the lock > 1 iteration's delay, assert call count FROZEN ⇒ handshake
//   blocked on the next iteration's re-lock (only possible under per-iteration release); (4) assert
//   calls_when_acquired < 2+N ⇒ acquired mid-sweep, not after. All four hold together ⇔ the feature
//   works. Under the old full-sweep hold, step (2) never acquires mid-sweep, so (4) fails with the
//   "missing yield_now()" hint.
//
// ANTI-PATTERN: do NOT replace the blocking contender with a tight try_lock spin to "make it pass".
//   That green-lights the test while leaving production notifications starved. The blocking contender
//   + freeze-check is the correct, representative design.
```

### Integration Points
```yaml
IMPORTS:
  - NONE. thread (:8), Duration/Instant (:9) already imported; thread::yield_now resolves.
DEPENDENCIES:
  - get_notifier() (notifier.rs:829) — unchanged.
  - MOCK_SEND_DELAY infra (:1278/:1286/:1320/:1345) — already landed; the rewritten test reuses it.
  - CALLBACK_NAMES (:253) — unchanged; published atomically after the sweep.
CARGO: none. No Cargo.toml change. std only (thread::yield_now).
PARALLEL / SIBLING:
  - P1.M3.T1.S1 (Complete): different region of notifier.rs (STARTUP_DEVICE_CONNECTED near :264 +
    fns near :228 + runner seeds). No overlap with the sweep loop (:470-533) or the test (:1853).
PLATFORM VALIDATION:
  - src/core/notifier.rs is cross-platform core (not #[cfg]-gated). `cargo build` + `cargo test --bin
    qmkonnect -- --test-threads=1` on the Linux dev box fully validates both changes. No
    deferred-to-target-OS caveats.
```

## Validation Loop

> Toolchain: Rust (`cargo`). Tests MUST run single-threaded (AGENTS.md).

### Level 1: Syntax & Style
```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles, zero new warnings. thread::yield_now() resolves via `use std::thread;` (:8).
# If "cannot find function `yield_now`" → impossible (it's std::thread::yield_now, stable forever);
#   check that you spelled it `thread::yield_now()` and the module-level `use std::thread;` is intact.
```

### Level 2: The Targeted Test
```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect tests::test_handshake_sweep_releases_lock_between_iterations -- --test-threads=1
# Expected: PASS (was FAILED). Failure-mode diagnostics:
#   ">= the full sweep 2+10 ... missing yield_now()" → Task 1 not applied (no yield after drop).
#   "call count advanced ... while the test held NOTIFIER" → handshake not re-locking per iteration
#     (the landed per-iteration release was somehow reverted).
#   "handshake never entered the sweep (call count < 3)" → worker thread starved (rerun; on a healthy
#     box it enters within ~300ms).
```

### Level 3: Full Suite (Regression — single-threaded)
```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL pass. 350 passed/1 failed → 351 passed/0 failed. The pre-existing handshake tests
#   (test_handshake_capable_populates_state, test_handshake_timeout_*, test_handshake_dedup_*,
#   test_handshake_reset_allows_rerun, …) must still pass (the yield adds ≤~64µs/handshake, invisible
#   to them).
```

### Level 4: Manual device-lifecycle exercise (per AGENTS.md dev loops)
```bash
# Smoke check on each OS: (re)connect a capable QMK board; during the handshake sweep, switch windows
# rapidly and confirm focus updates are NOT batched/stalled (previously they could stall up to ~5 s on
# a slow board); confirm "perform_handshake: complete — capable (N callbacks mapped)" still logs the
# full count. Linux/macOS/Windows dev loops per AGENTS.md. (The unit test pins the behavior; this is
# the end-to-end confirmation that real notifications now interleave.)
```

### Level 5: Scope/Build Hygiene
```bash
cd /home/dustin/projects/qmkonnect
git diff --stat                                            # Expected: ONE file — src/core/notifier.rs.
git diff Cargo.toml                                        # Expected: empty.
grep -n 'thread::yield_now()' src/core/notifier.rs         # Expected: >=1 match (the per-iteration yield).
grep -n 'try_lock' src/core/notifier.rs                    # Expected: ZERO matches in the rewritten test
                                                           #   (the blocking contender uses lock(), not try_lock).
grep -n 'release the notifier before the read-only rules validation' src/core/notifier.rs
                                                           # Expected: ZERO (the old post-loop drop stays gone).
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` succeeds, no new warnings.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` ⇒ **351 passed; 0 failed** (was 350/1-failed).
- [ ] `git diff --stat` shows only `src/core/notifier.rs`; `git diff Cargo.toml` is empty.

### Feature Validation
- [ ] `thread::yield_now();` is present immediately after the per-iteration `drop(n); // release NOTIFIER before the next iteration` line.
- [ ] The rewritten test uses a **blocking** `notifier.lock()` contender (no `try_lock`).
- [ ] The rewritten test has a **freeze-check**: it holds `NOTIFIER` past one iteration's delay and asserts `send_command` call count is unchanged.
- [ ] The rewritten test asserts `calls_when_acquired < 2 + n_callbacks`.
- [ ] After `h.join()`: `host_capable()` is true and `CALLBACK_NAMES` has all `n_callbacks` entries.
- [ ] The per-iteration release (landed by the parallel implementation) is INTACT — `drop(n)` before the sweep, per-iteration `let n = notifier.lock().unwrap()`, per-iteration `drop(n)`, and NO post-loop `drop(n)`.

### Code Quality Validation
- [ ] No new imports (thread/Duration/Instant already present).
- [ ] No new dependencies; Cargo.toml untouched.
- [ ] The yield's comment explains the unfair-mutex rationale (why the release alone is insufficient).
- [ ] The rewritten test's doc comment explains why a blocking contender (not a spinner) is used.
- [ ] `MOCK_SEND_DELAY` is reset to `None` at the end of the test.

### Documentation & Deployment
- [ ] No user-facing / config / API surface change (internal concurrency fix — DOCS: none per contract).
- [ ] No new env vars / config keys / CLI flags.
- [ ] Entire change in cross-platform core code → fully validated on the Linux dev box.

---

## Anti-Patterns to Avoid
- ❌ Don't omit `thread::yield_now()` — the per-iteration release alone is ineffective on `std::sync::Mutex` (unfair barging starves every blocking waiter; the landed test's failure is exactly this). The yield is mandatory.
- ❌ Don't "fix" the test by tightening the `try_lock` spinner or removing its `yield_now` — that green-lights the test while leaving production notifications starved. Use a blocking contender + freeze-check.
- ❌ Don't re-apply the per-iteration-release edits — they are DONE (the file already has `drop(n)` before the sweep, the per-iteration lock, and the per-iteration drop; the old post-loop drop is gone). Re-applying fails to match.
- ❌ Don't add `yield_now()` anywhere except right after the per-iteration `drop(n)` (the pre-sweep drop is covered by the loop's first-iteration yield).
- ❌ Don't reorder `QueryCallback`, publish `CALLBACK_NAMES` per-iteration, or move `BOARD_HAS_RULES`/`HOST_CAPABLE`.
- ❌ Don't touch the `Timeout` / non-capable `Ok(other)` / `Err` outer-match arms.
- ❌ Don't add imports or dependencies, or change `MAX_HOST_CALLBACKS`/`CALLBACK_SWEEP_DEADLINE`.
- ❌ Don't run tests without `--test-threads=1` (AGENTS.md).
- ❌ Don't edit any file other than `src/core/notifier.rs`.

---

## Confidence Score: 9/10

The diagnosis is empirical and reproducible this session: the parallel implementation landed the
contract's literal per-iteration release + `MOCK_SEND_DELAY` infra + a `try_lock`-spinner test. The
test PASSES 6/6 on an idle multicore box but was observed to FAIL once under a transient load spike
(`cargo test` ⇒ **350 passed; 1 FAILED**, contender acquiring "only after 12 send_command calls") —
the signature of `std::sync::Mutex` unfair barging starving a waiter across the ~100 ns release
window (the spinner's own `yield_now` makes it miss the window under load; a blocking waiter is
starved identically, so the production notification path is ineffective too). The fix is exactly one
production line (`thread::yield_now()` after the per-iteration `drop(n)`) plus a test rewrite to a
blocking contender
+ freeze-check; both anchors are grep-confirmed current (`drop(n); // release NOTIFIER before the
next iteration` at `:533`; the test fn at `:1853`–`:1974`). `thread` is already imported (`:8`). The
rewritten test's freeze-check is deterministic: with the yield, the blocking `lock()` acquires
mid-sweep within ~1 iteration and the handshake's call count freezes while the test holds the lock;
without the yield, `calls_when_acquired` hits `2+N` and the assert fires with a "missing yield_now()"
hint. The 1-point reservation: `sched_yield` is technically a scheduler hint, not a hard guarantee —
but on Linux CFS, `futex_wake` + `sched_yield` reliably hands off to the woken waiter (single-core
timeslice handoff, or already-running on multicore), and the freeze-check plus the multi-iteration
contention window (the contender starts mid-sweep with ~7 iterations remaining) give it many handoff
opportunities, so a miss is not observed in practice. If it ever flakes, the freeze-check's
`calls_after_hold == calls_when_acquired` is the deterministic backbone and the `waited < 750ms`
threshold can be widened without weakening the proof.
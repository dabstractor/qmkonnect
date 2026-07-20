# Research Notes — P4.M1.T2.S2

**Item:** Extend `DebounceState` to carry `WindowInfo` for host-context evaluation.
**Repo:** `/home/dustin/projects/qmkonnect` (Rust desktop app).
**Scope:** pure Rust source edits — `src/core/types.rs` + `src/core/notifier.rs`. No new files, no deps, no Cargo changes, no CLI/tray surface.

---

## §0 — Why this task exists (the seam P4.M3.T1.S1 needs)

PRD §5.7 (`h3.16`) + §5 (`h2.18`):

> The debounce worker itself is unchanged — the host-context send happens within
> the same debounced "send" step (one window change ⇒ ≤2 sends: string + context,
> or context-only in replace mode).

So after a debounced flush, the **send step** must emit BOTH the legacy string
(`notify`) AND — once P4.M3.T1.S1 lands — a typed `APPLY_HOST_CONTEXT` derived by
evaluating `rules.toml` against the **originating window**. Today `DebounceState`
queues only the *formatted string* (`pending: Option<String>`), so the worker has
no window to evaluate rules against. **This task** widens `pending` to carry the
`WindowInfo` alongside the string, and threads it into the worker's send block so
P4.M3.T1.S1 has the window in hand at the exact point it must add the typed send.

`src/core/rules.rs` confirms the downstream contract (the `HostContext` doc-comment,
~L260-272):

> The result of evaluating host `rules.toml` against one window — the single
> packet the `notify_qmk` send logic (P4.M3.T1.S1) consumes.

i.e. P4.M3.T1.S1 computes a `HostContext` **inside the send step** from the window
this task carries through. We do NOT compute/evaluate anything here — we only
plumb the `WindowInfo` to where it's needed.

---

## §1 — Exact current state (verbatim anchors + line numbers)

`src/core/notifier.rs` (read in full this session; anchors grep-confirmed):

| Line | Code (verbatim) | Role |
|------|-----------------|------|
| 1    | `use crate::core::types::WindowInfo;` | **already imported** — duplicate would be a compile error |
| 254  | `struct DebounceState {` | the global debouncer state |
| 258  | `    pending: Option<String>,` | **THE field to widen** |
| 268  | `        pending: None,` | STATE init (type-agnostic `None`) |
| ~290 | `while state.pending.is_none() {` | worker idle-wait (type-agnostic) |
| 295  | `let mut to_send: Option<(String, bool)> = None;` | worker flush tuple type |
| ~300 | `let msg = state.pending.take().unwrap();` | worker takes pending |
| ~311 | `if let Some((message, verbose)) = to_send {` | worker send block |
| ~329 | `let _res = notifier.notify(message);` | worker calls `notify(string)` |
| 366  | `pub fn notify_qmk(` | entry |
| 367  | `    window_info: &WindowInfo,` | **window already in scope here** |
| 386  | `state.pending = None;` | immediate branch clears pending (type-agnostic) |
| 389  | `state.pending = Some(message.clone());` | **debounce-queue store** |
| ~412 | `let _res = notifier.notify(message);` | immediate send (window_info already in scope) |
| 438  | `use crate::core::types::WindowInfo;` | re-import in `mod tests` |

`src/core/types.rs`:

```rust
#[derive(Debug, PartialEq)]          // line 1 — needs Clone added
pub struct WindowInfo {              // line 2
    pub app_class: String,
    pub title: String,
}
impl WindowInfo {
    pub fn new(app_class: String, title: String) -> Self { Self { app_class, title } }
}
```

---

## §2 — The five `.pending` touch sites (only TWO need real edits)

`grep -n "\.pending" src/core/notifier.rs`:

1. **~L290** `while state.pending.is_none()` — `Option::is_none()` is type-agnostic.
   **NO CHANGE.**
2. **~L300** `let msg = state.pending.take().unwrap();` — `msg` becomes
   `PendingMessage`. Rename `msg`→`pm`; the flush tuple type at L295 changes
   `String`→`PendingMessage`; the send-block destructure at ~L311 changes.
   **EDIT (mechanical).**
3. **L386** `state.pending = None;` (immediate branch) — `None` fits any
   `Option<T>`. **NO CHANGE.**
4. **L389** `state.pending = Some(message.clone());` — becomes
   `Some(PendingMessage { payload: message.clone(), window_info: window_info.clone() })`.
   **EDIT.**
5. **~L506** `state.pending = None;` (`reset_test_state`) — type-agnostic `None`.
   **NO CHANGE.**

So the contract's *"Ensure `reset_test_state()` is updated for the new pending
type"* is satisfied by **verification, not a code edit** — `= None` already
type-checks for `Option<PendingMessage>`. Forcing an edit here would be a no-op.
Document this so the implementer doesn't "fix" a non-issue.

---

## §3 — `WindowInfo` Clone safety (the one `types.rs` change)

`notify_qmk` takes `window_info: &WindowInfo`. To store an **owned** copy inside
`PendingMessage` (which lives in process-global `STATE` beyond the borrow),
`WindowInfo` must be `Clone`. Today it derives only `Debug, PartialEq`.

Adding `Clone` is **purely additive** (more capability, none removed). `String: Clone`,
so the derived impl is sound. Blast radius is zero:

- `grep WindowInfo src/**` shows all call sites either construct via
  `WindowInfo::new(s.clone(), s.clone())` or borrow `&WindowInfo`. None rely on
  WindowInfo being non-`Clone`.
- Several sites already hand-clone field-by-field (`windows.rs:165`,
  `x11.rs:131`, `hyprland.rs:439`) — they keep working; a future task *could* use
  `.clone()` on the whole struct, but that's out of scope here.
- The `PartialEq` test in `types.rs` (`test_window_info_equality`) is unaffected.

**Edit:** `src/core/types.rs:1` → `#[derive(Debug, Clone, PartialEq)]`.

> Optional: also derive `Eq` (String: Eq). NOT required by this task — keep the
> diff minimal (add `Clone` only). P4.M3.T1.S1 may add `Eq` if it needs to key a
> `HashMap<WindowInfo, _>`; leave that to it.

---

## §4 — `notify_qmk` signature is stable (7 callers, none touched)

`grep -rn "notify_qmk" src --include="*.rs" | grep -v notifier.rs`:

```
src/platforms/hyprland.rs   (x3)   if let Err(e) = notifier::notify_qmk(&window_info, verbose)
src/platforms/macos.rs      (x2)   let _ = notifier::notify_qmk(&window_info, verbose)
src/platforms/x11.rs        (x2)   if let Err(e) = notifier::notify_qmk(&window_info, verbose)
src/platforms/windows.rs    (x1)   if let Err(e) = notifier::notify_qmk(&window_info, verbose)
```

All 7 pass `(&WindowInfo, bool)`. **This task does NOT change the signature**, so
zero caller edits. The change is purely internal state plumbing. ✓

---

## §5 — The send-block threading design (worker vs immediate)

There are **two** send blocks today (both call `notifier.notify(string)`):

- **Worker** (`debounce_worker`, ~L311-340): flushes the queued `pending`. It does
  NOT currently have the window — it only has the formatted string. **This is the
  block that must gain `WindowInfo`** (carried via `pending`).
- **Immediate** (`notify_qmk`, ~L393-416): runs when the debounce window has
  elapsed. It ALREADY has `window_info` in scope (it's the `notify_qmk` param) —
  so the immediate block needs **no change** for window accessibility. P4.M3.T1.S1
  will extend BOTH blocks identically; the immediate one already has what it needs.

### Chosen worker edit (lint-clean, no underscore ceremony)

Use a **partial move** out of the flushed `PendingMessage` — moves `payload` into
the existing `message` local (sent unchanged), leaves `pm.window_info` discoverable
in the send block for P4.M3.T1.S1:

```rust
// L295
let mut to_send: Option<(PendingMessage, bool)> = None;   // was (String, bool)
...
// ~L300
let pm = state.pending.take().unwrap();                    // was: msg
let verbose = state.verbose;
state.last_sent_time = Some(Instant::now());
to_send = Some((pm, verbose));                             // was: (msg, verbose)
...
// ~L311
if let Some((pm, verbose)) = to_send {                     // was: (message, verbose)
    // `pm` carries the formatted payload (sent below) AND the originating
    // WindowInfo. P4.M3.T1.S1 consumes `pm.window_info` here to evaluate
    // rules.toml and emit APPLY_HOST_CONTEXT alongside the string send.
    let message = pm.payload;                               // partial move -> String
    ... // rest of send block UNCHANGED (uses `message`, calls notify(message) ~L329)
}
```

**Why partial move, not destructure:** `let PendingMessage { payload: message,
window_info: _window_info } = pm;` also works and is lint-clean (leading `_`
suppresses `unused_variables`), but the partial-move form is more idiomatic for
"use this field now, leave the rest for the next task" and makes `pm.window_info`
a discoverable, named seam. Either is acceptable; partial move is primary.

### Why NOT extract a shared `send_with_context` helper now

The two send blocks differ in control flow (worker logs+continues on `Err`;
immediate propagates with `?`) and in verbose label (`debounced` vs `immediate`).
Extracting a helper is a real refactor with its own risk and is **not required** by
the contract ("pass both payload and window_info to the send logic" = make window
available in the send block, which the partial move does). P4.M3.T1.S1 — which
actually adds identical logic to both blocks — is the natural moment to decide on
a helper (concrete duplication motivation). **Keep this task minimal.**

---

## §6 — Test design + worker-timing safety proof

### What existing tests cover (no change needed)

The 6 debounce tests assert `MOCK_LAST_MESSAGE` (the formatted string) and
`MOCK_CALL_COUNT`. Because we keep calling `notifier.notify(payload)` with the
identical string, **all existing assertions still hold** — they prove the payload
threading survived the type change. The debounce-timing behavior is unchanged.

### What they DON'T cover → the new test

None assert that `WindowInfo` is carried (it's not observable in the send yet —
P4.M3.T1.S1 makes it observable). Add a **white-box** test that inspects
`STATE.pending` after a debounced queue:

```rust
#[test]
fn test_debounced_pending_carries_window_info() {
    reset_test_state();
    set_notifier(Box::new(MockNotifier::new()));
    // Long window: the worker will NOT flush while we inspect `pending`.
    STATE.lock().unwrap().interval = Duration::from_secs(10);

    // Prime last_sent_time with an immediate send (so the next call debounces).
    let _ = notify_qmk(&WindowInfo::new("App1".into(), "Title1".into()), false);
    assert!(wait_for_count(1, Duration::from_millis(500)));

    // Second call inside the window -> queued as PendingMessage.
    let w2 = WindowInfo::new("App2".into(), "Title2".into());
    let _ = notify_qmk(&w2, false);

    // White-box: pending now carries BOTH the formatted payload AND the WindowInfo.
    let snap = {
        let st = STATE.lock().unwrap();
        st.pending.as_ref().map(|p| (p.payload.clone(), p.window_info.clone()))
    };
    let (payload, wi) = snap.expect("pending should hold the queued message");
    assert_eq!(payload, "App2\x1DTitle2");
    assert_eq!(wi, w2);
    // No manual cleanup: the NEXT test's reset_test_state() clears pending safely
    // (see §6.1 — it also nulls last_sent_time, keeping the worker's flush target
    // always-future until a real message arrives).
}
```

Accessibility: `PendingMessage` + its fields are private to `notifier`'s module,
but `mod tests` is a **child** module, so it can read private parent items (the
existing tests already read `state.pending`, `state.last_sent_time` the same way).
`WindowInfo: Clone + PartialEq` (after §3) ⇒ `assert_eq!(wi, w2)` compiles.

### §6.1 — Why leaving `pending = Some(..)` is safe (no manual drain needed)

The worker's inner loop is:

```rust
while to_send.is_none() {
    let last = state.last_sent_time.unwrap_or_else(Instant::now);  // <-- KEY
    let target = last + state.interval;
    let now = Instant::now();
    if now >= target {
        let msg = state.pending.take().unwrap();   // would panic if pending is None
        ...
    } else {
        state = COND.wait_timeout(state, target - now).unwrap().0;
    }
}
```

The panic site (`take().unwrap()` on `None`) is only reached when `now >= target`.
`reset_test_state()` sets **`last_sent_time = None`**, so on the next iteration
`last = Instant::now()` (fresh) ⇒ `target = now + interval` ⇒ `now < target`
**always** ⇒ the `take()` arm is never taken while pending is `None`. The worker
harmlessly `wait_timeout`s until a real message arrives. **Therefore** a test may
leave `pending = Some(..)` + a long interval; the next `reset_test_state()`
clears it without racing the worker. (Verified against the existing suite, which
relies on exactly this property.)

---

## §7 — Validation approach (project dev loop, AGENTS.md)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect                      # clean, no NEW warnings (see §8 caveat)
cargo test --bin qmkonnect -- --test-threads=1   # MANDATORY single-threaded (global STATE/COND/WORKER)
cargo clippy --bin qmkonnect --no-deps 2>/dev/null || true   # optional; no NEW warnings
git diff --stat                                  # expect ONLY src/core/types.rs + src/core/notifier.rs
```

`--test-threads=1` is **mandatory** crate-wide (AGENTS.md; the debouncer is
process-global mutable state shared across tests). Every test begins with
`reset_test_state()`. Never run the suite multi-threaded.

The new test + all 6 existing debounce tests + the 5 `test_send_command_*`
(P4.M1.T1.S1) + pattern (P2) + rules (P3) + types tests must all pass — **once
the crate compiles end-to-end** (see §8 for the pin caveat).

---

## §8 — Relationship to the parallel pin-flip P4.M1.T2.S1 (IMPORTANT validation caveat)

P4.M1.T2.S1 flips `Cargo.toml`'s `qmk_notifier` pin `v0.2.1`→`v0.3.0` + regens
`Cargo.lock`. **At research time `Cargo.toml:16` STILL reads `tag = "v0.2.1"`** —
i.e. S1 is in-flight (plan_status: "Implementing") and has NOT landed in this
tree yet. (P1.M1.T4.S1 — the crate v0.3.0 tag release — is Complete, so the
`v0.3.0` git tag IS pushed and resolvable; only the qmkonnect-side pin flip is
pending.)

### Why this does NOT block this task's edits

This task's edits (`WindowInfo` Clone in types.rs; `PendingMessage` +
`DebounceState.pending` in notifier.rs) are **pure qmkonnect-internal Rust** and
reference **no `qmk_notifier::` type**. They are type-correct regardless of the
crate pin version. There is **no file overlap** with S1 (it edits
`Cargo.toml`+`Cargo.lock`; this task edits `src/core/{types,notifier}.rs`). Either
may land first / in parallel.

### The validation caveat the implementer MUST understand

Because `src/core/notifier.rs` ALREADY contains P4.M1.T1.S1's `send_command` code
(the `Notifier::send_command` trait method + `QmkNotifier`/`MockNotifier` impls +
5 `test_send_command_*` tests) coded against qmk_notifier **v0.3.0** types
(`RunCommand`/`CommandResponse`/`HostOs`), while the pin is still **v0.2.1**:

- **`cargo build --bin qmkonnect` in the current tree will FAIL** — but ONLY on
  the pre-existing `send_command`/qmk_notifier v0.3.0 references, NOT on this
  task's edits. Those errors are S1's in-flight work, not a defect introduced here.
- **`cargo test` cannot compile the test binary** for the same reason, so the new
  `test_debounced_pending_carries_window_info` cannot be *run* in isolation until
  the pin flips.

### How to validate THIS task correctly

1. Make the src/ edits (Tasks 1-6).
2. Run `cargo build --bin qmkonnect`. Expected: errors ONLY about
   `send_command` / `RunCommand` / `CommandResponse` / `HostOs` / `qmk_notifier`
   (the pre-existing pin gap). **Confirm there are NO errors mentioning
   `PendingMessage`, `DebounceState`, `WindowInfo`, or `Clone`** — their absence
   proves this task's edits are type-correct. (Grep the build output:
   `cargo build 2>&1 | grep -iE 'PendingMessage|DebounceState|WindowInfo|Clone'`
   → expect empty.)
3. For the FULL green validation (`cargo build` clean + `cargo test` green,
   including the new test), the **v0.3.0 pin must be in place**. Two paths:
   - **PREFERRED:** land S1 (the pin flip) first or alongside; then `cargo build`
     + `cargo test --bin qmkonnect -- --test-threads=1` are green.
   - **LOCAL-VALIDATION FALLBACK** (do NOT commit this — it's S1's file): temporarily
     flip `Cargo.toml:16` to `tag = "v0.3.0"` + `cargo update -p qmk_notifier`,
     run build+test, confirm green, then **revert Cargo.toml/Cargo.lock** so this
     task's commit contains ONLY `src/core/{types,notifier}.rs` (the committed
     Cargo.toml pin change belongs to S1).
4. `git diff --stat` must show ONLY `src/core/types.rs` + `src/core/notifier.rs`.

> Net: this task is self-contained and correct; its *full* build/test validation
> is gated on the sibling pin flip (S1) landing — which is expected, since both
> are the two subtasks of parent **P4.M1.T2 "Pin qmk_notifier v0.3.0 and Extend
> DebounceState"** and are designed to land together.

---

## §9 — Anti-patterns / pitfalls checklist

- ❌ Do NOT add a second `use crate::core::types::WindowInfo;` — it's at L1 already
  (and L438 in `mod tests`). A duplicate import is a hard compile error.
- ❌ Do NOT change `notify_qmk`'s signature — 7 platform callers depend on it.
- ❌ Do NOT compute `HostContext` / call `send_command` here — that's P4.M3.T1.S1.
  This task only carries `WindowInfo` to the send block.
- ❌ Do NOT run tests multi-threaded — `--test-threads=1` is mandatory.
- ❌ Do NOT edit `reset_test_state()`'s `state.pending = None;` — it's type-agnostic
  and already correct for `Option<PendingMessage>`. "Ensure updated" = verify, not edit.
- ❌ Do NOT leave `window_info` bound-but-unused in the worker send block (rustc warns).
  Use the partial-move form (`let message = pm.payload;`) OR underscore-prefix.
- ❌ Do NOT commit a `Cargo.toml`/`Cargo.lock` pin change here — that's S1's file
  (§8). Use the local-validation fallback only transiently and revert before commit.
- ❌ Do NOT mistake the pre-existing `send_command`/qmk_notifier build errors (pin
  still v0.2.1) for defects in this task's edits (§8). Confirm via the §8 grep.
- ✅ DO keep `notifier.notify(message)` called with the IDENTICAL formatted string
  — existing tests assert on it; the payload bytes must not change.
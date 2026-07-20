# PRP — P4.M1.T2.S2: Extend `DebounceState` to carry `WindowInfo` for host-context evaluation

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This 2-point plumbing task widens the
> process-global debouncer's queued payload from `Option<String>` to
> `Option<PendingMessage>` where `struct PendingMessage { payload: String,
> window_info: WindowInfo }`, and threads that `WindowInfo` into the debounce
> worker's send block — so **P4.M3.T1.S1** (the host-context send) has the
> originating window in hand at the exact point it must evaluate `rules.toml`
> and emit `APPLY_HOST_CONTEXT`. The legacy string path (`notify_qmk`→`notify`)
> is byte-for-byte preserved; all existing debounce tests stay green.
>
> **Consumes:** `WindowInfo` (`src/core/types.rs`) + the existing `DebounceState`
> / `debounce_worker` / `notify_qmk` machinery in `src/core/notifier.rs`.
> **Consumed downstream by:** **P4.M3.T1.S1** ("Implement host-context
> evaluation and send in debounce worker") — its INPUT is precisely "the
> `WindowInfo` this task carries to the send block" (see the `HostContext`
> doc-comment in `src/core/rules.rs`: *the single packet the `notify_qmk` send
> logic (P4.M3.T1.S1) consumes*).
>
> **⚠️ PARALLEL-TASK CAVEAT (read before validating — see §Validation):** the
> sibling pin-flip **P4.M1.T2.S1** (`Cargo.toml` qmk_notifier `v0.2.1`→`v0.3.0`)
> is in-flight and **NOT yet in this tree** — `Cargo.toml:16` currently still
> reads `tag = "v0.2.1"`. Because `notifier.rs` already contains P4.M1.T1.S1's
> v0.3.0 `send_command` code, `cargo build`/`cargo test` will fail on those
> **pre-existing** errors until S1 lands. THIS task's edits are pure
> qmkonnect-internal Rust (no `qmk_notifier::` types), touch ONLY
> `src/core/{types,notifier}.rs`, and introduce **zero** new compile errors —
> full green validation is gated on S1's pin flip (both are subtasks of parent
> P4.M1.T2, designed to land together).

---

## Goal

**Feature Goal**: Carry the originating `WindowInfo` through the debounce
pipeline (`notify_qmk` → `DebounceState.pending` → `debounce_worker` send block)
alongside the formatted string, so the host-side-rules send (P4.M3.T1.S1) can
evaluate `rules.toml` against the window at flush time — without altering the
existing string-send bytes, the `notify_qmk` signature, or the debounce timing.

**Deliverable** (two files; **NO Cargo, no new files, no CLI/tray**):
1. **`src/core/types.rs:1`** — add `Clone` to `WindowInfo`'s derive list.
2. **`src/core/notifier.rs`** — add a private `PendingMessage { payload, window_info }`
   struct; widen `DebounceState.pending` to `Option<PendingMessage>`; store a
   `PendingMessage` in the debounce-queue branch; flush it through the worker and
   partial-move `payload` into the existing send block (leaving `pm.window_info`
   as the named seam for P4.M3.T1.S1); add one white-box test asserting the carry.

**Success Definition**:
- This task's edits introduce **zero new compile errors** (verified by grepping
  the build output for `PendingMessage`/`DebounceState`/`WindowInfo`/`Clone` →
  empty; the only build errors are the pre-existing `send_command`/qmk_notifier
  ones from the unresolved pin — see §Validation).
- Once the v0.3.0 pin is in place (S1 landed, or the local-validation fallback),
  `cargo build --bin qmkonnect` compiles **clean** (zero errors, zero NEW
  warnings — in particular no `unused variable: window_info`).
- Once the pin is in place, `cargo test --bin qmkonnect -- --test-threads=1` is
  **green**: the new `test_debounced_pending_carries_window_info` passes AND all
  6 existing debounce tests + the 5 `test_send_command_*` (P4.M1.T1.S1) +
  pattern/rules/types tests are unchanged (the formatted string sent to the
  device is identical).
- `git diff --stat` shows **exactly** `src/core/types.rs` + `src/core/notifier.rs`
  (NO `Cargo.toml`/`Cargo.lock` — those belong to S1).
- `notify_qmk(&WindowInfo, bool)` signature is unchanged (7 platform callers
  untouched).
- `DebounceState.pending` is `Option<PendingMessage>`; the worker send block has
  `pm.window_info` (or an equivalent binding) available at the point
  `notifier.notify(message)` is called — the seam P4.M3.T1.S1 will consume.

## User Persona (if applicable)

**Target User**: the **P4.M3.T1.S1 implementer** (the direct downstream
consumer). Secondary: anyone reasoning about debounce correctness.

**Use Case**: "When a window change is debounced and finally flushed, the send
step has both the formatted title string AND the structured `WindowInfo`, so it
can (a) send the legacy string and (b) compute + send the host context — in one
flush, ≤2 sends per window change (PRD §5.7)."

**Pain Points Addressed**: today the worker only knows the *formatted string*,
so it cannot run rules evaluation (which needs `app_class`/`title` separately,
not the `\x1D`-joined blob). This task removes that blockage without changing
any observable behavior.

## Why

- **PRD §5.7 (`h3.16`)** — *"the host-context send happens within the same
  debounced 'send' step (one window change ⇒ ≤2 sends: string + context, or
  context-only in replace mode)."* For that to be possible, the send step must
  have the window — which requires `pending` to carry it. This task is that
  prerequisite plumbing.
- **PRD §5 (`h2.18`)** / **§5.3 (`h3.12`)** — the debouncer's correctness
  property (one immediate + ≤1 follow-up of the final value) is preserved: only
  the *type* of the queued value changes, never the algorithm.
- **`src/core/rules.rs` (`HostContext` doc-comment)** documents the downstream
  contract: the `notify_qmk` send logic (P4.M3.T1.S1) consumes a `HostContext`
  packet derived from the window. This task delivers the window to that logic.
- **Cohesion**: P3.M1 (rules evaluator) and P2.M1 (pattern matcher) are complete;
  P4.M1.T1.S1 (`send_command` transport) is done. The only missing link between
  "a window change is observed" and "the host context is sent" is that the
  debouncer drops the window. This task restores it.

## What

A minimal, mechanical widening of the debouncer's queued payload. No new
behavior is visible to the device or to callers: `notify()` still receives the
exact same formatted string at the exact same cadence. The only observable
difference is internal — `DebounceState.pending` now also holds the `WindowInfo`,
and the worker's send block names it (`pm.window_info`) for the next task.

### Success Criteria
- [ ] `src/core/types.rs:1` derives `Clone` on `WindowInfo` (`#[derive(Debug, Clone, PartialEq)]`).
- [ ] `src/core/notifier.rs` defines `struct PendingMessage { payload: String, window_info: WindowInfo }` (private, module-level, beside `DebounceState`).
- [ ] `DebounceState.pending` is `Option<PendingMessage>` (was `Option<String>`).
- [ ] `notify_qmk`'s debounce-queue branch stores a `PendingMessage` (`payload: message.clone(), window_info: window_info.clone()`).
- [ ] `debounce_worker` flushes `(PendingMessage, bool)` and the send block has `pm.window_info` available where `notifier.notify(message)` is called.
- [ ] `notifier.notify(message)` is still called with the IDENTICAL formatted string in BOTH send paths (no byte change).
- [ ] New test `test_debounced_pending_carries_window_info` passes (white-box: `STATE.pending` holds the right `(payload, window_info)` after a debounced queue).
- [ ] This task's edits introduce ZERO new compile errors (the pre-existing `send_command`/qmk_notifier errors from the unresolved pin are NOT this task's — §Validation).
- [ ] `git diff --stat` = `src/core/types.rs` + `src/core/notifier.rs` ONLY (NO Cargo files — those are S1's).

## All Needed Context

### Context Completeness Check

_Pass_: an agent with no prior knowledge of this codebase can implement this
using only this PRP + codebase access, because: (a) the EXACT current anchors
(verbatim code + line numbers) for every touch site are tabulated in
`research/notes.md` §1 (grep-confirmed this session); (b) the five `.pending`
sites are classified into "EDIT (2)" vs "NO CHANGE (3 — type-agnostic)" in §2,
so the implementer knows precisely which lines to touch and which to leave alone
(notably `reset_test_state`'s `state.pending = None` needs NO edit — verified);
(c) the single `types.rs` change (`+Clone`) is justified and blast-radius-proven
in §3 (additive, `String: Clone`, no caller relies on non-Clone); (d) the chosen
worker edit is given as a concrete code block (partial-move form, lint-clean)
with the destructure alternative noted; (e) the `notify_qmk` signature is proven
stable (7 callers enumerated in §4) so the implementer won't touch it; (f) the
new test is given verbatim with a proof that leaving `pending=Some(..)` is safe
across tests (§6.1 — `reset_test_state` nulls `last_sent_time`, keeping the
worker's flush target always-future); (g) the validation commands are the
project's documented dev loop (`cargo build`, `cargo test --bin qmkonnect --
--test-threads=1` — single-threaded MANDATORY per AGENTS.md); (h) the
**critical pin-state caveat** is fully explained in §8 + §Validation — the
implementer knows exactly which build errors are pre-existing (S1's) vs. caused
by this task (none), and how to get a full green run without committing S1's files.

### Documentation & References

```yaml
# MUST READ — the verbatim research (THIS task's full contract + design + safety proofs)
- file: plan/002_637d65b6e9b8/P4M1T2S2/research/notes.md
  why: "§1 = exact current anchors (line numbers + verbatim code for every touch site).
        §2 = the 5 .pending sites classified EDIT(2) vs NO-CHANGE(3, type-agnostic) —
        prevents editing reset_test_state's `= None` (already correct). §3 = the one
        types.rs change (+Clone) + blast-radius proof. §4 = notify_qmk signature
        stable (7 callers). §5 = the worker send-block threading design (partial-move,
        lint-clean) with the destructure alternative. §6 = the new test (verbatim) +
        §6.1 proof that leaving pending=Some is safe across tests. §7 = validation.
        §8 = THE PIN-STATE CAVEAT (Cargo.toml still v0.2.1; build/test gated on S1).
        §9 = anti-patterns checklist."

# MUST READ — the spec sources of truth (selected sections are in this PRP's header)
- file: PRD.md   # (or merged prd_snapshot)
  why: "§5.7 (h3.16) — 'the host-context send happens within the same debounced send
        step (<=2 sends: string+context, or context-only)'. §5.3 (h3.12) — the debounce
        correctness property (one immediate + <=1 follow-up) that MUST be preserved.
        §5 (h2.18) — the notifier pipeline overview."
  section: "## 5. The Notification Pipeline & Debouncer  +  ### 5.3 / ### 5.7"

# MUST READ — the file THIS task edits (the debouncer)
- file: src/core/notifier.rs
  why: "contains DebounceState (L254), debounce_worker (~L283), notify_qmk (L366),
        reset_test_state (~L499). WindowInfo is ALREADY imported at L1 (do NOT re-add).
        The 5 .pending sites: ~L290 (is_none), ~L300 (take), L386 (= None immediate),
        L389 (= Some queue), ~L506 (= None reset)."
  pattern: "widen pending: Option<String> -> Option<PendingMessage>; thread the
            PendingMessage through the worker flush (L295/~L300/~L311) via partial move;
            store PendingMessage in the queue branch (L389)."
  gotcha: "~L290 (is_none), L386 (= None, immediate branch), ~L506 (= None, reset) are
           type-agnostic — DO NOT edit them. notify_qmk's signature (L366-368) is
           stable; do NOT change it. The file ALREADY contains P4.M1.T1.S1's send_command
           code (v0.3.0 types) — pre-existing build errors from the unresolved v0.2.1 pin
           are NOT this task's concern (notes §8)."

# MUST READ — the other file THIS task edits (the window type)
- file: src/core/types.rs
  why: "WindowInfo derive (L1) needs +Clone so notify_qmk(&WindowInfo) can store an
        owned copy in PendingMessage (which outlives the borrow in process-global STATE)."
  pattern: "#[derive(Debug, PartialEq)] -> #[derive(Debug, Clone, PartialEq)] (L1)."
  gotcha: "do NOT add Eq/Default/etc. unless needed — keep the diff to +Clone (P4.M3.T1.S1
           may add Eq later if it keys a HashMap). Adding Clone is additive; no caller breaks."

# MUST READ — the downstream consumer's contract (why this task exists)
- file: src/core/rules.rs
  why: "the HostContext doc-comment (~L260-272): HostContext is 'the single packet the
        notify_qmk send logic (P4.M3.T1.S1) consumes' — confirms the send step (where
        this task delivers window_info) is exactly where P4.M3.T1.S1 will compute+send
        APPLY_HOST_CONTEXT."
  section: "the `HostContext` doc-comment (~L260-272)"

# Reference — the predecessor that added the typed transport (no conflict)
- file: plan/002_637d65b6e9b8/P4M1T1S1/PRP.md
  why: "P4.M1.T1.S1 added send_command() to the Notifier trait + impls (already in
        notifier.rs, coded against v0.3.0). This task does NOT touch send_command/the
        trait; it only widens DebounceState. Read it to confirm there's no overlap.
        NOTE: P4.M1.T1.S1's code is the SOURCE of the pre-existing build errors while
        the pin is still v0.2.1 (notes §8)."

# MUST READ — the parallel pin-flip (the validation-gate sibling; NO file conflict)
- file: plan/002_637d65b6e9b8/P4M1T2S1/PRP.md
  why: "P4.M1.T2.S1 flips Cargo.toml qmk_notifier v0.2.1->v0.3.0 + regens Cargo.lock.
        It edits Cargo.toml + Cargo.lock ONLY; this task edits src/ ONLY. No overlap,
        either may land first. THIS task's full build/test validation is GATED on S1
        landing (or the local-validation fallback in notes §8) because notifier.rs
        already holds v0.3.0 send_command code against a still-v0.2.1 pin."
```

### Current Codebase tree (relevant subset)

```bash
src/core/
  types.rs        # L1: #[derive(Debug, PartialEq)] WindowInfo  -> +Clone (THIS TASK)
  notifier.rs     # L1:  use crate::core::types::WindowInfo;   (already present)
                  #      ALSO already contains P4.M1.T1.S1's send_command (v0.3.0 types)
                  #      -> pre-existing build errors while pin is v0.2.1 (NOT this task)
                  # L254: struct DebounceState { ... pending: Option<String> ... }  (WIDEN)
                  # ~L283: fn debounce_worker()  (THREAD PendingMessage through flush)
                  # L366: pub fn notify_qmk(&WindowInfo, bool)  (store PendingMessage in queue branch)
                  # ~L499: fn reset_test_state()  (NO edit — pending=None is type-agnostic)
                  # mod tests: 6 debounce tests + 5 send_command tests  (ADD 1 test)
  rules.rs        # P3.M1 — HostContext evaluator. UNCHANGED. (~L260-272 = downstream contract)
  pattern.rs      # P2.M1 — matcher. UNCHANGED.
  mod.rs          # UNCHANGED.
src/platforms/    # hyprland/macos/x11/windows — 7 callers of notify_qmk. UNCHANGED (signature stable).
Cargo.toml        # L16: qmk_notifier tag="v0.2.1" (STILL v0.2.1 — S1 owns the flip; UNCHANGED by this task).
```

### Desired Codebase tree with files to be changed

```bash
src/core/
  types.rs        # MODIFIED — L1 derive: +Clone.
  notifier.rs     # MODIFIED — +struct PendingMessage; pending: Option<PendingMessage>;
                  #   worker flush threads PendingMessage (partial-move payload in send block);
                  #   notify_qmk queue branch stores PendingMessage; +1 test.
# EVERYTHING ELSE UNCHANGED. No Cargo.toml/Cargo.lock (S1's files), no new files,
# no platforms/, no CLI/tray.
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (G1): WindowInfo is ALREADY imported in notifier.rs (L1) AND in mod tests (L438).
//   Do NOT add a second `use crate::core::types::WindowInfo;` — a duplicate import is a
//   hard compile error. The contract's "Add use ...WindowInfo" step is ALREADY satisfied.

// CRITICAL (G2): do NOT change notify_qmk's signature (&WindowInfo, bool). 7 platform
//   callers depend on it (hyprland x3, macos x2, x11 x2, windows x1). This task is purely
//   internal state plumbing.

// CRITICAL (G3): keep notifier.notify(message) called with the IDENTICAL formatted string.
//   The 6 existing debounce tests assert MOCK_LAST_MESSAGE == "<app>\x1D<title>". Do not
//   reformat, reorder, or drop the payload. The payload BYTES must not change.

// CRITICAL (G4 — THE PIN-STATE CAVEAT): Cargo.toml:16 currently pins qmk_notifier v0.2.1.
//   notifier.rs ALREADY contains P4.M1.T1.S1's send_command code (v0.3.0 types), so
//   `cargo build`/`cargo test` in the current tree FAIL on pre-existing send_command /
//   RunCommand / CommandResponse / HostOs errors — these are S1's in-flight pin-flip work,
//   NOT defects from this task. Verify this task's edits add ZERO new errors: grep the build
//   output for PendingMessage|DebounceState|WindowInfo|Clone (expect EMPTY). Do NOT commit
//   a Cargo.toml/Cargo.lock change here — that's S1's file (notes §8).

// GOTCHA (G5): reset_test_state()'s `state.pending = None;` (~L506) and the immediate
//   branch's `state.pending = None;` (L386) and the worker's `while state.pending.is_none()`
//   (~L290) are all TYPE-AGNOSTIC — they compile unchanged for Option<PendingMessage>. The
//   contract's "ensure reset_test_state updated" means VERIFY (it already type-checks),
//   NOT force an edit. Editing them is a no-op at best.

// GOTCHA (G6): in the worker send block, do NOT leave `window_info` bound-but-unused
//   (rustc warns `unused variable`). Use the partial-move form `let message = pm.payload;`
//   (leaves pm.window_info discoverable, no warning) OR destructure with a `_`-prefixed
//   name (`window_info: _window_info`). The partial-move form is preferred (cleaner seam).

// GOTCHA (G7): --test-threads=1 is MANDATORY (AGENTS.md). The debouncer is process-global
//   mutable STATE/COND/WORKER shared across tests; multi-threaded runs race. Every test
//   begins with reset_test_state(). Never run the suite without the flag.

// GOTCHA (G8): do NOT compute HostContext / call send_command / evaluate rules here — that
//   is P4.M3.T1.S1. This task ONLY carries WindowInfo to the send block. Adding rule
//   evaluation now would collide with P4.M3.T1.S1's scope and risk a merge conflict.

// GOTCHA (G9): WindowInfo needs Clone (the types.rs edit) because notify_qmk takes
//   &WindowInfo but PendingMessage (in process-global STATE) must OWN its copy. String is
//   Clone, so #[derive(Clone)] is sound. Do NOT reach for a lifetime parameter on
//   DebounceState (it's a 'static Lazy<Mutex<..>> — a borrow would not outlive it).
```

## Implementation Blueprint

### Data models and structure

```rust
// src/core/types.rs  (line 1 — add Clone)
#[derive(Debug, Clone, PartialEq)]
pub struct WindowInfo {
    pub app_class: String,
    pub title: String,
}

// src/core/notifier.rs  (new private struct, placed just ABOVE `struct DebounceState` at L254)
/// A debounced window-change message awaiting its flush: the formatted string
/// payload (sent as the legacy `notify` string) together with the originating
/// [`WindowInfo`]. The window is carried so the host-side-rules send
/// (P4.M3.T1.S1) can evaluate `rules.toml` and emit `APPLY_HOST_CONTEXT` at
/// flush time — without it the worker would only know the `\x1D`-joined blob.
struct PendingMessage {
    payload: String,
    window_info: WindowInfo,
}

// src/core/notifier.rs  (DebounceState.pending widened — L258)
struct DebounceState {
    last_sent_time: Option<Instant>,
    pending: Option<PendingMessage>,   // was: Option<String>
    verbose: bool,
    interval: Duration,
}
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: ADD Clone to WindowInfo (src/core/types.rs:1)
  - EDIT: change `#[derive(Debug, PartialEq)]` -> `#[derive(Debug, Clone, PartialEq)]`.
  - WHY: notify_qmk takes &WindowInfo; PendingMessage (in process-global STATE) must OWN
    its copy -> requires WindowInfo: Clone. String is Clone, so the derived impl is sound.
  - GOTCHA G9: do NOT add Eq/Default/Copy. Keep the diff to +Clone (P4.M3.T1.S1 may add
    Eq later). Additive only — no caller relies on WindowInfo being non-Clone (notes §3).
  - VERIFY: `grep -n 'derive' src/core/types.rs` shows the new derive list.

Task 2: ADD PendingMessage struct + WIDEN pending type (src/core/notifier.rs)
  - ADD: just ABOVE `struct DebounceState {` (L254), insert the `struct PendingMessage
    { payload: String, window_info: WindowInfo }` block shown in Data Models (with its
    doc-comment). Private (no `pub`) — only notifier.rs + its child `mod tests` use it.
  - EDIT L258: `pending: Option<String>,` -> `pending: Option<PendingMessage>,`.
  - DEPENDENCY: WindowInfo must be in scope — it ALREADY is (L1 `use crate::core::types::WindowInfo;`).
    Do NOT re-add the import (G1).
  - GOTCHA G1: no duplicate `use` statement.
  - VERIFY: `grep -n 'struct PendingMessage\|pending: Option<PendingMessage>' src/core/notifier.rs`
    -> both present (1 each).

Task 3: THREAD PendingMessage through the debounce_worker flush + send block (src/core/notifier.rs)
  - EDIT L295: `let mut to_send: Option<(String, bool)> = None;`
        ->  `let mut to_send: Option<(PendingMessage, bool)> = None;`
  - EDIT ~L300: `let msg = state.pending.take().unwrap();`  ->  `let pm = state.pending.take().unwrap();`
  - EDIT the next line: `to_send = Some((msg, verbose));`  ->  `to_send = Some((pm, verbose));`
  - EDIT ~L311: `if let Some((message, verbose)) = to_send {`
        ->  `if let Some((pm, verbose)) = to_send {`
            followed immediately (first line inside the block) by:
              // `pm` carries the formatted payload (sent below) AND the originating
              // WindowInfo. P4.M3.T1.S1 consumes `pm.window_info` here to evaluate
              // rules.toml and emit APPLY_HOST_CONTEXT alongside the string send.
              let message = pm.payload;   // partial move -> String; pm.window_info remains for P4.M3.T1.S1
  - KEEP the rest of the send block (~L312-340) BYTE-FOR-BYTE unchanged: it uses `message`
    (now the moved payload) in the verbose log, the #[cfg(test)] println, and
    `notifier.notify(message)` (~L329). The payload string is identical (G3).
  - WHY partial move: lint-clean (no `unused variable` warning — G6), and `pm.window_info`
    is a discoverable named seam for P4.M3.T1.S1. Alternative (also acceptable):
    `let PendingMessage { payload: message, window_info: _window_info } = pm;`.
  - GOTCHA G6: do NOT bind `window_info` without using it (rustc warns). The partial move
    avoids this; if you destructure, underscore-prefix the unused field.
  - VERIFY: `grep -n 'let pm = state.pending.take\|Option<(PendingMessage, bool)>\|let message = pm.payload' src/core/notifier.rs`
    -> 3 hits.

Task 4: STORE PendingMessage in the notify_qmk debounce-queue branch (src/core/notifier.rs:389)
  - EDIT L389: `state.pending = Some(message.clone());`
        ->  `state.pending = Some(PendingMessage {`
            `    payload: message.clone(),`
            `    window_info: window_info.clone(),`
            `});`
  - WHY: window_info is the notify_qmk param (L367, &WindowInfo); .clone() owns it for STATE.
  - KEEP L386 (`state.pending = None;` in the immediate/due branch) UNCHANGED — it is the
    correct "clear pending, we're sending now" and is type-agnostic (G5).
  - GOTCHA G3: the formatted `message` string is UNCHANGED — `notify` still gets "<app>\x1D<title>".
  - VERIFY: `grep -n 'PendingMessage { payload: message.clone()' src/core/notifier.rs` -> 1 hit.

Task 5: VERIFY the three type-agnostic .pending sites (NO code edits — G5)
  - ~L290 `while state.pending.is_none() {`  -> Option::is_none is generic; compiles unchanged.
  - L386 `state.pending = None;`            -> None fits Option<PendingMessage>; unchanged.
  - ~L506 `state.pending = None;` (reset_test_state) -> unchanged.
  - DO NOT edit these. The contract's "ensure reset_test_state() is updated" == verify it
    compiles, which it does. Editing is a no-op.
  - VERIFY: `git diff src/core/notifier.rs` shows NO change on these three lines.

Task 6: ADD the white-box carry test (src/core/notifier.rs, inside `mod tests`)
  - ADD (verbatim from notes §6) `fn test_debounced_pending_carries_window_info()`:
      * reset_test_state(); set_notifier(Box::new(MockNotifier::new()));
      * bump STATE.interval to 10s so the worker won't flush during inspection;
      * send w1 (immediate, primes last_sent_time), wait_for_count(1, 500ms);
      * send w2 (queues as PendingMessage);
      * lock STATE, snapshot pending.as_ref().map(|p| (p.payload.clone(), p.window_info.clone()));
      * assert_eq!(payload, "App2\x1DTitle2"); assert_eq!(wi, w2);
      * NO manual cleanup — the next reset_test_state() clears pending safely (notes §6.1).
  - NAMING: test_debounced_pending_carries_window_info (descriptive, snake_case).
  - PLACEMENT: alongside the other debounce tests in `mod tests` (after
    test_send_after_debounce_timeout or test_multiple_rapid_updates is fine).
  - COVERAGE: proves DebounceState carries BOTH payload and window_info (the task's OUTPUT).
  - GOTCHA G7: the suite runs single-threaded; this test leaves STATE.interval=10s + a queued
    pending — the NEXT test's reset_test_state() (sleep 150ms, then pending=None +
    last_sent_time=None + interval=50ms) harmlessly clears it. Proven safe in notes §6.1.
  - VERIFY (once the pin is v0.3.0): `cargo test --bin qmkonnect test_debounced_pending_carries_window_info -- --test-threads=1` -> PASS.

Task 7: VALIDATE (see the §Validation Loop for the pin caveat)
  - cargo build --bin qmkonnect   # confirm ZERO NEW errors from this task (G4 grep); pre-existing
                                  # send_command/pin errors are expected until S1 lands.
  - Once the v0.3.0 pin is in place (S1 landed OR the local-validation fallback in notes §8):
      cargo build --bin qmkonnect            # clean; NO warnings (esp. no unused window_info — G6).
      cargo test --bin qmkonnect -- --test-threads=1   # MANDATORY single-threaded (G7). All green.
  - git diff --stat              # expect ONLY src/core/types.rs + src/core/notifier.rs (NO Cargo).
```

### Implementation Patterns & Key Details

```rust
// The debounce_worker send block, AFTER the edit (only the binding + first line change;
// everything from the verbose-log onward is UNCHANGED and uses `message` exactly as before):

        if let Some((pm, verbose)) = to_send {                 // was: (message, verbose)
            // `pm` carries the formatted payload (sent below) AND the originating
            // WindowInfo. P4.M3.T1.S1 consumes `pm.window_info` here to evaluate
            // rules.toml and emit APPLY_HOST_CONTEXT alongside the string send.
            let message = pm.payload;                          // partial move -> String

            if verbose {
                let sanitized = message.replace('\x1D', "|");
                println!("[{}ms] Notified QMK (debounced): {}", crate::core::now_ms(), sanitized);
            }
            #[cfg(test)]
            println!("Sending debounced notification: {}", message);

            let notifier = get_notifier();
            let notifier = notifier.lock().unwrap();
            let _len = message.len();
            let _t0 = Instant::now();
            let _res = notifier.notify(message);              // <-- SAME bytes as before (G3)
            let _send_ms = _t0.elapsed().as_millis();
            if verbose {
                eprintln!("[{}ms] send took {}ms ({} bytes)", crate::core::now_ms(), _send_ms, _len);
            }
            if let Err(e) = _res {
                eprintln!("Error sending debounced notification: {}", e);
            }
        }
```

```rust
// The notify_qmk queue branch, AFTER the edit (only the `Some(...)` payload changes):

        } else {
            state.pending = Some(PendingMessage {
                payload: message.clone(),
                window_info: window_info.clone(),
            });
            COND.notify_one();
            false
        }
```

```rust
// The immediate-send branch needs NO change for window accessibility:
//   window_info is already the notify_qmk parameter (L367), in scope throughout the
//   immediate block (~L393-416). P4.M3.T1.S1 will extend BOTH send blocks; the immediate
//   one already has the window. Do NOT edit it here (keep the diff minimal).
```

### Integration Points

```yaml
TYPES (src/core/types.rs): WindowInfo gains Clone (derive). No new fields, no new impls.
  All existing WindowInfo construction/borrow sites keep working (Clone is additive).

NOTIFIER STATE (src/core/notifier.rs): DebounceState.pending widens String->PendingMessage.
  The 5 .pending touch sites: 2 edited (queue-store L389, worker-flush L295/~L300/~L311), 3
  type-agnostic and UNCHANGED (is_none ~L290, = None L386 immediate, = None ~L506 reset).

CALLERS: NONE touched. notify_qmk(&WindowInfo, bool) signature is stable (7 platform
  callers: hyprland x3, macos x2, x11 x2, windows x1). This task is internal plumbing.

CARGO (Cargo.toml/Cargo.lock): NONE — belongs to the sibling pin-flip P4.M1.T2.S1. Do NOT
  edit those files here (G4). Full build/test validation is gated on S1 landing (or the
  notes §8 local-validation fallback).

DOWNSTREAM (the consumer of this task's output):
  - P4.M3.T1.S1 ("host-context evaluation and send in debounce worker") consumes
    pm.window_info in the worker send block to compute a HostContext (src/core/rules.rs
    evaluate()) and emit APPLY_HOST_CONTEXT via send_command(). This task delivers the
    window to exactly that point. Do NOT implement that send here (G8).

CONFIG: none. DATABASE: none. ROUTES/CLI: none (P5.M1 owns CLI; this task has no surface).
MODULE REGISTRATION: none (pub mod notifier is long-standing; PendingMessage is private).
```

## Validation Loop

> **READ FIRST (the pin caveat — notes §8 / G4):** `Cargo.toml:16` currently pins
> qmk_notifier **v0.2.1**; `notifier.rs` already holds P4.M1.T1.S1's v0.3.0
> `send_command` code. So `cargo build`/`cargo test` in the CURRENT tree fail on
> **pre-existing** `send_command`/`RunCommand`/`CommandResponse`/`HostOs` errors
> — those are S1's in-flight pin-flip work, NOT this task. This task's own
> correctness is proven by (a) the G4 grep below (zero new errors) and (b) a full
> green run once the v0.3.0 pin is in place (S1 landed, or the §8 fallback).

### Step 0 (confirm the pin state — sets your validation expectations)

```bash
cd /home/dustin/projects/qmkonnect
grep -n 'tag =' Cargo.toml          # if "v0.2.1": build/test are gated on S1 (use the notes §8 fallback for a full green run)
                                     # if "v0.3.0": S1 has landed; proceed straight to full build+test below.
```

### Level 1: Syntax & Style (Immediate Feedback — works regardless of pin)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect 2>&1 | tee /tmp/build.log
# EXPECT (pin still v0.2.1): errors ONLY about send_command / RunCommand / CommandResponse /
#   HostOs / qmk_notifier (the pre-existing pin gap). Confirm THIS task added nothing:
grep -iE 'PendingMessage|DebounceState|WindowInfo|Clone' /tmp/build.log
# EXPECT: EMPTY. If it lists PendingMessage/DebounceState/WindowInfo/Clone errors, THIS task
#   has a real defect — fix it (the partial-move / Clone edits). Otherwise your edits are clean.

# Confirm the edits landed at the right anchors:
grep -n 'struct PendingMessage' src/core/notifier.rs            # expect 1
grep -n 'pending: Option<PendingMessage>' src/core/notifier.rs  # expect 1 (struct field)
grep -n 'Option<(PendingMessage, bool)>' src/core/notifier.rs   # expect 1 (worker flush)
grep -n 'let message = pm.payload' src/core/notifier.rs         # expect 1 (worker send block)
grep -n 'PendingMessage { payload: message.clone()' src/core/notifier.rs  # expect 1 (queue branch)
grep -n '#\[derive(Debug, Clone, PartialEq)\]' src/core/types.rs          # expect 1

# Confirm the type-agnostic sites were NOT touched (G5):
git diff src/core/notifier.rs | grep -E 'pending = None|pending\.is_none'  # expect NONE in diff
```

### Level 2: Unit Tests (Component Validation — requires the v0.3.0 pin)

> If Step 0 showed v0.2.1, apply the notes §8 local-validation fallback FIRST
> (temporarily flip Cargo.toml:16 to `tag = "v0.3.0"` + `cargo update -p
> qmk_notifier`; REVERT before commit). Then:

```bash
cd /home/dustin/projects/qmkonnect
# The new white-box carry test:
cargo test --bin qmkonnect test_debounced_pending_carries_window_info -- --test-threads=1
# EXPECT: PASS (pending holds ("App2\x1DTitle2", WindowInfo{App2,Title2}) after a debounced queue).

# All debounce + notifier tests (single-threaded is MANDATORY — G7):
cargo test --bin qmkonnect notifier:: -- --test-threads=1
# EXPECT: all green — the 6 existing debounce tests (string bytes unchanged, G3) + the 5
#   test_send_command_* (P4.M1.T1.S1) + the new carry test. No regression.
```

### Level 3: Full Suite (System Validation — requires the v0.3.0 pin)

```bash
cd /home/dustin/projects/qmkonnect
# The whole bin test suite, single-threaded:
cargo test --bin qmkonnect -- --test-threads=1
# EXPECT: ALL bin tests green — notifier (debounce + send_command + carry) + pattern (P2)
#   + rules (P3) + types + mod. No regression anywhere.
#   (types::tests still pass — adding Clone doesn't affect equality/construction.)

# Optional lint pass (no NEW warnings vs. main):
cargo clippy --bin qmkonnect --no-deps 2>/dev/null || true
```

### Level 4: Contract / Scope Validation

```bash
cd /home/dustin/projects/qmkonnect
# Gate 1 — "only the two src/ files changed (NO Cargo — those are S1's)":
git diff --stat
# EXPECT: exactly src/core/types.rs + src/core/notifier.rs. If Cargo.toml/Cargo.lock/
#   platforms/ appear, you've made an out-of-scope edit — revert it (the §8 fallback must
#   be reverted before commit).

# Gate 2 — "notify_qmk signature unchanged" (G2):
git diff src/core/notifier.rs | grep -E '^\+.*pub fn notify_qmk|^-.*pub fn notify_qmk'
# EXPECT: empty (the signature line is unchanged; 7 callers are safe).

# Gate 3 — "the formatted payload string is unchanged" (G3) — prove it via the debounce test:
cargo test --bin qmkonnect notifier::tests::test_debounce_subsequent_messages -- --test-threads=1
# EXPECT: PASS — MOCK_LAST_MESSAGE still equals "App2\x1DTitle2" (the payload bytes survived).

# Gate 4 — "the seam exists for P4.M3.T1.S1":
grep -n 'pm.window_info' src/core/notifier.rs   # the comment references it; the binding `pm`
#   is in scope at the notify(message) call. P4.M3.T1.S1 will add `let window_info = pm.window_info;`
#   (or use it directly) right after `let message = pm.payload;`.

# Gate 5 — "no duplicate WindowInfo import" (G1):
grep -c 'use crate::core::types::WindowInfo' src/core/notifier.rs
# EXPECT: 2 (one at L1 module-level, one at L438 in mod tests) — UNCHANGED from before.
#   If it's 3, you added a duplicate — remove it.

# Gate 6 — "this task added zero new compile errors" (G4):
grep -iE 'PendingMessage|DebounceState|WindowInfo|Clone' /tmp/build.log   # expect EMPTY
```

## Final Validation Checklist

### Technical Validation
- [ ] Step 0 pin-state checked; validation path chosen (full green requires v0.3.0 pin — S1 landed or §8 fallback).
- [ ] **G4 gate**: `cargo build` errors are ONLY the pre-existing `send_command`/qmk_notifier ones; `grep PendingMessage|DebounceState|WindowInfo|Clone` on the build log is EMPTY.
- [ ] (with v0.3.0 pin) `cargo build --bin qmkonnect` clean (no errors, no NEW warnings; no unused-variable warning — G6).
- [ ] (with v0.3.0 pin) `cargo test --bin qmkonnect -- --test-threads=1` green (no regression; new test passes).
- [ ] `git diff --stat` = `src/core/types.rs` + `src/core/notifier.rs` ONLY — NO Cargo.toml/Cargo.lock (Gate 1).

### Feature Validation (contract fidelity)
- [ ] `WindowInfo` derives `Clone` (types.rs L1).
- [ ] `PendingMessage { payload, window_info }` struct exists (private, beside `DebounceState`).
- [ ] `DebounceState.pending` is `Option<PendingMessage>`.
- [ ] `notify_qmk` queue branch stores `PendingMessage { payload: message.clone(), window_info: window_info.clone() }`.
- [ ] `debounce_worker` flushes `(PendingMessage, bool)`; send block has `pm.window_info` available at the `notify(message)` call (the P4.M3.T1.S1 seam).
- [ ] `notifier.notify(message)` called with the IDENTICAL formatted string in both paths (Gate 3 green).
- [ ] New test `test_debounced_pending_carries_window_info` passes (white-box carry proof).
- [ ] The three type-agnostic `.pending` sites (~L290/L386/~L506) were NOT edited (G5).

### Code Quality Validation
- [ ] `notify_qmk` signature unchanged (Gate 2; 7 callers safe).
- [ ] No duplicate `use crate::core::types::WindowInfo;` (Gate 5; G1).
- [ ] No `unused variable` warning — partial-move form used (G6).
- [ ] No out-of-scope work: no `send_command`/rules-eval/HostContext (that's P4.M3.T1.S1 — G8); no Cargo/CLI/tray edits (G4).
- [ ] File placement matches the desired tree (PendingMessage private in notifier.rs; Clone in types.rs).

### Documentation & Deployment
- [ ] `PendingMessage` has a doc-comment explaining it carries the window for P4.M3.T1.S1.
- [ ] The worker send-block comment marks the `pm.window_info` seam for P4.M3.T1.S1.
- [ ] Commit message notes: "plumbing only — carries WindowInfo through the debouncer for P4.M3.T1.S1; no behavior change; no Cargo edits (pin flip is P4.M1.T2.S1)."

---

## Anti-Patterns to Avoid

- ❌ Don't add a second `use crate::core::types::WindowInfo;` — it's already at L1 (and L438 in tests). Duplicate import = compile error (G1).
- ❌ Don't change `notify_qmk`'s signature — 7 platform callers depend on it (G2).
- ❌ Don't change the formatted payload bytes — existing tests assert on them; keep `notify` receiving `"<app>\x1D<title>"` (G3).
- ❌ Don't mistake the pre-existing `send_command`/qmk_notifier build errors (pin still v0.2.1) for defects in this task — confirm via the G4 grep; they're S1's in-flight work (G4 / notes §8).
- ❌ Don't commit a `Cargo.toml`/`Cargo.lock` pin change here — that's S1's file; the §8 local-validation fallback must be reverted before commit (G4).
- ❌ Don't edit the type-agnostic `.pending` sites (~L290 `is_none`, L386/~L506 `= None`) — they compile unchanged; "ensure reset_test_state updated" means verify, not edit (G5).
- ❌ Don't bind `window_info` unused in the worker send block — use the partial-move form (`let message = pm.payload;`) to stay lint-clean (G6).
- ❌ Don't run tests multi-threaded — `--test-threads=1` is mandatory (G7).
- ❌ Don't implement rule evaluation / `send_command` / `HostContext` here — that's P4.M3.T1.S1's scope and would collide (G8).
- ❌ Don't add `Eq`/`Default`/`Copy` to `WindowInfo` speculatively — add only `Clone` (the minimum this task needs); P4.M3.T1.S1 adds more if it needs them.
- ❌ Don't extract a shared `send_with_context` helper now — the two send blocks differ in control flow/label; P4.M3.T1.S1 (which adds identical logic to both) is the right moment to decide on a helper. Keep this task minimal.
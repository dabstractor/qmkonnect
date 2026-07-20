# PRP — P4.M3.T1.S1: Implement host-context evaluation and send in debounce worker

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This 2-point task **completes the
> host-side-rules pipeline end-to-end** (HOST_RULES.md §8(4)): in **both** send
> blocks of `src/core/notifier.rs` — the debounce-worker flush (L600-635) AND
> `notify_qmk`'s immediate-send path (L691-720) — it evaluates `rules.toml`
> against the window and emits `APPLY_HOST_CONTEXT`: **string-first** (stack),
> **context-only** (replace), or **context-clear** (no-match). When host rules are
> disabled (legacy board / no `rules.toml` / malformed), behavior is **bit-for-bit
> today's** (legacy string only). It also closes the one gap the handshake
> (P4.M2.T1.S1) left open — it discards the firmware's `board_rules_present` bit,
> which `rules::evaluate` needs.
>
> **Single file edited: `src/core/notifier.rs`.** No Cargo, no new files, no
> rules.rs/types.rs (consumed read-only), no CLI/tray/runner (P5).
>
> **Consumes (all VERIFIED LANDED in the current code):**
> `PendingMessage{payload,window_info}` + the worker/immediate send seams
> (P4.M1.T2.S2 — landed), `Notifier::send_command` + the mock recorder/queue
> (P4.M1.T1.S1 + P4.M2.T1.S1 — landed), `host_capable()` (L440) /
> `callback_names()` (L447) / `perform_handshake` (L265) / `reset_handshake_state`
> (L456) (P4.M2.T1.S1 — **landed**), and `rules::evaluate` / `rules::HostContext` /
> `rules::get_rules_paths` / `rules::parse_rules` (P3.M1 — landed).
>
> **Consumed downstream by:** nothing in the current plan — this is the terminal
> host-rules send task (P5 CLI/tray read state but don't change the send path).
>
> **SEQUENTIAL (not parallel) vs P4.M2.T1.S2:** S2 is being implemented in parallel
> and edits runners/tray/linux_tray + appends `handshake_action` to notifier.rs.
> S2 does NOT touch the send blocks (L600-720) or the `perform_handshake` body
> (L265-360) that THIS task edits — disjoint regions. Implementation of THIS task
> is sequential after P4.M2, so the 2 additive lines it adds to S1's landed
> `perform_handshake`/`reset_handshake_state` are edits to already-merged code —
> no merge conflict.

---

## ⚠️ READ FIRST — three non-obvious traps (all VERIFIED against real code)

1. **`board_rules_present` is DISCARDED by the landed handshake.** `perform_handshake`'s
   capable `Info` arm destructures `board_rules_present` (L286) and only LOGS it
   (L291); it stores `HOST_CAPABLE` (L332) but NEVER stores the board-rules bit.
   But `rules::evaluate` *needs* `board_has_rules: bool` (it folds the bit into
   `clear_board`). This task closes the gap: declare `BOARD_HAS_RULES: AtomicBool` +
   `board_has_rules()` here, and add **2 additive lines** to S1's landed code
   (store after L332; clear in `reset_handshake_state` L456-459). See research §0/§5
   + Implementation Task 1. (`AtomicBool`/`Ordering` already imported at L6.)
2. **The string send is CONDITIONAL, not unconditional.** In replace and no-match
   modes the legacy string must **NOT** be sent (only `APPLY_HOST_CONTEXT`). So
   evaluate FIRST, then branch on `(ctx.any_match, ctx.clear_board)`. Do **not**
   leave `notifier.notify(message)` running unconditionally before the host
   decision — that would send the string in replace mode (board would then match,
   breaking the replace contract). See Gotcha G5.
3. **Error propagation differs between the two call sites and MUST be preserved.**
   The worker SWALLOWS the legacy-string error (`if let Err(e) = _res { eprintln }`
   at L633); the immediate path PROPAGATES it (`_res?` at L718). The shared
   `dispatch_window_send` helper RETURNS the legacy-string `Result` and each call
   site handles it as before. The host-context send swallows its OWN errors (§5.4
   retry parity) so it never changes the string-result propagation. See Gotcha G4.

---

## Goal

**Feature Goal**: Make the per-window-change send logic implement the full
HOST_RULES.md §8(4) contract: when the connected board is host-capable
(`proto_ver == 2` + feature bit) **and** a `rules.toml` is present, evaluate the
rules and emit `APPLY_HOST_CONTEXT` — **stack** (send the legacy string first,
then `ApplyHostContext{layer, callbacks, clear_board:false}`), **replace**
(send only `ApplyHostContext{..., clear_board:true}`, no string), or **no-match**
(send only `ApplyHostContext{layer:None, callbacks:[], clear_board:false}`, no
string). When host rules are disabled (legacy board, no `rules.toml`, or a
malformed file), the legacy string-only path runs **identically to today**.

**Deliverable** (single file, `src/core/notifier.rs`; **NO Cargo, no new files**):
1. **`board_has_rules()` capability** — `static BOARD_HAS_RULES: AtomicBool` +
   `pub fn board_has_rules() -> bool` (closes the §0 gap); 2 additive lines in S1's
   landed `perform_handshake` (store `board_rules_present` after L332) +
   `reset_handshake_state` (clear, L456-459).
2. **6 host-context helpers** in a new band after `notify_qmk` (L~730):
   `host_context_for_window`, `dispatch_window_send`, `send_legacy_string`,
   `send_host_context`, `host_context_command`, `clear_host_context_command`.
3. **2 send-block rewrites** — debounce-worker flush (L600-635) + `notify_qmk`
   immediate (L691-720) — route through `dispatch_window_send` (the verbose
   log/timing moves into `send_legacy_string`; bytes + cadence + error-propagation
   policy preserved).
4. **6 tests** (4 orchestration via injected `HostContext`, 1 gate, 1 full-path).

**Success Definition**:
- A host-capable board with a matching **non-disabling** rule ⇒ 1 string send +
  1 `ApplyHostContext{clear_board:false}` (string FIRST). A matching **all-
  disabling** rule (or no board rules) ⇒ 0 string sends + 1
  `ApplyHostContext{clear_board:true}`. **No match** ⇒ 0 string sends + 1
  `ApplyHostContext{layer:None,clear_board:false}`.
- A legacy board, OR no `rules.toml`, OR a malformed `rules.toml` ⇒ **identical to
  today**: the legacy string is sent exactly as before (same bytes, same cadence,
  same verbose logs). Existing debounce tests stay green unmodified.
- `cargo build --bin qmkonnect` clean; `cargo test --bin qmkonnect --
  --test-threads=1` green (6 new + all existing).
- `git diff --stat` = `src/core/notifier.rs` ONLY.

## User Persona (if applicable)

**Target User**: the end user (transparent) + the host-rules feature itself (this
is the last code link between "window changed" and "the board acts on host rules").

**Use Case**: "I edit `rules.toml` (or it ships with the app) and my keyboard's
layers/callbacks adapt to the focused window **without reflashing** — the board
runs its own rules where I told it to (stack) or yields to the host (replace), and
a window with no rule cleanly clears the host layer."

**Pain Points Addressed**: today the debounce send only emits the legacy string;
even on a capable board with `rules.toml`, no `APPLY_HOST_CONTEXT` is ever sent, so
host layers/callbacks never activate. This task is the send that makes the whole
feature live.

## Why

- **PRD §8(4)** / **HOST_RULES.md §8(4)** — the canonical send logic (stack /
  replace / no-match) is exactly what this task implements.
- **PRD §4 (`h2.82`)** — the architecture's "send the string first iff the board
  has rules AND ≥1 matched rule is non-disabling; otherwise send only
  APPLY_HOST_CONTEXT with clear_board=1" — this task is that per-window decision,
  executed at the debounced send step.
- **PRD §5.7 (`h3.16`)** — "the host-context send happens within the same
  debounced 'send' step (one window change ⇒ ≤2 sends: string + context, or
  context-only in replace mode). Retry/cache for the typed command match the
  string path (§5.4)." This task adds that send (≤2 per window) and the retry parity.
- **PRD §8(8)** — backward compatibility: legacy firmware / no rules ⇒ string-only,
  board rules unaffected. The `host_capable()` + `rules.toml`-present gate +
  graceful-malformed-file fallback deliver exactly this.
- **Activates** the entire P1–P4 host-rules investment end-to-end (terminal send task).

## What

Additive host-context send logic in `notifier.rs`, routed through a shared
`dispatch_window_send` helper that both send blocks call. No change to the
`Notifier` trait, `notify()`/`send_command()` impls, `rules.rs`, `types.rs`, the
debounce TIMING/algorithm, `notify_qmk`'s signature, runners, tray, or CLI. The
only observable behavior change is the additional (or substituting)
`APPLY_HOST_CONTEXT` typed command on capable boards — invisible on legacy boards.

### Success Criteria
- [ ] `board_has_rules()` exists; `perform_handshake`'s capable arm stores
      `board_rules_present` into `BOARD_HAS_RULES`; `reset_handshake_state` clears it.
- [ ] `dispatch_window_send(notifier, filter, message, ctx, label, verbose)` exists
      and branches: `None`⇒string-only; `(any_match&&!clear_board)`⇒string-then-context;
      `(any_match&&clear_board)`⇒context-only(clear=true); `!any_match`⇒context-clear.
- [ ] `host_context_for_window(window_info, verbose)` returns `None` when
      `!host_capable()` OR no `rules.toml` OR malformed; else `Some(evaluate(...))`.
- [ ] Both send blocks (worker flush L600, immediate L691) call `dispatch_window_send`;
      worker swallows the string error, immediate propagates it (`?`) — both preserved.
- [ ] `send_host_context` retries 3× for device errors then swallows (§5.4 parity
      with `QmkNotifier::notify`); never changes the string-result propagation.
- [ ] Legacy string bytes/cadence/verbose-logs unchanged when host rules disabled.
- [ ] 6 new tests pass; all existing tests green; `--test-threads=1` honored.
- [ ] `git diff --stat` = `src/core/notifier.rs` ONLY.

## All Needed Context

### Context Completeness Check

_Pass_: an agent with no prior knowledge can implement this using only this PRP +
`research/notes.md`, because: (a) the EXACT verbatim CURRENT anchors for both send
blocks (L600-635, L691-720) + the module imports are in research §4 (copy-paste
old→new); (b) the full helper source (`dispatch_window_send`, `send_legacy_string`,
`send_host_context`, `host_context_for_window`, the two command builders) is in
research §3; (c) the `board_rules_present` gap is VERIFIED against the landed code
(L286/L291/L332) with the exact 2-line fix in research §0/§5; (d) the §8(4) branch
table (any_match × clear_board) is in research §2; (e) the crate enum shapes
(`ApplyHostContext`, `CommandResponse`) are quoted verbatim in research §1
(confirmed by the landed `test_send_command_records_call_sequence`); (f) 9 gotchas
(G1–G9) cover build-precondition, single-file scope, string-byte preservation,
error-propagation preservation, conditional-string, single-threaded tests, the
`&**notifier` deref, the board_has_rules gap, and the verbose-log label; (g) the
6-test plan with injected `HostContext` is in research §6; (h) verified validation
commands are in research §8. All line numbers in the research notes were checked
against the CURRENT 1363-line notifier.rs.

### Documentation & References

```yaml
# MUST READ — the verbatim research (THIS task's full contract + design + safety proofs)
- file: plan/002_637d65b6e9b8/P4M3T1S1/research/notes.md
  why: "§0 = dependency contract VERIFIED against landed code + the board_rules_present
        gap (the §0 trap, confirmed at L286/L291/L332). §1 = the v0.3.0 crate enum shapes.
        §2 = the §8(4) branch table + retry parity. §3 = FULL source of all 6 helpers +
        the static/accessor. §4 = verbatim CURRENT→NEW for both send blocks (L600-635,
        L691-720) — copy-paste. §5 = the 2 additive lines in S1's LANDED handshake.
        §6 = the 6-test plan. §7 = G1–G9. §8 = validation."

# MUST READ — the spec sources of truth (selected sections are in this PRP's header)
- file: spec/HOST_RULES.md
  why: "§8(4) is the CANONICAL send logic (stack/replace/no-match). §4 is the
        architecture/coexistence diagram (string-first iff board has rules AND ≥1
        non-disabling). §5.4 = retry/cache parity. §8(8) = backward compat (legacy ⇒
        string-only)."
  section: "§4 (Architecture & Coexistence), §8(4) (notify_qmk send logic), §5.4 (retry), §8(8)"

# MUST READ — the file THIS task edits (the only one)
- file: src/core/notifier.rs
  why: "contains both send blocks (debounce_worker flush L600-635 + notify_qmk immediate
        L691-720), the Notifier trait (L53) + QmkNotifier impl (L506) + MockNotifier impl
        (L788, with MOCK_RESPONSES queue L744 + set_mock_responses L774), configured_filter
        (L77), get_notifier (L286→ now further down), and S1's LANDED handshake:
        perform_handshake (L265, capable arm stores HOST_CAPABLE at L332), host_capable
        (L440), callback_names (L447), reset_handshake_state (L456-459). THIS TASK:
        rewrites the 2 send blocks, adds 6 helpers + BOARD_HAS_RULES after notify_qmk,
        adds 2 lines to S1's handshake, adds 6 tests in mod tests."
  pattern: "extract dispatch_window_send; route both send blocks through it; preserve
            string bytes + each call site's error-propagation policy; swallow host-context
            errors after §5.4 retry."
  gotcha: "do NOT touch DebounceState.pending/struct, the debounce TIMING, notify_qmk's
           signature, the trait, or rules.rs/types.rs. Do NOT send the string in
           replace/no-match (G5)."

# MUST READ — the consumer contract (evaluate + HostContext — read-only)
- file: src/core/rules.rs
  why: "evaluate(&RuleSet, &str, &str, &HashMap<String,u8>, board_has_rules: bool) ->
        HostContext (P3.M1.T2.S1). HostContext{layer:Option<u8>, callback_ids:Vec<u8>,
        clear_board:bool, any_match:bool} derives Debug+Clone+PartialEq. get_rules_paths
        + parse_rules are the IO gate. evaluate() ALREADY folds board_has_rules into
        clear_board and short-circuits no-match to {clear_board:false,any_match:false}."
  section: "HostContext, evaluate, get_rules_paths, parse_rules"

# MUST READ — the qmk_notifier v0.3.0 crate (the wire types)
- url: https://github.com/dabstractor/qmk_notifier/blob/v0.3.0/src/lib.rs
  why: "RunCommand::ApplyHostContext{layer:Option<u8>, callbacks:Vec<u8>, clear_board:bool};
        CommandResponse::{Ack{ok}, Info{...board_rules_present...}, Legacy{matched}, Timeout}.
        Both enums derive Debug+Clone+PartialEq+Eq (command.clone() in the retry loop is sound)."
  critical: "the crate resolves via Cargo.toml tag='v0.3.0' (P4.M1.T2.S1 = Complete). If
             cargo build fails to fetch the tag, that's an ENV/network issue, not a code
             issue (G1). The landed notifier.rs already uses these shapes, so the build is
             the source of truth."

# MUST READ — the predecessor PRPs (the seams this task builds on)
- file: plan/002_637d65b6e9b8/P4M1T2S2/PRP.md   # PendingMessage + the worker/immediate seams (LANDED)
  why: "P4.M1.T2.S2 landed PendingMessage{payload,window_info}, the worker flush of
        (pm, verbose), and the immediate block with window_info in scope. Its PRP
        explicitly DEFERRED the helper extraction to THIS task. Read to confirm the seams."
- file: plan/002_637d65b6e9b8/P4M2T1S1/PRP.md   # host_capable / callback_names / perform_handshake (LANDED)
  why: "S1 LANDED host_capable()/callback_names()/perform_handshake/reset_handshake_state()
        AND the mock's set_mock_responses/MOCK_RESPONSES queue. It destructures
        board_rules_present but does NOT store it — THIS task closes that gap (2 additive
        lines). Do NOT reimplement S1's logic."
- file: plan/002_637d65b6e9b8/P4M1T1S1/PRP.md   # send_command trait + mock recorder (LANDED)
  why: "P4.M1.T1.S1 added Notifier::send_command (thin transport, NO retry) + the mock
        recorder (MOCK_SEND_COMMAND_CALLS). This task wraps the ApplyHostContext send in
        the retry loop S1's rustdoc said is the CALLER's job."
- file: plan/002_637d65b6e9b8/P3M1T2S1/PRP.md   # evaluate / HostContext (LANDED)
  why: "the pure evaluator this task consumes. Its tests already pin evaluate() correctness;
        THIS task does NOT re-test evaluate — it tests the SEND orchestration around it."
```

### Current Codebase tree (relevant subset)

```bash
src/core/
  notifier.rs     # ← THIS TASK EDITS THIS FILE ONLY (now 1363 lines).
                   #   debounce_worker flush block        L600-635 (rewrite → dispatch_window_send)
                   #   notify_qmk immediate-send block    L691-720 (rewrite → dispatch_window_send)
                   #   AFTER notify_qmk (L~730): +6 helpers + BOARD_HAS_RULES + board_has_rules()
                   #   in S1's LANDED band: +1 line in perform_handshake after L332 (store board_rules_present)
                   #                        +1 line in reset_handshake_state L456-459 (clear)
                   #   mod tests: +6 tests
  rules.rs        # P3.M1 — evaluate/HostContext/get_rules_paths/parse_rules. UNCHANGED (read-only).
  pattern.rs      # P2.M1 matcher. UNCHANGED.
  types.rs        # WindowInfo (already Clone via P4.M1.T2.S2). UNCHANGED.
  mod.rs          # UNCHANGED.
Cargo.toml        # qmk_notifier tag="v0.3.0". UNCHANGED.
```

### Desired Codebase tree with files to be changed

```bash
src/core/notifier.rs     # MODIFIED (single file): 2 send-block rewrites + 6 helpers +
                         #   BOARD_HAS_RULES/board_has_rules + 2 lines in S1's handshake + 6 tests.
# EVERYTHING ELSE UNCHANGED. No Cargo, no new files, no rules.rs/types.rs/pattern.rs,
# no platforms/, no runners/, no tray/, no CLI.
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (G1 — build precondition): qmk_notifier v0.3.0 must resolve (Cargo.toml
//   tag="v0.3.0", P4.M1.T2.S1 = Complete). The landed notifier.rs already uses
//   ApplyHostContext/Info, so `cargo build` is the gate. A fetch failure is env/network.

// CRITICAL (G2 — single file): edit ONLY src/core/notifier.rs. rules.rs evaluate is
//   read-only; types.rs WindowInfo already has Clone; no Cargo/CLI/tray/runner.

// CRITICAL (G3 — string bytes unchanged): send_legacy_string calls notify() with the
//   IDENTICAL "{app_class}\x1D{title}" string. Existing debounce tests assert
//   MOCK_LAST_MESSAGE == "App2\x1DTitle2" — they must stay green. The .to_string()
//   (notify takes owned String) does not change bytes.

// CRITICAL (G4 — error propagation preserved): worker SWALLOWS the string error
//   (if let Err(e) = _res { eprintln } at L633); immediate PROPAGATES (_res? at L718).
//   dispatch_window_send RETURNS the legacy-string Result; each call site handles it as
//   before. The host-context send swallows its own errors (retry parity) — never changes this.

// CRITICAL (G5 — replace/no-match send NO string): evaluate FIRST, branch on
//   (any_match, clear_board). In replace (any_match && clear_board) and no-match
//   (!any_match) do NOT call notify() — only ApplyHostContext. MOCK_CALL_COUNT must
//   stay 0 in those branches. Sending the string in replace would let the board match,
//   breaking the replace contract.

// GOTCHA (G6 — single-threaded tests): cargo test --bin qmkonnect -- --test-threads=1
//   (process-global STATE/COND/WORKER/NOTIFIER/HANDSHAKE globals).

// GOTCHA (G7 — &**notifier, not as_ref): passing the locked Box<dyn Notifier> as a
//   &dyn Notifier arg uses &**notifier (*g → Box<dyn Notifier>; **g → dyn Notifier;
//   &**g → &dyn Notifier). as_ref() returns &Box<_> (wrong type).

// CRITICAL (G8 — board_rules_present gap, VERIFIED): P4.M2.T1.S1's perform_handshake
//   (LANDED) destructures board_rules_present (L286) but only logs it (L291). rules::evaluate
//   NEEDS board_has_rules. This task adds static BOARD_HAS_RULES + board_has_rules() AND
//   2 additive lines in S1's handshake (1 store in the capable Info arm AFTER the L332
//   HOST_CAPABLE.store(true,...); 1 clear in reset_handshake_state L456-459). Ordering/
//   AtomicBool already imported at L6. board_has_rules() is only read when host_capable()
//   is true, so a stale value on a non-capable board is never consulted.

// GOTCHA (G9 — verbose log label): the old "Notified QMK (debounced)"/"(immediate)"
//   strings + "send took Xms" timing move INTO send_legacy_string (label param) —
//   output unchanged for the string-sending branches. In replace/no-match NO string is
//   sent so no "Notified QMK" log prints (correct — don't log a string that wasn't sent);
//   send_host_context has its own terse verbose line.
```

## Implementation Blueprint

### Data models and structure

```rust
// ── board-rules capability bit (closes the P4.M2.T1.S1 gap) ──
// Place in the new helper band (after notify_qmk, before mod tests). Module-level
// static ⇒ visible to perform_handshake/reset_handshake_state in S1's band.
static BOARD_HAS_RULES: AtomicBool = AtomicBool::new(false);

/// Does the connected keyboard's keymap declare board rules? Populated by
/// [`perform_handshake`] (the firmware's `board_rules_present` bit) alongside
/// [`HOST_CAPABLE`]; read by [`host_context_for_window`] to pass into
/// [`crate::core::rules::evaluate`] so the stack-vs-replace decision knows whether
/// the board would run its own rules for the string. `false` until a capable
/// handshake sets it, and on legacy/offline boards (where host rules are disabled
/// anyway).
pub fn board_has_rules() -> bool {
    BOARD_HAS_RULES.load(Ordering::SeqCst)
}

// ── the 6 host-context helpers (full source in research/notes.md §3) ──
fn host_context_for_window(window_info: &WindowInfo, verbose: bool)
    -> Option<crate::core::rules::HostContext>;
fn dispatch_window_send(notifier: &dyn Notifier, filter: &DeviceFilter, message: &str,
    ctx: Option<crate::core::rules::HostContext>, label: &str, verbose: bool)
    -> Result<(), Box<dyn Error + Send + Sync>>;
fn send_legacy_string(notifier: &dyn Notifier, message: &str, label: &str, verbose: bool)
    -> Result<(), Box<dyn Error + Send + Sync>>;
fn send_host_context(notifier: &dyn Notifier, filter: &DeviceFilter,
    command: qmk_notifier::RunCommand, verbose: bool);
fn host_context_command(ctx: &crate::core::rules::HostContext) -> qmk_notifier::RunCommand;
fn clear_host_context_command() -> qmk_notifier::RunCommand;
```

(The complete bodies — including the §8(4) branch `match`, the §5.4 retry loop,
and the IO gate — are in `research/notes.md §3`. Copy them verbatim.)

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: ADD the board_has_rules capability (static + accessor + 2 lines in S1's LANDED handshake)
  - ADD (in the new helper band after notify_qmk): `static BOARD_HAS_RULES: AtomicBool =
    AtomicBool::new(false);` + `pub fn board_has_rules() -> bool { ... }` (Data Models).
  - EDIT S1's perform_handshake capable Info arm: immediately AFTER the existing L332
    `HOST_CAPABLE.store(true, Ordering::SeqCst);` line, add:
        BOARD_HAS_RULES.store(board_rules_present, Ordering::SeqCst);
    (board_rules_present is already destructured in that arm at L286 — S1 logs it at L291.)
  - EDIT S1's reset_handshake_state (L456-459): add `BOARD_HAS_RULES.store(false, Ordering::SeqCst);`
    alongside the existing HOST_CAPABLE/CALLBACK_NAMES/HAS_HANDSHAKED clears.
  - DEPENDENCIES: AtomicBool + Ordering ALREADY imported at module L6. The static is
    module-level ⇒ in scope inside perform_handshake/reset_handshake_state.
  - GOTCHA G8: these are the ONLY 2 touches to S1's LANDED code; they are additive.
    The else/Err arms (L343, L355) need NO edit (host_capable=false ⇒ board_has_rules
    never consulted).
  - VERIFY: grep -n 'BOARD_HAS_RULES\|pub fn board_has_rules' src/core/notifier.rs -> 3 hits
    (1 static, 1 fn, 1 doc); grep -n 'BOARD_HAS_RULES.store' src/core/notifier.rs -> 2 (Info arm + reset).

Task 2: ADD the 6 host-context helpers (new band after notify_qmk L~730, before mod tests)
  - INSERT: the 6 fns from research/notes.md §3 (host_context_for_window,
    dispatch_window_send, send_legacy_string, send_host_context, host_context_command,
    clear_host_context_command), each with Mode-A rustdoc.
  - DEPENDENCIES (all already in scope): host_capable/callback_names (S1, landed), board_has_rules
    (Task 1), crate::core::rules::{get_rules_paths, parse_rules, evaluate, HostContext},
    crate::core::now_ms, configured_filter (L77), qmk_notifier::{RunCommand, CommandResponse},
    Notifier trait, Instant/Duration/thread (already imported).
  - NAMING: snake_case fns; dispatch_window_send / send_legacy_string / send_host_context
    / host_context_for_window / host_context_command / clear_host_context_command.
  - GOTCHA G9: send_legacy_string takes a `label: &str` ("debounced"|"immediate") and
    prints the EXACT current log format. send_host_context's verbose line is terse.
  - GOTCHA G7: dispatch_window_send takes `notifier: &dyn Notifier`; call sites pass &**notifier.
  - GOTCHA G5: dispatch_window_send's match sends the string ONLY in the None + stack arms.
  - VERIFY: grep -n 'fn dispatch_window_send\|fn send_legacy_string\|fn send_host_context\|fn host_context_for_window\|fn host_context_command\|fn clear_host_context_command' src/core/notifier.rs -> 6.

Task 3: REWRITE the debounce-worker flush block (notifier.rs L600-635, inside fn debounce_worker)
  - REPLACE the `if let Some((pm, verbose)) = to_send { ... }` body with the NEW form
    in research/notes.md §4.1 (destructure PendingMessage, compute filter + ctx, lock
    notifier, call dispatch_window_send(&**notifier, &filter, &message, ctx, "debounced", verbose),
    SWALLOW the error via `if let Err(e) = _res { eprintln }`).
  - PRESERVE: the worker's swallow-the-string-error behavior (G4). The verbose log/timing
    moved into send_legacy_string (label "debounced").
  - GOTCHA G3: the message string passed to notify is byte-identical.
  - VERIFY: grep -n 'dispatch_window_send' src/core/notifier.rs -> 2 (worker + immediate).

Task 4: REWRITE the notify_qmk immediate-send block (notifier.rs L691-720)
  - REPLACE the `if send_immediately { ... }` body with the NEW form in research §4.2
    (compute filter + ctx, lock notifier, call dispatch_window_send(..., "immediate", verbose),
    PROPAGATE the error via `_res?`). The `else if verbose` Debouncing-log branch is UNCHANGED.
  - PRESERVE: the immediate path's propagate-via-? behavior (G4).
  - GOTCHA: window_info is the notify_qmk param (&WindowInfo), in scope; message is the
    local String — pass &message.
  - VERIFY: the notify_qmk signature (`pub fn notify_qmk(window_info: &WindowInfo, verbose:
    bool)`) is UNCHANGED (git diff shows no signature change).

Task 5: MID-POINT build gate
  - RUN: cargo build --bin qmkonnect   (expect clean — G1: v0.3.0 resolves)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1   (expect: ALL existing tests
    green — the debounce tests get the SAME string bytes via send_legacy_string; the
    mock send_command still returns Ack default. No new test yet.)

Task 6: ADD the 6 tests (append to #[cfg(test)] mod tests)
  - Append the 6 tests from research/notes.md §6:
      1. test_dispatch_legacy_string_only_when_no_host_context   (ctx=None ⇒ notify=1, cmd=0)
      2. test_dispatch_stack_sends_string_then_context           (stack ⇒ notify=1, ApplyHostContext{clear:false})
      3. test_dispatch_replace_sends_context_only                (replace ⇒ notify=0, ApplyHostContext{clear:true})
      4. test_dispatch_no_match_sends_clear_context              (no-match ⇒ notify=0, ApplyHostContext{layer:None})
      5. test_host_context_for_window_none_when_not_capable      (gate ⇒ None)
      6. test_notify_qmk_legacy_string_when_not_capable          (full path ⇒ string-only)
  - Each starts reset_test_state() + reset_handshake_state() + set_notifier(Box::new(MockNotifier::new())).
    Tests 1–4 lock the global notifier and pass &**guard as &dyn Notifier to
    dispatch_window_send with an INJECTED Option<HostContext> (no rules.toml needed).
  - NAMING: test_dispatch_* / test_host_context_for_window_* / test_notify_qmk_*.
  - GOTCHA G6: single-threaded. Tests construct crate::core::rules::HostContext{...} directly
    (fully-qualified or `use crate::core::rules::HostContext;` inside the test fn).
  - GOTCHA (ordering): the mock records notify + send_command in SEPARATE channels, so
    cross-channel "string before context" order isn't directly assertable; it is
    STRUCTURALLY guaranteed (send_legacy_string precedes send_host_context in the stack arm).
    Tests 2–4 assert COUNTS + the command shape.
  - DO NOT modify existing debounce/send_command/handshake tests.
  - VERIFY: cargo test --bin qmkonnect dispatch_window_send -- --test-threads=1 -> 4 passed;
    cargo test --bin qmkonnect host_context_for_window -- --test-threads=1 -> 1 passed.

Task 7: VALIDATE (build + full suite + scope)
  - cargo build --bin qmkonnect
  - cargo test --bin qmkonnect -- --test-threads=1     # MANDATORY single-threaded (G6). All green.
  - cargo clippy --bin qmkonnect --no-deps 2>/dev/null || true   # optional; no NEW warnings.
  - git diff --stat                                     # expect ONLY src/core/notifier.rs.
```

### Implementation Patterns & Key Details

```rust
// THE §8(4) branch table (the heart of this task) — dispatch_window_send:
match ctx {
    None => send_legacy_string(notifier, message, label, verbose),     // disabled → string-only
    Some(ctx) if ctx.any_match && !ctx.clear_board => {                // STACK
        let r = send_legacy_string(notifier, message, label, verbose); //   string FIRST
        send_host_context(notifier, filter, host_context_command(&ctx), verbose); // then context(clear=false)
        r
    }
    Some(ctx) if ctx.any_match => {                                    // REPLACE
        send_host_context(notifier, filter, host_context_command(&ctx), verbose); // context-only(clear=true)
        Ok(())
    }
    Some(_) => {                                                       // NO MATCH
        send_host_context(notifier, filter, clear_host_context_command(), verbose); // layer=None
        Ok(())
    }
}

// THE §5.4 retry parity (send_host_context) — mirrors QmkNotifier::notify's loop:
for attempt in 1..=3 {
    match notifier.send_command(command.clone(), filter) {
        Ok(_) => { /* verbose log */ return; }
        Err(e) => {
            let s = e.to_string().to_lowercase();
            if s.contains("no device found") || s.contains("permission denied") || s.contains("failed to open") {
                if attempt < 3 { thread::sleep(Duration::from_millis(100 * attempt as u64)); continue; }
                eprintln!("QMK device unavailable after {} attempts sending host context: {}", attempt, e);
                return; // swallowed
            }
            eprintln!("Error sending host context: {}", e);
            return; // non-device: log + swallow (don't fail the window send)
        }
    }
}

// THE IO gate (host_context_for_window) — None ⇒ legacy string-only:
if !host_capable() { return None; }
let path = crate::core::rules::get_rules_paths().into_iter().find(|p| p.exists())?;  // no rules.toml → None
let rules = match crate::core::rules::parse_rules(&path) { Ok(r) => r, Err(_) => return None }; // malformed → None
Some(crate::core::rules::evaluate(&rules, &window_info.app_class, &window_info.title,
                                  &callback_names(), board_has_rules()))

// THE call-site deref (both send blocks): pass the locked Box<dyn Notifier> as &dyn Notifier:
let notifier = get_notifier();
let notifier = notifier.lock().unwrap();
let _res = dispatch_window_send(&**notifier, &filter, &message, ctx, "debounced", verbose);
```

### Integration Points

```yaml
MODULE REGISTRATION: NONE. pub mod notifier is long-standing. This task adds items to
  the BODY of notifier.rs (helpers + static + tests) and rewrites 2 fn bodies + adds 2
  lines to S1's landed functions.

DEPENDENCIES (this task): qmk_notifier v0.3.0 (ALREADY pinned — G1), std
  {thread::sleep, time::{Duration, Instant}}, std::error::Error, AtomicBool/Ordering
  (imported by S1 at L6), Notifier trait, configured_filter, crate::core::rules::{
  HostContext, evaluate, get_rules_paths, parse_rules}, crate::core::now_ms. NO new Cargo.

UPSTREAM (consumed unchanged — all VERIFIED landed):
  - PendingMessage{payload, window_info} + worker/immediate seams (P4.M1.T2.S2).
  - Notifier::send_command (P4.M1.T1.S1) + MockNotifier recorder/queue (P4.M2.T1.S1).
  - host_capable()/callback_names()/perform_handshake/reset_handshake_state (P4.M2.T1.S1).

DOWNSTREAM CONSUMERS: NONE in the current plan (terminal host-rules send task).
  P5 CLI/tray read host_capable()/callback_names() but do NOT change the send path.

CONFIG: none. ROUTES/CLI: none (P5). DATABASE: none. TRAY: none (P5).
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# EXPECT: clean. G1: v0.3.0 resolves. If a typed-variant error appears, confirm the crate
#   fetched v0.3.0 (network/tag), not a code bug.

# Confirm the edits landed at the right anchors:
grep -n 'fn dispatch_window_send\|fn send_legacy_string\|fn send_host_context' src/core/notifier.rs  # 3
grep -n 'fn host_context_for_window\|fn host_context_command\|fn clear_host_context_command' src/core/notifier.rs  # 3
grep -n 'static BOARD_HAS_RULES\|pub fn board_has_rules' src/core/notifier.rs  # 2
grep -n 'BOARD_HAS_RULES.store' src/core/notifier.rs  # 2 (Info arm after L332 + reset)
grep -n 'dispatch_window_send(&\*\*notifier' src/core/notifier.rs  # 2 (worker + immediate)

cargo clippy --bin qmkonnect --no-deps 2>/dev/null || true   # optional; no NEW warnings.
```

### Level 2: Unit Tests (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# The 4 orchestration tests (single-threaded MANDATORY — G6):
cargo test --bin qmkonnect dispatch_window_send -- --test-threads=1
# EXPECT: 4 passed — None⇒string-only, stack⇒string+context(clear:false),
#   replace⇒context-only(clear:true), no-match⇒context-clear(layer:None).

# The gate + full-path tests:
cargo test --bin qmkonnect host_context_for_window -- --test-threads=1   # 1 passed (None when !capable)
cargo test --bin qmkonnect test_notify_qmk_legacy_string_when_not_capable -- --test-threads=1  # 1 passed
```

### Level 3: Full-crate regression (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# EXPECT: ALL bin tests green — the 6 new + S1's handshake tests + handshake_action (S2, if landed) +
#   the debounce tests (string bytes UNCHANGED via send_legacy_string — G3) + the 5
#   test_send_command_* (P4.M1.T1.S1) + pattern (P2) + rules (P3) + types + linux_tray.
#   Proves the rewrite preserved the string path AND added the host path without regressions.

git status --short && git diff --stat
# EXPECT: exactly src/core/notifier.rs. NOTHING in Cargo.toml, rules.rs, types.rs,
#   main.rs, platforms/, runners/, tray.rs, linux_tray.rs.
```

### Level 4: End-to-end / contract validation

```bash
# Gate 1 — "only one file changed":
git diff --stat   # expect ONLY src/core/notifier.rs.

# Gate 2 — "notify_qmk signature unchanged" (7 platform callers safe):
git diff src/core/notifier.rs | grep -E '^\+.*pub fn notify_qmk|^-.*pub fn notify_qmk'  # expect EMPTY.

# Gate 3 — "legacy string bytes unchanged" (G3) — the existing debounce test is the proof:
cargo test --bin qmkonnect notifier::tests::test_debounce_subsequent_messages -- --test-threads=1
# EXPECT: PASS — MOCK_LAST_MESSAGE still "App2\x1DTitle2".

# Gate 4 — "string NOT sent in replace/no-match" (G5) — the orchestration tests are the proof:
cargo test --bin qmkonnect test_dispatch_replace_sends_context_only -- --test-threads=1
cargo test --bin qmkonnect test_dispatch_no_match_sends_clear_context -- --test-threads=1
# EXPECT: both PASS — MOCK_CALL_COUNT == 0 (no notify call) in each.

# Gate 5 — "board_has_rules stored by the handshake" (G8):
grep -A1 'HOST_CAPABLE.store(true' src/core/notifier.rs | grep 'BOARD_HAS_RULES.store(board_rules_present'  # expect 1 hit.

# Live check (optional, needs a v2-capable board + a rules.toml): cargo run -- -v,
#   focus a window matching a rule, watch for "sent host context" verbose lines + the
#   firmware applying the host layer/callbacks.
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean (G1: v0.3.0 resolves; no NEW warnings).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` green (6 new + all existing; G6).
- [ ] `git diff --stat` = `src/core/notifier.rs` ONLY (Gate 1).
- [ ] (optional) `cargo clippy --bin qmkonnect --no-deps` introduces no NEW warnings.

### Feature Validation (contract fidelity — HOST_RULES.md §8(4))
- [ ] **Stack** (any_match && !clear_board): `notify` called (string) THEN `ApplyHostContext{layer,callbacks,clear_board:false}`.
- [ ] **Replace** (any_match && clear_board): `notify` NOT called; ONLY `ApplyHostContext{...,clear_board:true}`.
- [ ] **No match** (!any_match): `notify` NOT called; ONLY `ApplyHostContext{layer:None,callbacks:[],clear_board:false}`.
- [ ] **Disabled** (not capable / no rules.toml / malformed): legacy string ONLY (today's behavior).
- [ ] `board_has_rules()` is stored from the handshake's `board_rules_present` and passed into `evaluate`.
- [ ] `send_host_context` retries 3× for device errors then swallows (§5.4 parity).

### Code Quality Validation
- [ ] Legacy string bytes + cadence unchanged (Gate 3 green; G3).
- [ ] Worker swallows the string error; immediate propagates it (`?`) — both preserved (G4).
- [ ] String NOT sent in replace/no-match (Gate 4 green; G5).
- [ ] `notify_qmk` signature unchanged (Gate 2; 7 callers safe).
- [ ] `&**notifier` used (not `as_ref`) to pass `&dyn Notifier` (G7).
- [ ] Only 2 additive lines touch S1's LANDED handshake code (G8); the static/accessor live in this task's band.
- [ ] No out-of-scope work: no Cargo/CLI/tray/runner/rules.rs/types.rs edits.

### Documentation & Deployment
- [ ] Helpers have Mode-A rustdoc (`rust,ignore` fences — binary crate, no lib doctests).
- [ ] Inline comments at both send blocks note "routes through dispatch_window_send (HOST_RULES.md §8(4))".
- [ ] Commit message notes: "completes host-rules send pipeline — stack/replace/no-match in both debounce paths; stores board_rules_present for evaluate; legacy string-only path preserved bit-for-bit."

---

## Anti-Patterns to Avoid

- ❌ Don't send the legacy string UNCONDITIONALLY before the host decision — in
  replace/no-match it must NOT be sent (G5). Evaluate FIRST, branch on
  `(ctx.any_match, ctx.clear_board)`.
- ❌ Don't change the string bytes or each call site's error-propagation policy — the
  worker swallows, the immediate propagates; `dispatch_window_send` returns the
  legacy-string `Result` so each site keeps its policy (G3/G4).
- ❌ Don't add retry to `QmkNotifier::send_command` — its rustdoc (P4.M1.T1.S1) says
  retry is the CALLER's job. Put the retry in `send_host_context` (this task).
- ❌ Don't make the host-context send propagate errors into the window-send result —
  it swallows after §5.4 retry parity (the string, if sent, already went out; a typed-
  command failure must not fail the overall send).
- ❌ Don't touch rules.rs/types.rs/the trait/DebounceState/notify_qmk signature — this
  is a single-file, additive-in-scope change (G2).
- ❌ Don't run tests multi-threaded — `--test-threads=1` is mandatory (G6).
- ❌ Don't leave `board_rules_present` un-stored — `evaluate` needs `board_has_rules`;
  add the static + accessor + the 2 handshake lines (G8, VERIFIED gap).
- ❌ Don't try to assert cross-channel ordering (string-before-context) via the mock —
  it records `notify` and `send_command` in separate channels; the ordering is
  structurally guaranteed by the source order in `dispatch_window_send`'s stack arm.
- ❌ Don't edit S1's LANDED handshake beyond the 2 additive `.store(...)` lines — the
  static and accessor live in this task's own band; S1's structural code is untouched.
# Research Notes — P4.M3.T1.S1: Implement host-context evaluation and send in debounce worker

> Scope: **`src/core/notifier.rs` ONLY** (single file). This task completes the
> host-side-rules pipeline end-to-end: in BOTH send blocks (the debounce worker
> flush + `notify_qmk`'s immediate-send path) it evaluates `rules.toml` against
> the window and emits `APPLY_HOST_CONTEXT` per HOST_RULES.md §8(4) — string-first
> (stack), context-only (replace), or context-clear (no-match). The legacy string
> bytes + cadence are preserved bit-for-bit when host rules are disabled.
>
> **Single file** because: `rules.rs` (evaluate/HostContext) is a read-only
> consumer; `types.rs` already has `Clone` (P4.M1.T2.S2 — landed); the crate is
> pinned (P4.M1.T2.S1 — landed); no CLI/tray (P5). The item note says "DOCS: none
> — internal pipeline logic."
>
> **STATE NOTE:** P4.M2.T1.S1 (the handshake) has **LANDED** in `notifier.rs`
> (verified — see §0). P4.M2.T1.S2 (runner/poll integration) is being implemented
> in parallel and edits runners/tray/linux_tray + appends `handshake_action` to
> notifier.rs — NOT the send blocks or perform_handshake body this task touches
> (disjoint regions). Implementation of THIS task is sequential after P4.M2, so
> the 2 additive lines this task adds to `perform_handshake`/`reset_handshake_state`
> are edits to already-merged code — no merge conflict.

---

## §0 — Dependency contract (VERIFIED against the real, landed code)

File is now **1363 lines**. All anchors below are REAL line numbers in the current
`src/core/notifier.rs`.

### Inputs available NOW (verified by grep):

| Symbol | Source | Verified location / shape |
|---|---|---|
| `PendingMessage { payload: String, window_info: WindowInfo }` | P4.M1.T2.S2 (**LANDED**) | private struct; worker flush at L600 `if let Some((pm, verbose)) = to_send {`, L604 `let message = pm.payload;` partial-move, L602-603 comment names `pm.window_info` as the seam. |
| `Notifier::send_command(&self, RunCommand, &DeviceFilter) -> Result<CommandResponse, Box<Error+Send+Sync>>` | P4.M1.T1.S1 (**LANDED**) | trait L53; `QmkNotifier` impl L506 (thin transport, NO retry); `MockNotifier` impl L788 records into `MOCK_SEND_COMMAND_CALLS` then pops `MOCK_RESPONSES` (fallback `Ack{ok:true}`). |
| `host_capable() -> bool` | P4.M2.T1.S1 (**LANDED**) | L440. Reads `HOST_CAPABLE: AtomicBool` (L203-ish). |
| `callback_names() -> HashMap<String,u8>` | P4.M2.T1.S1 (**LANDED**) | L447-448. Clone of `CALLBACK_NAMES` (L208). Empty when not capable. |
| `perform_handshake(verbose)`, `reset_handshake_state()` | P4.M2.T1.S1 (**LANDED**) | perform_handshake L265; reset_handshake_state L456-459 (clears HOST_CAPABLE/CALLBACK_NAMES/HAS_HANDSHAKED). |
| `MockNotifier::set_mock_responses(Vec)` + `MOCK_RESPONSES` queue | P4.M2.T1.S1 (**LANDED**) | static L744; accessor L774; `send_command` pops at L797. **The mock ALWAYS returns `Ok(..)` — never `Err`.** ⇒ retry-error path NOT exercisable via mock (see §6). |
| `rules::evaluate(&RuleSet, &str, &str, &HashMap<String,u8>, board_has_rules: bool) -> HostContext` | P3.M1.T2.S1 (**LANDED**) | `src/core/rules.rs`. Pure. |
| `rules::HostContext { layer: Option<u8>, callback_ids: Vec<u8>, clear_board: bool, any_match: bool }` | P3.M1.T2.S1 (**LANDED**) | derives `Debug, Clone, PartialEq`. |
| `rules::get_rules_paths()`, `rules::parse_rules(&Path)` | P3.M1.T1 (**LANDED**) | platform config-dir candidates with `rules.toml` filename. |

### Module imports (L1-8) — VERIFIED, this task adds NOTHING:
```rust
use crate::core::types::WindowInfo;
use once_cell::sync::Lazy;
use std::collections::{BTreeSet, HashMap};
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};   // ← S1 added; BOARD_HAS_RULES reuses
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};
```
`AtomicBool` + `Ordering` are ALREADY imported ⇒ my `BOARD_HAS_RULES: AtomicBool`
needs no new import.

### The `board_rules_present` GAP — VERIFIED REAL in the landed code

`perform_handshake`'s capable `Info` arm (L281-340) **destructures `board_rules_present`
(L286) and only LOGS it (L291 in the verbose eprintln)**. It then does the SET_OS
sweep, validates, and at **L332 `HOST_CAPABLE.store(true, Ordering::SeqCst);`** —
but there is **NO** `BOARD_HAS_RULES.store(board_rules_present, ...)`. So the
firmware's board-rules bit is discarded.

But `rules::evaluate()` REQUIRES `board_has_rules: bool` (it folds the bit into
`clear_board = all_disabling || !board_has_rules`). This task CLOSES the gap:
- Add `static BOARD_HAS_RULES: AtomicBool` + `pub fn board_has_rules() -> bool`
  in THIS task's band (the new helper region, not S1's handshake block).
- **2 additive edits to S1's landed code** (sequential ⇒ no conflict):
  1. in `perform_handshake`'s capable arm, **immediately after L332
     `HOST_CAPABLE.store(true, Ordering::SeqCst);`** add:
     `BOARD_HAS_RULES.store(board_rules_present, Ordering::SeqCst);`
  2. in `reset_handshake_state` (L456-459) add, alongside the existing clears:
     `BOARD_HAS_RULES.store(false, Ordering::SeqCst);`
- `board_has_rules()` is ONLY consulted when `host_capable()` is true (the send
  gate), so a stale value on a non-capable board is never read. The else/Err arms
  (L343, L355) of `perform_handshake` need NO edit (host_capable=false there ⇒
  board_has_rules never consulted). Reset clears it for reconnect hygiene.

---

## §1 — The crate API this task codes to (qmk_notifier v0.3.0, pinned)

Confirmed from the EXISTING landed tests in `notifier.rs` (e.g.
`test_send_command_records_call_sequence` constructs these variants and compiles
against the pinned tag) + P4.M2.T1.S1 research §1:

```rust
pub enum RunCommand {
    SendMessage(String),
    ListDevices,
    QueryInfo, QueryCallback(u8), SetOs(HostOs),
    ApplyHostContext { layer: Option<u8>, callbacks: Vec<u8>, clear_board: bool },
}
#[repr(u8)] pub enum HostOs { Unsure=0, Linux=1, Windows=2, Macos=3, Ios=4 }  // ⚠ Macos lowercase
pub enum CommandResponse {
    Legacy { matched: bool },
    Info { proto_ver: u8, feature_flags: u8, callback_count: u8, board_rules_present: bool },
    CallbackName { index: u8, name: Option<String> },
    Ack { ok: bool },
    Timeout,
}
// Both enums: #[derive(Debug, Clone, PartialEq, Eq)]  -> command.clone() in the retry loop is sound.
```

**BUILD PRECONDITION (G1):** the qmk_notifier v0.3.0 git tag MUST be reachable
(`cargo fetch`). The build resolves it (P4.M1.T2.S1 = Complete; the landed notifier.rs
already references these v0.3.0-only shapes and compiles). If `cargo build` fails
to fetch the tag, that is an ENVIRONMENT/network issue (re-fetch / check git
remote), NOT a code issue. `cargo build` is the gate.

---

## §2 — Canonical send logic (HOST_RULES.md §8(4) — verbatim)

> For one debounced window change:
> - **Stack** (board has rules AND ≥1 matched rule non-disabling): send the
>   **string** first (`RunCommand::SendMessage`), await its `CommandResponse`,
>   then `ApplyHostContext { layer: L_h, callbacks, clear_board: false }`.
> - **Replace** (all matched rules disabling, OR board has no rules): send **only**
>   `ApplyHostContext { layer: L_h, callbacks, clear_board: true }` (no string).
> - **No match:** `ApplyHostContext { layer: None (0xFF), callbacks: empty,
>   clear_board: <per flag> }` — always clear host layer + disable all host callbacks.
> - Retry/cache parity with `SendMessage`.

The item contract maps `HostContext` → wire precisely:
| `ctx.any_match` | `ctx.clear_board` | action |
|---|---|---|
| `true` | `false` | **stack**: `notify(payload)` THEN `ApplyHostContext{layer, callbacks, clear_board:false}` |
| `true` | `true`  | **replace**: `ApplyHostContext{layer, callbacks, clear_board:true}` ONLY (no string) |
| `false` | (false) | **no-match**: `ApplyHostContext{layer:None, callbacks:[], clear_board:false}` ONLY (no string) |

`evaluate()` already folds `board_has_rules` into `clear_board`
(`clear_board = all_disabling || !board_has_rules`) and short-circuits no-match to
`{clear_board:false, any_match:false}` (rules.rs — avoids the vacuous-`all()`-true
bug). So the send logic branches ONLY on `(any_match, clear_board)`.

**Retry parity (§5.4):** `QmkNotifier::notify` (SendMessage) retries 3× for device
errors (`no device found` / `permission denied` / `failed to open`) then SWALLOWS
(returns `Ok`); non-device errors return `Err` immediately. `QmkNotifier::send_command`
is a thin transport (NO retry — per trait rustdoc it's the caller's job). ⇒ THIS
task wraps the `ApplyHostContext` send in the SAME 3-attempt device-error retry,
swallowed. See `send_host_context` in §3.

---

## §3 — Design: extract a shared send-orchestration helper (both call sites)

P4.M1.T2.S2 explicitly DEFERRED the helper extraction to THIS task ("P4.M3.T1.S1
which adds identical logic to both is the right moment to decide on a helper").
The two send blocks differ only in (a) the verbose log label ("debounced" vs
"immediate") and (b) error propagation (worker SWALLOWS the string error; immediate
PROPAGATES via `?`). Both differences are preserved by a helper that takes
`label: &str` and RETURNS the legacy-string `Result` (each call site keeps its
policy). The host-context send swallows its own errors (retry parity) so it never
affects the string-result propagation.

### Helper set (placed in a new band AFTER `notify_qmk` L~730, BEFORE `mod tests`):

```rust
// ── board-rules capability bit (closes the §0 gap) ──
static BOARD_HAS_RULES: AtomicBool = AtomicBool::new(false);
pub fn board_has_rules() -> bool { BOARD_HAS_RULES.load(Ordering::SeqCst) }

// ── evaluate host rules for one window, or None if host rules are disabled ──
fn host_context_for_window(window_info: &WindowInfo, verbose: bool)
    -> Option<crate::core::rules::HostContext>;

// ── the end-to-end send (HOST_RULES.md §8(4)); returns legacy-string Result ──
fn dispatch_window_send(
    notifier: &dyn Notifier,
    filter: &DeviceFilter,
    message: &str,
    ctx: Option<crate::core::rules::HostContext>,
    label: &str,        // "debounced" | "immediate"
    verbose: bool,
) -> Result<(), Box<dyn Error + Send + Sync>>;

// ── string send with the EXACT pre-host-rules verbose log + timing ──
fn send_legacy_string(notifier: &dyn Notifier, message: &str, label: &str, verbose: bool)
    -> Result<(), Box<dyn Error + Send + Sync>>;

// ── typed-context send with SendMessage-style 3-retry device-error parity ──
fn send_host_context(notifier: &dyn Notifier, filter: &DeviceFilter,
                     command: qmk_notifier::RunCommand, verbose: bool);

// ── HostContext -> RunCommand::ApplyHostContext builders ──
fn host_context_command(ctx: &crate::core::rules::HostContext) -> qmk_notifier::RunCommand;
fn clear_host_context_command() -> qmk_notifier::RunCommand;  // no-match: layer=None
```

### `dispatch_window_send` body (the §8(4) branch table):

```rust
fn dispatch_window_send(
    notifier: &dyn Notifier,
    filter: &DeviceFilter,
    message: &str,
    ctx: Option<crate::core::rules::HostContext>,
    label: &str,
    verbose: bool,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match ctx {
        // Host rules disabled (not capable / no rules.toml / malformed): legacy string only.
        None => send_legacy_string(notifier, message, label, verbose),

        // Stack: board runs (≥1 non-disabling matched rule). String first, then context.
        Some(ctx) if ctx.any_match && !ctx.clear_board => {
            let r = send_legacy_string(notifier, message, label, verbose);
            send_host_context(notifier, filter, host_context_command(&ctx), verbose);
            r
        }

        // Replace: all matched rules disabling, or board has no rules. Context only.
        Some(ctx) if ctx.any_match => {
            send_host_context(notifier, filter, host_context_command(&ctx), verbose);
            Ok(())
        }

        // No match: clear host layer + disable all callbacks. No string.
        Some(_) => {
            send_host_context(notifier, filter, clear_host_context_command(), verbose);
            Ok(())
        }
    }
}
```

### `send_legacy_string` (preserves the EXACT current log/timing output):

```rust
fn send_legacy_string(
    notifier: &dyn Notifier,
    message: &str,
    label: &str,
    verbose: bool,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if verbose {
        let sanitized = message.replace('\x1D', "|");
        println!("[{}ms] Notified QMK ({}): {}", crate::core::now_ms(), label, sanitized);
    }
    #[cfg(test)]
    println!("Sending {} notification: {}", label, message);

    let _len = message.len();
    let _t0 = Instant::now();
    let res = notifier.notify(message.to_string());   // notify takes owned String
    let _send_ms = _t0.elapsed().as_millis();
    if verbose {
        eprintln!("[{}ms] send took {}ms ({} bytes)", crate::core::now_ms(), _send_ms, _len);
    }
    res
}
```

### `send_host_context` (SendMessage-style retry parity — §5.4):

```rust
fn send_host_context(
    notifier: &dyn Notifier,
    filter: &DeviceFilter,
    command: qmk_notifier::RunCommand,
    verbose: bool,
) {
    // Retry/cache parity with SendMessage (PRD §5.4): up to 3 attempts for device
    // errors, then swallowed. Host-context failures never fail the overall window
    // send (the legacy string, if any, already went out).
    for attempt in 1..=3 {
        match notifier.send_command(command.clone(), filter) {
            Ok(_) => {
                if verbose {
                    eprintln!(
                        "[{}ms] sent host context (attempt {}): {:?}",
                        crate::core::now_ms(), attempt, command
                    );
                }
                return;
            }
            Err(e) => {
                let s = e.to_string().to_lowercase();
                if s.contains("no device found")
                    || s.contains("permission denied")
                    || s.contains("failed to open")
                {
                    if attempt < 3 {
                        thread::sleep(Duration::from_millis(100 * attempt as u64));
                        continue;
                    }
                    eprintln!(
                        "QMK device unavailable after {} attempts sending host context: {}",
                        attempt, e
                    );
                    return; // swallowed (parity with notify's device-error swallow)
                }
                eprintln!("Error sending host context: {}", e);
                return; // non-device error: log + swallow (don't fail the window send)
            }
        }
    }
}
```

### `host_context_for_window` (the IO gate; None ⇒ legacy string-only):

```rust
fn host_context_for_window(window_info: &WindowInfo, verbose: bool)
    -> Option<crate::core::rules::HostContext>
{
    if !host_capable() {
        return None;  // legacy/offline -> string-only (today's behavior)
    }
    let path = crate::core::rules::get_rules_paths().into_iter().find(|p| p.exists())?;
    let rules = match crate::core::rules::parse_rules(&path) {
        Ok(r) => r,
        Err(e) => {
            if verbose {
                eprintln!(
                    "Warning: could not parse {}: {} — host rules disabled for this window",
                    path.display(), e
                );
            }
            return None;  // malformed -> graceful string-only fallback
        }
    };
    let names = callback_names();
    Some(crate::core::rules::evaluate(
        &rules,
        &window_info.app_class,
        &window_info.title,
        &names,
        board_has_rules(),
    ))
}
```

### `host_context_command` / `clear_host_context_command`:

```rust
fn host_context_command(ctx: &crate::core::rules::HostContext) -> qmk_notifier::RunCommand {
    qmk_notifier::RunCommand::ApplyHostContext {
        layer: ctx.layer,
        callbacks: ctx.callback_ids.clone(),
        clear_board: ctx.clear_board,
    }
}

fn clear_host_context_command() -> qmk_notifier::RunCommand {
    // No-match: always clear the host layer (0xFF) + disable all host callbacks.
    qmk_notifier::RunCommand::ApplyHostContext {
        layer: None,
        callbacks: vec![],
        clear_board: false,
    }
}
```

---

## §4 — The two call-site edits (verbatim CURRENT anchors → NEW)

### 4.1 Debounce worker flush — CURRENT (notifier.rs L600-635, inside `fn debounce_worker`)

```rust
        if let Some((pm, verbose)) = to_send {
            // `pm` carries the formatted payload (sent below) AND the originating
            // WindowInfo. P4.M3.T1.S1 consumes `pm.window_info` here to evaluate
            // rules.toml and emit APPLY_HOST_CONTEXT alongside the string send.
            let message = pm.payload; // partial move -> String; pm.window_info remains for P4.M3.T1.S1
            if verbose {
                let sanitized = message.replace('\x1D', "|");
                println!(
                    "[{}ms] Notified QMK (debounced): {}",
                    crate::core::now_ms(),
                    sanitized
                );
            }
            #[cfg(test)]
            println!("Sending debounced notification: {}", message);
            let notifier = get_notifier();
            let notifier = notifier.lock().unwrap();
            let _len = message.len();
            let _t0 = Instant::now();
            let _res = notifier.notify(message);
            let _send_ms = _t0.elapsed().as_millis();
            if verbose {
                eprintln!(
                    "[{}ms] send took {}ms ({} bytes)",
                    crate::core::now_ms(),
                    _send_ms,
                    _len
                );
            }
            if let Err(e) = _res {
                eprintln!("Error sending debounced notification: {}", e);
            }
        }
```

NEW (verbose log/timing moves INTO `send_legacy_string` with label="debounced"):
```rust
        if let Some((pm, verbose)) = to_send {
            // Host-rules send (P4.M3.T1.S1 / HOST_RULES.md §8(4)): evaluate
            // rules.toml against this window and, when host-capable, emit
            // ApplyHostContext alongside (stack) or instead of (replace/no-match)
            // the legacy string. Legacy string bytes + cadence are unchanged.
            let PendingMessage { payload: message, window_info } = pm;
            let filter = configured_filter();
            let ctx = host_context_for_window(&window_info, verbose);
            let notifier = get_notifier();
            let notifier = notifier.lock().unwrap();
            let _res = dispatch_window_send(
                &**notifier, &filter, &message, ctx, "debounced", verbose,
            );
            if let Err(e) = _res {
                eprintln!("Error sending debounced notification: {}", e);
            }
        }
```

### 4.2 `notify_qmk` immediate-send block — CURRENT (notifier.rs L691-720)

```rust
    if send_immediately {
        if verbose {
            let sanitized = message.replace('\x1D', "|");
            println!(
                "[{}ms] Notified QMK (immediate): {}",
                crate::core::now_ms(),
                sanitized
            );
        }
        #[cfg(test)]
        println!("Sending notification immediately: {}", message);
        let notifier = get_notifier();
        let notifier = notifier.lock().unwrap();
        let _len = message.len();
        let _t0 = Instant::now();
        let _res = notifier.notify(message);
        let _send_ms = _t0.elapsed().as_millis();
        if verbose {
            eprintln!(
                "[{}ms] send took {}ms ({} bytes)",
                crate::core::now_ms(),
                _send_ms,
                _len
            );
        }
        _res?;
    } else if verbose {
        let sanitized = message.replace('\x1D', "|");
        println!(
            "[{}ms] Debouncing notification: {}",
            crate::core::now_ms(),
            sanitized
        );
    }
```

NEW (immediate path PROPAGATES the string result via `?` — preserved; label="immediate"):
```rust
    if send_immediately {
        let filter = configured_filter();
        let ctx = host_context_for_window(window_info, verbose);
        let notifier = get_notifier();
        let notifier = notifier.lock().unwrap();
        let _res = dispatch_window_send(
            &**notifier, &filter, &message, ctx, "immediate", verbose,
        );
        _res?;
    } else if verbose {
        let sanitized = message.replace('\x1D', "|");
        println!(
            "[{}ms] Debouncing notification: {}",
            crate::core::now_ms(),
            sanitized
        );
    }
```
`window_info` is the `notify_qmk` param (`&WindowInfo`) — in scope throughout. `message`
is the local `String` — pass `&message`.

### 4.3 `&**notifier` deref

`notifier` is `MutexGuard<'_, Box<dyn Notifier>>`. `&**notifier` → `&dyn Notifier`
(`*g` → `Box<dyn Notifier>`; `**g` → `dyn Notifier`; `&**g` → `&dyn Notifier`).
Method calls on `notifier.notify(..)` today rely on auto-deref; passing as a
`&dyn Notifier` arg needs the explicit `&**notifier`. (`as_ref()` returns `&Box<_>`
— wrong type — do NOT use it.)

---

## §5 — The 2 edits to S1's LANDED handshake (sequential, additive)

After S1 landed, `perform_handshake`'s capable `Info` arm has at **L332**:
```rust
            HOST_CAPABLE.store(true, Ordering::SeqCst);
```
ADD immediately AFTER that line:
```rust
            BOARD_HAS_RULES.store(board_rules_present, Ordering::SeqCst);
```
(`board_rules_present` is already destructured in that arm at L286 — S1 logs it at L291.)

And in `reset_handshake_state` (**L456-459**, which currently clears HOST_CAPABLE /
CALLBACK_NAMES / HAS_HANDSHAKED), ADD:
```rust
    BOARD_HAS_RULES.store(false, Ordering::SeqCst);
```

These are the ONLY touches to S1's code. They reference `BOARD_HAS_RULES` which
THIS task declares elsewhere in the module (module-level `static` ⇒ in scope
everywhere in `notifier.rs`).

---

## §6 — Test plan (hermetic; inject HostContext into `dispatch_window_send`)

The send ORCHESTRATION is tested by injecting `ctx: Option<HostContext>` directly
into `dispatch_window_send` — no rules.toml file control needed (the gate IO is
covered separately; evaluate() correctness is P3.M1.T2.S1's job, already green).

Setup helper (each test): `reset_test_state(); reset_handshake_state();
set_notifier(Box::new(MockNotifier::new()));` then lock the global notifier and
pass `&**guard` as `&dyn Notifier`. The mock records `notify`→`MOCK_CALL_COUNT`/
`MOCK_LAST_MESSAGE`, `send_command`→`MOCK_SEND_COMMAND_CALLS`; returns `Ok(Ack)`
(always — see §0 mock note).

| # | Test | ctx injected | assert notify count | assert send_command |
|---|---|---|---|---|
| 1 | `test_dispatch_legacy_string_only_when_no_host_context` | `None` | `==1`, last msg `"App\x1DTitle"` | empty (0) |
| 2 | `test_dispatch_stack_sends_string_then_context` | `Some({layer:Some(224),cb:[0,1],clear:false,any:true})` | `==1` | len 1 == `ApplyHostContext{layer:Some(224),cb:[0,1],clear:false}` |
| 3 | `test_dispatch_replace_sends_context_only` | `Some({layer:Some(225),cb:[2],clear:true,any:true})` | `==0` (NO string) | len 1 == `ApplyHostContext{...,clear:true}` |
| 4 | `test_dispatch_no_match_sends_clear_context` | `Some({layer:None,cb:[],clear:false,any:false})` | `==0` (NO string) | len 1 == `ApplyHostContext{layer:None,cb:[],clear:false}` |
| 5 | `test_host_context_for_window_none_when_not_capable` | (gate; call `host_context_for_window` directly with `host_capable()==false`) | — | returns `None` |
| 6 | `test_notify_qmk_legacy_string_when_not_capable` | (full `notify_qmk` path; `host_capable()==false`) | `==1` after wait | empty |

**Ordering caveat (document, don't workaround):** the mock records `notify` and
`send_command` in SEPARATE channels, so a cross-channel "string before context"
order assertion isn't feasible without editing S1's mock (out of scope). The §8(4)
ordering (string-first in stack) is STRUCTURALLY guaranteed — `dispatch_window_send`
calls `send_legacy_string` BEFORE `send_host_context` in the stack arm (source
order = execution order). Test 2 asserts BOTH happened (count 1 + 1); the order is
enforced by the code path.

**Retry path not exercisable via mock:** `MockNotifier::send_command` always
returns `Ok` (§0). The 3-retry device-error swallow in `send_host_context` is a
faithful copy of `QmkNotifier::notify`'s proven retry (same predicate strings,
same backoff). Verified by code inspection + the real-impl device-not-found test
pattern (`test_qmk_notifier_send_command_maps_device_not_found`).

**Existing tests that MUST stay green** (regression proof for string-bytes + the
mock seam): `test_immediate_send_first_message`, `test_debounce_subsequent_messages`,
`test_send_after_debounce_timeout`, `test_debounced_pending_carries_window_info`,
`test_send_command_records_call_sequence`.

---

## §7 — Gotchas (G1–G9)

- **G1 (build precondition):** qmk_notifier v0.3.0 must resolve. The landed
  notifier.rs already uses `ApplyHostContext`/`Info`, so `cargo build` is the gate.
  A fetch failure here is an env/network issue.
- **G2 (single file):** edit ONLY `src/core/notifier.rs`. Do NOT touch rules.rs
  (evaluate is read-only), types.rs (Clone already present), Cargo, or any
  platform/tray/runner/CLI file.
- **G3 (string bytes unchanged):** `send_legacy_string` calls `notifier.notify`
  with the IDENTICAL formatted string (`"{app_class}\x1D{title}"`). The existing
  debounce tests assert `MOCK_LAST_MESSAGE == "App2\x1DTitle2"` — they must stay
  green. The `.to_string()` (notify needs owned String) does not change bytes.
- **G4 (error propagation preserved):** worker SWALLOWS the string error
  (`if let Err(e) = _res { eprintln }` at L633); immediate PROPAGATES (`_res?` at
  L718). Both are preserved because `dispatch_window_send` RETURNS the legacy-string
  `Result` and each call site handles it as before. The host-context send swallows
  its own errors (retry parity) so it never changes the string-result propagation.
- **G5 (replace/no-match send NO string):** in those branches `MOCK_CALL_COUNT`
  must stay 0. Do NOT unconditionally send the string — evaluate FIRST, branch.
- **G6 (single-threaded tests):** `cargo test --bin qmkonnect -- --test-threads=1`
  (process-global STATE/COND/WORKER/NOTIFIER/HANDSHAKE globals).
- **G7 (`&**notifier` not `as_ref`):** passing the locked `Box<dyn Notifier>` as a
  `&dyn Notifier` arg uses `&**notifier`. `as_ref()` returns `&Box<_>` (wrong type).
- **G8 (board_rules_present gap — VERIFIED):** see §0. Add the static + accessor
  here; 2 additive lines in S1's LANDED handshake (L332 + reset_handshake_state).
  `Ordering`/`AtomicBool` already imported (L6). board_has_rules() is only read
  when host_capable() is true, so a stale value on a non-capable board is never
  consulted.
- **G9 (verbose log label):** the old "Notified QMK (debounced)" / "(immediate)"
  strings + "send took Xms" timing move INTO `send_legacy_string` (label param) —
  output unchanged for the string-sending branches. In replace/no-match NO string
  is sent so no "Notified QMK" log prints (correct — don't log a string that wasn't
  sent); `send_host_context` has its own terse verbose line.

---

## §8 — Validation

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect                                        # G1: v0.3.0 resolves
cargo test --bin qmkonnect -- --test-threads=1                     # G6: ALL green (6 new + existing)
cargo test --bin qmkonnect dispatch_window_send -- --test-threads=1  # the orchestration tests
git diff --stat                                                    # expect ONLY src/core/notifier.rs
```
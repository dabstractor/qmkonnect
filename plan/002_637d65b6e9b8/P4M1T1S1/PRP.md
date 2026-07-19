# PRP — P4.M1.T1.S1: Add `send_command()` to `Notifier` trait + `QmkNotifier` impl

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This task ADDS typed-command transport to a
> SINGLE file, `src/core/notifier.rs`: (1) a new `fn send_command(...)` method on
> the `Notifier` trait, (2) its `QmkNotifier` impl (builds `RunParameters` → calls
> `qmk_notifier::run` → maps `QmkError` → returns `CommandResponse`), and (3) a
> `MockNotifier` impl that records the call SEQUENCE (for P4.M3 ordering
> assertions) + a `reset_test_state()` extension that clears the log.
> **Consumes:** the `qmk_notifier` v0.3.0 public API (`RunCommand`,
> `CommandResponse`, `HostOs`, `RunParameters`, `run`, `QmkError`) — whose exact
> types are reproduced verbatim in `research/notes.md` §1 and the Context block
> below. **Consumed downstream by:** P4.M2.T1.S1 (handshake: `QueryInfo` /
> `QueryCallback` / `SetOs`) and P4.M3.T1.S1 (host-context send:
> `ApplyHostContext`, which asserts "string before context" ordering via this
> mock's call log).
>
> **PARALLEL-EXECUTION NOTE:** P3.M1.T2.S1 (the rules `evaluate()` engine) is
> being implemented concurrently and touches `src/core/rules.rs` ONLY. This task
> touches `src/core/notifier.rs` ONLY. The two files are disjoint — no merge
> conflict. `evaluate()`/`HostContext` are NOT consumed here (P4.M3.T1.S1 is the
> bridge that calls both `evaluate()` AND `send_command(ApplyHostContext{…})`).

---

## ⚠️ READ FIRST — the compile gate (G1)

This task references `qmk_notifier` **v0.3.0** types (`RunCommand::QueryInfo`,
`CommandResponse`, `HostOs`, …). `Cargo.toml:16` currently pins **v0.2.1**, which
has NONE of those types (only `SendMessage`/`ListDevices`). v0.3.0 is not yet
tagged/pushed (P1.M1.T4.S1 = "Ready", P4.M1.T2.S1 = "Planned"). **Before running
any `cargo build`/`cargo test` validation, apply the temporary path override in
the Validation Loop Step 0** — point qmk_notifier at the local v0.3.0 working
tree. The code changes themselves are notifier.rs-only; the override is a dev
expedient so the typed types resolve. See `research/notes.md` §2 for full detail.

---

## Goal

**Feature Goal**: Extend the `Notifier` trait in `src/core/notifier.rs` with a
second method, `fn send_command(&self, command: qmk_notifier::RunCommand, filter:
&DeviceFilter) -> Result<qmk_notifier::CommandResponse, Box<dyn Error + Send +
Sync>>`, implemented for both `QmkNotifier` (the real transport: builds
`RunParameters` from `command` + `filter`, calls `qmk_notifier::run`, maps
`QmkError` → boxed error, returns the `CommandResponse`) and the test
`MockNotifier` (records the call sequence into a `Vec<RunCommand>` static for
ordering assertions; returns an optimistic `Ok(Ack{ok:true})` default). `notify()`
is UNCHANGED (legacy string path). Mode-A rustdoc on `send_command`.

**Deliverable** (additions to `src/core/notifier.rs` ONLY):
1. **`Notifier` trait** (currently 1 method, lines 12–14) gains a second, REQUIRED
   method `send_command` (exact signature below) + Mode-A `///` rustdoc.
2. **`impl Notifier for QmkNotifier`** (currently `notify` only, lines 142–186)
   gains `send_command` — a thin transport wrapper: `RunParameters::new(command,
   f.vendor_id, f.product_id, f.usage_page, f.usage, false)` → `qmk_notifier::run`
   → `Ok(resp)` / `Err(Box::new(e))`. NO retry loop (D2).
3. **`MockNotifier`** (in `#[cfg(test)] mod tests`, lines ~399–415) gains a
   parallel `send_command` impl that pushes `command.clone()` into a new
   `MOCK_SEND_COMMAND_CALLS: Lazy<StdMutex<Vec<qmk_notifier::RunCommand>>>`
   static and returns `Ok(CommandResponse::Ack { ok: true })`.
4. **`reset_global_mock()`** gains one line: `MOCK_SEND_COMMAND_CALLS.lock().unwrap().clear();`
   (so `reset_test_state()` clears the log transitively — G7).
5. **A new `MockNotifier::get_send_command_calls() -> Vec<qmk_notifier::RunCommand>`**
   accessor (mirrors the existing `get_call_count`/`get_last_message`).
6. **5 NEW tests** appended to `#[cfg(test)] mod tests`, prefixed
   `test_send_command_*` (see Implementation Tasks Task 6).
7. **NO other files change** (modulo the temporary G1 path override for validation).

**Success Definition**:
- `Notifier` trait has exactly two methods: `notify` (unchanged) + `send_command`
  (exact signature, required, no default body).
- `QmkNotifier::send_command` calls `qmk_notifier::run` exactly once (no retry),
  maps `QmkError` via `Box::new(e)`, and returns the `CommandResponse` unchanged.
- `MockNotifier::send_command` appends `command.clone()` to
  `MOCK_SEND_COMMAND_CALLS` in call order and returns `Ok(Ack { ok: true })`.
- `reset_test_state()` (via `reset_global_mock()`) empties
  `MOCK_SEND_COMMAND_CALLS` alongside `MOCK_CALL_COUNT`/`MOCK_LAST_MESSAGE`.
- `cargo build --bin qmkonnect` clean (with the G1 path override in place); no NEW
  warnings.
- `cargo test --bin qmkonnect -- --test-threads=1` green: the 6 existing tests
  (unchanged) + the 5 new tests + all other crate tests; no regression.
- `git diff --stat` shows `src/core/notifier.rs` as the only source change.

## User Persona (if applicable)

**Target User**: the downstream typed-command consumers in P4:
- **P4.M2.T1.S1 (handshake):** calls `notifier.send_command(QueryInfo, &filter)`
  → matches `CommandResponse::Info { proto_ver: 2, feature_flags, callback_count,
  board_rules_present }`; then loops `send_command(QueryCallback(i), &filter)` to
  build the name→id map; then `send_command(SetOs(host_os), &filter)` once.
- **P4.M3.T1.S1 (host-context send):** per debounced window change, optionally
  calls `send_command(ApplyHostContext { layer, callbacks, clear_board }, &filter)`
  and asserts the mock recorded "string (`notify`) before context
  (`send_command`)" ordering via `get_send_command_calls()`.

**Use Case**: the typed-command transport primitive — every typed wire operation
(QueryInfo, QueryCallback, SetOs, ApplyHostContext) goes through this one method,
parameterized by the `RunCommand` enum, so the trait stays a single seam the test
mock can intercept and the real impl can route to `qmk_notifier::run`.

**Pain Points Addressed**: today the `Notifier` trait only supports the legacy
`notify(String)` string path; there is no way to send a typed command (the
handshake, SET_OS, APPLY_HOST_CONTEXT) through the trait, so P4.M2/P4.M3 cannot
be implemented against the existing seam. This task adds that capability with the
same `Box<dyn Error + Send + Sync>` error idiom and a mock recorder that captures
call order for the stack-vs-replace send-logic tests.

## Why

- **PRD §5.1 / §8(4)** — the `Notifier` trait / `QmkNotifier` must "gain the
  capability so the test mock asserts ordering (string before context)." This task
  is that capability.
- **PRD §5.7** — "the host-context send happens within the same debounced 'send'
  step … Retry/cache for the typed command match the string path (§5.4)." The
  typed transport primitive this task adds is what those sends ride on (retry is
  the caller P4.M3's job — see D2).
- **Unblocks P4.M2 (handshake) and P4.M3 (host-context send).** Neither can be
  written without a `send_command` on the trait.
- **Closes the P4.M1.T1 vertical slice** (trait extension) as a standalone,
  unit-testable change with zero debounce-state coupling (the worker itself is
  untouched — P4.M1.T2.S2 extends `DebounceState`, P4.M3.T1.S1 wires the worker).

## What

Pure additions to `src/core/notifier.rs`: 1 trait method + 1 `QmkNotifier` impl
method + 1 `MockNotifier` impl method + 1 static + 1 accessor + 1 line in
`reset_global_mock` + Mode-A rustdoc + 5 tests. No struct changes, no new deps
(modulo the temporary G1 pin override), no CLI/tray/worker wiring.

### Success Criteria
- [ ] **`Notifier` trait** has exactly two methods: `notify` (unchanged) + a
      REQUIRED `send_command` with EXACTLY this signature:
      `fn send_command(&self, command: qmk_notifier::RunCommand, filter: &DeviceFilter) -> Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>>`.
- [ ] **`QmkNotifier::send_command`** builds `RunParameters::new(command,
      filter.vendor_id, filter.product_id, filter.usage_page, filter.usage, false)`,
      calls `qmk_notifier::run(params)`, and returns `Ok(resp)` / `Err(Box::new(e))`.
      NO retry loop. NO `verbose=true`.
- [ ] **`notify()` is byte-for-byte unchanged** (legacy string path with its own
      retry; the contract says "Keep notify() as-is").
- [ ] **`MockNotifier::send_command`** does `MOCK_SEND_COMMAND_CALLS.lock().unwrap().push(command.clone())`
      then returns `Ok(qmk_notifier::CommandResponse::Ack { ok: true })`.
- [ ] **`MOCK_SEND_COMMAND_CALLS`** is `Lazy<StdMutex<Vec<qmk_notifier::RunCommand>>>`
      (G5: `StdMutex`, not `Mutex` — name shadowed at module top).
- [ ] **`MockNotifier::get_send_command_calls()`** returns
      `MOCK_SEND_COMMAND_CALLS.lock().unwrap().clone()` (a `Vec<RunCommand>`).
- [ ] **`reset_global_mock()`** clears all THREE recorders: `MOCK_CALL_COUNT`,
      `MOCK_LAST_MESSAGE`, AND `MOCK_SEND_COMMAND_CALLS` (G7 — single source of
      truth, so `reset_test_state()` clears the log transitively).
- [ ] **Error mapping is `Err(Box::new(e))`** where `e: qmk_notifier::QmkError`
      (G3 — `QmkError: Error + Send + Sync`, so this coerces directly; NO custom
      wrapper, NO `.to_string()`).
- [ ] **Mode-A rustdoc** on `send_command` explains typed-command support + that
      the caller owns retry/cache (cite PRD §5.7/§8(4)); ` ```rust,ignore ` examples
      (G9 — binary-only crate).
- [ ] **5 new tests** appended to `#[cfg(test)] mod tests`, prefixed
      `test_send_command_*` (disjoint from the 6 existing `test_*` tests).
- [ ] `cargo build --bin qmkonnect` clean (G1 override in place); `cargo test
      --bin qmkonnect -- --test-threads=1` green; `git diff --stat` = notifier.rs only.

## All Needed Context

### Context Completeness Check

_Pass_: an agent with no prior knowledge of this codebase can implement this
using only this PRP, because: (a) the EXACT current `Notifier` trait (1 method,
`Send+Sync`), `DeviceFilter` (4 `pub` fields), `QmkNotifier::notify` (the
`RunParameters::new` + `qmk_notifier::run` + `Box::new(e)` idiom to mirror), the
`MockNotifier` (the `MOCK_CALL_COUNT`/`MOCK_LAST_MESSAGE`/`StdMutex`/`reset_global_mock`
pattern to extend), and `reset_test_state()` (calls `reset_global_mock`) are ALL
reproduced verbatim with line numbers in `research/notes.md` §0; (b) the EXACT
v0.3.0 crate API (`RunCommand`'s 6 variants, `CommandResponse`'s 5 variants,
`HostOs`, `RunParameters::new`'s 6-arg shape, `run`'s dispatch, `QmkError`'s
`Error+Send+Sync` bound) is reproduced verbatim in §1; (c) the ONE genuine risk —
the v0.2.1-vs-v0.3.0 compile gate — is resolved (G1: temporary path override, with
the exact Cargo.toml line to change and why) and called out as validation Step 0;
(d) the five design decisions (required-not-default method, retry-free transport,
verbose=false, `Vec<RunCommand>` recorder + optimistic `Ack` default, Mode-A
rustdoc) are each resolved with rationale (D1–D5) so the implementer doesn't
guess; (e) the error-mapping idiom (`Box::new(e)`, NOT a wrapper) is pinned (G3)
with the proof that `QmkError` satisfies the trait-object bounds; (f) the 5-test
plan with verbatim assertions is in §5/Task 6; (g) 10 gotchas are pinned (G1–G10);
(h) the downstream consumer contract (P4.M2/P4.M3) and the scope wall
(notify/worker/CLI untouched) are in the Integration Points.

### Documentation & References

```yaml
# MUST READ — the spec sources of truth
- file: PRD.md   # (or the merged prd_snapshot — the selected sections are in this PRP's header)
  why: "§5.1 (Notifier trait), §5.7 (host-side-rules extension: 'the host-context
        send happens within the same debounced send step … Retry/cache for the
        typed command match the string path §5.4'), §8(4) (the Notifier trait /
        QmkNotifier 'gain the capability so the test mock asserts ordering
        (string before context)'). §8(5) is the handshake (P4.M2 consumer)."
  section: "## 5. The Notification Pipeline & Debouncer  AND  ## 8. QMKonnect Spec (4)/(5)"
  gotcha: "§5.7's 'Retry/cache parity with SendMessage' is the CALLER's job
           (P4.M3.T1.S1), NOT send_command's. send_command is retry-free (D2)."

# MUST READ — the verbatim research (THIS task's full contract + design decisions)
- file: plan/002_637d65b6e9b8/P4M1T1S1/research/notes.md
  why: "§0 reproduces the exact current notifier.rs state (trait @12-14, DeviceFilter
        @23-28, QmkNotifier::notify @142-186 with the RunParameters+run+Box::new(e)
        idiom, NOTIFIER static @188, MockNotifier @399-415, reset_test_state @419-443).
        §1 is the verbatim v0.3.0 crate API (RunCommand/CommandResponse/HostOs/
        RunParameters/run/QmkError — all pub, exact derives + signatures). §2 is the
        compile gate (v0.2.1 pin vs v0.3.0 need) + the path-override workaround.
        §3 the 5 design decisions (D1-D5). §4 the 10 gotchas (G1-G10). §5 the 5-test
        plan."

# MUST READ — the file THIS task edits (read it before editing; it is the source of truth)
- file: src/core/notifier.rs
  why: "contains the Notifier trait (12-14), DeviceFilter (23-28), QmkNotifier::notify
        (142-186 — the RunParameters::new + qmk_notifier::run + Box::new(e) idiom to
        MIRROR in send_command), the NOTIFIER static (188-189), get_notifier (214),
        set_notifier (295), and the #[cfg(test)] mod tests block (373-588) with
        MockNotifier (399-415), reset_global_mock (385-389), reset_test_state
        (419-443), and the 6 existing tests (unchanged)."
  pattern: "add send_command as a second method on the trait (mirror notify's
            Result<_, Box<dyn Error + Send + Sync>> return); add the QmkNotifier impl
            mirroring notify's RunParameters::new(...) + qmk_notifier::run(params)
            + Err(Box::new(e)) (but WITHOUT the retry loop); add the MockNotifier
            impl mirroring MOCK_LAST_MESSAGE's StdMutex<Option<T>> push pattern but
            as StdMutex<Vec<RunCommand>>."
  gotcha: "the test module aliases `use std::sync::Mutex as StdMutex;` (line 379)
           because `Mutex` is shadowed by the module-top import. Reuse `StdMutex`
           for the new Vec<RunCommand> static (G5). `reset_global_mock()` is the
           single place to clear the new log (G7)."

# MUST READ — the qmk_notifier v0.3.0 crate API (the types send_command codes to)
- file: ../qmk_notifier/src/lib.rs   # local working tree = v0.3.0 source
  why: "RunCommand enum @19 (SendMessage/ListDevices/QueryInfo/QueryCallback(u8)/
        SetOs(HostOs)/ApplyHostContext{layer,callbacks,clear_board}; derives
        Debug,Clone,PartialEq,Eq — Clone matters for the mock recorder). HostOs @65
        (Unsure/Linux/Windows/Macos/Ios). CommandResponse @86 (Legacy{matched}/
        Info{proto_ver,feature_flags,callback_count,board_rules_present}/
        CallbackName{index,name}/Ack{ok}/Timeout). RunParameters @120 +
        RunParameters::new(command,vid,pid,usage_page,usage,verbose) @140.
        run(params)->Result<CommandResponse,QmkError> @418 (dispatches ALL variants
        through one send path — send_command needs NO per-variant special-casing)."
  pattern: "build RunParameters::new(command, filter.vendor_id, filter.product_id,
            filter.usage_page, filter.usage, false) exactly as notify() does at
            line 148, then qmk_notifier::run(params)."
  gotcha: "the local ../qmk_notifier working tree is v0.3.0 BUT qmkonnect's
           Cargo.toml pins the v0.2.1 git TAG — which lacks these types. Apply
           the G1 path override before compiling."
- file: ../qmk_notifier/src/error.rs
  why: "QmkError enum + `impl std::error::Error for QmkError {}` (the line that makes
        Box::new(e) coerce to Box<dyn Error + Send + Sync>). Variants include
        DeviceNotFound{vendor_id,product_id,usage_page,usage} (raised at enumeration,
        before any open/write — safe to provoke in test 4 with a bogus filter)."
  critical: "QmkError is Debug + Display + Error + Send + Sync (HidError is Send+Sync;
             enums of Send+Sync fields are Send+Sync). So Err(Box::new(e)) works with
             NO adapter — do NOT wrap it (G3)."

# MUST READ — the predecessor PRPs (the crate contracts this task depends on)
- file: plan/002_637d65b6e9b8/P1M1T1S1/PRP.md   # HostOs + RunCommand variants
  why: "defines RunCommand's typed variants + HostOs. Confirms the derives
        (Clone matters — the mock clones RunCommand into the call log)."
- file: plan/002_637d65b6e9b8/P1M1T1S2/PRP.md   # CommandResponse enum
  why: "defines CommandResponse's 5 variants (Legacy/Info/CallbackName/Ack/Timeout).
        The mock's default return is Ack{ok:true} (D4)."
- file: plan/002_637d65b6e9b8/P1M1T3S2/PRP.md   # run() returns CommandResponse
  why: "confirms run()'s final v0.3.0 signature: run(RunParameters) ->
        Result<CommandResponse, QmkError>. This task codes to that signature."

# Reference — the crate-release + Cargo-pin siblings (the G1 resolution path)
- file: plan/002_637d65b6e9b8/P1M1T4S1/research/notes.md
  why: "§6/§9 explain WHY the v0.3.0 tag isn't available yet (P1.M1.T4.S1 = Ready)
        and that the downstream pin (P4.M1.T2.S1) resolves over HTTPS to origin
        (needs the tag pushed). Justifies the G1 temporary path-override as the
        dev workaround until both siblings land."

# Reference — Rust trait-object error idiom (Box<dyn Error + Send + Sync>)
- url: https://doc.rust-lang.org/std/error/trait.Error.html
  why: "confirms `impl std::error::Error for QmkError {}` makes QmkError a valid
        `Box<dyn Error>` source. Combined with `Send + Sync` (auto-derived for
        QmkError's all-Send+Sync fields), `Box::new(e)` coerces to the exact
        `Box<dyn Error + Send + Sync>` return type notify() already uses."
  critical: "do NOT introduce a custom error wrapper or .to_string() the QmkError —
             that loses the concrete type. Reuse notify()'s `Err(Box::new(e))` idiom."
```

### Current Codebase tree (relevant subset)

```bash
src/
  main.rs              # `mod core;` (binary-only crate — NO lib.rs; G9)
  core/
    mod.rs             # `pub mod notifier;` (registered long ago). UNCHANGED.
    notifier.rs        # ← THIS TASK EDITS THIS FILE ONLY.
                         #     trait Notifier (12-14) + DeviceFilter (23-28)
                         #     + QmkNotifier::notify (142-186) + NOTIFIER static (188)
                         #     + get_notifier (214) + set_notifier (295)
                         #     + notify_qmk (309) + debounce_worker (238)
                         #     + #[cfg(test)] mod tests (373-588: MockNotifier,
                         #       reset_global_mock, reset_test_state, 6 tests).
                         #   THIS TASK ADDS:
                         #     + send_command to trait Notifier (+rustdoc)
                         #     + send_command to impl Notifier for QmkNotifier
                         #     + send_command to impl Notifier for MockNotifier
                         #     + MOCK_SEND_COMMAND_CALLS static + get_send_command_calls()
                         #     + 1 line in reset_global_mock() (clear the log)
                         #     + 5 test_send_command_* tests
    rules.rs           # P3.M1 (evaluate/HostContext). UNCHANGED (P3.M1.T2.S1 edits it).
    pattern.rs         # P2.M1 matcher. UNCHANGED.
    types.rs           # WindowInfo. UNCHANGED.
Cargo.toml             # line 16 pins qmk_notifier v0.2.1. TEMPORARY path-override
                         # for validation (G1); revert before commit (or let
                         # P4.M1.T2.S1 flip to the v0.3.0 tag once pushed).
../qmk_notifier/       # the v0.3.0 crate source (local working tree). READ-ONLY here.
```

### Desired Codebase tree with files to be added/changed

```bash
src/
  core/
    notifier.rs        # MODIFIED (additive) — + `fn send_command` on trait Notifier
                         #     (+ Mode-A rustdoc) + same method on both impls
                         #     (QmkNotifier real transport; MockNotifier recorder)
                         #     + MOCK_SEND_COMMAND_CALLS static + get_send_command_calls()
                         #     + 1 clear-line in reset_global_mock + 5 new tests.
    # mod.rs, rules.rs, pattern.rs, types.rs, platforms/*, tray*, main.rs: ALL UNCHANGED
Cargo.toml             # TEMPORARY dev override ONLY (G1): flip line 16 to
                         #   qmk_notifier = { path = "../qmk_notifier" } while
                         #   validating; REVERT to the git-tag pin before commit.
                         #   (The permanent v0.3.0 tag pin is P4.M1.T2.S1's job.)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (G1 — THE compile gate): Cargo.toml:16 pins qmk_notifier v0.2.1 (git tag),
//   which has ONLY RunCommand::{SendMessage, ListDevices} and NO CommandResponse /
//   HostOs / typed variants. This task's code references the v0.3.0 types. BEFORE
//   `cargo build`/`cargo test`, temporarily set:
//     qmk_notifier = { path = "../qmk_notifier" }
//   (the local working tree IS v0.3.0). v0.3.0 is not yet tagged/pushed
//   (P1.M1.T4.S1=Ready, P4.M1.T2.S1=Planned). Revert before commit. See notes §2.
//
// CRITICAL (G2 — required method is safe): there are exactly TWO `impl Notifier for`
//   sites, BOTH in notifier.rs (QmkNotifier @142, MockNotifier @407). NO external
//   implementors (rg-confirmed). Adding a REQUIRED trait method compiles cleanly
//   once both in-file impls gain it. Do NOT add a default body (D1).
//
// CRITICAL (G3 — error mapping is Box::new(e), NOT a wrapper): qmk_notifier::QmkError
//   implements std::error::Error AND is Send+Sync (its variants hold only String /
//   HidError / primitives / structs of those — all Send+Sync). So:
//       Err(Box::new(e))   // e: qmk_notifier::QmkError
//   coerces directly to `Box<dyn Error + Send + Sync>`. This is the EXACT idiom
//   notify() uses at line 178. Do NOT .to_string() it (loses the type), do NOT
//   build a custom AnyError wrapper.
//
// CRITICAL (G4 — keep notify() byte-for-byte unchanged): the contract says "Keep
//   notify() as-is." Do NOT refactor its 3-attempt retry, do NOT reroute it through
//   send_command(SendMessage(..)) (that would change discard-the-result semantics
//   + retry ownership). notify stays the legacy string path; send_command is the
//   NEW typed path.
//
// GOTCHA (G5 — use StdMutex in the test module, NOT Mutex): the test module declares
//   `use std::sync::Mutex as StdMutex;` (line 379) because `Mutex` is shadowed by
//   the module-top `use std::sync::{Arc, Condvar, Mutex};`. The new call-log static
//   must be `Lazy<StdMutex<Vec<qmk_notifier::RunCommand>>>` (mirror the existing
//   `MOCK_LAST_MESSAGE: Lazy<StdMutex<Option<String>>>`).
//
// GOTCHA (G6 — clone RunCommand into the log): RunCommand arrives by value (moved).
//   The mock records via `MOCK_SEND_COMMAND_CALLS.lock().unwrap().push(command.clone())`.
//   RunCommand derives Clone (verified), so this is cheap and correct. (Moving would
//   also work since the mock doesn't reuse `command`, but `.clone()` is the defensive
//   choice matching MOCK_LAST_MESSAGE's `= Some(message)` move-vs-clone consistency.)
//
// GOTCHA (G7 — clear the log in reset_global_mock, the single source of truth):
//   reset_test_state() (line 419) calls reset_global_mock() (line 385). Add the new
//   clear to reset_global_mock() so BOTH callers reset it:
//       MOCK_SEND_COMMAND_CALLS.lock().unwrap().clear();
//   Do NOT also clear it inside reset_test_state() directly (avoid two sites).
//
// GOTCHA (G8 — single-threaded tests): `cargo test --bin qmkonnect --
//   --test-threads=1` (shared STATE/COND/WORKER/NOTIFIER globals; AGENTS.md). NEVER
//   multi-threaded — the existing tests already require this.
//
// GOTCHA (G9 — binary-only crate; doctests): there is NO lib.rs (src/main.rs
//   declares `mod core;`). Mode-A rustdoc on send_command: use ` ```rust,ignore `
//   fenced examples (do NOT add a bare ` ``` ` runnable doctest doing
//   `use qmkonnect::...` — it won't compile under `cargo test --doc`, and `--bin`
//   doesn't run doctests). Match the rules.rs/pattern.rs convention.
//
// GOTCHA (G10 — no new Cargo deps): qmk_notifier, once_cell::sync::Lazy,
//   std::sync::{Mutex as StdMutex, atomic::AtomicUsize}, std::error::Error are ALL
//   already imported/available in notifier.rs. NO new dependencies. The ONLY
//   Cargo.toml touch is the temporary G1 pin override (not a new crate).
```

## Implementation Blueprint

### Data models and structure

```rust
// ── (1) The Notifier trait gains a second, REQUIRED method ──
//   Place it AFTER `fn notify(...)` (line 13), still inside `pub trait Notifier`.
//   No default body (D1). Mode-A rustdoc (G9: ```rust,ignore).

/// Send a **typed** command to the QMK device and return its parsed reply.
///
/// This is the typed-command transport primitive backing the host-side-rules
/// pipeline (PRD §5.7, §8(4)/(5)): the capability handshake (`QueryInfo` /
/// `QueryCallback` / `SetOs`, P4.M2) and the per-window host-context send
/// (`ApplyHostContext`, P4.M3) both ride through this single method.
///
/// `notify()` remains the legacy string path (`SendMessage`); `send_command` is
/// the typed path — parameterized by [`qmk_notifier::RunCommand`] so the trait
/// stays one seam the test mock can intercept and the real impl can route to
/// [`qmk_notifier::run`].
///
/// **Retry / cache parity** (PRD §5.7: "Retry/cache for the typed command match
/// the string path §5.4") is the **caller's** responsibility (P4.M3.T1.S1), not
/// this method's — `send_command` is a thin transport wrapper: build
/// [`qmk_notifier::RunParameters`] from `command` + `filter`, call
/// [`qmk_notifier::run`], map [`qmk_notifier::QmkError`] to a boxed error, and
/// return the [`qmk_notifier::CommandResponse`] unchanged.
///
/// # Example
///
/// ```rust,ignore
/// use qmkonnect::core::notifier::{Notifier, QmkNotifier, DeviceFilter};
/// use qmk_notifier::{RunCommand, CommandResponse};
///
/// let notifier = QmkNotifier;
/// let filter = DeviceFilter {
///     vendor_id: None, product_id: None,
///     usage_page: 0xFF60, usage: 0x61,
/// };
/// match notifier.send_command(RunCommand::QueryInfo, &filter) {
///     Ok(CommandResponse::Info { proto_ver: 2, feature_flags, callback_count, .. }) => { /* capable */ }
///     Ok(_) => { /* legacy / timeout -> string-only fallback */ }
///     Err(e) => { /* device error — caller decides retry/cache (P4.M3) */ }
/// }
/// ```
fn send_command(
    &self,
    command: qmk_notifier::RunCommand,
    filter: &DeviceFilter,
) -> Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>>;


// ── (2) The QmkNotifier impl — thin transport wrapper, NO retry (D2) ──
//   Place AFTER `fn notify(...)` inside `impl Notifier for QmkNotifier` (after
//   line 186). Mirrors notify()'s RunParameters::new + qmk_notifier::run + Box::new(e),
//   but WITHOUT the retry loop.

impl Notifier for QmkNotifier {
    fn notify(&self, message: String) -> Result<(), Box<dyn Error + Send + Sync>> {
        // ... UNCHANGED (lines 143-185) ...
    }

    fn send_command(
        &self,
        command: qmk_notifier::RunCommand,
        filter: &DeviceFilter,
    ) -> Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>> {
        let params = qmk_notifier::RunParameters::new(
            command,
            filter.vendor_id,
            filter.product_id,
            filter.usage_page,
            filter.usage,
            false, // verbose — transport stays quiet; orchestration logs (D3)
        );
        match qmk_notifier::run(params) {
            Ok(resp) => Ok(resp),
            Err(e) => Err(Box::new(e)), // G3: QmkError: Error+Send+Sync, coerces directly
        }
    }
}


// ── (3) The MockNotifier recorder (inside #[cfg(test)] mod tests) ──
//   New static + accessor + impl method. Mirrors MOCK_CALL_COUNT/MOCK_LAST_MESSAGE.

// (near the existing MOCK_CALL_COUNT / MOCK_LAST_MESSAGE statics, ~line 382)
static MOCK_SEND_COMMAND_CALLS: Lazy<StdMutex<Vec<qmk_notifier::RunCommand>>> =
    Lazy::new(|| StdMutex::new(Vec::new()));

// (inside `impl MockNotifier`, near get_call_count / get_last_message)
fn get_send_command_calls() -> Vec<qmk_notifier::RunCommand> {
    MOCK_SEND_COMMAND_CALLS.lock().unwrap().clone()
}

// (inside `impl Notifier for MockNotifier`, AFTER the existing `fn notify`)
fn send_command(
    &self,
    command: qmk_notifier::RunCommand,
    _filter: &DeviceFilter,
) -> Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>> {
    MOCK_SEND_COMMAND_CALLS
        .lock()
        .unwrap()
        .push(command.clone()); // G6: RunCommand: Clone
    Ok(qmk_notifier::CommandResponse::Ack { ok: true }) // D4: optimistic default
}

// ── (4) reset_global_mock clears the new log too (G7) ──
fn reset_global_mock() {
    MOCK_CALL_COUNT.store(0, Ordering::SeqCst);
    *MOCK_LAST_MESSAGE.lock().unwrap() = None;
    MOCK_SEND_COMMAND_CALLS.lock().unwrap().clear(); // ← NEW LINE
}
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 0 (PREREQUISITE — the G1 compile gate): TEMPORARILY flip Cargo.toml:16
  - DO: change
      qmk_notifier = { package = "qmk_notifier", git = "...", tag = "v0.2.1" }
    to
      qmk_notifier = { path = "../qmk_notifier" }
  - WHY: the local ../qmk_notifier tree is v0.3.0 (has RunCommand typed variants +
    CommandResponse + HostOs); the pinned v0.2.1 git tag does NOT. Without this,
    Tasks 1-6 will not compile ("no variant `QueryInfo`", "cannot find type
    `CommandResponse`").
  - GOTCHA G1: this is a DEV-ONLY override (machine-local path). Revert before
    commit (Task 7) OR — if P1.M1.T4.S1 has tagged+pushed v0.3.0 and P4.M1.T2.S1
    has flipped the pin to tag="v0.3.0" — leave the git-tag pin and skip this
    override. Run `git ls-remote --tags origin v0.3.0` to check.

Task 1: ADD `fn send_command` to the `Notifier` trait (src/core/notifier.rs:12-14)
  - DO: insert the method (signature EXACTLY as in the Data-models block) AFTER
    `fn notify(...)` (line 13), still inside `pub trait Notifier: Send + Sync { ... }`.
    Include the Mode-A `///` rustdoc block (use ` ```rust,ignore ` per G9).
  - NAMING: send_command (exact, per contract).
  - VISIBILITY: trait method (no `pub` keyword needed inside a trait).
  - SIGNATURE: `fn send_command(&self, command: qmk_notifier::RunCommand, filter:
    &DeviceFilter) -> Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>>;`
  - NO default body (D1 — required method).
  - GOTCHA G2: both impls (QmkNotifier @142, MockNotifier @407) MUST gain it or
    the build breaks — Tasks 2 & 3 add them.

Task 2: ADD `fn send_command` to `impl Notifier for QmkNotifier` (after line 186)
  - DO: insert the method EXACTLY as in the Data-models block. Build RunParameters
    via `qmk_notifier::RunParameters::new(command, filter.vendor_id,
    filter.product_id, filter.usage_page, filter.usage, false)`, call
    `qmk_notifier::run(params)`, return `Ok(resp)` / `Err(Box::new(e))`.
  - NO retry loop (D2). verbose=false (D3).
  - MIRROR: notify()'s RunParameters::new(...) at line 148 (same 6-arg shape) and
    notify()'s `Err(Box::new(e))` at line 178 (G3 error-mapping idiom).
  - GOTCHA G3: `Err(Box::new(e))` — QmkError already satisfies Error+Send+Sync.
  - GOTCHA G4: do NOT touch notify()'s body.

Task 3: ADD the MockNotifier recorder (inside #[cfg(test)] mod tests)
  - DO (3a): add the static near MOCK_CALL_COUNT/MOCK_LAST_MESSAGE (~line 382):
      static MOCK_SEND_COMMAND_CALLS: Lazy<StdMutex<Vec<qmk_notifier::RunCommand>>> =
          Lazy::new(|| StdMutex::new(Vec::new()));
  - DO (3b): add the accessor inside `impl MockNotifier` (near get_call_count):
      fn get_send_command_calls() -> Vec<qmk_notifier::RunCommand> {
          MOCK_SEND_COMMAND_CALLS.lock().unwrap().clone()
      }
  - DO (3c): add the method inside `impl Notifier for MockNotifier` (after fn notify):
      fn send_command(&self, command: qmk_notifier::RunCommand, _filter: &DeviceFilter)
          -> Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>> {
          MOCK_SEND_COMMAND_CALLS.lock().unwrap().push(command.clone());
          Ok(qmk_notifier::CommandResponse::Ack { ok: true })
      }
  - GOTCHA G5: StdMutex (not Mutex — shadowed at module top).
  - GOTCHA G6: command.clone() (RunCommand: Clone).
  - NOTE: `_filter` is unused in the mock (prefix with `_` to silence the warning).
    The real impl (Task 2) uses filter; the mock only records the command.

Task 4: EXTEND reset_global_mock() to clear the new log (src/core/notifier.rs:385-389)
  - DO: add ONE line after the existing `*MOCK_LAST_MESSAGE.lock().unwrap() = None;`:
      MOCK_SEND_COMMAND_CALLS.lock().unwrap().clear();
  - WHY: reset_test_state() (line 419) calls reset_global_mock(), so the log is
    cleared transitively at the single source of truth (G7).
  - DO NOT also clear it inside reset_test_state() directly.

Task 5: VERIFY build + the existing tests still pass (mid-point gate)
  - RUN: cargo build --bin qmkonnect          (G1 override in place; expect clean)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1
         (expect: the 6 EXISTING tests still pass — they exercise notify() only,
          unchanged. send_command has no test callers yet but the mock compiles.)
  - IF a compile error names a typed variant/CommandResponse: G1 override is
    missing or stale — re-apply Task 0, then `cargo update -p qmk_notifier`.

Task 6: ADD 5 tests to #[cfg(test)] mod tests (append after test_threads_dont_interfere)
  - DO: append the following (all use `use super::*;` already at the module top;
    construct DeviceFilter via struct literal; construct RunCommand via
    `qmk_notifier::RunCommand::Variant`):

    1. test_send_command_records_call_sequence:
         reset_test_state(); set_notifier(Box::new(MockNotifier::new()));
         let f = DeviceFilter { vendor_id: Some(0x1234), product_id: Some(0x5678),
                                usage_page: 0xFF60, usage: 0x61 };
         let notifier = get_notifier(); let n = notifier.lock().unwrap();
         let _ = n.send_command(qmk_notifier::RunCommand::QueryInfo, &f);
         let _ = n.send_command(qmk_notifier::RunCommand::ApplyHostContext {
             layer: Some(224), callbacks: vec![0,1], clear_board: false }, &f);
         drop(n);
         let calls = MockNotifier::get_send_command_calls();
         assert_eq!(calls.len(), 2);
         assert_eq!(calls[0], qmk_notifier::RunCommand::QueryInfo);
         assert!(matches!(calls[1], qmk_notifier::RunCommand::ApplyHostContext{ layer: Some(224), .. }));
         // THE contract: "store sequence of calls for ordering assertions."
       (NOTE: set_notifier swaps the global; get_notifier() retrieves the Arc.
        Lock the Mutex, call send_command on the boxed trait object. The trait
        method is dynamically dispatched — exactly what P4.M2/P4.M3 will do.)

    2. test_send_command_reset_clears_log:
         reset_test_state(); set_notifier(Box::new(MockNotifier::new()));
         let f = DeviceFilter { vendor_id: None, product_id: None, usage_page: 0xFF60, usage: 0x61 };
         { let n = get_notifier().lock().unwrap();
           let _ = n.send_command(qmk_notifier::RunCommand::QueryInfo, &f); }
         assert!(!MockNotifier::get_send_command_calls().is_empty());
         reset_test_state();   // G7: must clear the log
         assert!(MockNotifier::get_send_command_calls().is_empty());

    3. test_send_command_returns_ok_ack_default:
         reset_test_state(); set_notifier(Box::new(MockNotifier::new()));
         let f = DeviceFilter { vendor_id: None, product_id: None, usage_page: 0xFF60, usage: 0x61 };
         let n = get_notifier().lock().unwrap();
         let resp = n.send_command(qmk_notifier::RunCommand::SetOs(qmk_notifier::HostOs::Linux), &f);
         assert!(matches!(resp, Ok(qmk_notifier::CommandResponse::Ack { ok: true })));
       (D4: optimistic default return. P4.M2/P4.M3 will extend the mock with
        configurable responses later — out of scope here.)

    4. test_qmk_notifier_send_command_maps_device_not_found:   # REAL impl, no hardware
         // The REAL QmkNotifier against a bogus filter. qmk_notifier::run enumerates
         // HID devices; a nonexistent VID/PID yields QmkError::DeviceNotFound at
         // enumeration (NO device is opened, NO report sent — safe on any machine).
         reset_test_state();   // (stabilizes NOTIFIER-free state; we use QmkNotifier directly)
         let qmk = QmkNotifier;
         let f = DeviceFilter { vendor_id: Some(0xFFFF), product_id: Some(0xFFFF),
                                usage_page: 0xFF60, usage: 0x61 };
         let res = qmk.send_command(qmk_notifier::RunCommand::QueryInfo, &f);
         assert!(res.is_err());
         let msg = res.unwrap_err().to_string().to_lowercase();
         assert!(msg.contains("no device found"),
             "expected DeviceNotFound, got: {msg}");
       (Validates: RunParameters built, run() called, QmkError mapped via Box::new(e)
        (G3). Does NOT swap the global NOTIFIER — QmkNotifier is a unit struct used
        directly. If a device with VID 0xFFFF:PID 0xFFFF raw-HID 0xFF60:0x61
        genuinely exists on the test machine, pick rarer values; DeviceNotFound is
        the overwhelmingly likely result.)

    5. test_send_command_notify_recorders_independent:   # G4 guard
         reset_test_state(); set_notifier(Box::new(MockNotifier::new()));
         let f = DeviceFilter { vendor_id: None, product_id: None, usage_page: 0xFF60, usage: 0x61 };
         { let n = get_notifier().lock().unwrap();
           let _ = n.notify("App\x1DTitle".to_string());            // notify recorder
           let _ = n.send_command(qmk_notifier::RunCommand::QueryInfo, &f); } // send_command recorder
         assert_eq!(MockNotifier::get_call_count(), 1);
         assert_eq!(MockNotifier::get_last_message(), Some("App\x1DTitle".to_string()));
         assert_eq!(MockNotifier::get_send_command_calls().len(), 1); // independent
         // notify's recorder and send_command's recorder don't cross-contaminate.

  - NAMING: test_send_command_* (disjoint from the existing test_* names).
  - GOTCHA G8: single-threaded (cargo test --bin qmkonnect -- --test-threads=1).
  - GOTCHA: do NOT modify the 6 existing tests — append only.

Task 7: REVERT the Cargo.toml G1 override (unless P4.M1.T2.S1 is landing in-session)
  - DO: restore Cargo.toml:16 to the git-tag pin:
      qmk_notifier = { package = "qmk_notifier", git = "https://github.com/dabstractor/qmk_notifier", tag = "v0.2.1" }
    (OR, if v0.3.0 is tagged+pushed and P4.M1.T2.S1 has flipped it, leave tag="v0.3.0".)
  - WHY: the path override is machine-local; committing it breaks other machines.
  - CONFIRM: `git diff --stat` shows ONLY src/core/notifier.rs (Cargo.toml reverted).
  - NOTE: after reverting, `cargo build` against v0.2.1 will FAIL on the typed
    variants — that is EXPECTED and correct: this task's code is v0.3.0-only by
    design, and the permanent resolution is P4.M1.T2.S1 (the v0.3.0 tag pin). The
    PRP's validation gates (below) are run BEFORE the revert, with the override
    in place. Document this in the commit message / handoff.
```

### Implementation Patterns & Key Details

```rust
// The canonical send_command for QmkNotifier (THIS IS THE CONTRACT — match it):
//
// fn send_command(&self, command: qmk_notifier::RunCommand, filter: &DeviceFilter)
//     -> Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>> {
//     let params = qmk_notifier::RunParameters::new(
//         command,                                  // by value (moved in)
//         filter.vendor_id, filter.product_id,      // Option<u16>
//         filter.usage_page, filter.usage,           // u16
//         false,                                    // verbose (D3)
//     );
//     match qmk_notifier::run(params) {
//         Ok(resp) => Ok(resp),
//         Err(e) => Err(Box::new(e)),               // G3: QmkError -> boxed, no wrapper
//     }
// }
//
// The canonical MockNotifier send_command (the recorder):
//
// fn send_command(&self, command: qmk_notifier::RunCommand, _filter: &DeviceFilter)
//     -> Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>> {
//     MOCK_SEND_COMMAND_CALLS.lock().unwrap().push(command.clone());  // G6
//     Ok(qmk_notifier::CommandResponse::Ack { ok: true })             // D4
// }
//
// KEY INVARIANTS:
//  - send_command is a REQUIRED trait method (no default body). Both impls provide it.
//  - QmkNotifier's impl is retry-FREE (D2): one run() call, one result. The caller
//    (P4.M3) owns retry/cache parity with the string path.
//  - Error mapping reuses notify()'s exact `Err(Box::new(e))` idiom (G3) — QmkError
//    is Error+Send+Sync, so Box::new(e) coerces to Box<dyn Error + Send + Sync>.
//  - The mock records the call SEQUENCE (Vec<RunCommand>, push order = call order)
//    so P4.M3 can assert "string (notify) before context (send_command)" via index.
//  - RunParameters::new takes `command` BY VALUE (move) — send_command's `command`
//    param is also by value (move), passed straight through. The mock clones before
//    recording because RunCommand: Clone and the move would prevent reuse (defensive).
//
// TEST FIXTURE IDIOM (mirror the existing mock tests):
//   reset_test_state();
//   set_notifier(Box::new(MockNotifier::new()));
//   let f = DeviceFilter { vendor_id: None, product_id: None,
//                          usage_page: 0xFF60, usage: 0x61 };
//   { let n = get_notifier().lock().unwrap();   // Arc<Mutex<Box<dyn Notifier>>>
//     let _ = n.send_command(qmk_notifier::RunCommand::QueryInfo, &f); }
//   let calls = MockNotifier::get_send_command_calls();
//   assert_eq!(calls, vec![qmk_notifier::RunCommand::QueryInfo]);
```

### Integration Points

```yaml
MODULE REGISTRATION:
  - NONE this task. `pub mod notifier;` has been in src/core/mod.rs since P1.M2.T1.
    This task only edits the BODY of notifier.rs (adds a trait method + 2 impl
    methods + a test static + 1 reset line + 5 tests).

DEPENDENCIES (this task): qmk_notifier v0.3.0 (TEMPORARY path override — G1; the
  permanent v0.3.0 tag pin is P4.M1.T2.S1). once_cell::sync::Lazy + std::sync::Mutex
  (aliased StdMutex in the test module) + std::error::Error + std::sync::atomic —
  ALL already imported in notifier.rs. NO new Cargo entries (other than the G1 flip).

UPSTREAM (consumed unchanged — all verified present):
  - qmk_notifier::RunCommand (lib.rs:19) — 6 variants, derives Clone (mock clones
    into the log). The `command` param type.
  - qmk_notifier::CommandResponse (lib.rs:86) — the return type (Ok payload); mock's
    default is Ack{ok:true}.
  - qmk_notifier::HostOs (lib.rs:65) — used inside SetOs(HostOs); reachable as
    qmk_notifier::HostOs (no import needed — fully qualified).
  - qmk_notifier::RunParameters + RunParameters::new (lib.rs:120,140) — 6-arg ctor;
    QmkNotifier::send_command builds one exactly as notify() does (line 148).
  - qmk_notifier::run (lib.rs:418) -> Result<CommandResponse, QmkError>.
  - qmk_notifier::QmkError (error.rs) — Error+Send+Sync; Box::new(e) maps it (G3).
  - DeviceFilter (notifier.rs:23-28) — the `filter: &DeviceFilter` param type;
    already pub, 4 pub fields. UNCHANGED.
  - NOTIFIER / get_notifier / set_notifier (notifier.rs:188,214,295) — the global
    trait-object seam; tests swap MockNotifier in via set_notifier and call
    send_command through get_notifier().lock(). UNCHANGED.

DOWNSTREAM CONSUMERS (later subtasks — do NOT implement them here):
  - P4.M2.T1.S1 (handshake): calls send_command(QueryInfo, &filter), matches
    CommandResponse::Info{ proto_ver:2, feature_flags, callback_count,
    board_rules_present }; loops send_command(QueryCallback(i), &filter); calls
    send_command(SetOs(host_os), &filter). WILL extend the mock with a configurable
    response queue for its happy-path/sad-path tests (the optimistic Ack default
    added here is a starting point, not a constraint).
  - P4.M3.T1.S1 (host-context send): calls send_command(ApplyHostContext{ layer,
    callbacks, clear_board }, &filter) per debounced window change; asserts the
    mock recorded "string (notify) before context (send_command)" ordering via
    get_send_command_calls(). Owns the retry/cache parity (D2).
  - P4.M1.T2.S2 (extend DebounceState): unrelated to the trait; carries WindowInfo
    for host-context evaluation. This task does NOT touch DebounceState.

CONFIG: none (no new config knob).
ROUTES: none (no CLI surface this subtask — P5.M1 is the CLI).
DATABASE: none.
```

## Validation Loop

### Step 0 (PREREQUISITE — the G1 compile gate)

```bash
cd /home/dustin/projects/qmkonnect
# Confirm the v0.3.0 types are NOT resolvable from the current pin:
git ls-remote --tags https://github.com/dabstractor/qmk_notifier v0.3.0   # expect: empty (tag not pushed)
# Apply the temporary path override so cargo resolves the LOCAL v0.3.0 tree:
#   edit Cargo.toml line 16 ->  qmk_notifier = { path = "../qmk_notifier" }
# Confirm the local tree is v0.3.0:
grep '^version' ../qmk_notifier/Cargo.toml   # expect: version = "0.3.0"
grep -n 'pub enum RunCommand\|pub enum CommandResponse' ../qmk_notifier/src/lib.rs  # expect both present
cargo update -p qmk_notifier                 # refresh the lock to the path source
```

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles clean, no NEW warnings. (Both impls gain send_command, so the
# required trait method is satisfied.) If rustc errors name a typed variant or
# CommandResponse, the G1 override is missing/stale — redo Step 0.

# Confirm the deliverables are present and the scope is right:
grep -n 'fn send_command' src/core/notifier.rs                 # expect 3 (trait + QmkNotifier + MockNotifier)
grep -n 'MOCK_SEND_COMMAND_CALLS' src/core/notifier.rs         # expect 3 (static decl + push + clear + accessor = ≥4)
grep -n 'static MOCK_SEND_COMMAND_CALLS' src/core/notifier.rs  # expect 1
grep -n 'get_send_command_calls' src/core/notifier.rs          # expect 2 (def + ≥1 test call)
grep -n 'Box::new(e)' src/core/notifier.rs                     # expect 2 (notify's + send_command's)
grep -n 'MOCK_SEND_COMMAND_CALLS.lock().unwrap().clear()' src/core/notifier.rs  # expect 1 (in reset_global_mock, G7)
# Confirm notify() body is unchanged (G4) — its retry loop + Ok-after-3 must remain:
grep -n 'for attempt in 1..=3' src/core/notifier.rs            # expect 1 (still in notify)
# Confirm send_command has NO retry loop (D2):
! grep -n 'for attempt' src/core/notifier.rs | grep -i send_command  # expect: no match
# Confirm additive only:
git diff --stat   # expect: src/core/notifier.rs (Cargo.toml is reverted in Step 7 / Task 7)
```

### Level 2: Unit Tests (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# Single-threaded is MANDATORY crate-wide (shared debouncer state, AGENTS.md / G8).
cargo test --bin qmkonnect notifier::tests::test_send_command -- --test-threads=1
# Expected: all 5 new tests pass:
#   - test_send_command_records_call_sequence (ordering — the contract)
#   - test_send_command_reset_clears_log (G7)
#   - test_send_command_returns_ok_ack_default (D4)
#   - test_qmk_notifier_send_command_maps_device_not_found (G3 + real run() wiring)
#   - test_send_command_notify_recorders_independent (G4)

# Targeted spot-checks (the highest-risk invariants):
cargo test --bin qmkonnect notifier::tests::test_send_command_records_call_sequence -- --test-threads=1
# Expected: calls[0]==QueryInfo, calls[1]==ApplyHostContext{..} (order preserved).
cargo test --bin qmkonnect notifier::tests::test_qmk_notifier_send_command_maps_device_not_found -- --test-threads=1
# Expected: real QmkNotifier against bogus filter -> Err whose to_string() contains
# "no device found" (proves RunParameters built + run() called + QmkError mapped).
```

### Level 3: Full-crate regression (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL bin tests green — the 6 existing notifier tests (UNCHANGED, they
# exercise notify() only) + the 5 new test_send_command_* + rules (P3.M1) + pattern
# (P2.M1 parity corpus) + types + mod. Proves the additions compile in the full
# crate context and didn't break the shared debouncer state or the trait-object seam.

# Confirm the change surface (after reverting the G1 override per Task 7):
git status --short
# Expected: only src/core/notifier.rs modified (additive). Cargo.toml REVERTED to
# the git-tag pin (the path override is dev-only). NOTHING in mod.rs, rules.rs,
# pattern.rs, types.rs, platforms/*, tray*, main.rs.
git diff --stat
# Expected: 1 file: src/core/notifier.rs (Cargo.toml reverted).
```

### Level 4: Semantic spot-check (the contract's two hard requirements)

```bash
# Requirement 1 — "store sequence of calls for ordering assertions":
#   Covered FUNCTIONALLY by test_send_command_records_call_sequence (Level 2),
#   which asserts calls[0]==QueryInfo and calls[1]==ApplyHostContext{..} in push
#   order. No extra manual step.
#
# Requirement 2 — "Implement for QmkNotifier: build RunParameters from the
#   command + filter, call qmk_notifier::run(params), map QmkError to boxed error,
#   return the CommandResponse":
#   Covered FUNCTIONALLY by test_qmk_notifier_send_command_maps_device_not_found
#   (Level 2), which proves the real impl builds params, calls run(), and maps
#   QmkError::DeviceNotFound -> a boxed error whose Display contains "no device
#   found". The happy-path Ok(resp) branch is type-checked by the compiler (the
#   return type pins CommandResponse). No extra manual step.
#
# Both requirements are asserted in code; the Level-2 run is the gate.
```

## Final Validation Checklist

### Technical Validation
- [ ] Step 0 (G1 override) applied; `cargo build --bin qmkonnect` clean (no NEW warnings).
- [ ] `cargo test --bin qmkonnect notifier::tests::test_send_command -- --test-threads=1` — all 5 pass.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` — full crate green (no regression; 6 existing tests unchanged).
- [ ] `git diff --stat` shows `src/core/notifier.rs` only (Cargo.toml reverted per Task 7).

### Feature Validation (contract fidelity)
- [ ] **`Notifier` trait** has exactly two methods: `notify` (unchanged) + a REQUIRED `send_command` with EXACTLY the contract signature: `fn send_command(&self, command: qmk_notifier::RunCommand, filter: &DeviceFilter) -> Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>>`.
- [ ] **`QmkNotifier::send_command`** builds `RunParameters::new(command, filter.vendor_id, filter.product_id, filter.usage_page, filter.usage, false)`, calls `qmk_notifier::run`, returns `Ok(resp)` / `Err(Box::new(e))`. NO retry loop (D2).
- [ ] **`notify()` byte-for-byte unchanged** (G4 — its retry loop + Ok-after-3-attempts intact).
- [ ] **`MockNotifier::send_command`** pushes `command.clone()` into `MOCK_SEND_COMMAND_CALLS` and returns `Ok(CommandResponse::Ack { ok: true })` (D4).
- [ ] **`MOCK_SEND_COMMAND_CALLS`** is `Lazy<StdMutex<Vec<qmk_notifier::RunCommand>>>` (G5).
- [ ] **`MockNotifier::get_send_command_calls()`** returns a clone of the log (Vec<RunCommand>).
- [ ] **`reset_global_mock()`** clears all three recorders including `MOCK_SEND_COMMAND_CALLS` (G7 — single source of truth, so `reset_test_state()` clears it too).
- [ ] **Error mapping** is `Err(Box::new(e))` (G3 — QmkError: Error+Send+Sync; no wrapper, no `.to_string()`).

### Code Quality Validation
- [ ] Mode-A rustdoc on `send_command` cites PRD §5.7/§8(4); ` ```rust,ignore ` example (G9).
- [ ] No new Cargo dependencies (only the temporary G1 pin flip, reverted before commit).
- [ ] The 6 existing tests + `notify()` body + `DeviceFilter` + `NOTIFIER`/`get_notifier`/`set_notifier` + `notify_qmk`/`debounce_worker`/`DebounceState` all UNCHANGED.
- [ ] No worker/debounce/CLI/tray wiring (scope wall — P4.M1.T2.S2/P4.M3/P5).

### Documentation & Deployment
- [ ] Mode-A rustdoc present on `send_command`; explains typed-command support + caller-owned retry/cache.
- [ ] No `docs/*.md` or README changes this task (Mode A — code-level docs only).
- [ ] Commit message / handoff notes the G1 compile gate (the code is v0.3.0-only; the permanent tag pin lands in P4.M1.T2.S1).

---

## Anti-Patterns to Avoid

- ❌ Do NOT add a default body to `send_command` (e.g. `Err("not supported")`).
      The contract pins an exact signature; both in-file impls provide real
      behavior; a default would let a future impl silently lack the capability and
      break P4.M2/P4.M3 at runtime. Make it REQUIRED (D1).
- ❌ Do NOT bake retry into `send_command`. `notify()`'s 3-attempt retry is legacy
      (the contract says "Keep notify() as-is"); the new typed primitive is a thin
      transport wrapper (build → run → map → return). Retry/cache parity with the
      string path is the CALLER's job (P4.M3.T1.S1) (D2).
- ❌ Do NOT wrap `QmkError` in a custom error type or `.to_string()` it. `QmkError:
      Error + Send + Sync`, so `Err(Box::new(e))` coerces directly to `Box<dyn
      Error + Send + Sync>` — reuse notify()'s exact idiom (G3).
- ❌ Do NOT touch `notify()`'s body. It stays the legacy string path with its own
      retry + discard-the-result semantics (G4). Do NOT reroute it through
      `send_command(SendMessage(..))`.
- ❌ Do NOT use `Mutex` for the mock's new `Vec<RunCommand>` static. The test
      module aliases `std::sync::Mutex as StdMutex` (line 379) because `Mutex` is
      shadowed at module top. Use `StdMutex` (G5).
- ❌ Do NOT clear the call log inside `reset_test_state()` directly. Add the clear
      to `reset_global_mock()` (the single source of truth both callers use) (G7).
- ❌ Do NOT forget the G1 compile gate. The pinned v0.2.1 lacks the typed types;
      `cargo build`/`cargo test` will fail on `QueryInfo`/`CommandResponse` until
      the path override (Step 0) is applied. Revert it before commit (Task 7).
- ❌ Do NOT commit the `path = "../qmk_notifier"` Cargo.toml override — it is a
      machine-local dev expedient. The permanent resolution is P4.M1.T2.S1 (flip
      the git tag to v0.3.0, after P1.M1.T4.S1 pushes it).
- ❌ Do NOT add runnable Rust doctests (` ``` `) that `use qmkonnect::...`. This is
      a binary-only crate (no lib.rs); they won't compile under `cargo test --doc`
      and `--bin` doesn't run doctests. Use ` ```rust,ignore ` or prose (G9).
- ❌ Do NOT run tests multi-threaded — `cargo test --bin qmkonnect --
      --test-threads=1` (shared `STATE`/`COND`/`WORKER`/`NOTIFIER` globals, G8).
- ❌ Do NOT edit `mod.rs`, `rules.rs`, `pattern.rs`, `types.rs`, `platforms/*`,
      `tray*`, `main.rs`, or `Cargo.toml` (beyond the temporary G1 flip). Additive
      to `src/core/notifier.rs` only.
- ❌ Do NOT wire `send_command` into the debounce worker / `notify_qmk` / any CLI
      flag / the tray. That is P4.M3 / P5. This task is the trait + impl + mock +
      recorder only (scope wall).
- ❌ Do NOT over-build the mock (e.g. a configurable response queue). The contract
      pins "store sequence of calls for ordering assertions" — a `Vec<RunCommand>`
      log + optimistic `Ack` default is the deliverable. P4.M2/P4.M3 extend the
      response behavior for THEIR tests (D4).
- ❌ Do NOT edit `PRD.md`, any `tasks.json`, `prd_snapshot.md`, `spec/*`, or any
      `plan/` file other than this item's own `PRP.md` + `research/`.

---

## Confidence Score: 9/10

This is a **small, additive, single-file change** (1 trait method + 2 impl methods
+ 1 test static + 1 accessor + 1 reset line + 5 tests) over a contract whose every
consumed symbol is verified present and reproduced verbatim (`Notifier` trait
shape, `DeviceFilter`, `QmkNotifier::notify`'s `RunParameters::new` + `run` +
`Box::new(e)` idiom, the `MockNotifier`/`reset_global_mock`/`reset_test_state`
recorder pattern, and the full qmk_notifier v0.3.0 API: `RunCommand`,
`CommandResponse`, `HostOs`, `RunParameters::new`, `run`, `QmkError`). The ONE
residual risk — the v0.2.1-vs-v0.3.0 compile gate (G1) — is fully mitigated by the
documented path-override (Validation Step 0) and the revert-before-commit rule
(Task 7). No design ambiguity remains (D1–D5 resolved with rationale); the
downstream consumers (P4.M2/P4.M3) are unblocked and the mock's fixed-`Ack` default
does not paint them into a corner (they extend, not rewrite).
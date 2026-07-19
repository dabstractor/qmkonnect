# PRP — P1.M1.T2.S2: Wire typed commands into `run()` dispatch and send path

> **Crate:** `qmk_notifier` (v0.2.1) at `/home/dustin/projects/qmk_notifier`
> (separate repo, git-tagged, pinned by QMKonnect per PRD §7/§4). Work in
> `/home/dustin/projects/qmk_notifier`.
> **Files:** `src/lib.rs` (PRIMARY) + `src/core.rs` (the 2-line S1→S2
> `#[allow(dead_code)]` handoff ONLY). No other file touched.
> **Scope line:** Replace the four `todo!()` typed arms in `run()` with a real
> dispatch that calls `build_typed_payload` (P1.M1.T2.S1) → `send_raw_report`
> (unchanged) for `QueryInfo`/`QueryCallback`/`SetOs`/`ApplyHostContext`. The
> firmware reply is still DRAINED by `burst_to_one`, so a documented
> `CommandResponse::Timeout` placeholder is returned; reply CAPTURE + parsing
> land in P1.M1.T3.S1/S2. Plus 4 deterministic dispatch tests + the
> `build_typed_payload` dead_code-allow removal.

---

## Goal

**Feature Goal**: Make `run()` actually SEND every typed `RunCommand` variant
(`QueryInfo`/`QueryCallback`/`SetOs`/`ApplyHostContext`) over the wire — building
the ETX-terminated payload via `build_typed_payload` (S1) and handing it to the
EXISTING `send_raw_report` (device cache + multi-report burst-write + IN-drain,
all unchanged) — so that the crate can issue a typed command end-to-end, leaving
only reply capture/parsing (P1.M1.T3) as the remaining gap.

**Deliverable**: `src/lib.rs` whose `run()` has NO `todo!()` arms: the 4 typed
variants dispatch through one collapsed or-pattern match arm that does
`core::build_typed_payload(&params.command)` → `send_raw_report(...)` → returns a
documented `CommandResponse::Timeout` placeholder; plus `run()`'s rustdoc updated
to describe the typed dispatch (not "stubbed with todo!()"); plus 4 new unit
tests proving each typed variant reaches `send_raw_report`. AND `src/core.rs`
with the `#[allow(dead_code)]` removed from `build_typed_payload` (its consumer
now exists) and its trailing doc sentence rewritten.

**Success Definition**: `cargo build` compiles with **zero warnings** (incl. no
"function `build_typed_payload` is never used" — its allow is gone because
`run()` now calls it); `cargo clippy --lib` introduces none; `cargo fmt --check`
exits 0; `cargo test --lib` passes with all existing tests (incl. S1's 7
`build_typed_payload_*` tests) + the 4 new dispatch tests green; NO `todo!()` in
`run()`; `send_raw_report`/`burst_to_one`/`batches_for`/`build_typed_payload`/
`RunCommand`/`HostOs`/`CommandResponse`/`error.rs`/`main.rs`/`Cargo.toml` all
unchanged except the documented `build_typed_payload` allow-removal + doc tweak.

## User Persona (if applicable)

**Target User**: The downstream implementer of P1.M1.T3.S1 (reply reader +
`parse_reply`) and P1.M1.T3.S2 (wire `parse_reply` into `run()`), and ultimately
the QMKonnect handshake/host-context pipeline (P4).

**Use Case**: `run(RunParameters{ command: RunCommand::SetOs(HostOs::Windows), vid, pid, page, usage, verbose })`
→ `run()` matches the typed arm → `build_typed_payload` yields `[0xF0, 0x03, 0x02, 0x03]`
→ `send_raw_report` frames it per-report as `[0x00, 0x81, 0x9F, 0xF0, 0x03, 0x02, 0x03, 0x00…]`
and burst-writes to every cached matching device → returns `Ok(CommandResponse::Timeout)`
(placeholder) until P1.M1.T3 captures the real ack.

**User Journey**: P4 handshake calls `run(QueryInfo)` → today (this subtask) the
bytes hit the wire correctly via the unchanged send path; tomorrow (P1.M1.T3)
the typed reply is captured and parsed into `CommandResponse::Info{…}`.

**Pain Points Addressed**: Removes the `todo!()`-panic wall that made every typed
command explode at runtime; makes the typed send path exercise the SAME proven
device-cache/burst/drain logic as legacy strings (no forked send path); leaves a
single, clearly-marked placeholder seam (the `Timeout` return) for P1.M1.T3 to
replace with real parsing.

## Why

- This is the **transport-wiring half** of the M1.T2 "Typed-Command Framing"
  task — S1 built the pure payload builder, S2 (this) is its ONE caller. After
  S2, the only remaining gap to a fully-typed round-trip is reading the reply
  (P1.M1.T3).
- It is **purely additive to the dispatch** — it reuses `send_raw_report`
  byte-for-byte (the item's "or restructure to build the full payload including
  header" alternative was already REJECTED in S1; the payload starts with `0xF0`,
  and `burst_to_one` prepends `[0x00,0x81,0x9F]` per report). No send-path fork,
  no cache/retry/drain change.
- It **closes the S1→S2 dead_code handoff**: S1 staged `build_typed_payload` as
  `#[allow(dead_code)]` with a doc note saying "remove it in S2 once `run()` calls
  this." S2 is that call, so the allow comes off and the doc is corrected.

## What

### Change 1 — `src/lib.rs`: switch the `run()` match to a borrow + collapse the typed arms

The current match is `match params.command { … }` (a MOVE). The typed arms need
`&RunCommand` (the whole command) to pass to `build_typed_payload`, which a move-
match forbids. The fix is a one-token change to `match &params.command { … }`.

> **Verified safe:** the only existing arm that binds inner data is
> `SendMessage(message)`, and it uses `message.as_bytes()` (auto-deref works on
> `&String`) plus `params.vendor_id`/`product_id`/`usage_page`/`usage`/`verbose`
> (all `Copy`, readable while `params.command` is borrowed). So switching the
> match kind requires **ZERO changes** to the `ListDevices`/`SendMessage` arm
> bodies — only the match expression token changes.

**Edit 1a** — the match expression:

```rust
// FIND (exact — prefix the `pub fn run` signature line so the anchor is
// UNIQUE: `    match params.command {` ALSO appears as a substring inside two
// TEST functions at 8-space indent (test_run_parameters_creation /
// test_run_parameters_list_devices) which inspect a constructed command and
// MUST NOT change. The fn signature pins this edit to the run() body only):
pub fn run(params: RunParameters) -> Result<CommandResponse, QmkError> {
    match params.command {
// REPLACE WITH:
pub fn run(params: RunParameters) -> Result<CommandResponse, QmkError> {
    match &params.command {
```

**Edit 1b** — replace the 4 `todo!()` typed arms + their header comment with ONE
collapsed dispatch arm. FIND this exact block:

```rust
        // --- Typed-command stubs. Dispatch + reply handling land in P1.M3.T3.S1.
        // `todo!()` expands to `!` (never), which coerces to CommandResponse, so
        // these arms compile UNCHANGED under the new signature. Do NOT wire real
        // logic here. Existing tests only construct ListDevices/SendMessage and
        // never reach these arms. ---
        RunCommand::QueryInfo => todo!("typed dispatch lands in P1.M3.T3.S1"),
        RunCommand::QueryCallback(_) => todo!("typed dispatch lands in P1.M3.T3.S1"),
        RunCommand::SetOs(_) => todo!("typed dispatch lands in P1.M3.T3.S1"),
        RunCommand::ApplyHostContext { .. } => {
            todo!("typed dispatch lands in P1.M3.T3.S1")
        }
```

REPLACE WITH:

```rust
        // --- Typed-command dispatch (v0.3.0). Each typed variant builds its
        // ETX-terminated payload via `build_typed_payload` (core.rs, P1.M1.T2.S1)
        // and sends it through the SAME `send_raw_report` path as legacy strings
        // (MatchKey device-cache lookup, multi-report burst-write, bounded
        // IN-drain). The firmware reply is currently DRAINED and discarded by
        // `burst_to_one`; reply CAPTURE + `parse_reply` land in P1.M1.T3.S1/S2,
        // which will replace the `CommandResponse::Timeout` placeholder below
        // with the real typed `CommandResponse`.
        //
        // Arms are collapsed into one or-pattern (not one-per-variant) because
        // the build+send is identical across variants. Per-variant divergence
        // arrives with reply PARSING (P1.M1.T3.S2), which will split this arm as
        // needed (or leave it collapsed if `parse_reply` decodes generically by
        // the `reply[1]` cmd-echo). Per-variant REQUEST docs live on the
        // `RunCommand` enum variants themselves. ---
        RunCommand::QueryInfo
        | RunCommand::QueryCallback(_)
        | RunCommand::SetOs(_)
        | RunCommand::ApplyHostContext { .. } => {
            let payload = core::build_typed_payload(&params.command);
            send_raw_report(
                &payload,
                params.vendor_id,
                params.product_id,
                params.usage_page,
                params.usage,
                params.verbose,
            )?;
            // Placeholder: the typed reply is drained, not captured. Reply
            // capture (P1.M1.T3.S1) replaces this with the real CommandResponse.
            Ok(CommandResponse::Timeout)
        }
```

**Edit 1c** — update `run()`'s `///` rustdoc. The current 3rd bullet says typed
variants are "stubbed with `todo!()`" (now false). FIND this exact doc block
(inside the `///` on `run`):

```rust
/// - [`RunCommand::SendMessage`] → [`CommandResponse::Legacy`] as a
///   **placeholder** (`matched: true`) until real reply parsing lands in
///   P1.M3.T3; the firmware's `response[0]` match-bool will be decoded there.
/// - [`RunCommand::ListDevices`] → [`CommandResponse::Timeout`]: no device
///   reply was captured because nothing was sent over the wire (list-only path).
/// - Typed variants (`QueryInfo`/`QueryCallback`/`SetOs`/`ApplyHostContext`)
///   are stubbed with `todo!()` until full dispatch + reply capture land in
///   P1.M3.T3.
```

REPLACE WITH:

```rust
/// - [`RunCommand::SendMessage`] → [`CommandResponse::Legacy`] as a
///   **placeholder** (`matched: true`) until real reply parsing lands in
///   P1.M1.T3; the firmware's `response[0]` match-bool will be decoded there.
/// - [`RunCommand::ListDevices`] → [`CommandResponse::Timeout`]: no device
///   reply was captured because nothing was sent over the wire (list-only path).
/// - Typed variants (`QueryInfo`/`QueryCallback`/`SetOs`/`ApplyHostContext`)
///   → build their ETX-terminated payload via `build_typed_payload` and send it
///   through the SAME [`send_raw_report`] path as legacy strings (device cache,
///   multi-report burst-write, IN-drain). The reply is currently DRAINED (not
///   captured) by `burst_to_one`, so a [`CommandResponse::Timeout`] placeholder
///   is returned; reply capture (P1.M1.T3.S1) will replace it with the real
///   typed [`CommandResponse`].
```

> Note: this also fixes the stale `P1.M3.T3` → `P1.M1.T3` plan numbering in the
> `SendMessage` and typed bullets (the code predates the renumbered plan tree).

### Change 2 — `src/core.rs`: the S1→S2 dead_code handoff (2 lines)

S1 staged `build_typed_payload` with `#[allow(dead_code)]` because only tests
referenced it. Now `run()` calls it (compiled, non-`cfg(test)` code), so the
allow is a no-op and S1's own doc says to remove it.

**Edit 2a** — delete the `#[allow(dead_code)]` attribute line directly above
`build_typed_payload`. FIND:

```rust
/// Consumer: the `run()` typed-dispatch arm (P1.M1.T2.S2). Until then this is
/// referenced only by tests, hence `#[allow(dead_code)]` — remove it in S2
/// once `run()` calls this.
#[allow(dead_code)]
pub(crate) fn build_typed_payload(cmd: &crate::RunCommand) -> Vec<u8> {
```

REPLACE WITH:

```rust
/// Consumer: the `run()` typed-dispatch arm in [`crate::run`] (P1.M1.T2.S2).
/// This is the request-side counterpart to the reply-side `parse_reply`
/// (P1.M1.T3.S1); the payload it returns feeds the unchanged
/// [`send_raw_report`] / [`burst_to_one`] send path.
pub(crate) fn build_typed_payload(cmd: &crate::RunCommand) -> Vec<u8> {
```

(If S1's exact doc wording differs slightly, the intent is: **remove the
`#[allow(dead_code)]` line** and **rewrite the trailing doc sentence** so it no
longer claims the fn is "referenced only by tests" / "hence `#[allow(dead_code)]`
— remove it in S2". Keep the rest of the function's rustdoc and body UNCHANGED.)

> **Do NOT touch** the `#[allow(dead_code)]` on `RESPONSE_MARKER` or
> `REPLY_READ_TIMEOUT_MS` — their consumers land in P1.M1.T3 (`parse_reply` +
> reply reader). And do NOT touch the 5 command constants
> (`CMD_DISCRIMINATOR`/`CMD_QUERY_INFO`/`CMD_QUERY_CALLBACK`/`CMD_SET_OS`/
> `CMD_APPLY_HOST_CONTEXT`) — S1 already removed their allows.

### Change 3 — `src/lib.rs`: add 4 dispatch tests to the existing `mod tests`

Append these 4 tests inside the existing `#[cfg(test)] mod tests { use super::*; … }`
block in `src/lib.rs` (after the last existing test, before the closing `}`):

```rust
    #[test]
    fn test_run_query_info_dispatches_to_send() {
        // Typed dispatch must BUILD + SEND (not `todo!()` panic). A bogus VID/PID
        // guarantees the device filter (`vendor_id.is_none_or(|v| dev_vid == v)`)
        // matches NOTHING on any machine — even one with a real QMK keyboard — so
        // `send_raw_report` deterministically returns `DeviceNotFound`. A
        // `todo!()` would have panicked and failed this test, so the assertion
        // proves the arm wired through to `send_raw_report`. Reply capture +
        // parsing land in P1.M1.T3; that is out of scope here.
        let params = RunParameters::new(
            RunCommand::QueryInfo,
            Some(0xDEAD),
            Some(0xBEEF),
            DEFAULT_USAGE_PAGE,
            DEFAULT_USAGE,
            false,
        );
        let result = run(params);
        assert!(
            matches!(result, Err(QmkError::DeviceNotFound { .. })),
            "QueryInfo must dispatch to send_raw_report; expected DeviceNotFound with bogus VID/PID, got {result:?}",
        );
    }

    #[test]
    fn test_run_query_callback_dispatches_to_send() {
        // Same dispatch proof as QueryInfo, but for an arg-carrying variant
        // (index = 5). build_typed_payload correctness is S1's job; here we only
        // assert the arm reaches send_raw_report.
        let params = RunParameters::new(
            RunCommand::QueryCallback(5),
            Some(0xDEAD),
            Some(0xBEEF),
            DEFAULT_USAGE_PAGE,
            DEFAULT_USAGE,
            false,
        );
        let result = run(params);
        assert!(
            matches!(result, Err(QmkError::DeviceNotFound { .. })),
            "QueryCallback must dispatch to send_raw_report; expected DeviceNotFound, got {result:?}",
        );
    }

    #[test]
    fn test_run_set_os_dispatches_to_send() {
        // Arg-carrying variant: HostOs::Linux ⇒ os_byte 1. Proves SetOs dispatches.
        let params = RunParameters::new(
            RunCommand::SetOs(HostOs::Linux),
            Some(0xDEAD),
            Some(0xBEEF),
            DEFAULT_USAGE_PAGE,
            DEFAULT_USAGE,
            false,
        );
        let result = run(params);
        assert!(
            matches!(result, Err(QmkError::DeviceNotFound { .. })),
            "SetOs must dispatch to send_raw_report; expected DeviceNotFound, got {result:?}",
        );
    }

    #[test]
    fn test_run_apply_host_context_dispatches_to_send() {
        // Struct-arg variant: layer=Some(224), 3 callbacks, clear_board=true.
        // Exercises the multi-field payload path through build_typed_payload and
        // proves ApplyHostContext dispatches.
        let params = RunParameters::new(
            RunCommand::ApplyHostContext {
                layer: Some(224),
                callbacks: vec![1, 2, 3],
                clear_board: true,
            },
            Some(0xDEAD),
            Some(0xBEEF),
            DEFAULT_USAGE_PAGE,
            DEFAULT_USAGE,
            false,
        );
        let result = run(params);
        assert!(
            matches!(result, Err(QmkError::DeviceNotFound { .. })),
            "ApplyHostContext must dispatch to send_raw_report; expected DeviceNotFound, got {result:?}",
        );
    }
```

> The tests use `RunCommand`, `HostOs`, `RunParameters`, `run`, `QmkError`,
> `DEFAULT_USAGE_PAGE`, `DEFAULT_USAGE` — ALL already in scope via the existing
> `use super::*;` at the top of `mod tests` (the `DEFAULT_*` consts are
> re-exported at the crate root; `HostOs`/`RunCommand`/`CommandResponse` are
> defined there). No new imports needed.

### Success Criteria

- [ ] `run()` contains NO `todo!()`; all 6 `RunCommand` variants dispatch
      (`ListDevices`/`SendMessage` unchanged; the 4 typed variants via the
      collapsed or-pattern arm).
- [ ] The typed arm calls `core::build_typed_payload(&params.command)` then
      `send_raw_report(&payload, params.vendor_id, params.product_id,
      params.usage_page, params.usage, params.verbose)?` and returns
      `Ok(CommandResponse::Timeout)` (documented placeholder).
- [ ] `match params.command` → `match &params.command` (the borrow fix).
- [ ] `run()`'s `///` doc updated: typed variants "build + send" (not "stubbed
      with todo!()"); `P1.M3` → `P1.M1` numbering fixed.
- [ ] `#[allow(dead_code)]` removed from `build_typed_payload` in `core.rs`;
      its trailing doc sentence rewritten (no longer claims test-only use).
      `RESPONSE_MARKER`/`REPLY_READ_TIMEOUT_MS` keeps its allow; the 5 command
      constants stay allow-free (S1 already did them).
- [ ] 4 new dispatch tests added; all pass; bogus VID/PID ⇒ deterministic
      `DeviceNotFound`.
- [ ] `send_raw_report`/`burst_to_one`/`batches_for`/`build_typed_payload` body/
      `RunCommand`/`HostOs`/`CommandResponse`/`error.rs`/`main.rs`/`Cargo.toml`
      unchanged except the documented `build_typed_payload` allow-removal + doc.
- [ ] `cargo build` → zero warnings; `cargo clippy --lib` → no new warnings;
      `cargo fmt --check` → exit 0; `cargo test --lib` → all pass.

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything
> needed to implement this successfully?"_ — **Yes.** The exact edit anchors
> (verbatim FIND/REPLACE for the match token, the `todo!()` block, the `run()`
> doc, the `build_typed_payload` allow), the exact replacement code (verbatim),
> the exact 4 tests (verbatim, ready to paste), the verified borrow-check
> reasoning, the deterministic test strategy, and verified build/clippy/fmt/test
> commands are all below. The implementer does not need to read the firmware
> source — the S1 PRP + `firmware_wire_contract.md` already canonicalized every
> byte, and this task adds NO new bytes (it reuses `build_typed_payload`'s output).

### Documentation & References

```yaml
# MUST READ — the previous subtask's PRP (the CONTRACT for build_typed_payload).
# S1 is being implemented in parallel and lands FIRST; S2 consumes its output.
- file: /home/dustin/projects/qmkonnect/plan/002_637d65b6e9b8/P1M1T2S1/PRP.md
  why: "Defines the exact signature `pub(crate) fn build_typed_payload(cmd:
        &crate::RunCommand) -> Vec<u8>`, its placement (after batches_for, before
        MatchKey in core.rs), its return shape ([0xF0][cmd_id][args][0x03],
        SendMessage/ListDevices => Vec::new()), and — critically — the S1→S2
        dead_code handoff: S1 stages the fn as #[allow(dead_code)] with a doc
        note saying 'remove it in S2 once run() calls this'. S2 is that call."
  section: "Goal, The new build_typed_payload function, Integration Points
            (DOWNSTREAM CONSUMER)"
  critical: "build_typed_payload's payload STARTS WITH 0xF0 (not 0x81 0x9F —
             burst_to_one prepends those per report). S2 hands the Vec straight
             to send_raw_report with NO further framing (ETX is already appended
             by the builder). Do NOT re-append ETX in run() (that would double-
             terminate); do NOT prepend 0x81 0x9F (that would double-header)."

# MUST READ — the file containing run() (the PRIMARY edit target).
- file: /home/dustin/projects/qmk_notifier/src/lib.rs
  why: "Contains run() (the match on params.command + the 4 todo!() typed arms
        to replace), RunCommand/HostOs/CommandResponse (the type surface, S1/T1
        output — DO NOT touch), RunParameters (the struct whose Copy fields are
        read in the arms), parse_cli_args (untouched), and the #[cfg(test)] mod
        tests (where the 4 dispatch tests go)."
  pattern: "run() is a single big match returning Result<CommandResponse,QmkError>.
            The SendMessage arm shows the verbose-block + send_raw_report call
            shape to mirror. The mod tests block uses `use super::*;` (brings
            crate-root types + re-exported consts into scope)."
  gotcha: "The match is currently `match params.command` (a MOVE). Switching to
           `match &params.command` (borrow) is REQUIRED so the typed arms can
           pass `&params.command` to build_typed_payload. Verified: the only arm
           that binds inner data is SendMessage(message), which uses only
           message.as_bytes() (auto-deref) + params.* Copy fields — so the
           borrow-match needs NO body changes to existing arms."

# MUST READ — the file containing build_typed_payload + send_raw_report.
- file: /home/dustin/projects/qmk_notifier/src/core.rs
  why: "(a) build_typed_payload lives here (S1) — S2 removes its
        #[allow(dead_code)] + rewrites its trailing doc sentence (the ONLY
        core.rs edit). (b) send_raw_report lives here — S2 CALLS it but does NOT
        change it. (c) burst_to_one shows why the reply is drained (the IN-drain
        loop) — explains the Timeout placeholder."
  section: "build_typed_payload (after batches_for), send_raw_report, burst_to_one
            (the IN-drain loop), the constants block (RESPONSE_MARKER +
            REPLY_READ_TIMEOUT_MS keep their allows)"
  critical: "Do NOT change send_raw_report/burst_to_one/batches_for — the typed
             payload feeds the UNCHANGED send path. Do NOT remove
             #[allow(dead_code)] from RESPONSE_MARKER or REPLY_READ_TIMEOUT_MS
             (P1.M1.T3 consumers). Do NOT touch the 5 command constants (S1
             already removed their allows). ONLY remove the allow on
             build_typed_payload itself."

# REFERENCE — the wire contract (canonical byte layouts). S2 adds no new bytes,
# but this explains WHY the payload starts with 0xF0 and the per-variant args.
- file: /home/dustin/projects/qmk_notifier/plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md
  why: "§Typed-Command Framing: the 33-byte hidapi buffer
        [0x00][0x81][0x9F][0xF0][cmd_id][args][0x03] — confirms the discriminator
        lands at firmware data[2] (so the payload handed to send_raw_report
        starts with 0xF0). §Command Table: per-cmd request args (S1 encodes
        these; S2 just forwards the Vec). §Reply Disambiguation: why the Timeout
        placeholder is the honest interim value (reply[0]==0x51 ⇒ typed;
        0/1 ⇒ legacy; no reply ⇒ timeout)."
  section: "Typed-Command Framing, Command Table, Reply Disambiguation"

# REFERENCE — crate PRD (public API + framing prose).
- file: /home/dustin/projects/qmk_notifier/PRD.md
  why: "§7 Crate Spec: run() returns Result<CommandResponse,_>; 'Typed variants
        build [0x81,0x9F,0xF0,cmd, args…] and reuse the same ETX-framed,
        multi-report chunking as strings.' §4.2 run(SendMessage) flow documents
        the send_raw_report device-cache/burst/drain path S2 reuses."
  section: "7. Crate Spec, 4.2 run(SendMessage) flow, 4.3 Error types"

# REFERENCE — research notes for THIS subtask (design decisions).
- docfile: plan/002_637d65b6e9b8/P1M1T2S2/research/notes.md
  why: "Documents the borrow-check decision (match &params.command), the
        collapsed-arm decision (DRY; T3.S2 splits if needed), the Timeout-
        placeholder rationale (drained reply; no consumer yet), the deterministic
        bogus-VID/PID test strategy, and the S1→S2 dead_code handoff."
```

### Current Codebase tree (run from the crate root `/home/dustin/projects/qmk_notifier`)

```bash
qmk_notifier/
├── Cargo.toml          # name="qmk_notifier", version="0.2.1", edition="2021"
│                       # deps: clap, hidapi, (toml/dirs/serde unused legacy — DO NOT TOUCH)
├── Cargo.lock
├── README.md
├── PRD.md              # crate PRD (§7, §4.2)
├── .gitignore          # contains only: /target
├── plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md   # WIRE SOURCE OF TRUTH
└── src
    ├── main.rs         # binary entrypoint — constructs ONLY SendMessage/ListDevices. DO NOT TOUCH.
    ├── error.rs        # QmkError (DeviceNotFound is a struct variant) — DO NOT TOUCH.
    ├── lib.rs          # <-- PRIMARY EDIT: run() match + typed arms + run() doc + 4 tests.
    │                   #   RunCommand/HostOs/CommandResponse/RunParameters/parse_cli_args — DO NOT TOUCH.
    └── core.rs         # <-- SECONDARY EDIT (2 lines): remove build_typed_payload's
                        #     #[allow(dead_code)] + rewrite its trailing doc sentence.
                        #     send_raw_report/burst_to_one/batches_for/constants — DO NOT TOUCH.
```

### Desired Codebase tree with files to be modified

```bash
src/
├── lib.rs   # MODIFIED: match &params.command; collapsed typed-dispatch arm (build+send+Timeout
│            #   placeholder); run() rustdoc updated; 4 dispatch tests appended to mod tests.
└── core.rs  # MODIFIED (2 lines): #[allow(dead_code)] removed from build_typed_payload;
             #   its trailing doc sentence rewritten.
# (no new files; main.rs/error.rs/Cargo.toml untouched)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL: the match MUST switch from `match params.command` (MOVE) to
//   `match &params.command` (BORROW). The typed arms pass &params.command to
//   build_typed_payload, which is impossible if params.command was moved into
//   the match. Verified safe: the SendMessage(message) arm only uses
//   message.as_bytes() (auto-deref on &String) + params.* Copy fields, so the
//   borrow-match needs NO body changes to existing arms.

// CRITICAL: the payload from build_typed_payload ALREADY starts with 0xF0 and
//   ALREADY ends with 0x03 (ETX). Hand it STRAIGHT to send_raw_report — do NOT
//   prepend 0x81 0x9F (burst_to_one does that per report) and do NOT append
//   another 0x03 (that would double-terminate). The legacy SendMessage path
//   appends ETX inside run(); the typed path folds ETX into the builder, so the
//   dispatch is literally `send_raw_report(&build_typed_payload(&cmd), …)`.

// CRITICAL: remove #[allow(dead_code)] from build_typed_payload ONLY.
//   RESPONSE_MARKER and REPLY_READ_TIMEOUT_MS MUST KEEP theirs (P1.M1.T3
//   consumers: parse_reply + reply reader). The 5 command constants
//   (CMD_DISCRIMINATOR/CMD_QUERY_INFO/CMD_QUERY_CALLBACK/CMD_SET_OS/
//   CMD_APPLY_HOST_CONTEXT) already lost theirs in S1 — leave them allow-free.

// CRITICAL: return CommandResponse::Timeout as a PLACEHOLDER for typed sends,
//   NOT a per-variant value (Ack{ok:true}, Info{…}). We did NOT parse any reply
//   (burst_to_one DRAINS it), so returning a "parsed" value would be a lie.
//   Timeout is honest ("no reply captured") and no consumer reads it yet (typed
//   commands are unreachable from the CLI; the P4 caller lands after P1.M1.T3
//   replaces this placeholder). Document it in the arm comment + run() rustdoc.

// CRITICAL: build_typed_payload is `pub(crate)` in core.rs (private module
//   `mod core;`) and is NOT in the `pub use core::{ … }` re-export at the crate
//   root (only send_raw_report / list_hid_devices / parse_hex_or_decimal / the
//   DEFAULT_* consts are re-exported). So from run() in lib.rs you MUST call it
//   as `core::build_typed_payload(&params.command)` — a BARE `build_typed_payload(…)`
//   fails with E0425 "cannot find function `build_typed_payload` in this scope"
//   (verified: there is NO `use core::build_typed_payload;` import). Contrast
//   send_raw_report, which IS re-exported and so is callable bare (as the
//   existing SendMessage arm does). This is the #1 compile trap in this task.

// GOTCHA: the tests must use a BOGUS VID/PID (e.g. Some(0xDEAD), Some(0xBEEF))
//   so the device filter matches nothing and send_raw_report returns
//   DeviceNotFound deterministically — even on a dev box with a real QMK
//   keyboard (the match predicate requires dev_vid == Some(0xDEAD), which no
//   real device satisfies). Using the default VID 0xFEED risks hitting real
//   hardware and getting a non-deterministic Ok/partial result.

// GOTCHA: QmkError::DeviceNotFound is a STRUCT variant —
//   `QmkError::DeviceNotFound { vendor_id, product_id, usage_page, usage }`.
//   Match it as `Err(QmkError::DeviceNotFound { .. })` (confirmed in error.rs).

// GOTCHA: the existing run() doc references the OLD plan numbering "P1.M3.T3".
//   The current plan tree uses "P1.M1.T3". Fix this drift while editing the doc
//   (the todo!() messages that contained "P1.M3.T3.S1" are being deleted, so the
//   only remaining references are in the doc bullets — update them to P1.M1.T3).

// NOTE: main.rs only constructs SendMessage/ListDevices via parse_cli_args, so
//   the typed dispatch is NOT reachable from the CLI binary yet (typed commands
//   are issued programmatically by QMKonnect P4). No main.rs change is needed or
//   wanted (CLI flags for typed commands are P5.M1, out of scope).

// NOTE: the SendMessage arm currently returns Ok(CommandResponse::Legacy{matched:true})
//   as ITS placeholder. Leave it — S2 does not change the SendMessage path.
```

## Implementation Blueprint

### Data models and structure

No new types. S2 wires existing types together: `RunParameters` → `run()` →
`build_typed_payload(&RunCommand)` → `send_raw_report(&[u8], …)` →
`CommandResponse::Timeout`. The "structure" is the dispatch flow, fixed by the
collapsed or-pattern arm above.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: READ current state of src/lib.rs and src/core.rs (anchors)
  - READ: /home/dustin/projects/qmk_notifier/src/lib.rs (run() + its /// doc +
          the mod tests block) and src/core.rs (build_typed_payload + the
          constants block).
  - LOCATE in lib.rs: (a) `match params.command {`; (b) the 4 todo!() typed arms
          + their header comment; (c) run()'s `///` 3-bullet doc; (d) the end of
          the `#[cfg(test)] mod tests` block (where the 4 tests append).
  - LOCATE in core.rs: the `#[allow(dead_code)]` line directly above
          `pub(crate) fn build_typed_payload`, and its trailing doc sentence.
  - CONFIRM: build_typed_payload exists with #[allow(dead_code)] (S1 landed).
          If S1 has NOT landed yet (build_typed_payload absent), STOP — this
          subtask depends on S1; re-check the plan status.
  - CONFIRM: send_raw_report signature is (data, vendor_id, product_id,
          usage_page, usage, verbose) -> Result<(), QmkError> (it is — unchanged).

Task 2: EDIT src/lib.rs — switch the match to a borrow (Edit 1a)
  - REPLACE the run() match line. NOTE: `    match params.command {` is NOT
          unique — it also appears inside two TEST fns at 8-space indent that
          MUST NOT change. Use the `pub fn run(...)` signature as the anchor
          prefix (see Edit 1a's exact FIND/REPLACE in "What").
  - The net change is `match params.command` → `match &params.command` in the
          run() body ONLY.
  - VERIFY the SendMessage/ListDevices arms STILL COMPILE unchanged after this
          (message becomes &String; message.as_bytes() auto-derefs). `cargo build`
          after this edit alone should still pass (todo!() arms still present).

Task 3: EDIT src/lib.rs — replace the todo!() arms with the collapsed dispatch (Edit 1b)
  - REPLACE the 4 todo!() arms + header comment (exact FIND text in "What") with
          the collapsed or-pattern arm (exact REPLACE text in "What").
  - CHECK: the arm calls core::build_typed_payload(&params.command) then send_raw_report(...)
          then returns Ok(CommandResponse::Timeout). The `?` propagates transport
          errors (DeviceNotFound etc.) exactly like the SendMessage arm.

Task 4: EDIT src/lib.rs — update run()'s /// doc (Edit 1c)
  - REPLACE the 3-bullet doc block (exact FIND in "What") with the updated text
          (exact REPLACE in "What"): typed variants now "build + send" (not
          "stubbed with todo!()"); P1.M3 → P1.M1 numbering fixed.

Task 5: EDIT src/core.rs — the dead_code handoff (Edits 2a)
  - DELETE the `#[allow(dead_code)]` line directly above build_typed_payload.
  - REWRITE its trailing doc sentence (the "Consumer: … Until then … hence
          #[allow(dead_code)] …" part) to state the real consumer (run()).
          Exact FIND/REPLACE in "What" (Edit 2a). If S1's exact wording differs,
          match the intent: remove the allow + drop the "test-only / remove in
          S2" prose.
  - DO NOT touch RESPONSE_MARKER / REPLY_READ_TIMEOUT_MS allows, the 5 command
          constants, or anything else in core.rs.

Task 6: ADD the 4 dispatch tests to src/lib.rs mod tests (Change 3)
  - APPEND the 4 #[test] fns (verbatim in "What") at the end of the existing
          `#[cfg(test)] mod tests { use super::*; … }` block, before its closing `}`.
  - NAMES: test_run_query_info_dispatches_to_send,
          test_run_query_callback_dispatches_to_send,
          test_run_set_os_dispatches_to_send,
          test_run_apply_host_context_dispatches_to_send.
  - Each asserts `matches!(run(params), Err(QmkError::DeviceNotFound { .. }))`
          with bogus VID/PID (0xDEAD/0xBEEF) — deterministic, no hardware.
  - NO new imports (use super::* covers RunCommand/HostOs/RunParameters/run/
          QmkError/DEFAULT_USAGE_PAGE/DEFAULT_USAGE).

Task 7: VALIDATE (do not skip)
  - RUN (from /home/dustin/projects/qmk_notifier):
          cargo fmt && cargo build && cargo clippy --lib &&
          cargo fmt --check && cargo test --lib
  - EXPECT: build 0 warnings (NO "function build_typed_payload is never used" —
          its allow is gone because run() calls it); clippy no new warnings;
          fmt --check exit 0; all tests pass (existing incl. S1's 7 + the 4 new).
  - IF "function build_typed_payload is never used": the typed arm isn't calling
          it — check Edit 1b landed and the arm body references
          core::build_typed_payload(&params.command).
  - IF E0500 "cannot move out of ... in pattern": you forgot Edit 1a (the match
          is still `match params.command` move, but the typed arm borrows
          &params.command). Apply Edit 1a.
  - SANITY: `git diff --stat` shows only src/lib.rs and src/core.rs changed.
```

### Implementation Patterns & Key Details

```rust
// === WHY match &params.command (not match params.command) ===
//   build_typed_payload needs &RunCommand (the WHOLE command). A move-match
//   `match params.command` consumes params.command, so the typed arms cannot
//   borrow it back. Switching to `match &params.command` borrows it; the arms
//   then pass &params.command to build_typed_payload, and params.vendor_id etc.
//   (Copy fields) stay readable. Verified: the SendMessage(message) arm only
//   uses message.as_bytes() (auto-deref on &String) — no owned-String use — so
//   the existing arm bodies compile unchanged under the borrow-match.

// === WHY collapse the 4 typed arms into one or-pattern ===
//   The build+send is identical across QueryInfo/QueryCallback/SetOs/
//   ApplyHostContext. A collapsed or-pattern is the DRY, idiomatic choice.
//   Per-variant divergence arrives with reply PARSING (P1.M1.T3.S2), which will
//   split the arm (trivial) — or leave it collapsed if parse_reply decodes
//   generically by reply[1] cmd-echo. Per-variant REQUEST docs already live on
//   the RunCommand enum variants (S1), so collapsing loses no documentation.

// === WHY the Timeout placeholder (not Ack/Info) ===
//   burst_to_one DRAINS the IN reply (discards it). We have NO parsed reply, so
//   returning a "parsed" CommandResponse variant would be a lie. Timeout is
//   honest ("no reply captured"). No consumer reads it yet: typed commands are
//   unreachable from the CLI (main.rs only does SendMessage/ListDevices), and
//   the P4 programmatic caller lands AFTER P1.M1.T3 replaces this placeholder.
//   The arm comment + run() rustdoc make the placeholder status explicit.

// === WHY bogus VID/PID in the tests ===
//   send_raw_report does real HID I/O (no mock seam). The device-match predicate
//   requires dev_vid == Some(0xDEAD) when the key's vendor_id is Some — no real
//   device satisfies that, so DeviceNotFound is deterministic on ANY machine
//   (even one with a QMK keyboard). A todo!() would panic and fail the test, so
//   the assertion proves dispatch wiring without touching real hardware.

// === The ? propagation ===
//   send_raw_report returns Result<(), QmkError>. The `?` in the typed arm
//   propagates DeviceNotFound / DeviceOpenError / PartialSendError /
//   SendReportError exactly as the SendMessage arm does — so transport failures
//   surface to the caller with the same error types QMKonnect already retries on
//   ("no device found" / "permission denied" / "failed to open" — PRD §4.3).
```

### Integration Points

```yaml
SOURCE FILES:
  - modify (primary): "/home/dustin/projects/qmk_notifier/src/lib.rs"
      - match params.command → match &params.command
      - 4 todo!() arms → 1 collapsed typed-dispatch arm (build+send+Timeout)
      - run() /// doc updated
      - 4 dispatch tests appended to mod tests
  - modify (secondary, 2 lines): "/home/dustin/projects/qmk_notifier/src/core.rs"
      - remove #[allow(dead_code)] from build_typed_payload
      - rewrite its trailing doc sentence

DEPENDENCIES / Cargo.toml:
  - none. No new crate deps.

PUBLIC API SURFACE:
  - run()'s SIGNATURE is unchanged (Result<CommandResponse, QmkError> — T1.S2
    already set it). Only the dispatch BODY changes. build_typed_payload stays
    pub(crate) (internal transport helper, NOT re-exported).

CONSUMES (treat as fixed, already landed by the time S2 runs):
  - P1.M1.T1.S1 (Complete): RunCommand (6 variants) + HostOs.
  - P1.M1.T1.S2 (Complete): CommandResponse enum; run() returns Result<CommandResponse,_>.
  - P1.M1.T2.S1 (lands first): build_typed_payload + its #[allow(dead_code)] stage.

DOWNSTREAM CONSUMER (do NOT implement now — listed for awareness):
  - P1.M1.T3.S1: "response reader + parse_reply" — modifies burst_to_one to
    OPTIONALLY return the first IN report instead of draining all; adds
    parse_reply (uses RESPONSE_MARKER + REPLY_READ_TIMEOUT_MS, currently
    allow-dead). This is what replaces S2's Timeout placeholder with real data.
  - P1.M1.T3.S2: "wire parse_reply into run()" — replaces the Timeout placeholder
    in the typed arm with parse_reply(reply, &params.command). May split the
    collapsed arm if per-variant parsing is needed.

OUT-OF-SCOPE (later subtasks — do NOT implement here):
  - P1.M1.T3.S1/S2: reply capture + parse_reply + wiring (replaces the placeholder).
  - P1.M1.T4.S1: bump crate version to 0.3.0 + tag.
  - P5.M1: CLI flags exposing typed commands (main.rs change).
```

## Validation Loop

> All commands run from the crate root: `/home/dustin/projects/qmk_notifier`

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmk_notifier

# Format the edited files (rustfmt default — no rustfmt.toml exists).
cargo fmt

# Build the whole crate — MUST compile with ZERO warnings.
cargo build 2>&1 | tee /tmp/build.log
# Expected: "Finished `dev` profile ..." and NO "warning:" lines.
#   - If "function `build_typed_payload` is never used": the typed arm didn't
#     land (Edit 1b) OR you forgot to remove its #[allow(dead_code)] is fine but
#     the call is missing — check the arm calls core::build_typed_payload(&params.command).
#     (If you LEFT the allow on by mistake, build still passes but clippy may
#      flag a redundant attribute — remove it per Edit 2a.)
#   - If E0500 "cannot move out of `params.command`": you forgot Edit 1a — the
#     match is still `match params.command` (move) but the arm borrows
#     &params.command. Change to `match &params.command`.
#   - If E0425 "cannot find function `build_typed_payload` in this scope": you
#     called it BARE. build_typed_payload is pub(crate) in `mod core` but is NOT
#     re-exported at the crate root, so it must be called as
#     `core::build_typed_payload(&params.command)` (or add a `use core::build_typed_payload;`).
#     Do NOT change the function to pub — qualify the call.

# Lint (default clippy — no .clippy.toml exists).
cargo clippy --lib 2>&1 | tee /tmp/clippy.log
# Expected: no warnings/errors specific to the typed arm or its tests.
#   clippy may suggest match ergonomics or `?` reshaping — accept sensible fixes,
#   but do NOT change the dispatch shape or the Timeout placeholder.

# Formatting check (CI-style gate).
cargo fmt --check
# Expected: exit code 0 (no diff). If non-zero, re-run `cargo fmt`.

# Sanity: confirm ONLY lib.rs and core.rs changed.
git diff --stat
# Expected: only src/lib.rs and src/core.rs listed.
```

### Level 2: Unit Tests (Component Validation)

```bash
cd /home/dustin/projects/qmk_notifier

# Run the 4 new dispatch tests in isolation first.
cargo test --lib test_run_query_info_dispatches_to_send -- --nocapture
cargo test --lib test_run_query_callback_dispatches_to_send -- --nocapture
cargo test --lib test_run_set_os_dispatches_to_send -- --nocapture
cargo test --lib test_run_apply_host_context_dispatches_to_send -- --nocapture
# Expected: each passes; each exercises run() with a typed variant and bogus
#   VID/PID ⇒ Err(DeviceNotFound) (proving dispatch, not todo!()).

# Run the full lib test suite.
cargo test --lib
# Expected: "test result: ok. <N> passed; 0 failed; 0 ignored; ...".
#   N = (existing lib.rs tests incl. S1's none-in-lib) + 4 new. The exact N is
#   not load-bearing; the gate is 0 failed.

# Cross-check: S1's build_typed_payload tests still pass (the builder is
# unchanged by S2; only its allow-dead attribute moved). These live in core.rs.
cargo test --lib build_typed_payload_ -- --nocapture
# Expected: 7 passed (S1's tests) — proves S2 did not regress the builder.

# Cross-check: the pre-existing run() tests still pass (no regression from the
# match-kind switch on the SendMessage/ListDevices arms).
cargo test --lib test_run_with_ -- --nocapture
# Expected: test_run_with_list_devices_command, test_run_with_send_message_command,
#   test_run_with_verbose_output all pass (the borrow-match didn't break them).
```

### Level 3: Integration Testing (System Validation)

```text
PARTIALLY APPLICABLE. run() with a typed command does real HID I/O via
send_raw_report, so a full round-trip needs a QMK keyboard with the v0.3.0
typed-command firmware (P1.M2 — not done yet). WITHOUT such hardware:

  - The 4 dispatch tests (Level 2) ARE the integration proof that run() routes
    typed commands to send_raw_report (DeviceNotFound with bogus VID/PID proves
    the send path was reached; a todo!() would have panicked).
  - On a dev box WITH a real (legacy, v0.2.x) QMK keyboard, an ad-hoc check:
      cargo run --quiet --       # the binary only takes a message or --list, so
                                 # typed commands are NOT reachable via the CLI yet.
    ⇒ there is NO CLI path to a typed command at this stage (typed commands are
    issued programmatically by QMKonnect P4, which lands after P1.M1.T3). So the
    Level-2 unit tests are the end-to-end verification for THIS subtask.

  Live-hardware validation of the typed round-trip is deferred to P1.M1.T3
  (reply capture) + P1.M2 (firmware) — out of scope here.
```

### Level 4: Creative & Domain-Specific Validation

```bash
cd /home/dustin/projects/qmk_notifier

# Confirm rustdoc renders (Mode A documentation): run()'s updated doc + the
# build_typed_payload doc tweak.
cargo doc --lib --no-deps 2>&1 | grep -iE "warning|error" || echo "docs clean (good)"

# Confirm NO todo!() remains in run() (the whole point of this subtask).
grep -n "todo!" src/lib.rs || echo "no todo!() in lib.rs (good)"

# Confirm the typed arm actually calls build_typed_payload (the S1→S2 handoff).
grep -n "build_typed_payload" src/lib.rs
# Expected: one hit — the `let payload = core::build_typed_payload(&params.command);` line.

# Confirm build_typed_payload no longer carries #[allow(dead_code)] (it has a
# real consumer now), while RESPONSE_MARKER / REPLY_READ_TIMEOUT_MS still do.
grep -nB1 "fn build_typed_payload\|RESPONSE_MARKER\|REPLY_READ_TIMEOUT_MS" src/core.rs
# Expected: build_typed_payload has NO #[allow(dead_code)] above it; the other
# two still do (their consumers are P1.M1.T3).

# Confirm zero dead-code warnings overall (the 5 command constants stay
# allow-free from S1; build_typed_payload is now allow-free from S2).
cargo build 2>&1 | grep -iE "never used|warning" || echo "zero dead-code warnings (good)"
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1 passed: `cargo build` → zero warnings (no "never used" for
      `build_typed_payload` — its allow is removed because `run()` calls it).
- [ ] Level 1 passed: `cargo clippy --lib` → zero new warnings.
- [ ] Level 1 passed: `cargo fmt --check` → exit 0.
- [ ] Level 2 passed: `cargo test --lib` → all pass, 0 failed (4 new dispatch +
      existing incl. S1's 7 `build_typed_payload_*` + the `test_run_with_*` set).
- [ ] The pre-existing `test_run_with_*` tests still pass (no regression from the
      `match &params.command` borrow switch on the SendMessage/ListDevices arms).

### Feature Validation

- [ ] `run()` contains NO `todo!()` (`grep -n "todo!" src/lib.rs` → empty).
- [ ] The collapsed typed arm calls `core::build_typed_payload(&params.command)` then
      `send_raw_report(&payload, params.vendor_id, params.product_id,
      params.usage_page, params.usage, params.verbose)?` and returns
      `Ok(CommandResponse::Timeout)`.
- [ ] `match` is `match &params.command` (borrow), enabling `&params.command` in
      the typed arm without reconstruction.
- [ ] `run()`'s `///` doc describes the typed dispatch (not "stubbed with
      todo!()"); `P1.M3` → `P1.M1` numbering fixed.
- [ ] `#[allow(dead_code)]` removed from `build_typed_payload`; its trailing doc
      sentence rewritten. `RESPONSE_MARKER`/`REPLY_READ_TIMEOUT_MS` keeps theirs.
- [ ] 4 dispatch tests added; bogus VID/PID ⇒ deterministic `DeviceNotFound`.
- [ ] `send_raw_report`/`burst_to_one`/`batches_for`/`build_typed_payload` body/
      `RunCommand`/`HostOs`/`CommandResponse`/`error.rs`/`main.rs`/`Cargo.toml`
      unchanged except the documented `build_typed_payload` allow-removal + doc.
- [ ] Only `src/lib.rs` and `src/core.rs` modified.

### Code Quality Validation

- [ ] The collapsed or-pattern arm mirrors the existing arm style (no new
      pattern; `?` propagation matches the SendMessage arm).
- [ ] New tests follow the block's existing style (`use super::*;`, snake_case,
      `matches!` assertion — consistent with `test_run_with_*`).
- [ ] The placeholder is HONEST (`Timeout`, not a fake parsed value) and
      documented in both the arm comment and `run()`'s rustdoc.
- [ ] No new Cargo.toml deps; no new types; no main.rs change.

### Documentation & Deployment

- [ ] `run()`'s rustdoc updated (Mode A) to cover the typed dispatch + the
      Timeout-placeholder caveat + the P1.M1.T3 forward-ref.
- [ ] `build_typed_payload`'s trailing doc sentence updated to name its real
      consumer (`run()`) and drop the stale "test-only / remove in S2" prose.
- [ ] No new environment variables or config.

---

## Anti-Patterns to Avoid

- ❌ Don't keep `match params.command` (move) — the typed arm needs
  `&params.command` for `build_typed_payload`, which a move-match forbids.
  Switch to `match &params.command`. (Leaving it move causes E0500 "cannot move
  out of `params.command`".)
- ❌ Don't reconstruct the typed `RunCommand` from the bound pieces (e.g.
  `RunCommand::ApplyHostContext{ layer, callbacks, clear_board }`) just to pass
  it to the builder — that's a symptom of forgetting the borrow-match. With
  `match &params.command`, `&params.command` is directly available.
- ❌ Don't call `build_typed_payload(…)` BARE from `run()` — it is `pub(crate)`
  in the private `mod core` and is NOT in the `pub use core::{ … }` re-export at
  the crate root (only `send_raw_report`/`list_hid_devices`/… are). A bare call
  fails with E0425 "cannot find function". Call it as
  `core::build_typed_payload(&params.command)`. Do NOT "fix" this by making the
  fn `pub` or adding a re-export — qualifying the call is the intended minimal
  change. (Verified: no `use core::build_typed_payload;` import exists.)
- ❌ Don't append ETX (`0x03`) or prepend `0x81 0x9F` in the typed arm —
  `build_typed_payload` ALREADY returns the ETX-terminated payload starting with
  `0xF0`. `burst_to_one` prepends `[0x00,0x81,0x9F]` per report. Double-framing
  corrupts the wire layout (the firmware misreads the command).
- ❌ Don't write 4 separate typed arms with duplicated build+send bodies — they
  are identical at this stage. Collapse into one or-pattern arm. Per-variant
  divergence (reply parsing) is P1.M1.T3.S2's job; it splits the arm then if
  needed. (YAGNI: don't speculate on divergence that isn't here yet.)
- ❌ Don't return a "parsed" `CommandResponse` (`Ack{ok:true}`, `Info{…}`) from
  the typed arm — we did NOT parse any reply (`burst_to_one` DRAINS it). Return
  the honest `Timeout` placeholder. A fake parsed value would mislead the
  downstream P4 caller (when it lands) into acting on data we never read.
- ❌ Don't remove `#[allow(dead_code)]` from `RESPONSE_MARKER` or
  `REPLY_READ_TIMEOUT_MS` — their consumers are P1.M1.T3 (`parse_reply` + reply
  reader). Remove it from `build_typed_payload` ONLY (its consumer is now `run()`).
- ❌ Don't re-add `#[allow(dead_code)]` to `build_typed_payload` "to be safe" —
  its consumer now exists; leaving the allow is misleading (implies it's dead)
  and S1's doc explicitly says to remove it in S2.
- ❌ Don't use the default VID `0xFEED` in the dispatch tests — a dev box may have
  a real QMK keyboard with that VID, making the test non-deterministic (Ok /
  partial). Use a bogus VID/PID (`Some(0xDEAD)`, `Some(0xBEEF)`) so
  `DeviceNotFound` is guaranteed on any machine.
- ❌ Don't add `#[should_panic]` tests — the WHOLE POINT is that the typed arms
  NO LONGER panic (`todo!()` is gone). Assert the deterministic
  `Err(DeviceNotFound)` instead.
- ❌ Don't change `send_raw_report` / `burst_to_one` / `batches_for` — the typed
  payload feeds the UNCHANGED send path. Reply capture (modifying `burst_to_one`
  to optionally return the first IN report) is P1.M1.T3.S1, not this task.
- ❌ Don't edit `main.rs` to expose typed commands on the CLI — that's P5.M1.
  `main.rs` only constructs `SendMessage`/`ListDevices` today and stays untouched.
- ❌ Don't touch `RunCommand` / `HostOs` / `CommandResponse` / `RunParameters` /
  `error.rs` / `Cargo.toml` — S2 wires existing types; it defines none.
- ❌ Don't leave the stale `P1.M3.T3` numbering in `run()`'s doc — the current
  plan tree is `P1.M1.T3`. Fix it while you're editing the doc (the `todo!()`
  messages that held `P1.M3.T3.S1` are being deleted anyway).

---

**Confidence Score: 10/10** for one-pass implementation success. The deliverable
is a one-token match-kind switch + one collapsed match arm (verbatim FIND/REPLACE)
+ a 3-bullet doc update + a 2-line core.rs allow-removal/doc-tweak + four
ready-to-paste deterministic tests, all against precise anchors in two files, with
the borrow-check reasoning verified (the `SendMessage` arm only auto-derefs
`message`), the S1→S2 dead_code handoff documented by S1 itself, the placeholder
semantics proven safe (no consumer reads it yet), and the test determinism
guaranteed by a bogus VID/PID. The dispatch reuses the unchanged `send_raw_report`
path, so there is zero risk of regressing the legacy string/cache/retry/drain
logic. The two real compile traps are both called out explicitly with their
exact error codes and one-token / one-prefix fixes: (1) the move-vs-borrow
match (E0500 → `match &params.command`); (2) the `core::` qualification of
`build_typed_payload` (E0425 — it is `pub(crate)` but NOT re-exported, so it
must be `core::build_typed_payload(…)`, NOT bare).
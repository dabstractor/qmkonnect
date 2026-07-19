# PRP — P1.M1.T2.S1: Typed-command payload builder + multi-report framing

> **Crate:** `qmk_notifier` (v0.2.1) at `/home/dustin/projects/qmk_notifier`.
> **Repo context:** This subtask is part of the QMKonnect `plan/002` orchestrator
> plan, but ALL edits land in the **`qmk_notifier` crate** (a separate repo that
> QMKonnect pins by git tag per PRD §7). Work in `/home/dustin/projects/qmk_notifier`.
> **File:** `src/core.rs` ONLY.
> **Scope line:** Add ONE pure function `build_typed_payload(&RunCommand) -> Vec<u8>`
> that serializes any typed `RunCommand` into the ETX-terminated wire payload
> `[0xF0][cmd_id][args…][0x03]`, ready to hand straight to the EXISTING
> `send_raw_report` (which already prepends `[0x00,0x81,0x9F]` per report). No
> `run()` change, no new I/O, no `send_raw_report`/`burst_to_one`/`batches_for`
> change. Plus 7 unit tests + the `#[allow(dead_code)]` cleanup the new consumer
> makes correct. Multi-report framing is achieved by REUSING the existing
> chunking path — no new framing code.

---

## Goal

**Feature Goal**: Give the crate a single pure function that turns a typed
`RunCommand` (`QueryInfo` / `QueryCallback` / `SetOs` / `ApplyHostContext`) into
the exact wire payload the firmware's `0xF0`-discriminator protocol expects
(`firmware_wire_contract.md` §Typed-Command Framing + §Command Table), including
the ETX terminator, so a downstream caller can do
`send_raw_report(&build_typed_payload(&cmd), …)` with zero further framing — and
have that payload transparently span multiple 30-byte reports via the
**unchanged** legacy chunking path (`APPLY_HOST_CONTEXT` callback list is
uncapped).

**Deliverable**: `src/core.rs` containing (a) the new `pub(crate) fn
build_typed_payload(cmd: &crate::RunCommand) -> Vec<u8>` with full rustdoc, (b)
the `#[allow(dead_code)]` removed from the 5 command constants it now consumes
(`CMD_DISCRIMINATOR`, `CMD_QUERY_INFO`, `CMD_QUERY_CALLBACK`, `CMD_SET_OS`,
`CMD_APPLY_HOST_CONTEXT`) and added to the fn itself, and (c) 7 new unit tests
inside the existing `#[cfg(test)] mod tests` block. Consumed by the `run()`
typed-dispatch arm in **P1.M1.T2.S2**.

**Success Definition**: `cargo build` compiles with **zero warnings**; `cargo
clippy --lib` introduces none; `cargo fmt --check` exits 0; `cargo test --lib`
passes with all existing tests + the 7 new tests green; `build_typed_payload`
exists with the exact signature, byte layouts, and ETX-termination specified
below; the existing 5 command constants lose their `#[allow(dead_code)]` while
`RESPONSE_MARKER` / `REPLY_READ_TIMEOUT_MS` keep theirs; no file other than
`src/core.rs` is modified; `send_raw_report`, `burst_to_one`, `batches_for`,
`run()`, `RunCommand`, `HostOs`, `CommandResponse`, `error.rs`, `main.rs`, and
`Cargo.toml` are all untouched.

## User Persona (if applicable)

**Target User**: The downstream implementer of P1.M1.T2.S2 (the `run()` typed
dispatch) — the ONE caller — and ultimately the QMKonnect handshake/host-context
pipeline (P4).

**Use Case**: `run()` matches a typed `RunCommand`, calls
`send_raw_report(&build_typed_payload(&cmd), vid, pid, page, usage, verbose)`,
then (P1.M1.T3) reads + parses the reply. Today (this subtask) the crate can
*build* every typed payload; tomorrow (S2) it *sends* them.

**User Journey**: `RunCommand::SetOs(HostOs::Windows)` → `build_typed_payload` →
`[0xF0, 0x03, 0x02, 0x03]` → `send_raw_report` → per-report buffer
`[0x00, 0x81, 0x9F, 0xF0, 0x03, 0x02, 0x03, 0x00…]` → firmware sees
`data[2]==0xF0` ⇒ typed ⇒ `data[3]==0x03` (SET_OS) ⇒ applies `os_byte=2`.

**Pain Points Addressed**: Removes hand-rolled byte-array construction from the
dispatch site; guarantees the `0x81,0x9F` header is NOT double-prepended (a real
trap — see Framing subtlety); makes the multi-report chunking of large
`APPLY_HOST_CONTEXT` callback lists a property of the *existing* send path
rather than bespoke code.

## Why

- This is the **pure-framing layer** of the M1.T2 "Typed-Command Framing" task —
  the smallest, I/O-free building block between the type surface (S1/S2, done)
  and the transport wiring (S2). Defining the builder before the dispatch keeps
  the dependency chain clean (types → pure builder → dispatch → reply reader).
- It is **purely additive to `core.rs`** — no behavior change to the send path,
  no `run()` change, no new deps. The only "changes" to existing items are
  removing now-unnecessary `#[allow(dead_code)]` attributes (the builder is the
  real consumer the constants' own comment anticipated) and updating that
  comment to match reality.
- It **locks the wire format** against the canonical contract so the firmware
  (P1.M2) and the host agree byte-for-byte; drift here silently breaks
  interop, so the exact payloads are asserted in 7 tests.

## What

### The new `build_typed_payload` function — placed in `src/core.rs`, AFTER `batches_for`, BEFORE `MatchKey`

```rust
/// Build the ETX-terminated wire payload for a typed [`crate::RunCommand`],
/// ready to hand unchanged to [`send_raw_report`].
///
/// Returns `[0xF0][cmd_id][args…][0x03]` — the typed discriminator (`0xF0`),
/// the command-ID byte, command-specific argument bytes, then the `0x03` ETX
/// terminator. [`send_raw_report`] / [`burst_to_one`] prepend the per-report
/// `[0x00][0x81][0x9F]` framing, so the firmware-side layout is
/// `[0x81][0x9F][0xF0][cmd_id][args…][0x03]` — the discriminator lands at
/// firmware `data[2]` (PRD §8; canonical layout in
/// `plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md` §Typed-Command
/// Framing). The returned `Vec` therefore starts with `0xF0`, NOT with
/// `0x81 0x9F` (that header is added by [`burst_to_one`]).
///
/// The payload is chunked by the SAME multi-report path as legacy strings
/// ([`batches_for`] / [`burst_to_one`], 30 payload bytes/report), so
/// `APPLY_HOST_CONTEXT` may span reports — its callback-id list is uncapped.
/// Because the ETX is appended here, the caller passes the `Vec` directly to
/// [`send_raw_report`] with no further framing.
///
/// Per-variant arg layouts (`firmware_wire_contract.md` §Command Table):
/// - [`crate::RunCommand::QueryInfo`]        (id `0x01`): no args.
/// - [`crate::RunCommand::QueryCallback`]    (id `0x02`): `[index]`.
/// - [`crate::RunCommand::SetOs`]            (id `0x03`): `[os_byte]` where
///   `os_byte = HostOs as u8` (mirrors QMK `os_variant_t`).
/// - [`crate::RunCommand::ApplyHostContext`] (id `0x05`): `[layer][flags][count][id…]`
///   — `layer` is the host-layer number or `0xFF` (clear) when `layer == None`;
///   `flags` bit 0 is `clear_board`; `count` is the callback-id count; `id…`
///   the full desired enabled set (firmware diffs, disable-before-enable).
///
/// Non-typed variants ([`crate::RunCommand::SendMessage`],
/// [`crate::RunCommand::ListDevices`]) are NOT typed commands and return an
/// empty `Vec`; the `run()` dispatch routes them through their own paths
/// (legacy string + ETX; `list_hid_devices`) and never reaches the typed send.
///
/// Consumer: the `run()` typed-dispatch arm (P1.M1.T2.S2). Until then this is
/// referenced only by tests, hence `#[allow(dead_code)]` — remove it in S2
/// once `run()` calls this.
#[allow(dead_code)]
pub(crate) fn build_typed_payload(cmd: &crate::RunCommand) -> Vec<u8> {
    use crate::RunCommand;

    let mut payload = Vec::new();
    payload.push(CMD_DISCRIMINATOR);

    match cmd {
        RunCommand::QueryInfo => {
            payload.push(CMD_QUERY_INFO);
        }
        RunCommand::QueryCallback(index) => {
            payload.push(CMD_QUERY_CALLBACK);
            payload.push(*index);
        }
        RunCommand::SetOs(os) => {
            payload.push(CMD_SET_OS);
            payload.push(*os as u8);
        }
        RunCommand::ApplyHostContext {
            layer,
            callbacks,
            clear_board,
        } => {
            payload.push(CMD_APPLY_HOST_CONTEXT);
            // layer: Some(n) ⇒ host-layer number (≥224 by convention);
            // None ⇒ 0xFF (clear host layer). See firmware_wire_contract.md.
            payload.push(layer.unwrap_or(0xFF));
            // flags: bit 0 = clear_board (firmware clears board layer/command
            // before applying host context). No other bits defined yet.
            payload.push(if *clear_board { 0x01 } else { 0x00 });
            // count: u8. Host invariant — callbacks.len() ≤ 255 (the firmware
            // callback registry is itself u8-bounded, so unreachable in
            // practice). `as u8` truncates if ever violated; validate upstream.
            payload.push(callbacks.len() as u8);
            payload.extend_from_slice(callbacks);
        }
        // Not typed commands — caller routes these elsewhere. Return empty so a
        // misuse is inert rather than a panic (a panic would wedged a misrouted
        // SendMessage in a live run() path).
        RunCommand::SendMessage(_) | RunCommand::ListDevices => return Vec::new(),
    }

    payload.push(0x03); // ETX terminator (signals end-of-message before chunking)
    payload
}
```

### The `#[allow(dead_code)]` cleanup on the command constants

Remove `#[allow(dead_code)]` from exactly these 5 (the builder now references
them in compiled, non-`cfg(test)` code):

```rust
/// Typed-command discriminator: first payload byte after 0x81 0x9F (PRD §10.1).
pub(crate) const CMD_DISCRIMINATOR: u8 = 0xF0;
// ...
pub(crate) const CMD_QUERY_INFO: u8 = 0x01;
pub(crate) const CMD_QUERY_CALLBACK: u8 = 0x02;
pub(crate) const CMD_SET_OS: u8 = 0x03;
pub(crate) const CMD_APPLY_HOST_CONTEXT: u8 = 0x05;
```

**KEEP** `#[allow(dead_code)]` on `RESPONSE_MARKER` and `REPLY_READ_TIMEOUT_MS`
(their consumers land in P1.M1.T3: `parse_reply` + the reply reader). Update the
constants' header comment so it no longer claims all of them are
allow-dead-awaiting-consumer — see Task 2 for the exact replacement text.

> **Why this is safe (empirically verified):** a `pub(crate) const` referenced
> only by a `#[allow(dead_code)]`-unused `pub(crate)` fn does **not** warn in
> `cargo build` — the fn's body is compiled code, so the reference counts.
> (A `#[cfg(test)]`-only reference would *not* count, which is why the constants
> needed the allows before the builder existed.) Verified on rustc 1.92.0.

### The 7 unit tests — inside the existing `#[cfg(test)] mod tests` block in `core.rs`

```rust
#[test]
fn build_typed_payload_query_info() {
    // QUERY_INFO (0x01): no args. Full payload = discriminator + cmd + ETX.
    let payload = build_typed_payload(&RunCommand::QueryInfo);
    assert_eq!(payload, vec![CMD_DISCRIMINATOR, CMD_QUERY_INFO, 0x03]);
    // Invariants every typed payload must satisfy (also asserted by exact-eq):
    assert_eq!(*payload.first().unwrap(), CMD_DISCRIMINATOR, "must start with 0xF0");
    assert_eq!(*payload.last().unwrap(), 0x03, "must end with ETX");
}

#[test]
fn build_typed_payload_query_callback() {
    // QUERY_CALLBACK (0x02): one arg = the registry index.
    let payload = build_typed_payload(&RunCommand::QueryCallback(7));
    assert_eq!(payload, vec![CMD_DISCRIMINATOR, CMD_QUERY_CALLBACK, 7, 0x03]);

    // Boundary: index 0 and 255 must still serialize (u8 range, no truncation).
    assert_eq!(
        build_typed_payload(&RunCommand::QueryCallback(0)),
        vec![CMD_DISCRIMINATOR, CMD_QUERY_CALLBACK, 0, 0x03]
    );
    assert_eq!(
        build_typed_payload(&RunCommand::QueryCallback(u8::MAX)),
        vec![CMD_DISCRIMINATOR, CMD_QUERY_CALLBACK, u8::MAX, 0x03]
    );
}

#[test]
fn build_typed_payload_set_os() {
    // os_byte mirrors QMK os_variant_t (firmware_wire_contract.md §SET_OS).
    for (os, os_byte) in [
        (HostOs::Unsure, 0u8),
        (HostOs::Linux, 1u8),
        (HostOs::Windows, 2u8),
        (HostOs::Macos, 3u8),
        (HostOs::Ios, 4u8),
    ] {
        let payload = build_typed_payload(&RunCommand::SetOs(os));
        assert_eq!(
            payload,
            vec![CMD_DISCRIMINATOR, CMD_SET_OS, os_byte, 0x03],
            "SET_OS({os:?}) must serialize to [0xF0][0x03][{os_byte}][ETX]"
        );
    }
}

#[test]
fn build_typed_payload_apply_host_context_set_layer() {
    // layer = Some(224) (HOST_LAYER_BASE), 3 callbacks, clear_board set.
    let payload = build_typed_payload(&RunCommand::ApplyHostContext {
        layer: Some(224),
        callbacks: vec![10, 20, 30],
        clear_board: true,
    });
    // [0xF0, 0x05, layer=224, flags=0x01, count=3, 10, 20, 30, ETX]
    assert_eq!(
        payload,
        vec![CMD_DISCRIMINATOR, CMD_APPLY_HOST_CONTEXT, 224, 0x01, 3, 10, 20, 30, 0x03]
    );
}

#[test]
fn build_typed_payload_apply_host_context_clear_layer() {
    // layer = None ⇒ wire byte 0xFF (clear host layer); clear_board false ⇒ flags 0.
    let payload = build_typed_payload(&RunCommand::ApplyHostContext {
        layer: None,
        callbacks: Vec::new(),
        clear_board: false,
    });
    // [0xF0, 0x05, 0xFF, 0x00, count=0, ETX]
    assert_eq!(
        payload,
        vec![CMD_DISCRIMINATOR, CMD_APPLY_HOST_CONTEXT, 0xFF, 0x00, 0, 0x03]
    );
}

#[test]
fn build_typed_payload_non_typed_returns_empty() {
    // SendMessage / ListDevices are NOT typed commands; the run() dispatch
    // routes them elsewhere. The builder returns an empty Vec (inert), never a
    // panic, so a misroute can't wedge a live send path.
    assert_eq!(
        build_typed_payload(&RunCommand::SendMessage("App\x1DTitle".to_string())),
        Vec::new()
    );
    assert_eq!(build_typed_payload(&RunCommand::ListDevices), Vec::new());
}

#[test]
fn build_typed_payload_multi_report_chunking() {
    // A large APPLY_HOST_CONTEXT must span multiple 30-byte reports via the
    // EXISTING batches_for path (no bespoke framing). 40 callback ids ⇒ payload
    // = [0xF0, 0x05, layer, flags, count=40, <40 ids>, ETX] = 46 bytes ⇒ 2 reports.
    let callbacks: Vec<u8> = (0..40u8).collect();
    let payload = build_typed_payload(&RunCommand::ApplyHostContext {
        layer: Some(224),
        callbacks: callbacks.clone(),
        clear_board: false,
    });
    // 5 header bytes (disc+cmd+layer+flags+count) + 40 ids + 1 ETX = 46.
    assert_eq!(payload.len(), 46);
    assert_eq!(*payload.first().unwrap(), CMD_DISCRIMINATOR);
    assert_eq!(*payload.last().unwrap(), 0x03, "ETX must be the final byte");
    // The id bytes are copied verbatim into the payload (after the 5-byte header).
    assert_eq!(&payload[5..5 + callbacks.len()], &callbacks[..]);
    // batches_for (the unchanged chunker): ceil(46 / 30) = 2 reports.
    assert_eq!(batches_for(&payload), 2, "46 payload bytes ⇒ 2 reports");
    // Sanity: a payload that fits one report still chunks to 1.
    assert_eq!(batches_for(&build_typed_payload(&RunCommand::QueryInfo)), 1);
}
```

> The tests reference `RunCommand` / `HostOs` (defined in `src/lib.rs`, the crate
> root). The existing `#[cfg(test)] mod tests` block already opens with
> `use super::*;` — that brings the `core` module's items (incl.
> `build_typed_payload` and the constants) into scope, but **not** `RunCommand`/
> `HostOs`. Add `use crate::{HostOs, RunCommand};` at the top of the `mod tests`
> block (Task 4) — it is test-only and does not affect `cargo build`.

### Success Criteria

- [ ] `pub(crate) fn build_typed_payload(cmd: &crate::RunCommand) -> Vec<u8>` exists
      in `src/core.rs`, placed after `batches_for` and before `MatchKey`.
- [ ] It returns exactly the byte layouts in the table below for each typed
      variant, ending in `0x03` (ETX), starting with `0xF0` (NOT `0x81 0x9F`).
- [ ] `SendMessage` / `ListDevices` return `Vec::new()` (no panic).
- [ ] `#[allow(dead_code)]` removed from `CMD_DISCRIMINATOR`, `CMD_QUERY_INFO`,
      `CMD_QUERY_CALLBACK`, `CMD_SET_OS`, `CMD_APPLY_HOST_CONTEXT`; KEPT on
      `RESPONSE_MARKER` and `REPLY_READ_TIMEOUT_MS`; ADDED to `build_typed_payload`.
- [ ] 7 new `#[test]` fns in the existing `core::tests` block; all pass.
- [ ] `send_raw_report`, `burst_to_one`, `batches_for`, `run()`, `RunCommand`,
      `HostOs`, `CommandResponse` unchanged; no file but `src/core.rs` modified.
- [ ] `cargo build` → zero warnings; `cargo clippy --lib` → no new warnings;
      `cargo fmt --check` → exit 0; `cargo test --lib` → all pass.

**Expected per-variant payloads** (`build_typed_payload` return value):

| Variant | Returned `Vec<u8>` |
| --- | --- |
| `QueryInfo` | `[0xF0, 0x01, 0x03]` |
| `QueryCallback(i)` | `[0xF0, 0x02, i, 0x03]` |
| `SetOs(os)` | `[0xF0, 0x03, os as u8, 0x03]` |
| `ApplyHostContext{layer:Some(n), cb, clr}` | `[0xF0, 0x05, n, flags, count, cb…, 0x03]` |
| `ApplyHostContext{layer:None, …}` | `[0xF0, 0x05, 0xFF, flags, count, cb…, 0x03]` |
| `SendMessage(_)` / `ListDevices` | `[]` (empty) |

where `flags = if clear_board { 0x01 } else { 0x00 }` and `count = callbacks.len() as u8`.

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything
> needed to implement this successfully?"_ — **Yes.** The exact function body
> (verbatim, with rustdoc), the exact 7 tests (verbatim), the precise placement
> anchor (after `batches_for`, before `MatchKey`), the exact constant-allow
> edits, the source-of-truth wire layouts (quoted), the empirically-verified
> dead_code reasoning, and verified build/clippy/fmt/test commands are all
> below. The implementer does not need to read any QMK firmware source —
> `firmware_wire_contract.md` canonicalizes every byte.

### Documentation & References

```yaml
# MUST READ — the canonical wire contract (request byte layouts + framing diagram)
- file: /home/dustin/projects/qmk_notifier/plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md
  why: "Single source of truth for every byte build_typed_payload emits. The
        §Typed-Command Framing ASCII diagram shows the 33-byte hidapi buffer
        [0x00][0x81][0x9F][0xF0][cmd_id][args][0x03] and states the discriminator
        is 'the first payload byte (byte[3] of the 33-byte hidapi buffer)' —
        which is WHY the payload starts with 0xF0 (burst_to_one prepends 0x00
        0x81 0x9F). §Command Table gives each cmd's request args. §Field
        Definitions (SET_OS/APPLY_HOST_CONTEXT requests) give os_byte +
        layer/flags/count/id encodings. §Constants pins every wire value."
  section: "Typed-Command Framing, Multi-report chunking, Command Table, Field
            Definitions (SET_OS request, APPLY_HOST_CONTEXT request), Constants"
  critical: "The payload handed to send_raw_report starts with 0xF0, NOT 0x81
             0x9F — that 2-byte magic header is prepended by burst_to_one per
             report. Double-prepending it (a real trap) puts 0x81 at data[2] and
             the firmware misreads the command as a no-match legacy string."

# MUST READ — the file being edited (read current state before editing)
- file: /home/dustin/projects/qmk_notifier/src/core.rs
  why: "Contains the constants block (lines ~10-40: the 5 command constants +
        RESPONSE_MARKER + REPLY_READ_TIMEOUT_MS, each with a temporary
        #[allow(dead_code)] and a header comment saying 'REMOVE each allow when
        its constant gains a real consumer'); send_raw_report; burst_to_one
        (which hardcodes request_data[1]=0x81 / [2]=0x9F and copies the caller's
        `data` into [3..]); batches_for; and the #[cfg(test)] mod tests block
        (where the 7 tests go). build_typed_payload goes AFTER batches_for,
        BEFORE MatchKey."
  pattern: "Fn style: /// doc + pub(crate) fn + #[allow(dead_code)] while
            unreferenced (mirrors how the constants are staged). Tests use the
            block's existing `use super::*;`."
  gotcha: "Do NOT change send_raw_report/burst_to_one/batches_for — the builder
           produces a Vec that feeds the UNCHANGED send path. Do NOT touch
           run()/RunCommand/HostOs (lib.rs). Do NOT remove #[allow(dead_code)]
           from RESPONSE_MARKER or REPLY_READ_TIMEOUT_MS."

# MUST READ — the type surface this builder consumes (S1 output, already committed)
- file: /home/dustin/projects/qmk_notifier/src/lib.rs
  why: "Defines RunCommand (6 variants) and HostOs (#[repr(u8)], 0-4) — the
        build_typed_payload input. Match exhaustively on &RunCommand; reference
        HostOs only implicitly via `*os as u8` (no need to name the type).
        run() still has 4 todo!() arms (S1 scaffolding) — DO NOT TOUCH; the
        dispatch that calls build_typed_payload is P1.M1.T2.S2."
  section: "pub enum RunCommand, pub enum HostOs, run() (the todo!() arms)"
  critical: "RunCommand lives at the crate root (lib.rs). From core.rs reference
             it as crate::RunCommand (the function signature) and
             `use crate::RunCommand;` inside the fn for the match arms. The
             lib.rs doc comments on SetOs/ApplyHostContext mention a phantom
             `build_command_data` (old numbering) — leave them; do NOT edit
             lib.rs (parallel-edit collision risk with P1.M1.T1.S2)."

# REFERENCE — crate PRD (public API contract + framing prose)
- file: /home/dustin/projects/qmk_notifier/PRD.md
  why: "§10.1 Framing: '[0x81,0x9F,0xF0,cmd, args…]' and 'reuse the same
        ETX-framed, multi-report chunking as strings'. §7 Crate Spec restates
        the RunCommand/CommandResponse API. §14 invariants pin wire values."
  section: "7. Crate Spec, 10.1 Framing, 14. Key Invariants"

# REFERENCE — merged-plan PRD sections (the authoritative cross-repo spec)
- docfile: (orchestrator merged PRD) §7 "Crate Spec (qmk_notifier, Rust)" and
           §5 "Wire Protocol (typed commands)"
  why: "§7: 'Typed variants build [0x81,0x9F,0xF0,cmd, args…] and reuse the same
        ETX-framed, multi-report chunking as strings.' §5 command table +
        response shapes (responses are out of scope here — parse_reply is
        P1.M1.T3)."

# REFERENCE — research notes for this subtask (dead_code proof + framing resolution)
- docfile: /home/dustin/projects/qmkonnect/plan/002_637d65b6e9b8/P1M1T2S1/research/notes.md
  why: "Documents the two non-obvious load-bearing facts: (1) the payload starts
        with 0xF0 because burst_to_one prepends 0x81 0x9F; (2) the empirical
        dead_code test proving the 5 command constants' #[allow(dead_code)] can
        be removed once build_typed_payload (compiled code) references them,
        while RESPONSE_MARKER/REPLY_READ_TIMEOUT_MS keep theirs. Also records
        the per-variant payload table and the placement decision."
```

### Current Codebase tree (run from the crate root `/home/dustin/projects/qmk_notifier`)

```bash
qmk_notifier/
├── Cargo.toml          # name="qmk_notifier", version="0.2.1", edition="2021"
│                       # deps: clap, hidapi, (toml/dirs/serde unused legacy — DO NOT TOUCH)
├── Cargo.lock
├── README.md
├── PRD.md              # crate PRD (§7 Crate Spec, §10.1 Framing, §14 invariants)
├── .gitignore          # contains only: /target
├── plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md   # WIRE SOURCE OF TRUTH
└── src
    ├── main.rs         # binary entrypoint (thin wrapper) — DO NOT TOUCH
    ├── error.rs        # QmkError enum — DO NOT TOUCH
    ├── lib.rs          # RunCommand(S1), HostOs(S1), CommandResponse(S2), RunParameters,
    │                   #   parse_cli_args, run (4 todo!() arms), mod tests — DO NOT TOUCH
    └── core.rs         # <-- FILE TO EDIT (ONLY): constants, send_raw_report,
                        #     burst_to_one, batches_for, MatchKey/DeviceCache, mod tests
```

### Desired Codebase tree with files to be modified

```bash
src/
└── core.rs   # MODIFIED ONLY: add build_typed_payload (after batches_for),
              #   remove #[allow(dead_code)] from 5 command constants,
              #   update the constants header comment, add 7 tests + a test-only
              #   `use crate::{HostOs, RunCommand};` in mod tests.
# (no new files; no other file touched)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL: the payload handed to send_raw_report starts with 0xF0, NOT 0x81 0x9F.
//   burst_to_one hardcodes request_data[1]=0x81 / [2]=0x9F and copies the
//   caller's `data` into request_data[3..]. So the caller's payload IS the bytes
//   AFTER [0x81,0x9F]. For a typed command that means the first payload byte is
//   the discriminator 0xF0 (firmware data[2]). Prepending 0x81 0x9F yourself
//   double-adds the header and the firmware misreads the command.

// CRITICAL: keep #[allow(dead_code)] on build_typed_payload itself until P1.M1.T2.S2.
//   Its only consumer is the run() typed-dispatch arm (S2). Until then it is
//   referenced only by tests, and a cfg(test)-only reference does NOT silence
//   dead_code in `cargo build`. Remove the allow in S2 once run() calls it.

// CRITICAL: remove #[allow(dead_code)] from exactly the 5 constants the builder
//   references (CMD_DISCRIMINATOR, CMD_QUERY_INFO, CMD_QUERY_CALLBACK, CMD_SET_OS,
//   CMD_APPLY_HOST_CONTEXT). KEEP it on RESPONSE_MARKER + REPLY_READ_TIMEOUT_MS
//   (consumers in P1.M1.T3). Verified: a const referenced by compiled (non-cfg-test)
//   code — even an allow-dead fn's body — does NOT warn.

// CRITICAL: do NOT change send_raw_report / burst_to_one / batches_for.
//   The builder produces a Vec<u8> that feeds the UNCHANGED send path. Multi-report
//   chunking of >30-byte payloads (large APPLY_HOST_CONTEXT) is automatic via
//   batches_for. Forking the send path (the item's rejected alternative) would
//   risk regressing the legacy string path + the drain/retry logic.

// CRITICAL: build_typed_payload's match must be EXHAUSTIVE over all 6 RunCommand
//   variants. SendMessage/ListDevices are NOT typed — return Vec::new() (inert,
//   not a panic) so a future misroute in run() can't wedge a live send.

// GOTCHA: RunCommand/HostOs live at the crate root (src/lib.rs). From core.rs,
//   the signature uses `&crate::RunCommand`; inside the fn do
//   `use crate::RunCommand;` for the match arms (and reference HostOs only via
//   `*os as u8`, which needs no import). Do NOT add a module-level `use` — keep
//   it function-local to avoid touching unrelated import groups.

// GOTCHA: the tests need RunCommand + HostOs in scope. The existing mod tests has
//   `use super::*;` (brings core's items) but NOT crate-root types. Add
//   `use crate::{HostOs, RunCommand};` at the top of the mod tests block.

// GOTCHA: callbacks.len() as u8 truncates if > 255. The firmware callback
//   registry is itself u8-bounded (callback_count is a u8 reply field), so this
//   is unreachable in practice — but it is a host invariant. Document it; do not
//   add a runtime check here (validation belongs upstream in the rules layer).

// NOTE: lib.rs RunCommand::SetOs/ApplyHostContext doc comments reference a phantom
//   `build_command_data` (old task numbering). Leave them — editing lib.rs risks a
//   parallel-edit collision with P1.M1.T1.S2 (also in lib.rs). Reconcile in a later
//   doc-pass task. They are harmless prose, not compiled.

// NOTE: toml/dirs/serde in Cargo.toml are unused legacy deps. Do NOT wire the
//   builder to serde. build_typed_payload is plain Vec<u8> byte-packing, no deps.
```

## Implementation Blueprint

### Data models and structure

No new types. `build_typed_payload` is a pure `&RunCommand -> Vec<u8>` transform.
The only "structure" is the byte layout, fixed by the wire contract (see the
Expected-per-variant-payloads table in "What").

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: READ the current state of src/core.rs (anchors + the constant block)
  - READ: /home/dustin/projects/qmk_notifier/src/core.rs in full.
  - LOCATE: (a) the constants block (CMD_DISCRIMINATOR … REPLY_READ_TIMEOUT_MS),
          each with its #[allow(dead_code)] + the header comment; (b) the
          `fn batches_for` closing `}`; (c) the `struct MatchKey` definition;
          (d) the `#[cfg(test)] mod tests { use super::*; ... }` block.
  - CONFIRM: batches_for formula = (data.len() + REPORT_LENGTH - 3) /
          PAYLOAD_PER_REPORT; PAYLOAD_PER_REPORT = REPORT_LENGTH - 2 = 30;
          burst_to_one sets request_data[1]=0x81, [2]=0x9F and copies `data` into
          [3..]. (This is WHY the payload starts with 0xF0.)
  - GOAL: know the exact insertion anchors so edits are surgical.

Task 2: REMOVE #[allow(dead_code)] from the 5 command constants + update the header comment
  - EDIT: delete the `#[allow(dead_code)]` line ABOVE each of CMD_DISCRIMINATOR,
          CMD_QUERY_INFO, CMD_QUERY_CALLBACK, CMD_SET_OS, CMD_APPLY_HOST_CONTEXT.
  - KEEP: the `#[allow(dead_code)]` on RESPONSE_MARKER and REPLY_READ_TIMEOUT_MS.
  - UPDATE the constants header comment block: replace the sentence claiming ALL
          constants carry a temporary allow + "REMOVE each allow when its
          constant gains a real consumer" with text stating that the 5 command
          constants now have a real consumer (build_typed_payload, this subtask)
          and only RESPONSE_MARKER + REPLY_READ_TIMEOUT_MS remain allow-dead
          (consumers land in P1.M1.T3: parse_reply + the reply reader).
  - VERIFY-AS-YOU-GO: after Task 3 lands the builder, `cargo build` must show
          ZERO warnings (if any of the 5 still warns, the builder isn't
          referencing it — fix the builder, do NOT re-add the allow).

Task 3: ADD build_typed_payload after batches_for, before MatchKey
  - INSERT: the full `#[allow(dead_code)] pub(crate) fn build_typed_payload(...)`
          block (see the "What" section — paste verbatim, rustdoc included).
  - PLACEMENT: immediately after `batches_for`'s closing `}` (one blank line),
          before the `/// Match parameters …` doc on `struct MatchKey`.
  - SIGNATURE: exactly `pub(crate) fn build_typed_payload(cmd: &crate::RunCommand)
          -> Vec<u8>`. Function-local `use crate::RunCommand;` for match arms.
  - BODY: push CMD_DISCRIMINATOR first; match the 6 variants (4 typed arms build
          cmd_id+args; SendMessage|ListDevices ⇒ `return Vec::new()`); push 0x03
          ETX at the end; return payload.
  - NAMING: snake_case fn; no new types.

Task 4: ADD the test-only import + the 7 tests to the existing mod tests block
  - ADD at the top of `#[cfg(test)] mod tests`: `use crate::{HostOs, RunCommand};`
          (the existing `use super::*;` stays; this only adds crate-root types).
  - ADD the 7 #[test] fns (see the "What" section — paste verbatim):
          build_typed_payload_query_info, build_typed_payload_query_callback,
          build_typed_payload_set_os, build_typed_payload_apply_host_context_set_layer,
          build_typed_payload_apply_host_context_clear_layer,
          build_typed_payload_non_typed_returns_empty,
          build_typed_payload_multi_report_chunking.
  - PATTERN: reuse the block's `use super::*;` for build_typed_payload +
          the constants + batches_for. snake_case test names.
  - DO NOT: call run() (it still has todo!() arms for typed cmds — would panic).
          DO NOT add #[should_panic].

Task 5: VALIDATE (do not skip)
  - RUN (from /home/dustin/projects/qmk_notifier):
          cargo fmt && cargo build && cargo clippy --lib &&
          cargo fmt --check && cargo test --lib
  - EXPECT: build 0 warnings; clippy no new warnings; fmt --check exit 0; all
          tests pass (existing + 7 new).
  - IF warning "constant `X` is never used": you removed an allow from a constant
          the builder doesn't reference — either the builder is missing the
          reference (fix it) OR it's RESPONSE_MARKER/REPLY_READ_TIMEOUT_MS (re-add
          the allow — those must keep it).
  - IF E0433 "cannot find type/variant RunCommand in this scope": the test block
          is missing `use crate::{HostOs, RunCommand};` (Task 4) OR the fn-local
          `use crate::RunCommand;` (Task 3) is absent.
  - SANITY: `git diff --stat src/core.rs` shows only core.rs changed.
```

### Implementation Patterns & Key Details

```rust
// === WHY the payload starts with 0xF0 (not 0x81 0x9F) ===
//   burst_to_one (core.rs) builds each report as:
//     request_data = [0u8; 33];
//     request_data[1] = 0x81;          // magic header byte 1
//     request_data[2] = 0x9F;          // magic header byte 2
//     request_data[3..].copy_from_slice(batch_data);   // <- the PAYLOAD
//   So `data` passed to send_raw_report is the payload AFTER [0x81,0x9F]. For a
//   typed command the first payload byte is the discriminator 0xF0, which lands
//   at firmware data[2] (report-ID byte stripped by hidapi). Confirmed by
//   firmware_wire_contract.md §Typed-Command Framing.

// === WHY reuse the existing send path (not a typed-specific one) ===
//   The legacy string path already does: append ETX → batches_for → burst_to_one
//   (prepend [0x00,0x81,0x9F] per report, copy payload, zero-pad tail) → drain.
//   Typed commands have IDENTICAL framing (ETX-terminated, 30-byte chunks), so
//   the SAME path carries them. APPLY_HOST_CONTEXT with >30 bytes of callback ids
//   simply spans reports — no cap, no special code.

// === WHY SendMessage/ListDevices return Vec::new() (not panic) ===
//   Exhaustive match is required (RunCommand is one enum). A panic on a non-typed
//   variant would wedge run() if a future dispatch misrouted a SendMessage here.
//   Returning empty is inert: send_raw_report(&[], …) computes batches_for(&[])
//   == 0 and sends nothing. (The run() dispatch in S2 won't reach this for
//   SendMessage/ListDevices anyway.)

// === WHY `*os as u8` needs no HostOs import ===
//   HostOs is #[repr(u8)], so `os: &HostOs` → `*os as u8` works via coercion;
//   the type name never appears in the builder body. Only RunCommand is named
//   (in the match arms), hence the function-local `use crate::RunCommand;`.

// === Multi-report invariant (asserted in the chunking test) ===
//   40 callback ids ⇒ payload = 5 (disc+cmd+layer+flags+count) + 40 + 1 (ETX)
//   = 46 bytes ⇒ batches_for = ceil(46/30) = 2 reports. The first report carries
//   bytes [0..30] (disc+cmd+layer+flags+count+25 ids), the second [30..46]
//   (15 ids + ETX). The firmware reassembles across reports exactly like a long
//   legacy string. ETX (0x03) is the FINAL byte of the WHOLE payload, so it lands
//   in the last report — correct end-of-message signal.
```

### Integration Points

```yaml
SOURCE FILES:
  - modify (only): "/home/dustin/projects/qmk_notifier/src/core.rs ONLY"
  - add:    "pub(crate) fn build_typed_payload(cmd: &crate::RunCommand) -> Vec<u8>
             (after batches_for, before MatchKey) + full rustdoc"
  - add:    "7 #[test] fns + `use crate::{HostOs, RunCommand};` in mod tests"
  - edit:   "remove #[allow(dead_code)] from the 5 command constants; update the
             constants header comment"

DEPENDENCIES / Cargo.toml:
  - none. No new crate deps. (toml/dirs/serde are unused legacy — do not touch.)

PUBLIC API SURFACE:
  - unchanged at the crate root. build_typed_payload is pub(crate) — internal
    transport helper, NOT re-exported. send_raw_report / burst_to_one /
    batches_for / RunCommand / HostOs / CommandResponse / run() signature all
    unchanged.

PARALLEL-SIBLING CONTRACT (P1.M1.T1.S2, in flight):
  - S2 adds CommandResponse to src/lib.rs. This task edits src/core.rs ONLY, so
    there is NO file-level collision. Do NOT edit lib.rs (would collide with S2).

DOWNSTREAM CONSUMER (do NOT implement now — listed for awareness):
  - P1.M1.T2.S2: "the run() typed-dispatch arm calls
                  send_raw_report(&build_typed_payload(&cmd), vid, pid, page,
                  usage, verbose), then (P1.M1.T3) reads + parses the reply. That
                  arm replaces the 4 todo!() stubs and changes run()'s return type
                  to Result<CommandResponse, QmkError>."

OUT-OF-SCOPE (later subtasks — do NOT implement here):
  - P1.M1.T2.S2: "wire typed commands into run() dispatch + send path (the ONE
                  caller of build_typed_payload)."
  - P1.M1.T3.S1: "parse_reply — decodes a 32-byte IN report into CommandResponse
                  (uses RESPONSE_MARKER, currently allow-dead)."
  - P1.M1.T3.S2: "run() return type Result<(),QmkError> → Result<CommandResponse,_>."
```

## Validation Loop

> All commands run from the crate root: `/home/dustin/projects/qmk_notifier`

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmk_notifier

# Format the edited file (rustfmt default — no rustfmt.toml exists).
cargo fmt

# Build the whole crate — MUST compile with ZERO warnings.
cargo build 2>&1 | tee /tmp/build.log
# Expected: "Finished `dev` profile ..." and NO "warning:" lines.
#   - If "constant `CMD_*` is never used": you removed an allow from a constant
#     the builder doesn't reference — fix the builder or re-add the allow
#     (RESPONSE_MARKER / REPLY_READ_TIMEOUT_MS must KEEP theirs).
#   - If "function `build_typed_payload` is never used": you forgot its
#     #[allow(dead_code)] (its consumer is P1.M1.T2.S2).

# Lint (default clippy — no .clippy.toml exists).
cargo clippy --lib 2>&1 | tee /tmp/clippy.log
# Expected: no warnings/errors specific to build_typed_payload or its tests.
#   clippy may suggest `vec![..]` or match ergonomics — accept sensible fixes,
#   but do NOT change the byte layout or the &RunCommand signature.

# Formatting check (CI-style gate).
cargo fmt --check
# Expected: exit code 0 (no diff). If non-zero, re-run `cargo fmt`.

# Sanity: confirm ONLY core.rs changed.
git diff --stat
# Expected: only src/core.rs listed.
```

### Level 2: Unit Tests (Component Validation)

```bash
cd /home/dustin/projects/qmk_notifier

# Run the 7 new tests in isolation first.
cargo test --lib build_typed_payload_ -- --nocapture
# Expected: 7 passed (query_info, query_callback, set_os,
#   apply_host_context_set_layer, apply_host_context_clear_layer,
#   non_typed_returns_empty, multi_report_chunking).

# Run the full lib test suite (core.rs + lib.rs unit tests).
cargo test --lib
# Expected: "test result: ok. <N> passed; 0 failed; 0 ignored; ...".
# Pre-existing: core.rs (~14 incl. the constants/batches_for tests) + lib.rs
# (S1's 4 + S2's 3 + greenfield ~6). The exact N is not load-bearing; the gate
# is 0 failed.

# Sanity: the pre-existing tests still pass (no regressions from the allow edits).
cargo test --lib batches_for_ -- --nocapture          # batches_for math unchanged
cargo test --lib typed_command_constants_match_ -- --nocapture   # constant values unchanged
cargo test --lib test_run_with_ -- --nocapture        # run() still returns ()/todo!() OK
```

### Level 3: Integration Testing (System Validation)

```text
NOT APPLICABLE for this subtask.
build_typed_payload is a PURE function — no HID I/O, no run() wiring. There is no
live-hardware or CLI path to exercise until the run() dispatch (P1.M1.T2.S2) and
the reply reader/parser (P1.M1.T3) land. The exact-byte + multi-report-chunking
unit tests in Level 2 ARE the end-to-end verification for this task (they assert
the precise wire layout against the canonical contract AND prove batches_for
chunks a >30-byte payload into ≥2 reports).
```

### Level 4: Creative & Domain-Specific Validation

```bash
cd /home/dustin/projects/qmk_notifier

# Confirm rustdoc renders (Mode A documentation):
cargo doc --lib --no-deps 2>&1 | grep -i "build_typed_payload" || \
  echo "build_typed_payload: documented (or no diagnostics — good)"

# Cross-check: the 5 command constants are now referenced by compiled code, so
# `cargo build` must NOT list them as dead. (RESPONSE_MARKER + REPLY_READ_TIMEOUT_MS
# are still allow-dead — that's expected.)
cargo build 2>&1 | grep -iE "never used|warning" || echo "zero dead-code warnings (good)"

# Optional: pretty-print a payload to eyeball the wire layout.
# (Ad-hoc; not required for the gate. If you add a temporary `#[test]` that
# prints, remove it before committing — keep exactly the 7 specified tests.)
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1 passed: `cargo build` → zero warnings (no "never used" for the 5
      command constants; build_typed_payload's allow suppresses its own).
- [ ] Level 1 passed: `cargo clippy --lib` → zero new warnings.
- [ ] Level 1 passed: `cargo fmt --check` → exit 0.
- [ ] Level 2 passed: `cargo test --lib` → all pass, 0 failed (7 new + existing).
- [ ] The pre-existing `batches_for_*` / `typed_command_constants_*` /
      `test_run_with_*` tests still pass (no regression from the allow edits).

### Feature Validation

- [ ] `pub(crate) fn build_typed_payload(cmd: &crate::RunCommand) -> Vec<u8>`
      present, placed after `batches_for` and before `MatchKey`.
- [ ] Per-variant payloads EXACTLY match the Expected-per-variant-payloads table
      (QueryInfo `[0xF0,0x01,0x03]`; QueryCallback `[0xF0,0x02,i,0x03]`; SetOs
      `[0xF0,0x03,os,0x03]`; ApplyHostContext `[0xF0,0x05,layer,flags,count,ids…,0x03]`
      with layer None⇒0xFF and flags bit0=clear_board).
- [ ] Every typed payload starts with `0xF0` and ends with `0x03` (ETX).
- [ ] SendMessage / ListDevices return `Vec::new()` (no panic).
- [ ] `#[allow(dead_code)]` removed from CMD_DISCRIMINATOR, CMD_QUERY_INFO,
      CMD_QUERY_CALLBACK, CMD_SET_OS, CMD_APPLY_HOST_CONTEXT; KEPT on
      RESPONSE_MARKER + REPLY_READ_TIMEOUT_MS; ADDED to build_typed_payload.
- [ ] `send_raw_report` / `burst_to_one` / `batches_for` / `run()` / `RunCommand`
      / `HostOs` / `CommandResponse` all unchanged.
- [ ] Only `src/core.rs` modified.

### Code Quality Validation

- [ ] Follows the crate's `pub(crate)` + `///` doc + `#[allow(dead_code)]`-while-
      staged convention (mirrors how the constants were staged).
- [ ] New tests follow the block's existing style (`use super::*;`, snake_case).
- [ ] No serde/Display/parse logic (this is byte-packing; parse_reply is P1.M1.T3).
- [ ] No new Cargo.toml deps.
- [ ] No lib.rs edit (avoids parallel-edit collision with P1.M1.T1.S2).

### Documentation & Deployment

- [ ] `build_typed_payload` has a multi-line rustdoc (Mode A) naming the wire
      layout, the per-variant args, the "starts with 0xF0 not 0x81 0x9F" rule,
      the multi-report reuse, and the SendMessage/ListDevices empty-Vec contract.
- [ ] Constants header comment updated to reflect which constants now have a
      consumer and which remain allow-dead.
- [ ] No new environment variables or config.

---

## Anti-Patterns to Avoid

- ❌ Don't make the payload start with `0x81 0x9F` — `burst_to_one` ALREADY
  prepends `[0x00, 0x81, 0x9F]` per report. The payload must start with `0xF0`
  (the discriminator), so it lands at firmware `data[2]`. Double-prepending the
  magic header puts `0x81` at `data[2]` and the firmware walks the bytes as a
  no-match legacy string.
- ❌ Don't change `send_raw_report` / `burst_to_one` / `batches_for` — the
  builder feeds the UNCHANGED send path. The item's "or restructure to build the
  full payload including header" alternative is REJECTED (it forks the send path
  and risks regressing the legacy string + drain/retry logic).
- ❌ Don't append ETX from the caller — `build_typed_payload` returns the
  ETX-terminated payload so P1.M1.T2.S2 can do
  `send_raw_report(&build_typed_payload(&cmd), …)` with no further framing.
  (Contrast: the legacy `SendMessage` path appends ETX inside `run()`; the typed
  path folds it into the builder so the dispatch is one call.)
- ❌ Don't omit the `SendMessage | ListDevices` match arm — `RunCommand` is one
  enum and the match must be exhaustive. Return `Vec::new()` (inert), NOT
  `unreachable!()`/`panic!()` (a future misroute would wedge a live send).
- ❌ Don't keep `#[allow(dead_code)]` on the 5 command constants the builder
  references — they now have a real consumer (the builder's body is compiled
  code). KEEP it only on RESPONSE_MARKER + REPLY_READ_TIMEOUT_MS (P1.M1.T3
  consumers). (Verified: rustc does not warn for a const referenced by an
  allow-dead fn's body.)
- ❌ Don't forget `#[allow(dead_code)]` on `build_typed_payload` itself — until
  P1.M1.T2.S2 wires it into `run()`, it's referenced only by tests, and a
  `cfg(test)`-only reference does NOT silence dead_code in `cargo build`.
- ❌ Don't add a module-level `use crate::RunCommand;` — keep it function-local
  to avoid touching unrelated import groups; the signature uses `&crate::RunCommand`.
- ❌ Don't add a runtime check for `callbacks.len() > 255` — the firmware registry
  is u8-bounded so it's unreachable; the `as u8` truncation is a documented host
  invariant. Validation belongs upstream in the rules layer, not the transport.
- ❌ Don't call `run()` in any test — it still has `todo!()` arms for typed
  commands (would panic). The 7 tests are pure construction + equality +
  `batches_for` checks. Don't add `#[should_panic]`.
- ❌ Don't edit `src/lib.rs` — P1.M1.T1.S2 is concurrently editing it; a collision
  risks breaking both tasks. The phantom `build_command_data` refs in lib.rs docs
  are harmless prose; reconcile in a later doc-pass.
- ❌ Don't reorder the payload bytes to "group the header" — the order is fixed by
  the wire contract: `0xF0, cmd_id, args…, ETX`. The firmware parses positionally.
- ❌ Don't split `build_typed_payload` into per-variant functions — one match in
  one fn is the cleanest consumer for the S2 dispatch and keeps the common
  discriminator+ETX framing in one place.

---

**Confidence Score: 10/10** for one-pass implementation success. The deliverable
is a single fully-specified pure function (verbatim body + rustdoc) and seven
ready-to-paste tests, placed against a precise anchor in one file, with the
source-of-truth wire layouts quoted, the empirically-verified `dead_code`
reasoning for the allow cleanup, the explicit "payload starts with 0xF0 not 0x81
0x9F" guard against the one real trap, and verified build/clippy/fmt/test
commands. The two non-obvious load-bearing facts (framing header ownership;
allow-dead reachability) are both proven rather than assumed, and the function
reuses the existing send path unchanged — so there is no risk of regressing the
legacy string path. The only judgment call (returning `Vec::new()` for
non-typed variants) is the safe/inert choice and is asserted by a test.
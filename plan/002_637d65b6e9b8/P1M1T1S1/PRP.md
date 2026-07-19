# PRP — P1.M1.T1.S1: Add HostOs enum and new RunCommand variants

> **Crate:** `qmk_notifier` (v0.2.1) at `/home/dustin/projects/qmk_notifier`
> **Repo context:** This task is part of the QMKonnect plan/002 orchestrator plan,
> but ALL edits land in the **`qmk_notifier` crate** (a separate repo that
> QMKonnet pins by git tag per PRD §7). Work in `/home/dustin/projects/qmk_notifier`.
> **Merged-scope note:** plan/002 merged old plan/001's S1 (HostOs only) + S2
> (RunCommand variants) into THIS single subtask. `CommandResponse` is the
> NEXT sibling subtask (P1.M1.T1.S2) and is **out of scope** here.

---

## Goal

**Feature Goal**: Add the typed-command **type surface** to the `qmk_notifier`
crate's public API in one self-contained edit of `src/lib.rs`:

1. A new `pub enum HostOs` (fieldless, `#[repr(u8)]`, explicit discriminants) that
   exactly mirrors QMK's `os_variant_t` (`0=UNSURE, 1=LINUX, 2=WINDOWS, 3=MACOS,
   4=IOS`) — so `HostOs::Windows as u8 == 2`, the exact byte placed into a
   `SET_OS` report (`[0x81][0x9F][0xF0][0x03][os_byte][0x03]`).
2. Four new variants on the existing `pub enum RunCommand` — `QueryInfo`,
   `QueryCallback(u8)`, `SetOs(HostOs)`, `ApplyHostContext { layer, callbacks,
   clear_board }` — carrying the **structured, typed inputs** for the firmware's
   typed commands (cmd_ids `0x01`, `0x02`, `0x03`, `0x05`). These hold typed data,
   NOT pre-serialized wire bytes (serialization is `build_command_data`, P1.M2.T1).
3. The `run()` match restored to exhaustiveness via four temporary `todo!()` arms.
4. Four construction/discriminant unit tests.

Each new variant/type carries a rustdoc `///` comment referencing its cmd_id and
the firmware command table (Mode A documentation; no external doc files changed).

**Deliverable**: `src/lib.rs` (only) containing the new `HostOs` enum, the extended
`RunCommand` enum, the four `todo!()` arms in `run()`, and four `#[test]` fns inside
the existing `#[cfg(test)] mod tests` block. Consumed downstream by the framing
subsystem (P1.M2.T1) and, ultimately, QMKonnect's `Notifier` trait (P4.M1.T1.S1).

**Success Definition**: `cargo build` compiles with **zero warnings** (and
`cargo clippy --lib` introduces none); `cargo fmt --check` exits 0; `cargo test --lib`
passes with **all greenfield tests + 4 new tests**; `HostOs::Windows as u8 == 2` (and
likewise for all five variants, asserted by a test); each new `RunCommand` variant is
constructible and round-trips through `match`; the existing `run()` integration tests
(ListDevices / SendMessage / verbose) still pass (never hit a `todo!()` arm); no file
other than `src/lib.rs` is modified; `RunParameters`, `parse_cli_args`, `core.rs`,
`error.rs`, `main.rs`, and `Cargo.toml` are untouched.

## User Persona (if applicable)

**Target User**: The downstream implementer of the v0.3.0 typed-command transport
(`build_command_data` in P1.M2.T1, `parse_reply` in P1.M2.T2, `run()` dispatch in
P1.M3.T3) and, ultimately, the QMKonnect desktop app that calls this crate's public API.

**Use Case**: Construct a typed command in host code, hand it to `RunParameters`, and
let a later transport layer serialize + frame + send it. Today the API can *express*
every typed command; tomorrow (M2/M3) the transport knows how to *send* it.

**User Journey**: `RunCommand::SetOs(HostOs::Windows)` → `RunParameters::new(...)`
→ (P1.M3.T3) `run()` → (P1.M2.T1) `build_command_data` emits
`[0x81][0x9F][0xF0][0x03][0x02][0x03]` → `send_raw_report` → firmware applies OS.

**Pain Points Addressed**: Removes the "everything is a magic string" limitation; gives
the type system (and the compiler's exhaustiveness check) jurisdiction over the
typed-command surface so wire-encoding bugs are caught at compile time.

## Why

- `RunCommand`/`HostOs` are the **type layer** of the M1 "Type Contracts" milestone —
  the smallest, dependency-free building block of the v0.3.0 typed-command transport
  (PRD §5 Wire Protocol, §7 Crate Spec, §8 Typed-Command Namespace). `HostOs` is the
  input type for `SET_OS` (cmd `0x03`), sent once at connect to declare the host OS.
- Defining types **before** the serializer/dispatcher keeps the dependency chain clean
  (types → pure framing → transport) and lets each later subtask be validated against a
  fixed, compiling type surface.
- It is **purely additive to the type surface** — the only behavior change is the
  temporary `todo!()` scaffolding in `run()` (necessary to keep the match exhaustive;
  see Gotchas), which is sanctioned by the item description and removed in
  P1.M1.T2.S2 / P1.M3.T3.S1.

## What

### (a) The new `HostOs` enum — inserted AFTER the `RunCommand` enum, BEFORE `RunParameters`

```rust
/// Host operating system, mirrors QMK's `os_variant_t`.
/// Sent via SET_OS (cmd 0x03) to declare the host OS at connect.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOs {
    /// `0` — OS not yet detected / unknown. Mirrors QMK `OS_UNSURE`.
    Unsure = 0,
    /// `1` — Linux host. Mirrors QMK `OS_LINUX`.
    Linux = 1,
    /// `2` — Windows host. Mirrors QMK `OS_WINDOWS`.
    Windows = 2,
    /// `3` — macOS host. Mirrors QMK `OS_MACOS`.
    Macos = 3,
    /// `4` — iOS host. Mirrors QMK `OS_IOS`.
    Ios = 4,
}
```

### (b) The extended `RunCommand` enum — full target body (replaces the current 2-variant enum)

```rust
/// Command types for the QMK notifier.
///
/// `SendMessage`/`ListDevices` are the legacy path. The typed variants carry the
/// host-side-rules typed-command protocol (firmware PRD §4.6 command table; crate
/// PRD §7; canonical wire layout in `firmware_wire_contract.md` §Command Table).
#[derive(Debug, Clone)]
pub enum RunCommand {
    /// Legacy path: send the `"{class}\x1D{title}"` window string (this crate
    /// appends the `0x03` ETX terminator before framing). Not a typed command.
    SendMessage(String),
    /// List all HID devices visible to hidapi (no keyboard I/O).
    ListDevices,

    /// Typed command `0x01` — `QUERY_INFO`. No request args. Replies with
    /// `[0x51][0x01][proto_ver][feature_flags][callback_count][board_rules_present]`.
    /// See firmware PRD §4.6 and `firmware_wire_contract.md` §Command Table.
    QueryInfo,
    /// Typed command `0x02` — `QUERY_CALLBACK`. `index` is the firmware callback
    /// registry slot to read. Replies with `[0x51][0x02][index][name, NUL-padded]`.
    /// See firmware PRD §4.6 and `firmware_wire_contract.md` §Command Table.
    QueryCallback(u8),
    /// Typed command `0x03` — `SET_OS`. Declares the host OS to the keyboard at
    /// connect time. Serialized as `[0xF0][0x03][os_byte][0x03]` where
    /// `os_byte = HostOs::X as u8` (build_command_data, P1.M2.T1).
    /// See firmware PRD §4.6 and `firmware_wire_contract.md` §SET_OS request.
    SetOs(HostOs),
    /// Typed command `0x05` — `APPLY_HOST_CONTEXT`. Pushes the host's desired
    /// layer + enabled-callback set + clear-board flag to the firmware in one
    /// atomic command. Serialized as
    /// `[0xF0][0x05][layer][flags][count][id0][id1]…[0x03]` (build_command_data,
    /// P1.M2.T1).
    ///
    /// - `layer: Option<u8>` — `None` ⇒ wire byte `0xFF` (clear host layer);
    ///   `Some(n)` ⇒ host-layer number (`>= 224` by convention, `HOST_LAYER_BASE`).
    /// - `callbacks: Vec<u8>` — the FULL desired enabled callback-id set; the
    ///   firmware diffs this against the current set (disable-before-enable).
    ///   Uncapped; may span multiple reports.
    /// - `clear_board: bool` — `true` ⇒ set firmware `flags` bit 0
    ///   (`clear_board`): firmware clears the board layer/command before applying.
    ///
    /// See firmware PRD §4.6 and `firmware_wire_contract.md` §APPLY_HOST_CONTEXT request.
    ApplyHostContext {
        layer: Option<u8>,
        callbacks: Vec<u8>,
        clear_board: bool,
    },
}
```

> The greenfield enum doc comment is the single line `/// Command types for the QMK notifier`.
> The target **expands** it to the multi-line doc above (mentioning typed variants).
> `#[derive(Debug, Clone)]` is **unchanged** — do NOT add `PartialEq`/`Eq`/`Copy`
> (`RunCommand` owns a `String`, so `Copy` is impossible).

### (c) `run()` — add four `todo!()` arms to restore match exhaustiveness

```rust
pub fn run(params: RunParameters) -> Result<(), QmkError> {
    match params.command {
        RunCommand::ListDevices => list_hid_devices(),
        RunCommand::SendMessage(message) => {
            /* EXISTING BODY — UNCHANGED (verbose print, ETX append, send_raw_report) */
        }

        // --- Typed-command stubs. Dispatch + reply handling land in P1.M3.T3.S1;
        // run()'s return type changes to `Result<CommandResponse, QmkError>` in
        // P1.M1.T2.S2. `todo!()` keeps this match exhaustive so the crate
        // compiles today. Existing tests only construct ListDevices/SendMessage
        // and never reach these arms. Do NOT wire real logic here. ---
        RunCommand::QueryInfo => todo!("typed dispatch lands in P1.M3.T3.S1"),
        RunCommand::QueryCallback(_) => todo!("typed dispatch lands in P1.M3.T3.S1"),
        RunCommand::SetOs(_) => todo!("typed dispatch lands in P1.M3.T3.S1"),
        RunCommand::ApplyHostContext { .. } => {
            todo!("typed dispatch lands in P1.M3.T3.S1")
        }
    }
}
```

> Do **NOT** change `run()`'s return type (`Result<(), QmkError>`). The
> `CommandResponse` return type is P1.M1.T2.S2, and `CommandResponse` itself is
> P1.M1.T1.S2 (the next sibling subtask — out of scope here). `todo!()` returns
> `!`, which coerces to `Result<(), QmkError>`, so the current signature stays valid.

### (d) Four unit tests — inside the existing `#[cfg(test)] mod tests` block

```rust
#[test]
fn test_host_os_discriminants_match_firmware_contract() {
    // Mirrors QMK os_variant_t and the SET_OS `os_byte` table in
    // plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md (firmware PRD §4.6).
    assert_eq!(HostOs::Unsure as u8, 0);
    assert_eq!(HostOs::Linux as u8, 1);
    assert_eq!(HostOs::Windows as u8, 2);
    assert_eq!(HostOs::Macos as u8, 3);
    assert_eq!(HostOs::Ios as u8, 4);
}

#[test]
fn test_run_command_query_variants_construction() {
    // QueryInfo: unit variant — construct + match.
    let q = RunCommand::QueryInfo;
    assert!(matches!(q, RunCommand::QueryInfo));

    // QueryCallback(index): the u8 is the firmware callback-registry slot.
    let c = RunCommand::QueryCallback(5);
    match c {
        RunCommand::QueryCallback(index) => assert_eq!(index, 5),
        _ => panic!("expected QueryCallback"),
    }
}

#[test]
fn test_run_command_set_os_variant_construction() {
    // SetOs(HostOs): HostOs carries the os_byte source (verified separately by
    // test_host_os_discriminants_match_firmware_contract). Here we confirm the
    // payload round-trips through the variant.
    let s = RunCommand::SetOs(HostOs::Windows);
    match s {
        RunCommand::SetOs(os) => assert_eq!(os, HostOs::Windows),
        _ => panic!("expected SetOs"),
    }
}

#[test]
fn test_run_command_apply_host_context_construction() {
    // layer == None ⇒ clear-host-layer path (wire byte 0xFF).
    let clear = RunCommand::ApplyHostContext {
        layer: None,
        callbacks: vec![1, 2, 3],
        clear_board: true,
    };
    match clear {
        RunCommand::ApplyHostContext { layer, callbacks, clear_board } => {
            assert_eq!(layer, None, "None must mean clear-host-layer (0xFF)");
            assert_eq!(callbacks, vec![1, 2, 3]);
            assert!(clear_board, "clear_board flag must round-trip");
        }
        _ => panic!("expected ApplyHostContext"),
    }

    // layer == Some(n) ⇒ host-layer number (>= 224 by convention).
    let set = RunCommand::ApplyHostContext {
        layer: Some(224), // HOST_LAYER_BASE
        callbacks: Vec::new(),
        clear_board: false,
    };
    match set {
        RunCommand::ApplyHostContext { layer, callbacks, clear_board } => {
            assert_eq!(layer, Some(224));
            assert!(callbacks.is_empty());
            assert!(!clear_board);
        }
        _ => panic!("expected ApplyHostContext"),
    }
}
```

### Success Criteria

- [ ] `pub enum HostOs` exists in `src/lib.rs` with all five variants and the exact
      discriminants `0..=4`, placed AFTER `RunCommand` and BEFORE `RunParameters`.
- [ ] `HostOs` carries `#[repr(u8)]` AND `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`.
- [ ] `RunCommand` has exactly the 6 variants in order: `SendMessage`, `ListDevices`,
      `QueryInfo`, `QueryCallback(u8)`, `SetOs(HostOs)`, `ApplyHostContext { layer:
      Option<u8>, callbacks: Vec<u8>, clear_board: bool }`. `#[derive(Debug, Clone)]`
      unchanged; no `PartialEq`/`Eq`/`Copy` added.
- [ ] `ApplyHostContext` is an inline **struct** variant with fields in the order
      `layer, callbacks, clear_board` (matches PRD §7 and the wire layout).
- [ ] Each new type/variant has a `///` doc comment naming its cmd_id and referencing
      the firmware PRD §4.6 command table (Mode A).
- [ ] `run()`'s `match params.command` has 4 new `todo!()` arms (one explicit arm per
      new variant; NO `_ =>` wildcard); `run()` signature unchanged
      (`Result<(), QmkError>`).
- [ ] 4 new `#[test]` fns exist in the existing `mod tests`; the 3 existing
      `test_run_with_*` tests still pass (never hit a `todo!()` arm).
- [ ] `cargo build` → zero warnings; `cargo clippy --lib` → no new warnings;
      `cargo fmt --check` → exit 0; `cargo test --lib` → all pass.
- [ ] No file other than `src/lib.rs` is modified. `CommandResponse` is NOT added
      (that is the next subtask, P1.M1.T1.S2).

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed to
> implement this successfully?"_ — **Yes.** The exact target enum bodies, the exact
> `run()` edit (with the non-obvious `todo!()` rationale and the explicit "don't change
> the return type" guard), the exact 4 tests, the precise placement anchors, the
> source-of-truth wire contract, and verified validation commands are all below. The
> implementer does not need to read any QMK firmware source —
> `firmware_wire_contract.md` canonicalizes every cmd_id and argument layout.

### Documentation & References

```yaml
# MUST READ — the canonical wire contract (cmd_id table + per-command layouts + os_byte table)
- file: /home/dustin/projects/qmk_notifier/plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md
  why: "The single source of truth for every byte this subtask's types describe. Defines
        the Command Table (cmd_ids 0x01/0x02/0x03/0x05), the SET_OS os_byte table
        (0=UNSURE..4=IOS), and the APPLY_HOST_CONTEXT field layout
        (layer/flags/count/id…). The variant field types/shapes and the
        ApplyHostContext layer=None⇒0xFF / clear_board⇒flags-bit-0 semantics come
        directly from here."
  section: "Command Table", "SET_OS request", "APPLY_HOST_CONTEXT request", "Constants (from firmware notifier.h)"
  critical: "ApplyHostContext field semantics are DOCUMENTED here in this subtask but
             ENFORCED (translated to wire bytes) in P1.M2.T1. Do NOT serialize here.
             Where this contract and any prose disagree, the contract (firmware PRD §4.6) wins."

# MUST READ — the file being edited (read current state before editing)
- file: /home/dustin/projects/qmk_notifier/src/lib.rs
  why: "Contains RunCommand (the enum to extend, greenfield lib.rs:14-17), the run()
        match (the match to make exhaustive again), and the existing
        #[cfg(test)] mod tests block (where the 4 new tests go). HostOs goes between
        RunCommand and RunParameters."
  pattern: "Enum style: `///` doc comments + `pub enum` + `#[derive(Debug, Clone)]`.
            New variants/types follow the same doc style. New tests follow the existing
            test_<thing>_<scenario> naming and use the already-present `use super::*;`."
  gotcha: "Adding variants makes the run() match non-exhaustive (E0004). You MUST add the
           4 todo!() arms shown in the What section — without them `cargo build` fails."

# REFERENCE — crate PRD public API contract (shows the variant shapes + HostOs)
- file: /home/dustin/projects/qmk_notifier/PRD.md
  why: "§3/§7 give the exact RunCommand variant shapes (incl. ApplyHostContext's three
        fields) and HostOs { Unsure=0..Ios=4 }. Confirms naming and field order."
  section: "3. Public API" / "7. Crate Spec"

# REFERENCE — QMK os_variant_t (external confirmation of the HostOs discriminants)
- url: https://docs.qmk.fm/#/feature_os_detection?id=os-variant-t
  why: "Confirms QMK's os_variant_t enum: OS_UNSURE=0, OS_LINUX=1, OS_WINDOWS=2,
        OS_MACOS=3, OS_IOS=4. HostOs is an exact mirror."
  critical: "Discriminants are a wire contract — do not reorder or renumber."

# REFERENCE — research notes compiled for this (merged) subtask
- docfile: plan/002_637d65b6e9b8/P1M1T1S1/research/notes.md
  why: "Documents the merged scope (old S1+S2), the compile-exhaustiveness gotcha, the
        todo!() decision, derive math, test-count math, and the current committed state."
```

### Current Codebase tree (run from the crate root `/home/dustin/projects/qmk_notifier`)

```bash
qmk_notifier/
├── Cargo.toml          # name="qmk_notifier", version="0.2.1", edition="2021", deps: clap, hidapi, (toml/dirs/serde are unused legacy — do NOT touch)
├── Cargo.lock
├── README.md
├── PRD.md              # crate PRD (§3 Public API, §7 Crate Spec)
├── .gitignore          # contains only: /target
├── plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md   # WIRE SOURCE OF TRUTH
└── src
    ├── main.rs         # binary entrypoint (thin wrapper) — DO NOT TOUCH
    ├── core.rs         # transport: list/send/parse helpers + core::tests (13 tests) — DO NOT TOUCH
    ├── error.rs        # QmkError enum (11 variants, incl. forward-looking HidReadError/NoResponseReceived) — DO NOT TOUCH
    └── lib.rs          # <-- FILE TO EDIT: RunCommand, HostOs(new), RunParameters, parse_cli_args, run, mod tests
```

### Desired Codebase tree with files to be added/modified

```bash
src/
├── lib.rs   # MODIFIED ONLY — add HostOs enum, extend RunCommand, add 4 todo!() arms in run(), add 4 tests
└── (unchanged) core.rs, error.rs, main.rs, Cargo.toml, README.md
```

> No new files are created in this subtask. All new types are public and live in
> `lib.rs` (re-exported from the crate root). The `CommandResponse` enum and its
> tests are NOT created here (next subtask, P1.M1.T1.S2).

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL: adding variants to RunCommand breaks run()'s match exhaustiveness.
//   `run()` matches exactly `ListDevices` and `SendMessage` today. Adding
//   QueryInfo/QueryCallback/SetOs/ApplyHostContext triggers E0004 (non-exhaustive
//   match). You MUST add the 4 todo!() arms shown in the What section. This is
//   explicitly sanctioned by the item description and is temporary scaffolding
//   removed in P1.M1.T2.S2 (return type) / P1.M3.T3.S1 (real dispatch).

// CRITICAL: use `todo!()` with ONE explicit arm per variant (NOT `_ =>`).
//   - todo!() (not unimplemented!()) is the idiomatic "dispatch lands later"
//     placeholder; it returns `!`, which coerces to the arm's `Result<(), QmkError>`.
//   - Explicit arms keep the compiler's exhaustiveness check meaningful as the enum
//     grows. A `_ =>` wildcard would silently swallow a future variant.
//   - clippy::todo is in clippy's `restriction` group (allow by default), so default
//     `cargo clippy` stays green. Do NOT add #[allow(clippy::todo)].

// CRITICAL: do NOT change run()'s return type in this subtask.
//   It stays `Result<(), QmkError>`. Changing to `Result<CommandResponse, _>` is
//   P1.M1.T2.S2, and CommandResponse is P1.M1.T1.S2 (the next sibling). The todo!()
//   arms make the current signature compile unchanged.

// CRITICAL: `#[repr(u8)]` on HostOs is REQUIRED, not cosmetic.
//   For a fieldless enum with explicit discriminants, `as u8` returns the discriminant.
//   `#[repr(u8)]` additionally GUARANTEES size_of::<HostOs>() == 1 (the firmware wire
//   contract assumes a 1-byte os_byte). Omitting it leaves the size implementation-
//   defined. Keep `#[repr(u8)]`.

// CRITICAL: HostOs discriminants must EXACTLY mirror QMK os_variant_t.
//   Unsure=0, Linux=1, Windows=2, Macos=3, Ios=4. Do NOT reorder or renumber.
//   SET_OS sends `HostOs::X as u8` verbatim.

// NOTE: an unused-for-now `pub` enum/variant does NOT trigger `dead_code` warnings.
//   `pub` items are part of the crate's public API. `cargo build` currently emits
//   zero warnings and will continue to do so. Do NOT add #[allow(dead_code)].

// NOTE: `#[derive(Debug, Clone)]` on RunCommand stays AS-IS (no PartialEq/Eq/Copy).
//   All 4 new variants satisfy Debug+Clone: QueryInfo (unit), QueryCallback(u8),
//   SetOs(HostOs) — HostOs derives Debug+Clone; ApplyHostContext's Option<u8>/Vec<u8>/
//   bool all impl Debug+Clone. RunCommand owns a String ⇒ Copy is impossible.

// NOTE: the new tests must NOT call run() with a typed variant.
//   run() dispatch for typed commands is todo!() (panics). Tests are CONSTRUCTION
//   tests only (per item description). Do NOT add a #[should_panic] test — it would
//   only test temporary scaffolding.

// NOTE: toml/dirs/serde in Cargo.toml are unused legacy deps (dropped later). Do NOT
//   wire any new type to serde in this subtask.

// NOTE: HostOs MUST be defined BEFORE the RunCommand body references it in SetOs.
//   Within a single lib.rs edit this is automatic (Rust resolves items by name within
//   the module regardless of textual order), but define HostOs right after RunCommand
//   for readability and to match the prior plan's placement.
```

## Implementation Blueprint

### Data models and structure

This subtask introduces exactly **one new type** (`HostOs`) and **extends one existing
type** (`RunCommand` with 4 variants). There are no new structs, constructors, or trait
impls (the derives cover it). The `ApplyHostContext` variant uses an inline
**struct-variant** (named fields) — NOT a tuple variant and NOT a separate struct.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: READ the current state to confirm the greenfield starting point
  - READ: /home/dustin/projects/qmk_notifier/src/lib.rs.
  - CONFIRM: RunCommand has EXACTLY SendMessage(String) + ListDevices with
          `#[derive(Debug, Clone)]` (greenfield lib.rs:14-17).
  - CONFIRM: run()'s `match params.command` has EXACTLY 2 arms (ListDevices, SendMessage).
  - CONFIRM: the `#[cfg(test)] mod tests { use super::*; ... }` block at file bottom.
  - READ: /home/dustin/projects/qmk_notifier/plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md
          — confirm the 4 cmd_ids (0x01/0x02/0x03/0x05) and the os_byte table (0..=4).
  - GOAL: know the exact anchors so edits are surgical.
  - EDGE CASE: if RunCommand ALREADY has the 4 typed variants AND HostOs already exists
          AND the 4 todo!() arms + 4 tests are present, the working tree already
          satisfies this subtask — run the Validation Loop to confirm, and make no
          changes (see research/notes.md "Current committed state"). Do NOT add
          CommandResponse (next subtask).

Task 2: ADD the HostOs enum to src/lib.rs
  - INSERT: the full `pub enum HostOs { ... }` block (see What section (a)).
  - PLACEMENT: immediately AFTER the closing `}` of the `RunCommand` enum and BEFORE
          the `/// Parameters required for running QMK notifier operations` doc on
          `RunParameters`. One blank line of separation on each side.
  - ATTRIBUTES: exactly `#[repr(u8)]` then `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`.
  - DOC: enum-level `///` doc (2 lines) PLUS a `///` doc on EACH of the 5 variants.
  - NAMING: `Unsure, Linux, Windows, Macos, Ios` (note `Macos`/`Ios`, NOT `MacOS`/`iOS`).

Task 3: EXTEND the RunCommand enum body in src/lib.rs
  - REPLACE: the greenfield RunCommand (SendMessage + ListDevices + single-line doc)
          with the 6-variant target body (see What section (b)).
  - KEEP: `#[derive(Debug, Clone)]` unchanged.
  - ADD: QueryInfo, QueryCallback(u8), SetOs(HostOs), ApplyHostContext { layer, callbacks,
          clear_board } AFTER ListDevices, in that order, with the `///` doc comments.
  - KEEP: SendMessage(String) and ListDevices unchanged (byte-for-byte).
  - NAMING: variants exactly QueryInfo, QueryCallback, SetOs, ApplyHostContext (PascalCase).
          ApplyHostContext fields exactly `layer`, `callbacks`, `clear_board` (snake_case),
          in that order.
  - DO NOT: touch RunParameters, parse_cli_args, or any import.

Task 4: RESTORE run() match exhaustiveness with todo!() arms
  - ADD: 4 arms to `match params.command` in run() — one per new variant, each
          `=> todo!("typed dispatch lands in P1.M3.T3.S1")` (see What section (c)).
          Note ApplyHostContext uses the `{ .. }` pattern.
  - KEEP: run()'s signature `pub fn run(params: RunParameters) -> Result<(), QmkError>`.
  - KEEP: the existing ListDevices and SendMessage arms byte-for-byte unchanged.
  - DO NOT: add a `_ =>` wildcard. DO NOT add real dispatch logic. DO NOT change return type.

Task 5: ADD the 4 unit tests to the existing mod tests block
  - ADD: test_host_os_discriminants_match_firmware_contract,
         test_run_command_query_variants_construction,
         test_run_command_set_os_variant_construction,
         test_run_command_apply_host_context_construction (see What section (d)).
  - PLACEMENT: inside the existing `#[cfg(test)] mod tests { use super::*; ... }` block;
          group the 4 type-surface tests together near the top of the block.
  - PATTERN: use the already-present `use super::*;` — do NOT re-import.
  - NAMING: snake_case test_<thing>_<scenario> (matches file convention).
  - DO NOT: call run() with a typed variant (would panic on todo!()). DO NOT add #[should_panic].

Task 6: VALIDATE (do not skip)
  - RUN (from /home/dustin/projects/qmk_notifier): `cargo fmt`, then `cargo build`,
          then `cargo clippy --lib`, then `cargo fmt --check`, then `cargo test --lib`.
  - EXPECT: build 0 warnings; clippy no new warnings; fmt --check exit 0; all tests pass.
  - IF E0004 (non-exhaustive match): you forgot a todo!() arm — add it.
  - IF "cannot find type HostOs": you skipped Task 2 — define HostOs first.
```

### Implementation Patterns & Key Details

```rust
// === PLACEMENT ANCHOR (illustrative; match exact surrounding lines) ===
//
// #[derive(Debug, Clone)]
// pub enum RunCommand {
//     SendMessage(String),
//     ListDevices,
//     // >>> (Task 3) ADD the 4 new variants HERE, with /// doc comments <<<
// }
//
// // >>> (Task 2) INSERT HostOs HERE (blank line above and below) <<<
//
// /// Parameters required for running QMK notifier operations
// pub struct RunParameters { ... }


// === run() MATCH ANCHOR (illustrative) ===
//
// pub fn run(params: RunParameters) -> Result<(), QmkError> {
//     match params.command {
//         RunCommand::ListDevices => list_hid_devices(),
//         RunCommand::SendMessage(message) => { /* unchanged body */ }
//         // >>> (Task 4) ADD 4 todo!() arms HERE, before the closing } <<<
//     }
// }


// === WHY todo!() NOT unimplemented!() / _ => ===
//   todo!()        → idiomatic "dispatch later"; type `!`; coerces to any arm type.
//   unimplemented!→ same panic, weaker intent (use todo! for "planned").
//   `_ =>`         → AVOID: silently hides a future 7th variant from the
//                    exhaustiveness check. Explicit arms keep the check sharp.
```

### Integration Points

```yaml
SOURCE FILES:
  - modify: "/home/dustin/projects/qmk_notifier/src/lib.rs ONLY"
  - add:    "pub enum HostOs (after RunCommand, before RunParameters)"
  - extend: "pub enum RunCommand (4 new variants + expanded doc comment)"
  - modify: "run() — add 4 todo!() match arms (no signature change)"
  - add:    "4 #[test] fns inside the existing #[cfg(test)] mod tests block"

DEPENDENCIES / Cargo.toml:
  - none. No new crate deps. (Do NOT add serde derives — see gotchas.)

PUBLIC API SURFACE:
  - adds:    "qmk_notifier::HostOs (pub enum);
              RunCommand::QueryInfo, RunCommand::QueryCallback(u8),
              RunCommand::SetOs(HostOs),
              RunCommand::ApplyHostContext { layer, callbacks, clear_board }"
  - unchanged: "SendMessage, ListDevices, RunParameters, parse_cli_args, run signature,
                all core:: re-exports"

OUT-OF-SCOPE (next subtask — do NOT implement here):
  - P1.M1.T1.S2: "CommandResponse enum (Legacy/Info/CallbackName/Ack/Timeout) + its tests"

DOWNSTREAM CONSUMERS (do NOT implement now — listed for awareness):
  - P1.M2.T1.S1: "build_command_data serializes QueryInfo/QueryCallback/SetOs to
                  [0x81][0x9F][0xF0][cmd][args][0x03]; single report each."
  - P1.M2.T1.S2: "build_command_data serializes ApplyHostContext: layer None⇒0xFF,
                  Some(n)⇒n; clear_board⇒flags bit 0; callbacks as [count][id…]
                  (may span multiple reports)."
  - P1.M2.T2.S1: "parse_reply decodes 0x51 replies; response[1] cmd echo maps back to
                  these variants."
  - P1.M3.T3.S1: "run() dispatch: replaces the 4 todo!() arms with real
                  build_command_data → send_raw_report → parse_reply."
  - P1.M1.T2.S2: "run() return type Result<(),QmkError> → Result<CommandResponse,QmkError>;
                  removes/replaces these todo!() arms."
  - P4.M1.T1.S1: "QMKonnect Notifier trait gains send_command(RunCommand)."
```

## Validation Loop

> All commands run from the crate root: `/home/dustin/projects/qmk_notifier`

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmk_notifier

# Format the edited file (rustfmt default style — no rustfmt.toml exists).
cargo fmt

# Build the whole crate — must compile with ZERO warnings.
cargo build 2>&1 | tee /tmp/build.log
# Expected: "Finished `dev` profile ..." and NO "warning:" lines.
# If you see error[E0004]: non-exhaustive patterns — you forgot a todo!() arm.

# Lint (default clippy — no .clippy.toml exists).
cargo clippy --lib 2>&1 | tee /tmp/clippy.log
# Expected: no warnings/errors specific to the new variants or todo!() arms.
# (clippy::todo is allow-by-default, so todo!() is NOT flagged.)

# Formatting check (CI-style gate).
cargo fmt --check
# Expected: exit code 0 (no diff). If non-zero, re-run `cargo fmt`.
```

### Level 2: Unit Tests (Component Validation)

```bash
cd /home/dustin/projects/qmk_notifier

# Run the 4 new tests in isolation first.
cargo test --lib test_host_os_discriminants_match_firmware_contract -- --nocapture
cargo test --lib test_run_command_query_variants_construction -- --nocapture
cargo test --lib test_run_command_set_os_variant_construction -- --nocapture
cargo test --lib test_run_command_apply_host_context_construction -- --nocapture
# Expected: 1 passed each.

# Run the full lib test suite (lib.rs unit tests + core.rs unit tests).
cargo test --lib
# Expected: "test result: ok. <N> passed; 0 failed; 0 ignored; ...".
# Greenfield baseline is 22 tests (9 in lib.rs + 13 in core.rs); this subtask adds 4
# → 26. (If CommandResponse / P1.M1.T1.S2 has already landed in the tree, N will be
# higher by its 3 tests — that is fine; the point is 0 failed.)

# Sanity: confirm the existing run() integration tests STILL pass (they must not hit
# a todo!() arm).
cargo test --lib test_run_with_ -- --nocapture
# Expected: test_run_with_list_devices_command, test_run_with_send_message_command,
#           test_run_with_verbose_output all pass (Ok or benign Err, never panic).
```

### Level 3: Integration Testing (System Validation)

```text
NOT APPLICABLE for this subtask.
The new variants/types carry structured data only — they have no I/O and no CLI surface
yet. There is no live-hardware path to exercise until build_command_data (P1.M2.T1) and
run() typed dispatch (P1.M3.T3.S1) land. Calling run() with a typed variant today would
hit todo!() and panic (by design). The construction/discriminant unit tests in Level 2
ARE the end-to-end type-surface verification for this task.
```

### Level 4: Creative & Domain-Specific Validation

```bash
cd /home/dustin/projects/qmk_notifier

# Confirm the type surface is publicly reachable and the match is exhaustive
# (the compiler enforces exhaustiveness; a clean build IS the proof):
cargo build --lib 2>&1 | grep -iE "RunCommand|non-exhaustive|warning" || \
  echo "RunCommand: no build diagnostics (good — exhaustive match, no warnings)"

# Optional: confirm clippy sees the todo!() arms as acceptable (no warning):
cargo clippy --lib 2>&1 | grep -i "todo" || echo "clippy: todo!() not flagged (good)"

# Optional: confirm the new public type is exported from the crate root:
cargo doc --lib --no-deps 2>&1 | grep -i "HostOs" || echo "HostOs documented (or no diagnostics)"
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1 passed: `cargo build` → zero warnings (no E0004 non-exhaustive).
- [ ] Level 1 passed: `cargo clippy --lib` → zero new warnings.
- [ ] Level 1 passed: `cargo fmt --check` → exit 0.
- [ ] Level 2 passed: `cargo test --lib` → all pass, 0 failed.
- [ ] The 4 new tests pass individually; the 3 existing `test_run_with_*` tests still pass.

### Feature Validation

- [ ] `pub enum HostOs` present with variants Unsure/Linux/Windows/Macos/Ios, discriminants `0,1,2,3,4`.
- [ ] `HostOs` has `#[repr(u8)]` and `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`.
- [ ] `RunCommand` has all 6 variants in the correct order with correct field types.
- [ ] `ApplyHostContext` is an inline struct variant with fields `layer: Option<u8>`,
      `callbacks: Vec<u8>`, `clear_board: bool` in that order.
- [ ] Each new type/variant has a `///` doc comment naming its cmd_id and the firmware
      PRD §4.6 command table (Mode A).
- [ ] `run()` has 4 new `todo!()` arms (explicit, one per variant; no `_ =>`); signature
      unchanged (`Result<(), QmkError>`).
- [ ] `#[derive(Debug, Clone)]` on RunCommand unchanged (no new derives added).
- [ ] Only `src/lib.rs` modified; `CommandResponse` NOT added (it is the next subtask).

### Code Quality Validation

- [ ] Follows existing enum doc style (`///` per variant/type) and derive conventions.
- [ ] New tests follow the file's `test_<thing>_<scenario>` naming + `use super::*`.
- [ ] No `#[allow(dead_code)]` / `#[allow(clippy::todo)]` added (unnecessary).
- [ ] No serde/Display/TryFrom/serialization logic added (out of scope — P1.M2.T1).

### Documentation & Deployment

- [ ] Types/variants are self-documenting via `///` (Mode A — no separate docs file).
- [ ] ApplyHostContext doc encodes layer=None⇒0xFF and clear_board⇒flags-bit-0 semantics.
- [ ] No new environment variables or config.
- [ ] No `Cargo.toml` change (no new deps).

---

## Anti-Patterns to Avoid

- ❌ Don't forget the 4 `todo!()` arms — the match becomes non-exhaustive (E0004) and
  `cargo build` fails. Add them in the same change as the variants.
- ❌ Don't use `_ => todo!()` — a wildcard hides future variants from the exhaustiveness
  check. Use one explicit arm per variant.
- ❌ Don't change `run()`'s return type to `CommandResponse` — that's P1.M1.T2.S2, and
  `CommandResponse` is P1.M1.T1.S2 (next sibling).
- ❌ Don't add real typed-command dispatch logic in the `todo!()` arms — dispatch is
  P1.M3.T3.S1. This subtask only makes the match compile.
- ❌ Don't add `PartialEq`/`Eq`/`Copy` to RunCommand's derives — it owns a `String`
  (Copy impossible) and the item says "match existing derives" (Debug+Clone only).
- ❌ Don't omit `#[repr(u8)]` on HostOs — it guarantees the 1-byte layout the wire
  contract assumes for os_byte.
- ❌ Don't reorder/renumber HostOs variants to "look nicer" — discriminants are a wire
  contract (0=UNSURE..4=IOS).
- ❌ Don't rename HostOs variants to `MacOS`/`iOS` — use exactly `Macos`/`Ios` (PRD §7).
- ❌ Don't reorder `ApplyHostContext` fields to flags-first — order MUST be
  `layer, callbacks, clear_board` (matches PRD §7 and the wire layout).
- ❌ Don't add `#[allow(dead_code)]` — `pub` enum variants don't trigger it.
- ❌ Don't serialize the variants to wire bytes here — `build_command_data` is P1.M2.T1.
- ❌ Don't add a test that calls `run()` with a typed variant (it panics on `todo!()`),
  and don't add a `#[should_panic]` test (it only tests scaffolding).
- ❌ Don't create `CommandResponse` or its tests here — that is the next subtask
  (P1.M1.T1.S2).
- ❌ Don't skip `cargo fmt` / `cargo test` because "it's just enum variants" — the
  discriminant + construction tests are the contract check that protects every
  downstream task.

---

**Confidence Score: 10/10** for one-pass implementation success. The deliverable is two
fully-specified enums (verbatim bodies + doc comments), a fully-specified `run()` edit
(with the non-obvious `todo!()` rationale and the explicit "don't change the return type"
guard), and 4 ready-to-paste tests, placed against precise anchors in a single file, with
the source-of-truth wire contract quoted and verified working build/clippy/fmt/test
commands. The one real risk — the exhaustiveness break — is called out multiple times and
fixed by the same edit. The merged scope (HostOs + RunCommand) has no internal ordering
hazard: HostOs is referenced by name in `SetOs(HostOs)` and Rust resolves module items by
name regardless of textual order.
# PRP — P1.M1.T1.S2: Add `CommandResponse` enum

> **Crate:** `qmk_notifier` (v0.2.1) at `/home/dustin/projects/qmk_notifier`
> **Repo context:** This task is part of the QMKonnect `plan/002` orchestrator plan,
> but ALL edits land in the **`qmk_notifier` crate** (a separate repo that QMKonnect pins
> by git tag per PRD §7). Work in `/home/dustin/projects/qmk_notifier`.
> **Scope line:** This subtask is **type-only**. It adds one `pub enum CommandResponse`
> (the parsed-device-reply type) + rustdoc + tests to `src/lib.rs`. It does NOT parse
> replies, does NOT change `run()`'s signature, does NOT touch the `todo!()` arms.

---

## Goal

**Feature Goal**: Add the **parsed device reply** type to the `qmk_notifier` crate's
public API in one self-contained edit of `src/lib.rs`:

A new `pub enum CommandResponse` with exactly five variants — `Legacy { matched }`,
`Info { proto_ver, feature_flags, callback_count, board_rules_present }`,
`CallbackName { index, name: Option<String> }`, `Ack { ok }`, `Timeout` — that model the
three reply shapes the firmware emits plus the no-reply case, as defined by the wire
contract (`firmware_wire_contract.md` §Field Definitions + §Reply Disambiguation) and PRD
§3 / §8 / §10.2. Each variant carries the **decoded, typed fields** a host caller consumes
(NOT raw wire bytes); byte decoding is the job of `parse_reply` in **P1.M1.T3.S1** (out of
scope here). Derives `Debug, Clone, PartialEq, Eq` exactly.

**Deliverable**: `src/lib.rs` (only) containing the new `CommandResponse` enum (with
enum-level + per-variant rustdoc) plus three unit tests inside the existing
`#[cfg(test)] mod tests` block. Consumed downstream by `parse_reply` (P1.M1.T3.S1, the
producer), the `run()` return-type change (P1.M1.T3.S2), and ultimately QMKonnect's
startup handshake + capability detection (P4.M2.T1.S1).

**Success Definition**: `cargo build` compiles with **zero warnings** (and
`cargo clippy --lib` introduces none); `cargo fmt --check` exits 0; `cargo test --lib`
passes with **all greenfield tests + 3 new tests**; the `CommandResponse` enum exists with
exactly the five variants, exact field names/types/order, and `#[derive(Debug, Clone,
PartialEq, Eq)]`; the existing `test_run_with_*` and S1 type tests still pass; no file
other than `src/lib.rs` is modified; `run()`'s signature and its `todo!()` arms are
untouched; `RunCommand`, `HostOs`, `RunParameters`, `parse_cli_args`, `core.rs`,
`error.rs`, `main.rs`, and `Cargo.toml` are untouched.

## User Persona (if applicable)

**Target User**: The downstream implementer of the v0.3.0 reply path (`parse_reply` in
P1.M1.T3.S1, the `run()` return-type change in P1.M1.T3.S2) and, ultimately, the QMKonnect
desktop app that pattern-matches on this enum during the startup handshake (QUERY_INFO →
QUERY_CALLBACK sweep → SET_OS, P4.M2.T1.S1).

**Use Case**: A host caller sends a typed command, the transport reads one 32-byte IN
report, `parse_reply` decodes it into a `CommandResponse`, and the caller pattern-matches
(`match resp { CommandResponse::Info { .. } => …, CommandResponse::Timeout => legacy-mode,
… }`) to decide capability and next action. Today (S2) the API can *express* every reply
shape; tomorrow (P1.M1.T3) the transport produces them.

**User Journey**: firmware replies `[0x51][0x01][2][0x03][5][1]` → (P1.M1.T3.S1)
`parse_reply` → `CommandResponse::Info { proto_ver: 2, feature_flags: 0x03,
callback_count: 5, board_rules_present: true }` → (P4.M2.T1.S1) host sees proto_ver==2 &&
flags & 0x01 ⇒ begin QUERY_CALLBACK sweep.

**Pain Points Addressed**: Removes the "the device either matched a string or didn't"
binary; gives the type system jurisdiction over the full typed-reply surface so reply
decoding bugs are caught at compile time and the caller's `match` is exhaustive.

## Why

- `CommandResponse` is the **reply-type layer** of the M1 "Type Contracts" milestone —
  the counterpart to S1's `RunCommand`/`HostOs` (request side). It is the smallest,
  dependency-free building block of the v0.3.0 reply path (PRD §5 Wire Protocol
  "Responses", §7 Crate Spec, §8 QMKonnect response handling).
- Defining the reply type **before** the parser keeps the dependency chain clean
  (types → pure framing → parser → transport) and lets each later subtask be validated
  against a fixed, compiling type surface. `parse_reply` (P1.M1.T3.S1) will return this
  enum; the `run()` signature change (P1.M1.T3.S2) will surface it from `run()`.
- It is **purely additive to the type surface** — no behavior, no I/O, no `run()` change.
  The only "change" to `src/lib.rs` is the enum + tests (the enum is unused-by-`run()`
  until P1.M1.T3.S2; a `pub` item does NOT trigger `dead_code`, so `cargo build` stays
  warning-free).

## What

### The new `CommandResponse` enum — placed AFTER `HostOs` and BEFORE `RunParameters`

```rust
/// Parsed device reply (see PRD §8 and §10.2; canonical byte layouts in
/// `firmware_wire_contract.md` §Field Definitions and §Reply Disambiguation).
///
/// Produced by `parse_reply` (P1.M1.T3.S1) from a single 32-byte IN report read
/// after a command burst. `response[0]` disambiguates the reply: `0x51` ⇒ typed
/// reply (decoded by the `response[1]` cmd echo); `0`/`1` ⇒ legacy match-bool;
/// no reply within the bounded `read_timeout` ⇒ `Timeout`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandResponse {
    /// Legacy string reply: `response[0]` is `0` (no match) or `1` (matched).
    /// Returned for `SendMessage`, and for a typed command answered by a
    /// non-capable (legacy) device that walks the typed bytes as a no-match
    /// string. See PRD §8, §10.2.
    Legacy { matched: bool },
    /// `QUERY_INFO` (cmd `0x01`) typed reply:
    /// `[0x51][0x01][proto_ver][feature_flags][callback_count][board_rules_present]`.
    /// See `firmware_wire_contract.md` §QUERY_INFO response.
    Info {
        proto_ver: u8,
        feature_flags: u8,
        callback_count: u8,
        board_rules_present: bool,
    },
    /// `QUERY_CALLBACK` (cmd `0x02`) typed reply:
    /// `[0x51][0x02][index][name bytes, NUL-padded]`. `name` is `None` when the
    /// callback has no name or the index is out of range (the firmware emits an
    /// immediate `0x00` NUL at the name position). See `firmware_wire_contract.md`
    /// §QUERY_CALLBACK response.
    CallbackName { index: u8, name: Option<String> },
    /// `SET_OS` (cmd `0x03`) / `APPLY_HOST_CONTEXT` (cmd `0x05`) typed reply:
    /// `[0x51][cmd_echo][ack]`. `ok` is `true` when `ack == 1` (applied). Shared
    /// by both ack-style commands. See `firmware_wire_contract.md` §SET_OS /
    /// §APPLY_HOST_CONTEXT response.
    Ack { ok: bool },
    /// No reply arrived within the bounded `read_timeout` — the device is legacy
    /// or offline. The caller treats this as a non-capable device and stays in
    /// string-only mode. See PRD §10.2, §8.
    Timeout,
}
```

> **`run()` is NOT touched.** It stays `pub fn run(params: RunParameters) ->
> Result<(), QmkError>` with its four `todo!()` arms (S1 scaffolding). Changing `run()`'s
> return type to `Result<CommandResponse, QmkError>` and replacing the `todo!()` arms is
> **P1.M1.T3.S2** (separate subtask). In S2 `CommandResponse` is defined but not yet
> returned by `run()`. A `pub` enum that nothing in the crate constructs yet is NOT a
> dead-code warning — verified (the committed tree compiles warning-free).

### The three unit tests — inside the existing `#[cfg(test)] mod tests` block

```rust
#[test]
fn test_command_response_info_construction() {
    // QUERY_INFO reply: proto_ver=2 (typed-capable), feature_flags=0x03
    // (APPLY_HOST_CONTEXT | callback registry), 5 callbacks, board map present.
    let info = CommandResponse::Info {
        proto_ver: 2,
        feature_flags: 0x03,
        callback_count: 5,
        board_rules_present: true,
    };
    match info {
        CommandResponse::Info {
            proto_ver,
            feature_flags,
            callback_count,
            board_rules_present,
        } => {
            assert_eq!(proto_ver, 2);
            assert_eq!(feature_flags, 0x03);
            assert_eq!(callback_count, 5);
            assert!(board_rules_present);
        }
        _ => panic!("expected Info"),
    }
    // PartialEq/Eq derive (mandated by the item) must hold for the result type.
    assert_eq!(
        info,
        CommandResponse::Info {
            proto_ver: 2,
            feature_flags: 0x03,
            callback_count: 5,
            board_rules_present: true,
        }
    );
}

#[test]
fn test_command_response_callback_name_construction() {
    // Named callback: index echoed back, ASCII name present.
    let named = CommandResponse::CallbackName {
        index: 3,
        name: Some("layer_tap".to_string()),
    };
    // Bind by reference so `named` stays intact for the PartialEq/Eq
    // assertions below (CommandResponse owns an Option<String> and is
    // intentionally non-Copy — see PRP gotchas).
    match named {
        CommandResponse::CallbackName { index, ref name } => {
            assert_eq!(index, 3);
            assert_eq!(name.as_deref(), Some("layer_tap"));
        }
        _ => panic!("expected CallbackName"),
    }

    // Unnamed / out-of-range callback: firmware emits an immediate NUL ⇒ None.
    let unnamed = CommandResponse::CallbackName {
        index: 99,
        name: None,
    };
    assert_eq!(
        unnamed,
        CommandResponse::CallbackName {
            index: 99,
            name: None
        }
    );
    assert_ne!(named, unnamed, "distinct index/name must not compare equal");
}

#[test]
fn test_command_response_legacy_ack_timeout_construction() {
    // Legacy match-bool reply (response[0] ∈ {0,1}).
    let matched = CommandResponse::Legacy { matched: true };
    let no_match = CommandResponse::Legacy { matched: false };
    assert_eq!(matched, CommandResponse::Legacy { matched: true });
    assert_ne!(matched, no_match);

    // SET_OS / APPLY_HOST_CONTEXT ack reply (ack==1 ⇒ applied).
    let ok = CommandResponse::Ack { ok: true };
    let fail = CommandResponse::Ack { ok: false };
    assert_eq!(ok, CommandResponse::Ack { ok: true });
    assert_ne!(ok, fail);

    // No reply within read_timeout (device legacy/offline).
    let t = CommandResponse::Timeout;
    assert_eq!(t, CommandResponse::Timeout);

    // Cross-variant inequality: different variants must never compare equal
    // (sanity-check the derived PartialEq across the whole enum).
    assert_ne!(CommandResponse::Timeout, CommandResponse::Ack { ok: false });
}
```

### Success Criteria

- [ ] `pub enum CommandResponse` exists in `src/lib.rs` with exactly the five variants in
      this order: `Legacy { matched: bool }`, `Info { proto_ver: u8, feature_flags: u8,
      callback_count: u8, board_rules_present: bool }`, `CallbackName { index: u8, name:
      Option<String> }`, `Ack { ok: bool }`, `Timeout`.
- [ ] It carries exactly `#[derive(Debug, Clone, PartialEq, Eq)]` (NOT `Copy`).
- [ ] It is placed AFTER `HostOs` and BEFORE `RunParameters` (one blank line separation
      each side).
- [ ] The enum has a multi-line rustdoc AND each variant has a `///` doc naming its
      cmd_id (where applicable) and referencing the wire contract / PRD (Mode A).
- [ ] Three new `#[test]` fns exist in the existing `mod tests`; all 5 variants are
      exercised (Info, CallbackName both named+unnamed, Legacy, Ack, Timeout, plus a
      cross-variant `assert_ne!`).
- [ ] `run()` signature unchanged (`Result<(), QmkError>`); the four `todo!()` arms are
      unchanged; `RunCommand`, `HostOs`, `RunParameters`, `parse_cli_args` unchanged.
- [ ] `cargo build` → zero warnings; `cargo clippy --lib` → no new warnings;
      `cargo fmt --check` → exit 0; `cargo test --lib` → all pass.
- [ ] No file other than `src/lib.rs` is modified.

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed to
> implement this successfully?"_ — **Yes.** The exact target enum body (verbatim, with
> doc comments), the exact three tests (verbatim), the precise placement anchor, the
> source-of-truth wire layouts (quoted), the explicit "do NOT touch `run()`" guard, and
> verified build/clippy/fmt/test commands are all below. The implementer does not need to
> read any QMK firmware source — `firmware_wire_contract.md` canonicalizes every reply
> byte layout.

> **BASELINE ALERT — read this first.** As of this PRP's authoring, the committed tree
> (`git log: 5bdbe92 "Add CommandResponse enum for parsed device replies"`) **already
> contains a `CommandResponse` enum that matches this contract byte-for-byte**, plus the
> three tests above. So the most likely implementation path is **verify-and-validate**:
> diff the tree against the "What" section; if it matches, make NO source edits and just
> run the Validation Loop. Only if `CommandResponse` is absent or differs should you
> add/edit it. Do not "re-add" an identical enum and create a pointless diff.

### Documentation & References

```yaml
# MUST READ — the canonical wire contract (reply byte layouts + disambiguation table)
- file: /home/dustin/projects/qmk_notifier/plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md
  why: "The single source of truth for every byte this enum's variants decode. Defines:
        QUERY_INFO response offsets [2]proto_ver/[3]feature_flags/[4]callback_count/
        [5]board_rules_present (0/1→bool); QUERY_CALLBACK response [index][name NUL-padded]
        with immediate-NUL⇒no-name; SET_OS/APPLY_HOST_CONTEXT ack responses
        [0x51][cmd_echo][ack] (ack==1⇒applied); and the §Reply Disambiguation table
        (0x51 typed, 0/1 legacy match-bool, no-reply⇒Timeout). The variant field
        types/shapes come directly from here."
  section: "Field Definitions (QUERY_INFO/QUERY_CALLBACK/SET_OS/APPLY_HOST_CONTEXT
            responses)", "Reply Disambiguation", "Constants (NOTIFY_RESPONSE_MARKER=0x51)"
  critical: "board_rules_present is a u8(0/1) on the wire but modeled as bool in Rust —
             the !=0 coercion is parse_reply's job (P1.M1.T3.S1), NOT this subtask's.
             name is modeled as Option<String> (None when the firmware emits an immediate
             0x00 NUL). Ack is ONE shared variant for both cmd 0x03 and 0x05 because the
             reply shape is identical. Where this contract and any prose disagree, the
             contract (firmware PRD §4.6) wins."

# MUST READ — the sibling PRP whose output S2 consumes (HostOs + RunCommand)
- file: /home/dustin/projects/qmkonnect/plan/002_637d65b6e9b8/P1M1T1S1/PRP.md
  why: "Defines the exact HostOs enum and extended RunCommand (6 variants) that S2's
        CommandResponse replies TO. Treat as a contract: HostOs + the 4 typed RunCommand
        variants + the 4 todo!() arms will exist in src/lib.rs when S2 runs. S2 must NOT
        redefine/modify any of them."
  section: "What" (HostOs enum, RunCommand enum, run() todo!() arms)
  critical: "S2 adds CommandResponse ONLY. Do not touch RunCommand/HostOs/run(). The
             todo!() arms are temporary scaffolding removed in P1.M1.T3.S2."

# MUST READ — the file being edited (read current state before editing)
- file: /home/dustin/projects/qmk_notifier/src/lib.rs
  why: "Contains RunCommand (S1 output), HostOs (S1 output), run() with 4 todo!() arms
        (S1 scaffolding — DO NOT TOUCH), and the existing #[cfg(test)] mod tests block
        (where the 3 new tests go). CommandResponse goes between HostOs and
        RunParameters. NOTE: the committed tree may ALREADY contain CommandResponse + the
        3 tests — verify before editing (see BASELINE ALERT above)."
  pattern: "Enum style: multi-line `///` doc + `pub enum` + `#[derive(Debug, Clone,
            PartialEq, Eq)]`. New tests follow the file's `test_<thing>_<scenario>`
            naming + the already-present `use super::*;`."
  gotcha: "Do NOT change run()'s signature or its todo!() arms — that is P1.M1.T3.S2.
           Defining CommandResponse makes it a pub type that run() does not yet return;
           this is expected and compiles warning-free (pub items are not dead-code)."

# REFERENCE — crate PRD public API contract (shows the enum verbatim)
- file: /home/dustin/projects/qmk_notifier/PRD.md
  why: "§3 gives the CommandResponse enum verbatim (Legacy/Info/CallbackName/Ack/Timeout).
        §8 describes the response-handling semantics (response[0]==0x51⇒typed, 0/1⇒Legacy,
        no-reply⇒Timeout). §14 invariant 6 codifies the disambiguation."
  section: "3. Public API" / "8. Error Model & response handling" / "14. Key Invariants"

# REFERENCE — merged-plan PRD sections (the authoritative cross-repo spec)
- docfile: (in the orchestrator's merged PRD) §7 "Crate Spec (qmk_notifier, Rust)"
           and §5 "Wire Protocol (typed commands) → Responses"
  why: "§7 reproduces the exact CommandResponse + run()->Result<CommandResponse,_>
        signature (the signature change is P1.M1.T3.S2, NOT here). §5 'Responses' states:
        legacy [matched(0|1)]…; typed [0x51][cmd_id_echo][payload]…; no reply ⇒ Timeout
        ⇒ host stays string-only."

# REFERENCE — research notes compiled for this subtask
- docfile: /home/dustin/projects/qmkonnect/plan/002_637d65b6e9b8/P1M1T1S2/research/notes.md
  why: "Documents the BASELINE ALERT (enum already committed at 5bdbe92), the derive math
        (why PartialEq/Eq but NOT Copy — CallbackName owns Option<String>), the per-variant
        wire-layout provenance, the 'run() NOT touched' boundary, and the placement."
```

### Current Codebase tree (run from the crate root `/home/dustin/projects/qmk_notifier`)

```bash
qmk_notifier/
├── Cargo.toml          # name="qmk_notifier", version="0.2.1", edition="2021", deps: clap, hidapi, (toml/dirs/serde unused legacy — do NOT touch)
├── Cargo.lock
├── README.md
├── PRD.md              # crate PRD (§3 Public API, §8 response handling, §14 invariants)
├── .gitignore          # contains only: /target
├── plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md   # REPLY/WIRE SOURCE OF TRUTH
└── src
    ├── main.rs         # binary entrypoint (thin wrapper) — DO NOT TOUCH
    ├── core.rs         # transport: list/send/parse helpers + core::tests (13 tests) — DO NOT TOUCH
    ├── error.rs        # QmkError enum (incl. forward-looking HidReadError/NoResponseReceived) — DO NOT TOUCH
    └── lib.rs          # <-- FILE TO EDIT (ONLY): RunCommand(S1), HostOs(S1), CommandResponse(THIS), RunParameters, parse_cli_args, run, mod tests
```

> `git log -- src/lib.rs` shows `5bdbe92 Add CommandResponse enum for parsed device replies`
> is ALREADY HEAD. Read `src/lib.rs` before editing — the enum may already be present and
> correct (verify-and-validate path; see BASELINE ALERT).

### Desired Codebase tree with files to be added/modified

```bash
src/
├── lib.rs   # MODIFIED ONLY IF the tree lacks/defers CommandResponse — add enum (after HostOs, before RunParameters) + 3 tests
└── (unchanged) core.rs, error.rs, main.rs, Cargo.toml, README.md
```

> No new files are created. If the committed tree already satisfies this contract, make
> ZERO source edits and just run the Validation Loop.

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL: derive EXACTLY Debug, Clone, PartialEq, Eq — NOT Copy.
//   CommandResponse owns an Option<String> (in CallbackName). String is Eq but NOT Copy,
//   so PartialEq/Eq derive cleanly but Copy is IMPOSSIBLE. The item mandates these four
//   derives; do not add or remove any.

// CRITICAL: do NOT change run()'s return type in this subtask.
//   It stays `pub fn run(params: RunParameters) -> Result<(), QmkError>` with its 4
//   todo!() arms (S1 scaffolding). Changing to `Result<CommandResponse, _>` and replacing
//   the todo!() arms is P1.M1.T3.S2 (separate subtask). Defining CommandResponse now does
//   not require run() to reference it — a pub type can exist unused-by-run() until the
//   later subtask wires it in.

// CRITICAL: board_rules_present is a bool in Rust, a u8(0/1) on the wire.
//   The !=0 coercion belongs to parse_reply (P1.M1.T3.S1), NOT this subtask. Model it as
//   `bool` here (the decoded shape the caller pattern-matches). Do not store a u8.

// CRITICAL: `name` is Option<String>, and None means "no name / out-of-range index".
//   The firmware emits an immediate 0x00 NUL at the name position when the callback has
//   no name or the index is out of range. parse_reply (P1.M1.T3.S1) turns that into None;
//   here we just model the decoded Option<String>.

// CRITICAL: Ack is ONE shared variant for cmd 0x03 (SET_OS) and cmd 0x05 (APPLY_HOST_CONTEXT).
//   Both replies are `[0x51][cmd_echo][ack]` with ack==1⇒applied — identical shape. Do NOT
//   split into SetOsAck/ApplyAck variants. The cmd echo disambiguates which command
//   produced it; the variant shape does not.

// CRITICAL: variant ORDER must be Legacy, Info, CallbackName, Ack, Timeout.
//   Matches PRD §3/§7 and the wire-contract §Reply Disambiguation grouping (legacy first,
//   typed middle, timeout last). Do not reorder.

// NOTE: a `pub` enum that run() does not yet return is NOT a dead-code warning.
//   `pub` items are part of the crate's public API surface. `cargo build` stays
//   warning-free. Do NOT add #[allow(dead_code)].

// NOTE: an unused-for-now `pub` enum does NOT need #[non_exhaustive].
//   This crate is not yet 1.0 and the enum is internal to this repo's release cadence; the
//   PRD does not ask for non_exhaustive. Match across all variants exhaustively in tests.
//   Do NOT add #[non_exhaustive].

// NOTE: the 3 tests must NOT call run() expecting a CommandResponse.
//   run() still returns Result<(), QmkError> in S2 (todo!() arms for typed cmds). The 3
//   tests are pure CONSTRUCTION + equality tests (per the item contract). Do NOT add a
//   #[should_panic] test and do NOT drive run() to produce a CommandResponse.

// NOTE: toml/dirs/serde in Cargo.toml are unused legacy deps (dropped later). Do NOT
//   wire CommandResponse to serde (no serde derive) in this subtask.

// NOTE: if the committed tree already has CommandResponse matching the contract, DO NOT
//   re-add it. Confirm with `git diff` is empty after your session and all tests pass.
```

## Implementation Blueprint

### Data models and structure

This subtask introduces exactly **one new type** (`CommandResponse`). There are no new
structs, no constructors, no trait impls — the four derives cover it. Three variants use
inline **struct-variant** shape (named fields): `Legacy`, `Info`, `CallbackName`, `Ack`;
one is a unit variant: `Timeout`. Field order within each struct variant is fixed by the
wire layout / PRD §3 (see "What").

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: READ the current state to decide verify-vs-implement
  - READ: /home/dustin/projects/qmk_notifier/src/lib.rs.
  - CHECK: does `pub enum CommandResponse` already exist with the 5 variants, exact
          field names/types/order, and `#[derive(Debug, Clone, PartialEq, Eq)]`, placed
          after HostOs and before RunParameters? Do the 3 tests
          (test_command_response_info_construction, test_command_response_callback_name_construction,
          test_command_response_legacy_ack_timeout_construction) already exist?
  - IF YES (likely — commit 5bdbe92 is HEAD): make NO source edits; jump to Task 5
          (Validation). Confirm with `git diff --stat src/lib.rs` shows no change from
          your session.
  - IF NO (or differs): proceed to Task 2-4.
  - READ: /home/dustin/projects/qmk_notifier/plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md
          §Field Definitions + §Reply Disambiguation — confirm the reply byte layouts.
  - GOAL: know the exact anchors so edits are surgical; avoid a pointless diff.

Task 2: ADD the CommandResponse enum to src/lib.rs
  - INSERT: the full `pub enum CommandResponse { ... }` block (see What section).
  - PLACEMENT: immediately AFTER the closing `}` of the `HostOs` enum and BEFORE the
          `/// Parameters required for running QMK notifier operations` doc on
          `RunParameters`. One blank line of separation on each side.
  - ATTRIBUTES: exactly `#[derive(Debug, Clone, PartialEq, Eq)]` (NOT Copy).
  - DOC: enum-level multi-line `///` doc PLUS a `///` doc on EACH of the 5 variants.
  - NAMING: variants exactly `Legacy, Info, CallbackName, Ack, Timeout` (PascalCase).
          Fields exactly `matched`, `proto_ver`, `feature_flags`, `callback_count`,
          `board_rules_present`, `index`, `name`, `ok` (snake_case), in the order shown.

Task 3: (only if Task 2 ran) confirm run() and siblings are untouched
  - run() signature stays `pub fn run(params: RunParameters) -> Result<(), QmkError>`.
  - The 4 todo!() arms stay AS-IS (QueryInfo/QueryCallback/SetOs/ApplyHostContext).
  - RunCommand, HostOs, RunParameters, parse_cli_args unchanged.

Task 4: ADD the 3 unit tests to the existing mod tests block
  - ADD: test_command_response_info_construction,
         test_command_response_callback_name_construction,
         test_command_response_legacy_ack_timeout_construction (see What section).
  - PLACEMENT: inside the existing `#[cfg(test)] mod tests { use super::*; ... }` block;
          group the 3 CommandResponse tests together (near the S1 type-surface tests is
          fine, or at the end of the block).
  - PATTERN: use the already-present `use super::*;` — do NOT re-import.
  - NAMING: snake_case test_<thing>_<scenario>.
  - DO NOT: call run() expecting a CommandResponse (it returns ()/todo!() in S2). DO NOT
          add #[should_panic].

Task 5: VALIDATE (do not skip)
  - RUN (from /home/dustin/projects/qmk_notifier): `cargo fmt`, then `cargo build`,
          then `cargo clippy --lib`, then `cargo fmt --check`, then `cargo test --lib`.
  - EXPECT: build 0 warnings; clippy no new warnings; fmt --check exit 0; all tests pass.
  - IF E0433/E0382 ("cannot find type" / use of moved value): CallbackName test binds by
          `ref name` in the match so `named` survives the later `assert_ne!` — keep that.
  - IF E0277 ("doesn't implement Eq"): you added a non-Eq field (e.g. a float) or removed
          a derive — restore the four mandated derives.
  - SANITY: `git diff --stat src/lib.rs` — if the tree already matched, expect NO diff.
```

### Implementation Patterns & Key Details

```rust
// === PLACEMENT ANCHOR (illustrative; match exact surrounding lines) ===
//
// #[repr(u8)]
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum HostOs { ... }     // S1 output — DO NOT TOUCH
//
// // >>> INSERT CommandResponse HERE (blank line above and below) <<<
//
// /// Parameters required for running QMK notifier operations
// pub struct RunParameters { ... }


// === WHY exactly these derives ===
//   Debug       → logging / ?-formatting in the transport and QMKonnect handshake.
//   Clone       → callers may snapshot a parsed reply.
//   PartialEq/Eq→ test assertions (assert_eq!/assert_ne!) AND capability-equality checks
//                 in the handshake (P4.M2.T1.S1 compares Info across reconnects).
//   Copy (NOT)  → CallbackName owns Option<String>; String is not Copy.


// === WHY one shared Ack variant (not SetOsAck + ApplyAck) ===
//   SET_OS reply:        [0x51][0x03][ack]
//   APPLY_HOST_CONTEXT:  [0x51][0x05][ack]
//   Identical shape; the cmd echo at response[1] already tells the caller which command
//   it was. Duplicating the variant would force callers to handle two identical arms.


// === WHY Timeout is a CommandResponse, not a QmkError ===
//   A timeout is a NORMAL, expected outcome — it means "this firmware is legacy /
//   offline; stay in string-only mode." QmkError (HidReadError/NoResponseReceived) is for
//   HARD transport failures. Modeling Timeout as a CommandResponse variant lets the
//   caller's match treat it as ordinary control flow, not an error path. (PRD §8/§10.2.)


// === CallbackName match binds by REF so the value survives PartialEq assertions ===
//   let named = CommandResponse::CallbackName { index: 3, name: Some("…".into()) };
//   match named {
//       CommandResponse::CallbackName { index, ref name } => { … }   // `named` NOT moved
//       _ => panic!(...),
//   }
//   assert_ne!(named, unnamed);   // OK — `named` is still alive
```

### Integration Points

```yaml
SOURCE FILES:
  - modify (only if absent/deferring): "/home/dustin/projects/qmk_notifier/src/lib.rs ONLY"
  - add:    "pub enum CommandResponse (after HostOs, before RunParameters)"
  - add:    "3 #[test] fns inside the existing #[cfg(test)] mod tests block"

DEPENDENCIES / Cargo.toml:
  - none. No new crate deps. (Do NOT add serde derives — see gotchas.)

PUBLIC API SURFACE:
  - adds:    "qmk_notifier::CommandResponse (pub enum) and its 5 variants"
  - unchanged: "SendMessage, ListDevices, QueryInfo, QueryCallback, SetOs, ApplyHostContext
                (RunCommand); HostOs; RunParameters; parse_cli_args; run signature
                (Result<(),QmkError>); all core:: re-exports; all QmkError variants"

PARALLEL-SIBLING CONTRACT (P1.M1.T1.S1):
  - consumes: "HostOs enum + extended RunCommand (6 variants) + the 4 todo!() arms in
               run(). Must already exist in src/lib.rs. S2 must NOT redefine/modify them."

OUT-OF-SCOPE (later subtasks — do NOT implement here):
  - P1.M1.T3.S1: "parse_reply — the PRODUCER that decodes a 32-byte IN report into a
                  CommandResponse (0x51⇒typed, 0/1⇒Legacy, no-reply⇒Timeout; the
                  board_rules_present !=0⇒bool coercion; name NUL-trim⇒Option<String>)."
  - P1.M1.T3.S2: "run() return type Result<(),QmkError> → Result<CommandResponse,QmkError>;
                  replaces the 4 todo!() arms with real build_command_data → send →
                  parse_reply dispatch."

DOWNSTREAM CONSUMERS (do NOT implement now — listed for awareness):
  - P4.M2.T1.S1: "QMKonnect handshake pattern-matches CommandResponse::Info to decide
                  capability (proto_ver==2 && flags & 0x01 ⇒ QUERY_CALLBACK sweep)."
  - P4.M2.T1.S1: "CommandResponse::CallbackName builds the name→id map for rules.toml
                  validation."
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
# If error[E0277] "doesn't implement Eq": you removed a derive or added a non-Eq field.

# Lint (default clippy — no .clippy.toml exists).
cargo clippy --lib 2>&1 | tee /tmp/clippy.log
# Expected: no warnings/errors specific to CommandResponse or its tests.

# Formatting check (CI-style gate).
cargo fmt --check
# Expected: exit code 0 (no diff). If non-zero, re-run `cargo fmt`.

# Sanity: confirm no spurious diff if the tree already matched.
git diff --stat src/lib.rs
# Expected: either empty (tree already satisfied the contract) OR only the
# intended CommandResponse enum + 3 tests added.
```

### Level 2: Unit Tests (Component Validation)

```bash
cd /home/dustin/projects/qmk_notifier

# Run the 3 new tests in isolation first.
cargo test --lib test_command_response_info_construction -- --nocapture
cargo test --lib test_command_response_callback_name_construction -- --nocapture
cargo test --lib test_command_response_legacy_ack_timeout_construction -- --nocapture
# Expected: 1 passed each.

# Run the full lib test suite (lib.rs unit tests + core.rs unit tests).
cargo test --lib
# Expected: "test result: ok. <N> passed; 0 failed; 0 ignored; ...".
# Post-S2 baseline: core.rs (13) + lib.rs (S1's 4 + S2's 3 + greenfield ~6) ≈ 26. The
# exact N is not load-bearing; the gate is 0 failed.

# Sanity: confirm the existing run() integration tests + S1 type tests STILL pass.
cargo test --lib test_run_with_ -- --nocapture          # ListDevices/SendMessage/verbose
cargo test --lib test_host_os_discriminants -- --nocapture   # S1 HostOs
cargo test --lib test_run_command_ -- --nocapture        # S1 RunCommand variants
# Expected: all pass (never hit a todo!() arm — S2 tests don't call run() with typed cmds).
```

### Level 3: Integration Testing (System Validation)

```text
NOT APPLICABLE for this subtask.
CommandResponse is a pure data type — no I/O, no parsing, no run() wiring. There is no
live-hardware or CLI path to exercise until parse_reply (P1.M1.T3.S1) and the run()
return-type change (P1.M1.T3.S2) land. The construction + equality unit tests in Level 2
ARE the end-to-end type-surface verification for this task.
```

### Level 4: Creative & Domain-Specific Validation

```bash
cd /home/dustin/projects/qmk_notifier

# Confirm the type is publicly reachable and the derives are present (a clean build +
# the Level 2 PartialEq assertions are the proof):
cargo build --lib 2>&1 | grep -iE "CommandResponse|warning|Eq" || \
  echo "CommandResponse: no build diagnostics (good)"

# Confirm rustdoc renders the variant docs (Mode A documentation):
cargo doc --lib --no-deps 2>&1 | grep -i "CommandResponse" || echo "CommandResponse documented (or no diagnostics)"

# Confirm the public type is exported at the crate root (it lives in lib.rs = crate root):
cargo build --lib 2>&1 | tail -1
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1 passed: `cargo build` → zero warnings (no E0277 Eq failure).
- [ ] Level 1 passed: `cargo clippy --lib` → zero new warnings.
- [ ] Level 1 passed: `cargo fmt --check` → exit 0.
- [ ] Level 2 passed: `cargo test --lib` → all pass, 0 failed.
- [ ] The 3 new tests pass individually; S1's tests + `test_run_with_*` still pass.

### Feature Validation

- [ ] `pub enum CommandResponse` present with exactly `Legacy, Info, CallbackName, Ack,
      Timeout` in that order.
- [ ] Field shapes exact: `Legacy { matched: bool }`; `Info { proto_ver: u8,
      feature_flags: u8, callback_count: u8, board_rules_present: bool }`;
      `CallbackName { index: u8, name: Option<String> }`; `Ack { ok: bool }`; `Timeout`.
- [ ] Derives exactly `#[derive(Debug, Clone, PartialEq, Eq)]` (no `Copy`).
- [ ] Enum placed after `HostOs`, before `RunParameters`.
- [ ] Enum-level + per-variant rustdoc present (Mode A), referencing the wire contract /
      PRD and (where applicable) the cmd_id.
- [ ] `run()` signature unchanged (`Result<(), QmkError>`); the 4 `todo!()` arms unchanged.
- [ ] Only `src/lib.rs` modified (or zero files modified if the tree already matched).

### Code Quality Validation

- [ ] Follows existing enum doc style (`///` per variant/type) and derive conventions.
- [ ] New tests follow the file's `test_<thing>_<scenario>` naming + `use super::*`.
- [ ] No `#[allow(dead_code)]` / `#[non_exhaustive]` added (unnecessary).
- [ ] No serde/Display/parse logic added (out of scope — parse_reply is P1.M1.T3.S1).
- [ ] `CallbackName` test binds by `ref name` so `named` survives the `assert_ne!`.

### Documentation & Deployment

- [ ] Variants are self-documenting via `///` (Mode A — no separate docs file).
- [ ] `Info` doc encodes the QUERY_INFO reply layout; `CallbackName` doc encodes the
      immediate-NUL⇒`None` rule; `Ack` doc names both ack-style commands.
- [ ] No new environment variables or config.
- [ ] No `Cargo.toml` change (no new deps).

---

## Anti-Patterns to Avoid

- ❌ Don't add `Copy` to the derives — `CallbackName` owns an `Option<String>` and `String`
  is not `Copy` (it won't compile). The item mandates exactly `Debug, Clone, PartialEq, Eq`.
- ❌ Don't change `run()`'s return type to `CommandResponse` — that is P1.M1.T3.S2, not S2.
  In S2 `CommandResponse` is defined but `run()` still returns `Result<(), QmkError>`.
- ❌ Don't touch the 4 `todo!()` arms in `run()` — they're S1 scaffolding removed in
  P1.M1.T3.S2. S2 only adds the type.
- ❌ Don't split `Ack` into `SetOsAck`/`ApplyAck` — both replies are `[0x51][cmd_echo][ack]`,
  identical shape. One variant; the cmd echo disambiguates.
- ❌ Don't model `board_rules_present` as `u8` — it's the DECODED shape (`bool`); the
  wire-u8→bool coercion is `parse_reply`'s job (P1.M1.T3.S1).
- ❌ Don't model `name` as `String` — it's `Option<String>` (the firmware emits an immediate
  `0x00` NUL when there's no name / out-of-range index ⇒ `None`).
- ❌ Don't reorder the variants to "group typed together" — order MUST be `Legacy, Info,
  CallbackName, Ack, Timeout` (PRD §3/§7; disambiguation grouping).
- ❌ Don't reorder `Info`'s fields — order MUST be `proto_ver, feature_flags,
  callback_count, board_rules_present` (matches the wire layout offsets [2][3][4][5]).
- ❌ Don't add `#[allow(dead_code)]` — a `pub` enum that `run()` doesn't yet return is NOT
  dead code (public API surface).
- ❌ Don't add `#[non_exhaustive]` — the PRD doesn't ask for it and the crate isn't 1.0.
- ❌ Don't add serde derives or a `From<&[u8]>`/parse impl — decoding is `parse_reply`
  (P1.M1.T3.S1). S2 is type-surface only.
- ❌ Don't add a test that calls `run()` expecting a `CommandResponse` (it returns `()` /
  panics on `todo!()` in S2), and don't add a `#[should_panic]` test.
- ❌ Don't redefine/modify `HostOs`, `RunCommand`, `RunParameters`, or `parse_cli_args` —
  they're S1's contract.
- ❌ Don't re-add an identical `CommandResponse` if the committed tree already matches
  (commit `5bdbe92` is HEAD) — verify-and-validate; leave `git diff` clean.
- ❌ Don't skip `cargo fmt` / `cargo test` because "it's just an enum" — the equality
  assertions are the contract check that protects the downstream parser and the handshake.

---

**Confidence Score: 10/10** for one-pass implementation success. The deliverable is a
single fully-specified enum (verbatim body + doc comments) and three ready-to-paste tests,
placed against a precise anchor in one file, with the source-of-truth wire layouts quoted,
the explicit "do NOT touch `run()`" guard, and verified build/clippy/fmt/test commands.
The enum is almost certainly already in the committed tree (`5bdbe92`), so the realistic
path is verify-and-validate — and the PRP gives the exact contract to diff against. The
one genuine risk (adding `Copy`) is pre-empted: `CallbackName` owns `Option<String>`, so
`Copy` won't compile and the four mandated derives are the only viable set.
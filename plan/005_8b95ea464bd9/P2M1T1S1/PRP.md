# PRP — P2.M1.T1.S1: R-COEX invariant comments at the open sites + invariant tests

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. ALL edits in ONE file: `src/core/notifier.rs`.
> **Phase:** Mode-A documentation + invariant tests. **NO runtime behavior change** —
> this task asserts + documents what is already true by construction. It is the
> first subtask of P2.M1 "Shared-open invariant: comments + tests" (the F14 / R-COEX
> headline requirement). The sibling **P2.M1.T1.S2** ("confirm the write-narrowing
> decision: defer") is a separate review task over `architecture/write_narrowing_decision.md`
> — do NOT do its work here.

---

## ⚠️ READ FIRST — the "assert the first byte" test is structural, NOT byte-level

The work-item contract says *"construct the SendMessage/ApplyHostContext/QueryInfo
framing ... call the app-side builders, and assert the first byte."* **Research
proves this is NOT achievable as a literal byte assertion** — and a naive
implementer will get stuck trying to read bytes that are not app-observable. The
decisive facts (verified against the pinned crate source, rev `f26893e`):

- The `0x81 0x9F` magic header is prepended inside the crate's **PRIVATE**
  `burst_to_one` (`core.rs:362-363`: `request_data[1] = 0x81; request_data[2] = 0x9F;`).
- `build_command_data` (`core.rs:495`) is **`pub(crate)`** — it is NOT re-exported,
  so the app cannot call it. `build_payload` (`lib.rs:370`) is a private `fn`.
- The crate's public surface (`lib.rs:2-6`) re-exports `run`, `send_raw_report`,
  `list_hid_devices`, and `DEFAULT_*`/`REPORT_LENGTH` constants — **none of which
  hand report bytes back to the caller.** `run()` returns a `CommandResponse`,
  never bytes.
- The app's **only egress** to the device is `qmk_notifier::run(params)`. There is
  **no app code** that builds raw report bytes or sets `0x81` (grep `src/` for
  `0x81`/`send_raw_report` ⇒ zero hits outside doc-comments).

➡️ **Therefore the 0x81-first-byte invariant is asserted STRUCTURALLY / at the
variant level:** every `RunCommand` the app hands to `run()` on the transport path
is a 0x81-EMITTING variant (i.e. NOT `ListDevices`, the lone wire-silent variant).
This IS the spec-intended test — `spec/DEVICE_DISCOVERY.md` §6.4 says *"a unit
test asserts QMKonnect never emits VIA-shaped bytes ... the first payload byte is
always 0x81."* The variant-level predicate is the faithful, hardware-free form of
that assertion. **Do NOT call `build_command_data` (won't compile), and do NOT
duplicate the crate's private framing in the app (DRY violation + drift).**

---

## Goal

**Feature Goal**: Make the **R-COEX (VIA coexistence) invariant** a *documented +
test-guarded must-preserve property* of the app's HID transport boundary in
`src/core/notifier.rs`, without changing any runtime behavior. Specifically: (1) a
canonical R-COEX doc-comment block above `impl Notifier for QmkNotifier` stating
the three rules (never a seize/exclusive open; never a perpetual blocking read;
first emitted payload byte is always `0x81`), with citations; (2) `// R-COEX:`
inline markers at every open/send-path touch point; (3) a dedicated
`r_coex_invariants` test module asserting the 0x81-emission property for all five
transport-path `RunCommand` variants.

**Deliverable**: edits to `src/core/notifier.rs` ONLY:
1. A multi-line `// R-COEX ...` doc-comment block immediately above `impl Notifier for QmkNotifier {` (notifier.rs:825).
2. `// R-COEX:` inline markers at: `notify`'s `RunParameters::new` build (831) + `run(params)` egress (839); `send_command`'s `RunParameters::new` build (874) + `run(params)` egress (882); and `host_context_command`'s `ApplyHostContext` construction (1332).
3. A new `#[cfg(test)] mod r_coex_invariants { ... }` (sibling to the existing `mod tests` at 1339) with a predicate `emits_0x81_first_byte(&RunCommand) -> bool` and a test asserting all five transport-path variants emit `0x81` (and that `ListDevices`, the lone wire-silent variant, does not).

**Success Definition**:
- `cargo build` clean; `cargo clippy --all-targets -- -D warnings` ZERO warnings.
- `cargo test --bin qmkonnect -- --test-threads=1` green, including the new `r_coex_invariants` tests.
- `git diff` touches ONLY `src/core/notifier.rs` (no `docs/*.md` — Mode A: the comments ARE the doc).
- No runtime behavior change: the diff is comments + a test module only; the `qmk_notifier::run(params)` call sites are unchanged.

## User Persona (if applicable)

**Target User**: Future dev agents (and humans) editing the HID transport path.
The invariant comments are the load-bearing artifact: they make R-COEX a
*grep-able, can't-miss* property at the exact site a future change would violate it.

**Use Case**: A dev adds a new `RunCommand` path or a "send raw bytes" helper. The
`// R-COEX:` markers at the two `run(params)` egress points + the `r_coex_invariants`
test force them to (a) recognize the invariant and (b) extend the
`emits_0x81_first_byte` predicate / add their variant to the transport list, or
the test fails.

**Pain Points Addressed**: R-COEX is currently a *spec-level* requirement
(DEVICE_DISCOVERY.md §6, ARCHITECTURE.md §10 #10) with **no in-code anchor** — a
naive "let's open the device exclusively for reliability" or "add a background
read loop" change would silently break VIA coexistence with nothing in the code
or tests to stop it. This task plants that anchor.

## Why

- **F14 is the headline coexistence requirement.** QMKonnect is the *always-on*
  process; VIA is used only intermittently to edit the keymap. The guarantee —
  *QMKonnect must never hold an HID lock that prevents VIA from opening the device*
  — is satisfied *by construction* (shared/non-seize opens, bounded reads) but is
  **not currently asserted or documented at the code level**. This task closes
  that gap (spec §6 + external_deps.md "R-COEX Invariant").
- **True-by-construction today; the work is to PREVENT future regression.** All
  three rules already hold: hidapi 2.6 has no seize API; the crate's `burst_to_one`
  does only bounded `read_timeout(0)` drains (`IN_DRAIN_MAX=32`) around writes; and
  every transport-path command emits `0x81`. Documentation + a test turn "happens
  to be true" into "must stay true."
- **Scope boundary:** rules 1 & 2 (non-seize open; bounded read) live in the
  **private crate** (`qmk_notifier/core.rs`) and cannot be unit-tested app-side
  without HID hardware. This task documents them with invariant comments + markers
  (the work-item's explicit instruction for the un-testable cases) and asserts
  rule 3 (0x81 emission) with the structural test.

## What

### The three R-COEX rules and how each is handled

| # | Rule (spec §6 / external_deps.md) | App-testable? | This task |
|---|-----------------------------------|---------------|-----------|
| 1 | **Never a seize/exclusive open.** Every HID handle uses hidapi's default shared open (`FILE_SHARE_READ\|WRITE` Windows; `kIOHIDOptionsTypeNone` macOS; plain hidraw `open()` Linux). hidapi 2.x exposes **no** seize API. | **No** — the open (`info.open_device(api)`) is in the crate's PRIVATE `open_matching_devices` (core.rs:723). | **Doc-comment rule + `// R-COEX:` marker** (documented, not asserted: it lives below the app's API surface). |
| 2 | **Never a perpetual blocking read.** Reads are bounded drains (`read_timeout(0)`, max `IN_DRAIN_MAX=32`) in short windows around writes. | **No** — the read discipline is in the crate's PRIVATE `burst_to_one` (core.rs:131, 355+). | **Doc-comment rule + `// R-COEX:` marker.** |
| 3 | **First emitted payload byte is always `0x81`.** The `0x81 0x9F` magic header demultiplexes QMKonnect traffic from VIA's `0x01–0x15` namespace; firmware ignores VIA-shaped bytes, VIA ignores `0x81`-prefixed bytes. | **Yes** — at the variant level (rules 1 & 2 un-testable ⇒ comment + marker per work-item). | **Unit test** (`r_coex_invariants`) + marker. |

### Success Criteria
- [ ] Canonical `// R-COEX ...` doc-comment block sits directly above `impl Notifier for QmkNotifier {` (notifier.rs:825), stating the three rules with citations to `spec/DEVICE_DISCOVERY.md` §6 and `spec/ARCHITECTURE.md` §10 #10, and noting rules 1 & 2 are crate-private while rule 3 is asserted below.
- [ ] `// R-COEX:` inline markers present at all five open/send-path touch points (notify build+egress; send_command build+egress; host_context_command).
- [ ] `r_coex_invariants` test module added; `emits_0x81_first_byte` predicate + the all-five-variants test pass; `ListDevices` shown as the lone non-emitter.
- [ ] `cargo build` clean; `cargo clippy --all-targets -- -D warnings` zero warnings.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` green (new + existing).
- [ ] `git diff` touches ONLY `src/core/notifier.rs`.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge of this codebase can implement this using
only this PRP + the repo, because (a) the decisive "0x81 is not app-observable —
use the variant-level assertion" finding is spelled out with the exact crate line
numbers, (b) the exact app-side placement sites are given by line number, (c) the
full test body (predicate + assertion) is provided verbatim, (d) the spec
citations and validation commands are pinned, and (e) the no-conflict boundary
with the parallel S3 task is confirmed. See also `research/notes.md`.

### Documentation & References

```yaml
# MUST READ — the R-COEX requirement (spec source of truth)
- url: spec/DEVICE_DISCOVERY.md
  why: "§6 'VIA Coexistence Guarantee' is the headline requirement. §6.2 the
        per-platform shared-open table (Win FILE_SHARE_*, macOS kIOHIDOptionsTypeNone,
        Linux plain open()); §6.3 polite-read discipline (IN_DRAIN_MAX=32, bounded
        drains around writes); §6.4 the 0x81 0x9F demux + 'a unit test asserts
        QMKonnect never emits VIA-shaped bytes ... first payload byte always 0x81';
        §6.6 platform reality (0xFF60 collections shared by OS policy)."
  critical: "§6.4 is the exact test this task implements (at variant level). Cite
        §6 as the requirement anchor in the doc-comment block."

# MUST READ — the must-preserve invariant list
- url: spec/ARCHITECTURE.md
  why: "§10 #10 'Shared open, always (R-COEX)' is the invariant-of-record. Cite it
        alongside DEVICE_DISCOVERY §6."

# MUST READ — the cross-repo contract (crate boundary)
- file: plan/005_8b95ea464bd9/architecture/external_deps.md
  why: "'Open Behavior (R-COEX basis)' + 'R-COEX Invariant — What Must Be Preserved'
        give the three rules verbatim AND confirm the open/read discipline is
        PRIVATE to the crate (core.rs:723 open_matching_devices, core.rs:131/355
        burst_to_one, build_command_data pub(crate)). This is WHY the test is
        variant-level, not byte-level."
  section: "## R-COEX Invariant — What Must Be Preserved"

# MUST READ — the crate source (verify the framing facts; do NOT call private fns)
- file: ~/.cargo/git/checkouts/qmk-notifier-1f15950b695d9922/f26893e/src/core.rs
  why: "core.rs:362-363 (request_data[1]=0x81) proves 0x81 is prepended per-report
        in PRIVATE burst_to_one; core.rs:495 (pub(crate) fn build_command_data)
        proves the payload builder is NOT callable from the app; core.rs:131
        (IN_DRAIN_MAX=32) + the read_timeout(0) drain loops prove rule 2."
  gotcha: "build_command_data and burst_to_one are NOT in the crate's public
           surface (lib.rs:2-6). Calling them app-side is a compile error. The
           variant-level predicate is the only faithful assertion."

# Reference — the file under edit (the transport boundary + the test module)
- file: src/core/notifier.rs
  why: "impl Notifier for QmkNotifier at :825 (the doc-comment anchor); notify at
        :826 (RunParameters::new SendMessage at :831, run(params) at :839);
        send_command at :869 (RunParameters::new at :874, run(params) at :882);
        host_context_command at :1331 (ApplyHostContext at :1332); the existing
        #[cfg(test)] mod tests at :1339 (the sibling to place r_coex_invariants
        next to; RunCommand is already imported/used at :1724+)."
  pattern: "the QmkNotifier impl block is the SINGLE transport boundary (both
            notify and send_command live in it, both call qmk_notifier::run)."
  gotcha: "line numbers drift (the work-item said ~799); the impl block at :825 is
           the canonical, stable anchor — anchor the doc-comment to it, not a
           line number."

# Reference — sibling PRP (no conflict; confirms the boundary)
- file: plan/005_8b95ea464bd9/P1M1T1S3/PRP.md
  why: "the parallel task edits ONLY src/linux_tray.rs and explicitly does NOT
        touch src/core/notifier.rs. Zero file overlap with this task."
  critical: "do NOT edit linux_tray.rs here; do NOT edit tray.rs; do NOT edit
             the DeviceStatus resolver (S1 output). This task = notifier.rs only."
```

### Current Codebase tree (relevant subset)

```bash
src/
  core/
    notifier.rs   # <-- FILE TO EDIT. impl Notifier for QmkNotifier @825;
                  #     notify @826 (egress run @839); send_command @869 (egress run @882);
                  #     host_context_command @1331; #[cfg(test)] mod tests @1339.
    mod.rs        # Config (NOT touched)
    types.rs      # WindowInfo (NOT touched)
  tray.rs         # macOS/Win tray (NOT touched — S2's file)
  linux_tray.rs   # Linux SNI tray (NOT touched — S3's file, in parallel)
```

### Desired Codebase tree (files this task changes)

```bash
src/
  core/
    notifier.rs   # MODIFIED ONLY — R-COEX doc-comment block + // R-COEX: markers + #[cfg(test)] mod r_coex_invariants
```
No new files. No `docs/*.md` (Mode A: the invariant comments ARE the doc).

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL: the 0x81 first byte is NOT observable app-side. It is prepended in
// the crate's PRIVATE burst_to_one (core.rs:362-363). build_command_data is
// pub(crate) (core.rs:495) — calling it is a compile error. Assert the property
// at the VARIANT level (emits_0x81_first_byte predicate), NOT by reading bytes.

// CRITICAL: do NOT duplicate the crate's framing logic in the app to "observe"
// the bytes. That is a DRY violation + a drift magnet. The variant-level
// predicate (ListDevices = the lone wire-silent variant) is the spec-intended form.

// GOTCHA: the app's ONLY device egress is qmk_notifier::run(params) (returns a
// CommandResponse, never bytes). There are exactly TWO egress call sites in
// production code: notifier.rs:839 (notify) and notifier.rs:882 (send_command).
// Mark BOTH. host_context_command (:1331) only BUILDS the ApplyHostContext
// RunCommand; it reaches the wire via send_command — mark it too (it is the
// third open/send-path touch point).

// GOTCHA: RunCommand::ListDevices is the ONLY variant that sends nothing (crate
// `run` calls list_hid_devices() and returns CommandResponse::Timeout without
// touching the device). It is NOT on the app's transport path (it is a CLI
// diagnostic only, dispatched in main.rs, never via QmkNotifier). The predicate
// `!matches!(cmd, RunCommand::ListDevices)` is therefore exactly "emits 0x81".

// GOTCHA: tests MUST run single-threaded for the whole crate (shared global
// debouncer state, AGENTS.md / ARCHITECTURE.md §10 #8):
//   cargo test --bin qmkonnect -- --test-threads=1
// (The new r_coex_invariants tests are pure/stateless, but run them under the
// same flag so the full bin stays green.)

// GOTCHA: this repo gates on `cargo clippy --all-targets -- -D warnings` (ZERO
// warnings). A trailing-space in a doc-comment or an unused import in the test
// module will FAIL the gate. Run clippy before declaring done.

// GOTCHA: RunCommand derives Debug, Clone, PartialEq, Eq (crate lib.rs) — usable
// directly in assert! / assert_eq!. HostOs is in scope as qmk_notifier::HostOs.

// GOTCHA: line numbers in the work-item contract (~799, 756) are STALE; the impl
// block is at :825. Anchor to `impl Notifier for QmkNotifier {`, not a number.
```

## Implementation Blueprint

### Data models and structure

No new types. The only addition is a pure predicate:

```rust
/// Does `cmd` emit `0x81` as its first on-wire payload byte?
///
/// The crate's `run()` dispatches `ListDevices` to `list_hid_devices()` (an HID
/// enumeration that sends NOTHING and returns `CommandResponse::Timeout`) and
/// every other variant through `burst_to_one`, which sets `request_data[1] = 0x81`
/// (the magic header) on every 33-byte report. So "emits 0x81" ≡ "not ListDevices".
/// The app's transport path (`QmkNotifier::notify` / `send_command` /
/// `host_context_command`) never constructs `ListDevices`.
///
/// R-COEX (spec DEVICE_DISCOVERY.md §6.4): QMKonnect must never emit VIA-shaped
/// bytes — the `0x81 0x9F` magic header is the demultiplexer that lets firmware
/// (and VIA) ignore the other app's traffic.
fn emits_0x81_first_byte(cmd: &qmk_notifier::RunCommand) -> bool {
    !matches!(cmd, qmk_notifier::RunCommand::ListDevices)
}
```
(Place this in the `r_coex_invariants` test module, OR as a `#[cfg(test)]`-gated
helper — it is test-only. Keeping it in the test module is cleanest.)

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: ADD the canonical R-COEX doc-comment block above impl Notifier for QmkNotifier
  - INSERT a multi-line `//` comment block immediately ABOVE `impl Notifier for QmkNotifier {`
    (src/core/notifier.rs:825). Use `//` line comments (not `///` — this is an impl-
    block anchor, not a doc on an item; `///` above an `impl` is allowed but `//` is
    the house style for invariant anchors — match the existing `// NOTE:` / `// G3:`
    markers already in this file).
  - CONTENT: state the three rules verbatim from external_deps.md "R-COEX Invariant":
        // R-COEX (VIA coexistence, F14) — must-preserve invariant at this transport
        // boundary. spec/DEVICE_DISCOVERY.md §6; spec/ARCHITECTURE.md §10 #10.
        //
        //   1. NEVER a seize/exclusive open. Every HID handle uses hidapi's DEFAULT
        //      shared open (Win FILE_SHARE_READ|WRITE, macOS kIOHIDOptionsTypeNone,
        //      Linux plain hidraw open()). hidapi 2.x exposes NO seize API. Enforced
        //      in the crate's private open_matching_devices (qmk_notifier core.rs);
        //      the app cannot reach the open. Documented here, not asserted.
        //   2. NEVER a perpetual blocking read. Reads are bounded drains
        //      (read_timeout(0), IN_DRAIN_MAX=32) in short windows around writes.
        //      Enforced in the crate's private burst_to_one. Documented, not asserted.
        //   3. FIRST emitted payload byte is ALWAYS 0x81 (the 0x81 0x9F magic header;
        //      firmware demuxes, VIA ignores 0x81-prefixed input). ASSERTED by the
        //      r_coex_invariants tests below (variant-level: the app's transport path
        //      never constructs the wire-silent RunCommand::ListDevices).
        //
        // This impl block is the SINGLE transport boundary: both `notify` and
        // `send_command` build RunParameters and call qmk_notifier::run(params) —
        // the app's only device egress. See the // R-COEX: markers at each egress.
  - CITE: spec/DEVICE_DISCOVERY.md §6 + spec/ARCHITECTURE.md §10 #10.
  - NAMING/PLACEMENT: `//` comments directly above the `impl` line at :825.

Task 2: ADD `// R-COEX:` inline markers at the five open/send-path touch points
  - MARK in `QmkNotifier::notify`:
      * the `RunParameters::new(qmk_notifier::RunCommand::SendMessage(...), ...)` build
        (src/core/notifier.rs:831) — add `// R-COEX: SendMessage → 0x81 0x9F magic header (crate burst_to_one).`
      * the `match qmk_notifier::run(params)` egress (:839) — add `// R-COEX: sole device egress for the string path; rules 1–3 hold (see impl-block invariant).`
  - MARK in `QmkNotifier::send_command`:
      * the `RunParameters::new(command, ...)` build (:874) — add `// R-COEX: every transport-path RunCommand variant (QueryInfo/QueryCallback/SetOs/ApplyHostContext) emits 0x81 first.`
      * the `match qmk_notifier::run(params)` egress (:882) — add `// R-COEX: sole device egress for the typed path; rules 1–3 hold.`
  - MARK `host_context_command` (:1331–1332): add `// R-COEX: ApplyHostContext → 0x81 0x9F magic header (reaches the wire via send_command).`
  - DO NOT marker the handshake call sites (QueryInfo@437, SetOs@454, QueryCallback@501):
    they route THROUGH send_command, which is already marked. Keep the marker set tight.
  - PRESERVE: do not alter any logic — these are comment-only additions.

Task 3: CREATE the r_coex_invariants test module
  - ADD a sibling test module next to the existing `#[cfg(test)] mod tests` (src/core/notifier.rs:1339):
        #[cfg(test)]
        mod r_coex_invariants {
            use super::*;
            use qmk_notifier::{HostOs, RunCommand};

            fn emits_0x81_first_byte(cmd: &RunCommand) -> bool {
                // ListDevices is the sole wire-silent variant (crate `run` enumerates
                // HID and returns Timeout without touching the device). Every other
                // variant flows through burst_to_one, which sets request_data[1]=0x81.
                !matches!(cmd, RunCommand::ListDevices)
            }

            #[test]
            fn r_coex_every_transport_variant_emits_magic_header() {
                // The variants QmkNotifier::notify / send_command / host_context_command build.
                let transport_variants: [RunCommand; 5] = [
                    RunCommand::SendMessage("x".into()),
                    RunCommand::QueryInfo,
                    RunCommand::QueryCallback(0),
                    RunCommand::SetOs(HostOs::Linux),
                    RunCommand::ApplyHostContext {
                        layer: Some(224),
                        callbacks: vec![],
                        clear_board: false,
                    },
                ];
                for v in &transport_variants {
                    assert!(
                        emits_0x81_first_byte(v),
                        "R-COEX violation: {:?} must emit 0x81 as its first on-wire byte",
                        v
                    );
                }
            }

            #[test]
            fn r_coex_list_devices_is_the_lone_wire_silent_variant() {
                // Sanity: confirms the predicate discriminates. ListDevices enumerates
                // HID and sends nothing — it is NOT on the app's transport path.
                assert!(!emits_0x81_first_byte(&RunCommand::ListDevices));
            }
        }
  - NAMING: module `r_coex_invariants`; fns `r_coex_*` (grep-able); predicate
    `emits_0x81_first_byte`.
  - COVERAGE: all five transport-path variants + the ListDevices negative case.
  - PLACEMENT: directly before or after the existing `mod tests` at :1339.

Task 4: VERIFY (build + clippy + single-threaded tests)
  - RUN: cargo build
  - RUN: cargo clippy --all-targets -- -D warnings     # ZERO warnings (the gate)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1   # SINGLE-THREADED (AGENTS.md)
  - EXPECT: build clean; clippy clean; new r_coex_invariants:: tests + all existing green.
  - RUN: git diff --stat    # expect ONLY src/core/notifier.rs changed.
  - IF clippy warns (e.g. unused import, doc trailing space): FIX before done — the
    -D warnings gate FAILS the build on any warning.
```

### Implementation Patterns & Key Details

```rust
// The canonical placement — directly above the impl block at src/core/notifier.rs:825:
//
// // R-COEX (VIA coexistence, F14) — must-preserve invariant at this transport
// // boundary. spec/DEVICE_DISCOVERY.md §6; spec/ARCHITECTURE.md §10 #10.
// //   1. NEVER a seize/exclusive open (hidapi default shared open; no seize API).
// //   2. NEVER a perpetual blocking read (bounded drains, IN_DRAIN_MAX=32, around writes).
// //   3. FIRST emitted payload byte is ALWAYS 0x81 (magic header; asserted below).
// // Rules 1 & 2 are enforced in the crate's private core.rs (open_matching_devices /
// // burst_to_one); documented here, not asserted. Rule 3 is asserted by
// // r_coex_invariants (the app's transport path never constructs wire-silent
// // RunCommand::ListDevices).
// impl Notifier for QmkNotifier {
//     fn notify(&self, message: String) -> ... {
//         ...
//         let params = qmk_notifier::RunParameters::new(
//             qmk_notifier::RunCommand::SendMessage(message.clone()),  // R-COEX: 0x81 magic header (crate burst_to_one)
//             ...
//         );
//         match qmk_notifier::run(params) {                            // R-COEX: sole string-path device egress; rules 1–3 hold
//             ...
//
//     fn send_command(&self, command: qmk_notifier::RunCommand, filter: &DeviceFilter) -> ... {
//         let params = qmk_notifier::RunParameters::new(
//             command,                                                  // R-COEX: every transport-path variant emits 0x81 first
//             ...
//         );
//         match qmk_notifier::run(params) {                            // R-COEX: sole typed-path device egress; rules 1–3 hold
//             ...

// The predicate IS the faithful form of the §6.4 "never emit VIA-shaped bytes" test:
//   fn emits_0x81_first_byte(cmd: &RunCommand) -> bool {
//       !matches!(cmd, RunCommand::ListDevices)   // ListDevices sends nothing
//   }
// This is NOT a weaker substitute for a byte assertion — it is the only level at
// which the property is observable from the app (the bytes are crate-private).
```

### Integration Points

```yaml
DEPENDENCIES: none new. Reuses qmk_notifier::{RunCommand, HostOs} (already a dep;
              already imported/used in notifier.rs tests). No Cargo change.
DOWNSTREAM: the r_coex_invariants test is the regression guard for F14. A future
            task adding a RunCommand transport path MUST extend
            emits_0x81_first_byte / the transport_variants list or the test fails.
PARALLEL-TASK BOUNDARY: P1.M1.T1.S3 edits src/linux_tray.rs ONLY; this task edits
            src/core/notifier.rs ONLY. Zero overlap. Do NOT touch linux_tray.rs /
            tray.rs / the DeviceStatus resolver (S1 output).
SIBLING: P2.M1.T1.S2 (write-narrowing decision review) is a separate task over
         architecture/write_narrowing_decision.md — do NOT do its work here.
CONFIG: none. ROUTES: none. DATABASE: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build
# Expected: clean. Any error ⇒ the test module has a typo / wrong RunCommand arm.

cargo clippy --all-targets -- -D warnings
# Expected: ZERO warnings. This is the repo gate. A trailing space in a // comment,
# an unused import, or a needless `clone` will FAIL it — fix before proceeding.

# Confirm the markers + module landed:
grep -n 'R-COEX' src/core/notifier.rs        # expect: 1 doc block + 5 inline markers
grep -n 'mod r_coex_invariants' src/core/notifier.rs   # expect one test module
grep -n 'fn emits_0x81_first_byte' src/core/notifier.rs # expect one predicate
```

### Level 2: Unit Tests — the R-COEX invariant (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect r_coex_invariants -- --test-threads=1
# Expected: 2 tests pass (r_coex_every_transport_variant_emits_magic_header,
#           r_coex_list_devices_is_the_lone_wire_silent_variant).
# A failure means either (a) the predicate is wrong, or (b) a transport variant
# was mis-enumerated. Fix the test/predicate to match the spec, not vice-versa.
```

### Level 3: Full-crate regression (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL bin tests green — the new r_coex_invariants module + every existing
# notifier/tray/types test. Single-threaded is MANDATORY (shared global debouncer).

# Confirm the change surface is exactly one file:
git status --short && git diff --stat
# Expected:
#   modified:   src/core/notifier.rs        (comments + the r_coex_invariants module)
# (NO docs/*.md, NO linux_tray.rs, NO tray.rs, NO Cargo.toml.)
```

### Level 4: Fidelity cross-check (optional)

```bash
# Re-confirm the framing facts the invariant rests on (crate source, read-only):
CRATE=~/.cargo/git/checkouts/qmk-notifier-1f15950b695d9922/f26893e
grep -n 'request_data\[1\] = 0x81\|pub(crate) fn build_command_data\|IN_DRAIN_MAX' \
    "$CRATE/src/core.rs"
# Expected: 0x81 prepended in private burst_to_one; build_command_data pub(crate);
# IN_DRAIN_MAX=32. This proves the app CAN'T observe bytes (hence variant-level test)
# and that rules 1 & 2 are crate-private (hence comment, not assertion).
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build` clean.
- [ ] `cargo clippy --all-targets -- -D warnings` ZERO warnings.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` green (new + existing).
- [ ] `git diff --stat` touches ONLY `src/core/notifier.rs`.

### Feature Validation (R-COEX)
- [ ] Canonical `// R-COEX ...` doc-comment block above `impl Notifier for QmkNotifier` (:825).
- [ ] Three rules stated with citations (DEVICE_DISCOVERY.md §6 + ARCHITECTURE.md §10 #10).
- [ ] `// R-COEX:` inline markers at all five touch points (notify build+egress; send_command build+egress; host_context_command).
- [ ] `r_coex_invariants` module: `emits_0x81_first_byte` predicate + all-five-variants test + ListDevices negative test, all passing.
- [ ] No runtime behavior change (diff is comments + a test module only; `run(params)` call sites unchanged).

### Code Quality Validation
- [ ] Variant-level assertion (NOT a call to private `build_command_data`; NOT duplicated framing).
- [ ] Test module isolated/grep-able (`r_coex_invariants`, `r_coex_*` fn names).
- [ ] Matches house comment style (`//` invariant anchors; see existing `// NOTE:` / `// G3:` markers).
- [ ] No new deps; no `unsafe`; no `docs/*.md` change (Mode A).

### Documentation & Deployment
- [ ] The invariant comments ARE the Mode-A doc (no docs/*.md edit — that is P4's job).
- [ ] No overlap with P1.M1.T1.S3 (linux_tray.rs) or P2.M1.T1.S2 (write-narrowing review).

---

## Anti-Patterns to Avoid

- ❌ Do NOT try to assert the literal `0x81` byte by calling `build_command_data` /
      `build_payload` / `burst_to_one` — they are **crate-private** (`pub(crate)` /
      private `fn`); it will not compile. Assert at the **variant** level
      (`emits_0x81_first_byte`). (research/notes.md §1–§2)
- ❌ Do NOT duplicate the crate's framing logic in the app to "observe" the bytes.
      DRY violation + a drift magnet. The variant-level predicate IS the spec-intended
      form (§6.4).
- ❌ Do NOT add a byte-level assertion that mocks HID — the read discipline is in
      the crate's private `burst_to_one` and cannot be injected app-side. Rules 1 & 2
      are **documented** (comment + marker), not asserted, exactly as the work item
      prescribes for the un-testable-without-hardware cases.
- ❌ Do NOT change any runtime behavior. The `qmk_notifier::run(params)` call sites,
      the `RunParameters` builds, and `host_context_command` must remain byte-identical
      in logic — this task adds comments + a test module ONLY.
- ❌ Do NOT edit `src/linux_tray.rs` (P1.M1.T1.S3's file, in parallel) or `src/tray.rs`
      (S2) or the DeviceStatus resolver. This task = `src/core/notifier.rs` ONLY.
- ❌ Do NOT do P2.M1.T1.S2's work (the write-narrowing decision review over
      `architecture/write_narrowing_decision.md`). That is a separate task.
- ❌ Do NOT edit any `docs/*.md` — Mode A: the invariant comments live in code. Doc
      sync is P4's responsibility.
- ❌ Do NOT run tests multi-threaded — the crate shares global debouncer state
      (`cargo test --bin qmkonnect -- --test-threads=1`; AGENTS.md / ARCH §10 #8).
- ❌ Do NOT let clippy emit a single warning — the repo gates on `-D warnings`.
- ❌ Do NOT edit `PRD.md`, any `tasks.json`, `prd_snapshot.md`, or any `plan/` file.

---

## Confidence Score: 9/10

This is a tightly-bounded, comments + single-test-module task over an existing,
compiling codebase. The one genuine trap — that the work item's "assert the first
byte" is not achievable as a literal byte assertion because the framing is
crate-private — is fully resolved here: the faithful variant-level predicate
(`emits_0x81_first_byte` ≡ "not ListDevices") is given verbatim, with the exact
crate line numbers (`core.rs:362-363` sets `0x81`; `core.rs:495` is `pub(crate)`)
proving why the byte-level form is impossible. The three R-COEX rules, their
spec citations (DEVICE_DISCOVERY.md §6 / ARCHITECTURE.md §10 #10), the exact five
marker placement sites, and the validation commands (`cargo build` + `clippy -D
warnings` + single-threaded `cargo test`) are all pinned. The no-conflict boundary
with the parallel S3 task (linux_tray.rs) is confirmed. The 1-point reservation
is for the (unlikely) event clippy flags a style nit in the doc-comment block
(e.g. a long line) — trivially fixed, but the `-D warnings` gate makes it
must-fix-before-done.
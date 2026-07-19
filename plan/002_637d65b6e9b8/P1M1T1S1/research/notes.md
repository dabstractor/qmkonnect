# Research Notes — P1.M1.T1.S1 (merged): Add HostOs enum + new RunCommand variants

> Crate: `qmk_notifier` at `/home/dustin/projects/qmk_notifier`.
> This subtask is the **merger** of old plan/001's S1 (HostOs only) + S2
> (RunCommand variants). `CommandResponse` is the NEXT sibling (old S3 =
> plan/002's P1.M1.T1.S2), out of scope here.

## 1. Merged-scope derivation (why this is one subtask in plan/002)

| old plan/001 subtask | old scope | plan/002 subtask |
|---|---|---|
| P1.M1.T1.S1 | HostOs enum only | **P1.M1.T1.S1 (THIS)** |
| P1.M1.T1.S2 | RunCommand typed variants + todo!() arms | **P1.M1.T1.S1 (THIS)** |
| P1.M1.T1.S3 | CommandResponse enum | P1.M1.T1.S2 (next) |

plan/002 collapsed the two type-definition subtasks into one because they share a
single edit site (`src/lib.rs`), a single body of research (the firmware wire
contract), and HostOs is the only cross-dependency (referenced by `SetOs(HostOs)`,
which Rust resolves by name regardless of textual order). The PRP is written from
the greenfield starting point (RunCommand = SendMessage + ListDevices only).

## 2. Current committed state — IMPORTANT for the implementing agent

The crate's `git log` (in `/home/dustin/projects/qmk_notifier`) shows this work is
**already committed** in the working tree:

```
ad8a06c Add P1.M1.T1.S3 task: CommandResponse enum research and PRP   # also IMPLEMENTS CommandResponse + 3 tests
93cff89 add HostOs enum and typed RunCommand variants                  # THIS subtask's exact scope
77d1414 Add task breakdown and architecture research                  # greenfield (pre-work)
```

- `93cff89` implements **exactly this subtask's scope**: HostOs enum, 4 RunCommand
  variants, 4 `todo!()` arms in `run()`, and the 4 tests
  (`test_host_os_discriminants_match_firmware_contract`,
  `test_run_command_query_variants_construction`,
  `test_run_command_set_os_variant_construction`,
  `test_run_command_apply_host_context_construction`).
- `ad8a06c` *additionally* implements `CommandResponse` + its 3 tests (the NEXT
  sibling subtask's scope). So the current `src/lib.rs` already contains MORE than
  this subtask requires.

**Consequence for validation:** if the implementing agent runs against HEAD,
`cargo test --lib` will report **29 passing** (lib.rs 16 + core.rs 13), not 26.
The +3 over this subtask's expected 26 are the `CommandResponse` tests from the
next sibling. The PRP's Level-2 validation is therefore phrased as "all pass, 0
failed" with a note about the test-count variance, rather than pinning a single
number. If the orchestrator resets `lib.rs` to the greenfield state before handing
the task to the agent, the count will be exactly 26 (22 baseline + 4 new).

If the working tree already matches the target (likely), the agent's correct action
is: run the Validation Loop to confirm green, make no edits, and explicitly NOT add
`CommandResponse` (out of scope).

## 3. Greenfield starting point (verified via `git show 93cff89~1:src/lib.rs`)

```rust
/// Command types for the QMK notifier          // single-line doc (greenfield)
#[derive(Debug, Clone)]
pub enum RunCommand {
    SendMessage(String),
    ListDevices,
}                                                // lib.rs:14-17 in greenfield
```

`run()` matches exactly `ListDevices` + `SendMessage` (2 arms, exhaustive).
Tests at greenfield: lib.rs = 9, core.rs = 13 → **22 total**.

## 4. The compile-exhaustiveness gotcha (the central risk)

Adding 4 variants to `RunCommand` breaks `run()`'s 2-arm match → E0004. The fix is
4 explicit `todo!()` arms (sanctioned by the item description). Decision rationale:
- `todo!()` not `unimplemented!()`: idiomatic "dispatch lands later"; type `!` coerces
  to `Result<(), QmkError>`; clearer intent.
- explicit arms not `_ =>`: a wildcard silently swallows a future 7th variant and
  defeats the exhaustiveness check the compiler provides for free.
- `clippy::todo` is in the `restriction` group (allow by default) → default clippy green.

The `todo!()` arms are temporary scaffolding removed in P1.M1.T2.S2 (return-type
change) / P1.M3.T3.S1 (real dispatch).

## 5. Why existing `run()` tests stay green after the `todo!()` arms

The 3 existing `run()` integration tests only construct ListDevices/SendMessage:
- `test_run_with_list_devices_command` → ListDevices arm (unchanged)
- `test_run_with_send_message_command` → SendMessage arm (unchanged)
- `test_run_with_verbose_output` → SendMessage arm (unchanged)

None hit a `todo!()` arm, so no panic. The 4 new tests construct variants and
pattern-match the value — they do **NOT** call `run()` (typed dispatch is P1.M3.T3).
Hence no `#[should_panic]` test is added.

## 6. Derive analysis (no changes needed beyond what's specified)

- `RunCommand`: keep `#[derive(Debug, Clone)]`. All 4 new variants satisfy these
  (unit, u8, HostOs which is Debug+Clone, and Option<u8>/Vec<u8>/bool). Do NOT add
  PartialEq/Eq/Copy (RunCommand owns a String → Copy impossible; item says "match
  existing derives").
- `HostOs`: `#[repr(u8)] #[derive(Debug, Clone, Copy, PartialEq, Eq)]`. Copy is
  idiomatic/safe for a fieldless repr(u8) enum; PartialEq/Eq enable `assert_eq!`
  in the construction test and downstream.

## 7. Wire contract (source of truth)

`/home/dustin/projects/qmk_notifier/plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md`
(canonical local mirror of firmware PRD §4.6):

- Command Table cmd_ids: `0x01 QUERY_INFO`, `0x02 QUERY_CALLBACK`, `0x03 SET_OS`,
  `0x05 APPLY_HOST_CONTEXT` (`0x04` reserved for VIA).
- `os_byte` (SET_OS arg): `0 UNSURE · 1 LINUX · 2 WINDOWS · 3 MACOS · 4 IOS`
  (mirrors QMK `os_variant_t`).
- APPLY_HOST_CONTEXT request: `[layer][flags][count][id0][id1]…`
  - `layer`: host-layer # (≥224 by convention) OR `0xFF` (clear) → maps to
    `Option<u8>` (None⇒0xFF).
  - `flags` bit 0 = `clear_board`.
  - `id…`: full desired enabled set; firmware diffs (disable-before-enable); uncapped,
    may span reports → maps to `Vec<u8>`.
- Framing (for awareness only — NOT implemented here): `[0x81][0x9F][0xF0][cmd_id][args…][0x03]`,
  ETX-framed, multi-report (30 payload bytes/report). `0xF0` can never begin a real
  matched string (sanitizer allows only 0x20–0x7E), so legacy firmware ignores typed cmds.

Where the wire contract and any prose disagree, **the firmware PRD §4.6 wins**.

## 8. File boundary (forbidden elsewhere)

Only `src/lib.rs` is modified. No change to: RunParameters, parse_cli_args, core.rs,
error.rs, main.rs, Cargo.toml, README, PRD.md, any tasks.json/prd_snapshot. No new files.
`CommandResponse` is NOT created (next subtask).

## 9. QMK os_variant_t cross-check

QMK `feature_os_detection` defines `os_variant_t` with exactly `OS_UNSURE=0,
OS_LINUX=1, OS_WINDOWS=2, OS_MACOS=3, OS_IOS=4`. The HostOs discriminants are an exact
mirror. (Confirmed by firmware_wire_contract.md; external URL
https://docs.qmk.fm/#/feature_os_detection?id=os-variant-t corroborates.)

## 10. Build/test commands (verified working in this crate)

- `cargo build` — compiles (0 warnings at greenfield and at HEAD).
- `cargo test --lib` — runs lib.rs + core.rs unit tests.
- `cargo fmt` / `cargo fmt --check` — rustfmt (no project rustfmt.toml → default style).
- `cargo clippy --lib` — default clippy (no .clippy.toml).
- All run from `/home/dustin/projects/qmk_notifier`.
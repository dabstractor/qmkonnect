# Research Notes — P1.M1.T1.S2 (Add CommandResponse enum)

## Scope (single sentence)

Add a `pub enum CommandResponse` (5 variants, `#[derive(Debug, Clone, PartialEq, Eq)]`,
Mode-A rustdoc) to `src/lib.rs` of the **`qmk_notifier` crate** (repo at
`/home/dustin/projects/qmk_notifier`, NOT the qmkonnect repo). This is the **parsed
device reply** type. It defines a type only — no behavior, no parsing, no `run()`
signature change.

## CRITICAL BASELINE FINDING — the enum is ALREADY committed in the tree

`git log --oneline -- src/lib.rs` in the crate shows:

```
5bdbe92 Add CommandResponse enum for parsed device replies        ← THIS TASK, already done
93cff89 add HostOs enum and typed RunCommand variants            ← P1.M1.T1.S1, already done
660c88f made vendor and product ids optional for matching
...
```

`src/lib.rs` (HEAD) already contains, byte-for-byte matching the item contract:

- `pub enum CommandResponse { Legacy { matched: bool }, Info { proto_ver, feature_flags, callback_count, board_rules_present }, CallbackName { index, name: Option<String> }, Ack { ok: bool }, Timeout }`
- `#[derive(Debug, Clone, PartialEq, Eq)]` (exactly the four derives the item mandates)
- Placed AFTER `HostOs` and BEFORE `RunParameters` (current textual order in the file is:
  `RunCommand` → `HostOs` → `CommandResponse` → `RunParameters`)
- A multi-line enum-level rustdoc + a `///` on EVERY variant
- 3 tests in the existing `#[cfg(test)] mod tests` block:
  - `test_command_response_info_construction`
  - `test_command_response_callback_name_construction`
  - `test_command_response_legacy_ack_timeout_construction`

**Conclusion:** This PRP must be written so the implementer either (a) adds the enum if
absent, or (b) **validates the existing tree** if present and correct. The most likely
outcome for plan/002 is (b) — verify-and-validate. The PRP encodes the exact target so
the implementer can diff the tree against the contract and either confirm or fix.

The parallel sibling (P1.M1.T1.S1) is "currently being implemented" per the orchestrator,
but its PRP explicitly EXCLUDES `CommandResponse` ("next subtask — out of scope"), so S1's
work cannot conflict with S2's `CommandResponse`.

## Why `run()` is NOT touched in S2

`run()` today is `pub fn run(params: RunParameters) -> Result<(), QmkError>` with 4
`todo!()` arms for the typed variants (legacy scaffolding from S1). The plan/002 task
graph keeps the return-type change separate:

- **P1.M1.T3.S1** — "Implement response reader and parser" (`parse_reply`, the producer
  of `CommandResponse`). Consumes this enum.
- **P1.M1.T3.S2** — "Change run() signature to return `CommandResponse`" — THAT is where
  `run()` becomes `Result<CommandResponse, QmkError>` and the `todo!()` arms are replaced.

So in S2 we ONLY define the type. We must NOT change `run()`'s signature or touch the
`todo!()` arms. The enum will be unused-by-`run()` for now (that is expected and is NOT a
dead-code warning — `pub` items are part of the public API surface; `cargo build` stays
warning-free, verified in the committed tree).

## Why these exact derives (`Debug, Clone, PartialEq, Eq`) and NOT `Copy`

- Item contract: "Derive Debug, Clone, PartialEq, Eq." — mandated, not a choice.
- `CallbackName { name: Option<String> }` owns a `String`. `String: Eq` ⇒ `PartialEq`/`Eq`
  derive cleanly. `String` is **not** `Copy` ⇒ `CommandResponse` cannot be `Copy`. Do NOT
  add `Copy`.
- `Eq` is valid because all field types (`bool`, `u8`, `Option<String>`) are `Eq`
  (no interior mutability, no floats).
- `Clone` is valid because all field types are `Clone`.
- The downstream consumer (`parse_reply`, P1.M1.T3.S1) returns this from `run()`, so
  `Debug` (logging) + `Clone` (caller may copy) + `PartialEq`/`Eq` (assertions in tests,
  capability-equality checks in the handshake, P4.M2.T1.S1) are all wanted.

## Why this field shape per variant (wire contract source of truth)

`plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md` is canonical:

### §Field Definitions — QUERY_INFO response `[0x51][0x01][proto_ver][feature_flags][callback_count][board_rules_present]`
- `[2] proto_ver` u8: `1` = legacy string-only firmware; `2` = typed-command capable.
- `[3] feature_flags` u8 bitmask: `0x01` APPLY_HOST_CONTEXT supported; `0x02` callback
  registry present; `0x04` reserved (VIA-coexist).
- `[4] callback_count` u8: number of entries in firmware's host-callback registry (0 if none).
- `[5] board_rules_present` u8 (0/1): `1` iff any board map (default or OS-specific) non-empty.
  ⇒ modeled as `bool` in Rust (parse_reply, P1.M1.T3.S1, does `!= 0`).

### §Field Definitions — QUERY_CALLBACK response `[0x51][0x02][index][name bytes, NUL-padded]`
- name absent OR index out of range ⇒ `[3]=0x00` (NUL immediately).
  ⇒ modeled as `name: Option<String>` — `None` when immediate NUL / empty.

### §SET_OS / §APPLY_HOST_CONTEXT response: `[0x51][cmd_echo][ack]`, `ack==1` ⇒ applied.
- Both ack-style commands share ONE variant `Ack { ok: bool }`. The cmd echo disambiguates
  which command produced it, but the SHAPE is identical so one variant suffices. (`ok` is
  `ack == 1`; parse_reply decides this.)

### §Reply Disambiguation (table)
| `response[0]` | Interpretation |
| `0x51` | Typed reply — decode by `response[1]` (cmd echo) |
| `0` | Legacy match-bool: NOT matched |
| `1` | Legacy match-bool: matched |
| *(no reply within timeout)* | Timeout — device legacy/offline; caller stays string-only |
| *(any other value)* | Treated as non-capable device → Timeout semantics |

⇒ `Legacy { matched: bool }` (response[0] ∈ {0,1}) and `Timeout` (no reply) are the two
non-typed variants. Constants: `NOTIFY_RESPONSE_MARKER = 0x51`; firmware legacy reply is
`response[0] = match; // 0 or 1` then `raw_hid_send(response, 32)`.

## PRD §8 / §10.2 confirmation (crate PRD.md)

- PRD §3 (lines ~133-138) gives the enum verbatim, matching the wire contract.
- PRD §8 (lines ~304-314): "For a `SendMessage`, `response[0]` is the legacy match-bool
  (`0`/`1`) ⇒ `CommandResponse::Legacy { matched }`. For a typed command,
  `response[0] == 0x51` ⇒ typed reply, decoded by `response[1]`. … no reply within the
  bounded `read_timeout` ⇒ `Timeout`."
- PRD §14 invariant 6 (line ~464): "Reply parsing disambiguates `0x51` (typed) from `0`/`1`
  (legacy match-bool) from no-reply (`Timeout`)."

## QmkError interplay (no change needed — awareness only)

`error.rs` already has forward-looking variants `HidReadError(String)` and
`NoResponseReceived(String)`. The transport layer (P1.M1.T3) decides which of those
hard-error paths vs. the soft `CommandResponse::Timeout` applies. For S2 (type-only) this
is irrelevant — just noting `Timeout` is a `CommandResponse` variant, NOT a `QmkError`.

## Placement

`CommandResponse` is defined AFTER `HostOs` and BEFORE `RunParameters`. (It currently sits
between `HostOs` and `RunParameters` in the committed tree — correct.) The enum references
no other crate-internal type (it is self-contained: `u8`, `bool`, `Option<String>`), so
textual order relative to `RunCommand`/`HostOs` is not load-bearing — Rust resolves module
items by name. But keep the committed placement for a clean diff.

## Test conventions (followed by the 3 existing/required tests)

- File convention: `#[cfg(test)] mod tests { use super::*; ... }` at the bottom of `lib.rs`.
- Test naming: `test_<thing>_<scenario>` snake_case.
- Construction tests use `match` with explicit arms + `assert_eq!`; the `PartialEq`/`Eq`
  derives let the tests also assert equality (`assert_eq!(variant, Variant{...})`) and
  inequality (`assert_ne!`) across variants — this is the contract check that protects the
  downstream parser and the qmkonnect handshake.
- Tests must NOT call `run()` with any path that produces a `CommandResponse` — `run()`
  still returns `Result<(), QmkError>` with `todo!()` arms in S2. The 3 tests are pure
  construction/equality tests.

## Validation commands (verified to exist / work in this crate)

All run from `/home/dustin/projects/qmk_notifier`:
- `cargo fmt`, `cargo build`, `cargo clippy --lib`, `cargo fmt --check`, `cargo test --lib`.
- No `rustfmt.toml` / `.clippy.toml` exists ⇒ default style/lints.
- Baseline test count post-S2: 13 (core.rs) + N (lib.rs). S1 added 4, S2 adds 3 →
  expected total in lib.rs: 13 (core) + ~13 (lib incl. 4 S1 + 3 S2) ≈ 26. The exact N is
  not load-bearing; the gate is "0 failed".
- No hardware needed (type-surface only); no integration test applicable in S2.

## Files NOT to touch

`core.rs`, `error.rs`, `main.rs`, `Cargo.toml`, `README.md`, `PRD.md`, `run()` body/signature,
`RunCommand`, `HostOs`, `RunParameters`, `parse_cli_args`. Only `src/lib.rs` and ONLY the
`CommandResponse` enum + its 3 tests. (If the tree already has them correctly, make NO
source edits — just run the validation loop.)
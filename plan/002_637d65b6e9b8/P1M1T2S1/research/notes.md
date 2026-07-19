# Research Notes — P1.M1.T2.S1: typed-command payload builder + multi-report framing

Crate under edit: **`/home/dustin/projects/qmk_notifier`** (a SEPARATE repo that
QMKonnect pins by git tag). All edits land in **`src/core.rs`** only.

## 1. The framing subtlety (the one thing an implementer can get wrong)

The item description is internally contradictory and resolves it in its final
"CRITICAL" sentence. The canonical wire layout
(`plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md` §Typed-Command
Framing) settles it definitively:

```
33-byte hidapi write() buffer:  [0x00][0x81][0x9F][0xF0][cmd_id][args…][0x03]
firmware-side (report ID stripped): [0x81][0x9F][0xF0][cmd_id][args…][0x03]
                                                          ^^^ data[2] = discriminator
```

`burst_to_one` (core.rs) ALREADY hardcodes the header:
```rust
let mut request_data = [0u8; REPORT_LENGTH + 1]; // 33 bytes
request_data[1] = 0x81;
request_data[2] = 0x9F;
// ... then copies the CALLER's `data` slice into request_data[3..]
```

⇒ The `data: &[u8]` passed to `send_raw_report` is the **payload** (bytes after
`[0x81,0x9F]`). For a typed command the FIRST payload byte must be `0xF0`. So:

**`build_typed_payload` returns `[0xF0][cmd_id][args…][0x03]`** (ETX appended),
NOT `[0x81][0x9F][0xF0]…`. The caller (P1.M1.T2.S2 dispatch) hands this `Vec`
straight to `send_raw_report`, which prepends `[0x00,0x81,0x9F]` per report.

The existing `batches_for` / `burst_to_one` / device cache are reused UNCHANGED
(item confirms: "reusable as-is"). Multi-report chunking is automatic: a payload
>30 bytes just spans reports exactly like a long legacy string.

### Why NOT to change burst_to_one

The item offered "or restructure to build the full payload including header" as
an alternative. REJECTED — it would fork the send path (one header-prepending
path for strings, one for typed) and risk regressing the legacy path + the
drain/retry logic. Reusing the existing `[0x00,0x81,0x9F]`-prepending path for
both is strictly simpler and the legacy ETX+chunk semantics are identical.

## 2. dead_code lint — empirically verified (load-bearing for the allow cleanup)

Question: if I add `build_typed_payload` as `pub(crate)` + `#[allow(dead_code)]`
(its only consumer lands in P1.M1.T2.S2), and it references the command
constants `CMD_DISCRIMINATOR/CMD_QUERY_INFO/...`, can I REMOVE those constants'
existing `#[allow(dead_code)]` now — or do they still warn because their only
non-test referencer (`build_typed_payload`) is itself dead?

Tested in `/tmp/deadcode_test` with rustc 1.92.0:
- A `pub(crate) const` referenced ONLY by a `#[allow(dead_code)]`-unused
  `pub(crate) fn` does **NOT** warn in `cargo build`. The fn's body is compiled
  code, so the reference counts — even though the fn is unreachable.
- A constant referenced ONLY by `#[cfg(test)]` code (the existing comment's
  case) **DOES** warn in `cargo build` (test code isn't compiled).

⇒ **Conclusion:** build_typed_payload is a "real consumer" (per the constants'
own comment: "REMOVE each allow when its constant gains a real consumer"). So:

- REMOVE `#[allow(dead_code)]` from the 5 command constants the builder
  references: `CMD_DISCRIMINATOR`, `CMD_QUERY_INFO`, `CMD_QUERY_CALLBACK`,
  `CMD_SET_OS`, `CMD_APPLY_HOST_CONTEXT`.
- KEEP `#[allow(dead_code)]` on `RESPONSE_MARKER` and `REPLY_READ_TIMEOUT_MS`
  (consumers land in P1.M1.T3: parse_reply + the reply reader). The builder
  never references these.
- ADD `#[allow(dead_code)]` to `build_typed_payload` itself (consumer lands in
  P1.M1.T2.S2; referenced by tests only until then). REMOVE that allow in S2
  when `run()`'s typed-dispatch arm calls it.

## 3. Exact per-variant payloads (from firmware_wire_contract.md §Command Table)

| Variant                          | Payload (what `build_typed_payload` returns)            |
| -------------------------------- | ------------------------------------------------------- |
| `QueryInfo`                      | `[0xF0, 0x01, 0x03]`                                    |
| `QueryCallback(i)`               | `[0xF0, 0x02, i, 0x03]`                                 |
| `SetOs(os)`                      | `[0xF0, 0x03, os as u8, 0x03]`                          |
| `ApplyHostContext{layer,cb,clr}` | `[0xF0, 0x05, layer_byte, flags, count, id…, 0x03]`     |

APPLY_HOST_CONTEXT field encoding:
- `layer_byte` = `layer.unwrap_or(0xFF)` (`None` ⇒ 0xFF clear; `Some(n)` ⇒ n).
- `flags` = `if clear_board { 0x01 } else { 0x00 }` (bit 0 = clear_board).
- `count` = `callbacks.len() as u8` (u8 ⇒ host invariant: ≤255 ids; the
  firmware registry is itself u8-bounded so this is impossible to violate in
  practice — still worth a comment).
- `id…` = the `callbacks` Vec verbatim (`extend_from_slice`).

`SendMessage` / `ListDevices` are NOT typed commands. Exhaustive match arm
returns `Vec::new()` (inert, not a panic) — the `run()` dispatch (S2) routes
them through the legacy string path / `list_hid_devices` and never reaches here.

## 4. Sibling/dependency contracts (treat as fixed)

- **P1.M1.T1.S1 (Complete):** `RunCommand` (6 variants) + `HostOs` (`#[repr(u8)]`,
  0–4) in `src/lib.rs`. `run()` still has 4 `todo!()` arms (removed in
  P1.M1.T3.S2, NOT this task).
- **P1.M1.T1.S2 (parallel, Ready):** `CommandResponse` enum in `src/lib.rs`.
  Irrelevant to the builder (builder = request side; CommandResponse = reply side).
- **P1.M1.T2.S2 (downstream consumer):** the `run()` typed-dispatch arm will call
  `send_raw_report(&build_typed_payload(&cmd), vid, pid, page, usage, verbose)`.
  That is the ONE caller; build_typed_payload stays allow-dead until then.

## 5. Placement & scope

- File: `src/core.rs` ONLY. (Editing lib.rs risks a parallel-edit collision with
  the P1.M1.T1.S2 implementer who is concurrently in lib.rs.)
- Placement: immediately AFTER `batches_for` (and its doc), BEFORE the
  `MatchKey` struct — it's a pure payload-builder feeding the send path.
- Tests: inside the existing `#[cfg(test)] mod tests` in core.rs (which already
  has `use super::*;` and the `batches_for`/constant tests). 7 new tests.

## 6. Known cosmetic debt (NOT fixed here — out of scope)

`src/lib.rs` RunCommand::SetOs / ApplyHostContext doc comments reference a
hypothetical `build_command_data` (P1.M2.T1 — old numbering). The real function
is `build_typed_payload` (P1.M1.T2.S1). These are harmless prose forward-refs
(not compiled). Reconcile in a later doc-pass task to avoid a parallel-edit
collision with the S2 implementer; do NOT touch lib.rs in this subtask.

## 7. No external research needed

This is an internal byte-packing function against a fully-documented in-repo
wire contract. No third-party library docs are required (hidapi usage is
unchanged — the builder produces a `Vec<u8>`, no HIDAPI calls).
# Research Notes — P1.M1.T2.S2: wire typed commands into `run()` dispatch + send path

Crate under edit: **`/home/dustin/projects/qmk_notifier`** (separate repo,
git-tagged, pinned by QMKonnect). This subtask is the **ONE caller** of
`build_typed_payload` (P1.M1.T2.S1). S1 lands FIRST; S2 starts after it.

## 0. What already exists when S2 starts (T1 + S1 are landed)

- **`run()` already returns `Result<CommandResponse, QmkError>`** (T1.S2 changed
  the signature ahead of the plan's T3.S2 — verified by reading `src/lib.rs`).
  So S2 does NOT touch the return type; only the 4 `todo!()` arms.
- The 4 typed arms are literally `todo!("typed dispatch lands in P1.M3.T3.S1")`.
- `build_typed_payload(cmd: &RunCommand) -> Vec<u8>` exists in `src/core.rs` with
  `#[allow(dead_code)]` on it (S1 added it; its doc says "remove it in S2 once
  `run()` calls this"). **S2 must remove that allow** (it's the documented handoff).
- `send_raw_report(data, vid, pid, page, usage, verbose) -> Result<(), QmkError>`
  already handles: MatchKey + device cache (`ensure_cache`), multi-report
  burst-write (`burst_to_one`, prepends `[0x00,0x81,0x9F]` per report), bounded
  IN-drain (discards replies), retry-on-total-failure. **UNCHANGED by S2.**
- `main.rs` (the binary CLI) only ever constructs `SendMessage`/`ListDevices`
  via `parse_cli_args`. Typed commands are NOT reachable from the CLI yet (that
  is P5.M1). ⇒ **main.rs is untouched.**

## 1. The borrow-check crux: `match params.command` vs `match &params.command`

`build_typed_payload` takes `&RunCommand` (the WHOLE command), but the current
`match params.command { … }` MOVES `params.command` into the match, so the typed
arms cannot borrow it back. Two ways out:

- **(chosen)** Switch to `match &params.command { … }`. Then each typed arm can
  pass `&params.command` straight to `build_typed_payload`, and `params.vendor_id`
  / `product_id` / `usage_page` / `usage` / `verbose` (all `Copy`) stay accessible
  because `params` is only borrowed, not moved. **Cost: the `SendMessage(message)`
  arm sees `message: &String` instead of `String` — but the arm only calls
  `message.as_bytes()` (auto-deref), so ZERO body changes are needed.** This is
  the smallest, cleanest change.
- (rejected) Keep the move-match and RECONSTRUCT `RunCommand::ApplyHostContext{
  layer, callbacks, clear_board }` from the bound pieces to pass to the builder.
  Clunky, especially for the struct variant.

Verified by reading the `SendMessage` arm: it uses only `message.as_bytes()` and
`params.*` Copy fields — nothing needs an owned `String`. So the borrow-match is
a one-token change with no body fallout.

### 1a. The `core::` qualification trap (E0425) — discovered mid-research

`build_typed_payload` is `pub(crate)` in `core.rs` (private `mod core;` in
lib.rs) but is **NOT** in the `pub use core::{ … }` re-export at the crate root
(only `send_raw_report` / `list_hid_devices` / `parse_hex_or_decimal` / the
`DEFAULT_*` consts are re-exported). So from `run()` in `lib.rs` it MUST be
called as **`core::build_typed_payload(&params.command)`** — a BARE
`build_typed_payload(…)` fails with **E0425** "cannot find function
`build_typed_payload` in this scope" (confirmed: there is no
`use core::build_typed_payload;` import). Contrast `send_raw_report`, which IS
re-exported and so is callable bare (as the existing `SendMessage` arm does).

This was caught by observing the concurrent in-progress implementation, which
correctly used `core::build_typed_payload(&params.command)`. It is the SECOND
compile trap (after the move-vs-borrow match) and is called out in the PRP's
gotchas, Level-1 diagnostics, and anti-patterns. Do NOT "fix" it by making the
fn `pub` or adding a re-export — qualifying the call is the minimal intended
change.

## 2. Collapse the 4 typed arms into ONE or-pattern arm (no per-variant helper)

The build+send is IDENTICAL across all 4 typed variants (build payload →
`send_raw_report` → return placeholder). So a single collapsed or-pattern arm is
the DRY, idiomatic choice:

```rust
RunCommand::QueryInfo
| RunCommand::QueryCallback(_)
| RunCommand::SetOs(_)
| RunCommand::ApplyHostContext { .. } => {
    let payload = core::build_typed_payload(&params.command);
    send_raw_report(&payload, params.vendor_id, params.product_id,
                     params.usage_page, params.usage, params.verbose)?;
    Ok(CommandResponse::Timeout)   // placeholder
}
```

Why NOT 4 separate arms / a `send_typed` helper:
- 4 separate arms = 4× duplicated build+send (a reviewer would flag it; nothing
  differs between them at this stage).
- A `send_typed(&params)` helper adds a layer of indirection for 2 lines of code
  used once.
- When P1.M1.T3.S2 wires per-variant **reply parsing** (which IS where variants
  diverge), THAT task splits this arm — trivial, and only when divergence is real
  (YAGNI). If `parse_reply` ends up generic (decode by `reply[1]` cmd-echo), the
  arm may never need splitting at all.

Per-variant documentation already lives on the `RunCommand` enum variants (S1),
so collapsing the arm loses no documentation.

## 3. Placeholder return value: `CommandResponse::Timeout` (and why it's safe)

After a successful typed send, the reply is **drained and discarded** by
`burst_to_one` (the IN-drain loop). So there is no reply to parse yet ⇒ S2 must
return *some* `CommandResponse`. Choices:

- `CommandResponse::Timeout` — semantically "no reply captured", which is
  literally true (it was drained). Chosen.
- per-variant (`Ack{ok:true}` for SET_OS/APPLY_HOST_CONTEXT, etc.) — would imply
  parsing we did NOT do. Rejected.

**Safety of the placeholder:** no consumer reads it at this stage. Typed
commands are unreachable from the CLI (`main.rs`), and the only programmatic
caller (QMKonnect P4 handshake/host-context) lands AFTER P1.M1.T3.S2 replaces
the placeholder with real parsing. So the wrong-but-documented `Timeout` cannot
leak into a live pipeline. Stated explicitly in run()'s rustdoc.

## 4. The S1→S2 `#[allow(dead_code)]` handoff (the ONE core.rs edit)

S1 stages `build_typed_payload` as `#[allow(dead_code)] pub(crate) fn …` (only
tests referenced it during S1). The moment S2's `run()` arm calls it, the fn has
a real consumer ⇒ the allow is now a no-op. S1's own doc says "remove it in S2
once `run()` calls this." So S2:

1. Deletes the `#[allow(dead_code)]` line directly above `build_typed_payload`.
2. Rewrites the function's trailing doc sentence (currently: "Until then this is
   referenced only by tests, hence `#[allow(dead_code)]` — remove it in S2…") to
   state the real consumer (`run()`).

Empirically verified by S1 (rustc 1.92.0): a `pub(crate) const`/`fn` referenced
by compiled non-`cfg(test)` code does not warn, so removing the allow is correct
once `run()` calls it. **Keep** `#[allow(dead_code)]` on `RESPONSE_MARKER` and
`REPLY_READ_TIMEOUT_MS` (consumers are P1.M1.T3 — `parse_reply` + reply reader).

## 5. Test strategy: deterministic, no-hardware, proves dispatch (not todo!())

`run()` calls `send_raw_report`, which does real HID I/O — there is no mock seam.
The existing run-level tests (`test_run_with_send_message_command`,
`test_run_with_list_devices_command`) accept Ok-or-various-Errs (hardware-
tolerant). For the typed arms the sharpest signal is: **did the arm dispatch to
`send_raw_report`, or did it `todo!()`-panic?**

Trick: use a **bogus VID/PID** (`Some(0xDEAD)`, `Some(0xBEEF)`) so the device
filter (`vendor_id.is_none_or(|v| dev_vid == v)`) matches NOTHING on any machine
— even one with a real QMK keyboard plugged in. ⇒ deterministic
`Err(QmkError::DeviceNotFound { .. })`. A `todo!()` would have panicked and
failed the test, so the assertion proves dispatch wiring without touching real
hardware or being flaky.

4 tests (one per typed variant: QueryInfo, QueryCallback, SetOs,
ApplyHostContext) — each is a regression guard that the arm was actually wired
(not left as a `todo!()`). They're near-identical but cheap, and each guards a
distinct variant.

`DeviceNotFound` is a struct variant: `QmkError::DeviceNotFound { vendor_id,
product_id, usage_page, usage }` (confirmed in `src/error.rs`). The tests match
`Err(QmkError::DeviceNotFound { .. })`.

## 6. Scope / files touched

| File | Change |
| --- | --- |
| `src/lib.rs` | `match params.command` → `match &params.command`; replace the 4 `todo!()` arms with ONE collapsed typed-dispatch arm (build + send + `Timeout` placeholder); update `run()`'s `///` doc (typed variants no longer "stubbed with todo!()"; fix `P1.M3`→`P1.M1` numbering drift); add 4 dispatch tests. |
| `src/core.rs` | Remove `#[allow(dead_code)]` from `build_typed_payload`; rewrite its trailing doc sentence. (The documented S1→S2 handoff — 2 lines.) |
| `main.rs`, `error.rs`, `Cargo.toml`, wire docs | UNCHANGED. |

## 7. Baseline numbers (verified this session)

- `cargo test --lib` → **37 passed, 0 failed** (pre-S1; S2 starts after S1 lands,
  so the real baseline will be 37 + S1's 7 = 44; after S2's 4 → 48). Exact count
  not load-bearing; gate is **0 failed**.
- `cargo fmt --check` → exit 0.
- `cargo build` → 0 warnings (pre-S1).
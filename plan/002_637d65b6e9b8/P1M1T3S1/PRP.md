# PRP — P1.M1.T3.S1: Response reader & parser (0x51 typed vs 0/1 legacy vs timeout)

> **Crate:** `qmk_notifier` (v0.2.1) at `/home/dustin/projects/qmk_notifier`
> (separate repo, git-tagged, pinned by QMKonnect per PRD §7/§4). Work in
> `/home/dustin/projects/qmk_notifier`.
> **Files:** `src/core.rs` (PRIMARY & ONLY). Do NOT touch `lib.rs`/`run()` (S2 owns it).
> **Scope line:** Add the PARSE layer above the (already-existing, already-tested)
> `parse_reply`: `classify_response(buf, expected_cmd)` (pure, testable — the
> expected_cmd echo guard; **the pipeline-critical deliverable S2 applies to
> `send_raw_report`'s captured reply**) + `read_typed_response(interface, expected_cmd,
> verbose)` (the item's explicit standalone read+parse primitive). Remove `parse_reply`'s
> now-redundant `#[allow(dead_code)]`. **Do NOT add `burst_and_read_one`** — it is
> redundant with the committed `burst_to_one`-captures refactor. Wire-up into `run()`
> is **out of scope** (P1.M1.T3.S2).

> ⚠️ **BASELINE RECONCILIATION — read before implementing.** This task was specified
> against an older baseline (the item assumed `burst_to_one` *drains* and
> `send_raw_report` returns `Result<(),_>`). That baseline is **gone**. Committed
> `71248cd` already took the item's *other* option (b): `burst_to_one` now **captures**
> the first reply (`-> (bool, Option<Vec<u8>>)`), `send_raw_report` now returns
> `Result<Option<Vec<u8>>, _>`, and `try_send_once` threads the first device's reply.
> `run()` ignores those bytes (returns the `Timeout` placeholder). **Consequence:** the
> genuinely-needed deliverable is `classify_response` (the parse that S2 applies to the
> already-captured bytes); `read_typed_response` is the item's requested standalone
> primitive; and `burst_and_read_one` is **redundant** (the capture already happens in
> `burst_to_one`). See "Baseline Reconciliation" below.

---

## Goal

**Feature Goal**: Give the crate the PARSE step that turns the firmware's captured
32-byte IN reply into a `CommandResponse` — closing the gap between "send_raw_report
captured the bytes" (committed `71248cd`) and "we have a typed reply to act on" (P4
handshake). The parser half (`parse_reply`/`parse_typed_reply`/`parse_callback_name`)
**already exists and is tested**; this task adds the command-echo validation guard on
top of it (`classify_response`) plus the item's standalone read+parse primitive
(`read_typed_response`), both staged `#[allow(dead_code)]` for the `run()` wiring
(P1.M1.T3.S2).

**Deliverable**: `src/core.rs` with TWO new functions, ONE allow removed, and ~5 new
unit tests:
1. `classify_response(buf: &[u8], expected_cmd: u8) -> CommandResponse` — **pure**,
   the testable, pipeline-critical core. Adds the NEW logic: a typed reply
   (`buf[0]==0x51`) whose echo (`buf[1]`) ≠ `expected_cmd` ⇒ `Timeout` (stale-reply
   guard); everything else delegates to the existing `parse_reply`. **This is what
   `run()` (S2) applies to `send_raw_report`'s captured `Option<Vec<u8>>`.**
2. `read_typed_response(interface, expected_cmd, verbose) -> Result<CommandResponse, QmkError>`
   — the item's explicit standalone primitive: bounded single-report read
   (`read_timeout(buf, REPLY_READ_TIMEOUT_MS)`) + `classify_response`. `Ok(0)`/timeout
   ⇒ `Ok(Timeout)` (**not** an error); HID error ⇒ `Err(HidReadError)`. allow-dead.
3. `#[allow(dead_code)]` **removed** from `parse_reply` (now called by allow-dead
   `classify_response` — one-hop, proven safe) + its doc updated.
4. **`burst_and_read_one` MUST NOT be present** — it is redundant with the committed
   capturing `burst_to_one`. If a parallel attempt added it, **remove it**.
5. ~5 new `classify_response_*` tests in `src/core.rs`'s `#[cfg(test)] mod tests`.

**Success Definition**: `cargo build` compiles with **zero warnings**; `cargo clippy
--lib` introduces none; `cargo fmt --check` exits 0; `cargo test --lib` passes with all
existing tests + the new `classify_response_*` tests green; `burst_to_one`/
`send_raw_report`/`try_send_once`/`parse_reply` body/`parse_typed_reply`/
`parse_callback_name`/`build_typed_payload`/`run()`/`lib.rs`/`error.rs`/`Cargo.toml`
all **unchanged** except the documented `parse_reply` allow-removal + doc tweak; **no
`burst_and_read_one` function exists** in `src/core.rs`.

## Baseline Reconciliation (the committed architecture — authoritative)

The CURRENT committed state of `src/core.rs` (verify with
`grep -n "fn burst_to_one\|fn send_raw_report\|fn try_send_once" src/core.rs`):

```rust
// burst_to_one: writes the burst, CAPTURES the first reply (bounded read), drains surplus.
fn burst_to_one(interface: &HidDevice, data: &[u8], batch_count: usize, verbose: bool)
    -> (bool, Option<Vec<u8>>);   // (write_success, captured_first_reply)

// try_send_once threads the reply: "first successful device wins".
fn try_send_once(key, data, batch_count, verbose)
    -> Result<(SendOutcome, Option<Vec<u8>>), QmkError>;

// send_raw_report returns the captured reply bytes (or None on timeout).
pub fn send_raw_report(data, vendor_id, product_id, usage_page, usage, verbose)
    -> Result<Option<Vec<u8>>, QmkError>;
```

So the **READ already happens inside `burst_to_one`**; `send_raw_report` hands the raw
bytes to its caller. `run()` currently calls `send_raw_report`, **ignores** the
`Option<Vec<u8>>`, and returns the `Timeout` placeholder (its doc at `lib.rs:~332` is
stale — "send_raw_report STILL returns Result<(), _>" — but fixing `lib.rs` is **S2's**
job, NOT this task).

**This reframes the item's two options:**
- Item (a) `read_typed_response(device, expected_cmd, verbose)` — KEEP (item's explicit
  contract). It's the standalone read+parse primitive. NOTE: the MAIN send flow already
  read inside `burst_to_one`, so S2 will parse via `send_raw_report`'s captured bytes +
  `classify_response` rather than re-reading through `read_typed_response`. Keep
  `read_typed_response` allow-dead (the item requested it; S2 uses it only if it chooses
  a direct-read path).
- Item (b) "modify `burst_to_one` **OR** add `burst_and_read_one`" — the **first**
  alternative ("modify burst_to_one") is **already DONE** (committed `71248cd`). The
  second alternative (`burst_and_read_one`) is therefore **REDUNDANT — do not add it**.
  (If present from a parallel attempt: remove it; wiring it into `run()` would
  double-read the kernel IN buffer.)

**The genuinely-new, pipeline-critical deliverable is `classify_response`** — the PARSE
that S2 applies to `send_raw_report`'s already-captured `Option<Vec<u8>>`.

## User Persona (if applicable)

**Target User**: The downstream implementer of **P1.M1.T3.S2** (wire reply parsing into
`run()` — replaces the `Ok(CommandResponse::Timeout)` placeholder) and ultimately the
QMKonnect **P4.M2.T1.S1** handshake (`QUERY_INFO` → `QUERY_CALLBACK` sweep → name→id map).

**Use Case**: P4 handshake calls `run(QueryInfo)` → (S2) the typed arm calls
`send_raw_report(&payload, …)` → `Ok(Some([0x51,0x01,proto_ver,feature_flags,
callback_count,board_rules_present]))` → `classify_response(&bytes, CMD_QUERY_INFO)`
validates echo==0x01 and `parse_reply` decodes `CommandResponse::Info{…}` → run() returns
it → handshake checks `proto_ver==2 && flags & 0x01`. A legacy device sends no typed
reply → `send_raw_report` returns `Ok(None)` → S2 maps `None` ⇒ `CommandResponse::Timeout`
→ handshake stays string-only.

## Why

- This is the **PARSE half** of the M1.T3 task. The parser (`parse_reply` family) landed
  earlier and is fully tested; this task adds the command-echo validation the parser alone
  can't enforce (`classify_response`) and the item's standalone reader
  (`read_typed_response`).
- It is **purely additive in `core.rs`** (two functions + one allow removed + tests).
  `burst_to_one`/`send_raw_report`/`try_send_once`/`parse_reply` body are **untouched**.
  `run()` is **untouched** (wiring is S2).
- It follows the **S1→S2 staging pattern** established by `build_typed_payload`
  (P1.M1.T2.S1 staged allow-dead; P1.M1.T2.S2 removed the allow when `run()` called it).
  Here `classify_response`/`read_typed_response` are staged allow-dead; S2 wires
  `classify_response` into `run()` and lifts its allow.

## What

All edits are in `/home/dustin/projects/qmk_notifier/src/core.rs`. **The file is under
concurrent edit** — locate every target by its **signature / doc text** (stable), NOT by
line number. Run this first to see what already exists:
```bash
grep -n "fn classify_response\|fn read_typed_response\|fn burst_and_read_one\|fn parse_reply\|fn parse_callback_name" src/core.rs
```
- If `classify_response`/`read_typed_response` already match the spec below and the build
  is clean, the parallel attempt may have already done the work — make only the DELTA
  edits (ensure `burst_and_read_one` is absent, `parse_reply`'s allow removed, tests present).
- If `burst_and_read_one` is present → **remove it** (redundant).

### Change 1 — Add `classify_response` (pure, testable) after `parse_callback_name`

Locate `parse_callback_name` (signature `fn parse_callback_name(bytes: &[u8]) -> Option<String>`).
Insert `classify_response` immediately AFTER its closing `}` and BEFORE the
`/// Match parameters a cached handle set was opened for.` doc on `MatchKey`:

```rust
/// Classify a captured reply, validating the typed command-echo against
/// `expected_cmd`.
///
/// The read-side counterpart to [`parse_reply`]: it adds ONE guard the parser alone
/// can't enforce — that a typed reply (`buf[0] == RESPONSE_MARKER`) echoes back the
/// command we actually sent (`buf[1] == expected_cmd`). A mismatch means a stale
/// reply is sitting in the IN buffer (e.g. a leftover from a previous command), so
/// we defensively treat it as [`CommandResponse::Timeout`] (the device is
/// "non-capable for this command right now") rather than mis-decoding it into the
/// wrong [`CommandResponse`] variant.
///
/// Logic (canonical layout: `firmware_wire_contract.md` §Reply Disambiguation):
/// - `buf` empty / `buf[0]` unknown ⇒ [`parse_reply`] (returns `Timeout`).
/// - `buf[0] ∈ {0,1}` ⇒ [`parse_reply`] (returns `Legacy`).
/// - `buf[0] == RESPONSE_MARKER`:
///     - `buf[1] == expected_cmd` ⇒ [`parse_reply`] (typed decode of the right
///       variant — Info / CallbackName / Ack).
///     - `buf[1] != expected_cmd` ⇒ [`CommandResponse::Timeout`] (stale echo).
///
/// Pure (no I/O) — the unit-testable core. **This is what the `run()` typed dispatch
/// (P1.M1.T3.S2) applies to [`send_raw_report`]'s captured `Option<Vec<u8>>`:**
/// `reply.map_or(CommandResponse::Timeout, |b| classify_response(&b, expected_cmd))`.
///
/// Consumer: `run()` via the typed dispatch (P1.M1.T3.S2); also
/// [`read_typed_response`] (this task).
#[allow(dead_code)]
pub(crate) fn classify_response(buf: &[u8], expected_cmd: u8) -> crate::CommandResponse {
    use crate::CommandResponse;
    if buf.first() == Some(&RESPONSE_MARKER) {
        let echo = buf.get(1).copied().unwrap_or(0);
        if echo != expected_cmd {
            // Stale or mismatched typed reply (echo ≠ command we sent). The IN
            // buffer holds a reply from a different command; treat as non-capable
            // for THIS command rather than mis-decoding into the wrong variant.
            return CommandResponse::Timeout;
        }
    }
    parse_reply(buf)
}
```

### Change 2 — Add `read_typed_response` (item contract, standalone) after `burst_to_one`

Locate `burst_to_one` (signature `fn burst_to_one(interface: &HidDevice, data: &[u8], batch_count: usize, verbose: bool) -> (bool, Option<Vec<u8>>)`). Insert `read_typed_response`
immediately AFTER its closing `}` and BEFORE the `/// Number of reports needed to carry`
doc on `batches_for`:

```rust
/// Read ONE typed reply from `interface`, then classify it.
///
/// The item's standalone read+parse primitive for the typed-command path (PRD §10.2).
/// It blocks up to [`REPLY_READ_TIMEOUT_MS`] for a single 32-byte IN report (the
/// firmware sends exactly one reply per report, `RAW_REPORT_SIZE = 32`), then:
/// - `Ok(n)` with `n > 0` ⇒ [`classify_response`] decodes it (typed if
///   `buf[0] == 0x51` & echo matches `expected_cmd`; legacy `0`/`1`; else Timeout).
/// - `Ok(0)` (poll timed out — no reply) ⇒ `Ok(CommandResponse::Timeout)`. This is
///   **not an error**: a non-capable (legacy) device never replies to a typed
///   command, and the caller treats Timeout as "stay in string-only mode" (PRD §8).
/// - `Err` ⇒ `Err(QmkError::HidReadError)` (a real HID transport failure).
///
/// `expected_cmd` is the command-ID byte we sent (e.g. [`CMD_QUERY_INFO`]); it guards
/// against decoding a stale reply left in the IN buffer. The 33-byte buffer matches
/// [`burst_to_one`]'s sizing; QMK raw HID uses report ID 0, so `read_timeout` returns
/// report DATA at `buf[0]` (no report-ID prefix on read), i.e. `buf[0] == 0x51` for a
/// typed reply.
///
/// **Note on the main send flow:** [`send_raw_report`] ALREADY captures the first
/// reply inside [`burst_to_one`] (committed `71248cd`) and returns it as
/// `Option<Vec<u8>>`. The `run()` typed dispatch (P1.M1.T3.S2) therefore parses via
/// `send_raw_report(...)? + classify_response`, NOT by re-reading through this
/// function (which would double-drain the kernel IN buffer). This function is the
/// item-requested standalone primitive for callers holding a handle without going
/// through [`send_raw_report`]; it is `#[allow(dead_code)]` until such a caller exists.
#[allow(dead_code)]
fn read_typed_response(
    interface: &HidDevice,
    expected_cmd: u8,
    verbose: bool,
) -> Result<crate::CommandResponse, QmkError> {
    use crate::CommandResponse;
    let mut buf = [0u8; REPORT_LENGTH + 1]; // 33 bytes — matches burst_to_one
    match interface.read_timeout(&mut buf, REPLY_READ_TIMEOUT_MS) {
        Ok(n) if n > 0 => {
            if verbose {
                println!("Read {} typed-reply byte(s): {:?}", n, &buf[..n]);
            }
            Ok(classify_response(&buf[..n], expected_cmd))
        }
        Ok(_) => {
            // Ok(0): no data within REPLY_READ_TIMEOUT_MS (poll timed out). NOT an
            // error — a legacy/non-capable device simply doesn't reply to typed
            // commands. The caller decides (PRD §10.2 ⇒ string-only fallback).
            if verbose {
                println!(
                    "No typed reply within {} ms (timeout).",
                    REPLY_READ_TIMEOUT_MS
                );
            }
            Ok(CommandResponse::Timeout)
        }
        Err(e) => {
            if verbose {
                println!("Error reading typed reply: {}", e);
            }
            Err(QmkError::HidReadError(e.to_string()))
        }
    }
}
```

### Change 3 — Remove `parse_reply`'s `#[allow(dead_code)]` + update its doc

Locate `parse_reply` (signature `pub(crate) fn parse_reply(response: &[u8]) -> crate::CommandResponse`).
It currently has `#[allow(dead_code)]` directly above it and a trailing doc sentence
saying it's "referenced only by tests" / "remove it in P1.M3.T3". FIND that attribute +
trailing doc sentence and:

- **DELETE** the `#[allow(dead_code)]` line above `parse_reply`. (Now safe: `classify_response` (allow-dead) calls it — one-hop, proven by the `build_typed_payload`→5-constants precedent documented in the module comment. **Verify** with `cargo build` after all edits: zero "never used" for `parse_reply`. If one unexpectedly appears, re-add the allow — but the precedent says it won't.)
- **REPLACE** the trailing doc sentence with:

```rust
/// Consumer: [`classify_response`] (P1.M1.T3.S1), which the `run()` typed dispatch
/// (P1.M1.T3.S2) applies to [`send_raw_report`]'s captured reply. `#[allow(dead_code)]`
/// was removed once `classify_response` (allow-dead) became its caller — a const/fn
/// referenced by an allow-dead fn's body does NOT warn (same rule that dropped the
/// command constants' allows once `build_typed_payload` referenced them).
```

(Keep the rest of `parse_reply`'s rustdoc and its body UNCHANGED.)

### Change 4 — Ensure NO `burst_and_read_one` exists

Locate any `fn burst_and_read_one`. If present (from a parallel attempt using an older
draft of this task), **DELETE the entire function** (signature, doc, body). It is
redundant with the committed capturing `burst_to_one`: `burst_to_one` already writes +
captures the first reply + drains surplus (`-> (bool, Option<Vec<u8>>)`), and
`send_raw_report` already returns those bytes. A `burst_and_read_one` would re-read the
kernel IN buffer (double-drain) if ever wired in. **Do not add it; remove it if present.**
Confirm with:
```bash
grep -n "burst_and_read_one" src/core.rs   # MUST print nothing
```

### Change 5 — Add ~5 `classify_response` tests to the existing `mod tests`

Append these inside the existing `#[cfg(test)] mod tests { use super::*; … use crate::{CommandResponse, HostOs, RunCommand}; … }` block, before its closing `}`:

```rust
    #[test]
    fn classify_response_typed_matching_echo_decodes() {
        // QUERY_INFO reply ([0x51][0x01][proto][flags][count][board]) with
        // expected_cmd = CMD_QUERY_INFO ⇒ typed decode into Info{..}.
        let response = [0x51, 0x01, 2, 0x03, 5, 1];
        assert_eq!(
            classify_response(&response, CMD_QUERY_INFO),
            CommandResponse::Info {
                proto_ver: 2,
                feature_flags: 0x03,
                callback_count: 5,
                board_rules_present: true,
            }
        );
    }

    #[test]
    fn classify_response_typed_mismatched_echo_is_timeout() {
        // The NEW guard: buf[0]==0x51 but buf[1] != expected_cmd ⇒ Timeout.
        // A QUERY_CALLBACK reply (echo 0x02) must NOT decode when we sent QUERY_INFO
        // (0x01) — it's a stale reply from a different command in the IN buffer.
        let response = [0x51, 0x02, 3, b'V', b'i', b'm', 0x00];
        assert_eq!(
            classify_response(&response, CMD_QUERY_INFO),
            CommandResponse::Timeout,
            "a typed reply echoing cmd 0x02 must NOT decode when we expected 0x01"
        );
    }

    #[test]
    fn classify_response_legacy_delegates_to_parse_reply() {
        // Legacy 0/1 replies: buf[0] != 0x51, so expected_cmd is IRRELEVANT —
        // delegate to parse_reply. (A legacy device ignores the typed bytes and
        // walks them as a no-match string, replying with its match-bool.)
        assert_eq!(
            classify_response(&[1], CMD_QUERY_INFO),
            CommandResponse::Legacy { matched: true }
        );
        assert_eq!(
            classify_response(&[0], CMD_SET_OS),
            CommandResponse::Legacy { matched: false }
        );
    }

    #[test]
    fn classify_response_empty_and_unknown_marker_are_timeout() {
        // Empty slice / unknown marker ⇒ Timeout (delegates to parse_reply's arms).
        assert_eq!(classify_response(&[], CMD_QUERY_INFO), CommandResponse::Timeout);
        assert_eq!(
            classify_response(&[0x42], CMD_QUERY_CALLBACK),
            CommandResponse::Timeout,
            "unknown marker ⇒ Timeout (parse_reply's `_ => Timeout` arm)"
        );
    }

    #[test]
    fn classify_response_ack_variants_require_matching_echo() {
        // SET_OS (0x03) and APPLY_HOST_CONTEXT (0x05) share the [0x51][echo][ack]
        // shape; each decodes ONLY when expected_cmd matches its echo.
        assert_eq!(
            classify_response(&[0x51, 0x03, 1], CMD_SET_OS),
            CommandResponse::Ack { ok: true }
        );
        assert_eq!(
            classify_response(&[0x51, 0x05, 0], CMD_APPLY_HOST_CONTEXT),
            CommandResponse::Ack { ok: false }
        );
        // Cross-mismatch: a 0x05 reply expected as 0x03 ⇒ Timeout (stale guard).
        assert_eq!(
            classify_response(&[0x51, 0x05, 1], CMD_SET_OS),
            CommandResponse::Timeout
        );
    }
```

> `classify_response`, the `CMD_*` consts, and `CommandResponse` are all in scope via
> the test module's existing `use super::*;` + `use crate::{CommandResponse, …};`. No
> new imports needed.

### Success Criteria

- [ ] `classify_response(buf, expected_cmd)` exists, is `pub(crate)`, pure, and: typed
      reply with `buf[1]==expected_cmd` ⇒ delegates to `parse_reply`; typed reply with
      `buf[1]!=expected_cmd` ⇒ `Timeout`; legacy `0/1` ⇒ `Legacy`; empty/unknown ⇒ `Timeout`.
- [ ] `read_typed_response(interface, expected_cmd, verbose)` exists, staged
      `#[allow(dead_code)]`, returns `Ok(classify_response(..))` on `Ok(n>0)`,
      `Ok(Timeout)` on `Ok(0)`, `Err(HidReadError)` on `Err`.
- [ ] **NO `burst_and_read_one` function exists** in `src/core.rs`.
- [ ] `parse_reply`'s `#[allow(dead_code)]` removed; its doc updated.
- [ ] ~5 `classify_response_*` tests added; all pass.
- [ ] `burst_to_one`/`send_raw_report`/`try_send_once`/`parse_reply` body/`run()`/
      `lib.rs`/`error.rs`/`Cargo.toml` unchanged except the documented `parse_reply`
      allow-removal + doc.
- [ ] `cargo build` → zero warnings (no "never used" for `parse_reply`).

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed to
> implement this successfully?"_ — **Yes.** The committed baseline (burst_to_one captures,
> send_raw_report returns Option<Vec<u8>>) is documented above; the exact code for the two
> new functions + 5 tests is verbatim; the parse_reply allow-removal is justified by a
> proven in-codebase precedent; the burst_and_read_one exclusion is explained; and
> anchor-resilient location instructions handle the concurrent-edit moving target.

### Documentation & References

```yaml
# MUST READ — the file under edit (PRIMARY & ONLY target).
- file: /home/dustin/projects/qmk_notifier/src/core.rs
  why: "(a) Contains parse_reply/parse_typed_reply/parse_callback_name — the PARSER,
        ALREADY IMPLEMENTED & TESTED (do NOT touch their bodies or tests). This task
        adds classify_response (after parse_callback_name) + read_typed_response
        (after burst_to_one). (b) burst_to_one NOW CAPTURES (committed 71248cd) — the
        baseline this task aligns to; do NOT add burst_and_read_one (redundant).
        (c) parse_reply loses its allow here. (d) RESPONSE_MARKER/CMD_* are the consts
        classify_response uses."
  critical: "parse_reply ALREADY EXISTS and is tested — do NOT re-implement parsing.
             classify_response DELEGATES to parse_reply (adds only the expected_cmd
             echo guard). burst_and_read_one MUST NOT exist (redundant with the
             capturing burst_to_one)."

# MUST READ — the wire contract (canonical reply byte layouts).
- file: /home/dustin/projects/qmk_notifier/plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md
  why: "§Reply Disambiguation: response[0]==0x51 ⇒ typed (decode by echo at [1]); 0/1
        ⇒ legacy; no reply ⇒ timeout. §Field Definitions: the per-command reply layouts
        parse_reply already decodes. §Constants: RESPONSE_MARKER=0x51, NOTIFY_CMD_* ids."
  section: "Reply Disambiguation, Field Definitions, Constants"
  critical: "The expected_cmd guard maps 1:1 to §Reply Disambiguation. Firmware is NOT
             yet implemented (§Firmware Implementation Status) — typed commands time
             out against current firmware, which is the designed fallback (Timeout)."

# MUST READ — the previous subtask's PRP (what run() looks like now + the S1→S2 pattern).
- file: /home/dustin/projects/qmkonnect/plan/002_637d65b6e9b8/P1M1T2S2/PRP.md
  why: "P1.M1.T2.S2 replaced run()'s todo!() typed arms with build_typed_payload →
        send_raw_report → Ok(CommandResponse::Timeout) PLACEHOLDER. The committed
        refactor went further: send_raw_report now returns Option<Vec<u8>>. THIS task
        (S1) does NOT touch run(); it provides classify_response (the parse S2 applies
        to those bytes) + read_typed_response (item primitive)."
  critical: "Do NOT wire classify_response/read_typed_response into run() in S1 — that
             is P1.M1.T3.S2. S1 only adds the core.rs primitives."

# REFERENCE — the build_typed_payload S1→S2 staging precedent (allow-dead pattern).
- file: /home/dustin/projects/qmkonnect/plan/002_637d65b6e9b8/P1M1T2S1/PRP.md
  why: "The EXACT staging pattern this task reuses: stage a pub(crate) fn as
        #[allow(dead_code)]; the consumer lands in S2. Also the proof that a const/fn
        referenced by an allow-dead fn's body does NOT warn — the basis for removing
        parse_reply's allow once classify_response (allow-dead) calls it."

# REFERENCE — hidapi read_timeout API.
- url: https://docs.rs/hidapi/latest/hidapi/struct.HidDevice.html#method.read_timeout
  why: "read_timeout(&mut data, timeout_ms) -> Result<usize, HidError>. Ok(n>0)=read n
        bytes; Ok(0)=timed out (BLOCKS up to timeout_ms when >0); Err=HID error. Same
        semantics the committed burst_to_one capture relies on."
  critical: "On a report-ID-0 interface (QMK raw HID), read returns report DATA at
             buf[0] (no report-ID prefix on READ). So buf[0]==0x51; parse_reply is correct."

# REFERENCE — research notes for THIS subtask (baseline reconciliation + decisions).
- docfile: plan/002_637d65b6e9b8/P1M1T3S1/research/notes.md
  why: "The baseline-change analysis (committed burst_to_one-captures), the deliverable
        reconciliation (why classify_response is core, read_typed_response is the item
        primitive, burst_and_read_one is excluded), the dead_code staging, and the
        moving-target verification approach."
```

### Current Codebase tree (crate root `/home/dustin/projects/qmk_notifier`)

```bash
qmk_notifier/
├── Cargo.toml          # version="0.2.1" — DO NOT TOUCH.
├── src
│   ├── main.rs         # binary entrypoint — DO NOT TOUCH.
│   ├── error.rs        # QmkError (HidReadError/SendReportError exist) — DO NOT TOUCH.
│   ├── lib.rs          # run() ignores send_raw_report's Option<Vec<u8>> (Timeout placeholder). DO NOT TOUCH (S2 owns run()).
│   └── core.rs         # <-- PRIMARY & ONLY EDIT: +classify_response, +read_typed_response;
│                       #     -parse_reply's allow; -burst_and_read_one if present; +5 tests.
│                       #     burst_to_one (captures)/send_raw_report (->Option<Vec<u8>>)/
│                       #     try_send_once/parse_reply body/build_typed_payload — UNCHANGED.
└── plan/001_b92a9b2b603f/architecture/{firmware_wire_contract,external_deps}.md
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL: the COMMITTED baseline has burst_to_one CAPTURING the reply (-> (bool,
//   Option<Vec<u8>>)) and send_raw_report returning Result<Option<Vec<u8>>, _>. This
//   task was specced against the OLD baseline (burst_to_one drains, send_raw_report
//   returns ()). ALIGN TO THE COMMITTED BASELINE: classify_response parses the
//   captured bytes; read_typed_response is the standalone primitive; burst_and_read_one
//   is REDUNDANT — do NOT add it.

// CRITICAL: burst_and_read_one MUST NOT exist in src/core.rs. The committed burst_to_one
//   already writes + captures + drains; send_raw_report returns the bytes. A
//   burst_and_read_one would re-read the kernel IN buffer (double-drain). If a parallel
//   attempt added it, DELETE it.

// CRITICAL: parse_reply ALREADY EXISTS and is tested. Do NOT re-implement/move it.
//   classify_response DELEGATES to it (only adds the expected_cmd echo guard on the
//   0x51 path). parse_reply's OWN tests stay green untouched.

// CRITICAL: removing parse_reply's #[allow(dead_code)] is safe — PROVEN one-hop
//   precedent: classify_response (allow-dead) calls parse_reply; the build_typed_payload
//   → 5-constants pattern (documented in core.rs's module comment) shows a const/fn
//   referenced by an allow-dead fn's body does NOT warn. VERIFY with cargo build.

// CRITICAL: Ok(0) from read_timeout is NOT an error — "poll timed out, no data".
//   read_typed_response returns Ok(CommandResponse::Timeout) on Ok(0), NOT
//   Err(NoResponseReceived). The item: "not an error — the caller decides".

// GOTCHA: the file is under CONCURRENT EDIT. Locate targets by SIGNATURE/doc text,
//   not line number. Run `grep -n "fn classify_response|fn read_typed_response|fn
//   burst_and_read_one|fn parse_reply|fn parse_callback_name" src/core.rs` first; make
//   only the delta edits needed if the parallel attempt already added the functions.

// GOTCHA: read_timeout returns report DATA at buf[0] for report-ID-0 (QMK), so
//   buf[0]==0x51 for a typed reply. Do NOT strip a leading report-ID byte. (Contrast
//   WRITE, which prefixes request_data[0]=0x00 — known hidapi asymmetry, not a bug.)

// GOTCHA: read_typed_response/burst_to_one are hardware-bound (no unit test, like the
//   existing send_raw_report/burst_to_one). Only classify_response is pure/testable.

// NOTE: REPLY_READ_TIMEOUT_MS stays 1000ms (use BY NAME; don't hardcode). Already
//   de-allowed + consumed by burst_to_one. Tuning is P4's job.

// NOTE: do NOT touch run() in lib.rs (replacing the Timeout placeholder with
//   send_raw_report+classify_response is P1.M1.T3.S2). The stale lib.rs doc
//   ("send_raw_report STILL returns Result<(),_>") is S2's to fix.
```

## Implementation Blueprint

### Data models and structure

No new types. S1 layers functions over EXISTING types: `HidDevice` (hidapi),
`CommandResponse` (lib.rs), `QmkError` (error.rs). Pipeline:
`send_raw_report` (captures `Option<Vec<u8>>`) → **`classify_response`** (parses) →
`CommandResponse`. `read_typed_response` is the standalone read+parse alternative.

### Implementation Tasks (ordered by dependencies — anchor-resilient)

```yaml
Task 1: INVENTORY current state (the file is a moving target)
  - RUN: grep -n "fn classify_response\|fn read_typed_response\|fn burst_and_read_one\|
          fn parse_reply\|fn parse_callback_name\|fn burst_to_one\|fn send_raw_report" src/core.rs
  - DETERMINE which of Change 1/2/3/4 are already done (parallel attempt) vs. needed.
  - CONFIRM: burst_to_one returns (bool, Option<Vec<u8>>) and send_raw_report returns
          Result<Option<Vec<u8>>, _> (committed baseline). If NOT, the baseline differs
          from this PRP — STOP and reconcile.

Task 2: ADD classify_response after parse_callback_name (Change 1)
  - INSERT (verbatim in "What") between parse_callback_name's closing `}` and the
          `/// Match parameters` doc. pub(crate), #[allow(dead_code)], pure, delegates
          to parse_reply. (If already present & matching, skip.)

Task 3: ADD read_typed_response after burst_to_one (Change 2)
  - INSERT (verbatim in "What") between burst_to_one's closing `}` and the
          `/// Number of reports` doc. #[allow(dead_code)]; uses REPLY_READ_TIMEOUT_MS
          + classify_response. (If already present & matching, skip.)

Task 4: REMOVE parse_reply's #[allow(dead_code)] + update its doc (Change 3)
  - DELETE the allow line above parse_reply; REPLACE its trailing doc sentence (exact
          text in "What"). Body + tests UNCHANGED. Verify with cargo build (Change 6).

Task 5: REMOVE burst_and_read_one if present (Change 4)
  - RUN: grep -n "burst_and_read_one" src/core.rs. If ANY hit: DELETE the entire fn
          (signature, doc, body). It is redundant with the capturing burst_to_one.
          Confirm zero hits afterward.

Task 6: ADD the 5 classify_response tests to mod tests (Change 5)
  - APPEND (verbatim in "What") before mod tests' closing `}`. (If already present &
          passing, skip.)

Task 7: VALIDATE (do not skip)
  - RUN: cargo fmt && cargo build && cargo clippy --lib && cargo fmt --check && cargo test --lib
  - EXPECT: build ZERO warnings (NO "never used" for parse_reply — Change 4 safe per
          precedent). clippy clean. fmt --check exit 0. All tests pass (existing +
          classify_response_*).
  - IF "never used: parse_reply": Change 4's allow-removal was premature — re-add it.
          (Per the build_typed_payload precedent it should be clean.)
  - SANITY: git diff --stat shows ONLY src/core.rs changed.
```

### Integration Points

```yaml
SOURCE FILES:
  - modify (ONLY): "/home/dustin/projects/qmk_notifier/src/core.rs"
      - +classify_response (after parse_callback_name)
      - +read_typed_response (after burst_to_one)
      - -parse_reply's #[allow(dead_code)] (+ doc)
      - -burst_and_read_one if present
      - +5 classify_response_* tests in mod tests

DEPENDENCIES / Cargo.toml: none new (hidapi already present).

PUBLIC API SURFACE: UNCHANGED. classify_response/read_typed_response are pub(crate) or
  private in the private `mod core` (NOT re-exported). run()'s signature unchanged.

CONSUMES (treat as fixed):
  - P1.M1.T1.S2 (Complete): CommandResponse enum.
  - P1.M1.T2.S1 (Complete): build_typed_payload + CMD_* consts + RESPONSE_MARKER.
  - Committed 71248cd: burst_to_one captures; send_raw_report -> Option<Vec<u8>>;
    try_send_once threads first_reply. REPLY_READ_TIMEOUT_MS de-allowed.
  - Pre-existing parse_reply/parse_typed_reply/parse_callback_name (parser, tested).

DOWNSTREAM CONSUMER (NOT this task):
  - P1.M1.T3.S2: replace run()'s typed-arm Timeout placeholder with:
        let reply = send_raw_report(&payload, …)?;            // Option<Vec<u8>>
        let expected = expected_cmd_for(&params.command);      // QueryInfo⇒0x01, …
        Ok(reply.map_or(CommandResponse::Timeout, |b| classify_response(&b, expected)))
    Also fixes the stale lib.rs doc ("send_raw_report STILL returns Result<(),_>").
    Removes classify_response's allow. read_typed_response's allow stays unless S2 uses it.
  - P4.M2.T1.S1: handshake — QUERY_INFO → (Info{proto_ver==2,flags&0x01}) →
    QUERY_CALLBACK sweep → name→id map.

OUT-OF-SCOPE (later subtasks):
  - P1.M1.T3.S2: wire classify_response into run() (touches lib.rs; fixes stale doc).
  - P1.M1.T4.S1: bump crate version to 0.3.0 + tag.
```

## Validation Loop

> All commands run from `/home/dustin/projects/qmk_notifier`.

### Level 1: Syntax & Style

```bash
cd /home/dustin/projects/qmk_notifier
cargo fmt
cargo build 2>&1 | tee /tmp/build.log   # MUST be zero warnings
#   - "never used: parse_reply" ⇒ Change 4 premature; re-add its allow (but precedent says clean).
#   - "never used: read_typed_response/classify_response" ⇒ missing their #[allow(dead_code)].
cargo clippy --lib 2>&1 | tee /tmp/clippy.log   # no new warnings
cargo fmt --check   # exit 0
git diff --stat     # only src/core.rs
```

### Level 2: Unit Tests

```bash
cd /home/dustin/projects/qmk_notifier
cargo test --lib classify_response_ -- --nocapture   # 5 new tests pass
cargo test --lib parse_reply_ -- --nocapture          # existing parse_reply tests UNCHANGED
cargo test --lib                                       # all pass, 0 failed
```

### Level 3: Integration Testing

```text
PARTIALLY APPLICABLE. read_typed_response does real HID I/O — a full round-trip needs
QMK firmware with typed-command support (P1.M2 — NOT implemented; typed commands time
out, the designed fallback). The 5 classify_response tests (Level 2) ARE the
verification of the pure decode+guard logic (every path: typed-match, typed-mismatch⇒
Timeout, legacy 0/1, empty/unknown⇒Timeout, ack both ways). read_typed_response is
compile-verified only (same bar as the hardware-bound burst_to_one/send_raw_report).
Live round-trip deferred to S2 (run() wiring) + P1.M2 (firmware).
```

### Level 4: Creative & Domain-Specific Validation

```bash
cd /home/dustin/projects/qmk_notifier
cargo doc --lib --no-deps 2>&1 | grep -iE "warning|error" || echo "docs clean (good)"
# parse_reply has NO allow above it; classify_response + read_typed_response each have one:
grep -nB1 "fn parse_reply\|fn read_typed_response\|fn classify_response" src/core.rs
# NO burst_and_read_one anywhere:
grep -n "burst_and_read_one" src/core.rs || echo "burst_and_read_one absent (good)"
# burst_to_one still captures (committed baseline, UNCHANGED):
grep -nA1 "fn burst_to_one" src/core.rs   # -> (bool, Option<Vec<u8>>)
cargo build 2>&1 | grep -iE "never used|warning" || echo "zero dead-code warnings (good)"
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build` → zero warnings (no "never used" for `parse_reply`).
- [ ] `cargo clippy --lib` → zero new warnings; `cargo fmt --check` → exit 0.
- [ ] `cargo test --lib` → all pass (5 new `classify_response_*` + existing).

### Feature Validation
- [ ] `classify_response` exists, pure, `pub(crate)`: typed+matching-echo ⇒ delegates to
      `parse_reply`; typed+mismatched-echo ⇒ `Timeout`; legacy 0/1 ⇒ `Legacy`; empty/unknown ⇒ `Timeout`.
- [ ] `read_typed_response` exists: `Ok(n>0)`⇒`Ok(classify_response)`, `Ok(0)`⇒`Ok(Timeout)`,
      `Err`⇒`Err(HidReadError)`. 33-byte buffer, `REPLY_READ_TIMEOUT_MS`.
- [ ] `parse_reply`'s `#[allow(dead_code)]` removed; doc updated.
- [ ] **NO `burst_and_read_one`** in `src/core.rs`.
- [ ] `burst_to_one`/`send_raw_report`/`try_send_once`/`parse_reply` body/`run()`/`lib.rs`/
      `error.rs`/`Cargo.toml` unchanged except the documented `parse_reply` allow-removal + doc.
- [ ] Only `src/core.rs` modified.

### Code Quality Validation
- [ ] `classify_response` delegates to `parse_reply` (no parsing duplication); only NEW
      logic is the expected_cmd echo guard.
- [ ] `Ok(0)` (timeout) is `Ok(Timeout)`, NOT an error — matches the item's contract.
- [ ] `read_typed_response`/`classify_response` carry `#[allow(dead_code)]` (staged for S2).
- [ ] New tests follow the block's existing style (`use super::*;`, `assert_eq!` on `CommandResponse`).

### Documentation & Deployment
- [ ] Rustdoc (Mode A) on `classify_response`/`read_typed_response` covers the parse
      semantics, the timeout-not-an-error contract, the report-ID-0 buffer layout, the
      baseline note (send_raw_report captures), and the S2 forward-ref.
- [ ] `parse_reply` doc updated with the real consumer chain.

---

## Anti-Patterns to Avoid

- ❌ Don't add `burst_and_read_one`. The committed `71248cd` already took the item's
  *other* option (modify `burst_to_one` to capture). `burst_and_read_one` is redundant
  (it would re-read the kernel IN buffer the capturing `burst_to_one` already drained).
  If present from a parallel attempt, DELETE it.
- ❌ Don't re-implement `parse_reply` or move/renumber its tests. It EXISTS and is tested.
  `classify_response` DELEGATES to it; the only new logic is the expected_cmd echo guard.
- ❌ Don't assume the old baseline (`burst_to_one` drains, `send_raw_report` returns `()`).
  Verify the committed baseline first (`grep -n "fn burst_to_one\|fn send_raw_report"`):
  burst_to_one returns `(bool, Option<Vec<u8>>)`, send_raw_report returns
  `Result<Option<Vec<u8>>, _>`. Align to THIS.
- ❌ Don't wire `classify_response`/`read_typed_response` into `run()`. That's S2
  (P1.M1.T3.S2), which also fixes the stale `lib.rs` doc. S1 only adds core.rs primitives.
- ❌ Don't return `Err(NoResponseReceived)` on a read timeout. `Ok(0)` is "poll timed out,
  no reply" — a NORMAL outcome for a legacy device. Return `Ok(CommandResponse::Timeout)`
  ("not an error — the caller decides").
- ❌ Don't strip a leading report-ID byte from the read buffer. QMK raw HID uses report ID
  0; `read_timeout` returns report DATA at `buf[0]` (`buf[0]==0x51` for a typed reply). The
  WRITE path's `0x00` prefix is a known hidapi asymmetry — don't "align" them.
- ❌ Don't hardcode the timeout (100/1000). Use `REPLY_READ_TIMEOUT_MS` by name; tuning is P4.
- ❌ Don't try to unit-test `read_typed_response` (can't construct a `HidDevice` in a unit
  test). Test `classify_response` (the pure layer); verify `read_typed_response` compiles.
- ❌ Don't anchor edits to line numbers — the file is under concurrent edit. Locate every
  target by its signature/doc text; run the inventory grep first and make only delta edits.
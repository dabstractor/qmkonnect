# PRP — P1.M1.T3.S1: Response reader & parser (0x51 typed vs 0/1 legacy vs timeout)

> **Crate:** `qmk_notifier` (v0.2.1) at `/home/dustin/projects/qmk_notifier`
> (separate repo, git-tagged, pinned by QMKonnect per PRD §7/§4). Work in
> `/home/dustin/projects/qmk_notifier`.
> **Files:** `src/core.rs` (PRIMARY & ONLY). No other file touched.
> **Scope line:** Add the READ layer above the (already-existing, already-tested)
> `parse_reply`: a pure `classify_response` (expected_cmd echo guard), an I/O
> `read_typed_response` (bounded single-report read + classify), and a
> `burst_and_read_one` (write-burst + read-one, the typed counterpart to
> `burst_to_one`). Drop the now-redundant `#[allow(dead_code)]` on
> `REPLY_READ_TIMEOUT_MS`. Wire-up into `run()` is **out of scope** (P1.M1.T3.S2).

---

## Goal

**Feature Goal**: Give the crate the ability to **READ** (not drain-and-discard)
the firmware's 32-byte IN reply after a typed-command burst, and decode it into a
`CommandResponse` — closing the gap between "we sent the bytes" (P1.M1.T2.S2, done
in parallel) and "we have a typed reply to act on" (P4 handshake). The parser
half (`parse_reply`/`parse_typed_reply`/`parse_callback_name`) **already exists
and is tested**; this task adds the reader + the command-echo validation guard on
top of it, staged `#[allow(dead_code)]` for the `run()` wiring (P1.M1.T3.S2).

**Deliverable**: `src/core.rs` with THREE new functions, ONE allow removed, ONE
allow-bearing comment corrected, and ~5 new unit tests:
1. `classify_response(buf: &[u8], expected_cmd: u8) -> CommandResponse` — **pure**,
   the testable core. Adds the NEW logic: a typed reply (`buf[0]==0x51`) whose echo
   (`buf[1]`) ≠ `expected_cmd` ⇒ `Timeout` (stale-reply guard); everything else
   delegates to the existing `parse_reply`.
2. `read_typed_response(interface, expected_cmd, verbose) -> Result<CommandResponse, QmkError>`
   — bounded single-report read (`read_timeout(buf, REPLY_READ_TIMEOUT_MS)`) +
   `classify_response`. `Ok(0)`/timeout ⇒ `Ok(Timeout)` (**not** an error); HID
   error ⇒ `Err(HidReadError)`.
3. `burst_and_read_one(interface, data, batch_count, expected_cmd, verbose) -> Result<CommandResponse, QmkError>`
   — byte-for-byte the SAME write loop as `burst_to_one`, then calls
   `read_typed_response` (reads ONE reply instead of draining all). Write failure ⇒
   `Err(SendReportError)`.
4. `#[allow(dead_code)]` **removed** from `REPLY_READ_TIMEOUT_MS` (now referenced by
   allow-dead `read_typed_response` — one-hop, proven safe by the `build_typed_payload`→
   5-constants precedent documented in the code).
5. The stale "RESPONSE_MARKER and REPLY_READ_TIMEOUT_MS still carry
   `#[allow(dead_code)]`" comment corrected.
6. ~5 new `classify_response_*` tests in `src/core.rs`'s `#[cfg(test)] mod tests`
   (the pure echo-guard is the only new *testable* surface; the two I/O functions
   are hardware-bound, like `burst_to_one`/`send_raw_report`, and have no unit test).

**Success Definition**: `cargo build` compiles with **zero warnings** (incl. no
"never used" for `REPLY_READ_TIMEOUT_MS` — its allow is gone because
`read_typed_response` references it); `cargo clippy --lib` introduces none;
`cargo fmt --check` exits 0; `cargo test --lib` passes with all 62+ existing tests
+ the new `classify_response_*` tests green; `parse_reply`/`parse_typed_reply`/
`parse_callback_name`/`burst_to_one`/`send_raw_report`/`batches_for`/`build_typed_payload`/
`run()`/`lib.rs`/`error.rs`/`Cargo.toml` all **unchanged** except the documented
`REPLY_READ_TIMEOUT_MS` allow-removal + comment + the parse_reply doc tweak.

## User Persona (if applicable)

**Target User**: The downstream implementer of **P1.M1.T3.S2** (wire reply parsing
into `run()` — replaces the `Ok(CommandResponse::Timeout)` placeholder with a real
typed reply) and ultimately the QMKonnect **P4.M2.T1.S1** handshake
(`QUERY_INFO` → `QUERY_CALLBACK` sweep → name→id map, pattern-matching on
`Info`/`Ack`/`Timeout`).

**Use Case**: P4 handshake calls `run(QueryInfo)` → (S2) the typed arm burst-writes
`[0xF0,0x01,0x03]` via `burst_and_read_one` → the firmware replies
`[0x51,0x01,proto_ver,feature_flags,callback_count,board_rules_present]` →
`read_typed_response` reads it → `classify_response` validates echo==0x01 and
`parse_reply` decodes `CommandResponse::Info{…}` → run() returns it → the handshake
checks `proto_ver==2 && flags & 0x01`. A legacy (non-capable) device sends no typed
reply → `read_typed_response` returns `Ok(Timeout)` → handshake stays string-only.

**User Journey**: Today (P1.M1.T2.S2) typed bytes hit the wire but the reply is
drained & discarded (`Ok(Timeout)` placeholder). After THIS task, the read+classify
primitives EXIST (staged). After S2, `run()` calls them end-to-end. After P4, the
handshake consumes the `CommandResponse`.

**Pain Points Addressed**: Removes the "we can send but can't hear back" gap. The
`expected_cmd` echo guard defends against a stale reply left in the IN buffer from a
prior command being mis-decoded into the wrong `CommandResponse` variant.

## Why

- This is the **READ half** of the M1.T3 "Response Parsing & run() Return Type
  Change" task. The parser (`parse_reply` family) landed earlier (with the
  `CommandResponse` enum, P1.M1.T1.S2) and is fully tested; this task adds the I/O
  reader that feeds it real bytes, plus the command-echo validation the parser alone
  can't enforce.
- It is **purely additive in `core.rs`**: three new functions + one allow removed +
  one comment fixed. `burst_to_one` is **untouched** (the legacy `SendMessage` drain
  path is byte-for-byte preserved — backward compat, as the item demands). `run()` is
  **untouched** (wiring is S2).
- It follows the **exact S1→S2 staging pattern** established by `build_typed_payload`
  (P1.M1.T2.S1 staged it `#[allow(dead_code)]`; P1.M1.T2.S2 removed the allow when
  `run()` called it). Here `read_typed_response`/`burst_and_read_one`/
  `classify_response` are staged allow-dead; S2 wires them into `run()` and lifts all
  remaining allows.

## What

All edits are in `/home/dustin/projects/qmk_notifier/src/core.rs`.

### Change 1 — Correct the stale `#[allow(dead_code)]` summary comment (lines ~14-25)

The module-level comment currently claims "Only RESPONSE_MARKER and
REPLY_READ_TIMEOUT_MS still carry `#[allow(dead_code)]`" — but RESPONSE_MARKER has
**no** allow (grep-confirmed: it's referenced by `parse_reply`), and after this task
REPLY_READ_TIMEOUT_MS loses its allow too. FIND this exact comment block:

```rust
// The 5 command constants (CMD_DISCRIMINATOR, CMD_QUERY_INFO, CMD_QUERY_CALLBACK,
// CMD_SET_OS, CMD_APPLY_HOST_CONTEXT) now have a real consumer:
// `build_typed_payload` (P1.M1.T2.S1) references them in compiled code, so they
// no longer need an `#[allow(dead_code)]` (verified: a const referenced by an
// allow-dead fn's body does NOT warn). Only RESPONSE_MARKER and
// REPLY_READ_TIMEOUT_MS still carry `#[allow(dead_code)]` — their consumers land
// in P1.M1.T3 (parse_reply + the reply reader).
```

REPLACE WITH:

```rust
// The 5 command constants (CMD_DISCRIMINATOR, CMD_QUERY_INFO, CMD_QUERY_CALLBACK,
// CMD_SET_OS, CMD_APPLY_HOST_CONTEXT) now have a real consumer:
// `build_typed_payload` (P1.M1.T2.S1) references them in compiled code, so they
// no longer need an `#[allow(dead_code)]` (verified: a const referenced by an
// allow-dead fn's body does NOT warn). RESPONSE_MARKER is referenced by
// `parse_reply`; REPLY_READ_TIMEOUT_MS by `read_typed_response` (P1.M1.T3.S1) —
// so neither carries an allow either. The remaining allow-dead items are the
// read/parse FUNCTIONS themselves (parse_reply, classify_response,
// read_typed_response, burst_and_read_one), whose consumer is `run()`
// (P1.M1.T3.S2); they drop their allows when run() goes live.
```

### Change 2 — Remove `REPLY_READ_TIMEOUT_MS`'s `#[allow(dead_code)]` (lines ~35-38)

FIND:

```rust
/// Bounded timeout (ms) for reading the first reply after a burst.
/// Must be > 0 (unlike the drain's non-blocking timeout=0).
#[allow(dead_code)]
const REPLY_READ_TIMEOUT_MS: i32 = 1000;
```

REPLACE WITH:

```rust
/// Bounded timeout (ms) for reading the first typed reply after a burst
/// ([`read_typed_response`]). Must be > 0 (unlike the drain's non-blocking
/// timeout=0) so the read BLOCKS for a real reply rather than polling. 1000 ms is
/// a conservative bound; P4's QUERY_CALLBACK sweep against a non-capable device may
/// want to lower it (each query against a silent device waits up to this long).
const REPLY_READ_TIMEOUT_MS: i32 = 1000;
```

> **Safe to de-allow:** `read_typed_response` (added in Change 4, staged
> `#[allow(dead_code)]`) references this constant. A const referenced by an
> allow-dead fn's body does NOT warn — this is the IDENTICAL one-hop pattern that let
> the 5 command constants drop their allows once `build_typed_payload` (then
> allow-dead) referenced them (see the comment in Change 1). **Verify** with
> `cargo build` after Change 4: zero "never used" for `REPLY_READ_TIMEOUT_MS`. (The
> existing `typed_command_constants_match_firmware_contract` test also asserts
> `REPLY_READ_TIMEOUT_MS > 0`, so it stays live in test builds regardless.)

### Change 3 — Add `classify_response` (pure, testable) after `parse_callback_name`

Insert this function immediately after `parse_callback_name`'s closing brace and
before the `/// Match parameters a cached handle set was opened for.` doc on
`MatchKey`. FIND:

```rust
fn parse_callback_name(bytes: &[u8]) -> Option<String> {
    let end = bytes.iter().position(|&b| b == 0x00).unwrap_or(bytes.len());
    let name_bytes = &bytes[..end];
    if name_bytes.is_empty() {
        return None;
    }
    String::from_utf8(name_bytes.to_vec()).ok()
}

/// Match parameters a cached handle set was opened for. The cache is rebuilt
```

REPLACE WITH (the new function is inserted between the two):

```rust
fn parse_callback_name(bytes: &[u8]) -> Option<String> {
    let end = bytes.iter().position(|&b| b == 0x00).unwrap_or(bytes.len());
    let name_bytes = &bytes[..end];
    if name_bytes.is_empty() {
        return None;
    }
    String::from_utf8(name_bytes.to_vec()).ok()
}

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
/// Pure (no I/O) — the unit-testable core of [`read_typed_response`].
///
/// Consumer: [`read_typed_response`] (P1.M1.T3.S1) → [`burst_and_read_one`] → the
/// `run()` typed dispatch (P1.M1.T3.S2).
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

/// Match parameters a cached handle set was opened for. The cache is rebuilt
```

### Change 4 — Add `read_typed_response` + `burst_and_read_one` after `burst_to_one`

Insert both functions immediately after `burst_to_one`'s closing brace and before
the `/// Number of reports needed to carry` doc on `batches_for`. FIND:

```rust
    let mut drain_buf = [0u8; REPORT_LENGTH + 1];
    for _ in 0..IN_DRAIN_MAX {
        match interface.read_timeout(&mut drain_buf, 0) {
            Ok(n) if n > 0 => continue,
            _ => break,
        }
    }

    true
}

/// Number of reports needed to carry `data.len()` payload bytes (0 when empty).
fn batches_for(data: &[u8]) -> usize {
```

REPLACE WITH (the two new functions are inserted between):

```rust
    let mut drain_buf = [0u8; REPORT_LENGTH + 1];
    for _ in 0..IN_DRAIN_MAX {
        match interface.read_timeout(&mut drain_buf, 0) {
            Ok(n) if n > 0 => continue,
            _ => break,
        }
    }

    true
}

/// Read ONE typed reply from `interface` after a command burst, then classify it.
///
/// The bounded read primitive for the typed-command path. It blocks up to
/// [`REPLY_READ_TIMEOUT_MS`] for a single 32-byte IN report (the firmware sends
/// exactly one reply per report, `RAW_REPORT_SIZE = 32`), then:
/// - `Ok(n)` with `n > 0` ⇒ [`classify_response`] decodes it (typed if
///   `buf[0] == 0x51` & echo matches `expected_cmd`; legacy `0`/`1`; else Timeout).
/// - `Ok(0)` (poll timed out — no reply) ⇒ `Ok(CommandResponse::Timeout)`. This is
///   **not an error**: a non-capable (legacy) device never replies to a typed
///   command, and the caller (the run() handshake, P4.M2.T1) treats Timeout as
///   "stay in string-only mode" (PRD §8, §10.2).
/// - `Err` ⇒ `Err(QmkError::HidReadError)` (a real HID transport failure).
///
/// Unlike [`burst_to_one`]'s drain loop, this reads EXACTLY ONE report (not a
/// bounded drain) — that one report IS the typed reply. `expected_cmd` is the
/// command-ID byte we sent (e.g. [`CMD_QUERY_INFO`]); it guards against decoding a
/// stale reply left in the IN buffer from a prior command. The 33-byte buffer
/// matches the drain loop's sizing; QMK raw HID uses report ID 0, so `read_timeout`
/// returns report DATA at `buf[0]` (no report-ID prefix on read), i.e. `buf[0] ==
/// 0x51` for a typed reply.
///
/// Consumer: [`burst_and_read_one`] (P1.M1.T3.S1); the `run()` typed dispatch via
/// the send path (P1.M1.T3.S2).
#[allow(dead_code)]
fn read_typed_response(
    interface: &HidDevice,
    expected_cmd: u8,
    verbose: bool,
) -> Result<crate::CommandResponse, QmkError> {
    use crate::CommandResponse;
    let mut buf = [0u8; REPORT_LENGTH + 1]; // 33 bytes — matches the drain loop
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

/// Burst-write `data` to `interface` as `batch_count` reports, then read ONE typed
/// reply (instead of draining all). The typed-command counterpart to
/// [`burst_to_one`].
///
/// The write half is byte-for-byte identical to [`burst_to_one`]'s write loop (same
/// `[0x00][0x81][0x9F]` per-report header, same 30-byte chunking). The read half
/// differs: where [`burst_to_one`] drains up to [`IN_DRAIN_MAX`] reports and
/// discards them, this reads **exactly one** reply via [`read_typed_response`] and
/// returns the parsed [`CommandResponse`].
///
/// Backward compatibility: the legacy [`crate::RunCommand::SendMessage`] path keeps
/// using [`burst_to_one`] (drain-discard) unchanged. Only typed commands route
/// through this capture-and-parse path — wired in P1.M1.T3.S2.
///
/// Returns the parsed reply, or [`QmkError::SendReportError`] on a write failure
/// (mirrors [`burst_to_one`]'s `false`-on-write-error, surfaced as an error here
/// since the caller can't proceed without a successful send). A read timeout is
/// **not** an error — it surfaces as `Ok(CommandResponse::Timeout)`.
///
/// Consumer: the `run()` typed dispatch (P1.M1.T3.S2).
#[allow(dead_code)]
fn burst_and_read_one(
    interface: &HidDevice,
    data: &[u8],
    batch_count: usize,
    expected_cmd: u8,
    verbose: bool,
) -> Result<crate::CommandResponse, QmkError> {
    // --- Write half: identical to burst_to_one's write loop. ---
    let mut request_data = [0u8; REPORT_LENGTH + 1];
    request_data[1] = 0x81;
    request_data[2] = 0x9F;

    for batch in 0..batch_count {
        let start_idx = batch * PAYLOAD_PER_REPORT;
        let end_idx = (start_idx + PAYLOAD_PER_REPORT).min(data.len());
        let batch_data = &data[start_idx..end_idx];

        request_data[3..].fill(0); // clear reused payload tail
        if !batch_data.is_empty() {
            request_data[3..3 + batch_data.len()].copy_from_slice(batch_data);
        }

        if verbose {
            println!("Sending batch {}/{}", batch + 1, batch_count);
            println!("{:?}", request_data);
        }

        if let Err(e) = interface.write(&request_data) {
            if verbose {
                println!("Error on batch {}: {}", batch + 1, e);
            }
            return Err(QmkError::SendReportError(e));
        }
    }

    // --- Read half: ONE typed reply (bounded timeout, NOT a drain loop). ---
    read_typed_response(interface, expected_cmd, verbose)
}

/// Number of reports needed to carry `data.len()` payload bytes (0 when empty).
fn batches_for(data: &[u8]) -> usize {
```

### Change 5 — Update `parse_reply`'s trailing doc sentence (lines ~404-407)

`parse_reply`'s doc still references the stale plan numbering "P1.M3.T3" and claims
it's "referenced only by tests". Its real consumer chain now includes
`classify_response`. The `#[allow(dead_code)]` STAYS (conservative — see Known
Gotchas) but the doc should be accurate. FIND:

```rust
/// Every field access in the typed path uses defensive `.get(...)` indexing —
/// firmware replies may be truncated, so missing bytes default to `0` rather
/// than panicking. Consumer: the `run()` typed dispatch (P1.M3.T3.S1). Until then
/// this is referenced only by tests, hence `#[allow(dead_code)]` — remove it in
/// P1.M3.T3 once `run()` calls it (same lifecycle as [`build_typed_payload`]).
```

REPLACE WITH:

```rust
/// Every field access in the typed path uses defensive `.get(...)` indexing —
/// firmware replies may be truncated, so missing bytes default to `0` rather
/// than panicking. Consumer chain: [`classify_response`] (P1.M1.T3.S1) →
/// [`read_typed_response`] → [`burst_and_read_one`] → the `run()` typed dispatch
/// (P1.M1.T3.S2). The `#[allow(dead_code)]` stays until `run()` goes live (S2
/// lifts it together with the read/classify functions' allows); it is cosmetic
/// now since `classify_response` (allow-dead) calls this.
```

### Change 6 — Add ~5 `classify_response` tests to the existing `mod tests`

Append these inside the existing `#[cfg(test)] mod tests { use super::*; … }` block
in `src/core.rs` (after the last `parse_reply_*` test, before the closing `}`):

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

> The tests use `classify_response`, `CMD_QUERY_INFO`/`CMD_QUERY_CALLBACK`/
> `CMD_SET_OS`/`CMD_APPLY_HOST_CONTEXT`, and `CommandResponse` — ALL in scope via
> the existing `use super::*;` + `use crate::{CommandResponse, HostOs, RunCommand};`
> at the top of `mod tests` (the command-ID consts are `pub(crate)` in `super`).
> No new imports needed.

### Success Criteria

- [ ] `classify_response(buf, expected_cmd)` exists, is `pub(crate)`, pure, and:
      typed reply with `buf[1]==expected_cmd` ⇒ delegates to `parse_reply`;
      typed reply with `buf[1]!=expected_cmd` ⇒ `Timeout`; legacy `0/1` ⇒ `Legacy`;
      empty/unknown ⇒ `Timeout`.
- [ ] `read_typed_response(interface, expected_cmd, verbose)` exists, staged
      `#[allow(dead_code)]`, returns `Ok(classify_response(..))` on `Ok(n>0)`,
      `Ok(Timeout)` on `Ok(0)`, `Err(HidReadError)` on `Err`.
- [ ] `burst_and_read_one(interface, data, batch_count, expected_cmd, verbose)`
      exists, staged `#[allow(dead_code)]`; its write loop is byte-identical to
      `burst_to_one`'s; write failure ⇒ `Err(SendReportError)`; otherwise calls
      `read_typed_response`.
- [ ] `#[allow(dead_code)]` removed from `REPLY_READ_TIMEOUT_MS`; its doc explains
      the timeout + the P4 tuning note.
- [ ] The stale "RESPONSE_MARKER and REPLY_READ_TIMEOUT_MS still carry allow"
      comment corrected; `parse_reply`'s doc updated (consumer chain + allow note).
- [ ] ~5 `classify_response_*` tests added; all pass.
- [ ] `burst_to_one`/`send_raw_report`/`parse_reply` body/`run()`/`lib.rs`/
      `error.rs`/`Cargo.toml` unchanged except the documented doc/allow tweaks.
- [ ] `cargo build` → zero warnings (no "never used" for `REPLY_READ_TIMEOUT_MS`).

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed
> to implement this successfully?"_ — **Yes.** Exact FIND/REPLACE anchors for every
> edit (the comment, the `REPLY_READ_TIMEOUT_MS` block, the two insertion seams, the
> `parse_reply` doc), verbatim replacement code (the three functions + the 5 tests),
> the verified dead_code staging rationale (with the in-codebase proof), the
> hidapi `read_timeout` semantics (with the in-codebase drain-loop precedent), and
> verified build/clippy/fmt/test commands are all below. The implementer does not
> need to read the firmware source — `firmware_wire_contract.md` already
> canonicalized every reply byte, and `parse_reply` already decodes them.

### Documentation & References

```yaml
# MUST READ — the file under edit (PRIMARY & ONLY target).
- file: /home/dustin/projects/qmk_notifier/src/core.rs
  why: "(a) Contains parse_reply/parse_typed_reply/parse_callback_name — the PARSER,
        ALREADY IMPLEMENTED & TESTED (do NOT touch their bodies or tests). This task
        adds the READER layer above them: classify_response (after parse_callback_name),
        read_typed_response + burst_and_read_one (after burst_to_one). (b) burst_to_one
        is the WRITE-LOOP TEMPLATE for burst_and_read_one (mirror its write half exactly).
        (c) REPLY_READ_TIMEOUT_MS loses its allow here. (d) RESPONSE_MARKER/CMD_* are the
        consts classify_response uses."
  section: "REPLY_READ_TIMEOUT_MS (line ~37), burst_to_one (~258, write loop + drain
            loop), parse_reply (~409, the parser — read-only), parse_callback_name
            (~462, insertion seam), batches_for (doc = insertion seam)"
  critical: "parse_reply ALREADY EXISTS and is tested — do NOT re-implement parsing.
             classify_response DELEGATES to parse_reply (it only adds the expected_cmd
             echo guard). burst_and_read_one's write loop must be byte-identical to
             burst_to_one's (copy it; only the tail differs: read_one vs drain)."

# MUST READ — the wire contract (canonical reply byte layouts).
- file: /home/dustin/projects/qmk_notifier/plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md
  why: "§Reply Disambiguation: response[0]==0x51 ⇒ typed (decode by echo at [1]); 0/1
        ⇒ legacy; no reply ⇒ timeout. §Field Definitions: the exact per-command reply
        layouts parse_reply already decodes (QUERY_INFO/QUERY_CALLBACK/SET_OS/
        APPLY_HOST_CONTEXT). §Constants: RESPONSE_MARKER=0x51, NOTIFY_CMD_* ids."
  section: "Reply Disambiguation, Field Definitions, Constants"
  critical: "The expected_cmd guard in classify_response maps 1:1 to §Reply
             Disambiguation's 'decode by response[1]'. Firmware is NOT yet implemented
             (§Firmware Implementation Status) — typed commands time out against current
             firmware, which is the designed fallback (Timeout)."

# MUST READ — the previous subtask's PRP (the CONTRACT for what run() looks like now).
- file: /home/dustin/projects/qmkonnect/plan/002_637d65b6e9b8/P1M1T2S2/PRP.md
  why: "P1.M1.T2.S2 (in parallel) replaces the 4 todo!() typed arms in run() with a
        collapsed or-pattern arm that does build_typed_payload → send_raw_report →
        Ok(CommandResponse::Timeout) PLACEHOLDER. THIS task (S1) does NOT touch run();
        it provides the read primitives that S2 will use to replace that placeholder.
        Confirms send_raw_report still returns Result<(),_> and burst_to_one still
        drains — both UNCHANGED here."
  section: "What (Change 1b — the typed arm), Integration Points (DOWNSTREAM CONSUMER)"
  critical: "Do NOT wire read_typed_response/burst_and_read_one into run() in S1 — that
             is P1.M1.T3.S2. S1 only adds the staged primitives in core.rs."

# REFERENCE — the build_typed_payload S1→S2 staging precedent (the allow-dead pattern).
- file: /home/dustin/projects/qmkonnect/plan/002_637d65b6e9b8/P1M1T2S1/PRP.md
  why: "Shows the EXACT staging pattern this task reuses: stage a new pub(crate) fn as
        #[allow(dead_code)] with a doc note 'consumer lands in S2'; S2 then removes the
        allow when run() calls it. The 5 command consts dropped their allows the same
        way (a const referenced by an allow-dead fn's body does NOT warn) — that is the
        proof REPLY_READ_TIMEOUT_MS can be de-allowed now."
  section: "Goal, The new build_typed_payload function, Anti-Patterns (allow-dead staging)"

# REFERENCE — hidapi read_timeout API (the read primitive this task uses).
- url: https://docs.rs/hidapi/latest/hidapi/struct.HidDevice.html#method.read_timeout
  why: "Signature: read_timeout(&mut data, timeout_ms: i32) -> Result<usize, HidError>.
        Ok(n>0)=read n bytes; Ok(0)=timed out (no data) — with timeout_ms>0 it BLOCKS up
        to that many ms; Err=real HID error. This is EXACTLY the behavior the existing
        drain loop relies on (it passes 0 = non-blocking poll). read_typed_response
        passes REPLY_READ_TIMEOUT_MS (>0) = blocking bounded read."
  critical: "On a single-report (report-ID-0) interface like QMK raw HID, read_timeout
             returns report DATA starting at buf[0] (no report-ID prefix on READ —
             contrast the WRITE path which must prefix 0x00). So buf[0]==0x51 for a
             typed reply. parse_reply(&buf[..n]) is correct."

# REFERENCE — research notes for THIS subtask (design decisions + dead_code proof).
- docfile: plan/002_637d65b6e9b8/P1M1T3S1/research/notes.md
  why: "The 6 design decisions: separate fn (not modify burst_to_one); 3-layer
        decomposition (classify/read/burst_and_read); dead_code staging (keep parse_reply's
        allow, drop REPLY_READ_TIMEOUT_MS's — proven); timeout value (keep 1000ms by name);
        buffer sizing (33 bytes, report-ID-0); read_timeout semantics."
- docfile: plan/002_637d65b6e9b8/P1M1T3S1/research/dead_code_precedent.md
  why: "The two in-codebase proofs that an allow-dead fn's references don't warn:
        (1) parse_typed_reply has no allow yet builds clean (only called by allow-dead
        parse_reply); (2) the documented build_typed_payload→5-consts precedent. ⇒
        REPLY_READ_TIMEOUT_MS is safe to de-allow once read_typed_response references it."
```

### Current Codebase tree (run from the crate root `/home/dustin/projects/qmk_notifier`)

```bash
qmk_notifier/
├── Cargo.toml          # name="qmk_notifier", version="0.2.1", edition="2021" — DO NOT TOUCH.
├── Cargo.lock
├── README.md
├── PRD.md              # crate PRD (§7, §4.2) — reference only.
├── .gitignore          # contains only: /target
├── plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md   # WIRE SOURCE OF TRUTH
└── src
    ├── main.rs         # binary entrypoint — only SendMessage/ListDevices. DO NOT TOUCH.
    ├── error.rs        # QmkError (HidReadError/SendReportError/NoResponseReceived exist) — DO NOT TOUCH.
    ├── lib.rs          # run() with the Timeout placeholder (P1.M1.T2.S2). DO NOT TOUCH (S2 owns run()).
    └── core.rs         # <-- PRIMARY & ONLY EDIT: add classify_response/read_typed_response/
                        #     burst_and_read_one; drop REPLY_READ_TIMEOUT_MS allow; fix comment;
                        #     add 5 tests. parse_reply/parse_typed_reply/parse_callback_name/
                        #     burst_to_one/send_raw_report/build_typed_payload — bodies UNCHANGED.
```

### Desired Codebase tree with files to be modified

```bash
src/
└── core.rs  # MODIFIED: +classify_response, +read_typed_response, +burst_and_read_one;
             #   -#[allow(dead_code)] on REPLY_READ_TIMEOUT_MS; corrected module comment;
             #   parse_reply doc tweak; +5 classify_response_* tests in mod tests.
# (no new files; main.rs/error.rs/lib.rs/Cargo.toml untouched)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL: parse_reply ALREADY EXISTS and is fully tested (9 parse_reply_* tests).
//   Do NOT re-implement, rename, or move it. classify_response DELEGATES to it — it
//   only adds the expected_cmd echo guard on the typed (0x51) path. parse_reply's
//   OWN tests stay green untouched (they don't pass expected_cmd, which is fine —
//   parse_reply has no such param).

// CRITICAL: burst_and_read_one's WRITE LOOP must be byte-for-byte identical to
//   burst_to_one's (same [0u8;REPORT_LENGTH+1] buffer, request_data[1]=0x81,[2]=0x9F,
//   same batch math via PAYLOAD_PER_REPORT, same `if let Err(e)=write(..)` handling).
//   The ONLY difference is the tail: burst_to_one drains (read_timeout(0) loop),
//   burst_and_read_one calls read_typed_response (read_timeout(REPLY_READ_TIMEOUT_MS),
//   ONE read). Copy the write loop; don't paraphrase it.

// CRITICAL: de-allowing REPLY_READ_TIMEOUT_MS is safe — PROVEN by two in-codebase
//   precedents (see research/dead_code_precedent.md): (1) parse_typed_reply has NO
//   allow yet builds clean (only called by allow-dead parse_reply); (2) the
//   documented build_typed_payload→5-consts pattern. read_typed_response (allow-dead)
//   references REPLY_READ_TIMEOUT_MS ⇒ same one-hop rule ⇒ no warning. VERIFY with
//   `cargo build` after Change 4. If (unexpectedly) a "never used" warning appears,
//   re-add the allow — but the precedent guarantees it won't.

// CRITICAL: do NOT remove parse_reply's #[allow(dead_code)] in S1. Its new caller
//   classify_response is itself only reachable via allow-dead read_typed_response
//   (TWO hops — unproven, unlike the one-hop REPLY_READ_TIMEOUT_MS case). parse_reply
//   is test-reachable anyway, so the allow is cosmetic. S2 removes it (and the three
//   new functions' allows) when run() goes live. Keep it conservative.

// CRITICAL: Ok(0) from read_timeout is NOT an error — it's "poll timed out, no data".
//   read_typed_response returns Ok(CommandResponse::Timeout) on Ok(0), NOT
//   Err(NoResponseReceived). The item is explicit: "not an error — the caller decides".
//   (NoResponseReceived exists in error.rs for OTHER use; do not use it here.)

// GOTCHA: read_timeout returns report DATA at buf[0] for a report-ID-0 interface
//   (QMK raw HID), so buf[0]==0x51 for a typed reply. Do NOT strip a leading report-ID
//   byte. parse_reply(&buf[..n]) is correct as-is. (Contrast the WRITE path, which
//   prefixes request_data[0]=0x00 — that asymmetry is a known hidapi quirk.)

// GOTCHA: the three new functions are hardware-bound (like burst_to_one/send_raw_report,
//   which have NO unit tests). Only classify_response is pure and unit-testable. Do
//   NOT try to unit-test read_typed_response/burst_and_read_one (you can't construct a
//   HidDevice in a unit test). Their verification is `cargo build` (they compile) +
//   future hardware integration (S2/P4). This matches the codebase's existing split:
//   pure logic is tested; HID-I/O is not.

// NOTE: REPLY_READ_TIMEOUT_MS stays 1000ms (its existing value). Use it BY NAME in
//   read_typed_response — do NOT hardcode 100 or 1000. The item's "e.g. 100ms" was
//   illustrative; tuning is P4's job (the QUERY_CALLBACK sweep against a silent device
//   waits up to this per query). Changing the value now would churn a tested constant
//   (`REPLY_READ_TIMEOUT_MS > 0` in typed_command_constants_match_firmware_contract).

// NOTE: do NOT touch run() in lib.rs. The typed arm currently returns the
//   Ok(CommandResponse::Timeout) PLACEHOLDER (P1.M1.T2.S2). Replacing it with a real
//   burst_and_read_one call is P1.M1.T3.S2. S1 only adds the core.rs primitives.
```

## Implementation Blueprint

### Data models and structure

No new types. S1 layers functions over the EXISTING types: `HidDevice` (hidapi),
`CommandResponse` (lib.rs, P1.M1.T1.S2), `QmkError` (error.rs). The "structure" is
the read pipeline: `burst_and_read_one` → `read_typed_response` → `classify_response`
→ `parse_reply`. Only `classify_response` and `parse_reply` are pure/testable; the
top two are I/O.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: READ current state of src/core.rs (anchors)
  - READ: /home/dustin/projects/qmk_notifier/src/core.rs. LOCATE: (a) the
          module comment block (~lines 14-25); (b) REPLY_READ_TIMEOUT_MS (~35-38);
          (c) burst_to_one's body (~258-330) — the write loop you'll COPY for
          burst_and_read_one, and the drain loop you will NOT copy; (d) parse_reply
          (~390-417) and its doc/allow — READ-ONLY; (e) parse_callback_name's closing
          brace (~480, insertion seam for classify_response); (f) batches_for's doc
          (insertion seam for read_typed_response/burst_and_read_one); (g) the end of
          mod tests (where the 5 tests append).
  - CONFIRM: parse_reply/parse_typed_reply/parse_callback_name ALREADY EXIST with
          passing tests (cargo test --lib parse_reply_). If they're absent, STOP —
          this task depends on them; re-check the plan status.
  - CONFIRM: REPLY_READ_TIMEOUT_MS currently carries #[allow(dead_code)] (line ~37);
          RESPONSE_MARKER does NOT (line ~29).

Task 2: EDIT the module comment (Change 1)
  - REPLACE the stale "Only RESPONSE_MARKER and REPLY_READ_TIMEOUT_MS still carry
          #[allow(dead_code)]" comment with the corrected version (exact FIND/REPLACE
          in "What"). This is a comment-only edit; no behavior change.

Task 3: EDIT REPLY_READ_TIMEOUT_MS — remove allow + update doc (Change 2)
  - DELETE the `#[allow(dead_code)]` line above REPLY_READ_TIMEOUT_MS; REPLACE its
          2-line doc with the 4-line version (exact text in "What").
  - NOTE: this will produce a "never used" warning UNTIL Change 4 lands
          (read_typed_response references it). So do Changes 3-4 before running the
          final `cargo build`, OR accept a transient warning mid-edit. (The allow
          removal is only clean once read_typed_response exists.)

Task 4: ADD classify_response after parse_callback_name (Change 3)
  - INSERT the classify_response fn (verbatim in "What") between parse_callback_name's
          closing `}` and the `/// Match parameters` doc. It is pub(crate),
          #[allow(dead_code)], pure, delegates to parse_reply.
  - CHECK: it reads RESPONSE_MARKER and calls parse_reply (both in scope — same module).

Task 5: ADD read_typed_response + burst_and_read_one after burst_to_one (Change 4)
  - INSERT both fns (verbatim in "What") between burst_to_one's closing `}` and the
          `/// Number of reports` doc.
  - CHECK: read_typed_response uses REPLY_READ_TIMEOUT_MS (now de-allowed in Task 3 —
          this is what makes that de-allow clean) and calls classify_response.
  - CHECK: burst_and_read_one's write loop is byte-identical to burst_to_one's; its
          tail calls read_typed_response. Write failure ⇒ Err(SendReportError).
  - CHECK: both are #[allow(dead_code)] (consumers land in S2).

Task 6: EDIT parse_reply's trailing doc (Change 5)
  - REPLACE its trailing doc sentence (exact FIND in "What") with the updated text
          naming the classify_response→read_typed_response→burst_and_read_one→run()
          consumer chain + the allow note. parse_reply's #[allow(dead_code)] STAYS.
          Body and tests UNCHANGED.

Task 7: ADD the 5 classify_response tests to mod tests (Change 6)
  - APPEND the 5 #[test] fns (verbatim in "What") at the end of mod tests, before its
          closing `}`.
  - NAMES: classify_response_typed_matching_echo_decodes,
          classify_response_typed_mismatched_echo_is_timeout,
          classify_response_legacy_delegates_to_parse_reply,
          classify_response_empty_and_unknown_marker_are_timeout,
          classify_response_ack_variants_require_matching_echo.
  - NO new imports (use super::* + the existing use crate::{CommandResponse,..} cover
          classify_response, the CMD_* consts, and CommandResponse).

Task 8: VALIDATE (do not skip)
  - RUN (from /home/dustin/projects/qmk_notifier):
          cargo fmt && cargo build && cargo clippy --lib &&
          cargo fmt --check && cargo test --lib
  - EXPECT: build ZERO warnings (NO "never used" for REPLY_READ_TIMEOUT_MS — Change 4
          made read_typed_response reference it; NO warning for the 3 new allow-dead
          fns — they carry allows). clippy clean. fmt --check exit 0. All tests pass
          (existing 62 incl. 9 parse_reply_* + 5 new classify_response_*).
  - IF "never used: REPLY_READ_TIMEOUT_MS": Change 4 didn't land or read_typed_response
          doesn't reference it by name — fix. (Per the build_typed_payload precedent it
          should be clean.)
  - SANITY: `git diff --stat` shows ONLY src/core.rs changed.
```

### Implementation Patterns & Key Details

```rust
// === WHY delegate classify_response to parse_reply (not re-implement) ===
//   parse_reply already disambiguates 0x51/0/1/other AND decodes every typed variant,
//   with 9 passing tests. classify_response's ONLY new logic is the expected_cmd echo
//   guard on the 0x51 path (mismatch ⇒ Timeout). Everything else delegates. This keeps
//   the parsing logic in ONE place (parse_reply) and the echo-validation in another
//   (classify_response) — single responsibility, no duplication.

// === WHY Ok(Timeout) instead of Err(NoResponseReceived) on a read timeout ===
//   The item is explicit: "On Ok(0) or timeout: return CommandResponse::Timeout
//   (not an error — the caller decides what to do)." A non-capable device simply
//   doesn't reply to typed commands; that's NORMAL (the designed legacy fallback),
//   not a transport failure. The P4 handshake treats Timeout as "stay string-only".
//   NoResponseReceived exists for a different (more exceptional) signaling need.

// === WHY keep parse_reply's allow but drop REPLY_READ_TIMEOUT_MS's ===
//   REPLY_READ_TIMEOUT_MS: one hop from allow-dead read_typed_response ⇒ proven safe
//   (build_typed_payload→5-consts precedent). parse_reply: TWO hops (reachable only via
//   classify_response ← allow-dead read_typed_response) ⇒ unproven, so keep its allow
//   (cosmetic — it's test-reachable anyway). S2 removes all remaining allows together.

// === WHY a separate burst_and_read_one (not a flag on burst_to_one) ===
//   The item offered both options. The separate fn keeps burst_to_one byte-for-byte
//   unchanged → zero risk to the proven legacy SendMessage drain path (backward compat).
//   A `capture: bool` flag on burst_to_one would ripple through try_send_once/
//   send_raw_report and risk the legacy path. Separate fn = minimal blast radius.

// === The write-error surface in burst_and_read_one ===
//   burst_to_one returns bool (false on write error) because send_raw_report counts
//   succeeded/failed devices. burst_and_read_one returns Result<CommandResponse,_>:
//   a write failure surfaces as Err(SendReportError(e)) — the caller (S2's send path)
//   can't get a reply without a successful send, so an error is the honest signal.
//   (S2 decides how to aggregate this across the multi-device cache; out of scope here.)
```

### Integration Points

```yaml
SOURCE FILES:
  - modify (ONLY): "/home/dustin/projects/qmk_notifier/src/core.rs"
      - +classify_response (after parse_callback_name)
      - +read_typed_response, +burst_and_read_one (after burst_to_one)
      - -#[allow(dead_code)] on REPLY_READ_TIMEOUT_MS (+ doc)
      - corrected module comment (~line 14-25)
      - parse_reply doc tweak (~line 404)
      - +5 classify_response_* tests in mod tests

DEPENDENCIES / Cargo.toml:
  - none. No new crate deps (hidapi already present; read_timeout already used by the
    drain loop).

PUBLIC API SURFACE:
  - UNCHANGED. classify_response/read_typed_response/burst_and_read_one are all
    pub(crate) or private in the private `mod core` (NOT re-exported at the crate
    root). run()'s signature is unchanged (Result<CommandResponse,_> — T1.S2 set it).

CONSUMES (treat as fixed, already landed):
  - P1.M1.T1.S2 (Complete): CommandResponse enum (Info/CallbackName/Ack/Legacy/Timeout).
  - P1.M1.T2.S1 (Complete): build_typed_payload + the 5 command-ID consts + RESPONSE_MARKER
    + REPLY_READ_TIMEOUT_MS.
  - P1.M1.T2.S2 (in parallel): run() typed arm = build_typed_payload → send_raw_report →
    Ok(CommandResponse::Timeout) PLACEHOLDER.
  - The pre-existing parse_reply/parse_typed_reply/parse_callback_name (parser, tested).

DOWNSTREAM CONSUMER (do NOT implement now — listed for awareness):
  - P1.M1.T3.S2: replace run()'s typed-arm Timeout placeholder with a real send+read.
    Will likely add a send_typed_report wrapper (cache lookup + burst_and_read_one per
    device) OR evolve send_raw_report to return the reply. Removes the 4 remaining
    allows (parse_reply, classify_response, read_typed_response, burst_and_read_one).
  - P4.M2.T1.S1: handshake — QUERY_INFO → (Info{proto_ver==2,flags&0x01}) →
    QUERY_CALLBACK sweep → name→id map. Pattern-matches on Info/Ack/Timeout.

OUT-OF-SCOPE (later subtasks — do NOT implement here):
  - P1.M1.T3.S2: wire read_typed_response/burst_and_read_one into run() (replaces the
    Timeout placeholder). Touches lib.rs + send path; removes remaining allows.
  - P1.M1.T4.S1: bump crate version to 0.3.0 + tag.
  - P4.M2.T1.S1: the handshake that consumes CommandResponse.
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
#   - If "never used: const `REPLY_READ_TIMEOUT_MS`": Change 4 (read_typed_response)
#     didn't land or doesn't reference it by name. Fix (the build_typed_payload precedent
#     guarantees it's de-allowable once referenced).
#   - If "never used: function `read_typed_response`/`burst_and_read_one`/
#     `classify_response`": you forgot their #[allow(dead_code)] attributes.
#   - If E0432/E0425 "cannot find ... in this scope": you used a bare name not in scope.
#     classify_response/read_typed_response/burst_and_read_one are in the same `mod core`
#     as parse_reply/burst_to_one/REPLY_READ_TIMEOUT_MS/RESPONSE_MARKER/CMD_*, so they're
#     all in scope. CommandResponse is `crate::CommandResponse` (use the path or a local
#     `use crate::CommandResponse;` like parse_reply does).

# Lint (default clippy — no .clippy.toml exists).
cargo clippy --lib 2>&1 | tee /tmp/clippy.log
# Expected: no warnings/errors specific to the new functions. clippy may suggest
#   ergonomics — accept sensible fixes but do NOT change the read/classify shape.

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

# Run the 5 new classify_response tests in isolation first.
cargo test --lib classify_response_ -- --nocapture
# Expected: 5 passed. classify_response_typed_matching_echo_decodes,
#   _typed_mismatched_echo_is_timeout (the NEW guard),
#   _legacy_delegates_to_parse_reply, _empty_and_unknown_marker_are_timeout,
#   _ack_variants_require_matching_echo.

# Re-run the existing parse_reply tests (must be UNCHANGED — we only edited their doc).
cargo test --lib parse_reply_ -- --nocapture
# Expected: 9 passed (parse_reply_info_reply, _info_board_rules_absent,
#   _callback_name_named, _callback_name_unnamed, _ack_set_os_applied,
#   _ack_apply_host_context_rejected, _empty_slice_is_timeout, _legacy_zero_is_no_match,
#   _legacy_one_is_matched, _typed_marker_only_is_timeout, _unknown_cmd_echo_is_timeout,
#   _unknown_marker_is_timeout, _callback_name_non_utf8_is_none,
#   _truncated_info_defaults_board_rules_false).

# Full lib test suite.
cargo test --lib
# Expected: "test result: ok. <N> passed; 0 failed; 0 ignored; ...". N = prior 62 + 5 new
#   = 67 (the exact count is not load-bearing; the gate is 0 failed).
```

### Level 3: Integration Testing (System Validation)

```text
PARTIALLY APPLICABLE. read_typed_response/burst_and_read_one do real HID I/O
(read_timeout/write), so a full round-trip needs a QMK keyboard with the v0.3.0
typed-command firmware (P1.M2 — NOT implemented yet; see firmware_wire_contract.md
§Firmware Implementation Status). WITHOUT such hardware:

  - The 5 classify_response tests (Level 2) ARE the verification that the pure
    decode+guard logic is correct — they exercise EVERY path read_typed_response feeds
    (typed-match, typed-mismatch⇒Timeout, legacy 0/1, empty/unknown⇒Timeout, ack both
    ways). Since read_typed_response is a thin I/O wrapper around classify_response
    (read_timeout result → classify_response), getting classify_response right IS
    getting the parse logic right.
  - The two I/O functions (read_typed_response/burst_and_read_one) are verified by
    `cargo build` (they compile, types/signatures correct) — same bar as the existing
    hardware-bound burst_to_one/send_raw_report (which also have no unit tests).

  Live-hardware validation of the typed round-trip is deferred to P1.M1.T3.S2 (run()
  wiring) + P1.M2 (firmware) — out of scope here. Once both land, `run(QueryInfo)`
  against typed-capable firmware returns Info{..}; against legacy firmware returns Timeout.
```

### Level 4: Creative & Domain-Specific Validation

```bash
cd /home/dustin/projects/qmk_notifier

# Confirm rustdoc renders (Mode A documentation) for the 3 new functions + the updated
# REPLY_READ_TIMEOUT_MS/parse_reply docs.
cargo doc --lib --no-deps 2>&1 | grep -iE "warning|error" || echo "docs clean (good)"

# Confirm REPLY_READ_TIMEOUT_MS no longer carries #[allow(dead_code)] (Change 2),
# while the 3 new functions + parse_reply still do (staged for S2).
grep -nB1 "REPLY_READ_TIMEOUT_MS\|fn read_typed_response\|fn burst_and_read_one\|fn classify_response\|fn parse_reply" src/core.rs
# Expected: REPLY_READ_TIMEOUT_MS has NO allow above it; the 4 functions each have
#   #[allow(dead_code)] directly above them (consumers land in S2).

# Confirm the new functions exist and are staged allow-dead (not accidentally pub).
grep -n "fn classify_response\|fn read_typed_response\|fn burst_and_read_one" src/core.rs
# Expected: 3 hits; classify_response is `pub(crate)` (tested); the other two are
#   private (they take &HidDevice, internal transport helpers).

# Confirm burst_to_one is UNCHANGED (still drains, still returns bool) — backward compat.
grep -nA2 "fn burst_to_one" src/core.rs
# Expected: `fn burst_to_one(interface: &HidDevice, data: &[u8], batch_count: usize, verbose: bool) -> bool`
#   — signature identical to before this task.

# Confirm zero dead-code warnings overall.
cargo build 2>&1 | grep -iE "never used|warning" || echo "zero dead-code warnings (good)"
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1 passed: `cargo build` → zero warnings (no "never used" for
      `REPLY_READ_TIMEOUT_MS` — Change 4 made `read_typed_response` reference it).
- [ ] Level 1 passed: `cargo clippy --lib` → zero new warnings.
- [ ] Level 1 passed: `cargo fmt --check` → exit 0.
- [ ] Level 2 passed: `cargo test --lib` → all pass, 0 failed (5 new
      `classify_response_*` + all existing incl. the 9 unchanged `parse_reply_*`).

### Feature Validation

- [ ] `classify_response` exists, pure, `pub(crate)`: typed+matching-echo ⇒ delegates
      to `parse_reply`; typed+mismatched-echo ⇒ `Timeout`; legacy 0/1 ⇒ `Legacy`;
      empty/unknown ⇒ `Timeout`.
- [ ] `read_typed_response` exists: `Ok(n>0)`⇒`Ok(classify_response)`, `Ok(0)`⇒`Ok(Timeout)`,
      `Err`⇒`Err(HidReadError)`. Reads into a 33-byte buffer with `REPLY_READ_TIMEOUT_MS`.
- [ ] `burst_and_read_one` exists: write loop byte-identical to `burst_to_one`; write
      failure ⇒ `Err(SendReportError)`; tail calls `read_typed_response`.
- [ ] `#[allow(dead_code)]` removed from `REPLY_READ_TIMEOUT_MS`; doc updated.
- [ ] Module comment corrected; `parse_reply` doc updated (consumer chain + allow note).
- [ ] `burst_to_one`/`send_raw_report`/`parse_reply` body/`run()`/`lib.rs`/`error.rs`/
      `Cargo.toml` unchanged except the documented doc/allow tweaks.
- [ ] Only `src/core.rs` modified.

### Code Quality Validation

- [ ] `classify_response` delegates to `parse_reply` (no parsing duplication); the only
      NEW logic is the expected_cmd echo guard.
- [ ] `burst_and_read_one`'s write loop is COPIED from `burst_to_one` (not paraphrased)
      — identical framing, chunking, error handling up to the read tail.
- [ ] The 3 new functions carry `#[allow(dead_code)]` (staged for S2), mirroring
      `build_typed_payload`'s S1→S2 pattern.
- [ ] New tests follow the block's existing style (`use super::*;`, snake_case,
      `assert_eq!` against `CommandResponse` variants — consistent with `parse_reply_*`).
- [ ] `Ok(0)` (timeout) is `Ok(Timeout)`, NOT an error — matches the item's "not an
      error — the caller decides".

### Documentation & Deployment

- [ ] Rustdoc (Mode A) on `classify_response`/`read_typed_response`/`burst_and_read_one`
      covers the read semantics, the timeout-not-an-error contract, the report-ID-0
      buffer layout, and the P1.M1.T3.S2 consumer forward-ref.
- [ ] `REPLY_READ_TIMEOUT_MS` doc explains the value + the P4 tuning note.
- [ ] No new environment variables or config.

---

## Anti-Patterns to Avoid

- ❌ Don't re-implement `parse_reply` or move/renumber its tests. It EXISTS and is
  tested (9 `parse_reply_*`). `classify_response` DELEGATES to it — the only new logic
  is the expected_cmd echo guard. Duplicating the 0x51/0/1 disambiguation + per-variant
  decode creates two sources of truth that will drift.
- ❌ Don't remove `parse_reply`'s `#[allow(dead_code)]` in S1. Its new caller
  `classify_response` is reachable only via allow-dead `read_typed_response` (TWO hops,
  unproven). Keep it conservative; S2 removes it (and the 3 new functions' allows) when
  `run()` goes live. (REPLY_READ_TIMEOUT_MS IS safe to de-allow — ONE hop, proven.)
- ❌ Don't wire `read_typed_response`/`burst_and_read_one` into `run()`. That's
  P1.M1.T3.S2. S1 only adds the staged primitives in `core.rs`. Touching `run()`/`lib.rs`
  collides with the in-parallel P1.M1.T2.S2 (which owns the typed arm) and with S2.
- ❌ Don't modify `burst_to_one` (add a `capture` flag, change its return, etc.). The
  item's "Alternatively, add a separate `burst_and_read_one()`" is the chosen path — it
  keeps the legacy `SendMessage` drain path byte-for-byte unchanged (backward compat).
  A flag on `burst_to_one` ripples through `try_send_once`/`send_raw_report` and risks
  the proven legacy send path.
- ❌ Don't return `Err(NoResponseReceived)` on a read timeout. `Ok(0)` from
  `read_timeout` is "poll timed out, no reply" — a NORMAL outcome for a non-capable
  (legacy) device. The item is explicit: return `Ok(CommandResponse::Timeout)`, "not an
  error — the caller decides". (`NoResponseReceived` exists for a different need; leave
  it unused here.)
- ❌ Don't strip a leading report-ID byte from the read buffer. QMK raw HID uses report
  ID 0; `read_timeout` returns report DATA at `buf[0]` (so `buf[0]==0x51` for a typed
  reply). Contrast the WRITE path which prefixes `0x00` — that asymmetry is a known
  hidapi quirk, NOT a bug to "fix" by aligning them.
- ❌ Don't hardcode the timeout (100 or 1000) in `read_typed_response`. Use
  `REPLY_READ_TIMEOUT_MS` by name. The item's "e.g. 100ms" was illustrative; the
  constant exists and is tested; tuning is P4's job.
- ❌ Don't try to unit-test `read_typed_response`/`burst_and_read_one` — you can't
  construct a `HidDevice` in a unit test. They're hardware-bound, like `burst_to_one`/
  `send_raw_report` (which have NO unit tests). Test `classify_response` (the pure
  layer); verify the I/O functions compile (`cargo build`) + defer live validation to
  S2/P4.
- ❌ Don't change `read_typed_response`'s write-error handling in `burst_and_read_one`
  to silently return `Ok(Timeout)`. A write FAILURE is a transport error, not a missing
  reply — surface it as `Err(SendReportError(e))` so the caller (S2) can distinguish
  "send broke" from "device didn't reply".
- ❌ Don't call the new functions from `cfg(test)` code expecting to exercise real HID
  I/O — there's no device. The 5 `classify_response_*` tests cover the parse logic; the
  I/O functions are compile-verified only.
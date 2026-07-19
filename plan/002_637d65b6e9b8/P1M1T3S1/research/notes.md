# Research Notes — P1.M1.T3.S1: Response reader & parser

## ⚠️ BASELINE CHANGED UNDER THIS TASK — read this first

This task was specified against an **OLDER baseline** (the item assumed
`burst_to_one` *drains* IN reports and `send_raw_report` returns `Result<(), _>`).
**That baseline no longer exists.** While this PRP was being researched, a parallel
implementation landed commit `71248cd` ("Evolve burst_to_one to capture first
reply"), which took the item's **OTHER option (b)** — *modifying* `burst_to_one` —
rather than adding a separate `burst_and_read_one`. The current COMMITTED state:

| Symbol | Committed state (71248cd) | Notes |
|---|---|---|
| `burst_to_one` | `-> (bool, Option<Vec<u8>>)` — **writes + captures first reply** (bounded `read_timeout(REPLY_READ_TIMEOUT_MS)`) + drains surplus | The item's option (b) "modify burst_to_one" — DONE. |
| `try_send_once` | `-> Result<(SendOutcome, Option<Vec<u8>>), _>` — threads `first_reply` ("first successful device wins") | Captures the reply up the send path. |
| `send_raw_report` | `-> Result<Option<Vec<u8>>, QmkError>` — returns the captured reply bytes (or `None`) | **Signature changed** from `Result<(), _>`. |
| `REPLY_READ_TIMEOUT_MS` | `const … = 1000`, **no allow** (consumed by `burst_to_one`) | Already de-allowed. |
| `parse_reply` | exists, `#[allow(dead_code)]`, tested (9+ tests) | Parser — UNCHANGED by this task. |
| `run()` (lib.rs) | calls `send_raw_report`, **IGNORES** the `Option<Vec<u8>>`, returns `Timeout` placeholder. Doc at line ~332 is STALE ("send_raw_report STILL returns Result<(), _>") | Wiring is **P1.M1.T3.S2** (NOT this task). |

### Consequence for THIS task's design
The item offered two ways to capture the reply: (b1) *modify `burst_to_one`* or
(b2) *add `burst_and_read_one`*. **(b1) was already chosen and committed.** Therefore:

- **`burst_and_read_one` is REDUNDANT — do NOT add it (remove it if a parallel
  attempt added it).** It write+reads+parses, but `burst_to_one` already captures and
  `send_raw_report` already returns the bytes; wiring `burst_and_read_one` into `run()`
  would **double-read** (the kernel IN buffer would be drained by the first read).
- **`read_typed_response(device, …)` is the item's explicit contract (option a) — keep
  it** as the standalone read+parse primitive, but note S2's main flow will use
  `send_raw_report`'s captured `Option<Vec<u8>> + classify_response` (the read already
  happened inside `burst_to_one`). `read_typed_response` stays `#[allow(dead_code)]`.
- **`classify_response(buf, expected_cmd)` is the genuinely-new, non-redundant,
  pipeline-critical deliverable** — it is the PARSE step S2 applies to
  `send_raw_report`'s captured bytes. THIS is the heart of the task.

### What S2 (P1.M1.T3.S2) will do with this (for awareness, NOT this task)
S2 replaces `run()`'s typed-arm `Ok(CommandResponse::Timeout)` placeholder with:
```rust
let reply = send_raw_report(&payload, …)?;            // Option<Vec<u8>> (already captured)
let expected = expected_cmd_for(&params.command);      // QueryInfo⇒0x01, etc.
Ok(reply.map_or(CommandResponse::Timeout, |b| classify_response(&b, expected)))
```
So **classify_response is the load-bearing deliverable**; `read_typed_response` is the
item-requested standalone primitive (S2 may use it for a direct-read path or not at all).

## Key design decisions (revised)

### 1. Deliverable set = classify_response + read_typed_response; NO burst_and_read_one
- `classify_response(buf, expected_cmd) -> CommandResponse` — PURE, testable. Adds the
  NEW logic: typed reply (`buf[0]==0x51`) with `buf[1] != expected_cmd` ⇒ `Timeout`
  (stale-reply guard); else delegates to `parse_reply`. **This is what S2 needs.**
- `read_typed_response(interface, expected_cmd, verbose) -> Result<CommandResponse, QmkError>`
  — item contract (option a). `read_timeout(buf, REPLY_READ_TIMEOUT_MS)`; on `Ok(n>0)`
  `classify_response(&buf[..n], expected_cmd)`; on `Ok(0)` ⇒ `Ok(Timeout)` (NOT an error);
  on `Err` ⇒ `Err(HidReadError)`. allow-dead (S2 may use it; main flow uses send_raw_report).
- `burst_and_read_one` — **EXCLUDED** (redundant with committed `burst_to_one`-captures).

### 2. Remove parse_reply's `#[allow(dead_code)]`
`classify_response` (allow-dead) calls `parse_reply` — ONE hop. Proven safe by the
`build_typed_payload`→5-constants precedent (a const/fn referenced by an allow-dead fn's
body does NOT warn; documented in core.rs's module comment). So parse_reply's allow is
now a no-op → remove it + update its doc. (Conservative fallback: if a "never used"
warning appears, re-add — but the precedent guarantees it won't.)

### 3. Timeout value: keep 1000ms, use the constant by NAME
Item says "e.g. 100ms". The constant is already 1000ms, de-allowed, and used by the
committed `burst_to_one`. `read_typed_response` references it BY NAME (no hardcode).
P4's QUERY_CALLBACK sweep against a silent device may want to lower it; NOT this task.

### 4. Buffer sizing: `[0u8; REPORT_LENGTH + 1]` (33 bytes)
Matches `burst_to_one`'s read buffer. QMK raw HID uses report ID 0; `read_timeout`
returns report DATA at `buf[0]` (no report-ID prefix on read), so `buf[0]==0x51` for a
typed reply. `parse_reply(&buf[..n])` is correct (contrast the WRITE path which prefixes
`0x00` — that asymmetry is a known hidapi quirk, not a bug).

### 5. hidapi read_timeout semantics
`read_timeout(&mut data, timeout_ms) -> Result<usize, HidError>`: `Ok(n>0)`=read n bytes;
`Ok(0)`=timed out (with `timeout_ms>0` it BLOCKS up to that many ms); `Err`=HID error.
Identical to what the committed `burst_to_one` capture + the drain loop rely on.
ref: https://docs.rs/hidapi/latest/hidapi/struct.HidDevice.html#method.read_timeout

## Why parse_reply already exists (not added by this task)
It was added in an earlier subtask (with `CommandResponse`, P1.M1.T1.S2) + commit
`e0587da` added edge-case tests. This task does NOT touch its body or tests — it adds the
read/classify layer above it and removes its (now-redundant) allow.

## Verification approach (the file is a MOVING TARGET under concurrent edits)
A parallel implementer is actively editing `src/core.rs`. **Do NOT anchor edits to line
numbers** — locate every target by its signature/doc text (which is stable). Before
editing, run `grep -n "fn classify_response\|fn read_typed_response\|fn burst_and_read_one\|fn parse_reply\|fn burst_to_one" src/core.rs` to see what already exists:
- If `classify_response`/`read_typed_response` are ALREADY present (from a parallel
  attempt) AND match the spec below AND the build is clean: the task may already be
  functionally complete — verify (a) `burst_and_read_one` is NOT present (or remove it),
  (b) `parse_reply`'s allow is removed, (c) tests pass. Make only the delta edits needed.
- If `burst_and_read_one` IS present: REMOVE it (it's redundant with the committed
  `burst_to_one`-captures; leaving it risks S2 double-reading).
# PRP — P1.M1.T3.S2: Wire `parse_reply` into `run()` (final v0.3.0 crate API)

> **Crate:** `qmk_notifier` at `/home/dustin/projects/qmk_notifier` (separate
> repo, git-tagged, pinned by QMKonnect per PRD §7/§4). Work in
> `/home/dustin/projects/qmk_notifier`.
> **Files:** `src/lib.rs` (PRIMARY) + `src/core.rs` (the `parse_reply`
> dead-code allow + stale doc/comment tidy) + `README.md` (Mode A docs).
> **Scope line:** `run()` stops DISCARDING the reply and returning placeholders;
> it now DECODES the reply bytes already captured by `send_raw_report` (returns
> `Option<Vec<u8>>` since commit `71248cd`) via `parse_reply` into the real
> `CommandResponse`. Removes `parse_reply`'s `#[allow(dead_code)]`, updates docs.
> **This is the final crate API surface for v0.3.0.**

---

## ⚠️ READ FIRST — the item description is STALE; the architecture is ahead of it

The item says `run()` "currently returns `Result<(), QmkError>`" and frames this
as a "BREAKING API change". **Both are false as of the committed HEAD.** Before
writing any code, re-verify with:

```bash
cd /home/dustin/projects/qmk_notifier
git log --oneline -8                       # expect "Evolve run() to return CommandResponse" + "Evolve burst_to_one to capture first reply"
grep -n "pub fn run" src/lib.rs            # expect: pub fn run(...) -> Result<CommandResponse, QmkError>
grep -n "pub fn send_raw_report" src/core.rs
sed -n '/pub fn send_raw_report/,/-> Result/p' src/core.rs   # expect: -> Result<Option<Vec<u8>>, QmkError>
```

**What is ALREADY done (do NOT redo):**
1. `run()`'s signature is ALREADY `Result<CommandResponse, QmkError>` (commit `a56465b`).
2. `CommandResponse` enum + `RunCommand` typed variants + `HostOs` exist (P1.M1.T1).
3. `build_typed_payload` exists and is wired into `run()`'s typed arm (P1.M1.T2).
4. `send_raw_report` ALREADY returns `Result<Option<Vec<u8>>, QmkError>` — the
   reply is captured INTERNALLY by `burst_to_one`'s bounded
   `read_timeout(REPLY_READ_TIMEOUT_MS)` read (commit `71248cd`).
5. `parse_reply(&[u8]) -> CommandResponse` EXISTS and is fully tested (14 tests).

**What is NOT done (THIS task):** `run()` currently calls `send_raw_report(...)?`
and **discards** the returned `Option<Vec<u8>>`, returning PLACEHOLDER values
(`Legacy{matched:true}` for SendMessage; `Timeout` for typed). `parse_reply` is
`#[allow(dead_code)]` because nothing calls it in non-test code. S2 is the
**small, surgical wiring** that decodes the already-captured reply.

> **Concurrency note:** P1.M1.T3.S1 ("implementing", in parallel) edits
> `src/core.rs` too. The file was observed changing between research reads
> (S1's `read_typed_response`/`burst_and_read_one`/`classify_response` appeared
> then disappeared). Therefore **re-read the current file state before each
> edit** and match on the distinctive TEXT FRAGMENTS given below (not line
> numbers, which drift). The edits below use the committed architecture
> (`send_raw_report`→`parse_reply`) and are robust regardless of S1's final state.

---

## Goal

**Feature Goal**: Close the last gap in the v0.3.0 typed-command round-trip: make
`run()` DECODE the device reply it already captures (via `send_raw_report`'s
built-in `burst_to_one` read) into the correct `CommandResponse` variant, instead
of discarding it and returning a placeholder. After S2, `run(QueryInfo)` against
typed-capable firmware returns `Info{…}`; `run(SendMessage)` against legacy
firmware returns `Legacy{matched: response[0]==1}`; a silent/offline device
returns `Timeout`. This is the final crate API surface for v0.3.0.

**Deliverable**: `src/lib.rs` whose `run()` (a) SendMessage arm and (b) collapsed
typed arm both capture `send_raw_report`'s `Option<Vec<u8>>` reply and decode it
via `core::parse_reply` (`None ⇒ Timeout`); (c) ListDevices arm unchanged
(`Timeout`, honest "nothing was sent"); `run()`'s rustdoc updated (no longer
"placeholder"). `src/core.rs` with `parse_reply`'s `#[allow(dead_code)]` REMOVED
(`run()` is now its consumer), its doc "Consumer" sentence corrected, and the
stale module-comment references to the non-existent `classify_response`/
`read_typed_response`/`burst_and_read_one` tidied. `README.md` "Programmatic
Usage" example updated to the new `Result<CommandResponse,…>` return.

**Success Definition**: `cargo build` compiles with **zero warnings** (critically:
NO "function `parse_reply` is never used" — its allow is gone because `run()`
calls it); `cargo clippy --lib` introduces none; `cargo fmt --check` exits 0;
`cargo test --lib` passes with all existing tests green and **unchanged** (the
dispatch tests still see `DeviceNotFound` before `parse_reply` is reached, so they
assert identically); `cargo doc --lib --no-deps` renders clean. `CommandResponse`
/`RunCommand`/`HostOs`/`RunParameters`/`parse_cli_args`/`build_typed_payload`/
`send_raw_report`/`burst_to_one`/`error.rs`/`main.rs`/`Cargo.toml` all unchanged
except the documented doc/allow tidy in `core.rs`.

## User Persona (if applicable)

**Target User**: The QMKonnect **P4** pipeline — specifically the handshake
(`P4.M2.T1.S1`: `run(QueryInfo)` → check `proto_ver==2 && flags&0x01` →
`run(QueryCallback(i))` sweep → name→id map) and the host-context send
(`P4.M3.T1.S1`: `run(ApplyHostContext{…})` → check `Ack{ok}`).

**Use Case**: `run(RunParameters{ command: RunCommand::QueryInfo, vid, pid, page,
usage, verbose })` → typed arm builds `[0xF0,0x01,0x03]` → `send_raw_report`
burst-writes it and captures the first reply → S2's glue hands the bytes to
`parse_reply` → typed-capable firmware replied `[0x51,0x01,proto,flags,count,board]`
⇒ returns `CommandResponse::Info{…}` → the P4 handshake reads `proto_ver`/`flags`.
A legacy device sends no typed reply ⇒ `None` ⇒ `Timeout` ⇒ handshake stays
string-only (PRD §10.2).

**User Journey**: Pre-S2, `run()` returns `Timeout`/`Legacy{matched:true}`
placeholders no matter what the device said. Post-S2, `run()` returns the REAL
reply. P4 can finally branch on `CommandResponse` variants.

**Pain Points Addressed**: Removes the "we captured the reply but threw it away"
gap. Makes the crate's v0.3.0 promise (`run() → CommandResponse`) actually true
end-to-end, not just at the type level.

## Why

- This is the **wire-up half** of M1.T3 "Response Parsing & run() Return Type
  Change". The reply CAPTURE landed in commit `71248cd` (inside
  `send_raw_report`/`burst_to_one`); the reply DECODE (`parse_reply`) landed in
  commit `ceb08c6`. S2 connects them at the `run()` boundary. After S2 the
  typed-command round-trip is complete on the host side (firmware lands in P1.M2).
- It is **minimal and additive to the dispatch**: it reuses the committed
  `send_raw_report → Option<Vec<u8>>` capture and the tested `parse_reply` decode.
  NO new send path, NO new cache/retry logic, NO new read primitive. The change
  per arm is ~3 lines (capture the `Option`, `map_or(parse_reply)`).
- It **lifts the last `#[allow(dead_code)]`** in the parse path: `parse_reply`
  becomes reachable from compiled (non-test) code via `run()`. This is the
  dead-code gate the whole M1.T3 chain has been staged toward.
- It does **NOT** touch qmkonnect: `QmkNotifier::notify` already matches `Ok(_)`
  and discards the result (PRD §4.3); adapting qmkonnect to *consume*
  `CommandResponse` is P4 (`send_command()` trait method), explicitly out of
  scope for the crate.

## What

All edits are surgical. **Re-read the current file before each edit** (S1 may have
moved text); match on the distinctive fragments, not line numbers.

### lib.rs — Edit A: SendMessage arm (decode the legacy reply)

The SendMessage arm currently discards the reply and returns a placeholder. FIND
the arm's tail (the stale comment + the `send_raw_report(...)?;` discard + the
placeholder return — the distinctive fragment is the stale comment `// send_raw_report STILL returns Result<(), QmkError> at this stage`):

```rust
            // send_raw_report STILL returns Result<(), QmkError> at this stage
            // (its evolution to Result<Option<Vec<u8>>, QmkError> is P1.M3.T2).
            // On success we return the placeholder Legacy{matched:true}; the
            // real response[0] match-bool is decoded in P1.M3.T3 via parse_reply.
            send_raw_report(
                &input_with_terminator,
                params.vendor_id,
                params.product_id,
                params.usage_page,
                params.usage,
                params.verbose,
            )?;

            Ok(CommandResponse::Legacy { matched: true })
```

REPLACE WITH:

```rust
            // send_raw_report returns the FIRST captured device reply as
            // Option<Vec<u8>> (None when no device replied within the bounded
            // read in burst_to_one). Decode it via parse_reply: for a legacy
            // device the reply is the match-bool (response[0] ∈ {0,1}) ⇒
            // Legacy{matched}; None ⇒ Timeout (no reply captured).
            let reply = send_raw_report(
                &input_with_terminator,
                params.vendor_id,
                params.product_id,
                params.usage_page,
                params.usage,
                params.verbose,
            )?;
            Ok(reply.map_or(CommandResponse::Timeout, |bytes| {
                core::parse_reply(&bytes)
            }))
```

### lib.rs — Edit B: typed arm (decode the typed reply)

The collapsed typed arm currently discards the reply and returns `Timeout`. FIND
(the distinctive fragment is `let payload = core::build_typed_payload(&params.command);`
followed by the discard + `// Placeholder: the typed reply is drained, not captured.`):

```rust
            let payload = core::build_typed_payload(&params.command);
            send_raw_report(
                &payload,
                params.vendor_id,
                params.product_id,
                params.usage_page,
                params.usage,
                params.verbose,
            )?;
            // Placeholder: the typed reply is drained, not captured. Reply
            // capture (P1.M1.T3.S1) replaces this with the real CommandResponse.
            Ok(CommandResponse::Timeout)
```

REPLACE WITH:

```rust
            let payload = core::build_typed_payload(&params.command);
            // send_raw_report returns the FIRST captured device reply as
            // Option<Vec<u8>> (None when no device replied). parse_reply
            // disambiguates generically by response[0]/[1]: a typed-capable
            // device replies [0x51][cmd_echo]… ⇒ Info/CallbackName/Ack; a legacy
            // device walks the typed bytes as a no-match string and replies 0/1
            // ⇒ Legacy; None ⇒ Timeout (non-capable/offline).
            let reply = send_raw_report(
                &payload,
                params.vendor_id,
                params.product_id,
                params.usage_page,
                params.usage,
                params.verbose,
            )?;
            Ok(reply.map_or(CommandResponse::Timeout, |bytes| {
                core::parse_reply(&bytes)
            }))
```

> **The two arms are intentionally NOT collapsed further.** SendMessage builds its
> payload inline (the `0x03`-terminated message bytes); the typed arm builds it
> via `build_typed_payload`. Only the *tail* (capture→`map_or(parse_reply)`) is
> identical — and that is fine; duplicating 4 lines is clearer than threading a
> shared helper through two different payload-construction prefixes. (YAGNI: do
> not extract a `decode(reply)` helper for two call sites.)

### lib.rs — Edit C: ListDevices arm comment (tidy stale forward-ref)

The arm body is unchanged (`Ok(CommandResponse::Timeout)`), but its comment
references stale plan numbers. FIND:

```rust
        RunCommand::ListDevices => {
            list_hid_devices()?;
            // Semantic: no device reply was received — nothing was sent.
            // Real reply capture arrives in P1.M3.T1/T3; ListDevices never sends.
            Ok(CommandResponse::Timeout)
        }
```

REPLACE WITH:

```rust
        RunCommand::ListDevices => {
            list_hid_devices()?;
            // ListDevices never sends over the wire, so no reply is captured —
            // Timeout is the honest "nothing was sent" value (PRD §10.2).
            Ok(CommandResponse::Timeout)
        }
```

> **ListDevices stays `Timeout`.** The item offered `Legacy{matched:false}` "or a
> no-op variant". `Timeout` IS the honest no-op: no bytes were sent, so no reply
> was captured. `Legacy{matched:false}` would falsely imply a match was attempted
> and failed. `Timeout` is semantically correct and requires no behavior change.

### lib.rs — Edit D: `run()` rustdoc bullets (no longer placeholders)

FIND the 3-bullet doc block on `run()` (distinctive fragment: `///   **placeholder** (`):

```rust
/// - [`RunCommand::SendMessage`] → [`CommandResponse::Legacy`] as a
///   **placeholder** (`matched: true`) until real reply parsing lands in
///   P1.M1.T3; the firmware's `response[0]` match-bool will be decoded there.
/// - [`RunCommand::ListDevices`] → [`CommandResponse::Timeout`]: no device
///   reply was captured because nothing was sent over the wire (list-only path).
/// - Typed variants (`QueryInfo`/`QueryCallback`/`SetOs`/`ApplyHostContext`)
///   → build their ETX-terminated payload via `build_typed_payload` and send it
///   through the SAME [`send_raw_report`] path as legacy strings (device cache,
///   multi-report burst-write, IN-drain). The reply is currently DRAINED (not
///   captured) by `burst_to_one`, so a [`CommandResponse::Timeout`] placeholder
///   is returned; reply capture (P1.M1.T3.S1) will replace it with the real
///   typed [`CommandResponse`].
```

REPLACE WITH:

```rust
/// - [`RunCommand::SendMessage`] → [`CommandResponse::Legacy`] `{ matched }`:
///   the firmware's `response[0]` match-bool (`1` ⇒ matched, `0` ⇒ no match),
///   decoded from the reply captured by [`send_raw_report`]. [`CommandResponse::Timeout`]
///   when no device replied.
/// - [`RunCommand::ListDevices`] → [`CommandResponse::Timeout`]: no device
///   reply was captured because nothing was sent over the wire (list-only path).
/// - Typed variants (`QueryInfo`/`QueryCallback`/`SetOs`/`ApplyHostContext`)
///   → build their ETX-terminated payload via `build_typed_payload`, send it
///   through the SAME [`send_raw_report`] path as legacy strings (device cache,
///   multi-report burst-write, reply capture), then decode the captured reply
///   via `parse_reply` into [`CommandResponse::Info`] /
///   [`CommandResponse::CallbackName`] / [`CommandResponse::Ack`] (typed-capable
///   device), [`CommandResponse::Legacy`] (legacy device that walked the typed
///   bytes as a no-match string), or [`CommandResponse::Timeout`] (no reply).
```

### core.rs — Edit E: remove `parse_reply`'s `#[allow(dead_code)]`

FIND the `#[allow(dead_code)]` line directly above `pub(crate) fn parse_reply`:

```rust
#[allow(dead_code)]
pub(crate) fn parse_reply(response: &[u8]) -> crate::CommandResponse {
```

REPLACE WITH (delete the allow line):

```rust
pub(crate) fn parse_reply(response: &[u8]) -> crate::CommandResponse {
```

> **Safe to de-allow:** after Edit A/B, `run()` calls `core::parse_reply(&bytes)`
> in compiled (non-test) code, so it is reachable. `cargo build` must show ZERO
> "never used" for `parse_reply`. (This is the one-hop consumer pattern already
> proven by `build_typed_payload` in P1.M1.T2.S2.)

### core.rs — Edit F: correct `parse_reply`'s stale "Consumer chain" doc

`parse_reply`'s trailing doc currently references the non-existent
`classify_response`/`read_typed_response`/`burst_and_read_one` (a stale half-state
from the parallel S1 work). FIND the distinctive fragment `Consumer chain: [\`classify_response\`]`:

```rust
/// Every field access in the typed path uses defensive `.get(...)` indexing —
/// firmware replies may be truncated, so missing bytes default to `0` rather
/// than panicking. Consumer chain: [`classify_response`] (P1.M1.T3.S1) →
/// [`read_typed_response`] → [`burst_and_read_one`] → the `run()` typed dispatch
/// (P1.M1.T3.S2). The `#[allow(dead_code)]` stays until `run()` goes live (S2
/// lifts it together with the read/classify functions' allows); it is cosmetic
/// now since `classify_response` (allow-dead) calls this.
```

REPLACE WITH:

```rust
/// Every field access in the typed path uses defensive `.get(...)` indexing —
/// firmware replies may be truncated, so missing bytes default to `0` rather
/// than panicking. Consumer: the `run()` SendMessage AND typed-dispatch arms in
/// [`crate::run`] (P1.M1.T3.S2), which feed it the reply bytes captured by
/// [`send_raw_report`]'s [`burst_to_one`] bounded read. Request-side counterpart:
/// [`build_typed_payload`].
```

> If the exact surrounding text differs (S1 may have reworded it), the intent is:
> **delete any `classify_response`/`read_typed_response`/`burst_and_read_one`
> references** and state that `run()` is the consumer. Keep the defensive-`.get()`
> sentence (it documents a real invariant).

### core.rs — Edit G: tidy the stale module-comment block

The constants-section module comment currently lists the non-existent functions
as "remaining allow-dead items". FIND the distinctive fragment
`The remaining allow-dead items are the read/parse`:

```rust
// The 5 command constants (CMD_DISCRIMINATOR, CMD_QUERY_INFO, CMD_QUERY_CALLBACK,
// CMD_SET_OS, CMD_APPLY_HOST_CONTEXT) now have a real consumer:
// `build_typed_payload` (P1.M1.T2.S1) references them in compiled code, so they
// no longer need an `#[allow(dead_code)]` (verified: a const referenced by an
// allow-dead fn's body does NOT warn). RESPONSE_MARKER is consumed by
// `parse_reply` (and by `classify_response`'s echo guard, P1.M1.T3.S1);
// REPLY_READ_TIMEOUT_MS is consumed by `burst_to_one`'s bounded reply capture
// and by `read_typed_response` (P1.M1.T3.S1). So neither carries
// `#[allow(dead_code)]`. The remaining allow-dead items are the read/parse
// FUNCTIONS themselves (`parse_reply`, `classify_response`,
// `read_typed_response`, `burst_and_read_one`), whose consumer is `run()`
// (P1.M1.T3.S2); they drop their allows when run() goes live.
```

REPLACE WITH:

```rust
// The 5 command constants (CMD_DISCRIMINATOR, CMD_QUERY_INFO, CMD_QUERY_CALLBACK,
// CMD_SET_OS, CMD_APPLY_HOST_CONTEXT) have a real consumer: `build_typed_payload`
// (P1.M1.T2.S1) references them in compiled code, so they carry no
// `#[allow(dead_code)]`. RESPONSE_MARKER is consumed by `parse_reply`;
// REPLY_READ_TIMEOUT_MS by `burst_to_one`'s bounded reply capture. `parse_reply`
// (P1.M1.T3) is consumed by `run()` (P1.M1.T3.S2), so none of these carry a
// dead-code allow once run() goes live.
```

### README.md — Edit H: "Programmatic Usage" example + note (Mode A docs)

The example still shows the pre-v0.3.0 `Ok(())` pattern and a "Round B (v0.3.0)
changes `run()`…" future-tense note. S2 lands that change, so update both. FIND:

```rust
use qmk_notifier::{RunParameters, RunCommand, run};

// Send a message — auto-discover (VID/PID = None ⇒ match any QMK keyboard)
let params = RunParameters::new(
    RunCommand::SendMessage("Hello keyboard!".to_string()),
    None,              // vendor_id  (Some(0xFEED) to disambiguate)
    None,              // product_id (Some(0x0000) to disambiguate)
    0xFF60,            // usage_page
    0x61,              // usage
    false,             // verbose
);

match run(params) {
    Ok(()) => println!("Message sent successfully"),   // v0.2.x: run() returns ()
    Err(e) => eprintln!("Error: {}", e),
}
```

> Round B (v0.3.0) changes `run()` to return `Result<CommandResponse, QmkError>`
> and adds typed-command variants (`QueryInfo`, `QueryCallback`, `SetOs`,
> `ApplyHostContext`). See `PRD.md` §10.

REPLACE WITH:

```rust
use qmk_notifier::{RunParameters, RunCommand, CommandResponse, run};

// Send a message — auto-discover (VID/PID = None ⇒ match any QMK keyboard)
let params = RunParameters::new(
    RunCommand::SendMessage("Hello keyboard!".to_string()),
    None,              // vendor_id  (Some(0xFEED) to disambiguate)
    None,              // product_id (Some(0x0000) to disambiguate)
    0xFF60,            // usage_page
    0x61,              // usage
    false,             // verbose
);

// run() returns Result<CommandResponse, QmkError>. The variant depends on the
// command and the device's reply:
match run(params) {
    Ok(CommandResponse::Legacy { matched }) => {
        println!("legacy match-bool reply: matched={matched}");
    }
    Ok(CommandResponse::Info { proto_ver, feature_flags, .. }) => {
        println!("typed-capable: proto {proto_ver}, flags 0x{feature_flags:02X}");
    }
    Ok(CommandResponse::Timeout) => {
        println!("no reply (legacy/offline device)");
    }
    Ok(other) => println!("reply: {other:?}"), // CallbackName | Ack
    Err(e) => eprintln!("Error: {}", e),
}
```

`run()` returns [`Result<CommandResponse, QmkError>`](PRD.md). The variants:
`Legacy { matched }` (legacy string reply, `response[0]` ∈ {0,1}),
`Info { proto_ver, feature_flags, callback_count, board_rules_present }`
(QUERY_INFO), `CallbackName { index, name }` (QUERY_CALLBACK),
`Ack { ok }` (SET_OS / APPLY_HOST_CONTEXT), and `Timeout` (no reply within the
bounded read — a non-capable/offline device). See `PRD.md` §7 and §10.

### Conditional — IF S1 left redundant functions, remove them (clean v0.3.0)

The committed architecture captures the reply INSIDE `send_raw_report`/
`burst_to_one` and decodes it with `parse_reply`. The parallel S1 work
(P1.M1.T3.S1) planned a SEPARATE capture path (`burst_and_read_one` →
`read_typed_response` → `classify_response`). **If those functions + their tests
exist in `src/core.rs` at implementation time, they are now redundant** — `run()`
uses `parse_reply` via `send_raw_report`, not them — and would be dead code in the
v0.3.0 release.

CHECK at implementation start:

```bash
grep -nE "fn (classify_response|read_typed_response|burst_and_read_one)" src/core.rs
grep -nE "fn (classify_response)_" src/core.rs   # their tests, if any
```

- **If ABSENT (expected):** nothing to do. (Edits F/G above already remove the
  stale *references* to them in comments/docs.)
- **If PRESENT:** remove the function definitions, their `#[allow(dead_code)]`
  attributes, and any `classify_response_*` tests (and update any comment that
  still names them). They are unused after Edit A/B route through `parse_reply`.
  Do NOT keep them "for later" — dead code in a tagged release is a liability,
  and the echo-guard hardening they represent can be re-added as a small
  `parse_reply`-internal check in a future task if P4's sweep shows stale-reply
  issues. (Document the removal in the commit message.)

> **Why `parse_reply` and not `classify_response`?** `parse_reply` is committed,
> has 14 passing tests, and reuses the single proven send path (`send_raw_report`
> owns the cache/retry/drain). The `expected_cmd` echo guard in
> `classify_response` defends against a stale reply from a *prior* command
> lingering in the IN buffer — a real but low risk (each burst gets a fresh read
> after the drain). It is a future hardening, not a v0.3.0 correctness
> requirement. `parse_reply` already treats an unknown `response[1]` cmd-echo as
> `Timeout`, which covers the malformed-reply case.

### Success Criteria

- [ ] SendMessage arm captures `send_raw_report`'s reply and returns
      `reply.map_or(CommandResponse::Timeout, |b| core::parse_reply(&b))`.
- [ ] Typed arm (still the collapsed or-pattern) captures `send_raw_report`'s
      reply and returns the same `map_or(parse_reply)` glue.
- [ ] ListDevices arm unchanged (`Ok(CommandResponse::Timeout)`); comment tidied.
- [ ] `run()`'s rustdoc bullets updated (no "placeholder"; lists the real decode).
- [ ] `#[allow(dead_code)]` removed from `parse_reply`; its doc + the module
      comment no longer reference `classify_response`/`read_typed_response`/
      `burst_and_read_one`.
- [ ] If S1's functions existed, they (and their tests) are removed.
- [ ] README "Programmatic Usage" shows the `Result<CommandResponse,…>` match.
- [ ] `cargo build` → zero warnings (no "never used: `parse_reply`").
- [ ] All existing tests pass unchanged; no behavior regression.

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed
> to implement this successfully?"_ — **Yes.** The exact FIND/REPLACE anchors
> (distinctive text fragments that survive S1's line-number drift), the verbatim
> replacement code, the verified `send_raw_report → Option<Vec<u8>>` return
> contract, the `core::parse_reply` qualified-call pattern (proven by
> `core::build_typed_payload`), the dispatch-test non-regression reasoning, the
> conditional S1-cleanup, and the verified build/clippy/fmt/test commands are all
> below. The implementer does not need to read the firmware source — `parse_reply`
> already decodes every reply byte and is tested.

### Documentation & References

```yaml
# MUST READ — the file containing run() (PRIMARY edit target).
- file: /home/dustin/projects/qmk_notifier/src/lib.rs
  why: "Contains run() with the SendMessage arm (Edit A), the collapsed typed arm
        (Edit B), the ListDevices arm (Edit C), run()'s 3-bullet rustdoc (Edit D),
        and the #[cfg(test)] mod tests (verify they stay green — they assert
        DeviceNotFound with bogus VID/PID, reached BEFORE parse_reply). RunCommand/
        HostOs/CommandResponse/RunParameters are the type surface (DO NOT touch)."
  pattern: "run() is `match &params.command { … } -> Result<CommandResponse,QmkError>`.
            The typed arm already calls `core::build_typed_payload(&params.command)`
            then `send_raw_report(...)? ` — mirror that qualified-call style for
            `core::parse_reply(&bytes)` (parse_reply is pub(crate) in mod core,
            NOT re-exported at the crate root — same trap as build_typed_payload)."
  gotcha: "send_raw_report returns Result<Option<Vec<u8>>, QmkError> (NOT Result<(),_>).
           The `?` strips the Result; the Ok payload is Option<Vec<u8>>. Capture it
           with `let reply = send_raw_report(...)?;` then `reply.map_or(..)`."

# MUST READ — the file containing send_raw_report + parse_reply (SECONDARY edit).
- file: /home/dustin/projects/qmk_notifier/src/core.rs
  why: "(a) send_raw_report's return contract (Result<Option<Vec<u8>>, QmkError>) —
        the Option is what run() decodes. (b) parse_reply is the decoder S2 wires
        in (Edit E removes its #[allow(dead_code)]; Edit F fixes its doc; Edit G
        tidies the module comment). (c) burst_to_one shows the capture (read_timeout
        + drain) so you understand WHY the bytes are already there."
  section: "send_raw_report (~line 142), burst_to_one (capture + drain), parse_reply
            (~line 489, the #[allow(dead_code)] to remove)"
  critical: "parse_reply is pub(crate) — call it from run() as core::parse_reply(&bytes).
             Do NOT change parse_reply's body (14 tests depend on it). Do NOT change
             send_raw_report/burst_to_one/build_typed_payload (the capture path is
             done). ONLY remove the allow + fix the doc/comment."

# MUST READ — the README (Mode A docs edit).
- file: /home/dustin/projects/qmk_notifier/README.md
  why: "The 'Programmatic Usage' section shows the stale Ok(()) match + a future-tense
        'Round B (v0.3.0) changes run()' note. S2 lands that change → update the
        example to match on CommandResponse variants and drop the future-tense note."
  section: "Programmatic Usage"

# MUST READ — the previous subtask's PRP (the CONTRACT for the reply-capture layer).
- file: /home/dustin/projects/qmkonnect/plan/002_637d65b6e9b8/P1M1T3S1/PRP.md
  why: "S1 planned classify_response/read_typed_response/burst_and_read_one. The
        committed architecture instead evolved send_raw_report to capture the reply
        directly (commit 71248cd) — making S1's separate capture path redundant.
        This PRP's 'Conditional' section handles whichever state S1 leaves. Read S1's
        PRP to know what functions/tests to look for and remove if present."
  critical: "S2 does NOT depend on S1's functions. run() uses parse_reply via
             send_raw_report (both committed). If S1's functions exist, REMOVE them
             (dead code); if absent, just tidy the stale references (Edits F/G)."

# REFERENCE — the wire contract (canonical reply byte layouts parse_reply decodes).
- file: /home/dustin/projects/qmk_notifier/plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md
  why: "§Reply Disambiguation: response[0]==0x51 ⇒ typed (decode by echo at [1]); 0/1
        ⇒ legacy; no reply ⇒ timeout — exactly what parse_reply implements and run()
        now surfaces. §Field Definitions: per-cmd reply layouts (Info/CallbackName/
        Ack). §Constants: RESPONSE_MARKER=0x51, cmd ids."
  section: "Reply Disambiguation, Field Definitions, Constants"

# REFERENCE — research notes for THIS subtask (design decisions + the stale-item analysis).
- docfile: plan/002_637d65b6e9b8/P1M1T3S2/research/notes.md
  why: "Documents: (1) the item description is stale (signature already changed);
        (2) send_raw_report already returns Option<Vec<u8>> (commit 71248cd); (3)
        parse_reply is the tested decoder with a dead-code allow; (4) the dispatch
        tests don't regress (bogus VID/PID ⇒ DeviceNotFound before parse_reply);
        (5) the parse_reply-over-classify_response decision; (6) qmkonnect is not
        modified (P4 owns CommandResponse consumption)."
```

### Current Codebase tree (run from the crate root `/home/dustin/projects/qmk_notifier`)

```bash
qmk_notifier/
├── Cargo.toml          # name="qmk_notifier", version="0.2.1", edition="2021" — DO NOT TOUCH.
├── Cargo.lock
├── README.md           # <-- EDIT H (Programmatic Usage).
├── PRD.md              # crate PRD (§7, §10) — reference only.
├── .gitignore          # /target only
├── plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md   # WIRE SOURCE OF TRUTH
└── src
    ├── main.rs         # binary entrypoint — only SendMessage/ListDevices. DO NOT TOUCH.
    ├── error.rs        # QmkError (DeviceNotFound struct variant, SendReportError(HidError)) — DO NOT TOUCH.
    ├── lib.rs          # <-- PRIMARY EDIT (A-D): run() arms + doc. RunCommand/HostOs/
    │                   #   CommandResponse/RunParameters/parse_cli_args — DO NOT TOUCH.
    └── core.rs         # <-- SECONDARY EDIT (E-G): parse_reply allow removal + doc/
                        #     comment tidy. send_raw_report/burst_to_one/build_typed_payload/
                        #   parse_reply BODY — DO NOT TOUCH. (Conditional: remove S1's
                        #   classify_response/read_typed_response/burst_and_read_one if present.)
```

### Desired Codebase tree with files to be modified

```bash
src/
├── lib.rs   # MODIFIED: SendMessage arm + typed arm decode via parse_reply; ListDevices
│            #   comment tidy; run() rustdoc bullets updated.
└── core.rs  # MODIFIED: -#[allow(dead_code)] on parse_reply; parse_reply doc + module
             #   comment corrected (no classify_response refs). (Conditional: S1 fns removed.)
README.md    # MODIFIED: Programmatic Usage example + note.
# (no new files; main.rs/error.rs/Cargo.toml untouched)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL: send_raw_report returns Result<Option<Vec<u8>>, QmkError>, NOT
//   Result<(),_>. The `?` yields the Option<Vec<u8>>. Capture it:
//     let reply = send_raw_report(...)?;
//     Ok(reply.map_or(CommandResponse::Timeout, |bytes| core::parse_reply(&bytes)))
//   Do NOT write `send_raw_report(...)?;` (that discards the reply — the OLD
//   placeholder behavior this task removes).

// CRITICAL: parse_reply is `pub(crate)` in the PRIVATE `mod core` and is NOT in
//   the `pub use core::{ … }` re-export at the crate root (only send_raw_report /
//   list_hid_devices / parse_hex_or_decimal / DEFAULT_* are). So from run() in
//   lib.rs you MUST call it as `core::parse_reply(&bytes)` — a BARE `parse_reply(…)`
//   fails E0425 "cannot find function". This is the IDENTICAL pattern already used
//   for `core::build_typed_payload(&params.command)` in the typed arm (verified:
//   no `use core::parse_reply;` import exists). Do NOT "fix" it by making the fn
//   `pub` or adding a re-export — qualify the call.

// CRITICAL: parse_reply's BODY is unchanged (14 tests depend on it). This task
//   only REMOVES its #[allow(dead_code)] and fixes its doc. Do not re-implement,
//   rename, or move it. run() is the new consumer; that's the whole change.

// CRITICAL: the existing dispatch tests (test_run_query_info_dispatches_to_send,
//   _query_callback_, _set_os_, _apply_host_context_) use bogus VID/PID
//   (Some(0xDEAD), Some(0xBEEF)) ⇒ send_raw_report returns Err(DeviceNotFound) ⇒
//   the `?` propagates it BEFORE parse_reply is reached. So those tests still
//   assert Err(DeviceNotFound) identically. DO NOT "fix" them — they're correct.

// CRITICAL: re-read the current src/core.rs before Edits E/F/G — S1 (parallel)
//   edits this file. Match on the distinctive TEXT FRAGMENTS (e.g.
//   "Consumer chain: [`classify_response`]", "The remaining allow-dead items"),
//   NOT line numbers. If S1 has landed classify_response/read_typed_response/
//   burst_and_read_one, apply the Conditional removal; if not, just fix the stale
//   references (the functions are named in comments/docs even when absent).

// GOTCHA: ListDevices returns Timeout, NOT Legacy{matched:false}. The item offered
//   both; Timeout is the honest "nothing was sent" value. Legacy{matched:false}
//   would falsely imply a match attempt. Keep Timeout. (The arm needs only a
//   comment tidy, no behavior change.)

// GOTCHA: CommandResponse is non-Copy (it owns Option<String> in CallbackName and
//   Vec data indirectly). In run() the `map_or` closure returns it by value — fine.
//   Don't try to `.clone()` the reply unnecessarily.

// NOTE: run()'s new logic (Option::map_or + parse_reply) is hardware-bound — you
//   cannot unit-test run()'s decode without a real device (send_raw_report needs
//   HID I/O). This EXACTLY matches the existing run() tests (test_run_with_*), which
//   also can't assert the Ok shape in CI. The decode IS verified indirectly:
//   parse_reply's 14 unit tests cover every byte path; the dispatch tests prove
//   run() reaches send_raw_report. No NEW unit test is required (adding one that
//   needs hardware would be skipped/non-deterministic). If you want belt-and-
//   suspenders, the Optional Enhancement below adds a pure glue test.

// OPTIONAL ENHANCEMENT (not required): if you want a pure unit test of the glue,
//   factor the decode into a tiny free fn and test it — BUT this is gold-plating
//   for a 1-line `map_or`. Prefer leaving run() inline (the two call sites are
//   clearer un-extracted). Do NOT add a #[ignore] hardware test — it adds churn.
```

## Implementation Blueprint

### Data models and structure

No new types. S2 connects three EXISTING pieces at the `run()` boundary:
`send_raw_report(...) -> Result<Option<Vec<u8>>, QmkError>` →
`Option::map_or(CommandResponse::Timeout, |b| core::parse_reply(&b))` →
`CommandResponse`. The "structure" is the decode glue, fixed by Edits A–D.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: RE-VERIFY the committed state (S1 edits core.rs concurrently)
  - RUN: cd /home/dustin/projects/qmk_notifier
  - RUN: grep -n "pub fn run" src/lib.rs
         # EXPECT: pub fn run(...) -> Result<CommandResponse, QmkError>
         # (If it STILL says Result<(),_> — STOP; you're on the wrong commit.
         #  The signature change is commit a56465b; rebase/checkout it first.)
  - RUN: sed -n '/pub fn send_raw_report/,/-> Result/p' src/core.rs | head -20
         # EXPECT: -> Result<Option<Vec<u8>>, QmkError>
         # (If it says Result<(),_> — STOP; commit 71248cd is missing.)
  - RUN: grep -n "allow(dead_code)" src/core.rs
         # EXPECT: a hit directly above `pub(crate) fn parse_reply` (the one to
         # remove). If S1 added more allows (classify_response etc.), note them
         # for the Conditional cleanup.
  - RUN: grep -nE "fn (classify_response|read_typed_response|burst_and_read_one)" src/core.rs || echo "S1 fns absent"
         # Decides whether the Conditional removal applies.

Task 2: EDIT src/lib.rs — SendMessage arm (Edit A)
  - REPLACE the stale "send_raw_report STILL returns Result<(),_>" comment + the
          discard + Ok(Legacy{matched:true}) placeholder with the capture +
          map_or(parse_reply) glue (verbatim in Edit A).
  - CHECK: the call is `core::parse_reply(&bytes)` (qualified), the `?` is on
          send_raw_report (not on parse_reply).

Task 3: EDIT src/lib.rs — typed arm (Edit B)
  - REPLACE the discard + Ok(Timeout) placeholder with the capture +
          map_or(parse_reply) glue (verbatim in Edit B). Keep `core::build_typed_payload`
          as-is.
  - CHECK: identical glue shape to Edit A.

Task 4: EDIT src/lib.rs — ListDevices comment + run() doc (Edits C, D)
  - REPLACE the stale "P1.M3" forward-ref comment (Edit C).
  - REPLACE the 3-bullet run() rustdoc (Edit D) — no "placeholder"; list the real
          decode per variant.

Task 5: EDIT src/core.rs — parse_reply allow + docs (Edits E, F, G)
  - DELETE the #[allow(dead_code)] above parse_reply (Edit E).
  - REPLACE parse_reply's "Consumer chain" doc (Edit F) — remove classify_response
          refs; name run() + send_raw_report as consumers.
  - REPLACE the module-comment block (Edit G) — remove the classify_response/
          read_typed_response/burst_and_read_one enumeration.
  - DO NOT touch parse_reply's BODY, send_raw_report, burst_to_one, build_typed_payload,
          or the command constants.

Task 6 (CONDITIONAL): REMOVE S1's redundant functions if present
  - IF Task 1's grep found classify_response/read_typed_response/burst_and_read_one:
          delete those fns + their #[allow(dead_code)] attrs + any classify_response_*
          tests. (They're dead after Tasks 2-3 route through parse_reply.)
  - IF absent: skip (Tasks 5 already removed the stale references).

Task 7: EDIT README.md — Programmatic Usage (Edit H)
  - REPLACE the Ok(()) example + "Round B (v0.3.0)" note with the CommandResponse
          match example + the variants paragraph.

Task 8: VALIDATE (do not skip)
  - RUN (from /home/dustin/projects/qmk_notifier):
          cargo fmt && cargo build && cargo clippy --lib &&
          cargo fmt --check && cargo test --lib
  - EXPECT: build ZERO warnings (NO "never used: parse_reply" — run() calls it;
          NO "never used: REPLY_READ_TIMEOUT_MS" — burst_to_one calls it). clippy
          clean. fmt --check exit 0. All tests pass (existing unchanged).
  - IF "never used: `parse_reply`": run() isn't calling it — check Edits A/B landed
          and reference `core::parse_reply(&bytes)`.
  - IF E0425 "cannot find function `parse_reply`": you called it BARE. Use
          `core::parse_reply(&bytes)`.
  - SANITY: git diff --stat shows src/lib.rs, src/core.rs, README.md (+ S1-fn
          deletions if Task 6 applied).
```

### Implementation Patterns & Key Details

```rust
// === The decode glue (the WHOLE behavioral change, in one idiom) ===
//   send_raw_report returns Option<Vec<u8>>: Some = a reply was captured; None =
//   no reply (timeout/silent). parse_reply decodes bytes → CommandResponse. The
//   glue is Option::map_or(Timeout, parse_reply):
let reply = send_raw_report(&payload, params.vendor_id, params.product_id,
                            params.usage_page, params.usage, params.verbose)?;
Ok(reply.map_or(CommandResponse::Timeout, |bytes| core::parse_reply(&bytes)))

// === WHY parse_reply is generic enough for BOTH arms (no per-variant split) ===
//   parse_reply disambiguates by response[0]: 0x51 ⇒ typed (decode by response[1]
//   cmd-echo → Info/CallbackName/Ack); 0/1 ⇒ Legacy{matched}; empty/unknown ⇒
//   Timeout. A legacy device that receives typed bytes walks them as a no-match
//   string and replies with its 0/1 match-bool — so the typed arm CAN legitimately
//   get a Legacy reply. parse_reply handles this uniformly; run() needs no
//   per-variant branching. (This is why the collapsed or-pattern typed arm stays
//   collapsed: the decode is variant-agnostic.)

// === WHY the dispatch tests don't regress ===
//   test_run_query_info_dispatches_to_send (et al.) use Some(0xDEAD)/Some(0xBEEF).
//   send_raw_report → try_send_once → ensure_cache → open_matching_devices returns
//   Err(DeviceNotFound) (no device matches the bogus VID/PID predicate). The `?`
//   propagates Err(DeviceNotFound) out of run() BEFORE parse_reply is reached. So
//   the assertion `matches!(result, Err(QmkError::DeviceNotFound { .. }))` still
//   holds — the tests are byte-for-byte correct unchanged.

// === WHY not extract a `decode(reply)` helper ===
//   Two call sites, 4 lines each. A helper saves nothing and obscures the (identical)
//   tail behind a name. Inline `map_or(parse_reply)` is clearer. YAGNI.

// === WHY Timeout (not Legacy{matched:false}) for ListDevices ===
//   ListDevices calls list_hid_devices() (enumerate + print, NO write, NO read). No
//   reply exists to decode. Timeout ("no reply captured") is honest;
//   Legacy{matched:false} would imply a match was attempted and failed. The item's
//   "or a no-op variant" ⇒ Timeout is that no-op.
```

### Integration Points

```yaml
SOURCE FILES:
  - modify (primary): "/home/dustin/projects/qmk_notifier/src/lib.rs"
      - SendMessage arm: capture reply → parse_reply (Edit A)
      - typed arm: capture reply → parse_reply (Edit B)
      - ListDevices arm: comment tidy (Edit C)
      - run() rustdoc: 3 bullets updated (Edit D)
  - modify (secondary): "/home/dustin/projects/qmk_notifier/src/core.rs"
      - remove #[allow(dead_code)] from parse_reply (Edit E)
      - parse_reply doc corrected (Edit F)
      - module comment block tidied (Edit G)
      - (CONDITIONAL: remove classify_response/read_typed_response/burst_and_read_one + tests)
  - modify (docs): "/home/dustin/projects/qmk_notifier/README.md"
      - Programmatic Usage example + note (Edit H)

DEPENDENCIES / Cargo.toml:
  - none. No new crate deps.

PUBLIC API SURFACE:
  - run()'s SIGNATURE is unchanged (Result<CommandResponse, QmkError> — already set
    by T1.S2/commit a56465b). Only the dispatch BODY changes (real decode instead of
    placeholders). parse_reply stays pub(crate) (internal decode helper, NOT
    re-exported). CommandResponse/RunCommand/HostOs unchanged.

CONSUMES (treat as fixed, already landed):
  - P1.M1.T1 (Complete): CommandResponse + RunCommand typed variants + HostOs.
  - P1.M1.T2 (Complete): build_typed_payload + the typed dispatch arm.
  - commit 71248cd: send_raw_report → Result<Option<Vec<u8>>, QmkError> (reply capture).
  - commit ceb08c6: parse_reply (tested, currently allow-dead).

DOWNSTREAM CONSUMER (do NOT modify here — listed for awareness):
  - QMKonnect QmkNotifier::notify (src/core/notifier.rs): currently does
    qmk_notifier::run(params) and matches Ok(_) => return Ok(()) (discards the
    result). After S2, Ok(_) still matches (now Ok(CommandResponse)). Adapting
    qmkonnect to CONSUME CommandResponse (send_command() trait method, handshake,
    host-context send) is P4 — explicitly out of scope for the crate.
  - P1.M2: the firmware typed-command handlers (QUERY_INFO/QUERY_CALLBACK/SET_OS/
    APPLY_HOST_CONTEXT) that PRODUCE the replies parse_reply decodes.

OUT-OF-SCOPE (later subtasks — do NOT implement here):
  - P1.M1.T4.S1: bump crate version to 0.3.0 + tag (S2 is the last code change
    before the tag).
  - P4.M1/M2/M3: qmkonnect consumption of CommandResponse (handshake, host-context).
  - P5.M1: CLI flags exposing typed commands (main.rs change).
```

## Validation Loop

> All commands run from the crate root: `/home/dustin/projects/qmk_notifier`

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmk_notifier

# Format the edited files (rustfmt default — no rustfmt.toml exists).
cargo fmt

# Build the whole crate — MUST compile with ZERO warnings.
cargo build 2>&1 | tee /tmp/build.log
# Expected: "Finished `dev` profile ..." and NO "warning:" lines.
#   - If "never used: function `parse_reply`": run() isn't calling it — check
#     Edits A/B landed and use `core::parse_reply(&bytes)` (qualified).
#   - If E0425 "cannot find function `parse_reply` in this scope": you called it
#     BARE. It's pub(crate) in the private `mod core`, NOT re-exported at the crate
#     root. Call it as `core::parse_reply(&bytes)` (same as `core::build_typed_payload`).
#   - If "never used: REPLY_READ_TIMEOUT_MS": you accidentally removed burst_to_one's
#     read — DON'T. burst_to_one still uses it; this task doesn't touch burst_to_one.

# Lint (default clippy — no .clippy.toml exists).
cargo clippy --lib 2>&1 | tee /tmp/clippy.log
# Expected: no new warnings. clippy may suggest match ergonomics on the map_or
#   closure — accept sensible fixes but do NOT change the decode shape.

# Formatting check (CI-style gate).
cargo fmt --check
# Expected: exit code 0 (no diff). If non-zero, re-run `cargo fmt`.

# Sanity: confirm ONLY the expected files changed.
git diff --stat
# Expected: src/lib.rs, src/core.rs, README.md (+ deletions if S1 fns removed).
```

### Level 2: Unit Tests (Component Validation)

```bash
cd /home/dustin/projects/qmk_notifier

# The decode core (parse_reply) — unchanged by S2, must still pass all 14.
cargo test --lib parse_reply_ -- --nocapture
# Expected: 14 passed (info/callback/ack/legacy/empty/unknown/truncated/non-utf8).
#   These are the proof the decode logic is correct; run() just calls it.

# The dispatch tests — UNCHANGED assertions (bogus VID/PID ⇒ DeviceNotFound before
# parse_reply). Must still pass identically.
cargo test --lib test_run_query_info_dispatches_to_send -- --nocapture
cargo test --lib test_run_query_callback_dispatches_to_send -- --nocapture
cargo test --lib test_run_set_os_dispatches_to_send -- --nocapture
cargo test --lib test_run_apply_host_context_dispatches_to_send -- --nocapture
# Expected: each passes (Err(DeviceNotFound) — the `?` propagates before parse_reply).

# The pre-existing run() tests (SendMessage/ListDevices/verbose).
cargo test --lib test_run_with_ -- --nocapture
# Expected: test_run_with_list_devices_command, test_run_with_send_message_command,
#   test_run_with_verbose_output all pass (no regression from the arm changes).

# build_typed_payload tests (unchanged by S2).
cargo test --lib build_typed_payload_ -- --nocapture
# Expected: 9 passed (S1/T2's builder tests).

# Full lib test suite.
cargo test --lib
# Expected: "test result: ok. <N> passed; 0 failed; 0 ignored; ...". The exact N
#   is not load-bearing; the gate is 0 failed. (If you removed S1's
#   classify_response_* tests in Task 6, N drops by 5 — expected.)
```

### Level 3: Integration Testing (System Validation)

```text
PARTIALLY APPLICABLE. run()'s new decode is hardware-bound — a full typed
round-trip needs a QMK keyboard with the v0.3.0 typed-command firmware (P1.M2 —
NOT implemented yet; see firmware_wire_contract.md §Firmware Implementation
Status). WITHOUT such hardware:

  - The dispatch tests (Level 2) prove run() routes to send_raw_report
    (DeviceNotFound with bogus VID/PID); the parse_reply tests prove the decode.
    Together they cover run()'s new logic: the only NEW code is the
    Option::map_or(parse_reply) glue, which is trivial and exercises paths both
    tests already pin.
  - On a dev box WITH a real (legacy, v0.2.x) QMK keyboard:
      cargo run --quiet -- "App\x1DTitle"     # SendMessage → legacy 0/1 reply
    run(SendMessage) now returns Legacy{matched: <response[0]==1>} instead of the
    old placeholder Legacy{matched:true}. (Verify with a println in a scratch test
    or `-v` verbose capture.) Typed commands are NOT reachable via the CLI binary
    yet (main.rs only does SendMessage/ListDevices; CLI flags are P5.M1), so the
    typed-arm decode is verified only by its unit-test coverage at this stage.

  Live-hardware validation of the typed round-trip is deferred to P1.M2 (firmware)
  + P4 (qmkonnect handshake) — out of scope here.
```

### Level 4: Creative & Domain-Specific Validation

```bash
cd /home/dustin/projects/qmk_notifier

# Confirm rustdoc renders (Mode A documentation) for run()'s updated bullets +
# parse_reply's corrected doc + the module comment.
cargo doc --lib --no-deps 2>&1 | grep -iE "warning|error" || echo "docs clean (good)"

# Confirm run() now calls parse_reply (the dead-code gate this task lifts).
grep -n "core::parse_reply" src/lib.rs
# Expected: two hits — the SendMessage arm and the typed arm.

# Confirm parse_reply no longer carries #[allow(dead_code)] (run() is its consumer).
grep -nB1 "fn parse_reply" src/core.rs
# Expected: NO #[allow(dead_code)] directly above `pub(crate) fn parse_reply`.

# Confirm NO placeholder returns remain in run() (the thing this task removes).
grep -n "CommandResponse::Legacy { matched: true }" src/lib.rs
# Expected: NO hits in run()'s body (the only Legacy{matched:...} is now produced
#   by parse_reply via the reply bytes, not hardcoded in run()).
grep -n "Placeholder: the typed reply" src/lib.rs
# Expected: NO hits (the comment is gone with Edit B).

# Confirm the stale classify_response/read_typed_response/burst_and_read_one
# references are gone from comments/docs (Edits F/G, or Task 6 removal).
grep -nE "classify_response|read_typed_response|burst_and_read_one" src/core.rs || echo "no stale refs (good)"

# Confirm zero dead-code warnings overall.
cargo build 2>&1 | grep -iE "never used|warning" || echo "zero dead-code warnings (good)"
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1 passed: `cargo build` → zero warnings (no "never used: `parse_reply`").
- [ ] Level 1 passed: `cargo clippy --lib` → zero new warnings.
- [ ] Level 1 passed: `cargo fmt --check` → exit 0.
- [ ] Level 2 passed: `cargo test --lib` → all pass, 0 failed.

### Feature Validation

- [ ] SendMessage arm captures `send_raw_report`'s reply and returns
      `reply.map_or(CommandResponse::Timeout, |b| core::parse_reply(&b))`.
- [ ] Typed arm captures the reply and returns the same `map_or(parse_reply)` glue.
- [ ] ListDevices arm returns `Ok(CommandResponse::Timeout)` (unchanged behavior).
- [ ] `run()` rustdoc updated (no "placeholder"; lists Info/CallbackName/Ack/Legacy/Timeout).
- [ ] `#[allow(dead_code)]` removed from `parse_reply`; parse_reply doc + module
      comment corrected (no classify_response/read_typed_response/burst_and_read_one refs).
- [ ] If S1's redundant functions existed, they (and their tests) are removed.
- [ ] README "Programmatic Usage" shows the `Result<CommandResponse,…>` match.
- [ ] Dispatch tests (`test_run_*_dispatches_to_send`) pass UNCHANGED (DeviceNotFound
      before parse_reply); parse_reply's 14 tests pass UNCHANGED.

### Code Quality Validation

- [ ] The decode glue mirrors existing idioms (`?` propagation + `Option::map_or`).
- [ ] No new types, no new deps, no main.rs/error.rs change.
- [ ] The chosen `parse_reply` path is committed+tested; no speculative new decode.
- [ ] Only `src/lib.rs`, `src/core.rs`, `README.md` modified.

### Documentation & Deployment

- [ ] `run()`'s rustdoc (Mode A) reflects the real per-variant decode.
- [ ] `parse_reply`'s doc names `run()` + `send_raw_report` as consumers.
- [ ] README "Programmatic Usage" updated to the v0.3.0 return type + variants.
- [ ] No new environment variables or config.

---

## Anti-Patterns to Avoid

- ❌ Don't treat this as a "signature change" — the item description is STALE.
  `run() -> Result<CommandResponse, QmkError>` is ALREADY the signature (commit
  `a56465b`). Re-verify in Task 1. Only the dispatch BODY changes.
- ❌ Don't write `send_raw_report(...)?;` (discard) — that's the OLD placeholder
  behavior. Capture the `Option<Vec<u8>>`: `let reply = send_raw_report(...)?;`
  then `reply.map_or(Timeout, |b| core::parse_reply(&b))`.
- ❌ Don't call `parse_reply(…)` BARE from `run()` — it is `pub(crate)` in the
  private `mod core` and is NOT re-exported at the crate root. A bare call fails
  E0425. Call it as `core::parse_reply(&bytes)` (identical to the existing
  `core::build_typed_payload(&params.command)` qualified call).
- ❌ Don't change `parse_reply`'s BODY — 14 tests depend on it. This task only
  REMOVES its `#[allow(dead_code)]` and fixes its doc.
- ❌ Don't change `send_raw_report` / `burst_to_one` / `build_typed_payload` — the
  reply capture is DONE (commit `71248cd`). run() consumes the captured bytes.
- ❌ Don't add a per-variant split to the typed arm — `parse_reply` decodes
  generically by `response[0]`/`response[1]` (a legacy device can reply 0/1 to a
  typed command). Keep the collapsed or-pattern.
- ❌ Don't change ListDevices to `Legacy{matched:false}` — `Timeout` is the honest
  "nothing was sent" value. `Legacy{matched:false}` falsely implies a failed match.
- ❌ Don't modify qmkonnect — `QmkNotifier::notify` already matches `Ok(_)` and
  discards the result; adapting it to consume `CommandResponse` is P4, out of
  scope for the crate.
- ❌ Don't keep S1's `classify_response`/`read_typed_response`/`burst_and_read_one`
  "for later" if they exist — they're redundant with the committed `send_raw_report`
  capture + `parse_reply` decode and would be dead code in a tagged release. Remove
  them (Task 6). The echo-guard hardening can be re-added inside `parse_reply` later
  if P4's sweep shows stale-reply issues.
- ❌ Don't use real VID/PID (`0xFEED`/`0x0000`) in any new test — a dev box may have
  a real QMK keyboard, making it non-deterministic. The existing dispatch tests use
  `Some(0xDEAD)`/`Some(0xBEEF)` for deterministic `DeviceNotFound`; follow suit.
- ❌ Don't add a `#[should_panic]` or `#[ignore]` hardware test for run()'s decode —
  it adds churn for no CI value (can't run without a device). The decode is covered
  by parse_reply's 14 unit tests; run()'s wiring by the dispatch tests.
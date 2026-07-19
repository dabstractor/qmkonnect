# Research Notes — P1.M1.T3.S2: Wire `parse_reply` into `run()` (final v0.3.0 API)

Crate: `qmk_notifier` at `/home/dustin/projects/qmk_notifier` (separate repo).
S2 runs in parallel with S1 (P1.M1.T3.S1, "implementing"). The file `src/core.rs`
was observed CHANGING between reads (S1 is actively editing it), so anchors below
use distinctive **text fragments**, not line numbers.

## 1. The item description is STALE — what's already done

The item claims `run()` "currently returns `Result<(), QmkError>`" and that this
is a "BREAKING API change". **False on both counts** — verified against the
committed HEAD (`a56465b`):

- `pub fn run(params: RunParameters) -> Result<CommandResponse, QmkError>` is
  ALREADY the signature (commit `a56465b "Evolve run() to return CommandResponse"`).
- The `CommandResponse` enum + `RunCommand` typed variants + `HostOs` all exist
  (P1.M1.T1, complete).
- `build_typed_payload` exists and is wired into `run()`'s typed arm
  (commits `f946a0e`, `ceb08c6`).

So S2 is NOT a signature change. It is the **reply-decode wiring** that the
item's bullets 3/4/5 actually describe, minus the stale preamble.

## 2. The reply-capture architecture ALREADY exists (commit `71248cd`)

The single most important finding: **`send_raw_report` was already evolved to
return the captured reply bytes.** It is no longer `Result<(), _>`:

```rust
pub fn send_raw_report(data, vid, pid, page, usage, verbose)
    -> Result<Option<Vec<u8>>, QmkError>
//   Ok(Some(bytes)) = all devices burst OK + first device replied within
//                     REPLY_READ_TIMEOUT_MS (the bounded read in burst_to_one).
//   Ok(None)        = burst OK but NO device replied (timeout / legacy silent).
//   Err(DeviceNotFound | PartialSendError | SendReportError) = transport failure.
```

Mechanism (committed, tested):
- `burst_to_one(interface, data, batch_count, verbose) -> (bool, Option<Vec<u8>>)`:
  after the burst-write succeeds, it does `interface.read_timeout(&mut buf,
  REPLY_READ_TIMEOUT_MS)` to capture the FIRST reply, then drains surplus IN
  reports (bounded `IN_DRAIN_MAX`). Returns `(write_success, first_reply_bytes)`.
- `try_send_once` collects `first_reply: Option<Vec<u8>>` (first successful
  device wins) and returns `(SendOutcome, Option<Vec<u8>>)`.
- `send_raw_report` returns that `Option<Vec<u8>>` up to the caller.

**Implication for S2:** `run()` does NOT need to do its own read or call any
`read_typed_response`/`burst_and_read_one`. It already RECEIVES the reply bytes
from `send_raw_report`. S2's only job is to decode them with `parse_reply`.

## 3. `parse_reply` — the decoder (committed, tested, currently dead)

```rust
#[allow(dead_code)]   // <-- S2 REMOVES this (run() becomes its consumer)
pub(crate) fn parse_reply(response: &[u8]) -> crate::CommandResponse
```

- Disambiguates by `response[0]`: `0x51`⇒typed (decode by `response[1]` cmd-echo
  → Info/CallbackName/Ack); `0`⇒`Legacy{matched:false}`; `1`⇒`Legacy{matched:true}`;
  empty/unknown⇒`Timeout`.
- Fully tested: **14 `parse_reply_*` tests** in `mod tests` (info/callback/ack/
  legacy/empty/unknown/truncated/non-utf8).
- It is currently `#[allow(dead_code)]` because `run()` does NOT call it yet —
  only tests do. **This is the central dead-code gate S2 lifts.**
- `parse_reply` is `pub(crate)` in `mod core` (private module, not re-exported at
  crate root). So `run()` (in lib.rs) calls it as `core::parse_reply(&bytes)` —
  the SAME qualified-call pattern already used for `core::build_typed_payload`.

## 4. Current `run()` (lib.rs) — what S2 changes

`run()` match is `match &params.command { … }` (borrow — already fixed in T2.S2).

- **ListDevices arm** → `Ok(CommandResponse::Timeout)`. **Keep.** Semantically
  honest ("no reply captured; nothing was sent"). The item offered
  `Legacy{matched:false}` "or a no-op variant"; `Timeout` IS the honest no-op.
  (Only a stale "P1.M3" forward-ref comment needs tidying.)
- **SendMessage arm** → currently calls `send_raw_report(...)?;` (DISCARDS the
  `Option<Vec<u8>>`) and returns the **placeholder** `Ok(CommandResponse::Legacy
  { matched: true })`. A stale comment claims "send_raw_report STILL returns
  Result<(), _>". S2: capture the reply, `reply.map_or(Timeout, |b|
  core::parse_reply(&b))`. For a legacy device this yields
  `Legacy{matched: response[0]==1}` — exactly the item's bullet 3.
- **Typed arm** (collapsed or-pattern QueryInfo|QueryCallback(_)|SetOs(_)|
  ApplyHostContext{..}) → `core::build_typed_payload(&params.command)` →
  `send_raw_report(&payload, ...)?;` (DISCARDS reply) → placeholder
  `Ok(CommandResponse::Timeout)`. S2: same `map_or(parse_reply)` glue.

The existing dispatch tests (`test_run_*_dispatches_to_send`) use bogus
VID/PID (0xDEAD/0xBEEF) so `send_raw_report` returns `Err(DeviceNotFound)`; the
`?` propagates it BEFORE `parse_reply` is reached. ⇒ **those tests stay green
unchanged** (they assert `Err(DeviceNotFound)`, still true).

## 5. The S1 functions are (currently) ABSENT — but stale refs linger

P1.M1.T3.S1's PRP planned `classify_response` / `read_typed_response` /
`burst_and_read_one` (an echo-guard wrapper + a parallel capture path). As of
this research turn they are **NOT in core.rs** (grep `fn classify_response|fn
read_typed_response|fn burst_and_read_one` → no matches) — because the
**committed** architecture (§2) already captures replies inside
`send_raw_report`/`burst_to_one`, making S1's separate capture path redundant.

BUT two stale references to them linger (S1 left a half-state / the S1 PRP's
intended doc text):
- The module-level constants comment (≈ the "remaining allow-dead items are the
  read/parse FUNCTIONS themselves (`parse_reply`, `classify_response`,
  `read_typed_response`, `burst_and_read_one`)…" block).
- `parse_reply`'s trailing doc sentence ("Consumer chain: [`classify_response`]
  (P1.M1.T3.S1) → [`read_typed_response`] → [`burst_and_read_one`] → …").

S2 MUST tidy both (the referenced fns don't exist). Decisive recommendation:
**use `parse_reply` directly** (committed + 14 tests); the echo-guard is a
hardening nicety, not a v0.3.0 requirement. If S1 later re-lands its functions,
they are redundant with the committed `send_raw_report` capture + `parse_reply`
decode and should be removed for a clean release.

## 6. The downstream caller (qmkonnect) — NO change needed in THIS crate

`QMKonnect::QmkNotifier::notify` (`src/core/notifier.rs`) does
`qmk_notifier::run(params)` and matches `Ok(_) => return Ok(())` (discards the
result). After S2, `Ok(_)` still matches (now `Ok(CommandResponse)` instead of
`Ok(())`-era — but qmkonnect already compiles against the
`Result<CommandResponse,_>` signature from T1.S2; it just ignores the variant).
**Adapting qmkonnect to USE `CommandResponse` is P4** (Notifier trait
`send_command()`), explicitly out of scope for the crate. So S2 touches the
crate only; qmkonnect is referenced for "why", not modified.

## 7. Design decision: `parse_reply` over `classify_response` (echo guard)

| Path | Status | Tests | Reuses cache/retry/drain? | Verdict |
|------|--------|-------|---------------------------|---------|
| `send_raw_report`→reply bytes→`parse_reply` | committed | 14 | YES (send_raw_report owns it) | **PRIMARY** |
| S1's `burst_and_read_one`→`read_typed_response`→`classify_response` | absent/uncommitted | 5 (if landed) | NO (re-implements cache bypass) | redundant → remove if present |

Chose `parse_reply` because it is committed, tested, and uses the single proven
send path. The `expected_cmd` echo guard in `classify_response` defends against
a stale reply from a prior command sitting in the IN buffer — a real (if low)
risk for P4's QUERY_CALLBACK sweep. Documented as an OPTIONAL future hardening;
NOT required for v0.3.0 API correctness.

## 8. README.md (Mode A docs) — the stale example

The "Programmatic Usage" section shows the pre-v0.3.0 pattern:
```rust
match run(params) {
    Ok(()) => println!("Message sent successfully"),   // v0.2.x: run() returns ()
    Err(e) => eprintln!("Error: {}", e),
}
```
plus a note "Round B (v0.3.0) changes run() to return Result<CommandResponse,…>".
S2 lands that change, so update the example to `match run(params) { Ok(resp) =>
match resp { CommandResponse::Legacy{matched} => …, … } }` and document the
variants. Drop the "Round B" future-tense note (it's now present tense).

## 9. Validation approach

- `cargo build` → zero warnings (the big one: no "function `parse_reply` is
  never used" once its `#[allow(dead_code)]` is gone AND run() calls it).
- `cargo clippy --lib` → no new warnings (the `map_or` glue is idiomatic).
- `cargo fmt --check` → exit 0.
- `cargo test --lib` → all pass (existing dispatch tests unaffected — bogus
  VID/PID still yields `DeviceNotFound` before `parse_reply`; parse_reply's 14
  tests unaffected). No NEW unit test is strictly needed: run()'s new logic is
  the trivial `Option::map_or` glue, and it's hardware-bound (can't construct a
  reply without a device) exactly like the existing `test_run_with_*` tests.
- `cargo doc --lib --no-deps` → renders run()'s updated doc + parse_reply's.
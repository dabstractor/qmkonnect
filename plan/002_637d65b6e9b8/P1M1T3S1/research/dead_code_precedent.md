# dead_code reachability — the load-bearing precedent

## The question
S1 adds `read_typed_response` (staged `#[allow(dead_code)]`, consumed by run() in S2).
It references `REPLY_READ_TIMEOUT_MS` and calls `classify_response`. Can we remove
`REPLY_READ_TIMEOUT_MS`'s existing `#[allow(dead_code)]` without a "never used"
warning, given its ONLY non-test user is itself allow-dead?

## Answer: YES — proven by TWO in-codebase precedents

### Precedent 1 — parse_typed_reply (empirical, grep-verified)
- `parse_reply` (core.rs:409) is `#[allow(dead_code)]` (line 408).
- `parse_typed_reply` (core.rs:426) has **NO** allow. Grep confirms it is called ONLY
  by `parse_reply` (line 415) in non-test code (tests call `parse_reply`, not
  `parse_typed_reply` directly).
- `cargo build` is **clean** (zero warnings). ⇒ an allow-dead function's call to a
  callee does NOT warn on the callee.

### Precedent 2 — build_typed_payload → 5 command constants (documented)
core.rs:20-23 comment (verbatim):
> "The 5 command constants (...) now have a real consumer: `build_typed_payload`
> (P1.M1.T2.S1) references them in compiled code, so they no longer need an
> `#[allow(dead_code)]` (**verified: a const referenced by an allow-dead fn's body
> does NOT warn**). Only RESPONSE_MARKER and REPLY_READ_TIMEOUT_MS still carry
> `#[allow(dead_code)]`..."

At that time `build_typed_payload` was allow-dead; its const references dropped the
consts' allows cleanly. **Identical pattern** to S1's
read_typed_response (allow-dead) → REPLY_READ_TIMEOUT_MS (drop allow).

## Conclusion for S1
- **REMOVE** `#[allow(dead_code)]` from `REPLY_READ_TIMEOUT_MS` — one-hop, proven.
  (Verify with `cargo build` after the edit; if a warning appears, re-add — but the
  precedent says it won't.)
- **KEEP** `#[allow(dead_code)]` on `parse_reply` — its new caller `classify_response`
  is reachable only via allow-dead `read_typed_response` (TWO hops, unproven). parse_reply
  is test-reachable anyway, so the allow is cosmetic. S2 removes it when run() goes live.
- New functions `read_typed_response`, `burst_and_read_one`, `classify_response` each
  get their OWN `#[allow(dead_code)]` (consumers land in S2), mirroring build_typed_payload's
  S1→S2 staging exactly.

## Sanity gate for the implementer
After all edits: `cargo build` MUST print zero warnings. If ANY "never used" warning
names REPLY_READ_TIMEOUT_MS / read_typed_response / burst_and_read_one /
classify_response, the corresponding allow was removed/omitted prematurely — re-add it.
The build_typed_payload precedent guarantees REPLY_READ_TIMEOUT_MS is safe to de-allow.
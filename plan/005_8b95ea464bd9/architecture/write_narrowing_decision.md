# Decision Record: Multi-Board Write Narrowing

## Date: Session 005 (Architecture Research)
## Status: DEFER

## Question
Can multi-board write-narrowing (DEVICE_DISCOVERY.md §4.2) land app-side, or does it require a crate change?

## Context
DEVICE_DISCOVERY.md §4.2 wants the write match set to become "Tier-1 AND kind == Capable"
so magic bursts don't go to pure-VIA boards.

## Findings

### 1. The crate's MatchKey is PRIVATE and filter-keyed
- `MatchKey { vid?, pid?, usage_page, usage }` — core.rs:641, **private**
- Not path-keyed. `open_matching_devices` opens ALL devices matching the filter.
- `send_raw_report` (the only public send) takes a MatchKey-equivalent param set.
- There is **NO per-path send API** and **NO per-device send API**.

### 2. App-side narrowing options
- **(a) VID/PID filter narrowing:** Only works when capable boards have a distinct
  VID/PID from VIA boards. Not generally true (many QMK boards share 0xFEED:0x0000).
- **(b) Crate API addition:** Add a path-scoped send or a capability field to MatchKey.
  This is a coordinated crate change — out of scope for a one-repo delta.
- **(c) App-side capability overlay:** Enumerate in-app, classify each candidate,
  but still send via the crate's filter-based API. The crate would still broadcast
  to ALL matching devices — the app can't restrict which handles the crate opens.

### 3. It is already harmless
VIA firmware ignores `0x81 0x9F`-prefixed input (the magic header). Bursts to a
pure-VIA board are silently dropped. The narrowing is a politeness/traffic
optimization, not a correctness requirement.

## Decision
**DEFER.** True per-board narrowing needs a coordinated `qmk-notifier` crate change
(new MatchKey field or path-scoped send), which is out of scope for this one-repo
delta. It is harmless today (VIA ignores magic). Record as a follow-up.

## Follow-up
When the crate adds per-path send or a capability match axis, revisit §4.2.
Until then, the app broadcasts to all Tier-1 matching devices — the one capable
board receives it, any co-present VIA board ignores it.

## Impact on This Delta
- P2 (R-COEX) lands the in-repo invariant (comments + tests) regardless.
- P3 (Settings picker) uses `classify_devices()` for the picker UI only — it
  does NOT affect the write path.
- No additional subtask is added for write-narrowing.
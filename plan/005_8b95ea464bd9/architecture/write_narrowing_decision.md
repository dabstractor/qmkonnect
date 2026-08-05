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

## Confirmation (Session 005, P2.M1.T1.S2)

Re-verified against the pinned crate `qmk-notifier` v0.3.0, rev `f26893e`
(Cargo.toml git-tag pin `?tag=v0.3.0`; Cargo.lock
`#f26893ed92fcb3698eadc13322c10d0f9b1a80c9`). All claims hold:

| Claim | Evidence (rev f26893e) |
|---|---|
| `MatchKey` private + filter-keyed `{vid?,pid?,usage_page,usage}` | `src/core.rs:641` — `struct MatchKey { vendor_id: Option<u16>, product_id: Option<u16>, usage_page: u16, usage: u16 }` (no `pub`; no path field) |
| `open_matching_devices` opens ALL matches | `src/core.rs:723` (filter_map open_device at `:751`) — `device_list().filter(device_matches…).filter_map(|info| info.open_device(api).ok())` |
| Only public send is filter-based | `src/core.rs:172` (`pub fn send_raw_report(data,vid,pid,page,usage,verbose)` builds `MatchKey` internally) + `src/lib.rs:404` (`pub fn run`) |
| Payload builder not app-callable | `src/core.rs:495` — `pub(crate) fn build_command_data` |
| No seize/exclusive API | `grep -rni 'seize'` ⇒ ZERO hits; `grep -rni 'exclusive'` ⇒ only CLI "mutually exclusive" arg-group comments (`lib.rs:231/247/348/920`); hidapi 2.x has no seize |

Exhaustive app-only-mechanism re-check (4 options):
- **(a) VID/PID filter narrowing** — non-general (many QMK boards share
  VID/PID, e.g. `0xFEED:0x0000`);
- **(b) crate API addition** (MatchKey capability field / path-scoped send) —
  coordinated two-repo change, out of scope for this delta;
- **(c) app-side capability overlay** (`classify_devices`, P3.M1) — classifies
  for the picker/status but STILL broadcasts via `run()`/`send_raw_report()`;
- **(d) app re-implements the transport** (opens hidapi + writes raw) — NOT
  clean: duplicates `burst_to_one` framing/cache/read-drain, DRY violation,
  splits the R-COEX surface.

All four rejected ⇒ no clean app-only write-narrowing mechanism exists.

§8 "prefer that" reconciled: DEVICE_DISCOVERY.md §8's closing note
("classification can live entirely in qmkonnect … prefer that") refers to
CLASSIFICATION (the picker + three-state status, P1/P3 via `classify_devices()`),
NOT the write path. The write path still bursts to every filter-matching device
through `qmk_notifier::run()` / `send_raw_report()`. It does NOT contradict
DEFER — "classify app-side" (fine, planned) ≠ "narrow writes app-side"
(impossible without a crate change).

Verdict: **DEFER confirmed.** Narrowing needs a coordinated crate change
(tracked by the Follow-up note above); it is harmless today (VIA firmware
ignores `0x81 0x9F`-prefixed input — DEVICE_DISCOVERY.md §6.4). No code change
in this delta; no follow-on subtask added; P2 closes with S1 (code) + S2 (this
confirmation).
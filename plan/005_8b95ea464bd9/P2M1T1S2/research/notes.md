# Research Notes — P2.M1.T1.S2: Confirm the write-narrowing DEFER decision

> Companion to `../PRP.md`. This task is a **decision-confirmation / research
> record** task, NOT a code task. The deliverable is a *Confirmation* section
> appended to
> `plan/005_8b95ea464bd9/architecture/write_narrowing_decision.md`. **No source
> code changes; no `docs/*.md` (Mode A).** This file records the re-verification
> of the decision record's claims against the **pinned** crate source and the
> exhaustive search for a clean app-only write-narrowing mechanism.

---

## 1. The decision under review (verbatim)

`plan/005_8b95ea464bd9/architecture/write_narrowing_decision.md`, Status: **DEFER**.

- **Question:** Can multi-board write-narrowing (DEVICE_DISCOVERY.md §4.2) land
  app-side, or does it require a crate change?
- **Finding 1:** The crate's `MatchKey { vid?, pid?, usage_page, usage }`
  (core.rs:641) is **private** and **filter-keyed** (not path-keyed).
  `open_matching_devices` opens ALL devices matching the filter. There is NO
  per-path send API and NO per-device send API.
- **Finding 2 (3 app-side options):** (a) VID/PID narrowing — only works with
  distinct VID/PID, not general; (b) crate API addition — coordinated crate
  change, out of scope; (c) app-side capability overlay — still broadcasts via
  the crate's filter API.
- **Finding 3:** It is harmless today — VIA firmware ignores `0x81 0x9F`-prefixed
  input; narrowing is a politeness/traffic optimization, not correctness.
- **Decision:** DEFER. Needs a coordinated crate change. No subtask added.
  P2 lands the in-repo R-COEX invariant (S1) regardless; P3 uses
  `classify_devices()` for the picker UI only (not the write path).

---

## 2. Re-verification against the PINNED crate source (Cargo.lock-pinned rev)

The crate is git-tagged `v0.3.0` (Cargo.toml:18), which Cargo.lock pins to rev
**`f26893e`** (`git+...?tag=v0.3.0#f26893ed92fcb3698eadc13322c10d0f9b1a80c9`).
Its checked-out source lives at:
`/home/dustin/.cargo/git/checkouts/qmk-notifier-1f15950b695d9922/f26893e/`

Every claim in the decision record was re-checked against this exact rev. **All
hold.** Below: the claim, the exact source location, and the evidence.

| Decision-record claim | Source location | Evidence (verified) |
|---|---|---|
| `MatchKey` is **private** + filter-keyed `{vid?,pid?,usage_page,usage}` | `core.rs:641` | `struct MatchKey { vendor_id: Option<u16>, product_id: Option<u16>, usage_page: u16, usage: u16 }` — no `pub` keyword ⇒ file-local private. No path/serial field. ✓ |
| `open_matching_devices` opens **ALL** matches, no per-path/per-device scope | `core.rs:723` | `fn open_matching_devices(api, key) -> Result<Vec<HidDevice>>`: filters `api.device_list()` by `device_matches(...)` then `device_infos.into_iter().filter_map(\|info\| info.open_device(api).ok()).collect()` — opens every match. No path selector. ✓ |
| `send_raw_report` (the only public send) takes a MatchKey-equivalent param set | `core.rs:172` | `pub fn send_raw_report(data, vendor_id: Option<u16>, product_id: Option<u16>, usage_page: u16, usage: u16, verbose) -> Result<Option<Vec<u8>>, QmkError>`; its body builds `let key = MatchKey { vendor_id, product_id, usage_page, usage }` — filter-based, no device/path handle. ✓ |
| The payload builder is **`pub(crate)`** (not callable app-side) | `core.rs:495` | `pub(crate) fn build_command_data(command: &crate::RunCommand) -> Vec<u8>` — `pub(crate)`, NOT re-exported. ✓ |
| Public surface is filter-based only (no per-device/path send, no seize) | `lib.rs:1-7` | `pub use core::{ list_hid_devices, parse_hex_or_decimal, send_raw_report, DEFAULT_PRODUCT_ID, DEFAULT_USAGE, DEFAULT_USAGE_PAGE, DEFAULT_VENDOR_ID, REPORT_LENGTH };` + `pub fn run(params) -> Result<CommandResponse, QmkError>` (`lib.rs:404`). No path-scoped send, no device-handle send. ✓ |
| No seize/exclusive open API anywhere | whole crate | `grep -rni 'seize\|exclusive\|O_EXCL\|exclusive_lock'` over `src/` returns **only** CLI arg-group comments ("mutually exclusive" selectors at lib.rs:231/247/348/920) — **zero** HID-seize calls. hidapi 2.x exposes no seize. ✓ |

**Conclusion of §2:** the decision record's factual basis is accurate and current
as of the pinned rev. The crate gives the app no lever to restrict which of the
filter-matching handles a burst lands on.

---

## 3. Exhaustive search for a clean app-only write-narrowing mechanism

The decision record listed 3 options. A 4th ("the app re-implements the
transport") was considered and rejected. **No clean app-only mechanism exists.**

### Option (a) — VID/PID filter narrowing via `send_raw_report`
Set `vendor_id`/`product_id` on the `RunParameters` so the crate's filter
excludes the VIA board. **Only works when the capable board has a distinct
VID/PID from the co-present VIA board.** Many QMK boards ship `0xFEED:0x0000`
(or the default `0xFF60`/`0x61` usage) regardless of firmware flavor, so this is
NOT generally true. → **Rejected as non-general.** (Matches decision record.)

### Option (b) — Coordinated crate API addition
Add a `kind`/capability field to `MatchKey`, or expose a path-scoped send, in
the `qmk-notifier` crate. This is a **two-repo change** (crate tag bump + app
Cargo.toml pin bump) and is explicitly out of scope for this one-repo delta.
DEVICE_DISCOVERY.md §8 itself flags this: the "Multi-board broadcast" row lists
the file as "`src/core/notifier.rs` **(+ crate `MatchKey`)**". → **Rejected as
out of scope.** (Matches decision record.)

### Option (c) — App-side capability overlay (classify, but still send via the crate)
`classify_devices()` (P3.M1.T1, planned) pings each candidate with
`QUERY_INFO` and tags it `Capable`/`NotQmkNotifier`. **But the app's only device
egress is `qmk_notifier::run(params)` / `send_raw_report(...)`, both of which
broadcast to ALL filter-matching handles.** The app cannot tell the crate "open
only the capable ones." Classification enriches the picker/status (P1/P3), not
the write path. → **Rejected for the WRITE path.** (Matches decision record.)

> **The §8 "prefer that" nuance — do not misread it.** DEVICE_DISCOVERY.md §8
> ends with: *"If the crate exposes the raw device list + a
> `send_command(QueryInfo, &filter)` (it does — `HOST_RULES.md` §7),
> classification can live entirely in qmkonnect and the crate need not change;
> **prefer that**."* This refers to **where CLASSIFICATION lives** (the picker +
> three-state status — P1/P3), **NOT** to write-narrowing. Classification-in-app
> is option (c) for the *status/picker*; it does nothing to narrow the *write*
> path, which still bursts to every filter match. **The "prefer that" note does
> NOT contradict the DEFER.** The implementing agent MUST record this
> distinction explicitly so a future reader does not revive the question.

### Option (d) — App opens devices directly via its own `hidapi` calls and writes raw
The app **does** already depend on `hidapi` (used by `startup_device_probe` /
`is_device_connected` / the status probe in `src/core/notifier.rs:130/169/219`
for ENUMERATION/probing, not writes). So it is *technically* possible for the
app to `info.open_device(api)` a specific path and `write()` reports itself,
bypassing the crate. **This is NOT clean:**
- It re-implements the crate's `burst_to_one` (the 33-byte stack buffer, the
  `[0x00, 0x81, 0x9F, payload…]` framing, multi-report chunking, the `0x03` ETX
  terminator, the bounded `read_timeout(0)` IN-drain `IN_DRAIN_MAX=32`)
  in the app — a DRY violation and a **drift magnet** (two burst implementations
  must stay byte-identical).
- It bypasses the device cache, re-introducing the per-notification enumerate+open
  cost the cache exists to avoid.
- It would have to re-derive the typed-command framing (`build_command_data`,
  currently `pub(crate)`).
- It muddies the R-COEX boundary (S1 documents the crate's private `burst_to_one`
  as the single open/read-discipline site; a second app-side transport would
  split that invariant across two locations).

→ **Rejected as not-clean** (massive DRY violation, defeats the crate's purpose,
splits the R-COEX surface). This is option (c) taken to its degenerate limit and
is exactly what a "clean mechanism" must avoid.

### Verdict
The 3 decision-record options are exhaustive; option (d) is the only remaining
conceivable app-only path and it is deliberately unclean. **No clean app-only
write-narrowing mechanism exists. DEFER is correct.** No follow-on subtask is
warranted (the remedy is a crate change, tracked by the decision record's
"Follow-up" note for when the crate adds per-path send or a capability match axis).

---

## 4. Harmlessness today (the "why DEFER is safe" leg)

- VIA firmware ignores `0x81 0x9F`-prefixed input (the magic header that
  demultiplexes QMKonnect traffic from VIA's `0x01–0x15` namespace). This is
  stated in the selected PRD content (h3.40 "Acknowledgements"; h3.29 §3.5) and
  in DEVICE_DISCOVERY.md §6.4 (the demux guarantee). So a magic burst to a
  pure-VIA board is silently dropped — no state change, no error.
- Consequently, broadcasting to all Tier-1 filter matches (one capable board +
  any co-present VIA boards) is **correct** today: the capable board acts on the
  message; the VIA board(s) ignore it. Narrowing is purely a politeness/traffic
  optimization (avoiding pointless bursts), not a correctness requirement.

---

## 5. Scope & boundary notes

- **No source code change.** This task touches ZERO `.rs` files. `cargo build`
  and `cargo test` are **unchanged** by this task (re-running them is a no-op
  proof of "no behavior change," not a real validation step).
- **Single artifact:** an appended `## Confirmation (Session 005, P2.M1.T1.S2)`
  section in `plan/005_8b95ea464bd9/architecture/write_narrowing_decision.md`.
  Mode A — no `docs/*.md`, no README.
- **Parallel-task boundary:** P2.M1.T1.S1 edits `src/core/notifier.rs` ONLY
  (R-COEX comments + `r_coex_invariants` test module). This task (S2) edits the
  architecture decision record ONLY. **Zero file overlap.** (Confirmed by
  reading the S1 PRP, which explicitly defers S2's work.)
- **On DEFER-confirmation, milestone P2 closes with S1 (code) + S2 (this
  confirmation) only.** No additional subtask is added (the work item contract
  says exactly this). The "Follow-up" note already in the record tracks the
  crate-side remedy.
- **Do NOT modify** `PRD.md`, any `tasks.json`, `prd_snapshot.md`, `.gitignore`,
  or any `.rs` source. The decision record (`plan/005.../architecture/*.md`) is
  the ONLY file this task writes to.

---

## 6. Confidence

- The decision record's 3 factual claims are re-verified line-for-line against
  the Cargo.lock-pinned crate rev `f26893e` (§2). All hold.
- The app-only-option analysis (§3) is exhaustive; the one conceivable extra
  path (app re-implements the transport) is rejected as not-clean.
- The §8 "prefer that" misread-trap is explicitly resolved (classification ≠
  write-narrowing).
- The DEFER is the expected and correct outcome; this task records the
  confirmation and closes the question. No code, no risk.
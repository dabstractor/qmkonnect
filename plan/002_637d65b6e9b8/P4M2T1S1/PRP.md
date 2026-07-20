# PRP — P4.M2.T1.S1: Implement handshake function and global capability state

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This 2-point task adds the **capability
> handshake** (HOST_RULES.md §8(5)) to `src/core/notifier.rs`: a process-global
> `HOST_CAPABLE` flag + `CALLBACK_NAMES` (name→id) map, a `HAS_HANDSHAKED`
> dedup guard, a `pub fn perform_handshake(verbose)` that runs `QUERY_INFO` →
> `SET_OS` → `QUERY_CALLBACK` sweep and populates the map, a best-effort
> `rules.toml` callback-name validator (warns, never fails), public accessors
> for P4.M3.T1.S1 (send logic) / P5.M1 (CLI), and a `reset_handshake_state()`
> for P4.M2.T1.S2 (device-transition re-trigger). Plus a configurable
> `MockNotifier` response queue (so the handshake's reply sequence can be
> scripted in tests) + 8 tests.
>
> **Consumes:** `send_command` on the `Notifier` trait (P4.M1.T1.S1 = Complete),
> `configured_filter()` (notifier.rs:77), `HostOs` / `RunCommand::QueryInfo` /
> `QueryCallback` / `SetOs` / `CommandResponse::Info` / `CallbackName` from the
> **qmk_notifier v0.3.0 crate — pinned at `Cargo.toml:19` (`tag = "v0.3.0"`,
> P4.M1.T2.S1 = Complete) AND the v0.3.0 tag is pushed (P1.M1.T4.S1 = Complete,
> verified via `git tag -l`)** ⇒ **NO compile-gate workaround needed**, and
> `rules::get_rules_paths` / `parse_rules` (P3.M1 = Complete).
>
> **Consumed downstream by:**
> - **P4.M2.T1.S2** ("Integrate handshake into startup and device-status poll")
>   calls `perform_handshake()` near `startup_device_probe` and on a
>   `is_device_connected()` false→true transition, using `reset_handshake_state()`
>   to re-trigger.
> - **P4.M3.T1.S1** (host-context send) gates `APPLY_HOST_CONTEXT` on
>   `host_capable()` and passes `callback_names()` into `rules::evaluate()`.
> - **P5.M1.T1.S1** (`--list-callbacks`) prints `callback_names()` (or "legacy").
>
> **SIBLING NOTE — P4.M1.T2.S2 is COMPLETE** (it was being implemented in parallel
> at research time; it has now landed: `struct PendingMessage` @L257,
> `DebounceState.pending: Option<PendingMessage>` @L268). Its edits live at
> **L257 and below**. This task's handshake block inserts in the **L182–184 band**
> (above all of P4.M1.T2.S2's code) and its mock/test edits are additive/replace
> inside `mod tests` (P4.M1.T2.S2 only APPENDED one carry test there; it did NOT
> touch `MockNotifier::send_command`). **Zero region overlap** — no merge risk.

---

## ⚠️ READ FIRST — no compile gate (G1)

Unlike P4.M1.T1.S1 (which had to temp-override `Cargo.toml` against v0.2.1), the
crate is **already pinned to v0.3.0** (`Cargo.toml:19`, P4.M1.T2.S1 = Complete)
**and** the `v0.3.0` git tag is pushed (P1.M1.T4.S1 = Complete; verified via
`git -C ../qmk_notifier tag -l 'v0.3.0'`). The typed types (`RunCommand::QueryInfo`,
`CommandResponse::Info`, `HostOs`, …) resolve directly from the git tag. **Do NOT
apply any path override** — `cargo build` works as-is. See `research/notes.md` §1.

---

## Goal

**Feature Goal**: Implement the host-side **capability handshake** that, once a
QMK device is connected, sends `QUERY_INFO` and — if the firmware advertises
`proto_ver == 2` + the `APPLY_HOST_CONTEXT` feature bit (`feature_flags & 0x01`) —
sends `SET_OS` once (host is OS-authoritative), sweeps `QUERY_CALLBACK(i)` for
`i in 0..callback_count` to build a name→id map, validates `rules.toml`'s
callback names against that map (warnings only), and sets a global `HOST_CAPABLE`
flag. Legacy/timeout devices leave the flag false (string-only — today's behavior,
bit-for-bit). The handshake runs **at most once per board boot**, deduped by a
`HAS_HANDSHAKED` guard that P4.M2.T1.S2 resets on a real device transition.

**Deliverable** (additions to `src/core/notifier.rs` ONLY — Mode-A rustdoc on
`perform_handshake`; no Cargo, no new files, no CLI/tray, no runner wiring):
1. **Module imports** — add `{BTreeSet, HashMap}` + `{AtomicBool, Ordering}`.
2. **3 process-global statics** — `HOST_CAPABLE: AtomicBool`, `CALLBACK_NAMES:
   Lazy<Mutex<HashMap<String, u8>>>`, `HAS_HANDSHAKED: AtomicBool` (all default
   false/empty).
3. **`pub fn perform_handshake(verbose: bool)`** — the core handshake (QueryInfo →
   capable-match → SetOs → sweep → validate → set flag), idempotent via the
   `HAS_HANDSHAKED` swap guard.
4. **`fn host_os() -> HostOs`** — `cfg!(target_os)` → `HostOs` mapping.
5. **`fn validate_rules_callback_names(verbose: bool)`** — best-effort
   rules.toml validation (warn, never fail).
6. **`fn unknown_callback_names(rules, known) -> Vec<String>`** — the pure,
   testable validation core.
7. **`pub fn host_capable() -> bool`**, **`pub fn callback_names() ->
   HashMap<String, u8>`**, **`pub fn reset_handshake_state()`** accessors/reset.
8. **`MockNotifier` extension** — `MOCK_RESPONSES: Lazy<StdMutex<VecDeque<
   CommandResponse>>>` + `set_mock_responses(Vec)`; `send_command` pops the front
   (fallback `Ack{ok:true}`); cleared in `reset_global_mock()`.
9. **8 tests** in `mod tests` (handshake happy/legacy/non-capable/timeout/dedup/
   reset/anonymous + the pure helper).

**Success Definition**:
- `perform_handshake` on a capable mock populates `HOST_CAPABLE=true` +
  `CALLBACK_NAMES={name:id}` and records the call sequence `[QueryInfo, SetOs(_),
  QueryCallback(0), …, QueryCallback(n-1)]` (SetOs BEFORE the sweep).
- On legacy (`proto_ver != 2`), non-capable (`flags & 0x01 == 0`), `Timeout`, or
  `Err`: `HOST_CAPABLE=false`, map cleared, **only** `QueryInfo` was sent.
- A second `perform_handshake` (no reset) is a **no-op** (dedup); after
  `reset_handshake_state()` it re-runs.
- `cargo build --bin qmkonnect` clean; `cargo test --bin qmkonnect --
  --test-threads=1` green (8 new tests + all existing — the 5
  `test_send_command_*` still get the `Ack{ok:true}` default because the
  response queue is empty when `set_mock_responses` isn't called).
- `git diff --stat` shows `src/core/notifier.rs` ONLY.

## User Persona (if applicable)

**Target User**: the downstream P4 implementers (S2 integration, P4.M3 send logic,
P5.M1 CLI) and, ultimately, the **end user** who gets "rules change without
reflashing": the handshake is what lets QMKonnect discover the keyboard's
capability + callback registry at connect time.

**Use Case**: "When I plug in my QMK keyboard, QMKonnect asks it 'do you speak
typed commands, and what callbacks do you have?' — and if so, remembers the
answer so the host-rules pipeline can drive layers + callbacks without reflashing.
If the keyboard is old firmware, QMKonnect silently stays in string-only mode
(today's behavior)."

**Pain Points Addressed**: today there is no capability discovery — QMKonnect
cannot know whether the connected board supports `APPLY_HOST_CONTEXT`, nor its
callback names, so the entire host-rules send path (P4.M3) has no gate. This task
provides the gate (`host_capable()`) and the registry (`callback_names()`).

## Why

- **PRD §8(5) / HOST_RULES.md §8(5)** — the handshake pseudocode is canonical:
  `run(QueryInfo)` → `Info{proto_ver:2, flags&0x01}` ⇒ `SetOs` + sweep + validate
  + `capable=true`; else `capable=false`. This task IS that handshake.
- **PRD §5.7 (`h3.16`)** — "runs a capability handshake at (re)connect
  (`QUERY_INFO`; gated on `proto_ver == 2`) + a `QUERY_CALLBACK` name sweep, and
  sends `SET_OS` once." This task is the handshake half; P4.M2.T1.S2 wires the
  "(re)connect" trigger.
- **PRD §5 / §8(8)** — backward compatibility: legacy firmware ⇒ string-only,
  board rules unaffected. The handshake's `else ⇒ capable=false` branch delivers
  exactly this (no typed command is ever sent to a non-v2 board).
- **C5 (locked design)** — "capability handshake with graceful fallback (gated on
  `proto_ver == 2`)."
- **Unblocks P4.M3.T1.S1** (the send logic needs `host_capable()` + the
  name→id map) and **P4.M2.T1.S2** (integration needs `perform_handshake` +
  `reset_handshake_state`).

## What

Additive, self-contained state + a function + accessors + mock scriptability in
`src/core/notifier.rs`. No change to `notify()` / the debounce worker / `notify_qmk`
/ the `Notifier` trait / runners / tray / CLI. The handshake is invoked by S2
(this task provides the callable; S2 decides when). The capability decision is
deterministic from the `QUERY_INFO` reply; everything else is graceful fallback.

### Success Criteria
- [ ] **Module imports** add `use std::collections::{BTreeSet, HashMap};` and
      `use std::sync::atomic::{AtomicBool, Ordering};` (G3 — precedence-safe vs the
      test module's own `Ordering` import).
- [ ] **3 statics** exist: `HOST_CAPABLE: AtomicBool = AtomicBool::new(false)`,
      `CALLBACK_NAMES: Lazy<Mutex<HashMap<String, u8>>>` (new empty HashMap),
      `HAS_HANDSHAKED: AtomicBool = AtomicBool::new(false)`, each with Mode-A rustdoc.
- [ ] **`pub fn perform_handshake(verbose: bool)`**: short-circuits if
      `HAS_HANDSHAKED.swap(true, SeqCst)` was already true; else builds
      `configured_filter()`, locks the global notifier, sends `QueryInfo`, and:
      on `Info{proto_ver:2, feature_flags, callback_count, ..}` with `flags & 0x01
      != 0` → `SetOs(host_os())`, sweep `QueryCallback(i)` for `i in
      0..callback_count` into a local map (publish after `drop(n)`), call
      `validate_rules_callback_names(verbose)`, set `HOST_CAPABLE=true`; else →
      `HOST_CAPABLE=false` + clear map.
- [ ] **`fn host_os()`** maps `cfg!(target_os)` → `HostOs` (linux→Linux,
      windows→Windows, **macos→Macos** (G7), else→Unsure).
- [ ] **`fn validate_rules_callback_names(verbose)`**: `get_rules_paths().find(exists)`
      → if none, return; `parse_rules` → on Err warn+return; else warn each name in
      `unknown_callback_names(&rules, &CALLBACK_NAMES clone)` (never fails; never
      downgrades capability).
- [ ] **`fn unknown_callback_names(&RuleSet, &HashMap) -> Vec<String>`**: deduped
      + sorted (`BTreeSet`) names in `callback_rules[].enable`/`.disable` absent
      from the known map.
- [ ] **Accessors** `host_capable() -> bool`, `callback_names() -> HashMap`,
      **`reset_handshake_state()`** (clears all 3 statics) are `pub`.
- [ ] **Mock extension**: `MOCK_RESPONSES: Lazy<StdMutex<VecDeque<CommandResponse>>>`
      + `set_mock_responses(Vec)`; `MockNotifier::send_command` STILL records into
      `MOCK_SEND_COMMAND_CALLS` (G4) THEN pops `MOCK_RESPONSES` front, returning
      `Ok(resp)` or the `Ok(Ack{ok:true})` fallback when empty (G5);
      `reset_global_mock()` clears `MOCK_RESPONSES`.
- [ ] **8 tests** pass (see Implementation Tasks Task 8).
- [ ] `git diff --stat` = `src/core/notifier.rs` ONLY.

## All Needed Context

### Context Completeness Check

_Pass_: an agent with no prior knowledge of this codebase can implement this
using only this PRP, because: (a) the EXACT current anchors (verbatim code + line
numbers for the module imports, `MockNotifier::send_command`, `reset_global_mock`,
the `MOCK_*` statics, the L182-184 insertion band) are in `research/notes.md` §0,
incl. the verbatim text of the two mock-extension sites; (b) the verbatim crate
enum shapes (`RunCommand`, `CommandResponse`, `HostOs` — incl. the `Macos`
lowercase gotcha) are in §1; (c) the canonical handshake pseudocode + the capable
predicate + the "at most once per boot" dedup are in §2; (d) the 7 design decisions
(idempotent fn, local-collect-then-publish, best-effort validation, queue-based
mock, etc.) are resolved with rationale in §3; (e) 9 gotchas are pinned (G1-G9,
incl. the precedence-safe `Ordering` import proof G3 and the backward-compat
queue-default G5); (f) the 8-test plan with verbatim assertions is in §5; (g) the
crate is ALREADY pinned to v0.3.0 AND the tag is pushed so no compile-gate dance
(G1); (h) the disjointness from the now-COMPLETE sibling P4.M1.T2.S2 is established
(insertion band L182-184 is above its PendingMessage(L257)/DebounceState(L264)
edits; mock edits are in a region it doesn't touch).

### Documentation & References

```yaml
# MUST READ — the verbatim research (THIS task's full contract + design + safety proofs)
- file: plan/002_637d65b6e9b8/P4M2T1S1/research/notes.md
  why: "§0 = exact current anchors (line numbers + verbatim code for module imports,
        the send_command mock, reset_global_mock, the MOCK_* statics, the L182-184
        insertion band — incl. the verbatim text of the two mock-extension sites).
        §1 = verbatim v0.3.0 crate enums (incl. HostOs::Macos). §2 = canonical
        handshake pseudocode + capable predicate. §3 = 7 design decisions (D1-D7).
        §4 = 9 gotchas (G1-G9). §5 = the 8-test plan. §6 = validation."

# MUST READ — the spec sources of truth (selected sections are in this PRP's header)
- file: spec/HOST_RULES.md
  why: "§8(5) is the CANONICAL handshake pseudocode (QueryInfo → SetOs → sweep →
        validate → capable; else capable=false; at-most-once-per-boot). §5 (wire
        protocol) defines proto_ver/feature_flags/callback_count/board_rules_present
        + the os_byte table. §9 = rules.toml schema (the validate step references
        callback_rules[].enable/disable names). §8(8) = backward-compat (legacy ⇒
        string-only)."
  section: "§5 (Wire Protocol), §8(5) (Startup handshake + SET_OS), §8(8) (Backward compat)"

# MUST READ — the file THIS task edits (the orchestration home)
- file: src/core/notifier.rs
  why: "contains configured_filter (L77), startup_device_probe (L119), is_device_connected
        (L169, ENDS L182), impl Notifier for QmkNotifier::send_command (L228, the transport
        to call), get_notifier (L285), NOTIFIER static (L249), and the #[cfg(test)] mod
        tests (L453) with MOCK_* statics (L461-464), reset_global_mock (L466), MockNotifier
        (L472) + its send_command (~L510, the mock to extend), reset_test_state (L517).
        INSERT the handshake block at L182-184 (after is_device_connected, before
        impl Notifier for QmkNotifier)."
  pattern: "add 3 statics + perform_handshake + host_os + validate_rules_callback_names +
            unknown_callback_names + 3 accessors in the L182-184 band; extend the mock
            with a response queue (MOCK_RESPONSES) + modify MockNotifier::send_command
            to pop it; +1 clear-line in reset_global_mock; append 8 tests."
  gotcha: "DO NOT touch PendingMessage (L257) / DebounceState (L264) / the worker / notify_qmk
           (L381) — that is the (now-complete) P4.M1.T2.S2 region. DO NOT re-add a path
           override (G1 — crate is already v0.3.0 + tag pushed)."

# MUST READ — the consumer contract (rules validation + downstream evaluate())
- file: src/core/rules.rs
  why: "provides pub get_rules_paths() + pub parse_rules(&Path) + pub RuleSet.callback_rules
        where each CallbackRule has pub enable + pub disable Vec<String>. These are exactly
        what validate_rules_callback_names + unknown_callback_names consume. evaluate()
        (P3.M1.T2.S1) takes &HashMap<String,u8> — the type callback_names() returns."
  section: "get_rules_paths, parse_rules, RuleSet, CallbackRule"

# MUST READ — the qmk_notifier v0.3.0 crate (the types this task codes to)
- file: ../qmk_notifier/src/lib.rs
  why: "RunCommand (L19: QueryInfo/QueryCallback(u8)/SetOs(HostOs)/ApplyHostContext),
        HostOs (L65: Unsure=0/Linux=1/Windows=2/Macos=3/Ios=4 — note Macos lowercase),
        CommandResponse (L86: Info{proto_ver,feature_flags,callback_count,
        board_rules_present}/CallbackName{index,name:Option<String>}/Ack{ok}/Timeout/
        Legacy{matched}). All derive Debug+Clone+PartialEq+Eq."
  pattern: "match on CommandResponse::Info { proto_ver: 2, feature_flags, callback_count,
            .. } with a guard `if feature_flags & 0x01 != 0`; loop
            `for i in 0..callback_count { send_command(QueryCallback(i), &filter) }`."
  gotcha: "the crate resolves via Cargo.toml:19 tag=\"v0.3.0\" (P4.M1.T2.S1 done) and the tag
           is pushed (P1.M1.T4.S1 done). No override."

# MUST READ — the predecessor PRPs (the seams this task builds on)
- file: plan/002_637d65b6e9b8/P4M1T1S1/PRP.md   # send_command trait method + Mock recorder
  why: "P4.M1.T1.S1 added Notifier::send_command + QmkNotifier impl + MockNotifier
        (records into MOCK_SEND_COMMAND_CALLS, returns Ok(Ack{ok:true})). Its PRP
        explicitly anticipated THIS task: 'P4.M2/P4.M3 will extend the mock with
        configurable responses later.' This task adds the configurable response QUEUE."
- file: plan/002_637d65b6e9b8/P4M1T2S2/PRP.md   # DebounceState WindowInfo carry (now COMPLETE)
  why: "P4.M1.T2.S2 added PendingMessage (L257) + widened DebounceState.pending to
        Option<PendingMessage> (L268). This task's insertion band (L182-184) and
        mock/test edits are disjoint. Read to confirm no overlap."

# Reference — the spec wire protocol (os_byte table, has_been_queried rationale)
- file: spec/PROTOCOL.md
  why: "§8 (Typed-Command Namespace) mirrors HOST_RULES §5: the 0x51 reply discriminator,
        proto_ver/feature_flags semantics, and the has_been_queried guard that makes
        'at most once per board boot' safe."
```

### Current Codebase tree (relevant subset)

```bash
src/core/
  types.rs        # WindowInfo (derives Clone, P4.M1.T2.S2). UNCHANGED.
  notifier.rs     # ← THIS TASK EDITS THIS FILE ONLY.
                   #     L77  configured_filter()        (USED, unchanged)
                   #     L119 startup_device_probe()     (S2 calls handshake near here; fn unchanged)
                   #     L169 is_device_connected()      (ends L182; S2 polls; fn unchanged)
                   #     L184 impl Notifier for QmkNotifier::send_command (L228) — CALLED by handshake
                   #     L249 NOTIFIER static / L285 get_notifier()      (USED, unchanged)
                   #     L257 struct PendingMessage / L264 DebounceState  ❌ P4.M1.T2.S2 — DO NOT TOUCH
                   #     L362 WORKER / L381 notify_qmk                    ❌ P4.M1.T2.S2 region
                   #     L453 #[cfg(test)] mod tests
                   #   THIS TASK ADDS (at L182-184 band):
                   #     + HOST_CAPABLE / CALLBACK_NAMES / HAS_HANDSHAKED statics
                   #     + perform_handshake / host_os / validate_rules_callback_names
                   #       / unknown_callback_names / host_capable / callback_names / reset_handshake_state
                   #   AND (in mod tests):
                   #     + MOCK_RESPONSES static + set_mock_responses
                   #     + modify MockNotifier::send_command (pop queue, fallback Ack)
                   #     + 1 clear-line in reset_global_mock + 8 tests
  rules.rs        # P3.M1 (get_rules_paths/parse_rules/RuleSet/evaluate). UNCHANGED (read-only consumer).
  pattern.rs      # P2.M1 matcher. UNCHANGED.
  mod.rs          # UNCHANGED (pub mod notifier long-standing).
Cargo.toml        # L19 = qmk_notifier tag="v0.3.0". UNCHANGED (already pinned — G1).
```

### Desired Codebase tree with files to be changed

```bash
src/core/
  notifier.rs     # MODIFIED (additive) — handshake state + fn + accessors + mock queue + 8 tests.
# EVERYTHING ELSE UNCHANGED. No Cargo, no new files, no rules.rs/pattern.rs/types.rs,
# no platforms/, no runners/ (S2 owns runner wiring), no tray/CLI (P5 owns that).
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (G1 — NO compile gate): Cargo.toml:19 ALREADY pins qmk_notifier tag="v0.3.0"
//   (P4.M1.T2.S1 = Complete) AND the v0.3.0 tag is pushed (P1.M1.T4.S1 = Complete,
//   verified via git tag -l). The typed types resolve from the git tag. Do NOT apply
//   P4.M1.T1.S1's temp path override — it is OBSOLETE. `cargo build` works as-is.

// CRITICAL (G2 — sibling region): P4.M1.T2.S2 is COMPLETE — it added PendingMessage (L257)
//   and widened DebounceState.pending to Option<PendingMessage> (L268). DO NOT touch any
//   of L257-and-below (PendingMessage/DebounceState/worker/notify_qmk). Insert the
//   handshake block at L182-184 (above all of it). Your mock/test edits are additive/
//   replace inside mod tests — no conflict.

// CRITICAL (G3 — Ordering import is precedence-safe): adding
//   `use std::sync::atomic::{AtomicBool, Ordering};` at MODULE top (L1-6) alongside
//   the test module's own `use std::sync::atomic::{AtomicUsize, Ordering};` (L457)
//   does NOT error: (a) explicit imports shadow glob imports (`use super::*`), and
//   (b) both resolve to the IDENTICAL path `std::sync::atomic::Ordering`. No E0252.

// CRITICAL (G4 — keep recording the call log): MockNotifier::send_command MUST still
//   `MOCK_SEND_COMMAND_CALLS.lock().unwrap().push(command.clone());` — P4.M3's
//   ordering assertions depend on it. The response-queue pop is ADDITIONAL, after
//   the push. Do not replace the push with the pop.

// CRITICAL (G5 — backward-compat default): when MOCK_RESPONSES is empty,
//   send_command returns `Ok(CommandResponse::Ack { ok: true })` (the existing
//   default). The 5 test_send_command_* tests never call set_mock_responses, so
//   they are unchanged. The edited method has exactly ONE return statement:
//   `Ok(resp.unwrap_or(CommandResponse::Ack { ok: true }))`.

// GOTCHA (G6 — single-threaded tests): `cargo test --bin qmkonnect --
//   --test-threads=1` (shared STATE/COND/WORKER/NOTIFIER/HANDSHAKE globals).
//   Every handshake test starts with reset_test_state() + reset_handshake_state()
//   + set_notifier(MockNotifier::new()).

// GOTCHA (G7 — HostOs::Macos lowercase): cfg!(target_os="macos") maps to
//   HostOs::Macos (lowercase 'os'), NOT HostOs::MacOS. linux→Linux, windows→Windows,
//   macos→Macos, else→Unsure.

// GOTCHA (G8 — validate reads the REAL platform rules.toml): get_rules_paths() may
//   find a dev's real rules.toml in the test env. That is HARMLESS: warnings print
//   (not asserted), HOST_CAPABLE/state unaffected. Tests assert STATE + call sequence,
//   never stderr. A malformed rules.toml ⇒ warn + skip (never fatal). Do not make the
//   handshake fail on a broken rules.toml.

// GOTCHA (G9 — binary-only crate; Mode-A rustdoc uses ```rust,ignore): there is NO
//   lib.rs. Any example fence in perform_handshake's rustdoc must be ```rust,ignore
//   (a bare ``` runnable doctest would fail `cargo test --doc` / not run on a bin).
//   Match rules.rs/pattern.rs convention.
```

## Implementation Blueprint

### Data models and structure

```rust
// ── Module imports (add to the L1-6 block) ──
use std::collections::{BTreeSet, HashMap};        // HashMap: CALLBACK_NAMES type; BTreeSet: helper
use std::sync::atomic::{AtomicBool, Ordering};    // G3: precedence-safe vs test module's Ordering

// ── 3 process-global statics (insert at the L182-184 band) ──
/// Host-rules capability flag, set by [`perform_handshake`] at (re)connect.
/// `true` ⇒ the connected keyboard advertised `proto_ver == 2` + the
/// `APPLY_HOST_CONTEXT` feature bit (`feature_flags & 0x01`); P4.M3.T1.S1 gates
/// the `APPLY_HOST_CONTEXT` send on this. `false` (default, or legacy/timeout) ⇒
/// string-only mode (today's behavior, bit-for-bit). Read via [`host_capable`].
static HOST_CAPABLE: AtomicBool = AtomicBool::new(false);

/// The keyboard's callback registry as a `name → id` map, populated by the
/// `QUERY_CALLBACK` sweep in [`perform_handshake`]. P4.M3.T1.S1's
/// [`rules::evaluate`] resolves `rules.toml` callback names through it; P5.M1's
/// `--list-callbacks` prints it. Read via [`callback_names`].
static CALLBACK_NAMES: Lazy<Mutex<HashMap<String, u8>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Dedup guard: the handshake runs **at most once per board boot** (the firmware
/// sets `has_been_queried` on the first `QUERY_INFO`). [`perform_handshake`] swaps
/// this to `true` on entry and short-circuits if already set. P4.M2.T1.S2 resets
/// it (via [`reset_handshake_state`]) on a real device transition
/// (`is_device_connected()` false→true) to re-trigger.
static HAS_HANDSHAKED: AtomicBool = AtomicBool::new(false);

// ── perform_handshake + helpers + accessors (same band) ──

/// The host OS, for the `SET_OS` command. Determined at build time from
/// `cfg!(target_os)`; the host is the OS source of truth while connected
/// (HOST_RULES.md §5 C12). Returns [`qmk_notifier::HostOs::Unsure`] on
/// non-Linux/Windows/macOS targets.
fn host_os() -> qmk_notifier::HostOs {
    if cfg!(target_os = "linux") {
        qmk_notifier::HostOs::Linux
    } else if cfg!(target_os = "windows") {
        qmk_notifier::HostOs::Windows
    } else if cfg!(target_os = "macos") {
        qmk_notifier::HostOs::Macos // G7: lowercase 'os' in both cfg and the variant
    } else {
        qmk_notifier::HostOs::Unsure
    }
}

/// Run the host-rules capability handshake against the connected QMK device.
///
/// Sends `QUERY_INFO`; if the reply is `Info { proto_ver: 2, feature_flags,
/// callback_count, .. }` with the `APPLY_HOST_CONTEXT` bit set
/// (`feature_flags & 0x01`), the device is **capable**: send `SET_OS` once (host
/// is OS-authoritative), sweep `QUERY_CALLBACK(i)` for `i in 0..callback_count`
/// into the global [`CALLBACK_NAMES`] `name → id` map, validate `rules.toml`'s
/// callback names against it (warnings only — never fatal), and set
/// [`HOST_CAPABLE`] `true`. Any other reply — legacy (`proto_ver != 2`),
/// non-capable (`flags & 0x01 == 0`), `Timeout`, or a device error — leaves
/// [`HOST_CAPABLE`] `false` and clears the map (string-only mode; today's
/// behavior, bit-for-bit).
///
/// **Idempotent per board boot**: the first call swaps [`HAS_HANDSHAKED`] to
/// `true` and runs; subsequent calls short-circuit. P4.M2.T1.S2 resets the guard
/// (via [`reset_handshake_state`]) on a real device transition to re-trigger.
///
/// `verbose` gates the chatty progress logging (matching `startup_device_probe`'s
/// convention); capability-downgrade and rules-mismatch WARNINGS always print.
///
/// # Example
///
/// ```rust,ignore
/// use qmkonnect::core::notifier;
///
/// // Called by the runner at startup (P4.M2.T1.S2) and on device reconnect:
/// notifier::perform_handshake(verbose);
/// if notifier::host_capable() {
///     // P4.M3.T1.S1: also send APPLY_HOST_CONTEXT per window change.
/// }
/// ```
pub fn perform_handshake(verbose: bool) {
    // Dedup: at most once per board boot (firmware has_been_queried). S2 resets.
    if HAS_HANDSHAKED.swap(true, Ordering::SeqCst) {
        if verbose {
            eprintln!(
                "[{}ms] perform_handshake: already handshaked this session — skipping",
                crate::core::now_ms()
            );
        }
        return;
    }

    let filter = configured_filter();
    let notifier = get_notifier();
    let n = notifier.lock().unwrap();

    match n.send_command(qmk_notifier::RunCommand::QueryInfo, &filter) {
        Ok(qmk_notifier::CommandResponse::Info {
            proto_ver: 2,
            feature_flags,
            callback_count,
            board_rules_present,
        }) if feature_flags & 0x01 != 0 => {
            if verbose {
                eprintln!(
                    "[{}ms] perform_handshake: proto v2 capable (flags={:#04x}, {} callbacks, board_rules={})",
                    crate::core::now_ms(), feature_flags, callback_count, board_rules_present
                );
            }
            // SET_OS once (host is OS-authoritative at connect). Best-effort.
            if let Err(e) = n.send_command(qmk_notifier::RunCommand::SetOs(host_os()), &filter) {
                eprintln!("Warning: SET_OS failed during handshake: {}", e);
            }
            // Callback sweep → local map (publish after dropping the notifier lock: D2).
            let mut local: HashMap<String, u8> = HashMap::new();
            for i in 0..callback_count {
                match n.send_command(qmk_notifier::RunCommand::QueryCallback(i), &filter) {
                    Ok(qmk_notifier::CommandResponse::CallbackName {
                        index,
                        name: Some(name),
                    }) => {
                        local.insert(name, index); // echo the firmware's index for robustness
                    }
                    Ok(qmk_notifier::CommandResponse::CallbackName { name: None, .. }) => {
                        if verbose {
                            eprintln!("[{}ms] perform_handshake: callback {} has no name — skipped",
                                crate::core::now_ms(), i);
                        }
                    }
                    Ok(other) => {
                        if verbose {
                            eprintln!("[{}ms] perform_handshake: callback {} unexpected reply {:?}",
                                crate::core::now_ms(), i, other);
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: QUERY_CALLBACK({}) failed: {}", i, e);
                    }
                }
            }
            drop(n); // release the notifier before the read-only rules validation
            {
                let mut names = CALLBACK_NAMES.lock().unwrap();
                names.clear();
                names.extend(local);
            }
            validate_rules_callback_names(verbose);
            HOST_CAPABLE.store(true, Ordering::SeqCst);
            if verbose {
                eprintln!(
                    "[{}ms] perform_handshake: complete — capable ({} callbacks mapped)",
                    crate::core::now_ms(),
                    CALLBACK_NAMES.lock().unwrap().len()
                );
            }
        }
        Ok(other) => {
            drop(n);
            HOST_CAPABLE.store(false, Ordering::SeqCst);
            CALLBACK_NAMES.lock().unwrap().clear();
            if verbose {
                eprintln!(
                    "[{}ms] perform_handshake: non-capable reply ({:?}) — string-only mode",
                    crate::core::now_ms(),
                    other
                );
            }
        }
        Err(e) => {
            drop(n);
            HOST_CAPABLE.store(false, Ordering::SeqCst);
            CALLBACK_NAMES.lock().unwrap().clear();
            if verbose {
                eprintln!(
                    "[{}ms] perform_handshake: device error ({}) — string-only mode",
                    crate::core::now_ms(),
                    e
                );
            }
        }
    }
}

/// Best-effort validation of `rules.toml` callback names against [`CALLBACK_NAMES`].
///
/// Reads the first existing `rules.toml` candidate ([`rules::get_rules_paths`]);
/// if none exists, host rules are disabled and there is nothing to validate. A
/// malformed `rules.toml` is warned about and skipped (the strict failure is
/// `--validate-rules`'s job, P5.M1) — it never fails the handshake. Unknown
/// callback names (referenced in `[[callback_rules]]` `enable`/`disable` but absent
/// from the keyboard's registry) are warned, one per line. [`HOST_CAPABLE`] is
/// unaffected (a broken rules file does not downgrade capability).
fn validate_rules_callback_names(verbose: bool) {
    let Some(path) = crate::core::rules::get_rules_paths().into_iter().find(|p| p.exists()) else {
        if verbose {
            eprintln!(
                "[{}ms] perform_handshake: no rules.toml found — skipping callback-name validation",
                crate::core::now_ms()
            );
        }
        return;
    };
    let rules = match crate::core::rules::parse_rules(&path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "Warning: could not parse {} ({}) — skipping callback-name validation",
                path.display(),
                e
            );
            return;
        }
    };
    let known = CALLBACK_NAMES.lock().unwrap().clone();
    let unknown = unknown_callback_names(&rules, &known);
    for name in &unknown {
        eprintln!(
            "Warning: rules.toml references callback \"{}\" which is not registered on this keyboard ({} known)",
            name,
            known.len()
        );
    }
    if verbose && !unknown.is_empty() {
        eprintln!(
            "[{}ms] perform_handshake: {} unknown callback name(s) in rules.toml",
            crate::core::now_ms(),
            unknown.len()
        );
    }
}

/// Callback names referenced by `rules.toml` but absent from the keyboard's
/// registry. Deduped + sorted (via `BTreeSet`) for deterministic output. This is
/// the pure, testable core of [`validate_rules_callback_names`].
fn unknown_callback_names(
    rules: &crate::core::rules::RuleSet,
    known: &HashMap<String, u8>,
) -> Vec<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for rule in &rules.callback_rules {
        for name in rule.enable.iter().chain(rule.disable.iter()) {
            if !known.contains_key(name) {
                seen.insert(name.clone());
            }
        }
    }
    seen.into_iter().collect()
}

/// Is the connected keyboard host-rules-capable (`proto_ver == 2` + `flags & 0x01`)?
/// P4.M3.T1.S1 gates `APPLY_HOST_CONTEXT` on this.
pub fn host_capable() -> bool {
    HOST_CAPABLE.load(Ordering::SeqCst)
}

/// The keyboard's `name → id` callback map (a clone). P4.M3.T1.S1 passes this into
/// [`rules::evaluate`]; P5.M1's `--list-callbacks` prints it. Empty when not capable.
pub fn callback_names() -> HashMap<String, u8> {
    CALLBACK_NAMES.lock().unwrap().clone()
}

/// Clear all handshake state (capability flag, callback map, dedup guard).
///
/// Called by P4.M2.T1.S2 on a real device transition (`is_device_connected()`
/// false→true) so the next [`perform_handshake`] re-runs, and by the handshake
/// tests for isolation.
pub fn reset_handshake_state() {
    HOST_CAPABLE.store(false, Ordering::SeqCst);
    CALLBACK_NAMES.lock().unwrap().clear();
    HAS_HANDSHAKED.store(false, Ordering::SeqCst);
}

// ── MockNotifier extension (inside #[cfg(test)] mod tests) ──
use std::collections::VecDeque;   // add near the test-module imports (after `use super::*;`)

// (new static, after MOCK_SEND_COMMAND_CALLS @L464)
static MOCK_RESPONSES: Lazy<StdMutex<VecDeque<qmk_notifier::CommandResponse>>> =
    Lazy::new(|| StdMutex::new(VecDeque::new()));

// (new accessor inside impl MockNotifier)
fn set_mock_responses(responses: Vec<qmk_notifier::CommandResponse>) {
    MOCK_RESPONSES.lock().unwrap().extend(responses);
}

// (MODIFY MockNotifier::send_command — keep the push, add the pop)
fn send_command(
    &self,
    command: qmk_notifier::RunCommand,
    _filter: &DeviceFilter,
) -> Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>> {
    MOCK_SEND_COMMAND_CALLS
        .lock()
        .unwrap()
        .push(command.clone());                                  // G4: keep the log
    let resp = MOCK_RESPONSES.lock().unwrap().pop_front();        // D5: pop scripted reply
    Ok(resp.unwrap_or(qmk_notifier::CommandResponse::Ack { ok: true })) // G5: empty → default
}

// (reset_global_mock — add 1 line before the closing brace)
fn reset_global_mock() {
    MOCK_CALL_COUNT.store(0, Ordering::SeqCst);
    *MOCK_LAST_MESSAGE.lock().unwrap() = None;
    MOCK_SEND_COMMAND_CALLS.lock().unwrap().clear();
    MOCK_RESPONSES.lock().unwrap().clear(); // NEW — transitively cleared by reset_test_state()
}
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: ADD module imports (src/core/notifier.rs L1-6)
  - EDIT: add two lines after the existing `use std::sync::{Arc, Condvar, Mutex};`:
      use std::collections::{BTreeSet, HashMap};
      use std::sync::atomic::{AtomicBool, Ordering};
  - WHY: CALLBACK_NAMES is Lazy<Mutex<HashMap<String, u8>>>; the 3 AtomicBool statics
    need AtomicBool + Ordering; unknown_callback_names uses BTreeSet for sorted dedup.
  - GOTCHA G3: precedence-safe vs the test module's own `Ordering` import (L457) —
    explicit-beats-glob + identical path. No E0252. (If paranoid, run `cargo build`
    immediately after this + Task 2 to confirm; fallback: import only AtomicBool at
    module top and fully-qualify `std::sync::atomic::Ordering::SeqCst` at the 6 sites.)
  - VERIFY: `grep -n 'use std::collections::{BTreeSet, HashMap}\|use std::sync::atomic::{AtomicBool, Ordering}' src/core/notifier.rs` -> 2 hits at the top.

Task 2: ADD the 3 statics + perform_handshake + helpers + accessors (L182-184 band)
  - INSERT: at the blank line L183 (between is_device_connected()'s closing brace at
    L182 and `impl Notifier for QmkNotifier {` at L184), the entire block from Data
    Models (3 statics + host_os + perform_handshake + validate_rules_callback_names +
    unknown_callback_names + host_capable + callback_names + reset_handshake_state),
    each with its Mode-A rustdoc.
  - DEPENDENCIES: configured_filter (L77), get_notifier (L285), crate::core::now_ms
    (core/mod.rs:57), crate::core::rules::{get_rules_paths, parse_rules, RuleSet},
    qmk_notifier::{RunCommand, CommandResponse, HostOs} — ALL already in scope/available
    (qmk_notifier is a crate dep; rules is `pub mod rules`).
  - NAMING: HOST_CAPABLE / CALLBACK_NAMES / HAS_HANDSHAKED (exact, per the item);
    perform_handshake / host_capable / callback_names / reset_handshake_state (pub);
    host_os / validate_rules_callback_names / unknown_callback_names (private).
  - PLACEMENT: the L182-184 band (NOT inside any fn/impl). Items may reference
    get_notifier (defined later at L285) — Rust allows forward item references
    within a module.
  - GOTCHA G2: do NOT place this inside the PendingMessage/DebounceState/worker/notify_qmk
    region (L257+). GOTCHA G7: HostOs::Macos (lowercase). GOTCHA G9: rustdoc ```rust,ignore.
  - VERIFY: `grep -n 'pub fn perform_handshake\|static HOST_CAPABLE\|static CALLBACK_NAMES\|static HAS_HANDSHAKED\|pub fn host_capable\|pub fn callback_names\|pub fn reset_handshake_state\|fn host_os\|fn unknown_callback_names' src/core/notifier.rs` -> 9 hits.

Task 3: EXTEND the MockNotifier with a response queue (inside #[cfg(test)] mod tests)
  - 3a: add `use std::collections::VecDeque;` near the test-module imports (after
    `use std::sync::Mutex as StdMutex;` at L458).
  - 3b: add the static after MOCK_SEND_COMMAND_CALLS (L464):
      static MOCK_RESPONSES: Lazy<StdMutex<VecDeque<qmk_notifier::CommandResponse>>> =
          Lazy::new(|| StdMutex::new(VecDeque::new()));
  - 3c: add the accessor inside `impl MockNotifier` (near get_send_command_calls):
      fn set_mock_responses(responses: Vec<qmk_notifier::CommandResponse>) {
          MOCK_RESPONSES.lock().unwrap().extend(responses);
      }
  - 3d: MODIFY MockNotifier::send_command (~L510): KEEP the
    `MOCK_SEND_COMMAND_CALLS.lock().unwrap().push(command.clone());` block (G4), THEN
    insert `let resp = MOCK_RESPONSES.lock().unwrap().pop_front();` and change the
    final line from `Ok(qmk_notifier::CommandResponse::Ack { ok: true })` to
    `Ok(resp.unwrap_or(qmk_notifier::CommandResponse::Ack { ok: true }))` (G5 — so the
    method has exactly ONE return statement, and the empty-queue default is preserved).
  - GOTCHA G4: do NOT remove/replace the push — P4.M3 ordering assertions need it.
  - GOTCHA G5: empty queue MUST fall back to Ack{ok:true} (preserves the 5 existing tests).
  - VERIFY: `grep -n 'MOCK_RESPONSES\|set_mock_responses\|pop_front' src/core/notifier.rs`
    -> static(1) + accessor(1) + impl-line(1) + pop_front(1).

Task 4: CLEAR the response queue in reset_global_mock (L466)
  - EDIT: add ONE line before reset_global_mock's closing brace (after the
    `MOCK_SEND_COMMAND_CALLS.lock().unwrap().clear();` line):
      MOCK_RESPONSES.lock().unwrap().clear();
  - WHY: reset_test_state() (L517) calls reset_global_mock(), so the queue is cleared
    transitively at the single source of truth (mirrors the send-command-log pattern).
  - VERIFY: `grep -n 'MOCK_RESPONSES.lock().unwrap().clear()' src/core/notifier.rs` -> 1 (in reset_global_mock).

Task 5: MID-POINT build gate
  - RUN: cargo build --bin qmkonnect   (expect clean — G1: no path override needed)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1
    (expect: ALL existing tests still green — the 5 test_send_command_* get the
    Ack{ok:true} default because the queue is empty; the debounce tests + the
    P4.M1.T2.S2 carry test unchanged. No handshake test exists yet, so
    perform_handshake is exercised only by being pub + compiled.)
  - IF a compile error names Ordering/ambiguity: G3 analysis failed — change the
    module-top import to `use std::sync::atomic::AtomicBool;` only and fully-qualify
    `std::sync::atomic::Ordering::SeqCst` in the handshake code (6 sites). (Unlikely.)

Task 6: ADD the 8 handshake tests (append to #[cfg(test)] mod tests, at the END
        of the module — after test_debounced_pending_carries_window_info + the
        other existing tests)
  - DO: append the 8 tests below. Each starts with reset_test_state() +
    reset_handshake_state() + set_notifier(Mock) + set_mock_responses([...]) as
    needed, then perform_handshake(false) + assertions on host_capable()/
    callback_names()/MockNotifier::get_send_command_calls().
  - NAMING: test_handshake_* (7) + test_unknown_callback_names_helper (1).
  - GOTCHA G6: single-threaded. GOTCHA G8: don't assert on stderr (real rules.toml
    may exist in the test env). GOTCHA: for the SetOs assertion use
    `matches!(calls[1], RunCommand::SetOs(_))` (portable) OR
    `calls[1] == RunCommand::SetOs(host_os())` (exact — host_os is reachable via super::*).
  - DO NOT modify the 5 test_send_command_* / the debounce tests / the P4.M1.T2.S2 carry test.

    // 1. capable happy path — populates state + exact call order
    #[test]
    fn test_handshake_capable_populates_state() {
        reset_test_state(); reset_handshake_state(); set_notifier(Box::new(MockNotifier::new()));
        set_mock_responses(vec![
            qmk_notifier::CommandResponse::Info { proto_ver: 2, feature_flags: 0x01, callback_count: 2, board_rules_present: true },
            qmk_notifier::CommandResponse::Ack { ok: true }, // SetOs
            qmk_notifier::CommandResponse::CallbackName { index: 0, name: Some("vim_lazy".into()) },
            qmk_notifier::CommandResponse::CallbackName { index: 1, name: Some("disable_vim".into()) },
        ]);
        perform_handshake(false);
        assert!(host_capable());
        let names = callback_names();
        assert_eq!(names.get("vim_lazy"), Some(&0));
        assert_eq!(names.get("disable_vim"), Some(&1));
        let calls = MockNotifier::get_send_command_calls();
        assert_eq!(calls.len(), 4);
        assert_eq!(calls[0], qmk_notifier::RunCommand::QueryInfo);
        assert!(matches!(calls[1], qmk_notifier::RunCommand::SetOs(_)));
        assert_eq!(calls[2], qmk_notifier::RunCommand::QueryCallback(0));
        assert_eq!(calls[3], qmk_notifier::RunCommand::QueryCallback(1));
    }

    // 2. legacy proto_ver==1 -> string-only, only QueryInfo sent
    #[test]
    fn test_handshake_legacy_proto_v1_string_only() {
        reset_test_state(); reset_handshake_state(); set_notifier(Box::new(MockNotifier::new()));
        set_mock_responses(vec![qmk_notifier::CommandResponse::Info {
            proto_ver: 1, feature_flags: 0x00, callback_count: 0, board_rules_present: true }]);
        perform_handshake(false);
        assert!(!host_capable());
        assert!(callback_names().is_empty());
        let calls = MockNotifier::get_send_command_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], qmk_notifier::RunCommand::QueryInfo);
    }

    // 3. proto v2 but feature_flags & 0x01 == 0 -> not capable
    #[test]
    fn test_handshake_no_feature_flag_string_only() {
        reset_test_state(); reset_handshake_state(); set_notifier(Box::new(MockNotifier::new()));
        set_mock_responses(vec![qmk_notifier::CommandResponse::Info {
            proto_ver: 2, feature_flags: 0x00, callback_count: 3, board_rules_present: true }]);
        perform_handshake(false);
        assert!(!host_capable());
        assert_eq!(MockNotifier::get_send_command_calls().len(), 1);
    }

    // 4. Timeout -> string-only
    #[test]
    fn test_handshake_timeout_string_only() {
        reset_test_state(); reset_handshake_state(); set_notifier(Box::new(MockNotifier::new()));
        set_mock_responses(vec![qmk_notifier::CommandResponse::Timeout]);
        perform_handshake(false);
        assert!(!host_capable());
        assert_eq!(MockNotifier::get_send_command_calls().len(), 1);
    }

    // 5. dedup: second perform_handshake is a no-op (HAS_HANDSHAKED guard)
    #[test]
    fn test_handshake_dedup_idempotent() {
        reset_test_state(); reset_handshake_state(); set_notifier(Box::new(MockNotifier::new()));
        set_mock_responses(vec![
            qmk_notifier::CommandResponse::Info { proto_ver: 2, feature_flags: 0x01, callback_count: 1, board_rules_present: true },
            qmk_notifier::CommandResponse::Ack { ok: true },
            qmk_notifier::CommandResponse::CallbackName { index: 0, name: Some("x".into()) }]);
        perform_handshake(false);
        assert!(host_capable());
        let after_first = MockNotifier::get_send_command_calls().len();
        perform_handshake(false); // MUST short-circuit
        let after_second = MockNotifier::get_send_command_calls().len();
        assert_eq!(after_first, after_second, "dedup: second perform_handshake must not re-send");
        assert!(host_capable());
    }

    // 6. reset_handshake_state clears everything and allows a re-run (the S2 path)
    #[test]
    fn test_handshake_reset_allows_rerun() {
        reset_test_state(); reset_handshake_state(); set_notifier(Box::new(MockNotifier::new()));
        set_mock_responses(vec![
            qmk_notifier::CommandResponse::Info { proto_ver: 2, feature_flags: 0x01, callback_count: 1, board_rules_present: true },
            qmk_notifier::CommandResponse::Ack { ok: true },
            qmk_notifier::CommandResponse::CallbackName { index: 0, name: Some("x".into()) }]);
        perform_handshake(false);
        assert!(host_capable());
        reset_handshake_state();
        assert!(!host_capable());
        assert!(callback_names().is_empty());
        // re-arm + re-handshake (S2's device-gain path)
        set_mock_responses(vec![
            qmk_notifier::CommandResponse::Info { proto_ver: 2, feature_flags: 0x01, callback_count: 1, board_rules_present: true },
            qmk_notifier::CommandResponse::Ack { ok: true },
            qmk_notifier::CommandResponse::CallbackName { index: 0, name: Some("y".into()) }]);
        perform_handshake(false);
        assert!(host_capable());
        assert_eq!(callback_names().get("y"), Some(&0));
    }

    // 7. name:None callback is skipped silently (no panic)
    #[test]
    fn test_handshake_skips_anonymous_callback() {
        reset_test_state(); reset_handshake_state(); set_notifier(Box::new(MockNotifier::new()));
        set_mock_responses(vec![
            qmk_notifier::CommandResponse::Info { proto_ver: 2, feature_flags: 0x01, callback_count: 2, board_rules_present: true },
            qmk_notifier::CommandResponse::Ack { ok: true },
            qmk_notifier::CommandResponse::CallbackName { index: 0, name: None },
            qmk_notifier::CommandResponse::CallbackName { index: 1, name: Some("named".into()) }]);
        perform_handshake(false);
        assert!(host_capable());
        let names = callback_names();
        assert_eq!(names.len(), 1);
        assert_eq!(names.get("named"), Some(&1));
    }

    // 8. pure helper: unknown names are deduped + sorted
    #[test]
    fn test_unknown_callback_names_helper() {
        let rules: crate::core::rules::RuleSet = toml::from_str(r#"
[[callback_rules]]
match = "a"
enable = ["known_a", "ghost"]
disable = ["known_b", "phantom"]
"#).unwrap();
        let mut known = HashMap::new();
        known.insert("known_a".to_string(), 0u8);
        known.insert("known_b".to_string(), 1u8);
        let unknown = unknown_callback_names(&rules, &known);
        assert_eq!(unknown, vec!["ghost".to_string(), "phantom".to_string()]);
    }

Task 7: VALIDATE (build + full suite + scope)
  - cargo build --bin qmkonnect            # clean (G1: crate already v0.3.0 + tag pushed).
  - cargo test --bin qmkonnect -- --test-threads=1   # MANDATORY single-threaded (G6). All green.
  - cargo clippy --bin qmkonnect --no-deps 2>/dev/null || true   # optional; no NEW warnings.
  - git diff --stat                        # expect ONLY src/core/notifier.rs.
```

### Implementation Patterns & Key Details

```rust
// THE capable-predicate match (HOST_RULES.md §8(5)) — guard on the feature bit:
match n.send_command(qmk_notifier::RunCommand::QueryInfo, &filter) {
    Ok(qmk_notifier::CommandResponse::Info {
        proto_ver: 2,            // typed-command capable
        feature_flags,
        callback_count,
        board_rules_present,
    }) if feature_flags & 0x01 != 0 => { /* capable path */ }
    Ok(_) => { /* legacy / non-capable / Timeout-as-other -> string-only */ }
    Err(_) => { /* device error -> string-only */ }
}
// NOTE: CommandResponse::Timeout is a separate variant; `Ok(_)` catches it (Timeout
// is not Info). The Err arm catches QmkError (e.g. DeviceNotFound). BOTH set
// HOST_CAPABLE=false. No typed command is ever sent in those arms (G8/§8(8)).

// THE dedup swap (idempotent per board boot):
if HAS_HANDSHAKED.swap(true, Ordering::SeqCst) { return; }  // already done this session
// `swap` returns the OLD value; if it was true, we skip. Race-safe even if the
// device-status poll thread and startup race (they won't — S2 serializes — but the
// atomics are correct regardless).

// THE local-collect-then-publish (D2 — no nested locks, no lock-across-HID):
let mut local: HashMap<String, u8> = HashMap::new();
for i in 0..callback_count { /* send_command + local.insert */ }
drop(n);                                              // release NOTIFIER lock
{ let mut names = CALLBACK_NAMES.lock().unwrap(); names.clear(); names.extend(local); }
validate_rules_callback_names(verbose);               // locks CALLBACK_NAMES (read) — NOTIFIER already dropped
HOST_CAPABLE.store(true, Ordering::SeqCst);

// THE mock pop (D5 — backward-compatible queue):
MOCK_SEND_COMMAND_CALLS.lock().unwrap().push(command.clone());   // G4: keep the log
let resp = MOCK_RESPONSES.lock().unwrap().pop_front();            // None when empty
Ok(resp.unwrap_or(qmk_notifier::CommandResponse::Ack { ok: true })) // G5: default
```

### Integration Points

```yaml
MODULE REGISTRATION: NONE. `pub mod notifier;` is long-standing in src/core/mod.rs.
  This task only adds items to the BODY of notifier.rs.

DEPENDENCIES (this task): qmk_notifier v0.3.0 (ALREADY pinned + tagged — G1), once_cell::sync::Lazy,
  std::sync::{Mutex, atomic::{AtomicBool, Ordering}}, std::collections::{HashMap, BTreeSet,
  VecDeque (tests only)}, std::error::Error — ALL already available. NO new Cargo entries.

UPSTREAM (consumed unchanged — all verified present):
  - Notifier::send_command (notifier.rs:51 trait / L228 QmkNotifier impl / ~L510 Mock impl).
  - configured_filter() (L77) -> DeviceFilter; get_notifier() (L285) -> Arc<Mutex<Box<dyn Notifier>>>.
  - crate::core::now_ms() (core/mod.rs:57).
  - crate::core::rules::{get_rules_paths, parse_rules, RuleSet.callback_rules -> CallbackRule.enable/.disable}.
  - qmk_notifier::{RunCommand, CommandResponse, HostOs} (lib.rs L19/L86/L65).

DOWNSTREAM CONSUMERS (later subtasks — do NOT implement them here):
  - P4.M2.T1.S2 (integration): calls perform_handshake(verbose) near startup_device_probe
    and on an is_device_connected() false->true transition; calls reset_handshake_state()
    to re-trigger. Owns the poll thread / runner wiring — THIS task does NOT.
  - P4.M3.T1.S1 (host-context send): gates ApplyHostContext on host_capable() and passes
    callback_names() into rules::evaluate(). Owns the debounce-worker send extension.
  - P5.M1.T1.S1 (--list-callbacks / --validate-rules): prints callback_names() (or "legacy").

CONFIG: none. ROUTES/CLI: none (P5.M1). DATABASE: none. TRAY: none (P5.M2).
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# EXPECT: clean build, zero errors, zero NEW warnings. (Crate v0.3.0 resolves from
#   Cargo.toml:19 — NO path override. If a typed-variant/CommandResponse error appears,
#   check Cargo.toml:19 is still tag="v0.3.0"; do NOT add a path override.)

# Confirm the deliverables landed:
grep -n 'pub fn perform_handshake' src/core/notifier.rs            # expect 1
grep -n 'static HOST_CAPABLE\|static CALLBACK_NAMES\|static HAS_HANDSHAKED' src/core/notifier.rs  # expect 3
grep -n 'pub fn host_capable\|pub fn callback_names\|pub fn reset_handshake_state' src/core/notifier.rs  # expect 3
grep -n 'fn host_os\|fn unknown_callback_names\|fn validate_rules_callback_names' src/core/notifier.rs  # expect 3
grep -n 'static MOCK_RESPONSES\|fn set_mock_responses' src/core/notifier.rs  # expect 2
grep -n 'pop_front' src/core/notifier.rs                                       # expect 1 (mock send_command)
grep -n 'MOCK_RESPONSES.lock().unwrap().clear()' src/core/notifier.rs          # expect 1 (reset_global_mock)
# Confirm the existing send_command mock still records (G4):
grep -c 'MOCK_SEND_COMMAND_CALLS.lock().unwrap().push' src/core/notifier.rs    # expect 1 (unchanged)
# Confirm PendingMessage/DebounceState/worker/notify_qmk were NOT touched (G2):
git diff src/core/notifier.rs | grep -E '^[+-].*(struct PendingMessage|struct DebounceState|pending:|debounce_worker|fn notify_qmk)'  # expect NONE
```

### Level 2: Unit Tests (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# The 8 new handshake tests (single-threaded MANDATORY — G6):
cargo test --bin qmkonnect notifier::tests::test_handshake_ -- --test-threads=1
cargo test --bin qmkonnect notifier::tests::test_unknown_callback_names_helper -- --test-threads=1
# EXPECT: all 8 pass. Spot-check the highest-risk ones:
cargo test --bin qmkonnect notifier::tests::test_handshake_capable_populates_state -- --test-threads=1
# EXPECT: host_capable()==true, callback_names()=={vim_lazy:0,disable_vim:1}, call order [QueryInfo,SetOs,QueryCallback(0),QueryCallback(1)].
cargo test --bin qmkonnect notifier::tests::test_handshake_dedup_idempotent -- --test-threads=1
# EXPECT: 2nd perform_handshake adds 0 calls (HAS_HANDSHAKED short-circuit).
```

### Level 3: Full-crate regression (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# EXPECT: ALL bin tests green — the 8 new handshake tests + the 5 test_send_command_*
#   (unchanged: queue empty -> Ack{ok:true} default, G5) + debounce tests (incl. the
#   P4.M1.T2.S2 carry test) + pattern (P2) + rules (P3) + types. Proves the additions
#   compile in the full crate and didn't disturb the shared globals or the trait-object seam.

cargo clippy --bin qmkonnect --no-deps 2>/dev/null || true   # optional; no NEW warnings.

git status --short && git diff --stat
# EXPECT: only src/core/notifier.rs modified (additive). NOTHING in Cargo.toml, mod.rs,
#   rules.rs, pattern.rs, types.rs, platforms/, runners/, tray*, main.rs.
```

### Level 4: Contract / Scope Validation

```bash
cd /home/dustin/projects/qmkonnect
# Gate 1 — "only src/core/notifier.rs changed":
git diff --stat
# EXPECT: exactly src/core/notifier.rs. If Cargo.toml (path override), rules.rs, or
#   runners/* appear, you've made an out-of-scope edit — revert it (G1/G2).

# Gate 2 — "no path override" (G1):
git diff Cargo.toml
# EXPECT: empty (the crate stays pinned to tag="v0.3.0"). A path override is a
#   machine-local dev expedient from P4.M1.T1.S1 and is OBSOLETE here.

# Gate 3 — "capable path sends SetOs BEFORE the sweep" (HOST_RULES.md §8(5)):
cargo test --bin qmkonnect notifier::tests::test_handshake_capable_populates_state -- --test-threads=1
# EXPECT: PASS — calls[1] is SetOs(_), calls[2..] are QueryCallback(0), QueryCallback(1).

# Gate 4 — "legacy/timeout never sends typed commands beyond QueryInfo" (§8(8)):
cargo test --bin qmkonnect 'notifier::tests::test_handshake_legacy_proto_v1_string_only' \
                                   'notifier::tests::test_handshake_timeout_string_only' -- --test-threads=1
# EXPECT: both PASS, each with calls.len()==1 (QueryInfo only).

# Gate 5 — "dedup + reset" (once-per-boot semantics):
cargo test --bin qmkonnect 'notifier::tests::test_handshake_dedup_idempotent' \
                                   'notifier::tests::test_handshake_reset_allows_rerun' -- --test-threads=1
# EXPECT: both PASS.

# Gate 6 — "PendingMessage/DebounceState/worker/notify_qmk untouched" (G2 / sibling safety):
git diff src/core/notifier.rs | grep -E '^[+-].*(PendingMessage|DebounceState|debounce_worker|notify_qmk)'
# EXPECT: empty (P4.M1.T2.S2 owns that region; this task does not collide).
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean (no errors, no NEW warnings; NO path override — G1).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` green (8 new + all existing; no regression).
- [ ] `git diff --stat` shows `src/core/notifier.rs` ONLY (Gate 1).
- [ ] `git diff Cargo.toml` empty (Gate 2 — no path override).
- [ ] (optional) `cargo clippy --bin qmkonnect --no-deps` introduces no NEW warnings.

### Feature Validation (contract fidelity)
- [ ] `perform_handshake` on a capable device sets `HOST_CAPABLE=true`, populates `CALLBACK_NAMES`, and records `[QueryInfo, SetOs(_), QueryCallback(0..n)]` (Gate 3).
- [ ] Legacy (`proto_ver != 2`) / non-capable (`flags & 0x01 == 0`) / `Timeout` / `Err` ⇒ `HOST_CAPABLE=false`, map cleared, only `QueryInfo` sent (Gate 4).
- [ ] `perform_handshake` is idempotent (dedup); `reset_handshake_state()` re-enables it (Gate 5).
- [ ] `unknown_callback_names` returns the deduped+sorted unknown set (test 8).
- [ ] `host_capable()`, `callback_names()`, `reset_handshake_state()` are `pub` (S2/P4.M3/P5.1 consumers).
- [ ] `MockNotifier::send_command` still records the call log (G4) AND pops the response queue with the `Ack{ok:true}` empty-default (G5).

### Code Quality Validation
- [ ] Handshake block placed at the L182-184 band (above PendingMessage/DebounceState — G2); not inside the worker/notify_qmk region.
- [ ] No duplicate/conflicting imports (G3 — Ordering precedence-safe).
- [ ] No out-of-scope work: no runner wiring (S2), no send logic (P4.M3), no CLI/tray (P5), no Cargo edits.
- [ ] Mode-A rustdoc on `perform_handshake` (and the statics/accessors) with ` ```rust,ignore ` examples (G9).

### Documentation & Deployment
- [ ] `perform_handshake` rustdoc explains: capable predicate, SET_OS-once, sweep, validate-warn-don't-fail, idempotent-per-boot, S2's reset trigger.
- [ ] Commit message notes: "handshake + global capability state; S2 wires the (re)connect trigger; no behavior change without S2."

---

## Anti-Patterns to Avoid

- ❌ Don't apply a `Cargo.toml` path override — the crate is already pinned to v0.3.0 + the tag is pushed (G1). It's obsolete and breaks other machines.
- ❌ Don't touch `PendingMessage` (L257) / `DebounceState` (L264) / the debounce worker / `notify_qmk` (L381) — that's the (now-complete) P4.M1.T2.S2 region (G2).
- ❌ Don't drop the `MOCK_SEND_COMMAND_CALLS.push(command.clone())` when adding the response-queue pop — P4.M3's ordering assertions need the log (G4).
- ❌ Don't change the mock's default return to anything but `Ack{ok:true}` when the queue is empty — the 5 existing `test_send_command_*` depend on it (G5).
- ❌ Don't run tests multi-threaded — `--test-threads=1` is mandatory (G6).
- ❌ Don't make `validate_rules_callback_names` fatal — a missing/malformed rules.toml or an unknown name WARNS and continues; capability is never downgraded by a broken rules file (D3/G8).
- ❌ Don't wire the handshake into the runners / the device-status poll / `startup_device_probe` — that is **P4.M2.T1.S2**'s job. This task provides the callable + reset; S2 decides when.
- ❌ Don't send `SET_OS` after the sweep or skip it — the order is `QueryInfo → SetOs → sweep` (HOST_RULES.md §8(5)).
- ❌ Don't hold the `CALLBACK_NAMES` lock across the `send_command` sweep — collect locally, publish after `drop(n)` (D2).
- ❌ Don't use a bare ` ``` ` doctest fence — binary crate, use ` ```rust,ignore ` (G9).
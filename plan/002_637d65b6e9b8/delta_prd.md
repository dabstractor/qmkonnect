# Delta PRD — Host-Side Window Rules (F11/F12)

**Product Requirements Document — Feature Delta**
Base: QMKonnect v0.2.4 master PRD (features F1–F10, all complete in prior session)
Delta: Adds F11 (Host-side window rules) + F12 (Named callback registry + typed commands)
Status: Design complete (`HOST_RULES.md`); implementation pending
Owner: Mulletware · License: MIT

> This is a **delta** PRD. It covers ONLY the host-side-window-rules feature
> added to the master PRD. Every other feature (F1–F10) is already implemented
> and unchanged; do not re-implement it. The canonical design is
> **`HOST_RULES.md`** in the repo — this document breaks that design into
> implementable work for the `qmkonnect` repo and names the cross-repo
> prerequisites. Read `HOST_RULES.md` first; it is authoritative on disagreement.

---

## 1. What Changed (diff vs. the prior completed PRD)

The previous session implemented the full QMKonnect application (F1–F10,
platform monitors, tray UI, packaging, CI, hardening). The current master PRD
adds **one large feature**: host-side window rules. Concretely, the master PRD
gained:

| Location | Change |
|---|---|
| Intro callout | New summary block pointing at `HOST_RULES.md` and the firmware `PRD.md` §4.6 / crate `PRD.md` §10 as canonical owners of the wire contract. |
| §2.2 Non-Goals | The "no behavior/layer logic on the desktop" non-goal is now annotated as *relaxed* by this feature (host optionally matches rules and stacks a layer + callbacks on top of the board's). |
| §4 Feature Set | **+F11** Host-side `rules.toml` → layers/callbacks with **no reflash**, stacking on board rules. **+F12** Named callback registry + typed Raw HID commands (`QUERY_INFO` / `QUERY_CALLBACK` / `APPLY_HOST_CONTEXT`) with a capability handshake. |
| §12 Future Work | New "Host-side window rules" bullet (the feature is designed but unimplemented). |
| §13 Glossary | +5 terms: `board layer/rules`, `host layer/rules`, `callback registry`, `typed command`, `APPLY_HOST_CONTEXT`. |
| §14 Doc Map | +`HOST_RULES.md` row. The inlined specs gained: `ARCHITECTURE.md` §5.7 (host-rules pipeline extension), `PROTOCOL.md` §8 (typed-command namespace), `CONFIG.md` (`rules.toml` path note + new CLI flags). |
| **New spec** | `HOST_RULES.md` — 14-section companion spec (goal, three-repo context, locked decisions, architecture/coexistence model, wire protocol, firmware summary, crate summary, qmkonnect spec, `rules.toml` schema, migration, implementation breakdown, testing, risks). |

**Net:** a single large feature spanning **three repos** (`qmk_notifier` crate,
`qmk-notifier` firmware, this `qmkonnect` app), plus documentation updates.
**No existing F1–F10 requirement is removed or materially modified** — host
rules stack *on top of* the existing string path and fall back to it on legacy
hardware. The only touch to completed code is additive extension (see §4).

---

## 2. Goal & Scope of This Delta

**Goal.** Let users define **app → layer** and **app → callback** rules in an
editable file (`rules.toml`) on their computer, with matching done by QMKonnect
on the host — so rules change **without reflashing firmware**. Both layer
switching and the existing `on_enable`/`on_disable` callback pattern are
supported. Host rules **stack on top of** the keyboard's existing firmware rules
(`DEFINE_SERIAL_LAYERS` / `DEFINE_SERIAL_COMMANDS`): board rules run first, then
host rules apply on top.

**Scope of THIS repo.** Implement the qmkonnect side:
`src/core/pattern.rs` (matcher), `src/core/rules.rs` (rules model + evaluation),
`src/core/notifier.rs` extension (handshake + stack/replace send sequencing),
CLI flags, tray "Reload rules" item, and the `rules.toml` template. Also: bump
the `qmk_notifier` crate pin to a typed-command-capable release.

**Out of scope for THIS repo (cross-repo prerequisites):**
- The `qmk-notifier` firmware changes (registry, host-layer tracker, typed
  dispatch, `SET_OS`, `has_been_queried`) — owned by `dabstractor/qmk-notifier`
  `PRD.md` §4.6/§14. We depend on it; we do not build it.
- The `qmk_notifier` crate transport changes (typed framing, `CommandResponse`,
  `HostOs`) — owned by `dabstractor/qmk_notifier` `PRD.md` §10. We consume a
  tagged release.
- VIA coexistence (reserved feature-flag bit `0x04`, Phase E).
- Host-side shell actions / keyboard macros (B3: callbacks are
  firmware-registered C functions only).

**Backward compatibility (hard requirement).**
- No `rules.toml` ⇒ identical to today (string-only).
- Legacy firmware (`proto_ver != 2` / handshake timeout) ⇒ string-only, board
  rules unaffected, **no typed command ever sent** to it.
- New firmware + old QMKonnect ⇒ old app sends only the string; typed commands
  never arrive (firmware ignores `0xF0`).

---

## 3. Locked Design Decisions (from `HOST_RULES.md` §3)

These are **already decided** — do not re-litigate:

- **B1 — Coexistence = per-window stack-or-replace, host-chosen via `clear_board`.**
  `clear_board=0` (stack): board runs its rules (board layer first → host layer on
  top; board callbacks first → host callbacks after). `clear_board=1` (replace):
  firmware clears its board layer/command first; host context drives the board.
  The host selects per window from `disable_firmware_config` (C10).
- **B2 — Callback identity = firmware registry + startup name query.** Firmware
  declares named callbacks (`DEFINE_HOST_CALLBACKS`); IDs are declaration order;
  QMKonnect queries names at (re)connect; `rules.toml` references callbacks by
  **name**. Re-query on reconnect makes cross-flash renumbering harmless.
- **C1/C2 — Format TOML; separate `rules.toml` next to `config.toml`.** C3 —
  hot-reload (fs watch + tray "Reload rules").
- **C4 — Full matcher parity**: port firmware `pattern_match.c` to Rust
  **including** `+` and classes (`\d \D \w \W \s \S \b \B .`) — all linear-time.
- **C5 — Capability handshake with graceful fallback**, gated on `proto_ver == 2`.
- **C7 — No-match ⇒ always clear** (host layer cleared, host callbacks'
  `on_disable` fire via desired-set diff). `on_no_match = "keep"` is dropped.
- **C8 — All matching callbacks fire; layers are exclusive** (first-match-wins,
  one host layer).
- **C9 — One global ruleset** for v1 (per-keyboard overrides deferred).
- **C10 — `disable_firmware_config` per-rule** (default `false`, global default
  under `[host]`, per-rule override). A window is "replace" iff **every** matched
  rule's effective flag is `true`.
- **C11 — Host layers reserved ≥ 224** (resolve above board layers; `255` = clear).
- **C12 — Host is the OS source of truth** while connected: `SET_OS` once at
  connect (firmware `OS_DETECTION` is the offline fallback).

---

## 4. Impact on Completed (F1–F10) Work

This feature is **additive**. The only completed files that change:

| File | Change | Why |
|---|---|---|
| `Cargo.toml` | Bump `qmk_notifier` git-tag pin from `v0.2.1` → new typed-command release. | New `RunCommand` variants + `run() -> Result<CommandResponse, _>`. |
| `src/core/notifier.rs` | Extend `notify_qmk` (after the debounced string send, optionally send `APPLY_HOST_CONTEXT`); add handshake + `SET_OS` near `startup_device_probe`; extend `Notifier` trait / `QmkNotifier` so the mock can assert call ordering. | Host-context send + capability detection. The debouncer itself is **unchanged** (host-context send happens within the same debounced "send" step). |
| `src/core/mod.rs` | Wire `rules`/`pattern` modules into config/startup; add `rules.toml` path resolution next to `config.toml`. | New module registration. |
| `src/main.rs` | Add `--list-callbacks`, `--validate-rules`, `--rules-path`; extend `-c`/`--config` to also seed a commented `rules.toml` template. | New CLI surface. |
| `src/tray.rs`, `src/linux_tray.rs` | Add a **"Reload rules"** menu item to all three menus (re-read `rules.toml`, re-validate, re-handshake if needed). | C3 hot-reload UX. |

**Everything else (platform monitors, debouncer state machine, device probes,
udev/systemd, packaging, CI, autostart, window-info dialogs) is untouched.**

---

## 5. Implementation Plan (by repo role)

> Per `HOST_RULES.md` §11. The crate and firmware rows are **prerequisites**;
> only the `qmkonnect` rows are tasks in this PRD.

### Cross-repo prerequisites (blocking — not tasks here)

- **`qmk_notifier` crate:** add typed-command framing (multi-report),
  `CommandResponse` reply parsing, `HostOs`, change `run()` to return
  `CommandResponse`. Tag a release. *(The matcher is NOT in the crate — it lives
  in qmkonnect.)*
- **`qmk-notifier` firmware:** `DEFINE_HOST_CALLBACKS`, `host_layer` /
  `host_cb_enabled`, typed dispatch, `QUERY_INFO` / `QUERY_CALLBACK` / `SET_OS` /
  `APPLY_HOST_CONTEXT` (with `clear_board`), `has_been_queried`, tests.

### qmkonnect tasks (phases below)

- Pin the crate; `src/core/pattern.rs` (full-parity matcher + ported corpus);
  `src/core/rules.rs` (model + evaluation); handshake + `SET_OS`; the
  `notify_qmk` stack/replace send logic + state; CLI flags; tray "Reload rules";
  config-path integration; tests.

---

## 6. Phases & Milestones

> Dependency graph: **P1** (matcher, pure logic, no deps) and **P2** (rules
> model, pure logic, depends on P1) can proceed immediately and be fully tested
> in isolation. **P3** (transport + handshake + send sequencing) is **blocked on
> the crate release** (cross-repo prerequisite). **P4** (CLI/tray UX) depends on
> P3. **P5** (docs) depends on all.

---

### Phase P1 — Pattern Matcher Port (`src/core/pattern.rs`)

**Goal.** A standalone, fully-tested Rust port of the firmware `pattern_match.c`
with **complete parity** (not a subset). This is foundational: the rules
evaluation (P2) and the long-term firmware-parity guarantee both depend on it.
No external deps; buildable/testable in isolation.

**Why its own phase:** it is pure logic with no crate dependency, has a concrete
source of truth (the firmware matcher + its test corpus), and is the single
largest correctness risk in the feature (a matcher drift between host and
firmware silently mis-routes layers). Isolating it makes parity reviewable.

- **Mode A docs:** rustdoc on `Pattern`, the matcher functions, and a module-level
  comment stating "full-parity port of firmware `pattern_match.c`; the firmware
  corpus is the source of truth."

#### Milestone P1.M1 — Matcher implementation + parity tests

| ID | Task | SP | Deps | PRD ref |
|---|---|---|---|---|
| P1.M1.T1 | **Full-parity `Pattern` enum + matcher.** Implement `src/core/pattern.rs`: `Pattern::Single(String)` (class-only, or whole-string when no title) and `Pattern::Parts(String, String)` (`WT(class,title)` — delimiter `0x1D` GS). Implement the matcher supporting `*` wildcard; `^`/`$` anchors; `+` quantifier; classes `\d \D \w \W \s \S \b \B`; `.`; escapes (`\^ \$ \* \\`); per-rule `case_sensitive` (default `false`). All linear-time (Thompson NFA). Subtasks: (S1) `Pattern` enum + parse of the two TOML shapes (`String` / `[String,String]`); (S2) the single-pattern matcher with anchors/wildcard/quantifier/classes/escapes; (S3) the delimiter-aware two-half matcher (GS split; left-vs-left AND right-vs-right; one-sided when only one side has GS). | 3 | none | `HOST_RULES.md` §9 (Pattern), §14 appendix; firmware `pattern_match.c` |
| P1.M1.T2 | **Port the firmware test corpus as parity tests.** Port the firmware `pattern_match` test suite (wildcards, `^`/`$`, `WT`, `+`, classes, case sensitivity, escapes) into `#[cfg(test)]` and assert identical results. Add `WT` two-half edge cases (one side empty, both empty, mismatched-presence of GS). | 2 | P1.M1.T1 | `HOST_RULES.md` §12, §14 |

---

### Phase P2 — rules.toml Model & Host Evaluation (`src/core/rules.rs`)

**Goal.** Parse/validate `rules.toml`, and evaluate host rules against a window
string to produce a `{host_layer: Option<u8>, desired_callbacks: BTreeSet<u8>,
replace: bool}` decision. Pure logic; fully unit-testable without the crate or
hardware.

**Why its own phase:** decoupled from transport — the rules model and the
stack-vs-replace decision logic can be designed, reviewed, and tested before any
HID round-trip exists.

- **Mode A docs:** rustdoc on `RuleSet` / `LayerRule` / `CallbackRule` /
  `HostException`; a module-level comment cross-referencing `HOST_RULES.md` §9
  schema and §4 coexistence model.

#### Milestone P2.M1 — Schema, parse/validate, evaluation

| ID | Task | SP | Deps | PRD ref |
|---|---|---|---|---|
| P2.M1.T1 | **`rules.toml` model + parse/validate.** Implement `src/core/rules.rs` per `HOST_RULES.md` §9: `RuleSet { host: HostDefaults, layer_rules, callback_rules }`, `LayerRule { pattern, layer, case_sensitive, disable_firmware_config: Option<bool> }`, `CallbackRule { pattern, enable, disable, case_sensitive, disable_firmware_config: Option<bool> }`, `Pattern` (untagged `Single`/`Parts`). `parse_rules(path)` via `toml::from_str`. Effective `disable_firmware_config` = rule override if `Some`, else `[host]` default (`false`). Unknown callback **names** are not errors here (resolved against the registry in P3) but are collected for later validation. Subtasks: (S1) struct model + serde; (S2) `parse_rules` + error messages; (S3) `validate_rules(&RuleSet, &name_to_id)` returning unknown-name warnings. | 2 | P1.M1.T1 | `HOST_RULES.md` §9, §8(6) |
| P2.M1.T2 | **Host rule evaluation → decision.** Implement the pure evaluation against a window string `s = "{class}\x1D{title}"`: (1) **layer** = first matching `layer_rule`'s `layer` (else none); (2) **callbacks** = union of `enable` across **all** matching `callback_rules`, minus the union of `disable` (explicit exclusion) — resolved to IDs via a `name_to_id` map (unknown names skipped with a warning); (3) **replace** = `true` iff **every** matched rule's effective `disable_firmware_config` is `true` (the string is shared by both board lanes, so it is sent iff the board has rules **and** ≥1 matched rule is non-disabling). No-match ⇒ `{layer: None, callbacks: empty}` (always clear; C7). | 2 | P2.M1.T1, P1.M1.T1 | `HOST_RULES.md` §4, §8(3) |
| P2.M1.T3 | **Evaluation unit tests.** Cover: first-match layer vs all-match callbacks; `disable` exclusion; mixed `disable_firmware_config` → stack; all-disabling → replace; no-match ⇒ clear; `Pattern::Single` vs whole-string-when-no-title parity; case-sensitivity toggle. | 1 | P2.M1.T2 | `HOST_RULES.md` §12 |

---

### Phase P3 — Crate Upgrade, Handshake & Send Sequencing

> **BLOCKED on the `qmk_notifier` crate release** (cross-repo prerequisite,
> `HOST_RULES.md` §7). Do not start P3.M1 until a typed-command-capable crate
> tag exists and is pinned.

**Goal.** Consume the upgraded crate, detect capability at (re)connect, and
extend `notify_qmk` so a debounced window change sends the right sequence
(string-then-context in stack mode; context-only in replace mode; clear on
no-match).

**Why its own phase:** this is the only phase that touches HID I/O and the
global `Notifier` trait; it carries the backward-compatibility and ordering
invariants, and it is where the handshake's graceful-fallback semantics live.

- **Mode A docs:** rustdoc on the handshake function and the stack/replace send
  logic; note the `proto_ver == 2` gate and "at most once per board boot"
  invariant. Update the `notify_qmk` doc comment (the host-context send is part
  of the debounced send step — at most 2 sends per window change).

#### Milestone P3.M1 — Crate pin + typed-command transport in QmkNotifier

| ID | Task | SP | Deps | PRD ref |
|---|---|---|---|---|
| P3.M1.T1 | **Pin the upgraded crate; adapt call sites.** Bump `Cargo.toml` `qmk_notifier` tag to the typed-command release. Update `QmkNotifier::notify` and probes to the new `run() -> Result<CommandResponse, QmkError>`. Preserve existing retry/cache semantics (device-class errors retried ≤3× then `Ok`; §5.4 unchanged). Subtasks: (S1) pin + `cargo update -p qmk_notifier`; (S2) migrate `SendMessage` call site to read `CommandResponse::Legacy { matched }`; (S3) confirm existing notifier tests still pass single-threaded. | 2 | *crate release* | `HOST_RULES.md` §7; crate `PRD.md` §10 |
| P3.M1.T2 | **Extend the `Notifier` trait for ordered sends.** The test mock must assert ordering (string-before-context in stack; context-only in replace). Extend the trait/`QmkNotifier` so a typed `ApplyHostContext` send is expressible alongside the legacy string, and record call order in `MockNotifier`. (Keep the existing `notify(String)` path; add the typed path rather than rewiring the debouncer.) | 1 | P3.M1.T1 | `HOST_RULES.md` §8(4), §12 |

#### Milestone P3.M2 — Capability handshake + `SET_OS`

| ID | Task | SP | Deps | PRD ref |
|---|---|---|---|---|
| P3.M2.T1 | **Handshake at (re)connect, gated on `proto_ver == 2`.** Near `startup_device_probe` (and re-triggered only on a real device transition via the existing `is_device_connected()` poll): send `QUERY_INFO`; on `Info { proto_ver: 2, feature_flags, callback_count, .. }` with `flags & 0x01`, send `SET_OS(host_os)` **once** (host is OS-authoritative at connect), then sweep `QUERY_CALLBACK(i)` for `0..callback_count` to build the `name → id` map, then `validate_rules` against it (warn, don't fail) and set `capable = true`. On `response[0] != 0x51` / `Timeout` / `proto_ver != 2` ⇒ `capable = false` (string-only). Run **at most once per board boot** (firmware `has_been_queried` guards mid-session reconnect side effects). Subtasks: (S1) handshake orchestration + `capable`/queried state (deduped via the device poll); (S2) `HostOs` resolution per platform (`0 UNSURE · 1 LINUX · 2 WINDOWS · 3 MACOS`); (S3) unit tests for handshake parsing (`Info{2,…}` ⇒ capable; legacy/timeout ⇒ string-only). | 3 | P3.M1.T1, P2.M1.T1 | `HOST_RULES.md` §5, §8(5) |

#### Milestone P3.M3 — `notify_qmk` stack/replace send sequencing

| ID | Task | SP | Deps | PRD ref |
|---|---|---|---|---|
| P3.M3.T1 | **Stack/replace send logic inside the debounced send step.** When `capable && rules.toml present`, after the (existing) debounced string decision, evaluate host rules (P2) and: **stack** (board has rules AND ≥1 matched rule non-disabling) ⇒ send the **string** first (`SendMessage`), await its `CommandResponse`, then `ApplyHostContext { layer, callbacks, clear_board: false }`; **replace** (all matched rules disabling, OR board has no rules) ⇒ send **only** `ApplyHostContext { layer, callbacks, clear_board: true }` (no string); **no-match** ⇒ `ApplyHostContext { layer: None (0xFF), callbacks: empty }` (always clear). One window change ⇒ ≤2 sends. The debouncer itself is unchanged — this happens within the existing "send" step. Retry/cache parity with `SendMessage`. Subtasks: (S1) the per-window decision + send ordering; (S2) host-state tracking for the next diff/logging; (S3) ordering tests via the extended mock (string-before-context in stack; context-only in replace; clear on no-match). | 3 | P3.M1.T2, P3.M2.T1, P2.M1.T2 | `HOST_RULES.md` §4, §8(4) |

---

### Phase P4 — CLI, Tray UX & Config-Path Integration

**Goal.** Surface the feature to the user: CLI discovery/validation flags, a
"Reload rules" tray item on all three platforms, `rules.toml` path resolution
and template seeding.

**Why its own phase:** small, UI-surface work that depends on P3 being usable;
keeps the CLI/tray changes in one reviewable chunk.

- **Mode A docs:** update `print_help()` output and the CLI doc comment;
  rustdoc on the new flags. (The user-facing `rules.toml` schema is documented in
  `HOST_RULES.md` §9 and `CONFIG.md` — keep in sync.)

#### Milestone P4.M1 — CLI flags + rules.toml paths/template

| ID | Task | SP | Deps | PRD ref |
|---|---|---|---|---|
| P4.M1.T1 | **CLI: `--list-callbacks`, `--validate-rules`, `--rules-path`; seed template.** In `src/main.rs`: `--list-callbacks` (run the handshake, print the keyboard's callback `name→id` table, or "legacy"); `--validate-rules [--rules-path <p>]` (parse `rules.toml`, schema check, flag unknown callback names, non-zero exit on error); `--rules-path` value flag (reuse `parse_value_flag`). Extend `-c`/`--config` to also seed a commented `rules.toml` template (next to `config.toml`) when absent. Add `rules.toml` path resolution alongside `config.toml` (same dir per platform: Linux `~/.config/qmk-notifier/`, Windows `%APPDATA%\QMKonnect\`, macOS `~/Library/Application Support/QMKonnect/`). Absent ⇒ host rules disabled. Subtasks: (S1) flag scan + dispatch + help text; (S2) path resolution + template seeding; (S3) `--validate-rules` exit codes. | 2 | P3.M2.T1, P2.M1.T1 | `HOST_RULES.md` §8(6); `CONFIG.md` §3, §4 |
| P4.M1.T2 | **Tray "Reload rules" on all three platforms.** Add a **"Reload rules"** item to the macOS/Windows menu (`src/tray.rs`) and the Linux SNI menu (`src/linux_tray.rs`): re-read `rules.toml`, re-validate, and re-handshake if needed (reuse P3.M2). Optional status line: `proto v2 · N callbacks`. Respect the existing `!Send` `MenuItem` invariant (mutate only on the event-loop thread / via `handle.update`). | 1 | P4.M1.T1, P3.M2.T1 | `HOST_RULES.md` §8(7); `UI.md` §1 |

---

### Phase P5 — Documentation Sync (Mode B)

> **Mode B — changeset-level docs.** This feature adds a companion spec
> (`HOST_RULES.md`, already present) and crosses README/docs and the master PRD's
> feature table/glossary. These updates only make sense once P1–P4 land; they
> ride as a final task depending on all of the above.

| ID | Task | SP | Deps | PRD ref |
|---|---|---|---|---|
| P5.M1.T1 | **Sync changeset-level docs.** (a) `Readme.md` — add host-side rules to the feature blurb + a short "edit `rules.toml`, no reflash" example; (b) `docs/qmk-integration.md` — the migration subsection (`HOST_RULES.md` §10: expose callbacks by name once, then move rules to host); (c) `docs/configuration.md` — `rules.toml` schema summary + new CLI flags, matching `CONFIG.md`; (d) `docs/examples.md` — a worked `rules.toml` mirroring the reference keymap; (e) `docs/troubleshooting.md` — legacy-firmware fallback, `--validate-rules`, `--list-callbacks`; (f) regenerate `docs/llms_full.txt`; (g) verify `spec/*.md` match the final code (code wins on disagreement — report drift); (h) confirm the master `PRD.md` feature table (F11/F12), §12 bullet, and §13 glossary terms are accurate against the shipped implementation. | 1 | P4.M1.T2, P3.M3.T1 | `HOST_RULES.md` §1 deliverables, §10 |

---

## 7. Testing Plan (summary — full detail: `HOST_RULES.md` §12)

- **P1 (`pattern.rs`):** full firmware-corpus parity (wildcards, anchors, `WT`,
  `+`, classes, escapes, case sensitivity). **This is the correctness keystone.**
- **P2 (`rules.rs`):** TOML parse success/error; first-match (layers) vs
  all-match (callbacks); `disable` exclusion; unknown callback names skipped;
  stack-vs-replace decision; no-match ⇒ clear.
- **P3 (handshake + send):** handshake parsing (`Info{2,…}` ⇒ capable;
  legacy/timeout ⇒ string-only); stack sends string-then-context; replace sends
  context-only; no-match clears — all asserted via the extended `MockNotifier`
  call ordering.
- **All notifier tests run single-threaded** (`--test-threads=1`; shared global
  debouncer state — unchanged from the existing constraint).

---

## 8. Risks (from `HOST_RULES.md` §13)

- **R1 — HID round-trips per change.** Stack mode = 2 sends/change (string +
  context); replace = 1. Mitigated by the existing debounce.
- **R3 — HID exclusivity.** Another Raw HID app (VIA) holding the device blocks
  QMKonnect. Out of scope (Phase E).
- **R4 — ID stability across flashes.** Mitigated by re-querying names on every
  reconnect (B2).
- **Cross-repo coordination risk (this PRD's addition).** P3 is blocked until the
  crate is released and the firmware advertises `proto_ver == 2`. P1/P2 can
  proceed and land independently in the meantime to de-risk the schedule.

---

## 9. Success Criteria (how "done" is judged for this delta)

1. A user can add/change a layer or callback rule by editing `rules.toml` and
   clicking **"Reload rules"** — **no reflash**.
2. Board (`DEFINE_*`) rules keep working unchanged; host rules apply on top in
   the documented order (board layer first → host layer on top; board callbacks
   first → host callbacks after) in **stack** mode; **replace** mode clears the
   board layer/command for that window.
3. **Old firmware + new QMKonnect** continues to work in string-only mode (graceful
   handshake fallback); no typed command is sent to firmware that doesn't
   advertise `proto_ver == 2`.
4. **New firmware + old QMKonnect** keeps working (old app sends only the legacy
   string).
5. `cargo test --bin qmkonnect -- --test-threads=1` passes, including the ported
   firmware matcher corpus (P1) and the new ordering tests (P3).
6. `--validate-rules` flags unknown callback names; `--list-callbacks` prints the
   `name→id` table (or "legacy").

---

*End of delta PRD. Canonical design: `HOST_RULES.md`. Wire contract: firmware
`PRD.md` §4.6. Transport: crate `PRD.md` §10. Return to the master `PRD.md` for
the product-level overview.*
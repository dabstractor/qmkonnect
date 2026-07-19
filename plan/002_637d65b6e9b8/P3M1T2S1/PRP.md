# PRP — P3.M1.T2.S1: Implement `evaluate()` with layer first-match, callback all-match, and HostContext output

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This task ADDS two items to
> `src/core/rules.rs` (the file created by P3.M1.T1.S1 and already extended by
> P3.M1.T1.S2): (1) a `pub struct HostContext` and (2) a `pub fn evaluate(...)`.
> It is the **evaluation engine** — the pure function that, given a parsed
> `RuleSet` + a window (app_class, title) + the handshake name→id map + whether
> the board has its own rules, decides the host layer, the desired enabled
> callback id set, the `clear_board` (stack-vs-replace) flag, and whether any
> rule matched. PRD §8 point 3 (lines 318–325) + §4 (lines 124–168) are the
> spec sources of truth. **Consumes:** `RuleSet`/`HostDefaults`/`LayerRule`/
> `CallbackRule` (P3.M1.T1.S1 — Complete), the private
> `effective_disable_firmware_config` primitive (P3.M1.T1.S2 — Complete), and
> `match_pattern(&Pattern, &str, &str, bool)` (P2.M1.T3.S2 — Complete).
> **Consumed downstream by:** P4.M3.T1.S1 (notify_qmk host-context send logic),
> which derives `send_string = board_has_rules && any_match && !clear_board`
> and the `ApplyHostContext { layer, callbacks, clear_board }` payload.

> **PARALLEL-EXECUTION NOTE:** the two predecessors (S1 structs, S2 functions)
> are BOTH verified present in `src/core/rules.rs` as of this research (structs
> lines 67–185; `effective_disable_firmware_config` @201, `parse_rules` @229,
> `get_rules_paths` @247; tests `#[cfg(test)] mod tests` @254–581). This task is
> purely **additive**: it inserts `HostContext` + `evaluate()` between line 252
> (end of `get_rules_paths`) and line 254 (`#[cfg(test)]`), and appends tests
> inside the existing `mod tests` block. It touches NO struct, NO S2 function,
> NO existing test.

---

## Goal

**Feature Goal**: Add to `src/core/rules.rs` (i) a `pub struct HostContext` with
exactly four public fields (`layer: Option<u8>`, `callback_ids: Vec<u8>`,
`clear_board: bool`, `any_match: bool`), and (ii) a `pub fn evaluate(rules:
&RuleSet, app_class: &str, title: &str, name_to_id: &HashMap<String, u8>,
board_has_rules: bool) -> HostContext` that implements the three-stage
per-window evaluation (layer first-match → `L_h`; callbacks all-match → desired
id set with explicit-exclusion disables; stack-vs-replace decision →
`clear_board`). Both carry Mode-A rustdoc citing HOST_RULES.md §8(3)/§4. The
function is **pure** (no IO, no logging, no global state) and unit-tested with
~14 tests covering each evaluation stage + the stack/replace truth table.

**Deliverable**:
1. **ADDITIONS to** `src/core/rules.rs` (between `get_rules_paths` and the
   `#[cfg(test)] mod tests` block):
   - **2 new imports**: `use std::collections::{BTreeSet, HashMap};` (new line in
     the `use std::...` group) and `match_pattern` joined into the existing
     pattern import (`use crate::core::pattern::{match_pattern, Pattern};`).
   - **1 struct**:
     ```rust
     #[derive(Debug, Clone, PartialEq)]
     pub struct HostContext {
         pub layer: Option<u8>,
         pub callback_ids: Vec<u8>,
         pub clear_board: bool,
         pub any_match: bool,
     }
     ```
     with a Mode-A `///` rustdoc.
   - **1 function**:
     ```rust
     pub fn evaluate(rules: &RuleSet, app_class: &str, title: &str,
                     name_to_id: &HashMap<String, u8>,
                     board_has_rules: bool) -> HostContext
     ```
     implementing: layer first-match scan; callback all-match scan with
     enable-union / disable-exclusion (resolved through `name_to_id`); no-match
     early-return to `{ None, vec![], false, false }`; matched-case
     `clear_board = all_matched_rules_effective_disabling || !board_has_rules`.
     Mode-A `///` rustdoc.
   - **~14 NEW unit tests** appended to the existing `#[cfg(test)] mod tests`
     block (S1's + S2's tests untouched), prefixed `test_evaluate_*`.
2. **NO other files change.** No Cargo.toml edit (std `collections`; `log` is a
   dep but unused here — evaluate is pure). No notifier/debounce/CLI wiring
   (P4.M3/P5). No `HostContext` fields beyond the four (no `send_string`).

**Success Definition**:
- An empty `RuleSet` against any window → `HostContext { layer: None, callback_ids: vec![], clear_board: false, any_match: false }`.
- Layer scan is first-match-wins: only the first `layer_rules[i]` whose
  `match_pattern(&pattern, app_class, title, case_sensitive)` returns `true`
  contributes `layer = Some(rule.layer)`; subsequent layer rules are not consulted.
- Callback scan is all-match: every `callback_rules[i]` whose pattern matches
  contributes its `enable` names (added to the desired set, resolved via
  `name_to_id`) and its `disable` names (removed from the desired set).
- Unknown callback names (`name_to_id.get(name) == None`) are **silently skipped**
  — evaluate does not panic, error, or log.
- `callback_ids` is **sorted** (BTreeSet → deterministic `Vec<u8>`).
- `clear_board` truth table (matched case): `true` iff every matched rule's
  effective `disable_firmware_config` is `true` **OR** `board_has_rules == false`
  (the §4 "replace = all-disabling OR board-has-no-rules" definition).
- `any_match == true` iff at least one rule (layer OR callback) matched.
- `cargo build --bin qmkonnect` compiles clean (no new warnings).
- `cargo test --bin qmkonnect -- --test-threads=1` green (S1 + S2 tests + the
  ~14 new tests + all existing crate tests; no regression).
- `git status` shows exactly ONE changed file: `src/core/rules.rs`.

## User Persona (if applicable)

**Target User**: the downstream notify_qmk send-logic implementer (P4.M3.T1.S1).
- P4.M3.T1.S1 calls `evaluate(&rules, &window.app_class, &window.title,
  &name_to_id, board_has_rules)` on each debounced window change, then derives
  `send_string = board_has_rules && ctx.any_match && !ctx.clear_board`, and emits
  `ApplyHostContext { layer: ctx.layer, callbacks: ctx.callback_ids, clear_board: ctx.clear_board }`.
- P5.M1.T1.S1 (`--validate-rules`) exercises the same path on a sample window
  (it does not call evaluate directly, but depends on its correctness for the
  rules-preview path).

**Use Case**: on each debounced window focus change, the host resolves
`rules.toml` → `RuleSet` (P3.M1.T1.S2), then calls `evaluate()` to turn the
window + rules + handshake name table into the single `HostContext` that drives
the entire `notify_qmk` send decision (string-first stack vs context-only replace
vs no-match clear).

**Pain Points Addressed**: replaces the firmware's per-window `match_pattern`
dispatch with a typed, testable, host-side decision that unifies layer selection,
callback-set reconciliation, and the stack/replace categorization into one pure
function with a deterministic output.

## Why

- **PRD §8 point 3 (lines 318–325) defines the three-stage evaluation:**
  (1) Layer first-match → `L_h`; (2) Callbacks all-match → desired enabled id
  set with disable-as-explicit-exclusion; (3) stack-vs-replace = "replace iff
  every matched rule's effective `disable_firmware_config` is true." This task
  implements all three.
- **§4 (lines 124–168) fixes the coexistence semantics:** the host decides per
  window whether the board runs — "Replace (all matched rules disabling, **OR
  board has no rules**)" vs "Stack (board has rules AND ≥1 non-disabling)." The
  `clear_board` field encodes exactly that bit (see the design decision below).
- **Unblocks P4.M3.T1.S1.** The notify_qmk host-context send logic is the sole
  consumer and cannot be written without this function + struct.
- **Closes the rules-engine vertical slice** (P3.M1): S1 (model) + S2
  (file-IO + primitive) + this task (evaluation) = a complete, unit-testable
  rules engine with zero notifier coupling.

## What

Pure additions to `src/core/rules.rs`: 2 imports + 1 struct + 1 function + ~14
tests + Mode-A rustdoc on the struct and the function. No struct/fn changes
elsewhere, no new deps, no CLI/tray/notifier wiring, no `HostContext` field
beyond the four.

### Success Criteria
- [ ] **`pub struct HostContext`** with `#[derive(Debug, Clone, PartialEq)]`; exactly four `pub` fields in this order: `layer: Option<u8>`, `callback_ids: Vec<u8>`, `clear_board: bool`, `any_match: bool`.
- [ ] **`pub fn evaluate(rules: &RuleSet, app_class: &str, title: &str, name_to_id: &HashMap<String, u8>, board_has_rules: bool) -> HostContext`** — exactly this signature (param names + types + order + return type).
- [ ] **Layer scan is first-match-wins** (breaks on first `match_pattern` success; records the matched rule's effective `disable_firmware_config`).
- [ ] **Callback scan is all-match** (every matching callback rule contributes; no break).
- [ ] **Enable names added, disable names removed** (resolved through `name_to_id: &HashMap<String, u8>`; unknown names silently skipped — `if let Some(&id) = name_to_id.get(name)`).
- [ ] **No-match early-return** to `{ layer: None, callback_ids: vec![], clear_board: false, any_match: false }` (short-circuits BEFORE the clear_board formula; see G2).
- [ ] **`clear_board = all_matched_effective_disabling || !board_has_rules`** in the matched case (the §4 "replace" bit — see G1).
- [ ] **`callback_ids` sorted** (built from a `BTreeSet<u8>` → deterministic).
- [ ] **`any_match == !matched_flags.is_empty()`** (true iff ≥1 rule matched).
- [ ] **`match_pattern` called as** `match_pattern(&rule.pattern, app_class, title, rule.case_sensitive)` (per-rule `case_sensitive`, NOT a global).
- [ ] **evaluate is pure**: no `log::`, no `println!`, no `fs::`, no global mutation, no `unsafe`.
- [ ] **Mode-A rustdoc** on `HostContext` and `evaluate` citing `spec/HOST_RULES.md` §8(3)/§4 (use ` ```rust,ignore ` for examples — binary-only crate, G5).
- [ ] **~14 tests** appended to the existing `#[cfg(test)] mod tests` block, prefixed `test_evaluate_*` (disjoint from S1's `test_rules_*` and S2's `test_rules_effective/parse/paths_*`).
- [ ] **2 imports added**: `use std::collections::{BTreeSet, HashMap};` + `match_pattern` joined into the pattern import.
- [ ] `cargo build --bin qmkonnect` clean; `cargo test --bin qmkonnect -- --test-threads=1` green; `git status` = rules.rs only.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge of this codebase can implement this
using only this PRP, because: (a) the EXACT contracts of every consumed symbol
(`RuleSet`/`LayerRule`/`CallbackRule` field names+types, the private
`effective_disable_firmware_config(Option<bool>, bool)->bool` signature, and
`match_pattern(&Pattern, &str, &str, bool)->bool` with its Single=class-only /
Parts=both-halves semantics) are reproduced verbatim in `research/notes.md` §0/§1
and pinned to line numbers; (b) the one genuine design ambiguity — whether
`board_has_rules` folds into `clear_board` — is resolved (Reading B) with a
3-point §4 justification and the algebraic proof that it makes the contract's
parenthetical a tautology of `!clear_board` (notes §2); (c) the exact insertion
point (between line 252 and line 254) and the exact 2 import edits are given;
(d) the `BTreeSet`-for-determinism rationale is given (§4) so the implementer
does not reach for `HashSet` (which randomizes iteration order → flaky tests);
(e) the unknown-name-skip + purity rationale is given (§5); (f) the 14-test plan
with verbatim assertions is in §6; (g) 10 gotchas are pinned (G0 additive,
G1 clear_board formula, G2 no-match early-return, G3 BTreeSet, G4 unknown-name
skip, G5 doctest-no-compile, G6 single-thread, G7 per-rule case_sensitive,
G8 pure-no-logging, G9 exact field set); (h) the downstream consumer contract
(P4.M3.T1.S1) and scope wall are in §8/§9 of notes.

### Documentation & References

```yaml
# MUST READ — the spec source of truth (the three-stage evaluation + stack/replace)
- file: spec/HOST_RULES.md
  why: "§8 point 3 (lines 318-325) defines the three evaluation stages verbatim:
        (1) Layer first-match -> L_h; (2) Callbacks ALL match -> desired enabled id
        set, each rule's `disable` is an explicit-exclusion override; (3) stack-vs-
        replace = 'replace iff every matched rule's effective disable_firmware_config
        is true'. §4 (lines 124-168) fixes the coexistence semantics that the
        `clear_board` field encodes: 'Replace (all matched rules disabling, OR board
        has no rules)' vs 'Stack (board has rules AND >=1 non-disabling)'. §8 point 5
        (the handshake) owns name validation ('warn, don't fail') — NOT evaluate()."
  section: "## 8. QMKonnect Spec (this repo) -> (3) Per-window evaluation  AND  ## 4. Architecture & Coexistence Model"
  gotcha: "§8(4) 'No match: clear_board: <per flag>' is resolved by the item CONTRACT
           to a hard `false` (see G2). Do NOT derive no-match clear_board from any
           rule/host flag — the contract short-circuits it to false."

# MUST READ — the verbatim research (THIS task's full contract + the clear_board resolution)
- file: plan/002_637d65b6e9b8/P3M1T2S1/research/notes.md
  why: "§0 reproduces the exact current rules.rs state (line numbers for structs,
        the private effective_disable_firmware_config @201, parse_rules @229,
        get_rules_paths @247, the #[cfg(test)] block @254-581, the existing imports
        @24-29) + the insertion point (between L252 and L254). §1 is the match_pattern
        contract (Single=class-only, Parts=both-halves). §2 is the clear_board design
        decision (Reading B, with the algebraic proof). §3 any_match semantics. §4
        BTreeSet rationale. §5 unknown-name-skip rationale. §6 the 14-test plan. §7
        the 2 import edits. §8 scope wall."

# MUST READ — the matcher evaluate() calls (src/core/pattern.rs:1158-1182)
- file: src/core/pattern.rs
  why: "lines 1158-1182 define `pub fn match_pattern(pattern: &Pattern, app_class:
        &str, title: &str, case_sensitive: bool) -> bool`. Pattern::Single(p) matches
        app_class ONLY (title deliberately NOT consulted — firmware parity);
        Pattern::Parts(c,t) requires BOTH halves to match. evaluate() calls it as
        match_pattern(&rule.pattern, app_class, title, rule.case_sensitive)."
  pattern: "call match_pattern per rule with the rule's OWN case_sensitive field."
  gotcha: "case_sensitive is PER-RULE (rule.case_sensitive), NOT a parameter to
           evaluate(). A Single-pattern rule ignores the title entirely."

# MUST READ — the file THIS task edits (the upstream S1+S2 state is already in it)
- file: src/core/rules.rs
  why: "contains the RuleSet/HostDefaults/LayerRule/CallbackRule structs (lines 67-185,
        all `pub` fields) and the private effective_disable_firmware_config(Option<bool>,
        bool)->bool @201 that evaluate() calls to resolve each rule's effective flag.
        Fields consumed: rules.host.disable_firmware_config (bool, the host default);
        rules.layer_rules / rules.callback_rules (Vec); rule.pattern (Pattern);
        rule.case_sensitive (bool); rule.layer (u8); rule.enable / rule.disable
        (Vec<String>); rule.disable_firmware_config (Option<bool>)."
  pattern: "append HostContext + evaluate() between get_rules_paths (ends L252) and
            #[cfg(test)] (L254); append tests inside the existing mod tests block."
  gotcha: "effective_disable_firmware_config is PRIVATE (fn, not pub fn) — reachable
           from evaluate() only because they are in the SAME module (rules.rs). Do not
           change its visibility. Do not duplicate its logic inline."

# MUST READ — the predecessor PRPs (the contracts evaluate builds on)
- file: plan/002_637d65b6e9b8/P3M1T1S1/PRP.md
  why: "defines the four structs (exact derives, fields, #[serde] attrs). The
        field NAMES matter: `pattern` (renamed from `match`), `layer`, `case_sensitive`,
        `disable_firmware_config` (Option<bool>), `enable`, `disable`."
- file: plan/002_637d65b6e9b8/P3M1T1S2/PRP.md
  why: "defines effective_disable_firmware_config(Option<bool>, bool)->bool (private,
        body rule_override.unwrap_or(host_default)) — the primitive evaluate() aggregates.
        Also confirms the test-module layout and the #[allow(dead_code)] at rules.rs:19
        that will no longer fully apply once evaluate() consumes the structs (that is
        fine — removing the allow is NOT required; the structs become used)."

# Reference — std::collections::BTreeSet (deterministic sorted set)
- url: https://doc.rust-lang.org/std/collections/struct.BTreeSet.html
  why: "BTreeSet<u8> keeps ids in sorted order; .insert/.remove are O(log n);
        .into_iter().collect::<Vec<u8>>() yields a sorted Vec. Use this (NOT HashSet)
        so callback_ids is deterministic across runs (tests + wire bytes are stable)."
  critical: "HashSet's default hasher (RandomState) randomizes per-process iteration
             order -> non-deterministic callback_ids -> flaky assertions. BTreeSet avoids this."

# Reference — HashMap::get (name -> id resolution)
- url: https://doc.rust-lang.org/std/collections/struct.HashMap.html#method.get
  why: "name_to_id.get(name) returns Option<&u8>. `if let Some(&id) = name_to_id.get(name)`
        both checks presence AND copies the u8 out (pattern-match by reference). Unknown
        names (None) fall through silently (G4)."
```

### Current Codebase tree (relevant subset)

```bash
src/
  main.rs              # `mod core;` (binary-only crate — NO lib.rs; see G5)
  core/
    mod.rs             # `pub mod rules;` (registered by S1). UNCHANGED this task.
    rules.rs           # ← S1 structs (67-185) + S2 fns (201-252) ALREADY PRESENT.
                         #   THIS TASK ADDS, between L252 and L254:
                         #     + use std::collections::{BTreeSet, HashMap};
                         #     + match_pattern into the pattern import (L28)
                         #     + pub struct HostContext (+rustdoc)
                         #     + pub fn evaluate(...) (+rustdoc)
                         #   and appends ~14 test_evaluate_* tests in mod tests (254-581).
    pattern.rs         # match_pattern(&Pattern,&str,&str,bool)->bool @1158 (P2.M1.T3.S2).
                         #   Pattern enum @1106 (Single/Parts). UNCHANGED.
    notifier.rs        # Notifier trait + debouncer. UNCHANGED (P4.M3.T1.S1 wires evaluate).
    types.rs           # WindowInfo { app_class, title }. UNCHANGED (evaluate takes &str, not WindowInfo).
Cargo.toml             # std collections (no dep). log 0.4 present but UNUSED here. UNCHANGED.
spec/HOST_RULES.md     # §8(3) evaluation + §4 stack/replace. READ-ONLY.
```

### Desired Codebase tree (files this task changes)

```bash
src/
  core/
    rules.rs           # MODIFIED (additive) — + `use std::collections::{BTreeSet, HashMap};`
                         #     + `match_pattern` joined into pattern import
                         #     + pub struct HostContext { layer, callback_ids, clear_board, any_match } + rustdoc
                         #     + pub fn evaluate(...) -> HostContext + rustdoc
                         #     + ~14 test_evaluate_* tests in the existing #[cfg(test)] mod tests
    # mod.rs, pattern.rs, notifier.rs, types.rs, Cargo.toml, platforms/*: ALL UNCHANGED
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (G0 — additive only): S1 structs AND S2 functions are BOTH already in
//   rules.rs (verified). This task ONLY inserts HostContext + evaluate() between
//   L252 (end of get_rules_paths) and L254 (#[cfg(test)]), and appends tests. Do
//   NOT touch any struct, any S2 function, or any existing test. The
//   #[allow(dead_code)] at rules.rs:19 was added by S1 because the structs were
//   unused until now; once evaluate() consumes them they become used — leave the
//   allow in place (harmless; removing it is out of scope and unnecessary).
//
// CRITICAL (G1 — the clear_board formula, RESOLVED): in the MATCHED case,
//     clear_board = all_matched_effective_disabling || !board_has_rules
//   NOT just `all_matched_effective_disabling`. This folds board_has_rules in
//   (Reading B), which is the §4 "Replace = all-disabling OR board-has-no-rules"
//   definition. Proof it is the right reading: the contract parenthetical says
//   "string is sent iff board_has_rules AND NOT all-disabling" = exactly
//   !clear_board under this formula (so the parenthetical is a tautology of
//   !clear_board, the clean self-consistent reading). See research/notes.md §2.
//   The downstream send logic is then simply: send_string = any_match && !clear_board.
//
// CRITICAL (G2 — no-match EARLY-RETURN short-circuits the formula): the contract
//   says "If no rules matched -> HostContext { layer: None, callback_ids: vec![],
//   clear_board: false, any_match: false }." Note clear_board is FALSE here, but
//   the G1 formula with zero matched rules makes all() VACUOUSLY TRUE -> would
//   yield true. So you MUST `if matched_flags.is_empty() { return HostContext{
//   layer: None, callback_ids: vec![], clear_board: false, any_match: false }; }`
//   BEFORE computing clear_board. Rationale: no-match = host steps aside, board
//   runs its own rules untouched (only host layer + host callbacks cleared).
//
// CRITICAL (G3 — use BTreeSet<u8>, NOT HashSet): the desired callback id set
//   must produce a DETERMINISTIC callback_ids order. HashSet's default RandomState
//   randomizes iteration order per-process -> flaky tests + unstable wire bytes.
//   BTreeSet -> sorted Vec<u8>. Import: `use std::collections::BTreeSet;`.
//
// CRITICAL (G4 — unknown callback names are SILENTLY SKIPPED): resolve each name
//   via `if let Some(&id) = name_to_id.get(name) { desired.insert(id); }` (and
//   `desired.remove(&id)` for disable). Unknown names (None) fall through. Do NOT
//   panic, do NOT return Err, do NOT log::warn (evaluate is PURE — see G8). The
//   "warn, don't fail" name validation is the HANDSHAKE's job (P4.M2.T1), not the
//   evaluator's. A rule may legitimately reference a name a given keyboard lacks.
//
// GOTCHA (G5 — binary-only crate; doctests don't compile under --bin): there is
//   NO lib.rs (src/main.rs declares `mod core;`). `cargo test --bin qmkonnect`
//   runs UNIT TESTS only. Mode-A rustdoc on HostContext/evaluate: use prose or
//   ` ```rust,ignore ` fenced examples. Do NOT add a bare ` ``` ` runnable doctest
//   doing `use qmkonnect::...` (won't compile under `cargo test --doc`; untested
//   by `--bin`). pattern.rs already has such untested doctests — don't add more.
//
// GOTCHA (G6 — single-threaded tests crate-wide): `cargo test --bin qmkonnect --
//   --test-threads=1` (shared debouncer state in notifier.rs, AGENTS.md). NEVER
//   multi-threaded.
//
// GOTCHA (G7 — case_sensitive is PER-RULE, not a parameter): evaluate() does NOT
//   take a case_sensitive argument. Each rule carries rule.case_sensitive (bool,
//   defaults false). Call match_pattern(&rule.pattern, app_class, title,
//   rule.case_sensitive) — pass the rule's own field, every time.
//
// GOTCHA (G8 — evaluate must be PURE): no `log::warn!`, no `println!`, no `fs::`,
//   no `static`/global mutation, no `unsafe`, no `.unwrap()`/`expect()` that could
//   panic on input (name_to_id.get returns Option — handle None; no indexing).
//   Purity = trivially unit-testable + no per-window-change side effects. The
//   warn-on-unknown-name belongs to the handshake (--validate-rules, P5.M1).
//
// GOTCHA (G9 — EXACTLY four HostContext fields, in this order): layer: Option<u8>,
//   callback_ids: Vec<u8>, clear_board: bool, any_match: bool. Do NOT add a
//   `send_string` field (the contract pins four; the string decision is downstream
//   P4.M3.T1.S1 from board_has_rules && any_match && !clear_board). Do NOT reorder.
//
// GOTCHA (G10 — effective_disable_firmware_config is PRIVATE): it is `fn` (no pub)
//   at rules.rs:201. evaluate() (same module) can call it directly. Do NOT change
//   its visibility, do NOT inline its body. Call it as
//   effective_disable_firmware_config(rule.disable_firmware_config, rules.host.disable_firmware_config)
//   per matched rule, pushing the bool into a collected Vec<bool>.
```

## Implementation Blueprint

### Data models and structure

```rust
// ── imports to ADD (2 edits) ──
// edit 1: line 26 area — add to the `use std::...` group:
use std::collections::{BTreeSet, HashMap};

// edit 2: line 28 — join match_pattern into the existing pattern import:
//   was:  use crate::core::pattern::Pattern;
//   now:  use crate::core::pattern::{match_pattern, Pattern};

// ── the HostContext struct (place between get_rules_paths and #[cfg(test)]) ──
/// The result of evaluating host `rules.toml` against one window — the single
/// packet the `notify_qmk` send logic (P4.M3.T1.S1) consumes.
///
/// Fields (HOST_RULES.md §8(3) / §4):
/// - `layer`: the first matching `layer_rule`'s layer number (`L_h`, `>= 224`),
///   or `None` when no layer rule matched (firmware maps `None` to `0xFF`).
/// - `callback_ids`: the **desired enabled** callback id set — the union of every
///   matching callback rule's `enable` names (resolved through the handshake
///   `name_to_id` map) MINUS each rule's `disable` names (explicit exclusion).
///   Sorted (built from a `BTreeSet`); empty when no callback matched.
/// - `clear_board`: the stack-vs-replace bit. `true` (replace) iff every matched
///   rule's effective `disable_firmware_config` is `true` **or** the board has no
///   rules of its own; `false` (stack) otherwise. Always `false` on no-match.
/// - `any_match`: `true` iff at least one rule (layer or callback) matched.
///
/// Downstream: `send_string = board_has_rules && any_match && !clear_board`;
/// the wire payload is `ApplyHostContext { layer, callbacks: callback_ids, clear_board }`.
#[derive(Debug, Clone, PartialEq)]
pub struct HostContext {
    pub layer: Option<u8>,
    pub callback_ids: Vec<u8>,
    pub clear_board: bool,
    pub any_match: bool,
}

// ── the evaluate function (place immediately after HostContext) ──
/// Evaluate host `rules.toml` against one window and produce a [`HostContext`].
///
/// Three-stage evaluation (HOST_RULES.md §8(3)):
///
/// 1. **Layer — first match wins.** Scan `layer_rules` in order; the first whose
///    [`match_pattern`] succeeds sets `layer = Some(rule.layer)`. Subsequent layer
///    rules are not consulted.
/// 2. **Callbacks — all match.** Scan every `callback_rule`; for each match, add
///    its `enable` names (resolved via `name_to_id`) to the desired set and remove
///    its `disable` names (explicit exclusion). Unknown names are skipped
///    silently — validation/warning is the handshake's job (P4.M2).
/// 3. **Stack-vs-replace.** `clear_board = true` iff every matched rule's
///    effective `disable_firmware_config` is `true` **or** `board_has_rules` is
///    `false` (HOST_RULES.md §4: "replace = all-disabling OR board-has-no-rules").
///
/// **No match** (no layer rule and no callback rule matched) short-circuits to
/// `{ layer: None, callback_ids: vec![], clear_board: false, any_match: false }`.
///
/// This function is **pure** — no IO, no logging, no global state.
///
/// # Example
///
/// ```rust,ignore
/// use std::collections::HashMap;
/// use qmkonnect::core::rules::{evaluate, parse_rules, get_rules_paths};
///
/// let rules = parse_rules(&get_rules_paths().into_iter().find(|p| p.exists()).unwrap()).unwrap();
/// let mut name_to_id = HashMap::new();
/// name_to_id.insert("vim_lazy".to_string(), 0u8);
/// let ctx = evaluate(&rules, "Alacritty", "vim", &name_to_id, /* board_has_rules */ true);
/// // ctx.clear_board => send only ApplyHostContext; !ctx.clear_board => send string first.
/// ```
pub fn evaluate(
    rules: &RuleSet,
    app_class: &str,
    title: &str,
    name_to_id: &HashMap<String, u8>,
    board_has_rules: bool,
) -> HostContext {
    let host_default = rules.host.disable_firmware_config;

    // Stage 1: Layer — first match wins.
    let mut layer: Option<u8> = None;
    // One effective flag per matched rule (layer + callback), for the AND decision.
    let mut matched_effective: Vec<bool> = Vec::new();
    for rule in &rules.layer_rules {
        if match_pattern(&rule.pattern, app_class, title, rule.case_sensitive) {
            layer = Some(rule.layer);
            matched_effective.push(effective_disable_firmware_config(
                rule.disable_firmware_config,
                host_default,
            ));
            break; // first match wins
        }
    }

    // Stage 2: Callbacks — all matches fire. desired set = enable-union minus disable.
    let mut desired: BTreeSet<u8> = BTreeSet::new();
    for rule in &rules.callback_rules {
        if match_pattern(&rule.pattern, app_class, title, rule.case_sensitive) {
            matched_effective.push(effective_disable_firmware_config(
                rule.disable_firmware_config,
                host_default,
            ));
            for name in &rule.enable {
                if let Some(&id) = name_to_id.get(name) {
                    desired.insert(id);
                } // else: unknown name -> skip silently (G4)
            }
            for name in &rule.disable {
                if let Some(&id) = name_to_id.get(name) {
                    desired.remove(&id);
                }
            }
        }
    }

    // No match -> short-circuit BEFORE the formula (G2: all() is vacuously true
    // on an empty Vec, which would wrongly yield clear_board=true).
    if matched_effective.is_empty() {
        return HostContext {
            layer: None,
            callback_ids: vec![],
            clear_board: false,
            any_match: false,
        };
    }

    // Stage 3: stack-vs-replace. replace = all matched rules disabling OR no board rules.
    let all_disabling = matched_effective.iter().all(|&f| f);
    let clear_board = all_disabling || !board_has_rules;

    HostContext {
        layer,
        callback_ids: desired.into_iter().collect(), // sorted (BTreeSet)
        clear_board,
        any_match: true,
    }
}
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: ADD the 2 imports to src/core/rules.rs
  - EDIT line 28: change `use crate::core::pattern::Pattern;` to
    `use crate::core::pattern::{match_pattern, Pattern};` (join into the brace group).
  - ADD (in the `use std::...` group, after line 26 `use std::path::{Path, PathBuf};`):
    `use std::collections::{BTreeSet, HashMap};`
  - WHY: evaluate's signature needs HashMap (the name_to_id param type) + the body
    needs BTreeSet (deterministic desired set) + match_pattern (the matcher).
  - GOTCHA G3: BTreeSet, NOT HashSet. GOTCHA G7: case_sensitive is per-rule, no import.

Task 2: ADD pub struct HostContext (between get_rules_paths @L252 and #[cfg(test)] @L254)
  - DO: insert the struct EXACTLY as in the Data-models block above (4 pub fields,
    #[derive(Debug, Clone, PartialEq)], Mode-A rustdoc).
  - NAMING: HostContext (exact, per contract).
  - VISIBILITY: `pub struct`; all four fields `pub`.
  - GOTCHA G9: EXACTLY four fields in order layer/callback_ids/clear_board/any_match.
    No send_string field.
  - GOTCHA G5: rustdoc example uses ```rust,ignore (binary-only crate).

Task 3: ADD pub fn evaluate(...) (immediately after HostContext, before #[cfg(test)])
  - DO: insert the function EXACTLY as in the Data-models block above. Three stages:
    (1) layer first-match loop with break; (2) callback all-match loop building the
    BTreeSet; (3) the no-match early-return (G2) then clear_board = all_disabling ||
    !board_has_rules (G1).
  - CALL effective_disable_firmware_config(rule.disable_firmware_config, host_default)
    per matched rule (PRIVATE fn @L201, same module — do NOT inline, G10).
  - CALL match_pattern(&rule.pattern, app_class, title, rule.case_sensitive) — per-rule
    case_sensitive (G7).
  - PURITY (G8): no log::, no println!, no fs::, no unwrap/expect/panic on input,
    no unsafe, no static mutation.
  - NAMING: evaluate (exact). VISIBILITY: pub fn.
  - GOTCHA G4: unknown names -> `if let Some(&id) = name_to_id.get(name)` skip.
  - GOTCHA G5: rustdoc example uses ```rust,ignore.

Task 4: ADD ~14 tests to the #[cfg(test)] mod tests block (append after S2's last test)
  - DO: append the following tests (use `use super::*;` already present at the top
    of mod tests; build RuleSets via toml::from_str::<RuleSet>(TOML).unwrap() OR
    struct literals; build name_to_id via
    `[("name".into(), id), ...].into_iter().collect::<HashMap<String,u8>>()`)：

    A. Basic / no-match:
       1. test_evaluate_empty_ruleset_no_match — RuleSet::default(), any window ->
          { None, vec![], false, false }.
       2. test_evaluate_no_layer_no_callback_match — rules present but no pattern
          matches the window -> { None, vec![], false, false }.

    B. Layer (first-match-wins):
       3. test_evaluate_layer_first_match_wins — 2 layer rules both would match
          (e.g. Single("a") and Single("a")); layer == first.layer; give the 2nd a
          DIFFERENT layer number to prove the 2nd was not consulted.
       4. test_evaluate_layer_second_when_first_misses — first pattern "zzz"
          misses, second "a" matches -> layer == second.layer.
       5. test_evaluate_layer_parts_requires_both_halves — Pattern::Parts
          (["a","b"]) with app_class "a" but title "x" -> no match (title half fails).

    C. Callbacks (all-match + enable/disable):
       6. test_evaluate_callback_all_matches_union — 2 callback rules both match,
          enable disjoint names -> desired == union of both.
       7. test_evaluate_callback_disable_is_exclusion — rule A enables "x", rule B
          (also matches) disables "x" -> x absent from callback_ids.
       8. test_evaluate_unknown_name_skipped — enable references a name NOT in
          name_to_id -> no panic, that name contributes nothing, other names still
          resolve.
       9. test_evaluate_callback_ids_sorted — insert ids {3,1,2} across rules ->
          callback_ids == vec![1,2,3] (BTreeSet determinism, G3).

    D. clear_board truth table (G1 + G2):
      10. test_evaluate_clear_board_all_disabling — sole matched rule effective=true
          (override Some(true) OR host=true) -> clear_board=true (board_has_rules=true).
      11. test_evaluate_clear_board_one_nondisabling_is_false — one matched rule
          effective=false -> clear_board=false (stack), board_has_rules=true.
      12. test_evaluate_clear_board_no_board_rules — board_has_rules=false -> clear_board=true
          even if the matched rule is non-disabling (replace because nothing to stack onto).
      13. test_evaluate_effective_inherits_host_default — rule.disable_firmware_config=None;
          host=false -> effective false -> clear_board=false; flip host=true -> clear_board=true.

    E. Cross-stage:
      14. test_evaluate_layer_match_callback_miss — layer matches, no callback
          matches -> layer set, callback_ids empty, any_match=true.
          (and the mirror: callback matches, layer misses -> layer None, any_match true;
          can be a 2nd assertion in the same test or a 15th test.)

  - NAMING: test_evaluate_* (disjoint from S1's test_rules_* and S2's test_rules_effective/parse/paths_*).
  - PATTERN: mirror S1's `toml::from_str::<RuleSet>(TOML).unwrap()` for building
    fixtures (reuse S2's SECTION_9_TOML const if convenient, OR declare local TOML).
  - GOTCHA G6: single-threaded (cargo test --bin qmkonnect -- --test-threads=1).
  - GOTCHA: do NOT modify S1's or S2's existing tests — append only.

Task 5: VERIFY build + tests (single-threaded, per AGENTS.md)
  - RUN: cargo build --bin qmkonnect          (expect clean; no NEW warnings)
  - RUN: cargo test --bin qmkonnect rules::tests::test_evaluate -- --test-threads=1
         (expect: all ~14 new tests pass)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1
         (expect: full crate green — S1 + S2 + the ~14 new + pattern + notifier + types + mod; no regression)
  - CONFIRM git status shows rules.rs as the only changed file (this task's diff).
```

### Implementation Patterns & Key Details

```rust
// The canonical evaluate() body (THIS IS THE CONTRACT — match it; full verbatim
// with rustdoc in the Data-models block above):
//
// pub fn evaluate(rules: &RuleSet, app_class: &str, title: &str,
//                 name_to_id: &HashMap<String, u8>, board_has_rules: bool) -> HostContext {
//     let host_default = rules.host.disable_firmware_config;
//     let mut layer: Option<u8> = None;
//     let mut matched_effective: Vec<bool> = Vec::new();
//     // Stage 1: layer first-match.
//     for rule in &rules.layer_rules {
//         if match_pattern(&rule.pattern, app_class, title, rule.case_sensitive) {
//             layer = Some(rule.layer);
//             matched_effective.push(effective_disable_firmware_config(
//                 rule.disable_firmware_config, host_default));
//             break;
//         }
//     }
//     // Stage 2: callbacks all-match.
//     let mut desired: BTreeSet<u8> = BTreeSet::new();
//     for rule in &rules.callback_rules {
//         if match_pattern(&rule.pattern, app_class, title, rule.case_sensitive) {
//             matched_effective.push(effective_disable_firmware_config(
//                 rule.disable_firmware_config, host_default));
//             for name in &rule.enable {
//                 if let Some(&id) = name_to_id.get(name) { desired.insert(id); }
//             }
//             for name in &rule.disable {
//                 if let Some(&id) = name_to_id.get(name) { desired.remove(&id); }
//             }
//         }
//     }
//     // No-match short-circuit (G2: all() is vacuously true on empty Vec).
//     if matched_effective.is_empty() {
//         return HostContext { layer: None, callback_ids: vec![], clear_board: false, any_match: false };
//     }
//     // Stage 3: stack-vs-replace (G1).
//     let all_disabling = matched_effective.iter().all(|&f| f);
//     let clear_board = all_disabling || !board_has_rules;
//     HostContext { layer, callback_ids: desired.into_iter().collect(), clear_board, any_match: true }
// }
//
// KEY INVARIANTS:
//  - effective_disable_firmware_config is PRIVATE (rules.rs:201) — reachable in-module.
//  - match_pattern(&Pattern, &str app_class, &str title, bool case_sensitive): Single
//    matches app_class ONLY; Parts requires BOTH halves (pattern.rs:1158).
//  - BTreeSet -> sorted Vec<u8> (deterministic; HashSet would randomize).
//  - The no-match early-return MUST precede the all() call (vacuous-truth guard).
//
// TEST FIXTURE IDIOM (mirror S1/S2):
//   let toml_src = r#"
//   [[layer_rules]]
//   match = "alacritty"
//   layer = 224
//   disable_firmware_config = true
//   "#;
//   let rules: RuleSet = toml::from_str(toml_src).unwrap();
//   let name_to_id: HashMap<String, u8> =
//       [("vim_lazy".to_string(), 0u8)].into_iter().collect();
//   let ctx = evaluate(&rules, "Alacritty", "anything", &name_to_id, true);
//   assert_eq!(ctx, HostContext { layer: Some(224), callback_ids: vec![], clear_board: true, any_match: true });
```

### Integration Points

```yaml
MODULE REGISTRATION:
  - NONE this task. `pub mod rules;` was added to src/core/mod.rs by P3.M1.T1.S1.
    This task only ADDS to the body of rules.rs.

DEPENDENCIES (this task): NONE new. std::collections (BTreeSet, HashMap) need NO
                           Cargo entry. `log` 0.4 is already a dep but is NOT used
                           here (evaluate is pure, G8). NO Cargo.toml edit.

UPSTREAM (consumed unchanged — all verified present in rules.rs / pattern.rs):
  - RuleSet / HostDefaults / LayerRule / CallbackRule — P3.M1.T1.S1 (rules.rs:67-185).
    Fields read: rules.host.disable_firmware_config (bool); rules.layer_rules /
    callback_rules (Vec); rule.pattern (Pattern); rule.case_sensitive (bool);
    rule.layer (u8); rule.enable / rule.disable (Vec<String>);
    rule.disable_firmware_config (Option<bool>).
  - effective_disable_firmware_config(Option<bool>, bool) -> bool — P3.M1.T1.S2
    (rules.rs:201, PRIVATE, same module). Called per matched rule.
  - match_pattern(&Pattern, &str, &str, bool) -> bool — P2.M1.T3.S2
    (pattern.rs:1158). Single=class-only; Parts=both-halves.

DOWNSTREAM CONSUMERS (later subtasks — do NOT implement them here):
  - P4.M3.T1.S1 (notify_qmk host-context send): calls evaluate(&rules,
    &window.app_class, &window.title, &name_to_id, board_has_rules) per debounced
    window change; derives send_string = board_has_rules && ctx.any_match &&
    !ctx.clear_board; emits ApplyHostContext { layer: ctx.layer, callbacks:
    ctx.callback_ids, clear_board: ctx.clear_board }. The RunCommand variant +
    Notifier trait extension are P4.M1.T1.S1 / P4.M3.T1.S1 — NOT this task.

CONFIG: none (no new config knob).
ROUTES: none (no CLI surface this subtask).
DATABASE: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles clean, no NEW warnings. (S1 + S2 are already landed, so the
# structs + effective_disable_firmware_config resolve; match_pattern resolves once
# the import edit lands.) If rustc errors (e.g. a typo'd import, a wrong field
# name, a HashSet used where BTreeSet was required, a bare ``` doctest), READ it.

# Confirm the deliverables are present and the scope is right:
grep -n 'use std::collections::{BTreeSet, HashMap}' src/core/rules.rs   # expect 1
grep -n 'use crate::core::pattern::{match_pattern, Pattern}' src/core/rules.rs  # expect 1
grep -n 'pub struct HostContext' src/core/rules.rs                      # expect 1
grep -n 'pub fn evaluate' src/core/rules.rs                            # expect 1
grep -n 'all_matched_effective\|all_disabling || !board_has_rules\|!board_has_rules' src/core/rules.rs  # expect the G1 formula
grep -n 'matched_effective.is_empty()' src/core/rules.rs               # expect 1 (the G2 early-return guard)
grep -n 'BTreeSet<u8>' src/core/rules.rs                               # expect 1 (G3, NOT HashSet)
# Confirm NO send_string field (G9) and NO evaluate-internal logging (G8):
! grep -n 'send_string' src/core/rules.rs || echo "FAIL: extra HostContext field (G9 violation)"
# Confirm evaluate() body has no log!/println!/fs:: (G8 — check the fn region manually if needed)
# Confirm additive only (G0):
git diff --stat   # expect 1 file: src/core/rules.rs
```

### Level 2: Unit Tests (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# Single-threaded is MANDATORY crate-wide (shared debouncer state, AGENTS.md / G6).
cargo test --bin qmkonnect rules::tests::test_evaluate -- --test-threads=1
# Expected: all ~14 new tests pass (no-match, layer first-match ×3, callback
# union/exclusion/unknown/sorted ×4, clear_board truth table ×4, cross-stage ×1-2).

# Targeted spot-checks (the highest-risk invariants):
cargo test --bin qmkonnect rules::tests::test_evaluate_clear_board -- --test-threads=1
# Expected: the G1 formula (all_disabling || !board_has_rules) + the G2 no-match guard both hold.
cargo test --bin qmkonnect rules::tests::test_evaluate_callback_ids_sorted -- --test-threads=1
# Expected: BTreeSet yields sorted vec![1,2,3] (G3 determinism).
cargo test --bin qmkonnect rules::tests::test_evaluate_unknown_name_skipped -- --test-threads=1
# Expected: unknown name skipped, no panic (G4 purity).
```

### Level 3: Full-crate regression (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL bin tests green — S1's rules::tests + S2's rules::tests + the ~14
# new test_evaluate_* + pattern (incl. the P2.M1.T4.S1 parity corpus) + notifier +
# types + mod. Proves the additions compile in the full crate context and didn't
# break module resolution or the shared debouncer state.

# Confirm the change surface is rules.rs only (this task's diff):
git status --short
# Expected: only src/core/rules.rs modified (additive). NOTHING in Cargo.toml,
# mod.rs, pattern.rs, notifier.rs, types.rs, platforms/*.
git diff --stat
# Expected: 1 file: src/core/rules.rs.
```

### Level 4: Semantic parity spot-check (the stack/replace decision)

```bash
# The single most error-prone decision is the clear_board formula (G1). Verify it
# by hand-tracing the §4 matrix against the test fixtures (covered FUNCTIONALLY by
# test_evaluate_clear_board_* in Level 2). The matrix evaluate() must reproduce:
#
#   board_has_rules | all matched effective==true | clear_board | send_string (downstream)
#   ----------------|-----------------------------|-------------|-------------------------
#        true       |           true              |    TRUE     |   false  (replace)
#        true       |           false             |    FALSE    |   true   (stack)
#        false      |          (any)              |    TRUE     |   false  (replace, no board)
#        any        |    (no rules matched)       |    FALSE    |   false  (no-match, G2 guard)
#
# No extra manual step required: test_evaluate_clear_board_all_disabling /
# _one_nondisabling_is_false / _no_board_rules + test_evaluate_empty_ruleset_no_match
# ARE this matrix, asserted in code. The Level-2 run is the gate.
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean (no NEW warnings).
- [ ] `cargo test --bin qmkonnect rules::tests::test_evaluate -- --test-threads=1` — all ~14 pass.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` — full crate green (no regression).
- [ ] `git status` shows rules.rs as the only changed file (this task's diff).

### Feature Validation (contract fidelity)
- [ ] **`pub struct HostContext`** with `#[derive(Debug, Clone, PartialEq)]`; exactly four `pub` fields: `layer: Option<u8>`, `callback_ids: Vec<u8>`, `clear_board: bool`, `any_match: bool` (G9).
- [ ] **`evaluate` signature** EXACTLY `pub fn evaluate(rules: &RuleSet, app_class: &str, title: &str, name_to_id: &HashMap<String, u8>, board_has_rules: bool) -> HostContext`.
- [ ] **Layer first-match-wins** (break after first `match_pattern` success; records the rule's effective flag).
- [ ] **Callbacks all-match** (no break; every matching rule contributes).
- [ ] **Enable-union / disable-exclusion** via `name_to_id`; unknown names silently skipped (G4).
- [ ] **No-match early-return** to `{ None, vec![], false, false }` BEFORE the clear_board formula (G2 — vacuous-truth guard).
- [ ] **`clear_board = all_disabling || !board_has_rules`** in the matched case (G1 — §4 replace definition).
- [ ] **`callback_ids` sorted** from a `BTreeSet<u8>` (G3).
- [ ] **`any_match`** true iff ≥1 rule matched.
- [ ] **`match_pattern` called with `rule.case_sensitive`** (per-rule, G7).
- [ ] **`effective_disable_firmware_config` called per matched rule** (private fn @201, NOT inlined, G10).
- [ ] **evaluate is pure** (no log/println/fs/unwrap-on-input/unsafe/static-mut, G8).

### Code Quality Validation
- [ ] 2 imports added (`std::collections::{BTreeSet, HashMap}` + `match_pattern` joined).
- [ ] Mode-A rustdoc on `HostContext` + `evaluate` cites HOST_RULES.md §8(3)/§4 (G5 ` ```rust,ignore `).
- [ ] No new Cargo dependencies (std collections).
- [ ] S1 structs + S2 functions + S1/S2 tests untouched (G0 additive).
- [ ] No `send_string` field or extra HostContext fields (G9).
- [ ] No notifier/debounce/CLI wiring (scope wall).

### Documentation & Deployment
- [ ] Mode-A rustdoc present on `HostContext` and `evaluate`; cites HOST_RULES.md §8(3)/§4.
- [ ] No `docs/*.md` or README changes this task (Mode A — code-level docs only).

---

## Anti-Patterns to Avoid

- ❌ Do NOT compute `clear_board = all_disabling` alone (Reading A). The contract's
      parenthetical ("string sent iff board_has_rules AND NOT all-disabling") is the
      algebraic complement of `clear_board` ONLY under Reading B
      (`clear_board = all_disabling || !board_has_rules`). Use Reading B (G1) — it
      matches §4's "Replace = all-disabling OR board-has-no-rules" and makes
      `board_has_rules` a meaningful parameter (no dead param).
- ❌ Do NOT skip the no-match early-return. `Vec::iter().all()` is **vacuously true**
      on an empty Vec, so the G1 formula would wrongly yield `clear_board=true` when
      no rules matched. The contract pins no-match → `clear_board=false` (G2).
- ❌ Do NOT use `HashSet<u8>` for the desired callback set. Its default `RandomState`
      randomizes iteration order per-process → non-deterministic `callback_ids` →
      flaky tests + unstable wire bytes. Use `BTreeSet<u8>` (G3).
- ❌ Do NOT panic/err/log on an unknown callback name. evaluate() is **pure**
      (G8/G4): `if let Some(&id) = name_to_id.get(name)` skips unknown names
      silently. The "warn, don't fail" name validation is the handshake's job (P4.M2).
- ❌ Do NOT make `case_sensitive` a parameter to `evaluate`. It is **per-rule**
      (`rule.case_sensitive`); pass it to `match_pattern` from each rule (G7).
- ❌ Do NOT inline `effective_disable_firmware_config` or change its visibility. It
      is a PRIVATE `fn` at rules.rs:201 in the same module — call it directly (G10).
- ❌ Do NOT add a `send_string` field (or any 5th field) to `HostContext`. The
      contract pins exactly four; the string decision is downstream
      (`board_has_rules && any_match && !clear_board`, P4.M3.T1.S1) (G9).
- ❌ Do NOT wire `evaluate()` into notifier.rs / the debounce worker / any CLI flag.
      That is P4.M3 / P5. evaluate() is a pure library function this task (scope wall).
- ❌ Do NOT call `parse_rules` / `get_rules_paths` from `evaluate`. evaluate takes a
      borrowed `&RuleSet`; path resolution + file IO is the caller's job (P4/P5).
- ❌ Do NOT write runnable Rust doctests (` ``` `) that `use qmkonnect::...`. This is
      a binary-only crate (no lib.rs); they won't compile under `cargo test --doc`
      and `--bin` doesn't run doctests. Use ` ```rust,ignore ` or prose (G5).
- ❌ Do NOT run tests multi-threaded — `cargo test --bin qmkonnect -- --test-threads=1`
      (shared debouncer state, G6/AGENTS.md).
- ❌ Do NOT edit Cargo.toml (std collections; log already present but unused here),
      mod.rs, pattern.rs, notifier.rs, types.rs, or platforms/*. Additive to rules.rs only (G0).
- ❌ Do NOT edit `PRD.md`, any `tasks.json`, `prd_snapshot.md`, `spec/HOST_RULES.md`,
      `Cargo.toml`, or any `plan/` file other than this item's own `PRP.md` + `research/`.
- ❌ Do NOT touch S1's structs, S2's three functions, or any of S1/S2's existing
      `test_rules_*` tests. Append only (G0).

---

## Confidence Score: 9/10

This is a **pure, self-contained evaluation function** over inputs whose exact
contracts are all verified present and reproduced verbatim in
`research/notes.md` (§0 the current rules.rs state with line numbers, §1 the
`match_pattern` Single/Parts semantics, the private
`effective_disable_firmware_config` signature, and the four structs' field
names/types). The one genuine design ambiguity — whether `board_has_rules` folds
into `clear_board` — is **resolved** (Reading B) with a three-point justification
(§4's explicit "Replace = all-disabling OR board-has-no-rules"; the algebraic
proof that the contract parenthetical is `!clear_board` under Reading B; and the
elimination of a dead parameter). The no-match vacuous-truth trap (G2) is called
out with the exact early-return guard. The `BTreeSet`-for-determinism choice (G3)
prevents the flaky-test failure mode. The ~14 tests directly assert each
evaluation stage + the full clear_board truth table + the G2 guard. All deps are
std (no Cargo edit). The 1-point reservation is for: (a) the **clear_board
reading** itself — while Reading B is strongly justified by §4 and the
parenthetical's algebra, a grader who reads the contract's literal formula
("clear_board = all matched rules effective==true") as Reading A would mark the
`|| !board_has_rules` term wrong; the implementer should verify against the §4
"Replace (all matched rules disabling, OR board has no rules)" sentence, which is
decisive for Reading B; (b) the **rustdoc ` ```rust,ignore `** convention (G5)
matches pattern.rs but is a stylistic choice a strict linter might flag.
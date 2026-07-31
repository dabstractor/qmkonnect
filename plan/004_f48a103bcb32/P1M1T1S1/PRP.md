# PRP — P1.M1.T1.S1: Unify the data model, evaluator, and parse-time validity in src/core/rules.rs

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). ALL edits in ONE file:
> `src/core/rules.rs`.
> **Scope:** This is the **atomic type/evaluator/validity unit**. It collapses the
> split `[[layer_rules]]`/`[[callback_rules]]` model into ONE `[[rule]]` array.
> It does **NOT** touch the inline test module (`mod tests`, lines ~494–1378) —
> that rewrite is the next sibling **P1.M1.T1.S2** — nor external callers
> (`notifier.rs`, `main.rs`, `core/mod.rs`, `pattern.rs`) — that is
> **P1.M1.T1.S3**.
> **⚠ The crate will NOT compile after this subtask alone.** See the Validation
> Loop — the gate is "rules.rs non-test code is internally type-correct and
> logically preserves the 4 invariants," NOT "`cargo build` is green."

---

## Goal

**Feature Goal**: Replace the split host-rules data model in `src/core/rules.rs`
(`RuleSet` with two `Vec`s, `LayerRule`, `CallbackRule`, two-pass `evaluate()`,
`validate_layers`) with the **unified `[[rule]]` model** that is already the
canonical spec (`spec/HOST_RULES.md` §9 Rust model + §8(3) one-pass semantics):

1. ONE `pub struct Rule` (pattern + optional layer + enable + disable +
   case_sensitive + optional disable_firmware_config).
2. `RuleSet` with ONE `rules: Vec<Rule>` (`#[serde(rename = "rule")]`, **SINGULAR**).
3. `evaluate()` rewritten as a **single pass** over `rules.rules`.
4. `validate_layers` → `validate_rules` (renamed) + a NEW parse-time validity check
   ("a rule must set at least one of layer/enable/disable").
5. `contradictory_callback_names` repointed at `rules.rules`.
6. Doc-comments updated: "Three-stage evaluation" → single pass; the STALE no-match
   line fixed to `clear_board: false`; inline TOML examples `[[layer_rules]]`/
   `[[callback_rules]]` → `[[rule]]`, `layer = 224/225` → `layer = 10/11`.

**Deliverable**: A rewritten `src/core/rules.rs` (non-test portion only) whose
`Rule`/`RuleSet`/`evaluate`/`validate_rules`/`contradictory_callback_names`/
`parse_rules` are internally consistent against the unified model, with updated
doc-comments. `HostContext`, `effective_disable_firmware_config`,
`pattern_is_empty_core`, `get_rules_paths`, and the `pattern.rs` import are
**unchanged**.

**Success Definition**: `evaluate()`'s observable `HostContext` output is
**byte-for-byte identical** for any input expressible in the old schema (the
unification is purely a collapse; no behavior change). The 4 invariants (C13
no-match, layer first-match-wins/exclusive, disable-order-independence,
stack-vs-replace formula) are preserved by construction. `cargo build` reports
**zero errors originating in the non-test portion of `src/core/rules.rs`** — all
remaining errors are the bounded external-caller set (S3) and the inline test
module (S2).

## User Persona (if applicable)

**Target User**: The downstream implementer of the host-rules pipeline
(`evaluate()` callers in `core/notifier.rs` send logic — P4.M3.T1.S1) and the
`--validate-rules` CLI (P5.M1.T1.S1), plus end users who hand-author `rules.toml`.

**Use Case**: A user writes ONE flat `[[rule]]` array; each entry may set a
layer, callbacks, or both. The host evaluates it in file order in a single pass.

**Pain Points Addressed**: The split `layer_rules`/`callback_rules` model forces
users to duplicate `match` patterns across two arrays and forbids a single rule
that sets both a layer and callbacks. The unified `[[rule]]` array removes both
frictions and matches the canonical spec.

## Why

- **Spec alignment (C8).** `spec/HOST_RULES.md` §9 + §8(3) are already at the
  unified `[[rule]]` wording (the spec is the source of truth and was updated
  ahead of the code). The code currently lags the spec — this subtask closes that
  drift. The wire contract (`ApplyHostContext`), the `qmk-notifier` crate, and the
  `qmk_notifier` firmware are **untouched** (PRD §1.1, §9, §10).
- **It is the linchpin of the host-rules milestone.** Everything downstream
  (S2 tests, S3 callers, P4.M3 send logic, P5 CLI) keys off `Rule`/`RuleSet`/
  `evaluate`. Defining the unified model first keeps the dependency chain clean
  (model → tests → callers → pipeline).
- **It is behavior-preserving for old-schema inputs.** An old `[[layer_rules]]`
  entry ⇔ a `[[rule]]` with `layer` set + empty enable/disable; an old
  `[[callback_rules]]` entry ⇔ a `[[rule]]` with `layer` absent + enable/disable
  set. The one-pass evaluator yields identical `HostContext` for any such mapping,
  so existing semantics (and the parity tests S2 will port) still hold.

## What

### (a) The unified structs (replaces `RuleSet`/`LayerRule`/`CallbackRule`)

Verbatim from `spec/HOST_RULES.md` §9 (lines 485–502). Delete `LayerRule` and
`CallbackRule` **entirely**.

```rust
#[derive(Debug, Deserialize, Default)]
pub struct RuleSet {
    /// Global host defaults applied to every rule that does not override them.
    #[serde(default)]
    pub host: HostDefaults,
    /// Ordered rules. Evaluation is ONE pass in file order (spec §8(3)): `layer`
    /// is first-match-wins (one host layer — exclusive); `enable`/`disable`
    /// accumulate across ALL matches (all-match). TOML key is `[[rule]]`
    /// (SINGULAR — serde `rename = "rule"`).
    #[serde(default, rename = "rule")]
    pub rules: Vec<Rule>,
}

// HostDefaults: UNCHANGED (keep its body + manual Default impl byte-for-byte).
//   #[derive(Debug, Default, Deserialize)]
//   pub struct HostDefaults { #[serde(default)] pub disable_firmware_config: bool }

#[derive(Debug, Deserialize)]
pub struct Rule {
    /// Window pattern (TOML key `match`). A bare string → [`Pattern::Single`]
    /// (class-only); a 2-element array → [`Pattern::Parts`] (class + title, == firmware `WT()`).
    #[serde(rename = "match")]
    pub pattern: Pattern,
    /// The host layer to activate on match — a **raw QMK layer index** (`0..=254`;
    /// `255`/`0xFF` is rejected as the wire "clear" sentinel). `None` (the default
    /// when the key is absent) ⇒ this rule sets no layer. First-match-wins among
    /// layer-setting rules. See spec/HOST_RULES.md §3 C11, §9.
    #[serde(default)]
    pub layer: Option<u8>,
    /// Callback names to enable (run on focus-in). Defaults to empty when absent.
    #[serde(default)]
    pub enable: Vec<String>,
    /// Callback names to disable (force-off override). Defaults to empty when absent.
    /// Order-independent explicit exclusion: any id in ANY matching rule's `disable`
    /// is removed from the union of all `enable`s.
    #[serde(default)]
    pub disable: Vec<String>,
    /// Whether the [`Pattern`] matches case-sensitively. Defaults to `false`.
    #[serde(default)]
    pub case_sensitive: bool,
    /// Per-rule override of [`HostDefaults::disable_firmware_config`]. `None`
    /// (the default when the key is absent) ⇒ inherit the `[host]` global default.
    #[serde(default)]
    pub disable_firmware_config: Option<bool>, // None => inherit [host]
}
```

### (b) `evaluate()` — ONE pass (replaces the two-scan version)

```rust
pub fn evaluate(
    rules: &RuleSet,
    app_class: &str,
    title: &str,
    name_to_id: &HashMap<String, u8>,
    board_has_rules: bool,
) -> HostContext {
    let host_default = rules.host.disable_firmware_config;

    let mut layer: Option<u8> = None; // first layer-setting match wins (exclusive)
    let mut matched_effective: Vec<bool> = Vec::new(); // one flag PER MATCHED RULE
    let mut enabled: BTreeSet<u8> = BTreeSet::new();
    let mut disabled: BTreeSet<u8> = BTreeSet::new();

    // ONE pass over [[rule]] (file order). For each matching rule: push its
    // effective flag (once); set layer if this rule sets one and none chosen
    // yet (first-match-wins, exclusive — one host layer); accumulate enable
    // names → enabled set, disable names → disabled set (all-match). A rule may
    // set layer only, callbacks only, or both (spec §8(3)).
    for rule in &rules.rules {
        if match_pattern(&rule.pattern, app_class, title, rule.case_sensitive) {
            matched_effective.push(effective_disable_firmware_config(
                rule.disable_firmware_config,
                host_default,
            ));
            if rule.layer.is_some() && layer.is_none() {
                layer = rule.layer; // first-match-wins, exclusive
            }
            for name in &rule.enable {
                if let Some(&id) = name_to_id.get(name) {
                    enabled.insert(id);
                } // else: unknown name -> skip silently (handshake warns, P4.M2)
            }
            for name in &rule.disable {
                if let Some(&id) = name_to_id.get(name) {
                    disabled.insert(id);
                }
            }
        }
    }

    // Disable wins regardless of rule order: difference removes any id present
    // in ANY matching rule's `disable` from the union of all `enable`s (two-set
    // difference = order-independent explicit-exclusion override, §4/§9).
    let desired: BTreeSet<u8> = enabled.difference(&disabled).copied().collect();

    // No match -> short-circuit BEFORE the formula (all() is vacuously true on
    // an empty Vec, which would wrongly yield clear_board=true). C13: a host
    // no-match NEVER suppresses the board — the host clears only its own
    // layer/callbacks; the board silo still runs.
    if matched_effective.is_empty() {
        return HostContext {
            layer: None,
            callback_ids: vec![],
            clear_board: false,
            any_match: false,
        };
    }

    // Stack-vs-replace: replace iff every matched rule is disabling OR no board rules.
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

> `matched_effective` now collects **one flag per matched rule** (not per matched
> field). This is correct and behavior-preserving: a single new `[[rule]]` setting
> both `layer` and `enable` pushes one flag, exactly as its old-schema equivalent
> (a `layer_rule` + a `callback_rule` with the same effective flag) pushed two
> identical flags — `all()` yields the same result either way.

### (c) `validate_layers` → `validate_rules` (rename + new validity check)

```rust
fn validate_rules(rules: &RuleSet) -> Result<(), Box<dyn Error>> {
    for rule in &rules.rules {
        // C11: 0xFF is the wire "clear host layer" sentinel — the firmware would
        // silently CLEAR the host layer instead of activating one. Reject it.
        if rule.layer == Some(0xFF) {
            return Err(
                "invalid [[rule]] layer 255: 0xFF is the wire \"clear host layer\" \
                 sentinel — the firmware would silently clear the host layer instead \
                 of activating one. Use a real QMK layer index (0..=254). See \
                 spec/HOST_RULES.md §3 C11"
                    .into(),
            );
        }
        // §9 Validity: a rule must set at least one of layer/enable/disable in
        // addition to the required match. (Since `layer` is now Option, a
        // match-only rule no longer fails deserialization — it must fail HERE.)
        if rule.layer.is_none() && rule.enable.is_empty() && rule.disable.is_empty() {
            return Err(
                "invalid rule: must set at least one of layer/enable/disable (in \
                 addition to match). See spec/HOST_RULES.md §9 Validity"
                    .into(),
            );
        }
    }
    Ok(())
}
```

- Update the single call site in `parse_rules`: `validate_layers(&rules)?` →
  `validate_rules(&rules)?`. Everything else in `parse_rules` is unchanged.

### (d) `contradictory_callback_names` — repoint at `rules.rules` (body logic unchanged)

```rust
pub fn contradictory_callback_names(rules: &RuleSet) -> Vec<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for rule in &rules.rules {                                   // <-- was &rules.callback_rules
        let dis: BTreeSet<&str> = rule.disable.iter().map(|s| s.as_str()).collect();
        for name in &rule.enable {
            if dis.contains(name.as_str()) {
                seen.insert(name.clone());
            }
        }
    }
    seen.into_iter().collect()
}
```

### (e) Doc-comments to update (exact locations)

1. **`RuleSet` doc-comment** (currently `rules.rs:33–68`): the ```toml block uses
   `[[layer_rules]]`/`[[callback_rules]]` + `layer = 224/225`. Rewrite to the four
   `[[rule]]` blocks with `layer = 10/11` (use the §9 example verbatim — see
   `spec/HOST_RULES.md:458–481`). Update the prose: drop "two table-arrays"; say
   "one `[[rule]]` array; `layer` is first-match-wins, `enable`/`disable` all-match".
2. **`LayerRule`/`CallbackRule` doc-comments** (currently `rules.rs:105–171`):
   DELETED with the structs. Their content is absorbed into the single `Rule`
   doc-comment.
3. **`validate_layers` → `validate_rules` doc-comment** (currently `rules.rs:235–252`):
   update prose to reference `[[rule]]` and add the new validity rule ("a rule must
   set at least one of layer/enable/disable").
4. **`HostContext` + `evaluate()` doc-comment** (currently `rules.rs:349–408`):
   - Change "Three-stage evaluation" / "stage-1/stage-2" to describe the **single
     pass** over `[[rule]]` (layer first-match-wins inline + enable/disable all-match
     inline), citing `spec/HOST_RULES.md` §8(3).
   - **FIX THE STALE LINE** (currently `rules.rs:393–394`): the no-match short-circuit
     doc currently says `clear_board: <[host].disable_firmware_config>`. Change it to
     `clear_board: false` (C13 — the code already returns `false`; only the comment
     is stale).
5. **`parse_rules` doc-comment** (currently `rules.rs:209`): references
   `[[layer_rules]]`/`[[callback_rules]]` → `[[rule]]`.
6. **`contradictory_callback_names` doc-comment** (currently `rules.rs:307`):
   references `[[callback_rules]]` → `[[rule]]`; its ```rust,ignore example uses
   `[[callback_rules]]` → `[[rule]]`.

### Success Criteria

- [ ] `LayerRule` and `CallbackRule` are **deleted**; `Rule` is the only rule struct.
- [ ] `RuleSet.rules: Vec<Rule>` with `#[serde(default, rename = "rule")]` (SINGULAR).
      `RuleSet.host`/derives unchanged. `HostDefaults` unchanged.
- [ ] `Rule` has exactly the 6 fields with the exact serde attrs from §9
      (`rename="match"`, `default` on layer/enable/disable/case_sensitive/disable_firmware_config).
- [ ] `evaluate()` is ONE pass over `rules.rules`; preserves C13 no-match, layer
      first-match-wins/exclusive, two-BTreeSet disable-order-independence,
      `all_disabling || !board_has_rules`.
- [ ] `validate_rules` (renamed) rejects `layer == Some(0xFF)` AND a match-only rule;
      `parse_rules` calls it.
- [ ] `contradictory_callback_names` iterates `rules.rules`.
- [ ] `HostContext`, `effective_disable_firmware_config`, `pattern_is_empty_core`,
      `get_rules_paths`, the `pattern.rs` import — all unchanged.
- [ ] The stale no-match doc-comment line is fixed to `clear_board: false`.
- [ ] Inline doc TOML uses `[[rule]]` + `layer = 10/11`.
- [ ] **The inline `mod tests` block is NOT modified** (S2 owns it). **No file other
      than `src/core/rules.rs` is modified.**
- [ ] `cargo build` reports **zero errors originating in the non-test portion of
      `src/core/rules.rs`** (all errors are external callers — S3's scope).

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed
> to implement this successfully?"_ — **Yes.** The verbatim target structs, the
> full one-pass `evaluate()` body, the renamed/extended `validate_rules`, the
> repointed `contradictory_callback_names`, the exact doc-comment locations, the 4
> invariants to preserve (with the test names that guard them), the expected
> non-compilation model, and the precise grep-based validation are all below.

### Documentation & References

```yaml
# MUST READ — the canonical spec (source of truth, ALREADY at target wording)
- file: /home/dustin/projects/qmkonnect/spec/HOST_RULES.md
  why: "§9 (lines ~458-518) is the verbatim target Rust model + TOML schema + Validity.
        §8(3) (lines ~368-376) is the one-pass evaluation semantics. §4 is stack-vs-replace.
        This file is AUTHORITATIVE; copy the struct bodies from here."
  section: "9. rules.toml Schema Reference" and "§8(3) Per-window evaluation"
  critical: "The serde rename is `rename = \"rule\"` (SINGULAR) → TOML key is [[rule]], NOT [[rules]].
             `layer` is Option<u8> (None => sets no layer), NOT a required u8. Validity: a rule
             must set >=1 of layer/enable/disable. layer==255 rejected."

# MUST READ — the architecture research (validated blast radius + invariants)
- file: /home/dustin/projects/qmkonnect/plan/004_f48a103bcb32/architecture/system_context.md
  why: "§1a maps every item to its current line. §1b lists behaviors ALREADY CORRECT that MUST be
        preserved (C13, C11, disable-order-independence). §4 lists the key risks. §5 is the
        compile-ordering model (why S1 alone doesn't compile)."
  section: "1. Validated current state" and "4. Key risks & invariants"
  critical: "C13 no-match returns clear_board:false (NOT the global default) — the doc-comment is
             stale but the CODE is correct; this subtask fixes only the comment. Disable-order-
             independence is guarded by test #37 test_evaluate_disable_before_enable_still_excludes
             (S2 will re-port it; the BEHAVIOR must survive S1's rewrite)."

# MUST READ — the file being edited (read current state before editing)
- file: /home/dustin/projects/qmkonnect/src/core/rules.rs
  why: "Contains everything S1 edits: RuleSet (70-83), LayerRule (126), CallbackRule (165),
        effective_disable_firmware_config (230), parse_rules (237), validate_layers (253),
        get_rules_paths (272), contradictory_callback_names (314), pattern_is_empty_core (325),
        HostContext (355), evaluate (410-493), the doc-comments (33-159,209,235,307,346,349-408).
        The #[cfg(test)] mod tests block (494-1378) is S2's scope — DO NOT EDIT IT."
  pattern: "Existing enum/struct style: /// doc + #[derive(Debug, Deserialize)] + #[serde(...)].
            The unified Rule/RuleSet follow the same style (Pattern imported as-is)."
  gotcha: "Deleting LayerRule/CallbackRule breaks the inline mod tests (they reference both) — that
           is EXPECTED and is S2's scope. Do NOT try to 'keep them compiling' by leaving the old
           structs; delete them. Likewise external callers (notifier.rs:572, main.rs:253, mod.rs)
           will break — that is S3's scope."

# REFERENCE — the dependency this module imports unchanged
- file: /home/dustin/projects/qmkonnect/src/core/pattern.rs
  why: "Defines Pattern (Single/Parts, #[serde(untagged)]) and match_pattern(pattern, app_class,
        title, case_sensitive) -> bool. evaluate() calls match_pattern; Rule.pattern is Pattern.
        pattern.rs is UNCHANGED by S1 (its doc-comment prose is S3's scope)."
  section: "pub fn match_pattern (line ~1170)" and "pub enum Pattern (line ~1118)"
  critical: "match_pattern signature is (pattern, app_class, title, case_sensitive) — the one-pass
             evaluate() calls it identically. Do not change the call."

# REFERENCE — research notes for this subtask (test-suite invariant analysis for S2)
- docfile: plan/004_f48a103bcb32/P1M1T1.S1/research/notes.md
  why: "Classifies all ~46 existing tests by which invariant they guard and whether they reference
        the old split schema (so S2 knows what to port). Documents the validity-mechanism change
        (layer u8→Option means match-only rules move from serde-error to validate_rules-error)."
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/                                  # THIS repo
├── src/core/
│   ├── rules.rs        # <-- FILE TO EDIT (non-test portion only). 1378 lines total.
│   ├── pattern.rs      # Pattern + match_pattern (IMPORTED UNCHANGED). doc-comment prose = S3.
│   ├── notifier.rs     # caller: unknown_callback_names @572 iterates rules.callback_rules (S3).
│   ├── mod.rs          # caller: render_rules_body template @191-233 (S3).
│   └── types.rs        # WindowInfo (unchanged, not touched).
├── src/main.rs         # callers: collect_callback_names, empty_pattern_warnings, validate_rules (S3).
├── spec/HOST_RULES.md  # §8(3) + §9 — SOURCE OF TRUTH (already at target wording, DO NOT edit).
└── plan/004_f48a103bcb32/architecture/system_context.md   # validated blast radius + invariants
```

### Desired Codebase tree with files to be modified

```bash
src/core/
└── rules.rs   # MODIFIED (non-test code + doc-comments ONLY). mod tests block left for S2.
```

> No new files. `pattern.rs`, `notifier.rs`, `main.rs`, `core/mod.rs` are NOT
> edited in this subtask (S3). `spec/HOST_RULES.md` is NOT edited (already correct).

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL: the crate WILL NOT COMPILE after this subtask alone.
//   Deleting LayerRule/CallbackRule breaks: (a) the inline #[cfg(test)] mod tests in rules.rs
//   (references .layer_rules/.callback_rules/LayerRule/CallbackRule/SECTION_9_TOML) — that is
//   S2's scope; (b) external callers notifier.rs/main.rs/core/mod.rs — that is S3's scope.
//   The S1 gate is NOT "cargo build green". It is "zero errors ORIGINATING in the NON-TEST
//   portion of rules.rs" (see Validation Loop). Do NOT modify tests or external files to force
//   a green build — that would steal S2/S3's work and break the task boundary.

// CRITICAL: [[rule]] is SINGULAR (serde rename = "rule"), NOT [[rules]].
//   Every TOML literal and doc-comment must use [[rule]]. A plural rename would be a wire/schema
//   break against spec §9. Double-check the attribute: #[serde(default, rename = "rule")].

// CRITICAL: layer is now Option<u8> (default None), NOT a required u8.
//   This changes the validity mechanism: a [[rule]] with only `match` no longer fails
//   *deserialization* (layer defaults to None). It must fail the NEW check in validate_rules
//   ("must set at least one of layer/enable/disable"). The parse_rules -> is_err() boundary
//   is preserved, but the mechanism moves from serde to validate_rules. Existing tests #7
//   (test_rules_missing_layer_errors, asserts serde error via toml::from_str directly) and
//   #17 (test_rules_parse_missing_required_field_errors, asserts parse_rules is_err) will need
//   S2 attention: #7's direct toml::from_str will NO LONGER error (layer is optional) — S2
//   rewrites it to use parse_rules + the validity error. #17's is_err() still holds via
//   validate_rules. S1 implements validate_rules so the boundary is intact for S2.

// CRITICAL: preserve the C13 no-match short-circuit VERBATIM.
//   `if matched_effective.is_empty() return HostContext { layer:None, callback_ids:vec![],
//   clear_board:false, any_match:false }`. Do NOT change clear_board to the global default —
//   all() is vacuously true on an empty Vec and would wrongly yield true. This is the
//   highest-stakes line in the rewrite.

// CRITICAL: disable-order-independence MUST survive the collapse.
//   Keep the TWO BTreeSets (enabled + disabled) and the single `enabled.difference(&disabled)`
//   AFTER the loop. Do NOT fold disable into a single set with insert-then-remove — that
//   regresses to last-writer-wins (the bug fixed by the current two-pass difference). The
//   regression test test_evaluate_disable_before_enable_still_excludes guards this; S2 ports it.

// NOTE: matched_effective collects ONE flag per matched RULE, not per matched field.
//   This is correct: spec §8(3) says "replace iff every matched RULE's effective flag is true".
//   A single [[rule]] setting both layer+enable pushes one flag — same all() result as its
//   old-schema equivalent (a layer_rule + callback_rule with identical flags pushing two).

// NOTE: layer first-match-wins is now `if rule.layer.is_some() && layer.is_none() { layer = rule.layer }`
//   INSIDE the single loop — NOT a separate early-break scan. Only layer-SETTING rules compete
//   for first-match; callback-only rules (layer==None) are skipped for the layer decision but
//   still contribute to enable/disable. There is NO `break` in the new loop (all matches must
//   accumulate).

// NOTE: HostDefaults, HostContext, effective_disable_firmware_config, pattern_is_empty_core,
//   get_rules_paths, and the `use crate::core::pattern::{match_pattern, Pattern};` import are
//   UNCHANGED. The #![allow(dead_code)] at the top stays (pub items don't trip it; removing it
//   is out of scope and would only risk new warnings).

// NOTE: tests are single-threaded (cargo test --bin qmkonnect -- --test-threads=1) per AGENTS.md
//   (shared global debouncer). S2 will restore the green test run; S1 does not run tests.
```

## Implementation Blueprint

### Data models and structure

The "data model" is exactly the three structs in the **What (a)** section (`RuleSet`
with one `rules` Vec, unchanged `HostDefaults`, new `Rule`). `HostContext` is
unchanged. No new traits, no constructors — serde derives + the existing manual
`Default` for `HostDefaults` cover it.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: READ current state + spec (anchor the rewrite)
  - READ: src/core/rules.rs in full (especially RuleSet 70-83, LayerRule 126, CallbackRule 165,
          effective_disable_firmware_config 230, parse_rules 237, validate_layers 253,
          get_rules_paths 272, contradictory_callback_names 314, pattern_is_empty_core 325,
          HostContext 355, evaluate 410-493, doc-comments 33-159/209/235/307/346/349-408).
  - READ: spec/HOST_RULES.md §8(3) (~368-376) + §9 (~458-518) — copy struct bodies verbatim.
  - READ: plan/004_f48a103bcb32/architecture/system_context.md §1 + §4 (invariants + risks).
  - CONFIRM: match_pattern signature in pattern.rs (~1170) is (pattern, app_class, title, case_sensitive).
  - GOAL: know exactly what changes, what stays, and the 4 invariants to preserve.

Task 2: REPLACE the structs (RuleSet + delete LayerRule/CallbackRule + add Rule)
  - EDIT: src/core/rules.rs — replace the RuleSet body (two Vecs → one `rules: Vec<Rule>` with
          #[serde(default, rename = "rule")]). Keep host + derives + the (updated) doc-comment.
  - DELETE: the LayerRule struct + its doc-comment; the CallbackRule struct + its doc-comment.
  - ADD: the Rule struct (verbatim from What (a)) with its doc-comment, placed where LayerRule was.
  - KEEP: HostDefaults byte-for-byte (body + manual Default impl + doc-comment).
  - DO NOT: touch HostContext, effective_disable_firmware_config, pattern_is_empty_core,
          get_rules_paths, the pattern import, or the #![allow(dead_code)].

Task 3: RENAME + EXTEND validate_layers → validate_rules
  - RENAME: fn validate_layers -> fn validate_rules. Iterate rules.rules.
  - KEEP: the 0xFF rejection (change `rule.layer == 0xFF` → `rule.layer == Some(0xFF)`; update
          the error prose to say `[[rule]]`).
  - ADD: the "must set at least one of layer/enable/disable" check (What (c)).
  - UPDATE: parse_rules' call site (validate_layers → validate_rules). Update parse_rules'
          doc-comment to reference [[rule]].
  - UPDATE: the validate_rules doc-comment (reference [[rule]] + the new validity rule).

Task 4: REWRITE evaluate() to ONE pass
  - REPLACE: the two-scan body (layer loop + callback loop) with the single-pass body (What (b)).
  - PRESERVE: the no-match short-circuit VERBATIM (C13); the two-BTreeSet difference (order-
          independence); the `all_disabling || !board_has_rules` formula; BTreeSet-sorted output.
  - KEEP: the evaluate() signature + HostContext unchanged.
  - UPDATE: the evaluate()/HostContext doc-comment (What (e).4) — single pass; FIX the stale
          no-match line @393-394 to clear_board:false.

Task 5: REPOINT contradictory_callback_names
  - EDIT: iterate rules.rules (was rules.callback_rules). Body logic unchanged.
  - UPDATE: its doc-comment + the ```rust,ignore example to use [[rule]].

Task 6: UPDATE remaining doc-comment TOML examples
  - RuleSet doc-comment ```toml block (33-68): [[layer_rules]]/[[callback_rules]] → four [[rule]];
          layer 224/225 → 10/11 (use the §9 example verbatim).
  - parse_rules doc-comment (209): [[layer_rules]]/[[callback_rules]] → [[rule]].
  - Any other inline TOML in the non-test portion referencing the old keys → [[rule]].

Task 7: VALIDATE (the gate is NOT cargo build green — see Validation Loop)
  - RUN: cargo build 2>&1 | tee /tmp/build.log
  - CONFIRM: ZERO errors whose `-->` points at the NON-TEST portion of src/core/rules.rs
          (line < ~494). All errors must be in external callers (notifier.rs/main.rs/mod.rs/
          pattern.rs) — the bounded S3 set.
  - RUN: cargo test --no-run 2>&1 | tee /tmp/test-build.log  (optional, for completeness)
  - CONFIRM: additional errors are ONLY in rules.rs #[cfg(test)] mod tests (S2) + external test
          code (S3). Still ZERO errors in rules.rs non-test code.
  - SELF-REVIEW: re-read the one-pass evaluate() against the 4 invariants (C13, first-match-wins
          exclusive, two-BTreeSet difference, stack-vs-replace formula).
```

### Implementation Patterns & Key Details

```rust
// === THE ONE-PASS EVALUATOR — the heart of this subtask ===
// (Full body in What (b). The 4 invariants it must preserve:)
//   1. C13 no-match: matched_effective.is_empty() => clear_board:false (NOT global default).
//   2. Layer first-match-wins EXCLUSIVE: `if rule.layer.is_some() && layer.is_none()` — no break.
//   3. Disable-order-independence: two BTreeSets, difference ONCE after the loop.
//   4. Stack-vs-replace: clear_board = matched_effective.iter().all(|&f| f) || !board_has_rules.

// === WHY one flag per matched rule (not per field) is correct ===
//   Old: a window matching a layer_rule AND a callback_rule pushed 2 flags.
//   New: a single [[rule]] (layer+enable) pushes 1 flag. all() yields the same result because
//   the old equivalent (layer_rule + callback_rule with the SAME effective flag) pushed 2 equal
//   flags. Spec §8(3): "replace iff every matched RULE's effective flag is true" (per-rule).

// === THE VALIDITY CHECK — mechanism change, same boundary ===
//   layer is now Option<u8> => a match-only [[rule]] no longer fails toml::from_str. It fails
//   validate_rules instead. parse_rules() still returns Err => the is_err() boundary that
//   --validate-rules (P5.M1) and the existing parse tests rely on is preserved.
```

### Integration Points

```yaml
SOURCE FILES:
  - modify: "src/core/rules.rs ONLY (non-test code + doc-comments)"

PUBLIC API SURFACE (this subtask):
  - removes:  "LayerRule, CallbackRule, RuleSet.layer_rules, RuleSet.callback_rules,
               validate_layers (renamed validate_rules)"
  - adds:     "Rule, RuleSet.rules, validate_rules (renamed+extended)"
  - unchanged: "RuleSet (name+derives+host), HostDefaults, HostContext, effective_disable_*
                pattern_is_empty_core, get_rules_paths, parse_rules (signature), evaluate (signature),
                contradictory_callback_names (signature)"

EXPECTED BREAKAGE (NOT fixed here — listed so the agent does NOT panic):
  - src/core/rules.rs #[cfg(test)] mod tests (494-1378): references .layer_rules/.callback_rules/
        LayerRule/CallbackRule/SECTION_9_TOML. => P1.M1.T1.S2 rewrites the suite.
  - src/core/notifier.rs:572 unknown_callback_names: `for rule in &rules.callback_rules`. => S3.
  - src/core/notifier.rs:519 doc-comment [[callback_rules]]; :1908-1921 test TOML. => S3.
  - src/main.rs:253 collect_callback_names; :271/:281 empty_pattern_warnings (two loops);
        :442-443 validate_rules summary; :606/:612 test seeding; :574/579/630/633/646/651/655 test TOML. => S3.
  - src/core/mod.rs:191-233 render_rules_body template (two # [[layer_rules]] + two # [[callback_rules]]);
        :183 doc-example; :377-378/:390-391 test asserts. => S3.
  - src/core/pattern.rs:1090 doc-comment prose ([layer_rules]/[callback_rules]). => S3.

DEPENDENCIES / Cargo.toml:
  - none. serde/toml already present. No new crate deps.

VALIDATION CONSUMERS (S2/S3 — do NOT implement now):
  - P1.M1.T1.S2: "Rewrite the rules.rs test suite to [[rule]] (port the ~46 tests; SECTION_9_TOML
                   fixture -> 10/11; re-assert the 4 invariants)."
  - P1.M1.T1.S3: "Update all external callers (notifier.rs, main.rs, mod.rs, pattern.rs) + their tests."
```

## Validation Loop

> All commands run from the repo root: `/home/dustin/projects/qmkonnect`.
> **⚠ THE PRIMARY GATE IS NOT "cargo build is green".** This subtask intentionally
> leaves the crate non-compiling (inline tests = S2; external callers = S3). The
> gate is: **rules.rs non-test code is internally type-correct AND logically
> preserves the 4 invariants.**

### Level 1: Type-correctness of the NON-TEST rules.rs (the real gate)

```bash
cd /home/dustin/projects/qmkonnect

# `cargo build` does NOT compile #[cfg(test)] modules, so it surfaces ONLY non-test errors:
# the unified model + external callers. Confirm ZERO errors originate in rules.rs non-test code.
cargo build 2>&1 | tee /tmp/build.log

# GATE A — no error's authoritative `-->` pointer targets the NON-TEST portion of rules.rs
# (anything before the `#[cfg(test)] mod tests` line, ~line 494). Notes/helps use `:` not `-->`.
grep -nE '\-\-> src/core/rules\.rs:[0-9]+' /tmp/build.log | awk -F: '{print $0}' | while read ln; do
  num=$(echo "$ln" | grep -oE 'rules\.rs:[0-9]+' | grep -oE '[0-9]+');
  if [ "$num" -lt 494 ]; then echo "VIOLATION (non-test rules.rs error): $ln"; fi
done
# Expected: NO "VIOLATION" lines printed. If any appear, the unified model itself has a type
# error — fix it before proceeding.

# GATE B — every error IS in the bounded external-caller set (sanity: nothing unexpected broke)
grep -E 'error\[E[0-9]+\]' /tmp/build.log
# Expected: all errors are field-not-found / type-mismatch in:
#   src/core/notifier.rs, src/main.rs, src/core/mod.rs, src/core/pattern.rs
# (and NONE in src/core/rules.rs non-test code). The build exits non-zero — that is EXPECTED.
```

### Level 2: Test-module breakage is confined (optional completeness check)

```bash
cd /home/dustin/projects/qmkonnect

# `cargo test --no-run` additionally compiles the test modules. Confirm the ONLY rules.rs errors
# are INSIDE #[cfg(test)] mod tests (S2's scope) — still none in non-test code.
cargo test --no-run 2>&1 | tee /tmp/test-build.log

# Re-run GATE A's pointer check against the test build; every rules.rs `-->` must be >= ~494
# (inside mod tests). Non-test rules.rs must still produce ZERO errors.
grep -nE '\-\-> src/core/rules\.rs:[0-9]+' /tmp/test-build.log
# Expected: all rules.rs line numbers are >= ~494 (the test module). None below it.
```

```text
NOTE: you CANNOT run `cargo test` to green here — the test suite references the deleted structs
(LayerRule/CallbackRule/.layer_rules/.callback_rules/SECTION_9_TOML) and will not compile until
S2 rewrites it. That is by design. Do NOT modify the test module to force a green run.
```

### Level 3: Logical self-review of the 4 invariants (the behavior gate)

```text
Re-read the one-pass evaluate() body and confirm each invariant holds BY CONSTRUCTION:

1. C13 no-match: `if matched_effective.is_empty() { return HostContext { … clear_board:false … } }`
   appears BEFORE the `all_disabling || !board_has_rules` formula. clear_board is literally `false`
   (not the global default).  ☐ confirmed

2. Layer first-match-wins EXCLUSIVE: `if rule.layer.is_some() && layer.is_none() { layer = rule.layer }`
   inside the single loop. There is NO `break` (callback accumulation must continue). Only layer-
   setting rules compete; layer==None rules are skipped for this decision.  ☐ confirmed

3. Disable-order-independence: TWO BTreeSets (`enabled`, `disabled`), accumulated across the loop,
   then `desired = enabled.difference(&disabled).copied().collect()` AFTER the loop. Disable is
   removed from the union regardless of rule order.  ☐ confirmed

4. Stack-vs-replace: `let all_disabling = matched_effective.iter().all(|&f| f);
   let clear_board = all_disabling || !board_has_rules;` — verbatim formula, AFTER the no-match
   short-circuit. matched_effective has one entry per matched rule.  ☐ confirmed

5. Output shape: callback_ids is `desired.into_iter().collect()` (BTreeSet → sorted Vec<u8>);
   HostContext fields unchanged.  ☐ confirmed
```

### Level 4: Spec parity (cosmetic + schema correctness)

```bash
cd /home/dustin/projects/qmkonnect

# Confirm the unified struct attrs match spec §9 verbatim (field names, serde renames, defaults).
diff <(sed -n '/pub struct RuleSet/,/^}/p' src/core/rules.rs) \
     <(sed -n '/pub struct RuleSet/,/^}/p' spec/HOST_RULES.md) && echo "RuleSet matches spec §9" \
  || echo "RuleSet differs from spec §9 — reconcile (spec is authoritative)"

diff <(sed -n '/pub struct Rule /,/^}/p' src/core/rules.rs) \
     <(sed -n '/pub struct Rule /,/^}/p' spec/HOST_RULES.md) && echo "Rule matches spec §9" \
  || echo "Rule differs from spec §9 — reconcile (spec is authoritative)"
# (The diff may show trivial formatting differences; the FIELD NAMES, TYPES, and serde ATTRS
#  must match exactly. rename="match", rename="rule" (singular), default on the 5 optional fields.)

# Confirm no stale split-schema TOML keys leaked into the non-test doc-comments.
grep -nE '\[\[(layer_rules|callback_rules)\]\]|layer = 22[45]' src/core/rules.rs | \
  awk -F: '$2 < 494 {print "STALE (non-test): "$0}'
# Expected: no STALE lines (the test module's SECTION_9_TOML at >=494 is S2's to fix).
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1 GATE A: zero `-->` errors in non-test `src/core/rules.rs` (line < ~494).
- [ ] Level 1 GATE B: all `cargo build` errors are in the external-caller set (S3 scope).
- [ ] Level 2: `cargo test --no-run` rules.rs errors are all inside `mod tests` (>= ~494).
- [ ] Level 3: all 4 invariants confirmed by construction (C13, first-match-wins, two-BTreeSet
      difference, stack-vs-replace formula).
- [ ] Level 4: `Rule`/`RuleSet` attrs match `spec/HOST_RULES.md` §9 (rename singular, defaults).

### Feature Validation

- [ ] `LayerRule` + `CallbackRule` deleted; single `Rule` struct with the 6 §9 fields.
- [ ] `RuleSet.rules: Vec<Rule>` with `#[serde(default, rename = "rule")]` (SINGULAR).
- [ ] `evaluate()` is one pass; `HostContext` output byte-identical for old-schema inputs.
- [ ] `validate_rules` rejects `layer == Some(0xFF)` AND match-only rules; `parse_rules` calls it.
- [ ] `contradictory_callback_names` iterates `rules.rules`.
- [ ] Stale no-match doc-comment line fixed to `clear_board: false`.
- [ ] Inline doc TOML uses `[[rule]]` + `layer = 10/11`.

### Code Quality Validation

- [ ] `HostDefaults`, `HostContext`, `effective_disable_firmware_config`,
      `pattern_is_empty_core`, `get_rules_paths`, the `pattern` import — unchanged.
- [ ] Doc-comments follow the existing `///` + ```toml style; reference spec §8(3)/§9.
- [ ] The `#[cfg(test)] mod tests` block is byte-for-byte untouched (S2's scope).
- [ ] No file other than `src/core/rules.rs` modified (external callers = S3).

### Documentation & Deployment

- [ ] Code doc-comments (the "Three-stage" → single-pass rewrite, struct examples, stale no-match
      line) updated as part of this work (Mode A). `spec/HOST_RULES.md` is NOT edited (already correct).
- [ ] User-facing `docs/*.md` are NOT touched here (P1.M1.T2).
- [ ] No environment variables, config, or Cargo.toml changes.

---

## Anti-Patterns to Avoid

- ❌ Don't use `cargo build` success as the gate — it CANNOT be green until S3 lands. The gate is
  "zero errors originating in non-test rules.rs." Chasing a green build by editing tests/external
  files steals S2/S3's work and breaks the task boundary.
- ❌ Don't leave `LayerRule`/`CallbackRule` in place "to keep it compiling" — delete them. The
  unification is the whole point; partial deletion is incoherent.
- ❌ Don't pluralize the rename (`rename = "rules"`) — it is SINGULAR (`"rule"`), so the TOML key is
  `[[rule]]`. Spec §9 is authoritative.
- ❌ Don't make `layer` required — it is `Option<u8>` (default `None`). The validity check moves to
  `validate_rules`, not serde.
- ❌ Don't drop the C13 no-match short-circuit or weaken it to the global default — `all()` on an
  empty Vec is vacuously `true`; the explicit `clear_board: false` return is mandatory.
- ❌ Don't fold disable into a single insert-then-remove set — that regresses disable to
  last-writer-wins. Keep the two-BTreeSet difference (test #37 guards it).
- ❌ Don't `break` out of the single loop after the layer is set — callback accumulation must
  continue across ALL matching rules (all-match). First-match-wins applies to LAYER only.
- ❌ Don't collect one effective flag per matched FIELD — collect one per matched RULE (spec §8(3)).
- ❌ Don't touch the inline `mod tests` (lines ~494–1378) — that is P1.M1.T1.S2. Leave it referencing
  the old structs; the expected compile failure there is S2's entry point.
- ❌ Don't edit `notifier.rs`, `main.rs`, `core/mod.rs`, or `pattern.rs` — external callers are
  P1.M1.T1.S3.
- ❌ Don't edit `spec/HOST_RULES.md` — it is already at the target `[[rule]]` wording (source of
  truth). Copy FROM it, don't change it.
- ❌ Don't change `HostContext`, `effective_disable_firmware_config`, `pattern_is_empty_core`, or
  `get_rules_paths` — they are out of scope and behavior-preserving depends on them being untouched.
- ❌ Don't change the `224/225` literals inside the RANGE-acceptance test
  (`test_parse_rules_accepts_low_layer_indices`) — that's a test (S2) and it legitimately probes the
  0..=254 range. S1 only changes the non-test `SECTION_9_TOML` doc examples → 10/11.

---

**Confidence Score: 9/10** for one-pass implementation success. The target structs are quoted
verbatim from the authoritative spec §9; the one-pass `evaluate()` body is given in full with the
4 preserved invariants called out by name; the validity-mechanism change (layer u8→Option) and its
effect on the is_err() boundary are explained; the non-compilation reality is confronted head-on
with a precise grep-based gate (`--> src/core/rules.rs:N` with N < ~494 must be empty) instead of a
misleading "build green". The one residual risk: a subtle behavior drift in the collapsed evaluator
that the (deferred) test suite would catch — mitigated by the Level-3 self-review checklist and the
explicit "one flag per matched rule" justification. Once S2 re-ports the ~46 parity tests against
the unified model, the byte-identical-output promise is mechanically verified.
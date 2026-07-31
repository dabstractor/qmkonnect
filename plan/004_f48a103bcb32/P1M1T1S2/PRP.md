# PRP — P1.M1.T1.S2: Rewrite the `src/core/rules.rs` test suite to the unified `[[rule]]` schema

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`).
> **Scope:** Rewrite ONLY the inline `#[cfg(test)] mod tests { … }` block in
> `src/core/rules.rs` (currently lines 494–1377, including the `SECTION_9_TOML`
> const at 499–521) so it compiles against and verifies S1's unified `[[rule]]`
> model. **This is test-only work internal to `rules.rs`** — no production code,
> no public API, no docs, no other file.
> **⚠ The crate will NOT compile end-to-end at S2 time** (external callers in
> `notifier.rs`/`main.rs`/`mod.rs`/`pattern.rs` are broken until S3 lands). The
> gate is NOT "`cargo test` green"; it is "the rules.rs test module is internally
> correct against the unified model." See the Validation Loop.

---

## Goal

**Feature Goal**: Port the existing `~46`-test `rules.rs` test suite from the
split `[[layer_rules]]`/`[[callback_rules]]` schema to S1's unified `[[rule]]`
schema, **preserving every observable `evaluate()`/`HostContext` assertion and
every validity `is_err()` boundary byte-for-byte**, while (a) updating the
`SECTION_9_TOML` fixture to the verbatim `spec/HOST_RULES.md` §9 example
(`layer = 10/11`), (b) reframing the two match-only-rule tests (#7/#17) from a
serde-missing-field error to the new `validate_rules` validity error (same
`is_err()` boundary, different mechanism), and (c) adding ONE new positive test
for the headline unified-model capability — a single `[[rule]]` carrying BOTH a
`layer` AND `enable`/`disable`.

**Deliverable**: A rewritten `#[cfg(test)] mod tests { … }` block (and the
`SECTION_9_TOML` const inside it) in `src/core/rules.rs`. ~36 of the 46 existing
tests are modified; 10 schema-agnostic tests are byte-untouched; 1 new test is
added. No file other than `src/core/rules.rs` is touched, and within it ONLY the
test module (non-test code is S1's scope).

**Success Definition**:
- **GATE A:** `cargo test --no-run` produces ZERO compile errors whose `-->`
  points into the `rules.rs` test module (line >= ~495). All remaining errors are
  in the external-caller files (S3's bounded scope).
- **GATE B (post-S3):** once S3 lands, `cargo test --bin qmkonnect -- --test-threads=1`
  passes with **all** tests green (the 36 rewritten + 10 untouched + 1 new).
- **GATE C (logical):** every preserved assertion (the `HostContext` field values
  and the validity `msg.contains(...)` checks listed per-test in
  `research/notes.md` §2) is unchanged from the pre-rewrite suite.

## User Persona (if applicable)

**Target User**: The host-rules pipeline maintainers (future P4.M3 send-logic,
P5 `--validate-rules` CLI) who rely on this suite as the **parity contract**
guarding `evaluate()`'s behavior, and the S3 implementer who will fix external
callers against the same unified model.

**Use Case**: A maintainer changes `evaluate()` or `validate_rules`; the suite
must (a) compile against the unified `Rule`/`RuleSet`, and (b) fail loudly if any
of the 4 invariants (C13 no-match, layer first-match-wins/exclusive,
disable-order-independence, stack-vs-replace) regresses.

**Pain Points Addressed**: S1 deleted `LayerRule`/`CallbackRule` and collapsed
the two arrays into one — the suite currently references all of those and won't
compile. This rewrite restores a green, behavior-preserving suite that also
documents the new validity rule and the new "one rule, layer+callbacks" capability.

## Why

- **Closes the S1 compile gap for the test module.** S1 deliberately left
  `mod tests` untouched (it's this subtask's scope). Until the suite is ported,
  `cargo test` can't run.
- **Preserves the parity contract.** The unification is *behavior-preserving for
  old-schema inputs* (S1's promise). This suite is the mechanical proof: every
  `evaluate()` test keeps its exact `HostContext` output; only the embedded TOML
  literals and the field-access paths change.
- **Documents the mechanism change.** `layer` is now `Option<u8>`, so a
  match-only rule no longer fails *deserialization* — it fails *validity*. The
  reframed #7/#17 + the optional renamed test make that boundary explicit for
  the `--validate-rules` CLI (P5.M1).
- **Unblocks S3 and downstream.** S3 fixes external callers against the same
  model; a correct rules.rs suite is the reference those callers' tests align to.

## What

A set of mechanical, behavior-preserving edits to the test module, plus one
addition and two reframes. The full per-test plan is in
`research/notes.md` §2 (authoritative inventory built by reading all 46 test
bodies). Summary of the edit classes:

### (a) `SECTION_9_TOML` (rules.rs:499–521) → verbatim spec §9 (458-481)

```toml
const SECTION_9_TOML: &str = r#"[[rule]]
match = "alacritty"
layer = 10
disable_firmware_config = true

[[rule]]
match = ["*chrome*", "*youtube*"]
layer = 11
case_sensitive = false

[[rule]]
match = "neovide"
enable = ["vim_lazy", "disable_vim"]
disable = ["vim_lazy"]

[[rule]]
match = ["*chrome*", "*claude*"]
enable = ["vim_lazy", "disable_vim"]
disable_firmware_config = true
"#;
```

> The spec §9 example has NO `[host]` table; the `host` field is `#[serde(default)]`
> so `rs.host.disable_firmware_config` defaults to `false` and test #1's
> `assert!(!(rs.host.disable_firmware_config))` still passes. (If you prefer to
> keep the explicit `[host]\ndisable_firmware_config = false\n\n` header, that's
> also fine — both pass; the item says "VERBATIM from spec §9", so prefer no header.)

### (b) Test #1 `test_rules_full_section9_example_parses` (524) — the fixture consumer

- `assert_eq!(rs.rules.len(), 4);` (was `layer_rules.len()==2` + `callback_rules.len()==2`).
- `rs.rules[0]`: `Pattern::Single("alacritty")`, `layer == Some(10)`, `!case_sensitive`, `disable_firmware_config == Some(true)`.
- `rs.rules[1]`: `Pattern::Parts("*chrome*","*youtube*")`, `layer == Some(11)`, `!case_sensitive`, `disable_firmware_config == None`.
- `rs.rules[2]`: `Pattern::Single("neovide")`, `enable == ["vim_lazy","disable_vim"]`, `disable == ["vim_lazy"]`, `!case_sensitive`, `disable_firmware_config == None`.
- `rs.rules[3]`: `Pattern::Parts("*chrome*","*claude*")`, `enable == ["vim_lazy","disable_vim"]`, `disable == Vec::<String>::new()`, `disable_firmware_config == Some(true)`.

### (c) Test #14 `test_rules_parse_valid_section9` (732)

- `rs.rules.len() == 4`; `rs.rules[0].layer == Some(10)`; `rs.rules[0].disable_firmware_config == Some(true)`.
- Drop the `layer_rules.len()`/`callback_rules.len()` split assertions.

### (d) The mechanical recipe for ~28 "CHANGE" tests (#2-6, #8-9, #21-33, #36, #38-44)

For each test whose TOML literal uses `[[layer_rules]]`/`[[callback_rules]]`:
1. Swap the table-array key → `[[rule]]` (SINGULAR — serde `rename = "rule"`).
2. If the resulting `[[rule]]` would have ONLY `match` (no layer/enable/disable),
   add a minimal field so it passes `validate_rules` (tests #4/#6 already keep a
   `layer`; tests #5/#26-29/#42-44 already have enable/disable).
3. Change field access `rs.layer_rules[i]`/`rs.callback_rules[i]` → `rs.rules[i]`
   (recompute index in file order; only tests #1/#14/#2/#4/#5/#6/#9 do direct field access).
4. Change layer-value assertions from bare `u8` (`== 224`) → `Option<u8>` (`== Some(224)`).
   **Preserve the numeric value** (e.g. test #4's `230`, #23's `224`, #24's `230`,
   #30/#31/#32/#33's `224`, #40's range `[0,28,100,224,254]`, #41's `5`+`255`).
5. **PRESERVE every `evaluate()` output assertion byte-for-byte** — `layer`
   (now `Option<u8>`), `callback_ids` (Vec<u8>), `clear_board` (bool), `any_match` (bool).

### (e) Tests #7 + #17 — REFRAME (validity-mechanism change)

`layer` is now `Option<u8>`, so `[[rule]] match = "x"` (no layer/enable/disable)
deserializes OK and must fail the NEW `validate_rules` check instead of serde.

- **#7 `test_rules_missing_layer_errors` (668):** currently calls
  `toml::from_str::<RuleSet>` directly and asserts `is_err()`. Under the unified
  schema `toml::from_str` SUCCEEDS. **Rewrite** to go through `parse_rules` (write
  the TOML to a temp file) and assert `parse_rules(&path).is_err()`, with the
  message asserting a validity phrase. **Optionally rename** to
  `test_parse_rules_rejects_rule_with_only_match` (the item sanctions this merge).
  TOML: `[[rule]]\nmatch = "x"\n`.
- **#17 `test_rules_parse_missing_required_field_errors` (771):** already goes
  through `parse_rules` (temp file). Just change the TOML literal to `[[rule]]
  match = "x"`; the `is_err()` holds via `validate_rules` (mechanism note in a
  comment). Boundary identical.

### (f) Test #8 `test_rules_missing_match_errors` (679) — CHANGE TOML only

`match` is a REQUIRED field (`rename = "match"`, NO `default`), so `[[rule]]`
with `layer = 224` but no `match` STILL fails serde. Change the TOML literal
`[[layer_rules]]` → `[[rule]]`; keep `is_err()`.

### (g) The 10 schema-agnostic tests — KEEP byte-untouched

#10 `test_rules_effective_some_true_wins`, #11 `…_some_false_wins`,
#12 `…_none_inherits_false`, #13 `…_none_inherits_true` (pure `effective_*` fn),
#15 `test_rules_parse_missing_file_errors`, #16 `…_malformed_toml_errors`,
#18 `test_rules_paths_swap_filename`, #19 `test_rules_paths_delegate_count`,
#45 `test_pattern_is_empty_core_single`, #46 `…_parts`. None reference the split
schema — leave them exactly as-is.

### (h) Test #20 `test_evaluate_empty_ruleset_no_match` (841) — COMPILE check only

`RuleSet::default()` now has `rules: Vec::default()`; the assertion
`ctx == HostContext { layer:None, callback_ids:vec![], clear_board:false, any_match:false }`
is unchanged. No edit needed beyond confirming it type-checks.

### (i) NEW positive test — the unified-model headline capability

Add one test (among the evaluate() parity tests, e.g. after #33) proving a SINGLE
`[[rule]]` can carry BOTH `layer` AND `enable`/`disable` (impossible under the
split schema). Reuses the existing `name_map` helper:

```rust
#[test]
fn test_evaluate_single_rule_layer_plus_callbacks() {
    // The unified model's key capability: ONE [[rule]] sets BOTH a layer and
    // callbacks (impossible under the old split schema). layer contributes to
    // first-match; callbacks accumulate; clear_board follows the single
    // effective flag for the matched rule.
    let toml = r#"
[[rule]]
match = "kitty"
layer = 9
enable = ["vim_lazy", "disable_vim"]
disable = ["vim_lazy"]
disable_firmware_config = true
"#;
    let rules: RuleSet = toml::from_str(toml).unwrap();
    let n2i = name_map(&[("vim_lazy", 1), ("disable_vim", 2)]);
    let ctx = evaluate(&rules, "kitty", "anything", &n2i, true);
    assert_eq!(ctx.layer, Some(9));        // layer from the same rule
    assert_eq!(ctx.callback_ids, vec![2]); // vim_lazy disabled, disable_vim survives
    assert!(ctx.any_match);
    assert!(ctx.clear_board);              // single matched rule, effective true -> replace
}
```

### Success Criteria

- [ ] `SECTION_9_TOML` is the verbatim spec §9 four-`[[rule]]` block with `layer = 10/11`.
- [ ] #1 + #14 rewritten per (b)/(c): `rs.rules.len()==4`, `Some(10)`/`Some(11)`, `dfc` assertions preserved.
- [ ] The ~28 mechanical CHANGE tests (d) compile against `rs.rules[i]` + `Option<u8>` layers; every `evaluate()` output assertion is byte-identical to pre-rewrite.
- [ ] #7 reframed to `parse_rules` + validity (optionally renamed `test_parse_rules_rejects_rule_with_only_match`); #17 TOML → `[[rule]]`, `is_err()` via `validate_rules`.
- [ ] #8 TOML → `[[rule]]`; `is_err()` via serde (match required) preserved.
- [ ] The 10 schema-agnostic tests (#10-13,15,16,18,19,45,46) are byte-untouched.
- [ ] #20 still compiles (no assertion change).
- [ ] The NEW test `test_evaluate_single_rule_layer_plus_callbacks` is added and asserts `Some(9)`/`[2]`/`any_match`/`clear_board`.
- [ ] **GATE A:** `cargo test --no-run` → zero `--> src/core/rules.rs:N` with `N >= ~495`.
- [ ] No file other than `src/core/rules.rs` is modified; within it, ONLY the test module.

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed
> to implement this successfully?"_ — **Yes.** The S1 contract (exact unified
> structs + renamed/extended `validate_rules` + one-pass `evaluate`), the verbatim
> target `SECTION_9_TOML`, the authoritative per-test inventory with every
> preserved assertion, the three RUN-FIRST sentinels, the #7/#17 reframe
> rationale, the new-test body, and the precise non-compilation gate (grep on
> `-->` pointers) are all below. The implementer need not read the firmware or
> external callers — only the rules.rs test module + S1's landed non-test code.

> **BASELINE ALERT.** When S2 runs, S1's non-test rewrite is LANDED (unified
> `Rule`/`RuleSet`/`evaluate`/`validate_rules`/`contradictory_callback_names`),
> but the inline `mod tests` still references the deleted `LayerRule`/`CallbackRule`
> /`.layer_rules`/`.callback_rules`/`SECTION_9_TOML`-with-224 — i.e. the test
> module does NOT compile. S3 (external callers) has NOT landed, so the crate as a
> whole doesn't compile either. S2's entry point is exactly that broken test module.

### Documentation & References

```yaml
# MUST READ — the sibling PRP whose output S2 consumes (the unified model CONTRACT)
- file: /home/dustin/projects/qmkonnect/plan/004_f48a103bcb32/P1M1T1S1/PRP.md
  why: "Defines the exact unified structs (RuleSet.rules: Vec<Rule> with rename=\"rule\"
        SINGULAR; Rule.layer: Option<u8>; enable/disable/dfc defaults), the renamed
        validate_rules (rejects Some(0xFF) AND match-only rules), the one-pass evaluate,
        and contradictory_callback_names over rules.rules. S2's tests must compile
        against EXACTLY this surface."
  section: "What (a) structs", "What (b) evaluate", "What (c) validate_rules"
  critical: "LayerRule/CallbackRule are DELETED. .layer_rules/.callback_rules are GONE.
             Any test referencing them is an expected S2 compile error (GATE A). layer
             assertions must move u8 -> Option<u8> (Some(...))."

# MUST READ — the canonical spec §9 (the verbatim target TOML + Rust model + Validity)
- file: /home/dustin/projects/qmkonnect/spec/HOST_RULES.md
  why: "§9 lines 458-481 = the verbatim SECTION_9_TOML target (four [[rule]] blocks,
        layer 10/11). §9 lines 485-502 = the verbatim Rule/RuleSet struct bodies.
        §9 Validity (514-518) = 'a rule must set >=1 of layer/enable/disable'. §8(3)
        = one-pass evaluation semantics. Copy FROM this file; do NOT edit it."
  section: "9. rules.toml Schema Reference" (TOML 458-481, Rust 485-502, Validity 514-518), "8(3)"
  critical: "[[rule]] is SINGULAR (rename = \"rule\"). layer is Option<u8>. A match-only
             rule fails validate_rules (not serde). layer==255 rejected."

# MUST READ — the file being edited (read the CURRENT test module before editing)
- file: /home/dustin/projects/qmkonnect/src/core/rules.rs
  why: "The #[cfg(test)] mod tests block (494-1377) is the ENTIRE edit surface. Read it
        in full before editing (every test body, every embedded TOML literal, every
        assertion). The non-test code (lines < 495) is S1's output — read it to confirm
        the exact field names/types you're compiling against, but DO NOT edit it."
  pattern: "Test style: #[test] fn test_<thing>_<scenario>; embedded TOML via raw strings
            r#\"...\"#; name_map(&[(name,id)]) helper; toml::from_str + parse_rules(tempfile).
            Single-threaded: cargo test --bin qmkonnect -- --test-threads=1 (AGENTS.md)."
  gotcha: "After editing, grep the test block for stale tokens (layer_rules|callback_rules|
           LayerRule|CallbackRule|SECTION_9_TOML) — expect ZERO hits except SECTION_9_TOML
           (the const name stays) and the intentional reframed tests."

# MUST READ — the architecture blast radius + invariants (confirms what S2 does NOT touch)
- file: /home/dustin/projects/qmkonnect/plan/004_f48a103bcb32/architecture/system_context.md
  why: "§1b enumerates the 4 behaviors that are ALREADY CORRECT and must be PRESERVED
        (C13 no-match, C11 0xFF rejection, disable-order-independence, stack-vs-replace) —
        these are exactly what the RUN-FIRST sentinels (#34/#35/#37) and the clear_board
        tests guard. §3 confirms external callers (notifier.rs:572, main.rs:253/271/281,
        mod.rs:191-233, pattern.rs:1090) are S3's scope, NOT S2's. §5 is the compile
        ordering model (why S2 alone can't green the build)."
  section: "1b. Behaviors ALREADY CORRECT", "3. Complete blast radius", "5. Dependency model"
  critical: "Do NOT touch external callers or their tests — that's S3. Do NOT touch non-test
             rules.rs — that's S1. S2 = the rules.rs #[cfg(test)] mod tests block ONLY."

# MUST READ — the authoritative per-test inventory (this subtask's research notes)
- docfile: /home/dustin/projects/qmkonnect/plan/004_f48a103bcb32/P1M1T1S2/research/notes.md
  why: "The item references plan/004/.../architecture/tests_research.md, but that file does
        NOT exist at that path (the real scout artifact is in .pi-subagents/artifacts/...,
        not promoted). notes.md §2 IS the authoritative inventory: all 46 tests classified
        CHANGE/REFRAME/COMPILE/KEEP with the EXACT preserved assertion per test, built by
        reading every test body. §4 lists the three RUN-FIRST sentinels; §5 the #7/#17
        reframe; §6 the new test; §7 the non-compilation gate."
  section: "§2 per-test inventory", "§4 RUN-FIRST sentinels", "§5 #7/#17 reframe", "§6 new test"
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/                                  # THIS repo
├── src/core/
│   ├── rules.rs        # <-- FILE TO EDIT (#[cfg(test)] mod tests block ONLY, 494-1377).
│   │                    #     non-test code (< 495) = S1's output (read-only for S2).
│   ├── pattern.rs      # Pattern + match_pattern (IMPORTED; doc-comment prose = S3).
│   ├── notifier.rs     # external caller (S3) — NOT touched here.
│   ├── mod.rs          # external caller render_rules_body (S3) — NOT touched here.
│   └── types.rs        # WindowInfo (unchanged).
├── src/main.rs         # external callers (S3) — NOT touched here.
├── spec/HOST_RULES.md  # §9 + §8(3) SOURCE OF TRUTH (already at target; copy FROM it).
└── plan/004_f48a103bcb32/architecture/system_context.md   # blast radius + 4 invariants
```

### Desired Codebase tree with files to be modified

```bash
src/core/
└── rules.rs   # MODIFIED — the #[cfg(test)] mod tests { … } block + SECTION_9_TOML const only
```

> No new files. No non-test edits. No external-caller edits (S3). No spec/docs edits.

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL: the crate WILL NOT fully compile at S2 time.
//   S1 landed the unified model; S3 (external callers) has NOT. So `cargo test --bin qmkonnect`
//   cannot build (external callers reference deleted LayerRule/CallbackRule/.layer_rules).
//   The S2 gate is GATE A: `cargo test --no-run` shows ZERO errors whose `-->` points into the
//   rules.rs test module (line >= ~495). Do NOT edit external files to chase a green build —
//   that steals S3's work and breaks the task boundary.

// CRITICAL: [[rule]] is SINGULAR (serde rename = "rule"), NOT [[rules]].
//   Every rewritten TOML literal must use [[rule]]. A plural key would silently parse to an
//   EMPTY rules vec (unknown field ignored under #[serde(default)]) and tests would pass for
//   the wrong reason. Double-check each literal.

// CRITICAL: layer is now Option<u8>, so layer assertions change u8 -> Option<u8>.
//   `== 224` becomes `== Some(224)`. A bare `== 224` is a type-mismatch compile error (GATE A
//   catches it). Preserve the NUMERIC value (don't "fix" 230/5/255 — only the §9 fixture is 10/11).

// CRITICAL: a [[rule]] with ONLY match no longer fails toml::from_str — it fails validate_rules.
//   Tests #7/#17 are match-only by intent; #7 must move from raw toml::from_str to parse_rules
//   (temp file) to hit the validity check; #17 already uses parse_rules (just swap the TOML key).
//   The parse_rules() -> is_err() boundary is PRESERVED; only the mechanism moves serde -> validity.

// CRITICAL: match is STILL required (rename="match", NO default).
//   Test #8 ([[rule]] with layer but no match) STILL fails serde. Keep its is_err(); only swap
//   the TOML key [[layer_rules]] -> [[rule]].

// CRITICAL: preserve #40's range-test literals (0,28,100,224,254).
//   test_parse_rules_accepts_low_layer_indices is a RANGE test, NOT the §9 fixture. The 224 in
//   it is intentional (probes the former floor). Only swap [[layer_rules]] -> [[rule]]; keep
//   the layer values. Do NOT "fix" 224 -> 10 here.

// CRITICAL: run #34, #35, #37 FIRST after the suite compiles.
//   These three are the ordering-bug sentinels for the one-pass merge (layer-match/callback-miss,
//   callback-match/layer-miss, disable-before-enable order-independence). If the one-pass collapse
//   has a subtle bug, these fail first and localize it.

// NOTE: single-threaded test execution is MANDATORY.
//   cargo test --bin qmkonnect -- --test-threads=1 (AGENTS.md: shared global debouncer state).
//   Parallel runs will flake. (Only relevant once S3 lands and the suite actually runs.)

// NOTE: RuleSet is NOT Clone (it owns Vec<Rule> with Strings).
//   Tests like #33 that need two host-default variants parse TWO fresh TOML copies (the existing
//   pattern). Don't try to .clone() a RuleSet.

// NOTE: the 10 schema-agnostic tests (#10-13,15,16,18,19,45,46) are byte-untouched.
//   Do not "tidy" them. They assert pure-helper behavior / file-IO / path transformation that
//   has no schema dependency. Touching them risks spurious failures.

// NOTE: the NEW test's layer must NOT be 255 (would trip validate_rules).
//   Use an arbitrary valid index (e.g. 9). Its point is layer+callbacks coexisting in one rule,
//   not probing the layer range.
```

## Implementation Blueprint

### Data models and structure

No data models are introduced — this is a test rewrite. The "structure" is the
test module's organization (sections A–E + the validity/contradictory/empty-pattern
groups), which is preserved. The only structural addition is the one new test.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: READ the current state + the S1 contract + the spec
  - READ: src/core/rules.rs in full — the non-test code (< 495, S1's output: confirm the exact
          Rule/RuleSet field names, Option<u8> layer, validate_rules name) AND the entire
          #[cfg(test)] mod tests block (494-1377, every body + TOML literal + assertion).
  - READ: spec/HOST_RULES.md §9 (458-481 target TOML; 485-502 struct bodies; 514-518 Validity)
          + §8(3) (one-pass semantics).
  - READ: plan/004_f48a103bcb32/P1M1T1S1/PRP.md "What" sections (the contract).
  - READ: plan/004_f48a103bcb32/P1M1T1S2/research/notes.md §2 (the authoritative per-test plan).
  - GOAL: know exactly which tests change, what each must preserve, and the exact target surface.

Task 2: REWRITE SECTION_9_TOML (rules.rs:499-521)
  - REPLACE: the split-schema const body with the verbatim spec §9 four-[[rule]] block (What (a)).
  - KEEP: the const name `SECTION_9_TOML` and its `///` doc comment (update the comment if it
          references 224/225 or split tables).
  - DO NOT: add a [host] table (spec §9 has none; the default-fallback is tested by #1's assertion).

Task 3: REWRITE the two SECTION_9_TOML consumers (#1 @524, #14 @732)
  - #1: per What (b) — rs.rules.len()==4; rs.rules[0..3] field/value assertions with Some(10)/Some(11).
  - #14: per What (c) — rs.rules.len()==4; rs.rules[0].layer==Some(10); dfc Some(true).
  - PRESERVE: all Pattern assertions and enable/disable/dfc value expectations.

Task 4: REWRITE the ~28 mechanical CHANGE tests (What (d))
  - For #2,#3,#4,#5,#6,#8,#9,#21-33,#36,#38,#41,#42,#43,#44: swap [[layer_rules]]/[[callback_rules]]
          -> [[rule]]; field access -> rs.rules[i]; layer values -> Some(...). PRESERVE every
          evaluate()/contradictory_*()/parse_rules output assertion (see notes.md §2 column).
  - For #39 (layer=255 via file): [[rule]] TOML; keep msg.contains("255") && msg.contains("clear").
  - For #40 (range test): [[rule]] TOML ONLY; KEEP literals [0,28,100,224,254].
  - For #41 (5 then 255): [[rule]] TOML; keep contains("255").
  - For #42,#43,#44 (contradictory): [[rule]] TOML; PRESERVE expected Vec output exactly.

Task 5: REFRAME #7 + #17 (What (e))
  - #7: rewrite to parse_rules(temp file) + is_err() + msg asserts a validity phrase. Optionally
          rename to test_parse_rules_rejects_rule_with_only_match. TOML: [[rule]] match="x".
  - #17: TOML -> [[rule]] match="x"; keep is_err() (now via validate_rules); add a mechanism comment.
  - #8: TOML -> [[rule]] layer=224 (no match); keep is_err() (serde: match required).

Task 6: ADD the new positive test (What (i))
  - ADD: test_evaluate_single_rule_layer_plus_callbacks among the evaluate() parity tests.
  - BODY: the verbatim test in What (i) (layer 9, enable [vim_lazy,disable_vim], disable [vim_lazy],
          dfc=true; asserts Some(9)/[2]/any_match/clear_board).
  - Reuse the existing name_map helper.

Task 7: CONFIRM the 10 schema-agnostic tests + #20 are untouched/compiling
  - #10-13,#15,#16,#18,#19,#45,#46: byte-untouched.
  - #20: confirm RuleSet::default() typechecks (it does); no assertion change.

Task 8: VALIDATE (GATE A is the real gate — see Validation Loop)
  - RUN: cargo test --no-run 2>&1 | tee /tmp/test-build.log
  - GATE A: grep for '--> src/core/rules.rs:N' with N >= 495 => expect ZERO. (Any such pointer
          is an S2 bug: a stale .layer_rules/LayerRule/bare-layer reference left in a test.)
  - CONFIRM remaining errors are ONLY in external callers (notifier.rs/main.rs/mod.rs/pattern.rs)
          = S3 scope.
  - GREP HYGIENE: grep -nE 'layer_rules|callback_rules|LayerRule|CallbackRule' src/core/rules.rs
          => expect ZERO hits in the test module (the SECTION_9_TOML const name is fine; the
          SPLIT-schema TOKENS must be gone).
  - SELF-REVIEW (GATE C): re-read each rewritten test against its preserved-assertion row in
          notes.md §2; confirm no HostContext value or msg.contains changed.

Task 9 (only meaningful post-S3): run the suite to green
  - Once S3 lands: cargo test --bin qmkonnect -- --test-threads=1 => ALL pass.
  - Run the three sentinels FIRST: cargo test --bin qmkonnect test_evaluate_layer_match_callback_miss
          test_evaluate_callback_match_layer_miss test_evaluate_disable_before_enable_still_excludes
          -- --test-threads=1
```

### Implementation Patterns & Key Details

```rust
// === THE MECHANICAL TOML SWAP (the most common edit) ===
//   BEFORE (split):                    AFTER (unified):
//   [[layer_rules]]                    [[rule]]
//   match = "a"                        match = "a"
//   layer = 224                        layer = 224          // value PRESERVED, now Option<u8>
//
//   [[callback_rules]]                 [[rule]]
//   match = "a"                        match = "a"
//   enable = ["x"]                     enable = ["x"]       // a callback-only rule is VALID
//                                                            // (>=1 of layer/enable/disable)


// === THE FIELD-ACCESS + ASSERTION SWAP (tests that index structs) ===
//   rs.layer_rules[0].layer == 224        =>  rs.rules[0].layer == Some(224)
//   rs.callback_rules[0].enable == vec![..] => rs.rules[2].enable == vec![..]  // §9 fixture index
//   rs.layer_rules.is_empty()             =>  rs.rules.is_empty()


// === THE #7 REFRAME — mechanism moves, boundary stays ===
//   BEFORE (serde error, layer was required u8):
//     let res = toml::from_str::<RuleSet>("[[layer_rules]]\nmatch=\"x\"\n");
//     assert!(res.is_err());
//   AFTER (validity error, layer is now Option<u8>):
//     // write [[rule]] match="x" to a temp file, then:
//     let res = parse_rules(&path);
//     assert!(res.is_err());
//     assert!(res.unwrap_err().to_string().contains("at least one of layer/enable/disable"));
//   (parse_rules -> is_err() boundary IDENTICAL to what --validate-rules / #17 rely on.)


// === WHY the new test matters (C8 headline) ===
//   Under the SPLIT schema a single rule could set layer XOR callbacks, never both.
//   The unified [[rule]] allows BOTH in one entry. test_evaluate_single_rule_layer_plus_callbacks
//   proves the one-pass evaluator lets the SAME rule contribute layer (first-match) AND callbacks
//   (all-match) AND a single effective flag (clear_board). This is the capability that justifies
//   the whole C8 unification.


// === THE NON-COMPILATION REALITY (do not panic) ===
//   cargo test --no-run will show errors in notifier.rs/main.rs/mod.rs/pattern.rs (S3) AND
//   possibly in the rules.rs test module (S2). GATE A = zero rules.rs-test-module errors.
//   External-caller errors are EXPECTED and are S3's entry point.
```

### Integration Points

```yaml
SOURCE FILES:
  - modify: "src/core/rules.rs — the #[cfg(test)] mod tests { … } block + SECTION_9_TOML const ONLY"

DEPENDENCIES / BUILD:
  - none new. serde/toml/tempfile already present. No Cargo.toml change.

PUBLIC API SURFACE:
  - none. Tests are not public API. No production signature changes (S1 owned those).

UPSTREAM CONTRACT (S1 — assumed LANDED when S2 runs):
  - consumes: "RuleSet.rules: Vec<Rule> (rename=\"rule\"); Rule.layer: Option<u8>; validate_rules
               (rejects Some(0xFF) + match-only); evaluate() one-pass over rules.rules;
               contradictory_callback_names over rules.rules; LayerRule/CallbackRule DELETED."

DOWNSTREAM (do NOT implement — listed for awareness):
  - P1.M1.T1.S3: "Fix external callers (notifier.rs:572, main.rs:253/271/281/442-443, mod.rs:191-
                  233/377-378/390-391, pattern.rs:1090) + their tests. Once S3 lands, the suite
                  S2 rewrote runs green."
  - P1.M1.T2: "Sync user-facing docs/*.md to [[rule]] (separate milestone)."

VALIDATION CONSUMERS:
  - The rules.rs test suite itself IS the parity contract for evaluate()/validate_rules. S3's
    external-caller tests align to the same unified model.
```

## Validation Loop

> All commands run from the repo root: `/home/dustin/projects/qmkonnect`.
> **⚠ THE PRIMARY GATE IS NOT "`cargo test` green".** At S2 time the crate can't
> compile (external callers = S3). The gate is **GATE A: the rules.rs test module
> compiles cleanly against S1's unified model** (zero `-->` pointers into it).

### Level 1: Test-module type-correctness (the real gate — GATE A)

```bash
cd /home/dustin/projects/qmkonnect

# `cargo test --no-run` compiles the test modules too. rustc reports errors across the whole
# crate even when some items are missing, so any stale rules.rs-test-module reference surfaces here.
cargo test --no-run 2>&1 | tee /tmp/test-build.log

# GATE A — zero compile errors whose `-->` points into the rules.rs TEST MODULE (line >= ~495).
grep -nE '\-\-> src/core/rules\.rs:[0-9]+' /tmp/test-build.log | while read ln; do
  num=$(echo "$ln" | grep -oE 'rules\.rs:[0-9]+' | grep -oE '[0-9]+');
  if [ "$num" -ge 495 ]; then echo "VIOLATION (rules.rs test module): $ln"; fi
done
# Expected: NO "VIOLATION" lines. Any VIOLATION = a stale .layer_rules/LayerRule/bare-layer
# reference left in a test — fix it before proceeding.

# Confirm every remaining error IS in the external-caller set (S3 scope — expected, not a bug).
grep -E 'error\[E[0-9]+\]|error:' /tmp/test-build.log | grep -oE 'src/[a-zA-Z0-9_/]+\.rs' | sort -u
# Expected: src/core/notifier.rs, src/main.rs, src/core/mod.rs, (maybe src/core/pattern.rs).
# NOT src/core/rules.rs (the test module is clean per GATE A).
```

### Level 2: Token hygiene (no stale split-schema tokens in the test module)

```bash
cd /home/dustin/projects/qmkonnect

# After editing, the test module must contain ZERO stale split-schema tokens.
sed -n '494,2000p' src/core/rules.rs | grep -nE '\[\[(layer_rules|callback_rules)\]\]|\.layer_rules|\.callback_rules|\bLayerRule\b|\bCallbackRule\b' \
  || echo "clean: no stale split-schema tokens in the rules.rs test module"
# Expected: "clean: ...". (SECTION_9_TOML the CONST NAME is fine; only the SPLIT TOKENS must go.)

# Confirm [[rule]] is SINGULAR everywhere in the test module's TOML literals.
sed -n '494,2000p' src/core/rules.rs | grep -cE '\[\[rule\]\]'
# Expected: a positive count (the rewritten literals). Cross-check NO [[rules]] (plural):
sed -n '494,2000p' src/core/rules.rs | grep -nE '\[\[rules\]\]' && echo "BUG: plural [[rules]]" || echo "ok: singular [[rule]] only"

# Confirm the §9 fixture uses 10/11 (not 224/225) — but the range test #40 KEEPS 224.
grep -nE 'layer = 2[2-5][0-9]' src/core/rules.rs
# Expected: ONLY inside test_parse_rules_accepts_low_layer_indices (#40, intentional) — nowhere
# else (notably NOT in SECTION_9_TOML). Review each hit to confirm it's #40.
```

### Level 3: Logical self-review (GATE C — the behavior-preservation gate)

```text
For each rewritten test, re-read its body against the PRESERVED-ASSERTION column in
research/notes.md §2 and confirm the observable output is unchanged:

- evaluate() parity tests (#21-#38): the HostContext { layer: Option<u8>, callback_ids: Vec<u8>,
  clear_board: bool, any_match: bool } values are byte-identical to pre-rewrite. The C13 no-match
  short-circuit (#21,#22) returns clear_board:false. First-match-wins (#23,#24,#34) and
  order-independent exclusion (#36,#37,#38) hold.  ☐ each confirmed

- validity tests (#39,#40,#41): parse_rules rejects Some(0xFF) with msg contains "255" && "clear";
  accepts the range [0,28,100,224,254]; reports the first bad layer.  ☐ confirmed

- contradictory tests (#42,#43,#44): same-rule overlap flagged; cross-rule NOT flagged; deduped+
  sorted output.  ☐ confirmed

- #7/#17: parse_rules is_err() on a match-only [[rule]] (validity mechanism). #8: serde is_err()
  on a match-less [[rule]] (match required).  ☐ confirmed

- NEW test: single [[rule]] with layer+enable+disable evaluates to Some(9)/[2]/any_match/clear_board.  ☐ confirmed
```

### Level 4: Post-S3 full run (the eventual green gate)

```bash
cd /home/dustin/projects/qmkonnect
# ONLY meaningful once S3 (external callers) has landed. Then:
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL tests pass (36 rewritten + 10 untouched + 1 new). Single-threaded is MANDATORY
# (AGENTS.md: shared global debouncer).

# Run the three ordering sentinels FIRST to localize any one-pass-merge regression:
cargo test --bin qmkonnect -- --test-threads=1 \
  test_evaluate_layer_match_callback_miss \
  test_evaluate_callback_match_layer_miss \
  test_evaluate_disable_before_enable_still_excludes
# Expected: 3 passed. If any fails, the one-pass evaluate() (S1) has an ordering bug — the
# rewrite itself is likely correct; flag it back to S1's logic, not the test literals.
```

## Final Validation Checklist

### Technical Validation

- [ ] GATE A: `cargo test --no-run` → zero `--> src/core/rules.rs:N` with `N >= ~495`.
- [ ] Level 2: no stale `layer_rules`/`callback_rules`/`LayerRule`/`CallbackRule` tokens in the test module.
- [ ] Level 2: `[[rule]]` is SINGULAR throughout; no `[[rules]]`.
- [ ] Level 2: `layer = 2xx` appears ONLY in #40's range test, never in `SECTION_9_TOML`.
- [ ] GATE C: every preserved assertion (notes.md §2) confirmed unchanged by re-reading.
- [ ] (post-S3) `cargo test --bin qmkonnect -- --test-threads=1` → all pass.

### Feature Validation

- [ ] `SECTION_9_TOML` is the verbatim spec §9 four-`[[rule]]` block with `layer = 10/11`.
- [ ] #1 + #14 consume `rs.rules[0..3]` with `Some(10)`/`Some(11)` + preserved Pattern/dfc asserts.
- [ ] The ~28 mechanical CHANGE tests use `rs.rules[i]` + `Some(...)` layers; `evaluate()` outputs byte-identical.
- [ ] #7 reframed (parse_rules + validity); #17 TOML→`[[rule]]` (is_err via validate_rules); #8 TOML→`[[rule]]` (is_err via serde).
- [ ] #40's range literals `[0,28,100,224,254]` PRESERVED (only the TOML key swapped).
- [ ] The 10 schema-agnostic tests are byte-untouched; #20 compiles unchanged.
- [ ] The NEW test `test_evaluate_single_rule_layer_plus_callbacks` is added with the specified assertions.

### Code Quality Validation

- [ ] Test style follows the existing `test_<thing>_<scenario>` naming + `name_map` helper.
- [ ] Embedded TOML literals use raw strings `r#"…"#` (existing convention).
- [ ] No `#[allow(...)]` attributes added (unnecessary).
- [ ] Only `src/core/rules.rs` modified; within it, ONLY the test module + `SECTION_9_TOML` const.

### Documentation & Deployment

- [ ] Mode A — test-only work; no user-facing/config/API surface change; no doc files touched.
- [ ] No environment variables, config, or Cargo.toml changes.

---

## Anti-Patterns to Avoid

- ❌ Don't use "`cargo test` green" as the S2 gate — it CAN'T be green until S3 lands. The gate is
  GATE A (zero rules.rs-test-module compile errors). Chasing a green build by editing external
  callers steals S3's work and breaks the task boundary.
- ❌ Don't pluralize `[[rules]]` — it is SINGULAR `[[rule]]` (serde `rename = "rule"`). A plural key
  would silently parse to an empty rules vec and tests would pass for the wrong reason.
- ❌ Don't leave a bare `layer == 224` (u8) — it must be `== Some(224)` (Option<u8>). It's a
  type-mismatch compile error; GATE A catches it, but fix it at edit time.
- ❌ Don't alter any `evaluate()` output assertion's VALUE — only the TOML literal + field-access
  path change. The unification is behavior-preserving (S1's promise); this suite is the proof.
- ❌ Don't "fix" the `224`/`254` literals in #40's range test — they're intentional (probing the
  0..=254 valid range). Only the §9 FIXTURE is 10/11.
- ❌ Don't leave #7 calling raw `toml::from_str` expecting an error — under the unified schema a
  match-only rule deserializes fine; it must go through `parse_rules` to hit `validate_rules`.
- ❌ Don't drop the `is_err()` boundary on #8 — `match` is still required (no serde default), so a
  match-less `[[rule]]` still fails serde. Keep `is_err()`; only swap the TOML key.
- ❌ Don't touch the 10 schema-agnostic tests (#10-13,15,16,18,19,45,46) — they have no schema
  dependency; "tidying" them risks spurious failures.
- ❌ Don't edit non-test `rules.rs` (S1's scope) or external callers (S3's scope) or spec/docs.
- ❌ Don't use a parallel test run — `--test-threads=1` is mandatory (AGENTS.md: shared global debouncer).
- ❌ Don't skip the three RUN-FIRST sentinels (#34/#35/#37) when the suite finally runs — they
  localize one-pass-merge ordering bugs that the other tests won't catch.
- ❌ Don't give the NEW test a `layer` of 255 (trips validate_rules) — use a valid index (e.g. 9).
- ❌ Don't rename `SECTION_9_TOML` — tests #1/#14 reference it by name; only its BODY changes.

---

**Confidence Score: 9/10** for one-pass implementation success. The deliverable is a mechanical,
behavior-preserving port of an existing suite: the S1 contract (exact unified structs + renamed
`validate_rules`), the verbatim spec §9 target TOML, the authoritative per-test inventory with
every preserved assertion, the three RUN-FIRST sentinels, the #7/#17 reframe rationale, and the
new-test body are all specified. The non-compilation reality is confronted with a precise grep-based
gate (GATE A: zero `-->` into the rules.rs test module) instead of a misleading "test green". The
one residual risk: a subtle preserved-assertion typo (e.g. dropping a `Some(...)`) — mitigated by
GATE A (type-mismatch compile error) and the Level-2 token-hygiene grep. Once S3 lands, the
single-threaded full run is the final confirmation, with the three sentinels run first.
# Research Notes — P1.M1.T1.S2 (Rewrite the rules.rs test suite to `[[rule]]`)

> **Authoritative per-test inventory.** The item description references
> `plan/004_f48a103bcb32/architecture/tests_research.md`, but that file does NOT
> exist at that path — the real scout artifact is at
> `.pi-subagents/artifacts/outputs/55dbc9e6/plan/004_f48a103bcb32/architecture/tests_research.md`
> (a subagent output, not promoted into the plan tree). Rather than depend on an
> un-promoted artifact, this file IS the authoritative inventory: it was built by
> reading every one of the 46 test bodies in `src/core/rules.rs:494-1377` in full.
> Line numbers are from the CURRENT (pre-S1/S2) tree and are anchors, not contracts.

## 0. The contract I build on (S1 output — treat as law)

S1 (parallel sibling, "Implementing") rewrites the NON-TEST portion of
`src/core/rules.rs`. Per its PRP, when S2 runs the model is exactly:

```rust
pub struct RuleSet {                                    // serde(rename none on the struct
    #[serde(default)] pub host: HostDefaults,          //   itself; host kept)
    #[serde(default, rename = "rule")] pub rules: Vec<Rule>,   // ← ONE array, SINGULAR rename
}
pub struct Rule {
    #[serde(rename = "match")] pub pattern: Pattern,        // required (NO default)
    #[serde(default)] pub layer: Option<u8>,                // ← was required u8; now Option
    #[serde(default)] pub enable: Vec<String>,
    #[serde(default)] pub disable: Vec<String>,
    #[serde(default)] pub case_sensitive: bool,
    #[serde(default)] pub disable_firmware_config: Option<bool>,
}
```
- `LayerRule` + `CallbackRule` are **DELETED**. `RuleSet.layer_rules`/`.callback_rules` are GONE.
- `evaluate()` is ONE pass over `rules.rules`; signature + `HostContext` unchanged.
- `validate_layers` → `validate_rules` (renamed). It now rejects (a) `layer == Some(0xFF)` AND
  (b) a rule with `layer.is_none() && enable.is_empty() && disable.is_empty()` (match-only).
- `contradictory_callback_names` iterates `rules.rules` (was `.callback_rules`).
- `parse_rules` signature unchanged; it calls `validate_rules` (renamed call site).
- `HostContext`, `effective_disable_firmware_config`, `pattern_is_empty_core`, `get_rules_paths`,
  `HostDefaults`, the `Pattern`/`match_pattern` import — UNCHANGED.

The crate will NOT compile end-to-end until S3 (external callers) lands. S2's gate is
"the rules.rs `#[cfg(test)] mod tests` block compiles cleanly against the unified model"
(see PRP Validation Loop).

## 1. The verbatim target `SECTION_9_TOML` (from `spec/HOST_RULES.md` §9, lines 458-481)

```toml
[[rule]]
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
```

Note: the spec §9 example has NO `[host]` table. Test #1's assertion
`assert!(!(rs.host.disable_firmware_config))` still passes because `host` is
`#[serde(default)]` → `HostDefaults::default()` → `disable_firmware_config: false`.
(If you prefer to keep an explicit `[host]\ndisable_firmware_config = false\n\n`
header at the top of SECTION_9_TOML, that is also fine — both pass. The item says
"VERBATIM from spec §9"; the verbatim block has no `[host]`, so prefer that.)

Layer change: `224`→`10`, `225`→`11`. Field-access change: the 4 entries are now
`rs.rules[0..3]` in order (was `rs.layer_rules[0..1]` + `rs.callback_rules[0..1]`).

## 2. The authoritative per-test inventory (46 tests, rules.rs:494-1377)

Legend: **CHANGE** = TOML/field-access rewrite, preserve assertion. **REFRAME** = mechanism
moves (serde → validate_rules) but `is_err()` boundary kept. **COMPILE** = just needs to
type-check against the new model. **KEEP** = schema-agnostic, byte-untouched.

| # | Test (line) | TOML schema | Field access | Action | PRESERVED assertion (do NOT alter the value) |
|---|---|---|---|---|---|
| 1 | `test_rules_full_section9_example_parses` (524) | SECTION_9_TOML | `.layer_rules`/`.callback_rules` | **CHANGE** | `!host.dfc`; `rs.rules.len()==4`; `rules[0].layer==Some(10)`+`dfc Some(true)`; `rules[1].layer==Some(11)`+`dfc None`+`Parts("*chrome*","*youtube*")`; `rules[2]` enable/disable + `Single("neovide")`; `rules[3]` enable + `Parts("*chrome*","*claude*")`+`dfc Some(true)` |
| 2 | `test_rules_missing_host_table_defaults_false` (581) | `[[layer_rules]]` | `.layer_rules` | **CHANGE** | `!host.dfc`; rules count 1 |
| 3 | `test_rules_empty_toml_is_all_default` (595) | (empty) | `.layer_rules.is_empty()`+`.callback_rules.is_empty()` | **CHANGE** | `!host.dfc`; `rules.is_empty()`; `== RuleSet::default().host.dfc` |
| 4 | `test_rules_layer_override_absent_is_none` (608) | `[[layer_rules]]` (layer 230) | `.layer_rules[0].dfc` | **CHANGE** | `rules[0].dfc == None` (keep a layer so rule is valid) |
| 5 | `test_rules_callback_enable_disable_default_empty` (620) | `[[callback_rules]]` | `.callback_rules[0]` | **CHANGE** | `enable==["vim_lazy"]`, `disable==[]` (and vice versa). NOTE: a `[[rule]]` with only `enable` is now VALID (no layer needed). |
| 6 | `test_rules_match_string_to_single_and_array_to_parts` (644) | `[[layer_rules]]` (layer 224) | `.layer_rules[0].pattern` | **CHANGE** | `Single("x")`, `Parts("a","b")` (keep a layer so rule is valid) |
| 7 | `test_rules_missing_layer_errors` (668) | `[[layer_rules]]` match-only, raw `toml::from_str` | — | **REFRAME** | Under unified: `[[rule]] match="x"` (no layer/enable/disable) → `toml::from_str` **SUCCEEDS** (layer Option). Must move to `parse_rules` (temp file) + assert `is_err()` AND msg mentions validity. Optionally RENAME to `test_parse_rules_rejects_rule_with_only_match`. |
| 8 | `test_rules_missing_match_errors` (679) | `[[layer_rules]]` layer-only, raw `toml::from_str` | — | **CHANGE** (TOML only) | `[[rule]] layer=224` (no match) → `match` is required (no default) → serde STILL errors. Keep `is_err()`. |
| 9 | `test_rules_default_propagates` (690) | — | `.layer_rules.is_empty()`+`.callback_rules.is_empty()` | **CHANGE** | `!host.dfc`; `rules.is_empty()` |
| 10 | `test_rules_effective_some_true_wins` (706) | — (pure fn) | — | **KEEP** | `effective_…(Some(true),false)==true` |
| 11 | `test_rules_effective_some_false_wins` (712) | — | — | **KEEP** | `==(false)` |
| 12 | `test_rules_effective_none_inherits_false` (717) | — | — | **KEEP** | `==(false)` |
| 13 | `test_rules_effective_none_inherits_true` (723) | — | — | **KEEP** | `==(true)` |
| 14 | `test_rules_parse_valid_section9` (732) | SECTION_9_TOML via file | `.layer_rules.len()==2`+`.callback_rules.len()==2` | **CHANGE** | `parse_rules` Ok; `rules.len()==4`; `rules[0].layer==Some(10)`; `rules[0].dfc==Some(true)` |
| 15 | `test_rules_parse_missing_file_errors` (754) | — (no file) | — | **KEEP** | `parse_rules` is_err |
| 16 | `test_rules_parse_malformed_toml_errors` (761) | malformed | — | **KEEP** | `parse_rules` is_err |
| 17 | `test_rules_parse_missing_required_field_errors` (771) | `[[layer_rules]]` match-only via file | — | **CHANGE** (TOML) | `[[rule]] match="x"` → validate_rules fails → `parse_rules` `is_err()`. Boundary identical (was serde, now validity). Keep `is_err()`. |
| 18 | `test_rules_paths_swap_filename` (792) | — | — | **KEEP** | path-count + same-dir + filename swap |
| 19 | `test_rules_paths_delegate_count` (815) | — | — | **KEEP** | `!get_rules_paths().is_empty()` on supported OS |
| 20 | `test_evaluate_empty_ruleset_no_match` (841) | — (`RuleSet::default()`) | — | **COMPILE** | `ctx == {None,vec![],false,false}`. Just needs `RuleSet::default()` to still typecheck (it does: `rules: Vec::default()`). |
| 21 | `test_evaluate_no_layer_no_callback_match` (858) | `[[layer_rules]]`+`[[callback_rules]]` | — | **CHANGE** | no match → `{None,vec![],false,false}` |
| 22 | `test_evaluate_no_match_clear_board_always_false` (889) | `[host] dfc=true`+`[[layer_rules]]` | — | **CHANGE** | C13: no-match → `clear_board:false` (NOT host default). |
| 23 | `test_evaluate_layer_first_match_wins` (913) | 2×`[[layer_rules]]` (224/225) | — | **CHANGE** | `ctx.layer==Some(224)`, `any_match` |
| 24 | `test_evaluate_layer_second_when_first_misses` (933) | 2×`[[layer_rules]]` | — | **CHANGE** | `ctx.layer==Some(230)` |
| 25 | `test_evaluate_layer_parts_requires_both_halves` (952) | `[[layer_rules]]` Parts | — | **CHANGE** | title-half fails → `layer None`, `!any_match` |
| 26 | `test_evaluate_callback_all_matches_union` (970) | 2×`[[callback_rules]]` | — | **CHANGE** | `callback_ids==[1,2]` |
| 27 | `test_evaluate_callback_disable_is_exclusion` (990) | 2×`[[callback_rules]]` | — | **CHANGE** | `callback_ids==[]` |
| 28 | `test_evaluate_unknown_name_skipped` (1010) | `[[callback_rules]]` | — | **CHANGE** | `callback_ids==[1]` (ghost skipped) |
| 29 | `test_evaluate_callback_ids_sorted` (1026) | 3×`[[callback_rules]]` | — | **CHANGE** | `callback_ids==[1,2,3]` (BTreeSet) |
| 30 | `test_evaluate_clear_board_all_disabling` (1051) | `[[layer_rules]]` dfc=true | — | **CHANGE** | `layer Some(224)`, `clear_board` true |
| 31 | `test_evaluate_clear_board_one_nondisabling_is_false` (1069) | `[host] true`+`[[layer_rules]]` dfc=false | — | **CHANGE** | `!clear_board` (stack) |
| 32 | `test_evaluate_clear_board_no_board_rules` (1089) | `[[layer_rules]]` dfc=false, `board_has_rules=false` | — | **CHANGE** | `clear_board` true (`!board_has_rules`) |
| 33 | `test_evaluate_effective_inherits_host_default` (1106) | 2 TOMLs `[host] false/true`+`[[layer_rules]]` | — | **CHANGE** | (a)`!clear_board` (b)`clear_board` |
| 34 | `test_evaluate_layer_match_callback_miss` (1143) | `[[layer_rules]]`+`[[callback_rules]]` | — | **CHANGE — RUN FIRST** | `layer Some(224)`, `callback_ids []`, `any_match`, `!clear_board` |
| 35 | `test_evaluate_callback_match_layer_miss` (1166) | `[[layer_rules]]`+`[[callback_rules]]` | — | **CHANGE — RUN FIRST** | `layer None`, `callback_ids [1]`, `any_match` |
| 36 | `test_evaluate_disable_after_enable_excludes` (1194) | 2×`[[callback_rules]]` | — | **CHANGE** | `callback_ids==[]` |
| 37 | `test_evaluate_disable_before_enable_still_excludes` (1214) | 2×`[[callback_rules]]` (disable first) | — | **CHANGE — RUN FIRST** | `callback_ids==[]` (order-independent exclusion) |
| 38 | `test_evaluate_disable_excludes_only_named_others_survive` (1238) | 2×`[[callback_rules]]` | — | **CHANGE** | `callback_ids==[2,3]` |
| 39 | `test_parse_rules_rejects_layer_255_clear_sentinel` (1263) | `[[layer_rules]]` layer=255 via file | — | **CHANGE** | `parse_rules` err; msg `contains("255")` && `contains("clear")` |
| 40 | `test_parse_rules_accepts_low_layer_indices` (1280) | `[[layer_rules]]` loop [0,28,100,224,254] | — | **CHANGE** (TOML only) | each layer is `Ok`. **KEEP the 224 literal** — it's a RANGE test, not the §9 fixture. |
| 41 | `test_parse_rules_reports_first_bad_layer` (1294) | 2×`[[layer_rules]]` (5, then 255) | — | **CHANGE** | err; msg `contains("255")` |
| 42 | `test_contradictory_callback_names_flags_same_rule_overlap` (1314) | `[[callback_rules]]` | — | **CHANGE** | `== ["foo"]` |
| 43 | `test_contradictory_callback_names_cross_rule_is_not_contradictory` (1328) | 2×`[[callback_rules]]` | — | **CHANGE** | `== []` |
| 44 | `test_contradictory_callback_names_deduped_sorted` (1345) | `[[callback_rules]]` | — | **CHANGE** | `== ["m","z"]` |
| 45 | `test_pattern_is_empty_core_single` (1362) | — | — | **KEEP** | empty-core detection |
| 46 | `test_pattern_is_empty_core_parts` (1370) | — | — | **KEEP** | empty-half detection |

**Counts:** CHANGE+REFRAME+COMPILE = #1-9 (9) + #14,#17 (2) + #20 (1) + #21-#44 (24) = **36 touched**;
KEEP = #10-13,15,16,18,19,45,46 = **10 untouched**. (The item's "40" is approximate and counts
#20 + borderline; the authoritative number from reading every body is 36 touched / 10 untouched.)
**Plus 1 NEW test** to add (single `[[rule]]` carrying BOTH layer AND enable/disable).

## 3. The mechanical rewrite recipe (applies to ~30 of the CHANGE rows)

For any test whose TOML literal uses `[[layer_rules]]` or `[[callback_rules]]`:
1. Swap the table-array key → `[[rule]]` (SINGULAR).
2. If the rule has no `layer`/`enable`/`disable` after the swap, ADD a minimal one so it passes
   `validate_rules` (e.g. tests #4/#6 set `layer = <n>`; tests #5/#26-#29 already have enable/disable).
3. Change field access `rs.layer_rules[i]`/`rs.callback_rules[i]` → `rs.rules[i]`, recomputed index
   in file order (the §9 fixture collapses 2+2 → 4 contiguous).
4. Change layer-value assertions `== 224`/`== 225` (bare u8) → `== Some(10)`/`== Some(11)` (Option<u8>).
   Range/fixture-internal layers like 230/#4, 5/#41, 255/#39 stay as their values but wrapped in `Some(...)`.
5. **PRESERVE every `evaluate()` output assertion byte-for-byte** (layer Option, callback_ids Vec,
   clear_board bool, any_match bool). The unification is behavior-preserving (S1's promise).

## 4. The three "RUN FIRST" sentinels (item-mandated ordering-bug catchers)

- **#34 `test_evaluate_layer_match_callback_miss`** — layer matches, callback misses. Under the old
  two-scan model these were independent arrays; under the one-pass model a single rule contributes
  to BOTH. This test (split into 2 rules) confirms layer-set + callback-empty coexist in one pass.
- **#35 `test_evaluate_callback_match_layer_miss`** — mirror: callback matches, layer misses.
- **#37 `test_evaluate_disable_before_enable_still_excludes`** — THE regression guard for
  disable-order-independence (two-BTreeSet difference). If the one-pass merge accidentally folded
  disable into insert-then-remove, this fails. Highest signal of a correctness regression.

Run these three first (once the suite compiles) before trusting the rest.

## 5. The #7/#17 reframing (validity-mechanism change)

Under the unified schema `layer` is `Option<u8>`, so a `[[rule]]` with ONLY `match` no longer fails
`toml::from_str` — it deserializes fine (layer=None). It must fail the NEW check in `validate_rules`
("must set at least one of layer/enable/disable"). The `parse_rules() → is_err()` boundary is
preserved; only the mechanism moves from serde to validate_rules.

- **#7** currently calls `toml::from_str::<RuleSet>` directly and asserts `is_err()`. After the
  reframe it MUST go through `parse_rules` (temp file) to hit `validate_rules`. Recommended: rename
  to `test_parse_rules_rejects_rule_with_only_match`, assert `is_err()` AND `msg.contains` a
  validity phrase. (The item explicitly sanctions merging #7 into this named test.)
- **#17** already goes through `parse_rules` (temp file) — just change the TOML literal to
  `[[rule]] match="x"`; the `is_err()` holds via validate_rules. Mechanism note in a comment.

## 6. The NEW positive test to add (unified-model key capability)

A single `[[rule]]` carrying BOTH a `layer` AND `enable`/`disable` parses AND evaluates such that
the layer contributes to first-match and the callbacks accumulate. This is the capability the split
schema FORBADE and the unified schema ENABLES — the headline behavior of C8. Sketch:

```rust
#[test]
fn test_evaluate_single_rule_layer_plus_callbacks() {
    // The unified model's key capability: ONE [[rule]] sets BOTH a layer and callbacks.
    // (Impossible under the old split schema.) layer contributes to first-match;
    // callbacks accumulate; clear_board follows the single effective flag.
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
    assert_eq!(ctx.layer, Some(9));            // layer from the same rule
    assert_eq!(ctx.callback_ids, vec![2]);     // vim_lazy disabled, disable_vim survives
    assert!(ctx.any_match);
    assert!(ctx.clear_board);                  // single matched rule, effective true -> replace
}
```
Place it among the evaluate() parity tests (after #33 or near the cross-stage group). Reuses the
existing `name_map` helper. (Layer 9 is arbitrary-valid, ≠ 255.)

## 7. Validation reality (the gate is NOT "cargo test green" at S2 time)

S1 lands the unified model; S3 (external callers: notifier.rs/main.rs/mod.rs/pattern.rs) has NOT
landed when S2 runs. So `cargo test --bin qmkonnect` cannot compile (external callers reference the
deleted `LayerRule`/`CallbackRule`/`.layer_rules`/`.callback_rules`). The S2 gate is:

- **GATE A (the real one):** `cargo test --no-run` → ZERO compile errors whose `-->` points into the
  rules.rs `#[cfg(test)] mod tests` block (line >= ~495). rustc reports errors across the whole crate
  even when some items are missing, so rules.rs-test-module errors WILL surface if S2 left any stale
  `.layer_rules`/`LayerRule`/bare-`layer` reference. All errors must be in the external-caller files
  (S3 scope). (Mirrors S1's GATE A, applied to the test module.)
- **GATE B (post-S3):** once S3 lands, `cargo test --bin qmkonnect -- --test-threads=1` → ALL pass.
  S2's job is to ensure the rules.rs tests are CORRECT so that when S3 lands the suite goes green.
- **GATE C (logical, now):** re-read each rewritten test against its PRESERVED-assertion column in
  §2; every `HostContext` value + every validity `msg.contains(...)` is unchanged from the pre-rewrite
  suite. (Single-threaded `--test-threads=1` is mandatory per AGENTS.md: shared global debouncer.)

## 8. Files NOT to touch (boundary discipline)

- `src/core/rules.rs` NON-TEST code (lines < ~495) — S1's scope. S2 edits ONLY the `#[cfg(test)] mod
  tests { ... }` block and the `SECTION_9_TOML` const inside it.
- `src/core/notifier.rs`, `src/main.rs`, `src/core/mod.rs`, `src/core/pattern.rs` + their tests — S3.
- `spec/HOST_RULES.md` — already at target wording (source of truth; copy FROM it).
- `docs/*.md`, `docs/llms_full.txt` — P1.M1.T2.
- Wire path, crate, firmware — untouched.

## 9. Risk inventory

1. **Stale field access** (`rs.layer_rules`/`rs.callback_rules`/`LayerRule`/`CallbackRule`) left in a
   test → GATE A catches it (compile error pointing into the test module). Mitigation: grep the test
   block for these tokens after editing; expect zero hits.
2. **Bare `layer == 224`** left as u8 instead of `Some(224)` → compile error (type mismatch u8 vs
   Option<u8>). GATE A catches.
3. **A `[[rule]]` left with only `match`** in a deserialization test → would now PASS serde and break
   an `is_err()` expectation. Only #7/#17 are match-only by intent; both are reframed to validate_rules.
4. **Behavioral drift** in a preserved assertion (e.g. callback order, clear_board) — GATE C
   self-review + the three RUN-FIRST sentinels (#34/#35/#37) catch.
5. **#40 range-test literal `224`** accidentally "fixed" to 10 — would weaken the range test. KEEP it.
6. **New test's layer** accidentally 255 — would hit validate_rules. Use a valid index (e.g. 9).
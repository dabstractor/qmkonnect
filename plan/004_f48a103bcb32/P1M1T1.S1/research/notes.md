# Research Notes — P1.M1.T1.S1: Unify rules.rs model + evaluator + validity

> Scope: collapse `[[layer_rules]]`/`[[callback_rules]]` → one `[[rule]]` array in
> `src/core/rules.rs` (non-test code + doc-comments ONLY). Inline `mod tests` is
> P1.M1.T1.S2; external callers are P1.M1.T1.S3.

## 1. Validated current state (`src/core/rules.rs`, 1378 lines)

| Item | Lines | Current shape | Target |
|---|---|---|---|
| `RuleSet` | 70–83 | `host` + `layer_rules:Vec<LayerRule>` (rename="layer_rules") + `callback_rules:Vec<CallbackRule>` (rename="callback_rules"); `#[derive(Debug, Deserialize, Default)]` | `host` + `rules:Vec<Rule>` (rename="rule", SINGULAR); same derives |
| `LayerRule` | 126 | `pattern`(rename="match"), `layer:u8` (REQUIRED), `case_sensitive`, `disable_firmware_config:Option<bool>` | DELETED |
| `CallbackRule` | 165 | `pattern`, `enable:Vec<String>`, `disable:Vec<String>`, `case_sensitive`, `disable_firmware_config:Option<bool>` | DELETED |
| `HostDefaults` | ~107 | `disable_firmware_config:bool` (default false), manual `Default` | UNCHANGED |
| `effective_disable_firmware_config` | 230 | `fn(Option<bool>, bool) -> bool` (pure) | UNCHANGED |
| `parse_rules` | 237 | read → `toml::from_str` → `validate_layers(&rules)` | call `validate_rules` instead (rename) |
| `validate_layers` | 253 | iter `layer_rules`, reject `layer==0xFF` | → `validate_rules`: iter `rules.rules`; reject `layer==Some(0xFF)` + NEW "must set ≥1 of layer/enable/disable" |
| `get_rules_paths` | 272 | delegates to `get_config_paths`, swaps filename | UNCHANGED |
| `contradictory_callback_names` | 314–327 | iter `callback_rules`, per-rule enable∩disable | iter `rules.rules` (body logic same) |
| `pattern_is_empty_core` | 325 | `&Pattern -> bool` | UNCHANGED |
| `HostContext` | ~355 | `layer:Option<u8>`, `callback_ids:Vec<u8>`, `clear_board:bool`, `any_match:bool`; `#[derive(Debug,Clone,PartialEq)]` | UNCHANGED |
| `evaluate()` | 410–493 | TWO scans (layer first-match break@429; callback all-match@444); no-match short-circuit@466 (clear_board:false, C13); formula@483 | ONE pass over `rules.rules` |

## 2. The 4 invariants `evaluate()` MUST preserve (behavior-preserving promise)

1. **C13 no-match** (rules.rs:466–480): `matched_effective.is_empty()` ⇒
   `HostContext { layer:None, callback_ids:vec![], clear_board:false, any_match:false }`.
   The global `[host].disable_firmware_config` does NOT affect no-match windows.
   (Doc-comment @393–394 is STALE — says `clear_board: <[host].disable_firmware_config>`;
   the code already returns `false`. S1 fixes the COMMENT only.)
2. **Layer first-match-wins, EXCLUSIVE** — one host layer. New form: `if rule.layer.is_some()
   && layer.is_none() { layer = rule.layer }` inside the single loop. NO `break`.
3. **Disable-order-independence** (rules.rs:441–463): two `BTreeSet`s (`enabled`+`disabled`),
   `desired = enabled.difference(&disabled)` ONCE after the loop. `disable` always wins
   regardless of rule order. Regressed-guard: `test_evaluate_disable_before_enable_still_excludes` (#37).
4. **Stack-vs-replace**: `clear_board = matched_effective.iter().all(|&f|f) || !board_has_rules`.
   `matched_effective` has ONE entry per matched rule (per spec §8(3) "every matched RULE").

**Why byte-identical for old-schema inputs:** an old `[[layer_rules]]` ⇔ a `[[rule]]` with
`layer` set + empty enable/disable; an old `[[callback_rules]]` ⇔ a `[[rule]]` with
`layer=None` + enable/disable. Interleaving in one array + one pass yields the same layer
(first layer-setting match) and same callback sets (union enable − union disable). Relative
order of layer-only vs callback-only rules is irrelevant to the result.

## 3. Validity-mechanism change (the subtle part)

- **Old:** `layer: u8` (REQUIRED) ⇒ a `[[layer_rules]]` with only `match` fails
  *deserialization* (serde error). Tests #7 (`test_rules_missing_layer_errors`, uses
  `toml::from_str` directly) and #17 (`test_rules_parse_missing_required_field_errors`,
  uses `parse_rules`) assert `is_err()`.
- **New:** `layer: Option<u8>` (default None) ⇒ a `[[rule]]` with only `match` does NOT
  fail deserialization. It must fail the NEW check in `validate_rules`
  ("must set at least one of layer/enable/disable"). `parse_rules` still returns Err ⇒
  the `is_err()` boundary is preserved.
- **Impact on tests (S2's scope, NOT S1's):**
  - Test #7 (`test_rules_missing_layer_errors`) asserts a SERDE error via direct
    `toml::from_str::<RuleSet>`. After S1, this no longer errors (layer optional). S2
    must rewrite it to call `parse_rules` and assert the validity error.
  - Test #17 (`test_rules_parse_missing_required_field_errors`) asserts `parse_rules`
    `is_err()` — still holds (via `validate_rules`). S2 updates the TOML to `[[rule]]`.
- S1 implements `validate_rules` so the boundary is intact for S2 to assert against.

## 4. Test-suite classification for S2 (~46 tests, lines 494–1378)

> S1 does NOT touch these. This catalog is S2's porting guide. "Schema" = does it
> reference `[[layer_rules]]`/`[[callback_rules]]`/`.layer_rules`/`.callback_rules`/
> `LayerRule`/`CallbackRule`/`SECTION_9_TOML`.

### Deserialization / model (port to `[[rule]]` + `Rule`/`rules`)
- `test_rules_full_section9_example_parses` — uses `SECTION_9_TOML` (224/225). Port to
  unified `[[rule]]` + 10/11; assert `rs.rules` entries (pattern/layer/enable/disable/override).
- `test_rules_missing_host_table_defaults_false`, `test_rules_empty_toml_is_all_default`,
  `test_rules_default_propagates` — assert `rs.rules.is_empty()` + host default false.
- `test_rules_layer_override_absent_is_none` — becomes `rs.rules[0].disable_firmware_config == None`.
- `test_rules_callback_enable_disable_default_empty` — becomes two `[[rule]]` entries;
  assert `enable`/`disable` default empty.
- `test_rules_match_string_to_single_and_array_to_parts` — `[[rule]]` + assert `Pattern`.
- `test_rules_missing_layer_errors` (#7) — **MECHANISM CHANGE** (see §3): rewrite to
  `parse_rules` + validity error.
- `test_rules_missing_match_errors` — `match` is still required (`#[serde(rename="match")]`,
  not `default`); serde error still fires. Port TOML to `[[rule]]`.

### effective_disable_firmware_config (4 truth-table tests — UNCHANGED logic)
- `test_rules_effective_some_true_wins`, `_some_false_wins`, `_none_inherits_false`,
  `_none_inherits_true` — no schema refs; pass unchanged.

### parse_rules (file IO)
- `test_rules_parse_valid_section9` — `SECTION_9_TOML` → 10/11; assert `rs.rules`.
- `test_rules_parse_missing_file_errors`, `test_rules_parse_malformed_toml_errors` — no schema refs.
- `test_rules_parse_missing_required_field_errors` (#17) — `is_err()` still holds via
  `validate_rules`; port TOML to `[[rule]]`.
- `test_rules_paths_swap_filename`, `test_rules_paths_delegate_count` — no schema refs.

### evaluate() — parity tests (PRESERVE observable HostContext; port TOML to `[[rule]]`)
- No-match: `test_evaluate_empty_ruleset_no_match`, `test_evaluate_no_layer_no_callback_match`,
  `test_evaluate_no_match_clear_board_always_false` (C13 — keep verbatim expectation).
- Layer: `test_evaluate_layer_first_match_wins`, `test_evaluate_layer_second_when_first_misses`,
  `test_evaluate_layer_parts_requires_both_halves`.
- Callbacks: `test_evaluate_callback_all_matches_union`, `test_evaluate_callback_disable_is_exclusion`,
  `test_evaluate_unknown_name_skipped`, `test_evaluate_callback_ids_sorted`.
- clear_board truth table: `test_evaluate_clear_board_all_disabling`,
  `test_evaluate_clear_board_one_nondisabling_is_false`, `test_evaluate_clear_board_no_board_rules`,
  `test_evaluate_effective_inherits_host_default`.
- Cross-stage: `test_evaluate_layer_match_callback_miss`, `test_evaluate_callback_match_layer_miss`.
- Order-independence (#37): `test_evaluate_disable_after_enable_excludes`,
  `test_evaluate_disable_before_enable_still_excludes`, `test_evaluate_disable_excludes_only_named_others_survive`.

### validate_layers → validate_rules (layer validation)
- `test_parse_rules_rejects_layer_255_clear_sentinel` — still rejects; update TOML to `[[rule]]`.
- `test_parse_rules_accepts_low_layer_indices` — **KEEP the 0,28,100,224,254 literals**
  (it's a range test, not the §9 fixture). Port TOML to `[[rule]]`.
- `test_parse_rules_reports_first_bad_layer` — port TOML; still reports 255.

### NEW tests S2 should ADD (for the new validity check)
- `test_parse_rules_rejects_match_only_rule` — a `[[rule]]` with only `match` ⇒ `parse_rules`
  `is_err()`, message contains "at least one of layer/enable/disable".

### --validate-rules helpers
- `test_contradictory_callback_names_flags_same_rule_overlap`,
  `test_contradictory_callback_names_cross_rule_is_not_contradictory`,
  `test_contradictory_callback_names_deduped_sorted` — port TOML to `[[rule]]`; body unchanged.
- `test_pattern_is_empty_core_single`, `test_pattern_is_empty_core_parts` — no schema refs.

## 5. The non-compilation reality (why the gate isn't "build green")

After S1 alone:
- `cargo build` (no test modules) fails ONLY in **external callers**:
  `notifier.rs:572` (`for rule in &rules.callback_rules`), `main.rs:253/271/281/442`,
  `core/mod.rs:183/191-233`. **Zero errors in rules.rs non-test code.**
- `cargo test --no-run` ADDITIONALLY fails in **rules.rs `#[cfg(test)] mod tests`** (refs
  deleted structs) + external test code. **Still zero errors in rules.rs non-test code.**

So the S1 gate is: `grep -nE '\-\-> src/core/rules\.rs:[0-9]+' build.log` ⇒ no line with a
number `< ~494` (the `#[cfg(test)] mod tests` start). The authoritative `-->` pointer (not
notes/helps, which use `:`) is the precise check. The full green run returns only after
S2 (tests) + S3 (callers) land.

## 6. External callers S3 must fix (bounded set — NOT S1's scope)

| File:line | Code | Fix |
|---|---|---|
| `notifier.rs:519` | doc-comment `[[callback_rules]]` | → `[[rule]]` |
| `notifier.rs:572` | `for rule in &rules.callback_rules` (unknown_callback_names) | → `&rules.rules` |
| `notifier.rs:1908-1921` | test TOML `[[callback_rules]]` | → `[[rule]]` |
| `main.rs:241,253` | doc + `for rule in &rules.callback_rules` (collect_callback_names) | → `&rules.rules` |
| `main.rs:271,281` | two loops (layer_rules + callback_rules) in empty_pattern_warnings | two FILTERED passes over `rules.rules` (`rule.layer.is_some()` / `is_none()`) to preserve "layer rule #N"/"callback rule #N" text |
| `main.rs:442-443` | `rs.layer_rules.len()`, `rs.callback_rules.len()` (validate_rules summary) | `rs.rules.len()` or derived split counts |
| `main.rs:606,612` | test seeding `rules.layer_rules.push(LayerRule{…})` | `rules.rules.push(Rule{…})` |
| `main.rs:574,579,630,633,646,651,655` | test TOML `[[layer_rules]]`/`[[callback_rules]]` | → `[[rule]]` |
| `core/mod.rs:183` | doc `rs.layer_rules.is_empty() && rs.callback_rules.is_empty()` | `rs.rules.is_empty()` |
| `core/mod.rs:191-233` | `render_rules_body` template (2 `# [[layer_rules]]` + 2 `# [[callback_rules]]`) | four `# [[rule]]` blocks |
| `core/mod.rs:377-378,390-391` | test asserts `body.contains("[[layer_rules]]")` etc. | single `body.contains("[[rule]]")` |
| `pattern.rs:1090` | doc-comment prose `[layer_rules] / [callback_rules]` | → `[rule]` (Pattern enum unchanged) |

## 7. Source of truth confirmation

`spec/HOST_RULES.md` §9 (lines 458–518) and §8(3) (368–376) are ALREADY at the unified
`[[rule]]` / `struct Rule` / `rename="rule"` / "One pass over `[[rule]]`" / "Validity"
wording. The spec was updated ahead of the code. S1 copies FROM the spec; it does NOT
edit the spec. The `system_context.md` architecture doc (§1–§5) is the validated blast
radius and invariant reference.
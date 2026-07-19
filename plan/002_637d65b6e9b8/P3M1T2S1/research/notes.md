# Research Notes — P3.M1.T2.S1: `evaluate()` + `HostContext`

> Pure-functions research. The deliverable is `src/core/rules.rs` additions:
> `pub struct HostContext` + `pub fn evaluate(...)`. No file-IO, no CLI, no
> notifier wiring. This task is the **evaluation engine** that consumes the S1
> structs + S2 `effective_disable_firmware_config` primitive + P2.M1.T3.S2
> `match_pattern`, and is consumed downstream by P4.M3.T1.S1.

## §0 — UPSTREAM STATE (verified by reading rules.rs as of this research)

BOTH predecessors have landed in `src/core/rules.rs` (581 lines):

- **S1 structs (lines 67–185):** `RuleSet`, `HostDefaults`, `LayerRule`,
  `CallbackRule` — all `pub` with `pub` fields. Exact shapes:
  - `RuleSet { host: HostDefaults, layer_rules: Vec<LayerRule>, callback_rules: Vec<CallbackRule> }` (lines 67–79)
  - `HostDefaults { disable_firmware_config: bool }` + `impl Default`→false (lines 93–108)
  - `LayerRule { pattern: Pattern, layer: u8, case_sensitive: bool, disable_firmware_config: Option<bool> }` (lines 129–145)
  - `CallbackRule { pattern: Pattern, enable: Vec<String>, disable: Vec<String>, case_sensitive: bool, disable_firmware_config: Option<bool> }` (lines 166–185)
- **S2 functions (lines 201–252):**
  - `fn effective_disable_firmware_config(rule_override: Option<bool>, host_default: bool) -> bool` (line 201, **PRIVATE**, body `rule_override.unwrap_or(host_default)`)
  - `pub fn parse_rules(path: &Path) -> Result<RuleSet, Box<dyn Error>>` (line 229)
  - `pub fn get_rules_paths() -> Vec<PathBuf>` (line 247)
- **Existing imports (lines 24–29):** `use std::error::Error; use std::fs; use std::path::{Path, PathBuf}; use crate::core::pattern::Pattern; use serde::Deserialize;`
- **Test module:** `#[cfg(test)] mod tests` begins at line 254, ends line 581. Contains S1's ~9 `test_rules_*` tests + S2's ~10 `test_rules_effective/parse/paths_*` tests.

**INSERTION POINT** for HostContext + evaluate(): **between line 252** (closing
`}` of `get_rules_paths`) **and line 254** (`#[cfg(test)]`). One blank line
separates them currently (line 253). Add the struct + fn there; append tests
inside the existing `mod tests` block (after the last S2 test, before the closing
`}` at line 581).

## §1 — `match_pattern` CONTRACT (the matcher evaluate() calls — pattern.rs:1158)

```rust
pub fn match_pattern(
    pattern: &Pattern,
    app_class: &str,
    title: &str,
    case_sensitive: bool,
) -> bool
```

Semantics (pattern.rs:1166–1182):
- `Pattern::Single(p)` → `pattern_match(p, app_class, case_sensitive)` — **title
  is NOT consulted** (firmware parity: a class-only rule never matches on title).
- `Pattern::Parts(c, t)` → `pattern_match(c, app_class, cs) && pattern_match(t, title, cs)`
  — **both halves must match**.

`evaluate()` calls it as `match_pattern(&rule.pattern, app_class, title, rule.case_sensitive)`.
The import to ADD: change line 28 `use crate::core::pattern::Pattern;` →
`use crate::core::pattern::{match_pattern, Pattern};` (add `match_pattern` to the
existing brace group — one edit, not a new line).

`case_sensitive` is PER-RULE (`rule.case_sensitive`, defaults `false`). It is NOT
a global or a parameter to evaluate() — each rule carries its own. ✓ (verified:
both LayerRule and CallbackRule have `case_sensitive: bool`.)

## §2 — THE `clear_board` DESIGN DECISION (resolved — READ THIS)

The item CONTRACT says:
> "If matched → clear_board = all matched rules have effective
> disable_firmware_config == true (AND board_has_rules is considered: string is
> sent iff board_has_rules AND NOT all-disabling)."

There are two readings of how `board_has_rules` interacts with `clear_board`:

- **Reading A (literal formula only):** `clear_board = all_matched_rules_disabling`.
  board_has_rules affects ONLY the downstream string-send decision. Problem: then
  board_has_rules changes NO field of HostContext (dead parameter).
- **Reading B (folds board_has_rules in):** `clear_board = all_matched_rules_disabling || !board_has_rules`.

**RESOLUTION — use Reading B.** Justification:

1. **§4 (Architecture) defines the stack/replace bit as the clear_board value:**
   - "Replace (all matched rules disabling, **OR board has no rules**): send only
     APPLY_HOST_CONTEXT{..., clear_board: true}"
   - "Stack (board has rules AND ≥1 matched rule non-disabling): ... clear_board: false"
   These are the EXACT two terms of Reading B: replace = `all_disabling || !board_has_rules`.

2. **The parenthetical is the algebraic complement of clear_board under Reading B.**
   Under Reading B: `!clear_board = !(all_disabling || !board_has_rules) = (!all_disabling) && board_has_rules`.
   The contract parenthetical says "string is sent iff **board_has_rules AND NOT all-disabling**" —
   that is **exactly `!clear_board`** (for the matched case). So Reading B makes the
   parenthetical a tautology of `!clear_board`, which is the clean, self-consistent
   reading. Under Reading A the parenthetical is an unrelated downstream fact and
   board_has_rules is a dead parameter.

3. **Makes board_has_rules meaningful** (no dead param, no clippy dead_code warning).

So:
```rust
let clear_board = all_matched_effective_disabling || !board_has_rules;
```
where `all_matched_effective_disabling = matched_flags.iter().all(|&f| f)` (vacuously
empty → handled by the no-match early-return, which sets clear_board=false).

**NO-MATCH case** (contract-explicit special case, overrides the formula):
`HostContext { layer: None, callback_ids: vec![], clear_board: false, any_match: false }`.
Note: under the formula, zero matched rules makes `all()` vacuously true → clear_board
would be true, which CONTRADICTS the contract's no-match=false. So the no-match case
MUST short-circuit before the formula. Rationale (semantic, from §8(4)): no-match means
"host steps aside, board runs its own rules untouched" → clear_board=false (don't touch
the board); only the host layer + host callbacks are cleared.

## §3 — `any_match` SEMANTICS

`any_match = true` iff **at least one rule matched** (a layer_rule OR a callback_rule).
NOT "desired callback set non-empty" (a rule may match yet contribute only unknown
names, or only disables). So track `any_match` independently: set a flag whenever
`match_pattern` succeeds on any layer_rule or callback_rule. Equivalently,
`any_match = !matched_effective_flags.is_empty()` (the flags vec gets one entry per
matched rule). The no-match early-return hardcodes `any_match: false`.

## §4 — DESIRED CALLBACK SET: `BTreeSet<u8>` (deterministic)

- Add enable names → insert resolved id; remove disable names → remove resolved id.
- Resolve via `name_to_id.get(name)` → `Option<&u8>`. Unknown name (None) → **skip
  silently** (see §5). 
- **Use `std::collections::BTreeSet<u8>`** (NOT HashSet): the default hasher randomizes
  per-process, so `HashSet` → non-deterministic `callback_ids` order → flaky tests +
  unstable wire bytes. `BTreeSet` → sorted, deterministic `.into_iter().collect::<Vec<u8>>()`.
  Import: `use std::collections::BTreeSet;`.
- Final `callback_ids: desired.into_iter().collect()` → sorted `Vec<u8>`.

`HashMap<String, u8>` is the type of the `name_to_id` PARAMETER → must also import it:
`use std::collections::HashMap;`. Combined: `use std::collections::{BTreeSet, HashMap};`.

## §5 — UNKNOWN CALLBACK NAMES: skip silently (warn is the handshake's job)

§8(5) handshake: "validate rules.toml names against name_to_id // warn, don't fail".
The WARN + validation is P4.M2.T1's job (it has the full name table). `evaluate()` is a
**pure function** (no side effects, easy to unit-test) — it must NOT fail or log on an
unknown name; it simply skips it (`if let Some(&id) = name_to_id.get(name) { desired.insert(id); }`).
Rationale: a rule may legitimately reference a callback name that a different keyboard
firmware doesn't define; the host degrades gracefully (that name contributes nothing).
Logging a `log::warn!` per call would make evaluate() impure and spammy on every window
change. Leave warning to the handshake/`--validate-rules` layer (P4.M2/P5.M1).

## §6 — TEST PLAN (~14 tests, all in the existing `mod tests` block)

Build rulesets IN-TEST via `toml::from_str::<RuleSet>(TOML).unwrap()` (mirrors S1's
idiom) OR construct `RuleSet` literals. `name_to_id` built via
`[("name".to_string(), id), ...].into_iter().collect::<HashMap<_,_>>()`.

1. `test_evaluate_empty_ruleset_no_match` — empty RuleSet → `{None, [], false, false}`.
2. `test_evaluate_layer_first_match_wins` — 2 layer rules, both would match; first wins
   (layer=first.layer; second never consulted — verify by giving the 2nd a different layer).
3. `test_evaluate_layer_second_when_first_misses` — first pattern misses, second matches.
4. `test_evaluate_layer_none_match` — no layer rule matches → layer=None.
5. `test_evaluate_callback_all_matches_union` — 2 callback rules both match; enables union.
6. `test_evaluate_callback_disable_is_exclusion` — rule A enables "x", rule B disables "x"
   (both match) → x absent from desired set.
7. `test_evaluate_unknown_name_skipped` — enable references a name not in name_to_id →
   skipped, no panic, other names still resolve.
8. `test_evaluate_clear_board_all_disabling` — every matched rule effective=true → clear_board=true.
9. `test_evaluate_clear_board_one_nondisabling_is_false` — one matched rule effective=false → clear_board=false (stack), even with board_has_rules=true.
10. `test_evaluate_clear_board_no_board_rules` — board_has_rules=false → clear_board=true (replace).
11. `test_evaluate_effective_inherits_host_default` — rule.disable_firmware_config=None,
    host.disable_firmware_config=false → effective=false → (sole matched rule) clear_board=false;
    flip host=true → effective=true → clear_board=true.
12. `test_evaluate_callback_ids_sorted` — insert ids {3,1,2} → callback_ids == vec![1,2,3] (BTreeSet).
13. `test_evaluate_layer_match_callback_miss` — layer matches, no callback matches → layer set, callback_ids empty, any_match=true.
14. `test_evaluate_callback_match_layer_miss` — no layer match, callback matches → layer None, callbacks set, any_match=true, clear_board per matched rule flags.

All tests: `cargo test --bin qmkonnect rules::tests::test_evaluate -- --test-threads=1` (single-threaded crate-wide, AGENTS.md).

## §7 — IMPORTS TO ADD (2 edits)

1. Line 28: `use crate::core::pattern::Pattern;` → `use crate::core::pattern::{match_pattern, Pattern};`
2. NEW line after line 26 (group with std imports): `use std::collections::{BTreeSet, HashMap};`
   (Place it among the `use std::...` group, before the `use crate::...` / `use serde::...` lines — matches the file's existing std-first ordering.)

NO Cargo.toml edit (std collections; log already a dep but unused here).

## §8 — SCOPE WALL (what NOT to do)

- Do NOT touch the S1 structs or S2 functions (additive — only ADD HostContext + evaluate + tests).
- Do NOT add a `send_string` field to HostContext (the contract pins exactly 4 fields; the
  string decision is downstream P4.M3.T1.S1 from `board_has_rules && any_match && !clear_board`).
- Do NOT wire evaluate() into notifier.rs / debounce / CLI (that's P4.M3 / P5).
- Do NOT call parse_rules / get_rules_paths from evaluate (evaluate takes a borrowed `&RuleSet`).
- Do NOT validate/normalize callback names beyond `name_to_id.get` (warn is P4.M2's job).
- Do NOT implement the no-match "keep" option (withdrawn by §3 C8 / item contract — always clear).
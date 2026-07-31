# Research Notes — P1.M1.T1.S3: Update all external code callers + tests to unified `[[rule]]`

## Task nature

This subtask **restores compilation** after S1's struct change. S1 deleted
`LayerRule`/`CallbackRule`/`RuleSet.layer_rules`/`RuleSet.callback_rules` and
introduced the unified `RuleSet.rules: Vec<Rule>` (SINGULAR serde rename `"rule"`).
Every EXTERNAL caller (outside `rules.rs`) that referenced the split fields is now
broken. S3 fixes those 4 files + their tests.

## CRITICAL naming reconciliation (scout artifact is WRONG on this)

The scout artifact `callers_research.md` (in `.pi-subagents/artifacts/outputs/55dbc9e6/...`)
uses **`[[rules]]` (PLURAL)** throughout its prose recommendations. That is WRONG —
it was written BEFORE the naming decision was locked. The actual locked design is:

- **`[[rule]]` is SINGULAR** — serde `rename = "rule"` (NOT `"rules"`).
- Confirmed by: S1's LANDED code (`rules.rs:82` `#[serde(default, rename = "rule")]`),
  `spec/HOST_RULES.md` §9 (line 447 `# one [[rule]] per`), the S1/S2 PRPs, and the
  item description itself ("four '# [[rule]]' blocks").

**Every TOML literal + doc-comment must use `[[rule]]` (singular).** Ignore the
scout artifact's `[[rules]]` mentions — they are pre-decision prose.

## S1 contract (what S3 compiles against — LANDED, verified)

`src/core/rules.rs` now defines (verified by grep):
- `RuleSet` (line 74): `host: HostDefaults`, `rules: Vec<Rule>` with
  `#[serde(default, rename = "rule")]` (SINGULAR). NO `layer_rules`/`callback_rules`.
- `Rule` (line 139): `pattern: Pattern` (`rename="match"`), `layer: Option<u8>`
  (`default`, **NOT required u8**), `enable: Vec<String>`, `disable: Vec<String>`,
  `case_sensitive: bool`, `disable_firmware_config: Option<bool>`.
- `LayerRule` + `CallbackRule`: **DELETED**.
- `Rule` and its fields are `pub` → constructible as `Rule { pattern, layer, enable, disable, case_sensitive, disable_firmware_config }`.

## The 8 current compile errors (verified: `cargo check --bin qmkonnect --offline`)

All in `notifier.rs` + `main.rs` (NON-TEST code). 6 root + 2 cascade:

| # | File:line | Error | Root/cascade | Fix |
|---|-----------|-------|--------------|-----|
| 1 | notifier.rs:572 | E0609 no field `callback_rules` | ROOT | `&rules.callback_rules` → `&rules.rules` |
| 2 | notifier.rs:574 | E0282 type annotations needed | CASCADE of #1 | resolves when #1 fixed |
| 3 | main.rs:253 | E0609 no field `callback_rules` | ROOT | `&rules.callback_rules` → `&rules.rules` |
| 4 | main.rs:255 | E0282 type annotations needed | CASCADE of #3 | resolves when #3 fixed |
| 5 | main.rs:271 | E0609 no field `layer_rules` | ROOT | filtered pass (see below) |
| 6 | main.rs:281 | E0609 no field `callback_rules` | ROOT | filtered pass (see below) |
| 7 | main.rs:442 | E0609 no field `layer_rules` | ROOT | derived count or single len |
| 8 | main.rs:443 | E0609 no field `callback_rules` | ROOT | derived count or single len |

**KEY INSIGHT:** `cargo check` (non-test) only catches errors in notifier.rs + main.rs.
The `mod.rs` (render_rules_body template string + test asserts) and `pattern.rs`
(doc-comment) fixes are NOT caught by `cargo check` because:
- mod.rs:183 is in a `///` doc-comment (ignored by compiler).
- mod.rs:191-233 is a STRING LITERAL (compiles fine; it's just text).
- mod.rs:377-378/390-391 are in `#[cfg(test)]` (cargo check skips tests).
- pattern.rs:1090 is in a `///` doc-comment.

BUT mod.rs + pattern.rs MUST still be fixed for: (a) the grep gate (`render_rules_body`
string contains `[[layer_rules]]`/`[[callback_rules]]`), (b) `cargo test` (mod.rs
test asserts reference the split schema), (c) correctness (the template is the
production string `qmkonnect -c` writes for users — it must say `[[rule]]`).

## The 4 files + every site (verified against actual source)

### src/core/notifier.rs
- **:519** doc-comment prose: `` `[[callback_rules]]` `` → `` `[[rule]]` ``.
- **:572** loop in `unknown_callback_names`: `for rule in &rules.callback_rules` → `&rules.rules`. Inner body UNCHANGED (`rule.enable.iter().chain(rule.disable.iter())` — Rule has those fields).
- **:1908-1921** test `test_unknown_callback_names_helper`: TOML `[[callback_rules]]` → `[[rule]]`. (Rule has enable/disable → passes validate_rules, BUT this test calls `unknown_callback_names` directly, not parse_rules, so validity isn't even checked. Just needs to deserialize. The rule has match+enable+disable so it's fine.)

### src/main.rs
- **:229** doc-comment prose: `callback_rules[].enable` + `callback_rules[].disable` → `rule[].enable` + `rule[].disable`.
- **:241** doc-example TOML (inside collect_callback_names `///`): `[[callback_rules]]` → `[[rule]]`.
- **:253** loop in `collect_callback_names`: `for rule in &rules.callback_rules` → `&rules.rules`. Inner body unchanged.
- **:271/281** `empty_pattern_warnings` TWO loops → **two filtered passes over `rules.rules`** (preserve "layer rule #N"/"callback rule #N" text + test asserts @624-625):
  - loop 1: `rules.rules.iter().enumerate().filter(|(_, r)| r.layer.is_some())` → "layer rule #N" (N = 1-based rank among layer rules)
  - loop 2: `rules.rules.iter().enumerate().filter(|(_, r)| r.layer.is_none())` → "callback rule #N"
  - OR: keep two separate enumerate loops, each filtering. The numbering must stay per-type.
- **:442-443** validate_rules summary: `rs.layer_rules.len()` / `rs.callback_rules.len()`. RECOMMENDATION: derive split counts via filter (preserves user-facing info): `rs.rules.iter().filter(|r| r.layer.is_some()).count()` + `.filter(|r| r.layer.is_none()).count()`. Alternative: single `rs.rules.len()` (simpler; no test asserts this text). Derive-split is lower-risk for UX.
- **:574,579** test TOML (`test_collect_callback_names_dedupes`): `[[callback_rules]]` ×2 → `[[rule]]`.
- **:592** test comment "no callback_rules" → "no rules".
- **:606** test struct push `LayerRule { pattern, layer: 224, ... }` → `Rule { pattern, layer: Some(224), enable: vec![], disable: vec![], case_sensitive: false, disable_firmware_config: None }`. **CRITICAL: layer u8 → Some(u8).**
- **:612** test struct push `CallbackRule { pattern: Parts("*",""), enable: vec![], disable: vec![], ... }` → `Rule { pattern: Parts("*",""), layer: None, enable: vec![], disable: vec![], case_sensitive: false, disable_firmware_config: None }`.
- **:630,633** test TOML (`test_empty_pattern_warnings_silent_for_real_patterns`): `[[layer_rules]]`/`[[callback_rules]]` → `[[rule]]`.
- **:646,651,655** test TOML (`test_contradictory_callback_warnings_flags_same_rule_overlap`): `[[callback_rules]]` ×3 → `[[rule]]`.

### src/core/mod.rs
- **:183** doc-example assertion: `rs.layer_rules.is_empty() && rs.callback_rules.is_empty()` → `rs.rules.is_empty()`.
- **:191-233** `render_rules_body` template string: swap the 4 `# [[layer_rules]]`/`# [[callback_rules]]` headers → `# [[rule]]`; merge the two comment dividers ("Layer rules: FIRST match wins..."/"Callback rules: ALL matches fire...") into ONE unified description (mirror spec §9 lines 449-453). Keep all 4 rule CONTENTS byte-identical (match/layer/enable/disable values). Keep the [host] block + intro unchanged.
- **:377-378** test asserts: `body.contains("[[layer_rules]]")` + `body.contains("[[callback_rules]]")` → single `body.contains("[[rule]]")`.
- **:390-391** test asserts: `rs.layer_rules.is_empty()` + `rs.callback_rules.is_empty()` → single `rs.rules.is_empty()`.

### src/core/pattern.rs
- **:1090** doc-comment prose: `` `[layer_rules]` / `[callback_rules]` `` → `` `[rule]` ``. Pattern enum UNCHANGED.

## Validation gating model (parallel with S2)

- **S3's OWN gate (needs only S1 + S3):** `cargo check --bin qmkonnect --offline` → 0 errors. (`cargo check` skips `#[cfg(test)]`, so it doesn't need S2's rules.rs test rewrite.) ALSO: `grep -rn 'layer_rules|callback_rules|LayerRule|CallbackRule' src/core/notifier.rs src/main.rs src/core/mod.rs src/core/pattern.rs` → 0 hits (the 4 S3-owned files).
- **The COMBINED gate (needs S2 + S3):** `cargo test --bin qmkonnect -- --test-threads=1` → all pass. (Compiles test modules incl. rules.rs's — which S2 rewrites.) AND `grep -rn 'layer_rules|callback_rules|LayerRule|CallbackRule' src/` → 0 hits (whole src/; rules.rs test module is S2's scope).
- If S3 runs BEFORE S2 lands: `cargo check` passes, but `cargo test` fails to compile (rules.rs test module stale). That's EXPECTED — document it; the test green-gate is the S2+S3 combined result. If S2 has ALREADY landed: both pass.

## list_callbacks (main.rs:318) is UNTOUCHED

Verified: it queries the firmware callback registry (`CALLBACK_NAMES`/`callback_names()`), NOT the rules structs. No split-schema access. Do not touch it.

## Sources verified
- S1 PRP + landed rules.rs (unified Rule/RuleSet confirmed).
- S2 PRP (the rules.rs test rewrite contract — S3 must not touch rules.rs).
- Scout artifact `callers_research.md` (quoted code; note its `[[rules]]` plural is WRONG).
- system_context.md §3 blast radius + §4 risks + §5 dependency model.
- spec/HOST_RULES.md §8(3) (line 370-376) + §9 (437-518) — the template/comment source of truth.
- Actual source: notifier.rs (505-589, 1895-1934), main.rs (225-294, 430-469, 560-665), mod.rs (175-249, 370-400), pattern.rs (1084-1095).
- `cargo check --bin qmkonnect --offline` (8 errors localized).
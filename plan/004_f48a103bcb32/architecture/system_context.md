# Architecture — Host-Rules Schema Unification (C8)

> **Session 004.** Single focused refactor: collapse the split
> `[[layer_rules]]`/`[[callback_rules]]` model into ONE `[[rule]]` array in the
> `qmkonnect` host app. The wire contract (`ApplyHostContext`), the
> `qmk-notifier` crate, and the `qmk_notifier` firmware are **untouched**.

---

## 1. Validated current state (CONFIRMED by codebase research)

### 1a. The split model — `src/core/rules.rs`

| Item | Location | Current definition |
|------|----------|--------------------|
| `RuleSet` | `rules.rs:70-83` | `host: HostDefaults`, `layer_rules: Vec<LayerRule>` (`rename="layer_rules"`), `callback_rules: Vec<CallbackRule>` (`rename="callback_rules"`) |
| `LayerRule` | `rules.rs:126` | `pattern: Pattern` (`rename="match"`), `layer: u8` (REQUIRED), `case_sensitive: bool`, `disable_firmware_config: Option<bool>` |
| `CallbackRule` | `rules.rs:165` | `pattern: Pattern`, `enable: Vec<String>`, `disable: Vec<String>`, `case_sensitive: bool`, `disable_firmware_config: Option<bool>` |
| `evaluate()` | `rules.rs:410-493` | **Two scans**: stage 1 `for rule in &rules.layer_rules` (first-match-wins, breaks @429); stage 2 `for rule in &rules.callback_rules` (all-match @444); no-match short-circuit @466 returns `clear_board:false` (C13); stack-vs-replace @483 |
| `validate_layers` | `rules.rs:253` | iterates `rules.layer_rules`, rejects `layer == 0xFF` |
| `contradictory_callback_names` | `rules.rs:314-316` | iterates `rules.callback_rules` |
| `effective_disable_firmware_config` | `rules.rs:230` | pure helper `Option<bool> → bool` (unchanged by refactor) |
| `pattern_is_empty_core` | `rules.rs:325` | pure `&Pattern → bool` (unchanged) |

### 1b. Behaviors that are ALREADY CORRECT and must be PRESERVED exactly

- **C13 no-match** (`rules.rs:466-480`): `matched_effective.is_empty()` ⇒ `HostContext { layer:None, callback_ids:vec![], clear_board:false, any_match:false }`. The global `[host].disable_firmware_config` does NOT affect no-match windows. The doc-comment at `rules.rs:393-394` is STALE (says `clear_board: <[host].disable_firmware_config>` — the code already returns `false`).
- **C11 0xFF rejection** (`validate_layers`): only `255` is rejected; no reserved-range floor.
- **Disable-order-independence** (`rules.rs:441-463`): two `BTreeSet`s (`enabled` + `disabled`) differenced ONCE at the end so `disable` always wins regardless of rule order. The regression test `test_evaluate_disable_before_enable_still_excludes` (#37) guards this.
- **D4 "Edit rules" tray item**: already in `tray.rs` + `linux_tray.rs`. Not touched.

---

## 2. The target spec — `spec/HOST_RULES.md` §9 + §8(3) (source of truth, ALREADY updated)

### 2a. Unified Rust model (`spec/HOST_RULES.md:485-502`)

```rust
#[derive(Debug, Deserialize, Default)]
pub struct RuleSet {
    #[serde(default)] pub host: HostDefaults,
    #[serde(default, rename = "rule")] pub rules: Vec<Rule>,   // <-- ONE array
}

#[derive(Debug, Deserialize)]
pub struct Rule {
    #[serde(rename = "match")] pub pattern: Pattern,           // required
    #[serde(default)] pub layer: Option<u8>,                    // None => no layer
    #[serde(default)] pub enable: Vec<String>,
    #[serde(default)] pub disable: Vec<String>,
    #[serde(default)] pub case_sensitive: bool,
    #[serde(default)] pub disable_firmware_config: Option<bool>, // None => inherit [host]
}
```

> **CRITICAL — TOML key is `[[rule]]` (SINGULAR)**, because `rename = "rule"`.
> NOT `[[rules]]`. Every TOML literal and doc must use `[[rule]]`.

### 2b. Unified TOML schema (`spec/HOST_RULES.md:458-481`)

```toml
[[rule]]
match = "alacritty"
layer = 10                          # small indices (NOT 224)
disable_firmware_config = true

[[rule]]
match = ["*chrome*", "*youtube*"]
layer = 11

[[rule]]
match = "neovide"
enable = ["vim_lazy", "disable_vim"]
disable = ["vim_lazy"]

[[rule]]
match = ["*chrome*", "*claude*"]
enable = ["vim_lazy", "disable_vim"]
disable_firmware_config = true
```

### 2c. Parse-time validity (`spec/HOST_RULES.md:514-518`)

> Every `[[rule]]` must set at least one of `layer`, `enable`, or `disable` (in
> addition to the required `match`); a rule that sets none is a parse error.
> `layer == 255` is likewise rejected.

### 2d. One-pass evaluation semantics (`spec/HOST_RULES.md:370-376`, §8(3))

> **One pass over `[[rule]]`** (file order). For each matching rule: if it sets
> `layer` and none is chosen yet ⇒ first-match-wins (one host layer — exclusive);
> its `enable`/`disable` accumulate into the callback sets (all-match). A rule
> may set `layer` only, callbacks only, or both.

desired enabled id set = `union(enable) − union(disable)` across all matching rules.
Stack-vs-replace: replace iff every matched rule's effective `disable_firmware_config` is `true`.

---

## 3. Complete blast radius (CONFIRMED by grep — repo-wide)

### 3a. `src/core/rules.rs` (the model + evaluator + tests)

- **Structs** (69-171), **evaluate** (423/444 loops), **validate_layers** (254),
  **contradictory_callback_names** (316), doc-comments (33,46,51,58,63,105,111,115,120,146,154,159,209,235,307,346,379,393-394).
- **Test suite** (~46 tests, lines 494-1376): 40 reference the split schema.
  `SECTION_9_TOML` constant (499-521) uses `224`/`225` → target `10`/`11`.
  ~20 `evaluate()` parity tests must PRESERVE observable `HostContext` output.

### 3b. `src/core/notifier.rs` (caller)

- **Line 519**: doc-comment prose `[[callback_rules]]`.
- **Line 572**: `for rule in &rules.callback_rules` in `unknown_callback_names()` — the callback-name validation loop. Body unchanged (`rule.enable.iter().chain(rule.disable.iter())`).
- **Line 1908-1921**: test `test_unknown_callback_names_helper` embeds `[[callback_rules]]` TOML.

### 3c. `src/main.rs` (CLI callers — 6 sites)

- **`collect_callback_names`** (253): `for rule in &rules.callback_rules`. Doc-example (241) embeds `[[callback_rules]]`.
- **`empty_pattern_warnings`** (271/281): TWO loops over `layer_rules` + `callback_rules`. RECOMMENDATION: keep two filtered passes over `rules.rules` (`rule.layer.is_some()` / `is_none()`) to preserve the "layer rule #1"/"callback rule #1" warning text + existing test assertions.
- **`validate_rules` summary** (442-443): `rs.layer_rules.len()`, `rs.callback_rules.len()`. Decide: single `rs.rules.len()` or derived split counts.
- **Test seeding** (606/612): `rules.layer_rules.push(LayerRule{…})` / `rules.callback_rules.push(CallbackRule{…})`.
- **Test TOML literals** (574,579,630,633,646,651,655): `[[layer_rules]]`/`[[callback_rules]]`.
- **`list_callbacks`** (318): NO split-schema access (firmware registry) — untouched.

### 3d. `src/core/mod.rs` (the SEEDED TEMPLATE — critical production caller)

> **⚠ FOUND BY RESEARCH — NOT in the PRD's enumerated blast radius.** This is the
> `render_rules_body()` function that `qmkonnect -c` writes to disk for users.

- **Lines 191-233**: the commented `rules.toml` template string with two `# [[layer_rules]]` + two `# [[callback_rules]]` blocks. Must become four `# [[rule]]` blocks.
- **Line 183**: doc-example assertion `rs.layer_rules.is_empty() && rs.callback_rules.is_empty()` → `rs.rules.is_empty()`.
- **Lines 377-378**: test asserts `body.contains("[[layer_rules]]")` + `body.contains("[[callback_rules]]")` → single `"[[rule]]"`.
- **Lines 390-391**: test asserts `rs.layer_rules.is_empty()` + `rs.callback_rules.is_empty()` → `rs.rules.is_empty()`.

### 3e. `src/core/pattern.rs` (doc-comment only)

- **Line 1090**: prose `rules.toml's [layer_rules] / [callback_rules]` → `[rule]`. The `Pattern` enum itself is unchanged.

### 3f. User-facing docs (`docs/*.md`)

| File | Lines | Content |
|------|-------|---------|
| `docs/configuration.md` | 265, 271-281, 309-332, 398 | schema table (2 table-arrays → 1), annotated TOML example, stack-vs-replace bullet |
| `docs/examples.md` | 294-318 | Example-4 TOML block (5 split headers → `[[rule]]`) |
| `docs/qmk-integration.md` | 211, 216, 230 | migration steps 2/3 + example header |
| `docs/troubleshooting.md` | 519-520 | parse-error "Fix" sentence |
| `docs/llms_full.txt` | (generated) | regenerated from the 4 above via `docs/generate_llms_full.sh` |

### 3g. Already-correct / NOT to be touched

- `spec/HOST_RULES.md` — the spec is ALREADY at target wording (references `[[rule]]`, `struct Rule`, `rename="rule"`, "One pass over `[[rule]]`", "Validity").
- `README.md` — mentions `rules.toml` as a one-line feature blurb only; no schema description.
- Wire path (`notifier.rs` send logic outside line 572), handshake, crate, firmware.

---

## 4. Key risks & invariants for the implementing agent

1. **`[[rule]]` is SINGULAR.** The serde rename is `rename = "rule"` (not `"rules"`). All TOML literals + docs must use `[[rule]]`.
2. **`evaluate()` is the highest-stakes change.** Collapsing two arrays into one means a single matching rule may contribute to BOTH layer-first-match AND callback-all-match. The no-match short-circuit (`matched_effective.is_empty()`) and the disable-order-independence (two-BTreeSet difference) MUST be preserved. Run tests #34/#35 (cross-stage) and #37 (order-independence) first after the rewrite.
3. **Validity mechanism change.** A `[[rule]]` with only `match` (no `layer`/`enable`/`disable`) no longer fails *deserialization* (since `layer` is now `Option`). It must fail the *validity* check inside `parse_rules`/`validate_rules`. Existing tests #7/#17 expected a serde error — they now expect a validity error (same `is_err()` boundary).
4. **`empty_pattern_warnings` numbering.** If collapsed to one pass, the "#N" numbering becomes ambiguous. Keep two filtered passes over `rules.rules` to preserve "layer rule #N" / "callback rule #N" text.
5. **`render_rules_body` (mod.rs) is a production caller.** Changing the struct breaks its tests (377-378). Must be updated in the same code change.
6. **Tests are single-threaded** (`cargo test --bin qmkonnect -- --test-threads=1`) per `AGENTS.md` (shared global debouncer).
7. **`layer = 224`/`225` → `10`/`11`** in the `SECTION_9_TOML` fixture + doc-comment examples, to match spec §9. But the range-acceptance test (#40, `test_parse_rules_accepts_low_layer_indices`) legitimately tests `0,28,100,224,254` — those literals STAY (it's a range test, not the §9 fixture).

---

## 5. Dependency / compile-ordering model

```
T1.S1 (rules.rs model + evaluate + validity)   ← defines the unified Rule/RuleSet
   ├── T1.S2 (rules.rs test suite)             ← depends on S1
   └── T1.S3 (external callers: notifier, main, mod, pattern) ← depends on S1
T2.S1 (docs/*.md rewrite)                       ← depends on T1 (code shape)
T2.S2 (regenerate llms_full.txt + verify)       ← depends on T2.S1
T3    (Mode B: changeset-level docs sweep)      ← depends on T1.S3 + T2.S2
```

`cargo test --bin qmkonnect -- --test-threads=1` passes only after T1.S1 + S2 + S3
all land (S1's struct change breaks every caller until fixed).

---

## Research artifacts (detailed scout outputs)

Full per-line findings are in the subagent artifact outputs:
- `.pi-subagents/artifacts/outputs/55dbc9e6/plan/004_f48a103bcb32/architecture/callers_research.md` — every caller site with quoted code.
- `.pi-subagents/artifacts/outputs/55dbc9e6/plan/004_f48a103bcb32/architecture/docs_research.md` — every doc reference with quoted text.
- `.pi-subagents/artifacts/outputs/55dbc9e6/plan/004_f48a103bcb32/architecture/tests_research.md` — all 46 tests classified.
# Delta PRD — Host-Rules Schema Unification (C8)

**Base:** v0.2.8 (session 003 snapshot) → **Target:** v0.2.8 (session 004 snapshot)
**Scope:** `qmkonnect` repo only (the wire contract is unchanged → `qmk-notifier` crate and `qmk_notifier` firmware are NOT touched).
**Size:** Medium — one focused refactor (unify the split `rules.toml` schema) + mechanical doc sync.

---

## 1. What Actually Changed (diff analysis)

A precise `diff` of `plan/003_*/prd_snapshot.md` → `plan/004_*/prd_snapshot.md` yields **295 changed lines**, all within the **Host-Side Window Rules (F11/F12)** design (`HOST_RULES.md`) plus mirror edits in `ARCHITECTURE.md` §5.7, `PROTOCOL.md` §8, and `UI.md` §1.1/§1.2/§2.3. The changes fall into four themes:

| # | Spec change | Where | Code status |
|---|---|---|---|
| **D1** | **C8 — unify `[[layer_rules]]`/`[[callback_rules]]` into ONE `[[rule]]` array** (`layer` becomes `Option<u8>`; a rule may set layer-only, callbacks-only, or both; parse-time validity: each rule must set ≥1 of `layer`/`enable`/`disable`) | `HOST_RULES.md` §3 C8, §8(3), §9 schema + Rust model, §10 migration, §11, §14; mirrors none | ❌ **NOT implemented** — `src/core/rules.rs` still defines `LayerRule`+`CallbackRule` with `rename = "layer_rules"`/`"callback_rules"` |
| D2 | **C11 — host layer is a raw QMK layer index** (withdraw the "≥ 224" reservation; only `255`/`0xFF` rejected as the clear sentinel; bounded by `layer_state_t` width) | `HOST_RULES.md` §3 C11, §5; `PROTOCOL.md` §8 | ✅ Already in code (`validate_layers` rejects 255; doc-comments say "raw index") |
| D3 | **C13 + C7 — independent silos**: host no-match clears the **host** layer/callbacks only (`clear_board: false`); the board silo always runs (string is sent even on host no-match) | `HOST_RULES.md` §3 C13/C7, §4 flow + semantics, §8(4); `ARCHITECTURE.md` §5.7 | ✅ Already in code (`evaluate()` no-match branch returns `clear_board: false`; `notifier.rs` C13 send logic) |
| D4 | **"Edit rules" tray item** replaces "Reload rules": hot-reload by re-parsing `rules.toml` on every window focus (no fs watch, no manual reload) + automatic desktop notification on parse failure | `HOST_RULES.md` §1, §3 C3, §8(7), §10, §11, §14; `UI.md` §1.1/§1.2/§2.3 | ✅ Already in code (`tray.rs` + `linux_tray.rs` have "Edit rules") |

**Only D1 (C8 schema unification) remains to be implemented.** D2/D3/D4 were landed in prior sessions and the code already conforms; they are noted here for awareness only and generate **no tasks**.

> **Why qmkonnect-only:** C8 reorganizes how rules are *stored in `rules.toml` and parsed by the host*. It does not change the wire payload — `evaluate()` still emits the same `HostContext { layer, callback_ids, clear_board, any_match }` and the same `APPLY_HOST_CONTEXT{[layer][flags][count][id…]}` command. The crate framing (§7) and firmware reception (§6) are byte-for-byte unaffected.

---

## 2. The Single Implementation Gap (D1)

**Current code** (`src/core/rules.rs`) uses the *split* schema:
```rust
pub struct RuleSet {
    pub host: HostDefaults,
    #[serde(rename = "layer_rules")]    pub layer_rules: Vec<LayerRule>,     // first-match-wins
    #[serde(rename = "callback_rules")] pub callback_rules: Vec<CallbackRule>, // all-match
}
pub struct LayerRule    { pattern, layer: u8,              case_sensitive, disable_firmware_config: Option<bool> }
pub struct CallbackRule { pattern, enable: Vec<String>, disable: Vec<String>, case_sensitive, disable_firmware_config: Option<bool> }
```
`evaluate()` does **two separate scans** (layer first-match, then callback all-match).

**Target spec** (`spec/HOST_RULES.md` §9 — already updated; `HOST_RULES.md` §3 C8, §8(3)) requires the **unified** schema:
```rust
pub struct RuleSet {
    pub host: HostDefaults,
    #[serde(rename = "rule")] pub rules: Vec<Rule>,
}
pub struct Rule {
    #[serde(rename = "match")] pub pattern: Pattern,
    #[serde(default)] pub layer: Option<u8>,                 // None ⇒ this rule sets no layer
    #[serde(default)] pub enable: Vec<String>,
    #[serde(default)] pub disable: Vec<String>,
    #[serde(default)] pub case_sensitive: bool,
    #[serde(default)] pub disable_firmware_config: Option<bool>,
}
```
with **one pass** over `rules`: the first matching rule with `layer.is_some()` wins (one host layer — exclusive); `enable`/`disable` accumulate across **all** matching rules (all-match); desired callback set = `union(enable) − union(disable)`. Parse-time **validity**: every `[[rule]]` must set ≥1 of `layer`/`enable`/`disable` (else a parse error), and `layer == 255` is rejected (existing `0xFF` rule, now over the unified `Rule.layer`).

**Blast radius (verified by grep):**
- `src/core/rules.rs` — the model, `evaluate()`, `validate_layers`→`validate_rules`, the `SECTION_9_TOML` test constant, the inline doc-comment examples (still show `layer = 224`; spec now uses `10`/`11`), and **~128** references to the split identifiers.
- Callers that iterate the split arrays: `src/core/notifier.rs:572` (`rules.callback_rules`), `src/main.rs:253,271,281` (`--list-callbacks`/`--validate-rules` listing) and `src/main.rs:606,612` (CLI seeding of `LayerRule`/`CallbackRule`).
- User-facing docs that document/示例 the split tables: `docs/configuration.md` (full schema table §271–278), `docs/examples.md` (§296–315), `docs/qmk-integration.md` (§211–230 migration + example), `docs/troubleshooting.md:520`.
- `docs/llms_full.txt` — generated concatenation, regenerated last.

**Already-correct and NOT to be touched:** the `evaluate()` no-match branch (returns `clear_board: false`, C13), the `0xFF` layer rejection (C11), the "Edit rules" tray item (D4), and `spec/HOST_RULES.md` (the spec doc is already at the target wording).

---

## 3. Backlog

### Phase P1 — Unify the host `rules.toml` schema (C8)

One focused refactor: collapse the split `[[layer_rules]]`/`[[callback_rules]]` model into a single `[[rule]]` array, port `evaluate()` to one-pass semantics, enforce the new parse-time validity, update all callers, and sync the user-facing docs. The wire contract, handshake, and crate/firmware are unchanged.

#### Milestone P1.M1 — Unify the `[[rule]]` schema in code and docs

**Task P1.M1.T1 — Port the rules model + evaluator to the unified `[[rule]]` array**

Merge `LayerRule`+`CallbackRule` into one `Rule`, change `RuleSet` to `rules: Vec<Rule>`, rewrite `evaluate()` as a single pass, and enforce parse-time validity (≥1 of layer/enable/disable; `layer != 255`). Then update every caller and fix the test suite.

- **P1.M1.T1.S1 — Unify the data model, evaluator, and parse-time validity in `src/core/rules.rs`** *(story points: 3)*
  - **CONTRACT:**
    1. **RESEARCH:** `src/core/rules.rs` (53 KB). The split model is at lines 68–230 (`RuleSet`, `LayerRule` @126, `CallbackRule` @165). `evaluate()` @410–493 does two scans (layer first-match @419–429, callback all-match @441–463, then no-match short-circuit @466–480 returning `clear_board:false`, then stack-vs-replace @483–491). `validate_layers` @253 iterates `rules.layer_rules` rejecting `layer==0xFF`. The target is `spec/HOST_RULES.md` §9 (lines ~440–520: `struct Rule { pattern, layer: Option<u8>, enable, disable, case_sensitive, disable_firmware_config }`, `RuleSet { host, #[serde(rename="rule")] rules }`) and §8(3) ("One pass over `[[rule]]`"). The no-match `clear_board:false` behavior (C13) and the `0xFF` rejection (C11) are **already correct in behavior** — preserve them exactly; only the *shape* changes.
    2. **INPUT:** `src/core/rules.rs`, `src/core/pattern.rs` (unchanged — the matcher is already full-parity), `spec/HOST_RULES.md` §9 (the Rust model + TOML schema + "Validity" paragraph).
    3. **LOGIC:**
       - Replace `LayerRule`+`CallbackRule` with a single `Rule { #[serde(rename="match")] pattern: Pattern, #[serde(default)] layer: Option<u8>, #[serde(default)] enable: Vec<String>, #[serde(default)] disable: Vec<String>, #[serde(default)] case_sensitive: bool, #[serde(default)] disable_firmware_config: Option<bool> }`.
       - `RuleSet`: replace the two `Vec`s with `#[serde(default, rename = "rule")] pub rules: Vec<Rule>`. Keep `host: HostDefaults` (unchanged).
       - Rewrite `evaluate()` to **one pass** over `rules`: for each rule where `match_pattern(...)` succeeds — push its effective `disable_firmware_config` to `matched_effective`; if `rule.layer.is_some() && layer.is_none()` set `layer = rule.layer` (first-match-wins, exclusive); accumulate `enable`/`disable` names into the enable/disable `BTreeSet`s (resolve via `name_to_id`, unknown names skipped — unchanged). After the loop, `desired = enabled − disabled` (unchanged two-set difference so `disable` is order-independent). **Preserve the no-match short-circuit verbatim** (empty `matched_effective` ⇒ `HostContext { layer:None, callback_ids:vec![], clear_board:false, any_match:false }` — C13). Then `clear_board = all_disabling || !board_has_rules`.
       - Rename `validate_layers`→`validate_rules`, iterate `rules.rules`, and extend it to reject a rule that sets **none** of `layer`/`enable`/`disable` (return a clear `Box<dyn Error>` message, same boundary as the `0xFF` rejection), plus the existing `layer == Some(0xFF)` check.
       - Update the `host_context_for_window`/`evaluate` **doc-comment** ("Three-stage evaluation"): rewrite stage 1/2 to describe the single pass over `[[rule]]`; **fix the stale no-match line** (`rules.rs:393–394` currently says `clear_board: <[host].disable_firmware_config>` — the code already returns `clear_board: false`; make the comment match C13).
       - Update inline doc-comment TOML examples that still show `layer = 224` (e.g. `rules.rs:48,117,130`) to small indices (the spec §9 example uses `10`/`11`) — pure cosmetic consistency with C11.
    4. **OUTPUT:** Updated `src/core/rules.rs`. `evaluate()`'s observable output (`HostContext`) is unchanged for any input that was expressible before, so the notifier send logic is unaffected.
    5. **DOCS:** *Mode A — none separate.* `spec/HOST_RULES.md` is already at the target wording; the code doc-comments are updated as part of this work (step 3). User-facing `docs/*.md` are handled in T2.

- **P1.M1.T1.S2 — Update callers and the test suite** *(story points: 2; depends on S1)*
  - **CONTRACT:**
    1. **RESEARCH:** Callers of the split fields (grep-verified): `src/core/notifier.rs:572` (`for rule in &rules.callback_rules`), `src/main.rs:253` (`rules.callback_rules`), `src/main.rs:271` (`rules.layer_rules.iter().enumerate()`), `src/main.rs:281` (`rules.callback_rules`), `src/main.rs:606` (`rules.layer_rules.push(LayerRule{…})`) and `src/main.rs:612` (`rules.callback_rules.push(CallbackRule{…})`). Tests: the `SECTION_9_TOML` constant (~`rules.rs:455`, uses `[[layer_rules]]`/`[[callback_rules]]` with `layer = 224`/`225`) and ~14 `evaluate()` tests + ~128 total split-schema references in `rules.rs`.
    2. **INPUT:** The unified `Rule`/`RuleSet` from S1.
    3. **LOGIC:**
       - `src/core/notifier.rs:572`: iterate `rules.rules` instead of `rules.callback_rules` (the callback-name validation loop — adapt to read `rule.enable`/`rule.disable` off the unified `Rule`).
       - `src/main.rs` `--list-callbacks`/`--validate-rules` listing (lines ~253, 271, 281): iterate `rules.rules`; each rule can now carry both a `layer` and callbacks, so the listing should show a rule's `layer` (if `Some`) **and** its `enable`/`disable` names together (one row per `[[rule]]`, not two separate tables).
       - `src/main.rs` CLI seeding (lines ~606, 612): the seed/template writer should emit the unified `[[rule]]` form (match the `spec/HOST_RULES.md` §9 commented template). If it currently pushes a synthetic `LayerRule`+`CallbackRule`, push `Rule` entries instead (or emit the template string directly).
       - Tests: rewrite `SECTION_9_TOML` to the **unified** `[[rule]]` form with `layer = 10`/`11` (verbatim from `spec/HOST_RULES.md` §9). Update the ~14 `evaluate()` tests to construct `RuleSet` with `rules: vec![Rule{…}]`. Add/adjust tests for the new validity rule: a `[[rule]]` with only `match` (no `layer`/`enable`/`disable`) ⇒ `parse_rules` errors; `layer = 255` ⇒ errors (already tested — keep). Keep the existing first-match-wins (layer) and all-match (callbacks) parity tests; they still hold under one pass.
    4. **OUTPUT:** Compiling, passing `cargo test --bin qmkonnect -- --test-threads=1`.
    5. **DOCS:** *Mode A — none separate.*

**Task P1.M1.T2 — Sync the user-facing docs to the unified `[[rule]]` schema**

Mechanical rewrite of every doc that shows or describes the split `[[layer_rules]]`/`[[callback_rules]]` tables, then regenerate the LLM concatenation. These are Mode-A doc updates that ride with the schema change; because they span four files and a regeneration, they are one focused task.

- **P1.M1.T2.S1 — Rewrite the schema in `docs/{configuration,examples,qmk-integration,troubleshooting}.md`** *(story points: 2; depends on P1.M1.T1)*
  - **CONTRACT:**
    1. **RESEARCH:** Verified references: `docs/configuration.md:271–278` (a full schema table with `[[layer_rules]]`/`[[callback_rules]]` rows — `match`/`layer`/`case_sensitive`/`disable_firmware_config` for layers, `match`/`enable`/`disable` for callbacks); `docs/examples.md:296–315` (two `[[layer_rules]]` + two `[[callback_rules]]` blocks); `docs/qmk-integration.md:211–230` (migration steps 2/3 + a `[[callback_rules]]` example); `docs/troubleshooting.md:520` ("every `[[layer_rules]]` entry **requires** `match` and `layer`"). The canonical target wording is `spec/HOST_RULES.md` §9 (TOML + Rust model + "Validity" paragraph) — already correct; mirror it.
    2. **INPUT:** `spec/HOST_RULES.md` §9 (source of truth), the unified `Rule` model from P1.M1.T1.
    3. **LOGIC:**
       - `docs/configuration.md:271–278`: replace the two table-array blocks with **one** `[[rule]]` table-array documenting `match` (required), `layer` (optional, raw QMK index, `!=255`), `enable`/`disable` (optional name lists), `case_sensitive` (default `false`), `disable_firmware_config` (optional override). Add the validity note: a rule must set ≥1 of `layer`/`enable`/`disable`. Keep the C11 wording already present (raw index, `layer_state_t` width).
       - `docs/examples.md:296–315`: rewrite the four blocks as four `[[rule]]` entries — layer-only rules keep `layer`; callback rules keep `enable`/`disable`; where the old example showed a layer rule and a callback rule for the same app, they may be merged into one `[[rule]]` (optional — match the spec §9 example shape).
       - `docs/qmk-integration.md:211–230`: migration step 2 → "add a `[[rule]]` entry with a `layer` field"; step 3 → "add a `[[rule]]` entry with `enable`/`disable`"; rewrite the example to `[[rule]]`.
       - `docs/troubleshooting.md:520`: "every `[[rule]]` entry **requires** `match` and at least one of `layer`/`enable`/`disable`".
    4. **OUTPUT:** Four updated doc files; no remaining `[[layer_rules]]`/`[[callback_rules]]` references in `docs/`.
    5. **DOCS:** *Mode A — this IS the documentation work.*

- **P1.M1.T2.S2 — Regenerate `docs/llms_full.txt` and verify the tree** *(story points: 0.5; depends on S1)*
  - **CONTRACT:**
    1. **RESEARCH:** `docs/llms_full.txt` is a committed generated concatenation of `README.md`+`docs/*.md` (built by `docs/generate_llms_full.sh`, stripping Jekyll front matter). It mirrors the doc sources verbatim, so it currently still contains the split-schema text. (Pattern established in session 003 P1.M1.T1.S2.)
    2. **INPUT:** The four fixed doc files from S1.
    3. **LOGIC:** Run `bash docs/generate_llms_full.sh` from the repo root. Verify: `grep -rn '\[\[layer_rules\]\]\|\[\[callback_rules\]\]' docs/ src/` returns **zero** hits (excluding `target/`, `plan/`, `.pi-subagents/`). `grep -rn '\[\[rule\]\]' docs/llms_full.txt` returns the expected hits. Run `cargo test --bin qmkonnect -- --test-threads=1` and `cargo check --bin qmkonnect --offline` — both clean.
    4. **OUTPUT:** Regenerated `docs/llms_full.txt`; verification report (grep counts + test/check status).
    5. **DOCS:** *Mode A — this IS the regeneration.*

---

## 4. Documentation Impact Summary

- **Mode A (doc-with-work):** the schema change touches `src/core/rules.rs` doc-comments (folded into P1.M1.T1.S1) and the four user-facing `docs/*.md` (P1.M1.T2). `spec/HOST_RULES.md` — the spec source of truth — is **already** at the target wording (verified: it references `[[rule]]`, `struct Rule`, `rename = "rule"`, "One pass over `[[rule]]`", "Validity"); no edit needed.
- **Mode B (changeset-level):** **none.** `README.md` mentions `rules.toml` only as a one-line feature blurb ("edit a `rules.toml` file" — version-agnostic, already accurate); it does not describe the schema and needs no change. There is no top-level capability list or architecture overview that references the split tables.

---

## 5. Notes for the Breakdown Agent

- **Do NOT re-implement D2/D3/D4** — they are already in code (C11 `0xFF` rejection, C13 no-match `clear_board:false`, "Edit rules" tray). Only D1 (C8 unification) generates work.
- **Preserve observable behavior:** `evaluate()` must return the *same* `HostContext` for any input expressible in the old schema. The refactor is a re-shaping of storage/parsing, not a semantic change to the wire path. The notifier's stack/replace/no-match send logic (`src/core/notifier.rs`) is unchanged in behavior — only the `rules.callback_rules` iteration site at line 572 needs to read from `rules.rules`.
- **Tests are single-threaded** (`--test-threads=1`, shared global debouncer) per `AGENTS.md`.
- **No crate/firmware coordination:** the `qmk-notifier` git-tag pin (`Cargo.toml`) and the `qmk_notifier` firmware are untouched — C8 is host-side file format only.
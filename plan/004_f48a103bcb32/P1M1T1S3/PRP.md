# PRP — P1.M1.T1.S3: Update all external code callers + tests to the unified `[[rule]]` schema

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`).
> **Scope:** The **4 caller files** outside `rules.rs` that reference S1's deleted
> split schema (`layer_rules`/`callback_rules`/`LayerRule`/`CallbackRule`):
> `src/core/notifier.rs`, `src/main.rs`, `src/core/mod.rs`, `src/core/pattern.rs`
> — their production code, doc-comments, and tests. This subtask **RESTORES
> COMPILATION** after S1's struct change. It does NOT touch `src/core/rules.rs`
> (S1's non-test code + S2's test module).
> **⚠ `[[rule]]` is SINGULAR.** The scout artifact `callers_research.md` uses
> `[[rules]]` (plural) in its prose — that is STALE, pre-decision wording.
> Every TOML literal + doc must use `[[rule]]` (serde `rename = "rule"`).

---

## Goal

**Feature Goal**: Migrate every external caller of the host-rules split schema
to S1's unified `RuleSet.rules: Vec<Rule>` model, so the crate compiles cleanly
and the full test suite passes — with the user-facing seeded `rules.toml`
template (`render_rules_body`) and all doc-comments showing the unified `[[rule]]`
schema verbatim from `spec/HOST_RULES.md` §9.

**Deliverable**: Edited `src/core/notifier.rs`, `src/main.rs`, `src/core/mod.rs`,
`src/core/pattern.rs` (production loops + struct construction + doc-comments +
test TOML literals + test assertions + the `render_rules_body` template string).
No new files. No `rules.rs` changes.

**Success Definition**:
- `cargo check --bin qmkonnect --offline` → **0 errors, 0 warnings** (restores
  compilation of non-test code — S3's primary gate; needs only S1 + S3).
- `cargo test --bin qmkonnect -- --test-threads=1` → **all tests pass**
  (the combined S2+S3 gate; needs S2's rules.rs test rewrite too — see Validation).
- `grep -rn 'layer_rules\|callback_rules\|LayerRule\|CallbackRule' src/` → **0 hits**
  in the whole `src/` tree (once S2 + S3 both land). S3's per-file gate: 0 hits in
  its 4 owned files.
- The seeded `rules.toml` template (`render_rules_body`) contains `[[rule]]`
  (singular) ×4 and parses to an all-default `RuleSet` (inert on fresh install).

## User Persona (if applicable)

**Target User**: (1) End users who run `qmkonnect -c` (or use "Edit rules") and
get a seeded `rules.toml` — they must see the unified `[[rule]]` schema. (2)
Maintainers of `--validate-rules` (P5.M1) and the send pipeline (P4.M3) who call
`collect_callback_names`/`unknown_callback_names`/`evaluate` against the unified model.

**Use Case**: A user edits `rules.toml` with one flat `[[rule]]` array; the host
validates it (`--validate-rules`) and evaluates it (one pass). The CLI summary and
empty-pattern warnings must reflect the unified shape without misleading text.

**Pain Points Addressed**: S1 collapsed `LayerRule`+`CallbackRule` into one `Rule`,
deleting `.layer_rules`/`.callback_rules`. Every external caller now fails to
compile (8 errors in `cargo check`). S3 fixes them so the build is green again and
the user-facing template matches the spec.

## Why

- **Restores compilation (the immediate blocker).** S1's struct change is atomic
  and correct, but it breaks every caller until S3 lands. The 8 `cargo check`
  errors (6 root `E0609` field-not-found + 2 cascade `E0282`) are all in
  `notifier.rs`/`main.rs`; `cargo check` succeeds once those 2 files are fixed.
  But S3 must ALSO fix `mod.rs` (template string + tests) and `pattern.rs`
  (doc-comment) — not for `cargo check`, but for the grep gate, `cargo test`, and
  correctness (the template is what users see).
- **Unblocks the test suite + downstream.** `cargo test` compiles `mod.rs`'s tests
  (which assert on the template's section markers); until S3 fixes them, the test
  binary won't build (combined with S2's rules.rs test rewrite).
- **Behavior-preserving.** The migration is purely a schema collapse: a
  `[[callback_rules]]` rule ⇔ a `[[rule]]` with `layer: None` + enable/disable; a
  `[[layer_rules]]` rule ⇔ a `[[rule]]` with `layer: Some(_)` + empty enable/disable.
  No `evaluate()`/`HostContext` behavior changes (S1 already preserved those).
- **Production template correctness.** `render_rules_body` (mod.rs:191-233) is the
  commented template `qmkonnect -c` writes to disk. It currently shows the OLD split
  schema — users would copy `[[layer_rules]]`/`[[callback_rules]]` and be confused.
  S3 rewrites it to the verbatim spec §9 four-`[[rule]]` shape.

## What

A set of mechanical edits across 4 files, plus one template rewrite and one
loop-structure decision. Every site is enumerated below with exact line numbers
(verified against the current source) and before→after.

### Success Criteria

- [ ] `cargo check --bin qmkonnect --offline` → 0 errors, 0 warnings.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` → all pass (needs S2 too).
- [ ] `grep -rn 'layer_rules\|callback_rules\|LayerRule\|CallbackRule' src/core/notifier.rs src/main.rs src/core/mod.rs src/core/pattern.rs` → 0 hits.
- [ ] All TOML literals + doc-comments in the 4 files use `[[rule]]` (SINGULAR), never `[[rules]]`.
- [ ] `empty_pattern_warnings` preserves "layer rule #N" / "callback rule #N" text (test @624-625 passes).
- [ ] `render_rules_body` template has 4 `# [[rule]]` blocks, parses to all-default `RuleSet`, and contains `[[rule]]`.
- [ ] Test seeding uses `Rule { layer: Some(...)/None, ... }` (layer is `Option<u8>`).
- [ ] No file other than the 4 listed is modified. `rules.rs` untouched.

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed
> to implement this successfully?"_ — **Yes.** The S1 contract (exact unified
> structs, SINGULAR rename, `layer: Option<u8>`), every site with exact line number
> + before/after, the `empty_pattern_warnings` filtered-pass decision, the verbatim
> target `render_rules_body` template body, the 8 localized compile errors (root vs
> cascade), the validation gating model (S3's `cargo check` gate vs the combined
> S2+S3 `cargo test` gate), and the `[[rule]]`-SINGULAR correction are all below.

> **BASELINE ALERT.** S1 has LANDED (unified `Rule`/`RuleSet` verified in rules.rs).
> S2 (rules.rs test rewrite) is being implemented IN PARALLEL. At S3 time:
> `cargo check` fails with 8 errors (all in notifier.rs/main.rs); `cargo test`
> additionally fails on rules.rs's stale test module (S2's scope) + mod.rs's stale
> test asserts (S3's scope). S3's entry point is exactly those broken caller files.

### Documentation & References

```yaml
# MUST READ — the S1 contract (the unified model S3 compiles against)
- file: /home/dustin/projects/qmkonnect/plan/004_f48a103bcb32/P1M1T1S1/PRP.md
  why: "Defines the exact unified structs: RuleSet.rules: Vec<Rule> (rename=\"rule\"
        SINGULAR); Rule.layer: Option<u8> (NOT required u8); enable/disable/case_sensitive/
        disable_firmware_config defaults; LayerRule/CallbackRule DELETED. Every S3 edit
        compiles against EXACTLY this surface."
  section: "What (a) structs"
  critical: ".layer_rules/.callback_rules are GONE -> every loop/field-access must become
             .rules. layer is Option<u8> -> struct-construction layer values must be Some(_).
             Rule + its fields are pub -> Rule { pattern, layer, enable, disable, ... } works."

# MUST READ — the S2 contract (the rules.rs test rewrite — S3 must NOT touch rules.rs)
- file: /home/dustin/projects/qmkonnect/plan/004_f48a103bcb32/P1M1T1S2/PRP.md
  why: "S2 owns the rules.rs #[cfg(test)] mod tests block. S3 must NOT edit rules.rs (would
        steal S2's work + break the task boundary). S3's grep/test gates account for S2's
        parallel scope: S3's per-file gate is the 4 caller files; the full-tree gate is S2+S3."
  section: "Scope" + "Validation Loop" (the non-compilation gating model)
  critical: "If S2 has NOT landed when S3 runs: cargo check PASSES (S3 fixed non-test callers)
             but cargo test fails to compile (rules.rs test module stale). That is EXPECTED;
             the test green-gate is the S2+S3 combined result. Do NOT edit rules.rs to chase it."

# MUST READ — the canonical spec (the template + comment wording source of truth)
- file: /home/dustin/projects/qmkonnect/spec/HOST_RULES.md
  why: "§9 (lines 437-518) = the verbatim [[rule]] TOML schema + Rust model + Validity that
        render_rules_body must mirror. §8(3) (lines 370-376) = the one-pass evaluation prose
        for the template's comment divider. Copy FROM this file; do NOT edit it."
  section: "8(3) Per-window evaluation" (370-376) + "9. rules.toml Schema Reference" (437-518)
  critical: "[[rule]] is SINGULAR (rename=\"rule\"). The 4-block §9 example (layer 10/11,
            neovide callbacks, chrome+claude) is the exact shape for render_rules_body."

# MUST READ — the architecture blast radius + invariants + dependency model
- file: /home/dustin/projects/qmkonnect/plan/004_f48a103bcb32/architecture/system_context.md
  why: "§3 enumerates the COMPLETE blast radius (every caller site with line numbers — matches
        the item description). §4 lists risks (esp. #4 empty_pattern_warnings numbering, #5
        render_rules_body is a production caller). §5 is the S1->S2->S3 compile-ordering model."
  section: "3. Complete blast radius" + "4. Key risks" + "5. Dependency model"
  critical: "render_rules_body (mod.rs) was FOUND BY RESEARCH, not in the original PRD blast
             radius — it IS production code (qmkonnect -c seeds it). Must be updated. list_callbacks
             (main.rs:318) is UNTOUCHED (firmware registry, not rules structs)."

# MUST READ — the detailed per-site scout research (quoted code + line numbers)
- file: /home/dustin/projects/qmkonnect/.pi-subagents/artifacts/outputs/55dbc9e6/plan/004_f48a103bcb32/architecture/callers_research.md
  why: "Quoted before/after code for every one of the ~16 sites across the 4 files, with
        rationale for each. The authoritative site-by-site reference."
  section: "1. notifier.rs", "2. main.rs", "3. mod.rs", "4. pattern.rs", "Summary table"
  critical: "WARNING: this artifact's prose uses [[rules]] (PLURAL) — that is WRONG. The locked
             design is [[rule]] (SINGULAR). Ignore the plural mentions; trust the S1 landed code
             + spec §9 + the item description (all singular)."

# MUST READ — the files being edited (read current code before editing)
- file: /home/dustin/projects/qmkonnect/src/core/notifier.rs
  why: "Sites: :519 doc-comment, :572 unknown_callback_names loop, :1908-1921 test TOML.
        Read 505-589 (doc+fn) + 1895-1934 (test) to confirm exact text before editing."
  pattern: "unknown_callback_names iterates callback_rules -> enable/disable union against the
            live CALLBACK_NAMES registry (HashMap<String,u8>). Inner body unchanged by unification."
  gotcha: "The test at 1908-1921 constructs RuleSet via toml::from_str then calls unknown_callback_names
           directly (NOT parse_rules) — so validate_rules isn't invoked; the [[rule]] just needs to
           deserialize (it has match+enable+disable, so it does)."

- file: /home/dustin/projects/qmkonnect/src/main.rs
  why: "Sites: :229/:241 doc-comment, :253 collect_callback_names loop, :271/:281 empty_pattern_warnings
        TWO loops, :442-443 validate_rules summary, :606/:612 test struct push, :574/579/630/633/646/651/655
        test TOML literals. Read 225-294 + 430-469 + 560-665."
  pattern: "collect_callback_names mirrors notifier::unknown_callback_names but returns the UNION
            regardless of the live registry. empty_pattern_warnings is a pure lint helper (#9 footgun)."
  gotcha: "empty_pattern_warnings test (@600-625) asserts ws[0].contains(\"layer rule #1\") +
           ws[1].contains(\"callback rule #1\") — so the filtered-pass approach MUST keep per-type
           1-based numbering. test seeding (@606) LayerRule.layer was bare u8 224 -> must become Some(224)."

- file: /home/dustin/projects/qmkonnect/src/core/mod.rs
  why: "Sites: :183 doc-example, :191-233 render_rules_body template STRING, :377-378/:390-391 test
        asserts. Read 175-249 + 370-400."
  pattern: "render_rules_body returns a commented (every line prefixed '# ') rules.toml template as
            a raw string. It's PRODUCTION: qmkonnect -c + 'Edit rules' tray item seed it to disk."
  gotcha: "The template currently has TWO comment dividers ('Layer rules: FIRST match wins...'/
           'Callback rules: ALL matches fire...') + 2x [[layer_rules]] + 2x [[callback_rules]]. Rewrite
           to ONE unified divider + 4x [[rule]] (see Implementation Blueprint for the verbatim target)."

- file: /home/dustin/projects/qmkonnect/src/core/pattern.rs
  why: "Site: :1090 doc-comment prose on the Pattern enum. Read 1084-1095."
  pattern: "Pattern enum (Single/Parts, serde untagged) is UNCHANGED by unification — only the
            doc-comment's stale '[layer_rules] / [callback_rules]' reference needs fixing."
  gotcha: "One-word prose fix. Don't touch the enum or match_pattern."
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/                                  # THIS repo
├── src/core/
│   ├── rules.rs        # S1 (LANDED, non-test) + S2 (test module, parallel). DO NOT EDIT in S3.
│   ├── notifier.rs     # <-- EDIT (sites a: :519 doc, :572 loop, :1908-1921 test)
│   ├── mod.rs          # <-- EDIT (sites g-k: :183 doc, :191-233 template, :377-378/:390-391 tests)
│   ├── pattern.rs      # <-- EDIT (site k: :1090 doc-comment)
│   └── types.rs        # WindowInfo (unchanged)
├── src/main.rs         # <-- EDIT (sites b-f: :229/:241/:253/:271/:281/:442-443/:574/579/592/606/612/630/633/646/651/655)
├── spec/HOST_RULES.md  # §8(3)+§9 SOURCE OF TRUTH (already [[rule]]; copy FROM it, don't edit)
└── plan/004_f48a103bcb32/
    ├── architecture/system_context.md                 # blast radius + risks + dependency model
    └── P1M1T1S1/PRP.md, P1M1T1S2/PRP.md               # the S1/S2 contracts
```

### Desired Codebase tree with files to be modified

```bash
src/core/
├── notifier.rs   # MODIFIED — :519 doc, :572 loop, :1908-1921 test TOML
├── mod.rs        # MODIFIED — :183 doc-example, :191-233 template, :377-378/:390-391 tests
└── pattern.rs    # MODIFIED — :1090 doc-comment (one-word prose)
src/
└── main.rs       # MODIFIED — :229/:241/:253/:271/:281/:442-443 + test sites
```

> No new files. `rules.rs` is NOT edited (S1 non-test + S2 test module).

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL: [[rule]] is SINGULAR (serde rename = "rule"), NOT [[rules]].
//   The scout artifact callers_research.md uses [[rules]] (plural) in its prose — that is
//   STALE pre-decision wording. The locked design (S1 landed code, spec §9, S1/S2 PRPs,
//   item description) is SINGULAR [[rule]]. Every TOML literal + doc must use [[rule]].
//   A plural [[rules]] key would silently parse to an EMPTY rules vec (unknown field under
//   #[serde(default)]) and tests would pass for the wrong reason.

// CRITICAL: layer is Option<u8>, so struct-construction layer values must be Some(_).
//   Test seeding (main.rs:606) LayerRule { layer: 224 } -> Rule { layer: Some(224) }.
//   A bare `layer: 224` is a type-mismatch compile error (E0308). The 2 E0282 "type
//   annotations needed" cascade errors (notifier.rs:574, main.rs:255) resolve AUTOMATICALLY
//   once the loop field-access (572/253) is fixed — don't chase them separately.

// CRITICAL: cargo check (non-test) only catches 6 root errors in notifier.rs + main.rs.
//   mod.rs (template STRING + test asserts) and pattern.rs (doc-comment) are NOT caught by
//   cargo check (strings/comments don't compile; tests are skipped). You MUST still fix them
//   for: (a) the grep gate (template string contains the stale tokens), (b) cargo test
//   (mod.rs test asserts reference the split schema), (c) correctness (the template is what
//   users see). Don't skip mod.rs/pattern.rs just because cargo check is green.

// CRITICAL: empty_pattern_warnings must keep per-type 1-based numbering.
//   The test (main.rs:624-625) asserts ws[0].contains("layer rule #1") + ws[1].contains
//   ("callback rule #1"). Use TWO filtered passes over rules.rules (rule.layer.is_some() /
//   rule.layer.is_none()), each with its own enumerate+1 counter. A single pass with a
//   unified index would break the test AND confuse users.

// CRITICAL: render_rules_body is PRODUCTION code (a string qmkonnect -c writes for users).
//   Its content is user-visible documentation of the schema. It must show [[rule]] (singular)
//   x4, mirror spec §9, and STILL parse to an all-default RuleSet (every line commented '# ').
//   The test_render_rules_body_parses_to_default_ruleset test (mod.rs:385-394) verifies the
//   inert-parse property — keep every active line commented.

// NOTE: list_callbacks (main.rs:318) is UNTOUCHED.
//   It queries the firmware callback registry (CALLBACK_NAMES/callback_names()), NOT the
//   rules structs. No split-schema access. "callback" there = firmware callbacks, not rules.

// NOTE: tests are single-threaded (cargo test --bin qmkonnect -- --test-threads=1) per AGENTS.md
//   (shared global debouncer state). Parallel runs flake.

// NOTE: the notifier.rs test (1908-1921) calls unknown_callback_names DIRECTLY (not parse_rules).
//   So validate_rules is NOT invoked on its RuleSet. The [[rule]] just needs to deserialize
//   (it has match+enable+disable -> fine). No validity concern there.

// NOTE: Rule + its fields are pub, so Rule { pattern, layer, enable, disable, case_sensitive,
//   disable_firmware_config } is constructible in test code without any new constructor.
```

## Implementation Blueprint

### Data models and structure

No new data models — S3 consumes S1's unified `Rule`/`RuleSet` as-is. The "structure"
is the 4 files' caller sites, each adapted to `.rules` + `Option<u8>` layer + `Rule{...}`.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: READ current state + the contracts (anchor every edit)
  - READ: src/core/rules.rs non-test code (confirm Rule/RuleSet exact field names/types,
          Option<u8> layer, pub fields). DO NOT edit rules.rs.
  - READ: the 4 caller files at the line ranges cited in Documentation & References.
  - READ: spec/HOST_RULES.md §8(3) (370-376) + §9 (437-518) — the template/comment source.
  - READ: plan/004/.../P1M1T1S1/PRP.md (S1 contract) + system_context.md §3-§5.
  - GOAL: know every site's exact before/after and why.

Task 2: FIX src/core/notifier.rs (3 sites)
  - :519 doc-comment: `[[callback_rules]]` -> `[[rule]]`.
  - :572 loop: `for rule in &rules.callback_rules` -> `for rule in &rules.rules`. Body UNCHANGED.
  - :1908-1921 test TOML: `[[callback_rules]]` -> `[[rule]]` (the rule has match+enable+disable).
  - NOTE: the :574 E0282 cascade resolves automatically once :572 is fixed.

Task 3: FIX src/core/pattern.rs (1 site)
  - :1090 doc-comment: `` `[layer_rules]` / `[callback_rules]` `` -> `` `[rule]` ``.
  - Pattern enum + match_pattern UNCHANGED.

Task 4: FIX src/main.rs production callers (5 sites)
  - :229 doc-comment prose: `callback_rules[].enable` + `callback_rules[].disable`
          -> `rule[].enable` + `rule[].disable`.
  - :241 doc-example TOML (inside ///): `[[callback_rules]]` -> `[[rule]]`.
  - :253 collect_callback_names loop: `for rule in &rules.callback_rules` -> `&rules.rules`. Body unchanged.
          (The :255 E0282 cascade resolves automatically.)
  - :271/:281 empty_pattern_warnings: replace the TWO loops with TWO filtered passes:
        // pass 1 (layer rules):
        let mut layer_n = 0;
        for rule in &rules.rules {
            if rule.layer.is_some() {
                layer_n += 1;
                if pattern_is_empty_core(&rule.pattern) {
                    out.push(format!("⚠  layer rule #{} has an empty `match` pattern ...", layer_n));
                }
            }
        }
        // pass 2 (callback rules):
        let mut cb_n = 0;
        for rule in &rules.rules {
            if rule.layer.is_none() {
                cb_n += 1;
                if pattern_is_empty_core(&rule.pattern) {
                    out.push(format!("⚠  callback rule #{} has an empty `match` pattern ...", cb_n));
                }
            }
        }
    (Preserve the exact warning MESSAGE text — only the loop structure + numbering changes.
     Keep the full multi-line format! string verbatim from the original.)
  - :442-443 validate_rules summary: derive split counts via filter (preserves user-facing info):
        let layer_count = rs.rules.iter().filter(|r| r.layer.is_some()).count();
        let cb_count    = rs.rules.iter().filter(|r| r.layer.is_none()).count();
        println!("rules.toml valid: {} layer rules, {} callback rules.", layer_count, cb_count);
    (Alternative acceptable form: single `rs.rules.len()` -> "rules.toml valid: {} rules." —
     no test asserts this text, so either is fine. Derive-split preserves the richer output.)

Task 5: FIX src/main.rs tests (struct seeding + TOML literals)
  - :606 LayerRule push -> Rule { pattern: Pattern::Single("".into()), layer: Some(224),
          enable: vec![], disable: vec![], case_sensitive: false, disable_firmware_config: None }.
          (CRITICAL: layer 224 -> Some(224).)
  - :612 CallbackRule push -> Rule { pattern: Pattern::Parts("*".into(), "".into()), layer: None,
          enable: vec![], disable: vec![], case_sensitive: false, disable_firmware_config: None }.
  - :592 test comment "no callback_rules" -> "no rules".
  - :574, :579 test TOML (test_collect_callback_names_dedupes): `[[callback_rules]]` x2 -> `[[rule]]`.
  - :630, :633 test TOML (test_empty_pattern_warnings_silent_for_real_patterns):
          `[[layer_rules]]` + `[[callback_rules]]` -> `[[rule]]` x2.
  - :646, :651, :655 test TOML (test_contradictory_callback_warnings_flags_same_rule_overlap):
          `[[callback_rules]]` x3 -> `[[rule]]`.
  - NOTE: the assertions in these tests (collect order, ws.len()==2, ws[0]/ws[1] substrings,
          contradictory ws.len()==1) are UNCHANGED — only the TOML keys + struct construction change.

Task 6: FIX src/core/mod.rs (4 sites — incl. the PRODUCTION template)
  - :183 doc-example assertion: `rs.layer_rules.is_empty() && rs.callback_rules.is_empty()`
          -> `rs.rules.is_empty()`.
  - :191-233 render_rules_body template STRING: rewrite the rule section. Replace the TWO comment
          dividers + 2x [[layer_rules]] + 2x [[callback_rules]] with ONE unified divider + 4x [[rule]].
          Use the VERBATIM target below (Implementation Patterns). Keep the [host] block + intro
          + all 4 rule CONTENTS (match/layer/enable/disable values) byte-identical.
  - :377-378 test asserts: `body.contains("[[layer_rules]]")` + `body.contains("[[callback_rules]]")`
          -> single `body.contains("[[rule]]")`.
  - :390-391 test asserts: `rs.layer_rules.is_empty()` + `rs.callback_rules.is_empty()`
          -> single `rs.rules.is_empty()`. (The rs.host.disable_firmware_config assert is UNCHANGED.)

Task 7: VALIDATE
  - RUN: cargo check --bin qmkonnect --offline  -> 0 errors, 0 warnings. (S3's primary gate.)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1  (needs S2 too; see Validation Loop).
  - GREP: grep -rn 'layer_rules\|callback_rules\|LayerRule\|CallbackRule' src/core/notifier.rs
          src/main.rs src/core/mod.rs src/core/pattern.rs  -> 0 hits (S3's per-file gate).
  - GREP: grep -rn '\[\[rules\]\]' src/  -> 0 hits (no plural mistake anywhere).
```

### Implementation Patterns & Key Details

```rust
// === THE render_rules_body TEMPLATE TARGET (the largest single edit) ===
// Replace the rule-section of the template string (from "Layer rules:..." to the
// final "# disable_firmware_config = true ... board can't match" line) with this
// VERBATIM block. Keep the intro/header/[host] block above it UNCHANGED. Every
// line stays commented with '# ' (inert-parse property preserved).
//
//   # Rules: one [[rule]] per (app × behavior). For each matching rule, `layer` is
//   # first-match-wins (one host layer active — exclusive); `enable`/`disable`
//   # accumulate across ALL matches (all-match). A rule MUST set at least one of
//   # `layer` / `enable` / `disable`; it may set layer only, callbacks only, or both.
//   # `layer` is a RAW QMK layer index (no reserved range): must be != 255 (the
//   # wire "clear" sentinel) and fit your layer_state width (<=15 default, <=31
//   # with LAYER_STATE_32BIT); pick one above your highest board layer so it wins.
//   # Patterns use shell-style globs: `*` is a wildcard, `^`/`$` anchor. A
//   # catch-all is `match = "*"` — an empty `match = ""` matches ONLY windows
//   # whose class is empty, not every window.
//   # [[rule]]
//   # match = "alacritty"                       # class-only pattern
//   # layer = 10
//   # disable_firmware_config = true           # optional override (inherits [host])
//   #
//   # [[rule]]
//   # match = ["*chrome*", "*youtube*"]         # [class_pattern, title_pattern]
//   # layer = 11
//   # case_sensitive = false                    # optional, default false
//   #
//   # [[rule]]
//   # match = "neovide"
//   # enable = ["vim_lazy", "disable_vim"]      # run on focus-in
//   # disable = ["vim_lazy"]                    # optional: force-off override
//   #
//   # [[rule]]
//   # match = ["*chrome*", "*claude*"]
//   # enable = ["vim_lazy", "disable_vim"]
//   # disable_firmware_config = true           # skip the string -> board can't match
//
// RATIONALE: the divider prose mirrors spec §9 lines 449-453 (the "one [[rule]] per..."
// paragraph) + §8(3). The 4 rule contents are byte-identical to the current template
// (alacritty/layer10, chrome+youtube/layer11, neovide/enable+disable, chrome+claude/enable).
// Only the headers ([[layer_rules]]/[[callback_rules]] -> [[rule]]) + the merged divider change.
// The "Callback names come from --list-callbacks" note is already in the intro (line ~198),
// so dropping it from the divider is fine (no info loss).


// === THE MECHANICAL LOOP SWAP (notifier.rs:572, main.rs:253) ===
//   BEFORE:  for rule in &rules.callback_rules {
//   AFTER:   for rule in &rules.rules {
//   The inner body (rule.enable.iter().chain(rule.disable.iter())) is UNCHANGED —
//   Rule has .enable/.disable (same as the old CallbackRule). A layer-only rule
//   (layer:Some, empty enable/disable) contributes nothing here — the chain yields
//   nothing, so no extra filter is needed (a comment noting this is nice-to-have).


// === THE STRUCT-CONSTRUCTION SWAP (main.rs:606/612 test seeding) ===
//   BEFORE (LayerRule):  layer: 224,   (bare u8 — LayerRule.layer was required u8)
//   AFTER  (Rule):       layer: Some(224),   (Option<u8>)
//   BEFORE (CallbackRule): (no layer field)
//   AFTER  (Rule):         layer: None,      (Option<u8>, explicit)


// === WHY two filtered passes in empty_pattern_warnings ===
//   The test asserts ws[0] contains "layer rule #1" and ws[1] contains "callback rule #1".
//   With one unified array, a single enumerate would number by FILE POSITION, breaking
//   both the text and the test. Two filtered passes (rule.layer.is_some() then .is_none()),
//   each with its own 1-based counter, preserve the per-type numbering AND the exact
//   message text (only the loop scaffolding changes, not the format! strings).
```

### Integration Points

```yaml
SOURCE FILES:
  - modify: "src/core/notifier.rs, src/main.rs, src/core/mod.rs, src/core/pattern.rs"
  - do NOT modify: "src/core/rules.rs (S1 non-test + S2 test module)"

PUBLIC API SURFACE:
  - none changed. The unified Rule/RuleSet are S1's public surface; S3 only changes
    HOW callers access them (.rules instead of .layer_rules/.callback_rules) + how
    tests construct them (Rule{...} instead of LayerRule{...}/CallbackRule{...}).

UPSTREAM CONTRACT (S1 — LANDED):
  - consumes: "RuleSet.rules: Vec<Rule> (rename=\"rule\" SINGULAR); Rule.layer: Option<u8>;
               Rule { pattern, layer, enable, disable, case_sensitive, disable_firmware_config };
               LayerRule/CallbackRule DELETED; .layer_rules/.callback_rules GONE."

PARALLEL SIBLING (S2 — being implemented):
  - S2 owns the rules.rs #[cfg(test)] mod tests block. S3 must NOT edit rules.rs.
  - Gating: cargo check (S3's gate) passes with S1+S3 alone; cargo test (the combined
    gate) needs S2 too. If S3 runs before S2 lands, cargo test will fail to compile on
    rules.rs's stale test module — that is EXPECTED, not an S3 bug.

DEPENDENCIES / Cargo.toml:
  - none new. serde/toml/tempfile already present.

DOWNSTREAM (do NOT implement — listed for awareness):
  - P1.M1.T2: "Sync user-facing docs/*.md to [[rule]] (separate milestone — S3 is code only,
               Mode A doc-COMMENTS, not docs/*.md)."
  - P4.M3.T1.S1: "send pipeline (notify_qmk) consumes evaluate()'s HostContext — unchanged by S3."
  - P5.M1.T1.S1: "--validate-rules CLI consumes collect_callback_names/empty_pattern_warnings/
                  validate_rules summary — S3 keeps their behavior/output stable."

VALIDATION CONSUMERS:
  - The grep gate (no layer_rules/callback_rules/LayerRule/CallbackRule in src/) is the
    completeness proof for the whole T1 milestone (once S2+S3 both land).
```

## Validation Loop

> All commands run from the repo root: `/home/dustin/projects/qmkonnect`.
> **S3's PRIMARY GATE is `cargo check` (0 errors).** The `cargo test` green gate is
> the COMBINED S2+S3 result (see the parallel-sibling note).

### Level 1: Restore compilation (S3's primary gate)

```bash
cd /home/dustin/projects/qmkonnect

# After all 4 files are edited, non-test code must compile clean.
cargo check --bin qmkonnect --offline 2>&1 | tee /tmp/s3-check.log
# Expected: "Finished `dev` profile ... target(s)" + exit 0 + NO warnings.
# If errors remain: grep -E 'error\[E' /tmp/s3-check.log and fix. The 8 baseline errors
# (notifier.rs:572/574, main.rs:253/255/271/281/442/443) must ALL be gone.

# Confirm zero warnings too (not just zero errors):
grep -cE '^warning' /tmp/s3-check.log   # expected: 0
```

### Level 2: Token hygiene (no stale split-schema tokens in the 4 files)

```bash
cd /home/dustin/projects/qmkonnect

# S3's per-file gate — the 4 owned files must be 100% clean.
grep -rnE 'layer_rules|callback_rules|LayerRule|CallbackRule' \
  src/core/notifier.rs src/main.rs src/core/mod.rs src/core/pattern.rs
# Expected: NO output (exit 1). Any hit is a missed site — fix it.

# No PLURAL mistake anywhere in src/ ([[rules]] would silently parse to empty).
grep -rnE '\[\[rules\]\]' src/
# Expected: NO output. Every literal must be [[rule]] (singular).

# The render_rules_body template must contain [[rule]] (singular) and parse inert.
grep -c '\[\[rule\]\]' src/core/mod.rs    # expected: >= 1 (the template; tests may add more)
```

### Level 3: Full test suite (the combined S2+S3 gate)

```bash
cd /home/dustin/projects/qmkonnect

# ⚠ This needs S2 (rules.rs test rewrite) to have LANDED. If S2 hasn't landed yet,
# this fails to COMPILE on rules.rs's stale test module — that is EXPECTED, not an S3
# bug. Run it anyway to confirm S3's OWN tests pass once the build is green.
cargo test --bin qmkonnect -- --test-threads=1 2>&1 | tee /tmp/s3-test.log

# If S2 has landed: expected "test result: ok. <N> passed".
# If S2 has NOT landed: expected COMPILE errors whose --> point into src/core/rules.rs
# (the test module, >= ~494) — NOT into the 4 S3 files. Confirm S3's files are clean:
grep -E '\-\-> src/(core/notifier|core/mod|core/pattern|main)\.rs' /tmp/s3-test.log
# Expected: NO output (S3's files compile clean; remaining errors are S2's rules.rs scope).

# The S3-owned tests specifically (run once the build is green):
cargo test --bin qmkonnect -- --test-threads=1 \
  test_unknown_callback_names_helper \
  test_collect_callback_names_dedupes \
  test_collect_callback_names_empty_when_no_rules \
  test_empty_pattern_warnings_flags_empty_single_and_parts \
  test_empty_pattern_warnings_silent_for_real_patterns \
  test_contradictory_callback_warnings_flags_same_rule_overlap \
  test_render_rules_body_contains_section_markers \
  test_render_rules_body_parses_to_default_ruleset
# Expected: 8 passed. These are the S3-touched tests; each must be green.
```

### Level 4: Correctness spot-checks (behavior preservation)

```bash
cd /home/dustin/projects/qmkonnect

# The seeded template must STILL parse to an all-default (inert) RuleSet — proves a
# fresh install's rules.toml is valid + disabled. (test_render_rules_body_parses_to_
# default_ruleset guards this, but verify the property directly:)
cat > /tmp/check_template.rs <<'EOF'
// quick check: extract render_rules_body output, parse, assert empty+default
EOF
# (Simplest: trust the test in Level 3. The template's every active line is '# '-prefixed,
#  so toml::from_str yields RuleSet::default() — rs.rules.is_empty() && !host.dfc.)

# Confirm empty_pattern_warnings preserves per-type numbering (the trickiest logic change):
cargo test --bin qmkonnect -- --test-threads=1 \
  test_empty_pattern_warnings_flags_empty_single_and_parts 2>&1 | tail -3
# Expected: "... 1 passed". This proves "layer rule #1" + "callback rule #1" survived.

# Confirm the validate_rules summary still prints (smoke — no test asserts its text,
# but verify it compiles + runs by invoking --validate-rules on the §9 fixture if a
# keyboard isn't required; otherwise trust the compile + the derive-split logic).
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1: `cargo check --bin qmkonnect --offline` → 0 errors, 0 warnings.
- [ ] Level 2: 0 `layer_rules`/`callback_rules`/`LayerRule`/`CallbackRule` hits in the 4 files.
- [ ] Level 2: 0 `[[rules]]` (plural) hits anywhere in src/.
- [ ] Level 3 (if S2 landed): `cargo test --bin qmkonnect -- --test-threads=1` → all pass.
- [ ] Level 3 (if S2 not landed): remaining compile errors are ONLY in `rules.rs` test module (S2), NOT in the 4 S3 files.
- [ ] Level 3: the 8 S3-touched tests pass (once build is green).

### Feature Validation

- [ ] notifier.rs:572 loop iterates `rules.rules`; :519 doc + :1908 test TOML use `[[rule]]`.
- [ ] main.rs:253 loop iterates `rules.rules`; :229/:241 doc uses `rule[]`/`[[rule]]`.
- [ ] main.rs empty_pattern_warnings keeps "layer rule #N"/"callback rule #N" via two filtered passes.
- [ ] main.rs:442-443 summary uses derived split counts (or single len).
- [ ] main.rs test seeding uses `Rule { layer: Some(...)/None, ... }`.
- [ ] main.rs all test TOML literals use `[[rule]]`.
- [ ] mod.rs render_rules_body has 4 `# [[rule]]` blocks + unified divider; parses to all-default RuleSet.
- [ ] mod.rs tests assert `[[rule]]` + `rs.rules.is_empty()`.
- [ ] pattern.rs:1090 doc says `[rule]`.

### Code Quality Validation

- [ ] Edits follow existing style (snake_case, `///` docs, raw-string TOML in tests).
- [ ] `[[rule]]` is SINGULAR everywhere (corrected vs the scout artifact's stale `[[rules]]`).
- [ ] No `#[allow(...)]` added; no new warnings.
- [ ] Only the 4 listed files modified; `rules.rs` untouched.

### Documentation & Deployment

- [ ] Mode A — code doc-COMMENTS (notifier.rs:519, pattern.rs:1090, mod.rs template) updated as part of this work.
- [ ] User-facing `docs/*.md` are NOT touched here (P1.M1.T2).
- [ ] No environment variables, config, or Cargo.toml changes.

---

## Anti-Patterns to Avoid

- ❌ Don't use `[[rules]]` (PLURAL) — it is SINGULAR `[[rule]]` (serde `rename = "rule"`). The
  scout artifact's plural mentions are STALE. A plural key silently parses to an empty rules vec.
- ❌ Don't leave a bare `layer: 224` in struct construction — it must be `layer: Some(224)`
  (`Option<u8>`). The 2 E0282 cascade errors (notifier.rs:574, main.rs:255) are NOT separate bugs —
  they resolve automatically once the loop field-access is fixed; don't "fix" them in isolation.
- ❌ Don't collapse `empty_pattern_warnings` to a single enumerate pass — the test asserts
  "layer rule #1" + "callback rule #1" (per-type 1-based numbering). Use two filtered passes.
- ❌ Don't skip `mod.rs`/`pattern.rs` just because `cargo check` is green — `cargo check` doesn't
  compile strings, comments, or tests. The grep gate + `cargo test` + user-facing correctness
  require those fixes. (The template string is production code users see.)
- ❌ Don't edit `src/core/rules.rs` — S1 owns the non-test code; S2 owns the test module. S3's
  scope is strictly the 4 caller files. Editing rules.rs steals sibling work + breaks the boundary.
- ❌ Don't touch `list_callbacks` (main.rs:318) — it queries the firmware callback registry, not the
  rules structs. "callback" there = firmware callbacks.
- ❌ Don't change any test's ASSERTIONS (collect order, ws.len()==2, ws[0]/ws[1] substrings,
  contradictory ws.len()==1, parse-to-default) — only the TOML keys + struct construction change.
  The migration is behavior-preserving.
- ❌ Don't rewrite the `render_rules_body` rule CONTENTS (match/layer/enable/disable values) — only
  the headers (`[[layer_rules]]`/`[[callback_rules]]` → `[[rule]]`) + the merged comment divider.
  Keep every active line `# `-prefixed (inert-parse property).
- ❌ Don't use a parallel test run — `--test-threads=1` is mandatory (AGENTS.md: shared global debouncer).
- ❌ Don't panic if `cargo test` fails to compile when S2 hasn't landed — the failure is in rules.rs's
  test module (S2's scope), NOT S3's files. Confirm via the `-->` pointer grep (Level 3).
- ❌ Don't "improve" the `validate_rules` summary text beyond the derive-split or single-count forms —
  no test asserts it, but keeping the split breakdown preserves the user-facing info.

---

**Confidence Score: 9/10** for one-pass implementation success. The deliverable is a mechanical
migration of ~16 enumerated sites across 4 files against S1's fully-specified unified model, with
every site's exact line number + before/after verified against the actual source, the 8 baseline
compile errors localized (6 root + 2 auto-resolving cascade), the `empty_pattern_warnings`
filtered-pass decision (to preserve per-type numbering + test asserts), the verbatim
`render_rules_body` template target, and the `[[rule]]`-SINGULAR correction (vs the scout artifact's
stale plural). The one residual risk: the `empty_pattern_warnings` filtered-pass logic (the only
non-mechanical change) — mitigated by the explicit two-pass code skeleton + the targeted test
(`test_empty_pattern_warnings_flags_empty_single_and_parts`). The parallel-S2 gating reality is
confronted head-on: S3's `cargo check` gate is independent; the `cargo test` green gate is the
combined S2+S3 result, with a `-->`-pointer grep to distinguish S3's files from S2's rules.rs scope.
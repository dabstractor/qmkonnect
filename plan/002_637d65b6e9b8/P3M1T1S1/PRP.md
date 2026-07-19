# PRP — P3.M1.T1.S1: Define RuleSet/HostDefaults/LayerRule/CallbackRule structs with serde

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This task creates ONE new file
> (`src/core/rules.rs`) with four `#[derive(Deserialize)]` structs that model
> `rules.toml`, and adds ONE line (`pub mod rules;`) to `src/core/mod.rs` to
> register the module. It is the **data-model foundation** for the host-side
> rules system (PRD §9 `rules.toml` Schema Reference). Source of truth for the
> struct shapes: **`spec/HOST_RULES.md` §9** (verbatim Rust model + canonical
> TOML examples). It **consumes** the `Pattern` enum shipped by P2.M1.T3.S2
> (already in-tree at `src/core/pattern.rs:1102-1113`, status: Complete) and is
> **consumed downstream** by P3.M1.T1.S2 (`parse_rules()` + effective
> `disable_firmware_config` resolution) and P3.M1.T2.S1 (`evaluate()`). PRD §14
> names `src/core/rules.rs` as the "rules.toml model + evaluation" module.

---

## Goal

**Feature Goal**: Create `src/core/rules.rs` containing four serde-deserializable
structs — `RuleSet`, `HostDefaults`, `LayerRule`, `CallbackRule` — that faithfully
model the `rules.toml` schema defined in `spec/HOST_RULES.md` §9, using the
existing `Pattern` enum (P2.M1.T3.S2) for the `match` field. Each struct carries
Mode-A rustdoc with the §9 TOML examples. Register the module in `src/core/mod.rs`.

**Deliverable**:
1. **NEW file** `src/core/rules.rs` with:
   - `use crate::core::pattern::Pattern;` (re-use, NOT redefine),
   - `#[derive(Debug, Deserialize, Default)] pub struct RuleSet { host: HostDefaults, layer_rules: Vec<LayerRule>, callback_rules: Vec<CallbackRule> }` with `#[serde(default)]` on each field,
   - `#[derive(Debug, Deserialize)] pub struct HostDefaults { #[serde(default)] disable_firmware_config: bool }` + a manual `impl Default` (→ `false`),
   - `#[derive(Debug, Deserialize)] pub struct LayerRule { #[serde(rename="match")] pattern: Pattern, layer: u8, #[serde(default)] case_sensitive: bool, #[serde(default)] disable_firmware_config: Option<bool> }`,
   - `#[derive(Debug, Deserialize)] pub struct CallbackRule { #[serde(rename="match")] pattern: Pattern, #[serde(default)] enable: Vec<String>, #[serde(default)] disable: Vec<String>, #[serde(default)] case_sensitive: bool, #[serde(default)] disable_firmware_config: Option<bool> }`,
   - Mode-A rustdoc (`///` blocks) on each struct with the §9 TOML examples in ` ```toml ` fences,
   - a `#[cfg(test)] mod tests` block with ~9 deserialization round-trip + strictness tests.
2. **ONE-line edit** to `src/core/mod.rs`: add `pub mod rules;` between
   `pub mod pattern;` and `pub mod types;`.

**Success Definition**:
- `toml::from_str::<RuleSet>(&SECTION_9_EXAMPLE_TOML)` succeeds and produces the
  expected nested struct values (host default false, two layer rules, two callback
  rules with correct `Pattern` variants, override `Option<bool>`s).
- A minimal/empty `rules.toml` (`""` or no `[host]`) deserializes to
  `RuleSet { host: HostDefaults { disable_firmware_config: false }, layer_rules: vec![], callback_rules: vec![] }`.
- A `[[layer_rules]]` missing the required `match` or `layer` key is a
  **deserialization error** (strictness for the future `--validate-rules`).
- `RuleSet::default()` compiles and yields the all-default ruleset (proves `Default`
  propagation through `HostDefaults` + empty `Vec`s).
- `cargo build --bin qmkonnect` compiles clean (no new warnings).
- `cargo test --bin qmkonnect -- --test-threads=1` is green (new `test_rules_*`
  tests + all existing tests; no regression).
- `git status` shows exactly TWO changed files: `src/core/rules.rs` (new) and
  `src/core/mod.rs` (one-line `pub mod rules;`).

## User Persona (if applicable)

**Target User**: downstream qmkonnect subtasks + `rules.toml` authors.
- **P3.M1.T1.S2** (`parse_rules()` + effective resolution): reads
  `rule_set.host.disable_firmware_config`, iterates `rule_set.layer_rules` /
  `rule_set.callback_rules`, and resolves each rule's `disable_firmware_config:
  Option<bool>` (None ⇒ inherit host).
- **P3.M1.T2.S1** (`evaluate()`): iterates `layer_rules` (first-match-wins) and
  `callback_rules` (all-match), reading `rule.layer: u8`, `rule.enable/disable:
  Vec<String>`, `rule.case_sensitive: bool`.
- **`rules.toml` authors** (humans): they write the §9 TOML; these structs make
  every documented form deserialize correctly.

**Use Case**: a user authors `rules.toml` with `[host]` + `[[layer_rules]]` +
`[[callback_rules]]` tables; `toml::from_str::<RuleSet>` turns it into a typed
nested struct that the resolver/evaluator consume without re-parsing.

**Pain Points Addressed**: gives the host-rules pipeline a typed, validated
in-memory model of `rules.toml` — replacing ad-hoc string parsing with serde's
strict, documented deserialization that errors on malformed input.

## Why

- **PRD §14 names `src/core/rules.rs`** as the "rules.toml model + evaluation"
  module. This task creates the model half; P3.M1.T2 creates the evaluation half.
- **HOST_RULES.md §9 is the locked schema.** Every field, default, and rename is
  decided there; this task is a faithful transcription into compiling Rust.
- **Unblocks P3.M1.T1.S2 + P3.M1.T2.S1.** Both depend on these exact struct
  shapes (the `Option<bool>` override, the `Vec<String>` callback lists, the
  `Pattern` import). Shipping the model first lets the resolver/evaluator be
  written against a stable type contract.
- **`--validate-rules` (P5.M1.T1.S1) needs strict deserialization.** The
  required-`match`/required-`layer` semantics (no `#[serde(default)]` on them)
  make a malformed `rules.toml` fail to parse — the exact behavior `--validate-rules`
  reports.

## What

A new `src/core/rules.rs` (created, not appended) + a one-line `mod.rs` edit. The
four structs are **verbatim** from `spec/HOST_RULES.md` §9 (see
`research/notes.md` §1 for the exact source). `Pattern` is **imported** from
`crate::core::pattern` (P2.M1.T3.S2) — never redefined.

### Success Criteria
- [ ] `pub struct RuleSet` with `#[derive(Debug, Deserialize, Default)]`; fields `host: HostDefaults`, `layer_rules: Vec<LayerRule>`, `callback_rules: Vec<CallbackRule>`, all `#[serde(default)]`.
- [ ] `pub struct HostDefaults` with `#[derive(Debug, Deserialize)]`; field `#[serde(default)] disable_firmware_config: bool`; a manual `impl Default for HostDefaults` returning `disable_firmware_config: false`.
- [ ] `pub struct LayerRule` with `#[derive(Debug, Deserialize)]`; `#[serde(rename="match")] pattern: Pattern`, `layer: u8`, `#[serde(default)] case_sensitive: bool`, `#[serde(default)] disable_firmware_config: Option<bool>`.
- [ ] `pub struct CallbackRule` with `#[derive(Debug, Deserialize)]`; `#[serde(rename="match")] pattern: Pattern`, `#[serde(default)] enable: Vec<String>`, `#[serde(default)] disable: Vec<String>`, `#[serde(default)] case_sensitive: bool`, `#[serde(default)] disable_firmware_config: Option<bool>`.
- [ ] `Pattern` is imported (`use crate::core::pattern::Pattern;`), NOT redefined.
- [ ] All four structs are `pub`; all fields are `pub` (per §9).
- [ ] Mode-A rustdoc on each struct with the §9 TOML examples in ` ```toml ` fences (NOT runnable Rust doctests — see G2).
- [ ] `pub mod rules;` added to `src/core/mod.rs` (between `pattern` and `types`).
- [ ] `#[cfg(test)] mod tests` with ~9 deserialization tests (full §9 example, defaults, Option<bool> inheritance, required-field strictness, Pattern variant dispatch).
- [ ] No `parse_rules()` / resolution / `evaluate()` / path logic (those are S2/T2).
- [ ] `cargo build --bin qmkonnect` clean; `cargo test --bin qmkonnect -- --test-threads=1` green; `git status` = 2 files only.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge of this codebase can implement this
using only this PRP, because (a) the ENTIRE §9 verbatim Rust model (every derive,
attribute, field, default) is reproduced in `research/notes.md` §1; (b) the
canonical TOML examples to embed in the rustdoc are reproduced verbatim in §2;
(c) the `Pattern` dependency contract (where it lives, its exact derives, the
`use` path) is in §3 and confirmed by reading `src/core/pattern.rs:1102-1113`;
(d) the repo conventions to follow (module registration in mod.rs, the
`#[derive(serde::Deserialize, Serialize, Default)]` + `#[serde(default)]` Config
pattern, `toml::from_str`, the per-module `#[cfg(test)] mod tests` idiom) are in
§4; (e) every field's attribute + default semantics are tabulated in §5; (f) the
`rename = "match"` correctness is justified in §6; (g) 10 gotchas are enumerated
in §7 (including the binary-only-crate doctest constraint G2 and the no-op
redundant renames G3); (h) the downstream consumer contracts (S2/T2 field reads)
are in §9; (i) the scope boundary is in §10.

### Documentation & References

```yaml
# MUST READ — the canonical schema (single source of truth for struct shapes)
- file: spec/HOST_RULES.md
  why: "§9 ('rules.toml Schema Reference') gives the VERBATIM Rust model for
        RuleSet/HostDefaults/LayerRule/CallbackRule/Pattern (every derive,
        attribute, field, default) AND the canonical TOML example to embed in
        the rustdoc. §3 (Locked Design Decisions) C10/C7/C8/C11 justify the
        per-rule disable_firmware_config Option<bool>, no-match-clear, and
        host-layer >= 224."
  section: "## 9. rules.toml Schema Reference" (the ```toml block + the ```rust model)
  gotcha: "§9 shows a Pattern enum in its rust snippet — that is ILLUSTRATIVE;
           the real Pattern is P2.M1.T3.S2's in pattern.rs. rules.rs IMPORTS it (G1)."

# MUST READ — the verbatim §9 model + the field/default semantics table (THIS task's contract)
- file: plan/002_637d65b6e9b8/P3M1T1S1/research/notes.md
  why: "§1 reproduces the §9 Rust model verbatim (with the `use crate::core::pattern::Pattern`
        import). §2 reproduces the §9 TOML examples verbatim. §3 confirms Pattern is
        in-tree at pattern.rs:1102-1113 (Complete). §4 lists the mod.rs conventions.
        §5 tabulates every field's attribute + absent-value. §7 has 10 gotchas
        (G1 no-redefine, G2 doctest-no-compile, G3 redundant-renames-noop, G4 manual
        Default, G5 Option<bool>=None, G6 required-errors, G7 no-logic-here, G8
        single-threaded tests, G9 mod ordering, G10 pub visibility). §8 is the
        recommended 9-test set. §9 is the downstream contract. §10 is the scope wall."

# MUST READ — the Pattern enum (the INPUT dependency — DO NOT redefine)
- file: src/core/pattern.rs
  why: "lines 1102-1113 define `#[derive(Debug, Clone, PartialEq, Deserialize)]
        #[serde(untagged)] pub enum Pattern { Single(String), Parts(String, String) }`.
        THIS task does `use crate::core::pattern::Pattern;` and embeds it as the
        `pattern` field of LayerRule/CallbackRule. serde untagged already gives
        string->Single, 2-array->Parts, wrong-length->error (no work here)."
  pattern: "the enum is `pub` and `Deserialize`; using it inside another Deserialize
            struct is zero-config."
  gotcha: "do NOT copy the Pattern enum into rules.rs (G1). Import it."

# MUST READ — the file THIS task edits for module registration
- file: src/core/mod.rs
  why: "top of file declares `pub mod notifier; pub mod pattern; pub mod types;`.
        Add ONE line `pub mod rules;` between `pattern;` and `types;` (alpha order).
        The Config struct (same file) shows the repo derive convention:
        `#[derive(serde::Deserialize, serde::Serialize, Default)]` + `#[serde(default)]`
        + `#[serde(default = \"fn\")]` for non-std defaults. rules.rs follows the
        `#[serde(default)]` idiom but derives Deserialize ONLY (rules are read, not
        written). parse_config() shows `toml::from_str::<Config>` — rules tests mirror it."
  pattern: "the `#[cfg(test)] mod tests { use super::*; ... }` block at the file tail
            is the test idiom to copy into rules.rs."
  gotcha: "mod.rs needs ONLY the `pub mod rules;` line. Do NOT add a rules field to
           Config, do NOT wire rules into startup (that's P3.M1.T1.S2/P4)."

# MUST READ — the upstream Pattern contract (confirms Single/Parts/untagged)
- file: plan/002_637d65b6e9b8/P2M1T3S2/PRP.md
  why: "locks the Pattern enum definition (Single(String), Parts(String, String),
        #[serde(untagged)], derives Debug/Clone/PartialEq/Deserialize) and its home in
        pattern.rs. Confirms string->Single, 2-array->Parts, 1/3-array->error. THIS
        task reuses it unchanged."
  section: "## What" (the Pattern enum + the serde untagged dispatch)

# Reference — serde field attributes (default, rename) used pervasively here
- url: https://serde.rs/field-attrs.html
  why: "documents #[serde(default)] (use type's Default when the field is absent) and
        #[serde(rename = \"...\")] (map a Rust field name to a different serialization
        key). Both are used on every optional field and on `pattern` respectively."
  critical: "#[serde(default)] on an Option<T> field yields None (not Some(Default)) —
             this is exactly the 'None => inherit [host]' semantics (G5). rename takes
             a &str; `match` (a Rust keyword) is fine as a string rename value."

# Reference — serde container attributes (derive, untagged on Pattern)
- url: https://serde.rs/derive-attrs.html
  why: "documents #[derive(Deserialize)] and the container-level attribute syntax. The
        Pattern enum's #[serde(untagged)] is documented at serde.rs/enum-representations."
```

### Current Codebase tree (relevant subset)

```bash
src/
  main.rs              # `mod core;` (binary-only crate — NO lib.rs; see G2)
  core/
    mod.rs             # `pub mod notifier; pub mod pattern; pub mod types;` + Config + parse_config
                         # ← EDIT: add ONE line `pub mod rules;`
    pattern.rs         # Pattern enum (P2.M1.T3.S2) at lines 1102-1113; pub fn match_pattern (1155)
                         #   + #[cfg(test)] mod tests (serde tests use #[serde(rename="match")] helper)
    notifier.rs        # Notifier trait + debouncer (unchanged)
    types.rs           # WindowInfo (unchanged)
  tray.rs / linux_tray.rs / platforms/ ...  # unchanged
Cargo.toml             # serde 1.0 (+derive), toml 0.9, thiserror 1.0 ALREADY deps (unchanged)
spec/HOST_RULES.md     # §9 = the schema source of truth (READ-ONLY)
```

### Desired Codebase tree (files this task changes)

```bash
src/
  core/
    rules.rs           # NEW — 4 structs (RuleSet/HostDefaults/LayerRule/CallbackRule) + `use
                         #         crate::core::pattern::Pattern` + Mode-A rustdoc + #[cfg(test)] mod tests
    mod.rs             # MODIFIED (one line) — + `pub mod rules;` (between pattern; and types;)
    # pattern.rs, notifier.rs, types.rs, main.rs, Cargo.toml: UNCHANGED
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (G1 — do NOT redefine Pattern): the `Pattern` enum (Single/Parts,
//   #[serde(untagged)]) is P2.M1.T3.S2's type, ALREADY in `src/core/pattern.rs`
//   (lines 1102-1113, status Complete). rules.rs must `use crate::core::pattern::Pattern;`.
//   HOST_RULES.md §9 re-shows Pattern in its snippet ONLY to make the doc
//   self-contained — copying it into rules.rs is a DUPLICATE-DEFINITION error.
//
// CRITICAL (G2 — binary-only crate; doctests don't run under `--bin`): there is
//   NO lib.rs (`src/main.rs` declares `mod core;`). The AGENTS.md validation
//   command `cargo test --bin qmkonnect` runs UNIT TESTS ONLY, not doctests. A
//   runnable Rust doctest (` ``` ` block) doing `use qmkonnect::core::rules::RuleSet`
//   would NOT compile under `cargo test --doc` (no library target to link against).
//   MITIGATION: embed the §9 examples as ` ```toml ` fenced blocks in the rustdoc —
//   rustdoc renders these as documentation text and does NOT compile them. (If a
//   runnable Rust usage example is added, mark it ``` ```rust,ignore ```.) The
//   existing pattern.rs already carries two runnable `qmkonnect::core::pattern::`
//   doctests that are likewise untested by `--bin` — do NOT multiply that surface.
//
// GOTCHA (G3 — two redundant renames are no-ops): §9 writes
//   `#[serde(default, rename = "layer_rules")] pub layer_rules: Vec<LayerRule>` and
//   likewise for callback_rules. The `rename = "layer_rules"` is a NO-OP (field name
//   already equals the TOML key `[[layer_rules]]`). Keep them verbatim (spec fidelity,
//   zero cost) OR drop them (cleaner) — byte-identical deserialization either way.
//   The ONLY rename that MATTERS is `#[serde(rename = "match")]` on the `pattern`
//   field (Rust field `pattern`, TOML key `match`).
//
// GOTCHA (G4 — manual Default for HostDefaults): §9 gives HostDefaults a MANUAL
//   `impl Default { fn default() -> Self { Self { disable_firmware_config: false } } }`
//   rather than `#[derive(Default)]`. Both yield `false` (bool's default). Follow §9
//   (manual impl) so the "default = stack" intent is explicit AND because it's what
//   makes `RuleSet`'s `#[serde(default)] host: HostDefaults` resolve to the §9 default.
//
// CRITICAL (G5 — Option<bool> default is None, NOT Some(false)): the
//   `disable_firmware_config: Option<bool>` field with `#[serde(default)]` defaults to
//   `None` when the TOML key is absent. This is CORRECT and intentional: `None` means
//   "inherit the [host] global default" (resolved by P3.M1.T1.S2). Do NOT change the
//   type to `bool` (that would lose the inherit semantics).
//
// CRITICAL (G6 — required fields error if absent): `pattern` (TOML `match`) and
//   `layer` have NO `#[serde(default)]` — a `[[layer_rules]]` missing either is a
//   deserialization ERROR. This is DESIRED (malformed rules.toml must fail the future
//   --validate-rules). Do NOT add defaults to them.
//
// GOTCHA (G7 — NO logic in this task): do NOT implement parse_rules(), effective
//   resolution, evaluate(), HostContext, or file-path resolution. Those are
//   P3.M1.T1.S2 / P3.M1.T2.S1 / P4. THIS task is the bare data model + module
//   registration + rustdoc + (recommended) deserialization tests.
//
// GOTCHA (G8 — single-threaded tests crate-wide): `cargo test --bin qmkonnect --
//   --test-threads=1` (shared debouncer state in notifier.rs, AGENTS.md). NEVER run
//   multi-threaded.
//
// GOTCHA (G9 — module ordering): add `pub mod rules;` to mod.rs BETWEEN
//   `pub mod pattern;` and `pub mod types;` (alpha order: notifier < pattern < rules
//   < types). One line, nothing else in mod.rs changes.
//
// GOTCHA (G10 — pub visibility everywhere): all four structs are `pub` (consumed
//   cross-module by S2/T2). Every field is `pub` (§9 shows `pub` on each). The crate
//   has no global #![allow(dead_code)]; `pub` items don't warn as dead code, so the
//   not-yet-consumed structs compile without warnings.
//
// CRATE QUIRK: serde (1.0 + derive) and toml (0.9) are ALREADY Cargo deps
//   (Cargo.toml lines 12, 21). No Cargo.toml edit. `toml::from_str::<RuleSet>` is the
//   parse entry (same as mod.rs::parse_config's `toml::from_str::<Config>`).
```

## Implementation Blueprint

### Data models and structure

Four structs + one import. Verbatim from `spec/HOST_RULES.md` §9 (see
`research/notes.md` §1). `Pattern` is imported, not defined.

```rust
//! Host-side `rules.toml` data model (PRD §9 / HOST_RULES.md §9).
//!
//! These structs are the serde-deserialization boundary for `rules.toml`. They
//! are consumed by the resolver (effective `disable_firmware_config`) and the
//! evaluator (`evaluate()`), both implemented in later subtasks.

use crate::core::pattern::Pattern;   // P2.M1.T3.S2 — Single/Parts, #[serde(untagged)]
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct RuleSet {
    #[serde(default)]
    pub host: HostDefaults,
    #[serde(default, rename = "layer_rules")]
    pub layer_rules: Vec<LayerRule>,
    #[serde(default, rename = "callback_rules")]
    pub callback_rules: Vec<CallbackRule>,
}

#[derive(Debug, Deserialize)]
pub struct HostDefaults {
    #[serde(default)]
    pub disable_firmware_config: bool, // default false (stack)
}
impl Default for HostDefaults {
    fn default() -> Self {
        Self { disable_firmware_config: false }
    }
}

#[derive(Debug, Deserialize)]
pub struct LayerRule {
    #[serde(rename = "match")]
    pub pattern: Pattern,
    pub layer: u8,
    #[serde(default)]
    pub case_sensitive: bool,
    #[serde(default)]
    pub disable_firmware_config: Option<bool>, // None => inherit [host]
}

#[derive(Debug, Deserialize)]
pub struct CallbackRule {
    #[serde(rename = "match")]
    pub pattern: Pattern,
    #[serde(default)]
    pub enable: Vec<String>,
    #[serde(default)]
    pub disable: Vec<String>,
    #[serde(default)]
    pub case_sensitive: bool,
    #[serde(default)]
    pub disable_firmware_config: Option<bool>, // None => inherit [host]
}
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CREATE src/core/rules.rs — imports + the four structs
  - DO: create the file with the module-level //! doc, `use crate::core::pattern::Pattern;`,
        `use serde::Deserialize;`, and the four structs EXACTLY as in the Data-models
        block above (derives, attributes, field types, pub-ness).
  - FOLLOW: spec/HOST_RULES.md §9 verbatim (reproduced in research/notes.md §1).
  - NAMING: RuleSet, HostDefaults, LayerRule, CallbackRule (PascalCase); fields
            host/layer_rules/callback_rules/disable_firmware_config/pattern/layer/
            case_sensitive/enable/disable (snake_case) — all as §9 specifies.
  - GOTCHA G1: Pattern is IMPORTED, never redefined.
  - GOTCHA G3: the `rename = "layer_rules"`/`"callback_rules"` are no-ops but kept
               verbatim (or dropped — equivalent). KEEP `rename = "match"`.
  - GOTCHA G4: HostDefaults has a MANUAL `impl Default` (→ false), not derive.
  - GOTCHA G5: disable_firmware_config is Option<bool> (defaults None).
  - GOTCHA G6: pattern + layer have NO #[serde(default)] (required).
  - GOTCHA G10: all structs + fields are pub.
  - PLACEMENT: new file src/core/rules.rs.

Task 2: ADD Mode-A rustdoc (`///` blocks) on each struct
  - DO: above each `pub struct`, add a `///` doc block explaining the struct's role
        and embedding the relevant slice of the §9 TOML example in a ` ```toml ` fence.
        - RuleSet:        the whole §9 example skeleton ([host] + [[layer_rules]] + [[callback_rules]]).
        - HostDefaults:    the [host] table (`disable_firmware_config = false`).
        - LayerRule:       the two [[layer_rules]] examples (Single "alacritty"+override,
                           Parts ["*chrome*","*youtube*"]+case_sensitive).
        - CallbackRule:    the two [[callback_rules]] examples (enable/disable lists,
                           Parts with override).
  - SOURCE: the verbatim TOML is in research/notes.md §2 / spec/HOST_RULES.md §9.
  - GOTCHA G2: use ` ```toml ` fences (rendered as text, NOT compiled as doctests).
               Do NOT use bare ` ``` ` (runnable Rust doctest) with a `qmkonnect::`
               path — it won't compile under `cargo test --doc` in this bin-only crate.
               If a runnable Rust example is desired, mark it ``` ```rust,ignore ```.
  - CITE: each doc block references "spec/HOST_RULES.md §9" and, for the Pattern
          field, the firmware WT()/0x1D split (see pattern.rs Pattern rustdoc).

Task 3: REGISTER the module — one-line edit to src/core/mod.rs
  - DO: add `pub mod rules;` to the top-of-file module declarations, between
        `pub mod pattern;` and `pub mod types;`.
  - FIND: the three-line block `pub mod notifier; \n pub mod pattern; \n pub mod types;`.
  - PRESERVE: every other line of mod.rs (Config, helpers, parse_config, its tests).
  - GOTCHA G9: alpha order (rules between pattern and types). Nothing else changes.

Task 4: ADD the #[cfg(test)] mod tests block to rules.rs (the validation gate)
  - DO: append `#[cfg(test)] mod tests { use super::*; ... }` at the file tail, with ~9 tests:
        1. test_rules_full_section9_example_parses — toml::from_str::<RuleSet>(&SECTION_9_TOML)
           asserts host.disable_firmware_config==false; layer_rules.len()==2;
           layer_rules[0].pattern==Pattern::Single("alacritty".into()) && .layer==224 &&
           .disable_firmware_config==Some(true) && .case_sensitive==false;
           layer_rules[1].pattern==Pattern::Parts("*chrome*".into(),"*youtube*".into()) &&
           .layer==225 && .case_sensitive==false && .disable_firmware_config==None;
           callback_rules.len()==2; [0].enable==["vim_lazy","disable_vim"] && .disable==["vim_lazy"];
           [1].disable_firmware_config==Some(true).
        2. test_rules_missing_host_table_defaults_false — TOML with rules but no [host]
           → host.disable_firmware_config==false (#[serde(default)] + manual Default).
        3. test_rules_empty_toml_is_all_default — toml::from_str::<RuleSet>("") == RuleSet::default()
           (host=false, empty vecs).
        4. test_rules_layer_override_absent_is_none — a [[layer_rules]] without
           disable_firmware_config → .disable_firmware_config==None (inherit host, G5).
        5. test_rules_callback_enable_disable_default_empty — a [[callback_rules]] with only
           match+enable → .disable==vec![] (and vice versa).
        6. test_rules_match_string_to_single — `match = "x"` → Pattern::Single; `match = ["a","b"]`
           → Pattern::Parts (delegates to Pattern's untagged serde).
        7. test_rules_missing_layer_errors — a [[layer_rules]] with match but no layer → is_err() (G6).
        8. test_rules_missing_match_errors — a [[layer_rules]] with layer but no match → is_err() (G6).
        9. test_rules_default_propagates — RuleSet::default(): host.disable_firmware_config==false,
           layer_rules.is_empty(), callback_rules.is_empty() (proves Default, G4).
  - NAMING: prefix test_rules_ (disjoint from pattern.rs test_mp_/test_parity_/test_pattern_serde_
            and mod.rs test_*).
  - HELPER: define a `const SECTION_9_TOML: &str = r#"..."#;` (the verbatim §9 example) at the
            head of mod tests for tests 1 (and reuse slices for others).
  - PATTERN: mirror mod.rs/pattern.rs serde-round-trip idiom (`toml::from_str::<RuleSet>(...).unwrap()`).
  - GOTCHA G8: the crate-wide command is single-threaded; individual `cargo test` invocations
               inherit that via the runner flag.
  - PLACEMENT: src/core/rules.rs, file tail.

Task 5: VERIFY build + tests (single-threaded, per AGENTS.md)
  - RUN: cargo build --bin qmkonnect          (expect clean; no NEW warnings)
  - RUN: cargo test --bin qmkonnect rules -- --test-threads=1
         (expect: all test_rules_* pass)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1
         (expect: full crate green — new rules tests + pattern + notifier + types + mod; no regression)
  - CONFIRM git status shows EXACTLY two files: src/core/rules.rs (new) + src/core/mod.rs (modified).
  - IF a deserialization test fails: re-read spec/HOST_RULES.md §9 + research/notes.md §1/§5.
        The struct shapes are the spec; a failure = a transcription slip (wrong attr, wrong
        type, missing pub). Do NOT "fix" the test to match a divergent struct.
```

### Implementation Patterns & Key Details

```rust
// The canonical skeleton (this IS the spec — match it; full verbatim in research/notes.md §1).
//
// //! Host-side `rules.toml` data model (PRD §9 / HOST_RULES.md §9).
// use crate::core::pattern::Pattern;   // P2.M1.T3.S2
// use serde::Deserialize;
//
// /// (Mode-A rustdoc with ```toml example — see Task 2)
// #[derive(Debug, Deserialize, Default)]
// pub struct RuleSet {
//     #[serde(default)] pub host: HostDefaults,
//     #[serde(default, rename = "layer_rules")] pub layer_rules: Vec<LayerRule>,
//     #[serde(default, rename = "callback_rules")] pub callback_rules: Vec<CallbackRule>,
// }
//
// /// ...
// #[derive(Debug, Deserialize)]
// pub struct HostDefaults {
//     #[serde(default)] pub disable_firmware_config: bool,
// }
// impl Default for HostDefaults {
//     fn default() -> Self { Self { disable_firmware_config: false } }
// }
//
// /// ...
// #[derive(Debug, Deserialize)]
// pub struct LayerRule {
//     #[serde(rename = "match")] pub pattern: Pattern,
//     pub layer: u8,
//     #[serde(default)] pub case_sensitive: bool,
//     #[serde(default)] pub disable_firmware_config: Option<bool>,
// }
//
// /// ...
// #[derive(Debug, Deserialize)]
// pub struct CallbackRule {
//     #[serde(rename = "match")] pub pattern: Pattern,
//     #[serde(default)] pub enable: Vec<String>,
//     #[serde(default)] pub disable: Vec<String>,
//     #[serde(default)] pub case_sensitive: bool,
//     #[serde(default)] pub disable_firmware_config: Option<bool>,
// }
//
// // Test idiom (Task 4):
// // #[cfg(test)]
// // mod tests {
// //     use super::*;
// //     const SECTION_9_TOML: &str = r#"[host]\ndisable_firmware_config = false\n..."#;
// //     #[test] fn test_rules_full_section9_example_parses() {
// //         let rs: RuleSet = toml::from_str(SECTION_9_TOML).unwrap();
// //         assert_eq!(rs.host.disable_firmware_config, false);
// //         assert_eq!(rs.layer_rules.len(), 2);
// //         assert_eq!(rs.layer_rules[0].pattern, Pattern::Single("alacritty".into()));
// //         assert_eq!(rs.layer_rules[0].layer, 224);
// //         assert_eq!(rs.layer_rules[0].disable_firmware_config, Some(true));
// //         // ...
// //     }
// // }
//
// NOTE: Pattern on the RHS is the IMPORTED type from pattern.rs. Pattern derives
// PartialEq, so `rs.layer_rules[0].pattern == Pattern::Single("alacritty".into())`
// works directly (no custom Eq needed).
```

### Integration Points

```yaml
MODULE REGISTRATION:
  - file: src/core/mod.rs
    change: "add `pub mod rules;` between `pub mod pattern;` and `pub mod types;`"
    pattern: "matches the existing `pub mod notifier; pub mod pattern; pub mod types;` block"

DEPENDENCIES (this task): NONE new. serde (1.0 + derive) and toml (0.9) are ALREADY
                           Cargo deps (Cargo.toml lines 12, 21). No Cargo.toml edit.

UPSTREAM (already present — consumed unchanged):
  - pub enum Pattern { Single(String), Parts(String, String) } — P2.M1.T3.S2,
    src/core/pattern.rs:1102-1113. Imported via `use crate::core::pattern::Pattern;`.
    Its serde #[serde(untagged)] gives string->Single, 2-array->Parts,
    wrong-length->error FOR FREE (no work here).

DOWNSTREAM CONSUMERS (later subtasks — do NOT implement them here):
  - P3.M1.T1.S2 (parse_rules + effective resolution): will read
    rule_set.host.disable_firmware_config and each rule's
    disable_firmware_config: Option<bool> (None => inherit host). The Option<bool>
    shape is THIS task's contract.
  - P3.M1.T2.S1 (evaluate): will iterate rule_set.layer_rules (first-match) /
    callback_rules (all-match), reading rule.layer, rule.enable/disable,
    rule.case_sensitive. THIS task ships those field types verbatim.

CONFIG: none (no new config knob — rules.toml path resolution is P3.M1.T1.S2 / platform code).
ROUTES: none (no CLI surface this subtask — --validate-rules is P5.M1.T1.S1).
DATABASE: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles clean, no NEW warnings. If rustc errors (e.g. a missing pub,
# a typo'd attribute, a duplicate Pattern definition from G1), READ it and fix.

# Confirm the deliverables are present:
test -f src/core/rules.rs && echo "rules.rs created" || echo "MISSING rules.rs"
grep -n 'pub mod rules;' src/core/mod.rs          # expect one line, between pattern & types
grep -nE 'pub struct (RuleSet|HostDefaults|LayerRule|CallbackRule)' src/core/rules.rs  # expect 4
grep -n 'use crate::core::pattern::Pattern' src/core/rules.rs   # expect 1 (the import, G1)
grep -n 'impl Default for HostDefaults' src/core/rules.rs       # expect 1 (manual impl, G4)
grep -cE '#\[serde\(rename = "match"\)\]' src/core/rules.rs     # expect 2 (Layer + Callback)
# Confirm NO duplicate Pattern definition (G1):
! grep -nE 'pub enum Pattern' src/core/rules.rs || echo "FAIL: Pattern redefined (G1 violation)"
```

### Level 2: Unit Tests — deserialization contract (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# Single-threaded is MANDATORY crate-wide (shared debouncer state, AGENTS.md).
cargo test --bin qmkonnect rules -- --test-threads=1
# Expected: all ~9 test_rules_* pass (full §9 example, defaults, Option<bool> None
# inheritance, required-field strictness, Pattern variant dispatch, Default propagation).
# Filter to individual tests to see them in isolation:
cargo test --bin qmkonnect rules::tests::test_rules_full_section9_example_parses -- --test-threads=1
cargo test --bin qmkonnect rules::tests::test_rules_missing_layer_errors -- --test-threads=1
cargo test --bin qmkonnect rules::tests::test_rules_default_propagates -- --test-threads=1
```

### Level 3: Full-crate regression (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL bin tests green — new rules::tests + pattern (incl. the 1225-case
# P2.M1.T4.S1 parity corpus if landed) + notifier + types + mod. Proves the new
# module + the `pub mod rules;` line compile in the full crate context and didn't
# break module resolution.

# Confirm the change surface is exactly two files:
git status --short
# Expected:
#   new file:    src/core/rules.rs
#   modified:    src/core/mod.rs        (one line: `pub mod rules;`)
git diff --stat
# Expected: only those two files; nothing in Cargo.toml, pattern.rs, notifier.rs, etc.
```

### Level 4: Schema-fidelity cross-check (optional, high-confidence)

```bash
# Manually deserialize a hand-written rules.toml to eyeball the nested struct (the
# `Debug` derive makes this readable). Confirms the §9 shapes round-trip end-to-end.
cd /home/dustin/projects/qmkonnect
cat > /tmp/rules_check.toml <<'EOF'
[host]
disable_firmware_config = false

[[layer_rules]]
match = "alacritty"
layer = 224
disable_firmware_config = true

[[callback_rules]]
match = ["*chrome*", "*claude*"]
enable = ["vim_lazy"]
EOF
# (No standalone harness exists yet — parse_rules(path) is P3.M1.T1.S2. This Level-4
#  check is covered FUNCTIONALLY by test_rules_full_section9_example_parses in Level 2,
#  which feeds the verbatim §9 TOML through toml::from_str::<RuleSet>. No extra step
#  is required for this task; the Level-2 test IS the schema-fidelity gate.)
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean (no NEW warnings).
- [ ] `cargo test --bin qmkonnect rules -- --test-threads=1` — all `test_rules_*` pass.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` — full crate green (no regression).
- [ ] `git status` shows exactly TWO files: `src/core/rules.rs` (new) + `src/core/mod.rs` (modified).

### Feature Validation (schema fidelity)
- [ ] **RuleSet** derives `Debug, Deserialize, Default`; `host`/`layer_rules`/`callback_rules` all `#[serde(default)]`.
- [ ] **HostDefaults** derives `Debug, Deserialize`; `disable_firmware_config: bool` with `#[serde(default)]`; manual `impl Default` → `false`.
- [ ] **LayerRule** derives `Debug, Deserialize`; `#[serde(rename="match")] pattern: Pattern`; `layer: u8` (required); `case_sensitive: bool` (default false); `disable_firmware_config: Option<bool>` (default None).
- [ ] **CallbackRule** derives `Debug, Deserialize`; `#[serde(rename="match")] pattern: Pattern`; `enable`/`disable: Vec<String>` (default empty); `case_sensitive: bool` (default false); `disable_firmware_config: Option<bool>` (default None).
- [ ] **Pattern is imported** (`use crate::core::pattern::Pattern;`), NOT redefined (G1).
- [ ] **Full §9 example parses** to the expected nested values (test 1).
- [ ] **Missing `[host]`** → `host.disable_firmware_config == false` (test 2).
- [ ] **Empty TOML** → `RuleSet::default()`-equivalent (test 3).
- [ ] **Override absent** → `disable_firmware_config == None` (test 4, G5).
- [ ] **`enable`/`disable` default to empty** when omitted (test 5).
- [ ] **`match` string → Single; 2-array → Parts** (test 6).
- [ ] **Missing required `layer`** → `is_err()` (test 7, G6).
- [ ] **Missing required `match`** → `is_err()` (test 8, G6).
- [ ] **`RuleSet::default()`** propagates (test 9, G4).

### Code Quality Validation
- [ ] All four structs + all fields are `pub` (G10); no dead-code warnings.
- [ ] Mode-A rustdoc on each struct with ` ```toml ` fenced §9 examples (G2 — NOT runnable Rust doctests).
- [ ] `pub mod rules;` added to mod.rs in alpha position (G9); nothing else in mod.rs changed.
- [ ] No `Serialize` derive (rules are read-only — §9 mandates Deserialize only).
- [ ] No `parse_rules()` / resolution / `evaluate()` / path logic (G7 — scope boundary).
- [ ] No new Cargo dependencies (serde + toml already present).
- [ ] No `unsafe`; no `static`; no `mut` at module scope.

### Documentation & Deployment
- [ ] Mode-A rustdoc (code-level) present on all four structs; cites `spec/HOST_RULES.md` §9.
- [ ] TOML examples in the rustdoc match §9 verbatim (`research/notes.md` §2).
- [ ] No `docs/*.md` or README changes this task (Mode A — code-level docs only).

---

## Anti-Patterns to Avoid

- ❌ Do NOT redefine `Pattern` in rules.rs. It is P2.M1.T3.S2's type in `pattern.rs`
      (Complete). `use crate::core::pattern::Pattern;` only (G1). HOST_RULES.md §9's
      in-snippet `Pattern` is illustrative.
- ❌ Do NOT add `Serialize` to the structs. §9 mandates `Deserialize`-only (rules.toml
      is read, never written back). The Config struct derives both — that's Config.
- ❌ Do NOT implement `parse_rules()`, effective-`disable_firmware_config` resolution,
      `evaluate()`, `HostContext`, or file-path resolution. Those are P3.M1.T1.S2 /
      P3.M1.T2.S1 / P4. This task is the bare data model (G7).
- ❌ Do NOT change `disable_firmware_config: Option<bool>` to `bool`. `None` is the
      "inherit `[host]`" signal (G5); collapsing it loses the inherit semantics.
- ❌ Do NOT add `#[serde(default)]` to `pattern` or `layer`. They are REQUIRED — a rule
      missing `match` or `layer` must be a parse error (G6, for `--validate-rules`).
- ❌ Do NOT write runnable Rust doctests (` ``` `) that `use qmkonnect::core::rules::...`.
      This is a binary-only crate (no lib.rs); such doctests won't compile under
      `cargo test --doc`, and `cargo test --bin` doesn't run doctests anyway. Use
      ` ```toml ` fences for the §9 examples (G2). If a Rust example is needed, mark it
      ```` ```rust,ignore ````.
- ❌ Do NOT drop the `#[serde(rename = "match")]` on `pattern`. It is the one NECESSARY
      rename (Rust field `pattern`, TOML key `match`). The `rename = "layer_rules"`/
      `"callback_rules"` are no-ops and may be kept or dropped (G3).
- ❌ Do NOT use `#[derive(Default)]` on `HostDefaults`. §9 specifies a MANUAL `impl
      Default` (→ `false`) to make the "stack" default explicit (G4). (Behaviorally
      identical to derive, but follow the spec.)
- ❌ Do NOT edit anything in mod.rs other than the single `pub mod rules;` line (G9).
      No Config change, no startup wiring.
- ❌ Do NOT run tests multi-threaded — `cargo test --bin qmkonnect -- --test-threads=1`
      (shared debouncer state, G8/AGENTS.md).
- ❌ Do NOT edit `PRD.md`, any `tasks.json`, `prd_snapshot.md`, `spec/HOST_RULES.md`,
      `Cargo.toml`, or any `plan/` file other than this item's own `PRP.md` + `research/`.

---

## Confidence Score: 9/10

This is a well-bounded **data-model transcription**: the four struct shapes are
given **verbatim** in `spec/HOST_RULES.md` §9 (reproduced in `research/notes.md` §1
with every derive/attribute/field/default), the canonical TOML examples to embed in
the rustdoc are reproduced verbatim (§2), and the sole input dependency (`Pattern`)
is already in-tree and Complete (P2.M1.T3.S2, confirmed by reading `pattern.rs`).
The repo conventions (mod.rs module registration, `#[serde(default)]` + `toml::from_str`
idiom, per-module `#[cfg(test)] mod tests`) are all confirmed from the existing
`mod.rs`/`pattern.rs`. serde + toml are already Cargo deps (no `Cargo.toml` change).
The 10 gotchas (notably G1 no-redefine-Pattern, G2 doctest-doesn't-compile-in-bin-crate,
G5 Option<bool>=None, G6 required-fields-error) are each pinned to a concrete failure
mode caught by the ~9 deserialization tests. The 1-point reservation is for: (a) the
doctest-vs-TOML-fence distinction (G2 — mitigated by mandating ` ```toml ` fences, but
an implementer who copies pattern.rs's runnable-doctest style would add an untested
doctest); (b) the redundant-renames decision (G3 — harmless either way, but a pedantic
implementer might bikeshed it); and (c) the §9 example's exact `Pattern` equality
assertions in tests (Pattern derives `PartialEq`, so `== Pattern::Single(s.into())`
works — but the 2-array `Parts` assertion needs both tuple members correct). All three
are low-risk and immediately caught by `cargo test`. Scope is cleanly bounded from the
upstream `Pattern` (untouched, imported) and the downstream `parse_rules()`/`evaluate()`
(not implemented — G7), so there is no risk of over- or under-building.
# Research Notes — P3.M1.T1.S1: RuleSet/HostDefaults/LayerRule/CallbackRule structs (serde)

> **Scope of THIS subtask:** define the four deserializable structs + register the
> module + Mode-A rustdoc. It does NOT implement `parse_rules()` (P3.M1.T1.S2),
> effective-`disable_firmware_config` resolution (P3.M1.T1.S2), or `evaluate()`
> (P3.M1.T2.S1). It ships the **data model** those consumers read.

---

## 1. Canonical Rust model — `spec/HOST_RULES.md` §9 (VERBATIM)

This is the single source of truth for the struct shapes. Every field, derive,
attribute, default, and the `Pattern` import come from here.

```rust
use serde::Deserialize;
use crate::core::pattern::Pattern;   // ← from P2.M1.T3.S2 (already in-tree); NOT redefined here

#[derive(Debug, Deserialize, Default)]
pub struct RuleSet {
    #[serde(default)] pub host: HostDefaults,
    #[serde(default, rename = "layer_rules")]    pub layer_rules: Vec<LayerRule>,
    #[serde(default, rename = "callback_rules")] pub callback_rules: Vec<CallbackRule>,
}

#[derive(Debug, Deserialize)]
pub struct HostDefaults {
    #[serde(default)] pub disable_firmware_config: bool,   // default false (stack)
}
impl Default for HostDefaults { fn default() -> Self { Self { disable_firmware_config: false } } }

#[derive(Debug, Deserialize)]
pub struct LayerRule {
    #[serde(rename = "match")] pub pattern: Pattern,
    pub layer: u8,
    #[serde(default)] pub case_sensitive: bool,
    #[serde(default)] pub disable_firmware_config: Option<bool>,  // None => inherit [host]
}

#[derive(Debug, Deserialize)]
pub struct CallbackRule {
    #[serde(rename = "match")] pub pattern: Pattern,
    #[serde(default)] pub enable: Vec<String>,
    #[serde(default)] pub disable: Vec<String>,
    #[serde(default)] pub case_sensitive: bool,
    #[serde(default)] pub disable_firmware_config: Option<bool>,  // None => inherit [host]
}

// Pattern is NOT redefined here — it lives in pattern.rs (P2.M1.T3.S2):
//   #[derive(Debug, Clone, PartialEq, Deserialize)] #[serde(untagged)]
//   pub enum Pattern { Single(String), Parts(String, String) }
// rules.rs only `use`s it.
```

**Decision — the redundant `rename`s:** the `rename = "layer_rules"` and
`rename = "callback_rules"` attributes on `RuleSet` are **no-ops** (the Rust
field name already equals the TOML key `[[layer_rules]]` / `[[callback_rules]]`).
Keeping them matches the spec literally and documents intent harmlessly; dropping
them produces byte-identical deserialization. The ONLY rename that is *necessary*
is `#[serde(rename = "match")]` on the `pattern` field (Rust field `pattern`, TOML
key `match`). **Recommendation: keep all three verbatim from §9** (spec fidelity,
zero behavioral cost).

## 2. The canonical TOML examples (verbatim from `spec/HOST_RULES.md` §9)

These are the exact strings to embed in the struct rustdoc (Mode-A docs). Using
` ```toml ` fences renders them as documentation text — they are NOT compiled as
doctests (see §7 gotcha).

```toml
[host]
disable_firmware_config = false   # global default: false = stack (board runs), true = replace
# On no match the host layer is always cleared and all host callbacks disabled.

# Layer rules: FIRST match wins. One host layer active at a time (>= 224).
[[layer_rules]]
match = "alacritty"                       # class-only pattern
layer = 224
disable_firmware_config = true           # optional override (default inherits [host])

[[layer_rules]]
match = ["*chrome*", "*youtube*"]         # [class_pattern, title_pattern] (== WT())
layer = 225
case_sensitive = false                    # optional, default false

# Callback rules: ALL matches fire. Names come from the keyboard's registry
# (run `qmkonnect --list-callbacks` to see them).
[[callback_rules]]
match = "neovide"
enable = ["vim_lazy", "disable_vim"]      # run on focus-in
disable = ["vim_lazy"]                    # optional: force-off override

[[callback_rules]]
match = ["*chrome*", "*claude*"]
enable = ["vim_lazy", "disable_vim"]
disable_firmware_config = true           # for this window, skip the string -> board can't match
```

## 3. The `Pattern` dependency contract (P2.M1.T3.S2 — COMPLETE, in-tree)

- **Location:** `src/core/pattern.rs` lines 1102–1113.
- **Definition (confirmed by reading the file):**
  ```rust
  #[derive(Debug, Clone, PartialEq, Deserialize)]
  #[serde(untagged)]
  pub enum Pattern {
      Single(String),
      Parts(String, String),
  }
  ```
- **Import in rules.rs:** `use crate::core::pattern::Pattern;` (full crate path —
  `crate::core::pattern::Pattern`, since rules.rs lives in `crate::core::rules`).
  Inside the same parent module, `use super::pattern::Pattern;` is equivalent and
  slightly tighter; either compiles. The in-tree pattern.rs already has
  `pub fn match_pattern(&Pattern, app_class, title, case_sensitive) -> bool`
  (line 1155) — NOT needed by THIS task (that's P3.M1.T2.S1's consumer), but
  confirms `Pattern` is the public type to embed.
- **serde dispatch (free, no custom visitor):** bare TOML string → `Single`;
  2-element array → `Parts`; 1/3-element array or non-string → **error** (desired
  strictness for the future `--validate-rules`).
- **CRITICAL: do NOT redefine `Pattern` in rules.rs.** HOST_RULES.md §9 *shows* a
  `Pattern` enum in its snippet, but that is illustrative — the real, shipped type
  is P2.M1.T3.S2's in `pattern.rs`. Redefining it creates a duplicate-type error
  and breaks the `use`. The item description INPUT line ("Pattern enum from
  P2.M1.T3.S2") confirms the import approach.

## 4. Repo conventions to follow (from `src/core/mod.rs`)

- **Module registration:** `src/core/mod.rs` currently declares (top of file):
  ```rust
  pub mod notifier;
  pub mod pattern;
  pub mod types;
  ```
  THIS task adds ONE line — `pub mod rules;` — in alphabetical-ish position (after
  `pattern;`, before `types;` matches the existing alpha ordering: notifier <
  pattern < rules < types). Single-line additive edit; nothing else in mod.rs
  changes.
- **Struct derive convention:** `Config` in mod.rs uses
  `#[derive(serde::Deserialize, serde::Serialize, Default)]`. For rules, §9
  mandates `Deserialize`-only (rules are READ, never written back) — so do NOT add
  `Serialize`. `RuleSet` additionally derives `Default` (needed so a missing/empty
  `rules.toml` deserializes to an all-default ruleset). `LayerRule`/`CallbackRule`
  derive `Debug, Deserialize` only (no `Default` — they have a required `pattern`
  + `layer`). `Pattern` itself already derives `Debug, Clone, PartialEq,
  Deserialize` (P2.M1.T3.S2).
- **`#[serde(default)]` on container fields:** mod.rs's `Config` uses
  `#[serde(default)]` pervasively so a partial TOML still parses. rules.rs follows
  the same idiom: every optional field gets `#[serde(default)]`. Required fields
  (`pattern`, `layer`) get none (a rule missing `match` or `layer` is a TOML error
  — correct for `--validate-rules`).
- **TOML parsing entrypoint:** `toml::from_str` (toml = "0.9", Cargo.toml line 21;
  used in `mod.rs::parse_config`). serde 1.0 + `derive` feature is a Cargo dep
  (line 12). **No Cargo.toml change.**
- **Test module convention:** each core module has its own
  `#[cfg(test)] mod tests { use super::*; ... }` (mod.rs, pattern.rs, types.rs,
  notifier.rs all do). rules.rs gets its own, appended at the file's tail.

## 5. Field-by-field default semantics (why each attribute is there)

| struct          | field                       | type               | attr                         | absent → value              | why |
|-----------------|-----------------------------|--------------------|------------------------------|-----------------------------|-----|
| `RuleSet`       | `host`                      | `HostDefaults`     | `#[serde(default)]`          | `HostDefaults::default()`   | a rules.toml with no `[host]` table still parses; global default applies |
| `RuleSet`       | `layer_rules`               | `Vec<LayerRule>`   | `#[serde(default)]`          | empty `Vec`                 | a rules.toml with only callbacks (or vice-versa) parses |
| `RuleSet`       | `callback_rules`            | `Vec<CallbackRule>`| `#[serde(default)]`          | empty `Vec`                 | same |
| `HostDefaults`  | `disable_firmware_config`   | `bool`             | `#[serde(default)]`          | `false` (= stack)           | global default = board runs its own rules |
| `LayerRule`     | `pattern` (TOML `match`)    | `Pattern`          | `#[serde(rename="match")]`   | **required** (errors if absent) | a rule with no `match` is malformed |
| `LayerRule`     | `layer`                     | `u8`               | (none)                       | **required** (errors if absent) | a layer rule with no layer is meaningless |
| `LayerRule`     | `case_sensitive`            | `bool`             | `#[serde(default)]`          | `false`                     | firmware default is case-insensitive |
| `LayerRule`     | `disable_firmware_config`   | `Option<bool>`     | `#[serde(default)]`          | `None`                      | `None` ⇒ inherit `[host]` default (resolved by P3.M1.T1.S2) |
| `CallbackRule`  | `pattern` (TOML `match`)    | `Pattern`          | `#[serde(rename="match")]`   | **required**                | as above |
| `CallbackRule`  | `enable`                    | `Vec<String>`      | `#[serde(default)]`          | empty `Vec`                 | a rule may only `disable` |
| `CallbackRule`  | `disable`                   | `Vec<String>`      | `#[serde(default)]`          | empty `Vec`                 | a rule may only `enable` |
| `CallbackRule`  | `case_sensitive`            | `bool`             | `#[serde(default)]`          | `false`                     | firmware default is case-insensitive |
| `CallbackRule`  | `disable_firmware_config`   | `Option<bool>`     | `#[serde(default)]`          | `None`                      | `None` ⇒ inherit `[host]` default |

**`Default` propagation:** `RuleSet: Default` requires every field type to impl
`Default`. `HostDefaults` has a manual `Default` impl (→ `false`); `Vec<T>:
Default` (→ empty). ✓ Both satisfied. Note: `#[derive(Default)]` on
`HostDefaults` would also yield `false` (bool's default) — the manual impl in §9
is explicit-by-intent, not a behavioral necessity. Follow §9 verbatim (manual
impl).

## 6. serde `rename = "match"` — correctness check

- `match` is a **Rust keyword**, but serde's `rename` takes a plain `&str` — no
  conflict. Confirmed idiomatic (e.g. the existing pattern.rs serde tests already
  use `#[serde(rename="match")]` on a helper `W` struct — see P2.M1.T3.S2 PRP).
- `match` is a valid **TOML bare key** (TOML keys are arbitrary UTF-8; `match` is
  not reserved in TOML). Confirmed: the §9 example uses `match = "alacritty"`.
- `toml::from_str::<RuleSet>(...)` therefore maps the TOML `[[layer_rules]]
  match = "alacritty"` table-array element to `LayerRule { pattern:
  Pattern::Single("alacritty".into()), .. }`. No custom visitor.

## 7. Gotchas (with pinning)

- **G1 — do NOT redefine `Pattern`.** It is P2.M1.T3.S2's type in `pattern.rs`.
  rules.rs `use`s it (`use crate::core::pattern::Pattern;`). HOST_RULES.md §9
  shows a `Pattern` enum in its snippet ONLY to make the doc self-contained — the
  real definition is already shipped. Redefining → duplicate-definition compile
  error.
- **G2 — binary-only crate, doctests don't run under `--bin`.** There is NO
  `lib.rs` (`src/main.rs` declares `mod core;`). `cargo test --bin qmkonnect`
  (the AGENTS.md mandate) runs **unit tests only**, NOT doctests. A runnable Rust
  doctest (` ``` ` block) that does `use qmkonnect::core::rules::RuleSet;` would
  NOT compile under `cargo test --doc` (no library target to link). **Mitigation:
  embed the §9 examples as ` ```toml ` fenced blocks** — rustdoc renders these as
  documentation text and does NOT compile them as doctests. This is exactly what
  "Mode-A rustdoc with TOML examples" asks for. (If a runnable Rust usage example
  is desired, mark it ``` ```rust,ignore ``` so it never compiles.) The existing
  pattern.rs has two runnable doctests using `qmkonnect::core::pattern::...` —
  they are likewise untested by `--bin`; do NOT add more.
- **G3 — redundant `rename`s are no-ops.** `rename = "layer_rules"` on field
  `layer_rules` (and `callback_rules`) changes nothing — the default serde
  behavior already maps field name → same TOML key. Keep them (spec fidelity) or
  drop them (cleaner) — identical behavior. The NECESSARY rename is
  `rename = "match"` on `pattern`.
- **G4 — `RuleSet: Default` needs `HostDefaults: Default`.** §9 gives
  `HostDefaults` a manual `Default` impl (not `#[derive(Default)]`). Both produce
  `disable_firmware_config: false`. Follow §9 (manual impl) so the
  "default = stack" intent is explicit and matches the spec.
- **G5 — `Option<bool>` default is `None`, not `false`.** A `disable_firmware_config:
  Option<bool>` field with `#[serde(default)]` defaults to `None` (not `Some(false)`).
  This is correct: `None` means "inherit the `[host]` global default" (resolved by
  P3.M1.T1.S2). Do NOT change to `bool`.
- **G6 — required fields error if absent.** `pattern` (TOML `match`) and
  `layer` have NO `#[serde(default)]`, so a `[[layer_rules]]` missing either is a
  deserialization error. This is DESIRED (malformed rules.toml must fail
  `--validate-rules`, P5). Do not add defaults to them.
- **G7 — do NOT implement logic here.** No `parse_rules()`, no
  `effective_disable_firmware_config()`, no `evaluate()`, no file-path resolution.
  Those are P3.M1.T1.S2 / P3.M1.T2.S1. THIS task is the bare data model + module
  registration + rustdoc + (recommended) deserialization tests.
- **G8 — single-threaded tests crate-wide.** `cargo test --bin qmkonnect --
  --test-threads=1` (shared debouncer state in notifier.rs, AGENTS.md).
- **G9 — module registration ordering.** Add `pub mod rules;` to mod.rs between
  `pub mod pattern;` and `pub mod types;` (alpha order). One line.
- **G10 — `pub` visibility.** All four structs and the re-used `Pattern` are
  `pub` (consumed cross-module by P3.M1.T1.S2/T2 via
  `use crate::core::rules::{RuleSet, LayerRule, CallbackRule, HostDefaults};`).
  Fields are `pub` too (§9 shows `pub` on every field). The crate carries no
  `#![allow(dead_code)]` globally, but `pub` items don't warn as dead code.

## 8. Recommended deserialization test set (the validation gate)

No upstream test pattern for rules exists yet (rules.rs is new). Mirror the
mod.rs/pattern.rs serde-round-trip idiom: `toml::from_str::<RuleSet>(TOML).unwrap()`
then assert fields. Cover:
1. **Full §9 example** parses → assert host default, both layer_rules
   (Single+layer+override, Parts+layer+case_sensitive), both callback_rules
   (enable/disable lists, override).
2. **Empty/missing `[host]`** → `host.disable_firmware_config == false` (the
   `#[serde(default)]` + manual `Default`).
3. **No `[host]` AND no rules** (empty string) → `RuleSet::default()`-equivalent.
4. **Layer rule inherits `disable_firmware_config`** (field absent) → `None`.
5. **Callback rule `enable`/`disable` default to empty** when omitted.
6. **`match` as single string → `Pattern::Single`**; as 2-array → `Pattern::Parts`**.
7. **Missing required `layer` → `is_err()`** (G6 strictness).
8. **Missing required `match` → `is_err()`** (G6 strictness).
9. **`RuleSet::default()`** yields `host.disable_firmware_config == false`, empty
   vecs (proves `Default` propagation).

~9 tests. Naming prefix `test_rules_` (disjoint from `test_mp_`/`test_parity_`/
`test_pattern_serde_` in pattern.rs and the `test_*` in mod.rs).

## 9. Downstream consumer contracts (do NOT implement — just satisfy)

- **P3.M1.T1.S2 (`parse_rules()` + effective resolution):** will
  `use crate::core::rules::{RuleSet, HostDefaults, LayerRule, CallbackRule};`
  and read `.host.disable_firmware_config`, `.layer_rules`, `.callback_rules`,
  per-rule `.disable_firmware_config: Option<bool>` (None ⇒ inherit host). The
  `Option<bool>` shape is THIS task's contract for that resolution.
- **P3.M1.T2.S1 (`evaluate()`):** will iterate `rule_set.layer_rules` (first-match
  via `pattern::match_pattern(&rule.pattern, &wi.app_class, &wi.title,
  rule.case_sensitive)`) and `rule_set.callback_rules` (all-match), producing a
  `HostContext`. It reads `rule.layer: u8`, `rule.enable/disable: Vec<String>`.
  THIS task ships those field types verbatim.

## 10. Scope boundary (do NOT do)

- ❌ `parse_rules(path)` / file-path resolution (P3.M1.T1.S2 / platform config
  paths). This task: structs only.
- ❌ `effective_disable_firmware_config()` resolution logic (P3.M1.T1.S2).
- ❌ `evaluate()` / `HostContext` (P3.M1.T2.S1).
- ❌ Redefine `Pattern` (it's P2.M1.T3.S2's, in pattern.rs).
- ❌ Touch `mod.rs` beyond the one `pub mod rules;` line.
- ❌ Touch `Cargo.toml` (serde + toml already deps).
- ❌ Touch `pattern.rs`, `notifier.rs`, `types.rs`.
- ❌ `docs/*.md` / README (Mode A = code-level rustdoc only).
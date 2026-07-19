# PRP — P3.M1.T1.S2: effective_disable_firmware_config + parse_rules() + get_rules_paths()

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This task ADDS three functions to
> `src/core/rules.rs` (the file created by the immediately-preceding item
> P3.M1.T1.S1, which ships the `RuleSet`/`HostDefaults`/`LayerRule`/`CallbackRule`
> structs). The three functions are: (1) the per-rule
> `effective_disable_firmware_config` resolution primitive, (2) a `parse_rules`
> file reader that is a drop-in twin of `src/core/mod.rs::parse_config`, and (3) a
> `get_rules_paths` path resolver that **delegates** to the existing
> `crate::platforms::get_config_paths()` and swaps the final filename
> `config.toml → rules.toml`. They are the **file-IO + path-resolution +
> per-rule-primitive** layer between the data model (S1) and the `evaluate()`
> engine (P3.M1.T2.S1). PRD §9 (`rules.toml` Schema Reference) lines 455–456 and
> §8 point 1 (line 305) are the spec sources of truth. **Consumes:** `RuleSet` +
> `Pattern` from P3.M1.T1.S1 / P2.M1.T3.S2. **Consumed downstream by:**
> P3.M1.T2.S1 (`evaluate()`), P4.M3.T1.S1 (notify_qmk host-context send),
> P5.M1.T1.S1 (`--validate-rules`), P5.M2.T1 (tray "Reload rules").

> **PARALLEL-EXECUTION NOTE:** THIS item runs in parallel with P3.M1.T1.S1 (which
> creates `src/core/rules.rs` + the four structs). The implementer may find S1's
> work in one of three states — see Task 1 / G0. The intended END STATE is ONE
> file containing BOTH S1's structs AND S2's three functions. This task is purely
> **additive** to rules.rs: it touches NO struct definition and NO existing test.

---

## Goal

**Feature Goal**: Add to `src/core/rules.rs` (i) a one-line per-rule
`effective_disable_firmware_config(rule_override: Option<bool>, host_default: bool)
-> bool` resolver (override-else-host-default), (ii) a `pub fn parse_rules(path:
&Path) -> Result<RuleSet, Box<dyn Error>>` that reads + deserializes a
`rules.toml` file (faithful twin of `parse_config`), and (iii) a `pub fn
get_rules_paths() -> Vec<PathBuf>` that derives the `rules.toml` candidate paths
by delegating to `crate::platforms::get_config_paths()` and swapping each path's
filename via `PathBuf::with_file_name("rules.toml")`. Each `pub fn` carries
Mode-A rustdoc. The functions are unit-tested with temp-file + pure-function +
transformation-invariant tests.

**Deliverable**:
1. **ADDITIONS to** `src/core/rules.rs` (the file S1 creates):
   - **4 std imports** at the top: `use std::error::Error;`, `use std::fs;`,
     `use std::path::Path;` (and `PathBuf` — grouped `use std::path::{Path, PathBuf};`
     matches mod.rs style). These join S1's existing `use crate::core::pattern::Pattern;`
     and `use serde::Deserialize;`.
   - **3 functions**:
     - `fn effective_disable_firmware_config(rule_override: Option<bool>, host_default: bool) -> bool` — module-private (`fn`, not `pub fn`, per contract); body `rule_override.unwrap_or(host_default)`.
     - `pub fn parse_rules(path: &Path) -> Result<RuleSet, Box<dyn Error>>` — reads file via `fs::read_to_string`, deserializes via `toml::from_str::<RuleSet>`, returns it. Mode-A `///` rustdoc.
     - `pub fn get_rules_paths() -> Vec<PathBuf>` — `crate::platforms::get_config_paths().into_iter().map(|p| p.with_file_name("rules.toml")).collect()`. Mode-A `///` rustdoc.
   - **~10 NEW unit tests** appended to the existing `#[cfg(test)] mod tests` block (S1's tests untouched): 4 truth-table tests for the primitive, 4 temp-file tests for `parse_rules`, 2 transformation-invariant tests for `get_rules_paths`.
2. **NO other files change.** `src/core/mod.rs` was already edited by S1 (`pub mod rules;`); platforms/ is untouched (we delegate, not duplicate). Cargo.toml unchanged.

**Success Definition**:
- `effective_disable_firmware_config` returns the 4 truth-table values (Some wins; None inherits host_default) — pure function, 4 tests.
- `parse_rules(&temp_file_with_section9_toml)` succeeds and yields a `RuleSet` whose `host.disable_firmware_config == false`, `layer_rules.len() == 2`, `layer_rules[0].disable_firmware_config == Some(true)`, etc. (end-to-end file read + serde deserialize through the S1 structs).
- `parse_rules(&nonexistent_path)` → `Err` (io::Error propagates).
- `parse_rules(&file_with_malformed_toml)` → `Err` (toml::de::Error).
- `parse_rules(&file_with_layer_rule_missing_layer)` → `Err` (S1's required-field strictness surfaces through the file path — the `--validate-rules` contract).
- `get_rules_paths()` returns exactly `crate::platforms::get_config_paths()` with each path's filename swapped to `rules.toml`, parent directory unchanged. Verified by a transformation-invariant test that needs NO env vars and passes on every platform.
- `cargo build --bin qmkonnect` compiles clean (no new warnings).
- `cargo test --bin qmkonnect -- --test-threads=1` green (S1's tests + the ~10 new tests + all existing crate tests; no regression).
- `git status` shows at most ONE changed file: `src/core/rules.rs` (the additions). (If S1 landed separately and was committed, this task's diff is rules.rs-only. If S1 is still in-flight in the same working tree, both sets of edits naturally coalesce into the one file — which is the intended end state.)

## User Persona (if applicable)

**Target User**: downstream qmkonnect subtasks that read/resolve `rules.toml`.
- **P3.M1.T2.S1** (`evaluate()`): calls `effective_disable_firmware_config(rule.disable_firmware_config, rule_set.host.disable_firmware_config)` per matched rule, then aggregates "replace iff ALL matched effective flags true".
- **P4.M3.T1.S1** (notify_qmk host-context send): calls `get_rules_paths()` to find & `parse_rules()` to load the ruleset at startup / on "Reload rules"; absent ⇒ string-only.
- **P5.M1.T1.S1** (`--validate-rules`): calls `get_rules_paths().find(exists).map(parse_rules)` and reports the `Result`'s error on malformed input.

**Use Case**: on each debounced window change (or on `--validate-rules` / tray "Reload rules"), the app resolves the `rules.toml` path, reads + deserializes it once into a typed `RuleSet`, then resolves each matched rule's effective `disable_firmware_config`. This task ships exactly those three building blocks.

**Pain Points Addressed**: gives the rules pipeline (a) a typed file reader with strict serde validation (malformed `rules.toml` fails loud — the `--validate-rules` guarantee), and (b) a DRY path resolver that inherits the platform's exact directory logic instead of re-encoding it.

## Why

- **PRD §9 (lines 455–456) defines effective resolution:** "A rule's effective `disable_firmware_config` = its override if `Some`, else the `[host]` default." This task ships that primitive so P3.M1.T2.S1 can aggregate it.
- **HOST_RULES.md §8 (point 1, line 305) fixes rules.toml's location:** "alongside `config.toml` (Linux `~/.config/qmk-notifier/rules.toml`, Windows `%APPDATA%\QMKonnect\`, macOS `~/Library/Application Support/QMKonnect/`)." `get_rules_paths` enforces "same directory" by deriving from the existing `get_config_paths()` — zero duplication, zero drift.
- **`parse_config` (mod.rs:91) is the established reader pattern.** `parse_rules` is its twin — same error type (`Box<dyn Error>`), same two-step (`fs::read_to_string` → `toml::from_str`), so the codebase has ONE config-file-reading idiom, not two.
- **Unblocks P3.M1.T2.S1, P4.M3.T1.S1, P5.M1.T1.S1, P5.M2.T1.** All four call `get_rules_paths()` + `parse_rules()` + the effective-resolution primitive.
- **`--validate-rules` (P5.M1.T1.S1) depends on parse_rules failing strict.** The required-`match`/required-`layer` strictness shipped by S1 (no `#[serde(default)]`) flows through `parse_rules` as a hard `Err` — exactly what the CLI reports.

## What

Pure additions to `src/core/rules.rs`: 4 std imports + 3 functions + ~10 tests + Mode-A rustdoc on the two `pub fn`s. No struct changes, no new deps, no platform `cfg` duplication (delegation only), no CLI/tray/notifier wiring.

### Success Criteria
- [ ] **4 std imports added** to rules.rs: `std::error::Error`, `std::fs`, `std::path::Path`, `std::path::PathBuf` (S1's `Pattern`/`Deserialize` imports untouched).
- [ ] **`fn effective_disable_firmware_config(rule_override: Option<bool>, host_default: bool) -> bool`** — module-private (`fn`, NOT `pub fn`); body `rule_override.unwrap_or(host_default)`.
- [ ] **`pub fn parse_rules(path: &Path) -> Result<RuleSet, Box<dyn Error>>`** — reads via `fs::read_to_string(path)?`, deserializes via `toml::from_str(&text)?`, returns `Ok(rules)`. Mode-A `///` rustdoc.
- [ ] **`pub fn get_rules_paths() -> Vec<PathBuf>`** — body `crate::platforms::get_config_paths().into_iter().map(|p| p.with_file_name("rules.toml")).collect()`. Mode-A `///` rustdoc.
- [ ] `parse_rules` uses `with_file_name` (NOT string-replace) — see G1.
- [ ] `get_rules_paths` contains NO `#[cfg(target_os = ...)]` (delegates to platforms, not re-cfg'd) — see G2.
- [ ] `parse_rules` takes a single `&Path` and does NOT iterate paths — see G3.
- [ ] Error type is `Box<dyn Error>` (not `+ Send + Sync`) — see G4.
- [ ] `effective_disable_firmware_config` is NOT `pub` (module-private, per contract) — see G5.
- [ ] No `evaluate()` / aggregation / `HostContext` logic (P3.M1.T2.S1) — see G6.
- [ ] No callback-name validation in parse_rules (handshake job, P4.M2) — see G7.
- [ ] Mode-A rustdoc on `parse_rules` and `get_rules_paths` uses ` ```rust,ignore ` or prose (NOT bare ` ``` ` runnable doctests — binary-only crate, G8).
- [ ] ~10 new tests appended to the `#[cfg(test)] mod tests` block (S1's tests untouched) — see Implementation Tasks Task 4.
- [ ] `cargo build --bin qmkonnect` clean; `cargo test --bin qmkonnect -- --test-threads=1` green; `git status` = rules.rs only.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge of this codebase can implement this
using only this PRP, because: (a) the EXACT `parse_config` twin pattern (every
line, error type, two-step body) is reproduced verbatim in `research/notes.md` §3
and pinned to `src/core/mod.rs:91-98`; (b) the `get_config_paths()` dispatcher +
all three platform implementations (every path, every env var, every fallback)
are tabulated in §4, confirming every path ends in a literal `config.toml`
filename so `with_file_name("rules.toml")` is universally correct; (c) the
`effective_disable_firmware_config` primitive's one-line body + 4-row truth table
are in §2 (spec quote + implementation); (d) the upstream S1 contract (imports,
struct shapes, test module) is reproduced in §1 so the implementer knows exactly
what already exists in rules.rs; (e) the parallel-execution file-state handling
(G0) is spelled out; (f) `tempfile` availability for file-IO tests is confirmed
(both `[dev-dependencies]` line 31 and regular dep line 38, with
`hyprland.rs:597-625` precedent); (g) the dependency direction `core → platforms`
is confirmed as established (`mod.rs:73`, `notifier.rs:36` already call
`crate::platforms::get_config_paths()`); (h) 12 gotchas are pinned (G0 parallel,
G1 with_file_name, G2 delegate-don't-recfg, G3 single-path, G4 error-type, G5
private-fn, G6 no-aggregation, G7 no-name-validation, G8 doctest, G9
single-thread, G10 no-env-mutation, G11 additive-only, G12 RuleSet-in-scope); (i)
the 10-test plan with verbatim code is in §6; (j) the downstream consumer
contracts are in §8; (k) the scope wall is in §9.

### Documentation & References

```yaml
# MUST READ — the spec source of truth (effective resolution + rules.toml location)
- file: spec/HOST_RULES.md
  why: "§9 lines 455-456 define effective_disable_firmware_config ('override if Some,
        else [host] default'). §8 point 1 (line 305) fixes rules.toml location as
        'alongside config.toml' (same directory). §4 line 140 restates the 'replace iff
        EVERY matched rule effective==true' aggregation (CONTEXT for why the primitive
        exists; aggregation itself is P3.M1.T2.S1, NOT this task)."
  section: "## 9. rules.toml Schema Reference (the 'effective' paragraph at the end)"
  gotcha: "§4's 'replace iff ALL matched rules' is the DOWNSTREAM aggregation — do NOT
           implement it here (G6). This task ships the per-rule primitive only."

# MUST READ — the verbatim research (THIS task's full contract)
- file: plan/002_637d65b6e9b8/P3M1T1S2/research/notes.md
  why: "§1 reproduces the S1 upstream contract (rules.rs imports + structs + test module).
        §2 gives the primitive body + 4-row truth table. §3 reproduces parse_config
        VERBATIM (the twin to mirror) with line refs. §4 tabulates every platform's
        get_config_paths() output + the with_file_name rationale. §5 lists the exact
        imports to add. §6 is the 10-test plan with verbatim code. §7 has 12 gotchas.
        §8/§9 are consumer contracts + scope wall."

# MUST READ — the pattern parse_rules mirrors EXACTLY (src/core/mod.rs:91-98)
- file: src/core/mod.rs
  why: "lines 91-98 define parse_config(config_path: &Path) -> Result<Config, Box<dyn Error>>
        { fs::read_to_string(config_path)?; toml::from_str(&config_str)?; Ok(config) }.
        parse_rules is a drop-in twin (Config→RuleSet). The error type Box<dyn Error>
        (NOT +Send+Sync), the `use std::{error::Error, fs, path::Path}` import grouping,
        and the 'no normalize/validate' comment are all conventions to copy."
  pattern: "two-step read-then-deserialize; `?` propagates both io::Error and toml::de::Error
            into Box<dyn Error>."
  gotcha: "do NOT add Send+Sync to the error type (G4); do NOT iterate paths in parse_rules
           (G3 — it takes ONE path, like parse_config)."

# MUST READ — the function get_rules_paths DELEGATES to (src/platforms/mod.rs:63-77)
- file: src/platforms/mod.rs
  why: "get_config_paths() is the cfg-dispatcher returning Vec<PathBuf> for linux/windows/macos
        (empty Vec elsewhere). get_rules_paths() calls THIS then maps with_file_name. The
        dependency direction core→platforms is ALREADY established: mod.rs:73
        (configured_timing) and notifier.rs:36 both call crate::platforms::get_config_paths()."
  pattern: "delegate, do NOT re-cfg per platform (G2)."
  gotcha: "do NOT copy the 3 platform cfg blocks into rules.rs — that drifts from the real
           resolver (e.g. linux.rs:119 empty-XDG-treated-as-unset guard)."

# MUST READ — the upstream S1 contract (rules.rs as S1 ships it)
- file: plan/002_637d65b6e9b8/P3M1T1S1/PRP.md
  why: "defines the four structs (RuleSet/HostDefaults/LayerRule/CallbackRule), the imports
        (`use crate::core::pattern::Pattern; use serde::Deserialize;`), the `pub mod rules;`
        mod.rs registration, and S1's ~9 test_rules_* tests. THIS task adds to that file
        WITHOUT touching the structs or S1's tests. Field types consumed here:
        rule_set.host.disable_firmware_config: bool; rule.disable_firmware_config: Option<bool>."
  section: "## What (Data models block) + Implementation Tasks Task 1/4"

# MUST READ — confirm tempfile is available for parse_rules file-IO tests
- file: Cargo.toml
  why: "tempfile = '3.0' appears BOTH as [dev-dependencies] (line 31) and regular dep (line 38).
        So #[cfg(test)] blocks can use tempfile::TempDir::new(). Precedent: src/platforms/hyprland.rs:597-625."
  gotcha: "serde 1.0+derive (line 12), toml 0.9 (line 21) are already deps — NO Cargo.toml edit."

# Reference — PathBuf::with_file_name (the tool get_rules_paths uses)
- url: https://doc.rust-lang.org/std/path/struct.PathBuf.html#method.with_file_name
  why: "documents with_file_name(file_name) -> PathBuf: replaces ONLY the final path component
        (the file name), leaving the parent directory. Exactly the 'swap config.toml → rules.toml
        in the same dir' operation. Returns a new PathBuf (consumes self via map)."
  critical: "if the path has no file name, this is a no-op — but every get_config_paths() entry
             ends in a literal 'config.toml' filename (verified in research/notes.md §4 table),
             so this never no-ops. Use this, NOT string-replace (G1)."

# Reference — Option::unwrap_or (the primitive's body)
- url: https://doc.rust-lang.org/std/option/enum.Option.html#method.unwrap_or
  why: "Option<bool>::unwrap_or(default) returns the inner bool if Some, else default. The
        effective_disable_firmware_config body is literally rule_override.unwrap_or(host_default)."
```

### Current Codebase tree (relevant subset)

```bash
src/
  main.rs              # `mod core;` (binary-only crate — NO lib.rs; see G8)
  core/
    mod.rs             # parse_config (lines 91-98) ← THE PATTERN parse_rules mirrors;
                         #   configured_timing() (line 73) already calls
                         #   crate::platforms::get_config_paths() — proves the dep direction.
                         #   UNCHANGED this task (S1 already added `pub mod rules;`).
    rules.rs            # ← CREATED by P3.M1.T1.S1 (structs). THIS TASK ADDS to it:
                         #     +4 imports, +3 functions, +~10 tests, +rustdoc on 2 pub fns.
    pattern.rs          # Pattern enum (P2.M1.T3.S2) — imported by rules.rs (S1). UNCHANGED.
    notifier.rs         # Notifier trait + debouncer. UNCHANGED (line 36 also calls
                         #   crate::platforms::get_config_paths()).
    types.rs            # WindowInfo. UNCHANGED.
  platforms/
    mod.rs              # get_config_paths() dispatcher (lines 63-77). UNCHANGED — DELEGATED TO.
    linux.rs            # get_config_paths() (line 116). UNCHANGED.
    macos.rs            # get_config_paths() (line 385). UNCHANGED.
    windows.rs          # get_config_paths() (line 434). UNCHANGED.
Cargo.toml             # serde 1.0+derive (12), toml 0.9 (21), tempfile 3.0 (31 dev + 38 main). UNCHANGED.
spec/HOST_RULES.md     # §9 effective resolution + §8 rules.toml location. READ-ONLY.
```

### Desired Codebase tree (files this task changes)

```bash
src/
  core/
    rules.rs           # MODIFIED (additive) — + `use std::{error::Error, fs, path::{Path, PathBuf}};`
                         #     + fn effective_disable_firmware_config (private, 1-line)
                         #     + pub fn parse_rules (twin of parse_config) + rustdoc
                         #     + pub fn get_rules_paths (delegates to get_config_paths) + rustdoc
                         #     + ~10 new tests in the existing #[cfg(test)] mod tests
    # mod.rs, pattern.rs, notifier.rs, types.rs, platforms/*, Cargo.toml: ALL UNCHANGED
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (G0 — runs in PARALLEL with P3.M1.T1.S1): rules.rs may not exist yet when
//   this task starts (S1 creates it). If S1 has landed → just ADD imports + 3 fns +
//   tests. If S1 has NOT landed → create rules.rs with the 3 fns + imports + tests AND
//   a clear `// P3.M1.T1.S1 owns the RuleSet/HostDefaults/LayerRule/CallbackRule structs`
//   marker; S1's structs merge in cleanly (disjoint regions: this task adds
//   imports/functions/tests, S1 adds struct definitions). END STATE: one file, both.
//
// CRITICAL (G1 — use `with_file_name`, NOT string-replace): get_rules_paths must do
//   `p.with_file_name("rules.toml")`. NEVER `p.to_string_lossy().replace("config.toml",
//   "rules.toml")`. `with_file_name` touches ONLY the final component and returns a
//   PathBuf directly (no encoding round-trip on Windows `\` paths). String replace
//   could mangle a directory named "config.toml" and is fragile.
//
// CRITICAL (G2 — delegate to get_config_paths, do NOT re-cfg per platform): the item
//   description explicitly says "lives in rules.rs but calls platforms::get_config_paths()".
//   Replicating the 3 `#[cfg(target_os=...)]` blocks would DUPLICATE the resolver and
//   drift from the real logic (e.g. linux.rs:119's empty-XDG_CONFIG_HOME-treated-as-unset
//   guard). One `.map(|p| p.with_file_name("rules.toml"))` call, zero cfg.
//
// CRITICAL (G3 — parse_rules takes ONE path, does NOT iterate): mirror parse_config
//   (single &Path). The find-existing-file iteration (`.find(|p| p.exists())`) is the
//   CALLER's job (P5 CLI / P4.3 notifier). Do NOT make parse_rules loop over
//   get_rules_paths().
//
// CRITICAL (G4 — error type `Box<dyn Error>`, NOT `+ Send + Sync`): match parse_config
//   EXACTLY. The Send+Sync variant is the qmk_notifier crate's `run()` signature; the
//   app's own config/rules file readers use the plain `Box<dyn Error>` form. Both
//   io::Error and toml::de::Error impl std::error::Error → convert via `?`.
//
// CRITICAL (G5 — effective_disable_firmware_config is module-private `fn`, NOT pub fn):
//   the item contract writes `fn` (no pub) for it, `pub fn` for the other two. Its only
//   consumer is P3.M1.T2.S1's evaluate() in the SAME module. The #[cfg(test)] mod tests
//   block (child of rules.rs) sees private items via `use super::*`. Do NOT over-expose.
//
// CRITICAL (G6 — do NOT aggregate the "replace iff all rules true" decision here): that
//   is evaluate()'s job (P3.M1.T2.S1). THIS task ships the per-rule primitive ONLY. The
//   item description's RESEARCH NOTE (point 1) describes the aggregation for CONTEXT.
//
// CRITICAL (G7 — do NOT validate callback names in parse_rules): §8 point 5's "validate
//   rules.toml names against name_to_id // warn, don't fail" needs the handshake
//   name→id map (P4.M2) and is explicitly a WARN. parse_rules does strict STRUCTURAL
//   deserialization only (the free validation from serde + S1's required-field strictness).
//
// GOTCHA (G8 — binary-only crate; doctests don't run under `--bin`): there is NO lib.rs
//   (src/main.rs declares `mod core;`). `cargo test --bin qmkonnect` runs UNIT TESTS
//   only. Mode-A rustdoc on parse_rules/get_rules_paths: use prose, or a ` ```rust,ignore `
//   fenced example. Do NOT add a bare ` ``` ` runnable doctest doing `use qmkonnect::...`
//   (won't compile under `cargo test --doc`; untested by `--bin`). pattern.rs has two
//   such untested doctests already — don't multiply that surface.
//
// GOTCHA (G9 — single-threaded tests crate-wide): `cargo test --bin qmkonnect --
//   --test-threads=1` (shared debouncer state in notifier.rs, AGENTS.md). NEVER
//   multi-threaded.
//
// GOTCHA (G10 — NO env mutation in get_rules_paths tests): assert the TRANSFORMATION
//   invariant (rules.toml in the same dir as config.toml, filename swapped) against the
//   REAL platform resolver output. Do NOT setenv/unsetenv XDG_CONFIG_HOME/APPDATA/home
//   (process-global, racy, platform-specific — the invariant test is strictly better).
//
// GOTCHA (G11 — additive only; do NOT touch S1's structs or S1's tests): this task adds
//   imports + 3 functions + NEW tests. If S1's `test_rules_*` tests exist, leave them;
//   append `test_rules_parse_*` / `test_rules_effective_*` / `test_rules_paths_*` to the
//   SAME `#[cfg(test)] mod tests` block.
//
// GOTCHA (G12 — RuleSet is in scope already): parse_rules deserializes to RuleSet, which
//   is defined in the SAME file (rules.rs). No import needed. Tests use `use super::*`.
//
// CRATE QUIRK: serde (1.0+derive, Cargo.toml line 12), toml (0.9, line 21), and
//   tempfile (3.0, lines 31 dev + 38 main) are ALL already deps. NO Cargo.toml edit.
//   `crate::platforms::get_config_paths()` is pub and returns Vec<PathBuf> — zero-config.
```

## Implementation Blueprint

### Data models and structure

No new data models (S1 owns the structs). This task adds **functions** that
consume S1's `RuleSet` + the existing `crate::platforms::get_config_paths()`.

```rust
// ── imports to ADD at the top of src/core/rules.rs (join S1's Pattern/Deserialize) ──
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

// ── function 1: the per-rule primitive (module-private, per contract G5) ──
/// Resolve a single rule's effective `disable_firmware_config`.
///
/// A rule's effective flag is its per-rule override when `Some`, otherwise the
/// `[host]` global default. This is the per-rule input to the stack-vs-replace
/// decision computed by `evaluate()` (P3.M1.T2): the window is "replace" iff
/// EVERY matched rule's effective flag is `true` (HOST_RULES.md §9).
fn effective_disable_firmware_config(rule_override: Option<bool>, host_default: bool) -> bool {
    rule_override.unwrap_or(host_default)
}

// ── function 2: the file reader (twin of core::mod::parse_config) ──
/// Read and deserialize a `rules.toml` file into a [`RuleSet`].
///
/// This is the host-side-rules counterpart to [`crate::parse_config`]: it reads
/// the file at `path` (via [`fs::read_to_string`]) and deserializes it with
/// [`toml::from_str`]. A missing/unreadable file yields an [`io::Error`](std::io::Error);
/// malformed TOML, or a `[[layer_rules]]`/`[[callback_rules]]` table missing the
/// required `match` or `layer` key, yields a [`toml::de::Error`]. Both propagate
/// as `Box<dyn Error>` — which is exactly the strict failure `--validate-rules`
/// (P5.M1) reports.
///
/// `path` is a SINGLE candidate (typically the first existing entry of
/// [`get_rules_paths`]); resolving the candidate list is the caller's job, mirroring
/// how `parse_config` is fed by `configured_timing()`'s `.find(|p| p.exists())`.
///
/// # Example
///
/// ```rust,ignore
/// use qmkonnect::core::rules::{get_rules_paths, parse_rules};
///
/// if let Some(path) = get_rules_paths().into_iter().find(|p| p.exists()) {
///     let rules = parse_rules(&path)?;   // Err on malformed rules.toml
///     // ... evaluate(rules, window) ...
/// }
/// ```
pub fn parse_rules(path: &Path) -> Result<RuleSet, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    let rules: RuleSet = toml::from_str(&text)?;
    Ok(rules)
}

// ── function 3: the path resolver (delegates to platforms::get_config_paths) ──
/// Return the candidate `rules.toml` paths, in platform preference order.
///
/// `rules.toml` lives **alongside `config.toml`** (HOST_RULES.md §8): same
/// directory, swapped filename. This function derives the list by delegating to
/// [`crate::platforms::get_config_paths`] and swapping each entry's final
/// filename component to `rules.toml` (via [`PathBuf::with_file_name`]).
///
/// On Linux this is `$XDG_CONFIG_HOME/qmk-notifier/rules.toml`,
/// `~/.config/qmk-notifier/rules.toml`, `/etc/qmk-notifier/rules.toml`; on
/// macOS `~/Library/Application Support/QMKonnect/rules.toml` (+ fallbacks); on
/// Windows `%APPDATA%\QMKonnect\rules.toml` (+ fallbacks). An absent file at
/// every candidate ⇒ the caller disables host rules (string-only, legacy path).
pub fn get_rules_paths() -> Vec<PathBuf> {
    crate::platforms::get_config_paths()
        .into_iter()
        .map(|p| p.with_file_name("rules.toml"))
        .collect()
}
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: RECONCILE rules.rs state (parallel with S1 — see G0)
  - DO: `cat src/core/rules.rs` (or read it). Determine S1's landing state:
    (a) S1 LANDED (structs + `use crate::core::pattern::Pattern;` + `use serde::Deserialize;`
        + a #[cfg(test)] mod tests block present): proceed to Task 2 (additive).
    (b) S1 NOT YET LANDED (file absent or lacks structs): create/seed rules.rs with the
        4 std imports (Task 2) + the 3 functions (Task 3) + tests (Task 4), and add a
        comment marker `// NOTE: the RuleSet/HostDefaults/LayerRule/CallbackRule structs
        // are defined by P3.M1.T1.S1 (spec/HOST_RULES.md §9).` where the structs will
        go. Do NOT define the structs (that's S1 — defining them here would conflict
        when S1 lands). parse_rules references `RuleSet` (used in the fn signature +
        body) — this compiles only once S1's struct exists; if S1 hasn't landed, the
        build will fail on the missing type, which is EXPECTED until S1 merges. The
        parallel orchestrator lands both before the build gate.
  - PRESERVE: whatever S1 has already written (structs, imports, tests). Additive only.
  - GOTCHA G0: the end state is ONE file with both S1's structs and S2's functions.

Task 2: ADD the 4 std imports to the top of src/core/rules.rs
  - DO: add (grouped, matching src/core/mod.rs's `use std::{...}` style):
        `use std::error::Error;`
        `use std::fs;`
        `use std::path::{Path, PathBuf};`
    Place them ABOVE S1's `use crate::core::pattern::Pattern;` / `use serde::Deserialize;`
    (std imports first, then crate/external — matches mod.rs ordering).
  - WHY: parse_rules needs Error (return type) + fs (read_to_string) + Path (param);
        get_rules_paths needs PathBuf (return). effective_... needs none (pure).
  - GOTCHA G12: do NOT import RuleSet — it's defined in the same file.

Task 3: ADD the 3 functions to src/core/rules.rs (place AFTER the structs, BEFORE #[cfg(test)])
  - DO: add the three functions EXACTLY as in the Data-models block above:
        - `fn effective_disable_firmware_config(...)` — PRIVATE (no `pub`), 1-line body.
        - `pub fn parse_rules(path: &Path) -> Result<RuleSet, Box<dyn Error>>` — twin of parse_config.
        - `pub fn get_rules_paths() -> Vec<PathBuf>` — delegates to get_config_paths + with_file_name.
  - FOLLOW: the Data-models block in this PRP (verbatim body + rustdoc).
  - NAMING: snake_case; effective_disable_firmware_config / parse_rules / get_rules_paths (exact,
            per the item contract).
  - VISIBILITY: `fn` (private) for effective_; `pub fn` for the other two (G5).
  - GOTCHA G1: get_rules_paths uses `p.with_file_name("rules.toml")`, NOT string replace.
  - GOTCHA G2: get_rules_paths contains NO `#[cfg(target_os=...)]` — it delegates.
  - GOTCHA G3: parse_rules takes ONE `&Path`, does NOT call get_rules_paths/iterate.
  - GOTCHA G4: error type `Box<dyn Error>`, not `+ Send + Sync`.
  - GOTCHA G6: do NOT add evaluate() or the "all matched rules" aggregation.
  - GOTCHA G7: do NOT add callback-name validation.
  - GOTCHA G8: rustdoc uses ` ```rust,ignore ` for the example (binary-only crate).
  - PLACEMENT: src/core/rules.rs, between the struct definitions and the `#[cfg(test)] mod tests`.

Task 4: ADD ~10 tests to the #[cfg(test)] mod tests block in src/core/rules.rs
  - DO: append the following tests to S1's existing `#[cfg(test)] mod tests { use super::*; ... }`
        (if S1's block exists; otherwise create the block). Prefix `test_rules_` with
        `effective_`/`parse_`/`paths_` sub-prefixes (disjoint from S1's `test_rules_*`
        struct tests, pattern.rs's `test_mp_`/`test_parity_`/`test_pattern_serde_`, mod.rs's `test_*`).

    A. effective_disable_firmware_config (pure, 4 tests — the truth table):
       1. test_rules_effective_some_true_wins   — effective_disable_firmware_config(Some(true),  false) == true
       2. test_rules_effective_some_false_wins  — effective_disable_firmware_config(Some(false), true)  == false
       3. test_rules_effective_none_inherits_false — effective_disable_firmware_config(None, false) == false
       4. test_rules_effective_none_inherits_true  — effective_disable_firmware_config(None, true)  == true

    B. parse_rules (file-IO via tempfile::TempDir, 4 tests):
       5. test_rules_parse_valid_section9 — write the §9 TOML to a TempDir file,
          parse_rules(&path).unwrap(); assert host.disable_firmware_config==false,
          layer_rules.len()==2, layer_rules[0].disable_firmware_config==Some(true),
          layer_rules[0].layer==224, callback_rules.len()==2. (Reuse S1's §9 const if
          present; else declare a local `const SECTION_9: &str = r#"..."#;`.)
       6. test_rules_parse_missing_file_errors — parse_rules(&Path::new("/nonexistent/qmk-rules-xyz.toml")).is_err()
          (io::Error propagates).
       7. test_rules_parse_malformed_toml_errors — write `not = valid = toml = =` to a TempDir
          file, parse_rules(&path).is_err() (toml::de::Error).
       8. test_rules_parse_missing_required_field_errors — write a `[[layer_rules]]` with
          `match="x"` but no `layer` to a TempDir file, parse_rules(&path).is_err()
          (S1's required-field strictness surfaces through the file path — the
          --validate-rules contract).

    C. get_rules_paths (transformation invariant, ENV-INDEPENDENT, 2 tests):
       9. test_rules_paths_swap_filename —
            let cfg = crate::platforms::get_config_paths();
            let rul = super::get_rules_paths();
            assert_eq!(cfg.len(), rul.len());
            for (c, r) in cfg.iter().zip(rul.iter()) {
                assert_eq!(c.parent(), r.parent(), "rules.toml must be in the SAME dir as config.toml");
                assert_eq!(r.file_name(), Some(std::ffi::OsStr::new("rules.toml")));
                assert_eq!(c.file_name(), Some(std::ffi::OsStr::new("config.toml")));
            }
          (Robust: passes on every platform incl. the empty-Vec unknown-platform case.)
      10. test_rules_paths_delegate_count — on linux/macos/windows, `get_rules_paths().len() >= 1`
          (sanity that delegation returned real paths); guarded by #[cfg(any(target_os="linux",
          target_os="macos", target_os="windows"))]. (Optional — test 9's len-equality
          already implies this; include for an explicit positive assertion on supported platforms.)

  - NAMING: test_rules_{effective|parse|paths}_* (disjoint prefixes).
  - HELPER: `tempfile::TempDir::new().unwrap()` (Cargo.toml dev-dep line 31; precedent
            hyprland.rs:602). Write via `std::fs::write(dir.path().join("rules.toml"), TOML).unwrap()`.
  - PATTERN: mirror mod.rs's `toml::from_str::<Config>(...).unwrap()` idiom for parse_rules's
             positive case; `.is_err()` for the negative cases.
  - GOTCHA G9: single-threaded (`cargo test --bin qmkonnect -- --test-threads=1`).
  - GOTCHA G10: NO env mutation — test 9 uses the REAL platform resolver output.
  - GOTCHA G11: do NOT modify S1's existing test_rules_* tests; append new ones.
  - PLACEMENT: src/core/rules.rs, inside the existing #[cfg(test)] mod tests block.

Task 5: VERIFY build + tests (single-threaded, per AGENTS.md)
  - RUN: cargo build --bin qmkonnect          (expect clean; no NEW warnings)
         — NOTE: if running BEFORE S1 lands, this fails on the missing RuleSet type
           (expected; the orchestrator lands both before the gate). After S1 merges,
           it must compile clean.
  - RUN: cargo test --bin qmkonnect rules::tests::test_rules_effective -- --test-threads=1
         (expect: 4 primitive tests pass)
  - RUN: cargo test --bin qmkonnect rules::tests::test_rules_parse -- --test-threads=1
         (expect: 4 parse_rules tests pass)
  - RUN: cargo test --bin qmkonnect rules::tests::test_rules_paths -- --test-threads=1
         (expect: 2 get_rules_paths tests pass)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1
         (expect: full crate green — S1's rules tests + the ~10 new + pattern + notifier
          + types + mod; no regression)
  - CONFIRM git status shows rules.rs as the only changed file (this task's diff).
  - IF a parse_rules test fails on a VALID §9 TOML: the struct shapes are S1's contract
        (spec/HOST_RULES.md §9); a deserialization failure = an S1 transcription slip,
        NOT a parse_rules bug. Report it but do NOT change the structs here (G11).
```

### Implementation Patterns & Key Details

```rust
// The canonical 3-function layer (this IS the contract — match it; full verbatim in
// research/notes.md §2/§3/§4).
//
// // imports (Task 2):
// use std::error::Error;
// use std::fs;
// use std::path::{Path, PathBuf};
//
// // function 1 — private primitive (G5):
// fn effective_disable_firmware_config(rule_override: Option<bool>, host_default: bool) -> bool {
//     rule_override.unwrap_or(host_default)
// }
//
// // function 2 — twin of core::parse_config (G3 single-path, G4 error type):
// pub fn parse_rules(path: &Path) -> Result<RuleSet, Box<dyn Error>> {
//     let text = fs::read_to_string(path)?;
//     let rules: RuleSet = toml::from_str(&text)?;
//     Ok(rules)
// }
//
// // function 3 — delegate + swap filename (G1 with_file_name, G2 no cfg):
// pub fn get_rules_paths() -> Vec<PathBuf> {
//     crate::platforms::get_config_paths()
//         .into_iter()
//         .map(|p| p.with_file_name("rules.toml"))
//         .collect()
// }
//
// // Test idiom (Task 4):
// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn test_rules_effective_some_true_wins() {
//         assert_eq!(effective_disable_firmware_config(Some(true), false), true);
//     }
//
//     #[test]
//     fn test_rules_parse_valid_section9() {
//         let dir = tempfile::TempDir::new().unwrap();
//         let path = dir.path().join("rules.toml");
//         std::fs::write(&path, SECTION_9_TOML).unwrap();   // the §9 example
//         let rs = parse_rules(&path).unwrap();
//         assert_eq!(rs.host.disable_firmware_config, false);
//         assert_eq!(rs.layer_rules.len(), 2);
//         assert_eq!(rs.layer_rules[0].disable_firmware_config, Some(true));
//     }
//
//     #[test]
//     fn test_rules_parse_missing_file_errors() {
//         let p = Path::new("/nonexistent/qmk-rules-xyz-9f8e7.toml");
//         assert!(parse_rules(p).is_err());
//     }
//
//     #[test]
//     fn test_rules_paths_swap_filename() {
//         let cfg = crate::platforms::get_config_paths();
//         let rul = get_rules_paths();
//         assert_eq!(cfg.len(), rul.len());
//         for (c, r) in cfg.iter().zip(rul.iter()) {
//             assert_eq!(c.parent(), r.parent());
//             assert_eq!(r.file_name(), Some(std::ffi::OsStr::new("rules.toml")));
//         }
//     }
// }
//
// NOTE: `RuleSet`, `effective_disable_firmware_config`, `parse_rules`, `get_rules_paths`
// are all in the same module — `use super::*;` brings them into the test scope. The
// PRIVATE `effective_disable_firmware_config` is reachable from the child test module.
```

### Integration Points

```yaml
MODULE REGISTRATION:
  - NONE this task. `pub mod rules;` was added to src/core/mod.rs by P3.M1.T1.S1.
    This task only ADDS to the body of rules.rs.

DEPENDENCIES (this task): NONE new. serde (1.0 + derive), toml (0.9), and tempfile
                           (3.0, dev + main) are ALL already Cargo deps. NO Cargo.toml edit.

UPSTREAM (consumed unchanged):
  - RuleSet / HostDefaults / LayerRule / CallbackRule — P3.M1.T1.S1, src/core/rules.rs.
    Fields read: rule_set.host.disable_firmware_config: bool;
    rule.disable_firmware_config: Option<bool>. (parse_rules deserializes INTO RuleSet.)
  - crate::platforms::get_config_paths() -> Vec<PathBuf> — src/platforms/mod.rs:63.
    Already pub; already called from core (mod.rs:73, notifier.rs:36). get_rules_paths
    delegates to it. NO change to platforms/.

DOWNSTREAM CONSUMERS (later subtasks — do NOT implement them here):
  - P3.M1.T2.S1 (evaluate): calls effective_disable_firmware_config(rule.disable_firmware_config,
    rule_set.host.disable_firmware_config) per matched rule; aggregates "replace iff ALL true".
  - P4.M3.T1.S1 (notify_qmk host-context send): get_rules_paths().into_iter().find(|p| p.exists())
    .map(parse_rules) at startup / on "Reload rules".
  - P5.M1.T1.S1 (--validate-rules): same find+parse; reports the Err.
  - P5.M2.T1 (tray "Reload rules"): calls into the reload path (get_rules_paths + parse_rules).

CONFIG: none (no new config knob).
ROUTES: none (no CLI surface this subtask — --validate-rules is P5.M1.T1.S1).
DATABASE: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles clean, no NEW warnings. (If running before P3.M1.T1.S1 lands,
# this fails on the missing RuleSet type — EXPECTED; the orchestrator lands both
# before the gate. After S1 merges, it must compile clean.) If rustc errors AFTER
# both are landed (e.g. a typo'd import, a pub where private was required, a
# string-replace instead of with_file_name), READ it and fix.

# Confirm the deliverables are present:
grep -nE 'use std::\{(error::Error|fs|path)' src/core/rules.rs          # expect the 4 std imports
grep -n 'fn effective_disable_firmware_config' src/core/rules.rs        # expect 1 (private fn, G5)
grep -n 'pub fn parse_rules' src/core/rules.rs                          # expect 1
grep -n 'pub fn get_rules_paths' src/core/rules.rs                     # expect 1
grep -n 'with_file_name("rules.toml")' src/core/rules.rs               # expect 1 (G1 — NOT string replace)
grep -n 'crate::platforms::get_config_paths' src/core/rules.rs         # expect 1 (the delegation, G2)
# Confirm NO per-platform cfg duplication (G2):
! grep -nE '#\[cfg\(target_os' src/core/rules.rs || echo "FAIL: cfg duplication (G2 violation)"
# Confirm NO evaluate()/aggregation (G6):
! grep -nE 'fn evaluate|all_matched|replace_iff' src/core/rules.rs || echo "FAIL: aggregation logic (G6 violation)"
```

### Level 2: Unit Tests (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# Single-threaded is MANDATORY crate-wide (shared debouncer state, AGENTS.md).
cargo test --bin qmkonnect rules::tests::test_rules_effective -- --test-threads=1
# Expected: 4 primitive truth-table tests pass (Some wins ×2, None inherits ×2).

cargo test --bin qmkonnect rules::tests::test_rules_parse -- --test-threads=1
# Expected: 4 parse_rules tests pass (valid §9 round-trip, missing-file err,
# malformed-toml err, missing-required-field err).

cargo test --bin qmkonnect rules::tests::test_rules_paths -- --test-threads=1
# Expected: 2 get_rules_paths tests pass (filename-swap invariant, delegate-count sanity).

# All new tests at once:
cargo test --bin qmkonnect rules -- --test-threads=1
# Expected: all ~10 new tests + S1's ~9 test_rules_* struct tests pass.
```

### Level 3: Full-crate regression (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL bin tests green — S1's rules::tests + the ~10 new rules::tests +
# pattern (incl. the P2.M1.T4.S1 parity corpus) + notifier + types + mod. Proves the
# additions compile in the full crate context and didn't break module resolution or
# the shared debouncer state.

# Confirm the change surface is rules.rs only (this task's diff):
git status --short
# Expected: only src/core/rules.rs modified (additive). NOTHING in Cargo.toml,
# mod.rs, pattern.rs, notifier.rs, types.rs, platforms/*.
git diff --stat
# Expected: 1 file: src/core/rules.rs.
```

### Level 4: End-to-end file-IO sanity (optional, high-confidence)

```bash
# Manually exercise parse_rules + get_rules_paths against a hand-written rules.toml,
# to eyeball that the file-read + serde path works on the REAL platform location
# (not just a TempDir). Confirms the §9 shapes round-trip through an actual file.
cd /home/dustin/projects/qmkonnect
cat > /tmp/qmkonnect_rules_l4.toml <<'EOF'
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
# (parse_rules/get_rules_paths have no standalone CLI yet — --validate-rules is P5.M1.T1.S1.
#  This Level-4 check is covered FUNCTIONALLY by test_rules_parse_valid_section9 in Level 2,
#  which writes the §9 TOML to a TempDir file and runs parse_rules end-to-end. The
#  get_rules_paths real-location behavior is covered by test_rules_paths_swap_filename,
#  which asserts against the live crate::platforms::get_config_paths() output. No extra
#  manual step is required for this task; the Level-2 tests ARE the end-to-end gate.)
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean (no NEW warnings) — once P3.M1.T1.S1 has landed.
- [ ] `cargo test --bin qmkonnect rules::tests::test_rules_effective -- --test-threads=1` — 4 pass.
- [ ] `cargo test --bin qmkonnect rules::tests::test_rules_parse -- --test-threads=1` — 4 pass.
- [ ] `cargo test --bin qmkonnect rules::tests::test_rules_paths -- --test-threads=1` — 2 pass.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` — full crate green (no regression).
- [ ] `git status` shows rules.rs as the only changed file (this task's diff).

### Feature Validation (contract fidelity)
- [ ] **4 std imports** present: `std::error::Error`, `std::fs`, `std::path::Path`, `std::path::PathBuf`.
- [ ] **`effective_disable_firmware_config`** is module-private `fn` (NOT `pub fn`); body `rule_override.unwrap_or(host_default)`.
- [ ] **`parse_rules(path: &Path) -> Result<RuleSet, Box<dyn Error>>`** reads via `fs::read_to_string` + deserializes via `toml::from_str`; takes ONE path (no iteration).
- [ ] **`get_rules_paths() -> Vec<PathBuf>`** delegates to `crate::platforms::get_config_paths()` + `with_file_name("rules.toml")`; contains NO `#[cfg(target_os=...)]`.
- [ ] **Truth table** correct: `(Some(true),false)→true`, `(Some(false),true)→false`, `(None,false)→false`, `(None,true)→true` (tests 1–4).
- [ ] **Valid §9 TOML** parses through a real file to the expected nested `RuleSet` (test 5).
- [ ] **Missing file** → `Err` (test 6, io::Error).
- [ ] **Malformed TOML** → `Err` (test 7, toml::de::Error).
- [ ] **Missing required `layer`** → `Err` (test 8, S1 strictness surfaces through file path).
- [ ] **Path swap invariant**: every `get_rules_paths()` entry is the `get_config_paths()` entry with filename `rules.toml` in the SAME directory (test 9, env-independent).

### Code Quality Validation
- [ ] `parse_rules` is a faithful twin of `parse_config` (same error type, same two-step body).
- [ ] `get_rules_paths` uses `with_file_name` (G1), not string replace.
- [ ] `get_rules_paths` delegates (G2), no cfg duplication.
- [ ] `effective_disable_firmware_config` is private (G5); no over-exposure.
- [ ] Mode-A rustdoc on `parse_rules` + `get_rules_paths` (G8 — ` ```rust,ignore ` or prose, not bare runnable doctest).
- [ ] No `evaluate()` / aggregation / `HostContext` (G6).
- [ ] No callback-name validation (G7).
- [ ] No env mutation in tests (G10).
- [ ] S1's structs + S1's tests untouched (G11 — additive only).
- [ ] No new Cargo dependencies (serde/toml/tempfile already present).
- [ ] No `unsafe`; no `static`; no module-scope `mut`.

### Documentation & Deployment
- [ ] Mode-A rustdoc present on `parse_rules` and `get_rules_paths`; cites `HOST_RULES.md` §9/§8.
- [ ] No `docs/*.md` or README changes this task (Mode A — code-level docs only).

---

## Anti-Patterns to Avoid

- ❌ Do NOT implement `evaluate()`, the "replace iff ALL matched rules effective==true"
      aggregation, or `HostContext`. Those are P3.M1.T2.S1. This task ships the
      per-rule `effective_disable_firmware_config` PRIMITIVE only (G6).
- ❌ Do NOT make `parse_rules` iterate over `get_rules_paths()`. It takes ONE `&Path`,
      exactly like `parse_config` (G3). The find-existing-file loop is the caller's job.
- ❌ Do NOT replicate the per-platform `#[cfg(target_os=...)]` blocks in
      `get_rules_paths`. Delegate to `crate::platforms::get_config_paths()` and map
      `with_file_name` (G2) — that's the whole point of "lives in rules.rs but calls
      platforms::get_config_paths()".
- ❌ Do NOT use string replacement (`to_string_lossy().replace("config.toml", ...)`)
      to swap the filename. Use `PathBuf::with_file_name("rules.toml")` — it touches
      only the final component and is encoding-safe on Windows (G1).
- ❌ Do NOT change the error type to `Box<dyn Error + Send + Sync>`. Match `parse_config`
      exactly: `Box<dyn Error>` (G4).
- ❌ Do NOT make `effective_disable_firmware_config` `pub`. The contract writes `fn`
      (no pub); its only consumer (evaluate) is in the same module; the test block sees
      private items (G5).
- ❌ Do NOT add callback-name validation to `parse_rules`. That needs the handshake
      name→id map (P4.M2) and is explicitly a WARN, not a hard error (G7). parse_rules
      does strict structural deserialization only.
- ❌ Do NOT modify S1's structs (RuleSet/HostDefaults/LayerRule/CallbackRule) or S1's
      existing `test_rules_*` tests. This task is purely additive (G11). If a parse_rules
      test fails on valid §9 TOML, the bug is in S1's struct shapes — report it, don't
      paper over it here.
- ❌ Do NOT write runnable Rust doctests (` ``` `) that `use qmkonnect::...`. This is a
      binary-only crate (no lib.rs); they won't compile under `cargo test --doc` and
      `--bin` doesn't run doctests anyway. Use ` ```rust,ignore ` or prose (G8).
- ❌ Do NOT mutate env vars (XDG_CONFIG_HOME/APPDATA/home) in get_rules_paths tests.
      Assert the transformation invariant against the real resolver output (G10).
- ❌ Do NOT run tests multi-threaded — `cargo test --bin qmkonnect -- --test-threads=1`
      (shared debouncer state, G9/AGENTS.md).
- ❌ Do NOT edit Cargo.toml (serde/toml/tempfile all present), platforms/* (delegated to,
      unchanged), mod.rs (S1 already registered the module), or any other source file.
- ❌ Do NOT edit `PRD.md`, any `tasks.json`, `prd_snapshot.md`, `spec/HOST_RULES.md`,
      `Cargo.toml`, or any `plan/` file other than this item's own `PRP.md` + `research/`.

---

## Confidence Score: 9/10

This is a tightly-bounded **three-function additive layer** over an existing,
well-understood pattern. (1) `effective_disable_firmware_config` is a one-liner
(`Option::unwrap_or`) with a 4-row truth table that's directly unit-testable. (2)
`parse_rules` is a verbatim twin of `parse_config` (`src/core/mod.rs:91-98`,
reproduced in `research/notes.md` §3) — same error type, same two-step body, only
the target type changes (`Config → RuleSet`). (3) `get_rules_paths` is a one-line
`.map(|p| p.with_file_name("rules.toml")).collect()` over the existing
`crate::platforms::get_config_paths()` — confirmed pub, confirmed already called
from core (`mod.rs:73`, `notifier.rs:36`), confirmed every returned path ends in a
literal `config.toml` filename (so `with_file_name` never no-ops). All deps
(serde 1.0+derive, toml 0.9, tempfile 3.0) are already in Cargo.toml. The 12
gotchas are each pinned to a concrete failure mode caught by the ~10 tests
(notably G1 with_file_name-vs-replace, G2 delegate-don't-recfg, G4 error-type,
G5 private-fn, G6 no-aggregation). The 1-point reservation is for: (a) the
**parallel-execution file-state handling** (G0 — rules.rs may not exist when this
task starts; the implementer must handle the "S1 not yet landed" case without
defining the structs, which would conflict on merge — mitigated by Task 1's
explicit reconciliation, but it's the one non-deterministic input); (b) the
`test_rules_parse_valid_section9` test depends on S1's struct shapes being correct
(a parse failure there = an S1 transcription slip, not a parse_rules bug — flagged
in Task 5); and (c) the `get_rules_paths` empty-Vec edge case on unknown platforms
(test 9 handles it via `len==len` + empty zip, but an implementer who adds a
test-10 `assert!(!get_rules_paths().is_empty())` without the `#[cfg]` guard would
falsely fail on a future non-Linux/macOS/Windows CI target — mitigated by the
explicit `#[cfg(any(target_os=...))]` guard in the test spec). All three are
low-risk and immediately caught by `cargo test`. Scope is cleanly bounded from
upstream S1 (structs untouched), the evaluator P3.M1.T2 (no aggregation), and the
CLI/tray P5 (no wiring), so there is no risk of over- or under-building.
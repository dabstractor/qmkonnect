# Research Notes — P3.M1.T1.S2: effective_disable_firmware_config + parse_rules() + get_rules_paths()

> **Scope of THIS subtask:** ADD three functions to `src/core/rules.rs` (the file
> created by P3.M1.T1.S1 — assumed present per the parallel-execution contract):
>   1. `fn effective_disable_firmware_config(rule_override: Option<bool>, host_default: bool) -> bool`
>   2. `pub fn parse_rules(path: &Path) -> Result<RuleSet, Box<dyn Error>>`
>   3. `pub fn get_rules_paths() -> Vec<PathBuf>`
> plus Mode-A rustdoc on `parse_rules` and `get_rules_paths`. NO changes to the
> structs themselves (P3.M1.T1.S1 owns those), NO `evaluate()` (P3.M1.T2.S1), NO
> CLI/tray wiring (P5). It is the **file-IO + path-resolution + per-rule
> resolution primitive** layer between the data model (S1) and the evaluator (T2).

---

## 1. The exact upstream contract (P3.M1.T1.S1 — assumed delivered verbatim)

`src/core/rules.rs` exists (created by S1) and contains, at minimum:

```rust
use crate::core::pattern::Pattern;   // S1 PRP §3 — full crate path (or super::pattern::Pattern)
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct RuleSet {
    #[serde(default)] pub host: HostDefaults,
    #[serde(default, rename = "layer_rules")] pub layer_rules: Vec<LayerRule>,
    #[serde(default, rename = "callback_rules")] pub callback_rules: Vec<CallbackRule>,
}
// + HostDefaults (manual Default → false), LayerRule, CallbackRule
// + a #[cfg(test)] mod tests block (the ~9 test_rules_* from S1)
```

THIS task ADDS std imports + 3 functions to that file. It does NOT touch the
struct definitions, the `Pattern` import, or S1's tests. The `#[cfg(test)] mod
tests` block gets NEW tests appended (test naming prefix `test_rules_` continues,
disjoint from S1's; e.g. `test_rules_parse_*`, `test_rules_effective_*`,
`test_rules_paths_*`).

**CRITICAL:** because this runs in PARALLEL with S1, the implementer may find
S1's work in one of three states. The PRP's Task 1 handles all three (see PRP
§Implementation Tasks G0 — the file may not exist yet; create it, but ONLY with
the function layer, leaving a clear marker that S1 owns the structs). The
intended end state is: one file with BOTH S1's structs AND S2's functions.

---

## 2. The `effective_disable_firmware_config` primitive — spec & semantics

**Source of truth:** `spec/HOST_RULES.md` §9 (lines 455–456):
> "A rule's effective `disable_firmware_config` = its override if `Some`, else
> the `[host]` default. The window is **replace** iff every matched rule's
> effective flag is `true`."

**Spec §4 (architecture, line 140) restates:**
> "window is 'replace' iff EVERY matched rule has disable_firmware_config=true"

**The contract (item description):**
> "Implement `fn effective_disable_firmware_config(rule_override: Option<bool>, host_default: bool) -> bool` (override.unwrap_or(host_default))."

Implementation (one line):
```rust
/// Resolve a single rule's effective `disable_firmware_config`.
fn effective_disable_firmware_config(rule_override: Option<bool>, host_default: bool) -> bool {
    rule_override.unwrap_or(host_default)
}
```

**Truth table (the 4 cases to unit-test):**
| `rule_override` | `host_default` | result | meaning |
|-----------------|----------------|--------|--------|
| `Some(true)`    | `false`        | `true`  | per-rule override WINS (board cleared for this window) |
| `Some(false)`   | `true`         | `false` | per-rule override WINS (board keeps running) |
| `None`          | `false`        | `false` | no override → inherit [host] default (false = stack) |
| `None`          | `true`         | `true`  | no override → inherit [host] default (true = replace) |

**Visibility decision — `fn`, NOT `pub fn`:** the contract literally writes
`fn effective_disable_firmware_config(...)` (no `pub`), whereas it writes `pub fn
parse_rules` and `pub fn get_rules_paths`. The ONLY consumer is P3.M1.T2.S1's
`evaluate()`, which lives in the SAME `src/core/rules.rs` module — so a
module-private `fn` is reachable. It is also reachable from the `#[cfg(test)] mod
tests { use super::*; }` block (child modules see parent's private items). Making
it `pub` would be harmless but deviates from the contract; follow the contract
exactly: **private `fn`**. (If a future cross-module caller needs it, promote to
`pub` then — YAGNI now.)

**SCOPE boundary — do NOT aggregate here:** the "replace iff ALL matched rules
effective==true" aggregation belongs to `evaluate()` (P3.M1.T2.S1), NOT this
primitive. THIS task ships the per-rule resolver only. The item description
explicitly separates them ("LOGIC: Implement `fn effective_disable_firmware_config`"
vs. the downstream P3.M1.T2 evaluate). Do NOT write an `all_matched_rules_replace()`
helper here.

---

## 3. `parse_rules` — the EXACT pattern to mirror (`src/core/mod.rs::parse_config`)

**Verbatim from `src/core/mod.rs` lines 91–98** (confirmed by reading):
```rust
pub fn parse_config(config_path: &Path) -> Result<Config, Box<dyn Error>> {
    let config_str = fs::read_to_string(config_path)?;
    let config: Config = toml::from_str(&config_str)?;

    // No need to normalize or validate - TOML parser handles it

    Ok(config)
}
```

`parse_rules` is a drop-in twin, deserializing to `RuleSet` instead of `Config`:
```rust
pub fn parse_rules(path: &Path) -> Result<RuleSet, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    let rules: RuleSet = toml::from_str(&text)?;
    Ok(rules)
}
```

**Why this is correct (every detail matches the established convention):**
- **Error type `Box<dyn Error>`** — identical to `parse_config`. NOT `Box<dyn Error
  + Send + Sync>` (that's the crate `run()`'s signature; the app's own config
  reader uses the non-Send-Sync form). Matching it keeps `rules.toml` reading on
  the same error-type rail as `config.toml` reading.
- **`fs::read_to_string(path)?`** — `std::fs::read_to_string` returns
  `io::Result<String>`; `?` converts `io::Error → Box<dyn Error>` (io::Error impls
  `std::error::Error`). A missing/unreadable file → error propagates. This is the
  desired "absent file fails parse_rules" behavior; the CALLER (P5 CLI / P4.3
  notifier) does `get_rules_paths().into_iter().find(|p| p.exists()).map(parse_rules)`
  so a missing file is skipped BEFORE parse_rules is called — but parse_rules
  itself faithfully errors on a genuinely-unreadable path (defensive; mirrors
  parse_config which also expects an existing path).
- **`toml::from_str(&text)?`** — `toml = "0.9"` (Cargo.toml line 21, already a
  dep). `toml::de::Error` impls `std::error::Error` → converts via `?`. Malformed
  TOML, OR a `[[layer_rules]]` missing required `match`/`layer` (the §9
  no-`#[serde(default)]` strictness shipped by S1) → error. This is exactly the
  schema-validation `--validate-rules` (P5.M1.T1.S1) will report.
- **No normalization/validation logic** — the `// No need to normalize or validate`
  comment in parse_config applies equally: serde + the §9 struct shapes do all
  validation. Do NOT add callback-name validation here (that needs the handshake
  name→id map, §8 point 5 — it's P4.M2's job, and it's "warn, don't fail").

**`parse_rules` does NOT resolve paths.** It takes ONE `&Path` and reads it. Path
resolution is `get_rules_paths()`'s job. Clean separation, mirrors the
parse_config / get_config_paths split exactly (parse_config also takes a single
path; configured_timing() does the find-and-parse orchestration).

---

## 4. `get_rules_paths` — mirror `get_config_paths()`, swap the filename

**The dispatcher** (`src/platforms/mod.rs` lines 63–77, confirmed by reading):
```rust
pub fn get_config_paths() -> Vec<std::path::PathBuf> {
    #[cfg(target_os = "linux")]   return linux::get_config_paths();
    #[cfg(target_os = "windows")] return windows::get_config_paths();
    #[cfg(target_os = "macos")]   return macos::get_config_paths();
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    return Vec::new();
}
```

**What each platform returns (every path ends in a literal `config.toml` filename):**

| platform | paths (in order) |
|----------|------------------|
| **linux** (`src/platforms/linux.rs:116`) | `$XDG_CONFIG_HOME/qmk-notifier/config.toml`; `~/.config/qmk-notifier/config.toml`; `/etc/qmk-notifier/config.toml` |
| **macos** (`src/platforms/macos.rs:385`) | `~/Library/Application Support/QMKonnect/config.toml`; `~/.config/qmk-notifier/config.toml`; `/etc/qmk-notifier/config.toml` |
| **windows** (`src/platforms/windows.rs:434`) | `%APPDATA%\QMKonnect\config.toml`; `%LOCALAPPDATA%\QMKonnect\config.toml`; `<exe_dir>\config.toml` |

**HOST_RULES.md §8 (line 305) — rules.toml lives ALONGSIDE config.toml:**
> "rules.toml — new module src/core/rules.rs, alongside config.toml (Linux
> `~/.config/qmk-notifier/rules.toml`, Windows `%APPDATA%\QMKonnect\`, macOS
> `~/Library/Application Support/QMKonnect/`)."

So: **same directory, swap `config.toml` → `rules.toml`.**

**The implementation (the contract):** lives in `rules.rs`, CALLS
`crate::platforms::get_config_paths()` to get the base directories, then swaps
the final filename component:
```rust
pub fn get_rules_paths() -> Vec<PathBuf> {
    crate::platforms::get_config_paths()
        .into_iter()
        .map(|p| p.with_file_name("rules.toml"))
        .collect()
}
```

**Why `with_file_name("rules.toml")` is the correct tool (not string replace):**
- `PathBuf::with_file_name(name)` replaces ONLY the final path component (the file
  name), leaving the parent directory untouched. Every config path's final
  component is literally `config.toml` (verified above), so
  `p.with_file_name("rules.toml")` yields the rules path in the same directory.
- It is MORE robust than `p.to_string_lossy().replace("config.toml", "rules.toml")`:
  string replace would mangle a hypothetical directory named `config.toml`
  (improbable, but `with_file_name` is unambiguously correct). And it produces a
  `PathBuf` directly (no string round-trip, no encoding pitfalls on Windows
  where the separator is `\`).
- It is idempotent and zero-config: it inherits the platform's exact directory
  resolution (XDG env handling, APPDATA/LOCALAPPDATA, exe-dir fallback, the
  "empty XDG_CONFIG_HOME treated as unset" guard in linux.rs:119) FOR FREE —
  because it delegates to `get_config_paths()`. rules.rs never duplicates path
  logic (DRY; the single source of truth stays in platforms/).

**Dependency direction is sound:** `core` → `platforms` is the ESTABLISHED
direction (`src/core/mod.rs:73` already calls `crate::platforms::get_config_paths()`
inside `configured_timing()`; `src/core/notifier.rs:36` does the same). So
rules.rs calling `crate::platforms::get_config_paths()` introduces NO new coupling
and NO circular dependency. The item description explicitly mandates this:
> "This function lives in rules.rs but calls platforms::get_config_paths() to get the base directories."

**Do NOT replicate per-platform `cfg` blocks** inside get_rules_paths. Delegating
to `get_config_paths()` is the whole point — one `map`, no `#[cfg(target_os=...)]`.

**Edge case — empty `get_config_paths()` (unknown platform):** returns an empty
`Vec`. The caller's `.find(|p| p.exists())` yields `None` → host rules disabled.
Correct (no rules.toml found ⇒ string-only behavior, §8 point 8).

---

## 5. Imports THIS task adds to `src/core/rules.rs`

S1's rules.rs begins with (do NOT touch these):
```rust
use crate::core::pattern::Pattern;
use serde::Deserialize;
```

THIS task adds, ABOVE the structs (top of file, grouped with std imports per repo
convention — see `src/core/mod.rs` which puts `use std::{error::Error, fs,
path::Path, sync::OnceLock, time::Instant};` at the top):
```rust
use std::error::Error;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
```
(Or the grouped form `use std::path::{Path, PathBuf};` — both compile; match the
mod.rs style. `fs` is imported as the module (used as `fs::read_to_string`), NOT
`use std::fs::read_to_string` — this mirrors `parse_config` which does exactly
`use std::fs;` then `fs::read_to_string(...)`.)

**No new Cargo dependencies.** `std`, `serde` (1.0 + derive), `toml` (0.9) are all
already deps. `tempfile` (needed only for parse_rules file-IO tests) is BOTH a
regular dep (Cargo.toml line 38, used by linux.rs:352 at runtime for atomic udev
writes) AND a `[dev-dependencies]` entry (line 31, used by hyprland.rs tests) —
so `#[cfg(test)]` blocks can freely use `tempfile::TempDir::new()`. Confirmed by
`src/platforms/hyprland.rs:597-625` which uses `tempfile::TempDir::new().unwrap()`
in tests.

---

## 6. Test plan — 3 function families, ~10 tests (append to S1's `mod tests`)

All tests live in the existing `#[cfg(test)] mod tests { use super::*; ... }` block
at the tail of `src/core/rules.rs`. Prefix `test_rules_` continues (disjoint from
S1's `test_rules_*` by using `parse`/`effective`/`paths` sub-prefixes; disjoint
from pattern.rs's `test_mp_`/`test_parity_`/`test_pattern_serde_` and mod.rs's
`test_*`).

### A. `effective_disable_firmware_config` (pure, 4 tests — the truth table)
1. `test_rules_effective_some_true_wins` — `(Some(true), false) == true`
2. `test_rules_effective_some_false_wins` — `(Some(false), true) == false`
3. `test_rules_effective_none_inherits_false` — `(None, false) == false`
4. `test_rules_effective_none_inherits_true` — `(None, true) == true`

### B. `parse_rules` (file-IO, needs temp files — 4 tests)
5. `test_rules_parse_valid_section9` — write the §9 example TOML to a
   `tempfile::TempDir` file, `parse_rules(&path).unwrap()`, assert
   `host.disable_firmware_config == false`, `layer_rules.len() == 2`,
   `layer_rules[0].disable_firmware_config == Some(true)`, etc. (Reuses the §9
   constant from S1's test 1 if present, else re-declare a local `const`.)
6. `test_rules_parse_missing_file_errors` — `parse_rules(&Path::new("/nonexistent/qmk-rules-xyz.toml"))`
   is `Err` (`fs::read_to_string` io::Error propagates).
7. `test_rules_parse_malformed_toml_errors` — write `not = valid = toml = =` to a
   temp file → `Err` (`toml::de::Error`).
8. `test_rules_parse_missing_required_field_errors` — write a `[[layer_rules]]`
   with `match` but no `layer` → `Err` (the S1 strictness, exercised through the
   file path). Proves the end-to-end "malformed rules.toml fails parse_rules"
   contract that `--validate-rules` relies on.

### C. `get_rules_paths` (path transformation — 2 tests, ENV-INDEPENDENT)
9. `test_rules_paths_swap_filename` — the core invariant:
   ```rust
   let cfg = crate::platforms::get_config_paths();
   let rul = super::get_rules_paths();
   assert_eq!(cfg.len(), rul.len());
   for (c, r) in cfg.iter().zip(rul.iter()) {
       assert_eq!(c.parent(), r.parent(), "rules.toml must be in the SAME dir as config.toml");
       assert_eq!(r.file_name(), Some(std::ffi::OsStr::new("rules.toml")));
   }
   ```
   This is ROBUST: it asserts the transformation property (same dir, swapped
   filename) WITHOUT depending on any env var (XDG_CONFIG_HOME, APPDATA, home).
   It passes on every platform (including the empty-Vec unknown-platform case:
   `zip` of two empty iterators yields nothing, the loop body never runs, both
   `assert_eq!` on `.len()` hold as `0 == 0`). It is the cleanest possible test.
10. `test_rules_paths_nonempty_on_supported_platforms` — on linux/macos/windows,
    `get_rules_paths().len() >= 1` (sanity that delegation actually returned
    paths). On other targets, `assert!(get_rules_paths().is_empty())`. Use a
    `#[cfg(...)]` split or just assert the len matches get_config_paths len
    (covered by test 9 already, so this is optional / can be folded into 9).

**Why no env-mutating test for get_rules_paths:** setting/unsetting
`XDG_CONFIG_HOME`/`APPDATA` in a test is racy (process-global env) and
platform-specific. The transformation-invariant test (9) is strictly better:
it verifies the contract ("rules.toml alongside config.toml") using whatever the
real platform resolver returned, so it's both more portable and more faithful.
The actual per-platform path VALUES are already tested by the existing
platforms/{linux,macos,windows}.rs behavior (and by the app running); THIS task
only needs to prove the `config.toml → rules.toml` swap is correct.

**Single-threaded:** `cargo test --bin qmkonnect -- --test-threads=1` (AGENTS.md;
shared debouncer state). Even though env-mutating tests are avoided, the
crate-wide rule still applies.

---

## 7. Gotchas (pinned to concrete failure modes)

- **G0 — parallel with S1 (file may not exist yet).** THIS task runs in parallel
  with P3.M1.T1.S1 (which creates rules.rs + the structs). The implementer may
  find: (a) S1 already landed (rules.rs + structs present) → just ADD the imports
  + 3 functions + new tests; (b) S1 not yet landed → create rules.rs WITH a
  placeholder note that the structs are S1's, add the 3 functions + imports, and
  leave S1's structs for S1 (the two will merge cleanly because they touch
  disjoint regions: imports/functions/tests vs struct definitions). The PRP's
  Task ordering handles both. The END STATE is one file with both.
- **G1 — `with_file_name`, NOT string replace.** Use
  `p.with_file_name("rules.toml")`. Never `.to_string_lossy().replace(...)`.
  (`with_file_name` touches only the final component; string replace could hit a
  directory named "config.toml" and is encoding-fragile on Windows.)
- **G2 — delegate to `get_config_paths()`, do NOT re-cfg per platform.** The whole
  point of "lives in rules.rs but calls platforms::get_config_paths()" is DRY:
  one `map`, no `#[cfg(target_os = ...)]` duplication. Replicating the 3
  platform cfg blocks would drift from the real resolver (e.g. the
  empty-XDG-treated-as-unset guard in linux.rs).
- **G3 — `parse_rules` takes ONE path, does NOT iterate.** It mirrors
  `parse_config` (single `&Path`). Path iteration (`.find(|p| p.exists())`) is
  the caller's job (P5.M1 CLI / P4.M3 notifier). Do NOT make parse_rules loop
  over get_rules_paths().
- **G4 — error type is `Box<dyn Error>`, NOT `Box<dyn Error + Send + Sync>`.**
  Match `parse_config` exactly. The Send+Sync variant is the qmk_notifier crate's
  `run()` signature; the app's own config/rules readers use the plain form.
- **G5 — `effective_disable_firmware_config` is module-private `fn`, NOT `pub fn`.**
  The contract writes `fn` (no pub) for it, `pub fn` for the other two. Its only
  consumer (evaluate(), P3.M1.T2.S1) is in the same module. The test block
  (`use super::*`) can see private items. Do NOT over-expose.
- **G6 — do NOT aggregate the "replace iff all rules true" decision here.** That
  is `evaluate()`'s job (P3.M1.T2.S1). THIS task ships the per-rule primitive
  only. The item description's RESEARCH NOTE (point 1) describes the downstream
  aggregation semantics for CONTEXT, not for implementation here.
- **G7 — do NOT validate callback names in parse_rules.** The §8 point-5
  "validate rules.toml names against name_to_id // warn, don't fail" needs the
  handshake name→id map (P4.M2). It is explicitly a WARN, not a hard error.
  parse_rules does strict STRUCTURAL deserialization only (the free validation
  from serde + S1's required-field strictness).
- **G8 — binary-only crate; doctests don't run under `--bin`.** (Same as S1's G2.)
  Mode-A rustdoc on parse_rules/get_rules_paths should use ` ```rust,ignore `
  fences if a runnable example is included, or plain prose + ` ```toml `. Do NOT
  add a bare ` ``` ` runnable doctest that does `use qmkonnect::...` — it won't
  compile under `cargo test --doc` and `--bin` doesn't run doctests anyway. Best:
  plain `///` prose doc (no code fence needed for these two functions), or a
  ``` ```rust,ignore ``` example showing caller usage.
- **G9 — single-threaded tests crate-wide.** `cargo test --bin qmkonnect --
  --test-threads=1` (AGENTS.md; shared debouncer state in notifier.rs).
- **G10 — no env mutation in get_rules_paths tests.** Assert the
  transformation invariant (same dir, swapped filename) against the REAL
  platform resolver output. Do NOT setenv/unsetenv XDG_CONFIG_HOME/APPDATA (racy,
  process-global, platform-specific — test 9 is strictly better).
- **G11 — do NOT touch S1's structs or S1's tests.** This task is purely
  additive: new imports + 3 functions + new tests. If S1's `test_rules_*` tests
  exist, leave them; append new `test_rules_parse_*`/`test_rules_effective_*`/
  `test_rules_paths_*` tests to the SAME `mod tests` block (or a sibling — but
  one block is idiomatic; mod.rs/pattern.rs each have one `mod tests`).
- **G12 — `RuleSet` must be in scope for parse_rules.** Since parse_rules
  deserializes to `RuleSet` and lives in the same file, no import is needed
  (`RuleSet` is defined in the same module). `use super::*;` in tests brings it
  in for tests too.

---

## 8. Downstream consumer contracts (do NOT implement — just satisfy)

- **P3.M1.T2.S1 (`evaluate()`):** will call
  `effective_disable_firmware_config(rule.disable_firmware_config,
  rule_set.host.disable_firmware_config)` for each matched layer/callback rule,
  then aggregate: window is "replace" iff ALL matched rules' effective flags are
  `true`. THIS task's primitive is the per-rule input to that aggregation.
- **P5.M1.T1.S1 (`--validate-rules`):** will do
  `get_rules_paths().into_iter().find(|p| p.exists()).map(parse_rules)` (or
  iterate the first that exists), then report parse errors. THIS task's
  `parse_rules` returning `Err` on malformed/missing-required-field is exactly
  what `--validate-rules` surfaces.
- **P4.M3.T1.S1 (notify_qmk host-context send):** will call
  `get_rules_paths()` at startup / on "Reload rules" to (re)load the ruleset; if
  none exists, host rules are disabled (string-only). THIS task's
  `get_rules_paths()` is the path source.
- **P5.M2.T1 ("Reload rules" tray item):** calls into the same reload path
  (which calls get_rules_paths + parse_rules).

---

## 9. Scope boundary (do NOT do)

- ❌ Redefine or modify the RuleSet/HostDefaults/LayerRule/CallbackRule structs
      (S1 owns them; this task only consumes them).
- ❌ Implement `evaluate()` / `HostContext` / the stack-vs-replace aggregation
      (P3.M1.T2.S1). THIS task ships the per-rule primitive only.
- ❌ CLI flags (`--validate-rules`, `--list-callbacks`, `--rules-path`) — P5.M1.
- ❌ Tray "Reload rules" item — P5.M2.
- ❌ Startup handshake / proto_ver gating — P4.M2.
- ❌ Wire parse_rules into notifier.rs / main.rs / debounce — P4.M3 / P5.
- ❌ Touch Cargo.toml (serde, toml, tempfile all already deps).
- ❌ Touch platforms/ files (get_rules_paths DELEGATES to the existing
      get_config_paths; platforms/ is unchanged).
- ❌ Touch pattern.rs, notifier.rs, types.rs, mod.rs (beyond what S1 already
      did — and THIS task doesn't even touch mod.rs; rules.rs already registered
      by S1's `pub mod rules;`).
- ❌ docs/*.md / README (Mode A = code-level rustdoc on the two pub fns only).
# PRP — P5.M1.T1.S1: `--list-callbacks` / `--validate-rules` / `--rules-path` dispatch in `src/main.rs`

> **Repo under change:** the **qmkonnect** desktop app (Rust binary) at
> `/home/dustin/projects/qmkonnect`. This 2-point task adds three user-facing
> **diagnostic CLI flags** to `src/main.rs` (PRD §4 / `spec/HOST_RULES.md` §8(6)):
> `--list-callbacks`, `--validate-rules`, and `--rules-path`, and extends
> `-c`/`--config` to seed a commented `rules.toml` template alongside `config.toml`.
>
> **Files touched (2):** `src/main.rs` (primary — all 3 flag branches + `print_help`
> + one line in `create_config` + a pure `collect_callback_names` helper + the file's
> first `#[cfg(test)] mod tests`) and `src/core/mod.rs` (`render_rules_body` +
> `create_default_rules`, mirroring the existing `render_config_body`/`create_default_config`
> precedent). **No Cargo, no notifier.rs, no rules.rs, no platforms/, no runners/, no tray.**
>
> **Consumes (all LANDED, read-only):** `perform_handshake` / `host_capable` /
> `callback_names` / `is_device_connected` (**P4.M2.T1.S1/S2 — LANDED & Complete**,
> real code in `src/core/notifier.rs` at L171/265/440/447), and `rules::parse_rules` /
> `rules::get_rules_paths` / `RuleSet` / `CallbackRule` (P3.M1 — landed).
>
> **Consumed downstream by:** P5.M2.T1.S1/S2 ("Reload rules" tray item — reuses the
> validation path) and P6.M1.T1.S1 (docs/configuration.md CLI reference).
>
> **PARALLEL vs P4.M3.T1.S1 (host-context send, "Implementing"):** that task edits
> `src/core/notifier.rs` ONLY and does NOT touch `src/main.rs` ⇒ **zero file-level
> conflict**. This task does not call `board_has_rules()` (the CLI validation path
> checks schema + names, not the runtime send decision). The two are independent.

---

## ⚠️ READ FIRST — two non-obvious traps

1. **The handshake is LANDED — consume its public API, never reimplement it.**
   `perform_handshake(verbose)`, `host_capable()`, `callback_names()`, and
   `is_device_connected()` are **real, present code** in `src/core/notifier.rs`
   (L171/265/440/447 — re-grepped this session, P4.M2.T1.S1/S2 = Complete). Call them
   via `crate::core::notifier::`. `perform_handshake` is self-contained (builds its own
   `DeviceFilter` internally) — pass ONLY `verbose`; it maps no-device/legacy/timeout to
   `host_capable()==false` without panicking. The handshake's `unknown_callback_names`
   helper (L423) is **private** — main.rs CANNOT call it, hence task helper
   `collect_callback_names` (D6).

2. **Unknown callback names are WARNINGS (exit 0), NOT fatal — and `perform_handshake`
   ALSO emits its own `⚠` warnings.** `--validate-rules` exits non-zero ONLY on
   parse/schema errors. Unknown callback names print `⚠` but return `Ok`, because
   `rules::evaluate` skips unknown names silently (rules.rs
   `test_evaluate_unknown_name_skipped`) and a device may be disconnected. NOTE:
   `perform_handshake` internally runs its own `validate_rules_callback_names` against
   the **default** rules.toml during the sweep, printing its own `⚠` lines **always**
   (regardless of `verbose`) — so when `--validate-rules` calls `perform_handshake` to
   populate the name map, two sets of `⚠` lines may appear (the handshake's for the
   default file; this tool's authoritative ones for the resolved file). This is benign
   supplementary noise (both non-fatal); do NOT try to suppress the handshake's output.
   See D5b/G6.

---

## Goal

**Feature Goal**: Add three diagnostic CLI flags to `src/main.rs` that let a user
(a) see their keyboard's callback name→id table after a live handshake
(`--list-callbacks`), (b) lint a `rules.toml` for schema/callback-name errors before
relying on it (`--validate-rules`, with optional `--rules-path <p>`), and (c) get a
commented `rules.toml` template seeded alongside `config.toml` on `-c`/`--config` — so
host-rules setup is self-documenting and debuggable without reflashing or guessing.

**Deliverable** (2 files):
- **`src/main.rs`**: 3 new early-return flag branches in `run()` (after
  `--show-window-info`, before the runner), a `print_help()` update (3 new flags + the
  currently-missing `--show-window-info` line), one line added to `create_config()` to
  seed `rules.toml`, a small pure helper `collect_callback_names(&RuleSet) ->
  BTreeSet<String>`, and the file's first `#[cfg(test)] mod tests` (5 pure-logic tests).
- **`src/core/mod.rs`**: `pub fn render_rules_body() -> String` + `pub fn
  create_default_rules(path: &Path) -> Result<(), Box<dyn Error>>`, mirroring the
  existing `render_config_body`/`create_default_config` pair; 4 new tests.

**Success Definition**:
- `qmkonnect --help` lists `--list-callbacks`, `--validate-rules`, `--rules-path` (and
  `--show-window-info`), grouped after `--list-devices`.
- `qmkonnect --list-callbacks` with a v2-capable board connected prints the name→id table
  (sorted by id); with legacy firmware prints "Legacy firmware (no callback support)…";
  with no board prints a clear "no device" message.
- `qmkonnect --validate-rules` parses the first existing `rules.toml` and prints "valid:
  N layer rules, M callback rules" (exit 0); a malformed file exits non-zero with the
  parse error; `--rules-path <p>` overrides the location (missing explicit path ⇒ non-zero).
- `qmkonnect -c` writes a **fully-commented** `rules.toml` (parses to all-defaults, host
  rules still disabled) next to `config.toml`; re-running is a no-op.
- `cargo build --bin qmkonnect` clean; `cargo test --bin qmkonnect -- --test-threads=1`
  green (9 new + all existing).
- `git diff --stat` = `src/main.rs` + `src/core/mod.rs` ONLY.

## User Persona (if applicable)

**Target User**: the end user configuring host rules, plus the developer integrating a
new keyboard.

**Use Case**: "I wrote a `rules.toml` referencing callback names; before I trust it, I
run `--validate-rules` to catch typos/schema errors. To discover the exact callback names
my firmware advertises, I run `--list-callbacks`. On first install `-c` drops a commented
template I can edit."

**User Journey**: install → `qmkonnect -c` (gets `config.toml` + commented `rules.toml`)
→ edit `rules.toml` → `qmkonnect --validate-rules` (green) → `qmkonnect --list-callbacks`
(confirms names) → start the service (no args).

**Pain Points Addressed**: today there is NO way to validate `rules.toml` short of running
the service and watching for silent misbehavior, and NO way to discover a keyboard's
callback names short of reading firmware source.

## Why

- **PRD §4 (`h2.59`)** + **HOST_RULES.md §8(6)** — these flags are the canonical CLI
  contract for the host-rules feature.
- **HOST_RULES.md §9** — the `-c` seeding drops the §9 schema as a commented template,
  making the file format self-documenting (no separate doc lookup to author a valid
  `rules.toml`).
- **PRD §8(8)** (backward compat) — a fully-commented template parses to all-defaults ⇒ a
  fresh install behaves identically to today (host rules disabled) until the user opts in.
- **Unblocks** P5.M2 (tray "Reload rules" reuses the validation path) and P6 (docs
  reference the flags).

## What

Three new early-return branches in `run()` + a `print_help()` update + a one-line
`create_config()` extension + a small `core::render_rules_body`/`create_default_rules`
pair. No change to the runner, tray, notifier, rules evaluator, debounce, or any platform
code. The only observable behavior change is the new flag dispatches and the extra
`rules.toml` file written by `-c`.

### Success Criteria
- [ ] `--list-callbacks` branch: pre-checks `is_device_connected()`; if connected calls
      `perform_handshake(true)` then prints the `callback_names()` table (sorted by id) or
      "Legacy firmware (no callback support)…"; if not connected prints a clear no-device
      message; always `return Ok(())`.
- [ ] `--validate-rules` branch: resolves path via `--rules-path` (explicit; missing ⇒
      Err/exit 1) else `get_rules_paths().find(exists)` (none ⇒ info/exit 0); `parse_rules`
      Err ⇒ `return Err(...)` (exit 1); Ok ⇒ optional device-connected handshake +
      unknown-name warnings (exit 0) + success summary.
- [ ] `--rules-path` is parsed via `parse_value_flag` inside the `--validate-rules` branch
      only; no standalone action.
- [ ] `print_help()` documents the 3 new flags + `--show-window-info`.
- [ ] `create_config()` seeds `rules.toml` via `core::create_default_rules` (no-op if it
      exists); the template is **fully commented** (parses to all-defaults).
- [ ] 9 new tests pass; all existing tests green; `--test-threads=1`.
- [ ] `git diff --stat` = `src/main.rs` + `src/core/mod.rs` ONLY.

## All Needed Context

### Context Completeness Check

_Pass_: an agent with no prior knowledge can implement this using only this PRP +
`research/notes.md`, because: (a) the handshake is LANDED with exact signatures + line
numbers in research §0.1 (`perform_handshake(verbose)` L265, `host_capable()` L440,
`callback_names()` L447, `is_device_connected()` L171 — all `pub`); (b) `unknown_callback_names`
(L423) is confirmed PRIVATE ⇒ main.rs owns `collect_callback_names` (D6); (c) the verbatim
CURRENT `run()` dispatch order + anchors are in research §1.1 (exact insertion point: after
`--show-window-info` L124, before `runners::create_runner` L126); (d) the verbatim `print_help()`
text is in research §1.4 (copy → add 4 lines); (e) the verbatim `create_config()` body is in
research §1.3 (add 1 line); (f) the verbatim `parse_value_flag` is in research §1.2 (reused
as-is, now tested); (g) the rules module API (`parse_rules`/`get_rules_paths`/`RuleSet`/
`CallbackRule`) is in research §0.3; (h) the §9 schema (the exact template content) is in
research §2.3; (i) the 8 design decisions (D1-D8) + 9 gotchas (G1-G9) + 9-test plan are in
research §3-§5; (j) verified validation commands are in research §6.

### Documentation & References

```yaml
# MUST READ — the verbatim research (THIS task's full contract + design + safety)
- file: plan/002_637d65b6e9b8/P5M1T1S1/research/notes.md
  why: "§0 = the 3 dependency contracts (handshake LANDED w/ line numbers / rules / send).
        §1 = verbatim CURRENT main.rs anchors (run() order L70-128, parse_value_flag L190,
        create_config L269, print_help L130). §2 = the spec sources (PRD §4, HOST_RULES
        §8(6)/§9 verbatim). §3 = D1-D8 design. §4 = G1-G9 gotchas. §5 = 9-test plan. §6 = validation."

# MUST READ — the spec sources of truth (selected sections are in this PRP's header)
- file: spec/HOST_RULES.md
  why: "§8(6) is the CANONICAL CLI contract (--list-callbacks/--validate-rules/--rules-path
        + -c seeds rules.toml). §9 is the verbatim rules.toml schema the seeded template must
        mirror (commented out)."
  section: "§8(6) (CLI), §9 (rules.toml Schema Reference)"

# MUST READ — the file THIS task edits (primary)
- file: src/main.rs
  why: "run() dispatch (L70-128) is where the 3 branches insert (after --show-window-info L111-124,
        before runners::create_runner L126). print_help (L130) + create_config (L269) +
        parse_value_flag (L190) are the other edit sites. Has NO #[cfg(test)] today — this task
        adds the first."
  pattern: "args.iter().any(|a| a == \"--flag\") boolean scan; parse_value_flag for --flag value /
            --flag=value; early `return Ok(())` / `return Err(...)`."
  gotcha: "do NOT touch the runner, -h/-c/-r/-l/--list-devices/--show-window-info branches,
           reload_config (L204), get_config_path, or init_logging."

# MUST READ — the other file THIS task edits (the rules-template helper)
- file: src/core/mod.rs
  why: "render_config_body (L96) + create_default_config (L134) are the EXACT precedent to mirror
        for render_rules_body + create_default_rules (same no-op-if-exists, same fs::create_dir_all
        on parent, same println confirmation). parse_config (L82) shows the fs::read_to_string +
        toml::from_str idiom. Has #[cfg(test)] at L201 (render_config_body_round_trips at L203)."
  pattern: "pub fn render_X_body() -> String (pure) + pub fn create_default_X(path) -> Result
            (no-op if exists)."
  gotcha: "the rules template MUST be fully commented (G7) so it parses to all-defaults."

# MUST READ — the handshake LANDED source (consume read-only)
- file: src/core/notifier.rs
  why: "the PUBLIC handshake surface this task calls: pub fn perform_handshake(verbose: bool) (L265),
        pub fn host_capable()->bool (L440), pub fn callback_names()->HashMap<String,u8> (L447),
        pub fn is_device_connected()->bool (L171). perform_handshake ALSO runs the private
        validate_rules_callback_names on the default rules.toml (⚠ warnings always print — D5b).
        unknown_callback_names (L423) is PRIVATE — hence D6's collect_callback_names in main.rs."
  section: "perform_handshake (L265), host_capable (L440), callback_names (L447), is_device_connected (L171)"

# MUST READ — the rules module (P3.M1, landed — read-only consumer)
- file: src/core/rules.rs
  why: "parse_rules(&Path)->Result<RuleSet,Box<dyn Error>> (strict: missing match/layer + malformed
        TOML surface as Err). get_rules_paths()->Vec<PathBuf>. RuleSet{host,layer_rules,callback_rules};
        CallbackRule{enable:Vec<String>, disable:Vec<String>}. The §9 schema example is in the module
        doc comment."
  section: "parse_rules, get_rules_paths, RuleSet, CallbackRule"

# MUST READ — the parallel item (conflict-free; context only)
- file: plan/002_637d65b6e9b8/P4M3T1S1/PRP.md
  why: "P4.M3.T1.S1 edits src/core/notifier.rs ONLY (board_has_rules + send logic) and does NOT touch
        src/main.rs. Confirms zero file conflict and that this task need not call board_has_rules()."
```

### Current Codebase tree (relevant subset)

```bash
src/
  main.rs            # ← THIS TASK EDITS (primary): +3 flag branches in run(),
                     #   print_help update, +1 line in create_config, +collect_callback_names,
                     #   +#[cfg(test)] mod tests (first in this file).
  core/
    mod.rs           # ← THIS TASK EDITS: +render_rules_body +create_default_rules +4 tests.
    notifier.rs      # P4.M2 handshake LANDED + P4.M3 send (in progress). UNCHANGED (consumed read-only).
    rules.rs         # P3.M1 parse_rules/get_rules_paths/RuleSet. UNCHANGED (read-only).
    pattern.rs       # P2.M1 matcher. UNCHANGED.
    types.rs         # UNCHANGED.
spec/HOST_RULES.md   # §8(6) CLI + §9 schema. READ-ONLY reference.
Cargo.toml           # L19 qmk_notifier tag="v0.3.0". UNCHANGED.
```

### Desired Codebase tree with files to be changed

```bash
src/main.rs          # MODIFIED: +3 flag branches, print_help, create_config line,
                     #   collect_callback_names helper, #[cfg(test)] mod tests (5 tests).
src/core/mod.rs      # MODIFIED: +render_rules_body +create_default_rules +4 tests.
# EVERYTHING ELSE UNCHANGED. No Cargo, no notifier.rs, no rules.rs, no platforms/,
# no runners/, no tray.rs, no linux_tray.rs.
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (G1 — build precondition): qmk_notifier v0.3.0 must resolve (Cargo.toml:19).
//   P4.M2 (handshake) already builds against it in the current tree, so if the handshake
//   compiles, v0.3.0 resolves. A fetch failure is env/network, not a code bug — halt+report.

// CRITICAL (G2 — handshake LANDED; consume, don't reimplement): perform_handshake/host_capable/
//   callback_names/is_device_connected are REAL pub fns (notifier.rs L171/265/440/447). Call via
//   crate::core::notifier::. unknown_callback_names (L423) is PRIVATE => main.rs owns
//   collect_callback_names (D6).

// CRITICAL (G3 — single source of truth): always validate via rules::parse_rules. Its strictness
//   (missing match/layer, malformed TOML) IS the schema check. Do NOT re-parse by hand — it would
//   diverge from the runtime evaluator's acceptance.

// CRITICAL (G5 — explicit-path-missing ≠ no-path-found): --rules-path foo where foo doesn't
//   exist => Err/exit 1 (user asked for a specific file). No --rules-path and no candidate exists
//   => info/exit 0 (host rules disabled is valid). Do NOT conflate.

// CRITICAL (G6 — unknown names are warnings, exit 0): do NOT make unknown callback names fatal.
//   evaluate skips them silently; a device may be disconnected. Only parse/schema errors exit
//   non-zero. NOTE (D5b): perform_handshake ALSO prints its own ⚠ warnings for the DEFAULT
//   rules.toml during the sweep — benign supplementary noise; do NOT try to suppress it.

// CRITICAL (G7 — template fully commented): render_rules_body prefixes EVERY non-blank line with
//   "# ". An uncommented template would activate bogus example rules and break a fresh install's
//   legacy parity. The seeded file must parse_rules to all-defaults.

// GOTCHA (G4 — --rules-path alone is a no-op): parsed ONLY inside --validate-rules. No standalone
//   action. print_help says "use with --validate-rules".

// GOTCHA (G8 — binary crate, no lib doctests): Mode-A rustdoc uses ```rust,ignore fences.
//   Match rules.rs/pattern.rs/notifier.rs.

// GOTCHA (G9 — first #[cfg(test)] in main.rs): keep tests pure-logic only (parse_value_flag,
//   collect_callback_names). Do NOT unit-test run() (env::args + IO).
```

## Implementation Blueprint

### Data models and structure

```rust
// ── in src/main.rs ──

/// Collect every callback name referenced by a parsed `rules.toml` (the union of all
/// `callback_rules[].enable` + `callback_rules[].disable`), deduped + sorted. Pure (no IO,
/// no globals) ⇒ thread-safe + unit-testable. Used by `--validate-rules` to report names not
/// present in the live handshake map. (`BTreeSet` ⇒ deterministic sorted output.) Required
/// because notifier.rs's `unknown_callback_names` (L423) is private (D6).
fn collect_callback_names(rules: &crate::core::rules::RuleSet) -> std::collections::BTreeSet<String> {
    let mut names = std::collections::BTreeSet::new();
    for rule in &rules.callback_rules {
        for n in rule.enable.iter().chain(rule.disable.iter()) {
            names.insert(n.clone());
        }
    }
    names
}

// ── in src/core/mod.rs ──

/// Render a fully-commented `rules.toml` template (the `spec/HOST_RULES.md` §9 schema with
/// every active line prefixed by `# `). A freshly-seeded file therefore parses to an
/// all-default `RuleSet` (host rules disabled) — a brand-new install behaves identically to
/// today until the user uncomments and edits. Mirrors `render_config_body`.
pub fn render_rules_body() -> String { /* the §9 schema, every active line commented */ }

/// Create a default (commented) `rules.toml` next to `config.toml`. No-op + message if it
/// already exists (mirrors `create_default_config`). Creates the parent dir.
pub fn create_default_rules(rules_path: &std::path::Path) -> Result<(), Box<dyn Error>> { /* ... */ }
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: ADD render_rules_body + create_default_rules to src/core/mod.rs
  - ADD (beside render_config_body L96 / create_default_config L134): `pub fn
    render_rules_body() -> String` returning the §9 schema (research §2.3) with EVERY
    active line prefixed by "# " (G7). Include the header comment block, `[host]`, two
    `[[layer_rules]]`, two `[[callback_rules]]` examples.
  - ADD: `pub fn create_default_rules(rules_path: &Path) -> Result<(), Box<dyn Error>>`
    mirroring create_default_config EXACTLY: if exists -> println + return Ok; else
    fs::create_dir_all(parent)?; fs::write(path, render_rules_body())?; println confirm.
  - FOLLOW pattern: src/core/mod.rs render_config_body (L96) + create_default_config (L134).
  - NAMING: render_rules_body / create_default_rules (parallel to the config pair).
  - GOTCHA G7: every active line commented; the rendered body MUST toml::from_str::<
    rules::RuleSet> to an all-default RuleSet (Test 7 proves this).
  - VERIFY: grep -n 'fn render_rules_body\|fn create_default_rules' src/core/mod.rs -> 2.

Task 2: ADD collect_callback_names helper to src/main.rs
  - ADD (e.g. just above run(), or above print_help): the fn from Data Models above.
  - DEPENDENCIES: crate::core::rules::RuleSet (in scope via `core` mod). std::collections::BTreeSet.
  - NAMING: collect_callback_names; returns BTreeSet<String>.
  - GOTCHA G2: notifier::unknown_callback_names is PRIVATE (L423) -> this own helper is REQUIRED.
  - VERIFY: grep -n 'fn collect_callback_names' src/main.rs -> 1.

Task 3: ADD the 3 new flag branches to run() (after --show-window-info, before runner)
  - INSERT (between the #[cfg(any(macos,windows))] --show-window-info block L111-124 and
    `let mut runner = runners::create_runner(verbose)?;` L126):

    // --list-callbacks: handshake the connected keyboard and print its callback table.
    if args.iter().any(|a| a == "--list-callbacks") {
        return list_callbacks(verbose);
    }

    // --validate-rules: lint rules.toml (schema + callback-name warnings).
    if args.iter().any(|a| a == "--validate-rules") {
        let rules_path = parse_value_flag(&args, "--rules-path").map(PathBuf::from);
        return validate_rules(rules_path, verbose);
    }

    // (--rules-path has no standalone action; it is consumed by --validate-rules above.)
  - ADD two helper fns (after run(), near create_config): `fn list_callbacks(verbose)
    -> Result<(), Box<dyn Error>>` and `fn validate_rules(rules_path: Option<PathBuf>,
    verbose: bool) -> Result<(), Box<dyn Error>>` with bodies per research §3 D2/D3/D4/D5:
      list_callbacks:
        if !crate::core::notifier::is_device_connected() {
            println!("No QMK device connected. Connect a keyboard with host-rules firmware and re-run.");
            return Ok(());
        }
        crate::core::notifier::perform_handshake(true);
        if crate::core::notifier::host_capable() {
            let names = crate::core::notifier::callback_names();  // HashMap<String,u8>
            if names.is_empty() {
                println!("Connected keyboard reports 0 callbacks.");
            } else {
                let mut rows: Vec<_> = names.into_iter().collect();
                rows.sort_by_key(|(_, id)| *id);
                println!("Callback name -> id ({}):", rows.len());
                for (name, id) in rows { println!("  {id:>3}  {name}"); }
            }
        } else {
            println!("Legacy firmware (no callback support) — host rules will run in string-only mode.");
        }
        Ok(())
      validate_rules:
        // resolve path (D3): explicit --rules-path (missing => Err) else first existing candidate.
        let path = match rules_path {
            Some(p) => {
                if !p.exists() { eprintln!("rules file not found: {}", p.display()); return Err(format!("rules file not found: {}", p.display()).into()); }
                p
            }
            None => match crate::core::rules::get_rules_paths().into_iter().find(|p| p.exists()) {
                Some(p) => p,
                None => { println!("No rules.toml found (host rules disabled). Nothing to validate."); return Ok(()); }
            },
        };
        println!("Validating {}", path.display());
        let rs = match crate::core::rules::parse_rules(&path) {
            Ok(rs) => rs,
            Err(e) => { eprintln!("rules.toml invalid: {e}"); return Err(e); }   // exit non-zero (D4)
        };
        // optional name validation (D5): only if a device is connected + capable.
        if crate::core::notifier::is_device_connected() {
            crate::core::notifier::perform_handshake(verbose);   // populates callback_names(); may ALSO print
                                                                 // its own ⚠ warnings on the DEFAULT rules.toml (D5b — benign)
            if crate::core::notifier::host_capable() {
                let known = crate::core::notifier::callback_names();
                let unknown = collect_callback_names(&rs).into_iter().filter(|n| !known.contains_key(n));
                let mut warned = false;
                for n in unknown { eprintln!("⚠  unknown callback: {n}"); warned = true; }
                if !warned { println!("All callback names recognized."); }
            } else {
                println!("Legacy firmware — callback-name validation skipped (schema-only).");
            }
        } else {
            println!("Device not connected — callback-name validation skipped (schema-only).");
        }
        println!("rules.toml valid: {} layer rules, {} callback rules.", rs.layer_rules.len(), rs.callback_rules.len());
        Ok(())
  - FOLLOW pattern: the existing -h/-c/-r/-l/--list-devices branches (early return).
  - GOTCHA G6: unknown names are warnings (eprintln) — do NOT return Err for them.
  - GOTCHA D5b: do NOT try to suppress perform_handshake's own ⚠ output (no flag exists).
  - VERIFY: grep -n 'fn list_callbacks\|fn validate_rules' src/main.rs -> 2;
    grep -n '"--list-callbacks"\|"--validate-rules"\|"--rules-path"' src/main.rs -> 3.

Task 4: UPDATE print_help() — add 3 new flags + the missing --show-window-info line
  - EDIT print_help(): after the `--list-devices` println, ADD (wording per PRD §4 / §8(6)):
        println!("  --show-window-info  [macOS/Windows] open the Window Information dialog");
        println!("      --list-callbacks   Handshake the keyboard; print its callback name->id table");
        println!("      --validate-rules   Parse rules.toml; report schema/callback-name errors");
        println!("          --rules-path <path>  Override the rules.toml location (with --validate-rules)");
  - GOTCHA: --show-window-info is dispatched (L111) but currently MISSING from help — add it.
  - VERIFY: grep -n 'list-callbacks\|validate-rules\|rules-path\|show-window-info' src/main.rs -> >=4.

Task 5: EXTEND create_config() to seed rules.toml
  - EDIT create_config() (L269): after `core::create_default_config(&config_path)?;` (L277) ADD:
        let rules_path = config_dir.join("rules.toml");
        core::create_default_rules(&rules_path)?;
  - PRESERVE: the existing config.toml seeding (order: config first, then rules).
  - GOTCHA G7: create_default_rules no-ops if rules.toml already exists.
  - VERIFY: grep -n 'create_default_rules' src/main.rs -> 1.

Task 6: MID-POINT build gate
  - RUN: cargo build --bin qmkonnect   (expect clean — G1 v0.3.0; handshake LANDED).
  - RUN: cargo run -- --help | grep -E 'list-callbacks|validate-rules|rules-path'  (3 hits).

Task 7: ADD the 5 main.rs tests (new #[cfg(test)] mod tests at end of main.rs)
  - ADD `#[cfg(test)] mod tests { use super::*; ... }` with 5 pure tests (research §5.1):
      test_parse_value_flag_space_form, test_parse_value_flag_equals_form,
      test_parse_value_flag_absent, test_collect_callback_names_dedupes,
      test_collect_callback_names_empty_when_no_rules.
  - For collect_callback_names tests, build a RuleSet via `toml::from_str::<
    crate::core::rules::RuleSet>(r#"..."#)` (parse from str; tempfile NOT needed).
  - NAMING: test_<fn>_<scenario>.
  - GOTCHA G9: pure only; no run()/env::args/IO. Still run --test-threads=1 (AGENTS.md).
  - VERIFY: cargo test --bin qmkonnect parse_value_flag -- --test-threads=1 -> 3 passed;
    cargo test --bin qmkonnect collect_callback_names -- --test-threads=1 -> 2 passed.

Task 8: ADD the 4 core/mod.rs tests (append to existing #[cfg(test)] mod tests at L201)
  - APPEND (research §5.2): test_render_rules_body_fully_commented (every non-blank line
    starts with '#'), test_render_rules_body_parses_to_default_ruleset
    (toml::from_str::<rules::RuleSet> of the body => Ok, 0 rules),
    test_create_default_rules_noop_if_exists (tempfile; pre-write; call => unchanged),
    test_create_default_rules_writes_when_absent (tempfile; absent => written; re-call no-op).
  - FOLLOW pattern: the existing render_config_body_round_trips test (L203) + tempfile usage.
  - VERIFY: cargo test --bin qmkonnect render_rules_body -- --test-threads=1 -> 2 passed;
    cargo test --bin qmkonnect create_default_rules -- --test-threads=1 -> 2 passed.

Task 9: VALIDATE (build + full suite + scope)
  - cargo build --bin qmkonnect
  - cargo test --bin qmkonnect -- --test-threads=1     # MANDATORY single-threaded (AGENTS.md). All green.
  - cargo clippy --bin qmkonnect --no-deps 2>/dev/null || true   # no NEW warnings.
  - git diff --stat                                     # expect src/main.rs + src/core/mod.rs ONLY.
```

### Implementation Patterns & Key Details

```rust
// THE boolean-flag scan + early-return idiom (every new branch follows this):
if args.iter().any(|a| a == "--list-callbacks") {
    return list_callbacks(verbose);
}

// THE value-flag extraction (--rules-path), reusing the existing parse_value_flag:
let rules_path = parse_value_flag(&args, "--rules-path").map(PathBuf::from);
// accepts BOTH `--rules-path foo.toml` and `--rules-path=foo.toml` (parse_value_flag L190).

// THE path-resolution priority (D3) — explicit-missing is an error, no-candidate is info:
let path = match rules_path {
    Some(p) if !p.exists() => return Err(format!("rules file not found: {}", p.display()).into()), // G5
    Some(p) => p,
    None => match rules::get_rules_paths().into_iter().find(|p| p.exists()) {
        Some(p) => p,
        None => { println!("No rules.toml found. Nothing to validate."); return Ok(()); }  // G5 exit 0
    },
};

// THE validation outcome split (D4 parse-fatal vs D5 name-warning):
let rs = match rules::parse_rules(&path) {
    Ok(rs) => rs,
    Err(e) => { eprintln!("rules.toml invalid: {e}"); return Err(e); }   // exit NON-ZERO (G6)
};
// ... unknown names print ⚠ but do NOT return Err (G6: exit 0) ...
// (D5b: perform_handshake may ALSO print ⚠ for the default rules.toml — benign.)

// THE no-op-if-exists template write (mirrors create_default_config verbatim):
pub fn create_default_rules(rules_path: &Path) -> Result<(), Box<dyn Error>> {
    if rules_path.exists() {
        println!("rules.toml already exists at: {}", rules_path.display());
        return Ok(());
    }
    if let Some(parent) = rules_path.parent() { fs::create_dir_all(parent)?; }
    fs::write(rules_path, render_rules_body())?;
    println!("rules.toml template created at: {}", rules_path.display());
    Ok(())
}

// THE fully-commented template (G7): every active line prefixed with "# " so it
// parses to all-defaults. render_rules_body() returns the §9 schema commented out.
```

### Integration Points

```yaml
MODULE REGISTRATION: NONE. `mod core`/`mod main` are long-standing. This task adds items to
  the BODY of main.rs (3 branches + 2 helper fns + 1 helper + tests) and core/mod.rs
  (2 fns + tests).

DEPENDENCIES (this task): std collections BTreeSet/HashMap, std::path::{Path,PathBuf},
  std::fs (core/mod.rs only), toml (core/mod.rs test only — already a dep). NO new Cargo.

UPSTREAM (consumed unchanged — all LANDED/verified):
  - perform_handshake/host_capable/callback_names/is_device_connected (P4.M2.T1.S1/S2 LANDED).
  - rules::parse_rules/get_rules_paths/RuleSet/CallbackRule (P3.M1 LANDED).
  - platforms::create_config_dir (existing). core::create_default_config (existing L134).

DOWNSTREAM CONSUMERS:
  - P5.M2.T1.S1/S2 ("Reload rules" tray) — will want to re-call the validation path; consider
    extracting validate_rules' core into a pub helper later (out of scope here).
  - P6.M1.T1.S1 (docs/configuration.md) — references the new flags verbatim.

CONFIG: none new. ROUTES: none. DATABASE: none. TRAY: none (P5.M2). CLI: yes (this task).
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# EXPECT: clean. G1: v0.3.0 resolves (handshake LANDED compiles against it); the new
#   crate::core::notifier::{perform_handshake,host_capable,callback_names,is_device_connected}
#   calls resolve to real pub fns.

# Confirm the edits landed at the right anchors:
grep -n 'fn list_callbacks\|fn validate_rules\|fn collect_callback_names' src/main.rs   # 3
grep -n '"--list-callbacks"\|"--validate-rules"\|"--rules-path"' src/main.rs             # 3
grep -n 'create_default_rules' src/main.rs                                               # 1
grep -n 'fn render_rules_body\|fn create_default_rules' src/core/mod.rs                  # 2

cargo clippy --bin qmkonnect --no-deps 2>/dev/null || true   # optional; no NEW warnings.
```

### Level 2: Unit Tests (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# main.rs pure-logic tests (first #[cfg(test)] in this file):
cargo test --bin qmkonnect parse_value_flag -- --test-threads=1            # 3 passed
cargo test --bin qmkonnect collect_callback_names -- --test-threads=1      # 2 passed

# core/mod.rs template tests:
cargo test --bin qmkonnect render_rules_body -- --test-threads=1           # 2 passed
cargo test --bin qmkonnect create_default_rules -- --test-threads=1        # 2 passed
```

### Level 3: Manual CLI smoke (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
# Help lists the new flags (+ the previously-missing --show-window-info):
cargo run -- --help | grep -E 'list-callbacks|validate-rules|rules-path|show-window-info'   # 4 hits

# --validate-rules with NO rules.toml => exit 0, info message (G5):
cargo run -- --validate-rules ; echo "exit=$?"                                                # exit=0

# --validate-rules --rules-path <missing> => exit NON-zero (G5):
cargo run -- --validate-rules --rules-path /tmp/does-not-exist.toml ; echo "exit=$?"          # exit!=0

# --validate-rules on a malformed file => exit NON-zero (schema error, G6):
printf 'not = valid = toml' > /tmp/bad-rules.toml
cargo run -- --validate-rules --rules-path /tmp/bad-rules.toml ; echo "exit=$?"                # exit!=0
rm -f /tmp/bad-rules.toml

# -c seeds a fully-commented rules.toml that parses to all-defaults (G7):
# (run in a throwaway HOME to avoid touching the real config dir):
HOME=$(mktemp -d) cargo run -- -c
# then inspect the seeded rules.toml in that HOME's config dir — every active line commented.
```

> `--list-callbacks` needs real v2-capable hardware ⇒ manual-only (not automatable in CI).
> On a connected legacy board it prints "Legacy firmware (no callback support)…"; with no
> board it prints the no-device message.

### Level 4: Full-crate regression + scope gate

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# EXPECT: ALL bin tests green — the 9 new + handshake (P4.M2) + send (P4.M3, if landed) +
#   debounce + rules (P3) + pattern (P2) + types + linux_tray. Proves the new
#   flags/helpers didn't regress anything.

git status --short && git diff --stat
# EXPECT: exactly src/main.rs + src/core/mod.rs. NOTHING in Cargo.toml, notifier.rs,
#   rules.rs, types.rs, platforms/, runners/, tray.rs, linux_tray.rs.
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean (G1 v0.3.0; handshake LANDED resolves).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` green (9 new + all existing; AGENTS.md).
- [ ] `git diff --stat` = `src/main.rs` + `src/core/mod.rs` ONLY (scope gate).
- [ ] (optional) `cargo clippy --bin qmkonnect --no-deps` introduces no NEW warnings.

### Feature Validation (contract fidelity — PRD §4 / HOST_RULES.md §8(6))
- [ ] `--help` lists `--list-callbacks`, `--validate-rules`, `--rules-path`, `--show-window-info`.
- [ ] `--list-callbacks`: no-device message; or handshake → table (capable) / "Legacy firmware…".
- [ ] `--validate-rules`: valid ⇒ summary + exit 0; malformed ⇒ exit non-zero; no file ⇒ exit 0 info.
- [ ] `--validate-rules --rules-path <missing>` ⇒ exit non-zero (G5).
- [ ] Unknown callback names print `⚠` but exit 0 (G6); perform_handshake's own `⚠` lines are benign (D5b).
- [ ] `-c` seeds a fully-commented `rules.toml` (parses to all-defaults; G7); re-run is no-op.

### Code Quality Validation
- [ ] New branches follow the `args.iter().any(...)` + early-return idiom (parse_value_flag for values).
- [ ] `render_rules_body`/`create_default_rules` mirror `render_config_body`/`create_default_config`.
- [ ] `collect_callback_names` is pure (BTreeSet ⇒ deterministic) — required because
      `notifier::unknown_callback_names` is private (G2/D6).
- [ ] No out-of-scope work: no Cargo/notifier.rs/rules.rs/platforms/runners/tray edits.
- [ ] Did NOT reimplement the handshake (consumed via `crate::core::notifier::`).

### Documentation & Deployment
- [ ] New fns have Mode-A rustdoc (`rust,ignore` fences — binary crate, G8).
- [ ] print_help text matches PRD §4 / HOST_RULES.md §8(6) wording.
- [ ] Commit message notes: "adds --list-callbacks/--validate-rules/--rules-path diagnostics;
      -c seeds a commented rules.toml template; extends print_help (adds the missing
      --show-window-info line)."

---

## Anti-Patterns to Avoid

- ❌ Don't reimplement the handshake (perform_handshake/host_capable/callback_names) — they
  are LANDED pub fns (notifier.rs L171/265/440/447); consume via `crate::core::notifier::` (G2).
- ❌ Don't try to call `notifier::unknown_callback_names` (L423) — it is **private**. main.rs
  owns its own `collect_callback_names` (D6/G2).
- ❌ Don't re-parse `rules.toml` by hand — `rules::parse_rules` IS the schema check (its
  strictness for missing `match`/`layer` + malformed TOML is the validation); re-implementing
  diverges from the runtime evaluator (G3).
- ❌ Don't make unknown callback names fatal — they're warnings (exit 0). Only parse/schema
  errors exit non-zero (G6).
- ❌ Don't conflate explicit-`--rules-path`-missing (error) with no-candidate-found (info) —
  they have different exit codes (G5).
- ❌ Don't try to suppress `perform_handshake`'s own `⚠` warnings on the default rules.toml —
  no flag exists; they're benign supplementary noise (D5b/G6).
- ❌ Don't seed an UNCOMMENTED rules.toml template — it would activate bogus example rules and
  break a fresh install's legacy parity. Every active line must be `# `-prefixed so it parses
  to all-defaults (G7).
- ❌ Don't add a standalone `--rules-path` action — it's a modifier for `--validate-rules` only (G4).
- ❌ Don't unit-test `run()` (it calls `env::args()` + IO/device dispatch) — test the pure
  helpers (`parse_value_flag`, `collect_callback_names`) and the template renderer instead (G9).
- ❌ Don't run tests multi-threaded — `--test-threads=1` is mandatory (AGENTS.md; shared globals
  in other bin tests).
- ❌ Don't edit Cargo.toml/notifier.rs/rules.rs/platforms/runners/tray — this is a 2-file change
  (main.rs + core/mod.rs) only.
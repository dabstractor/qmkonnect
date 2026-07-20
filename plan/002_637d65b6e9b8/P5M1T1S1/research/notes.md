# Research Notes — P5.M1.T1.S1: `--list-callbacks` / `--validate-rules` / `--rules-path` dispatch in `src/main.rs`

> **Repo under change:** the **qmkonnect** desktop app (Rust binary) at
> `/home/dustin/projects/qmkonnect`. This 2-point task adds three user-facing
> **diagnostic CLI flags** to `src/main.rs` (PRD §4 / `spec/HOST_RULES.md` §8(6)):
> `--list-callbacks`, `--validate-rules`, and `--rules-path`, and extends
> `-c`/`--config` to seed a commented `rules.toml` template alongside `config.toml`.
>
> **Files touched (2):** `src/main.rs` (primary — all 3 flag branches +
> `print_help` + one line in `create_config` + the first `#[cfg(test)] mod tests`
> in this file) and `src/core/mod.rs` (`render_rules_body` + `create_default_rules`,
> mirroring the existing `render_config_body`/`create_default_config` precedent).
> **No Cargo, no notifier.rs, no rules.rs, no platforms/, no runners/, no tray.**
>
> **Status note:** the capability handshake (**P4.M2.T1.S1 + S2**) is now **LANDED
> and Complete** — `perform_handshake`/`host_capable`/`callback_names`/
> `is_device_connected` are real, present code in `src/core/notifier.rs` (signatures
> + line numbers in §0.1). This task consumes them read-only. The host-context send
> (**P4.M3.T1.S1**) is still "Implementing" but edits `src/core/notifier.rs` ONLY —
> zero file conflict with this task (which edits `src/main.rs` + `src/core/mod.rs`).

---

## §0 — Dependency contracts (all LANDED/verified against current source)

### §0.1 The capability handshake — **P4.M2.T1.S1/S2 (LANDED, read-only)**

Verified present in `src/core/notifier.rs` (re-grepped this session):

```
L171  pub fn is_device_connected() -> bool                       # read-only HID enumerate
L202  static HOST_CAPABLE: AtomicBool = AtomicBool::new(false);
L208  static CALLBACK_NAMES: Lazy<Mutex<HashMap<String, u8>>> = Lazy::new(|| Mutex::new(HashMap::new()));
L216  static HAS_HANDSHAKED: AtomicBool = AtomicBool::new(false);
L222  fn host_os() -> qmk_notifier::HostOs                       # PRIVATE (not needed here)
L265  pub fn perform_handshake(verbose: bool)                    # idempotent (HAS_HANDSHAKED swap)
L423  fn unknown_callback_names(rules, known) -> Vec<String>     # PRIVATE (see D6)
L440  pub fn host_capable() -> bool
L447  pub fn callback_names() -> HashMap<String, u8>             # returns a CLONE (safe to iterate)
L456  pub fn reset_handshake_state()                             # NOT needed by this CLI task
L469  pub enum HandshakeAction / L495 pub fn handshake_action    # P4.M2.T1.S2 (NOT needed here)
```

**This task consumes ONLY (via `crate::core::notifier::`):**

```rust
pub fn perform_handshake(verbose: bool);                 // idempotent per boot. QueryInfo -> (SetOs ->
                                                         // QueryCallback sweep) -> populates CALLBACK_NAMES
                                                         // + sets HOST_CAPABLE=true on the capable arm.
                                                         // legacy/timeout/no-device => capable=false, empty map. No panic.
pub fn host_capable() -> bool;                           // HOST_CAPABLE.load(SeqCst)
pub fn callback_names() -> std::collections::HashMap<String, u8>;  // CALLBACK_NAMES.clone()
pub fn is_device_connected() -> bool;                    // cheap pre-check (no HID open)
```

- **`perform_handshake` is self-contained** — it builds its own `DeviceFilter` internally
  (via `configured_filter()`). The caller passes ONLY `verbose`. It maps device errors /
  legacy / timeout to `host_capable()==false` without panicking.
- **`unknown_callback_names` (L423) is PRIVATE** — main.rs CANNOT call it. ⇒ main.rs
  owns its own pure `collect_callback_names` (D6). Verified private (no `pub`).
- **D5b (handshake side-effect):** `perform_handshake` ALSO runs the private
  `validate_rules_callback_names` against the **DEFAULT** `rules.toml` path during the
  sweep, emitting `⚠ unknown callback` warnings **always, regardless of `verbose`**
  (per its docstring: "capability-downgrade and rules-mismatch WARNINGS always print").
  ⇒ when `--validate-rules` calls `perform_handshake(verbose)` to populate the name map,
  the handshake may ALSO print its own `⚠` warnings about the **default** rules.toml. If
  the user passed `--rules-path <other>`, the handshake validates a DIFFERENT file. This
  is **benign supplementary noise** — both outputs are non-fatal warnings. This task's own
  `collect_callback_names` check is the **authoritative** one for the file being validated.
  In the common case (`--rules-path` unset ⇒ same default file) they validate the same
  file (minor duplicate `⚠` lines; acceptable). See D5b/G6.

### §0.2 The host-context send — **P4.M3.T1.S1 (CONTRACT, parallel, in progress)**

P4.M3.T1.S1 (parallel item, still "Implementing") adds `board_has_rules()` + the
stack/replace/no-match send logic to `src/core/notifier.rs`. **It does NOT touch
`src/main.rs`** ⇒ zero file-level merge conflict. This task does NOT call
`board_has_rules()` (the CLI validation path checks schema + names, not the runtime
send decision). `unknown_callback_names` being private (§0.1) further confirms this
task must own its own name-collection helper.

### §0.3 The rules module — **P3.M1 (LANDED, read-only)**

All present in `src/core/rules.rs` TODAY (read in full):

```rust
pub fn parse_rules(path: &Path) -> Result<RuleSet, Box<dyn Error>>;   // fs read + toml::from_str
pub fn get_rules_paths() -> Vec<PathBuf>;                              // config paths, filename -> rules.toml

#[derive(Debug, Deserialize, Default)]
pub struct RuleSet { pub host: HostDefaults, pub layer_rules: Vec<LayerRule>, pub callback_rules: Vec<CallbackRule> }
pub struct LayerRule    { pub pattern: Pattern, pub layer: u8, pub case_sensitive: bool, pub disable_firmware_config: Option<bool> }
pub struct CallbackRule { pub pattern: Pattern, pub enable: Vec<String>, pub disable: Vec<String>, pub case_sensitive: bool, pub disable_firmware_config: Option<bool> }
```

- `parse_rules` Err covers **missing file** (`io::Error`), **malformed TOML**, and
  **missing required `match`/`layer`** (`toml::de::Error`). All coerce to
  `Box<dyn Error>` — exactly the strict failure `--validate-rules` reports. Its strictness
  IS the schema check (pinned by rules.rs tests `test_rules_parse_missing_required_field_errors` etc.).
- `get_rules_paths()` returns candidates in platform preference order; "first existing"
  is the resolver default (mirrors `configured_filter` / `get_config_path`).

---

## §1 — Verbatim CURRENT anchors in `src/main.rs` (re-grepped this session)

### §1.1 `run()` dispatch order (L70-128) — the insertion point

```
L70  fn run() -> Result<(), Box<dyn Error>> {
L72    let args: Vec<String> = env::args().collect();
L73    let verbose = args.iter().any(|a| a == "-v" || a == "--verbose");
L75    if -h/--help  { print_help(); return Ok(()); }            # L75-79
L81    if -c/--config { return create_config(); }                # L81-83
L86    if -r/--reload { ... parse_value_flag --config/--user/--uid; return reload_config(...); }  # L86-95
L97    if -l/--list  { print_platforms(); return Ok(()); }       # L97-101
L103   if --list-devices { crate::core::notifier::list_devices()?; return Ok(()); }   # L103-109
L111   #[cfg(macos|windows)] if --show-window-info { ...; return Ok(()); }            # L111-124
L126   let mut runner = runners::create_runner(verbose)?;         # <<< INSERT NEW FLAGS ABOVE THIS LINE (L125-126)
L127   runner.run(&args)
L128 }
```

**Boolean-flag scan idiom (every branch):** `args.iter().any(|arg| arg == "--flag")`.
**Value-flag idiom:** `parse_value_flag(&args, "--flag")` returns `Option<String>`
(accepts `--flag value` and `--flag=value`). Both are the established patterns this
task MUST follow (item CONTRACT point 1).

### §1.2 `parse_value_flag` (L190-201) — verbatim, reused as-is

```rust
fn parse_value_flag(args: &[String], name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        if a == name {
            return iter.next().cloned();
        }
        if let Some(rest) = a.strip_prefix(&prefix) {
            return Some(rest.to_string());
        }
    }
    None
}
```
Currently **untested** (no `#[cfg(test)] mod tests` in main.rs at all). This task adds the
first unit tests to main.rs.

### §1.3 `create_config()` (L269-279) — the function this task EXTENDS

```rust
fn create_config() -> Result<(), Box<dyn Error>> {
    println!("Creating configuration...");
    let config_dir = platforms::create_config_dir()?;     // L273
    let config_path = config_dir.join("config.toml");
    core::create_default_config(&config_path)?;           // L277
    Ok(())                                                  // <<< EXTEND: also seed rules.toml before this Ok(())
}
```
The extension: after `config.toml`, also seed `rules.toml` via a new
`core::create_default_rules(&config_dir.join("rules.toml"))?`. Same directory, same
no-op-if-exists semantics.

### §1.4 `print_help()` (L130-160) — verbatim CURRENT

```rust
fn print_help() {
    println!("QMKonnect v{}", env!("CARGO_PKG_VERSION"));
    println!("Usage: qmkonnect [OPTIONS]");
    println!("\nOptions:");
    println!("  -h, --help     Display this help message");
    println!("  -v, --verbose  Enable verbose logging");
    println!("  -c, --config   Create a configuration file");
    println!("  -r, --reload   Reload configuration and update system files");
    println!("      --config <path>  Config file to use with --reload");
    println!("      --user <name>    Invoking user for sudo'd --reload (Linux)");
    println!("      --uid <n>        Invoking uid for sudo'd --reload (Linux)");
    println!("  -l, --list     List supported platforms");
    println!("  --list-devices List connected HID devices (VID/PID discovery)");
    // ... Windows Options block (L142-147), then "Running without options..." (L149)
}
```
**NOTE:** `--show-window-info` (which IS dispatched at L111) is currently MISSING from
print_help. This task adds the 3 new flags AND fixes that omission (low risk, improves
help parity with PRD §4).

---

## §2 — The spec sources of truth

### §2.1 PRD §4 CLI Reference (selected_prd_content `h2.59`)

```
      --show-window-info  [macOS/Windows] open the Window Information dialog directly
      --list-callbacks      handshake → print the keyboard's callback name→id table
      --validate-rules      parse rules.toml; report schema/callback-name errors
      --rules-path <path>   override the rules.toml location
```
+ `-c, --config  Create a default (commented-out) configuration file`

### §2.2 HOST_RULES.md §8(6) — the canonical CLI contract (L360-362)

> **(6) CLI:** `--list-callbacks` (handshake → name→id table, or "legacy");
> `--validate-rules [--rules-path <p>]` (parse + schema check; flag unknown callback
> names; non-zero exit on error); `--rules-path`. `-c`/`--config` seeds a commented
> `rules.toml` template.

### §2.3 HOST_RULES.md §9 — `rules.toml` schema (verbatim, L375-411)

This is the **exact template** `render_rules_body()` must emit (commented-out, so a fresh
file parses to all-defaults and host rules stay disabled until the user uncomments). Key
sections the template MUST contain:

```toml
[host]
disable_firmware_config = false   # global default: false = stack, true = replace

[[layer_rules]]
match = "alacritty"
layer = 224
disable_firmware_config = true

[[layer_rules]]
match = ["*chrome*", "*youtube*"]
layer = 225
case_sensitive = false

[[callback_rules]]
match = "neovide"
enable = ["vim_lazy", "disable_vim"]
disable = ["vim_lazy"]

[[callback_rules]]
match = ["*chrome*", "*claude*"]
enable = ["vim_lazy", "disable_vim"]
disable_firmware_config = true
```
**CRITICAL:** the seeded template must be ENTIRELY commented out (`# ` prefix on every
active line) so that: (a) a brand-new install `parse_rules`-es to a valid all-default
`RuleSet` (host rules disabled), and (b) `--validate-rules` on the freshly-seeded file
reports "valid (0 rules)". An uncommented template would immediately activate bogus rules
referencing names the user's keyboard doesn't have.

---

## §3 — Design decisions (D1-D8)

- **D1 — Dispatch order & mutual exclusivity.** Insert the 3 new flag checks in `run()`
  AFTER the `--show-window-info` block (L124) and BEFORE `runners::create_runner` (L126).
  Each branch ends in `return Ok(())` (or `return Err(...)`), matching the existing
  early-return pattern. Order: `--list-callbacks` first, then `--validate-rules` (which
  reads `--rules-path`). `--rules-path` has NO standalone action — it is consumed only
  inside the `--validate-rules` branch via `parse_value_flag`. If `--rules-path` appears
  alone, it is silently ignored (the runner starts) — documented in print_help as "use with
  --validate-rules".

- **D2 — `--list-callbacks` UX.** Cheap pre-check `is_device_connected()` (notifier.rs L171,
  read-only enumerate). If false ⇒ print "No QMK device connected. Connect a keyboard with
  host-rules firmware and re-run." and `return Ok(())` (exit 0). If connected ⇒
  `perform_handshake(true)` (the `true` enables verbose handshake logging), then branch on
  `host_capable()`:
  - `true` ⇒ print a header + the `callback_names()` table sorted by **id** (stable,
    human-readable: `  0  vim_lazy`). Empty map on a capable board ⇒ "Connected keyboard
    reports 0 callbacks."
  - `false` ⇒ print "Legacy firmware (no callback support) — host rules will run in
    string-only mode." (item CONTRACT point 3).

- **D3 — `--validate-rules` path resolution.** Resolve the path in this priority:
  1. `parse_value_flag(&args, "--rules-path")` if present (user override) — if the explicit
     path does NOT exist, that IS an error (exit non-zero: "rules file not found: <path>").
  2. else `rules::get_rules_paths().into_iter().find(|p| p.exists())` — the same "first
     existing candidate" idiom used by `configured_filter`/`get_config_path`. If NONE exist
     ⇒ print "No rules.toml found (host rules disabled). Nothing to validate." and
     `return Ok(())` (exit 0 — absence is not an error; legacy mode is valid).

- **D4 — `--validate-rules` schema check.** Call `rules::parse_rules(&path)`. On
  `Err(e)` ⇒ `eprintln!("rules.toml invalid: {e}")` + `return Err(e)` (propagates to
  `main()` ⇒ `process::exit(1)`). On `Ok(rs)` ⇒ proceed to name validation + print success.
  Reuses parse_rules' built-in strictness (missing `match`/`layer`, malformed TOML all
  surface here).

- **D5 — `--validate-rules` callback-name validation (warnings, NOT fatal).** If
  `is_device_connected()` ⇒ `perform_handshake(verbose)` first (populate the name→id map),
  then collect every `enable`/`disable` name across `rs.callback_rules` and report any NOT
  in `callback_names()` as `⚠ unknown callback: <name>`. If NOT connected ⇒ print "Device
  not connected — callback-name validation skipped (schema-only)." Unknown names do NOT
  change the exit code (exit 0): they are warnings, because (a) `evaluate` tolerates unknown
  names silently (rules.rs `test_evaluate_unknown_name_skipped`), (b) a device may simply
  not be connected. Schema/parse errors are the ONLY fatal condition.

- **D5b — the handshake's own warnings are supplementary.** `perform_handshake` internally
  runs `validate_rules_callback_names` against the **default** rules.toml during the sweep
  (§0.1), printing its own `⚠` warnings always. So when `--validate-rules` calls
  `perform_handshake(verbose)`, two sets of `⚠` lines may appear: the handshake's (for the
  default file) and this tool's authoritative ones (for the resolved file). This is BENIGN:
  both are non-fatal warnings; in the common case (`--rules-path` unset) they target the
  same file. Do NOT try to suppress the handshake's output (no flag exists); just let both
  print. This tool's `⚠` lines are the authoritative result for the file under validation.

- **D6 — Callback-name collection is a pure, tested helper in main.rs.** Add
  `fn collect_callback_names(rules: &rules::RuleSet) -> std::collections::BTreeSet<String>`
  (collects+dedupes every `enable`/`disable` name across `callback_rules`). The unknown-set
  is `collect_callback_names(&rs).into_iter().filter(|n| !known.contains_key(n))`. This is
  REQUIRED because the handshake's `unknown_callback_names` (notifier.rs L423) is **private**
  (verified — no `pub`). BTreeSet ⇒ deterministic, sorted output. ~6 lines, self-contained.

- **D7 — `-c`/`--config` rules.toml seeding lives in core/mod.rs.** Add
  `pub fn render_rules_body() -> String` + `pub fn create_default_rules(path: &Path) ->
  Result<(), Box<dyn Error>>` to `src/core/mod.rs`, **mirroring** the existing
  `render_config_body` (L96) + `create_default_config` (L134) pair EXACTLY (same
  no-op-if-exists message, same `fs::create_dir_all` on parent, same `println!` confirmation).
  `create_config()` in main.rs gains ONE line: `core::create_default_rules(&config_dir.join("rules.toml"))?;`.
  Rationale: the template renderer is reusable (P5.M2 tray "Reload rules" may re-seed; P6
  docs reference it) and keeps main.rs thin — exactly the precedent `render_config_body` set.

- **D8 — `print_help()` parity with PRD §4.** Add the 3 new flags AND the currently-missing
  `--show-window-info` line, grouped after `--list-devices`. Wording follows PRD §4
  (`h2.59`) and HOST_RULES.md §8(6) verbatim where possible.

---

## §4 — Gotchas (G1-G9)

- **G1 (build precondition):** `qmk_notifier` v0.3.0 must resolve (Cargo.toml:19
  `tag = "v0.3.0"`, P4.M1.T2.S1 Complete; the handshake P4.M2 already builds against it, so
  if the handshake compiles in the current tree, v0.3.0 resolves). A fetch failure is an
  env/network issue, not a code bug in this task.

- **G2 (handshake LANDED — consume, don't reimplement):** `perform_handshake`/`host_capable`/
  `callback_names`/`is_device_connected` are REAL present code (notifier.rs L171/265/440/447).
  Call via `crate::core::notifier::`. Do NOT reimplement them. (`unknown_callback_names`
  L423 is PRIVATE — hence D6's own `collect_callback_names`.)

- **G3 (single source of truth for validation):** do NOT re-parse `rules.toml` by hand —
  always go through `rules::parse_rules`. Its strictness (missing required fields, malformed
  TOML) IS the schema check; the existing rules.rs tests pin it.

- **G4 (`--rules-path` alone is a no-op).** It is parsed ONLY inside the `--validate-rules`
  branch. Do not add a standalone `--rules-path` action. (If the user runs
  `qmkonnect --rules-path x.toml` alone, the app starts the runner — acceptable; print_help
  says "use with --validate-rules".)

- **G5 (explicit-path-missing vs no-path-found differ).** `--rules-path foo` where `foo`
  doesn't exist ⇒ **error, exit non-zero** (user asked for a specific file). No `--rules-path`
  and no candidate exists ⇒ **info, exit 0** (host rules simply disabled). Conflating these
  breaks the tool's contract.

- **G6 (unknown names are warnings, exit 0).** Do NOT make unknown callback names fatal.
  `evaluate` skips them (rules.rs `test_evaluate_unknown_name_skipped`), and a device may be
  disconnected. Only parse/schema errors exit non-zero (D5). NOTE (D5b): `perform_handshake`
  ALSO prints its own `⚠` warnings for the default rules.toml during the sweep — benign
  supplementary noise; do NOT try to suppress it.

- **G7 (template MUST be fully commented).** `render_rules_body()` prefixes EVERY active
  line with `# `. An uncommented template would activate bogus rules referencing example names
  ("vim_lazy") the user's keyboard lacks, and would make a brand-new install behave
  non-legacy. The seeded file must `parse_rules` to a valid all-default `RuleSet`.

- **G8 (binary crate, no lib doctests).** Mode-A rustdoc uses ` ```rust,ignore ` fences
  (main.rs + core/mod.rs are bin-crate code; `use qmkonnect::...` won't resolve in doctests).
  Match rules.rs/pattern.rs/notifier.rs.

- **G9 (no `#[cfg(test)] mod tests` exists in main.rs today).** This task adds the FIRST
  test module to main.rs. Keep it pure-logic only (`parse_value_flag`, `collect_callback_names`)
  — do NOT attempt to unit-test `run()` (it calls `env::args()` and dispatches to IO/device;
  that's integration territory, covered by the manual `cargo run -- --flag` validation in
  Level 3).

---

## §5 — Test plan

### §5.1 New `#[cfg(test)] mod tests` in `src/main.rs` (first tests in this file)

1. `test_parse_value_flag_space_form` — `["--rules-path", "x.toml"]` ⇒ `Some("x.toml")`.
2. `test_parse_value_flag_equals_form` — `["--rules-path=x.toml"]` ⇒ `Some("x.toml")`.
3. `test_parse_value_flag_absent` — `[]` ⇒ `None`.
4. `test_collect_callback_names_dedupes` — a RuleSet with overlapping enable/disable names
   ⇒ BTreeSet of the union, sorted, deduped.
5. `test_collect_callback_names_empty_when_no_rules` — default RuleSet ⇒ empty set.

(All pure — no IO, no device, no global state ⇒ thread-safe; still run with
`--test-threads=1` per AGENTS.md since OTHER bin tests share globals.)

### §5.2 New tests in `src/core/mod.rs` `#[cfg(test)] mod tests`

6. `test_render_rules_body_fully_commented` — every non-blank line starts with `#` (G7);
   body contains the §9 section markers (`[host]`, `[[layer_rules]]`, `[[callback_rules]]`,
   `disable_firmware_config`).
7. `test_render_rules_body_parses_to_default_ruleset` — `toml::from_str::<rules::RuleSet>`
   of the rendered body ⇒ Ok, all-default (0 layer rules, 0 callback rules) — proves the
   seeded file is valid+inert.
8. `test_create_default_rules_noop_if_exists` — pre-create the file; call
   `create_default_rules` ⇒ Ok, content UNCHANGED (no-op, mirrors create_default_config).
9. `test_create_default_rules_writes_when_absent` — absent file ⇒ Ok, file now exists with
   the rendered body; re-call ⇒ no-op (idempotent).

---

## §6 — Validation (project dev loop, AGENTS.md)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect                                  # clean (G1 v0.3.0; handshake LANDED)
cargo test --bin qmkonnect -- --test-threads=1               # MANDATORY single-threaded (AGENTS.md)
cargo clippy --bin qmkonnect --no-deps 2>/dev/null || true   # no NEW warnings
git diff --stat                                              # expect src/main.rs + src/core/mod.rs ONLY
```

Manual smoke (Level 3): `cargo run -- --help` (new flags listed);
`cargo run -- --validate-rules` (no rules.toml ⇒ exit 0 info); `cargo run -- --validate-rules
--rules-path /tmp/bad.toml` (malformed ⇒ exit 1); `cargo run -- -c` then inspect the seeded
`rules.toml` (fully commented, parses). `--list-callbacks` needs real hardware ⇒ manual only.
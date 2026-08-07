# PRP — P2.M1.T2.S1: Add `LinuxConfig` struct + `[linux]` table to Config schema

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). **File edited:** `src/core/mod.rs`
> (the `Config` struct + manual `Default` impl + `render_config_body` +
> `render_default_config_template` + the inline `#[cfg(test)]` tests), plus a ~3-line
> close-out of the staged `// TODO(P2.M1.T2.S1)` seam in `src/platforms/mod.rs::create_monitor`
> (left by the parallel item P2.M1.T1.S1). **No Cargo.toml.** No new backend modules
> (wayland/gnome/atspi are P2.M2/P2.M3/P2.M4). No `docs/*` prose (Mode A: the schema is
> self-documenting via the `[linux]` table + commented render hints).
>
> **What this does:** adds a `LinuxConfig` struct (`backend: Option<String>`,
> `gnome_poll_interval_ms: Option<u64>`) and wires it into `Config` as
> `#[serde(default)] pub linux: LinuxConfig`, so a user can override the Linux
> window-monitor backend (`config.toml` → `[linux] backend = "x11"`) without a recompile.
> The save renderer (`render_config_body`) + seeder (`render_default_config_template`)
> are extended to round-trip/seed the `[linux]` table, and the staged
> `create_monitor` TODO is closed so the `backend` override actually reaches
> `select_linux_backend`. This is the **config half** of F16 (cross-DE Linux monitor,
> PRD §4 F16); the dispatcher half is P2.M1.T1.S1.
>
> **Source of truth:** `CONFIG.md` §1/§1.3 (PRD snapshot lines 2937–3015: exact schema +
> field table), `PLATFORMS.md` §6 (the consumer `select_linux_backend`), and the production
> code itself (`src/core/mod.rs` — where the PRD's idealized `#[derive(Default)]` and the
> code's manual `Default` impl disagree, **the code wins**; see GOTCHA-1). `research/notes.md`
> holds the verified current-state findings + the locked design decisions.

---

## Goal

**Feature Goal**: Add a `[linux]` TOML table to `config.toml` with two optional fields
(`backend`, `gnome_poll_interval_ms`) that deserialize into a new `LinuxConfig` struct on
`Config`, with zero-config parity (a config with no `[linux]` table behaves identically to
today), round-trip-safe rendering in the Settings save path, and a closed `create_monitor`
seam so `backend` reaches `select_linux_backend`'s `forced` parameter.

**Deliverable** (concrete code; compiles + passes tests on the dev box TODAY with `default` features):
- `src/core/mod.rs`:
  - `pub struct LinuxConfig { backend: Option<String>, gnome_poll_interval_ms: Option<u64> }` with `#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default)]` and `#[serde(default)]` on both fields.
  - `Config` gains `#[serde(default)] pub linux: LinuxConfig`.
  - The MANUAL `impl Default for Config` gains `linux: LinuxConfig::default()` (do NOT switch to `#[derive(Default)]` — GOTCHA-1).
  - `render_config_body` appends a `[linux]` block reflecting actual values (active when `Some`, commented hint when `None`).
  - `render_default_config_template` appends a fully-commented `[linux]` hint.
  - Inline tests: update the breaking full-literal test (line ~620); add `[linux]` parse + round-trip tests; add `linux` assertions to the existing defaults tests.
- `src/platforms/mod.rs`: replace the staged `// TODO(P2.M1.T2.S1)` in `create_monitor`'s Linux arm with the actual `cached_config().linux.backend` → `forced` wiring (normalize `Some("auto")` → `None`).

**Success Definition**:
- `cargo build --bin qmkonnect` compiles clean (no new warnings).
- `cargo test --bin qmkonnect -- --test-threads=1` passes (existing tests + new `[linux]` parse/round-trip tests).
- A config with `[linux] backend = "x11"` parses to `cfg.linux.backend == Some("x11".into())`; a config WITHOUT a `[linux]` table parses to `cfg.linux == LinuxConfig::default()`.
- `render_config_body` round-trips the `[linux]` table (a save after a manual `[linux]` edit preserves it); `render_default_config_template` parses to all-default (inert).
- `git diff --stat` shows ONLY `src/core/mod.rs` and `src/platforms/mod.rs` (no Cargo.toml, no docs/*, no PRD/tasks.json).
- `configured_filter()` (notifier.rs) and `configured_timing()` (mod.rs) are UNAFFECTED (they only read existing fields).

## User Persona (if applicable)

**Target User**: a Linux user who needs to force a specific window-monitor backend for
debugging or an unusual setup (e.g. pin `x11` on a multi-monitor XWayland edge case, or
`hyprland` to bypass a flaky foreign-toplevel probe). In normal use the `[linux]` table is
absent and auto-selection (foreign-toplevel → GNOME → Hyprland → AT-SPI → X11) is correct
on every supported desktop.

**Use Case**: user edits `config.toml`, adds `[linux] backend = "x11"`, restarts QMKonnect
(or relies on the live re-read at the next `create_monitor` call). The verbose log prints
`select_linux_backend: forced backend 'x11'` (per P2.M1.T1.S1) and the app uses X11.

**Pain Points Addressed**: makes the F16 cross-DE backend selection *configurable* without a
recompile (today backend choice is feature-flag-only); gives debuggers a single-line knob
(`backend = "…"`) that errors loudly with every probe result when the forced backend is
unavailable (`PLATFORMS.md` §6).

## Why

- **F16 (PRD §4) requires a configurable cross-DE backend.** `CONFIG.md` §1.3 + `PLATFORMS.md`
  §6 mandate an optional `[linux] backend` override on top of the runtime auto-selector. This
  task IS the config schema for that override; the selector itself is P2.M1.T1.S1 (parallel).
- **Round-trip safety is load-bearing.** `render_config_body` exists specifically to stop a
  Settings-dialog VID/PID save from clobbering the user's other fields. Adding a field to
  `Config` without teaching the renderer to serialize it would reintroduce exactly that bug
  for the `[linux]` table — silently stripping a manual override on the next save.
- **Closes the staged seam.** The parallel item (P2.M1.T1.S1) ships `create_monitor` with a
  `// TODO(P2.M1.T2.S1): wire [linux] backend` seam and `select_linux_backend(verbose, None)`.
  Without THIS task, the seam is orphaned and `[linux] backend` parses but is dead.

## What

[User-visible behavior + technical requirements]

1. **`LinuxConfig` struct** — two `Option` fields, `#[serde(default)]` on both, derives
   `serde::{Deserialize,Serialize} + Clone + Debug + Default` (Clone required because
   `Config` derives `Clone`; Default safe because both fields are `Option`).
2. **`Config.linux`** — `#[serde(default)] pub linux: LinuxConfig`, added at the end of the
   struct. The MANUAL `impl Default for Config` gains `linux: LinuxConfig::default()` (do NOT
   derive `Default` on Config — GOTCHA-1).
3. **`render_config_body`** — appends a `[linux]` block: `backend` rendered active when
   `Some`, commented hint when `None`; `gnome_poll_interval_ms` likewise. Preserves round-trip.
4. **`render_default_config_template`** — appends a fully-commented `[linux]` hint (parses to
   default; inert on a fresh `-c` seed).
5. **`create_monitor` (platforms/mod.rs)** — the staged TODO is replaced with the wiring:
   `cached_config().ok().and_then(|c| c.linux.backend)` → normalize `Some("auto")` → `None` →
   pass as `forced` to `select_linux_backend`.
6. **Tests** — update the breaking full-literal test; add `[linux]` parse + round-trip tests;
   add `linux` assertions to the existing defaults tests.

### Success Criteria

- [ ] `pub struct LinuxConfig { backend: Option<String>, gnome_poll_interval_ms: Option<u64> }` exists with `#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default)]` + `#[serde(default)]` on both fields.
- [ ] `Config` has `#[serde(default)] pub linux: LinuxConfig`.
- [ ] The MANUAL `impl Default for Config` includes `linux: LinuxConfig::default()` (Config is NOT `#[derive(Default)]` — debounce_ms=50 invariant preserved; GOTCHA-1).
- [ ] `LinuxConfig::default() == LinuxConfig { backend: None, gnome_poll_interval_ms: None }`.
- [ ] A config with `[linux] backend = "x11"\ngnome_poll_interval_ms = 2000` parses to `Some("x11")` / `Some(2000)`.
- [ ] A config WITHOUT a `[linux]` table parses to `cfg.linux == LinuxConfig::default()` (zero-config parity — existing `empty_config_is_all_none` still passes).
- [ ] `render_config_body` round-trips the `[linux]` table (assert `backend` survives a write+re-parse in the updated `render_config_body_preserves_non_vidpid_fields` test).
- [ ] `render_default_config_template` parses to all-default including `linux: LinuxConfig::default()` (fully commented `[linux]` hint).
- [ ] `create_monitor`'s Linux arm reads `cached_config().linux.backend` (no TODO remains) and normalizes `Some("auto")` → `None`.
- [ ] `configured_filter()` / `configured_timing()` unchanged (no edits to their bodies).
- [ ] `cargo build --bin qmkonnect` clean; `cargo test --bin qmkonnect -- --test-threads=1` green.
- [ ] `git diff --stat` shows ONLY `src/core/mod.rs` + `src/platforms/mod.rs`.

## All Needed Context

### Context Completeness Check
_Pass._ An agent with no prior knowledge of this codebase can implement this from: the exact
PRD schema (CONFIG.md §1/§1.3, quoted in References); the verbatim current `Config` struct +
manual `Default` impl + `render_config_body` + the two render-round-trip tests (all in
`src/core/mod.rs`, quoted); the locked design decisions (keep manual Default, render the table
for round-trip, close the create_monitor TODO); the precise gotchas (GOTCHA-1 manual Default,
GOTCHA-2 Clone, GOTCHA-3 round-trip, GOTCHA-4 the line-620 full literal, GOTCHA-5 auto normalization);
and grep-gateable invariants validated with no extra crates.

### Documentation & References

```yaml
# MUST READ — the exact schema being implemented (PRD contract)
- docfile: plan/007_fb356ba503b4/prd_snapshot.md
  why: "CONFIG.md §1 (snapshot lines 2937-2963): the Config + LinuxConfig struct definitions
        with field types + serde attrs. §1.3 (lines 2993-3015): the [linux] table TOML example +
        the field table (backend default 'auto', gnome_poll_interval_ms default 1000, both
        Option, backend is 'diagnostic in normal use'). NOTE the PRD shows #[derive(Default)] on
        Config — the CODE uses a manual impl (GOTCHA-1); code wins."
  section: "## 1. Config Schema + ### 1.3 Linux monitor backend ([linux])"

# MUST READ — the consumer contract (the parallel item's select_linux_backend signature)
- docfile: plan/007_fb356ba503b4/prd_snapshot.md
  why: "PLATFORMS.md §6 (snapshot line 2034): '[linux] backend = \"foreign-toplevel\" | \"gnome\"
        | \"hyprland\" | \"atspi\" | \"x11\" | \"auto\" (default auto) in config.toml. A forced
        backend that is unavailable errors loudly with every probe result; auto is the normal path.'
        Line 2171: '[linux] gnome_poll_interval_ms' is the GNOME backend drift-poll cadence.
        This fixes the EXACT valid value set for `backend` + confirms gnome_poll_interval_ms is
        the GNOME-backend knob."
  section: "PLATFORMS.md §6 (backend override) + §8 (gnome_poll_interval_ms)"

# MUST READ — the file THIS task edits (verbatim current Config + Default + renderers + tests)
- file: src/core/mod.rs
  why: "(1) Config struct (lines ~26-47): #[derive(serde::Deserialize, serde::Serialize, Clone)]
        — NO derive Default; 6 fields. (2) impl Default for Config (lines ~53-72): MANUAL impl
        with a load-bearing doc comment — debounce_ms MUST stay 50 (GOTCHA-1). (3) parse_config
        (line ~190), cached_config/cached_config_at (lines ~150-185), configured_timing (line ~194).
        (4) render_default_config_template (line ~204): the -c seeder, fully commented. (5)
        render_config_body (line ~237): the SAVE renderer with #[cfg_attr(not(any(macos,windows)),
        allow(dead_code))]; serializes the FULL config for round-trip. (6) Tests: line ~579
        render_config_body_round_trips (uses ..Config::default() — OK), line ~620
        render_config_body_preserves_non_vidpid_fields (FULL 6-field literal — MUST UPDATE,
        GOTCHA-4), line ~560 empty_config_is_all_none, line ~571 legacy_config...,
        line ~587 render_default_config_template_round_trips_to_defaults."
  pattern: "Option fields render active-when-Some / commented-when-None (see vendor_id in
            render_config_body); timing fields always render their value. Mirror for the [linux] block."
  gotcha: "KEEP the manual impl Default (GOTCHA-1). LinuxConfig MUST derive Clone (Config derives
           Clone; GOTCHA-2). The line-620 test full-literal breaks on the new field (GOTCHA-4)."

# MUST READ — the staged TODO to close (the parallel item's create_monitor seam)
- file: src/platforms/mod.rs
  why: "create_monitor's Linux arm (lines ~46-58) ships (per P2.M1.T1.S1) as:
        #[cfg(target_os = \"linux\")]
        {
            // TODO(P2.M1.T2.S1): wire `[linux] backend` from core::cached_config() into the `forced` arg.
            return linux::select_linux_backend(verbose, None);
        }
        THIS task replaces the TODO: read core::cached_config().ok().and_then(|c| c.linux.backend),
        normalize Some(\"auto\") -> None, pass as forced. select_linux_backend(verbose, forced:
        Option<&str>) already takes the override as a parameter (P2.M1.T1.S1 contract)."
  pattern: "forced = backend.as_deref().filter(|b| !b.eq_ignore_ascii_case(\"auto\")); then
            linux::select_linux_backend(verbose, forced)."
  gotcha: "treat None AND Some(\"auto\") the same (both = auto-selection, forced=None). Do NOT
           pass Some(\"auto\") through as a forced name — select_linux_backend's forced path errors
           if the name isn't a compiled-in backend candidate (\"auto\" is not a candidate name)."

# MUST READ — confirmed filter/timing unaffected (the functions to NOT touch)
- file: src/core/notifier.rs
  why: "configured_filter() lives HERE (not mod.rs). It reads cfg.vendor_id/product_id/usage_page/
        usage ONLY. Adding a `linux` field to Config is transparent to it (it never references
        cfg.linux). Confirms the item-spec constraint 'configured_filter() unaffected'."
  pattern: "configured_filter() destructures only the device-identifying Option fields."
  gotcha: "do NOT add any cfg.linux read to notifier.rs — gnome_poll_interval_ms is consumed by
           the GNOME backend (P2.M3.T2, not written yet); backend is consumed by create_monitor
           (this task). No notifier.rs change."

# REFERENCE — the parallel item's PRP (the dispatcher contract + the staged seam)
- file: plan/007_fb356ba503b4/P2M1T1S1/PRP.md
  why: "defines select_linux_backend(verbose, forced: Option<&str>) and the create_monitor seam
        this task closes. Confirms 'forced' is Option<&str> (so backend: Option<String> maps via
        .as_deref()), and that a forced-unavailable backend errors LOUDLY with every probe result
        (so 'auto' must NEVER reach the forced path). Treat as contract: the dispatcher exists
        exactly as specified when this task runs."
  section: "## Implementation Blueprint (Task 7: create_monitor delegates) + the forced-override semantics"

# REFERENCE — the existing PRP house style
- file: plan/007_fb356ba503b4/P2M1T1S1/PRP.md
  why: "the established PRP format for this plan: verbatim Implementation Patterns, per-gotcha
        'CRITICAL' callouts, grep-gateable Level-3 validation, an explicit Anti-Patterns list."
  pattern: "Implementation Patterns block with copy-ready Rust; Validation Loop with concrete grep/cargo commands."

# REFERENCE — design decisions + verified current state (this task's own research)
- docfile: plan/007_fb356ba503b4/P2M1T2S1/research/notes.md
  why: "the locked design (manual Default kept; render_config_body extended for round-trip;
        create_monitor TODO closed; auto-normalization) + the verified current-state quotes
        (Config struct, manual Default, render_config_body, the line-620 breaking test, the
        create_monitor seam) + the 8 gotchas."
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
src/
  core/
    mod.rs            # EDIT: +LinuxConfig struct; Config +linux field; manual Default +linux;
                        #       render_config_body +[linux] block; render_default_config_template +[linux] hint;
                        #       update line-620 test; +new [linux] tests; +linux assertions
    notifier.rs       # UNTOUCHED (configured_filter reads only device-identifying fields)
    rules.rs / types.rs / pattern.rs   # untouched
  platforms/
    mod.rs            # EDIT: create_monitor Linux arm — close the staged TODO(P2.M1.T2.S1)
  runners/            # untouched
  tray.rs / linux_tray.rs / main.rs / autostart.rs   # untouched
Cargo.toml            # UNTOUCHED (no new features/deps)
```

### Desired Codebase tree with files added/changed

```bash
src/core/mod.rs       # +pub struct LinuxConfig; Config +linux; impl Default +linux; render_* +[linux]; tests updated/added
src/platforms/mod.rs  # create_monitor Linux arm: TODO → cached_config().linux.backend → forced (auto-normalized)
# (no new files; no Cargo.toml; no docs/*)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (GOTCHA-1 — manual Default, NOT derived): Config keeps its MANUAL `impl Default`.
//   The PRD's idealized schema shows #[derive(... Default)] on Config, but the production code
//   deliberately uses a manual impl with a load-bearing doc comment: #[derive(Default)] would
//   ZERO-INIT debounce_ms: u64 to 0 (not 50), silently DISABLING DEBOUNCING and breaking the
//   invariant that Config::default() / an empty config.toml / configured_timing() all describe
//   the same zero-config state. CODE WINS: keep the manual impl, add `linux: LinuxConfig::default()`
//   to it, and add a code comment noting the PRD drift. LinuxConfig itself CAN safely
//   #[derive(Default)] (both fields Option → None = auto/1000-at-use-site); honor the item-spec ask.
//
// CRITICAL (GOTCHA-2 — Clone is required on LinuxConfig): Config derives `Clone`. Adding
//   `pub linux: LinuxConfig` means LinuxConfig MUST also derive `Clone` or Config's derive fails
//   to compile. So LinuxConfig derives at minimum serde::{Deserialize,Serialize} + Clone + Default;
//   add `Debug` too (cheap; useful for the select_linux_backend verbose path + {:?} diagnostics).
//
// CRITICAL (GOTCHA-3 — render_config_body MUST round-trip the [linux] table): render_config_body
//   is the SAVE renderer used by every Settings-dialog write path; it exists specifically so a
//   VID/PID edit doesn't clobber the user's other fields. If you add `linux` to Config WITHOUT
//   teaching render_config_body to serialize it, a manual `[linux] backend = "x11"` is STRIPPED
//   on the next Settings save (reintroducing the exact bug the fn was created to fix). Render the
//   ACTUAL values: active line when Some, commented hint when None (mirror the vendor_id pattern).
//
// CRITICAL (GOTCHA-4 — the line-~620 full struct literal breaks): the test
//   `render_config_body_preserves_non_vidpid_fields` constructs Config with a FULL 6-field literal
//   (no `..Config::default()`). Adding `linux` makes it fail to compile. UPDATE it: add `linux`
//   (ideally set backend: Some("x11".into()), gnome_poll_interval_ms: Some(2000)) AND assert they
//   round-trip. The line-~579 test uses `..Config::default()` so it's fine (struct-update fills linux).
//
// CRITICAL (GOTCHA-5 — None vs Some("auto") normalization at the use site): in create_monitor,
//   BOTH `backend == None` and `backend == Some("auto")` mean "auto-selection". Pass `forced = None`
//   in BOTH cases. Do NOT forward Some("auto") as a forced name — select_linux_backend's forced
//   path treats the name as a compiled-in backend candidate, and "auto" is NOT a candidate name
//   (it would hit the 'forced backend not compiled into this binary' loud-Err path). Filter:
//   `backend.as_deref().filter(|b| !b.eq_ignore_ascii_case("auto"))`.
//
// GOTCHA (GOTCHA-6 — single-threaded tests): the crate shares global debouncer state. Run ALL
//   tests with `cargo test --bin qmkonnect -- --test-threads=1` (AGENTS.md). The new config tests
//   are hermetic (pure toml::from_str round-trips) but run under the same single-threaded harness.
//
// GOTCHA (GOTCHA-7 — don't change render_config_body's cfg_attr): render_config_body carries
//   `#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]` because
//   today it's only CALLED by the macos/win Settings dialogs (Linux uses zenity). Adding the
//   [linux] block doesn't change who CALLS it — leave the cfg_attr as-is. (parse_config on Linux
//   reads the [linux] table regardless; render_config_body not being called on Linux is fine.)
//
// GOTCHA (configured_filter / configured_timing truly unaffected): configured_filter() is in
//   notifier.rs and reads only vendor_id/product_id/usage_page/usage. configured_timing() reads
//   only debounce_ms/poll_interval_ms. Both get a whole Config from cached_config() and ignore the
//   new field — adding `linux` is transparent. Do NOT edit either fn.
```

## Implementation Blueprint

### Data models and structure

```rust
// src/core/mod.rs — LinuxConfig (NEW) + Config field (NEW)

/// Optional Linux window-monitor backend overrides (`[linux]` table in `config.toml`).
///
/// Both fields are `Option` + `#[serde(default)]`, so a config with NO `[linux]` table
/// (or `[linux] backend = "auto"`) deserializes to all-`None` ⇒ runtime auto-selection in
/// `select_linux_backend` (foreign-toplevel → GNOME → Hyprland → AT-SPI → X11).
///
/// `backend` is diagnostic in normal use — auto-selection is correct on every supported desktop
/// (PLATFORMS.md §6, CONFIG.md §1.3). `gnome_poll_interval_ms` is the GNOME backend's
/// drift-correcting poll cadence (consumed by the GNOME backend, P2.M3.T2).
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default)]
pub struct LinuxConfig {
    /// Force a specific backend. One of: `foreign-toplevel | gnome | hyprland | atspi | x11 |
    /// auto`. `None` (or `"auto"`) ⇒ runtime priority order. A forced backend that is unavailable
    /// errors loudly with every probe result. (`select_linux_backend` takes this as `forced`.)
    #[serde(default)]
    pub backend: Option<String>,
    /// GNOME backend drift-correcting poll cadence (ms). `None` ⇒ 1000 (resolved at the GNOME
    /// backend use site). Hot-re-read each tick (like `poll_interval_ms`). Ignored by every
    /// non-GNOME backend.
    #[serde(default)]
    pub gnome_poll_interval_ms: Option<u64>,
}

// Config gains the field (KEEP the existing derives + the MANUAL Default — do NOT derive Default):
#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub struct Config {
    // ... existing 6 fields unchanged ...
    /// Linux window-monitor backend overrides (`[linux]` table). Absent ⇒ auto-selection.
    /// `#[serde(default)]` ⇒ a config without a `[linux]` table deserializes to
    /// `LinuxConfig::default()` (both `None`).
    #[serde(default)]
    pub linux: LinuxConfig,
}
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: ADD the LinuxConfig struct to src/core/mod.rs
  - PLACE: immediately ABOVE the `#[derive(...)] pub struct Config` definition (logical grouping;
           the Config field's doc comment references it).
  - DERIVE: #[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default)]  (Clone required —
           GOTCHA-2; Default safe — both fields Option; Debug for verbose logging).
  - FIELDS: `#[serde(default)] pub backend: Option<String>` + `#[serde(default)]
            pub gnome_poll_interval_ms: Option<u64>` (both Option + serde default).
  - DOC: a `///` block mapping both fields to CONFIG.md §1.3 (backend = the forced-override knob
         consumed by select_linux_backend; gnome_poll_interval_ms = the GNOME drift-poll cadence
         consumed by the GNOME backend P2.M3.T2); note both-None = auto-selection.
  - NAMING: `LinuxConfig` (matches PRD); fields `backend` / `gnome_poll_interval_ms` (exact PRD names).

Task 2: ADD the `linux` field to Config + UPDATE the manual Default impl
  - ADD at the END of `pub struct Config`: `#[serde(default)] pub linux: LinuxConfig,` with a `///`
    doc noting absent-table ⇒ LinuxConfig::default() ⇒ auto-selection.
  - DO NOT add `Default` to Config's derive list (GOTCHA-1 — would zero-init debounce_ms to 0).
  - UPDATE the MANUAL `impl Default for Config`: add `linux: LinuxConfig::default(),` to the Self {…}
    literal. Update its doc comment to mention `linux: LinuxConfig::default()` is part of the
    zero-config state, and ADD a one-line note that the PRD's `#[derive(Default)]` is intentionally
    NOT used (code-wins drift note — GOTCHA-1).
  - PRESERVE: every other Config field + serde attr + the debounce_ms=50 invariant.

Task 3: EXTEND render_config_body to round-trip the [linux] table
  - APPEND to the `format!(…)` body in render_config_body (after the poll_interval_ms block) a
    `[linux]` section rendered from `config.linux`:
      * A header comment block explaining the [linux] table (optional, Linux-only, auto by default).
      * `backend`: active `backend = "<v>"` when Some; commented `# backend = "auto"` when None.
      * `gnome_poll_interval_ms`: active `<v>` when Some; commented `# gnome_poll_interval_ms = 1000`
        when None.
  - FOLLOW the EXACT existing pattern for Option fields (vendor_id/usage_page render
    active-when-Some / commented-hint-when-None). Compute the two lines via `match` BEFORE the
    format!() (mirror `vid_line`/`usage_page_line`).
  - GOTCHA: keep the existing #[cfg_attr(not(any(target_os="macos", target_os="windows")),
    allow(dead_code))] on the fn (GOTCHA-7 — don't touch it).
  - PRESERVE: every existing rendered field (vid/pid/usage_page/usage/debounce/poll).

Task 4: EXTEND render_default_config_template with a commented [linux] hint
  - APPEND a fully-COMMENTED `[linux]` block to the template string (every line `#`-prefixed, like
    the existing fields), so a freshly-seeded `-c` file parses to all-default (inert) — mirrors the
    existing fully-commented style.
    Example tail:
      #\n\
      # Linux window-monitor backend ([linux]). Both fields OPTIONAL; omit the whole\n\
      # table (or backend = "auto") for runtime auto-selection. (Linux only.)\n\
      # [linux]\n\
      # backend = "auto"                 # auto | foreign-toplevel | gnome | hyprland | atspi | x11\n\
      # gnome_poll_interval_ms = 1000    # GNOME backend drift-poll cadence (ms)\n
  - GOTCHA: every active line must be commented so the seeded file parses to LinuxConfig::default()
    (the existing test `render_default_config_template_round_trips_to_defaults` asserts this).

Task 5: CLOSE the create_monitor TODO in src/platforms/mod.rs
  - FIND: the `#[cfg(target_os = "linux")]` block in create_monitor (ships per P2.M1.T1.S1 with
    `// TODO(P2.M1.T2.S1): wire [linux] backend from core::cached_config() into the forced arg.`
    followed by `return linux::select_linux_backend(verbose, None);`).
  - REPLACE the TODO + None with:
        let forced = core::cached_config()
            .ok()
            .and_then(|c| c.linux.backend)
            .and_then(|b| match b.to_ascii_lowercase().as_str() {
                "auto" | "" => None,        // auto / empty = runtime priority order
                _ => Some(b),               // force the named backend
            });
        if verbose {
            if let Some(ref b) = forced { println!("config [linux] backend = {b:?} (forced)"); }
        }
        return linux::select_linux_backend(verbose, forced.as_deref());
  - GOTCHA-5: normalize Some("auto") (and empty) to None — do NOT forward "auto" as a forced name
    (select_linux_backend would loud-Err: "auto" is not a compiled-in candidate).
  - PRESERVE: the macOS/Windows arms + every other line of create_monitor.

Task 6: UPDATE the breaking test (line ~620) + ADD linux assertions to existing defaults tests
  - UPDATE `render_config_body_preserves_non_vidpid_fields` (full 6-field literal — BREAKS on the
    new field): add `linux: LinuxConfig { backend: Some("x11".into()),
    gnome_poll_interval_ms: Some(2000) }` to the literal, and ADD assertions that the round-tripped
    parsed Config preserves BOTH linux fields:
        assert_eq!(parsed.linux.backend, original.linux.backend);
        assert_eq!(parsed.linux.gnome_poll_interval_ms, original.linux.gnome_poll_interval_ms);
  - UPDATE `empty_config_is_all_none`: ADD `assert_eq!(cfg.linux, LinuxConfig::default());`
    (proves zero-config parity — a config with no [linux] table is the auto default).
  - UPDATE `render_default_config_template_round_trips_to_defaults`: ADD the same linux assertion.
  - UPDATE `render_config_body_round_trips` (uses ..Config::default() — still compiles): ADD
    `assert_eq!(cfg.linux, LinuxConfig::default());` after both parses (default-rendered body must
    parse to default linux).

Task 7: ADD new [linux]-specific tests to the existing #[cfg(test)] mod tests
  - `linux_config_parses_from_table`:
        let cfg: Config = toml::from_str("[linux]\nbackend = \"x11\"\ngnome_poll_interval_ms = 2000\n").unwrap();
        assert_eq!(cfg.linux.backend.as_deref(), Some("x11"));
        assert_eq!(cfg.linux.gnome_poll_interval_ms, Some(2000));
        // other fields stay None/default
        assert_eq!(cfg.vendor_id, None);
  - `linux_config_absent_table_is_default`:
        let cfg: Config = toml::from_str("debounce_ms = 100\n").unwrap();
        assert_eq!(cfg.linux, LinuxConfig::default());   // both None
  - `linux_config_partial_table_only_backend`:
        let cfg: Config = toml::from_str("[linux]\nbackend = \"hyprland\"\n").unwrap();
        assert_eq!(cfg.linux.backend.as_deref(), Some("hyprland"));
        assert_eq!(cfg.linux.gnome_poll_interval_ms, None);   // unset field stays None
  - `linux_config_backend_auto_parses_to_some_auto`:
        let cfg: Config = toml::from_str("[linux]\nbackend = \"auto\"\n").unwrap();
        assert_eq!(cfg.linux.backend.as_deref(), Some("auto"));   // auto-normalization is at the USE site (create_monitor), NOT parse
  - `render_config_body_emits_linux_block_when_set`:
        let cfg = Config { linux: LinuxConfig { backend: Some("x11".into()), ..Default::default() }, ..Config::default() };
        let body = render_config_body(&cfg);
        assert!(body.contains("[linux]"));
        assert!(body.contains("backend = \"x11\""));
  - `render_config_body_comments_linux_block_when_default`:
        let body = render_config_body(&Config::default());
        // default (both None) -> the [linux] block is present but backend/gnome lines are COMMENTED hints
        // (so a Settings save of a default config doesn't activate a bogus override).
        assert!(body.contains("[linux]"));
        assert!(!body.contains("\nbackend = \"auto\""));   // active line must NOT appear (it'd be a comment)
  - NAMING: `test fn` snake_case; group under a `// --- P2.M1.T2.S1 — [linux] table ---` header comment.
  - COVERAGE: parse (full/partial/absent), auto passthrough, render active-vs-commented, round-trip.

Task 8: VALIDATE (no edits) — see Validation Loop.
  - cargo build --bin qmkonnect  (clean; no new warnings)
  - cargo test --bin qmkonnect -- --test-threads=1  (existing + new tests green)
  - grep gates (Level 3): LinuxConfig struct; Config.linux field; manual Default has linux;
    render_config_body has [linux]; create_monitor TODO gone + cached_config().linux read;
    git diff --stat == 2 files.

Task 9: NEVER do these (out of scope / forbidden)
  - DO NOT add wayland/gnome/atspi Cargo features or deps (P2.M1.T2.S2).
  - DO NOT create wayland_ft.rs / gnome.rs / atspi.rs (P2.M2/P2.M3/P2.M4).
  - DO NOT switch Config to #[derive(Default)] (GOTCHA-1 — breaks debounce_ms).
  - DO NOT edit configured_filter() (notifier.rs) or configured_timing() (mod.rs) bodies.
  - DO NOT wire gnome_poll_interval_ms into any backend (the GNOME backend doesn't exist yet —
    P2.M3.T2; this task only parses + round-trips it).
  - DO NOT add docs/* prose (Mode A: the schema is self-documenting via [linux] + commented hints).
  - DO NOT edit PRD.md / tasks.json / prd_snapshot.md / Cargo.toml / .gitignore.
```

### Implementation Patterns & Key Details

```rust
// ===== LinuxConfig + Config field (Tasks 1-2) =====
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default)]
pub struct LinuxConfig {
    /// Force a specific backend (foreign-toplevel | gnome | hyprland | atsi | x11 | auto).
    /// None (or "auto") ⇒ runtime priority order in select_linux_backend. Diagnostic in normal use.
    #[serde(default)]
    pub backend: Option<String>,
    /// GNOME backend drift-correcting poll cadence (ms). None ⇒ 1000 (resolved at the GNOME use site).
    #[serde(default)]
    pub gnome_poll_interval_ms: Option<u64>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone)]   // NO derive Default (GOTCHA-1)
pub struct Config {
    #[serde(default)] pub vendor_id: Option<u16>,
    #[serde(default)] pub product_id: Option<u16>,
    #[serde(default)] pub usage_page: Option<u16>,
    #[serde(default)] pub usage: Option<u16>,
    #[serde(default = "default_debounce_ms")] pub debounce_ms: u64,
    #[serde(default = "default_poll_interval_ms")] pub poll_interval_ms: u64,
    /// Linux window-monitor backend overrides (`[linux]` table). Absent ⇒ auto-selection.
    #[serde(default)]
    pub linux: LinuxConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            vendor_id: None, product_id: None, usage_page: None, usage: None,
            debounce_ms: DEFAULT_DEBOUNCE_MS, poll_interval_ms: DEFAULT_POLL_INTERVAL_MS,
            linux: LinuxConfig::default(),   // ← NEW (both None = auto / 1000-at-use-site)
        }
    }
}
// NOTE (GOTCHA-1): the merged PRD (CONFIG.md §1) sketches `#[derive(... Default)]` on Config.
// The production code intentionally keeps this MANUAL impl: #[derive(Default)] would zero-init
// `debounce_ms` to 0 (not 50), silently disabling debouncing. Code wins (the standing PRD rule).

// ===== render_config_body [linux] block (Task 3) — mirrors the vendor_id Option pattern =====
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
pub fn render_config_body(config: &Config) -> String {
    // ... existing vid_line / pid_line / usage_page_line / usage_line ...
    let backend_line = match &config.linux.backend {
        Some(b) => format!("backend = {b:?}"),                       // active: backend = "x11"
        None => r#"# backend = "auto"   # auto | foreign-toplevel | gnome | hyprland | atsi | x11"#
            .to_string(),
    };
    let gnome_line = match config.linux.gnome_poll_interval_ms {
        Some(ms) => format!("gnome_poll_interval_ms = {ms}"),
        None => "# gnome_poll_interval_ms = 1000   # GNOME backend drift-poll cadence (ms)".to_string(),
    };
    format!(
        // ... existing header + vid/pid/usage_page/usage + debounce/poll ...
        "...\n\
         poll_interval_ms = {poll}\n\
         \n\
         # Linux window-monitor backend ([linux]). Both fields OPTIONAL; omit the table (or\n\
         # backend = \"auto\") for runtime auto-selection (foreign-toplevel -> GNOME -> Hyprland\n\
         # -> AT-SPI -> X11). A forced backend that is unavailable errors loudly. (Linux only.)\n\
         [linux]\n\
         {backend_line}\n\
         {gnome_line}\n",
        // ... existing args ...
        backend_line = backend_line,
        gnome_line = gnome_line,
    )
}

// ===== create_monitor wiring (Task 5) — closes the staged TODO =====
// src/platforms/mod.rs
#[cfg(target_os = "linux")]
{
    let forced = crate::core::cached_config()
        .ok()
        .and_then(|c| c.linux.backend)
        .and_then(|b| match b.to_ascii_lowercase().as_str() {
            "auto" | "" => None,   // GOTCHA-5: auto/empty = runtime priority order
            _ => Some(b),
        });
    if self_verbose && forced.is_some() {
        println!("config [linux] backend = {:?} (forced)", forced);
    }
    return linux::select_linux_backend(verbose, forced.as_deref());
}
// (the macOS/Windows arms + the not(any(...)) fallback are UNCHANGED)
```

### Integration Points

```yaml
MODULE REGISTRATION:
  - NONE. LinuxConfig is a plain struct in core/mod.rs (already part of the crate via `pub mod`).
    No new `pub mod` line.

SERDE SCHEMA:
  - add to: `pub struct Config` in src/core/mod.rs
  - pattern: "#[serde(default)] pub linux: LinuxConfig"  (serde default ⇒ LinuxConfig::default())
  - compat: a config.toml WITHOUT a [linux] table still parses (serde default fills the field);
    a config WITH a [linux] table binds the new fields. Legacy configs are unaffected.

DEFAULT IMPL:
  - update: the MANUAL `impl Default for Config` (+linux: LinuxConfig::default()).
  - DO NOT: switch Config to #[derive(Default)] (GOTCHA-1 — debounce_ms=0 regression).

RENDERERS (round-trip safety):
  - render_config_body: +[linux] block (active-when-Some / commented-when-None) — preserves a
    manual [linux] override through a Settings save.
  - render_default_config_template: +fully-commented [linux] hint — seeded file parses to default.

DOWNSTREAM CONSUMERS:
  - create_monitor (src/platforms/mod.rs): reads cached_config().linux.backend, normalizes
    Some("auto")/empty -> None, passes as `forced` to select_linux_backend. (Closes the TODO the
    parallel item P2.M1.T1.S1 staged.)
  - GNOME backend (P2.M3.T2, NOT written): will read gnome_poll_interval_ms at its poll site. This
    task only parses + round-trips it; no consumer wiring for it exists yet.

CONFIG: none (no new env vars; the table IS the config).
ROUTES: none (no CLI surface — --validate-rules / --list-callbacks are unaffected; there is no
        `--backend` flag; backend is set via the [linux] table only, per PLATFORMS.md §6).
DATABASE: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles clean. If rustc warns/errors on src/core/mod.rs or src/platforms/mod.rs
# (e.g. "missing field `linux` in init of `Config`" — the line-620 test you forgot to update),
# READ it and fix before proceeding.

# Confirm the additions are present:
grep -n 'pub struct LinuxConfig' src/core/mod.rs          # expect one definition
grep -n 'pub linux: LinuxConfig' src/core/mod.rs          # expect one field on Config
grep -n 'linux: LinuxConfig::default()' src/core/mod.rs   # expect one line in impl Default
grep -n '\[linux\]' src/core/mod.rs                        # expect render_config_body + render_default_config_template
grep -n 'TODO(P2.M1.T2.S1)' src/platforms/mod.rs          # expect ZERO matches (TODO closed)
grep -n 'c.linux.backend' src/platforms/mod.rs            # expect one read in create_monitor
```

### Level 2: Unit Tests — parse + round-trip (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# Single-threaded is MANDATORY crate-wide (shared debouncer state — AGENTS.md).
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL bin tests green — the new [linux] tests + the updated round-trip/defaults tests +
# the existing cache/filter/timing tests. A failure in a [linux] test means the Rust diverged
# from the PRD schema (CONFIG.md §1.3) — fix the Rust, not the test.

# Filter to just the new tests to see them individually:
cargo test --bin qmkonnect core::tests::linux -- --test-threads=1
cargo test --bin qmkonnect core::tests::render_config_body -- --test-threads=1
```

### Level 3: Cross-component regression (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --release        # release build (default features = hyprland,macos,linux-tray) must succeed
# Expected: clean. Proves the [linux] wiring in create_monitor compiles against the real
# select_linux_backend signature from the parallel item.

# Confirm the change surface is exactly two files:
git status --short
# Expected:
#   modified:   src/core/mod.rs
#   modified:   src/platforms/mod.rs
git diff --stat
# Expected: only those two files; Cargo.toml, docs/*, PRD/tasks.json untouched.

# Hand-verify a forced backend config parses + the wiring normalizes auto:
cat > /tmp/qmk_test.toml <<'EOF'
[linux]
backend = "auto"
gnome_poll_interval_ms = 1500
EOF
# (a smoke parse via a one-off Rust snippet or `cargo run -- -v` with XDG_CONFIG_HOME pointed at
#  a temp dir containing this config — optional; the unit tests already prove the parse + the
#  auto-normalization logic is unit-testable if you factor it into a tiny pure helper.)
```

### Level 4: Fidelity cross-check (optional, high-confidence)

```bash
# Cross-check the [linux] table parses identically to the PRD §1.3 example by round-tripping it
# through render_config_body + parse_config:
cd /home/dustin/projects/qmkonnect
# (covered by the render_config_body_emits_linux_block_when_set + the updated
#  render_config_body_preserves_non_vidpid_fields tests — they assert the exact Some/None
#  rendering + the write+re-parse round-trip. No additional manual step needed.)
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean (no NEW warnings on the two edited files).
- [ ] `cargo build --release` clean (create_monitor wiring compiles against select_linux_backend).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` — full crate green.
- [ ] `git status` shows exactly TWO modified files: `src/core/mod.rs`, `src/platforms/mod.rs`.

### Feature Validation (parity with CONFIG.md §1.3)
- [ ] `LinuxConfig { backend: Option<String>, gnome_poll_interval_ms: Option<u64> }` exists with `#[derive(serde::{Deserialize,Serialize}, Clone, Debug, Default)]` + `#[serde(default)]` on both fields.
- [ ] `Config` has `#[serde(default)] pub linux: LinuxConfig`.
- [ ] Manual `impl Default for Config` includes `linux: LinuxConfig::default()` (NOT derived — GOTCHA-1).
- [ ] `[linux] backend = "x11"` parses to `Some("x11")`; `[linux] gnome_poll_interval_ms = 2000` parses to `Some(2000)`.
- [ ] A config WITHOUT a `[linux]` table parses to `cfg.linux == LinuxConfig::default()` (zero-config parity).
- [ ] `render_config_body` round-trips the `[linux]` table (asserted in the updated `render_config_body_preserves_non_vidpid_fields`).
- [ ] `render_default_config_template` parses to all-default including `linux` (fully-commented `[linux]` hint).
- [ ] `create_monitor`'s Linux arm reads `cached_config().linux.backend`, normalizes `Some("auto")`/empty → `None`, passes as `forced` (no `TODO(P2.M1.T2.S1)` remains).

### Code Quality Validation
- [ ] `LinuxConfig` derives `Clone` (required by Config's `Clone` derive — GOTCHA-2).
- [ ] Field/doc naming matches the PRD (`backend`, `gnome_poll_interval_ms`, `[linux]`).
- [ ] `render_config_body` follows the existing Option render pattern (active-when-Some / commented-when-None).
- [ ] `render_config_body`'s `#[cfg_attr(not(any(macos,windows)), allow(dead_code))]` is UNCHANGED (GOTCHA-7).
- [ ] `configured_filter()` (notifier.rs) + `configured_timing()` (mod.rs) bodies UNCHANGED.
- [ ] No new Cargo dependencies; no `unsafe`; no external docs changed (Mode A).
- [ ] Scope respected: NO new backend modules, NO Cargo features, NO gnome_poll_interval_ms consumer wiring (GNOME backend is P2.M3.T2).

### Documentation & Deployment
- [ ] Code-level docs present on `LinuxConfig` + the `Config.linux` field (Mode A — the `[linux]` table is self-documenting).
- [ ] The PRD↔code `Default`-derivation drift is noted in a code comment (GOTCHA-1).

---

## Anti-Patterns to Avoid

- ❌ Do NOT switch `Config` to `#[derive(Default)]`. The PRD sketches it, but the production code's
      manual impl is load-bearing: deriving would zero-init `debounce_ms` to 0 (not 50), silently
      disabling debouncing. Code wins (GOTCHA-1). `LinuxConfig` CAN safely derive `Default`.
- ❌ Do NOT forget `Clone` on `LinuxConfig`. `Config` derives `Clone`; a non-`Clone` field fails the
      derive. Add `Debug` too (verbose logging) (GOTCHA-2).
- ❌ Do NOT add `linux` to `Config` without teaching `render_config_body` to serialize the `[linux]`
      table. A Settings save would then strip a manual `[linux] backend = "x11"` override — the exact
      bug the save renderer was created to fix. Render active-when-Some / commented-when-None (GOTCHA-3).
- ❌ Do NOT leave the line-~620 full struct-literal test un-updated. It constructs `Config { 6 fields }`
      with no `..Config::default()`; adding `linux` breaks compilation. Add `linux` + assert round-trip (GOTCHA-4).
- ❌ Do NOT forward `Some("auto")` as a forced backend name to `select_linux_backend`. "auto" is NOT
      a compiled-in candidate; the forced path would loud-Err. Normalize `Some("auto")`/empty → `None`
      in `create_monitor` (GOTCHA-5).
- ❌ Do NOT touch `render_config_body`'s `#[cfg_attr(not(any(macos,windows)), allow(dead_code))]`.
      It reflects who CALLS the fn (macos/win Settings dialogs); the `[linux]` block doesn't change
      that (GOTCHA-7).
- ❌ Do NOT edit `configured_filter()` (notifier.rs) or `configured_timing()` (mod.rs). They read only
      their existing fields; the new `linux` field is transparent to them.
- ❌ Do NOT wire `gnome_poll_interval_ms` into any backend. The GNOME backend is P2.M3.T2 (not written
      yet). This task only parses + round-trips it.
- ❌ Do NOT add a `--backend` CLI flag. Backend override is `[linux] table`-only per PLATFORMS.md §6.
- ❌ Do NOT add wayland/gnome/atspi Cargo features or backend modules (P2.M1.T2.S2 / P2.M2 / P2.M3 / P2.M4).
- ❌ Do NOT run tests multi-threaded — the crate shares debouncer state
      (`cargo test --bin qmkonnect -- --test-threads=1`).
- ❌ Do NOT edit `PRD.md`, any `tasks.json`, `prd_snapshot.md`, `Cargo.toml`, or `.gitignore`.

---

## Confidence Score: 9/10

This is a well-bounded, single-struct-addition port of an exact, quoted PRD schema (CONFIG.md §1/§1.3)
into a file whose entire relevant surface — the `Config` struct, the manual `Default` impl, both
renderers, and the two round-trip tests — is reproduced verbatim in `research/notes.md` §2 and the
Implementation Blueprint. The two non-obvious traps are both fully derived and pinned by tests: the
manual-vs-derived `Default` question (GOTCHA-1 — deriving would break `debounce_ms=50`; pinned by the
existing `empty_config_is_all_none` + the new `linux_config_absent_table_is_default` tests) and the
`render_config_body` round-trip invariant (GOTCHA-3 — pinned by the updated
`render_config_body_preserves_non_vidpid_fields` asserting `backend` survives a write+re-parse). The
`Clone` requirement (GOTCHA-2) is a compile-time fact the build immediately enforces; the line-~620
full-literal break (GOTCHA-4) is caught by `grep "Config {"` (3 sites, all enumerated); the
`auto`-normalization (GOTCHA-5) is the only behavioral subtlety at the use site and is documented with
the exact `filter` expression. The `create_monitor` close-out is a ~3-line fill of a seam the parallel
item explicitly staged for this task (sequential, non-conflicting), which makes `[linux] backend`
functional rather than dead. No new deps, no `unsafe`, no new modules, no architectural decisions
remain open. The 1-point reservation is for the (unlikely) event an implementer skips the
`render_config_body` round-trip extension despite GOTCHA-3 (which would ship a latent Settings-save
override-stripping bug caught only by the updated round-trip test) or forwards `"auto"` as a forced
name despite GOTCHA-5 (caught at runtime by the dispatcher's loud-Err + a unit test). Scope is cleanly
bounded from the dispatcher (P2.M1.T1.S1), the Cargo features (P2.M1.T2.S2), and every backend module
(P2.M2–P2.M5), so there is no risk of over- or under-building.
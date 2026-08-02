# PRP — P1.M2.T2.S2: Migrate the 3 settings-dialog `config.toml` saves to `atomic_write`

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`) — Rust menu-bar/tray daemon.
> **Two files edited:** `src/tray.rs` (2 sites) + `src/linux_tray.rs` (1 site) — **three one-line swaps**.
> **Scope:** migrate ONLY the 3 settings-dialog save call sites. The 2 `core/mod.rs`
> seeder sites (`mod.rs:218`, `mod.rs:334`) are **P1.M2.T2.S1** — do NOT touch them.
> **Dependency (CONTRACT):** P1.M2.T1.S1 added `pub fn atomic_write(path: &Path,
> content: &str) -> Result<(), Box<dyn Error>>` to `src/core/mod.rs` (verified present
> at line 212 this session). P1.M2.T2.S1 migrates the seeders (assume it lands as specified).

---

## Goal

**Feature Goal**: Replace the three non-atomic `std::fs::write` settings-dialog saves
in `src/tray.rs` (`#[cfg(windows)]` `show_settings_dialog` line 878; `#[cfg(macos)]`
`show_settings_dialog_with_pool` line 1276) and `src/linux_tray.rs` (`write_config`
line 822) with calls to the `atomic_write` helper (built by P1.M2.T1.S1), so that a
save triggered by the user clicking OK in a Settings dialog can never be observed by
the concurrent notifier-thread reader (`parse_config` / `configured_filter`, run per
window-change with no locking) in a truncated or partial state. This is the
tray-dialog half of bug-hunt finding #1 ("Non-atomic config/rules file writes"); the
seeder half is P1.M2.T2.S1.

**Deliverable**: THREE one-line edits:
- `tray.rs:878`  →  `crate::core::atomic_write(config_path, &config_content)?;`
- `tray.rs:1276` →  `crate::core::atomic_write(config_path, &config_content)?;`
- `linux_tray.rs:822` →  `crate::core::atomic_write(&path, &content)?;`

No new functions, no new imports, no new tests, no other files touched.

**Success Definition**:
- All three call sites call `crate::core::atomic_write(...)` with byte-identical content
  to what `fs::write` received (the path arg shape is unchanged; the content arg gains a `&`).
- `cargo build --bin qmkonnect` compiles on Linux with no new warnings — this validates
  the `linux_tray.rs:822` edit (see Platform-Validation Reality below).
- `cargo test --bin qmkonnect -- --test-threads=1` is fully green (existing count unchanged).
- `git diff --stat` shows ONLY `src/tray.rs` and `src/linux_tray.rs`.
- The two `tray.rs` edits (`#[cfg(windows)]`, `#[cfg(macos)`) are mechanical mirrors of
  the validated `linux_tray.rs` edit and of P1.M2.T2.S1's seeder swaps; they compile-check
  on their target OS per the AGENTS.md dev loop (Windows build on Windows, macOS on macOS).

## User Persona (if applicable)

**Target User**: (1) The QMKonnect daemon's **notifier thread** — the concurrent reader
that re-parses `config.toml` on every window change (`configured_filter` → `parse_config`)
with no locking. (2) Any user who edits VID/PID in the Windows / macOS / Linux Settings
dialog while the daemon is running.

**Use Case**: User opens Settings → edits VID/PID → OK. The tray thread calls
`render_config_body(&merged)` (pure `String`), then writes the file. Before: a notifier
thread `read_to_string` landing during the truncate-then-write could see an empty/partial
file (silent wrong-defaults or a spurious parse-error notification). After:
`atomic_write` stages the body in `.{file_name}.tmp` then `rename`s over the target, so the
reader sees old-or-new, never partial.

**Pain Points Addressed**: closes the read race for the 3 dialog save paths. Impact today
is Low (readers degrade gracefully to defaults + a one-time notification), but a partial
write parsing as *valid TOML with wrong values* could briefly persist — atomic replace
eliminates that window.

## Why

- **Closes the tray-dialog half of finding #1.** These are 3 of the 5 non-atomic call
  sites catalogued in `architecture/config_writes_research.md` §1 (the other 2 are the
  seeders, P1.M2.T2.S1). This task + P1.M2.T2.S1 together retire finding #1 entirely.
- **Trivial, mechanical, reviewable.** `atomic_write` is a drop-in for
  `fs::write(path, content)?` (same path shape; same return type; `?` propagates unchanged).
  Each call site is a one-line change with zero behavior delta.
- **No new dependency, no new test surface.** `atomic_write` lives in `core` and is
  reached via the fully-qualified `crate::core::` path the rest of these files already use;
  the helper's own unit tests land in P1.M2.T1.S1.

## What

Make exactly three one-line edits. Nothing else.

### Edit 1 — `src/tray.rs:878` (Windows `show_settings_dialog`, `#[cfg(target_os="windows")]`)
```rust
// BEFORE (current, 12-space indent, line 878):
            std::fs::write(config_path, config_content)?;

// AFTER:
            crate::core::atomic_write(config_path, &config_content)?;
```
- `config_path` is `&std::path::Path` (fn param) — pass DIRECTLY (it is already `&Path`;
  `atomic_write`'s first arg is `&Path`). Do NOT write `&config_path` (needless `&&Path`).
- `config_content` is `String` — pass `&config_content` (`&String` → `&str`). The current
  `fs::write` call moves it by value; `atomic_write` borrows, hence the leading `&`.

### Edit 2 — `src/tray.rs:1276` (macOS `show_settings_dialog_with_pool`, `#[cfg(target_os="macos")]`)
```rust
// BEFORE (current, 20-space indent, line 1276):
                    std::fs::write(config_path, config_content)?;

// AFTER:
                    crate::core::atomic_write(config_path, &config_content)?;
```
- Same arg shapes as Edit 1 (`config_path: &Path`; `config_content: String` → `&config_content`).
- Note the 20-space indent (deeper nesting than Edit 1's 12-space indent ⇒ the two lines are
  individually unique in `tray.rs`).

### Edit 3 — `src/linux_tray.rs:822` (`write_config`, module `#![cfg(all(target_os="linux", feature="linux-tray"))]`)
```rust
// BEFORE (current, 4-space indent, line 822):
    std::fs::write(&path, content)?;

// AFTER:
    crate::core::atomic_write(&path, &content)?;
```
- `path` is `std::path::PathBuf` (from `dir.join("config.toml")`) — pass `&path`
  (`&PathBuf` → `&Path` via deref coercion), exactly as the current `fs::write` does.
- `content` is `String` — pass `&content`. The `?` propagates into `write_config`'s
  `Result<std::path::PathBuf, Box<dyn std::error::Error>>` unchanged.

### Success Criteria
- [ ] `tray.rs:878` calls `crate::core::atomic_write(config_path, &config_content)?;`.
- [ ] `tray.rs:1276` calls `crate::core::atomic_write(config_path, &config_content)?;`.
- [ ] `linux_tray.rs:822` calls `crate::core::atomic_write(&path, &content)?;`.
- [ ] Final file content byte-identical to before (same content arg flows through `atomic_write`).
- [ ] No new `use` lines added — `atomic_write` is called fully-qualified (`crate::core::…`),
      matching the universal convention in both files (see Known Gotchas).
- [ ] `cargo build --bin qmkonnect` compiles on Linux with no new warnings (validates Edit 3).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` fully green (existing count; no tests added/removed).
- [ ] `git diff --stat` shows ONLY `src/tray.rs` and `src/linux_tray.rs`.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can make all three edits using only the exact
anchors above (indentation-unique, grep-confirmed), the verified arg types/shapes, the
fully-qualified-call convention, and the verified `cargo build`/`cargo test` gates.

### Documentation & References

```yaml
# MUST READ — the dependency (CONTRACT) whose output this task consumes
- file: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/P1M2T1S1/PRP.md
  why: "defines atomic_write's exact signature + semantics — this task is a pure consumer"
  critical: "atomic_write(path: &Path, content: &str) -> Result<(), Box<dyn Error>>. It is a
        drop-in for fs::write(path, content)? — the path arg shape is identical and the `?`
        propagates unchanged. ONE difference: it takes content as &str, so each new call adds
        a leading `&` to the content arg."

# MUST READ — the sibling (CONTRACT) being implemented in parallel
- file: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/P1M2T2S1/PRP.md
  why: "migrates the 2 core/mod.rs seeder sites (218, 334) via the SAME one-line swap. This
        task is the tray-dialog half. Confirms the 'no new import, no new test, no behavior
        change' contract and the contract-text-imprecision note pattern."
  critical: "P1.M2.T2.S1 owns mod.rs:218 + mod.rs:334. This task owns tray.rs:878, tray.rs:1276,
        linux_tray.rs:822. No overlap. P1.M2.T2.S1 calls atomic_write UNQUALIFIED (same module);
        THIS task calls it fully-qualified as crate::core::atomic_write (different module)."

# MUST READ — PRD / bug-hunt context (the "why")
- url: spec/PRD.md (heading h2.1, finding #1 "Non-atomic config/rules file writes")
  why: the exact defect this fixes (truncate-then-write read race; graceful but spurious behavior)
  critical: "impact is LOW (readers degrade to defaults); the win is eliminating the brief
        'valid TOML with wrong values' window. Do NOT over-engineer — three one-line swaps."

- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/architecture/config_writes_research.md
  why: "catalogues all 5 non-atomic call sites; confirms tray.rs:878 (Windows dialog),
        tray.rs:1276 (macOS dialog), linux_tray.rs:822 (write_config) are the 3 sites in scope
        here; the other 2 (mod.rs:218/334) are P1.M2.T2.S1. §1.A + §2 (render_config_body callers)."
  section: "1.A All write call sites — Production writers for config.toml" + "2. render_config_body"

# MUST READ — the files being edited (exact current state, verified this session)
- file: src/tray.rs
  why: both settings-dialog write sites + the imports + the surrounding read/merge pattern
  pattern: "tray.rs has NO `use crate::core::` imports — every core call is fully-qualified
        (crate::core::render_config_body @ :876 & :1275; crate::core::parse_config @ :763/:1191;
        crate::core::create_default_config @ :681/:709). The ONLY top `use` lines are
        `use tao::{…}` (:2) and `use tray_icon::{…}` (:7). Windows fn show_settings_dialog @ :752
        (config_path: &std::path::Path); fs::write @ :878. macOS fn show_settings_dialog_with_pool
        @ :1185 (config_path: &std::path::Path); fs::write @ :1276."
  gotcha: "do NOT add `use crate::core::atomic_write;` — it would be the ONLY such import in the
        file and breaks the fully-qualified convention. Call it `crate::core::atomic_write(...)`.
        The two write lines differ in INDENTATION (12 vs 20 spaces) so each is grep/edit-unique."

- file: src/linux_tray.rs
  why: the write_config fn + the module cfg-gate + imports
  pattern: "module is `#![cfg(all(target_os=\"linux\", feature=\"linux-tray\"))]` (:36).
        write_config @ :805 returns Result<PathBuf, Box<dyn Error>>; path is PathBuf (dir.join,
        :807); fs::write(&path, content) @ :822. NO `use crate::core::` imports; core calls are
        fully-qualified (crate::core::render_config_body @ :821; crate::core::parse_config @ :814).
        Top `use` lines (:38-43) are ksni/std — none from crate::core."
  gotcha: "the entire module is Linux+linux-tray gated, so write_config only compiles on Linux.
        Pass &path (PathBuf→&Path) and &content (String→&str). No new import."

# REFERENCE — atomic_write's implementation (lands in P1.M2.T1.S1; do NOT redefine/re-call differently)
- file: src/core/mod.rs:212 (pub fn atomic_write)
  why: "confirms atomic_write stages `.{file_name}.tmp` in path.parent() then fs::rename over the
        target — same content, atomic to a concurrent reader; cleans up its temp on error"
  critical: "atomic_write already owns its own temp cleanup on error; these 3 call sites need NO
        temp cleanup of their own — the single `?` at each site is the complete error path."
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
src/tray.rs            # EDIT: line 878 (Windows dialog) + line 1276 (macOS dialog)
  - :2 use tao::{…}; :7 use tray_icon::{…}   (the ONLY top `use` lines; NO use crate::core)
  - :681/:709  crate::core::create_default_config (fully-qualified — the convention)
  - :752  #[cfg(windows)] fn show_settings_dialog(config_path: &std::path::Path)  <- fs::write @ :878
  - :1185 #[cfg(macos)]  fn show_settings_dialog_with_pool(config_path: &std::path::Path)  <- fs::write @ :1276
src/linux_tray.rs      # EDIT: line 822 (write_config)
  - :36 #![cfg(all(target_os="linux", feature="linux-tray"))]   (whole module Linux-gated)
  - :38-43 use ksni…/std (NO use crate::core)
  - :805 fn write_config(...) -> Result<PathBuf, Box<dyn Error>>   <- fs::write(&path, content) @ :822
src/core/mod.rs        # DO NOT TOUCH (P1.M2.T1.S1 atomic_write @ :212; P1.M2.T2.S1 seeders @ :218/:334)
Cargo.toml             # DO NOT TOUCH
```

### Desired Codebase tree
**Only `src/tray.rs` and `src/linux_tray.rs` change** (three one-line edits). No new files,
no new modules, no Cargo.toml edit, no new tests.

### Known Gotchas of our codebase & Library Quirks
```rust
// CRITICAL: call atomic_write FULLY-QUALIFIED as `crate::core::atomic_write(...)`. Do NOT add
//   `use crate::core::atomic_write;`. Both tray.rs and linux_tray.rs have ZERO `use crate::core::`
//   imports — every core call (render_config_body, parse_config, create_default_config) is
//   fully-qualified. A lone import for atomic_write would be the only one in the file and breaks
//   the convention in review. (The work-item contract's "add an import" suggestion is a minor
//   inaccuracy, like P1.M2.T2.S1's `&config_path` note. The new line sits directly below
//   `crate::core::render_config_body(&merged)` — match THAT call's fully-qualified form.)

// CRITICAL: add a leading `&` to the CONTENT arg at every site. The current `fs::write` calls
//   pass the String by VALUE (move); atomic_write takes `&str`. So: `&config_content` (sites 1+2)
//   and `&content` (site 3). Forgetting the `&` moves a String into a &str slot — the compiler
//   will flag it, but get it right the first time.

// CRITICAL: pass the PATH arg in the SAME shape the current fs::write uses. tray.rs sites 1+2:
//   `config_path` is already `&Path` (fn param) — pass it DIRECTLY (not `&config_path`, which is
//   `&&Path`). linux_tray.rs site 3: `path` is `PathBuf` — pass `&path` (`&PathBuf`→`&Path`), exactly
//   as the current `fs::write(&path, …)` does.

// CRITICAL (PLATFORM VALIDATION): the dev box is Linux. `cargo build`/`cargo test` on Linux compile
//   linux_tray.rs (linux-tray is in Cargo.toml `default` features) but NOT the #[cfg(windows)]/
//   #[cfg(macos)] tray.rs sites. So on Linux: Edit 3 (linux_tray.rs:822) IS compile-validated;
//   Edits 1+2 (tray.rs:878/:1276) are NOT — they compile-check only on Windows / macOS respectively
//   (per the AGENTS.md dev loop). They are mechanical mirrors of Edit 3 (same &Path + &str shapes),
//   so the risk is low, but do NOT claim they are validated on a Linux box. If you can cross-compile
//   (`cargo check --target x86_64-pc-windows-msvc` / x86_64-apple-darwin), do so; otherwise note
//   the tray.rs edits as deferred-to-target-OS in the report.

// GOTCHA: this is a NO-BEHAVIOR-CHANGE migration. The content passed to atomic_write is
//   byte-identical to what fs::write received; the return type (Result<(), Box<dyn Error>>) and the
//   `?` propagation are identical. The ONLY difference is the write mechanism
//   (truncate-then-write → temp+rename). Do not "improve" the content, the messages, the
//   read/merge logic above each line, or the return handling below.

// GOTCHA: there are NO unit tests for show_settings_dialog / show_settings_dialog_with_pool /
//   write_config (platform UI code, exercised manually per the AGENTS.md dev loop). The
//   migration's proof is: cargo build (Linux validates Edit 3) + the full --test-threads=1 suite
//   staying green (no test added/removed) + the byte-identical-content argument. Do NOT add tests.

// GOTCHA: tests MUST run single-threaded: `cargo test --bin qmkonnect -- --test-threads=1`
//   (AGENTS.md) — the suite shares global debouncer state. Parallel runs flap.

// GOTCHA: do NOT touch the out-of-scope write sites: tray.rs:204 (autostart marker file) and
//   linux_tray.rs:762 (apply_device_rule udev-rule staging for pkexec install). Neither is a
//   config/rules save; both are different concerns.
```

## Implementation Blueprint

### Data models and structure
None. `Config` is unchanged; the inputs to all three call sites (`&Path`/`&PathBuf` +
`String` content) are already computed above each line and flow unchanged into
`atomic_write` (only the content gains a borrow `&`).

### Implementation Tasks (ordered by dependencies)

```yaml
Task 0: CONFIRM the dependency is present (P1.M2.T1.S1 landed atomic_write)
  - RUN: grep -n 'pub fn atomic_write' src/core/mod.rs
  - EXPECT: one match around line ~212.
  - IF ABSENT: P1.M2.T1.S1 has not landed yet. This task depends on it — do NOT proceed.
    Expected signature: pub fn atomic_write(path: &Path, content: &str) -> Result<(), Box<dyn Error>>

Task 1: EDIT src/tray.rs — swap the Windows dialog write (line 878)
  - OLD (exact, 12-space indent, grep-confirmed unique): "            std::fs::write(config_path, config_content)?;"
  - NEW: "            crate::core::atomic_write(config_path, &config_content)?;"
  - NOTE: config_path is &Path (pass directly); config_content is String (pass &config_content).
  - PRESERVE: the read/merge logic above (current_config, merged, render_config_body) and the
    "Configuration saved successfully" comment + Ok(()) below.

Task 2: EDIT src/tray.rs — swap the macOS dialog write (line 1276)
  - OLD (exact, 20-space indent, grep-confirmed unique): "                    std::fs::write(config_path, config_content)?;"
  - NEW: "                    crate::core::atomic_write(config_path, &config_content)?;"
  - NOTE: same arg shapes as Task 1. The 20-space indent (vs Task 1's 12-space) makes this line
    individually unique in tray.rs.
  - PRESERVE: the surrounding NSAlert/parse flow and the (Err(e),_) error arm below.

Task 3: EDIT src/linux_tray.rs — swap write_config's write (line 822)
  - OLD (exact, 4-space indent, grep-confirmed unique): "    std::fs::write(&path, content)?;"
  - NEW: "    crate::core::atomic_write(&path, &content)?;"
  - NOTE: path is PathBuf (pass &path → &Path); content is String (pass &content). The `?` still
    propagates into write_config's Result<PathBuf, Box<dyn Error>>.
  - PRESERVE: the render_config_body(&config) above and `Ok(path)` below.

Task 4: VALIDATE (no edits)
  - cargo build --bin qmkonnect          # Linux: compiles; validates Edit 3 (linux_tray.rs).
                                           # Edits 1+2 are #[cfg(windows/macos)] — NOT compiled on Linux.
  - cargo test --bin qmkonnect -- --test-threads=1   # full suite green; existing count unchanged
  - git diff --stat                       # ONLY src/tray.rs + src/linux_tray.rs
  - grep -n 'fs::write' src/tray.rs src/linux_tray.rs   # the 3 migrated sites are GONE from
                                           # production code; remaining matches are tray.rs:204
                                           # (autostart marker) + linux_tray.rs:762 (udev staging) —
                                           # both out of scope, expected unchanged.

Task 5: (OPTIONAL, if a cross-toolchain is available) compile-check the tray.rs edits
  - IF `rustup target list --installed` shows x86_64-pc-windows-msvc / x86_64-apple-darwin:
      cargo check --target x86_64-pc-windows-msvc --bin qmkonnect
      cargo check --target x86_64-apple-darwin  --bin qmkonnect
    to compile-check Edits 1+2 on the Linux box. Otherwise note in the report that Edits 1+2
    are mechanical mirrors of Edit 3 and defer final compile-check to the target-OS builds
    (Windows: AGENTS.md Windows loop; macOS: AGENTS.md macOS loop).

Task 6: NEVER do these (out of scope / forbidden)
  - DO NOT migrate src/core/mod.rs:218 or :334 — those are P1.M2.T2.S1.
  - DO NOT add `use crate::core::atomic_write;` — call it fully-qualified (crate::core::…).
  - DO NOT add new tests (contract: existing tests pass; atomic_write's own tests are P1.M2.T1.S1).
  - DO NOT touch the read/merge logic (parse_config / render_config_body / merged) or the
    Ok(()) / Ok(path) returns.
  - DO NOT touch tray.rs:204 (autostart marker) or linux_tray.rs:762 (udev-rule staging).
  - DO NOT edit Cargo.toml, PRD.md, tasks.json, prd_snapshot.md, or any other source file.
```

### Implementation Patterns & Key Details
```rust
// PATTERN: atomic_write is a drop-in for fs::write(path, content)?, with ONE shape delta —
//   it borrows the content. The path arg keeps its current shape; the content arg gains `&`.
//
//   // Windows dialog  (tray.rs:878) — config_path: &Path; config_content: String
//   crate::core::atomic_write(config_path, &config_content)?;
//
//   // macOS dialog    (tray.rs:1276) — config_path: &Path; config_content: String
//   crate::core::atomic_write(config_path, &config_content)?;
//
//   // Linux write_config (linux_tray.rs:822) — path: PathBuf; content: String
//   crate::core::atomic_write(&path, &content)?;
//
// WHY fully-qualified (not an import): tray.rs and linux_tray.rs have ZERO `use crate::core::`
//   imports — every core symbol is called as crate::core::…. The new line sits directly under
//   crate::core::render_config_body(&merged); matching that call is the local idiom. (Adding a
//   lone `use crate::core::atomic_write;` would be the file's only such import — out of place.)
//
// WHY this is safe: each call site's parent dir already exists (the config dir is created by
//   get_config_paths / create_config_dir upstream). atomic_write's temp lives in path.parent()
//   (same fs) so rename is atomic. atomic_write cleans up its own temp on error, so the single
//   `?` at each site is the complete error path — no extra cleanup needed.
//
// ANTI-PATTERN: do NOT pass `&config_path` at the tray.rs sites (=> &&Path). config_path is
//   already &Path — pass it directly. (Only the CONTENT gains a `&`.)
//
// ANTI-PATTERN: do NOT "improve" anything else — content, read/merge logic, messages, returns.
//   This is a pure mechanism swap with zero behavior delta.
```

### Integration Points
```yaml
IMPORTS: none new. (atomic_write called fully-qualified as crate::core::atomic_write in both files.)
DEPENDENCY: P1.M2.T1.S1 — pub fn atomic_write present in src/core/mod.rs:212 (Task 0 confirms).
CARGO:   none. No Cargo.toml change.
PARALLEL (no conflict):
  - P1.M2.T1.S1 adds atomic_write to src/core/mod.rs (different file). Composes once landed.
  - P1.M2.T2.S1 migrates the 2 core/mod.rs seeder sites (different file). No overlap with this task.
PLATFORM VALIDATION (CRITICAL):
  - Linux box: cargo build validates Edit 3 (linux_tray.rs:822) only. Edits 1+2 (tray.rs #[cfg
    windows/macos]) are NOT compiled on Linux — defer their compile-check to Windows / macOS
    builds per the AGENTS.md dev loop. They are mechanical mirrors of Edit 3 (same &Path + &str).
```

## Validation Loop

> Toolchain: Rust (`cargo`). Project has no ruff/mypy. `cargo build` + `cargo test` are the gates.
> Tests MUST run single-threaded (AGENTS.md — shared debouncer state).

### Level 1: Syntax & Style (Immediate Feedback)
```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles with zero new warnings. On Linux this validates Edit 3 (linux_tray.rs).
# If "cannot find function `atomic_write`" → P1.M2.T1.S1 has not landed (see Task 0).
# (Edits 1+2 in tray.rs are #[cfg(windows/macos)] — not compiled on Linux; see Task 5.)
```

### Level 2: Manual settings-dialog exercise (per AGENTS.md dev loop)
```bash
# There are NO unit tests for these three platform UI functions. Validate via the dev loop:
#   Linux:   cargo build --bin qmkonnect  (already Level 1), then run the binary, open Settings,
#            change VID/PID, OK, and confirm config.toml is rewritten correctly + no .tmp lingers.
#   Windows: AGENTS.md Windows loop — cargo build --release; taskkill; .\target\release\qmkonnect.exe;
#            open Settings, OK, confirm config.toml rewritten + no .tmp lingers.
#   macOS:   AGENTS.md macOS loop — packaging/macos build+install; open /Applications/QMKonnect.app;
#            Settings, OK, confirm config.toml rewritten + no .tmp lingers.
# Expected (all OSes): the saved config.toml has exactly the merged VID/PID + preserved fields;
#                      the old content is fully replaced; no `config.toml.tmp` file remains.
```

### Level 3: Full Suite (Regression — AGENTS.md mandates single-threaded)
```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL existing tests still pass. Net test count UNCHANGED (no tests added or removed).
#           --test-threads=1 is REQUIRED (shared global debouncer state; parallel runs flap).
```

### Level 4: Scope/Build Hygiene
```bash
cd /home/dustin/projects/qmkonnect
git diff --stat                 # Expected: ONLY src/tray.rs and src/linux_tray.rs.
git diff Cargo.toml             # Expected: empty.
# The 3 migrated sites are gone from production code; remaining fs::write matches are out-of-scope:
grep -n 'fs::write' src/tray.rs src/linux_tray.rs
# Expected: NO match at tray.rs:878/:1276 or linux_tray.rs:822.
#           tray.rs:204 (autostart marker) + linux_tray.rs:762 (udev staging) remain — UNCHANGED.
# Also confirm the core/mod.rs seeder sites are NOT touched here (they are P1.M2.T2.S1):
git diff src/core/mod.rs        # Expected: empty (this task does not edit core/mod.rs).
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` succeeds on Linux with no new warnings (validates Edit 3).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` → full suite green, test count unchanged.
- [ ] `git diff --stat` shows ONLY `src/tray.rs` and `src/linux_tray.rs`.

### Feature Validation
- [ ] `tray.rs:878` calls `crate::core::atomic_write(config_path, &config_content)?;`.
- [ ] `tray.rs:1276` calls `crate::core::atomic_write(config_path, &config_content)?;`.
- [ ] `linux_tray.rs:822` calls `crate::core::atomic_write(&path, &content)?;`.
- [ ] Final file content byte-identical to before at all three sites (mechanism swap only).
- [ ] Manual settings-dialog save (per OS dev loop) rewrites config.toml correctly with no `.tmp` left.

### Code Quality Validation
- [ ] No new `use` lines; `atomic_write` called fully-qualified (`crate::core::…`) in both files.
- [ ] Content arg borrows (`&config_content` / `&content`); path arg in its current shape.
- [ ] No new tests added (contract: existing tests pass; helper tests are P1.M2.T1.S1).
- [ ] No behavior change — content, read/merge logic, messages, returns all identical.
- [ ] `src/core/mod.rs` UNCHANGED (seeders are P1.M2.T2.S1); tray.rs:204 & linux_tray.rs:762 untouched.

### Documentation & Deployment
- [ ] No user-facing / config / API surface change (internal mechanism swap — DOCS: none per contract).
- [ ] No new env vars / config keys / CLI flags.
- [ ] Report notes that Edits 1+2 (tray.rs #[cfg windows/macos]) are compile-validated only on their
      target OS (Windows/macOS builds per AGENTS.md); Edit 3 is validated on the Linux box.

---

## Anti-Patterns to Avoid
- ❌ Don't add `use crate::core::atomic_write;` — both files use fully-qualified `crate::core::` calls
  exclusively (zero such imports today). Call it `crate::core::atomic_write(...)`.
- ❌ Don't forget the `&` on the content arg — `fs::write` moves the String; `atomic_write` borrows
  (`&str`). Omitting it is a compile error; get it right the first time.
- ❌ Don't pass `&config_path` at the tray.rs sites (yields `&&Path`); `config_path` is already `&Path`.
  (Only the CONTENT gains a `&`; the path arg keeps its current shape.)
- ❌ Don't migrate `src/core/mod.rs:218` or `:334` — those are P1.M2.T2.S1.
- ❌ Don't touch `tray.rs:204` (autostart marker) or `linux_tray.rs:762` (udev-rule staging) — out of scope.
- ❌ Don't add new tests — the contract asks only that existing tests pass; `atomic_write`'s own tests are P1.M2.T1.S1.
- ❌ Don't run tests without `--test-threads=1` (AGENTS.md — shared debouncer state).
- ❌ Don't claim the tray.rs (`#[cfg windows/macos]`) edits are validated on a Linux box — `cargo build`
  on Linux compiles only `linux_tray.rs`. Defer the tray.rs compile-check to Windows / macOS.
- ❌ Don't edit Cargo.toml, PRD.md, tasks.json, or any file other than `src/tray.rs` and `src/linux_tray.rs`.
- ❌ Don't "improve" anything — this is a pure mechanism swap with zero behavior delta.

---

## Confidence Score: 9/10

The task is three mechanical one-line swaps with exact, grep-confirmed, indentation-unique anchors,
a verified drop-in dependency signature (`atomic_write` present at `core/mod.rs:212`, baseline
`cargo build` green this session), zero behavior change, and no new test surface. The two nuances
the PRP nails: (1) call `atomic_write` fully-qualified (the contract's import suggestion conflicts
with the file's universal convention — verified zero `use crate::core::` imports in both files), and
(2) add a leading `&` to the content arg (the only arg-shape delta from `fs::write`). The 1-point
reservation is the platform-validation split: on a Linux box only Edit 3 (`linux_tray.rs:822`) is
compile-validated; Edits 1+2 (`tray.rs` `#[cfg windows/macos]`) defer to their target-OS builds per
AGENTS.md — they are mechanical mirrors of Edit 3, so risk is low, but the implementer must not
falsely report them as compile-checked on Linux. This is mitigated by Task 5's optional cross-check
and the explicit "deferred-to-target-OS" report note.
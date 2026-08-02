# PRP — P1.M2.T2.S1: Migrate `create_default_config` + `create_default_rules` to `atomic_write`

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`) — Rust menu-bar/tray daemon.
> **One file edited:** `src/core/mod.rs` — **two one-line swaps** (lines 218 and 334).
> **Scope:** migrate ONLY the two `core/mod.rs` seeder write call sites. The other
> three call sites (`tray.rs:878`, `tray.rs:1276`, `linux_tray.rs:822`) are
> **P1.M2.T2.S2** — do NOT touch them.
> **Dependency (CONTRACT):** P1.M2.T1.S1 adds `pub fn atomic_write(path: &Path,
> content: &str) -> Result<(), Box<dyn Error>>` to this same file. Assume it lands
> exactly as specified there.

---

## Goal

**Feature Goal**: Replace the two non-atomic `std::fs::write` seeders in
`src/core/mod.rs` — `create_default_config` (line 218) and `create_default_rules`
(line 334) — with calls to the `atomic_write` helper (built in parallel by
P1.M2.T1.S1), so that first-run seeding of `config.toml` / `rules.toml` can never
be observed by a concurrent reader (the notifier thread's `parse_config` /
`parse_rules`) in a truncated or partial state. This is the `core/mod.rs` half of
bug-hunt finding #1 ("Non-atomic config/rules file writes").

**Deliverable**: TWO one-line edits in `src/core/mod.rs`:
- `fs::write(config_path, default_config)?;`  →  `atomic_write(config_path, &default_config)?;`
- `fs::write(rules_path, render_rules_body())?;`  →  `atomic_write(rules_path, &render_rules_body())?;`

No new functions, no new imports, no new tests, no other files touched.

**Success Definition**:
- Both call sites call `atomic_write` with the SAME content that `fs::write`
  received (final file content byte-identical to before).
- `cargo build --bin qmkonnect` compiles with no new warnings.
- `cargo test --bin qmkonnect -- --test-threads=1` is fully green (existing count
  unchanged). In particular `test_create_default_rules_noop_if_exists` and
  `test_create_default_rules_writes_when_absent` pass (the latter exercises the
  migrated line 334 write path).
- `git diff --stat` shows ONLY `src/core/mod.rs`.

## User Persona (if applicable)

**Target User**: (1) The QMKonnect daemon's **notifier thread** — the concurrent
reader that re-parses `config.toml` / `rules.toml` on every window change with no
locking. (2) A brand-new user on first run (`qmkonnect -c` or auto-seed) whose
`config.toml` / `rules.toml` is being written while the daemon may already be
running and reading. (3) The P1.M2.T2.S2 implementer, who will mirror this exact
swap at the three tray-dialog call sites.

**Use Case**: First launch → `create_default_config(config_path)` runs on the
startup thread; the notifier thread may concurrently call `configured_filter` →
`parse_config`. Before: a `read_to_string` landing during the truncate-then-write
could see an empty/partial file (silent wrong-defaults or a spurious parse error).
After: `atomic_write` stages the body in `.config.toml.tmp` then `rename`s over the
target, so the reader sees old-or-new, never partial.

**Pain Points Addressed**: closes the read race for the seeder paths (the
tray-dialog paths close in P1.M2.T2.S2). Impact today is Low (readers degrade
gracefully to defaults), but a partial write parsing as *valid TOML with wrong
values* could briefly persist — atomic replace eliminates that window.

## Why

- **Closes the `core/mod.rs` half of finding #1.** The seeders are 2 of the 5
  non-atomic call sites catalogued in `architecture/config_writes_research.md` §1.
  This task + P1.M2.T2.S2 together retire finding #1 entirely.
- **Trivial, mechanical, reviewable.** Because `atomic_write` is a drop-in for
  `fs::write(path, content)?` (same param shapes, same return type), each call site
  is a one-line change with zero behavior delta. Isolating the `core/mod.rs` sites
  from the tray-dialog sites keeps each PR small and independently reviewable.
- **No new dependency, no new test surface.** `atomic_write` lives in the same
  module (in scope unqualified), `use std::path::Path;` is already imported
  (mod.rs:8), and the helper's own unit tests land in P1.M2.T1.S1 — so this task
  adds no tests, just relies on the existing suite.

## What

Make exactly two edits in `src/core/mod.rs`. Nothing else.

### Edit 1 — `create_default_config`, line 218
```rust
// BEFORE (current, line 216-218):
    // Write the config file
    fs::write(config_path, default_config)?;

// AFTER:
    // Write the config file
    atomic_write(config_path, &default_config)?;
```
- `config_path` is `&Path` (the function's param) — pass it directly (it is already
  the type `atomic_write`'s first arg expects). Do NOT write `&config_path`
  (needless `&&Path`).
- `default_config` is a `String` — pass `&default_config` (`&String` → `&str`).

### Edit 2 — `create_default_rules`, line 334
```rust
// BEFORE (current, line 333-334):

    fs::write(rules_path, render_rules_body())?;

// AFTER:

    atomic_write(rules_path, &render_rules_body())?;
```
- `rules_path` is `&Path` — pass directly. `render_rules_body()` returns `String`
  → `&render_rules_body()`.

### Success Criteria
- [ ] Line 218 calls `atomic_write(config_path, &default_config)?;` (was `fs::write(...)`).
- [ ] Line 334 calls `atomic_write(rules_path, &render_rules_body())?;` (was `fs::write(...)`).
- [ ] Final file content unchanged (same `content` arg flows through `atomic_write`).
- [ ] No new `use` lines added (`Path`, `fs`, `Error` all already imported; `atomic_write` is in-scope unqualified in the same module).
- [ ] No-op-if-exists guards, `create_dir_all`, and the `println!` user messages untouched.
- [ ] `cargo build --bin qmkonnect` compiles, no new warnings.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` fully green (existing count; no tests added/removed).
- [ ] `git diff --stat` shows ONLY `src/core/mod.rs`.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can make both edits using only the exact
anchors above (grep-confirmed unique), the verified return-type/`?` propagation,
the in-scope-no-qualification note, and the verified `cargo test ... --test-threads=1`
gate — all of which are in this PRP.

### Documentation & References

```yaml
# MUST READ — the dependency (CONTRACT) whose output this task consumes
- file: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/P1M2T1S1/PRP.md
  why: "defines atomic_write's exact signature, placement, and semantics — this task
        is a pure consumer of it"
  critical: "atomic_write(path: &Path, content: &str) -> Result<(), Box<dyn Error>>; it is
        a pub fn in core (same module as both call sites) => called UNQUALIFIED; it is a
        drop-in for fs::write(path, content)? so the `?` propagates unchanged. `use std::path::Path;`
        is ALREADY imported (mod.rs:8) — do NOT re-add."

# MUST READ — PRD / bug-hunt context (the "why")
- url: spec/PRD.md (heading h2.1, finding #1 "Non-atomic config/rules file writes")
  why: the exact defect this fixes (truncate-then-write read race; graceful but spurious behavior)
  critical: "impact is LOW (readers degrade to defaults); the win is eliminating the brief
        'valid TOML with wrong values' window. Do NOT over-engineer — this task is two one-line swaps."
- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/architecture/config_writes_research.md
  why: "catalogues all 5 non-atomic call sites; confirms mod.rs:218 (create_default_config) and
        mod.rs:334 (create_default_rules) are the 2 sites in scope here; the other 3 are tray.rs:878,
        tray.rs:1276, linux_tray.rs:822 (=> P1.M2.T2.S2)"
  section: "1. All write call sites" (tables A & B)

# MUST READ — the file being edited (exact current state, verified this session)
- file: src/core/mod.rs
  why: both functions + the exact edit anchors + the imports + the existing tests
  pattern: "L8 use std::path::Path;  (already imported — no new use). create_default_config fn @ 203,
           fs::write @ 218. create_default_rules fn @ 323, fs::write @ 334. #[cfg(test)] mod tests
           with test_create_default_rules_noop_if_exists @ 512 and test_create_default_rules_writes_when_absent @ 530."
  gotcha: "the no-op-if-exists guard (if path.exists() return Ok(())) and the create_dir_all(parent)
           both run BEFORE the migrated line and must NOT be touched. Pass config_path/rules_path
           directly (they are &Path), NOT &config_path (would be &&Path)."

# REFERENCE — atomic_write's implementation (lands in P1.M2.T1.S1; do NOT redefine)
- file: src/core/mod.rs (P1.M2.T1.S1 adds atomic_write right after render_config_body, ~L201)
  why: "confirms atomic_write stages `.{file_name}.tmp` in path.parent() then fs::rename over the
        target — same content, atomic to a concurrent reader"
  critical: "atomic_write ALREADY cleans up its temp on error; create_default_config/_rules do NOT
             need any temp cleanup of their own — the single `?` is sufficient."

# REFERENCE — the tests that gate this migration
- file: src/core/mod.rs:512 (test_create_default_rules_noop_if_exists) and :530 (test_create_default_rules_writes_when_absent)
  why: "the only existing tests that reach the migrated write path (site 2). They assert byte-identical
        content, so they pass unchanged after the swap. create_default_config (site 1) has NO dedicated
        write-path test (only render_default_config_template_round_trips_to_defaults @ 409, which tests
        the renderer, not the write)."
  critical: "Do NOT add new tests — the contract asks only that existing tests pass. atomic_write's own
             unit tests land in P1.M2.T1.S1."
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
src/core/mod.rs        # EDIT: line 218 (create_default_config) + line 334 (create_default_rules)
  - L8 use std::path::Path;                 (already imported — no new use)
  - L157-200 render_config_body              <- atomic_write lands right after (P1.M2.T1.S1)
  - L203-228 create_default_config           <- fs::write @ L218 (EDIT)
  - L254-322 render_rules_body
  - L323-342 create_default_rules            <- fs::write @ L334 (EDIT)
  - L371+ #[cfg(test)] mod tests
       L409 render_default_config_template_round_trips_to_defaults (renderer only)
       L512 test_create_default_rules_noop_if_exists        (no-op guard)
       L530 test_create_default_rules_writes_when_absent    (EXERCISES migrated L334)
src/tray.rs            # DO NOT TOUCH (P1.M2.T2.S2: lines 878, 1276)
src/linux_tray.rs      # DO NOT TOUCH (P1.M2.T2.S2: line 822)
Cargo.toml             # DO NOT TOUCH
```

### Desired Codebase tree
**Only `src/core/mod.rs` changes** (two one-line edits). No new files, no new
modules, no Cargo.toml edit, no new tests.

### Known Gotchas of our codebase & Library Quirks
```rust
// CRITICAL: `atomic_write` lands in the SAME module (core) via P1.M2.T1.S1, so it is
//   in scope at both call sites WITHOUT qualification (no `crate::core::` prefix,
//   no `use` import). `use std::path::Path;` is already at mod.rs:8 — do NOT re-add it.

// CRITICAL: pass config_path / rules_path DIRECTLY. They are already `&Path` (the
//   function params). Writing `&config_path` yields `&&Path`, which compiles via
//   auto-deref but is needlessly indirect. The work-item contract text shows
//   `atomic_write(&config_path, …)` — that leading `&` is a minor inaccuracy (the
//   actual current code is `fs::write(config_path, …)`, no `&`). Both compile; the
//   clean direct form is preferred.

// CRITICAL: this is a NO-BEHAVIOR-CHANGE migration. The content passed to
//   atomic_write is byte-identical to what fs::write received; the return type
//   (Result<(), Box<dyn Error>>) and the `?` propagation are identical. The ONLY
//   difference is the write mechanism (truncate-then-write → temp+rename). Do not
//   "improve" the content, the messages, the error handling, or the guards.

// GOTCHA: the no-op-if-exists guard (`if path.exists() { … return Ok(()); }`) and
//   `fs::create_dir_all(parent)` both run BEFORE the migrated line. They MUST stay
//   untouched — atomic_write assumes the parent dir exists (create_default_config/
//   _rules guarantee it via create_dir_all above). Do not move or duplicate them.

// GOTCHA: `create_default_config` (site 1) has NO dedicated write-path test — its
//   regression proof is `cargo build` + the full `--test-threads=1` suite staying
//   green. Do NOT add a test for it (scope creep); the contract asks only that
//   existing tests pass.

// GOTCHA: tests MUST run single-threaded: `cargo test --bin qmkonnect -- --test-threads=1`
//   (AGENTS.md) — the suite shares global debouncer state. Parallel runs flap.
```

## Implementation Blueprint

### Data models and structure
None. `Config` / `RuleSet` are unchanged; the inputs to both call sites (`&Path`
+ `String` content) are already computed above each line and flow unchanged into
`atomic_write`.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 0: CONFIRM the dependency is present (P1.M2.T1.S1 landed atomic_write)
  - RUN: grep -n 'pub fn atomic_write' src/core/mod.rs
  - EXPECT: one match around line ~201 (right after render_config_body).
  - IF ABSENT: P1.M2.T1.S1 has not landed yet. This task depends on it — do NOT
    proceed until atomic_write exists (it is the parallel, in-flight prerequisite).
    The expected signature is:
      pub fn atomic_write(path: &Path, content: &str) -> Result<(), Box<dyn Error>>

Task 1: EDIT src/core/mod.rs — swap create_default_config's write (line 218)
  - OLD (exact, grep-confirmed unique): "    fs::write(config_path, default_config)?;"
  - NEW: "    atomic_write(config_path, &default_config)?;"
  - NOTE: config_path is &Path (pass directly, NOT &config_path); default_config is
    String (pass &default_config).
  - PRESERVE: the "// Write the config file" comment above it, the no-op guard
    above, create_dir_all above, and all println! messages below.

Task 2: EDIT src/core/mod.rs — swap create_default_rules's write (line 334)
  - OLD (exact, grep-confirmed unique): "    fs::write(rules_path, render_rules_body())?;"
  - NEW: "    atomic_write(rules_path, &render_rules_body())?;"
  - NOTE: rules_path is &Path (pass directly); render_rules_body() returns String
    (pass &render_rules_body()).
  - PRESERVE: the no-op guard above, create_dir_all above, and all println! below.

Task 3: VALIDATE (no edits)
  - cargo build --bin qmkonnect          # compiles; no new warnings
  - cargo test --bin qmkonnect -- --test-threads=1   # full suite green; existing count
  - cargo test --bin qmkonnect test_create_default_rules -- --test-threads=1  # the 2 gating tests
  - git diff --stat                       # ONLY src/core/mod.rs
  - grep -n 'fs::write' src/core/mod.rs   # the two seeder sites are GONE from production;
                                           # remaining fs::write matches are only inside
                                           # #[cfg(test)] mod tests (fixtures) — expected

Task 4: NEVER do these (out of scope / forbidden)
  - DO NOT migrate tray.rs:878, tray.rs:1276, or linux_tray.rs:822 — those are P1.M2.T2.S2.
  - DO NOT add new tests (contract: existing tests must pass; atomic_write's own tests are P1.M2.T1.S1).
  - DO NOT add imports (Path/fs/Error already imported; atomic_write is same-module, in scope).
  - DO NOT touch the no-op guards, create_dir_all, or the println! messages.
  - DO NOT edit Cargo.toml, PRD.md, tasks.json, prd_snapshot.md, or any other source file.
```

### Implementation Patterns & Key Details
```rust
// PATTERN: atomic_write is a drop-in for fs::write(path, content)?. Same param
//   shapes (&Path, &str), same return type (Result<(), Box<dyn Error>>). The `?`
//   propagates unchanged. The content is byte-identical. Only the mechanism changes.
//
//   // create_default_config (mod.rs:218)
//   atomic_write(config_path, &default_config)?;
//
//   // create_default_rules (mod.rs:334)
//   atomic_write(rules_path, &render_rules_body())?;
//
// WHY this is safe: both call sites already guarantee the parent dir exists (the
//   `fs::create_dir_all(parent)?;` a few lines above). atomic_write's temp lives in
//   path.parent() (same fs) so rename is atomic. atomic_write cleans up its own temp
//   on error, so the single `?` here is the complete error path — no extra cleanup.
//
// ANTI-PATTERN: do NOT pass &config_path (=> &&Path). config_path is already &Path.
//   Pass it directly. (The contract's `&config_path` is a minor inaccuracy; both
//   compile but the direct form is idiomatic.)
//
// ANTI-PATTERN: do NOT "improve" anything else — content, messages, guards, ordering.
//   This is a pure mechanism swap with zero behavior delta.
```

### Integration Points
```yaml
IMPORTS: none new. (use std::path::Path at mod.rs:8; use std::fs; use std::error::Error — all present)
DEPENDENCY: P1.M2.T1.S1 — pub fn atomic_write must be present in src/core/mod.rs (Task 0 confirms).
CARGO:   none. No Cargo.toml change.
PARALLEL (no conflict):
  - P1.M2.T1.S1 adds atomic_write to src/core/mod.rs (different region: ~L201, between
    render_config_body and create_default_config). This task edits lines 218 and 334.
    Both compose cleanly once P1.M2.T1.S1 lands.
DOWNSTREAM (sibling, NOT this task):
  - P1.M2.T2.S2 migrates the 3 tray-dialog call sites (tray.rs:878/:1276, linux_tray.rs:822)
    via the same one-line swap (atomic_write imported as crate::core::atomic_write there,
    since it is a different module).
```

## Validation Loop

> Toolchain: Rust (`cargo`). Project has no ruff/mypy. `cargo build` + `cargo test`
> are the gates. Tests MUST run single-threaded (AGENTS.md — shared debouncer state).

### Level 1: Syntax & Style (Immediate Feedback)
```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles with zero new warnings. If atomic_write is "not found", P1.M2.T1.S1
# has not landed — see Task 0.
```

### Level 2: The gating unit tests (Component Validation)
```bash
cd /home/dustin/projects/qmkonnect
# The two tests that reach the migrated write path (site 2) / no-op guard:
cargo test --bin qmkonnect test_create_default_rules -- --test-threads=1
# Expected: 2 passed, 0 failed:
#   test_create_default_rules_noop_if_exists   (exercises no-op guard; doesn't hit the write)
#   test_create_default_rules_writes_when_absent  (EXERCISES migrated line 334 + idempotency)
```

### Level 3: Full Suite (Regression — AGENTS.md mandates single-threaded)
```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL existing tests still pass (incl. render_config_body/render_rules_body/
#           create_default_* paths). Net test count UNCHANGED (no tests added or removed).
#           --test-threads=1 is REQUIRED (shared global debouncer state; parallel runs flap).
```

### Level 4: Scope/Build Hygiene
```bash
cd /home/dustin/projects/qmkonnect
git diff --stat                 # Expected: ONLY src/core/mod.rs.
git diff Cargo.toml             # Expected: empty.
# The two seeder fs::write sites are gone from production code; remaining fs::write
# matches are only test fixtures inside #[cfg(test)]:
grep -n 'fs::write' src/core/mod.rs
# Expected: no matches at line 218 or 334; remaining matches are inside mod tests.
# (And the 3 tray-dialog sites in tray.rs / linux_tray.rs are UNCHANGED — that's P1.M2.T2.S2.)
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` succeeds with no new warnings.
- [ ] `cargo test --bin qmkonnect test_create_default_rules -- --test-threads=1` → 2 passed, 0 failed.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` → full suite green, test count unchanged.
- [ ] `git diff --stat` shows ONLY `src/core/mod.rs`.

### Feature Validation
- [ ] Line 218 calls `atomic_write(config_path, &default_config)?;` (final content byte-identical).
- [ ] Line 334 calls `atomic_write(rules_path, &render_rules_body())?;` (final content byte-identical).
- [ ] No-op-if-exists guards still short-circuit before the write (untouched).
- [ ] `create_dir_all(parent)` still runs before the write (untouched).
- [ ] No new `use` lines; `atomic_write` called unqualified (same module).

### Code Quality Validation
- [ ] Both edits pass the path directly (`&Path`, not `&&Path`).
- [ ] No new tests added (contract: existing tests pass; helper tests are P1.M2.T1.S1).
- [ ] No behavior change — content, messages, guards, error handling all identical.
- [ ] The 3 tray-dialog call sites in `tray.rs` / `linux_tray.rs` are UNCHANGED (P1.M2.T2.S2).

### Documentation & Deployment
- [ ] No user-facing / config / API surface change (internal mechanism swap — DOCS: none per contract).
- [ ] No new env vars / config keys / CLI flags.

---

## Anti-Patterns to Avoid
- ❌ Don't migrate the 3 tray-dialog call sites (`tray.rs:878/:1276`, `linux_tray.rs:822`) — that's P1.M2.T2.S2.
- ❌ Don't pass `&config_path` / `&rules_path` (yields `&&Path`); they're already `&Path` — pass directly.
- ❌ Don't add new tests — the contract asks only that existing tests pass; `atomic_write`'s own unit tests are P1.M2.T1.S1.
- ❌ Don't add imports — `Path`/`fs`/`Error` are already imported; `atomic_write` is same-module, in scope unqualified.
- ❌ Don't touch the no-op guards, `create_dir_all`, the `println!` messages, or the content strings.
- ❌ Don't run tests without `--test-threads=1` (AGENTS.md — shared debouncer state).
- ❌ Don't edit `Cargo.toml`, `PRD.md`, `tasks.json`, or any file other than `src/core/mod.rs`.
- ❌ Don't "improve" anything — this is a pure mechanism swap with zero behavior delta.

---

## Confidence Score: 9/10

The task is two mechanical one-line swaps with an exact, grep-confirmed anchor for
each, a drop-in dependency signature (verified against the P1.M2.T1.S1 contract),
zero behavior change, and an existing test (`test_create_default_rules_writes_when_absent`)
that already exercises the migrated line-334 write path. The 1-point reservation is
for the (small) risk that P1.M2.T1.S1 has not landed yet when implementation starts
— mitigated by Task 0's explicit presence check and the hard dependency declaration.
The dominant risk this PRP neutralizes is scope creep (touching the 3 tray-dialog
sites, adding tests, or "improving" the content) — all explicitly fenced off.
# PRP — P1.M2.T1.S1: Add `atomic_write()` to `src/core/mod.rs`

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`) — Rust menu-bar/tray daemon.
> **One file edited:** `src/core/mod.rs` (add the public helper + 3 unit tests).
> **Scope:** the cross-platform **`atomic_write` helper only**. Migrating the 5 call sites
> is the **next** subtask (P1.M2.T2.S1/S2) — do NOT touch them here.
> **Parallel context:** P1.M1.T1.S1 (mutex poison recovery) edits `src/core/notifier.rs` —
> a **different file**. No overlap; the two edits compose cleanly.

---

## Goal

**Feature Goal**: Add a single, reusable, cross-platform `atomic_write` helper to
`src/core/mod.rs` that writes `config.toml`/`rules.toml` (and any future file) via
**temp-file-in-same-directory + `rename`**, so a concurrent reader on the notifier thread
(`parse_config` / `parse_rules`, run per window-change with no locking) can **never observe a
truncated or partial file** during a save. This is the bug-hunt report's lower-severity
finding #1 ("Non-atomic config/rules file writes") and the shared foundation for P1.M2.T2.

**Deliverable**: a `pub fn atomic_write(path: &Path, content: &str) -> Result<(), Box<dyn Error>>`
in `src/core/mod.rs` (placed next to `render_config_body`, ~line 201), implemented with
**`std::fs` only** (NO `tempfile` crate added to `[dependencies]`), plus 3 unit tests in the
existing `mod tests` block. The signature is a drop-in replacement for `fs::write(path, content)?`,
so P1.M2.T2 can migrate each call site with a one-line change.

**Success Definition**:
- `atomic_write` writes `content` to `path` such that the final file content is exactly `content`,
  and at no point does `path` exist in a truncated/partial state (rename is the only mutation of
  `path`; the body is staged in a sibling `.…tmp` file in the **same directory**).
- On any error after the temp file is created, the helper best-effort removes the temp (no `.tmp`
  lingers).
- The 3 tests pass: creates-correct-content, replaces-existing, cleans-up-temp-on-error.
- `cargo test --bin qmkonnect -- --test-threads=1` is green (all existing + 3 new).
- `cargo build` succeeds with `std::fs` only — `Cargo.toml` `[dependencies]` is **unchanged**
  (`tempfile` stays a `[dev-dependencies]` entry, used by the tests).

## User Persona (if applicable)

**Target User**: (1) The **QMKonnect daemon's notifier thread** — the concurrent reader that
re-parses `config.toml`/`rules.toml` on every window change. (2) The **P1.M2.T2 implementer**
who will swap `fs::write` → `atomic_write` at the 5 call sites. (3) End users, who currently
(rarely, transiently) see a spurious "rules.toml invalid" notification or a brief device-filter
mismatch if a save races a read.

**Use Case**: User edits a setting in the Windows/macOS/Linux Settings dialog → the tray thread
calls `atomic_write(config_path, &render_config_body(...))` → the notifier thread's next
`configured_filter`/`parse_config` read sees either the *old* or the *new* complete file —
**never** a half-written one.

**User Journey**: dialog OK → `render_config_body` (pure `String`) → `atomic_write(path, &body)`
→ writes `.{name}.tmp` in `path`'s dir → `fs::rename(tmp, path)` (atomic) → readers see new
content on next read. On failure → temp removed, original file untouched.

**Pain Points Addressed**: closes the truncate-then-write read race (bug-hunt finding #1). The
impact today is low (readers degrade gracefully to defaults + a one-time notification), but a
partial write that parses as *valid TOML with wrong values* could briefly persist — atomic
replace eliminates that window entirely.

## Why

- **Foundation for the whole P1.M2 milestone.** P1.M2.T2.S1/S2 migrate 5 call sites
  (`mod.rs:218`, `mod.rs:334`, `tray.rs:878`, `tray.rs:1276`, `linux_tray.rs:822`) — every one
  of them needs this helper to exist first. Building + testing the helper in isolation (this
  subtask) makes the 5 trivial, mechanical migrations safe to review independently.
- **Copies a proven codebase pattern, adapted for cross-platform + std-only.** The only existing
  atomic writer is `write_rule_atomic` (`src/platforms/linux.rs:336`) — but it's **Linux-only**
  (targets `/etc/udev/rules.d`, uses the `tempfile` crate for `PermissionDenied` handling + `persist`).
  Config/rules files live in a per-user config dir the process already owns (created via
  `fs::create_dir_all`), so there are **no permission issues** and **no need for the `tempfile`
  crate** — `std::fs::write` + `std::fs::rename` is sufficient and works on Windows/macOS/Linux.
- **`rename(2)` is atomic only within the same filesystem** — hence the temp MUST live in the
  target's parent directory (same filesystem), which is exactly what the implementation does.
- **Minimal, surgical, no-API-surface change.** Adding a `pub` helper inside the `core` module
  adds no new dependency, no config key, no CLI flag, no user-visible behavior by itself.

## What

Add to `src/core/mod.rs` (between `render_config_body`'s closing brace ~line 200 and the
`create_default_config` doc comment at line 203):

```rust
/// Atomically write `content` to `path` via a temp file in `path`'s parent directory
/// followed by `fs::rename`, so a concurrent reader (e.g. `parse_config` / `parse_rules`
/// on the notifier thread) can never observe a truncated or partial file.
///
/// Uses ONLY `std::fs` (no `tempfile` crate): the temp (`.{file_name}.tmp`) lives in the
/// SAME directory as `path`, so `rename` is atomic (same filesystem). Config/rules files
/// are in a per-user dir the process already owns, so there are no permission concerns
/// (unlike `write_rule_atomic`, which targets `/etc/udev/rules.d` and needs `tempfile`).
///
/// On any error after the temp file is created, the temp is removed best-effort. If
/// `fs::write` itself fails, no temp exists and the cleanup is a harmless no-op.
///
/// Signature is a drop-in for `fs::write(path, content)?`.
pub fn atomic_write(path: &Path, content: &str) -> Result<(), Box<dyn Error>> {
    let parent = path.parent().unwrap_or(Path::new("."));
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("atomic_write: path has no file name: {}", path.display()))?;
    // Same parent dir as target => same filesystem => rename is atomic. Leading dot hides
    // the temp on Unix; the name is unique per target within its directory.
    let tmp = parent.join(format!(".{}.tmp", file_name.to_string_lossy()));

    // Stage the body in the temp, then atomically rename it over the target.
    let result = (|| -> Result<(), Box<dyn Error>> {
        fs::write(&tmp, content)?;
        fs::rename(&tmp, path)?;
        Ok(())
    })();

    // If anything failed after the temp was created, remove it (best-effort). A bare `?`
    // would short-circuit past this cleanup, hence the captured-result guard.
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}
```

### Success Criteria
- [ ] `atomic_write(path, content)` produces a file at `path` whose content equals `content`.
- [ ] The temp file `.{file_name}.tmp` is created in `path.parent()` (same dir → same fs →
      atomic rename) and is NOT left behind on success.
- [ ] On error after temp creation, the temp is removed (best-effort) — no `.tmp` lingers.
- [ ] `path` is mutated only by `fs::rename` (never truncated in place) → readers see old-or-new,
      never partial.
- [ ] Implementation uses `std::fs` only; `Cargo.toml` `[dependencies]` unchanged.
- [ ] The 3 unit tests pass; the full suite stays green under `--test-threads=1`.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can add the function + tests using only the exact
placement, the copy-paste implementation above, the verified test designs, and the verified
`cargo test --bin qmkonnect ... --test-threads=1` gate — all of which are in this PRP.

### Documentation & References

```yaml
# MUST READ — PRD / bug-hunt context (the "why")
- url: spec/PRD.md (heading h2.1, finding #1 "Non-atomic config/rules file writes")
  why: the exact defect this fixes (truncate-then-write read race; graceful but spurious behavior)
  critical: impact is LOW (readers degrade to defaults); the win is eliminating the brief
            "valid TOML with wrong values" window — do not over-engineer (no fsync, no locking)
- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/architecture/config_writes_research.md
  why: confirms all 5 non-atomic call sites + the one atomic precedent; severity Low, self-healing
  section: "1. All write call sites" (table) + "C. The ONE atomic helper" + "2. render_config_body"

# MUST READ — the file being edited
- file: src/core/mod.rs
  why: imports already present (no new `use`); placement point; existing test conventions
  pattern: "lines 6-10 imports (std::error::Error, std::fs, std::path::Path); render_config_body
           at :157-200; create_default_config at :203; #[cfg(test)] mod tests at :371 with
           `use super::*` and `tempfile::TempDir::new()` fixtures at :515/:533"
  gotcha: do NOT add `use std::fs` etc. — they are ALREADY imported. Do NOT touch create_default_config
          (:203, uses fs::write at :218) or create_default_rules (:323, fs::write at :334) — those
          are P1.M2.T2.S1 migrations, out of scope here.

# MUST READ — the only atomic-write precedent (pattern to adapt, NOT copy)
- file: src/platforms/linux.rs
  why: write_rule_atomic (:336) — same temp-in-same-dir + rename idea, but Linux-only + tempfile crate
  pattern: "tempfile::NamedTempFile::new_in(dir) → write_all → sync_all → persist(path)"
  gotcha: do NOT copy sync_all/fsync (config/rules don't need crash-durability) and do NOT import the
          tempfile crate into production — std::fs only. write_rule_atomic targets /etc/udev/rules.d
          (needs PermissionDenied handling); config/rules are in a user-owned dir (no perm issues).

# REFERENCE — the 5 call sites this helper enables (migrated in P1.M2.T2 — DO NOT touch here)
- file: src/core/mod.rs:218 (create_default_config fs::write) and :334 (create_default_rules fs::write)
- file: src/tray.rs:878 (Windows dialog) and :1276 (macOS dialog) — std::fs::write(config_path, …)?
- file: src/linux_tray.rs:822 (write_config) — std::fs::write(&path, content)?

# REFERENCE — Cargo deps
- file: Cargo.toml
  why: confirm `tempfile = "3.0"` is in [dev-dependencies] (line 31) — available for tests,
       must NOT be added to [dependencies]
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
src/core/mod.rs        # EDIT: add atomic_write (~L201) + 3 tests (in mod tests at L371)
  - imports L6-10: std::error::Error, std::fs, std::path::Path  (no new imports needed)
  - render_config_body L157-200   <- atomic_write goes right after this
  - create_default_config L203    (fs::write at L218 — P1.M2.T2, do not touch)
  - create_default_rules  L323    (fs::write at L334 — P1.M2.T2, do not touch)
  - #[cfg(test)] mod tests L371   (use super::*; TempDir fixtures at L515/L533)
src/platforms/linux.rs # READ ONLY: write_rule_atomic L336 (the precedent to adapt)
Cargo.toml             # READ ONLY: tempfile is [dev-dependencies] L31 — no edit
```

### Desired Codebase tree
**Only `src/core/mod.rs` changes** (one new `pub fn` + 3 `#[test]` fn's). No new files, no new
modules, no Cargo.toml edit.

### Known Gotchas of our codebase & Library Quirks
```rust
// CRITICAL: `rename(2)` is atomic ONLY within the same filesystem. The temp file MUST be in
// path.parent() (same dir as the target) — NOT in /tmp or std::env::temp_dir() (which may be
// a different mount → rename returns EXDEV and is NOT atomic). The implementation uses
// path.parent().unwrap_or(Path::new(".")).

// CRITICAL: do NOT use the bare `?` operator across write+rename if you need cleanup — `?`
// short-circuits PAST the remove_file. The implementation captures the result of an inner
// closure and cleans up when it's Err. (Verified: on rename failure the temp genuinely
// remains until explicitly removed.)

// GOTCHA: a deterministic temp name (.{file_name}.tmp) means two CONCURRENT atomic_writes to
// the SAME path would race on the temp. All 5 call sites are on the UI/tray thread (serial,
// distinct files), so this is not a practical concern and is explicitly out of scope. The
// concurrent party here is the READER (notifier thread), and atomic replace fully protects it.
// Do NOT "fix" this with the tempfile crate or a random suffix — the contract forbids the crate
// and prescribes the deterministic name.

// GOTCHA: config/rules writes do NOT need fsync/crash-durability — the defect is the *read
// race*, not power loss. write_rule_atomic calls sync_all because udev rules must survive
// crashes; config/rules do not have that requirement. Keep it to write+rename only.

// GOTCHA: tests MUST run single-threaded: `cargo test --bin qmkonnect -- --test-threads=1`
// (AGENTS.md) — the suite shares global debouncer state. Parallel runs flap.

// GOTCHA: `tempfile` (the crate) is already a [dev-dependencies] entry — use
// tempfile::TempDir::new() in the tests; do NOT add it to [dependencies].
```

## Implementation Blueprint

### Data models and structure
No data models — this is a single free function. (`Config`/`RuleSet` structs are unchanged; the
inputs are a `&Path` and a `&str` already produced by `render_config_body`/`render_rules_body`.)

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: ADD `atomic_write` to src/core/mod.rs
  - IMPLEMENT: pub fn atomic_write(path: &Path, content: &str) -> Result<(), Box<dyn Error>>
    body: derive parent = path.parent().unwrap_or(Path::new(".")); file_name = path.file_name()
    (Err if None); tmp = parent.join(format!(".{}.tmp", file_name.to_string_lossy()));
    inner closure { fs::write(&tmp, content)?; fs::rename(&tmp, path)?; Ok(()) }; on Err,
    `let _ = fs::remove_file(&tmp);`; return result.
  - FOLLOW pattern: the std-only adaptation described above (NOT write_rule_atomic, which needs tempfile)
  - NAMING: atomic_write (snake_case, pub); param names path/content match fs::write for drop-in feel
  - PLACEMENT: src/core/mod.rs, immediately AFTER render_config_body's closing brace (~L201),
    BEFORE the create_default_config doc comment (~L203). Add a `///` doc comment (see "What").
  - IMPORTS: NONE new — std::error::Error, std::fs, std::path::Path already imported at L6-8.
  - DEPENDENCIES: std::fs only. Do NOT add the tempfile crate to [dependencies].

Task 2: ADD 3 unit tests to the existing `mod tests` (src/core/mod.rs:371+)
  - tests use `use super::*` (already present) → atomic_write, Path, fs in scope; use
    `tempfile::TempDir::new().unwrap()` for fixtures (already the convention at L515/L533).
  - NAMING: test_atomic_write_creates_correct_content / _replaces_existing / _cleans_up_temp_on_error
  - COVERAGE: happy-path create, overwrite-existing, error-cleanup. All read back via
    std::fs::read_to_string and assert_eq!.

  Task 2a: test_atomic_write_creates_correct_content
    - dir = TempDir::new(); path = dir.path().join("config.toml")
    - atomic_write(&path, "vendor_id = 0xfeed\n").unwrap()
    - assert_eq!(std::fs::read_to_string(&path).unwrap(), "vendor_id = 0xfeed\n")
    - assert no file in dir.path() whose name ends with ".tmp" (temp must not linger on success)

  Task 2b: test_atomic_write_replaces_existing
    - dir = TempDir::new(); path = dir.path().join("config.toml")
    - std::fs::write(&path, "# STALE content\n").unwrap()   // pre-existing stale file
    - atomic_write(&path, "poll_interval_ms = 250\n").unwrap()
    - assert_eq!(std::fs::read_to_string(&path).unwrap(), "poll_interval_ms = 250\n")
    - assert the stale content is fully gone (no concatenation/append)

  Task 2c: test_atomic_write_cleans_up_temp_on_error
    - dir = TempDir::new(); target = dir.path().join("config.toml")
    - std::fs::create_dir(&target).unwrap()   // target is a DIRECTORY, not a file
    - result = atomic_write(&target, "body")
    - assert!(result.is_err(), "rename of a temp file over a directory must fail (EISDIR)")
    - enumerate dir.path(): assert NO entry whose file_name ends with ".tmp" remains
      (the cleanup branch removed the staged temp; verified that rename(file,dir) fails AND
       leaves the temp behind until explicitly removed, so this genuinely exercises cleanup)
    - NOTE: this is the verified-reliable approach. (Alt if rename-over-dir behaves oddly on a
      given CI: a path containing a NUL byte, e.g. dir.path().join("bad\u{0}name"), which fails
      at fs::write before temp creation — still asserts no .tmp lingers, but exercises cleanup
      less directly. Prefer the directory-as-target form.)

Task 3: VALIDATE (no edits)
  - cargo test --bin qmkonnect atomic_write -- --test-threads=1   # the 3 new tests pass
  - cargo test --bin qmkonnect -- --test-threads=1                # full suite green
  - cargo build                                                   # compiles; no new dep
  - git diff --stat  → only src/core/mod.rs changed (no Cargo.toml)

Task 4: NEVER do these (out of scope / forbidden)
  - DO NOT migrate the 5 fs::write call sites — that is P1.M2.T2.S1/S2.
  - DO NOT add the `tempfile` crate to [dependencies] (it stays [dev-dependencies] for tests).
  - DO NOT add fsync/sync_all (config/rules don't need crash-durability).
  - DO NOT edit PRD.md, any tasks.json, prd_snapshot.md, Cargo.toml, or other source files.
```

### Implementation Patterns & Key Details
```rust
// The captured-result guard idiom (do NOT use bare `?` here — it skips cleanup):
let result = (|| -> Result<(), Box<dyn Error>> {
    fs::write(&tmp, content)?;   // temp created here
    fs::rename(&tmp, path)?;     // atomic replace; if this fails, temp still exists
    Ok(())
})();
if result.is_err() {
    let _ = fs::remove_file(&tmp);   // best-effort; no-op if write never created it
}
result

// Why same-directory temp (NOT /tmp):
//   fs::rename is atomic only within ONE filesystem. /tmp may be tmpfs (different mount).
//   path.parent() guarantees the same fs as path. This is the single most important invariant.
```

### Integration Points
```yaml
IMPORTS: none new. (std::error::Error / std::fs / std::path::Path at src/core/mod.rs:6-8)
CARGO:   none. `tempfile = "3.0"` already in [dev-dependencies] (Cargo.toml:31) — tests use it.
DOWNSTREAM (this helper unblocks — DO NOT implement here):
  - P1.M2.T2.S1: mod.rs:218 + :334  → fs::write(...)  becomes atomic_write(...)? 
  - P1.M2.T2.S2: tray.rs:878/:1276 + linux_tray.rs:822 → same one-line swap (import via crate::core::atomic_write)
PARALLEL (no conflict): P1.M1.T1.S1 edits src/core/notifier.rs (different file).
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)
```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect          # compiles; expect zero errors/warnings on mod.rs
# (Project has no ruff/mypy — it is Rust. `cargo build` + `cargo test` are the gates.
#  If the repo uses clippy in CI, also run: cargo clippy --bin qmkonnect -- -D warnings)
```

### Level 2: Unit Tests (Component Validation)
```bash
cd /home/dustin/projects/qmkonnect
# The 3 new tests in isolation:
cargo test --bin qmkonnect atomic_write -- --test-threads=1
# Expected: 3 passed, 0 failed (test_atomic_write_creates_correct_content,
#           test_atomic_write_replaces_existing, test_atomic_write_cleans_up_temp_on_error)
```

### Level 3: Full Suite (Regression — AGENTS.md mandates single-threaded)
```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL existing tests still pass (incl. render_config_body/render_rules_body/create_default_*)
#           + the 3 new atomic_write tests. Net: previous-count + 3. --test-threads=1 is REQUIRED
#           (shared global debouncer state; parallel runs flap).
```

### Level 4: Scope/Build Hygiene
```bash
cd /home/dustin/projects/qmkonnect
git diff --stat              # Expected: ONLY src/core/mod.rs. No Cargo.toml, no other src files.
git diff Cargo.toml          # Expected: empty (tempfile stays [dev-dependencies]; no new prod dep).
grep -n 'tempfile' src/core/mod.rs   # Expected: matches only INSIDE #[cfg(test)] mod tests (TempDir),
                                     # never in the atomic_write production body.
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` succeeds with no new warnings on `mod.rs`.
- [ ] `cargo test --bin qmkonnect atomic_write -- --test-threads=1` → 3 passed, 0 failed.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` → full suite green (existing + 3 new).
- [ ] `git diff --stat` shows ONLY `src/core/mod.rs`.

### Feature Validation
- [ ] `atomic_write` writes exact `content` to `path` (test 2a).
- [ ] `atomic_write` fully replaces pre-existing content (test 2b).
- [ ] On error, no `.tmp` file lingers (test 2c — directory-as-target, verified EISDIR).
- [ ] Temp lives in `path.parent()` (same fs) so `rename` is atomic.
- [ ] `path` is mutated only by `rename` (readers see old-or-new, never partial).

### Code Quality Validation
- [ ] Follows existing `mod.rs` conventions: `pub fn`, `Box<dyn Error>`, `///` doc comment.
- [ ] Placement is "next to `render_config_body`" (between it and `create_default_config`).
- [ ] No new `use` lines (imports already present); `std::fs` only in the production body.
- [ ] The `tempfile` crate is NOT added to `[dependencies]` (stays `[dev-dependencies]`).
- [ ] No call sites migrated (that is P1.M2.T2 — out of scope).

### Documentation & Deployment
- [ ] Doc comment on `atomic_write` explains the same-dir + rename rationale and the std-only choice.
- [ ] No user-facing / config / API surface change (internal helper — DOCS: none per contract).

---

## Anti-Patterns to Avoid
- ❌ Don't create the temp in `/tmp` or `std::env::temp_dir()` — rename would cross filesystems (EXDEV) and is NOT atomic. Use `path.parent()`.
- ❌ Don't use bare `?` across `write`+`rename` — it skips cleanup. Capture the result and clean up on Err.
- ❌ Don't add `fsync`/`sync_all` — config/rules don't need crash-durability (this fixes a *read* race, not power loss).
- ❌ Don't add the `tempfile` crate to production deps (the contract forbids it; config/rules are in a user-owned dir with no permission handling needed).
- ❌ Don't migrate the 5 `fs::write` call sites here — that's P1.M2.T2.S1/S2.
- ❌ Don't run tests without `--test-threads=1` (AGENTS.md — shared debouncer state).
- ❌ Don't edit Cargo.toml, PRD.md, tasks.json, or any file other than `src/core/mod.rs`.

---

## Confidence Score: 9/10

The task is small, self-contained, and precisely specified. Imports are already present; the
precedent (`write_rule_atomic`) is well understood and explicitly scoped away (std-only,
no permission handling); the test designs are verified reliable this session
(`rename(file → existing_dir)` returns EISDIR and leaves the temp in place, genuinely exercising
the cleanup branch). The 1-point reservation is for CI-specific rename-over-directory behavior
on non-Linux runners — mitigated by documenting the NUL-byte alternative for test (c).
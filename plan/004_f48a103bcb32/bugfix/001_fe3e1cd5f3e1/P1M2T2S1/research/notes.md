# Research Notes — P1.M2.T2.S1: Migrate create_default_config + create_default_rules to atomic_write

## Task in one line

Swap two `fs::write(...)?` call sites in `src/core/mod.rs` (the config & rules
seeders) for `atomic_write(...)?` (the helper built in parallel by P1.M2.T1.S1).
**No behavior change** — content and error handling identical; only the write
mechanism changes (truncate-then-write → temp-in-same-dir + rename).

## Repo under change

- **QMKonnect** Rust daemon, `/home/dustin/projects/qmkonnect`. The bug-hunt
  remediation plan (`plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/`).
- ONE file edited: `src/core/mod.rs`. Two one-line changes inside two existing
  functions.

## Dependency contract — P1.M2.T1.S1 (parallel, treated as a CONTRACT)

P1.M2.T1.S1 adds to `src/core/mod.rs` (placed right after `render_config_body`,
~line 201, before `create_default_config`):

```rust
pub fn atomic_write(path: &Path, content: &str) -> Result<(), Box<dyn Error>>
```

Key properties consumed by THIS task:
- **Drop-in for `fs::write(path, content)?`** — same param shapes (`&Path`, `&str`),
  same return type `Result<(), Box<dyn Error>>`. So `?` propagates unchanged.
- Uses `std::fs` only (no `tempfile` crate in production). Temp `.{name}.tmp`
  lives in `path.parent()` (same fs → atomic `rename`); cleans up the temp on error.
- `use std::path::Path;` is **already imported** at `src/core/mod.rs:8` — NO new
  import needed by this task (atomic_write lives in the SAME module, so it's in
  scope without qualification).
- atomic_write is `pub fn` in `core::` → called unqualified inside `mod.rs`.

## The two call sites (verified by read this session)

### Site 1 — create_default_config, src/core/mod.rs:218
```rust
pub fn create_default_config(config_path: &Path) -> Result<(), Box<dyn Error>> {
    if config_path.exists() { ... return Ok(()); }   // no-op guard (line ~205)
    if let Some(parent) = config_path.parent() { fs::create_dir_all(parent)?; }  // line ~213
    let default_config = render_default_config_template();   // line 215 (String)
    // Write the config file
    fs::write(config_path, default_config)?;                 // ← LINE 218, MIGRATE
    ...
}
```
- `config_path: &Path` (already the right type for atomic_write's first arg).
- `default_config: String` → pass `&default_config` (`&String` coerces to `&str`).

### Site 2 — create_default_rules, src/core/mod.rs:334
```rust
pub fn create_default_rules(rules_path: &Path) -> Result<(), Box<dyn Error>> {
    if rules_path.exists() { ... return Ok(()); }   // no-op guard (line ~327)
    if let Some(parent) = rules_path.parent() { fs::create_dir_all(parent)?; }
    fs::write(rules_path, render_rules_body())?;    // ← LINE 334, MIGRATE
    ...
}
```
- `rules_path: &Path`. `render_rules_body()` returns `String` → `&render_rules_body()`.

## Exact edits (oldText → newText), both unique in the file (grep-confirmed)

```
    fs::write(config_path, default_config)?;        →   atomic_write(config_path, &default_config)?;
    fs::write(rules_path, render_rules_body())?;    →   atomic_write(rules_path, &render_rules_body())?;
```

**Idiomatic form note:** pass `config_path` / `rules_path` directly (they are
already `&Path`); do NOT write `&config_path` (would be `&&Path`, which compiles
via auto-deref but is needlessly indirect). The work-item contract text wrote
`atomic_write(&config_path, &default_config)?` — that `&` on the path is a minor
contract inaccuracy (the current code at line 218 is `fs::write(config_path,
default_config)?`, no `&`); either compiles, but the clean form is preferred.

## Imports

- `use std::path::Path;` — **already present at line 8.** Do NOT re-add.
- `use std::fs;` — already present (used by `create_dir_all` above each site).
- `use std::error::Error;` — already present (the `Box<dyn Error>` return type).
- atomic_write is defined in the SAME module (`core`), so it is in scope
  unqualified at both call sites. No `crate::core::` prefix, no `use` needed.

## Behavior-change analysis (NONE — the point)

| Aspect | Before (`fs::write`) | After (`atomic_write`) | Changed? |
|---|---|---|---|
| Final file content | `default_config` / `render_rules_body()` | identical (same `content` arg) | No |
| Return type | `Result<(), Box<dyn Error>>` | identical | No |
| Error propagation | `?` | `?` (same) | No |
| No-op-if-exists guard | untouched (returns `Ok(())` before the write) | untouched | No |
| `create_dir_all(parent)` | untouched (runs before the write) | untouched | No |
| Write mechanism | truncate-then-write (`O_TRUNC`) | temp `.{name}.tmp` + `rename` | **Yes (the fix)** |
| Concurrent-reader visibility | could see truncated/partial mid-write | sees old-or-new, never partial | **Yes (the fix)** |

The `println!` user messages after each write are UNCHANGED.

## Tests that exercise the migrated paths (must keep passing)

Enumerated this session (`grep -n '#\[test\]' src/core/mod.rs`), the relevant ones:

- **`test_create_default_rules_noop_if_exists`** (mod.rs:512) — pre-creates a
  sentinel `rules.toml`, calls `create_default_rules`, asserts content UNCHANGED.
  Exercises the **no-op guard** (returns before reaching line 334), so it does
  NOT hit the migrated write — but it must still pass.
- **`test_create_default_rules_writes_when_absent`** (mod.rs:530) — calls
  `create_default_rules` on an absent nested path, asserts the file is created
  with `render_rules_body()` content, then asserts idempotency on re-call. This
  **DOES** reach the migrated line 334 (the absent-file branch). It is the
  primary regression proof for site 2. atomic_write produces byte-identical final
  content, so it passes unchanged. (It also re-creates a sentinel + re-calls,
  hitting the no-op guard — fine.)

- **`create_default_config` has NO dedicated write-path test.** Only
  `render_default_config_template_round_trips_to_defaults` (mod.rs:409) exists,
  and it tests the *renderer* (`render_default_config_template`), NOT the
  write. So site 1 (line 218) is exercised only by manual/CLI first-run seeding
  (`qmkonnect -c`) and the no-op guard. The work-item contract's mention of
  "test_config_default_generation" is **imprecise** — no such test exists; the
  real regression guard for site 2 is the two `test_create_default_rules_*` tests
  above, and for site 1 there is none beyond `cargo build` + the full suite
  staying green.

## Scope boundaries (do NOT do)

- Do NOT migrate the OTHER 3 call sites — `tray.rs:878`, `tray.rs:1276`,
  `linux_tray.rs:822` — those are **P1.M2.T2.S2**.
- Do NOT add new tests (the contract asks only that existing tests pass; the
  atomic_write helper itself is unit-tested in P1.M2.T1.S1).
- Do NOT add imports (all needed ones already present).
- Do NOT touch the no-op guards, `create_dir_all`, or the `println!` messages.
- Do NOT edit Cargo.toml, PRD.md, tasks.json, or any other file.

## Validation gate (AGENTS.md-mandated single-threaded)

```bash
cargo test --bin qmkonnect -- --test-threads=1
```
Must be fully green (existing count unchanged — no tests added). Specifically
`test_create_default_rules_noop_if_exists` and `test_create_default_rules_writes_when_absent`
must pass. `cargo build --bin qmkonnect` must compile with no new warnings.

## Conclusion

Two one-line, mechanical, risk-free swaps gated by an existing test that already
exercises site 2's write path. The dominant (small) risk is a typo in the edit
string; mitigated by the exact anchors and the `--test-threads=1` gate. No new
tests, no new imports, no behavior change.
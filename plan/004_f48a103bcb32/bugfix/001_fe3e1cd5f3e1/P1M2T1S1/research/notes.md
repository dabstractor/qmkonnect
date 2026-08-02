# Research Notes — P1.M2.T1.S1: Add `atomic_write()` to `src/core/mod.rs`

**Repo**: QMKonnect (`/home/dustin/projects/qmkonnect`) — Rust desktop app.
**Target file**: `src/core/mod.rs` (single file; function + tests only).
**Scope boundary**: this subtask adds ONLY the helper + its 3 tests. Migrating the 5 call
sites is **P1.M2.T2.S1/S2** (do NOT touch them here).

---

## Parallel-context check (no conflict)

P1.M1.T1.S1 (mutex poison recovery) is implemented in parallel in `src/core/notifier.rs` —
a **different file**. Its PRP adds 1 test to notifier.rs (344→345). This task adds 3 tests to
`mod.rs`. They compose: neither edits the other's file. Baseline = "all existing tests pass
+ 3 new atomic_write tests pass".

---

## Imports already present in `src/core/mod.rs` (lines 6-10) — NO new imports needed

```rust
use std::error::Error;   // for Box<dyn Error>
use std::fs;             // fs::write / fs::rename / fs::remove_file
use std::path::Path;     // &Path param + Path::new(".")
use std::sync::OnceLock;
use std::time::Instant;
```
➡️ `atomic_write` can use `fs::*`, `Path`, `Box<dyn Error>` with zero new `use` lines.

## Placement

"next to `render_config_body`" (`mod.rs:157-200`). `render_config_body` ends at `}` ~line 200;
`create_default_config` starts at line 203. Insert `atomic_write` **between** them (after the
`render_config_body` closing brace, before the `create_default_config` doc comment).

## Function signature (contract, verbatim)

```rust
pub fn atomic_write(path: &Path, content: &str) -> Result<(), Box<dyn Error>>
```
- `pub` so `tray.rs` / `linux_tray.rs` (P1.M2.T2) can import it via `crate::core::atomic_write`.
- Signature is drop-in compatible with `fs::write(path, content)?` (same `Result<(), Box<dyn Error>>`).
- **std::fs ONLY** — do NOT add the `tempfile` crate to `[dependencies]`. (`tempfile` IS a
  `[dev-dependencies]` entry at `Cargo.toml:31`/`:37` — used in tests only, already available.)

## Implementation design (verified semantics)

```rust
/// Atomically write `content` to `path` via temp-file-in-same-dir + rename, so a
/// concurrent reader (`parse_config`/`parse_rules` on the notifier thread) can never
/// observe a truncated/partial file. Uses ONLY std::fs (no tempfile crate).
pub fn atomic_write(path: &Path, content: &str) -> Result<(), Box<dyn Error>> {
    let parent = path.parent().unwrap_or(Path::new("."));
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("atomic_write: path has no file name: {}", path.display()))?;
    // Same PARENT dir as target => same filesystem => `rename(2)` is atomic.
    // Leading dot hides the temp on Unix; unique per target within its dir.
    let tmp = parent.join(format!(".{}.tmp", file_name.to_string_lossy()));

    // Write the body to the temp, then atomically rename over the target.
    let result = (|| -> Result<(), Box<dyn Error>> {
        fs::write(&tmp, content)?;
        fs::rename(&tmp, path)?;
        Ok(())
    })();

    // If anything failed AFTER the temp was created, remove it (best-effort).
    // If the temp was never created (fs::write failed), remove_file is a harmless no-op.
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}
```
- Temp name `.{file_name}.tmp` (e.g. `config.toml` → `.config.toml.tmp`). Contract example was
  `.{stem}.tmp`; using the full file_name avoids extension edge cases, stays same-dir/hidden/unique.
- **Why the closure guard, not bare `?`:** `?` short-circuits past cleanup. The inner closure
  captures the result so the outer guard can clean up on Err.
- **Known limitation (out of scope, documented):** the temp name is deterministic, so two
  concurrent `atomic_write`s to the *same* path would race on `.config.toml.tmp`. All 5 call
  sites are on the UI/tray thread (serial, distinct files), so this is not practical. The goal
  this subtask achieves — concurrent READERS never see a partial file — is fully met.

## Precedent (the codebase's only atomic writer — Linux-only, udev rules)

`src/platforms/linux.rs:336` `write_rule_atomic`: `tempfile::NamedTempFile::new_in(dir)` →
`write_all` → `sync_all` → `persist(path)` (rename). It targets `/etc/udev/rules.d` (needs the
crate for `PermissionDenied` handling + randomized names). `atomic_write` is the std-only,
config/rules analog. **Do NOT copy `sync_all`/`fsync`** — config/rules writes don't need
crash-durability fsync (the bug is the *read race*, not power-loss), and the contract says
std::fs only. (If crash-durability were ever wanted, `tmp.as_file().sync_all()` is the knob.)

## The 5 call sites (for P1.M2.T2 migration — DO NOT touch here)

| # | Site | Current (non-atomic) | Migration target |
|---|------|---------------------|------------------|
| 1 | `src/core/mod.rs:218` `create_default_config` | `fs::write(config_path, default_config)?` | `atomic_write(config_path, &default_config)?` |
| 2 | `src/core/mod.rs:334` `create_default_rules` | `fs::write(rules_path, render_rules_body())?` | `atomic_write(rules_path, &render_rules_body())?` |
| 3 | `src/tray.rs:878` `show_settings_dialog` (Windows) | `std::fs::write(config_path, config_content)?` | P1.M2.T2.S2 |
| 4 | `src/tray.rs:1276` `show_settings_dialog_with_pool` (macOS) | `std::fs::write(config_path, config_content)?` | P1.M2.T2.S2 |
| 5 | `src/linux_tray.rs:822` `write_config` (Linux) | `std::fs::write(&path, content)?` | P1.M2.T2.S2 |

## Test conventions (existing `mod tests` at `mod.rs:371`, `use super::*`)

- Tests use `#[test] fn snake_case_name()`, assert with `assert_eq!`/`assert!`, return `()`.
- Temp dirs via `tempfile::TempDir::new().unwrap()` (already at mod.rs:515, 533). Read-back with
  `std::fs::read_to_string`. `use super::*` brings `atomic_write`, `Path`, etc. into scope.
- 3 new tests (contract (a)(b)(c)):
  - `(a) test_atomic_write_creates_correct_content` — write known content, read back, compare.
  - `(b) test_atomic_write_replaces_existing` — pre-create stale content, atomic_write new, assert new.
  - `(c) test_atomic_write_cleans_up_temp_on_error` — **directory-as-target** (verified EISDIR):
    create the TARGET path as a directory so `fs::rename(tmp_file, dir)` fails AFTER the temp is
    written → the cleanup branch runs → assert no `.tmp` lingers in the dir. (Alt: NUL-byte path,
    also verified to fail — but directory-as-target genuinely exercises temp-creation-then-cleanup.)

## Validation (verified commands)

```bash
cargo test --bin qmkonnect atomic_write -- --test-threads=1     # the 3 new tests
cargo test --bin qmkonnect -- --test-threads=1                  # full suite (AGENTS.md: single-threaded)
cargo build                                                     # compiles; no tempfile added to [dependencies]
```
- `--test-threads=1` is MANDATORY (AGENTS.md) — shared debouncer state across tests.
- Confirmed `tempfile` is in `[dev-dependencies]` (Cargo.toml:31) → no Cargo.toml edit needed.
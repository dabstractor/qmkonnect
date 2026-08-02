# Research Notes — P1.M2.T2.S2 (tray.rs + linux_tray.rs settings-dialog saves)

## TL;DR
Migrate the **3 settings-dialog `config.toml` save call sites** from `std::fs::write`
to the `atomic_write` helper (landed at `src/core/mod.rs:212` by P1.M2.T1.S1). Each is
a one-line swap; no logic change. This is the tray-dialog half of bug-hunt finding #1
(the `core/mod.rs` seeder half is the sibling P1.M2.T2.S1).

## The 3 call sites (exact, verified this session)

### 1. `src/tray.rs:878` — Windows `show_settings_dialog` (`#[cfg(target_os="windows")]`, fn @ 752)
- Context (lines 876-878):
  ```rust
  let config_content = crate::core::render_config_body(&merged);
  std::fs::write(config_path, config_content)?;
  ```
- `config_path` type: `&std::path::Path` (fn param, line 753).
- `config_content` type: `String` (from render_config_body). Currently passed by VALUE/move.
- Indentation: **12 spaces**.
- NEW: `crate::core::atomic_write(config_path, &config_content)?;`

### 2. `src/tray.rs:1276` — macOS `show_settings_dialog_with_pool` (`#[cfg(target_os="macos")]`, fn @ 1185)
- Context (lines 1275-1276):
  ```rust
  let config_content = crate::core::render_config_body(&merged);
  std::fs::write(config_path, config_content)?;
  ```
- `config_path` type: `&std::path::Path` (fn param, line 1185-1187).
- `config_content` type: `String`. Passed by value.
- Indentation: **20 spaces** (deeper nesting than site 1 ⇒ unique vs site 1).
- NEW: `crate::core::atomic_write(config_path, &config_content)?;`

### 3. `src/linux_tray.rs:822` — `write_config` (entire module `#![cfg(all(target_os="linux", feature="linux-tray"))]`, fn @ 805)
- Context (lines 821-822):
  ```rust
  let content = crate::core::render_config_body(&config);
  std::fs::write(&path, content)?;
  ```
- `path` type: `std::path::PathBuf` (from `dir.join("config.toml")`, line 807). Passed as `&path` (`&PathBuf` → derefs to `&Path`).
- `content` type: `String`. Passed by value.
- fn returns `Result<std::path::PathBuf, Box<dyn std::error::Error>>` ⇒ `?` propagates cleanly.
- Indentation: **4 spaces**.
- NEW: `crate::core::atomic_write(&path, &content)?;`

## atomic_write signature (CONTRACT from P1.M2.T1.S1, present at core/mod.rs:212)
```rust
pub fn atomic_write(path: &Path, content: &str) -> Result<(), Box<dyn Error>>
```
Drop-in for `fs::write(path, content)?`. TWO differences from the current calls:
1. **Path**: same `&Path` shape (config_path is already &Path; &path is &PathBuf → &Path via deref). No change to how the path arg is passed.
2. **Content**: `atomic_write` takes `&str`, but the current `fs::write` calls pass the
   `String` content by VALUE (move). So each new call MUST add a leading `&` to the
   content arg: `&config_content` / `&content`. (Forgetting the `&` moves the String into
   a `&str` slot — Rust will tell you, but get it right the first time.)

## Style decision: FULLY-QUALIFIED, no import (contract's import suggestion is a minor inaccuracy)
The work-item contract says: *"Add `use crate::core::atomic_write;` import at the top of
tray.rs and linux_tray.rs if not already in scope."*

**This conflicts with the file convention.** Verified this session:
- `tray.rs` has ZERO `use crate::core::…` imports. It calls EVERYTHING fully-qualified:
  `crate::core::render_config_body`, `crate::core::parse_config`, `crate::core::create_default_config`,
  `crate::core::Config::default()`, `crate::core::notifier::*`. (The only top `use` lines are
  `use tao::{…}` and `use tray_icon::{…}`.)
- `linux_tray.rs` likewise has no `use crate::core::…` — it uses `crate::core::render_config_body`,
  `crate::core::parse_config`, etc. fully-qualified.

➡️ Therefore the IDIOMATIC migration is the fully-qualified form
`crate::core::atomic_write(...)`, NOT a new `use` import. Adding an import for a single
symbol would be the ONLY `use crate::core::` line in the file and would look out of place
in review. (Same kind of "contract text imprecision" that P1.M2.T2.S1 flagged re `&config_path`.)
The new lines sit one statement below `crate::core::render_config_body(&merged)` — matching
that fully-qualified call is the natural local pattern.

## Platform-gating reality (CRITICAL for validation)
The dev box is **Linux**. Build behavior:
- `linux_tray.rs` is `#![cfg(all(target_os="linux", feature="linux-tray"))]`; `linux-tray`
  is in `default = ["hyprland","macos","linux-tray"]` (Cargo.toml:96). ⇒ **`cargo build` on
  Linux compiles linux_tray.rs** → edit 3 IS validated here.
- `show_settings_dialog` is `#[cfg(target_os="windows")]`; `show_settings_dialog_with_pool`
  is `#[cfg(target_os="macos")]`. ⇒ **Neither tray.rs site compiles on Linux.** Edits 1 & 2
  are validated ONLY by their target-OS builds (Windows: `cargo build` on Windows;
  macOS: `cargo build` on macOS) per the AGENTS.md dev loop.

Baseline confirmed this session: `cargo build --bin qmkonnect` on Linux → exit 0 (atomic_write
present & reachable from linux_tray.rs).

➡️ On a Linux implementer box, `cargo build`/`cargo test` validate edit 3 only. Edits 1 & 2 are
mechanical mirrors of edit 3 (and of P1.M2.T2.S1's seeder swaps) — same `&Path` + `&str`
shapes — so they are low-risk, but they CANNOT be compile-checked here. The PRP must state
this explicitly so the implementer does NOT falsely believe the tray.rs edits are
validated, and does NOT block on a cross-compile they can't run.

## Out-of-scope write sites (do NOT touch)
- `src/tray.rs:204` — autostart first-run marker file (`b"1"`), not config/rules.
- `src/linux_tray.rs:762` — `apply_device_rule` stages a udev rule to `/tmp/…tmp` for
  `pkexec install` (the privileged install is what atomizes it). Different concern.
- `src/core/mod.rs:218` + `:334` — P1.M2.T2.S1's seeder sites (sibling, parallel).

## Tests that gate this migration
- There are NO dedicated unit tests for `show_settings_dialog` / `show_settings_dialog_with_pool`
  / `write_config` (they're platform UI code, exercised manually via the AGENTS.md dev loop).
- The migration's correctness proof is: `cargo build` (linux_tray.rs compiles), the full
  `cargo test --bin qmkonnect -- --test-threads=1` suite stays green (no test added/removed),
  and the byte-identical-content argument (atomic_write is a mechanism swap, same content).
- atomic_write's own behavior is covered by its 3 unit tests (P1.M2.T1.S1).

## Conclusion
Three mechanical one-line swaps. Two nuances the PRP must nail:
1. Add `&` to the content arg (atomic_write takes `&str`; current calls move the String).
2. Use fully-qualified `crate::core::atomic_write(...)` — NOT an import — to match the
   file's universal `crate::core::` convention (contract's import suggestion is a minor inaccuracy).
Plus the platform-validation reality (Linux validates only linux_tray.rs; tray.rs sites
defer to Windows/macOS builds).
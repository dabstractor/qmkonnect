# PRP — P1.M2.T1.S1: Add `reset_handshake_state()` + `perform_handshake()` to the Windows tray save path (`tray.rs`)

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`) — Rust menu-bar/tray daemon.
> **One file edited:** `src/tray.rs` — two insertions inside the Windows-only `show_settings_dialog`
> save block (`#[cfg(target_os = "windows")]`). **No other file.**
> **Scope:** the **Windows** Settings-dialog save path ONLY. The macOS save path (`show_macos_settings_dialog`
> / `show_settings_dialog_with_pool`) and the Linux path (`linux_tray.rs::save_and_notify`) are the **sibling
> subtasks** P1.M2.T1.S2/S3 — do NOT touch them here.
> **Parallel context:** P1.M1.T3.S2 (in parallel) edits `packaging/windows/inno/QMKonnect.iss` — a packaging
> file, unrelated to `src/tray.rs`. No overlap.

---

## ⚠️ READ FIRST — two contract corrections (verified against source + architecture research)

1. **`verbose` is NOT in scope** in `show_settings_dialog`. The task contract says *"perform_handshake(verbose).
   The `verbose` variable is in scope in the tray save function"* — **that is false**. `show_settings_dialog
   (config_path: &Path)` (`tray.rs:838`) and its caller `handle_settings_click()` (`tray.rs:742`, shared
   Win+macOS) have **no `verbose` parameter**; `verbose` exists only in `setup_tray(verbose)` (`tray.rs:298`).
   The authoritative architecture research — `architecture/bug_findings.md` line 132 — states verbatim:
   *"`verbose` is not in scope here — pass `false` or add a param."*
   ➡️ **This PRP uses `perform_handshake(false)`** (the minimal, no-ripple choice sanctioned by bug_findings.md).
   The threading-`verbose` alternative is documented below but is NOT recommended for this task (it touches
   the shared `handle_settings_click` → conflicts with the macOS sibling S2).

2. **The edit is `#[cfg(target_os = "windows")]` and CANNOT be compiled on the Linux dev box.** `cargo build` /
   `cargo test` on Linux compile only the Linux path — the Windows save block is cfg-gated out and is NOT
   type-checked here. Cross-check to `x86_64-pc-windows-msvc` was attempted and FAILS on this box (the
   `eventlog` build.rs needs missing mingw `windmc`/`windres`). **Definitive validation is on a Windows host**
   (mirrors the prev PRP P1.M1.T3.S2 platform-gate split). See Validation Loop.

Everything else in the contract is accurate and verified: the save block location, the `current_config` move,
`atomic_write`, and both notifier functions.

---

## Goal

**Feature Goal**: When a user changes the VID/PID filter in the **Windows** Settings dialog and saves,
immediately **reset the handshake state and re-run the handshake** so the global `CALLBACK_NAMES`
name→id map is rebuilt for the **newly-selected board** — instead of continuing to use the old board's
callback map until a replug. Fixes Bug 4 (PRD ID 3): the multi-capable-board case where board B's rules
silently use board A's name→id mapping (wrong IDs / dropped commands).

**Deliverable**: `src/tray.rs` with two insertions inside the `if let Some(dr) = result { … }` save arm of
`show_settings_dialog` (Windows): (1) a pre-move snapshot of the old VID/PID; (2) a post-`atomic_write`
conditional `reset_handshake_state()` + `perform_handshake(false)` when the VID/PID actually changed.
No signature changes, no new imports (fully-qualified `crate::core::notifier::` paths), no new tests.

**Success Definition**:
- On a Windows host: `cargo build` compiles the edited `show_settings_dialog` cleanly; `cargo test --bin
  qmkonnect -- --test-threads=1` is green (no regression).
- The handshake block runs ONLY when `merged.vendor_id != old_vid || merged.product_id != old_pid`
  (unchanged save ⇒ no spurious reset).
- After a VID/PID change save, `CALLBACK_NAMES` reflects the newly-selected board (manual verification:
  two capable boards A+B → handshake A → Settings → pick B → save → B's callback map is live, no replug).
- `git diff` is confined to the two insertions inside the Windows save block of `src/tray.rs`.

## User Persona (if applicable)

**Target User**: a user with **multiple QMK keyboards that both run the qmk_notifier module** (both
"capable" boards), who uses the Settings dialog to switch which board QMKonnect targets.

**Use Case**: Boards A and B are both plugged in. App starts, handshakes A (builds A's `CALLBACK_NAMES`).
User opens Settings, picks B in the device listbox, clicks OK. Without this fix, B's `rules.toml` callbacks
resolve against A's name→id map (positional IDs differ ⇒ wrong callbacks fire or commands drop). With this
fix, the save resets + re-handshakes, so B's map is live immediately.

**User Journey**: Settings click → modal device picker → pick B → OK → `atomic_write(config.toml, B's
vid/pid)` succeeds → VID/PID-diff detected → `reset_handshake_state()` (clears A's map + dedup guard) →
`perform_handshake(false)` (queries B, rebuilds `CALLBACK_NAMES` for B) → next window notification uses B's map.

**Pain Points Addressed**: eliminates the stale-callback-map bug when switching capable boards via Settings
(no replug/restart required). Single-board users are unaffected (their save changes nothing → no reset).

## Why

- **Closes Bug 4 (PRD ID 3)** — the only remaining correctness gap in the multi-capable-board Settings flow.
  Today the save writes the new VID/PID but the handshake state (`HOST_CAPABLE`, `CALLBACK_NAMES`,
  `HAS_HANDSHAKED`) still belongs to the old board, and the `PresenceTracker` Gain/Loss loop does not
  re-trigger a handshake for a *filter* change (only a real device transition does).
- **Foundation is already in place.** `reset_handshake_state()` (`notifier.rs:814`) and `perform_handshake`
  (`notifier.rs:353`) are both `pub` and already wired into the device-transition path (`tray.rs:455/458`).
  This subtask simply calls the SAME pair from the Settings save path when the filter changes.
- **Safe by construction.** `perform_handshake` is idempotent (guarded by `HAS_HANDSHAKED`); the preceding
  `reset_handshake_state()` clears that guard so the re-handshake actually runs. It reads `config.toml`
  fresh (`configured_filter()`, `notifier.rs:525`), so the just-written VID/PID takes effect. It releases
  the notifier lock per sweep iteration (`notifier.rs:555`, the #4 contention fix) so a synchronous call
  from the tray thread cannot starve window notifications or deadlock.
- **Minimal + scoped.** Two insertions, one cfg-gated function, no signature/import/test churn — exactly
  the Windows half of the three-platform fix (macOS/Linux are the siblings S2/S3).

## What

Two insertions inside the `if let Some(dr) = result { … }` save arm of `show_settings_dialog`
(`src/tray.rs`, ~lines 968-989). The surrounding existing code is UNCHANGED.

```rust
        if let Some(dr) = result {
            // ── INSERT 1: snapshot pre-save VID/PID BEFORE the move ──
            // Config is Clone (not Copy), so `let mut merged = current_config;`
            // below MOVES current_config. vendor_id/product_id are Option<u16>
            // (Copy), so copying them out here is valid and required for the
            // post-save diff check.
            let old_vid = current_config.vendor_id;
            let old_pid = current_config.product_id;

            let mut merged = current_config;
            if let Some((v, p)) = dr.chosen {
                merged.vendor_id = Some(v);
                merged.product_id = Some(p);
            } else if let Some((v, p)) = dr.manual {
                merged.vendor_id = v;
                merged.product_id = p;
            }
            let config_content = crate::core::render_config_body(&merged);

            crate::core::atomic_write(config_path, &config_content)?;

            // ── INSERT 2: if VID/PID changed, reset + re-handshake for the ──
            //    newly-selected board (Bug 4 / PRD ID 3). reset_handshake_state
            //    clears HOST_CAPABLE/BOARD_HAS_RULES/CALLBACK_NAMES/HAS_HANDSHAKED;
            //    perform_handshake then re-runs (its HAS_HANDSHAKED guard was just
            //    cleared) and reads config.toml fresh, so the just-written VID/PID
            //    selects the new board and rebuilds its name→id map. `false` =
            //    non-verbose (verbose is not in scope here — bug_findings.md §132).
            if merged.vendor_id != old_vid || merged.product_id != old_pid {
                crate::core::notifier::reset_handshake_state();
                crate::core::notifier::perform_handshake(false);
            }

            // Configuration saved successfully - no success dialog needed
            // The QMK connection is established fresh for each notification,
            // so no restart is required for the changes to take effect
        }
```

### Success Criteria
- [ ] Pre-move snapshot (`old_vid`/`old_pid`) is taken BEFORE `let mut merged = current_config;` (line ~971).
- [ ] The reset+handshake block is placed AFTER `crate::core::atomic_write(config_path, &config_content)?;`
      succeeds (the `?` returns early on error, so the block only runs on a successful write).
- [ ] The block is guarded by `merged.vendor_id != old_vid || merged.product_id != old_pid`.
- [ ] Uses fully-qualified `crate::core::notifier::reset_handshake_state()` / `::perform_handshake(false)`.
- [ ] No change to `show_settings_dialog`'s signature, no new `use`, no edit outside the Windows save block.
- [ ] On Windows: `cargo build` clean; `cargo test --bin qmkonnect -- --test-threads=1` green.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can make the two insertions using only the exact code above, the
verified line anchors, the `perform_handshake(false)` resolution (with its justification), and the
Windows-host validation gate — all present in this PRP.

### Documentation & References

```yaml
# MUST READ — the bug being fixed
- docfile: plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/architecture/bug_findings.md
  why: Bug 4 (PRD ID 3) root cause + the EXACT recommended fix; line 132 settles the `verbose` question
  section: "Bug 4 (Minor, PRD ID 3): Settings VID/PID change doesn't reset handshake"
  critical: "verbose is not in scope here — pass false or add a param" → this PRP passes false
- url: spec/PRD.md (heading h2.3/h3.3 "Issue 1: Settings-dialog VID/PID change does not reset the handshake")
  why: user-facing repro (two capable boards, pick B, rules use A's name map)

# MUST READ — the file & function being edited
- file: src/tray.rs
  why: the Windows save block to patch; confirm line anchors (cfg gate L824, fn L838, move L971, write L981)
  pattern: "show_settings_dialog(config_path) → if let Some(dr)=result { let mut merged=current_config;
            ...render_config_body(&merged); atomic_write(config_path,&config_content)?; }"
  gotcha: "Config is Clone NOT Copy (src/core/mod.rs:12) → `merged` MOVES current_config. Snapshot
           vendor_id/product_id (Option<u16>, Copy) BEFORE the move line. Reading current_config.* after
           the move is a borrow-checker error."

# MUST READ — the two notifier functions being called (pub, fully-qualified → no `use` needed)
- file: src/core/notifier.rs
  why: confirm signatures + behavior so the call is correct
  pattern: "reset_handshake_state() @ L814 (clears HOST_CAPABLE/BOARD_HAS_RULES/CALLBACK_NAMES/HAS_HANDSHAKED);
            perform_handshake(verbose: bool) @ L353 → perform_handshake_with: idempotent via
            HAS_HANDSHAKED.swap (L511), reads configured_filter() fresh (L525), drops notifier lock before
            the sweep (L555). reset() clears HAS_HANDSHAKED first so perform_handshake RE-RUNS."
  gotcha: "perform_handshake takes a `bool` — verbose is NOT in scope in show_settings_dialog, so pass `false`.
           Do NOT change show_settings_dialog's signature or thread verbose (that hits the shared
           handle_settings_click → macOS sibling S2 scope)."

# REFERENCE — sibling save paths (do NOT edit here; P1.M2.T1.S2 / S3 own them)
- file: src/tray.rs (show_macos_settings_dialog @1566, show_settings_dialog_with_pool @1625) — macOS sibling S2
- file: src/linux_tray.rs (save_and_notify @718) — Linux sibling S3
- file: src/tray.rs:455/458 — the EXISTING device-transition callsite of the same reset/perform pair (the
  pattern to mirror; there `verbose` IS in scope because it's inside setup_tray's poll loop)
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
src/tray.rs              # EDIT: 2 insertions in show_settings_dialog's Windows save block (L968-989)
  - L824 #[cfg(target_os="windows")] gate on show_settings_dialog
  - L838 fn show_settings_dialog(config_path)        <- verbose is NOT a param
  - L849 current_config = parse_config(config_path)
  - L968 if let Some(dr) = result { … }              <- save arm
  - L971 let mut merged = current_config;            <- MOVES current_config (snapshot BEFORE this)
  - L979 render_config_body(&merged)
  - L981 atomic_write(config_path, &config_content)? <- handshake block goes AFTER this
  - L742 handle_settings_click()                     <- shared Win+macOS caller (DO NOT change)
src/core/notifier.rs     # READ ONLY: reset_handshake_state @814, perform_handshake @353
src/core/mod.rs          # READ ONLY: Config is Clone-not-Copy (L12); vendor_id/product_id Option<u16> (Copy)
```

### Desired Codebase tree
**Only `src/tray.rs` changes** — two insertions inside the Windows save block. No new files, no signature
changes, no new imports, no new tests.

### Known Gotchas of our codebase & Library Quirks
```rust
// CRITICAL (platform gate): show_settings_dialog is #[cfg(target_os="windows")]. On the Linux dev box,
// `cargo build`/`cargo test` DO NOT compile or type-check this function — they build only the Linux path.
// An all-green Linux build is NOT proof the Windows edit compiles. Definitive validation is on a Windows
// host (AGENTS.md Windows loop). Cross-check to x86_64-pc-windows-msvc FAILS on this box (eventlog build.rs
// needs missing mingw windmc/windres).

// CRITICAL (move semantics): `let mut merged = current_config;` MOVES current_config (Config is Clone, not
// Copy — src/core/mod.rs:12). The snapshot `let old_vid = current_config.vendor_id;` MUST precede that line.
// vendor_id/product_id are Option<u16> (Copy), so copying them out before the move is valid; accessing
// current_config.* AFTER the move is a compile error.

// CRITICAL (verbose): perform_handshake(verbose: bool) needs a bool, but `verbose` is NOT in scope in
// show_settings_dialog (nor in its shared caller handle_settings_click). Pass `false`. Do NOT thread
// verbose through (touches shared handle_settings_click → macOS sibling S2 scope). (bug_findings.md §132)

// GOTCHA (idempotency): perform_handshake is guarded by `if HAS_HANDSHAKED.swap(true) { return; }`. The
// PRECEDING reset_handshake_state() sets HAS_HANDSHAKED=false, so the swap returns false and the handshake
// RE-RUNS. Calling perform_handshake WITHOUT reset first would no-op (swap returns true). Order matters.

// GOTCHA (fresh config): perform_handshake reads configured_filter() FRESH (notifier.rs:525), so the
// VID/PID written by atomic_write IS used. No cache to invalidate.

// GOTCHA (threading): this runs synchronously on the tray event-loop thread (the modal dialog just closed).
// perform_handshake is bounded (CALLBACK_SWEEP_DEADLINE for a buggy board; ~ms for a real one) and releases
// the notifier lock per sweep iteration (notifier.rs:555), so it cannot deadlock or starve notifications.

// GOTCHA (tests): no unit test is added — the Win32 dialog spawns a real message loop (not unit-testable;
// the existing tray.rs `mod tests` @ L2984 covers only pure helpers like device_status_text). Verify manually.
```

## Implementation Blueprint

### Data models and structure
No data models. The edit reads `current_config.vendor_id`/`.product_id` (`Option<u16>`, `Copy`) and calls two
existing `pub` notifier functions. `Config`, `DialogResult` (`dr.chosen`/`dr.manual`) are unchanged.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: INSERT the pre-move VID/PID snapshot (src/tray.rs, inside `if let Some(dr) = result {`)
  - IMPLEMENT: two lines, immediately BEFORE `let mut merged = current_config;` (~L971):
        let old_vid = current_config.vendor_id;
        let old_pid = current_config.product_id;
  - WHY: Config is Clone-not-Copy; the next line MOVES current_config. These Copy fields must be snapshotted
    before the move (accessing current_config.* after would be a borrow-checker error).
  - PLACEMENT: src/tray.rs, inside the Windows `show_settings_dialog` save arm, before L971.

Task 2: INSERT the reset + re-handshake block (src/tray.rs, AFTER atomic_write succeeds)
  - IMPLEMENT: immediately AFTER `crate::core::atomic_write(config_path, &config_content)?;` (~L981):
        if merged.vendor_id != old_vid || merged.product_id != old_pid {
            crate::core::notifier::reset_handshake_state();
            crate::core::notifier::perform_handshake(false);
        }
  - WHY: the `?` returns early on write error, so this only runs on a successful write. reset clears
    HAS_HANDSHAKED (so perform_handshake's idempotent guard lets it re-run) + CALLBACK_NAMES; perform_handshake
    reads config.toml fresh → targets the newly-selected board → rebuilds its name→id map.
  - NAMING/STYLE: fully-qualified crate::core::notifier:: paths (no new `use`). `false` for verbose
    (verbose is NOT in scope — see contract-correction #1). Add a brief `//` comment citing Bug 4.
  - PLACEMENT: src/tray.rs, same Windows save arm, after the atomic_write line.
  - DEPENDENCIES: Task 1 (uses old_vid/old_pid).

Task 3: VALIDATE — Linux (no-regression) + Windows host (definitive)
  - LINUX (this box — proves NO regression in the Linux build; does NOT typecheck the Windows edit):
        cargo build
        cargo test --bin qmkonnect -- --test-threads=1
  - WINDOWS HOST (DEFINITIVE — the only place the #[cfg(windows)] edit compiles), per AGENTS.md:
        cargo build
        cargo test --bin qmkonnect -- --test-threads=1
    Expected: clean build; all tests green. (If a Windows host is unavailable, pair the Linux build with a
    rigorous line-by-line textual review of the two insertions against the exact code in "What".)
  - MANUAL (the actual feature): two capable boards A+B → start (handshake A) → Settings → pick B → save →
    B's callback map is live (no replug). Single-board / unchanged save → no reset fires.

Task 4: NEVER do these (out of scope / forbidden)
  - DO NOT change show_settings_dialog's signature or thread `verbose` through (hits shared handle_settings_click
    → macOS sibling S2 scope). Use perform_handshake(false).
  - DO NOT edit the macOS save path (show_macos_settings_dialog / show_settings_dialog_with_pool) or
    linux_tray.rs::save_and_notify — those are siblings S2/S3.
  - DO NOT add a unit test for the Win32 dialog (it spawns a real message loop — not unit-testable).
  - DO NOT spawn a thread for the handshake (contract + bug-findings specify a synchronous call).
  - DO NOT trust a green Linux `cargo build` as proof the Windows edit compiles (platform gate — see Gotchas).
  - DO NOT edit PRD.md, any tasks.json, prd_snapshot.md, or any file other than src/tray.rs.
```

### Implementation Patterns & Key Details
```rust
// The device-transition callsite already mirrors this exact pair (src/tray.rs:455/458, inside setup_tray's
// poll loop where `verbose` IS in scope). This task reuses the SAME two calls from the Settings save path,
// the only difference being `false` (verbose not in scope) and the VID/PID-diff guard.

// Order is load-bearing: reset FIRST (clears HAS_HANDSHAKED), THEN perform_handshake (whose guard would
// otherwise no-op). reset also clears CALLBACK_NAMES so the stale old-board map cannot leak into the window.
```

### Integration Points
```yaml
IMPORTS: none. Fully-qualified crate::core::notifier::{reset_handshake_state, perform_handshake} paths.
CALLS:  reset_handshake_state() (notifier.rs:814) + perform_handshake(false) (notifier.rs:353).
        perform_handshake → configured_filter() reads config.toml fresh (notifier.rs:525) → the just-written
        VID/PID selects the new board.
DOWNSTREAM: none. (The macOS/Linux save paths are siblings S2/S3 — independent functions.)
PARALLEL:   P1.M1.T3.S2 edits packaging/windows/inno/QMKonnect.iss — no overlap.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)
```bash
cd /home/dustin/projects/qmkonnect
# NOTE: on Linux this compiles ONLY the Linux path — the Windows edit is cfg-gated out and NOT checked here.
cargo build                 # expect success (no regression in the Linux build)
# Definitive type-check of the Windows edit REQUIRES a Windows host (see Level 3).
```

### Level 2: Tests (Regression — AGENTS.md mandates single-threaded)
```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: all existing tests pass (the new code is in a non-testable Win32 dialog path; no new test added).
# --test-threads=1 is REQUIRED (AGENTS.md — shared global debouncer state).
```

### Level 3: Windows-Host Definitive Build (the ONLY place the edit compiles)
```bash
# On a Windows host (AGENTS.md Windows dev loop):
cd <repo>
cargo build
cargo test --bin qmkonnect -- --test-threads=1
# Expected: clean build (the edited show_settings_dialog type-checks), all tests green.
# (Cross-check `cargo check --target x86_64-pc-windows-msvc` FAILS on the Linux box — the eventlog build.rs
#  needs mingw windmc/windres — so the Windows host is mandatory for compiling this cfg-gated code.)
```

### Level 4: Manual Feature Verification (the actual fix)
```
Precondition: TWO QMK keyboards flashed with the qmk_notifier module ("capable" boards A and B), Windows host.
1. Plug in A and B. Start QMKonnect (handshake runs for A — A's CALLBACK_NAMES built).
2. Open tray → Settings. The device listbox shows A and B.
3. Select board B, click OK.
4. EXPECT: reset_handshake_state() + perform_handshake(false) fire (verify via verbose logs if desired by
   temporarily building with verbose, or via the observable effect): B's rules now resolve against B's
   callback map (not A's). No replug/restart needed.
5. Single-board / unchanged-save regression: open Settings, change nothing, OK → NO reset fires
   (the `merged.vendor_id != old_vid || merged.product_id != old_pid` guard is false).
```

## Final Validation Checklist

### Technical Validation
- [ ] Linux: `cargo build` succeeds; `cargo test --bin qmkonnect -- --test-threads=1` green (no regression).
- [ ] Windows host: `cargo build` clean (the cfg-gated edit type-checks); tests green.
- [ ] `git diff --stat` shows ONLY `src/tray.rs`.

### Feature Validation
- [ ] Pre-move snapshot precedes `let mut merged = current_config;` (move-safe).
- [ ] Reset+handshake block is AFTER `atomic_write(...)?` (runs only on successful write).
- [ ] Block guarded by `merged.vendor_id != old_vid || merged.product_id != old_pid`.
- [ ] Manual multi-board test (A→B switch) rebuilds B's callback map without replug.
- [ ] Unchanged save does NOT reset (guard false).

### Code Quality Validation
- [ ] Fully-qualified `crate::core::notifier::` paths (no new `use`).
- [ ] `perform_handshake(false)` — no signature change, no `verbose` threading (no macOS-sibling bleed).
- [ ] No edit outside the Windows `show_settings_dialog` save block.
- [ ] Brief `//` comment citing Bug 4 / PRD ID 3 on the new block.

### Documentation & Deployment
- [ ] No user-facing/config/API/CLI change (internal handshake lifecycle — DOCS: none per contract).
- [ ] Inline comment explains the reset+re-handshake rationale for future readers.

---

## Anti-Patterns to Avoid
- ❌ Don't pass `verbose` to `perform_handshake` — it's not in scope. Pass `false`. Don't thread it through
  `handle_settings_click` (shared Win+macOS → macOS sibling S2 conflict).
- ❌ Don't snapshot VID/PID AFTER `let mut merged = current_config;` — that line moves `current_config`
  (Config is Clone, not Copy); the borrow checker rejects it. Snapshot BEFORE.
- ❌ Don't call `perform_handshake` WITHOUT `reset_handshake_state()` first — the `HAS_HANDSHAKED` guard would
  no-op it. Order is load-bearing.
- ❌ Don't trust a green Linux `cargo build` as proof the Windows edit compiles — it's `#[cfg(windows)]` and
  isn't compiled on Linux. Validate on a Windows host.
- ❌ Don't edit the macOS/Linux save paths — those are siblings S2/S3.
- ❌ Don't add a unit test for the Win32 dialog (real message loop — not unit-testable) or spawn a handshake thread.
- ❌ Don't edit PRD.md, tasks.json, prd_snapshot.md, or any file other than `src/tray.rs`.

---

## Confidence Score: 8/10

The change is tiny (two insertions, ~7 lines) and the design is fully verified: both notifier functions
exist and behave as required (idempotency guard cleared by the preceding reset; fresh config read; no
deadlock/starvation), and the exact save-block anchors are confirmed. The score is 8 rather than 9-10 for two
reasons: (1) the literal contract's `verbose` claim was wrong and required a documented correction
(`perform_handshake(false)` — the bug-findings-sanctioned choice), and (2) the Windows-only code cannot be
compiled on the Linux dev box, so one-pass success depends on the implementer heeding the platform-gate
warning and validating on a Windows host (or doing a rigorous textual review). Both risks are explicitly
mitigated in this PRP.
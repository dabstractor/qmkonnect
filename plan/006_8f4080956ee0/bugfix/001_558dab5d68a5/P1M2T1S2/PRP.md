# PRP — P1.M2.T1.S2: Add `reset_handshake_state()` + `perform_handshake()` to the macOS tray save path (`tray.rs`)

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`) — Rust menu-bar/tray daemon.
> **One file edited:** `src/tray.rs` — two insertions inside the macOS-only save arm of
> `show_settings_dialog_with_pool` (`#[cfg(target_os = "macos")]`). **No other file.**
> **Scope:** the **macOS** Settings-dialog save path ONLY. The Windows save path
> (`show_settings_dialog`) is sibling **P1.M2.T1.S1**; the Linux path
> (`linux_tray.rs::save_and_notify`) is sibling **P1.M2.T1.S3** — do NOT touch them here.
> **Parallel context:** P1.M2.T1.S1 (parallel) edits the Windows save arm of the same file
> (`src/tray.rs`) — a *different* `#[cfg]`-gated function, no overlap with the macOS arm.

---

## ⚠️ READ FIRST — two contract facts (verified against source + architecture research)

1. **`verbose` is NOT in scope** in either macOS settings function — exactly like the Windows
   sibling. `grep -n "verbose" src/tray.rs` in the macOS region (L1566-1920) returns **NO
   matches.** `show_macos_settings_dialog(config_path: …)` (L1589) and
   `show_settings_dialog_with_pool(config_path: &Path)` (L1648) take no `verbose`, and their
   shared caller `handle_settings_click` (L742) has none either. The authoritative
   `architecture/bug_findings.md` line 132 states verbatim: *"`verbose` is not in scope here —
   pass `false` or add a param."*
   ➡️ **This PRP uses `perform_handshake(false)`** (the minimal, no-ripple choice). Do NOT
   thread `verbose` through `handle_settings_click` — it is shared Win+macOS, so that would
   bleed into sibling S1's scope.

2. **The edit is `#[cfg(target_os = "macos")]` and CANNOT be compiled on the Linux dev box.**
   `cargo build` / `cargo test` on Linux compile only the Linux path — the macOS save block is
   cfg-gated out and is NOT type-checked here (the `objc` crate / Apple frameworks are macOS-
   only). **Definitive validation is on a macOS host** (AGENTS.md macOS loop). Same
   platform-gate split as the Windows sibling S1.

Everything else in the contract is accurate and verified: the save-block location, the
`current_config` move, `atomic_write`, and both notifier functions.

---

## Goal

**Feature Goal**: When a user changes the VID/PID filter in the **macOS** Settings dialog and
saves, immediately **reset the handshake state and re-run the handshake** so the global
`CALLBACK_NAMES` name→id map is rebuilt for the **newly-selected board** — instead of
continuing to use the old board's callback map until a replug. Fixes Bug 4 (PRD ID 3) on
macOS: the multi-capable-board case where board B's rules silently use board A's name→id
mapping (wrong IDs / dropped commands).

**Deliverable**: `src/tray.rs` with two insertions inside the `(Ok(vid), Ok(pid)) => { … }`
save arm of `show_settings_dialog_with_pool` (macOS): (1) a pre-move snapshot of the old
VID/PID; (2) a post-`atomic_write` conditional `reset_handshake_state()` +
`perform_handshake(false)` when the VID/PID actually changed. No signature changes, no new
imports (fully-qualified `crate::core::notifier::` paths), no new tests.

**Success Definition**:
- On a macOS host: `cargo build` compiles the edited `show_settings_dialog_with_pool` cleanly;
  `cargo test --bin qmkonnect -- --test-threads=1` is green (no regression).
- The handshake block runs ONLY when `merged.vendor_id != old_vid || merged.product_id != old_pid`
  (unchanged save ⇒ no spurious reset).
- After a VID/PID change save, `CALLBACK_NAMES` reflects the newly-selected board (manual
  verification: two capable boards A+B → handshake A → Settings → pick B → save → B's callback
  map is live, no replug).
- `git diff` is confined to the two insertions inside the macOS save arm of `src/tray.rs`.

## User Persona (if applicable)

**Target User**: a user with **multiple QMK keyboards that both run the qmk_notifier module**
(both "capable" boards), who uses the macOS Settings dialog (NSAlert + accessory view) to
switch which board QMKonnect targets.

**Use Case**: Boards A and B are both plugged in. App starts, handshakes A (builds A's
`CALLBACK_NAMES`). User opens the tray → Settings, picks B's radio row (or types B's hex IDs
under Advanced), clicks OK. Without this fix, B's `rules.toml` callbacks resolve against A's
name→id map (positional IDs differ ⇒ wrong callbacks fire or commands drop). With this fix,
the save resets + re-handshakes, so B's map is live immediately.

**User Journey**: tray → Settings → modal device picker (`runModal` blocks) → pick B → OK →
`atomic_write(config.toml, B's vid/pid)` succeeds → VID/PID-diff detected →
`reset_handshake_state()` (clears A's map + the `HAS_HANDSHAKED` guard) →
`perform_handshake(false)` (queries B, rebuilds `CALLBACK_NAMES` for B) → next window
notification uses B's map.

**Pain Points Addressed**: eliminates the stale-callback-map bug when switching capable boards
via Settings (no replug/restart required). Single-board users are unaffected (their save
changes nothing → no reset).

## Why

- **Closes the macOS half of Bug 4 (PRD ID 3)** — the only remaining correctness gap in the
  multi-capable-board Settings flow on macOS. Today the save writes the new VID/PID but the
  handshake state (`HOST_CAPABLE`, `CALLBACK_NAMES`, `HAS_HANDSHAKED`) still belongs to the old
  board, and the `PresenceTracker` Gain/Loss loop does not re-trigger a handshake for a *filter*
  change (only a real device transition does).
- **Foundation is already in place.** `reset_handshake_state()` (`notifier.rs:814`) and
  `perform_handshake` (`notifier.rs:353`) are both `pub` and already wired into the device-
  transition path (`tray.rs:455/458`). This subtask calls the SAME pair from the macOS Settings
  save path when the filter changes — mirroring the Windows sibling S1 one-to-one.
- **Safe by construction.** `perform_handshake` is idempotent (guarded by `HAS_HANDSHAKED`); the
  preceding `reset_handshake_state()` clears that guard so the re-handshake actually runs. It
  reads `config.toml` fresh (`configured_filter()`, `notifier.rs:83/521`), so the just-written
  VID/PID takes effect. It releases the notifier lock per sweep iteration (`notifier.rs:555`,
  the #4 contention fix) so a synchronous call from the tray thread (right after `runModal`
  returns) cannot starve window notifications or deadlock.
- **Minimal + scoped.** Two insertions, one cfg-gated match arm, no signature/import/test churn
  — exactly the macOS half of the three-platform fix (Windows = S1, Linux = S3).

## What

Two insertions inside the `(Ok(vid), Ok(pid)) => { … }` save arm of
`show_settings_dialog_with_pool` (`src/tray.rs`, ~lines 1891-1902). The surrounding existing
code is UNCHANGED. The two insertions are at **20-space indentation** (inside `unsafe { if
response == 1000 { match (…) { (Ok,Ok) => { … } } } }`).

```rust
                (Ok(vid), Ok(pid)) => {
                    // ── INSERT 1: snapshot pre-save VID/PID BEFORE the move ──
                    // Config is Clone (not Copy) — `let mut merged = current_config;`
                    // below MOVES current_config. vendor_id/product_id are Option<u16>
                    // (Copy), so copying them out here is valid and required for the
                    // post-save diff check.
                    let old_vid = current_config.vendor_id;
                    let old_pid = current_config.product_id;

                    // PRESERVE every non-VID/PID field … (existing comment)
                    let mut merged = current_config;
                    if let Some((v, p)) = chosen {
                        merged.vendor_id = Some(v);
                        merged.product_id = Some(p);
                    } else {
                        merged.vendor_id = vid;
                        merged.product_id = pid;
                    }
                    let config_content = crate::core::render_config_body(&merged);
                    crate::core::atomic_write(config_path, &config_content)?;

                    // ── INSERT 2: if VID/PID changed, reset + re-handshake for ──
                    //    the newly-selected board (Bug 4 / PRD ID 3).
                    //    reset_handshake_state clears HOST_CAPABLE/BOARD_HAS_RULES/
                    //    CALLBACK_NAMES/HAS_HANDSHAKED; perform_handshake then re-runs
                    //    (its HAS_HANDSHAKED guard was just cleared) and reads config.toml
                    //    fresh, so the just-written VID/PID selects the new board and
                    //    rebuilds its name→id map. `false` = non-verbose (verbose is not
                    //    in scope here — bug_findings.md §132).
                    if merged.vendor_id != old_vid || merged.product_id != old_pid {
                        crate::core::notifier::reset_handshake_state();
                        crate::core::notifier::perform_handshake(false);
                    }
                }
```

> NOTE on `vid`/`pid`: in this arm they are already `Option<u16>` from `parse_id_field`
> (the macOS path assigns them directly to `merged.vendor_id`/`merged.product_id` in the
> `else` branch). That is unchanged — only the two INSERT blocks are new.

### Success Criteria
- [ ] Pre-move snapshot (`old_vid`/`old_pid`) is taken BEFORE `let mut merged = current_config;`
      (line ~1892), inside the `(Ok(vid), Ok(pid)) =>` arm.
- [ ] The reset+handshake block is placed AFTER `crate::core::atomic_write(config_path,
      &config_content)?;` succeeds (line ~1901; the `?` returns early on error, so the block
      only runs on a successful write).
- [ ] The block is guarded by `merged.vendor_id != old_vid || merged.product_id != old_pid`.
- [ ] Uses fully-qualified `crate::core::notifier::reset_handshake_state()` /
      `::perform_handshake(false)`.
- [ ] No change to `show_settings_dialog_with_pool`'s signature, no new `use`, no edit outside
      the macOS `(Ok(vid), Ok(pid)) =>` save arm.
- [ ] On macOS: `cargo build` clean; `cargo test --bin qmkonnect -- --test-threads=1` green.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can make the two insertions using only the exact code
above, the verified line anchors, the `perform_handshake(false)` resolution (with its
justification), and the macOS-host validation gate — all present in this PRP.

### Documentation & References

```yaml
# MUST READ — the bug being fixed
- docfile: plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/architecture/bug_findings.md
  why: Bug 4 (PRD ID 3) root cause + the EXACT recommended fix; line 132 settles the `verbose` question
  section: "Bug 4 (Minor, PRD ID 3): Settings VID/PID change doesn't reset handshake"
  critical: "verbose is not in scope here — pass false or add a param" → this PRP passes false; macOS
            location is ~L1877 (same pattern as Windows)
- url: spec/PRD.md (heading h2.3/h3.3 "Issue 1: Settings-dialog VID/PID change does not reset the handshake")
  why: user-facing repro (two capable boards, pick B, rules use A's name map); lists src/tray.rs:1877/1878

# MUST READ — the file & function being edited
- file: src/tray.rs
  why: the macOS save arm to patch; confirm line anchors (cfg gate, fn L1648, move L1892, write L1901)
  pattern: "show_settings_dialog_with_pool(config_path) → runModal → if response==1000 { match
            (parse_id_field,parse_id_field) { (Ok(vid),Ok(pid))=>{ let mut merged=current_config;
            ...render_config_body(&merged); atomic_write(config_path,&config_content)?; } } }"
  gotcha: "Config is Clone NOT Copy (src/core/mod.rs:23) → `merged` MOVES current_config. Snapshot
           vendor_id/product_id (Option<u16>, Copy) BEFORE the move line (L1892). Reading
           current_config.* after the move is a borrow-checker error."

# MUST READ — the two notifier functions being called (pub, fully-qualified → no `use` needed)
- file: src/core/notifier.rs
  why: confirm signatures + behavior so the call is correct
  pattern: "reset_handshake_state() @ L814 (clears HOST_CAPABLE/BOARD_HAS_RULES/CALLBACK_NAMES/HAS_HANDSHAKED);
            perform_handshake(verbose: bool) @ L353 → idempotent via HAS_HANDSHAKED.swap, reads
            configured_filter() fresh (L521, fn @ L83), drops notifier lock before the sweep (L555).
            reset() clears HAS_HANDSHAKED first so perform_handshake RE-RUNS."
  gotcha: "perform_handshake takes a `bool` — verbose is NOT in scope in show_settings_dialog_with_pool
           (nor in show_macos_settings_dialog / shared handle_settings_click), so pass `false`."

# REFERENCE — the sibling Windows fix (the pattern this task mirrors 1:1) + other save paths (do NOT edit)
- docfile: plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/P1M2T1S1/PRP.md
  why: the Windows save-arm fix — identical two-insertion design, same `perform_handshake(false)`,
       same move-semantics + idempotency reasoning. This task is the macOS twin.
- file: src/tray.rs (show_settings_dialog @838, Windows save arm ~L968-989) — sibling S1, DO NOT edit
- file: src/linux_tray.rs (save_and_notify @718) — sibling S3, DO NOT edit
- file: src/tray.rs:455/458 — the EXISTING device-transition callsite of the same reset/perform pair
  (the pattern to mirror; there `verbose` IS in scope because it's inside setup_tray's poll loop)
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
src/tray.rs              # EDIT: 2 insertions in show_settings_dialog_with_pool's macOS save arm (L1891-1902)
  - L742  handle_settings_click()                       <- shared Win+macOS caller (DO NOT change)
  - L792  macOS branch → show_macos_settings_dialog      <- no save here; delegates at L1605
  - L1589 fn show_macos_settings_dialog(config_path)     <- verbose is NOT a param; just delegates
  - L1605   show_settings_dialog_with_pool(config_path)
  - L1648 fn show_settings_dialog_with_pool(config_path) <- #[cfg(macos)]; verbose NOT a param
  - L1649   current_config = parse_config(config_path)
  - L1867   response = msg_send![alert, runModal]         <- BLOCKS the tray thread
  - L1869   if response == 1000 {
  - L1887     match (parse_id_field(vendor), parse_id_field(product)) {
  - L1891       (Ok(vid), Ok(pid)) => { … SAVE … }        <- EDIT HERE (20-space indent)
  - L1892         let mut merged = current_config;        <- MOVES current_config (snapshot BEFORE)
  - L1900         render_config_body(&merged)
  - L1901         atomic_write(config_path,&config_content)? <- handshake block goes AFTER this
  - L1903       (Err(e),_) | (_,Err(e)) => { show_macos_error_message(...) }  <- no save, no edit
src/core/notifier.rs     # READ ONLY: reset_handshake_state @814, perform_handshake @353 (verbose:bool)
src/core/mod.rs          # READ ONLY: Config is Clone-not-Copy (L23); vendor_id/product_id Option<u16> (Copy)
```

### Desired Codebase tree
**Only `src/tray.rs` changes** — two insertions inside the macOS `(Ok(vid), Ok(pid)) =>` save
arm. No new files, no signature changes, no new imports, no new tests.

### Known Gotchas of our codebase & Library Quirks
```rust
// CRITICAL (platform gate): show_settings_dialog_with_pool (and show_macos_settings_dialog) are
// #[cfg(target_os="macos")]. On the Linux dev box, `cargo build`/`cargo test` DO NOT compile or
// type-check this function — they build only the Linux path (the objc crate / Apple frameworks
// are macOS-only). An all-green Linux build is NOT proof the macOS edit compiles. Definitive
// validation is on a macOS host (AGENTS.md macOS loop).

// CRITICAL (move semantics): `let mut merged = current_config;` (L1892) MOVES current_config
// (Config is Clone, not Copy — src/core/mod.rs:23). The snapshot `let old_vid = current_config.vendor_id;`
// MUST precede that line. vendor_id/product_id are Option<u16> (Copy), so copying them out before
// the move is valid; accessing current_config.* AFTER the move is a compile error.

// CRITICAL (verbose): perform_handshake(verbose: bool) needs a bool, but `verbose` is NOT in scope
// in show_settings_dialog_with_pool (nor show_macos_settings_dialog, nor the shared
// handle_settings_click). grep for "verbose" in the macOS region (L1566-1920) = empty. Pass `false`.
// Do NOT thread verbose through handle_settings_click (touches the shared Win+macOS caller →
// Windows sibling S1 scope). (bug_findings.md §132)

// GOTCHA (idempotency): perform_handshake is guarded by `if HAS_HANDSHAKED.swap(true) { return; }`.
// The PRECEDING reset_handshake_state() sets HAS_HANDSHAKED=false, so the swap returns false and the
// handshake RE-RUNS. Calling perform_handshake WITHOUT reset first would no-op. Order matters.

// GOTCHA (fresh config): perform_handshake reads configured_filter() FRESH (notifier.rs:521/83), so
// the VID/PID written by atomic_write IS used. No cache to invalidate.

// GOTCHA (threading): this runs synchronously on the tray event-loop thread (runModal just returned).
// perform_handshake is bounded (CALLBACK_SWEEP_DEADLINE for a buggy board; ~ms for a real one) and
// releases the notifier lock per sweep iteration (notifier.rs:555), so it cannot deadlock or starve
// notifications.

// GOTCHA (tests): no unit test is added — NSAlert runModal spawns a real Cocoa modal loop (not
// unit-testable; the existing tray.rs `mod tests` covers only pure helpers). Verify manually on macOS.
```

## Implementation Blueprint

### Data models and structure
No data models. The edit reads `current_config.vendor_id`/`.product_id` (`Option<u16>`, `Copy`)
and calls two existing `pub` notifier functions. `Config`, the `match`'s `vid`/`pid`
(`Option<u16>` from `parse_id_field`) and `chosen` (`Option<(u16,u16)>`) are unchanged.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: INSERT the pre-move VID/PID snapshot (src/tray.rs, inside the `(Ok(vid), Ok(pid)) =>` arm)
  - IMPLEMENT: two lines, immediately BEFORE `let mut merged = current_config;` (~L1892):
        let old_vid = current_config.vendor_id;
        let old_pid = current_config.product_id;
  - WHY: Config is Clone-not-Copy; the next line MOVES current_config. These Copy fields must be
    snapshotted before the move (accessing current_config.* after would be a borrow-checker error).
  - PLACEMENT: src/tray.rs, inside the macOS `show_settings_dialog_with_pool` save arm
    (`(Ok(vid),Ok(pid)) => { … }`), before L1892. 20-space indentation (matches `let mut merged`).

Task 2: INSERT the reset + re-handshake block (src/tray.rs, AFTER atomic_write succeeds)
  - IMPLEMENT: immediately AFTER `crate::core::atomic_write(config_path, &config_content)?;` (~L1901):
        if merged.vendor_id != old_vid || merged.product_id != old_pid {
            crate::core::notifier::reset_handshake_state();
            crate::core::notifier::perform_handshake(false);
        }
  - WHY: the `?` returns early on write error, so this only runs on a successful write. reset clears
    HAS_HANDSHAKED (so perform_handshake's idempotent guard lets it re-run) + CALLBACK_NAMES;
    perform_handshake reads config.toml fresh → targets the newly-selected board → rebuilds its
    name→id map.
  - NAMING/STYLE: fully-qualified crate::core::notifier:: paths (no new `use`). `false` for verbose
    (verbose is NOT in scope — see contract-correction #1). Add a brief `//` comment citing Bug 4.
  - PLACEMENT: src/tray.rs, same macOS save arm, after the atomic_write line. 20-space indentation.
  - DEPENDENCIES: Task 1 (uses old_vid/old_pid).

Task 3: VALIDATE — Linux (no-regression) + macOS host (definitive)
  - LINUX (this box — proves NO regression in the Linux build; does NOT typecheck the macOS edit):
        cargo build
        cargo test --bin qmkonnect -- --test-threads=1
  - macOS HOST (DEFINITIVE — the only place the #[cfg(macos)] edit compiles), per AGENTS.md:
        cargo test --bin qmkonnet -- --test-threads=1
        cd packaging/macos && ./clean.sh && ./build.sh && ./install.sh
        open /Applications/QMKonnect.app
    Expected: clean build; all tests green; app launches. (If a macOS host is unavailable, pair the
    Linux build with a rigorous line-by-line textual review of the two insertions against the exact
    code in "What".)
  - MANUAL (the actual feature): two capable boards A+B → start (handshake A) → Settings → pick B →
    save → B's callback map is live (no replug). Single-board / unchanged save → no reset fires.

Task 4: NEVER do these (out of scope / forbidden)
  - DO NOT change show_settings_dialog_with_pool's (or show_macos_settings_dialog's) signature or
    thread `verbose`. Use perform_handshake(false).
  - DO NOT edit the Windows save path (show_settings_dialog) or linux_tray.rs::save_and_notify —
    those are siblings S1/S3.
  - DO NOT add a unit test for the NSAlert runModal dialog (real Cocoa modal loop — not unit-testable).
  - DO NOT spawn a thread for the handshake (contract + bug-findings specify a synchronous call).
  - DO NOT trust a green Linux `cargo build` as proof the macOS edit compiles (platform gate — see Gotchas).
  - DO NOT edit PRD.md, any tasks.json, prd_snapshot.md, or any file other than src/tray.rs.
```

### Implementation Patterns & Key Details
```rust
// The device-transition callsite already mirrors this exact pair (src/tray.rs:455/458, inside
// setup_tray's poll loop where `verbose` IS in scope). This task reuses the SAME two calls from the
// macOS Settings save path, the only differences being `false` (verbose not in scope) and the
// VID/PID-diff guard.

// Order is load-bearing: reset FIRST (clears HAS_HANDSHAKED), THEN perform_handshake (whose guard
// would otherwise no-op). reset also clears CALLBACK_NAMES so the stale old-board map cannot leak.

// Indentation: the macOS save arm is nested `unsafe { if response==1000 { match { (Ok,Ok) => { … } } } }`
// → the two insertions sit at 20 spaces, aligned with `let mut merged` and `atomic_write`.
```

### Integration Points
```yaml
IMPORTS: none. Fully-qualified crate::core::notifier::{reset_handshake_state, perform_handshake} paths.
CALLS:  reset_handshake_state() (notifier.rs:814) + perform_handshake(false) (notifier.rs:353).
        perform_handshake → configured_filter() reads config.toml fresh (notifier.rs:521/83) → the
        just-written VID/PID selects the new board.
DOWNSTREAM: none. (The Windows/Linux save paths are siblings S1/S3 — independent functions.)
PARALLEL:   P1.M2.T1.S1 edits the Windows save arm of the same src/tray.rs — different #[cfg] gate,
            no overlap.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)
```bash
cd /home/dustin/projects/qmkonnect
# NOTE: on Linux this compiles ONLY the Linux path — the macOS edit is cfg-gated out and NOT checked here.
cargo build                 # expect success (no regression in the Linux build)
# Definitive type-check of the macOS edit REQUIRES a macOS host (see Level 3).
```

### Level 2: Tests (Regression — AGENTS.md mandates single-threaded)
```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: all existing tests pass (the new code is in a non-testable macOS dialog path; no new test).
# --test-threads=1 is REQUIRED (AGENTS.md — shared global debouncer state).
```

### Level 3: macOS-Host Definitive Build (the ONLY place the edit compiles)
```bash
# On a macOS host (AGENTS.md macOS dev loop):
cd <repo>
cargo test --bin qmkonnect -- --test-threads=1     # single-threaded (shared debouncer state)
cd packaging/macos && ./clean.sh && ./build.sh && ./install.sh   # clean → build → install
open /Applications/QMKonnect.app                                  # test the installed bundle
# Expected: clean build (the edited show_settings_dialog_with_pool type-checks), all tests green,
#           app launches (grant the Screen-Recording prompt if it appears).
# (Cross-check from the Linux box is not viable — the objc crate / Apple frameworks are macOS-only.)
```

### Level 4: Manual Feature Verification (the actual fix)
```
Precondition: TWO QMK keyboards flashed with the qmk_notifier module ("capable" boards A and B), macOS host.
1. Plug in A and B. Start QMKonnect (handshake runs for A — A's CALLBACK_NAMES built).
2. Open tray → Settings. The NSAlert device list shows A and B.
3. Select board B's radio row (or type B's hex IDs under Advanced), click OK.
4. EXPECT: reset_handshake_state() + perform_handshake(false) fire: B's rules now resolve against B's
   callback map (not A's). No replug/restart needed.
5. Single-board / unchanged-save regression: open Settings, change nothing, OK → NO reset fires
   (the `merged.vendor_id != old_vid || merged.product_id != old_pid` guard is false).
```

## Final Validation Checklist

### Technical Validation
- [ ] Linux: `cargo build` succeeds; `cargo test --bin qmkonnect -- --test-threads=1` green (no regression).
- [ ] macOS host: `cargo test` green; `./build.sh`/`./install.sh` clean (the cfg-gated edit type-checks);
      app launches via `open`.
- [ ] `git diff --stat` shows ONLY `src/tray.rs`.

### Feature Validation
- [ ] Pre-move snapshot precedes `let mut merged = current_config;` (move-safe).
- [ ] Reset+handshake block is AFTER `atomic_write(...)?` (runs only on successful write).
- [ ] Block guarded by `merged.vendor_id != old_vid || merged.product_id != old_pid`.
- [ ] Manual multi-board test (A→B switch) rebuilds B's callback map without replug.
- [ ] Unchanged save does NOT reset (guard false).

### Code Quality Validation
- [ ] Fully-qualified `crate::core::notifier::` paths (no new `use`).
- [ ] `perform_handshake(false)` — no signature change, no `verbose` threading (no Windows-sibling bleed).
- [ ] No edit outside the macOS `(Ok(vid), Ok(pid)) =>` save arm of `show_settings_dialog_with_pool`.
- [ ] Brief `//` comment citing Bug 4 / PRD ID 3 on the new block.
- [ ] Indentation matches the surrounding 20-space save-arm lines.

### Documentation & Deployment
- [ ] No user-facing/config/API/CLI change (internal handshake lifecycle — DOCS: none per contract).
- [ ] Inline comment explains the reset+re-handshake rationale for future readers.

---

## Anti-Patterns to Avoid
- ❌ Don't pass `verbose` to `perform_handshake` — it's not in scope (grep-confirmed empty in the macOS
  region). Pass `false`. Don't thread it through `handle_settings_click` (shared Win+macOS → Windows
  sibling S1 conflict).
- ❌ Don't snapshot VID/PID AFTER `let mut merged = current_config;` — that line moves `current_config`
  (Config is Clone, not Copy); the borrow checker rejects it. Snapshot BEFORE.
- ❌ Don't call `perform_handshake` WITHOUT `reset_handshake_state()` first — the `HAS_HANDSHAKED` guard
  would no-op it. Order is load-bearing.
- ❌ Don't trust a green Linux `cargo build` as proof the macOS edit compiles — it's `#[cfg(macos)]` and
  isn't compiled on Linux. Validate on a macOS host.
- ❌ Don't edit the Windows/Linux save paths or `show_macos_settings_dialog` (delegator, no save) —
  those are siblings / out of scope.
- ❌ Don't add a unit test for the NSAlert runModal dialog (real Cocoa modal loop — not unit-testable)
  or spawn a handshake thread.
- ❌ Don't edit PRD.md, tasks.json, prd_snapshot.md, or any file other than `src/tray.rs`.

---

## Confidence Score: 8/10

The change is tiny (two insertions, ~7 lines) and the design is fully verified and mirrors the
Windows sibling S1 one-to-one: both notifier functions exist and behave as required (idempotency
guard cleared by the preceding reset; fresh config read; no deadlock/starvation), the macOS save-
block anchors are confirmed (L1892 move, L1901 write, single save arm at 20-space indent), and
`verbose` is grep-confirmed NOT in scope (→ `false`). The score is 8 rather than 9-10 because the
macOS-only code cannot be compiled on the Linux dev box, so one-pass success depends on the
implementer heeding the platform-gate warning and validating on a macOS host (or doing a rigorous
textual review). Both risks are explicitly mitigated in this PRP.
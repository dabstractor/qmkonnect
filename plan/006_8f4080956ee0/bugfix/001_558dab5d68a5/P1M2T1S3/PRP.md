# PRP — P1.M2.T1.S3: Add `reset_handshake_state()` + `perform_handshake()` to the Linux tray save path (`linux_tray.rs`)

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`) — Rust menu-bar/tray daemon.
> **One file edited:** `src/linux_tray.rs` — two insertions inside `save_and_notify`
> (L718). **No other file.**
> **Scope:** the **Linux** Settings-dialog save path ONLY (`save_and_notify`, the shared
> save tail for both the zenity picker path at L856 and the `--forms` Advanced path at L925).
> The Windows save path (`tray.rs::show_settings_dialog`) is sibling **P1.M2.T1.S1**
> (Complete); the macOS path (`tray.rs::show_settings_dialog_with_pool`) is sibling
> **P1.M2.T1.S2** (parallel) — do NOT touch them here.
> **Parallel context:** P1.M2.T1.S2 (parallel) edits `src/tray.rs` — a *different file*,
> no overlap with `src/linux_tray.rs`.

---

## ⚠️ READ FIRST — two contract facts (verified against source + architecture research)

1. **The pre-save VID/PID helper is `current_config_vidpid()`, NOT `current_vidpid()`.**
   The task contract names a helper `fn current_vidpid()` at "L988-1012" — that does **not
   exist**. `grep -n "fn current_vidpid" src/linux_tray.rs` returns no match. The real
   helper is `fn current_config_vidpid() -> (Option<u16>, Option<u16>)` at **L1006**. It
   reads the first existing config candidate (`get_config_paths()` → `parse_config()`) and
   returns `(vendor_id, product_id)`, or `(None, None)` on a fresh install.
   ➡️ **This PRP uses `current_config_vidpid()`** for the pre-save snapshot.

2. **`verbose` is NOT in scope** — exactly like siblings S1/S2. `save_and_notify(vendor_id,
   product_id)` (L718) takes only vid/pid, and its two call sites (L856 picker, L925 forms)
   pass no `verbose` either. The authoritative `architecture/bug_findings.md` line 132
   states verbatim: *"`verbose` is not in scope here — pass `false` or add a param."*
   ➡️ **This PRP uses `perform_handshake(false)`** (the minimal, no-ripple choice). Do NOT
   add a `verbose` param or touch the two call sites.

### 🎯 Why this task is LOWER-RISK than siblings S1/S2 (read this)
`src/main.rs:17` declares `mod linux_tray;` and **this file IS compiled and type-checked on
the Linux dev box** (it is the SNI tray for the Linux build; it is *not* `#[cfg]`-gated out
on Linux, unlike S1's `#[cfg(windows)]` block and S2's `#[cfg(macos)]` block). Therefore
**`cargo build` and `cargo test --bin qmkonnect -- --test-threads=1` on this box ARE
definitive for this edit** — there is no platform-host caveat. One-pass success is
verifiable here. This is the single biggest confidence difference from S1/S2.

### Why the edit is SIMPLER than S1/S2 (no move-semantics hazard)
S1/S2 snapshot `current_config.vendor_id/product_id` *before* a `let mut merged =
current_config;` line that MOVES `current_config` (Config is Clone, not Copy). **S3 has no
such local.** `save_and_notify` receives the NEW vid/pid as its parameters and reads the OLD
pair fresh from disk via `current_config_vidpid()` (which returns owned `Option<u16>` —
`Copy`). So the snapshot is one trivial line and there is no borrow-checker move hazard.

Everything else in the contract is accurate and verified: the save location, `write_config`,
and both notifier functions.

---

## Goal

**Feature Goal**: When a user changes the VID/PID filter in the **Linux** Settings dialog
(either the zenity device picker or the `--forms` Advanced entry) and saves, immediately
**reset the handshake state and re-run the handshake** so the global `CALLBACK_NAMES`
name→id map is rebuilt for the **newly-selected board** — instead of continuing to use the
old board's callback map until a replug. Fixes Bug 4 (PRD ID 3) on Linux: the
multi-capable-board case where board B's rules silently use board A's name→id mapping
(wrong IDs / dropped commands).

**Deliverable**: `src/linux_tray.rs` with two insertions inside `save_and_notify`: (1) a
pre-`write_config` snapshot of the old VID/PID via `current_config_vidpid()`; (2) a
post-`write_config` conditional `reset_handshake_state()` + `perform_handshake(false)` at
the top of the `Ok(path) =>` arm when the VID/PID actually changed. No signature changes, no
new imports (fully-qualified `crate::core::notifier::` paths), no call-site edits, no new
tests.

**Success Definition**:
- On the Linux dev box: `cargo build` compiles the edited `save_and_notify` cleanly;
  `cargo test --bin qmkonnect -- --test-threads=1` is green (no regression).
- The handshake block runs ONLY when `(vendor_id, product_id) != (old_vid, old_pid)`
  (unchanged save ⇒ no spurious reset).
- After a VID/PID change save, `CALLBACK_NAMES` reflects the newly-selected board (manual
  verification: two capable boards A+B → handshake A → Settings → pick B → save → B's
  callback map is live, no replug).
- `git diff` is confined to the two insertions inside `save_and_notify` of
  `src/linux_tray.rs`.

## User Persona (if applicable)

**Target User**: a user with **multiple QMK keyboards that both run the qmk_notifier module**
(both "capable" boards), on a Linux/Wayland desktop using an SNI-hosting status bar
(Waybar / SwayNC / KDE Plasma / GNOME+AppIndicator), who uses the zenity Settings dialog to
switch which board QMKonnect targets.

**Use Case**: Boards A and B are both plugged in. App starts, handshakes A (builds A's
`CALLBACK_NAMES`). User opens the tray → Settings. If ≥1 capable board is detected and it's
not the clean-auto single-board case, the zenity `--list` device picker appears; the user
picks B's row and clicks OK (→ `save_and_notify(Some(v), Some(p))` at L856). Without this
fix, B's `rules.toml` callbacks resolve against A's name→id map (positional IDs differ ⇒
wrong callbacks fire or commands drop). With this fix, the save resets + re-handshakes, so
B's map is live immediately.

**User Journey**: tray → Settings → zenity picker (blocks) → pick B → OK →
`write_config(B's vid/pid)` succeeds → VID/PID-diff detected →
`reset_handshake_state()` (clears A's map + the `HAS_HANDSHAKED` guard) →
`perform_handshake(false)` (reads config.toml fresh, queries B, rebuilds `CALLBACK_NAMES`
for B) → `apply_device_rule` + `notify` proceed as before → next window notification uses
B's map.

**Pain Points Addressed**: eliminates the stale-callback-map bug when switching capable
boards via the Linux Settings dialog (no replug/restart required). Single-board / clean-auto
users are unaffected (their save changes nothing → no reset).

## Why

- **Closes the Linux half of Bug 4 (PRD ID 3)** — the last of the three platform save paths.
  Today `save_and_notify` writes the new VID/PID (and applies the udev rule) but leaves the
  handshake state (`HOST_CAPABLE`, `CALLBACK_NAMES`, `HAS_HANDSHAKED`) belonging to the old
  board, and the `PresenceTracker` Gain/Loss loop does not re-trigger a handshake for a
  *filter* change (only a real device transition does).
- **Foundation is already in place.** `reset_handshake_state()` (`notifier.rs:814`) and
  `perform_handshake` (`notifier.rs:353`) are both `pub` and already wired into the
  device-transition path (`tray.rs:455/458`). This subtask calls the SAME pair from the
  Linux Settings save tail when the filter changes — mirroring the Windows sibling S1 and
  the macOS sibling S2 one-to-one.
- **Safe by construction.** `perform_handshake` is idempotent (guarded by `HAS_HANDSHAKED`);
  the preceding `reset_handshake_state()` clears that guard so the re-handshake actually
  runs. It reads `config.toml` fresh (`configured_filter()`, `notifier.rs:83/521`), so the
  just-written VID/PID takes effect. It releases the notifier lock per sweep iteration
  (`notifier.rs:555`, the #4 contention fix) so a synchronous call from the tray thread
  (right after the blocking zenity dialog returns) cannot starve window notifications or
  deadlock.
- **Minimal + scoped + verifiable here.** Two insertions, one function, no signature/import/
  call-site/test churn. Unlike S1/S2, the edit compiles on the Linux dev box, so it is
  fully validated locally before any hardware test.

## What

Two insertions inside `save_and_notify` (`src/linux_tray.rs`, L718–748). The surrounding
existing code is UNCHANGED. The two insertions are at **4-space indentation** (top level of
`save_and_notify`) and **12-space indentation** (inside the `Ok(path) => { … }` arm),
matching the existing arm body.

```rust
fn save_and_notify(vendor_id: Option<u16>, product_id: Option<u16>) {
    let vid_str = vendor_id
        .map(|v| format!("0x{v:04x}"))
        .unwrap_or_else(|| "auto".to_string());
    let pid_str = product_id
        .map(|p| format!("0x{p:04x}"))
        .unwrap_or_else(|| "auto".to_string());

    // ── INSERT 1: snapshot the PRE-save VID/PID BEFORE write_config ──
    // write_config (below) overwrites config.toml, so current_config_vidpid()
    // must run NOW (calling it after write_config would read the NEW values,
    // making the diff check useless). Returns (Option<u16>, Option<u16>);
    // (None, None) on a fresh install (auto-discovery). vendor_id/product_id
    // are the dialog's NEW values (function params).
    let (old_vid, old_pid) = current_config_vidpid();

    match write_config(vendor_id, product_id) {
        Ok(path) => {
            // ── INSERT 2: if VID/PID changed, reset + re-handshake for the ──
            //    newly-selected board (Bug 4 / PRD ID 3). reset_handshake_state
            //    clears HOST_CAPABLE/BOARD_HAS_RULES/CALLBACK_NAMES/HAS_HANDSHAKED;
            //    perform_handshake then re-runs (its HAS_HANDSHAKED guard was just
            //    cleared) and reads config.toml fresh, so the just-written VID/PID
            //    selects the new board and rebuilds its name→id map. `false` =
            //    non-verbose (verbose is not in scope here — bug_findings.md §132).
            if (vendor_id, product_id) != (old_vid, old_pid) {
                crate::core::notifier::reset_handshake_state();
                crate::core::notifier::perform_handshake(false);
            }

            let outcome = apply_device_rule(vendor_id, product_id);
            let detail = match outcome {
                ApplyOutcome::AutoDiscovery => {
                    "Auto-discovery in effect (any standard QMK keyboard).".to_string()
                }
                ApplyOutcome::Applied => "Device rule applied.".to_string(),
                ApplyOutcome::NeedsManual(how) => how,
            };
            notify(
                "QMKonnect — settings saved",
                &format!(
                    "vendor_id = {vid_str}, product_id = {pid_str}\n{detail}\n{}",
                    path.display()
                ),
            );
        }
        Err(e) => {
            eprintln!("Settings: failed to write config: {}", e);
            notify("QMKonnect — could not save", &e.to_string());
        }
    }
}
```

> The two insertions are the **only** changes. The `apply_device_rule` call, the `detail`
> match, the `notify` calls, and the `Err` arm are all byte-for-byte unchanged.

### Success Criteria
- [ ] Pre-save snapshot `(old_vid, old_pid)` is taken from `current_config_vidpid()` BEFORE
      the `match write_config(...)` line (~L725) — i.e. before the config is overwritten.
- [ ] The reset+handshake block is the FIRST statement inside the `Ok(path) => {` arm
      (before `let outcome = apply_device_rule(...)`).
- [ ] The block is guarded by `(vendor_id, product_id) != (old_vid, old_pid)`.
- [ ] Uses fully-qualified `crate::core::notifier::reset_handshake_state()` /
      `::perform_handshake(false)`.
- [ ] No change to `save_and_notify`'s signature, no new `use`, no edit to its two call
      sites (L856, L925), no edit outside `save_and_notify`.
- [ ] On the Linux dev box: `cargo build` clean; `cargo test --bin qmkonnect --
      --test-threads=1` green.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can make the two insertions using only the exact
code above, the verified line anchors, the two contract corrections (`current_config_vidpid`
name + `perform_handshake(false)`), and the Linux-dev-box validation gate — all present in
this PRP.

### Documentation & References

```yaml
# MUST READ — the bug being fixed
- docfile: plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/architecture/bug_findings.md
  why: Bug 4 (PRD ID 3) root cause + the EXACT recommended fix; line 132 settles the `verbose` question
  section: "Bug 4 (Minor, PRD ID 3): Settings VID/PID change doesn't reset handshake"
  critical: "verbose is not in scope here — pass false or add a param" → this PRP passes false;
            Linux location is save_and_notify @ L718 (after write_config succeeds)
- url: spec/PRD.md (heading h2.3/h3.3 "Issue 1: Settings-dialog VID/PID change does not reset the handshake")
  why: user-facing repro (two capable boards, pick B, rules use A's name map); lists src/linux_tray.rs (save_and_notify)

# MUST READ — the file & function being edited
- file: src/linux_tray.rs
  why: save_and_notify to patch; confirm line anchors (fn L718, match write_config L725, Ok arm L726)
  pattern: "save_and_notify(vendor_id, product_id) → [vid_str/pid_str] → match write_config(vendor_id, product_id)
            { Ok(path) => { apply_device_rule; detail; notify } Err(e) => { eprintln; notify } }"
  gotcha: "write_config OVERWRITES config.toml, so current_config_vidpid() (which reads the file) MUST be
           called BEFORE the match — not inside the Ok arm, where the config is already the new value."

# MUST READ — the pre-save VID/PID helper (CONTRACT CORRECTION: it is NOT `current_vidpid`)
- file: src/linux_tray.rs
  why: the helper used for the snapshot
  pattern: "fn current_config_vidpid() -> (Option<u16>, Option<u16>) @ L1006 — reads the first existing config
            candidate (get_config_paths() → parse_config()) and returns (cfg.vendor_id, cfg.product_id), or
            (None, None) when no config exists (fresh install ⇒ auto-discovery). Private (same module as
            save_and_notify ⇒ callable directly, no `use`)."
  gotcha: "The task contract calls this `current_vidpid()` — that name does NOT exist. Use current_config_vidpid()."

# MUST READ — the two notifier functions being called (pub, fully-qualified → no `use` needed)
- file: src/core/notifier.rs
  why: confirm signatures + behavior so the call is correct
  pattern: "reset_handshake_state() @ L814 (clears HOST_CAPABLE/BOARD_HAS_RULES/CALLBACK_NAMES/HAS_HANDSHAKED);
            perform_handshake(verbose: bool) @ L353 → perform_handshake_with @ L509: idempotent via
            HAS_HANDSHAKED.swap (L511), reads configured_filter() fresh (L521, fn @ L83), drops notifier lock
            before the sweep (L555). reset() clears HAS_HANDSHAKED first so perform_handshake RE-RUNS."
  gotcha: "perform_handshake takes a `bool` — verbose is NOT in scope in save_and_notify (nor in its two
           call sites), so pass `false`. Do NOT add a `verbose` param to save_and_notify (would force
           editing the L856/L925 call sites for no benefit)."

# REFERENCE — the sibling fixes (the pattern this task mirrors 1:1) + other save paths (do NOT edit)
- docfile: plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/P1M2T1S1/PRP.md
  why: the Windows save-block fix — identical two-insertion design, same `perform_handshake(false)`,
       same idempotency reasoning. This task is the Linux twin (simpler: no move-semantics, compiles on dev box).
- docfile: plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/P1M2T1S2/PRP.md
  why: the macOS save-arm fix — parallel sibling editing src/tray.rs (different file, no overlap).
- file: src/tray.rs (show_settings_dialog @838 Windows save arm, show_settings_dialog_with_pool @1648 macOS) — siblings S1/S2, DO NOT edit
- file: src/tray.rs:455/458 — the EXISTING device-transition callsite of the same reset/perform pair
  (the pattern to mirror; there `verbose` IS in scope because it's inside setup_tray's poll loop)
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect  (this file IS compiled on Linux — no cfg-gate caveat)
src/linux_tray.rs         # EDIT: 2 insertions in save_and_notify (L718–748)
  - L718  fn save_and_notify(vendor_id: Option<u16>, product_id: Option<u16>)  <- verbose NOT a param
  - L719-724  vid_str / pid_str formatting
  - L725  match write_config(vendor_id, product_id) {                          <- snapshot goes BEFORE this
  - L726    Ok(path) => { … }                                                  <- reset+handshake at TOP of this arm
  - L727      let outcome = apply_device_rule(vendor_id, product_id);          <- (unchanged)
  - L856  save_and_notify(Some(v), Some(p));   <- picker-path call site (DO NOT change)
  - L925  save_and_notify(vid, pid);           <- forms-path call site (DO NOT change)
  - L1006 fn current_config_vidpid() -> (Option<u16>, Option<u16>)             <- the snapshot helper (NOT `current_vidpid`)
  - L1022 fn write_config(vendor_id, product_id) -> Result<PathBuf, Box<dyn Error>>
src/core/notifier.rs      # READ ONLY: reset_handshake_state @814, perform_handshake @353 (verbose:bool)
src/main.rs:17            # READ ONLY: `mod linux_tray;` (no cfg gate ⇒ compiled + type-checked on Linux)
```

### Desired Codebase tree
**Only `src/linux_tray.rs` changes** — two insertions inside `save_and_notify`. No new files,
no signature changes, no new imports, no call-site edits, no new tests.

### Known Gotchas of our codebase & Library Quirks
```rust
// CRITICAL (snapshot timing): current_config_vidpid() READS config.toml from disk. write_config() OVERWRITES
// it. So the snapshot MUST precede the `match write_config(...)` line — placing it inside the Ok(path) arm
// (after write_config) would read the NEW values and the diff check `(new) != (old)` would always be false.
// (Compare S1/S2, where the hazard was a Rust move; here it is a filesystem write.)

// CRITICAL (helper name): the helper is `current_config_vidpid()`, NOT `current_vidpid()`. The task contract
// mislabeled it. grep confirms `current_vidpid` does not exist anywhere in linux_tray.rs.

// CRITICAL (verbose): perform_handshake(verbose: bool) needs a bool, but `verbose` is NOT in scope in
// save_and_notify (nor in the two callers at L856/L925). Pass `false`. Do NOT add a `verbose` param to
// save_and_notify — it would force editing both call sites for no benefit and risk diverging from siblings.
// (bug_findings.md §132)

// ADVANTAGE (platform): linux_tray.rs is the Linux SNI tray and IS compiled/type-checked on the Linux dev
// box (src/main.rs:17 `mod linux_tray;`, no cfg gate). So `cargo build` + `cargo test` HERE are definitive —
// unlike S1 (#[cfg(windows)]) and S2 (#[cfg(macos)]), which are cfg-gated out on Linux.

// GOTCHA (idempotency): perform_handshake is guarded by `if HAS_HANDSHAKED.swap(true) { return; }` (L511).
// The PRECEDING reset_handshake_state() sets HAS_HANDSHAKED=false, so the swap returns false and the
// handshake RE-RUNS. Calling perform_handshake WITHOUT reset first would no-op. Order matters.

// GOTCHA (fresh config): perform_handshake reads configured_filter() FRESH (notifier.rs:521/83), so the
// VID/PID written by write_config IS used. No cache to invalidate.

// GOTCHA (threading): this runs synchronously on the tray's SNI service thread (the blocking zenity dialog
// just returned). perform_handshake is bounded (CALLBACK_SWEEP_DEADLINE for a buggy board; ~ms for a real
// one) and releases the notifier lock per sweep iteration (notifier.rs:555), so it cannot deadlock or
// starve notifications.

// GOTCHA (tests): no unit test is added — save_and_notify writes a real config (write_config → atomic_write
// to a platform config path), shells out to apply_device_rule (pkexec/udevadm) + notify, and calls the
// global handshake. Not unit-testable without heavy mocking (matches S1/S2; verify via integration per
// bug_findings.md). The existing linux_tray.rs `mod tests` @ L1169 covers pure helpers only — unchanged.
```

## Implementation Blueprint

### Data models and structure
No data models. The edit reads the pre-save VID/PID via `current_config_vidpid()` (returns
`(Option<u16>, Option<u16>)`, all `Copy`) and calls two existing `pub` notifier functions.
`vendor_id`/`product_id` (the new values, `Option<u16>`, the function params) are unchanged.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: INSERT the pre-save VID/PID snapshot (src/linux_tray.rs, before `match write_config`)
  - IMPLEMENT: one line, immediately BEFORE `match write_config(vendor_id, product_id) {` (~L725):
        let (old_vid, old_pid) = current_config_vidpid();
  - WHY: current_config_vidpid() reads config.toml from disk; write_config() (the next line) overwrites it.
    Snapshotting now captures the PRE-save values needed for the post-save diff. (None,None) on a fresh
    install is fine — the new value will differ and trigger the reset, which is correct for first-config.
  - PLACEMENT: src/linux_tray.rs, inside save_and_notify, after the vid_str/pid_str formatting block and
    before the `match` line. 4-space indentation (top level of the function body).
  - NAMING: MUST be current_config_vidpid() — NOT current_vidpid() (which does not exist).

Task 2: INSERT the reset + re-handshake block (src/linux_tray.rs, at the TOP of the Ok(path) arm)
  - IMPLEMENT: as the FIRST statement inside `Ok(path) => {` (before `let outcome = apply_device_rule(...)`):
        if (vendor_id, product_id) != (old_vid, old_pid) {
            crate::core::notifier::reset_handshake_state();
            crate::core::notifier::perform_handshake(false);
        }
  - WHY: this arm only runs on a successful write (the Err arm handles write failure). reset clears
    HAS_HANDSHAKED (so perform_handshake's idempotent guard lets it re-run) + CALLBACK_NAMES;
    perform_handshake reads config.toml fresh → targets the newly-selected board → rebuilds its name→id map.
    Comparing the NEW params against the OLD snapshot (not re-reading the file, which now holds the new value).
  - NAMING/STYLE: fully-qualified crate::core::notifier:: paths (no new `use`). `false` for verbose (verbose
    is NOT in scope — see contract fact #2). Add a brief `//` comment citing Bug 4.
  - PLACEMENT: src/linux_tray.rs, the Ok(path) => { … } arm of save_and_notify's match, as its first
    statement. 12-space indentation (inside `Ok(path) => {`).
  - DEPENDENCIES: Task 1 (uses old_vid/old_pid).

Task 3: VALIDATE — Linux dev box (DEFINITIVE for this edit) + integration
  - LINUX DEV BOX (this box — compiles + type-checks linux_tray.rs; this IS the definitive gate):
        cargo build
        cargo test --bin qmkonnect -- --test-threads=1
    Expected: clean build; all existing tests green. (Unlike S1/S2, NO platform-host caveat —
    linux_tray.rs is compiled here, so a green build proves the edit type-checks.)
  - INTEGRATION (the actual feature): two capable boards A+B → start (handshake A) → Settings → pick B →
    save → B's callback map is live (no replug). Single-board / unchanged save → no reset fires.

Task 4: NEVER do these (out of scope / forbidden)
  - DO NOT change save_and_notify's signature or add a `verbose` param (would force editing the L856/L925
    call sites for no benefit). Use perform_handshake(false).
  - DO NOT use a helper named `current_vidpid` — it does not exist. Use current_config_vidpid().
  - DO NOT place the snapshot INSIDE the Ok(path) arm / after write_config — write_config overwrites the
    file; the snapshot must precede `match write_config`.
  - DO NOT edit the Windows/macOS save paths (src/tray.rs) — those are siblings S1/S2.
  - DO NOT add a unit test for save_and_notify (writes real config + shells out — not unit-testable) or
    spawn a handshake thread (contract + bug-findings specify a synchronous call).
  - DO NOT edit PRD.md, any tasks.json, prd_snapshot.md, or any file other than src/linux_tray.rs.
```

### Implementation Patterns & Key Details
```rust
// The device-transition callsite already mirrors this exact pair (src/tray.rs:455/458, inside setup_tray's
// poll loop where `verbose` IS in scope). This task reuses the SAME two calls from the Linux Settings save
// path, the only differences being `false` (verbose not in scope) and the VID/PID-diff guard.

// Order is load-bearing: reset FIRST (clears HAS_HANDSHAKED), THEN perform_handshake (whose guard would
// otherwise no-op). reset also clears CALLBACK_NAMES so the stale old-board map cannot leak.

// Diff against the SNAPSHOT, not a re-read: after write_config, current_config_vidpid() would return the
// new values. Compare the in-scope new params (vendor_id, product_id) against old_vid/old_pid directly.

// Indentation: save_and_notify is flat (no unsafe/match nesting like S2's macOS arm) — snapshot at 4 spaces,
// reset block at 12 spaces (inside `Ok(path) => {`).
```

### Integration Points
```yaml
IMPORTS: none. Fully-qualified crate::core::notifier::{reset_handshake_state, perform_handshake} paths.
HELPER:  current_config_vidpid() @ src/linux_tray.rs:1006 — same module as save_and_notify, callable directly.
CALLS:   reset_handshake_state() (notifier.rs:814) + perform_handshake(false) (notifier.rs:353).
         perform_handshake → configured_filter() reads config.toml fresh (notifier.rs:521/83) → the
         just-written VID/PID selects the new board.
DOWNSTREAM: none. (The Windows/macOS save paths are siblings S1/S2 — independent functions/files.)
PARALLEL:   P1.M2.T1.S2 edits src/tray.rs (macOS arm) — a different FILE, zero overlap.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)
```bash
cd /home/dustin/projects/qmkonnect
cargo build                 # DEFINITIVE for this edit: linux_tray.rs IS compiled on Linux.
# Expected: clean build (save_and_notify type-checks with the two new insertions).
```

### Level 2: Tests (Regression — AGENTS.md mandates single-threaded)
```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: all existing tests pass (the new code is in a non-testable side-effecting path; no new test).
# --test-threads=1 is REQUIRED (AGENTS.md — shared global debouncer state).
```

### Level 3: Build artifacts (if running the full Linux app loop)
```bash
# Optional — only if you intend to launch the SNI tray app on Linux (AGENTS.md has no Linux packaging loop,
# but the binary runs directly). The edit needs no packaging change.
cd /home/dustin/projects/qmkonnect
cargo build --release
# Expected: clean release build.
```

### Level 4: Manual Feature Verification (the actual fix)
```
Precondition: TWO QMK keyboards flashed with the qmk_notifier module ("capable" boards A and B), Linux/Wayland
host with an SNI-hosting status bar (Waybar/SwayNC/KDE/etc.).
1. Plug in A and B. Start QMKonnect (handshake runs for A — A's CALLBACK_NAMES built).
2. Open tray → Settings. The zenity --list picker shows A and B (≥1 capable board, not clean-auto).
3. Select board B's row, click OK (→ save_and_notify(Some(B_vid), Some(B_pid)) @ L856).
4. EXPECT: reset_handshake_state() + perform_handshake(false) fire (the (vendor_id,product_id) != (old_vid,old_pid)
   guard is true): B's rules now resolve against B's callback map (not A's). No replug/restart needed.
5. Unchanged-save regression: open Settings, fall through to the --forms (or re-pick the SAME board), leave the
   values as-is, OK → NO reset fires (the `(vendor_id, product_id) != (old_vid, old_pid)` guard is false).
6. First-config sanity: on a fresh install (no config.toml ⇒ current_config_vidpid() = (None,None)), set a
   VID/PID → the guard is true → reset+handshake fire (correct: first handshake for the newly-targeted board).
```

## Final Validation Checklist

### Technical Validation
- [ ] Linux dev box: `cargo build` clean; `cargo test --bin qmkonnect -- --test-threads=1` green
      (this IS definitive — linux_tray.rs compiles here; no platform-host caveat).
- [ ] `git diff --stat` shows ONLY `src/linux_tray.rs`.

### Feature Validation
- [ ] Pre-save snapshot via `current_config_vidpid()` precedes `match write_config(...)` (write-safe).
- [ ] Reset+handshake block is the FIRST statement in the `Ok(path) =>` arm (runs only on successful write).
- [ ] Block guarded by `(vendor_id, product_id) != (old_vid, old_pid)`.
- [ ] Manual multi-board test (A→B switch) rebuilds B's callback map without replug.
- [ ] Unchanged save does NOT reset (guard false). First-config save DOES reset (old=(None,None)).

### Code Quality Validation
- [ ] Uses `current_config_vidpid()` (NOT the nonexistent `current_vidpid`).
- [ ] Fully-qualified `crate::core::notifier::` paths (no new `use`).
- [ ] `perform_handshake(false)` — no signature change, no `verbose` param, no call-site edits.
- [ ] No edit outside `save_and_notify` in `src/linux_tray.rs`.
- [ ] Brief `//` comment citing Bug 4 / PRD ID 3 on the new block.
- [ ] Indentation matches the surrounding lines (4-space snapshot, 12-space reset block).

### Documentation & Deployment
- [ ] No user-facing/config/API/CLI change (internal handshake lifecycle — DOCS: none per contract).
- [ ] Inline comment explains the reset+re-handshake rationale for future readers.

---

## Anti-Patterns to Avoid
- ❌ Don't use a helper called `current_vidpid` — it doesn't exist. The helper is
  `current_config_vidpid()` (L1006). The task contract mislabeled it.
- ❌ Don't place the snapshot AFTER `write_config` (or inside the Ok arm) — write_config overwrites
  config.toml, so current_config_vidpid() would then read the NEW values and the diff check is always
  false. Snapshot BEFORE `match write_config`.
- ❌ Don't pass `verbose` to `perform_handshake` — it's not in scope (save_and_notify and its two call
  sites take only vid/pid). Pass `false`. Don't add a `verbose` param (useless call-site churn).
- ❌ Don't call `perform_handshake` WITHOUT `reset_handshake_state()` first — the `HAS_HANDSHAKED` guard
  would no-op it. Order is load-bearing.
- ❌ Don't treat this like S1/S2's platform-gate caveat — `linux_tray.rs` IS compiled on the Linux dev
  box, so a green `cargo build` here genuinely proves the edit type-checks. Validate locally.
- ❌ Don't edit the Windows/macOS save paths (`src/tray.rs`) — those are siblings S1/S2.
- ❌ Don't add a unit test for `save_and_notify` (writes real config + shells out to pkexec/udevadm/zenity
  — not unit-testable) or spawn a handshake thread.
- ❌ Don't edit PRD.md, tasks.json, prd_snapshot.md, or any file other than `src/linux_tray.rs`.

---

## Confidence Score: 9/10

The change is tiny (two insertions, ~7 lines) and fully verified. Unlike siblings S1/S2, it
is **simpler** (no `current_config` move hazard — the new vid/pid are function params, the
old pair read fresh from disk) and **lower-risk** (`src/linux_tray.rs` is compiled and
type-checked on the Linux dev box, so `cargo build` + `cargo test` here are *definitive* —
no platform-host caveat). Both notifier functions exist and behave as required (idempotency
guard cleared by the preceding reset; fresh config read; no deadlock/starvation), the
save_and_notify anchors are confirmed (fn L718, match L725, Ok arm L726), and the snapshot
timing (before write_config) is correct. The score is 9 rather than 10 only because the
literal task contract contained a wrong helper name (`current_vidpid`) and the `verbose`
question required resolution to `false` — both explicitly corrected and justified in this
PRP, so the residual risk to one-pass success is minimal.
# PRP — P4.M2.T1.S2: Integrate handshake into startup and device-status poll

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This 1-point task **wires** the capability
> handshake (produced by P4.M2.T1.S1) into the app lifecycle: (a) each runner
> calls `perform_handshake(verbose)` right after `startup_device_probe` when a
> device is already connected, and (b) the two device-status **poll threads**
> (`src/tray.rs` macOS/Windows, `src/linux_tray.rs` Linux) run the handshake on a
> `false→true` transition and `reset_handshake_state()` on a `true→false`
> transition, so the handshake fires **at most once per board boot** and re-runs
> on reconnect. A tiny pure `handshake_action(prev, now)` helper (+ `HandshakeAction`
> enum + 1 test) is added to `src/core/notifier.rs` to make the directional logic
> unit-testable on the Linux dev box and DRY across the 3 call sites.
>
> **Consumes (treat as a COMPLETED contract — `plan/002_637d65b6e9b8/P4M2T1S1/PRP.md`):**
> `pub fn perform_handshake(verbose: bool)` (idempotent via `HAS_HANDSHAKED`),
> `pub fn reset_handshake_state()`, `pub fn is_device_connected()`
> (notifier.rs:169), `pub fn startup_device_probe(verbose)` (notifier.rs:119).
> (At research time S1's symbols are not yet in notifier.rs — expected; this task
> appends after them once S1 lands.)
>
> **Consumed downstream by:** **P4.M3.T1.S1** (host-context send reads
> `host_capable()`/`callback_names()` — this task is what *populates* them at
> connect time) and the tray "Reload rules" item (P5.M2) which will re-handshake.
>
> **PARALLEL-EXECUTION NOTE:** P4.M2.T1.S1 is being implemented concurrently and
> owns the handshake *block* in `notifier.rs` (L182–184 band). This task APPENDS a
> small helper + 1 test AFTER S1's `reset_handshake_state`. S1 is **complete**
> when S2 implements ⇒ no merge conflict. This task also edits `runners/*.rs`,
> `tray.rs`, `linux_tray.rs` — none of which S1 (or any sibling) touches.

---

## ⚠️ READ FIRST — two non-obvious traps

1. **`setup_tray()` and `linux_tray::spawn()` take NO `verbose` arg today**
   (tray.rs:250, linux_tray.rs:231). You MUST add `verbose: bool` to BOTH and
   update all **5 call sites** (G1). A missed site = compile error (good).
2. **The naive `match (prev,now) { (Some(true),false)=>Loss, (_,true)=>Gain, _=>None }`
   is WRONG** — it mis-classifies `(Some(true), true)` (no change) as `Gain`.
   Use the **guarded** Gain arm `(p, true) if p != Some(true)` (see Data Models).
   The unit test (Task 7) pins this.

---

## Goal

**Feature Goal**: Make the capability handshake run automatically at the right
moments — once at startup if a device is present, and again whenever the
device-status poll detects a real `false→true` transition (reconnect), while
resetting the dedup guard on `true→false` (disconnect) so the next reconnect
re-runs it. After this task, `host_capable()` / `callback_names()` are populated
for the lifetime of a board boot with **zero user action**, and P4.M3's
host-context send path has its gating state ready.

**Deliverable** (edits to 5 existing files + 1 appended helper/test in
`notifier.rs`; **no new files, no Cargo, no CLI/tray-menu work**):
1. `src/core/notifier.rs` — append `pub enum HandshakeAction { None, Gain, Loss }`
   + `pub fn handshake_action(prev: Option<bool>, now: bool) -> HandshakeAction`
   (after S1's `reset_handshake_state`) + 1 unit test in `mod tests`.
2. `src/runners/windows.rs` — conditional handshake after the probe at **L48**
   (console) and **L95** (tray); pass `self.verbose` to `tray::setup_tray()` at L108.
3. `src/runners/macos.rs` — conditional handshake after the probe at **L27**;
   pass `self.verbose` to `crate::tray::setup_tray()` at L44.
4. `src/runners/linux.rs` — conditional handshake after the probe at **L27**;
   pass `self.verbose` to `crate::linux_tray::spawn()` at L46 & L60 and to
   `crate::tray::setup_tray()` at L70.
5. `src/tray.rs` — `setup_tray()` → `setup_tray(verbose: bool)`; add directional
   handshake dispatch to the poll thread (L368–385).
6. `src/linux_tray.rs` — `spawn()` → `spawn(verbose: bool)`; add directional
   handshake dispatch to the poll thread (L248–272), **outside** the `update` closure.

**Success Definition**:
- On launch with a capable device connected: `perform_handshake` runs once at
  startup (runner); the poll thread's first tick is a dedup no-op (`HAS_HANDSHAKED`).
- On hot-plug while running: the poll thread detects `false→true` and runs
  `perform_handshake` (on its own thread — UI never blocks).
- On hot-unplug: the poll detects `true→false` and calls `reset_handshake_state`
  (clears `HOST_CAPABLE`/`CALLBACK_NAMES`/`HAS_HANDSHAKED`); the next plug re-runs.
- `cargo build --bin qmkonnect` clean (Linux, default features ⇒ compiles
  `linux_tray.rs`); `cargo test --bin qmkonnect -- --test-threads=1` green
  (new `test_handshake_action_transitions` + S1's 8 + all existing).
- `git diff --stat` shows exactly: `src/core/notifier.rs`, `src/runners/{windows,macos,linux}.rs`, `src/tray.rs`, `src/linux_tray.rs`.

## User Persona (if applicable)

**Target User**: the end user (transparent) + the downstream P4.3/P5 implementers.
**Use Case**: "I plug in my QMK keyboard and QMKonnect *just knows* it speaks
typed commands and what its callbacks are — I never run a command. When I unplug
and replug, it rediscovers."
**Pain Points Addressed**: today there is no trigger for the handshake, so the
state S1 computes is never populated at runtime. This task is the trigger.

## Why

- **PRD §8(5) / HOST_RULES.md §8(5)** — "Near `startup_device_probe`, once a
  device is connected … The handshake runs **at most once per board boot** …
  Re-trigger only on a real device transition via the existing
  `is_device_connected()` poll." This task IS that wiring.
- **PRD §5.6 / §4 (`h3.15`, `h2.43`)** — the status probe "runs on a background
  thread (3 s macOS/Windows, 1 s Linux) and only fires a UI update on a
  transition." This task adds the handshake lifecycle to exactly those transition
  points without altering the UI-update cadence.
- **PRD §8(8)** — backward compat: legacy firmware ⇒ `perform_handshake` sets
  `HOST_CAPABLE=false` (string-only). This task invokes it at the same moments
  regardless; the handshake itself decides capability.
- **Unblocks P4.M3.T1.S1** (needs `host_capable()` populated at runtime).

## What

Additive wiring + one tiny pure helper. **No change** to `perform_handshake` /
`reset_handshake_state` internals (S1 owns those), the debounce worker, the
`Notifier` trait, `notify_qmk`, CLI, or tray menu items. The handshake is invoked
on background/poll threads (non-blocking to UI) and at startup (before the poll
thread exists).

### Success Criteria
- [ ] **`handshake_action` helper** in `notifier.rs` classifies all 6
      `(Option<bool>, bool)` rows per the table in Implementation Blueprint; the
      Gain arm is **guarded** (`(p, true) if p != Some(true)`) so `(Some(true),true)` ⇒ None.
- [ ] **4 runner probe sites** (windows×2, macos×1, linux×1): immediately after
      `startup_device_probe(self.verbose)`, `if is_device_connected() {
      perform_handshake(self.verbose); }`.
- [ ] **tray.rs poll thread**: on `last != Some(connected)`, dispatch
      `handshake_action(last, connected)` → Gain⇒`perform_handshake(verbose)`,
      Loss⇒`reset_handshake_state()`, None⇒{}; then the existing
      `send_event(DeviceStatus(connected))` runs unchanged.
- [ ] **linux_tray.rs poll thread**: the SAME dispatch, in the poll-loop body
      **before** the combined device/dark guard, and **outside** the
      `poll_handle.update(…)` closure (G2).
- [ ] **5 call sites** pass `self.verbose` to the now-parameterized `setup_tray`/`spawn`.
- [ ] `handshake_action` runs on the **poll thread** in both trays (never the UI
      event loop, never the ksni D-Bus thread).
- [ ] `git diff --stat` = the 6 files listed above; nothing else.

## All Needed Context

### Context Completeness Check

_Pass_: an agent with no prior knowledge can implement this using only this PRP +
`research/notes.md`, because: (a) the EXACT current anchors (verbatim code + line
numbers for all 4 runner probe sites, both poll threads, both `setup_tray`/`spawn`
signatures, all 5 call sites) are in `research/notes.md` §0; (b) the transition
semantics table (all 6 rows) is in §2; (c) the pure helper's full code — including
the **corrected guarded Gain arm** that avoids the `(Some(true),true)` mis-class
bug — is in §3; (d) 10 gotchas (G1–G10) cover verbose plumbing, D-Bus-thread HID
safety, startup+poll idempotency, reset-only-on-real-Loss, Linux default features,
cfg-gate preservation, reassign ordering, no-S1-conflict, and start ordering; (e)
the start-ordering proof (handshake completes before the poll thread spawns) is in
§6; (f) verified validation commands are in §7.

### Documentation & References

```yaml
# MUST READ — the verbatim research (THIS task's contract + design + safety proofs)
- file: plan/002_637d65b6e9b8/P4M2T1S2/research/notes.md
  why: "§0 = exact current anchors (line numbers + verbatim code for all 4 runner
        probe sites, both poll threads, both signatures, all 5 call sites). §2 =
        transition table (6 rows). §3 = the helper code INCLUDING the guarded Gain
        arm (avoids the (Some(true),true) mis-class bug). §5 = 10 gotchas (G1–G10).
        §6 = start-ordering proof. §7 = validation commands."

# MUST READ — the S1 contract (perform_handshake / reset_handshake_state / HAS_HANDSHAKED)
- file: plan/002_637d65b6e9b8/P4M2T1S1/PRP.md
  why: "S1 is the producer. This task treats it as COMPLETE. Its `perform_handshake(verbose)`
        is idempotent (HAS_HANDSHAKED.swap short-circuit); its `reset_handshake_state()`
        clears all 3 statics; its `host_capable()`/`callback_names()` are what this
        task's triggers POPULATE for P4.M3. Do NOT reimplement any of S1's logic."

# MUST READ — the canonical handshake semantics (the spec this task operationalizes)
- file: spec/HOST_RULES.md
  why: "§8(5) is the canonical pseudocode: 'Near startup_device_probe, once a device
        is connected … runs at most once per board boot … Re-trigger only on a real
        device transition via the existing is_device_connected() poll.' This task IS
        the trigger wiring. §8(8) = backward-compat (legacy ⇒ string-only, automatic
        via perform_handshake's else-branch)."
  section: "§8(5) (Startup handshake + SET_OS), §8(8) (Backward compat)"

# MUST READ — the files THIS task edits
- file: src/runners/windows.rs
  why: "L48 (run_console_mode) + L95 (run_tray_app) = the two startup_device_probe
        sites; L108 = tray::setup_tray() call site. All have self.verbose in scope."
  pattern: "after each `crate::core::notifier::startup_device_probe(self.verbose);`
            add the conditional handshake block; change L108 to setup_tray(self.verbose)."
- file: src/runners/macos.rs
  why: "L27 = startup_device_probe site; L44 = crate::tray::setup_tray() call site."
- file: src/runners/linux.rs
  why: "L27 = startup_device_probe site; L46 + L60 = crate::linux_tray::spawn() call
        sites (hyprland + non-hyprland paths); L70 = crate::tray::setup_tray() (the
        non-linux-tray fallback, feature-gated). All have self.verbose in scope."
- file: src/tray.rs
  why: "L250 = setup_tray() signature (NO args → add verbose); L368–385 = the macOS/Windows
        poll thread (cfg-gated). proxy is tao EventLoopProxy<UserEvent> (~L274). The
        DeviceStatus(connected) event-loop arm (~L481) is UNCHANGED."
  gotcha: "the poll block is #[cfg(any(target_os='macos',target_os='windows'))] — NOT
           compiled on Linux. Validate by symmetry + macOS/Windows dev loops (G6)."
- file: src/linux_tray.rs
  why: "L231 = spawn() signature (NO args → add verbose); L248–272 = the Linux poll
        thread. Dispatch = poll_handle.update(|t:&mut QmkTray|{…}) which runs on ksni's
        D-Bus thread. QmkTray struct L67–70. DEVICE_POLL_INTERVAL=1s (L57)."
  gotcha: "perform_handshake MUST run in the poll-loop body, NOT inside the update
           closure (G2 — HID I/O on the D-Bus thread wedges the tray icon)."

# MUST READ — where the helper + test land (APPEND ONLY; S1 owns the block above it)
- file: src/core/notifier.rs
  why: "S1 adds perform_handshake/reset_handshake_state/host_capable/etc. at the
        L182–184 band. This task APPENDS handshake_action + HandshakeAction AFTER
        reset_handshake_state, and appends 1 test in mod tests. is_device_connected
        (L169) + startup_device_probe (L119) are called unchanged."
  pattern: "additive append; do NOT modify S1's handshake code or DebounceState/worker/notify_qmk."

# Reference — the dev test loops (for validating the macos/windows cfg blocks)
- file: AGENTS.md
  why: "the macOS loop (cargo test → packaging/macos clean/build/install → open
        /Applications/QMKonnect.app) and Windows loop (cargo test → build → taskkill
        → run exe) are how the cfg'd-out-on-Linux tray.rs edits get validated on
        their real platforms."
```

### Current Codebase tree (relevant subset)

```bash
src/
  core/
    notifier.rs   # S1 adds the handshake block at L182–184. THIS TASK APPENDS:
                  #   + HandshakeAction enum + handshake_action fn (after reset_handshake_state)
                  #   + 1 test in mod tests
                  # is_device_connected (L169) + startup_device_probe (L119) called unchanged.
    mod.rs        # now_ms(). UNCHANGED.
  runners/
    mod.rs        # PlatformRunner trait, create_runner. UNCHANGED.
    windows.rs    # ← EDIT: handshake after L48 + L95; setup_tray(self.verbose) at L108.
    macos.rs      # ← EDIT: handshake after L27; setup_tray(self.verbose) at L44.
    linux.rs      # ← EDIT: handshake after L27; spawn(self.verbose) at L46+L60; setup_tray(self.verbose) at L70.
  tray.rs         # ← EDIT: setup_tray(verbose) signature (L250); poll thread handshake dispatch (L368–385).
  linux_tray.rs   # ← EDIT: spawn(verbose) signature (L231); poll thread handshake dispatch (L248–272, outside update closure).
Cargo.toml        # L100 default = ["hyprland","macos","linux-tray"]. UNCHANGED.
```

### Desired Codebase tree with files to be changed

```bash
src/core/notifier.rs     # MODIFIED (additive append): + HandshakeAction, + handshake_action, + 1 test.
src/runners/windows.rs   # MODIFIED: +2 handshake blocks, 1 call-site arg.
src/runners/macos.rs     # MODIFIED: +1 handshake block, 1 call-site arg.
src/runners/linux.rs     # MODIFIED: +1 handshake block, 3 call-site args.
src/tray.rs              # MODIFIED: signature + poll-thread dispatch.
src/linux_tray.rs        # MODIFIED: signature + poll-thread dispatch.
# EVERYTHING ELSE UNCHANGED. No Cargo, no new files, no CLI/tray-menu, no S1 edits.
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (G1 — verbose plumbing): setup_tray() (tray.rs:250) and spawn()
//   (linux_tray.rs:231) take NO args today. Add `verbose: bool` to BOTH and update
//   all 5 call sites (windows.rs:108, macos.rs:44, linux.rs:46/60/70). A missed
//   site = compile error E0061 (wrong number of args) — fails loud, which is good.

// CRITICAL (G2 — handshake on POLL thread, not UI/D-Bus thread): perform_handshake
//   does HID I/O. In tray.rs run it inside the std::thread::spawn poll closure
//   (before send_event) — the tao event loop is NOT blocked. In linux_tray.rs run
//   it in the poll-loop BODY, NEVER inside poll_handle.update(|t:&mut QmkTray|{…})
//   — that closure runs on ksni's D-Bus thread; blocking it wedges the tray icon
//   (AGENTS.md: "if the icon looks dimmed/unclickable, the main thread is wedged").

// CRITICAL (G4 — reset only on real Loss): handshake_action(Some(true),false)=Loss.
//   None→false and Some(false)→false ⇒ None. A spurious reset would clear
//   HOST_CAPABLE mid-session. The test (Task 7) pins all 6 rows.

// GOTCHA (G6 — Linux default features): default = ["hyprland","macos","linux-tray"]
//   (Cargo.toml:100). A plain `cargo build`/`cargo test` on Linux compiles + tests
//   linux_tray.rs. tray.rs's macos/windows poll block is cfg'd OUT on Linux ⇒ NOT
//   compiled natively (validate by symmetry + AGENTS.md platform loops).

// GOTCHA (G8 — linux_tray reassign ordering): keep `last_device = Some(connected)`
//   INSIDE the existing combined `if last_device != Some(connected) || last_dark !=
//   Some(dark)` guard so dark-only changes still update the tray unchanged. The
//   directional handshake check reads last_device BEFORE that reassign.

// GOTCHA (G9 — no S1 conflict): S1 is COMPLETE when S2 implements. Append the
//   helper AFTER reset_handshake_state. Do NOT modify S1's perform_handshake/statics/mock.

// GOTCHA (G10 — start ordering): every runner does probe → handshake → THEN
//   setup_tray()/spawn() (which spawns the poll thread). So the startup handshake
//   finishes before the poll thread exists. The poll's first tick (None→true) calls
//   perform_handshake again but HAS_HANDSHAKED (set at startup) makes it a no-op.
```

## Implementation Blueprint

### Data models and structure

```rust
// ── src/core/notifier.rs — APPEND after S1's `reset_handshake_state` (L182–184 band) ──

/// What the host-rules handshake lifecycle should do for a device-status transition.
///
/// Computed by [`handshake_action`] from the previous and current
/// [`is_device_connected`] results, and consumed by the device-status poll
/// threads ([crate::tray] on macOS/Windows, [crate::linux_tray] on Linux) and
/// the startup path so the three call sites stay in lockstep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeAction {
    /// No transition, or `None → false` at startup: nothing to do.
    None,
    /// Device became present (`None → true` or `Some(false) → true`): run
    /// [`perform_handshake`]. Idempotent via [`HAS_HANDSHAKED`] — a no-op if the
    /// runner already handshooked at startup, so the poll thread's first tick on
    /// an already-connected device is harmless.
    Gain,
    /// Device went away (`Some(true) → false`): call [`reset_handshake_state`] so
    /// the next [`HandshakeAction::Gain`] re-runs the handshake.
    Loss,
}

/// Classify a device-status transition into a handshake lifecycle action.
///
/// Pure mapping; unit-tested without a device or UI thread. The poll threads call
/// this with their previous and latest [`is_device_connected`] results.
///
/// | `prev`          | `now`   | Action  |
/// | ----------------|---------|---------|
/// | `None`          | `true`  | `Gain`  | (startup already connected)
/// | `None`          | `false` | `None`  |
/// | `Some(false)`   | `true`  | `Gain`  | (reconnect)
/// | `Some(true)`    | `true`  | `None`  | (no change)
/// | `Some(false)`   | `false` | `None`  | (no change)
/// | `Some(true)`    | `false` | `Loss`  | (real disconnect)
pub fn handshake_action(prev: Option<bool>, now: bool) -> HandshakeAction {
    match (prev, now) {
        (Some(true), false) => HandshakeAction::Loss,           // real disconnect
        (p, true) if p != Some(true) => HandshakeAction::Gain,  // None→true OR false→true
        _ => HandshakeAction::None,                              // no change OR None→false
    }
}
// NOTE: the Gain arm MUST be guarded (`if p != Some(true)`) — the naive `(_, true)
// => Gain` would mis-classify (Some(true), true) as Gain. The test pins this.
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: ADD handshake_action + HandshakeAction to src/core/notifier.rs (APPEND after S1's reset_handshake_state)
  - INSERT: the enum + fn from Data Models, placed immediately AFTER `pub fn
    reset_handshake_state() { … }` (S1's last item in the L182–184 band). Mode-A
    rustdoc with the transition table.
  - DEPENDENCIES: none new (pure fn; Option<bool> + bool only). S1's items may be
    referenced in rustdoc links ([perform_handshake], [reset_handshake_state],
    [HAS_HANDSHAKED]) — they exist once S1 lands.
  - NAMING: `HandshakeAction` (CamelCase enum), `handshake_action` (snake_case fn).
  - GOTCHA: the Gain arm is GUARDED (`(p, true) if p != Some(true)`). Do NOT write
    `(_, true) => Gain` — it mis-classifies (Some(true),true).
  - GOTCHA G9: append ONLY; do not touch S1's perform_handshake/statics/mock/tests.
  - VERIFY: `grep -n 'pub enum HandshakeAction\|pub fn handshake_action' src/core/notifier.rs` -> 2 hits, both AFTER reset_handshake_state.

Task 2: ADD the handshake_action unit test to notifier.rs mod tests (APPEND)
  - APPEND after S1's 8 handshake tests:
      #[test]
      fn test_handshake_action_transitions() {
          assert_eq!(handshake_action(None, true), HandshakeAction::Gain);
          assert_eq!(handshake_action(None, false), HandshakeAction::None);
          assert_eq!(handshake_action(Some(false), true), HandshakeAction::Gain);
          assert_eq!(handshake_action(Some(true), false), HandshakeAction::Loss);
          assert_eq!(handshake_action(Some(true), true), HandshakeAction::None);
          assert_eq!(handshake_action(Some(false), false), HandshakeAction::None);
      }
    (bare names — mod tests has `use super::*`.)
  - VERIFY: `cargo test --bin qmkonnect handshake_action -- --test-threads=1` -> 1 passed.

Task 3: ADD the startup handshake to the 4 runner probe sites
  - windows.rs L48 (run_console_mode): immediately after
    `crate::core::notifier::startup_device_probe(self.verbose);` insert:
        if crate::core::notifier::is_device_connected() {
            crate::core::notifier::perform_handshake(self.verbose);
        }
  - windows.rs L95 (run_tray_app): SAME block.
  - macos.rs L27: SAME block.
  - linux.rs L27: SAME block.
  - WHY direct `if` (not the helper): matches the item wording ("call
    perform_handshake(verbose) if is_device_connected() returns true"); no Loss is
    possible at startup so the helper's bidirectionality is unneeded here.
  - GOTCHA G10: insert BEFORE the monitor.start()/setup_tray()/spawn() calls so the
    handshake completes before the poll thread exists (start-ordering proof §6).
  - VERIFY: `grep -rn 'perform_handshake(self.verbose)' src/runners/` -> 4 hits.

Task 4: PLUMB verbose through setup_tray() — signature + 3 call sites
  - src/tray.rs L250: `pub fn setup_tray() {` -> `pub fn setup_tray(verbose: bool) {`.
  - windows.rs L108: `tray::setup_tray();` -> `tray::setup_tray(self.verbose);`.
  - macos.rs L44: `crate::tray::setup_tray();` -> `crate::tray::setup_tray(self.verbose);`.
  - linux.rs L70: `crate::tray::setup_tray();` -> `crate::tray::setup_tray(self.verbose);`
    (this is the non-linux-tray fallback; still update for consistency — compiles only
    when feature linux-tray is OFF).
  - GOTCHA G1: a missed call site ⇒ E0061 (wrong number of args). All 3 callers are
    inside fn run/run_tray_app with self.verbose in scope.
  - VERIFY: `grep -rn 'setup_tray(' src/` -> def(1, with `verbose: bool`) + 3 callers each passing self.verbose.

Task 5: ADD directional handshake dispatch to the tray.rs poll thread (L368–385)
  - REPLACE the poll-thread closure body. Current:
        std::thread::spawn(move || {
            let mut last: Option<bool> = None;
            loop {
                let connected = crate::core::notifier::is_device_connected();
                if last != Some(connected) {
                    last = Some(connected);
                    let _ = status_proxy.send_event(UserEvent::DeviceStatus(connected));
                }
                std::thread::sleep(std::time::Duration::from_secs(3));
            }
        });
    NEW:
        std::thread::spawn(move || {
            let mut last: Option<bool> = None;
            loop {
                let connected = crate::core::notifier::is_device_connected();
                if last != Some(connected) {
                    // Handshake lifecycle on THIS poll thread (non-blocking to the
                    // UI event loop). Gain ⇒ perform_handshake (idempotent via
                    // HAS_HANDSHAKED if the runner already handshooked at startup);
                    // Loss ⇒ reset so the next gain re-runs.
                    match crate::core::notifier::handshake_action(last, connected) {
                        crate::core::notifier::HandshakeAction::Gain => {
                            crate::core::notifier::perform_handshake(verbose);
                        }
                        crate::core::notifier::HandshakeAction::Loss => {
                            crate::core::notifier::reset_handshake_state();
                        }
                        crate::core::notifier::HandshakeAction::None => {}
                    }
                    last = Some(connected);
                    let _ = status_proxy.send_event(UserEvent::DeviceStatus(connected));
                }
                std::thread::sleep(std::time::Duration::from_secs(3));
            }
        });
  - GOTCHA G2: the match runs in the spawn closure (poll thread) — NOT on the tao
    event loop. Good. (send_event is non-blocking.)
  - GOTCHA G6: this whole block is #[cfg(any(target_os="macos",target_os="windows"))]
    — NOT compiled on Linux. Keep the cfg gate intact.
  - VERIFY (on macos/windows): cargo build; on Linux: read-review only (cfg'd out).

Task 6: PLUMB verbose through spawn() + ADD directional dispatch to the linux_tray.rs poll thread (L231, L248–272)
  - 6a: src/linux_tray.rs L231: `pub fn spawn() -> Option<ksni::blocking::Handle<QmkTray>> {`
        -> `pub fn spawn(verbose: bool) -> Option<ksni::blocking::Handle<QmkTray>> {`.
  - 6b: linux.rs L46: `crate::linux_tray::spawn();` -> `crate::linux_tray::spawn(self.verbose);`.
        linux.rs L60: SAME.
  - 6c: in the poll-thread closure (L248–272), INSERT the directional dispatch in
        the poll-loop body BEFORE the combined device/dark guard, and OUTSIDE the
        poll_handle.update closure. Current loop body (after `tick = tick.wrapping_add(1);`):
            if last_device != Some(connected) || last_dark != Some(dark) {
                last_device = Some(connected);
                last_dark = Some(dark);
                let _ = poll_handle.update(|t: &mut QmkTray| {
                    t.device_connected = connected;
                    t.dark_mode = dark;
                });
            }
        INSERT just BEFORE that `if`:
            // Handshake lifecycle on a real device transition. Runs on THIS poll
            // thread — NEVER inside poll_handle.update, whose closure executes on
            // ksni's D-Bus thread (HID I/O there would wedge the tray icon).
            if last_device != Some(connected) {
                match crate::core::notifier::handshake_action(last_device, connected) {
                    crate::core::notifier::HandshakeAction::Gain => {
                        crate::core::notifier::perform_handshake(verbose);
                    }
                    crate::core::notifier::HandshakeAction::Loss => {
                        crate::core::notifier::reset_handshake_state();
                    }
                    crate::core::notifier::HandshakeAction::None => {}
                }
            }
        (Leave the existing combined guard + update closure UNCHANGED.)
  - GOTCHA G2: the match is in the poll-loop body, NOT inside poll_handle.update.
    CRITICAL — the update closure runs on ksni's D-Bus thread.
  - GOTCHA G8: do NOT move `last_device = Some(connected);` out of the combined
    guard — dark-only changes must still trigger the tray update.
  - GOTCHA G5: `verbose` is Copy (bool); used in the closure without move issues.
  - VERIFY: `grep -n 'pub fn spawn(verbose' src/linux_tray.rs` -> 1; `grep -rn 'linux_tray::spawn(' src/` -> 2 callers passing self.verbose; `grep -n 'handshake_action(last_device' src/linux_tray.rs` -> 1.

Task 7: VALIDATE (build + full suite + scope)
  - cargo build --bin qmkonnect                  # Linux default features ⇒ compiles linux_tray.rs
  - cargo test --bin qmkonnect -- --test-threads=1   # MANDATORY single-threaded (shared globals).
  - cargo test --bin qmkonnect handshake_action -- --test-threads=1   # the new test in isolation
  - git diff --stat                              # expect exactly the 6 files.
```

### Implementation Patterns & Key Details

```rust
// THE directional dispatch (identical shape in both poll threads):
match crate::core::notifier::handshake_action(last, connected) {
    crate::core::notifier::HandshakeAction::Gain => crate::core::notifier::perform_handshake(verbose),
    crate::core::notifier::HandshakeAction::Loss => crate::core::notifier::reset_handshake_state(),
    crate::core::notifier::HandshakeAction::None => {}
}
// `last` is the PREVIOUS is_device_connected() result (Option<bool>, init None).
// On the first tick (None), handshake_action(None, connected) ⇒ Gain if connected
// (no-op via HAS_HANDSHAKED if the runner already handshooked), None if not.

// THE startup block (identical at all 4 runner probe sites):
crate::core::notifier::startup_device_probe(self.verbose);
if crate::core::notifier::is_device_connected() {
    crate::core::notifier::perform_handshake(self.verbose);
}
// Direct `if` (not the helper): no Loss is possible at startup, and this matches
// the item wording verbatim.

// THE guarded Gain arm (the subtle correctness point):
(p, true) if p != Some(true) => HandshakeAction::Gain,
// Without the guard, `(_, true) => Gain` would mis-classify (Some(true), true)
// (no change) as Gain. The unit test's `handshake_action(Some(true), true) == None`
// row pins this.
```

### Integration Points

```yaml
MODULE REGISTRATION: NONE. All modules already declared. This task edits fn BODIES
  + 2 signatures + appends 1 helper + 1 test.

DEPENDENCIES (this task): NONE new. perform_handshake/reset_handshake_state/is_device_connected
  (S1 + notifier.rs:169/119), std Option/bool. No Cargo changes.

UPSTREAM (consumed unchanged):
  - S1 (P4.M2.T1.S1, COMPLETE): perform_handshake(verbose), reset_handshake_state(),
    host_capable(), callback_names(), HAS_HANDSHAKED static.
  - is_device_connected() (notifier.rs:169), startup_device_probe(verbose) (notifier.rs:119).

DOWNSTREAM CONSUMERS (later subtasks — do NOT implement them here):
  - P4.M3.T1.S1 (host-context send): reads host_capable()/callback_names() which THIS
    task's triggers populate at connect time. Gates APPLY_HOST_CONTEXT on host_capable().
  - P5.M2 (tray "Reload rules"): will call perform_handshake(verbose) again after re-reading
    rules.toml (re-arms via reset_handshake_state first).

CONFIG: none. ROUTES/CLI: none (P5). DATABASE: none. TRAY MENU: none (P5.M2 adds the item).
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# EXPECT: clean. Linux default features (Cargo.toml:100) ⇒ compiles linux_tray.rs
#   (so spawn(verbose) + the poll dispatch type-check). tray.rs's macos/windows poll
#   block is cfg'd OUT on Linux ⇒ NOT compiled here (validate by symmetry + AGENTS.md
#   platform loops). If a `setup_tray`/`spawn` call-site E0061 appears, you missed
#   passing self.verbose (G1).

cargo clippy --bin qmkonnect --no-deps 2>/dev/null || true   # optional; no NEW warnings.
```

### Level 2: Unit Tests (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# The new directional-logic test (single-threaded MANDATORY — shared globals):
cargo test --bin qmkonnect handshake_action -- --test-threads=1
# EXPECT: 1 passed. Spot-check the highest-risk row (the guarded Gain arm):
#   handshake_action(Some(true), true) == None  (NOT Gain).

# S1's handshake tests still green (proves the called fns behave; this task didn't touch them):
cargo test --bin qmkonnect notifier::tests::test_handshake_ -- --test-threads=1
# EXPECT: all 8 pass.
```

### Level 3: Full-crate regression (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# EXPECT: ALL bin tests green — the new handshake_action test + S1's 8 + the 5
#   test_send_command_* + debounce (P4.M1.T2.S2) + pattern (P2) + rules (P3) +
#   types + linux_tray::tests (status_text/parse_id). Proves the wiring compiles in
#   the full crate and didn't disturb shared globals or the trait seam.

git status --short && git diff --stat
# EXPECT: exactly src/core/notifier.rs, src/runners/{windows,macos,linux}.rs,
#   src/tray.rs, src/linux_tray.rs. NOTHING in Cargo.toml, main.rs, platforms/, etc.
```

### Level 4: Platform validation (macOS/Windows cfg blocks)

```bash
# The tray.rs poll-thread edit (Task 5) is #[cfg(any(macos,windows))] — NOT compiled
# on Linux. Validate it on its real platform via the AGENTS.md dev loop:

# macOS:
cargo test --bin qmkonnect -- --test-threads=1
cd packaging/macos && ./clean.sh && ./build.sh && ./install.sh && cd ../..
open /Applications/QMKonnect.app   # plug/unplug a QMK board; watch perform_handshake fire on reconnect

# Windows (PowerShell):
cargo test --bin qmkonnect -- --test-threads=1
cargo build --release
taskkill /IM qmkonnect.exe /F
.\target\release\qmkonnect.exe -v   # -v prints the perform_handshake progress lines on plug/unplug

# Linux (native on this box): the spawn(verbose) + poll dispatch (Task 6) ARE compiled;
# `cargo build` above already type-checked them. Runtime check:
cargo run -- -v   # then plug/unplug; expect "perform_handshake: ..." lines on each gain/loss
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean (Linux; no NEW warnings).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` green (new test + S1's 8 + all existing).
- [ ] `git diff --stat` shows exactly the 6 files (notifier.rs + 3 runners + tray.rs + linux_tray.rs).
- [ ] (optional) `cargo clippy --bin qmkonnect --no-deps` introduces no NEW warnings.

### Feature Validation (contract fidelity)
- [ ] `handshake_action` returns Gain for `(None,true)` + `(Some(false),true)`, Loss for
      `(Some(true),false)`, None for the other 3 rows (incl. `(Some(true),true)`).
- [ ] All 4 runner probe sites gate `perform_handshake(self.verbose)` on `is_device_connected()`.
- [ ] Both poll threads dispatch Gain⇒perform_handshake / Loss⇒reset_handshake_state / None⇒{}.
- [ ] `verbose` is plumbed through `setup_tray(verbose)` + `spawn(verbose)` and all 5 call sites pass it.
- [ ] linux_tray.rs handshake dispatch is OUTSIDE the `poll_handle.update(…)` closure (G2).
- [ ] Handshake runs at most once per board boot (HAS_HANDSHAKED) and re-runs after a real disconnect+reconnect.

### Code Quality Validation
- [ ] The Gain arm of `handshake_action` is guarded (`(p, true) if p != Some(true)`) — not bare `(_, true)`.
- [ ] cfg gates on tray.rs poll block and linux_tray.rs spawn preserved (G6/G7).
- [ ] No S1 code modified (G9); helper + test are additive appends.
- [ ] `last_device = Some(connected)` reassign stays inside the combined guard in linux_tray.rs (G8).
- [ ] Startup handshake inserted before `monitor.start()`/`setup_tray()`/`spawn()` (G10 start ordering).

### Documentation & Deployment
- [ ] `handshake_action` rustdoc includes the 6-row transition table.
- [ ] Inline comments at both poll threads note "runs on THIS poll thread, not the UI/D-Bus thread".
- [ ] Commit message notes: "wires perform_handshake into runner startup + device-status poll threads; adds handshake_action helper; populates host_capable() for P4.M3."

---

## Anti-Patterns to Avoid

- ❌ Don't write the Gain arm as bare `(_, true) => Gain` — it mis-classifies `(Some(true), true)` (no change) as Gain. Use the guarded `(p, true) if p != Some(true)`.
- ❌ Don't run `perform_handshake` inside `poll_handle.update(|t:&mut QmkTray|{…})` in linux_tray.rs — that closure runs on ksni's D-Bus thread; HID I/O there wedges the tray icon (G2).
- ❌ Don't forget a `setup_tray`/`spawn` call site when adding the `verbose` param — it fails loud (E0061), but check all 5 (G1).
- ❌ Don't move `last_device = Some(connected)` out of linux_tray.rs's combined guard — dark-only changes would stop updating the tray (G8).
- ❌ Don't modify S1's `perform_handshake`/`reset_handshake_state`/statics/mock — this task only CALLS them and APPENDS a helper (G9).
- ❌ Don't add the handshake to the UI event-loop arm (`DeviceStatus(connected)` in tray.rs) — run it on the poll thread before `send_event`, so the UI never blocks.
- ❌ Don't run tests multi-threaded — `--test-threads=1` is mandatory (shared globals, from S1).
- ❌ Don't touch the debounce worker / `notify_qmk` / `DebounceState` — that's P4.M1.T2.S2's region.
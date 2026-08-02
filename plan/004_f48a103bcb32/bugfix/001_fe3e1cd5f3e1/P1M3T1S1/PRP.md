# PRP — P1.M3.T1.S1: Add `startup_device_state` API and seed poll-thread initial state

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`) — Rust menu-bar/tray daemon.
> **Files edited (5):** `src/core/notifier.rs` (new `OnceLock` static + 2 `pub fn`s + 2 tests
> + 1 import word), `src/runners/linux.rs` (1 call), `src/runners/macos.rs` (1 call),
> `src/runners/windows.rs` (2 calls), `src/tray.rs` (1 seed line), `src/linux_tray.rs` (1 seed line).
> **Scope:** close bug-hunt Finding #5 ("Transient first-tick device-absent race after a startup
> handshake"). The poll threads start from `None`; a transient first-tick `false` never produces the
> `Loss` that resets `HAS_HANDSHAKED`, so a reconnect within one poll interval skips `SET_OS`.
> Fix: capture the startup `is_device_connected()` probe once into a process-global
> `OnceLock<bool>`, and seed each poll thread's `last` tracker from it — so a transient first-tick
> `false` after a connected startup correctly yields `Loss → reset → Gain → re-handshake`.
> **Dependency:** none hard. Soft-depends on the existing `is_device_connected()` (notifier.rs:216)
> and `handshake_action` (notifier.rs:689) — both already present & unchanged.

---

## Goal

**Feature Goal**: Eliminate the first-tick device-absent lifecycle desync (Finding #5). Today the
runner seeds `HAS_HANDSHAKED=true` at startup while the poll thread's `last` tracker starts at
`None` and can jump straight to `Some(false)` (via the `handshake_action(None, false) == None`
no-op arm) without ever recording a `Loss` — so `HAS_HANDSHAKED` stays `true` and a reconnect's
`Gain` short-circuits `perform_handshake`, skipping the `SET_OS` re-send on a freshly-rebooted
board. After this change the poll thread's initial `last` is derived from the **same** startup
device probe the runner used, reconciling the two trackers.

**Deliverable**:
1. `src/core/notifier.rs`: add `static STARTUP_DEVICE_CONNECTED: OnceLock<bool> = OnceLock::new();`
   (next to `HAS_HANDSHAKED`), plus two `pub fn`s — `record_startup_device_state()` and
   `startup_device_was_connected() -> bool` — and add `OnceLock` to the `std::sync` import.
2. `src/runners/{linux,macos,windows}.rs`: call `crate::core::notifier::record_startup_device_state();`
   immediately before each `if crate::core::notifier::is_device_connected() { … perform_handshake … }`
   block (4 sites: linux×1, macos×1, windows×2).
3. `src/tray.rs:385` and `src/linux_tray.rs:262`: change the poll-thread seed from `None` to
   `Some(crate::core::notifier::startup_device_was_connected())`.
4. 2 new unit tests in `notifier.rs`'s `#[cfg(test)] mod tests`.

**Success Definition**:
- `cargo test --bin qmkonnect -- --test-threads=1` is green and the count rises from **348 → 350**
  (the 2 new tests). On the Linux dev box this ALSO compile-validates the `notifier.rs` core
  (static + fns + tests), the `linux.rs` runner call, and the `linux_tray.rs` seed.
- `git diff --stat` shows ONLY the 6 files above.
- A code-reading check confirms: seeded `Some(true)` startup + transient first-tick `false`
  ⇒ `handshake_action(Some(true), false) == Loss` ⇒ `reset_handshake_state()` ⇒ next tick
  `true` ⇒ `Gain` ⇒ `perform_handshake` re-sends `SET_OS` (the documented fix trace).

## User Persona (if applicable)

**Target User**: the QMKonnect **poll threads** (macOS/Windows `tray.rs` device-status poller;
Linux `linux_tray.rs` poller) and the **device lifecycle** itself. Indirectly the end user whose
keyboard firmware was freshly rebooted and needs `SET_OS` re-applied so OS-conditional board-side
rules keep firing.

**Use Case**: App starts with a QMK board already plugged in → runner handshakes at startup.
Within ~3 s (one poll interval) the board is power-cycled (unplug/replug). Before this fix, a
transient first-tick `false` hid the `Loss`, so the reconnect skipped `SET_OS`. After this fix
the poll thread seeds `Some(true)` from the startup probe, the transient `false` is recorded as a
real `Loss`, and the reconnect correctly re-handshakes.

**User Journey**: (1) QMKonnect launches, board present → startup handshake runs, `SET_OS` sent.
(2) Poll thread spawns, seeds `last = Some(startup_device_was_connected()) == Some(true)`.
(3) Transient hidapi hiccup → first tick reads `false` → `handshake_action(Some(true), false) ==
Loss` → `reset_handshake_state()` (clears `HAS_HANDSHAKED`). (4) Next tick reads `true` →
`Gain` → `perform_handshake` re-runs (token now free) → `SET_OS` re-sent. ✓

**Pain Points Addressed**: closes the narrow but real window (Finding #5, severity Medium) where a
power-cycle inside one poll interval of a transient first-tick hiccup leaves a freshly-rebooted
board missing `current_os`, silently degrading OS-conditional host rules until the next genuine
`Some(true)→false` `Loss`.

## Why

- **Closes Finding #5 of the bug-hunt report (PRD h2.1 #5).** The race is fully analyzed in
  `architecture/handshake_race_research.md` §Finding #5 and confirmed step-by-step; the recommended
  fix is option (b): "seed the poll thread's `last` from the startup `is_device_connected()` result
  so it never starts `None`-but-already-handshooked." This task implements exactly that.
- **Least invasive reconciliation.** The alternative — changing `handshake_action(None, false)`
  semantics — risks spurious resets on legitimate cold starts (device genuinely absent at startup).
  Seeding `last` from the startup probe touches only initialization, leaves the transition table
  bit-for-bit unchanged, and reuses the already-correct `(Some(true), false) == Loss` arm.
- **No new dependency, no behavior change on the happy path.** The happy path (startup absent →
  seed `Some(false)` → first tick `true` → `Gain` → handshake) is unchanged in effect; only the
  transient-false-after-connected-startup path is corrected.

## What

### Code changes

**A. `src/core/notifier.rs`** — import, static, 2 fns, 2 tests.
1. Import (line 7): `use std::sync::{Arc, Condvar, Mutex};` → `use std::sync::{Arc, Condvar, Mutex, OnceLock};`
2. New static, immediately after `static HAS_HANDSHAKED` (currently notifier.rs:260-261):
   ```rust
   /// The device-presence snapshot captured ONCE at startup by
   /// [`record_startup_device_state`], read by the poll threads to seed their
   /// `last` tracker (P1.M3.T1.S1 / Finding #5). Without it the poll thread
   /// starts at `None` and a transient first-tick `false` after a connected
   /// startup never records a `Loss`, so [`HAS_HANDSHAKED`] stays `true` and a
   /// reconnect skips the `SET_OS` re-send. `OnceLock` is set-once: the first
   /// runner to call [`record_startup_device_state`] wins; subsequent calls are
   /// no-ops (the poll threads only ever read).
   static STARTUP_DEVICE_CONNECTED: OnceLock<bool> = OnceLock::new();
   ```
3. New `pub fn`s, immediately after `is_device_connected()` (currently ends at notifier.rs:226):
   ```rust
   /// Capture the device-presence snapshot ONCE for the poll threads (P1.M3.T1.S1).
   ///
   /// Called by each runner on the main thread, immediately before its
   /// `if is_device_connected() { perform_handshake() }` startup block. Stores
   /// the result in the set-once [`STARTUP_DEVICE_CONNECTED`]; the poll threads
   /// read it via [`startup_device_was_connected`] to seed their `last` tracker
   /// so a transient first-tick `false` (after a connected startup) is correctly
   /// classified as a `Loss` (resetting [`HAS_HANDSHAKED`]) instead of a no-op.
   /// A second call is a silent no-op (OnceLock::set returns Err, discarded).
   pub fn record_startup_device_state() {
       let _ = STARTUP_DEVICE_CONNECTED.set(is_device_connected());
   }

   /// The device-presence value captured at startup by [`record_startup_device_state`]
   /// (P1.M3.T1.S1). Poll threads seed their `last: Option<bool>` with
   /// `Some(startup_device_was_connected())`. Defaults to `false` if
   /// [`record_startup_device_state`] has not been called yet (e.g. in unit tests
   /// before any record) — harmless: the poll thread then behaves as "absent at
   /// startup", identical to the pre-fix cold-start path.
   pub fn startup_device_was_connected() -> bool {
       *STARTUP_DEVICE_CONNECTED.get().unwrap_or(&false)
   }
   ```
4. Two tests in the existing `#[cfg(test)] mod tests` block (which already does
   `use super::*;` at notifier.rs:1209, so call the fns bare). Place them near the other
   `handshake_action` test (notifier.rs:2036). See Implementation Tasks for exact bodies.

**B. `src/runners/{linux,macos,windows}.rs`** — 4 call-site insertions. Insert one line,
`crate::core::notifier::record_startup_device_state();`, immediately before each
`if crate::core::notifier::is_device_connected() {` block (after the
`startup_device_probe(self.verbose);` + the comment that precedes the `if`). Sites:
`linux.rs:31`, `macos.rs:31`, `windows.rs:52` (console), `windows.rs:105` (tray app).

**C. `src/tray.rs:385` + `src/linux_tray.rs:262`** — seed the poll threads:
```rust
// tray.rs:385 — BEFORE:
            let mut last: Option<bool> = None;
// AFTER:
            let mut last: Option<bool> =
                Some(crate::core::notifier::startup_device_was_connected());
```
```rust
// linux_tray.rs:262 — BEFORE:
        let mut last_device: Option<bool> = None;
// AFTER:
        let mut last_device: Option<bool> =
            Some(crate::core::notifier::startup_device_was_connected());
```
(Keep the surrounding `let mut` + `std::thread::spawn`/`loop` structure byte-identical; only the
initializer changes. Two-line initializer to stay within the file's existing column discipline.)

### Success Criteria
- [ ] `notifier.rs` import line reads `use std::sync::{Arc, Condvar, Mutex, OnceLock};`.
- [ ] `static STARTUP_DEVICE_CONNECTED: OnceLock<bool> = OnceLock::new();` present right after `HAS_HANDSHAKED`.
- [ ] `pub fn record_startup_device_state()` + `pub fn startup_device_was_connected() -> bool` present after `is_device_connected()`.
- [ ] All 4 runner sites call `crate::core::notifier::record_startup_device_state();` immediately before their `if is_device_connected() {` block.
- [ ] `tray.rs` poll seed = `Some(crate::core::notifier::startup_device_was_connected())`.
- [ ] `linux_tray.rs` poll seed = `Some(crate::core::notifier::startup_device_was_connected())`.
- [ ] 2 new tests pass; `cargo test --bin qmkonnect -- --test-threads=1` count = 348 → **350**, 0 failed.
- [ ] `git diff --stat` lists exactly the 6 files (`notifier.rs`, `runners/{linux,macos,windows}.rs`, `tray.rs`, `linux_tray.rs`).

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge of this codebase can implement every edit from the exact
anchors below (grep-confirmed, indentation-unique except the 2 identical `windows.rs` blocks which
are disambiguated by trailing context), the verified `OnceLock` availability (Rust 1.70+, edition
2021), the file's `use super::*;` test convention, and the AGENTS.md single-threaded `cargo test`
gate.

### Documentation & References

```yaml
# MUST READ — the bug-hunt analysis that defines this task (the "why" + the recommended fix)
- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/architecture/handshake_race_research.md
  why: "Finding #5 traces the exact race: startup handshake sets HAS_HANDSHAKED=true; poll thread
        seeds last=None; transient first-tick false → handshake_action(None,false)==None → no Loss
        → HAS_HANDSHAKED stays true → reconnect Gain short-circuits perform_handshake → SET_OS
        not re-sent. Recommends fix option (b): seed poll thread last from startup probe."
  critical: "the ONLY reset trigger is handshake_action(Some(true), false)==Loss (notifier.rs:691).
        handshake_action(None, false)==None is the bug path. The fix makes the poll thread START at
        Some(startup_probe) instead of None, so a connected startup seeds Some(true) and the
        transient false becomes a real Loss. Do NOT change handshake_action's table — seed instead."
  section: "Finding #5 — First-tick device-absent race skips the Loss that resets HAS_HANDSHAKED"

# MUST READ — PRD context (the defect this fixes)
- url: spec/PRD.md (heading h2.1, finding #5 "Transient first-tick device-absent race after a startup handshake")
  why: "the PRD-recorded defect; severity is Medium. Benign for the SAME board (firmware remembers
        state), so severity is low-to-medium — but a genuine power-cycle within one poll interval
        of a transient first-tick hiccup skips SET_OS on the freshly-rebooted board."

# MUST READ — the file owning the new API + tests (exact current state, verified this session)
- file: src/core/notifier.rs
  why: "adds the OnceLock static + 2 pub fns + 2 tests + the import word. is_device_connected()
        (:216-226) is the probe both record_startup_device_state() and the runners call.
        HAS_HANDSHAKED (:260) is where the new static sits. handshake_action (:689-693) is the
        transition table the fix relies on (unchanged). reset_handshake_state (:649-654) is the
        Loss action. The #[cfg(test)] mod tests (:1208) does `use super::*;` (:1209)."
  pattern: "statics are declared as `static NAME: Type = …;` with a doc comment; pub fns are
        `pub fn name(...) { … }` with `///` doc. Existing handshake lifecycle fns (is_device_connected,
        perform_handshake, reset_handshake_state, handshake_action) are the style template."
  gotcha: "OnceLock is NOT currently imported (line 7 is `use std::sync::{Arc, Condvar, Mutex};`).
        Add it to that SAME use list → `use std::sync::{Arc, Condvar, Mutex, OnceLock};` (matches
        core/mod.rs:9 + linux_tray.rs:500). Do NOT add a bare `use std::sync::OnceLock;` line."

# MUST READ — the runners (4 call sites for record_startup_device_state)
- file: src/runners/linux.rs
  why: "1 call site. The `if crate::core::notifier::is_device_connected() {` block is at :31.
        Insert record_startup_device_state() right after startup_device_probe(self.verbose); and
        the comment, immediately before the `if`. cfg(target_os=\"linux\") — compiles on the Linux box."
- file: src/runners/macos.rs
  why: "1 call site, identical pattern, `if` at :31. cfg(target_os=\"macos\") — NOT compiled on Linux;
        validates on macOS builds (AGENTS.md macOS loop)."
- file: src/runners/windows.rs
  why: "2 call sites (run_console_mode `if` at :52; run_tray_app `if` at :105), identical pattern.
        cfg(target_os=\"windows\") — NOT compiled on Linux; validates on Windows builds. The two
        blocks are byte-identical → disambiguate edits by TRAILING context (see Implementation Tasks)."
  gotcha: "both windows.rs blocks have the SAME preceding context (startup_device_probe + comment +
        if). edit's oldText must be unique → include the distinct trailing line: console arm is
        followed by a blank line + `ctrlc::set_handler(move || {`; tray arm by a blank line +
        `// Start the monitor before setting up the tray (matches the working order).`"

# MUST READ — the poll-thread seed sites
- file: src/tray.rs
  why: "the macOS/Windows device-status poll thread. Seed at :385 `let mut last: Option<bool> = None;`
        inside `#[cfg(any(target_os=\"macos\", target_os=\"windows\"))]` (block starts :378). Change
        None → Some(startup_device_was_connected()). NOT compiled on Linux → validates on macOS/Windows."
- file: src/linux_tray.rs
  why: "the Linux SNI poll thread. Seed at :262 `let mut last_device: Option<bool> = None;`. Change
        None → Some(startup_device_was_connected()). Compiles on Linux (linux-tray default feature)
        → validated on the Linux dev box."

# REFERENCE — the sibling task that will also edit notifier.rs (no overlap, different region)
- file: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/  (P1.M3.T2.S1 — not yet started)
  why: "P1.M3.T2.S1 will restructure the NOTIFIER lock inside perform_handshake_with (notifier.rs:388+).
        THIS task adds a static (~:264) + 2 fns (~:228) + tests; it does NOT touch perform_handshake_with
        internals. Different regions of the same file → low merge conflict."
  critical: "do NOT pull perform_handshake_with apart or restructure its locking here — that is
        P1.M3.T2.S1's scope. Keep this task purely additive (new static + 2 fns + 2 seeds + 4 calls)."
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
src/core/notifier.rs      # ADD: OnceLock import word (:7); static STARTUP_DEVICE_CONNECTED (~:264);
                          #      pub fn record_startup_device_state + startup_device_was_connected (~:228);
                          #      2 tests in #[cfg(test)] mod tests (~:1208+)
  - :7   use std::sync::{Arc, Condvar, Mutex};   (← add OnceLock)
  - :216 pub fn is_device_connected() -> bool    (the probe; ends :226)  ← new fns go right after
  - :260 static HAS_HANDSHAKED: AtomicBool       ← new OnceLock static goes right after (:263)
  - :649 pub fn reset_handshake_state()          (the Loss action)
  - :689 pub fn handshake_action(prev, now)      (the table — UNCHANGED; the fix relies on it)
  - :1208 #[cfg(test)] mod tests { use super::*; … }   ← 2 new tests here
src/runners/linux.rs      # EDIT: insert record_startup_device_state() call before `if` at :31   (Linux: compiles)
src/runners/macos.rs      # EDIT: insert record_startup_device_state() call before `if` at :31   (macOS build)
src/runners/windows.rs    # EDIT: 2 insertions before `if` at :52 (console) and :105 (tray)       (Windows build)
src/tray.rs               # EDIT: :385 seed None → Some(startup_device_was_connected())           (macOS/Windows build)
  - :378 #[cfg(any(target_os="macos", target_os="windows"))] { … poll thread … }
src/linux_tray.rs         # EDIT: :262 seed None → Some(startup_device_was_connected())           (Linux: compiles)
Cargo.toml                # DO NOT TOUCH (edition 2021 already supports OnceLock; no new deps)
```

### Desired Codebase tree
No new files, no new modules. The 6 edits above are the entirety of the change. `Cargo.toml`
unchanged (OnceLock is in `std`, stable since 1.70, edition 2021).

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (IMPORT): add OnceLock to the EXISTING `use std::sync::{Arc, Condvar, Mutex};` line
//   (notifier.rs:7) → `use std::sync::{Arc, Condvar, Mutex, OnceLock};`. Do NOT add a separate
//   `use std::sync::OnceLock;` line — it would duplicate the path and break the file's grouping.
//   core/mod.rs:9 and linux_tray.rs:500 both use the `use std::sync::OnceLock;` form, but those
//   files have no other std::sync item on the same line; notifier.rs ALREADY imports 3 items from
//   std::sync, so appending OnceLock to that group is the local idiom.

// CRITICAL (OnceLock is set-once): record_startup_device_state() does `let _ = …set(…)`. The FIRST
//   call wins; later calls are silent no-ops. This is INTENTIONAL and correct: exactly one runner
//   path runs per process, and it calls record() once on the main thread before any poll thread
//   exists. The poll threads only ever READ via startup_device_was_connected(). Do NOT change set()
//   to expect success or to overwrite — there is no OnceLock::replace in std.

// CRITICAL (default before record): startup_device_was_connected() uses `.get().unwrap_or(&false)`.
//   Before ANY record call (e.g. a unit test that reads before recording, or a hypothetical code
//   path that spawns a poll thread before the runner) it returns false. That is the SAFE default:
//   the poll thread then seeds Some(false), identical to the pre-fix cold-start behavior (first real
//   tick true → Gain → handshake). Do NOT panic if unset.

// CRITICAL (PLATFORM VALIDATION): runners/mod.rs cfg-gates the modules —
//   `#[cfg(target_os="linux")] mod linux`, `#[cfg windows] mod windows`, `#[cfg macos] mod macos`.
//   On the Linux dev box ONLY linux.rs + linux_tray.rs + notifier.rs (the core: static, 2 fns,
//   2 tests) compile & test. The macos.rs + windows.rs runner calls and the tray.rs poll-seed are
//   #[cfg] away on Linux → they compile-check ONLY on macOS / Windows builds (AGENTS.md loops).
//   They are one-line additions calling an ALREADY-COMPILED pub fn (record_startup_device_state /
//   startup_device_was_connected both exist in notifier.rs and are compiled on every OS), so the
//   compile risk is near-zero, but do NOT claim they are validated on a Linux box. Note them as
//   deferred-to-target-OS in the report (like P1.M2.T2.S2's tray.rs edits).

// CRITICAL (windows.rs has TWO identical blocks): run_console_mode (:52) and run_tray_app (:105)
//   contain byte-identical `startup_device_probe / comment / if is_device_connected` text. The
//   `edit` tool requires a UNIQUE oldText per call. Disambiguate by including the distinct TRAILING
//   line in each oldText (console → `ctrlc::set_handler`; tray → `// Start the monitor`). See
//   Implementation Tasks Task 3 for exact disambiguated anchors.

// GOTCHA: do NOT change handshake_action's transition table. The fix RELIES on the existing
//   (Some(true), false) == Loss arm. Changing (None, false) to Loss would cause spurious resets on
//   legitimate cold starts (device genuinely absent at startup). Seeding last is the surgical fix.

// GOTCHA: keep the poll-thread edits to the INITIALIZER ONLY. tray.rs:385 and linux_tray.rs:262 are
//   `let mut last[: Option<bool>] = None;`. Change `= None` to
//   `= Some(crate::core::notifier::startup_device_was_connected())`. Leave the surrounding
//   std::thread::spawn / loop / match / last = Some(connected) / sleep structure untouched. The
//   two-line initializer (wrap the Some(...) onto its own line) respects the file's column budget.

// GOTCHA: tests MUST run single-threaded: `cargo test --bin qmkonnect -- --test-threads=1`
//   (AGENTS.md — shared global debouncer/mock state). Parallel runs flap.

// GOTCHA: OnceLock is process-global & set-once, and tests run in one process. A test that calls
//   record_startup_device_state() freezes the value for the rest of the process. On a CI box with
//   no QMK device, is_device_connected()==false, so the frozen value is false. The deterministic
//   test asserts the accessor equals the LIVE probe (both false on CI), not a fixed environment-
//   specific value. See Implementation Tasks Task 6.

// GOTCHA: do NOT fold record_startup_device_state() into startup_device_probe(). They have
//   different responsibilities: startup_device_probe is a read-only VID/PID-typo hint (#16);
//   record_startup_device_state records lifecycle state for the poll threads. The contract keeps
//   them separate. (See DRY note in research/findings.md.)
```

## Implementation Blueprint

### Data models and structure
None. No new types. `STARTUP_DEVICE_CONNECTED: OnceLock<bool>` is a process-global primitive;
`record_startup_device_state()` / `startup_device_was_connected()` are thin accessors over it.
`handshake_action` / `HandshakeAction` / `HAS_HANDSHAKED` / `reset_handshake_state` are unchanged.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: EDIT src/core/notifier.rs — add OnceLock to the import (line 7)
  - OLD (exact): "use std::sync::{Arc, Condvar, Mutex};"
  - NEW: "use std::sync::{Arc, Condvar, Mutex, OnceLock};"
  - NOTE: the only std::sync::OnceLock-related change. Matches core/mod.rs:9 + linux_tray.rs:500
    availability; edition 2021 supports it (Cargo.toml:4).
  - PRESERVE: the other 8 `use` lines (:1-6, :8-10) unchanged.

Task 2: EDIT src/core/notifier.rs — add the static + 2 pub fns
  Sub-task 2a (static): insert immediately AFTER the HAS_HANDSHAKED block. Anchor — the two-line
    static + its 4-line doc comment currently read:
      "        (\"is_device_connected()\" false→true) to re-trigger.\nstatic HAS_HANDSHAKED: AtomicBool = AtomicBool::new(false);\n"
    Append (after that line, before the blank line + `/// Dedup guard for the malformed-`rules.toml`…`):
      "/// The device-presence snapshot captured ONCE at startup by\n/// [`record_startup_device_state`], read by the poll threads to seed their\n/// `last` tracker (P1.M3.T1.S1 / Finding #5). Without it the poll thread\n/// starts at `None` and a transient first-tick `false` after a connected\n/// startup never records a `Loss`, so [`HAS_HANDSHAKED`] stays `true` and a\n/// reconnect skips the `SET_OS` re-send. `OnceLock` is set-once: the first\n/// runner to call [`record_startup_device_state`] wins; subsequent calls are\n/// no-ops (the poll threads only ever read).\nstatic STARTUP_DEVICE_CONNECTED: OnceLock<bool> = OnceLock::new();\n"
  Sub-task 2b (fns): insert immediately AFTER is_device_connected()'s closing brace. Anchor — the
    function ends with:
      "        // If HID can't be enumerated at all, treat the device as absent.\n        Err(_) => false,\n    }\n}\n"
    Append (after that `}`, before the `\n// ===` separator comment + the handshake section):
      "\n/// Capture the device-presence snapshot ONCE for the poll threads (P1.M3.T1.S1).\n///\n/// Called by each runner on the main thread, immediately before its\n/// `if is_device_connected() { perform_handshake() }` startup block. Stores\n/// the result in the set-once [`STARTUP_DEVICE_CONNECTED`]; the poll threads\n/// read it via [`startup_device_was_connected`] to seed their `last` tracker\n/// so a transient first-tick `false` (after a connected startup) is correctly\n/// classified as a `Loss` (resetting [`HAS_HANDSHAKED`]) instead of a no-op.\n/// A second call is a silent no-op (OnceLock::set returns Err, discarded).\npub fn record_startup_device_state() {\n    let _ = STARTUP_DEVICE_CONNECTED.set(is_device_connected());\n}\n\n/// The device-presence value captured at startup by [`record_startup_device_state`]\n/// (P1.M3.T1.S1). Poll threads seed their `last: Option<bool>` with\n/// `Some(startup_device_was_connected())`. Defaults to `false` if\n/// [`record_startup_device_state`] has not been called yet (e.g. in unit tests\n/// before any record) — harmless: the poll thread then behaves as \"absent at\n/// startup\", identical to the pre-fix cold-start path.\npub fn startup_device_was_connected() -> bool {\n    *STARTUP_DEVICE_CONNECTED.get().unwrap_or(&false)\n}\n"
  - FOLLOW pattern: is_device_connected / perform_handshake / reset_handshake_state (doc-comment +
    `pub fn name() { … }`).
  - PRESERVE: is_device_connected()'s body and the handshake-section separator comment below.

Task 3: EDIT src/runners/{linux,macos,windows}.rs — insert the record call (4 sites)
  The line to insert is IDENTICAL at all 4 sites:
      "        crate::core::notifier::record_startup_device_state();\n"
  (8-space indent — matches the runner body indent.) It goes immediately AFTER the comment block
  that precedes the `if`, i.e. on its own line right before `        if crate::core::notifier::is_device_connected() {`.

  Sub-task 3a (linux.rs:31 + macos.rs:31 — each file has ONE site, unique within its file):
    - IMPORTANT: the comment line ABOVE the `if` wraps DIFFERENTLY in the two files:
        linux.rs :50-51 → "// now (poll-thread reconnects are handled in linux_tray.rs / tray.rs)." then
                    "// Completes before the poll thread exists; idempotent via HAS_HANDSHAKED."
        macos.rs :50-51 → "// now (poll-thread reconnects are handled in tray.rs). Completes before the" then
                    "// poll thread exists; idempotent via HAS_HANDSHAKED."
      So do NOT anchor on the comment (it differs). Anchor on the 3-line `if`-BLOCK, which is
      byte-identical across all 4 sites AND unique within each of linux.rs / macos.rs (grep confirms
      `if crate::core::notifier::is_device_connected() {` appears exactly once in each file):
        OLD (3-line block, 8-space / 12-space indent — matches the runner body):
            "        if crate::core::notifier::is_device_connected() {\n            crate::core::notifier::perform_handshake(self.verbose);\n        }\n"
        NEW (prepend the record line before the `if`, keep the block byte-identical):
            "        crate::core::notifier::record_startup_device_state();\n        if crate::core::notifier::is_device_connected() {\n            crate::core::notifier::perform_handshake(self.verbose);\n        }\n"
      Edit linux.rs and macos.rs SEPARATELY (each is a single-file edit; the 3-line block is unique
      within each). PRESERVE the comment block above and everything below the `}`.

  Sub-task 3b (windows.rs:52 console — disambiguate by TRAILING ctrlc context):
    OLD (unique via trailing ctrlc line):
        "        if crate::core::notifier::is_device_connected() {\n            crate::core::notifier::perform_handshake(self.verbose);\n        }\n\n        ctrlc::set_handler(move || {\n            println!(\"\\nReceived Ctrl+C, shutting down...\");\n            process::exit(0);\n        })?;\n\n        println!(\"Starting Windows monitor...\");"
    NEW: insert the record line before the `if` (keep everything else byte-identical):
        "        crate::core::notifier::record_startup_device_state();\n        if crate::core::notifier::is_device_connected() {\n            crate::core::notifier::perform_handshake(self.verbose);\n        }\n\n        ctrlc::set_handler(move || {\n            println!(\"\\nReceived Ctrl+C, shutting down...\");\n            process::exit(0);\n        })?;\n\n        println!(\"Starting Windows monitor...\");"

  Sub-task 3c (windows.rs:105 tray app — disambiguate by TRAILING monitor comment):
    OLD (unique via the "Start the monitor" comment):
        "        if crate::core::notifier::is_device_connected() {\n            crate::core::notifier::perform_handshake(self.verbose);\n        }\n\n        // Start the monitor before setting up the tray (matches the working order).\n        let mut monitor = monitor;"
    NEW: insert the record line before the `if`:
        "        crate::core::notifier::record_startup_device_state();\n        if crate::core::notifier::is_device_connected() {\n            crate::core::notifier::perform_handshake(self.verbose);\n        }\n\n        // Start the monitor before setting up the tray (matches the working order).\n        let mut monitor = monitor;"

  - NAMING: the call is fully-qualified `crate::core::notifier::record_startup_device_state()` —
    matches the file's universal fully-qualified notifier-call convention (every notifier call in
    these runners is `crate::core::notifier::…`). Do NOT add a `use` line.
  - PRESERVE: startup_device_probe(self.verbose); above and perform_handshake(self.verbose); below.

Task 4: EDIT src/tray.rs:385 — seed the macOS/Windows poll thread
  - OLD (exact, 12-space indent): "            let mut last: Option<bool> = None;"
  - NEW (two-line initializer to respect column budget):
        "            let mut last: Option<bool> =\n                Some(crate::core::notifier::startup_device_was_connected());"
  - NOTE: this line is inside #[cfg(any(target_os="macos", target_os="windows"))] → NOT compiled on
    Linux. Compiles on macOS/Windows builds. The call is fully-qualified (tray.rs has ZERO
    `use crate::core::` imports — matches P1.M2.T2.S2's convention note).
  - PRESERVE: the surrounding `std::thread::spawn(move || { … loop { … } })` structure.

Task 5: EDIT src/linux_tray.rs:262 — seed the Linux poll thread
  - OLD (exact, 8-space indent): "        let mut last_device: Option<bool> = None;"
  - NEW (two-line initializer):
        "        let mut last_device: Option<bool> =\n            Some(crate::core::notifier::startup_device_was_connected());"
  - NOTE: compiles on Linux (linux-tray default feature) → validated on the dev box. Fully-qualified
    call (linux_tray.rs has NO `use crate::core::` imports — see P1.M2.T2.S2).
  - PRESERVE: the `let mut last_dark`, `let mut tick`, and the loop body.

Task 6: ADD 2 tests to src/core/notifier.rs #[cfg(test)] mod tests (place near the existing
        handshake_action test at :2036; `use super::*;` is already in scope at :1209)
  - TEST A — test_poll_thread_seeded_from_startup:
        #[test]
        fn test_poll_thread_seeded_from_startup() {
            // record_startup_device_state() probes is_device_connected() once and freezes the
            // result in the set-once STARTUP_DEVICE_CONNECTED. startup_device_was_connected()
            // then returns that bool. OnceLock is process-global & set-once, and tests run in one
            // process — so the FIRST record call wins; later calls are no-ops. We assert the
            // round-trip contract: after recording, the accessor matches the live probe (on a CI
            // box with no QMK device both are false; on a box with a device both are true).
            record_startup_device_state();
            assert_eq!(
                startup_device_was_connected(),
                is_device_connected(),
                "after record_startup_device_state, startup_device_was_connected must match the \
                 live is_device_connected probe (OnceLock freezes the first record's value)"
            );
        }
  - TEST B — test_handshake_action_loss_on_seeded_true_to_false (documents the fix's reliance):
        #[test]
        fn test_handshake_action_loss_on_seeded_true_to_false() {
            // The P1.M3.T1.S1 seed fix relies on this exact transition being `Loss`: when the poll
            // thread is seeded from startup_device_was_connected() == Some(true) and the first tick
            // transiently reads false, handshake_action(Some(true), false) MUST be Loss so
            // reset_handshake_state() fires and the subsequent reconnect re-sends SET_OS. (Also
            // asserted in test_handshake_action_transitions; restated here to pin the invariant
            // the seed fix depends on.)
            assert_eq!(handshake_action(Some(true), false), HandshakeAction::Loss);
        }
  - FOLLOW pattern: test_handshake_action_transitions (:2036) — bare `handshake_action(...)` /
    `HandshakeAction::…` assertions (super::* is imported). NAMING: test_{subject}_{scenario}.
  - COVERAGE: TEST A exercises record/read round-trip + the default-false path (when run before any
    prior record) or the frozen-value path (OnceLock already set). TEST B pins the Loss arm.

Task 7: VALIDATE (no edits)
  - cargo build --bin qmkonnect
      # Linux: compiles. Validates notifier.rs (Tasks 1-2,6), linux.rs (3a), linux_tray.rs (5).
      # macos.rs/windows.rs/tray.rs are #[cfg] away on Linux (see Platform-Validation Reality).
  - cargo test --bin qmkonnect -- --test-threads=1
      # Full suite green; count 348 → 350. --test-threads=1 is REQUIRED (AGENTS.md).
  - git diff --stat     # exactly 6 files: notifier.rs, runners/{linux,macos,windows}.rs, tray.rs, linux_tray.rs
  - grep -n 'STARTUP_DEVICE_CONNECTED\|record_startup_device_state\|startup_device_was_connected' src/core/notifier.rs
      # 1 static + 2 fns + 2 references inside them + (in tests) 2 references = sane.

Task 8: NEVER do these (out of scope / forbidden)
  - DO NOT change handshake_action's transition table (the fix RELIES on the existing Loss arm).
  - DO NOT touch perform_handshake / perform_handshake_with internals (that is P1.M3.T2.S1).
  - DO NOT add a `use crate::core::notifier::…` import in tray.rs / linux_tray.rs / the runners —
    call fully-qualified (crate::core::notifier::…), matching the universal convention in those files.
  - DO NOT fold record_startup_device_state() into startup_device_probe() (different responsibilities).
  - DO NOT add a separate `use std::sync::OnceLock;` line in notifier.rs — append OnceLock to the
    existing line-7 `use std::sync::{…}` group.
  - DO NOT edit Cargo.toml, PRD.md, tasks.json, prd_snapshot.md, or any other source file.
  - DO NOT claim the macos.rs/windows.rs runner calls or the tray.rs poll-seed are validated on a
    Linux box — they are #[cfg] away on Linux. Note them as deferred-to-target-OS.
```

### Implementation Patterns & Key Details
```rust
// PATTERN: the new accessor pair is a thin set-once / read-over-OnceLock, matching the file's
// existing lifecycle-state style (HAS_HANDSHAKED: AtomicBool + perform_handshake/reset_handshake_state).
//   pub fn record_startup_device_state() {
//       let _ = STARTUP_DEVICE_CONNECTED.set(is_device_connected());  // first call wins
//   }
//   pub fn startup_device_was_connected() -> bool {
//       *STARTUP_DEVICE_CONNECTED.get().unwrap_or(&false)             // false until recorded
//   }
//
// WHY OnceLock (not AtomicBool): the startup snapshot is write-EXACTLY-ONCE then read by N poll
//   threads forever. OnceLock expresses that contract (and gives a safe "default before set" via
//   unwrap_or(&false)) without needing a separate "have we initialized?" flag. AtomicBool could
//   race on concurrent first-record calls; OnceLock cannot (it's the std idiom for one-shot init).
//
// WHY the seed is Some(...) not a bool field: the poll thread's `last: Option<bool>` distinguishes
//   "no observation yet" (None) from "observed absent" (Some(false)). Seeding with
//   Some(startup_probe) removes the None-only-on-first-tick window that caused Finding #5:
//   handshake_action(Some(true), false) == Loss now fires on a transient first-tick false.
//
// WHY fully-qualified calls in the runners/tray/linux_tray: those files have ZERO `use crate::core::
//   notifier::` imports — every notifier symbol is `crate::core::notifier::…`. Adding a lone import
//   for the new fn would be the file's only such import and break the convention. (Same idiom as
//   P1.M2.T2.S2's atomic_write note.)
//
// ANTI-PATTERN: do NOT change `handshake_action(None, false)` to `Loss`. That would spuriously
//   reset HAS_HANDSHAKED on a legitimate cold start (device genuinely absent at startup, no prior
//   handshake). The seed is the surgical fix; the table stays bit-for-bit.
//
// ANTI-PATTERN: do NOT make record_startup_device_state() panic or error if already set. The
//   `let _ = …set(…)` deliberately discards the Err — a second call (e.g. a future code path, or
//   a test after the first record) MUST be a silent no-op, not a panic.
```

### Integration Points
```yaml
IMPORTS:
  - notifier.rs:7 add `OnceLock` to the existing `use std::sync::{…}` group (ONE word appended).
  - runners/{linux,macos,windows}.rs: NO new imports (fully-qualified crate::core::notifier:: call).
  - tray.rs / linux_tray.rs: NO new imports (fully-qualified crate::core::notifier:: call).
DEPENDENCIES:
  - is_device_connected() (notifier.rs:216) — already present, unchanged; record_startup_device_state
    delegates to it.
  - handshake_action (notifier.rs:689) — already present, UNCHANGED; the fix relies on its existing
    (Some(true), false) == Loss arm.
CARGO: none. No Cargo.toml change. OnceLock is in std (stable 1.70; edition 2021).
PARALLEL / SIBLING (no overlap, clean merges):
  - P1.M2.T2.S2 (parallel, being implemented now): edits tray.rs:878/:1276 + linux_tray.rs:822
    (settings-dialog writes). THIS task edits tray.rs:385 + linux_tray.rs:262 (poll-thread seeds).
    Same files, lines far apart → clean merge.
  - P1.M3.T2.S1 (sibling, not started): will restructure the NOTIFIER lock inside
    perform_handshake_with (notifier.rs:388+). THIS task adds a static (~:264) + 2 fns (~:228) +
    tests; it does NOT touch perform_handshake_with. Different regions of notifier.rs.
PLATFORM VALIDATION (CRITICAL):
  - Linux dev box: compiles + tests notifier.rs (Tasks 1-2, 6 — the CORE of the fix: static, 2 fns,
    2 tests), linux.rs (3a), linux_tray.rs (5). These are the substantive, logic-bearing changes.
  - macOS/Windows builds (AGENTS.md loops): compile-check macos.rs (3a), windows.rs (3b,3c),
    tray.rs (4). These are one-line additions of a call to an already-compiled pub fn — near-zero
    compile risk. Note them as deferred-to-target-OS in the report.
```

## Validation Loop

> Toolchain: Rust (`cargo`). No ruff/mypy. `cargo build` + `cargo test` are the gates.
> Tests MUST run single-threaded (AGENTS.md — shared global debouncer/mock state).

### Level 1: Syntax & Style (Immediate Feedback)
```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles with zero new warnings. On Linux this validates notifier.rs (Tasks 1-2,6),
#   linux.rs (3a), linux_tray.rs (5). macos.rs/windows.rs/tray.rs are #[cfg] away on Linux.
# If "cannot find type `OnceLock`" → Task 1 import not applied (OnceLock must be in the line-7 group).
# If "cannot find function `record_startup_device_state`" → Task 2 fns not added before the call sites.
```

### Level 2: Unit Tests (Component Validation)
```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect tests::test_poll_thread_seeded_from_startup -- --test-threads=1
cargo test --bin qmkonnect tests::test_handshake_action_loss_on_seeded_true_to_false -- --test-threads=1
# Expected: both pass. (tests:: filter targets the #[cfg(test)] mod tests members.)
```

### Level 3: Full Suite (Regression — AGENTS.md mandates single-threaded)
```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL tests pass. Count rises 348 → 350 (the 2 new tests). 0 failed, 0 ignored.
# --test-threads=1 is REQUIRED (shared global debouncer/mock state; parallel runs flap).
```

### Level 4: Manual device-lifecycle exercise (per AGENTS.md dev loops)
```bash
# The poll-thread fix is exercised via the platform dev loops (no HID unit harness). On each OS:
#   Linux:   cargo build --bin qmkonnect; run; plug a QMK board; watch the startup handshake fire
#            (verbose); unplug/replug within ~3 s; confirm a 2nd handshake + SET_OS fires on reconnect
#            (was skipped before the fix on a transient first-tick false).
#   macOS:   AGENTS.md macOS loop — packaging/macos clean+build+install; open QMKonnect.app; same
#            plug/unplug/replug sequence; confirm SET_OS re-sent on reconnect.
#   Windows: AGENTS.md Windows loop — cargo build --release; taskkill; .\target\release\qmkonnect.exe;
#            same sequence; confirm SET_OS re-sent.
# Expected (all OSes): the menu/tray status flips connected→disconnected→connected and the verbose
#   log shows perform_handshake (with SET_OS) firing on the reconnect after a transient first-tick
#   absence. (Hard to reproduce deterministically — the unit tests pin the logic; this is a smoke
#   check that the seeded poll thread still drives the tray status correctly.)
```

### Level 5: Scope/Build Hygiene
```bash
cd /home/dustin/projects/qmkonnect
git diff --stat                 # Expected: exactly 6 files (notifier.rs, runners/{linux,macos,windows}.rs, tray.rs, linux_tray.rs).
git diff Cargo.toml             # Expected: empty.
grep -n 'STARTUP_DEVICE_CONNECTED' src/core/notifier.rs
                                # Expected: 1 static decl + 2 references (record sets, startup_device_was_connected reads).
grep -n 'record_startup_device_state' src/runners/*.rs
                                # Expected: 4 matches — linux.rs, macos.rs, windows.rs×2.
grep -n 'startup_device_was_connected' src/tray.rs src/linux_tray.rs
                                # Expected: 1 match each (the poll-thread seeds).
grep -n 'let mut last: Option<bool> = None;\|let mut last_device: Option<bool> = None;' src/tray.rs src/linux_tray.rs
                                # Expected: ZERO matches (both seeds now start with Some(...)).
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` succeeds on Linux with no new warnings (validates notifier.rs core + linux.rs + linux_tray.rs).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` → full suite green, count 348 → **350**.
- [ ] `git diff --stat` shows exactly the 6 files; `git diff Cargo.toml` is empty.

### Feature Validation
- [ ] `notifier.rs:7` imports `OnceLock` (appended to the `std::sync::{…}` group).
- [ ] `static STARTUP_DEVICE_CONNECTED: OnceLock<bool>` present right after `HAS_HANDSHAKED`.
- [ ] `pub fn record_startup_device_state()` + `pub fn startup_device_was_connected() -> bool` present after `is_device_connected()`.
- [ ] 4 runner sites call `crate::core::notifier::record_startup_device_state();` before their `if is_device_connected()` block (linux×1, macos×1, windows×2).
- [ ] `tray.rs` + `linux_tray.rs` poll seeds = `Some(crate::core::notifier::startup_device_was_connected())`; no `= None` seeds remain.
- [ ] Both new tests pass; the seeded-`Some(true)` → transient-`false` → `Loss` → reset → reconnect `Gain` → re-handshake trace is documented in code comments.
- [ ] `handshake_action` transition table UNCHANGED (the fix relies on the existing Loss arm — verify `git diff` shows no edit near notifier.rs:689-693).

### Code Quality Validation
- [ ] `OnceLock` appended to the existing line-7 import group (no separate `use std::sync::OnceLock;`).
- [ ] All runner/tray/linux_tray notifier calls fully-qualified (`crate::core::notifier::…`); no new `use` lines added.
- [ ] New fns carry `///` doc comments explaining the Finding #5 fix (matches the file's doc style).
- [ ] `record_startup_device_state()` uses `let _ = …set(…)` (silent no-op on second call) — no panic/unwrap on the set.
- [ ] `startup_device_was_connected()` uses `.unwrap_or(&false)` (safe default before any record).
- [ ] No new dependencies; Cargo.toml untouched.

### Documentation & Deployment
- [ ] No user-facing / config / API surface change (internal lifecycle fix — DOCS: none per contract).
- [ ] No new env vars / config keys / CLI flags.
- [ ] Report notes that macos.rs/windows.rs runner calls + tray.rs poll-seed are compile-validated only on their target OS (macOS/Windows builds per AGENTS.md); the notifier.rs core + linux.rs + linux_tray.rs are validated on the Linux box.

---

## Anti-Patterns to Avoid
- ❌ Don't change `handshake_action`'s transition table (e.g. mapping `(None, false)` to `Loss`) — that would spuriously reset `HAS_HANDSHAKED` on legitimate cold starts. Seed the poll thread instead; rely on the existing `(Some(true), false) == Loss` arm.
- ❌ Don't add a separate `use std::sync::OnceLock;` line in notifier.rs — append `OnceLock` to the existing line-7 `use std::sync::{Arc, Condvar, Mutex};` group.
- ❌ Don't make `record_startup_device_state()` panic or `.expect()` on a second call — `let _ = …set(…)` makes a repeat call a silent no-op (OnceLock is set-once by design).
- ❌ Don't add `use crate::core::notifier::record_startup_device_state;` / `…startup_device_was_connected;` in the runners, tray.rs, or linux_tray.rs — call them fully-qualified (`crate::core::notifier::…`), matching those files' universal convention (zero such imports today).
- ❌ Don't fold `record_startup_device_state()` into `startup_device_probe()` — they have different responsibilities (typo-hint vs lifecycle-state snapshot).
- ❌ Don't touch `perform_handshake` / `perform_handshake_with` internals — that is P1.M3.T2.S1's scope (NOTIFIER lock restructure). This task is purely additive (new static + 2 fns + seeds + calls).
- ❌ Don't run tests without `--test-threads=1` (AGENTS.md — shared global debouncer/mock state; parallel runs flap).
- ❌ Don't claim the macos.rs/windows.rs runner calls or the tray.rs poll-seed are validated on a Linux box — they're `#[cfg]`-gated; defer their compile-check to macOS/Windows builds per AGENTS.md.
- ❌ Don't edit Cargo.toml, PRD.md, tasks.json, prd_snapshot.md, or any file beyond the 6 listed.
- ❌ Don't change anything beyond the initializer on the two poll-thread seeds — leave `loop` / `match` / `last = Some(connected)` / `sleep` untouched.

---

## Confidence Score: 9/10

The change is small, purely additive, and every anchor is verified current this session (import line
7, `is_device_connected` end at :226, `HAS_HANDSHAKED` at :260, `handshake_action` at :689-693, the
`#[cfg(test)] mod tests` at :1208 with `use super::*`, runner `if`-blocks at linux.rs:31 / macos.rs:31
/ windows.rs:52 & :105, poll-thread seeds at tray.rs:385 & linux_tray.rs:262). `OnceLock` is std
(stable 1.70, edition 2021), already used in core/mod.rs:9 + linux_tray.rs:500. Baseline
`cargo test --bin qmkonnect -- --test-threads=1` is **348 passed** this session → expect **350**.
The Linux dev box compile-validates the substantive core (notifier.rs static + fns + tests, linux.rs
call, linux_tray.rs seed); the macos.rs/windows.rs runner calls and the tray.rs seed are
deferred-to-target-OS but are one-line additions of a call to an already-compiled pub fn. The 1-point
reservation: the two new unit tests are necessarily lightweight (OnceLock is set-once / process-global
and `is_device_connected()` does real HID enumeration not mockable here), so they pin the round-trip
contract + the Loss-arm invariant rather than the full transient-first-tick race end-to-end — that
race is exercised only via the manual dev-loop (Level 4), which is hard to reproduce deterministically.
The logic correctness rests on the seed value + the already-tested `handshake_action` table, both
now pinned by tests.
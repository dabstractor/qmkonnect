# PRP — P1.M1.T1.S1: Add DeviceStatus (three-state) to src/core/notifier.rs and a device_status() resolver

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). ALL edits in ONE file:
> `src/core/notifier.rs`.
> **Scope:** The status-line **resolver only**. Add a `DeviceStatus` enum + a
> `device_status()` function that derives the three-state value from the TWO
> already-maintained booleans (`is_device_connected()` + `host_capable()`). No UI
> change — the trays consume this in S2/S3. **No per-path pinging.**
> `is_device_connected()` and `host_capable()` are **unchanged**.

---

## Goal

**Feature Goal**: Add a public three-state device-status type and resolver to
`src/core/notifier.rs` so the tray/menu-bar status line (S2/S3) can render a
**truthful** value instead of a boolean:

| State | Derivation (from existing state) |
|---|---|
| `Disconnected` | `!is_device_connected()` — 0 Tier-1 boards |
| `NoModule` | `is_device_connected() && !host_capable()` — ≥1 Tier-1 board, 0 capable |
| `Connected` | `is_device_connected() && host_capable()` — ≥1 capable board |

The "No module" state is the headline value of F13: it tells the user "you have a
QMK board, but it isn't running the qmk_notifier firmware" instead of a false-green
"Connected" that silently does nothing (`spec/DEVICE_DISCOVERY.md` §3).

**Deliverable**: `src/core/notifier.rs` containing (1) `pub enum DeviceStatus {
Connected, NoModule, Disconnected }` with derives, (2) `pub fn device_status() ->
DeviceStatus` reading the two existing predicates, (3) a private pure helper
`fn classify_device_status(present: bool, capable: bool) -> DeviceStatus` (the
testable truth table), and (4) unit tests covering all three derivations. A
doc-comment on `device_status()`/`DeviceStatus` citing `DEVICE_DISCOVERY.md` §3 and
explaining the derivation.

**Success Definition**: `cargo build` compiles with zero warnings; `cargo test --bin
qmkonnect -- --test-threads=1` passes with the new tests; `is_device_connected()`
and `host_capable()` are byte-for-byte unchanged; no UI file is touched; no new HID
I/O or pinging is added.

## User Persona (if applicable)

**Target User**: The S2/S3 tray implementers (who render the three states) and,
ultimately, the end user who currently sees a misleading "Connected" when their
board lacks the qmk_notifier module.

**Use Case**: The status-poll thread (already running) calls `device_status()` on
each poll and refreshes the tray label/icon only on a transition — exactly as it
does today with the boolean, just three-valued now.

**User Journey**: User plugs in a vanilla QMK board (no qmk_notifier) → tray shows
`⚠ QMK board found — no qmk_notifier module (flash it)` instead of a false green.
They flash the module → next handshake sets `HOST_CAPABLE` → tray flips to
`● Device Connected`.

**Pain Points Addressed**: Eliminates the false-green "Connected" for VIA-only /
un-flashed boards. The `NoModule` state is actionable ("flash it") instead of
silent.

## Why

- **It is the resolver half of F13 (the headline).** The spec (`DEVICE_DISCOVERY.md`
  §3, `ARCHITECTURE.md` §5.6) mandates a three-state status line. The two booleans
  it derives from are **already maintained** by the existing poll-thread lifecycle
  (`is_device_connected()` enumerates; `host_capable()` is set/reset by the
  handshake Gain/Loss the poll threads already drive). So this subtask needs **no
  new probing** — it is a pure read of existing state.
- **It unblocks S2/S3 with a stable contract.** The trays need a single, typed
  value to render (text + icon + the Linux one-shot notify). Defining
  `DeviceStatus` + `device_status()` first lets S2/S3 be implemented and tested
  against a fixed surface.
- **It is additive and low-risk.** One new enum, one new function, one private
  helper, a few tests. Nothing existing changes semantically.

## What

### (a) The enum

```rust
/// Three-state device status for the tray/menu-bar status line
/// (`spec/DEVICE_DISCOVERY.md` §3). Derived from the two booleans the existing
/// poll-thread lifecycle already maintains — see [`device_status`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus {
    /// ≥1 **capable** board present (`is_device_connected() && host_capable()`).
    /// Tray: `● Device Connected`. Icon: solid `U+25CF`, full alpha.
    Connected,
    /// ≥1 Tier-1 board present, **0 capable** (`is_device_connected() &&
    /// !host_capable()`). The truthful "flash qmk_notifier" state. Tray:
    /// `⚠ QMK board found — no qmk_notifier module (flash it)`.
    NoModule,
    /// 0 Tier-1 boards present (`!is_device_connected()`). Tray:
    /// `○ No Device Connected`. Icon: hollow `U+25CB`, dimmed.
    Disconnected,
}
```

> `PartialEq`/`Eq` so S2/S3 (and the tests) can compare `prev == new` to detect
> transitions; `Copy` because it is a 3-variant fieldless enum (free, idiomatic).

### (b) The resolver + private testable helper

```rust
/// The device-status for the tray/menu-bar status line right now
/// (`spec/DEVICE_DISCOVERY.md` §3, `ARCHITECTURE.md` §5.6).
///
/// Derives the three-state value from the two booleans the existing poll-thread
/// lifecycle already maintains — it does **not** send any HID command or open any
/// device:
/// - [`is_device_connected()`] — pure Tier-1 enumeration (any `0xFF60`/`0x61`
///   interface matching the configured filter; never opens/sends).
/// - [`host_capable()`] — reads the [`HOST_CAPABLE`] `AtomicBool`, set `true` by
///   the handshake on a capable `QUERY_INFO` reply and reset `false` on a device
///   Loss / failure.
///
/// | Status        | Condition                              |
/// |---------------|----------------------------------------|
/// | `Disconnected`| `!is_device_connected()`               |
/// | `NoModule`    | `is_device_connected() && !host_capable()` |
/// | `Connected`   | `is_device_connected() && host_capable()`  |
///
/// **Transient caveat:** right after a device Gain, `host_capable()` is `false`
/// until `perform_handshake` completes (sub-second); the line may briefly read
/// `NoModule` before flipping to `Connected`. Acceptable per spec.
///
/// The pure truth table lives in [`classify_device_status`] so it is unit-testable
/// without a real device (Tier-1 enumeration reflects actual hardware, which is
/// absent in CI).
pub fn device_status() -> DeviceStatus {
    classify_device_status(is_device_connected(), host_capable())
}

/// Pure three-state classifier — the testable truth table for [`device_status`].
///
/// Split out so the three derivations can be unit-tested deterministically:
/// [`is_device_connected`] enumerates real HID hardware (always `false` in CI),
/// so [`device_status`] itself can only naturally produce [`DeviceStatus::Disconnected`]
/// in the test environment. This helper takes the two booleans directly.
fn classify_device_status(present: bool, capable: bool) -> DeviceStatus {
    if !present {
        DeviceStatus::Disconnected
    } else if capable {
        DeviceStatus::Connected
    } else {
        DeviceStatus::NoModule
    }
}
```

> The helper is a **private implementation detail** — the public API is the no-arg
> `device_status()` exactly as the contract specifies. It exists solely to make
> the three-row truth table deterministically testable (see Gotchas).

### (c) Placement

Place `DeviceStatus`, `device_status()`, and `classify_device_status()` together in
the **status cluster**, immediately after `reset_handshake_state()` (line ~710) —
that is where the capability/handshake state lives, and `device_status()` reads
`host_capable()` which is right there (line 689). Leave one blank line of separation.

### (d) The unit tests (inside the existing `#[cfg(test)] mod tests` block)

```rust
// ---- DeviceStatus three-state derivation (P1.M1.T1.S1) ----

#[test]
fn test_classify_device_status_truth_table() {
    // All three rows of the §3 table, deterministically (no hardware needed).
    use DeviceStatus::*;
    // present=false dominates regardless of `capable`:
    assert_eq!(classify_device_status(false, false), Disconnected);
    assert_eq!(classify_device_status(false, true), Disconnected);
    // present=true, not capable -> the headline NoModule state:
    assert_eq!(classify_device_status(true, false), NoModule);
    // present=true, capable -> Connected:
    assert_eq!(classify_device_status(true, true), Connected);
}

#[test]
fn test_device_status_is_disconnected_in_ci_without_hardware() {
    // device_status() wires is_device_connected() (Tier-1 enumerate) + host_capable().
    // In CI there is no 0xFF60/0x61 board, so is_device_connected() == false and the
    // result MUST be Disconnected — even if a stale HOST_CAPABLE=true lingered
    // (present=false dominates; a stale capability flag can never fabricate a
    // false "NoModule"/"Connected"). Drive both HOST_CAPABLE values to prove it.
    reset_handshake_state(); // HOST_CAPABLE = false
    assert_eq!(device_status(), DeviceStatus::Disconnected);

    HOST_CAPABLE.store(true, Ordering::SeqCst); // simulate a stale capable flag
    assert_eq!(
        device_status(),
        DeviceStatus::Disconnected,
        "no Tier-1 board present must dominate a stale HOST_CAPABLE"
    );
    reset_handshake_state(); // restore HOST_CAPABLE = false (isolation)
}
```

> `HOST_CAPABLE` is a module-level `static AtomicBool` (line 270), so the test
> module (which does `use super::*;`) can set it directly. `reset_handshake_state()`
> restores it to `false` for test isolation (tests are single-threaded per AGENTS.md).

### Success Criteria

- [ ] `pub enum DeviceStatus { Connected, NoModule, Disconnected }` exists with
      `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`.
- [ ] `pub fn device_status() -> DeviceStatus` derives the §3 table from
      `is_device_connected()` + `host_capable()`; sends no HID command.
- [ ] Private `fn classify_device_status(present, capable) -> DeviceStatus` holds
      the truth table.
- [ ] `device_status()`/`DeviceStatus` doc-comment cites `spec/DEVICE_DISCOVERY.md`
      §3 and explains the derivation (Mode A — no per-path ping assumption).
- [ ] 2 new tests pass (`test_classify_device_status_truth_table`,
      `test_device_status_is_disconnected_in_ci_without_hardware`).
- [ ] `is_device_connected()` and `host_capable()` are unchanged.
- [ ] No file other than `src/core/notifier.rs` is modified; no UI touched.

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed
> to implement this successfully?"_ — **Yes.** The verbatim enum + resolver + helper
> bodies, the §3 derivation table, the exact placement anchor, the ready-to-paste
> tests, the testability rationale (why a private helper is needed), and the
> verified validation commands are all below.

### Documentation & References

```yaml
# MUST READ — the architecture research (validated existing-state reuse + derivation table)
- file: /home/dustin/projects/qmkonnect/plan/005_8b95ea464bd9/architecture/notifier_mechanisms.md
  why: "Confirms is_device_connected() (216) is pure Tier-1 enumerate (never opens); host_capable()
        (689) reads HOST_CAPABLE (270); HOST_CAPABLE is set by perform_handshake_with on Gain and
        reset by reset_handshake_state() on Loss; the poll threads already keep host_capable() correct.
        Gives the exact three-state derivation table and the 'No new pinging required' conclusion."
  section: "Tier-1 Presence", "Tier-2 Capability", "Three-State Derivation (§2.1)"
  critical: "device_status() must NOT add any HID I/O / pinging — it only reads the two existing
             booleans. is_device_connected() and host_capable() stay UNCHANGED."

# MUST READ — the spec the doc-comment cites (the authoritative three-state table + tray text/icons)
- file: /home/dustin/projects/qmkonnect/spec/DEVICE_DISCOVERY.md
  why: "§3 is the source-of-truth three-state semantics (Connected/NoModule/Disconnected conditions,
        tray text, icons, the Linux one-shot notify-send). The DeviceStatus doc-comment cites it."
  section: "3. Device-Status Semantics (three states)"
  critical: "The 'NoModule' state is the headline value of F13 — truthful 'flash qmk_notifier'
             feedback instead of false-green. Variant names: Connected / NoModule / Disconnected."

# MUST READ — the file being edited (confirm exact current code before editing)
- file: /home/dustin/projects/qmkonnect/src/core/notifier.rs
  why: "Contains is_device_connected() (216), HOST_CAPABLE (270), host_capable() (689),
        reset_handshake_state() (705), and the #[cfg(test)] mod tests block. device_status() goes
        after reset_handshake_state() (~710); the tests go inside the existing mod tests."
  pattern: "Public-fn doc style: `///` + the function's contract. Statics are module-level `static X:
            AtomicBool`. The test module does `use super::*;` so HOST_CAPABLE is directly settable."
  gotcha: "is_device_connected() enumerates REAL hardware (false in CI) — so device_status() can only
           naturally return Disconnected under `cargo test`. The private classify_device_status()
           helper is the seam that makes all three rows deterministically testable. Do NOT try to mock
           hidapi (no trait seam exists; out of scope)."

# REFERENCE — the status-probe thread that will call device_status() (S2/S3, not this subtask)
- file: /home/dustin/projects/qmkonnect/spec/ARCHITECTURE.md
  why: "§5.6 documents the poll thread (3s macOS/Windows, 1s Linux) that refreshes the status line on
        a transition. device_status() is its new read; cadence/transition logic is unchanged (S2/S3)."
  section: "5.6 Status probe"

# REFERENCE — research notes for this subtask (testability analysis)
- docfile: plan/005_8b95ea464bd9/P1M1T1S1/research/notes.md
  why: "Documents the testability constraint (is_device_connected == real hardware), why the private
        classify_device_status helper is the clean seam, HOST_CAPABLE direct-set access from tests,
        and the per-board-count deferral to P3's classify_devices."
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/                       # THIS repo
├── spec/
│   ├── DEVICE_DISCOVERY.md      # §3 = the authoritative three-state table (doc-comment cites it)
│   └── ARCHITECTURE.md          # §5.6 = the status-poll thread (S2/S3 wiring)
└── src/core/
    └── notifier.rs              # <-- FILE TO EDIT. is_device_connected@216, HOST_CAPABLE@270,
                                 #     host_capable@689, reset_handshake_state@705, mod tests.
```

### Desired Codebase tree with files to be modified

```bash
src/core/
└── notifier.rs   # MODIFIED ONLY — add DeviceStatus enum + device_status() + classify_device_status()
                  #                      + 2 unit tests. is_device_connected/host_capable unchanged.
```

> No new files. No UI file (`tray.rs`/`linux_tray.rs`) is touched — that is S2/S3.

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL: device_status() must NOT ping or open any device.
//   It is a pure read of is_device_connected() (Tier-1 enumerate) + host_capable() (AtomicBool).
//   Do NOT add a per-candidate QUERY_INFO here — that is P3's classify_devices() (a different,
//   cache-backed Tier-2 mechanism). This subtask reuses the handshake's already-maintained
//   HOST_CAPABLE flag. Adding I/O here would (a) violate the contract, (b) break the "cheap status
//   poll" NFR, and (c) collide with P3's design.

// CRITICAL: is_device_connected() and host_capable() are UNCHANGED.
//   They are still used by the write-path/broadcast decision, the device-presence snapshot, and the
//   picker Tier-1 pass (per the architecture doc). device_status() only READS them.

// CRITICAL: is_device_connected() enumerates real hardware — false in CI.
//   So device_status() can only naturally return Disconnected under `cargo test`. To unit-test all
//   three derivations deterministically, the pure truth table lives in the PRIVATE
//   classify_device_status(present, capable) helper, which device_status() delegates to. Do NOT try
//   to inject/mock hidapi (no trait seam; out of scope). Do NOT make device_status() take args
//   (the contract fixes its signature as no-arg).

// NOTE: HOST_CAPABLE is a module-level `static AtomicBool` (line 270).
//   The test module does `use super::*;` so it can set it directly:
//   `HOST_CAPABLE.store(true, Ordering::SeqCst);`. Always restore via reset_handshake_state()
//   (or store(false)) at the end of a test — tests are single-threaded (shared global state).

// NOTE: the simpler 3-variant enum (no per-board counts) is intentional for P1.
//   The spec table's "N Devices Connected" pluralization and per-board classification are P3's job
//   (classify_devices + ClassifiedDevice). P1's DeviceStatus is the flat three-state value the status
//   line renders today; per-board richness layers on later without changing this enum's variants.

// NOTE: place the new items in the STATUS cluster (after reset_handshake_state @710), not next to
//   is_device_connected (@216). device_status() reads host_capable() (@689); grouping it with the
//   capability/handshake state is the discoverable placement.

// NOTE: tests run `cargo test --bin qmkonnect -- --test-threads=1` (shared mock globals + DebounceState,
//   per AGENTS.md). The new tests are pure (no DebounceState), but follow the same protocol.
```

## Implementation Blueprint

### Data models and structure

The only data model is the 3-variant fieldless `DeviceStatus` enum (What (a)). No
structs, no constructors, no trait impls (the derives cover it). The "logic" is the
3-row truth table in `classify_device_status`.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CONFIRM the exact anchors
  - READ: src/core/notifier.rs — is_device_connected() (216), HOST_CAPABLE (270), host_capable() (689),
          reset_handshake_state() (705). Confirm the status cluster ends ~710.
  - READ: spec/DEVICE_DISCOVERY.md §3 (the table the doc-comment cites) and the architecture doc's
          "Three-State Derivation" section (the exact derivation).
  - CONFIRM: the test module uses `use super::*;` (so HOST_CAPABLE + classify_device_status are in scope).
  - GOAL: anchor placement + copy the derivation verbatim.

Task 2: ADD DeviceStatus + device_status() + classify_device_status()
  - INSERT (after reset_handshake_state @~710): the `pub enum DeviceStatus` (What a), then
          `pub fn device_status()` (What b), then `fn classify_device_status(present, capable)` (What b).
  - DERIVES: exactly #[derive(Debug, Clone, Copy, PartialEq, Eq)].
  - DOC: device_status()/DeviceStatus doc-comment cites `spec/DEVICE_DISCOVERY.md` §3 + the derivation
          table + the "no per-path ping" note (Mode A).
  - DO NOT: modify is_device_connected(), host_capable(), reset_handshake_state(), HOST_CAPABLE, or any
          handshake/poll code. DO NOT add HID I/O.

Task 3: ADD the 2 unit tests
  - INSERT (inside #[cfg(test)] mod tests, near the handshake tests): the two tests in What (d).
  - test_classify_device_status_truth_table: all 3 rows of the helper (Disconnected×2, NoModule, Connected).
  - test_device_status_is_disconnected_in_ci_without_hardware: set HOST_CAPABLE both ways, assert
          device_status() == Disconnected (present dominates), restore via reset_handshake_state().
  - DO NOT: add a test that assumes a real device is present (CI has none). DO NOT mock hidapi.

Task 4: VALIDATE
  - RUN: cargo build  (zero warnings)
  - RUN: cargo clippy --bin qmkonnect  (no new warnings)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1  (existing + 2 new pass)
  - RUN: grep confirming is_device_connected/host_capable unchanged (see Validation Loop).
```

### Implementation Patterns & Key Details

```rust
// === THE DERIVATION (verbatim from spec §3 / architecture doc) ===
// Disconnected = !present
// NoModule     =  present && !capable   // the headline F13 value
// Connected    =  present &&  capable
// `present` dominates: a stale HOST_CAPABLE=true can never fabricate NoModule/Connected when no
// board is enumerated. (This is the property the CI integration test asserts.)

// === WHY A PRIVATE HELPER (the testability seam) ===
// is_device_connected() calls hidapi::HidApi::new() and enumerates real interfaces — always false
// in CI. device_status() (no-arg, per contract) can only return Disconnected there. The pure
// classify_device_status(present, capable) takes the two booleans directly, so all three rows are
// unit-testable without hardware. device_status() is just:
//   classify_device_status(is_device_connected(), host_capable())

// === DOC-COMMENT MUST CITE §3 + EXPLAIN DERIVATION ===
// So a future reader does NOT assume device_status() opens devices / sends QUERY_INFO. State
// explicitly: "derives from is_device_connected() + host_capable(); sends no HID command."
```

### Integration Points

```yaml
SOURCE FILES:
  - modify: "src/core/notifier.rs ONLY"

PUBLIC API SURFACE:
  - adds: "DeviceStatus (pub enum), device_status() -> DeviceStatus (pub fn)"
  - unchanged: "is_device_connected(), host_capable(), reset_handshake_state(), HOST_CAPABLE,
                configured_filter(), all handshake/poll/write code"

DEPENDENCIES / Cargo.toml:
  - none. No new deps. No HID I/O.

DOWNSTREAM CONSUMERS (do NOT implement now — S2/S3):
  - P1.M1.T1.S2: "src/tray.rs (macOS/Windows) poll thread calls device_status(); renders the three
                  labels/icons (UI.md §1.1, DEVICE_DISCOVERY.md §3)."
  - P1.M1.T1.S3: "src/linux_tray.rs renders the three states + the Disconnected→NoModule one-shot
                  notify-send (uses the once-guard pattern at notifier.rs:299)."

DEFERRED (NOT this subtask):
  - P3.M1.T1: "classify_devices() + ClassifiedDevice — the per-board Tier-2 mechanism and the
               'N Devices Connected' pluralization. P1's DeviceStatus is the flat three-state value;
               P3 layers per-board richness on top WITHOUT changing these variants."
```

## Validation Loop

> All commands run from the repo root: `/home/dustin/projects/qmkonnect`.

### Level 1: Syntax & Style

```bash
cd /home/dustin/projects/qmkonnect

cargo build 2>&1 | tee /tmp/build.log
# Expected: "Finished" — zero warnings. (DeviceStatus is a fieldless enum; device_status() is a pure
# read of two existing fns — nothing can go wrong type-wise. If it fails, check the helper's bool args.)

cargo clippy --bin qmkonnect 2>&1 | tee /tmp/clippy.log | grep -iE 'warning|error' || echo "clippy clean"
# Expected: no new warnings. (An unused-for-now pub fn/enum does NOT trip dead_code — pub items are
# public API. Do NOT add #[allow(dead_code)]; S2/S3 will use it.)

cargo fmt --check
# Expected: exit 0. If non-zero, run `cargo fmt`.
```

### Level 2: Unit Tests (the real gate)

```bash
cd /home/dustin/projects/qmkonnect

# The 2 new tests in isolation.
cargo test --bin qmkonnect test_classify_device_status_truth_table -- --test-threads=1 --nocapture
cargo test --bin qmkonnect test_device_status_is_disconnected_in_ci_without_hardware -- --test-threads=1 --nocapture
# Expected: 1 passed each. (Truth table: Disconnected/NoModule/Connected all covered; CI integration:
# device_status() == Disconnected even with a stale HOST_CAPABLE=true.)

# Full suite — single-threaded (shared global state, per AGENTS.md).
cargo test --bin qmkonnect -- --test-threads=1 2>&1 | tail -3
# Expected: "test result: ok. <N+2> passed; 0 failed; ...". (N = pre-existing count; +2 new.)
```

### Level 3: Integration (no UI yet — sanity only)

```text
NOT REQUIRED for this subtask. device_status() is a pure read; no UI consumes it until S2/S3. The
Level-2 tests ARE the proof the derivation is correct. Live-device validation (seeing NoModule flip to
Connected after a handshake) happens in S2/S3 against real hardware via the AGENTS.md dev loop.
```

### Level 4: Scope-preservation grep (prove nothing existing changed)

```bash
cd /home/dustin/projects/qmkonnect

# (a) is_device_connected() and host_capable() bodies are untouched.
diff <(git show HEAD:src/core/notifier.rs | sed -n '/pub fn is_device_connected/,/^}/p') \
     <(sed -n '/pub fn is_device_connected/,/^}/p' src/core/notifier.rs) && echo "is_device_connected: unchanged" \
  || echo "is_device_connected CHANGED — revert (out of scope)"

diff <(git show HEAD:src/core/notifier.rs | sed -n '/pub fn host_capable/,/^}/p') \
     <(sed -n '/pub fn host_capable/,/^}/p' src/core/notifier.rs) && echo "host_capable: unchanged" \
  || echo "host_capable CHANGED — revert (out of scope)"

# (b) No HID I/O sneaked into device_status() (it must call only is_device_connected + host_capable).
sed -n '/pub fn device_status/,/^}/p' src/core/notifier.rs | grep -iE 'HidApi|send_command|write|open' \
  && echo "VIOLATION: device_status() does HID I/O" || echo "device_status(): no HID I/O (good)"

# (c) No UI file touched.
git status --short src/tray.rs src/linux_tray.rs
# Expected: empty (no modifications).
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1: `cargo build` zero warnings; `cargo clippy --bin qmkonnect` no new warnings; `cargo fmt --check` exit 0.
- [ ] Level 2: both new tests pass; full suite `<N+2> passed; 0 failed` (`--test-threads=1`).
- [ ] Level 4 (a): `is_device_connected()` and `host_capable()` byte-for-byte unchanged.
- [ ] Level 4 (b): `device_status()` body contains no HID I/O.
- [ ] Level 4 (c): `src/tray.rs` / `src/linux_tray.rs` unmodified.

### Feature Validation

- [ ] `DeviceStatus` has exactly `Connected`, `NoModule`, `Disconnected` with the §3 derives.
- [ ] `device_status()` derives `Disconnected = !present`, `NoModule = present && !capable`, `Connected = present && capable`.
- [ ] `classify_device_status` private helper holds the truth table (all 3 rows tested).
- [ ] doc-comment cites `spec/DEVICE_DISCOVERY.md` §3 and states "no HID command / per-path ping".
- [ ] The NoModule case (`present && !capable`) is explicitly covered by a test assertion.

### Code Quality Validation

- [ ] Enum derives match codebase convention (`Debug, Clone, Copy, PartialEq, Eq`).
- [ ] Public API is the no-arg `device_status()`; helper is private.
- [ ] Tests follow the file's `test_<thing>_<scenario>` naming + `use super::*;`.
- [ ] HOST_CAPABLE is restored via `reset_handshake_state()` in the CI test (isolation).
- [ ] No `#[allow(dead_code)]` (unnecessary for `pub` items).

### Documentation & Deployment

- [ ] Mode A: the `device_status()`/`DeviceStatus` doc-comment is the documentation (cites §3).
- [ ] No user-facing doc file changed here (S2/S3 + P4 handle `UI.md`/`docs/*.md`).
- [ ] No environment variables, config, or Cargo.toml changes.

---

## Anti-Patterns to Avoid

- ❌ Don't add per-path pinging / `QUERY_INFO` to `device_status()` — it is a pure read of the two
  existing booleans. Per-candidate classification is P3's `classify_devices()` (cache-backed Tier-2),
  a different mechanism. Adding I/O here violates the contract, breaks the cheap-poll NFR, and
  collides with P3.
- ❌ Don't change `is_device_connected()` or `host_capable()` — they're retained for the write-path /
  broadcast decision, the device-presence snapshot, and the picker Tier-1 pass. `device_status()` only
  reads them.
- ❌ Don't make `device_status()` take arguments — the contract fixes its signature as no-arg (it reads
  global state). The private `classify_device_status(present, capable)` is the testable seam, not the
  public API.
- ❌ Don't try to mock `hidapi` / `is_device_connected()` to force `present=true` in tests — there is no
  trait seam and it's out of scope. Use the private `classify_device_status` helper for the truth table;
  use the CI integration test (no device ⇒ `Disconnected`) for `device_status()` itself.
- ❌ Don't add a 4th variant or per-board count fields — P1 uses the flat three-state enum. The
  "N Devices Connected" pluralization and `ClassifiedDevice` richness are P3; layering them later must
  not require changing these variants.
- ❌ Don't place the new items next to `is_device_connected()` (line 216) — group them in the status
  cluster after `reset_handshake_state()` (~710), where `host_capable()` lives.
- ❌ Don't add `#[allow(dead_code)]` — `pub` enum/fn are public API; they don't trigger it (and S2/S3
  will consume them immediately after).
- ❌ Don't forget to restore `HOST_CAPABLE` (via `reset_handshake_state()`) in the CI test — tests are
  single-threaded and share global state.
- ❌ Don't run tests without `--test-threads=1` — shared mock globals + `DebounceState` (AGENTS.md).

---

**Confidence Score: 9/10** for one-pass implementation success. The deliverable is
a 3-variant enum + a 3-line pure resolver + a private truth-table helper, all
quoted verbatim from the architecture doc's validated derivation table, with
ready-to-paste tests and a precise placement anchor. The one design decision worth
flagging — the private `classify_device_status` seam — is forced by a real
constraint (`is_device_connected()` enumerates hardware ⇒ always `false` in CI) and
is the only way to satisfy the contract's "unit tests for the three derivations"
deterministically; it keeps the public `device_status()` signature exactly as
specified. The scope-preservation greps (Level 4) make verification deterministic.
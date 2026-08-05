# PRP — P1.M1.T1.S2: Render three-state status in `src/tray.rs` (macOS/Windows)

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). ALL edits in ONE file:
> `src/tray.rs`.
> **Scope:** The macOS/Windows tray **status-line rendering** (line 2 of the menu).
> Change the status from a boolean (`is_device_connected()`) to S1's three-state
> `DeviceStatus` (`device_status()`), so the tray shows a truthful `⚠ No module`
> state instead of a false-green "Connected" for VIA-only / un-flashed boards.
> **The handshake lifecycle stays keyed on the Tier-1 presence bool** — do NOT gate
> it on the three-state value. `src/core/notifier.rs` (S1) and `src/linux_tray.rs`
> (S3) are NOT touched.

---

## Goal

**Feature Goal**: Render the three-state device status in the macOS/Windows tray
(`src/tray.rs`) by switching the status payload from `bool` to S1's `DeviceStatus`:
`● Device Connected` / `⚠ QMK board found — no qmk_notifier module (flash it)` /
`○ No Device Connected`. The status-poll thread emits the three-state value on a
transition (its own tracker), while the handshake Gain/Loss lifecycle remains
keyed on the `is_device_connected()` bool exactly as today. First-paint reads
`device_status()` so the menu is correct before the first poll.

**Deliverable**: `src/tray.rs` (only) with five coordinated edits: (A) the
`UserEvent::DeviceStatus` variant carries `DeviceStatus`; (B) `device_status_text`
is a three-branch `match` over `DeviceStatus`; (C) first-paint calls
`device_status()`; (D) the poll thread tracks `last_status: Option<DeviceStatus>`
separately and sends `UserEvent::DeviceStatus(status)` on its transition, while
the handshake block is byte-unchanged on the bool; (E) the event-loop arm binds
`status: DeviceStatus`. Plus one small cfg-gated unit test asserting the three
text/glyph mappings.

**Success Definition**: `cargo build` (macOS/Windows, or non-Hyprland Linux)
compiles with zero warnings; `cargo clippy --bin qmkonnect` adds none; the new
tray.rs test passes (macOS/Windows) and the full single-threaded suite stays green;
`is_device_connected()` / `handshake_action` / `perform_handshake` /
`reset_handshake_state` / `startup_device_was_connected` are read-only (unchanged);
no file other than `src/tray.rs` is modified; the `device_status_text` doc-comment
cites `spec/UI.md` §4 (Mode A).

## User Persona (if applicable)

**Target User**: A user who plugs in a vanilla QMK board (no qmk_notifier module)
and currently sees a misleading `● Device Connected` that silently does nothing.

**Use Case**: User connects a VIA-only board → tray shows `⚠ QMK board found — no
qmk_notifier module (flash it)` (truthful, actionable) instead of a false green.
They flash qmk_notifier → the runner's handshake sets `HOST_CAPABLE` → the poll
thread's `last_status` tracker flips `NoModule → Connected` → tray shows
`● Device Connected`.

**User Journey**: Disconnected (`○`) → plug in vanilla board → NoModule (`⚠`) on
the next ≤3s poll → flash firmware + handshake → Connected (`●`) on the next poll.

**Pain Points Addressed**: Eliminates the false-green "Connected" for boards that
can't act on QMKonnect's magic bytes. The `NoModule` state tells the user exactly
what to do ("flash it").

## Why

- **It is the macOS/Windows half of F13 (the headline).** `spec/DEVICE_DISCOVERY.md`
  §3 + `spec/UI.md` §4 mandate a three-state status line. S1 delivered the resolver
  (`DeviceStatus` + `device_status()`); this subtask renders it on macOS/Windows.
  (Linux/SNI + the Disconnected→NoModule `notify-send` is S3.)
- **The handshake lifecycle is deliberately left on the bool.** `handshake_action`
  Gain/Loss is a Tier-1 *presence* event; gating it on the three-state value would
  mis-fire (e.g. skip a Loss when a board leaves but a NoModule state lingers, or
  spurious handshakes on NoModule↔Connected). The item explicitly requires the
  handshake stay bool-keyed.
- **The dual-tracker is what makes NoModule→Connected visible.** That transition
  happens while the bool stays `true` (the handshake sets `HOST_CAPABLE`), so a
  bool-keyed UI event would never fire. A separate `last_status` tracker catches
  it on the next 3s poll.

## What

Five coordinated edits to `src/tray.rs` (all already `#[cfg(any(target_os =
"macos", target_os = "windows"))]`-gated; the cfg gates are UNCHANGED). Exact
before→after in `research/notes.md` §1; summarized here.

### (A) `UserEvent` variant — line ~41
```rust
#[cfg(any(target_os = "macos", target_os = "windows"))]
DeviceStatus(crate::core::notifier::DeviceStatus),   // was: DeviceStatus(bool)
```
Fully-qualified type (the file fully-qualifies all `crate::core::notifier::*` refs;
no module-level import — see Gotchas).

### (B) `device_status_text` — line ~660 → three-branch match
```rust
/// Label for the macOS/Windows device-status menu item (line 2). Three states
/// per `spec/UI.md` §4 / `spec/DEVICE_DISCOVERY.md` §3: a solid dot (≥1 capable
/// board), a warning glyph (QMK board present but no qmk_notifier module), or a
/// hollow dot (0 Tier-1 boards). The "No module" warning is the truthful F13
/// value — see `device_status()` in `src/core/notifier.rs`.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn device_status_text(status: crate::core::notifier::DeviceStatus) -> String {
    use crate::core::notifier::DeviceStatus; // function-local use → terse arms, no cfg-import issue
    match status {
        // U+25CF BLACK CIRCLE — solid dot; ≥1 capable board.
        DeviceStatus::Connected => "\u{25CF}  Device Connected".to_string(),
        // U+26A0 WARNING SIGN — QMK board present, no qmk_notifier module.
        DeviceStatus::NoModule => {
            "\u{26A0}  QMK board found \u{2014} no qmk_notifier module (flash it)".to_string()
        }
        // U+25CB WHITE CIRCLE — hollow dot; 0 Tier-1 boards.
        DeviceStatus::Disconnected => "\u{25CB}  No Device Connected".to_string(),
    }
}
```
> Glyphs as `\u{}` escapes (matches the existing `\u{25CF}`/`\u{25CB}` style). Two
> spaces after every glyph. The No-module em-dash is `\u{2014}` (NOT a hyphen).
> The doc-comment cites `spec/UI.md` §4 (Mode A requirement).

### (C) First-paint MenuItem — line ~316 (only the argument changes; `false` stays)
```rust
let device_status_i = MenuItem::new(
    device_status_text(crate::core::notifier::device_status()),  // was: is_device_connected()
    false,  // disabled (non-clickable label) — the "No module" warning stays a disabled item
    None,
);
```

### (D) Poll thread — line ~384-406: DUAL tracker (handshake on bool; UI event on DeviceStatus)
```rust
let mut last: Option<bool> =
    Some(crate::core::notifier::startup_device_was_connected());   // handshake (UNCHANGED)
let mut last_status: Option<crate::core::notifier::DeviceStatus> =
    Some(crate::core::notifier::device_status());                  // NEW: UI-event tracker (seed ⇒ no spurious first-tick event)
loop {
    let connected = crate::core::notifier::is_device_connected();
    if last != Some(connected) {
        // ---- handshake block: UNCHANGED, stays keyed on the Tier-1 bool ----
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
    }
    // ---- UI status: three-state, sent only on ITS transition (NEW) ----
    // Computed AFTER the handshake block so a same-tick Gain + perform_handshake
    // (which may set HOST_CAPABLE ⇒ Connected) is reflected in the payload now.
    let status = crate::core::notifier::device_status();
    if last_status != Some(status) {
        let _ = status_proxy.send_event(UserEvent::DeviceStatus(status));
        last_status = Some(status);
    }
    std::thread::sleep(std::time::Duration::from_secs(3));
}
```

### (E) Event-loop arm — line ~507-510 (rename the binding; type is inferred)
```rust
#[cfg(any(target_os = "macos", target_os = "windows"))]
Event::UserEvent(UserEvent::DeviceStatus(status)) => {
    device_status_i.set_text(device_status_text(status));
}
```

### (F) NEW: a cfg-gated unit test for the three text mappings
```rust
#[cfg(all(test, any(target_os = "macos", target_os = "windows")))]
mod tests {
    use super::device_status_text;
    use crate::core::notifier::DeviceStatus;

    #[test]
    fn test_device_status_text_three_states() {
        assert_eq!(device_status_text(DeviceStatus::Connected),
            "\u{25CF}  Device Connected");
        assert_eq!(device_status_text(DeviceStatus::NoModule),
            "\u{26A0}  QMK board found \u{2014} no qmk_notifier module (flash it)");
        assert_eq!(device_status_text(DeviceStatus::Disconnected),
            "\u{25CB}  No Device Connected");
    }
}
```
> There is NO existing test module in `src/tray.rs` — this adds one. It is gated
> to macOS/Windows (where `device_status_text` exists); on default Linux CI the
> whole file is skipped (module-level `#![cfg(...)]`), on non-Hyprland Linux the
> mod is cfg'd out. Deterministic — no hardware, no `DebounceState`.

### Success Criteria
- [ ] `UserEvent::DeviceStatus` carries `crate::core::notifier::DeviceStatus` (A).
- [ ] `device_status_text(DeviceStatus) -> String` has the three exact strings (B); its doc cites `spec/UI.md` §4.
- [ ] First-paint calls `device_status()`; `false` (disabled) preserved (C).
- [ ] Poll thread keeps the handshake block byte-unchanged on `is_device_connected()`; adds a separate `last_status` tracker + transition-send of the three-state event (D).
- [ ] Event-loop arm binds `status` and calls `device_status_text(status)` (E).
- [ ] The three-state text test exists and passes on macOS/Windows (F).
- [ ] `cargo build` zero warnings; `cargo clippy --bin qmkonnect` no new warnings; `cargo fmt --check` exit 0.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` green (existing + new).
- [ ] No file other than `src/tray.rs` modified.

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed
> to implement this successfully?"_ — **Yes.** The S1 contract (`DeviceStatus` +
> `device_status()`), the five exact before→after edits, the dual-tracker rationale
> (why the handshake must stay bool-keyed while the UI event tracks DeviceStatus),
> the exact three strings (glyph escapes + em-dash + spacing), the cfg-gating
> reality, the ready-to-paste test, and the verified validation commands are all below.

### Documentation & References

```yaml
# MUST READ — the sibling PRP whose output S2 consumes (the DeviceStatus CONTRACT)
- file: /home/dustin/projects/qmkonnect/plan/005_8b95ea464bd9/P1M1T1S1/PRP.md
  why: "Defines the exact DeviceStatus enum (Connected/NoModule/Disconnected; derives
        Debug,Clone,Copy,PartialEq,Eq) and the no-arg device_status() -> DeviceStatus
        resolver in src/core/notifier.rs. S2 calls crate::core::notifier::device_status()
        and pattern-matches on crate::core::notifier::DeviceStatus. is_device_connected(),
        host_capable(), handshake_action(), perform_handshake(), reset_handshake_state(),
        startup_device_was_connected() are UNCHANGED and remain available."
  section: "What (a) enum", "What (b) resolver"
  critical: "device_status() sends NO HID command — it reads is_device_connected() +
             host_capable(). Do NOT add probing in tray.rs. The handshake lifecycle MUST
             stay keyed on the bool (is_device_connected()), NOT on DeviceStatus."

# MUST READ — the authoritative three-state table (the text + glyphs S2 renders)
- file: /home/dustin/projects/qmkonnect/spec/UI.md
  why: "§4 'Device-Connection Status Indicator' is the verbatim source for the three tray
        strings: ●  Device Connected / ⚠  QMK board found — no qmk_notifier module (flash it)
        / ○  No Device Connected. The device_status_text doc-comment MUST cite this section
        (Mode A)."
  section: "4. Device-Connection Status Indicator" (lines 233-247)
  critical: "Two spaces after every glyph. The No-module em-dash is U+2014 (—), not a hyphen.
             P1 does NOT do the '● N Devices Connected' pluralization (that is P3's
             classify_devices); use the singular 'Device Connected' text exactly."

# MUST READ — the spec the doc-comment also cites (semantics of the three states)
- file: /home/dustin/projects/qmkonnect/spec/DEVICE_DISCOVERY.md
  why: "§3 'Device-Status Semantics (three states)' defines WHEN each state holds (Connected
        = ≥1 capable; NoModule = ≥1 Tier-1, 0 capable; Disconnected = 0 Tier-1) and confirms
        the poll cadence is unchanged (transitions drive the UI, not every poll)."
  section: "3. Device-Status Semantics (three states)"

# MUST READ — the file being edited (confirm exact current code before editing)
- file: /home/dustin/projects/qmkonnect/src/tray.rs
  why: "Contains all five edit sites: UserEvent (36-46, variant @41), first-paint MenuItem
        (315-319), poll thread (384-406), event-loop arm (507-510), device_status_text
        (660-669). Line 1 is the module-level cfg gate `#![cfg(not(all(target_os=\"linux\",
        feature=\"hyprland\")))]`. The file fully-qualifies crate::core::notifier::* (no use
        import)."
  pattern: "Existing device_status_text uses `\\u{25CF}`/`\\u{25CB}` escapes (not raw glyphs).
            The poll thread seeds `last = Some(startup_device_was_connected())` to avoid a
            spurious first-tick handshake — mirror that for `last_status`."
  gotcha: "tray.rs is NOT compiled on default (Hyprland) Linux, and the macOS/Windows items
           are further cfg'd. A module-level `use crate::core::notifier::DeviceStatus;` would
           be an UNUSED IMPORT on non-Hyprland Linux (uses cfg'd out). Use the FUNCTION-LOCAL
           `use` inside device_status_text (it's already cfg-gated) — see What (B)."

# MUST READ — the tray-surfaces architecture (the five-site map + parity notes)
- file: /home/dustin/projects/qmkonnect/plan/005_8b95ea464bd9/architecture/tray_surfaces.md
  why: "Maps every edit site with line numbers; confirms macOS/Windows is TEXT-ONLY (no icon
        dim, no tooltip change — those are Linux/S3). Confirms the handshake block lives in
        the SAME poll thread as the status event (lines 390-401), and that the event is the
        ONLY consumer of UserEvent::DeviceStatus."
  section: "macOS/Windows Tray (src/tray.rs)" — UserEvent, device_status_text, Status Poll, Event Loop Arm, First-Paint
  critical: "macOS/Windows = text only. Do NOT add icon-dim/alpha logic (that's Linux/S3:
             DIM_ALPHA ~35%). The 'No module' item is a disabled MenuItem (parity with today)."

# REFERENCE — research notes for this subtask (exact before→after + dual-tracker rationale)
- docfile: /home/dustin/projects/qmkonnect/plan/005_8b95ea464bd9/P1M1T1S2/research/notes.md
  why: "§1 = the five sites with exact before→after. §1 Site D = the dual-tracker design and
        WHY (the NoModule→Connected transition happens while the bool stays true). §2 = the
        cfg-gating reality (why a function-local `use` beats a module-level import). §3 = the
        new test. §5 = the S2 boundary (notifier.rs = S1; linux_tray.rs = S3)."
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/                       # THIS repo
├── spec/
│   ├── UI.md                    # §4 = the three-state text table (doc cites it; Mode A)
│   └── DEVICE_DISCOVERY.md      # §3 = the three-state semantics
├── src/
│   ├── core/notifier.rs         # S1 output: DeviceStatus + device_status() (CALL ONLY)
│   ├── tray.rs                  # <-- FILE TO EDIT (macOS/Windows tray). module-level cfg-gated.
│   └── linux_tray.rs            # S3 (Linux SNI) — NOT touched here
└── plan/005_8b95ea464bd9/architecture/tray_surfaces.md   # the five-site map + parity notes
```

### Desired Codebase tree with files to be modified

```bash
src/
└── tray.rs   # MODIFIED ONLY — UserEvent variant, device_status_text, first-paint, poll thread,
              #                     event-loop arm, + one cfg-gated test mod. cfg gates unchanged.
```

> No new files. `src/core/notifier.rs` (S1) and `src/linux_tray.rs` (S3) are NOT touched.

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL: the handshake lifecycle MUST stay keyed on is_device_connected() (the bool).
//   Do NOT gate handshake_action/perform_handshake/reset on DeviceStatus. The handshake is a
//   Tier-1 PRESENCE event; the three-state value can flip (NoModule<->Connected) while presence
//   is unchanged. The item explicitly forbids gating the handshake on the three-state value.
//   => Keep the handshake block (Site D) byte-for-byte; only ADD the last_status tracker + send.

// CRITICAL: use a SEPARATE last_status tracker for the UI event (do not reuse `last`).
//   The headline NoModule->Connected transition happens while the bool stays `true` (the
//   handshake sets HOST_CAPABLE). A bool-keyed event would NEVER fire for it, leaving the UI
//   stuck on "No module". The separate Option<DeviceStatus> tracker catches it on the next poll.

// CRITICAL: compute `status` AFTER the handshake block in the loop.
//   A same-tick Gain + perform_handshake may set HOST_CAPABLE => Connected. Reading device_status()
//   after the handshake block makes the payload reflect that immediately (within the 3s cadence).

// CRITICAL: seed last_status = Some(device_status()) to avoid a spurious first-tick event.
//   The first-paint (Site C) already rendered the correct text synchronously; the seed mirrors
//   today's `last = Some(startup_device_was_connected())` no-spurious-first-tick philosophy.

// CRITICAL: do NOT add a module-level `use crate::core::notifier::DeviceStatus;`.
//   tray.rs line 1 is `#![cfg(not(all(target_os="linux", feature="hyprland")))]` — on non-Hyprland
//   Linux the file compiles but all DeviceStatus uses are cfg'd to macOS/Windows, so a module-level
//   import is UNUSED there => warning. Use the FUNCTION-LOCAL `use` inside device_status_text
//   (which is itself cfg-gated). Fully-qualify in the UserEvent variant + the signature param.

// CRITICAL: glyphs as \u{} escapes; two spaces; em-dash \u{2014} in the No-module line.
//   Match the existing "\u{25CF}  Device Connected" style (escape + two spaces). The No-module
//   string's dash is an em-dash (U+2014), NOT '-' or '--'. The unit test pins the exact bytes.

// NOTE: macOS/Windows tray is TEXT-ONLY for this feature.
//   Do NOT add icon alpha/dim or tooltip logic — that's Linux/S3 (DIM_ALPHA ~35%). The "No module"
//   warning is a disabled MenuItem (the `false` arg at first-paint), same as today's hollow-dot.

// NOTE: tray.rs has NO existing test module; the new one is gated to macOS/Windows.
//   `#[cfg(all(test, any(target_os = "macos", target_os = "windows")))] mod tests`. On default
//   Linux CI the whole file is skipped; on non-Hyprland Linux the mod is cfg'd out. The test is
//   pure (string mapping) — no hardware, no DebounceState, no single-threading concern.

// NOTE: tests run single-threaded: cargo test --bin qmkonnect -- --test-threads=1 (AGENTS.md).
//   Shared global debouncer/mock state. The new tray.rs test is pure but follows the protocol.

// NOTE: P1 does NOT do the "● N Devices Connected" pluralization.
//   Use the singular "Device Connected". The per-board count + classify_devices is P3 (a later
//   milestone); it layers on WITHOUT changing this rendering. Do not add a count here.

// NOTE: device_status() sends NO HID command (S1 contract). tray.rs must not add any probing.
//   The cheap 3s poll stays a read of existing state (is_device_connected enumerate +
//   host_capable AtomicBool). Adding I/O would violate the contract + break the cheap-poll NFR.
```

## Implementation Blueprint

### Data models and structure

No new data models. S2 consumes S1's `DeviceStatus` enum and retypes one `UserEvent`
variant + one function parameter. The only structural addition is the `last_status:
Option<DeviceStatus>` tracker in the poll thread and the small test module.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CONFIRM the exact anchors + S1 output
  - READ: src/tray.rs — UserEvent (36-46), first-paint (315-319), poll thread (384-406),
          event-loop arm (507-510), device_status_text (660-669). Confirm line 1 module cfg.
  - CONFIRM S1 landed: grep -n "pub enum DeviceStatus\|pub fn device_status" src/core/notifier.rs
          => both present. (If absent, S1 hasn't landed — STOP; S2 depends on it.)
  - READ: spec/UI.md §4 (233-247) + spec/DEVICE_DISCOVERY.md §3 (the three strings + semantics).
  - READ: plan/005_8b95ea464bd9/architecture/tray_surfaces.md (the five-site map).
  - GOAL: know the five exact edit sites + the verbatim strings.

Task 2: RETYPE the UserEvent variant (Site A, ~line 41)
  - EDIT: DeviceStatus(bool) -> DeviceStatus(crate::core::notifier::DeviceStatus).
  - KEEP: the #[cfg(any(target_os = "macos", target_os = "windows"))] gate. KEEP other variants.

Task 3: REWRITE device_status_text (Site B, ~line 660)
  - REPLACE: the 2-branch bool fn with the 3-branch DeviceStatus match (What (B)).
  - DERIVES: none (pure fn). Function-local `use crate::core::notifier::DeviceStatus;`.
  - DOC: the doc-comment cites spec/UI.md §4 + spec/DEVICE_DISCOVERY.md §3 (Mode A).
  - STRINGS: exact — \u{25CF}  Device Connected / \u{26A0}  QMK board found \u{2014} no
          qmk_notifier module (flash it) / \u{25CB}  No Device Connected.
  - KEEP: the #[cfg(any(target_os = "macos", target_os = "windows"))] gate.

Task 4: UPDATE first-paint (Site C, ~line 316)
  - EDIT: the argument device_status_text(is_device_connected()) -> device_status_text(device_status()).
  - KEEP: the `false` (disabled) arg and the cfg gate.

Task 5: UPDATE the poll thread (Site D, ~line 384-406) — DUAL tracker
  - ADD: `let mut last_status: Option<crate::core::notifier::DeviceStatus> =
          Some(crate::core::notifier::device_status());` next to the existing `last` seed.
  - KEEP: the `last` seed + the entire handshake block (last != Some(connected) => handshake_action)
          byte-for-byte. Do NOT touch the handshake.
  - ADD: after the handshake block, compute `let status = device_status();` and a
          `if last_status != Some(status) { send_event(DeviceStatus(status)); last_status = Some(status); }`.
  - KEEP: the 3s sleep + the cfg gate on the spawned-thread block.

Task 6: UPDATE the event-loop arm (Site E, ~line 507-510)
  - EDIT: bind `status` (was `connected`); call device_status_text(status).
  - KEEP: the cfg gate on the arm.

Task 7: ADD the cfg-gated test module (Site F)
  - ADD: at end of file, #[cfg(all(test, any(target_os = "macos", target_os = "windows")))] mod tests
          with test_device_status_text_three_states asserting the three exact strings (What (F)).

Task 8: VALIDATE (do not skip)
  - RUN: cargo fmt, cargo build, cargo clippy --bin qmkonnect, cargo fmt --check.
  - RUN: cargo test --bin qmkonnect -- --test-threads=1.
  - EXPECT: build 0 warnings; clippy no new warnings; fmt --check exit 0; tests green (existing + new on macOS/Windows).
  - IF "unused import: DeviceStatus" on non-Hyprland Linux: you added a module-level use — remove it;
          use the function-local use inside device_status_text (Gotchas).
  - IF "mismatched types: expected bool, found DeviceStatus" near the poll thread: you reused `last`
          (bool) for the status comparison — use the separate `last_status` tracker.
```

### Implementation Patterns & Key Details

```rust
// === WHY THE HANDSHAKE STAYS ON THE BOOL (the core design constraint) ===
//   handshake_action(prev_presence, cur_presence) => Gain/Loss/None is a Tier-1 PRESENCE event.
//   DeviceStatus can flip NoModule<->Connected while presence is unchanged (the handshake itself
//   flips host_capable). Gating the handshake on DeviceStatus would mis-fire. So:
//     - handshake block: keyed on `connected: bool` (UNCHANGED).
//     - UI event: keyed on `status: DeviceStatus` (NEW, separate tracker).

// === WHY last_status IS A SEPARATE TRACKER (the headline fix) ===
//   NoModule -> Connected: board present the whole time (bool stays true), handshake sets
//   HOST_CAPABLE. A bool-keyed event never fires for this. The separate Option<DeviceStatus>
//   tracker catches it on the next 3s poll => the UI flips to "Connected". Without this, the
//   tray would stay stuck on "No module" after a successful handshake.

// === WHY FUNCTION-LOCAL `use` (not module-level) ===
//   tray.rs is module-cfg'd out on Hyprland Linux, and the DeviceStatus items are further
//   cfg'd to macOS/Windows. A module-level `use DeviceStatus` is unused on non-Hyprland Linux
//   (warning). The function-local `use` inside device_status_text (itself cfg-gated) is clean.

// === WHY \u{} ESCAPES + EXACT SPACING ===
//   Matches the existing "\u{25CF}  Device Connected" style. Two spaces after each glyph. The
//   No-module dash is an em-dash (\u{2014}). The unit test pins the exact bytes so a typo
//   (e.g. a hyphen, or one space) fails loudly.
```

### Integration Points

```yaml
SOURCE FILES:
  - modify: "src/tray.rs ONLY"

PUBLIC API SURFACE:
  - none new. S2 CONSUMES S1's pub DeviceStatus + device_status().
  - changes: "UserEvent::DeviceStatus variant payload bool -> DeviceStatus (private enum,
              tray.rs-internal). device_status_text signature bool -> DeviceStatus (private fn)."

DEPENDENCIES / Cargo.toml:
  - none. No new deps. No HID I/O.

UPSTREAM CONTRACT (S1 — assumed LANDED when S2 runs):
  - consumes: "crate::core::notifier::DeviceStatus {Connected,NoModule,Disconnected};
               crate::core::notifier::device_status() -> DeviceStatus (no-arg, no HID I/O).
               is_device_connected/handshake_action/perform_handshake/reset_handshake_state/
               startup_device_was_connected UNCHANGED."

DOWNSTREAM / SIBLINGS (do NOT implement — listed for awareness):
  - P1.M1.T1.S3: "src/linux_tray.rs renders the three states (SNI) + the Disconnected->NoModule
                  one-shot notify-send + icon dim. Its parity test (linux_tray.rs:948) mirrors
                  these three strings — keep them byte-identical."
  - P3.M1.T1: "classify_devices() + ClassifiedDevice — the per-board Tier-2 mechanism + the
               '● N Devices Connected' pluralization. P1's rendering uses the singular text;
               P3 layers the count WITHOUT changing DeviceStatus variants."

VALIDATION CONSUMERS:
  - The tray line-2 status is the user-facing surface; live three-state observation is via the
    AGENTS.md build/install/open loop (needs hardware). The deterministic proof is the S1
    resolver tests + the S2 device_status_text unit test.
```

## Validation Loop

> All commands run from the repo root: `/home/dustin/projects/qmkonnect`.
> On default (Hyprland) Linux, `src/tray.rs` is not compiled — run these on macOS, Windows,
> or a non-Hyprland Linux build to exercise the S2 code.

### Level 1: Syntax & Style

```bash
cd /home/dustin/projects/qmkonnect

cargo fmt
cargo build 2>&1 | tee /tmp/build.log
# Expected: "Finished" — zero warnings. If "unused import: DeviceStatus" on non-Hyprland Linux,
# you added a module-level use — switch to the function-local use (Gotchas).

cargo clippy --bin qmkonnect 2>&1 | tee /tmp/clippy.log | grep -iE 'warning|error' || echo "clippy clean"
# Expected: no new warnings. (The cfg-gated test mod + the pure device_status_text are clean.)

cargo fmt --check
# Expected: exit 0.
```

### Level 2: Unit Tests (the deterministic gate)

```bash
cd /home/dustin/projects/qmkonnect

# The new tray.rs test (runs on macOS/Windows only):
cargo test --bin qmkonnect test_device_status_text_three_states -- --test-threads=1 --nocapture
# Expected: 1 passed (on macOS/Windows). On non-Hyprland Linux the mod is cfg'd out (0 collected
# for this name is fine there). On default Hyprland Linux the whole file is absent.

# Full suite — single-threaded (AGENTS.md: shared global debouncer/mock state):
cargo test --bin qmkonnect -- --test-threads=1 2>&1 | tail -3
# Expected: "test result: ok. <N+1> passed; 0 failed; ..." on macOS/Windows (N = pre-existing;
# the +1 is the new tray test). On Linux the tray test isn't collected (file/mod cfg'd) — still 0 failed.
```

### Level 3: Integration (live three-state observation — needs hardware + the dev loop)

```text
NOT a CI gate (requires real HID hardware). Verified via the AGENTS.md dev loop on macOS/Windows:
  cargo test --bin qmkonnect -- --test-threads=1
  cd packaging/macos && ./clean.sh && ./build.sh && ./install.sh   # (macOS)
  open /Applications/QMKonnect.app
Then observe line 2 across:
  - No board plugged in          -> "○  No Device Connected"
  - Vanilla QMK board (no module)-> "⚠  QMK board found — no qmk_notifier module (flash it)"
  - After flashing qmk_notifier  -> flips to "●  Device Connected" within ~3s (the last_status tracker)
The deterministic proof of the TEXT mapping is the Level-2 unit test; live UX confirms the
poll-thread wiring + the handshake-driven NoModule->Connected flip.
```

### Level 4: Scope-preservation grep (prove the handshake + notifier are untouched)

```bash
cd /home/dustin/projects/qmkonnect

# (a) The handshake block in the poll thread is byte-unchanged (still keyed on the bool).
grep -nA2 'handshake_action(last, connected)' src/tray.rs
# Expected: the Gain/Loss/None match arms still reference `connected` (bool), unchanged.

# (b) No module-level `use ... DeviceStatus` (would warn on non-Hyprland Linux).
grep -nE '^use .*DeviceStatus' src/tray.rs && echo "BUG: module-level use (remove)" || echo "ok: no module-level DeviceStatus use"

# (c) src/core/notifier.rs and src/linux_tray.rs untouched.
git status --short src/core/notifier.rs src/linux_tray.rs
# Expected: empty.

# (d) device_status_text has exactly three branches + cites UI.md §4.
grep -nA20 'fn device_status_text' src/tray.rs | grep -E 'Connected|NoModule|Disconnected|UI\.md'
# Expected: three variant arms + a UI.md §4 doc reference.
```

## Final Validation Checklist

### Technical Validation
- [ ] Level 1: `cargo build` zero warnings; `cargo clippy --bin qmkonnect` no new warnings; `cargo fmt --check` exit 0.
- [ ] Level 2: `cargo test --bin qmkonnect -- --test-threads=1` green; the new tray test passes on macOS/Windows.
- [ ] Level 4 (a): handshake block byte-unchanged (still keyed on `connected: bool`).
- [ ] Level 4 (b): no module-level `use ... DeviceStatus`.
- [ ] Level 4 (c): `src/core/notifier.rs` + `src/linux_tray.rs` unmodified.

### Feature Validation
- [ ] `UserEvent::DeviceStatus` carries `DeviceStatus` (A).
- [ ] `device_status_text` has the three exact strings (●/⚠/○, two spaces, em-dash) + cites UI.md §4 (B).
- [ ] First-paint calls `device_status()`; `false` (disabled) preserved (C).
- [ ] Poll thread: handshake block unchanged on the bool; separate `last_status` tracker sends the three-state event on transition (D).
- [ ] Event-loop arm binds `status` (E).
- [ ] The three-state text test exists (F).

### Code Quality Validation
- [ ] Glyphs as `\u{}` escapes; spacing/em-dash exact (matches existing style).
- [ ] Function-local `use` inside `device_status_text` (no module-level import; no cfg warning).
- [ ] The cfg gates on all five sites are unchanged.
- [ ] Tests single-threaded (`--test-threads=1`, AGENTS.md); the new test is pure.

### Documentation & Deployment
- [ ] Mode A: `device_status_text` doc-comment cites `spec/UI.md` §4 (+ DEVICE_DISCOVERY.md §3).
- [ ] No user-facing doc file changed here (P4 handles docs/*.md).
- [ ] No environment variables, config, or Cargo.toml changes.

---

## Anti-Patterns to Avoid

- ❌ Don't gate the handshake (`handshake_action`/`perform_handshake`/`reset`) on `DeviceStatus` —
  it MUST stay keyed on the `is_device_connected()` bool. The handshake block is byte-unchanged.
- ❌ Don't reuse the bool `last` tracker for the UI event — the NoModule→Connected flip happens while
  the bool is unchanged; use the separate `last_status: Option<DeviceStatus>` tracker.
- ❌ Don't compute `status` BEFORE the handshake block in the loop — compute it AFTER so a same-tick
  Gain+handshake (setting HOST_CAPABLE) is reflected in the payload immediately.
- ❌ Don't seed `last_status = None` — that emits a redundant first-tick event (first-paint already
  rendered the text). Seed `Some(device_status())`.
- ❌ Don't add a module-level `use crate::core::notifier::DeviceStatus;` — it's an unused import on
  non-Hyprland Linux. Use the function-local `use` inside `device_status_text`.
- ❌ Don't paste raw emoji glyphs (●⚠○) — use `\u{25CF}`/`\u{26A0}`/`\u{25CB}` escapes (file style).
- ❌ Don't use a hyphen or `--` in the No-module string — it's an em-dash `\u{2014}`.
- ❌ Don't add icon-dim/alpha/tooltip logic to macOS/Windows — that's Linux/S3 (text-only here).
- ❌ Don't make the "No module" item clickable — it stays a disabled MenuItem (`false`), parity today.
- ❌ Don't add the "● N Devices Connected" pluralization — that's P3 (`classify_devices`); use singular.
- ❌ Don't add HID I/O / probing in tray.rs — `device_status()` is a pure read (S1 contract).
- ❌ Don't touch `src/core/notifier.rs` (S1) or `src/linux_tray.rs` (S3).
- ❌ Don't drop the cfg gates on the five sites — they're load-bearing (platform exclusion).
- ❌ Don't run tests without `--test-threads=1` (AGENTS.md: shared global debouncer/mock state).

---

**Confidence Score: 9/10** for one-pass implementation success. The deliverable is five
small, fully-specified edits (with exact before→after) + one pure test, all consuming a
stable S1 contract (`DeviceStatus` + `device_status()`). The two design subtleties —
(a) the handshake must stay bool-keyed while the UI event tracks `DeviceStatus`
separately, and (b) the function-local `use` to avoid a cfg'd unused-import — are both
called out with rationale, and the scope-preservation greps make verification
deterministic. The one residual risk (a typo in the No-module em-dash/spacing) is
pinned by the unit test's exact-string assertions. Live three-state UX (the
NoModule→Connected flip) is verified via the AGENTS.md dev loop, not CI.
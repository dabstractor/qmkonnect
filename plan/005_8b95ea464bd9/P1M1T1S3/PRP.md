# PRP — P1.M1.T1.S3: Render three-state status in `src/linux_tray.rs` + the Disconnected→NoModule one-shot `notify-send`

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). ALL edits in ONE file:
> `src/linux_tray.rs`.
> **Scope:** The **Linux SNI tray** half of F13 (the three-state device status).
> Switch `QmkTray.device_connected: bool` → a `DeviceStatus` field; render the
> three states in the menu line, icon alpha, and tooltip; add a **one-shot
> `notify-send`** on the Disconnected→NoModule transition. The handshake
> lifecycle stays keyed on the Tier-1 `is_device_connected()` bool (dual-tracker,
> mirroring S2). `src/core/notifier.rs` (S1) and `src/tray.rs` (S2) are NOT touched.
> **⚠ Linux is a richer surface than S2**: S2 was text-only; S3 = text + icon dim
> + tooltip + the one-shot notification.

---

## Goal

**Feature Goal**: Render the truthful three-state device status on the Linux SNI
tray (`src/linux_tray.rs`): `●  Device Connected` / `⚠  QMK board found — no
qmk_notifier module (flash it)` / `○  No Device Connected`, with the icon at
**full alpha for Connected AND NoModule** (device present, just maybe not capable)
and **dimmed (~35%) for Disconnected**. On the **Disconnected→NoModule transition
only**, fire a **one-shot** `notify-send` (fires once per entry into NoModule,
re-arms on exit) with the same message + a link to `docs/qmk-integration.md`.

**Deliverable**: `src/linux_tray.rs` (only) with: (1) the `device_connected: bool`
field retyped to `device_status: DeviceStatus`; (2) a new private
`device_status_text(DeviceStatus) -> String` function (mirroring tray.rs/S2 for
parity + testability) consumed by `menu()`; (3) the menu line, icon-alpha
(`icon_pixmap`), hidden structural toggle, and tooltip all re-derived from the new
field; (4) the poll thread on a **dual tracker** (handshake on the bool
UNCHANGED; UI field on `last_status: Option<DeviceStatus>`); (5) a new module-level
`static NO_MODULE_NOTIFIED: AtomicBool` once-guard (modeled on
`RULES_INVALID_NOTIFIED`) firing `notify(...)` once on Disconnected→NoModule; (6)
the two existing tests updated (`status_text_uses_parity_glyphs`,
`new_tray_probes_initial_state`).

**Success Definition**: `cargo build` + `cargo clippy --all-targets -- -D warnings`
clean; `cargo test --bin qmkonnect -- --test-threads=1` green (existing + updated);
the three status strings are byte-identical to tray.rs/S2 (parity); the one-shot
notify fires exactly once per Disconnected→NoModule entry (re-arming on exit); the
handshake lifecycle stays bool-keyed; no file other than `src/linux_tray.rs` is
modified; the `QmkTray` status doc-comment cites `DEVICE_DISCOVERY.md` §3 + the
one-shot rationale (Mode A).

## User Persona (if applicable)

**Target User**: A Linux user who plugs in a vanilla QMK board (no qmk_notifier)
and currently sees a misleading `●  Device Connected` that silently does nothing.

**Use Case**: User connects a VIA-only board → SNI tray shows
`⚠  QMK board found — no qmk_notifier module (flash it)` (truthful) AND a desktop
notification fires once ("flash it — see docs/qmk-integration.md"). They flash
qmk_notifier → handshake sets `HOST_CAPABLE` → next 1s poll flips NoModule→Connected
→ tray shows `●  Device Connected` + the icon goes full-alpha.

**User Journey**: Disconnected (`○`, dimmed icon) → plug vanilla board → NoModule
(`⚠`, full-alpha icon) on next ≤1s poll + one desktop notification → flash firmware
+ handshake → Connected (`●`, full-alpha icon) on next poll.

**Pain Points Addressed**: Eliminates the false-green "Connected" for boards that
can't act on QMKonnect's magic bytes. The one-shot notify makes the "flash it"
guidance unmissable on first appearance (without nagging on every 1s poll).

## Why

- **It is the Linux half of F13 (the headline).** `spec/DEVICE_DISCOVERY.md` §3 +
  `spec/UI.md` §4 mandate a three-state status line. S1 delivered the resolver;
  S2 renders macOS/Windows; S3 renders Linux/SNI + the one-shot notify (Linux-only
  per §3).
- **Linux carries the richer UX.** Unlike macOS/Windows (text-only in S2), the SNI
  tray dims the icon on disconnect (visible in the bar in realtime) and §3 adds a
  one-shot desktop notification on the "you found a board but it lacks the module"
  transition — the most actionable moment for the user.
- **The handshake stays bool-keyed (dual-tracker).** `handshake_action` Gain/Loss
  is a Tier-1 *presence* event; gating it on `DeviceStatus` would mis-fire
  (NoModule↔Connected flips while presence is unchanged). The handshake block is
  byte-unchanged on the bool; a SEPARATE `last_status` tracker drives the UI field
  + the notify — exactly as S2 does (the headline NoModule→Connected flip happens
  while the bool stays `true`).
- **The one-shot guard prevents notification spam.** The poll runs every 1s; a
  board stuck in NoModule would re-notify every tick without the `AtomicBool`
  once-guard. Modeled on `RULES_INVALID_NOTIFIED` (fire-once / re-arm-on-exit).

## What

Six coordinated edits to `src/linux_tray.rs` (all inside the module's
`#[cfg(all(target_os = "linux", feature = "linux-tray"))]` gate — UNCHANGED).
Exact before→after in `research/notes.md`; summarized here.

### (A) Field + import — lines 65-68 + module top
```rust
// module-level (safe: the whole module is Linux+feature-gated; no cfg-unused-import risk)
use crate::core::notifier::DeviceStatus;

/// The tray item. ... status line (parity with macOS line 2; three states per
/// spec/DEVICE_DISCOVERY.md §3). ...
pub struct QmkTray {
    device_status: DeviceStatus,   // was: device_connected: bool
    dark_mode: bool,
}
```
The QmkTray doc-comment cites `spec/DEVICE_DISCOVERY.md` §3 + the one-shot notify
rationale (Mode A).

### (B) `new()` seed — line 76
```rust
device_status: crate::core::notifier::device_status(),   // was: is_device_connected()
```

### (C) NEW private `device_status_text` function (parity with tray.rs/S2)
Extract the inlined `if self.device_connected { ... } else { ... }` (menu():143-152)
into a function so (a) the parity test asserts on real output and (b) it is
byte-verifiable against tray.rs's same-named function. Place it near `detect_dark_mode`/`dim_icon` (the helper cluster):
```rust
/// Label for the Linux SNI device-status menu item (line 1). Three states per
/// `spec/UI.md` §4 / `spec/DEVICE_DISCOVERY.md` §3 — byte-identical to
/// `src/tray.rs::device_status_text` (parity; the test
/// `status_text_uses_parity_glyphs` pins it).
fn device_status_text(status: DeviceStatus) -> String {
    match status {
        // U+25CF BLACK CIRCLE — ≥1 capable board.
        DeviceStatus::Connected => "\u{25CF}  Device Connected".to_string(),
        // U+26A0 WARNING SIGN — QMK board present, no qmk_notifier module.
        DeviceStatus::NoModule =>
            "\u{26A0}  QMK board found \u{2014} no qmk_notifier module (flash it)".to_string(),
        // U+25CB WHITE CIRCLE — 0 Tier-1 boards.
        DeviceStatus::Disconnected => "\u{25CB}  No Device Connected".to_string(),
    }
}
```
> Glyphs as `\u{}` escapes (file style). Two spaces after every glyph. The
> No-module em-dash is `\u{2014}` (NOT a hyphen). Strings MUST be byte-identical
> to tray.rs/S2.

### (D) `menu()` — line 143 (status) + line 170 (hidden toggle)
```rust
// status line (was: inlined if/else):
let status = device_status_text(self.device_status);
// ...
// hidden structural toggle — present when a device is present (Connected OR NoModule),
// absent when Disconnected (preserves the LayoutUpdated count-change trick):
if self.device_status != DeviceStatus::Disconnected {
    items.push(MenuItem::Standard(StandardItem { label: String::new(), visible: false, ... }));
}
```
> The toggle must stay present-vs-absent (NOT three-way): it exists to change the
> item *count* on connect↔disconnect so ksni emits `LayoutUpdated`. A board in
> NoModule is still "present", so the toggle stays.

### (E) `icon_pixmap()` dim — line 128 + `tool_tip()` — line 95
```rust
// icon: full alpha when a device is present (Connected OR NoModule); dimmed for Disconnected.
.map(|i| vec![if self.device_status != DeviceStatus::Disconnected { i } else { dim_icon(i) }])
// tooltip: three-state (realtime indicator per the existing comment):
let description = match self.device_status {
    DeviceStatus::Connected => "Window activity notifier — device connected",
    DeviceStatus::NoModule => "Window activity notifier — QMK board found, no qmk_notifier module",
    DeviceStatus::Disconnected => "Window activity notifier — NO DEVICE CONNECTED",
};
```
> The icon-alpha logic is the S3-specific piece S2 did NOT have: NoModule keeps
> FULL alpha (the board IS present, just not capable), only Disconnected dims.

### (F) Poll thread — lines 259-301: DUAL tracker + one-shot notify
```rust
let mut last_device: Option<bool> =
    Some(crate::core::notifier::startup_device_was_connected());   // handshake (UNCHANGED)
let mut last_status: Option<DeviceStatus> =
    Some(crate::core::notifier::device_status());                  // NEW: UI + notify tracker (seed ⇒ no spurious first-tick)
let mut last_dark: Option<bool> = None;
let mut tick: u32 = 0;
loop {
    let connected = crate::core::notifier::is_device_connected();  // handshake bool
    let status = crate::core::notifier::device_status();           // UI three-state
    let dark = /* unchanged color-scheme poll */;
    tick = tick.wrapping_add(1);

    // ---- handshake: Tier-1 presence transition (UNCHANGED, stays bool-keyed) ----
    if last_device != Some(connected) {
        match crate::core::notifier::handshake_action(last_device, connected) {
            crate::core::notifier::HandshakeAction::Gain => crate::core::notifier::perform_handshake(verbose),
            crate::core::notifier::HandshakeAction::Loss => crate::core::notifier::reset_handshake_state(),
            crate::core::notifier::HandshakeAction::None => {}
        }
        last_device = Some(connected);
    }

    // ---- one-shot notify on Disconnected -> NoModule ONLY (NEW) ----
    if last_status == Some(DeviceStatus::Disconnected) && status == DeviceStatus::NoModule {
        // fire once per entry into NoModule (AtomicBool once-guard, mirrors RULES_INVALID_NOTIFIED):
        if !NO_MODULE_NOTIFIED.swap(true, Ordering::SeqCst) {
            notify(
                "QMK board found \u{2014} no qmk_notifier module",
                "This QMK board isn't running the qmk_notifier firmware QMKonnect talks to. \
                 Flash it: docs/qmk-integration.md",
            );
        }
    }
    // re-arm the one-shot when leaving NoModule (a later re-entry fires again):
    if status != DeviceStatus::NoModule {
        NO_MODULE_NOTIFIED.store(false, Ordering::SeqCst);
    }

    // ---- tray UI on status OR dark transition (keyed on status, NOT the bool) ----
    if last_status != Some(status) || last_dark != Some(dark) {
        last_status = Some(status);
        last_dark = Some(dark);
        let _ = poll_handle.update(|t: &mut QmkTray| {
            t.device_status = status;
            t.dark_mode = dark;
        });
    }
    std::thread::sleep(DEVICE_POLL_INTERVAL);
}
```
Plus the new module-level guard (place near the other module-level `static`s/consts, e.g. near `DEVICE_POLL_INTERVAL`):
```rust
/// One-shot guard for the Disconnected→NoModule `notify-send` (DEVICE_DISCOVERY.md §3).
/// `swap(true)` fires the notification the first time; `store(false)` re-arms it when
/// the device leaves NoModule — so the notification fires at most once per entry into
/// NoModule, never on every 1s poll tick. Mirrors `RULES_INVALID_NOTIFIED`
/// (src/core/notifier.rs).
static NO_MODULE_NOTIFIED: AtomicBool = AtomicBool::new(false);
```
> `Ordering` + `AtomicBool` must be in scope (add `use std::sync::atomic::{AtomicBool, Ordering};`
> if not already imported — check the file's existing imports).

### Success Criteria
- [ ] `QmkTray.device_status: DeviceStatus` (field renamed/retyped); doc-comment cites §3 + one-shot (A).
- [ ] `new()` seeds `device_status()` (B).
- [ ] `device_status_text(DeviceStatus) -> String` exists with the three exact strings (parity with tray.rs) (C).
- [ ] `menu()` uses `device_status_text`; the hidden toggle is present when `!= Disconnected` (D).
- [ ] `icon_pixmap()` full-alpha for `!= Disconnected`, dimmed for `Disconnected`; tooltip three-state (E).
- [ ] Poll thread: handshake block byte-unchanged on the bool; separate `last_status` tracker drives the UI field; one-shot `notify` on Disconnected→NoModule guarded by `NO_MODULE_NOTIFIED` (F).
- [ ] `NO_MODULE_NOTIFIED: AtomicBool` exists (modeled on `RULES_INVALID_NOTIFIED`).
- [ ] Tests updated; `status_text_uses_parity_glyphs` asserts the 3 strings from `device_status_text`.
- [ ] `cargo build` + `cargo clippy --all-targets -- -D warnings` clean; `cargo test --bin qmkonnect -- --test-threads=1` green.
- [ ] No file other than `src/linux_tray.rs` modified.

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed
> to implement this successfully?"_ — **Yes.** The S1 contract (`DeviceStatus` +
> `device_status()`), the S2 parity strings (exact bytes to match), the 11 enumerated
> `device_connected` sites with before→after, the dual-tracker design (why the
> handshake stays bool-keyed), the icon-alpha rule (full for Connected+NoModule),
> the one-shot `AtomicBool` guard idiom (quoted from `RULES_INVALID_NOTIFIED`), the
> notify() helper signature, the verbatim poll-thread rewrite, and the verified
> validation commands are all below.

> **BASELINE ALERT.** S1 is LANDED (`DeviceStatus`@719, `device_status()`@761 in
> notifier.rs). The crate currently COMPILES (0 errors). S3 is the Linux half; its
> edits are the only changes in the working tree. S2 (macOS/Windows) runs in parallel
> but edits a DIFFERENT file (`src/tray.rs`) — no conflict; the two must merely
> produce byte-identical status strings (parity).

### Documentation & References

```yaml
# MUST READ — the sibling PRP whose output S3 consumes (the DeviceStatus CONTRACT)
- file: /home/dustin/projects/qmkonnect/plan/005_8b95ea464bd9/P1M1T1S1/PRP.md
  why: "Defines the exact DeviceStatus enum (Connected/NoModule/Disconnected; derives
        Debug,Clone,Copy,PartialEq,Eq) and the no-arg device_status() -> DeviceStatus in
        notifier.rs. S3 calls crate::core::notifier::device_status() and pattern-matches
        on crate::core::notifier::DeviceStatus."
  section: "What (a) enum", "What (b) resolver"
  critical: "device_status() sends NO HID command. The handshake lifecycle MUST stay keyed
             on is_device_connected() (the bool), NOT on DeviceStatus (dual-tracker)."

# MUST READ — the parallel sibling PRP (the parity strings S3 must match byte-for-byte)
- file: /home/dustin/projects/qmkonnect/plan/005_8b95ea464bd9/P1M1T1S2/PRP.md
  why: "S2 defines the three exact status strings (\\u{25CF}  Device Connected / \\u{26A0}  QMK
        board found \\u{2014} no qmk_notifier module (flash it) / \\u{25CB}  No Device Connected)
        and the dual-tracker poll-thread pattern (handshake on bool; UI on DeviceStatus) that S3
        must mirror. S3's device_status_text MUST be byte-identical to tray.rs's."
  section: "What (B) device_status_text", "What (D) poll thread"
  critical: "PARITY: the parity test (linux_tray.rs:948) enforces the glyph match. Extract the
             same device_status_text function so both files are verifiably identical. S2 is
             TEXT-ONLY; S3 ALSO does icon-dim + tooltip + the one-shot notify (richer surface)."

# MUST READ — the authoritative three-state table + the one-shot notify requirement
- file: /home/dustin/projects/qmkonnect/spec/DEVICE_DISCOVERY.md
  why: "§3 'Device-Status Semantics (three states)' is the source of truth: the three
        conditions, tray text, icons (full-alpha for Connected+NoModule, dimmed for Disconnected),
        AND 'On Linux this also fires a one-shot notify-send on the Disconnected→No-module
        transition with the same message + a link to docs/qmk-integration.md.' The QmkTray
        doc-comment cites this section (Mode A)."
  section: "3. Device-Status Semantics (three states)"
  critical: "The one-shot notify is Linux-ONLY (§3). It fires on Disconnected->NoModule ONLY (not
             on every transition, not on Connected->NoModule). Full-alpha icon for NoModule
             (the board IS present)."

# MUST READ — the status strings (Mode A doc source for device_status_text)
- file: /home/dustin/projects/qmkonnect/spec/UI.md
  why: "§4 'Device-Connection Status Indicator' gives the verbatim three tray strings. The
        device_status_text doc-comment cites it (Mode A)."
  section: "4. Device-Connection Status Indicator"

# MUST READ — the tray-surfaces architecture (the Linux SNI site map + parity notes)
- file: /home/dustin/projects/qmkonnect/plan/005_8b95ea464bd9/architecture/tray_surfaces.md
  why: "Maps every Linux edit site with line numbers (struct@66, status@137, icon dim@156,
        poll@259, notify@846, DIM_ALPHA@923, parity test@948). Confirms Linux = text + icon +
        tooltip (vs macOS/Windows text-only). Confirms the parity requirement + the 1s poll cadence."
  section: "Linux SNI Tray (src/linux_tray.rs)" + "Key Parity Requirements"
  critical: "Icon dim is DIM_ALPHA=90 (~35%). NoModule needs FULL alpha. The hidden structural
             toggle (menu:170) drives LayoutUpdated via item-count change — keep it present-vs-absent."

# MUST READ — the once-guard idiom to mirror (RULES_INVALID_NOTIFIED)
- file: /home/dustin/projects/qmkonnect/src/core/notifier.rs
  why: "RULES_INVALID_NOTIFIED (line 299) is the exact AtomicBool fire-once/re-arm pattern S3
        mirrors as NO_MODULE_NOTIFIED: `static X: AtomicBool = AtomicBool::new(false);`
        `if !X.swap(true, SeqCst) { notify(...); }` (fire once, line 1168);
        `X.store(false, SeqCst);` (re-arm, line 1161)."
  section: "RULES_INVALID_NOTIFIED static (299) + its swap/store usage (1161/1168)"
  critical: "swap(true, SeqCst) returns false ONLY the first time -> fire. store(false, SeqCst)
             re-arms so a later re-entry into NoModule fires again. Do NOT use a plain bool
             (the poll thread is the only writer, but AtomicBool matches the codebase idiom + is
             correct under any future multi-thread use)."

# MUST READ — the file being edited (confirm exact current code before editing)
- file: /home/dustin/projects/qmkonnect/src/linux_tray.rs
  why: "Contains all 11 edit sites: struct (66-72), new() (74-79), tool_tip (95-111),
        icon_pixmap (113-130), menu status (143-152), menu hidden toggle (170-181), poll thread
        (259-301), notify helper (846-859), dim_icon/DIM_ALPHA (923-936), tests (948/953)."
  pattern: "ksni::Tray impl; menu() builds Vec<MenuItem>; icon_pixmap returns Vec<Icon>; the poll
            thread is spawned in spawn() and updates via poll_handle.update(|t: &mut QmkTray| {...}).
            notify(summary, body) shells out to notify-send with --app-name/--icon."
  gotcha: "device_connected is used in 11 places — ALL must become device_status (bool->DeviceStatus).
           Sites that used it as a presence bool (icon dim, hidden toggle, tooltip) become
           `!= Disconnected` (present = Connected OR NoModule). The poll thread needs a DUAL tracker:
           keep last_device (bool) for the handshake; add last_status (DeviceStatus) for the UI +
           the one-shot notify."

# REFERENCE — research notes (exact before→after + dual-tracker + once-guard idiom)
- docfile: plan/005_8b95ea464bd9/P1M1T1S3/research/notes.md
  why: "§11-site table; the verbatim poll-thread rewrite; the RULES_INVALID_NOTIFIED idiom quote;
        the notify message suggestion; the module-level import safety note."
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/                       # THIS repo
├── spec/
│   ├── DEVICE_DISCOVERY.md      # §3 = three-state table + the one-shot notify requirement (doc cites it)
│   └── UI.md                    # §4 = the three status strings (Mode A)
├── src/
│   ├── core/notifier.rs         # S1 output: DeviceStatus@719 + device_status@761 + RULES_INVALID_NOTIFIED@299 (CALL ONLY)
│   ├── tray.rs                  # S2 output (parallel): device_status_text (parity target — match byte-for-byte)
│   └── linux_tray.rs            # <-- FILE TO EDIT (Linux SNI tray; cfg linux+linux-tray, default-on)
└── plan/005_8b95ea464bd9/architecture/tray_surfaces.md   # the Linux site map + parity notes
```

### Desired Codebase tree with files to be modified

```bash
src/
└── linux_tray.rs   # MODIFIED ONLY — field retype + device_status_text fn + menu/icon/tooltip/poll/tests/once-guard
```

> No new files. `src/core/notifier.rs` (S1) and `src/tray.rs` (S2) are NOT touched.

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL: the handshake lifecycle MUST stay keyed on is_device_connected() (the bool).
//   Do NOT gate handshake_action/perform_handshake/reset on DeviceStatus. The handshake is a
//   Tier-1 PRESENCE event; DeviceStatus can flip NoModule<->Connected while presence is unchanged.
//   => Keep last_device (bool) + the handshake block; ADD last_status (DeviceStatus) for the UI + notify.

// CRITICAL: the tray update condition must key on last_status, NOT last_device.
//   The headline NoModule->Connected flip happens while the bool stays `true` (handshake sets
//   HOST_CAPABLE). If the update block kept `last_device != Some(connected)`, the tray would never
//   refresh for that flip. Key it on `last_status != Some(status) || last_dark != Some(dark)`.

// CRITICAL: icon alpha — full for Connected AND NoModule; dimmed for Disconnected ONLY.
//   NoModule means a board IS present (just not capable) -> full alpha (the dim is the "absent"
//   signal, not the "not capable" signal). So `if self.device_status != Disconnected { full } else { dim }`.

// CRITICAL: the hidden structural toggle stays present-vs-absent, NOT three-way.
//   It exists to change the item COUNT on connect<->disconnect so ksni emits LayoutUpdated. A
//   board in NoModule is still "present", so the toggle stays: `if self.device_status != Disconnected`.

// CRITICAL: the one-shot notify fires on Disconnected->NoModule ONLY.
//   Not on Connected->NoModule, not on every poll, not on every NoModule tick. The guard:
//   `if last_status == Some(Disconnected) && status == NoModule && !NO_MODULE_NOTIFIED.swap(true, SeqCst)`.
//   Re-arm with `if status != NoModule { NO_MODULE_NOTIFIED.store(false, SeqCst); }` so a later
//   re-entry fires again.

// CRITICAL: notify-rust is DELIBERATELY avoided — use the existing notify(summary, body) helper.
//   notify-rust's nested tokio runtime panics inside ksni's handler thread (spec §7.3). The existing
//   notify() shells out to notify-send. Do NOT replace it with notify-rust.

// CRITICAL: glyphs as \u{} escapes; two spaces; em-dash \u{2014} — byte-identical to tray.rs/S2.
//   The parity test pins these. A typo (hyphen for em-dash, one space) fails loudly.

// NOTE: seed last_status = Some(device_status()) to avoid a spurious first-tick event.
//   new() already rendered the correct text/icon synchronously; the seed mirrors today's
//   `last_device = Some(startup_device_was_connected())` no-spurious-first-tick philosophy.

// NOTE: linux_tray.rs is cfg(all(target_os="linux", feature="linux-tray")) — default-on on Linux.
//   A module-level `use crate::core::notifier::DeviceStatus;` is SAFE here (unlike tray.rs, where
//   it'd be unused on non-Hyprland Linux): the whole module compiles or doesn't. Use it for terse arms.

// NOTE: tests run on Linux CI (the module + its #[cfg(test)] mod are compiled when linux-tray is on).
//   cargo test --bin qmkonnect -- --test-threads=1 (AGENTS.md: shared global debouncer/mock state).

// NOTE: the parity test currently asserts standalone string LITERALS (not menu output). Extract
//   device_status_text as a function so the test asserts on real output (and matches tray.rs).

// NOTE: P1 does NOT do the "● N Devices Connected" pluralization (P3's classify_devices). Use the
//   singular "Device Connected" text exactly.
```

## Implementation Blueprint

### Data models and structure

No new data models. S3 consumes S1's `DeviceStatus` enum and retypes one struct
field. The structural additions are: the `device_status_text` helper fn, the
`last_status: Option<DeviceStatus>` poll-thread tracker, and the
`NO_MODULE_NOTIFIED: AtomicBool` once-guard.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CONFIRM anchors + S1/S2 output
  - READ: src/linux_tray.rs — struct (66-79), tool_tip (95-111), icon_pixmap (113-130),
          menu (137-181), poll thread (259-301), notify (846-859), dim_icon (923-936), tests (948-960).
  - CONFIRM S1 landed: grep -n 'pub enum DeviceStatus\|pub fn device_status' src/core/notifier.rs.
  - READ: spec/DEVICE_DISCOVERY.md §3 (three-state + one-shot notify) + spec/UI.md §4 (strings).
  - READ: the S2 PRP "What (B)" for the exact parity strings to copy byte-for-byte.
  - GOAL: know all 11 sites + the verbatim strings + the dual-tracker design.

Task 2: RETYPE the field + add the import (Site A)
  - EDIT line 68: device_connected: bool -> device_status: DeviceStatus.
  - ADD: module-level `use crate::core::notifier::DeviceStatus;` (near other use statements).
  - UPDATE line 65 doc-comment: cite spec/DEVICE_DISCOVERY.md §3 + the one-shot rationale (Mode A).

Task 3: ADD device_status_text (Site C — new function)
  - ADD the private fn (What C) in the helper cluster (near dim_icon/detect_dark_mode).
  - STRINGS: byte-identical to tray.rs/S2 (\u{25CF}/\u{26A0}/\u{25CB}, 2 spaces, \u{2014} em-dash).
  - DOC: cites spec/UI.md §4 + spec/DEVICE_DISCOVERY.md §3 + the tray.rs parity note.

Task 4: UPDATE menu() (Site D — lines 143 + 170)
  - line 143: replace the inlined if/else with `let status = device_status_text(self.device_status);`.
  - line 170: `if self.device_status != DeviceStatus::Disconnected { ... hidden toggle ... }`.

Task 5: UPDATE icon_pixmap + tool_tip (Site E — lines 128 + 95)
  - line 128: `if self.device_status != DeviceStatus::Disconnected { i } else { dim_icon(i) }`.
  - line 95: tooltip description -> three-branch match (Connected/NoModule/Disconnected prose).

Task 6: UPDATE new() (Site B — line 76)
  - device_connected: is_device_connected() -> device_status: device_status().

Task 7: ADD the once-guard + REWRITE the poll thread (Site F — lines 259-301)
  - ADD: `static NO_MODULE_NOTIFIED: AtomicBool = AtomicBool::new(false);` (module-level, near DEVICE_POLL_INTERVAL).
  - ADD (if needed): `use std::sync::atomic::{AtomicBool, Ordering};` (check existing imports first).
  - REWRITE the poll thread per What (F): keep last_device (bool) + handshake block; add last_status
          (DeviceStatus) seed; compute `status` each tick; one-shot notify on Disconnected->NoModule
          (guarded by NO_MODULE_NOTIFIED.swap); re-arm on leaving NoModule; update block keys on
          last_status (not last_device); poll_handle.update sets t.device_status = status.
  - KEEP: the 1s sleep, the color-scheme throttle (tick % COLOR_SCHEME_POLL_EVERY), the cfg gate.

Task 8: UPDATE the tests (lines 948 + 953)
  - status_text_uses_parity_glyphs (948): assert the 3 strings from device_status_text (assert
          device_status_text(Connected).starts_with('\u{25CF}'), NoModule starts_with('\u{26A0}'),
          Disconnected starts_with('\u{25CB}')). Keep the test name (or rename for clarity).
  - new_tray_probes_initial_state (953): tray.device_connected -> tray.device_status.
  - parse_id_handles_prefix_case_and_auto (960): UNCHANGED.

Task 9: VALIDATE (do not skip)
  - RUN: cargo fmt, cargo build, cargo clippy --all-targets -- -D warnings, cargo fmt --check.
  - RUN: cargo test --bin qmkonnect -- --test-threads=1.
  - EXPECT: build 0 warnings; clippy clean (-D warnings); fmt exit 0; tests green.
  - GREP: grep -n 'device_connected' src/linux_tray.rs -> 0 hits (the field is gone).
```

### Implementation Patterns & Key Details

```rust
// === WHY THE HANDSHAKE STAYS ON THE BOOL (the core design constraint) ===
//   handshake_action(prev_presence, cur_presence) => Gain/Loss/None is a Tier-1 PRESENCE event.
//   DeviceStatus flips NoModule<->Connected while presence is unchanged (the handshake itself
//   flips host_capable). Gating the handshake on DeviceStatus would mis-fire. So:
//     - handshake block: keyed on `connected: bool` (UNCHANGED, last_device tracker).
//     - UI field + notify: keyed on `status: DeviceStatus` (NEW, last_status tracker).

// === WHY last_status IS A SEPARATE TRACKER (the headline fix) ===
//   NoModule -> Connected: board present the whole time (bool stays true), handshake sets
//   HOST_CAPABLE. A bool-keyed UI update would never fire for this. The separate last_status
//   tracker catches it on the next 1s poll => the tray flips to "Connected" + full-alpha icon.

// === WHY THE ONE-SHOT GUARD (AtomicBool, not a plain bool) ===
//   The poll runs every 1s. A board stuck in NoModule would re-notify every tick without the guard.
//   swap(true, SeqCst) returns false ONLY the first time -> fire; store(false) on leaving NoModule
//   re-arms for a later re-entry. Mirrors RULES_INVALID_NOTIFIED exactly.

// === WHY ICON FULL-ALPHA FOR NOMODULE ===
//   The dim is the "absent" signal (no board at all). NoModule means a board IS present (just not
//   capable) -> full alpha. Only Disconnected dims. The menu warning glyph (⚠) carries the
//   "not capable" signal; the icon stays full-brightness either way a board is plugged in.

// === WHY EXTRACT device_status_text (parity + testability) ===
//   The parity test (948) must verify the ACTUAL menu output, not standalone literals. Extracting
//   the function lets the test assert on real output AND makes linux_tray.rs's device_status_text
//   byte-verifiable against tray.rs's same-named function (the parity requirement).
```

### Integration Points

```yaml
SOURCE FILES:
  - modify: "src/linux_tray.rs ONLY"

PUBLIC API SURFACE:
  - none new. S3 CONSUMES S1's pub DeviceStatus + device_status().
  - changes (all private to the cfg-gated module): "QmkTray.device_connected: bool ->
    device_status: DeviceStatus; new device_status_text private fn; NO_MODULE_NOTIFIED static."

DEPENDENCIES / Cargo.toml:
  - none. std::sync::atomic already used elsewhere (or add the use). No new deps. No HID I/O.

UPSTREAM CONTRACT (S1 — LANDED):
  - consumes: "crate::core::notifier::DeviceStatus {Connected,NoModule,Disconnected};
               crate::core::notifier::device_status() -> DeviceStatus (no-arg, no HID I/O).
               is_device_connected/handshake_action/perform_handshake/reset_handshake_state/
               startup_device_was_connected UNCHANGED."

PARITY CONTRACT (S2 — parallel):
  - S2's src/tray.rs::device_status_text produces the SAME three strings. S3's same-named fn must
    be byte-identical. The parity test (linux_tray.rs:948) is the deterministic proof.

DOWNSTREAM / SIBLINGS (do NOT implement):
  - P3.M1.T1: "classify_devices() + ClassifiedDevice — the per-board Tier-2 mechanism + the
               '● N Devices Connected' pluralization. P1 uses singular text; P3 layers the count
               WITHOUT changing DeviceStatus variants."

VALIDATION CONSUMERS:
  - The SNI status line + icon + the one-shot notify are the user-facing surfaces; live three-state
    observation is via the AGENTS.md Linux dev loop (needs hardware + an SNI host). The deterministic
    proof is the parity test (strings) + the S1 resolver tests (derivation).
```

## Validation Loop

> All commands run from the repo root: `/home/dustin/projects/qmkonnect`. The
> `src/linux_tray.rs` module is compiled on Linux with the `linux-tray` feature
> (default-on); its tests run in Linux CI.

### Level 1: Syntax & Style

```bash
cd /home/dustin/projects/qmkonnect

cargo fmt
cargo build 2>&1 | tee /tmp/build.log
# Expected: "Finished" — zero warnings. If "no field device_connected" or "mismatched types",
# a site was missed — grep -n 'device_connected' src/linux_tray.rs should be EMPTY.

cargo clippy --all-targets -- -D warnings 2>&1 | tee /tmp/clippy.log | grep -iE 'warning|error' || echo "clippy clean"
# Expected: "clippy clean". -D warnings treats any warning as an error (the contract requires it).

cargo fmt --check
# Expected: exit 0.
```

### Level 2: Unit Tests (the deterministic gate)

```bash
cd /home/dustin/projects/qmkonnect

# The updated parity test (runs on Linux CI):
cargo test --bin qmkonnect status_text_uses_parity_glyphs -- --test-threads=1 --nocapture
# Expected: 1 passed (asserts the 3 strings from device_status_text: \u{25CF}/\u{26A0}/\u{25CB}).

# The updated initial-state test:
cargo test --bin qmkonnect new_tray_probes_initial_state -- --test-threads=1 --nocapture
# Expected: 1 passed (reads tray.device_status, not tray.device_connected).

# Full suite — single-threaded (AGENTS.md: shared global debouncer/mock state):
cargo test --bin qmkonnect -- --test-threads=1 2>&1 | tail -3
# Expected: "test result: ok. <N> passed; 0 failed; ..." (N includes the 2 updated linux_tray tests).
```

### Level 3: Integration (live three-state + one-shot notify — needs hardware + SNI host)

```text
NOT a CI gate (requires real HID hardware + an SNI-hosting bar like Waybar). Verified via the
AGENTS.md Linux dev loop:
  cargo test --bin qmkonnect -- --test-threads=1
  cargo build --release
  # run ./target/release/qmkonnect in your Hyprland session with an SNI bar
Then observe across:
  - No board               -> "○  No Device Connected", icon dimmed
  - Plug vanilla QMK board -> "⚠  QMK board found — no qmk_notifier module (flash it)", icon FULL
                               alpha, AND exactly ONE desktop notification fires (not on every tick)
  - Plug a SECOND time after unplugging/replugging vanilla board -> the notify fires AGAIN
                               (the re-arm on leaving NoModule worked)
  - Flash qmk_notifier      -> flips to "●  Device Connected", icon full alpha, within ~1s
The deterministic proof of the TEXT is the Level-2 parity test; live UX confirms the poll-thread
wiring, the icon-alpha rule, and the one-shot guard.
```

### Level 4: Scope-preservation grep (prove the handshake + notifier/tray untouched)

```bash
cd /home/dustin/projects/qmkonnect

# (a) No stale device_connected references remain in linux_tray.rs.
grep -n 'device_connected' src/linux_tray.rs && echo "BUG: stale field ref" || echo "ok: field fully renamed"

# (b) The handshake block is byte-unchanged (still keyed on the bool).
grep -nA3 'handshake_action(last_device, connected)' src/linux_tray.rs
# Expected: the Gain/Loss/None arms still reference `connected` (bool), unchanged.

# (c) The one-shot guard exists + uses the swap/store idiom.
grep -n 'NO_MODULE_NOTIFIED' src/linux_tray.rs
# Expected: the static decl + a swap(true) (fire) + a store(false) (re-arm).

# (d) src/core/notifier.rs and src/tray.rs untouched.
git status --short src/core/notifier.rs src/tray.rs
# Expected: empty.

# (e) device_status_text parity with tray.rs (byte-identical strings).
grep -c '\u{26A0}' src/linux_tray.rs   # expected: >= 1 (the NoModule glyph in device_status_text)
```

## Final Validation Checklist

### Technical Validation
- [ ] Level 1: `cargo build` zero warnings; `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --check` exit 0.
- [ ] Level 2: `cargo test --bin qmkonnect -- --test-threads=1` green; the 2 updated linux_tray tests pass.
- [ ] Level 4 (a): no `device_connected` refs in linux_tray.rs.
- [ ] Level 4 (b): handshake block byte-unchanged (still keyed on `connected: bool`).
- [ ] Level 4 (c): `NO_MODULE_NOTIFIED` static + swap/store idiom present.
- [ ] Level 4 (d): `src/core/notifier.rs` + `src/tray.rs` unmodified.

### Feature Validation
- [ ] `QmkTray.device_status: DeviceStatus`; doc cites §3 + one-shot rationale (A).
- [ ] `device_status_text(DeviceStatus)` has the three exact parity strings (C).
- [ ] `menu()` uses `device_status_text`; hidden toggle is `!= Disconnected` (D).
- [ ] `icon_pixmap()` full-alpha for `!= Disconnected`; tooltip three-state (E).
- [ ] Poll thread: handshake unchanged on bool; `last_status` tracker drives the UI field; one-shot notify on Disconnected→NoModule (F).
- [ ] `NO_MODULE_NOTIFIED` fires once per NoModule entry, re-arms on exit.
- [ ] Tests updated (parity glyphs x3; `device_status` field read).

### Code Quality Validation
- [ ] Module-level `use DeviceStatus` (safe — module cfg-gated); glyphs as `\u{}` escapes; spacing/em-dash exact.
- [ ] The once-guard uses `AtomicBool` + `Ordering::SeqCst` (matches `RULES_INVALID_NOTIFIED`).
- [ ] notify-rust NOT introduced (uses the existing `notify-send` helper).
- [ ] Tests single-threaded (`--test-threads=1`).

### Documentation & Deployment
- [ ] Mode A: QmkTray doc-comment cites `spec/DEVICE_DISCOVERY.md` §3 + one-shot rationale; `device_status_text` cites UI.md §4.
- [ ] No user-facing doc file changed here (P4 handles docs/*.md).
- [ ] No environment variables, config, or Cargo.toml changes.

---

## Anti-Patterns to Avoid

- ❌ Don't gate the handshake on `DeviceStatus` — it MUST stay keyed on `is_device_connected()` (bool). Keep `last_device` + the handshake block byte-unchanged.
- ❌ Don't key the tray update on `last_device` (bool) — the NoModule→Connected flip keeps the bool `true`; key it on `last_status`.
- ❌ Don't dim the icon for NoModule — NoModule means a board IS present; full alpha. Only Disconnected dims (`!= Disconnected` ⇒ full).
- ❌ Don't make the hidden structural toggle three-way — it must stay present-vs-absent (item-count change for `LayoutUpdated`); NoModule is "present".
- ❌ Don't fire the notify on every NoModule tick or on Connected→NoModule — only on **Disconnected→NoModule**, guarded by `NO_MODULE_NOTIFIED.swap(true)`, re-armed on exit.
- ❌ Don't forget to re-arm (`store(false)` when leaving NoModule) — without it, a later replug-into-NoModule wouldn't notify.
- ❌ Don't use a plain `bool` for the once-guard — mirror `RULES_INVALID_NOTIFIED`'s `AtomicBool` + `Ordering::SeqCst`.
- ❌ Don't switch to `notify-rust` — it panics in ksni's handler thread (spec §7.3). Use the existing `notify(summary, body)` `notify-send` helper.
- ❌ Don't leave the status text as an inlined `if/else` — extract `device_status_text` so the parity test asserts on real output + matches tray.rs byte-for-byte.
- ❌ Don't paste raw emoji glyphs — use `\u{25CF}`/`\u{26A0}`/`\u{25CB}` escapes (file style). Don't use a hyphen for the em-dash (`\u{2014}`).
- ❌ Don't miss a `device_connected` site — grep must be EMPTY after the rename (all 11 sites: field, new, tool_tip, icon_pixmap, menu status, menu toggle, poll update, test, + comments).
- ❌ Don't add the "● N Devices Connected" pluralization — that's P3 (`classify_devices`); use singular.
- ❌ Don't add HID I/O / probing — `device_status()` is a pure read (S1 contract).
- ❌ Don't touch `src/core/notifier.rs` (S1) or `src/tray.rs` (S2).
- ❌ Don't run tests without `--test-threads=1` (AGENTS.md: shared global debouncer/mock state).

---

**Confidence Score: 9/10** for one-pass implementation success. The deliverable is a
single-file, multi-site edit consuming a stable S1 contract (`DeviceStatus` +
`device_status()`), with all 11 `device_connected` sites enumerated with before→after,
the verbatim poll-thread rewrite (dual-tracker + one-shot guard), the icon-alpha rule
(full for Connected+NoModule), the byte-identical parity strings (pinned by the test),
and the `RULES_INVALID_NOTIFIED` once-guard idiom quoted. The three design subtleties —
(a) handshake stays bool-keyed while the UI tracks DeviceStatus separately, (b) the
update condition keys on `last_status` not `last_device`, (c) the one-shot guard fires
only on Disconnected→NoModule and re-arms on exit — are each called out with rationale.
The one residual risk (a missed `device_connected` site or a parity-string typo) is
pinned by the Level-4 grep (zero `device_connected` refs) and the Level-2 parity test.
Live three-state + one-shot UX is verified via the AGENTS.md Linux dev loop, not CI.
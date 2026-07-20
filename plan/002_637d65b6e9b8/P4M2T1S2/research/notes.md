# Research Notes — P4.M2.T1.S2 (Integrate handshake into startup + device-status poll)

> Companion to `../PRP.md`. All line numbers verified against the current tree
> (`/home/dustin/projects/qmkonnect`, HEAD 2025-07-20). S1 (`perform_handshake`,
> `reset_handshake_state`, `host_capable`, `HAS_HANDSHAKED`, …) is being
> implemented in parallel and is treated as a **completed contract** — see
> `../../P4M2T1S1/PRP.md`. **As of this writing S1's symbols are NOT yet in
> notifier.rs** (`grep perform_handshake src/core/notifier.rs` ⇒ 0) — that is
> expected; this task appends AFTER them once S1 lands.

---

## §0 — Exact current anchors (verbatim)

### 0.1 Runners — the `startup_device_probe` call sites (4 total)

Each runner calls `crate::core::notifier::startup_device_probe(self.verbose)`
exactly once per entry path. `self.verbose: bool` is in scope at every site.

| File | Line | Context | Next call after it |
|---|---|---|---|
| `src/runners/windows.rs` | **48** | `run_console_mode(&self)` | `monitor.start()` then Ctrl-C loop |
| `src/runners/windows.rs` | **95** | `run_tray_app(&self)` | `monitor.start()` then `tray::setup_tray()` (L108) |
| `src/runners/macos.rs` | **27** | `fn run(&mut self,..)` | `monitor_thread` spawn (L38) then `crate::tray::setup_tray()` (L44) |
| `src/runners/linux.rs` | **27** | `fn run(&mut self,..)` | `ctrlc::set_handler` then `linux_tray::spawn()` (L46 / L60) |

**Insertion shape** (identical at all 4 sites, immediately AFTER the probe line):
```rust
crate::core::notifier::startup_device_probe(self.verbose);
// NEW: if a device is already connected, run the capability handshake now.
// (Poll-thread reconnects are handled in tray.rs / linux_tray.rs.)
if crate::core::notifier::is_device_connected() {
    crate::core::notifier::perform_handshake(self.verbose);
}
```

### 0.2 tray.rs — the macOS/Windows device-status poll thread (L368–385)

`UserEvent` enum (L36–46), `DeviceStatus(bool)` variant **L41**, cfg-gated
`#[cfg(any(target_os="macos", target_os="windows"))]`.

`setup_tray()` signature **L250**: `pub fn setup_tray() {` — **takes NO args**.

The poll thread (verbatim, L368–385):
```rust
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let status_proxy = proxy.clone();
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
    }
```
- `proxy` is `tao::event_loop::EventLoopProxy<UserEvent>` (`event_loop.create_proxy()` ~L274).
- The event-loop arm handling `DeviceStatus(connected)` sets
  `device_status_i.set_text(...)` — **UNCHANGED** by this task.
- Transition guard: `last: Option<bool>` init `None`; fires on **ANY** change
  (`None→x`, `false→true`, `true→false`). Needs directional split (see §3).

### 0.3 linux_tray.rs — the Linux SNI device-status poll thread (L248–272)

`spawn()` signature **L231**: `pub fn spawn() -> Option<ksni::blocking::Handle<QmkTray>> {`
— **takes NO args**.

`QmkTray` struct (**L67–70**): `device_connected: bool, dark_mode: bool` (both private; no channel fields).

Constants: `DEVICE_POLL_INTERVAL = Duration::from_secs(1)` (L57);
`COLOR_SCHEME_POLL_EVERY = 10` (L61).

The poll thread (verbatim, L248–272):
```rust
    let poll_handle = handle.clone();
    std::thread::spawn(move || {
        let mut last_device: Option<bool> = None;
        let mut last_dark: Option<bool> = None;
        let mut tick: u32 = 0;
        loop {
            let connected = crate::core::notifier::is_device_connected();
            let dark = if tick.is_multiple_of(COLOR_SCHEME_POLL_EVERY) {
                detect_dark_mode()
            } else {
                last_dark.unwrap_or(true)
            };
            tick = tick.wrapping_add(1);

            if last_device != Some(connected) || last_dark != Some(dark) {
                last_device = Some(connected);
                last_dark = Some(dark);
                let _ = poll_handle.update(|t: &mut QmkTray| {
                    t.device_connected = connected;
                    t.dark_mode = dark;
                });
            }
            std::thread::sleep(DEVICE_POLL_INTERVAL);
        }
    });
```
- Dispatch = `poll_handle.update(|t: &mut QmkTray| {…})` (NOT mpsc). The closure
  runs on **ksni's D-Bus thread** ⇒ HID I/O (`perform_handshake`) must run in the
  poll-loop BODY, OUTSIDE that closure.
- The guard at L262 conflates device + dark changes; directional device logic
  must be split out (see §3).

### 0.4 Call sites that must gain `(self.verbose)` (signature changes)

`grep -rn "::spawn()\|setup_tray(" src/`:
| Symbol | Call sites |
|---|---|
| `setup_tray` | `src/runners/windows.rs:108`, `src/runners/macos.rs:44`, `src/runners/linux.rs:70` |
| `linux_tray::spawn` | `src/runners/linux.rs:46`, `src/runners/linux.rs:60` |

All 5 are inside `fn run(&mut self,..)` / `fn run_tray_app(&self)` ⇒ `self.verbose`
is in scope. (`src/runners/mod.rs` and `src/main.rs` do NOT call these directly.)

---

## §1 — What S1 delivers (the contract this task consumes)

From `../../P4M2T1S1/PRP.md` (treating as COMPLETE):
- `pub fn perform_handshake(verbose: bool)` — idempotent per board boot via
  `HAS_HANDSHAKED.swap(true, SeqCst)` short-circuit; sends QueryInfo → (if capable)
  SetOs + QueryCallback sweep + rules validation; sets `HOST_CAPABLE`.
- `pub fn reset_handshake_state()` — clears `HOST_CAPABLE`, `CALLBACK_NAMES`,
  `HAS_HANDSHAKED` (re-arms the dedup so the next `perform_handshake` re-runs).
- `pub fn host_capable() -> bool`, `pub fn callback_names() -> HashMap<String,u8>`.
- 3 statics at the **L182–184 band** of `src/core/notifier.rs` (above `DebounceState`).

This task does NOT touch any of S1's code except to APPEND a small pure helper
(`handshake_action` + `HandshakeAction`) + 1 test, placed AFTER `reset_handshake_state`.
S1 is complete when S2 implements ⇒ **no merge conflict**.

### notifier.rs existing anchors this task reads/calls (unchanged)
- `pub fn startup_device_probe(verbose: bool)` — notifier.rs:119
- `pub fn is_device_connected() -> bool` — notifier.rs:169
- `fn get_notifier() -> Arc<Mutex<Box<dyn Notifier>>>` — notifier.rs:285
- `static NOTIFIER: Lazy<Arc<Mutex<Box<dyn Notifier>>>>` — notifier.rs:249

---

## §2 — The transition-semantics table (the whole novel logic)

| prev (`Option<bool>`) | now (`bool`) | Action | Why |
|---|---|---|---|
| `None` | `true` | **Gain** | device already connected at startup (poll's first tick) |
| `None` | `false` | None | nothing connected, never was |
| `Some(false)` | `true` | **Gain** | reconnect after a real loss |
| `Some(true)` | `true` | None | no change |
| `Some(false)` | `false` | None | no change |
| `Some(true)` | `false` | **Loss** | real disconnect → reset for next gain |

Encoded as a match (see §3). **Gain ⇒ `perform_handshake(verbose)`** (idempotent
— a no-op if the runner already handshooked at startup, so the poll's first tick on an
already-connected device is harmless). **Loss ⇒ `reset_handshake_state()`**. None ⇒ no-op.

---

## §3 — Design decision: a pure `handshake_action` helper in notifier.rs

**Why centralize** (vs inline `if/else` in 2 cfg-gated poll threads):
1. **Testability on the Linux dev box.** tray.rs's poll block is
   `#[cfg(any(target_os="macos",target_os="windows"))]` ⇒ NOT compiled on Linux.
   linux_tray.rs's block IS compiled (`default = ["hyprland","macos","linux-tray"]`,
   Cargo.toml L100). A pure helper in notifier.rs compiles + tests on ALL platforms,
   giving a deterministic validation gate for the only novel logic in this task.
2. **DRY.** Identical directional logic in ≥3 call sites (2 poll threads + the
   startup-`None` semantics). One source of truth.
3. **Repo precedent.** `linux_tray.rs:894` already tests pure helpers
   (`status_text_uses_parity_glyphs`, `parse_id…`) — "testable core, thin shell" style.

```rust
/// What the host-rules handshake lifecycle should do for a device-status transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeAction {
    None,  // no transition, or None→false at startup
    Gain,  // device became present  ⇒ perform_handshake (idempotent via HAS_HANDSHAKED)
    Loss,  // device went away       ⇒ reset_handshake_state (re-arm for next gain)
}

/// Classify a device-status transition into a handshake lifecycle action.
pub fn handshake_action(prev: Option<bool>, now: bool) -> HandshakeAction {
    match (prev, now) {
        (Some(true), false) => HandshakeAction::Loss,           // real disconnect
        (p, true) if p != Some(true) => HandshakeAction::Gain,  // None→true OR false→true
        _ => HandshakeAction::None,                              // no change OR None→false
    }
}
```

**⚠️ SUBTLE BUG TO AVOID:** the naive `(_, true) => Gain` mis-classifies
`(Some(true), true)` (no change, should be None) as Gain, because `(_, true)`
matches it. The Gain arm MUST be **guarded** (`(p, true) if p != Some(true)`).
The test (§4) pins this.

---

## §4 — The test (notifier.rs `mod tests`, appended after S1's tests)

```rust
#[test]
fn test_handshake_action_transitions() {
    assert_eq!(handshake_action(None, true), HandshakeAction::Gain);       // startup connected
    assert_eq!(handshake_action(None, false), HandshakeAction::None);      // startup disconnected
    assert_eq!(handshake_action(Some(false), true), HandshakeAction::Gain);// reconnect
    assert_eq!(handshake_action(Some(true), false), HandshakeAction::Loss);// real disconnect
    assert_eq!(handshake_action(Some(true), true), HandshakeAction::None); // no change (connected)
    assert_eq!(handshake_action(Some(false), false), HandshakeAction::None);// no change (disconnected)
}
```
Runs on Linux dev box (notifier compiles unconditionally). Single-threaded
(`--test-threads=1`, shared globals from S1).

---

## §5 — Gotchas (G1–G10)

- **G1 (verbose plumbing):** `setup_tray()` (tray.rs:250) and `spawn()`
  (linux_tray.rs:231) currently take NO args. MUST add `verbose: bool` to BOTH
  AND update all 5 call sites (windows.rs:108, macos.rs:44, linux.rs:46/60/70).
  A missed call site ⇒ compile error E0061 (fails loud — good).
- **G2 (handshake on the POLL thread, not UI/D-Bus thread):** `perform_handshake`
  does HID I/O (opens device, sends reports). In tray.rs it runs in the
  `std::thread::spawn` poll closure (before `send_event`) — fine, the tao event
  loop is not blocked. In linux_tray.rs it MUST run in the poll-loop BODY, NOT
  inside `poll_handle.update(|t: &mut QmkTray| {…})` (that closure runs on ksni's
  D-Bus thread — blocking it wedges the tray icon per AGENTS.md).
- **G3 (startup+first-poll idempotency):** runner handshakes at startup BEFORE
  `setup_tray()`/`spawn()` starts the poll thread (see §6). The poll's first tick
  sees `last=None, connected=true` ⇒ Gain ⇒ `perform_handshake` again — but
  `HAS_HANDSHAKED` (set at startup) makes it a no-op. **No double-handshake.**
- **G4 (reset only on real Loss):** `handshake_action(Some(true),false)=Loss`.
  `None→false` and `Some(false)→false` ⇒ None (no spurious reset that would clear
  `HOST_CAPABLE` mid-session). Pinned by the test.
- **G5 (bool is Copy):** `verbose` used in the poll closure AND (tray.rs) elsewhere
  ⇒ no move/borrow issue. Just reference it.
- **G6 (Linux default features):** `default = ["hyprland","macos","linux-tray"]`
  (Cargo.toml:100) ⇒ a plain `cargo build`/`cargo test` on Linux compiles +
  tests `linux_tray.rs`. The macos/windows blocks in tray.rs are cfg'd OUT on
  Linux ⇒ **not compiled natively** (validate by symmetry + the macOS/Windows
  dev loops in AGENTS.md).
- **G7 (preserve cfg gates):** the tray.rs handshake code stays inside the
  existing `#[cfg(any(target_os="macos",target_os="windows"))]` block. The
  linux_tray.rs code stays inside `spawn()` (which is itself under
  `#[cfg(feature="linux-tray")]` via the module). Don't add cross-platform calls
  that would compile on the wrong OS.
- **G8 (linux_tray reassign ordering):** keep `last_device = Some(connected)`
  INSIDE the existing combined `if last_device != Some(connected) || last_dark !=
  Some(dark)` guard (L262) so dark-only changes still update the tray unchanged.
  The directional handshake check reads `last_device` BEFORE that reassign.
- **G9 (no S1 conflict):** S1 is COMPLETE when S2 runs. The helper is appended
  AFTER `reset_handshake_state` in the L182–184 band. Additive; no overlap with
  S1's `perform_handshake`/statics/mock code.
- **G10 (ordering: handshake before poll thread exists):** every runner does
  `startup_device_probe` → (NEW handshake) → `setup_tray()`/`spawn()` (which
  spawns the poll thread). So the startup handshake finishes before the poll
  thread starts. Even if racy, `HAS_HANDSHAKED` (atomic) + the `Arc<Mutex<…>>`
  `NOTIFIER` guard make it safe.

---

## §6 — Start ordering proof (why startup handshake + poll first-tick is safe)

windows.rs `run_tray_app`: L95 probe → **handshake** → `monitor.start()` →
L108 `tray::setup_tray()` (spawns poll thread ~L370). ⇒ poll thread starts AFTER
startup handshake completes (same thread, sequential).

macos.rs `run`: L27 probe → **handshake** → L38 monitor_thread spawn → L44
`crate::tray::setup_tray()` (poll thread). Same.

linux.rs `run`: L27 probe → **handshake** → ctrlc handler → L46/L60
`crate::linux_tray::spawn()` (poll thread). Same.

⇒ At poll-thread first tick, `HAS_HANDSHAKED` is already `true` if the device was
connected at startup ⇒ the poll's `Gain`→`perform_handshake` is a dedup no-op. The
poll's real job is catching **post-startup** connect/disconnect.

---

## §7 — Validation commands (verified against this Linux box)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect                                     # Linux default features ⇒ compiles linux_tray.rs
cargo test --bin qmkonnect -- --test-threads=1                 # +new test, S1's 8, all existing green
cargo test --bin qmkonnect handshake_action -- --test-threads=1 # the new directional test in isolation
git diff --stat                                                 # notifier.rs + 3 runners + tray.rs + linux_tray.rs
```
macos/windows cfg blocks: validated by structural symmetry with the Linux path +
the platform dev loops in AGENTS.md (can't `cargo build` them on Linux).
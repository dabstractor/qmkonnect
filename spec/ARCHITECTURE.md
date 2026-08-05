# SPEC — Architecture & System Design

> Companion to `PRD.md`. Defines the software architecture, repository layout,
> module responsibilities, the end-to-end data flow, the concurrency/threading
> model, the trait design, and the error model. Read alongside the source tree.

---

## 1. Repository Layout

```
qmkonnect/
├── Cargo.toml                 # deps + features; pinned qmk-notifier v0.3.0 (git tag)
├── Cargo.lock
├── .cargo/config.toml         # windows-msvc: +crt-static (no VC++ Redist)
├── release.toml               # cargo-release: tag v<x.y.z>, push (no crates.io publish)
├── rust-toolchain             # (optional) pin; MSRV 1.88 via Cargo.toml rust-version
├── src/
│   ├── main.rs                # CLI dispatch + run() entry + init_logging
│   ├── core/
│   │   ├── mod.rs             # Config struct, parse_config, render_config_body, timing
│   │   ├── notifier.rs        # Notifier trait, QmkNotifier, debouncer, device filter, probes
│   │   └── types.rs           # WindowInfo { app_class, title }
│   ├── platforms/
│   │   ├── mod.rs             # WindowMonitor trait + dispatchers (config paths, list windows)
│   │   ├── windows.rs         # WinEventHook + polling fallback + window enumeration
│   │   ├── macos.rs           # NSWorkspace observer + CGWindowList + enumeration
│   │   ├── hyprland.rs        # EventListener IPC + reconnect + poll burst + enumeration
│   │   ├── linux.rs           # udev rule render/repair/reload, config paths, root-aware resolve
│   │   └── x11.rs             # xprop-polling fallback monitor
│   ├── runners/
│   │   ├── mod.rs             # PlatformRunner trait + create_runner
│   │   ├── windows.rs         # single-instance mutex + tray-app/console modes
│   │   ├── macos.rs           # monitor thread + tray loop
│   │   └── linux.rs           # monitor + (optional) SNI tray lifecycle
│   ├── tray.rs                # macOS/Windows tray (tao+tray-icon+muda), dialogs, autostart(macOS)
│   ├── linux_tray.rs          # SNI tray (ksni) + GTK window-info dialog + zenity settings
│   ├── autostart.rs           # Windows HKCU Run autostart
│   └── bin/
│       └── hid_id.rs          # standalone udev helper: parse report descriptor → ID_QMKONNECT=1
├── packaging/                 # platform installers + icons (see SPEC_PACKAGING.md)
└── docs/                      # Jekyll site (installation, configuration, qmk-integration, …)
```

**Two binaries** are produced from one crate (`Cargo.toml`):
- `qmkonnect` (`src/main.rs`) — the app.
- `qmkonnect-hid-id` (`src/bin/hid_id.rs`) — pure-`std` udev helper (builds on
  every target; only used on Linux in practice).

---

## 2. Module Responsibilities

### 2.1 `core/` — platform-independent core

- **`types::WindowInfo`** — `{ app_class: String, title: String }`. The single
  data type every platform monitor produces.
- **`core::Config`** — the deserialized TOML config; all device-ID fields are
  `Option<u16>` (`None` = auto-discovery). See `SPEC_CONFIG.md`.
- **`core::parse_config` / `render_config_body` / `create_default_config`** —
  read/write the config file; `render_config_body` is the **single shared
  renderer** every write path (CLI, Win32 dialog, NSAlert, zenity, GTK) uses so
  the file format never drifts.
- **`core::configured_timing()` / `configured_debounce_ms()`** — re-read
  `debounce_ms`/`poll_interval_ms` from config each call (hot config).
- **`core::notifier`** — the notification pipeline (§5 below): the `Notifier`
  trait, `QmkNotifier`, `DeviceFilter`, `configured_filter()`,
  `is_device_connected()`, `startup_device_probe()`, `list_devices()`,
  `notify_qmk()`, and the debounce worker.

### 2.2 `platforms/` — window detection (per-OS)

Each platform implements `WindowMonitor` (trait in `mod.rs`) and a set of free
functions dispatched from `mod.rs`: `get_config_paths()`, `create_config_dir()`,
`list_foreground_windows()`. See `SPEC_PLATFORMS.md`.

### 2.3 `runners/` — process lifecycle (per-OS)

Each platform implements `PlatformRunner`. A runner wires together: singleton
guard (Windows), signal handling (`ctrlc`), the startup device probe, starting
the monitor, and driving (or parking for) the tray event loop. See §7.

### 2.4 `tray.rs` — macOS + Windows UI

Compiled for **`cfg(not(all(target_os="linux", feature="hyprland")))`** — i.e.
the `tray-icon`/`tao` path is active on macOS, Windows, and the non-Hyprland
Linux build. Contains the tray setup, menu, Settings dialogs (Win32 + NSAlert),
the "Show Window Information" dialogs (Win32 + NSWindow), the device-status
polling thread, and the macOS `autostart` submodule (SMAppService). See
`SPEC_UI.md`.

### 2.5 `linux_tray.rs` — Linux SNI tray (feature `linux-tray`)

StatusNotifierItem over D-Bus via `ksni` (own thread), plus a native GTK
window-info popup and zenity-based settings. See `SPEC_LINUX.md` §6 and
`SPEC_UI.md`.

### 2.6 `autostart.rs` — Windows autostart (HKCU `Run`)

Self-contained `#[cfg(target_os="windows")]` module. See `SPEC_UI.md` §4.

### 2.7 `bin/hid_id.rs` — udev helper

Pure-`std`; parses a hidraw interface's HID report descriptor and prints
`ID_QMKONNECT=1` when it carries the QMK Raw HID signature. See
`SPEC_LINUX.md` §3.

---

## 3. The Platform Divergence Problem (and how the code resolves it)

The single hardest architectural constraint is that **each OS disagrees about
who owns the main thread**:

| OS | Main-thread owner | Consequence |
|---|---|---|
| **Windows** | The Win32 message loop (pumped by the `tao` event loop) — `WINEVENT_OUTOFCONTEXT` hooks are delivered there | Tray loop *is* the hook pump; a 100 ms polling thread is a belt-and-suspenders fallback |
| **macOS** | The Core Foundation run loop (`CFRunLoopRun`) **and** the `tao` event loop both want main | Monitor runs on a **background thread** (`CFRunLoopRun` blocks there); tray/`tao` owns main |
| **Hyprland** | A blocking Unix-socket IPC listener — no GUI loop at all | Monitor's `start()` blocks the calling thread; tray (ksni) runs on its own D-Bus thread |
| **X11** | No GUI loop needed | Monitor polls in a background thread; tray (`tray.rs`, non-SNI) or a park loop owns main |

The codebase resolves this with:
1. A single `Send` `WindowMonitor` trait (the former non-`Send` variant existed
   only because Hyprland's blocking `start()` stored the listener; it no longer
   does).
2. Three thin per-OS runners (`runners/{windows,macos,linux}.rs`) that each
   pick the right thread for the monitor vs. the tray.
3. The **debouncer in core** decouples *when a window change is observed* from
   *when it's sent* (§5), so thread boundaries never affect protocol timing.

> **Roadmap (not required to ship):** make every monitor non-blocking and
> event-pushing (`start()` spawns the listener, pushes `WindowInfo` into a
> channel), collapse to one generic host loop, and delete the three runners. The
> macOS/Windows GUI loop must stay on main; everything else is incidental. See
> `REMAINING_ISSUES.md` §"Architecture unification".

---

## 4. End-to-End Data Flow (detailed)

```
 [foreground window changes]
        │
        ▼  platform monitor (src/platforms/*)
 WindowInfo { app_class, title }
        │
        ▼  notifier::notify_qmk(&wi, verbose)        ── src/core/notifier.rs
 format!("{app_class}\x1D{title}")                    (GS = 0x1D)
        │
        ▼  DebounceState (Mutex + Condvar, single worker thread)
   ┌────┴───────────────────────────────────┐
   │ due now?  ──yes──►  QmkNotifier.notify  │
   │   no  ──►  pending = msg; COND.notify   │   worker waits out remainder of
   └─────────────────────────────────────────┘   window measured from last *send*
        │
        ▼  QmkNotifier::notify(msg)
 configured_filter()  ──► DeviceFilter { vid?, pid?, usage_page, usage }
 qmk_notifier::RunParameters::new(SendMessage(msg), vid, pid, page, usage, false)
 qmk_notifier::run(params)
        │
        ▼  qmk-notifier crate (src/core.rs)
 append ETX (0x03)
 frame into 32-byte reports: [0x00, 0x81, 0x9F, <30 payload>]  (33-byte hidapi buffer)
 open_matching_devices (usage/page + optional vid/pid)  [cached]
 burst_to_one: write all reports back-to-back, drain IN acks
        │
        ▼  USB Raw HID  (usage page 0xFF60 / usage 0x61)
        │
        ▼  keyboard firmware: notifier.c
 validate 0x81 0x9F → strip → append to 256-byte buffer until ETX
 sanitize_string (ASCII only) → process_full_message()
 match command_map / layer_map → enable_command / activate_layer
```

The debounce and retry semantics are in §5; the byte-level protocol is in
`SPEC_PROTOCOL.md`; the firmware side is in `SPEC_FIRMWARE.md`.

---

## 5. The Notification Pipeline & Debouncer (`src/core/notifier.rs`)

### 5.1 The `Notifier` trait

```rust
pub trait Notifier: Send + Sync {
    fn notify(&self, message: String) -> Result<(), Box<dyn Error + Send + Sync>>;
}
```

`QmkNotifier` is the real impl (uses `qmk_notifier::run`). Tests swap in a
`MockNotifier` via `set_notifier()` (global `Lazy<Arc<Mutex<Box<dyn Notifier>>>>`).

### 5.2 Device filter resolution (`configured_filter`)

Re-reads `config.toml` **on every call** (hot config). Builds:

```rust
pub struct DeviceFilter {
    pub vendor_id: Option<u16>,   // None = match any (auto-discovery)
    pub product_id: Option<u16>,  // None = match any
    pub usage_page: u16,          // default 0xFF60
    pub usage: u16,               // default 0x61
}
```

Used by `is_device_connected()`, `startup_device_probe()`, and
`QmkNotifier::notify()`. The match predicate is the **same** in all three:
`usage_page == && usage == && vid.is_none_or(==) && pid.is_none_or(==)`.

This is the **Tier-1** (presence) predicate only. A **Tier-2 capability** layer
sits on top of it for the status line and the write path: `classify_devices()`
(`DEVICE_DISCOVERY.md` §2) sends one `QUERY_INFO` per Tier-1 candidate and tags
it `Capable` or `NotQmkNotifier`. The **write match set** is Tier-1 **AND**
`kind == Capable`, so magic bursts go only to qmk_notifier boards (and, when
several are present, to all of them — broadcast, `DEVICE_DISCOVERY.md` §4). The
hot `configured_filter()` itself is unchanged; capability is a separate,
cached classification, not a per-notification cost.

### 5.3 Debounce design (the key correctness property)

State (process-global, behind `Lazy<Mutex<DebounceState>>` + `Lazy<Condvar>`):

```rust
struct DebounceState {
    last_sent_time: Option<Instant>,  // None until the first send
    pending: Option<String>,          // newest queued message
    verbose: bool,
    interval: Duration,               // from configured_debounce_ms(); 0 disables
}
```

**Algorithm (`notify_qmk` + `debounce_worker`):**
1. On each call: if `now - last_sent_time >= interval` (or never sent), **send
   immediately**, set `last_sent_time = now`, clear `pending`.
2. Otherwise: set `pending = message` (overwriting any older pending), signal
   the worker via `COND.notify_one()`.
3. The **single worker thread** (spawned once via `Lazy<JoinHandle>`,
   `ensure_worker()` touches it) waits on the condvar until `pending` is set,
   then waits out the *remainder of the window measured from `last_sent_time`*
   (not from when the message arrived), and flushes exactly the newest pending
   value.

**Why this matters:** because each new pending message does **not** reset
`last_sent_time`, a rapid burst (Alt-Tab spam) collapses to **exactly one
immediate send plus at most one follow-up** of the final value — never a flood,
never a lost final state. `debounce_ms = 0` disables coalescing (every change
sends immediately).

**Testing constraint:** the debouncer is global mutable state shared across
tests, so the suite **must** run single-threaded:
`cargo test --bin qmkonnect -- --test-threads=1`. Each test calls
`reset_test_state()` (flush, reset `last_sent_time`, reset the mock counter).

### 5.4 Send retry & graceful failure (`QmkNotifier::notify`)

- Up to **3 attempts** with linear backoff (100 ms, 200 ms).
- Retries **only** for device-class errors (`"no device found"`,
  `"permission denied"`, `"failed to open"`).
- After 3 device failures: **logs and returns `Ok(())`** — deliberately, so a
  transient unplug never restart-loops the service. (The trade-off: a typo'd
  VID is silent at runtime, which is why `startup_device_probe` exists — §5.5.)
- Non-device errors propagate immediately.

### 5.5 Startup probe (`startup_device_probe`)

Called once at startup by every runner. Read-only `hidapi` enumeration
(never opens the device). On a miss, prints a clear diagnostic naming the
configured filter and pointing at `--list-devices`. This is the answer to "a
typo'd VID fails silently at runtime" (#16).

### 5.6 Status probe (`is_device_connected` / `classify_devices`)

Read-only Tier-1 enumeration (`is_device_connected()`); `true` iff any
interface matches the filter. Backs the device-presence snapshot and the
broadcast decision. Runs on a background thread (3 s macOS/Windows,
1 s Linux) and only fires a UI update on a transition.

The tray status line is driven by **`classify_devices()`** (Tier-2, cache-backed
— `DEVICE_DISCOVERY.md` §2.3), producing a **three-state** value rather than a
boolean: **Connected** (≥1 capable board), **No module** (≥1 Tier-1 board, 0
capable — the truthful "flash qmk_notifier" state), **Disconnected** (0 Tier-1
boards). Classification is event-driven (runs once per device appearance, then
TTL-cached), so the frequent status poll stays cheap. See `DEVICE_DISCOVERY.md`
§3 for the full state machine and `UI.md` §4 for the rendered text/icons.

### 5.7 Host-side-rules extension

Host-side rules extend this pipeline; the full design is in
`HOST_RULES.md` and the wire contract is canonical in the firmware `PRD.md` §4.6.
In summary, after the debounced string send, QMKonnect additionally:
- runs a **capability handshake** at (re)connect (`QUERY_INFO`; gated on
  `proto_ver == 2`) + a `QUERY_CALLBACK` name sweep, and sends `SET_OS` once
  (the host is the OS source of truth while connected);
- evaluates `rules.toml` against the window and sends an `APPLY_HOST_CONTEXT`
  typed command (the `clear_board` flag selects per-window stack vs replace —
  see `HOST_RULES.md` §4); on no-match it clears the host layer + callbacks
  only — the board's own rules still run (host/board are independent silos, C13).
The debounce worker itself is unchanged — the host-context send happens within
the same debounced "send" step (one window change ⇒ ≤2 sends: string + context,
or context-only in replace mode). Retry/cache for the typed command match the
string path (§5.4). The host-side matcher is ported into `src/core/pattern.rs`
(full parity with the firmware matcher).

---

## 6. Concurrency Model (per component)

| Component | Thread | Sync primitive | Notes |
|---|---|---|---|
| Debouncer state | worker thread (1) | `Mutex<DebounceState>` + `Condvar` | `Lazy`-spawned, lives for process |
| `QmkNotifier` | caller of `notify_qmk` | `Arc<Mutex<Box<dyn Notifier>>>` | global `Lazy` |
| Windows monitor | hook delivered on message loop thread; 100 ms polling thread | `AtomicBool G_VERBOSE`, `AtomicIsize G_HOOK`, `Mutex<Option<(String,String)>> LAST_WINDOW_INFO` | replaced former `static mut` (UB) |
| macOS monitor | background thread running `CFRunLoopRun` | `AtomicBool VERBOSE` | tray/`tao` owns main |
| Hyprland monitor | calling thread blocks on `EventListener::start_listener`; optional `poll_interval_ms` poller thread; transient poll-burst threads | `Arc<Mutex<Option<WindowState>>>` | reconnect backoff is local to `start()` |
| Device-status poll | background thread | `EventLoopProxy<UserEvent>` (macOS/Win) / `handle.update()` (Linux ksni) | UI mutated only on main thread (muda `!Send`) |
| qmk-notifier device cache | caller | `LazyLock<Mutex<Option<DeviceCache>>>` | invalidated on any write error |

**Critical thread-safety invariants:**
- `muda::MenuItem` / `CheckMenuItem` are backed by `Rc<RefCell<…>>` → **`!Send`**.
  Mutate them **only on the event-loop thread**. Background threads deliver
  state via `tao::EventLoopProxy<UserEvent>` (macOS/Windows) or ksni's
  `handle.update(closure)` (Linux).
- `env::set_var` is unsound in a threaded context (Edition 2024 hard-errors);
  Hyprland's `check_hyprland_environment` sets `HYPRLAND_INSTANCE_SIGNATURE`
  **once, on the main thread, before any listener spawns**.
- `panic = "abort"` in release means `catch_unwind` supervisors are no-ops;
  crash recovery relies on systemd `Restart=always` (Linux) and the user
  relaunching (macOS/Windows tray apps).

---

## 7. Process Lifecycle (`runners/`)

All three runners share the same skeleton:
1. `create_monitor(verbose)` → `Box<dyn WindowMonitor>`.
2. Print startup banner; `startup_device_probe(verbose)` (clear miss diagnostic).
3. `ctrlc::set_handler` → `process::exit(0)` (immediate, no unwind).
4. Start the monitor (thread placement differs — §3).
5. Drive the tray (macOS/Windows) or park (Hyprland+SNI).

### 7.1 Windows (`runners/windows.rs`)
- `--tray-app` (default): `is_already_running()` (named mutex via
  `single-instance` crate, **leaked** to hold for process life) → start monitor
  → `tray::setup_tray()` (blocks).
- `--console`: `AllocConsole`, run monitor on the calling thread, block on a
  sleep loop (for debugging).
- `windows_subsystem = "windows"` (in `main.rs` attribute) → no console window.

### 7.2 macOS (`runners/macos.rs`)
- Monitor on a background thread (`CFRunLoopRun` blocks there).
- `tray::setup_tray()` on main (blocks until Quit).

### 7.3 Linux (`runners/linux.rs`)
- **Hyprland build:** optionally `linux_tray::spawn()` (ksni, own thread,
  handle kept alive), then `monitor.start()?` **blocks** the calling thread on
  the IPC listener.
- **Non-Hyprland (X11) build:** monitor on a background thread; if `linux-tray`
  is on, park the main thread (ksni owns its loop); otherwise drive
  `tray::setup_tray()`.

---

## 8. Error Model

- **Traits return `Result<(), Box<dyn std::error::Error>>`** (or
  `Box<dyn Error + Send + Sync>` for the notifier). No bespoke error enum in the
  app core today (the historical `core/errors.rs` + `core/validation.rs` were
  orphaned and removed).
- **`qmk-notifier` crate** has its own `QmkError` enum (`DeviceNotFound`,
  `DeviceOpenError`, `PartialSendError`, `SendReportError`, …).
- **Fail-loudly vs. fail-soft** is a deliberate, per-call-site choice:
  - *Fail loud*: `startup_device_probe` (typo'd VID), `resolve_config_for_reload`
    (root with no config — the heart of fixing #26), X11 monitor when `xprop` is
    missing, Hyprland monitor when the socket is absent at startup.
  - *Fail soft*: `QmkNotifier::notify` device errors (don't restart-loop),
    tray registration on Linux without D-Bus (run trayless), screen-recording
    permission missing on macOS (send app name only).
- **Logging**: Windows Event Log (source `"QMKonnect"`) by default, console when
  launched with `-v`; `eprintln!`/`println!` elsewhere. Verbose timestamps use a
  process-local monotonic epoch (`core::now_ms()`), not wall-clock.

---

## 9. Build Profile & MSRV

`Cargo.toml` `[profile.release]`:
```toml
opt-level = "z"      # optimize for size
lto = true
codegen-units = 1
panic = "abort"
strip = true
```
`.cargo/config.toml` (Windows MSVC only): `rustflags = ["-C", "target-feature=+crt-static"]`
→ statically links UCRT+vcruntime → **no VC++ Redistributable** dependency.

**MSRV Rust 1.88** (enforced via `rust-version`; image 0.25.x is the floor).

**Feature flags** (default = `["hyprland", "macos", "linux-tray"]`):
- `hyprland` — Hyprland IPC monitor (default-on Linux).
- `macos` — the Cocoa/CoreGraphics deps.
- `linux-tray` — ksni SNI + GTK window-info dialog (default-on Linux).

`--no-default-features` yields the minimal trayless service build. Features are
inert off-platform (e.g. `macos` on Linux), so plain `cargo build --release`
produces the full app with a tray on every OS.

---

## 10. Key Invariants a Dev Agent Must Preserve

1. **GS is `0x1D`; ETX is `0x03`.** The payload is `"{class}\x1D{title}"`; the
   crate appends ETX. Never change without coordinating both halves.
2. **First send immediate; bursts collapse to one follow-up.** Don't reset
   `last_sent_time` on each pending message.
3. **Device matching is usage-page/usage primary, VID/PID optional.** Never
   require VID/PID.
4. **Config is re-read every notification/poll.** Don't cache it in a long-lived
   struct.
5. **`render_config_body` is the single config-file writer.** All dialogs/CLI
   share it.
6. **The udev fallback rule is exactly one physical line starting with a match
   key.** A multi-line/assignment-only line re-permissions every device on the
   host (`SPEC_LINUX.md` §5).
7. **`MenuItem` is `!Send`** — mutate only on the event-loop thread.
8. **Tests are single-threaded** (shared global debouncer).
9. **Tier-2 capability before action.** A board is only written to / reported
   "Connected" if it answered the `0x81 0x9F` `QUERY_INFO` probe
   (`classify_devices`, `DEVICE_DISCOVERY.md` §2). Never treat a pure-`0xFF60`
   (e.g. VIA-only) board as a target.
10. **Shared open, always (R-COEX).** Every HID handle is opened shared /
    non-seize (`hidapi`'s default) and input reports are read only in bounded
    drains around a write — never a seize, never a perpetual blocking read. The
    always-on QMKonnect must never lock out the intermittently-used VIA app
    (`DEVICE_DISCOVERY.md` §6).

---

*Continue with `SPEC_PROTOCOL.md`.*

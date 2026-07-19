# PRD — QMKonnect

**Product Requirements Document & Master Specification**
Version: 0.2.4 · Status: Beta · Owner: Mulletware · License: MIT

> **Host-side window rules:** edit
> layer/callback rules in `rules.toml` with no reflash. Fully specified in the
> companion `HOST_RULES.md`; the **typed-command wire contract is owned by the
> firmware spec** ([`dabstractor/qmk-notifier` `PRD.md`
> §4.6](https://github.com/dabstractor/qmk-notifier/blob/main/PRD.md)), transported
> by the `qmk_notifier` crate ([`PRD.md`
> §10](https://github.com/dabstractor/qmk_notifier/blob/main/PRD.md)). See F11/F12
> below and the Document Map.

> This is the **master** document for the QMKonnect desktop application. It
> defines the product, its goals, its users, and its feature set, and it
> **hard-links** the detailed technical specifications that together are complete
> enough for a developer agent to reimplement the entire application from
> scratch. Read this first, then follow the links in [§14 Document Map](#14-document-map).

---

## 1. What QMKonnect Is

**QMKonnect** is a cross-platform desktop daemon that detects the foreground
window (its application class and title) and streams that information to a QMK
keyboard over USB Raw HID. The keyboard — running the companion
**[qmk-notifier](https://github.com/dabstractor/qmk-notifier)** firmware module
— pattern-matches the incoming string against user-defined rules and reacts by
switching layers and/or invoking callbacks. The result is a *context-aware
keyboard*: your keymap adapts automatically to the app you're using.

QMKonnect is the **desktop half** of a strictly two-part system. It only *sends*
window metadata; it does not decide behavior. All layer/command logic lives in
firmware. The two halves communicate over a tiny, well-defined wire protocol
(see `PROTOCOL.md`).

```
   ┌──────────────────────────┐      Raw HID (usage 0xFF60/0x61)     ┌─────────────────────────┐
   │  QMKonnect (desktop)     │  ─────────────────────────────────►  │  qmk-notifier (firmware)│
   │  Windows / macOS / Linux │   "{app_class}\x1D{title}\x03"       │  layer switch / callback│
   └──────────────────────────┘                                       └─────────────────────────┘
        ▲ watches foreground window                                          ▲ runs on the MCU
```

### 1.1 The broader ecosystem

QMKonnect is one node in a small ecosystem. A dev agent must understand all of:

| Project | Repo | Role |
|---|---|---|
| **QMKonnect** | `dabstractor/qmkonnect` (this) | Cross-platform desktop app: window detection + Raw HID send |
| **qmk-notifier** | `dabstractor/qmk-notifier` | QMK **firmware module** (C): receives, reassembles, pattern-matches, acts |
| **qmk_notifier** | `dabstractor/qmk_notifier` (note underscore) | Rust **library** the desktop app links for the Raw HID transport (device cache, burst-write, framing) |
| **qmk_firmware** | `qmk/qmk_firmware` | Upstream QMK; the keyboard's firmware that hosts both modules above |

> **Naming hazard (read once):** `qmk-notifier` (hyphen) is the firmware C
> module; `qmk_notifier` (underscore) is the Rust transport crate. QMKonnect
> depends on the latter (`qmk_notifier` v0.2.1, git tag). The user's keymap
> depends on the former. Both are required end-to-end.

---

## 2. Goals & Non-Goals

### 2.1 Goals

1. **Zero-config for a single standard QMK keyboard.** On every platform, a user
   with a default QMK keyboard running qmk-notifier needs to install QMKonnect
   and *nothing else* — no vendor/product IDs, no udev reload, no sudo. Device
   discovery is by the stable QMK Raw HID signature (usage page `0xFF60` /
   usage `0x61`).
2. **Cross-platform, native-feeling, and unobtrusive.** A menu-bar icon
   (macOS), system-tray icon (Windows), or StatusNotifierItem (Linux) with a
   minimal menu, a discoverable "Open at Login" toggle (default on), and a
   device-connection status line.
3. **Low latency, low resource use.** Immediate first-window notification,
   debounced bursts so rapid Alt-Tab spam collapses to one update, minimal CPU.
4. **Graceful degradation, never crashes.** Missing permission? Send app name
   only. Keyboard unplugged? Retry with backoff, never take down the service.
   Typo'd config? Probe once at startup and say so clearly.
5. **Per-user, no-admin install.** No elevation anywhere; per-user installer on
   Windows, user systemd service on Linux, app bundle on macOS.

### 2.2 Non-Goals (explicitly out of scope for the beta)

- Behavior/layer logic on the desktop side. QMKonnect is a pure sensor. *(To be
  relaxed by the Host-Side Window Rules feature — see `HOST_RULES.md` —
  where the host optionally matches rules and stacks a layer + callbacks on top
  of the board's.)*
- X11 support as a first-class target (a fallback `xprop`-polling monitor
  exists but Hyprland is the supported Linux surface).
- Other Wayland compositors (Sway, KDE-Wayland, GNOME, …) — Hyprland only.
- Code signing / notarization of distributed binaries (unsigned Windows,
  ad-hoc-signed macOS). The build scripts *support* a stable Developer ID.
- A cross-platform settings GUI toolkit. Each platform uses its native surface
  (Win32 / Cocoa / zenity+GTK).
- Multi-keyboard *management* (VID/PID disambiguation is supported; richer UX
  is future work).

---

## 3. Target Users & Personas

- **Power user / developer** with a custom QMK keyboard (e.g., a split like the
  Dactyl-Manuform) who wants context layers (vim mode in terminals, a numpad
  layer in Calculator, gaming layers for Steam games). This is the primary
  persona; the reference keymap in this PRD is exactly that user.
- **QMK hobbyist** who runs qmk-notifier and wants a turnkey desktop notifier
  without hand-editing config or writing a script.
- **Tinkerer on a non-Hyprland desktop** — partially served today; a clear
  roadmap exists (see §12 and `PLATFORMS.md`).

---

## 4. Top-Level Feature Set

| # | Feature | Where specified |
|---|---|---|
| F1 | Foreground-window detection (app class + title) per platform | `PLATFORMS.md` |
| F2 | Raw HID transport with burst-write, device cache, retry | `PROTOCOL.md` |
| F3 | Auto device discovery by usage page/usage (optional VID/PID) | `PROTOCOL.md` §3 |
| F4 | Debounced coalescing of rapid window changes (configurable) | `ARCHITECTURE.md` §6 |
| F5 | TOML config with zero-config defaults + CLI flags | `CONFIG.md` |
| F6 | Tray / menu-bar UI with settings, device status, window info | `UI.md` |
| F7 | "Open at Login" toggle, default on (HKCU Run / SMAppService / systemd) | `UI.md` §4 |
| F8 | Per-platform installer + CI release pipeline | `PACKAGING.md` |
| F9 | Linux: static udev rule + usage-page helper + root-aware reload | `LINUX.md` |
| F10 | Companion firmware module contract (`qmk-notifier`) | `FIRMWARE.md` |
| **F11** | **Host-side window rules:** edit `rules.toml` to map apps → layers/callbacks with **no reflash** (stacks on top of board rules) | `HOST_RULES.md` |
| **F12** | **Named callback registry** + typed Raw HID commands (`QUERY_INFO` / `QUERY_CALLBACK` / `APPLY_HOST_CONTEXT`) with a capability handshake | `HOST_RULES.md` |

---

## 5. Supported Platforms (Compatibility Matrix)

| Platform | Supported version | App model | Install | Autostart | Notes |
|---|---|---|---|---|---|
| **Windows** | 10 / 11, **x64 only** | Per-user tray app (`windows_subsystem="windows"`) | Inno Setup `.exe` (no admin) | HKCU `Run` (default on) | Static CRT link → no VC++ Redistributable |
| **macOS** | 13 Ventura+ (for SMAppService) | Menu-bar app bundle (`LSUIElement`) | `.app` in a `.dmg` | `SMAppService` (default on) | Screen Recording permission needed for titles |
| **Linux** | Hyprland (Wayland) | systemd user service + SNI tray | Arch `PKGBUILD` / binary | systemd `BindsTo` device | udev rule grants permissions; SNI bar required to *see* the icon |

Not supported: 32-bit Windows, Windows ≤ 8.1, X11 as a primary target, other
Wayland compositors. The Rust **MSRV is 1.88** (enforced by `rust-version`).

---

## 6. The End-to-End Data Flow (30-second tour)

1. **Platform monitor** detects a foreground-window focus change
   (`src/platforms/{windows,macos,hyprland,x11}.rs`) and produces a
   `WindowInfo { app_class, title }`.
2. **Notifier pipeline** formats the payload `"{app_class}\x1D{title}"`, passes
   it to the **debouncer** (`src/core/notifier.rs`).
3. **Debouncer**: first change after a quiet period is sent *immediately*;
   subsequent changes within the `debounce_ms` window (default 50 ms) are
   collapsed to exactly one follow-up send of the newest value.
4. **`QmkNotifier::notify`** builds `RunParameters` (resolving the device filter
   from config on every call) and calls `qmk_notifier::run`.
5. **`qmk_notifier` crate** appends the `ETX` (0x03) terminator, frames the
   payload into 32-byte Raw HID reports prefixed `0x81 0x9F`, burst-writes to
   every matching interface (cached), and drains acks.
6. **Keyboard firmware** (`notifier.c`) validates the `0x81 0x9F` header,
   strips it, reassembles into a 256-byte buffer until `ETX`, sanitizes to
   ASCII, and runs `process_full_message` → matches against `command_map` /
   `layer_map` (defined by the user's `DEFINE_SERIAL_COMMANDS` /
   `DEFINE_SERIAL_LAYERS`) → toggles the active layer / invokes callbacks.

The full byte-level protocol is in `PROTOCOL.md`; the debounce/concurrency
model is in `ARCHITECTURE.md`.

---

## 7. Key Product Behaviors (the "feel")

- **Window class is the stable identifier.** Windows uses the Win32 window
  *class* (`GetClassNameW`); macOS uses the app's `localizedName`; Hyprland uses
  `initial_class`; X11 uses `WM_CLASS`. These are the strings the user matches
  in firmware. The "Show Window Information" dialog (macOS/Windows/native GTK on
  Linux) exists *precisely* so users can discover the exact class/title strings
  to put in their `DEFINE_SERIAL_*` rules.
- **Empty workspaces report empty.** On Hyprland, an empty workspace sends an
  empty `app_class`+`title`, which deactivates any active notifier layer on the
  keyboard. (Windows/macOS don't generate focus events for "no window".)
- **Internal/shell windows are filtered** (e.g. `Shell_TrayWnd`,
  `ApplicationFrameWindow`, tray-overflow flyouts) so switching to the tray
  doesn't spam the keyboard. See `PLATFORMS.md` §1.4.
- **Device status is live in the tray.** A read-only `hidapi` enumeration
  (never opens the device) runs on a background thread and flips the menu's
  "● Device Connected / ○ No Device Connected" line within ~1–3 s of
  plug/unplug.
- **"Open at Login" defaults on, but never fights the user.** First run writes
  the autostart entry and a marker file; subsequent launches never re-enable it
  if the user turned it off (macOS) / the registry is the single source of truth
  (Windows).
- **Configuration is hot.** VID/PID/timing are re-read from `config.toml` on
  every notification and every status poll, so editing the file (or saving the
  Settings dialog) takes effect within ~3 s with no restart.

---

## 8. Configuration Summary (full detail: `CONFIG.md`)

A single TOML file, **all fields optional** (zero-config by default):

| Key | Default | Meaning |
|---|---|---|
| `vendor_id` | unset (`None` → match any) | USB VID; set only to disambiguate multiple QMK keyboards |
| `product_id` | unset (`None` → match any) | USB PID; set only to disambiguate |
| `usage_page` | `0xff60` | HID usage page; set only if firmware overrode `RAW_USAGE_PAGE` |
| `usage` | `0x61` | HID usage; set only if firmware overrode `RAW_USAGE_ID` |
| `debounce_ms` | `50` | Burst-coalescing window (ms); `0` disables debouncing |
| `poll_interval_ms` | `0` | (Hyprland only) active-window poll cadence; `0` = rely on IPC events |

**Paths** (historical naming preserved):
- Linux: `~/.config/qmk-notifier/config.toml`
- Windows: `%APPDATA%\QMKonnect\config.toml`
- macOS: `~/Library/Application Support/QMKonnect/config.toml`

**CLI**: `qmkonnect [-v] [-c] [-r [--config P --user U --uid N]] [-l] [--list-devices]
[--show-window-info] [--tray-app|--console] [-h]`

---

## 9. Security & Privacy

- **No telemetry, no network.** Window metadata is sent only to the locally
  attached keyboard over USB. Nothing leaves the machine.
- **Per-user install, no elevation** on every platform.
- **Minimal permissions**: Windows needs none for HID or foreground detection;
  macOS needs *Screen Recording* (not Accessibility) only for window **titles**
  (app name works without it); Linux needs hidraw access granted by the static
  udev rule (membership in `input` group, or the `uaccess` ACL).
- **No world-writable device nodes.** The udev rule uses `GROUP="input",
  MODE="0660"` plus `TAG+="uaccess"` (never `0666`), and the build actively
  detects/repairs a historically-dangerous multi-line rule form that corrupted
  host-wide device permissions (`LINUX.md` §5).

---

## 10. Non-Functional Requirements

**Performance.** Minimal CPU and memory footprint; fast startup and
initialization; immediate (debounced) detection of foreground-window changes.
Realized by: a single long-lived debounce worker thread, a cached set of opened
HID handles (re-enumeration only on key change or write failure), and a
size-optimized release profile (`opt-level="z"`, LTO, single codegen unit,
stripped). The default `debounce_ms = 50` collapses rapid Alt-Tab bursts to one
immediate send plus at most one follow-up.

**Reliability & graceful degradation.** Never crash on a missing permission, an
unplugged keyboard, or a malformed event. Concretely: missing macOS Screen
Recording ⇒ send app name only and keep running; keyboard gone ⇒ retry with
backoff then return `Ok` (never restart-loop); typo'd config ⇒ a one-time
startup probe prints a clear diagnostic. Linux relies on systemd `Restart=always`
for crash recovery; macOS/Windows tray apps are relaunched by the user or at
login. Release builds use `panic = "abort"` (no unwind scaffolding).

**Logging & diagnostics (platform-appropriate).**
- **Windows:** Windows Event Log, source `"QMKonnect"`, by default; prints to the
  console when launched from a terminal with `-v`.
- **Linux:** stderr, captured by systemd to the journal
  (`journalctl --user -u qmkonnect`); `-v` for verbose.
- **macOS:** stderr/console; `-v` for verbose.
- Verbose timestamps use a process-local monotonic epoch (`core::now_ms()`), not
  wall-clock, to avoid system-clock skew.

**User feedback channels.** The tray/menu-bar device-status line
("● Device Connected" / "○ No Device Connected"), verbose logs, and the
startup device-probe diagnostic. (No modal error dialogs on the hot path —
failures degrade silently and recover automatically.)

---

## 11. Success Criteria (how "done" is judged)

1. A fresh install on any supported platform, with a default QMK keyboard +
   qmk-notifier firmware, **switches keyboard layers when the user changes app**
   with zero configuration.
2. Unplugging the keyboard shows "○ No Device Connected" in the tray within a
   few seconds; replugging restores "● Device Connected" and notifications
   resume — no restart, no crash.
3. The "Open at Login" toggle accurately reflects and controls the real
   autostart entry on all three platforms.
4. `cargo test --bin qmkonnect -- --test-threads=1` passes (the debouncer has
   shared global state; tests must be serial).
5. Per-platform clean build + install + launch loop works (see `AGENTS.md` and
   `PACKAGING.md` §6).

---

## 12. Beta Status & Future Work

- **Host-side window rules.** Edit layer/callback rules in `rules.toml` on the host — no reflash — stacking
  on top of the board's `DEFINE_*` rules. Fully specified in `HOST_RULES.md`; it
  spans the `qmk_notifier` crate, the `qmk-notifier` firmware, and this app.
- **Linux surface is narrow** (Hyprland-only). Broader Wayland + X11 is planned.
- **Binaries are unsigned** (Windows) / ad-hoc signed, not notarized (macOS).
  This causes the macOS Screen-Recording re-prompt loop on every rebuild; a
  stable Developer ID + notarization is the intended fix.
- **Settings UX** is native-per-platform today (Win32 / NSAlert / zenity+GTK);
  a richer cross-platform UI is future work.
- **Architecture unification**: three near-duplicate runners and a dual-trait
  monitor design exist because each OS disagrees on who owns the main thread.
  The roadmap (see `REMAINING_ISSUES.md` §"Architecture unification") is to make
  every monitor non-blocking/event-pushing and own one host loop. Not required
  to ship, but the cleanest end state.
- **Multi-keyboard management** beyond VID/PID disambiguation.
- **A device-arrival "launcher" on Windows** (the true udev analog) — separate,
  larger design; today autostart-at-login covers the use case.

---

## 13. Glossary

| Term | Meaning |
|---|---|
| **GS** | Group Separator, ASCII `0x1D`. Delimits `app_class` and `title` in the payload. |
| **ETX** | End of Text, ASCII `0x03`. Terminates a reassembled firmware message. |
| **Raw HID** | QMK feature (`RAW_ENABLE`) exposing a 32-byte vendor-defined HID interface. |
| **usage page / usage** | HID descriptor fields. QMK Raw HID defaults: page `0xFF60`, usage `0x61`. |
| **SNI** | StatusNotifierItem — the freedesktop D-Bus tray spec Linux bars host. |
| **qmk_notifier** (underscore) | The Rust transport crate QMKonnect links. |
| **qmk-notifier** (hyphen) | The C firmware module that runs on the keyboard. |
| **`WT(class, title)`** | Firmware macro building a `class\x1Dtitle` pattern. |
| **device filter** | Resolved `{vid?, pid?, usage_page, usage}` used to match HID interfaces. |
| **burst-write** | Sending all 32-byte reports of a long message back-to-back without per-report ack. |
| **board layer / board rules** | The layer/callback state driven by the keyboard's own `DEFINE_SERIAL_LAYERS`/`DEFINE_SERIAL_COMMANDS` matcher against the string QMKonnect sends. See `HOST_RULES.md`. |
| **host layer / host rules** | The layer/callback state driven by QMKonnect matching `rules.toml` on the host and sending `APPLY_HOST_CONTEXT`; stacks **on top of** the board layer. See `HOST_RULES.md`. |
| **callback registry** | The firmware's named, ordered list of host-invokable callbacks (`DEFINE_HOST_CALLBACKS`); the host resolves names→IDs via `QUERY_CALLBACK`. See `HOST_RULES.md` §6. |
| **typed command** | A Raw HID command in the `0x81 0x9F 0xF0` namespace (vs. the legacy string path). See `HOST_RULES.md` §5. |
| **`APPLY_HOST_CONTEXT`** | The typed command carrying the host's desired layer + enabled-callback set; the firmware diffs and applies it. See `HOST_RULES.md` §5. |

---

## 14. Document Map

The master PRD (this file) hard-links these companion specifications. A dev agent
should read them in roughly this order; each is self-contained but assumes the
PRD.

| Document | Scope |
|---|---|
| **`PRD.md`** (this) | Product vision, goals, users, features, glossary, doc map. |
| # SPEC — Architecture & System Design

> Companion to `PRD.md`. Defines the software architecture, repository layout,
> module responsibilities, the end-to-end data flow, the concurrency/threading
> model, the trait design, and the error model. Read alongside the source tree.

---

## 1. Repository Layout

```
qmkonnect/
├── Cargo.toml                 # deps + features; pinned qmk_notifier v0.2.1 (git tag)
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
        ▼  qmk_notifier crate (src/core.rs)
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

### 5.6 Status probe (`is_device_connected`)

Read-only enumeration; `true` iff any interface matches the filter. Backs the
tray device-status line. Runs on a background thread (3 s macOS/Windows,
1 s Linux) and only fires a UI update on a transition.

### 5.7 Host-side-rules extension

Host-side rules extend this pipeline; the full design is in
`HOST_RULES.md` and the wire contract is canonical in the firmware `PRD.md` §4.6.
In summary, after the debounced string send, QMKonnect additionally:
- runs a **capability handshake** at (re)connect (`QUERY_INFO`; gated on
  `proto_ver == 2`) + a `QUERY_CALLBACK` name sweep, and sends `SET_OS` once
  (the host is the OS source of truth while connected);
- evaluates `rules.toml` against the window and sends an `APPLY_HOST_CONTEXT`
  typed command (the `clear_board` flag selects per-window stack vs replace —
  see `HOST_RULES.md` §4); on no-match it always clears the host layer +
  callbacks.
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
| qmk_notifier device cache | caller | `LazyLock<Mutex<Option<DeviceCache>>>` | invalidated on any write error |

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
- **`qmk_notifier` crate** has its own `QmkError` enum (`DeviceNotFound`,
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

---

*Continue with `SPEC_PROTOCOL.md`.* | Repository layout, module map, end-to-end data flow, concurrency/threading model, trait design, error model, the platform-divergence problem. |
| # SPEC — Raw HID Wire Protocol & Transport

> Companion to `PRD.md` / `SPEC_ARCHITECTURE.md`. This is the **exact** contract
> between the QMKonnect desktop app and the qmk-notifier firmware module. Get any
> byte wrong and the two halves will not talk. Covers: message format, report
> framing, all constants, device discovery/matching, the `qmk_notifier` crate
> contract, retry/cache behavior.

---

## 1. The Payload (logical message)

```
{application_class}\x1D{window_title}
```

| Field | Source | Notes |
|---|---|---|
| `application_class` | Win32 window class / macOS `localizedName` / Hyprland `initial_class` / X11 `WM_CLASS` | The stable identifier users match in firmware |
| `\x1D` | ASCII **Group Separator** (decimal 29, `"GS"`) | The delimiter; firmware macro `GS_DELIMITER "\x1D"` |
| `window_title` | the window's title (trimmed) | May be empty (empty workspace, or no Screen Recording perm on macOS) |

**Examples QMKonnect produces:**
- VS Code: `code\x1Dmain.rs - qmkonnect`
- Firefox: `firefox\x1DGitHub - Mozilla Firefox`
- Empty Hyprland workspace: `\x1D` (both empty)
- macOS without Screen Recording: `Safari\x1D` (app name, empty title)

> The desktop app builds the payload **without** a terminator. The `qmk_notifier`
> crate appends the terminator (§2.2).

---

## 2. Report Framing (the byte-level protocol)

### 2.1 HID interface

QMK's Raw HID feature (`RAW_ENABLE = yes`, pulled in by the module's `rules.mk`)
exposes a vendor-defined HID interface with:
- **usage page** `0xFF60` (QMK default `RAW_USAGE_PAGE`, overridable in firmware)
- **usage** `0x61` (QMK default `RAW_USAGE_ID`, overridable in firmware)

This is the **stable signature** QMKonnect auto-discovers by. Exactly one
interface of a typical QMK keyboard (which has ~4 interfaces) carries it.

### 2.2 Logical report size = 32 bytes

```
RAW_REPORT_SIZE = 32   (notifier.c)
REPORT_LENGTH   = 32   (qmk_notifier crate, DEFAULT)
```

> **Critical:** 32 is the *logical* report on **every** QMK USB protocol — it is
> NOT the same as `RAW_EPSIZE` (the USB packet size):
> - ChibiOS (STM32/RP2040/ATSAM) and LUFA (ATmega32U4): endpoint = 32.
> - V-USB (low-speed AVR): endpoint = 8, but the driver **reassembles** a
>   32-byte logical report and guards on `length == 32`. Passing 8 is rejected.
>
> 32 is therefore the single value `raw_hid_send()` accepts on any board.

### 2.3 On-the-wire layout (what `hidapi::HidDevice::write` receives)

The `qmk_notifier` crate builds a **33-byte buffer** per report (hidapi's
`write()` contract demands a leading report-ID byte; the interface has no
report ID so it's `0x00`):

```
 byte[0]      = 0x00              (report ID — hidapi write() leading byte)
 byte[1]      = 0x81              (magic header byte 1 — "this is a notifier message")
 byte[2]      = 0x9F              (magic header byte 2)
 byte[3..33]  = <up to 30 payload bytes>   (zero-filled for the final report)
```

So **30 payload bytes per report** (`PAYLOAD_PER_REPORT = REPORT_LENGTH - 2`).

### 2.4 Message framing across reports

A payload longer than 30 bytes is split into `ceil(len / 30)` back-to-back
reports. The end of the logical message is signaled by an **ETX terminator**
(ASCII `0x03`) appended to the payload *before* framing:

```
batches_for(data) = (data.len() + REPORT_LENGTH - 3) / PAYLOAD_PER_REPORT
                   = (len + 29) / 30            // ceiling
```

The terminator is appended in `qmk_notifier::run`:
```rust
let mut input_with_terminator = Vec::with_capacity(input.len() + 1);
input_with_terminator.extend_from_slice(input);   // input = "{class}\x1D{title}"
input_with_terminator.push(0x03);                 // ETX
send_raw_report(&input_with_terminator, …)
```

### 2.5 Why burst-write is safe without per-report ACK

QMK's raw-HID **OUT** endpoint buffers up to `RAW_OUT_CAPACITY` (4) reports and
drains them all in one main-loop pass (`raw_hid_task`: `while (receive_report())
raw_hid_receive()`). The OUT endpoint provides its own backpressure — when the
device buffer is full it NAKs the transfer and the host's `write()` blocks until
space frees. **Reports are never dropped**, so burst-write is safe for ANY title
length.

(The firmware sends a 32-byte reply per report via `raw_hid_send(response,
RAW_REPORT_SIZE)` — fixed in qmk-notifier commit `01a51935`, which corrected the
response size from the header-stripped `30` to the full `32`. The older "ack is
silently dropped by QMK because `length == RAW_EPSIZE`" wording was stale
carryover from the pre-fix firmware. The crate drains pending IN-side reports
after each burst, bounded, so accumulated replies can't wedge the device; the
typed-command path reads and parses them — see §8.)

---

## 3. Device Discovery & Matching

### 3.1 The match predicate (pure)

A HID interface matches when:
```
interface.usage_page == required_usage_page
  AND interface.usage == required_usage
  AND (required_vid.is_none()  OR interface.vendor_id == required_vid)
  AND (required_pid.is_none()  OR interface.product_id == required_pid)
```

`usage_page`/`usage` are **always required** (default `0xFF60`/`0x61`).
`vendor_id`/`product_id` are **optional** (`None` ⇒ match any ⇒ auto-discovery).

### 3.2 The two discovery modes

| Mode | Config | Behavior |
|---|---|---|
| **Auto (default)** | `vendor_id`/`product_id` unset | Matches any interface with usage page `0xFF60` / usage `0x61`. One standard QMK keyboard → just works. |
| **Disambiguation** | `vendor_id` and/or `product_id` set | Narrows to that VID/PID among multiple QMK boards. Either may be omitted (omitted ⇒ wildcard for that axis). |
| **Custom usage** | `usage_page`/`usage` set | For firmware that overrode `RAW_USAGE_PAGE`/`RAW_USAGE_ID`. Rare. |

### 3.3 Defaults exposed by the `qmk_notifier` crate

```rust
pub const DEFAULT_VENDOR_ID:  u16 = 0xFEED;   // legacy; unused for matching when None
pub const DEFAULT_PRODUCT_ID: u16 = 0x0000;   // legacy; unused for matching when None
pub const DEFAULT_USAGE_PAGE: u16 = 0xFF60;   // THE primary identifier
pub const DEFAULT_USAGE:      u16 = 0x61;     // THE primary identifier
pub const REPORT_LENGTH:      usize = 32;
```

QMKonnect's `configured_filter()` resolves to these defaults when config omits
them.

### 3.4 VID/PID shown vs. matched

The legacy `DEFAULT_VENDOR_ID = 0xFEED` / `DEFAULT_PRODUCT_ID = 0x0000` are
**not** used for matching in auto mode — `None` means "match any". They remain
only as historical fallbacks in the crate's CLI. QMKonnect passes `Option<u16>`
through and `None` always means wildcard.

---

## 4. The `qmk_notifier` Crate Contract (v0.2.1)

QMKonnect links `qmk_notifier` (underscore) as a git-tagged dependency:
```toml
qmk_notifier = { package = "qmk_notifier",
                 git = "https://github.com/dabstractor/qmk_notifier",
                 tag = "v0.2.1" }
```

### 4.1 Public API surface (what QMKonnect calls)

```rust
pub const DEFAULT_USAGE_PAGE: u16;   // 0xFF60
pub const DEFAULT_USAGE: u16;        // 0x61
pub const REPORT_LENGTH: usize;      // 32

pub enum RunCommand { SendMessage(String), ListDevices }

pub struct RunParameters {
    pub command: RunCommand,
    pub vendor_id: Option<u16>,   // None = match any
    pub product_id: Option<u16>,  // None = match any
    pub usage_page: u16,          // required (default 0xFF60)
    pub usage: u16,               // required (default 0x61)
    pub verbose: bool,
}

impl RunParameters {
    pub fn new(command, vendor_id, product_id, usage_page, usage, verbose) -> Self;
}

pub fn run(params: RunParameters) -> Result<(), QmkError>;
pub fn list_hid_devices() -> Result<(), QmkError>;   // verbose device dump
pub fn send_raw_report(data, vid, pid, page, usage, verbose) -> Result<(), QmkError>;
```

### 4.2 `run(SendMessage)` flow

1. Append `0x03` (ETX) to the message bytes.
2. `send_raw_report(payload, vid, pid, page, usage, verbose)`:
   - Compute `batch_count = batches_for(payload)`.
   - **Cache lookup** (`ensure_cache`): if the global `Mutex<Option<DeviceCache>>`
     holds handles opened for the same `MatchKey`, reuse them; otherwise
     enumerate `HidApi`, filter by the predicate, open every match
     (`open_matching_devices`).
   - **Burst to every cached device** (`burst_to_one`): fill the 33-byte stack
     buffer (`[0x00, 0x81, 0x9F, payload…]`), `write()` each report, then
     drain IN-side acks (bounded `IN_DRAIN_MAX = 32`).
   - **Outcome** per attempt: `AllSucceeded` / `Partial{succeeded, failed}` /
     `TotalFailure`. On any failure the cache is **invalidated** (dropped) so
     the next call re-enumerates. `TotalFailure` triggers one retry
     (`SEND_RETRIES = 1`) that rebuilds the cache first.

### 4.3 Error types the app reacts to

QMKonnect's `QmkNotifier::notify` retries only on error strings containing
`"no device found"`, `"permission denied"`, or `"failed to open"` (from
`QmkError::DeviceNotFound` / `DeviceOpenError` / hidapi open failures). Other
errors (e.g. `PartialSendError`) propagate immediately.

### 4.4 Why a device cache

Enumerating the HID bus + opening handles was the dominant per-notification
cost. The cache (`LazyLock<Mutex<Option<DeviceCache>>>`) reuses one `HidApi`
context and the opened handles across calls, rebuilding only when the match key
changes or a write fails (stale handle after replug).

> **Cache caveat (intentional):** a newly-plugged *additional* matching device
> is not picked up until a write fails or the key changes. Fine for the
> single-keyboard case; the replug case is handled via write-failure
> invalidation.

---

## 5. The Firmware Reception Side (summary — full detail: `SPEC_FIRMWARE.md`)

`hid_notify(data, length)` in `notifier.c`:
1. **Guard:** `length < 2 || data[0] != 0x81 || data[1] != 0x9F` ⇒ discard
   (this is what makes qmk-notifier coexist with other Raw HID modules on the
   same interface).
2. Strip the 2 header bytes; iterate the remaining bytes.
3. Append each byte to a static 256-byte `msg_buffer` until an **ETX** (`0x03`):
   - On ETX: NUL-terminate, `sanitize_string` (ASCII-only), reset index, call
     `process_full_message(buffer)`, break.
   - On overflow (`msg_index >= MSG_BUFFER_SIZE-1`): reset index (drop message).
4. `process_full_message` always: `disable_command()` first, then scan
   `command_map` (first match) and `layer_map` (first match); `deactivate_layer`
   then `activate_layer(layer_found)` / `enable_command(cmd_found)`.
5. **Ack:** `raw_hid_send(response, RAW_REPORT_SIZE)` where `response[0] =
   match` (1 if something matched, else 0). The host receives this 32-byte reply
   (fixed in qmk-notifier `01a51935`; see §2.5). The legacy `0`/`1` match-bool
   reply is distinct from the typed `0x51`-marked reply (§8).

---

## 6. Discovery / Diagnostics CLI

| Flag | Effect |
|---|---|
| `--list-devices` | `core::notifier::list_devices()` → enumerates HID without opening; prints `vid:pid  page:usage  product` for every device. The VID/PID discovery tool. |
| (startup) | `startup_device_probe(verbose)` → one read-only enumerate against the configured filter; prints "Found …" or a clear "No device matching …" diagnostic. |

---

## 7. Protocol Constant Reference

| Constant | Value | Where |
|---|---|---|
| Group Separator (GS) | `0x1D` (29) | delimiter in payload; firmware `GS_DELIMITER` |
| End of Text (ETX) | `0x03` (3) | message terminator; firmware `ETX_TERMINATOR`; appended by crate |
| Magic header | `0x81 0x9F` | first 2 payload bytes; firmware coexistence guard |
| Report ID byte | `0x00` | leading byte of the 33-byte hidapi write buffer |
| `RAW_REPORT_SIZE` / `REPORT_LENGTH` | 32 | logical report size (all QMK protocols) |
| Payload per report | 30 | `REPORT_LENGTH - 2` (after the 2 magic bytes) |
| Firmware buffer | 256 | `MSG_BUFFER_SIZE` |
| Default usage page | `0xFF60` | `DEFAULT_USAGE_PAGE` |
| Default usage | `0x61` | `DEFAULT_USAGE` |
| Typed discriminator | `0xF0` | `data[2]` after `0x81 0x9F` ⇒ typed cmd (§8) |
| Typed response marker | `0x51` | vs legacy `0`/`1` match-bool (§8) |

---

## 8. Typed-Command Namespace

> **Canonical owner: the firmware spec** (`dabstractor/qmk-notifier`, `PRD.md`
> §4.6). This section mirrors the transport-relevant summary for desktop work; if
> the two disagree, **the firmware PRD §4.6 wins**. The desktop orchestration
> (handshake, per-window send logic, `rules.toml`) is in `HOST_RULES.md`; the
> transport API is in the `qmk_notifier` crate `PRD.md` §10.

**Discriminator:** `data[2] == 0xF0` ⇒ typed command; anything else ⇒ legacy
string (unchanged). `0xF0` can never begin a real matched string (sanitizer
allows only `0x20–0x7E`), so legacy firmware safely ignores typed commands.

**Framing:** `[0x81][0x9F][0xF0][cmd_id][ args… ][0x03]`, **ETX-framed and
multi-report** like strings (chunked at 30 payload bytes/report). Multi-report
framing removes any fixed cap on `APPLY_HOST_CONTEXT`'s callback-id list.

**Responses (32-byte):** legacy string ⇒ `[matched(0|1)]…`; typed ⇒
`[0x51][cmd_id_echo][payload]…`; no reply within timeout ⇒ `Timeout` ⇒ host
stays in string-only mode.

**Command table:**

| `cmd_id` | Name | Request args | Response payload |
| --- | --- | --- | --- |
| `0x01` | `QUERY_INFO` | none | `[proto_ver][feature_flags][callback_count][board_rules_present]` |
| `0x02` | `QUERY_CALLBACK` | `[index]` | `[index][name, NUL-padded]` |
| `0x03` | `SET_OS` | `[os_byte]` | `[ack]` |
| `0x04` | *(reserved — VIA, Phase E)* | — | — |
| `0x05` | `APPLY_HOST_CONTEXT` | `[layer][flags][count][id…]` | `[ack]` |

- `proto_ver`: `1` = legacy string-only firmware; `2` = typed-command capable.
  Firmware-owned.
- `feature_flags`: `0x01` `APPLY_HOST_CONTEXT`; `0x02` callback registry; `0x04`
  *(reserved)* VIA.
- `os_byte`: `0 UNSURE · 1 LINUX · 2 WINDOWS · 3 MACOS · 4 IOS` (mirrors QMK
  `os_variant_t`). The host sends `SET_OS` once at connect; while connected the
  host's OS is **authoritative** for `current_os`.
- `layer`: desired host-layer number, or `0xFF` (clear). **Host layers reserved ≥
  224** so they resolve above board layers.
- `flags` bit 0 = **`clear_board`**: firmware clears its board `activated_layer` +
  current command before applying the host context — the per-window "replace"
  semantics (`disable_firmware_config` in `rules.toml`).
- `id…`: the full desired enabled callback-id set; firmware diffs
  (disable-before-enable).

**Handshake (at (re)connect, once per board boot):** `QUERY_INFO` → if
`response[0]==0x51` & `proto_ver==2` & `flags & 0x01` → `QUERY_CALLBACK` sweep →
`name→id` map → validate `rules.toml` names. Else (`response[0] != 0x51` or
timeout) ⇒ legacy ⇒ string-only. The firmware sets `has_been_queried` on the
first `QUERY_INFO` to keep a mid-session reconnect from clearing an active board
layer against legacy firmware.

The `qmk_notifier` crate frames these and returns a parsed
`CommandResponse`; see the crate `PRD.md` §10.

---

*Continue with `SPEC_PLATFORMS.md`.* | The Raw HID wire protocol: payload format, report framing, constants, the `qmk_notifier` crate contract, device matching & discovery, retry/cache. |
| # SPEC — Platform Window Monitoring & OS Integration

> Companion to `PRD.md` / `SPEC_ARCHITECTURE.md`. Deep dive into how each
> platform detects the foreground window, what string it reports as
> `application_class`, how windows are filtered, the config-path conventions,
> and the per-OS permission model. Covers `src/platforms/*.rs` and the
> `list_foreground_windows()` enumerations.

---

## 1. The Shared Contract

Every platform implements one trait and a set of free functions dispatched from
`src/platforms/mod.rs`:

```rust
pub trait WindowMonitor: Send {
    fn platform_name(&self) -> &str;
    fn start(&mut self) -> Result<(), Box<dyn Error>>;
    fn stop(&mut self) -> Result<(), Box<dyn Error>> { Ok(()) } // default no-op
}

// Dispatchers (return the right platform's impl):
pub fn create_monitor(verbose: bool) -> Box<dyn WindowMonitor>;
pub fn get_config_paths() -> Vec<PathBuf>;
pub fn create_config_dir() -> Result<PathBuf, Box<dyn Error>>;
pub fn list_foreground_windows() -> Vec<(String, String)>;  // (class, title)
```

On a focus change, a monitor calls `core::notifier::notify_qmk(&WindowInfo,
verbose)` — never formats or sends the HID payload itself.

### 1.1 What `application_class` is, per platform

| Platform | `application_class` value | API |
|---|---|---|
| **Windows** | Win32 window **class name** | `GetClassNameW(hwnd)` |
| **macOS** | the app's **`localizedName`** | `[NSWorkspace.frontmostApplication localizedName]` |
| **Hyprland** | the client's **`initial_class`** | `hyprland::data::Client::get_active().initial_class` |
| **X11** | `WM_CLASS` **class** (2nd field), fallback to instance (1st) | `xprop -id <wid> WM_CLASS` |

> Users discover these exact strings via the "Show Window Information" dialog
> (`SPEC_UI.md` §3) and match them in firmware (`DEFINE_SERIAL_LAYERS` /
> `DEFINE_SERIAL_COMMANDS`). **macOS is case-sensitive as displayed** (e.g.
> `"Safari"`); Windows classes are usually PascalCase (`Chrome_WidgetWin_1`);
> Hyprland classes are lowercase (`firefox`, `neovide`).

### 1.2 Titles

| Platform | Title source | Notes |
|---|---|---|
| Windows | `GetWindowTextW` (trimmed) | Trailing-space padding stripped |
| macOS | `CGWindowListCopyWindowInfo` → `kCGWindowName` for the frontmost app's window | **Requires Screen Recording** (§4.2); empty without it |
| Hyprland | `Client::get_active().title` | |
| X11 | `xprop … _NET_WM_NAME` | |

### 1.3 Empty-workspace semantics

- **Hyprland:** an empty workspace reports `WindowInfo { app_class: "", title: "" }`
  → payload `"\x1D"` → firmware deactivates any active layer. This is desired
  (no app focused ⇒ neutral keymap).
- **Windows / macOS:** no focus event is generated for "no window", so the
  keyboard retains the last-reported app until the next real focus change.

### 1.4 Window filtering (`should_ignore_window`, Windows/macOS)

Internal/shell windows that briefly grab foreground must not be reported:

**Windows** ignores these classes (`src/platforms/windows.rs`):
```
ForegroundStaging, XamlExplorerHostIslandWindow,
Windows.UI.Composition.DesktopWindowContentBridge,
Windows.UI.Input.InputSite.WindowClass, TaskSwitcherWnd,
TaskSwitcherOverlayWnd, Windows.UI.Core.CoreWindow,
ApplicationFrameWindow,                  // UWP frame — want the real content
TopLevelWindowForOverflowXamlIsland,     // Win11 tray-overflow flyout
NotifyIconOverflowWindow,                // Win10 tray-overflow flyout
Shell_TrayWnd, Shell_SecondaryTrayWnd    // taskbar(s)
```
Plus: empty titles are rejected **unless** the class is in an allowlist
(`CASCADIA_HOSTING_WINDOW_CLASS` terminal, `Chrome_WidgetWin_1`), and titles
shorter than 2 chars are rejected.

**macOS:** filters to apps with `activationPolicy == NSApplicationActivationPolicyRegular`
(0) — i.e. Dock-visible apps — and `isFinishedLaunching == YES`. No shell chrome
to filter.

**Hyprland:** `list_foreground_windows()` filters to `mapped` clients; the live
monitor reports whatever Hyprland says is active (including empty).

---

## 2. Windows Monitor (`src/platforms/windows.rs`)

### 2.1 Detection mechanism (belt + suspenders)

1. **`SetWinEventHook(EVENT_OBJECT_FOCUS, EVENT_OBJECT_FOCUS, …,
   WINEVENT_OUTOFCONTEXT)`** — the primary signal. The callback
   `event_proc(hwnd,…)` runs on the thread pumping the message loop (the `tao`
   tray loop on the shipped app). Each focus event → `handle_focus_change`.
2. **100 ms polling thread** (`GetForegroundWindow()` compared to
   `last_hwnd`) — a fallback for events the hook can miss. Deduped against
   `LAST_WINDOW_INFO`.

### 2.2 `handle_focus_change(hwnd)`
- `get_window_info(hwnd)` → `WindowInfo` (class via `GetClassNameW`, title via
  `GetWindowTextW`).
- Skip if `should_ignore_window`.
- Dedup against `LAST_WINDOW_INFO` (`Mutex<Option<(String,String)>>`) to kill
  feedback loops.
- `notify_qmk(&window_info, verbose)`.

### 2.3 Thread-safe globals (replaced former `static mut` UB)
- `G_VERBOSE: AtomicBool`
- `G_HOOK: AtomicIsize` (holds the `HWINEVENTHOOK` handle)
- `LAST_WINDOW_INFO: Mutex<Option<(String,String)>>`

### 2.4 `list_foreground_windows()` (the tray dialog data)
`EnumWindows` over visible (`IsWindowVisible`) top-level windows, reusing
`get_window_info` + `should_ignore_window` so the list **exactly matches** what
the live monitor would report.

### 2.5 Config paths (`get_config_paths`)
1. `%APPDATA%\QMKonnect\config.toml` (primary)
2. `%LOCALAPPDATA%\QMKonnect\config.toml` (secondary)
3. exe directory (fallback)

`create_config_dir()` → `%APPDATA%\QMKonnect`.

---

## 3. macOS Monitor (`src/platforms/macos.rs`)

### 3.1 Detection mechanism
- Registers an observer on `NSWorkspace.sharedWorkspace.notificationCenter`
  for **`NSWorkspaceDidActivateApplicationNotification`**. The handler class
  `RustNotificationObserver` (declared once via `ClassDecl`) implements
  `observeNotification:` → calls `get_active_window_info` → `notify_qmk`.
- `start()` calls `ensure_screen_recording_permission` (§4.2), sets up the
  observer, captures the initial frontmost app, then **`CFRunLoopRun()`** blocks
  the calling thread (a background thread — the tray owns main).

### 3.2 `get_active_window_info()`
- `[NSWorkspace.frontmostApplication localizedName]` → app name.
- Walk `CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, 0)`; find
  the entry whose `kCGWindowOwnerName == app_name`; read `kCGWindowName` → title.
- Returns `WindowInfo { app_class: app_name, title }`.

### 3.3 `list_foreground_windows()` (tray dialog data)
Iterates `NSWorkspace.runningApplications`, keeps `activationPolicy == 0`
(Regular) and `isFinishedLaunching`, maps each to `(localizedName, title)` via a
pre-built `owner → title` map from the CG window list, sorts alphabetically.

### 3.4 Config paths
1. `~/Library/Application Support/QMKonnect/config.toml` (primary)
2. `~/.config/qmk-notifier/config.toml` (XDG-style fallback)
3. `/etc/qmk-notifier/config.toml` (system-wide last resort)

`create_config_dir()` → `~/Library/Application Support/QMKonnect`.

---

## 4. Permission Model (per-OS)

### 4.1 Windows
- **None required** for HID access or foreground-window detection.
- The app runs per-user; no elevation.

### 4.2 macOS — Screen Recording (not Accessibility)
- Window **titles** come from `CGWindowListCopyWindowInfo`, which requires
  **Screen Recording** permission (macOS 10.15+). Without it, titles come back
  empty — but the frontmost **app name** still works, so the app keeps running.
- `ensure_screen_recording_permission(verbose)`:
  - `CGPreflightScreenCaptureAccess()` → if already granted, continue.
  - Else `CGRequestScreenCaptureAccess()` (pops the system dialog, returns
    immediately) and **keep running** (graceful degradation; the app picks up
    titles once the user grants access and quits/reopens).
- **Ad-hoc signing re-prompt loop:** local builds are ad-hoc signed, so the
  `cdhash` changes every rebuild; macOS keys the grant to the signature and
  re-prompts even though System Settings shows it granted. `tccutil reset
  ScreenCapture io.mulletware.qmkonnect` resets it; a stable `CODESIGN_IDENTITY`
  (Developer ID) stops the loop. See `SPEC_PACKAGING.md` §5.

### 4.3 Linux — hidraw permissions (full detail: `SPEC_LINUX.md`)
- Default QMK keyboards need no manual setup: the static udev rule grants
  `GROUP="input", MODE="0660", TAG+="uaccess"` to any `0xFF60/0x61` interface.
- Users may need to be in the `input` group (or rely on the `uaccess` ACL).
- Custom VID/PID users generate a config-driven fallback rule via
  `sudo qmkonnect -r`.

---

## 5. Hyprland Monitor (`src/platforms/hyprland.rs`)

### 5.1 Detection mechanism
- `wait_for_hyprland(verbose)` first (handles the boot race): probe
  `HYPRLAND_INSTANCE_SIGNATURE` + the socket under `$XDG_RUNTIME_DIR/hypr/<sig>/.socket.sock`;
  if absent, scan `$XDG_RUNTIME_DIR/hypr/*/​.socket.sock` and **set** the env
  var once (main thread) so the `hyprland` crate picks the right instance.
  Verify IPC with `Monitors::get()`. Exponential backoff, 30 s timeout.
- Create an `EventListener` and register handlers:
  - `add_active_window_changed_handler` → `handle_window_state_change`
  - `add_workspace_changed_handler` → `handle_workspace_change` (re-derives active window)
  - `add_window_closed_handler` → `handle_window_state_change`
  - `add_layer_opened_handler` / `add_layer_closed_handler` →
    `handle_window_state_change` **+** `spawn_poll_burst` (scratchpad/layer focus)
- `listener.start_listener()` **blocks**; on error, reconnect with backoff.

### 5.2 `handle_window_state_change`
- `Client::get_active()` → if `Some`, report `initial_class` + `title`. If
  `None`, report the empty-window `WindowInfo` (deactivates layers).
- Updates `last_window_state` (`Arc<Mutex<Option<WindowState>>>`) for dedup.

### 5.3 Reconnect backoff (fixes #7)
- `INITIAL_RECONNECT_MS = 100`, `MAX_RECONNECT_MS = 10_000`, growth `×3`.
- **Reset to initial** when a listener that stayed up ≥
  `STABLE_CONNECTION_THRESHOLD` (5 s) is lost, so long-uptime sessions don't
  stick at the 10 s cap.
- **Hard-fail** only if the very first attempt dies within 2 s of startup
  (Hyprland genuinely unavailable).

### 5.4 Polling strategies (two distinct ones)
- **Optional periodic poll** (`poll_interval_ms > 0`, default 0 = off): a thread
  polls `Client::get_active()` on the configured cadence and dedups against
  `last_window_state`. Corrects IPC drift (notably `movetoworkspacesilent`
  scratchpad dismissals where the `activewindow` event lags).
- **Poll burst after layer events** (`spawn_poll_burst`): 5× 100 ms polls after
  a layer open/close, to absorb the timing gap where focus hasn't settled at
  event time. Replaces the former permanent 100 ms poller.

### 5.5 `list_foreground_windows()` (tray dialog data)
`Clients::get()` filtered to `mapped`, mapped to `(class, title)`, with the
active window moved to front (so `.next()` reports the focused window).

---

## 6. X11 Monitor (`src/platforms/x11.rs`) — fallback, non-default build

- Built only with `--no-default-features` (no `hyprland`).
- `get_active_window_info()`: `xprop -root _NET_ACTIVE_WINDOW` → window id →
  `xprop -id <wid> WM_CLASS _NET_WM_NAME`. `WM_CLASS` second field (class) is
  preferred; first field (instance) is the fallback. `0x0` ⇒ empty workspace.
- **Fails loudly** if `xprop` is missing (never emits placeholder strings — #14).
- Polls every **500 ms** on a background thread (X11 focus changes are
  user-driven; latency is acceptable for a fallback).

---

## 7. Where Each Monitor Runs (thread summary)

| Monitor | Thread | Why |
|---|---|---|
| Windows | hook on message-loop thread (main, via `tao`); 100 ms poll thread | `WINEVENT_OUTOFCONTEXT` needs a pumped loop |
| macOS | background thread (`CFRunLoopRun` blocks) | tray/`tao` owns main |
| Hyprland | calling thread (`start_listener` blocks); optional poller thread | no GUI loop |
| X11 | background thread | tray/park owns main |

(Full concurrency table in `SPEC_ARCHITECTURE.md` §6.)

---

## 8. Internal Window Filtering Reference (Windows)

The full ignore-list and the empty-title allowlist live in
`should_ignore_window` (`src/platforms/windows.rs`). When adding a new app that
spuriously grabs focus, add its **window class** (locale-independent), never its
title. Both Win11 (XAML island) and Win10 (classic) shell generations are
covered.

---

*Continue with `SPEC_UI.md`.* | Per-OS window monitoring (Windows WinEventHook, macOS NSWorkspace, Hyprland IPC, X11), window filtering, config paths, permissions. |
| # SPEC — Tray / Menu-Bar UI, Dialogs & Autostart

> Companion to `PRD.md` / `SPEC_ARCHITECTURE.md`. The full user-facing surface:
> tray/menu-bar icon + menu, the Settings dialogs, the "Show Window
> Information" dialogs, the live device-status indicator, and the per-platform
> "Open at Login" autostart. Covers `src/tray.rs` (macOS/Windows),
> `src/linux_tray.rs` (Linux SNI + GTK), and `src/autostart.rs` (Windows).

---

## 1. The Tray Surface (per platform)

| Platform | Crate stack | Where the icon shows | Menu model |
|---|---|---|---|
| **macOS** | `tray-icon` + `tao` + `objc` | Menu bar | muda `Menu` (native Cocoa) |
| **Windows** | `tray-icon` + `tao` + `windows` | System tray | muda `Menu` (native Win32) |
| **Linux** | `ksni` (SNI over `zbus`) | Any SNI-hosting bar (Waybar, SwayNC, KDE, GNOME+AppIndicator) | D-Bus `com.canonical.dbusmenu` serialized tree |

`src/tray.rs` is compiled for `cfg(not(all(target_os="linux", feature="hyprland")))`
— i.e. macOS, Windows, and the non-Hyprland Linux build. The Hyprland/Linux
build uses `src/linux_tray.rs` (feature `linux-tray`, default-on).

### 1.1 Menu layout (macOS / Windows, identical item set modulo labels)

```
About QMKonnect                              ← PredefinedMenuItem::about
●  Device Connected   /  ○ No Device Connected   ← disabled MenuItem (line 2)
─────────────                                ← separator
[Launch at Login  /  Open at Login]          ← CheckMenuItem (macOS / Windows)
Settings                                     ← MenuItem
─────────────                                ← separator
Show Window Information...                   ← MenuItem (macOS/Windows only)
─────────────                                ← separator
Quit                                         ← MenuItem
```

- **Line 2 (device status):** a **disabled** `MenuItem` whose text is refreshed
  by a background thread (§4). Solid dot `U+25CF` = connected, hollow `U+25CB` =
  absent. Synchronous probe at first paint so the initial state is correct.
- **Autostart toggle:** `CheckMenuItem`. macOS = "Launch at Login"
  (SMAppService); Windows = "Open at Login" (HKCU `Run`). Initial checkmark
  reflects real system state.
- **Show Window Information:** macOS/Windows only (Linux exposes windows via
  `hyprctl` natively, plus the SNI menu surfaces it through a GTK popup).

### 1.2 The Linux SNI menu (`src/linux_tray.rs`)

```
●  Device Connected   /  ○ No Device Connected   ← disabled StandardItem (line 1)
(hidden structural toggle)                   ← visible:false, forces LayoutUpdated redraw
─────────────
Settings…                                    ← zenity --forms (writes config.toml)
─────────────
Show Window Information                      ← notify-send / native GTK popup
─────────────
Quit                                         ← process::exit(0)
```

The hidden structural item is deliberate: changing the *count* of visible items
forces ksni to emit `LayoutUpdated` (the signal every SNI host honors to redraw
an *open* popup), whereas `ItemsPropertiesUpdated` is ignored by some hosts
(e.g. Quickshell) for open menus.

### 1.3 Icon handling
- **macOS:** monochrome **template** asset `IconTemplate.png` loaded from the
  bundle's `Resources/`; `with_icon_as_template(true)` so macOS tints it to the
  bar. Falls back to a generated 16×16 white square.
- **Windows:** `IconTray-dark.png` beside the exe (installer drops it), zoomed
  ~20% (clamped to headroom) so the glyph renders larger in the fixed tray slot.
- **Linux:** two embedded variants — `IconTray-dark.png` (light outline, for
  dark bars) and `IconTray-light.png` (dark outline, for light bars) — selected
  by querying the `org.freedesktop.appearance.color-scheme` portal (1=dark,
  2=light, 0=no pref→dark). The icon is **dimmed to ~35% alpha** when the device
  is absent (disconnect visible in realtime; `NewIcon` is honored by hosts).

### 1.4 The `EventLoopProxy` pattern (macOS/Windows)
`muda::MenuItem` is `!Send` (`Rc<RefCell<…>>`). Background threads (device-status
poll, deferred autostart register) deliver state to the main thread via
`tao::EventLoopProxy<UserEvent>`:

```rust
enum UserEvent {
    MenuEvent(MenuEvent),
    DeviceStatus(bool),   // macOS/Windows
    AutostartSync,        // macOS — re-sync checkbox after deferred register
}
```

The event-loop arm mutates menu items (the only safe place).

---

## 2. Settings Dialogs

All three write `config.toml` via the **shared** `core::render_config_body(vid,
pid)` so the file format is identical everywhere. VID/PID fields are optional
(empty/`"auto"` ⇒ `None` ⇒ auto-discovery). Config is hot, so a save takes
effect within ~3 s (no restart).

### 2.1 Windows — native Win32 dialog (`show_settings_dialog`)
- A registered `QMKSettingsDialog` window class, `WS_OVERLAPPED|WS_CAPTION|
  WS_SYSMENU|WS_VISIBLE`, 400×200, centered, `COLOR_3DFACE` background, app icon.
- Controls (ids): `1001` Vendor ID `EDIT`, `1002` Product ID `EDIT`, `1003` OK
  `BUTTON`, `1004` Cancel `BUTTON`; static labels. Fields pre-filled with the
  current 4-digit hex (empty if `None`).
- `settings_dialog_proc`: on OK, `GetDlgItemTextW` both fields,
  `parse_id_field` each (empty/`auto` ⇒ `None`), store `(Option<u16>,
  Option<u16>)` in the shared `DIALOG_RESULT: Mutex`, `DestroyWindow`. On parse
  error, `MessageBoxW`. Modal loop via `GetMessageW`.
- Result written to `config_path` by `render_config_body`. No success dialog
  (the connection is rebuilt per notification).

### 2.2 macOS — `NSAlert` + accessory view (`show_macos_settings_dialog`)
- Wraps in an `NSAutoreleasePool` (background `LSUIElement` apps lack a main
  pool). `NSAlert` with message text showing current `format_id_hex` values
  ("auto" when `None`); OK/Cancel buttons; accessory `NSView` with two
  `NSTextField`s pre-filled.
- On OK (response `1000`), read both fields, `parse_id_field`, write via
  `render_config_body`. Errors via `NSAlert` (critical).

### 2.3 Linux — `zenity --forms` (`show_settings_dialog_linux`)
- `zenity --forms --title=QMK Settings --add-entry="Vendor ID (hex)"
  --add-entry="Product ID (hex)"`, text shows current values. `--ok-label=Copy`
  is *not* used here (that's the window-info dialog).
- Parse the `|`-separated stdout; `parse_id` each (empty/`auto` ⇒ `None`).
- On save: `write_config` then `apply_device_rule(vid,pid)`:
  - Both `None` ⇒ no rule needed (static usage-page rule covers it); best-effort
    `udevadm control --reload-rules` + `udevadm trigger`.
  - At least one `Some` ⇒ render the VID/PID rule, stage under `std::env::temp_dir()`
    (not a predictable `/tmp` name), install via **`pkexec`**
    (`install -m644 …/99-qmkonnect.rules && udevadm … && rm`). If pkexec is
    unavailable/cancelled, surface "Run: `sudo qmkonnect -r`" (which is now
    root-aware — `SPEC_LINUX.md` §4).
- Notifications via `notify-send` (`--app-name=QMKonnect --icon=input-keyboard`).

### 2.4 `parse_id_field` / `parse_id` (shared logic)
- Trim; empty **or** literal `"auto"`/`"AUTO"` ⇒ `Ok(None)`.
- Strip optional `0x`/`0X` prefix; `u16::from_str_radix(_, 16)` ⇒ `Ok(Some(v))`.
- Anything else ⇒ `Err`.

---

## 3. "Show Window Information" Dialogs

Purpose: let users discover the **exact** `class`/`title` strings to put in
their `DEFINE_SERIAL_LAYERS` / `DEFINE_SERIAL_COMMANDS`. Each row shows
`<class>  —  <title>` (class only if title empty) and a per-row **Copy** that
copies `"class|title"` (or just `class` when title is empty) — the config-style
form.

### 3.1 Windows — native Win32 (`show_window_info_dialog`)
- Registered `QMKWindowInfoDialog` class; `WS_OVERLAPPED|WS_CAPTION|WS_SYSMENU|
  WS_THICKFRAME|WS_MINIMIZEBOX|WS_MAXIMIZEBOX|WS_VSCROLL`; resizable (min
  480×320, default 760×520); white background; Segoe UI font.
- Layout: fixed bold **header** ("Class (what QMKonnect reports) — Window
  title"), scrollable **rows** (one read-only selectable `EDIT` label + a
  **Copy** `BUTTON` per row, 26 px row height), fixed **footer** tip.
- Scroll: `WM_VSCROLL`/`WM_MOUSEWHEEL` (3 rows/wheel notch)/thumb; an
  `AtomicI32 WININFO_SCROLL_POS` shared with the WndProc; `wininfo_relayout`
  repositions in-view rows and hides off-screen ones.
- Copy: control ids `WI_IDC_COPY_BASE = 6000` + row index; on `WM_COMMAND` with
  `id >= 6000`, look up the row in `WINDOW_INFO_ROWS` and
  `copy_to_clipboard_windows(hwnd, text)` (`CF_UNICODETEXT`, `GlobalAlloc`
  `GMEM_MOVEABLE`, ownership transfers to OS on success — do not `GlobalFree`).
- Debug CLI: `qmkonnect --show-window-info` opens it directly (no tray).

### 3.2 macOS — `NSWindow` + `NSScrollView` (`show_macos_window_info_dialog`)
- `NSAutoreleasePool`; `[NSApp activateIgnoringOtherApps:YES]` (background apps
  must activate or windows can't become key).
- `NSWindow` (`alloc` → `initWithContentRect:styleMask:backing:defer:`),
  `setReleasedWhenClosed:NO`, `center`, titled+closable+miniaturizable.
- `NSScrollView` with an `NSView` document holding one row per app (origin is
  bottom-left, so rows are top-aligned by counting down). Each row: an
  `NSTextField` label (selectable, truncating-tail) + an `NSButton` with an SF
  Symbol (`doc.on.doc`, macOS 11+; `respondsToSelector:`-guarded, else "Copy"
  text). Button `tag = row index`; target `RustWindowInfoCopyTarget` →
  `wi_copy_row:` → `copy_to_pasteboard_macos`.
- Modal via `[NSApp runModalForWindow:]`; the `RustWindowInfoWindowDelegate`
  calls `[NSApp stopModal]` on `windowWillClose:`.

### 3.3 Linux — native GTK popup, zenity fallback (`show_window_info_linux`)
- **Native GTK popup** (preferred): a single owner thread runs `gtk::main()`
  for the process lifetime; requests arrive over an `mpsc` channel polled from
  the main loop. Each request opens a `GtkWindow` (`WindowType::Toplevel`,
  **dialog type-hint** + `set_resizable(false)` + fixed default size → floats on
  every tiling compositor), with a `ScrolledWindow`+`ListBox` (`vexpand`) of
  rows, each a `Label` (end-ellipsized) + a **Copy** `Button` →
  `Clipboard::set_text("class|title")`. 640×760 default.
  - Why not zenity: zenity `--forms` floats but caps the list at ~3–4 rows;
    zenity `--list` is tall but tiles. No single zenity invocation is both.
- **zenity fallback** (`show_window_info_linux_zenity`): `--forms --add-list`
  with `--ok-label=Copy`; height-capped (~3–4 rows) — a hard zenity limitation.
  Select a row → Copy → copies `class|title`. Clipboard via `wl-copy` then
  `xclip`. A `notify-send` notification confirms the copy or reports clipboard
  unavailability.
- Runs on a dedicated thread so ksni's IPC thread stays responsive.

### 3.4 Shared row store
`WINDOW_INFO_ROWS: Mutex<Vec<(String,String)>>` (`tray.rs`, macOS/Windows) —
both the copy-button target and the Win32 WndProc look up the row to copy **by
index**. Only one dialog open at a time, so a single shared slot suffices.

---

## 4. Device-Connection Status Indicator

A **read-only** `hidapi` enumeration (`core::notifier::is_device_connected()`)
runs on a background thread and refreshes the tray's status line **only on a
transition** (to avoid needless UI/D-Bus traffic).

| Platform | Poll cadence | Delivery to UI |
|---|---|---|
| macOS / Windows | 3 s | `EventLoopProxy.send_event(UserEvent::DeviceStatus(bool))` → event-loop arm sets `device_status_i.set_text(...)` |
| Linux (ksni) | 1 s | `handle.update(|t| t.device_connected = …)` (re-serializes menu + icon; also re-checks color scheme every 10 ticks) |

Status text: `"●  Device Connected"` (solid `U+25CF`) / `"○  No Device
Connected"` (hollow `U+25CB`). The probe **never opens the device** (pure
enumeration), so it cannot disturb the keyboard.

---

## 5. Tray Lifecycle in the Runners

- **Windows:** `run_tray_app()` → singleton guard → start monitor →
  `tray::setup_tray()` (blocks until Quit).
- **macOS:** monitor on a background thread → `tray::setup_tray()` on main
  (blocks until Quit).
- **Linux/Hyprland:** `linux_tray::spawn()` (ksni, own thread; handle kept
  alive) → `monitor.start()` blocks on the IPC listener.
- **Linux/X11:** monitor on a background thread → if `linux-tray`, park main;
  else `tray::setup_tray()`.

`Quit` (any platform) → `tray_icon.take()` + `ControlFlow::Exit` (macOS/Windows)
or `process::exit(0)` (Linux).

---

## 6. Autostart ("Open at Login" / "Launch at Login")

Default **on** on first run on every platform, with an obvious in-app toggle.
Never fights the user afterwards.

### 6.1 Windows — HKCU `Run` key (`src/autostart.rs`)
- **Single source of truth:** value name `"QMKonnect"` under
  `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`, type
  `REG_SZ`, data = `current_exe()` path. Shared with the installer
  (`install.ps1` / `QMKonnect.iss` / `uninstall.ps1`) — same name everywhere.
- `is_enabled()`: `RegGetValueW(HKCU, SUBKEY, VALUE, RRF_RT_REG_SZ, …)`; presence-based
  (a Task-Manager "Disabled" override under `StartupApproved\Run` is intentionally
  *not* consulted — most apps behave this way).
- `set_enabled(true)`: `RegOpenKeyExW(KEY_SET_VALUE)` → `RegSetValueExW(REG_SZ,
  UTF-16 incl. NUL)` → `RegCloseKey`. `set_enabled(false)`:
  `RegDeleteValueW`.
- Path **self-heals**: written from `current_exe()` at toggle time.
- Tray handler `handle_open_at_login_click`: muda flips the native check on
  click *before* dispatching, so `is_checked()` is already the new desired
  state; persist it, then `set_checked(is_enabled())` to revert visibly on
  failure.

### 6.2 macOS — `SMAppService` (`tray.rs` `mod autostart`)
- Links `ServiceManagement.framework`. `SMAppService.mainApp()` `register`/
  `unregister` (macOS 13+). Status raw values: 0=notRegistered, 1=enabled,
  2=requiresApproval, 3=notFound.
- **First-run default-on** (`autostart_first_run_default_on`): deferred onto the
  main run loop via `dispatch::Queue::main().exec_async` (registration's XPC
  round-trip never blocks the Init callback); gated by a marker file
  `~/Library/Application Support/QMKonnect/.autostart_initialized` so it never
  re-enables after the user turns it off. Signals `UserEvent::AutostartSync` to
  re-sync the checkbox.
- Tray handler `handle_launch_at_login_click`: derives desired state from the
  **real** `is_enabled()` (robust to muda's auto-toggle), performs register/
  unregister, mirrors outcome into the checkmark.

### 6.3 Linux — systemd user service (`SPEC_LINUX.md` §6)
- The static udev rule sets `ENV{SYSTEMD_USER_WANTS}+="qmkonnect.service"` and
  the service is `BindsTo=dev-qmkonnect_device.device` (the symlink the rule
  creates) → the service **starts when the keyboard appears, stops when it
  disappears**. `systemctl --global enable` (from `post_install`) makes it
  start at login.

---

## 7. Activation Policy & Dock Icon (macOS)
- `Info.plist` sets `LSUIElement = true` (launch-time: no Dock icon, no
  CMD-Tab).
- **But** `tao`'s runtime default promotes to Regular in
  `applicationDidFinishLaunching`, overriding `LSUIElement`. So `setup_tray()`
  sets `EventLoopExtMacOS::set_activation_policy(ActivationPolicy::Accessory)`
  **before** `run()` — the only place tao honors it. Accessory apps can still
  surface windows (Settings, Window Info) transiently.

---

*Continue with `SPEC_LINUX.md`.* | Tray/menu-bar UI, menu layouts, Settings dialogs, "Show Window Information" dialogs, device-status indicator, "Open at Login" autostart. |
| # SPEC — Linux Integration (udev, systemd, SNI tray)

> Companion to `PRD.md` / `SPEC_PLATFORMS.md` / `SPEC_UI.md`. Everything
> Linux-specific that is *not* the Hyprland window monitor itself: the static
> udev rule + `qmkonnect-hid-id` helper, the config-driven fallback rule,
> dangerous-rule detection/repair, the root-aware `--reload`, the systemd user
> service, the SNI tray, and the GTK window-info dialog. Covers
> `src/platforms/linux.rs`, `src/bin/hid_id.rs`, `src/linux_tray.rs`, and
> `packaging/linux/`.

---

## 1. The Two-Rule Strategy ("hybrid")

Linux device permissions use **two complementary rules**, so nobody gets left out:

| Rule | File | Who it covers | When written |
|---|---|---|---|
| **Static usage-page rule** | `/usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules` | **Every default QMK keyboard** (usage page `0xFF60` / usage `0x61`) | Shipped by the package; **never regenerated from config** |
| **Config-driven fallback** | `/etc/udev/rules.d/99-qmkonnect.rules` | Custom-usage/page users, or VID/PID disambiguation | Generated **on demand** by `qmkonnect --reload` / the Settings dialog |

The static rule is numbered **69** so it runs before any user-generated
`99-qmkonnect.rules`. Default users therefore need **no `--reload`, no sudo**.

---

## 2. The Static Rule

```
# packaging/linux/udev/69-qmkonnect-rawhid.rules
SUBSYSTEM=="hidraw", IMPORT{program}="/usr/lib/udev/qmkonnect-hid-id %S%p"
ENV{ID_QMKONNECT}=="1", GROUP="input", MODE="0660", TAG+="uaccess", \
  SYMLINK+="qmkonnect_device", TAG+="systemd", \
  ENV{SYSTEMD_USER_WANTS}+="qmkonnect.service"
```

- **`IMPORT{program}`** runs `qmkonnect-hid-id` with the hidraw syspath (`%S%p`);
  it prints `ID_QMKONNECT=1` iff the interface carries the QMK signature (§3).
- **`ENV{ID_QMKONNECT}=="1"`** gates everything that follows (so non-matching
  devices are untouched).
- **Permissions:** `GROUP="input", MODE="0660"` (group-accessible hidraw node)
  **+** `TAG+="uaccess"` (per-session ACL via systemd-logind). `uaccess` is
  primary; the `GROUP`/`MODE` fallback is required because `uaccess` is applied
  once at device-add and is *not* retried — on a mid-session replug it can race
  logind and leave the node at the kernel default `0600 root`, locking out the
  app until reboot.
- **`SYMLINK+="qmkonnect_device"`** + **`TAG+="systemd"`** +
  **`ENV{SYSTEMD_USER_WANTS}+="qmkonnect.service"`** → systemd starts the user
  service when the device appears (and the `BindsTo` in the service stops it when
  the device disappears).

---

## 3. The `qmkonnect-hid-id` Helper (`src/bin/hid_id.rs`)

Pure **`std`** (no hidapi, no heavy deps — runs in udev context, must start
fast). Second bin target in `Cargo.toml`:
```toml
[[bin]]
name = "qmkonnect-hid-id"
path = "src/bin/hid_id.rs"
```

### 3.1 Behavior
- Resolve syspath: `argv[1]` if given (udev passes `%S%p`), else `$DEVPATH`
  prefixed with `/sys` (udev sets `DEVPATH` absolute, so strip the leading `/`
  before joining).
- Read `<syspath>/device/report_descriptor` (binary). Unreadable ⇒ exit 0
  printing nothing (udev treats no stdout as "no properties").
- Walk the HID report-descriptor item stream looking for the QMK signature: a
  **Global Usage Page** item (`bType==1, bTag==0`) set to `0xFF60`, followed by a
  **Local Usage** item (`bType==2, bTag==0`) set to `0x61`. Items between them
  are ignored.
- On match: print exactly `ID_QMKONNECT=1\n` and exit 0. No match ⇒ exit 0
  printing nothing.

### 3.2 HID item parsing (short + long items)
- Prefix byte `b`: `size = match b & 0x03 {0=>0,1=>1,2=>2,3=>4}`;
  `bType = (b>>2)&0x03`; `bTag = (b>>4)&0x0F`.
- `0xFE` prefix ⇒ **long item**: next byte is the data size; skip `2 + size`
  bytes (bounds-checked).
- Data read little-endian (`read_le`, 0–4 bytes).
- Bounds-check every item; truncation ⇒ exit 0 (no match), never panic.

### 3.3 Verified byte signatures (real hardware)
- Usage page `0xFF60` appears as `06 60 ff` (global, tag 0, 2 data bytes LE).
- Usage `0x61` appears as `09 61` (local, tag 0, 1 data byte). *(Tag 0, not 2 —
  tag 2 is Usage Maximum and would emit `29 61`.)*

---

## 4. The Config-Driven Fallback Rule + Root-Aware Reload (`src/platforms/linux.rs`)

### 4.1 `render_vidpid_rule(vendor_id, product_id) -> Option<String>`
- **`None`** when both IDs are `None` (the static rule already covers that case).
- Otherwise emits **exactly one physical rule line** beginning with the `KERNEL==`
  match key:
  ```
  # Managed by qmkonnect --reload; edit config.toml then re-run to update.
  KERNEL=="hidraw*", SUBSYSTEM=="hidraw", [ATTRS{idVendor}=="VVVV", ]\
  [ATTRS{idProduct}=="PPPP", ]\
  TAG+="uaccess", GROUP="input", MODE="0660", SYMLINK+="qmkonnect_device", \
  TAG+="systemd", ENV{SYSTEMD_USER_WANTS}+="qmkonnect.service"
  ```
- When only one of VID/PID is set, the unset `ATTRS{...}` clause is **omitted
  entirely** — udev `ATTRS=="..."` cannot wildcard (`=="*"` is invalid), so the
  unset side matches any value.

### 4.2 `update_udev_rules(vendor_id, product_id, verbose)` (used by `--reload`)
- Read the existing `/etc/udev/rules.d/99-qmkonnect.rules`; check if it's the
  globally-dangerous legacy form (§5).
- If both IDs unset: no fallback rule needed — *unless* a dangerous legacy rule
  is on disk, in which case **purge** it. Otherwise no-op (static rule covers it).
- If a rule is needed: write it **atomically** via `tempfile::NamedTempFile`
  in the rules dir + `sync_all` + `persist` (no predictable `/tmp` staging, no
  `sudo mv` race, no `sudo` invocation that fails without a TTY).
- On `PermissionDenied` (non-root): print the exact rule + a copy-paste
  `sudo tee … <<'EOF' … EOF` command instead of failing.

### 4.3 `reload_udev_rules()`
- Runs `udevadm control --reload-rules` directly (no `sudo`); succeeds as root,
  logs a non-fatal warning otherwise.

### 4.4 Root-aware config resolution (`resolve_config_for_reload`) — fixes #26
Under plain `sudo`, `HOME=/root`, so the normal search never finds the invoking
user's `~/.config/qmk-notifier/config.toml` and the old code **silently
no-op'd** without writing the rule. New resolution order:
1. Explicit `--config <path>` wins.
2. **When root:** prefer the *invoking* user's config — resolve a target
   uid/name from `--uid` > `--user` > `$SUDO_UID` > `$PKEXEC_UID` (and
   `$SUDO_USER`), look up the home via **`getent passwd <key>`** (always
   present, no unsafe; field 6), then a last-resort single-config scan of
   `/home/*`.
3. The normal search path (`get_config_paths`).
4. **Fail loudly** — list every path tried and exit non-zero (never silently
   return `Ok`).

The reload CLI passes `--config`/`--user`/`--uid` (value flags parsed by
`main::parse_value_flag`).

---

## 5. Dangerous-Legacy-Rule Detection & Repair

> An older build wrote `/etc/udev/rules.d/99-qmkonnect.rules` as a **multi-line
> rule with no backslash continuations**. Because udev treats every newline as
> the end of a rule (a trailing comma does **not** continue a line), the
> bare-assignment lines matched **every device on the host** and re-permissioned
> them to `root:input 0660` — breaking `/dev/null`, `/dev/kvm`, `/dev/fuse`,
> and crashing libvirt/QEMU VMs.

### 5.1 `is_rule_globally_dangerous(content) -> bool`
- First **join** backslash-continuations (`\` + newline → space; handles LF/CRLF).
- Then flag any remaining line whose **first key is an assignment** (`=`/`+=`/
  `:=`/`-=`) rather than a match (`==`/`!=`). A line with no leading match key
  matches every device.

### 5.2 `rule_line_has_leading_match_key(line) -> bool`
Skips the key name (`[A-Z_]+`), an optional `{...}` payload (`ATTRS{...}`,
`ENV{...}`, `IMPORT{...}`), and checks the operator is `==`/`!=`.

### 5.3 Repair path
`update_udev_rules` checks `is_rule_globally_dangerous` on the existing rule;
if dangerous, it prints a critical "Repairing globally-dangerous legacy udev
rule" notice and **overwrites** it with the correct single-line form (or purges
it when no fallback rule is needed). Regression tests assert the rendered rule
is always a single safe line starting with a match key, and that the exact
broken form from the bug report is detected.

---

## 6. The systemd User Service

### 6.1 Template (`packaging/linux/systemd/qmkonnect.service.template`)
```ini
[Unit]
Description=QMKonnect - QMK Keyboard Window Notifier
After=graphical-session.target
BindsTo=dev-qmkonnect_device.device      # the symlink the static rule creates
StartLimitBurst=5
StartLimitIntervalSec=60

[Service]
Type=simple
ExecStart=/usr/bin/qmkonnect
Restart=always
RestartSec=5
Environment=RUST_BACKTRACE=1
PrivateTmp=false
ProtectSystem=full
ProtectHome=false
NoNewPrivileges=true
ReadWritePaths=/dev
ReadWritePaths=%t

[Install]
WantedBy=default.target
```
- **`BindsTo=dev-qmkonnect_device.device`**: stops the service when the keyboard
  unplugs; waits for it at boot.
- **`Restart=always`** + `panic="abort"` ⇒ crash recovery without `catch_unwind`.
- The package's `post_install` instantiates it to
  `/usr/lib/systemd/user/qmkonnect.service` and runs
  `systemctl --global enable qmkonnect.service`.

### 6.2 Why the service is optional
The static udev rule's `SYSTEMD_USER_WANTS` starts the service on device arrival
*if it's enabled*. The user can instead run `qmkonnect & disown` directly. The
service is the recommended path for hotplug auto-start.

---

## 7. SNI Tray (`src/linux_tray.rs`, feature `linux-tray`)

StatusNotifierItem over the session D-Bus via **`ksni`** (`features=["blocking"]`),
pure-Rust (no GTK main loop). Runs on **its own D-Bus thread**; the Hyprland
monitor blocks separately on its IPC listener. See `SPEC_UI.md` §1.2–1.4 for the
menu/icon/status details.

### 7.1 `spawn() -> Option<Handle>`
- `QmkTray { device_connected, dark_mode }.assume_sni_available(true).spawn()`:
  - `assume_sni_available(true)` ⇒ register-and-wait rather than hard-failing
    when no SNI host is running. So: no bar at startup ⇒ the item waits silently
    and appears when one starts; no bar at all ⇒ runs headless forever; no
    session D-Bus ⇒ logs the error and runs trayless (returns `None`).
- Poll thread: every **1 s** re-probe `is_device_connected()` (re-reads config
  every call), every **10** ticks re-query the color-scheme portal; on a
  transition call `handle.update(|t| { t.device_connected = …; t.dark_mode = …; })`
  (ksni re-serializes menu + icon; SNI hosts repaint).

### 7.2 Color-scheme detection (`detect_dark_mode`)
Shells out to `dbus-send` reading
`org.freedesktop.portal.Settings.Read org.freedesktop.appearance color-scheme`
(`1`=dark, `2`=light, `0`=no pref → default **dark**). `dbus-send` is chosen over a
zbus variant-deserialization coupling. `parse_color_scheme` is unit-tested.

### 7.3 Why no notify-rust
The "Show Window Information" notification uses `notify-send` (shelled out)
because `notify-rust`'s blocking `show()` spawns a nested tokio runtime, which
panics inside ksni's handler thread.

---

## 8. Config Paths (Linux)

`get_config_paths()` (`src/platforms/linux.rs`), in order:
1. `$XDG_CONFIG_HOME/qmk-notifier/config.toml`
2. `~/.config/qmk-notifier/config.toml`
3. `/etc/qmk-notifier/config.toml`

`create_config_dir()` → `$XDG_CONFIG_HOME/qmk-notifier` or
`~/.config/qmk-notifier`.

> **Naming:** Linux uses `qmk-notifier/` (historical; preserves existing
> installs). Windows/macOS use `QMKonnect/`. This divergence is documented, not
> a bug.

---

## 9. Linux Dependencies (`Cargo.toml`)

```toml
[target.'cfg(target_os = "linux")'.dependencies]
hyprland   = { version = "0.4.0-beta.2", optional = true }   # feature "hyprland"
libxdo     = "0.6"
tempfile   = "3.0"
libc       = "0.2"                          # only geteuid() for root-aware reload
ksni       = { version = "0.3", optional = true, features = ["blocking"] }   # feature "linux-tray"
gtk        = { version = "0.18", optional = true }                           # feature "linux-tray" (window-info popup)
```

- **`libxdo`** is an unconditional dep (used by the non-default X11 path).
- **`gtk` 0.18** is already compiled into the binary via libappindicator/
  tray-icon, so the GTK popup reuses it (free dep). Runs on a dedicated thread;
  ksni's IPC thread stays pure-IPC.
- **Platform libs:** Ubuntu/Debian `libxdo-dev libudev-dev`; Fedora
  `libxdo-devel systemd-devel`. The Arch build links `-lhidapi-hidraw` (not
  `-lhidapi-libusb`) so usage/usage_page matching works.

---

*Continue with `SPEC_CONFIG.md`.* | Linux-specific: static udev rule, `qmkonnect-hid-id` helper, config-driven fallback rule, dangerous-rule detection/repair, root-aware `--reload`, systemd service, SNI tray, GTK window-info dialog. |
| # SPEC — Configuration & CLI

> Companion to `PRD.md`. The complete TOML schema, defaults, the shared
> config-body renderer, per-OS config paths, and the full CLI flag reference.
> Covers `src/core/mod.rs` and `src/main.rs`.

---

## 1. Config Schema (`src/core/mod.rs`)

```rust
#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct Config {
    #[serde(default)] pub vendor_id:      Option<u16>,  // None = match any (auto)
    #[serde(default)] pub product_id:     Option<u16>,  // None = match any (auto)
    #[serde(default)] pub usage_page:     Option<u16>,  // default 0xFF60 at use site
    #[serde(default)] pub usage:          Option<u16>,  // default 0x61 at use site
    #[serde(default = "default_debounce_ms")]      pub debounce_ms: u64,      // 50
    #[serde(default = "default_poll_interval_ms")] pub poll_interval_ms: u64, // 0
}
```

**Every device-identifying field is `Option` with `#[serde(default)]`**, so:
- A **missing** field deserializes to `None` ⇒ "match any" / default-at-use-site.
- An **empty** (`""`) / brand-new config file is valid (all `None`).
- **Legacy** files that set `vendor_id = 0xfeed` keep working (become `Some(0xfeed)`).

`#[derive(Default)]` gives `Config::default()` = all `None` + timing defaults.

### 1.1 Field reference

| Key | Type | Default | Meaning |
|---|---|---|---|
| `vendor_id` | `Option<u16>` (hex in TOML) | `None` | USB vendor ID. Set only to disambiguate multiple QMK keyboards. |
| `product_id` | `Option<u16>` (hex in TOML) | `None` | USB product ID. Set only to disambiguate. |
| `usage_page` | `Option<u16>` | `0xff60` (resolved at use site) | HID usage page. Set only if firmware overrode `RAW_USAGE_PAGE`. |
| `usage` | `Option<u16>` | `0x61` (resolved at use site) | HID usage. Set only if firmware overrode `RAW_USAGE_ID`. |
| `debounce_ms` | `u64` | `50` | Burst-coalescing window (ms). `0` disables debouncing (every change sends immediately). |
| `poll_interval_ms` | `u64` | `0` | (Hyprland only) periodic active-window poll cadence (ms). `0` = rely on IPC events. |

> TOML hex literals (`0xfeed`) are supported by `toml`'s integer parsing and
> deserialize into `u16` directly.

### 1.2 Resolution at use sites
- `configured_filter()` (`core/notifier.rs`) resolves `usage_page`/`usage` to
  `qmk_notifier::DEFAULT_USAGE_PAGE`/`DEFAULT_USAGE` when `None`. VID/PID stay
  `Option` (auto-discovery).
- `configured_timing()` resolves `debounce_ms`/`poll_interval_ms` to defaults
  when the file is missing or the field is unset.

**Hot config:** both are re-read from disk on **every** notification and every
status poll, so editing the file (or saving the Settings dialog) takes effect
within ~3 s — no restart.

---

## 2. The Shared Config-Body Renderer (`render_config_body`)

```rust
pub fn render_config_body(vendor_id: Option<u16>, product_id: Option<u16>) -> String
```

The **single** function every write path uses (CLI `create_default_config`, the
Win32 dialog, the NSAlert dialog, the zenity/GTK Linux dialog). Guarantees the
file format never drifts. Output:

```toml
# QMKonnect Configuration
#
# All fields are OPTIONAL. By default QMKonnect auto-discovers any QMK
# keyboard using the standard Raw HID usage page (0xFF60 / 0x61). Set
# vendor_id/product_id only to disambiguate among multiple QMK
# keyboards, or usage_page/usage to target a board that overrode
# RAW_USAGE_PAGE/RAW_USAGE_ID in its firmware.
#
# usage_page = 0xff60
# usage      = 0x61
#
# Debounce window (ms) for coalescing rapid window-change bursts before
# sending to the keyboard. 0 disables debouncing entirely. Default 50.
# debounce_ms = 50
#
# (Hyprland only) periodic active-window poll interval (ms).
# 0 disables. Default 0.
# poll_interval_ms = 0

vendor_id  = 0xfeed          # OR:  "# vendor_id  = 0xfeed   # unset: auto-discovery"
product_id = 0x0000          # OR:  "# product_id = 0x0000   # unset: auto-discovery"
```

A value is **explicit** when `Some` (uncommented), **commented-out** when `None`
("auto-discovery"). Timing/usage options are always shown as commented hints.

---

## 3. Config Paths (per-OS)

| OS | Primary | Secondary | System-wide |
|---|---|---|---|
| **Linux** | `$XDG_CONFIG_HOME/qmk-notifier/config.toml` | `~/.config/qmk-notifier/config.toml` | `/etc/qmk-notifier/config.toml` |
| **Windows** | `%APPDATA%\QMKonnect\config.toml` | `%LOCALAPPDATA%\QMKonnect\config.toml` | (exe dir fallback) |
| **macOS** | `~/Library/Application Support/QMKonnect/config.toml` | `~/.config/qmk-notifier/config.toml` (XDG) | `/etc/qmk-notifier/config.toml` |

> Linux preserves the historical `qmk-notifier/` dir name so existing installs
> keep working. Windows/macOS use `QMKonnect/`. The macOS XDG + `/etc` fallbacks
> exist so a config written on one platform can be found on another.

`create_config_dir()` returns the primary dir (creating it). The Settings
dialogs call it before writing, so the directory always exists.

> **Host-side `rules.toml`:** lives in the **same
directory** as `config.toml` (e.g. `~/.config/qmk-notifier/rules.toml`,
`%APPDATA%\QMKonnect\rules.toml`,
`~/Library/Application Support/QMKonnect/rules.toml`). Absent ⇒ host rules
disabled (string-only). Schema: `HOST_RULES.md` §9.

---

## 4. CLI Reference (`src/main.rs`)

```
qmkonnect [OPTIONS]

Options:
  -h, --help            Display this help message
  -v, --verbose         Enable verbose logging
  -c, --config          Create a default (commented-out) configuration file
  -r, --reload          Reload configuration and update system files (Linux)
      --config <path>   Config file to use with --reload
      --user <name>     Invoking user for sudo'd --reload (Linux)
      --uid <n>         Invoking uid for sudo'd --reload (Linux)
  -l, --list            List supported platforms (this build)
      --list-devices    List connected HID devices (VID/PID discovery)
      --show-window-info  [macOS/Windows] open the Window Information dialog directly
      --tray-app          [Windows] run as tray app (default)
      --console           [Windows] allocate a console and run for debugging
      --list-callbacks      handshake → print the keyboard's callback name→id table
      --validate-rules      parse rules.toml; report schema/callback-name errors
      --rules-path <path>   override the rules.toml location
```

Running with **no options** starts the notifier service (the tray app on
Windows/macOS, the monitor on Linux).

### 4.1 Flag semantics
- `-h/--help`, `-v/--verbose` — boolean flags scanned from `env::args()`.
- `-c/--config` — `create_config()`: `create_config_dir()` then
  `create_default_config(<dir>/config.toml)` (no-op + message if it exists).
- `-r/--reload` — `reload_config(verbose, config, user, uid)`:
  - Resolves the config path (Linux: **root-aware** `resolve_config_for_reload`,
    §`SPEC_LINUX.md` 4.4; else `get_config_path()`).
  - `parse_config`, read VID/PID as `Option<u16>`.
  - Linux: `update_udev_rules(vid, pid, verbose)` (writes/repairs/purges the
    fallback rule — no-ops cleanly when both unset) + `reload_udev_rules()`.
  - Prints "Configuration reloaded successfully."
- `--config`/`--user`/`--uid` — value flags parsed by `parse_value_flag`
  (accepts `--flag value` and `--flag=value`). Only meaningful with `--reload`
  on Linux.
- `--list-devices` — `core::notifier::list_devices()` (read-only HID enumerate;
  prints `vid:pid  page:usage  product`).
- `--show-window-info` — `platforms::list_foreground_windows()` then the
  platform dialog (macOS/Windows only). A debug aid to test the dialog in
  isolation.
- `--tray-app`/`--console` — Windows-only runner modes.

### 4.2 Logging
- **Windows:** `init_logging()` tries `eventlog::init("QMKonnect", Info)`
  (Windows Event Log, source `"QMKonnect"`) first, falls back to `env_logger`
  (console). `-v` from a terminal prints there.
- **macOS/Linux:** `eprintln!`/`println!` (verbose timestamps via
  `core::now_ms()`, a process-local monotonic epoch — not wall-clock).

---

## 5. Parsing Robustness
- `parse_config` is a straight `toml::from_str` — no normalization/validation
  beyond what TOML + serde provide. Unknown keys are ignored by serde default
  (forward-compatible).
- Hex field parsing in the UI (`parse_id_field`/`parse_id`): trims, accepts
  optional `0x`/`0X`, empty or literal `"auto"` ⇒ `None`, else
  `u16::from_str_radix(_, 16)`. Garbage ⇒ `Err` surfaced in the dialog.

---

## 6. Config UX per Platform (summary; full detail `SPEC_UI.md` §2)
- **Windows/macOS:** tray → Settings → native dialog → writes `config.toml`
  via `render_config_body`.
- **Linux:** tray → Settings… → `zenity --forms` → `write_config` +
  `apply_device_rule` (pkexec install). Or edit the file + `sudo qmkonnect -r`.

---

*Continue with `SPEC_PACKAGING.md`.* | TOML schema, defaults, render body, config paths per OS, CLI flag reference. |
| # SPEC — Build, Packaging & Release

> Companion to `PRD.md`. Cargo build profile, the per-platform installers (Inno
> Setup / Arch PKGBUILD / macOS DMG), the CI release workflow, code signing, and
> the committed dev test loop. Covers `Cargo.toml`, `.cargo/config.toml`,
> `release.toml`, `.github/workflows/release.yml`, and `packaging/`.

---

## 1. Cargo Build Profile

`Cargo.toml` `[profile.release]` (optimize for size):
```toml
opt-level   = "z"     # size
lto         = true
codegen-units = 1
panic       = "abort"   # no unwind; systemd Restart=always recovers crashes
strip       = true
```

`.cargo/config.toml` (Windows MSVC only):
```toml
[target.'cfg(all(target_os = "windows", target_env = "msvc"))']
rustflags = ["-C", "target-feature=+crt-static"]
```
⇒ statically links UCRT + vcruntime → **no Visual C++ Redistributable**
dependency on Windows (cost: ~135 KB larger exe).

`src/main.rs` top: `#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]`
⇒ no console window on Windows.

**MSRV Rust 1.88** (`rust-version` in `Cargo.toml`; image 0.25.x is the floor).

---

## 2. Features & Binaries

```toml
[features]
default    = ["hyprland", "macos", "linux-tray"]
hyprland   = ["dep:hyprland"]
macos      = ["dep:objc", "dep:core-foundation", "dep:core-graphics", "dep:dispatch"]
linux-tray = ["dep:ksni", "dep:gtk"]

[[bin]] name = "qmkonnect"        path = "src/main.rs"
[[bin]] name = "qmkonnect-hid-id" path = "src/bin/hid_id.rs"   # pure std; udev helper
```

- Plain `cargo build --release` produces the **full app with a tray** on every OS
  (off-platform features are inert no-ops).
- `--no-default-features` yields the minimal trayless service build.
- **Linux Arch build** links `-lhidapi-hidraw` (not `-lhidapi-libusb`) so
  usage/usage_page matching works: `RUSTFLAGS="-C link-arg=-lhidapi-hidraw" cargo build --release`.

---

## 3. Windows Packaging

### 3.1 The shipped installer — Inno Setup (per-user, no admin)
`packaging/windows/inno/QMKonnect.iss` → `QMKonnect-Setup.exe` (built by
`packaging/windows/inno/build.ps1`; needs `winget install JRSoftware.InnoSetup`).

- **Per-user:** `PrivilegesRequired=lowest`, `DefaultDirName={localappdata}\Programs\QMKonnect`,
  `DisableDirPage=yes` (fixed location). `AppId={{FAAE1F7A-...}}` is the stable
  upgrade identity (constant across versions).
- **Files:** `qmkonnect.exe`, `Icon.ico`, `IconTray-dark.png` → `{app}`.
- **`[Registry]`** writes the **HKCU `Run` value `"QMKonnect"`** (default-on
  autostart; `uninsdeletevalue`). This is the **single source of truth** shared
  with `src/autostart.rs` and `install.ps1` — keep the value name identical.
- **`[Icons]`** Start Menu shortcut (manual launch).
- **`[Code] KillRunningInstance`** in `InitializeSetup`/`InitializeUninstall`:
  `taskkill /IM qmkonnect.exe /F /T` so the single-instance mutex releases and
  the exe can be overwritten.
- **`[Run]`** launches the app after an *interactive* install (`skipifsilent`
  avoids a tray-less background process on `/VERYSILENT`).
- Version injected from `Cargo.toml` (`#define MyAppVersion`).

### 3.2 `install.ps1` / `uninstall.ps1` (the PowerShell equivalent)
`install.ps1`: stops any running instance, copies exe + icon assets to
`%LOCALAPPDATA%\Programs\QMKonnect`, Start Menu `.lnk`, writes the HKCU `Run`
value, registers an Add/Remove-Programs uninstall entry (DisplayName/Version/
Publisher/InstallLocation/UninstallString), launches the app.

`uninstall.ps1`: `Remove-ItemProperty … Run -Name QMKonnect`, removes the
install dir + shortcuts.

### 3.3 The legacy WiX MSI (Session-0 service) — NOT shipped
`packaging/windows/installer.wxs` + `build-installer.ps1` (needs WiX v3) build
an MSI that installs a **Session-0 service**. A service **cannot** show a tray
icon in the interactive session, so this is the wrong vehicle for the tray app.
It remains as a legacy build path only; the **tray app + Inno installer is what
ships**. (CI's `windows` job currently uses the WiX path — flag for the dev
agent to reconcile with the Inno path.)

### 3.4 Runtime dependencies
**None.** The release binary statically links the C runtime (`+crt-static`), so
`QMKonnect-Setup.exe` runs on any clean Windows 10/11 x64 machine. Toolchain
prereq: **Visual Studio Build Tools** with *Desktop development with C++*
(MSVC + Windows SDK); use the default `stable-x86_64-pc-windows-msvc` host, not
`gnu`, or the `windows`-crate link step fails.

---

## 4. Linux Packaging

### 4.1 Arch PKGBUILD (`packaging/linux/arch/`)
- `build()`: `RUSTFLAGS="-C link-arg=-lhidapi-hidraw" cargo build --release`
  (builds both `qmkonnect` and `qmkonnect-hid-id`).
- `package()` installs:
  - `qmkonnect` → `/usr/bin/qmkonnect`
  - `qmkonnect-hid-id` → `/usr/lib/udev/qmkonnect-hid-id`
  - `69-qmkonnect-rawhid.rules` → `/usr/lib/udev/rules.d/`
  - `qmkonnect.service.template` → `/usr/lib/systemd/user/` (instantiated by `post_install`)
- `depends=('systemd' 'hidapi' 'libusb' 'zenity' 'libnotify')`.
- `backup=("usr/lib/systemd/user/qmkonnect.service.template")` — only the
  (user-instantiated) template is preserved across upgrades; the static rule
  and helper are package-owned; the on-demand `99-qmkonnect.rules` is user-generated.
- `options=(!strip)`.

### 4.2 `qmkonnect.install` (pacman hooks)
- `post_install`: instantiate the service template; `udevadm control --reload-rules && udevadm trigger`;
  `systemctl --global enable qmkonnect.service`; print zero-config next-steps.
- `post_upgrade`: re-instantiate the template + reload udev. **Does not** call
  `qmkonnect --reload` (needs root + a config that may not exist yet).
- `post_remove`: `systemctl --global disable`; stop+disable per-user services;
  `rm -f /etc/udev/rules.d/99-qmkonnect.rules` + the instantiated service; reload udev.

### 4.3 Other distros (binary install)
Install the binary + the static rule + helper + (optional) service template by
hand — documented in `docs/installation.md`:
```bash
cargo build --release
sudo install -m755 target/release/qmkonnect        /usr/local/bin/qmkonnect
sudo install -m755 target/release/qmkonnect-hid-id /usr/lib/udev/qmkonnect-hid-id
sudo install -m644 packaging/linux/udev/69-qmkonnect-rawhid.rules /usr/lib/udev/rules.d/
sudo udevadm control --reload && sudo udevadm trigger
```

---

## 5. macOS Packaging

### 5.1 `packaging/macos/build.sh`
- `cargo build --release`.
- Assembles `QMKonnect.app/Contents/{MacOS/qmkonnect, Resources/{Icon.icns, IconTemplate.png}}`.
- Generates `Info.plist`:
  ```xml
  CFBundleExecutable    = qmkonnect
  CFBundleIdentifier    = io.mulletware.qmkonnect
  CFBundleName          = QMKonnect
  CFBundleIconFile      = Icon.icns
  LSUIElement           = true        # menu-bar-only: no Dock, no CMD-Tab
  ```
- **Codesign:** `codesign --deep --force --sign "$CODESIGN_IDENTITY"` where
  `$CODESIGN_IDENTITY` defaults to `-` (ad-hoc). For distribution, set
  `CODESIGN_IDENTITY="Developer ID Application: … (TEAMID)"` for a stable,
  TCC-persistent signature.
- Builds `QMKonnect.dmg` (UDZO) with an `/Applications` symlink.

### 5.2 `clean.sh` — run BEFORE every reinstall
The #1 cause of "I rebuilt but nothing changed":
1. `pkill -f QMKonnect.app`.
2. Eject any mounted `QMKonnect` DMGs.
3. `lsregister -u` stale copies (`/Applications`, `~/.Trash`).
4. `rm -rf` old bundles.
5. `tccutil reset ScreenCapture io.mulletware.qmkonnect` (ad-hoc `cdhash`
   changes every build → TCC re-prompts even though Settings shows it granted).

### 5.3 `install.sh` / `uninstall.sh`
- `install.sh`: mount the DMG, copy `QMKonnect.app` to `/Applications`.
- `uninstall.sh`: remove the app, the **Launch at Login** `SMAppService` entry,
  and per-user config.

### 5.4 Test via `open /Applications/QMKonnect.app`
**Never** test the menu bar by running `target/release/qmkonnect` directly —
outside a real bundle the menu-bar icon and template path don't work. The raw
binary is fine for CLI subcommands.

---

## 6. The Dev Test Loop (`AGENTS.md`)

### 6.1 macOS
```bash
cargo test --bin qmkonnect -- --test-threads=1   # shared debouncer state
cd packaging/macos && ./clean.sh && ./build.sh && ./install.sh
open /Applications/QMKonnect.app                  # grant the one Screen-Recording prompt
```
If the icon looks dimmed/unclickable, the main thread is wedged →
`sample <pid> 2 | grep -i mutex` (healthy = `nextEventMatchingMask`). Always
rebuild before sampling (a stale binary misleads).

### 6.2 Windows (PowerShell)
```powershell
cargo test --bin qmkonnect -- --test-threads=1
cargo build --release
taskkill /IM qmkonnect.exe /F     # mandatory — single-instance mutex
.\target\release\qmkonnect.exe     # run in YOUR session, never via sc/services.msc
```
Exclude `target\`, `~/.cargo`, and the project dir from Windows Defender
(real-time scanning makes builds crawl). The Inno installer:
`powershell -NoProfile -ExecutionPolicy Bypass -File packaging\windows\inno\build.ps1`
→ `packaging\windows\inno\Output\QMKonnect-Setup.exe`.

### 6.3 Linux
```bash
cargo test --bin qmkonnect -- --test-threads=1
cargo build --release                       # builds qmkonnect + qmkonnect-hid-id
cargo clippy --all-targets -- -D warnings
# package: cd packaging/linux/arch && makepkg -f && sudo pacman -U qmkonnect-*.pkg.tar.zst
```

---

## 7. CI Release (`.github/workflows/release.yml`)

**Triggers:** push of a `v*` tag (builds **and** publishes) or `workflow_dispatch`
(builds **without** publishing — dry-run the whole pipeline and download real
artifacts before cutting a tag).

- `qmk_notifier` is a pinned git dep (`tag = "vX.Y.Z"`), so a plain
  `actions/checkout` of this repo suffices.
- **macOS job:** `cargo build`, `packaging/macos/build.sh`. If repo var
  `ENABLE_MACOS_NOTARIZE=true` + `APPLE_*` secrets: import Developer ID cert,
  set `CODESIGN_IDENTITY`, then `notarytool submit … --wait` + `stapler staple`.
  Renames `QMKonnect-<ver>-macos.dmg`, uploads artifact.
- **Windows job:** `cargo build`, install WiX v3, `build-installer.ps1` → MSI.
  *(Note: the shipped end-user artifact is the Inno `QMKonnect-Setup.exe`; the
  CI Windows job currently builds the WiX MSI. A dev agent should reconcile
  this — run `packaging/windows/inno/build.ps1` to produce the setup exe and
  upload that as the primary artifact.)*
- **Linux job:** Arch build via `makepkg`/docker → `.pkg.tar.zst` + standalone
  binary.
- Version is read from `cargo metadata` (single source of truth in `Cargo.toml`);
  installer/PKGBUILD versions are injected from it (no `pre-release-replacements`).

---

## 8. Release Chore (`release.toml` + `cargo-release`)

QMKonnect is a **binary app**, never published to crates.io (`publish = false`).
`cargo release <level>` (e.g. `cargo release 0.3.0`):
1. bumps `version` in `Cargo.toml` (+ `Cargo.lock`),
2. commits the bump,
3. creates an annotated `v<version>` tag,
4. pushes commit + tag to `origin`.

Pushing the tag triggers `release.yml`. **Nothing tags or publishes on its own**
— the maintainer controls *when* a release happens. Releases cut from `main`
(`allow-branch = ["main"]`).

---

## 9. Build Outputs (gitignored, never commit)
`target/`, `QMKonnect.app/`, `*.dmg`, `*.msi`, `*.exe` installers,
`arch/pkg/`, `docs/_site/`. Regenerated by the build scripts.

---

*Continue with `SPEC_FIRMWARE.md`.* | Cargo build profile, per-platform installers (Inno/PKGBUILD/DMG), CI release workflow, code signing, the dev test loop. |
| # SPEC — Firmware Integration (qmk-notifier module)

> Companion to `PRD.md` / `SPEC_PROTOCOL.md`. The **keyboard-side** contract: the
> `qmk-notifier` C module (companion repo `dabstractor/qmk-notifier`), how a
> user's keymap integrates it, the pattern-matching syntax, and the reference
> keymap this PRD was validated against. QMKonnect the desktop app does **not**
> implement any of this — it is documented here so a dev agent understands the
> complete end-to-end system and the strings the desktop must produce.

---

## 1. The Module at a Glance

`qmk-notifier` (hyphen) is a QMK **module** (a git submodule under a keyboard
directory). It provides:
- **`notifier.c`** — receives Raw HID, validates the magic header, reassembles
  multi-report messages, sanitizes, and dispatches to the user's maps.
- **`notifier.h`** — the `command_map_t` / `layer_map_t` structs and the
  `DEFINE_SERIAL_COMMANDS` / `DEFINE_SERIAL_LAYERS` / `WT(...)` macros.
- **`pattern_match.c/.h`** — wildcard + anchor + escape-sequence matcher.
- **`rules.mk`** — the single line that wires it in:
  ```make
  RAW_ENABLE = yes
  SRC += qmk-notifier/notifier.c
  ```

The module is **coexistence-safe**: it inspects only messages beginning with the
magic bytes `0x81 0x9F` and ignores everything else, so other Raw HID modules
(e.g. `qmk-field-kit`) can share the same interface.

> **Canonical firmware spec.** This document is the *desktop-facing* view of the
firmware contract. The firmware repo's `PRD.md`
([`dabstractor/qmk-notifier`](https://github.com/dabstractor/qmk-notifier)) is
**authoritative** — including per-OS `DEFINE_SERIAL_*_OS` maps (selected by the
detected host OS) and the typed-command namespace
§4.6: `QUERY_INFO` / `QUERY_CALLBACK` / `SET_OS` / `APPLY_HOST_CONTEXT` with the
`clear_board` flag, the `host_layer` tracker, and `DEFINE_HOST_CALLBACKS`).
Where this file and the firmware `PRD.md` disagree, the firmware wins.

---

## 2. Integration Steps (the user's keymap)

### Step 1 — add the submodule
```bash
cd <qmk_firmware>/keyboards/<your_keyboard>   # e.g. handwired/dactyl_manuform/5x7_1
git submodule add https://github.com/dabstractor/qmk-notifier.git qmk-notifier
```

### Step 2 — include the module's `rules.mk` (in your keymap's `rules.mk`)
```make
include keyboards/handwired/<manufacturer>/<keyboard>/qmk-notifier/rules.mk
```
That single line enables `RAW_ENABLE` **and** compiles `notifier.c`. Do **not**
hand-write `SRC += lib/...` or point at a non-existent `qmk_notifier.c` — that
fails to link.

> The reference keymap (`keyboards/handwired/dactyl_manuform/5x7_1/rules.mk`)
> pulls in three modules this way:
> ```make
> include keyboards/handwired/dactyl_manuform/5x7_1/qmk-vim/rules.mk
> include keyboards/handwired/dactyl_manuform/5x7_1/qmk-notifier/rules.mk
> include keyboards/handwired/dactyl_manuform/5x7_1/qmk-field-kit/rules.mk
> SERIAL_DRIVER = vendor
> ```
> (The `field_kit_process_message` call in `raw_hid_receive` below is from
> qmk-field-kit — a separate module sharing the interface.)

### Step 3 — wire `raw_hid_receive` (in your `keymap.c`)
```c
#include QMK_KEYBOARD_H
#include "./qmk-notifier/notifier.h"

void raw_hid_receive(uint8_t *data, uint8_t length) {
    hid_notify(data, length);   // qmk-notifier entry point
    // (other Raw HID modules can be tried first/after; qmk-notifier
    //  ignores anything not starting with 0x81 0x9F)
}
```

The reference keymap does both field-kit and notifier:
```c
void raw_hid_receive(uint8_t *data, uint8_t length) {
    field_kit_process_message(data, length);
    hid_notify(data, length);
}
```

### Step 4 — define your rules (anywhere `#include`-d from `keymap.c`)

Using the two macros (full syntax in §3):
```c
DEFINE_SERIAL_LAYERS({
    { "*calculator",           _NUMPAD },
    { WT("*chrome*", "*jitsi*"), _JITSI },
    { WT("tty$", "^terminal$"),  _TERMINAL },
    { "steam_app*",            _GAMING },
});
DEFINE_SERIAL_COMMANDS({
    { "neovide", &disable_vim },
    { WT("*chrome*", "*claude*"), &vim_lazy_insert, &disable_vim },
});
```

### Step 5 — build & flash
```bash
qmk compile -kb <your_keyboard> -km <your_keymap>
qmk flash   -kb <your_keyboard> -km <your_keymap>
```
**QMKonnect cannot communicate with the keyboard until this firmware is flashed.**

---

## 3. The Module API (macros & structs)

From `notifier.h`:
```c
typedef void (*callback_t)(void);

typedef struct {
    const char *pattern;
    callback_t on_enable;
    callback_t on_disable;      // may be NULL
    const bool case_sensitive;
} command_map_t;

typedef struct {
    const char *pattern;
    const int layer;
    const bool case_sensitive;
} layer_map_t;

#define GS_DELIMITER      "\x1D"                 // ASCII 31 (Group Separator)
#define ETX_TERMINATOR    "\x03"                 // ASCII 3 (End of Text)
#define WINDOW_TITLE(classname, title)  classname GS_DELIMITER title
#define WT(...) WINDOW_TITLE(__VA_ARGS__)

#define DEFINE_SERIAL_COMMANDS(...)   /* defines user_command_map[] + getters */
#define DEFINE_SERIAL_LAYERS(...)     /* defines user_layer_map[] + getters   */
```

### 3.1 `DEFINE_SERIAL_LAYERS({ … })`
An array of `{ pattern, layer, case_sensitive }`. On a match, the matched layer
is activated (the previously-activated notifier layer is deactivated first, so
only one notifier layer is active at a time).

### 3.2 `DEFINE_SERIAL_COMMANDS({ … })`
An array of `{ pattern, on_enable, on_disable, case_sensitive }`. The 4th field
(`case_sensitive`) is **optional** in the layer macro but the command struct
declares it; the example keymaps omit it in some rows (aggregate-init zero-fills
→ `false`/NULL). On a match, `on_enable()` runs; the previous command's
`on_disable()` (if any) runs first. `on_disable` may be `NULL`.

### 3.3 `WT(class, title)` / `WINDOW_TITLE(class, title)`
Expands to the literal `class "\x1D" title` — i.e. a pattern containing the GS
delimiter. The matcher then requires **both** halves to match against the
class and title respectively. A bare pattern (no `WT`) matches only the
`application_class` part of the message.

---

## 4. Pattern-Matching Syntax (`pattern_match.c`)

`bool pattern_match(const char *pattern, const char *str, bool case_sensitive)`:

| Construct | Meaning |
|---|---|
| `*` | Wildcard — any sequence (including empty). Combinable with anchors. |
| `^` at start | Anchor to the beginning of the string. |
| `$` at end | Anchor to the end of the string. |
| `^…$` together | Exact full-string match. |
| `\^` `\$` `\*` `\\` | Literal escaped character. |
| `\d \D` | Digit / non-digit. |
| `\w \W` | Word char / non-word char. |
| `\s \S` | Whitespace / non-whitespace. |
| `\b \B` | Word boundary / non-boundary. |
| `.` | Any char except newline. |

No anchors ⇒ **substring** match (backward-compatible). Case sensitivity is per
-row (the `case_sensitive` field).

### 4.1 The delimiter-aware matcher (`match_pattern` in `notifier.c`)
- If the **pattern** has a GS delimiter but the **message** doesn't (or vice
  versa), matching is done on the appropriate side only.
- If both have it, both halves must match (`pattern_left` vs `msg_left` AND
  `pattern_right` vs `msg_right`).
- First-match-wins in each map (scan order = definition order).

---

## 5. Firmware Reception Flow (`hid_notify` → `process_full_message`)

This is what runs on the MCU for every Raw HID report QMKonnect sends (full
byte detail in `SPEC_PROTOCOL.md` §5):

1. **Guard:** `length < 2 || data[0] != 0x81 || data[1] != 0x9F` ⇒ discard.
2. Strip the 2 header bytes; iterate the rest into the static 256-byte
   `msg_buffer` until an **ETX** (`0x03`):
   - On ETX: NUL-terminate, `sanitize_string` (strip non-ASCII/non-essential),
     reset index, call `process_full_message(buffer)`, break.
   - On overflow (`msg_index >= 255`): reset index (drop the message).
3. `process_full_message`:
   - Always `disable_command()` first (run the previous command's `on_disable`).
   - Scan `command_map` (first match) → remember; scan `layer_map` (first match)
     → remember.
   - `deactivate_layer()` (the previous notifier layer).
   - If a command matched: `enable_command(cmd)`.
   - If a layer matched: `activate_layer(layer)` (`layer_on`).
4. **Ack:** `raw_hid_send(response[32])` with `response[0] = (match ? 1 : 0)`.
   (QMK silently drops this today due to the `length == RAW_EPSIZE` guard — see
   `SPEC_PROTOCOL.md` §2.5.)

**Key invariants:**
- Only **one** notifier layer is active at a time (the previous is always
  deactivated before a new one activates, and an unmatched message deactivates).
- `sanitize_string` keeps only printable ASCII (32–126) plus tab/newline/CR/GS/ETX.
  So any non-ASCII in a window title (emoji, accented chars) is stripped before
  matching — patterns should be ASCII.

---

## 6. The Reference Keymap (validated against this PRD)

A real-world keymap — the maintainer's Dactyl-Manuform 5×7 (RP2040, split,
`SERIAL_DRIVER = vendor`), in a `<keyboard>/keymaps/default/` directory alongside
a `serial_command.c` — is the canonical example of both macros in real use:

```c
DEFINE_SERIAL_COMMANDS({
    { "neovide", &disable_vim },
    { WT("*tty$", "^terminal$"), &disable_vim },
    { WT("*tty$", "*tty"), &disable_vim },
    { "*iterm*", &disable_vim },
    { WT("^Claude$", "^Claude$"), &vim_lazy_insert, &disable_vim }, // claude desktop
    { WT("*chrome*", "*claude*"), &vim_lazy_insert, &disable_vim }, // claude.ai
    { WT("*chrome*", "*chatgpt*"), &vim_lazy_insert, &disable_vim },
    { WT("*chrome*", "*deepseek*"), &vim_lazy_insert, &disable_vim },
    { WT("*chrome*", "*gemini*"), &vim_lazy_insert, &disable_vim },
    { WT("^brave-browser$", "*Claude - Brave$"), &vim_lazy_insert, &disable_vim },
    { WT("^brave-browser$", "*ChatGPT - Brave$"), &vim_lazy_insert, &disable_vim },
    { WT("^brave-browser$", "*Deepseek - Brave$"), &vim_lazy_insert, &disable_vim },
    { WT("^brave-browser$", "gemini*"), &vim_lazy_insert, &disable_vim },
    { WT("^brave-browser$", "*ai*studio*"), &vim_lazy_insert, &disable_vim },
    { WT("^brave-browser$", "^zoho mail"), &vim_lazy_insert, &disable_vim },
    { "Mulletware Wiki", &vim_lazy_insert, &disable_vim },
    { WT("*", "*orderlands*"), &disable_vim },
    { WT("steam_app*", "*"), &disable_vim },
    { WT("cs2", "Counter-Strike 2"), &disable_vim },
});

DEFINE_SERIAL_LAYERS({
    { "*calculator", _NUMPAD },
    { WT("*chrome*", "*jitsi*"), _JITSI },
    { WT("tty$", "^terminal$"), _TERMINAL },
    { WT("tty$", "tty"), _TERMINAL },
    { "*iterm*", _TERMINAL },
    { WT("*alacritty*", "*matterhorn*"), _MATTERHORN },
    { "*clickup*", _CLICKUP },
    { "*neovide*", _NEOVIM },
    { "chrome*", _BROWSER },
    { WT("brave-browser", "*"), _BROWSER },
    { WT("firefox", "*"), _BROWSER },
    { WT("org.gnome.Nautilus", "*"), _BROWSER },
    { "*inkscape*", _INKSCAPE },
    { "blender", _BLENDER },
    { "borderlands*", _GAMING },
    { WT("steam_app*", "*orderlands*"), _GAMING },
    { "steam_app*", _GAMING },
    { WT("cs2", "Counter-Strike 2"), _GAMING },
});
```

**What this demonstrates** (and what the desktop app must therefore produce):
- Bare patterns match the **class** alone: `"chrome*"`, `"blender"`,
  `"*neovide*"`, `"steam_app*"`.
- `WT(class, title)` matches both: `WT("brave-browser", "*Claude - Brave$")`.
- Anchors for precision: `WT("^Claude$", "^Claude$")` (exact class+title for the
  Claude desktop app, so a browser tab titled "Claude" doesn't trip it).
- Case sensitivity is off by default (`"Counter-Strike 2"` matches case-insensitively).
- Commands and layers are **independent** — a window can match both a command
  (toggle vim) and a layer (switch keymap) simultaneously.

### 6.1 Hardware (`keyboard.json`)
Dactyl-Manuform 5×7-1, manufacturer `dabstractor`, MCU RP2040 (`bootloader
rp2040`), split with `SERIAL_DRIVER = vendor`, features include `raw_hid`,
`encoder_map`, `tri_layer`, `caps_word`, `leader`, `nkro`, `os_detection`,
`console`. The user also has `qmk-vim` and `qmk-field-kit` modules integrated.

---

## 7. Desktop ↔ Firmware Contract Summary

For QMKonnect to drive this keymap, it must, on every focus change, send a Raw
HID burst whose reassembled logical message is exactly:

```
0x81 0x9F  <class bytes…>  0x1D  <title bytes…>  0x03
```

where `<class>` is one of the strings the keymap matches (`neovide`, `firefox`,
`brave-browser`, `steam_app12345`, …) and `<title>` is the window title (or
empty). Everything in `SPEC_PROTOCOL.md` exists to produce exactly that. The
"Show Window Information" dialog (`SPEC_UI.md` §3) exists so users can see the
exact `<class>`/`<title>` to put in their `DEFINE_SERIAL_*` rules.

---

## 8. Debugging the Firmware Side
- No built-in debug callback. Add your own `printf` inside a callback (or
  temporarily inside `hid_notify`) with `CONSOLE_ENABLE = yes`, then `qmk console`.
- On the desktop side, `qmkonnect -v` prints the sanitized payload (`\x1D`
  shown as `|`) and send timing, confirming what's on the wire.

---

*This concludes the specification set. Return to `PRD.md` for the product-level
overview and the document map.* | The `qmk-notifier` firmware module contract, keymap integration steps, pattern-matching syntax, the user's reference keymap. |
| # SPEC — Host-Side Window Rules & Callbacks

> Companion to `PRD.md`. Design for the feature that moves app→layer and
> app→callback matching onto the host so
> rules can change **without reflashing**. Host rules **stack on top of** the
> board's `DEFINE_*` rules (board first, host on top; board callbacks first,
> host callbacks after). Read alongside `PROTOCOL.md` (wire framing),
> `FIRMWARE.md` (the qmk-notifier module), `CONFIG.md` (config schema), and
> `ARCHITECTURE.md` (the notification pipeline). Spans **three repos**: the
> `qmk_notifier` Rust crate, the `qmk-notifier` firmware, and `qmkonnect`.
> Registered in the PRD feature table (F11/F12), §12 Future Work, and the
> Document Map.

---

## 1. Goal & Deliverables

**Goal.** Let users define **app → layer** and **app → callback** rules in an
editable file on their computer (`rules.toml`), with the matching done by
QMKonnect on the host — so rules change **without reflashing firmware**. Both
layer switching *and* arbitrary firmware callbacks (the existing `command_map`
`on_enable`/`on_disable` pattern) are supported. Host rules **stack on top of**
the keyboard's existing firmware rules (`DEFINE_SERIAL_LAYERS` /
`DEFINE_SERIAL_COMMANDS`): the board's rules always run first, then host rules
apply on top. Nothing existing is removed or deprecated.

**Deliverables.**

- **`qmk_notifier` crate:** typed-command framing + response parsing, and new
  `RunCommand` variants. (Transport-only — the matcher is NOT here; it lives in
  `qmkonnect`.)
- **`qmk-notifier` firmware:** a named callback registry
  (`DEFINE_HOST_CALLBACKS`), a separate host-layer tracker, host-callback enable
  state, typed-command dispatch inside `hid_notify()`, and handlers for
  `QUERY_INFO` / `QUERY_CALLBACK` / `APPLY_HOST_CONTEXT`.
- **`qmkonnect`:** `rules.toml` parsing + validation, host-side rule evaluation,
  a startup capability/name handshake, `notify_qmk` extended to send the host
  context after the legacy string, CLI flags (`--list-callbacks`,
  `--validate-rules`), a "Reload rules" tray item, and per-platform rules-file
  paths.
- **Docs:** updates to `docs/qmk-integration.md`, `docs/configuration.md`,
  `docs/examples.md`, `docs/troubleshooting.md`, `Readme.md`, and a regenerated
  `docs/llms_full.txt`, plus the migration subsection (§10).

**Success definition.**

- A user can add/change a layer or callback rule by editing `rules.toml` and
  clicking "Reload rules" — **no reflash**.
- Board (`DEFINE_*`) rules keep working unchanged; host rules apply on top in the
  documented order (board layer first → host layer on top; board callbacks first
  → host callbacks after).
- Old firmware (current release) + new QMKonnect continues to work in
  string-only mode (graceful fallback via the handshake); no host commands are
  sent to firmware that doesn't advertise support.
- New firmware + old QMKonnect keeps working (old app only sends the legacy
  string; new typed commands are simply never sent).
- All existing tests pass; new unit/integration tests cover matcher parity, the
  state machine, handshake fallback, and wire framing.

## 2. Context: How It Works Today & the Three-Repo Reality

Today QMKonnect builds `{app_class}{GS}{title}` and calls
`qmk_notifier::run(RunParameters::new(RunCommand::SendMessage(msg), …))` (see
`ARCHITECTURE.md` §5, `PROTOCOL.md` §4). The **`qmk_notifier` crate** owns all
wire framing (the `0x81 0x9F` header, 32-byte chunking, the `0x03` ETX
terminator, and the response read). The **`qmk-notifier` firmware** (`notifier.c`)
receives in `hid_notify()`, reassembles to ETX, and `process_full_message()`
matches `command_map` (first match → `on_enable`; previous `on_disable` first)
and `layer_map` (first match → `activate_layer`/`layer_on`; previous
`deactivate_layer`/`layer_off` first), tracking a **single** `activated_layer`.
It always replies with a 32-byte report whose `response[0] = matched` (0/1).

Because the wire protocol is shared, this feature touches **all three repos**:

| Repo | Role | Change here |
| --- | --- | --- |
| `qmk_notifier` (Rust crate) | Host-side framing + HID I/O | Typed commands, response parsing, matcher module |
| `qmk-notifier` (firmware C) | On-keyboard receiver/matcher | Registry, host-layer tracker, typed-command dispatch |
| `qmkonnect` (this app) | Window detection + rules | `rules.toml`, host matcher, handshake, sequencing |

## 3. Locked Design Decisions

> **Design.** The wire contract is owned by the firmware spec
> (`dabstractor/qmk-notifier`, `PRD.md` §4.6 — **canonical**); the transport by
> the `qmk_notifier` crate (`PRD.md` §10); the host-side orchestration by this
> document.

- **B1 — Coexistence = per-window stack-or-replace, host-chosen via
  `clear_board`.** The firmware offers **both**: with `clear_board=0` the board
  runs its rules (board layer first → host layer on top; board callbacks first →
  host callbacks after); with `clear_board=1` the firmware clears its board
  layer/command first and the host context drives the board. The host selects per
  window from `disable_firmware_config` (C10). Board rules are never silently
  discarded — the host decides whether they run.
- **B2 — Callback identity = firmware registry + startup name query.** Firmware
  declares named callbacks (`DEFINE_HOST_CALLBACKS`); IDs are declaration order;
  QMKonnect queries names at (re)connect and the rules file references callbacks
  by **name**. Re-querying on every reconnect makes cross-flash renumbering
  harmless.
- **B3 — "Arbitrary callback" = firmware-registered C functions only** (the
  existing `on_enable`/`on_disable` pattern). Host-side actions (shell/launch)
  and host-driven keyboard macros are **out of scope**.
- **C1 — Format: TOML.** C2 — separate `rules.toml` next to `config.toml`. C3 —
  hot-reload (fs watch + tray "Reload rules"). **C4 — full matcher parity**: port
  the firmware `pattern_match.c` to Rust **including** `+` and the classes
  (`\d \D \w \W \s \S \b \B .`) — they are linear-time in the firmware NFA, so
  there is no perf reason to subset. C5 — capability handshake with graceful
  fallback (gated on `proto_ver == 2`). C6 — VIA coexistence is a future phase
  (feature_flags bit `0x04` reserved). **C7 — no-match ⇒ always clear** (the
  `on_no_match = "keep"` option is dropped; the host layer is cleared and host
  callbacks' `on_disable` fires via the desired-set diff). C8 — **all matching
  callbacks fire**; layers are exclusive (first-match-wins, one host layer). C9 —
  one global ruleset for v1 (per-keyboard overrides later). **C10 —
  `disable_firmware_config` per-rule** (default `false`, global default under
  `[host]`, per-rule override on `[[layer_rules]]`/`[[callback_rules]]`): a
  matched rule with it `true` contributes to a **replace** decision for that
  window. **C11 — host layers reserved ≥ 224** so they resolve above board layers
  under QMK's highest-layer-wins rule (`255 = LAYER_UNSET`/clear). **C12 — host is
  the OS source of truth** while connected: `SET_OS` once at connect
  (host-authoritative; firmware `OS_DETECTION` is the offline fallback).

## 4. Architecture & Coexistence Model

Per-window-change data flow (the `disable_firmware_config` / `clear_board` model):

```
window focus changes
        │
        ▼
debounce (existing, configurable ms)
        │
        ▼
build string  s = "{app_class}\x1D{title}"        (existing)
        │
        ▼
(if host-capable AND rules.toml present)
evaluate host rules against s
   • layer_rules   : first match → L_h  (else none)
   • callback_rules: ALL matches → desired callback id set
   • window is "replace" iff EVERY matched rule has disable_firmware_config=true
        │
   ┌──── replace, OR board has no rules ────┐   ┌── stack (>=1 rule non-disabling) AND board has rules ──┐
   ▼                                        ▼   ▼                                                         ▼
 ② APPLY_HOST_CONTEXT{L_h, set,            ① Send STRING_MATCH(s) ──► firmware runs BOARD rules
      clear_board=1}  (NO string sent)         (disable prev cmd/layer, enable matched) ◄─ response[0]=matched
   ──► firmware clears board layer/cmd,    ② APPLY_HOST_CONTEXT{L_h, set, clear_board=0}
       then applies host layer + callbacks   ──► firmware applies host layer on top, syncs host callbacks
   ◄── response[0]=0x51 ack                  ◄── response[0]=0x51 ack
        │
        ▼
on no match ⇒ APPLY_HOST_CONTEXT{layer=0xFF, set=empty}  (clear host layer + disable all host callbacks)
update host state for next diff/logging
```

**Coexistence semantics (precise):**

- The host decides, per window, whether the board runs: send the **string first**
  iff the board has rules **and** ≥1 matched rule is non-disabling (stack);
  otherwise send **only** `APPLY_HOST_CONTEXT` with `clear_board=1` (replace).
  The string is shared by both board lanes, so it is sent at most once.
- Firmware maintains **two independent layer trackers**: `activated_layer`
  (board, selected per-OS via round-A multi-OS) and `host_layer` (driven by
  `APPLY_HOST_CONTEXT`). They are orthogonal; host layers sit ≥ 224 so they
  resolve above board layers. In **replace** mode the board tracker is cleared
  for that window (the host's `clear_board` flag) and re-engages on the next
  string send.
- Callbacks: in stack mode board callbacks fire during string processing, then
  host callbacks during `APPLY_HOST_CONTEXT`. In replace mode only host callbacks
  fire. The `disable` field in a callback rule is an **explicit-exclusion**
  override; the natural focus-out `on_disable` comes free from the desired-set
  diff (a callback leaving the desired set is disabled by the firmware).
- If `rules.toml` is absent or the keyboard is legacy (`proto_ver != 2`), only ①
  the legacy string runs — today's behavior, bit-for-bit. Host rules are gated on
  `proto_ver == 2`.

## 5. Wire Protocol (typed commands)

> **Canonical: firmware `PRD.md` §4.6.** This section summarizes the
> transport-relevant detail; the firmware owns the byte layout and this document
> defers to it on disagreement. See `PROTOCOL.md` §8 for the desktop mirror and
> the `qmk_notifier` crate `PRD.md` §10 for the transport API.

- **Discriminator:** `data[2] == 0xF0` ⇒ typed command; else legacy string
  (unchanged). `0xF0` can never begin a real matched string (sanitizer allows
  only `0x20–0x7E`), so **legacy firmware safely ignores typed commands**.
- **Framing:** `[0x81][0x9F][0xF0][cmd_id][ args… ][0x03]`, **ETX-framed and
  multi-report** like strings (chunked at 30 payload bytes/report). This removes
  the earlier "≤26 callbacks per report" cap — `APPLY_HOST_CONTEXT` may span
  reports. (The old v1 single-report/≤26 limit is withdrawn.)
- **Responses:** legacy `[matched(0|1)]…`; typed `[0x51][cmd_id_echo][payload]…`;
  no reply ⇒ `Timeout` ⇒ host stays string-only.

**Command table** (firmware §4.6 is authoritative for field definitions):

| `cmd_id` | Name | Request args | Response payload |
| --- | --- | --- | --- |
| `0x01` | `QUERY_INFO` | none | `[proto_ver][feature_flags][callback_count][board_rules_present]` |
| `0x02` | `QUERY_CALLBACK` | `[index]` | `[index][name, NUL-padded]` |
| `0x03` | `SET_OS` | `[os_byte]` | `[ack]` |
| `0x04` | *(reserved — VIA, Phase E)* | — | — |
| `0x05` | `APPLY_HOST_CONTEXT` | `[layer][flags][count][id…]` | `[ack]` |

- `proto_ver`: `1` = legacy string-only firmware; `2` = typed-command capable. Firmware-owned.
- `feature_flags`: `0x01` `APPLY_HOST_CONTEXT`; `0x02` callback registry; `0x04`
  *(reserved)* VIA.
- `os_byte`: `0 UNSURE · 1 LINUX · 2 WINDOWS · 3 MACOS · 4 IOS`. The host sends
  `SET_OS` once at connect; while connected the host OS is **authoritative** for
  `current_os` (firmware `OS_DETECTION` is the offline fallback).
- `APPLY_HOST_CONTEXT.layer`: desired host-layer number (`≥ 224`), or `0xFF`
  (clear). `flags` bit 0 = **`clear_board`** ⇒ firmware clears its board
  `activated_layer` + current command before applying the host context (the
  per-window "replace"). `id…` = the full desired enabled set; firmware diffs
  (disable-before-enable).

**Handshake & `has_been_queried`:** at (re)connect the host sends `QUERY_INFO`
**at most once per board boot** — the firmware sets `has_been_queried` on the
first `QUERY_INFO`, so a mid-session HID re-enumeration against **legacy** firmware
cannot clear an active board layer (legacy walks `QUERY_INFO` as a no-match
string and `process_full_message` always disables/deactivates first — harmless
only when board state is fresh). If `response[0]==0x51` & `proto_ver==2` &
`flags & 0x01` ⇒ `QUERY_CALLBACK` sweep → `name→id` map → validate `rules.toml`.
Else (`response[0] != 0x51` or timeout) ⇒ legacy ⇒ string-only; **never send typed
commands**. (Typed commands bypass `process_full_message`, so they have no
board side effect on `proto_ver==2` firmware.)

## 6. Firmware Spec (`qmk-notifier`)

> **Canonical: firmware `PRD.md` §14 (+ §4.6 wire, §4.7 OS).** This section is a
> desktop-facing summary; the firmware repo owns the authoritative spec.

Firmware requirements:

- **Named callback registry** — `DEFINE_HOST_CALLBACKS({ … })` + weak-default
  accessors (`get_host_callbacks`/`_size`). `ID = array index`, stable per build;
  re-queried by name on every reconnect. Bounded by `HOST_CALLBACK_MAX` (static
  array; the wire no longer caps the id list — multi-report — but the firmware's
  static ceiling is real, so the host must not reference ids ≥
  `HOST_CALLBACK_MAX`; `QUERY_INFO.callback_count` reports the true count).
  ```c
  typedef struct { const char *name; callback_t on_enable; callback_t on_disable; } host_callback_t;
  host_callback_t* get_host_callbacks(void);
  size_t           get_host_callbacks_size(void);
  ```
- **Second layer tracker** `host_layer` (independent of board `activated_layer`)
  + `host_cb_enabled[]`. `set_host_layer(layer)`: `layer_on/off` the host tracker
  only; `0xFF` ⇒ clear. `apply_host_callbacks(ids, count)`: disable-before-enable
  diff (fire `on_disable` for ids leaving the set, `on_enable` for ids entering).
- **Typed dispatch** at the top of `hid_notify()`: `data[2]==0xF0` ⇒
  `handle_typed_command()` (return; **no** `process_full_message` side effect);
  else legacy string (unchanged). Handlers:
  - `QUERY_INFO` / `QUERY_CALLBACK` — answerable before any string seen; the
    firmware sets `has_been_queried` on the first `QUERY_INFO`.
  - `APPLY_HOST_CONTEXT` — honor `clear_board` (flags bit 0): if set,
    `deactivate_layer()` the board `activated_layer` + `disable_command()` the
    board command **first**, then `set_host_layer()` + `apply_host_callbacks()`.
  - `SET_OS` (`0x03`) — update `current_os` (host-authoritative while a host is
    connected; firmware `OS_DETECTION` resumes as the offline fallback).
- **Tests:** `set_host_layer` (on/off/clear; independence from board layer),
  `apply_host_callbacks` (diff ordering; idempotence), typed-command round-trips,
  `clear_board` clearing, `SET_OS` updating `current_os`.

## 7. Crate Spec (`qmk_notifier`, Rust)

> **Canonical: the crate `PRD.md` §10.** This section is a summary. The crate is
> **transport-only** — it does no matching (the matcher lives in `qmkonnect`, §8).

API additions (`run()` returns `CommandResponse` instead of `()`):

```rust
pub enum RunCommand {
    SendMessage(String),                                                // legacy string
    ListDevices,
    QueryInfo,                                                          // 0x01
    QueryCallback(u8),                                                  // 0x02
    SetOs(HostOs),                                                      // 0x03
    ApplyHostContext { layer: Option<u8>, callbacks: Vec<u8>, clear_board: bool }, // 0x05
}

#[repr(u8)]
pub enum HostOs { Unsure = 0, Linux = 1, Windows = 2, Macos = 3, Ios = 4 }  // mirrors os_variant_t

pub enum CommandResponse {
    Legacy { matched: bool },              // response[0] in {0,1}
    Info { proto_ver: u8, feature_flags: u8, callback_count: u8, board_rules_present: bool },
    CallbackName { index: u8, name: Option<String> },
    Ack { ok: bool },
    Timeout,
}

pub fn run(params: RunParameters) -> Result<CommandResponse, QmkError>;
```

- **Framing:** `SendMessage` keeps the existing header+chunk+ETX path. Typed
  variants build `[0x81,0x9F,0xF0,cmd, args…]` and reuse the **same ETX-framed,
  multi-report chunking** as strings — so `APPLY_HOST_CONTEXT` may span reports
  (no fixed callback-id cap). The device cache + retry logic are unchanged.
- **Response parse:** after a typed burst, read one 32-byte IN report;
  `response[0]==0x51` ⇒ typed (decode by `cmd_echo`); `in {0,1}` ⇒ `Legacy`; no
  reply ⇒ `Timeout`.

**Release:** tag the release; `qmkonnect/Cargo.toml` pins the crate by git tag.

## 8. QMKonnect Spec (this repo)

**(1) `rules.toml`** — new module `src/core/rules.rs`, alongside `config.toml`
(Linux `~/.config/qmk-notifier/rules.toml`, Windows `%APPDATA%\QMKonnect\`,
macOS `~/Library/Application Support/QMKonnect/`). Absent ⇒ host rules disabled
(string-only). Schema in §9; CLI seeding in (6).

**(2) Host matcher — in qmkonnect, NOT the crate.** Port the firmware
`pattern_match.c` to Rust at `src/core/pattern.rs` (**full parity**: `* ^ $ WT +`
and `\d \D \w \W \s \S \b \B .` — all linear-time). Port the firmware test corpus
as parity tests. Semantics:
- `Pattern::Single(p)`: if the window has a title, match `p` against
  **app_class only** (firmware parity); else against the whole string.
- `Pattern::Parts(c, t)`: both halves must match.

**(3) Per-window evaluation** (`src/core/notifier.rs`). After debounce:
1. **Layer:** first matching `layer_rule` ⇒ `L_h` (else none).
2. **Callbacks:** **all** matching `callback_rules` ⇒ desired enabled id set;
   each rule's `disable` names are an **explicit exclusion** (removed from the
   desired set, so the firmware's diff fires their `on_disable`).
3. **Stack-vs-replace:** the window is **replace** iff every matched rule's
   effective `disable_firmware_config` is `true`.

**(4) `notify_qmk` send logic** (the `disable_firmware_config` / `clear_board`
model). For one debounced window change:
- **Stack** (board has rules AND ≥1 matched rule non-disabling): send the
  **string** first (`RunCommand::SendMessage`), await its `CommandResponse`, then
  `ApplyHostContext { layer: L_h, callbacks, clear_board: false }`.
- **Replace** (all matched rules disabling, OR board has no rules): send **only**
  `ApplyHostContext { layer: L_h, callbacks, clear_board: true }` (no string →
  board can't match → firmware clears its board layer/cmd via the flag).
- **No match:** `ApplyHostContext { layer: None (0xFF), callbacks: empty,
  clear_board: <per flag> }` — always clear the host layer + disable all host
  callbacks (`on_no_match` is always clear; the `keep` option is withdrawn).
- The `Notifier` trait / `QmkNotifier` gain the capability so the test mock
  asserts ordering (string before context). Retry/cache parity with `SendMessage`.

**(5) Startup handshake + `SET_OS`.** Near `startup_device_probe`, once a device
is connected:
```text
resp = run(QueryInfo)
match resp {
  Info { proto_ver: 2, feature_flags, callback_count, .. } if flags & 0x01 => {
      run(SetOs(host_os))                                  // host is OS-authoritative at connect
      for i in 0..callback_count { name_to_id.insert(run(QueryCallback(i)).name, i) }
      validate rules.toml names against name_to_id         // warn, don't fail
      capable = true
  }
  _ => capable = false   // legacy/offline → string-only
}
```
The handshake runs **at most once per board boot** — the firmware's
`has_been_queried` guards against mid-session-reconnect side effects on legacy
firmware, and host-rules are gated on `proto_ver == 2`. Re-trigger only on a real
device transition via the existing `is_device_connected()` poll, deduped by the
`capable`/`has_been_queried` state.

**(6) CLI:** `--list-callbacks` (handshake → name→id table, or "legacy");
`--validate-rules [--rules-path <p>]` (parse + schema check; flag unknown callback
names; non-zero exit on error); `--rules-path`. `-c`/`--config` seeds a commented
`rules.toml` template.

**(7) Tray/UX:** add **"Reload rules"** to all three menus (re-read `rules.toml`,
re-validate, re-handshake if needed). Optional status line: `proto v2 · N callbacks`.

**(8) Backward compatibility:** no `rules.toml` ⇒ identical to today; legacy
firmware (`proto_ver != 2` / timeout) ⇒ string-only, board rules unaffected; new
firmware + old QMKonnect ⇒ old app sends only the string, typed commands never
arrive.

## 9. `rules.toml` Schema Reference

```toml
# rules.toml — host-side window rules.
# disable_firmware_config chooses, per window, whether the board runs its own
# rules (stack) or is cleared and driven solely by the host (replace). Global
# default under [host]; per-rule override below. Host layers are >= 224.
# Run `qmkonnect --validate-rules` after editing.

[host]
disable_firmware_config = false   # global default: false = stack (board runs), true = replace
# On no match the host layer is always cleared and all host callbacks disabled.

# Layer rules: FIRST match wins. One host layer active at a time (>= 224).
[[layer_rules]]
match = "alacritty"                       # class-only pattern
layer = 224
disable_firmware_config = true           # optional override (default inherits [host])

[[layer_rules]]
match = ["*chrome*", "*youtube*"]         # [class_pattern, title_pattern] (== WT())
layer = 225
case_sensitive = false                    # optional, default false

# Callback rules: ALL matches fire. Names come from the keyboard's registry
# (run `qmkonnect --list-callbacks` to see them). The disable list is an
# explicit-exclusion override; focus-out on_disable fires automatically via the
# desired-set diff.
[[callback_rules]]
match = "neovide"
enable = ["vim_lazy", "disable_vim"]      # run on focus-in
disable = ["vim_lazy"]                    # optional: force-off override

[[callback_rules]]
match = ["*chrome*", "*claude*"]
enable = ["vim_lazy", "disable_vim"]
disable_firmware_config = true           # for this window, skip the string -> board can't match
```

Rust model (`src/core/rules.rs`):

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct RuleSet {
    #[serde(default)] pub host: HostDefaults,
    #[serde(default, rename = "layer_rules")]    pub layer_rules: Vec<LayerRule>,
    #[serde(default, rename = "callback_rules")] pub callback_rules: Vec<CallbackRule>,
}

#[derive(Debug, Deserialize)]
pub struct HostDefaults {
    #[serde(default)] pub disable_firmware_config: bool,   // default false (stack)
}
impl Default for HostDefaults { fn default() -> Self { Self { disable_firmware_config: false } } }

#[derive(Debug, Deserialize)]
pub struct LayerRule {
    #[serde(rename = "match")] pub pattern: Pattern,
    pub layer: u8,
    #[serde(default)] pub case_sensitive: bool,
    #[serde(default)] pub disable_firmware_config: Option<bool>,  // None => inherit [host]
}

#[derive(Debug, Deserialize)]
pub struct CallbackRule {
    #[serde(rename = "match")] pub pattern: Pattern,
    #[serde(default)] pub enable: Vec<String>,
    #[serde(default)] pub disable: Vec<String>,
    #[serde(default)] pub case_sensitive: bool,
    #[serde(default)] pub disable_firmware_config: Option<bool>,  // None => inherit [host]
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Pattern {
    Single(String),                 // "foo"  -> class only
    Parts(String, String),          // ["cls","ttl"]
}
```

A rule's effective `disable_firmware_config` = its override if `Some`, else the
`[host]` default. The window is **replace** iff every matched rule's effective
flag is `true` (the string is shared by both board lanes, so it is sent iff the
board has rules **and** ≥1 matched rule is non-disabling). Match semantics are a
**full-parity port** of the firmware `pattern_match.c` (incl. `+` and
`\d \D \w \W \s \S \b \B .` — all linear-time in the NFA). `case_sensitive` per
rule (default `false`).

## 10. Migration from `DEFINE_*`

Board rules keep working, so migration is **incremental and optional**:

1. **Expose callbacks by name** (one-time firmware change): add
   `DEFINE_HOST_CALLBACKS({ … })` listing the functions you already use in
   `DEFINE_SERIAL_COMMANDS`. Reflash once.
2. **Move a layer rule to the host:** add a `[[layer_rules]]` entry to
   `rules.toml`; **remove** it from `DEFINE_SERIAL_LAYERS` to avoid the same
   layer being driven by both trackers (harmless but confusing). No reflash
   needed for future edits.
3. **Move a callback rule to the host:** add a `[[callback_rules]]` entry;
   **remove** it from `DEFINE_SERIAL_COMMANDS` (callbacks are additive — if kept
   in both, the same `on_enable` would fire twice).
4. Iterate by editing `rules.toml` + "Reload rules" — no reflashing.

## 11. Implementation Breakdown (by repo)

One coordinated change across the three repos:

- **`qmk_notifier` crate:** typed-command framing (multi-report),
  `CommandResponse` reply parsing, `HostOs`, `run()` → `CommandResponse`. Tag the
  release. *(The matcher is NOT added here — it lives in qmkonnect.)*
- **`qmk-notifier` firmware:** `DEFINE_HOST_CALLBACKS`,
  `host_layer`/`host_cb_enabled`, typed dispatch, `QUERY_INFO`/`QUERY_CALLBACK`/
  `SET_OS`/`APPLY_HOST_CONTEXT` (with `clear_board`), `has_been_queried`, tests.
- **`qmkonnect`:** pin the crate; `src/core/rules.rs` + `src/core/pattern.rs`
  (full-parity matcher + ported corpus); handshake + `SET_OS`; the `notify_qmk`
  `disable_firmware_config`/`clear_board` send logic + state; CLI flags; tray
  "Reload rules"; config-path integration; tests.
- **Docs:** `Readme.md`, `docs/qmk-integration.md`, `docs/configuration.md`,
  `docs/examples.md`, `docs/troubleshooting.md`, regenerated `docs/llms_full.txt`.
- **VIA coexistence (separate, out of scope here):** a dispatching
  `raw_hid_receive` (`0x81 0x9F`+`0xF0` → notifier, else → VIA).

## 12. Testing Plan

**`qmk_notifier` crate:** unit-test framing of each `RunCommand` (incl.
multi-report `APPLY_HOST_CONTEXT`) and response decoding (`0x51` typed vs `0`/`1`
legacy vs `Timeout`).

**`qmk-notifier` firmware:** unit-test `set_host_layer` (on/off/clear;
independence from board `activated_layer`) and `apply_host_callbacks`
(disable-before-enable; idempotent re-apply; unknown ids ignored); integration:
typed-command round-trips, `clear_board` clearing, `SET_OS` updating `current_os`.

**`qmkonnect`:** unit-test (`src/core/rules.rs`) TOML parse success/error, matcher
first-match (layers) vs all-match (callbacks), `disable` exclusion, unknown
callback names skipped; unit-test (`src/core/pattern.rs`) **full matcher parity**
by porting the firmware `pattern_match` corpus (wildcards, `^`/`$`, `WT`, `+`,
classes, case sensitivity) and asserting identical results; unit-test handshake
parsing (`Info { proto_ver: 2 }` ⇒ capable; legacy/timeout ⇒ string-only) and the
`disable_firmware_config` ⇒ stack/replace send decision; unit-test ordering — the
`Notifier` mock records calls and asserts string-before-context (stack) and
context-only (replace); integration per `AGENTS.md`.

## 13. Risks & Open Questions

- **R1 — HID round-trips per change.** Stack mode = two sends (string + context)
  per debounced change; replace mode = one. Mitigated by the existing debounce.
- **R2 — `APPLY_HOST_CONTEXT` size — RESOLVED.** Typed commands are ETX-framed /
  multi-report (like strings), so the callback-id list is uncapped; the earlier
  "≤26 ids per report" v1 limit is withdrawn. (`HOST_CALLBACK_MAX` remains the
  firmware's static array ceiling; the host validates against
  `QUERY_INFO.callback_count`.)
- **R3 — HID exclusivity.** Another Raw HID app (VIA) holding the device blocks
  QMKonnect. Phase E.
- **R4 — ID stability across flashes.** Mitigated by re-querying names on every
  reconnect (IDs positional, names stable).
- **R5 — Multiple keyboards.** v1 = one global ruleset; per-keyboard overrides
  deferred.
- **R6 — Legacy handshake side effect — RESOLVED.** The firmware sets
  `has_been_queried` on the first `QUERY_INFO`, and host-rules are gated on
  `proto_ver == 2`; the host handshakes at most once per board boot. Legacy
  firmware never receives typed commands.
- **Q1 — `default_layer` / a "default" no-match mode.** Reserved in the schema
  but not wired (`on_no_match` is always `clear`). Add if a use case appears.
- **Q2 — `disable` list semantics — RESOLVED.** `disable` = explicit exclusion
  (removed from the desired enabled set; the firmware's diff fires `on_disable`).
  Focus-out `on_disable` also fires automatically when a callback leaves the
  desired set across window changes.
- **Q3 — Board matcher stays first-match.** Host callbacks use all-match (C8);
  board `DEFINE_SERIAL_COMMANDS` keeps first-match for backward compatibility.

## 14. Appendix — File Layout Touched & Pattern Subset

**File layout:**

```
qmkonnect/
  Cargo.toml                              # pin qmk_notifier crate by git tag
  src/core/notifier.rs                    # notify_qmk extension, handshake, SET_OS, state
  src/core/rules.rs                       # NEW: rules.toml model + evaluation
  src/core/pattern.rs                     # NEW: full-parity matcher (ported from firmware)
  src/core/mod.rs                         # wire rules into config/startup
  src/main.rs                             # --list-callbacks / --validate-rules
  src/tray.rs / src/linux_tray.rs         # "Reload rules" menu item
  Readme.md, docs/*.md, docs/llms_full.txt
qmk_notifier/  (external crate)
  src/lib.rs / src/core.rs                # RunCommand variants, HostOs, CommandResponse, run()
qmk-notifier/  (external firmware)
  notifier.h / notifier.c                 # host_callback_t, DEFINE_HOST_CALLBACKS,
                                          #   host_layer, host_cb_enabled, typed dispatch,
                                          #   SET_OS, clear_board, has_been_queried
```

**Pattern matching semantics** — a **full-parity** port of the firmware
`pattern_match.c` into `qmkonnect::pattern` (not a subset): `*` wildcard; `^`/`$`
anchors; two-part `WT(class,title)` / `Pattern::Parts` (delimiter `0x1D`, GS); `X+`
quantifier; classes `\d \D \w \W \s \S \b \B`; `.`; escapes. All linear-time
(Thompson NFA). `case_sensitive` per rule (default `false`). The firmware matcher
+ its test corpus are the single source of truth for match semantics.

---

*The wire contract is canonical in the firmware `PRD.md` §4.6; transport in the
`qmk_notifier` crate `PRD.md` §10. Return to `PRD.md` for the product-level
overview and the Document Map.* | **Host-side `rules.toml`** (no-reflash layer/callback rules, per-rule `disable_firmware_config`), the typed-command wire mirror (canonical: firmware `PRD.md` §4.6), named callback registry, three-repo rollout. |

> **Living source of truth:** the production codebase itself
> (`src/`, `Cargo.toml`, `packaging/`). Where a spec and the code disagree, the
> code wins; report the drift. The specs capture the *intended* design at
> v0.2.4.

---

*End of PRD. Continue with `ARCHITECTURE.md`.*

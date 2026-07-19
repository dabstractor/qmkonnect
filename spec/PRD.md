# PRD — QMKonnect

**Product Requirements Document & Master Specification**
Version: 0.2.4 · Status: Beta · Owner: Mulletware · License: MIT

> **Planned feature (v0.3.0, "round B"):** Host-side window rules — edit
> layer/callback rules in `rules.toml` with no reflash. Fully specified in the
> companion `HOST_RULES.md`; the **typed-command wire contract is owned by the
> firmware spec** ([`dabstractor/qmk-notifier` `PRD.md`
> §4.6](https://github.com/dabstractor/qmk-notifier/blob/main/PRD.md)), transported
> by the `qmk_notifier` crate ([`SPEC.md`
> §10](https://github.com/dabstractor/qmk_notifier/blob/main/SPEC.md)). See F11/F12
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
  relaxed by the planned Host-Side Window Rules feature — see `HOST_RULES.md` —
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
| **F11** *(planned v0.3.0)* | **Host-side window rules:** edit `rules.toml` to map apps → layers/callbacks with **no reflash** (stacks on top of board rules) | `HOST_RULES.md` |
| **F12** *(planned v0.3.0)* | **Named callback registry** + typed Raw HID commands (`QUERY_INFO` / `QUERY_CALLBACK` / `APPLY_HOST_CONTEXT`) with a capability handshake | `HOST_RULES.md` |

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

- **Host-side window rules (planned, v0.3.0).** The biggest planned feature:
  edit layer/callback rules in `rules.toml` on the host — no reflash — stacking
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
| **board layer / board rules** *(planned)* | The layer/callback state driven by the keyboard's own `DEFINE_SERIAL_LAYERS`/`DEFINE_SERIAL_COMMANDS` matcher against the string QMKonnect sends. See `HOST_RULES.md`. |
| **host layer / host rules** *(planned)* | The layer/callback state driven by QMKonnect matching `rules.toml` on the host and sending `APPLY_HOST_CONTEXT`; stacks **on top of** the board layer. See `HOST_RULES.md`. |
| **callback registry** *(planned)* | The firmware's named, ordered list of host-invokable callbacks (`DEFINE_HOST_CALLBACKS`); the host resolves names→IDs via `QUERY_CALLBACK`. See `HOST_RULES.md` §6. |
| **typed command** *(planned)* | A Raw HID command in the `0x81 0x9F 0xF0` namespace (vs. the legacy string path). See `HOST_RULES.md` §5. |
| **`APPLY_HOST_CONTEXT`** *(planned)* | The typed command carrying the host's desired layer + enabled-callback set; the firmware diffs and applies it. See `HOST_RULES.md` §5. |

---

## 14. Document Map

The master PRD (this file) hard-links these companion specifications. A dev agent
should read them in roughly this order; each is self-contained but assumes the
PRD.

| Document | Scope |
|---|---|
| **`PRD.md`** (this) | Product vision, goals, users, features, glossary, doc map. |
| @ARCHITECTURE.md | Repository layout, module map, end-to-end data flow, concurrency/threading model, trait design, error model, the platform-divergence problem. |
| @PROTOCOL.md | The Raw HID wire protocol: payload format, report framing, constants, the `qmk_notifier` crate contract, device matching & discovery, retry/cache. |
| @PLATFORMS.md | Per-OS window monitoring (Windows WinEventHook, macOS NSWorkspace, Hyprland IPC, X11), window filtering, config paths, permissions. |
| @UI.md | Tray/menu-bar UI, menu layouts, Settings dialogs, "Show Window Information" dialogs, device-status indicator, "Open at Login" autostart. |
| @LINUX.md | Linux-specific: static udev rule, `qmkonnect-hid-id` helper, config-driven fallback rule, dangerous-rule detection/repair, root-aware `--reload`, systemd service, SNI tray, GTK window-info dialog. |
| @CONFIG.md | TOML schema, defaults, render body, config paths per OS, CLI flag reference. |
| @PACKAGING.md | Cargo build profile, per-platform installers (Inno/PKGBUILD/DMG), CI release workflow, code signing, the dev test loop. |
| @FIRMWARE.md | The `qmk-notifier` firmware module contract, keymap integration steps, pattern-matching syntax, the user's reference keymap. |
| @HOST_RULES.md | **Round B (v0.3.0), specified — not yet implemented:** host-side `rules.toml` (no-reflash layer/callback rules, per-rule `disable_firmware_config`), the typed-command wire mirror (canonical: firmware `PRD.md` §4.6), named callback registry, three-repo rollout. |

> **Living source of truth:** the production codebase itself
> (`src/`, `Cargo.toml`, `packaging/`). Where a spec and the code disagree, the
> code wins; report the drift. The specs capture the *intended* design at
> v0.2.4.

---

*End of PRD. Continue with `ARCHITECTURE.md`.*

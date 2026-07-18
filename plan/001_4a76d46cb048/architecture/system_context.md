# System Context — QMKonnect

## Project Identity

- **Name:** QMKonnect — cross-platform desktop daemon for QMK keyboards
- **Cargo.toml version:** 0.2.8 (PRD specifies 0.2.4 — **drift noted**)
- **Status:** Beta (per PRD §12)
- **License:** MIT
- **MSRV:** Rust 1.88 (`rust-version` in Cargo.toml; image 0.25.x is the floor)
- **Codebase size:** ~7,582 lines of Rust across 18 source files
- **Tests:** 33 unit tests, all passing (`cargo test --bin qmkonnect -- --test-threads=1`)

## The Two-Part System

QMKonnect is the **desktop half** of a strictly two-part system:

```
┌──────────────────────────┐      Raw HID (0xFF60/0x61)     ┌─────────────────────────┐
│  QMKonnect (desktop)     │  ─────────────────────────────►│  qmk-notifier (firmware)│
│  Windows / macOS / Linux │   "{app_class}\x1D{title}\x03" │  layer switch / callback│
└──────────────────────────┘                                └─────────────────────────┘
```

The desktop app **only sends** window metadata. All layer/command logic lives in firmware.

## Ecosystem (Three Repos + QMK Upstream)

| Project | Repo | Role |
|---------|------|------|
| **QMKonnect** | `dabstractor/qmkonnect` (this) | Desktop app: window detection + Raw HID send |
| **qmk-notifier** | `dabstractor/qmk-notifier` | QMK **firmware module** (C): receives, reassembles, pattern-matches |
| **qmk_notifier** | `dabstractor/qmk_notifier` (underscore) | Rust **library**: Raw HID transport (device cache, burst-write, framing) |
| **qmk_firmware** | `qmk/qmk_firmware` | Upstream QMK; hosts both modules |

**Dependency:** QMKonnect links `qmk_notifier` v0.2.1 (git tag pinned in Cargo.toml).
The user's keymap links `qmk-notifier` (firmware C module).

## Existing PRP Context

- **PRP 002:** "Host-Side Window Rules — Layer Switching & Arbitrary Callbacks" (Draft/Approved).
  This is a major upcoming feature spanning all three repos: typed-command framing,
  a public pattern-matcher module, `rules.toml` parsing on the host, a capability
  handshake, and firmware callback registry. It builds on the existing Raw HID protocol.

## Feature Inventory (PRD §4)

| # | Feature | Status | Implementation |
|---|---------|--------|----------------|
| F1 | Foreground-window detection (per platform) | ✅ Implemented | `src/platforms/{windows,macos,hyprland,x11}.rs` |
| F2 | Raw HID transport (burst-write, cache, retry) | ✅ Implemented | `qmk_notifier` crate + `src/core/notifier.rs` |
| F3 | Auto device discovery (usage page/usage) | ✅ Implemented | `configured_filter()` in `src/core/notifier.rs` |
| F4 | Debounced coalescing of rapid changes | ✅ Implemented | `DebounceState` + worker thread in `src/core/notifier.rs` |
| F5 | TOML config with zero-config defaults + CLI | ✅ Implemented | `src/core/mod.rs` + `src/main.rs` |
| F6 | Tray/menu-bar UI + dialogs | ✅ Implemented | `src/tray.rs` (macOS/Win), `src/linux_tray.rs` (Linux SNI) |
| F7 | "Open at Login" toggle (default on) | ✅ Implemented | `src/autostart.rs` (Win), `src/tray.rs mod autostart` (macOS) |
| F8 | Per-platform installer + CI | ✅ Implemented | `packaging/` + `.github/workflows/` |
| F9 | Linux: static udev rule + helper + reload | ✅ Implemented | `src/bin/hid_id.rs`, `src/platforms/linux.rs` |
| F10 | Companion firmware module contract | ✅ Documented | `spec/FIRMWARE.md` (external repo) |

## Key Architectural Decisions

1. **Platform divergence by thread ownership:** Each OS disagrees about who owns
   the main thread (Windows: message loop via tao, macOS: CFRunLoop + tao, Hyprland:
   blocking IPC socket, X11: no GUI loop). Resolved with three thin per-OS runners.

2. **Debouncer in core:** Decouples *when a window change is observed* from *when
   it's sent*. Single worker thread, Mutex+Condvar, global state.

3. **Config is hot:** Re-read from disk on every notification and status poll.
   Exception: debounce interval is loaded once (Gap G5).

4. **Fail-soft on device errors:** `QmkNotifier::notify` returns `Ok(())` after 3
   retries to prevent restart loops. The startup probe (`startup_device_probe`)
   catches typos loudly.

5. **Read-only device probes:** `is_device_connected` and `startup_device_probe`
   never open the device — pure hidapi enumeration, cannot disturb the keyboard.

## Build Configuration

- **Profile:** `opt-level="z"`, LTO, single codegen unit, `panic="abort"`, stripped
- **Features (default):** `["hyprland", "macos", "linux-tray"]`
- **Windows:** `+crt-static` (no VC++ Redistributable), `windows_subsystem="windows"`
- **Binaries:** `qmkonnect` (app) + `qmkonnect-hid-id` (udev helper)

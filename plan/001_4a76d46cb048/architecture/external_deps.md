# External Dependencies & Constraints — QMKonnect

## Rust Crate Dependencies (Cargo.toml)

### Cross-platform
| Crate | Version | Purpose |
|-------|---------|---------|
| `serde` / `serde_json` | 1.0 | Config serialization |
| `qmk_notifier` | v0.2.1 (git tag) | **Core transport**: Raw HID framing, device cache, burst-write |
| `hidapi` | 2.6 | HID device enumeration and I/O (via qmk_notifier) |
| `ctrlc` | 3.4 | Signal handling (process::exit(0) on Ctrl-C) |
| `once_cell` | 1.21 | Lazy statics (NOTIFIER, STATE, COND, WORKER) — could use std LazyLock |
| `dirs` | 5.0 | Platform config/home directories |
| `toml` | 0.9 | Config file parsing |
| `tao` | 0.32.8 | Cross-platform event loop (tray/tao) |
| `tray-icon` | 0.20.0 | System tray icon |
| `image` | 0.25.5 | PNG icon decoding (no default features, `png` only) |
| `thiserror` | 1.0 | Error derive (used in qmk_notifier) |
| `log` | 0.4 | Logging facade |
| `env_logger` | 0.11 | Logger implementation (Windows + general) |

### Linux-specific (`cfg(target_os = "linux")`)
| Crate | Version | Feature | Purpose |
|-------|---------|---------|---------|
| `hyprland` | 0.4.0-beta.2 | `hyprland` (optional) | Hyprland IPC (event listener, Client/Monitors/Clients) |
| `libxdo` | 0.6 | — | X11 automation (used by x11 fallback) |
| `tempfile` | 3.0 | — | Atomic udev rule writes |
| `libc` | 0.2 | — | `geteuid()` for root-aware reload |
| `ksni` | 0.3 | `linux-tray` (optional, `["blocking"]`) | StatusNotifierItem over D-Bus |
| `gtk` | 0.18 | `linux-tray` (optional) | Native GTK window-info popup |

### macOS-specific (`cfg(target_os = "macos")`)
| Crate | Version | Feature | Purpose |
|-------|---------|---------|---------|
| `objc` | 0.2.7 | `macos` (optional) | Objective-C runtime interop |
| `objc2-foundation` | 0.3.0 | — | Foundation framework bindings |
| `objc2-core-foundation` | 0.3.0 | — | CoreFoundation bindings |
| `core-foundation` | 0.9 | `macos` (optional) | CFBundle, CFURL |
| `core-graphics` | 0.23.2 | `macos` (optional) | CGWindowList for titles |
| `libc` | 0.2 | — | — |
| `dispatch` | 0.2 | `macos` (optional) | GCD main queue (deferred autostart) |

### Windows-specific (`cfg(target_os = "windows")`)
| Crate | Version | Purpose |
|-------|---------|---------|
| `windows` | 0.52.0 | Win32 API (many features: WinEventHook, Registry, UI, etc.) |
| `eventlog` | 0.2.2 | Windows Event Log integration |
| `single-instance` | 0.3 | Named-mutex single-instance guard |

### Dev dependencies
`proptest` 1.0, `tempfile` 3.0, `mockall` 0.11

## The `qmk_notifier` Crate Contract (v0.2.1)

This is the **most critical external dependency**. QMKonnect links it for all
keyboard I/O. Public API surface:

```rust
// Constants
pub const DEFAULT_USAGE_PAGE: u16 = 0xFF60;
pub const DEFAULT_USAGE: u16 = 0x61;
pub const REPORT_LENGTH: usize = 32;

// Commands
pub enum RunCommand { SendMessage(String), ListDevices }

// Parameters
pub struct RunParameters {
    pub command: RunCommand,
    pub vendor_id: Option<u16>,    // None = match any
    pub product_id: Option<u16>,   // None = match any
    pub usage_page: u16,           // required (default 0xFF60)
    pub usage: u16,                // required (default 0x61)
    pub verbose: bool,
}

// Entry points
pub fn run(params: RunParameters) -> Result<(), QmkError>;
pub fn list_hid_devices() -> Result<(), QmkError>;
pub fn send_raw_report(data, vid, pid, page, usage, verbose) -> Result<(), QmkError>;
```

**Protocol contract:** Appends `0x03` (ETX), frames into 33-byte hidapi buffers
(`[0x00, 0x81, 0x9F, <30 payload>...]`), burst-writes to cached matching devices.

**Error types QMKonnect reacts to:** Strings containing `"no device found"`,
`"permission denied"`, `"failed to open"` → retried; others propagate immediately.

## Platform Build Requirements

| Platform | Toolchain | System deps |
|----------|-----------|-------------|
| **Windows** | `stable-x86_64-pc-windows-msvc` (NOT gnu), VS Build Tools (Desktop C++) | None (static CRT) |
| **macOS** | `stable-aarch64-apple-darwin` / `stable-x86_64-apple-darwin` | Xcode Command Line Tools |
| **Linux** | stable | `libxdo-dev`, `libudev-dev` (Ubuntu); `libxdo-devel`, `systemd-devel` (Fedora); `-lhidapi-hidraw` (Arch) |

## Protocol Constants (must never change without coordinating both halves)

| Constant | Value | Where |
|----------|-------|-------|
| Group Separator (GS) | `0x1D` (29) | class/title delimiter in payload |
| End of Text (ETX) | `0x03` (3) | message terminator (appended by crate) |
| Magic header | `0x81 0x9F` | first 2 payload bytes (firmware guard) |
| Report ID byte | `0x00` | leading byte of 33-byte hidapi write buffer |
| Report size | 32 | logical report (all QMK protocols) |
| Payload per report | 30 | after the 2 magic bytes |
| Firmware buffer | 256 | `MSG_BUFFER_SIZE` |
| Default usage page | `0xFF60` | `DEFAULT_USAGE_PAGE` |
| Default usage | `0x61` | `DEFAULT_USAGE` |

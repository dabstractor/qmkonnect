# qmk-notifier Crate Boundary & External Dependencies

## qmk-notifier v0.3.0 — Git Dependency

### Cargo.toml (qmkonnect root)
```toml
qmk-notifier = { git = "https://github.com/dabstractor/qmk-notifier", tag = "v0.3.0" }
hidapi = "2.6"
```
Cargo.lock pins: rev `f26893ed92fcb3698eadc13322c10d0f9b1a80c9`.
Crate source: `~/.cargo/git/checkouts/qmk-notifier-1f15950b695d9922/f26893e/`

### Public API Surface (src/lib.rs)

```rust
pub fn run(params: RunParameters) -> Result<CommandResponse, QmkError>;

pub enum RunCommand {
    SendMessage(String),
    ListDevices,
    QueryInfo,
    QueryCallback(u8),
    SetOs(HostOs),
    ApplyHostContext { layer: Option<u8>, callbacks: Vec<u8>, clear_board: bool },
}

pub enum CommandResponse {
    Legacy { matched: bool },
    Info { proto_ver: u8, feature_flags: u8, callback_count: u8, board_rules_present: bool },
    CallbackName { index: u8, name: Option<String> },
    Ack { ok: bool },
    Timeout,
}

pub enum HostOs { Unsure=0, Linux=1, Windows=2, Macos=3, Ios=4 }

pub struct RunParameters {
    command: RunCommand,
    vendor_id: Option<u16>,
    product_id: Option<u16>,
    usage_page: u16,
    usage: u16,
    verbose: bool,
}

pub fn send_raw_report(data: &[u8], vid: Option<u16>, pid: Option<u16>,
                       usage_page: u16, usage: u16, verbose: bool)
    -> Result<Option<Vec<u8>>, QmkError>;

// Constants
pub const DEFAULT_VENDOR_ID: u16 = 0xFEED;   // matching-dead legacy
pub const DEFAULT_PRODUCT_ID: u16 = 0x0000;  // matching-dead legacy
pub const DEFAULT_USAGE_PAGE: u16 = 0xFF60;
pub const DEFAULT_USAGE: u16 = 0x61;
pub const REPORT_LENGTH: usize = 32;
```

### PRIVATE Internals (NOT accessible from qmkonnect)

```rust
// core.rs:641 — PRIVATE, filter-keyed (NOT path-keyed)
struct MatchKey {
    vendor_id: Option<u16>,
    product_id: Option<u16>,
    usage_page: u16,
    usage: u16,
}

// core.rs:723 — PRIVATE
fn open_matching_devices(api: &HidApi, key: &MatchKey) -> Result<Vec<HidDevice>, QmkError>
// Opens via info.open_device(api) — hidapi's DEFAULT non-seize open.
// NO seize/exclusive flag anywhere.

// core.rs:660 — PRIVATE global cache
static DEVICE_CACHE: LazyLock<Mutex<Option<DeviceCache>>>  // keyed by MatchKey
```

### Read Discipline (core.rs, in the crate — NOT in qmkonnect)

```rust
const REPLY_READ_TIMEOUT_MS: i32 = 1000;  // core.rs:69
const IN_DRAIN_MAX: usize = 32;           // core.rs:131
const SEND_RETRIES: usize = 1;            // core.rs:138
```

**Protocol:**
1. Pre-send drain: `read_timeout(0)` up to `IN_DRAIN_MAX` non-blocking reads (discard stale replies)
2. Burst-write: send all report fragments to all matching handles
3. Bounded capture: `read_timeout(REPLY_READ_TIMEOUT_MS)` — keep last non-empty reply
4. Post-capture surplus drain: `read_timeout(0)` loop bounded by `IN_DRAIN_MAX`

### CRITICAL CONSTRAINT

**There is NO per-path send and NO per-device send in the crate.** The only send
primitives (`run()`, `send_raw_report()`) take a `MatchKey` (vid/pid/page/usage)
and broadcast to ALL matching devices. `MatchKey`, `open_matching_devices`, and
`DEVICE_CACHE` are all **private** to `core.rs`.

**Implication for multi-board write-narrowing (§4.2):** Cannot be done app-side
without either (a) a coordinated crate API addition (new per-path send or
capability field in MatchKey), or (b) VID/PID filter narrowing (works only when
capable boards have distinct VID/PID from VIA boards — not generally true).

**Decision (per PRD §2.2):** DEFER write-narrowing. It is harmless today (VIA
firmware ignores 0x81-prefixed magic). Record as follow-up.

---

## hidapi Version

| Source | Declared |
|--------|----------|
| qmkonnect `Cargo.toml` | `"2.6"` |
| qmk-notifier crate `Cargo.toml` | `"2.4.1"` |
| Cargo.lock (unified) | `2.6.6` |

The unified build uses 2.6.6 — specs assert "QMKonnect links hidapi 2.6" which is
accurate for the resolved build. Low risk.

### Open Behavior (R-COEX basis)
- **Windows:** `FILE_SHARE_READ | FILE_SHARE_WRITE` (default `CreateFileW` flags in hidapi)
- **macOS:** `kIOHIDOptionsTypeNone` (default IOKit open)
- **Linux:** plain `open()` on hidraw (default O_RDWR, no exclusive)

All three are **non-seize**. hidapi 2.x does NOT expose any seize/exclusive API.
The crate's `info.open_device(api)` uses this default path.

---

## Other Dependencies

| Crate | Version | Role |
|-------|---------|------|
| `tray-icon` | (latest) | macOS/Windows tray icon + menu |
| `tao` | (latest) | macOS/Windows event loop (EventLoopBuilder\<UserEvent\>) |
| `ksni` | (linux-tray feature) | Linux StatusNotifierItem D-Bus tray |
| `zenity` (CLI) | system | Linux settings dialog (shelled out) |
| `notify-send` (CLI) | system | Linux desktop notifications |
| `objc` | (macOS) | Objective-C runtime for NSAlert/NSWindow |
| `windows` | (Windows) | Win32 API for dialog windows |
| `serde` / `toml` | (all) | Config file serialization |
| `crossbeam`/`parking_lot` | (all) | Concurrency primitives |

## R-COEX Invariant — What Must Be Preserved

1. **Never introduce a seize/exclusive open.** All HID handles must use the
   default shared open (already structurally enforced — hidapi offers no seize API).
2. **Never a perpetual blocking read.** Reads must be bounded drains
   (`read_timeout(0)` non-blocking, max `IN_DRAIN_MAX=32`) around writes.
3. **First emitted payload byte is always `0x81`.** This is the magic header that
   firmware uses to distinguish QMKonnect traffic. VIA ignores 0x81-prefixed input.

The invariant is **true-by-construction** today. The work is to **document it
(comments at open sites) and assert it (tests on the emission path)**.
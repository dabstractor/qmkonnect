# SPEC — Raw HID Wire Protocol & Transport

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
v0.3.0 typed-command path reads and parses them — see §8.)

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
| Typed discriminator (round B) | `0xF0` | `data[2]` after `0x81 0x9F` ⇒ typed cmd (§8) |
| Typed response marker (round B) | `0x51` | vs legacy `0`/`1` match-bool (§8) |

---

## 8. Typed-Command Namespace (round B / v0.3.0)

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

- `proto_ver`: `1` = legacy/multi-OS firmware (today); `2` = round-B firmware.
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

The `qmk_notifier` crate (v0.3.0) frames these and returns a parsed
`CommandResponse`; see the crate `PRD.md` §10.

---

*Continue with `SPEC_PLATFORMS.md`.*

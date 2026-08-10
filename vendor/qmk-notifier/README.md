# QMK Notifier Tool

A high-performance Rust application that sends string commands to QMK keyboards via Raw HID interface.

## Overview

I built `qmk_notifier` to communicate with QMK-powered keyboards from your desktop. It efficiently handles serializing messages and sending them in batches to overcome the 32-byte HID report size limitation, enabling seamless communication with your keyboard for dynamic layer and command management.

This crate is the **transport layer** of a three-part ecosystem (see `PRD.md` for the full contract):
- **qmk_notifier** (this crate): Rust library + CLI that owns Raw-HID wire framing, the device cache, burst-write, and the v0.3.0 typed-command transport (`QueryInfo`, `QueryCallback`, `SetOs`, `ApplyHostContext`) alongside the legacy window-string path, plus reply parsing. Transport only — it does no matching.
- **[qmkonnect](https://github.com/dabstractor/qmkonnect)**: Cross-platform desktop daemon that detects the foreground window, runs the host-side matcher (`rules.toml`), and sends via this crate.
- **[qmk-notifier](https://github.com/dabstractor/qmk-notifier)**: QMK firmware module that receives, pattern-matches, and toggles layers/features. It owns the canonical wire protocol.

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/dabstractor/qmk_notifier.git
cd qmk_notifier

# Build the project
cargo build --release

# The binary will be available at target/release/qmk_notifier
```

### Dependencies

Make sure you have the HID API development libraries installed:

```bash
# For Debian/Ubuntu
sudo apt install libhidapi-dev libudev-dev

# For Fedora
sudo dnf install hidapi-devel

# For Arch Linux
sudo pacman -S hidapi

# For macOS
brew install hidapi
```

## Usage

VID/PID are optional (omit them for auto-discovery by usage page/usage — the zero-config path). Only the message (or `--list`) is required:

```bash
# Send a message — auto-discover any QMK keyboard (usage page 0xFF60 / usage 0x61)
qmk_notifier "your_message_here"

# Disambiguate among multiple QMK keyboards with explicit VID/PID
qmk_notifier --vendor-id 0xFEED --product-id 0x0000 "your_message_here"

# List all connected HID devices
qmk_notifier --list

# Query a typed-capable board's capability info (proto_ver, feature_flags, callback_count)
qmk_notifier --query-info

# Enumerate the firmware callback registry (QueryInfo then a QueryCallback sweep)
qmk_notifier --list-callbacks

# Enable verbose output
qmk_notifier -v "your_message_here"
```

## Command Line Options

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `message` | | — | Message to send to keyboard (positional; ETX appended automatically). |
| `--vendor-id` | `-i` | auto (`None`) | USB vendor ID, decimal or `0xHEX` (e.g. `65261` or `0xFEED`). Omit ⇒ match any. |
| `--product-id` | `-p` | auto (`None`) | USB product ID. Omit ⇒ match any. |
| `--usage-page` | `-u` | `0xFF60` | HID usage page, decimal or `0xHEX`. |
| `--usage` | `-a` | `0x61` | HID usage, decimal or `0xHEX`. |
| `--verbose` | `-v` | off | Enable verbose transport logging. |
| `--list` | `-l` | off | List all available HID devices. |
| `--query-info` | | off | Query device capability info (QUERY_INFO, cmd 0x01). |
| `--list-callbacks` | | off | List firmware callback registry (QueryInfo + QueryCallback sweep). |
| `--help` | | | Display help information. |

*One of `message`, `--list`, `--query-info`, or `--list-callbacks` must be provided.

## Default Reference Values

VID/PID are **optional** — omit them for **auto-discovery** (the library and CLI
default to `None`, matching any device by usage page/usage). The `0xFEED`/`0x0000`
values below are **reference/typical values** (the `DEFAULT_VENDOR_ID` /
`DEFAULT_PRODUCT_ID` constants), useful when you need to disambiguate among
multiple QMK keyboards — they are NOT the matching default.

- Vendor ID: `0xFEED` (65261 decimal) — *reference value; default is `None` (match any)*
- Product ID: `0x0000` (0 decimal) — *reference value; default is `None` (match any)*
- Usage Page: `0xFF60` (65376 decimal) — always required; this is the QMK Raw-HID default
- Usage: `0x61` (97 decimal) — always required; this is the QMK Raw-HID default
- Typed-command and legacy-string messages are both terminated with ETX (`0x03`)

## Programmatic Usage

This package can also be used as a library in other Rust projects:

```rust
use qmk_notifier::{RunParameters, RunCommand, HostOs, run};

// Query a typed-capable board's capability info — auto-discover
// (VID/PID = None ⇒ match any QMK keyboard by usage page/usage).
let params = RunParameters::new(
    RunCommand::QueryInfo,
    None,              // vendor_id  (Some(0xFEED) to disambiguate)
    None,              // product_id (Some(0x0000) to disambiguate)
    0xFF60,            // usage_page
    0x61,              // usage
    false,             // verbose
);

// run() returns Result<CommandResponse, QmkError>. The variant depends on the
// command and the device's reply:
match run(params) {
    Ok(qmk_notifier::CommandResponse::Info { proto_ver, feature_flags, .. }) => {
        println!("typed-capable: proto {proto_ver}, flags 0x{feature_flags:02X}");
    }
    Ok(qmk_notifier::CommandResponse::Timeout) => {
        // No reply within the bounded read — a legacy/offline device.
        // The caller treats this as a non-capable board and stays in string-only mode.
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

The legacy `SendMessage("…".to_string())` path still works the same way — it returns
`CommandResponse::Legacy { matched }` (`response[0]` ∈ {0,1}) when the firmware
replies, or `Timeout` when it does not:

`run()` returns [`Result<CommandResponse, QmkError>`](PRD.md). The variants:
`Legacy { matched }` (legacy string reply, `response[0]` ∈ {0,1}),
`Info { proto_ver, feature_flags, callback_count, board_rules_present }`
(QUERY_INFO), `CallbackName { index, name }` (QUERY_CALLBACK),
`Ack { ok }` (SET_OS / APPLY_HOST_CONTEXT), and `Timeout` (no reply within the
bounded read — a non-capable/offline device). See `PRD.md` §7 and §10.

## Technical Details

- I wrote this in Rust for maximum performance and reliability
- Uses the `hidapi` crate for cross-platform HID communication
- Each report is 32 logical bytes (a 33-byte hidapi buffer with a leading `0x00`
  report-ID byte); messages longer than 30 payload bytes are automatically batched
  across multiple reports, terminated by ETX (`0x03`)
- Includes proper error handling and device detection (zero-config auto-discovery
  by usage page/usage when VID/PID are omitted)
- **Typed-command transport (v0.3.0):** commands are framed as
  `[0x81][0x9F][0xF0][cmd_id][args…][0x03]` — the `0x81 0x9F` magic header, a `0xF0`
  typed discriminator (which can never begin a real matched string, so legacy
  firmware safely no-matches it), the command byte (`0x01` QueryInfo, `0x02`
  QueryCallback, `0x03` SetOs, `0x05` ApplyHostContext), args, and ETX. Typed
  commands reuse the same multi-report burst-write and device cache as legacy strings
- **Reply capture & parsing (v0.3.1):** the firmware emits one 32-byte reply per
  report processed, so after each burst-write the crate reads up to `batch_count`
  replies and retains the **last** non-empty one — the reply to the ETX-terminating
  report, which carries the real result. (A non-blocking IN-buffer drain **before**
  each send first flushes any stale replies left by a prior command, so they cannot
  contaminate the capture.) `response[0] == 0x51` ⇒ typed reply (decoded by the
  command-echo byte into `Info`/`CallbackName`/`Ack`); `0`/`1` ⇒ legacy match-bool;
  no reply ⇒ `Timeout` (a non-capable/offline device — the caller stays in
  string-only mode)
- Configurable for any QMK keyboard with Raw HID support

## Integration with QMK Keyboards

This tool is designed to work with my [qmk-notifier](https://github.com/dabstractor/qmk-notifier) QMK module. For setup instructions, refer to the qmk-notifier README.

To use this with QMK, your keyboard firmware must:
1. Have Raw HID enabled ([QMK Raw HID Documentation](https://docs.qmk.fm/#/feature_rawhid))
2. Include the qmk-notifier module

## Example Use Cases

- Automatically change keyboard layers based on active application
- Toggle features like Vim mode when specific applications gain focus
- Create application-specific macros and shortcuts
- Set up dynamic key remapping based on context

## Why Rust?

I chose Rust for this project to ensure:
- Maximum performance with minimal overhead
- Memory safety without garbage collection
- Cross-platform compatibility
- Reliable and predictable behavior

## Related Projects

- **[qmk-notifier](https://github.com/dabstractor/qmk-notifier)**: My QMK module that receives and processes the commands from this tool
- **[hyprland-qmk-window-notifier](https://github.com/dabstractor/hyprland-qmk-window-notifier)**: My Wayland companion tool for automatic application detection
- **[zigotica/active-app-qmk-layer-updater](https://github.com/zigotica/active-app-qmk-layer-updater)**: Similar tool for Windows, macOS, and X11 environments

## Contributing

Feel free to submit issues or pull requests on GitHub.

## License

[MIT License](LICENSE)

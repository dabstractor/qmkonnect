mod core;
pub use core::{
    list_hid_devices, parse_hex_or_decimal, send_raw_report, DEFAULT_PRODUCT_ID, DEFAULT_USAGE,
    DEFAULT_USAGE_PAGE, DEFAULT_VENDOR_ID, REPORT_LENGTH,
};

use clap::{Arg, ArgAction, ArgGroup, ArgMatches, Command};

// Export our error type
mod error;
pub use error::QmkError;

/// Command types for the QMK notifier.
///
/// `SendMessage`/`ListDevices` are the legacy path. The typed variants carry the
/// host-side-rules typed-command protocol (PRD §3, §10; framing in §10.1;
/// canonical wire layout in `firmware_wire_contract.md` §Command Table).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunCommand {
    /// Legacy path: send the `"{class}\x1D{title}"` window string (this crate
    /// appends the `0x03` ETX terminator before framing). Not a typed command.
    SendMessage(String),
    /// List all HID devices visible to hidapi (no keyboard I/O).
    ListDevices,

    /// Typed command `0x01` — `QUERY_INFO`. No request args. Replies with
    /// `[0x51][0x01][proto_ver][feature_flags][callback_count][board_rules_present]`.
    /// See PRD §10.1 (Framing) and `firmware_wire_contract.md` §Command Table.
    QueryInfo,
    /// Typed command `0x02` — `QUERY_CALLBACK`. `index` is the firmware callback
    /// registry slot to read. Replies with `[0x51][0x02][index][name, NUL-padded]`.
    /// See PRD §10.1 and `firmware_wire_contract.md` §Command Table.
    QueryCallback(u8),
    /// Typed command `0x03` — `SET_OS`. Declares the host OS to the keyboard at
    /// connect time. Serialized as `[0xF0][0x03][os_byte][0x03]` where
    /// `os_byte = HostOs::X as u8` (build_command_data, P1.M2.T1).
    /// See PRD §10.1 and `firmware_wire_contract.md` §SET_OS request.
    SetOs(HostOs),
    /// Typed command `0x05` — `APPLY_HOST_CONTEXT`. Pushes the host's desired
    /// layer + enabled-callback set + clear-board flag to the firmware in one
    /// atomic command. Serialized as
    /// `[0xF0][0x05][layer][flags][count][id0][id1]…[0x03]` (build_command_data,
    /// P1.M2.T1).
    ///
    /// - `layer: Option<u8>` — `None` ⇒ wire byte `0xFF` (clear host layer);
    ///   `Some(n)` ⇒ host-layer number (`>= 224` by convention, `HOST_LAYER_BASE`).
    /// - `callbacks: Vec<u8>` — the FULL desired enabled callback-id set; the
    ///   firmware diffs this against the current set (disable-before-enable).
    ///   Uncapped; may span multiple reports.
    /// - `clear_board: bool` — `true` ⇒ set firmware `flags` bit 0
    ///   (`clear_board`): firmware clears the board layer/command before applying.
    ///
    /// See PRD §10.1 and `firmware_wire_contract.md` §APPLY_HOST_CONTEXT request.
    ApplyHostContext {
        layer: Option<u8>,
        callbacks: Vec<u8>,
        clear_board: bool,
    },
}

/// Host operating system, mirrors QMK's `os_variant_t`.
/// Sent via SET_OS (cmd 0x03) to declare the host OS at connect.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOs {
    /// `0` — OS not yet detected / unknown. Mirrors QMK `OS_UNSURE`.
    Unsure = 0,
    /// `1` — Linux host. Mirrors QMK `OS_LINUX`.
    Linux = 1,
    /// `2` — Windows host. Mirrors QMK `OS_WINDOWS`.
    Windows = 2,
    /// `3` — macOS host. Mirrors QMK `OS_MACOS`.
    Macos = 3,
    /// `4` — iOS host. Mirrors QMK `OS_IOS`.
    Ios = 4,
}

/// Parsed device reply (see PRD §8 and §10.2; canonical byte layouts in
/// `firmware_wire_contract.md` §Field Definitions and §Reply Disambiguation).
///
/// Produced by `parse_reply` (P1.M2.T2) from a single 32-byte IN report read
/// after a command burst. `response[0]` disambiguates the reply: `0x51` ⇒ typed
/// reply (decoded by the `response[1]` cmd echo); `0`/`1` ⇒ legacy match-bool;
/// no reply within the bounded `read_timeout` ⇒ `Timeout`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandResponse {
    /// Legacy string reply: `response[0]` is `0` (no match) or `1` (matched).
    /// Returned for `SendMessage`, and for a typed command answered by a
    /// non-capable (legacy) device that walks the typed bytes as a no-match
    /// string. See PRD §8, §10.2.
    Legacy { matched: bool },
    /// `QUERY_INFO` (cmd `0x01`) typed reply:
    /// `[0x51][0x01][proto_ver][feature_flags][callback_count][board_rules_present]`.
    /// See `firmware_wire_contract.md` §QUERY_INFO response.
    Info {
        proto_ver: u8,
        feature_flags: u8,
        callback_count: u8,
        board_rules_present: bool,
    },
    /// `QUERY_CALLBACK` (cmd `0x02`) typed reply:
    /// `[0x51][0x02][index][name bytes, NUL-padded]`. `name` is `None` when the
    /// callback has no name or the index is out of range (the firmware emits an
    /// immediate `0x00` NUL at the name position). See `firmware_wire_contract.md`
    /// §QUERY_CALLBACK response.
    CallbackName { index: u8, name: Option<String> },
    /// `SET_OS` (cmd `0x03`) / `APPLY_HOST_CONTEXT` (cmd `0x05`) typed reply:
    /// `[0x51][cmd_echo][ack]`. `ok` is `true` when `ack == 1` (applied). Shared
    /// by both ack-style commands. See `firmware_wire_contract.md` §SET_OS /
    /// §APPLY_HOST_CONTEXT response.
    Ack { ok: bool },
    /// No reply arrived within the bounded `read_timeout` — the device is legacy
    /// or offline. The caller treats this as a non-capable device and stays in
    /// string-only mode. See PRD §10.2, §8.
    Timeout,
}

/// Parameters required for running QMK notifier operations
#[derive(Debug, Clone)]
pub struct RunParameters {
    pub command: RunCommand,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub usage_page: u16,
    pub usage: u16,
    pub verbose: bool,
}

impl RunParameters {
    /// Create new RunParameters with all required fields
    ///
    /// `vendor_id` and `product_id` are `Option<u16>`: `None` means "match any"
    /// device (auto-discovery by usage page/usage). `usage_page` and `usage` are
    /// always required and act as the primary identifier.
    pub fn new(
        command: RunCommand,
        vendor_id: Option<u16>,
        product_id: Option<u16>,
        usage_page: u16,
        usage: u16,
        verbose: bool,
    ) -> Self {
        Self {
            command,
            vendor_id,
            product_id,
            usage_page,
            usage,
            verbose,
        }
    }
}

/// Build the clap `Command` for `qmk_notifier` (PRD §11 *CLI*). Pure: it only
/// configures the parser; the actual `get_matches()`/`try_get_matches_from()`
/// call is made by the caller (so tests can use the no-exit `try_*` form).
fn build_cli_command() -> Command {
    Command::new("QMK Keyboard Communication Tool")
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about("Sends raw HID reports to QMK keyboards; --query-info / --list-callbacks diagnose typed-capable boards")
        .arg(
            Arg::new("message")
                .help("Message to send to keyboard")
                .index(1),
        )
        .arg(
            Arg::new("vendor-id")
                .short('i')
                .long("vendor-id")
                .value_name("VID")
                .help("USB vendor ID (decimal or 0xHEX format) [default: auto (match any)]")
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("product-id")
                .short('p')
                .long("product-id")
                .value_name("PID")
                .help("USB product ID (decimal or 0xHEX format) [default: auto (match any)]")
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("usage-page")
                .short('u')
                .long("usage-page")
                .value_name("USAGE_PAGE")
                .help("HID usage page (decimal or 0xHEX format) [default: 0xFF60]")
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("usage")
                .short('a')
                .long("usage")
                .value_name("USAGE")
                .help("HID usage (decimal or 0xHEX format) [default: 0x61]")
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Enable verbose output")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("list")
                .short('l')
                .long("list")
                .help("List all HID devices")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("create-config")
                .long("create-config")
                .help("Create example configuration file (REMOVED)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("query-info")
                .long("query-info")
                .help("Query device capability info (QUERY_INFO, cmd 0x01)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("list-callbacks")
                .long("list-callbacks")
                .help("List the firmware callback registry (runs QUERY_INFO, then sweeps QUERY_CALLBACK)")
                .action(ArgAction::SetTrue),
        )
        // The four "action" selectors are mutually exclusive: at most one may be
        // present (clap rejects combinations). `required(false)` lets zero through
        // so `--verbose`-alone reaches select_command's MissingRequiredParameter
        // arm (and no-args at all still triggers arg_required_else_help above).
        .group(
            ArgGroup::new("action")
                .args(["message", "list", "create-config", "query-info", "list-callbacks"])
                .multiple(false)
                .required(false),
        )
        // Show help when no arguments are provided
        .arg_required_else_help(true)
}

/// Resolve the parsed CLI matches into a single [`RunCommand`].
///
/// Priority (PRD §11; the `action` ArgGroup makes these mutually exclusive, so
/// the order is defensive): `--create-config` (removed-feature trap) > `--list` >
/// `--query-info` > `--list-callbacks` > `message` positional. `--query-info` and
/// `--list-callbacks` BOTH map to [`RunCommand::QueryInfo`] — the library sees no
/// difference (both run a single QUERY_INFO). The CLI-only callback sweep that
/// distinguishes `--list-callbacks` is a multi-call flow owned by `main.rs`, not
/// the library's single-command [`run`] API; `main.rs` detects the flag itself
/// (out-of-band, via `std::env::args`) because [`RunParameters`] (PRD §3) carries
/// no sweep flag.
///
/// `--create-config` is part of the `action` ArgGroup, so combining it with any
/// other action is rejected by clap as a conflict (exit 2) — consistent with the
/// rest of the group. Supplied alone it still surfaces the `RemovedFeature`
/// diagnostic.
fn select_command(matches: &ArgMatches) -> Result<RunCommand, QmkError> {
    if matches.get_flag("create-config") {
        return Err(QmkError::RemovedFeature(
            "Config file creation has been removed. All parameters must be provided explicitly."
                .to_string(),
        ));
    }
    if matches.get_flag("list") {
        return Ok(RunCommand::ListDevices);
    }
    if matches.get_flag("query-info") {
        return Ok(RunCommand::QueryInfo);
    }
    if matches.get_flag("list-callbacks") {
        return Ok(RunCommand::QueryInfo);
    }
    if let Some(message) = matches.get_one::<String>("message") {
        return Ok(RunCommand::SendMessage(message.to_string()));
    }
    Err(QmkError::MissingRequiredParameter(
        "one of message, --list, --query-info, or --list-callbacks".to_string(),
    ))
}

/// Build [`RunParameters`] from already-parsed clap matches (PRD §3, §11). Pure:
/// takes `&ArgMatches`, returns `Result<RunParameters, QmkError>` — never exits
/// the process. This is the testable core of [`parse_cli_args`].
fn parse_matches(matches: &ArgMatches) -> Result<RunParameters, QmkError> {
    // Parse parameters with defaults BEFORE resolving the action.
    //
    // VID/PID default to `None` (auto: match any device by usage page/usage).
    // When the flag is present, parse it into `Some(value)`. Usage page/usage
    // remain required and default to the QMK raw-HID convention.
    //
    // Validating the ID values first avoids a misleading ordering: a user who
    // passes e.g. `--vendor-id 0xGGGG` with no action would otherwise be told
    // they are "missing a parameter" and only discover the bad hex after adding
    // an action. Surfacing the parse error first is clearer UX.
    let vendor_id = matches
        .get_one::<String>("vendor-id")
        .map(|s| parse_hex_or_decimal(s))
        .transpose()?; // Option<u16>, None when flag absent

    let product_id = matches
        .get_one::<String>("product-id")
        .map(|s| parse_hex_or_decimal(s))
        .transpose()?;

    let usage_page = matches
        .get_one::<String>("usage-page")
        .map(|s| parse_hex_or_decimal(s))
        .transpose()?
        .unwrap_or(DEFAULT_USAGE_PAGE);

    let usage = matches
        .get_one::<String>("usage")
        .map(|s| parse_hex_or_decimal(s))
        .transpose()?
        .unwrap_or(DEFAULT_USAGE);

    let command = select_command(matches)?;

    let verbose = matches.get_flag("verbose");

    Ok(RunParameters::new(
        command, vendor_id, product_id, usage_page, usage, verbose,
    ))
}

/// Parse command-line arguments into [`RunParameters`] (PRD §3 *Public API*,
/// §11 *CLI*).
///
/// Returns the documented `Result<RunParameters, QmkError>` so a caller can do
/// `run(parse_cli_args()?)` directly. The returned [`RunParameters`] holds a
/// single [`RunCommand`] plus the device-targeting fields, ready for [`run`].
///
/// `--query-info` and `--list-callbacks` BOTH resolve to [`RunCommand::QueryInfo`]
/// (the library sees no difference — both run a single QUERY_INFO; `run` returns
/// [`CommandResponse::Info`] on a typed-capable board). The CLI-only follow-up
/// sweep that distinguishes `--list-callbacks` (loop
/// `QueryCallback(0..callback_count)` after `run` returns
/// [`CommandResponse::Info`]) is a multi-call flow owned by the binary
/// (`main.rs`), NOT the library's single-command [`run`] API. Because
/// [`RunParameters`] (PRD §3) carries no such sweep flag, `main.rs` detects
/// `--list-callbacks` itself by inspecting raw `std::env::args` out-of-band.
///
/// The action selectors (`message`, `--list`, `--create-config`, `--query-info`,
/// `--list-callbacks`) are mutually exclusive (clap `ArgGroup`). `--help`,
/// `--version`, unknown flags, and action conflicts are handled by clap's own
/// print-and-exit UX; post-parse logic errors surface as [`QmkError`]
/// (`RemovedFeature` for `--create-config`; `MissingRequiredParameter` when no
/// action is given).
pub fn parse_cli_args() -> Result<RunParameters, QmkError> {
    let matches = build_cli_command().get_matches();
    parse_matches(&matches)
}

/// Build the on-wire payload for `command` — the bytes AFTER the `0x81 0x9F`
/// magic header (which `burst_to_one` prepends per 33-byte report).
///
/// Delegates EVERY arm to [`core::build_command_data`] (the single source of
/// truth for on-wire payloads, per the architecture doc §build_command_data),
/// and preserves the SendMessage verbose length print as a side-effect. With the
/// reconciliation, `SendMessage`, the typed commands, and `ListDevices` all flow
/// through one builder; nothing is framed inline here.
fn build_payload(command: &RunCommand, verbose: bool) -> Vec<u8> {
    let data = core::build_command_data(command);
    if verbose {
        println!(
            "Message length: {} bytes (including ETX terminator)",
            data.len()
        );
    }
    data
}

/// Execute the notifier command described by `params` and return the parsed
/// device reply (PRD §3 *Public API*, §8 *Response Handling*, §10 *Typed-Command
/// Transport*).
///
/// # Dispatch
/// - [`RunCommand::ListDevices`] → calls [`list_hid_devices`] (prints the HID
///   enumeration) and returns [`CommandResponse::Timeout`]: nothing is sent over
///   the wire, so there is no reply to parse.
/// - Every other variant ([`RunCommand::SendMessage`] and the typed commands
///   [`RunCommand::QueryInfo`] / [`RunCommand::QueryCallback`] /
///   [`RunCommand::SetOs`] / [`RunCommand::ApplyHostContext`]) shares one path:
///   build the on-wire payload (`build_payload`) → burst-send it via
///   [`send_raw_report`] (device cache, multi-report burst-write, bounded reply
///   read) → parse the FIRST captured reply with `core::parse_reply`.
///
/// # Reply → `CommandResponse` mapping
/// - `Ok(Some(reply))` → `core::parse_reply(&reply)`:
///   - `SendMessage` reply (`response[0]` ∈ `{0,1}`) ⇒ [`CommandResponse::Legacy`].
///   - `0x51` typed reply ⇒ [`CommandResponse::Info`] / [`CommandResponse::CallbackName`]
///     / [`CommandResponse::Ack`] (decoded by the `response[1]` cmd-echo).
/// - `Ok(None)` (no device replied within the bounded read — legacy / offline
///   device) ⇒ [`CommandResponse::Timeout`]; the caller treats this as a
///   non-capable device and stays in string-only mode (PRD §8, §10.2).
/// - `Err(QmkError::DeviceNotFound)` ⇒ no interface matched the VID/PID/usage
///   predicate (the zero-config path matches by usage page/usage when VID/PID
///   are `None`). `Err(QmkError::PartialSendError { .. })` /
///   `Err(QmkError::SendReportError(..))` ⇒ transport failure (PRD §9).
pub fn run(params: RunParameters) -> Result<CommandResponse, QmkError> {
    match &params.command {
        RunCommand::ListDevices => {
            list_hid_devices()?;
            // ListDevices never sends over the wire, so there is no device reply
            // to parse. Timeout is the semantic match for "no reply received".
            Ok(CommandResponse::Timeout)
        }
        // Every command that sends a report shares one dispatch path: build the
        // payload → burst-send → parse the first captured reply (or Timeout if
        // no device replied within the bounded read). `command` binds to
        // &RunCommand (match ergonomics on `&params.command`), so it's passable
        // straight to build_payload / core::build_command_data.
        command @ (RunCommand::SendMessage(_)
        | RunCommand::QueryInfo
        | RunCommand::QueryCallback(_)
        | RunCommand::SetOs(_)
        | RunCommand::ApplyHostContext { .. }) => {
            if params.verbose {
                let vid = params
                    .vendor_id
                    .map(|v| format!("0x{v:04X}"))
                    .unwrap_or_else(|| "any".into());
                let pid = params
                    .product_id
                    .map(|p| format!("0x{p:04X}"))
                    .unwrap_or_else(|| "any".into());
                println!("Using VID: {vid}, PID: {pid}");
                println!(
                    "Using Usage Page: 0x{:04X}, Usage: 0x{:04X}",
                    params.usage_page, params.usage
                );
            }

            let data = build_payload(command, params.verbose);

            match send_raw_report(
                &data,
                params.vendor_id,
                params.product_id,
                params.usage_page,
                params.usage,
                params.verbose,
            )? {
                Some(reply) => Ok(core::parse_reply(&reply)),
                None => Ok(CommandResponse::Timeout),
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_os_discriminants_match_firmware_contract() {
        // Mirrors QMK os_variant_t and the SET_OS `os_byte` table in
        // plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md.
        assert_eq!(HostOs::Unsure as u8, 0);
        assert_eq!(HostOs::Linux as u8, 1);
        assert_eq!(HostOs::Windows as u8, 2);
        assert_eq!(HostOs::Macos as u8, 3);
        assert_eq!(HostOs::Ios as u8, 4);
    }

    #[test]
    fn test_run_command_query_variants_construction() {
        // QueryInfo: unit variant — construct + match.
        let q = RunCommand::QueryInfo;
        assert!(matches!(q, RunCommand::QueryInfo));

        // QueryCallback(index): the u8 is the firmware callback-registry slot.
        let c = RunCommand::QueryCallback(5);
        match c {
            RunCommand::QueryCallback(index) => assert_eq!(index, 5),
            _ => panic!("expected QueryCallback"),
        }
    }

    #[test]
    fn test_run_command_set_os_variant_construction() {
        // SetOs(HostOs): HostOs carries the os_byte source (verified separately by
        // test_host_os_discriminants_match_firmware_contract). Here we confirm the
        // payload round-trips through the variant.
        let s = RunCommand::SetOs(HostOs::Windows);
        match s {
            RunCommand::SetOs(os) => assert_eq!(os, HostOs::Windows),
            _ => panic!("expected SetOs"),
        }
    }

    #[test]
    fn test_run_command_apply_host_context_construction() {
        // layer == None ⇒ clear-host-layer path (wire byte 0xFF).
        let clear = RunCommand::ApplyHostContext {
            layer: None,
            callbacks: vec![1, 2, 3],
            clear_board: true,
        };
        match clear {
            RunCommand::ApplyHostContext {
                layer,
                callbacks,
                clear_board,
            } => {
                assert_eq!(layer, None, "None must mean clear-host-layer (0xFF)");
                assert_eq!(callbacks, vec![1, 2, 3]);
                assert!(clear_board, "clear_board flag must round-trip");
            }
            _ => panic!("expected ApplyHostContext"),
        }

        // layer == Some(n) ⇒ host-layer number (>= 224 by convention).
        let set = RunCommand::ApplyHostContext {
            layer: Some(224), // HOST_LAYER_BASE
            callbacks: Vec::new(),
            clear_board: false,
        };
        match set {
            RunCommand::ApplyHostContext {
                layer,
                callbacks,
                clear_board,
            } => {
                assert_eq!(layer, Some(224));
                assert!(callbacks.is_empty());
                assert!(!clear_board);
            }
            _ => panic!("expected ApplyHostContext"),
        }
    }

    #[test]
    fn test_run_parameters_creation() {
        let params = RunParameters::new(
            RunCommand::SendMessage("test".to_string()),
            Some(0xFEED),
            Some(0x0000),
            0xFF60,
            0x61,
            true,
        );

        assert_eq!(params.vendor_id, Some(0xFEED));
        assert_eq!(params.product_id, Some(0x0000));
        assert_eq!(params.usage_page, 0xFF60);
        assert_eq!(params.usage, 0x61);
        assert!(params.verbose);

        match params.command {
            RunCommand::SendMessage(msg) => assert_eq!(msg, "test"),
            _ => panic!("Expected SendMessage command"),
        }
    }

    #[test]
    fn test_run_parameters_auto_discovery() {
        // VID/PID omitted -> None means "match any" (the keystone path).
        let params = RunParameters::new(
            RunCommand::SendMessage("auto".to_string()),
            None,
            None,
            DEFAULT_USAGE_PAGE,
            DEFAULT_USAGE,
            false,
        );

        assert_eq!(params.vendor_id, None);
        assert_eq!(params.product_id, None);
        assert_eq!(params.usage_page, DEFAULT_USAGE_PAGE);
        assert_eq!(params.usage, DEFAULT_USAGE);
    }

    #[test]
    fn test_run_parameters_list_devices() {
        let params = RunParameters::new(
            RunCommand::ListDevices,
            Some(0x1234),
            Some(0x5678),
            0xABCD,
            0xEF01,
            false,
        );

        match params.command {
            RunCommand::ListDevices => {}
            _ => panic!("Expected ListDevices command"),
        }
        assert!(!params.verbose);
    }

    #[test]
    fn test_parse_hex_or_decimal_hex() {
        assert_eq!(parse_hex_or_decimal("0xFEED").unwrap(), 0xFEED);
        assert_eq!(parse_hex_or_decimal("0X1234").unwrap(), 0x1234);
    }

    #[test]
    fn test_parse_hex_or_decimal_decimal() {
        assert_eq!(parse_hex_or_decimal("1234").unwrap(), 1234);
        assert_eq!(parse_hex_or_decimal("65261").unwrap(), 65261); // 0xFEED in decimal
    }

    #[test]
    fn test_parse_hex_or_decimal_invalid() {
        assert!(parse_hex_or_decimal("0xGGGG").is_err());
        assert!(parse_hex_or_decimal("invalid").is_err());
        assert!(parse_hex_or_decimal("").is_err());
    }

    #[test]
    fn test_run_with_list_devices_command() {
        let params = RunParameters::new(
            RunCommand::ListDevices,
            Some(0xFEED),
            Some(0x0000),
            0xFF60,
            0x61,
            false,
        );

        // This should not panic and should return Ok or an appropriate error
        // We can't test the actual device listing without real hardware
        let result = run(params);
        // The function should complete without panicking
        // Result may be Ok or Err depending on system HID devices
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_run_with_send_message_command() {
        // Explicit VID/PID still works (disambiguation). Use a bogus VID/PID
        // (matching the dispatch-proof tests below) so this test stays hermetic
        // and never side-effects on a real `0xFEED:0x0000` keyboard that may be
        // plugged into a developer's machine during `cargo test`.
        let params = RunParameters::new(
            RunCommand::SendMessage("test message".to_string()),
            Some(0xDEAD),
            Some(0xBEEF),
            0xFF60,
            0x61,
            false,
        );

        // This will likely fail with DeviceNotFound unless the exact device exists
        let result = run(params);
        match result {
            Ok(_) => {
                // Success is also acceptable if a device is connected. The reply
                // is decoded by parse_reply; we deliberately do NOT assert its
                // shape here (no hardware in CI).
            }
            Err(QmkError::DeviceNotFound { .. }) => {
                // Expected when no devices are found
            }
            Err(QmkError::PartialSendError { .. }) => {
                // Expected if some devices fail
            }
            Err(e) => {
                // Other errors might occur depending on the environment
                println!("An unexpected error occurred: {}", e);
            }
        }
    }

    #[test]
    fn test_run_with_verbose_output() {
        let params = RunParameters::new(
            RunCommand::SendMessage("verbose test".to_string()),
            Some(0xDEAD),
            Some(0xBEEF),
            0xABCD,
            0xEF01,
            true, // verbose = true
        );

        // Test that verbose flag is properly handled
        let result = run(params);
        // Should handle verbose output without panicking
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_command_response_info_construction() {
        // QUERY_INFO reply: proto_ver=2 (typed-capable), feature_flags=0x03
        // (APPLY_HOST_CONTEXT | callback registry), 5 callbacks, board map present.
        let info = CommandResponse::Info {
            proto_ver: 2,
            feature_flags: 0x03,
            callback_count: 5,
            board_rules_present: true,
        };
        match info {
            CommandResponse::Info {
                proto_ver,
                feature_flags,
                callback_count,
                board_rules_present,
            } => {
                assert_eq!(proto_ver, 2);
                assert_eq!(feature_flags, 0x03);
                assert_eq!(callback_count, 5);
                assert!(board_rules_present);
            }
            _ => panic!("expected Info"),
        }
        // PartialEq/Eq derive (mandated by the item) must hold for the result type.
        assert_eq!(
            info,
            CommandResponse::Info {
                proto_ver: 2,
                feature_flags: 0x03,
                callback_count: 5,
                board_rules_present: true,
            }
        );
    }

    #[test]
    fn test_command_response_callback_name_construction() {
        // Named callback: index echoed back, ASCII name present.
        let named = CommandResponse::CallbackName {
            index: 3,
            name: Some("layer_tap".to_string()),
        };
        // Bind by reference so `named` stays intact for the PartialEq/Eq
        // assertions below (CommandResponse owns an Option<String> and is
        // intentionally non-Copy — see PRP gotchas).
        match named {
            CommandResponse::CallbackName { index, ref name } => {
                assert_eq!(index, 3);
                assert_eq!(name.as_deref(), Some("layer_tap"));
            }
            _ => panic!("expected CallbackName"),
        }

        // Unnamed / out-of-range callback: firmware emits an immediate NUL ⇒ None.
        let unnamed = CommandResponse::CallbackName {
            index: 99,
            name: None,
        };
        assert_eq!(
            unnamed,
            CommandResponse::CallbackName {
                index: 99,
                name: None
            }
        );
        assert_ne!(named, unnamed, "distinct index/name must not compare equal");
    }

    #[test]
    fn test_command_response_legacy_ack_timeout_construction() {
        // Legacy match-bool reply (response[0] ∈ {0,1}).
        let matched = CommandResponse::Legacy { matched: true };
        let no_match = CommandResponse::Legacy { matched: false };
        assert_eq!(matched, CommandResponse::Legacy { matched: true });
        assert_ne!(matched, no_match);

        // SET_OS / APPLY_HOST_CONTEXT ack reply (ack==1 ⇒ applied).
        let ok = CommandResponse::Ack { ok: true };
        let fail = CommandResponse::Ack { ok: false };
        assert_eq!(ok, CommandResponse::Ack { ok: true });
        assert_ne!(ok, fail);

        // No reply within read_timeout (device legacy/offline).
        let t = CommandResponse::Timeout;
        assert_eq!(t, CommandResponse::Timeout);

        // Cross-variant inequality: different variants must never compare equal
        // (sanity-check the derived PartialEq across the whole enum).
        assert_ne!(CommandResponse::Timeout, CommandResponse::Ack { ok: false });
    }

    #[test]
    fn test_run_query_info_dispatches_to_send() {
        // Typed dispatch must BUILD + SEND (not `todo!()` panic). A bogus VID/PID
        // guarantees the device filter (`vendor_id.is_none_or(|v| dev_vid == v)`)
        // matches NOTHING on any machine — even one with a real QMK keyboard — so
        // `send_raw_report` deterministically returns `DeviceNotFound`. A
        // `todo!()` would have panicked and failed this test, so the assertion
        // proves the arm wired through to `send_raw_report`. Reply capture +
        // parsing land in P1.M1.T3; that is out of scope here.
        let params = RunParameters::new(
            RunCommand::QueryInfo,
            Some(0xDEAD),
            Some(0xBEEF),
            DEFAULT_USAGE_PAGE,
            DEFAULT_USAGE,
            false,
        );
        let result = run(params);
        assert!(
            matches!(result, Err(QmkError::DeviceNotFound { .. })),
            "QueryInfo must dispatch to send_raw_report; expected DeviceNotFound with bogus VID/PID, got {result:?}",
        );
    }

    #[test]
    fn test_run_query_callback_dispatches_to_send() {
        // Same dispatch proof as QueryInfo, but for an arg-carrying variant
        // (index = 5). build_command_data correctness is S1's job; here we only
        // assert the arm reaches send_raw_report.
        let params = RunParameters::new(
            RunCommand::QueryCallback(5),
            Some(0xDEAD),
            Some(0xBEEF),
            DEFAULT_USAGE_PAGE,
            DEFAULT_USAGE,
            false,
        );
        let result = run(params);
        assert!(
            matches!(result, Err(QmkError::DeviceNotFound { .. })),
            "QueryCallback must dispatch to send_raw_report; expected DeviceNotFound, got {result:?}",
        );
    }

    #[test]
    fn test_run_set_os_dispatches_to_send() {
        // Arg-carrying variant: HostOs::Linux ⇒ os_byte 1. Proves SetOs dispatches.
        let params = RunParameters::new(
            RunCommand::SetOs(HostOs::Linux),
            Some(0xDEAD),
            Some(0xBEEF),
            DEFAULT_USAGE_PAGE,
            DEFAULT_USAGE,
            false,
        );
        let result = run(params);
        assert!(
            matches!(result, Err(QmkError::DeviceNotFound { .. })),
            "SetOs must dispatch to send_raw_report; expected DeviceNotFound, got {result:?}",
        );
    }

    #[test]
    fn test_run_apply_host_context_dispatches_to_send() {
        // Struct-arg variant: layer=Some(224), 3 callbacks, clear_board=true.
        // Exercises the multi-field payload path through build_command_data and
        // proves ApplyHostContext dispatches.
        let params = RunParameters::new(
            RunCommand::ApplyHostContext {
                layer: Some(224),
                callbacks: vec![1, 2, 3],
                clear_board: true,
            },
            Some(0xDEAD),
            Some(0xBEEF),
            DEFAULT_USAGE_PAGE,
            DEFAULT_USAGE,
            false,
        );
        let result = run(params);
        assert!(
            matches!(result, Err(QmkError::DeviceNotFound { .. })),
            "ApplyHostContext must dispatch to send_raw_report; expected DeviceNotFound, got {result:?}",
        );
    }

    // ---- P1.M4.T1.S1: --query-info / --list-callbacks flag parsing ----
    // Build ArgMatches via the no-exit try_get_matches_from (NEVER get_matches* in a
    // test — it exits the process on error), then exercise the pure parse_matches.
    fn cli_for(args: &[&str]) -> RunParameters {
        let matches = build_cli_command()
            .try_get_matches_from(args)
            .expect("test args should parse");
        parse_matches(&matches).expect("test args should resolve to RunParameters")
    }

    #[test]
    fn test_parse_query_info_flag() {
        let params = cli_for(&["qmk_notifier", "--query-info"]);
        assert!(matches!(params.command, RunCommand::QueryInfo));
        // Defaults preserved.
        assert_eq!(params.usage_page, DEFAULT_USAGE_PAGE);
        assert_eq!(params.usage, DEFAULT_USAGE);
        assert_eq!(params.vendor_id, None);
        assert_eq!(params.product_id, None);
    }

    #[test]
    fn test_parse_list_callbacks_flag() {
        let params = cli_for(&["qmk_notifier", "--list-callbacks"]);
        // --list-callbacks maps to QueryInfo (the library sees no difference from
        // --query-info). The callback sweep is now a CLI-only concern detected
        // out-of-band by main.rs via std::env::args; RunParameters carries no
        // sweep flag, so there is nothing further to assert here.
        assert!(matches!(params.command, RunCommand::QueryInfo));
    }

    #[test]
    fn test_query_info_combines_with_device_flags() {
        // Device-targeting flags are orthogonal to the action group.
        let params = cli_for(&[
            "qmk_notifier",
            "--query-info",
            "--vendor-id",
            "0xFEED",
            "-v",
        ]);
        assert!(matches!(params.command, RunCommand::QueryInfo));
        assert_eq!(params.vendor_id, Some(0xFEED));
        assert!(params.verbose);
    }

    #[test]
    fn test_message_and_list_still_parse() {
        let params = cli_for(&["qmk_notifier", "hello"]);
        assert!(matches!(params.command, RunCommand::SendMessage(s) if s == "hello"));

        let params = cli_for(&["qmk_notifier", "--list"]);
        assert!(matches!(params.command, RunCommand::ListDevices));
    }

    #[test]
    fn test_action_selectors_are_mutually_exclusive() {
        // Each combination must be a clap conflict error (Err, NOT a process exit —
        // try_get_matches_from returns Result).
        let cases: &[&[&str]] = &[
            &["qmk_notifier", "--query-info", "msg"],
            &["qmk_notifier", "--query-info", "--list-callbacks"],
            &["qmk_notifier", "--list-callbacks", "msg"],
            &["qmk_notifier", "--list", "--query-info"],
        ];
        for args in cases {
            assert!(
                build_cli_command()
                    .try_get_matches_from(args.iter().copied())
                    .is_err(),
                "expected clap to reject conflicting actions: {args:?}",
            );
        }
    }

    #[test]
    fn test_no_action_given_is_missing_parameter() {
        // --verbose alone: not a clap error (group is required(false)), but
        // select_command finds no action ⇒ MissingRequiredParameter.
        let matches = build_cli_command()
            .try_get_matches_from(["qmk_notifier", "--verbose"])
            .expect("--verbose alone parses at the clap level");
        let result = parse_matches(&matches);
        assert!(
            matches!(result, Err(QmkError::MissingRequiredParameter(_))),
            "expected MissingRequiredParameter, got {result:?}",
        );
    }
}

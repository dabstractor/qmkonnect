use crate::error::QmkError;
use hidapi::{HidApi, HidDevice};
use std::sync::{LazyLock, Mutex, MutexGuard};

/// Minimal HID I/O surface used by [`burst_to_one`]: the two methods it actually
/// calls on an open HID handle. Abstracting these behind a trait lets `burst_to_one`
/// be genericized over `impl RawHid` (next subtask) so its reply-capture logic can
/// be unit-tested with a `FakeHid` double instead of requiring a physical keyboard.
///
/// `pub(crate)`: an internal testability seam — intentionally NOT exported from the
/// crate. The blanket impl below delegates to `hidapi::HidDevice` unchanged, so
/// runtime behavior is identical to calling hidapi directly.
pub(crate) trait RawHid {
    /// Write a raw HID OUT report. Mirrors `hidapi::HidDevice::write`.
    fn write(&self, data: &[u8]) -> Result<usize, hidapi::HidError>;
    /// Read one IN report, blocking up to `timeout` ms. `Ok(0)` = timeout/no-data
    /// (NOT an error); `Ok(n>0)` = real read. Mirrors `hidapi::HidDevice::read_timeout`.
    fn read_timeout(&self, buf: &mut [u8], timeout: i32) -> Result<usize, hidapi::HidError>;
}

impl RawHid for hidapi::HidDevice {
    fn write(&self, data: &[u8]) -> Result<usize, hidapi::HidError> {
        // Fully-qualified-by-type: REQUIRED. A bare `self.write(data)` would resolve
        // to `RawHid::write` (the method we are defining) and recurse infinitely —
        // see "Known Gotchas". `hidapi::HidDevice::write` calls hidapi's real impl.
        hidapi::HidDevice::write(self, data)
    }
    fn read_timeout(&self, buf: &mut [u8], timeout: i32) -> Result<usize, hidapi::HidError> {
        hidapi::HidDevice::read_timeout(self, buf, timeout)
    }
}

// Default constants
pub const DEFAULT_VENDOR_ID: u16 = 0xFEED;
pub const DEFAULT_PRODUCT_ID: u16 = 0x0000;
pub const DEFAULT_USAGE_PAGE: u16 = 0xFF60;
pub const DEFAULT_USAGE: u16 = 0x61;
pub const REPORT_LENGTH: usize = 32;

// --- Typed-command transport constants (v0.3.0) -----------------------------
// Wire vocabulary for the typed-command path (PRD §10.1, §10.2). Mirror of the
// firmware `notifier.h` #defines (canonicalized in
// `plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md` §Constants).
// `pub(crate)` (not `pub`): internal transport constants, NOT public API.
//
// The 5 command constants (CMD_DISCRIMINATOR, CMD_QUERY_INFO, CMD_QUERY_CALLBACK,
// CMD_SET_OS, CMD_APPLY_HOST_CONTEXT) have a real consumer: `build_command_data`
// (P1.M1.T2.S1) references them in compiled code, so they carry no
// `#[allow(dead_code)]`. RESPONSE_MARKER is consumed by `parse_reply`;
// REPLY_READ_TIMEOUT_MS by `burst_to_one`'s bounded reply capture. `parse_reply`
// (P1.M1.T3) is consumed by `run()` (P1.M1.T3.S2), so none of these carry a
// dead-code allow once run() goes live.

/// Typed-command discriminator: first payload byte after 0x81 0x9F (PRD §10.1).
pub(crate) const CMD_DISCRIMINATOR: u8 = 0xF0;
/// Typed-response marker: response[0] == 0x51 means typed reply (PRD §10.2).
pub(crate) const RESPONSE_MARKER: u8 = 0x51;
/// Command IDs from firmware PRD §4.6 command table.
pub(crate) const CMD_QUERY_INFO: u8 = 0x01;
pub(crate) const CMD_QUERY_CALLBACK: u8 = 0x02;
pub(crate) const CMD_SET_OS: u8 = 0x03;
pub(crate) const CMD_APPLY_HOST_CONTEXT: u8 = 0x05;
/// Bounded timeout (ms) for reading each device reply after a burst
/// (`burst_to_one` reads up to `batch_count` replies at this timeout each).
/// Must be > 0 (unlike the drain's non-blocking timeout=0) so the read BLOCKS
/// for a real reply rather than polling. 1000 ms is a conservative bound; P4's
/// QUERY_CALLBACK sweep against a non-capable device may want to lower it (each
/// query against a silent device waits up to this long).
const REPLY_READ_TIMEOUT_MS: i32 = 1000;

/// ETX terminator byte appended to every on-wire payload (PRD §14 invariant 1).
/// Used by both the SendMessage (legacy string) path and the typed-command
/// path in `build_command_data`; the caller (`run`) appends NOTHING — the ETX
/// is already in the returned payload (architecture doc §build_command_data).
pub(crate) const ETX_TERMINATOR_BYTE: u8 = 0x03;

pub fn parse_hex_or_decimal(input: &str) -> Result<u16, QmkError> {
    if input.starts_with("0x") || input.starts_with("0X") {
        u16::from_str_radix(&input[2..], 16).map_err(|e| QmkError::InvalidHexValue(e.to_string()))
    } else {
        input
            .parse::<u16>()
            .map_err(|e| QmkError::InvalidDecimalValue(e.to_string()))
    }
}

pub fn list_hid_devices() -> Result<(), QmkError> {
    let api = HidApi::new().map_err(|e| QmkError::HidApiInitError(e.to_string()))?;

    println!("Available HID devices:");
    for device in api.device_list() {
        println!(
            "VID: 0x{:04X}, PID: 0x{:04X}, Usage Page: 0x{:04X}, Usage: 0x{:04X}, Path: {:?}",
            device.vendor_id(),
            device.product_id(),
            device.usage_page(),
            device.usage(),
            device.path()
        );

        match device.open_device(&api) {
            Ok(opened_device) => {
                if let Ok(Some(manufacturer)) = opened_device.get_manufacturer_string() {
                    println!("  Manufacturer: {}", manufacturer);
                }
                if let Ok(Some(product)) = opened_device.get_product_string() {
                    println!("  Product: {}", product);
                }
            }
            Err(_) => {
                println!("  (Unable to open device for more details)");
            }
        }
        println!();
    }

    Ok(())
}

/// Payload bytes carried per 32-byte raw-HID report.
///
/// Each report is laid out as `[report_id = 0, 0x81, 0x9F, <30 payload bytes…>]`
/// inside a `REPORT_LENGTH + 1` byte buffer (the leading byte is the report ID
/// demanded by HIDAPI's `write()` contract). 2 of those bytes are header
/// overhead, leaving `REPORT_LENGTH - 2` = 30 bytes of payload per report.
const PAYLOAD_PER_REPORT: usize = REPORT_LENGTH - 2;

/// Ceiling on how many IN-side reports we drain after a burst. The firmware
/// sends a 32-byte reply per report (fixed in qmk-notifier commit `01a51935`,
/// which corrected the response size from the header-stripped `30` to the full
/// `RAW_REPORT_SIZE = 32`). v0.2.x of this crate only *drains/discards* those
/// replies; the v0.3.0 typed-command path reads and parses them. Draining is
/// still required with a persistent handle so replies don't accumulate in the
/// kernel IN buffer and stall `raw_hid_send` on the device. Bounded so a
/// misbehaving IN endpoint can't wedge the notifier.
const IN_DRAIN_MAX: usize = 32;

/// On a total send failure (e.g. the keyboard was unplugged/replugged and the
/// cached handle is now stale), rebuild the device cache and retry this many
/// times before giving up. Only retries when *zero* devices succeeded, so a
/// partial send is never re-sent (no duplicate notifications).
const SEND_RETRIES: usize = 1;

/// Burst-send `data` to every raw-HID interface matching the VID/PID/usage-page/
/// usage predicate, retrying the whole send (with a cache rebuild) once on a
/// total failure. Returns the LAST device reply captured by the burst-write path
/// (the ETX-report reply).
///
/// # Return
/// - `Ok(Some(bytes))` — every matched device accepted the burst AND at least
///   one device replied within `REPLY_READ_TIMEOUT_MS` (the bounded reads in
///   `burst_to_one`). `bytes` is the last (ETX-report) reply's raw IN report (up to
///   `REPORT_LENGTH` + 1 bytes). Decode it downstream via `parse_reply`
///   into a [`crate::CommandResponse`] (PRD §8, §10.2).
/// - `Ok(None)` — the burst succeeded but NO device replied within
///   `REPLY_READ_TIMEOUT_MS` (timeout / read failure / a legacy device that
///   sends no typed reply). The caller treats `None` as a non-capable device and
///   stays in string-only mode (PRD §10.2, §8).
/// - `Err(QmkError::DeviceNotFound)` — no interface matched the predicate.
/// - `Err(QmkError::PartialSendError { succeeded, failed })` — some devices
///   accepted the burst, some did not. A partial send is NEVER retried (PRD §14
///   invariant 4), so any captured reply is discarded on this path.
/// - `Err(QmkError::SendReportError(..))` — every device failed after exhausting
///   retries.
///
/// `data` carries ONLY the payload after the `0x81 0x9F` magic header:
/// `burst_to_one` prepends that header (and the leading report-ID byte) per
/// 33-byte report. For legacy strings the caller appends the `0x03` ETX terminator
/// first; for typed commands `build_command_data` produces the
/// `[0xF0][cmd][args][0x03]` payload (PRD §4, §10.1). Multi-report burst-write
/// and the device cache are shared by all command types.
pub fn send_raw_report(
    data: &[u8],
    vendor_id: Option<u16>,
    product_id: Option<u16>,
    usage_page: u16,
    usage: u16,
    verbose: bool,
) -> Result<Option<Vec<u8>>, QmkError> {
    let key = MatchKey {
        vendor_id,
        product_id,
        usage_page,
        usage,
    };
    let batch_count = batches_for(data);

    if verbose {
        println!("Request data ({} bytes):", data.len());
        println!("{:?}", data);
    }

    for attempt in 0..=SEND_RETRIES {
        match try_send_once(&key, data, batch_count, verbose)? {
            (SendOutcome::AllSucceeded, reply) => return Ok(reply),
            (SendOutcome::Partial { succeeded, failed }, _) => {
                return Err(QmkError::PartialSendError { succeeded, failed });
            }
            (SendOutcome::TotalFailure, _) if attempt < SEND_RETRIES => {
                // The cache was already invalidated inside try_send_once; the
                // next iteration re-enumerates + reopens.
                if verbose {
                    println!(
                        "All sends failed; rebuilding device cache and retrying (attempt {}/{}).",
                        attempt + 2,
                        SEND_RETRIES + 1
                    );
                }
                continue;
            }
            (SendOutcome::TotalFailure, _) => {
                return Err(QmkError::SendReportError(hidapi::HidError::HidApiError {
                    message: "Failed to send to any devices".to_string(),
                }));
            }
        }
    }

    unreachable!("the retry loop always returns on its first or final iteration")
}

/// Outcome of a single send attempt against the cached device handles.
#[derive(Debug)]
enum SendOutcome {
    /// Every device accepted the full burst.
    AllSucceeded,
    /// Some devices succeeded, some failed.
    Partial { succeeded: usize, failed: usize },
    /// No device accepted the burst (the cache has been invalidated).
    TotalFailure,
}

/// One full attempt: ensure the cache is populated for `key`, then burst-write
/// `data` to every cached device. Invalidates the cache on any write error so
/// the next attempt/call re-enumerates + reopens.
fn try_send_once(
    key: &MatchKey,
    data: &[u8],
    batch_count: usize,
    verbose: bool,
) -> Result<(SendOutcome, Option<Vec<u8>>), QmkError> {
    let mut cache = lock_cache();
    ensure_cache(&mut cache, key, verbose)?;

    let device_count = cache.as_ref().expect("cache populated").devices.len();
    if verbose {
        println!("Found {} matching device(s).", device_count);
    }

    let mut succeeded = 0usize;
    let mut failed = 0usize;
    let mut first_reply: Option<Vec<u8>> = None; // first successful device wins

    {
        let devices: &Vec<HidDevice> = &cache.as_ref().expect("cache populated").devices;
        for (device_idx, interface) in devices.iter().enumerate() {
            if verbose {
                let device_path = match interface.get_device_info() {
                    Ok(info) => format!("{:?}", info.path()),
                    Err(_) => "N/A".to_string(),
                };
                println!(
                    "Sending to device {}/{}: Path: {}",
                    device_idx + 1,
                    device_count,
                    device_path
                );
            }

            let (success, reply) = burst_to_one(interface, data, batch_count, verbose);
            if success {
                succeeded += 1;
                if first_reply.is_none() {
                    first_reply = reply; // first successful device wins (transport_evolution.md §KDD #4)
                }
            } else {
                failed += 1;
                if verbose {
                    println!(
                        "Failed to send message to device {}/{}.",
                        device_idx + 1,
                        device_count
                    );
                }
            }
        }
    }

    // A write error means the cached handle is suspect (e.g. a replug made the
    // old fd stale). Drop the cache so the next attempt/call re-enumerates.
    if failed > 0 {
        *cache = None;
        if verbose {
            println!("Invalidating device cache after a write error.");
        }
    }

    let outcome = if succeeded == 0 {
        SendOutcome::TotalFailure
    } else if failed > 0 {
        SendOutcome::Partial { succeeded, failed }
    } else {
        SendOutcome::AllSucceeded
    };
    Ok((outcome, first_reply))
}

/// DRAIN any stale IN-side replies left by a prior send (Issue 3), then
/// burst-write `data` to a single device as `batch_count` back-to-back raw-HID
/// reports, then CAPTURE the last device reply (the ETX-report reply, which
/// carries the real result), then drain any
/// surplus IN-side reports.
///
/// Returns `(false, None)` on the first write error; otherwise `(true, reply)`,
/// where `reply` is `Some(bytes)` when a reply arrived within
/// `REPLY_READ_TIMEOUT_MS`, or `None` on timeout / read failure. The bool is the
/// write-success flag (same semantics as the pre-v0.3.0 `-> bool` form); the
/// `Option<Vec<u8>>` is the LAST captured IN report (the ETX-report reply),
/// `parse_reply` into a `CommandResponse` (PRD §8, §10.2).
///
/// Reply capture (v0.3.1): after the burst-write succeeds, up to `batch_count`
/// IN reports are read with a bounded `read_timeout(REPLY_READ_TIMEOUT_MS)` each,
/// and the LAST non-empty reply is retained. The firmware emits one 32-byte reply
/// per report processed (PRD §4.4); for a multi-report message only the final
/// reply — the ETX report — carries the real result (legacy match-bool or a
/// typed `0x51` reply), so overwriting `reply` each iteration keeps the correct
/// one. Surplus IN reports (beyond `batch_count`) are then drained non-blocking
/// (bounded by `IN_DRAIN_MAX`) as a safety net so a persistent handle does not
/// stall on accumulated replies. `read_timeout` returns `Ok(0)` on timeout/no-data
/// (NOT an error) and `Ok(n > 0)` on a real read; a timeout/error breaks the
/// capture loop early (`external_deps.md` §read_timeout semantics).
///
/// Pre-send drain (v0.3.1, Issue 3): BEFORE the burst-write, up to
/// `IN_DRAIN_MAX` IN reports are read non-blocking (`read_timeout(0)`) and
/// discarded. USB latency can deliver a *prior* send's reply into the kernel
/// IN buffer *after* that prior send's own (post-capture) drain loop ended;
/// without this pre-send flush, the current send's bounded capture read would
/// grab that stale reply instead of its own, making `CommandResponse`
/// nondeterministic across rapid sequential sends. Same loop shape as the
/// surplus drain below; `read_timeout(0)` returns `Ok(0)` on no data (NOT an
/// error), so the loop breaks after one `Ok(0)` read in the common
/// no-stale-data case (cheap: one non-blocking poll).
///
/// Burst-write is safe without a per-report ack: QMK's raw-HID OUT endpoint
/// buffers up to `RAW_OUT_CAPACITY` (4) reports and drains them all in one
/// main-loop pass (`raw_hid_task`: `while (receive_report(...))
/// raw_hid_receive(...)`). The OUT endpoint provides its own backpressure — when
/// the device buffer is full it NAKs the transfer and the host's `write()`
/// blocks until space frees. Reports are never dropped, so burst-write is safe
/// for ANY title length. See IMPLEMENTATION_PLAN.md.
///
/// HID I/O note: `interface` is generic over the [`RawHid`] trait (both
/// `hidapi::HidDevice` and the `FakeHid` test double implement it), so this
/// function's reply-capture logic is unit-testable without a physical keyboard.
fn burst_to_one<T: RawHid>(
    interface: &T,
    data: &[u8],
    batch_count: usize,
    verbose: bool,
) -> (bool, Option<Vec<u8>>) {
    let mut request_data = [0u8; REPORT_LENGTH + 1]; // stack array (was vec!)
    request_data[1] = 0x81;
    request_data[2] = 0x9F;

    // Drain any stale IN-side replies left by a prior send (Issue 3). USB latency
    // can deliver a prior send's reply after that send's drain loop ended; without
    // this pre-send flush, the next send's bounded read would capture the stale
    // reply instead of its own.
    let mut stale_buf = [0u8; REPORT_LENGTH + 1];
    for _ in 0..IN_DRAIN_MAX {
        match interface.read_timeout(&mut stale_buf, 0) {
            Ok(n) if n > 0 => continue,
            _ => break,
        }
    }

    for batch in 0..batch_count {
        let start_idx = batch * PAYLOAD_PER_REPORT;
        let end_idx = (start_idx + PAYLOAD_PER_REPORT).min(data.len());
        let batch_data = &data[start_idx..end_idx];

        request_data[3..].fill(0); // clear reused payload tail
        if !batch_data.is_empty() {
            request_data[3..3 + batch_data.len()].copy_from_slice(batch_data);
        }

        if verbose {
            println!("Sending batch {}/{}", batch + 1, batch_count);
            println!("{:?}", request_data);
        }

        if let Err(e) = interface.write(&request_data) {
            if verbose {
                println!("Error on batch {}: {}", batch + 1, e);
            }
            return (false, None);
        }
    }

    // Capture the LAST device reply with a bounded timeout (v0.3.1 fix for
    // Issue 1). The firmware emits one 32-byte reply per report processed (PRD
    // §4.4); for a multi-report message only the LAST reply — the ETX report —
    // carries the real result (legacy match-bool, or a typed 0x51 reply). So we
    // read up to batch_count replies and OVERWRITE `reply` each iteration,
    // keeping the last non-empty one. Unlike the drain below (non-blocking,
    // discards surplus), each read here WAITS up to REPLY_READ_TIMEOUT_MS for a
    // real reply. read_timeout returns Ok(0) on timeout/no-data (NOT an error)
    // and Ok(n>0) when data was read; a timeout/error breaks the loop early.
    // (PRD §4.4, §8; external_deps.md §read_timeout semantics.)
    let mut reply: Option<Vec<u8>> = None;
    let mut read_buf = [0u8; REPORT_LENGTH + 1];
    for _ in 0..batch_count.max(1) {
        match interface.read_timeout(&mut read_buf, REPLY_READ_TIMEOUT_MS) {
            Ok(n) if n > 0 => {
                reply = Some(read_buf[..n].to_vec()); // overwrite ⇒ keep LAST (ETX) reply
                if verbose {
                    println!("Captured device reply: {} bytes", n);
                }
            }
            _ => break, // Ok(0) = timeout, Err = read failure ⇒ stop (no more replies)
        }
    }

    // Drain any pending IN-side reports (non-blocking). The firmware sends a
    // valid 32-byte reply per report (qmk-notifier commit `01a51935`, which
    // fixed the response size from the header-stripped `30` to `RAW_REPORT_SIZE`
    // = 32); the old "ack silently dropped by send_raw_hid because length ==
    // RAW_EPSIZE" note described the pre-fix firmware. v0.2.x of this crate
    // discards the reply here; the v0.3.0 typed-command path reads and parses
    // it instead. Draining keeps a persistent handle from stalling on
    // accumulated replies.
    //
    // Note: read_timeout(0) returns Ok(0) on "no data" (poll times out), not an
    // error, so we break on Ok(0)/Err and only keep draining on a real read
    // (n > 0). Bounded by IN_DRAIN_MAX so a flooding IN endpoint can't wedge us.
    let mut drain_buf = [0u8; REPORT_LENGTH + 1];
    for _ in 0..IN_DRAIN_MAX {
        match interface.read_timeout(&mut drain_buf, 0) {
            Ok(n) if n > 0 => continue,
            _ => break,
        }
    }

    (true, reply)
}

/// Number of reports needed to carry `data.len()` payload bytes (0 when empty).
fn batches_for(data: &[u8]) -> usize {
    (data.len() + REPORT_LENGTH - 3) / PAYLOAD_PER_REPORT
}

/// Build the ETX-terminated on-wire payload for ANY [`crate::RunCommand`],
/// ready to hand unchanged to [`send_raw_report`].
///
/// This is the SINGLE source of truth for on-wire payloads (architecture doc
/// §build_command_data, Design Decision #1): the caller (`run()`) appends
/// NOTHING — the [`ETX_TERMINATOR_BYTE`] terminator is already in the returned
/// payload.
///
/// # Variant → payload shape
///
/// - **SendMessage** (legacy string path): returns the raw message bytes
///   followed by a single [`ETX_TERMINATOR_BYTE`] — NO `0xF0` discriminator
///   (PRD §14 invariant 1). The message may span multiple reports via
///   [`batches_for`] / [`burst_to_one`] like any other payload.
/// - **Typed commands** (`QueryInfo` / `QueryCallback` / `SetOs` /
///   `ApplyHostContext`): return `[0xF0][cmd_id][args…][ETX]` — the typed
///   discriminator (`CMD_DISCRIMINATOR` = `0xF0`), the command-ID byte,
///   command-specific argument bytes, then [`ETX_TERMINATOR_BYTE`].
///   [`send_raw_report`] / [`burst_to_one`] prepend the per-report
///   `[0x00][0x81][0x9F]` framing, so the firmware-side layout is
///   `[0x81][0x9F][0xF0][cmd_id][args…][0x03]` — the discriminator lands at
///   firmware `data[2]` (PRD §8; canonical layout in
///   `plan/001_b92a9b2b603f/architecture/firmware_wire_contract.md`
///   §Typed-Command Framing). The returned `Vec` therefore starts with `0xF0`,
///   NOT with `0x81 0x9F` (that header is added by [`burst_to_one`]).
/// - **ListDevices** is NOT a wire command (`run()` routes it to
///   [`list_hid_devices`]); this builder returns an empty `Vec` defensively so
///   a misroute is inert rather than a panic.
///
/// Per-variant arg layouts (`firmware_wire_contract.md` §Command Table):
/// - [`crate::RunCommand::QueryInfo`]        (id `0x01`): no args.
/// - [`crate::RunCommand::QueryCallback`]    (id `0x02`): `[index]`.
/// - [`crate::RunCommand::SetOs`]            (id `0x03`): `[os_byte]` where
///   `os_byte = HostOs as u8` (mirrors QMK `os_variant_t`).
/// - [`crate::RunCommand::ApplyHostContext`] (id `0x05`): `[layer][flags][count][id…]`
///   — `layer` is the host-layer number or `0xFF` (clear) when `layer == None`;
///   `flags` bit 0 is `clear_board`; `count` is the callback-id count; `id…`
///   the full desired enabled set (firmware diffs, disable-before-enable).
///
/// Consumer: [`crate::run`] via `lib.rs::build_payload` (the thin verbose
/// wrapper). This is the request-side counterpart to the reply-side
/// [`parse_reply`] (P1.M1.T3); the payload it returns feeds the unchanged
/// [`send_raw_report`] / [`burst_to_one`] send path.
pub(crate) fn build_command_data(command: &crate::RunCommand) -> Vec<u8> {
    use crate::RunCommand;

    // Legacy string path: raw message bytes + single ETX. NO 0xF0 discriminator.
    // (PRD §14 invariant 1; the crate appends the ETX, not the caller.)
    if let RunCommand::SendMessage(msg) = command {
        let mut data = msg.as_bytes().to_vec();
        data.push(ETX_TERMINATOR_BYTE);
        return data;
    }
    // Not a wire command — builder is defensive (run() never calls us for this).
    if matches!(command, RunCommand::ListDevices) {
        return Vec::new();
    }

    // Typed path: [0xF0][cmd_id][args…][ETX]
    let mut payload = Vec::new();
    payload.push(CMD_DISCRIMINATOR);

    match command {
        RunCommand::QueryInfo => {
            payload.push(CMD_QUERY_INFO);
        }
        RunCommand::QueryCallback(index) => {
            payload.push(CMD_QUERY_CALLBACK);
            payload.push(*index);
        }
        RunCommand::SetOs(os) => {
            payload.push(CMD_SET_OS);
            payload.push(*os as u8);
        }
        RunCommand::ApplyHostContext {
            layer,
            callbacks,
            clear_board,
        } => {
            payload.push(CMD_APPLY_HOST_CONTEXT);
            // layer: Some(n) ⇒ host-layer number (≥224 by convention);
            // None ⇒ 0xFF (clear host layer). See firmware_wire_contract.md.
            payload.push(layer.unwrap_or(0xFF));
            // flags: bit 0 = clear_board (firmware clears board layer/command
            // before applying host context). No other bits defined yet.
            payload.push(if *clear_board { 0x01 } else { 0x00 });
            // count: u8. Defensive CLAMP at 255 (P1.M2.T1.S2 contract; PRD §10.1
            // says the transport is uncapped while the firmware registry is
            // u8-bounded, HOST_CALLBACK_MAX=32). `as u8` alone WRAPS — e.g.
            // len==256 ⇒ count 0, telling the firmware "zero callbacks" while
            // 256 id bytes follow ⇒ parse drift. `.min(255)` guarantees the
            // count byte never lies below the real intent (255 == u8::MAX).
            payload.push(callbacks.len().min(255) as u8);
            payload.extend_from_slice(callbacks);
        }
        // SendMessage and ListDevices are handled by the early-return guards
        // above; they never reach this match.
        RunCommand::SendMessage(_) | RunCommand::ListDevices => {
            unreachable!("SendMessage/ListDevices handled by the early-return guards")
        }
    }

    payload.push(ETX_TERMINATOR_BYTE); // ETX terminator (signals end-of-message before chunking)
    payload
}

/// Parse a raw device reply into a [`crate::CommandResponse`].
///
/// `response[0]` disambiguates the reply
/// (`firmware_wire_contract.md` §Reply Disambiguation):
/// - [`crate::RESPONSE_MARKER`] (`0x51`) ⇒ typed reply, decoded by `response[1]`
///   (the command-echo byte) via [`parse_typed_reply`].
/// - `0` ⇒ [`crate::CommandResponse::Legacy`] `{ matched: false }`.
/// - `1` ⇒ [`crate::CommandResponse::Legacy`] `{ matched: true }`.
/// - empty, or any other marker ⇒ [`crate::CommandResponse::Timeout`] (treat as
///   a non-capable / legacy / offline device; the caller stays in string-only
///   mode — PRD §8, §10.2).
///
/// Every field access in the typed path uses defensive `.get(...)` indexing —
/// firmware replies may be truncated, so missing bytes default to `0` rather
/// than panicking. Consumer: the `run()` SendMessage AND typed-dispatch arms in
/// [`crate::run`] (P1.M1.T3.S2), which feed it the reply bytes captured by
/// [`send_raw_report`]'s [`burst_to_one`] bounded read. Request-side counterpart:
/// [`build_command_data`].
pub(crate) fn parse_reply(response: &[u8]) -> crate::CommandResponse {
    use crate::CommandResponse;
    if response.is_empty() {
        return CommandResponse::Timeout;
    }
    match response[0] {
        RESPONSE_MARKER => parse_typed_reply(response),
        0 => CommandResponse::Legacy { matched: false },
        1 => CommandResponse::Legacy { matched: true },
        _ => CommandResponse::Timeout, // unknown marker ⇒ treat as non-capable
    }
}

/// Decode a typed reply (`response[0] == RESPONSE_MARKER`) by its `response[1]`
/// command-echo byte. Field layouts per `firmware_wire_contract.md` §Field
/// Definitions; every byte is read with `.get(i).copied().unwrap_or(0)` so a
/// truncated reply never panics.
fn parse_typed_reply(response: &[u8]) -> crate::CommandResponse {
    use crate::CommandResponse;
    let cmd_echo = response.get(1).copied().unwrap_or(0);
    match cmd_echo {
        CMD_QUERY_INFO => CommandResponse::Info {
            proto_ver: response.get(2).copied().unwrap_or(0),
            feature_flags: response.get(3).copied().unwrap_or(0),
            callback_count: response.get(4).copied().unwrap_or(0),
            // u8 (0/1) on the wire ⇒ bool (the != 0 coercion).
            board_rules_present: response.get(5).copied().unwrap_or(0) != 0,
        },
        CMD_QUERY_CALLBACK => {
            let index = response.get(2).copied().unwrap_or(0);
            // Defensive slice: 3.min(len) guarantees the start index never exceeds
            // len, so a reply shorter than 4 bytes yields an empty name slice
            // (⇒ None) instead of panicking on `response[3..]`.
            let name = parse_callback_name(&response[3.min(response.len())..]);
            CommandResponse::CallbackName { index, name }
        }
        // SET_OS (0x03) and APPLY_HOST_CONTEXT (0x05) share the ack shape
        // [0x51][cmd_echo][ack]; ack == 1 ⇒ applied (ok: true).
        CMD_SET_OS | CMD_APPLY_HOST_CONTEXT => CommandResponse::Ack {
            ok: response.get(2).copied().unwrap_or(0) != 0,
        },
        _ => CommandResponse::Timeout, // unknown cmd echo ⇒ non-capable
    }
}

/// Decode a NUL-terminated ASCII callback name from `bytes`.
///
/// Reads up to the first `0x00` NUL or end of slice. Returns `None` when the name
/// is empty (NUL-at-start or empty slice) — the firmware emits an immediate `0x00`
/// when a callback has no name or the index is out of range
/// (`firmware_wire_contract.md` §QUERY_CALLBACK response). `String::from_utf8`
/// succeeds for the documented ASCII names (`0x20–0x7E`); invalid UTF-8 yields
/// `None` via `.ok()` (lossy substitution is deliberately NOT used).
fn parse_callback_name(bytes: &[u8]) -> Option<String> {
    let end = bytes.iter().position(|&b| b == 0x00).unwrap_or(bytes.len());
    let name_bytes = &bytes[..end];
    if name_bytes.is_empty() {
        return None;
    }
    String::from_utf8(name_bytes.to_vec()).ok()
}

/// Match parameters a cached handle set was opened for. The cache is rebuilt
/// whenever a request uses a different key.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct MatchKey {
    vendor_id: Option<u16>,
    product_id: Option<u16>,
    usage_page: u16,
    usage: u16,
}

/// A long-lived HID context plus the handles opened from it for a [`MatchKey`].
/// Kept in a global [`Mutex`] so repeated notifications reuse the same handles
/// (and the same enumerated `HidApi`) instead of re-scanning the whole HID bus
/// on every call. Enumerating + opening was, with the per-report reads gone,
/// the dominant per-notification cost.
struct DeviceCache {
    /// Long-lived HID context the handles were opened from. Retained (rather
    /// than dropped after opening) to pin the context's lifetime for the cached
    /// handles and to enable a cheaper `refresh_devices()`-based rebuild later.
    /// It is intentionally not read on the hot path.
    #[allow(dead_code)]
    api: HidApi,
    devices: Vec<HidDevice>,
    key: MatchKey,
}

static DEVICE_CACHE: LazyLock<Mutex<Option<DeviceCache>>> = LazyLock::new(|| Mutex::new(None));

/// Lock the global device cache, recovering from a poisoned mutex (a previous
/// holder panicked) rather than propagating the panic.
fn lock_cache() -> MutexGuard<'static, Option<DeviceCache>> {
    DEVICE_CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Ensure the cache slot holds a handle set for `key`. (Re)builds it — full
/// `HidApi` enumeration + open — only when empty or when the key changes; the
/// hot path reuses the cached handles and returns immediately.
///
/// Note: a newly-plugged *additional* matching device is not picked up until a
/// write fails (forcing a rebuild) or the match key changes. This is the
/// intentional trade-off of caching and is fine for the single-keyboard use
/// case (the replug case is handled: a stale handle fails the write, which
/// invalidates the cache and triggers this rebuild).
fn ensure_cache(
    cache: &mut Option<DeviceCache>,
    key: &MatchKey,
    verbose: bool,
) -> Result<(), QmkError> {
    if let Some(existing) = cache.as_ref() {
        if &existing.key == key {
            if verbose {
                println!(
                    "Reusing cached device handle ({} device(s)).",
                    existing.devices.len()
                );
            }
            return Ok(());
        }
    }

    if verbose {
        println!("Building device cache (enumerating HID bus)...");
    }

    // Release any previously-cached handles BEFORE opening new ones. On macOS
    // (IOKit) a HID interface is exclusive within a process: opening a device
    // that is still held open by a stale cached handle fails with
    // kIOReturnExclusiveAccess (0xE00002C5). A key change (e.g. the narrowed
    // per-device classify filter vs. the broad handshake filter) would otherwise
    // re-open the same still-held board and fail, leaving the board unusable in
    // the running process until restart. Dropping first guarantees a clean open
    // (release-then-open is confirmed to succeed on macOS).
    *cache = None;

    let api = HidApi::new().map_err(|e| QmkError::HidApiInitError(e.to_string()))?;
    let devices = open_matching_devices(&api, key)?;

    let count = devices.len();
    *cache = Some(DeviceCache {
        api,
        devices,
        key: *key,
    });

    if verbose {
        println!("Cached {} matching device(s).", count);
    }
    Ok(())
}

/// Enumerate `api` and open every interface matching `key`. Mirrors the old
/// per-call enumeration but reuses an existing `HidApi` so the cache can share a
/// single context across calls.
fn open_matching_devices(api: &HidApi, key: &MatchKey) -> Result<Vec<HidDevice>, QmkError> {
    let device_infos: Vec<_> = api
        .device_list()
        .filter(|d| {
            device_matches(
                d.vendor_id(),
                d.product_id(),
                d.usage_page(),
                d.usage(),
                key.vendor_id,
                key.product_id,
                key.usage_page,
                key.usage,
            )
        })
        .collect();

    if device_infos.is_empty() {
        return Err(QmkError::DeviceNotFound {
            vendor_id: key.vendor_id,
            product_id: key.product_id,
            usage_page: key.usage_page,
            usage: key.usage,
        });
    }

    let opened_devices: Vec<HidDevice> = device_infos
        .into_iter()
        .filter_map(|info| info.open_device(api).ok())
        .collect();

    if opened_devices.is_empty() {
        return Err(QmkError::DeviceOpenError(
            "Found matching HID devices, but could not open any of them for communication. Check permissions (udev rules on Linux)."
                .to_string(),
        ));
    }

    Ok(opened_devices)
}

/// Pure match predicate for the raw-HID interface filter.
///
/// A device matches when its usage page/usage equal the required values, and
/// its VID/PID equal the given values when those are `Some`. `None` VID/PID
/// means "match any" (the default auto-discovery path).
#[allow(clippy::too_many_arguments)]
fn device_matches(
    dev_vendor_id: u16,
    dev_product_id: u16,
    dev_usage_page: u16,
    dev_usage: u16,
    vendor_id: Option<u16>,
    product_id: Option<u16>,
    usage_page: u16,
    usage: u16,
) -> bool {
    dev_usage_page == usage_page
        && dev_usage == usage
        && vendor_id.is_none_or(|v| dev_vendor_id == v)
        && product_id.is_none_or(|p| dev_product_id == p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommandResponse, HostOs, RunCommand};
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;

    // === FakeHid test double (P1.M1.T2.S1) =====================================
    // An in-process stand-in for `hidapi::HidDevice` that implements [`RawHid`],
    // so the (generic) [`burst_to_one`] can be unit-tested without a physical
    // keyboard. Models the firmware's reply semantics with two reply queues:
    //
    // - `pre_write_replies`  — stale IN-buffer data read BEFORE any `write()`
    //   (the kind a prior send leaves behind; flushed by the Issue-3 pre-send
    //   drain in P1.M1.T3.S1).
    // - `post_write_replies` — the firmware's fresh per-report replies read
    //   AFTER `write()` (one 32-byte reply per report; PRD §4.4).
    //
    // A `Cell<bool>` "written" latch selects which queue `read_timeout` pops:
    // before the first `write()` it serves `pre_write_replies`, after it serves
    // `post_write_replies` (one-shot latch — never reset; FakeHid is single-use
    // per test). An empty queue returns `Ok(0)` to mirror hidapi's
    // `read_timeout` timeout/no-data semantics (NOT an error).
    struct FakeHid {
        pre_write_replies: RefCell<VecDeque<Vec<u8>>>,
        post_write_replies: RefCell<VecDeque<Vec<u8>>>,
        written: Cell<bool>,
        /// Recorded `write()` calls, for asserting how many reports were sent.
        writes: RefCell<Vec<Vec<u8>>>,
    }

    impl RawHid for FakeHid {
        fn write(&self, data: &[u8]) -> Result<usize, hidapi::HidError> {
            self.writes.borrow_mut().push(data.to_vec());
            self.written.set(true);
            Ok(data.len())
        }
        fn read_timeout(&self, buf: &mut [u8], _timeout: i32) -> Result<usize, hidapi::HidError> {
            let queue = if self.written.get() {
                &self.post_write_replies
            } else {
                &self.pre_write_replies
            };
            match queue.borrow_mut().pop_front() {
                Some(reply) => {
                    let n = reply.len().min(buf.len());
                    buf[..n].copy_from_slice(&reply[..n]);
                    Ok(n)
                }
                None => Ok(0), // empty queue ⇒ timeout/no-data (matches hidapi)
            }
        }
    }

    impl FakeHid {
        /// Empty double, written-latch unset (reads serve the pre-write queue).
        fn new() -> Self {
            Self {
                pre_write_replies: RefCell::new(VecDeque::new()),
                post_write_replies: RefCell::new(VecDeque::new()),
                written: Cell::new(false),
                writes: RefCell::new(Vec::new()),
            }
        }
        /// Queue a reply that `read_timeout` will return BEFORE any `write()`
        /// (stale IN-buffer data from a prior send).
        fn push_pre(&self, reply: Vec<u8>) {
            self.pre_write_replies.borrow_mut().push_back(reply);
        }
        /// Queue a reply that `read_timeout` will return AFTER the first
        /// `write()` (a firmware reply to a sent report).
        fn push_post(&self, reply: Vec<u8>) {
            self.post_write_replies.borrow_mut().push_back(reply);
        }
    }

    const DEV_VID: u16 = 0xFEED;
    const DEV_PID: u16 = 0x0000;
    const DEV_PAGE: u16 = 0xFF60;
    const DEV_USAGE: u16 = 0x61;

    #[test]
    fn matches_when_all_four_equal() {
        assert!(device_matches(
            DEV_VID,
            DEV_PID,
            DEV_PAGE,
            DEV_USAGE,
            Some(DEV_VID),
            Some(DEV_PID),
            DEV_PAGE,
            DEV_USAGE,
        ));
    }

    #[test]
    fn matches_by_usage_page_alone_when_vid_pid_none() {
        // The keystone auto-discovery path: VID/PID omitted.
        assert!(device_matches(
            DEV_VID, DEV_PID, DEV_PAGE, DEV_USAGE, None, None, DEV_PAGE, DEV_USAGE,
        ));
    }

    #[test]
    fn matches_arbitrary_vid_pid_when_none() {
        // Any VID/PID device with the right usage/page matches.
        assert!(device_matches(
            0x1234, 0xABCD, DEV_PAGE, DEV_USAGE, None, None, DEV_PAGE, DEV_USAGE,
        ));
    }

    #[test]
    fn rejects_wrong_usage_page_even_when_vid_pid_match() {
        assert!(!device_matches(
            DEV_VID,
            DEV_PID,
            DEV_PAGE,
            DEV_USAGE,
            Some(DEV_VID),
            Some(DEV_PID),
            0x1234,
            DEV_USAGE,
        ));
    }

    #[test]
    fn rejects_wrong_usage() {
        assert!(!device_matches(
            DEV_VID, DEV_PID, DEV_PAGE, DEV_USAGE, None, None, DEV_PAGE, 0x99,
        ));
    }

    #[test]
    fn rejects_wrong_vid_when_some() {
        assert!(!device_matches(
            DEV_VID,
            DEV_PID,
            DEV_PAGE,
            DEV_USAGE,
            Some(0x1111),
            None,
            DEV_PAGE,
            DEV_USAGE,
        ));
    }

    #[test]
    fn rejects_wrong_pid_when_some() {
        assert!(!device_matches(
            DEV_VID,
            DEV_PID,
            DEV_PAGE,
            DEV_USAGE,
            None,
            Some(0x2222),
            DEV_PAGE,
            DEV_USAGE,
        ));
    }

    #[test]
    fn vid_can_disambiguate_while_pid_any() {
        assert!(device_matches(
            DEV_VID,
            DEV_PID,
            DEV_PAGE,
            DEV_USAGE,
            Some(DEV_VID),
            None,
            DEV_PAGE,
            DEV_USAGE,
        ));
    }

    #[test]
    fn batches_for_empty_is_zero() {
        assert_eq!(batches_for(&[]), 0);
    }

    #[test]
    fn batches_for_single_byte_is_one() {
        assert_eq!(batches_for(&[0x03]), 1);
    }

    #[test]
    fn batches_for_exact_report_multiple() {
        // 30 payload bytes = exactly one report; 60 = exactly two.
        assert_eq!(batches_for(&[0u8; 30]), 1);
        assert_eq!(batches_for(&[0u8; 60]), 2);
    }

    #[test]
    fn batches_for_one_over_splits() {
        // A single byte past a report boundary forces an extra report.
        assert_eq!(batches_for(&[0u8; 31]), 2);
        assert_eq!(batches_for(&[0u8; 61]), 3);
    }

    #[test]
    fn match_key_equality_drives_cache_rebuild() {
        let auto = MatchKey {
            vendor_id: None,
            product_id: None,
            usage_page: DEV_PAGE,
            usage: DEV_USAGE,
        };
        let explicit = MatchKey {
            vendor_id: Some(DEV_VID),
            product_id: Some(DEV_PID),
            usage_page: DEV_PAGE,
            usage: DEV_USAGE,
        };
        assert_eq!(auto, auto);
        assert_ne!(auto, explicit);
    }

    #[test]
    fn build_command_data_query_info() {
        // QUERY_INFO (0x01): no args. Full payload = discriminator + cmd + ETX.
        let payload = build_command_data(&RunCommand::QueryInfo);
        assert_eq!(
            payload,
            vec![CMD_DISCRIMINATOR, CMD_QUERY_INFO, ETX_TERMINATOR_BYTE]
        );
        // Invariants every typed payload must satisfy (also asserted by exact-eq):
        assert_eq!(
            *payload.first().unwrap(),
            CMD_DISCRIMINATOR,
            "must start with 0xF0"
        );
        assert_eq!(
            *payload.last().unwrap(),
            ETX_TERMINATOR_BYTE,
            "must end with ETX"
        );
    }

    #[test]
    fn build_command_data_query_callback() {
        // QUERY_CALLBACK (0x02): one arg = the registry index.
        let payload = build_command_data(&RunCommand::QueryCallback(7));
        assert_eq!(
            payload,
            vec![
                CMD_DISCRIMINATOR,
                CMD_QUERY_CALLBACK,
                7,
                ETX_TERMINATOR_BYTE
            ]
        );

        // Boundary: index 0 and 255 must still serialize (u8 range, no truncation).
        assert_eq!(
            build_command_data(&RunCommand::QueryCallback(0)),
            vec![
                CMD_DISCRIMINATOR,
                CMD_QUERY_CALLBACK,
                0,
                ETX_TERMINATOR_BYTE
            ]
        );
        assert_eq!(
            build_command_data(&RunCommand::QueryCallback(u8::MAX)),
            vec![
                CMD_DISCRIMINATOR,
                CMD_QUERY_CALLBACK,
                u8::MAX,
                ETX_TERMINATOR_BYTE
            ]
        );
    }

    #[test]
    fn build_command_data_set_os() {
        // os_byte mirrors QMK os_variant_t (firmware_wire_contract.md §SET_OS).
        for (os, os_byte) in [
            (HostOs::Unsure, 0u8),
            (HostOs::Linux, 1u8),
            (HostOs::Windows, 2u8),
            (HostOs::Macos, 3u8),
            (HostOs::Ios, 4u8),
        ] {
            let payload = build_command_data(&RunCommand::SetOs(os));
            assert_eq!(
                payload,
                vec![CMD_DISCRIMINATOR, CMD_SET_OS, os_byte, ETX_TERMINATOR_BYTE],
                "SET_OS({os:?}) must serialize to [0xF0][0x03][{os_byte}][ETX]"
            );
        }
    }

    #[test]
    fn build_command_data_apply_host_context_set_layer() {
        // layer = Some(224) (HOST_LAYER_BASE), 3 callbacks, clear_board set.
        let payload = build_command_data(&RunCommand::ApplyHostContext {
            layer: Some(224),
            callbacks: vec![10, 20, 30],
            clear_board: true,
        });
        // [0xF0, 0x05, layer=224, flags=0x01, count=3, 10, 20, 30, ETX]
        assert_eq!(
            payload,
            vec![
                CMD_DISCRIMINATOR,
                CMD_APPLY_HOST_CONTEXT,
                224,
                0x01,
                3,
                10,
                20,
                30,
                ETX_TERMINATOR_BYTE
            ]
        );
    }

    #[test]
    fn build_command_data_apply_host_context_clear_layer() {
        // layer = None ⇒ wire byte 0xFF (clear host layer); clear_board false ⇒ flags 0.
        let payload = build_command_data(&RunCommand::ApplyHostContext {
            layer: None,
            callbacks: Vec::new(),
            clear_board: false,
        });
        // [0xF0, 0x05, 0xFF, 0x00, count=0, ETX]
        assert_eq!(
            payload,
            vec![
                CMD_DISCRIMINATOR,
                CMD_APPLY_HOST_CONTEXT,
                0xFF,
                0x00,
                0,
                ETX_TERMINATOR_BYTE
            ]
        );
    }

    // NEW — SendMessage now produces a REAL payload (reconciliation with the
    // architecture doc: build_command_data is the single source of truth for
    // on-wire payloads, including the legacy string path). The payload is the
    // raw message bytes + a single ETX; it must NOT carry the 0xF0 discriminator
    // (that is the typed-command path only).
    #[test]
    fn build_command_data_send_message() {
        // Legacy string path: raw bytes + ETX, NO 0xF0 discriminator.
        let payload = build_command_data(&RunCommand::SendMessage("App\x1DTitle".to_string()));
        let mut expected = "App\x1DTitle".as_bytes().to_vec();
        expected.push(ETX_TERMINATOR_BYTE);
        assert_eq!(payload, expected);
        assert!(
            !payload.contains(&CMD_DISCRIMINATOR),
            "SendMessage must NOT carry the 0xF0 discriminator"
        );
        assert_eq!(
            *payload.last().unwrap(),
            ETX_TERMINATOR_BYTE,
            "must end with ETX"
        );
    }

    // KEPT — ListDevices is not a wire command; builder returns empty defensively.
    #[test]
    fn build_command_data_list_devices_empty() {
        assert_eq!(build_command_data(&RunCommand::ListDevices), Vec::new());
    }

    // Pin the ETX const value against regression. The assertion is on a const,
    // so clippy::assertions_on_constants is allowed here deliberately (the test
    // exists to make a value drift a compile-test failure).
    #[allow(clippy::assertions_on_constants)]
    #[test]
    fn etx_terminator_byte_is_0x03() {
        assert_eq!(ETX_TERMINATOR_BYTE, 0x03);
    }

    #[test]
    fn build_command_data_multi_report_chunking() {
        // A large APPLY_HOST_CONTEXT must span multiple 30-byte reports via the
        // EXISTING batches_for path (no bespoke framing). 40 callback ids ⇒ payload
        // = [0xF0, 0x05, layer, flags, count=40, <40 ids>, ETX] = 46 bytes ⇒ 2 reports.
        let callbacks: Vec<u8> = (0..40u8).collect();
        let payload = build_command_data(&RunCommand::ApplyHostContext {
            layer: Some(224),
            callbacks: callbacks.clone(),
            clear_board: false,
        });
        // 5 header bytes (disc+cmd+layer+flags+count) + 40 ids + 1 ETX = 46.
        assert_eq!(payload.len(), 46);
        assert_eq!(*payload.first().unwrap(), CMD_DISCRIMINATOR);
        assert_eq!(
            *payload.last().unwrap(),
            ETX_TERMINATOR_BYTE,
            "ETX must be the final byte"
        );
        // The id bytes are copied verbatim into the payload (after the 5-byte header).
        assert_eq!(&payload[5..5 + callbacks.len()], &callbacks[..]);
        // batches_for (the unchanged chunker): ceil(46 / 30) = 2 reports.
        assert_eq!(batches_for(&payload), 2, "46 payload bytes ⇒ 2 reports");
        // Sanity: a payload that fits one report still chunks to 1.
        assert_eq!(batches_for(&build_command_data(&RunCommand::QueryInfo)), 1);
    }

    // Pin every wire-protocol const value. Several assertions are on consts
    // (e.g. REPLY_READ_TIMEOUT_MS > 0), so clippy::assertions_on_constants is
    // allowed here deliberately — these exist to make a const drift a
    // compile-time test failure.
    #[allow(clippy::assertions_on_constants)]
    #[test]
    fn typed_command_constants_match_firmware_contract() {
        // Wire-protocol values are the canonical source of truth from the firmware
        // notifier.h (see firmware_wire_contract.md §Constants). Drift here would
        // silently break host<->firmware interop, so pin every value.
        assert_eq!(
            CMD_DISCRIMINATOR, 0xF0,
            "NOTIFY_CMD_DISCRIMINATOR: typed-command first payload byte after 0x81 0x9F"
        );
        assert_eq!(
            RESPONSE_MARKER, 0x51,
            "NOTIFY_RESPONSE_MARKER: response[0]==0x51 means typed reply"
        );
        assert_eq!(CMD_QUERY_INFO, 0x01, "NOTIFY_CMD_QUERY_INFO");
        assert_eq!(CMD_QUERY_CALLBACK, 0x02, "NOTIFY_CMD_QUERY_CALLBACK");
        assert_eq!(CMD_SET_OS, 0x03, "NOTIFY_CMD_SET_OS");
        assert_eq!(
            CMD_APPLY_HOST_CONTEXT, 0x05,
            "NOTIFY_CMD_APPLY_HOST_CONTEXT"
        );
        // REPLY_READ_TIMEOUT_MS is a host-side choice (bounded blocking read of the
        // first reply after a burst), NOT a firmware value. Its invariant is the
        // documented "Must be > 0" (unlike the drain loop's non-blocking 0).
        assert!(
            REPLY_READ_TIMEOUT_MS > 0,
            "reply read timeout must block for a real reply, not poll like the drain's 0"
        );
    }

    #[test]
    fn build_command_data_apply_host_context_representative_ids() {
        // Item-contract representative case: callbacks=[1,5,10] ⇒ count=3 then
        // the three id bytes verbatim. Full byte sequence (firmware_wire_contract.md
        // §APPLY_HOST_CONTEXT request): [0xF0,0x05,layer,flags,count=3,1,5,10,0x03].
        let payload = build_command_data(&RunCommand::ApplyHostContext {
            layer: Some(224), // HOST_LAYER_BASE
            callbacks: vec![1, 5, 10],
            clear_board: false,
        });
        assert_eq!(
            payload,
            vec![0xF0, 0x05, 224, 0x00, 3, 1, 5, 10, ETX_TERMINATOR_BYTE]
        );
    }

    #[test]
    fn build_command_data_apply_host_context_clamps_count_at_255() {
        // Edge case (P1.M2.T1.S2 contract): callbacks.len() > 255 must CLAMP the
        // count byte to 255 — NOT truncate. `256u8 as u8 == 0`, which would tell
        // the firmware "zero callbacks follow" while 256 id bytes + ETX actually
        // follow ⇒ catastrophic parse drift. `.min(255)` prevents that.
        // In practice callbacks.len() <= HOST_CALLBACK_MAX (32), so this path is
        // unreachable on the happy path; the clamp is the defensive contract and
        // this test pins it against regression.
        let callbacks: Vec<u8> = (0..=255u8).collect(); // 256 elements (0..255)
        let payload = build_command_data(&RunCommand::ApplyHostContext {
            layer: Some(224),
            callbacks,
            clear_board: false,
        });
        // Header: [0xF0, 0x05, layer=224, flags=0x00], then COUNT MUST BE 255.
        assert_eq!(&payload[..5], &[0xF0, 0x05, 224, 0x00, 255]);
        // All 256 ids copied verbatim (item reference: extend_from_slice(callbacks)
        // copies the full slice; only the count byte is clamped).
        assert_eq!(payload.len(), 5 + 256 + 1, "header(5) + ids(256) + ETX(1)");
        assert_eq!(
            *payload.last().unwrap(),
            ETX_TERMINATOR_BYTE,
            "ETX must remain the final byte"
        );
        // PROOF OF FIX: with the old `callbacks.len() as u8`, the count byte here
        // would have been 0 (256 as u8 == 0) and this assertion would fail.
    }

    #[test]
    fn parse_reply_info_reply() {
        // QUERY_INFO typed reply (firmware_wire_contract.md §QUERY_INFO response):
        // [0x51][0x01][proto_ver][feature_flags][callback_count][board_rules_present].
        let response = [0x51, 0x01, 2, 0x03, 5, 1];
        assert_eq!(
            parse_reply(&response),
            CommandResponse::Info {
                proto_ver: 2,
                feature_flags: 0x03,
                callback_count: 5,
                board_rules_present: true,
            }
        );
    }

    #[test]
    fn parse_reply_info_board_rules_absent() {
        // Same shape, but board_rules_present byte == 0 ⇒ bool false (the != 0
        // coercion in parse_typed_reply's CMD_QUERY_INFO arm).
        let response = [0x51, 0x01, 2, 0x03, 5, 0];
        assert_eq!(
            parse_reply(&response),
            CommandResponse::Info {
                proto_ver: 2,
                feature_flags: 0x03,
                callback_count: 5,
                board_rules_present: false,
            }
        );
    }

    #[test]
    fn parse_reply_callback_name_named() {
        // QUERY_CALLBACK typed reply: [0x51][0x02][index][name bytes, NUL-padded].
        // "Vim" = [V, i, m] = [0x56, 0x69, 0x6d], then a NUL terminator ends the name.
        let response = [0x51, 0x02, 3, b'V', b'i', b'm', 0x00, 0x00];
        assert_eq!(
            parse_reply(&response),
            CommandResponse::CallbackName {
                index: 3,
                name: Some("Vim".to_string()),
            }
        );
    }

    #[test]
    fn parse_reply_callback_name_unnamed() {
        // NUL at the name start ⇒ None. The firmware emits an immediate 0x00 when
        // the callback has no name or the requested index is out of range.
        let response = [0x51, 0x02, 5, 0x00, 0x00];
        assert_eq!(
            parse_reply(&response),
            CommandResponse::CallbackName {
                index: 5,
                name: None
            }
        );
    }

    #[test]
    fn parse_reply_ack_set_os_applied() {
        // SET_OS (cmd 0x03) ack: [0x51][0x03][ack]; ack == 1 ⇒ ok: true.
        let response = [0x51, 0x03, 1];
        assert_eq!(parse_reply(&response), CommandResponse::Ack { ok: true });
    }

    #[test]
    fn parse_reply_ack_apply_host_context_rejected() {
        // APPLY_HOST_CONTEXT (cmd 0x05) ack: [0x51][0x05][ack]; ack == 0 ⇒ ok: false.
        // Shares the CMD_SET_OS | CMD_APPLY_HOST_CONTEXT arm with the test above.
        let response = [0x51, 0x05, 0];
        assert_eq!(parse_reply(&response), CommandResponse::Ack { ok: false });
    }

    #[test]
    fn parse_reply_empty_slice_is_timeout() {
        // Empty input ⇒ Timeout. A zero-byte IN report means no reply arrived
        // within the bounded read_timeout (the caller passes &[] when
        // read_timeout returned Ok(0)). firmware_wire_contract.md §Reply
        // Disambiguation: "no reply within timeout ⇒ Timeout"; the caller treats
        // Timeout as a legacy/non-capable device and stays in string-only mode
        // (PRD §8, §10.2). Item edge case #1.
        assert_eq!(parse_reply(&[]), CommandResponse::Timeout);
    }

    #[test]
    fn parse_reply_legacy_zero_is_no_match() {
        // Legacy string-mode reply: response[0] == 0 is the match-bool "no
        // match" (the firmware wrote the bool, NOT a typed 0x51 marker). Item
        // edge case #2. firmware_wire_contract.md §Reply Disambiguation.
        assert_eq!(
            parse_reply(&[0]),
            CommandResponse::Legacy { matched: false }
        );
    }

    #[test]
    fn parse_reply_legacy_one_is_matched() {
        // Legacy string-mode reply: response[0] == 1 is the match-bool "matched".
        // Item edge case #3.
        assert_eq!(parse_reply(&[1]), CommandResponse::Legacy { matched: true });
    }

    #[test]
    fn parse_reply_typed_marker_only_is_timeout() {
        // Degenerate "too short" typed reply: [0x51] alone (len 1 < 3).
        // parse_typed_reply reads cmd_echo via response.get(1).copied().unwrap_or(0)
        // ⇒ None ⇒ 0 (the slice has no index 1). 0 is not a known cmd id, so the
        // unknown-cmd-echo arm yields Timeout. CRITICAL: this MUST NOT panic — a
        // bare response[1] index would panic on a len-1 slice. The defensive
        // .get() is the guard this test certifies. Item edge case #4.
        assert_eq!(parse_reply(&[0x51]), CommandResponse::Timeout);
    }

    #[test]
    fn parse_reply_unknown_cmd_echo_is_timeout() {
        // Typed marker present, but response[1] == 0xFF is not a known command
        // echo (valid ids are 0x01/0x02/0x03/0x05; 0x04 is VIA-reserved and also
        // unknown to this crate). Treated as a non-capable / future-firmware
        // reply ⇒ Timeout. Exercises the `_ => Timeout` arm in parse_typed_reply.
        // Item edge case #5.
        assert_eq!(parse_reply(&[0x51, 0xFF]), CommandResponse::Timeout);
    }

    #[test]
    fn parse_reply_unknown_marker_is_timeout() {
        // response[0] == 0x42 is neither 0x51 (typed), 0 (legacy no-match), nor
        // 1 (legacy match). Per firmware_wire_contract.md §Reply Disambiguation,
        // any other marker byte ⇒ treat as a non-capable device ⇒ Timeout.
        // Exercises the `_ => Timeout` arm in the top-level parse_reply match.
        // Item edge case #6.
        assert_eq!(parse_reply(&[0x42]), CommandResponse::Timeout);
    }

    #[test]
    fn parse_reply_callback_name_non_utf8_is_none() {
        // QUERY_CALLBACK reply whose name bytes are NOT valid UTF-8. 0xFF and
        // 0xFE are never legal UTF-8 bytes (UTF-8 uses 0x00–0xF4), so
        // String::from_utf8 fails ⇒ .ok() ⇒ None. parse_callback_name
        // deliberately uses .ok() (NOT from_utf8_lossy) so a corrupt name is
        // treated as ABSENT (None) rather than replaced with U+FFFD.
        // Layout: [0x51][0x02][index=1][0xFF][0xFE][0x00 NUL terminator].
        // Item edge case #7.
        let response = [0x51, 0x02, 1, 0xFF, 0xFE, 0x00];
        assert_eq!(
            parse_reply(&response),
            CommandResponse::CallbackName {
                index: 1,
                name: None
            }
        );
    }

    #[test]
    fn parse_reply_truncated_info_defaults_board_rules_false() {
        // Truncated QUERY_INFO reply: only 4 bytes, so board_rules_present
        // (offset [5]) AND callback_count (offset [4]) are absent. Both must
        // default via .get(i).copied().unwrap_or(0): callback_count ⇒ 0,
        // board_rules_present ⇒ 0 ⇒ false (the != 0 coercion). The PRESENT
        // fields (proto_ver=2, feature_flags=0x03) must still decode normally.
        // CRITICAL: this MUST NOT panic on a bare response[5] index — the
        // defensive .get() is the guard this test certifies. Item edge case #8.
        let response = [0x51, 0x01, 2, 0x03];
        assert_eq!(
            parse_reply(&response),
            CommandResponse::Info {
                proto_ver: 2,
                feature_flags: 0x03,
                callback_count: 0,
                board_rules_present: false,
            }
        );
    }

    #[test]
    fn fakehid_write_records_calls_and_switches_read_queue() {
        let fake = FakeHid::new();
        fake.push_pre(vec![0xAA; 33]); // stale IN-buffer byte (Issue-3 shape)
        fake.push_post({
            let mut r = vec![0u8; 33];
            r[0] = 1;
            r
        }); // firmware match reply

        // BEFORE any write(): read_timeout serves the PRE-write (stale) queue.
        let mut buf = [0u8; 33];
        let n = fake.read_timeout(&mut buf, 0).expect("pre-write read Ok");
        assert_eq!(n, 33);
        assert_eq!(
            buf[0], 0xAA,
            "pre-write read must pop the stale pre-queue reply"
        );

        // write() records the call and flips the written latch to the POST queue.
        let report = [0x00u8, 0x81, 0x9F, 0x03];
        fake.write(&report).expect("write Ok");
        assert_eq!(
            fake.writes.borrow().len(),
            1,
            "write must record exactly one call"
        );
        assert_eq!(
            fake.writes.borrow()[0],
            report.to_vec(),
            "write must record the exact bytes sent"
        );

        // AFTER write(): read_timeout serves the POST-write (firmware) queue.
        let n = fake.read_timeout(&mut buf, 0).expect("post-write read Ok");
        assert_eq!(n, 33);
        assert_eq!(
            buf[0], 1,
            "post-write read must pop the firmware reply, not stale data"
        );
    }

    #[test]
    fn fakehid_read_timeout_returns_zero_when_queue_empty() {
        let fake = FakeHid::new();
        // Empty pre-write queue ⇒ Ok(0), mirroring hidapi read_timeout timeout
        // semantics (NOT an Err). burst_to_one's drain relies on this to stop.
        let mut buf = [0u8; 33];
        let n = fake
            .read_timeout(&mut buf, 0)
            .expect("empty queue must be Ok, not Err");
        assert_eq!(n, 0, "empty queue ⇒ Ok(0) (hidapi timeout semantics)");
        // buf left untouched (no copy on the None arm).
        assert!(buf.iter().all(|&b| b == 0));
    }

    #[test]
    fn fakehid_drives_generic_burst_to_one() {
        // Proves FakeHid is usable as `burst_to_one(&fake, …)` (the T: RawHid
        // bound) and that one queued reply is captured. Asserts ONLY on success,
        // write-count, and reply presence — NOT the reply's byte value, which is
        // what the T2.S2 capture rewrite changes (keeps this test stable).
        let fake = FakeHid::new();
        fake.push_post({
            let mut r = vec![0u8; 33];
            r[0] = 1;
            r
        }); // one firmware reply

        let payload = vec![0u8; 10]; // 10 bytes ⇒ 1 report (PAYLOAD_PER_REPORT=30)
        let batch_count = batches_for(&payload);
        assert_eq!(batch_count, 1);

        let (success, reply) = burst_to_one(&fake, &payload, batch_count, false);
        assert!(success, "write must succeed (FakeHid::write returns Ok)");
        assert_eq!(
            fake.writes.borrow().len(),
            batch_count,
            "burst_to_one must write exactly batch_count reports"
        );
        assert!(reply.is_some(), "the one queued reply must be captured");
    }

    #[test]
    fn burst_to_one_captures_last_reply_for_multi_report() {
        // Issue-1 regression: a multi-report payload makes the firmware emit one
        // 32-byte reply per report (PRD §4.4). The first N-1 replies have
        // response[0] == 0 (the message is incomplete — no ETX seen, so the
        // legacy match-bool stays false); only the LAST reply — the ETX report —
        // carries the real result. burst_to_one must capture the LAST reply,
        // not the first (the v0.3.0 capture-first bug returned the intermediate 0).
        // Proven live: 2-report send -> firmware replies = [0, 1] (TEST_RESULTS.md).
        let payload = vec![0u8; 31]; // 31 bytes => 2 reports (PAYLOAD_PER_REPORT = 30)
        let batch_count = batches_for(&payload);
        assert_eq!(
            batch_count, 2,
            "31-byte payload must span exactly 2 reports"
        );

        let fake = FakeHid::new();
        // Reply to report 1: intermediate, message incomplete (no ETX) => match
        // is false => the per-report ack is response[0] == 0.
        fake.push_post(vec![0u8; 33]);
        // Reply to report 2: the ETX report => the firmware computes the real
        // match-bool here => response[0] == 1 (the result we must return).
        fake.push_post({
            let mut r = vec![0u8; 33];
            r[0] = 1;
            r
        });

        let (success, reply) = burst_to_one(&fake, &payload, batch_count, false);
        assert!(
            success,
            "write path must succeed (FakeHid::write returns Ok)"
        );
        let reply = reply.expect("must capture the ETX-report reply, not time out");
        assert_eq!(
            reply[0], 1,
            "must capture the ETX-report reply (1), not the intermediate [0] reply"
        );
        assert_eq!(
            fake.writes.borrow().len(),
            2,
            "must write exactly 2 reports for a 2-report payload"
        );
    }

    #[test]
    fn burst_to_one_single_report_unchanged() {
        // Single-report payloads were never affected by Issue 1 (1 report => 1
        // reply => the only reply IS the ETX reply). This guards against the
        // capture-last rewrite (P1.M1.T2.S2) regressing the batch_count == 1
        // path: the loop must still capture the one reply it reads.
        let payload = vec![0u8; 10]; // 10 bytes => 1 report
        let batch_count = batches_for(&payload);
        assert_eq!(batch_count, 1, "10-byte payload must fit in 1 report");

        let fake = FakeHid::new();
        // The one (ETX) reply: match-bool == 1.
        fake.push_post({
            let mut r = vec![0u8; 33];
            r[0] = 1;
            r
        });

        let (success, reply) = burst_to_one(&fake, &payload, batch_count, false);
        assert!(success, "write path must succeed");
        let reply = reply.expect("must capture the single reply");
        assert_eq!(
            reply[0], 1,
            "must capture the single (ETX) reply's match-bool"
        );
        assert_eq!(
            fake.writes.borrow().len(),
            1,
            "must write exactly 1 report for a single-report payload"
        );
    }

    #[test]
    fn burst_to_one_drains_stale_replies_before_send() {
        // Issue-3 regression: a stale reply left in the IN buffer by a PRIOR send
        // must be drained BEFORE this send's write(), so the capture loop reads only
        // THIS send's fresh reply. FakeHid models stale IN-buffer data with its
        // pre_write_replies queue (served by read_timeout while the `written` latch
        // is false, i.e. before any write()). The pre-send drain pops that queue; if
        // it did not, the stale reply would linger. (See architecture/reply_capture_
        // design.md §Test: Pre-Send Drain.)
        let fake = FakeHid::new();
        // Stale IN-buffer reply from a previous send (all-zero report: [0]==0).
        fake.push_pre(vec![0u8; 33]);
        // The FRESH reply to THIS send's single report: match-bool == 1.
        fake.push_post({
            let mut r = vec![0u8; 33];
            r[0] = 1;
            r
        });

        let payload = vec![0x41u8; 10]; // 10 bytes => 1 report (PAYLOAD_PER_REPORT = 30)
        let batch_count = batches_for(&payload);
        assert_eq!(batch_count, 1, "10-byte payload must fit in 1 report");

        let (success, reply) = burst_to_one(&fake, &payload, batch_count, false);
        assert!(
            success,
            "write path must succeed (FakeHid::write returns Ok)"
        );
        let reply = reply.expect("must capture the fresh reply, not time out");

        // (a) Contract assertion (item clause 3): the captured reply is the FRESH
        //     one, whose [0]==1 — NOT the stale [0].
        assert_eq!(
            reply[0], 1,
            "must capture the fresh reply ([0]==1), not the stale reply ([0]==0)"
        );

        // (b) DECISIVE assertion: the pre-send drain must have CONSUMED the stale
        //     reply before write() flipped the read queue to post_write_replies.
        //     See the ⚠️ CRITICAL note in the PRP for why (a) alone is a
        //     false-passing test: FakeHid switches read queues on write(), so
        //     without the drain the stale reply would simply sit unread in
        //     pre_write_replies and reply[0] would STILL be 1. This is_empty()
        //     check is what makes the test fail if the drain block is removed —
        //     i.e. a genuine regression test.
        assert!(
            fake.pre_write_replies.borrow().is_empty(),
            "the pre-send drain must consume the stale reply before write() flips the read queue"
        );
        assert_eq!(
            fake.writes.borrow().len(),
            1,
            "must write exactly 1 report for a single-report payload"
        );
    }

    #[test]
    fn burst_to_one_empty_in_buffer_is_clean() {
        // Issue-3 clean-path baseline: when there is NO stale IN-buffer data, the
        // pre-send drain is a no-op (reads Ok(0), breaks after one iteration) and
        // normal reply capture is undisturbed. This is the common case on the hot
        // path (a quiet device has nothing to drain), so the drain must not stall,
        // consume the fresh reply, or otherwise perturb capture. Pairs with
        // burst_to_one_drains_stale_replies_before_send to cover both halves of
        // the drain's contract.
        let fake = FakeHid::new();
        // pre_write_replies deliberately LEFT EMPTY (no stale data).
        fake.push_post({
            let mut r = vec![0u8; 33];
            r[0] = 1;
            r
        }); // the fresh (only) reply

        let payload = vec![0x41u8; 10]; // 1 report
        let batch_count = batches_for(&payload);
        assert_eq!(batch_count, 1);

        let (success, reply) = burst_to_one(&fake, &payload, batch_count, false);
        assert!(success, "write path must succeed");
        let reply = reply.expect("must capture the single fresh reply");
        assert_eq!(
            reply[0], 1,
            "the fresh reply must be captured intact (drain must not steal it)"
        );
        assert_eq!(fake.writes.borrow().len(), 1, "must write exactly 1 report");
    }
}

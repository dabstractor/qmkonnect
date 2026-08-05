use crate::core::types::WindowInfo;
use once_cell::sync::Lazy;
use std::collections::{BTreeSet, HashMap};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

// Debounce interval now lives in DebounceState (loaded from config); see
// core::configured_debounce_ms().

// Trait to abstract the notification functionality
pub trait Notifier: Send + Sync {
    fn notify(&self, message: String) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Send a **typed** command to the QMK device and return its parsed reply.
    ///
    /// This is the typed-command transport primitive backing the host-side-rules
    /// pipeline (PRD §5.7, §8(4)/(5)): the capability handshake (`QueryInfo` /
    /// `QueryCallback` / `SetOs`, P4.M2) and the per-window host-context send
    /// (`ApplyHostContext`, P4.M3) both ride through this single method.
    ///
    /// `notify()` remains the legacy string path (`SendMessage`); `send_command`
    /// is the typed path — parameterized by [`qmk_notifier::RunCommand`] so the
    /// trait stays one seam the test mock can intercept and the real impl can
    /// route to [`qmk_notifier::run`].
    ///
    /// **Retry / cache parity** (PRD §5.7: "Retry/cache for the typed command
    /// match the string path §5.4") is the **caller's** responsibility
    /// (P4.M3.T1.S1), not this method's — `send_command` is a thin transport
    /// wrapper: build [`qmk_notifier::RunParameters`] from `command` + `filter`,
    /// call [`qmk_notifier::run`], map [`qmk_notifier::QmkError`] to a boxed
    /// error, and return the [`qmk_notifier::CommandResponse`] unchanged.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use qmkonnect::core::notifier::{Notifier, QmkNotifier, DeviceFilter};
    /// use qmk_notifier::{RunCommand, CommandResponse};
    ///
    /// let notifier = QmkNotifier;
    /// let filter = DeviceFilter {
    ///     vendor_id: None, product_id: None,
    ///     usage_page: 0xFF60, usage: 0x61,
    /// };
    /// match notifier.send_command(RunCommand::QueryInfo, &filter) {
    ///     Ok(CommandResponse::Info { proto_ver: 2, feature_flags, callback_count, .. }) => { /* capable */ }
    ///     Ok(_) => { /* legacy / timeout -> string-only fallback */ }
    ///     Err(e) => { /* device error — caller decides retry/cache (P4.M3) */ }
    /// }
    /// ```
    fn send_command(
        &self,
        command: qmk_notifier::RunCommand,
        filter: &DeviceFilter,
    ) -> Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>>;
}

// Real implementation that uses qmk_notifier
pub struct QmkNotifier;

/// Resolved device match criteria. VID/PID are optional (`None` = match any,
/// i.e. auto-discovery by usage page/usage); usage page/usage are always set
/// (defaulting to QMK's raw-HID convention). Carrying all four together keeps
/// the hidapi enumerate/match sites and the `RunParameters` build in sync.
pub struct DeviceFilter {
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub usage_page: u16,
    pub usage: u16,
}

/// Resolve the device match criteria from the user's config file, falling back
/// to QMK's raw-HID defaults. VID/PID are optional (`None` = auto-discovery)
/// when unset; usage page/usage default to `0xFF60`/`0x61` (overridable for
/// boards that changed `RAW_USAGE_PAGE`/`RAW_USAGE_ID` in firmware). Read via
/// `cached_config()` (mtime-keyed cache in core): re-stats every call,
/// re-reads+re-parses only when the file's mtime/size change — so config edits
/// still take effect on the next call (hot-config), without the redundant
/// per-call disk read.
fn configured_filter() -> DeviceFilter {
    let cfg = crate::core::cached_config().ok();
    DeviceFilter {
        vendor_id: cfg.as_ref().and_then(|c| c.vendor_id),
        product_id: cfg.as_ref().and_then(|c| c.product_id),
        usage_page: cfg
            .as_ref()
            .and_then(|c| c.usage_page)
            .unwrap_or(qmk_notifier::DEFAULT_USAGE_PAGE),
        usage: cfg
            .as_ref()
            .and_then(|c| c.usage)
            .unwrap_or(qmk_notifier::DEFAULT_USAGE),
    }
}

/// If the user's `config.toml` is malformed, return its path + the parse error
/// so [`startup_device_probe`] can report it clearly (PRD §2.1 Goal 4). Returns
/// `None` when no config exists (valid: defaults apply) or it parses cleanly.
///
/// The hot readers (`configured_filter`/`configured_timing`) deliberately
/// swallow parse errors via `.ok()` to degrade gracefully at runtime; this
/// function re-parses independently (pure, no global side effects) purely so a
/// startup diagnostic can distinguish "no config" from "broken config". Both
/// read failures (e.g. permission denied) and TOML/serde failures are reported.
fn config_parse_error() -> Option<(PathBuf, String)> {
    let path = crate::platforms::get_config_paths()
        .into_iter()
        .find(|p| p.exists())?;
    config_parse_error_at(&path)
}

/// Pure, hermetically-testable core of [`config_parse_error`]: given an
/// explicit config path, return it + the error when parsing fails, or `None`
/// when it parses cleanly. [`config_parse_error`] resolves the path (first
/// existing candidate) then delegates here.
fn config_parse_error_at(path: &Path) -> Option<(PathBuf, String)> {
    match crate::core::parse_config(path) {
        Ok(_) => None,
        Err(e) => Some((path.to_path_buf(), e.to_string())),
    }
}

/// List all HID devices WITHOUT opening them — pure enumeration from the kernel
/// device list, so it can never disturb the keyboard. Backs the `--list-devices`
/// flag for VID/PID discovery (#17).
pub fn list_devices() -> Result<(), Box<dyn Error>> {
    let api = hidapi::HidApi::new()?;
    println!("Available HID devices (vendor:product  usage_page:usage  product):");
    for d in api.device_list() {
        println!(
            "  {:#06x}:{:#06x}  {:#06x}:{:#06x}  {}",
            d.vendor_id(),
            d.product_id(),
            d.usage_page(),
            d.usage(),
            d.product_string().unwrap_or(""),
        );
    }
    Ok(())
}

/// One-time startup probe for the configured device. Read-only enumeration (it
/// neither opens nor sends to the device) so it cannot disrupt the keyboard.
/// Prints a clear diagnostic when the configured device isn't found, instead of
/// the runtime path's silent retry-and-give-up (#16).
pub fn startup_device_probe(verbose: bool) {
    // Surface a malformed `config.toml` at startup (PRD §2.1 Goal 4: "probe
    // once and say so clearly"). The hot-config readers (`configured_filter`/
    // `configured_timing`) swallow parse errors and fall back to defaults —
    // correct for graceful degradation, but it means the probe below would run
    // against the *defaulted* filter and print "Found QMK device" even when the
    // user's config (with overridden usage_page/debounce_ms/poll_interval_ms) is
    // unparseable, actively masking the typo. Re-parse once here purely to report
    // a failure before probing.
    if let Some((path, err)) = config_parse_error() {
        eprintln!(
            "Warning: could not parse {}: {} — using default config values. \
             Fix the error above for your settings to take effect.",
            path.display(),
            err
        );
    }

    let f = configured_filter();

    let found = match hidapi::HidApi::new() {
        Ok(api) => api.device_list().any(|d| {
            d.usage_page() == f.usage_page
                && d.usage() == f.usage
                && f.vendor_id.is_none_or(|v| d.vendor_id() == v)
                && f.product_id.is_none_or(|p| d.product_id() == p)
        }),
        Err(e) => {
            eprintln!("Warning: could not enumerate HID devices: {}", e);
            return;
        }
    };

    let vid = f
        .vendor_id
        .map(|v| format!("{v:#06x}"))
        .unwrap_or_else(|| "any".to_string());
    let pid = f
        .product_id
        .map(|p| format!("{p:#06x}"))
        .unwrap_or_else(|| "any".to_string());

    if found {
        if verbose {
            println!(
                "Found QMK device {}:{} (raw HID {:#06x}:{:#06x})",
                vid, pid, f.usage_page, f.usage
            );
        }
    } else {
        println!(
            "No device matching {}:{} (raw HID usage page {:#06x}, usage {:#06x}) found.\n\
             Leave vendor_id/product_id unset in config.toml for auto-discovery,\n\
             or set them to disambiguate among multiple QMK keyboards.\n\
             Run `qmkonnect --list-devices` to see connected HID devices.",
            vid, pid, f.usage_page, f.usage
        );
    }
}

/// Is the configured QMK device currently plugged in?
///
/// This is a pure read-only enumeration of the kernel HID device list — it
/// never opens the device and never sends a report, so it cannot disturb the
/// keyboard. It backs the tray status indicator (macOS line 2 / the Linux SNI
/// status line / the Windows status item): the tray polls this on a background
/// thread and refreshes the label only when the answer changes.
pub fn is_device_connected() -> bool {
    let f = configured_filter();

    match hidapi::HidApi::new() {
        Ok(api) => api.device_list().any(|d| {
            d.usage_page() == f.usage_page
                && d.usage() == f.usage
                && f.vendor_id.is_none_or(|v| d.vendor_id() == v)
                && f.product_id.is_none_or(|p| d.product_id() == p)
        }),
        // If HID can't be enumerated at all, treat the device as absent.
        Err(_) => false,
    }
}

/// Capture the device-presence snapshot ONCE for the poll threads (P1.M3.T1.S1).
///
/// Called by each runner on the main thread, immediately before its
/// `if is_device_connected() { perform_handshake() }` startup block. Stores
/// the result in the set-once [`STARTUP_DEVICE_CONNECTED`]; the poll threads
/// read it via [`startup_device_was_connected`] to seed their `last` tracker
/// so a transient first-tick `false` (after a connected startup) is correctly
/// classified as a `Loss` (resetting [`HAS_HANDSHAKED`]) instead of a no-op.
/// A second call is a silent no-op (OnceLock::set returns Err, discarded).
pub fn record_startup_device_state() {
    let _ = STARTUP_DEVICE_CONNECTED.set(is_device_connected());
}

/// The device-presence value captured at startup by [`record_startup_device_state`]
/// (P1.M3.T1.S1). Poll threads seed their `last: Option<bool>` with
/// `Some(startup_device_was_connected())`. Defaults to `false` if
/// [`record_startup_device_state`] has not been called yet (e.g. in unit tests
/// before any record) — harmless: the poll thread then behaves as "absent at
/// startup", identical to the pre-fix cold-start path.
pub fn startup_device_was_connected() -> bool {
    *STARTUP_DEVICE_CONNECTED.get().unwrap_or(&false)
}

// ============================================================================
// Host-rules capability handshake (P4.M2.T1.S1, HOST_RULES.md §8(5))
// ============================================================================
// Once a QMK device is connected, `perform_handshake` discovers whether it
// speaks typed commands (`proto_ver == 2` + the `APPLY_HOST_CONTEXT` feature
// bit) and — if so — sweeps `QUERY_CALLBACK(i)` to build a `name → id` map.
// Legacy / non-capable / timeout / error replies leave `HOST_CAPABLE` false
// (string-only mode = today's behavior, bit-for-bit). The handshake runs at
// most once per board boot (the `HAS_HANDSHAKED` guard); P4.M2.T1.S2 resets it
// on a real device transition (false→true) to re-trigger.

/// Host-rules capability flag, set by [`perform_handshake`] at (re)connect.
/// `true` ⇒ the connected keyboard advertised `proto_ver == 2` + the
/// `APPLY_HOST_CONTEXT` feature bit (`feature_flags & 0x01`); P4.M3.T1.S1 gates
/// the `APPLY_HOST_CONTEXT` send on this. `false` (default, or legacy/timeout) ⇒
/// string-only mode (today's behavior, bit-for-bit). Read via [`host_capable`].
static HOST_CAPABLE: AtomicBool = AtomicBool::new(false);

/// The keyboard's callback registry as a `name → id` map, populated by the
/// `QUERY_CALLBACK` sweep in [`perform_handshake`]. P4.M3.T1.S1's
/// [`crate::core::rules::evaluate`] resolves `rules.toml` callback names through
/// it; P5.M1's `--list-callbacks` prints it. Read via [`callback_names`].
static CALLBACK_NAMES: Lazy<Mutex<HashMap<String, u8>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Dedup guard: the handshake runs **at most once per board boot** (the firmware
/// sets `has_been_queried` on the first `QUERY_INFO`). [`perform_handshake`]
/// swaps this to `true` on entry and short-circuits if already set. P4.M2.T1.S2
/// resets it (via [`reset_handshake_state`]) on a real device transition
/// (`is_device_connected()` false→true) to re-trigger.
static HAS_HANDSHAKED: AtomicBool = AtomicBool::new(false);

/// The device-presence snapshot captured ONCE at startup by
/// [`record_startup_device_state`], read by the poll threads to seed their
/// `last` tracker (P1.M3.T1.S1 / Finding #5). Without it the poll thread
/// starts at `None` and a transient first-tick `false` after a connected
/// startup never records a `Loss`, so [`HAS_HANDSHAKED`] stays `true` and a
/// reconnect skips the `SET_OS` re-send. `OnceLock` is set-once: the first
/// runner to call [`record_startup_device_state`] wins; subsequent calls are
/// no-ops (the poll threads only ever read).
static STARTUP_DEVICE_CONNECTED: OnceLock<bool> = OnceLock::new();

/// Dedup guard for the malformed-`rules.toml` desktop notification
/// (HOST_RULES.md §7). Set the first time a parse failure is notified; cleared
/// on a successful parse — so the notification fires at most once per broken
/// state, not on every window focus change.
static RULES_INVALID_NOTIFIED: AtomicBool = AtomicBool::new(false);

/// The host OS, for the `SET_OS` command. Determined at build time from
/// `cfg!(target_os)`; the host is the OS source of truth while connected
/// (HOST_RULES.md §5 C12). Returns [`qmk_notifier::HostOs::Unsure`] on
/// non-Linux/Windows/macOS targets.
fn host_os() -> qmk_notifier::HostOs {
    if cfg!(target_os = "linux") {
        qmk_notifier::HostOs::Linux
    } else if cfg!(target_os = "windows") {
        qmk_notifier::HostOs::Windows
    } else if cfg!(target_os = "macos") {
        qmk_notifier::HostOs::Macos // G7: lowercase 'os' in both cfg and the variant
    } else {
        qmk_notifier::HostOs::Unsure
    }
}

/// Run the host-rules capability handshake against the connected QMK device.
///
/// Sends `QUERY_INFO`; if the reply is `Info { proto_ver: 2, feature_flags,
/// callback_count, .. }` with the `APPLY_HOST_CONTEXT` bit set
/// (`feature_flags & 0x01`), the device is **capable**: send `SET_OS` once (host
/// is OS-authoritative), sweep `QUERY_CALLBACK(i)` for `i in 0..callback_count`
/// into the global [`CALLBACK_NAMES`] `name → id` map, validate `rules.toml`'s
/// callback names against it (warnings only — never fatal), and set
/// [`HOST_CAPABLE`] `true`. Any other reply — legacy (`proto_ver != 2`),
/// non-capable (`flags & 0x01 == 0`), `Timeout`, or a device error — leaves
/// [`HOST_CAPABLE`] `false` and clears the map (string-only mode; today's
/// behavior, bit-for-bit).
///
/// **Idempotent per board boot**: the first call swaps [`HAS_HANDSHAKED`] to
/// `true` and runs; subsequent calls short-circuit. P4.M2.T1.S2 resets the guard
/// (via [`reset_handshake_state`]) on a real device transition to re-trigger.
///
/// `verbose` gates the chatty progress logging (matching `startup_device_probe`'s
/// convention); capability-downgrade and rules-mismatch WARNINGS always print.
///
/// # Example
///
/// ```rust,ignore
/// use qmkonnect::core::notifier;
///
/// // Called by the runner at startup (P4.M2.T1.S2) and on device reconnect:
/// notifier::perform_handshake(verbose);
/// if notifier::host_capable() {
///     // P4.M3.T1.S1: also send APPLY_HOST_CONTEXT per window change.
/// }
/// ```
pub fn perform_handshake(verbose: bool) {
    perform_handshake_with(verbose, HandshakeOptions::full());
}

/// Knobs that distinguish the live connect/reconnect handshake from the
/// read-only `--validate-rules` lint.
///
/// The default [`Self::full`] option runs the whole capable arm (`SET_OS` +
/// callback sweep + default-rules validation); [`Self::validation`] is for
/// `--validate-rules`, which must NOT mutate firmware state (#6) and owns its
/// own callback-name check against the (possibly `--rules-path`-overridden)
/// file under validation (#7).
#[derive(Clone, Copy, Debug)]
pub struct HandshakeOptions {
    /// Send `SET_OS` once the board is confirmed capable (HOST_RULES.md §5 C12:
    /// host is OS-authoritative at connect). The live path sets this `true`;
    /// `--validate-rules` sets it `false` so the lint never writes `current_os`
    /// to the firmware.
    pub set_os: bool,
    /// Run the built-in `rules.toml` callback-name validation (against the
    /// DEFAULT rules path) after the sweep. The live path sets this `true`;
    /// `--validate-rules` sets it `false` because the lint does its own
    /// callback-name check against the file under validation — otherwise
    /// mismatch warnings about `~/.config/qmkonnect/rules.toml` intermix with
    /// the output for the file being linted.
    pub validate_default_rules: bool,
}

impl HandshakeOptions {
    /// Full live-connect handshake: send `SET_OS` + validate the default rules.
    /// This is what [`perform_handshake`] (the runner, tray reconnect, and
    /// `--list-callbacks`) uses.
    pub fn full() -> Self {
        Self {
            set_os: true,
            validate_default_rules: true,
        }
    }

    /// Read-only `--validate-rules` handshake: skip `SET_OS` (no firmware
    /// mutation) and skip the default-rules callback-name check (the lint owns
    /// that against the file under validation).
    pub fn validation() -> Self {
        Self {
            set_os: false,
            validate_default_rules: false,
        }
    }
}

/// Defensive ceiling on the `QUERY_CALLBACK` sweep (#4). The firmware's
/// `HOST_CALLBACK_MAX` bounds its static array; a `callback_count` above this is
/// almost certainly a misbehaving/buggy firmware, so we stop sweeping and warn
/// rather than risk a long stall. Generous well above any realistic keyboard's
/// registry.
const MAX_HOST_CALLBACKS: u8 = 64;

/// Worst-case wall-clock budget for the `QUERY_CALLBACK` sweep (#4). Each
/// timed-out query blocks up to ~`REPLY_READ_TIMEOUT_MS` (1 s) in the crate;
/// the sweep holds the global notifier mutex, so without a budget a buggy board
/// that reports `callback_count = 255` then stops replying could wedge EVERY
/// window notification for ~255 s. Five seconds is generous for a real keyboard
/// (handful of callbacks, each replying in well under a second) but bounds the
/// stall hard.
const CALLBACK_SWEEP_DEADLINE: Duration = Duration::from_secs(5);

/// Run the host-rules capability handshake with explicit [`HandshakeOptions`].
///
/// This is the full implementation; [`perform_handshake`] is a thin wrapper
/// that passes [`HandshakeOptions::full`]. See [`perform_handshake`] for the
/// behaviour and the dedup/state semantics; `opts` only gates the two
/// side-effecting steps that a read-only lint wants to skip (`SET_OS` and the
/// default-rules callback-name validation).
pub fn perform_handshake_with(verbose: bool, opts: HandshakeOptions) {
    // Dedup: at most once per board boot (firmware has_been_queried). S2 resets.
    if HAS_HANDSHAKED.swap(true, Ordering::SeqCst) {
        if verbose {
            eprintln!(
                "[{}ms] perform_handshake: already handshaked this session — skipping",
                crate::core::now_ms()
            );
        }
        return;
    }

    let filter = configured_filter();
    let notifier = get_notifier();
    let n = notifier.lock().unwrap();

    match n.send_command(qmk_notifier::RunCommand::QueryInfo, &filter) {
        Ok(qmk_notifier::CommandResponse::Info {
            proto_ver: 2,
            feature_flags,
            callback_count,
            board_rules_present,
        }) if feature_flags & 0x01 != 0 => {
            if verbose {
                eprintln!(
                    "[{}ms] perform_handshake: proto v2 capable (flags={:#04x}, {} callbacks, board_rules={})",
                    crate::core::now_ms(), feature_flags, callback_count, board_rules_present
                );
            }
            // SET_OS once (host is OS-authoritative at connect). Best-effort.
            // Skipped in the read-only `--validate-rules` mode (#6) so the lint
            // never mutates firmware `current_os`.
            if opts.set_os {
                if let Err(e) = n.send_command(qmk_notifier::RunCommand::SetOs(host_os()), &filter)
                {
                    eprintln!("Warning: SET_OS failed during handshake: {}", e);
                }
            }
            // Release the QueryInfo+SetOs lock BEFORE the sweep. The sweep now re-acquires
            // NOTIFIER per iteration (#4) so a window-notification send (`notify_qmk`'s
            // immediate arm or `debounce_worker`'s flush) can acquire it between any two
            // `QueryCallback` iterations instead of blocking for the whole sweep. Each
            // `send_command` opens the HID device independently (`qmk_notifier::run`), so
            // releasing and re-acquiring between iterations is safe — no shared device-handle
            // state. `CALLBACK_NAMES` is published atomically AFTER the sweep, and
            // `BOARD_HAS_RULES`/`HOST_CAPABLE` are set after it too, so a concurrent
            // notification during the sweep only sees the pre-handshake (stale/empty) callback
            // map — identical to the pre-handshake state.
            drop(n);
            // Callback sweep → local map (publish after the sweep: D2).
            // #4 (secondary bound): `MAX_HOST_CALLBACKS` + `CALLBACK_SWEEP_DEADLINE` still cap a
            // misbehaving firmware per iteration as defense-in-depth (the primary mitigation is
            // the per-iteration lock release above).
            let sweep_start = Instant::now();
            let sweep_cap = callback_count.min(MAX_HOST_CALLBACKS);
            if callback_count > MAX_HOST_CALLBACKS {
                eprintln!(
                    "Warning: firmware reported {} callbacks; sweeping only the first {} \
                     (a real keyboard stays well under HOST_CALLBACK_MAX)",
                    callback_count, MAX_HOST_CALLBACKS
                );
            }
            let mut local: HashMap<String, u8> = HashMap::new();
            for i in 0..sweep_cap {
                // #4: deadline check at the TOP of each iteration, BEFORE re-locking NOTIFIER,
                // so a per-iteration release actually hands the lock to a waiter before we stop.
                if sweep_start.elapsed() > CALLBACK_SWEEP_DEADLINE {
                    eprintln!(
                        "Warning: callback sweep exceeded {}s budget at index {} \
                         ({} of {} done) — stopping early to avoid wedging notifications",
                        CALLBACK_SWEEP_DEADLINE.as_secs(),
                        i,
                        local.len(),
                        sweep_cap
                    );
                    break;
                }
                // Re-acquire NOTIFIER for THIS iteration only — a window notification can now
                // interleave between any two iterations.
                let n = notifier.lock().unwrap();
                match n.send_command(qmk_notifier::RunCommand::QueryCallback(i), &filter) {
                    Ok(qmk_notifier::CommandResponse::CallbackName {
                        index,
                        name: Some(name),
                    }) => {
                        local.insert(name, index); // echo the firmware's index for robustness
                    }
                    Ok(qmk_notifier::CommandResponse::CallbackName { name: None, .. }) => {
                        if verbose {
                            eprintln!(
                                "[{}ms] perform_handshake: callback {} has no name — skipped",
                                crate::core::now_ms(),
                                i
                            );
                        }
                    }
                    Ok(other) => {
                        if verbose {
                            eprintln!(
                                "[{}ms] perform_handshake: callback {} unexpected reply {:?}",
                                crate::core::now_ms(),
                                i,
                                other
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: QUERY_CALLBACK({}) failed: {}", i, e);
                    }
                }
                drop(n); // release NOTIFIER before the next iteration (per-iteration release)
                         // #4: yield so a window-notification waiter (`notify_qmk`'s immediate send or
                         // `debounce_worker`'s flush — both BLOCKING on NOTIFIER) actually gets to acquire
                         // the lock before we re-lock for the next iteration. Without this, std::sync::Mutex's
                         // unfair barging re-acquires in ~ns and starves the woken waiter for the whole sweep,
                         // defeating the per-iteration release. sched_yield is ~1µs and a no-op when nothing
                         // else is runnable (N<=64 iterations => <=~64µs/handshake, negligible).
                thread::yield_now();
            }
            {
                let mut names = CALLBACK_NAMES.lock().unwrap();
                names.clear();
                names.extend(local);
            }
            // #7: skip the default-rules callback-name check in `--validate-rules`
            // mode — the lint owns that against the (possibly overridden) file
            // under validation, so default-file warnings don't intermix.
            if opts.validate_default_rules {
                validate_rules_callback_names(verbose);
            }
            // #5: set BOARD_HAS_RULES BEFORE HOST_CAPABLE so there is no window
            // in which host_capable()==true but board_has_rules() still reads the
            // stale `false` left by reset_handshake_state (which would force a
            // spurious replace for one window even when the user configured
            // stack). When host_capable() flips true, board_has_rules() is
            // already correct.
            BOARD_HAS_RULES.store(board_rules_present, Ordering::SeqCst);
            HOST_CAPABLE.store(true, Ordering::SeqCst);
            if verbose {
                eprintln!(
                    "[{}ms] perform_handshake: complete — capable ({} callbacks mapped)",
                    crate::core::now_ms(),
                    CALLBACK_NAMES.lock().unwrap().len()
                );
            }
        }
        // LOW-1: a `Timeout` means the firmware never confirmed receipt of
        // QUERY_INFO (no reply at all — flaky USB, host busy, or a TOCTOU
        // unplug mid-send). Re-querying on the next poll/reconnect is safe and
        // desirable, so release the dedup token. This is distinct from a
        // genuine legacy `Info` reply (proto_ver != 2 / no feature bit), where
        // the firmware *did* set `has_been_queried` and the token must stay
        // consumed (R6 mid-session-reconnect side effect).
        Ok(qmk_notifier::CommandResponse::Timeout) => {
            drop(n);
            HOST_CAPABLE.store(false, Ordering::SeqCst);
            CALLBACK_NAMES.lock().unwrap().clear();
            HAS_HANDSHAKED.store(false, Ordering::SeqCst); // transient — allow retry
            if verbose {
                eprintln!(
                    "[{}ms] perform_handshake: query timed out (transient) — string-only mode, will retry on reconnect",
                    crate::core::now_ms()
                );
            }
        }
        Ok(other) => {
            drop(n);
            HOST_CAPABLE.store(false, Ordering::SeqCst);
            CALLBACK_NAMES.lock().unwrap().clear();
            if verbose {
                eprintln!(
                    "[{}ms] perform_handshake: non-capable reply ({:?}) — string-only mode",
                    crate::core::now_ms(),
                    other
                );
            }
        }
        Err(e) => {
            drop(n);
            HOST_CAPABLE.store(false, Ordering::SeqCst);
            CALLBACK_NAMES.lock().unwrap().clear();
            // LOW-1: a device error means QUERY_INFO never landed on the
            // firmware — release the dedup token so the next poll/reconnect
            // retries the handshake against the capable board.
            HAS_HANDSHAKED.store(false, Ordering::SeqCst);
            if verbose {
                eprintln!(
                    "[{}ms] perform_handshake: device error ({}) — string-only mode, will retry on reconnect",
                    crate::core::now_ms(),
                    e
                );
            }
        }
    }
}

/// Best-effort validation of `rules.toml` callback names against [`CALLBACK_NAMES`].
///
/// Reads the first existing `rules.toml` candidate
/// ([`crate::core::rules::get_rules_paths`]); if none exists, host rules are
/// disabled and there is nothing to validate. A malformed `rules.toml` is warned
/// about and skipped (the strict failure is `--validate-rules`'s job, P5.M1) — it
/// never fails the handshake. Unknown callback names (referenced in
/// `[[rule]]` `enable`/`disable` but absent from the keyboard's
/// registry) are warned, one per line. [`HOST_CAPABLE`] is unaffected (a broken
/// rules file does not downgrade capability).
fn validate_rules_callback_names(verbose: bool) {
    let Some(path) = crate::core::rules::get_rules_paths()
        .into_iter()
        .find(|p| p.exists())
    else {
        if verbose {
            eprintln!(
                "[{}ms] perform_handshake: no rules.toml found — skipping callback-name validation",
                crate::core::now_ms()
            );
        }
        return;
    };
    let rules = match crate::core::rules::parse_rules(&path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "Warning: could not parse {} ({}) — skipping callback-name validation",
                path.display(),
                e
            );
            return;
        }
    };
    let known = CALLBACK_NAMES.lock().unwrap().clone();
    let unknown = unknown_callback_names(&rules, &known);
    for name in &unknown {
        eprintln!(
            "Warning: rules.toml references callback \"{}\" which is not registered on this keyboard ({} known)",
            name,
            known.len()
        );
    }
    if verbose && !unknown.is_empty() {
        eprintln!(
            "[{}ms] perform_handshake: {} unknown callback name(s) in rules.toml",
            crate::core::now_ms(),
            unknown.len()
        );
    }
}

/// Callback names referenced by `rules.toml` but absent from the keyboard's
/// registry. Deduped + sorted (via `BTreeSet`) for deterministic output. This is
/// the pure, testable core of [`validate_rules_callback_names`].
fn unknown_callback_names(
    rules: &crate::core::rules::RuleSet,
    known: &HashMap<String, u8>,
) -> Vec<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for rule in &rules.rules {
        for name in rule.enable.iter().chain(rule.disable.iter()) {
            if !known.contains_key(name) {
                seen.insert(name.clone());
            }
        }
    }
    seen.into_iter().collect()
}

/// Is the connected keyboard host-rules-capable (`proto_ver == 2` +
/// `flags & 0x01`)? P4.M3.T1.S1 gates `APPLY_HOST_CONTEXT` on this.
pub fn host_capable() -> bool {
    HOST_CAPABLE.load(Ordering::SeqCst)
}

/// The keyboard's `name → id` callback map (a clone). P4.M3.T1.S1 passes this
/// into [`crate::core::rules::evaluate`]; P5.M1's `--list-callbacks` prints it.
/// Empty when not capable.
pub fn callback_names() -> HashMap<String, u8> {
    CALLBACK_NAMES.lock().unwrap().clone()
}

/// Clear all handshake state (capability flag, callback map, dedup guard).
///
/// Called by P4.M2.T1.S2 on a real device transition (`is_device_connected()`
/// false→true) so the next [`perform_handshake`] re-runs, and by the handshake
/// tests for isolation.
pub fn reset_handshake_state() {
    HOST_CAPABLE.store(false, Ordering::SeqCst);
    BOARD_HAS_RULES.store(false, Ordering::SeqCst);
    CALLBACK_NAMES.lock().unwrap().clear();
    HAS_HANDSHAKED.store(false, Ordering::SeqCst);
}

/// Three-state device status for the tray/menu-bar status line
/// (`spec/DEVICE_DISCOVERY.md` §3). Derived from the two booleans the existing
/// poll-thread lifecycle already maintains — see [`device_status`].
// Consumed by the S2/S3 tray poll threads (src/tray.rs / src/linux_tray.rs);
// until those land, non-test builds flag this pub enum as unused.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus {
    /// ≥1 **capable** board present (`is_device_connected() && host_capable()`).
    /// Tray: `● Device Connected`. Icon: solid `U+25CF`, full alpha.
    Connected,
    /// ≥1 Tier-1 board present, **0 capable** (`is_device_connected() &&
    /// !host_capable()`). The truthful "flash qmk_notifier" state. Tray:
    /// `⚠ QMK board found — no qmk_notifier module (flash it)`.
    NoModule,
    /// 0 Tier-1 boards present (`!is_device_connected()`). Tray:
    /// `○ No Device Connected`. Icon: hollow `U+25CB`, dimmed.
    Disconnected,
}

/// The device-status for the tray/menu-bar status line right now
/// (`spec/DEVICE_DISCOVERY.md` §3, `ARCHITECTURE.md` §5.6).
///
/// Derives the three-state value from the two booleans the existing poll-thread
/// lifecycle already maintains — it does **not** send any HID command or open any
/// device (no per-path `QUERY_INFO` ping; that is P3's cache-backed
/// `classify_devices()`):
/// - [`is_device_connected()`] — pure Tier-1 enumeration (any `0xFF60`/`0x61`
///   interface matching the configured filter; never opens/sends).
/// - [`host_capable()`] — reads the [`HOST_CAPABLE`] `AtomicBool`, set `true` by
///   the handshake on a capable `QUERY_INFO` reply and reset `false` on a device
///   Loss / failure.
///
/// | Status        | Condition                              |
/// |---------------|----------------------------------------|
/// | `Disconnected`| `!is_device_connected()`               |
/// | `NoModule`    | `is_device_connected() && !host_capable()` |
/// | `Connected`   | `is_device_connected() && host_capable()`  |
///
/// **Transient caveat:** right after a device Gain, `host_capable()` is `false`
/// until `perform_handshake` completes (sub-second); the line may briefly read
/// `NoModule` before flipping to `Connected`. Acceptable per spec.
///
/// The pure truth table lives in [`classify_device_status`] so it is unit-testable
/// without a real device (Tier-1 enumeration reflects actual hardware, which is
/// absent in CI).
// Consumed by the S2/S3 tray poll threads (src/tray.rs / src/linux_tray.rs);
// until those land, non-test builds flag this pub fn as unused.
#[allow(dead_code)]
pub fn device_status() -> DeviceStatus {
    classify_device_status(is_device_connected(), host_capable())
}

/// Pure three-state classifier — the testable truth table for [`device_status`].
///
/// Split out so the three derivations can be unit-tested deterministically:
/// [`is_device_connected`] enumerates real HID hardware (always `false` in CI),
/// so [`device_status`] itself can only naturally produce [`DeviceStatus::Disconnected`]
/// in the test environment. This helper takes the two booleans directly.
#[allow(dead_code)]
fn classify_device_status(present: bool, capable: bool) -> DeviceStatus {
    if !present {
        DeviceStatus::Disconnected
    } else if capable {
        DeviceStatus::Connected
    } else {
        DeviceStatus::NoModule
    }
}

// ===== Device classification (P3.M1) — per-candidate capability tier =====
// The data model + TTL cache infrastructure for the discovered-device picker
// (`spec/DEVICE_DISCOVERY.md` §2). Populated by `classify_devices()`
// (P3.M1.T1.S2 — not yet implemented) and read by the picker (P3.M2) + the
// status resolver. This section ships the TYPES + CACHE + HELPERS only; S2
// owns the hidapi/send_command probe logic.

/// Per-device capability classification — the result of the Tier-2 capability
/// probe (`spec/DEVICE_DISCOVERY.md` §2.2). A `Capable` board replied to the
/// `QUERY_INFO` typed command with `proto_ver == 2` + the host-rules feature
/// bit; its four fields mirror the `qmk_notifier::CommandResponse::Info`
/// variant (crate rev `f26893e`, `lib.rs:95-99`) field-for-field. Every other
/// reply (`Legacy` / `Timeout` / an error / no reply — the pure-VIA case) or a
/// Tier-1-present-but-unprobed interface classifies as `NotQmkNotifier`.
///
/// This is the **per-device** complement of the AGGREGATE three-state
/// [`DeviceStatus`] tray status (P1.M1.T1.S1): `device_status()` is
/// conceptually a fold over a set of these per-device kinds (produced by S2's
/// `classify_devices()`). Distinct names, distinct semantics.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceKind {
    /// The board advertised `proto_ver == 2` + the host-rules feature bit. The
    /// four fields mirror `qmk_notifier::CommandResponse::Info` (crate rev
    /// `f26893e`) field-for-field so S2's probe match is a 1:1 structural copy.
    Capable {
        proto_ver: u8,
        feature_flags: u8,
        callback_count: u8,
        board_rules_present: bool,
    },
    /// Tier-1-present but not qmk_notifier-capable: a pure-VIA / Vial board, a
    /// legacy reply, or a board that timed out (the normal pure-VIA case — the
    /// firmware's `raw_hid_receive` never answers the magic header).
    NotQmkNotifier,
}

/// One enumerated Tier-1 HID interface (`usage_page == 0xFF60 && usage ==
/// 0x61`) plus its Tier-2 classification (`spec/DEVICE_DISCOVERY.md` §2.3 /
/// §5). `path` is the stable hidapi `DeviceInfo::path()` and the
/// [`CLASSIFICATION_CACHE`] key (the picker/status care WHICH physical device
/// is capable). Returned by `classify_devices()` (P3.M1.T1.S2) and rendered
/// row-by-row by the discovered-device picker (P3.M2 — the `kind` column:
/// `Capable` ⇒ "qmk_notifier ✓", `NotQmkNotifier` ⇒ "QMK board, no module").
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct ClassifiedDevice {
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub product_name: Option<String>,
    pub usage_page: u16,
    pub usage: u16,
    pub kind: DeviceKind,
}

/// TTL for [`CLASSIFICATION_CACHE`] entries (`spec/DEVICE_DISCOVERY.md` §2.3).
/// Default 5 s so the hot status-poll thread (macOS/Windows 3 s, Linux 1 s) does
/// not re-ping on every tick — classification is **event-driven** (once per
/// device appearance), with the cached `DeviceKind` reused until the device
/// disappears or the TTL expires. Mirrors the `CALLBACK_SWEEP_DEADLINE`
/// `Duration`-const idiom.
#[allow(dead_code)]
const CLASSIFICATION_TTL: Duration = Duration::from_secs(5);

/// Per-device classification cache, keyed by the stable hidapi `path`
/// (`spec/DEVICE_DISCOVERY.md` §2.3). Value is `(DeviceKind, Instant)` where the
/// `Instant` stamps when it was classified for the TTL check. **PRIVATE** —
/// access via the three `classification_cache_*` helpers below (mirrors the
/// `HOST_CAPABLE`/`CALLBACK_NAMES` private-static + pub-reader/writer idiom).
///
/// Keyed by **path** (not vid/pid) because the crate has no per-path send
/// (`external_deps.md`: `MatchKey` is private + filter-keyed); S2 narrows the
/// *filter* to a candidate's vid/pid, while the picker/status care about which
/// *physical interface* is capable. Populated by `classify_devices()` (S2) on a
/// Tier-1 false→true transition; read by the picker (P3.M2) + the status
/// resolver; cleared on a real device transition (device-loss / board swap).
#[allow(dead_code)]
static CLASSIFICATION_CACHE: Lazy<Mutex<HashMap<String, (DeviceKind, Instant)>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Look up a device's cached [`DeviceKind`] by its stable hidapi `path`
/// (`spec/DEVICE_DISCOVERY.md` §2.3 / §2.4). Returns `None` when the path is
/// absent OR when the entry is older than [`CLASSIFICATION_TTL`] (lazy expiry —
/// the stale entry is left in place; a later [`classification_cache_insert`]
/// overwrites it; S2 is the eviction authority). Returns `Some(kind)` (cloned)
/// only when fresh. S2's `classify_devices()` calls this before pinging a
/// candidate, and after every successful per-candidate probe.
///
/// Side-effect-free: never mutates the map and never calls `Instant::now()` at
/// module load. A poisoned lock yields `None` (the crate has no poison-recovery
/// policy; a cache miss is the safe degradation, not a panic).
#[allow(dead_code)]
pub fn classification_cache_get(path: &str) -> Option<DeviceKind> {
    let map = CLASSIFICATION_CACHE.lock().ok()?;
    let (kind, stamped) = map.get(path)?;
    if stamped.elapsed() < CLASSIFICATION_TTL {
        Some(kind.clone())
    } else {
        None
    }
}

/// Record (or refresh) a device's classification, stamping `Instant::now()`
/// (`spec/DEVICE_DISCOVERY.md` §2.3). Overwrites any prior entry for `path` —
/// so a stale-but-not-yet-expired entry is refreshed in place. Called by S2's
/// `classify_devices()` after each successful per-candidate probe. A poisoned
/// lock is a no-op (never panics on the cache write path).
#[allow(dead_code)]
pub fn classification_cache_insert(path: &str, kind: DeviceKind) {
    if let Ok(mut map) = CLASSIFICATION_CACHE.lock() {
        map.insert(path.to_string(), (kind, Instant::now()));
    }
}

/// Drop every cached entry (`spec/DEVICE_DISCOVERY.md` §2.3). Called on a real
/// device transition (device-loss) and by the tray "Reload rules" / picker
/// "Rescan" path so stale classifications don't survive a board swap. A
/// poisoned lock is a no-op.
#[allow(dead_code)]
pub fn classification_cache_clear() {
    if let Ok(mut map) = CLASSIFICATION_CACHE.lock() {
        map.clear();
    }
}

// ===== (end Device classification — P3.M1) =====

/// What the host-rules handshake lifecycle should do for a device-status transition.
///
/// Computed by [`handshake_action`] from the previous and current
/// [`is_device_connected`] results, and consumed by the device-status poll
/// threads (`tray` on macOS/Windows, `linux_tray` on Linux) and
/// the startup path so the three call sites stay in lockstep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeAction {
    /// No transition, or `None → false` at startup: nothing to do.
    None,
    /// Device became present (`None → true` or `Some(false) → true`): run
    /// [`perform_handshake`]. Idempotent via [`HAS_HANDSHAKED`] — a no-op if the
    /// runner already handshooked at startup, so the poll thread's first tick on
    /// an already-connected device is harmless.
    Gain,
    /// Device went away (`Some(true) → false`): call [`reset_handshake_state`] so
    /// the next [`HandshakeAction::Gain`] re-runs the handshake.
    Loss,
}

/// Classify a device-status transition into a handshake lifecycle action.
///
/// Pure mapping; unit-tested without a device or UI thread. The poll threads call
/// this with their previous and latest [`is_device_connected`] results.
///
/// | `prev`          | `now`   | Action  |
/// | --------------- | ------- | ------- |
/// | `None`          | `true`  | `Gain`  | (startup already connected)
/// | `None`          | `false` | `None`  |
/// | `Some(false)`   | `true`  | `Gain`  | (reconnect)
/// | `Some(true)`    | `true`  | `None`  | (no change)
/// | `Some(false)`   | `false` | `None`  | (no change)
/// | `Some(true)`    | `false` | `Loss`  | (real disconnect)
pub fn handshake_action(prev: Option<bool>, now: bool) -> HandshakeAction {
    match (prev, now) {
        (Some(true), false) => HandshakeAction::Loss, // real disconnect
        (p, true) if p != Some(true) => HandshakeAction::Gain, // None→true OR false→true
        _ => HandshakeAction::None,                   // no change OR None→false
    }
}
// NOTE: the Gain arm is GUARDED (`if p != Some(true)`) — the naive `(_, true)
// => Gain` would mis-classify (Some(true), true) (no change) as Gain.

// R-COEX (VIA coexistence, F14) — must-preserve invariant at this transport
// boundary. spec/DEVICE_DISCOVERY.md §6; spec/ARCHITECTURE.md §10 #10.
//
//   1. NEVER a seize/exclusive open. Every HID handle uses hidapi's DEFAULT
//      shared open (Win FILE_SHARE_READ|WRITE, macOS kIOHIDOptionsTypeNone,
//      Linux plain hidraw open()). hidapi 2.x exposes NO seize API. Enforced
//      in the crate's private open_matching_devices (qmk_notifier core.rs);
//      the app cannot reach the open. Documented here, not asserted.
//   2. NEVER a perpetual blocking read. Reads are bounded drains
//      (read_timeout(0), IN_DRAIN_MAX=32) in short windows around writes.
//      Enforced in the crate's private burst_to_one. Documented, not asserted.
//   3. FIRST emitted payload byte is ALWAYS 0x81 (the 0x81 0x9F magic header;
//      firmware demuxes, VIA ignores 0x81-prefixed input). ASSERTED by the
//      r_coex_invariants tests below (variant-level: the app's transport path
//      never constructs the wire-silent RunCommand::ListDevices).
//
// This impl block is the SINGLE transport boundary: both `notify` and
// `send_command` build RunParameters and call qmk_notifier::run(params) —
// the app's only device egress. See the // R-COEX: markers at each egress.
impl Notifier for QmkNotifier {
    fn notify(&self, message: String) -> Result<(), Box<dyn Error + Send + Sync>> {
        let f = configured_filter();

        // Retry device connection with exponential backoff
        for attempt in 1..=3 {
            let params = qmk_notifier::RunParameters::new(
                // R-COEX: SendMessage → 0x81 0x9F magic header (crate burst_to_one).
                qmk_notifier::RunCommand::SendMessage(message.clone()),
                f.vendor_id,
                f.product_id,
                f.usage_page,
                f.usage,
                false, // verbose
            );
            // R-COEX: sole device egress for the string path; rules 1–3 hold (see impl-block invariant).
            match qmk_notifier::run(params) {
                Ok(_) => return Ok(()),
                Err(e) => {
                    let error_str = e.to_string().to_lowercase();

                    // Only retry for device-related errors
                    if error_str.contains("no device found")
                        || error_str.contains("permission denied")
                        || error_str.contains("failed to open")
                    {
                        if attempt < 3 {
                            let delay = Duration::from_millis(100 * attempt as u64);
                            thread::sleep(delay);
                            continue;
                        }

                        // After 3 attempts, log and return success to prevent service restart
                        eprintln!("QMK device unavailable after {} attempts: {}", attempt, e);
                        return Ok(()); // Don't fail the service for device issues
                    }

                    // For non-device errors, fail immediately
                    return Err(Box::new(e));
                }
            }
        }

        Ok(())
    }

    fn send_command(
        &self,
        command: qmk_notifier::RunCommand,
        filter: &DeviceFilter,
    ) -> Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>> {
        let params = qmk_notifier::RunParameters::new(
            command, // R-COEX: every transport-path RunCommand variant (QueryInfo/QueryCallback/SetOs/ApplyHostContext) emits 0x81 first.
            filter.vendor_id,
            filter.product_id,
            filter.usage_page,
            filter.usage,
            false, // verbose — transport stays quiet; orchestration logs (D3)
        );
        // R-COEX: sole device egress for the typed path; rules 1–3 hold.
        match qmk_notifier::run(params) {
            Ok(resp) => Ok(resp),
            Err(e) => Err(Box::new(e)), // G3: QmkError: Error+Send+Sync, coerces directly
        }
    }
}

// Static instance of the notifier
static NOTIFIER: Lazy<Arc<Mutex<Box<dyn Notifier>>>> =
    Lazy::new(|| Arc::new(Mutex::new(Box::new(QmkNotifier) as Box<dyn Notifier>)));

// A debounced window-change message awaiting its flush: the formatted string
// payload (sent as the legacy `notify` string) together with the originating
// [`WindowInfo`]. The window is carried so the host-side-rules send
// (P4.M3.T1.S1) can evaluate `rules.toml` and emit `APPLY_HOST_CONTEXT` at
// flush time — without it the worker would only know the `\x1D`-joined blob.
struct PendingMessage {
    payload: String,
    window_info: WindowInfo,
}

// Debounce state. A single, long-lived worker thread (see WORKER) consumes
// `pending` messages, replacing the former "spawn a thread per burst" scheme.
//
// `debounce_ms` is hot config (PRD §8, ARCHITECTURE.md §10 #4, CONFIG.md §1.2):
// it is re-read from `configured_debounce_ms()` on every notification, so it
// is intentionally re-resolved here each call — editing `config.toml` takes
// effect within ~3 s with no restart. The underlying value IS now mtime-cached
// in `cached_config()` (shared by `configured_timing` + `configured_filter`,
// coalescing the per-send double-read), but `interval()` still re-resolves the
// effective window each call: an mtime change invalidates the cache on the
// next call (~instant, not via a TTL delay), so hot-config is preserved.
// (`poll_interval_ms` is hot too, but it lives in the Hyprland poll thread, not
// here — that thread re-reads `configured_timing()` on every iteration; see
// `hyprland.rs`.)
struct DebounceState {
    /// `None` until the first notification has actually been sent.
    last_sent_time: Option<Instant>,
    /// Latest message queued for a debounced send.
    pending: Option<PendingMessage>,
    verbose: bool,
    /// Test-only override of the debounce window; when `Some`, used instead of
    /// re-reading `configured_debounce_ms()`. Production code leaves this
    /// `None` so the window is genuinely hot-config.
    #[cfg(test)]
    interval_override: Option<Duration>,
}

#[cfg(not(test))]
impl DebounceState {
    /// Effective debounce window, re-read fresh from config each call (hot).
    fn interval(&self) -> Duration {
        Duration::from_millis(crate::core::configured_debounce_ms())
    }
}

#[cfg(test)]
impl DebounceState {
    /// Effective debounce window: the test override if set, else the live
    /// config value (so tests that don't pin the interval still observe
    /// hot-config behavior).
    fn interval(&self) -> Duration {
        self.interval_override
            .unwrap_or_else(|| Duration::from_millis(crate::core::configured_debounce_ms()))
    }
}

static STATE: Lazy<Mutex<DebounceState>> = Lazy::new(|| {
    Mutex::new(DebounceState {
        last_sent_time: None,
        pending: None,
        verbose: false,
        #[cfg(test)]
        interval_override: None,
    })
});
static COND: Lazy<Condvar> = Lazy::new(Condvar::new);

fn get_notifier() -> Arc<Mutex<Box<dyn Notifier>>> {
    Arc::clone(&NOTIFIER)
}

/// The single debounce worker. It blocks until a message is pending, then waits
/// out the remainder of the debounce window (measured from the last *sent*
/// notification) and flushes the newest pending message. Because each new
/// pending message does not reset `last_sent_time`, a rapid burst collapses to
/// exactly one follow-up send.
fn debounce_worker() {
    loop {
        let to_send = {
            let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());

            // Wait until something is actually queued.
            while state.pending.is_none() {
                state = COND.wait(state).unwrap_or_else(|e| e.into_inner());
            }

            // Wait out the debounce window relative to the last send.
            let mut to_send: Option<(PendingMessage, bool)> = None;
            while to_send.is_none() {
                let last = state.last_sent_time.unwrap_or_else(Instant::now);
                let target = last + state.interval();
                let now = Instant::now();
                if now >= target {
                    // The pending message may have been cleared by
                    // `notify_qmk`'s immediate-send (`due`) branch — which sets
                    // `pending = None` and bumps `last_sent_time` — while we
                    // were parked in `wait_timeout`. Taking `.unwrap()` on an
                    // empty pending would PANIC (crashing the worker thread;
                    // with `panic = "abort"` in release, the whole service).
                    // Re-check and, if it raced to None, fall back to the outer
                    // wait for the next message instead of crashing.
                    let pm = match state.pending.take() {
                        Some(pm) => pm,
                        None => break,
                    };
                    let verbose = state.verbose;
                    state.last_sent_time = Some(Instant::now());
                    to_send = Some((pm, verbose));
                } else {
                    state = COND
                        .wait_timeout(state, target - now)
                        .unwrap_or_else(|e| e.into_inner())
                        .0;
                }
            }
            to_send
        };

        if let Some((pm, verbose)) = to_send {
            // Host-rules send (P4.M3.T1.S1 / HOST_RULES.md §8(4)): evaluate
            // rules.toml against this window and, when host-capable, emit
            // ApplyHostContext alongside (stack) or instead of (replace/no-match)
            // the legacy string. Legacy string bytes + cadence are unchanged.
            let PendingMessage {
                payload: message,
                window_info,
            } = pm;
            let filter = configured_filter();
            let ctx = host_context_for_window(&window_info, verbose);
            let notifier = get_notifier();
            let notifier = notifier.lock().unwrap();
            let _res =
                dispatch_window_send(&**notifier, &filter, &message, ctx, "debounced", verbose);
            if let Err(e) = _res {
                eprintln!("Error sending debounced notification: {}", e);
            }
        }
    }
}

// Spawn the worker exactly once on first use.
static WORKER: Lazy<thread::JoinHandle<()>> = Lazy::new(|| thread::spawn(debounce_worker));

/// Ensure the debounce worker is running. Cheap after the first call.
fn ensure_worker() {
    let _ = &*WORKER;
}

// For testing: Set a custom notifier
#[cfg(test)]
pub fn set_notifier(notifier: Box<dyn Notifier>) {
    // Ensure the static has been initialized first
    let _ = &*NOTIFIER;

    {
        let mut n = NOTIFIER.lock().unwrap();
        *n = notifier;
    }
}

pub fn notify_qmk(
    window_info: &WindowInfo,
    verbose: bool,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let message = format!("{}{}{}", window_info.app_class, "\x1D", window_info.title);
    ensure_worker();

    let send_immediately = {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        state.verbose = verbose;

        let now = Instant::now();
        let interval = state.interval();
        let due = state
            .last_sent_time
            .map(|t| now.duration_since(t) >= interval)
            .unwrap_or(true); // Never sent before -> send immediately.

        if due {
            state.last_sent_time = Some(now);
            state.pending = None;
            true
        } else {
            state.pending = Some(PendingMessage {
                payload: message.clone(),
                window_info: window_info.clone(),
            });
            COND.notify_one();
            false
        }
    };

    if send_immediately {
        // Routes through dispatch_window_send (HOST_RULES.md §8(4)); the string
        // result is propagated via `?` (preserved from the pre-host-rules path).
        let filter = configured_filter();
        let ctx = host_context_for_window(window_info, verbose);
        let notifier = get_notifier();
        let notifier = notifier.lock().unwrap();
        let _res = dispatch_window_send(&**notifier, &filter, &message, ctx, "immediate", verbose);
        _res?;
    } else if verbose {
        let sanitized = message.replace('\x1D', "|");
        println!(
            "[{}ms] Debouncing notification: {}",
            crate::core::now_ms(),
            sanitized
        );
    }

    Ok(())
}

// ============================================================================
// Host-context send pipeline (P4.M3.T1.S1 / HOST_RULES.md §8(4))
// ============================================================================
// When the connected board is host-capable (proto v2 + feature bit) AND a
// `rules.toml` is present, evaluate it against each window change and emit
// `ApplyHostContext` alongside (stack) or instead of (replace/no-match) the
// legacy `SendMessage` string. When host rules are disabled (legacy board, no
// `rules.toml`, or a malformed file) the legacy string-only path runs
// bit-for-bit as before. Both send blocks (the debounce worker flush and
// `notify_qmk`'s immediate path) route through [`dispatch_window_send`].

/// Does the connected keyboard's keymap declare board rules? Populated by
/// [`perform_handshake`] (the firmware's `board_rules_present` bit) alongside
/// [`HOST_CAPABLE`]; read by [`host_context_for_window`] to pass into
/// [`crate::core::rules::evaluate`] so the stack-vs-replace decision knows
/// whether the board would run its own rules for the string. `false` until a
/// capable handshake sets it, and on legacy/offline boards (where host rules
/// are disabled anyway).
static BOARD_HAS_RULES: AtomicBool = AtomicBool::new(false);

/// Read [`BOARD_HAS_RULES`]. Only consulted when [`host_capable`] is `true`
/// (the send gate), so a stale value on a non-capable board is never read.
pub fn board_has_rules() -> bool {
    BOARD_HAS_RULES.load(Ordering::SeqCst)
}

/// Evaluate `rules.toml` against one window, or `None` when host rules are
/// disabled (not host-capable, no `rules.toml` present, or a malformed file).
///
/// Returning `None` signals [`dispatch_window_send`] to send the legacy string
/// only — identical to the pre-host-rules behavior (HOST_RULES.md §8(8)).
///
/// # Example
///
/// ```rust,ignore
/// let ctx = host_context_for_window(&window_info, verbose);
/// match ctx {
///     None => { /* legacy string-only path */ }
///     Some(c) => { /* stack / replace / no-match per c.any_match & c.clear_board */ }
/// }
/// ```
fn host_context_for_window(
    window_info: &WindowInfo,
    verbose: bool,
) -> Option<crate::core::rules::HostContext> {
    if !host_capable() {
        return None; // legacy/offline -> string-only (today's behavior)
    }
    let path = crate::core::rules::get_rules_paths()
        .into_iter()
        .find(|p| p.exists())?; // no rules.toml -> None -> string-only
    let rules = match crate::core::cached_rules_at(&path) {
        Ok(r) => {
            // A good parse re-arms the malformed-file notification (so a later
            // breakage notifies again). HOST_RULES.md §7.
            RULES_INVALID_NOTIFIED.store(false, Ordering::SeqCst);
            r
        }
        Err(e) => {
            // §7: never silent on a bad file — fire ONE desktop notification per
            // broken state (swap returns the prior flag; notify only on the
            // false→true transition), then fall back to string-only.
            if !RULES_INVALID_NOTIFIED.swap(true, Ordering::SeqCst) {
                crate::platforms::notify("QMKonnect: rules.toml invalid", &format!("{e}"));
            }
            if verbose {
                eprintln!(
                    "Warning: could not parse {}: {} — host rules disabled for this window",
                    path.display(),
                    e
                );
            }
            return None; // malformed -> graceful string-only fallback
        }
    };
    let names = callback_names();
    Some(crate::core::rules::evaluate(
        &rules,
        &window_info.app_class,
        &window_info.title,
        &names,
        board_has_rules(),
    ))
}

/// The end-to-end per-window send (HOST_RULES.md §8(4)).
///
/// Branches on the optional host [`crate::core::rules::HostContext`]:
/// - `None` (host rules disabled) → legacy string only.
/// - non-replace (`!clear_board` — covers stack **and** no-match) → string
///   first, then context. The board silo always runs (C13); for a no-match the
///   context just clears the host layer/callbacks (clear_board=false ⇒ the
///   board is untouched).
/// - replace (`clear_board`: matched, every rule disabling, or no board rules)
///   → context only (no string).
///
/// Returns the legacy-string send `Result` so each call site keeps its own
/// error-propagation policy (the worker swallows; `notify_qmk` propagates via
/// `?`). The host-context send swallows its own errors (§5.4 retry parity) so
/// it never changes the string-result propagation.
///
/// # Example
///
/// ```rust,ignore
/// let notifier = get_notifier();
/// let notifier = notifier.lock().unwrap();
/// let _res = dispatch_window_send(&**notifier, &filter, &message, ctx, "debounced", verbose);
/// ```
fn dispatch_window_send(
    notifier: &dyn Notifier,
    filter: &DeviceFilter,
    message: &str,
    ctx: Option<crate::core::rules::HostContext>,
    label: &str,
    verbose: bool,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match ctx {
        // Host rules disabled (not capable / no rules.toml / malformed): legacy string only.
        None => send_legacy_string(notifier, message, label, verbose),

        // Non-replace (stack OR no-match): the board silo always runs (C13) —
        // send the string first, then the host context. For a stack match the
        // context applies the host layer on top; for a no-match it clears the
        // host layer/callbacks only (clear_board=false ⇒ board untouched).
        Some(ctx) if !ctx.clear_board => {
            let r = send_legacy_string(notifier, message, label, verbose);
            send_host_context(notifier, filter, host_context_command(&ctx), verbose);
            r
        }

        // Replace (matched, every rule disabling, or board has no rules):
        // context only — no string, so the board can't match and the firmware
        // clears its own board layer/cmd via clear_board=1.
        Some(ctx) => {
            send_host_context(notifier, filter, host_context_command(&ctx), verbose);
            Ok(())
        }
    }
}

/// Send the legacy `SendMessage` string with the exact pre-host-rules verbose
/// log + timing (the `label` is "debounced" or "immediate"). The bytes passed
/// to [`Notifier::notify`] are identical to the pre-host-rules code path —
/// `notify` takes an owned `String`, so `.to_string()` does not change bytes.
fn send_legacy_string(
    notifier: &dyn Notifier,
    message: &str,
    label: &str,
    verbose: bool,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if verbose {
        let sanitized = message.replace('\x1D', "|");
        println!(
            "[{}ms] Notified QMK ({}): {}",
            crate::core::now_ms(),
            label,
            sanitized
        );
    }
    #[cfg(test)]
    println!("Sending {} notification: {}", label, message);

    let _len = message.len();
    let _t0 = Instant::now();
    let res = notifier.notify(message.to_string());
    let _send_ms = _t0.elapsed().as_millis();
    if verbose {
        eprintln!(
            "[{}ms] send took {}ms ({} bytes)",
            crate::core::now_ms(),
            _send_ms,
            _len
        );
    }
    res
}

/// Send a typed host-context [`qmk_notifier::RunCommand`] with `SendMessage`-style retry/cache
/// parity (PRD §5.4): up to 3 attempts for device errors, then swallowed.
/// Host-context failures never fail the overall window send (the legacy string,
/// if any, already went out).
fn send_host_context(
    notifier: &dyn Notifier,
    filter: &DeviceFilter,
    command: qmk_notifier::RunCommand,
    verbose: bool,
) {
    for attempt in 1..=3 {
        match notifier.send_command(command.clone(), filter) {
            Ok(_) => {
                if verbose {
                    eprintln!(
                        "[{}ms] sent host context (attempt {}): {:?}",
                        crate::core::now_ms(),
                        attempt,
                        command
                    );
                }
                return;
            }
            Err(e) => {
                let s = e.to_string().to_lowercase();
                if s.contains("no device found")
                    || s.contains("permission denied")
                    || s.contains("failed to open")
                {
                    if attempt < 3 {
                        thread::sleep(Duration::from_millis(100 * attempt as u64));
                        continue;
                    }
                    eprintln!(
                        "QMK device unavailable after {} attempts sending host context: {}",
                        attempt, e
                    );
                    return; // swallowed (parity with notify's device-error swallow)
                }
                eprintln!("Error sending host context: {}", e);
                return; // non-device error: log + swallow (don't fail the window send)
            }
        }
    }
}

/// Build the `ApplyHostContext` command for a matched [`crate::core::rules::HostContext`] (stack or
/// replace). `clear_board` carries the stack-vs-replace decision.
fn host_context_command(ctx: &crate::core::rules::HostContext) -> qmk_notifier::RunCommand {
    // R-COEX: ApplyHostContext → 0x81 0x9F magic header (reaches the wire via send_command).
    qmk_notifier::RunCommand::ApplyHostContext {
        layer: ctx.layer,
        callbacks: ctx.callback_ids.clone(),
        clear_board: ctx.clear_board,
    }
}

#[cfg(test)]
mod r_coex_invariants {
    use qmk_notifier::{HostOs, RunCommand};

    fn emits_0x81_first_byte(cmd: &RunCommand) -> bool {
        // ListDevices is the sole wire-silent variant (crate `run` enumerates
        // HID and returns Timeout without touching the device). Every other
        // variant flows through burst_to_one, which sets request_data[1]=0x81
        // (the 0x81 0x9F magic header) on every 33-byte report.
        !matches!(cmd, RunCommand::ListDevices)
    }

    #[test]
    fn r_coex_every_transport_variant_emits_magic_header() {
        // The variants QmkNotifier::notify / send_command / host_context_command build.
        let transport_variants: [RunCommand; 5] = [
            RunCommand::SendMessage("x".into()),
            RunCommand::QueryInfo,
            RunCommand::QueryCallback(0),
            RunCommand::SetOs(HostOs::Linux),
            RunCommand::ApplyHostContext {
                layer: Some(224),
                callbacks: vec![],
                clear_board: false,
            },
        ];
        for v in &transport_variants {
            assert!(
                emits_0x81_first_byte(v),
                "R-COEX violation: {:?} must emit 0x81 as its first on-wire byte",
                v
            );
        }
    }

    #[test]
    fn r_coex_list_devices_is_the_lone_wire_silent_variant() {
        // Sanity: confirms the predicate discriminates. ListDevices enumerates
        // HID and sends nothing — it is NOT on the app's transport path.
        assert!(!emits_0x81_first_byte(&RunCommand::ListDevices));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::WindowInfo;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex as StdMutex;

    // Use a shared global mock for testing
    static MOCK_CALL_COUNT: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));
    static MOCK_LAST_MESSAGE: Lazy<StdMutex<Option<String>>> = Lazy::new(|| StdMutex::new(None));
    static MOCK_SEND_COMMAND_CALLS: Lazy<StdMutex<Vec<qmk_notifier::RunCommand>>> =
        Lazy::new(|| StdMutex::new(Vec::new()));
    static MOCK_RESPONSES: Lazy<StdMutex<VecDeque<qmk_notifier::CommandResponse>>> =
        Lazy::new(|| StdMutex::new(VecDeque::new()));
    // LOW-1: error injection for testing the transient-failure retry path. When
    // non-empty, `send_command` drains one entry per call and returns `Err`
    // (instead of consulting MOCK_RESPONSES).
    static MOCK_SEND_COMMAND_ERRORS: Lazy<StdMutex<VecDeque<String>>> =
        Lazy::new(|| StdMutex::new(VecDeque::new()));
    // P1.M3.T2.S1 (#4): optional per-`send_command` artificial delay so the callback sweep
    // has a measurable, controllable duration. Used by
    // test_handshake_sweep_releases_lock_between_iterations to prove NOTIFIER is released
    // between sweep iterations. `None` in production and by default.
    static MOCK_SEND_DELAY: Lazy<StdMutex<Option<Duration>>> = Lazy::new(|| StdMutex::new(None));

    fn reset_global_mock() {
        MOCK_CALL_COUNT.store(0, Ordering::SeqCst);
        *MOCK_LAST_MESSAGE.lock().unwrap() = None;
        MOCK_SEND_COMMAND_CALLS.lock().unwrap().clear();
        MOCK_RESPONSES.lock().unwrap().clear();
        MOCK_SEND_COMMAND_ERRORS.lock().unwrap().clear();
        *MOCK_SEND_DELAY.lock().unwrap() = None;
    }

    struct MockNotifier;

    impl MockNotifier {
        fn new() -> Self {
            reset_global_mock();
            Self
        }

        fn get_call_count() -> usize {
            MOCK_CALL_COUNT.load(Ordering::SeqCst)
        }

        fn get_last_message() -> Option<String> {
            MOCK_LAST_MESSAGE.lock().unwrap().clone()
        }

        fn get_send_command_calls() -> Vec<qmk_notifier::RunCommand> {
            MOCK_SEND_COMMAND_CALLS.lock().unwrap().clone()
        }

        fn set_mock_responses(responses: Vec<qmk_notifier::CommandResponse>) {
            MOCK_RESPONSES.lock().unwrap().extend(responses);
        }

        fn set_mock_send_errors(errors: Vec<String>) {
            MOCK_SEND_COMMAND_ERRORS.lock().unwrap().extend(errors);
        }

        /// P1.M3.T2.S1 (#4): inject a per-`send_command` wall-clock delay so the sweep is wide
        /// enough for a contending thread to acquire NOTIFIER between iterations. Pass `None`
        /// to disable (the default; production code never sets this).
        fn set_send_delay(delay: Option<Duration>) {
            *MOCK_SEND_DELAY.lock().unwrap() = delay;
        }
    }

    impl Notifier for MockNotifier {
        fn notify(&self, message: String) -> Result<(), Box<dyn Error + Send + Sync>> {
            #[cfg(test)]
            println!("MockNotifier.notify called with: {}", message);
            MOCK_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
            *MOCK_LAST_MESSAGE.lock().unwrap() = Some(message);
            Ok(())
        }

        fn send_command(
            &self,
            command: qmk_notifier::RunCommand,
            _filter: &DeviceFilter,
        ) -> Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>> {
            MOCK_SEND_COMMAND_CALLS
                .lock()
                .unwrap()
                .push(command.clone());
            // P1.M3.T2.S1 (#4): optional artificial delay to widen the sweep window for the
            // per-iteration lock-release test (wall-clock sleep, so CI CPU slowdown can't shrink it).
            if let Some(d) = *MOCK_SEND_DELAY.lock().unwrap() {
                thread::sleep(d);
            }
            // LOW-1: if an error is queued, return it (drains one per call).
            if let Some(msg) = MOCK_SEND_COMMAND_ERRORS.lock().unwrap().pop_front() {
                return Err(msg.into());
            }
            let resp = MOCK_RESPONSES.lock().unwrap().pop_front();
            Ok(resp.unwrap_or(qmk_notifier::CommandResponse::Ack { ok: true }))
        }
    }

    /// Reset the shared debouncer so the next `notify_qmk` is treated as the
    /// first message (sent immediately), and drain any in-flight work.
    fn reset_test_state() {
        // Let any worker flush in progress finish first.
        thread::sleep(Duration::from_millis(150));

        {
            let mut state = STATE.lock().unwrap();
            state.last_sent_time = None;
            state.pending = None;
            state.verbose = false;
            state.interval_override = Some(Duration::from_millis(50));
            // Wake the worker so it re-evaluates (pending is now None -> it waits).
            COND.notify_all();
        }

        reset_global_mock();

        // Give the woken worker a moment to settle back into its wait.
        thread::sleep(Duration::from_millis(50));
    }

    fn wait_for_count(target: usize, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if MockNotifier::get_call_count() >= target {
                return true;
            }
            thread::sleep(Duration::from_millis(5));
        }
        false
    }

    #[test]
    fn test_immediate_send_first_message() {
        reset_test_state();
        set_notifier(Box::new(MockNotifier::new()));

        let window_info = WindowInfo::new("TestApp".to_string(), "Test Title".to_string());
        let result = notify_qmk(&window_info, true);
        assert!(result.is_ok());

        assert!(wait_for_count(1, Duration::from_millis(500)));
        assert_eq!(MockNotifier::get_call_count(), 1);
        assert_eq!(
            MockNotifier::get_last_message(),
            Some(format!(
                "{}\x1D{}",
                window_info.app_class, window_info.title
            ))
        );
    }

    #[test]
    fn test_debounce_subsequent_messages() {
        reset_test_state();
        set_notifier(Box::new(MockNotifier::new()));

        // reset_test_state() uses a 50ms debounce window, but this test sleeps
        // only 20ms to assert the 2nd message is still pending. On a loaded CI
        // runner a 20ms sleep can overshoot the 50ms window and the worker
        // flushes early (count jumps to 2). Widen this test's window so that
        // can't happen; production interval is unaffected.
        STATE.lock().unwrap().interval_override = Some(Duration::from_millis(200));

        // First message - sent immediately
        let window1 = WindowInfo::new("App1".to_string(), "Title1".to_string());
        let _ = notify_qmk(&window1, true);
        assert!(wait_for_count(1, Duration::from_millis(500)));
        assert_eq!(MockNotifier::get_call_count(), 1);

        // Second message within the debounce window - should be queued, NOT sent yet.
        let window2 = WindowInfo::new("App2".to_string(), "Title2".to_string());
        let _ = notify_qmk(&window2, true);

        // Confirm it is still pending (no premature flush).
        thread::sleep(Duration::from_millis(20));
        assert_eq!(MockNotifier::get_call_count(), 1);

        // After the window elapses the worker flushes exactly one follow-up.
        assert!(wait_for_count(2, Duration::from_millis(500)));
        assert_eq!(MockNotifier::get_call_count(), 2);
        assert_eq!(
            MockNotifier::get_last_message(),
            Some(format!("{}\x1D{}", window2.app_class, window2.title))
        );
    }

    /// B3 regression: `debounce_ms` must be hot-config — the window is re-read
    /// from `configured_debounce_ms()` on every send, not cached at startup.
    /// Here `interval_override` stands in for the live config value: after a
    /// short-window burst, widening it mid-flight must extend the coalescing
    /// window (the queued message does NOT flush under the old, short window).
    #[test]
    fn test_debounce_ms_is_hot_config() {
        reset_test_state();
        set_notifier(Box::new(MockNotifier::new()));

        // Start with a short window, send the first message immediately.
        STATE.lock().unwrap().interval_override = Some(Duration::from_millis(200));
        let window1 = WindowInfo::new("App1".to_string(), "Title1".to_string());
        let _ = notify_qmk(&window1, true);
        assert!(wait_for_count(1, Duration::from_millis(500)));
        assert_eq!(MockNotifier::get_call_count(), 1);

        // Second message within the (current) 200ms window — queued.
        let window2 = WindowInfo::new("App2".to_string(), "Title2".to_string());
        let _ = notify_qmk(&window2, true);
        thread::sleep(Duration::from_millis(20));
        assert_eq!(MockNotifier::get_call_count(), 1); // still pending

        // Edit "config" (the override) to a much wider window mid-flight. If the
        // interval were cached at startup, the queued message would flush ~200ms
        // after the first send. Because it is hot, the wider window takes over
        // and the message stays pending past the old 200ms deadline.
        STATE
            .lock()
            .unwrap()
            .interval_override
            .replace(Duration::from_secs(30));
        COND.notify_all(); // wake the worker so it re-reads the new window

        // Well past the old 200ms window — must NOT have flushed.
        thread::sleep(Duration::from_millis(400));
        assert_eq!(
            MockNotifier::get_call_count(),
            1,
            "queued message must respect the hot-configured (widened) window"
        );
    }

    #[test]
    fn test_send_after_debounce_timeout() {
        reset_test_state();
        set_notifier(Box::new(MockNotifier::new()));

        // First message - sent immediately
        let window1 = WindowInfo::new("App1".to_string(), "Title1".to_string());
        let _ = notify_qmk(&window1, true);
        assert!(wait_for_count(1, Duration::from_millis(500)));

        // Second message within the debounce period - queued
        let window2 = WindowInfo::new("App2".to_string(), "Title2".to_string());
        let _ = notify_qmk(&window2, true);

        // Wait for the debounce timer to fully complete and send
        assert!(wait_for_count(2, Duration::from_millis(500)));
        assert_eq!(MockNotifier::get_call_count(), 2);
        assert_eq!(
            MockNotifier::get_last_message(),
            Some(format!("{}\x1D{}", window2.app_class, window2.title))
        );
    }

    #[test]
    fn test_multiple_rapid_updates() {
        reset_test_state();
        set_notifier(Box::new(MockNotifier::new()));

        // reset_test_state() uses a 50ms debounce window, which a slow/loaded CI
        // runner can overrun mid-burst (the entire burst must land *inside* the
        // window for it to collapse to a single send). Widen it here so the rapid
        // updates reliably coalesce regardless of scheduler jitter.
        STATE.lock().unwrap().interval_override = Some(Duration::from_millis(200));

        // First message - sent immediately
        let _ = notify_qmk(
            &WindowInfo::new("App1".to_string(), "Title1".to_string()),
            true,
        );
        assert!(wait_for_count(1, Duration::from_millis(500)));

        // Several rapid updates, each well inside the debounce window so they all
        // collapse into a single follow-up send of the newest value.
        for i in 2..=5 {
            let _ = notify_qmk(
                &WindowInfo::new(format!("App{}", i), format!("Title{}", i)),
                true,
            );
            thread::sleep(Duration::from_millis(5));
        }

        // Only the last value should be flushed.
        assert!(wait_for_count(2, Duration::from_millis(500)));
        assert_eq!(MockNotifier::get_call_count(), 2);
        assert_eq!(
            MockNotifier::get_last_message(),
            Some("App5\x1DTitle5".to_string())
        );
    }

    #[test]
    fn test_verbose_mode() {
        reset_test_state();
        set_notifier(Box::new(MockNotifier::new()));

        let window_info = WindowInfo::new("VerboseApp".to_string(), "Test Verbose".to_string());
        let result = notify_qmk(&window_info, true);
        assert!(result.is_ok());

        assert!(wait_for_count(1, Duration::from_millis(500)));
        assert_eq!(MockNotifier::get_call_count(), 1);
    }

    #[test]
    fn test_threads_dont_interfere() {
        reset_test_state();
        set_notifier(Box::new(MockNotifier::new()));

        // Start several threads that all send notifications near-simultaneously.
        let mut handles = vec![];
        for i in 1..=5 {
            let window_info =
                WindowInfo::new(format!("ThreadApp{}", i), format!("Thread {} Title", i));
            let handle = thread::spawn(move || {
                let _ = notify_qmk(&window_info, false);
            });
            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.join();
        }

        // Wait for debouncing to settle. The burst should collapse to 1-2 sends.
        thread::sleep(Duration::from_millis(400));
        let count = MockNotifier::get_call_count();
        println!("Final call count after threaded test: {}", count);
        assert!(count >= 1);
    }

    #[test]
    fn test_debounced_pending_carries_window_info() {
        reset_test_state();
        set_notifier(Box::new(MockNotifier::new()));
        // Long window: the worker will NOT flush while we inspect `pending`.
        STATE.lock().unwrap().interval_override = Some(Duration::from_secs(10));

        // Prime last_sent_time with an immediate send (so the next call debounces).
        let _ = notify_qmk(&WindowInfo::new("App1".into(), "Title1".into()), false);
        assert!(wait_for_count(1, Duration::from_millis(500)));

        // Second call inside the window -> queued as PendingMessage.
        let w2 = WindowInfo::new("App2".into(), "Title2".into());
        let _ = notify_qmk(&w2, false);

        // White-box: pending now carries BOTH the formatted payload AND the WindowInfo.
        let snap = {
            let st = STATE.lock().unwrap();
            st.pending
                .as_ref()
                .map(|p| (p.payload.clone(), p.window_info.clone()))
        };
        let (payload, wi) = snap.expect("pending should hold the queued message");
        assert_eq!(payload, "App2\x1DTitle2");
        assert_eq!(wi, w2);

        // Cleanup: shrink the interval back to the reset default so the worker
        // flushes the queued w2 quickly (pending -> None via the normal take()
        // path) BEFORE the next test's reset_test_state() runs. Leaving a 10s
        // interval + Some(..) pending would race reset's `pending = None`: the
        // worker's wait_timeout would later reach `take().unwrap()` on an empty
        // pending and poison the shared mutex.
        STATE.lock().unwrap().interval_override = Some(Duration::from_millis(50));
        assert!(wait_for_count(2, Duration::from_millis(500)));
    }

    #[test]
    fn test_send_command_records_call_sequence() {
        reset_test_state();
        set_notifier(Box::new(MockNotifier::new()));

        let f = DeviceFilter {
            vendor_id: Some(0x1234),
            product_id: Some(0x5678),
            usage_page: 0xFF60,
            usage: 0x61,
        };

        {
            let notifier = get_notifier();
            let n = notifier.lock().unwrap();
            let _ = n.send_command(qmk_notifier::RunCommand::QueryInfo, &f);
            let _ = n.send_command(
                qmk_notifier::RunCommand::ApplyHostContext {
                    layer: Some(224),
                    callbacks: vec![0, 1],
                    clear_board: false,
                },
                &f,
            );
        }

        let calls = MockNotifier::get_send_command_calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], qmk_notifier::RunCommand::QueryInfo);
        assert!(matches!(
            calls[1],
            qmk_notifier::RunCommand::ApplyHostContext {
                layer: Some(224),
                ..
            }
        ));
    }

    #[test]
    fn test_send_command_reset_clears_log() {
        reset_test_state();
        set_notifier(Box::new(MockNotifier::new()));

        let f = DeviceFilter {
            vendor_id: None,
            product_id: None,
            usage_page: 0xFF60,
            usage: 0x61,
        };

        {
            let notifier = get_notifier();
            let n = notifier.lock().unwrap();
            let _ = n.send_command(qmk_notifier::RunCommand::QueryInfo, &f);
        }
        assert!(!MockNotifier::get_send_command_calls().is_empty());

        reset_test_state(); // G7: must clear the log via reset_global_mock
        assert!(MockNotifier::get_send_command_calls().is_empty());
    }

    #[test]
    fn test_send_command_returns_ok_ack_default() {
        reset_test_state();
        set_notifier(Box::new(MockNotifier::new()));

        let f = DeviceFilter {
            vendor_id: None,
            product_id: None,
            usage_page: 0xFF60,
            usage: 0x61,
        };

        let notifier = get_notifier();
        let n = notifier.lock().unwrap();
        let resp = n.send_command(
            qmk_notifier::RunCommand::SetOs(qmk_notifier::HostOs::Linux),
            &f,
        );
        assert!(matches!(
            resp,
            Ok(qmk_notifier::CommandResponse::Ack { ok: true })
        ));
    }

    #[test]
    fn test_qmk_notifier_send_command_maps_device_not_found() {
        reset_test_state(); // stabilize state; we use QmkNotifier directly

        let qmk = QmkNotifier;
        let f = DeviceFilter {
            vendor_id: Some(0xFFFF),
            product_id: Some(0xFFFF),
            usage_page: 0xFF60,
            usage: 0x61,
        };

        let res = qmk.send_command(qmk_notifier::RunCommand::QueryInfo, &f);
        assert!(res.is_err());
        let msg = res.unwrap_err().to_string().to_lowercase();
        assert!(
            msg.contains("no device found"),
            "expected DeviceNotFound, got: {msg}"
        );
    }

    #[test]
    fn test_send_command_notify_recorders_independent() {
        reset_test_state();
        set_notifier(Box::new(MockNotifier::new()));

        let f = DeviceFilter {
            vendor_id: None,
            product_id: None,
            usage_page: 0xFF60,
            usage: 0x61,
        };

        {
            let notifier = get_notifier();
            let n = notifier.lock().unwrap();
            let _ = n.notify("App\x1DTitle".to_string());
            let _ = n.send_command(qmk_notifier::RunCommand::QueryInfo, &f);
        }

        assert_eq!(MockNotifier::get_call_count(), 1);
        assert_eq!(
            MockNotifier::get_last_message(),
            Some("App\x1DTitle".to_string())
        );
        assert_eq!(MockNotifier::get_send_command_calls().len(), 1);
    }

    // ========================================================================
    // config.toml parse-error diagnostic (PRD §2.1 Goal 4)
    // ========================================================================

    #[test]
    fn test_config_parse_error_at_reports_malformed_toml() {
        // A malformed config.toml must surface a parse error so
        // startup_device_probe can report it instead of silently probing against
        // defaulted values. Writes a broken file to a TempDir (hermetic — does
        // not touch the user's real config) and checks the path + message.
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        // Duplicate key: valid TOML grammar but a serde error on the second
        // `vendor_id` — the kind of typo a user makes (e.g. editing by hand).
        std::fs::write(&path, "vendor_id = 0x1234\nvendor_id = 0x5678\n").unwrap();

        let (err_path, msg) =
            config_parse_error_at(&path).expect("a malformed config must surface a parse error");
        assert_eq!(err_path, path);
        assert!(!msg.is_empty(), "error message must be non-empty");
    }

    #[test]
    fn test_config_parse_error_at_none_for_valid_config() {
        // A well-formed config parses cleanly -> None (no error to report).
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "vendor_id = 0xfeed\ndebounce_ms = 100\n").unwrap();

        assert!(config_parse_error_at(&path).is_none());
    }

    #[test]
    fn test_config_parse_error_at_reports_wrong_type() {
        // A value of the wrong type (string where a u64 is expected) is a serde
        // error, not a grammar error — must still be surfaced.
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, b"debounce_ms = \"fifty\"\n").unwrap();

        let (err_path, _msg) =
            config_parse_error_at(&path).expect("a wrong-type value must surface a parse error");
        assert_eq!(err_path, path);
    }

    // ========================================================================
    // Host-rules capability handshake (P4.M2.T1.S1)
    // ========================================================================
    // Each handshake test starts from a clean slate: reset_test_state() drains
    // the debouncer + mock, reset_handshake_state() clears HOST_CAPABLE /
    // CALLBACK_NAMES / HAS_HANDSHAKED, set_notifier installs a fresh Mock, and
    // set_mock_responses scripts the reply sequence. Single-threaded (G6).

    #[test]
    fn test_handshake_capable_populates_state() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        MockNotifier::set_mock_responses(vec![
            qmk_notifier::CommandResponse::Info {
                proto_ver: 2,
                feature_flags: 0x01,
                callback_count: 2,
                board_rules_present: true,
            },
            qmk_notifier::CommandResponse::Ack { ok: true }, // SetOs
            qmk_notifier::CommandResponse::CallbackName {
                index: 0,
                name: Some("vim_lazy".into()),
            },
            qmk_notifier::CommandResponse::CallbackName {
                index: 1,
                name: Some("disable_vim".into()),
            },
        ]);
        perform_handshake(false);
        assert!(host_capable());
        let names = callback_names();
        assert_eq!(names.get("vim_lazy"), Some(&0));
        assert_eq!(names.get("disable_vim"), Some(&1));
        let calls = MockNotifier::get_send_command_calls();
        assert_eq!(calls.len(), 4);
        assert_eq!(calls[0], qmk_notifier::RunCommand::QueryInfo);
        assert!(matches!(calls[1], qmk_notifier::RunCommand::SetOs(_)));
        assert_eq!(calls[2], qmk_notifier::RunCommand::QueryCallback(0));
        assert_eq!(calls[3], qmk_notifier::RunCommand::QueryCallback(1));
    }

    /// #4 / P1.M3.T2.S1: the callback sweep must release NOTIFIER between iterations so a
    /// window-notification send (`notify_qmk` immediate / `debounce_worker` flush — both BLOCKING
    /// on NOTIFIER) is not starved for the whole (up to ~5 s) sweep. The per-iteration release is
    /// landed; the sweep also `yield_now()`s after each drop so a woken blocking waiter is more
    /// likely to run before the sweep re-locks (std::sync::Mutex unfair barging would otherwise
    /// re-acquire in ~ns).
    ///
    /// This test DETERMINISTICALLY proves per-iteration release + re-locking with a three-step
    /// shape that does NOT depend on winning a `std::sync::Mutex` barging race (which is
    /// probabilistic on multicore and cannot be made deterministic with `yield_now()` alone):
    ///   1. ACQUIRE mid-sweep via a non-yielding `try_lock` spinner. The spinner continuously
    ///      polls, so it reliably catches the inter-iteration release window (the lock is free
    ///      for the loop overhead between two iterations). Under a full-sweep hold the spinner
    ///      could NEVER acquire mid-sweep, so step 2 would never run and the count-assert fires.
    ///      (The spinner is a DETECTION mechanism for the release window — it is not modeling the
    ///      production blocking path; that path is helped by the production `yield_now()`.)
    ///   2. FREEZE-CHECK: once acquired, HOLD NOTIFIER past one iteration's delay and assert the
    ///      `send_command` call count did NOT advance — the worker is blocked on its next
    ///      iteration's re-lock. This proves per-iteration re-locking (not a one-shot release).
    ///   3. RELEASE + re-acquire mid-sweep + FREEZE-CHECK again, proving the worker re-locks
    ///      EVERY iteration, then let it finish and verify the atomic post-sweep CALLBACK_NAMES.
    #[test]
    fn test_handshake_sweep_releases_lock_between_iterations() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));

        let n_callbacks: u8 = 10;
        // Per-call delay: each sweep iteration holds NOTIFIER for ~150ms (the sleep is inside
        // `send_command`, under the per-iteration lock) and frees it for the loop overhead
        // between iterations. The non-yielding spinner catches that free window deterministically.
        let per_call_delay = Duration::from_millis(150);
        MockNotifier::set_send_delay(Some(per_call_delay));

        let mut responses = vec![qmk_notifier::CommandResponse::Info {
            proto_ver: 2,
            feature_flags: 0x01,
            callback_count: n_callbacks,
            board_rules_present: false,
        }];
        // SetOs consumes one response (its reply is ignored, but the mock still pops FIFO — see
        // test_handshake_capable_populates_state). Queue an Ack for it.
        responses.push(qmk_notifier::CommandResponse::Ack { ok: true });
        for i in 0..n_callbacks {
            responses.push(qmk_notifier::CommandResponse::CallbackName {
                index: i,
                name: Some(format!("cb_{}", i)),
            });
        }
        MockNotifier::set_mock_responses(responses);

        let notifier = get_notifier();

        let h = thread::spawn(move || {
            perform_handshake(false);
        });

        // Wait until the worker has finished the PRE-SWEEP phase (QueryInfo + SetOs) and entered
        // the sweep. From here on NOTIFIER is acquired/released PER ITERATION.
        let pre_sweep_deadline = Instant::now() + Duration::from_millis(2000);
        loop {
            if MockNotifier::get_send_command_calls().len() >= 2 {
                break;
            }
            if Instant::now() >= pre_sweep_deadline {
                panic!("handshake never finished pre-sweep (QueryInfo+SetOs): call count < 2");
            }
            thread::sleep(Duration::from_millis(5));
        }

        // Helper: acquire NOTIFIER mid-sweep via a non-yielding `try_lock` spinner. Returns the
        // guard and the call count at the moment of acquisition. Under a full-sweep hold this
        // would spin until the whole sweep finished, so `calls_at_acquire` would hit 2+N and the
        // caller's count-assert fires — making the bug (Finding #4) visible deterministically.
        // Timeout is a safety net (the spinner normally catches a release window within ~1
        // iteration); it must exceed the full sweep time so a genuine release is never missed.
        let acquire_mid_sweep = |label: &str| -> std::sync::MutexGuard<'_, Box<dyn Notifier>> {
            let spin_deadline = Instant::now() + Duration::from_millis(5000);
            loop {
                if let Ok(g) = notifier.try_lock() {
                    let _ = label; // label is for readability at call sites
                    return g;
                }
                if Instant::now() >= spin_deadline {
                    panic!(
                        "{}: never acquired NOTIFIER mid-sweep via try_lock within 5s \
                         (call count = {}) — the sweep is holding NOTIFIER across the whole sweep \
                         (Finding #4 not fixed)",
                        label,
                        MockNotifier::get_send_command_calls().len()
                    );
                }
                // Intentionally NO yield: a yielding spinner misses the ~ns release window under
                // load. The continuous poll is what makes this deterministic.
            }
        };

        // === HOLD #1: acquire mid-sweep, then FREEZE-CHECK. ===
        let guard1 = acquire_mid_sweep("hold #1");
        let calls_at_hold1 = MockNotifier::get_send_command_calls().len();
        // We grabbed NOTIFIER mid-sweep, not after the full sweep. (Under a full-sweep hold the
        // spinner only ever acquires at 2+N, failing this assert.)
        assert!(
            calls_at_hold1 < 2 + n_callbacks as usize,
            "contender acquired NOTIFIER only after {} send_command calls (>= the full sweep \
             2+{}); the sweep did NOT release NOTIFIER between iterations.",
            calls_at_hold1,
            n_callbacks
        );
        // Sleep well past one iteration's delay so a worker that failed to re-lock per iteration
        // would have sent several more QueryCallbacks.
        thread::sleep(per_call_delay + Duration::from_millis(150));
        let calls_at_freeze1 = MockNotifier::get_send_command_calls().len();
        assert_eq!(
            calls_at_freeze1, calls_at_hold1,
            "send_command call count advanced from {} to {} while the test held NOTIFIER — the \
             handshake was NOT blocked on the per-iteration re-lock (Finding #4: the sweep may be \
             holding NOTIFIER across iterations).",
            calls_at_hold1, calls_at_freeze1
        );

        // === RELEASE: the worker must now proceed — proving the release lets a notification ===
        // waiter interleave between iterations.
        drop(guard1);

        // Wait until the worker advances several iterations (proves release worked AND the sweep
        // re-locks per iteration, not just once).
        let advance_deadline = Instant::now() + Duration::from_millis(3000);
        loop {
            if MockNotifier::get_send_command_calls().len() >= calls_at_hold1 + 3 {
                break;
            }
            if Instant::now() >= advance_deadline {
                panic!(
                    "handshake did not advance into the sweep after NOTIFIER was released (call \
                     count stayed at {}, expected >= {})",
                    MockNotifier::get_send_command_calls().len(),
                    calls_at_hold1 + 3
                );
            }
            thread::sleep(Duration::from_millis(5));
        }
        let calls_before_rehold = MockNotifier::get_send_command_calls().len();
        // Sanity: it has NOT finished the whole sweep yet (we're still mid-flight).
        assert!(
            calls_before_rehold < 2 + n_callbacks as usize,
            "handshake finished the whole sweep ({} calls) before we could re-acquire — the \
             per-call delay may be too short for this box",
            calls_before_rehold
        );

        // === HOLD #2: acquire mid-sweep AGAIN + FREEZE-CHECK. This confirms the worker ===
        // re-locks EVERY iteration, not just once after the first release.
        let guard2 = acquire_mid_sweep("hold #2");
        thread::sleep(per_call_delay + Duration::from_millis(150));
        let calls_at_freeze2 = MockNotifier::get_send_command_calls().len();
        assert_eq!(
            calls_at_freeze2, calls_before_rehold,
            "send_command call count advanced from {} to {} while the test held NOTIFIER mid-sweep \
             — the handshake was NOT blocked on the per-iteration re-lock (regression: the sweep \
             is holding NOTIFIER across iterations again).",
            calls_before_rehold, calls_at_freeze2
        );
        drop(guard2);

        h.join().unwrap();

        // CALLBACK_NAMES is published atomically AFTER the sweep: all N callbacks present once
        // perform_handshake returns, and the board is host-capable.
        assert!(host_capable());
        let names = callback_names();
        assert_eq!(
            names.len(),
            n_callbacks as usize,
            "all {} callbacks must be mapped after the sweep completes",
            n_callbacks
        );
        for i in 0..n_callbacks {
            let key = format!("cb_{}", i);
            assert_eq!(
                names.get(&key),
                Some(&i),
                "callback {} missing/wrong in CALLBACK_NAMES",
                key
            );
        }

        // Clean up the delay so it can't bleed into later single-threaded tests.
        MockNotifier::set_send_delay(None);
    }

    #[test]
    fn test_handshake_legacy_proto_v1_string_only() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        MockNotifier::set_mock_responses(vec![qmk_notifier::CommandResponse::Info {
            proto_ver: 1,
            feature_flags: 0x00,
            callback_count: 0,
            board_rules_present: true,
        }]);
        perform_handshake(false);
        assert!(!host_capable());
        assert!(callback_names().is_empty());
        let calls = MockNotifier::get_send_command_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], qmk_notifier::RunCommand::QueryInfo);
    }

    #[test]
    fn test_handshake_no_feature_flag_string_only() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        MockNotifier::set_mock_responses(vec![qmk_notifier::CommandResponse::Info {
            proto_ver: 2,
            feature_flags: 0x00,
            callback_count: 3,
            board_rules_present: true,
        }]);
        perform_handshake(false);
        assert!(!host_capable());
        assert_eq!(MockNotifier::get_send_command_calls().len(), 1);
    }

    #[test]
    fn test_handshake_timeout_string_only() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        MockNotifier::set_mock_responses(vec![qmk_notifier::CommandResponse::Timeout]);
        perform_handshake(false);
        assert!(!host_capable());
        assert_eq!(MockNotifier::get_send_command_calls().len(), 1);
    }

    /// LOW-1: a `Timeout` is a transient failure (the firmware never confirmed
    /// receipt of QUERY_INFO), so the dedup token must be released — the next
    /// poll/reconnect retries the handshake. Without this, a one-time flaky
    /// QUERY_INFO against a capable board would disable host rules until a
    /// physical replug.
    #[test]
    fn test_handshake_timeout_releases_dedup_token() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        // First attempt: timeout (transient).
        MockNotifier::set_mock_responses(vec![qmk_notifier::CommandResponse::Timeout]);
        perform_handshake(false);
        assert!(!host_capable());
        assert_eq!(MockNotifier::get_send_command_calls().len(), 1);

        // The token was released, so a second perform_handshake (e.g. the next
        // poll tick after the transient cleared) re-sends QUERY_INFO and this
        // time the board answers capable.
        MockNotifier::set_mock_responses(vec![
            qmk_notifier::CommandResponse::Info {
                proto_ver: 2,
                feature_flags: 0x01,
                callback_count: 0,
                board_rules_present: false,
            },
            qmk_notifier::CommandResponse::Ack { ok: true },
        ]);
        perform_handshake(false);
        assert!(host_capable());
        // A retry actually happened: more than the single QUERY_INFO above.
        assert!(MockNotifier::get_send_command_calls().len() > 1);
    }

    /// LOW-1: a device error on the first QUERY_INFO (TOCTOU unplug mid-send,
    // permission flap) is likewise transient — the token is released so the
    // handshake retries on the next reconnect.
    #[test]
    fn test_handshake_device_error_releases_dedup_token() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        // First attempt: device error (e.g. failed to open).
        MockNotifier::set_mock_send_errors(vec!["failed to open device".to_string()]);
        perform_handshake(false);
        assert!(!host_capable());
        assert_eq!(MockNotifier::get_send_command_calls().len(), 1);

        // Token released -> a capable reply on retry re-enables host rules.
        MockNotifier::set_mock_responses(vec![
            qmk_notifier::CommandResponse::Info {
                proto_ver: 2,
                feature_flags: 0x01,
                callback_count: 0,
                board_rules_present: false,
            },
            qmk_notifier::CommandResponse::Ack { ok: true },
        ]);
        perform_handshake(false);
        assert!(host_capable());
        assert!(MockNotifier::get_send_command_calls().len() > 1);
    }

    /// LOW-1 negative control: a genuine *legacy* reply (proto_ver != 2) is NOT
    /// transient — the firmware DID set `has_been_queried`, so the token must
    /// STAY consumed (re-querying risks the R6 mid-session-reconnect side
    /// effect). Only Timeout/Err are treated as retryable.
    #[test]
    fn test_handshake_legacy_reply_keeps_dedup_token() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        // proto_ver 1: genuine legacy reply, not a transient timeout.
        MockNotifier::set_mock_responses(vec![qmk_notifier::CommandResponse::Info {
            proto_ver: 1,
            feature_flags: 0x00,
            callback_count: 0,
            board_rules_present: false,
        }]);
        perform_handshake(false);
        assert!(!host_capable());
        let after_first = MockNotifier::get_send_command_calls().len();

        // Second call must short-circuit: legacy replies keep the token.
        perform_handshake(false);
        assert_eq!(
            MockNotifier::get_send_command_calls().len(),
            after_first,
            "legacy reply must keep the dedup token consumed"
        );
    }

    #[test]
    fn test_handshake_dedup_idempotent() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        MockNotifier::set_mock_responses(vec![
            qmk_notifier::CommandResponse::Info {
                proto_ver: 2,
                feature_flags: 0x01,
                callback_count: 1,
                board_rules_present: true,
            },
            qmk_notifier::CommandResponse::Ack { ok: true },
            qmk_notifier::CommandResponse::CallbackName {
                index: 0,
                name: Some("x".into()),
            },
        ]);
        perform_handshake(false);
        assert!(host_capable());
        let after_first = MockNotifier::get_send_command_calls().len();
        perform_handshake(false); // MUST short-circuit
        let after_second = MockNotifier::get_send_command_calls().len();
        assert_eq!(
            after_first, after_second,
            "dedup: second perform_handshake must not re-send"
        );
        assert!(host_capable());
    }

    #[test]
    fn test_handshake_reset_allows_rerun() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        MockNotifier::set_mock_responses(vec![
            qmk_notifier::CommandResponse::Info {
                proto_ver: 2,
                feature_flags: 0x01,
                callback_count: 1,
                board_rules_present: true,
            },
            qmk_notifier::CommandResponse::Ack { ok: true },
            qmk_notifier::CommandResponse::CallbackName {
                index: 0,
                name: Some("x".into()),
            },
        ]);
        perform_handshake(false);
        assert!(host_capable());
        reset_handshake_state();
        assert!(!host_capable());
        assert!(callback_names().is_empty());
        // re-arm + re-handshake (S2's device-gain path)
        MockNotifier::set_mock_responses(vec![
            qmk_notifier::CommandResponse::Info {
                proto_ver: 2,
                feature_flags: 0x01,
                callback_count: 1,
                board_rules_present: true,
            },
            qmk_notifier::CommandResponse::Ack { ok: true },
            qmk_notifier::CommandResponse::CallbackName {
                index: 0,
                name: Some("y".into()),
            },
        ]);
        perform_handshake(false);
        assert!(host_capable());
        assert_eq!(callback_names().get("y"), Some(&0));
    }

    #[test]
    fn test_handshake_skips_anonymous_callback() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        MockNotifier::set_mock_responses(vec![
            qmk_notifier::CommandResponse::Info {
                proto_ver: 2,
                feature_flags: 0x01,
                callback_count: 2,
                board_rules_present: true,
            },
            qmk_notifier::CommandResponse::Ack { ok: true },
            qmk_notifier::CommandResponse::CallbackName {
                index: 0,
                name: None,
            },
            qmk_notifier::CommandResponse::CallbackName {
                index: 1,
                name: Some("named".into()),
            },
        ]);
        perform_handshake(false);
        assert!(host_capable());
        let names = callback_names();
        assert_eq!(names.len(), 1);
        assert_eq!(names.get("named"), Some(&1));
    }

    #[test]
    fn test_unknown_callback_names_helper() {
        let rules: crate::core::rules::RuleSet = toml::from_str(
            r#"
[[rule]]
match = "a"
enable = ["known_a", "ghost"]
disable = ["known_b", "phantom"]
"#,
        )
        .unwrap();
        let mut known = HashMap::new();
        known.insert("known_a".to_string(), 0u8);
        known.insert("known_b".to_string(), 1u8);
        let unknown = unknown_callback_names(&rules, &known);
        assert_eq!(unknown, vec!["ghost".to_string(), "phantom".to_string()]);
    }

    #[test]
    fn test_handshake_action_transitions() {
        assert_eq!(handshake_action(None, true), HandshakeAction::Gain);
        assert_eq!(handshake_action(None, false), HandshakeAction::None);
        assert_eq!(handshake_action(Some(false), true), HandshakeAction::Gain);
        assert_eq!(handshake_action(Some(true), false), HandshakeAction::Loss);
        assert_eq!(handshake_action(Some(true), true), HandshakeAction::None);
        assert_eq!(handshake_action(Some(false), false), HandshakeAction::None);
    }

    #[test]
    fn test_poll_thread_seeded_from_startup() {
        // record_startup_device_state() probes is_device_connected() once and freezes the
        // result in the set-once STARTUP_DEVICE_CONNECTED. startup_device_was_connected()
        // then returns that bool. OnceLock is process-global & set-once, and tests run in one
        // process — so the FIRST record call wins; later calls are no-ops. We assert the
        // round-trip contract: after recording, the accessor matches the live probe (on a CI
        // box with no QMK device both are false; on a box with a device both are true).
        record_startup_device_state();
        assert_eq!(
            startup_device_was_connected(),
            is_device_connected(),
            "after record_startup_device_state, startup_device_was_connected must match the \
             live is_device_connected probe (OnceLock freezes the first record's value)"
        );
    }

    #[test]
    fn test_handshake_action_loss_on_seeded_true_to_false() {
        // The P1.M3.T1.S1 seed fix relies on this exact transition being `Loss`: when the poll
        // thread is seeded from startup_device_was_connected() == Some(true) and the first tick
        // transiently reads false, handshake_action(Some(true), false) MUST be Loss so
        // reset_handshake_state() fires and the subsequent reconnect re-sends SET_OS. (Also
        // asserted in test_handshake_action_transitions; restated here to pin the invariant
        // the seed fix depends on.)
        assert_eq!(handshake_action(Some(true), false), HandshakeAction::Loss);
    }

    // ========================================================================
    // Host-context send pipeline (P4.M3.T1.S1 / HOST_RULES.md §8(4))
    // ========================================================================
    // The send ORCHESTRATION is tested by injecting `ctx: Option<HostContext>`
    // directly into `dispatch_window_send` — no rules.toml file control needed
    // (the gate IO is covered separately; evaluate() correctness is P3.M1.T2.S1's
    // job, already green). The mock records `notify` -> MOCK_CALL_COUNT/
    // MOCK_LAST_MESSAGE and `send_command` -> MOCK_SEND_COMMAND_CALLS in separate
    // channels, so the string-before-context ORDER is structurally guaranteed by
    // the source order in dispatch_window_send's stack arm (send_legacy_string
    // precedes send_host_context) and asserted here via counts + command shape.

    /// Helper: build a default test DeviceFilter (the mock ignores it).
    fn test_filter() -> DeviceFilter {
        DeviceFilter {
            vendor_id: Some(0x1234),
            product_id: Some(0x5678),
            usage_page: 0xFF60,
            usage: 0x61,
        }
    }

    #[test]
    fn test_dispatch_legacy_string_only_when_no_host_context() {
        // ctx=None (host rules disabled) -> legacy string ONLY, no typed command.
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));

        let f = test_filter();
        let message = "App\x1DTitle";
        {
            let notifier = get_notifier();
            let n = notifier.lock().unwrap();
            let _res = dispatch_window_send(&**n, &f, message, None, "debounced", false);
            assert!(_res.is_ok());
        }

        assert_eq!(MockNotifier::get_call_count(), 1);
        assert_eq!(
            MockNotifier::get_last_message().as_deref(),
            Some("App\x1DTitle")
        );
        assert!(MockNotifier::get_send_command_calls().is_empty());
    }

    #[test]
    fn test_dispatch_stack_sends_string_then_context() {
        // Stack (any_match && !clear_board): string FIRST then context (clear:false).
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));

        use crate::core::rules::HostContext;
        let ctx = HostContext {
            layer: Some(224),
            callback_ids: vec![0, 1],
            clear_board: false,
            any_match: true,
        };
        let f = test_filter();
        let message = "App\x1DTitle";
        {
            let notifier = get_notifier();
            let n = notifier.lock().unwrap();
            let _res = dispatch_window_send(&**n, &f, message, Some(ctx), "immediate", false);
            assert!(_res.is_ok());
        }

        // String sent (count 1) + one ApplyHostContext{layer:Some(224),clear:false}.
        assert_eq!(MockNotifier::get_call_count(), 1);
        assert_eq!(
            MockNotifier::get_last_message().as_deref(),
            Some("App\x1DTitle")
        );
        let calls = MockNotifier::get_send_command_calls();
        assert_eq!(calls.len(), 1);
        assert!(matches!(
            calls[0],
            qmk_notifier::RunCommand::ApplyHostContext {
                layer: Some(224),
                callbacks: _,
                clear_board: false,
            }
        ));
    }

    #[test]
    fn test_dispatch_replace_sends_context_only() {
        // Replace (any_match && clear_board): context ONLY (clear:true), NO string.
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));

        use crate::core::rules::HostContext;
        let ctx = HostContext {
            layer: Some(225),
            callback_ids: vec![2],
            clear_board: true,
            any_match: true,
        };
        let f = test_filter();
        let message = "App\x1DTitle";
        {
            let notifier = get_notifier();
            let n = notifier.lock().unwrap();
            let _res = dispatch_window_send(&**n, &f, message, Some(ctx), "debounced", false);
            assert!(_res.is_ok());
        }

        // NO string sent (count 0); one ApplyHostContext{...,clear:true}.
        assert_eq!(MockNotifier::get_call_count(), 0);
        let calls = MockNotifier::get_send_command_calls();
        assert_eq!(calls.len(), 1);
        assert!(matches!(
            calls[0],
            qmk_notifier::RunCommand::ApplyHostContext {
                layer: Some(225),
                clear_board: true,
                ..
            }
        ));
    }

    #[test]
    fn test_dispatch_no_match_sends_string_then_clear_context() {
        // C13: a host no-match NEVER suppresses the board — the string IS sent
        // (board silo runs), THEN ApplyHostContext{layer:None,clear:false} clears
        // the host layer/callbacks only (board untouched).
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));

        use crate::core::rules::HostContext;
        let ctx = HostContext {
            layer: None,
            callback_ids: vec![],
            clear_board: false,
            any_match: false,
        };
        let f = test_filter();
        let message = "App\x1DTitle";
        {
            let notifier = get_notifier();
            let n = notifier.lock().unwrap();
            let _res = dispatch_window_send(&**n, &f, message, Some(ctx), "immediate", false);
            assert!(_res.is_ok());
        }

        // String sent FIRST (count 1, message preserved), then one
        // ApplyHostContext{layer:None,clear:false} (board untouched).
        assert_eq!(MockNotifier::get_call_count(), 1);
        assert_eq!(
            MockNotifier::get_last_message().as_deref(),
            Some("App\x1DTitle")
        );
        let calls = MockNotifier::get_send_command_calls();
        assert_eq!(calls.len(), 1);
        assert!(matches!(
            calls[0],
            qmk_notifier::RunCommand::ApplyHostContext {
                layer: None,
                callbacks: _,
                clear_board: false,
            }
        ));
    }

    #[test]
    fn test_host_context_for_window_none_when_not_capable() {
        // The gate: when not host-capable, host_context_for_window returns None
        // (regardless of rules.toml) -> caller sends the legacy string only.
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));

        assert!(!host_capable()); // sanity: reset cleared the capability bit
        let window_info = WindowInfo::new("TestApp".to_string(), "Title".to_string());
        assert!(host_context_for_window(&window_info, false).is_none());
    }

    #[test]
    fn test_notify_qmk_legacy_string_when_not_capable() {
        // Full notify_qmk path on a legacy board: host rules disabled -> the
        // legacy string is sent exactly once (first message is immediate) and
        // no typed ApplyHostContext command is emitted.
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));

        assert!(!host_capable());
        let window_info = WindowInfo::new("TestApp".to_string(), "Test Title".to_string());
        let result = notify_qmk(&window_info, false);
        assert!(result.is_ok());

        assert!(wait_for_count(1, Duration::from_secs(2)));
        assert_eq!(MockNotifier::get_call_count(), 1);
        assert_eq!(
            MockNotifier::get_last_message().as_deref(),
            Some("TestApp\x1DTest Title")
        );
        assert!(MockNotifier::get_send_command_calls().is_empty());
    }

    // ========================================================================
    // HandshakeOptions (#6/#7), callback-sweep cap (#4), board_has_rules (#5)
    // ========================================================================

    /// #6: `--validate-rules` runs the handshake in read-only mode — it must NOT
    /// send `SET_OS`, so a lint never mutates the firmware's `current_os`. Only
    /// `QUERY_INFO` (and any callback queries) go out.
    #[test]
    fn test_handshake_validation_mode_skips_set_os() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        MockNotifier::set_mock_responses(vec![qmk_notifier::CommandResponse::Info {
            proto_ver: 2,
            feature_flags: 0x01,
            callback_count: 0,
            board_rules_present: true,
        }]);
        perform_handshake_with(false, HandshakeOptions::validation());
        assert!(host_capable());
        let calls = MockNotifier::get_send_command_calls();
        assert_eq!(calls.len(), 1, "validation mode must send only QUERY_INFO");
        assert_eq!(calls[0], qmk_notifier::RunCommand::QueryInfo);
        assert!(
            !calls
                .iter()
                .any(|c| matches!(c, qmk_notifier::RunCommand::SetOs(_))),
            "validation mode must NOT send SET_OS (#6)"
        );
    }

    /// Contrast: the full live handshake (perform_handshake wrapper) DOES send
    /// `SET_OS`. (Pins the #6 regression: a change that accidentally gated
    /// `SET_OS` off in the full path would be caught here.)
    #[test]
    fn test_handshake_full_mode_sends_set_os() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        MockNotifier::set_mock_responses(vec![
            qmk_notifier::CommandResponse::Info {
                proto_ver: 2,
                feature_flags: 0x01,
                callback_count: 0,
                board_rules_present: true,
            },
            qmk_notifier::CommandResponse::Ack { ok: true }, // SET_OS
        ]);
        perform_handshake_with(false, HandshakeOptions::full());
        assert!(host_capable());
        let calls = MockNotifier::get_send_command_calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], qmk_notifier::RunCommand::QueryInfo);
        assert!(matches!(calls[1], qmk_notifier::RunCommand::SetOs(_)));
    }

    /// #4: a `callback_count` above `MAX_HOST_CALLBACKS` is clamped — only
    /// `MAX_HOST_CALLBACKS` `QUERY_CALLBACK` round-trips go out, so a buggy
    /// firmware reporting 255 cannot wedge the global notifier mutex for ~255s.
    /// (The mock returns its default `Ack` for the unsupplied replies, which the
    /// sweep logs-and-skips — the count of `QueryCallback` calls is what matters.)
    #[test]
    fn test_handshake_sweep_caps_at_max() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        MockNotifier::set_mock_responses(vec![
            qmk_notifier::CommandResponse::Info {
                proto_ver: 2,
                feature_flags: 0x01,
                callback_count: 70, // > MAX_HOST_CALLBACKS (64)
                board_rules_present: true,
            },
            qmk_notifier::CommandResponse::Ack { ok: true }, // SET_OS
        ]);
        perform_handshake(false);
        assert!(host_capable());
        let calls = MockNotifier::get_send_command_calls();
        let query_callbacks = calls
            .iter()
            .filter(|c| matches!(c, qmk_notifier::RunCommand::QueryCallback(_)))
            .count();
        assert_eq!(
            query_callbacks, MAX_HOST_CALLBACKS as usize,
            "sweep must clamp to MAX_HOST_CALLBACKS, not trust callback_count"
        );
    }

    /// #4: a realistic (small) `callback_count` sweeps that exact count — the
    /// cap only bites on absurd values, never a real keyboard.
    #[test]
    fn test_handshake_sweep_small_count_uncapped() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        MockNotifier::set_mock_responses(vec![
            qmk_notifier::CommandResponse::Info {
                proto_ver: 2,
                feature_flags: 0x01,
                callback_count: 3,
                board_rules_present: true,
            },
            qmk_notifier::CommandResponse::Ack { ok: true },
            qmk_notifier::CommandResponse::CallbackName {
                index: 0,
                name: Some("a".into()),
            },
            qmk_notifier::CommandResponse::CallbackName {
                index: 1,
                name: Some("b".into()),
            },
            qmk_notifier::CommandResponse::CallbackName {
                index: 2,
                name: Some("c".into()),
            },
        ]);
        perform_handshake(false);
        let calls = MockNotifier::get_send_command_calls();
        let query_callbacks = calls
            .iter()
            .filter(|c| matches!(c, qmk_notifier::RunCommand::QueryCallback(_)))
            .count();
        assert_eq!(query_callbacks, 3);
    }

    /// #5: after a capable handshake, `board_has_rules()` reflects the firmware's
    /// reported bit. `BOARD_HAS_RULES` is now set BEFORE `HOST_CAPABLE` so there
    /// is no window where `host_capable()` is true but `board_has_rules()` is
    /// stale — this test exercises that both are consistent immediately after.
    #[test]
    fn test_handshake_sets_board_has_rules_from_reported_bit() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        MockNotifier::set_mock_responses(vec![
            qmk_notifier::CommandResponse::Info {
                proto_ver: 2,
                feature_flags: 0x01,
                callback_count: 0,
                board_rules_present: true,
            },
            qmk_notifier::CommandResponse::Ack { ok: true },
        ]);
        assert!(!board_has_rules()); // sanity: reset cleared it
        perform_handshake(false);
        assert!(host_capable());
        assert!(board_has_rules());
    }

    // ========================================================================
    // BUG-HUNT FIX: debounce worker must not panic when `notify_qmk`'s
    // immediate-send (`due`) branch clears `pending` while the worker is parked
    // in its inner wait loop. Formerly the inner loop did
    // `state.pending.take().unwrap()`, which panicked on the cleared pending —
    // crashing the worker (and, with `panic = "abort"` in release, the whole
    // service) under rapid window switching at a debounce boundary.
    // ========================================================================
    #[test]
    fn test_debounce_worker_survives_pending_cleared_mid_wait() {
        reset_test_state();
        set_notifier(Box::new(MockNotifier::new()));
        // Long interval so the worker parks in the INNER wait_timeout once a
        // message is queued, giving a wide window to reproduce the race.
        STATE.lock().unwrap().interval_override = Some(Duration::from_secs(30));

        // Prime last_sent_time + drain via an immediate send (count -> 1).
        let _ = notify_qmk(&WindowInfo::new("A".into(), "1".into()), false);
        assert!(wait_for_count(1, Duration::from_millis(500)));

        // Queue a pending message -> worker enters the inner wait.
        let _ = notify_qmk(&WindowInfo::new("B".into(), "2".into()), false);
        thread::sleep(Duration::from_millis(150)); // let the worker park

        // Reproduce `notify_qmk`'s `due` branch racing at the debounce
        // boundary: clear pending AND shrink the interval so the worker's
        // next inner iteration sees now >= target with pending == None.
        {
            let mut s = STATE.lock().unwrap();
            s.pending = None;
            s.interval_override = Some(Duration::from_millis(1));
            COND.notify_all();
        }

        // Give the worker time to wake and (formerly) panic. With the fix it
        // gracefully falls back to waiting for the next message.
        thread::sleep(Duration::from_millis(300));

        // STATE must NOT be poisoned (worker did not panic).
        assert!(
            STATE.lock().is_ok(),
            "worker panicked on cleared pending (immediate-send / flush race) — \
             STATE poisoned"
        );

        // The worker must still be alive and able to process a new message:
        // restore a normal interval and send a fresh window; it should flush.
        STATE.lock().unwrap().interval_override = Some(Duration::from_millis(50));
        let _ = notify_qmk(&WindowInfo::new("C".into(), "3".into()), false);
        assert!(
            wait_for_count(2, Duration::from_millis(500)),
            "worker must keep processing after the survived race"
        );
    }

    /// Defense-in-depth: debounce_worker / notify_qmk recover from a poisoned STATE
    /// mutex (debug/test builds only). Release uses `panic = "abort"`, so a panic
    /// kills the process before any re-lock and poisoning is impossible there.
    ///
    /// We verify the recovery idiom on a LOCAL `Mutex<DebounceState>` rather than
    /// the global `STATE`: std `Mutex` poison cannot be cleared on stable Rust, so
    /// poisoning the global `STATE` would permanently contaminate it and break
    /// `reset_test_state()` (STATE.lock().unwrap()) plus the `STATE.lock().is_ok()`
    /// assertion in `test_debounce_worker_survives_pending_cleared_mid_wait` for
    /// every test run afterward. The idiom `unwrap_or_else(|e| e.into_inner())` is
    /// generic over Mutex<T>, so a local mutex is a faithful proof that the four
    /// hardened production sites (STATE.lock @ worker+notify_qmk, COND.wait,
    /// COND.wait_timeout) recover identically. We additionally call notify_qmk on
    /// the (unpoisoned) global STATE to confirm the hardening didn't regress the
    /// normal path.
    #[test]
    fn test_debounce_worker_survives_poisoned_state() {
        // --- Part A: the recovery idiom survives a poisoned Mutex<DebounceState>. ---
        let local: Mutex<DebounceState> = Mutex::new(DebounceState {
            last_sent_time: None,
            pending: None,
            verbose: false,
            interval_override: Some(Duration::from_millis(50)),
        });

        // Poison it: lock, then panic under catch_unwind. The guard is dropped during
        // unwinding, which sets the mutex's poison flag (permanent on stable Rust).
        let panic_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = local.lock().unwrap();
            panic!("intentional: poison the mutex");
        }));
        assert!(
            panic_res.is_err(),
            "helper must panic to set the poison flag"
        );
        assert!(
            local.lock().is_err(),
            "mutex must be poisoned after the panic"
        );

        // Recovery — the EXACT idiom the four hardened production sites use:
        // PoisonError::into_inner() returns the inner guard, usable despite poison.
        {
            let mut guard = local.lock().unwrap_or_else(|e| e.into_inner());
            guard.interval_override = Some(Duration::from_millis(10)); // prove it's usable
            assert_eq!(guard.interval_override, Some(Duration::from_millis(10)));
        }
        // (COND.wait / COND.wait_timeout share the same PoisonError::into_inner shape:
        //  wait         -> PoisonError<MutexGuard>.into_inner()              -> MutexGuard
        //  wait_timeout -> PoisonError<(MutexGuard, WaitTimeoutResult)>.into_inner().0 -> MutexGuard)

        // --- Part B: notify_qmk still works on the (unpoisoned) global STATE ---
        // (confirms the 4-site hardening didn't regress the normal/unpoisoned path).
        reset_test_state();
        set_notifier(Box::new(MockNotifier::new()));
        let res = notify_qmk(&WindowInfo::new("PoisonChk".into(), "t".into()), false);
        assert!(res.is_ok());
        assert!(
            wait_for_count(1, Duration::from_millis(500)),
            "notify_qmk must still flush on an unpoisoned STATE after hardening"
        );
    }

    // ---- DeviceStatus three-state derivation (P1.M1.T1.S1) ----

    #[test]
    fn test_classify_device_status_truth_table() {
        // All three rows of the §3 table, deterministically (no hardware needed).
        use DeviceStatus::*;
        // present=false dominates regardless of `capable`:
        assert_eq!(classify_device_status(false, false), Disconnected);
        assert_eq!(classify_device_status(false, true), Disconnected);
        // present=true, not capable -> the headline NoModule state:
        assert_eq!(classify_device_status(true, false), NoModule);
        // present=true, capable -> Connected:
        assert_eq!(classify_device_status(true, true), Connected);
    }

    #[test]
    fn test_device_status_is_disconnected_in_ci_without_hardware() {
        // device_status() wires is_device_connected() (Tier-1 enumerate) + host_capable().
        // When NO Tier-1 board is present (the CI case — `is_device_connected()`
        // enumerates real HID hardware, so it is false on clean CI runners), the
        // result MUST be Disconnected even if a stale HOST_CAPABLE=true lingered
        // (present=false dominates; a stale capability flag can never fabricate a
        // false "NoModule"/"Connected"). That dominance is also proved directly +
        // deterministically by test_classify_device_status_truth_table's
        // classify_device_status(false, true) == Disconnected row, which does not
        // depend on hardware. Here we assert the live device_status() path too, but
        // ONLY when this machine genuinely has no matching board — a developer box
        // may have a QMK board plugged in, in which case device_status() legitimately
        // reads NoModule/Connected and the Disconnected assertion does not apply.
        let present = is_device_connected();
        if !present {
            reset_handshake_state(); // HOST_CAPABLE = false
            assert_eq!(device_status(), DeviceStatus::Disconnected);

            HOST_CAPABLE.store(true, Ordering::SeqCst); // simulate a stale capable flag
            assert_eq!(
                device_status(),
                DeviceStatus::Disconnected,
                "no Tier-1 board present must dominate a stale HOST_CAPABLE"
            );
            reset_handshake_state(); // restore HOST_CAPABLE = false (isolation)
        } else {
            // A board is present on this machine — the Disconnected branch is not
            // reachable here. Still confirm device_status() matches the helper for
            // both HOST_CAPABLE values (the public fn delegates correctly).
            reset_handshake_state();
            assert_eq!(device_status(), classify_device_status(present, false));
            HOST_CAPABLE.store(true, Ordering::SeqCst);
            assert_eq!(device_status(), classify_device_status(present, true));
            reset_handshake_state(); // restore HOST_CAPABLE = false (isolation)
        }
    }

    // ===== Device classification cache tests (P3.M1.T1.S1) =====
    // Pure helper tests — no HID mock needed (the helpers only lock a static
    // HashMap). Each test starts with classification_cache_clear() to avoid
    // cross-test bleed (the static outlives tests; crate tests are single-
    // threaded per AGENTS.md).

    fn capable_sample() -> DeviceKind {
        DeviceKind::Capable {
            proto_ver: 2,
            feature_flags: 1,
            callback_count: 4,
            board_rules_present: true,
        }
    }

    #[test]
    fn test_classification_cache_insert_then_get() {
        classification_cache_clear();
        let kind = capable_sample();
        classification_cache_insert("p-capable", kind.clone());
        let got = classification_cache_get("p-capable");
        assert_eq!(got, Some(kind));
    }

    #[test]
    fn test_classification_cache_miss() {
        classification_cache_clear();
        // An unseen path yields None.
        assert_eq!(classification_cache_get("never-inserted"), None);
        // Inserting one path does not populate a different path.
        classification_cache_insert("p-a", capable_sample());
        assert_eq!(classification_cache_get("p-b"), None);
    }

    #[test]
    fn test_classification_cache_clear() {
        classification_cache_clear();
        classification_cache_insert("p-clear", capable_sample());
        assert!(classification_cache_get("p-clear").is_some());
        classification_cache_clear();
        assert_eq!(classification_cache_get("p-clear"), None);
    }

    #[test]
    fn test_classification_cache_overwrite() {
        classification_cache_clear();
        classification_cache_insert("p-ow", capable_sample());
        classification_cache_insert("p-ow", DeviceKind::NotQmkNotifier);
        assert_eq!(
            classification_cache_get("p-ow"),
            Some(DeviceKind::NotQmkNotifier)
        );
    }

    #[test]
    fn test_classification_cache_ttl_expiry() {
        classification_cache_clear();
        classification_cache_insert("p-ttl", capable_sample());
        // Sanity: fresh entry hits.
        assert!(classification_cache_get("p-ttl").is_some());
        // Simulate expiry by rewriting the stored Instant to the past
        // (same-module test reaching into the private static is fine).
        CLASSIFICATION_CACHE
            .lock()
            .unwrap()
            .insert(
                "p-ttl".to_string(),
                (
                    capable_sample(),
                    Instant::now() - CLASSIFICATION_TTL - Duration::from_millis(1),
                ),
            );
        assert_eq!(classification_cache_get("p-ttl"), None);
    }

    #[test]
    fn test_classification_cache_notqmk_variant() {
        classification_cache_clear();
        let kind = DeviceKind::NotQmkNotifier;
        classification_cache_insert("p-nq", kind.clone());
        assert_eq!(classification_cache_get("p-nq"), Some(kind));
    }

    #[test]
    fn test_devicekind_classifieddevice_derives() {
        // DeviceKind::Capable PartialEq sanity (field-for-field equality).
        let cap_a = capable_sample();
        let cap_b = capable_sample();
        assert_eq!(cap_a, cap_b);
        // Clone produces an equal value (DeviceKind: Clone is required by
        // classification_cache_get's owned return).
        assert_eq!(cap_a.clone(), cap_b);
        // NotQmkNotifier unit variant PartialEq + Clone sanity.
        assert_eq!(DeviceKind::NotQmkNotifier, DeviceKind::NotQmkNotifier.clone());
        // The two variants are distinct.
        assert_ne!(cap_a, DeviceKind::NotQmkNotifier);

        // ClassifiedDevice PartialEq + Clone sanity (the picker clones rows).
        let dev_a = ClassifiedDevice {
            path: "p-dev".to_string(),
            vendor_id: 0xFEED,
            product_id: 0x0000,
            product_name: Some("Dactyl".to_string()),
            usage_page: 0xFF60,
            usage: 0x61,
            kind: cap_a.clone(),
        };
        assert_eq!(dev_a, dev_a.clone());
        assert_eq!(dev_a.kind, cap_b);
    }
}

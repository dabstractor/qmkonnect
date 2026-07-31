use crate::core::types::WindowInfo;
use once_cell::sync::Lazy;
use std::collections::{BTreeSet, HashMap};
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
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
/// to QMK's raw-HID defaults. VID/PID default to `None` (auto-discovery) when
/// unset; usage page/usage default to `0xFF60`/`0x61` (overridable for boards
/// that changed `RAW_USAGE_PAGE`/`RAW_USAGE_ID` in firmware). Read per-call so
/// config changes take effect without restarting the service.
fn configured_filter() -> DeviceFilter {
    let cfg = crate::platforms::get_config_paths()
        .into_iter()
        .find(|p| p.exists())
        .and_then(|p| crate::core::parse_config(&p).ok());
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
                if let Err(e) = n.send_command(qmk_notifier::RunCommand::SetOs(host_os()), &filter) {
                    eprintln!("Warning: SET_OS failed during handshake: {}", e);
                }
            }
            // Callback sweep → local map (publish after dropping the notifier lock: D2).
            // #4: bound both the count (`MAX_HOST_CALLBACKS`) and the wall clock
            // (`CALLBACK_SWEEP_DEADLINE`) so a misbehaving firmware cannot wedge
            // the global notifier mutex (and every notification behind it).
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
                if sweep_start.elapsed() > CALLBACK_SWEEP_DEADLINE {
                    eprintln!(
                        "Warning: callback sweep exceeded {}s budget at index {} \
                         ({} of {} done) — stopping early to avoid wedging notifications",
                        CALLBACK_SWEEP_DEADLINE.as_secs(), i, local.len(), sweep_cap
                    );
                    break;
                }
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
            }
            drop(n); // release the notifier before the read-only rules validation
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
/// `[[callback_rules]]` `enable`/`disable` but absent from the keyboard's
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
    for rule in &rules.callback_rules {
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

impl Notifier for QmkNotifier {
    fn notify(&self, message: String) -> Result<(), Box<dyn Error + Send + Sync>> {
        let f = configured_filter();

        // Retry device connection with exponential backoff
        for attempt in 1..=3 {
            let params = qmk_notifier::RunParameters::new(
                qmk_notifier::RunCommand::SendMessage(message.clone()),
                f.vendor_id,
                f.product_id,
                f.usage_page,
                f.usage,
                false, // verbose
            );
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
            command,
            filter.vendor_id,
            filter.product_id,
            filter.usage_page,
            filter.usage,
            false, // verbose — transport stays quiet; orchestration logs (D3)
        );
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
// `debounce_ms` (and `poll_interval_ms`) are hot config (PRD §8,
// ARCHITECTURE.md §10 #4, CONFIG.md §1.2): they are re-read from
// `configured_debounce_ms()` on every notification and every status poll, so
// they are intentionally NOT cached here — editing `config.toml` takes effect
// within ~3 s with no restart.
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
            let mut state = STATE.lock().unwrap();

            // Wait until something is actually queued.
            while state.pending.is_none() {
                state = COND.wait(state).unwrap();
            }

            // Wait out the debounce window relative to the last send.
            let mut to_send: Option<(PendingMessage, bool)> = None;
            while to_send.is_none() {
                let last = state.last_sent_time.unwrap_or_else(Instant::now);
                let target = last + state.interval();
                let now = Instant::now();
                if now >= target {
                    let pm = state.pending.take().unwrap();
                    let verbose = state.verbose;
                    state.last_sent_time = Some(Instant::now());
                    to_send = Some((pm, verbose));
                } else {
                    state = COND.wait_timeout(state, target - now).unwrap().0;
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
        let mut state = STATE.lock().unwrap();
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
    let rules = match crate::core::rules::parse_rules(&path) {
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
    qmk_notifier::RunCommand::ApplyHostContext {
        layer: ctx.layer,
        callbacks: ctx.callback_ids.clone(),
        clear_board: ctx.clear_board,
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

    fn reset_global_mock() {
        MOCK_CALL_COUNT.store(0, Ordering::SeqCst);
        *MOCK_LAST_MESSAGE.lock().unwrap() = None;
        MOCK_SEND_COMMAND_CALLS.lock().unwrap().clear();
        MOCK_RESPONSES.lock().unwrap().clear();
        MOCK_SEND_COMMAND_ERRORS.lock().unwrap().clear();
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
[[callback_rules]]
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
}

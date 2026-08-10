use crate::core::types::WindowInfo;
use once_cell::sync::Lazy;
use std::collections::{BTreeSet, HashMap};
use std::error::Error;
use std::path::{Path, PathBuf};
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

/// The `--list-devices` kind-column label for a classified Tier-1 device
/// (`spec/DEVICE_DISCOVERY.md` §8): `qmk_notifier` for a capable board,
/// `qmk-only` for a QMK raw-HID board that isn't running the qmk_notifier module.
/// Pure; unit-tested (`kind_label_matches_spec`).
fn kind_label(kind: &DeviceKind) -> &'static str {
    match kind {
        DeviceKind::Capable { .. } => "qmk_notifier",
        DeviceKind::NotQmkNotifier => "qmk-only",
    }
}

/// Print every HID device the kernel can see, WITHOUT opening any of them — the
/// VID/PID discovery tool (`spec/DEVICE_DISCOVERY.md` §8 / `PROTOCOL.md` §6).
/// Read-only enumeration (never seizes a device). Adds a **`kind`** column from a
/// one-shot [`classify_devices`] pass: Tier-1 QMK raw-HID boards that answered
/// the capability probe show `qmk_notifier` (capable) or `qmk-only` (QMK board,
/// no qmk_notifier module); all other interfaces show `-`.
///
/// `classify_devices` runs against the *configured* filter, so when `vendor_id`/
/// `product_id` are set, boards outside that filter are not classified and show
/// `-` (the common no-VID/PID case classifies all Tier-1 boards). If the HID
/// classification itself fails, the kind map is empty and every cell is `-` — no
/// panic, no error (§7.2/§8).
///
/// `verbose` is forwarded to [`classify_devices`]: `-v` prints per-candidate probe
/// diagnostics to **stderr** (the **stdout** table stays clean).
pub fn list_devices(verbose: bool) -> Result<(), Box<dyn Error>> {
    let api = hidapi::HidApi::new()?;

    // One-shot Tier-2 classification (cache-backed; pings only on a cold/stale
    // cache). Keyed by the stable hidapi `path` (mirrors enumerate_candidates)
    // so each enumerated interface maps to its own classification. Returns [] on
    // any HID error ⇒ the kind column degrades to `-` everywhere (G5).
    let kind_by_path: std::collections::HashMap<String, DeviceKind> = classify_devices(verbose)
        .into_iter()
        .map(|c| (c.path, c.kind))
        .collect();

    println!("Available HID devices (vendor:product  usage_page:usage  product  kind):");
    for d in api.device_list() {
        let kind = kind_by_path
            .get(&d.path().to_string_lossy().to_string())
            .map(kind_label)
            .unwrap_or("-");
        println!(
            "  {:#06x}:{:#06x}  {:#06x}:{:#06x}  {}  {}",
            d.vendor_id(),
            d.product_id(),
            d.usage_page(),
            d.usage(),
            d.product_string().unwrap_or(""),
            kind,
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

/// Maximum number of attempts for a single [`RunCommand::QueryCallback`] in
/// [`query_callback_with_retry`] (Finding #1). A transient mis-read can surface
/// the reply as a generic `Ack` instead of a `CallbackName`; a single retry on
/// that specific index clears the overwhelming majority of such transients
/// without re-running the whole sweep. The per-iteration lock is released
/// between attempts so a window notification can still interleave.
const QUERY_CALLBACK_MAX_ATTEMPTS: usize = 2;

/// Query one callback index, retrying once on a transient mis-parse.
///
/// Returns `Some((name, index))` when a `CallbackName` reply (with a name) is
/// decoded; `None` otherwise (no name, an unexpected reply after all retries,
/// or an I/O error). Each attempt re-acquires `notifier` for itself and releases
/// it before returning/yielding, mirroring the sweep's per-iteration lock
/// discipline (#4) so a window-notification send can interleave between
/// attempts.
///
/// This is qmkonnect-side hardening for Finding #1: the root cause of the
/// transient mis-parse lives outside this repo (firmware timing or the
/// `qmk-notifier` crate's bounded read), but retrying the single affected index
/// clears it without re-running the whole sweep, and (combined with the
/// empty-map warning in the caller) prevents a session-long silent no-op of
/// host-rule callback toggles.
fn query_callback_with_retry(
    notifier: &Arc<Mutex<Box<dyn Notifier>>>,
    index: u8,
    filter: &DeviceFilter,
    verbose: bool,
) -> Option<(String, u8)> {
    for attempt in 0..QUERY_CALLBACK_MAX_ATTEMPTS {
        let n = notifier.lock().unwrap_or_else(|e| e.into_inner());
        let reply = n.send_command(qmk_notifier::RunCommand::QueryCallback(index), filter);
        drop(n); // release NOTIFIER before any retry / the sweep's yield
        match reply {
            Ok(qmk_notifier::CommandResponse::CallbackName {
                index: idx,
                name: Some(name),
            }) => return Some((name, idx)),
            Ok(qmk_notifier::CommandResponse::CallbackName { name: None, .. }) => {
                if verbose {
                    eprintln!(
                        "[{}ms] perform_handshake: callback {} has no name — skipped",
                        crate::core::now_ms(),
                        index
                    );
                }
                return None;
            }
            Ok(other) => {
                // Transient mis-parse: the firmware (or the crate's bounded read)
                // surfaced a generic Ack where a CallbackName was expected. Retry
                // once on this index; if it still mis-parses, log + give up.
                if attempt + 1 < QUERY_CALLBACK_MAX_ATTEMPTS {
                    if verbose {
                        eprintln!(
                            "[{}ms] perform_handshake: callback {} unexpected reply {:?} — retrying",
                            crate::core::now_ms(),
                            index,
                            other
                        );
                    }
                    thread::yield_now();
                    continue;
                }
                if verbose {
                    eprintln!(
                        "[{}ms] perform_handshake: callback {} unexpected reply {:?}",
                        crate::core::now_ms(),
                        index,
                        other
                    );
                }
                return None;
            }
            Err(e) => {
                eprintln!("Warning: QUERY_CALLBACK({}) failed: {}", index, e);
                return None;
            }
        }
    }
    None
}

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
    let n = notifier.lock().unwrap_or_else(|e| e.into_inner());

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
                let mapped = query_callback_with_retry(&notifier, i, &filter, verbose);
                if let Some((name, index)) = mapped {
                    local.insert(name, index);
                } // None: logged inside; transient mis-read retried there
                  // #4: yield so a window-notification waiter (`notify_qmk`'s immediate send or
                  // `debounce_worker`'s flush — both BLOCKING on NOTIFIER) actually gets to acquire
                  // the lock before we re-lock for the next iteration. Without this, std::sync::Mutex's
                  // unfair barging re-acquires in ~ns and starves the woken waiter for the whole sweep,
                  // defeating the per-iteration release. sched_yield is ~1µs and a no-op when nothing
                  // else is runnable (N<=64 iterations => <=~64µs/handshake, negligible).
                thread::yield_now();
            }
            // Finding #1: a transient mis-read of one QUERY_CALLBACK reply can leave the
            // map empty despite a nonzero firmware `callback_count`, silently no-op'ing
            // every host-rule callback toggle for the whole session (the handshake is
            // deduped per board boot). Surface that as a non-verbose warning so the user
            // isn't left guessing why their `vim_lazy` rule did nothing.
            if callback_count > 0 && local.is_empty() {
                eprintln!(
                    "Warning: firmware reported {} callbacks but none could be mapped \
                     — host-rule callback toggles (enable/disable) will be no-ops this session. \
                     Reconnect the keyboard to retry the handshake.",
                    callback_count
                );
            }
            {
                let mut names = CALLBACK_NAMES.lock().unwrap_or_else(|e| e.into_inner());
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
            // Warm the per-path cache from this handshake result so classify_devices
            // reads a TTL hit (single-ping-per-appearance, §2.4). Best-effort:
            // enumerate_candidates finds 0 Tier-1 devices under MockNotifier.
            warm_cache_from_handshake(DeviceKind::Capable {
                proto_ver: 2,
                feature_flags,
                callback_count,
                board_rules_present,
            });
            if verbose {
                eprintln!(
                    "[{}ms] perform_handshake: complete — capable ({} callbacks mapped)",
                    crate::core::now_ms(),
                    CALLBACK_NAMES
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .len()
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
            CALLBACK_NAMES
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clear();
            HAS_HANDSHAKED.store(false, Ordering::SeqCst); // transient — allow retry
                                                           // Warm the per-path cache (best-effort no-op under MockNotifier).
            warm_cache_from_handshake(DeviceKind::NotQmkNotifier);
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
            CALLBACK_NAMES
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clear();
            // Warm the per-path cache (best-effort no-op under MockNotifier).
            warm_cache_from_handshake(DeviceKind::NotQmkNotifier);
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
            CALLBACK_NAMES
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clear();
            // LOW-1: a device error means QUERY_INFO never landed on the
            // firmware — release the dedup token so the next poll/reconnect
            // retries the handshake against the capable board.
            HAS_HANDSHAKED.store(false, Ordering::SeqCst);
            // Warm the per-path cache (best-effort no-op under MockNotifier).
            warm_cache_from_handshake(DeviceKind::NotQmkNotifier);
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
    let known = CALLBACK_NAMES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
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
    CALLBACK_NAMES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Clear all handshake state (capability flag, callback map, dedup guard).
///
/// Called by P4.M2.T1.S2 on a real device transition (`is_device_connected()`
/// false→true) so the next [`perform_handshake`] re-runs, and by the handshake
/// tests for isolation.
// Linux has no background presence probe (macOS/Windows only), so this is only
// reached from those runners + tests; allow dead code on Linux.
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
pub fn reset_handshake_state() {
    HOST_CAPABLE.store(false, Ordering::SeqCst);
    BOARD_HAS_RULES.store(false, Ordering::SeqCst);
    CALLBACK_NAMES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
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
///   the handshake on a capable `QUERY_INFO` reply and reset `false` on a
///   capable-board Loss (the tray poll threads' [`PresenceTracker`] detects a
///   capable board leaving even when a non-capable Tier-1 board remains — Finding
///   #1) or a handshake failure.
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

// ===== Device classification — S2: classify_devices (P3.M1.T1.S2) =====
// The Tier-2 per-candidate capability probe + cache mechanics. S1 (above)
// ships the TYPES + CACHE + HELPERS; this section ships the probe that
// POPULATES the cache. See `spec/DEVICE_DISCOVERY.md` §2 (the algorithm
// source of truth) + the gotchas pinned in this section's comments.

/// Classify a `QUERY_INFO` reply into a [`DeviceKind`] (`spec/DEVICE_DISCOVERY.md`
/// §2.2).
///
/// `Ok(Info { proto_ver: 2, .. })` ⇒ [`DeviceKind::Capable`] (records all four
/// fields verbatim so the picker can show them); every other reply
/// (`Legacy` / `Timeout` / `Ack` / `CallbackName` / `Info { proto_ver != 2 }`)
/// and every `Err(_)` ⇒ [`DeviceKind::NotQmkNotifier`]. No board is harmed: the
/// `0x81 0x9F` magic header is silently ignored by VIA/Vial's `raw_hid_receive`
/// (the R-COEX guarantee — §2.2).
///
/// Does NOT gate on `feature_flags & 0x01`. The `APPLY_HOST_CONTEXT` bit is the
/// handshake's gate for the host-rules SEND (`perform_handshake_with` @~444);
/// the classifier records `feature_flags` so the consumer (the picker / status
/// resolver) can read it. Adding the gate here would hide capable-but-no-host-
/// rules boards from the picker — diverges from §2.2.
#[allow(dead_code)]
fn classify_reply(
    resp: Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>>,
) -> DeviceKind {
    match resp {
        Ok(qmk_notifier::CommandResponse::Info {
            proto_ver: 2,
            feature_flags,
            callback_count,
            board_rules_present,
        }) => DeviceKind::Capable {
            proto_ver: 2,
            feature_flags,
            callback_count,
            board_rules_present,
        },
        _ => DeviceKind::NotQmkNotifier,
    }
}

/// One enumerated Tier-1 HID interface, pre-classification (the cache key +
/// the four Tier-1 narrowers). Factored out of [`classify_devices`] so
/// [`classify_candidates`] is testable without a real HID bus — the enumerate
/// step talks to `hidapi::HidApi::new()` (uncontrollable in a unit test; the CI
/// box may have 0 or N QMK boards), but the classify step is pure w.r.t. a
/// `Vec<Candidate>` + the queued mock responses.
#[allow(dead_code)]
struct Candidate {
    path: String,
    vendor_id: u16,
    product_id: u16,
    product_name: Option<String>,
    usage_page: u16,
    usage: u16,
}

/// Enumerate the Tier-1 HID candidates (`spec/DEVICE_DISCOVERY.md` §2.3):
/// `usage_page == 0xFF60 && usage == 0x61` plus the optional vid/pid narrowers
/// from `configured_filter()`. Verbatim `.filter`/`.map`/`.collect` mirror of
/// [`is_device_connected`] @~216 (which uses `.any`). Read-only enumeration —
/// `HidApi::new()` never opens the device and never sends a report, so it is
/// R-COEX safe (identical to the poll-thread enumeration that already runs every
/// tick). Returns `vec![]` if hidapi cannot enumerate (the "device absent"
/// degradation).
///
/// `d.path()` returns `&CStr` (hidapi 2.6.3, `DeviceInfo::path`), not `&OsStr` —
/// convert via `.to_string_lossy().to_string()` so a non-UTF8 path (rare but
/// possible on Windows) degrades instead of panicking.
fn enumerate_candidates() -> Vec<Candidate> {
    let f = configured_filter();
    match hidapi::HidApi::new() {
        Ok(api) => api
            .device_list()
            .filter(|d| {
                d.usage_page() == f.usage_page
                    && d.usage() == f.usage
                    && f.vendor_id.is_none_or(|v| d.vendor_id() == v)
                    && f.product_id.is_none_or(|p| d.product_id() == p)
            })
            .map(|d| Candidate {
                path: d.path().to_string_lossy().to_string(),
                vendor_id: d.vendor_id(),
                product_id: d.product_id(),
                product_name: d.product_string().map(|s| s.to_string()),
                usage_page: d.usage_page(),
                usage: d.usage(),
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// The pure, MockNotifier-testable core of the Tier-2 probe (`spec/
/// DEVICE_DISCOVERY.md` §2.3): classify each Tier-1 candidate by consulting the
/// [`CLASSIFICATION_CACHE`] (TTL hit ⇒ reuse, no ping) or else pinging the board
/// with a vid/pid-narrowed [`DeviceFilter`], then caching the result. This is
/// factored out of [`classify_devices`] so the unit tests can drive it with a
/// hand-built `Vec<Candidate>` + a queued mock response queue (deterministic,
/// no real HID) — `classify_devices` itself only provides the hidapi shell.
///
/// Per-candidate mechanism: the `qmk_notifier` crate has **no per-path send**
/// (`external_deps.md`: `MatchKey` is private + filter-keyed — every send
/// broadcasts to ALL devices matching the filter). vid/pid narrowing is the sole
/// app-side knob, so each candidate's `DeviceFilter` is pinned to
/// `(Some(c.vendor_id), Some(c.product_id), c.usage_page, c.usage)`. **LIMITATION
/// (`DEVICE_DISCOVERY.md` §4.3):** this is a true single-device ping ONLY when
/// vid/pid is unique on the bus; two boards sharing vid/pid (e.g. a split pair)
/// both match the narrowed filter and both get pinged — the app cannot attribute
/// the single reply to a specific path. The handshake has the same limitation;
/// both are bounded by the same single-vid/pid-on-bus assumption.
///
/// Lock discipline: the notifier lock is acquired **per candidate** (short
/// scope), mirroring the callback sweep's per-iteration re-acquire
/// (`perform_handshake_with` @~446) so a concurrent `notify_qmk` / debounce
/// flush can interleave between candidates. Holding one lock across all
/// candidates would starve the notification path. `.lock().unwrap_or_else(|e|
/// e.into_inner())` matches `perform_handshake_with`'s poison-recovery idiom
/// (PRD §10).
#[allow(dead_code)]
fn classify_candidates(candidates: Vec<Candidate>, verbose: bool) -> Vec<ClassifiedDevice> {
    let notifier = get_notifier();
    candidates
        .into_iter()
        .map(|c| {
            let kind = match classification_cache_get(&c.path) {
                Some(k) => {
                    if verbose {
                        eprintln!(
                            "[{}ms] classify: cache hit {}",
                            crate::core::now_ms(),
                            c.path
                        );
                    }
                    k
                }
                None => {
                    // Narrow the filter to this candidate's vid/pid (the crate
                    // has no per-path send — see the §4.3 limitation above).
                    let narrowed = DeviceFilter {
                        vendor_id: Some(c.vendor_id),
                        product_id: Some(c.product_id),
                        usage_page: c.usage_page,
                        usage: c.usage,
                    };
                    let resp = notifier
                        .lock()
                        .unwrap()
                        .send_command(qmk_notifier::RunCommand::QueryInfo, &narrowed);
                    let kind = classify_reply(resp);
                    classification_cache_insert(&c.path, kind.clone());
                    kind
                }
            };
            ClassifiedDevice {
                path: c.path,
                vendor_id: c.vendor_id,
                product_id: c.product_id,
                product_name: c.product_name,
                usage_page: c.usage_page,
                usage: c.usage,
                kind,
            }
        })
        .collect()
}

/// Drop cache entries whose `path` is no longer present in the Tier-1 candidate
/// set (`spec/DEVICE_DISCOVERY.md` §2.3 — eviction on disappearance). A board
/// that unplugged mid-session must not keep advertising a stale `DeviceKind`.
/// Uses `Vec::contains` (n = board count ≈ 1-2) to avoid importing `HashSet`.
#[allow(dead_code)]
fn invalidate_absent_cache_entries(candidates: &[Candidate]) {
    let present: Vec<&str> = candidates.iter().map(|c| c.path.as_str()).collect();
    if let Ok(mut map) = CLASSIFICATION_CACHE.lock() {
        map.retain(|path, _| present.contains(&path.as_str()));
    }
}

/// Enumerate present Tier-1 candidates, invalidate absent cache entries, then
/// classify each (`spec/DEVICE_DISCOVERY.md` §2.3). The top-level Tier-2 entry
/// point: called by the discovered-device picker (P3.M2), the `device_status()`
/// per-device resolver (P1), and the `--list-devices` kind column (P4.M1.T1.S1).
///
/// Algorithm: `enumerate_candidates` (the hidapi shell) →
/// `invalidate_absent_cache_entries` (drop disappeared paths) →
/// [`classify_candidates`] (the pure, cache-aware per-candidate core). Cache
/// hits (within [`CLASSIFICATION_TTL`]) skip the ping; misses narrow a
/// [`DeviceFilter`] to the candidate's vid/pid and ping `QUERY_INFO` (the
/// mechanism + its multi-same-vid/pid limitation are documented on
/// [`classify_candidates`]). The handshake path ([`perform_handshake_with`])
/// warm-feeds the same cache via [`warm_cache_from_handshake`] so the status
/// path stays single-ping-per-appearance (§2.4).
#[allow(dead_code)]
pub fn classify_devices(verbose: bool) -> Vec<ClassifiedDevice> {
    let candidates = enumerate_candidates();
    invalidate_absent_cache_entries(&candidates);
    classify_candidates(candidates, verbose)
}

/// Whether the broadcast handshake result can be safely warm-stamped into the
/// per-path cache. True only when **at most one** Tier-1 board is present: the
/// handshake is filter-keyed (broadcast — the crate has no per-path send), so
/// its single `QUERY_INFO` reply can be attributed to a path ONLY when there is
/// exactly one candidate (broadcast == unicast). With ≥2 boards on the bus — the
/// headline F13 mixed case (a capable board + a pure-VIA board) — the handshake
/// cannot tell which path replied, and stamping every enumerated path with the
/// one result would mislabel the non-capable board `✓ qmk_notifier` in the
/// discovered-device picker until [`CLASSIFICATION_TTL`] expires (Finding #2).
/// Multi-board classification is therefore left to [`classify_devices`]'s
/// per-candidate (vid/pid-narrowed) probe, which CAN attribute when vid/pid
/// differ. Pure so the policy is unit-testable.
fn handshake_warm_eligible(candidate_count: usize) -> bool {
    candidate_count <= 1
}

/// Warm the per-path [`CLASSIFICATION_CACHE`] from the handshake result
/// (best-effort, `spec/DEVICE_DISCOVERY.md` §2.4 — single-ping-per-appearance).
///
/// `perform_handshake_with` already pings `QUERY_INFO` once per boot; without
/// this cross-feed, [`classify_devices`] would re-ping on the first status call
/// (2 pings per appearance). To keep it to 1 in the common (single-board) case,
/// the handshake stamps its result into the per-path cache so `classify_devices`
/// reads a warm cache (TTL hit ⇒ no re-ping).
///
/// **Scope guard ([`handshake_warm_eligible`], Finding #2):** the stamp happens
/// ONLY when ≤1 Tier-1 board is present. The handshake is filter-keyed
/// (broadcast, no per-path attribution); with ≥2 boards its single reply cannot
/// be attributed to a specific path, so stamping every enumerated path would
/// mislabel a co-present non-capable (VIA/Vial) board `✓ qmk_notifier` in the
/// picker. Multi-board classification is left to [`classify_devices`]'s
/// per-candidate vid/pid-narrowed probe. No-op in tests (the handshake tests use
/// `MockNotifier` with no real HID, so `enumerate_candidates` finds 0 devices —
/// 0 ≤ 1, but the loop body stamps nothing).
fn warm_cache_from_handshake(kind: DeviceKind) {
    // Gate real HID enumeration out of the test binary. The cargo-test harness
    // runs each test on a worker thread, and `enumerate_candidates` ->
    // `hidapi::HidApi::new()` traps with SIGTRAP when driven off the main thread
    // on macOS (the app always calls this from the main/poll thread, so this is
    // a test-only hazard). The handshake tests use MockNotifier with no real
    // hardware, so there is nothing to warm anyway — making this a true no-op
    // under `cfg!(test)` matches the doc comment above. Production
    // (`cfg!(test) == false`) is unchanged.
    if cfg!(test) {
        return;
    }
    let candidates = enumerate_candidates();
    if !handshake_warm_eligible(candidates.len()) {
        return;
    }
    for c in candidates {
        classification_cache_insert(&c.path, kind.clone());
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
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
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
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
pub fn handshake_action(prev: Option<bool>, now: bool) -> HandshakeAction {
    match (prev, now) {
        (Some(true), false) => HandshakeAction::Loss, // real disconnect
        (p, true) if p != Some(true) => HandshakeAction::Gain, // None→true OR false→true
        _ => HandshakeAction::None,                   // no change OR None→false
    }
}
// NOTE: the Gain arm is GUARDED (`if p != Some(true)`) — the naive `(_, true)
// => Gain` would mis-classify (Some(true), true) (no change) as Gain.

// ===== Capable-board presence lifecycle (Finding #1) =====
// The poll threads used to key the handshake lifecycle on Tier-1 PRESENCE
// (`is_device_connected()` — any `0xFF60` interface). That drops the
// capable-board-lost signal in the headline F13 mixed multi-board case: when a
// capable board is unplugged while a non-capable (VIA/Vial/legacy) board
// remains, Tier-1 presence stays `true` ⇒ no `Loss` ⇒ `HOST_CAPABLE` goes
// stale ⇒ the tray falsely shows "Connected" and a replug of a *different*
// capable board never re-handshakes. The types below key the lifecycle on
// CAPABLE-board presence instead, re-probing only when the Tier-1 path set
// changes (so the hot poll loop never pings on a stable bus — Finding #3).

/// Enumerate the present Tier-1 HID `path`s (the stable hidapi `path` of each
/// `0xFF60`/`0x61` interface matching [`configured_filter`]). Cheap read-only
/// enumeration — never opens a device or sends a report (identical machinery to
/// [`is_device_connected`], just collecting the paths instead of folding to a
/// bool). Used by [`PresenceTracker`] to detect a bus change (plug/unplug) so it
/// re-probes capable presence only then.
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
pub fn tier1_paths() -> Vec<String> {
    enumerate_candidates().into_iter().map(|c| c.path).collect()
}

/// Pure core of [`PresenceTracker::tick`]: decide the handshake action + the new
/// capable flag from a path-set change, the Tier-1 presence, and an optional
/// fresh re-probe result. Split out so the transition logic is unit-testable
/// without HID hardware (the HID-dependent steps — enumerating paths + the
/// per-candidate re-probe — live in [`PresenceTracker::tick`]).
///
/// * `paths_changed` ⇒ the caller ran [`classify_devices`] and supplies its
///   `any(Capable)` fold as `reprobed_capable`.
/// * `!paths_changed` ⇒ the bus is stable; `reprobed_capable` is `None` and the
///   last known flag is reused (a board cannot change firmware without a
///   replug, which changes the path set).
/// * `!tier1_present` ⇒ capable is definitively `false` regardless (no board).
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
fn presence_tick_decision(
    last_capable: bool,
    paths_changed: bool,
    tier1_present: bool,
    reprobed_capable: Option<bool>,
) -> (HandshakeAction, bool) {
    let capable = if !tier1_present {
        false
    } else if paths_changed {
        reprobed_capable.unwrap_or(false)
    } else {
        last_capable
    };
    (handshake_action(Some(last_capable), capable), capable)
}

/// Tracks capable-board presence across poll ticks for the tray poll threads
/// (`src/tray.rs` macOS/Windows, `src/linux_tray.rs`). The poll thread calls
/// [`tick`](Self::tick) each iteration and applies the returned
/// [`HandshakeAction`] (`Gain` ⇒ [`perform_handshake`], `Loss` ⇒
/// [`reset_handshake_state`]).
///
/// **Why capable-keyed, not Tier-1-keyed (Finding #1):** see the section note
/// above — keying on [`is_device_connected`] drops the capable-board-lost signal
/// in the mixed multi-board case (F13 / `DEVICE_DISCOVERY.md` §1). Keying on
/// **capable-board** presence makes a capable-board unplug a real `Loss` (reset
/// + re-arm) and a capable-board (re)plug a real `Gain` (re-handshake).
///
/// **Why path-set-gated re-probing (Finding #3):** recomputing capable presence
/// every tick would ping each board on every [`CLASSIFICATION_TTL`] expiry
/// (~5 s) — the proto-v1 board-layer reset side effect. Instead the capable set
/// is re-probed (via the cache-backed [`classify_devices`]) ONLY when the Tier-1
/// path *set* changes — a physical plug/unplug — so the hot poll loop never
/// pings on a stable bus. Between changes capable presence is stable (firmware
/// cannot change without a replug, which changes the path set).
///
/// [`device_status`] stays consistent because the poll applies the action before
/// reading it: `Loss` ⇒ [`reset_handshake_state`] ⇒ `HOST_CAPABLE=false` ⇒
/// `NoModule`; `Gain` ⇒ [`perform_handshake`] ⇒ `HOST_CAPABLE=true` ⇒
/// `Connected` (after the sub-second handshake; the brief `NoModule` window is
/// the documented transient caveat).
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
/// Bounded extra re-probes a PRESENT-but-not-yet-confirmed-capable board gets
/// after it appears (a plug or a path-set change / hot-swap) before its
/// not-capable classification is trusted. Without this, a SINGLE transient
/// not-capable re-probe right after a hot-swap — the board still enumerating,
/// or the HID transport's stale post-unplug device cache (the `qmk_notifier`
/// crate only rebuilds it on a write-failure / key-change) — latches the board
/// `NoModule` until a restart, because the path set is then stable and no later
/// tick re-probes. At the ~3s poll cadence this is ~18s of grace; a physical
/// replug re-arms it.
const PRESENCE_REPROBE_BUDGET: u32 = 6;

/// Pure: should this poll tick re-probe capable presence? Re-probe when the
/// Tier-1 path set changed (a plug/unplug/hot-swap) OR when a present board is
/// still within its post-plug retry budget but hasn't been confirmed capable.
/// Split out so the retry policy is unit-testable without HID hardware.
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
fn reprobe_needed(paths_changed: bool, tier1_present: bool, last_capable: bool, budget: u32) -> bool {
    tier1_present && (paths_changed || (budget > 0 && !last_capable))
}

pub struct PresenceTracker {
    last_paths: Vec<String>,
    last_capable: bool,
    /// Remaining grace re-probes for a present board that hasn't been confirmed
    /// capable. Reset to [`PRESENCE_REPROBE_BUDGET`] on any path-set change;
    /// decremented each re-probe; zeroed once a board is confirmed capable.
    reprobe_budget: u32,
}

#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
impl PresenceTracker {
    /// Seed from the live bus + the startup-handshake result. Construct on the
    /// poll thread right after the runner's startup handshake so `last_capable`
    /// reflects it (the first tick then sees no transition ⇒ no spurious action).
    pub fn new() -> Self {
        let last_paths = tier1_paths();
        let last_capable = host_capable();
        // If a board is present at construction but the startup handshake did not
        // confirm it capable (e.g. it was still enumerating), arm the retry budget
        // so the poll thread re-probes instead of trusting that single result.
        let reprobe_budget = if !last_paths.is_empty() && !last_capable {
            PRESENCE_REPROBE_BUDGET
        } else {
            0
        };
        Self {
            last_paths,
            last_capable,
            reprobe_budget,
        }
    }

    /// One poll tick. Enumerates the Tier-1 paths; if the path *set* changed
    /// since the last tick, re-probes capable presence via [`classify_devices`]
    /// and folds to `any(Capable)`. Returns the [`HandshakeAction`] to apply.
    pub fn tick(&mut self, verbose: bool) -> HandshakeAction {
        let paths = tier1_paths();
        let tier1_present = !paths.is_empty();
        let paths_changed = paths != self.last_paths;

        // A path-set change (plug/unplug/hot-swap) re-arms the retry budget for
        // a present board, so a transient not-capable re-probe doesn't strand it.
        if paths_changed {
            self.reprobe_budget = if tier1_present { PRESENCE_REPROBE_BUDGET } else { 0 };
        }

        // Re-probe on a path-set change, OR while a present board is still within
        // its post-plug grace window but hasn't been confirmed capable. This is
        // the hot-swap recovery the single-shot path-change re-probe missed: a
        // transient first miss is retried instead of latching NoModule.
        let reprobe_this_tick = reprobe_needed(
            paths_changed,
            tier1_present,
            self.last_capable,
            self.reprobe_budget,
        );
        let reprobed = if reprobe_this_tick {
            self.reprobe_budget = self.reprobe_budget.saturating_sub(1);
            Some(
                classify_devices(verbose)
                    .iter()
                    .any(|d| matches!(d.kind, DeviceKind::Capable { .. })),
            )
        } else {
            None
        };

        // `presence_tick_decision`'s 2nd arg means "the caller re-probed this
        // tick" (use the fresh result rather than last_capable): pass the combined
        // flag so a grace-window retry's result is honoured exactly like a
        // path-change re-probe.
        let (action, capable) =
            presence_tick_decision(self.last_capable, reprobe_this_tick, tier1_present, reprobed);
        if capable {
            self.reprobe_budget = 0; // confirmed — stop pinging (Finding #3 preserved)
        }
        self.last_capable = capable;
        self.last_paths = paths;
        action
    }
}

impl Default for PresenceTracker {
    fn default() -> Self {
        Self::new()
    }
}

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
            let notifier = notifier.lock().unwrap_or_else(|e| e.into_inner());
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
        let notifier = notifier.lock().unwrap_or_else(|e| e.into_inner());
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
    // P3.M1.T1.S2: per-`send_command` filter tuples `(vid, pid, usage_page, usage)`
    // so classify_candidates tests can assert the per-candidate vid/pid NARROWING
    // (the chosen per-candidate mechanism — the crate has no per-path send).
    // Records tuples, NOT `DeviceFilter`, so no `Clone`/`PartialEq` derive is
    // added to the production struct purely for test convenience.
    type MockSendCommandFilter = (Option<u16>, Option<u16>, u16, u16);
    static MOCK_SEND_COMMAND_FILTERS: Lazy<StdMutex<Vec<MockSendCommandFilter>>> =
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
        MOCK_SEND_COMMAND_FILTERS.lock().unwrap().clear();
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

        /// P3.M1.T1.S2: the per-`send_command` filter tuples `(vid, pid,
        /// usage_page, usage)`, in call order — lets a classify_candidates
        /// test assert each candidate's ping was narrowed to its vid/pid.
        fn get_send_command_filters() -> Vec<(Option<u16>, Option<u16>, u16, u16)> {
            MOCK_SEND_COMMAND_FILTERS.lock().unwrap().clone()
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
            filter: &DeviceFilter,
        ) -> Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>> {
            MOCK_SEND_COMMAND_CALLS
                .lock()
                .unwrap()
                .push(command.clone());
            // P3.M1.T1.S2: record the per-call filter tuple so the classify
            // tests can assert the per-candidate vid/pid narrowing.
            MOCK_SEND_COMMAND_FILTERS.lock().unwrap().push((
                filter.vendor_id,
                filter.product_id,
                filter.usage_page,
                filter.usage,
            ));
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
    fn test_handshake_action_loss_on_seeded_true_to_false() {
        // PresenceTracker seeds `last_capable` from `host_capable()` (the startup
        // handshake result). When a capable startup is followed by the capable
        // board vanishing, presence_tick_decision yields handshake_action(
        // Some(true), false) — which MUST be Loss so reset_handshake_state()
        // fires, HAS_HANDSHAKED clears, and the subsequent reconnect re-runs the
        // handshake (re-sending SET_OS). (Also asserted in
        // test_handshake_action_transitions; restated here to pin the invariant
        // PresenceTracker depends on.)
        assert_eq!(handshake_action(Some(true), false), HandshakeAction::Loss);
    }

    // ===== PresenceTracker / presence_tick_decision (Finding #1) =====
    // Pure transition logic for the capable-board-keyed handshake lifecycle.
    // No HID needed: presence_tick_decision takes the path-set-change flag, the
    // Tier-1 presence, and an optional fresh re-probe result. These encode the
    // headline F13 mixed multi-board fix (a capable board unplugging while a
    // non-capable Tier-1 board remains is a real Loss).

    #[test]
    fn test_presence_tick_capable_unplug_with_non_capable_remaining_is_loss() {
        // Finding #1 headline: capable board A unplugs while pure-VIA board B
        // remains. last_capable was true (A was capable). The path set changed
        // (A's path gone) ⇒ re-probe returns any(Capable)=false (only B left).
        // tier1_present stays true (B is still a 0xFF60 interface). Decision MUST
        // be Loss so reset_handshake_state() clears HOST_CAPABLE + HAS_HANDSHAKED
        // (otherwise the tray would falsely show "Connected" and a replug of a
        // different capable board would never re-handshake).
        let (action, capable) = presence_tick_decision(true, true, true, Some(false));
        assert_eq!(action, HandshakeAction::Loss);
        assert!(!capable);
    }

    #[test]
    fn test_presence_tick_capable_replug_different_board_is_gain() {
        // After the Loss above, a *different* capable board A' is plugged (path
        // set changes again). last_capable is now false; re-probe finds a Capable
        // board ⇒ Gain ⇒ perform_handshake re-runs (HAS_HANDSHAKED was cleared by
        // the Loss's reset). This is the replug-resumes-without-restart guarantee.
        let (action, capable) = presence_tick_decision(false, true, true, Some(true));
        assert_eq!(action, HandshakeAction::Gain);
        assert!(capable);
    }

    #[test]
    fn test_presence_tick_stable_bus_no_reprobe_no_action() {
        // Stable bus (no plug/unplug): paths_changed=false ⇒ reuse last_capable,
        // no re-probe (the hot poll loop never pings on a stable bus — Finding #3).
        // Whether capable or not, a stable bus ⇒ None.
        assert_eq!(
            presence_tick_decision(true, false, true, None),
            (HandshakeAction::None, true)
        );
        assert_eq!(
            presence_tick_decision(false, false, true, None),
            (HandshakeAction::None, false)
        );
    }

    #[test]
    fn test_presence_tick_all_unplugged_forces_loss() {
        // The whole bus goes empty (last Tier-1 board unplugs). tier1_present=false
        // forces capable=false regardless of paths_changed, so a previously-capable
        // bus records Loss. (paths_changed would be true here, but the empty-bus
        // arm dominates — no re-probe is needed/possible with no candidates.)
        assert_eq!(
            presence_tick_decision(true, true, false, None),
            (HandshakeAction::Loss, false)
        );
        // And a stable-empty bus that was already not-capable stays None.
        assert_eq!(
            presence_tick_decision(false, false, false, None),
            (HandshakeAction::None, false)
        );
    }

    #[test]
    fn test_presence_tick_boot_reprobe_capable_is_none_when_already_capable() {
        // At boot PresenceTracker::new seeds last_capable from host_capable(). If
        // the startup handshake already found a capable board, the first tick's
        // re-probe (path set empty→non-empty) returning capable=true matches the
        // seed ⇒ None (no spurious re-handshake; perform_handshake is itself
        // idempotent via HAS_HANDSHAKED, but the action is cleanly None here too).
        assert_eq!(
            presence_tick_decision(true, true, true, Some(true)),
            (HandshakeAction::None, true)
        );
    }

    #[test]
    fn test_presence_tick_reprobe_not_capable_after_not_capable_is_none() {
        // Mirror: boot with a non-capable Tier-1 board (handshake left
        // HOST_CAPABLE=false). First-tick re-probe confirms not-capable ⇒ None.
        assert_eq!(
            presence_tick_decision(false, true, true, Some(false)),
            (HandshakeAction::None, false)
        );
    }

    // ===== reprobe_needed: the hot-swap retry policy (PresenceTracker grace window) =====
    // A board that is PRESENT but classified not-capable gets a bounded number of
    // re-probes after it appears, so a single transient not-capable result right
    // after a plug/hot-swap doesn't latch NoModule until restart.

    #[test]
    fn test_reprobe_needed_on_path_change() {
        // A plug/hot-swap (path set changed) with a board present ⇒ always re-probe,
        // regardless of last_capable or budget.
        assert!(reprobe_needed(true, true, false, 0));
        assert!(reprobe_needed(true, true, true, 0));
    }

    #[test]
    fn test_reprobe_needed_grace_window_for_unconfirmed_present_board() {
        // Stable bus, board present, not yet capable, budget remaining ⇒ keep
        // re-probing (the hot-swap recovery: a transient first miss is retried).
        assert!(reprobe_needed(false, true, false, 3));
        // Budget exhausted ⇒ stop re-probing (trust the not-capable result).
        assert!(!reprobe_needed(false, true, false, 0));
        // Already capable ⇒ no need to keep probing (Finding #3 preserved).
        assert!(!reprobe_needed(false, true, true, 3));
    }

    #[test]
    fn test_reprobe_needed_no_board_present() {
        // No Tier-1 board present ⇒ never re-probe, regardless of other inputs.
        assert!(!reprobe_needed(true, false, false, 6));
        assert!(!reprobe_needed(false, false, false, 6));
    }

    #[test]
    fn test_hot_swap_transient_miss_then_capable_is_gain() {
        // The headline fix: a capable board hot-swapped in. Tick 1 re-probes and
        // TRANSIENTLY misses (board still enumerating / stale cache) ⇒ not-capable
        // ⇒ Loss. Tick 2 (stable path, but grace budget armed) re-probes again and
        // now sees Capable ⇒ the decision must be Gain so perform_handshake runs.
        // Before the grace window, tick 2 would have seen paths_changed=false ⇒
        // reused the false result ⇒ None ⇒ stranded NoModule.

        // Tick 1: paths changed, present, re-probe said not-capable.
        let (action1, capable1) = presence_tick_decision(true, true, true, Some(false));
        assert_eq!(action1, HandshakeAction::Loss);
        assert!(!capable1); // last_capable is now false

        // Tick 2: stable path (paths_changed=false) but the tracker re-probed
        // again within its grace window and this time found Capable. Passing
        // reprobe_this_tick=true makes the decision use the fresh result.
        let (action2, capable2) = presence_tick_decision(false, true, true, Some(true));
        assert_eq!(action2, HandshakeAction::Gain); // re-handshake fires
        assert!(capable2);
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
        // #4 clamps the sweep to MAX_HOST_CALLBACKS indices; Finding #1 then
        // retries each transient-misparse index up to QUERY_CALLBACK_MAX_ATTEMPTS
        // (the mock's default `Ack` is exactly such a transient, so every index
        // retries once). The sweep still visits at most MAX_HOST_CALLBACKS
        // distinct indices — the retry only re-queries a single index, never
        // grows the sweep past the cap.
        assert!(
            query_callbacks <= (MAX_HOST_CALLBACKS as usize) * QUERY_CALLBACK_MAX_ATTEMPTS,
            "sweep+retry must stay bounded by MAX_HOST_CALLBACKS * QUERY_CALLBACK_MAX_ATTEMPTS, got {}",
            query_callbacks
        );
        let distinct_indices = calls
            .iter()
            .filter_map(|c| match c {
                qmk_notifier::RunCommand::QueryCallback(i) => Some(*i),
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            distinct_indices.len(),
            MAX_HOST_CALLBACKS as usize,
            "sweep must clamp to MAX_HOST_CALLBACKS distinct indices, not trust callback_count"
        );
    }

    /// Finding #1: a transient mis-parse of a `QUERY_CALLBACK` reply (the
    /// firmware/crate surfaces a generic `Ack` where a `CallbackName` was
    /// expected) is retried once on that single index, recovering the name
    /// without re-running the whole sweep. This pins the qmkonnect-side
    /// hardening for the real-hardware transient from the validation report
    /// (1-in-~20 occurrence that otherwise left `CALLBACK_NAMES` empty for the
    /// session).
    #[test]
    fn test_handshake_sweep_retries_transient_callback_misparse() {
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
            qmk_notifier::CommandResponse::Ack { ok: true }, // SET_OS
            // First QUERY_CALLBACK(0) transiently mis-parses as a generic Ack
            // (the exact signature from the real-hardware validation run):
            qmk_notifier::CommandResponse::Ack { ok: true },
            // Retry clears it — the firmware answers properly:
            qmk_notifier::CommandResponse::CallbackName {
                index: 0,
                name: Some("vim_lazy".into()),
            },
        ]);
        perform_handshake(false);
        assert!(host_capable());
        // The name WAS mapped despite the transient (without the retry this
        // session would silently no-op every `vim_lazy` host-rule toggle).
        assert_eq!(callback_names().get("vim_lazy"), Some(&0));
        // Exactly two QUERY_CALLBACK(0) round-trips went out: initial + retry.
        let cb0_calls = MockNotifier::get_send_command_calls()
            .iter()
            .filter(|c| matches!(c, qmk_notifier::RunCommand::QueryCallback(0)))
            .count();
        assert_eq!(cb0_calls, QUERY_CALLBACK_MAX_ATTEMPTS);
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
    #[cfg_attr(
        target_os = "macos",
        ignore = "live HID enumeration (is_device_connected) traps with SIGTRAP when the cargo-test harness runs it off the main thread on macOS; the present=false dominance logic it asserts is covered deterministically by test_classify_device_status_truth_table"
    )]
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
    fn kind_label_matches_spec() {
        // §8 labels: Capable ⇒ "qmk_notifier", NotQmkNotifier ⇒ "qmk-only".
        use super::{kind_label, DeviceKind};
        let capable = DeviceKind::Capable {
            proto_ver: 2,
            feature_flags: 1,
            callback_count: 0,
            board_rules_present: false,
        };
        assert_eq!(kind_label(&capable), "qmk_notifier");
        assert_eq!(kind_label(&DeviceKind::NotQmkNotifier), "qmk-only");
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
        CLASSIFICATION_CACHE.lock().unwrap().insert(
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
        assert_eq!(
            DeviceKind::NotQmkNotifier,
            DeviceKind::NotQmkNotifier.clone()
        );
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

    // ===== classify_devices (P3.M1.T1.S2) tests =====
    // The Tier-2 per-candidate capability classifier. Pure tests (A) drive
    // `classify_reply` directly; MockNotifier tests (B/C/D) use the standard
    // handshake-test setup idiom (`reset_test_state` + `reset_handshake_state`
    // + `set_notifier(MockNotifier::new())`) + `classification_cache_clear()`
    // (the static outlives tests; crate tests are single-threaded per AGENTS.md).
    // The MockNotifier's FIFO `MOCK_RESPONSES` queue gives per-candidate ordering
    // (candidate i ⇔ response i); `get_send_command_filters()` asserts the
    // per-candidate vid/pid NARROWING.

    /// Build a Tier-1 `Candidate` for the tests (the four fields + the cache key).
    fn candidate(path: &str, vid: u16, pid: u16) -> Candidate {
        Candidate {
            path: path.to_string(),
            vendor_id: vid,
            product_id: pid,
            product_name: None,
            usage_page: 0xFF60,
            usage: 0x61,
        }
    }

    // ── A. classify_reply (pure — no mock, 6 tests) ──

    #[test]
    fn test_classify_reply_info_proto2_capable() {
        // §2.2: Info{proto_ver:2} ⇒ Capable carrying all four fields.
        let resp: Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>> =
            Ok(qmk_notifier::CommandResponse::Info {
                proto_ver: 2,
                feature_flags: 0x01,
                callback_count: 3,
                board_rules_present: true,
            });
        assert_eq!(
            classify_reply(resp),
            DeviceKind::Capable {
                proto_ver: 2,
                feature_flags: 0x01,
                callback_count: 3,
                board_rules_present: true,
            }
        );
    }

    #[test]
    fn test_classify_reply_info_proto2_no_feature_bit_still_capable() {
        // G2: the classifier does NOT gate on feature_flags & 0x01. A
        // proto-v2 board with no APPLY_HOST_CONTEXT bit is STILL Capable
        // (the handshake gates; the classifier records the flags).
        let resp: Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>> =
            Ok(qmk_notifier::CommandResponse::Info {
                proto_ver: 2,
                feature_flags: 0x00,
                callback_count: 0,
                board_rules_present: false,
            });
        assert_eq!(
            classify_reply(resp),
            DeviceKind::Capable {
                proto_ver: 2,
                feature_flags: 0x00,
                callback_count: 0,
                board_rules_present: false,
            }
        );
    }

    #[test]
    fn test_classify_reply_info_proto1_notqmk() {
        // The literal `proto_ver: 2` arm does NOT match a proto-v1 reply ⇒ falls
        // to `_` ⇒ NotQmkNotifier (replied but not the typed-command protocol).
        let resp: Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>> =
            Ok(qmk_notifier::CommandResponse::Info {
                proto_ver: 1,
                feature_flags: 0,
                callback_count: 0,
                board_rules_present: false,
            });
        assert_eq!(classify_reply(resp), DeviceKind::NotQmkNotifier);
    }

    #[test]
    fn test_classify_reply_legacy_notqmk() {
        let resp: Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>> =
            Ok(qmk_notifier::CommandResponse::Legacy { matched: true });
        assert_eq!(classify_reply(resp), DeviceKind::NotQmkNotifier);
    }

    #[test]
    fn test_classify_reply_timeout_notqmk() {
        let resp: Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>> =
            Ok(qmk_notifier::CommandResponse::Timeout);
        assert_eq!(classify_reply(resp), DeviceKind::NotQmkNotifier);
    }

    #[test]
    fn test_classify_reply_ack_notqmk() {
        // Ack is the empty-queue default the mock returns — not a capable reply.
        let resp: Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>> =
            Ok(qmk_notifier::CommandResponse::Ack { ok: true });
        assert_eq!(classify_reply(resp), DeviceKind::NotQmkNotifier);
    }

    #[test]
    fn test_classify_reply_err_notqmk() {
        // A transport error degrades to NotQmkNotifier (no board harmed).
        let resp: Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>> =
            Err("device error".into());
        assert_eq!(classify_reply(resp), DeviceKind::NotQmkNotifier);
    }

    // ── B. classify_candidates (MockNotifier, 5 tests — the core) ──

    #[test]
    fn test_classify_candidates_capable() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        classification_cache_clear();

        let c = candidate("p-cap", 0x1234, 0x5678);
        MockNotifier::set_mock_responses(vec![qmk_notifier::CommandResponse::Info {
            proto_ver: 2,
            feature_flags: 0x01,
            callback_count: 2,
            board_rules_present: true,
        }]);
        let result = classify_candidates(vec![c], false);

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].kind,
            DeviceKind::Capable {
                proto_ver: 2,
                feature_flags: 0x01,
                callback_count: 2,
                board_rules_present: true,
            }
        );
        // Exactly one ping (cache miss).
        let calls = MockNotifier::get_send_command_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], qmk_notifier::RunCommand::QueryInfo);
        // The filter was narrowed to this candidate's vid/pid (G4 mechanism).
        let filters = MockNotifier::get_send_command_filters();
        assert_eq!(filters.len(), 1);
        assert_eq!(
            filters[0],
            (Some(0x1234u16), Some(0x5678u16), 0xFF60u16, 0x61u16)
        );
    }

    #[test]
    fn test_classify_candidates_mixed() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        classification_cache_clear();

        // 3 candidates: distinct vid/pid so the narrowing is per-candidate.
        let cands = vec![
            candidate("p-a", 0x1111, 0x2222),
            candidate("p-b", 0x3333, 0x4444),
            candidate("p-c", 0x5555, 0x6666),
        ];
        // FIFO queue gives per-candidate ordering (candidate i ⇔ response i).
        MockNotifier::set_mock_responses(vec![
            qmk_notifier::CommandResponse::Info {
                proto_ver: 2,
                feature_flags: 0x01,
                callback_count: 0,
                board_rules_present: false,
            },
            qmk_notifier::CommandResponse::Legacy { matched: true },
            qmk_notifier::CommandResponse::Timeout,
        ]);
        let result = classify_candidates(cands, false);

        assert_eq!(result.len(), 3);
        assert_eq!(
            result[0].kind,
            DeviceKind::Capable {
                proto_ver: 2,
                feature_flags: 0x01,
                callback_count: 0,
                board_rules_present: false,
            }
        );
        assert_eq!(result[1].kind, DeviceKind::NotQmkNotifier);
        assert_eq!(result[2].kind, DeviceKind::NotQmkNotifier);
        // 3 pings, each narrowed to its own vid/pid.
        assert_eq!(MockNotifier::get_send_command_calls().len(), 3);
        let filters = MockNotifier::get_send_command_filters();
        assert_eq!(filters.len(), 3);
        assert_eq!(
            filters[0],
            (Some(0x1111u16), Some(0x2222u16), 0xFF60u16, 0x61u16)
        );
        assert_eq!(
            filters[1],
            (Some(0x3333u16), Some(0x4444u16), 0xFF60u16, 0x61u16)
        );
        assert_eq!(
            filters[2],
            (Some(0x5555u16), Some(0x6666u16), 0xFF60u16, 0x61u16)
        );
    }

    #[test]
    fn test_classify_candidates_cache_hit_skips_ping() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        classification_cache_clear();

        let c = candidate("p-hit", 0x1234, 0x5678);
        // Pre-warm the cache: the probe must NOT re-ping (TTL hit).
        classification_cache_insert(
            &c.path,
            DeviceKind::Capable {
                proto_ver: 2,
                feature_flags: 0x01,
                callback_count: 0,
                board_rules_present: false,
            },
        );
        // EMPTY response queue — a ping here would pop the default Ack
        // (NotQmkNotifier) and clobber the cached Capable, so the empty queue
        // + the call-count assertion together prove no ping happened.
        let result = classify_candidates(vec![c], false);

        assert_eq!(
            result[0].kind,
            DeviceKind::Capable {
                proto_ver: 2,
                feature_flags: 0x01,
                callback_count: 0,
                board_rules_present: false,
            }
        );
        // Cache hit ⇒ NO ping at all.
        assert!(MockNotifier::get_send_command_calls().is_empty());
        assert!(MockNotifier::get_send_command_filters().is_empty());
    }

    #[test]
    fn test_classify_candidates_cache_miss_pings_and_caches() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        classification_cache_clear();

        let c = candidate("p-miss", 0x1234, 0x5678);
        // First call: cache miss, queue a Capable reply ⇒ pings once + caches.
        MockNotifier::set_mock_responses(vec![qmk_notifier::CommandResponse::Info {
            proto_ver: 2,
            feature_flags: 0x01,
            callback_count: 0,
            board_rules_present: false,
        }]);
        let first = classify_candidates(vec![candidate("p-miss", 0x1234, 0x5678)], false);
        assert_eq!(
            first[0].kind,
            DeviceKind::Capable {
                proto_ver: 2,
                feature_flags: 0x01,
                callback_count: 0,
                board_rules_present: false,
            }
        );
        assert_eq!(MockNotifier::get_send_command_calls().len(), 1);

        // Second call: EMPTY queue, cache is warm ⇒ still Capable, NO new ping.
        let second = classify_candidates(vec![c], false);
        assert_eq!(first[0].kind, second[0].kind);
        assert_eq!(MockNotifier::get_send_command_calls().len(), 1); // unchanged
    }

    #[test]
    fn test_classify_candidates_ttl_re_ping() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        classification_cache_clear();

        let c = candidate("p-ttl", 0x1234, 0x5678);
        // Seed the cache with a Capable kind, then age the stamp past TTL so the
        // next get is a miss (same idiom as test_classification_cache_ttl_expiry).
        classification_cache_insert(
            &c.path,
            DeviceKind::Capable {
                proto_ver: 2,
                feature_flags: 0x01,
                callback_count: 0,
                board_rules_present: false,
            },
        );
        CLASSIFICATION_CACHE.lock().unwrap().insert(
            c.path.clone(),
            (
                DeviceKind::Capable {
                    proto_ver: 2,
                    feature_flags: 0x01,
                    callback_count: 0,
                    board_rules_present: false,
                },
                Instant::now() - CLASSIFICATION_TTL - Duration::from_millis(1),
            ),
        );
        // The re-ping will pop this Timeout ⇒ NotQmkNotifier (new result cached).
        MockNotifier::set_mock_responses(vec![qmk_notifier::CommandResponse::Timeout]);
        let result = classify_candidates(vec![c], false);

        assert_eq!(result[0].kind, DeviceKind::NotQmkNotifier);
        // TTL expired ⇒ exactly one re-ping.
        assert_eq!(MockNotifier::get_send_command_calls().len(), 1);
    }

    // ── C. invalidate_absent_cache_entries (pure, 1 test) ──

    #[test]
    fn test_invalidate_drops_absent_paths() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        classification_cache_clear();

        classification_cache_insert("p1", DeviceKind::NotQmkNotifier);
        classification_cache_insert("p2", DeviceKind::NotQmkNotifier);
        // Only p1 is present in the candidate set ⇒ p2 must be evicted.
        invalidate_absent_cache_entries(&[candidate("p1", 0x1234, 0x5678)]);

        let map = CLASSIFICATION_CACHE.lock().unwrap();
        assert!(map.contains_key("p1"));
        assert!(!map.contains_key("p2"));
    }

    // ── D. classify_devices smoke (the env-dependent shell, 1 test) ──

    #[test]
    #[cfg_attr(
        target_os = "macos",
        ignore = "classify_devices -> enumerate_candidates -> hidapi::HidApi::new() traps with SIGTRAP off the main thread under the cargo-test harness on macOS; the wiring it smoke-tests is env-dependent by design"
    )]
    fn test_classify_devices_smoke_returns_vec() {
        reset_test_state();
        reset_handshake_state();
        set_notifier(Box::new(MockNotifier::new()));
        classification_cache_clear();

        // enumerate_candidates touches real HID — env-dependent. Just prove the
        // enumerate → invalidate → classify wiring compiles + runs without panic.
        // Do NOT assert a count (0 on a box with no QMK board, N on one with N).
        let result = classify_devices(false);
        let _ = result.len();
    }

    // ── E. handshake_warm_eligible (Finding #2 policy, pure) ──

    #[test]
    fn test_handshake_warm_eligible_single_board_only() {
        // The broadcast handshake can attribute its single reply to a path ONLY
        // when ≤1 Tier-1 board is present (broadcast == unicast). With ≥2 boards
        // it must NOT warm-stamp (it would mislabel a co-present non-capable
        // board `✓ qmk_notifier` in the picker until the TTL expires).
        assert!(handshake_warm_eligible(0)); // no board: stamp loops over nothing
        assert!(handshake_warm_eligible(1)); // single board: broadcast == unicast
        assert!(!handshake_warm_eligible(2)); // mixed: cannot attribute
        assert!(!handshake_warm_eligible(3));
    }
}

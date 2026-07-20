use crate::core::types::WindowInfo;
use once_cell::sync::Lazy;
use std::error::Error;
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
struct DebounceState {
    /// `None` until the first notification has actually been sent.
    last_sent_time: Option<Instant>,
    /// Latest message queued for a debounced send.
    pending: Option<PendingMessage>,
    verbose: bool,
    /// Debounce window; 0 disables coalescing (every change sends immediately).
    /// Loaded from config at init.
    interval: Duration,
}

static STATE: Lazy<Mutex<DebounceState>> = Lazy::new(|| {
    Mutex::new(DebounceState {
        last_sent_time: None,
        pending: None,
        verbose: false,
        interval: Duration::from_millis(crate::core::configured_debounce_ms()),
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
                let target = last + state.interval;
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
            // `pm` carries the formatted payload (sent below) AND the originating
            // WindowInfo. P4.M3.T1.S1 consumes `pm.window_info` here to evaluate
            // rules.toml and emit APPLY_HOST_CONTEXT alongside the string send.
            let message = pm.payload; // partial move -> String; pm.window_info remains for P4.M3.T1.S1

            if verbose {
                let sanitized = message.replace('\x1D', "|");
                println!(
                    "[{}ms] Notified QMK (debounced): {}",
                    crate::core::now_ms(),
                    sanitized
                );
            }

            #[cfg(test)]
            println!("Sending debounced notification: {}", message);

            let notifier = get_notifier();
            let notifier = notifier.lock().unwrap();
            let _len = message.len();
            let _t0 = Instant::now();
            let _res = notifier.notify(message);
            let _send_ms = _t0.elapsed().as_millis();
            if verbose {
                eprintln!(
                    "[{}ms] send took {}ms ({} bytes)",
                    crate::core::now_ms(),
                    _send_ms,
                    _len
                );
            }
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
        let interval = state.interval;
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
        if verbose {
            let sanitized = message.replace('\x1D', "|");
            println!(
                "[{}ms] Notified QMK (immediate): {}",
                crate::core::now_ms(),
                sanitized
            );
        }

        #[cfg(test)]
        println!("Sending notification immediately: {}", message);

        let notifier = get_notifier();
        let notifier = notifier.lock().unwrap();
        let _len = message.len();
        let _t0 = Instant::now();
        let _res = notifier.notify(message);
        let _send_ms = _t0.elapsed().as_millis();
        if verbose {
            eprintln!(
                "[{}ms] send took {}ms ({} bytes)",
                crate::core::now_ms(),
                _send_ms,
                _len
            );
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::WindowInfo;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex as StdMutex;

    // Use a shared global mock for testing
    static MOCK_CALL_COUNT: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));
    static MOCK_LAST_MESSAGE: Lazy<StdMutex<Option<String>>> = Lazy::new(|| StdMutex::new(None));
    static MOCK_SEND_COMMAND_CALLS: Lazy<StdMutex<Vec<qmk_notifier::RunCommand>>> =
        Lazy::new(|| StdMutex::new(Vec::new()));

    fn reset_global_mock() {
        MOCK_CALL_COUNT.store(0, Ordering::SeqCst);
        *MOCK_LAST_MESSAGE.lock().unwrap() = None;
        MOCK_SEND_COMMAND_CALLS.lock().unwrap().clear();
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
            Ok(qmk_notifier::CommandResponse::Ack { ok: true })
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
            state.interval = Duration::from_millis(50);
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
        STATE.lock().unwrap().interval = Duration::from_millis(200);

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
        STATE.lock().unwrap().interval = Duration::from_millis(200);

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
        STATE.lock().unwrap().interval = Duration::from_secs(10);

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
        STATE.lock().unwrap().interval = Duration::from_millis(50);
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
            qmk_notifier::RunCommand::ApplyHostContext { layer: Some(224), .. }
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
}

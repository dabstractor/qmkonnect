#![cfg(all(target_os = "linux", feature = "hyprland"))]
use crate::core::notifier;
use crate::core::types::WindowInfo;
use crate::platforms::WindowMonitor;
use hyprland::{
    data::{Client, Clients},
    event_listener::{EventListener, WorkspaceEventData},
    shared::HyprData,
    shared::HyprDataActiveOptional,
};
use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime},
};

/// Initial delay (ms) before the first reconnect attempt.
const INITIAL_RECONNECT_MS: u64 = 100;
/// Maximum reconnect backoff (ms).
const MAX_RECONNECT_MS: u64 = 10_000;
/// A listener that stayed up at least this long is treated as a stable
/// connection; on its loss the backoff is reset to the initial value (#7).
const STABLE_CONNECTION_THRESHOLD: Duration = Duration::from_secs(5);
/// How long [`hyprland_socket_is_live`] will block waiting for a single
/// Hyprland IPC socket to accept a connection. See that function for why a
/// bound is needed at all.
const SOCKET_PROBE_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(PartialEq, Debug, Clone)]
struct WindowState {
    app_class: String,
    title: String,
}

pub struct HyprlandMonitor {
    last_window_state: Arc<Mutex<Option<WindowState>>>,
    verbose: bool,
}

impl HyprlandMonitor {
    pub fn new(verbose: bool) -> Self {
        Self {
            last_window_state: Arc::new(Mutex::new(None)),
            verbose,
        }
    }
}

impl WindowMonitor for HyprlandMonitor {
    fn platform_name(&self) -> &str {
        "Hyprland"
    }

    fn start(&mut self) -> Result<(), Box<dyn Error>> {
        // Wait for Hyprland to become available (handles boot race condition)
        wait_for_hyprland(self.verbose)?;

        if self.verbose {
            println!("Starting Hyprland window monitor");
        }

        let fn_start = Instant::now();
        let mut delay_ms = INITIAL_RECONNECT_MS;

        // Optional periodic active-window poll. The IPC event stream can miss
        // (or be late on) focus changes — notably when a scratchpad is
        // dismissed via `movetoworkspacesilent`, where Hyprland refocuses the
        // underlying window implicitly and the `activewindow` event lags or
        // never fires. Polling the live active-window state on a short cadence
        // corrects any such drift. `poll_window_state` dedups against
        // `last_window_state`, so steady-state polls are no-ops (one cheap IPC
        // call each). Disabled when `poll_interval_ms == 0` (the default).
        //
        // HOT-CONFIG (PRD §7): the interval is re-read from `configured_timing()`
        // on EVERY iteration, so editing `config.toml` takes effect on the next
        // tick — including 0→N (enable), N→0 (disable), and N→M (cadence change)
        // — with no restart. The thread is always spawned (even when polling is
        // initially disabled) so a live 0→N edit can start it; while disabled it
        // just sleeps on a slow re-check cadence.
        let lws = Arc::clone(&self.last_window_state);
        let verbose = self.verbose;
        thread::spawn(move || {
            if verbose {
                println!(
                    "[{}ms] periodic active-window poll thread started (cadence re-read from config each tick; dormant when poll_interval_ms=0)",
                    crate::core::now_ms()
                );
            }
            loop {
                let poll_interval_ms = crate::core::configured_timing().1;
                if poll_interval_ms == 0 {
                    // Polling disabled in config. Sleep on a slow cadence and
                    // re-check so a live 0→N edit re-enables polling without a
                    // restart (the thread stays alive on purpose rather than
                    // breaking, which would make a later 0→N edit a no-op).
                    thread::sleep(Duration::from_secs(1));
                    continue;
                }
                thread::sleep(Duration::from_millis(poll_interval_ms));
                if let Err(err) = poll_window_state(&lws, verbose) {
                    eprintln!("Error in periodic poll: {}", err);
                }
            }
        });

        loop {
            // Re-resolve the live Hyprland instance on every reconnect attempt.
            // A listener failure almost always means the socket we were using
            // went away (compositor crashed/restarted), and the replacement
            // gets a *new* signature — so dialing the stale one would never
            // recover. `check_hyprland_environment` re-points
            // $HYPRLAND_INSTANCE_SIGNATURE at a live socket, or returns Err if
            // none is up yet (we then back off and retry).
            if let Err(e) = check_hyprland_environment() {
                if self.verbose {
                    println!(
                        "Reconnect: no live Hyprland socket yet ({}ms): {}",
                        crate::core::now_ms(),
                        e
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                delay_ms = std::cmp::min(delay_ms * 3, MAX_RECONNECT_MS);
                continue;
            }

            // Create a new event listener for each attempt
            let mut listener = EventListener::new();
            let verbose = self.verbose;
            let last_window_state = Arc::clone(&self.last_window_state);

            // Set up the window change handler
            let lwc = Arc::clone(&last_window_state);
            listener.add_active_window_changed_handler(move |_| {
                if let Err(err) = handle_window_state_change(&lwc, verbose) {
                    eprintln!("Error handling window change: {}", err);
                }
            });

            // Add workspace change handler
            let lws = Arc::clone(&last_window_state);
            listener.add_workspace_changed_handler(move |workspace_event| {
                if let Err(err) = handle_workspace_change(workspace_event, &lws, verbose) {
                    eprintln!("Error handling workspace change: {}", err);
                }
            });

            // Add window closed handler
            let lwc = Arc::clone(&last_window_state);
            listener.add_window_closed_handler(move |_| {
                if let Err(err) = handle_window_state_change(&lwc, verbose) {
                    eprintln!("Error handling window close: {}", err);
                }
            });

            // Layer surface (e.g. scratchpad) handlers. We rely on events rather
            // than a permanent 100ms poller (#8): each layer event queries the
            // active window immediately, then fires a short bounded poll burst to
            // absorb the timing gap where focus hasn't settled yet at event time.
            let lws = Arc::clone(&last_window_state);
            listener.add_layer_opened_handler(move |_| {
                if let Err(err) = handle_window_state_change(&lws, verbose) {
                    eprintln!("Error handling layer open: {}", err);
                }
                spawn_poll_burst(Arc::clone(&lws), verbose);
            });

            let lws = Arc::clone(&last_window_state);
            listener.add_layer_closed_handler(move |_| {
                if let Err(err) = handle_window_state_change(&lws, verbose) {
                    eprintln!("Error handling layer close: {}", err);
                }
                spawn_poll_burst(Arc::clone(&lws), verbose);
            });

            let attempt_start = Instant::now();
            match listener.start_listener() {
                Ok(_) => {
                    // start_listener() blocks until the listener stops; this arm
                    // only runs on a clean shutdown.
                    return Ok(());
                }
                Err(e) => {
                    // Hard-fail if the very first attempt dies within 2s of
                    // startup (Hyprland genuinely unavailable).
                    if fn_start.elapsed() < Duration::from_millis(2000) {
                        return Err(format!(
                            "Failed to start event listener: {}\nAre you sure Hyprland is running?",
                            e
                        )
                        .into());
                    }

                    // A connection that stayed up a while was stable: reset the
                    // backoff so long-uptime sessions don't get stuck at the 10s
                    // cap on every reconnect (#7).
                    if attempt_start.elapsed() >= STABLE_CONNECTION_THRESHOLD {
                        delay_ms = INITIAL_RECONNECT_MS;
                    }

                    if self.verbose {
                        println!(
                            "Lost connection to Hyprland, retrying in {}ms: {}",
                            delay_ms, e
                        );
                    }
                    // Sleep with exponential backoff
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    delay_ms = std::cmp::min(delay_ms * 3, MAX_RECONNECT_MS);
                }
            }
        }
    }
}

/// Waits for Hyprland to become available with exponential backoff.
/// This handles the race condition where the service starts before Hyprland is ready.
fn wait_for_hyprland(verbose: bool) -> Result<(), Box<dyn Error>> {
    use hyprland::data::Monitors;

    const MAX_WAIT_SECS: u64 = 30;
    const INITIAL_DELAY_MS: u64 = 100;

    let start = SystemTime::now();
    let mut delay_ms = INITIAL_DELAY_MS;

    loop {
        let elapsed = start.elapsed().unwrap_or(Duration::from_secs(0));

        // Check if Hyprland environment is available (socket exists)
        if let Err(e) = check_hyprland_environment() {
            if elapsed.as_secs() >= MAX_WAIT_SECS {
                return Err(format!("Timed out waiting for Hyprland: {}", e).into());
            }
            if verbose {
                println!("Waiting for Hyprland environment ({}ms): {}", delay_ms, e);
            }
            thread::sleep(Duration::from_millis(delay_ms));
            delay_ms = std::cmp::min(delay_ms * 2, 2000); // Cap at 2 seconds
            continue;
        }

        // Environment exists, now verify IPC connection works
        match Monitors::get() {
            Ok(_) => {
                if verbose && elapsed.as_millis() > 100 {
                    println!("Hyprland ready after {}ms", elapsed.as_millis());
                }
                return Ok(());
            }
            Err(e) => {
                if elapsed.as_secs() >= MAX_WAIT_SECS {
                    return Err(format!("Timed out waiting for Hyprland IPC: {}", e).into());
                }
                if verbose {
                    println!("Hyprland IPC not ready ({}ms): {}", delay_ms, e);
                }
                thread::sleep(Duration::from_millis(delay_ms));
                delay_ms = std::cmp::min(delay_ms * 2, 2000);
            }
        }
    }
}

/// Probe whether a Hyprland IPC socket has a live listener behind it.
///
/// Returns `true` only when a client can actually `connect(2)` to `path`. This
/// is the distinction a mere file-existence check can't make: a Hyprland that
/// crashed (or a second instance that died) leaves its `.socket.sock` on disk,
/// so `path.exists()` treats a *dead* instance as reachable and silently pins
/// qmkonnect to a socket nobody is listening on — every later IPC call then
/// fails with `Connection refused` and the monitor never starts.
///
/// `connect(2)` on a local `AF_UNIX` socket is normally instantaneous (a dead
/// listener returns `ECONNREFUSED` at once), but we run it on a short-lived
/// thread with a hard [`SOCKET_PROBE_TIMEOUT`] so a pathological socket can
/// never wedge the startup scan. [`UnixStream`] has no `connect_timeout` (unlike
/// [`TcpStream`](std::net::TcpStream)), which is why this isn't a one-liner.
///
/// [`UnixStream`]: std::os::unix::net::UnixStream
fn hyprland_socket_is_live(path: &Path) -> bool {
    use std::os::unix::net::UnixStream;
    use std::sync::mpsc;

    let path = path.to_path_buf();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        // `Ok` ⇒ a process accepted the connection ⇒ the instance is alive.
        // Any error (refused, no entry, permission, …) ⇒ not reachable by us.
        let reachable = UnixStream::connect(&path).is_ok();
        let _ = tx.send(reachable);
    });
    rx.recv_timeout(SOCKET_PROBE_TIMEOUT).unwrap_or(false)
}

/// Resolve the Hyprland instance to talk to and guarantee
/// `$HYPRLAND_INSTANCE_SIGNATURE` points at a *live* one.
///
/// Candidate signatures are gathered in priority order:
///   1. `$HYPRLAND_INSTANCE_SIGNATURE` — the instance the session declares.
///      Preferred because, when it's live, it names the user's own seat rather
///      than some other instance on a multi-seat box.
///   2. every `<sig>` directory under `$XDG_RUNTIME_DIR/hypr/` — the recovery
///      path used when the env var is unset *or* points at a dead instance
///      (e.g. a systemd user service that inherited the signature of a
///      now-dead Hyprland).
///
/// The first candidate whose `.socket.sock` actually accepts a connection wins.
/// If that candidate isn't the current `$HYPRLAND_INSTANCE_SIGNATURE`, the env
/// var is republished so the `hyprland` crate (which selects its instance from
/// it) targets the live socket — this is the self-heal. We no longer shell out
/// to `ps` (#15); a connectivity check against the socket is authoritative.
///
/// Returns `Err` only when no live socket is reachable anywhere; the caller
/// ([`wait_for_hyprland`]) then backs off and retries until Hyprland comes up.
fn check_hyprland_environment() -> Result<(), Box<dyn Error>> {
    let runtime_dir = env::var("XDG_RUNTIME_DIR")
        .map_err(|_| "XDG_RUNTIME_DIR is not set; cannot locate Hyprland socket".to_string())?;
    let hypr_dir = PathBuf::from(&runtime_dir).join("hypr");

    // Candidate signatures, deduped, session-declared instance first.
    let mut candidates: Vec<String> = Vec::new();
    if let Ok(sig) = env::var("HYPRLAND_INSTANCE_SIGNATURE") {
        if !sig.is_empty() {
            candidates.push(sig);
        }
    }
    if let Ok(entries) = fs::read_dir(&hypr_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if !candidates.iter().any(|c| c == name) {
                    candidates.push(name.to_string());
                }
            }
        }
    }

    // First candidate with a live IPC socket wins. `socket.exists()` is only a
    // cheap fast-path to skip socket-less directories without spawning a probe
    // thread; the real decision is the connectivity check, which is what tells
    // a live instance apart from the stale socket file a crashed one leaves.
    for sig in &candidates {
        let socket_path = hypr_dir.join(sig).join(".socket.sock");
        if !socket_path.exists() {
            continue;
        }
        if hyprland_socket_is_live(&socket_path) {
            // The hyprland crate selects its instance via this env var, so
            // republish it whenever it differs from the live one we found
            // (stale / dead-value recovery). Runs once, on the main thread,
            // before any listener is spawned.
            // NOTE: wrap in `unsafe {}` if/when bumping to edition 2024.
            if env::var("HYPRLAND_INSTANCE_SIGNATURE").ok().as_deref() != Some(sig.as_str()) {
                env::set_var("HYPRLAND_INSTANCE_SIGNATURE", sig);
            }
            return Ok(());
        }
    }

    let hint = if candidates.is_empty() {
        "no Hyprland instance directories found under $XDG_RUNTIME_DIR/hypr/".to_string()
    } else {
        format!(
            "found {} instance dir(s) but none have a live IPC socket \
             (a crashed Hyprland can leave stale socket files behind): [{}]",
            candidates.len(),
            candidates.join(", ")
        )
    };
    Err(format!("No reachable Hyprland socket. {hint}. Is Hyprland running?").into())
}

/// A short, bounded poll burst spawned after layer (scratchpad) events to catch
/// focus changes that settle just after the event fires. Replaces the former
/// permanent 100ms poller (#8).
fn spawn_poll_burst(last_window_state: Arc<Mutex<Option<WindowState>>>, verbose: bool) {
    thread::spawn(move || {
        for _ in 0..5 {
            thread::sleep(Duration::from_millis(100));
            if let Err(err) = poll_window_state(&last_window_state, verbose) {
                eprintln!("Error polling window state: {}", err);
            }
        }
    });
}

fn poll_window_state(
    last_window_state: &Arc<Mutex<Option<WindowState>>>,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    // Get current window state
    let current_window_state = match Client::get_active() {
        Ok(Some(active_window)) => Some(WindowState {
            app_class: active_window.initial_class.clone(),
            title: active_window.title.clone(),
        }),
        Ok(None) => {
            // No active window - we're on an empty workspace
            if verbose {
                println!("Poll detected empty workspace");
            }
            Some(WindowState {
                app_class: "".to_string(),
                title: "".to_string(),
            })
        }
        Err(err) => {
            eprintln!("Failed to get active window info in poll: {}", err);
            None
        }
    };

    // Compare with last known state
    let mut last_state = last_window_state.lock().unwrap();
    let window_changed = match (&*last_state, &current_window_state) {
        (None, Some(_)) => true,
        (Some(_), None) => true,
        (Some(last), Some(current)) => {
            // Covers window-to-window changes as well as transitions to/from an
            // empty workspace (represented as empty class and title), while
            // repeated identical states compare equal and are not re-reported.
            last.app_class != current.app_class || last.title != current.title
        }
        (None, None) => false,
    };

    // If window changed, update state and notify
    if window_changed {
        if verbose {
            if let Some(ws) = &current_window_state {
                println!(
                    "[{}ms] poll detected window state change: {} | {}",
                    crate::core::now_ms(),
                    ws.app_class,
                    ws.title
                );
            } else {
                println!(
                    "[{}ms] poll detected window state change (empty)",
                    crate::core::now_ms()
                );
            }
        }
        if let Some(window_state) = &current_window_state {
            let window_info =
                WindowInfo::new(window_state.app_class.clone(), window_state.title.clone());
            if let Err(e) = notifier::notify_qmk(&window_info, verbose) {
                eprintln!("Error notifying QMK: {}", e);
            }
        }
        *last_state = current_window_state;
    }

    Ok(())
}

fn handle_window_state_change(
    last_window_state: &Arc<Mutex<Option<WindowState>>>,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    // Resolve the current window state: Some for an active window, Some(empty)
    // for an empty workspace, None on error (already logged below). Mirrors how
    // poll_window_state builds its `current_window_state`.
    let current_window_state = match Client::get_active() {
        Ok(Some(active_window)) => {
            if verbose {
                println!(
                    "[{}ms] activewindow event: {} | {}",
                    crate::core::now_ms(),
                    active_window.initial_class,
                    active_window.title
                );
            }
            Some(WindowState {
                app_class: active_window.initial_class.clone(),
                title: active_window.title.clone(),
            })
        }
        Ok(None) => {
            // No active window - we're on an empty workspace.
            if verbose {
                println!("Empty workspace detected");
            }
            Some(WindowState {
                app_class: "".to_string(),
                title: "".to_string(),
            })
        }
        Err(err) => {
            eprintln!("Failed to get active window info: {}", err);
            None
        }
    };

    let current_window_state = match current_window_state {
        Some(ws) => ws,
        // Error already logged; do not touch last_window_state or notify.
        None => return Ok(()),
    };

    // Dedup + update atomically in ONE critical section (mirrors
    // `poll_window_state`). The previous two-lock form (compare under lock,
    // release, then re-lock to update) left a TOCTOU window: `spawn_poll_burst`
    // fires a poll thread from these same event handlers, and that thread could
    // read the *same* stale `last_window_state`, also conclude "changed", also
    // update, and also notify — re-introducing exactly the duplicate this dedup
    // was added to prevent. Holding the lock across compare+update closes the
    // race. The WindowInfo (a cheap clone of the two strings) is captured under
    // the lock; `notify_qmk` runs AFTER the lock is dropped because it takes the
    // debounce STATE/NOTIFIER locks, which must not be acquired while holding
    // `last_window_state`.
    let window_info = {
        let mut last_state = last_window_state.lock().unwrap();
        let changed = match &*last_state {
            None => true,
            Some(last) => {
                last.app_class != current_window_state.app_class
                    || last.title != current_window_state.title
            }
        };
        if changed {
            let wi = WindowInfo::new(
                current_window_state.app_class.clone(),
                current_window_state.title.clone(),
            );
            *last_state = Some(current_window_state);
            Some(wi)
        } else {
            None
        }
    };

    if let Some(wi) = window_info {
        if let Err(e) = notifier::notify_qmk(&wi, verbose) {
            eprintln!("Error notifying QMK: {}", e);
        }
    }

    Ok(())
}

/// List currently-mapped clients as `(class, title)` pairs (§7a).
///
/// Used by the Linux SNI tray's "Show Window Information" item so the same
/// data shape macOS/Windows return is available on Hyprland. Only mapped
/// (on-screen) windows are included; transient focus shifts (scratchpads,
/// layer surfaces) are irrelevant for a user-facing list. The active window is
/// surfaced first when present so the notification path (which takes `.next()`)
/// reports the focused window.
///
/// `#[allow(dead_code)]`: only reached when a tray (the SNI `linux-tray`
/// build) actually calls the platform dispatcher; dead in the trayless default
/// build, exactly like `core::notifier::is_device_connected`.
#[allow(dead_code)]
pub fn list_foreground_windows() -> Vec<(String, String)> {
    let clients = match Clients::get() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to enumerate Hyprland clients: {}", e);
            return Vec::new();
        }
    };

    let mut rows: Vec<(String, String)> = clients
        .iter()
        .filter(|c| c.mapped)
        .map(|c| (c.class.clone(), c.title.clone()))
        .collect();

    // Move the active window to the front so callers taking `.next()` report
    // the focused window (parity with the macOS/Windows "active window" notion).
    if let Ok(Some(active)) = Client::get_active() {
        let key = (active.class.clone(), active.title.clone());
        if let Some(pos) = rows.iter().position(|r| *r == key) {
            rows.swap(0, pos);
        }
    }

    rows
}

fn handle_workspace_change(
    workspace_event: WorkspaceEventData,
    last_window_state: &Arc<Mutex<Option<WindowState>>>,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    if verbose {
        println!("Workspace changed to {}", workspace_event.id);
    }

    // Check if the workspace is empty by checking for active window
    handle_window_state_change(last_window_state, verbose)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_state() {
        let state1 = WindowState {
            app_class: "App1".to_string(),
            title: "Title1".to_string(),
        };

        let state2 = state1.clone();

        assert_eq!(state1.app_class, state2.app_class);
        assert_eq!(state1.title, state2.title);
        assert_eq!(state1, state2);
    }

    #[test]
    fn test_hyprland_monitor_creation() {
        let monitor = HyprlandMonitor::new(true);
        assert_eq!(monitor.platform_name(), "Hyprland");
        assert!(monitor.verbose);

        let monitor = HyprlandMonitor::new(false);
        assert_eq!(monitor.platform_name(), "Hyprland");
        assert!(!monitor.verbose);
    }

    // Note: Most functionality in HyprlandMonitor heavily depends on
    // the actual Hyprland environment, so we can only unit test the
    // basic parts without specialized mocks. The socket-liveness probe
    // below is the exception: it's hermetic (real local sockets in a
    // TempDir, no running Hyprland required).

    #[test]
    fn hyprland_socket_is_live_accepts_a_listening_socket() {
        use std::os::unix::net::UnixListener;
        let dir = tempfile::TempDir::new().unwrap();
        let socket = dir.path().join(".socket.sock");
        // A bound, still-open listener ⇒ connect() succeeds ⇒ reported live.
        let _listener = UnixListener::bind(&socket).unwrap();
        assert!(hyprland_socket_is_live(&socket));
        // `_listener` and `dir` (TempDir) clean up on drop.
    }

    #[test]
    fn hyprland_socket_is_live_rejects_a_dead_leftover_socket() {
        use std::os::unix::net::UnixListener;
        let dir = tempfile::TempDir::new().unwrap();
        let socket = dir.path().join(".socket.sock");
        // Bind then drop: std does NOT unlink the path on drop, so the socket
        // file is left behind with no listener — the exact stale state a
        // crashed Hyprland leaves. connect() must now refuse.
        let listener = UnixListener::bind(&socket).unwrap();
        drop(listener);
        assert!(!hyprland_socket_is_live(&socket));
    }

    #[test]
    fn hyprland_socket_is_live_false_for_a_missing_path() {
        let dir = tempfile::TempDir::new().unwrap();
        let missing = dir.path().join("does-not-exist.sock");
        assert!(!hyprland_socket_is_live(&missing));
    }
}

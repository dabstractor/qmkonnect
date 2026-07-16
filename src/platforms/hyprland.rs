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
    path::PathBuf,
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
        let (_, poll_interval_ms) = crate::core::configured_timing();
        if poll_interval_ms > 0 {
            let lws = Arc::clone(&self.last_window_state);
            let verbose = self.verbose;
            thread::spawn(move || {
                let interval = Duration::from_millis(poll_interval_ms);
                if verbose {
                    println!(
                        "[{}ms] periodic active-window poll every {}ms",
                        crate::core::now_ms(),
                        poll_interval_ms
                    );
                }
                loop {
                    thread::sleep(interval);
                    if let Err(err) = poll_window_state(&lws, verbose) {
                        eprintln!("Error in periodic poll: {}", err);
                    }
                }
            });
        }

        loop {
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

fn check_hyprland_environment() -> Result<(), Box<dyn Error>> {
    // Preferred path: the session exports HYPRLAND_INSTANCE_SIGNATURE.
    if let Ok(signature) = env::var("HYPRLAND_INSTANCE_SIGNATURE") {
        if let Ok(runtime_dir) = env::var("XDG_RUNTIME_DIR") {
            let socket_path = PathBuf::from(&runtime_dir)
                .join("hypr")
                .join(&signature)
                .join(".socket.sock");

            if socket_path.exists() {
                return Ok(());
            }
        }
    }

    // Fallback: discover an instance under $XDG_RUNTIME_DIR/hypr/<sig>/.socket.sock.
    // Covers systemd-launched services that don't inherit the session env. We no
    // longer shell out to `ps` (#15) — the socket scan is authoritative.
    if let Ok(runtime_dir) = env::var("XDG_RUNTIME_DIR") {
        let hypr_dir = PathBuf::from(&runtime_dir).join("hypr");
        if let Ok(entries) = fs::read_dir(&hypr_dir) {
            for entry in entries.flatten() {
                let socket_path = entry.path().join(".socket.sock");
                if socket_path.exists() {
                    // The hyprland crate selects its instance via this env var,
                    // so publish the discovered signature. This runs once, on the
                    // main thread, before any listener is spawned.
                    // NOTE: wrap in `unsafe {}` if/when bumping to edition 2024.
                    env::set_var("HYPRLAND_INSTANCE_SIGNATURE", entry.file_name());
                    return Ok(());
                }
            }
        }
    }

    Err("Hyprland socket not found under $XDG_RUNTIME_DIR/hypr/<signature>/.socket.sock. Is Hyprland running?".into())
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
    match Client::get_active() {
        Ok(Some(active_window)) => {
            if verbose {
                println!(
                    "[{}ms] activewindow event: {} | {}",
                    crate::core::now_ms(),
                    active_window.initial_class,
                    active_window.title
                );
            }
            let window_info = WindowInfo::new(
                active_window.initial_class.clone(),
                active_window.title.clone(),
            );

            // Update last known state
            {
                let mut last_state = last_window_state.lock().unwrap();
                *last_state = Some(WindowState {
                    app_class: active_window.initial_class.clone(),
                    title: active_window.title.clone(),
                });
            }

            if let Err(e) = notifier::notify_qmk(&window_info, verbose) {
                eprintln!("Error notifying QMK: {}", e);
            }
        }
        Ok(None) => {
            // No active window - we're on an empty workspace
            if verbose {
                println!("Empty workspace detected");
            }

            // Create a special window info for empty workspace
            let window_info = WindowInfo::new("".to_string(), "".to_string());

            // Update last known state
            {
                let mut last_state = last_window_state.lock().unwrap();
                *last_state = Some(WindowState {
                    app_class: "".to_string(),
                    title: "".to_string(),
                });
            }

            if let Err(e) = notifier::notify_qmk(&window_info, verbose) {
                eprintln!("Error notifying QMK: {}", e);
            }
        }
        Err(err) => {
            eprintln!("Failed to get active window info: {}", err);
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
    // basic parts without specialized mocks.
}

#![cfg(all(target_os = "linux", feature = "hyprland"))]
use crate::core::notifier;
use crate::core::types::WindowInfo;
use crate::platforms::WindowMonitor;
use hyprland::{
    data::Client,
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
    time::{Duration, SystemTime},
};

// Custom struct to store window state that implements Clone
#[derive(PartialEq, Debug)]
struct WindowState {
    app_class: String,
    title: String,
}

impl Clone for WindowState {
    fn clone(&self) -> Self {
        Self {
            app_class: self.app_class.clone(),
            title: self.title.clone(),
        }
    }
}

pub struct HyprlandMonitor {
    event_listener: Option<EventListener>,
    last_window_state: Arc<Mutex<Option<WindowState>>>,
    polling_active: Arc<Mutex<bool>>,
    verbose: bool,
}

impl HyprlandMonitor {
    pub fn new(verbose: bool) -> Self {
        Self {
            event_listener: None,
            last_window_state: Arc::new(Mutex::new(None)),
            polling_active: Arc::new(Mutex::new(false)),
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

        {
            // Set polling to active
            {
                let mut active = self.polling_active.lock().unwrap();
                *active = true;
            }

            // Start polling thread for scratchpad detection
            let polling_active = Arc::clone(&self.polling_active);
            let last_window_state = Arc::clone(&self.last_window_state);
            let verbose = self.verbose;

            thread::spawn(move || {
                let poll_interval = Duration::from_millis(100);
                while *polling_active.lock().unwrap() {
                    // Poll for window state
                    if let Err(err) = poll_window_state(&last_window_state, verbose) {
                        eprintln!("Error polling window state: {}", err);
                    }
                    thread::sleep(poll_interval);
                }
                if verbose {
                    println!("Window state polling thread stopped");
                }
            });

            let start: SystemTime = SystemTime::now();
            let mut delay_ms = 101;

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

                // Add layer surface (like scratchpads) handlers - note the correct method names
                let lws = Arc::clone(&last_window_state);
                listener.add_layer_opened_handler(move |_| {
                    if let Err(err) = handle_window_state_change(&lws, verbose) {
                        eprintln!("Error handling layer open: {}", err);
                    }
                });

                let lws = Arc::clone(&last_window_state);
                listener.add_layer_closed_handler(move |_| {
                    if let Err(err) = handle_window_state_change(&lws, verbose) {
                        eprintln!("Error handling layer close: {}", err);
                    }
                });

                // Try to start the listener
                match listener.start_listener() {
                    Ok(_) => {
                        // This branch never executes because start_listener blocks
                        // until an error occurs. Including it for API correctness.
                        self.event_listener = Some(listener);
                        return Ok(());
                    }
                    Err(e) => {
                        if start.elapsed().unwrap().as_millis() < 2001 {
                            // Stop polling thread
                            {
                                let mut active = self.polling_active.lock().unwrap();
                                *active = false;
                            }
                            return Err(format!("Failed to start event listener: {}\nAre you sure Hyprland is running?", e).into());
                        }
                        // Otherwise, retry with exponential backoff
                        if self.verbose {
                            println!(
                                "Lost connection to Hyprland, retrying in {}ms: {}",
                                delay_ms, e
                            );
                        }
                        // Sleep with exponential backoff
                        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                        // Exponential backoff with a cap
                        delay_ms = std::cmp::min(delay_ms * 3, 10000); // Cap at 10 seconds
                    }
                }
            }
        }
    }

    fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        {
            // Stop the polling thread
            {
                let mut active = self.polling_active.lock().unwrap();
                *active = false;
            }
            self.event_listener = None;
        }
        Ok(())
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
        match check_hyprland_environment() {
            Err(e) => {
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
            Ok(_) => {}
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
    // First try environment variable approach
    if let Ok(signature) = env::var("HYPRLAND_INSTANCE_SIGNATURE") {
        if let Ok(runtime_dir) = env::var("XDG_RUNTIME_DIR") {
            let socket_path = PathBuf::from(&runtime_dir)
                .join("hypr")
                .join(signature)
                .join(".socket.sock");

            if socket_path.exists() {
                return Ok(());
            }
        }
    }

    // If environment variables aren't set, try to find the socket directly
    if let Ok(runtime_dir) = env::var("XDG_RUNTIME_DIR") {
        let hypr_dir = PathBuf::from(&runtime_dir).join("hypr");
        if hypr_dir.exists() {
            // Look for any instance directories
            if let Ok(entries) = fs::read_dir(&hypr_dir) {
                for entry in entries.flatten() {
                    let socket_path = entry.path().join(".socket.sock");
                    if socket_path.exists() {
                        // Found a socket! Set the environment variable
                        let instance_sig = entry.file_name();
                        env::set_var(
                            "HYPRLAND_INSTANCE_SIGNATURE",
                            instance_sig.to_string_lossy().as_ref(),
                        );
                        return Ok(());
                    }
                }
            }
        }
    }

    // If we get here, we need to check if Hyprland is actually running
    let ps_output = std::process::Command::new("ps")
        .args(["-e", "-o", "comm="])
        .output()?;

    let processes = String::from_utf8_lossy(&ps_output.stdout);
    if processes.lines().any(|line| line.contains("Hyprland")) {
        // Hyprland is running but we can't find the socket
        Err("Hyprland is running but socket not found. Ensure XDG_RUNTIME_DIR is set and accessible.".into())
    } else {
        Err("Hyprland is not running".into())
    }
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
            // Only consider it changed if either:
            // 1. We're moving from a window to empty workspace
            // 2. We're moving from empty workspace to a window
            // 3. We're changing between different windows
            // But NOT if we're staying on an empty workspace
            if last.app_class == "empty" && current.app_class == "empty" {
                false // Don't report repeated empty workspace states
            } else {
                last.app_class != current.app_class || last.title != current.title
            }
        }
        (None, None) => false,
    };

    // If window changed, update state and notify
    if window_changed {
        if verbose {
            println!("Poll detected window state change");
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

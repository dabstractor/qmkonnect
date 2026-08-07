#![cfg(target_os = "linux")]
use crate::core::notifier;
use crate::core::types::WindowInfo;
use crate::platforms::WindowMonitor;
use std::error::Error;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub struct X11Monitor {
    verbose: bool,
    running: Arc<AtomicBool>,
}

impl X11Monitor {
    pub fn new(verbose: bool) -> Self {
        Self {
            verbose,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Read the focused window via `xprop`. Returns `Ok(None)` when no window is
    /// focused (empty desktop). This is a real implementation (not the former
    /// "X11Application/Active Window" stub) — see issue #14.
    fn get_active_window_info(&self) -> Result<Option<WindowInfo>, Box<dyn Error>> {
        // 1. Resolve the active window id from the root window.
        let root_out = Command::new("xprop")
            .args(["-root", "_NET_ACTIVE_WINDOW"])
            .output()?;

        if !root_out.status.success() {
            return Err(String::from_utf8_lossy(&root_out.stderr).into());
        }

        let root_stdout = String::from_utf8_lossy(&root_out.stdout);
        // Line looks like: _NET_ACTIVE_WINDOW(WINDOW): window id # 0x4a00006
        let wid = root_stdout
            .lines()
            .find(|l| l.contains("_NET_ACTIVE_WINDOW"))
            .and_then(|l| l.split_whitespace().last())
            .ok_or("_NET_ACTIVE_WINDOW not present")?;

        // 0x0 / 0 means "no focused window" (empty desktop).
        if wid == "0x0" || wid == "0" {
            return Ok(None);
        }

        // 2. Fetch WM_CLASS and _NET_WM_NAME for that window.
        let prop_out = Command::new("xprop")
            .args(["-id", wid, "WM_CLASS", "_NET_WM_NAME"])
            .output()?;

        if !prop_out.status.success() {
            return Err(String::from_utf8_lossy(&prop_out.stderr).into());
        }

        let prop_stdout = String::from_utf8_lossy(&prop_out.stdout);
        let mut app_class = String::new();
        let mut title = String::new();

        for line in prop_stdout.lines() {
            if line.starts_with("WM_CLASS") {
                if let Some(rest) = line.split_once('=').map(|(_, r)| r) {
                    app_class = parse_wm_class(rest).unwrap_or_default();
                }
            } else if line.starts_with("_NET_WM_NAME") {
                if let Some(rest) = line.split_once('=').map(|(_, r)| r) {
                    title = rest.trim().trim_matches('"').to_string();
                }
            }
        }

        Ok(Some(WindowInfo::new(app_class, title)))
    }
}

/// Availability probe for the X11 backend (PLATFORMS.md §6). Returns `Ok` ONLY
/// when all three hold:
///   1. `$DISPLAY` is set AND non-empty;
///   2. `$WAYLAND_DISPLAY` is **unset** (or empty) — X11 is NEVER selected under
///      a Wayland compositor (Invariant 11, ARCHITECTURE.md §10): XWayland sets
///      `$DISPLAY` but reports focus unreliably for native Wayland windows;
///   3. `xprop` is on PATH (the X11 monitor shells out to `xprop`).
///
/// An empty env value is treated as unset (matches `get_config_paths()`).
/// Side-effect-free (no env mutation) so a re-probe is safe.
pub(crate) fn probe_available(_verbose: bool) -> Result<(), String> {
    let display = std::env::var("DISPLAY").ok().filter(|s| !s.is_empty());
    if display.is_none() {
        return Err("$DISPLAY is not set".into());
    }
    // Invariant 11 (ARCHITECTURE.md §10): X11 is NEVER selected under a Wayland
    // compositor. XWayland sets $DISPLAY but reports focus unreliably for native
    // Wayland windows, so picking X11 there would silently report wrong windows.
    if std::env::var("WAYLAND_DISPLAY")
        .ok()
        .filter(|s| !s.is_empty())
        .is_some()
    {
        return Err(
            "Wayland session ($WAYLAND_DISPLAY set) — X11 is never selected under a Wayland \
             compositor (XWayland focus is unreliable for native windows; PLATFORMS.md §6/§10)"
                .into(),
        );
    }
    // xprop presence WITHOUT depending on a live X server: `xprop -version`
    // (and every other xprop invocation) itself tries to open $DISPLAY and
    // fails when it can't, so it can't tell "installed" from "display
    // reachable". Resolve the binary on PATH instead (works headless / under
    // Wayland / on a CI box). A missing binary ⇒ not installed ⇒ the gate
    // fails. NOT `xprop -root` (that needs a running X server).
    match std::process::Command::new("which")
        .arg("xprop")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(s) if s.success() => Ok(()),
        Ok(_) => Err("`xprop` not found on PATH (install xorg-xprop)".into()),
        Err(_) => Err("`which` not available to verify `xprop`".into()),
    }
}

/// Parse the **class** out of the `WM_CLASS` property's `= …` remainder.
///
/// `xprop` prints `WM_CLASS(STRING) = "instance", "Class"`. After
/// [`split_once('=')`](str::split_once) the caller passes
/// `rest = ' "instance", "Class"'`. Splitting on `,` (not `"`) means a leading
/// space or the `, ` separator can't shift the field index; then trim + strip the
/// quotes. Prefers the **class** (2nd field) and falls back to the **instance**
/// (1st field) for degenerate single-field output. Returns `None` when no non-empty
/// field is present.
fn parse_wm_class(rest: &str) -> Option<String> {
    let parts: Vec<&str> = rest
        .split(',')
        .map(|s| s.trim().trim_matches('"'))
        .filter(|s| !s.is_empty())
        .collect();
    parts
        .get(1)
        .or_else(|| parts.first())
        .map(|s| s.to_string())
}

impl WindowMonitor for X11Monitor {
    fn platform_name(&self) -> &str {
        "Linux (X11)"
    }

    fn start(&mut self) -> Result<(), Box<dyn Error>> {
        if self.verbose {
            println!("Starting Linux X11 window monitor");
        }

        // Fail loudly (instead of pretending to work) if xprop is unavailable.
        // #14: never emit placeholder strings.
        let xprop_ok = Command::new("xprop")
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !xprop_ok {
            return Err(
                "X11 monitor requires `xprop` (xorg-xprop), which was not found on PATH. \
                 Install it, or build with the `hyprland` feature for Hyprland support."
                    .into(),
            );
        }

        self.running.store(true, Ordering::SeqCst);

        let running = Arc::clone(&self.running);
        let verbose = self.verbose;

        // Poll for focus changes. xprop is invoked twice per cycle, so use a
        // modest interval (X11 focus changes are user-driven; latency is fine
        // for a fallback platform).
        thread::spawn(move || {
            let poll_interval = Duration::from_millis(500);
            let mut last_window: Option<(String, String)> = None;

            while running.load(Ordering::SeqCst) {
                // A throwaway monitor owns the read helper.
                let probe = X11Monitor::new(verbose);
                match probe.get_active_window_info() {
                    Ok(Some(window_info)) => {
                        let current = (window_info.app_class.clone(), window_info.title.clone());
                        if last_window.as_ref() != Some(&current) {
                            if verbose {
                                println!(
                                    "Window changed - Class: '{}', Title: '{}'",
                                    window_info.app_class, window_info.title
                                );
                            }
                            if let Err(e) = notifier::notify_qmk(&window_info, verbose) {
                                eprintln!("Failed to notify QMK: {}", e);
                            }
                            last_window = Some(current);
                        }
                    }
                    Ok(None) => {
                        // Empty workspace — clear so the next focus notifies.
                        if last_window.is_some() {
                            let window_info = WindowInfo::new(String::new(), String::new());
                            if let Err(e) = notifier::notify_qmk(&window_info, verbose) {
                                eprintln!("Failed to notify QMK: {}", e);
                            }
                            last_window = None;
                        }
                    }
                    Err(e) => {
                        if verbose {
                            eprintln!("xprop query failed: {}", e);
                        }
                    }
                }

                thread::sleep(poll_interval);
            }

            if verbose {
                println!("Linux X11 monitor thread stopped");
            }
        });

        if self.verbose {
            println!("Linux X11 monitor started - polling for window changes");
        }

        Ok(())
    }

    fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        if self.verbose {
            println!("Stopping Linux X11 window monitor");
        }
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_wm_class_returns_class_not_instance() {
        // xprop prints: WM_CLASS(STRING) = "instance", "Class"
        // `rest` is the '= '-suffixed remainder passed to parse_wm_class.
        // Regression: the OLD split-on-quote returned "firefox" (the instance).
        assert_eq!(
            parse_wm_class(r#" "firefox", "Firefox""#),
            Some("Firefox".to_string())
        );

        // End-to-end: extract `rest` exactly as the call site does, then parse.
        let line = r#"WM_CLASS(STRING) = "firefox", "Firefox""#;
        let rest = line.split_once('=').map(|(_, r)| r).unwrap();
        assert_eq!(parse_wm_class(rest), Some("Firefox".to_string()));

        // Multi-word class (instance/class differ in casing + spacing).
        assert_eq!(
            parse_wm_class(r#" "google-chrome", "Google Chrome""#),
            Some("Google Chrome".to_string())
        );
    }

    #[test]
    fn parse_wm_class_single_field_falls_back_to_first() {
        // Degenerate: only one quoted field. Falls back to the first (and only).
        assert_eq!(
            parse_wm_class(r#" "Navigator""#),
            Some("Navigator".to_string())
        );
    }

    #[test]
    fn parse_wm_class_empty_or_whitespace_is_none() {
        // Empty / whitespace-only / only-empty-quotes ⇒ no non-empty field ⇒ None.
        assert_eq!(parse_wm_class(""), None);
        assert_eq!(parse_wm_class("   "), None);
        assert_eq!(parse_wm_class(r#" "", """#), None);
    }
}

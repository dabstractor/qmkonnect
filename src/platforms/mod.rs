mod hyprland;
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(all(target_os = "linux", not(feature = "hyprland")))]
mod x11;

// Define the WindowMonitor trait. A single `Send` trait serves every platform:
// Hyprland's `start()` blocks on its IPC listener, so it no longer needs to keep
// the listener around in `self` (which was the only reason a non-`Send` variant
// existed).
pub trait WindowMonitor: Send {
    fn platform_name(&self) -> &str;
    fn start(&mut self) -> Result<(), Box<dyn std::error::Error>>;

    #[allow(dead_code)]
    fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Default no-op; platform impls override where a real stop exists.
        Ok(())
    }
}

// Export Linux module's functions
#[cfg(target_os = "linux")]
pub use linux::*;

use std::error::Error;

// Return a platform-specific monitor implementation
pub fn create_monitor(verbose: bool) -> Result<Box<dyn WindowMonitor>, Box<dyn Error>> {
    // Platform-specific implementations
    #[cfg(all(target_os = "linux", feature = "hyprland"))]
    {
        use hyprland::HyprlandMonitor;
        Ok(Box::new(HyprlandMonitor::new(verbose)))
    }

    #[cfg(all(target_os = "linux", not(feature = "hyprland")))]
    {
        use x11::X11Monitor;
        Ok(Box::new(X11Monitor::new(verbose)))
    }

    #[cfg(target_os = "macos")]
    {
        use macos::MacOSMonitor;
        Ok(Box::new(MacOSMonitor::new(verbose)))
    }

    #[cfg(target_os = "windows")]
    {
        use windows::WindowsMonitor;
        Ok(Box::new(WindowsMonitor::new(verbose)))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    Err("No suitable monitor for this platform".into())
}

// Get configuration paths based on current platform
pub fn get_config_paths() -> Vec<std::path::PathBuf> {
    #[cfg(target_os = "linux")]
    return linux::get_config_paths();

    #[cfg(target_os = "windows")]
    return windows::get_config_paths();

    #[cfg(target_os = "macos")]
    return macos::get_config_paths();

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    return Vec::new(); // Default for other platforms
}

// List currently-running foreground windows as `(class, title)` pairs.
// Implemented for the tray-bearing platforms (macOS / Windows) and for the
// Linux/Hyprland build (so the SNI tray's "Show Window Information" item has
// data to surface); returns an empty list everywhere else.
pub fn list_foreground_windows() -> Vec<(String, String)> {
    #[cfg(target_os = "macos")]
    return macos::list_foreground_windows();

    #[cfg(target_os = "windows")]
    return windows::list_foreground_windows();

    #[cfg(all(target_os = "linux", feature = "hyprland"))]
    return hyprland::list_foreground_windows();

    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", feature = "hyprland")
    )))]
    return Vec::new();
}

// Create configuration directory based on current platform
pub fn create_config_dir() -> Result<std::path::PathBuf, Box<dyn Error>> {
    #[cfg(target_os = "linux")]
    return linux::create_config_dir();

    #[cfg(target_os = "windows")]
    return windows::create_config_dir();

    #[cfg(target_os = "macos")]
    return macos::create_config_dir();

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        // Default implementation for other platforms
        let config_dir = if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
            std::path::PathBuf::from(xdg_config).join("qmkonnect")
        } else if let Some(home) = dirs::home_dir() {
            home.join(".config").join("qmkonnect")
        } else {
            return Err("Could not determine configuration directory".into());
        };

        std::fs::create_dir_all(&config_dir)?;
        Ok(config_dir)
    }
}

/// Best-effort, non-blocking desktop notification (fire-and-forget). Surfaces a
/// malformed `rules.toml` to the user (HOST_RULES.md §7). The caller
/// (`host_context_for_window`) dedupes, so this fires at most once per broken
/// state. Failures (no daemon / binary) are silently ignored.
pub fn notify(title: &str, body: &str) {
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("notify-send")
            .args(["--app-name=QMKonnect", "--icon=input-keyboard", title, body])
            .status();
    }
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification {} with title {}",
            osa_string(body),
            osa_string(title)
        );
        let _ = std::process::Command::new("osascript")
            .args(["-e", &script])
            .status();
    }
    #[cfg(target_os = "windows")]
    {
        // Dep-free stand-in for a WinRT toast (a true toast needs an
        // AppUserModelID + Start Menu shortcut to render). Non-blocking via a
        // spawned thread; mirrors tray.rs's MessageBoxW idiom.
        let body_w: Vec<u16> = body.encode_utf16().chain(std::iter::once(0)).collect();
        let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        std::thread::spawn(move || {
            use windows::core::PCWSTR;
            use windows::Win32::Foundation::HWND;
            use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
            unsafe {
                let _ = MessageBoxW(
                    HWND(0),
                    PCWSTR(body_w.as_ptr()),
                    PCWSTR(title_w.as_ptr()),
                    MB_OK | MB_ICONERROR,
                );
            }
        });
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = (title, body); // unsupported platform — no-op
    }
}

/// Quote a Rust string as an AppleScript (`osascript`) string literal.
#[cfg(target_os = "macos")]
fn osa_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Open `path` in the system default application for its type (the "Edit rules"
/// tray action — HOST_RULES.md §7). Returns the spawn `Result` so the caller
/// can log a launch failure; the call itself returns once the app is launched.
pub fn open_in_default_app(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(path).status()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(path).status()?;
    }
    #[cfg(target_os = "windows")]
    {
        // `cmd /C start "" <path>` — the empty title is required or `start`
        // treats the path as the window title.
        std::process::Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(path)
            .status()?;
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        return Err("open_in_default_app: unsupported platform".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock implementation for testing
    pub struct MockWindowMonitor {
        platform_name: String,
        start_called: bool,
        stop_called: bool,
    }

    impl MockWindowMonitor {
        pub fn new(platform_name: &str) -> Self {
            Self {
                platform_name: platform_name.to_string(),
                start_called: false,
                stop_called: false,
            }
        }

        pub fn was_start_called(&self) -> bool {
            self.start_called
        }

        pub fn was_stop_called(&self) -> bool {
            self.stop_called
        }
    }

    impl WindowMonitor for MockWindowMonitor {
        fn platform_name(&self) -> &str {
            &self.platform_name
        }

        fn start(&mut self) -> Result<(), Box<dyn Error>> {
            self.start_called = true;
            Ok(())
        }

        fn stop(&mut self) -> Result<(), Box<dyn Error>> {
            self.stop_called = true;
            Ok(())
        }
    }

    #[test]
    fn test_window_monitor_implementation() {
        let mut monitor = MockWindowMonitor::new("Mock Platform");

        // Test platform_name
        assert_eq!(monitor.platform_name(), "Mock Platform");

        // Test start
        let result = monitor.start();
        assert!(result.is_ok());
        assert!(monitor.was_start_called());

        // Test stop
        let result = monitor.stop();
        assert!(result.is_ok());
        assert!(monitor.was_stop_called());
    }
}

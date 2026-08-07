mod hyprland;
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
// X11 is unconditional on Linux now: the runtime selector `select_linux_backend`
// probes it (last in priority order, never under Wayland). The file-level
// `#![cfg(target_os = "linux")]` in x11.rs is the authoritative gate
// (matches the hyprland.rs pattern); no cfg here avoids the duplicated-attribute
// lint. PLATFORMS.md §6/§10.
mod x11;
// foreign-toplevel Wayland backend (PLATFORMS.md §7 — priority #1). Hand-rolled
// wlr protocol (sctk 0.20 exposes only the ext variant, which has no activation
// state — see wayland_ft.rs module docs for the correction).
#[cfg(all(target_os = "linux", feature = "wayland"))]
mod wayland_ft;
// GNOME Shell-extension D-Bus client backend (PLATFORMS.md §8 — priority #2).
// GNOME (Mutter) advertises neither foreign-toplevel protocol; the
// `qmkonnect@mulletware` extension republishes the active window over D-Bus.
#[cfg(all(target_os = "linux", feature = "gnome"))]
mod gnome;

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

    /// True iff `start()` BLOCKS the calling thread (e.g. Hyprland's IPC
    /// listener loop). `false` (the default) means `start()` spawns its own
    /// worker thread and returns promptly (X11 / foreign-toplevel / GNOME /
    /// AT-SPI). The Linux runner branches on this so it can park main / drive
    /// the tray for spawn-and-return backends (PLATFORMS.md §6, ARCHITECTURE.md
    /// §2.2/§11). The default matches every current+future backend except
    /// Hyprland, which overrides to `true`.
    fn start_blocks_calling_thread(&self) -> bool {
        false
    }
}

// Export Linux module's functions
#[cfg(target_os = "linux")]
pub use linux::*;

use std::error::Error;

// Return a platform-specific monitor implementation
pub fn create_monitor(verbose: bool) -> Result<Box<dyn WindowMonitor>, Box<dyn Error>> {
    // Platform-specific implementations
    #[cfg(target_os = "linux")]
    {
        // On Linux the runtime selector probes each compiled-in backend
        // (select_linux_backend, PLATFORMS.md §6) and returns the first present
        // one — or `Err` when none is available (the runner then keeps the tray
        // + device pipeline alive). The `[linux] backend` config override
        // (CONFIG.md §1.3) is wired into `forced` here (P2.M1.T2.S1): a config
        // value of `None` or `"auto"` ⇒ auto-selection (forced=None); any other
        // value forces that named backend (loud-Err if it's unavailable).
        let forced = crate::core::cached_config()
            .ok()
            .and_then(|c| c.linux.backend)
            .and_then(|b| match b.to_ascii_lowercase().as_str() {
                "auto" | "" => None, // auto / empty = runtime priority order
                _ => Some(b),         // force the named backend
            });
        if verbose {
            if let Some(ref b) = forced {
                println!("config [linux] backend = {b:?} (forced)");
            }
        }
        linux::select_linux_backend(verbose, forced.as_deref())
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
// Implemented for the tray-bearing platforms (macOS / Windows) and on Linux for
// the foreign-toplevel Wayland backend (priority #1) and the Hyprland-IPC
// backend (so the SNI tray's "Show Window Information" item has data to
// surface); returns an empty list everywhere else.
#[cfg_attr(
    not(any(target_os = "macos", target_os = "windows", feature = "linux-tray")),
    allow(dead_code)
)]
pub fn list_foreground_windows() -> Vec<(String, String)> {
    #[cfg(target_os = "macos")]
    return macos::list_foreground_windows();

    #[cfg(target_os = "windows")]
    return windows::list_foreground_windows();

    // foreign-toplevel is priority #1 on Linux (matches select_linux_backend
    // ordering). Only compiled in under the `wayland` feature.
    #[cfg(all(target_os = "linux", feature = "wayland"))]
    return wayland_ft::list_foreground_windows();

    #[cfg(all(target_os = "linux", not(feature = "wayland"), feature = "hyprland"))]
    return hyprland::list_foreground_windows();

    // GNOME Shell-extension backend (PLATFORMS.md §8 — priority #2). Reached
    // when foreign-toplevel isn't compiled in (mirrors the hyprland branch's
    // not(feature="wayland") gating so the cfg ladder stays mutually
    // exclusive). Single-window read via the extension's D-Bus proxy.
    #[cfg(all(
        target_os = "linux",
        not(feature = "wayland"),
        not(feature = "hyprland"),
        feature = "gnome"
    ))]
    return gnome::list_foreground_windows();

    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", feature = "wayland"),
        all(target_os = "linux", not(feature = "wayland"), feature = "hyprland"),
        all(
            target_os = "linux",
            not(feature = "wayland"),
            not(feature = "hyprland"),
            feature = "gnome"
        )
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

// ── Windows toast identity (P1.M4.T1.S1) ─────────────────────────────────────
// Stable AppUserModelID for this app. A WinRT toast will not render unless the
// process sets this AUMID (via set_aumid() below) AND a Start Menu shortcut
// advertises it (P1.M4.T2.S1, Inno installer). Convention is Publisher.App; the
// publisher/name mirror packaging/windows/inno/QMKonnect.iss (MyAppName=
// "QMKonnect", MyAppPublisher="Mulletware"). NOTE: this is NOT the Inno `AppId`
// GUID ({FAAE1F7A-…}) — that is the installer upgrade identity; the AUMID is the
// toast identity and the two are distinct.
pub const APP_AUMID: &str = "Mulletware.QMKonnect";

// Compile-time guard: a blanked AUMID would silently break toasts at runtime;
// fail the build instead. (Plain &str ⇒ compiles on every platform, so this also
// gates the Linux dev build.)
#[allow(dead_code)]
const _APP_AUMID_NONEMPTY: () = {
    assert!(!APP_AUMID.is_empty());
};

/// Set this process's AppUserModelID so WinRT toasts originate from "Mulletware.
/// QMKonnect" (must match the Start Menu shortcut's System.AppUserModel.ID).
/// Call once at startup on Windows, before any toast. Pure Win32 shell32 — no
/// COM init needed. Idempotent; failure is non-fatal (toasts just won't render).
#[cfg(target_os = "windows")]
pub fn set_aumid() {
    // NOTE: qualify the extern crate as `::windows` — this module declares a
    // `mod windows;` submodule (the platform impl in windows.rs), so a bare
    // `windows::…` resolves to the SUBMODULE, not the windows-rs crate.
    use ::windows::core::PCWSTR;
    use ::windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
    // PCWSTR wants a NUL-terminated UTF-16 buffer; keep the Vec alive across the call.
    let wide: Vec<u16> = APP_AUMID.encode_utf16().chain(std::iter::once(0)).collect();
    let hr = unsafe { SetCurrentProcessExplicitAppUserModelID(PCWSTR(wide.as_ptr())) };
    if let Err(e) = hr {
        log::warn!("set_aumid: SetCurrentProcessExplicitAppUserModelID failed: {e}");
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
        // WinRT toast (P1.M4.T1.S2). Fire-and-forget; the caller dedupes and
        // show_toast swallows all failures. Replaces the former focus-stealing
        // MessageBoxW modal (bug-hunt Finding #3 / spec UI.md §2.3).
        show_toast(title, body);
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = (title, body); // unsupported platform — no-op
    }
}

/// Send a WinRT toast for the "rules.toml invalid" notification (P1.M4.T1.S2).
///
/// Fire-and-forget: swallows all errors (same posture as the former
/// `MessageBoxW`, but logs a `warn!` on the rare failure — matches `set_aumid`).
/// Runs on a SHORT-LIVED worker thread because every WinRT call needs the
/// calling thread COM-initialized (`CoInitializeEx`), and the window-event
/// thread that reaches `notify()` may already hold an incompatible (MTA)
/// apartment — a fresh STA worker avoids `RPC_E_CHANGED_MODE`. The worker exits
/// in <1 ms (`Show` is non-blocking), so this is NOT the former "leaks until
/// click" defect (bug-hunt Finding #3). The toast actually renders only once a
/// Start Menu shortcut advertises `APP_AUMID` (P1.M4.T2.S1); until then `Show`
/// returns `Ok(())` and nothing is visible — by design (research §Q7).
#[cfg(target_os = "windows")]
fn show_toast(title: &str, body: &str) {
    // Own the strings for the moved closure (notify's &str don't outlive it).
    let title = title.to_string();
    let body = body.to_string();
    std::thread::spawn(move || {
        // `::windows` = the windows-rs crate (bare `windows` here is the submodule).
        use ::windows::core::HSTRING;
        use ::windows::Data::Xml::Dom::XmlDocument;
        use ::windows::Win32::System::Com::{
            CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
        };
        use ::windows::UI::Notifications::{ToastNotification, ToastNotificationManager};

        // 1. COM apartment on THIS thread (STA). Required for every WinRT call
        //    below; `let _ =` because S_FALSE (already-init) is benign and
        //    RPC_E_CHANGED_MODE is impossible on a fresh thread. Same thread
        //    does all WinRT work + Show (apartments don't transfer).
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }

        // 2. Fire-and-forget: build XML → load → wrap → notifier → show. Log a
        //    warn on failure (matches set_aumid's posture; init_logging ran first).
        let res = (|| -> ::windows::core::Result<()> {
            let xml = build_toast_xml(&title, &body);
            let doc = XmlDocument::new()?;
            doc.LoadXml(&HSTRING::from(xml.as_str()))?;
            let toast = ToastNotification::CreateToastNotification(&doc)?;
            let notifier =
                ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_AUMID))?;
            notifier.Show(&toast)?;
            Ok(())
        })();
        if let Err(e) = res {
            log::warn!("show_toast: toast failed: {e}");
        }

        // 3. Release COM objects (RAII: they dropped when the closure above
        //    returned) then uninitialize. Optional (thread exit cleans up
        //    anyway) but tidy and matches the init.
        unsafe {
            CoUninitialize();
        }
    });
}

/// Build the `ToastText02` toast XML (bold title + wrapped body) for `show_toast`.
/// Factored out so the cfg(windows) unit test can verify escaping + parseability
/// without calling `Show` (no shortcut needed).
#[cfg(target_os = "windows")]
fn build_toast_xml(title: &str, body: &str) -> String {
    format!(
        "<toast><visual><binding template=\"ToastText02\">\
         <text id=\"1\">{}</text>\
         <text id=\"2\">{}</text>\
         </binding></visual></toast>",
        xml_escape(title),
        xml_escape(body),
    )
}

/// Escape a string for XML element content. The toast body is a TOML parse
/// error that may contain `& " < > '` — unescaped, `LoadXml` rejects it and the
/// toast silently never fires. Escape `&` first to avoid double-escaping.
#[cfg(target_os = "windows")]
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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

#[cfg(all(test, target_os = "windows"))]
mod toast_tests {
    use super::*;

    /// The toast XML must be well-formed AND correctly escaped for the body's
    /// special characters (the body is an arbitrary TOML parse-error string).
    /// We verify by loading it into a real XmlDocument — the same parse
    /// `show_toast` performs — WITHOUT calling `Show` (no shortcut needed, no
    /// toast pops during `cargo test`).
    ///
    /// NOTE: runs only on Windows (the implementing agent is on Linux, so this
    /// is a DEFERRED gate — see PRP Validation Level 5 / AGENTS.md Windows dev
    /// loop).
    #[test]
    fn toast_xml_is_well_formed_and_escapes_special_chars() {
        // `::windows` = the windows-rs crate (bare `windows` here is the submodule).
        use ::windows::core::HSTRING;
        use ::windows::Data::Xml::Dom::XmlDocument;
        use ::windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};

        // XmlDocument::new() is a WinRT activation → needs COM on this test
        // thread (cargo test may run the test on a worker thread with no
        // apartment).
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }

        // A realistic TOML-error body with the chars that break XML if unescaped.
        let xml = build_toast_xml(
            "QMKonnect: rules.toml invalid",
            "expected `=` at line 5 & column 3, found \"<weird>\"",
        );
        let doc = XmlDocument::new().expect("XmlDocument::new");
        doc.LoadXml(&HSTRING::from(xml.as_str()))
            .expect("toast XML must parse after xml_escape");
        // If LoadXml returned Ok, the XML is well-formed and escaping is correct.
    }
}

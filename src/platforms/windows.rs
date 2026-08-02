#![cfg(target_os = "windows")]
use crate::core::notifier;
use crate::core::types::WindowInfo;
use crate::platforms::WindowMonitor;
use std::error::Error;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, GetClassNameW, GetForegroundWindow, GetWindowTextW,
    GetWindowThreadProcessId, EVENT_OBJECT_FOCUS, EVENT_OBJECT_NAMECHANGE, WINEVENT_OUTOFCONTEXT,
};

// Thread-safe replacements for the former `static mut` globals (issue #5).
// AtomicPtr is unconditionally Send+Sync, so it can hold the hook handle without
// depending on whether HWINEVENTHOOK itself is Send.
static G_VERBOSE: AtomicBool = AtomicBool::new(false);
static G_HOOK: AtomicIsize = AtomicIsize::new(0);
// Handle for the separate EVENT_OBJECT_NAMECHANGE hook. Kept distinct from
// G_HOOK (the focus hook) because NAMECHANGE must be hooked on its own range
// — folding both into one `SetWinEventHook(eventMin, eventMax, …)` would hook
// every event *between* them and flood the callback. See `start()`.
static G_NAME_HOOK: AtomicIsize = AtomicIsize::new(0);
static LAST_WINDOW_INFO: Mutex<Option<(String, String)>> = Mutex::new(None);

pub struct WindowsMonitor {
    verbose: bool,
    running: Arc<AtomicBool>,
}

impl WindowsMonitor {
    pub fn new(verbose: bool) -> Self {
        Self {
            verbose,
            running: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl WindowMonitor for WindowsMonitor {
    fn platform_name(&self) -> &str {
        "Windows"
    }

    fn start(&mut self) -> Result<(), Box<dyn Error>> {
        if self.verbose {
            println!("Starting Windows window monitor");
        }

        G_VERBOSE.store(self.verbose, Ordering::SeqCst);

        unsafe {
            let _h_instance = GetModuleHandleA(None).unwrap_or_default();
            let hook = SetWinEventHook(
                EVENT_OBJECT_FOCUS,
                EVENT_OBJECT_FOCUS,
                None, // Use None for h_instance when using WINEVENT_OUTOFCONTEXT
                Some(event_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            );

            if hook.0 == 0 {
                return Err("Failed to set up Windows event hook".into());
            }

            G_HOOK.store(hook.0, Ordering::SeqCst);

            // Hook element/window NAMECHANGE too so in-app title edits are
            // surfaced: a browser tab switch, a document/sheet change, or any
            // title edit within an ALREADY-FOCUSED app changes the title without
            // a focus transition, so EVENT_OBJECT_FOCUS never fires and the
            // foreground window's new title would otherwise go unreported
            // (title-pattern host rules like `["*chrome*","*youtube*"]` would
            // silently stop reacting as the user tabs around inside Chrome).
            // NAMECHANGE fires for the element whose name changed — often a
            // CHILD window — so `event_proc` re-derives the FOREGROUND window for
            // this event instead of trusting the event's own hwnd. Failure is
            // non-fatal: the focus hook + poller still cover focus transitions,
            // so we only lose the in-app title edge.
            let name_hook = SetWinEventHook(
                EVENT_OBJECT_NAMECHANGE,
                EVENT_OBJECT_NAMECHANGE,
                None,
                Some(event_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            );
            if name_hook.0 != 0 {
                G_NAME_HOOK.store(name_hook.0, Ordering::SeqCst);
            } else if self.verbose {
                eprintln!(
                    "Warning: failed to hook EVENT_OBJECT_NAMECHANGE; \
                     in-app title changes may be missed"
                );
            }

            if self.verbose {
                println!("Windows event hook established successfully");
            }

            // Initial notification for the currently active window
            handle_focus_change(GetForegroundWindow());
        }

        self.running.store(true, Ordering::SeqCst);

        // Background thread polls for window changes as a fallback. The WinEvent
        // hook (focus + name-change) is the primary, low-latency signal; this
        // poller catches transitions the hook misses (notably apps that don't
        // emit EVENT_OBJECT_NAMECHANGE for in-window title edits). It calls
        // `handle_focus_change` unconditionally each tick: `handle_focus_change`
        // derives the foreground window's (class, title) and dedups via
        // `LAST_WINDOW_INFO`, so this naturally surfaces BOTH focus changes and
        // in-window title changes. (The previous form gated on HWND equality,
        // which skipped any title change on the same top-level window.)
        let running = Arc::clone(&self.running);
        let verbose = self.verbose;
        thread::spawn(move || {
            if verbose {
                println!("Starting Windows polling thread as fallback");
            }

            while running.load(Ordering::SeqCst) {
                unsafe {
                    let current_hwnd = GetForegroundWindow();
                    if current_hwnd.0 != 0 {
                        handle_focus_change(current_hwnd);
                    }
                }
                thread::sleep(Duration::from_millis(100));
            }

            if verbose {
                println!("Windows polling thread stopped");
            }
        });

        if self.verbose {
            println!("Windows monitor started - events will be processed automatically");
        }

        Ok(())
    }

    fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        if self.verbose {
            println!("Stopping Windows window monitor");
        }
        let hook = G_HOOK.swap(0, Ordering::SeqCst);
        if hook != 0 {
            unsafe {
                let _ = UnhookWinEvent(HWINEVENTHOOK(hook));
            }
        }
        let name_hook = G_NAME_HOOK.swap(0, Ordering::SeqCst);
        if name_hook != 0 {
            unsafe {
                let _ = UnhookWinEvent(HWINEVENTHOOK(name_hook));
            }
        }
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }
}

unsafe extern "system" fn event_proc(
    _h_win_event_hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _id_event_thread: u32,
    _dwms_event_time: u32,
) {
    // A NAMECHANGE fires for the element whose name changed — frequently a
    // CHILD window (e.g. a browser tab's document pane) rather than the
    // top-level foreground window whose title we report. Re-derive the
    // foreground window for this event so `handle_focus_change` reports the
    // foreground app's (possibly new) title; its `LAST_WINDOW_INFO` dedup
    // suppresses no-op re-reports when the foreground title didn't actually
    // change. Focus events pass their own hwnd (it IS the focused window).
    let target = if event == EVENT_OBJECT_NAMECHANGE {
        GetForegroundWindow()
    } else {
        hwnd
    };
    handle_focus_change(target);
}

fn handle_focus_change(hwnd: HWND) {
    let verbose = G_VERBOSE.load(Ordering::SeqCst);

    if let Ok(Some(window_info)) = get_window_info(hwnd) {
        // Filter out Windows internal components and empty windows
        if should_ignore_window(&window_info) {
            if verbose {
                println!(
                    "Ignoring internal window - Class: '{}', Title: '{}'",
                    window_info.app_class, window_info.title
                );
            }
            return;
        }

        // Check if this is the same window as last time to prevent feedback loops
        let current_window = (window_info.app_class.clone(), window_info.title.clone());
        let duplicate = {
            let mut last = LAST_WINDOW_INFO.lock().unwrap();
            let is_dup = last.as_ref() == Some(&current_window);
            if !is_dup {
                *last = Some(current_window);
            }
            is_dup
        };

        if duplicate {
            if verbose {
                println!(
                    "Duplicate window event ignored - Class: '{}', Title: '{}'",
                    window_info.app_class, window_info.title
                );
            }
            return;
        }

        if verbose {
            println!(
                "Window focus changed - Class: '{}', Title: '{}'",
                window_info.app_class, window_info.title
            );
        }

        if let Err(e) = notifier::notify_qmk(&window_info, verbose) {
            eprintln!("Failed to notify QMK: {}", e);
        }
    }
}

fn should_ignore_window(window_info: &WindowInfo) -> bool {
    // Filter out Windows internal components.
    //
    // NOTE: `ApplicationFrameWindow` (the UWP shell frame) and the hosted
    // `Windows.UI.Core.CoreWindow` content window are deliberately NOT in
    // this list. `get_window_info` already resolves the frame to its real
    // content window, so by the time we get here a UWP app reports as its
    // content class with a meaningful title ("Calculator", "Settings", ...).
    // Re-filtering `CoreWindow` here would throw that resolved content away
    // -- the original bug that hid every UWP app. Stray top-level `CoreWindow`
    // shells without a frame are rare and carry empty titles, so they are
    // caught by the empty-title rule below instead.
    let ignore_classes = [
        "ForegroundStaging",
        "XamlExplorerHostIslandWindow",
        "Windows.UI.Composition.DesktopWindowContentBridge",
        "Windows.UI.Input.InputSite.WindowClass",
        "TaskSwitcherWnd",
        "TaskSwitcherOverlayWnd",
        // Shell / tray chrome hosted by explorer.exe. These are never an app
        // the user is "using", but they are top-level windows that grab
        // foreground briefly when opened — e.g. clicking the tray-overflow
        // chevron to reach a hidden icon focuses the overflow flyout, which
        // would otherwise be reported (and sent to the keyboard). We identify
        // them by window CLASS, not title: titles are locale-dependent, so a
        // non-English Windows would defeat a title match. Both generations are
        // covered: Win11 (XAML island) and Win10 (classic).
        "TopLevelWindowForOverflowXamlIsland", // Win11 tray-overflow flyout
        "NotifyIconOverflowWindow",            // Win10 tray-overflow flyout
        "Shell_TrayWnd",                       // taskbar (also caught by empty-title)
        "Shell_SecondaryTrayWnd",              // Win11 secondary-monitor taskbar
    ];

    if ignore_classes
        .iter()
        .any(|&class| window_info.app_class == class)
    {
        return true;
    }

    if window_info.title.is_empty() {
        // Allow some specific classes even with empty titles (like some games or tools)
        let allow_empty_title = [
            "CASCADIA_HOSTING_WINDOW_CLASS", // Terminal apps
            "Chrome_WidgetWin_1",            // Chrome/Electron apps
        ];

        if !allow_empty_title
            .iter()
            .any(|&class| window_info.app_class == class)
        {
            return true;
        }
    }

    // Ignore very short titles that are likely not real applications
    if window_info.title.len() < 2 && !window_info.title.is_empty() {
        return true;
    }

    false
}

/// Class name of the outer shell window that hosts every UWP/Store app
/// (Calculator, Settings, Photos, Weather, ...). The frame is owned by
/// `ApplicationFrameHost.exe`; the real app content lives in a descendant
/// window owned by the app's own process.
const APPLICATION_FRAME_CLASS: &str = "ApplicationFrameWindow";

/// State carried into the `EnumChildWindows` callback when resolving the
/// content window hosted by an `ApplicationFrameWindow`.
struct ContentWindowSearch {
    /// Process ID of the `ApplicationFrameHost` frame window. The real content
    /// window belongs to a *different* process (the app itself), so any
    /// descendant sharing this PID is just frame chrome and must be skipped.
    frame_pid: u32,
    /// First matching content window found, if any.
    found: Option<HWND>,
}

/// Given a focused `ApplicationFrameWindow`, locate the hosted UWP content
/// window.
///
/// UWP apps (Calculator, Settings, Photos, ...) are wrapped in a shell frame
/// (`ApplicationFrameWindow`, owned by `ApplicationFrameHost.exe`). The
/// actual app content is a descendant `Windows.UI.Core.CoreWindow` (or
/// similar) owned by the app's own process. We walk descendants and pick the
/// first visible descendant window that belongs to a process other than the
/// frame's. Returns the content `HWND`, or `None` if none was found (the
/// frame may briefly have no child during launch/teardown).
unsafe fn find_uwp_content_window(frame_hwnd: HWND) -> Option<HWND> {
    let mut frame_pid: u32 = 0;
    GetWindowThreadProcessId(frame_hwnd, Some(&mut frame_pid as *mut u32));

    let mut search = ContentWindowSearch {
        frame_pid,
        found: None,
    };

    let _ = EnumChildWindows(
        frame_hwnd,
        Some(content_window_proc),
        windows::Win32::Foundation::LPARAM(&mut search as *mut _ as isize),
    );

    search.found
}

/// `EnumChildWindows` callback for `find_uwp_content_window`. See that
/// function for the selection criteria.
unsafe extern "system" fn content_window_proc(
    hwnd: HWND,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::BOOL {
    use windows::Win32::UI::WindowsAndMessaging::IsWindowVisible;

    let search = &mut *(lparam.0 as *mut ContentWindowSearch);

    if !IsWindowVisible(hwnd).as_bool() {
        return windows::Win32::Foundation::BOOL(1); // continue
    }

    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid as *mut u32));

    if pid != search.frame_pid && pid != 0 {
        // First visible descendant owned by a different process than the frame.
        search.found = Some(hwnd);
        return windows::Win32::Foundation::BOOL(0); // stop
    }

    windows::Win32::Foundation::BOOL(1) // continue
}

fn get_window_info(hwnd: HWND) -> Result<Option<WindowInfo>, Box<dyn Error>> {
    unsafe {
        if hwnd.0 == 0 {
            return Ok(None);
        }

        let mut class_name_w: [u16; 256] = [0; 256];
        let class_name_len = GetClassNameW(hwnd, &mut class_name_w);
        let app_class = if class_name_len > 0 {
            let os_string = OsString::from_wide(&class_name_w[..class_name_len as usize]);
            os_string.to_string_lossy().into_owned()
        } else {
            String::new()
        };

        // UWP/Store apps (Calculator, Settings, Photos, ...) are hosted inside
        // an `ApplicationFrameWindow` shell. When focus lands on that frame,
        // resolve and report the *actual* app content window instead: the frame
        // itself carries only a generic class and an often-empty title, and is
        // explicitly filtered out by `should_ignore_window`. The content
        // window has a meaningful title ("Calculator", "Settings", ...) and a
        // more specific class.
        let (report_hwnd, report_class) = if app_class == APPLICATION_FRAME_CLASS {
            if let Some(content_hwnd) = find_uwp_content_window(hwnd) {
                let mut content_class_w: [u16; 256] = [0; 256];
                let content_class_len = GetClassNameW(content_hwnd, &mut content_class_w);
                let content_class = if content_class_len > 0 {
                    let os_string =
                        OsString::from_wide(&content_class_w[..content_class_len as usize]);
                    os_string.to_string_lossy().into_owned()
                } else {
                    app_class.clone()
                };
                (content_hwnd, content_class)
            } else {
                // No hosted content (frame is mid-launch/teardown) — report
                // nothing rather than the empty frame.
                return Ok(None);
            }
        } else {
            (hwnd, app_class)
        };

        let mut window_text_w: [u16; 512] = [0; 512];
        let window_text_len = GetWindowTextW(report_hwnd, &mut window_text_w);
        let title = if window_text_len > 0 {
            let os_string = OsString::from_wide(&window_text_w[..window_text_len as usize]);
            // Trim surrounding whitespace: some windows pad their title with
            // dozens of trailing spaces (e.g. terminal host titles), which
            // would otherwise bloat the HID message AND the tray dialog's
            // per-row Copy output.
            os_string.to_string_lossy().trim().to_owned()
        } else {
            String::new()
        };

        Ok(Some(WindowInfo::new(report_class, title)))
    }
}

/// Enumerate all top-level foreground windows.
///
/// Returns `(class, title)` for every visible window that QMKonnect itself
/// would report — i.e. it applies the same `should_ignore_window` filter used
/// by the live monitor, so the list shows exactly the values you can match
/// against in your QMK config.
pub fn list_foreground_windows() -> Vec<(String, String)> {
    let mut result: Vec<(String, String)> = Vec::new();

    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::EnumWindows(
            Some(enum_windows_proc),
            windows::Win32::Foundation::LPARAM(&mut result as *mut _ as isize),
        );
    }

    result
}

unsafe extern "system" fn enum_windows_proc(
    hwnd: HWND,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::BOOL {
    use windows::Win32::UI::WindowsAndMessaging::IsWindowVisible;

    let result = &mut *(lparam.0 as *mut Vec<(String, String)>);

    if !IsWindowVisible(hwnd).as_bool() {
        return windows::Win32::Foundation::BOOL(1);
    }

    if let Ok(Some(info)) = get_window_info(hwnd) {
        if !should_ignore_window(&info) {
            result.push((info.app_class, info.title));
        }
    }

    // 1 == continue enumeration.
    windows::Win32::Foundation::BOOL(1)
}

// Windows-specific configuration path handling
pub fn get_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Primary location: %APPDATA%\QMKonnect\config.toml
    if let Ok(app_data) = std::env::var("APPDATA") {
        paths.push(
            PathBuf::from(app_data)
                .join("QMKonnect")
                .join("config.toml"),
        );
    }

    // Secondary location: %LOCALAPPDATA%\QMKonnect\config.toml
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        paths.push(
            PathBuf::from(local_app_data)
                .join("QMKonnect")
                .join("config.toml"),
        );
    }

    // Fallback to executable directory
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            paths.push(exe_dir.join("config.toml"));
        }
    }

    paths
}

// Create Windows configuration directory
pub fn create_config_dir() -> Result<PathBuf, Box<dyn Error>> {
    // Use %APPDATA% for user configuration
    let config_dir = if let Ok(app_data) = std::env::var("APPDATA") {
        PathBuf::from(app_data).join("QMKonnect")
    } else {
        return Err("Could not determine APPDATA directory".into());
    };

    std::fs::create_dir_all(&config_dir)?;

    Ok(config_dir)
}

#![allow(unexpected_cfgs)]
#![cfg(target_os = "macos")]
use crate::core::notifier;
use crate::core::types::WindowInfo;
use crate::platforms::WindowMonitor;
use std::error::Error;
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use core_foundation::{
    array::CFArray,
    base::{CFRange, CFRelease, TCFType},
    dictionary::CFDictionaryRef,
    runloop::CFRunLoopRun,
    string::CFString,
};

use core_graphics::window::{kCGWindowListOptionOnScreenOnly, CGWindowListCopyWindowInfo};

use objc::{class, msg_send, runtime::Object, sel, sel_impl};

// Existing extern block for various symbols
extern "C" {
    static NSWorkspaceDidActivateApplicationNotification: *const Object;
    static kCGWindowOwnerName: *const c_void;
    static kCGWindowName: *const c_void;
}

// Screen recording permissions (macOS 10.15+):
extern "C" {
    /// Returns true if the app already has screen recording permission.
    fn CGPreflightScreenCaptureAccess() -> bool;
    /// Requests screen recording permission by displaying the system modal prompt.
    /// Returns immediately (it does not wait for the user's response).
    fn CGRequestScreenCaptureAccess() -> bool;
}

// Define nil as a null pointer
const NIL: *mut Object = std::ptr::null_mut();

// Global verbose setting readable from the Objective-C notification callback.
// Replacing the former `static mut VERBOSE` (data race / UB) with an atomic.
static VERBOSE: AtomicBool = AtomicBool::new(false);

// Channel that hands the frontmost-window info captured by the AppKit
// notification observer (which fires on the MAIN thread) to a dedicated
// background worker that runs [`notifier::notify_qmk`].
//
// WHY this exists (status-tray-freeze-after-sleep bug):
// `notify_qmk` performs synchronous HID I/O (`qmk_notifier` -> `hidapi`). On
// macOS, hidapi schedules its IOHIDManager on `CFRunLoopGetCurrent()`
// (hid.c:442/934) and, during enumeration, explicitly spins that run loop via
// `CFRunLoopRunInMode` (hid.c:495). AppKit delivers
// `NSWorkspaceDidActivateApplicationNotification` to our observer ON THE MAIN
// THREAD. So calling `notify_qmk` straight from the observer makes the HID
// enumerate/read re-enter the main CFRunLoop; that re-entrant spin re-delivers
// any queued app-activation notifications (which accumulate while the Mac
// sleeps / sits idle), re-entering the observer while `notify_qmk` still holds
// the global `NOTIFIER` Mutex. `std::sync::Mutex` is non-reentrant, so the
// re-entrant `notify_qmk` blocks forever on a lock the same thread already
// holds: the main thread is wedged and the menu-bar item stops responding. (A
// live `sample` of the wedged process shows exactly this — the main thread
// parked in `__psynch_mutexwait` beneath a re-entrant
// `applicationStatusSubsystemCallback` / LaunchServices notification block.)
// Routing the event to a worker thread means the main thread never touches HID
// and never re-enters its run loop from inside the observer, breaking the
// deadlock at its root.
static NOTIFY_TX: std::sync::OnceLock<std::sync::mpsc::SyncSender<WindowInfo>> =
    std::sync::OnceLock::new();

pub struct MacOSMonitor {
    verbose: bool,
    running: bool,
}

impl MacOSMonitor {
    pub fn new(verbose: bool) -> Self {
        Self {
            verbose,
            running: false,
        }
    }

    /// Returns true when screen-recording permission is already granted.
    /// When not granted, kicks off the async permission request but does NOT
    /// block: window titles are redacted until the user grants access, while
    /// the frontmost application name keeps working. We therefore keep running
    /// (see issue #13) instead of hard-failing.
    fn ensure_screen_recording_permission(verbose: bool) -> bool {
        unsafe {
            if CGPreflightScreenCaptureAccess() {
                if verbose {
                    println!("Screen recording permission already granted.");
                }
                return true;
            }

            if verbose {
                println!(
                    "Screen recording permission not yet granted — requesting. \
                     Window titles will be unavailable until access is granted \
                     in System Settings > Privacy & Security > Screen Recording."
                );
            }
            // Pops the system dialog (returns immediately). Keep running either way.
            CGRequestScreenCaptureAccess();
            false
        }
    }

    fn setup_observers(&mut self) -> Result<(), Box<dyn Error>> {
        // Publish the verbose flag for the Objective-C callback.
        VERBOSE.store(self.verbose, Ordering::SeqCst);

        // Spawn the off-main-thread notify worker exactly once. See NOTIFY_TX
        // for why notify_qmk must never run on the main thread.
        if NOTIFY_TX.get().is_none() {
            let (tx, rx) = std::sync::mpsc::sync_channel::<WindowInfo>(64);
            if NOTIFY_TX.set(tx).is_ok() {
                std::thread::spawn(move || {
                    while let Ok(window_info) = rx.recv() {
                        let verbose = VERBOSE.load(Ordering::SeqCst);
                        let _ = notifier::notify_qmk(&window_info, verbose);
                    }
                });
            }
        }

        let workspace: *mut Object = unsafe { msg_send![class!(NSWorkspace), sharedWorkspace] };
        let notification_center: *mut Object = unsafe { msg_send![workspace, notificationCenter] };

        // Register a custom observer class once. Recreating it on every start()
        // would fail (class already registered), so skip declaration if present.
        use objc::declare::ClassDecl;
        use objc::runtime::{Class, Sel};

        if Class::get("RustNotificationObserver").is_none() {
            let superclass = Class::get("NSObject")
                .ok_or("NSObject class not found — Objective-C runtime unavailable")?;
            let mut decl = ClassDecl::new("RustNotificationObserver", superclass)
                .ok_or("Failed to declare RustNotificationObserver class")?;

            extern "C" fn notification_handler(_: &Object, _: Sel, _: *mut Object) {
                // Capture the frontmost window on the main thread (cheap:
                // NSWorkspace + CGWindowList — no HID, no locks) and hand it to
                // the background worker. See NOTIFY_TX for why `notify_qmk`
                // must NOT run on this thread.
                if let Ok(Some(window_info)) = get_active_window_info() {
                    if let Some(tx) = NOTIFY_TX.get() {
                        // Non-blocking: if the worker is busy and the bounded
                        // queue is full, drop the event. Rapid switches are
                        // coalesced by the debouncer anyway, and the main
                        // thread must never block here.
                        let _ = tx.try_send(window_info);
                    }
                }
            }

            unsafe {
                decl.add_method(
                    sel!(observeNotification:),
                    notification_handler as extern "C" fn(&Object, Sel, *mut Object),
                );
            }
            decl.register();
        }

        // Create an instance of our custom class
        let observer: *mut Object = unsafe {
            msg_send![
                Class::get("RustNotificationObserver")
                    .ok_or("RustNotificationObserver class missing after registration")?,
                new
            ]
        };

        // Add the observer to the notification center
        let _: () = unsafe {
            msg_send![notification_center,
                addObserver: observer
                selector: sel!(observeNotification:)
                name: NSWorkspaceDidActivateApplicationNotification
                object: NIL
            ]
        };

        // Don't release the observer; it must stay alive for the run loop.
        let _ = observer;

        self.running = true;
        Ok(())
    }
}

impl WindowMonitor for MacOSMonitor {
    fn platform_name(&self) -> &str {
        "macOS"
    }

    fn start(&mut self) -> Result<(), Box<dyn Error>> {
        if self.verbose {
            println!("Starting macOS window monitor");
        }

        // Degrade gracefully (issue #13): a missing screen-recording permission
        // only costs us window *titles*; the app name still works, so keep going.
        let _permission_granted = Self::ensure_screen_recording_permission(self.verbose);

        self.setup_observers()?;

        // Capture the initial active application.
        let _ = get_active_window_info().map(|info| {
            if let Some(window_info) = info {
                if let Err(e) = notifier::notify_qmk(&window_info, self.verbose) {
                    eprintln!("Failed to notify QMK: {}", e);
                }
            }
        });

        // Pump the run loop on this thread. The notification observer fires on
        // activation changes and pushes updates to QMK.
        unsafe { CFRunLoopRun() };

        Ok(())
    }

    fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        self.running = false;
        // Stopping CFRunLoop from another thread is best-effort; the process
        // exit path handles cleanup. Kept for API symmetry with the trait.
        Ok(())
    }
}

fn get_active_window_info() -> Result<Option<WindowInfo>, Box<dyn Error>> {
    unsafe {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let app: *mut Object = msg_send![workspace, frontmostApplication];

        if app.is_null() {
            return Ok(None);
        }

        let app_name: *mut Object = msg_send![app, localizedName];
        let app_name_str = nsstring_to_string(app_name);

        // Window titles come from CGWindowListCopyWindowInfo, which requires
        // screen-recording permission. Without it the title is simply empty.
        let window_list = CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, 0);
        let window_array: CFArray<core_foundation::dictionary::CFDictionary> =
            CFArray::wrap_under_get_rule(window_list as *const _);
        let count = window_array.len();

        let mut window_title = String::new();

        for i in 0..count {
            let range = CFRange {
                location: i as isize,
                length: 1,
            };
            let info = window_array.get_values(range)[0] as CFDictionaryRef;

            let owner_name_ref = core_foundation::dictionary::CFDictionaryGetValue(
                info as CFDictionaryRef,
                kCGWindowOwnerName as *const _,
            );

            if owner_name_ref.is_null() {
                continue;
            }

            let owner_name = CFString::wrap_under_get_rule(owner_name_ref as *const _);
            let owner_name_str = cfstring_to_string(&owner_name);

            if owner_name_str == app_name_str {
                let window_name_ref = core_foundation::dictionary::CFDictionaryGetValue(
                    info as CFDictionaryRef,
                    kCGWindowName as *const _,
                );

                if !window_name_ref.is_null() {
                    let window_name = CFString::wrap_under_get_rule(window_name_ref as *const _);
                    window_title = cfstring_to_string(&window_name);
                }

                break;
            }
        }

        CFRelease(window_list as *const c_void);

        Ok(Some(WindowInfo::new(app_name_str, window_title)))
    }
}

/// Enumerate currently-running foreground applications.
///
/// Returns one entry per running app with a *regular* activation policy
/// (the apps that appear in the Dock / app switcher). Each entry is
/// `(class, title)` where:
///   * `class` is the app's `localizedName` — exactly the value QMKonnect sends
///     as `application_class` and that you match against in your QMK config.
///   * `title` is the title of one of the app's on-screen windows, looked up via
///     the Core Graphics window list (requires Screen Recording permission;
///     empty when unavailable).
///
/// Results are sorted alphabetically by class for easy scanning.
pub fn list_foreground_windows() -> Vec<(String, String)> {
    let mut result: Vec<(String, String)> = Vec::new();
    let title_map = build_owner_title_map();

    unsafe {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            return result;
        }

        let apps: *mut Object = msg_send![workspace, runningApplications];
        if apps.is_null() {
            return result;
        }
        let count: usize = msg_send![apps, count];

        for i in 0..count {
            let app: *mut Object = msg_send![apps, objectAtIndex: i as isize];
            if app.is_null() {
                continue;
            }

            // NSApplicationActivationPolicyRegular == 0 (apps shown to the user).
            let policy: isize = msg_send![app, activationPolicy];
            if policy != 0 {
                continue;
            }

            let is_finished: bool = msg_send![app, isFinishedLaunching];
            if !is_finished {
                continue;
            }

            let name_ns: *mut Object = msg_send![app, localizedName];
            let name = nsstring_to_string(name_ns);
            if name.is_empty() {
                continue;
            }

            let title = title_map.get(&name).cloned().unwrap_or_default();
            result.push((name, title));
        }
    }

    result.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    result
}

/// Build a map of `owner name -> first non-empty window title` from the
/// on-screen Core Graphics window list. Requires Screen Recording permission;
/// returns an empty map (and thus empty titles) when unavailable.
fn build_owner_title_map() -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();

    unsafe {
        let window_list = CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, 0);
        if window_list.is_null() {
            return map;
        }
        let window_array: CFArray<core_foundation::dictionary::CFDictionary> =
            CFArray::wrap_under_get_rule(window_list as *const _);
        let count = window_array.len();

        for i in 0..count {
            let range = CFRange {
                location: i as isize,
                length: 1,
            };
            let info = window_array.get_values(range)[0] as CFDictionaryRef;

            let owner_ref = core_foundation::dictionary::CFDictionaryGetValue(
                info as CFDictionaryRef,
                kCGWindowOwnerName as *const _,
            );
            if owner_ref.is_null() {
                continue;
            }
            let owner = CFString::wrap_under_get_rule(owner_ref as *const _).to_string();

            let title_ref = core_foundation::dictionary::CFDictionaryGetValue(
                info as CFDictionaryRef,
                kCGWindowName as *const _,
            );
            let title = if !title_ref.is_null() {
                CFString::wrap_under_get_rule(title_ref as *const _).to_string()
            } else {
                String::new()
            };

            // Keep the first non-empty title per owner.
            let needs_update = match map.get(&owner) {
                None => true,
                Some(existing) => existing.is_empty() && !title.is_empty(),
            };
            if needs_update {
                map.insert(owner, title);
            }
        }

        CFRelease(window_list as *const c_void);
    }

    map
}

fn nsstring_to_string(nsstring: *mut Object) -> String {
    unsafe {
        if nsstring.is_null() {
            return String::new();
        }
        let utf8: *const i8 = msg_send![nsstring, UTF8String];
        if utf8.is_null() {
            return String::new();
        }
        let len = libc::strlen(utf8);
        let bytes = std::slice::from_raw_parts(utf8 as *const u8, len);
        String::from_utf8_lossy(bytes).into_owned()
    }
}

fn cfstring_to_string(cf_string: &CFString) -> String {
    cf_string.to_string()
}

// macOS-specific configuration path handling
pub fn get_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Primary location: ~/Library/Application Support/QMKonnect/config.toml
    if let Some(home) = dirs::home_dir() {
        paths.push(
            home.join("Library")
                .join("Application Support")
                .join("QMKonnect")
                .join("config.toml"),
        );
    }

    // Secondary location: ~/.config/qmkonnect/config.toml (XDG-style fallback)
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".config").join("qmkonnect").join("config.toml"));
    }

    // System-wide config as last resort
    paths.push(PathBuf::from("/etc/qmkonnect/config.toml"));

    paths
}

// Create macOS configuration directory
pub fn create_config_dir() -> Result<PathBuf, Box<dyn Error>> {
    // Use ~/Library/Application Support/QMKonnect for macOS
    let config_dir = if let Some(home) = dirs::home_dir() {
        home.join("Library")
            .join("Application Support")
            .join("QMKonnect")
    } else {
        return Err("Could not determine home directory".into());
    };

    // Create directory if it doesn't exist
    std::fs::create_dir_all(&config_dir)?;

    Ok(config_dir)
}

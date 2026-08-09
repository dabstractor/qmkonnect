#![cfg(not(all(target_os = "linux", feature = "hyprland")))]
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder},
};

use tray_icon::{
    menu::{AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIconBuilder,
};

#[cfg(target_os = "macos")]
mod objc_types {
    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct NSPoint {
        pub x: f64,
        pub y: f64,
    }

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct NSSize {
        pub width: f64,
        pub height: f64,
    }

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct NSRect {
        pub origin: NSPoint,
        pub size: NSSize,
    }
}

enum UserEvent {
    MenuEvent(MenuEvent),
    /// Latest three-state device-status probe result, delivered from the
    /// background polling thread to refresh the macOS/Windows tray status line
    /// (line 2). Three states per `spec/UI.md` §4 / `spec/DEVICE_DISCOVERY.md`
    /// §3; the handshake lifecycle (Gain/Loss) is deliberately keyed on the
    /// Tier-1 presence bool separately from this payload.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    DeviceStatus(crate::core::notifier::DeviceStatus),
    /// Re-sync the macOS "Launch at Login" checkbox with the real SMAppService
    /// status, delivered after the deferred first-run register completes.
    #[cfg(target_os = "macos")]
    AutostartSync,
}

// Shared result slot for the Windows settings dialog, replacing the former
// Arc::into_raw + mem::forget leak (#9).
//
// The discovered-device picker (spec/DEVICE_DISCOVERY.md §5 / spec/UI.md §2.0)
// made this a struct: `chosen` is the listbox selection (a concrete `(vid,pid)`
// from `ClassifiedDevice`), `manual` is the typed hex pair from the Advanced
// fields (each field `None` ⇒ auto-discovery). The save path applies `chosen`
// first, else `manual`, else leaves the open-time config's VID/PID as-is.
#[cfg(target_os = "windows")]
#[derive(Clone, Default)]
struct DialogResult {
    chosen: Option<(u16, u16)>,
    manual: Option<(Option<u16>, Option<u16>)>,
}

#[cfg(target_os = "windows")]
static DIALOG_RESULT: std::sync::Mutex<Option<DialogResult>> = std::sync::Mutex::new(None);

// The LISTBOX row store for the discovered-device picker (spec/DEVICE_DISCOVERY.md
// §5). Mirrors `WINDOW_INFO_ROWS` below (the Win32 dialog proc is a free
// `extern "system" fn` with no per-call user data, so a static is the only way
// to carry the index→device mapping from `populate_device_picker` to the OK
// arm). Populated on initial open + [Rescan]; read by the OK arm (1003) to map
// a selected listbox index → `(vendor_id, product_id)`. Only one settings dialog
// is open at a time, so a single shared slot is sufficient.
#[cfg(target_os = "windows")]
static PICKER_DEVICES: std::sync::Mutex<Vec<crate::core::notifier::ClassifiedDevice>> =
    std::sync::Mutex::new(Vec::new());

// The dialog-OPEN config's vid/pid, captured before the controls are created so
// the [Rescan] arm (`settings_dialog_proc`) can re-evaluate the three picker
// cases (spec/DEVICE_DISCOVERY.md §5.1) without re-reading the config file —
// the user is mid-edit, so the live edit fields are NOT authoritative here.
#[cfg(target_os = "windows")]
static DIALOG_OPEN_VIDPID: std::sync::Mutex<(Option<u16>, Option<u16>)> =
    std::sync::Mutex::new((None, None));

// Control IDs for the discovered-device picker controls. The existing settings
// dialog uses 1001-1004 (VID EDIT, PID EDIT, OK, Cancel) and the window-info
// dialog uses 4001-4013 / 5000+ / 6000+ (see the comment block near
// `show_window_info_dialog`), so these new IDs sit safely between 1004 and 4001.
#[cfg(target_os = "windows")]
const IDC_DEVICE_LIST: i32 = 1010; // LISTBOX (the picker)
#[cfg(target_os = "windows")]
const IDC_RESCAN: i32 = 1011; // [Rescan] BUTTON
#[cfg(target_os = "windows")]
const IDC_ADVANCED_GROUP: i32 = 1012; // BS_GROUPBOX (visual frame; NEVER in WM_COMMAND)
#[cfg(target_os = "windows")]
const IDC_HEADER: i32 = 1013; // WC_STATICW (the "Detected:" / "No QMK..." header line)

/// Format an optional ID for display: its 4-digit hex, or "auto" when unset.
/// macOS-only: only the macOS settings dialog renders IDs into its message
/// text; the Windows dialog reads/writes the edit fields directly.
#[cfg(target_os = "macos")]
fn format_id_hex(id: Option<u16>) -> String {
    id.map(|v| format!("{:04x}", v))
        .unwrap_or_else(|| "auto".to_string())
}

/// Parse a hex ID from a settings-dialog field. Empty input or the literal
/// "auto" yield `None` (auto-discovery); otherwise `Some(value)`. Tolerates an
/// optional `0x` prefix and surrounding whitespace; anything else is an error.
#[cfg(any(target_os = "windows", target_os = "macos"))]
fn parse_id_field(input: &str) -> Result<Option<u16>, Box<dyn std::error::Error>> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto") {
        return Ok(None);
    }
    let hex_str = trimmed.trim_start_matches("0x").trim_start_matches("0X");
    u16::from_str_radix(hex_str, 16)
        .map(Some)
        .map_err(|e| format!("Invalid hex value '{}': {}", input, e).into())
}

// Rows displayed by the "Show Window Information..." dialog. The macOS
// copy-button target object and the Windows dialog WndProc both look up the row
// to copy by index from here. Only one such dialog is open at a time, so a
// single shared slot is sufficient.
#[cfg(any(target_os = "macos", target_os = "windows"))]
static WINDOW_INFO_ROWS: std::sync::Mutex<Vec<(String, String)>> =
    std::sync::Mutex::new(Vec::new());

// ===========================================================================
// Launch-at-login (macOS)
//
// Backed by SMAppService (ServiceManagement.framework, macOS 13+). It
// registers the .app bundle to open at login, shows up in System Settings ->
// General -> Login Items, and Quit always quits — the system relaunches only
// on the next login, never on exit. On macOS < 13 the class is absent and
// every call degrades to a no-op (checkbox reads off).
// ===========================================================================
#[cfg(target_os = "macos")]
mod autostart {
    use objc::runtime::{Class, Object};
    use objc::{msg_send, sel, sel_impl};
    use std::ptr;

    // Link ServiceManagement.framework so the SMAppService class is present in
    // the Obj-C runtime at launch (Class::get would otherwise return None and
    // we'd silently no-op).
    #[link(name = "ServiceManagement", kind = "framework")]
    extern "C" {}

    // SMAppServiceStatus raw values (ServiceManagement/SMAppService.h):
    //   0 = notRegistered, 1 = enabled, 2 = requiresApproval, 3 = notFound.
    const STATUS_ENABLED: isize = 1;

    /// True when the framework is present and the app is registered AND enabled.
    pub fn is_enabled() -> bool {
        unsafe {
            let service = match main_app_service() {
                Some(s) => s,
                None => return false,
            };
            let status: isize = msg_send![service, status];
            status == STATUS_ENABLED
        }
    }

    /// Register the main app bundle to launch at login.
    pub fn enable() -> Result<(), String> {
        unsafe {
            let service = main_app_service().ok_or_else(|| {
                "Launch at login requires macOS 13 (Ventura) or newer".to_string()
            })?;
            let mut err: *mut Object = ptr::null_mut();
            let ok: bool = msg_send![service, registerAndReturnError: &mut err];
            if ok {
                Ok(())
            } else {
                Err(nserror_description(err)
                    .unwrap_or_else(|| "Failed to enable launch at login".to_string()))
            }
        }
    }

    /// Unregister the login item.
    pub fn disable() -> Result<(), String> {
        unsafe {
            let service = main_app_service().ok_or_else(|| {
                "Launch at login requires macOS 13 (Ventura) or newer".to_string()
            })?;
            let mut err: *mut Object = ptr::null_mut();
            let ok: bool = msg_send![service, unregisterAndReturnError: &mut err];
            if ok {
                Ok(())
            } else {
                Err(nserror_description(err)
                    .unwrap_or_else(|| "Failed to disable launch at login".to_string()))
            }
        }
    }

    /// Shared SMAppService for the main app, or None when the class isn't
    /// available (macOS < 13).
    unsafe fn main_app_service() -> Option<*mut Object> {
        let cls = Class::get("SMAppService")?;
        let service: *mut Object = msg_send![cls, mainAppService];
        (!service.is_null()).then_some(service)
    }

    unsafe fn nserror_description(err: *mut Object) -> Option<String> {
        if err.is_null() {
            return None;
        }
        let desc: *mut Object = msg_send![err, localizedDescription];
        if desc.is_null() {
            return None;
        }
        let utf8: *const i8 = msg_send![desc, UTF8String];
        if utf8.is_null() {
            return None;
        }
        Some(
            std::ffi::CStr::from_ptr(utf8)
                .to_string_lossy()
                .into_owned(),
        )
    }
}

/// One-time default-on for launch-at-login. Registers the login item on the
/// very first launch only; a persisted marker ensures we never fight the user
/// afterwards (if they turn it off — in the tray or System Settings — we won't
/// re-enable it on the next launch).
///
/// Deferred onto the main run loop's next free iteration via `dispatch_async`
/// so the registration's XPC round-trip never blocks the launch-critical Init
/// callback, then signals the tray to re-sync the checkbox. SMAppService calls
/// must stay on the main thread, which the main serial queue guarantees.
#[cfg(target_os = "macos")]
fn autostart_first_run_default_on(proxy: tao::event_loop::EventLoopProxy<UserEvent>) {
    dispatch::Queue::main().exec_async(move || {
        if let Some(marker) = autostart_marker_path() {
            if !marker.exists() {
                if let Err(e) = autostart::enable() {
                    eprintln!("Could not enable launch at login by default: {}", e);
                }
                // Write the marker regardless of success so an unsupported OS
                // / transient failure doesn't retry (and log) on every launch.
                let _ = std::fs::write(&marker, b"1");
            }
        }
        // Reflect the (possibly just-changed) status in the tray checkbox.
        let _ = proxy.send_event(UserEvent::AutostartSync);
    });
}

#[cfg(target_os = "macos")]
fn autostart_marker_path() -> Option<std::path::PathBuf> {
    crate::platforms::create_config_dir()
        .ok()
        .map(|d| d.join(".autostart_initialized"))
}

/// Tray "Launch at Login" toggle handler. Derives the desired state from the
/// real system status (not the checkbox) so we're robust to any auto-toggle
/// behaviour in the menu backend, performs the (un)register, then mirrors the
/// outcome into the checkmark.
#[cfg(target_os = "macos")]
fn handle_launch_at_login_click(item: &tray_icon::menu::CheckMenuItem) {
    let want_on = !autostart::is_enabled();
    let result = if want_on {
        autostart::enable()
    } else {
        autostart::disable()
    };
    match result {
        Ok(()) => item.set_checked(want_on),
        Err(e) => show_macos_error_message(&format!("Could not change \"Launch at Login\": {}", e)),
    }
}

/// Tray "Open at Login" toggle handler (Windows). muda flips the native check
/// on click *before* dispatching the event, so [`CheckMenuItem::is_checked`]
/// is already the new desired state; we persist it, then re-derive the
/// checkmark from the real registry value so a failed write visibly reverts.
/// Mirrors the macOS [`handle_launch_at_login_click`].
#[cfg(target_os = "windows")]
fn handle_open_at_login_click(item: &tray_icon::menu::CheckMenuItem) {
    let want = item.is_checked();
    crate::autostart::set_enabled(want);
    // Reflect reality: no-op on success, reverts the check on failure.
    item.set_checked(crate::autostart::is_enabled());
}

#[allow(unused_variables, clippy::vec_init_then_push)]
pub fn setup_tray(verbose: bool) {
    // Use the standard tray-icon implementation for all platforms
    // The dock icon hiding is handled by Info.plist LSUIElement=true

    // `mut` is only needed for the macOS activation-policy call below; on
    // Windows/Linux the binding is never mutated, so suppress the lint there.
    #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    // Tray-only on macOS: an Accessory activation policy means no Dock icon
    // and no CMD-Tab entry. LSUIElement in Info.plist sets the launch-time
    // policy, but tao's runtime default is Regular — in
    // applicationDidFinishLaunching it calls [NSApp setActivationPolicy:Regular]
    // (app_state.rs `apply_activation_policy`), overriding LSUIElement and
    // promoting us to a foreground app. Set Accessory here, before run(),
    // which is the only place tao honors it. Accessory apps can still surface
    // windows (Settings, Window Info dialogs) transiently, then return to the
    // background when the dialog closes.
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
    }

    let proxy = event_loop.create_proxy();

    MenuEvent::set_event_handler(Some({
        let proxy = proxy.clone();
        move |event| {
            let _ = proxy.send_event(UserEvent::MenuEvent(event));
        }
    }));

    let tray_menu = Menu::new();

    let settings_i = MenuItem::new("Settings", true, None);
    // "Edit rules" — open rules.toml in the system editor (HOST_RULES.md §7).
    // Sits in the prefs group with Settings (ungated, like Settings/Quit, so the
    // fallback X11-Linux build gets it too).
    let edit_rules_i = MenuItem::new("Edit rules", true, None);
    let quit_i = MenuItem::new("Quit", true, None);

    // "Launch at Login" toggle (macOS only; backed by SMAppService). The
    // initial checkmark is a placeholder — it is synced from the real system
    // status in the Init handler below.
    #[cfg(target_os = "macos")]
    let launch_at_login_i =
        tray_icon::menu::CheckMenuItem::new("Launch at Login", true, false, None);

    // "Open at Login" toggle (Windows only; backed by the HKCU `Run` key — see
    // src/autostart.rs). The initial checkmark reflects the real registry
    // state right now, so the first paint is already correct.
    #[cfg(target_os = "windows")]
    let open_at_login_i = tray_icon::menu::CheckMenuItem::new(
        "Open at Login",
        true,
        crate::autostart::is_enabled(),
        None,
    );

    // macOS/Windows show the configured device's connection status as line 2
    // of the tray menu. It is a disabled (non-clickable) label kept fresh by
    // the background probe thread below. Initial text reflects a synchronous
    // probe so the first paint is already correct.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let device_status_i = MenuItem::new(
        device_status_text(crate::core::notifier::device_status()),
        false, // disabled (non-clickable label) — the "No module" warning stays a disabled item
        None,
    );

    // macOS / Windows hide window class/title from the user, so the tray offers
    // a dedicated "Show Window Information..." entry to discover them. Linux
    // exposes them readily, so it is omitted there.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let window_info_i = MenuItem::new("Show Window Information...", true, None);
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let sep_wininfo = PredefinedMenuItem::separator();

    let about_i = PredefinedMenuItem::about(
        None,
        Some(AboutMetadata {
            name: Some("QMKonnect".to_string()),
            copyright: Some("Copyright Mulletware 2026".to_string()),
            ..Default::default()
        }),
    );
    let sep_about = PredefinedMenuItem::separator();
    let sep_before_quit = PredefinedMenuItem::separator();

    let mut menu_items: Vec<&dyn tray_icon::menu::IsMenuItem> = Vec::new();
    menu_items.push(&about_i);

    // Line 2: device-connection status (macOS/Windows).
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    menu_items.push(&device_status_i);

    menu_items.push(&sep_about);

    // Launch-at-login toggle sits in the "prefs" group with Settings.
    #[cfg(target_os = "macos")]
    menu_items.push(&launch_at_login_i);

    menu_items.push(&settings_i);

    // Windows "Open at Login" sits in the prefs group right under Settings
    // (… Settings → Open at Login → sep → Show Window Information …).
    #[cfg(target_os = "windows")]
    menu_items.push(&open_at_login_i);

    // "Edit rules" sits in the prefs group right after Settings (+ "Open at
    // Login" on Windows), before the Show-Window-Information separator.
    menu_items.push(&edit_rules_i);

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        menu_items.push(&sep_wininfo);
        menu_items.push(&window_info_i);
    }

    menu_items.push(&sep_before_quit);
    menu_items.push(&quit_i);

    let _ = tray_menu.append_items(&menu_items);

    let mut tray_icon = None;

    // macOS/Windows: poll the configured device's presence on a background
    // thread and refresh the status menu item (line 2) only when it changes.
    // Read-only enumeration never opens the device, so it can't disturb the
    // keyboard.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let status_proxy = proxy.clone();
        std::thread::spawn(move || {
            // Capable-board presence tracker (Finding #1): keys the handshake
            // lifecycle on CAPABLE-board presence (not Tier-1 presence),
            // re-probing only when the Tier-1 path set changes (a plug/unplug)
            // so the hot loop never pings on a stable bus (Finding #3). A
            // capable board unplugging while a non-capable Tier-1 board remains
            // is now a real Loss (reset + re-arm); a different capable board
            // replugging is a real Gain (re-handshake). See PresenceTracker.
            let mut presence = crate::core::notifier::PresenceTracker::new();
            // UI-status tracker: keyed on the three-state DeviceStatus (separate
            // from the handshake lifecycle — the NoModule→Connected flip happens
            // while capable presence is stable, driven by the handshake setting
            // HOST_CAPABLE). Seeded ⇒ no spurious first-tick event (first-paint
            // already rendered it).
            let mut last_status: Option<crate::core::notifier::DeviceStatus> =
                Some(crate::core::notifier::device_status());
            loop {
                // Handshake lifecycle on THIS poll thread (non-blocking to the
                // UI event loop). Gain ⇒ perform_handshake (idempotent via
                // HAS_HANDSHAKED if the runner already handshooked at startup);
                // Loss ⇒ reset so the next gain re-runs.
                match presence.tick(verbose) {
                    crate::core::notifier::HandshakeAction::Gain => {
                        crate::core::notifier::perform_handshake(verbose);
                    }
                    crate::core::notifier::HandshakeAction::Loss => {
                        crate::core::notifier::reset_handshake_state();
                    }
                    crate::core::notifier::HandshakeAction::None => {}
                }
                // ---- UI status: three-state, sent only on ITS transition. ----
                // Computed AFTER the handshake block so a same-tick Gain +
                // perform_handshake (which may set HOST_CAPABLE ⇒ Connected) is
                // reflected in the payload now (within the 3s cadence).
                let status = crate::core::notifier::device_status();
                if last_status != Some(status) {
                    let _ = status_proxy.send_event(UserEvent::DeviceStatus(status));
                    last_status = Some(status);
                }
                std::thread::sleep(std::time::Duration::from_secs(3));
            }
        });
    }

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(tao::event::StartCause::Init) => {
                let (icon, icon_is_template) = {
                    #[cfg(target_os = "macos")]
                    {
                        // The menu-bar icon is the dedicated monochrome template
                        // asset (IconTemplate.png), which macOS tints to the bar.
                        // Fall back to the generated default if it is absent.
                        match load_template_icon_from_bundle() {
                            Some(template_icon) => (template_icon, true),
                            None => (create_default_icon(), false),
                        }
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        // Windows: load a real tray PNG (the generated 16x16
                        // white square is nearly invisible on a light taskbar).
                        // Falls back to the generated default when no asset is
                        // present. Linux/other targets also get the default.
                        match load_windows_tray_icon() {
                            Some(icon) => (icon, false),
                            None => (create_default_icon(), false),
                        }
                    }
                };

                tray_icon = Some(
                    TrayIconBuilder::new()
                        .with_menu(Box::new(tray_menu.clone()))
                        .with_tooltip("QMKonnect")
                        .with_icon(icon)
                        .with_icon_as_template(icon_is_template)
                        .build()
                        .unwrap(),
                );

                // We have to request a redraw here to have the icon actually show up.
                // Tao only exposes a redraw method on the Window so we use core-foundation directly.
                #[cfg(target_os = "macos")]
                unsafe {
                    use objc2_core_foundation::{CFRunLoopGetMain, CFRunLoopWakeUp};

                    let rl = CFRunLoopGetMain().unwrap();
                    CFRunLoopWakeUp(&rl);
                }

                // Launch-at-login: reflect the real system status in the
                // checkbox immediately, then kick off the one-time default-on
                // (deferred so its registration work never blocks this Init).
                #[cfg(target_os = "macos")]
                {
                    launch_at_login_i.set_checked(autostart::is_enabled());
                    autostart_first_run_default_on(proxy.clone());
                }
            }

            Event::UserEvent(UserEvent::MenuEvent(event)) => {
                if event.id == quit_i.id() {
                    println!("Exited");
                    tray_icon.take();
                    *control_flow = ControlFlow::Exit;
                } else if event.id == settings_i.id() {
                    handle_settings_click();
                } else if event.id == edit_rules_i.id() {
                    // "Edit rules": seed rules.toml if absent + open it in the
                    // system editor (HOST_RULES.md §7). Fire-and-forget on a
                    // background thread (keep I/O off the event loop).
                    std::thread::spawn(crate::core::edit_rules);
                }

                #[cfg(any(target_os = "macos", target_os = "windows"))]
                {
                    if event.id == window_info_i.id() {
                        handle_window_info_click();
                    }
                }

                #[cfg(target_os = "macos")]
                {
                    if event.id == launch_at_login_i.id() {
                        handle_launch_at_login_click(&launch_at_login_i);
                    }
                }

                #[cfg(target_os = "windows")]
                {
                    if event.id == open_at_login_i.id() {
                        handle_open_at_login_click(&open_at_login_i);
                    }
                }
            }

            // Background probe thread reports the latest three-state device status.
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            Event::UserEvent(UserEvent::DeviceStatus(status)) => {
                device_status_i.set_text(device_status_text(status));
            }

            // Deferred first-run register finished: refresh the checkbox from
            // the real status (covers the first-launch unregistered->enabled
            // transition that happened after the Init sync above).
            #[cfg(target_os = "macos")]
            Event::UserEvent(UserEvent::AutostartSync) => {
                launch_at_login_i.set_checked(autostart::is_enabled());
            }

            _ => {}
        }
    });
}

#[cfg(target_os = "macos")]
fn load_icon(path: &std::path::Path) -> Result<tray_icon::Icon, Box<dyn std::error::Error>> {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open(path)?.into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    Ok(tray_icon::Icon::from_rgba(
        icon_rgba,
        icon_width,
        icon_height,
    )?)
}

fn create_default_icon() -> tray_icon::Icon {
    // Create a simple 16x16 default icon if no icon file is found
    let rgba = vec![255u8; 16 * 16 * 4]; // White 16x16 icon
    tray_icon::Icon::from_rgba(rgba, 16, 16).expect("Failed to create default icon")
}

/// Load a real tray icon for Windows. The generated 16x16 white default is
/// nearly invisible on a light taskbar, so try the dedicated dark tray PNG
/// (`IconTray-dark.png`) next to the executable first (where the installer
/// drops it), then the dev source-tree `packaging/` copy, and finally fall
/// back to `None` (caller uses the generated default). Mirrors the macOS
/// `load_template_icon_from_bundle` lookup and the Windows dialog's
/// `load_app_icon` path search. The `image` crate is cross-platform, so the
/// decode is inlined here rather than reusing the macOS-gated `load_icon`.
#[cfg(target_os = "windows")]
fn load_windows_tray_icon() -> Option<tray_icon::Icon> {
    let exe_path = std::env::current_exe().ok()?;
    let exe_dir = exe_path.parent()?;
    let candidates = [
        exe_dir.join("IconTray-dark.png"),
        std::path::Path::new("packaging/IconTray-dark.png").to_path_buf(),
    ];
    for p in candidates {
        if !p.exists() {
            continue;
        }
        let img = match image::open(&p) {
            Ok(img) => img.into_rgba8(),
            Err(_) => continue,
        };
        // The system-tray slot is a fixed size, so to make the glyph render
        // larger we zoom the opaque content in ~20% (canvas size unchanged).
        let img = zoom_in_about_20_percent(img);
        let (w, h) = img.dimensions();
        if let Ok(icon) = tray_icon::Icon::from_rgba(img.into_raw(), w, h) {
            return Some(icon);
        }
    }
    None
}

/// Non-Windows stub so the icon-selection block compiles on every target;
/// only Windows actually loads a PNG here.
#[cfg(not(target_os = "windows"))]
fn load_windows_tray_icon() -> Option<tray_icon::Icon> {
    None
}

/// Zoom an icon's visible (opaque) content in by ~20%, keeping the canvas size
/// unchanged. The system-tray slot is a fixed size, so the only way to make the
/// glyph appear larger is to enlarge the content relative to the canvas. The
/// scale is clamped to the available headroom so the glyph never spills past the
/// canvas edge (no clipping): a glyph that already fills its canvas is enlarged
/// by as much as fits rather than cropping the artwork.
#[cfg(target_os = "windows")]
fn zoom_in_about_20_percent(img: image::RgbaImage) -> image::RgbaImage {
    use image::imageops::{resize, FilterType};

    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return img;
    }

    // Opaque-content extent (alpha > 0). Its longest side over the shorter
    // canvas side is the headroom: the max scale that keeps the glyph inside.
    let raw = img.as_raw();
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut any_opaque = false;
    for y in 0..h {
        for x in 0..w {
            if raw[((y * w + x) * 4 + 3) as usize] != 0 {
                any_opaque = true;
                if x < min_x {
                    min_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if x > max_x {
                    max_x = x;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }
    }
    if !any_opaque {
        return img;
    }
    let content = (max_x - min_x + 1).max(max_y - min_y + 1) as f32;
    let canvas = w.min(h) as f32;
    let scale = 1.2_f32.min(canvas / content);
    if scale <= 1.0 {
        return img;
    }

    let nw = ((w as f32) * scale).round() as u32;
    let nh = ((h as f32) * scale).round() as u32;
    let scaled = resize(&img, nw, nh, FilterType::Lanczos3);

    // Center-crop back to the original canvas size.
    let dx = (nw - w) / 2;
    let dy = (nh - h) / 2;
    let mut out = image::RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            out.put_pixel(x, y, *scaled.get_pixel(dx + x, dy + y));
        }
    }
    out
}

/// Label for the macOS/Windows device-status menu item (line 2). Three states
/// per `spec/UI.md` §4 / `spec/DEVICE_DISCOVERY.md` §3: a solid dot (≥1 capable
/// board), a warning glyph (QMK board present but no qmk_notifier module), or a
/// hollow dot (0 Tier-1 boards). The "No module" warning is the truthful F13
/// value — see [`device_status`](crate::core::notifier::device_status) in
/// `src/core/notifier.rs`.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn device_status_text(status: crate::core::notifier::DeviceStatus) -> String {
    use crate::core::notifier::DeviceStatus; // function-local use → terse arms, no cfg-import issue
    match status {
        // U+25CF BLACK CIRCLE — solid dot; ≥1 capable board.
        DeviceStatus::Connected => "\u{25CF}  Device Connected".to_string(),
        // U+26A0 WARNING SIGN — QMK board present, no qmk_notifier module.
        DeviceStatus::NoModule => {
            "\u{26A0}  QMK board found \u{2014} no qmk_notifier module (flash it)".to_string()
        }
        // U+25CB WHITE CIRCLE — hollow dot; 0 Tier-1 boards.
        DeviceStatus::Disconnected => "\u{25CB}  No Device Connected".to_string(),
    }
}

fn handle_settings_click() {
    #[cfg(target_os = "windows")]
    {
        use crate::platforms;

        // Get or create the config directory
        match platforms::create_config_dir() {
            Ok(config_dir) => {
                let config_path = config_dir.join("config.toml");

                // Create default config if it doesn't exist
                if !config_path.exists() {
                    if let Err(e) = crate::core::create_default_config(&config_path) {
                        show_error_message(&format!("Failed to create configuration file: {}", e));
                        return;
                    }
                }

                // Show the settings dialog
                if let Err(e) = show_settings_dialog(&config_path) {
                    show_error_message(&format!("Failed to show settings dialog: {}", e));
                }
            }
            Err(e) => {
                show_error_message(&format!("Failed to access configuration directory: {}", e));
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        use crate::platforms;

        // Get or create the config directory
        match platforms::create_config_dir() {
            Ok(config_dir) => {
                let config_path = config_dir.join("config.toml");

                // Create default config if it doesn't exist
                if !config_path.exists() {
                    if let Err(e) = crate::core::create_default_config(&config_path) {
                        show_macos_error_message(&format!(
                            "Failed to create configuration file: {}",
                            e
                        ));
                        return;
                    }
                }

                // Show the settings dialog
                if let Err(e) = show_macos_settings_dialog(&config_path) {
                    show_macos_error_message(&format!("Failed to show settings dialog: {}", e));
                }
            }
            Err(e) => {
                show_macos_error_message(&format!(
                    "Failed to access configuration directory: {}",
                    e
                ));
            }
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        // No native settings GUI on Linux/X11 tray builds; point the user at the
        // config file instead of dead-ending silently (#20).
        match crate::platforms::get_config_paths()
            .into_iter()
            .find(|p| p.exists())
        {
            Some(path) => {
                println!("Edit your configuration at: {}", path.display());
                println!("Set vendor_id and product_id, then restart qmkonnect.");
            }
            None => {
                println!("No config file found. Run `qmkonnect -c` to create one.");
            }
        }
    }
}

#[cfg(target_os = "windows")]
/// Render the Windows native Settings dialog (`QMKSettingsDialog` window class,
/// `WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU`, control IDs 1001-1013 — see
/// `spec/UI.md` §2.1). The primary surface is a live LISTBOX of discovered
/// devices built from `classify_devices` (`spec/DEVICE_DISCOVERY.md` §5);
/// the legacy VID/PID hex `EDIT` controls are relocated under an "Advanced /
/// manual override" `BS_GROUPBOX` disclosure. Selecting a listbox row writes
/// that board's `vid`/`pid` to `config.toml` via `render_config_body`; a
/// [Rescan] button clears the classification cache + re-classifies. The
/// zero-config case (one capable board, no VID/PID set) hides the picker and
/// shows a static `Detected: <name>` line. The dialog proc stores its result
/// in `DIALOG_RESULT`; this function reads it back after the message loop and
/// applies `chosen` first, else `manual`, else leaves the open-time VID/PID
/// (`spec/UI.md` §2.0).
fn show_settings_dialog(config_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::ptr;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DispatchMessageW, GetMessageW, LoadCursorW, RegisterClassW, ShowWindow,
        TranslateMessage, IDC_ARROW, MSG, SW_SHOW, WNDCLASSW, WS_CAPTION, WS_OVERLAPPED,
        WS_SYSMENU, WS_VISIBLE,
    };

    // Load current configuration
    let current_config = match crate::core::parse_config(config_path) {
        Ok(config) => config,
        Err(_) => crate::core::Config::default(),
    };

    // Reset the shared result slot (replaces the former Arc::into_raw/forget leak, #9).
    DIALOG_RESULT.lock().unwrap().take();

    unsafe {
        let h_instance = GetModuleHandleW(None)?;
        let class_name = to_wide_string("QMKSettingsDialog");
        let window_title = to_wide_string("QMK Settings");

        // Load application icon
        let app_icon = load_app_icon();

        // Register window class
        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(settings_dialog_proc),
            hInstance: h_instance.into(),
            lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH((15 + 1) as isize), // COLOR_3DFACE + 1
            hIcon: app_icon,
            ..Default::default()
        };

        RegisterClassW(&wnd_class);

        // Get screen dimensions to center the dialog
        let screen_width = windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
            windows::Win32::UI::WindowsAndMessaging::SM_CXSCREEN,
        );
        let screen_height = windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
            windows::Win32::UI::WindowsAndMessaging::SM_CYSCREEN,
        );

        let dialog_width = 420;
        let dialog_height = 380;
        let x = (screen_width - dialog_width) / 2;
        let y = (screen_height - dialog_height) / 2;

        // Create the dialog window (centered on screen)
        let hwnd = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            windows::core::PCWSTR(class_name.as_ptr()),
            windows::core::PCWSTR(window_title.as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            x,
            y,
            dialog_width,
            dialog_height,
            HWND(0),
            None,
            h_instance,
            Some(ptr::null()),
        );

        if hwnd.0 == 0 {
            return Err("Failed to create settings dialog window".into());
        }

        // Capture the dialog-OPEN vid/pid so the [Rescan] arm can re-evaluate
        // the three picker cases without re-reading the config file — the user
        // is mid-edit, so the live edit fields are NOT authoritative.
        *DIALOG_OPEN_VIDPID.lock().unwrap() = (current_config.vendor_id, current_config.product_id);

        // Create controls
        create_dialog_controls(hwnd, h_instance.into(), &current_config)?;

        // Set the window icon directly using a standard Windows icon
        // This will show the blue information icon, which is better than no icon
        let icon = windows::Win32::UI::WindowsAndMessaging::LoadIconW(
            None,
            windows::Win32::UI::WindowsAndMessaging::IDI_INFORMATION,
        )
        .unwrap_or(windows::Win32::UI::WindowsAndMessaging::HICON(0));

        if icon.0 != 0 {
            windows::Win32::UI::WindowsAndMessaging::SendMessageW(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::WM_SETICON,
                windows::Win32::Foundation::WPARAM(
                    windows::Win32::UI::WindowsAndMessaging::ICON_SMALL as usize,
                ),
                windows::Win32::Foundation::LPARAM(icon.0 as isize),
            );
            windows::Win32::UI::WindowsAndMessaging::SendMessageW(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::WM_SETICON,
                windows::Win32::Foundation::WPARAM(
                    windows::Win32::UI::WindowsAndMessaging::ICON_BIG as usize,
                ),
                windows::Win32::Foundation::LPARAM(icon.0 as isize),
            );
        }

        ShowWindow(hwnd, SW_SHOW);

        // Message loop
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Get the result
        let result = DIALOG_RESULT.lock().unwrap().take();

        if let Some(dr) = result {
            // Save to file, PRESERVING every non-VID/PID field
            // (usage_page/usage/debounce_ms/poll_interval_ms): overlay the
            // dialog's VID/PID onto the config parsed at dialog-open time and
            // serialize the full struct. Previously this rendered a VID/PID-only
            // body and silently reset the user's other fields on every save.
            //
            // Apply `chosen` first, else `manual`, else leave the open-time
            // VID/PID as-is (spec/UI.md §2.0). `chosen` is the listbox pick —
            // a concrete (u16,u16) from ClassifiedDevice; `manual` is the
            // typed hex pair (each None ⇒ auto-discovery). When neither is set
            // the user clicked OK without changing anything, so we keep the
            // open-time config.
            //
            // Bug 4 (PRD ID 3): snapshot the VID/PID BEFORE the move below.
            // `current_config` is moved into `merged` on the next line (Config is
            // Clone, not Copy), so capture these Copy Option<u16> fields now to
            // diff against the post-save values and decide whether the handshake
            // must be reset for the newly-selected board.
            let old_vid = current_config.vendor_id;
            let old_pid = current_config.product_id;
            let mut merged = current_config;
            if let Some((v, p)) = dr.chosen {
                merged.vendor_id = Some(v);
                merged.product_id = Some(p);
            } else if let Some((v, p)) = dr.manual {
                merged.vendor_id = v;
                merged.product_id = p;
            }
            let config_content = crate::core::render_config_body(&merged);

            crate::core::atomic_write(config_path, &config_content)?;

            // Bug 4 (PRD ID 3): if the VID/PID changed, reset + re-handshake for
            // the newly-selected board so CALLBACK_NAMES (name→id map) is rebuilt
            // for it instead of continuing to use the old board's map until a
            // replug. reset_handshake_state() clears HOST_CAPABLE / BOARD_HAS_RULES
            // / CALLBACK_NAMES / HAS_HANDSHAKED; perform_handshake then re-runs
            // (its HAS_HANDSHAKED guard was just cleared) and reads config.toml
            // fresh (configured_filter), so the just-written VID/PID selects the
            // new board. `false` = non-verbose (verbose is not in scope here; see
            // bug_findings.md §132 — do NOT thread it through handle_settings_click,
            // which is shared with the macOS path).
            if merged.vendor_id != old_vid || merged.product_id != old_pid {
                crate::core::notifier::reset_handshake_state();
                crate::core::notifier::perform_handshake(false);
            }

            // Configuration saved successfully - no success dialog needed
            // The QMK connection is established fresh for each notification,
            // so no restart is required for the changes to take effect
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
/// Build one LISTBOX row for a discovered device (`spec/DEVICE_DISCOVERY.md
/// §5.1 / §3): a ✓/✗ capability glyph, the HID `product_name` (or
/// `(unnamed)`), the `0xVID:0xPID` pair, and a short status label. The name is
/// space-padded (`format!("{:<22}", ...)`) instead of `LBS_USETABSTOPS` +
/// `LB_SETTABSTOPS` — tab stops are in dialog-template units and would need
/// conversion for this pixel-based dialog, whereas a padded string is robust
/// for the 2-4 rows the picker shows (research §5).
fn picker_row_text(d: &crate::core::notifier::ClassifiedDevice) -> String {
    use crate::core::notifier::DeviceKind;
    let (glyph, status) = match d.kind {
        DeviceKind::Capable { .. } => ("\u{2713}", "qmk_notifier"), // ✓
        DeviceKind::NotQmkNotifier => ("\u{2717}", "QMK board, no module"), // ✗
    };
    let name = d.product_name.as_deref().unwrap_or("(unnamed)");
    format!(
        "{}  {:<22} 0x{:04X}:0x{:04X}  {}",
        glyph, name, d.vendor_id, d.product_id, status
    )
}

#[cfg(target_os = "windows")]
/// Populate the discovered-device LISTBOX + header for the three cases in
/// `spec/DEVICE_DISCOVERY.md` §5.1, and toggle the LISTBOX + [Rescan]
/// visibility accordingly. Reused by the initial dialog open (from
/// `create_dialog_controls`) and by the [Rescan] arm of `settings_dialog_proc`,
/// so both paths produce identical rendering. `open_vid`/`open_pid` are the
/// dialog-OPEN config's vid/pid (from `DIALOG_OPEN_VIDPID`), NOT the live edit
/// fields — the user is mid-edit, so the clean-auto case is judged against the
/// open-time config.
///
/// The three cases:
/// 1. **empty** — no Tier-1 devices: hide the picker, header "No QMK keyboards
///    detected...".
/// 2. **clean-auto** — exactly one `Capable` board AND `open_vid`/`open_pid`
///    are both `None` (the zero-config promise): hide the picker, header
///    `Detected: <name>`.
/// 3. **picker** — otherwise (≥2 boards, or 1 non-capable board, or VID/PID
///    already set): show the LISTBOX with one row per device.
fn populate_device_picker(
    hwnd: windows::Win32::Foundation::HWND,
    devices: &[crate::core::notifier::ClassifiedDevice],
    open_vid: Option<u16>,
    open_pid: Option<u16>,
) {
    use crate::core::notifier::DeviceKind;
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetDlgItem, SendMessageW, SetDlgItemTextW, ShowWindow, LB_ADDSTRING, LB_RESETCONTENT,
        SW_HIDE, SW_SHOW,
    };

    // Mirror the device list into the static so the OK arm can map a selected
    // listbox index → (vendor_id, product_id) (PICKER_DEVICES mirrors
    // WINDOW_INFO_ROWS — the proc has no per-call user data).
    *PICKER_DEVICES.lock().unwrap() = devices.to_vec();

    unsafe {
        let lb = GetDlgItem(hwnd, IDC_DEVICE_LIST);
        // LB_RESETCONTENT clears any prior rows (initial open is empty anyway,
        // but [Rescan] reuses this path).
        let _ = SendMessageW(lb, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
        for d in devices {
            // Runtime-built text ⇒ to_wide_string (w! only works on literals).
            let text = to_wide_string(&picker_row_text(d));
            let _ = SendMessageW(lb, LB_ADDSTRING, WPARAM(0), LPARAM(text.as_ptr() as isize));
        }

        // Resolve the three picker cases (spec/DEVICE_DISCOVERY.md §5.1).
        let clean_auto = devices.len() == 1
            && matches!(devices[0].kind, DeviceKind::Capable { .. })
            && open_vid.is_none()
            && open_pid.is_none();
        let (header, show_picker) = if devices.is_empty() {
            (
                "No QMK keyboards detected. Enter IDs manually below.".to_string(),
                false,
            )
        } else if clean_auto {
            // The zero-config case: hide the picker, show "Detected: <name>".
            let name = devices[0].product_name.as_deref().unwrap_or("(unnamed)");
            (format!("Detected: {}", name), false)
        } else {
            ("Detected keyboard(s) — choose one:".to_string(), true)
        };

        // Always set the header (it always exists).
        let header_w = to_wide_string(&header);
        let _ = SetDlgItemTextW(hwnd, IDC_HEADER, windows::core::PCWSTR(header_w.as_ptr()));

        // Toggle the LISTBOX + [Rescan] together (both are irrelevant when the
        // picker is hidden).
        let cmd = if show_picker { SW_SHOW } else { SW_HIDE };
        let _ = ShowWindow(lb, cmd);
        let rescan = GetDlgItem(hwnd, IDC_RESCAN);
        let _ = ShowWindow(rescan, cmd);
    }
}

#[cfg(target_os = "windows")]
/// Create every control in the Windows Settings dialog (`spec/UI.md` §2.1 +
/// `spec/DEVICE_DISCOVERY.md` §5.3): a header static, the discovered-device
/// LISTBOX (the picker), a [Rescan] button, and an "Advanced / manual
/// override" `BS_GROUPBOX` that visually contains the relocated VID/PID labels
/// + `EDIT`s, then the OK + Cancel buttons. The group box is created BEFORE
/// its children so the children are higher in z-order and paint on top; it is
/// purely visual and never appears in `WM_COMMAND`. After creation the VID/PID
/// fields are prefilled and `populate_device_picker` classifies + fills the
/// LISTBOX per the three §5.1 cases.
fn create_dialog_controls(
    hwnd: windows::Win32::Foundation::HWND,
    h_instance: windows::Win32::Foundation::HINSTANCE,
    config: &crate::core::Config,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::ptr;
    use windows::Win32::UI::Controls::{WC_BUTTONW, WC_EDITW, WC_LISTBOX, WC_STATICW};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, SetDlgItemTextW, BS_GROUPBOX, HMENU, LBS_HASSTRINGS, LBS_NOINTEGRALHEIGHT,
        LBS_NOTIFY, WINDOW_STYLE, WS_CHILD, WS_EX_CLIENTEDGE, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
    };

    unsafe {
        // 1. Header static (IDC_HEADER=1013): the "Detected:" / "No QMK..."
        //    line; text is set by populate_device_picker.
        CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            WC_STATICW,
            windows::core::PCWSTR::null(),
            WS_CHILD | WS_VISIBLE,
            16,
            14,
            388,
            18,
            hwnd,
            None,
            h_instance,
            Some(ptr::null()),
        );

        // 2. LISTBOX (IDC_DEVICE_LIST=1010): the discovered-device picker.
        //    LBS_NOTIFY/LBS_HASSTRINGS/LBS_NOINTEGRALHEIGHT are raw i32 and do
        //    NOT impl BitOr<WINDOW_STYLE>, so the codebase cast pattern
        //    (tray.rs set_font) is: cast each as u32, OR, wrap in WINDOW_STYLE.
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            WC_LISTBOX,
            windows::core::PCWSTR::null(),
            WINDOW_STYLE(
                WS_CHILD.0
                    | WS_VISIBLE.0
                    | WS_TABSTOP.0
                    | WS_VSCROLL.0
                    | LBS_NOTIFY as u32
                    | LBS_HASSTRINGS as u32
                    | LBS_NOINTEGRALHEIGHT as u32,
            ),
            16,
            36,
            388,
            110,
            hwnd,
            HMENU(IDC_DEVICE_LIST as isize),
            h_instance,
            Some(ptr::null()),
        );

        // 3. [Rescan] button (IDC_RESCAN=1011): clears the classification
        //    cache + re-classifies + repopulates the LISTBOX.
        CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            WC_BUTTONW,
            windows::core::w!("Rescan"),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            314,
            152,
            90,
            26,
            hwnd,
            HMENU(IDC_RESCAN as isize),
            h_instance,
            Some(ptr::null()),
        );

        // 4. Advanced group box (IDC_ADVANCED_GROUP=1012): purely visual frame
        //    (BS_GROUPBOX never sends BN_CLICKED and is NEVER branched on in
        //    WM_COMMAND). Created BEFORE its child labels/edits so those
        //    children are higher in z-order and paint on top of the frame.
        CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            WC_BUTTONW,
            windows::core::w!("Advanced / manual override"),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | BS_GROUPBOX as u32),
            14,
            188,
            392,
            120,
            hwnd,
            HMENU(IDC_ADVANCED_GROUP as isize),
            h_instance,
            Some(ptr::null()),
        );

        // 5. VID label (no id) — inside the group box.
        CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            WC_STATICW,
            windows::core::w!("Vendor ID (hex):"),
            WS_CHILD | WS_VISIBLE,
            30,
            218,
            130,
            20,
            hwnd,
            None,
            h_instance,
            Some(ptr::null()),
        );

        // 6. VID text box (1001) — SAME ID, relocated under the group box.
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            WC_EDITW,
            windows::core::PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            170,
            216,
            110,
            24,
            hwnd,
            HMENU(1001),
            h_instance,
            Some(ptr::null()),
        );

        // 7. PID label (no id) — inside the group box.
        CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            WC_STATICW,
            windows::core::w!("Product ID (hex):"),
            WS_CHILD | WS_VISIBLE,
            30,
            250,
            130,
            20,
            hwnd,
            None,
            h_instance,
            Some(ptr::null()),
        );

        // 8. PID text box (1002) — SAME ID, relocated under the group box.
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            WC_EDITW,
            windows::core::PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            170,
            248,
            110,
            24,
            hwnd,
            HMENU(1002),
            h_instance,
            Some(ptr::null()),
        );

        // 9. OK button (1003) — SAME ID, relocated.
        CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            WC_BUTTONW,
            windows::core::w!("OK"),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            230,
            324,
            80,
            30,
            hwnd,
            HMENU(1003),
            h_instance,
            Some(ptr::null()),
        );

        // 10. Cancel button (1004) — SAME ID, relocated.
        CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            WC_BUTTONW,
            windows::core::w!("Cancel"),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            318,
            324,
            80,
            30,
            hwnd,
            HMENU(1004),
            h_instance,
            Some(ptr::null()),
        );

        // Set initial values. Empty when unset (None = auto-discovery) so the
        // user can leave a field blank to keep it on auto.
        let vendor_text = to_wide_string(
            &config
                .vendor_id
                .map(|v| format!("{:04x}", v))
                .unwrap_or_default(),
        );
        let product_text = to_wide_string(
            &config
                .product_id
                .map(|p| format!("{:04x}", p))
                .unwrap_or_default(),
        );

        let _ = SetDlgItemTextW(hwnd, 1001, windows::core::PCWSTR(vendor_text.as_ptr()));
        let _ = SetDlgItemTextW(hwnd, 1002, windows::core::PCWSTR(product_text.as_ptr()));

        // Classify discovered devices + populate the LISTBOX / header per the
        // three §5.1 cases. (classify_devices runs HID I/O; the cache is warm
        // from the handshake so usually <50ms — see the [Rescan] note.)
        let devices = crate::core::notifier::classify_devices(true);
        populate_device_picker(hwnd, &devices, config.vendor_id, config.product_id);
    }

    Ok(())
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn settings_dialog_proc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{
        DefWindowProcW, DestroyWindow, GetDlgItem, GetDlgItemTextW, MessageBoxW, PostQuitMessage,
        SendMessageW, LB_ERR, LB_GETCURSEL, MB_ICONERROR, MB_OK, WM_CLOSE, WM_COMMAND, WM_DESTROY,
    };

    match msg {
        WM_COMMAND => {
            let control_id = (wparam.0 & 0xFFFF) as u32;
            match control_id {
                1003 => {
                    // OK button — read the listbox selection first (chosen),
                    // then the Advanced hex fields (manual). chosen is a
                    // concrete (vid,pid) from ClassifiedDevice; manual is the
                    // typed pair (each None ⇒ auto). The save path applies
                    // chosen first, else manual (spec/UI.md §2.0).
                    let chosen = {
                        let lb = GetDlgItem(hwnd, IDC_DEVICE_LIST);
                        // LB_GETCURSEL returns the 0-based index OR LB_ERR (-1)
                        // when nothing is selected. Cast .0 as i32 and check
                        // `!= LB_ERR && >= 0` — index 0 is a VALID selection.
                        let sel = SendMessageW(
                            lb,
                            LB_GETCURSEL,
                            windows::Win32::Foundation::WPARAM(0),
                            windows::Win32::Foundation::LPARAM(0),
                        )
                        .0 as i32;
                        if sel != LB_ERR && sel >= 0 {
                            PICKER_DEVICES
                                .lock()
                                .unwrap()
                                .get(sel as usize)
                                .map(|d| (d.vendor_id, d.product_id))
                        } else {
                            None
                        }
                    };

                    let mut vendor_buffer = [0u16; 256];
                    let mut product_buffer = [0u16; 256];

                    GetDlgItemTextW(hwnd, 1001, &mut vendor_buffer);
                    GetDlgItemTextW(hwnd, 1002, &mut product_buffer);

                    let vendor_str = String::from_utf16_lossy(&vendor_buffer)
                        .trim_end_matches('\0')
                        .to_string();
                    let product_str = String::from_utf16_lossy(&product_buffer)
                        .trim_end_matches('\0')
                        .to_string();

                    match (parse_id_field(&vendor_str), parse_id_field(&product_str)) {
                        (Ok(vendor_id), Ok(product_id)) => {
                            // Store result via the shared static slot (#9).
                            *DIALOG_RESULT.lock().unwrap() = Some(DialogResult {
                                chosen,
                                manual: Some((vendor_id, product_id)),
                            });
                            let _ = DestroyWindow(hwnd);
                        }
                        (Err(e), _) | (_, Err(e)) => {
                            let error_msg = to_wide_string(&format!("Invalid input: {}", e));
                            let _ = MessageBoxW(
                                hwnd,
                                windows::core::PCWSTR(error_msg.as_ptr()),
                                windows::core::w!("Error"),
                                MB_OK | MB_ICONERROR,
                            );
                        }
                    }
                }
                id if id == IDC_RESCAN as u32 => {
                    // [Rescan]: clear the classification cache + re-classify +
                    // repopulate the LISTBOX. NOTE (G7): runs HID I/O inline on
                    // the tray thread; the cache is warm from the handshake so
                    // usually <50ms, but a cold classify is ~N×(read timeout).
                    // Acceptable for v1 (the spec does not require a worker); do
                    // NOT spawn a thread — the listbox must repopulate
                    // synchronously before this proc returns.
                    crate::core::notifier::classification_cache_clear();
                    let devices = crate::core::notifier::classify_devices(true);
                    // Use the dialog-OPEN vid/pid (NOT the live edit fields):
                    // the user is mid-edit, so the clean-auto case must be
                    // judged against the open-time config.
                    let (vid, pid) = *DIALOG_OPEN_VIDPID.lock().unwrap();
                    populate_device_picker(hwnd, &devices, vid, pid);
                }
                1004 => {
                    // Cancel button
                    let _ = DestroyWindow(hwnd);
                }
                _ => {}
            }
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
        }
        WM_DESTROY => {
            PostQuitMessage(0);
        }
        _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
    }
    windows::Win32::Foundation::LRESULT(0)
}

#[cfg(target_os = "windows")]
fn to_wide_string(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(target_os = "windows")]
fn load_app_icon() -> windows::Win32::UI::WindowsAndMessaging::HICON {
    use std::path::Path;
    use windows::Win32::UI::WindowsAndMessaging::{
        LoadIconW, LoadImageW, IDI_INFORMATION, IMAGE_ICON, LR_DEFAULTSIZE, LR_LOADFROMFILE,
    };

    unsafe {
        // Try to find the ICO file (Windows native format)
        let exe_path = std::env::current_exe().unwrap_or_default();
        let exe_dir = exe_path.parent().unwrap_or_else(|| Path::new("."));

        // Try these paths in order
        let icon_paths = [
            exe_dir.join("Icon.ico"),
            Path::new("packaging/Icon.ico").to_path_buf(),
        ];

        for icon_path in &icon_paths {
            if icon_path.exists() {
                let icon_path_wide = to_wide_string(&icon_path.to_string_lossy());
                let hicon = LoadImageW(
                    None,
                    windows::core::PCWSTR(icon_path_wide.as_ptr()),
                    IMAGE_ICON,
                    0,
                    0, // Use default size
                    LR_DEFAULTSIZE | LR_LOADFROMFILE,
                );

                if let Ok(icon) = hicon {
                    if icon.0 != 0 {
                        return windows::Win32::UI::WindowsAndMessaging::HICON(icon.0);
                    }
                }
            }
        }

        // Fallback to standard Windows information icon
        LoadIconW(None, IDI_INFORMATION)
            .unwrap_or(windows::Win32::UI::WindowsAndMessaging::HICON(0))
    }
}

#[cfg(target_os = "windows")]
fn show_error_message(message: &str) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    unsafe {
        let error_msg = to_wide_string(message);
        let title = to_wide_string("QMKonnect - Error");
        MessageBoxW(
            HWND(0),
            windows::core::PCWSTR(error_msg.as_ptr()),
            windows::core::PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }

    // Also log to console/event log
    eprintln!("Settings error: {}", message);
}

#[cfg(target_os = "macos")]
/// One macOS picker row's label: the ✓/✗ capability glyph, the live HID
/// `product_name` (or `(unnamed)`), the `0xVID:0xPID` pair, and the status
/// suffix (`qmk_notifier` / `QMK board, no module`). Built from a
/// [`crate::core::notifier::ClassifiedDevice`] per `spec/DEVICE_DISCOVERY.md`
/// §5.1 / §3. Pure string builder; unit-tested by `test_picker_row_text_glyphs`.
///
/// `#[cfg(target_os = "macos")]` (NOT `any(macos, windows)`): the parallel
/// sibling P3.M2.T1.S1 adds a Windows-gated `fn picker_row_text`. The two are
/// mutually exclusive by `target_os`, so exactly one compiles per platform —
/// no symbol collision and no merge conflict on the function name.
fn picker_row_text(d: &crate::core::notifier::ClassifiedDevice) -> String {
    use crate::core::notifier::DeviceKind;
    let (glyph, status) = match d.kind {
        DeviceKind::Capable { .. } => ("\u{2713}", "qmk_notifier"), // ✓
        DeviceKind::NotQmkNotifier => ("\u{2717}", "QMK board, no module"), // ✗
    };
    let name = d.product_name.as_deref().unwrap_or("(unnamed)");
    format!(
        "{}  {:<22} 0x{:04X}:0x{:04X}  {}",
        glyph, name, d.vendor_id, d.product_id, status
    )
}

// The Advanced checkbox's `toggleAdvanced:` action is an `extern "C" fn` (an
// Obj-C method registered on `RustMacSettingsTarget`) and CANNOT capture Rust
// locals, so it reads the two NSTextField pointers from this static. Mirrors
// `WINDOW_INFO_ROWS` (@the established pattern for a free/extern fn that needs
// shared state). Only one Settings dialog is open at a time, so a single
// 2-slot array is sufficient.
// Raw `*mut Object` is `!Send`/`!Sync`, which would make the `Mutex` (and thus
// the `static`) non-`Sync`. Obj-C objects are reference-counted and safe for the
// message sends performed here (always under the lock), so the SendPtr newtype
// vouches for `Send` — the only bound `Mutex<T>: Sync` requires.
#[cfg(target_os = "macos")]
struct SendPtr(*mut objc::runtime::Object);
#[cfg(target_os = "macos")]
unsafe impl Send for SendPtr {}
#[cfg(target_os = "macos")]
static ADVANCED_FIELDS: std::sync::Mutex<[Option<SendPtr>; 2]> =
    std::sync::Mutex::new([None, None]);

#[cfg(target_os = "macos")]
/// The Advanced checkbox's action: reads the checkbox `state` (it flips its own
/// state on click before the action fires) and shows the two hex fields when
/// checked (`state == 1` / `NSOnState`) or hides them when unchecked. The field
/// pointers come from [`ADVANCED_FIELDS`] (an extern fn cannot capture locals).
/// Template: [`wi_copy_row`] (reads a STATIC + the sender).
extern "C" fn mac_toggle_advanced(
    _this: &objc::runtime::Object,
    _sel: objc::runtime::Sel,
    sender: *mut objc::runtime::Object,
) {
    use objc::runtime::{NO, YES};
    use objc::{msg_send, sel, sel_impl};
    // NSOnState = 1 ⇒ checked ⇒ show (setHidden:NO); else hide.
    let state: isize = unsafe { msg_send![sender, state] };
    let hide = if state == 1 { NO } else { YES };
    if let Ok(fields) = ADVANCED_FIELDS.lock() {
        for field_opt in fields.iter() {
            if let Some(field) = field_opt {
                if !field.0.is_null() {
                    let _: () = unsafe { msg_send![field.0, setHidden: hide] };
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn show_macos_settings_dialog(
    config_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use objc::runtime::{Class, Object};
    use objc::{msg_send, sel, sel_impl};

    unsafe {
        // Background apps (LSUIElement) lack a main autorelease pool, so create
        // one for the lifetime of the dialog.
        let pool_class =
            Class::get("NSAutoreleasePool").ok_or("Failed to get NSAutoreleasePool class")?;
        let pool: *mut Object = msg_send![pool_class, new];
        if pool.is_null() {
            return Err("Failed to create autorelease pool".into());
        }

        let result = show_settings_dialog_with_pool(config_path);

        let _: () = msg_send![pool, drain];
        result
    }
}

#[cfg(target_os = "macos")]
/// Build and run the macOS Settings `NSAlert` (`spec/UI.md` §2.2). The legacy
/// two-`NSTextField` hex surface is now relocated under an "Advanced / manual
/// override" disclosure checkbox, and a live, self-populating **picker** of
/// discovered devices (`classify_devices`, `spec/DEVICE_DISCOVERY.md` §5.1)
/// is the new primary surface in the accessory view: one `NSButton` radio row
/// per `ClassifiedDevice`, titled with `picker_row_text` (the ✓/✗ glyph +
/// `product_name` + `0xVID:0xPID` + status). Selecting a radio row is the
/// disambiguation: it writes that board's VID/PID into `config.toml` via the
/// shared `render_config_body` renderer.
///
/// The zero-config case (one capable board, no VID/PID set) is preserved: no
/// picker is shown and the header reads `Detected: <name>`.
///
/// # Platform-specific deviations (Mode A)
/// - **No `[ Rescan ]` button.** Unlike Windows' modal `GetMessageW` loop,
///   `runModal` BLOCKS the tray thread for the whole dialog lifetime, so there
///   is no "dialog-open" window during which a board could be flashed and
///   re-scanned. `classify_devices(true)` is called ONCE before building the
///   accessory view.
/// - **Manual vertical layout, not `NSStackView`.** The spec (§5.3) names
///   `NSStackView`, but with the legacy `objc = 0.2.7` crate that needs Auto
///   Layout constraints (fragile via raw `msg_send!`). The existing dialog
///   already uses manual `setFrame` in a plain `NSView`; this extends that
///   exact pattern. The §5.3 row SEMANTICS are fully honored.
/// - **`NSRadioButton` (=4) is deprecated but functional.** Apple marks it in
///   favor of `NSSwitchButton` + a coordinator; it remains fully supported on
///   all current macOS. A safety net reads the FIRST `NSOnState` row on OK.
///
/// The dialog is synchronous (`runModal` blocks), so `chosen` (a radio pick →
/// concrete `(u16,u16)`) and `manual` (the two typed fields → `Option<u16>`
/// each) are plain locals — NO `DIALOG_RESULT` static is needed on macOS
/// (unlike the Windows free `WndProc`). The Advanced toggle's `toggleAdvanced:`
/// action is an `extern "C" fn` (registered on `RustMacSettingsTarget`) that
/// cannot capture locals, so it reads the two field pointers from the
/// [`ADVANCED_FIELDS`] static.
fn show_settings_dialog_with_pool(
    config_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use objc::runtime::{Class, Object, NO, YES};
    use objc::{class, declare::ClassDecl, msg_send, sel, sel_impl};

    let current_config = match crate::core::parse_config(config_path) {
        Ok(config) => config,
        Err(_) => crate::core::Config::default(),
    };

    // Classify the Tier-1 devices ONCE (runModal blocks, so there is no
    // dialog-open window to re-scan within — no [Rescan] button on macOS).
    let devices = crate::core::notifier::classify_devices(true);
    let clean_auto = devices.len() == 1
        && matches!(
            devices[0].kind,
            crate::core::notifier::DeviceKind::Capable { .. }
        )
        && current_config.vendor_id.is_none()
        && current_config.product_id.is_none();
    let has_capable = devices
        .iter()
        .any(|d| matches!(d.kind, crate::core::notifier::DeviceKind::Capable { .. }));
    let show_picker = !devices.is_empty() && !clean_auto;

    unsafe {
        let alert_class = Class::get("NSAlert").ok_or("Failed to get NSAlert class")?;
        let alert: *mut Object = msg_send![alert_class, new];
        if alert.is_null() {
            return Err("Failed to create NSAlert instance".into());
        }

        let title = create_nsstring("QMK Settings")?;
        // The message text shows the current format_id_hex (UI.md §2.2 contract).
        let message_text = if devices.is_empty() {
            format!(
                "No QMK keyboards detected. Use Advanced to enter IDs manually.\n\nCurrent — Vendor ID: {} / Product ID: {}",
                format_id_hex(current_config.vendor_id),
                format_id_hex(current_config.product_id)
            )
        } else if clean_auto {
            format!(
                "Detected: {}. Auto-discovery is active.\n\nVendor ID: {} / Product ID: {}",
                devices[0].product_name.as_deref().unwrap_or("(unnamed)"),
                format_id_hex(current_config.vendor_id),
                format_id_hex(current_config.product_id)
            )
        } else {
            format!(
                "Select a detected keyboard below (or use Advanced for manual entry).\n\nVendor ID: {} / Product ID: {}",
                format_id_hex(current_config.vendor_id),
                format_id_hex(current_config.product_id)
            )
        };
        let message = create_nsstring(&message_text)?;
        let _: () = msg_send![alert, setMessageText: title];
        let _: () = msg_send![alert, setInformativeText: message];

        let ok_button_title = create_nsstring("OK")?;
        let cancel_button_title = create_nsstring("Cancel")?;
        let _: *mut Object = msg_send![alert, addButtonWithTitle: ok_button_title];
        let _: *mut Object = msg_send![alert, addButtonWithTitle: cancel_button_title];

        // --- Register the Advanced toggle's target class (once). --------------
        // Mirrors RustWindowInfoCopyTarget (wi_copy_row template). The
        // toggleAdvanced: action reads the checkbox state + ADVANCED_FIELDS.
        if Class::get("RustMacSettingsTarget").is_none() {
            let superclass = Class::get("NSObject").ok_or("NSObject class not found")?;
            let mut decl = ClassDecl::new("RustMacSettingsTarget", superclass)
                .ok_or("failed to declare RustMacSettingsTarget")?;
            decl.add_method(
                sel!(toggleAdvanced:),
                mac_toggle_advanced
                    as extern "C" fn(
                        &objc::runtime::Object,
                        objc::runtime::Sel,
                        *mut objc::runtime::Object,
                    ),
            );
            decl.register();
        }
        let target: *mut Object = msg_send![
            Class::get("RustMacSettingsTarget").ok_or("RustMacSettingsTarget missing")?,
            new
        ];

        // --- The two relocated hex fields (now under the Advanced disclosure). --
        // Bottom of the container (origin is bottom-left). Widened for visibility.
        let textfield_class = Class::get("NSTextField").ok_or("Failed to get NSTextField class")?;

        let vendor_field: *mut Object = msg_send![textfield_class, new];
        if vendor_field.is_null() {
            return Err("Failed to create vendor ID text field".into());
        }
        let vendor_value = create_nsstring(&format_id_hex(current_config.vendor_id))?;
        let _: () = msg_send![vendor_field, setStringValue: vendor_value];
        let _: () = msg_send![vendor_field, setFrame: objc_types::NSRect {
            origin: objc_types::NSPoint { x: 0.0, y: 0.0 },
            size: objc_types::NSSize { width: 300.0, height: 22.0 }
        }];

        let product_field: *mut Object = msg_send![textfield_class, new];
        if product_field.is_null() {
            return Err("Failed to create product ID text field".into());
        }
        let product_value = create_nsstring(&format_id_hex(current_config.product_id))?;
        let _: () = msg_send![product_field, setStringValue: product_value];
        let _: () = msg_send![product_field, setFrame: objc_types::NSRect {
            origin: objc_types::NSPoint { x: 0.0, y: 30.0 },
            size: objc_types::NSSize { width: 300.0, height: 22.0 }
        }];

        // Carry the two field pointers to the extern toggle fn (G8).
        *ADVANCED_FIELDS.lock().unwrap() =
            [Some(SendPtr(vendor_field)), Some(SendPtr(product_field))];

        // --- The Advanced checkbox (NSSwitchButton=3, a checkbox). -------------
        // Default: unchecked + fields HIDDEN when capable boards exist (the
        // picker is the primary surface); checked + fields SHOWN when there are
        // no capable boards (so the user can type). G5/G6.
        let adv_btn: *mut Object = msg_send![class!(NSButton), new];
        if adv_btn.is_null() {
            return Err("Failed to create Advanced checkbox".into());
        }
        let adv_title = create_nsstring("Advanced / manual override")?;
        let _: () = msg_send![adv_btn, setTitle: adv_title];
        let _: () = msg_send![adv_btn, setButtonType: 3u64]; // NSSwitchButton (checkbox)
        let _: () = msg_send![adv_btn, setFrame: objc_types::NSRect {
            origin: objc_types::NSPoint { x: 0.0, y: 60.0 },
            size: objc_types::NSSize { width: 300.0, height: 22.0 }
        }];
        let _: () = msg_send![adv_btn, setTarget: target];
        let _: () = msg_send![adv_btn, setAction: sel!(toggleAdvanced:)];
        let init_state: isize = if has_capable { 0 } else { 1 };
        let _: () = msg_send![adv_btn, setState: init_state];
        let hide_fields = if has_capable { YES } else { NO };
        let _: () = msg_send![vendor_field, setHidden: hide_fields];
        let _: () = msg_send![product_field, setHidden: hide_fields];

        // --- The device radio rows (ONLY in the picker case). ------------------
        // NSRadioButton=4: siblings in the same superview auto-enforce mutual
        // exclusivity (no NSMatrix needed). G4 (deprecated but functional).
        let mut row_btns: Vec<*mut Object> = Vec::new();
        let row_h: f64 = 22.0;
        let rows_base_y: f64 = 88.0; // above the Advanced checkbox (60 + 22)
        if show_picker {
            for (i, d) in devices.iter().enumerate() {
                let row: *mut Object = msg_send![class!(NSButton), new];
                if row.is_null() {
                    return Err("Failed to create picker radio row".into());
                }
                let lbl = create_nsstring(&picker_row_text(d))?;
                let _: () = msg_send![row, setTitle: lbl];
                let _: () = msg_send![row, setButtonType: 4u64]; // NSRadioButton
                let _: () = msg_send![row, setTag: i as isize];
                let _: () = msg_send![row, setFrame: objc_types::NSRect {
                    origin: objc_types::NSPoint {
                        x: 0.0,
                        y: rows_base_y + (i as f64) * row_h,
                    },
                    size: objc_types::NSSize { width: 360.0, height: row_h }
                }];
                row_btns.push(row);
            }
        }

        // --- The header label (read-only NSTextField) at the TOP. -------------
        // Read-only-label idiom from the window-info dialog: setBezeled:NO /
        // setDrawsBackground:NO / setEditable:NO / setSelectable:YES.
        let header_text = if devices.is_empty() {
            "No QMK keyboards detected.".to_string()
        } else if clean_auto {
            format!(
                "Detected: {}",
                devices[0].product_name.as_deref().unwrap_or("(unnamed)")
            )
        } else {
            "Detected keyboard(s) — choose one:".to_string()
        };
        let header: *mut Object = msg_send![class!(NSTextField), new];
        if header.is_null() {
            return Err("Failed to create header label".into());
        }
        let h_ns = create_nsstring(&header_text)?;
        let _: () = msg_send![header, setStringValue: h_ns];
        let _: () = msg_send![header, setBezeled: NO];
        let _: () = msg_send![header, setDrawsBackground: NO];
        let _: () = msg_send![header, setEditable: NO];
        let _: () = msg_send![header, setSelectable: YES];
        let rows_count = if show_picker { devices.len() } else { 0 };
        let header_y = rows_base_y + (rows_count as f64) * row_h + 4.0;
        let _: () = msg_send![header, setFrame: objc_types::NSRect {
            origin: objc_types::NSPoint { x: 0.0, y: header_y },
            size: objc_types::NSSize { width: 360.0, height: 18.0 }
        }];

        // --- The container NSView (dynamic height; origin is bottom-left). -----
        // The accessory view's frame determines the alert's content height, so
        // it must be computed AFTER every subview's frame is known (G11).
        let container_height = header_y + 18.0 + 8.0; // header + padding
        let view_class = Class::get("NSView").ok_or("Failed to get NSView class")?;
        let container_view: *mut Object = msg_send![view_class, new];
        if container_view.is_null() {
            return Err("Failed to create container view".into());
        }
        let _: () = msg_send![container_view, setFrame: objc_types::NSRect {
            origin: objc_types::NSPoint { x: 0.0, y: 0.0 },
            size: objc_types::NSSize { width: 360.0, height: container_height }
        }];
        let _: () = msg_send![container_view, addSubview: header];
        for row in &row_btns {
            let _: () = msg_send![container_view, addSubview: *row];
        }
        let _: () = msg_send![container_view, addSubview: adv_btn];
        let _: () = msg_send![container_view, addSubview: vendor_field];
        let _: () = msg_send![container_view, addSubview: product_field];
        let _: () = msg_send![alert, setAccessoryView: container_view];

        // 1000 = OK (NSAlertFirstButtonReturn), 1001 = Cancel.
        let response: isize = msg_send![alert, runModal];

        if response == 1000 {
            // chosen: first NSOnState radio row → concrete (u16,u16) (G4 safety
            // net — take-first guarantees a wrong VID/PID can never be picked).
            let chosen: Option<(u16, u16)> = row_btns.iter().enumerate().find_map(|(i, _)| {
                let s: isize = msg_send![row_btns[i], state]; // NSOnState = 1
                (s == 1).then(|| (devices[i].vendor_id, devices[i].product_id))
            });
            // manual: read both typed fields → parse_id_field each (G10: each
            // Option<u16>; blank/"auto" ⇒ None ⇒ auto-discovery). The fields
            // were prefilled with the open-time hex ⇒ "leave as-is" when the
            // user changes nothing + no row picked.
            let vendor_nsstring: *mut Object = msg_send![vendor_field, stringValue];
            let product_nsstring: *mut Object = msg_send![product_field, stringValue];
            let vendor_str = nsstring_to_rust_string(vendor_nsstring)?;
            let product_str = nsstring_to_rust_string(product_nsstring)?;

            match (parse_id_field(&vendor_str), parse_id_field(&product_str)) {
                (Ok(vid), Ok(pid)) => {
                    // Snapshot pre-save VID/PID BEFORE the move below — Config
                    // is Clone (not Copy), so `let mut merged = current_config`
                    // moves it; vendor_id/product_id are Option<u16> (Copy), so
                    // copying them out here is valid and required for the
                    // post-save diff check (Bug 4 / PRD ID 3).
                    let old_vid = current_config.vendor_id;
                    let old_pid = current_config.product_id;

                    // PRESERVE every non-VID/PID field
                    // (usage_page/usage/debounce_ms/poll_interval_ms): overlay
                    // the dialog's result onto the config parsed at dialog-open
                    // time. chosen takes precedence over manual (G10); manual
                    // applies only when no radio row was selected.
                    let mut merged = current_config;
                    if let Some((v, p)) = chosen {
                        merged.vendor_id = Some(v);
                        merged.product_id = Some(p);
                    } else {
                        merged.vendor_id = vid;
                        merged.product_id = pid;
                    }
                    let config_content = crate::core::render_config_body(&merged);
                    crate::core::atomic_write(config_path, &config_content)?;

                    // Bug 4 / PRD ID 3: if VID/PID changed, reset the handshake
                    // state and re-run the handshake for the newly-selected
                    // board. reset_handshake_state clears HOST_CAPABLE/
                    // BOARD_HAS_RULES/CALLBACK_NAMES/HAS_HANDSHAKED;
                    // perform_handshake then re-runs (its HAS_HANDSHAKED guard
                    // was just cleared) and reads config.toml fresh, so the
                    // just-written VID/PID selects the new board and rebuilds its
                    // name→id map. `false` = non-verbose (verbose is not in
                    // scope here — bug_findings.md §132).
                    if merged.vendor_id != old_vid || merged.product_id != old_pid {
                        crate::core::notifier::reset_handshake_state();
                        crate::core::notifier::perform_handshake(false);
                    }
                }
                (Err(e), _) | (_, Err(e)) => {
                    show_macos_error_message(&format!("Invalid input: {}", e));
                }
            }
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn create_nsstring(s: &str) -> Result<*mut objc::runtime::Object, Box<dyn std::error::Error>> {
    use objc::runtime::{Class, Object};
    use objc::{msg_send, sel, sel_impl};
    use std::ffi::CString;

    unsafe {
        let c_string = CString::new(s)?;
        let nsstring_class = Class::get("NSString").ok_or("Failed to get NSString class")?;
        let nsstring: *mut Object =
            msg_send![nsstring_class, stringWithUTF8String: c_string.as_ptr()];

        if nsstring.is_null() {
            return Err("Failed to create NSString".into());
        }

        Ok(nsstring)
    }
}

#[cfg(target_os = "macos")]
fn nsstring_to_rust_string(
    nsstring: *mut objc::runtime::Object,
) -> Result<String, Box<dyn std::error::Error>> {
    use objc::{msg_send, sel, sel_impl};

    unsafe {
        let utf8_ptr: *const i8 = msg_send![nsstring, UTF8String];
        if utf8_ptr.is_null() {
            return Err("Failed to get UTF8 string from NSString".into());
        }

        let c_str = std::ffi::CStr::from_ptr(utf8_ptr);
        Ok(c_str.to_string_lossy().into_owned())
    }
}

#[cfg(target_os = "macos")]
fn show_macos_error_message(message: &str) {
    use objc::runtime::{Class, Object};
    use objc::{msg_send, sel, sel_impl};

    eprintln!("Settings error: {}", message);

    unsafe {
        if let Some(alert_class) = Class::get("NSAlert") {
            let alert: *mut Object = msg_send![alert_class, new];
            if !alert.is_null() {
                if let Ok(title) = create_nsstring("QMKonnect - Error") {
                    if let Ok(msg) = create_nsstring(message) {
                        let _: () = msg_send![alert, setMessageText: title];
                        let _: () = msg_send![alert, setInformativeText: msg];
                        let _: () = msg_send![alert, setAlertStyle: 2]; // NSAlertStyleCritical
                        let _: isize = msg_send![alert, runModal];

                        // Cleanup
                        let _: () = msg_send![title, release];
                        let _: () = msg_send![msg, release];
                        let _: () = msg_send![alert, release];
                    }
                }
            }
        }
    }
}

// Removed broken native macOS implementation
// The tray-icon crate handles this properly, and LSUIElement=true in Info.plist handles dock hiding

// Removed native menu delegate - using tray-icon crate instead

#[cfg(target_os = "macos")]
fn bundle_resource(name: &str) -> Option<std::path::PathBuf> {
    // Resolve a path inside the app bundle's Resources directory without
    // panicking when run as a raw (unbundled) binary: `executable_url()` and
    // `to_path()` both return Option, so propagate None via `?`.
    let bundle = core_foundation::bundle::CFBundle::main_bundle();
    let exec_url = bundle.executable_url()?;
    let exec_path = exec_url.to_path()?;
    let resources_path = exec_path.parent()?.join("../Resources");
    Some(resources_path.join(name))
}

/// Load the dedicated monochrome template asset for the menu bar (macOS).
/// Returns None if the asset is absent (caller falls back to the generated
/// default icon).
#[cfg(target_os = "macos")]
fn load_template_icon_from_bundle() -> Option<tray_icon::Icon> {
    let icon_path = bundle_resource("IconTemplate.png")?;
    load_icon(&icon_path).ok()
}

// ===========================================================================
// "Show Window Information..." dialog (macOS + Windows)
//
// Lists every currently-running foreground window with the value QMKonnect
// actually reports as `application_class` (the macOS localizedName / the Win32
// window class) and the window title. Each row has a copy button that copies
// the *class* — the identifier you match against in your QMK config.
// ===========================================================================

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn handle_window_info_click() {
    let rows = crate::platforms::list_foreground_windows();

    #[cfg(target_os = "macos")]
    {
        if let Err(e) = show_macos_window_info_dialog(&rows) {
            show_macos_error_message(&format!("Failed to show window information: {}", e));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Err(e) = show_window_info_dialog(&rows) {
            show_error_message(&format!("Failed to show window information: {}", e));
        }
    }
}

/// What a row's Copy button puts on the clipboard: the class and the title
/// joined by `|`, or just the class when the window has no title (QMKonnect
/// matches on both when available).
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn copy_text_for_row(row: &(String, String)) -> String {
    let (class, title) = row;
    if title.is_empty() {
        class.clone()
    } else {
        format!("{}|{}", class, title)
    }
}

// ---------------------------------------------------------------------------
// Windows implementation: a Win32 dialog listing every foreground window with
// the class QMKonnect reports and its title. Each row has a selectable,
// read-only label (so you can highlight + Ctrl+C) and a "Copy" button that
// copies "class|title". The window is resizable; rows reflow and scroll between
// a fixed header/footer. Copying uses the clipboard (CF_UNICODETEXT).
// ---------------------------------------------------------------------------

// Layout (pixels). Header + footer are fixed; rows scroll between them.
#[cfg(target_os = "windows")]
const WI_MARGIN: i32 = 14;
#[cfg(target_os = "windows")]
const WI_TOP_PAD: i32 = 14;
#[cfg(target_os = "windows")]
const WI_HEADER_H: i32 = 20;
#[cfg(target_os = "windows")]
const WI_HEADER_GAP: i32 = 8;
#[cfg(target_os = "windows")]
const WI_ROW_H: i32 = 26;
#[cfg(target_os = "windows")]
const WI_LABEL_H: i32 = 22;
#[cfg(target_os = "windows")]
const WI_LABEL_DY: i32 = 2;
#[cfg(target_os = "windows")]
const WI_BTN_W: i32 = 84;
#[cfg(target_os = "windows")]
const WI_BTN_H: i32 = 22;
#[cfg(target_os = "windows")]
const WI_BTN_DY: i32 = 2;
#[cfg(target_os = "windows")]
const WI_BTN_GAP: i32 = 10;
#[cfg(target_os = "windows")]
const WI_FOOTER_H: i32 = 18;
#[cfg(target_os = "windows")]
const WI_FOOTER_GAP: i32 = 8;
#[cfg(target_os = "windows")]
const WI_BOTTOM_PAD: i32 = 12;
// Default + minimum window size (resizable, wider than before).
#[cfg(target_os = "windows")]
const WI_DEF_W: i32 = 760;
#[cfg(target_os = "windows")]
const WI_DEF_H: i32 = 520;
#[cfg(target_os = "windows")]
const WI_MIN_W: i32 = 480;
#[cfg(target_os = "windows")]
const WI_MIN_H: i32 = 320;
// Child control ids (avoid the settings dialog's 1001-1004 range).
#[cfg(target_os = "windows")]
const WI_IDC_HEADER: i32 = 4001;
#[cfg(target_os = "windows")]
const WI_IDC_FOOTER: i32 = 4002;
#[cfg(target_os = "windows")]
const WI_IDC_EMPTY: i32 = 4003;
#[cfg(target_os = "windows")]
const WI_IDC_LABEL_BASE: i32 = 5000; // one read-only label per row
#[cfg(target_os = "windows")]
const WI_IDC_COPY_BASE: i32 = 6000; // one Copy button per row
/// Current vertical scroll offset (pixels), shared with the dialog WndProc.
#[cfg(target_os = "windows")]
static WININFO_SCROLL_POS: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

#[cfg(target_os = "windows")]
pub(crate) fn show_window_info_dialog(
    rows: &[(String, String)],
) -> Result<(), Box<dyn std::error::Error>> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{GetStockObject, UpdateWindow, WHITE_BRUSH};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DispatchMessageW, GetMessageW, GetSystemMetrics, LoadCursorW,
        RegisterClassW, ShowWindow, TranslateMessage, IDC_ARROW, MSG, SM_CXSCREEN, SM_CYSCREEN,
        SW_SHOW, WNDCLASSW, WS_CAPTION, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU,
        WS_THICKFRAME, WS_VSCROLL,
    };

    *WINDOW_INFO_ROWS.lock().unwrap() = rows.to_vec();
    // Start each dialog fresh at the top.
    WININFO_SCROLL_POS.store(0, std::sync::atomic::Ordering::SeqCst);

    unsafe {
        let h_instance = GetModuleHandleW(None)?;
        let class_name = to_wide_string("QMKWindowInfoDialog");
        let window_title = to_wide_string("QMKonnect — Window Information");

        let app_icon = load_app_icon();

        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(window_info_dialog_proc),
            hInstance: h_instance.into(),
            lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            // COLOR_WINDOW (white) so the dialog reads like the macOS scroll
            // view, not a gray property sheet.
            hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(GetStockObject(WHITE_BRUSH).0),
            hIcon: app_icon,
            ..Default::default()
        };
        RegisterClassW(&wnd_class);

        // Fonts live for the duration of the modal loop below.
        let normal_font = create_segoe_ui_font(false);
        let bold_font = create_segoe_ui_font(true);

        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);
        let x = (screen_width - WI_DEF_W) / 2;
        let y = (screen_height - WI_DEF_H) / 2;

        // Created hidden: WS_VISIBLE would trigger WM_SIZE before the child
        // controls exist. We create the controls, then ShowWindow.
        let hwnd = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            windows::core::PCWSTR(class_name.as_ptr()),
            windows::core::PCWSTR(window_title.as_ptr()),
            WS_OVERLAPPED
                | WS_CAPTION
                | WS_SYSMENU
                | WS_THICKFRAME
                | WS_MINIMIZEBOX
                | WS_MAXIMIZEBOX
                | WS_VSCROLL,
            x,
            y,
            WI_DEF_W,
            WI_DEF_H,
            HWND(0),
            None,
            h_instance,
            Some(std::ptr::null()),
        );

        if hwnd.0 == 0 {
            return Err("Failed to create window information dialog".into());
        }

        create_window_info_rows(hwnd, h_instance.into(), normal_font, bold_font)?;
        wininfo_relayout(hwnd);

        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);

        // Modal message loop until WM_DESTROY posts WM_QUIT.
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}

/// Create a Segoe UI font (the modern Windows UI face) at ~9pt. `bold` for the
/// header. Returns an `HFONT` owned for the dialog's lifetime by the caller.
#[cfg(target_os = "windows")]
fn create_segoe_ui_font(bold: bool) -> windows::Win32::Graphics::Gdi::HFONT {
    use windows::Win32::Graphics::Gdi::CreateFontW;
    unsafe {
        let face = to_wide_string("Segoe UI");
        CreateFontW(
            -12, // ~9pt at 96dpi (character height)
            0,
            0,
            0,
            if bold { 700 } else { 400 }, // FW_BOLD / FW_NORMAL
            0,
            0,
            0,
            1, // DEFAULT_CHARSET
            0, // OUT_DEFAULT_PRECIS
            0, // CLIP_DEFAULT_PRECIS
            5, // CLEARTYPE_QUALITY
            0, // DEFAULT_PITCH
            windows::core::PCWSTR(face.as_ptr()),
        )
    }
}

#[cfg(target_os = "windows")]
unsafe fn create_window_info_rows(
    hwnd: windows::Win32::Foundation::HWND,
    h_instance: windows::Win32::Foundation::HINSTANCE,
    normal_font: windows::Win32::Graphics::Gdi::HFONT,
    bold_font: windows::Win32::Graphics::Gdi::HFONT,
) -> Result<(), Box<dyn std::error::Error>> {
    use windows::Win32::UI::Controls::{WC_BUTTONW, WC_EDITW, WC_STATICW};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, SendMessageW, ES_AUTOHSCROLL, ES_NOHIDESEL, ES_READONLY, HMENU,
        WINDOW_STYLE, WM_SETFONT, WS_CHILD, WS_TABSTOP, WS_VISIBLE,
    };

    let rows = WINDOW_INFO_ROWS.lock().unwrap();
    let n = rows.len();

    // Helper to apply a font to a control.
    let set_font = |ctl: windows::Win32::Foundation::HWND,
                    font: windows::Win32::Graphics::Gdi::HFONT| {
        let _ = SendMessageW(
            ctl,
            WM_SETFONT,
            windows::Win32::Foundation::WPARAM(font.0 as usize),
            windows::Win32::Foundation::LPARAM(1),
        );
    };

    // Header (bold). Positioned by wininfo_relayout.
    let header_text = to_wide_string("Class (what QMKonnect reports)   \u{2014}   Window title");
    let header = CreateWindowExW(
        windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
        WC_STATICW,
        windows::core::PCWSTR(header_text.as_ptr()),
        WS_CHILD | WS_VISIBLE,
        0,
        0,
        0,
        0,
        hwnd,
        HMENU(WI_IDC_HEADER as isize),
        h_instance,
        Some(std::ptr::null()),
    );
    set_font(header, bold_font);

    // Footer hint.
    let footer_text = to_wide_string(
        "Tip: select any text to copy it (Ctrl+C), or click a row's Copy button (copies \"class|title\").",
    );
    let footer = CreateWindowExW(
        windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
        WC_STATICW,
        windows::core::PCWSTR(footer_text.as_ptr()),
        WS_CHILD | WS_VISIBLE,
        0,
        0,
        0,
        0,
        hwnd,
        HMENU(WI_IDC_FOOTER as isize),
        h_instance,
        Some(std::ptr::null()),
    );
    set_font(footer, normal_font);

    if n == 0 {
        let empty = to_wide_string("No foreground windows detected.");
        let empty_ctl = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            WC_STATICW,
            windows::core::PCWSTR(empty.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            0,
            0,
            0,
            0,
            hwnd,
            HMENU(WI_IDC_EMPTY as isize),
            h_instance,
            Some(std::ptr::null()),
        );
        set_font(empty_ctl, normal_font);
        return Ok(());
    }

    for (i, (class, title)) in rows.iter().enumerate() {
        let id = i as i32;
        let line = if title.is_empty() {
            class.clone()
        } else {
            format!("{}    \u{2014}    {}", class, title)
        };
        let wide = to_wide_string(&line);
        // Read-only, borderless, selectable EDIT. ES_NOHIDESEL keeps the
        // selection visible when focus moves to the Copy button.
        let label = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            WC_EDITW,
            windows::core::PCWSTR(wide.as_ptr()),
            WS_CHILD
                | WS_VISIBLE
                | WINDOW_STYLE(ES_READONLY as u32 | ES_AUTOHSCROLL as u32 | ES_NOHIDESEL as u32),
            0,
            0,
            0,
            0,
            hwnd,
            HMENU((WI_IDC_LABEL_BASE + id) as isize),
            h_instance,
            Some(std::ptr::null()),
        );
        set_font(label, normal_font);

        let copy_text = to_wide_string("Copy");
        let btn = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            WC_BUTTONW,
            windows::core::PCWSTR(copy_text.as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            0,
            0,
            0,
            0,
            hwnd,
            HMENU((WI_IDC_COPY_BASE + id) as isize),
            h_instance,
            Some(std::ptr::null()),
        );
        set_font(btn, normal_font);
    }

    Ok(())
}

#[cfg(target_os = "windows")]
unsafe fn wininfo_client_size(hwnd: windows::Win32::Foundation::HWND) -> (i32, i32) {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

    let mut rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut rc);
    (rc.right - rc.left, rc.bottom - rc.top)
}

/// Compute the scroll geometry from the current client size and row count:
/// `(viewport_top, viewport_bottom, viewport_h, max_scroll)`. The header and
/// footer are fixed; rows live in the viewport between them.
#[cfg(target_os = "windows")]
unsafe fn wininfo_geometry(hwnd: windows::Win32::Foundation::HWND) -> (i32, i32, i32, i32) {
    let (_, ch) = wininfo_client_size(hwnd);
    let n = WINDOW_INFO_ROWS.lock().unwrap().len() as i32;
    let content_top = WI_TOP_PAD + WI_HEADER_H + WI_HEADER_GAP;
    let footer_y = ch - WI_BOTTOM_PAD - WI_FOOTER_H;
    let viewport_top = content_top;
    let viewport_bottom = footer_y - WI_FOOTER_GAP;
    let viewport_h = (viewport_bottom - viewport_top).max(0);
    let max_scroll = (n * WI_ROW_H - viewport_h).max(0);
    (viewport_top, viewport_bottom, viewport_h, max_scroll)
}

/// Move a child control by id. Null-safe (skips controls that don't exist yet,
/// e.g. during the initial WM_SIZE before `create_window_info_rows` runs).
#[cfg(target_os = "windows")]
unsafe fn wininfo_move_id(
    hwnd: windows::Win32::Foundation::HWND,
    id: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    use windows::Win32::UI::WindowsAndMessaging::{GetDlgItem, MoveWindow};
    let ctl = GetDlgItem(hwnd, id);
    if ctl.0 != 0 {
        let _ = MoveWindow(ctl, x, y, w, h, true);
    }
}

#[cfg(target_os = "windows")]
unsafe fn wininfo_set_scrollbar(
    hwnd: windows::Win32::Foundation::HWND,
    max_scroll: i32,
    page: i32,
    pos: i32,
) {
    use windows::Win32::UI::Controls::SetScrollInfo;
    use windows::Win32::UI::WindowsAndMessaging::{
        SB_VERT, SCROLLINFO, SIF_DISABLENOSCROLL, SIF_PAGE, SIF_POS, SIF_RANGE,
    };

    let si = SCROLLINFO {
        cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
        fMask: SIF_RANGE | SIF_PAGE | SIF_POS | SIF_DISABLENOSCROLL,
        nMin: 0,
        nMax: max_scroll,
        nPage: page.max(0) as u32,
        nPos: pos,
        nTrackPos: 0,
    };
    SetScrollInfo(hwnd, SB_VERT, &si, true);
}

/// Reposition every child to match the current size and scroll offset. Called
/// on init, WM_SIZE, and every scroll/wheel event. Header/footer are fixed;
/// rows scroll between them; off-screen rows are hidden to avoid needless
/// repaint.
#[cfg(target_os = "windows")]
unsafe fn wininfo_relayout(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetDlgItem, MoveWindow, ShowWindow, SW_HIDE, SW_SHOWNA,
    };

    let (cw, ch) = wininfo_client_size(hwnd);
    let (viewport_top, viewport_bottom, viewport_h, max_scroll) = wininfo_geometry(hwnd);
    let n = WINDOW_INFO_ROWS.lock().unwrap().len() as i32;
    let scroll = WININFO_SCROLL_POS
        .load(std::sync::atomic::Ordering::SeqCst)
        .clamp(0, max_scroll);
    WININFO_SCROLL_POS.store(scroll, std::sync::atomic::Ordering::SeqCst);

    let content_top = WI_TOP_PAD + WI_HEADER_H + WI_HEADER_GAP;
    let footer_y = ch - WI_BOTTOM_PAD - WI_FOOTER_H;
    let label_w = (cw - 2 * WI_MARGIN - WI_BTN_W - WI_BTN_GAP).max(60);
    let btn_x = cw - WI_MARGIN - WI_BTN_W;

    // Fixed header + footer.
    wininfo_move_id(
        hwnd,
        WI_IDC_HEADER,
        WI_MARGIN,
        WI_TOP_PAD,
        cw - 2 * WI_MARGIN,
        WI_HEADER_H,
    );
    wininfo_move_id(
        hwnd,
        WI_IDC_FOOTER,
        WI_MARGIN,
        footer_y,
        cw - 2 * WI_MARGIN,
        WI_FOOTER_H,
    );
    if GetDlgItem(hwnd, WI_IDC_EMPTY).0 != 0 {
        wininfo_move_id(
            hwnd,
            WI_IDC_EMPTY,
            WI_MARGIN,
            content_top + 4,
            cw - 2 * WI_MARGIN,
            WI_ROW_H,
        );
    }

    // Rows: position in-view ones, hide the rest.
    for i in 0..n {
        let abs_y = content_top + i * WI_ROW_H;
        let vis_y = abs_y - scroll;
        let in_view = vis_y + WI_ROW_H > viewport_top && vis_y < viewport_bottom;
        let cmd = if in_view { SW_SHOWNA } else { SW_HIDE };
        let label = GetDlgItem(hwnd, WI_IDC_LABEL_BASE + i);
        let btn = GetDlgItem(hwnd, WI_IDC_COPY_BASE + i);
        if label.0 != 0 {
            let _ = ShowWindow(label, cmd);
            if in_view {
                let _ = MoveWindow(
                    label,
                    WI_MARGIN,
                    vis_y + WI_LABEL_DY,
                    label_w,
                    WI_LABEL_H,
                    true,
                );
            }
        }
        if btn.0 != 0 {
            let _ = ShowWindow(btn, cmd);
            if in_view {
                let _ = MoveWindow(btn, btn_x, vis_y + WI_BTN_DY, WI_BTN_W, WI_BTN_H, true);
            }
        }
    }

    wininfo_set_scrollbar(hwnd, max_scroll, viewport_h, scroll);
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn window_info_dialog_proc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::Graphics::Gdi::{
        GetStockObject, SetBkMode, SetTextColor, HDC, TRANSPARENT, WHITE_BRUSH,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        DefWindowProcW, DestroyWindow, PostQuitMessage, MINMAXINFO, SB_BOTTOM, SB_LINEDOWN,
        SB_LINEUP, SB_PAGEDOWN, SB_PAGEUP, SB_THUMBPOSITION, SB_THUMBTRACK, SB_TOP,
        SCROLLBAR_COMMAND, WM_CLOSE, WM_COMMAND, WM_CTLCOLORSTATIC, WM_DESTROY, WM_GETMINMAXINFO,
        WM_MOUSEWHEEL, WM_SIZE, WM_VSCROLL,
    };

    match msg {
        WM_SIZE => {
            wininfo_relayout(hwnd);
            let _ = windows::Win32::Graphics::Gdi::UpdateWindow(hwnd);
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_GETMINMAXINFO => {
            // Enforce a minimum size so the layout never collapses.
            let pmmi = lparam.0 as *mut MINMAXINFO;
            if !pmmi.is_null() {
                (*pmmi).ptMinTrackSize.x = WI_MIN_W;
                (*pmmi).ptMinTrackSize.y = WI_MIN_H;
            }
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_CTLCOLORSTATIC => {
            // White background + black text for the static labels AND the
            // read-only edits (both send WM_CTLCOLORSTATIC), matching the
            // macOS scroll-view look.
            let hdc = HDC(wparam.0 as isize);
            let _ = SetTextColor(hdc, COLORREF(0));
            let _ = SetBkMode(hdc, TRANSPARENT);
            windows::Win32::Foundation::LRESULT(GetStockObject(WHITE_BRUSH).0)
        }
        WM_VSCROLL => {
            // The low word of wParam is the SB_* scroll code. The windows
            // crate types these as `SCROLLBAR_COMMAND` (a newtype that derives
            // PartialEq/Eq but NOT BitOr), so combine the aliased arms with
            // match guards (`||`) instead of `|` patterns.
            let (_, _, vh, max_scroll) = wininfo_geometry(hwnd);
            let cur = WININFO_SCROLL_POS.load(std::sync::atomic::Ordering::SeqCst);
            let code = SCROLLBAR_COMMAND((wparam.0 & 0xFFFF) as i32);
            let new = match code {
                SB_LINEDOWN => cur + WI_ROW_H,
                SB_PAGEUP => cur - vh,
                SB_PAGEDOWN => cur + vh,
                SB_BOTTOM => i32::MAX,
                c if c == SB_LINEUP || c == SB_TOP => cur - WI_ROW_H,
                c if c == SB_THUMBTRACK || c == SB_THUMBPOSITION => (wparam.0 >> 16) as u16 as i32,
                _ => cur,
            };
            WININFO_SCROLL_POS.store(
                new.clamp(0, max_scroll),
                std::sync::atomic::Ordering::SeqCst,
            );
            wininfo_relayout(hwnd);
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let (_, _, _, max_scroll) = wininfo_geometry(hwnd);
            let delta = ((wparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
            let cur = WININFO_SCROLL_POS.load(std::sync::atomic::Ordering::SeqCst);
            // WHEEL_DELTA (120) per notch; scroll three rows per notch.
            let new = cur - (delta / 120) * WI_ROW_H * 3;
            WININFO_SCROLL_POS.store(
                new.clamp(0, max_scroll),
                std::sync::atomic::Ordering::SeqCst,
            );
            wininfo_relayout(hwnd);
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            if id >= WI_IDC_COPY_BASE {
                let idx = (id - WI_IDC_COPY_BASE) as usize;
                let text = WINDOW_INFO_ROWS
                    .lock()
                    .unwrap()
                    .get(idx)
                    .map(copy_text_for_row);
                if let Some(t) = text {
                    copy_to_clipboard_windows(hwnd, &t);
                }
                return windows::Win32::Foundation::LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            windows::Win32::Foundation::LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Copy `text` to the clipboard as CF_UNICODETEXT (Windows).
#[cfg(target_os = "windows")]
fn copy_to_clipboard_windows(hwnd: windows::Win32::Foundation::HWND, text: &str) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    // Standard Windows clipboard format value (winuser.h). Defined here as a
    // plain `u32` because `SetClipboardData` takes the format as `u32` and the
    // windows-crate constant lives behind the heavyweight `Win32_System_Ole`
    // feature, which we don't otherwise need.
    const CF_UNICODETEXT: u32 = 13;

    let wide: Vec<u16> = OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let byte_len = wide.len() * 2;

    unsafe {
        if OpenClipboard(hwnd).is_err() {
            return;
        }
        let _ = EmptyClipboard();

        // `GlobalAlloc` returns `Result<HGLOBAL>`; `GlobalLock` returns a raw
        // pointer directly (NOT a Result). On a successful `SetClipboardData`
        // the OS takes ownership of the HGLOBAL, so it must not be freed here.
        if let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE, byte_len) {
            let ptr = GlobalLock(hmem);
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr as *mut u8, byte_len);
                let _ = GlobalUnlock(hmem);
                // HGLOBAL(*mut c_void) -> HANDLE(isize): re-wrap the pointer
                // value; the cast is always valid for a Windows handle.
                let _ = SetClipboardData(
                    CF_UNICODETEXT,
                    windows::Win32::Foundation::HANDLE(hmem.0 as isize),
                );
            }
        }

        let _ = CloseClipboard();
    }
}

// ---------------------------------------------------------------------------
// macOS implementation: an NSWindow containing an NSScrollView whose document
// view holds one row per app (label + SF-Symbol copy button). The copy-button
// target and the windowWillClose delegate are tiny NSObject subclasses.
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
extern "C" fn wi_copy_row(
    _this: &objc::runtime::Object,
    _sel: objc::runtime::Sel,
    sender: *mut objc::runtime::Object,
) {
    // `msg_send!` expands to `sel!` + the `sel_impl` trait, so they must be
    // imported into this function's own scope.
    use objc::{msg_send, sel, sel_impl};
    let idx: isize = unsafe { msg_send![sender, tag] };
    let text = WINDOW_INFO_ROWS
        .lock()
        .unwrap()
        .get(idx as usize)
        .map(copy_text_for_row);
    if let Some(t) = text {
        copy_to_pasteboard_macos(&t);
    }
}

#[cfg(target_os = "macos")]
extern "C" fn wi_window_will_close(
    _this: &objc::runtime::Object,
    _sel: objc::runtime::Sel,
    _notif: *mut objc::runtime::Object,
) {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, stopModal];
    }
}

// Escape closes the macOS "Show Window Information" window. NSResponder sends
// `cancelOperation:` when the user presses Escape (NSWindow's default
// `performKeyEquivalent:` routes Escape to it on the responder chain); the
// default impl is a no-op, so override it on the window subclass (registered in
// `show_macos_window_info_dialog_inner`) to close the window →
// `windowWillClose:` → `[NSApp stopModal]`. (Keyboard-power-user UX; spec/UI.md
// §3.5.)
#[cfg(target_os = "macos")]
extern "C" fn wi_cancel_operation(
    this: &objc::runtime::Object,
    _sel: objc::runtime::Sel,
    _sender: *mut objc::runtime::Object,
) {
    use objc::{msg_send, sel, sel_impl};
    let _: () = unsafe { msg_send![this, close] };
}

#[cfg(target_os = "macos")]
fn copy_to_pasteboard_macos(text: &str) {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    let nsstring = match create_nsstring(text) {
        Ok(s) => s,
        Err(_) => return,
    };

    unsafe {
        let pb: *mut Object = msg_send![class!(NSPasteboard), generalPasteboard];
        if pb.is_null() {
            return;
        }
        let _: () = msg_send![pb, clearContents];
        let array: *mut Object = msg_send![class!(NSArray), arrayWithObject: nsstring];
        let _: () = msg_send![pb, writeObjects: array];
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn show_macos_window_info_dialog(
    rows: &[(String, String)],
) -> Result<(), Box<dyn std::error::Error>> {
    use objc::runtime::{Class, Object};
    use objc::{msg_send, sel, sel_impl};

    // Publish rows for the per-row copy-button target to read by index.
    *WINDOW_INFO_ROWS.lock().unwrap() = rows.to_vec();

    unsafe {
        // Background (LSUIElement) apps lack a main autorelease pool; create one
        // for the lifetime of the dialog.
        let pool_class =
            Class::get("NSAutoreleasePool").ok_or("NSAutoreleasePool class not found")?;
        let pool: *mut Object = msg_send![pool_class, new];
        if pool.is_null() {
            return Err("Failed to create autorelease pool".into());
        }

        let result = show_macos_window_info_dialog_inner();

        let _: () = msg_send![pool, drain];
        result
    }
}

#[cfg(target_os = "macos")]
fn show_macos_window_info_dialog_inner() -> Result<(), Box<dyn std::error::Error>> {
    use objc::runtime::{Class, Object, NO, YES};
    use objc::{class, declare::ClassDecl, msg_send, sel, sel_impl};

    unsafe {
        let app: *mut Object = msg_send![
            Class::get("NSApplication").ok_or("NSApplication class not found")?,
            sharedApplication
        ];
        // A background app must activate or its windows can't become key.
        let _: () = msg_send![app, activateIgnoringOtherApps: YES];

        // --- Register the copy-button target class (once). ------------------
        if Class::get("RustWindowInfoCopyTarget").is_none() {
            let superclass = Class::get("NSObject").ok_or("NSObject class not found")?;
            let mut decl = ClassDecl::new("RustWindowInfoCopyTarget", superclass)
                .ok_or("failed to declare RustWindowInfoCopyTarget")?;
            decl.add_method(
                sel!(copyRow:),
                wi_copy_row
                    as extern "C" fn(
                        &objc::runtime::Object,
                        objc::runtime::Sel,
                        *mut objc::runtime::Object,
                    ),
            );
            decl.register();
        }
        let target: *mut Object = msg_send![
            Class::get("RustWindowInfoCopyTarget").ok_or("RustWindowInfoCopyTarget missing")?,
            new
        ];

        // --- Register the window delegate class (once). ---------------------
        if Class::get("RustWindowInfoWindowDelegate").is_none() {
            let superclass = Class::get("NSObject").ok_or("NSObject class not found")?;
            let mut decl = ClassDecl::new("RustWindowInfoWindowDelegate", superclass)
                .ok_or("failed to declare RustWindowInfoWindowDelegate")?;
            decl.add_method(
                sel!(windowWillClose:),
                wi_window_will_close
                    as extern "C" fn(
                        &objc::runtime::Object,
                        objc::runtime::Sel,
                        *mut objc::runtime::Object,
                    ),
            );
            decl.register();
        }
        let delegate: *mut Object = msg_send![
            Class::get("RustWindowInfoWindowDelegate")
                .ok_or("RustWindowInfoWindowDelegate missing")?,
            new
        ];

        // --- Register the NSWindow subclass that closes on Escape ----------
        // (cancelOperation: → wi_cancel_operation → close). Built once; alloc'd
        // below in place of a bare NSWindow. spec/UI.md §3.5.
        if Class::get("RustWindowInfoWindow").is_none() {
            let superclass = Class::get("NSWindow").ok_or("NSWindow class not found")?;
            let mut decl = ClassDecl::new("RustWindowInfoWindow", superclass)
                .ok_or("failed to declare RustWindowInfoWindow")?;
            decl.add_method(
                sel!(cancelOperation:),
                wi_cancel_operation
                    as extern "C" fn(
                        &objc::runtime::Object,
                        objc::runtime::Sel,
                        *mut objc::runtime::Object,
                    ),
            );
            decl.register();
        }

        // --- Build the window. ---------------------------------------------
        let n = WINDOW_INFO_ROWS.lock().unwrap().len();

        let window_width: f64 = 500.0;
        let window_height: f64 = 420.0;
        let content_rect = objc_types::NSRect {
            origin: objc_types::NSPoint { x: 0.0, y: 0.0 },
            size: objc_types::NSSize {
                width: window_width,
                height: window_height,
            },
        };
        // NSTitledWindowMask(1) | NSClosableWindowMask(2) | NSMiniaturizableWindowMask(4)
        let style_mask: u64 = 1 | 2 | 4;
        let backing: u64 = 2; // NSBackingStoreBuffered

        // Create the window: `alloc` (a class method) THEN
        // `initWithContentRect:styleMask:backing:defer:` on the instance.
        // Sending that instance selector straight to the class raises
        // doesNotRecognizeSelector and aborts the process.
        let allocated: *mut Object = msg_send![
            Class::get("RustWindowInfoWindow").ok_or("RustWindowInfoWindow missing")?,
            alloc
        ];
        let window: *mut Object = msg_send![
            allocated,
            initWithContentRect: content_rect
            styleMask: style_mask
            backing: backing
            defer: NO
        ];
        if window.is_null() {
            let _: () = msg_send![delegate, release];
            let _: () = msg_send![target, release];
            return Err("Failed to create window".into());
        }

        // We own the window; closing must not release it out from under us.
        let _: () = msg_send![window, setReleasedWhenClosed: NO];

        let title = create_nsstring("QMKonnect — Window Information")?;
        let _: () = msg_send![window, setTitle: title];
        let _: () = msg_send![window, setDelegate: delegate];
        let _: () = msg_send![window, center];

        let content_view: *mut Object = msg_send![window, contentView];

        // --- Scroll view + document view (rows live here). -----------------
        let scroll_view: *mut Object = msg_send![class!(NSScrollView), new];
        let _: () = msg_send![scroll_view, setFrame: content_rect];
        let _: () = msg_send![scroll_view, setHasVerticalScroller: YES];
        let _: () = msg_send![scroll_view, setAutohidesScrollers: YES];
        let _: () = msg_send![scroll_view, setDrawsBackground: NO];

        let row_height: f64 = 30.0;
        let doc_width = content_rect.size.width;
        let doc_height = (n as f64 * row_height).max(content_rect.size.height);
        let doc_view: *mut Object = msg_send![class!(NSView), new];
        let _: () = msg_send![
            doc_view,
            setFrame: objc_types::NSRect {
                origin: objc_types::NSPoint { x: 0.0, y: 0.0 },
                size: objc_types::NSSize {
                    width: doc_width,
                    height: doc_height,
                },
            }
        ];
        let _: () = msg_send![doc_view, setWantsLayer: YES];

        let rows = WINDOW_INFO_ROWS.lock().unwrap();
        for (i, (class, title_text)) in rows.iter().enumerate() {
            // NSView origin is bottom-left, so top-align rows by counting down.
            let y = doc_height - (i as f64 + 1.0) * row_height;
            let row_view: *mut Object = msg_send![class!(NSView), new];
            let _: () = msg_send![
                row_view,
                setFrame: objc_types::NSRect {
                    origin: objc_types::NSPoint { x: 0.0, y },
                    size: objc_types::NSSize {
                        width: doc_width,
                        height: row_height,
                    },
                }
            ];

            let label_text = if title_text.is_empty() {
                class.clone()
            } else {
                format!("{}    \u{2014}    {}", class, title_text)
            };
            let label: *mut Object = msg_send![class!(NSTextField), new];
            let ns = create_nsstring(&label_text)?;
            let _: () = msg_send![label, setStringValue: ns];
            let _: () = msg_send![label, setBezeled: NO];
            let _: () = msg_send![label, setDrawsBackground: NO];
            let _: () = msg_send![label, setEditable: NO];
            let _: () = msg_send![label, setSelectable: YES];
            let _: () = msg_send![label, setLineBreakMode: 3u64]; // NSLineBreakByTruncatingTail
            let font: *mut Object = msg_send![class!(NSFont), systemFontOfSize: 12.0f64];
            let _: () = msg_send![label, setFont: font];
            let _: () = msg_send![
                label,
                setFrame: objc_types::NSRect {
                    origin: objc_types::NSPoint { x: 12.0, y: 6.0 },
                    size: objc_types::NSSize {
                        width: doc_width - 12.0 - 104.0,
                        height: 18.0,
                    },
                }
            ];
            let _: () = msg_send![row_view, addSubview: label];

            // Copy button: SF Symbol where available, text fallback otherwise.
            let button: *mut Object = msg_send![class!(NSButton), new];
            let _: () = msg_send![button, setTag: i as isize];
            let _: () = msg_send![button, setTarget: target];
            let _: () = msg_send![button, setAction: sel!(copyRow:)];
            // imageWithSystemSymbolName:accessibilityDescription: is macOS 11+.
            // Guard with respondsToSelector: so older OSes get a text button
            // instead of crashing on an unrecognized selector.
            let sym_sel = sel!(imageWithSystemSymbolName:accessibilityDescription:);
            let responds: bool = msg_send![class!(NSImage), respondsToSelector: sym_sel];
            if responds {
                let sym_name = create_nsstring("doc.on.doc")?;
                let sym_desc = create_nsstring("Copy class")?;
                let symbol: *mut Object = msg_send![
                    class!(NSImage),
                    imageWithSystemSymbolName: sym_name
                    accessibilityDescription: sym_desc
                ];
                if !symbol.is_null() {
                    let _: () = msg_send![button, setImage: symbol];
                    let _: () = msg_send![button, setImagePosition: 1u64]; // NSImageOnly
                } else {
                    let btn_title = create_nsstring("Copy")?;
                    let _: () = msg_send![button, setTitle: btn_title];
                }
            } else {
                let btn_title = create_nsstring("Copy")?;
                let _: () = msg_send![button, setTitle: btn_title];
            }
            let _: () = msg_send![
                button,
                setFrame: objc_types::NSRect {
                    origin: objc_types::NSPoint { x: doc_width - 96.0, y: 1.0 },
                    size: objc_types::NSSize {
                        width: 88.0,
                        height: row_height - 2.0,
                    },
                }
            ];
            let _: () = msg_send![row_view, addSubview: button];

            let _: () = msg_send![doc_view, addSubview: row_view];
        }
        drop(rows);

        if n == 0 {
            let label: *mut Object = msg_send![class!(NSTextField), new];
            let ns = create_nsstring("No foreground windows detected.")?;
            let _: () = msg_send![label, setStringValue: ns];
            let _: () = msg_send![label, setBezeled: NO];
            let _: () = msg_send![label, setDrawsBackground: NO];
            let _: () = msg_send![label, setEditable: NO];
            let _: () = msg_send![
                label,
                setFrame: objc_types::NSRect {
                    origin: objc_types::NSPoint { x: 12.0, y: doc_height - 24.0 },
                    size: objc_types::NSSize {
                        width: doc_width - 24.0,
                        height: 18.0,
                    },
                }
            ];
            let _: () = msg_send![doc_view, addSubview: label];
        }

        let _: () = msg_send![scroll_view, setDocumentView: doc_view];
        let _: () = msg_send![content_view, addSubview: scroll_view];

        // Background (LSUIElement) apps are not reliably active, so
        // runModalForWindow: alone may leave the window buried behind other
        // apps. Activate explicitly and order the window front first.
        let _: () = msg_send![app, activateIgnoringOtherApps: YES];
        let _: () = msg_send![window, makeKeyAndOrderFront: std::ptr::null_mut::<Object>()];

        // Run modal until the window is closed (the delegate calls stopModal).
        let _: isize = msg_send![app, runModalForWindow: window];

        // --- Cleanup -------------------------------------------------------
        let _: () = msg_send![window, setDelegate: std::ptr::null_mut::<Object>()];
        let _: () = msg_send![window, release];
        let _: () = msg_send![delegate, release];
        let _: () = msg_send![target, release];
    }

    Ok(())
}

#[cfg(all(test, any(target_os = "macos", target_os = "windows")))]
mod tests {
    use super::{device_status_text, picker_row_text};
    use crate::core::notifier::DeviceStatus;

    #[test]
    fn test_device_status_text_three_states() {
        // The three exact tray strings per `spec/UI.md` §4 /
        // `spec/DEVICE_DISCOVERY.md` §3: glyphs as \u{} escapes, two spaces
        // after each glyph, and an em-dash (\u{2014}) in the No-module line.
        assert_eq!(
            device_status_text(DeviceStatus::Connected),
            "\u{25CF}  Device Connected"
        );
        assert_eq!(
            device_status_text(DeviceStatus::NoModule),
            "\u{26A0}  QMK board found \u{2014} no qmk_notifier module (flash it)"
        );
        assert_eq!(
            device_status_text(DeviceStatus::Disconnected),
            "\u{25CB}  No Device Connected"
        );
    }

    // The pure LISTBOX row builder for the discovered-device picker
    // (`spec/DEVICE_DISCOVERY.md` §5.1 / §3). The Win32 dialog itself spawns a
    // real message loop and is NOT unit-testable; only this string builder is.
    #[cfg(target_os = "windows")]
    #[test]
    fn test_picker_row_text_glyphs() {
        use crate::core::notifier::{ClassifiedDevice, DeviceKind};

        let capable = ClassifiedDevice {
            path: String::new(),
            vendor_id: 0xFEED,
            product_id: 0x0000,
            product_name: Some("Dactyl".into()),
            usage_page: 0xFF60,
            usage: 0x61,
            kind: DeviceKind::Capable {
                proto_ver: 2,
                feature_flags: 1,
                callback_count: 0,
                board_rules_present: false,
            },
        };
        let notqmk = ClassifiedDevice {
            kind: DeviceKind::NotQmkNotifier,
            vendor_id: 0x3434,
            product_id: 0x0123,
            product_name: Some("Keychron".into()),
            ..capable.clone()
        };

        let cap_row = picker_row_text(&capable);
        let nq_row = picker_row_text(&notqmk);

        assert!(
            cap_row.starts_with('\u{2713}'),
            "capable row starts with ✓: {cap_row}"
        );
        assert!(cap_row.contains("0xFEED:0x0000") && cap_row.contains("qmk_notifier"));
        assert!(
            nq_row.starts_with('\u{2717}'),
            "notqmk row starts with ✗: {nq_row}"
        );
        assert!(nq_row.contains("0x3434:0x0123") && nq_row.contains("QMK board, no module"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_picker_row_text_glyphs() {
        use crate::core::notifier::{ClassifiedDevice, DeviceKind};

        let capable = ClassifiedDevice {
            path: String::new(),
            vendor_id: 0xFEED,
            product_id: 0x0000,
            product_name: Some("Dactyl".into()),
            usage_page: 0xFF60,
            usage: 0x61,
            kind: DeviceKind::Capable {
                proto_ver: 2,
                feature_flags: 1,
                callback_count: 0,
                board_rules_present: false,
            },
        };
        let notqmk = ClassifiedDevice {
            kind: DeviceKind::NotQmkNotifier,
            vendor_id: 0x3434,
            product_id: 0x0123,
            product_name: Some("Keychron".into()),
            ..capable.clone()
        };

        let cap_row = picker_row_text(&capable);
        let nq_row = picker_row_text(&notqmk);

        assert!(
            cap_row.starts_with('\u{2713}'),
            "capable row starts with ✓: {cap_row}"
        );
        assert!(cap_row.contains("0xFEED:0x0000") && cap_row.contains("qmk_notifier"));
        assert!(
            nq_row.starts_with('\u{2717}'),
            "notqmk row starts with ✗: {nq_row}"
        );
        assert!(nq_row.contains("0x3434:0x0123") && nq_row.contains("QMK board, no module"));
    }
}

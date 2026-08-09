//! StatusNotifierItem (SNI) system-tray entry for the Linux/Wayland build.
//!
//! Registers QMKonnect over the session D-Bus as a `StatusNotifierItem` so that
//! any SNI-hosting status bar (Waybar, SwayNC, ironbar, KDE Plasma, GNOME via
//! the AppIndicator/KStatusNotifierItem extension, …) can render the menu-bar
//! icon and menu.
//!
//! This is the freedesktop standard that Steam, Discord, ClickUp, etc. use to
//! appear in the tray *without targeting any particular compositor* — Hyprland
//! itself is uninvolved; the bar talks D-Bus directly. It is pure IPC (no GTK
//! main loop, no X11), so it slots into the Hyprland IPC runner unchanged:
//! ksni owns its own D-Bus thread and the runner keeps blocking on its event
//! listener.
//!
//! User-facing interactions shell out to standard desktop tools instead of
//! pulling extra D-Bus/GUI crates:
//!   * "Show Window Information" → `zenity --forms --add-list` (a real dialog,
//!     so it floats on every tiling compositor; the chosen row is copied to the
//!     clipboard).
//!   * "Settings…"              → `zenity --forms` (writes `config.toml`).
//!
//! # Dark-theme icon handling
//! Unlike macOS menu-bar *template* images (which the system auto-tints to the
//! bar) or GTK *symbolic* icons (which the theme recolors), an SNI `IconPixmap`
//! is a literal bitmap the host renders verbatim — there is no auto-adaptation.
//! So we read the desktop's preferred color scheme ourselves, via the same
//! `org.freedesktop.appearance.color-scheme` Settings portal that GTK/libadwaita
//! and KDE consult, and serve a matching pre-rendered variant: a light-outlined
//! icon for dark bars and a dark-outlined icon for light bars. The poll thread
//! re-checks the portal periodically and swaps the variant (calling
//! `handle.update()`) when the user toggles their theme.
//!
//! If no SNI-hosting bar is running there is simply no host to render the icon,
//! which is expected on Linux (see HANDOFF_LINUX_TRAY_PARITY.md §3/§9) — the
//! notifier keeps running trayless.
#![cfg(all(target_os = "linux", feature = "linux-tray"))]

use crate::core::notifier::DeviceStatus;
use ksni::blocking::TrayMethods;
use ksni::menu::StandardItem;
use ksni::{Category, Icon, MenuItem, ToolTip};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Official QMKonnect icon rendered for a **dark** bar: the full-color mark with
/// a light outline so its dark keys stay visible against dark panels. 128×128.
const TRAY_ICON_DARK_PNG: &[u8] = include_bytes!("../packaging/IconTray-dark.png");
/// Official QMKonnect icon rendered for a **light** bar: the full-color mark with
/// a dark outline. 128×128.
const TRAY_ICON_LIGHT_PNG: &[u8] = include_bytes!("../packaging/IconTray-light.png");

/// How often the background thread re-checks device presence. Kept short so
/// the menu's device-status line reflects connect/disconnect events in near
/// realtime while the menu is open (ksni emits a DBusMenu `ItemsPropertiesUpdated`
/// signal on each flip, which SNI hosts refresh). Only re-serializes on an
/// actual transition to avoid needless D-Bus traffic (§6e).
const DEVICE_POLL_INTERVAL: Duration = Duration::from_secs(1);
/// Re-query the color-scheme portal every N device polls (~30 s). Theme flips
/// are rare and each check spawns `dbus-send`, so it's throttled relative to the
/// cheap HID enumeration.
const COLOR_SCHEME_POLL_EVERY: u32 = 10;

/// One-shot guard for the Disconnected→NoModule `notify-send`
/// (`spec/DEVICE_DISCOVERY.md` §3). `swap(true)` fires the notification the first
/// time a poll sees that transition; `store(false)` re-arms it when the device
/// leaves NoModule — so the notification fires at most once per entry into
/// NoModule, never on every 1s poll tick. Mirrors `RULES_INVALID_NOTIFIED`
/// (`src/core/notifier.rs`).
static NO_MODULE_NOTIFIED: AtomicBool = AtomicBool::new(false);

/// The tray item. Mutable state is refreshed by a background thread which then
/// tells ksni to re-serialize (icon + menu):
///   * `device_status` — the leading status line. Three states per
///     `spec/DEVICE_DISCOVERY.md` §3 (Connected / NoModule / Disconnected) —
///     parity with the macOS/Windows line-1 text (`src/tray.rs`). On Linux the
///     richer surface also dims the icon on Disconnected (full alpha for
///     Connected AND NoModule — a board is present either way) and fires a
///     one-shot `notify-send` on the Disconnected→NoModule transition (the
///     "flash qmk_notifier" moment), guarded by [`NO_MODULE_NOTIFIED`] so it
///     fires once per entry into NoModule and re-arms on exit.
///   * `dark_mode`        — which icon variant to serve (see module docs).
pub struct QmkTray {
    device_status: DeviceStatus,
    dark_mode: bool,
}

impl QmkTray {
    fn new() -> Self {
        Self {
            // Synchronous probes so the first paint is already correct.
            device_status: crate::core::notifier::device_status(),
            dark_mode: detect_dark_mode(),
        }
    }
}

impl ksni::Tray for QmkTray {
    fn id(&self) -> String {
        "QMKonnect".to_string()
    }

    fn title(&self) -> String {
        "QMKonnect".to_string()
    }

    fn tool_tip(&self) -> ToolTip {
        // Reflected live by SNI hosts on hover (NewToolTip) — unlike the open
        // menu, which Waybar renders as a static snapshot. So the tooltip is a
        // realtime connection indicator.
        let description = match self.device_status {
            DeviceStatus::Connected => "Window activity notifier — device connected",
            DeviceStatus::NoModule => {
                "Window activity notifier — QMK board found, no qmk_notifier module"
            }
            DeviceStatus::Disconnected => "Window activity notifier — NO DEVICE CONNECTED",
        };
        ToolTip {
            icon_name: String::new(),
            icon_pixmap: Vec::new(),
            title: "QMKonnect".to_string(),
            description: description.to_string(),
        }
    }

    fn category(&self) -> Category {
        Category::Hardware
    }

    // `icon_name` is left empty (its default) so the host renders our embedded
    // pixmap instead of resolving a themed name. This guarantees the official
    // QMKonnect icon (in the right theme variant) shows everywhere.

    /// Embedded official icon in the variant matching the detected color scheme.
    fn icon_pixmap(&self) -> Vec<Icon> {
        let png = if self.dark_mode {
            TRAY_ICON_DARK_PNG
        } else {
            TRAY_ICON_LIGHT_PNG
        };
        // Fade the icon when NO device is present so the disconnect is visible
        // in the bar in realtime. A board in NoModule is still *present* (just
        // not capable), so it keeps full alpha; only Disconnected dims. The menu
        // warning glyph (⚠) carries the "not capable" signal, not the icon.
        // SNI hosts repaint the icon on `NewIcon` (unlike the open menu, which
        // Waybar caches as a static snapshot).
        decode_icon(png)
            .map(|i| {
                vec![if self.device_status != DeviceStatus::Disconnected {
                    i
                } else {
                    dim_icon(i)
                }]
            })
            .unwrap_or_default()
    }

    fn menu(&self) -> Vec<MenuItem<QmkTray>> {
        // Line 1: device-connection status (disabled, non-clickable) — the Linux
        // equivalent of the macOS tray's "line 2" (macOS leads with an About
        // item). Three states per `spec/DEVICE_DISCOVERY.md` §3: solid dot =
        // ≥1 capable board; warning glyph = QMK board present, no qmk_notifier
        // module; hollow dot = no board. Backed by the platform-independent,
        // read-only `device_status()` resolver (`src/core/notifier.rs`).
        let status = device_status_text(self.device_status);

        let mut items = Vec::new();

        items.push(MenuItem::Standard(StandardItem {
            label: status.to_string(),
            enabled: false,
            activate: Box::new(|_| {}),
            ..Default::default()
        }));

        // Invisible structural toggle (rendered as nothing: `visible: false`).
        // Present only when a device is present (Connected OR NoModule — a board
        // is there either way), so the top-level item *count* differs between
        // the present and disconnected menus. That count change is what forces
        // ksni to emit DBusMenu `LayoutUpdated` (see `update_menu` in ksni's
        // service.rs) instead of only `ItemsPropertiesUpdated`. `LayoutUpdated`
        // is the signal every host honors to re-fetch and redraw an
        // *already-open* popup; `ItemsPropertiesUpdated` is an optional
        // optimization some hosts (e.g. Quickshell) ignore for open menus.
        // Both present↔disconnected transitions change the count, so both force
        // a live redraw — with no extra visible line.
        if self.device_status != DeviceStatus::Disconnected {
            items.push(MenuItem::Standard(StandardItem {
                label: String::new(),
                visible: false,
                enabled: false,
                activate: Box::new(|_| {}),
                ..Default::default()
            }));
        }

        items.push(MenuItem::Separator);
        // Settings dialog (zenity) — writes config.toml on save. Parity with
        // the macOS/Windows "Settings" entry.
        items.push(MenuItem::Standard(StandardItem {
            label: "Settings…".to_string(),
            activate: Box::new(|_| {
                show_settings_dialog_linux();
            }),
            ..Default::default()
        }));
        // "Edit rules" — open rules.toml in the system editor (HOST_RULES.md
        // §7). Seeds the commented template if absent, then xdg-opens it. Rule
        // changes apply automatically (re-parsed on the next window change), so
        // there is no reload step. ksni runs `activate` on its D-Bus thread and
        // forbids blocking, so the (fast) seed+open runs on a spawned thread.
        items.push(MenuItem::Standard(StandardItem {
            label: "Edit rules".to_string(),
            activate: Box::new(|_| {
                std::thread::spawn(crate::core::edit_rules);
            }),
            ..Default::default()
        }));
        items.push(MenuItem::Separator);
        // §7 opt 2: surface the active window's class/title via a desktop
        // notification (`notify-send`). Shells out instead of using the
        // notify-rust crate because notify-rust's blocking `show()` spawns a
        // nested tokio runtime, which panics inside ksni's handler thread.
        items.push(MenuItem::Standard(StandardItem {
            label: "Show Window Information".to_string(),
            activate: Box::new(|_| {
                show_window_info_linux();
            }),
            ..Default::default()
        }));
        items.push(MenuItem::Separator);
        items.push(MenuItem::Standard(StandardItem {
            label: "Quit".to_string(),
            activate: Box::new(|_| {
                std::process::exit(0);
            }),
            ..Default::default()
        }));

        items
    }
}

/// Build the SNI item, register it on the session D-Bus, and start a background
/// thread that keeps the device-status line and the icon theme-variant fresh.
///
/// The tray is fully optional and lazy: `assume_sni_available(true)` makes ksni
/// register-and-wait rather than hard-failing when no SNI host is running. So:
///   * No tray host at startup → the item waits silently and appears as soon as
///     one starts (e.g. Waybar launched later, or restarted).
///   * A machine with no tray host at all → the notifier runs headless forever
///     (the window monitor is independent of the tray); no icon, no crash.
///   * No session D-Bus at all → `spawn()` returns an error; we log it and run
///     trayless. The Hyprland monitor uses its own Unix socket, so it's fine.
///
/// Returns the [`ksni::blocking::Handle`] on success; the caller should hold it
/// for the process lifetime (dropping it unregisters the tray). On the rare
/// failure (no D-Bus) the error is logged and `None` is returned so the
/// notifier keeps running trayless instead of hard-failing (§9).
pub fn spawn(verbose: bool) -> Option<ksni::blocking::Handle<QmkTray>> {
    let handle = match QmkTray::new().assume_sni_available(true).spawn() {
        Ok(h) => h,
        Err(e) => {
            eprintln!(
                "Warning: could not register StatusNotifierItem tray (running trayless): {}",
                e
            );
            return None;
        }
    };

    // ksni caches icon/menu until told something changed. Poll the configured
    // device's presence (every tick) and the color scheme (throttled), and
    // update the tray only on a transition so we avoid needless D-Bus traffic.
    // `is_device_connected()` re-reads config.toml every call, so changes made
    // via the Settings dialog are reflected within one poll interval.
    let poll_handle = handle.clone();
    std::thread::spawn(move || {
        // Capable-board presence tracker (Finding #1): keys the handshake
        //   lifecycle on CAPABLE-board presence (not Tier-1 presence),
        //   re-probing only when the Tier-1 path set changes (a plug/unplug)
        //   so the hot loop never pings on a stable bus (Finding #3). A
        //   capable board unplugging while a non-capable Tier-1 board remains
        //   is now a real Loss (reset + re-arm); a different capable board
        //   replugging is a real Gain (re-handshake). See PresenceTracker.
        //   Mirrors src/tray.rs (parity).
        let mut presence = crate::core::notifier::PresenceTracker::new();
        // UI-status tracker: keyed on the three-state DeviceStatus (separate
        //   from the handshake lifecycle — the NoModule->Connected flip happens
        //   while capable presence is stable, driven by the handshake setting
        //   host_capable). Seed = Some(...) avoids a spurious first-tick event
        //   (new() already rendered the correct text/icon synchronously).
        let mut last_status: Option<DeviceStatus> = Some(crate::core::notifier::device_status());
        let mut last_dark: Option<bool> = None;
        let mut tick: u32 = 0;
        loop {
            // Handshake lifecycle on a capable-board transition. Runs on THIS
            // poll thread — NEVER inside poll_handle.update, whose closure
            // executes on ksni's D-Bus thread (HID I/O there would wedge the
            // tray icon).
            match presence.tick(verbose) {
                crate::core::notifier::HandshakeAction::Gain => {
                    crate::core::notifier::perform_handshake(verbose);
                }
                crate::core::notifier::HandshakeAction::Loss => {
                    crate::core::notifier::reset_handshake_state();
                }
                crate::core::notifier::HandshakeAction::None => {}
            }

            let status = crate::core::notifier::device_status();
            let dark = if tick.is_multiple_of(COLOR_SCHEME_POLL_EVERY) {
                detect_dark_mode()
            } else {
                last_dark.unwrap_or(true)
            };
            tick = tick.wrapping_add(1);

            // One-shot `notify-send` on the Disconnected->NoModule transition
            // ONLY (spec/DEVICE_DISCOVERY.md §3). Not on Connected->NoModule,
            // not on every tick. NO_MODULE_NOTIFIED.swap(true) fires once per
            // entry into NoModule; store(false) on leaving NoModule re-arms it.
            if last_status == Some(DeviceStatus::Disconnected)
                && status == DeviceStatus::NoModule
                && !NO_MODULE_NOTIFIED.swap(true, Ordering::SeqCst)
            {
                notify(
                    "QMK board found \u{2014} no qmk_notifier module",
                    "This QMK board isn't running the qmk_notifier firmware QMKonnect talks to. \
                     Flash it: docs/qmk-integration.md",
                );
            }
            // Re-arm when leaving NoModule so a later re-entry notifies again.
            if status != DeviceStatus::NoModule {
                NO_MODULE_NOTIFIED.store(false, Ordering::SeqCst);
            }

            // Tray UI on a status OR dark transition (keyed on `last_status`,
            // the three-state value — the NoModule->Connected flip is driven by
            // the handshake setting host_capable while capable presence is
            // stable, so a capable-keyed event alone would never fire for it).
            if last_status != Some(status) || last_dark != Some(dark) {
                last_status = Some(status);
                last_dark = Some(dark);
                let _ = poll_handle.update(|t: &mut QmkTray| {
                    t.device_status = status;
                    t.dark_mode = dark;
                });
            }
            std::thread::sleep(DEVICE_POLL_INTERVAL);
        }
    });

    eprintln!("StatusNotifierItem tray registered");
    Some(handle)
}

/// Show the foreground windows in a small, floating popup and copy the chosen
/// one's `class|title` to the clipboard — parity with the macOS/Windows
/// "Show Window Information…" dialog, adapted for Linux.
///
/// **Why `zenity --forms --add-list` (and not `--text-info`/`--list`):** zenity
/// builds only its *simple* dialog modes (`--forms`, `--entry`, `--info`, …) as
/// a real `GtkDialog`, which advertises the standard EWMH/Wayland "dialog"
/// surface type at creation — so **every** tiling compositor (Hyprland, Sway,
/// i3, river, …) floats it automatically, with zero compositor-specific code.
/// The content modes (`--text-info`, `--list`) are plain top-level windows that
/// get tiled, and on Wayland there's no client-side way to retype a window after
/// it's mapped (the X11 `xprop`/`wmctrl` trick has no Wayland equivalent). The
/// old `--text-info` + `hyprctl dispatch togglefloating` workaround was both
/// Hyprland-specific *and* silently broken on Hyprland 0.55, where
/// `hyprctl dispatch` switched to a Lua-expression syntax. Using a dialog mode
/// sidesteps all of that.
///
/// zenity has no per-row button widget, so the macOS/Windows per-row "Copy"
/// button maps to the GTK idiom for the same task: select a row → click **Copy**
/// (the relabelled OK button) → its `class|title` lands on the clipboard. (`yad`
/// can render literal per-row buttons via `--field="…":BTN`, but it's a separate
/// dependency most distros don't ship by default, so we stay on zenity.)
///
/// Falls back to a desktop notification if `zenity` is absent.
pub(crate) fn show_window_info_linux() {
    let rows = crate::platforms::list_foreground_windows();
    if rows.is_empty() {
        notify(
            "QMKonnect — Window Information",
            "No foreground windows detected.",
        );
        return;
    }

    // Native GTK popup: floating (dialog window-type hint) + a tall scrollable
    // list + a Copy button per row, no column/field headers. The zenity
    // `--forms` popup floats but caps the list at ~4 rows, and zenity `--list`
    // is tall but tiles — neither is acceptable, so we build a real window.
    // Runs on a dedicated thread (ksni's IPC thread stays responsive) and falls
    // back to the zenity popup below only when GTK can't init (headless).
    match gtk_dialog::sender() {
        Some(tx) => match tx.send(rows) {
            Ok(()) => (),
            Err(recovered) => show_window_info_linux_zenity(&recovered.0),
        },
        None => show_window_info_linux_zenity(&rows),
    }
}

/// zenity fallback for [`show_window_info_linux`] (used only when the native GTK
/// popup is unavailable). A floating `--forms` dialog whose embedded list is
/// height-capped at ~3–4 rows — a hard zenity limitation (see the ⚠️ note).
fn show_window_info_linux_zenity(rows: &[(String, String)]) {
    // Build the selectable list values. `--list-values` is `|`-separated, so a
    // literal `|` inside a class/title would split one entry into several;
    // replace it. Display form is `class  —  title` (class first so it scans
    // easily); the copied form is the config-style `class|title`.
    let display: Vec<String> = rows
        .iter()
        .map(|(class, title)| {
            let c = sanitize_list_value(class);
            if title.is_empty() {
                c
            } else {
                // U+2014 EM DASH — readable separator; class stays first.
                format!("{c}  \u{2014}  {}", sanitize_list_value(title))
            }
        })
        .collect();
    let list_values = display.join("|");

    const DIALOG_TITLE: &str = "QMKonnect Window Information";

    // `--forms` is a dialog → floats everywhere. No `--show-header` (a nameless
    // column header is pure cruft here) and an empty `--add-list=` field name
    // (the default "Window" label is redundant with the `--text` line). On OK
    // zenity (forms.c) prints the GtkTreeSelection — the selected row's value;
    // empty on Cancel or when Copy is pressed with nothing selected. `--ok-label`
    // relabels OK → Copy so the button describes what it does.
    //
    // ⚠️ Height: zenity gives the list's scrolled window a fixed 100 px minimum
    // and never sets `vexpand`, so `--height` only adds dead padding — the list
    // itself stays ~3–4 rows regardless. This is a hard zenity limitation; a
    // genuinely tall, scrollable, floating list needs a native dialog (see
    // LINUX_SUPPORT_ROADMAP.md / future GTK dialog).
    let output = Command::new("zenity")
        .args([
            "--forms",
            &format!("--title={DIALOG_TITLE}"),
            "--text=Select a window, then click Copy.",
            "--add-list=",
            &format!("--list-values={list_values}"),
            "--ok-label=Copy",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

    let chosen = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Ok(_) => return, // user cancelled / closed the dialog
        Err(e) => {
            eprintln!("Show Window Information: could not launch zenity: {e}");
            notify(
                "QMKonnect — Window Information",
                "Install zenity to view the full window list.",
            );
            return;
        }
    };

    if chosen.is_empty() {
        return; // OK pressed with no row selected
    }

    // Map the displayed string back to its row and copy `class|title` (parity
    // with the macOS/Windows Copy buttons). Falls back to the raw selection if
    // zenity returned something we didn't build (defensive — shouldn't happen).
    let to_copy = display
        .iter()
        .position(|d| d == &chosen)
        .and_then(|i| rows.get(i))
        .map(|(class, title)| {
            if title.is_empty() {
                class.clone()
            } else {
                format!("{class}|{title}")
            }
        })
        .unwrap_or_else(|| chosen.clone());

    if copy_to_clipboard(&to_copy) {
        notify("QMKonnect — Copied", &to_copy);
    } else {
        notify(
            "QMKonnect — clipboard unavailable",
            "Install wl-clipboard or xclip to enable copying.",
        );
    }
}

/// A row value for `zenity --forms --add-list`. `--list-values` is split on
/// `|`, so any literal `|` would fracture a row into several entries; swap it
/// for the visually-similar broken bar (U+00A6) so the text stays intact and
/// still reads naturally.
fn sanitize_list_value(s: &str) -> String {
    s.replace('|', "\u{00A6}")
}

/// Copy `text` to the Wayland/X11 clipboard, preferring `wl-copy`
/// (wl-clipboard, standard on Wayland) and falling back to `xclip`. Returns
/// whether a clipboard tool was available.
fn copy_to_clipboard(text: &str) -> bool {
    /// Feed `text` to a clipboard program's stdin. `args[0]` is the program.
    fn pipe(args: &[&str], text: &str) -> Option<()> {
        let mut child = Command::new(args[0])
            .args(&args[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
        Some(())
    }
    // wl-copy (wl-clipboard, Wayland standard) first, then xclip (X11 / XWayland).
    pipe(&["wl-copy"], text)
        .or_else(|| pipe(&["xclip", "-selection", "clipboard"], text))
        .is_some()
}

// ===========================================================================
// Native GTK "Show Window Information" popup.
//
// Why this exists instead of zenity: zenity's floating dialog (`--forms`) caps
// its embedded list at ~3–4 rows (zenity gives the scrolled window a 100 px
// minimum and never sets `vexpand`), and zenity's tall list (`--list`) is a
// plain top-level window that every tiling compositor tiles fullscreen. There
// is no single zenity invocation that is both floating AND tall. A native
// `GtkWindow` with a dialog type-hint floats on every tiling compositor, and a
// `ScrolledWindow`/`ListBox` with `vexpand` grows to show all rows — so we get a
// floating, tall, scrollable list with a per-row Copy button (macOS/Windows
// parity), no column header, no field label.
//
// GTK is single-threaded and `gtk::init()` may run only once per process, so a
// single owner thread (started lazily on first use) runs the GTK main loop for
// the process lifetime. Requests arrive over a channel and are polled from the
// main loop; each opens an independent window. `gtk` is already compiled into
// the binary (via libappindicator/tray-icon), so this dependency is free.
// ===========================================================================
mod gtk_dialog {
    use std::sync::mpsc::{self, Sender};
    use std::sync::OnceLock;
    use std::time::Duration;

    use gtk::glib;
    use gtk::prelude::*;
    use gtk::{
        Adjustment, Box as GtkBox, Button, Clipboard, Label, ListBox, Orientation, PolicyType,
        ScrolledWindow, SelectionMode, Window, WindowPosition, WindowType,
    };

    /// The rows to display: `(class, title)`.
    type Rows = Vec<(String, String)>;

    /// Channel to the single GTK owner thread; `None` once GTK is known to be
    /// unavailable (headless / no display).
    static SENDER: OnceLock<Option<Sender<Rows>>> = OnceLock::new();

    /// Lazily start the GTK owner thread and return a sender for dialog
    /// requests, or `None` if GTK can't initialize.
    pub(crate) fn sender() -> Option<&'static Sender<Rows>> {
        SENDER
            .get_or_init(|| {
                let (tx, rx) = mpsc::channel::<Rows>();
                let (init_tx, init_rx) = mpsc::channel::<bool>();
                std::thread::Builder::new()
                    .name("qmkonnect-gtk".into())
                    .spawn(move || {
                        if gtk::init().is_err() {
                            let _ = init_tx.send(false);
                            return;
                        }
                        let _ = init_tx.send(true);
                        // Poll the request channel from the GTK main loop; each
                        // ready message opens an independent window.
                        glib::timeout_add_local(Duration::from_millis(50), move || {
                            while let Ok(rows) = rx.try_recv() {
                                show_window(&rows);
                            }
                            true.into()
                        });
                        gtk::main();
                    })
                    .ok()?;
                // First-use handshake: block briefly until init resolves.
                (init_rx.recv_timeout(Duration::from_secs(5)) == Ok(true)).then_some(tx)
            })
            .as_ref()
    }

    /// Build and show one floating, tall, headerless window-info dialog. Runs on
    /// the GTK owner thread.
    fn show_window(rows: &[(String, String)]) {
        let win = Window::new(WindowType::Toplevel);
        win.set_title("QMKonnect — Window Information");
        // Dialog type-hint → every tiling compositor floats this (the whole
        // point: zenity's tall `--list` is a normal window and gets tiled).
        win.set_type_hint(gtk::gdk::WindowTypeHint::Dialog);
        win.set_position(WindowPosition::Center);
        win.set_default_size(640, 760);
        // A fixed size (min == max) is the signal Hyprland and other tiling
        // compositors use to float a native Wayland window as a dialog — a
        // resizable toplevel gets tiled. (On Wayland the X11 `Dialog` type-hint
        // above is largely ignored; this is what actually floats it.) Fixed size
        // + a scrolled list = a tall, floating popup that scrolls every row.
        win.set_resizable(false);

        let vbox = GtkBox::new(Orientation::Vertical, 8);
        vbox.set_border_width(10);

        let help = Label::new(Some("Click a row's Copy button to copy class|title."));
        help.set_xalign(0.0);
        vbox.pack_start(&help, false, false, 0);

        let scroll = ScrolledWindow::new(None::<&Adjustment>, None::<&Adjustment>);
        // vexpand → the list grows to fill the dialog instead of being capped.
        scroll.set_vexpand(true);
        scroll.set_min_content_height(420);
        // No horizontal scrollbar: clamp rows to the viewport width so the
        // per-row Copy button (packed at the right) stays visible and the window
        // label ellipsizes instead of pushing the button off-screen.
        scroll.set_policy(PolicyType::Never, PolicyType::Automatic);
        let list = ListBox::new();
        list.set_selection_mode(SelectionMode::None);

        for (class, title) in rows {
            let row = GtkBox::new(Orientation::Horizontal, 8);
            row.set_hexpand(true);
            let text = if title.is_empty() {
                class.clone()
            } else {
                format!("{class}  \u{2014}  {title}")
            };
            let lbl = Label::new(Some(&text));
            lbl.set_xalign(0.0);
            // Take all remaining width and truncate the window text with an
            // ellipsis so the Copy button to its right is never pushed off.
            lbl.set_hexpand(true);
            lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
            row.pack_start(&lbl, true, true, 0);

            let copy_text = if title.is_empty() {
                class.clone()
            } else {
                format!("{class}|{title}")
            };
            let btn = Button::with_label("Copy");
            btn.connect_clicked(move |_| {
                let clipboard = Clipboard::get(&gtk::gdk::Atom::intern("CLIPBOARD"));
                clipboard.set_text(&copy_text);
            });
            row.pack_start(&btn, false, false, 0);
            list.insert(&row, -1);
        }
        scroll.add(&list);
        vbox.pack_start(&scroll, true, true, 0);

        let close = Button::with_label("Close");
        let win_for_close = win.clone();
        close.connect_clicked(move |_| {
            win_for_close.close();
        });
        vbox.pack_start(&close, false, false, 0);

        // Escape closes the window. This is a keyboard-power-user app, so every
        // popup must be dismissible by key — and the sibling Settings popup
        // (zenity) already closes on Escape natively. The native GTK window had
        // no key handler, so on Hyprland/Wayland only the mouse Close button
        // worked. Key events propagate from the focused child up to this
        // toplevel in GTK3, so this fires regardless of which row/button is
        // focused. (spec/UI.md §3.5 — keyboard dismissal requirement.)
        let win_for_escape = win.clone();
        win.connect_key_press_event(move |_, event| {
            if event.keyval() == gtk::gdk::keys::constants::Escape {
                win_for_escape.close();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });

        win.add(&vbox);
        win.show_all();
    }
}

/// One picker row's three `zenity --list` columns: the live `product_name` (or
/// `(unnamed)` when the HID descriptor carries none), the `0xVID:0xPID` in
/// uppercase (for parity with the spec example), and the capability glyph +
/// status. Built from a [`ClassifiedDevice`] per `spec/DEVICE_DISCOVERY.md`
/// §5.1 / §3 (the ✓/✗ + "qmk_notifier" / "QMK board, no module" semantics).
/// Pure; unit-tested.
fn picker_columns(d: &crate::core::notifier::ClassifiedDevice) -> (String, String, String) {
    use crate::core::notifier::DeviceKind;
    let (glyph, status) = match d.kind {
        DeviceKind::Capable { .. } => ("\u{2713}", "qmk_notifier"), // ✓
        DeviceKind::NotQmkNotifier => ("\u{2717}", "QMK board, no module"), // ✗
    };
    let name = d.product_name.as_deref().unwrap_or("(unnamed)").to_string();
    let vidpid = format!("0x{:04X}:0x{:04X}", d.vendor_id, d.product_id);
    let cap = format!("{glyph} {status}");
    (name, vidpid, cap)
}

/// Parse the `zenity --list --print-column=2` stdout (`0xFEED:0x0000`) back to a
/// concrete `(u16, u16)`. Returns `None` on any malformed input (no colon,
/// non-hex, or a missing half). Reuses [`parse_id`] for each half — since an
/// empty/`auto` half yields `None` there, a half-missing selection (e.g.
/// `"feed:"`) also resolves to `None` here. Pure; unit-tested.
fn parse_vidpid(s: &str) -> Option<(u16, u16)> {
    let mut it = s.trim().splitn(2, ':');
    let vid = parse_id(it.next()?).ok()??; // ?? : Result→Option, Option<u16>→u16
    let pid = parse_id(it.next()?).ok()??;
    Some((vid, pid))
}

/// Persist VID/PID, apply the udev device rule (pkexec), and notify the user.
/// Extracted verbatim from the former inline tail of [`show_settings_dialog_linux`]
/// so the picker path and the manual `--forms` path share identical save
/// behavior (including the [`ApplyOutcome`] notify detail). The
/// [`apply_device_rule`]/pkexec flow is unchanged (both `None` ⇒ no rule; at
/// least one `Some` ⇒ install the VID/PID fallback rule privileged).
fn save_and_notify(vendor_id: Option<u16>, product_id: Option<u16>) {
    let vid_str = vendor_id
        .map(|v| format!("0x{v:04x}"))
        .unwrap_or_else(|| "auto".to_string());
    let pid_str = product_id
        .map(|p| format!("0x{p:04x}"))
        .unwrap_or_else(|| "auto".to_string());

    // Snapshot the PRE-save VID/PID before write_config overwrites config.toml.
    // (None, None) on a fresh install (auto-discovery) is fine — it will differ
    // from the new value and correctly trigger the reset on first config.
    let (old_vid, old_pid) = current_config_vidpid();

    match write_config(vendor_id, product_id) {
        Ok(path) => {
            // Bug 4 (PRD ID 3): if the VID/PID filter actually changed, reset the
            // handshake state and re-run the handshake for the newly-selected board
            // so CALLBACK_NAMES reflects it. reset clears HAS_HANDSHAKED (so the
            // idempotent perform_handshake actually re-runs) and the stale name→id
            // map; perform_handshake reads config.toml fresh and rebuilds the map.
            // `false` = non-verbose (verbose is not in scope in save_and_notify).
            if (vendor_id, product_id) != (old_vid, old_pid) {
                crate::core::notifier::reset_handshake_state();
                crate::core::notifier::perform_handshake(false);
            }

            let outcome = apply_device_rule(vendor_id, product_id);
            let detail = match outcome {
                ApplyOutcome::AutoDiscovery => {
                    "Auto-discovery in effect (any standard QMK keyboard).".to_string()
                }
                ApplyOutcome::Applied => "Device rule applied.".to_string(),
                ApplyOutcome::NeedsManual(how) => how,
            };
            notify(
                "QMKonnect — settings saved",
                &format!(
                    "vendor_id = {vid_str}, product_id = {pid_str}\n{detail}\n{}",
                    path.display()
                ),
            );
        }
        Err(e) => {
            eprintln!("Settings: failed to write config: {}", e);
            notify("QMKonnect — could not save", &e.to_string());
        }
    }
}

/// Run the discovered-device picker as a `zenity --list` dialog (three columns:
/// Device / VID:PID / Capability) and return the selected board's
/// `(vid, pid)` parsed from the printed column-2 cell.
///
/// Returns `None` on cancel/close (zenity exit 1) OR on an OK with no selection
/// (exit 0 + empty stdout) — both fall through to the `--forms` Advanced path in
/// [`show_settings_dialog_linux`]. No notification is fired here: a missing
/// zenity would make both dialogs fail, and the `--forms` (which follows) has its
/// own zenity-missing notify that covers the case.
fn run_device_picker(devices: &[crate::core::notifier::ClassifiedDevice]) -> Option<(u16, u16)> {
    // Build argv: flags first, then the 3 column headers, then N×3 values.
    // Each value is pushed as its own arg element (Rust's Command does NOT go
    // through a shell, so the ✓ glyph + spaces are fine — no quoting).
    let mut args: Vec<String> = vec![
        "--list".into(),
        "--title=QMK Settings".into(),
        "--print-column=2".into(), // print only the VID:PID cell of the chosen row
        "--hide-header".into(),    // 1-3 rows ⇒ headers add noise
        "--width=520".into(),
        "--text=Select a detected keyboard (or Cancel for manual entry):".into(),
        "--column=Device".into(),
        "--column=VID:PID".into(),
        "--column=Capability".into(),
    ];
    for d in devices {
        let (name, vidpid, cap) = picker_columns(d);
        args.push(name);
        args.push(vidpid);
        args.push(cap);
    }
    let output = Command::new("zenity")
        .args(args.iter().map(String::as_str))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let out = match output {
        Ok(o) if o.status.success() => o, // G4: success gate
        _ => return None,                 // cancel/close/non-zero ⇒ no pick
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    if stdout.trim().is_empty() {
        return None; // G4: OK-with-no-selection ⇒ fall through to --forms
    }
    parse_vidpid(&stdout) // None if malformed (defensive)
}

/// Settings dialog: a discovered-device picker (`zenity --list`) shown *before*
/// the existing VID/PID `zenity --forms` (the Advanced / manual override), per
/// `spec/DEVICE_DISCOVERY.md` §5 (the Discovered-Device Picker) and the Linux
/// `--forms` contract in `spec/UI.md` §2.3 / §2.4.
///
/// The two dialogs run **sequentially** (separate `Command::output()` calls):
/// the picker first; if a row is selected, its board's `(vid, pid)` is written
/// via [`save_and_notify`] and the `--forms` is **skipped** (chosen-first
/// precedence). If the user cancels the picker or selects nothing, the `--forms`
/// opens as the Advanced / manual override. The three cases (`spec/DEVICE_DISCOVERY.md`
/// §5.1):
///
///   * **empty** (0 devices): picker skipped; `--forms` text
///     "No QMK keyboards detected. Enter IDs manually below."
///   * **clean-auto** (1 capable board + no VID/PID configured): picker skipped
///     to preserve the zero-config promise — auto-discovery already targets the
///     single board, so nothing is written. `--forms` text "Detected: \u{3c}name>.
///     Auto-discovery is active."
///   * **picker** (≥2 boards, or 1 non-capable board, or 1 capable board with a
///     VID/PID already set): the `zenity --list` is shown.
///
/// **Mode-A deviations, documented (`spec/DEVICE_DISCOVERY.md` §5.3):**
///   * **No `[Rescan]` button.** Unlike the Windows message loop or the macOS
///     `runModal`, the two zenity dialogs are sequential synchronous blocks —
///     there is no "open dialog" window to click a button within.
///     `classify_devices(true)` is called once per invocation; re-opening
///     Settings refreshes (after the 5 s cache TTL, the probe re-runs).
///   * **The `--list` tiles on pure tiling WMs** (Sway/i3/hyprland): `--list` is
///     a normal toplevel, unlike `--forms` which floats. This is an accepted
///     tradeoff — the device count is tiny (1–3 keyboards), so a short tiled
///     list is still usable, and `--list` provides the exact single-select →
///     print-selection semantics needed. The window-info dialog
///     ([`show_window_info_linux`]) avoids tiling via a heavyweight native GTK
///     popup, but that plumbing is unjustified for a 3-row device list.
///
/// The monitor + device-status poll re-read config on every notification / poll,
/// so a save takes effect within ~3 s with no restart needed.
fn show_settings_dialog_linux() {
    use crate::core::notifier::{classify_devices, DeviceKind};

    // Classify once per open (reads the warm 5 s-TTL cache ⇒ ~free; re-open after
    // TTL = fresh probe). G5/D5: do NOT clear the cache on open (parity with the
    // macOS/Windows siblings).
    let devices = classify_devices(true);
    let (cur_vid, cur_pid) = current_config_vidpid();

    // CASE B (clean-auto): exactly one capable board AND no VID/PID configured.
    // The picker is skipped — auto-discovery already targets the single board, so
    // there is nothing to choose and nothing to write (zero-config promise, §5.1).
    let clean_auto = devices.len() == 1
        && matches!(devices[0].kind, DeviceKind::Capable { .. })
        && cur_vid.is_none()
        && cur_pid.is_none();
    // The picker is shown only when devices were found AND it's not clean-auto.
    let picker = !devices.is_empty() && !clean_auto;

    // CASE C: run the picker. A real selection short-circuits straight to the
    // save path and skips the --forms (chosen-first precedence, D4).
    if picker {
        if let Some((v, p)) = run_device_picker(&devices) {
            save_and_notify(Some(v), Some(p));
            return; // SKIP the --forms (the disambiguation is done)
        }
        // else: cancel / no-selection ⇒ fall through to the --forms (Advanced).
    }

    // The --forms (Advanced / manual override), reached by empty / clean-auto /
    // picker-fallthrough. The --text reflects the case (G14) so the user
    // understands what they're seeing; the current values are shown in all three.
    let (cur_vid_h, cur_pid_h) = current_config_hex();
    let prefix = if devices.is_empty() {
        "No QMK keyboards detected. Enter IDs manually below.".to_string()
    } else if clean_auto {
        format!(
            "Detected: {}. Auto-discovery is active.",
            devices[0].product_name.as_deref().unwrap_or("(unnamed)")
        )
    } else {
        "Advanced / manual override — enter hex VID/PID.".to_string()
    };
    let text = format!(
        "{prefix}\n\
         Current: vendor_id = 0x{cur_vid_h}   product_id = 0x{cur_pid_h}\n\
         Enter hex values (the 0x prefix is optional; blank = auto-discovery):"
    );

    let output = Command::new("zenity")
        .args([
            "--forms",
            "--title=QMK Settings",
            &format!("--text={text}"),
            "--add-entry=Vendor ID (hex)",
            "--add-entry=Product ID (hex)",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

    let out = match output {
        Ok(o) if o.status.success() => o,
        Ok(_) => return, // user cancelled or closed the dialog
        Err(e) => {
            eprintln!("Settings: could not launch zenity: {}", e);
            notify(
                "QMKonnect — Settings unavailable",
                "Install zenity, or edit config.toml directly.",
            );
            return;
        }
    };

    // zenity --forms prints field values separated by '|' on stdout.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut parts = stdout.trim_end_matches('\n').split('|');
    let vid_in = parts.next().unwrap_or_default().trim();
    let pid_in = parts.next().unwrap_or_default().trim();

    let (vid, pid) = match (parse_id(vid_in), parse_id(pid_in)) {
        (Ok(v), Ok(p)) => (v, p),
        _ => {
            notify(
                "QMKonnect — invalid input",
                "Vendor/Product ID must be hex (e.g. feed / 0000), or blank for auto-discovery.",
            );
            return;
        }
    };

    // G10: identical behavior to the pre-refactor inline tail (now extracted).
    save_and_notify(vid, pid);
}

/// Outcome of applying the device rule from the Settings dialog.
enum ApplyOutcome {
    /// Both IDs unset: no rule needed (static usage-page rule covers it).
    AutoDiscovery,
    /// A VID/PID fallback rule was installed and udev reloaded.
    Applied,
    /// Could not install privileged (pkexec absent/cancelled/failed). Carries a
    /// short manual instruction.
    NeedsManual(String),
}

/// Apply the device-permission rule for the just-saved VID/PID.
///
/// * Both `None` → no rule is needed; the static usage-page rule already grants
///   permissions to any 0xFF60/0x61 device, so we just best-effort reload udev.
/// * At least one `Some` → render the on-demand fallback rule, stage it in the
///   process temp dir (not a predictable `/tmp` name), and install it via
///   `pkexec` (install + udevadm reload/trigger). If pkexec is unavailable or
///   the user cancels, surface a manual `sudo qmkonnect -r` instruction.
fn apply_device_rule(vendor_id: Option<u16>, product_id: Option<u16>) -> ApplyOutcome {
    let rule = match crate::platforms::render_vidpid_rule(vendor_id, product_id) {
        Some(r) => r,
        None => {
            // Static usage-page rule covers default keyboards; just refresh.
            let _ = std::process::Command::new("udevadm")
                .args(["control", "--reload-rules"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = std::process::Command::new("udevadm")
                .args(["trigger", "--subsystem-match=hidraw"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            return ApplyOutcome::AutoDiscovery;
        }
    };

    // Stage the rule under the process temp dir, then install privileged.
    let tmp = std::env::temp_dir().join("qmkonnect-rule.tmp");
    if std::fs::write(&tmp, &rule).is_err() {
        return ApplyOutcome::NeedsManual("Settings saved. Run: sudo qmkonnect -r".to_string());
    }
    let install_cmd = format!(
        "install -m 644 {tmp} /etc/udev/rules.d/99-qmkonnect.rules && \
         udevadm control --reload-rules && \
         udevadm trigger --subsystem-match=hidraw && rm -f {tmp}",
        tmp = tmp.display()
    );
    match std::process::Command::new("pkexec")
        .args(["sh", "-c", &install_cmd])
        .status()
    {
        Ok(s) if s.success() => ApplyOutcome::Applied,
        // pkexec missing / cancelled / failed — reload is root-aware now (#26),
        // so a plain `sudo qmkonnect -r` will find the config and write the rule.
        _ => ApplyOutcome::NeedsManual("Settings saved. Run: sudo qmkonnect -r".to_string()),
    }
}

/// Read the currently-configured VID/PID as lowercased 4-digit hex strings
/// (without `0x`), or `"auto"` when unset (auto-discovery). Derives from
/// [`current_config_vidpid`] so there is a single config-read.
fn current_config_hex() -> (String, String) {
    let (v, p) = current_config_vidpid();
    let fmt = |id: Option<u16>| match id {
        Some(x) => format!("{x:04x}"),
        None => "auto".to_string(),
    };
    (fmt(v), fmt(p))
}

/// The currently-configured VID/PID as raw `Option<u16>` values (the clean-auto
/// check in [`show_settings_dialog_linux`] needs the real Options, not the
/// display strings from [`current_config_hex`]). Reads the first existing config
/// candidate via `crate::platforms::get_config_paths()` — the established
/// pattern shared with [`write_config`]. Returns `(None, None)` when no config
/// exists yet (fresh install) ⇒ auto-discovery.
fn current_config_vidpid() -> (Option<u16>, Option<u16>) {
    crate::platforms::get_config_paths()
        .into_iter()
        .find(|p| p.exists())
        .and_then(|p| crate::core::parse_config(&p).ok())
        .map(|cfg| (cfg.vendor_id, cfg.product_id))
        .unwrap_or((None, None))
}

/// Persist VID/PID to the config file, preserving every other field. `None`
/// means "auto-discovery" (the field is written commented out). Starts from the
/// current parsed config (wherever it lives among the candidate paths) and
/// overlays only the dialog's VID/PID, so the user's usage_page/usage/
/// debounce_ms/poll_interval_ms survive a VID/PID edit (previously they were
/// silently reset to defaults). Shares the renderer with `qmkonnect -c` and the
/// macOS/Windows dialogs so every write path agrees on the format.
fn write_config(
    vendor_id: Option<u16>,
    product_id: Option<u16>,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let dir = crate::platforms::create_config_dir()?;
    let path = dir.join("config.toml");
    // Preserve existing non-VID/PID fields: start from the current config
    // (first existing candidate, mirroring current_config_hex's search) and
    // overlay only the dialog's VID/PID.
    let mut config = crate::platforms::get_config_paths()
        .into_iter()
        .find(|p| p.exists())
        .and_then(|p| crate::core::parse_config(&p).ok())
        .unwrap_or_default();
    config.vendor_id = vendor_id;
    config.product_id = product_id;
    let content = crate::core::render_config_body(&config);
    crate::core::atomic_write(&path, &content)?;
    Ok(path)
}

/// Parse a hex VID/PID, tolerating an optional `0x` prefix and surrounding
/// whitespace. Empty input (or the literal "auto") yields `None`
/// (auto-discovery); otherwise `Some(value)`. Garbage is an error.
fn parse_id(input: &str) -> Result<Option<u16>, Box<dyn std::error::Error>> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto") {
        return Ok(None);
    }
    let s = trimmed.trim_start_matches("0x").to_lowercase();
    if s.is_empty() {
        return Ok(None);
    }
    u16::from_str_radix(&s, 16)
        .map(Some)
        .map_err(|e| format!("invalid hex '{input}': {e}").into())
}

/// Fire a desktop notification via `notify-send` if present. Best-effort:
/// failures (no daemon / binary) are logged, not fatal.
fn notify(summary: &str, body: &str) {
    match Command::new("notify-send")
        .args([
            "--app-name=QMKonnect",
            "--icon=input-keyboard",
            summary,
            body,
        ])
        .status()
    {
        Ok(s) if !s.success() => eprintln!("notify-send exited non-zero: {s}"),
        Err(e) => eprintln!("could not run notify-send: {e}"),
        _ => {}
    }
}

/// Whether the desktop prefers a dark color scheme.
///
/// Queries the freedesktop `org.freedesktop.portal.Settings` portal — the same
/// source GTK/libadwaita and KDE use — for `org.freedesktop.appearance.color-scheme`
/// (`1` = dark, `2` = light, `0` = no preference). On any failure (no portal,
/// `dbus-send` missing, parse error) we default to **dark**, because its
/// light-outlined icon variant remains legible across the widest range of bar
/// backgrounds.
///
/// Shells out to `dbus-send` (ubiquitous on Linux desktops) to avoid coupling to
/// a specific zbus version's variant-deserialization shape.
fn detect_dark_mode() -> bool {
    let out = match Command::new("dbus-send")
        .args([
            "--session",
            "--print-reply",
            "--reply-timeout=2000",
            "--dest=org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings.Read",
            "string:org.freedesktop.appearance",
            "string:color-scheme",
        ])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return true, // default dark
    };
    parse_color_scheme(&String::from_utf8_lossy(&out.stdout))
}

/// Parse the `uint32` value out of a `dbus-send` reply body for the color-scheme
/// read. Returns `true` for dark. Separated from [`detect_dark_mode`] so it can
/// be unit-tested.
fn parse_color_scheme(reply: &str) -> bool {
    // Reply body line looks like: `   variant       variant          uint32 1`
    let scheme = reply.lines().find_map(|l| {
        l.split("uint32")
            .nth(1)
            .and_then(|t| t.trim().parse::<u32>().ok())
    });
    match scheme {
        Some(2) => false, // explicit light
        _ => true,        // 1 = dark, 0 = no preference (default dark), or unparseable
    }
}

/// Decode an embedded PNG icon. Both variants are tiny so this is cheap.
fn decode_icon(png: &[u8]) -> Option<Icon> {
    let img = image::load_from_memory(png).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    Some(Icon {
        width: w as i32,
        height: h as i32,
        data: img.into_raw(),
    })
}

/// Label for the Linux SNI device-status menu item (line 1). Three states per
/// `spec/UI.md` §4 / `spec/DEVICE_DISCOVERY.md` §3 — byte-identical to
/// `src/tray.rs::device_status_text` (parity; the test
/// `status_text_uses_parity_glyphs` pins it).
fn device_status_text(status: DeviceStatus) -> String {
    match status {
        // U+25CF BLACK CIRCLE — ≥1 capable board.
        DeviceStatus::Connected => "\u{25CF}  Device Connected".to_string(),
        // U+26A0 WARNING SIGN — QMK board present, no qmk_notifier module.
        DeviceStatus::NoModule => {
            "\u{26A0}  QMK board found \u{2014} no qmk_notifier module (flash it)".to_string()
        }
        // U+25CB WHITE CIRCLE — 0 Tier-1 boards.
        DeviceStatus::Disconnected => "\u{25CB}  No Device Connected".to_string(),
    }
}

/// Fade an icon to ~35% opacity for the disconnected state. SNI pixmaps carry
/// a separate alpha channel, so scaling alpha alone reads as "inactive" without
/// changing the hue.
fn dim_icon(icon: Icon) -> Icon {
    const DIM_ALPHA: u8 = 90; // ~35% of 255
    let mut data = icon.data;
    for chunk in data.chunks_exact_mut(4) {
        chunk[3] = chunk[3].saturating_mul(DIM_ALPHA) / 255;
    }
    Icon {
        width: icon.width,
        height: icon.height,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_text_uses_parity_glyphs() {
        // Three states per spec/DEVICE_DISCOVERY.md §3 / spec/UI.md §4.
        // Byte-identical to src/tray.rs::device_status_text (the parity
        // contract between the macOS/Windows and Linux SNI trays).
        use crate::core::notifier::DeviceStatus;
        assert!(device_status_text(DeviceStatus::Connected).starts_with('\u{25CF}'));
        assert!(device_status_text(DeviceStatus::NoModule).starts_with('\u{26A0}'));
        assert!(device_status_text(DeviceStatus::Disconnected).starts_with('\u{25CB}'));
        // Full strings (pins the em-dash + wording byte-for-byte):
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

    #[test]
    fn new_tray_probes_initial_state() {
        // Initial state mirrors live (read-only) probes; both outcomes valid.
        let tray = QmkTray::new();
        let _ = (tray.device_status, tray.dark_mode);
    }

    #[test]
    fn parse_id_handles_prefix_case_and_auto() {
        // Explicit hex values (with/without 0x prefix, any case) -> Some.
        assert_eq!(parse_id("feed").unwrap(), Some(0xfeed));
        assert_eq!(parse_id("0xFEED").unwrap(), Some(0xfeed));
        assert_eq!(parse_id("  0x0000 ").unwrap(), Some(0));
        // Empty / "auto" -> None (auto-discovery).
        assert_eq!(parse_id("").unwrap(), None);
        assert_eq!(parse_id("auto").unwrap(), None);
        assert_eq!(parse_id("AUTO").unwrap(), None);
        // Garbage -> error.
        assert!(parse_id("nothex").is_err());
    }

    #[test]
    fn color_scheme_parser_matches_spec() {
        // 1 = dark, 2 = light, 0 = no preference (default dark).
        let mk = |n: u32| {
            format!("method return time=1 sender=:1.1 -> destination=:1.2\n   variant       variant          uint32 {n}\n")
        };
        assert!(parse_color_scheme(&mk(1))); // dark
        assert!(!parse_color_scheme(&mk(2))); // light
        assert!(parse_color_scheme(&mk(0))); // no preference -> dark
        assert!(parse_color_scheme("garbage no uint here")); // unparseable -> dark
    }

    #[test]
    fn embedded_icons_decode() {
        // Both theme variants must decode to a real 128×128 RGBA pixmap.
        for (name, png) in [("dark", TRAY_ICON_DARK_PNG), ("light", TRAY_ICON_LIGHT_PNG)] {
            let icon = decode_icon(png).unwrap_or_else(|| panic!("{name} icon should decode"));
            assert_eq!(icon.width, 128);
            assert_eq!(icon.height, 128);
            assert_eq!(
                icon.data.len(),
                (icon.width as usize) * (icon.height as usize) * 4
            );
        }
    }

    #[test]
    fn test_picker_columns() {
        // spec/DEVICE_DISCOVERY.md §5.1 / §3: ✓ qmk_notifier vs ✗ "QMK board,
        // no module"; name or "(unnamed)"; VID:PID in uppercase.
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
        let (n, vp, c) = picker_columns(&capable);
        assert_eq!(n, "Dactyl");
        assert_eq!(vp, "0xFEED:0x0000");
        assert!(
            c.starts_with('\u{2713}') && c.contains("qmk_notifier"),
            "cap: {c}"
        );

        // NotQmkNotifier variant (different VID/PID + name).
        let notqmk = ClassifiedDevice {
            kind: DeviceKind::NotQmkNotifier,
            vendor_id: 0x3434,
            product_id: 0x0123,
            product_name: Some("Keychron".into()),
            ..capable.clone()
        };
        let (n2, vp2, c2) = picker_columns(&notqmk);
        assert_eq!(n2, "Keychron");
        assert_eq!(vp2, "0x3434:0x0123");
        assert!(
            c2.starts_with('\u{2717}') && c2.contains("QMK board, no module"),
            "cap: {c2}"
        );

        // Unnamed board (product_name is None).
        let unnamed = ClassifiedDevice {
            product_name: None,
            kind: DeviceKind::NotQmkNotifier,
            vendor_id: 0x3434,
            product_id: 0x0123,
            ..capable.clone()
        };
        let (n3, vp3, _c3) = picker_columns(&unnamed);
        assert_eq!(n3, "(unnamed)");
        assert_eq!(vp3, "0x3434:0x0123");
    }

    #[test]
    fn test_parse_vidpid() {
        // Valid selections parse back to concrete (vid, pid).
        assert_eq!(parse_vidpid("0xFEED:0x0000"), Some((0xFEED, 0x0000)));
        assert_eq!(parse_vidpid("feed:0x123"), Some((0xFEED, 0x0123)));
        assert_eq!(parse_vidpid("  0xFEED:0x0000  "), Some((0xFEED, 0x0000))); // trimmed
                                                                               // Malformed / empty / half-missing selections → None (fall through).
        assert_eq!(parse_vidpid(""), None);
        assert_eq!(parse_vidpid("feed"), None); // no colon
        assert_eq!(parse_vidpid("feed:"), None); // missing pid
        assert_eq!(parse_vidpid(":123"), None); // missing vid
        assert_eq!(parse_vidpid("garbage:x"), None); // non-hex vid
                                                     // splitn(2): the pid half carries a stray '|' → parse_id rejects it.
        assert_eq!(parse_vidpid("0xFEED:0x0000|extra"), None);
    }
}

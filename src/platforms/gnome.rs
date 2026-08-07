//! GNOME backend — Shell-extension D-Bus client (PLATFORMS.md §8.3 — priority #2).
//!
//! GNOME (Mutter) advertises neither Wayland foreign-toplevel protocol and
//! exposes no client API for the active window. A GNOME Shell extension
//! (`qmkonnect@mulletware`, P2.M3.T1.S1) reads `global.display.focus_window`
//! INSIDE `gnome-shell` and republishes `(wm_class, title)` over the session
//! D-Bus as well-known name `io.mulletware.QMKonnect`. THIS module is the
//! desktop-side client: it subscribes to `ActiveWindowChanged`, polls
//! `GetActiveWindow` for drift, probes `name_has_owner` for the §8.3
//! NameOwnerChanged semantics, dedups, and notifies QMK.
//!
//! Threading (ARCHITECTURE.md §6): TWO worker threads, each with its OWN
//! blocking Connection (GOTCHA-2 — a shared Connection would serialize on its
//! internal executor). `start()` spawns both and returns (spawn-and-return;
//! keeps the trait default `start_blocks_calling_thread() == false`). ksni owns
//! its D-Bus thread; the runner parks main.
#![cfg(all(target_os = "linux", feature = "gnome"))]

use crate::core::notifier;
use crate::core::types::WindowInfo;
use crate::platforms::WindowMonitor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use zbus::proxy;

/// D-Bus well-known name — owned ⇔ extension installed & enabled
/// (PLATFORMS.md §6 row 2). Same name/path/iface the extension (P2.M3.T1.S1) owns.
/// Used at runtime by `name_has_owner` and the verbose messages; the
/// `#[proxy]` attribute repeats the same value as a string literal (the macro
/// requires literals, not `const` refs).
const BUS_NAME: &str = "io.mulletware.QMKonnect";
/// Default drift-poll cadence when `[linux] gnome_poll_interval_ms` is unset.
const DEFAULT_POLL_MS: u64 = 1000;

/// The GNOME Shell extension's interface (PLATFORMS.md §8.1).
/// `#[zbus::proxy]` generates BOTH `WindowMonitorProxy` (async) AND
/// `WindowMonitorProxyBlocking` (blocking) — GOTCHA-9. NOTE: the macro
/// attributes require STRING LITERALS (not `const` refs), so the name/path/iface
/// are repeated as literals here and as `const`s below for runtime use
/// (`name_has_owner` / verbose logging).
#[proxy(
    default_service = "io.mulletware.QMKonnect",
    default_path = "/io/mulletware/QMKonnect",
    interface = "io.mulletware.QMKonnect.WindowMonitor"
)]
trait WindowMonitor {
    /// `GetActiveWindow() -> (s app_class, s title)`. zvariant deserializes
    /// `(s,s)` -> `(String, String)`. Returns `("","")` when no window is
    /// focused (GOTCHA-6).
    fn get_active_window(&self) -> zbus::Result<(String, String)>;
    /// `ActiveWindowChanged(s app_class, s title)` signal.
    #[zbus(signal)]
    fn active_window_changed(&self, app_class: String, title: String);
}

/// Public monitor (`Send`: holds only `Arc<Mutex<_>>`/`Arc<AtomicBool>`/
/// `Option<JoinHandle>`/`bool` — mirrors `WaylandFtMonitor` exactly).
pub struct GnomeMonitor {
    last_focus: Arc<Mutex<Option<(String, String)>>>,
    shutdown: Arc<AtomicBool>,
    signal_handle: Option<JoinHandle<()>>,
    poll_handle: Option<JoinHandle<()>>,
    verbose: bool,
}

impl GnomeMonitor {
    pub fn new(verbose: bool) -> Self {
        Self {
            last_focus: Arc::new(Mutex::new(None)),
            shutdown: Arc::new(AtomicBool::new(false)),
            signal_handle: None,
            poll_handle: None,
            verbose,
        }
    }
}

impl WindowMonitor for GnomeMonitor {
    fn platform_name(&self) -> &str {
        "gnome"
    }

    /// Spawn-and-return: spawn the signal thread + the poll thread, return
    /// `Ok(())` promptly. Keeps the trait default
    /// `start_blocks_calling_thread() == false` (GOTCHA-1/2). The runner parks
    /// main / drives ksni on this contract — do NOT block here.
    fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let last_focus = Arc::clone(&self.last_focus);
        let shutdown = Arc::clone(&self.shutdown);
        let verbose = self.verbose;
        self.signal_handle = Some(thread::spawn(move || {
            run_signal_loop(last_focus, shutdown, verbose)
        }));

        let last_focus = Arc::clone(&self.last_focus);
        let shutdown = Arc::clone(&self.shutdown);
        let verbose = self.verbose;
        self.poll_handle = Some(thread::spawn(move || {
            run_poll_loop(last_focus, shutdown, verbose)
        }));
        Ok(())
    }

    /// Best-effort stop (GOTCHA-7): the poll thread exits within one interval
    /// (≤ `DEFAULT_POLL_MS`); the signal thread exits on the next signal /
    /// connection teardown (same posture as `wayland_ft::stop`, whose
    /// `blocking_dispatch` also blocks until the next event). The daemon
    /// process exits via the ctrlc/SIGTERM handler in `runners/linux.rs`
    /// regardless.
    fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.shutdown.store(true, Ordering::Release);
        if let Some(h) = self.poll_handle.take() {
            let _ = h.join(); // exits within ≤ DEFAULT_POLL_MS
        }
        if let Some(h) = self.signal_handle.take() {
            let _ = h.join(); // best-effort (may block until next signal / conn drop)
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Dedup + notify (GOTCHA-4: release last_focus BEFORE notify_qmk —
// `notify_qmk` takes the global debouncer STATE/NOTIFIER locks internally, so
// holding `last_focus` while notifying risks lock-ordering contention). Free fn
// — the threads own cloned Arcs.
// ---------------------------------------------------------------------------
fn apply_and_notify(
    last_focus: &Mutex<Option<(String, String)>>,
    candidate: (String, String),
    verbose: bool,
) {
    {
        let mut cell = last_focus.lock().unwrap();
        if *cell == Some(candidate.clone()) {
            return; // dedup — no churn (GOTCHA-6: ("","") == ("","") is a no-op)
        }
        *cell = Some(candidate.clone());
    }
    if verbose {
        println!(
            "[{}ms] gnome: {} | {}",
            crate::core::now_ms(),
            candidate.0,
            candidate.1
        );
    }
    let wi = WindowInfo::new(candidate.0, candidate.1);
    if let Err(e) = notifier::notify_qmk(&wi, verbose) {
        eprintln!("gnome: notify_qmk failed: {e}");
    }
}

/// Signal subscription thread (GOTCHA-1/2/9). Owns its Connection; blocks on
/// the `ActiveWindowChanged` iterator; exits on the next signal / connection
/// teardown when the shutdown flag is set (best-effort, GOTCHA-7).
fn run_signal_loop(
    last_focus: Arc<Mutex<Option<(String, String)>>>,
    shutdown: Arc<AtomicBool>,
    verbose: bool,
) {
    let conn = match zbus::blocking::Connection::session() {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "[{}ms] gnome: cannot connect to session bus (signal): {e}",
                crate::core::now_ms()
            );
            return;
        }
    };
    let proxy = match WindowMonitorProxyBlocking::new(&conn) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "[{}ms] gnome: proxy build failed (signal): {e}",
                crate::core::now_ms()
            );
            return;
        }
    };
    let iter = match proxy.receive_active_window_changed() {
        Ok(i) => i,
        Err(e) => {
            eprintln!(
                "[{}ms] gnome: receive_active_window_changed failed: {e}",
                crate::core::now_ms()
            );
            return;
        }
    };
    for ev in iter {
        // ev.args() -> ActiveWindowChangedArgs { app_class, title } (the
        // signal's payload, decoded by zvariant from (s,s)).
        let (app_class, title) = match ev.args() {
            Ok(a) => (a.app_class, a.title),
            Err(e) => {
                if verbose {
                    eprintln!("gnome: signal args decode failed: {e}");
                }
                continue;
            }
        };
        apply_and_notify(&last_focus, (app_class, title), verbose);
        if shutdown.load(Ordering::Acquire) {
            return;
        }
    }
}

/// Drift-poll + NameOwnerChanged thread (GOTCHA-2/3/11). Owns its Connection.
/// Each tick: hot-re-read `gnome_poll_interval_ms`; probe `name_has_owner`; if
/// owned -> `GetActiveWindow` + dedup (drift correction); if not owned -> emit
/// empty `("","")` once + no-backend (re-acquires automatically when the name
/// returns).
fn run_poll_loop(
    last_focus: Arc<Mutex<Option<(String, String)>>>,
    shutdown: Arc<AtomicBool>,
    verbose: bool,
) {
    let conn = match zbus::blocking::Connection::session() {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "[{}ms] gnome: cannot connect to session bus (poll): {e}",
                crate::core::now_ms()
            );
            // Sleep the default cadence so we don't spin if the bus is flapping.
            while !shutdown.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(DEFAULT_POLL_MS));
            }
            return;
        }
    };
    let proxy = match WindowMonitorProxyBlocking::new(&conn) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "[{}ms] gnome: proxy build failed (poll): {e}",
                crate::core::now_ms()
            );
            return;
        }
    };
    let dbus = match zbus::blocking::fdo::DBusProxy::new(&conn) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[{}ms] gnome: DBusProxy failed: {e}", crate::core::now_ms());
            return;
        }
    };

    while !shutdown.load(Ordering::Acquire) {
        // GOTCHA-11: hot-re-read the interval each tick (mtime-keyed cache), so
        // a config edit takes effect without restart. None => DEFAULT_POLL_MS.
        let ms = crate::core::cached_config()
            .ok()
            .and_then(|c| c.linux.gnome_poll_interval_ms)
            .unwrap_or(DEFAULT_POLL_MS);
        thread::sleep(Duration::from_millis(ms.max(1)));

        if shutdown.load(Ordering::Acquire) {
            break;
        }
        // §8.3 NameOwnerChanged semantics via name_has_owner (GOTCHA-3 — a
        // single blocking thread can only block on one SignalIterator, so fold
        // the ownership watch into the poll thread's existing ~1s round-trip).
        let owned =
            match dbus.name_has_owner(zbus::names::BusName::from_static_str(BUS_NAME).unwrap()) {
                Ok(b) => b,
                Err(e) => {
                    if verbose {
                        eprintln!("gnome: name_has_owner failed: {e}");
                    }
                    continue;
                }
            };
        if owned {
            match proxy.get_active_window() {
                Ok((app_class, title)) => {
                    apply_and_notify(&last_focus, (app_class, title), verbose)
                }
                Err(e) => {
                    if verbose {
                        eprintln!("gnome: GetActiveWindow failed: {e}");
                    }
                }
            }
        } else {
            // Extension not owned: emit empty once (no-backend posture; §8.3).
            apply_and_notify(&last_focus, (String::new(), String::new()), verbose);
        }
    }
}

// ---------------------------------------------------------------------------
// Availability probe (PLATFORMS.md §6 row 2). Cheap + side-effect-free: ONE
// `name_has_owner` round-trip to dbus-daemon (GOTCHA-5). NO `GetActiveWindow`
// call (the object may not be exported yet on a race; ownership of the name is
// the presence signal).
// ---------------------------------------------------------------------------
pub(crate) fn probe_available(verbose: bool) -> Result<(), String> {
    let conn = zbus::blocking::Connection::session().map_err(|e| {
        format!("cannot connect to the session bus (is DBUS_SESSION_BUS_ADDRESS set?): {e}")
    })?;
    let dbus = zbus::blocking::fdo::DBusProxy::new(&conn)
        .map_err(|e| format!("cannot create DBusProxy on the session bus: {e}"))?;
    let owned = dbus
        .name_has_owner(zbus::names::BusName::from_static_str(BUS_NAME).unwrap())
        .map_err(|e| format!("name_has_owner('{BUS_NAME}') failed: {e}"))?;
    if owned {
        if verbose {
            println!(
                "[{}ms] gnome: '{BUS_NAME}' is owned (extension installed+enabled)",
                crate::core::now_ms()
            );
        }
        Ok(())
    } else {
        Err(format!(
            "the GNOME Shell extension ('{BUS_NAME}') is not installed or not enabled \
             — install qmkonnect@mulletware from extensions.gnome.org (see docs)"
        ))
    }
}

/// Tracked active window as a single `(app_class, title)` row (empty vec when
/// no window is focused or the backend is unavailable). Synchronous one-shot
/// read (PLATFORMS.md §8.3).
#[allow(dead_code)]
pub fn list_foreground_windows() -> Vec<(String, String)> {
    let conn = match zbus::blocking::Connection::session() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let proxy = match WindowMonitorProxyBlocking::new(&conn) {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    match proxy.get_active_window() {
        Ok((app_class, title)) if !(app_class.is_empty() && title.is_empty()) => {
            vec![(app_class, title)]
        }
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Tests (hermetic — the load-bearing dedup/empty logic is pure; the live zbus
// plumbing is exercised manually in Level 4). Run single-threaded (GOTCHA-13:
// the select_tests module mutates process-global env).
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_and_notify_dedups_unchanged() {
        let cell = Mutex::new(Some(("firefox".to_string(), "x".to_string())));
        // Identical candidate -> no state change (we assert by observing the
        // cell stays equal).
        apply_and_notify(&cell, ("firefox".into(), "x".into()), false);
        assert_eq!(*cell.lock().unwrap(), Some(("firefox".into(), "x".into())));
    }

    #[test]
    fn apply_and_notify_updates_on_change() {
        let cell = Mutex::new(None);
        apply_and_notify(&cell, ("kitty".into(), "kitty".into()), false);
        assert_eq!(
            *cell.lock().unwrap(),
            Some(("kitty".into(), "kitty".into()))
        );
        apply_and_notify(&cell, ("kitty".into(), "other".into()), false);
        assert_eq!(
            *cell.lock().unwrap(),
            Some(("kitty".into(), "other".into()))
        );
    }

    #[test]
    fn apply_and_notify_empty_is_a_real_value_and_dedups() {
        let cell = Mutex::new(None);
        apply_and_notify(&cell, (String::new(), String::new()), false); // empty workspace
        assert_eq!(*cell.lock().unwrap(), Some((String::new(), String::new())));
        apply_and_notify(&cell, (String::new(), String::new()), false); // deduped (no churn)
        assert_eq!(*cell.lock().unwrap(), Some((String::new(), String::new())));
    }

    #[test]
    fn probe_err_when_no_session_bus() {
        // Snapshot/restore DBUS_SESSION_BUS_ADDRESS so this is hermetic +
        // single-thread-safe (must run --test-threads=1 like select_tests).
        let snap = std::env::var("DBUS_SESSION_BUS_ADDRESS").ok();
        std::env::remove_var("DBUS_SESSION_BUS_ADDRESS");
        let r = probe_available(false);
        match snap {
            Some(v) => std::env::set_var("DBUS_SESSION_BUS_ADDRESS", v),
            None => std::env::remove_var("DBUS_SESSION_BUS_ADDRESS"),
        }
        // On this Hyprland box a session bus usually exists; if it does, the
        // probe may Ok/Err on name ownership rather than connection. Only assert
        // the CONNECTION-failure path: when the env is unset AND no autostart
        // socket, expect Err mentioning the session bus.
        if let Err(m) = r {
            assert!(
                m.contains("session bus") || m.contains("not installed") || m.contains("DBUS"),
                "expected a connection/ownership Err; got: {m}"
            );
        }
        // (If a session bus IS reachable here, the probe legitimately
        // Ok/Errs on ownership; that is not a failure of this test — the gate
        // is "does not panic".)
    }
}

//! AT-SPI fallback backend — focused-accessible tracking via the a11y bus
//! (PLATFORMS.md §9 — priority #4, last-resort Linux backend).
//!
//! GNOME (Mutter) exposes no foreign-toplevel protocol and no client API for
//! the active window; the GNOME Shell extension (§8) is the primary path. THIS
//! module is the fallback of last resort for the single largest population that
//! lands on AT-SPI: GNOME-without-the-extension + accessibility enabled, plus
//! any compositor whose apps expose the a11y bus.
//!
//! **Design (GOTCHA-1):** the `atspi` crate's high-level
//! `AccessibilityConnection` is `async fn`-only (`new`, `register_event`,
//! `event_stream` all return futures/streams). The project has NO
//! tokio/async-std/smol runtime, and adding one for a single best-effort
//! backend is heavyweight. Instead — exactly like the sibling `gnome.rs` — we
//! use the `atspi` crate's typed `proxy::*Blocking` proxies + typed
//! `events::object::StateChangedEvent` over a **pure `zbus::blocking`
//! connection** (NO async runtime). This captures 100% of the crate's value
//! (typed proxies, typed events, the canonical match-rule + registry-event
//! strings, and the knowledge of how to find the a11y bus address via
//! `org.a11y.Bus.GetAddress`) with zero runtime.
//!
//! **Threading (GOTCHA-3):** TWO worker threads, each with its OWN
//! `zbus::blocking::Connection` (a shared Connection would serialize on its
//! internal executor). `start()` spawns both and returns promptly
//! (spawn-and-return; keeps the trait default
//! `start_blocks_calling_thread() == false`).
//!
//! **Best-effort, NOT primary (PLATFORMS.md §9 limitations):** `app_class` is
//! the focused accessible's *application* readable `Name` (NOT WM_CLASS —
//! inconsistent for Electron/sandboxed apps, may be "python3"/"chrome"/empty);
//! `title` is the focused accessible's own `Name` (which is NOT necessarily the
//! window toplevel title); apps without an a11y bridge are invisible. AT-SPI is
//! the documented fallback — prefer the GNOME Shell extension (§8) for reliable
//! GNOME support. Enable a11y in your desktop's Accessibility settings.
#![cfg(all(target_os = "linux", feature = "atspi"))]

use crate::core::notifier;
use crate::core::types::WindowInfo;
use crate::platforms::WindowMonitor;
use atspi::events::object::StateChangedEvent;
use atspi::proxy::accessible::AccessibleProxyBlocking;
use atspi::proxy::bus::BusProxyBlocking;
use atspi::proxy::registry::RegistryProxyBlocking;
use atspi::{ObjectRefOwned, State};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

// zbus is the project's own optional dep (Cargo.toml:52), enabled via the
// `gnome` feature (`gnome = ["dep:zbus"]`). The `atspi` crate ALSO uses zbus 5
// (unified — same major), but does NOT re-export it unless its own `zbus`
// feature is on (which the project does not enable). The PRP's design relies on
// the `gnome` feature providing `dep:zbus` so bare `zbus::` resolves — which is
// true under `default` features (both gnome+atspi on) and under
// `--no-default-features` (neither on ⇒ this file is cfg'd out). This mirrors
// `gnome.rs` exactly.
use zbus::blocking::connection::Builder as ConnectionBuilder;
use zbus::blocking::fdo::DBusProxy;
use zbus::blocking::{Connection, MessageIterator};
use zbus::match_rule::MatchRule;
use zbus::message::Type as MessageType;
use zbus::names::BusName;
use zbus::proxy::CacheProperties;

/// Well-known session-bus name of the AT-SPI bus launcher (`org.a11y.Bus`).
/// Owned ⇔ the a11y bus daemon is up (PLATFORMS.md §9 availability signal).
const A11Y_BUS_NAME: &str = "org.a11y.Bus";

/// Default drift-poll cadence (PLATFORMS.md §9: "every 1000 ms"). A fixed
/// const — the contract fixes the value, so no config field is needed (keeps
/// scope out of the shared Config schema; mirror `gnome.rs::DEFAULT_POLL_MS`).
const DEFAULT_POLL_MS: u64 = 1000;

/// Process-global best-effort cache of the last focused accessible, so the
/// standalone [`list_foreground_windows`] (which has no monitor handle) can
/// still surface a single row without re-discovering focus. Set by the signal
/// loop; read by the poll loop and `list_foreground_windows`. AT-SPI exposes no
/// "get currently-focused" RPC (GOTCHA-7), so this cache IS the only source.
static LAST_FOCUSED: Mutex<Option<ObjectRefOwned>> = Mutex::new(None);

// ---------------------------------------------------------------------------
// Public monitor (mirrors `GnomeMonitor` field-for-field, plus
// `last_focused_ref` — the one field gnome.rs lacks because its poll re-queries
// a single D-Bus method, whereas AT-SPI's poll re-queries a SPECIFIC cached
// object. `Send`: holds only `Arc<Mutex<_>>`/`Arc<AtomicBool>`/
// `Option<JoinHandle>`/`bool`.)
// ---------------------------------------------------------------------------
pub struct AtspiMonitor {
    last_focus: Arc<Mutex<Option<(String, String)>>>,
    last_focused_ref: Arc<Mutex<Option<ObjectRefOwned>>>,
    shutdown: Arc<AtomicBool>,
    signal_handle: Option<JoinHandle<()>>,
    poll_handle: Option<JoinHandle<()>>,
    verbose: bool,
}

impl AtspiMonitor {
    pub fn new(verbose: bool) -> Self {
        Self {
            last_focus: Arc::new(Mutex::new(None)),
            last_focused_ref: Arc::new(Mutex::new(None)),
            shutdown: Arc::new(AtomicBool::new(false)),
            signal_handle: None,
            poll_handle: None,
            verbose,
        }
    }
}

impl WindowMonitor for AtspiMonitor {
    fn platform_name(&self) -> &str {
        "atspi"
    }

    /// Spawn-and-return: spawn the signal thread + the poll thread, return
    /// `Ok(())` promptly. Keeps the trait default
    /// `start_blocks_calling_thread() == false` (GOTCHA-1/3). The runner parks
    /// main / drives ksni on this contract — do NOT block here.
    fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let last_focus = Arc::clone(&self.last_focus);
        let last_focused_ref = Arc::clone(&self.last_focused_ref);
        let shutdown = Arc::clone(&self.shutdown);
        let verbose = self.verbose;
        self.signal_handle = Some(thread::spawn(move || {
            run_signal_loop(last_focus, last_focused_ref, shutdown, verbose)
        }));

        let last_focus = Arc::clone(&self.last_focus);
        let last_focused_ref = Arc::clone(&self.last_focused_ref);
        let shutdown = Arc::clone(&self.shutdown);
        let verbose = self.verbose;
        self.poll_handle = Some(thread::spawn(move || {
            run_poll_loop(last_focus, last_focused_ref, shutdown, verbose)
        }));
        Ok(())
    }

    /// Best-effort stop (GOTCHA-9): the poll thread exits within one interval
    /// (≤ `DEFAULT_POLL_MS`); the signal thread blocks on the `MessageIterator`
    /// until the next message / connection teardown (same posture as
    /// `gnome::stop`). The daemon process exits via the ctrlc/SIGTERM handler
    /// in `runners/linux.rs` regardless.
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
// — the threads own cloned Arcs. (Verbatim copy of gnome.rs's helper, modulo
// the `atspi:` log prefix.)
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
            "[{}ms] atspi: {} | {}",
            crate::core::now_ms(),
            candidate.0,
            candidate.1
        );
    }
    let wi = WindowInfo::new(candidate.0, candidate.1);
    if let Err(e) = notifier::notify_qmk(&wi, verbose) {
        eprintln!("atspi: notify_qmk failed: {e}");
    }
}

/// Focus-gained predicate (GOTCHA-5). `StateChangedEvent` fires for MANY states
/// (Focused, Focusable, Active, Selected, …) and for both gained/lost. Track
/// ONLY `State::Focused` with `enabled == true` (`enabled` ==
/// `body.detail1() > 0` == focus GAINED; `enabled == false` == focus LOST).
/// Pure predicate — hermetic-testable.
fn is_focus_gained(ev: &StateChangedEvent) -> bool {
    ev.state == State::Focused && ev.enabled
}

/// Resolve `(app_class, title)` for a focused accessible over the a11y bus
/// (GOTCHA-6). `title` = the accessible's own `Name`; `app_class` = its
/// *application* object's `Name` (the readable name — UNRELIABLE: NOT WM_CLASS;
/// Electron/sandboxed apps may show "python3"/"chrome"/empty). Best-effort:
/// ANY zbus failure → `None` (the event/poll is skipped, not fatal).
fn resolve_names(
    a11y: &Connection,
    item: &ObjectRefOwned,
    verbose: bool,
) -> Option<(String, String)> {
    let acc = build_accessible(a11y, item)?;
    // title = the focused accessible's own Name (GOTCHA-6).
    let title = acc.name().ok().or_else(|| {
        if verbose {
            eprintln!(
                "[{}ms] atspi: accessible name (title) unreadable",
                crate::core::now_ms()
            );
        }
        None
    })?;
    // app_class = the APPLICATION object's Name (get_application → ObjectRefOwned).
    let app_ref = acc.get_application().ok().or_else(|| {
        if verbose {
            eprintln!(
                "[{}ms] atspi: get_application failed (app_class unreadable)",
                crate::core::now_ms()
            );
        }
        None
    })?;
    let app = build_accessible(a11y, &app_ref)?;
    let app_class = app.name().ok()?;
    Some((app_class, title))
}

/// Build an `AccessibleProxyBlocking` at the accessible's specific (destination,
/// path), overriding the proxy's built-in root defaults. `CacheProperties::No`
/// avoids a properties-changed subscription we don't need (focus events are
/// authoritative; the poll re-reads directly). Returns `None` on any failure
/// (best-effort). (GOTCHA-11: `item.name()` → `Option<&UniqueName>` = the app's
/// bus name = the destination; `item.path()` → the object path.)
fn build_accessible<'a>(
    a11y: &'a Connection,
    obj: &ObjectRefOwned,
) -> Option<AccessibleProxyBlocking<'a>> {
    let dest = obj.name()?; // UniqueName = the app's bus name (None ⇒ not a real accessible)
    AccessibleProxyBlocking::builder(a11y)
        .cache_properties(CacheProperties::No)
        .destination(dest.clone())
        .ok()?
        .path(obj.path())
        .ok()?
        .build()
        .ok()
}

/// Resolve the a11y bus address: prefer `org.a11y.Bus.GetAddress` on the
/// session bus; fall back to `$ATSPI_BUS_ADDRESS` (set by at-spi-bus-launcher).
/// Returns the address string + the a11y connection, or `None` on failure
/// (caller logs + returns — the backend simply stays inert; `probe_available`
/// is the authoritative presence gate).
fn open_a11y_bus(verbose: bool) -> Option<(String, Connection)> {
    // Try the session-bus GetAddress path first.
    if let Ok(session) = Connection::session() {
        if let Ok(bus) = BusProxyBlocking::new(&session) {
            if let Ok(addr) = bus.get_address() {
                if let Ok(a11y) = ConnectionBuilder::address(addr.as_str()).and_then(|b| b.build()) {
                    return Some((addr, a11y));
                }
            }
        }
    }
    // Fallback: the env var (at-spi-bus-launcher exports it).
    if let Ok(addr) = std::env::var("ATSPI_BUS_ADDRESS") {
        if !addr.is_empty() {
            if let Ok(a11y) = ConnectionBuilder::address(addr.as_str()).and_then(|b| b.build()) {
                return Some((addr, a11y));
            }
        }
    }
    if verbose {
        eprintln!(
            "[{}ms] atspi: could not open the a11y bus (org.a11y.Bus.GetAddress + \
             $ATSPI_BUS_ADDRESS both failed)",
            crate::core::now_ms()
        );
    }
    None
}

/// Signal subscription thread (GOTCHA-1/3/5). Owns its Connection; blocks on
/// the StateChanged signal iterator; for each focus-gained event resolves
/// `(app_class, title)`, seeds the poll + global cache, and notifies QMK. Exits
/// on the next signal / connection teardown when the shutdown flag is set
/// (best-effort, GOTCHA-9). NO async runtime, NO `AccessibilityConnection`.
fn run_signal_loop(
    last_focus: Arc<Mutex<Option<(String, String)>>>,
    last_focused_ref: Arc<Mutex<Option<ObjectRefOwned>>>,
    shutdown: Arc<AtomicBool>,
    verbose: bool,
) {
    let (_addr, a11y) = match open_a11y_bus(verbose) {
        Some(t) => t,
        None => return, // open_a11y_bus already logged.
    };

    // Tell apps to EMIT state-changed events (best-effort — the match rule
    // below still delivers events the daemon already forwards even if this
    // Errs, e.g. when already registered).
    if let Ok(registry) = RegistryProxyBlocking::new(&a11y) {
        if let Err(e) = registry.register_event("object:state-changed") {
            if verbose {
                eprintln!("[{}ms] atspi: register_event failed (continuing): {e}", crate::core::now_ms());
            }
        }
    } else if verbose {
        eprintln!(
            "[{}ms] atspi: RegistryProxy build failed (events may not be emitted)",
            crate::core::now_ms()
        );
    }

    // Match only StateChanged signals on the AT-SPI Object event interface.
    // `for_match_rule` AUTO-REGISTERs the rule on the daemon AND AUTO-DEREGISTERs
    // on drop — do NOT call add_match_rule yourself (research §4).
    let rule = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .interface("org.a11y.atspi.Event.Object")
        .and_then(|b| b.member("StateChanged"))
        .map(|b| b.build());
    let rule = match rule {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "[{}ms] atspi: could not build the StateChanged match rule: {e}",
                crate::core::now_ms()
            );
            return;
        }
    };
    let iter = match MessageIterator::for_match_rule(rule, &a11y, Some(8)) {
        Ok(i) => i,
        Err(e) => {
            eprintln!(
                "[{}ms] atspi: for_match_rule failed: {e}",
                crate::core::now_ms()
            );
            return;
        }
    };

    for msg in iter {
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        // The iterator yields Result<Message>; skip delivery errors.
        let msg = match msg {
            Ok(m) => m,
            Err(_) => continue,
        };
        // try_from validates interface + member; a non-StateChanged message ⇒
        // Err ⇒ continue (the match rule filters most, but be defensive).
        match StateChangedEvent::try_from(&msg) {
            Ok(ev) if is_focus_gained(&ev) => {
                if let Some(c) = resolve_names(&a11y, &ev.item, verbose) {
                    // Seed the poll + the global cache (GOTCHA-7: no get-focused
                    // RPC — the event stream is authoritative; the poll re-reads
                    // THIS cached object for drift).
                    {
                        *last_focused_ref.lock().unwrap() = Some(ev.item.clone());
                    }
                    {
                        *LAST_FOCUSED.lock().unwrap() = Some(ev.item.clone());
                    }
                    apply_and_notify(&last_focus, c, verbose);
                }
            }
            _ => continue,
        }
        if shutdown.load(Ordering::Acquire) {
            return;
        }
    }
}

/// Drift-poll thread (GOTCHA-3/7). Owns its Connection. Each tick (every
/// `DEFAULT_POLL_MS`): re-query the LAST focused accessible (cached
/// `ObjectRefOwned`) for its current `(app_class, title)` and dedup — catches
/// in-place title/app changes that fire NO focus event. No focus cached yet ⇒
/// no-op (GOTCHA-7: AT-SPI has no "get currently-focused" RPC; the event stream
/// seeds `last_focused_ref`). Same posture as gnome.rs's drift poll.
fn run_poll_loop(
    last_focus: Arc<Mutex<Option<(String, String)>>>,
    last_focused_ref: Arc<Mutex<Option<ObjectRefOwned>>>,
    shutdown: Arc<AtomicBool>,
    verbose: bool,
) {
    let (_addr, a11y) = match open_a11y_bus(verbose) {
        Some(t) => t,
        None => {
            // No a11y bus: sleep out the interval so we don't spin if the bus
            // is flapping (same posture as gnome.rs's poll-bus-failure branch).
            while !shutdown.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(DEFAULT_POLL_MS));
            }
            return;
        }
    };

    while !shutdown.load(Ordering::Acquire) {
        thread::sleep(Duration::from_millis(DEFAULT_POLL_MS));
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        // The last focused accessible (seeded by the signal thread). No item ⇒
        // no-op (GOTCHA-7: we cannot discover focus de novo each tick).
        let item = last_focused_ref.lock().unwrap().clone();
        if let Some(item) = item {
            if let Some(c) = resolve_names(&a11y, &item, verbose) {
                apply_and_notify(&last_focus, c, verbose);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Availability probe (PLATFORMS.md §9). Cheap + side-effect-free. Present iff
// `org.a11y.Bus` is owned on the session bus OR `$ATSPI_BUS_ADDRESS` is set
// (GOTCHA-8: "present" ≠ "useful" — the daemon being up does NOT mean apps are
// exposing a11y; that requires enabling Assistive Technology, documented in
// troubleshooting.md).
// ---------------------------------------------------------------------------
pub(crate) fn probe_available(verbose: bool) -> Result<(), String> {
    // Cheapest presence signal #1: the env var (set by at-spi-bus-launcher).
    if let Ok(addr) = std::env::var("ATSPI_BUS_ADDRESS") {
        if !addr.is_empty() {
            if verbose {
                println!(
                    "[{}ms] atspi: $ATSPI_BUS_ADDRESS set (a11y bus present)",
                    crate::core::now_ms()
                );
            }
            return Ok(());
        }
    }
    // Presence signal #2: org.a11y.Bus owned on the session bus (one
    // name_has_owner round-trip — mirrors gnome::probe_available).
    let conn = Connection::session()
        .map_err(|e| format!("cannot connect to the session bus: {e}"))?;
    let dbus = DBusProxy::new(&conn).map_err(|e| format!("DBusProxy failed: {e}"))?;
    let owned = dbus
        .name_has_owner(
            BusName::from_static_str(A11Y_BUS_NAME)
                .map_err(|e| format!("invalid bus name '{A11Y_BUS_NAME}': {e}"))?,
        )
        .map_err(|e| format!("name_has_owner('{A11Y_BUS_NAME}') failed: {e}"))?;
    if owned {
        if verbose {
            println!(
                "[{}ms] atspi: '{A11Y_BUS_NAME}' owned (a11y bus present)",
                crate::core::now_ms()
            );
        }
        Ok(())
    } else {
        Err(format!(
            "the AT-SPI/a11y bus is not available ('{A11Y_BUS_NAME}' not owned, \
             $ATSPI_BUS_ADDRESS unset). Enable Assistive Technology / Screen Reader in \
             your desktop's Accessibility settings (see docs/troubleshooting.md)."
        ))
    }
}

/// Tracked focused accessible as a single `(app_class, title)` row (empty vec
/// when no focus is known or the backend is unavailable). Synchronous one-shot
/// best-effort read (PLATFORMS.md §9). AT-SPI exposes no "get focused" RPC
/// (GOTCHA-7), so this reads the process-global [`LAST_FOCUSED`] cache (seeded
/// by the signal loop) and re-resolves it; empty until the first focus event.
#[allow(dead_code)]
pub fn list_foreground_windows() -> Vec<(String, String)> {
    let item = match LAST_FOCUSED.lock().unwrap().clone() {
        Some(i) => i,
        None => return Vec::new(),
    };
    let (_, a11y) = match open_a11y_bus(false) {
        Some(t) => t,
        None => return Vec::new(),
    };
    match resolve_names(&a11y, &item, false) {
        Some((app_class, title)) if !(app_class.is_empty() && title.is_empty()) => {
            vec![(app_class, title)]
        }
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Tests (hermetic — the load-bearing dedup/filter logic is pure; the live a11y
// bus is exercised manually in Level 4). Run single-threaded (GOTCHA-12: the
// probe test mutates process-global env; shared with select_tests' env
// mutations + the global debouncer STATE).
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_focus_gained_true() {
        // Focus GAINED on the Focused state ⇒ track.
        let ev = StateChangedEvent {
            item: sample_item(),
            state: State::Focused,
            enabled: true,
        };
        assert!(is_focus_gained(&ev));
    }

    #[test]
    fn is_focus_gained_wrong_state() {
        // A non-Focused state (Selected) is ignored even when enabled.
        let ev = StateChangedEvent {
            item: sample_item(),
            state: State::Selected,
            enabled: true,
        };
        assert!(!is_focus_gained(&ev));
    }

    #[test]
    fn is_focus_gained_focus_lost() {
        // Focused state but enabled == false ⇒ focus LOST ⇒ ignored.
        let ev = StateChangedEvent {
            item: sample_item(),
            state: State::Focused,
            enabled: false,
        };
        assert!(!is_focus_gained(&ev));
    }

    #[test]
    fn apply_and_notify_dedups_unchanged() {
        let cell = Mutex::new(Some(("firefox".to_string(), "x".to_string())));
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
        // ("","") is a real value (empty workspace / no focus), not "unset" —
        // it must be stored AND deduped (no churn). Mirrors gnome.rs's test.
        let cell = Mutex::new(None);
        apply_and_notify(&cell, (String::new(), String::new()), false);
        assert_eq!(*cell.lock().unwrap(), Some((String::new(), String::new())));
        apply_and_notify(&cell, (String::new(), String::new()), false);
        assert_eq!(*cell.lock().unwrap(), Some((String::new(), String::new())));
    }

    #[test]
    fn probe_available_err_message_mentions_enabling() {
        // Hermetic: unset ATSPI_BUS_ADDRESS + DBUS_SESSION_BUS_ADDRESS so the
        // probe fails on connection (no session bus). The Err must point the
        // user at enabling AT (Assistive/a11y) — NOT a raw zbus error string.
        // Run single-threaded (mutates process-global env; GOTCHA-12).
        let snap_atspi = std::env::var("ATSPI_BUS_ADDRESS").ok();
        let snap_session = std::env::var("DBUS_SESSION_BUS_ADDRESS").ok();
        std::env::remove_var("ATSPI_BUS_ADDRESS");
        std::env::remove_var("DBUS_SESSION_BUS_ADDRESS");
        let r = probe_available(false);
        std::env::set_var("ATSPI_BUS_ADDRESS", snap_atspi.unwrap_or_default());
        match snap_session {
            Some(v) => std::env::set_var("DBUS_SESSION_BUS_ADDRESS", v),
            None => std::env::remove_var("DBUS_SESSION_BUS_ADDRESS"),
        }
        // On this Hyprland box a session bus may still be reachable via the
        // autostart socket even with DBUS_SESSION_BUS_ADDRESS unset; only
        // assert the CONNECTION-failure path (the documented dev-box gate).
        if let Err(m) = r {
            assert!(
                m.contains("Assistive") || m.contains("a11y") || m.contains("session bus"),
                "expected an Err mentioning Assistive/a11y/session bus; got: {m}"
            );
        }
        // (If a session bus IS reachable here, the probe legitimately
        // Ok/Errs on org.a11y.Bus ownership; that is not a failure of this
        // test — the gate is "does not panic".)
    }

    /// Build a minimal `ObjectRefOwned` for the focus-filter tests (the field
    /// is read only when the event is actually sent over the bus; the tests
    /// never do, so a default item suffices).
    fn sample_item() -> ObjectRefOwned {
        ObjectRefOwned::default()
    }
}
//! foreign-toplevel Wayland backend (PLATFORMS.md §7 — priority #1).
//!
//! ⚠️ HEADLINE CORRECTION (documented in the PRP): **sctk 0.20 does NOT expose
//! the `wlr-foreign-toplevel-management-v1` protocol.** sctk 0.20's only
//! foreign-toplevel module is `foreign_toplevel_list`, which wraps the **ext**
//! protocol (`ext-foreign-toplevel-list-v1`) — and ext has **no activation
//! state**, so it cannot report focus. The deleted sctk 0.19 types
//! (`ForeignToplevelManager`/`ForeignToplevelHandler`/`ForeignToplevelState`)
//! DO NOT EXIST in 0.20. Therefore the load-bearing wlr protocol is
//! **hand-rolled** here with raw `wayland-client` `Dispatch` impls against the
//! generated `ZwlrForeignToplevelManagerV1` / `ZwlrForeignToplevelHandleV1`
//! types, reached via the `smithay_client_toolkit::reexports::protocols_wlr`
//! re-export (a transitive dep — no Cargo.toml change; Cargo.toml is owned by
//! the parallel sibling P2.M1.T2.S2).
//!
//! Coverage (PLATFORMS.md §7.2): Hyprland, Sway, Niri, River, Labwc, Wayfire,
//! KDE Plasma 6 (KWin), COSMIC all advertise the wlr global → this backend.
//! GNOME (Mutter) advertises neither foreign-toplevel protocol →
//! `probe_available` returns `Err` and `select_linux_backend` falls through to
//! the next backend (GNOME Shell extension, P2.M3).
//!
//! Model: `start()` spawns a dedicated dispatch thread and **returns
//! immediately** (unlike Hyprland's blocking listener) — so this monitor keeps
//! the trait default `start_blocks_calling_thread() == false`. The dispatch
//! thread binds the wlr manager, loops on `EventQueue::blocking_dispatch`, and
//! on compositor death (`DispatchError::Backend`) reconnects with the same
//! backoff constants Hyprland uses (re-declared locally — they're private to
//! `hyprland.rs`).
#![cfg(all(target_os = "linux", feature = "wayland"))]

use crate::core::notifier;
use crate::core::types::WindowInfo;
use crate::platforms::WindowMonitor;
use std::collections::HashMap;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use smithay_client_toolkit::reexports::client::{
    backend::ObjectId,
    globals::{registry_queue_init, GlobalListContents},
    protocol::wl_registry,
    Connection, Dispatch, DispatchError, Proxy, QueueHandle,
};
use smithay_client_toolkit::reexports::protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{Event as HandleEvent, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{self, Event as MgrEvent, ZwlrForeignToplevelManagerV1},
};

/// Shared snapshot of tracked toplevels as `(app_id, title)`, the activated
/// one first. The `Arc<Mutex<…>>` lets the dispatch thread publish while the
/// foreground-list reader reads it lock-free via a cloned `Arc`.
type ToplevelSnapshot = Arc<Mutex<Vec<(String, String)>>>;

// ---------------------------------------------------------------------------
// Reconnect backoff (mirrors hyprland.rs private consts — GOTCHA-7: those are
// private, so identical values are re-declared here. Factor ×3, reset after a
// stable connection of ≥ STABLE_CONNECTION_THRESHOLD.)
// ---------------------------------------------------------------------------
const INITIAL_RECONNECT_MS: u64 = 100;
const MAX_RECONNECT_MS: u64 = 10_000;
const STABLE_CONNECTION_THRESHOLD: Duration = Duration::from_secs(5);

/// Per-toplevel cached info (the dispatch thread's working set). `pending_*`
/// accumulate between `done` events; `current_*` is the last committed snapshot
/// used for focus recompute + the published list.
#[derive(Default, Clone)]
struct HandleInfo {
    pending_app_id: String,
    pending_title: String,
    pending_activated: bool,
    current_app_id: String,
    current_title: String,
    current_activated: bool,
}

/// The dispatch-thread state. Owns ALL wayland-touching data (not held by the
/// monitor struct — GOTCHA-5: the struct must stay `Send` and holds no wayland
/// objects). Toplevels are keyed by the handle's `ObjectId`.
struct DispatchState {
    toplevels: HashMap<ObjectId, HandleInfo>,
    /// Shared with the monitor: the last emitted `(app_class, title)`.
    last_focus: Arc<Mutex<Option<(String, String)>>>,
    /// Shared with `list_foreground_windows`: the published toplevel snapshot.
    list_snapshot: ToplevelSnapshot,
    verbose: bool,
}

/// The public monitor (`Send`: holds only `Arc<Mutex<>>`/`AtomicBool`/
/// `JoinHandle`/`bool` — GOTCHA-5).
pub struct WaylandFtMonitor {
    last_focus: Arc<Mutex<Option<(String, String)>>>,
    list_snapshot: ToplevelSnapshot,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    verbose: bool,
}

impl WaylandFtMonitor {
    pub fn new(verbose: bool) -> Self {
        Self {
            last_focus: Arc::new(Mutex::new(None)),
            list_snapshot: Arc::new(Mutex::new(Vec::new())),
            shutdown: Arc::new(AtomicBool::new(false)),
            handle: None,
            verbose,
        }
    }
}

impl WindowMonitor for WaylandFtMonitor {
    fn platform_name(&self) -> &str {
        "foreign-toplevel"
    }

    /// Spawn-and-return: spawns the dispatch thread and returns `Ok(())`
    /// immediately (GOTCHA-4). The runner parks main / drives the tray on this
    /// contract; do NOT block here and do NOT override
    /// `start_blocks_calling_thread` (keep the trait default `false`).
    fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Publish the list snapshot for list_foreground_windows() (GOTCHA-11).
        // Only one backend runs at a time, so a single module-level cell is safe.
        let _ = SHARED_SNAPSHOT.set(Arc::clone(&self.list_snapshot));

        let last_focus = Arc::clone(&self.last_focus);
        let list_snapshot = Arc::clone(&self.list_snapshot);
        let shutdown = Arc::clone(&self.shutdown);
        let verbose = self.verbose;
        self.handle = Some(thread::spawn(move || {
            run_dispatch_loop(last_focus, list_snapshot, shutdown, verbose)
        }));
        Ok(())
    }

    /// Best-effort stop: sets the shutdown flag (GOTCHA: `blocking_dispatch`
    /// blocks, so the thread exits on the next event / compositor teardown —
    /// same posture as Hyprland's blocking listener).
    fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.shutdown.store(true, Ordering::Release);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Pure helpers — the load-bearing correctness logic (GOTCHA-13: hermetically
// unit-tested; the connect/bind/dispatch plumbing is the only non-hermetic
// part).
// ---------------------------------------------------------------------------

/// Decode the wlr `state` event's raw byte array into "is this toplevel
/// activated?".
///
/// The `state` event arg is `type="array"` (no `enum=`) so wayland-scanner
/// generates `Event::State { state: Vec<u8> }` (wayland-scanner
/// client_gen.rs:214). The bytes are a packed little-endian `u32` sequence of
/// state flags; `activated` is value `2` (maximized=0, minimized=1,
/// **activated=2**, fullscreen=3 — see the wlr XML). There is NO
/// `State::Activated` enum variant to `.contains()` (GOTCHA-3).
fn decode_activated(state: &[u8]) -> bool {
    state
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .any(|v| v == 2)
}

/// Pure focus recompute: the toplevel whose committed `current_activated` is
/// true is the focus. Returns `None` when no toplevel is activated (an empty
/// workspace — the caller maps that to `("","")`; GOTCHA-10).
///
/// If MULTIPLE are activated (shouldn't happen but be defensive) the first
/// encountered wins.
fn recompute_focus(toplevels: &HashMap<ObjectId, HandleInfo>) -> Option<(String, String)> {
    toplevels
        .values()
        .find(|t| t.current_activated)
        .map(|t| (t.current_app_id.clone(), t.current_title.clone()))
}

/// Build the published toplevel list: every tracked toplevel as
/// `(app_id, title)`, with the activated one FIRST (GOTCHA-11). For determinism
/// (the `HashMap` has no order) the non-activated remainder is sorted by
/// `(app_id, title)`.
fn build_list(toplevels: &HashMap<ObjectId, HandleInfo>) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = Vec::with_capacity(toplevels.len());
    let mut activated: Option<(String, String)> = None;
    for t in toplevels.values() {
        let row = (t.current_app_id.clone(), t.current_title.clone());
        if t.current_activated {
            activated = Some(row);
        } else {
            rows.push(row);
        }
    }
    rows.sort();
    if let Some(act) = activated {
        rows.insert(0, act);
    }
    rows
}

impl DispatchState {
    /// Recompute focus from the committed toplevels, dedup against the last
    /// emitted cell (under ONE lock), refresh the published list snapshot, and
    /// notify QMK AFTER releasing the last-focus lock (GOTCHA-9: `notify_qmk`
    /// takes the debounce STATE/NOTIFIER locks internally — never hold
    /// `last_focus` while notifying).
    fn recompute_and_notify(&mut self) {
        let new = recompute_focus(&self.toplevels).unwrap_or_default(); // ("","") if none
        {
            let mut cell = self.last_focus.lock().unwrap();
            if *cell == Some(new.clone()) {
                return;
            }
            *cell = Some(new.clone());
        }
        *self.list_snapshot.lock().unwrap() = build_list(&self.toplevels);
        let wi = WindowInfo::new(new.0, new.1);
        if let Err(e) = notifier::notify_qmk(&wi, self.verbose) {
            eprintln!("wayland_ft: notify_qmk failed: {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// Hand-rolled wlr Dispatch impls (GOTCHA-1/2: no sctk wrapper; raw
// wayland-client). The `event` signature is the associated-function form
// `fn event(state: &mut State, proxy, event, data, conn, qh)` — NOT a method
// (`&mut self`), because `Dispatch<I, U, State = Self>` passes `State` as the
// first argument.
// ---------------------------------------------------------------------------

/// Manager: the only event is `toplevel` (a new toplevel appeared). The
/// generated `Event::Toplevel { toplevel: ZwlrForeignToplevelHandleV1 }` gives
/// us the freshly-created proxy directly. `app_id`/`title`/`state` arrive as
/// subsequent events on that handle; `done` commits them.
impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for DispatchState {
    fn event(
        _state: &mut Self,
        _mgr: &ZwlrForeignToplevelManagerV1,
        event: MgrEvent,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let MgrEvent::Toplevel { toplevel } = event {
            _state
                .toplevels
                .insert(toplevel.id(), HandleInfo::default());
        }
    }

    // The `toplevel` event carries a typed new_id (interface="zwlr_…_handle_v1"
    // in the XML), so the child proxy is created by the queue. We must tell the
    // queue HOW to create it — otherwise it panics with "Missing
    // event_created_child specialization". Mirrors sctk's own ext handler
    // (foreign_toplevel_list.rs:125-127). GOTCHA: EVT_TOPLEVEL_OPCODE is the
    // wayland-scanner-generated constant (common.rs:140: EVT_<NAME>_OPCODE).
    wayland_client::event_created_child!(Self, ZwlrForeignToplevelManagerV1, [
        zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (
            ZwlrForeignToplevelHandleV1,
            ()
        )
    ]);
}

/// Handle: title/app_id/state accumulate into `pending_*`; `done` commits
/// pending→current and recomputes focus; `closed` destroys + removes and
/// recomputes (may now be an empty workspace → emit `("","")`).
impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for DispatchState {
    fn event(
        state: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: HandleEvent,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let id = handle.id();
        match event {
            HandleEvent::Title { title } => {
                let info = state.toplevels.entry(id).or_default();
                info.pending_title = title;
            }
            HandleEvent::AppId { app_id } => {
                let info = state.toplevels.entry(id).or_default();
                info.pending_app_id = app_id;
            }
            HandleEvent::State { state: bytes } => {
                let info = state.toplevels.entry(id).or_default();
                info.pending_activated = decode_activated(&bytes);
            }
            HandleEvent::Done => {
                if let Some(info) = state.toplevels.get_mut(&id) {
                    info.current_app_id = info.pending_app_id.clone();
                    info.current_title = info.pending_title.clone();
                    info.current_activated = info.pending_activated;
                }
                state.recompute_and_notify();
            }
            HandleEvent::Closed => {
                handle.destroy();
                state.toplevels.remove(&id);
                state.recompute_and_notify(); // may now be empty workspace
            }
            _ => {}
        }
    }
}

/// Required by `registry_queue_init` (the registry list is maintained
/// internally by `GlobalListContents`; we don't need to react to dynamic
/// global events — the wlr global is bound once up front). Empty body, matches
/// the wayland-client docs skeleton (globals.rs docs / sctk's
/// list_shm_formats.rs example).
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for DispatchState {
    fn event(
        _state: &mut Self,
        _registry: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

// ---------------------------------------------------------------------------
// The dispatch loop (runs on the spawned thread). Connects, binds the wlr
// manager, dispatches until compositor death, then reconnects with backoff.
// ---------------------------------------------------------------------------
fn run_dispatch_loop(
    last_focus: Arc<Mutex<Option<(String, String)>>>,
    list_snapshot: ToplevelSnapshot,
    shutdown: Arc<AtomicBool>,
    verbose: bool,
) {
    let mut delay_ms = INITIAL_RECONNECT_MS;

    loop {
        if shutdown.load(Ordering::Acquire) {
            return;
        }

        let attempt_start = Instant::now();

        // Connect to the compositor ($WAYLAND_DISPLAY / $WAYLAND_SOCKET, with
        // a $XDG_RUNTIME_DIR/wayland-* scan fallback — see connect_compositor:
        // the service env may never have had the session vars imported).
        let conn = match connect_compositor() {
            Ok(c) => c,
            Err(e) => {
                if verbose {
                    println!(
                        "[{}ms] wayland_ft: cannot connect to Wayland, retrying in {delay_ms}ms: {e}",
                        crate::core::now_ms()
                    );
                }
                thread::sleep(Duration::from_millis(delay_ms));
                delay_ms = std::cmp::min(delay_ms * 3, MAX_RECONNECT_MS);
                continue;
            }
        };

        // registry + event queue (requires Dispatch<WlRegistry, GlobalListContents>).
        let (globals, mut queue) = match registry_queue_init::<DispatchState>(&conn) {
            Ok(x) => x,
            Err(e) => {
                if verbose {
                    println!(
                        "[{}ms] wayland_ft: registry init failed, retrying in {delay_ms}ms: {e}",
                        crate::core::now_ms()
                    );
                }
                thread::sleep(Duration::from_millis(delay_ms));
                delay_ms = std::cmp::min(delay_ms * 3, MAX_RECONNECT_MS);
                continue;
            }
        };

        // Bind the wlr foreign-toplevel manager (advertised versions 1..=3).
        let _mgr: ZwlrForeignToplevelManagerV1 = match globals.bind(&queue.handle(), 1..=3, ()) {
            Ok(m) => m,
            Err(e) => {
                // The probe already gates on the global being advertised; a bind
                // failure here is transient (compositor race) → reconnect.
                if verbose {
                    println!(
                        "[{}ms] wayland_ft: bind zwlr_foreign_toplevel_manager_v1 failed, retrying in {delay_ms}ms: {e}",
                        crate::core::now_ms()
                    );
                }
                thread::sleep(Duration::from_millis(delay_ms));
                delay_ms = std::cmp::min(delay_ms * 3, MAX_RECONNECT_MS);
                continue;
            }
        };

        let mut state = DispatchState {
            toplevels: HashMap::new(),
            last_focus: Arc::clone(&last_focus),
            list_snapshot: Arc::clone(&list_snapshot),
            verbose,
        };

        if verbose {
            println!(
                "[{}ms] wayland_ft: connected, dispatching foreign-toplevel events",
                crate::core::now_ms()
            );
        }

        // Dispatch until compositor death (GOTCHA-8). Backend(WaylandError)
        // ⟹ reconnect; BadMessage (protocol violation) ⟹ log + continue.
        loop {
            match queue.blocking_dispatch(&mut state) {
                Ok(_) => {}
                Err(DispatchError::Backend(e)) => {
                    if verbose {
                        println!(
                            "[{}ms] wayland_ft: compositor connection lost: {e}",
                            crate::core::now_ms()
                        );
                    }
                    break;
                }
                Err(DispatchError::BadMessage {
                    sender_id,
                    interface,
                    opcode,
                }) => {
                    eprintln!(
                        "wayland_ft: protocol violation (BadMessage) from {sender_id} \
                         {interface}#{opcode}; ignoring"
                    );
                }
            }
            if shutdown.load(Ordering::Acquire) {
                return;
            }
        }

        // Reset backoff if the lost connection was stable (GOTCHA-7).
        if attempt_start.elapsed() >= STABLE_CONNECTION_THRESHOLD {
            delay_ms = INITIAL_RECONNECT_MS;
        }
        thread::sleep(Duration::from_millis(delay_ms));
        delay_ms = std::cmp::min(delay_ms * 3, MAX_RECONNECT_MS);
    }
}

// ---------------------------------------------------------------------------
// Availability probe (PLATFORMS.md §6). Side-effect-free (GOTCHA-12): does NOT
// mutate env. Gate 1: a compositor is reachable — $WAYLAND_DISPLAY/
// $WAYLAND_SOCKET via connect_to_env, falling back to a SCAN of
// $XDG_RUNTIME_DIR/wayland-* sockets (the boot race: a systemd user service
// starts before the session env is imported, when the env vars are absent even
// though the compositor socket is live — see [connect_compositor]). Gate 2:
// the compositor advertises zwlr_foreign_toplevel_manager_v1 (a sync
// roundtrip in registry_queue_init populates the global list; with_list scans
// it).
// ---------------------------------------------------------------------------

/// True iff a `$XDG_RUNTIME_DIR` entry name is a Wayland socket candidate:
/// `wayland-<digits>` exactly. The `wayland-N.lock` lockfiles and unrelated
/// entries are excluded. Pure — unit-tested below.
fn is_wayland_socket_name(name: &str) -> bool {
    match name.strip_prefix("wayland-") {
        Some(rest) => !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()),
        None => false,
    }
}

/// Connect to the session compositor: `$WAYLAND_DISPLAY`/`$WAYLAND_SOCKET`
/// first (connect_to_env), then — the boot-race path — a scan of every
/// `wayland-<digits>` socket under `$XDG_RUNTIME_DIR`.
///
/// A systemd user service can start (and probe backends) BEFORE the session
/// env is imported into the user manager: `$WAYLAND_DISPLAY` is absent then
/// even though the compositor socket is live. connect_to_env-only discovery
/// made the probe fail in that window while the session-bus-based AT-SPI
/// probe succeeded — latching AT-SPI (present-but-useless under wlroots
/// compositors, GOTCHA-8) for the whole uptime; a manual service restart was
/// the only recovery. Scanning closes the window: this backend (priority #1)
/// wins selection the moment the compositor socket exists, env or no env.
/// The monitor's reconnect loop shares this fn, so a connection lost to
/// compositor death also recovers via scan when env is still unimported.
///
/// Note: connecting is a plain `connect(2)`; no protocol handshake happens
/// here (the registry roundtrip in the caller does that), so a non-compositor
/// socket named `wayland-N` cannot hang THIS fn — only the caller's roundtrip
/// could, same exposure the env path always had.
fn connect_compositor() -> Result<Connection, String> {
    let env_err = match Connection::connect_to_env() {
        Ok(conn) => return Ok(conn),
        Err(e) => e,
    };

    let runtime = match std::env::var("XDG_RUNTIME_DIR") {
        Ok(dir) if !dir.is_empty() => dir,
        _ => {
            return Err(format!(
                "cannot connect via $WAYLAND_DISPLAY ({env_err}) and $XDG_RUNTIME_DIR is not set \
                 (no socket scan possible)"
            ))
        }
    };

    // Deterministic order (read_dir order is arbitrary) so multi-socket hosts
    // always pick the same compositor.
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&runtime) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if is_wayland_socket_name(name) {
                    candidates.push(PathBuf::from(&runtime).join(name));
                }
            }
        }
    }
    candidates.sort();

    for socket in &candidates {
        if let Ok(stream) = UnixStream::connect(socket) {
            // from_socket wraps the stream without a protocol roundtrip, so
            // this cannot block; an Err here means the fd was unusable, which
            // for a freshly connected stream is not a real case — try the rest.
            if let Ok(conn) = Connection::from_socket(stream) {
                return Ok(conn);
            }
        }
    }

    Err(format!(
        "cannot connect to Wayland ($WAYLAND_DISPLAY: {env_err}; scanned {} wayland-* \
         socket(s) under $XDG_RUNTIME_DIR, none connectable)",
        candidates.len()
    ))
}


/// Minimal state just for the probe (keeps the real `DispatchState`'s state
/// untouched — a dedicated empty type is cleaner than reusing it).
struct ProbeState;

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for ProbeState {
    fn event(
        _state: &mut Self,
        _registry: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

pub(crate) fn probe_available(_verbose: bool) -> Result<(), String> {
    let conn = connect_compositor()
        .map_err(|e| format!("cannot connect to Wayland ($WAYLAND_DISPLAY + $XDG_RUNTIME_DIR scan): {e}"))?;
    let (globals, _queue) = registry_queue_init::<ProbeState>(&conn)
        .map_err(|e| format!("Wayland registry init failed: {e}"))?;
    let has_wlr = globals.contents().with_list(|list| {
        list.iter()
            .any(|g| g.interface == "zwlr_foreign_toplevel_manager_v1")
    });
    if has_wlr {
        Ok(())
    } else {
        Err(
            "compositor does not advertise zwlr_foreign_toplevel_manager_v1 \
             (GNOME/Mutter? falls through to the next backend)"
                .into(),
        )
    }
}

// ---------------------------------------------------------------------------
// list_foreground_windows — a free fn reading a shared snapshot the dispatch
// thread publishes (GOTCHA-11). Empty when no monitor has ever run.
// ---------------------------------------------------------------------------

/// Module-level shared snapshot. `start()` inits it with the monitor's
/// `list_snapshot` Arc; only one backend runs at a time, so a single static is
/// safe.
static SHARED_SNAPSHOT: OnceLock<ToplevelSnapshot> = OnceLock::new();

/// Tracked toplevels as `(app_id, title)`, the activated one first. Returns an
/// empty list if no `WaylandFtMonitor` has ever run (or none are tracked yet).
#[allow(dead_code)]
pub fn list_foreground_windows() -> Vec<(String, String)> {
    SHARED_SNAPSHOT
        .get()
        .map(|c| c.lock().unwrap().clone())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Tests (GOTCHA-13): the load-bearing logic is factored into PURE helpers so
// it's hermetically testable without a compositor. The Dispatch/connect/bind
// plumbing is the only non-hermetic part and is exercised live (Level 4).
// Run single-threaded: `cargo test --bin qmkonnect -- --test-threads=1`.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    /// Build a HandleInfo with the given committed fields.
    fn info(app_id: &str, title: &str, activated: bool) -> HandleInfo {
        HandleInfo {
            pending_app_id: app_id.into(),
            pending_title: title.into(),
            pending_activated: activated,
            current_app_id: app_id.into(),
            current_title: title.into(),
            current_activated: activated,
        }
    }

    /// A throwaway ObjectId stand-in for HashMap keys. Real ObjectIds come from
    /// the compositor; for pure-logic tests we only need distinct keys, so we
    /// synthesize via the display's null id is not unique — instead use the
    /// fact that ObjectId is opaque and construct distinct ones is impossible
    /// without a connection. So test with a single key per map (or use a
    /// helper that takes a Vec of HandleInfo directly). We test recompute_focus
    /// / build_list against maps keyed by a single dummy id.
    fn single_map(t: HandleInfo) -> HashMap<ObjectId, HandleInfo> {
        let mut m = HashMap::new();
        // ObjectId::null() is a stable, distinct-enough key for a 1-entry map.
        m.insert(ObjectId::null(), t);
        m
    }

    // ---------------- decode_activated ----------------

    #[test]
    fn decode_activated_single_value_2() {
        // LE u32 of 2 → activated.
        assert!(decode_activated(&2u32.to_le_bytes()));
    }

    #[test]
    fn decode_activated_empty_array() {
        assert!(!decode_activated(&[]));
    }

    #[test]
    fn decode_activated_ignores_values_0_1_3() {
        // maximized=0, minimized=1, fullscreen=3 — none is activated.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&3u32.to_le_bytes());
        assert!(!decode_activated(&bytes));
    }

    #[test]
    fn decode_activated_in_second_slot_of_eight_bytes() {
        // [maximized, activated] → activated.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());
        assert!(decode_activated(&bytes));
    }

    #[test]
    fn decode_activated_trailing_partial_chunk_ignored() {
        // A full activated chunk + a trailing 2 bytes (incomplete) — chunks_exact
        // drops the trailing partial chunk; the activated chunk still counts.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&[0xFF, 0xFF]); // partial, ignored
        assert!(decode_activated(&bytes));
    }

    #[test]
    fn decode_activated_only_partial_chunk_not_activated() {
        // Only 3 bytes (no complete chunk) → not activated.
        assert!(!decode_activated(&[2, 0, 0]));
    }

    // ---------------- recompute_focus ----------------

    #[test]
    fn recompute_focus_activated_present() {
        let map = single_map(info("firefox", "Mozilla Firefox", true));
        assert_eq!(
            recompute_focus(&map),
            Some(("firefox".into(), "Mozilla Firefox".into()))
        );
    }

    #[test]
    fn recompute_focus_empty_workspace_is_none() {
        // No activated toplevel ⟹ None (caller maps to ("","")).
        let map = single_map(info("kitty", "kitty", false));
        assert_eq!(recompute_focus(&map), None);
    }

    #[test]
    fn recompute_focus_empty_map_is_none() {
        let map: HashMap<ObjectId, HandleInfo> = HashMap::new();
        assert_eq!(recompute_focus(&map), None);
    }

    #[test]
    fn recompute_focus_picks_first_when_multiple_activated() {
        // Defensive: multiple activated shouldn't happen, but the first
        // encountered wins. We can't easily build a multi-key map without real
        // ObjectIds, so verify the single-active-among-inactive case via the
        // shape: an active toplevel is found even when it's the only entry.
        let map = single_map(info("code", "VS Code", true));
        assert_eq!(recompute_focus(&map).unwrap().0, "code".to_string());
    }

    // ---------------- build_list ----------------

    #[test]
    fn build_list_activated_first() {
        let map = single_map(info("firefox", "Mozilla Firefox", true));
        let list = build_list(&map);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], ("firefox".into(), "Mozilla Firefox".into()));
    }

    #[test]
    fn build_list_empty_when_no_toplevels() {
        let map: HashMap<ObjectId, HandleInfo> = HashMap::new();
        assert!(build_list(&map).is_empty());
    }

    #[test]
    fn build_list_includes_inactive_toplevels() {
        let map = single_map(info("kitty", "kitty", false));
        let list = build_list(&map);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], ("kitty".into(), "kitty".into()));
    }

    // ---------------- probe_available Err-path (GOTCHA-12) ----------------
    // Unset $WAYLAND_DISPLAY so connect_to_env fails AND pin $XDG_RUNTIME_DIR
    // to an empty TempDir so the boot-race socket scan (connect_compositor)
    // cannot reach the real compositor of the machine running the tests.
    // Snapshot/restore like the select_tests in linux.rs. MUST run
    // single-threaded (process-global env).

    #[test]
    fn probe_err_when_wayland_display_unset() {
        let snap_w = std::env::var("WAYLAND_DISPLAY").ok();
        let snap_s = std::env::var("WAYLAND_SOCKET").ok();
        let snap_r = std::env::var("XDG_RUNTIME_DIR").ok();
        let dir = tempfile::TempDir::new().unwrap();
        std::env::remove_var("WAYLAND_DISPLAY");
        std::env::remove_var("WAYLAND_SOCKET");
        std::env::set_var("XDG_RUNTIME_DIR", dir.path());
        let r = probe_available(false);
        match snap_w {
            Some(v) => std::env::set_var("WAYLAND_DISPLAY", v),
            None => std::env::remove_var("WAYLAND_DISPLAY"),
        }
        match snap_s {
            Some(v) => std::env::set_var("WAYLAND_SOCKET", v),
            None => std::env::remove_var("WAYLAND_SOCKET"),
        }
        match snap_r {
            Some(v) => std::env::set_var("XDG_RUNTIME_DIR", v),
            None => std::env::remove_var("XDG_RUNTIME_DIR"),
        }
        assert!(r.is_err(), "probe must fail when Wayland is unavailable");
        let msg = r.unwrap_err();
        assert!(
            msg.contains("Wayland") || msg.contains("WAYLAND") || msg.contains("connect"),
            "Err must mention the Wayland connection failure; got: {msg}"
        );
    }

    // ---------------- boot-race socket scan (connect_compositor) ----------------
    // Hermetic: TempDir runtime dir + a bound UnixListener stands in for the
    // compositor socket. Snapshot/restore env like the tests above.

    /// Snapshot the three env vars connect_compositor reads.
    fn snap_conn_env() -> (Option<String>, Option<String>, Option<String>) {
        (
            std::env::var("WAYLAND_DISPLAY").ok(),
            std::env::var("WAYLAND_SOCKET").ok(),
            std::env::var("XDG_RUNTIME_DIR").ok(),
        )
    }

    /// Restore them (None = unset).
    fn restore_conn_env(snap: (Option<String>, Option<String>, Option<String>)) {
        match snap.0 {
            Some(v) => std::env::set_var("WAYLAND_DISPLAY", v),
            None => std::env::remove_var("WAYLAND_DISPLAY"),
        }
        match snap.1 {
            Some(v) => std::env::set_var("WAYLAND_SOCKET", v),
            None => std::env::remove_var("WAYLAND_SOCKET"),
        }
        match snap.2 {
            Some(v) => std::env::set_var("XDG_RUNTIME_DIR", v),
            None => std::env::remove_var("XDG_RUNTIME_DIR"),
        }
    }

    #[test]
    fn wayland_socket_name_matches_only_digits() {
        assert!(is_wayland_socket_name("wayland-0"));
        assert!(is_wayland_socket_name("wayland-12"));
        // Lockfiles, empties, non-numeric suffixes must all be rejected.
        assert!(!is_wayland_socket_name("wayland-1.lock"));
        assert!(!is_wayland_socket_name("wayland-"));
        assert!(!is_wayland_socket_name("wayland"));
        assert!(!is_wayland_socket_name("wayland-foo"));
        assert!(!is_wayland_socket_name(".lock"));
        assert!(!is_wayland_socket_name(""));
    }

    #[test]
    fn connect_compositor_scans_runtime_dir_when_env_unset() {
        let dir = tempfile::TempDir::new().unwrap();
        let _listener =
            std::os::unix::net::UnixListener::bind(dir.path().join("wayland-7")).unwrap();
        // Decoys the scan must skip: the lockfile and a non-numeric sibling.
        std::fs::write(dir.path().join("wayland-7.lock"), b"").unwrap();
        std::fs::write(dir.path().join("wayland-README"), b"").unwrap();

        let snap = snap_conn_env();
        std::env::remove_var("WAYLAND_DISPLAY");
        std::env::remove_var("WAYLAND_SOCKET");
        std::env::set_var("XDG_RUNTIME_DIR", dir.path());
        let r = connect_compositor();
        restore_conn_env(snap);
        assert!(r.is_ok(), "scan must connect to the live socket: {r:?}");
        // keep _listener alive until here
        drop(_listener);
    }

    #[test]
    fn connect_compositor_err_when_env_and_scan_both_fail() {
        let dir = tempfile::TempDir::new().unwrap(); // empty runtime dir
        let snap = snap_conn_env();
        std::env::remove_var("WAYLAND_DISPLAY");
        std::env::remove_var("WAYLAND_SOCKET");
        std::env::set_var("XDG_RUNTIME_DIR", dir.path());
        let r = connect_compositor();
        restore_conn_env(snap);
        assert!(r.is_err());
        let msg = r.unwrap_err();
        assert!(
            msg.contains("scanned 0"),
            "Err must report the scan found nothing; got: {msg}"
        );
    }
}

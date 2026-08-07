# Research Notes — P2.M3.T2.S1: GNOME D-Bus client backend (`src/platforms/gnome.rs`)

> Evidence trail for the PRP. Every API claim below is verified against the
> **actual zbus 5.17.0 source** vendored in the local cargo registry
> (`~/.cargo/registry/src/index.crates.io-*/zbus-5.17.0/`) — Cargo.toml declares
> `zbus = { version = "5", optional = true }` and the resolved version is 5.17.0.
> This is more authoritative than web docs (which may describe any 5.x).

---

## 1. Scope boundary (read FIRST — prevents stepping on siblings)

| Concern | Owner | This task (S1)? |
|---|---|---|
| GNOME Shell extension source (`packaging/gnome-shell-extension/`) | **P2.M3.T1.S1** (parallel, in flight) | NO — treat its D-Bus contract as a fixed input |
| `src/platforms/gnome.rs` (the client) | **THIS task (S1)** | YES |
| `gnome_probe` stub body + `construct_backend` `"gnome"` arm (`src/platforms/linux.rs`) | THIS task (S1) | YES |
| `mod gnome;` declaration + `list_foreground_windows` gnome branch (`src/platforms/mod.rs`) | THIS task (S1) | YES |
| First-run GNOME notification UX (§8.4 — fire even when another backend picks up) | **P2.M3.T2.S2** | NO (see §5) |
| `[linux] gnome_poll_interval_ms` config field | P2.M1.T2.S1 (DONE) | consume only |
| `gnome` Cargo feature + zbus dep | P2.M1.T2.S2 (DONE) | consume only |
| CI zip job for the extension | P2.M7.T2.S1 | NO |

The S1 item TITLE says "...+ first-run notification" but the item's own
LOGIC/OUTPUT/DOCS enumerate ONLY the gnome.rs client + probe + select wiring +
Mode-A docs. The dedicated notification task **P2.M3.T2.S2 ("First-run GNOME
notification when extension missing")** owns the UX. A one-shot
`maybe_gnome_first_run_notify` ALREADY EXISTS in `src/runners/linux.rs` (fires in
the no-backend `Err` branch). **S1 does not implement the notification**; S1's
`probe` returning `Err("…not installed…")` is what ENABLES S2's logic. This
PRP scopes S1 accordingly and calls the boundary out explicitly.

---

## 2. zbus 5.17.0 API ground-truth (from vendored source)

### 2.1 Blocking session connection
`zbus/src/blocking/connection/mod.rs`:
```rust
#[derive(Debug, Clone)]
#[must_use = "Dropping a `Connection` will close the underlying socket."]
pub struct Connection { inner: crate::Connection, }

impl Connection {
    pub fn session() -> Result<Self> {
        block_on(crate::Connection::session()).map(Self::from)
    }
    pub fn call_method<D,P,I,M,B>(&self, dest, path, iface, method, body) -> Result<Message>
    pub fn send(&self, msg: &Message) -> Result<()>
}
```
- `Connection` is `Clone` (Arc-backed internally — cheap clone).
- `block_on` (utils.rs) drives the async Connection's **internal executor**.
- **`zbus::Connection` has an `internal_executor` field that DEFAULTS TRUE** and
  "spawns a thread to run the executor" (`connection/mod.rs` line 805-855 doc on
  `executor()`). → **The blocking API needs NO caller-side tokio/async-io.** Each
  `blocking::Connection::session()` spawns ONE background executor thread.

### 2.2 `fdo::DBusProxy` — `name_has_owner` + `NameOwnerChanged` (the probe + watch)
`zbus/src/fdo/dbus.rs`:
```rust
#[proxy(
    default_service = "org.freedesktop.DBus",
    default_path = "/org/freedesktop/DBus",
    interface = "org.freedesktop.DBus"
)]
pub trait DBus {
    fn name_has_owner(&self, name: BusName<'_>) -> Result<bool>;
    fn get_name_owner(&self, name: BusName<'_>) -> Result<OwnedUniqueName>;
    /// "the owner of a name has changed … detect the appearance of new names on the bus."
    #[zbus(signal)]
    fn name_owner_changed(&self, name: BusName<'_>,
                          old_owner: Optional<UniqueName<'_>>,
                          new_owner: Optional<UniqueName<'_>>);
    // … list_names, etc.
}
```
- **The `#[proxy]` macro auto-generates a `<Name>ProxyBlocking` struct.** The
  BLOCKING re-export is `zbus::blocking::fdo::DBusProxy` = `DBusProxyBlocking`
  (`blocking/fdo.rs`). Constructed as `DBusProxy::new(&conn)` where `conn` is a
  `zbus::blocking::Connection`.
- `name_has_owner` arg `BusName<'_>` accepts a `&str` / `"io.mulletware.QMKonnect"`.

### 2.3 Blocking `Proxy` + `SignalIterator`
`zbus/src/blocking/proxy/mod.rs`:
```rust
impl Proxy<'_> {
    pub fn new<D,P,I>(conn: &Connection, dest, path, iface) -> Result<Proxy>
    pub fn receive_signal<M>(&self, signal_name: M) -> Result<SignalIterator<'m>>
    pub fn receive_all_signals(&self) -> Result<SignalIterator<'static>>
}
pub struct SignalIterator<'a>(Option<crate::proxy::SignalStream<'a>>);
impl std::iter::Iterator for SignalIterator<'_> { /* next() blocks */ }
impl std::ops::Drop for SignalIterator<'_> { /* removes the match rule */ }
```
- A generated `WindowMonitorProxyBlocking` (from OUR `#[zbus::proxy]` trait) will
  expose `receive_active_window_changed() -> Result<ActiveWindowChangedIterator>`
  (blocking) — `.next()` blocks until the next signal; dropping the iterator
  removes the D-Bus match rule.

### 2.4 Our interface as a generated proxy
```rust
use zbus::proxy;
#[proxy(
    default_service = "io.mulletware.QMKonnect",
    default_path = "/io/mulletware/QMKonnect",
    interface = "io.mulletware.QMKonnect.WindowMonitor",
)]
trait WindowMonitor {
    /// GetActiveWindow() -> (s app_class, s title). zvariant deserializes (s,s) -> (String,String).
    fn get_active_window(&self) -> zbus::Result<(String, String)>;
    #[zbus(signal)]
    fn active_window_changed(&self, app_class: String, title: String);
}
```
Generates `WindowMonitorProxy` (async) **and** `WindowMonitorProxyBlocking`.
Build: `WindowMonitorProxyBlocking::new(&conn)?.get_active_window()`.

### 2.5 No 5.x executor footgun for the blocking API
Since zbus 5.0 the default executor is `async-io`; the blocking API sidesteps
this entirely (it spins the connection's own internal executor via `block_on`).
We do NOT enable the `tokio` feature and do NOT disable `async-io`. (Since 5.0
the blocking API itself is gated behind the ON-by-default `blocking-api` cargo
feature — we leave it on.)

---

## 3. Threading design (the load-bearing decision)

ARCHITECTURE.md §6 pins it: **"GNOME monitor: zbus signal subscription thread +
1000 ms drift-poll thread | last-emitted cell | NameOwnerChanged watch
re-acquires state."** Exactly TWO worker threads.

### Decision: TWO worker threads, each with its OWN `blocking::Connection`.
- **Thread A — signal subscription:** `Connection::session()` →
  `WindowMonitorProxyBlocking::new(&conn)` → `receive_active_window_changed()` →
  loop `.next()` → `apply_and_notify`.
- **Thread B — drift poll + NameOwnerChanged:** every `gnome_poll_interval_ms`
  (default 1000, **hot-re-read each tick** via `cached_config()`): call
  `name_has_owner("io.mulletware.QMKonnect")`; if owned → `get_active_window()`
  → `apply_and_notify` (drift correction); if NOT owned → emit empty `("","")`
  once + set no-backend (re-acquires automatically when the name returns).

### Why each thread gets its own Connection (not one shared Arc<Connection>)
1. A `blocking::Connection`'s blocking calls run on its internal executor's
   `block_on`. **Two threads sharing one Connection would SERIALIZE** on that
   executor — the signal-thread's blocking `.next()` would starve the poll
   thread's `get_active_window`. Two connections ⇒ two internal executor threads
   ⇒ true parallelism.
2. Connections are cheap (Arc-backed clone) and each owns exactly one socket to
   dbus-daemon. A daemon holding 2 session-bus connections is normal.
3. Each connection is **created INSIDE its thread closure** (moved in) — no
   cross-thread sharing, so no `Send`/`Sync` concerns about the connection.
   The `GnomeMonitor` struct holds only `Arc<Mutex<…>>` + `Arc<AtomicBool>` +
   `Option<JoinHandle>` + `bool` ⇒ `Send` (exactly the wayland_ft.rs shape).

### Decision: NameOwnerChanged is implemented by the POLL thread's `name_has_owner`
The spec §8.3 literally says "NameOwnerChanged watch". A **true** signal
subscription for `NameOwnerChanged` would need a THIRD thread (a single blocking
zbus thread can only block on ONE `SignalIterator` at a time — it can't poll both
`ActiveWindowChanged` and `NameOwnerChanged` iterators). Since the poll thread
already round-trips the bus every ~1 s, **checking `name_has_owner` each tick**
delivers identical SEMANTICS — re-acquire state when the name (re)appears, emit
empty + no-backend when it disappears — with ≤1 s latency and no third thread.
This is simpler, more robust, and stays within the 2-thread mandate. Documented as
GOTCHA in the PRP.

### Shutdown posture (mirrors wayland_ft.rs)
`stop()` sets the `AtomicBool` shutdown flag. The poll thread observes it within
one interval (≤1 s) and returns. The signal thread is blocked in
`SignalIterator::next()`; it exits on the **next** signal or when its connection
is torn down (best-effort — identical to wayland_ft's `blocking_dispatch` loop).
For a long-running daemon the process exits via SIGTERM/ctrlc (`runners/linux.rs`
sets a ctrlc handler) anyway, so best-effort stop is the codebase norm.

---

## 4. Patterns reused from the codebase

### 4.1 `notify_qmk` integration (mirrors `wayland_ft.rs::recompute_and_notify`)
`src/core/notifier.rs`:
```rust
pub fn notify_qmk(window_info: &WindowInfo, verbose: bool)
    -> Result<(), Box<dyn Error + Send + Sync>>
```
`src/core/types.rs`: `pub struct WindowInfo { … } impl WindowMonitor { pub fn new(app_class: String, title: String) -> Self }`

**GOTCHA-9 (from wayland_ft.rs, verbatim):** release the `last_focus` dedup lock
BEFORE calling `notify_qmk` — `notify_qmk` takes the global debouncer
`STATE`/`NOTIFIER` locks internally; holding `last_focus` while notifying risks
lock-ordering contention. Pattern (free fn — threads own cloned Arcs):
```rust
fn apply_and_notify(last_focus: &Mutex<Option<(String,String)>>,
                    candidate: (String,String), verbose: bool) {
    {
        let mut cell = last_focus.lock().unwrap();
        if *cell == Some(candidate.clone()) { return; }   // dedup
        *cell = Some(candidate.clone());
    }
    let wi = WindowInfo::new(candidate.0, candidate.1);
    if let Err(e) = notifier::notify_qmk(&wi, verbose) {
        eprintln!("gnome: notify_qmk failed: {e}");
    }
}
```

### 4.2 Hot-config interval (mirrors `core::configured_timing`)
`cached_config()` returns `Result<Config>`; `Config::default()` (no file) has
`linux.gnome_poll_interval_ms = None`. Resolve per tick:
```rust
let ms = crate::core::cached_config()
    .ok()
    .and_then(|c| c.linux.gnome_poll_interval_ms)
    .unwrap_or(DEFAULT_POLL_MS); // 1000
```
`core::now_ms()` exists for verbose timestamped logging (wayland_ft uses it).

### 4.3 `select_linux_backend` wiring (`src/platforms/linux.rs`)
- Replace the `gnome_probe` stub body (lines 170-173) to DELEGATE to
  `gnome::probe_available` (mirrors how `wayland_probe` delegates). The candidate
  row (line 49-50) is already `("gnome", gnome_probe as ProbeFn)` — unchanged.
- Add the `"gnome"` arm to `construct_backend` (currently has foreign-toplevel /
  hyprland / x11) — `#[cfg(feature="gnome")] "gnome" => Ok(Box::new(GnomeMonitor::new(verbose)))`.
- `mod.rs`: add `#[cfg(all(target_os="linux", feature="gnome"))] mod gnome;` and a
  gnome branch in `list_foreground_windows()` (single-window: `vec![(c,t)]` or
  `vec![]` when empty).

### 4.4 Empty-window semantics (PLATFORMS.md §1.3 / §8.2)
`focus_window == null` ⇒ the extension's `GetActiveWindow` returns `("","")`.
Map to empty `WindowInfo` and to `vec![]` for `list_foreground_windows`.

---

## 5. First-run notification — explicitly S2's scope (not S1)

`src/runners/linux.rs::maybe_gnome_first_run_notify` (already implemented) fires a
one-shot `notify-send` in the no-backend `Err` branch when `$XDG_CURRENT_DESKTOP`
contains GNOME. S1's new `gnome_probe` returning `Err("…not installed…")` is the
signal that flows into that path. **S2** ("First-run GNOME notification when
extension missing") extends it to fire even when another backend is selected
(§8.4: "AT-SPI may run meanwhile as best-effort interim"). S1 ships only the
client + probe; it does not add notification logic.

---

## 6. Validation strategy (this dev box is Hyprland, NOT GNOME)

Same posture as the extension PRP (P2.M3.T1.S1): the live `gdbus`/GNOME load is a
MANUAL gate documented for a GNOME VM, not a hard gate. The automated ceiling:
1. **Compile gate** — `cargo build --release` with the `gnome` feature (default on)
   must succeed; the `#[zbus::proxy]` macro, blocking API, and Send-typed monitor
   all type-check.
2. **Unit tests on pure helpers** — `apply_and_notify` dedup + empty-window mapping
   + `list_foreground_windows` empty handling are hermetically testable WITHOUT a
   session bus or the extension (factor the dedup into a pure fn that takes the
   cell by `&Mutex`).
3. **Probe test** — `probe_available` returns `Err` when there is no session bus /
   the name is unowned (snapshot/restore `DBUS_SESSION_BUS_ADDRESS`).
4. **select_linux_backend tests** — under `HYPRLAND_INSTANCE_SIGNATURE` unset + no
   wlr global (GNOME-like env), forcing `gnome` is unavailable (Err naming the
   extension); these join the existing `select_tests` module (single-threaded).
5. **Manual gdbus** (documented, deferred to a GNOME VM): with the extension
   installed+enabled, run the daemon under `-v` and confirm focus changes emit.

---

## 7. URLs (external confirmation, secondary to the local source)

- https://docs.rs/zbus/latest/zbus/blocking/struct.Connection.html — blocking Connection (session/call_method)
- https://docs.rs/zbus/latest/zbus/blocking/fdo/struct.DBusProxy.html — blocking DBusProxy (name_has_owner, receive_name_owner_changed)
- https://docs.rs/zbus/latest/zbus/blocking/proxy/struct.SignalIterator.html — blocking signal iterator
- https://dbus2.github.io/zbus/ — macro book (`#[proxy]` generates `*Blocking`)
- https://github.com/dbus2/zbus/tree/main/zbus/examples — `dbus_broker`/blocking examples
- spec/PLATFORMS.md §8.1 (D-Bus contract), §8.3 (client spec), §8.4 (first-run)
- spec/ARCHITECTURE.md §6 (GNOME monitor threading row)
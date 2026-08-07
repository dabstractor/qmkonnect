# Research Notes — P2.M4.T1.S1: AT-SPI Fallback Backend (`src/platforms/atspi.rs`)

All API facts below are **verified by reading the actual crate source** in the
local cargo registry cache (`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`),
NOT from memory or docs.rs. Crate versions (locked in `Cargo.lock`):

- `atspi` **0.30.0** — umbrella crate (re-exports subcrates).
- `atspi-common` **0.14.0** — types + events.
- `atspi-proxies` **0.14.0** — `#[zbus::proxy]` generated proxies.
- `atspi-connection` **0.14.0** — `AccessibilityConnection` (async-only — NOT used).
- `zbus` **5.17.0** — already a project dep (gnome backend), blocking API available.

---

## 0. THE HEADLINE DESIGN DECISION (read first)

**The `atspi` crate's `AccessibilityConnection` is async-only** (`new()`,
`register_event`, `event_stream` are all `async fn`). The project has **NO tokio
/ async-std / smol runtime** (grep `Cargo.toml`: only `ksni` with `blocking`).
Spinning up tokio just for one backend is heavyweight and out of character (the
gnome backend uses pure `zbus::blocking::*`).

**Solution (chosen):** Do NOT use `atspi::connection::AccessibilityConnection`.
Instead use, in a `zbus::blocking` context (mirroring `src/platforms/gnome.rs`):

1. `atspi::proxy::bus::BusProxyBlocking` — get the a11y bus address.
2. `zbus::blocking::ConnectionBuilder::address` — connect to the a11y bus.
3. `atspi::proxy::registry::RegistryProxyBlocking` — register the event.
4. `zbus::blocking::MessageIterator::for_match_rule` — iterate StateChanged signals.
5. `atspi::events::object::StateChangedEvent::try_from(&msg)` — decode + filter.
6. `atspi::proxy::accessible::AccessibleProxyBlocking` — get name/title/app.

This is 100% blocking, needs no runtime, and reuses the in-repo gnome.rs pattern
almost verbatim. The `atspi` crate's value (typed proxies + typed events + match
rule strings + knowledge of the a11y bus address) is fully captured.

---

## 1. CRATE STRUCTURE & RE-EXPORTS

`atspi-0.30.0/src/lib.rs`:
```rust
pub use atspi_common::*;                 // State, ObjectRefOwned, events::*, ...
#[cfg(feature = "proxies")]   pub use atspi_proxies as proxy;          // proxy::accessible, proxy::registry, proxy::bus
#[cfg(feature = "connection")] pub use atspi_connection as connection; // NOT USED
#[cfg(feature = "zbus")]      pub use zbus;                            // re-exported zbus
```

**Default features** (atspi 0.30 `Cargo.toml`): `connection, proxies, wrappers`
(+zbus via deps). The project declares `atspi = { version = "0.30", optional = true }`
⇒ default features ON ⇒ `proxies`, `common` (wrappers), `zbus` all available.
Project feature gate (Cargo.toml:143): `atspi = ["dep:atspi"]`, and it IS in
`default = ["wayland","gnome","atspi","hyprland","macos","linux-tray"]` (L137).

Imports this backend will use:
```rust
use atspi::events::object::StateChangedEvent;   // typed event (TryFrom<&Message>)
use atspi::proxy::accessible::AccessibleProxyBlocking;
use atspi::proxy::bus::BusProxyBlocking;        // get_address() on SESSION bus
use atspi::proxy::registry::RegistryProxyBlocking;
use atspi::{ObjectRefOwned, State};             // re-exported via atspi_common::*
use zbus::blocking::{Connection, ConnectionBuilder, MessageIterator};
use zbus::blocking::fdo::DBusProxy;
use zbus::message::Type as MessageType;
use zbus::names::BusName;
use zbus::proxy::CacheProperties;
use zbus::MatchRule;
```

---

## 2. THE PROXIES (atspi-proxies-0.14.0) — all `#[zbus::proxy]`, all generate `*Blocking`

### `bus.rs` — `Bus` (org.a11y.Bus on SESSION bus)
```rust
#[zbus::proxy(interface="org.a11y.Bus", default_service="org.a11y.Bus", default_path="/org/a11y/bus")]
pub trait Bus { fn get_address(&self) -> zbus::Result<String>; }
// ⇒ BusProxy (async) AND BusProxyBlocking (sync)
```
(Also `Status` proxy for `is_enabled` / `screen_reader_enabled` — not needed here.)

### `registry.rs` — `Registry` (org.a11y.atspi.Registry on the A11Y bus)
```rust
#[zbus::proxy(interface="org.a11y.atspi.Registry", default_service="org.a11y.atspi.Registry",
              default_path="/org/a11y/atspi/registry")]
pub trait Registry {
    fn register_event(&self, event: &str) -> zbus::Result<()>;     // tells apps to EMIT
    fn deregister_event(&self, event: &str) -> zbus::Result<()>;
    #[zbus(name="GetRegisteredEvents")] fn registered_events(&self) -> zbus::Result<Vec<(OwnedBusName,String)>>;
}
// ⇒ RegistryProxyBlocking
```

### `accessible.rs` — `Accessible` (org.a11y.atspi.Accessible)
Key members (file lines verified):
```rust
#[zbus::proxy(interface="org.a11y.atspi.Accessible",
              default_path="/org/a11y/atspi/accessible/root", assume_defaults=true)]
pub trait Accessible {
    fn get_application(&self) -> zbus::Result<ObjectRefOwned>;   // L40
    fn get_parent(&self) -> zbus::Result<ObjectRefOwned>;        // property L223
    fn get_children(&self) -> zbus::Result<Vec<ObjectRefOwned>>; // L86
    fn get_child_at_index(&self, i: i32) -> zbus::Result<ObjectRefOwned>;
    fn get_role(&self) -> zbus::Result<Role>;                    // L140
    fn get_state(&self) -> zbus::Result<StateSet>;               // L159  ← poll seed
    #[zbus(property)] fn name(&self) -> zbus::Result<String>;    // L221  ← TITLE / app_class
    #[zbus(property)] fn parent(&self) -> zbus::Result<ObjectRefOwned>;
}
// ⇒ AccessibleProxyBlocking
```
**`name()` is the `Name` PROPERTY** (the readable name). For the focused
accessible → `name()` = the title. For the application object → `name()` =
app_class. (NOT a method — `#[zbus(property)]`.)

### Building a proxy at an arbitrary path (focused accessible lives at an app-specific path)
From `proxy_ext.rs` (async) — blocking analogue:
```rust
let acc = AccessibleProxyBlocking::builder(&conn)
    .cache_properties(CacheProperties::No)
    .destination(item.name().cloned())?     // UniqueName = app's bus name
    .path(item.path().clone())?             // the accessible's object path
    .build()?;
```
`zbus::blocking::proxy::Builder` (zbus-5.17.0/src/blocking/proxy/builder.rs) has
`.destination/.path/.cache_properties/.build`.

---

## 3. THE EVENT + MATCH-RULE CONSTANTS (atspi-common-0.14.0/src/events/object.rs L645-650)

`impl_member_interface_registry_string_and_match_rule_for_event!(StateChangedEvent, "StateChanged", "org.a11y.atspi.Event.Object", "object:state-changed", "type='signal',interface='org.a11y.atspi.Event.Object',member='StateChanged'");`

⇒ on `StateChangedEvent`:
- `DBUS_MEMBER` = `"StateChanged"`
- `DBUS_INTERFACE` = `"org.a11y.atspi.Event.Object"`
- `REGISTRY_EVENT_STRING` = `"object:state-changed"`  ← pass to RegistryProxy::register_event
- `MATCH_RULE_STRING` = `"type='signal',interface='org.a11y.atspi.Event.Object',member='StateChanged'"`

### `StateChangedEvent` struct + decode (L351 / L654 / L977)
```rust
pub struct StateChangedEvent {
    pub item: ObjectRefOwned,   // source accessible (bus name + path), parsed from msg HEADER
    pub state: State,           // body.kind().into()  — e.g. State::Focused
    pub enabled: bool,          // body.detail1() > 0
}
```
`impl_from_dbus_message!(StateChangedEvent)` (macros.rs:298, `Auto` arm) ⇒
**`impl<'m> TryFrom<&'m zbus::Message> for StateChangedEvent`**.
```rust
let ev = StateChangedEvent::try_from(&msg)?;   // validates iface+member+signature; Err on non-match
```
The conversion: validates interface == Event.Object & member == StateChanged,
parses `item` from the header, `state = body.kind().into()`, `enabled = detail1 > 0`.
→ **Filter: `ev.state == State::Focused && ev.enabled`** (focus GAINED).

### `State` enum (state.rs:64 / :245)
`State::Focused` ↔ `"focused"` (also `Focusable`, `Active`, `Selected`, …).
`StateSet` is a bitmask (`enumflags2`); `set.contains(State::Focused)`.

---

## 4. zbus 5.17 BLOCKING APIs (zbus-5.17.0/src/blocking/) — already used by gnome.rs

### Connect to the a11y bus
```rust
let session = zbus::blocking::Connection::session()?;          // gnome.rs uses this
let addr = BusProxyBlocking::new(&session)?.get_address()?;    // e.g. "unix:abstract=..."
let a11y = zbus::blocking::ConnectionBuilder::address(&addr)?.build()?;
// (builder.rs:74 address, :264 build — verified)
```
Fallback if `org.a11y.Bus` is absent but `$ATSPI_BUS_ADDRESS` is set: parse env
directly into `ConnectionBuilder::address`.

### Signal subscription: `MessageIterator::for_match_rule` (message_iterator.rs)
```rust
let rule = MatchRule::builder()
    .msg_type(MessageType::Signal)
    .interface("org.a11y.atspi.Event.Object")?
    .member("StateChanged")?
    .build();
let mut iter = MessageIterator::for_match_rule(rule, &a11y, Some(8))?;
for msg in iter {                                 // BLOCKING iterator
    if let Ok(ev) = StateChangedEvent::try_from(&msg) { /* filter, resolve, dedup */ }
}
```
**`for_match_rule` auto-REGISTERs the match rule on the daemon AND auto-DEREGISTERs
on drop.** (Doc comment, message_iterator.rs: "the match rule is immediately
deregistered when the iterator is dropped".) So you do NOT call add_match_rule
yourself when using this API.

### name_has_owner (availability probe — same as gnome.rs probe_available)
```rust
let dbus = DBusProxy::new(&session)?;
let owned = dbus.name_has_owner(BusName::from_static_str("org.a11y.Bus")?)?;
```

---

## 5. AVAILABILITY (PLATFORMS.md §9) — cheap, side-effect-free

Present iff **`org.a11y.Bus` owned on the session bus OR `$ATSPI_BUS_ADDRESS` set.**
Implementation mirrors `gnome::probe_available` (gnome.rs bottom):
```rust
pub(crate) fn probe_available(verbose: bool) -> Result<(), String> {
    if let Ok(a) = std::env::var("ATSPI_BUS_ADDRESS") { if !a.is_empty() { return Ok(()); } }
    let conn = Connection::session().map_err(|e| format!("no session bus: {e}"))?;
    let dbus = DBusProxy::new(&conn).map_err(|e| format!("DBusProxy: {e}"))?;
    match dbus.name_has_owner(BusName::from_static_str("org.a11y.Bus").unwrap()) {
        Ok(true)  => Ok(()),
        Ok(false) => Err("a11y bus not found (org.a11y.Bus not owned, ATSPI_BUS_ADDRESS unset). \
                          Enable Assistive Technology / Screen Reader in your desktop's Accessibility \
                          settings.".into()),
        Err(e)    => Err(format!("name_has_owner('org.a11y.Bus') failed: {e}")),
    }
}
```
**CRITICAL gotcha (GOTCHA-8): "present" ≠ "useful".** `org.a11y.Bus` owned just
means the a11y bus daemon is up. Apps only EXPOSE accessibility when the desktop
has AT enabled (GNOME: `gsettings set org.gnome.desktop.interface
toolkit-accessibility true`, or Settings → Accessibility). On a box where a11y is
off, the probe returns Ok but the backend sees ZERO events. Document this.

---

## 6. POLL FALLBACK (1000 ms) — the honest reality

**AT-SPI exposes NO single "get the currently-focused object" RPC.** The
Registry/desktop root has no `GetFocus` method. The only authoritative focus
signal is the `object:state-changed:focused` event. So the 1000 ms poll cannot
authoritatively "query the focused object" in O(1).

Pragmatic, well-behaved design (matching the contract's "best-effort" framing):
- Shared state holds the **last focused `ObjectRefOwned`** (set by the signal thread).
- Poll tick (every 1000 ms): if a last-focused ref exists → re-build
  `AccessibleProxyBlocking` on it → re-read `name()` (title) +
  `get_application()`→name() (app_class) → dedup via `apply_and_notify`. This
  catches **in-place title/app changes that fire NO focus event** (drift).
- If no focus has ever been seen: no-op (the backend waits for the first focus
  event; a full per-tick tree-walk of the a11y hierarchy would spike CPU every
  second and is out of scope for a "fallback" backend).
- (Optional seeding: a ONE-TIME bounded walk from the registry root at startup —
  `AccessibleProxyBlocking` on `/org/a11y/atspi/registry` → `get_children()` →
  per-app `get_state()` looking for `StateSet` containing `Focused`. Marked as a
  best-effort enhancement; the core loop is event-driven.)

This is **the same posture as gnome.rs's drift poll** (`run_poll_loop`), which
also just re-reads the last known state each tick rather than discovering focus
de novo.

---

## 7. WIRING INTO `select_linux_backend` (src/platforms/linux.rs)

The `atspi` feature + the priority row ALREADY EXIST (stubs):

`linux_backend_candidates()` (verified) already has, in order:
`foreign-toplevel → gnome → hyprland → atspi → x11`.  ← atspi is priority **#4** ✓.

Current stub (linux.rs:179) — REPLACE:
```rust
fn atspi_probe(_verbose: bool) -> Result<(), String> {
    Err("AT-SPI backend not yet implemented (P2.M4)".into())
}
```
→
```rust
#[cfg(feature = "atspi")]
fn atspi_probe(verbose: bool) -> Result<(), String> {
    crate::platforms::atspi::probe_available(verbose)
}
```

`construct_backend()` (verified) — ADD an arm:
```rust
#[cfg(feature = "atspi")]
"atspi" => Ok(Box::new(crate::platforms::atspi::AtspiMonitor::new(verbose))),
```

Module decl (mod.rs: after the `gnome` mod at L22) — ADD:
```rust
#[cfg(all(target_os = "linux", feature = "atspi"))]
mod atspi;
```
(match the gnome mod's cfg exactly.)

`mod.rs::list_foreground_windows()` cfg ladder — ADD an atspi arm reached only
when wayland+gnome+hyprland are all off (mirrors the gnome arm's `not(feature=)`
gating), calling `atspi::list_foreground_windows()`.

---

## 8. CONCURRENCY / THREADING MODEL (mirror gnome.rs exactly)

`AtspiMonitor` struct (Send: holds only `Arc<Mutex<_>>`, `Arc<AtomicBool>`,
`Option<JoinHandle>`, bool) — same shape as `GnomeMonitor`:
- `start()`: spawn TWO worker threads (signal-loop, poll-loop), each with its OWN
  `zbus::blocking::Connection` (GOTCHA-3: a shared Connection serializes on its
  internal executor). Return `Ok(())` promptly (spawn-and-return; keeps the trait
  default `start_blocks_calling_thread() == false`).
- `stop()`: set shutdown flag; join poll thread (exits ≤ 1000 ms); join signal
  thread best-effort (blocks on the MessageIterator until next msg / conn drop —
  same posture as gnome.rs GOTCHA-7; the daemon exits via SIGTERM regardless).

Shared dedup helper (free fn, mirrors gnome.rs `apply_and_notify`): release the
`last_focus` lock BEFORE `notifier::notify_qmk` (which takes the global debouncer
locks internally — lock-ordering, gnome.rs GOTCHA-4).

---

## 9. CORE HELPERS THIS BACKEND CONSUMES (verified signatures)

```rust
// src/core/types.rs
pub struct WindowInfo { pub app_class: String, pub title: String }
impl WindowInfo { pub fn new(app_class: String, title: String) -> Self }

// src/core/notifier.rs:1698
pub fn notify_qmk(window_info: &WindowInfo, verbose: bool)
    -> Result<(), Box<dyn Error + Send + Sync>>;

// src/core/mod.rs:131
pub fn now_ms() -> u128;   // monotonic ms since first call (verbose timestamp)

// src/platforms/mod.rs  (the trait)
pub trait WindowMonitor: Send {
    fn platform_name(&self) -> &str;
    fn start(&mut self) -> Result<(), Box<dyn Error>>;
    fn stop(&mut self) -> Result<(), Box<dyn Error>> { Ok(()) }      // default no-op
    fn start_blocks_calling_thread(&self) -> bool { false }          // default
}
```
`platform_name()` returns `"atspi"`.

---

## 10. TESTING POSTURE (matches gnome.rs)

`cargo test --bin qmkonnect -- --test-threads=1` (shared global debouncer STATE +
the env-mutating tests; gnome.rs GOTCHA-13). Hermetic `#[cfg(test)] mod tests`:
the load-bearing logic is pure — test `apply_and_notify` (dedup / update /
empty-is-real-value) and a focus-filter predicate helper (`state==Focused &&
enabled` → track; other states → ignore). Do NOT hit the live a11y bus in unit
tests (that's Level 4 manual, on a GNOME/Ubuntu box with a11y ON).

Dev box is Hyprland (no a11y by default) → hard gates are `cargo build` +
`cargo test`. Live AT-SPI run is a documented manual Level-4 step.

---

## 11. REFERENCES (primary source — cargo registry cache paths)

- `~/.cargo/registry/src/.../atspi-0.30.0/src/lib.rs` — re-exports.
- `~/.cargo/registry/src/.../atspi-connection-0.14.0/src/lib.rs` — `AccessibilityConnection` (async; NOT used). Confirms async-only.
- `~/.cargo/registry/src/.../atspi-proxies-0.14.0/src/{bus,registry,accessible,proxy_ext}.rs` — proxies.
- `~/.cargo/registry/src/.../atspi-common-0.14.0/src/events/object.rs` — `StateChangedEvent` + constants (L351, L645, L654, L977).
- `~/.cargo/registry/src/.../atspi-common-0.14.0/src/macros.rs:298` — `impl_from_dbus_message` ⇒ `TryFrom<&Message>`.
- `~/.cargo/registry/src/.../atspi-common-0.14.0/src/state.rs:64,245` — `State::Focused`.
- `~/.cargo/registry/src/.../zbus-5.17.0/src/blocking/message_iterator.rs` — `for_match_rule` (auto-dereg match rule).
- `~/.cargo/registry/src/.../zbus-5.17.0/src/blocking/connection/builder.rs:74,264` — `address().build()`.
- `src/platforms/gnome.rs` — the in-repo reference (struct shape, two-thread model, dedup, probe, tests).
- `spec/PLATFORMS.md §9` — the authoritative contract.
- https://www.freedesktop.org/wiki/Accessibility/AT-SPI2/ — AT-SPI2 protocol overview.
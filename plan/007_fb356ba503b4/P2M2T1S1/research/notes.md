# Research Notes — P2.M2.T1.S1 (wayland_ft.rs)

> Verified by reading the ACTUAL crate sources from `~/.cargo/registry/src` +
> extracted `.crate` archives (not memory). All API facts below are
> source-confirmed for the exact versions resolved in `Cargo.lock`:
> `smithay-client-toolkit 0.20.0`, `wayland-client 0.31.15`,
> `wayland-protocols-wlr 0.3.12`, `wayland-scanner 0.31.11`.

## §1. THE LOAD-BEARING CORRECTION (sctk 0.20 does NOT wrap the wlr protocol)

The task description + `spec/PLATFORMS.md` §7.3 both say: *"smithay-client-toolkit
(feature foreign-toplevel) provides the wlr-protocol
ForeignToplevelManager/ForeignToplevelHandler"*. **This is FACTUALLY WRONG for
sctk 0.20.** Verified directly from `sctk-0.20.0/src/lib.rs`:

- sctk 0.20 exposes `pub mod foreign_toplevel_list` (line 30 of lib.rs) — the
  **EXT** protocol (`ext-foreign-toplevel-list-v1`). There is **NO**
  `foreign_toplevel` module, no `ForeignToplevelManager`, no `ForeignToplevelHandler`,
  no `ForeignToplevelState`. (Those names are the **sctk 0.19** API, now deleted.)
- `sctk-0.20.0/src/foreign_toplevel_list.rs` imports
  `wayland_protocols::ext::foreign_toplevel_list::v1::client::*` and exposes
  `ForeignToplevelList`, `ForeignToplevelListHandler` (new_toplevel/update_toplevel/
  toplevel_closed/finished), `ForeignToplevelInfo { title, app_id, identifier }`,
  `ForeignToplevelData`, and the `delegate_foreign_toplevel_list!` macro.
- **The EXT protocol has NO activation/focus state.** `ForeignToplevelInfo` has
  only `{title, app_id, identifier}` — no `activated`. So sctk's wrapped module
  CANNOT be the focus source. This matches §7.2's own note that ext "does not
  report activation" — but it means sctk 0.20's wrapped module is the wrong tool
  for the load-bearing job.

**Consequence:** the wlr-foreign-toplevel-management-v1 protocol (the ONLY one
that reports `activated`) must be **hand-rolled** with raw `wayland-client`
`Dispatch` impls. The good news (§2): the wlr generated types are available with
**NO new Cargo dependency**.

## §2. wlr generated types — available via sctk's re-export (no new dep)

`smithay-client-toolkit 0.20.0/src/lib.rs:22` re-exports:
```rust
pub use wayland_protocols_wlr as protocols_wlr;   // version 0.3.x (lockfile: 0.3.12)
```
and `wayland-protocols-wlr` is a transitive dep (it's in `Cargo.lock`, line ~
`name = "wayland-protocols-wlr"`, version 0.3.12). sctk itself uses this exact
path shape for layer-shell (`sctk/src/shell/wlr_layer/mod.rs:15`):
`use wayland_protocols_wlr::layer_shell::v1::client::{...}`.

Therefore the wlr foreign-toplevel client types are reachable as:
```rust
use smithay_client_toolkit::reexports::protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
    zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
};
```
Module path confirmed: `wayland-protocols-wlr-0.3.12/src/lib.rs:47` declares
`pub mod foreign_toplevel { pub mod v1 { wayland_protocol!(...xml...); } }`, and
`protocol_macro.rs` generates `pub mod client` under it. **No Cargo.toml change
needed** for this task — the `wayland` feature from P2.M1.T2.S2 already pulls
sctk (which brings protocols_wlr) + wayland-client.

(Verified: the wlr XML is at `wayland-protocols-wlr-0.3.12/wlr-protocols/unstable/
wlr-foreign-toplevel-management-unstable-v1.xml`.)

## §3. The wlr protocol contract (from the XML)

Interface `zwlr_foreign_toplevel_manager_v1`:
- **event `toplevel`** — `<arg name="toplevel" type="new_id"
  interface="zwlr_foreign_toplevel_handle_v1"/>` → creates a new handle proxy.
- **request `stop`** — client tells server it's done (we don't need it).

Interface `zwlr_foreign_toplevel_handle_v1` (version 3):
- **event `title`** — `{ title: String }`
- **event `app_id`** — `{ app_id: String }`  ← this is `app_class`
- **event `state`** — `{ state: Vec<u8> }`  ← see §4 (array of u32 flags)
- **event `done`** — commit the pending title/app_id/state for this handle
- **event `closed`** — the toplevel is gone; the client should `destroy()` the proxy
- (also output_enter/output_leave/parent — irrelevant to us)

**enum `state`** (XML lines 147-157):
| name | value | meaning |
|---|---|---|
| maximized | 0 | maximized |
| minimized | 1 | minimized |
| **activated** | **2** | **the toplevel is active (focused)** |
| fullscreen | 3 | fullscreen (since v2) |

## §4. The `state` event arg is `Vec<u8>` (NOT Vec<State>) — DECODE MANUALLY

`wayland-scanner-0.31.11/src/client_gen.rs:214`:
```rust
Type::Array => if arg.allow_null { quote!{ Option<Vec<u8>> } } else { quote!{ Vec<u8> } },
```
The wlr `state` event arg is `<arg name="state" type="array"/>` (no `enum=`
attribute, not allow-null) → generates **`Event::State { state: Vec<u8> }`**.
The bytes are a packed sequence of **little-endian u32** state values. Activation
check:
```rust
let activated = state.chunks_exact(4)
    .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
    .any(|v| v == 2); // 2 == wlr enum "activated"
```
(This is the load-bearing correctness detail. Do NOT look for an `Activated`
enum variant — there isn't one in the generated code for this untyped array.)

## §5. wayland-client 0.31 connect / dispatch API (verified from source)

- `Connection::connect_to_env() -> Result<Connection, ConnectError>`
  (`conn.rs:46`). Reads `$WAYLAND_DISPLAY` (then `$WAYLAND_SOCKET`). **Err if
  unset/unresolvable** — this is probe gate #1.
- **`globals::registry_queue_init::<State>(&conn) -> Result<(GlobalList, EventQueue<State>), GlobalError>`**
  (`globals.rs:78`). Creates the queue, binds `wl_registry`, does ONE sync
  roundtrip, returns the populated `GlobalList` + the `EventQueue`. Requires
  `State: Dispatch<wl_registry::WlRegistry, GlobalListContents> + 'static`.
- `GlobalList::bind::<I, State, U>(&self, qh, version: RangeInclusive<u32>, udata) -> Result<I, BindError>`
  (`globals.rs:151`). **`Err(BindError)` if the global isn't advertised** — this
  is probe gate #2 (bind the manager; if absent, the compositor doesn't support
  wlr-foreign-toplevel).
- `GlobalList::contents() -> &GlobalListContents`; `GlobalListContents::with_list(|&[Global]| -> T)`
  (`globals.rs:322`). `Global { name: u32, interface: String, version: u32 }`.
  Interface name string to test: **`"zwlr_foreign_toplevel_manager_v1"`**.
- `EventQueue::handle() -> QueueHandle<State>` (`event_queue.rs:375`).
- `EventQueue::blocking_dispatch(&mut state) -> Result<usize, DispatchError>`
  (`event_queue.rs:398`). **The main loop primitive.** Returns `Err(DispatchError::Backend(WaylandError))`
  on compositor death/disconnect → trigger reconnect. (`DispatchError::BadMessage`
  is a protocol violation; log + continue is fine.)
- `EventQueue::roundtrip(&mut state) -> Result<usize, DispatchError>` (`event_queue.rs:420`).
- `Connection::flush() -> Result<(), WaylandError>` (`conn.rs:124`).

### The canonical State/Dispatch skeleton (from globals.rs module doc, lines 18-57)
```rust
struct State { /* ... */ }
// REQUIRED by registry_queue_init: handle (or ignore) dynamic global add/remove.
impl wayland_client::Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(&mut self, _proxy: &wl_registry::WlRegistry, _event: wl_registry::Event,
             _data: &GlobalListContents, _conn: &Connection, _qh: &QueueHandle<Self>) {}
}
// then:
let conn = Connection::connect_to_env()?;
let (globals, mut queue) = registry_queue_init::<State>(&conn)?;
let manager: ZwlrForeignToplevelManagerV1 = globals.bind(&queue.handle(), 1..=3, ())?;
```

## §6. Send/!Send (from wayland-backend, confirmed by researcher)

Under sctk's default **pure-Rust** backend (`Backend` holds `Arc<ConnectionState>`),
`Connection` / `EventQueue` / `QueueHandle` / proxies are **`Send + Sync`**. The
one caveat: enabling the `client_system` feature ANYWHERE flips them to `!Send`.
We do NOT enable it (the deps from P2.M1.T2.S2 are the pure-Rust default), so the
types are `Send`. **Design decision nonetheless: construct ALL wayland objects ON
the spawned dispatch thread** and have the monitor struct hold ONLY
`Arc<Mutex<...>>` cells + a `JoinHandle` + shutdown `Arc<AtomicBool>`. This keeps
the struct trivially `Send` (satisfying `trait WindowMonitor: Send`) and matches
PLATFORMS.md §7.3 ("monitor owns an Arc<Mutex<Option<(String,String)>>> last-state
cell shared with the event thread").

## §7. Hand-rolled Dispatch impls needed (wlr)

- `impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for State` — on
  `Event::Toplevel { toplevel }` insert a fresh `HandleInfo::default()` keyed by
  `toplevel.id()` into `State.toplevels`.
- `impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for State` — match:
  - `Title { title }` → `info.pending_title = title`
  - `AppId { app_id }` → `info.pending_app_id = app_id`
  - `State { state }` → `info.pending_activated = decode_activated(&state)`
  - `Done` → commit pending→current, then `recompute_focus_and_notify(state)`
  - `Closed` → remove from map, `handle.destroy()`, then recompute (empty-workspace)
- Per-handle user data type is `()`; ALL info is stored in
  `State.toplevels: HashMap<ObjectId, HandleInfo>` (keyed by `handle.id()`),
  avoiding wayland-client `ObjectData` complexity. (If event-created-child data
  routing needs the `wayland_client::event_created_child!` macro, mirror
  `sctk-0.20.0/src/foreign_toplevel_list.rs:125-127` — but the wlr `toplevel`
  event carries a typed new_id, so auto-creation should suffice.)

`recompute_focus_and_notify`: scan `toplevels`, find the one with
`current.activated == true`; build `(app_class, title)` (empty if none activated
→ empty-workspace); compare to the last emitted cell; if changed, update the cell
+ `notifier::notify_qmk(&WindowInfo::new(...), verbose)` + refresh the shared
list snapshot (activated first).

## §8. list_foreground_windows() — must read the RUNNING monitor's snapshot

Unlike Hyprland (whose free `list_foreground_windows()` queries IPC live), the
wlr-tracked toplevels live INSIDE the dispatch thread's `State`. So the module
publishes a snapshot the free function can read:
```rust
static SHARED_SNAPSHOT: OnceLock<Arc<Mutex<Vec<(String,String)>>>> = OnceLock::new();
```
The dispatch thread writes `(app_id, title)` pairs (activated first) on every
recompute; `wayland_ft::list_foreground_windows()` reads it (empty if no monitor
ever ran). `mod.rs::list_foreground_windows()` routes to wayland_ft when
`feature="wayland"` is on (priority #1, before hyprland).

## §9. Reconnect backoff (reuse Hyprland constants)

The constants `INITIAL_RECONNECT_MS=100`, `MAX_RECONNECT_MS=10_000`,
`STABLE_CONNECTION_THRESHOLD=5s`, factor ×3 are **private** to `hyprland.rs`
(`const` items). Per the task ("reuse Hyprland constants") we **re-declare
identical consts in `wayland_ft.rs`** (same values) — they are private to
hyprland.rs so can't be imported. The reconnect loop: on
`Err(DispatchError::Backend(_))` sleep `delay_ms`, `delay_ms = min(delay_ms*3,
MAX)`, reset to `INITIAL` if the connection stayed up ≥ STABLE, then re-run
connect+bind. Check the shutdown `Arc<AtomicBool>` at the top of each iteration.

## §10. probe_available (availability, side-effect-free)

1. `Connection::connect_to_env()` → Err(reason) if `$WAYLAND_DISPLAY`
   unresolvable. (gate 1)
2. `registry_queue_init::<ProbeState>(&conn)` → enumerates globals in one
   roundtrip. (needs a minimal `ProbeState` with the empty WlRegistry Dispatch
   impl — OR reuse a shared tiny state.)
3. `globals.contents().with_list(|g| g.iter().any(|x| x.interface ==
   "zwlr_foreign_toplevel_manager_v1"))` → if false, Err("compositor does not
   advertise zwlr_foreign_toplevel_manager_v1"). (gate 2)

Return `Ok(())` only when both pass. This is the probe `select_linux_backend`
calls (the candidate named `"foreign-toplevel"`, feature `wayland`, priority #1).

## §11. ext-foreign-toplevel cross-check — DEFERRED for v1 (functionally inert)

The contract says "bind ext if present ... cross-check app_ids". After analysis:
**ext adds ZERO functional value** — wlr already provides app_id, title,
activation, AND list_foreground_windows data. ext (per the spec itself) "does not
report activation" and "use only to populate list_foreground_windows and
cross-check app_ids" (both already covered by wlr). Wiring sctk's
`ForeignToplevelList` wrapper would force the `State` to ALSO impl
`ForeignToplevelListHandler` + delegate (a second handler mechanism beside the
hand-rolled wlr Dispatch), adding real complexity for an inert cross-check.

**Decision: defer the ext cross-check to a follow-up subtask.** Document it with
the exact sctk API to use when picked up (`ForeignToplevelList::new(&globals, &qh)`,
impl `ForeignToplevelListHandler`, `delegate_foreign_toplevel_list!(State)`). The
wlr-only backend is 100% spec-compliant for focus tracking + list + reconnect +
probe. This maximizes one-pass success. (If a reviewer insists on ext, it's a
small additive change with no behavioral effect.)

## §12. Files touched by this task (scope)

- **CREATE** `src/platforms/wayland_ft.rs` — `#![cfg(all(target_os="linux",
  feature="wayland"))]`; `WaylandFtMonitor` struct + `WindowMonitor` impl +
  `probe_available` + `list_foreground_windows` + hand-rolled wlr Dispatch impls
  + reconnect loop.
- **MODIFY** `src/platforms/linux.rs` — replace the `wayland_probe` STUB with a
  real probe delegating to `crate::platforms::wayland_ft::probe_available`; add a
  `"foreign-toplevel"` arm to `construct_backend` (the candidate NAME in
  `linux_backend_candidates()` is already `"foreign-toplevel"`).
- **MODIFY** `src/platforms/mod.rs` — declare
  `#[cfg(all(target_os="linux", feature="wayland"))] mod wayland_ft;` and route
  `list_foreground_windows()` to wayland_ft when the feature is on (before
  hyprland).
- **NO Cargo.toml change** — the `wayland` feature + deps are already there from
  P2.M1.T2.S2 (the contract). wlr types come free via sctk's `protocols_wlr`
  re-export.

## §13. Test strategy (must be hermetic — no real compositor in CI/dev box)

- Unit-test the **pure** functions: `decode_activated(&[u8])` (construct
  byte arrays with/without the value 2), `recompute_focus_from_map` (build a
  HashMap of HandleInfos, assert the chosen (app_class,title) + empty-workspace
  when none activated), list ordering (activated first).
- The Dispatch impls + connect/bind cannot be unit-tested without a compositor;
  keep the logic FACTORED into pure helpers so the helpers ARE testable (mirror
  how `hyprland.rs` tests `hyprland_socket_is_live` but not the listener).
- The `probe_available` Err paths CAN be tested by unsetting `$WAYLAND_DISPLAY`
  (same env-snapshot/restore pattern as the existing `select_tests` module in
  `linux.rs`).
- `cargo test --bin qmkonnect -- --test-threads=1` (shared debouncer state —
  Invariant 8, AGENTS.md).
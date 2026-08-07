# Research Notes — P2.M1.T1.S1: `select_linux_backend` runtime dispatcher

## Current state (verified by reading the repo)

### `src/platforms/mod.rs` — `create_monitor` is COMPILE-TIME either/or today
```rust
pub fn create_monitor(verbose: bool) -> Result<Box<dyn WindowMonitor>, Box<dyn Error>> {
    #[cfg(all(target_os = "linux", feature = "hyprland"))]
    { Ok(Box::new(hyprland::HyprlandMonitor::new(verbose))) }       // blocks start()
    #[cfg(all(target_os = "linux", not(feature = "hyprland")))]
    { Ok(Box::new(x11::X11Monitor::new(verbose))) }                  // spawn-and-return start()
    ...
}
```
- `mod x11;` is gated `#[cfg(all(target_os = "linux", not(feature = "hyprland")))]` — i.e. X11
  is ONLY compiled when the hyprland feature is OFF. This is the compile-time either/or this task
  replaces with **runtime** selection.
- `mod hyprland;` is unconditional (its own `#![cfg(all(target_os="linux", feature="hyprland"))]`
  inside the file gates the body). Both can coexist after this task.

### `WindowMonitor` trait (`mod.rs` lines ~12-27)
```rust
pub trait WindowMonitor: Send {
    fn platform_name(&self) -> &str;
    fn start(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> { Ok(()) } // default no-op
}
```
- No threading-model hint today. **The blocking-vs-spawn distinction is currently encoded in the
  runner's `#[cfg]` split**, not in the trait. Need a runtime way to express it (see Design §3).

### `src/runners/linux.rs` — two `#[cfg]` branches (must merge into ONE runtime path)
- Hyprland branch: `monitor.start()?` **blocks the calling thread** on the IPC listener; tray
  spawned first on its own thread.
- Non-Hyprland (X11) branch: monitor on a **background thread** (`thread::spawn`), main thread
  drives `tray::setup_tray` (no linux-tray) or `loop { park() }` (linux-tray).
- Today `create_monitor` is called with `?` at the top → Err would bail the whole process. The
  no-backend fallback must NOT bail.

### Backend threading models (PRD §11 — verified against source)
| Backend | `start()` | Compiled today? |
|---|---|---|
| Hyprland | **blocks calling thread** (own reconnect loop) | yes (feature `hyprland`) |
| X11 | spawn-and-return (`thread::spawn` @ x11.rs:136) | yes (cfg `not(hyprland)`) |
| foreign-toplevel | spawn-and-return (will spawn EventQueue thread) | NO (P2.M2) |
| GNOME | spawn-and-return (zbus on bg thread) | NO (P2.M3) |
| AT-SPI | spawn-and-return (bg thread) | NO (P2.M4) |

⇒ Only Hyprland blocks; all current + future others spawn-and-return. A trait default of
`false` with Hyprland overriding to `true` is the clean, forward-compatible hint.

### `src/platforms/hyprland.rs` — reusable probe primitives (lines 285-368)
- `hyprland_socket_is_live(path: &Path) -> bool` — hermetic `connect(2)` liveness probe (thread +
  `recv_timeout`, ~500ms cap). Has 3 passing tests using `tempfile::TempDir` + `UnixListener`.
- `check_hyprland_environment() -> Result<(), Box<dyn Error>>` — resolves
  `$XDG_RUNTIME_DIR/hypr/<sig>/.socket.sock` for `$HYPRLAND_INSTANCE_SIGNATURE` (+ recovery scan),
  returns Ok iff a live socket accepts a connection. **This IS the Hyprland availability probe** —
  but it's currently private. Expose a `pub(crate) fn probe_available() -> Result<(), String>`
  thin wrapper, or make `check_hyprland_environment` `pub(crate)`.

### `src/core/mod.rs` — Config has NO `[linux]` table yet
```rust
pub struct Config { vendor_id, product_id, usage_page, usage, debounce_ms, poll_interval_ms }
```
- Loaded via `cached_config() -> Result<Config, Box<dyn Error>>` / `parse_config(path)`.
- `LinuxConfig` + `[linux] backend` is **P2.M1.T2.S1** (a LATER sibling task). So THIS task cannot
  read `[linux] backend` from Config. ⇒ **`select_linux_backend` takes the override as a PARAMETER**
  (`forced: Option<&str>`); `create_monitor` passes `None` now; T2.S1 wires it from Config.

### Cargo features today (`Cargo.toml`)
- `default = ["hyprland", "macos", "linux-tray"]`.
- `wayland`, `gnome`, `atspi` features **DO NOT EXIST** (added in P2.M1.T2.S2).
- X11 has no feature flag today (gated only by `cfg(not(feature="hyprland"))`); PRD makes it
  **unconditional on Linux** (P2.M5.T1).

## Spec (verified verbatim — `prd_snapshot.md`)

### Priority order (PLATFORMS.md §6, snapshot lines 2012-2055)
| # | Backend | Feature | Availability probe |
|---|---|---|---|
| 1 | foreign-toplevel | `wayland` | `$WAYLAND_DISPLAY` resolvable AND `zwlr_foreign_toplevel_manager_v1` global advertised |
| 2 | GNOME | `gnome` | D-Bus name `io.mulletware.QMKonnect` owned on session bus |
| 3 | Hyprland | `hyprland` | `$HYPRLAND_INSTANCE_SIGNATURE` + a live socket |
| 4 | AT-SPI | `atspi` | a11y bus reachable (`org.a11y.Bus` owned OR `$ATSPI_BUS_ADDRESS`) |
| 5 | X11 | always | `$DISPLAY` set AND `$WAYLAND_DISPLAY` UNSET AND `xprop` present |

### Behaviors mandated (PLATFORMS.md §6)
- `[linux] backend` override (default `auto`): a forced backend that's unavailable **errors loudly
  with EVERY probe result**; `auto` = first-available.
- Verbose mode: print each candidate, its probe result, the chosen backend.
- **No-backend fallback**: if every probe fails → returns `Err`; the **runner still starts the
  tray + device-status poll + HID pipeline** (app not useless), emits no window events. **GNOME
  one-shot `notify-send`** fires here (§8.4: `$XDG_CURRENT_DESKTOP` contains `GNOME` AND name not
  owned → notification pointing at the extension; at most once per launch).

### Invariant 11 (ARCHITECTURE.md §10) — the headline correctness gate
> Never select the X11 backend under a Wayland compositor. `select_linux_backend` gates X11 on
> `$WAYLAND_DISPLAY` being **unset**; XWayland sets `$DISPLAY` but reports focus unreliably.

### §2.2 (ARCHITECTURE.md) — runner treats backend uniformly
> The runner then treats the chosen backend uniformly as a `Box<dyn WindowMonitor>`; the
> blocking-vs-spawn distinction is handled per-backend.

### §8 Error model
- Traits return `Result<(), Box<dyn std::error::Error>>`.
- **Fail-loud** examples include X11 (xprop missing), Hyprland (socket absent) — but the
  no-backend case is **fail-soft at the runner** (the dispatcher returns Err, the runner does NOT
  restart-loop; it keeps the tray alive).
- Logging: `eprintln!`/`println!`; verbose uses process-local monotonic `core::now_ms()`, not
  wall-clock.

## Design decisions (locked)

1. **Candidate framework.** `select_linux_backend` builds a priority-ordered list of
   `(name, probe_fn)` candidates, feature-gated per backend. Probes return
   `Result<(), String>` (Ok = available; Err(reason) = why not). Construction is a separate
   feature-gated `match` on the chosen name returning `Box<dyn WindowMonitor>`. Rationale: cleanly
   extends; no closure-of-unwritten-types; the construction match only has arms for backends that
   actually exist + a catch-all Err for not-yet-wired stubs.

2. **wayland/gnome/atspi rows are feature-gated stubs.** Today those features are undefined ⇒ the
   `#[cfg(feature = "...")]` rows are NOT compiled ⇒ no breakage. When P2.M1.T2.S2 adds the
   features, the stub probes return `Err("not yet implemented (P2.Mx)")` so the dispatcher skips
   them and falls through to hyprland/x11. Each backend task (P2.M2/M3/M4) REPLACES its stub probe
   with a real one AND adds its construction match arm. **Catch-all construction arm prevents a
   not-yet-wired backend from constructing garbage even if its probe later returns Ok.**

3. **`hyprland_probe` + `x11_probe` are REAL today** (the backends that exist). hyprland reuses
   `check_hyprland_environment`/`hyprland_socket_is_live`. x11 checks the three-way gate
   (`$DISPLAY` set, `$WAYLAND_DISPLAY` unset, `xprop -version` succeeds).

4. **Threading-model hint via trait method.** Add
   `fn start_blocks_calling_thread(&self) -> bool { false }` to `WindowMonitor` (default false =
   spawn-and-return; Hyprland overrides `true`). Lets the runner unify into ONE Linux path and
   decide at runtime. Default `false` matches every current+future backend except Hyprland.

5. **Override plumbing is deferred to T2.S1.** `select_linux_backend(verbose, forced: Option<&str>)`;
   `create_monitor` passes `None` today with a `// TODO(P2.M1.T2.S1)` comment. Avoids a dead
   function and keeps create_monitor stable.

6. **Runner: one merged Linux path** that (a) spawns tray, (b) on `Ok(monitor)` either blocks on
   `start()` (if `start_blocks_calling_thread()`) or spawns it + parks/drives tray, (c) on `Err`
   keeps tray + device pipeline + HID alive, fires GNOME one-shot notify, parks main.

7. **cfg gate changes:** `mod x11;` → `#[cfg(target_os = "linux")]` (unconditional on Linux).
   `mod hyprland;` stays feature-gated. The runner drops its two `#[cfg]` branches.

## Key risks / coordination notes
- **tray.rs cfg coupling:** `tray.rs` is compiled for
  `cfg(not(all(target_os="linux", feature="hyprland")))`. When hyprland feature is ON, `tray.rs`
  (Win32/NSAlert setup_tray) is absent on Linux — so the non-linux-tray fallback must NOT call
  `crate::tray::setup_tray` under hyprland+linux. The merged runner must respect this: the
  `#[cfg(not(feature="linux-tray"))] crate::tray::setup_tray(...)` call is only valid when tray.rs
  exists, i.e. also gate on `not(feature="hyprland")`. Verify with a grep gate.
- **No breakage at T2.S2:** the feature-gated stub rows must be SELF-CONTAINED (not reference
  `wayland_ft::WaylandMonitor` etc.) so that adding the features in P2.M1.T2.S2 does not break the
  build before P2.M2/M3/M4 land. Use inline `Err` probe stubs + the catch-all construction arm.
- **Single-threaded tests** (Invariant 8): any new tests must run `--test-threads=1`.
- `platforms::notify(title, body)` (mod.rs) already shells to `notify-send` on Linux — reuse it
  for the GNOME first-run notification (do NOT spawn notify-send ad hoc).
- `linux_tray::spawn(verbose) -> Option<ksni::blocking::Handle<QmkTray>>` — keep the handle alive
  for process lifetime (drop = tray disappears).
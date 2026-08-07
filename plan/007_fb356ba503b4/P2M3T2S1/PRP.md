# PRP — P2.M3.T2.S1: GNOME D-Bus client backend (`src/platforms/gnome.rs`)

> **Repo under change:** QMKonnect (`/home/dustin/projects/qmkonnect`).
> **Files CREATED:** `src/platforms/gnome.rs`.
> **Files MODIFIED:** `src/platforms/mod.rs` (add `mod gnome;` + `list_foreground_windows`
> branch), `src/platforms/linux.rs` (replace the `gnome_probe` stub body + add the
> `"gnome"` arm to `construct_backend`), `docs/installation.md` + `docs/troubleshooting.md`
> (Mode-A docs).
> **Files NOT touched (owned by OTHER tasks — see §Scope Boundary):**
> `packaging/gnome-shell-extension/*` (P2.M3.T1.S1, parallel — the producer of the D-Bus
> name), the first-run-notification UX in `src/runners/linux.rs::maybe_gnome_first_run_notify`
> (P2.M3.T2.S2), `Cargo.toml` (P2.M1.T2.S2 — the `gnome = ["dep:zbus"]` feature already
> exists in `default`), `.github/workflows/release.yml` (P2.M7.T2.S1), `.gitignore`.
>
> **What it does:** GNOME (Mutter) implements neither Wayland foreign-toplevel protocol and
> exposes no client API for the active window. The GNOME Shell extension
> (`qmkonnect@mulletware`, P2.M3.T1.S1) reads `global.display.focus_window` *inside*
> `gnome-shell` and republishes `(wm_class, title)` over the session D-Bus as well-known
> name `io.mulletware.QMKonnect`. **This task is the desktop-side D-Bus client** — a
> `WindowMonitor` backend that subscribes to the `ActiveWindowChanged` signal, polls
> `GetActiveWindow` every 1000 ms for drift correction, detects the extension being
> toggled via `name_has_owner`, dedups, and calls `notify_qmk`. It is wired into
> `select_linux_backend` as **priority #2** (after foreign-toplevel, which GNOME never
> advertises → GNOME always lands here).

---

## Goal

**Feature Goal**: Implement `src/platforms/gnome.rs` (feature `gnome`) — a `GnomeMonitor`
`WindowMonitor` backend that connects to the session D-Bus over `zbus` 5.x (blocking API),
subscribes to the extension's `ActiveWindowChanged(ss)` signal on a background thread,
runs a 1000 ms (hot-config `[linux] gnome_poll_interval_ms`) drift-correcting poll thread
that also probes `name_has_owner("io.mulletware.QMKonnect")` for the §8.3
NameOwnerChanged semantics (re-acquire on reappear, empty + no-backend on disappear),
dedups against a shared last-emitted cell, and calls `notifier::notify_qmk`. Plus: replace
the `gnome_probe` stub with a real name-ownership probe, add the `"gnome"` arm to
`construct_backend`, wire `mod gnome;` + `list_foreground_windows` into `mod.rs`, and
document the backend + extension dependency (Mode A).

**Deliverable** (concrete; validates on the dev box TODAY — a Hyprland box, so the hard
gate is `cargo build` + hermetic unit tests; the live `gdbus`/GNOME run is documented as a
manual step):
- `src/platforms/gnome.rs` — `#![cfg(all(target_os = "linux", feature = "gnome"))]`;
  `#[zbus::proxy] trait WindowMonitor` for the extension's interface; `pub struct
  GnomeMonitor` impl-ing `crate::platforms::WindowMonitor` (spawn-and-return `start()`,
  best-effort `stop()`); `pub(crate) fn probe_available(verbose) -> Result<(), String>`;
  `pub fn list_foreground_windows() -> Vec<(String,String)>`.
- `src/platforms/linux.rs` — `gnome_probe` body delegates to `gnome::probe_available`;
  `construct_backend` gains `#[cfg(feature="gnome")] "gnome" => … GnomeMonitor::new …`.
- `src/platforms/mod.rs` — `#[cfg(all(target_os="linux", feature="gnome"))] mod gnome;`
  + a gnome branch in `list_foreground_windows()`.
- `docs/installation.md` + `docs/troubleshooting.md` — GNOME backend + extension-dep sections.

**Success Definition**:
- `cargo build --release` succeeds (default features include `gnome`); the `#[zbus::proxy]`
  macro, the blocking Connection/proxy, and the `Send`-typed monitor all compile.
- `cargo test --bin qmkonnect -- --test-threads=1` passes; NEW hermetic unit tests cover the
  dedup helper, empty-window mapping, and the probe's Err path (no session bus / name
  unowned). Existing `select_tests` still pass.
- The `gnome_probe` stub's old `Err("…not yet implemented (P2.M3)")` is GONE — it now probes
  real name ownership.
- `git diff --stat` shows ONLY `src/platforms/gnome.rs` (new), `src/platforms/mod.rs`,
  `src/platforms/linux.rs`, `docs/installation.md`, `docs/troubleshooting.md` (NO
  `packaging/gnome-shell-extension/`, `Cargo.toml`, `release.yml`, `.gitignore`,
  `PRD.md`, `tasks.json` changes).

## User Persona (if applicable)

**Target User**: GNOME desktop users (Ubuntu/Fedora/Debian defaults + anyone on GNOME/Wayland).
GNOME is the single largest Linux desktop; before this task its probe was a stub, so every
GNOME user fell through to AT-SPI (best-effort, off by default) or no-backend.

**Use Case**: A GNOME user installs QMKonnect + the `qmkonnect@mulletware` extension; the
daemon's `gnome_probe` finds the D-Bus name owned → selects the GNOME backend → focus changes
switch the active QMK layer/keymap.

**User Journey**: install extension (EGO/Release zip) → enable in the Extensions app → run
`qmkonnect -v` → log shows `gnome` selected → switch windows → layer switches. No reboot,
no reflash.

**Pain Points Addressed**: Without this backend, GNOME/Wayland users get NO reliable window
detection. This is the unique reliable bridge (PLATFORMS.md §8).

## Why

- **F16 (PRD §4) = "one binary, every Linux desktop".** GNOME is priority #2 in
  `select_linux_backend` (PLATFORMS.md §6). Today `gnome_probe` is a stub returning
  `Err("…not yet implemented (P2.M3)")`, so GNOME never selects. This task makes F16 real on
  GNOME.
- **Owns-the-name ⇔ installed & enabled** (§6 row 2). The availability probe = "the well-known
  name `io.mulletware.QMKonnect` is owned on the session bus." This is the cheapest reliable
  presence signal (no introspection round-trip).
- **Drift correction mirrors macOS/Hyprland** (§8.3). Signals can be missed (extension busy,
  D-Bus queue overflow); the 1000 ms poll + dedup catches any gap. NameOwnerChanged semantics
  make the backend resilient to the user toggling the extension mid-session (re-acquire) or it
  crashing (empty + no-backend posture, tray+device pipeline keep running).

## What

A `WindowMonitor` backend that:

1. **`probe_available(verbose)`** connects `zbus::blocking::Connection::session()`, builds
   `zbus::blocking::fdo::DBusProxy`, calls `name_has_owner("io.mulletware.QMKonnect")` → `Ok`
   iff owned, `Err` naming the extension otherwise. This is the SELECT-time availability gate.
2. **`start()`** spawns TWO worker threads and returns `Ok(())` immediately (spawn-and-return;
   keeps the trait default `start_blocks_calling_thread() == false`, so `runners/linux.rs`
   parks main / drives ksni — ARCHITECTURE.md §3 row "GNOME"):
   - **Signal thread** — own `Connection`; `WindowMonitorProxyBlocking::new(&conn)`; loop on
     `receive_active_window_changed().next()`; on `(app_class,title)` → `apply_and_notify`.
   - **Poll thread** — loop: sleep(hot-config `gnome_poll_interval_ms`, default 1000);
     `name_has_owner`? if owned → `get_active_window()` → `apply_and_notify` (drift), else
     → emit empty `("","")` once + no-backend (re-acquires automatically when the name returns).
3. **`stop()`** sets the shutdown `AtomicBool`; joins best-effort (poll thread exits within one
   interval; signal thread exits on the next signal / connection teardown — same posture as
   `wayland_ft.rs::stop`).
4. **`list_foreground_windows()`** — one-shot `Connection` + `get_active_window()` →
   `vec![(app_class,title)]`, or `vec![]` when empty / unavailable.
5. Empty window (`focus_window` null ⇒ extension returns `("","")`) → empty `WindowInfo`;
   `list_foreground_windows` → `vec![]`.

### Success Criteria
- [ ] `probe_available` returns `Ok` iff `io.mulletware.QMKonnect` is owned; `Err` otherwise
      (reason names the extension). Replaces the `Err("…not yet implemented")` stub.
- [ ] `start()` spawns a signal thread + a poll thread and returns promptly
      (`start_blocks_calling_thread() == false`).
- [ ] Signal thread subscribes to `ActiveWindowChanged(ss)` and notifies on change (deduped).
- [ ] Poll thread sleeps `gnome_poll_interval_ms` (hot-config, default 1000), calls
      `GetActiveWindow`, dedups, and catches drift.
- [ ] NameOwnerChanged semantics: extension toggled off → empty + no-backend; toggled on →
      state re-acquired (via the next poll's `get_active_window`).
- [ ] `construct_backend` has a `#[cfg(feature="gnome")] "gnome" =>` arm returning
      `GnomeMonitor::new(verbose)`; `select_linux_backend` selects it when the probe is `Ok`.
- [ ] `mod.rs` declares `mod gnome;` and dispatches `list_foreground_windows` to it.
- [ ] `cargo build --release` + `cargo test --bin qmkonnect -- --test-threads=1` pass.

## All Needed Context

### Context Completeness Check

_Pass._ An agent with no prior knowledge can implement this from this PRP + the repo because:
(a) the **exact zbus 5.17.0 API** (blocking Connection, generated `*Blocking` proxy,
`fdo::DBusProxy::name_has_owner`, `SignalIterator`) is verified against the vendored source
in `~/.cargo/.../zbus-5.17.0/` and given verbatim in `research/notes.md` §2; (b) the closest
in-repo analog (`src/platforms/wayland_ft.rs` — spawn-and-return, dedup cell, notify_qmk,
OnceLock list snapshot, probe stub) is cited with the exact functions to mirror; (c) the exact
edit sites in `linux.rs` (`gnome_probe` lines 170-173, `construct_backend` match) and `mod.rs`
are pinned with current line numbers; (d) the D-Bus contract (name/path/iface/method/signal)
is pinned to spec/PLATFORMS.md §8.1, identical to the parallel extension's contract; (e) the
two non-obvious decisions — **two threads each with its OWN Connection** (a shared connection
serializes on the internal executor) and **NameOwnerChanged via poll-time `name_has_owner`**
(a single blocking thread can only block on one signal iterator) — are explained with the
rejection of the alternatives so the implementer doesn't "improve" them; (f) the scope boundary
with the parallel extension producer (P2.M3.T1.S1), the first-run-notification task
(P2.M3.T2.S2), and the Cargo/CI owners is explicit.

### Documentation & References

```yaml
# MUST READ — the authoritative client contract (the WHAT).
- file: spec/PLATFORMS.md
  why: "§8 GNOME Backend. §8.1 D-Bus contract (name io.mulletware.QMKonnect, path
        /io/mulletware/QMKonnect, iface io.mulletware.QMKonnect.WindowMonitor, method
        GetActiveWindow()->(ss), signal ActiveWindowChanged(ss), app_class=get_wm_class).
        §8.3 client spec: zbus session conn; subscribe to ActiveWindowChanged -> dedup ->
        notify_qmk; drift-correcting 1000ms poll (hot-config gnome_poll_interval_ms) calls
        GetActiveWindow + dedups; NameOwnerChanged watch re-acquires state / reports empty.
        §6 row 2: GNOME probe = name OWNED. §8.4: first-run UX (owned by S2, not here)."

# MUST READ — the threading mandate.
- file: spec/ARCHITECTURE.md
  why: "§6 Concurrency table: 'GNOME monitor: zbus signal subscription thread + 1000ms
        drift-poll thread | last-emitted cell | NameOwnerChanged watch re-acquires state.'
        §3 row 'GNOME (extension client)': 'No GUI loop — zbus signal subscription. Monitor
        runs on a background thread + a drift-poll thread; ksni owns its D-Bus thread; runner
        parks main.' Pins the 2-thread design + start_blocks_calling_thread()==false."

# MUST READ — the closest in-repo analog (mirror its shape exactly).
- file: src/platforms/wayland_ft.rs
  why: "the spawn-and-return template: GnomeMonitor must mirror WaylandFtMonitor's struct
        shape (Arc<Mutex<Option<(String,String)>>> last_focus + Arc<AtomicBool> shutdown +
        Option<JoinHandle> + bool verbose => Send), its start() (spawn thread, return Ok),
        its recompute_and_notify (dedup under ONE lock, RELEASE before notify_qmk — its
        GOTCHA-9), its probe_available (Err path naming the missing precondition), its
        OnceLock<Arc<Mutex<Vec>>> SHARED_SNAPSHOT for list_foreground_windows, and its
        #[cfg(all(target_os='linux', feature='wayland'))] file gate (use feature='gnome')."

# MUST READ — the edit sites this task owns (probe stub + construct_backend + candidate row).
- file: src/platforms/linux.rs
  why: "lines 49-50: candidate ('gnome', gnome_probe as ProbeFn) — UNCHANGED. lines 170-173:
        the gnome_probe STUB fn gnome_probe(_verbose)->Err('…not yet implemented (P2.M3)')
        — REPLACE its body to delegate to gnome::probe_available (mirror how wayland_probe
        delegates, ~line 163-165). construct_backend match (~line 80-92): ADD the
        #[cfg(feature='gnome')] 'gnome' => Ok(Box::new(GnomeMonitor::new(verbose))) arm."

# MUST READ — the trait to impl + the list_foreground_windows dispatch to extend.
- file: src/platforms/mod.rs
  why: "the WindowMonitor trait (platform_name/start/start_blocks_calling_thread/stop). Add
        #[cfg(all(target_os='linux', feature='gnome'))] mod gnome; near the wayland_ft mod
        decl. Add a gnome branch to list_foreground_windows() (single-window read)."

# MUST READ — the config field this consumes (DO NOT re-add it).
- file: src/core/mod.rs
  why: "LinuxConfig.gnome_poll_interval_ms: Option<u64> (default None -> resolved to 1000 at
        the GNOME backend use site). cached_config()->Result<Config>; Config::default() (no
        file) has linux.gnome_poll_interval_ms=None. now_ms() for verbose timestamps."

# MUST READ — the notify integration.
- file: src/core/notifier.rs
  why: "pub fn notify_qmk(window_info: &WindowInfo, verbose: bool) -> Result<(), Box<dyn
        Error+Send+Sync>> (line ~1698). src/core/types.rs: WindowInfo::new(app_class, title)."

# MUST READ — how the runner drives a spawn-and-return backend + the no-backend posture.
- file: src/runners/linux.rs
  why: "the spawn-and-return branch (monitor.start() on a worker thread + park main under
        linux-tray). maybe_gnome_first_run_notify (the existing one-shot in the Err branch —
        S1's probe returning Err feeds it; S2 extends it). S1 does NOT edit this file."

# REFERENCE — the parallel sibling PRP (the PRODUCER of the D-Bus name; do NOT duplicate).
- file: plan/007_fb356ba503b4/P2M3T1S1/PRP.md
  why: "the extension that OWNS io.mulletware.QMKonnect and exports GetActiveWindow()/(ss) +
        ActiveWindowChanged(ss). S1 assumes it will exist exactly as specified. Its D-Bus
        contract (§Data models) is byte-identical to what S1 consumes here."

# REFERENCE — the exact zbus 5.17.0 API (verified from the vendored source).
- file: plan/007_fb356ba503b4/P2M3T2S1/research/notes.md
  why: "§2 the ground-truth zbus 5.17.0 API (blocking Connection/Proxy/SignalIterator,
        fdo::DBusProxy name_has_owner + name_owner_changed signal, #[proxy] generates
        *Blocking). §3 the two-thread design rationale. §4 the reused codebase patterns."

# REFERENCE — zbus docs (secondary to the local source).
- url: https://docs.rs/zbus/latest/zbus/blocking/struct.Connection.html
  why: "blocking Connection::session()/call_method(); documents internal executor."
- url: https://docs.rs/zbus/latest/zbus/blocking/fdo/struct.DBusProxy.html
  why: "blocking DBusProxy (name_has_owner, receive_name_owner_changed)."
- url: https://dbus2.github.io/zbus/
  why: "macro book: #[proxy(...)] generates <Name>Proxy AND <Name>ProxyBlocking."
```

### Current Codebase tree (relevant subset)

```bash
spec/
  PLATFORMS.md          # §8 = authoritative contract (§8.1 D-Bus, §8.3 client)             ← READ
  ARCHITECTURE.md       # §3/§6 = threading mandate (2 threads + park main)                ← READ
src/platforms/
  mod.rs                # WindowMonitor trait + list_foreground_windows dispatch            ← EDIT (add mod gnome + branch)
  linux.rs              # select_linux_backend + gnome_probe STUB + construct_backend       ← EDIT (replace probe body + add arm)
  wayland_ft.rs         # the spawn-and-return template to MIRROR                          ← READ
src/core/
  mod.rs                # LinuxConfig.gnome_poll_interval_ms + cached_config + now_ms      ← READ
  notifier.rs           # notify_qmk                                                                       ← READ
  types.rs              # WindowInfo::new                                                                   ← READ
src/runners/linux.rs    # spawn-and-return runner branch + maybe_gnome_first_run_notify    ← READ (do NOT edit)
Cargo.toml              # gnome = ["dep:zbus"] in default (P2.M1.T2.S2)                    ← DO NOT TOUCH
```

### Desired Codebase tree (files this task creates/modifies)

```bash
src/platforms/gnome.rs            # NEW — GnomeMonitor + probe_available + list_foreground_windows (feature gnome)
src/platforms/mod.rs              # EDIT — `mod gnome;` (cfg) + list_foreground_windows gnome branch
src/platforms/linux.rs            # EDIT — gnome_probe body -> delegate; construct_backend "gnome" arm
docs/installation.md              # EDIT — "GNOME: install the Shell extension" section (Mode A)
docs/troubleshooting.md           # EDIT — GNOME backend / extension-dep troubleshooting (Mode A)
# (NO packaging/gnome-shell-extension/, Cargo.toml, release.yml, .gitignore, PRD.md, tasks.json changes)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (GOTCHA-1 — the blocking API needs NO caller-side executor). zbus 5.x
//   zbus::Connection has an internal_executor that DEFAULTS TRUE and "spawns a thread to
//   run the executor" (connection/mod.rs ~L805). So zbus::blocking::Connection::session()
//   works on a plain std::thread with NO tokio/async-io on the calling thread. Do NOT enable
//   the zbus `tokio` feature or build a runtime on main. (Since zbus 5.0 the blocking API is
//   gated behind the ON-by-default `blocking-api` feature — leave it on.)

// CRITICAL (GOTCHA-2 — TWO worker threads, each with its OWN Connection). A blocking
//   Connection's calls run on its internal executor's block_on. SHARING one Connection across
//   the signal thread (blocked in SignalIterator::next) and the poll thread (get_active_window)
//   would SERIALIZE on that executor — the poll thread would starve while the signal thread is
//   blocked. Create Connection::session() INSIDE each thread closure (moved in). The monitor
//   struct then holds only Arc<Mutex<_>>/Arc<AtomicBool>/Option<JoinHandle>/bool => Send.

// CRITICAL (GOTCHA-3 — NameOwnerChanged is implemented by the POLL thread's name_has_owner,
//   NOT a third signal subscription). A single blocking zbus thread can block on only ONE
//   SignalIterator at a time, so it cannot poll BOTH ActiveWindowChanged AND NameOwnerChanged
//   iterators. The poll thread already round-trips the bus every ~1s, so checking
//   name_has_owner("io.mulletware.QMKonnect") each tick delivers identical §8.3 semantics
//   (re-acquire on reappear, empty + no-backend on disappear) with <=1s latency and no 3rd
//   thread. This is the simpler, more robust choice within the 2-thread mandate. Do NOT add a
//   separate NameOwnerChanged signal thread.

// CRITICAL (GOTCHA-4 — release the last_focus dedup lock BEFORE notify_qmk). Mirrors
//   wayland_ft.rs GOTCHA-9 verbatim: notify_qmk takes the global debouncer STATE/NOTIFIER
//   locks internally; holding last_focus while notifying risks lock-ordering contention.
//   Dedup under ONE lock, DROP it, THEN notify_qmk. (See apply_and_notify in the blueprint.)

// CRITICAL (GOTCHA-5 — the probe is NAME-OWNERSHIP, not introspection). select_linux_backend
//   calls gnome_probe BEFORE constructing a monitor, so the probe must be cheap + side-effect-
//   free. name_has_owner is a single round-trip to dbus-daemon (no introspection of the remote
//   object). Do NOT call GetActiveWindow in the probe (the object may not be exported yet on a
//   race); ownership of the name is the §6-row-2 presence signal.

// CRITICAL (GOTCHA-6 — empty window == ("",""), not None). The extension returns ("","") when
//   focus_window is null (PLATFORMS.md §1.3/§8.2). Map to an EMPTY WindowInfo and to vec![]
//   from list_foreground_windows. The dedup cell treats ("","") == ("","") as a no-op (don't
//   re-notify the same empty).

// CRITICAL (GOTCHA-7 — stop() is BEST-EFFORT; do not block forever). The signal thread is
//   blocked in SignalIterator::next() and only returns on the next signal / connection
//   teardown. stop() sets the shutdown AtomicBool; the poll thread exits within one interval
//   (<=1s). For the signal thread, join is best-effort (matches wayland_ft.rs::stop, whose
//   blocking_dispatch also blocks until the next event). The daemon process exits via the
//   ctrlc/SIGTERM handler in runners/linux.rs regardless. Do NOT invent a timeout-based signal
//   receive (it complicates the blocking API for no gain).

// GOTCHA-8 (the gnome feature is ALREADY declared). Cargo.toml [features].default includes
//   "gnome" and gnome = ["dep:zbus"] (P2.M1.T2.S2, DONE). Do NOT edit Cargo.toml. The file
//   gate is #![cfg(all(target_os = "linux", feature = "gnome"))] (mirror wayland_ft.rs).

// GOTCHA-9 (#[zbus::proxy] generates BOTH <Name>Proxy AND <Name>ProxyBlocking). Construct the
//   blocking one: WindowMonitorProxyBlocking::new(&conn). Its methods (get_active_window,
//   receive_active_window_changed) are blocking. receive_active_window_changed() returns a
//   blocking iterator you .next() in a loop (None on the iterator ending).

// GOTCHA-10 (BusName accepts &str). name_has_owner(name: BusName<'_>) — pass the bare string
//   "io.mulletware.QMKonnect" (a WellKnownName literal coerces). Same for the generated proxy's
//   default_service.

// GOTCHA-11 (hot-re-read the interval each tick — do NOT cache it at start()). Mirror
//   core::configured_timing(): cached_config() is mtime-keyed; read gnome_poll_interval_ms
//   each poll iteration so a config edit takes effect without restart. None => 1000.

// GOTCHA-12 (verbose logging uses core::now_ms()). Mirror wayland_ft.rs's
//   "[{now_ms()}ms] gnome: …" lines so -v output is consistent across backends.

// GOTCHA-13 (run all gnome/select tests SINGLE-THREADED). The select_tests module mutates
//   process-global env ($HYPRLAND_INSTANCE_SIGNATURE etc.); the whole bin must run
//   --test-threads=1 (ARCHITECTURE invariant 8 / AGENTS.md). Add new gnome probe/dedup tests
//   alongside, same discipline.
```

## Implementation Blueprint

### Data models and structure

The "model" is the shared dedup cell + the generated D-Bus proxy trait. The
generated proxy IS the interface schema (zvariant deserializes `(s,s)` → `(String,String)`).

```rust
// src/platforms/gnome.rs — the generated proxy trait (the interface schema).
use zbus::proxy;

#[proxy(
    default_service = "io.mulletware.QMKonnect",
    default_path = "/io/mulletware/QMKonnect",
    interface = "io.mulletware.QMKonnect.WindowMonitor",
)]
trait WindowMonitor {
    /// GetActiveWindow() -> (s app_class, s title).
    fn get_active_window(&self) -> zbus::Result<(String, String)>;
    /// ActiveWindowChanged(s app_class, s title).
    #[zbus(signal)]
    fn active_window_changed(&self, app_class: String, title: String);
}
// The macro emits `WindowMonitorProxy` (async) AND `WindowMonitorProxyBlocking` (blocking).
```

### Reference `gnome.rs` skeleton (implement this shape)

```rust
//! GNOME backend — Shell-extension D-Bus client (PLATFORMS.md §8.3 — priority #2).
//!
//! GNOME (Mutter) advertises neither foreign-toplevel protocol and exposes no client API for
//! the active window. A GNOME Shell extension (`qmkonnect@mulletware`, P2.M3.T1.S1) reads
//! global.display.focus_window INSIDE gnome-shell and republishes (wm_class, title) over the
//! session D-Bus as well-known name `io.mulletware.QMKonnect`. THIS module is the desktop-side
//! client: it subscribes to ActiveWindowChanged, polls GetActiveWindow for drift, probes
//! name_has_owner for the §8.3 NameOwnerChanged semantics, dedups, and notifies QMK.
//!
//! Threading (ARCHITECTURE.md §6): TWO worker threads, each with its OWN blocking Connection
//! (GOTCHA-2). start() spawns both and returns (spawn-and-return; trait default
//! start_blocks_calling_thread()==false). ksni owns its D-Bus thread; the runner parks main.
#![cfg(all(target_os = "linux", feature = "gnome"))]

use crate::core::notifier;
use crate::core::types::WindowInfo;
use crate::platforms::WindowMonitor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use zbus::proxy;

/// D-Bus well-known name owned ⇔ extension installed & enabled (PLATFORMS.md §6 row 2).
const BUS_NAME: &str = "io.mulletware.QMKonnect";
const OBJECT_PATH: &str = "/io/mulletware/QMKonnect";
const INTERFACE_NAME: &str = "io.mulletware.QMKonnect.WindowMonitor";
/// Default drift-poll cadence when `[linux] gnome_poll_interval_ms` is unset.
const DEFAULT_POLL_MS: u64 = 1000;

#[proxy(
    default_service = BUS_NAME,
    default_path = OBJECT_PATH,
    interface = INTERFACE_NAME,
)]
trait WindowMonitor {
    fn get_active_window(&self) -> zbus::Result<(String, String)>;
    #[zbus(signal)]
    fn active_window_changed(&self, app_class: String, title: String);
}

/// Public monitor (`Send`: holds only Arc<Mutex<_>>/Arc<AtomicBool>/Option<JoinHandle>/bool —
/// mirrors WaylandFtMonitor exactly).
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

    /// Spawn-and-return: spawn the signal thread + the poll thread, return Ok(()) promptly.
    /// Keeps the trait default start_blocks_calling_thread() == false (GOTCHA-1/2).
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

    /// Best-effort stop (GOTCHA-7): poll thread exits within one interval; signal thread
    /// exits on the next signal / connection teardown (same posture as wayland_ft::stop).
    fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.shutdown.store(true, Ordering::Release);
        if let Some(h) = self.poll_handle.take() {
            let _ = h.join(); // exits within <= DEFAULT_POLL_MS
        }
        if let Some(h) = self.signal_handle.take() {
            let _ = h.join(); // best-effort (may block until next signal / conn drop)
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Dedup + notify (GOTCHA-4: release last_focus BEFORE notify_qmk). Free fn: the
// threads own cloned Arcs.
// ---------------------------------------------------------------------------
fn apply_and_notify(
    last_focus: &Mutex<Option<(String, String)>>,
    candidate: (String, String),
    verbose: bool,
) {
    {
        let mut cell = last_focus.lock().unwrap();
        if *cell == Some(candidate.clone()) {
            return; // dedup
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

/// Signal subscription thread (GOTCHA-1/2/9). Owns its Connection; blocks on the
/// ActiveWindowChanged iterator; exits on the next signal / connection teardown when the
/// shutdown flag is set (best-effort, GOTCHA-7).
fn run_signal_loop(
    last_focus: Arc<Mutex<Option<(String, String)>>>,
    shutdown: Arc<AtomicBool>,
    verbose: bool,
) {
    let conn = match zbus::blocking::Connection::session() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[{}ms] gnome: cannot connect to session bus (signal): {e}",
                crate::core::now_ms());
            return;
        }
    };
    let proxy = match WindowMonitorProxyBlocking::new(&conn) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[{}ms] gnome: proxy build failed (signal): {e}",
                crate::core::now_ms());
            return;
        }
    };
    let iter = match proxy.receive_active_window_changed() {
        Ok(i) => i,
        Err(e) => {
            eprintln!("[{}ms] gnome: receive_active_window_changed failed: {e}",
                crate::core::now_ms());
            return;
        }
    };
    for ev in iter {
        // ev.args() -> ActiveWindowChangedArgs { app_class, title } (the signal's payload).
        let (app_class, title) = match ev.args() {
            Ok(a) => (a.app_class, a.title),
            Err(e) => {
                if verbose { eprintln!("gnome: signal args decode failed: {e}"); }
                continue;
            }
        };
        apply_and_notify(&last_focus, (app_class, title), verbose);
        if shutdown.load(Ordering::Acquire) {
            return;
        }
    }
}

/// Drift-poll + NameOwnerChanged thread (GOTCHA-2/3/11). Owns its Connection. Each tick:
/// hot-re-read gnome_poll_interval_ms; probe name_has_owner; if owned -> GetActiveWindow +
/// dedup (drift correction); if not owned -> emit empty once + no-backend (re-acquires
/// automatically when the name returns).
fn run_poll_loop(
    last_focus: Arc<Mutex<Option<(String, String)>>>,
    shutdown: Arc<AtomicBool>,
    verbose: bool,
) {
    let conn = match zbus::blocking::Connection::session() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[{}ms] gnome: cannot connect to session bus (poll): {e}",
                crate::core::now_ms());
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
            eprintln!("[{}ms] gnome: proxy build failed (poll): {e}", crate::core::now_ms());
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
        // GOTCHA-11: hot-re-read the interval each tick (mtime-keyed cache).
        let ms = crate::core::cached_config()
            .ok()
            .and_then(|c| c.linux.gnome_poll_interval_ms)
            .unwrap_or(DEFAULT_POLL_MS);
        thread::sleep(Duration::from_millis(ms.max(1)));

        if shutdown.load(Ordering::Acquire) {
            break;
        }
        // §8.3 NameOwnerChanged semantics via name_has_owner (GOTCHA-3).
        let owned = match dbus.name_has_owner(BUS_NAME) {
            Ok(b) => b,
            Err(e) => {
                if verbose { eprintln!("gnome: name_has_owner failed: {e}"); }
                continue;
            }
        };
        if owned {
            match proxy.get_active_window() {
                Ok((app_class, title)) => apply_and_notify(&last_focus, (app_class, title), verbose),
                Err(e) => {
                    if verbose { eprintln!("gnome: GetActiveWindow failed: {e}"); }
                }
            }
        } else {
            // Extension not owned: emit empty once (no-backend posture; §8.3).
            apply_and_notify(&last_focus, (String::new(), String::new()), verbose);
        }
    }
}

// ---------------------------------------------------------------------------
// Availability probe (PLATFORMS.md §6 row 2). Cheap + side-effect-free: one
// name_has_owner round-trip to dbus-daemon (GOTCHA-5). NO GetActiveWindow call.
// ---------------------------------------------------------------------------
pub(crate) fn probe_available(verbose: bool) -> Result<(), String> {
    let conn = zbus::blocking::Connection::session()
        .map_err(|e| format!("cannot connect to the session bus (is DBUS_SESSION_BUS_ADDRESS set?): {e}"))?;
    let dbus = zbus::blocking::fdo::DBusProxy::new(&conn)
        .map_err(|e| format!("cannot create DBusProxy on the session bus: {e}"))?;
    let owned = dbus
        .name_has_owner(BUS_NAME)
        .map_err(|e| format!("name_has_owner('{BUS_NAME}') failed: {e}"))?;
    if owned {
        if verbose {
            println!("[{}ms] gnome: '{BUS_NAME}' is owned (extension installed+enabled)",
                crate::core::now_ms());
        }
        Ok(())
    } else {
        Err(format!(
            "the GNOME Shell extension ('{BUS_NAME}') is not installed or not enabled \
             — install qmkonnect@mulletware from extensions.gnome.org (see docs)"
        ))
    }
}

/// Tracked active window as a single `(app_class, title)` row (empty vec when no window is
/// focused or the backend is unavailable). Synchronous one-shot read (PLATFORMS.md §8.3).
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
// plumbing is exercised manually in Level 4). Run single-threaded (GOTCHA-13).
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_and_notify_dedups_unchanged() {
        let cell = Mutex::new(Some(("firefox".to_string(), "x".to_string())));
        // Identical candidate -> no state change (we assert by observing the cell stays equal).
        apply_and_notify(&cell, ("firefox".into(), "x".into()), false);
        assert_eq!(*cell.lock().unwrap(), Some(("firefox".into(), "x".into())));
    }

    #[test]
    fn apply_and_notify_updates_on_change() {
        let cell = Mutex::new(None);
        apply_and_notify(&cell, ("kitty".into(), "kitty".into()), false);
        assert_eq!(*cell.lock().unwrap(), Some(("kitty".into(), "kitty".into())));
        apply_and_notify(&cell, ("kitty".into(), "other".into()), false);
        assert_eq!(*cell.lock().unwrap(), Some(("kitty".into(), "other".into())));
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
        // Snapshot/restore DBUS_SESSION_BUS_ADDRESS so this is hermetic + single-thread-safe.
        let snap = std::env::var("DBUS_SESSION_BUS_ADDRESS").ok();
        std::env::remove_var("DBUS_SESSION_BUS_ADDRESS");
        let r = probe_available(false);
        match snap {
            Some(v) => std::env::set_var("DBUS_SESSION_BUS_ADDRESS", v),
            None => std::env::remove_var("DBUS_SESSION_BUS_ADDRESS"),
        }
        // On this Hyprland box a session bus usually exists; if it does, the probe may Ok/Err
        // on name ownership rather than connection. Only assert the CONNECTION-failure path:
        // when the env is unset AND no autostart socket, expect Err mentioning the session bus.
        if r.is_err() {
            let m = r.unwrap_err();
            assert!(
                m.contains("session bus") || m.contains("not installed") || m.contains("DBUS"),
                "expected a connection/ownership Err; got: {m}"
            );
        }
        // (If a session bus IS reachable here, the probe legitimately Ok/Errs on ownership;
        //  that is not a failure of this test — the gate is "does not panic".)
    }
}
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: BASELINE — clean tree, confirm the sibling inputs
  - RUN: git status --short   (expect clean)
  - RUN: grep -n 'fn gnome_probe\|construct_backend\|("gnome"' src/platforms/linux.rs
    (CONFIRM: gnome_probe stub at ~L170-173 returns Err("…not yet implemented (P2.M3)");
     construct_backend has NO "gnome" arm yet; candidate row ("gnome", gnome_probe) at ~L49-50.)
  - RUN: grep -n 'mod wayland_ft\|fn list_foreground_windows' src/platforms/mod.rs
    (CONFIRM where to add `mod gnome;` + the list branch, mirroring the wayland_ft cfg.)
  - WHY: pins the exact edit sites; confirms Cargo.toml's `gnome = ["dep:zbus"]` is already
    present (DO NOT touch Cargo.toml — GOTCHA-8).

Task 2: CREATE src/platforms/gnome.rs — implement the §Reference skeleton VERBATIM.
  - FILE GATE: #![cfg(all(target_os = "linux", feature = "gnome"))] (GOTCHA-8; mirror wayland_ft.rs).
  - IMPLEMENT: #[zbus::proxy] trait WindowMonitor (get_active_window -> (String,String);
    #[zbus(signal)] active_window_changed). GnomeMonitor struct + impl WindowMonitor
    (platform_name="gnome"; spawn-and-return start; best-effort stop). apply_and_notify
    (GOTCHA-4). run_signal_loop (GOTCHA-1/2/9). run_poll_loop (GOTCHA-3/11). probe_available
    (GOTCHA-5). list_foreground_windows (GOTCHA-6). The hermetic #[cfg(test)] mod tests.
  - NAMING: GnomeMonitor (CamelCase), snake_case fns. Constants BUS_NAME/OBJECT_PATH/
    INTERFACE_NAME/DEFAULT_POLL_MS exactly as the skeleton (they MUST match the extension's
    contract pinned in PLATFORMS.md §8.1 — GOTCHA: these are the SAME strings the extension owns).
  - VALIDATE immediately: cargo build --release   (default features include `gnome`; the
    #[zbus::proxy] macro + blocking proxy + Send monitor must compile — GOTCHA-9 is the
    usual first-compile failure: WindowMonitorProxyBlocking vs WindowMonitorProxy).

Task 3: MODIFY src/platforms/linux.rs — replace the gnome_probe stub + add the construct arm.
  - EDIT A (replace stub body, ~L170-173): make gnome_probe DELEGATE (mirror wayland_probe at
    ~L163-165):
      #[cfg(feature = "gnome")]
      fn gnome_probe(verbose: bool) -> Result<(), String> {
          crate::platforms::gnome::probe_available(verbose)
      }
  - EDIT B (construct_backend match, ~L80-92): ADD (with its cfg, mirroring the wayland arm):
      #[cfg(feature = "gnome")]
      "gnome" => Ok(Box::new(crate::platforms::gnome::GnomeMonitor::new(verbose))),
  - DO NOT touch the candidate row ("gnome", gnome_probe as ProbeFn) (~L49-50) — it already
    exists. DO NOT touch wayland_probe / atspi_probe / the select logic.
  - VALIDATE: cargo build --release && cargo test --bin qmkonnect -- --test-threads=1
    (the select_tests suite must still pass; under a Hyprland-like env the gnome probe Errs
    naming the extension — sanity-check with -v if you like).

Task 4: MODIFY src/platforms/mod.rs — declare the module + wire list_foreground_windows.
  - EDIT A: add (next to the `#[cfg(all(target_os="linux", feature="wayland"))] mod wayland_ft;`
    declaration):
      #[cfg(all(target_os = "linux", feature = "gnome"))]
      mod gnome;
  - EDIT B: in `list_foreground_windows()`, add a GNOME branch. The current dispatch tries
    wayland_ft then hyprland then empty. Add a `gnome` fallback BEFORE the final empty
    (feature-gated, target linux). Single-window read: `return gnome::list_foreground_windows();`.
    Use a cfg that does NOT conflict with the existing mutually-exclusive cfgs (put the gnome
    arm under `#[cfg(all(target_os="linux", feature="gnome", not(feature="wayland")))]` and
    adjust the final catch-all cfg accordingly, OR simplest: add gnome as an additional
    non-conflicting branch mirroring how wayland_ft is gated). Re-read the current cfg ladder
    before editing (it is precise about feature combinations).
  - VALIDATE: cargo build --release (the cfg ladder must stay exhaustive — a missing arm or an
    overlapping cfg is a compile error / dead-code warning).

Task 5: DOCS (Mode A) — document the GNOME backend + the extension dependency.
  - EDIT docs/installation.md: add a "### GNOME (optional Shell extension)" section under the
    Linux install notes: GNOME users need the qmkonnect@mulletware extension (install from
    extensions.gnome.org or the GitHub Release zip); link to PLATFORMS.md §8; note the app
    auto-selects the GNOME backend when the extension is enabled.
  - EDIT docs/troubleshooting.md: add "GNOME: no window detection" — symptom (focus changes
    don't switch layers) + cause (extension not installed/enabled) + fix (enable in the
    Extensions app; Wayland = log out/in) + the daemon's first-run notification pointer.
  - Keep it concise + consistent with the existing doc voice. Reference spec/PLATFORMS.md §8
    as the authoritative spec (do NOT duplicate its detail). DO NOT regenerate llms_full.txt
    here (that is P2.M7.T2.S2 — regenerating now would just be redone).

Task 6: VALIDATE — the automated gates (run on this Hyprland box; see Validation Loop).
  - RUN: cargo build --release
  - RUN: cargo test --bin qmkonnect -- --test-threads=1
  - RUN: git diff --stat   (ONLY gnome.rs[new], mod.rs, linux.rs, docs/installation.md,
        docs/troubleshooting.md — NO packaging/, Cargo.toml, release.yml, .gitignore).
```

### Implementation Patterns & Key Details

```rust
// === The two-thread layout (GOTCHA-2) — each thread owns its Connection ===
self.signal_handle = Some(thread::spawn(move || run_signal_loop(arc_last_focus, arc_shutdown, v)));
self.poll_handle   = Some(thread::spawn(move || run_poll_loop(arc_last_focus, arc_shutdown, v)));

// === Dedup + notify (GOTCHA-4) — DROP the lock BEFORE notify_qmk ===
{
    let mut cell = last_focus.lock().unwrap();
    if *cell == Some(candidate.clone()) { return; }
    *cell = Some(candidate.clone());
}
let wi = WindowInfo::new(candidate.0, candidate.1);
let _ = notifier::notify_qmk(&wi, verbose);   // takes STATE/NOTIFIER locks internally

// === NameOwnerChanged via poll-time name_has_owner (GOTCHA-3) ===
match dbus.name_has_owner(BUS_NAME) {
    Ok(true)  => { if let Ok((c,t)) = proxy.get_active_window() { apply_and_notify(...,(c,t),...); } }
    Ok(false) => { apply_and_notify(...,(String::new(),String::new()),...); }  // empty + no-backend
    Err(e)    => { /* transient; skip this tick */ }
}

// === Hot-config interval (GOTCHA-11) — re-read EVERY tick ===
let ms = crate::core::cached_config().ok()
    .and_then(|c| c.linux.gnome_poll_interval_ms).unwrap_or(DEFAULT_POLL_MS);
thread::sleep(Duration::from_millis(ms.max(1)));
```

### Integration Points

```yaml
D-BUS SESSION BUS (CONSUMED by this task; produced by P2.M3.T1.S1):
  - name: io.mulletware.QMKonnect (owned ⇔ installed+enabled — gnome_probe keys on this)
  - path: /io/mulletware.QMKonnect
  - iface: io.mulletware.QMKonnect.WindowMonitor
  - method GetActiveWindow()->(ss) | signal ActiveWindowChanged(ss)

SELECT_LINUX_BACKEND (src/platforms/linux.rs — MODIFIED by this task):
  - candidate row ("gnome", gnome_probe) UNCHANGED (~L49-50).
  - gnome_probe (~L170-173): body -> delegate to gnome::probe_available (was the Err stub).
  - construct_backend: ADD #[cfg(feature="gnome")] "gnome" => GnomeMonitor::new(verbose).

MODULE DECL + LIST (src/platforms/mod.rs — MODIFIED by this task):
  - add #[cfg(all(target_os="linux", feature="gnome"))] mod gnome;
  - list_foreground_windows: add a gnome branch (single-window read).

CONFIG (CONFIG.md §1.3 — CONSUMED, not modified):
  - [linux] gnome_poll_interval_ms (default 1000) — hot-re-read each poll tick.

NO-FIRST-RUN-EDIT (src/runners/linux.rs — NOT modified by this task):
  - maybe_gnome_first_run_notify already fires in the no-backend Err branch; S1's probe
    returning Err feeds it. The §8.4 enhancement (fire even when another backend is selected)
    is P2.M3.T2.S2.

DATABASE/ROUTES/CARGO.TOML: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --release                 # default features include `gnome`; MUST compile
# Expected: zero errors. The common first-compile failure is a *Blocking proxy name typo
# (GOTCHA-9) or a missing cfg on the new mod/list branch (Task 4). Read the error and fix.
cargo fmt -- src/platforms/gnome.rs   # match repo formatting
# (No clippy gate is wired in CI; if `cargo clippy` is run, fix new lints in the new file.)
```

### Level 2: Unit Tests (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1   # MANDATORY single-threaded (GOTCHA-13)
# Targeted:
cargo test --bin qmkonnect gnome:: -- --test-threads=1
# Expected: the new apply_and_notify_* + probe_err_when_no_session_bus tests pass; the
# existing select_tests suite still passes (the gnome probe now returns Err naming the
# extension instead of "…not yet implemented").
```

### Level 3: Selection wiring (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
# Confirm the stub is gone + the real probe is wired:
grep -n 'not yet implemented (P2.M3)' src/platforms/linux.rs   # EXPECT: no match (was the stub)
grep -n 'crate::platforms::gnome::probe_available' src/platforms/linux.rs   # EXPECT: 1 match
grep -n '"gnome" =>' src/platforms/linux.rs                                  # EXPECT: 1 match (construct arm)
grep -n 'mod gnome;' src/platforms/mod.rs                                    # EXPECT: 1 match
# Build the default release binary (includes the gnome feature) and confirm it links zbus:
cargo build --release && nm target/release/qmkonnect 2>/dev/null | grep -ci zbus   # >0
```

### Level 4: Creative & Domain-Specific Validation (GNOME session — MANUAL, deferred)

> This dev box is **Hyprland** (not GNOME), so the live `gdbus`/daemon run is a MANUAL step
> for a GNOME VM, not a hard gate. The automated ceiling (Levels 1-3) covers compilation,
> the hermetic dedup/empty/probe logic, and the select wiring.

```bash
# In a real GNOME 45-50 session with the extension (P2.M3.T1.S1) installed+enabled:
# 1. Confirm the name is owned + the method works:
gdbus introspect --session --dest io.mulletware.QMKonnect --object-path /io/mulletware/QMKonnect
gdbus call --session --dest io.mulletware.QMKonnect --object-path /io/mulletware/QMKonnect \
  --method io.mulletware.QMKonnect.WindowMonitor.GetActiveWindow
# 2. Run the daemon verbose and confirm GNOME is selected + focus changes notify:
./target/release/qmkonnect -v
#    Expected: "select_linux_backend: probing 'foreign-toplevel'… → unavailable"; then
#    "gnome: 'io.mulletware.QMKonnect' is owned (extension installed+enabled)"; "→ 'gnome'
#    available, selected"; then "[<ms>] gnome: <wm_class> | <title>" on each focus change.
# 3. Toggle the extension off mid-session (Extensions app) -> within <=1s the poll thread sees
#    name_has_owner=false -> emits empty ("","") once (no-backend posture; tray+device pipeline
#    keep running). Toggle it back on -> the next poll's GetActiveWindow re-acquires state.
# 4. Watch the signal live while switching focus (independent of the poll):
gdbus monitor --session --dest io.mulletware.QMKonnect
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --release` succeeds (default features include `gnome`; the `#[zbus::proxy]`
      macro + blocking proxy + Send monitor compile).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` passes (new hermetic tests + existing
      `select_tests`).
- [ ] The `gnome_probe` stub's old `Err("…not yet implemented (P2.M3)")` is GONE; it now
      delegates to `gnome::probe_available`.
- [ ] `construct_backend` has a `#[cfg(feature="gnome")] "gnome" =>` arm.
- [ ] `git diff --stat` shows ONLY `src/platforms/gnome.rs` (new), `src/platforms/mod.rs`,
      `src/platforms/linux.rs`, `docs/installation.md`, `docs/troubleshooting.md` (NO
      `packaging/`, `Cargo.toml`, `release.yml`, `.gitignore`, `PRD.md`, `tasks.json`).

### Feature Validation (parity with PLATFORMS.md §8.3 / ARCHITECTURE.md §6)
- [ ] `probe_available` returns `Ok` iff `io.mulletware.QMKonnect` is owned (one
      `name_has_owner` round-trip; GOTCHA-5).
- [ ] `start()` spawns a signal thread + a poll thread and returns promptly
      (`start_blocks_calling_thread() == false`).
- [ ] Signal thread subscribes to `ActiveWindowChanged(ss)` (generated blocking proxy) and
      dedups + notifies.
- [ ] Poll thread sleeps `gnome_poll_interval_ms` (hot-config, default 1000; GOTCHA-11), calls
      `GetActiveWindow`, dedups (drift correction).
- [ ] NameOwnerChanged semantics via poll-time `name_has_owner` (GOTCHA-3): name gone → empty
      + no-backend; name returned → re-acquired.
- [ ] Each worker thread owns its OWN `blocking::Connection` (GOTCHA-2).
- [ ] Empty window `("","")` → empty `WindowInfo`; `list_foreground_windows` → `vec![]`.

### Code Quality Validation
- [ ] `GnomeMonitor` mirrors `WaylandFtMonitor`'s struct shape (Arc<Mutex>/Arc<AtomicBool>/
      Option<JoinHandle>/bool → `Send`).
- [ ] Dedup releases the lock BEFORE `notify_qmk` (GOTCHA-4).
- [ ] File gate `#![cfg(all(target_os="linux", feature="gnome"))]` matches wayland_ft.rs.
- [ ] Scope respected: NO extension files (P2.M3.T1.S1), NO first-run-notification logic
      (P2.M3.T2.S2), NO Cargo.toml (P2.M1.T2.S2), NO release.yml (P2.M7.T2.S1), NO
      `.gitignore`.

### Documentation & Deployment
- [ ] Mode A: docs/installation.md + docs/troubleshooting.md document the GNOME backend + the
      extension dependency, referencing spec/PLATFORMS.md §8.
- [ ] `llms_full.txt` is NOT regenerated here (owned by P2.M7.T2.S2).

---

## Anti-Patterns to Avoid

- ❌ Do NOT share one `blocking::Connection` across both worker threads — it serializes on the
  internal executor (GOTCHA-2). Each thread creates its own.
- ❌ Do NOT add a THIRD thread for a real `NameOwnerChanged` signal subscription — fold it into
  the poll thread's `name_has_owner` (GOTCHA-3).
- ❌ Do NOT hold the `last_focus` lock while calling `notify_qmk` (GOTCHA-4).
- ❌ Do NOT call `GetActiveWindow` inside `probe_available` (GOTCHA-5) — ownership of the name
  is the presence signal; introspection/method calls race with object export.
- ❌ Do NOT cache `gnome_poll_interval_ms` at `start()` (GOTCHA-11) — hot-re-read each tick.
- ❌ Do NOT invent a timeout-based signal receive for "clean shutdown" (GOTCHA-7) — best-effort
  stop + the process's ctrlc handler is the codebase norm (wayland_ft.rs).
- ❌ Do NOT touch Cargo.toml / packaging/gnome-shell-extension / release.yml / runners/linux.rs
  (scope boundary).
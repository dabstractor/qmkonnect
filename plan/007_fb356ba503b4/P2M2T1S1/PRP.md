# PRP — P2.M2.T1.S1: Core `wayland_ft.rs` (wlr focus tracking + ext cross-check + spawn-and-return + reconnect)

> **Repo under change:** QMKonnect (`/home/dustin/projects/qmkonnect`). **Files
> edited:** CREATE `src/platforms/wayland_ft.rs`; MODIFY `src/platforms/linux.rs`
> (replace the `wayland_probe` stub + add a `construct_backend` arm); MODIFY
> `src/platforms/mod.rs` (declare the module + route `list_foreground_windows`).
> **No `Cargo.toml` change** — the `wayland` feature + the `smithay-client-toolkit`
> 0.20 / `wayland-client` 0.31 deps are already declared by the parallel sibling
> P2.M1.T2.S2 (treat that PRP as a contract). The wlr-protocol generated types
> arrive **for free** via `smithay_client_toolkit::reexports::protocols_wlr`
> (transitive dep; verified in `Cargo.lock`). **No `docs/*` prose** (Mode A:
> `spec/PLATFORMS.md` §7.2 is the reference coverage table).
>
> **What this does:** implements the foreign-toplevel Wayland backend — the
> **priority-#1** Linux window monitor (F16 / PRD §4). One Wayland client speaking
> `wlr-foreign-toplevel-management-v1` reports the *focused* toplevel (the `state`
> event's `activated` flag), `app_class` = the toplevel `app_id`, and a
> `list_foreground_windows()` snapshot. `start()` **spawns** the dispatch thread
> and **returns immediately** (unlike Hyprland's blocking listener); reconnects
> with the Hyprland backoff constants on compositor death. Covers Hyprland, Sway,
> Niri, River, Labwc, Wayfire, KDE Plasma 6 (KWin), COSMIC; GNOME (Mutter)
> advertises neither foreign-toplevel protocol and falls through to P2.M3.
>
> **⚠️ HEADLINE CORRECTION — read before writing any code.** The task description
> and `spec/PLATFORMS.md` §7.3 both state: *"smithay-client-toolkit provides the
> wlr-protocol ForeignToplevelManager/ForeignToplevelHandler"*. **That is factually
> wrong for sctk 0.20.** sctk 0.20 exposes a module named `foreign_toplevel_list`
> which wraps the **EXT** protocol (`ext-foreign-toplevel-list-v1`) — and the ext
> protocol **has no activation state**. There is no `ForeignToplevelManager` /
> `ForeignToplevelHandler` / `ForeignToplevelState` type in sctk 0.20 (those are
> the deleted 0.19 API). Therefore the load-bearing wlr protocol must be
> **hand-rolled** with raw `wayland-client` `Dispatch` impls (full evidence +
> exact API in `research/notes.md` §§1–7). Do NOT search for `ForeignToplevelManager`
> in sctk — it is not there; you will waste a turn.

---

## Goal

**Feature Goal**: Implement `src/platforms/wayland_ft.rs` — a `WaylandFtMonitor`
implementing `WindowMonitor` that tracks the focused Wayland toplevel via
`wlr-foreign-toplevel-management-v1` and emits `notify_qmk` on focus change
(empty workspace → `WindowInfo { app_class:"", title:"" }`), with a
spawn-and-return `start()`, reconnect-with-backoff on compositor death, an
availability `probe_available()`, and a `list_foreground_windows()` snapshot —
and wire it into `select_linux_backend` as priority #1 behind the `wayland`
feature.

**Deliverable** (concrete; compiles + passes tests on the dev box TODAY):
- `src/platforms/wayland_ft.rs` — `#![cfg(all(target_os = "linux", feature = "wayland"))]`
  with: `WaylandFtMonitor` struct (holds only `Arc<Mutex<Option<(String,String)>>>`
  + shared list snapshot + shutdown flag + `JoinHandle` + `verbose`); the
  `WindowMonitor` impl (`platform_name="foreign-toplevel"`, spawn-and-return
  `start()`, default `start_blocks_calling_thread()==false`, best-effort `stop()`);
  `pub(crate) fn probe_available(verbose) -> Result<(), String>`;
  `pub fn list_foreground_windows() -> Vec<(String,String)>`; hand-rolled
  `Dispatch` impls for the wlr manager + handle; pure helpers
  `decode_activated(&[u8]) -> bool` + `recompute_focus(...)`; reconnect loop
  reusing the Hyprland backoff constants (re-declared locally); unit tests for
  the pure helpers + the probe Err-path.
- `src/platforms/linux.rs` — replace the `wayland_probe` STUB (currently
  `Err("foreign-toplevel Wayland backend not yet implemented (P2.M2)")`) with a
  real probe that delegates to
  `crate::platforms::wayland_ft::probe_available`; add a `"foreign-toplevel"`
  arm to `construct_backend` returning
  `Box::new(crate::platforms::wayland_ft::WaylandFtMonitor::new(verbose))`.
  (The candidate NAME in `linux_backend_candidates()` is ALREADY
  `"foreign-toplevel"` — do not rename it.)
- `src/platforms/mod.rs` — add
  `#[cfg(all(target_os = "linux", feature = "wayland"))] mod wayland_ft;` and
  route `list_foreground_windows()` to `wayland_ft::list_foreground_windows()`
  when the feature is on (BEFORE the hyprland arm, mirroring selection priority).

**Success Definition**:
- `cargo build --bin qmkonnect` (default features) compiles clean — `wayland_ft`
  is compiled in (it's in `default`).
- `cargo build --bin qmkonnect --no-default-features` compiles clean —
  `wayland_ft` is absent (the `wayland` feature + its module cfg gate it out);
  the `wayland_probe` stub + `construct_backend` "foreign-toplevel" arm are also
  absent (they're `#[cfg(feature = "wayland")]`).
- `cargo test --bin qmkonnect -- --test-threads=1` passes — the new pure-helper
  tests (decode_activated, recompute_focus incl. empty-workspace, list ordering)
  pass, and the probe Err-path test (unset `$WAYLAND_DISPLAY`) passes. No
  regression in the existing `select_tests` (the wayland probe now returns a real
  `Err(reason)` instead of the stub string when `$WAYLAND_DISPLAY` is unset,
  which is what those tests already assert via the X11/hyprland paths).
- `grep -rn 'ForeignToplevelManager' src/` returns nothing (we hand-roll; the
  type doesn't exist in sctk 0.20 — see the HEADLINE CORRECTION).
- `git diff --stat` shows ONLY the 3 files above (no `Cargo.toml`, no `docs/*`,
  no PRD/tasks.json, no `select_linux_backend` candidate-list edit).

## User Persona (if applicable)

**Target User**: Linux users on a wlroots-derived compositor or KDE Plasma 6 /
COSMIC (the F16 "one binary, every desktop" promise). Also the downstream
backends P2.M3 (GNOME) / P2.M4 (AT-SPI) / P2.M5 (X11) which are LOWER priority —
this task winning priority #1 means on Hyprland/Sway/KDE/etc. THIS backend runs,
not the Hyprland-IPC or X11 one.

**Use Case**: A user launches QMKonnect under Sway; `select_linux_backend` probes
"foreign-toplevel" first, `probe_available` confirms `$WAYLAND_DISPLAY` resolves
and the wlr global is advertised, constructs `WaylandFtMonitor`, whose `start()`
spawns the dispatch thread and returns; the runner parks main + drives the tray.
On every focus change the keyboard layer switches; the tray's "Show Window
Information" lists all toplevels (active first).

**Pain Points Addressed**: Today (with only the `wayland` feature declared but no
backend), `select_linux_backend`'s wayland probe returns the stub `Err`, so on
every non-Hyprland wlroots compositor (Sway/Niri/River/Labwc/Wayfire) AND on KDE
Plasma / COSMIC, the app falls through to X11 (unreliable under XWayland) or
no-backend. This backend makes F16 actually work on those desktops.

## Why

- **F16 (PRD §4) requires one binary on every Linux desktop.** The
  foreign-toplevel backend is the single highest-leverage one: it covers every
  wlroots compositor + KDE Plasma 6 + COSMIC in one client. PLATFORMS.md §7
  designates it priority #1. Without it, the F16 promise is hollow on Sway/KDE.
- **Hyprland already works, but only via its IPC** (§5) — an incidental detail of
  that one compositor. The standard `wlr-foreign-toplevel-management-v1` protocol
  reports the same active window on Hyprland too (§7.4), so this backend
  supersedes the Hyprland-IPC path on the default build (the IPC backend stays
  compiled as a fallback when `wayland` is off).
- **GNOME (Mutter) advertises neither protocol** → falls through to P2.M3
  (Shell extension over D-Bus). The probe gate (`wlr` global advertised) makes
  that fall-through automatic and correct.
- **The spawn-and-return model** (vs Hyprland's blocking listener) is what lets
  the runner park main / drive the tray uniformly for the non-Hyprland backends
  (ARCHITECTURE §11; `WindowMonitor::start_blocks_calling_thread()` defaults to
  `false` and we keep the default).

## What

A `WaylandFtMonitor` that:

1. **`start()`** spawns a dedicated thread that: connects `wl_display`
   (`Connection::connect_to_env`), enumerates globals (`registry_queue_init`),
   binds `zwlr_foreign_toplevel_manager_v1`, then loops on
   `EventQueue::blocking_dispatch(&mut state)`. `start()` returns `Ok(())`
   IMMEDIATELY (spawn-and-return). On `Err(DispatchError::Backend(_))`
   (compositor death) it reconnects with backoff (constants in §Gotchas), checking
   a shutdown flag between attempts.
2. **Per-toplevel tracking** (in the thread's `State`, keyed by `handle.id()`):
   on the manager's `toplevel` event insert a fresh handle; on `title`/`app_id`/
   `state` update pending fields; on `done` commit pending→current then recompute;
   on `closed` remove + `destroy()` then recompute. `app_class` = the toplevel's
   `app_id` (reverse-DNS passed through verbatim).
3. **Focus recompute** (pure helper, unit-tested): the toplevel whose committed
   `state` includes `activated` (value `2`) is the focus. If it differs from the
   last emitted `(app_class, title)` → update the shared last-state cell +
   `notifier::notify_qmk(&WindowInfo::new(app_class, title), verbose)` + refresh
   the shared list snapshot (activated first). **No activated toplevel →
   `WindowInfo { app_class:"", title:"" }`** (empty workspace; deactivates
   layers).
4. **`stop()`** sets the shutdown flag (best-effort; `blocking_dispatch` is
   blocking, so the thread exits on the next event/compositor-teardown — same
   posture as Hyprland's blocking listener).
5. **`list_foreground_windows()`** reads a shared snapshot the dispatch thread
   publishes (all tracked toplevels as `(app_id, title)`, activated first; empty
   if no monitor ever ran).
6. **`probe_available()`** — side-effect-free: `Connection::connect_to_env()`
   succeeds (gate 1: `$WAYLAND_DISPLAY` resolvable) AND the compositor advertises
   `zwlr_foreign_toplevel_manager_v1` (gate 2: `with_list` scan after a sync
   roundtrip).

### Success Criteria
- [ ] `WaylandFtMonitor: WindowMonitor` with `platform_name()=="foreign-toplevel"`,
      default `start_blocks_calling_thread()==false`, spawn-and-return `start()`.
- [ ] Focus tracked from the wlr `state` event's `activated` flag (value 2),
      decoded from the raw `Vec<u8>` array (no `Activated` enum variant — it
      doesn't exist).
- [ ] Empty workspace (no activated toplevel) emits `WindowInfo{"",""}`.
- [ ] Reconnect with backoff on `DispatchError::Backend`, reusing the Hyprland
      constants (100→…→10_000 ×3, reset after 5s stable).
- [ ] `probe_available` gated on `$WAYLAND_DISPLAY` resolvable + wlr global
      advertised.
- [ ] `list_foreground_windows()` returns tracked toplevels (activated first).
- [ ] Pure helpers (`decode_activated`, `recompute_focus`) unit-tested; probe
      Err-path tested by unsetting `$WAYLAND_DISPLAY`.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can implement this from this PRP + the
repo, because (a) the HEADLINE CORRECTION (sctk 0.20 ≠ wlr) prevents the #1
failure mode (searching for a nonexistent type), (b) the EXACT module path for
the wlr types is given with the no-new-dep rationale, (c) the EXACT generated
event signatures (`Title{title:String}`, `AppId{app_id:String}`,
`State{state:Vec<u8>}`, `Done`, `Closed`) + the `state` byte-decode (LE u32,
value 2) are source-confirmed, (d) the connect/dispatch entry points
(`connect_to_env` + `registry_queue_init` + `GlobalList::bind` +
`blocking_dispatch` + `DispatchError::Backend`) are pinned to file:line, (e) the
two existing-pattern files to mirror (`hyprland.rs` for the monitor struct /
backoff constants / probe / list shape; `linux.rs` for the candidate-list +
construct_backend + probe signature) are identified with exact symbols, (f) the
scope boundary with the parallel sibling (P2.M1.T2.S2 owns Cargo.toml; this task
touches NO Cargo.toml) is explicit. See `research/notes.md` for the full evidence
trail.

### Documentation & References

```yaml
# MUST READ — the authoritative backend spec (the WHAT; note §7.3's sctk claim is
# corrected by this PRP's HEADLINE CORRECTION — the protocols/coverage/tracking
# semantics are still authoritative)
- file: spec/PLATFORMS.md
  why: "§7 is the spec for THIS backend: §7.1 protocols (wlr=load-bearing focus;
        ext=cross-check, no activation), §7.2 coverage table (Hyprland/Sway/Niri/
        River/Labwc/Wayfire/KDE6/COSMIC=✅; GNOME=❌→§8), §7.3 implementation
        (spawn-and-return start(), per-toplevel tracking, app_class=app_id,
        empty-workspace semantics, reconnect reusing Hyprland §5.3 constants,
        list_foreground_windows activated-first). §6 is the selector contract
        (priority #1, feature `wayland`, probe = $WAYLAND_DISPLAY resolvable AND
        wlr global advertised). §11 thread summary."
  pattern: "WaylandFtMonitor mirrors HyprlandMonitor's Arc<Mutex<Option<…>>> last-
            state shape, probe_available() -> Result<(),String> signature, and
            list_foreground_windows() -> Vec<(String,String)> signature."

# MUST READ — the file to MIRROR (monitor struct, backoff constants, probe, list)
- file: src/platforms/hyprland.rs
  why: "the closest existing backend. COPY its shape: `pub struct HyprlandMonitor
        { last_window_state: Arc<Mutex<Option<WindowState>>>, verbose: bool }` +
        `impl WindowMonitor` (platform_name/start/start_blocks_calling_thread);
        the backoff consts INITIAL_RECONNECT_MS=100 / MAX_RECONNECT_MS=10_000 /
        STABLE_CONNECTION_THRESHOLD=5s (factor ×3) — RE-DECLARE identical consts
        in wayland_ft.rs (they are PRIVATE to hyprland.rs); `pub(crate) fn
        probe_available(_verbose: bool) -> Result<(), String>`; `#[allow(dead_code)]
        pub fn list_foreground_windows() -> Vec<(String,String)>` (activated first
        via swap-to-front)."
  pattern: "handle_focus_change-style dedup: compare (app_class,title) to the last
            cell under ONE lock, emit notify_qmk AFTER dropping the lock (it takes
            the debounce STATE/NOTIFIER locks — never hold last-state while
            notifying)."
  gotcha: "hyprland's start() BLOCKS (start_blocks_calling_thread==true) — wayland_ft
           must NOT block (keep the default false) and must spawn its own thread."

# MUST READ — the file to EDIT (candidate list is already correct; edit probe + construct)
- file: src/platforms/linux.rs
  why: "(1) the wayland_probe STUB at the bottom returns
        Err(\"foreign-toplevel Wayland backend not yet implemented (P2.M2)\") —
        REPLACE its body with
        `crate::platforms::wayland_ft::probe_available(_verbose)` (keep the
        #[cfg(feature=\"wayland\")] gate + signature). (2) `construct_backend`
        (middle of file) has arms only for \"hyprland\" + \"x11\" — ADD an arm:
        `#[cfg(feature=\"wayland\")] \"foreign-toplevel\" => Ok(Box::new(…::
        WaylandFtMonitor::new(verbose)))`. (3) DO NOT touch
        linux_backend_candidates() — the `(\"foreign-toplevel\", wayland_probe as
        ProbeFn)` row ALREADY EXISTS and is priority #1."
  pattern: "probe_available must be side-effect-free (no env mutation) — mirror
            hyprland::probe_available's read-only posture. The select_tests module
            here shows the env-snapshot/restore pattern for the probe test."

# MUST READ — the file to EDIT (declare module + route list_foreground_windows)
- file: src/platforms/mod.rs
  why: "(1) add `#[cfg(all(target_os=\"linux\", feature=\"wayland\"))] mod
        wayland_ft;` next to the existing `mod hyprland;`. (2) in
        `list_foreground_windows()`, add an arm BEFORE the hyprland one:
        `#[cfg(all(target_os=\"linux\", feature=\"wayland\"))] return
        wayland_ft::list_foreground_windows();` (priority #1). Keep the existing
        `#[cfg_attr(not(any(macOS, windows, feature=\"linux-tray\")),
        allow(dead_code))]` attribute."
  pattern: "the mod declarations are plain `mod x;` gated by cfg(target_os) — match
            the `#[cfg(target_os=\"linux\")] mod x11;` comment style. list_foreground
            _windows is a free fn dispatching by cfg — wayland arm first."

# MUST READ — the trait contract + the spawn-and-return contract
- file: src/platforms/mod.rs  (the `pub trait WindowMonitor: Send` block)
  why: "the trait: `fn platform_name(&self)->&str`, `fn start(&mut self)->Result<(),
        Box<dyn Error>>`, `fn stop(&mut self)` (default no-op — override for the
        shutdown flag), `fn start_blocks_calling_thread(&self)->bool` (DEFAULT
        false — DO NOT override; wayland_ft is spawn-and-return). The `: Send` bound
        is why the struct holds only Arc<Mutex>/JoinHandle/AtomicBool/bool."

# MUST READ — the notify path (unchanged; call exactly as Hyprland does)
- file: src/core/notifier.rs
  why: "`pub fn notify_qmk(window_info: &WindowInfo, verbose: bool) -> Result<(),
        Box<dyn Error + Send + Sync>>` (line 1698). Deduped + debounced internally;
        call it on every focus CHANGE (it's cheap when unchanged)."
- file: src/core/types.rs
  why: "`WindowInfo::new(app_class: String, title: String)` + derive(Clone,PartialEq).
        Empty workspace = WindowInfo::new(\"\".into(), \"\".into())."

# REFERENCE — sctk 0.20 source (the re-export you'll use; confirms NO new dep)
- file: ~/.cargo/registry/src/index.crates.io-*/smithay-client-toolkit-0.20.0/src/lib.rs
  why: "line 22: `pub use wayland_protocols_wlr as protocols_wlr;` — the re-export
        that gives you the wlr types via `smithay_client_toolkit::reexports::
        protocols_wlr::foreign_toplevel::v1::client::*`. ALSO confirms sctk 0.20's
        foreign module is `foreign_toplevel_list` (EXT) at lib.rs:30 — there is NO
        `foreign_toplevel`/`ForeignToplevelManager` module (the HEADLINE CORRECTION)."

# REFERENCE — the generated wlr interface (XML → Rust mapping)
- file: ~/.cargo/registry/cache/*/wayland-protocols-wlr-0.3.12.crate
        (or the extracted src; the XML lives at wlr-protocols/unstable/
         wlr-foreign-toplevel-management-unstable-v1.xml inside it)
  why: "the authoritative protocol contract. manager event `toplevel` (new_id
        handle); handle events title/app_id/state/done/closed; enum state:
        maximized=0,minimized=1,ACTIVATED=2,fullscreen=3. The `state` event arg is
        `<arg name=\"state\" type=\"array\"/>` (no enum= attr) → generated Rust
        `Event::State { state: Vec<u8> }` (wayland-scanner client_gen.rs:214).
        Decode as LE u32 chunks; activated ⟺ any chunk == 2."

# REFERENCE — wayland-client 0.31 connect/dispatch (verified signatures)
- url: https://docs.rs/wayland-client/0.31/wayland_client/
  why: "`Connection::connect_to_env()`, `globals::registry_queue_init::<State>(&conn)
        -> Result<(GlobalList, EventQueue<State>), GlobalError>`, `GlobalList::bind(qh,
        1..=3, ())`, `GlobalListContents::with_list(|&[Global]| …)`,
        `EventQueue::blocking_dispatch(&mut state) -> Result<usize, DispatchError>`,
        `DispatchError::{BadMessage{..}, Backend(WaylandError)}`."
  critical: "registry_queue_init REQUIRES `State: Dispatch<wl_registry::WlRegistry,
            GlobalListContents>` — provide a (possibly empty-body) impl (see
            research/notes.md §5 for the canonical skeleton from globals.rs docs)."

# REFERENCE — the parallel sibling PRP (Cargo.toml contract; do NOT duplicate)
- file: plan/007_fb356ba503b4/P2M1T2S2/PRP.md
  why: "owns Cargo.toml: `wayland = [\"dep:smithay-client-toolkit\",
        \"dep:wayland-client\"]` + the two optional deps. THIS task touches NO
        Cargo.toml. The `wayland` feature is already in `default`, so
        wayland_ft.rs compiles under `cargo build` (default) and is absent under
        `--no-default-features`."
```

### Current Codebase tree (relevant subset)

```bash
Cargo.toml                         # ← P2.M1.T2.S2 owns this; DO NOT TOUCH (wayland feature already declared)
src/
  core/
    notifier.rs                    # notify_qmk(window_info, verbose)  ← CALL as Hyprland does
    types.rs                       # WindowInfo::new(app_class, title)
    mod.rs                         # now_ms() for verbose logs (optional)
  platforms/
    mod.rs                         # WindowMonitor trait; list_foreground_windows() dispatcher  ← EDIT (mod decl + route)
    linux.rs                       # select_linux_backend + construct_backend + wayland_probe STUB  ← EDIT (probe body + construct arm)
    hyprland.rs                    # the backend to MIRROR (struct shape, consts, probe, list)  ← READ ONLY
    x11.rs                         # lower-priority fallback  ← READ ONLY
spec/
  PLATFORMS.md                     # §6 (selector) + §7 (THIS backend) — authoritative  ← READ ONLY
  ARCHITECTURE.md                  # §11 (thread summary), invariants  ← READ ONLY
```

### Desired Codebase tree (files this task changes)

```bash
src/platforms/wayland_ft.rs        # NEW — WaylandFtMonitor + probe_available + list_foreground_windows + wlr Dispatch impls + tests
src/platforms/linux.rs             # MODIFIED — wayland_probe stub → real probe; +construct_backend "foreign-toplevel" arm
src/platforms/mod.rs               # MODIFIED — +`mod wayland_ft;` (cfg-gated); +list_foreground_windows wayland arm (priority #1)
# (NO Cargo.toml / docs / PRD / tasks.json changes)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (GOTCHA-1 — the HEADLINE CORRECTION): sctk 0.20 does NOT expose the
//   wlr protocol. There is NO `ForeignToplevelManager`/`ForeignToplevelHandler`/
//   `ForeignToplevelState` — those are the DELETED sctk 0.19 API. sctk 0.20's
//   `foreign_toplevel_list` module wraps the EXT protocol, which has NO
//   activation. Do NOT `use smithay_client_toolkit::foreign_toplevel::*` (it
//   does not exist) and do NOT look for an `Activated` flag in sctk. Hand-roll
//   the wlr protocol via Dispatch (research/notes.md §1, §7).

// CRITICAL (GOTCHA-2 — the wlr types come via a RE-EXPORT, no new dep): use
//   `smithay_client_toolkit::reexports::protocols_wlr::foreign_toplevel::v1::
//   client::{zwlr_foreign_toplevel_manager_v1, zwlr_foreign_toplevel_handle_v1}`.
//   Do NOT add `wayland-protocols-wlr` to Cargo.toml (it's already a transitive
//   dep via sctk; adding it would duplicate the version constraint and is out of
//   this task's scope — Cargo.toml is owned by P2.M1.T2.S2). Verified in Cargo.lock.

// CRITICAL (GOTCHA-3 — the `state` event is Vec<u8>, NOT Vec<State>): the wlr
//   handle's `state` event generates `Event::State { state: Vec<u8> }`
//   (wayland-scanner client_gen.rs:214 — `type=array` w/o enum= ⇒ Vec<u8>). The
//   bytes are a packed LE-u32 sequence; `activated` is value 2. Decode with:
//     state.chunks_exact(4).map(|c| u32::from_le_bytes([c[0],c[1],c[2],c[3]])).any(|v| v == 2)
//   There is NO `State::Activated` enum variant to `.contains()`. (research §4.)

// CRITICAL (GOTCHA-4 — start() must SPAWN and RETURN, not block): unlike
//   Hyprland (start_blocks_calling_thread==true), wayland_ft keeps the trait
//   DEFAULT (false) and spawns its own thread. DO NOT override
//   start_blocks_calling_thread(). The runner relies on this to park main / drive
//   the tray (PLATFORMS.md §11, mod.rs doc on the trait method).

// CRITICAL (GOTCHA-5 — the struct must be Send; hold no wayland objects in it):
//   `trait WindowMonitor: Send`. Although the pure-Rust wayland backend makes
//   Connection/EventQueue/QueueHandle/proxies Send+Sync, construct ALL of them ON
//   the spawned dispatch thread. The struct holds ONLY: `Arc<Mutex<Option<(String,
//   String)>>>` (last focus), the shared `Arc<Mutex<Vec<(String,String)>>>` list
//   snapshot, `Arc<AtomicBool>` shutdown, `Option<JoinHandle<()>>`, `bool verbose`.

// GOTCHA-6 (the candidate NAME is "foreign-toplevel", NOT "wayland"):
//   linux_backend_candidates() already has the row `#[cfg(feature="wayland")]
//   ("foreign-toplevel", wayland_probe as ProbeFn)`. construct_backend must arm
//   on the STRING "foreign-toplevel". Do NOT rename the candidate.

// GOTCHA-7 (reconnect consts are PRIVATE to hyprland.rs): INITIAL_RECONNECT_MS,
//   MAX_RECONNECT_MS, STABLE_CONNECTION_THRESHOLD, factor ×3 are `const` items in
//   hyprland.rs (private). RE-DECLARE identical consts in wayland_ft.rs (same
//   values: 100, 10_000, 5s, ×3). Do NOT try to import them.

// GOTCHA-8 (reconnect trigger): `EventQueue::blocking_dispatch` returns
//   `Err(DispatchError::Backend(WaylandError))` on compositor death. That is your
//   reconnect signal. `DispatchError::BadMessage{..}` is a protocol violation —
//   log + continue (do NOT reconnect on it). There is no dedicated ConnectionLost
//   variant (research §5).

// GOTCHA-9 (notify AFTER releasing the last-state lock): notify_qmk takes the
//   debounce STATE/NOTIFIER locks internally. Never call it while holding the
//   last-state Mutex (mirrors hyprland.rs handle_window_state_change: build the
//   WindowInfo under the lock, drop the lock, THEN notify).

// GOTCHA-10 (empty workspace = no activated toplevel): if the recompute finds
//   zero activated toplevels, emit WindowInfo{app_class:"", title:""} — this
//   DEACTIVATES layers. Do not skip the emit; do not retain the last window.

// GOTCHA-11 (list_foreground_windows is a FREE fn reading a shared snapshot):
//   unlike Hyprland's (which queries IPC live), the wlr toplevels live in the
//   dispatch thread's State. Publish a snapshot via a module-level
//   `static SHARED: OnceLock<Arc<Mutex<Vec<(String,String)>>>>` that start() inits
//   and the dispatch thread writes (activated first). The free fn reads it (empty
//   if no monitor ran). Only ONE backend runs at a time, so a single static is safe.

// GOTCHA-12 (probe is side-effect-free + reads env): probe_available must NOT
//   mutate env (select_linux_backend may re-probe). It reads $WAYLAND_DISPLAY
//   implicitly via Connection::connect_to_env. The select_tests in linux.rs
//   snapshot/restore env — mirror that for the unset-$WAYLAND_DISPLAY test.

// GOTCHA-13 (tests are hermetic — no compositor on the dev box / CI): the
//   Dispatch impls + connect/bind CANNOT be unit-tested without a compositor.
//   FACTOR the logic into PURE helpers (decode_activated, recompute_focus,
//   build_list) and test THOSE. Mirror hyprland.rs testing (tests the socket
//   probe, not the listener). Run `cargo test --bin qmkonnect -- --test-threads=1`.

// GOTCHA-14 (ext-foreign-toplevel cross-check is DEFERRED): the contract says
//   "bind ext if present". ext is functionally INERT (no activation; list/app_id
//   already come from wlr). Wiring sctk's ForeignToplevelList would force a 2nd
//   handler mechanism in State for zero behavioral gain. DEFER ext to a follow-up
//   (document the exact sctk API: ForeignToplevelList::new(&globals,&qh) +
//   impl ForeignToplevelListHandler + delegate_foreign_toplevel_list!(State)).
//   The wlr-only backend is fully spec-compliant. Do NOT wire ext in this task.
```

## Implementation Blueprint

### Data models and structure

```rust
// src/platforms/wayland_ft.rs — top of file
#![cfg(all(target_os = "linux", feature = "wayland"))]

use crate::core::notifier;
use crate::core::types::WindowInfo;
use crate::platforms::WindowMonitor;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

// `wayland-client` is a direct (feature-gated) dep — import EVERYTHING wayland-side
// from it directly; use sctk ONLY for the `protocols_wlr` re-export (the wlr types).
use wayland_client::{
    globals::{registry_queue_init, GlobalList, GlobalListContents},
    protocol::wl_registry,
    Connection, Dispatch, DispatchError, ObjectId, Proxy, QueueHandle,
};
use smithay_client_toolkit::reexports::protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{self, Event as HandleEvent, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{self, Event as MgrEvent, ZwlrForeignToplevelManagerV1},
};

// ---- reconnect backoff (mirrors hyprland.rs private consts; research §9) ----
const INITIAL_RECONNECT_MS: u64 = 100;
const MAX_RECONNECT_MS: u64 = 10_000;
const STABLE_CONNECTION_THRESHOLD: Duration = Duration::from_secs(5);
// factor ×3 (matches hyprland.rs)

/// Per-toplevel cached info (the thread's working set). `pending_*` accumulate
/// between `done` events; `current_*` is the last committed snapshot.
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
/// monitor struct — GOTCHA-5). toplevels keyed by handle.id().
struct DispatchState {
    toplevels: HashMap<wayland_client::ObjectId, HandleInfo>,
    last_focus: Arc<Mutex<Option<(String, String)>>>,   // shared with the monitor
    list_snapshot: Arc<Mutex<Vec<(String, String)>>>,   // shared with list_foreground_windows
    verbose: bool,
}

/// The public monitor (Send: holds only Arc<Mutex>/AtomicBool/JoinHandle/bool).
pub struct WaylandFtMonitor {
    last_focus: Arc<Mutex<Option<(String, String)>>>,
    list_snapshot: Arc<Mutex<Vec<(String, String)>>>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    verbose: bool,
}
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: BASELINE — confirm the build is green BEFORE any edit
  - RUN: cargo build --bin qmkonnect                 (default; wayland feature compiles today)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1   (must pass today)
  - WHY: establishes that later breakage is THIS task's fault. NOTE: the wayland
    probe stub currently returns Err("…not yet implemented (P2.M2)"), which is the
    expected transitional state — select_tests do not exercise wayland_probe's Ok
    path, so they stay green.
  - IF the baseline is red: STOP and report (do not build on a red baseline).

Task 2: CREATE src/platforms/wayland_ft.rs — the pure helpers FIRST (testable in isolation)
  - IMPLEMENT `fn decode_activated(state: &[u8]) -> bool`: LE-u32 chunks, any == 2.
  - IMPLEMENT `fn recompute_focus(toplevels: &HashMap<ObjectId, HandleInfo>)
      -> Option<(String,String)>`: scan for the single toplevel with
      current_activated==true; return Some((app_id,title)); None if none activated
      (caller maps None → ("","")). If MULTIPLE are activated (shouldn't happen
      but be defensive), pick the first encountered.
  - IMPLEMENT `fn build_list(toplevels: &HashMap<ObjectId, HandleInfo>)
      -> Vec<(String,String)>`: all current toplevels as (app_id,title), the
      activated one FIRST (stable order otherwise: insertion order is fine; the
      HashMap has no order, so sort by (app_id,title) for determinism after
      promoting the activated one).
  - WRITE unit tests for these three (construct synthetic byte arrays /
    HandleInfo maps): activated-present, activated-absent (empty workspace),
    multiple-toplevels ordering, decode edge cases (empty array, 4-byte == 2,
    8-byte with 2 in 2nd slot, value 0/1/3 must NOT count).
  - WHY FIRST: these are the load-bearing correctness logic and the ONLY things
    unit-testable without a compositor (GOTCHA-13). Get them green before the
    wayland plumbing.

Task 3: ADD the hand-rolled wlr Dispatch impls (research §7)
  - IMPLEMENT `impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for DispatchState`:
      on `MgrEvent::Toplevel { toplevel }` →
      `self.toplevels.insert(toplevel.id(), HandleInfo::default())`. (The
      new_id is typed in the XML so wayland-client auto-creates the proxy; if
      event-created-child routing needs help, add
      `wayland_client::event_created_child!(...)` mirroring sctk's
      foreign_toplevel_list.rs:125-127 — but the typed new_id should suffice.)
  - IMPLEMENT `impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for DispatchState`:
      match `HandleEvent::{Title{title}, AppId{app_id}, State{state}, Done, Closed,
      _}`:
        Title/AppId/State → update pending_* (State uses decode_activated)
        Done → commit pending→current; call `self.recompute_and_notify()`
        Closed → `handle.destroy()`; remove from map; `self.recompute_and_notify()`
  - IMPLEMENT `impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for
      DispatchState` with an EMPTY body (required by registry_queue_init — GOTCHA,
      research §5; the registry list is maintained internally).
  - IMPLEMENT `impl DispatchState { fn recompute_and_notify(&mut self) }`:
      let new = recompute_focus(&self.toplevels).unwrap_or_default();
      { let mut cell = self.last_focus.lock().unwrap(); if *cell == Some(new.clone())
        { return; } *cell = Some(new.clone()); }   // compare+update under ONE lock
      *self.list_snapshot.lock().unwrap() = build_list(&self.toplevels);
      let wi = WindowInfo::new(new.0, new.1);      // built under lock, notify AFTER (GOTCHA-9)
      drop — lock released — then notifier::notify_qmk(&wi, self.verbose).

Task 4: ADD probe_available (research §10)
  - IMPLEMENT `pub(crate) fn probe_available(_verbose: bool) -> Result<(), String>`:
      let conn = Connection::connect_to_env()
          .map_err(|e| format!("cannot connect to Wayland ($WAYLAND_DISPLAY?): {e}"))?;
      // minimal state just to satisfy registry_queue_init's Dispatch bound:
      let (globals, _queue) = registry_queue_init::<ProbeState>(&conn)
          .map_err(|e| format!("Wayland registry init failed: {e}"))?;
      let has_wlr = globals.contents().with_list(|list|
          list.iter().any(|g| g.interface == "zwlr_foreign_toplevel_manager_v1"));
      if has_wlr { Ok(()) } else {
          Err("compositor does not advertise zwlr_foreign_toplevel_manager_v1 \
               (GNOME/Mutter? falls through to the next backend)".into())
      }
    where `struct ProbeState;` with the empty `Dispatch<WlRegistry, GlobalListContents>`
    impl (or reuse DispatchState if it's cheap — a dedicated ProbeState is cleaner
    and avoids touching real state). This is side-effect-free (GOTCHA-12).
  - ADD a test: unset $WAYLAND_DISPLAY (snapshot/restore like linux.rs select_tests)
    → assert probe_available().is_err() and the err mentions Wayland/display.

Task 5: ADD list_foreground_windows (the free fn; GOTCHA-11)
  - IMPLEMENT the module-level `static SHARED_SNAPSHOT:
      OnceLock<Arc<Mutex<Vec<(String,String)>>>> = OnceLock::new();`.
  - IMPLEMENT `#[allow(dead_code)] pub fn list_foreground_windows() ->
      Vec<(String,String)>`: `SHARED_SNAPSHOT.get().map(|c|
      c.lock().unwrap().clone()).unwrap_or_default()`.
  - start() inits SHARED_SNAPSHOT with the monitor's list_snapshot Arc before
    spawning (so list_foreground_windows sees updates).

Task 6: ADD WaylandFtMonitor + WindowMonitor impl + the start()/stop() loop
  - IMPLEMENT `WaylandFtMonitor::new(verbose) -> Self`: init last_focus=None,
      list_snapshot=empty, shutdown=false, handle=None.
  - IMPLEMENT `impl WindowMonitor for WaylandFtMonitor`:
      platform_name() -> "foreign-toplevel"
      start() -> { init SHARED_SNAPSHOT; Arc-clone last_focus/list_snapshot/
        shutdown; thread::spawn(move || run_dispatch_loop(...)); store JoinHandle;
        return Ok(()) IMMEDIATELY (GOTCHA-4) }
      stop() -> { shutdown.store(true, Release); if let Some(h)=handle.take()
        { let _=h.join(); } Ok(()) }   // best-effort (GOTCHA: blocking_dispatch)
      // DO NOT override start_blocks_calling_thread (keep default false — GOTCHA-4)
  - IMPLEMENT `fn run_dispatch_loop(last_focus, list_snapshot, shutdown, verbose)`:
      let mut delay_ms = INITIAL_RECONNECT_MS;
      let fn_start = Instant::now();
      loop {
          if shutdown.load(Acquire) { return; }      // stop() took effect (GOTCHA)
          let attempt_start = Instant::now();
          // connect + bind
          let conn = match Connection::connect_to_env() { Ok(c)=>c, Err(e)=>{log+backoff;continue} };
          let (globals, mut queue) = match registry_queue_init::<DispatchState>(&conn)
              { Ok(x)=>x, Err(e)=>{log+backoff;continue} };
          let _mgr: ZwlrForeignToplevelManagerV1 = match globals.bind(
              &queue.handle(), 1..=3, ()) { Ok(m)=>m, Err(e)=>{log+backoff;continue} };
          let mut state = DispatchState { toplevels: HashMap::new(),
              last_focus: last_focus.clone(), list_snapshot: list_snapshot.clone(), verbose };
          // dispatch until compositor death
          loop {
              match queue.blocking_dispatch(&mut state) {
                  Ok(_) => {}
                  Err(DispatchError::Backend(e)) => { log; break; }   // reconnect (GOTCHA-8)
                  Err(DispatchError::BadMessage{..}) => { log; /* continue, don't reconnect */ }
              }
              if shutdown.load(Acquire) { return; }
          }
          // backoff (GOTCHA-7): reset if stable ≥ STABLE_CONNECTION_THRESHOLD
          if attempt_start.elapsed() >= STABLE_CONNECTION_THRESHOLD { delay_ms = INITIAL_RECONNECT_MS; }
          thread::sleep(Duration::from_millis(delay_ms));
          delay_ms = std::cmp::min(delay_ms * 3, MAX_RECONNECT_MS);
      }
    NOTE: on the FIRST attempt failing fast (within 2s of start) under no-Wayland,
    consider returning Err from start() — but start() has already spawned + returned,
    so log + keep reconnecting (the monitor is "running" but waiting for a compositor;
    parity with how a tray app survives a compositor restart). Keep this behavior.
  - Wire SHARED_SNAPSHOT init in start() BEFORE spawn.

Task 7: EDIT src/platforms/linux.rs — replace stub + add construct arm
  - REPLACE the `wayland_probe` STUB body:
        #[cfg(feature = "wayland")]
        fn wayland_probe(_verbose: bool) -> Result<(), String> {
            crate::platforms::wayland_ft::probe_available(_verbose)
        }
  - ADD to `construct_backend` (next to the hyprland/x11 arms):
        #[cfg(feature = "wayland")]
        "foreign-toplevel" => Ok(Box::new(
            crate::platforms::wayland_ft::WaylandFtMonitor::new(verbose))),
  - DO NOT touch linux_backend_candidates() (the "foreign-toplevel" row exists).
  - DO NOT touch the `#![allow(unexpected_cfgs)]` or the existing comment about
    features (it's still accurate — features land in P2.M1.T2.S2 which is parallel).

Task 8: EDIT src/platforms/mod.rs — declare module + route list_foreground_windows
  - ADD `#[cfg(all(target_os = "linux", feature = "wayland"))] mod wayland_ft;`
    near `mod hyprland;` / the x11 declaration.
  - In `list_foreground_windows()`, add an arm BEFORE the hyprland one:
        #[cfg(all(target_os = "linux", feature = "wayland"))]
        return wayland_ft::list_foreground_windows();
    (priority #1 — matches select_linux_backend ordering). Keep the existing
    cfg_attr allow(dead_code) and the trailing default-return Vec::new() arms.
  - UPDATE the list_foreground_windows doc comment to mention wayland_ft.

Task 9: VALIDATE — build matrix + tests + grep guards
  - RUN: cargo build --bin qmkonnect                       (default; wayland_ft compiles)
  - RUN: cargo build --bin qmkonnect --no-default-features (wayland_ft ABSENT; probe stub + construct arm absent)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1    (new helper tests + probe Err-path; no regression)
  - RUN: grep -rn 'ForeignToplevelManager' src/            (empty — GOTCHA-1)
  - RUN: grep -rn 'foreign_toplevel_list' src/platforms/wayland_ft.rs  (empty — we don't use the EXT wrapper; GOTCHA-14)
  - RUN: git diff --stat                                  (ONLY the 3 files)
```

### Implementation Patterns & Key Details

```rust
// === decode_activated — the load-bearing correctness helper (GOTCHA-3) ===
fn decode_activated(state: &[u8]) -> bool {
    // wlr state event = packed LE u32 flags; activated == 2.
    state.chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .any(|v| v == 2)
}

// === focus recompute — pure, unit-tested (GOTCHA-10 empty-workspace) ===
fn recompute_focus(toplevels: &HashMap<ObjectId, HandleInfo>) -> Option<(String, String)> {
    toplevels.values()
        .find(|t| t.current_activated)
        .map(|t| (t.current_app_id.clone(), t.current_title.clone()))
    // None ⟺ no activated toplevel ⟺ caller emits WindowInfo{"",""}.
}

// === recompute_and_notify — dedup under ONE lock, notify AFTER (GOTCHA-9) ===
impl DispatchState {
    fn recompute_and_notify(&mut self) {
        let new = recompute_focus(&self.toplevels).unwrap_or_default(); // ("","") if none
        let changed = {
            let mut cell = self.last_focus.lock().unwrap();
            if *cell == Some(new.clone()) { return; }
            *cell = Some(new.clone());
            true
        };
        *self.list_snapshot.lock().unwrap() = build_list(&self.toplevels);
        let wi = WindowInfo::new(new.0, new.1);
        if let Err(e) = notifier::notify_qmk(&wi, self.verbose) {
            eprintln!("wayland_ft: notify_qmk failed: {e}");
        }
    }
}

// === Dispatch<ZwlrForeignToplevelHandleV1, ()> — the event router ===
impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for DispatchState {
    fn event(&mut self, handle: &ZwlrForeignToplevelHandleV1, event: HandleEvent,
             _data: &(), _conn: &Connection, _qh: &QueueHandle<Self>) {
        let id = handle.id();
        let info = self.toplevels.entry(id.clone()).or_default();
        match event {
            HandleEvent::Title { title }   => info.pending_title = title,
            HandleEvent::AppId { app_id }  => info.pending_app_id = app_id,
            HandleEvent::State { state }   => info.pending_activated = decode_activated(&state),
            HandleEvent::Done => {
                info.current_app_id = info.pending_app_id.clone();
                info.current_title = info.pending_title.clone();
                info.current_activated = info.pending_activated;
                drop(info); // release the borrow before recompute_and_notify mutates self
                self.recompute_and_notify();
            }
            HandleEvent::Closed => {
                handle.destroy();
                drop(info);
                self.toplevels.remove(&id);
                self.recompute_and_notify();   // may now be empty workspace
            }
            _ => {}
        }
    }
}

// === Dispatch<ZwlrForeignToplevelManagerV1, ()> — toplevel creation ===
impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for DispatchState {
    fn event(&mut self, _mgr: &ZwlrForeignToplevelManagerV1, event: MgrEvent,
             _data: &(), _conn: &Connection, _qh: &QueueHandle<Self>) {
        if let MgrEvent::Toplevel { toplevel } = event {
            self.toplevels.insert(toplevel.id(), HandleInfo::default());
            // app_id/title/state arrive as subsequent events; Done commits.
        }
    }
}

// === start() — spawn-and-return (GOTCHA-4) ===
fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    // Publish the list snapshot for list_foreground_windows() (GOTCHA-11).
    let _ = SHARED_SNAPSHOT.set(Arc::clone(&self.list_snapshot));
    let last_focus = Arc::clone(&self.last_focus);
    let list_snapshot = Arc::clone(&self.list_snapshot);
    let shutdown = Arc::clone(&self.shutdown);
    let verbose = self.verbose;
    self.handle = Some(thread::spawn(move || run_dispatch_loop(
        last_focus, list_snapshot, shutdown, verbose)));
    Ok(())   // returns IMMEDIATELY
}
```

### Integration Points

```yaml
SELECT_LINUX_BACKEND (src/platforms/linux.rs):
  - the "foreign-toplevel" candidate row ALREADY EXISTS (priority #1, feature wayland).
  - EDIT: wayland_probe stub body → delegates to wayland_ft::probe_available.
  - EDIT: construct_backend gains a "foreign-toplevel" arm.
  - RESULT: on any wlroots/KDE6/COSMIC compositor, select picks foreign-toplevel.

MOD.RS DISPATCH:
  - `mod wayland_ft;` (cfg-gated target_os=linux + feature wayland).
  - list_foreground_windows() routes to wayland_ft first (feature wayland), then hyprland.

NOTIFIER (unchanged): notify_qmk(&WindowInfo, verbose) — called exactly as Hyprland calls it.

TRAIT CONTRACT (unchanged): WindowMonitor: Send; keep start_blocks_calling_thread() default false.

CARGO.TOML: NONE (owned by P2.M1.T2.S2; wayland feature + sctk/wayland-client deps already declared).
DATABASE/ROUTES: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect --no-default-features   # parses; wayland_ft ABSENT (proves the cfg gate)
cargo build --bin qmkonnect                          # default; wayland_ft COMPILES
# Expected: clean. If cargo errors on a missing type/impl (e.g. ForeignToplevelManager,
# or a Dispatch bound), READ research/notes.md §§1,5,7 — the exact API is there.

# Guard: no reference to the nonexistent sctk wlr types (GOTCHA-1):
grep -rn 'ForeignToplevelManager\|ForeignToplevelHandler\|ForeignToplevelState' src/
# Expected: empty.
grep -rn 'foreign_toplevel_list' src/platforms/wayland_ft.rs
# Expected: empty (we don't use the EXT wrapper — GOTCHA-14).
```

### Level 2: Unit Tests (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# The pure helpers + probe Err-path:
cargo test --bin qmkonnect -- --test-threads=1 wayland_ft
# Expected: decode_activated_* (value 2 present/absent, value 0/1/3 ignored, empty,
#   multi-chunk), recompute_focus_* (activated present, empty-workspace None,
#   multiple-toplevels), build_list_* (activated-first ordering) all pass.

# Full suite (no regression — shared debouncer state ⇒ single-threaded):
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL green. The select_tests in linux.rs are unaffected (they don't
# assert the wayland probe's Ok path; the unset-$WAYLAND_DISPLAY probe test is NEW).
```

### Level 3: Feature-toggle + selection wiring (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
# default build includes the backend:
cargo build --bin qmkonnect
# minimal build excludes it (proves the cfg gate + that construct_backend's new arm
# is also feature-gated out):
cargo build --bin qmkonnect --no-default-features
# force-build just the wayland deps (smoke):
cargo build --bin qmkonnect --features wayland

# Verify the selector wiring without a compositor (the probe returns a real Err now):
# (unset WAYLAND so foreign-toplevel is unavailable → should fall through; on a
# dev box with no Hyprland either, it reaches x11 or no-backend)
WAYLAND_DISPLAY= ./target/debug/qmkonnect -v 2>&1 | grep -i 'foreign-toplevel\|probing\|available'
# Expected: verbose log shows "probing 'foreign-toplevel'…" → "unavailable: cannot
# connect to Wayland…" (the NEW probe message, not the old "not yet implemented" stub).
```

### Level 4: Creative & Domain-Specific Validation (compositor required — DEFERRED)

> A real compositor (Sway/Hyprland/KWin) is NOT available on the dev box / CI, so
> the live focus-tracking loop is NOT validated here — it is validated by the pure
> helpers (Level 2) which encode ALL the load-bearing logic (decode + recompute +
> empty-workspace). Manual live validation (when a compositor is available): run
> `sway` (or a nested `weston`), launch QMKonnect under it, switch focus between
> two windows, and confirm the tray "Show Window Information" + the keyboard layer
> change. This is the AGENTS.md Linux dev loop's domain; record results in the
> handoff, not as a PRP gate.

```bash
# (Deferred — requires a compositor; documented for the implementer's manual check.)
# Under Sway/Hyprland:
#   ./target/debug/qmkonnect -v 2>&1 | grep -iE 'foreign-toplevel|notify_qmk|activated'
# Switch focus between two windows → expect a notify on each change + empty-window
# notify when focusing the empty desktop.
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` (default) compiles — `wayland_ft.rs` is compiled in.
- [ ] `cargo build --bin qmkonnect --no-default-features` compiles — `wayland_ft.rs`, the probe, and the construct arm are all ABSENT.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` passes — new helper/probe tests green, no regression.
- [ ] `git diff --stat` shows ONLY `src/platforms/wayland_ft.rs` + `src/platforms/linux.rs` + `src/platforms/mod.rs`.

### Feature Validation (parity with PLATFORMS.md §7)
- [ ] `WaylandFtMonitor: WindowMonitor` with `platform_name()=="foreign-toplevel"`, default `start_blocks_calling_thread()==false`, spawn-and-return `start()`.
- [ ] Focus decoded from the wlr `state` event's LE-u32 `activated` (value 2) — `decode_activated` unit-tested (GOTCHA-3).
- [ ] Empty workspace (no activated toplevel) emits `WindowInfo{"",""}` — `recompute_focus` returns None → unwrapped to `("","")` (GOTCHA-10).
- [ ] Reconnect on `DispatchError::Backend` with backoff 100→10_000 ×3, reset after 5s stable (GOTCHA-7/8).
- [ ] `probe_available`: Err when `$WAYLAND_DISPLAY` unset; Err when wlr global not advertised; Ok when both pass (GOTCHA-12).
- [ ] `list_foreground_windows()` returns tracked toplevels, activated first (GOTCHA-11).
- [ ] `wayland_probe` stub REPLACED (delegates to real probe); `construct_backend` has the `"foreign-toplevel"` arm (GOTCHA-6).

### Code Quality Validation
- [ ] Mirrors `hyprland.rs` struct/trait/probe/list shape; reconnect consts re-declared identically (not imported — GOTCHA-7).
- [ ] `notify_qmk` called AFTER releasing the last-focus lock (GOTCHA-9).
- [ ] Struct holds no wayland objects — only `Arc<Mutex<>>`/`AtomicBool`/`JoinHandle`/`bool` (Send — GOTCHA-5).
- [ ] No reference to the nonexistent sctk wlr types (GOTCHA-1); no EXT wrapper wiring (deferred — GOTCHA-14).
- [ ] Logic factored into pure, hermetically-testable helpers (GOTCHA-13).
- [ ] Scope respected: NO Cargo.toml edit (P2.M1.T2.S2 owns it); NO select_linux_backend candidate-list edit; NO docs/PRD/tasks.json edit.

### Documentation & Deployment
- [ ] Mode A: the coverage table in PLATFORMS.md §7.2 is the reference (no docs prose added).
- [ ] The `#[cfg(...)]` gates + module doc comment cite PLATFORMS.md §7 + the sctk-0.20 correction so the next agent doesn't re-trip it.

---

## Anti-Patterns to Avoid

- ❌ Do NOT search for / `use` `smithay_client_toolkit::foreign_toplevel::*` or
      `ForeignToplevelManager`/`ForeignToplevelHandler`/`ForeignToplevelState`. They
      do NOT exist in sctk 0.20 (deleted 0.19 API). The wlr protocol is HAND-ROLLED
      via `smithay_client_toolkit::reexports::protocols_wlr::foreign_toplevel::v1::
      client::*` (GOTCHA-1/2). This is the single most likely way to waste a turn.
- ❌ Do NOT look for an `Activated` enum variant / `.contains()` on the state event.
      The wlr `state` arg generates `Event::State { state: Vec<u8> }`; decode LE-u32
      chunks, `activated == 2` (GOTCHA-3).
- ❌ Do NOT block in `start()` or override `start_blocks_calling_thread()` to true.
      wayland_ft SPAWNS the dispatch thread and returns; keep the trait default false
      (GOTCHA-4). The runner parks main / drives the tray on this contract.
- ❌ Do NOT hold wayland objects (Connection/EventQueue/QueueHandle/proxies) in the
      `WaylandFtMonitor` struct. Construct them ON the dispatch thread; the struct
      holds only `Arc<Mutex<>>`/`AtomicBool`/`JoinHandle`/`bool` (Send — GOTCHA-5).
- ❌ Do NOT call `notify_qmk` while holding the `last_focus` Mutex. Build the
      `WindowInfo` under the lock, drop it, THEN notify (GOTCHA-9 — the notifier
      takes its own locks; mirrors hyprland.rs).
- ❌ Do NOT skip the empty-workspace emit. Zero activated toplevels ⟹ emit
      `WindowInfo{"",""}` (deactivates layers — GOTCHA-10).
- ❌ Do NOT add `wayland-protocols-wlr` to Cargo.toml. It's already a transitive dep
      via sctk; use the re-export. Cargo.toml is owned by P2.M1.T2.S2 (GOTCHA-2).
- ❌ Do NOT wire the EXT protocol (`ForeignToplevelList`) in this task. It is
      functionally inert (no activation) and would add a second handler mechanism for
      zero behavioral gain. DEFER it (documented) — GOTCHA-14.
- ❌ Do NOT edit `linux_backend_candidates()` — the `"foreign-toplevel"` row already
      exists (priority #1). Only the probe body + a construct arm change (GOTCHA-6).
- ❌ Do NOT reconnect on `DispatchError::BadMessage`. That's a protocol violation
      (log + continue). Reconnect ONLY on `DispatchError::Backend` (GOTCHA-8).
- ❌ Do NOT try to import the reconnect constants from `hyprland.rs` (they're
      private). Re-declare identical consts in `wayland_ft.rs` (GOTCHA-7).
- ❌ Do NOT run tests multi-threaded (`cargo test --bin qmkonnect -- --test-threads=1`
      — shared debouncer state, Invariant 8).
- ❌ Do NOT make `list_foreground_windows()` query the compositor live. The toplevels
      live in the dispatch thread; publish a shared snapshot the free fn reads
      (GOTCHA-11).
- ❌ Do NOT edit `PRD.md`, any `tasks.json`, `prd_snapshot.md`, `Cargo.toml`, any
      `docs/*`, or any other `plan/` file than this item's own `PRP.md` + `research/`.

---

## Confidence Score: 8/10

The two highest risks for a window-monitor backend — (a) getting the Wayland API
right and (b) hermetic testability — are both resolved with source-confirmed
specificity:

- **The Wayland API is pinned to file:line.** The HEADLINE CORRECTION (sctk 0.20
  ≠ wlr; §1/notes) eliminates the #1 failure mode (chasing a nonexistent type).
  The exact wlr module path (via the `protocols_wlr` re-export, no new dep — §2),
  the exact generated event signatures (`Title{title:String}`, `AppId{app_id}`,
  `State{state:Vec<u8>}`, `Done`, `Closed` — §3/4, confirmed against
  wayland-scanner client_gen.rs:214), the LE-u32-activated-==-2 decode (§4), and
  the connect/dispatch entry points (`connect_to_env` + `registry_queue_init` +
  `GlobalList::bind` + `blocking_dispatch` + `DispatchError::Backend` — §5) are
  all read from the actual crate sources, not memory.
- **Testability is guaranteed by design.** The load-bearing logic
  (`decode_activated`, `recompute_focus`, `build_list`) is factored into PURE
  helpers that are unit-tested without a compositor (GOTCHA-13); the Dispatch
  plumbing + connect/bind are the only non-hermetic parts, and they're thin
  routing to those helpers.

The 2-point reservation is for the two genuinely-non-hermetic pieces that can't be
unit-tested on a compositor-less box: (1) **event-created-child routing** — the wlr
`toplevel` event's new_id is typed in the XML so wayland-client should auto-create
the `ZwlrForeignToplevelHandleV1` proxy and route to my `Dispatch` impl, but if it
needs the `event_created_child!` macro (as sctk's ext module does at
foreign_toplevel_list.rs:125), the agent must add it — this is documented in Task 3
as the fallback, so it's a known recovery, not a dead end; (2) **the live
focus-tracking loop** under a real compositor (Level 4), which is deferred to
manual validation per AGENTS.md and cannot be a PRP gate. Neither blocks one-pass
compilation or the pure-logic correctness. Scope is tight (3 files, no Cargo.toml,
explicit non-overlap with the parallel P2.M1.T2.S2), and the existing-pattern file
to mirror (`hyprland.rs`) is identified symbol-for-symbol.
# PRP — P2.M4.T1.S1: Core AT-SPI Fallback Backend (`src/platforms/atspi.rs`)

> **Repo under change:** QMKonnect (`/home/dustin/projects/qmkonnect`).
> **Files CREATED:** `src/platforms/atspi.rs` (the new backend).
> **Files MODIFIED:** `src/platforms/mod.rs` (add `mod atspi;` decl + an
> `atspi` arm in `list_foreground_windows`), `src/platforms/linux.rs` (replace
> the `atspi_probe` stub + add the `atspi` arm to `construct_backend`),
> `docs/troubleshooting.md` (Mode-A: best-effort nature + a11y-enable requirement).
> **Files NOT touched (owned by OTHER tasks — see §Scope Boundary):** `Cargo.toml`
> (the `atspi` dep + feature are already in `default` — P2.M1.T2.S2, DONE),
> `src/core/mod.rs` / `LinuxConfig` (atspi is priority #4 in the candidate list
> already; no config field needed — poll is a hardcoded 1000 ms const, see §Why),
> `src/runners/linux.rs` (P2.M3.T2.S2 — implementing in parallel; S2 consumes a
> `gnome::probe_available` it owns and moves its own call site; this PRP does not
> touch it), `src/platforms/gnome.rs` (P2.M3.T2.S1 — COMPLETE, consumed as a
> PATTERN only, not edited), `docs/installation.md` /
> `docs/qmk-integration.md` (S1/S2 of P2.M3 own those; this task uses
> `docs/troubleshooting.md`), `.github/workflows/*`, `PRD.md`, `tasks.json`.
>
> **What it does:** PLATFORMS.md §9 — the last-ditch Linux window backend. It
> connects to the AT-SPI/a11y bus, subscribes to `object:state-changed:focused`,
> tracks the focused accessible, reports `app_class` = the focused accessible's
> **application** `Name` (readable name, UNRELIABLE — see limitations) and
> `title` = the focused accessible's own `Name`, with a 1000 ms drift-poll. It is
> selected as **priority #4** in `select_linux_backend`
> (foreign-toplevel → gnome → hyprland → **atspi** → x11) and is the GNOME
> fallback when the Shell extension (§8) isn't installed + the emergency path
> elsewhere. **Best-effort, not primary.**

---

## Goal

**Feature Goal**: Ship a working, best-effort AT-SPI window-focus backend
(`src/platforms/atspi.rs`, Cargo feature `atspi`) that tracks the focused
accessible on the a11y bus and notifies QMK — mirroring the structure and
threading posture of the already-complete GNOME backend (`src/platforms/gnome.rs`)
— using the **typed** `atspi` 0.30 proxies + events over a **pure `zbus::blocking`**
connection (NO async runtime — see §Why / GOTCHA-1), wired into
`select_linux_backend` as priority #4.

**Deliverable** (concrete; validates on the dev box TODAY — a Hyprland box, so
the hard gates are `cargo build` + hermetic unit tests; the live a11y-bus run is
a documented manual Level-4 step on a GNOME/Ubuntu box with a11y ON):
- `src/platforms/atspi.rs` — `pub struct AtspiMonitor` implementing `WindowMonitor`
  (`platform_name() == "atspi"`; spawn-and-return `start()`; best-effort `stop()`),
  a `pub(crate) fn probe_available(verbose) -> Result<(), String>` (a11y-bus
  presence), a `pub fn list_foreground_windows() -> Vec<(String,String)>`
  (best-effort single row), two worker-thread loops (signal + 1000 ms poll), a
  shared dedup helper, and a hermetic `#[cfg(test)] mod tests`.
- `src/platforms/mod.rs` — `#[cfg(all(target_os = "linux", feature = "atspi"))] mod atspi;`
  + an `atspi` arm in the `list_foreground_windows()` cfg ladder.
- `src/platforms/linux.rs` — replace the `atspi_probe` stub (currently returns a
  fixed `Err`) with a call to `atspi::probe_available`; add the `"atspi"` arm to
  `construct_backend`.
- `docs/troubleshooting.md` — a Mode-A subsection documenting the best-effort
  nature + the "enable Assistive Technology / Screen Reader" requirement.

**Success Definition**:
- `cargo build --release` (default features — includes `atspi`) succeeds with NO
  new warnings beyond the repo baseline, and `cargo build --no-default-features`
  still compiles (atspi is optional).
- `cargo test --bin qmkonnect -- --test-threads=1` passes; NEW hermetic tests
  cover the dedup helper (unchanged / change / empty-is-real-value) and a
  focus-filter predicate (`State::Focused && enabled` → track; other states /
  `enabled == false` → ignore).
- `qmkonnect -v` on a box with NO a11y bus prints `select_linux_backend: probing
  'atspi'… → 'atspi' unavailable: <the probe's Err>` and falls through to the
  next backend (no panic, no hang).
- `git diff --stat` shows ONLY `src/platforms/atspi.rs`, `src/platforms/mod.rs`,
  `src/platforms/linux.rs`, `docs/troubleshooting.md` (NO `Cargo.toml`,
  `src/core/*`, `src/runners/*`, `gnome.rs`, other docs, CI, or `tasks.json`).

## User Persona (if applicable)

**Target User**: Linux desktop users on a compositor that (a) has no
foreign-toplevel Wayland protocol (GNOME/Mutter), (b) has no QMKonnect Shell
extension installed, and (c) DOES have accessibility enabled (screen-reader /
AT users, testers, anyone who toggled "Assistive Technology"). This is the
single largest population that lands on AT-SPI.

**Use Case**: A GNOME user installed QMKonnect but not the Shell extension, AND
they have a screen reader on (so the a11y bus is up). Window focus changes
should still switch their QMK layer — best-effort.

**User Journey**: enable Accessibility in desktop Settings → launch `qmkonnect`
→ foreign-toplevel unavailable (GNOME), GNOME backend unavailable (no extension)
→ AT-SPI probe succeeds (a11y bus present) → AT-SPI selected → focus changes
emit `state-changed:focused` → QMK layer switches. If a11y is OFF, the AT-SPI
probe fails and the tray shows a no-backend posture (the GNOME first-run hint
from P2.M3.T2.S2 fires regardless).

**Pain Points Addressed**: Without this backend, GNOME-without-extension + a11y-on
users get X11 (if available) or nothing on pure-Wayland. AT-SPI gives them a
functioning (if imperfect) path with zero extra install steps.

## Why

- **§9 is the documented fallback of last resort.** It exists specifically for
  "GNOME without the extension" + "any compositor with accessibility enabled."
  It is the backend that makes the §8.4 "AT-SPI may run meanwhile" promise real.
- **The async-only crate is the headline obstacle — and there's a clean fix.**
  The `atspi` crate's high-level `AccessibilityConnection` is `async fn`-only
  (`atspi-connection-0.14.0/src/lib.rs`: `new`, `register_event`, `event_stream`
  are all `async`), and the project deliberately has NO tokio/async-std/smol
  runtime (grep `Cargo.toml`). Adding tokio for one backend is heavyweight and
  breaks the project's "minimal service build" posture. **The fix:** skip
  `AccessibilityConnection` entirely and use the `atspi` crate's typed
  `proxy::*Blocking` proxies + typed `events::object::StateChangedEvent` over a
  raw `zbus::blocking` connection — exactly what `src/platforms/gnome.rs` already
  does for the GNOME D-Bus client. This captures 100% of the `atspi` crate's
  value (typed proxies, typed events, the canonical match-rule strings, and the
  knowledge of how to find the a11y bus address via `org.a11y.Bus.GetAddress`)
  with no runtime. (Research `plan/007_fb356ba503b4/P2M4T1S1/research/notes.md`
  §0–§4 — every API below was read from the actual crate source in the local
  cargo registry cache.)
- **Poll is a hardcoded 1000 ms const, not a config field.** The contract
  (`PLATFORMS.md §9` + the item description) says "every 1000 ms" — a fixed
  value, not "configurable." Adding an `atspi_poll_interval_ms` to
  `core::LinuxConfig` would broaden scope into the shared Config schema +
  `render_config_body` + the documented defaults string (and risk conflict with
  the parallel P2.M3.T2.S2 runner work). Keep it as `const DEFAULT_POLL_MS:
  u64 = 1000;` inside `atspi.rs` (mirror `gnome.rs`'s `DEFAULT_POLL_MS`). A
  future task can promote it to config if needed.
- **Honest about limitations.** §9 lists them: app_class is the readable name
  (not WM_CLASS), titles vary (focused accessible ≠ toplevel), apps without an
  a11y bridge are invisible. The PRP bakes these into the code comments AND the
  troubleshooting doc so no one mistakes this for a primary backend.

## What

A `WindowMonitor` backend that:
1. **`probe_available(verbose)`**: returns `Ok` iff the a11y bus is present —
   `org.a11y.Bus` is owned on the session bus **OR** `$ATSPI_BUS_ADDRESS` is set.
   Cheap + side-effect-free (one `name_has_owner` round-trip). Returns a helpful
   `Err` pointing at enabling AT when absent. (GOTCHA-8: "present" ≠ "useful.")
2. **`start()`**: spawn-and-return two worker threads (signal-loop + poll-loop),
   each with its OWN `zbus::blocking::Connection`; return `Ok(())` promptly.
3. **Signal loop**: connect to session bus → `BusProxyBlocking::get_address()` →
   connect to the a11y bus → `RegistryProxyBlocking::register_event("object:state-changed")`
   → `MessageIterator::for_match_rule` on
   `type='signal',interface='org.a11y.atspi.Event.Object',member='StateChanged'`
   → for each `Message`, `StateChangedEvent::try_from(&msg)` → filter
   `state == State::Focused && enabled` → resolve `title` = focused accessible's
   `name()`, `app_class` = its application's `name()` → dedup → `notify_qmk`.
4. **Poll loop** (every `DEFAULT_POLL_MS` = 1000 ms): own a11y connection; each
   tick, re-query the **last focused accessible** (cached `ObjectRefOwned`) for
   its current title/app → dedup (drift-catcher for in-place title changes). No
   focus cached yet → no-op (GOTCHA-7: AT-SPI has no get-focused RPC).
5. **`list_foreground_windows()`**: synchronous best-effort single row from the
   last focused accessible; empty on any failure.
6. **Docs**: troubleshooting.md Mode-A subsection on best-effort + enabling a11y.

### Success Criteria
- [ ] `src/platforms/atspi.rs` exists, gated `#![cfg(all(target_os = "linux", feature = "atspi"))]`,
      implements `WindowMonitor` (`platform_name()=="atspi"`, `start()`,
      `stop()`), and exposes `pub(crate) fn probe_available(verbose) ->
      Result<(),String>` + `pub fn list_foreground_windows() -> Vec<(String,String)>`.
- [ ] `AtspiMonitor` is `Send` (holds only `Arc<Mutex<_>>`/`Arc<AtomicBool>`/
      `Option<JoinHandle>`/bool — mirrors `GnomeMonitor`).
- [ ] `start()` spawns TWO threads (signal + poll), each with its OWN
      `zbus::blocking::Connection`, and returns promptly (default
      `start_blocks_calling_thread() == false`).
- [ ] The signal loop uses `atspi::proxy::bus::BusProxyBlocking::get_address()` +
      `zbus::blocking::ConnectionBuilder::address` (NO tokio / NO
      `AccessibilityConnection`) and `atspi::events::object::StateChangedEvent`
      with the `state == State::Focused && enabled` filter.
- [ ] `app_class` = the focused accessible's **application** `Name`; `title` =
      the focused accessible's own `Name` (both via `AccessibleProxyBlocking::name()`).
- [ ] `probe_available` returns `Ok` when `org.a11y.Bus` is owned OR
      `$ATSPI_BUS_ADDRESS` is set; a helpful `Err` otherwise (mentions enabling AT).
- [ ] `mod atspi;` declared in `mod.rs` under the exact cfg; `atspi_probe` stub
      in `linux.rs` replaced to call `atspi::probe_available`; `"atspi"` arm added
      to `construct_backend`; atspi is priority **#4** in the candidate list
      (already true — DO NOT reorder).
- [ ] `docs/troubleshooting.md` has a Mode-A subsection under "Linux Issues".
- [ ] `cargo build --release` + `cargo build --no-default-features` both succeed;
      `cargo test --bin qmkonnect -- --test-threads=1` passes with NEW hermetic tests.

## All Needed Context

### Context Completeness Check

_Pass._ An agent with no prior knowledge of this codebase can implement this from
this PRP + the repo because: (a) the **in-repo sibling to clone** is named and
pin-pointed (`src/platforms/gnome.rs` — same struct shape, same two-thread model,
same dedup helper, same probe/list functions, same test posture); (b) every
`atspi` 0.30 / `zbus` 5.17 API used is quoted verbatim from the crate source
with the exact type paths + the exact constant strings (match-rule, registry
event string) in §Reference Implementation / §Known Gotchas; (c) the async-only
trap and its blocking fix (the single biggest risk) is explained with the exact
imports to use and the exact API to AVOID; (d) the three precise edit sites
(stub at `linux.rs:179`, `construct_backend` match, `mod.rs` decl + ladder) are
pinned to current locations; (e) the AT-SPI "no get-focused RPC" reality (the
second-biggest risk, which a naïve agent would get wrong by inventing a
`registry.get_focus()` call) is called out with the correct drift-poll design;
(f) the scope boundary with Cargo.toml / core / runners / gnome.rs / other docs
is explicit so the agent doesn't sprawl.

### Documentation & References

```yaml
# MUST READ — the authoritative contract (the WHAT + the limitations).
- file: spec/PLATFORMS.md
  why: "§9 AT-SPI Fallback Backend (verified present): crate `atspi`; subscribe to
        `object:state-changed:focused`; app_class = focused accessible's APPLICATION
        Name; title = focused accessible's Name; 1000 ms poll fallback; availability
        = org.a11y.Bus owned OR $ATSPI_BUS_ADDRESS; a11y is OFF until 'Assistive
        Technology / Screen Reader' enabled — document it; known limitations
        (readable name not WM_CLASS; titles vary; apps without a11y bridge invisible;
        prefer the GNOME Shell extension §8). §6 (priority order: foreign-toplevel →
        gnome → hyprland → atspi → x11, atspi=#4) + §1.1 (what application_class is)
        + §1.2 (titles)."

# MUST READ — the in-repo backend to CLONE (structure, threading, dedup, tests).
- file: src/platforms/gnome.rs
  why: "The closest sibling: same two-thread spawn-and-return model, same Arc/Mutex
        dedup (`apply_and_notify` — release lock BEFORE notify_qmk), same
        `probe_available` via `fdo::DBusProxy::name_has_owner`, same
        `list_foreground_windows` single-row shape, same `#[cfg(test)] mod tests`
        (dedup + empty-workspace + probe-err), same `start_blocks_calling_thread()`
        default-false posture, same best-effort `stop()` (GOTCHA-7). Copy the
        struct field-for-field; swap the D-Bus plumbing for the atspi a11y-bus
        plumbing described in §Reference Implementation."
  pattern: "struct GnomeMonitor { last_focus, shutdown, signal_handle, poll_handle, verbose }
            + fn apply_and_notify + fn run_signal_loop + fn run_poll_loop + probe_available
            + list_foreground_windows + #[cfg(test)] mod tests"
  gotcha: "GOTCHA-3 (each thread owns its OWN blocking Connection) + GOTCHA-4 (release
           last_focus lock before notify_qmk) + GOTCHA-7 (best-effort stop) carry over
           VERBATIM."

# MUST READ — the wiring sites (probe stub + construct_backend).
- file: src/platforms/linux.rs
  why: "L179 `fn atspi_probe(_verbose) -> Result<(),String> { Err(\"AT-SPI backend
        not yet implemented (P2.M4)\") }` — REPLACE body with
        `crate::platforms::atspi::probe_available(_verbose)`. `construct_backend()`
        match (after the `gnome` arm): ADD
        `#[cfg(feature=\"atspi\")] \"atspi\" => Ok(Box::new(crate::platforms::atspi::AtspiMonitor::new(verbose))),`.
        `linux_backend_candidates()` ALREADY lists atspi as priority #4 — DO NOT
        touch the order. The file's `#![allow(unexpected_cfgs)]` (L3) is fine; the
        `atspi` feature now exists (P2.M1.T2.S2 DONE)."

# MUST READ — the module-declaration site + list_foreground_windows ladder.
- file: src/platforms/mod.rs
  why: "L22 `#[cfg(all(target_os=\"linux\",feature=\"gnome\"))] mod gnome;` — ADD the
        atspi twin directly after it: `#[cfg(all(target_os=\"linux\",feature=\"atspi\"))]
        mod atspi;`. `list_foreground_windows()` cfg ladder (~L300+): ADD an atspi arm
        reached only when wayland+gnome+hyprland are all off (mirror the gnome arm's
        `not(feature=\"wayland\"),not(feature=\"hyprland\")` gating + add `not(gnome)`),
        calling `return atspi::list_foreground_windows();`. The `WindowMonitor` trait
        (L18) + `create_monitor` (no change — forced=\"atspi\" already flows through)."

# MUST READ — the Cargo dep/features (DO NOT EDIT — confirm only).
- file: Cargo.toml
  why: "L53 `atspi = { version=\"0.30\", optional=true }`; L143 `atspi=[\"dep:atspi\"]`;
        L137 `default=[\"wayland\",\"gnome\",\"atspi\",\"hyprland\",\"macos\",\"linux-tray\"]`.
        The dep + feature + default-membership ALL exist (P2.M1.T2.S2 DONE). Do NOT
        edit Cargo.toml. atspi 0.30's DEFAULT features give us proxies+common(wrappers)+zbus."

# MUST READ — the consumed core helpers (exact signatures, verified).
- file: src/core/notifier.rs
  why: "L1698 `pub fn notify_qmk(window_info: &WindowInfo, verbose: bool) ->
        Result<(), Box<dyn Error+Send+Sync>>`. Takes the global debouncer locks
        internally — hence release last_focus BEFORE calling (gnome.rs GOTCHA-4)."
- file: src/core/types.rs
  why: "`pub struct WindowInfo { app_class: String, title: String }` + `pub fn new(app_class, title)`.
        Build as `WindowInfo::new(app_class, title)` before notify_qmk (same as gnome)."
- file: src/core/mod.rs
  why: "L131 `pub fn now_ms() -> u128` (the `[Nms]` verbose timestamp). NO config
        field needed for atspi — the 1000 ms poll is a local const."

# MUST READ — the Mode-A doc target.
- file: docs/troubleshooting.md
  why: "Under `## Linux Issues` (L319) add a Mode-A subsection: 'AT-SPI (a11y) backend
        is best-effort + requires enabling Assistive Technology'. Cover: (1) it's a
        fallback of last resort (GNOME-without-extension + a11y-on); (2) app_class is
        the readable name not WM_CLASS (Electron/sandboxed may show 'python3'/'chrome'/empty);
        (3) titles are the focused accessible not the window title; (4) enable a11y:
        GNOME `gsettings set org.gnome.desktop.interface toolkit-accessibility true`
        or Settings → Accessibility; (5) prefer the GNOME Shell extension (link §8)."

# REFERENCE — the atspi/zbus API (read from the local crate cache, not docs.rs).
- file: ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/atspi-connection-0.14.0/src/lib.rs
  why: "PROVES AccessibilityConnection is async-only (new/register_event/event_stream
        are `async fn`) — the justification for the blocking-zbus design. DO NOT use it."
- file: ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/atspi-proxies-0.14.0/src/accessible.rs
  why: "AccessibleProxy (#[zbus::proxy] ⇒ AccessibleProxyBlocking auto-generated).
        L221 `#[zbus(property)] fn name(&self) -> zbus::Result<String>` (the Name = title).
        L40 `fn get_application(&self) -> zbus::Result<ObjectRefOwned>` (→ app object for app_class).
        L159 `fn get_state(&self) -> zbus::Result<StateSet>` (poll seed; contains(State::Focused))."
- file: ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/atspi-proxies-0.14.0/src/registry.rs
  why: "RegistryProxyBlocking::register_event(&str) — pass \"object:state-changed\" so apps EMIT."
- file: ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/atspi-proxies-0.14.0/src/bus.rs
  why: "BusProxyBlocking::new(&session_bus)?.get_address() -> zbus::Result<String> (the a11y bus addr)."
- file: ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/atspi-common-0.14.0/src/events/object.rs
  why: "L645 constants: REGISTRY_EVENT_STRING=\"object:state-changed\"; MATCH_RULE_STRING=
        \"type='signal',interface='org.a11y.atspi.Event.Object',member='StateChanged'\".
        L351 `struct StateChangedEvent { item: ObjectRefOwned, state: State, enabled: bool }`."
- file: ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/atspi-common-0.14.0/src/macros.rs
  why: "L298 `impl_from_dbus_message!(StateChangedEvent)` ⇒
        `impl TryFrom<&zbus::Message> for StateChangedEvent` ⇒ `StateChangedEvent::try_from(&msg)`."
- file: ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/zbus-5.17.0/src/blocking/message_iterator.rs
  why: "`MessageIterator::for_match_rule(rule, &conn, Some(8))` — blocking signal iterator that
        AUTO-REGISTERS the match rule on the daemon AND AUTO-DEREGISTERS on drop (doc-confirmed)."
- file: ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/zbus-5.17.0/src/blocking/connection/builder.rs
  why: "L74 `ConnectionBuilder::address(addr)`, L264 `.build()` — connect to the a11y bus from the
        address string returned by BusProxyBlocking::get_address()."

# REFERENCE — the design-decision research brief (full API write-up).
- file: plan/007_fb356ba503b4/P2M4T1S1/research/notes.md
  why: "The complete, source-verified atspi 0.30 + zbus 5.17 blocking API reference +
        the poll-fallback honesty note + the exact import list + the wiring diff."

# REFERENCE — the parallel sibling (CONSUMED as a contract, not edited).
- file: plan/007_fb356ba503b4/P2M3T2S2/PRP.md
  why: "S2 rewrites `src/runners/linux.rs::maybe_gnome_first_run_notify` (consumes
        gnome::probe_available). This PRP does NOT touch runners/linux.rs — no
        conflict. S2 uses docs/qmk-integration.md; this PRP uses docs/troubleshooting.md
        — no doc conflict. S1 (gnome.rs) is COMPLETE — consumed as a pattern only."
```

### Current Codebase tree (relevant subset)

```bash
spec/PLATFORMS.md              # §9 = authoritative contract (the WHAT + limitations)           ← READ
src/platforms/gnome.rs         # THE sibling to clone (struct, 2 threads, dedup, probe, tests) ← READ (pattern)
src/platforms/linux.rs         # atspi_probe stub (L179) + construct_backend match             ← EDIT
src/platforms/mod.rs           # mod decl site (L22, after gnome) + list_foreground_windows    ← EDIT
src/platforms/x11.rs           # file-level `#![cfg(target_os="linux")]` gate pattern           ← READ (pattern)
Cargo.toml                     # atspi dep+feature+default ALREADY exist (L53/L137/L143)       ← READ (do NOT edit)
src/core/{notifier,types,mod}.rs  # notify_qmk / WindowInfo::new / now_ms (consumed)           ← READ
docs/troubleshooting.md        # Mode-A doc target (Linux Issues, L319)                        ← EDIT
src/platforms/atspi.rs         # DOES NOT EXIST YET                                            ← CREATE
```

### Desired Codebase tree (files this task modifies/creates)

```bash
src/platforms/atspi.rs         # CREATE — AtspiMonitor + probe_available + list_foreground_windows + 2 loops + tests
src/platforms/mod.rs           # EDIT   — add `mod atspi;` decl + an atspi arm in list_foreground_windows
src/platforms/linux.rs         # EDIT   — replace atspi_probe stub body + add "atspi" construct_backend arm
docs/troubleshooting.md        # EDIT   — Mode-A subsection (best-effort + enable a11y)
# (NO Cargo.toml, src/core/*, src/runners/*, gnome.rs, other docs, CI, .gitignore, PRD.md, tasks.json)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (GOTCHA-1 — the whole design). The `atspi` crate's high-level
//   `AccessibilityConnection` is ASYNC-ONLY (atspi-connection-0.14.0/src/lib.rs:
//   `pub async fn new()`, `pub async fn register_event()`, `pub fn event_stream()`
//   returns a `Stream`). The project has NO tokio/async-std/smol runtime
//   (Cargo.toml grep). DO NOT use AccessibilityConnection, event_stream, or any
//   `.await`. Instead use the atspi crate's typed PROXIES + typed EVENT over a
//   raw `zbus::blocking` connection (same posture as src/platforms/gnome.rs).
//   This captures the crate's value (typed proxies/events/canonical strings +
//   knowledge of the a11y bus address) with zero runtime.

// CRITICAL (GOTCHA-2). atspi-proxies are `#[zbus::proxy(...)]` ⇒ the macro
//   AUTO-GENERATES a `*Blocking` variant for each trait (unless `blocking=false`,
//   which none set). So AccessibleProxyBlocking, RegistryProxyBlocking,
//   BusProxyBlocking all EXIST and are sync. Use those (NOT the async *Proxy).

// (GOTCHA-3). Each worker thread MUST own its OWN `zbus::blocking::Connection`
//   (a shared Connection serializes on its internal executor). gnome.rs has TWO
//   threads each with `Connection::session()` for exactly this reason.

// (GOTCHA-4). Release the `last_focus` Mutex lock BEFORE `notifier::notify_qmk` —
//   notify_qmk takes the global debouncer STATE/NOTIFIER locks internally, so
//   holding last_focus while notifying risks lock-ordering contention. This is a
//   free `apply_and_notify` fn with the lock dropped in its own block (copy gnome.rs).

// (GOTCHA-5 — the focus filter). StateChangedEvent fires for MANY states
//   (Focused, Focusable, Active, Selected, …) and for both enabled/disabled.
//   Track ONLY `ev.state == atspi::State::Focused && ev.enabled` (enabled ==
//   body.detail1() > 0 == focus GAINED). Ignore everything else.

// (GOTCHA-6 — app_class vs title). `title` = the focused accessible's own
//   `AccessibleProxyBlocking::name()`. `app_class` = the accessible returned by
//   `get_application()` (an ObjectRefOwned), built into ANOTHER
//   AccessibleProxyBlocking, then `.name()` = the APPLICATION's readable Name.
//   This is the readable name (NOT WM_CLASS) — inconsistent for Electron/sandboxed
//   apps (may be "python3"/"chrome"/empty). DOCUMENT as a limitation; do NOT try
//   to "fix" it — there is no WM_CLASS in AT-SPI.

// CRITICAL (GOTCHA-7 — the poll cannot "query the focused object"). AT-SPI has
//   NO single "get currently-focused accessible" RPC on the Registry/desktop
//   root (no GetFocus / get_active_descendant at the top level). DO NOT invent a
//   `registry.get_focused()` call — it does not exist. The 1000 ms poll is a
//   DRIFT-CATCHER: re-query the LAST focused accessible (cached ObjectRefOwned)
//   for its current title/app and dedup (catches in-place changes that fire no
//   focus event). Before any focus event has been seen, the poll is a no-op. The
//   event stream is authoritative. (Same posture as gnome.rs's drift poll.)

// (GOTCHA-8 — present ≠ useful). probe_available returning Ok only means the
//   a11y bus DAEMON is up (org.a11y.Bus owned / ATSPI_BUS_ADDRESS set). Apps only
//   EXPOSE accessibility when the desktop has AT enabled. On a box with a11y OFF
//   the probe still returns Ok but the backend sees ZERO events. Document in
//   troubleshooting.md. Do NOT try to detect "is any app exposing a11y" in the
//   probe — that's not a cheap O(1) check and §9 only asks for bus presence.

// (GOTCHA-9). `stop()` is best-effort (gnome.rs GOTCHA-7 posture): the poll
//   thread joins within ≤ DEFAULT_POLL_MS (1000 ms); the signal thread blocks on
//   the MessageIterator until the next message / connection teardown — join is
//   best-effort (`let _ = h.join()`). The daemon process exits via the
//   ctrlc/SIGTERM handler in runners/linux.rs regardless.

// (GOTCHA-10). File-level gate is `#![cfg(all(target_os="linux", feature="atspi"))]`
//   (match gnome.rs EXACTLY — its first line). The `mod atspi;` in mod.rs repeats
//   the same cfg (gnome.rs's `#![cfg]` + mod.rs cfg coexist without the
//   duplicated-attribute lint because the lint is about a single item having two
//   cfgs, not file+mod — x11.rs uses the same two-site pattern).

// (GOTCHA-11). ObjectRefOwned accessors: `.name() -> Option<&UniqueName<'static>>`
//   (the sender = the app's bus name; Some for any real accessible), `.path() ->
//   &ObjectPath<'static>`. Build AccessibleProxyBlocking via the BUILDER (not
//   ::new, which uses the default root path) with destination+path override.

// (GOTCHA-12). Run tests `--test-threads=1` (shared global debouncer STATE +
//   the env-mutating probe test; gnome.rs GOTCHA-13). Hermetic tests only — the
//   load-bearing dedup/filter logic is pure; do NOT hit the live a11y bus in unit
//   tests (Level 4 manual).
```

## Implementation Blueprint

### Data models and structure

No new public data models beyond the backend. Internals mirror `GnomeMonitor`:

```rust
pub struct AtspiMonitor {
    last_focus: Arc<Mutex<Option<(String, String)>>>,     // last NOTIFIED (app_class,title) — for dedup
    last_focused_ref: Arc<Mutex<Option<ObjectRefOwned>>>, // last focused accessible — for the poll to re-query
    shutdown: Arc<AtomicBool>,
    signal_handle: Option<JoinHandle<()>>,
    poll_handle: Option<JoinHandle<()>>,
    verbose: bool,
}
```
(`last_focused_ref` is the one field gnome.rs lacks — needed because the AT-SPI
poll re-queries a specific object rather than a single RPC. `ObjectRefOwned` is
`Clone` (atspi-common object_ref.rs) so it can be cheaply copied into the poll thread.)

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CREATE src/platforms/atspi.rs  — module skeleton + probe_available
  - FILE-LEVEL GATE: `#![cfg(all(target_os = "linux", feature = "atspi"))]` (first line; match gnome.rs/x11.rs).
  - MODULE DOC: 3-6 lines citing PLATFORMS.md §9, the async-only-avoided blocking design (GOTCHA-1),
    the two-thread model (GOTCHA-3), and the best-effort posture.
  - IMPORTS: the exact set in research/notes.md §1 (atspi::events::object::StateChangedEvent;
    atspi::proxy::{accessible::AccessibleProxyBlocking, bus::BusProxyBlocking, registry::RegistryProxyBlocking};
    atspi::{ObjectRefOwned, State}; zbus::blocking::{Connection, ConnectionBuilder, MessageIterator};
    zbus::blocking::fdo::DBusProxy; zbus::message::Type as MessageType; zbus::names::BusName;
    zbus::proxy::CacheProperties; zbus::MatchRule; crate::core::{notifier, types::WindowInfo, now_ms};
    crate::platforms::WindowMonitor; std::sync::{Arc, atomic::{AtomicBool, Ordering}, Mutex};
    std::thread::{self, JoinHandle}; std::time::Duration).
  - CONST: `const DEFAULT_POLL_MS: u64 = 1000;` (mirrors gnome.rs).
  - CONST: `const A11Y_BUS_NAME: &str = "org.a11y.Bus";`
  - IMPLEMENT `pub(crate) fn probe_available(verbose: bool) -> Result<(), String>` per §Reference
    Implementation (env ATSPI_BUS_ADDRESS short-circuit → session-bus name_has_owner).
  - NAMING: `AtspiMonitor` struct, `probe_available` (crate-visible), `list_foreground_windows` (pub),
    `platform_name() == "atspi"`.
  - PLACEMENT: src/platforms/atspi.rs (NEW file).

Task 2: src/platforms/atspi.rs — AtspiMonitor + WindowMonitor impl
  - IMPLEMENT `pub struct AtspiMonitor` (fields above) + `pub fn new(verbose: bool) -> Self`.
  - IMPLEMENT `impl WindowMonitor for AtspiMonitor`:
      `platform_name(&self) -> &str { "atspi" }`
      `start(&mut self) -> Result<(), Box<dyn Error>>`: clone Arcs, spawn run_signal_loop + run_poll_loop
        threads, store JoinHandles, return Ok(()) promptly (spawn-and-return; default
        start_blocks_calling_thread()==false — DO NOT override).
      `stop(&mut self)`: shutdown.store(true,Release); join poll (exits ≤1000ms); join signal best-effort.
  - FOLLOW pattern: src/platforms/gnome.rs (GnomeMonitor / start / stop — copy field-for-field).
  - DEPENDENCIES: Task 1's probe_available + the helper fns in Task 3.

Task 3: src/platforms/atspi.rs — dedup helper + focus filter + name resolution
  - IMPLEMENT `fn apply_and_notify(last_focus, candidate, verbose)`: lock cell → if ==Some(candidate) return;
    else store; DROP lock; verbose-print `[Nms] atspi: {app_class} | {title}`; WindowInfo::new;
    notifier::notify_qmk; on Err eprintln!("atspi: notify_qmk failed: {e}"). COPY gnome.rs verbatim.
  - IMPLEMENT `fn is_focus_gained(ev: &StateChangedEvent) -> bool { ev.state == State::Focused && ev.enabled }`
    (hermetic-testable pure predicate — Task 6).
  - IMPLEMENT `fn resolve_names(a11y_conn, item: &ObjectRefOwned, verbose) -> Option<(String,String)>`:
    build AccessibleProxyBlocking on (item.name(), item.path()); title = acc.name(); app_obj = acc.get_application()?;
    build AccessibleProxyBlocking on (app_obj.name(), app_obj.path()); app_class = app.name();
    return Some((app_class, title)). On ANY zbus Err → log (if verbose) + return None (best-effort).
    Use `AccessibleProxyBlocking::builder(&conn).cache_properties(CacheProperties::No).destination(name)?.path(path)?.build()`.
  - FOLLOW pattern: gnome.rs::apply_and_notify (lock-release-before-notify).

Task 4: src/platforms/atspi.rs — run_signal_loop (the event stream)
  - IMPLEMENT `fn run_signal_loop(last_focus, last_focused_ref, shutdown, verbose)`:
    1. session = Connection::session()? (on Err: eprintln + return — gnome.rs posture).
    2. addr = BusProxyBlocking::new(&session)?.get_address()? (the a11y bus address string).
       Fallback: if that Errs, try std::env::var("ATSPI_BUS_ADDRESS"); if both fail, eprintln + return.
    3. a11y = ConnectionBuilder::address(&addr)?.build()?.
    4. registry = RegistryProxyBlocking::new(&a11y)?; let _ = registry.register_event("object:state-changed");
       (best-effort — log on Err but continue; the match rule still delivers events the daemon already forwards.)
    5. rule = MatchRule::builder().msg_type(MessageType::Signal)
         .interface("org.a11y.atspi.Event.Object")?.member("StateChanged")?.build();
       iter = MessageIterator::for_match_rule(rule, &a11y, Some(8))?; (AUTO-registers + AUTO-deregisters the rule.)
    6. for msg in iter { if shutdown.load(Acquire) { return; }
         match StateChangedEvent::try_from(&msg) {  // validates iface+member; Err ⇒ non-match ⇒ continue
           Ok(ev) if is_focus_gained(&ev) => {
             if let Some(c) = resolve_names(&a11y, &ev.item, verbose) {
               { *last_focused_ref.lock().unwrap() = Some(ev.item.clone()); }  // seed the poll
               apply_and_notify(&last_focus, c, verbose);
             }
           }
           _ => continue,
         }
         if shutdown.load(Acquire) { return; } }
  - FOLLOW pattern: gnome.rs::run_signal_loop (blocking signal iterator; per-msg shutdown check).
  - GOTCHA: do NOT call add_match_rule yourself — for_match_rule does it (zbus doc-confirmed, research §4).

Task 5: src/platforms/atspi.rs — run_poll_loop (1000 ms drift-catcher) + list_foreground_windows
  - IMPLEMENT `fn run_poll_loop(last_focus, last_focused_ref, shutdown, verbose)`:
    a11y connection (own Connection; same addr-get as Task 4 step 2-3; on Err sleep out the interval like gnome.rs).
    loop { thread::sleep(Duration::from_millis(DEFAULT_POLL_MS)); if shutdown.load(Acquire) { return; }
      let item = last_focused_ref.lock().unwrap().clone();          // the last focused accessible
      if let Some(item) = item {
        if let Some(c) = resolve_names(&a11y, &item, verbose) { apply_and_notify(&last_focus, c, verbose); }
      } }   // no item ⇒ no-op (GOTCHA-7: no get-focused RPC; the event stream seeds last_focused_ref).
  - IMPLEMENT `pub fn list_foreground_windows() -> Vec<(String,String)>`: own a11y conn; read
    last_focused_ref-equivalent? NO — list_foreground_windows is a one-shot pub fn with NO access to the
    monitor's Arc state (it's a standalone fn like gnome.rs::list_foreground_windows). Make it query the
    session→a11y bus + re-resolve the last-known focused? It has no cached state. So: best-effort ONE-SHOT
    read — connect to a11y bus, get the registry/root accessible, return Vec::new() if no focus is readily
    known (documented best-effort). PRACTICAL IMPL: return Vec::new() with a `#[allow(dead_code)]` is
    acceptable for the trayless path, BUT to honor §9 "(e) best-effort single focused accessible" implement:
    connect a11y → if the module keeps a `static LAST_FOCUSED: Mutex<Option<ObjectRefOwned>>` (set in
    resolve_names/apply_and_notify as a process-global best-effort cache), read it + resolve_names →
    vec![(app_class,title)] else Vec::new(). Use a `static` cache (lazy/OnceLock) so the standalone fn can
    see the last focus without a monitor handle. (Simplest correct option — see §Reference Implementation.)
  - PLACEMENT: src/platforms/atspi.rs.

Task 6: src/platforms/atspi.rs — #[cfg(test)] mod tests (hermetic)
  - IMPLEMENT: test `is_focus_gained_true` (state=Focused, enabled=true → true);
    `is_focus_gained_wrong_state` (state=Selected → false); `is_focus_gained_focus_lost`
    (state=Focused, enabled=false → false); `apply_and_notify_dedups_unchanged`;
    `apply_and_notify_updates_on_change`; `apply_and_notify_empty_is_real_value`;
    `probe_available_err_message_mentions_enabling` (unset ATSPI_BUS_ADDRESS + no session bus ⇒ Err
    string contains "Assistive" OR "a11y" OR "session bus" — snapshot/restore env like gnome.rs test).
  - FOLLOW pattern: gnome.rs::tests (pure-logic assertions; env snapshot/restore for the probe test).
  - PLACEMENT: src/platforms/atspi.rs (bottom, `#[cfg(test)] mod tests`).

Task 7: EDIT src/platforms/mod.rs — module decl + list_foreground_windows arm
  - ADD after the `gnome` mod decl (L22): `#[cfg(all(target_os="linux", feature="atspi"))] mod atspi;`
  - ADD an atspi arm to the `list_foreground_windows()` cfg ladder: reached only when wayland+gnome+hyprland
    are all off — mirror the gnome arm's gating and append `not(feature="atspi")` is WRONG; instead add a NEW
    arm `#[cfg(all(target_os="linux", not(feature="wayland"), not(feature="hyprland"), not(feature="gnome"),
    feature="atspi"))] return atspi::list_foreground_windows();` and extend the final `#[cfg(not(any(...)))]`
    catch-all's `any(...)` so the arms stay mutually exclusive.
  - NAMING: matches the existing cfg-ladder style exactly.

Task 8: EDIT src/platforms/linux.rs — replace probe stub + add construct_backend arm
  - REPLACE the body of `fn atspi_probe` (L179) with `crate::platforms::atspi::probe_available(_verbose)`.
    Keep the `#[cfg(feature="atspi")]` row in linux_backend_candidates() UNCHANGED (atspi is priority #4).
  - ADD to `construct_backend()` match, after the `gnome` arm:
    `#[cfg(feature="atspi")] "atspi" => Ok(Box::new(crate::platforms::atspi::AtspiMonitor::new(verbose))),`
  - PRESERVE: the candidate ordering (foreign-toplevel → gnome → hyprland → atspi → x11), the forced-backend
    loud-Err path, and every other arm.

Task 9: EDIT docs/troubleshooting.md — Mode-A subsection
  - ADD under `## Linux Issues` (L319) a new `### AT-SPI (a11y) backend — best-effort + requires enabling accessibility` subsection:
    (1) what it is (last-resort fallback, selected only when foreign-toplevel/gnome/hyprland are unavailable);
    (2) enable it: GNOME `gsettings set org.gnome.desktop.interface toolkit-accessibility true` (or Settings →
    Accessibility); presence = `org.a11y.Bus` owned / `$ATSPI_BUS_ADDRESS` set;
    (3) limitations: app_class is the readable name not WM_CLASS (Electron/sandboxed → "python3"/"chrome"/empty);
    titles are the focused accessible not the window title; apps without an a11y bridge are invisible;
    (4) prefer the GNOME Shell extension for reliable GNOME support (cross-link §8 / the GNOME first-run hint).
  - MODE A: do NOT duplicate spec detail; link to spec/PLATFORMS.md §9.
```

### Implementation Patterns & Key Details

```rust
// ── probe_available (mirrors gnome.rs probe_available structure) ──────────────
pub(crate) fn probe_available(verbose: bool) -> Result<(), String> {
    // Cheapest presence signal #1: the env var (set by at-spi-bus-launcher).
    if let Ok(addr) = std::env::var("ATSPI_BUS_ADDRESS") {
        if !addr.is_empty() {
            if verbose { println!("[{}ms] atspi: $ATSPI_BUS_ADDRESS set (a11y bus present)", crate::core::now_ms()); }
            return Ok(());
        }
    }
    // Presence signal #2: org.a11y.Bus owned on the session bus.
    let conn = zbus::blocking::Connection::session()
        .map_err(|e| format!("cannot connect to the session bus: {e}"))?;
    let dbus = zbus::blocking::fdo::DBusProxy::new(&conn)
        .map_err(|e| format!("DBusProxy failed: {e}"))?;
    let owned = dbus
        .name_has_owner(zbus::names::BusName::from_static_str(A11Y_BUS_NAME).unwrap())
        .map_err(|e| format!("name_has_owner('{A11Y_BUS_NAME}') failed: {e}"))?;
    if owned { if verbose { println!("[{}ms] atspi: '{A11Y_BUS_NAME}' owned", crate::core::now_ms()); } Ok(()) }
    else { Err(format!(
        "the AT-SPI/a11y bus is not available ('{A11Y_BUS_NAME}' not owned, \
         $ATSPI_BUS_ADDRESS unset). Enable Assistive Technology / Screen Reader in \
         your desktop's Accessibility settings (see docs/troubleshooting.md).")) }
}

// ── resolve_names: title + app_class via TWO AccessibleProxyBlocking ──────────
fn resolve_names(
    a11y: &zbus::blocking::Connection,
    item: &ObjectRefOwned,
    verbose: bool,
) -> Option<(String, String)> {
    let acc = build_accessible(a11y, item)?;            // builder w/ destination+path override
    let title = acc.name().ok()?;                        // focused accessible's Name = TITLE
    let app_ref = acc.get_application().ok()?;           // ObjectRefOwned → application object
    let app = build_accessible(a11y, &app_ref)?;
    let app_class = app.name().ok()?;                    // application's Name = APP_CLASS (readable, unreliable)
    Some((app_class, title))
}

fn build_accessible(
    a11y: &zbus::blocking::Connection,
    obj: &ObjectRefOwned,
) -> Option<AccessibleProxyBlocking> {
    let dest = obj.name()?;                              // UniqueName (the app's bus name)
    AccessibleProxyBlocking::builder(a11y)
        .cache_properties(zbus::proxy::CacheProperties::No)
        .destination(dest).ok()?
        .path(obj.path()).ok()?
        .build().ok()
}

// ── run_signal_loop core (the blocking event stream — NO async runtime) ───────
let session = Connection::session()?;                    // zbus::blocking
let addr = BusProxyBlocking::new(&session)?.get_address()?;
let a11y = ConnectionBuilder::address(&addr)?.build()?;
let _ = RegistryProxyBlocking::new(&a11y)?.register_event("object:state-changed"); // apps emit
let rule = MatchRule::builder().msg_type(MessageType::Signal)
    .interface("org.a11y.atspi.Event.Object")?.member("StateChanged")?.build();
for msg in MessageIterator::for_match_rule(rule, &a11y, Some(8))? {  // auto match-rule reg/dereg
    if shutdown.load(Ordering::Acquire) { return; }
    if let Ok(ev) = StateChangedEvent::try_from(&msg) {   // TryFrom<&Message> (macros.rs:298)
        if is_focus_gained(&ev) {
            if let Some(c) = resolve_names(&a11y, &ev.item, verbose) {
                { *last_focused_ref.lock().unwrap() = Some(ev.item.clone()); } // seed poll
                apply_and_notify(&last_focus, c, verbose);
            }
        }
    }
}

// ── CRITICAL: do NOT do this (async-only, no runtime) ─────────────────────────
// let atspi = atspi::AccessibilityConnection::new().await?;   // ❌ async; no tokio in project
// let mut s = atspi.event_stream();                           // ❌ Stream; needs an executor
```

### Integration Points

```yaml
CARGO (NO CHANGE):
  - dep: `atspi = { version="0.30", optional=true }` (Cargo.toml:53 — already present)
  - feature: `atspi = ["dep:atspi"]` (Cargo.toml:143 — already present)
  - default: already in `default=[...]` (Cargo.toml:137 — already present)
  note: "atspi 0.30 pulls atspi-common 0.14 + atspi-proxies 0.14 + atspi-connection 0.14, all on zbus 5
         (UNIFIED with the gnome backend's zbus — no version split)."

MODULES (src/platforms/mod.rs):
  - add: `#[cfg(all(target_os="linux", feature="atspi"))] mod atspi;` (after the gnome mod)
  - add arm to: `list_foreground_windows()` cfg ladder (lowest among linux backends)

BACKEND SELECTOR (src/platforms/linux.rs):
  - replace body: `fn atspi_probe` (L179) → `crate::platforms::atspi::probe_available(_verbose)`
  - add arm: `construct_backend()` → `"atspi" => AtspiMonitor::new(verbose)` (#[cfg(feature="atspi")])
  - DO NOT change: `linux_backend_candidates()` ordering (atspi stays priority #4)

CONFIG (NO CHANGE):
  - no new field: the 1000 ms poll is a local `const DEFAULT_POLL_MS: u64 = 1000;`
    (the contract fixes the value; adding config would broaden scope into the shared schema).

DOCS (docs/troubleshooting.md):
  - add subsection under `## Linux Issues`: best-effort + enable a11y + limitations (Mode A)
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
# Run after creating atspi.rs — fix before proceeding.
cargo build --release                                   # default features (includes atspi)
# Expected: compiles with NO new warnings beyond repo baseline. If the atspi/zbus
# imports are wrong, READ the compiler error — the type paths in §Reference Impl are verified.

# Confirm the optional path still compiles (atspi is optional; nothing should force it).
cargo build --no-default-features
# Expected: compiles (the `mod atspi;` + atspi_probe arm are #[cfg(feature="atspi")] ⇒ absent).

cargo clippy --bin qmkonnect -- -D warnings 2>/dev/null || cargo clippy --bin qmkonnect
# Expected: no NEW clippy lints from this file (mirror gnome.rs's lint-clean posture).
```

### Level 2: Unit Tests (Component Validation)

```bash
# MUST run single-threaded (shared global debouncer STATE + env-mutating probe test; GOTCHA-12).
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL pass, including the NEW atspi hermetic tests:
#   is_focus_gained_true / _wrong_state / _focus_lost
#   apply_and_notify_dedups_unchanged / _updates_on_change / _empty_is_real_value
#   probe_available_err_message_mentions_enabling
# If a test fails, debug the PURE logic (dedup/filter) — do NOT add live-bus calls to unit tests.
```

### Level 3: Integration Testing (System Validation)

```bash
# Backend selection wiring (no a11y bus on this Hyprland box ⇒ atspi probe Errs + falls through).
./target/release/qmkonnect -v 2>&1 | grep -A2 "select_linux_backend: probing 'atspi'"
# Expected (dev box): "probing 'atspi'…" → "→ 'atspi' unavailable: <Err mentioning Assistive/a11y/bus>"
#   then selection continues to the next backend (x11). No panic, no hang, daemon keeps running.

# Forced-backend loud-Err path (forces atspi; expects the Err + every probe result printed).
./target/release/qmkonnect -v 2>&1 | sed -n '/forced backend/,/no Linux window backend/p' # (with config [linux] backend="atspi")
# Expected: a clear Err listing every probe result (matches select_linux_backend's forced path).

# Confirm the binary enumerates atspi as a compiled-in candidate.
./target/release/qmkonnect -v 2>&1 | grep -i "probed: \["   # (the no-backend Err's candidate list)
# Expected: list contains "atspi" between "hyprland" and "x11" (priority #4).
```

### Level 4: Creative & Domain-Specific Validation (live a11y bus — MANUAL)

```bash
# REQUIRES a real a11y bus. On the Hyprland dev box a11y is OFF by default ⇒ these are
# DOCUMENTED manual steps to run on a GNOME/Ubuntu box (or after enabling a11y here):
#
# 1. Enable a11y (GNOME):  gsettings set org.gnome.desktop.interface toolkit-accessibility true
#    (or Settings → Accessibility → enable a screen reader briefly to bring the bus up).
# 2. Confirm the bus:      dbus-send --session --dest=org.freedesktop.DBus --print-reply \
#                          /org/freedesktop/DBus org.freedesktop.DBus.NameHasOwner \
#                          string:org.a11y.Bus   # → true
# 3. Run with verbose + forced atspi, focus a Firefox window, switch to a terminal:
#    ./target/release/qmkonnect -v 2>&1 | grep atspi
#    Expected: "[Nms] atspi: Firefox | <title>" then "[Nms] atspi: <term-class> | <title>" on focus change.
#    (app_class is the readable name — e.g. "Firefox" ✓; for an Electron app expect "chrome"/"python3".)
# 4. Title-change drift: in the SAME focused window change the title (e.g. new tab, rename) WITHOUT
#    moving focus — within ~1 s the poll should re-notify with the new title (deduped if unchanged).
#
# These are best-effort expectations (§9 limitations). Record observations; do NOT block the PRP on them.
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --release` succeeds (default features incl. atspi) — no new warnings.
- [ ] `cargo build --no-default-features` succeeds (atspi optional, cleanly excluded).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` passes; NEW hermetic atspi tests pass.
- [ ] `cargo clippy --bin qmkonnect` introduces no new lints from atspi.rs.

### Feature Validation
- [ ] All Success Criteria in §What met (probe, start/stop, signal loop, poll, names, wiring).
- [ ] Level 3 wiring check: `atspi` appears as priority #4; probe Errs cleanly + falls through on the dev box.
- [ ] Level 4 live check (manual, documented): focus changes on a GNOME+a11y box emit `notify_qmk`.
- [ ] `app_class` = application `Name`, `title` = focused accessible `Name` (NOT swapped).
- [ ] Focus filter tracks ONLY `State::Focused && enabled` (verified by hermetic tests).

### Code Quality Validation
- [ ] Mirrors `src/platforms/gnome.rs` structure (struct shape, two threads, dedup helper, probe, tests).
- [ ] File-level + mod-level cfg gate match the gnome pattern (no duplicated-attribute lint).
- [ ] NO async/await, NO tokio, NO `AccessibilityConnection` (GOTCHA-1 — the design).
- [ ] No invented AT-SPI calls (no `registry.get_focused()`; GOTCHA-7 — the poll is a drift-catcher).
- [ ] Lock released before `notify_qmk` (GOTCHA-4).

### Scope & Documentation
- [ ] `git diff --stat` shows ONLY atspi.rs (new), mod.rs, linux.rs, troubleshooting.md.
- [ ] NO edits to Cargo.toml, src/core/*, src/runners/*, gnome.rs, other docs, CI, PRD.md, tasks.json.
- [ ] docs/troubleshooting.md Mode-A subsection covers best-effort + enable-a11y + limitations.
- [ ] Code is self-documenting (module doc cites §9 + the blocking design + the gotchas).

---

## Anti-Patterns to Avoid

- ❌ Don't use `atspi::AccessibilityConnection` / `event_stream()` / any `.await` — async-only, no runtime (GOTCHA-1).
- ❌ Don't invent `registry.get_focused()` / `get_active_descendant()` on the Registry — it doesn't exist; the poll is a drift-catcher, not a focus discovery (GOTCHA-7).
- ❌ Don't call `add_match_rule` yourself when using `MessageIterator::for_match_rule` — it auto-registers (research §4).
- ❌ Don't confuse title vs app_class (title = focused accessible `name()`; app_class = the *application* object's `name()`).
- ❌ Don't track non-Focused states or focus-LOST events (`enabled == false`) — only `State::Focused && enabled`.
- ❌ Don't add a config field for the 1000 ms poll — the contract fixes it; a const keeps scope tight.
- ❌ Don't reorder the candidate list — atspi is already priority #4.
- ❌ Don't edit Cargo.toml / core / runners / gnome.rs / other docs — see §Scope Boundary.
- ❌ Don't hold the `last_focus` lock across `notify_qmk` (global debouncer locks — GOTCHA-4).
- ❌ Don't hit the live a11y bus in unit tests — hermetic only (Level 4 is manual).

---

**Confidence Score: 9/10** for one-pass implementation success. The single
deduction is for the live-a11y-bus validation being manual/Level-4 (the dev box
has no a11y bus), which is unavoidable for any AT-SPI work. Everything else — the
exact API surface, the in-repo pattern to clone, the three precise edit sites, the
async-only trap and its blocking fix, and the "no get-focused RPC" reality — is
verified from primary crate sources and pinned in this PRP.
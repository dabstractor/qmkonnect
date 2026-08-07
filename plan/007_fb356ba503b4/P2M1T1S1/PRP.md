# PRP — P2.M1.T1.S1: `select_linux_backend` runtime dispatcher + probe stubs + no-backend fallback

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). **Files edited:** `src/platforms/mod.rs`
> (the `WindowMonitor` trait gains one method; `create_monitor` delegates to the new dispatcher),
> `src/platforms/linux.rs` (add `select_linux_backend` + the per-backend probe functions), `src/runners/linux.rs`
> (merge the two `#[cfg]` branches into one runtime path that handles the no-backend `Err` gracefully + fires the
> GNOME first-run notification), `src/platforms/hyprland.rs` (expose a `pub(crate) fn probe_available()` reusing
> the existing socket-liveness check), `src/platforms/x11.rs` (add `pub(crate) fn probe_available()` = the
> Wayland/DISPLAY/xprop gate). **No Cargo.toml.** No new backend files (wayland_ft.rs/gnome.rs/atspi.rs are
> P2.M2/P2.M3/P2.M4). No docs/* in this task (Mode A: selection is logged; the prose doc update is P2.M7.T1.S1).
>
> **What this does:** replaces the **compile-time** `cfg(feature="hyprland")` either/or in `create_monitor`
> with a **runtime** priority prober (`PLATFORMS.md` §6): foreign-toplevel → GNOME → Hyprland → AT-SPI → X11.
> Each compiled-in backend gets a cheap availability probe (env var / D-Bus name / socket existence); the first
> `Ok` wins; verbose mode prints every candidate, result, and the chosen backend; a `[linux] backend` override is
> honored (forced-unavailable errors loudly with every probe result). If **every** probe fails, the dispatcher
> returns `Err` and the **runner stays alive** — tray + device-status poll + HID pipeline keep running, no window
> events are emitted, and on GNOME a one-shot `notify-send` points the user at the Shell extension.
>
> **Scope boundary (critical):** the wayland/gnome/atspi backend **monitors do not exist yet** and their Cargo
> **features do not exist yet** (both land in P2.M1.T2.S2 / P2.M2 / P2.M3 / P2.M4). This task ships the
> **dispatcher + probe framework** with REAL probes for the two backends that exist today (Hyprland, X11) and
> **feature-gated probe stubs** for the three that don't (which are not compiled today because the features are
> undefined, and which each future backend task replaces). The construction `match` only has arms for backends
> that actually exist + a catch-all `Err`, so a not-yet-wired backend can never construct garbage.
>
> **Source of truth:** `PLATFORMS.md` §6 (priority table + override + logging + no-backend fallback), §8.4
> (GNOME first-run notification), §10/§11 (X11 never under Wayland; thread summary); `ARCHITECTURE.md` §2.2
> (`create_monitor` delegates to `select_linux_backend`), §7.3 (Linux runner), §8 (error model), §10
> **Invariant 11** (never select X11 under Wayland); `CONFIG.md` §1.3 (`[linux] backend` — plumbed in T2.S1).
> `research/notes.md` holds the verified current-state findings + locked design decisions.

---

## Goal

**Feature Goal**: One QMKonnect Linux binary selects its window-monitor backend at **runtime** by probing
each compiled-in backend in a fixed priority order, instead of the current compile-time
`cfg(feature="hyprland")` either/or. The selector logs its decision verbosely, honors an optional forced
override, and degrades gracefully (keeps the tray/device pipeline alive + a GNOME hint) when no backend is
available — so the same binary works on Hyprland, Sway, KDE, COSMIC, GNOME (with/without extension), and X11.

**Deliverable** (concrete code, compiles + passes tests on the Linux dev box TODAY with `default` features):
- `src/platforms/linux.rs`: `pub fn select_linux_backend(verbose: bool, forced: Option<&str>) -> Result<Box<dyn WindowMonitor>, Box<dyn Error>>` + the per-backend probe functions (`hyprland_probe`, `x11_probe` real; `wayland_probe`/`gnome_probe`/`atspi_probe` feature-gated stubs) + a private candidate-list builder.
- `src/platforms/mod.rs`: `WindowMonitor` gains `fn start_blocks_calling_thread(&self) -> bool { false }`; `create_monitor` on Linux calls `select_linux_backend(self.verbose, None)` (the override is `None` until P2.M1.T2.S1 wires `[linux] backend`).
- `src/platforms/hyprland.rs`: `pub(crate) fn probe_available() -> Result<(), String>` (thin wrapper over the existing socket-liveness logic).
- `src/platforms/x11.rs`: `pub(crate) fn probe_available() -> Result<(), String>` = the Invariant-11 gate.
- `src/runners/linux.rs`: ONE merged Linux path that spawns the tray, then on `Ok(monitor)` blocks-on-start (Hyprland) or spawns-and-parks (everyone else), and on `Err` keeps the tray/device/HID pipeline alive, fires the GNOME one-shot notification, and parks main.

**Success Definition**:
- `cargo test --bin qmkonnect -- --test-threads=1` passes (existing tests + new probe/dispatcher tests).
- `cargo build --release` (default features = `hyprland,macos,linux-tray`) succeeds; `cargo build --release --no-default-features` (trayless X11-only service build) ALSO succeeds — the merged runner must work in BOTH feature configurations.
- On the dev box, `WAYLAND_DISPLAY= DISPLAY=:0 cargo run -- -v` selects `x11` (real X11 probe); `WAYLAND_DISPLAY=wayland-0 cargo run -- -v` selects `hyprland` (x11 probe fails the Wayland gate) — verifying Invariant 11 and the priority order with the backends that exist today.
- `select_linux_backend(.., Some("x11"))` with `$WAYLAND_DISPLAY` set returns a **loud `Err`** listing every probe result (forced-unavailable), never silently picking X11.
- Verbose output prints each candidate name + its probe result + the chosen backend.
- The no-backend `Err` path does NOT exit the process (tray + HID stay alive); on `XDG_CURRENT_DESKTOP=*GNOME*` it fires exactly one `notify-send`.
- `git diff --stat` shows ONLY the 5 source files above (no Cargo.toml, no docs/*, no PRD/tasks.json).

## User Persona (if applicable)

**Target User**: a Linux user on any desktop environment. Today the app only works if compiled with the right
feature (`hyprland` vs default X11) — a GNOME-on-Wayland user running the default build gets the X11 monitor
under XWayland and silently wrong focus (Invariant 11 violation). After this task the same binary probes the
session at runtime and picks the right backend (or degrades gracefully + tells GNOME users to install the
extension).

**Use Case**: user installs the distro package (one binary) and logs into Hyprland, GNOME, KDE, or an X11
session — the app detects the right window source automatically; on GNOME-without-extension it still shows the
tray + device status and nudges them toward the extension.

**Pain Points Addressed**: closes the F16 "one binary, every DE" gap (PRD §4 F16); fixes the latent
"picked X11 on GNOME-Wayland" trap (Invariant 11); makes backend choice debuggable from the verbose log.

## Why

- **F16 (PRD §4) mandates runtime backend selection.** `PLATFORMS.md` §6 is explicit: "`platforms::create_monitor`
  delegates to `select_linux_backend(verbose)`, which probes each compiled-in backend … and returns the first
  that is present." This task IS that dispatcher.
- **The current `cfg` either/or is the wrong abstraction.** `mod x11` is only compiled under
  `cfg(not(feature="hyprland"))`, so the default binary (`hyprland` feature ON) has NO X11 monitor and a non-
  Hyprland session falls through to nothing. Runtime probing with X11 unconditional on Linux fixes both the
  GNOME-Wayland trap and the "wrong feature, no monitor" failure.
- **Foundational for P2.M2–P2.M5.** The wayland/gnome/atspi/X11 backend implementations each just *register a
  candidate* (probe + construction arm) into this dispatcher; they cannot land without the framework this task
  provides. P2.M1.T2.S1 (config `[linux]` table + features) plugs into the `forced` parameter this task defines.
- **Graceful degradation is a product requirement, not a nice-to-have.** `PLATFORMS.md` §6 "No-backend fallback":
  "the runner still starts the tray + device-status poll + HID pipeline (the app is not useless)." Today an `Err`
  from `create_monitor` propagates via `?` and exits; this task changes the runner to keep the app useful.

## What

### Approach: candidate-list dispatcher + per-backend probes + one merged runner path

1. **Dispatcher** (`select_linux_backend` in `linux.rs`): build a priority-ordered `Vec<(&'static str, fn(bool) -> Result<(), String>)>`
   of `(name, probe)`, each row `#[cfg(feature="…")]`-gated so only compiled-in backends appear. In `auto` mode,
   probe in order, return the first `Ok`'s constructed monitor. In `forced` mode, find the named candidate; if its
   probe fails (or the name isn't compiled in), error **loudly with every probe result**.
2. **Probes** are cheap (env var + socket/D-Bus/name existence). `hyprland_probe` + `x11_probe` are real today;
   `wayland_probe`/`gnome_probe`/`atspi_probe` are `#[cfg(feature="…")]` stubs returning `Err("not yet implemented
   (P2.Mx)")` — NOT compiled today (features undefined) ⇒ zero risk now; each future backend task replaces its stub.
3. **Construction** is a separate `#[cfg]`-gated `match` on the chosen name, with arms only for backends that
   exist + a catch-all `Err`, so a not-yet-wired stub can never produce a bad monitor.
4. **`create_monitor`** (mod.rs): on Linux, `select_linux_backend(self.verbose, None)` (override wired in T2.S1).
5. **Threading hint**: `WindowMonitor::start_blocks_calling_thread(&self) -> bool { false }` (default; Hyprland → `true`).
6. **Runner** (linux.rs): one path — spawn tray; `Ok(m)` ⇒ block-on-`start()` if `start_blocks_calling_thread()` else
   `thread::spawn(start)` + park/drive-tray; `Err(e)` ⇒ log, keep tray + `startup_device_probe` + handshake already
   ran (they're before the monitor), fire GNOME one-shot `notify`, park main.
7. **cfg gates**: `mod x11;` → `#[cfg(target_os = "linux")]` (unconditional); runner drops its two `#[cfg]` branches.

### Success Criteria

- [ ] `select_linux_backend(verbose, None)` exists in `linux.rs`, is `pub`, returns `Result<Box<dyn WindowMonitor>, Box<dyn Error>>`, and is called by `create_monitor` on Linux.
- [ ] The priority order in the candidate list is **foreign-toplevel, gnome, hyprland, atspi, x11** (verify by reading the `push` order; x11 is LAST and unconditional).
- [ ] `x11_probe` returns `Ok` ONLY when `$DISPLAY` is set AND `$WAYLAND_DISPLAY` is unset AND `xprop -version` succeeds (Invariant 11). Has unit tests for each combination.
- [ ] `hyprland_probe` returns `Ok` ONLY when `$HYPRLAND_INSTANCE_SIGNATURE` is set AND a live socket accepts a connection (reuses `hyprland_socket_is_live`); hermetic unit tests (TempDir + UnixListener) mirror the existing `hyprland_socket_is_live_*` tests.
- [ ] `select_linux_backend(.., Some("x11"))` under `$WAYLAND_DISPLAY=wayland-0` returns `Err` whose message lists every probe result (forced-unavailable is loud).
- [ ] `select_linux_backend(.., Some("nonsense"))` returns `Err` naming the compiled-in backends.
- [ ] Verbose mode prints, per candidate: the candidate name, the probe result (`Ok`/`Err(reason)`), and the chosen backend name.
- [ ] `wayland_probe`/`gnome_probe`/`atspi_probe` exist as `#[cfg(feature="…")]` stubs returning `Err`; they are SELF-CONTAINED (do not reference `wayland_ft::`/`gnome::`/`atspi::` modules or types).
- [ ] The construction `match` has real arms for `hyprland` (cfg `feature="hyprland"`) and `x11`, plus a catch-all `Err`; NO arm references a not-yet-existing backend type.
- [ ] `WindowMonitor` has `fn start_blocks_calling_thread(&self) -> bool { false }`; `HyprlandMonitor` overrides to `true`; `X11Monitor` uses the default.
- [ ] `runners/linux.rs` has a SINGLE Linux monitor path (no `#[cfg(feature="hyprland")]` vs `#[cfg(not(...))]` split for the monitor); on `Err` from `create_monitor` it does NOT exit — it keeps the tray alive, fires the GNOME one-shot notify (when `$XDG_CURRENT_DESKTOP` contains `GNOME`), and parks main.
- [ ] `mod x11;` gate is `#[cfg(target_os = "linux")]` (no longer `not(feature="hyprland")`).
- [ ] `cargo build --release` AND `cargo build --release --no-default-features` both succeed.
- [ ] `git diff --stat` shows ONLY the 5 source files.

## All Needed Context

### Context Completeness Check

_Pass._ An agent with no prior knowledge of this codebase can implement this from: the verbatim current
`create_monitor`/runner/trait (quoted in References); the exact priority table + probe predicates (PLATFORMS.md §6,
quoted); the existing reusable probe primitive (`hyprland_socket_is_live` + its 3 passing tests as the pattern to
clone for the dispatcher tests); the locked design decisions (candidate framework, feature-gated self-contained
stubs, trait threading hint, override-as-parameter); the precise gotchas (the `tray.rs` cfg coupling under
hyprland+linux; single-threaded tests; no breakage at T2.S2; Invariant 11; `platforms::notify` reuse); and
grep-gateable invariants validated on the Linux dev box with no extra crates.

### Documentation & References

```yaml
# MUST READ — the spec being implemented (verbatim priority table + override + logging + no-backend fallback)
- docfile: plan/007_fb356ba503b4/prd_snapshot.md
  why: "PLATFORMS.md §6 (snapshot lines 2012-2055): the 5-row priority table with the EXACT probe predicate per
        backend; '[linux] backend' override semantics (forced-unavailable errors loudly with EVERY probe result);
        verbose logging requirement (each candidate + result + chosen); the no-backend fallback posture (runner
        keeps tray + device-status + HID alive; GNOME one-shot notify-send §8.4). §8.4 (lines 2179-2186): GNOME
        first-run notify predicate = $XDG_CURRENT_DESKTOP contains GNOME AND name not owned; fires at most once per
        launch. §10 (lines 2226-2240): X11 never under Wayland (gate = $WAYLAND_DISPLAY unset). §11 thread table:
        ONLY Hyprland blocks start(); all others spawn-and-return."
  section: "PLATFORMS.md §6 + §8.4 + §10 + §11"

# MUST READ — the architecture invariants + module responsibilities this task must honor
- docfile: plan/007_fb356ba503b4/prd_snapshot.md
  why: "ARCHITECTURE.md §2.2 (lines 465-477): 'On Linux create_monitor() delegates to select_linux_backend() (in
        linux.rs/mod.rs) … returns the first present one; the runner then treats the chosen backend uniformly as a
        Box<dyn WindowMonitor>; the blocking-vs-spawn distinction is handled per-backend.' §7.3 (lines 760-767):
        the current two-branch runner (hyprland blocks; x11 spawns+parks). §8 (lines 770-789): error model — traits
        return Result<(), Box<dyn Error>>; fail-loud vs fail-soft is per-call (no-backend = fail-soft at runner).
        §10 Invariant 11 (lines 824-873): 'Never select the X11 backend under a Wayland compositor … gates X11 on
        $WAYLAND_DISPLAY being unset.' CONFIG.md §1.3 (lines 2993-3015): [linux] backend default 'auto' (plumbed
        into the `forced` param by T2.S1, NOT this task)."
  section: "ARCHITECTURE.md §2.2 + §7.3 + §8 + §10 (Invariant 11); CONFIG.md §1.3"

# MUST READ — the dispatcher's current compile-time either/or (the code being replaced)
- file: src/platforms/mod.rs
  why: "create_monitor() (lines ~31-58) today: #[cfg(all(linux, feature=hyprland))] Ok(HyprlandMonitor) ELSE
        #[cfg(all(linux, not(feature=hyprland)))] Ok(X11Monitor). mod gates: line 7-8 `#[cfg(all(linux,
        not(feature=hyprland")))] mod x11;` (THIS gate changes to unconditional linux). The WindowMonitor trait
        (lines ~12-27): platform_name/start/stop — ADD start_blocks_calling_thread here. notify(title,body) (the
        notify-send shell-out) — reuse for the GNOME first-run notify. MockWindowMonitor in the tests module — add
        the new trait method there too or it won't compile."
  pattern: "the trait shape + the cfg-dispatch shape being replaced; the notify() helper signature."
  gotcha: "MockWindowMonitor in mod.rs #[cfg(test)] implements WindowMonitor — adding a trait method with a default
           body keeps it compiling (no override needed), but if you make the method required (no default) you MUST
           add it to MockWindowMonitor + every impl. KEEP THE DEFAULT BODY."

# MUST READ — the probe primitive to reuse + the test pattern to clone
- file: src/platforms/hyprland.rs
  why: "(1) hyprland_socket_is_live(path: &Path) -> bool (lines 285-300): hermetic connect(2) liveness probe
        (thread + recv_timeout(SOCKET_PROBE_TIMEOUT=500ms)) — REUSE for hyprland_probe. (2) check_hyprland_environment()
        (lines 312-368): resolves $XDG_RUNTIME_DIR/hypr/<sig>/.socket.sock for $HYPRLAND_INSTANCE_SIGNATURE (+ a
        recovery scan of sibling dirs), returns Ok iff a live socket accepts a connection — THIS IS the hyprland
        availability probe; expose it via a thin pub(crate) fn probe_available() -> Result<(), String>. (3) The 3
        tests at lines ~635-660 (hyprland_socket_is_live_accepts_a_listening_socket / _rejects_a_dead_leftover /
        _false_for_a_missing_path) using tempfile::TempDir + UnixListener — CLONE this pattern for the dispatcher's
        hermetic probe tests."
  pattern: "probe = env var set + a live AF_UNIX socket (connect, not just exists). test = TempDir + UnixListener."
  gotcha: "check_hyprland_environment is currently PRIVATE + returns Box<dyn Error>; the new probe_available() should
           map its Err to a String reason and leave the env-var self-heal (set HYPRLAND_INSTANCE_SIGNATURE) to the
           CONSTRUCTION/start path, NOT the probe (the probe must be side-effect-free so a failed-then-retried
           selection doesn't mutate env). Consider a read-only variant or call the socket resolution without the
           env::set_var."

# MUST READ — the X11 monitor (probe target + thread model)
- file: src/platforms/x11.rs
  why: "(1) X11Monitor::start() (line 106) calls thread::spawn (line 136) and RETURNS — spawn-and-return (so
        start_blocks_calling_thread() = false, the default). (2) It shells to `xprop`; the probe must verify xprop
        EXISTS without depending on $DISPLAY. (3) The cfg gate `#![cfg(all(linux, not(feature=hyprland")))]` at the
        TOP of the file must change to `#![cfg(target_os = \"linux\")]` (unconditional) to match the mod.rs gate
        change. (4) parse_wm_class tests (lines 200+) are unaffected."
  pattern: "add pub(crate) fn probe_available(verbose) -> Result<(), String> = the 3-way gate; existing start()
            already spawn-and-returns."
  gotcha: "the FILE-LEVEL cfg (`#!`) inside x11.rs mirrors the `mod x11;` cfg in mod.rs — change BOTH to
           #[cfg(target_os=\"linux\")] or you get a 'file is empty under this cfg' error under --no-default-features."

# MUST READ — the runner to merge (two cfg branches → one runtime path)
- file: src/runners/linux.rs
  why: "the WHOLE file. Today: `let mut monitor = platforms::create_monitor(self.verbose)?;` (propagates Err →
        exits — MUST change). Two #[cfg] branches: hyprland (lines ~38-49: spawn tray, `monitor.start()?` blocks
        main) vs not-hyprland (lines ~51-79: spawn tray, monitor on thread, drive setup_tray OR park). MERGE into
        one path keyed on monitor.start_blocks_calling_thread(). The no-backend Err branch: spawn tray, fire GNOME
        one-shot notify via platforms::notify(), park main. startup_device_probe + handshake run BEFORE the monitor
        (keep them where they are — they must run even in the no-backend case)."
  pattern: "tray spawn first (linux_tray::spawn OR tray::setup_tray), then monitor; park main when no blocking
            start() or no monitor."
  gotcha: "tray.rs is compiled for cfg(not(all(linux, feature=hyprland))) — when hyprland feature is ON, tray.rs is
           ABSENT on Linux, so `crate::tray::setup_tray(...)` must be gated on BOTH not(linux-tray) AND
           not(feature=hyprland). The linux_tray::spawn path is fine under hyprland. The GNOME-notify must fire in
           the Err branch only, and ONLY when $XDG_CURRENT_DESKTOP contains GNOME (case-insensitive substring)."

# MUST READ — config plumbing boundary (why the override is a PARAMETER, not read from Config here)
- file: src/core/mod.rs
  why: "pub struct Config (lines ~24-47) has NO [linux] table today (vendor_id/product_id/usage_page/usage/
        debounce_ms/poll_interval_ms only). cached_config() (line 187) + parse_config(path) (line 209) load it.
        LinuxConfig + [linux] backend lands in P2.M1.T2.S1. So select_linux_backend MUST take the override as a
        parameter (forced: Option<&str>); create_monitor passes None now; T2.S1 changes None to read config."
  pattern: "select_linux_backend(verbose, forced: Option<&str>); create_monitor: select_linux_backend(v, None)
            with a // TODO(P2.M1.T2.S1) comment."
  gotcha: "do NOT add a LinuxConfig field to Config in this task (that's T2.S1's deliverable + would touch the serde
           schema + render_config_body). Passing None is the honest minimal change."

# REFERENCE — the existing PRP house style (mirror its verbatim-pattern + grep-gate density)
- file: plan/007_fb356ba503b4/P1M5T2S1/PRP.md
  why: "the established PRP format for this plan: verbatim Implementation Patterns, per-gotcha 'CRITICAL' callouts,
        grep-gateable Level-3 validation, an explicit Anti-Patterns list. Mirror that density here."
  pattern: "Implementation Patterns block with copy-ready Rust; Validation Loop with concrete grep/cargo commands."

# REFERENCE — design decisions + verified current state (this task's own research)
- docfile: plan/007_fb356ba503b4/P2M1T1S1/research/notes.md
  why: "the locked design (candidate framework; feature-gated self-contained stubs; trait threading hint; override-
        as-parameter; one merged runner path) + the verified current-state quotes (create_monitor, trait, runner
        branches, hyprland probe primitives, Config shape) + the coordination risks (tray.rs cfg coupling; no
        breakage at T2.S2; single-threaded tests)."
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
src/
  core/
    mod.rs            # Config (NO [linux] table yet — T2.S1); cached_config(); now_ms()
  platforms/
    mod.rs            # EDIT: WindowMonitor trait (+start_blocks_calling_thread); create_monitor (→delegate); mod gates; notify()
    linux.rs          # EDIT: ADD select_linux_backend + probes (the core deliverable lives here)
    hyprland.rs       # EDIT: expose probe_available(); start() blocks calling thread (override trait method=true)
    x11.rs            # EDIT: ADD probe_available(); FILE-LEVEL cfg → unconditional linux; start() spawn-and-return (default)
    macos.rs / windows.rs   # untouched (their create_monitor arms stay)
  runners/
    linux.rs          # EDIT: merge two #[cfg] branches → one runtime path + no-backend graceful fallback + GNOME notify
    mod.rs            # untouched (PlatformRunner trait, create_runner)
Cargo.toml            # UNTOUCHED (no new features/deps here — T2.S2 adds wayland/gnome/atspi features)
```

### Desired Codebase tree with files added/changed

```bash
src/platforms/linux.rs   # +select_linux_backend(verbose, forced) +{hyprland,x11}_probe +{wayland,gnome,atspi}_probe stubs +candidate builder + #[cfg(test)] tests
src/platforms/mod.rs     # WindowMonitor +start_blocks_calling_thread(default false); create_monitor Linux arm → select_linux_backend; mod x11 gate → unconditional; MockWindowMonitor OK (default body)
src/platforms/hyprland.rs# +pub(crate) fn probe_available() -> Result<(),String> (reuse socket liveness); HyprlandMonitor +start_blocks_calling_thread(){true}
src/platforms/x11.rs     # +pub(crate) fn probe_available() -> Result<(),String> (3-way gate); file cfg → #[cfg(target_os="linux")]
src/runners/linux.rs     # one merged path; graceful Err; GNOME one-shot notify
# (no new files; no Cargo.toml; no docs/*)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (Invariant 11 — the headline correctness gate): x11_probe MUST return Ok ONLY when $WAYLAND_DISPLAY
//   is UNSET. XWayland sets $DISPLAY under GNOME/KDE/COSMIC-Wayland but reports focus unreliably for native
//   Wayland windows, so picking X11 there silently reports wrong windows. Gate = std::env::var("WAYLAND_DISPLAY")
//   is Err OR empty AND std::env::var("DISPLAY") is Ok non-empty AND xprop -version exits 0. (Treat empty env
//   values as unset — same discipline as get_config_paths(), where PathBuf::from("").join(..) is a relative path.)

// CRITICAL (the override is a PARAMETER, not read from Config here): Config has NO [linux] table until
//   P2.M1.T2.S1. select_linux_backend(verbose, forced: Option<&str>); create_monitor passes None with a
//   // TODO(P2.M1.T2.S1): wire [linux] backend from cached_config(). Do NOT add LinuxConfig to Config in this task.

// CRITICAL (feature-gated stubs must be SELF-CONTAINED — no breakage at T2.S2): wayland/gnome/atspi Cargo features
//   do NOT exist today. Write the candidate rows as #[cfg(feature="wayland")] etc. — they're not compiled now.
//   But when T2.S2 ADDS those features, the rows WILL compile. So the stub probe closures must NOT reference
//   wayland_ft::/gnome::/atspi:: modules or types (they don't exist). Use inline `|_v| Err("not yet implemented
//   (P2.M2)".into())`. The construction match must NOT have a wayland/gnome/atspi arm referencing those types —
//   a catch-all Err arm handles any name that slipped through. (This is why construction is a SEPARATE match, not
//   a candidate-stored closure: a closure returning Box<dyn WindowMonitor> would have to name the type.)

// CRITICAL (tray.rs cfg coupling): tray.rs is compiled for cfg(not(all(target_os="linux", feature="hyprland"))).
//   Under default features (hyprland ON), tray.rs is ABSENT on Linux. So the merged runner's non-SNI fallback
//   `crate::tray::setup_tray(...)` must be gated #[cfg(all(not(feature="linux-tray"), not(feature="hyprland")))]
//   — NOT just #[cfg(not(feature="linux-tray"))]. Under hyprland+linux+not(linux-tray) there is NO tray at all
//   (the Hyprland minimal-service build); that's the existing behavior — preserve it. linux_tray::spawn is fine
//   under hyprland (separate module).

// CRITICAL (single-threaded tests — Invariant 8): the global debouncer is shared state. Run ALL tests with
//   --test-threads=1 (the AGENTS.md dev loop already mandates this). New probe tests are hermetic (TempDir +
//   env-var manipulation) but still run under the same single-threaded harness.

// CRITICAL (the probe must be SIDE-EFFECT-FREE for retry-safety): check_hyprland_environment() currently
//   self-heals by republishing $HYPRLAND_INSTANCE_SIGNATURE (env::set_var) when the declared instance is dead but
//   a sibling is live. Do NOT do that in the PROBE — a forced-backend re-probe or a future re-selection would
//   mutate env. Keep self-heal in the CONSTRUCTION/start path. The probe resolves the socket READ-ONLY.

// GOTCHA (env-var reads treat "" as unset): std::env::var returns Ok("") for an empty value. For WAYLAND_DISPLAY,
//   DISPLAY, XDG_RUNTIME_DIR, HYPRLAND_INSTANCE_SIGNATURE, XDG_CURRENT_DESKTOP — treat Ok("") as UNSET (match the
//   existing get_config_paths()/create_config_dir() discipline). An empty WAYLAND_DISPLAY must NOT gate X11 on.

// GOTCHA (xprop presence without depending on $DISPLAY): the x11 probe must succeed even on a headless CI box with
//   no $DISPLAY, as long as xprop is installed. Use `Command::new("xprop").arg("-version")` and check
//   output.status.success() (NOT just .is_ok() — a missing binary gives Err, which is correct). Do NOT use
//   `xprop -root` (needs a live X server).

// GOTCHA (the GNOME notify fires ONCE per launch, in the Err branch): platforms::notify(title, body) shells to
//   notify-send (fire-and-forget, swallows failure). Call it ONCE in the runner's no-backend Err branch, guarded
//   by $XDG_CURRENT_DESKTOP containing "GNOME" (case-insensitive). Because the branch is entered at most once per
//   process, the one-shot is automatic — no dedup state needed.

// GOTCHA (do not change start()'s behavior): X11Monitor::start() already spawns a thread and returns; Hyprland's
//   start() already blocks. Do NOT rewrite them. The new trait method start_blocks_calling_thread() only LABELS
//   the existing behavior so the runner can branch at runtime.

// GOTCHA (edition 2021 — env::set_var is not yet unsafe): if the agent bumps to edition 2024 mid-task, env::set_var
//   needs unsafe{} (the existing hyprland.rs already notes this). Keep edition 2021 (Cargo.toml) — no change here.
```

## Implementation Blueprint

### Data models and structure

```rust
// src/platforms/mod.rs — extend the trait (DEFAULT body so every existing impl + MockWindowMonitor still compile)
pub trait WindowMonitor: Send {
    fn platform_name(&self) -> &str;
    fn start(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }

    /// True iff `start()` BLOCKS the calling thread (e.g. Hyprland's IPC listener loop). False (the default)
    /// means `start()` spawns its own worker thread and returns promptly (X11 / foreign-toplevel / GNOME / AT-SPI).
    /// The Linux runner branches on this so it can park main / drive the tray for spawn-and-return backends
    /// (PLATFORMS.md §6, ARCHITECTURE.md §2.2/§11). Default `false` matches every current+future backend except
    /// Hyprland, which overrides to `true`.
    fn start_blocks_calling_thread(&self) -> bool { false }
}

// src/platforms/linux.rs — a probed backend candidate (probe only; construction is a separate match — see gotcha)
type ProbeFn = fn(verbose: bool) -> Result<(), String>;
struct BackendCandidate { name: &'static str, probe: ProbeFn }
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: EDIT src/platforms/mod.rs — extend WindowMonitor + fix mod gates
  - ADD to trait WindowMonitor: `fn start_blocks_calling_thread(&self) -> bool { false }` (DEFAULT body — keeps
    MockWindowMonitor + MacOS/Windows impls compiling; only Hyprland overrides).
  - CHANGE `mod x11;` gate from `#[cfg(all(target_os = "linux", not(feature = "hyprland")))]` to
    `#[cfg(target_os = "linux")]` (X11 is unconditional on Linux now — PLATFORMS.md §6/§10).
  - NAMING: the trait method is `start_blocks_calling_thread` (descriptive; `bool` return).
  - PRESERVE: every other trait method, the notify()/open_in_default_app() helpers, the macOS/Windows arms.
  - NOTE: MockWindowMonitor (in the #[cfg(test)] mod) needs NO change (inherits the default body).

Task 2: EDIT src/platforms/x11.rs — add probe_available + unconditional cfg
  - CHANGE the FILE-LEVEL gate `#![cfg(all(target_os = "linux", not(feature = "hyprland")))]` to
    `#![cfg(target_os = "linux")]` (must mirror the mod.rs `mod x11;` gate change from Task 1 — or cargo errors
    "file has no items under this cfg" under --no-default-features).
  - ADD `pub(crate) fn probe_available(_verbose: bool) -> Result<(), String>` implementing the 3-way gate:
      1. $DISPLAY set AND non-empty, else Err("$DISPLAY not set").
      2. $WAYLAND_DISPLAY UNSET (Err OR empty), else Err("Wayland session ($WAYLAND_DISPLAY set) — X11 never
         selected under a Wayland compositor (Invariant 11)").  ← THE headline gate.
      3. `xprop -version` exits 0, else Err("xprop not found").
    Return Ok(()) only when all three pass.
  - NAMING: `probe_available`; signature EXACTLY `fn(verbose: bool) -> Result<(), String>` (matches the ProbeFn
    type so the candidate list stores it as a plain `fn` pointer, no closure).
  - PRESERVE: X11Monitor::start() (already spawn-and-return — start_blocks_calling_thread() uses the trait default).

Task 3: EDIT src/platforms/hyprland.rs — expose probe_available (side-effect-free) + override trait method
  - ADD `pub(crate) fn probe_available(verbose: bool) -> Result<(), String>` that READS-ONLY:
      1. $HYPRLAND_INSTANCE_SIGNATURE set AND non-empty, else Err("…").
      2. $XDG_RUNTIME_DIR set AND non-empty, else Err("…").
      3. The socket $XDG_RUNTIME_DIR/hypr/<sig>/.socket.sock EXISTS and hyprland_socket_is_live(&it) is true,
         else Err("no live Hyprland IPC socket for instance <sig>").
    Do NOT call check_hyprland_environment() directly if it self-heals (env::set_var) — extract or replicate the
    READ-ONLY resolution (the existing logic up to the live-socket check, minus the env::set_var). Reusing
    hyprland_socket_is_live() is fine (it's side-effect-free).
  - ADD to `impl WindowMonitor for HyprlandMonitor`: `fn start_blocks_calling_thread(&self) -> bool { true }`.
  - PRESERVE: start() (keeps its own reconnect/self-heal — that runs after selection, where env mutation is fine).

Task 4: CREATE the dispatcher in src/platforms/linux.rs — select_linux_backend + candidate builder + stub probes
  - ADD a private `fn linux_backend_candidates() -> Vec<BackendCandidate>` pushing rows IN PRIORITY ORDER:
        #[cfg(feature = "wayland")]  push ("foreign-toplevel", wayland_probe)   // STUB (feature undefined → excluded)
        #[cfg(feature = "gnome")]    push ("gnome",             gnome_probe)     // STUB
        #[cfg(feature = "hyprland")] push ("hyprland",          hyprland::probe_available)
        #[cfg(feature = "atspi")]    push ("atspi",             atspi_probe)     // STUB
        (always on linux)            push ("x11",               x11::probe_available)
    The wayland/gnome/atspi probe fns are feature-gated stubs (Task 5); x11/hyprland are the real ones (Tasks 2-3).
  - ADD `pub fn select_linux_backend(verbose: bool, forced: Option<&str>) -> Result<Box<dyn WindowMonitor>, Box<dyn Error>>`:
      * If `forced == Some(name)`:
          - find the candidate by name; if not in the compiled-in list → Err("forced backend '<name>' not compiled
            into this binary (compiled-in: [foreign-toplevel?, …])").
          - run its probe; on Ok → construct (Task 6); on Err(reason) → LOUD Err that ALSO lists every other
            candidate's probe result (the spec: "errors loudly with every probe result"). Run all probes for the
            diagnostic even though only the forced one's availability decides.
      * Else (auto / None):
          - iterate in priority order; verbose-print "select_linux_backend: probing '<name>'…"; on Ok →
            verbose-print "  → '<name>' available, selected"; construct + return; on Err(reason) → verbose-print
            "  → '<name>' unavailable: <reason>" and continue.
          - if none Ok → Err("no Linux window backend available (probed: [names])").
  - IMPORT: `use crate::platforms::{WindowMonitor};` + `#[cfg(feature="hyprland")] use crate::platforms::hyprland;`
    + `use crate::platforms::x11;` (x11 always exists on linux now).
  - PLACEMENT: top of the `#![cfg(target_os="linux")]` linux.rs, near the other pub fns (it's the headline export).
  - NAMING: `select_linux_backend`, `BackendCandidate`, `ProbeFn`.

Task 5: ADD the feature-gated probe STUBS in src/platforms/linux.rs (self-contained — see gotcha)
  - `#[cfg(feature = "wayland")] fn wayland_probe(_v: bool) -> Result<(), String> { Err("foreign-toplevel backend
    not yet implemented (P2.M2)".into()) }` — and IDENTICAL-shape `gnome_probe` (P2.M3) / `atspi_probe` (P2.M4),
    each `#[cfg(feature = "...")]`.
  - CRITICAL: these reference NO external modules/types (the features are undefined today → not compiled; when
    T2.S2 adds the features they compile as always-Err stubs the dispatcher skips). Each future backend task
    REPLACES its stub with a real probe (and adds its construction arm in Task 6's match).

Task 6: ADD the construction match (separate from probes — see gotcha) in src/platforms/linux.rs
  - private `fn construct_backend(name: &str, verbose: bool) -> Result<Box<dyn WindowMonitor>, Box<dyn Error>>`:
        match name {
          #[cfg(feature = "hyprland")] "hyprland" => Ok(Box::new(hyprland::HyprlandMonitor::new(verbose))),
          "x11"                                  => Ok(Box::new(x11::X11Monitor::new(verbose))),
          other => Err(format!("backend '{other}' was selected but its construction is not wired in this build")),
        }
    No wayland/gnome/atspi arm (they don't exist) — the catch-all Err covers a stub that ever returns Ok (it won't).
  - CALL from select_linux_backend on a successful probe (both auto + forced paths).

Task 7: EDIT src/platforms/mod.rs — wire create_monitor on Linux to the dispatcher
  - REPLACE the two Linux `#[cfg]` arms in create_monitor with ONE:
        #[cfg(target_os = "linux")]
        {
            // TODO(P2.M1.T2.S1): wire `[linux] backend` from cached_config() into the `forced` arg.
            linux::select_linux_backend(verbose, None)
        }
  - The macOS/Windows arms are UNCHANGED.

Task 8: EDIT src/runners/linux.rs — one merged runtime path + no-backend graceful fallback + GNOME notify
  - REMOVE the two `#[cfg(all(linux, feature="hyprland"))]` / `#[cfg(all(linux, not(feature="hyprland")))]`
    monitor blocks; replace with ONE Linux path driven by runtime selection:
      let monitor_result = platforms::create_monitor(self.verbose);   // no longer `?`
      // (tray spawn — IDENTICAL in both Ok and Err branches; factor it up)
      #[cfg(feature = "linux-tray")] let _tray_handle = crate::linux_tray::spawn(self.verbose);
      #[cfg(all(not(feature = "linux-tray"), not(feature = "hyprland")))] crate::tray::setup_tray(self.verbose);
      //   ↑ tray.rs exists only under not(all(linux, feature=hyprland)); gate setup_tray on BOTH
      match monitor_result {
        Ok(mut monitor) => {
            if self.verbose { println!("Using platform: {}", monitor.platform_name()); }
            if monitor.start_blocks_calling_thread() {
                monitor.start()?;                      // Hyprland: blocks main on the IPC loop
            } else {
                std::thread::spawn(move || { if let Err(e) = monitor.start() { eprintln!("Monitor error: {e}"); } });
                #[cfg(feature = "linux-tray")] loop { std::thread::park(); }   // SNI owns its thread
                // (under not(linux-tray) AND not(hyprland): tray::setup_tray above drives a blocking loop already)
            }
        }
        Err(e) => {
            eprintln!("No Linux window backend available; running tray + device pipeline only. ({e})");
            maybe_gnome_first_run_notify(self.verbose);     // one-shot, guarded by $XDG_CURRENT_DESKTOP ~ "GNOME"
            #[cfg(feature = "linux-tray")] loop { std::thread::park(); }
            // (under not(linux-tray): if we reach here with no tray loop driven, also park; see Anti-Patterns)
        }
      }
  - ADD private `fn maybe_gnome_first_run_notify(verbose: bool)`:
      if std::env::var("XDG_CURRENT_DESKTOP").ok().filter(|s| !s.is_empty())
          .map(|s| s.to_ascii_uppercase().contains("GNOME")).unwrap_or(false)
      {
          if verbose { println!("GNOME session with no window backend — firing one-shot extension hint"); }
          crate::platforms::notify(
              "QMKonnect needs the GNOME Shell extension",
              "Window detection requires the QMKonnect GNOME Shell extension — install it from extensions.gnome.org (see docs).",
          );
      }
  - PRESERVE: the startup banner, startup_device_probe, the handshake-on-connected, and the ctrlc handler BEFORE
    the monitor selection (they must run in BOTH the Ok and Err/no-backend cases — the app is "not useless").
  - NAMING: `maybe_gnome_first_run_notify`.

Task 9: ADD tests (single-threaded — they run under the existing `cargo test --bin qmkonnect -- --test-threads=1`)
  - In linux.rs `#[cfg(test)]` (or a new `#[cfg(test)] mod select_tests`):
      * `x11_probe_ok_when_display_set_and_wayland_unset_with_xprop` — set env (DISPLAY, unset WAYLAND_DISPLAY),
        skip if xprop absent (#[cfg]) so it passes on a box without xprop too; assert Ok. (Use a helper that
        returns early-pass when xprop is missing so CI without xprop doesn't fail.)
      * `x11_probe_err_when_wayland_display_set` — set WAYLAND_DISPLAY=wayland-0; assert Err mentions Invariant 11
        / Wayland. (This is the headline regression: X11 must NEVER be selected under Wayland.)
      * `x11_probe_err_when_display_unset` — unset DISPLAY; assert Err.
      * `x11_probe_err_when_no_xprop` — only meaningful if xprop absent; otherwise this is covered by the env tests.
      * `hyprland_probe_err_when_no_signature` — clear HYPRLAND_INSTANCE_SIGNATURE; assert Err.
      * `hyprland_probe_ok_with_a_live_socket` — TempDir + bind a UnixListener at
        $XDG_RUNTIME_DIR/hypr/<sig>/.socket.sock, set HYPRLAND_INSTANCE_SIGNATURE=<sig>; assert Ok. (CLONE the
        existing hyprland_socket_is_live_* test shape.) NOTE: env-var mutation in tests is process-global →
        single-threaded is mandatory; restore env in the test (or accept process-wide state under --test-threads=1).
      * `select_forced_unknown_backend_is_loud_err` — select_linux_backend(v, Some("nonsense")) lists compiled-in
        backends in its Err.
      * `select_forced_x11_under_wayland_is_loud_err` — WAYLAND_DISPLAY set, forced Some("x11"); assert Err AND the
        message includes "every probe result" (assert it mentions at least the x11 probe reason).
      * `select_auto_picks_first_available` — craft env so x11 is the only available (no hyprland sig, DISPLAY set,
        WAYLAND_DISPLAY unset, xprop present); assert Ok and platform_name()=="X11".
  - FOLLOW pattern: the existing hyprland.rs socket tests (TempDir + UnixListener) + linux.rs udev tests.
  - COVERAGE: every probe predicate's Ok + each Err reason; the forced-loud-err path; auto first-available.
  - NAMING: `test fn` snake_case, `<subject>_<condition>_<expectation>`.

Task 10: VALIDATE (no edits) — see Validation Loop.
  - cargo test --bin qmkonnect -- --test-threads=1
  - cargo build --release (default features) AND cargo build --release --no-default-features
  - grep gates (Level 3): the priority order; x11 unconditional; the Wayland gate; the trait method; the merged
    runner; the GNOME notify; self-contained stubs; git diff --stat == 5 files.

Task 11: NEVER do these (out of scope / forbidden)
  - DO NOT add wayland/gnome/atspi Cargo features or deps (P2.M1.T2.S2).
  - DO NOT add a LinuxConfig field / [linux] table to Config (P2.M1.T2.S1).
  - DO NOT create wayland_ft.rs / gnome.rs / atspi.rs (P2.M2/P2.M3/P2.M4).
  - DO NOT make the real wayland/gnome/atspi construction arms (only hyprland + x11 + catch-all Err).
  - DO NOT rewrite X11Monitor::start() or HyprlandMonitor::start() (only LABEL them via the trait method).
  - DO NOT add docs/* prose (the Mode-A deliverable is the verbose log; the doc section is P2.M7.T1.S1).
  - DO NOT edit PRD.md / tasks.json / prd_snapshot.md / Cargo.toml / .gitignore.
```

### Implementation Patterns & Key Details

```rust
// ===== src/platforms/mod.rs — the trait extension + the mod gate (Task 1) =====
pub trait WindowMonitor: Send {
    fn platform_name(&self) -> &str;
    fn start(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }

    /// True iff `start()` blocks the calling thread (Hyprland's IPC loop). Default `false` = spawn-and-return
    /// (X11 / foreign-toplevel / GNOME / AT-SPI). The Linux runner branches on this (PLATFORMS.md §6,
    /// ARCHITECTURE.md §2.2/§11). Only Hyprland overrides to `true`.
    fn start_blocks_calling_thread(&self) -> bool { false }
}
// ...
#[cfg(target_os = "linux")]          // was: #[cfg(all(target_os = "linux", not(feature = "hyprland")))]
mod x11;

// ===== src/platforms/mod.rs — create_monitor delegates (Task 7) =====
pub fn create_monitor(verbose: bool) -> Result<Box<dyn WindowMonitor>, Box<dyn Error>> {
    #[cfg(target_os = "linux")]
    {
        // TODO(P2.M1.T2.S1): wire `[linux] backend` from core::cached_config() into the `forced` arg.
        return linux::select_linux_backend(verbose, None);
    }
    #[cfg(target_os = "macos")] { use macos::MacOSMonitor; Ok(Box::new(MacOSMonitor::new(verbose))) }
    #[cfg(target_os = "windows")] { use windows::WindowsMonitor; Ok(Box::new(WindowsMonitor::new(verbose))) }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    Err("No suitable monitor for this platform".into())
}

// ===== src/platforms/x11.rs — the probe (Task 2) — THE Invariant-11 gate =====
#![cfg(target_os = "linux")]   // was: #![cfg(all(target_os = "linux", not(feature = "hyprland")))]
// ...
/// Availability probe: $DISPLAY set AND $WAYLAND_DISPLAY unset AND xprop present. Never Ok under Wayland.
pub(crate) fn probe_available(_verbose: bool) -> Result<(), String> {
    let display = std::env::var("DISPLAY").ok().filter(|s| !s.is_empty());
    if display.is_none() {
        return Err("$DISPLAY is not set".into());
    }
    // Invariant 11 (ARCHITECTURE.md §10): X11 is NEVER selected under a Wayland compositor.
    if std::env::var("WAYLAND_DISPLAY").ok().filter(|s| !s.is_empty()).is_some() {
        return Err("Wayland session ($WAYLAND_DISPLAY set) — X11 is never selected under a Wayland compositor \
                    (XWayland focus is unreliable for native windows; PLATFORMS.md §6/§10)".into());
    }
    let xprop = std::process::Command::new("xprop").arg("-version").output();
    match xprop {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(format!("`xprop -version` exited non-zero ({}); is xprop installed?",
                             o.status.code().unwrap_or(-1))),
        Err(_) => Err("`xprop` not found on PATH".into()),
    }
}

// ===== src/platforms/hyprland.rs — the probe (Task 3) — READ-ONLY, reuses the socket liveness check =====
/// Availability probe (side-effect-free): $HYPRLAND_INSTANCE_SIGNATURE + a LIVE IPC socket. Does NOT self-heal
/// env (that stays in start()).
pub(crate) fn probe_available(_verbose: bool) -> Result<(), String> {
    let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok().filter(|s| !s.is_empty())
        .ok_or_else(|| "$HYPRLAND_INSTANCE_SIGNATURE is not set".to_string())?;
    let runtime = std::env::var("XDG_RUNTIME_DIR").ok().filter(|s| !s.is_empty())
        .ok_or_else(|| "$XDG_RUNTIME_DIR is not set".to_string())?;
    let socket = std::path::PathBuf::from(&runtime).join("hypr").join(&sig).join(".socket.sock");
    if !socket.exists() {
        return Err(format!("no Hyprland socket at {} (instance {sig})", socket.display()));
    }
    if !hyprland_socket_is_live(&socket) {
        return Err(format!("Hyprland socket exists but no listener (crashed instance {sig}?)"));
    }
    Ok(())
}
// ...
impl WindowMonitor for HyprlandMonitor {
    fn platform_name(&self) -> &str { "Hyprland" }
    fn start_blocks_calling_thread(&self) -> bool { true }   // ← the override (Task 3)
    fn start(&mut self) -> Result<(), Box<dyn Error>> { /* unchanged */ }
}

// ===== src/platforms/linux.rs — the dispatcher + candidate builder + stubs (Tasks 4-6) =====
type ProbeFn = fn(verbose: bool) -> Result<(), String>;
struct BackendCandidate { name: &'static str, probe: ProbeFn }

/// Priority-ordered list of compiled-in backends (PLATFORMS.md §6). Each `#[cfg(feature=…)]` row is absent when
/// its feature is off; the feature-undefined stubs (wayland/gnome/atspi) are simply not compiled today.
fn linux_backend_candidates() -> Vec<BackendCandidate> {
    let mut v: Vec<BackendCandidate> = Vec::new();
    #[cfg(feature = "wayland")]
    v.push(BackendCandidate { name: "foreign-toplevel", probe: wayland_probe });
    #[cfg(feature = "gnome")]
    v.push(BackendCandidate { name: "gnome", probe: gnome_probe });
    #[cfg(feature = "hyprland")]
    v.push(BackendCandidate { name: "hyprland", probe: hyprland::probe_available });
    #[cfg(feature = "atspi")]
    v.push(BackendCandidate { name: "atspi", probe: atspi_probe });
    // X11 is unconditional on Linux (always last — lowest priority; never under Wayland via its own probe).
    v.push(BackendCandidate { name: "x11", probe: x11::probe_available });
    v
}

/// Construct the chosen backend's monitor. Only real backends have arms; a not-yet-wired name (a stub whose probe
/// somehow returned Ok) hits the catch-all Err. Separated from the probe list so stub rows never name unwritten types.
fn construct_backend(name: &str, verbose: bool) -> Result<Box<dyn WindowMonitor>, Box<dyn Error>> {
    match name {
        #[cfg(feature = "hyprland")]
        "hyprland" => Ok(Box::new(crate::platforms::hyprland::HyprlandMonitor::new(verbose))),
        "x11" => Ok(Box::new(crate::platforms::x11::X11Monitor::new(verbose))),
        other => Err(format!("backend '{other}' was selected but its construction is not wired in this build").into()),
    }
}

/// Runtime Linux backend selector (PLATFORMS.md §6). `forced` overrides the priority order (default `auto`);
/// a forced backend that is unavailable errors LOUDLY with every probe result. `None` ⇒ auto first-available.
pub fn select_linux_backend(
    verbose: bool,
    forced: Option<&str>,
) -> Result<Box<dyn WindowMonitor>, Box<dyn Error>> {
    let candidates = linux_backend_candidates();
    let names: Vec<&str> = candidates.iter().map(|c| c.name).collect();

    if let Some(want) = forced {
        if verbose { println!("select_linux_backend: forced backend '{want}'"); }
        // Loud diagnostic: run EVERY probe so the user sees why the forced one failed.
        let mut diag: Vec<String> = Vec::new();
        for c in &candidates {
            let r = (c.probe)(verbose);
            diag.push(format!("  {}: {}", c.name, r.as_ref().err().map(|e| e.as_str()).unwrap_or("available")));
        }
        match candidates.iter().find(|c| c.name == want) {
            Some(c) => match (c.probe)(verbose) {
                Ok(()) => construct_backend(want, verbose),
                Err(reason) => Err(format!(
                    "forced backend '{want}' is unavailable ({reason}). Every probe result:\n{}",
                    diag.join("\n")
                ).into()),
            },
            None => Err(format!(
                "forced backend '{want}' is not compiled into this binary (compiled-in: [{}])",
                names.join(", ")
            ).into()),
        }
    } else {
        for c in &candidates {
            if verbose { println!("select_linux_backend: probing '{}'…", c.name); }
            match (c.probe)(verbose) {
                Ok(()) => {
                    if verbose { println!("  → '{name}' available, selected", name = c.name); }
                    return construct_backend(c.name, verbose);
                }
                Err(reason) => {
                    if verbose { println!("  → '{name}' unavailable: {reason}", name = c.name); }
                }
            }
        }
        Err(format!("no Linux window backend available (probed: [{}])", names.join(", ")).into())
    }
}

// Feature-gated probe STUBS (Tasks 5) — self-contained (no external-module refs). Undefined features ⇒ not compiled.
#[cfg(feature = "wayland")]
fn wayland_probe(_verbose: bool) -> Result<(), String> {
    Err("foreign-toplevel Wayland backend not yet implemented (P2.M2)".into())
}
#[cfg(feature = "gnome")]
fn gnome_probe(_verbose: bool) -> Result<(), String> {
    Err("GNOME Shell-extension backend not yet implemented (P2.M3)".into())
}
#[cfg(feature = "atspi")]
fn atspi_probe(_verbose: bool) -> Result<(), String> {
    Err("AT-SPI backend not yet implemented (P2.M4)".into())
}

// ===== src/runners/linux.rs — one merged path + no-backend fallback (Task 8) =====
impl PlatformRunner for LinuxRunner {
    fn run(&mut self, _args: &[String]) -> Result<(), Box<dyn Error>> {
        println!("QMKonnect started");
        if self.verbose { println!("Verbose logging enabled"); }

        // These run in BOTH the monitor-Ok and no-backend cases (the app stays useful: PLATFORMS.md §6).
        crate::core::notifier::startup_device_probe(self.verbose);
        if crate::core::notifier::is_device_connected() {
            crate::core::notifier::perform_handshake(self.verbose);
        }
        ctrlc::set_handler(move || { println!("\nReceived interrupt, shutting down..."); std::process::exit(0); })?;

        // Tray — IDENTICAL in both branches; spawn first so the icon is up before we block/park.
        #[cfg(feature = "linux-tray")]
        let _tray_handle = crate::linux_tray::spawn(self.verbose);
        // tray.rs exists only under cfg(not(all(linux, feature="hyprland"))): gate setup_tray on BOTH.
        #[cfg(all(not(feature = "linux-tray"), not(feature = "hyprland")))]
        crate::tray::setup_tray(self.verbose);

        match platforms::create_monitor(self.verbose) {   // no `?` — Err is handled, not fatal
            Ok(mut monitor) => {
                if self.verbose { println!("Using platform: {}", monitor.platform_name()); }
                if monitor.start_blocks_calling_thread() {
                    monitor.start()?;                       // Hyprland: blocks main on the IPC loop
                } else {
                    let _h = std::thread::spawn(move || {
                        if let Err(e) = monitor.start() { eprintln!("Monitor error: {e}"); }
                    });
                    #[cfg(feature = "linux-tray")]
                    loop { std::thread::park(); }            // SNI owns its D-Bus thread; park main
                    // (not(linux-tray) AND not(hyprland): tray::setup_tray above already drives a blocking loop.)
                }
            }
            Err(e) => {
                eprintln!("No Linux window backend available; running tray + device pipeline only. ({e})");
                maybe_gnome_first_run_notify(self.verbose);
                #[cfg(feature = "linux-tray")]
                loop { std::thread::park(); }
                #[cfg(all(not(feature = "linux-tray"), not(feature = "hyprland")))]
                { /* tray::setup_tray above is the blocking loop; fall through to its return */ }
                // If neither tray path is active (hyprland feature ON, linux-tray OFF), there is no blocking loop
                // in this branch — park main explicitly so the process stays alive for the device pipeline.
                #[cfg(all(feature = "hyprland", not(feature = "linux-tray")))]
                loop { std::thread::park(); }
            }
        }
        println!("Monitor stopped, exiting.");
        Ok(())
    }
}

fn maybe_gnome_first_run_notify(verbose: bool) {
    let gnome = std::env::var("XDG_CURRENT_DESKTOP")
        .ok().filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_uppercase().contains("GNOME"))
        .unwrap_or(false);
    if gnome {
        if verbose { println!("GNOME session with no window backend — firing one-shot extension hint"); }
        crate::platforms::notify(
            "QMKonnect needs the GNOME Shell extension",
            "Window detection requires the QMKonnect GNOME Shell extension — install it from extensions.gnome.org (see docs).",
        );
    }
}
```

### Integration Points

```yaml
TRAIT:
  - add to: src/platforms/mod.rs WindowMonitor
  - method: `fn start_blocks_calling_thread(&self) -> bool { false }` (default body; Hyprland overrides true)
MODULE CFG:
  - change: src/platforms/mod.rs `mod x11;` → `#[cfg(target_os = "linux")]`; src/platforms/x11.rs file-level `#![cfg(target_os = "linux")]`
  - keep: `mod hyprland;` body stays `#![cfg(all(target_os = "linux", feature = "hyprland"))]`
DISPATCHER:
  - add to: src/platforms/linux.rs (pub fn select_linux_backend + BackendCandidate + construct_backend + stub probes)
  - signature: `pub fn select_linux_backend(verbose: bool, forced: Option<&str>) -> Result<Box<dyn WindowMonitor>, Box<dyn Error>>`
CALLER:
  - create_monitor (mod.rs) Linux arm → `linux::select_linux_backend(verbose, None)`
  - TODO(P2.M1.T2.S1) marker: wire `[linux] backend` from core::cached_config() into `forced`
RUNNER:
  - rewrite: src/runners/linux.rs run() — ONE merged path; no-backend Err → tray+notify+park (no exit)
  - add: fn maybe_gnome_first_run_notify(verbose) using crate::platforms::notify (the existing notify-send shell-out)
CONFIG (DEFERRED to P2.M1.T2.S1):
  - NOT touched here. select_linux_backend takes the override as a parameter precisely so T2.S1 can plug it in
    without changing the dispatcher's signature.
PRODUCES (for downstream tasks):
  - the candidate framework + construct_backend match that P2.M2/P2.M3/P2.M4 extend (each adds its probe + arm)
  - the `forced` parameter that P2.M1.T2.S1 wires from Config
CONSUMES:
  - hyprland_socket_is_live (existing, reused by hyprland::probe_available)
  - platforms::notify (existing, reused for the GNOME hint)
  - linux_tray::spawn / tray::setup_tray (existing, unchanged)
SIBLING COORDINATION (zero conflict):
  - P2.M1.T2.S1 (LinuxConfig + features): lands AFTER this; only edits Cargo.toml + core/mod.rs Config + the
    create_monitor `None`→config call. The dispatcher signature is stable for it.
  - P2.M2/M3/M4/M5 (backends): each replaces its feature-gated stub probe + adds a construct arm. No overlap with
    this task's files except linux.rs's candidate list (additive `push`).
```

## Validation Loop

> The implementing agent runs on a **Linux dev box**. All gates are local: no D-Bus/GNOME/Hyprland session
> required (probes are hermetic; env-var-driven tests). The headline correctness property (Invariant 11 — never
> X11 under Wayland) is grep-gateable AND unit-tested.

### Level 1: Syntax & Style (after each file)
```bash
cd /home/dustin/projects/qmkonnect
cargo fmt -- src/platforms/mod.rs src/platforms/linux.rs src/platforms/hyprland.rs src/platforms/x11.rs src/runners/linux.rs
cargo build --release                              # default features (hyprland, macos, linux-tray)
cargo build --release --no-default-features        # trayless X11-only service build — MUST also succeed
# Expected: both compile clean. If --no-default-features fails, the file-level cfg on x11.rs OR a tray.rs
#   cfg-gate is wrong (see gotchas: setup_tray must be gated on BOTH not(linux-tray) AND not(hyprland)).
```

### Level 2: Unit Tests (single-threaded — Invariant 8)
```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL existing tests pass + the new select_tests/x11_probe/hyprland_probe tests pass.
# Focus the new tests:
cargo test --bin qmkonnect select -- --test-threads=1
cargo test --bin qmkonnect x11_probe -- --test-threads=1
cargo test --bin qmkonnect hyprland_probe -- --test-threads=1
# The headline regression: X11 MUST never be Ok under Wayland.
cargo test --bin qmkonnect x11_probe_err_when_wayland_display_set -- --test-threads=1
```

### Level 3: grep invariants — the dispatcher is structurally correct (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
L=src/platforms/linux.rs
grep -nE 'pub fn select_linux_backend' "$L"                 # Expected: 1 (the headline export)
# Priority order — x11 MUST be pushed LAST:
grep -nE 'v\.push\(BackendCandidate' "$L" | tail -1          # Expected: the x11 row (lowest priority)
! grep -qE 'push\(BackendCandidate.*name: "x11"' <(grep -vE 'name: "(foreign-toplevel|gnome|hyprland|atspi)"' /dev/stdin <<<"$(grep -nE 'push\(BackendCandidate' "$L")") && echo "OK: x11 is last"  # sanity
# Self-contained stubs (no external-module refs):
grep -nE 'fn (wayland|gnome|atspi)_probe' "$L"               # Expected: 3, each #[cfg(feature=…)]-gated
! grep -qE 'wayland_ft::|gnome::Wayland|atspi::' "$L" && echo "OK: stubs reference no unwritten modules"
# Construction has NO wayland/gnome/atspi arm (catch-all only):
grep -nE '"(foreign-toplevel|gnome|atspi)" =>' "$L"          # Expected: NOTHING (those arms don't exist)
grep -nE 'other =>' "$L"                                     # Expected: 1 (the catch-all Err)
# Forced-loud-err lists every probe result:
grep -nE 'Every probe result|every probe' "$L"               # Expected: 1

M=src/platforms/mod.rs
grep -nE 'fn start_blocks_calling_thread' "$M"               # Expected: 1 (the trait method, default false)
grep -nE 'select_linux_backend\(verbose, None\)' "$M"        # Expected: 1 (create_monitor delegates)
grep -nE 'TODO\(P2\.M1\.T2\.S1\)' "$M"                       # Expected: 1 (the override-wiring marker)
grep -nE '#\[cfg\(target_os = "linux"\)\]$' "$M"             # Expected: the `mod x11;` line is unconditional
! grep -qE 'cfg\(all\(target_os = "linux", not\(feature = "hyprland"\)\)\) mod x11' "$M" && echo "OK: x11 no longer not(hyprland)-gated"

X=src/platforms/x11.rs
grep -nE '#\[cfg\(target_os = "linux"\)\]' "$X" | head -1    # Expected: file-level gate is unconditional linux
grep -nE 'pub\(crate\) fn probe_available' "$X"              # Expected: 1
grep -nE 'WAYLAND_DISPLAY' "$X"                              # Expected: ≥1 (the Invariant-11 gate)

H=src/platforms/hyprland.rs
grep -nE 'pub\(crate\) fn probe_available' "$H"              # Expected: 1
grep -nE 'fn start_blocks_calling_thread\(&self\) -> bool \{ true \}' "$H"  # Expected: 1 (the override)

R=src/runners/linux.rs
! grep -qE 'cfg\(all\(target_os = "linux", (not\()?feature = "hyprland"' "$R" && echo "OK: runner has no cfg(hyprland) split"
grep -nE 'maybe_gnome_first_run_notify' "$R"                 # Expected: 1 def + 1 call
grep -nE 'XDG_CURRENT_DESKTOP' "$R"                          # Expected: 1 (the GNOME guard)
grep -nE 'platforms::notify\(' "$R"                          # Expected: 1 (the notify-send reuse)
# tray.rs cfg coupling: setup_tray gated on BOTH not(linux-tray) AND not(hyprland):
grep -nE 'cfg\(all\(not\(feature = "linux-tray"\), not\(feature = "hyprland"\)\)\)' "$R"  # Expected: ≥1

git diff --stat   # Expected: ONLY src/platforms/{mod,linux,hyprland,x11}.rs + src/runners/linux.rs (5 files)
```

### Level 4: Runtime smoke (the priority order + Invariant 11, with the backends that exist today)
```bash
cd /home/dustin/projects/qmkonnect
cargo build --release
# Auto: no Hyprland sig, no Wayland, DISPLAY set, xprop present ⇒ picks X11:
env -u HYPRLAND_INSTANCE_SIGNATURE -u WAYLAND_DISPLAY DISPLAY=:0 ./target/release/qmkonnect -v &
sleep 2; kill %1 2>/dev/null
# Expected verbose log: "probing 'hyprland'" → unavailable ($HYPRLAND_INSTANCE_SIGNATURE not set); then
#   "probing 'x11'" → available, selected; "Using platform: X11". (wayland/gnome/atspi rows are absent — features off.)

# Invariant 11: WAYLAND_DISPLAY set ⇒ X11 probe fails, no backend (default build has no wayland/gnome):
env -u HYPRLAND_INSTANCE_SIGNATURE WAYLAND_DISPLAY=wayland-0 ./target/release/qmkonnect -v &
sleep 2; kill %1 2>/dev/null
# Expected: every probe fails → "No Linux window backend available" + (if XDG_CURRENT_DESKTOP~GNOME) the notify.
# (No process exit until Ctrl+C — the no-backend fallback keeps it alive.)

# Forced override under Wayland ⇒ LOUD err with every probe result:
env -u HYPRLAND_INSTANCE_SIGNATURE WAYLAND_DISPLAY=wayland-0 ./target/release/qmkonnect -v &
sleep 2; kill %1 2>/dev/null
# (Forced path is exercised via config, which is None today; the UNIT TEST select_forced_x11_under_wayland_is_loud_err
#  covers the loud-err message directly — that's the gate until T2.S1 wires the override.)
```

## Final Validation Checklist

### Technical Validation
- [ ] Level 1: `cargo build --release` AND `cargo build --release --no-default-features` both clean.
- [ ] Level 2: `cargo test --bin qmkonnect -- --test-threads=1` all green (existing + new probe/dispatcher tests).
- [ ] Level 3: every grep invariant above passes; `git diff --stat` == 5 source files.
- [ ] Level 4: verbose log shows the priority probing + the chosen backend; WAYLAND_DISPLAY set ⇒ no X11 (Invariant 11).

### Feature Validation
- [ ] `select_linux_backend` exists, is `pub`, returns `Result<Box<dyn WindowMonitor>, Box<dyn Error>>`, called by `create_monitor`.
- [ ] Priority order is foreign-toplevel → gnome → hyprland → atspi → x11 (x11 LAST, unconditional).
- [ ] `x11_probe` is Ok ONLY with `$DISPLAY` set + `$WAYLAND_DISPLAY` unset + xprop present (Invariant 11).
- [ ] Forced-unavailable returns a loud Err listing every probe result; forced-unknown lists compiled-in backends.
- [ ] Verbose mode prints each candidate, its probe result, and the chosen backend.
- [ ] No-backend Err path keeps the tray + device pipeline alive (no process exit) + fires the GNOME one-shot notify.
- [ ] wayland/gnome/atspi are feature-gated self-contained stubs (not compiled today; no unwritten-module refs).

### Code Quality Validation
- [ ] `WindowMonitor` gains `start_blocks_calling_thread` with a default body (MockWindowMonitor + macos/windows impls unchanged).
- [ ] `hyprland_probe` reuses `hyprland_socket_is_live` (no duplicated liveness logic); is side-effect-free.
- [ ] `mod x11;` + x11.rs file-level cfg both `#[cfg(target_os = "linux")]` (no `not(hyprland)`).
- [ ] The runner's `tray::setup_tray` call is gated on BOTH `not(linux-tray)` AND `not(feature="hyprland")` (tray.rs cfg coupling).
- [ ] Tests are hermetic (TempDir + UnixListener for the socket test; env-var manipulation under `--test-threads=1`).
- [ ] Anti-patterns avoided (see below).

### Documentation & Deployment
- [ ] Mode A deliverable: the verbose log IS the documentation (priority order + override + chosen backend are printed).
- [ ] No docs/* prose touched (the PLATFORMS/CONFIG doc section is P2.M7.T1.S1; the `[linux] backend` table is T2.S1).
- [ ] No new env vars; the GNOME hint reuses `platforms::notify` (existing notify-send shell-out).

---

## Anti-Patterns to Avoid

- ❌ Don't read `[linux] backend` from `Config` in this task — `LinuxConfig` doesn't exist yet (P2.M1.T2.S1). Pass the override as a parameter (`forced: Option<&str>`); `create_monitor` passes `None`.
- ❌ Don't add the `wayland`/`gnome`/`atspi` Cargo features or deps — that's P2.M1.T2.S2. The stub probes are `#[cfg(feature=…)]`-gated and simply not compiled today.
- ❌ Don't reference `wayland_ft::`/`gnome::`/`atspi::` modules/types in the stub rows or the construction match — those don't exist. The construction match has arms ONLY for `hyprland` + `x11` + a catch-all `Err`.
- ❌ Don't make `start_blocks_calling_thread` a required trait method (no default body) — that forces edits to MockWindowMonitor + macos + windows impls. KEEP the default `{ false }`.
- ❌ Don't self-heal env in the probe (`env::set_var` for `$HYPRLAND_INSTANCE_SIGNATURE`) — that belongs in `start()`/construction, where a retry won't double-mutate. The probe is read-only.
- ❌ Don't gate X11 on `not(feature="hyprland")` anywhere — X11 is unconditional on Linux now; its WAYLAND gate is the probe's job, not a compile-time cfg.
- ❌ Don't call `crate::tray::setup_tray` under `hyprland`+`linux` — `tray.rs` isn't compiled there. Gate the call on BOTH `not(linux-tray)` AND `not(feature="hyprland")`.
- ❌ Don't propagate `create_monitor`'s `Err` with `?` in the runner — that exits the process and defeats the no-backend fallback. Handle `Err` (keep tray + device pipeline + GNOME notify, park main).
- ❌ Don't shell out to `notify-send` ad hoc for the GNOME hint — reuse `platforms::notify(title, body)` (it already does, and swallows failure).
- ❌ Don't rewrite `X11Monitor::start()` or `HyprlandMonitor::start()` — only LABEL their threading model via the new trait method.
- ❌ Don't treat an empty env var as set — `WAYLAND_DISPLAY=""` must NOT gate X11 off (treat `Ok("")` as unset, matching `get_config_paths()`).
- ❌ Don't run tests multi-threaded — the global debouncer + the socket-test env-var mutation are shared state (`--test-threads=1`, Invariant 8).
- ❌ Don't edit Cargo.toml / docs/* / PRD.md / tasks.json / prd_snapshot.md / .gitignore.

---

## Confidence Score

**8.5/10** for one-pass implementation success. The deliverable is a self-contained dispatcher whose two real
probes (hyprland, x11) reuse an existing, tested socket-liveness primitive and a straightforward env gate; the
priority order, override semantics, verbose logging, and no-backend fallback are specified verbatim from
PLATFORMS.md §6 + §8.4; the trait threading hint is a single default-body method; and every invariant is
grep-gateable or unit-testable on the Linux dev box with no extra crates. The two honest risks that hold it below
9.5: (1) the runner merge touches the `tray.rs` cfg coupling under `hyprland`+`linux` (the `--no-default-features`
build is the canary — it MUST still compile, and the PRP spells out the exact double-gate); (2) the no-backend Err
branch's "keep alive" parking must be correct under all four feature combinations (linux-tray × hyprland), which
the Implementation Patterns enumerates explicitly. Both are addressed by concrete validation gates (Level 1's
dual build + Level 3's grep invariants), so a careful implementing agent clears them in one pass. The deferred
pieces (the three real backends, the Cargo features, the `[linux]` config table) are explicitly out of scope and
intentionally left as additive hooks for P2.M1.T2 / P2.M2 / P2.M3 / P2.M4 — this task compiles and is useful
without any of them.
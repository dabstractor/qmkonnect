# SPEC — Platform Window Monitoring & OS Integration

> Companion to `PRD.md` / `SPEC_ARCHITECTURE.md`. Deep dive into how each
> platform detects the foreground window, what string it reports as
> `application_class`, how windows are filtered, the config-path conventions,
> and the per-OS permission model. Covers `src/platforms/*.rs` and the
> `list_foreground_windows()` enumerations.

---

## 1. The Shared Contract

Every platform implements one trait and a set of free functions dispatched from
`src/platforms/mod.rs`:

```rust
pub trait WindowMonitor: Send {
    fn platform_name(&self) -> &str;
    fn start(&mut self) -> Result<(), Box<dyn Error>>;
    fn stop(&mut self) -> Result<(), Box<dyn Error>> { Ok(()) } // default no-op
}

// Dispatchers (return the right platform's impl). On Linux `create_monitor`
// delegates to `select_linux_backend`, which probes each compiled-in backend
// for availability in priority order and returns the first that responds (§6):
//   foreign-toplevel → GNOME → Hyprland → AT-SPI → X11
pub fn create_monitor(verbose: bool) -> Result<Box<dyn WindowMonitor>, Box<dyn Error>>;
pub fn select_linux_backend(verbose: bool) -> Result<Box<dyn WindowMonitor>, Box<dyn Error>>;  // Linux only
pub fn get_config_paths() -> Vec<PathBuf>;
pub fn create_config_dir() -> Result<PathBuf, Box<dyn Error>>;
pub fn list_foreground_windows() -> Vec<(String, String)>;  // (class, title)
```

On a focus change, a monitor calls `core::notifier::notify_qmk(&WindowInfo,
verbose)` — never formats or sends the HID payload itself.

### 1.1 What `application_class` is, per platform

| Platform | `application_class` value | API |
|---|---|---|
| **Windows** | Win32 window **class name** | `GetClassNameW(hwnd)` |
| **macOS** | the app's **`localizedName`** | `[NSWorkspace.frontmostApplication localizedName]` |
| **Hyprland** | the client's **`initial_class`** | `hyprland::data::Client::get_active().initial_class` |
| **X11** | `WM_CLASS` **class** (2nd field), fallback to instance (1st) | `xprop -id <wid> WM_CLASS` |
| **Wayland** (foreign-toplevel) | the toplevel's **`app_id`** (`.desktop` basename, e.g. `firefox`, `org.gnome.Nautilus`) | `wlr-foreign-toplevel-management-v1` handle `app_id` event (§7) |
| **GNOME** (Shell extension) | `MetaWindow.get_wm_class()` (the WM_CLASS class — same value X11 reports) | republished over D-Bus by the `qmkonnect@mulletware` extension (§8) |
| **AT-SPI** (fallback) | the focused accessible's **application `Name`** (readable name, *unreliable*) | `org.a11y.atspi.Application` → `Name` (§9) |

> Users discover these exact strings via the "Show Window Information" dialog
> (`SPEC_UI.md` §3) and match them in firmware (`DEFINE_SERIAL_LAYERS` /
> `DEFINE_SERIAL_COMMANDS`). **macOS is case-sensitive as displayed** (e.g.
> `"Safari"`); Windows classes are usually PascalCase (`Chrome_WidgetWin_1`);
> Hyprland classes are lowercase (`firefox`, `neovide`).

### 1.2 Titles

| Platform | Title source | Notes |
|---|---|---|
| Windows | `GetWindowTextW` (trimmed) | Trailing-space padding stripped |
| macOS | `CGWindowListCopyWindowInfo` → `kCGWindowName` for the frontmost app's window | **Requires Screen Recording** (§4.2); empty without it |
| Hyprland | `Client::get_active().title` | |
| X11 | `xprop … _NET_WM_NAME` | |
| Wayland (foreign-toplevel) | toplevel `title` event | |
| GNOME (extension) | `MetaWindow.get_title()` over D-Bus | |
| AT-SPI (fallback) | focused accessible's `Name` (unreliable) | |

### 1.3 Empty-workspace semantics

- **Hyprland:** an empty workspace reports `WindowInfo { app_class: "", title: "" }`
  → payload `"\x1D"` → firmware deactivates any active layer. This is desired
  (no app focused ⇒ neutral keymap).
- **Windows / macOS:** no focus event is generated for "no window", so the
  keyboard retains the last-reported app until the next real focus change.
- **Wayland (foreign-toplevel) / GNOME / Hyprland:** an empty workspace (no
  toplevel carries the `activated` state / `focus_window` is null) reports the
  empty `WindowInfo` → firmware deactivates any active layer (same as Hyprland).

### 1.4 Window filtering (`should_ignore_window`, Windows/macOS)

Internal/shell windows that briefly grab foreground must not be reported:

**Windows** ignores these classes (`src/platforms/windows.rs`):
```
ForegroundStaging, XamlExplorerHostIslandWindow,
Windows.UI.Composition.DesktopWindowContentBridge,
Windows.UI.Input.InputSite.WindowClass, TaskSwitcherWnd,
TaskSwitcherOverlayWnd, Windows.UI.Core.CoreWindow,
ApplicationFrameWindow,                  // UWP frame — want the real content
TopLevelWindowForOverflowXamlIsland,     // Win11 tray-overflow flyout
NotifyIconOverflowWindow,                // Win10 tray-overflow flyout
Shell_TrayWnd, Shell_SecondaryTrayWnd    // taskbar(s)
```
Plus: empty titles are rejected **unless** the class is in an allowlist
(`CASCADIA_HOSTING_WINDOW_CLASS` terminal, `Chrome_WidgetWin_1`), and titles
shorter than 2 chars are rejected.

**macOS:** filters to apps with `activationPolicy == NSApplicationActivationPolicyRegular`
(0) — i.e. Dock-visible apps — and `isFinishedLaunching == YES`. No shell chrome
to filter.

**Hyprland:** `list_foreground_windows()` filters to `mapped` clients; the live
monitor reports whatever Hyprland says is active (including empty).

---

## 2. Windows Monitor (`src/platforms/windows.rs`)

### 2.1 Detection mechanism (belt + suspenders)

1. **`SetWinEventHook(EVENT_OBJECT_FOCUS, …, WINEVENT_OUTOFCONTEXT)`** — the
   primary focus signal. The callback `event_proc(hwnd,…)` runs on the thread
   pumping the message loop (the `tao` tray loop on the shipped app). Each
   focus event → `handle_focus_change(hwnd)`.
2. **`SetWinEventHook(EVENT_OBJECT_NAMECHANGE, …)`** — a *second* hook that
   surfaces **in-app title edits** (browser tab switches, document/sheet
   changes, …) which change the title without a focus transition. NAMECHANGE
   fires for the element whose name changed — frequently a CHILD window — so
   `event_proc` re-derives the **foreground** window (`GetForegroundWindow()`)
   for this event rather than trusting the event's own `hwnd`. Without this
   hook, title-pattern host rules (e.g. `match = ["*chrome*","*youtube*"]`)
   would silently stop reacting as the user tabs around inside an already-
   focused app. (Failure to install this hook is non-fatal: the focus hook +
   poller still cover focus transitions.)
3. **100 ms polling thread** (`GetForegroundWindow()` → `handle_focus_change`)
   — a fallback for transitions the hooks can miss (notably apps that don't
   emit `EVENT_OBJECT_NAMECHANGE` for in-window title edits). It calls
   `handle_focus_change` unconditionally each tick; `handle_focus_change`'s
   `(class,title)` dedup (`LAST_WINDOW_INFO`) is the real gate, so this surfaces
   BOTH focus changes and same-window title changes (the former form gated on
   HWND equality and so missed title-only changes).

### 2.2 `handle_focus_change(hwnd)`
- `get_window_info(hwnd)` → `WindowInfo` (class via `GetClassNameW`, title via
  `GetWindowTextW`).
- Skip if `should_ignore_window`.
- Dedup against `LAST_WINDOW_INFO` (`Mutex<Option<(String,String)>>`) to kill
  feedback loops. The dedup key is the **(class, title)** pair, so identical
  re-reports (e.g. a NAMECHANGE that didn't alter the foreground title, or a
  poller tick with no change) collapse to a single send.
- `notify_qmk(&window_info, verbose)`.

### 2.3 Thread-safe globals (replaced former `static mut` UB)
- `G_VERBOSE: AtomicBool`
- `G_HOOK: AtomicIsize` (holds the focus `HWINEVENTHOOK` handle)
- `G_NAME_HOOK: AtomicIsize` (holds the `EVENT_OBJECT_NAMECHANGE` hook handle)
- `LAST_WINDOW_INFO: Mutex<Option<(String,String)>>`

### 2.4 `list_foreground_windows()` (the tray dialog data)
`EnumWindows` over visible (`IsWindowVisible`) top-level windows, reusing
`get_window_info` + `should_ignore_window` so the list **exactly matches** what
the live monitor would report.

### 2.5 Config paths (`get_config_paths`)
1. `%APPDATA%\QMKonnect\config.toml` (primary)
2. `%LOCALAPPDATA%\QMKonnect\config.toml` (secondary)
3. exe directory (fallback)

`create_config_dir()` → `%APPDATA%\QMKonnect`.

---

## 3. macOS Monitor (`src/platforms/macos.rs`)

### 3.1 Detection mechanism
- Registers an observer on `NSWorkspace.sharedWorkspace.notificationCenter`
  for **`NSWorkspaceDidActivateApplicationNotification`**. The handler class
  `RustNotificationObserver` (declared once via `ClassDecl`) implements
  `observeNotification:` → calls `get_active_window_info` → hands the result to
  the `NOTIFY_TX` worker (`notify_qmk` must never run on the main thread — see
  `NOTIFY_TX` in source).
- **Title-change poller** (500 ms): the activation notification only fires on
  APP SWITCHES, so in-app title edits (a browser tab switch, a document/sheet
  change within an already-focused app) would never be surfaced — title-pattern
  host rules would silently stop reacting as the user tabs around inside the
  focused app. A background thread polls `get_active_window_info` on a 500 ms
  cadence and pushes to `NOTIFY_TX` only when the frontmost (class, title)
  changes; the debouncer further coalesces any burst. Mirrors the Hyprland
  `poll_interval_ms` design (§5.4).
- `start()` calls `ensure_screen_recording_permission` (§4.2), sets up the
  observer, captures the initial frontmost app, then **`CFRunLoopRun()`** blocks
  the calling thread (a background thread — the tray owns main).

### 3.2 `get_active_window_info()`
- `[NSWorkspace.frontmostApplication localizedName]` → app name.
- Walk `CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, 0)`; find
  the entry whose `kCGWindowOwnerName == app_name`; read `kCGWindowName` → title.
- Returns `WindowInfo { app_class: app_name, title }`.

### 3.3 `list_foreground_windows()` (tray dialog data)
Iterates `NSWorkspace.runningApplications`, keeps `activationPolicy == 0`
(Regular) and `isFinishedLaunching`, maps each to `(localizedName, title)` via a
pre-built `owner → title` map from the CG window list, sorts alphabetically.

### 3.4 Config paths
1. `~/Library/Application Support/QMKonnect/config.toml` (primary)
2. `~/.config/qmkonnect/config.toml` (XDG-style fallback)
3. `/etc/qmkonnect/config.toml` (system-wide last resort)

`create_config_dir()` → `~/Library/Application Support/QMKonnect`.

---

## 4. Permission Model (per-OS)

### 4.1 Windows
- **None required** for HID access or foreground-window detection.
- The app runs per-user; no elevation.

### 4.2 macOS — Screen Recording (not Accessibility)
- Window **titles** come from `CGWindowListCopyWindowInfo`, which requires
  **Screen Recording** permission (macOS 10.15+). Without it, titles come back
  empty — but the frontmost **app name** still works, so the app keeps running.
- `ensure_screen_recording_permission(verbose)`:
  - `CGPreflightScreenCaptureAccess()` → if already granted, continue.
  - Else `CGRequestScreenCaptureAccess()` (pops the system dialog, returns
    immediately) and **keep running** (graceful degradation; the app picks up
    titles once the user grants access and quits/reopens).
- **Ad-hoc signing re-prompt loop:** local builds are ad-hoc signed, so the
  `cdhash` changes every rebuild; macOS keys the grant to the signature and
  re-prompts even though System Settings shows it granted. `tccutil reset
  ScreenCapture io.mulletware.qmkonnect` resets it; a stable `CODESIGN_IDENTITY`
  (Developer ID) stops the loop. See `SPEC_PACKAGING.md` §5.

### 4.3 Linux — hidraw permissions (full detail: `SPEC_LINUX.md`)
- Default QMK keyboards need no manual setup: the static udev rule grants
  `GROUP="input", MODE="0660", TAG+="uaccess"` to any `0xFF60/0x61` interface.
- Users may need to be in the `input` group (or rely on the `uaccess` ACL).
- Custom VID/PID users generate a config-driven fallback rule via
  `sudo qmkonnect -r`.

---

## 5. Hyprland Monitor (`src/platforms/hyprland.rs`)

### 5.1 Detection mechanism
- `wait_for_hyprland(verbose)` first (handles the boot race): probe
  `HYPRLAND_INSTANCE_SIGNATURE` + the socket under `$XDG_RUNTIME_DIR/hypr/<sig>/.socket.sock`;
  if absent, scan `$XDG_RUNTIME_DIR/hypr/*/​.socket.sock` and **set** the env
  var once (main thread) so the `hyprland` crate picks the right instance.
  Verify IPC with `Monitors::get()`. Exponential backoff, 30 s timeout.
- Create an `EventListener` and register handlers:
  - `add_active_window_changed_handler` → `handle_window_state_change`
  - `add_workspace_changed_handler` → `handle_workspace_change` (re-derives active window)
  - `add_window_closed_handler` → `handle_window_state_change`
  - `add_layer_opened_handler` / `add_layer_closed_handler` →
    `handle_window_state_change` **+** `spawn_poll_burst` (scratchpad/layer focus)
- `listener.start_listener()` **blocks**; on error, reconnect with backoff.

### 5.2 `handle_window_state_change`
- `Client::get_active()` → if `Some`, report `initial_class` + `title`. If
  `None`, report the empty-window `WindowInfo` (deactivates layers).
- Dedup + update `last_window_state` (`Arc<Mutex<Option<WindowState>>>`)
  **atomically in one critical section** (mirrors `poll_window_state`): the
  compare and the update share a single lock acquisition so a concurrent poll-
  burst thread (spawned from these same handlers) cannot read the same stale
  state and double-notify. `notify_qmk` runs after the lock is dropped.

### 5.3 Reconnect backoff (fixes #7)
- `INITIAL_RECONNECT_MS = 100`, `MAX_RECONNECT_MS = 10_000`, growth `×3`.
- **Reset to initial** when a listener that stayed up ≥
  `STABLE_CONNECTION_THRESHOLD` (5 s) is lost, so long-uptime sessions don't
  stick at the 10 s cap.
- **Hard-fail** only if the very first attempt dies within 2 s of startup
  (Hyprland genuinely unavailable).

### 5.4 Polling strategies (two distinct ones)
- **Optional periodic poll** (`poll_interval_ms`, default 0 = off): a thread
  polls `Client::get_active()` and dedups against `last_window_state`. Corrects
  IPC drift (notably `movetoworkspacesilent` scratchpad dismissals where the
  `activewindow` event lags). **Hot-config (PRD §7):** the interval is re-read
  from `configured_timing()` on every iteration, so a live edit to
  `config.toml` takes effect on the next tick — including `0→N` (enable),
  `N→0` (disable), and `N→M` (cadence change) — with no restart. The thread is
  always spawned (even when polling is initially off) so a `0→N` edit can start
  it; while disabled it sleeps on a slow re-check cadence.
- **Poll burst after layer events** (`spawn_poll_burst`): 5× 100 ms polls after
  a layer open/close, to absorb the timing gap where focus hasn't settled at
  event time. Replaces the former permanent 100 ms poller.

### 5.5 `list_foreground_windows()` (tray dialog data)
`Clients::get()` filtered to `mapped`, mapped to `(class, title)`, with the
active window moved to front (so `.next()` reports the focused window).

---

## 6. Linux Backend Selection (`select_linux_backend`)

Linux has five monitor backends (§5 Hyprland, §7 foreign-toplevel, §8 GNOME,
§9 AT-SPI, §10 X11). `platforms::create_monitor` delegates to
`select_linux_backend(verbose)`, which probes each compiled-in backend for
availability in **priority order** and returns the first that is present as
`Box<dyn WindowMonitor>`. This replaces the old compile-time
`cfg(feature="hyprland")` either/or: **every backend is compiled in by default
and the right one is chosen at runtime**, so a single binary works across GNOME,
KDE Plasma, COSMIC, Hyprland, Sway, Niri, the wlroots family, and the X11 DE
tail.

Priority (first available wins):

| # | Backend | Feature | Availability probe |
|---|---|---|---|
| 1 | **foreign-toplevel** (Wayland) | `wayland` | `$WAYLAND_DISPLAY` resolvable **and** the compositor advertises the `zwlr_foreign_toplevel_manager_v1` global |
| 2 | **GNOME** (Shell extension) | `gnome` | the D-Bus well-known name `io.mulletware.QMKonnect` is owned on the session bus (§8) |
| 3 | **Hyprland** (IPC) | `hyprland` | `$HYPRLAND_INSTANCE_SIGNATURE` + a live socket (§5). *Superseded by #1 on Hyprland; retained as a fallback when the `wayland` feature is off.* |
| 4 | **AT-SPI** (a11y bus) | `atspi` | the a11y bus is reachable (`org.a11y.Bus` owned or `$ATSPI_BUS_ADDRESS`) |
| 5 | **X11** | *(always on Linux)* | `$DISPLAY` set **and `$WAYLAND_DISPLAY` unset** and `xprop` present — *never* under a Wayland compositor (XWayland focus is unreliable; §10) |

**Config override:** `[linux] backend = "foreign-toplevel" | "gnome" | "hyprland" | "atspi" | "x11" | "auto"` (default `auto`) in `config.toml` (`CONFIG.md` §1.3). A forced backend that is unavailable errors loudly with every probe result; `auto` is the normal path.

**Logging:** in verbose mode the selector prints each candidate, its probe
result, and the chosen backend, so a "why did it pick X?" question is always
answerable from the log.

**No-backend fallback (the GNOME-Wayland-without-extension case):** if every
probe fails (e.g. GNOME on Wayland with the extension uninstalled and a11y off),
`select_linux_backend` returns `Err`. The runner still starts the tray +
device-status poll + HID pipeline (the app is not useless — it shows connection
state and the Settings/rules UIs), but emits no window events until a backend
becomes available. On GNOME specifically a one-shot `notify-send` fires pointing
the user at the extension (§8.4).

**Feature gating:** backends whose feature is off are absent from the binary
(their table row is skipped at compile time). `default =
["wayland","gnome","atspi","hyprland","macos","linux-tray"]`; X11 is
unconditionally compiled on Linux. `--no-default-features` yields a trayless
service build with only the X11 backend.

---

## 7. Foreign-toplevel Wayland Backend (`src/platforms/wayland_ft.rs`, feature `wayland`)

The single highest-leverage backend: one Wayland client speaking the
foreign-toplevel protocols covers every wlroots-derived compositor **plus KDE
Plasma and COSMIC**.

### 7.1 Protocols
- **`wlr-foreign-toplevel-management-unstable-v1`** — the **load-bearing**
  protocol. It is the one that reports **activation/focus state** (the `state`
  event's `activated` flag), which is how QMKonnect knows *which* toplevel is
  focused. Bind `zwlr_foreign_toplevel_manager_v1` and track each
  `zwlr_foreign_toplevel_handle_v1`.
- **`ext-foreign-toplevel-list-v1`** — the upstream successor (merged into
  `wayland-protocols` staging in 2024). It lists toplevels
  (`app_id`/`title`/`identifier`) but **does not report activation**. Bind it
  when present and use it only to populate `list_foreground_windows()` (the tray
  "Show Window Information" dialog) and to cross-check `app_id`s — never as the
  focus source.

### 7.2 Coverage (activation via wlr-foreign-toplevel)

| Compositor | wlr-foreign-toplevel | Notes |
|---|---|---|
| Hyprland | ✅ | so this backend also covers Hyprland — §5 IPC is the legacy fallback |
| Sway | ✅ | |
| Niri | ✅ | (+ its own `niri msg` JSON IPC, not used) |
| River, Labwc, Wayfire | ✅ | wlroots family |
| KDE Plasma 6 (KWin) | ✅ | also implements `ext-foreign-toplevel-list-v1` |
| COSMIC (Smithay) | ✅ | verify on current COSMIC; also implements `ext-foreign-toplevel-list-v1` |
| GNOME (Mutter) | ❌ | neither protocol → falls through to §8 |

> **If a compositor advertises only `ext-foreign-toplevel-list-v1` (no wlr
global):** there is no activation source, so this backend cannot determine focus.
The selector treats the wlr global as the availability gate; a compositor
without it is **not** covered by this backend and falls through. (Watch the
upstream `ext` family — a future activation-reporting extension would let us
drop the wlr dependency.)

### 7.3 Implementation
- **Crate:** `smithay-client-toolkit` (feature `foreign-toplevel`) provides the
  wlr-protocol `ForeignToplevelManager`/`ForeignToplevelHandler`. For the `ext`
  protocol, generate bindings from the staging `wayland-protocols` XML with
  `wayland-scanner` (or use sctk if a given version exposes it). Pin sctk; pick
  the API surface it offers.
- **Thread model:** `start()` **spawns** a dedicated thread running the
  `EventQueue` dispatch loop and **returns immediately** (unlike Hyprland's
  blocking listener). The monitor owns an `Arc<Mutex<Option<(String,String)>>>`
  last-state cell shared with the event thread. The runner parks main / drives
  the tray separately (same shape as the X11/non-Hyprland runner — §11).
- **Per-toplevel tracking:** on the manager's `toplevel` event create a handle;
  cache its `app_id`, `title`, and `state` bitmask; update on
  `title`/`app_id`/`state` events; remove on `closed`; commit on `done`. After
  each `done`, recompute the toplevel whose `state` includes `activated`; if it
  differs from the last reported `(app_class,title)`, emit `notify_qmk`.
- **`app_class` = the toplevel's `app_id`** (typically the `.desktop` basename:
  `firefox`, `org.gnome.Nautilus`, `code`, `google-chrome`). Reverse-DNS
  `app_id`s are passed through verbatim — users match what the window-info dialog
  shows.
- **Empty workspace:** when no toplevel carries the `activated` state → emit
  `WindowInfo { app_class:"", title:"" }` (deactivates layers), mirroring §1.3.
- **Reconnect:** on wl_display error (compositor crash/restart), reconnect with
  backoff reusing the Hyprland §5.3 constants (`INITIAL_RECONNECT_MS=100`,
  `MAX_RECONNECT_MS=10_000`, ×3, reset after `STABLE_CONNECTION_THRESHOLD=5 s`).
- **`list_foreground_windows()`:** snapshot all tracked toplevels as
  `(app_id, title)`, the activated one first.

### 7.4 Why it also replaces the Hyprland IPC backend
Hyprland implements `wlr-foreign-toplevel-management-v1`, so this backend
reports the same active window the Hyprland IPC backend does, via the standard
protocol. The Hyprland-IPC backend (§5) is **retained** behind the `hyprland`
feature for one release cycle as a fallback and for its Hyprland-specific
scratchpad poll-burst behavior; it is **not** in the default selection path when
`wayland` is compiled in (priority #1 wins). It may be removed once this backend
proves stable on Hyprland.

---

## 8. GNOME Backend — Shell Extension + D-Bus Client

GNOME (Mutter) implements neither foreign-toplevel protocol and exposes no
client API for the active window, so the active window is read **inside
`gnome-shell`** by a small extension and republished over the session D-Bus,
where QMKonnect subscribes. This is the same approach every "active window on
GNOME" app uses.

**Two deliverables:**
1. **`qmkonnect@mulletware` GNOME Shell extension** (`packaging/gnome-shell-extension/`) — runs in the `gnome-shell` process; the only thing that can read `global.display.focus_window`.
2. **`src/platforms/gnome.rs`** (feature `gnome`) — the desktop-side D-Bus client; subscribes to the extension's signal and notifies QMK.

### 8.1 The D-Bus contract (owned by both halves)
- Well-known name: **`io.mulletware.QMKonnect`** (owned ⇔ extension is installed & enabled).
- Object path: **`/io/mulletware/QMKonnect`**.
- Interface: **`io.mulletware.QMKonnect.WindowMonitor`**:
  - method **`GetActiveWindow() → (s app_class, s title)`** — synchronous current-state read.
  - signal **`ActiveWindowChanged(s app_class, s title)`** — emitted on focus transition (and on `enable()` for the initial state).
  - read properties `AppClass:s`, `Title:s` — for `org.freedesktop.DBus.Properties` polling.
- `app_class` = **`MetaWindow.get_wm_class()`** (the WM_CLASS *class* — the same string the X11 backend reports, e.g. `Firefox`, `Gnome-terminal`), chosen for parity with the firmware-pattern world. `title` = `MetaWindow.get_title()`.

### 8.2 The extension (`packaging/gnome-shell-extension/`)
- GJS, GNOME Shell 45+ APIs (`global.display`, `Meta.Window`). `metadata.json`:
  `uuid = "qmkonnect@mulletware"`, `shell-version` covering the supported GNOME
  line (e.g. `["45","46","47","48","49","50"]`), `version` = the QMKonnect
  release, `url`/`settings-schema` optional.
- `enable()`: acquire `io.mulletware.QMKonnect`; export the object/interface on
  the session bus; connect `global.display.connect('notify::focus-window', …)`;
  emit the initial state.
- `_onFocus()`: `let w = global.display.focus_window;` → `w ? [w.get_wm_class() ?? "", w.get_title() ?? ""] : ["",""]`; dedup against last-emitted; emit `ActiveWindowChanged`.
- `disable()`: disconnect, release the name, unexport.
- Fallbacks: `get_wm_class()` may be `null` for some apps → empty class (the
  title still carries info); `get_description()` is not used (unreliable).

### 8.3 The client (`src/platforms/gnome.rs`, feature `gnome`)
- `zbus` session connection. Subscribe to `ActiveWindowChanged` (zbus proxy
  signal / `add_match`); on signal → dedup → `notify_qmk`.
- **Drift-correcting poll thread** (default **1000 ms**, hot-config via
  `[linux] gnome_poll_interval_ms`, `CONFIG.md` §1.3): calls `GetActiveWindow`
  and dedups — catches any missed signal (mirrors the macOS/Hyprland poll
  design). Always spawned.
- **NameOwnerChanged watch:** if the extension is toggled (name appears /
  disappears) mid-session, the monitor re-acquires initial state from
  `GetActiveWindow`; if the name goes away the backend reports empty and the
  §6 "no-backend" posture applies.

### 8.4 First-run UX on GNOME
If the session is GNOME (`$XDG_CURRENT_DESKTOP` contains `GNOME`) **and** the
extension name is not owned at startup, fire a one-shot desktop notification
(via `platforms::notify`, `LINUX.md`): *"QMKonnect needs the GNOME Shell
extension for window detection — install it from extensions.gnome.org (see
docs)."* The daemon keeps running (tray + device status); the AT-SPI backend may
run meanwhile (§9) as a best-effort interim. The notification fires at most once
per launch.

### 8.5 Distribution of the extension
Built as **`qmkonnect@mulletware.shell-extension.zip`** (the EGO upload format:
top-level `metadata.json` + `extension.js` + optional `prefs.js` + sources).
Published on **extensions.gnome.org** and attached to each GitHub Release. CI
(`PACKAGING.md` §9) builds the zip from `packaging/gnome-shell-extension/`. The
QMKonnect app **does not** install/load it (it cannot run inside gnome-shell);
it only points users to it.

---

## 9. AT-SPI Fallback Backend (`src/platforms/atspi.rs`, feature `atspi`)

The last-ditch backend for any compositor with accessibility enabled — primarily
a **GNOME fallback when the extension isn't installed** and an emergency path
elsewhere. **Best-effort, not primary** (see limitations).

- **Crate:** `atspi` (zbus to the a11y bus).
- **Mechanism:** subscribe to `object:state-changed:focused` events; track the
  focused accessible. `app_class` = the focused accessible's **application
  `Name`** (`org.a11y.atspi.Application` → `Name`); `title` = the focused
  accessible's `Name`.
- **Poll fallback:** every 1000 ms query the a11y registry's focused object and
  dedup (same shape as §8.3).
- **Availability:** the a11y bus is present (`org.a11y.Bus` owned or
  `$ATSPI_BUS_ADDRESS`). Most distros ship a11y **off** until the user enables
  "Assistive Technology / Screen Reader" in Settings — document that enabling
  a11y is required for this backend.
- **Known limitations (why it's not the GNOME primary):**
  - `app_class` is the app's *readable* name, not `WM_CLASS` — usually fine
    (`"Firefox"`) but inconsistent for Electron/sandboxed apps (`"python3"`,
    `"chrome"`, or empty).
  - Titles vary (the focused *accessible*, not the toplevel, is reported).
  - Apps that don't expose a11y (some games, some Qt apps without the bridge)
    are invisible.
  - Use the GNOME Shell extension (§8) for reliable GNOME support.

---

## 10. X11 Monitor (`src/platforms/x11.rs`)

The lowest-priority backend; compiled in on **every** Linux build (no longer
`--no-default-features`-only). Selected only for genuine X11 sessions.

- `get_active_window_info()`: `xprop -root _NET_ACTIVE_WINDOW` → window id →
  `xprop -id <wid> WM_CLASS _NET_WM_NAME`. `WM_CLASS` second field (class) is
  preferred; first field (instance) is the fallback. `0x0` ⇒ empty workspace.
- **Fails loudly** if `xprop` is missing (never emits placeholder strings — #14).
- Polls every **500 ms** on a background thread (X11 focus changes are
  user-driven; latency is acceptable for a fallback).
- **Never selected under Wayland:** the selector gates X11 on
  `$WAYLAND_DISPLAY` being **unset**, because under a Wayland compositor
  `$DISPLAY` is set by XWayland but its notion of focus is unreliable for native
  Wayland windows (§6 priority #5). This prevents the "picked X11 on
  GNOME-Wayland and reported wrong windows" trap.

---

## 11. Where Each Monitor Runs (thread summary)

| Monitor | Thread | Why |
|---|---|---|
| Windows | hook on message-loop thread (main, via `tao`); 100 ms poll thread | `WINEVENT_OUTOFCONTEXT` needs a pumped loop |
| macOS | background thread (`CFRunLoopRun` blocks) | tray/`tao` owns main |
| Hyprland | calling thread (`start_listener` blocks); optional poller thread | no GUI loop |
| foreign-toplevel (Wayland) | background thread (`EventQueue` dispatch loop); spawn-and-return `start()` | no blocking listener; runner parks main / drives tray |
| GNOME (extension client) | zbus signal subscription on a background thread; 1000 ms drift poll thread | no GUI loop; tray/ksni owns its D-Bus thread |
| AT-SPI | a11y-bus event subscription on a background thread; 1000 ms poll thread | as above |
| X11 | background thread | tray/park owns main |

(Full concurrency table in `SPEC_ARCHITECTURE.md` §6.)

---

## 12. Internal Window Filtering Reference (Windows)

The full ignore-list and the empty-title allowlist live in
`should_ignore_window` (`src/platforms/windows.rs`). When adding a new app that
spuriously grabs focus, add its **window class** (locale-independent), never its
title. Both Win11 (XAML island) and Win10 (classic) shell generations are
covered.

---

*Continue with `SPEC_UI.md`.*

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

// Dispatchers (return the right platform's impl):
pub fn create_monitor(verbose: bool) -> Box<dyn WindowMonitor>;
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

### 1.3 Empty-workspace semantics

- **Hyprland:** an empty workspace reports `WindowInfo { app_class: "", title: "" }`
  → payload `"\x1D"` → firmware deactivates any active layer. This is desired
  (no app focused ⇒ neutral keymap).
- **Windows / macOS:** no focus event is generated for "no window", so the
  keyboard retains the last-reported app until the next real focus change.

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

1. **`SetWinEventHook(EVENT_OBJECT_FOCUS, EVENT_OBJECT_FOCUS, …,
   WINEVENT_OUTOFCONTEXT)`** — the primary signal. The callback
   `event_proc(hwnd,…)` runs on the thread pumping the message loop (the `tao`
   tray loop on the shipped app). Each focus event → `handle_focus_change`.
2. **100 ms polling thread** (`GetForegroundWindow()` compared to
   `last_hwnd`) — a fallback for events the hook can miss. Deduped against
   `LAST_WINDOW_INFO`.

### 2.2 `handle_focus_change(hwnd)`
- `get_window_info(hwnd)` → `WindowInfo` (class via `GetClassNameW`, title via
  `GetWindowTextW`).
- Skip if `should_ignore_window`.
- Dedup against `LAST_WINDOW_INFO` (`Mutex<Option<(String,String)>>`) to kill
  feedback loops.
- `notify_qmk(&window_info, verbose)`.

### 2.3 Thread-safe globals (replaced former `static mut` UB)
- `G_VERBOSE: AtomicBool`
- `G_HOOK: AtomicIsize` (holds the `HWINEVENTHOOK` handle)
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
  `observeNotification:` → calls `get_active_window_info` → `notify_qmk`.
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
2. `~/.config/qmk-notifier/config.toml` (XDG-style fallback)
3. `/etc/qmk-notifier/config.toml` (system-wide last resort)

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
- Updates `last_window_state` (`Arc<Mutex<Option<WindowState>>>`) for dedup.

### 5.3 Reconnect backoff (fixes #7)
- `INITIAL_RECONNECT_MS = 100`, `MAX_RECONNECT_MS = 10_000`, growth `×3`.
- **Reset to initial** when a listener that stayed up ≥
  `STABLE_CONNECTION_THRESHOLD` (5 s) is lost, so long-uptime sessions don't
  stick at the 10 s cap.
- **Hard-fail** only if the very first attempt dies within 2 s of startup
  (Hyprland genuinely unavailable).

### 5.4 Polling strategies (two distinct ones)
- **Optional periodic poll** (`poll_interval_ms > 0`, default 0 = off): a thread
  polls `Client::get_active()` on the configured cadence and dedups against
  `last_window_state`. Corrects IPC drift (notably `movetoworkspacesilent`
  scratchpad dismissals where the `activewindow` event lags).
- **Poll burst after layer events** (`spawn_poll_burst`): 5× 100 ms polls after
  a layer open/close, to absorb the timing gap where focus hasn't settled at
  event time. Replaces the former permanent 100 ms poller.

### 5.5 `list_foreground_windows()` (tray dialog data)
`Clients::get()` filtered to `mapped`, mapped to `(class, title)`, with the
active window moved to front (so `.next()` reports the focused window).

---

## 6. X11 Monitor (`src/platforms/x11.rs`) — fallback, non-default build

- Built only with `--no-default-features` (no `hyprland`).
- `get_active_window_info()`: `xprop -root _NET_ACTIVE_WINDOW` → window id →
  `xprop -id <wid> WM_CLASS _NET_WM_NAME`. `WM_CLASS` second field (class) is
  preferred; first field (instance) is the fallback. `0x0` ⇒ empty workspace.
- **Fails loudly** if `xprop` is missing (never emits placeholder strings — #14).
- Polls every **500 ms** on a background thread (X11 focus changes are
  user-driven; latency is acceptable for a fallback).

---

## 7. Where Each Monitor Runs (thread summary)

| Monitor | Thread | Why |
|---|---|---|
| Windows | hook on message-loop thread (main, via `tao`); 100 ms poll thread | `WINEVENT_OUTOFCONTEXT` needs a pumped loop |
| macOS | background thread (`CFRunLoopRun` blocks) | tray/`tao` owns main |
| Hyprland | calling thread (`start_listener` blocks); optional poller thread | no GUI loop |
| X11 | background thread | tray/park owns main |

(Full concurrency table in `SPEC_ARCHITECTURE.md` §6.)

---

## 8. Internal Window Filtering Reference (Windows)

The full ignore-list and the empty-title allowlist live in
`should_ignore_window` (`src/platforms/windows.rs`). When adding a new app that
spuriously grabs focus, add its **window class** (locale-independent), never its
title. Both Win11 (XAML island) and Win10 (classic) shell generations are
covered.

---

*Continue with `SPEC_UI.md`.*

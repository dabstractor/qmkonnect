# QMKonnect Platform Layer — Deep-Dive Analysis

Scope: the platform abstraction (`src/platforms/`), the per-OS lifecycle runners
(`src/runners/`), and their interaction with `src/core`, `src/tray.rs`, and
`src/linux_tray.rs`. Findings are keyed to concrete file paths and line ranges.

Build verification: `cargo build` (Linux, default features = `hyprland` +
`macos`(inert) + `linux-tray`) **passes** in 21.86s, no warnings. The
Windows/macOS modules cannot be compiled on this host, so their findings are
based on static reading plus cross-referencing the runner/tray callers.

---

## 1. `src/platforms/mod.rs` — the trait & dispatchers

### Files Retrieved
- `src/platforms/mod.rs:1-31` — module/cfg gates + `WindowMonitor` trait.
- `src/platforms/mod.rs:32-61` — `create_monitor(verbose)`.
- `src/platforms/mod.rs:63-79` — `get_config_paths()`.
- `src/platforms/mod.rs:81-99` — `list_foreground_windows()`.
- `src/platforms/mod.rs:100-130` — `create_config_dir()`.
- `src/platforms/mod.rs:132-200` — `MockWindowMonitor` test helper.

### The `WindowMonitor` trait (`mod.rs:14-27`)
```rust
pub trait WindowMonitor: Send {
    fn platform_name(&self) -> &str;
    fn start(&mut self) -> Result<(), Box<dyn std::error::Error>>;

    #[allow(dead_code)]
    fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Default no-op; platform impls override where a real stop exists.
        Ok(())
    }
}
```
- Single concrete trait: `platform_name`, `start` (required), `stop` (default
  no-op). `Send` is required so the monitor can be moved onto the runner thread
  (Hyprland's `start()` blocks on the IPC listener — see the comment at
  `mod.rs:9-13`).
- There is **no `stop` requirement**: only Windows and X11 actually implement a
  meaningful `stop` (they flip an `AtomicBool`). macOS's `stop` (`macos.rs:179`)
  only sets a never-read field; Hyprland never returns from `start()` so its
  inherited no-op `stop` is unreachable.

### Module gating (`mod.rs:1-8`)
```rust
mod hyprland;                              // always parsed on linux, body cfg-gated
mod linux;                                 // #[cfg(target_os = "linux")]
#[cfg(target_os = "macos")] mod macos;
#[cfg(target_os = "windows")] mod windows;
#[cfg(all(target_os = "linux", not(feature = "hyprland")))] mod x11;
```
- `mod hyprland;` has **no outer `cfg`**; the file itself carries
  `#![cfg(all(target_os = "linux", feature = "hyprland"))]` at `hyprland.rs:1`,
  which is what gates it. The `x11` module is mutually exclusive with the
  `hyprland` feature.
- `pub use linux::*;` (`mod.rs:11`, `#[cfg(target_os = "linux")]`) re-exports the
  whole `linux.rs` public surface (`get_config_paths`, `create_config_dir`,
  `render_vidpid_rule`, `update_udev_rules`, `reload_udev_rules`,
  `resolve_config_for_reload`). This is how `main.rs` reaches
  `platforms::resolve_config_for_reload` / `platforms::update_udev_rules`.

### Dispatchers
- **`create_monitor`** (`mod.rs:32-61`): cfg-ladder returning
  `Box<dyn WindowMonitor>`. Linux uses `hyprland` when the feature is on, else
  `x11`; macOS → `MacOSMonitor`; Windows → `WindowsMonitor`. Unsupported OS →
  `Err`.
- **`get_config_paths`** (`mod.rs:63-79`): delegates per-OS. Note the Linux arm
  calls `linux::get_config_paths()` explicitly even though `pub use linux::*`
  already brings it in — harmless redundancy.
- **`list_foreground_windows`** (`mod.rs:81-99`): returns `Vec<(String,String)>`
  for macOS, Windows, and **Linux+hyprland only**. The final catch-all arm
  returns `Vec::new()`. **There is no X11 implementation** (see §5 /
  review-finding).
- **`create_config_dir`** (`mod.rs:100-130`): per-OS delegate with a portable
  XDG fallback for non-Linux/macOS/Windows.

---

## 2. `src/platforms/windows.rs` — WinEventHook + polling

### Files Retrieved
- `windows.rs:25-27` — process-global statics (former `static mut`, fixed by #5).
- `windows.rs:29-41` — `WindowsMonitor` struct.
- `windows.rs:43-111` — `start()`: hook install + polling thread.
- `windows.rs:112-133` — `stop()`.
- `windows.rs:137-147` — `event_proc` (Win32 callback).
- `windows.rs:149-196` — `handle_focus_change` (dedup + notify).
- `windows.rs:198-263` — `should_ignore_window` (filter).
- `windows.rs:265-330` — UWP `ApplicationFrameWindow` content resolution.
- `windows.rs:332-396` — `get_window_info`.
- `windows.rs:398-432` — `list_foreground_windows` + `enum_windows_proc`.
- `windows.rs:434-478` — config paths / dir.

### Detection mechanism — dual: hook + poll
1. **WinEventHook** (`start()`, `windows.rs:60-70`): `SetWinEventHook` for
   `EVENT_OBJECT_FOCUS`, range 0/0 (all processes/threads),
   `WINEVENT_OUTOFCONTEXT`. The handle is stored in the `G_HOOK` `AtomicIsize`.
2. **Polling fallback thread** (`windows.rs:96-110`): a spawned thread loops
   every **100 ms**, calling `GetForegroundWindow()`, and on an HWND change calls
   `handle_focus_change`. This is the belt-and-suspenders path; the comment at
   `windows.rs:92` calls it a "fallback".

### Thread model
- `start()` runs on the runner's calling thread. It installs the hook there,
  spawns the 100 ms poller, and returns immediately (it does **not** block).
- The `running: Arc<AtomicBool>` gates the poller loop; `stop()` flips it false
  and unhooks (`windows.rs:114-131`).
- The hook callback `event_proc` (`windows.rs:137-147`) is invoked by the OS on
  the thread that called `SetWinEventHook` and pumps messages.

### Window filtering — `should_ignore_window` (`windows.rs:198-263`)
- A hardcoded `ignore_classes` list: `ForegroundStaging`,
  `XamlExplorerHostIslandWindow`, several `Windows.UI.*` composition/input
  bridge classes, `TaskSwitcherWnd`, task switcher overlays, and shell/tray
  chrome (`TopLevelWindowForOverflowXamlIsland`, `NotifyIconOverflowWindow`,
  `Shell_TrayWnd`, `Shell_SecondaryTrayWnd`). Shell filtering is **by class, not
  title**, deliberately locale-independent (`windows.rs:240-256`).
- Deliberately **excludes** `ApplicationFrameWindow` and `CoreWindow` from the
  ignore list (`windows.rs:206-218`): `get_window_info` resolves the UWP frame
  to its content window first, so re-filtering `CoreWindow` would re-hide every
  UWP app.
- Empty titles are ignored *unless* the class is in an allowlist
  (`CASCADIA_HOSTING_WINDOW_CLASS`, `Chrome_WidgetWin_1`) — `windows.rs:245-258`.
- Titles shorter than 2 chars (non-empty) are dropped (`windows.rs:261-263`).

### UWP resolution (`windows.rs:265-396`)
- `APPLICATION_FRAME_CLASS = "ApplicationFrameWindow"`. When the focused window
  is the frame, `find_uwp_content_window` (`windows.rs:294-306`) walks descendants
  via `EnumChildWindows` and picks the first *visible* descendant whose PID
  differs from the frame's `ApplicationFrameHost.exe` PID. The content window's
  real class + title is then reported. If no content is hosted (mid-launch), it
  returns `Ok(None)` rather than reporting the empty frame (`windows.rs:364-371`).
- `get_window_info` also **trims** window text (`windows.rs:386`) to avoid
  trailing-space bloat in HID messages.

### Dedup (`handle_focus_change`, `windows.rs:162-180`)
- `LAST_WINDOW_INFO: Mutex<Option<(String,String)>>` holds the last
  `(app_class, title)`; if the new pair is identical, it is dropped. This is the
  only guard against hook+poll double-reporting the same window.

### `list_foreground_windows` (`windows.rs:398-432`)
- `EnumWindows` + `enum_windows_proc` callback; for each visible top-level
  window it runs the **same** `get_window_info` → `should_ignore_window` pipeline
  the live monitor uses, so the tray "Show Window Information" dialog shows
  exactly the values you can match in a QMK config.

### Config paths (`windows.rs:434-478`)
- `%APPDATA%\QMKonnect\config.toml` (primary) →
  `%LOCALAPPDATA%\QMKonnect\config.toml` (secondary) → exe-dir `config.toml`
  (fallback). `create_config_dir` uses `%APPDATA%\QMKonnect`.

---

## 3. `src/platforms/macos.rs` — NSWorkspace observer + CGWindowList

### Files Retrieved
- `macos.rs:1-58` — externs, screen-recording permission FFI, `VERBOSE` static.
- `macos.rs:46-57` — `MacOSMonitor` struct.
- `macos.rs:58-145` — `ensure_screen_recording_permission` + `setup_observers`.
- `macos.rs:147-184` — `start()` / `stop()`.
- `macos.rs:187-258` — `get_active_window_info`.
- `macos.rs:260-309` — `list_foreground_windows`.
- `macos.rs:311-363` — `build_owner_title_map`.
- `macos.rs:365-432` — string helpers + config paths/dir.

### Detection mechanism — NSWorkspaceDidActivateApplicationNotification
- `start()` (`macos.rs:148-170`): requests screen-recording permission
  (non-blocking — `CGRequestScreenCaptureAccess` returns immediately, issue #13),
  registers an observer, fires an initial notification, then **blocks forever** in
  `CFRunLoopRun()` (`macos.rs:167`).
- `setup_observers` (`macos.rs:60-145`): declares a custom Obj-C class
  `RustNotificationObserver` (guarded by `Class::get(...).is_none()` so repeated
  `start()` is safe), adds a method `observeNotification:` that, on each app
  activation, calls `get_active_window_info()` and notifies QMK. Registers it for
  `NSWorkspaceDidActivateApplicationNotification` on the shared workspace's
  notification center.
- The verbose flag is shared with the callback via the `VERBOSE: AtomicBool`
  static (`macos.rs:44`), replacing a former `static mut`.

### Thread model
- `start()` blocks the calling thread on `CFRunLoopRun()`. The macOS runner
  (`runners/macos.rs`) therefore spawns the monitor on a **separate thread** and
  runs `tray::setup_tray()` (tao event loop) on the main thread. `drop(monitor_thread)`
  on exit is a deliberate non-join (comment `runners/macos.rs:50-53`).
- `running: bool` field is **dead state**: written at `macos.rs:142` and
  `macos.rs:180`, **never read** anywhere. `stop()` cannot actually stop
  `CFRunLoopRun` from another thread (acknowledged at `macos.rs:175-178`).

### Window info — `get_active_window_info` (`macos.rs:187-258`)
- `NSWorkspace.sharedWorkspace.frontmostApplication.localizedName` → app name.
- Title via `CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly)`,
  matching the CG window whose `kCGWindowOwnerName` equals the frontmost app's
  name, then reading `kCGWindowName`. **Requires Screen Recording permission** —
  without it the title is silently empty (the app name still works).

### `list_foreground_windows` (`macos.rs:260-309`)
- Enumerates `runningApplications`, keeps only `activationPolicy == 0` (Regular)
  and `isFinishedLaunching`, pairs each `localizedName` with a title from
  `build_owner_title_map()` (the same CG-window-list owner→title map), sorts
  alphabetically by class.

### `should_ignore_window` — **not present** on macOS
- macOS has no equivalent of Windows' class blocklist. Filtering is implicit via
  `activationPolicy == Regular` (Dock apps only), which excludes menu-bar/helper
  agents. There is no title-length or empty-title guard.

### Config paths (`macos.rs:385-432`)
- `~/Library/Application Support/QMKonnect/config.toml` (primary) →
  `~/.config/qmk-notifier/config.toml` (XDG fallback) →
  `/etc/qmk-notifier/config.toml` (system). `create_config_dir` uses the
  Application Support path.

---

## 4. `src/platforms/hyprland.rs` — EventListener IPC + reconnect + polling

### Files Retrieved
- `hyprland.rs:13-37` — reconnect/poll tuning constants.
- `hyprland.rs:39-58` — `HyprlandMonitor` struct.
- `hyprland.rs:53-209` — `start()` (the big reconnect loop).
- `hyprland.rs:211-272` — `wait_for_hyprland`.
- `hyprland.rs:274-307` — `hyprland_socket_is_live`.
- `hyprland.rs:309-367` — `check_hyprland_environment` (self-heal).
- `hyprland.rs:369-448` — poll burst + `poll_window_state`.
- `hyprland.rs:450-522` — `handle_window_state_change`.
- `hyprland.rs:524-549` — `list_foreground_windows`.
- `hyprland.rs:551-565` — `handle_workspace_change`.

### Detection mechanism — Hyprland IPC `EventListener`
`start()` (`hyprland.rs:55-209`) is a reconnect loop:
1. `wait_for_hyprland()` — exponential startup wait (cap 30 s, delay doubling to
   2 s) until both the socket env is present *and* `Monitors::get()` IPC works.
2. `check_hyprland_environment()` — self-heal: re-resolves a **live** Hyprland
   instance signature and republishes `$HYPRLAND_INSTANCE_SIGNATURE`.
3. Builds a fresh `EventListener`, registers handlers, then `listener.start_listener()`
   which **blocks** until the connection drops.

Handlers registered each attempt:
- `add_active_window_changed_handler` → `handle_window_state_change`.
- `add_workspace_changed_handler` → `handle_workspace_change` (re-queries active
  window to catch empty-workspace transitions).
- `add_window_closed_handler` → `handle_window_state_change`.
- `add_layer_opened_handler` / `add_layer_closed_handler` → immediate state query
  **plus** `spawn_poll_burst` (5×100 ms) to absorb the timing gap where focus
  hasn't settled (scratchpad dismissal via `movetoworkspacesilent` — the comment
  at `hyprland.rs:64-74`). Replaced a former permanent 100 ms poller (#8).

### Reconnect / backoff strategy (`hyprland.rs:13-37, 188-207`)
- `INITIAL_RECONNECT_MS = 100`, `MAX_RECONNECT_MS = 10_000`, growth factor **×3**
  (not ×2).
- `STABLE_CONNECTION_THRESHOLD = 5 s`: if a listener stayed up ≥5 s, the backoff
  **resets** to the initial value on loss (#7 — long-uptime sessions don't get
  stuck at the 10 s cap).
- Hard-fail only if the **very first** attempt dies within 2 s of process start
  (`hyprland.rs:189-196`), with a "is Hyprland running?" message.

### Optional periodic poll (`hyprland.rs:78-99`)
- Driven by `poll_interval_ms` from config (default **0 = disabled**, see
  `core/mod.rs` `DEFAULT_POLL_INTERVAL_MS`). When >0, a thread polls
  `Client::get_active()` every N ms; `poll_window_state` dedups against
  `last_window_state`, so steady-state polls are no-ops (one cheap IPC each).

### Socket liveness — `hyprland_socket_is_live` (`hyprland.rs:274-307`)
- Not a `path.exists()` check (a crashed Hyprland leaves a stale `.socket.sock`).
  It actually `connect(2)`s, on a short-lived thread with a hard
  `SOCKET_PROBE_TIMEOUT = 500 ms` (no `UnixStream::connect_timeout` exists).
  Hermetically unit-tested (`hyprland.rs:600-635`).

### Self-heal — `check_hyprland_environment` (`hyprland.rs:309-367`)
- Candidate signatures: `$HYPRLAND_INSTANCE_SIGNATURE` first, then every dir under
  `$XDG_RUNTIME_DIR/hypr/`. First with a live `.socket.sock` wins; if it differs
  from the env var, republish it so the `hyprland` crate retargets. No `ps` shell-out
  (#15).

### Window filtering — **none**
- No `should_ignore_window` equivalent. Whatever Hyprland reports as the active
  client (class + title) is forwarded. Empty workspace is reported as
  `WindowInfo::new("", "")` (`hyprland.rs:496-517`).

### `list_foreground_windows` (`hyprland.rs:524-549`)
- `Clients::get()`, keep `mapped` windows, `(class, title)`, then swap the active
  window (`Client::get_active()`) to index 0 so `.next()`-style callers report the
  focused window. `#[allow(dead_code)]` — only reached when a tray build links it.

---

## 5. `src/platforms/x11.rs` — xprop-polling fallback

### Files Retrieved
- `x11.rs:12-20` — `X11Monitor` struct.
- `x11.rs:22-85` — `get_active_window_info` (xprop-based).
- `x11.rs:87-156` — `start()` (polling thread).
- `x11.rs:158-168` — `stop()`.

### Detection mechanism — pure polling (no events)
- `start()` (`x11.rs:89-145`): asserts `xprop -version` exists (hard-fails
  otherwise — issue #14, never emits placeholder strings), then spawns a thread
  polling every **500 ms**.
- `get_active_window_info` (`x11.rs:24-85`): two `xprop` subprocess invocations
  per cycle:
  1. `xprop -root _NET_ACTIVE_WINDOW` → parse window id (`0x0`/`0` = empty desktop
     → `Ok(None)`).
  2. `xprop -id <wid> WM_CLASS _NET_WM_NAME` → class (prefers the 2nd
     `WM_CLASS` field, the class, falling back to the instance) + title.
- The poll loop dedups against `last_window: Option<(String,String)>` locally
  (`x11.rs:114-126`); transitions to/from empty workspace notify with empty
  `WindowInfo`.

### Thread model
- The monitor's whole life is the polling thread; `running: Arc<AtomicBool>`
  gates it. `start()` returns immediately. Each poll constructs a **throwaway**
  `X11Monitor::new(verbose)` probe (`x11.rs:117`) — harmless but slightly
  wasteful (the `running` Arc isn't shared with the probe; it's only used as a
  namespace for `get_active_window_info`).

### Window filtering — **none**
- No `should_ignore_window`; whatever xprop returns is forwarded.

### `list_foreground_windows` — **not implemented**
- `X11Monitor` has no enumeration path. The `mod.rs` dispatcher returns
  `Vec::new()` for the X11 build, so the SNI tray "Show Window Information"
  item (`linux_tray.rs:303`) reports "No foreground windows detected." even on a
  populated X11 desktop. **Feature gap** (see review-findings).

### Config paths
- None in this file; Linux paths come from `linux.rs` via `pub use linux::*`.

---

## 6. `src/platforms/linux.rs` — udev rule rendering + root-aware reload

### Files Retrieved
- `linux.rs:10-93` — `render_vidpid_rule` (+ the critical udev line-semantics doc).
- `linux.rs:95-114` — `rule_line_has_leading_match_key`.
- `linux.rs:55-93` — `is_rule_globally_dangerous` (legacy repair detector).
- `linux.rs:116-155` — `get_config_paths`.
- `linux.rs:157-237` — `resolve_config_for_reload` (root-aware, #26).
- `linux.rs:239-293` — `resolve_homes` / `getent_home`.
- `linux.rs:295-401` — `update_udev_rules` / `write_rule_atomic` / `purge_rule`.
- `linux.rs:402-432` — `reload_udev_rules` + `create_config_dir`.
- `linux.rs:434-600` — regression tests.

This file is **not** a `WindowMonitor`; it holds the Linux config/udev helpers
re-exported by `mod.rs`. Gated `#![cfg(target_os = "linux")]` and compiled for
**all** Linux builds (hyprland or x11).

### `render_vidpid_rule` (`linux.rs:38-57`)
- Returns `None` when both VID/PID are unset (the static usage-page rule
  `69-qmkonnect-rawhid.rules` covers any 0xFF60/0x61 device). Otherwise emits a
  **single physical udev line** beginning with `KERNEL=="hidraw*",`.
- The doc comment (`linux.rs:26-37`) is load-bearing: udev ends a rule at every
  newline (only a trailing `\` continues; a trailing comma does **not**), so a
  line whose first key is an assignment (`GROUP=`/`MODE=`/`TAG+=`…) matches
  **every device on the host**. This is the `BUG_linux_udev_global_device_permissions`
  regression.
- When only one of VID/PID is set, only that `ATTRS{...}` clause is emitted (udev
  `ATTRS` can't wildcard).

### Legacy-rule repair — `is_rule_globally_dangerous` (`linux.rs:55-93`)
- Joins `\`-continuations, then flags any remaining non-comment line whose first
  key isn't a match key (`==`/`!=`). `rule_line_has_leading_match_key`
  (`linux.rs:95-114`) parses the leading `[A-Z_]+{...}` then checks the operator.

### Root-aware config resolution — `resolve_config_for_reload` (`linux.rs:157-237`)
- Fixes #26: under `sudo`, `HOME=/root`, so a plain search would never find the
  invoking user's config and reload would silently no-op.
- Order: explicit `--config` → (when root) invoking-user home via
  `--uid`/`--user`/`$SUDO_UID`/`$SUDO_USER`/`$PKEXEC_UID` (resolved through
  `getent passwd`) → single-config `/home/*` scan → normal `get_config_paths()`
  → **fail loudly** (never silently no-op).
- `is_root = libc::geteuid() == 0` (`linux.rs:172`) — the only `libc` use, per
  the Cargo note.

### `update_udev_rules` (`linux.rs:295-342`)
- If both IDs unset and the on-disk rule is dangerous → `purge_rule`; else no-op.
- Otherwise overwrites atomically (`write_rule_atomic`, `tempfile::NamedTempFile`
  in the rules dir + `persist`) when root. Non-root: prints a copy-paste
  `sudo tee …` instead of attempting `sudo` (systemd/GUI contexts have no TTY).

### Config paths (`linux.rs:116-155`)
- `$XDG_CONFIG_HOME/qmk-notifier/config.toml` →
  `~/.config/qmk-notifier/config.toml` → `/etc/qmk-notifier/config.toml`.

---

## 7. `src/runners/` — per-OS lifecycle

### `src/runners/mod.rs`
- `PlatformRunner` trait (`mod.rs:13-16`): single required method
  `fn run(&mut self, args: &[String]) -> Result<(), Box<dyn Error>>`.
- `create_runner(verbose)` (`mod.rs:19-37`): cfg-ladder → `WindowsRunner` /
  `MacOSRunner` / `LinuxRunner`. Unsupported OS → `Err`.
- Modules gated per-OS.

### `src/runners/windows.rs`
- `WindowsRunner::run` (`windows.rs:81-99`): `--console` → `run_console_mode`;
  `--tray-app` (or default) → `run_tray_app`.
- **Single-instance guard** (`windows.rs:23-38`): named mutex via the
  `single-instance` crate ("qmkonnect-app-id"). On success the owner is
  `Box::leak`-ed to hold the mutex for process lifetime (replaces a former
  `static mut INSTANCE` data race, #5).
- **`run_tray_app`** (`windows.rs:54-79`): guard → `create_monitor` →
  `startup_device_probe` (read-only, #16) → `monitor.start()` (non-blocking) →
  `tray::setup_tray()` (blocks).
- **`run_console_mode`** (`windows.rs:40-52`): no tray; main thread ends in
  `loop { sleep(1s) }`. **It never pumps Win32 messages**, so in console mode the
  `EVENT_OBJECT_FOCUS` hook callbacks are never delivered — detection relies
  entirely on the 100 ms polling thread (see §2 / review-finding).
- Ctrl-C handler calls `process::exit(0)`.

### `src/runners/macos.rs`
- `run` (`macos.rs:17-54`): `create_monitor` → `startup_device_probe` → Ctrl-C
  handler → spawns `monitor.start()` on a **thread** (it blocks on `CFRunLoopRun`)
  → `tray::setup_tray()` on the main thread → on tray exit, `drop(monitor_thread)`
  (deliberately not joined).

### `src/runners/linux.rs`
- `run` (`linux.rs:17-86`) is split by cfg:
  - **Hyprland** (`linux.rs:41-48`): optionally `linux_tray::spawn()` (SNI, on the
    `linux-tray` feature), then `monitor.start()?` **blocks the main thread** on
    the IPC listener.
  - **X11** (`linux.rs:55-85`): `linux_tray::spawn()` if enabled; spawn monitor on
    a thread; if no `linux-tray`, drive `tray::setup_tray()` (tao loop) on the main
    thread; if `linux-tray` is on, `thread::park()` loop (ksni owns its own D-Bus
    thread).
- Both arms share: `create_monitor` → `startup_device_probe` → Ctrl-C handler.
- Comment (`linux.rs:30-32`): relies on systemd `Restart=always` + release
  `panic = "abort"` for crash recovery instead of former `catch_unwind`.

---

## 8. Architecture — how the pieces connect

```
main.rs
 ├─ CLI dispatch: -c (create config) / -r (reload) / -l / --list-devices
 │                / --show-window-info / default → runner
 ├─ runners::create_runner(verbose) ──▶ PlatformRunner::run(args)
 │     └─ windows:  guard → monitor.start() (async) → tray::setup_tray() (block)
 │        macos:   monitor.start() on thread → tray::setup_tray() (block, main)
 │        linux:   monitor.start() (block, hyprland) OR thread + tray/park (x11)
 └─ platforms::create_monitor ──▶ Box<dyn WindowMonitor>
       ├─ WindowsMonitor: WinEventHook + 100ms poller ──▶ notifier::notify_qmk
       ├─ MacOSMonitor:   NSWorkspace observer + CFRunLoopRun ──▶ notify_qmk
       ├─ HyprlandMonitor: EventListener IPC + reconnect loop ──▶ notify_qmk
       └─ X11Monitor:    500ms xprop poller ──▶ notify_qmk
```

- **Data shape**: every monitor produces `core::types::WindowInfo { app_class,
  title }` and hands it to `core::notifier::notify_qmk`, which formats
  `"{app_class}\x1D{title}"` and sends it through a single **debounce worker**
  (`core/notifier.rs`) to the QMK keyboard via the `qmk_notifier` crate.
- **Debounce**: one long-lived worker thread (`WORKER`), a single
  `DebounceState` behind a `Mutex`+`Condvar`. First message sends immediately;
  subsequent ones within the window (default 50 ms, configurable) collapse to one
  follow-up of the newest value. Interval is loaded from config once at init.
- **Device match**: `core::notifier::configured_filter()` re-reads config.toml on
  every call (VID/PID optional = auto-discovery; usage page/usage default
  `0xFF60`/`0x61`). `startup_device_probe`, `is_device_connected`, and `notify`
  all share it. All three are **read-only HID enumeration** — they never open the
  device.
- **Tray surfaces**:
  - macOS/Windows: `src/tray.rs` (tao + tray-icon). Owns Settings dialog,
    "Show Window Information", device-status line (polled every 3 s), launch-at-
    login (macOS SMAppService / Windows HKCU Run key via `src/autostart.rs`).
    **Gated out** of the Linux+hyprland build (`tray.rs:1`).
  - Linux: `src/linux_tray.rs` (ksni StatusNotifierItem over D-Bus, on the
    `linux-tray` feature). Device-status line polled every 1 s; dark/light icon
    via the freedesktop color-scheme portal; "Show Window Information" via a
    native GTK popup (`gtk_dialog`) with a zenity fallback; Settings via zenity.

---

## 9. Code quality / compile gaps / risks

### Confirmed gaps & issues

1. **`running` field on `MacOSMonitor` is dead state** (`macos.rs:50, 142, 180`).
   Written twice, never read. `stop()` can't actually halt `CFRunLoopRun` from
   another thread. *Severity: low* (cosmetic; process exit handles cleanup).
2. **X11 build has no `list_foreground_windows`** (`x11.rs`, `mod.rs:81-99`).
   The SNI tray "Show Window Information" reports empty on the X11+`linux-tray`
   build (`linux_tray.rs:303` → `mod.rs` catch-all `Vec::new()`). The data is
   obtainable (`xprop -root` + per-window props) but not wired up. *Severity:
   medium* (feature silently degraded on a non-default build config; default
   features are `hyprland`, so most users hit the Hyprland path which *is*
   implemented).
3. **Windows console-mode hook is dead** (`runners/windows.rs:40-52`).
   `run_console_mode` never pumps Win32 messages, so `WINEVENT_OUTOFCONTEXT`
   `event_proc` callbacks are never delivered; detection depends solely on the
   100 ms poller. Tray mode is fine (tao pumps messages). *Severity: low-medium*
   (console mode is a debugging path; the poller covers it).
4. **`MacOSMonitor` has no window-class filter** (no `should_ignore_window`).
   Filtering is only implicit via `activationPolicy == Regular`. No title-length
   or empty-title guard. *Severity: low* (Regular-policy apps are already
   user-meaningful).
5. **Redundant config-path call** (`mod.rs:71` `linux::get_config_paths()` while
   `pub use linux::*` already imports it). Harmless.

### Strengths (things done well)
- Thread-safety hardening: all former `static mut` globals replaced with atomics
  / `Mutex` (#5: `G_VERBOSE`, `G_HOOK`, `VERBOSE`; `single-instance` leak instead
  of `static mut INSTANCE`).
- udev rule rendering is single-line-safe and has strong regression coverage
  (`linux.rs:434-600`) including the exact broken multi-line form.
- Root-aware reload (`resolve_config_for_reload`) fails loudly instead of
  silently no-op'ing under `sudo` (#26).
- Hyprland socket-liveness probe (`hyprland_socket_is_live`) is hermetically
  tested and correctly distinguishes a live socket from a crashed-instance leftover.
- Read-only HID enumeration everywhere the device presence is checked — never
  disturbs the keyboard.

### Tests present
- `platforms/mod.rs`: `MockWindowMonitor` trait test.
- `platforms/hyprland.rs`: `WindowState` equality, monitor creation, three
  socket-liveness probe tests (listening / dead-leftover / missing).
- `platforms/linux.rs`: extensive udev rule rendering + dangerous-rule detection
  regression suite (8 tests).
- `core/notifier.rs`: debounce/coalescing behavior suite (6 tests).
- `core/mod.rs`: config parsing round-trip tests (4 tests).

No unit tests in `windows.rs` / `macos.rs` / `x11.rs` (all require the host OS /
a live desktop, which the test infra doesn't mock).

---

## 10. Start Here (for an agent acting on this)

1. **`src/platforms/mod.rs`** — read first; it defines the `WindowMonitor` trait,
   the cfg-gated module structure, and the four dispatchers that map a request to
   a concrete platform impl.
2. **`src/core/notifier.rs`** — the universal sink every monitor writes to
   (`notify_qmk` + the debounce worker); essential to understand before touching
   any monitor.
3. Then the target platform file. The most behaviorally complex is
   **`src/platforms/hyprland.rs`** (reconnect loop, socket self-heal, poll burst).
   The most filtering-heavy is **`src/platforms/windows.rs`** (UWP resolution +
   class blocklist).
4. **`src/runners/<os>.rs`** for any lifecycle/threading change — monitor↔tray
   threading differs per OS (block-on-main vs. thread+park vs. thread+tray-loop).

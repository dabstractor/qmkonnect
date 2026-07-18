# QMKonnect Core Layer — Deep-Dive Analysis

Scope: the four core files that form the platform-independent engine of QMKonnect.

| File | Lines | Role |
|------|-------|------|
| `src/main.rs` | 280 | CLI dispatch, logging init, entry point |
| `src/core/mod.rs` | 214 | `Config` struct, parse/render/default, timing helpers |
| `src/core/notifier.rs` | 604 | `Notifier` trait, `QmkNotifier`, debouncer, `DeviceFilter`, probes |
| `src/core/types.rs` | 37 | `WindowInfo` value type |

Supporting context pulled in for completeness: `src/core/mod.rs` re-exports the
`platforms` and `runners` modules that the core depends on, and `Cargo.toml`
pins `qmk_notifier = "0.2.1"` (git tag) — its public API (`run`, `RunParameters`,
`RunCommand`, `DEFAULT_USAGE_PAGE`, `DEFAULT_USAGE`, `REPORT_LENGTH`) is consumed
by `notifier.rs` and is documented below.

---

## 1. `src/main.rs` — Entry point & CLI dispatch

### 1.1 Module wiring (lines 1–26)
- `#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]` — on
  Windows the binary is a GUI app (no console) by default; the `--console` and
  `--tray-app` flags (documented in `print_help`) re-establish a console session.
- Top-level module declarations:
  - `core`, `platforms`, `runners`, `tray` — always compiled.
  - `autostart` — `#[cfg(target_os = "windows")]` only (HKCU `Run` key).
  - `linux_tray` — `#[cfg(all(target_os = "linux", feature = "linux-tray"))]` only
    (StatusNotifierItem), opt-in feature; absent from the default build.

### 1.2 Logging init (lines 28–52)
Two platform-gated `init_logging()` overloads:
- **Windows** (`init_logging`, lines 28–46): tries `eventlog::init("QMKonnect",
  log::Level::Info)` first; on failure falls back to `env_logger::init()` and
  prints an `eprintln!` notice. The fallback returns `Ok(())` regardless, so a
  logging backend failure never aborts startup.
- **Non-Windows** (`init_logging`, lines 48–52): **no-op** (returns `Ok(())`).
  The comment says "env_logger is only available on Windows in this
  configuration." → **Logging on macOS/Linux does nothing unless a platform
  runner wires its own logger** (see Gap G1).

### 1.3 `main()` / `run()` dispatch (lines 54–128)
- `main()` (54–68): initializes logging, then calls `run()`, mapping any error
  to `error!` (Windows) / `eprintln!` (elsewhere) + `process::exit(1)`.
- `run()` (70–128): manually scans `env::args()` (no clap at the binary level —
  clap lives inside the `qmk_notifier` library, unused here). Flag handling, in
  order of precedence (first match wins):
  1. `-h`/`--help` → `print_help()` (130–171), returns `Ok`.
  2. `-v`/`--verbose` → only sets the `verbose: bool` local; does **not** short-
     circuit. It is threaded into `-r`/`--reload` and into the runner.
  3. `-c`/`--config` → `create_config()` (269–280).
  4. `-r`/`--reload` → `reload_config(verbose, config, user, uid)` (204–267).
     Value flags `--config <path>`, `--user <name>`, `--uid <n>` are parsed via
     `parse_value_flag` (190–202) which supports both `--flag value` and
     `--flag=value` syntax.
  5. `-l`/`--list` → `print_platforms()` (173–184).
  6. `--list-devices` → `crate::core::notifier::list_devices()` (read-only HID
     enumeration, issue #17).
  7. `--show-window-info` (macOS/Windows only) → renders the window-info dialog
     without the tray, for isolated testing of the window path.
  8. **Else** (default run) → `runners::create_runner(verbose)?.run(&args)`.

### 1.4 `reload_config()` (lines 204–267)
- Resolves the config file:
  - Linux: `platforms::resolve_config_for_reload(config, user, uid)?` — root-aware
    (under `sudo`, `HOME=/root` so it resolves the invoking user via
    `$SUDO_UID`/`$SUDO_USER`/`getent`, or the explicit `--user`/`--uid` flags),
    and **fails loudly** rather than silently no-op'ing (issue #26).
  - Non-Linux: explicit `config` path if given & exists, else `get_config_path()`
    (173–188) which walks `platforms::get_config_paths()` for the first existing
    file. `get_config_path` is `#[cfg(not(target_os = "linux"))]`.
- Parses via `core::parse_config`, prints the resolved VID/PID (or "auto") when
  verbose.
- Linux only: `platforms::update_udev_rules(vendor_id, product_id, verbose)` and
  `platforms::reload_udev_rules()`. Both are wrapped in `if let Err(e)` that only
  prints a warning when `verbose` — **failures are otherwise swallowed** (Gap G2).
- Both VID/PID are `Option<u16>`; `update_udev_rules` no-ops cleanly when both are
  `None` (auto-discovery covered by the static udev rule).

### 1.5 `create_config()` (lines 269–280)
`platforms::create_config_dir()?` then `core::create_default_config(&config_dir
.join("config.toml"))?`. Zero-config default: every device-identifying field is
written commented out, so a fresh install auto-discovers any QMK keyboard by
usage page `0xFF60` / usage `0x61`.

### 1.6 `parse_value_flag()` (lines 190–202)
```rust
fn parse_value_flag(args: &[String], name: &str) -> Option<String>
```
Iterates argv; returns the token after `name` (space form) or the substring after
`name=` (equals form). Used only by `--reload`'s `--config`/`--user`/`--uid`.

---

## 2. `src/core/mod.rs` — Config model, parse, render, timing

### 2.1 Module exports (lines 1–2)
```rust
pub mod notifier;
pub mod types;
```
No `platforms`/`runners` declarations here (those are siblings at the crate
root). The crate root `mod core;` exposes this module as `crate::core`.

### 2.2 `Config` struct (lines 15–39)
```rust
#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct Config {
    #[serde(default)] pub vendor_id: Option<u16>,      // None = match any
    #[serde(default)] pub product_id: Option<u16>,     // None = match any
    #[serde(default)] pub usage_page: Option<u16>,     // None = 0xFF60 default
    #[serde(default)] pub usage: Option<u16>,          // None = 0x61 default
    #[serde(default = "default_debounce_ms")] pub debounce_ms: u64,       // default 50
    #[serde(default = "default_poll_interval_ms")] pub poll_interval_ms: u64, // default 0
}
```
Design points:
- **Auto-discovery by default**: VID/PID/usage-page/usage all deserialize to
  `None` when absent → "match any QMK keyboard by 0xFF60/0x61". Legacy files
  with explicit `vendor_id = 0xfeed` keep working (become `Some(0xfeed)`).
- **No validation / normalization** is done in `parse_config` (the TOML parser
  alone enforces types). A negative or absurd `debounce_ms` is accepted as-is;
  `0` has a defined meaning (disable debouncing) but is not range-checked (Gap G3).
- `Default` derive is present but **never used** at runtime (the canonical path
  is `render_config_body(None, None)` → file, then re-parse). The `Default`
  derive gives `debounce_ms = 0`/`poll_interval_ms = 0` because the serde default
  functions are *not* invoked by `Default` — so `Config::default()` differs from
  a parsed empty file (`50`/`0`). Subtle inconsistency (Gap G4).

### 2.3 Constants & default functions (lines 41–53)
```rust
const DEFAULT_DEBOUNCE_MS: u64 = 50;
const DEFAULT_POLL_INTERVAL_MS: u64 = 0;
fn default_debounce_ms() -> u64 { DEFAULT_DEBOUNCE_MS }
fn default_poll_interval_ms() -> u64 { DEFAULT_POLL_INTERVAL_MS }
```

### 2.4 `now_ms()` (lines 55–60)
```rust
pub fn now_ms() -> u128
```
Process-local monotonic epoch via `static START: OnceLock<Instant>` initialized to
`Instant::now()` on first call. Used only for verbose log timestamps (`[<ms>ms]`
prefixes in `notifier.rs`). Avoids wall-clock skew. Not used for timing
decisions (those use `Instant` directly inside `DebounceState`).

### 2.5 Config-reading helpers (lines 63–78)
```rust
pub fn configured_debounce_ms() -> u64           // = configured_timing().0
pub fn configured_timing() -> (u64, u64)         // (debounce_ms, poll_interval_ms)
```
`configured_timing` walks `crate::platforms::get_config_paths()`, parses the first
existing file, and returns `(cfg.debounce_ms, cfg.poll_interval_ms)`; on any
error or missing file it returns the defaults `(50, 0)`. **Read per-call** so a
config edit takes effect on the next notification without a restart — but the
debounce `interval` is only read once at `STATE` init (see §3.6 Gap G5).

### 2.6 `parse_config()` (lines 80–92)
```rust
pub fn parse_config(config_path: &Path) -> Result<Config, Box<dyn Error>>
```
`fs::read_to_string` → `toml::from_str`. Boxed dynamic error. Comment notes "no
need to normalize or validate" — see Gap G3.

### 2.7 `render_config_body()` (lines 94–130)
```rust
pub fn render_config_body(vendor_id: Option<u16>, product_id: Option<u16>) -> String
```
Renders a `config.toml` body. VID/PID lines are explicit (`vendor_id  = 0x{v:04x}`)
when `Some`, commented (`# vendor_id  = 0xfeed   # unset: auto-discovery`) when
`None`. Always comments out `usage_page`, `usage`, `debounce_ms`,
`poll_interval_ms` as hints. This is the **single source of truth** for the file
format — used by both `create_default_config` and every platform's settings-dialog
write path so they stay in sync. Unit-tested for round-trip (`render_config_body_round_trips`).

### 2.8 `create_default_config()` (lines 132–155)
```rust
pub fn create_default_config(config_path: &Path) -> Result<(), Box<dyn Error>>
```
- No-op (with a "already exists" message) if the file already exists.
- Creates parent dir with `fs::create_dir_all`.
- Writes `render_config_body(None, None)` and prints guidance.

### 2.9 Tests (lines 157–214)
Four unit tests covering: empty config → all-`None` device fields + serde default
timing; legacy explicit-IDs config; partial (usage_page only); render round-trip
for both `None`/`Some` forms. Coverage is solid for the parse/render contract.

---

## 3. `src/core/notifier.rs` — Notifier trait, debouncer, device filter, probes

### 3.1 `Notifier` trait & `QmkNotifier` (lines 12–17)
```rust
pub trait Notifier: Send + Sync {
    fn notify(&self, message: String) -> Result<(), Box<dyn Error + Send + Sync>>;
}
pub struct QmkNotifier;
```
A single-method trait abstracting "send a string to the keyboard". The
`Send + Sync` bound lets the trait object live behind an `Arc<Mutex<Box<dyn
Notifier>>>` (the global `NOTIFIER`, §3.5). `QmkNotifier` is a unit struct — all
state is in the `qmk_notifier` crate's global `DeviceCache`.

### 3.2 `DeviceFilter` & `configured_filter()` (lines 23–55)
```rust
pub struct DeviceFilter {
    pub vendor_id: Option<u16>,   // None = match any
    pub product_id: Option<u16>,  // None = match any
    pub usage_page: u16,          // always set (default 0xFF60)
    pub usage: u16,               // always set (default 0x61)
}
fn configured_filter() -> DeviceFilter   // private (not pub)
```
- Reads the config per-call (same `platforms::get_config_paths()` walk as
  `configured_timing`). VID/PID default to `None`; usage_page/usage default to
  `qmk_notifier::DEFAULT_USAGE_PAGE` (`0xFF60`) / `qmk_notifier::DEFAULT_USAGE`
  (`0x61`). Config edits take effect without restart.
- **The matching predicate is duplicated in three places** (Gap G6, severity
  medium — DRY/consistency risk):
  1. `startup_device_probe` (lines 77–125): inline `api.device_list().any(|d|
     d.usage_page() == f.usage_page && d.usage() == f.usage && f.vendor_id.is_none_or(..)
     && f.product_id.is_none_or(..))`.
  2. `is_device_connected` (lines 127–140): the **exact same** inline closure.
  3. Inside `qmk_notifier` (`device_matches`, core.rs) — the authoritative one.

### 3.3 `list_devices()` (lines 57–75)
```rust
pub fn list_devices() -> Result<(), Box<dyn Error>>
```
Read-only HID enumeration (never opens a device) backing `--list-devices`
(issue #17). Prints `VID:PID  usage_page:usage  product` for every device.

### 3.4 `startup_device_probe()` & `is_device_connected()` (lines 77–140)
- `startup_device_probe(verbose: bool)` (77–125): **one-time** startup check
  (read-only enumeration, never opens/sends — cannot disturb the keyboard). Prints
  a clear diagnostic when the configured device is absent, instead of the runtime
  path's silent retry-and-give-up (issue #16). Distinguishes a found/verbose-print
  case from a not-found case with actionable guidance.
- `is_device_connected() -> bool` (127–140): backs the tray status indicator
  (macOS line 2 / Linux SNI status / Windows status item). The tray polls this on
  a background thread and refreshes only on change. Treats any HID-enumeration
  failure as "absent".

Both reuse the matching predicate described in §3.2.

### 3.5 Global notifier state (lines 188–212)
```rust
static NOTIFIER: Lazy<Arc<Mutex<Box<dyn Notifier>>>> =
    Lazy::new(|| Arc::new(Mutex::new(Box::new(QmkNotifier) as Box<dyn Notifier>)));
fn get_notifier() -> Arc<Mutex<Box<dyn Notifier>>> { Arc::clone(&NOTIFIER) }
```
`once_cell::sync::Lazy` (crate dep `once_cell = "1.21"`). Default notifier is
`QmkNotifier`; swapped via `set_notifier` in tests.

### 3.6 Debounce pipeline — state, worker, algorithm

**State struct** (lines 193–202):
```rust
struct DebounceState {
    last_sent_time: Option<Instant>,  // None until first send
    pending: Option<String>,          // newest queued message
    verbose: bool,
    interval: Duration,               // 0 disables coalescing
}
```
**Statics** (lines 204–212):
```rust
static STATE: Lazy<Mutex<DebounceState>> = Lazy::new(|| Mutex::new(DebounceState {
    last_sent_time: None, pending: None, verbose: false,
    interval: Duration::from_millis(crate::core::configured_debounce_ms()),
}));
static COND: Lazy<Condvar> = Lazy::new(Condvar::new);
```

**Worker thread** (lines 223–284), `debounce_worker()`:
- Runs an infinite loop. Inside one iteration:
  1. Locks `STATE`; **waits on `COND`** while `pending.is_none()` (no work).
  2. Computes `target = last_sent_time.unwrap_or_else(Instant::now) + interval`.
  3. If `now >= target`: take the pending message, record `last_sent_time = now`,
     release the lock, send. Otherwise `COND.wait_timeout(state, target - now)`
     and re-loop.
- Spawns lazily: `static WORKER: Lazy<thread::JoinHandle<()>> = Lazy::new(||
  thread::spawn(debounce_worker));` (line 286). `ensure_worker()` (289–293)
  force-initializes `WORKER` on first `notify_qmk` call.
- **Algorithm invariant**: because new pending messages do **not** reset
  `last_sent_time`, a rapid burst collapses to exactly one follow-up send of the
  newest message. The window is measured from the last *sent* notification, not
  the last *queued* one.
- `interval == 0` (debounce disabled): `target == last_sent_time` so the worker
  flushes immediately on each wake — every change sends.

**`notify_qmk()`** (lines 305–364) — public entry point:
```rust
pub fn notify_qmk(window_info: &WindowInfo, verbose: bool)
    -> Result<(), Box<dyn Error + Send + Sync>>
```
- Builds the message: `format!("{}{}{}", window_info.app_class, "\x1D",
  window_info.title)` — `\x1D` (ASCII Group Separator) is the class/title
  delimiter, sanitized to `|` for verbose console output.
- Decision (under `STATE` lock):
  - `due = last_sent_time.map(|t| now.duration_since(t) >= interval).unwrap_or(true)`.
  - If `due`: set `last_sent_time = now`, clear `pending`, send **immediately**
    (synchronously, holding the notifier lock but **not** the state lock).
  - Else: set `pending = Some(message)`, `COND.notify_one()`, return `Ok`.
- Immediate-send path also times the send (`_send_ms`) for verbose `eprintln!`.

### 3.7 `QmkNotifier::notify()` — the real send (lines 142–186)
- Builds `qmk_notifier::RunParameters::new(RunCommand::SendMessage(msg), vid,
  pid, usage_page, usage, verbose=false)`, calls `qmk_notifier::run(params)`.
- **Retry with backoff**: up to 3 attempts. On an error whose lowercase string
  contains `"no device found"` / `"permission denied"` / `"failed to open"`,
  sleeps `100 * attempt` ms and retries; on the 3rd failure it **logs and returns
  `Ok(())`** so the service isn't restarted by a transient device issue. Non-
  device errors fail immediately.
- **Note (Gap G7)**: `verbose` is hard-coded to `false` in the `RunParameters`
  here, so the verbose knob is lost for `qmk_notifier`'s own diagnostic output.
  This is intentional (the notifier owns its own verbose logging) but worth
  flagging — `notify_qmk`'s `verbose` only governs the local `[<ms>ms]` prints.

### 3.8 `set_notifier()` — test seam (lines 295–303)
```rust
#[cfg(test)]
pub fn set_notifier(notifier: Box<dyn Notifier>)
```
Force-inits `NOTIFIER`, then swaps the boxed notifier under the lock. **Test-
only** — production cannot replace the notifier. The test suite installs a
`MockNotifier` that counts calls and records the last message in module-level
`AtomicUsize`/`StdMutex<Option<String>>` statics (lines 313–460).

### 3.9 Test helpers & suites (lines 305–604)
- `reset_test_state()` (lines 384–405): drains in-flight work (150 ms sleep),
  resets `last_sent_time`/`pending`/`verbose`/`interval = 50ms`, wakes the worker,
  resets the global mock.
- Suites: immediate first send; subsequent-message debounce (widened to 200 ms to
  dodge CI scheduler jitter — see test comments); send-after-timeout; multi-rapid-
  update collapse to newest; verbose mode; threads-don't-interfere (asserts
  `count >= 1`, tolerates 1–2 sends from a near-simultaneous burst).
- Tests force `--test-threads=1` (per AGENTS.md) because `STATE` is shared global
  state across all tests in the binary.

---

## 4. `src/core/types.rs` — `WindowInfo`

```rust
#[derive(Debug, PartialEq)]
pub struct WindowInfo {
    pub app_class: String,
    pub title: String,
}
impl WindowInfo {
    pub fn new(app_class: String, title: String) -> Self { Self { app_class, title } }
}
```
Plain value type. `PartialEq` enables dedup/comparison by platform monitors
(only equal windows should re-trigger a notification). **Not `Clone` or `Eq`**
(Gap G8, low severity — `Eq` is trivially derivable since `String: Eq`, and
`Clone` would be convenient for callers that move the value). Two unit tests
(creation + equality). No `Default`.

---

## 5. External dependency: `qmk_notifier` v0.2.1 (consumed by notifier.rs)

`Cargo.toml` (lines 16–19) pins:
```toml
qmk_notifier = { package = "qmk_notifier", git = "https://github.com/dabstractor/qmk_notifier", tag = "v0.2.1" }
hidapi = "2.6"
once_cell = "1.21"
```
`Cargo.lock` resolves v0.2.1 to commit `32986053…` (`~/.cargo/git/checkouts/
qmk_notifier-a54e3247c1b61fcf/3298605/`). Public API surface used by QMKonnect:

- Constants (`src/core.rs`):
  ```rust
  pub const DEFAULT_VENDOR_ID: u16 = 0xFEED;
  pub const DEFAULT_PRODUCT_ID: u16 = 0x0000;
  pub const DEFAULT_USAGE_PAGE: u16 = 0xFF60;   // used by notifier::configured_filter
  pub const DEFAULT_USAGE: u16 = 0x61;          // used by notifier::configured_filter
  pub const REPORT_LENGTH: usize = 32;
  ```
- `pub enum RunCommand { SendMessage(String), ListDevices }`
- `pub struct RunParameters { command, vendor_id: Option<u16>, product_id:
  Option<u16>, usage_page: u16, usage: u16, verbose: bool }` with
  `RunParameters::new(...)`.
- `pub fn run(params: RunParameters) -> Result<(), QmkError>` — appends an `0x03`
  ETX terminator to the message, then `send_raw_report` bursts it across up to N
  32-byte raw-HID reports (`[0x00, 0x81, 0x9F, <30 payload>…]`). Multi-device:
  opens *every* matching interface and bursts to all (PartialSendError on a mixed
  outcome). Caches device handles in a global `Mutex<Option<DeviceCache>>`
  (`LazyLock`), invalidating on any write failure and rebuilding on next call;
  retries once on total failure (`SEND_RETRIES = 1`). Drain of IN-side acks is
  bounded (`IN_DRAIN_MAX = 32`) and currently a no-op (firmware ack is dropped).

This is where the real keyboard I/O lives; `QmkNotifier` is a thin retry/recovery
wrapper around `qmk_notifier::run`.

---

## 6. How the pieces connect (architecture)

```
                 ┌──────────────────── src/main.rs ────────────────────┐
   argv ───────► │ run() : manual flag scan                            │
                 │   -c → create_config → core::create_default_config  │
                 │   -r → reload_config → core::parse_config           │
                 │        (Linux: platforms::update/reload_udev_rules) │
                 │   --list-devices → notifier::list_devices           │
                 │   default → runners::create_runner(verbose).run()   │
                 └───────────────────────┬─────────────────────────────┘
                                         │ platform runner
                                         ▼
              platform WindowMonitor (mac/win/x11/hyprland)
                  calls on every active-window change
                                         │
                                         ▼
              notifier::notify_qmk(&WindowInfo, verbose)
                                         │
                 ┌───────────────────────┴────────────────────────────┐
                 ▼                                                    ▼
       immediate send (due)                              queue + COND.notify_one
       notifier.lock().notify(msg)                       debounce_worker flushes
                                                            │
                                                            ▼
                                              QmkNotifier::notify(msg)
                                                retry/backoff ×3 →
                                              qmk_notifier::run(RunParameters)
                                                → cached HID burst-write
```
- **Config flow**: every read (parse, filter, timing) re-walks
  `platforms::get_config_paths()` and re-parses the file — no in-memory cache,
  enabling live config edits. Exception: the debounce `interval` is captured once
  into `STATE` at first access and is **not** refreshed thereafter (Gap G5).
- **Device-filter flow**: `configured_filter()` → `DeviceFilter` → consumed by
  both probes (`startup_device_probe`, `is_device_connected`) and by
  `QmkNotifier::notify` (→ `qmk_notifier::RunParameters`).
- **Test seam**: `set_notifier(Box<dyn Notifier>)` swaps the global; tests run
  single-threaded because `STATE`/`COND`/`WORKER` are process-global.

---

## 7. Key constants & values (consolidated)

| Constant | Value | Location |
|----------|-------|----------|
| `DEFAULT_DEBOUNCE_MS` | `50` | `core/mod.rs:41` |
| `DEFAULT_POLL_INTERVAL_MS` | `0` | `core/mod.rs:42` |
| Class/title delimiter | `\x1D` (ASCII GS, sanitized to `\|` for logs) | `notifier.rs:309` |
| QMK `DEFAULT_USAGE_PAGE` | `0xFF60` | `qmk_notifier core.rs` |
| QMK `DEFAULT_USAGE` | `0x61` | `qmk_notifier core.rs` |
| `DEFAULT_VENDOR_ID` | `0xFEED` | `qmk_notifier core.rs` (legacy, not used by default) |
| `DEFAULT_PRODUCT_ID` | `0x0000` | `qmk_notifier core.rs` (legacy, not used by default) |
| `REPORT_LENGTH` | `32` | `qmk_notifier core.rs` |
| QMK retry attempts | `3` | `notifier.rs:145` |
| QMK retry backoff | `100 * attempt` ms | `notifier.rs:164` |

---

## 8. Code-quality findings & gaps

| ID | Severity | Location | Finding |
|----|----------|----------|---------|
| G1 | Medium | `main.rs:48–52` | Non-Windows `init_logging` is a no-op — `log` macros emit nothing on macOS/Linux unless a platform runner installs a logger. Verbose `println!`/`eprintln!` still works, but `log::*` calls are silently dropped. |
| G2 | Low | `main.rs:251–260` | `update_udev_rules`/`reload_udev_rules` failures only print a warning when `verbose`; otherwise swallowed. A failed reload reports "Configuration reloaded successfully." |
| G3 | Low | `core/mod.rs:80–92` | No config validation/normalization. `debounce_ms` / `poll_interval_ms` accept any `u64`; `0` is meaningful but unbounded high values aren't flagged. |
| G4 | Low | `core/mod.rs:15` | `#[derive(Default)]` on `Config` yields `debounce_ms=0`/`poll_interval_ms=0` (the serde default fns aren't invoked by `Default`), so `Config::default()` ≠ a parsed empty file (`50`/`0`). `Default` appears unused at runtime — latent foot-gun. |
| G5 | Medium | `notifier.rs:204–211` | `DebounceState.interval` is read from `configured_debounce_ms()` **once** at static init and never refreshed. Config edits to `debounce_ms` do not take effect without a restart, despite the per-call config reads elsewhere advertising "live config." |
| G6 | Medium | `notifier.rs:77–125`, `127–140` | The VID/PID/usage matching predicate is inlined identically in `startup_device_probe` and `is_device_connected`, and a third copy lives in `qmk_notifier::device_matches`. Extract a single `DeviceFilter::matches(&hidapi::DeviceInfo)` to guarantee consistency. |
| G7 | Low | `notifier.rs:152–159` | `QmkNotifier::notify` hard-codes `verbose=false` into `RunParameters`, so `qmk_notifier`'s own diagnostics never surface even under `-v`. Intentional but undocumented from this layer. |
| G8 | Low | `types.rs:7` | `WindowInfo` derives `Debug, PartialEq` only. `Eq` is trivially sound and `Clone`/`Default` would help platform callers that move or reset the value. |
| G9 | Low | `notifier.rs:188`, `204`, `212`, `286` | All global statics (`NOTIFIER`, `STATE`, `COND`, `WORKER`) use `once_cell::sync::Lazy`; std `std::sync::LazyLock` (stable since 1.80) could drop the `once_cell` dep — `qmk_notifier` already uses `LazyLock`. |
| G10 | Low | `notifier.rs:1–2` comment | Stale comment: "Debounce interval now lives in DebounceState (loaded from config); see core::configured_debounce_ms()." is accurate but the per-call refresh (G5) doesn't actually happen. |
| G11 | Info | `notifier.rs:145–186` | Returning `Ok(())` after exhausting retries deliberately hides device errors from the service supervisor (prevents restart storms). Reasonable, but means a persistent device fault is invisible except via `eprintln!` — pair with a tray status indicator (which `is_device_connected` provides). |
| G12 | Info | `main.rs:70–128` | CLI parsing is hand-rolled argv scanning (clap is only in the `qmk_notifier` lib). Fine for the small flag set but means no `--help` grouping/validation; `parse_value_flag` re-implements what clap provides. |

No correctness blockers were found in the four core files. The debouncer
algorithm is well-specified and tested (burst-collapse invariant is asserted in
`test_multiple_rapid_updates`), the config round-trip is tested, and the device
probes are read-only by construction (they never call `open_device`).

---

## 9. Start here

Open **`src/core/notifier.rs`** first — it is the heart of the core layer
(notifier trait, the entire debounce state machine + worker thread, the device
filter/probes, and the retry-wrapped real send). Then `src/core/mod.rs` for the
config model and the timing helpers that feed the debouncer. `src/main.rs` and
`src/core/types.rs` are straightforward and can be skimmed.

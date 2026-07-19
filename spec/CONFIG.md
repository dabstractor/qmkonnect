# SPEC — Configuration & CLI

> Companion to `PRD.md`. The complete TOML schema, defaults, the shared
> config-body renderer, per-OS config paths, and the full CLI flag reference.
> Covers `src/core/mod.rs` and `src/main.rs`.

---

## 1. Config Schema (`src/core/mod.rs`)

```rust
#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct Config {
    #[serde(default)] pub vendor_id:      Option<u16>,  // None = match any (auto)
    #[serde(default)] pub product_id:     Option<u16>,  // None = match any (auto)
    #[serde(default)] pub usage_page:     Option<u16>,  // default 0xFF60 at use site
    #[serde(default)] pub usage:          Option<u16>,  // default 0x61 at use site
    #[serde(default = "default_debounce_ms")]      pub debounce_ms: u64,      // 50
    #[serde(default = "default_poll_interval_ms")] pub poll_interval_ms: u64, // 0
}
```

**Every device-identifying field is `Option` with `#[serde(default)]`**, so:
- A **missing** field deserializes to `None` ⇒ "match any" / default-at-use-site.
- An **empty** (`""`) / brand-new config file is valid (all `None`).
- **Legacy** files that set `vendor_id = 0xfeed` keep working (become `Some(0xfeed)`).

`#[derive(Default)]` gives `Config::default()` = all `None` + timing defaults.

### 1.1 Field reference

| Key | Type | Default | Meaning |
|---|---|---|---|
| `vendor_id` | `Option<u16>` (hex in TOML) | `None` | USB vendor ID. Set only to disambiguate multiple QMK keyboards. |
| `product_id` | `Option<u16>` (hex in TOML) | `None` | USB product ID. Set only to disambiguate. |
| `usage_page` | `Option<u16>` | `0xff60` (resolved at use site) | HID usage page. Set only if firmware overrode `RAW_USAGE_PAGE`. |
| `usage` | `Option<u16>` | `0x61` (resolved at use site) | HID usage. Set only if firmware overrode `RAW_USAGE_ID`. |
| `debounce_ms` | `u64` | `50` | Burst-coalescing window (ms). `0` disables debouncing (every change sends immediately). |
| `poll_interval_ms` | `u64` | `0` | (Hyprland only) periodic active-window poll cadence (ms). `0` = rely on IPC events. |

> TOML hex literals (`0xfeed`) are supported by `toml`'s integer parsing and
> deserialize into `u16` directly.

### 1.2 Resolution at use sites
- `configured_filter()` (`core/notifier.rs`) resolves `usage_page`/`usage` to
  `qmk_notifier::DEFAULT_USAGE_PAGE`/`DEFAULT_USAGE` when `None`. VID/PID stay
  `Option` (auto-discovery).
- `configured_timing()` resolves `debounce_ms`/`poll_interval_ms` to defaults
  when the file is missing or the field is unset.

**Hot config:** both are re-read from disk on **every** notification and every
status poll, so editing the file (or saving the Settings dialog) takes effect
within ~3 s — no restart.

---

## 2. The Shared Config-Body Renderer (`render_config_body`)

```rust
pub fn render_config_body(vendor_id: Option<u16>, product_id: Option<u16>) -> String
```

The **single** function every write path uses (CLI `create_default_config`, the
Win32 dialog, the NSAlert dialog, the zenity/GTK Linux dialog). Guarantees the
file format never drifts. Output:

```toml
# QMKonnect Configuration
#
# All fields are OPTIONAL. By default QMKonnect auto-discovers any QMK
# keyboard using the standard Raw HID usage page (0xFF60 / 0x61). Set
# vendor_id/product_id only to disambiguate among multiple QMK
# keyboards, or usage_page/usage to target a board that overrode
# RAW_USAGE_PAGE/RAW_USAGE_ID in its firmware.
#
# usage_page = 0xff60
# usage      = 0x61
#
# Debounce window (ms) for coalescing rapid window-change bursts before
# sending to the keyboard. 0 disables debouncing entirely. Default 50.
# debounce_ms = 50
#
# (Hyprland only) periodic active-window poll interval (ms).
# 0 disables. Default 0.
# poll_interval_ms = 0

vendor_id  = 0xfeed          # OR:  "# vendor_id  = 0xfeed   # unset: auto-discovery"
product_id = 0x0000          # OR:  "# product_id = 0x0000   # unset: auto-discovery"
```

A value is **explicit** when `Some` (uncommented), **commented-out** when `None`
("auto-discovery"). Timing/usage options are always shown as commented hints.

---

## 3. Config Paths (per-OS)

| OS | Primary | Secondary | System-wide |
|---|---|---|---|
| **Linux** | `$XDG_CONFIG_HOME/qmk-notifier/config.toml` | `~/.config/qmk-notifier/config.toml` | `/etc/qmk-notifier/config.toml` |
| **Windows** | `%APPDATA%\QMKonnect\config.toml` | `%LOCALAPPDATA%\QMKonnect\config.toml` | (exe dir fallback) |
| **macOS** | `~/Library/Application Support/QMKonnect/config.toml` | `~/.config/qmk-notifier/config.toml` (XDG) | `/etc/qmk-notifier/config.toml` |

> Linux preserves the historical `qmk-notifier/` dir name so existing installs
> keep working. Windows/macOS use `QMKonnect/`. The macOS XDG + `/etc` fallbacks
> exist so a config written on one platform can be found on another.

`create_config_dir()` returns the primary dir (creating it). The Settings
dialogs call it before writing, so the directory always exists.

> **Host-side `rules.toml`:** lives in the **same
directory** as `config.toml` (e.g. `~/.config/qmk-notifier/rules.toml`,
`%APPDATA%\QMKonnect\rules.toml`,
`~/Library/Application Support/QMKonnect/rules.toml`). Absent ⇒ host rules
disabled (string-only). Schema: `HOST_RULES.md` §9.

---

## 4. CLI Reference (`src/main.rs`)

```
qmkonnect [OPTIONS]

Options:
  -h, --help            Display this help message
  -v, --verbose         Enable verbose logging
  -c, --config          Create a default (commented-out) configuration file
  -r, --reload          Reload configuration and update system files (Linux)
      --config <path>   Config file to use with --reload
      --user <name>     Invoking user for sudo'd --reload (Linux)
      --uid <n>         Invoking uid for sudo'd --reload (Linux)
  -l, --list            List supported platforms (this build)
      --list-devices    List connected HID devices (VID/PID discovery)
      --show-window-info  [macOS/Windows] open the Window Information dialog directly
      --tray-app          [Windows] run as tray app (default)
      --console           [Windows] allocate a console and run for debugging
      --list-callbacks      handshake → print the keyboard's callback name→id table
      --validate-rules      parse rules.toml; report schema/callback-name errors
      --rules-path <path>   override the rules.toml location
```

Running with **no options** starts the notifier service (the tray app on
Windows/macOS, the monitor on Linux).

### 4.1 Flag semantics
- `-h/--help`, `-v/--verbose` — boolean flags scanned from `env::args()`.
- `-c/--config` — `create_config()`: `create_config_dir()` then
  `create_default_config(<dir>/config.toml)` (no-op + message if it exists).
- `-r/--reload` — `reload_config(verbose, config, user, uid)`:
  - Resolves the config path (Linux: **root-aware** `resolve_config_for_reload`,
    §`SPEC_LINUX.md` 4.4; else `get_config_path()`).
  - `parse_config`, read VID/PID as `Option<u16>`.
  - Linux: `update_udev_rules(vid, pid, verbose)` (writes/repairs/purges the
    fallback rule — no-ops cleanly when both unset) + `reload_udev_rules()`.
  - Prints "Configuration reloaded successfully."
- `--config`/`--user`/`--uid` — value flags parsed by `parse_value_flag`
  (accepts `--flag value` and `--flag=value`). Only meaningful with `--reload`
  on Linux.
- `--list-devices` — `core::notifier::list_devices()` (read-only HID enumerate;
  prints `vid:pid  page:usage  product`).
- `--show-window-info` — `platforms::list_foreground_windows()` then the
  platform dialog (macOS/Windows only). A debug aid to test the dialog in
  isolation.
- `--tray-app`/`--console` — Windows-only runner modes.

### 4.2 Logging
- **Windows:** `init_logging()` tries `eventlog::init("QMKonnect", Info)`
  (Windows Event Log, source `"QMKonnect"`) first, falls back to `env_logger`
  (console). `-v` from a terminal prints there.
- **macOS/Linux:** `eprintln!`/`println!` (verbose timestamps via
  `core::now_ms()`, a process-local monotonic epoch — not wall-clock).

---

## 5. Parsing Robustness
- `parse_config` is a straight `toml::from_str` — no normalization/validation
  beyond what TOML + serde provide. Unknown keys are ignored by serde default
  (forward-compatible).
- Hex field parsing in the UI (`parse_id_field`/`parse_id`): trims, accepts
  optional `0x`/`0X`, empty or literal `"auto"` ⇒ `None`, else
  `u16::from_str_radix(_, 16)`. Garbage ⇒ `Err` surfaced in the dialog.

---

## 6. Config UX per Platform (summary; full detail `SPEC_UI.md` §2)
- **Windows/macOS:** tray → Settings → native dialog → writes `config.toml`
  via `render_config_body`.
- **Linux:** tray → Settings… → `zenity --forms` → `write_config` +
  `apply_device_rule` (pkexec install). Or edit the file + `sudo qmkonnect -r`.

---

*Continue with `SPEC_PACKAGING.md`.*

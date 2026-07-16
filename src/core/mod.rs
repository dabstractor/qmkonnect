pub mod notifier;
pub mod types;

use std::error::Error;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;

// Define the Config struct. VID/PID (and usage page/usage) are OPTIONAL: a
// missing field deserializes to `None`, which means "match any" (auto-discovery
// by the standard QMK raw-HID usage page/usage). Existing config files that set
// `vendor_id = 0xfeed` keep working (they become `Some(0xfeed)`).
#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct Config {
    /// USB vendor ID. `None` = match any (auto-discovery by usage page/usage).
    #[serde(default)]
    pub vendor_id: Option<u16>,
    /// USB product ID. `None` = match any (auto-discovery by usage page/usage).
    #[serde(default)]
    pub product_id: Option<u16>,
    /// HID usage page. `None` = QMK raw-HID default (0xFF60). Set this only to
    /// target a board that overrode `RAW_USAGE_PAGE` in its firmware.
    #[serde(default)]
    pub usage_page: Option<u16>,
    /// HID usage. `None` = QMK raw-HID default (0x61). Set this only to target
    /// a board that overrode `RAW_USAGE_ID` in its firmware.
    #[serde(default)]
    pub usage: Option<u16>,
    /// Debounce window (ms) for coalescing rapid window-change bursts before
    /// sending to the keyboard. 0 disables debouncing (every change sends
    /// immediately). Defaults to 50.
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,
    /// (Hyprland only) periodic active-window poll interval (ms). 0 disables
    /// polling (events come from the IPC listener instead). Defaults to 0.
    #[serde(default = "default_poll_interval_ms")]
    pub poll_interval_ms: u64,
}

const DEFAULT_DEBOUNCE_MS: u64 = 50;
const DEFAULT_POLL_INTERVAL_MS: u64 = 0;

fn default_debounce_ms() -> u64 {
    DEFAULT_DEBOUNCE_MS
}

fn default_poll_interval_ms() -> u64 {
    DEFAULT_POLL_INTERVAL_MS
}

/// Monotonic milliseconds since the first call. Used only for verbose log
/// timestamps, so a process-local epoch (not wall-clock) is correct and avoids
/// any system-clock skew.
pub fn now_ms() -> u128 {
    static START: OnceLock<Instant> = OnceLock::new();
    let start = START.get_or_init(Instant::now);
    start.elapsed().as_millis()
}

/// Read the debounce window (ms) from the user's config, falling back to the
/// default when unset or when no config exists.
pub fn configured_debounce_ms() -> u64 {
    configured_timing().0
}

/// Read both timing knobs (debounce ms, poll interval ms) from the user's
/// config, falling back to defaults when unset or when no config exists. Both
/// default when the file is missing, so a fresh zero-config install behaves
/// correctly.
pub fn configured_timing() -> (u64, u64) {
    crate::platforms::get_config_paths()
        .into_iter()
        .find(|p| p.exists())
        .and_then(|p| parse_config(&p).ok())
        .map(|cfg| (cfg.debounce_ms, cfg.poll_interval_ms))
        .unwrap_or((DEFAULT_DEBOUNCE_MS, DEFAULT_POLL_INTERVAL_MS))
}

pub fn parse_config(config_path: &Path) -> Result<Config, Box<dyn Error>> {
    let config_str = fs::read_to_string(config_path)?;
    let config: Config = toml::from_str(&config_str)?;

    // No need to normalize or validate - TOML parser handles it

    Ok(config)
}

/// Render a `config.toml` body for VID/PID values. A line is explicit when the
/// value is `Some`, or commented out ("auto-discovery") when `None`. Timing and
/// usage/page options are always shown as commented hints. Used by
/// `create_default_config` and every platform's settings-dialog write path so
/// all of them agree on the file format.
pub fn render_config_body(vendor_id: Option<u16>, product_id: Option<u16>) -> String {
    let vid_line = match vendor_id {
        Some(v) => format!("vendor_id  = 0x{v:04x}"),
        None => "# vendor_id  = 0xfeed   # unset: auto-discovery".to_string(),
    };
    let pid_line = match product_id {
        Some(p) => format!("product_id = 0x{p:04x}"),
        None => "# product_id = 0x0000   # unset: auto-discovery".to_string(),
    };
    format!(
        "# QMKonnect Configuration\n\
         #\n\
         # All fields are OPTIONAL. By default QMKonnect auto-discovers any QMK\n\
         # keyboard using the standard Raw HID usage page (0xFF60 / 0x61). Set\n\
         # vendor_id/product_id only to disambiguate among multiple QMK\n\
         # keyboards, or usage_page/usage to target a board that overrode\n\
         # RAW_USAGE_PAGE/RAW_USAGE_ID in its firmware.\n\
         #\n\
         # usage_page = 0xff60\n\
         # usage      = 0x61\n\
         #\n\
         # Debounce window (ms) for coalescing rapid window-change bursts before\n\
         # sending to the keyboard. 0 disables debouncing entirely. Default 50.\n\
         # debounce_ms = 50\n\
         #\n\
         # (Hyprland only) periodic active-window poll interval (ms).\n\
         # 0 disables. Default 0.\n\
         # poll_interval_ms = 0\n\
         \n\
         {vid_line}\n\
         {pid_line}\n"
    )
}

/// Create a default (zero-config) config file. Every device-identifying field
/// is written commented out, so out-of-the-box QMKonnect auto-discovers any QMK
/// keyboard by usage page/usage — no VID/PID, no `--reload`, no sudo needed
/// (the static udev rule grants permissions for any 0xFF60/0x61 device).
pub fn create_default_config(config_path: &Path) -> Result<(), Box<dyn Error>> {
    if config_path.exists() {
        println!("Configuration already exists at: {}", config_path.display());
        return Ok(());
    }

    // Make sure the directory exists
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Zero-config default: every device-identifying field commented out.
    let default_config = render_config_body(None, None);

    // Write the config file
    fs::write(config_path, default_config)?;

    println!(
        "Configuration created successfully at: {}",
        config_path.display()
    );
    println!(
        "By default QMKonnect auto-discovers any QMK keyboard (usage page\n\
         0xFF60 / usage 0x61). Set vendor_id/product_id only to disambiguate\n\
         among multiple QMK keyboards, then run `qmkonnect -r` to install the\n\
         matching udev rule (Linux only)."
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_config_is_all_none() {
        // A brand-new / empty config is valid and means "auto-discover any QMK
        // keyboard by usage page/usage".
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg.vendor_id, None);
        assert_eq!(cfg.product_id, None);
        assert_eq!(cfg.usage_page, None);
        assert_eq!(cfg.usage, None);
        // Timing fields still take their serde defaults when absent.
        assert_eq!(cfg.debounce_ms, DEFAULT_DEBOUNCE_MS);
        assert_eq!(cfg.poll_interval_ms, DEFAULT_POLL_INTERVAL_MS);
    }

    #[test]
    fn legacy_config_with_explicit_ids_parses_to_some() {
        // Existing config files in the wild set vendor_id/product_id as plain
        // u16 hex. They must keep working (now Some(...)).
        let cfg: Config =
            toml::from_str("vendor_id = 0xfeed\nproduct_id = 0x0000\ndebounce_ms = 100\n").unwrap();
        assert_eq!(cfg.vendor_id, Some(0xfeed));
        assert_eq!(cfg.product_id, Some(0x0000));
        assert_eq!(cfg.debounce_ms, 100);
    }

    #[test]
    fn partial_config_only_usage_page() {
        let cfg: Config = toml::from_str("usage_page = 0xff60\n").unwrap();
        assert_eq!(cfg.usage_page, Some(0xff60));
        assert_eq!(cfg.vendor_id, None);
        assert_eq!(cfg.product_id, None);
    }

    #[test]
    fn render_config_body_round_trips() {
        // Rendering (None, None) must parse back to all-None device IDs.
        let body = render_config_body(None, None);
        let cfg: Config = toml::from_str(&body).unwrap();
        assert_eq!(cfg.vendor_id, None);
        assert_eq!(cfg.product_id, None);

        // Rendering explicit values must parse back to Some.
        let body = render_config_body(Some(0xfeed), Some(0x1234));
        let cfg: Config = toml::from_str(&body).unwrap();
        assert_eq!(cfg.vendor_id, Some(0xfeed));
        assert_eq!(cfg.product_id, Some(0x1234));
    }
}

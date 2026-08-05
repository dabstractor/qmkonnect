pub mod notifier;
pub mod pattern;
pub mod rules;
pub mod types;

use once_cell::sync::Lazy;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::atomic::AtomicU64;
#[cfg(test)]
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Instant;
use std::time::SystemTime;

// Define the Config struct. VID/PID (and usage page/usage) are OPTIONAL: a
// missing field deserializes to `None`, which means "match any" (auto-discovery
// by the standard QMK raw-HID usage page/usage). Existing config files that set
// `vendor_id = 0xfeed` keep working (they become `Some(0xfeed)`).
#[derive(serde::Deserialize, serde::Serialize, Clone)]
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

impl Default for Config {
    /// The canonical default config: auto-discovery (every device-identifying
    /// field `None`) and the runtime timing defaults (`debounce_ms = 50`,
    /// `poll_interval_ms = 0`). This MUST agree with the serde
    /// `default = ...` attributes above so that `Config::default()`, an empty
    /// `config.toml`, and `configured_timing()` all describe the SAME
    /// zero-config state. Otherwise a Settings-dialog save that falls back to
    /// `Config::default()` (no existing config) would write a different — and
    /// surprising — value for a timing field (e.g. `debounce_ms = 0`, silently
    /// disabling debouncing). A manual impl is used instead of `#[derive(Default)]`
    /// because the derive would zero-init `debounce_ms`, not match the serde
    /// default.
    fn default() -> Self {
        Self {
            vendor_id: None,
            product_id: None,
            usage_page: None,
            usage: None,
            debounce_ms: DEFAULT_DEBOUNCE_MS,
            poll_interval_ms: DEFAULT_POLL_INTERVAL_MS,
        }
    }
}

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

/// mtime+size-keyed cache for the hot-config `config.toml` read. Re-stats every
/// call (cheap `stat()`); re-reads+re-parses only when the resolved file's
/// (path, mtime, size) change. Preserves hot-config: editing `config.toml`
/// invalidates on the NEXT call (~instant — no TTL delay). Keyed on path too,
/// so a relocated candidate never serves a stale entry from a different file.
/// Shared by `configured_timing` + `configured_filter` so the per-send
/// double-read is coalesced to one parse per mtime.
static CONFIG_CACHE: Lazy<ConfigCache<Config>> = Lazy::new(|| Mutex::new(None));

/// mtime+size-keyed cache for the hot-config `rules.toml` read
/// (`host_context_for_window`). Same contract as [`CONFIG_CACHE`].
/// [`crate::core::rules::parse_rules`] stays uncached for its other callers
/// (`validate_rules_callback_names`, `--validate-rules`, tests).
static RULES_CACHE: Lazy<RulesCache> = Lazy::new(|| Mutex::new(None));

// Test-only observables: incremented ONLY on a cache miss (the fall-through to
// parse_config/parse_rules). Tests snapshot the delta to prove HIT/MISS —
// ns-mtime Linux can't fake a hit via mtime/size control, so the counter is the
// only rigorous, platform-independent observable.
#[cfg(test)]
static CONFIG_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static RULES_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);

/// (resolved path, mtime, size) hot-config cache key + parsed value. Factored
/// out to keep the `Lazy<Mutex<Option<…>>>` static types under clippy's
/// `type_complexity` threshold (one `Mutex<Option<(…,T)>>` tuple per cache).
type ConfigCache<T> = Mutex<Option<(PathBuf, SystemTime, u64, T)>>;
/// [`CONFIG_CACHE`] for the `rules.toml` value type.
type RulesCache = ConfigCache<crate::core::rules::RuleSet>;

/// Hermetic, testable core: cache `config.toml` at `path` by
/// (path, mtime, size). On a cache HIT returns the stored [`Config`] clone (no
/// disk read, no parse). On a MISS calls [`parse_config`] and stores the
/// result. Parse ERRORS ARE NOT CACHED (a later valid edit must re-read).
/// Mirror of the `parse_config` / `config_parse_error_at` `_at` convention.
///
/// Poison recovery uses the `unwrap_or_else(|e| e.into_inner())` idiom
/// (P1.M1.T1.S1) — a panic in one caller must not poison the cache for the
/// whole process.
pub fn cached_config_at(path: &Path) -> Result<Config, Box<dyn Error>> {
    let meta = path.metadata()?;
    let mtime = meta.modified()?;
    let size = meta.len();
    {
        let cache = CONFIG_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((cp, cm, cs, cfg)) = cache.as_ref() {
            if cp == path && *cm == mtime && *cs == size {
                return Ok(cfg.clone());
            }
        }
    }
    #[cfg(test)]
    CONFIG_CACHE_MISSES.fetch_add(1, Ordering::SeqCst);
    let cfg = parse_config(path)?; // errors NOT cached (re-read on next call)
    *CONFIG_CACHE.lock().unwrap_or_else(|e| e.into_inner()) =
        Some((path.to_path_buf(), mtime, size, cfg.clone()));
    Ok(cfg)
}

/// Hermetic, testable core for `rules.toml` — identical shape to
/// [`cached_config_at`], wrapping [`crate::core::rules::parse_rules`].
pub fn cached_rules_at(path: &Path) -> Result<crate::core::rules::RuleSet, Box<dyn Error>> {
    let meta = path.metadata()?;
    let mtime = meta.modified()?;
    let size = meta.len();
    {
        let cache = RULES_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((cp, cm, cs, rs)) = cache.as_ref() {
            if cp == path && *cm == mtime && *cs == size {
                return Ok(rs.clone());
            }
        }
    }
    #[cfg(test)]
    RULES_CACHE_MISSES.fetch_add(1, Ordering::SeqCst);
    let rs = crate::core::rules::parse_rules(path)?; // errors NOT cached
    *RULES_CACHE.lock().unwrap_or_else(|e| e.into_inner()) =
        Some((path.to_path_buf(), mtime, size, rs.clone()));
    Ok(rs)
}

/// Resolve the first existing `config.toml` candidate and return it via the
/// mtime cache. When NO candidate exists, returns [`Config::default`] WITHOUT
/// caching (cheap; and avoids serving a stale default after the user later
/// creates the file). Errors from [`cached_config_at`] propagate (callers
/// swallow via `.ok()` — same as today's `parse_config` usage).
pub fn cached_config() -> Result<Config, Box<dyn Error>> {
    match crate::platforms::get_config_paths()
        .into_iter()
        .find(|p| p.exists())
    {
        Some(p) => cached_config_at(&p),
        None => Ok(Config::default()),
    }
}

/// Read both timing knobs (debounce ms, poll interval ms) from the user's
/// config, falling back to defaults when unset or when no config exists. Both
/// default when the file is missing, so a fresh zero-config install behaves
/// correctly. Reads via [`cached_config`] (mtime-keyed): an edit invalidates
/// on the next call (~instant, no TTL) so hot-config is preserved.
pub fn configured_timing() -> (u64, u64) {
    cached_config()
        .ok()
        .map(|cfg| (cfg.debounce_ms, cfg.poll_interval_ms))
        .unwrap_or((DEFAULT_DEBOUNCE_MS, DEFAULT_POLL_INTERVAL_MS))
}

pub fn parse_config(config_path: &Path) -> Result<Config, Box<dyn Error>> {
    let config_str = fs::read_to_string(config_path)?;
    let config: Config = toml::from_str(&config_str)?;

    // No need to normalize or validate - TOML parser handles it

    Ok(config)
}

/// Render a fully-commented zero-config `config.toml` template. Every field
/// is shown as a commented-out hint, so a freshly-seeded file parses to
/// all-default (auto-discovery, default debounce/poll) and behaves identically
/// to having no config at all. Used by [`create_default_config`] (the `-c`
/// seeder / first-run seed).
pub fn render_default_config_template() -> String {
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
     # vendor_id  = 0x????   # unset: auto-discover any QMK keyboard (recommended)\n\
     # product_id = 0x????   # unset: auto-discover any QMK keyboard (recommended)\n"
        .to_string()
}

/// Render a `config.toml` body that **preserves every field** of `config`.
/// This is the **save renderer** used by every platform's Settings-dialog write
/// path: the optional device-identifying fields (`vendor_id`/`product_id`/
/// `usage_page`/`usage`) are written explicitly when `Some` and as commented-out
/// "auto-discovery" hints when `None`, and the always-present timing fields
/// (`debounce_ms`/`poll_interval_ms`) are written with their actual values.
///
/// The Settings dialog edits only VID/PID, so each save path reads the current
/// [`Config`], overlays the dialog's VID/PID, and serializes the full struct via
/// this function — guaranteeing the user's `usage_page`/`usage`/`debounce_ms`/
/// `poll_interval_ms` survive a VID/PID edit (previously they were silently
/// reset to defaults because the save path rendered a VID/PID-only body).
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
pub fn render_config_body(config: &Config) -> String {
    let vid_line = match config.vendor_id {
        Some(v) => format!("vendor_id  = 0x{v:04x}"),
        None => "# vendor_id  = 0x????   # unset: auto-discover any QMK keyboard (recommended)"
            .to_string(),
    };
    let pid_line = match config.product_id {
        Some(p) => format!("product_id = 0x{p:04x}"),
        None => "# product_id = 0x????   # unset: auto-discover any QMK keyboard (recommended)"
            .to_string(),
    };
    let usage_page_line = match config.usage_page {
        Some(u) => format!("usage_page = 0x{u:04x}"),
        None => "# usage_page = 0xff60".to_string(),
    };
    let usage_line = match config.usage {
        Some(u) => format!("usage      = 0x{u:02x}"),
        None => "# usage      = 0x61".to_string(),
    };
    format!(
        "# QMKonnect Configuration\n\
         #\n\
         # All fields are OPTIONAL. By default QMKonnect auto-discovers any QMK\n\
         # keyboard using the standard Raw HID usage page (0xFF60 / 0x61). Set\n\
         # vendor_id/product_id only to disambiguate among multiple QMK\n\
         # keyboards, or usage_page/usage to target a board that overrode\n\
         # RAW_USAGE_PAGE/RAW_USAGE_ID in its firmware.\n\
         {vid_line}\n\
         {pid_line}\n\
         {usage_page_line}\n\
         {usage_line}\n\
         \n\
         # Debounce window (ms) for coalescing rapid window-change bursts before\n\
         # sending to the keyboard. 0 disables debouncing entirely. Default 50.\n\
         debounce_ms = {debounce}\n\
         \n\
         # (Hyprland only) periodic active-window poll interval (ms).\n\
         # 0 disables. Default 0.\n\
         poll_interval_ms = {poll}\n",
        debounce = config.debounce_ms,
        poll = config.poll_interval_ms,
    )
}

/// Atomically write `content` to `path` via a temp file in `path`'s parent directory
/// followed by `fs::rename`, so a concurrent reader (e.g. `parse_config` / `parse_rules`
/// on the notifier thread) can never observe a truncated or partial file.
///
/// Uses ONLY `std::fs` (no `tempfile` crate): the temp (`.{file_name}.tmp`) lives in the
/// SAME directory as `path`, so `rename` is atomic (same filesystem). Config/rules files
/// are in a per-user dir the process already owns, so there are no permission concerns
/// (unlike `write_rule_atomic`, which targets `/etc/udev/rules.d` and needs `tempfile`).
///
/// On any error after the temp file is created, the temp is removed best-effort. If
/// `fs::write` itself fails, no temp exists and the cleanup is a harmless no-op.
///
/// Signature is a drop-in for `fs::write(path, content)?`.
pub fn atomic_write(path: &Path, content: &str) -> Result<(), Box<dyn Error>> {
    let parent = path.parent().unwrap_or(Path::new("."));
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("atomic_write: path has no file name: {}", path.display()))?;
    // Same parent dir as target => same filesystem => rename is atomic. Leading dot hides
    // the temp on Unix; the name is unique per target within its directory.
    let tmp = parent.join(format!(".{}.tmp", file_name.to_string_lossy()));

    // Stage the body in the temp, then atomically rename it over the target.
    let result = (|| -> Result<(), Box<dyn Error>> {
        fs::write(&tmp, content)?;
        fs::rename(&tmp, path)?;
        Ok(())
    })();

    // If anything failed after the temp was created, remove it (best-effort). A bare `?`
    // would short-circuit past this cleanup, hence the captured-result guard.
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
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
    let default_config = render_default_config_template();

    // Write the config file
    atomic_write(config_path, &default_config)?;

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

/// Render a fully-commented `rules.toml` template (the `spec/HOST_RULES.md` §9
/// schema with every active line prefixed by `# `).
///
/// A freshly-seeded file therefore parses to an all-default `RuleSet` (host
/// rules disabled) — a brand-new install behaves identically to today until
/// the user uncomments and edits entries. Mirrors [`render_config_body`].
///
/// This is the host-rules counterpart to [`render_config_body`]: a pure renderer
/// (no IO) kept here so the `-c` seeder ([`create_default_rules`]) and the
/// "Edit rules" tray seeder ([`edit_rules`]) agree on the file format.
///
/// # Example
///
/// ```rust,ignore
/// use qmkonnect::core::render_rules_body;
/// let body = render_rules_body();
/// // Every active line is commented out, so:
/// let rs: qmkonnect::core::rules::RuleSet = toml::from_str(&body).unwrap();
/// assert!(rs.rules.is_empty());
/// ```
pub fn render_rules_body() -> String {
    // Every active line is prefixed with `# ` (G7) so the seeded file parses to
    // an all-default RuleSet (host rules disabled). The §9 schema verbatim, as a
    // commented-out template the user can edit. A raw string (`r#"..."#`) keeps
    // the embedded `"`s (TOML string values) literal without escaping.
    r#"# QMKonnect Host Rules (rules.toml)
#
# Host rules map the active window to a keyboard layer + callback set.
# See spec/HOST_RULES.md for the full schema. Everything here is commented
# out: uncomment and edit to enable host rules. As-is, this file parses to
# an all-default ruleset (host rules disabled) and a fresh install behaves
# identically to today.
#
# Callback names come from your keyboard's registry — run
# `qmkonnect --list-callbacks` with the keyboard connected to see them.

# Global default for whether the board runs its own config (false = stack)
# or is replaced by the host layer (true = replace). Per-rule overrides win.
# On no match the host layer is always cleared and all host callbacks
# disabled.
# [host]
# disable_firmware_config = false

# Rules: one [[rule]] per (app × behavior). For each matching rule, `layer` is
# first-match-wins (one host layer active — exclusive); `enable`/`disable`
# accumulate across ALL matches (all-match). A rule MUST set at least one of
# `layer` / `enable` / `disable`; it may set layer only, callbacks only, or both.
# `layer` is a RAW QMK layer index (no reserved range): must be != 255 (the
# wire "clear" sentinel) and fit your layer_state width (<=15 default, <=31
# with LAYER_STATE_32BIT); pick one above your highest board layer so it wins.
# Patterns use shell-style globs: `*` is a wildcard, `^`/`$` anchor. A
# catch-all is `match = "*"` — an empty `match = ""` matches ONLY windows
# whose class is empty, not every window.
# [[rule]]
# match = "alacritty"                       # class-only pattern
# layer = 10
# disable_firmware_config = true           # optional override (inherits [host])
#
# [[rule]]
# match = ["*chrome*", "*youtube*"]         # [class_pattern, title_pattern]
# layer = 11
# case_sensitive = false                    # optional, default false
#
# [[rule]]
# match = "neovide"
# enable = ["vim_lazy", "disable_vim"]      # run on focus-in
# disable = ["vim_lazy"]                    # optional: force-off override
#
# [[rule]]
# match = ["*chrome*", "*claude*"]
# enable = ["vim_lazy", "disable_vim"]
# disable_firmware_config = true           # skip the string -> board can't match
"#
    .to_string()
}

/// Create a default (commented) `rules.toml` next to `config.toml`.
///
/// No-op + message if it already exists (mirrors [`create_default_config`]).
/// Creates the parent dir when needed. The rendered body is fully commented
/// (see [`render_rules_body`]) so a fresh install's `rules.toml` parses to an
/// all-default `RuleSet` — host rules stay disabled until the user opts in.
///
/// # Example
///
/// ```rust,ignore
/// use qmkonnect::core::create_default_rules;
/// create_default_rules(&config_dir.join("rules.toml"))?;  // no-op if it exists
/// ```
pub fn create_default_rules(rules_path: &Path) -> Result<(), Box<dyn Error>> {
    if rules_path.exists() {
        println!("rules.toml already exists at: {}", rules_path.display());
        return Ok(());
    }

    // Make sure the directory exists (mirrors create_default_config).
    if let Some(parent) = rules_path.parent() {
        fs::create_dir_all(parent)?;
    }

    atomic_write(rules_path, &render_rules_body())?;

    println!("rules.toml template created at: {}", rules_path.display());
    println!(
        "Host rules are disabled by default (the template is fully commented
\
         out). Uncomment and edit entries to enable host rules, then run
\
         `qmkonnect --validate-rules` to check your file."
    );

    Ok(())
}

/// "Edit rules" tray action (HOST_RULES.md §7): ensure `rules.toml` exists —
/// seed the commented template next to `config.toml` if absent (same body as
/// `qmkonnect -c`; a no-op if it already exists) — then open it in the system
/// default editor. Fire-and-forget (the tray spawns it on a background thread);
/// errors are logged, not fatal.
pub fn edit_rules() {
    let dir = match crate::platforms::create_config_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Edit rules: could not create config dir: {e}");
            return;
        }
    };
    let path = dir.join("rules.toml");
    if let Err(e) = create_default_rules(&path) {
        eprintln!("Edit rules: could not seed {}: {e}", path.display());
        return;
    }
    if let Err(e) = crate::platforms::open_in_default_app(&path) {
        eprintln!("Edit rules: could not open {}: {e}", path.display());
    }
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
    fn render_default_config_template_round_trips_to_defaults() {
        // The seeded template is fully commented -> parses to all-default: the
        // device-identifying fields are None and timing falls back to the serde
        // defaults. A fresh install behaves identically to having no config.
        let body = render_default_config_template();
        let cfg: Config = toml::from_str(&body).unwrap();
        assert_eq!(cfg.vendor_id, None);
        assert_eq!(cfg.product_id, None);
        assert_eq!(cfg.usage_page, None);
        assert_eq!(cfg.usage, None);
        assert_eq!(cfg.debounce_ms, DEFAULT_DEBOUNCE_MS);
        assert_eq!(cfg.poll_interval_ms, DEFAULT_POLL_INTERVAL_MS);
    }

    #[test]
    fn render_config_body_round_trips() {
        // A default config -> all device IDs None, timing at the defaults.
        let body = render_config_body(&Config::default());
        let cfg: Config = toml::from_str(&body).unwrap();
        assert_eq!(cfg.vendor_id, None);
        assert_eq!(cfg.product_id, None);
        assert_eq!(cfg.usage_page, None);
        assert_eq!(cfg.usage, None);
        assert_eq!(cfg.debounce_ms, DEFAULT_DEBOUNCE_MS);
        assert_eq!(cfg.poll_interval_ms, DEFAULT_POLL_INTERVAL_MS);

        // Explicit VID/PID -> parse back to Some (timing still at defaults).
        let body = render_config_body(&Config {
            vendor_id: Some(0xfeed),
            product_id: Some(0x1234),
            ..Config::default()
        });
        let cfg: Config = toml::from_str(&body).unwrap();
        assert_eq!(cfg.vendor_id, Some(0xfeed));
        assert_eq!(cfg.product_id, Some(0x1234));
    }

    #[test]
    fn template_has_no_0xfeed_literal() {
        // §9 gate: "the seeded template contains no literal 0xfeed."
        let seeded = render_default_config_template();
        assert!(
            !seeded.contains("0xfeed"),
            "seeded template still has 0xfeed: {seeded:?}"
        );
        assert!(
            seeded.contains("0x????"),
            "seeded template missing the 0x???? hint: {seeded:?}"
        );
        // The save renderer's None body (Config::default() = all None) must ALSO
        // be clean — G1: both renderers.
        let saved = render_config_body(&Config::default());
        assert!(
            !saved.contains("0xfeed"),
            "save-renderer None body still has 0xfeed: {saved:?}"
        );
        assert!(
            saved.contains("0x????"),
            "save-renderer None body missing 0x????: {saved:?}"
        );
    }

    #[test]
    fn render_config_body_preserves_non_vidpid_fields() {
        // Bug-hunt HIGH finding: saving VID/PID via the Settings dialog must NOT
        // clobber the user's usage_page/usage/debounce_ms/poll_interval_ms.
        // render_config_body serializes the FULL config, so every set field
        // round-trips through a write+re-parse.
        let original = Config {
            vendor_id: Some(0x1234),
            product_id: Some(0x5678),
            usage_page: Some(0xff61),
            usage: Some(0x61),
            debounce_ms: 120,
            poll_interval_ms: 250,
        };
        let body = render_config_body(&original);
        let parsed: Config = toml::from_str(&body).unwrap();
        assert_eq!(parsed.vendor_id, original.vendor_id);
        assert_eq!(parsed.product_id, original.product_id);
        assert_eq!(parsed.usage_page, original.usage_page);
        assert_eq!(parsed.usage, original.usage);
        assert_eq!(parsed.debounce_ms, original.debounce_ms);
        assert_eq!(parsed.poll_interval_ms, original.poll_interval_ms);
    }

    // ========================================================================
    // P5.M1.T1.S1 — render_rules_body + create_default_rules
    // ========================================================================

    #[test]
    fn test_render_rules_body_fully_commented() {
        // G7: every non-blank line in the rendered template must start with `#`
        // so the seeded file parses to an all-default (inert) RuleSet. An
        // uncommented template would activate bogus example rules.
        let body = render_rules_body();
        for (i, line) in body.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.is_empty() {
                continue;
            }
            assert!(
                trimmed.starts_with('#'),
                "line {} is not commented: {line:?}",
                i + 1
            );
        }

        // The template must contain the §9 section markers so the user sees
        // the full schema shape to edit.
        assert!(body.contains("[host]"));
        assert!(body.contains("[[rule]]"));
        assert!(body.contains("disable_firmware_config"));
    }

    #[test]
    fn test_render_rules_body_parses_to_default_ruleset() {
        // The commented template must deserialize to a valid all-default
        // RuleSet (0 layer rules, 0 callback rules) — proves the seeded file is
        // both valid AND inert on a fresh install (legacy parity).
        let body = render_rules_body();
        let rs: rules::RuleSet = toml::from_str(&body)
            .expect("render_rules_body must parse to a valid all-default RuleSet");
        assert!(rs.rules.is_empty());
        assert!(!(rs.host.disable_firmware_config));
    }

    #[test]
    fn test_create_default_rules_noop_if_exists() {
        // Pre-create the file with sentinel content; calling create_default_rules
        // must be a no-op (content UNCHANGED), mirroring create_default_config.
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("rules.toml");
        let sentinel = "# pre-existing sentinel\n";
        std::fs::write(&path, sentinel).unwrap();

        create_default_rules(&path).unwrap();

        let after = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            after, sentinel,
            "existing rules.toml must not be overwritten"
        );
    }

    #[test]
    fn test_create_default_rules_writes_when_absent() {
        // Absent file => written with the rendered body; re-call is a no-op
        // (idempotent, mirrors create_default_config).
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("nested").join("rules.toml");
        assert!(!path.exists());

        create_default_rules(&path).unwrap();
        assert!(path.exists(), "rules.toml should be created when absent");
        let written = std::fs::read_to_string(&path).unwrap();
        assert_eq!(written, render_rules_body());

        // Re-call must be a no-op: overwrite the file with sentinel, re-call,
        // sentinel must survive (idempotent).
        let sentinel = "# sentinel after first write\n";
        std::fs::write(&path, sentinel).unwrap();
        create_default_rules(&path).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            sentinel,
            "second create_default_rules must not overwrite"
        );
    }

    #[test]
    fn test_atomic_write_creates_correct_content() {
        // Happy path: atomic_write stages the body in a sibling .tmp and renames it
        // over the target, so the final file content is exactly `content`.
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("config.toml");

        atomic_write(&path, "vendor_id = 0xfeed\n").unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "vendor_id = 0xfeed\n",
            "final file content must equal the content passed to atomic_write"
        );

        // No temp file must linger in the target directory after a successful write.
        let lingering_tmp = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .any(|e| e.file_name().to_string_lossy().ends_with(".tmp"));
        assert!(
            !lingering_tmp,
            "no .tmp file should remain after a successful atomic_write"
        );
    }

    #[test]
    fn test_atomic_write_replaces_existing() {
        // Overwrite an existing (stale) file: atomic replace must fully replace the
        // content, never concatenate or append.
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "# STALE content\n").unwrap();

        atomic_write(&path, "poll_interval_ms = 250\n").unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "poll_interval_ms = 250\n",
            "atomic_write must fully replace pre-existing content"
        );
    }

    #[test]
    fn test_atomic_write_cleans_up_temp_on_error() {
        // rename of a temp file over a DIRECTORY target fails (EISDIR), exercising the
        // cleanup branch: the staged .tmp must be removed rather than left behind.
        let dir = tempfile::TempDir::new().unwrap();
        let target = dir.path().join("config.toml");
        std::fs::create_dir(&target).unwrap();

        let result = atomic_write(&target, "body");
        assert!(
            result.is_err(),
            "rename of a temp file over a directory must fail (EISDIR)"
        );

        // Enumerate the directory: no .tmp file should remain after the error path.
        let lingering_tmp = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .any(|e| e.file_name().to_string_lossy().ends_with(".tmp"));
        assert!(
            !lingering_tmp,
            "no .tmp file should linger after atomic_write fails mid-write"
        );
    }

    // ========================================================================
    // P1.M5.T1.S1 — mtime+size-keyed Config/Rules cache (cached_config[_at],
    // cached_rules_at). The cache re-stats every call but only re-reads+re-parses
    // when (path, mtime, size) change. Hot-config preserved (no TTL).
    // ========================================================================

    #[test]
    fn test_config_cache_hit_avoids_reparse() {
        // A cache HIT provably skips re-parse: on Linux ext4 mtime has NANOSECOND
        // resolution so two rapid writes always differ — you CANNOT force a hit
        // by controlling mtime. The only rigorous, platform-independent observable
        // is the test-only CONFIG_CACHE_MISSES counter, incremented ONLY on the
        // fall-through to parse_config. Delta 0 across two unchanged calls = HIT.
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "debounce_ms = 100\npoll_interval_ms = 7\n").unwrap();

        let before = CONFIG_CACHE_MISSES.load(Ordering::SeqCst);
        let c1 = cached_config_at(&path).unwrap();
        assert_eq!(c1.debounce_ms, 100);
        assert_eq!(c1.poll_interval_ms, 7);
        let after_first = CONFIG_CACHE_MISSES.load(Ordering::SeqCst);
        assert_eq!(
            after_first - before,
            1,
            "first call is a MISS -> parse_config runs once"
        );

        // Second call, file unchanged -> cache HIT -> parse_config must NOT run.
        let c2 = cached_config_at(&path).unwrap();
        assert_eq!(c2.debounce_ms, 100, "HIT returns the same parsed value");
        let after_second = CONFIG_CACHE_MISSES.load(Ordering::SeqCst);
        assert_eq!(
            after_second - after_first,
            0,
            "second call is a HIT -> no re-parse (mtime+size unchanged)"
        );
    }

    #[test]
    fn test_config_cache_invalidates_on_change() {
        // Hot-config is preserved: a change invalidates on the next call. Verified
        // for BOTH a size change AND an mtime-only change (same byte length), using
        // std::fs::FileTimes to advance mtime deterministically (no flaky sleep).
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "debounce_ms = 100\n").unwrap();
        let _ = cached_config_at(&path).unwrap(); // prime the cache
        let before = CONFIG_CACHE_MISSES.load(Ordering::SeqCst);

        // (a) SIZE change: longer value -> different cache key -> re-parse.
        std::fs::write(&path, "debounce_ms = 2000\n").unwrap();
        let c = cached_config_at(&path).unwrap();
        assert_eq!(c.debounce_ms, 2000, "size change must invalidate");
        let after_size = CONFIG_CACHE_MISSES.load(Ordering::SeqCst);
        assert_eq!(after_size - before, 1, "size change -> MISS (re-parse)");

        // (b) MTIME-only change: same byte length ("100" -> "999"), advance mtime
        // deterministically via FileTimes (stable 1.75; MSRV 1.88 satisfies).
        std::fs::write(&path, "debounce_ms = 999\n").unwrap();
        let f = std::fs::File::options().write(true).open(&path).unwrap();
        let times = std::fs::FileTimes::new()
            .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(2));
        f.set_times(times).unwrap();
        drop(f);
        let c = cached_config_at(&path).unwrap();
        assert_eq!(
            c.debounce_ms, 999,
            "mtime change (same size) must invalidate"
        );
        let after_mtime = CONFIG_CACHE_MISSES.load(Ordering::SeqCst);
        assert_eq!(
            after_mtime - after_size,
            1,
            "mtime change -> MISS (re-parse) — hot-config preserved"
        );
    }

    #[test]
    fn test_rules_cache_hit_and_invalidation_and_no_error_caching() {
        // Parallel to the two config tests, for cached_rules_at. Also asserts the
        // don't-cache-failures rule: a malformed file returns Err WITHOUT storing,
        // so fixing it re-reads cleanly (host_context_for_window's re-arm logic
        // depends on this).
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("rules.toml");
        std::fs::write(&path, "[[rule]]\nmatch = \"x\"\nlayer = 1\n").unwrap();

        // HIT: second call with unchanged file skips re-parse.
        let before = RULES_CACHE_MISSES.load(Ordering::SeqCst);
        let _ = cached_rules_at(&path).unwrap();
        let _ = cached_rules_at(&path).unwrap();
        let after_two = RULES_CACHE_MISSES.load(Ordering::SeqCst);
        assert_eq!(
            after_two - before,
            1,
            "first=MISS, second=HIT -> exactly one parse"
        );

        // INVALIDATE: different size -> re-parse, and the new value is picked up.
        std::fs::write(&path, "[[rule]]\nmatch = \"y\"\nlayer = 22\n").unwrap();
        let rs = cached_rules_at(&path).unwrap();
        let after_three = RULES_CACHE_MISSES.load(Ordering::SeqCst);
        assert_eq!(after_three - after_two, 1, "size change -> MISS");
        assert_eq!(
            rs.rules[0].layer,
            Some(22),
            "invalidation picked up the new value"
        );

        // NO ERROR CACHING: a malformed file returns Err and is NOT stored, so
        // fixing it re-reads.
        std::fs::write(&path, "this is = = not valid toml\n").unwrap();
        assert!(cached_rules_at(&path).is_err(), "malformed -> Err");
        std::fs::write(&path, "[[rule]]\nmatch = \"z\"\nlayer = 3\n").unwrap();
        let rs =
            cached_rules_at(&path).expect("error was NOT cached -> fixed file re-reads cleanly");
        assert_eq!(rs.rules[0].layer, Some(3));
    }
}

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod core;
mod platforms;
mod runners;
mod tray;

// Per-user "Open at Login" autostart (Windows) backed by the HKCU `Run` key.
// Self-contained so it merges cleanly with the parallel macOS SMAppService
// work (which lives privately inside `tray.rs`). See HANDOFF_WINDOWS_OPEN_AT_LOGIN.md.
#[cfg(target_os = "windows")]
mod autostart;

// StatusNotifierItem (SNI) tray for the Linux/Wayland build. Included in
// the default build via the `linux-tray` feature (see Cargo.toml `default`).
#[cfg(all(target_os = "linux", feature = "linux-tray"))]
mod linux_tray;

use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::process;

#[cfg(target_os = "windows")]
use log::{error, info};

#[cfg(target_os = "windows")]
fn init_logging() -> Result<(), Box<dyn Error>> {
    // Try to initialize Windows Event Log first
    match eventlog::init("QMKonnect", log::Level::Info) {
        Ok(()) => {
            info!("Windows Event Log initialized");
            Ok(())
        }
        Err(e) => {
            // Fallback to console logging if event log fails
            env_logger::init();
            eprintln!(
                "Failed to initialize Windows Event Log, using console: {}",
                e
            );
            Ok(())
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn init_logging() -> Result<(), Box<dyn Error>> {
    // For non-Windows platforms, we'll use simple console logging
    // env_logger is only available on Windows in this configuration
    Ok(())
}

fn main() {
    // Initialize logging first
    if let Err(e) = init_logging() {
        eprintln!("Failed to initialize logging: {}", e);
        process::exit(1);
    }

    if let Err(e) = run() {
        #[cfg(target_os = "windows")]
        error!("Application error: {}", e);
        #[cfg(not(target_os = "windows"))]
        eprintln!("Application error: {}", e);
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    let verbose = args.iter().any(|arg| arg == "-v" || arg == "--verbose");

    // Check for help
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_help();
        return Ok(());
    }

    // Check for configuration mode
    if args.iter().any(|arg| arg == "-c" || arg == "--config") {
        return create_config();
    }

    // Check for reload mode
    if args.iter().any(|arg| arg == "-r" || arg == "--reload") {
        // Value flags for root-aware config resolution (Linux #26): a sudo'd
        // `qmkonnect -r` has HOME=/root, so let the user point us at their config
        // explicitly when auto-detection of the invoking user fails.
        let config = parse_value_flag(&args, "--config").map(PathBuf::from);
        let user = parse_value_flag(&args, "--user");
        let uid = parse_value_flag(&args, "--uid").and_then(|s| s.parse::<u32>().ok());
        return reload_config(verbose, config, user, uid);
    }

    // Check for platform list mode
    if args.iter().any(|arg| arg == "-l" || arg == "--list") {
        print_platforms();
        return Ok(());
    }

    // List connected HID devices (VID/PID discovery) — read-only enumeration.
    if args.iter().any(|arg| arg == "--list-devices") {
        crate::core::notifier::list_devices()?;
        return Ok(());
    }

    // Debug aid: render the "Show Window Information" dialog directly, without
    // the tray, so the window-path can be tested/inspected in isolation.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if args.iter().any(|arg| arg == "--show-window-info") {
        let rows = platforms::list_foreground_windows();
        #[cfg(target_os = "macos")]
        {
            crate::tray::show_macos_window_info_dialog(&rows)?;
            return Ok(());
        }
        #[cfg(target_os = "windows")]
        {
            crate::tray::show_window_info_dialog(&rows)?;
            return Ok(());
        }
    }

    // --list-callbacks: handshake the connected keyboard and print its
    // callback name->id table (PRD §4 / HOST_RULES.md §8(6)). Needs real
    // v2-capable hardware; manual-only validation.
    if args.iter().any(|a| a == "--list-callbacks") {
        return list_callbacks(verbose);
    }

    // --validate-rules: lint rules.toml (schema via rules::parse_rules +
    // optional callback-name warnings). --rules-path overrides the location
    // (G4: --rules-path alone is a no-op; it is consumed only here).
    if args.iter().any(|a| a == "--validate-rules") {
        let rules_path = parse_value_flag(&args, "--rules-path").map(PathBuf::from);
        return validate_rules(rules_path, verbose);
    }

    // Use platform-specific runner
    let mut runner = runners::create_runner(verbose)?;
    runner.run(&args)
}

fn print_help() {
    println!("QMKonnect v{}", env!("CARGO_PKG_VERSION"));
    println!("Usage: qmkonnect [OPTIONS]");
    println!("\nOptions:");
    println!("  -h, --help     Display this help message");
    println!("  -v, --verbose  Enable verbose logging");
    println!("  -c, --config   Create a configuration file");
    println!("  -r, --reload   Reload configuration and update system files");
    println!("      --config <path>  Config file to use with --reload");
    println!("      --user <name>    Invoking user for sudo'd --reload (Linux)");
    println!("      --uid <n>        Invoking uid for sudo'd --reload (Linux)");
    println!("  -l, --list     List supported platforms");
    println!("  --list-devices List connected HID devices (VID/PID discovery)");

    // PRD §4 / HOST_RULES.md §8(6): the host-rules diagnostic CLI flags.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    println!("  --show-window-info  [macOS/Windows] open the Window Information dialog");
    println!("      --list-callbacks   Handshake the keyboard; print its callback name->id table");
    println!("      --validate-rules   Parse rules.toml; report schema/callback-name errors");
    println!(
        "          --rules-path <path>  Override the rules.toml location (with --validate-rules)"
    );

    #[cfg(target_os = "windows")]
    {
        println!("\nWindows Options:");
        println!("  --console              Run in console mode (for debugging)");
        println!("  --tray-app             Run as tray application");
    }

    println!("\nRunning without options will start the notifier service");
}

fn print_platforms() {
    println!("Supported platforms (this build):");

    #[cfg(all(target_os = "linux", feature = "hyprland"))]
    println!("  Linux (Hyprland)");

    #[cfg(all(target_os = "linux", not(feature = "hyprland")))]
    println!("  Linux (X11)");

    #[cfg(target_os = "macos")]
    println!("  macOS");

    #[cfg(target_os = "windows")]
    println!("  Windows");
}

// Used only off-Linux (Linux resolves root-aware via
// `platforms::resolve_config_for_reload`), so gate it to those targets.
#[cfg(not(target_os = "linux"))]
fn get_config_path() -> Result<PathBuf, Box<dyn Error>> {
    // Get platform-specific config paths
    let config_paths = platforms::get_config_paths();

    // Try each path in order
    for path in config_paths {
        if path.exists() {
            return Ok(path);
        }
    }

    Err("No configuration file found in any of the expected locations".into())
}

/// Parse a `--flag <value>` or `--flag=<value>` option from argv. Returns the
/// value when present, mirroring how the boolean flags are scanned from
/// `env::args`. Used by the reload subcommand's `--config`/`--user`/`--uid`.
fn parse_value_flag(args: &[String], name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        if a == name {
            return iter.next().cloned();
        }
        if let Some(rest) = a.strip_prefix(&prefix) {
            return Some(rest.to_string());
        }
    }
    None
}

/// Collect every callback name referenced by a parsed `rules.toml` (the union
/// of all `callback_rules[].enable` + `callback_rules[].disable`), deduped +
/// sorted. Pure (no IO, no globals) ⇒ thread-safe + unit-testable. Used by
/// `--validate-rules` to report names not present in the live handshake map.
/// (`BTreeSet` ⇒ deterministic sorted output.) Required because
/// `notifier::unknown_callback_names` is private (D6/G2) — main.rs owns its own
/// collector.
///
/// # Example
///
/// ```rust,ignore
/// use qmkonnect::core::rules::RuleSet;
/// let rules: RuleSet = toml::from_str(r#"
/// [[callback_rules]]
/// match = "a"
/// enable = ["x", "y"]
/// disable = ["x"]
/// "#).unwrap();
/// let names = collect_callback_names(&rules);
/// assert_eq!(names.iter().collect::<Vec<_>>(), [&"x", &"y"]);
/// ```
fn collect_callback_names(
    rules: &crate::core::rules::RuleSet,
) -> std::collections::BTreeSet<String> {
    let mut names = std::collections::BTreeSet::new();
    for rule in &rules.callback_rules {
        for n in rule.enable.iter().chain(rule.disable.iter()) {
            names.insert(n.clone());
        }
    }
    names
}

/// `--list-callbacks`: handshake the connected keyboard and print its callback
/// name→id table (sorted by id). With legacy firmware prints "Legacy firmware
/// (no callback support)…"; with no board prints a clear no-device message.
/// Always returns `Ok(())` (exit 0) — discovery is informational, never fatal.
fn list_callbacks(verbose: bool) -> Result<(), Box<dyn Error>> {
    if !crate::core::notifier::is_device_connected() {
        println!(
            "No QMK device connected. Connect a keyboard with host-rules firmware and re-run."
        );
        return Ok(());
    }

    crate::core::notifier::perform_handshake(verbose);

    if crate::core::notifier::host_capable() {
        let names = crate::core::notifier::callback_names(); // HashMap<String, u8>
        if names.is_empty() {
            println!("Connected keyboard reports 0 callbacks.");
        } else {
            let mut rows: Vec<_> = names.into_iter().collect();
            rows.sort_by_key(|(_, id)| *id);
            println!("Callback name -> id ({}):", rows.len());
            for (name, id) in rows {
                println!("  {id:>3}  {name}");
            }
        }
    } else {
        println!(
            "Legacy firmware (no callback support) — host rules will run in string-only mode."
        );
    }

    Ok(())
}

/// `--validate-rules`: lint a `rules.toml` for schema/callback-name errors.
///
/// Path resolution (D3): an explicit `--rules-path` that does not exist is an
/// error (exit non-zero); no `--rules-path` and no candidate found is info
/// (exit 0 — host rules disabled is valid). Schema/parse errors exit non-zero
/// (D4); unknown callback names are warnings (exit 0 — G6: `evaluate` skips
/// them silently and a device may be disconnected).
///
/// NOTE (D5b): when a device is connected, `perform_handshake` ALSO runs its
/// own rules-mismatch check against the DEFAULT rules.toml during the sweep,
/// emitting its own `⚠` warnings always (regardless of `verbose`). Those are
/// benign supplementary noise; this function's own `⚠` lines are the
/// authoritative result for the file under validation.
fn validate_rules(rules_path: Option<PathBuf>, verbose: bool) -> Result<(), Box<dyn Error>> {
    // Resolve the path: explicit --rules-path (missing => Err, G5) else first
    // existing candidate (none => info/exit 0, G5).
    let path = match rules_path {
        Some(p) => {
            if !p.exists() {
                eprintln!("rules file not found: {}", p.display());
                return Err(format!("rules file not found: {}", p.display()).into());
            }
            p
        }
        None => match crate::core::rules::get_rules_paths()
            .into_iter()
            .find(|p| p.exists())
        {
            Some(p) => p,
            None => {
                println!("No rules.toml found (host rules disabled). Nothing to validate.");
                return Ok(());
            }
        },
    };

    println!("Validating {}", path.display());

    // Schema check via the single source of truth (G3: parse_rules' strictness
    // — missing match/layer, malformed TOML — IS the validation).
    let rs = match crate::core::rules::parse_rules(&path) {
        Ok(rs) => rs,
        Err(e) => {
            eprintln!("rules.toml invalid: {e}");
            return Err(e); // exit non-zero (D4/G6)
        }
    };

    // Optional callback-name validation (D5): only if a device is connected +
    // capable. Unknown names are WARNINGS (exit 0) — not fatal (G6).
    if crate::core::notifier::is_device_connected() {
        // Populates the name→id map. D5b: this may ALSO print the handshake's
        // own ⚠ warnings on the default rules.toml — benign supplementary noise.
        crate::core::notifier::perform_handshake(verbose);
        if crate::core::notifier::host_capable() {
            let known = crate::core::notifier::callback_names();
            let unknown = collect_callback_names(&rs)
                .into_iter()
                .filter(|n| !known.contains_key(n));
            let mut warned = false;
            for n in unknown {
                eprintln!("⚠  unknown callback: {n}");
                warned = true;
            }
            if !warned {
                println!("All callback names recognized.");
            }
        } else {
            println!("Legacy firmware — callback-name validation skipped (schema-only).");
        }
    } else {
        println!("Device not connected — callback-name validation skipped (schema-only).");
    }

    println!(
        "rules.toml valid: {} layer rules, {} callback rules.",
        rs.layer_rules.len(),
        rs.callback_rules.len()
    );
    Ok(())
}

fn reload_config(
    verbose: bool,
    config: Option<PathBuf>,
    user: Option<String>,
    uid: Option<u32>,
) -> Result<(), Box<dyn Error>> {
    println!("Reloading configuration...");

    // Resolve the config file. On Linux this is root-aware: under `sudo` HOME is
    // /root, so the plain search would never find the invoking user's config
    // and the old code silently no-op'd without writing any rule (#26). It now
    // resolves the invoking user via $SUDO_UID/$SUDO_USER/`getent` (or the
    // explicit flags) and — crucially — FAILS LOUDLY instead of returning Ok.
    #[cfg(target_os = "linux")]
    let config_path = platforms::resolve_config_for_reload(config, user, uid)?;
    #[cfg(not(target_os = "linux"))]
    let config_path = {
        let _ = (user, uid); // unused off Linux
        match config {
            Some(p) if p.exists() => Ok(p),
            _ => get_config_path(),
        }?
    };

    // Parse configuration using our improved parser
    let config = core::parse_config(&config_path)?;

    // The values are now Option<u16> (None = auto-discovery).
    let vendor_id = config.vendor_id;
    let product_id = config.product_id;

    if verbose {
        let vid = vendor_id
            .map(|v| format!("{v:#06x}"))
            .unwrap_or_else(|| "auto".to_string());
        let pid = product_id
            .map(|p| format!("{p:#06x}"))
            .unwrap_or_else(|| "auto".to_string());
        println!("Read configuration from {}", config_path.display());
        println!("Using vendor_id: {}, product_id: {}", vid, pid);
    }

    // Update platform-specific configuration
    #[cfg(target_os = "linux")]
    {
        // Write the on-demand VID/PID fallback rule. update_udev_rules itself
        // no-ops (cleanly) when both IDs are unset — default keyboards are
        // auto-discovered by usage page/usage and covered by the static rule.
        if let Err(e) = platforms::update_udev_rules(vendor_id, product_id, verbose) {
            if verbose {
                println!("Warning: Could not update udev rules: {}", e);
            }
        }

        if let Err(e) = platforms::reload_udev_rules() {
            if verbose {
                println!("Warning: Could not reload udev rules: {}", e);
            }
        }
    }

    println!("Configuration reloaded successfully.");
    Ok(())
}

fn create_config() -> Result<(), Box<dyn Error>> {
    println!("Creating configuration...");

    // platforms::create_config_dir dispatches per-OS and has a portable fallback
    let config_dir = platforms::create_config_dir()?;

    // Create the config file using our new function
    let config_path = config_dir.join("config.toml");
    core::create_default_config(&config_path)?;

    // Seed a fully-commented rules.toml template next to config.toml (PRD §4 /
    // HOST_RULES.md §8(6)/§9). No-op if it already exists (G7: the template is
    // fully commented so it parses to all-defaults — host rules disabled).
    let rules_path = config_dir.join("rules.toml");
    core::create_default_rules(&rules_path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- parse_value_flag: the 3 forms it accepts (space, equals, absent) ----
    // These are the first unit tests in main.rs (G9: previously untested).

    #[test]
    fn test_parse_value_flag_space_form() {
        // `--flag value` form.
        let args = vec!["--rules-path".to_string(), "x.toml".to_string()];
        assert_eq!(
            parse_value_flag(&args, "--rules-path"),
            Some("x.toml".to_string())
        );
    }

    #[test]
    fn test_parse_value_flag_equals_form() {
        // `--flag=value` form.
        let args = vec!["--rules-path=x.toml".to_string()];
        assert_eq!(
            parse_value_flag(&args, "--rules-path"),
            Some("x.toml".to_string())
        );
    }

    #[test]
    fn test_parse_value_flag_absent() {
        // Flag not present at all.
        let args: Vec<String> = vec!["-v".to_string(), "--verbose".to_string()];
        assert_eq!(parse_value_flag(&args, "--rules-path"), None);
    }

    // ---- collect_callback_names: dedupe + sorted union + empty default ----

    #[test]
    fn test_collect_callback_names_dedupes() {
        // Overlapping enable/disable names across rules => BTreeSet union,
        // deduped + sorted. D6: this helper is required because
        // notifier::unknown_callback_names is private.
        let toml = r#"
[[callback_rules]]
match = "a"
enable = ["vim_lazy", "disable_vim"]
disable = ["vim_lazy"]

[[callback_rules]]
match = "b"
enable = ["alpha"]
disable = ["beta", "disable_vim"]
"#;
        let rules: crate::core::rules::RuleSet = toml::from_str(toml).unwrap();
        let names = collect_callback_names(&rules);
        let collected: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        assert_eq!(collected, vec!["alpha", "beta", "disable_vim", "vim_lazy"]);
    }

    #[test]
    fn test_collect_callback_names_empty_when_no_rules() {
        // A default RuleSet (no callback_rules) => empty set.
        let rules = crate::core::rules::RuleSet::default();
        let names = collect_callback_names(&rules);
        assert!(names.is_empty());
    }
}

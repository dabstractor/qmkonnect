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

// StatusNotifierItem (SNI) tray for the Linux/Wayland build. Opt-in via the
// `linux-tray` feature; absent from the default build entirely.
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

    Ok(())
}

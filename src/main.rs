#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod core;
mod platforms;
mod runners;
mod tray;

#[cfg(target_os = "windows")]
mod service;

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
        return reload_config(verbose);
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
    println!("  -l, --list     List supported platforms");

    #[cfg(target_os = "windows")]
    {
        println!("\nWindows Options:");
        println!("  --console              Run in console mode (for debugging)");
        println!("  --tray-app             Run as tray application");
        println!("\nWindows Service Options:");
        println!("  --service              Run as Windows service (used internally)");
        println!("  --install-service      Install Windows service");
        println!("  --uninstall-service    Uninstall Windows service");
        println!("  --start-service        Start Windows service");
        println!("  --stop-service         Stop Windows service");
    }

    println!("\nRunning without options will start the notifier service");
}



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

fn reload_config(verbose: bool) -> Result<(), Box<dyn Error>> {
    println!("Reloading configuration...");

    // Get config path
    let config_path = match get_config_path() {
        Ok(path) => path,
        Err(e) => {
            println!("Note: Could not update system configuration: {}", e);
            return Ok(());
        }
    };

    // Parse configuration using our improved parser
    let config = core::parse_config(&config_path)?;

    // The values are already u16
    let vendor_id = config.vendor_id;
    let product_id = config.product_id;

    if verbose {
        println!("Read configuration from {}", config_path.display());
        println!(
            "Using vendor_id: {:#06x}, product_id: {:#06x}",
            vendor_id, product_id
        );
    }

    // Update platform-specific configuration
    #[cfg(target_os = "linux")]
    {
        // Convert the numeric values back to hex strings for udev rules
        let vendor_id_hex = platforms::decimal_to_hex(vendor_id);
        let product_id_hex = platforms::decimal_to_hex(product_id);

        if let Err(e) = platforms::update_udev_rules(vendor_id_hex, product_id_hex, verbose) {
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

    // Create config directory with platform-specific method
    #[cfg(target_os = "linux")]
    let config_dir = platforms::create_config_dir()?;

    #[cfg(target_os = "windows")]
    let config_dir = platforms::create_config_dir()?;

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let config_dir = {
        // Default implementation for other platforms
        if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(xdg_config).join("qmk-notifier")
        } else if let Some(home) = dirs::home_dir() {
            home.join(".config").join("qmk-notifier")
        } else {
            return Err("Could not determine configuration directory".into());
        }
    };

    // Create the config file using our new function
    let config_path = config_dir.join("config.toml");
    core::create_default_config(&config_path)?;

    Ok(())
}

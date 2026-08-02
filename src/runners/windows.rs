use crate::platforms;
use crate::runners::PlatformRunner;
use crate::tray;
use log::{error, info};
use single_instance::SingleInstance;
use std::error::Error;
use std::process;

pub struct WindowsRunner {
    verbose: bool,
}

impl WindowsRunner {
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }

    /// Detect a previous instance via a named mutex. The owning `SingleInstance`
    /// is intentionally leaked so the mutex stays held for the process lifetime,
    /// avoiding the former `static mut INSTANCE` data race (#5) entirely.
    fn is_already_running() -> Result<bool, Box<dyn Error>> {
        let instance = SingleInstance::new("qmkonnect-app-id").map_err(|e| -> Box<dyn Error> {
            format!("Failed to create single instance: {}", e).into()
        })?;

        if !instance.is_single() {
            // Another instance is already running.
            return Ok(true);
        }

        // Hold the mutex for the life of the process by leaking it.
        Box::leak(Box::new(instance));
        Ok(false)
    }

    fn run_console_mode(&self) -> Result<(), Box<dyn Error>> {
        // Console mode for Windows debugging.
        println!("Creating Windows monitor...");
        let monitor = platforms::create_monitor(self.verbose)?;

        println!("QMKonnect started in console mode");
        if self.verbose {
            println!("Verbose logging enabled");
            println!("Using platform: {}", monitor.platform_name());
        }

        // Read-only startup probe so a typo'd VID/PID is obvious immediately (#16).
        crate::core::notifier::startup_device_probe(self.verbose);
        // If a device is already connected at startup, run the capability handshake
        // now (poll-thread reconnects are handled in tray.rs). Completes before the
        // poll thread exists; idempotent via HAS_HANDSHAKED.
        crate::core::notifier::record_startup_device_state();
        if crate::core::notifier::is_device_connected() {
            crate::core::notifier::perform_handshake(self.verbose);
        }

        ctrlc::set_handler(move || {
            println!("\nReceived Ctrl+C, shutting down...");
            process::exit(0);
        })?;

        println!("Starting Windows monitor...");
        let mut monitor = monitor;
        if let Err(e) = monitor.start() {
            eprintln!("Failed to start Windows monitor: {}", e);
            return Err(e);
        }

        if self.verbose {
            println!("Windows monitor started successfully");
        }

        println!("Press Ctrl+C to exit...");
        println!("Now switch between different applications to test window detection...");
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }

    fn run_tray_app(&self) -> Result<(), Box<dyn Error>> {
        // Singleton guard.
        if Self::is_already_running()? {
            if self.verbose {
                println!("Another instance is already running, exiting");
            }
            info!("Another instance is already running, exiting");
            return Ok(());
        }

        if self.verbose {
            println!("No other instance detected, starting application");
        }
        info!("Starting QMKonnect as tray application");

        let monitor = platforms::create_monitor(self.verbose)?;

        if self.verbose {
            info!("Using platform: {}", monitor.platform_name());
        }

        // Read-only startup probe so a typo'd VID/PID is obvious immediately (#16).
        crate::core::notifier::startup_device_probe(self.verbose);
        // If a device is already connected at startup, run the capability handshake
        // now (poll-thread reconnects are handled in tray.rs). Completes before the
        // poll thread exists; idempotent via HAS_HANDSHAKED.
        crate::core::notifier::record_startup_device_state();
        if crate::core::notifier::is_device_connected() {
            crate::core::notifier::perform_handshake(self.verbose);
        }

        // Start the monitor before setting up the tray (matches the working order).
        let mut monitor = monitor;
        if let Err(e) = monitor.start() {
            error!("Failed to start Windows monitor: {}", e);
            return Err(e);
        }
        if self.verbose {
            println!("Windows monitor started successfully");
        }

        // The tray event loop blocks until the user quits.
        tray::setup_tray(self.verbose);

        info!("Tray application shutting down");
        Ok(())
    }
}

impl PlatformRunner for WindowsRunner {
    fn run(&mut self, args: &[String]) -> Result<(), Box<dyn Error>> {
        // Check if running as tray app
        if args.iter().any(|arg| arg == "--tray-app") {
            info!("Starting as tray application");
            return self.run_tray_app();
        }

        // Console mode (for debugging)
        if args.iter().any(|arg| arg == "--console") {
            unsafe {
                use windows::Win32::System::Console::AllocConsole;
                let _ = AllocConsole();
            }
            return self.run_console_mode();
        }

        // Default behavior on Windows: run as tray app
        info!("Starting as tray application (default)");
        self.run_tray_app()
    }
}

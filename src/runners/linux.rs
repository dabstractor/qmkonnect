use crate::platforms;
use crate::runners::PlatformRunner;
use std::error::Error;
use std::process;

pub struct LinuxRunner {
    verbose: bool,
}

impl LinuxRunner {
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }
}

impl PlatformRunner for LinuxRunner {
    fn run(&mut self, _args: &[String]) -> Result<(), Box<dyn Error>> {
        let mut monitor = platforms::create_monitor(self.verbose)?;

        println!("QMKonnect started");
        if self.verbose {
            println!("Verbose logging enabled");
            println!("Using platform: {}", monitor.platform_name());
        }

        // Read-only startup probe so a typo'd VID/PID is obvious immediately (#16).
        crate::core::notifier::startup_device_probe(self.verbose);
        // If a device is already connected at startup, run the capability handshake
        // now (poll-thread reconnects are handled in linux_tray.rs / tray.rs).
        // Completes before the poll thread exists; idempotent via HAS_HANDSHAKED.
        crate::core::notifier::record_startup_device_state();
        if crate::core::notifier::is_device_connected() {
            crate::core::notifier::perform_handshake(self.verbose);
        }

        // Exit promptly on Ctrl+C / SIGTERM. We rely on systemd Restart=always
        // (and `panic = "abort"` in release) for crash recovery instead of the
        // former `catch_unwind` scaffolding, which is a no-op under panic=abort.
        ctrlc::set_handler(move || {
            println!("\nReceived interrupt, shutting down...");
            process::exit(0);
        })?;

        // Hyprland: the monitor's start() blocks the calling thread on the IPC
        // event listener. There is no GUI loop to run alongside it.
        #[cfg(all(target_os = "linux", feature = "hyprland"))]
        {
            // Register the StatusNotifierItem tray (if enabled) before blocking
            // on the IPC listener. ksni owns its D-Bus thread; we only need to
            // keep the handle alive for the process lifetime. Failure (e.g. no
            // session bus) is logged, not fatal.
            #[cfg(feature = "linux-tray")]
            let _tray_handle = crate::linux_tray::spawn(self.verbose);

            monitor.start()?;
        }

        // Non-Hyprland Linux (X11): run the monitor in a background thread and
        // either drive the system-tray event loop on this (main) thread, or —
        // when the SNI tray is in use — block here since ksni already runs on
        // its own D-Bus thread.
        #[cfg(all(target_os = "linux", not(feature = "hyprland")))]
        {
            // The SNI tray (if enabled) runs on its own thread; keep the handle
            // alive for the process lifetime.
            #[cfg(feature = "linux-tray")]
            let _tray_handle = crate::linux_tray::spawn(self.verbose);

            let monitor_handle = std::thread::spawn(move || {
                if let Err(e) = monitor.start() {
                    eprintln!("Monitor error: {}", e);
                }
            });

            #[cfg(not(feature = "linux-tray"))]
            {
                crate::tray::setup_tray(self.verbose);
                if self.verbose {
                    println!("System tray icon initialized");
                }
            }

            // When the SNI tray is enabled there is no blocking GUI loop to
            // drive, so park the main thread; Ctrl+C / SIGTERM still exits the
            // process via the handler above. The loop guards against spurious
            // unparks.
            #[cfg(feature = "linux-tray")]
            {
                if self.verbose {
                    println!("StatusNotifierItem tray active; blocking on main thread");
                }
                loop {
                    std::thread::park();
                }
            }

            // The tray exited (user quit); let the monitor wind down with the process.
            // We deliberately don't join: the monitor thread is torn down on exit.
            drop(monitor_handle);
        }

        println!("Monitor stopped, exiting.");
        Ok(())
    }
}

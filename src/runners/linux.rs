#![cfg(target_os = "linux")]

use crate::platforms;
use crate::runners::PlatformRunner;
use std::error::Error;
use std::panic::{self, AssertUnwindSafe};
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

        // Set up signal handling for immediate exit
        ctrlc::set_handler(move || {
            println!("\nReceived Ctrl+C, shutting down...");
            // Force immediate exit - no waiting or additional complexity
            process::exit(0);
        })?;

        // For Hyprland, start the monitor on the main thread with panic recovery
        #[cfg(all(target_os = "linux", feature = "hyprland"))]
        {
            // Use panic recovery to prevent crashes from taking down the entire application
            let result = panic::catch_unwind(AssertUnwindSafe(|| {
                monitor.start()
            }));
            
            match result {
                Ok(Ok(())) => {
                    // Monitor completed normally (e.g., Ctrl+C)
                    if self.verbose {
                        println!("Hyprland monitor completed normally");
                    }
                }
                Ok(Err(e)) => {
                    // Monitor encountered an error - return it to trigger systemd restart
                    eprintln!("Hyprland monitor error: {}", e);
                    return Err(e);
                }
                Err(panic_info) => {
                    // Monitor panicked - log it and trigger systemd restart
                    eprintln!("Hyprland monitor panicked: {:?}", panic_info);
                    return Err("Monitor thread panicked".into());
                }
            }
        }

        // For non-Hyprland Linux, start the monitor in a supervised thread
        #[cfg(all(target_os = "linux", not(feature = "hyprland")))]
        {
            use std::thread;
            let verbose = self.verbose;
            
            // Supervised thread with panic recovery
            let monitor_thread = thread::spawn(move || {
                // Use panic recovery to prevent crashes from taking down the entire application
                let result = panic::catch_unwind(AssertUnwindSafe(|| {
                    if let Err(e) = monitor.start() {
                        eprintln!("Monitor error: {}", e);
                        return Err(e);
                    }
                    Ok(())
                }));
                
                match result {
                    Ok(Ok(())) => {
                        // Monitor completed normally
                        if verbose {
                            println!("Monitor thread completed normally");
                        }
                    }
                    Ok(Err(e)) => {
                        // Monitor encountered an error - log it but don't crash the app
                        eprintln!("Monitor thread failed: {}", e);
                    }
                    Err(panic_info) => {
                        // Monitor panicked - log it but don't crash the app
                        eprintln!("Monitor thread panicked: {:?}", panic_info);
                    }
                }
                Ok(())
            });

            // Setup tray icon for non-Hyprland Linux
            crate::tray::setup_tray();

            if self.verbose {
                println!("System tray icon initialized");
            }

            // Join the monitor thread - don't return errors to avoid restart loops
            if let Err(e) = monitor_thread.join() {
                eprintln!("Error joining Monitor thread: {:?}", e);
                // Don't return error - let the application continue running
            }
        }

        // If we reach here, the monitor stopped on its own
        println!("Monitor stopped, exiting.");

        Ok(())
    }
}
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
        println!("QMKonnect started");
        if self.verbose {
            println!("Verbose logging enabled");
        }

        // These run in BOTH the monitor-Ok and no-backend cases — the app stays
        // useful even without a window backend (PLATFORMS.md §6 "No-backend
        // fallback"): the tray + device-status poll + HID pipeline keep running.
        crate::core::notifier::startup_device_probe(self.verbose);
        // If a device is already connected at startup, run the capability handshake
        // now (poll-thread reconnects are handled in linux_tray.rs / tray.rs).
        // Completes before the poll thread exists; idempotent via HAS_HANDSHAKED.
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

        // Tray — IDENTICAL in both the Ok and Err branches; spawned first so the
        // icon is up before we block/park.
        //
        // cfg coupling (tray.rs): tray.rs is compiled for
        // `cfg(not(all(target_os="linux", feature="hyprland")))`. Under the
        // default build (hyprland ON) tray.rs is ABSENT on Linux, so
        // `crate::tray::setup_tray` must be gated on BOTH `not(linux-tray)` AND
        // `not(hyprland)`. `linux_tray::spawn` is fine under hyprland (separate
        // SNI module that owns its own D-Bus thread).
        #[cfg(feature = "linux-tray")]
        let _tray_handle = crate::linux_tray::spawn(self.verbose);
        #[cfg(all(not(feature = "linux-tray"), not(feature = "hyprland")))]
        crate::tray::setup_tray(self.verbose);

        // The merged runtime path. `create_monitor` delegates to
        // `select_linux_backend`, which probes each compiled-in backend at runtime
        // and returns `Err` when none is available. We do NOT propagate that `Err`
        // with `?` — the no-backend case keeps the tray + device pipeline alive.
        match platforms::create_monitor(self.verbose) {
            Ok(mut monitor) => {
                if self.verbose {
                    println!("Using platform: {}", monitor.platform_name());
                }
                if monitor.start_blocks_calling_thread() {
                    // Hyprland: start() blocks the calling thread on the IPC
                    // event listener. There is no GUI loop to run alongside it.
                    monitor.start()?;
                } else {
                    // Spawn-and-return backends (X11, and the future foreign-
                    // toplevel / GNOME / AT-SPI): run the monitor on a worker
                    // thread and keep main alive via the tray loop or park.
                    #[cfg(feature = "linux-tray")]
                    let monitor_verbose = self.verbose;
                    let _monitor_handle = std::thread::spawn(move || {
                        if let Err(e) = monitor.start() {
                            eprintln!("Monitor error: {}", e);
                        }
                    });

                    // When the SNI tray is enabled there is no blocking GUI loop
                    // to drive (ksni owns its D-Bus thread), so park main;
                    // Ctrl+C / SIGTERM still exits via the handler above.
                    #[cfg(feature = "linux-tray")]
                    {
                        if monitor_verbose {
                            println!("StatusNotifierItem tray active; blocking on main thread");
                        }
                        loop {
                            std::thread::park();
                        }
                    }
                    // (under not(linux-tray) AND not(hyprland): tray::setup_tray
                    // above already drives a blocking loop — fall through.)
                }
            }
            Err(e) => {
                // No window backend available (every probe failed). The app is NOT
                // useless: the tray + device-status poll + HID pipeline are all
                // already running above. Fire the GNOME one-shot extension hint,
                // then keep main alive. PLATFORMS.md §6 / §8.4.
                eprintln!(
                    "No Linux window backend available; running tray + device pipeline only. ({e})"
                );
                maybe_gnome_first_run_notify(self.verbose);

                // Keep main alive so the tray + poll thread keep running. Under
                // linux-tray, park main (ksni owns its D-Bus thread). Under
                // not(linux-tray) AND not(hyprland), tray::setup_tray above is
                // already the blocking loop — fall through. Under hyprland+
                // not(linux-tray) there is no tray loop at all, so park main
                // explicitly to keep the device pipeline alive.
                #[cfg(feature = "linux-tray")]
                loop {
                    std::thread::park();
                }
                #[cfg(all(feature = "hyprland", not(feature = "linux-tray")))]
                loop {
                    std::thread::park();
                }
                #[cfg(all(not(feature = "linux-tray"), not(feature = "hyprland")))]
                {
                    // tray::setup_tray above is the blocking loop; nothing to do.
                }
            }
        }

        println!("Monitor stopped, exiting.");
        Ok(())
    }
}

/// Fire a one-shot `notify-send` hint on GNOME sessions with no window backend
/// (PLATFORMS.md §8.4). Guarded by `$XDG_CURRENT_DESKTOP` containing "GNOME"
/// (case-insensitive). Because this is only called from the no-backend `Err`
/// branch (entered at most once per process), the one-shot is automatic — no
/// dedup state needed. Reuses `platforms::notify` (the existing notify-send
/// shell-out that swallows failure).
fn maybe_gnome_first_run_notify(verbose: bool) {
    let gnome = std::env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_uppercase().contains("GNOME"))
        .unwrap_or(false);
    if gnome {
        if verbose {
            println!("GNOME session with no window backend — firing one-shot extension hint");
        }
        crate::platforms::notify(
            "QMKonnect needs the GNOME Shell extension",
            "Window detection requires the QMKonnect GNOME Shell extension — install it from extensions.gnome.org (see docs).",
        );
    }
}

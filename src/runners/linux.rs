use crate::platforms;
use crate::runners::PlatformRunner;
use std::error::Error;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};

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

        // First-run login autostart for binary-only installs (Scoop /
        // cargo-binstall / generic tarball) with no system-package postinst to
        // install /etc/xdg/autostart. Idempotent + marker-gated; a safe no-op
        // for packaged installs (LINUX.md §6.3).
        crate::platforms::ensure_xdg_autostart(self.verbose);

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

        // PLATFORMS.md §8.4: on a GNOME session where the Shell extension is
        // missing, fire a one-shot hint — REGARDLESS of which backend ends up
        // selected (AT-SPI may win selection and the hint must still fire).
        // Idempotent per process via the AtomicBool guard below. This runs BEFORE
        // backend selection so it covers BOTH the Ok and Err branches.
        maybe_gnome_first_run_notify(self.verbose);

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
                // already running above. Keep main alive. PLATFORMS.md §6. (The
                // §8.4 GNOME one-shot extension hint already fired above, before
                // backend selection.)
                eprintln!(
                    "No Linux window backend available; running tray + device pipeline only. ({e})"
                );

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

/// §8.4 one-shot guard: the GNOME extension hint fires at most once per
/// process. Fire-once-and-stay (never re-armed) — unlike
/// `linux_tray.rs::NO_MODULE_NOTIFIED`, which re-arms on a state transition;
/// PLATFORMS.md §8.4 is strictly "at most once per launch", period.
static GNOME_FIRST_RUN_FIRED: AtomicBool = AtomicBool::new(false);

/// `true` iff `$XDG_CURRENT_DESKTOP` contains `GNOME` (case-insensitive).
/// Pure so it is unit-testable. PLATFORMS.md §8.4. (Real Ubuntu sets
/// `ubuntu:GNOME`; GNOME Flashback/Fedora set `GNOME`/`GNOME-classic`.)
fn gnome_session() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_uppercase().contains("GNOME"))
        .unwrap_or(false)
}

/// Consume the §8.4 one-shot: returns `true` the first time it is called on a
/// GNOME session this process, `false` thereafter. Pure except for the
/// AtomicBool mutation (which is exactly what we test). Does NOT touch D-Bus
/// or `notify-send` — that keeps the once-per-launch invariant hermetically
/// testable without shelling out or hitting the session bus.
fn consume_gnome_hint_shot(flag: &AtomicBool) -> bool {
    if flag.swap(true, Ordering::SeqCst) {
        return false; // already fired this launch
    }
    gnome_session()
}

/// Fire a one-shot `notify-send` hint on GNOME sessions where the Shell
/// extension is missing (PLATFORMS.md §8.4). Fires when: GNOME session AND
/// the well-known name `io.mulletware.QMKonnect` is NOT owned on the session
/// bus (reuses `gnome::probe_available`, Ok ⇔ name owned ⇔ extension
/// installed+enabled). Idempotent per process via [`GNOME_FIRST_RUN_FIRED`],
/// so it is safe to call from any startup path — it now runs before backend
/// selection, covering both the `Ok` and `Err` branches. Reuses
/// `platforms::notify` (the existing notify-send shell-out that swallows
/// failure).
fn maybe_gnome_first_run_notify(verbose: bool) {
    if !consume_gnome_hint_shot(&GNOME_FIRST_RUN_FIRED) {
        return;
    }
    // GOTCHA-3: skip when the GNOME backend isn't compiled in — pointing a
    // user at the extension is misleading if no client can consume it (the
    // exotic --no-default-features trayless service build skips `gnome`).
    #[cfg(not(feature = "gnome"))]
    {
        let _ = verbose;
        return;
    }
    // GOTCHA-4: reuse S1's probe (Ok ⇔ name owned ⇔ extension installed+enabled).
    #[cfg(feature = "gnome")]
    {
        if crate::platforms::gnome::probe_available(false).is_ok() {
            return; // extension present — nothing to hint
        }
        if verbose {
            println!("GNOME session without the Shell extension — firing one-shot hint");
        }
        crate::platforms::notify(
            "QMKonnect needs the GNOME Shell extension",
            "Window detection needs the QMKonnect GNOME Shell extension — install it \
             from extensions.gnome.org (see the QMKonnect docs).",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_xdg(value: Option<&str>, f: impl FnOnce()) {
        let snap = std::env::var("XDG_CURRENT_DESKTOP").ok();
        match value {
            Some(v) => std::env::set_var("XDG_CURRENT_DESKTOP", v),
            None => std::env::remove_var("XDG_CURRENT_DESKTOP"),
        }
        f();
        match snap {
            Some(v) => std::env::set_var("XDG_CURRENT_DESKTOP", v),
            None => std::env::remove_var("XDG_CURRENT_DESKTOP"),
        }
    }

    #[test]
    fn gnome_session_detects_bare_gnome() {
        with_xdg(Some("GNOME"), || assert!(gnome_session()));
    }

    #[test]
    fn gnome_session_detects_ubuntu_gnome() {
        // Real Ubuntu default: colon-separated desktop list.
        with_xdg(Some("ubuntu:GNOME"), || assert!(gnome_session()));
    }

    #[test]
    fn gnome_session_case_insensitive() {
        with_xdg(Some("gnome"), || assert!(gnome_session()));
    }

    #[test]
    fn gnome_session_rejects_non_gnome() {
        with_xdg(Some("KDE"), || assert!(!gnome_session()));
    }

    #[test]
    fn gnome_session_unset_is_false() {
        with_xdg(None, || assert!(!gnome_session()));
    }

    #[test]
    fn gnome_session_empty_is_false() {
        with_xdg(Some(""), || assert!(!gnome_session()));
    }

    #[test]
    fn consume_gnome_hint_shot_is_one_shot() {
        with_xdg(Some("ubuntu:GNOME"), || {
            let flag = AtomicBool::new(false);
            assert!(consume_gnome_hint_shot(&flag)); // first call proceeds
            assert!(!consume_gnome_hint_shot(&flag)); // second call is a no-op
            assert!(!consume_gnome_hint_shot(&flag)); // …and stays that way
        });
    }

    #[test]
    fn consume_gnome_hint_shot_false_when_not_gnome() {
        with_xdg(Some("KDE"), || {
            let flag = AtomicBool::new(false);
            assert!(!consume_gnome_hint_shot(&flag)); // not GNOME → never proceeds
        });
    }
}

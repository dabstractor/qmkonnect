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
        // and returns `Err` when none is available. We do NOT propagate that
        // `Err` with `?` — a total probe failure is usually TEMPORARY: the
        // service can start before the graphical session exists (systemd
        // linger starts the user manager — and every `WantedBy=default.target`
        // service — at BOOT, before anyone logs in; the udev
        // `SYSTEMD_USER_WANTS` trigger fires at pre-login coldplug of an
        // already-plugged keyboard; and under a display manager `default.target`
        // races the compositor). In that window no $WAYLAND_DISPLAY / $DISPLAY /
        // $HYPRLAND_INSTANCE_SIGNATURE has been imported into the user-manager
        // environment yet, so EVERY probe fails. A one-shot probe there left the
        // app backendless until a manual `systemctl --user restart` after the
        // session's `dbus-update-activation-environment` import (the reboot
        // bug); `acquire_backend_with_backoff` re-probes until the session
        // shows up instead (PLATFORMS.md §6 "No-backend fallback").
        let mut consecutive_start_failures: u32 = 0;
        loop {
            let mut monitor = match platforms::create_monitor(self.verbose) {
                Ok(monitor) => monitor,
                Err(first_err) => {
                    // Loud first notice — the tray + device pipeline is already
                    // running above, so this is a degradation notice, not a
                    // fatal one. (The §8.4 GNOME one-shot extension hint
                    // already fired above, before backend selection.)
                    eprintln!(
                        "No Linux window backend available yet ({first_err}); running tray \
                         + device pipeline only, re-probing until the session is up…"
                    );
                    acquire_backend_with_backoff(self.verbose)
                }
            };
            let backend = monitor.platform_name().to_owned();
            if self.verbose {
                println!("Using platform: {backend}");
            }

            if monitor.start_blocks_calling_thread() {
                // Hyprland: start() blocks the calling thread on the IPC
                // event listener. There is no GUI loop to run alongside it.
                // It only returns when the listener gives up (compositor
                // death etc.) — re-acquire in-process rather than exit:
                // a crash-flapping process would otherwise hit systemd's
                // StartLimitBurst and leave the service DEAD until a
                // manual restart.
                match monitor.start() {
                    Ok(()) => println!("Monitor '{backend}' stopped."),
                    Err(e) => eprintln!("Monitor error ('{backend}'): {e}"),
                }
                eprintln!(
                    "Window backend '{backend}' exited; re-acquiring a window backend…"
                );
                monitor_reacquire_cooldown(&mut consecutive_start_failures);
                continue;
            }

            // Spawn-and-return backends (X11, and the foreign-toplevel /
            // GNOME / AT-SPI): start() hands off to its own worker thread(s) —
            // which own reconnect themselves — and returns. `Err` here means
            // the backend never came up: re-acquire (previously this logged
            // once and main parked forever behind a blind window pipeline).
            match monitor.start() {
                Ok(()) => break, // healthy: the monitor's own threads take over
                Err(e) => {
                    eprintln!(
                        "Window backend '{backend}' failed to start: {e}; re-acquiring…"
                    );
                    monitor_reacquire_cooldown(&mut consecutive_start_failures);
                }
            }
        }

        // When the SNI tray is enabled there is no blocking GUI loop
        // to drive (ksni owns its D-Bus thread): park main until Ctrl+C /
        // SIGTERM (the handler above exits the process). The diverging tail
        // satisfies the fn's Result return via never-type coercion. Under
        // not(linux-tray) AND not(hyprland), tray::setup_tray above already
        // drove a blocking loop — fall through and return normally.
        #[cfg(feature = "linux-tray")]
        {
            if self.verbose {
                println!("StatusNotifierItem tray active; blocking on main thread");
            }
            loop {
                std::thread::park();
            }
        }
        #[cfg(not(feature = "linux-tray"))]
        {
            println!("Monitor stopped, exiting.");
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Backend re-acquisition — the boot race (PLATFORMS.md §6 "No-backend
// fallback"). The service can start before the graphical session exists
// (linger boot start, udev SYSTEMD_USER_WANTS coldplug, or default.target
// racing the display manager), when no session env vars have been imported
// into the user-manager environment yet. Total probe failure is therefore
// usually "too early", not "never": re-probe with backoff instead of parking
// forever (the old behavior needed a manual `systemctl --user restart` after
// every reboot).
// ---------------------------------------------------------------------------

/// Delay before the FIRST backend re-probe (ms). The boot/login race is
/// seconds-scale (DM → compositor → `dbus-update-activation-environment`
/// import), so retry briskly at first, then back off.
const BACKEND_RETRY_INITIAL_MS: u64 = 2_000;
/// Backoff cap (ms). A linger-enabled machine can sit pre-login for minutes
/// to hours; at cap each probe is a handful of env reads + socket connects,
/// so a 30 s poll is negligible.
const BACKEND_RETRY_MAX_MS: u64 = 30_000;
/// Non-verbose heartbeat: re-log the persistent failure every N attempts so a
/// long wait stays visible in the journal without per-attempt spam (at the
/// 30 s cap this is one line every ~5 minutes).
const BACKEND_RETRY_HEARTBEAT_EVERY: u32 = 10;

/// Delay before re-probe attempt `attempt` (1 = the first re-probe after the
/// initial loud failure): `2s, 4s, 8s, 16s, 30s, 30s, …` (capped). Pure —
/// unit-tested below.
fn backend_retry_delay_ms(attempt: u32) -> u64 {
    // Clamp the shift so absurd attempt counts (a linger box waiting hours)
    // can neither overflow nor panic: 2s ≪ 16 doublings to reach any cap.
    let shift = attempt.saturating_sub(1).min(16);
    BACKEND_RETRY_INITIAL_MS
        .saturating_mul(1u64 << shift)
        .min(BACKEND_RETRY_MAX_MS)
}

/// True on heartbeat attempts — every [`BACKEND_RETRY_HEARTBEAT_EVERY`]th.
/// Pure — unit-tested below.
fn is_heartbeat_attempt(attempt: u32) -> bool {
    attempt % BACKEND_RETRY_HEARTBEAT_EVERY == 0
}

// ---------------------------------------------------------------------------
// Monitor re-acquisition cooldown — the failed-start counterpart of
// `acquire_backend_with_backoff` (which covers total probe failure). A
// backend that is ACQUIRED but fails to run used to leave the service either
// dead (blocking `start()` error → process exit → systemd `StartLimitBurst=5`
// can permanently stop the unit after flapping) or blind (spawn-style
// `start()` error → logged once, main parked forever behind a live tray over
// a dead window pipeline). Both paths now re-acquire after this cooldown.
// ---------------------------------------------------------------------------

const MONITOR_REACQUIRE_INITIAL_MS: u64 = 2_000;
const MONITOR_REACQUIRE_MAX_MS: u64 = 30_000;

/// Delay before re-acquiring a window backend after `consecutive_failures`
/// back-to-back failed starts (1 = the first failure): `2s, 6s, 18s, 30s, 30s, …`
/// (×3 growth off a 2 s base, capped — mirrors the backends' internal
/// reconnect curves: wayland_ft.rs / hyprland.rs GOTCHA-7). Pure —
/// unit-tested below. The caller's counter is a CONSECUTIVE-failure streak:
/// the acquire-and-run loop breaks out on the first healthy start, so the
/// streak only ever grows within one failure streak and is implicitly reset
/// by that break.
fn monitor_reacquire_delay_ms(consecutive_failures: u32) -> u64 {
    let n = consecutive_failures.saturating_sub(1).min(3);
    MONITOR_REACQUIRE_INITIAL_MS
        .saturating_mul(3u64.saturating_pow(n))
        .min(MONITOR_REACQUIRE_MAX_MS)
}

/// Sleep before the next acquisition attempt and bump the failure streak.
fn monitor_reacquire_cooldown(consecutive_failures: &mut u32) {
    *consecutive_failures = consecutive_failures.saturating_add(1);
    std::thread::sleep(std::time::Duration::from_millis(
        monitor_reacquire_delay_ms(*consecutive_failures),
    ));
}

/// Re-probe `create_monitor` with backoff until a backend is available, then
/// return it, logging the self-heal. The caller (`run`) has already logged
/// the first failure loudly. Sleeps on the calling thread — Ctrl+C / SIGTERM
/// still exit via the handler installed in `run()`. Never returns `Err`: the
/// loop is unbounded by design (a machine may sit pre-login under linger for
/// hours; the probes are cheap).
fn acquire_backend_with_backoff(verbose: bool) -> Box<dyn platforms::WindowMonitor> {
    let mut attempt: u32 = 0;
    loop {
        attempt += 1;
        std::thread::sleep(std::time::Duration::from_millis(
            backend_retry_delay_ms(attempt),
        ));
        match platforms::create_monitor(verbose) {
            Ok(monitor) => {
                println!(
                    "Window backend '{}' became available on re-probe attempt {attempt}; \
                     starting window monitoring.",
                    monitor.platform_name()
                );
                return monitor;
            }
            Err(e) => {
                if verbose {
                    println!("Backend re-probe attempt {attempt}: still none ({e})");
                } else if is_heartbeat_attempt(attempt) {
                    eprintln!(
                        "Still no Linux window backend after {attempt} re-probes (last: {e}); \
                         still trying…"
                    );
                }
            }
        }
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

    // ------------- backend re-probe backoff (PLATFORMS.md §6 boot race) --------------

    #[test]
    fn backend_retry_delay_doubles_from_initial() {
        assert_eq!(backend_retry_delay_ms(1), BACKEND_RETRY_INITIAL_MS);
        assert_eq!(backend_retry_delay_ms(2), 2 * BACKEND_RETRY_INITIAL_MS);
        assert_eq!(backend_retry_delay_ms(3), 4 * BACKEND_RETRY_INITIAL_MS);
    }

    #[test]
    fn backend_retry_delay_caps_at_max() {
        // 2s, 4s, 8s, 16s, 30s (cap hits at attempt 5), 30s, …
        assert_eq!(backend_retry_delay_ms(4), 16_000);
        assert_eq!(backend_retry_delay_ms(5), BACKEND_RETRY_MAX_MS);
        assert_eq!(backend_retry_delay_ms(6), BACKEND_RETRY_MAX_MS);
        // A linger box can wait hours pre-login: absurd attempt counts must
        // neither overflow nor panic — they pin at the cap.
        assert_eq!(backend_retry_delay_ms(1_000_000), BACKEND_RETRY_MAX_MS);
        assert_eq!(backend_retry_delay_ms(u32::MAX), BACKEND_RETRY_MAX_MS);
    }

    #[test]
    fn heartbeat_is_every_tenth_attempt() {
        for attempt in 1..=21u32 {
            assert_eq!(
                is_heartbeat_attempt(attempt),
                attempt % BACKEND_RETRY_HEARTBEAT_EVERY == 0,
                "attempt {attempt}"
            );
        }
        assert!(!is_heartbeat_attempt(1)); // the loud first notice covers it
        assert!(is_heartbeat_attempt(10));
        assert!(is_heartbeat_attempt(20));
    }

    // ------------- monitor re-acquisition cooldown (failed starts) --------------

    #[test]
    fn monitor_reacquire_delay_grows_by_three_then_caps() {
        assert_eq!(monitor_reacquire_delay_ms(1), 2_000);
        assert_eq!(monitor_reacquire_delay_ms(2), 6_000);
        assert_eq!(monitor_reacquire_delay_ms(3), 18_000);
        assert_eq!(monitor_reacquire_delay_ms(4), MONITOR_REACQUIRE_MAX_MS);
        assert_eq!(monitor_reacquire_delay_ms(5), MONITOR_REACQUIRE_MAX_MS);
        // A long failure streak must neither overflow nor panic — it pins at
        // the cap.
        assert_eq!(monitor_reacquire_delay_ms(u32::MAX), MONITOR_REACQUIRE_MAX_MS);
    }

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


# QMKonnect — Remaining Issues

From the repo audit on 2026-07-09. Already fixed in the working tree (not listed below): config VID/PID wired into the HID send path, dead per-notification HID enumeration removed, `-l/--list` implemented, debounced-send verbose fix, dead `"empty"` branch removed, duplicated `create_config` cfg blocks collapsed, unused deps (`hidapi`, `block`, `anyhow`) removed, `WindowState` Clone derived.

## Critical

### 1. macOS target does not compile
`src/platforms/macos.rs` has three independent compile errors that Linux builds can't catch (cfg-gated):
- Imports `crate::core::{QMKError, QMKResult}` and `crate::core::errors::PlatformError`, but `src/core/mod.rs` never declares the `errors` module.
- `WindowMonitor::start()` is implemented returning `QMKResult<()>`, which doesn't match the trait's `Result<(), Box<dyn Error>>`.
- `macos.rs:264` — `.map_err()` called on an `Option` inside an unbalanced `Ok(...)` (type/syntax error).

Minimal fix: revert macos.rs error handling to `Box<dyn Error>` returns. Must be verified on a Mac.

### 2. Orphaned error-handling modules
`src/core/errors.rs` (292 lines) and `src/core/validation.rs` (338 lines) from commit 39c7c78 are never declared in `core/mod.rs`, so they are never compiled. Either wire them in (and fix macOS against them) or delete them. Currently they look like safety infrastructure but provide nothing.

### 3. `panic = "abort"` defeats panic recovery
The release profile aborts on panic, making every `panic::catch_unwind` supervisor in `src/runners/linux.rs` dead code in release builds. Decision: keep `abort` and delete the catch_unwind scaffolding (rely on systemd `Restart=always` — fine for this app), or switch to `unwind`.

### 4. udev update path is fragile and insecure
`src/platforms/linux.rs::update_udev_rules`:
- Writes new rules to the fixed, predictable path `/tmp/99-qmkonnect.rules.tmp`, then `sudo mv`s to `/etc/udev/rules.d/` — a local attacker can race the window to install arbitrary udev rules as root.
- `sudo` won't work from a systemd service or GUI context (no TTY).
- The rule template uses `MODE="0666"` (world-writable hidraw node); `TAG+="uaccess"` is the modern, safer mechanism.

With config-driven VID/PID now working, consider whether per-user udev rewriting is needed at all — a static rule matching the QMK raw HID usage page removes the whole sudo dance.

## Correctness / resource management

### 5. `static mut` data races (UB)
- `src/platforms/windows.rs`: `G_VERBOSE`, `G_HOOK`, `LAST_WINDOW_INFO` are mutated/read from both the hook callback and the polling thread.
- `src/platforms/macos.rs`: `static mut VERBOSE`.

Real undefined behavior; rustc edition 2024 hard-errors on these patterns. Replace with `AtomicBool` / `Mutex` / `OnceLock`.

### 6. Pre-existing test failures
`test_debounce_subsequent_messages` and `test_multiple_rapid_updates` fail on unmodified HEAD. The tests assume a ~600ms debounce window; `DEBOUNCE_INTERVAL` is 50ms. Update the tests to match the current design (or restore the intended interval). Tests must run with `cargo test --bin qmkonnect -- --test-threads=1` (shared global debouncer state).

### 7. Hyprland reconnect backoff never resets
In `src/platforms/hyprland.rs::start`, `delay_ms` grows across reconnects but is never reset after a successful reconnect, so long-uptime sessions eventually wait the full 10s cap on every reconnect.

### 8. Hyprland polling thread is likely redundant
A thread polls `Client::get_active()` every 100ms forever (10 IPC round-trips/sec) even though the event listener already registers `layer_opened/closed` handlers — which is what polling was added for (scratchpads). Try event-only; if a scratchpad case regresses, poll briefly after layer events instead of continuously.

### 9. Windows settings dialog leaks an `Arc` per open
`src/tray.rs::settings_dialog_proc` uses an `Arc::into_raw` + `mem::forget` dance. Replace the shared-state scheme with a `static Mutex`.

### 10. Windows service shutdown is a no-op + forced exit
`src/service.rs`: `run_service_impl` "stores" the monitor in `SERVICE_CONTEXT` but the slot is never populated, so the stop-the-monitor block does nothing; a detached thread force-`exit(0)`s 5 seconds after stop regardless of thread state.

### 11. Debounce timer spawns a thread per burst
Minor at this scale; a single long-lived timer thread or a `CondVar` wait would be cleaner.

## OS integration

### 12. Windows service mode: consider deleting it
- Services run in session 0 and cannot show UI, yet the service spawns a tray thread (wasted).
- `sc create` uses `depend=Tcpip` (no TCP dependency exists) and the `binPath=` quoting (`binPath="...exe" --service` as a single arg) is the classic sc.exe quoting trap.
- `qmkonnect_service_failure_analysis.md` documents the mode being troublesome.

The tray app + Run-key autostart covers the use case; deleting service mode removes ~450 lines (service.rs + sc plumbing).

### 13. macOS screen-recording permission handling
`CGWindowListCopyWindowInfo` (window titles) requires screen recording permission, and `start()` hard-fails while the permission dialog is still on screen. Degrade gracefully: send app name only when permission is missing, keep running, pick up titles once granted.

### 14. X11 monitor is a stub sending garbage
`src/platforms/x11.rs` sends the literal strings `"X11Application"/"Active Window"` or `"Linux"/"Desktop"`. Either implement it (`_NET_ACTIVE_WINDOW` → `WM_CLASS` / `_NET_WM_NAME`) or make it fail loudly instead of pretending to work.

### 15. `check_hyprland_environment` unsoundness and shelling out
Calls `env::set_var` from a threaded context (unsound in edition 2024) and shells out to `ps` to detect Hyprland. Trust the socket scan and drop both.

## Architecture unification (roadmap)

The platform divergence stems from one constraint: each OS disagrees about who owns the main thread (hyprland: blocking socket listener, no GUI loop; macOS: CFRunLoop + tao both want main; Windows: `WINEVENT_OUTOFCONTEXT` hooks need a pumped message loop, which the tao tray loop provides). Unify by:

1. **Make every monitor non-blocking and event-pushing** — `start()` spawns the platform listener and pushes `WindowInfo` into a channel/callback; never blocks.
2. **One host loop owns the main thread** — the tao event loop where a tray exists (it is simultaneously the Win32 message pump and the macOS run loop, eliminating the Windows polling fallback and the separate CFRunLoop thread); a plain park loop on hyprland.
3. **Collapse the duplication that falls out** — single `WindowMonitor: Send` trait (the non-`Send` variant exists only because hyprland's blocking `start()` stores the listener; stop storing it), one generic runner instead of three (`runners/{linux,macos,windows}.rs`), debouncer stays in core.

The only genuinely irreducible platform constraint: macOS/Windows GUI loops must be on the main thread and macOS observers need a pumped run loop. Everything else (dual traits, three runners, polling threads) is incidental.

## Config UX

### 16. Misconfigured VID/PID fails silently
`notify()` swallows device-not-found after 3 retries and returns `Ok` (deliberate, to avoid service restart loops) — reasonable for transient unplugs, terrible for diagnosing a typo'd VID. Do one device probe at startup with the configured IDs and print a clear "no device matching 0xXXXX:0xXXXX found — check config.toml" message.

### 17. Add `--list-devices`
`qmk_notifier::list_hid_devices()` is already exported; a passthrough flag lets users discover their VID/PID without external tools.

### 18. Arch `post_install` interactive prompts
`packaging/linux/arch/qmkonnect.install` prompts for IDs via stdin — pacman hooks aren't reliably interactive (breaks under AUR helpers), and `logname` fails in some contexts. With config-driven IDs working, stop prompting and print "edit ~/.config/qmk-notifier/config.toml" instead.

### 19. Config directory naming inconsistency
Linux uses `qmk-notifier/`, Windows/macOS use `QMKonnect/`. Pick one (keeping `qmk-notifier` on Linux preserves existing installs; document the difference otherwise).

### 20. Linux tray builds have no settings UI
The tray settings dialog exists for Windows/macOS only; X11 builds get a "not yet implemented" println. Fine to leave file-only, but the menu item shouldn't dead-end silently.

## Housekeeping

### 21. Delete leftover source files
`src/tray_broken.rs` (957 lines), `src/tray_clean.rs`, `src/tray_rs_backup` (all untracked), and strip the `DEBUG: CRASH INVESTIGATION` printlns throughout `src/tray.rs`.

### 22. Remove committed build artifacts
Tracked in git: `QMK-Window-Notifier-Setup.msi`, `QMK-Window-Notifier-Setup.wixpdb`, `installer.wixobj` (repo root), `packaging/linux/arch/pkg/**` (an entire extracted package tree including the compiled binary — why `git status` is always dirty), `packaging/linux/arch/qmkonnect-0.1.0-1-x86_64.pkg.tar.zst`, `packaging/macos/QMKonnect.dmg`, `packaging/windows/QMK-Window-Notifier-Setup.msi`, `docs/_site/`. Add to `.gitignore` and `git rm --cached`; .git is 16MB for a 5k-line project.

### 23. Stray files
- `query` at repo root (contains just "QMKonnect").
- `qmkonnect_service_failure_analysis.md` — resolved incident report; archive or delete.

### 24. Cargo.toml / PKGBUILD metadata
- `authors = ["Your Name <your.email@example.com>"]` in Cargo.toml and `# Maintainer: Your Name` in PKGBUILD.
- `readme = "README.md"` but the file is `Readme.md`.
- `log`/`env_logger` listed twice (common deps + Windows target deps).
- `toml = "0.5"` is old (current 0.9); low urgency.

### 25. Help text and dead trait scaffolding
- `src/core/config.rs` `ConfigManager` trait + `create_config_manager()` are `#[allow(dead_code)]` and entirely unused — main.rs duplicates the logic inline. Either adopt the trait or delete it.
- `WindowMonitor::stop()` is `#[allow(dead_code)]` on most platforms; unify or drop from the trait.

# Gap Analysis & Known Issues — QMKonnect

> Synthesized from: REMAINING_ISSUES.md (repo audit), scout agent deep-dive of
> all source files, and PRD §12 (Beta Status & Future Work).
> The codebase is at v0.2.8; PRD specifies v0.2.4.

## Status Assessment

All 10 PRD features (F1-F10) are **implemented and functional**. All 33 tests
pass. The gaps below are code-quality, robustness, and future-work items, not
missing features.

## Verified Gaps (from source code analysis)

### Medium Severity

| ID | Location | Finding |
|----|----------|---------|
| G5 | `notifier.rs:204-211` | **Debounce interval not live.** `DebounceState.interval` is read from `configured_debounce_ms()` **once** at static init and never refreshed. Config edits to `debounce_ms` require a restart, contradicting the "hot config" promise elsewhere. |
| G6 | `notifier.rs:77-125, 127-140` | **Matching predicate triplicated.** The VID/PID/usage matching logic is inlined identically in `startup_device_probe`, `is_device_connected`, and `qmk_notifier::device_matches`. Should extract `DeviceFilter::matches()`. |
| G1 | `main.rs:48-52` | **Non-Windows logging is a no-op.** `init_logging()` on macOS/Linux returns `Ok(())` without installing a logger. `log::*` macros silently drop output. Only `eprintln!`/`println!` verbose prints work. |
| P-X11 | `platforms/mod.rs:81-99` | **X11 `list_foreground_windows` returns empty.** The SNI tray "Show Window Information" reports nothing on the X11+`linux-tray` build. Data is obtainable via `xprop -root` but not wired. |
| P-WinConsole | `runners/windows.rs:40-52` | **Console-mode WinEventHook is dead.** `run_console_mode` never pumps Win32 messages, so `WINEVENT_OUTOFCONTEXT` callbacks never fire. Detection depends solely on the 100ms poller. |

### Low Severity

| ID | Location | Finding |
|----|----------|---------|
| G3 | `core/mod.rs:80-92` | No config validation. `debounce_ms`/`poll_interval_ms` accept any `u64` without range checks. |
| G4 | `core/mod.rs:15` | `#[derive(Default)]` on Config yields `debounce_ms=0` (serde defaults not invoked by `Default`). `Config::default()` ≠ parsed empty file. |
| G7 | `notifier.rs:152-159` | `verbose` hard-coded to `false` in `QmkNotifier::notify`'s `RunParameters`. Intentional but undocumented. |
| G8 | `types.rs:7` | `WindowInfo` derives `Debug, PartialEq` only. Missing `Eq`, `Clone`, `Default`. |
| G9 | `notifier.rs:188,204,212,286` | Uses `once_cell::sync::Lazy`; std `LazyLock` (stable 1.80) would drop the dependency. |
| P-MacDeadState | `macos.rs:50,142,180` | `running: bool` field on `MacOSMonitor` is dead state (written twice, never read). `stop()` can't halt `CFRunLoopRun`. |
| P-MacNoFilter | `macos.rs` | macOS has no `should_ignore_window`. Filtering is implicit via `activationPolicy == Regular`. |

### Packaging / Integration Gaps

| ID | Location | Finding |
|----|----------|---------|
| PKG-SvcPath | `service.template` vs `docs/installation.md` | Manual install copies to `/usr/local/bin/` but service expects `/usr/bin/qmkonnect`. Service fails with status=203/EXEC. |
| PKG-TplVestige | `qmkonnect.service.template` | `.template` suffix is vestigial (no substitution). `post_install` copies verbatim. Misleading naming. |
| PKG-DualWin | `install.ps1` + `QMKonnect.iss` | Two parallel per-user installers for the same target. Different ARP entries. Mixed install paths create stale/orphan entries. |
| PKG-MacVer | `build.sh` (macOS) | No `CFBundleShortVersionString`/`CFBundleVersion` in generated `Info.plist`. Version not read from `Cargo.toml` (unlike Windows scripts). |
| PKG-UdevPath | `69-qmkonnect-rawhid.rules` | Hardcoded `/usr/lib/udev/qmkonnect-hid-id` helper path. Distros using `/lib/udev/` won't find it. |

## REMAINING_ISSUES.md — Already Fixed (confirmed by code review + git log)

| Issue | Status | Evidence |
|-------|--------|----------|
| #1 macOS compile errors | ✅ Fixed | Code compiles; uses `Box<dyn Error>` consistently |
| #2 Orphaned error modules | ✅ Fixed | `core/errors.rs` and `core/validation.rs` removed |
| #3 `panic="abort"` defeats catch_unwind | ✅ Resolved | catch_unwind removed; systemd `Restart=always` used |
| #4 udev update path insecure | ✅ Fixed | Atomic writes via `NamedTempFile`, no `/tmp` race, no `sudo mv`, uses `pkexec` |
| #5 `static mut` data races | ✅ Fixed | All replaced with `AtomicBool`/`AtomicIsize`/`Mutex` |
| #6 Test failures | ✅ Fixed | All 33 tests pass; debounce window widened in tests |
| #7 Hyprland backoff never resets | ✅ Fixed | `STABLE_CONNECTION_THRESHOLD` resets backoff |
| #8 Hyprland redundant polling | ✅ Fixed | Replaced with poll burst after layer events |
| #9 Windows settings dialog Arc leak | ✅ Fixed | Uses `Mutex<Option<...>>` instead |
| #10 Windows service shutdown no-op | ✅ Resolved | Service mode deleted entirely (tray app only) |
| #11 Debounce timer per-burst thread | ✅ Fixed | Single long-lived worker thread via `Lazy<JoinHandle>` |
| #12 Windows service mode | ✅ Resolved | Service mode deleted (~450 lines removed) |
| #13 macOS screen-recording handling | ✅ Fixed | `ensure_screen_recording_permission` degrades gracefully |
| #14 X11 monitor stub | ✅ Fixed | Real `_NET_ACTIVE_WINDOW`/`WM_CLASS` implementation |
| #15 check_hyprland_environment unsoundness | ✅ Fixed | No `env::set_var` from threads, no `ps` shell-out |
| #16 Misconfigured VID/PID silent | ✅ Fixed | `startup_device_probe` at startup |
| #17 `--list-devices` | ✅ Fixed | `list_devices()` implemented |
| #18 Arch post_install interactive prompts | ✅ Fixed | No stdin prompts; prints config path |
| #19 Config dir naming inconsistency | ✅ Documented | Linux `qmk-notifier/` preserved; Windows/macOS `QMKonnect/` |
| #20 Linux tray no settings UI | ✅ Fixed | zenity `--forms` settings dialog |
| #21 Leftover source files | ✅ Fixed | `tray_broken.rs`, `tray_clean.rs`, etc. removed |
| #22 Committed build artifacts | ✅ Fixed | gitignored and `git rm --cached` |
| #23 Stray files | ✅ Fixed | `query`, service failure analysis removed |
| #24 Cargo.toml metadata | ✅ Fixed | Authors, readme path, duplicate deps corrected |
| #25 Dead trait scaffolding | ✅ Fixed | Unused traits removed |

## PRD §12 — Future Work (Roadmap)

| Item | Status | Notes |
|------|--------|-------|
| Broader Wayland + X11 support | Not started | Hyprland-only today; X11 fallback exists |
| Code signing / notarization | Not started | Unsigned (Win) / ad-hoc (macOS); build scripts support Developer ID |
| Richer cross-platform UI | Not started | Native per-platform today (Win32/Cocoa/zenity+GTK) |
| Architecture unification | Not started | Three near-duplicate runners + dual-trait design; roadmap to unify |
| Multi-keyboard management | Not started | VID/PID disambiguation exists; richer UX is future |
| Windows device-arrival launcher | Not started | Autostart-at-login covers the use case today |
| Host-side window rules (PRP 002) | Draft/Approved | Major feature: `rules.toml` + host-side matching + firmware handshake |

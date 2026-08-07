# System Context & Codebase State Assessment

## Executive Summary

The QMKonnect codebase at v0.2.8 is **feature-complete for F1-F14** (390 tests
passing). The remaining PRD work is **F15: Community Package-Manager Distribution**.

## Codebase State by Feature

### F1-F10: Core Application (COMPLETE)
- **F1** Foreground window detection: `src/platforms/{windows,macos,hyprland,x11}.rs`
- **F2** Raw HID transport: via `qmk-notifier` crate v0.3.0 (git-tagged dep)
- **F3** Auto device discovery: `configured_filter()`, `is_device_connected()`
- **F4** Debounced coalescing: `notify_qmk()` + `DebounceState` (global, Mutex+Condvar)
- **F5** TOML config: `src/core/mod.rs` — Config, parse_config, render_config_body
- **F6** Tray UI: `src/tray.rs` (3147 lines), `src/linux_tray.rs` (1332 lines)
- **F7** Open at Login: `src/autostart.rs` (Windows HKCU), `tray.rs` autostart mod (macOS SMAppService), systemd BindsTo (Linux)
- **F8** Installers: Inno Setup (Windows), Arch PKGBUILD (Linux), DMG build.sh (macOS)
- **F9** Linux udev: `src/platforms/linux.rs`, `src/bin/hid_id.rs`, static rule + config-driven fallback
- **F10** Firmware contract: documented in spec, not implemented in this repo (external)

### F11: Host-Side Window Rules (COMPLETE)
- `src/core/rules.rs` (1411 lines): `RuleSet`, `Rule`, `Pattern`, `parse_rules()`, `evaluate()`, `HostContext`
- `src/core/pattern.rs` (10158 lines): full-parity firmware matcher port with tests
- `notify_qmk()` in `src/core/notifier.rs`: dual-send logic (string + APPLY_HOST_CONTEXT)
  - Stack mode: string first, then context with clear_board=false
  - Replace mode: context only with clear_board=true
  - No-match: string + context{layer:0xFF, clear_board:false}

### F12: Named Callback Registry + Typed Commands (COMPLETE)
- `Notifier::send_command()`: typed-command transport via `qmk_notifier::RunCommand`
- `perform_handshake()` / `perform_handshake_with()`: QUERY_INFO → QUERY_CALLBACK sweep → SET_OS
- `validate_rules_callback_names()`: warns on unknown callback names
- `callback_names()`: global HashMap<String, u8> from handshake
- CLI: `--list-callbacks`, `--validate-rules`, `--rules-path` (all in `src/main.rs`)

### F13: Two-Tier Device Discovery (COMPLETE)
- `classify_devices()`: Tier-1 enumerate + Tier-2 QUERY_INFO probe per candidate
- `DeviceStatus` enum: `Connected(usize)`, `NoModule`, `Disconnected` (three-state)
- `PresenceTracker`: event-driven re-probe on path-set change (plug/unplug)
- `CLASSIFICATION_CACHE`: path→(kind, expiry) with TTL (5s default)
- Discovered-device picker: Win32 LISTBOX (`IDC_DEVICE_LIST`), macOS NSStackView, Linux zenity --list
- `warm_cache_from_handshake()`: handshake → classification cache cross-feed (single-board only)

### F14: VIA Coexistence (COMPLETE)
- R-COEX satisfied by construction: hidapi default open is shared/non-seize on all platforms
- Bounded IN drains (`IN_DRAIN_MAX = 32`) after writes only
- `0x81 0x9F` magic header is the protocol demultiplexer
- Tests assert first payload byte is always `0x81`

### F15: Community Package-Manager Distribution (NOT IMPLEMENTED)
**This is the primary remaining work.** No community channel configs exist:
- **AUR**: PKGBUILD exists at `packaging/linux/arch/PKGBUILD` but not published to AUR
- **Homebrew**: No cask or formula
- **Scoop**: No manifest
- **Winget**: No manifest
- **Nix flake**: No flake.nix
- **mise**: No plugin
- **asdf**: No plugin

## Current CI Pipeline
`.github/workflows/release.yml` builds and publishes to GitHub Releases only:
- macOS: `.dmg` (universal binary, optional notarization)
- Windows: `.exe` (Inno Setup, per-user)
- Linux: binary tarball + `.pkg.tar.zst` (Arch)

No community channel publishing jobs exist.

## Documentation State
- `docs/installation.md`: Has direct installer instructions, mentions "no AUR package"
- `docs/configuration.md`: Config schema documented (no F15 content)
- `README.md`: Basic project info, no distribution channel list
- `docs/llms_full.txt`: Regenerated aggregate (111KB)

## Key Architectural Patterns to Follow
1. **Per-user, no-admin philosophy**: All channels must honor this where possible
2. **Version injection from Cargo.toml**: Single source of truth via `cargo metadata`
3. **Build outputs gitignored**: Never commit installers/packages
4. **release.toml + cargo-release**: Tag-driven, maintainer-controlled releases
5. **Existing PKGBUILD pattern**: The Arch PKGBUILD at `packaging/linux/arch/` is the template for other channels
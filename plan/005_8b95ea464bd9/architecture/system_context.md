# System Context — QMKonnect Device Discovery Delta (Session 005)

## Project Overview
QMKonnect is a cross-platform (Windows/macOS/Linux) menu-bar tray app that sends
the active window's application/title context to a QMK keyboard running the
`qmk_notifier` firmware module. The app uses the `qmk-notifier` Rust crate (v0.3.0,
git dep) as its HID transport, which wraps `hidapi` for device enumeration and
communication.

## Delta Scope
- **F13:** Two-tier device discovery (Tier-1 HID presence + Tier-2 capability probe) → three-state tray status + discovered-device Settings picker.
- **F14:** VIA coexistence guarantee (R-COEX) — assert the already-true shared-open behavior.
- **D3:** `0xFEED` config-template cleanup.
- **D4:** `--list-devices` kind column.
- **D5/D6:** Doc-only / already shipped — no code tasks.

## Key Files (confirmed by research)

| File | Lines | Responsibility |
|------|-------|----------------|
| `src/core/notifier.rs` | ~2816 | HID device management, handshake, debounce, host-rules eval, classification (NEW) |
| `src/core/mod.rs` | ~600 | Config struct, renderers, parser, cache |
| `src/tray.rs` | ~2380 | macOS + Windows tray UI (tray-icon + tao), settings dialogs |
| `src/linux_tray.rs` | ~995 | Linux SNI tray (ksni), settings dialog (zenity) |
| `src/main.rs` | ~565 | CLI dispatch (flat if-chain, no clap) |
| `spec/DEVICE_DISCOVERY.md` | ~450 | Authoritative design doc for F13/F14 |

## Module Dependency Graph
```
config.toml → cached_config() → configured_filter() → DeviceFilter {vid?, pid?, page, usage}
                                                        │
                    ┌───────────────────────────────────┼──────────────────────────┐
                    ▼                                   ▼                          ▼
          is_device_connected()               perform_handshake()            list_devices()
          (Tier-1 enumerate)                  (Tier-2: QUERY_INFO)           (CLI enumerate)
                    │                                   │
                    ▼                                   ▼
          handshake_action(prev, now)         qmk_notifier::run(params)
          → {Gain, Loss, None}                 → CommandResponse::{Info, ...}
                    │                                   │
          Gain → perform_handshake()           Sets HOST_CAPABLE AtomicBool
          Loss → reset_handshake_state()
```

## Build & Test
- **macOS:** `cargo test --bin qmkonnect -- --test-threads=1` (single-threaded — shared mock globals)
- **Windows:** Same test command; `cargo build --release`; run exe directly in user session
- **Linux:** Same test command; Linux SNI tray behind `feature = "linux-tray"`
- All tests share process-wide mock globals (`MOCK_RESPONSES`, `DebounceState`) → MUST be single-threaded.
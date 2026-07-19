# System Context — Host-Side Window Rules (F11/F12)

## Current Architecture

QMKonnect is a cross-platform desktop daemon (v0.2.8, Beta) that detects the
foreground window and streams `{app_class}\x1D{title}` to a QMK keyboard over
USB Raw HID. The keyboard's firmware (qmk-notifier module) pattern-matches the
string and switches layers / invokes callbacks.

### Repository Layout (all three repos local)

| Repo | Path | Role | Current State |
|------|------|------|---------------|
| **qmkonnect** | `/home/dustin/projects/qmkonnect` | Desktop app (this repo) | v0.2.8, no host-rules code |
| **qmk_notifier** | `/home/dustin/projects/qmk_notifier` | Rust transport crate | v0.2.1, string-only API |
| **qmk-notifier** | `/home/dustin/projects/qmk-notifier` | C firmware module | Scaffolded host-state, no typed dispatch |

### Current Data Flow (string-only)

```
platform monitor → WindowInfo{app_class, title}
  → notify_qmk() → format!("{app_class}\x1D{title}")
    → debounce (50ms coalescing)
      → QmkNotifier::notify(msg)
        → qmk_notifier::run(RunParameters::new(SendMessage(msg), vid, pid, page, usage, false))
          → crate appends ETX (0x03), frames into 32-byte reports [0x00, 0x81, 0x9F, payload…]
            → burst-write to cached HID devices → drain IN acks
```

### Key Files & Their Roles

| File | Lines | Role |
|------|-------|------|
| `src/core/notifier.rs` | 604 | Notifier trait, QmkNotifier, debounce state machine, device probes |
| `src/core/mod.rs` | 214 | Config struct, parse_config, render_config_body, timing |
| `src/core/types.rs` | 37 | WindowInfo { app_class, title } |
| `src/main.rs` | ~140 | CLI dispatch (hand-rolled, no clap) |
| `src/tray.rs` | ~500 | macOS/Windows tray (tao + tray-icon + muda) |
| `src/linux_tray.rs` | ~350 | Linux SNI tray (ksni) |
| `src/runners/mod.rs` | ~38 | PlatformRunner trait + create_runner |
| `src/platforms/mod.rs` | ~120 | WindowMonitor trait + dispatchers |
| `src/platforms/linux.rs` | ~300 | udev rules, config paths, root-aware reload |

### What Does NOT Exist Yet (Greenfield)

- ❌ `src/core/pattern.rs` — full-parity matcher port (not created)
- ❌ `src/core/rules.rs` — rules.toml schema and evaluation (not created)
- ❌ Typed-command support in Notifier trait (single `notify(msg)` method only)
- ❌ Capability handshake logic
- ❌ `--list-callbacks`, `--validate-rules`, `--rules-path` CLI flags
- ❌ "Reload rules" tray menu item
- ❌ rules.toml path resolution alongside config.toml

## Planned Data Flow (with host rules)

```
platform monitor → WindowInfo{app_class, title}
  → notify_qmk() → format!("{app_class}\x1D{title}")
    → debounce (50ms coalescing)
      → IF rules.toml present AND device is host-capable (proto_ver==2):
          evaluate rules → layer (first-match) + callbacks (all-match)
          determine stack vs replace per disable_firmware_config
          STACK: send string first → then APPLY_HOST_CONTEXT{clear_board=0}
          REPLACE: send ONLY APPLY_HOST_CONTEXT{clear_board=1}
          NO MATCH: APPLY_HOST_CONTEXT{layer=0xFF, callbacks=empty}
        ELSE (no rules.toml OR legacy firmware):
          send string only (identical to today)
```

## Critical Invariants

1. **GS = 0x1D; ETX = 0x03** — payload delimiter and terminator
2. **Debounce: first send immediate, bursts collapse to one follow-up**
3. **Device matching: usage-page/usage primary, VID/PID optional**
4. **Config re-read every notification/poll** (hot config)
5. **render_config_body is the single config-file writer**
6. **MenuItem is !Send — mutate only on event-loop thread**
7. **Tests are single-threaded** (shared global debouncer state)
8. **Host layers ≥ 224** so they resolve above board layers
9. **0xF0 discriminator** can never begin a real matched string (sanitizer allows 0x20–0x7E)
10. **Legacy firmware never receives typed commands** (handshake gates on proto_ver==2)
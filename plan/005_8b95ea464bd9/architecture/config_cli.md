# Config & CLI Architecture — Current State for D3/D4

## Config Struct — src/core/mod.rs:23-48

```rust
#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub vendor_id: Option<u16>,      // None = auto-discovery
    #[serde(default)]
    pub product_id: Option<u16>,     // None = auto-discovery
    #[serde(default)]
    pub usage_page: Option<u16>,     // None = 0xFF60
    #[serde(default)]
    pub usage: Option<u16>,          // None = 0x61
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,            // default 50
    #[serde(default = "default_poll_interval_ms")]
    pub poll_interval_ms: u64,       // default 0 (Hyprland)
}
```

## 0xFEED Locations — ALL must be cleaned up

### Code

| File | Line(s) | Content |
|------|---------|---------|
| `src/core/mod.rs` | 238-239 | `render_default_config_template`: `# vendor_id = 0xfeed` / `# product_id = 0x0000` |
| `src/core/mod.rs` | 263-266 | `render_config_body` None arms: `# vendor_id = 0xfeed` / `# product_id = 0x0000` |
| `README.md` | 232-233, 305-308 | `# vendor_id = 0xfeed` / `# product_id = 0x0000` in config examples |
| `docs/configuration.md` | 71-72, 182-183 | `# vendor_id = 0xfeed` / `# product_id = 0x0000` in config examples |

### Target
Replace all `0xfeed` literals with `0x????` and the comment "unset: auto-discover
any QMK keyboard (recommended)". Match `CONFIG.md` §2 wording.

**Note:** `render_config_body` (the save renderer) ALSO uses the `0xfeed` literal
in its `None` arms — the PRD only mentions `render_default_config_template`, but
BOTH renderers must be updated.

---

## CLI Dispatch — src/main.rs:79-152

Flat if-chain (no clap). Relevant flags:

| Flag | Handler | Line |
|------|---------|------|
| `-v` / `--verbose` | captured once, threaded | 79 |
| `-c` / `--config` | `create_config()` | 102 |
| `--list-devices` | `core::notifier::list_devices()?` | 115-119 |
| `-l` / `--list` | `print_platforms()` | 109 |
| `--list-callbacks` | `list_callbacks(verbose)` | 135 |
| `--validate-rules` | `validate_rules(rules_path, verbose)` | 143 |
| (fall-through) | `runners::create_runner(verbose)?.run(&args)` | 150 |

## `list_devices()` — src/core/notifier.rs:129-143

```rust
pub fn list_devices() -> Result<(), Box<dyn Error>> {
    let api = hidapi::HidApi::new()?;
    println!("Available HID devices (vendor:product  usage_page:usage  product):");
    for d in api.device_list() {
        println!("  {:#06x}:{:#06x}  {:#06x}:{:#06x}  {}",
            d.vendor_id(), d.product_id(), d.usage_page(), d.usage(),
            d.product_string().unwrap_or(""));
    }
    Ok(())
}
```

**Change target (D4):** After enumeration, run `classify_devices()` once and add a
`kind` column (`qmk_notifier` / `qmk-only` / etc.).

---

## Documentation Files

| File | Content | 0xfeed? | F13/F14 mention? |
|------|---------|---------|------------------|
| `docs/configuration.md` | VID/PID field reference, CLI table | YES (L71-72, L182-183) | NO |
| `docs/installation.md` | Setup guide, VID/PID guidance (L103) | NO | NO |
| `docs/troubleshooting.md` | Config check, custom VID/PID (L130, L156-157) | NO | NO |
| `docs/generate_llms_full.sh` | Concatenates 8 files → `docs/llms_full.txt` | — | — |
| `docs/llms_full.txt` | Generated concat (2803 lines) | YES (from README) | NO |
| `README.md` | Project overview, config examples | YES (L232-233, L305-308) | NO |

### generate_llms_full.sh
Concatenates (in order): README.md, docs/index.md, installation.md,
qmk-integration.md, configuration.md, usage.md, examples.md, troubleshooting.md.
Strips Jekyll front-matter. **Must re-run after any docs/ or README.md edit.**

---

## Settings Dialog Write Path (all three platforms)

All three dialogs follow the same pattern:
1. Read current config via `parse_config(config_path)` (fallback `Config::default()`)
2. Overlay VID/PID from user input
3. Render via `render_config_body(&merged)` — preserves all other fields
4. `atomic_write(config_path, body)`

**Linux extra:** `apply_device_rule(vid, pid)` via `pkexec` for udev rules.
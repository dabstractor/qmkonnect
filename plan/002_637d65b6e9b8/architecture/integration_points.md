# Integration Points — QMKonnect Host-Rules Wiring

## Notifier Trait Extension

### Current (src/core/notifier.rs:12-14)
```rust
pub trait Notifier: Send + Sync {
    fn notify(&self, message: String) -> Result<(), Box<dyn Error + Send + Sync>>;
}
```

### Required Extension
The trait needs methods to send typed commands and receive `CommandResponse`.
The mock must be able to record call ordering (string-before-context).

Approach: add a `send_command` method that accepts `qmk_notifier::RunCommand`
and returns `qmk_notifier::CommandResponse`. The `notify()` method stays as-is
for backward compatibility (string-only path). The new debounce send logic in
`notify_qmk()` calls `send_command()` for typed operations.

## DebounceState Extension

### Current (src/core/notifier.rs:193-202)
```rust
struct DebounceState {
    last_sent_time: Option<Instant>,
    pending: Option<String>,
    verbose: bool,
    interval: Duration,
}
```

### Required Changes
The debounce state needs to carry the full WindowInfo (not just the formatted
string) so the host-context evaluation can run after debounce. The pending
message should become `Option<WindowInfo>` or a struct carrying both the string
and the WindowInfo, so the debounced worker can evaluate rules and send context.

## Config Path Integration

### Current (src/core/mod.rs, src/platforms/mod.rs)
Config paths are resolved via `platforms::get_config_paths()`:
- Linux: `~/.config/qmk-notifier/config.toml`
- Windows: `%APPDATA%\QMKonnect\config.toml`
- macOS: `~/Library/Application Support/QMKonnect/config.toml`

### Required: rules.toml Path Resolution
`rules.toml` lives in the **same directory** as `config.toml`. Need a function
like `get_rules_paths() -> Vec<PathBuf>` that mirrors `get_config_paths()` but
substitutes `rules.toml` for `config.toml`. Absent ⇒ host rules disabled.

## CLI Integration Points

### Current (src/main.rs)
Hand-rolled CLI scanning with `args.iter().any(...)` and `parse_value_flag()`.
Dispatch order: `-h` → `-c` → `-r` → `-l` → `--list-devices` → `--show-window-info` → runner.

### Required New Flags
- `--list-callbacks` — handshake → name→id table (or "legacy")
- `--validate-rules` — parse + schema check rules.toml; non-zero exit on error
- `--rules-path <path>` — override rules.toml location
- `-c` extension — also seed a commented `rules.toml` template

## Tray Integration Points

### macOS/Windows (src/tray.rs)
Menu built via `Vec<&dyn tray_icon::menu::IsMenuItem>`. Adding "Reload rules"
requires:
1. Create `MenuItem::new("Reload rules", true, None)`
2. Push into `menu_items` vec after Settings
3. Handle in `event_loop.run()` match arm on `reload_rules_i.id()`
4. On click: re-read rules.toml, re-validate, re-handshake if needed

### Linux SNI (src/linux_tray.rs)
Menu rebuilt on every `handle.update()`. Adding "Reload rules" requires:
1. Add a new `MenuItem::Separator` + `StandardItem` in `QmkTray::menu()`
2. The `activate` closure calls rules reload logic
3. No structural toggle needed (menu is always rebuilt)

## Startup Handshake Integration

### Current Startup Flow (runners/*.rs)
```
create_monitor(verbose) → startup_device_probe(verbose) → ctrlc handler → start monitor → tray
```

### Required Handshake
Near `startup_device_probe`, once a device is connected:
1. `run(QueryInfo)` → check `response[0]==0x51` && `proto_ver==2` && `flags & 0x01`
2. If capable: `run(SetOs(host_os))` → host is OS-authoritative
3. `run(QueryCallback(i))` for i in 0..callback_count → name→id map
4. Validate rules.toml callback names against the map
5. Set `capable = true` / `false` (gates host-rules)

Re-trigger on device transition via `is_device_connected()` poll.

## Device Status Poll Extension

### Current (tray.rs device-status thread)
3s poll (macOS/Windows) or 1s (Linux). Emits `DeviceStatus(bool)` on transition.

### Required Extension
On a `false → true` transition (device appeared), trigger the handshake.
Track `capable` and `has_been_queried` state so the handshake runs at most
once per board boot.

## Test Infrastructure

### Current Test Pattern (src/core/notifier.rs:374-604)
```rust
static MOCK_CALL_COUNT: AtomicU32;
static MOCK_LAST_MESSAGE: Mutex<Option<String>>;
struct MockNotifier;
fn reset_test_state();  // flush, reset STATE, reset mock counter
fn wait_for_count(target, timeout);
```

### Required Test Extensions
- Mock must record typed-command calls (not just string messages)
- Mock must return canned `CommandResponse` values for handshake tests
- Ordering assertion: string-before-context (stack mode)
- New tests for: rules evaluation, stack/replace decision, no-match clear,
  handshake parsing, capability gating
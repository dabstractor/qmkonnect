# Bug Findings & Fix Details — QMKonnect Bugfix PRD

## Bug 1 (Major, PRD ID 1): Hyprland dialog shows `class`, keyboard receives `initial_class`

### Root Cause
`list_foreground_windows()` in `src/platforms/hyprland.rs` constructs its
return tuples using `c.class.clone()` (L571) and `active.class.clone()` (L577).
However, the **notification paths** (`handle_window_state_change` L479, and
`poll_window_state` L398) use `active_window.initial_class.clone()`.

The `hyprland` crate's `Client` struct exposes both fields:
- `initial_class` (L239 in crate `data/regular.rs`): set once at window creation; stable.
- `class` (L241): can change at runtime (e.g., Electron apps re-class on navigation).

`initial_class` is the value sent to the keyboard; `class` (shown in the dialog)
can diverge, causing rules to silently fail when a user copies the dialog value.

### Fix
Change `c.class.clone()` → `c.initial_class.clone()` at L571, and
`active.class.clone()` → `active.initial_class.clone()` at L577, both inside
`list_foreground_windows()`.

### Testing
No test currently covers `list_foreground_windows()` (it calls live Hyprland IPC
via `Clients::get()`/`Client::get_active()`, which can't be mocked without a
running compositor). The fix is a 2-field-access change in one function; verify
manually on a Hyprland session with an app where `class != initial_class`.

---

## Bug 2 (Major, PRD ID 2): X11 WM_CLASS parser returns instance instead of class

### Root Cause
`src/platforms/x11.rs` L68-74 parses `WM_CLASS(STRING) = "instance", "Class"`:
```rust
let quoted: Vec<&str> = rest.split('"').filter(|s| !s.is_empty()).collect();
app_class = quoted.get(1).or_else(|| quoted.first())...;
```
For `rest = ' "instance", "Class"'`, `split('"')` yields:
`[" ", "instance", ", ", "Class", ""]`. After filtering empty strings:
`[" ", "instance", ", ", "Class"]`. The leading space (from `= `) and the `, `
separator are non-empty, so they occupy indices 0 and 2. `.get(1)` returns
`"instance"` — the **instance**, not the class.

### Fix
Replace the split-on-quote approach with split-on-comma + trim:
```rust
let parts: Vec<&str> = rest
    .split(',')
    .map(|s| s.trim().trim_matches('"'))
    .filter(|s| !s.is_empty())
    .collect();
app_class = parts.get(1).or_else(|| parts.first())...;
```
For `rest = ' "instance", "Class"'`: split(',') → `[' "instance"', ' "Class"']`
→ trim + trim_matches('"') → `['instance', 'Class']` → `.get(1)` = `"Class"`. ✓

### Testing
The parsing logic is inline in `get_active_window()`. **Recommendation**: extract
the WM_CLASS line → class string parsing into a small pure helper function (e.g.,
`fn parse_wm_class(line: &str) -> Option<String>`) so it can be unit-tested with
sample xprop output strings. x11.rs currently has **no** `#[cfg(test)]` module.

---

## Bug 3 (Major, PRD ID 4): Windows autostart path written unquoted

### Root Cause — Two locations:

**A. `src/autostart.rs` `enable()` / `current_exe_wide()` (L57, L102-103):**
`current_exe_wide()` returns the raw exe path as a null-terminated UTF-16 buffer.
`enable()` writes this directly as `REG_SZ` data. If the path contains spaces
(`C:\Users\John Doe\...`), Windows may fail to resolve the unquoted "service
path" or it becomes a security vector (unquoted path traversal).

**B. `packaging/windows/inno/QMKonnet.iss` [Registry] section:**
```ini
ValueData: "{app}\{#MyAppExeName}"
```
Same issue — the installer writes the unquoted path to the `Run` key.

### Fix
**A. `autostart.rs`**: Wrap the path in double-quotes before encoding to UTF-16.
In `current_exe_wide()` (or `enable()`), prepend `"` (U+0022) and append `"`
before the NUL terminator. The `REG_SZ` value will then be:
`"C:\Users\John Doe\AppData\Local\Programs\QMKonnect\QMKonnect.exe"`.

**B. `QMKonnect.iss`**: Inno uses `""` for literal double-quotes in string
literals. Change to:
```ini
ValueData: """{app}\{#MyAppExeName}"""
```

### Testing
- `autostart.rs` is `#[cfg(windows)]` only. Test by verifying `current_exe_wide()`
  output starts/ends with `"` (0x0022). The existing `is_enabled()` presence
  check will still match (the value exists, just quoted).
- The `.iss` change is verified by building the installer and checking the
  registry value post-install on a path with spaces.

---

## Bug 4 (Minor, PRD ID 3): Settings VID/PID change doesn't reset handshake

### Root Cause
When multiple capable boards (A, B) are present and the user switches from A to B
in Settings, the config save path writes the new VID/PID but does **not** call
`reset_handshake_state()` or `perform_handshake()`. The global `CALLBACK_NAMES`
map still reflects board A's callback registry, so board B's rules resolve names
through A's map → wrong IDs or dropped commands.

No plug/unplug event fires (both boards remain connected), so the
`PresenceTracker` Gain/Loss loop doesn't trigger a re-handshake.

### Fix
In each of the three platform save paths, after writing the config and detecting
that VID/PID actually changed:
```rust
crate::core::notifier::reset_handshake_state();
crate::core::notifier::perform_handshake(verbose);
```
Only reset when VID/PID differs from the pre-save value (avoid unnecessary reset
on an unchanged save). `reset_handshake_state()` is pub at `notifier.rs:814`.
`perform_handshake` is pub at `notifier.rs:353`.

### Locations
- **Windows**: `src/tray.rs` ~L979 (after `atomic_write`, before `Ok(())`).
  Must compare `merged.vendor_id/product_id` vs pre-merge `current_config`.
  Note: `current_config` is moved into `merged`; snapshot VID/PID before the move.
- **macOS**: `src/tray.rs` ~L1877 (same pattern).
- **Linux**: `src/linux_tray.rs` `save_and_notify` L718 (after `write_config`
  succeeds). `verbose` is not in scope here — pass `false` or add a param.

### Testing
Integration-level: requires two physical QMK boards with different callback
registries. Unit-testing the reset call is feasible by checking that
`HOST_CAPABLE` goes false and `CALLBACK_NAMES` clears after the call.

---

## Bug 5 (Minor, PRD ID 5): Windows title-length heuristic uses byte length

### Root Cause
`should_ignore_window()` in `src/platforms/windows.rs` checks:
```rust
if window_info.title.len() < 2 && !window_info.title.is_empty() {
    return true;
}
```
Rust's `String::len()` returns **byte** length. A 1-char emoji (4 bytes UTF-8)
passes (`4 >= 2`), while a 1-char ASCII title (1 byte) is ignored (`1 < 2`).

### Fix
Change to character count:
```rust
if window_info.title.chars().count() < 2 && !window_info.title.is_empty() {
    return true;
}
```

### Testing
`should_ignore_window()` takes `&WindowInfo` — pure function, easily unit-tested
with various title inputs (1-char ASCII, 1-char emoji, empty, 2-char, multi-byte
strings). No test module currently exists in `windows.rs`.

---

## Documentation Impact Summary
- **Mode A (with-work)**: None of the 5 fixes change documented user-facing
  behavior surface. The docs already say "window class" generically, and the
  fixes make the code match the docs MORE closely.
- **Mode B (changeset-level)**: A final docs task should evaluate whether
  `docs/troubleshooting.md` §"Pattern matches the real window class?" (L571) or
  `docs/configuration.md` §rules need a note about Hyprland `initial_class` vs
  X11 `class` consistency. `README.md` does not need changes.
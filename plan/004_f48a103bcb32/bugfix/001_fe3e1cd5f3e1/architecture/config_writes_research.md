# Config / Rules Write Atomicity — Scout Findings

## Summary

The bug-hunt report's claim is **CONFIRMED**: every `config.toml` / `rules.toml`
save path uses `std::fs::write` (truncate-then-write) with **no temp-file +
rename**. The one atomic-write helper in the codebase
(`write_rule_atomic`) targets **udev rules** (`/etc/udev/rules.d`), not config
or rules files, and relies on the Linux-only `tempfile` crate.

The readers (`parse_config`, `parse_rules`) run on the **notifier thread per
window-change / per-notification** with no locking, while saves run on the
**tray/UI thread**. A concurrent reader can therefore observe a truncated or
empty file mid-save. Impact is **transient and self-healing** (graceful
fallback to defaults / string-only), not permanent corruption — but it is a
real race producing a spurious "rules.toml invalid" notification or a brief
device-filter mismatch during a save.

Severity: **Low** (graceful degradation, tiny window, self-healing). The
report's framing ("data loss") is overstated — `std::fs::write` itself
completes atomically from the writer's side; only a *concurrent reader* sees
intermediate state.

---

## 1. All write call sites (`grep -rn 'fs::write\|File::create\|write_all' src/`)

### A. Production writers for `config.toml` — ALL NON-ATOMIC

| # | File:Line | Function | Writes | Atomic? |
|---|-----------|----------|--------|---------|
| 1 | `src/core/mod.rs:218` | `create_default_config` | seeds `config.toml` (first-run / `qmkonnect -c`) | **No** — `fs::write(config_path, default_config)` |
| 2 | `src/tray.rs:878` | `show_settings_dialog` (`#[cfg(windows)]`, fn @ 752) | `config.toml` after `render_config_body` — Windows Settings dialog OK | **No** — `std::fs::write(config_path, config_content)?` |
| 3 | `src/tray.rs:1276` | `show_settings_dialog_with_pool` (`#[cfg(macos)]`, fn @ 1185) | `config.toml` after `render_config_body` — macOS Settings dialog OK | **No** — `std::fs::write(config_path, config_content)?` |
| 4 | `src/linux_tray.rs:822` | `write_config` (fn @ 805) | `config.toml` after `render_config_body` — Linux Settings dialog | **No** — `std::fs::write(&path, content)?` |

### B. Production writer for `rules.toml` — NON-ATOMIC

| # | File:Line | Function | Writes | Atomic? |
|---|-----------|----------|--------|---------|
| 5 | `src/core/mod.rs:334` | `create_default_rules` | seeds `rules.toml` template (first-run; also called by `edit_rules`) | **No** — `fs::write(rules_path, render_rules_body())` |

> Note: `rules.toml` has **no settings-dialog save path** — the only producer is
> the seeder (`create_default_rules`, a no-op if the file exists) plus whatever
> external editor the user opens (`edit_rules` → `open_in_default_app`, outside
> our control). So the rules.toml race is narrower than config.toml's.

### C. The ONE atomic helper — NOT config/rules

| File:Line | Function | Writes | Atomic? |
|-----------|----------|--------|---------|
| `src/platforms/linux.rs:344-360` | `write_rule_atomic` (fn @ 336) | udev rule in `/etc/udev/rules.d` | **Yes** — `tempfile::NamedTempFile::new_in(dir)` → `write_all` → `sync_all` → `persist(path)` (rename) |

This is the proven pattern to copy, but it is Linux-only (see §6).

### D. Out of scope (not config.toml / rules.toml)

- `src/tray.rs:204` — writes autostart first-run **marker file** (byte `b"1"`).
- `src/linux_tray.rs:762` — `apply_device_rule` stages a udev rule to
  `/tmp/qmkonnect-rule.tmp` for `pkexec install` (a *staging* file, not the
  final target; the privileged `install` is what atomizes it).
- `src/linux_tray.rs:468` — `stdin.write_all` (piping to a child process).
- All `core/notifier.rs`, `core/rules.rs`, `core/mod.rs:518/545` writes inside
  `#[cfg(test)]` blocks — test fixtures only.

---

## 2. `render_config_body` — definition + callers

- **Definition:** `src/core/mod.rs:157` — `pub fn render_config_body(config: &Config) -> String`.
  Pure renderer (no IO): serializes the *full* `Config` (VID/PID/usage_page/usage
  as commented-out hints when `None`, plus literal `debounce_ms`/`poll_interval_ms`).
- **Callers (all then `std::fs::write`):**
  - `src/tray.rs:876` → write at `:878` (Windows dialog)
  - `src/tray.rs:1275` → write at `:1276` (macOS dialog)
  - `src/linux_tray.rs:821` → write at `:822` (Linux `write_config`)
- Sibling pure renderer for rules: `render_rules_body` (`core/mod.rs:241`), used
  only by the seeder `create_default_rules` (`:334`).

---

## 3. Reader side of the race (could observe partial writes)

No lock guards config/rules file IO (verified: the only mutexes in the codebase
guard the notifier handle, debounce state, dialog result, callback registry —
**none** around the config/rules files). All reads use `fs::read_to_string`,
which returns whatever bytes are on disk at read time.

### Config readers (`parse_config` → `fs::read_to_string` + `toml::from_str`)
`src/core/mod.rs:106`.

| Caller | File:Line | Cadence | On partial read |
|--------|-----------|---------|-----------------|
| `configured_filter` | `core/notifier.rs:80` (called @ 167,217,400,701,886,954) | **per notification / per window change** (hot) | `.ok()` → `None` → `DeviceFilter` with defaults (auto-discovery, DEFAULT_USAGE_PAGE/USAGE). Transient wrong-device targeting. |
| `configured_timing` | `core/mod.rs:~98` | per send (debounce/poll) | `.ok()` → defaults (50 ms debounce) |
| `current_config_hex` | `linux_tray.rs:779` | Settings dialog open | wrong pre-fill of dialog |
| `write_config` | `linux_tray.rs:812` | on Save (Linux) | **read-modify-write race with itself + concurrent save** |
| dialogs read `current_config` | tray.rs (Win/macOS) | on Save | stale VID/PID overlay |
| `config_parse_error` | `notifier.rs:106` | startup only | diagnostic only |

### Rules readers (`parse_rules` → `fs::read_to_string` + `toml::from_str` + `validate_rules`)
`src/core/rules.rs:210`.

| Caller | File:Line | Cadence | On partial read |
|--------|-----------|---------|-----------------|
| `host_context_for_window` | `core/notifier.rs:1023` (fn @ 1013; invoked @ 887 in debounced send) | **per window change** (hot) | `Err` → fires **one** "QMKonnect: rules.toml invalid" desktop notification, falls back to string-only (host rules disabled) for that window |
| `validate_rules_callback_names` | `core/notifier.rs:584` (fn @ 563) | per handshake (connect) | `Err` → warn, skip (non-fatal) |
| `--validate-rules` CLI | `main.rs:407` | on demand | reports error |

---

## 4. Race window & concrete impact

`std::fs::write` opens with `O_WRONLY|O_CREAT|O_TRUNC`, which **zeros the file
before writing** the new bytes. A `read_to_string` landing in that gap sees:
- **empty file** → `toml::from_str("")` *succeeds*: config → all-`None` defaults;
  rules → empty `RuleSet` (host rules disabled). Silent wrong behavior.
- **partial body** → `toml::from_str` *errors*. Config: swallowed by `.ok()`.
  Rules: spurious "rules.toml invalid" notification + disabled host rules.

The file is **never left permanently broken** — the writer always completes the
full write; the next reader after completion sees correct data. So this is a
**transient, self-healing** race, not data loss. The two observable symptoms:
1. A one-off spurious "rules.toml invalid" desktop notification during a save.
2. A brief device-filter / host-rules blip for a single window change during a save.

---

## 5. Is the `tempfile` crate available in non-test code?

- `Cargo.toml:31` — `tempfile = "3.0"` under **`[dev-dependencies]`** → NOT usable in non-test code.
- `Cargo.toml:37` — `tempfile = "3.0"` under **`[target.'cfg(target_os = "linux")'.dependencies]`** → available in non-test code **on Linux only**.

**Conclusion:** `write_rule_atomic` can use `tempfile::NamedTempFile` because it
is Linux-only. To make the **config/rules** writers atomic on macOS/Windows you
must either:
- **(a)** promote `tempfile` to a top-level `[dependencies]` entry (adds a small
  transitive footprint — `fastrand`, `getrandom`, `windows-sys`/`libc` — to
  macOS/Windows release binaries), or
- **(b)** implement a std-only atomic helper (write to `<path>.tmp` in the same
  dir via `fs::write`, then `fs::rename` over the target). `rename` is atomic on
  the same filesystem on all three platforms; the `.tmp` + target share a parent
  dir so they're on the same volume. No new dependency.

Option (b) is the lighter fix and matches the existing Linux helper's
shape (`NamedTempFile::persist` is itself `rename` under the hood).

---

## 6. Recommended fix shape (for the implementing agent)

A single cross-platform helper in `src/core/mod.rs` (next to `render_config_body`):

```rust
pub fn atomic_write(path: &Path, content: &str) -> Result<(), Box<dyn Error>> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let tmp = dir.join(format!(
        ".{}.tmp", path.file_name().and_then(|s| s.to_str()).unwrap_or("cfg")
    ));
    fs::write(&tmp, content)?;
    // fsync optional; rename is the atomicity guarantee (same-fs, same dir).
    fs::rename(&tmp, path)?;
    Ok(())
}
```
Then replace the five call sites (§1 A/B) — each currently
`fs::write(<path>, <content>)` — with `atomic_write(<path>, &<content>)`.

Caveats to handle in the helper:
- Best-effort cleanup of a stale `.tmp` on failure (so a crash mid-write
  doesn't leave litter); tolerate its absence.
- `config.toml`/`rules.toml` live in a per-user config dir the process already
  owns (created via `fs::create_dir_all`), so no root/permission wrangling is
  needed (unlike `write_rule_atomic`, which has a `PermissionDenied` branch for
  `/etc/udev/rules.d`).

---

## Files Retrieved

1. `src/core/mod.rs` (lines 92-345, 357-365) — `parse_config`, `render_default_config_template`, `render_config_body`, `create_default_config` (write @ 218), `render_rules_body`, `create_default_rules` (write @ 334), `edit_rules`.
2. `src/tray.rs` (lines 840-888, 1155-1289) — Windows `show_settings_dialog` write @ 878; macOS `show_macos_settings_dialog`/`show_settings_dialog_with_pool` write @ 1276.
3. `src/linux_tray.rs` (lines 694, 741-835) — `write_config` (write @ 822), `apply_device_rule` (udev staging @ 762).
4. `src/platforms/linux.rs` (lines 336-360) — `write_rule_atomic` (the atomic template to copy).
5. `src/core/notifier.rs` (lines 78-110, 560-590, 880-920, 1010-1060) — readers `configured_filter`, `host_context_for_window`, `validate_rules_callback_names`.
6. `src/core/rules.rs` (lines 197-215) — `parse_rules`.
7. `Cargo.toml` (lines 31, 37) — `tempfile` dev-dep vs Linux-only dep.

## Start Here
`src/platforms/linux.rs:336` (`write_rule_atomic`) — the only correct atomic-write
example in the repo; copy its temp-file → `sync` → rename shape into a
cross-platform helper. Then audit the five call sites in `src/core/mod.rs`
(218, 334), `src/tray.rs` (878, 1276), and `src/linux_tray.rs` (822).
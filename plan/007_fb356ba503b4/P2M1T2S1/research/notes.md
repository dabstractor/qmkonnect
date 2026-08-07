# Research Notes — P2.M1.T2.S1: Add `LinuxConfig` struct + `[linux]` table to Config schema

**Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). **File edited:** `src/core/mod.rs`
(the Config schema + parsers + renderers). **PRD contract:** `CONFIG.md` §1/§1.3
(snapshot lines 2937–3015) + `PLATFORMS.md` §6 (the consumer,
`select_linux_backend`). **Source of truth:** the production code itself
(`src/core/mod.rs`); where the PRD's idealized schema and the code disagree, the
code wins — see §3 (the `Default` derivation question).

---

## 1. The exact PRD schema (CONFIG.md §1 + §1.3, snapshot lines 2937–3015)

```rust
#[derive(serde::Deserialize, serde::Serialize, Default)]          // PRD "intended design"
pub struct Config {
    #[serde(default)] pub vendor_id:      Option<u16>,
    #[serde(default)] pub product_id:     Option<u16>,
    #[serde(default)] pub usage_page:     Option<u16>,
    #[serde(default)] pub usage:          Option<u16>,
    #[serde(default = "default_debounce_ms")]      pub debounce_ms: u64,      // 50
    #[serde(default = "default_poll_interval_ms")] pub poll_interval_ms: u64, // 0
    #[serde(default)] pub linux: LinuxConfig,    // ← NEW (this task)
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct LinuxConfig {
    #[serde(default)] pub backend:             Option<String>, // None = auto
    #[serde(default)] pub gnome_poll_interval_ms: Option<u64>, // None = 1000
}
```

```toml
[linux]
backend = "auto"                 # auto | foreign-toplevel | gnome | hyprland | atsi | x11
gnome_poll_interval_ms = 1000    # GNOME backend drift-poll cadence (ms)
```

| Key | Type | Default | Meaning |
|---|---|---|---|
| `linux.backend` | `Option<String>` | `"auto"` | Force a specific backend (diagnostic). A forced backend that is unavailable errors loudly with every probe result (`PLATFORMS.md` §6). |
| `linux.gnome_poll_interval_ms` | `Option<u64>` | `1000` | GNOME backend drift-correcting poll cadence (ms); hot-re-read each tick. |

> `backend` is diagnostic in normal use — auto selection is correct on every
> supported desktop. Both fields are `Option` + `#[serde(default)]`, so a config
> without a `[linux]` table (or `backend = "auto"`) uses the runtime priority order.

---

## 2. The CURRENT `src/core/mod.rs` state (the code being edited — verified)

The current `Config` does NOT match the PRD's idealized `#[derive(... Default)]`:

```rust
#[derive(serde::Deserialize, serde::Serialize, Clone)]   // ← NO derive Default
pub struct Config {
    #[serde(default)] pub vendor_id: Option<u16>,
    #[serde(default)] pub product_id: Option<u16>,
    #[serde(default)] pub usage_page: Option<u16>,
    #[serde(default)] pub usage: Option<u16>,
    #[serde(default = "default_debounce_ms")] pub debounce_ms: u64,        // 50
    #[serde(default = "default_poll_interval_ms")] pub poll_interval_ms: u64, // 0
}

impl Default for Config {          // ← MANUAL impl (NOT derived) — load-bearing
    /// ... MUST agree with the serde `default = ...` attributes so that
    /// `Config::default()`, an empty `config.toml`, and `configured_timing()`
    /// all describe the SAME zero-config state. ... A manual impl is used instead
    /// of `#[derive(Default)]` because the derive would zero-init `debounce_ms`,
    /// not match the serde default.
    fn default() -> Self {
        Self { vendor_id: None, product_id: None, usage_page: None, usage: None,
               debounce_ms: DEFAULT_DEBOUNCE_MS, poll_interval_ms: DEFAULT_POLL_INTERVAL_MS }
    }
}
```

**Key functions in `src/core/mod.rs`:**
- `parse_config(path) -> Result<Config, _>` → `toml::from_str` (line ~190).
- `cached_config()` / `cached_config_at(path)` → mtime+size-keyed cache (returns whole `Config`; adding a field is transparent).
- `configured_timing() -> (u64, u64)` → reads `debounce_ms`/`poll_interval_ms` (UNAFFECTED — only reads existing fields).
- `render_default_config_template() -> String` → the `-c` seeder (fully commented template; parses to all-default).
- `render_config_body(config: &Config) -> String` → the **save renderer** used by every Settings-dialog write path; serializes the FULL config so every field round-trips through a write+re-parse. Carries `#[cfg_attr(not(any(macos, windows)), allow(dead_code))]` (only the macos/win Settings dialogs use it; Linux uses zenity).

**`configured_filter()`** lives in `src/core/notifier.rs` (NOT mod.rs) — it reads `vendor_id`/`product_id`/`usage_page`/`usage`. Adding `linux` to Config is transparent to it.

---

## 3. THE `Default` derivation question (the #1 gotcha — code wins over PRD)

The PRD shows `#[derive(... Default)]` on `Config`. **DO NOT switch to it.** The
current code deliberately uses a MANUAL `impl Default` because:

- `#[derive(Default)]` would zero-init `debounce_ms: u64` to **0**, but the serde
  default is **50** (`default_debounce_ms`). A `Config::default()` from the derive
  would SILENTLY DISABLE DEBOUNCING — a correctness regression, and it would break
  the documented invariant that `Config::default()`, an empty `config.toml`, and
  `configured_timing()` all describe the same zero-config state.

**Resolution (what this task does):**
- KEEP the manual `impl Default for Config`. Add `linux: LinuxConfig::default()` to it.
- For `LinuxConfig`, `#[derive(Default)]` is SAFE: both fields are `Option`
  (`backend → None`, `gnome_poll_interval_ms → None`), which is exactly the
  "auto / 1000-at-use-site" default. The item description explicitly asks for
  `#[derive(Default)]` on LinuxConfig — honor it.
- Report the PRD↔code drift in a code comment (the merged PRD's standing rule:
  *"Where a spec and the code disagree, the code wins; report the drift."*).

---

## 4. `Clone` is REQUIRED on `LinuxConfig` (compile constraint)

`Config` derives `Clone` (`#[derive(serde::Deserialize, serde::Serialize, Clone)]`).
Adding `pub linux: LinuxConfig` means `LinuxConfig` MUST also be `Clone` or the
derive on `Config` fails to compile. So `LinuxConfig` derives at minimum
`serde::Deserialize, serde::Serialize, Clone, Default`. Add `Debug` too (cheap,
useful for `verbose` logging / `{:?}` diagnostics — the `select_linux_backend`
verbose path already prints config-derived decisions). The derive list for
`LinuxConfig`:

```rust
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default)]
pub struct LinuxConfig { ... }
```

---

## 5. `render_config_body` — round-trip safety (the #2 gotcha)

`render_config_body` is the **save renderer**: every platform's Settings-dialog
write path reads the current `Config`, overlays the dialog's VID/PID, and
serializes the FULL struct via this fn. Its doc comment is explicit:
*"guaranteeing the user's usage_page/usage/debounce_ms/poll_interval_ms survive a
VID/PID edit (previously they were silently reset to defaults)."*

If this task adds `linux` to `Config` but does NOT render the `[linux]` table in
`render_config_body`, then a user who manually set `[linux] backend = "x11"` would
**lose that override** the next time they edit VID/PID in Settings (the file is
regenerated without the table). This is the SAME class of bug the fn was created
to fix.

**Resolution:** extend `render_config_body` to emit a `[linux]` block that reflects
the ACTUAL `config.linux` values (active when `Some`, commented hint when `None`),
mirroring exactly how `vendor_id`/`usage_page` are rendered. This preserves the
round-trip invariant AND matches the existing test
`render_config_body_preserves_non_vidpid_fields` (which must be updated to also
assert `linux` round-trips). The item description's "commented hints only" is
honored for the `None` case (the common fresh-install path renders comments, so a
seeded/Settings-saved file still parses to `LinuxConfig::default()`).

```text
# (appended to the render_config_body output, after poll_interval_ms)
#
# Linux window-monitor backend ([linux]). Both fields OPTIONAL; omit the whole
# table (or backend = "auto") for runtime auto-selection (foreign-toplevel ->
# GNOME -> Hyprland -> AT-SPI -> X11). A forced backend that is unavailable errors
# loudly with every probe result. (Linux only; ignored on macOS/Windows.)
[linux]
backend = "x11"                      # when Some — active line
# backend = "auto"                   # when None — commented hint
gnome_poll_interval_ms = 2000        # when Some
# gnome_poll_interval_ms = 1000      # when None
```

**`render_default_config_template`** (the `-c` seeder): add a fully-commented
`[linux]` block (parses to default, inert on fresh install — mirrors the existing
fully-commented style).

---

## 6. Construction sites that BREAK when adding a field (verified by grep)

`grep -rn "Config {" src/` found exactly these struct-literal sites:
- `src/core/mod.rs:53` — `impl Default for Config` (the manual impl). UPDATE: add `linux: LinuxConfig::default()`.
- `src/core/mod.rs:579` — test `render_config_body_round_trips`, uses `..Config::default()` → **OK, won't break** (struct-update syntax fills `linux`).
- `src/core/mod.rs:620` — test `render_config_body_preserves_non_vidpid_field`, a FULL 6-field literal → **MUST UPDATE** (add `linux`, ideally set `backend: Some("x11".into())` and assert it round-trips).

No other `Config { ... }` literals in `src/` (notifier/tray/main/platforms all read
Config via `cached_config()`, never construct it literally). So only 3 sites touch.

---

## 7. The downstream consumer: `create_monitor` TODO (parallel item P2.M1.T1.S1)

The parallel item's PRP (treated as contract) ships `create_monitor` in
`src/platforms/mod.rs` with a staged seam:

```rust
#[cfg(target_os = "linux")]
{
    // TODO(P2.M1.T2.S1): wire `[linux] backend` from core::cached_config() into the `forced` arg.
    return linux::select_linux_backend(verbose, None);
}
```

`select_linux_backend(verbose, forced: Option<&str>)` already takes the override as
a parameter. THIS task closes that TODO by reading `cached_config().linux.backend`
and passing it as `forced` — a ~3-line change in `create_monitor`. Without it,
`[linux] backend` parses but is DEAD (never consulted), so the feature is inert.

**Scope decision:** the item description's OUTPUT is "LinuxConfig wired into Config"
(the field). But the parallel item explicitly defers the `create_monitor` wiring to
THIS task via the TODO. Closing it makes the field functional. Include it as the
FINAL task (clearly marked, ~3 lines in `src/platforms/mod.rs`), so the PRP ships a
working feature, not a dead schema. The change composes cleanly with the parallel
item (T1.S1 writes the TODO; T2.S1 fills it — sequential, not conflicting).

`gnome_poll_interval_ms` is consumed by the GNOME backend (P2.M3.T2, not yet
written). For THIS task it just needs to PARSE + round-trip; no consumer wiring
exists yet. (The `backend = "auto"` / `None` normalization is also done at the use
site in `create_monitor`: treat `None` and `Some("auto")` the same — `forced = None`.)

---

## 8. Gotchas summary

- **GOTCHA-1 (manual `Default`, not derived):** Config keeps its MANUAL `impl Default`
  (deriving would zero-init `debounce_ms` to 0, breaking debouncing). Add
  `linux: LinuxConfig::default()` to the manual impl. LinuxConfig CAN safely
  `#[derive(Default)]` (both fields `Option`). Report the PRD drift in a comment.
- **GOTCHA-2 (Clone required):** `LinuxConfig` must derive `Clone` (Config derives
  `Clone`; a non-`Clone` field fails the derive). Add `Debug` too (logging).
- **GOTCHA-3 (render_config_body round-trip):** if `render_config_body` doesn't
  emit the `[linux]` table, a Settings save strips a user's manual override. Render
  the actual values (active when `Some`, commented when `None`) — same pattern as
  `vendor_id`. Update `render_config_body_preserves_non_vidpid_fields` to assert it.
- **GOTCHA-4 (full struct literal at line 620):** the test constructs Config with a
  6-field literal; adding `linux` makes it fail to compile. Add `linux` (+ set it to
  a `Some` value to assert round-trip).
- **GOTCHA-5 (`None` vs `Some("auto")`):** at the use site, BOTH mean auto-selection.
  In `create_monitor`, normalize `backend == Some("auto")` to `forced = None` before
  passing to `select_linux_backend`. (`None` already means auto; treat "auto" the same.)
- **GOTCHA-6 (single-threaded tests):** the crate shares global debouncer state.
  `cargo test --bin qmkonnect -- --test-threads=1` (AGENTS.md). The new config tests
  are hermetic but still run under the single-threaded harness.
- **GOTCHA-7 (no cross-platform rendering leak):** `render_config_body` has
  `#[cfg_attr(not(any(macos, windows)), allow(dead_code))]` — it's only CALLED by the
  macos/win Settings dialogs today. Adding the `[linux]` block is fine (Linux parses
  it via `parse_config` regardless of whether `render_config_body` is called there).
  Do NOT change the `cfg_attr`.
- **GOTCHA-8 (MSRV):** the crate targets a recent stable (MSRV 1.88 per existing
  `FileTimes` usage). `Option<String>` / `Option<u64>` / `#[derive(Default)]` are
  ancient — no version concern.
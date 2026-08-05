# QMKonnect — Validation Report

**Codebase:** QMKonnect v0.2.8 (cross-platform QMK window→keyboard daemon, Rust)
**Validated on:** Linux x86_64 (Arch, rustc 1.92.0) with a real qmk_notifier-capable
keyboard on the bus (Dactyl-Manuform 5×7-1, `0x1209:0x7f00`).
**Date:** 2025-08-05
**Method:** Deep code review (PRD + 11 specs vs. `src/`), plus the `validate.sh`
script (7 phases, 27 checks) exercising the actual dev-loop / CI gates and
end-to-end CLI workflows against real and isolated-`HOME` environments.

---

## TL;DR

| Result | Count |
|---|---|
| ✅ Passed | 24 / 27 |
| ❌ Failed | 3 (2 of which are documented quality gates) |
| ⏭ Skipped | 0 |

The application is **functionally solid and spec-faithful**. The end-to-end path
works against real hardware: device discovery, the typed-command capability
handshake, callback-name sweep, host-rule evaluation, debounced sends, the
`qmkonnect-hid-id` udev helper, and the single-line udev-rule safety guarantee
all behave exactly as the specs require. The three failures are **hygiene /
metadata** issues (formatting, a clippy lint, and a README path case mismatch),
**not** behavioral bugs.

**Headline finding:** `cargo fmt --check` and `cargo clippy -D warnings` both
fail, and CI gates on `fmt` — so **a push of this tree to `main` would fail the
`fmt` CI job**.

---

## What works (validated end-to-end)

These all pass in `validate.sh` and were confirmed independently:

- **Release build** is clean (no warnings) for the default full-app profile
  (`opt-level="z"`, LTO, `panic="abort"`, `strip`). Builds for all four variants:
  default, `--all-targets` (CI gate), `--no-default-features` (trayless service),
  and the `qmkonnect-hid-id` udev helper bin.
- **All 383 unit tests pass** single-threaded (`--test-threads=1`), including the
  shared global-debouncer state, the firmware-corpus pattern-matcher parity
  suite (~9500-line `pattern.rs`), the `rules` evaluator (first-match layer vs
  all-match callbacks, order-independent disable-exclusion), the R-COEX /
  debounce / cache suites, and the dangerous-udev-rule regression sentinel.
- **`--list-devices`** correctly classifies the real board's `0xff60:0x0061`
  interface as `qmk_notifier` (Tier-2 capability probe) and shows the `kind`
  column; every other interface shows `-`.
- **`--list-callbacks`** performed a live typed-command handshake against the
  connected Dactyl-Manuform and printed `0  vim_lazy` — proving the entire
  `QUERY_INFO` → `QUERY_CALLBACK` transport path works end-to-end on real
  firmware.
- **`--validate-rules`** handles every documented case correctly: valid ruleset
  (exit 0, unknown-callback warning), `layer = 255` rejected (exit 1, the `0xFF`
  clear-sentinel guard), match-only rule rejected (exit 1, the §9 Validity
  check), malformed TOML rejected (exit 1).
- **`-c`** creates a clean `config.toml` + `rules.toml` pair in an isolated
  `XDG_CONFIG_HOME`; the seeded template has **no `0xfeed` literal** (the §7.2
  cleanup gate) and round-trips through the parser.
- **`--reload`** generates a **single physical line** starting with the
  `KERNEL==` match key (not the historically-dangerous multi-line form),
  resolves the invoking user's config under an isolated `HOME` (root-aware), and
  prints the exact `sudo tee … <<'EOF'` install command as a non-root user.
- **`qmkonnect-hid-id`** prints `ID_QMKONNECT=1` for the real QMK report
  descriptor (`06 60 ff 09 61 …`) and prints nothing for a non-QMK mouse.
- **Spec invariants hold:** wire payload uses GS `0x1D` + magic `0x81 0x9F`
  (ETX `0x03` appended by the crate); the static udev rule is one line and
  imports the helper; the systemd template has `BindsTo` + `Restart=always`; the
  R-COEX test asserts every transport `RunCommand` variant emits `0x81` first.
- **F13 three-state status** (Connected / NoModule / Disconnected), the
  discovered-device picker, and the capability handshake are fully wired into
  both `tray.rs` (macOS/Windows) and `linux_tray.rs`, not stubbed.

---

## Issues found

### ❌ 1. [CRITICAL] `cargo fmt --check` fails — CI `fmt` job would fail

**Evidence**
```
$ cargo fmt --all -- --check; echo $?
1
```
4 files have formatting drift:
- `src/core/mod.rs`
- `src/core/notifier.rs`
- `src/linux_tray.rs`
- `src/tray.rs`

**Why it matters.** `.github/workflows/ci.yml` defines a `fmt` job that runs
exactly `cargo fmt --all -- --check`. This tree, if pushed to `main`, **fails
that CI job** (and `ci.yml` only runs on `push: branches: [main]`, so the
failure surfaces at merge time, not on the feature branch). The diffs are all
mechanical rustfmt reflow (e.g. wrapping long `.to_string()` chains and
`assert!(...)` macros) — a single `cargo fmt --all` fixes all four files.

**Severity:** Critical for CI hygiene; zero behavioral impact.

---

### ❌ 2. [MEDIUM] `cargo clippy -D warnings` fails (documented dev-loop gate)

**Evidence**
```
$ cargo clippy --all-targets -- -D warnings
error: very complex type used. Consider factoring parts into `type` definitions
   --> src/core/mod.rs:107
    static CONFIG_CACHE: Lazy<Mutex<Option<(PathBuf, SystemTime, u64, Config)>>> = ...
error: ... src/core/mod.rs:114   (RULES_CACHE)
error: ... src/core/notifier.rs:1825
```
3 `clippy::type_complexity` errors on static cache type aliases.

**Why it matters.** `AGENTS.md` (Linux dev loop) documents
`cargo clippy --all-targets -- -D warnings` as a required step. It currently
fails. It is **not** enforced by CI (`ci.yml` runs only fmt + build + test), so
it does not block releases — but the documented developer loop is broken.

**Fix:** either factor the tuple types into `type ConfigCacheKey = …;` aliases
(recommended) or add `#[allow(clippy::type_complexity)]` at the three static
definitions (the cache types are intentionally inline). Newer rustc (1.92 here)
tightened this lint vs. the version that originally passed.

**Severity:** Medium (dev-loop friction; no runtime effect).

---

### ❌ 3. [LOW] `Cargo.toml` readme path is case-mismatched on Linux

**Evidence**
```
$ grep '^readme' Cargo.toml        # → readme = "Readme.md"
$ ls Readme.md                     # → No such file or directory
$ ls README.md                     # → README.md  (the real file)
$ cargo metadata --no-deps ...     # → "readme":"Readme.md"  (non-existent path)
```

**Why it matters.** `Cargo.toml` declares `readme = "Readme.md"` but the file is
`README.md`. On case-sensitive filesystems (Linux, which is the dev/CI/release
platform) the declared path does not resolve; `cargo metadata` advertises a
readme that doesn't exist. Harmless for building (the field isn't read at
compile time) and the crate is `publish = false`, but it's incorrect metadata
and would surprise packaging tooling / crates-viewer tooling. (`macOS`'s default
case-insensitive FS masks it locally.) One-character fix: `Readme.md` →
`README.md`.

**Severity:** Low.

---

### ⚠ 4. [LOW / cosmetic] `--no-default-features` build emits 7 dead-code warnings

**Evidence**
```
$ cargo build --release --no-default-features
warning: function `render_config_body` is never used       (src/core/mod.rs:255)
warning: function `startup_device_was_connected` is never used (src/core/notifier.rs:289)
warning: function `reset_handshake_state` is never used     (src/core/notifier.rs:759)
warning: enum `HandshakeAction` is never used               (src/core/notifier.rs:1190)
warning: function `handshake_action` is never used          (src/core/notifier.rs:1216)
warning: function `list_foreground_windows` is never used   (src/platforms/mod.rs:81)
warning: unused variable: `verbose`                         (src/tray.rs:297)
```

**Why it matters.** These functions/enum are only reachable via platform- or
feature-specific code paths (`hyprland` / `linux-tray` / the tray-bearing
platforms). The **default** full-app build is warning-clean; only the minimal
trayless `--no-default-features` build (documented for the "trayless service"
target) surfaces them. They are benign — no behavior risk — but the minimal
build is noisier than the spec implies. `#[cfg]`-gating or `#[allow(dead_code)]`
per symbol would silence them.

**Severity:** Low / cosmetic.

---

### ℹ 5. [INFO, not a bug] `env::set_var` + Edition 2024 note

`src/platforms/hyprland.rs:358` calls `env::set_var("HYPRLAND_INSTANCE_SIGNATURE", …)`.
It already runs **once, on the main thread, before any listener spawns** (safe
today, Edition 2021) and carries an explicit `// NOTE: wrap in unsafe {} if/when
bumping to edition 2024.` comment. This matches the ARCHITECTURE spec exactly.
**No action required** — flagged only for completeness; it is a documented
forward-compatibility note, not a current defect.

---

## Coverage / methodology notes

- **Validated against real hardware:** the connected Dactyl-Manuform runs
  qmk_notifier with `vim_lazy` registered, so the discovery probe, handshake,
  callback sweep, and `--list-devices` kind column were exercised against live
  firmware rather than mocks.
- **Cross-platform surfaces not executed here** (Linux-only host): the Windows
  (`src/platforms/windows.rs`, `src/autostart.rs`, `src/tray.rs` Win32 paths) and
  macOS (`src/platforms/macos.rs`, `tray.rs` Cocoa/SMAppService paths) modules
  were reviewed against `spec/PLATFORMS.md` / `spec/UI.md` and **compile** under
  CI's `--all-targets` matrix, but their runtime behavior (tray icon, window
  filtering, autostart) could only be statically inspected, not driven. The
  Windows window-class ignore list and empty-title allowlist match the spec
  verbatim (with a documented, deliberate omission of `ApplicationFrameWindow` /
  `CoreWindow`, which are resolved-to-content upstream).
- **No production-data mutation:** all CLI E2E checks ran under an isolated
  `HOME`/`XDG_CONFIG_HOME`; no real user config or system udev rule was touched.

---

## Conclusion

QMKonnect is in strong shape for a beta. The two-tier device discovery,
capability handshake, host-side rules engine (matcher parity + stack/replace
semantics + order-independent disable-exclusion), debounce coalescing, and the
Linux udev/systemd integration are all implemented faithfully to the specs and
**work end-to-end against real hardware**.

The only action items are hygiene:
1. **Run `cargo fmt --all`** (restores the CI `fmt` gate) — highest priority.
2. **Resolve the 3 `clippy::type_complexity` errors** (restores the documented
   `AGENTS.md` dev loop).
3. **Fix `Cargo.toml` `readme = "Readme.md"` → `"README.md"`** (correct metadata).
4. *(optional)* Silence the 7 `--no-default-features` dead-code warnings.

None of these are behavioral bugs; the product behaves correctly.
# QMKonnect — Validation Report

**Date:** 2026-08-05
**Scope:** Deep codebase analysis + end-to-end validation of the five defects in the PRD
**Host:** Linux x86_64 (Rust 1.92.0, default feature set = `hyprland` + `macos` + `linux-tray`)
**Validator:** Automated via `./validate.sh` (23 gates, exit 0) + manual E2E + source audit

---

## TL;DR

**All five PRD-described defects are already FIXED in the current source tree.**
The fixes carry targeted regression tests (one explicitly labelled *"THIS IS THE
REGRESSION CASE"*), and every quality gate is green: **390 unit tests pass**
(default build), **381 pass** (minimal/X11 build), **clippy clean**,
**rustfmt clean**, **release build succeeds**.

| # | PRD defect | Severity | Status | Regression test |
|---|------------|----------|--------|-----------------|
| 1 | Hyprland dialog shows `class` not `initial_class` | Major | ✅ Fixed | (no UT — shells to Hyprland IPC; verified by audit) |
| 2 | X11 `WM_CLASS` parser returns instance not class | Major | ✅ Fixed | `parse_wm_class_returns_class_not_instance` ✔ runs |
| 3 | VID/PID change doesn't reset handshake | Minor | ✅ Fixed | call-site audit (6 sites) + logic read |
| 4 | Windows autostart path written unquoted | Major | ✅ Fixed | `current_exe_wide_is_quoted` (cfg windows) |
| 5 | Windows title heuristic counts bytes not chars | Minor | ✅ Fixed | `test_ignore_one_char_emoji_title` (cfg windows) |

**No new bugs were introduced.** Two non-defect observations are noted at the end.

---

## Validation Method

`validate.sh` runs six phases, each mirroring the project's real tooling
(`AGENTS.md` mandates single-threaded tests due to shared debouncer state):

1. **Lint** — `cargo clippy --all-targets -- -D warnings`
2. **Type check / compile** — `cargo check --all-targets`, debug + release + `--no-default-features`
3. **Style** — `cargo fmt --all -- --check`
4. **Unit tests** — both the default build **and** `--no-default-features`
   (the latter compiles `x11.rs` and actually *runs* the Issue-2 regression test)
5. **End-to-end** — real CLI journeys (`-c`, `--validate-rules`, `--list-callbacks`, `--list`, `--help`) in an **isolated `HOME` with scrubbed `XDG_CONFIG_HOME`** so the operator's real config is never touched
6. **PRD defect-absence checks** — static source assertions proving each fix is present

**Result: 23 passed, 0 failed, exit 0.**

---

## Per-Defect Findings (all verified fixed)

### Issue 1 — Hyprland "Show Window Information" (Major) ✅ FIXED
- **PRD claim:** dialog at `hyprland.rs:571,577` uses `class`; notifications use `initial_class`.
- **Current code:** `list_foreground_windows()` (the function feeding the dialog via `linux_tray.rs:384`) maps `c.initial_class.clone()` (line 571) and keys on `active.initial_class.clone()` (line 577). The notification path uses `initial_class` at lines 398/474/479/489. **They now agree.**
- **Audit:** a grep for any bare `.class`/`c.class`/`client.class` read in `hyprland.rs` (excluding `initial_class`/`app_class`) returns **nothing**.
- **Test gap:** no dedicated unit test (the function shells out to the live Hyprland IPC socket). Verified by code audit + it compiles into the default build. *Recommendation only — not a defect.*

### Issue 2 — X11 `WM_CLASS` parser (Major) ✅ FIXED — **regression test RUNS**
- **PRD claim:** `parse_wm_class` returns the instance (1st field) instead of the class (2nd) due to a leading-space index shift.
- **Current code** (`x11.rs:89`): splits on `,` (not `"`), trims + strips quotes, filters empties, then `.get(1).or_else(|| parts.first())` → **prefers the class**, falls back to instance only for degenerate single-field output. The comma-split makes a leading space/index shift impossible.
- **Regression test:** `parse_wm_class_returns_class_not_instance` asserts `Some("Firefox")` from input `"firefox", "Firefox"`. **Executed and passing** in the `--no-default-features` build on this host:
  ```
  test platforms::x11::tests::parse_wm_class_returns_class_not_instance ... ok
  ```

### Issue 3 — VID/PID change doesn't reset handshake (Minor) ✅ FIXED
- **PRD claim:** changing the VID/PID filter in Settings doesn't reset the handshake, so board B reuses board A's callback name→id map.
- **Current code:** all three save paths snapshot the pre-save VID/PID, write config, then conditionally reset:
  - `src/tray.rs:1002` (Windows) and `src/tray.rs:1921` (macOS) — `if merged.vendor_id != old_vid || merged.product_id != old_pid { reset_handshake_state(); perform_handshake(false); }`
  - `src/linux_tray.rs:307` and `:740` (`save_and_notify`) — same guard.
- **Total:** 6 `reset_handshake_state()` call sites; the guard condition is correct (fires exactly when VID/PID changed). The fix rebuilds the callback map for the newly-selected board.

### Issue 4 — Windows autostart path unquoted (Major) ✅ FIXED
- **PRD claim:** HKCU `Run` value written without quotes → spaced paths fail / unquoted-service-path vector.
- **Current code:**
  - `src/autostart.rs` `current_exe_wide()` wraps the path in `0x0022` (`"`) on both ends → layout `[0x0022 …path… 0x0022 0x0000]`. **9 occurrences of `0x0022`.**
  - `packaging/windows/inno/QMKonnect.iss:105` — `ValueData: """{app}\{#MyAppExeName}"""` (Inno triple-quote = one literal `"` each end → quoted path).
- **Regression tests:** `current_exe_wide_is_quoted` + `current_exe_wide_quotes_the_actual_exe_path` assert the quote layout and that the quoted content equals `current_exe()`. *(File is `cfg(windows)` — these run on Windows builds/CI, verified by audit on this host.)*

### Issue 5 — Windows title-length heuristic (Minor) ✅ FIXED
- **PRD claim:** `title.len() < 2` counts bytes → a 1-char emoji (4 bytes) is kept while 1-char ASCII is dropped.
- **Current code** (`windows.rs:308`): `window_info.title.chars().count() < 2 && !window_info.title.is_empty()` → counts **Unicode scalars**, so a 1-char emoji (1 scalar) is now ignored just like 1-char ASCII.
- **Regression test:** `test_ignore_one_char_emoji_title` — comment literally says *"BEFORE the fix this was KEPT (len()=4 >= 2). THIS IS THE REGRESSION CASE."* *(File is `cfg(windows)` — runs on Windows builds/CI.)*

---

## Build / Test / Lint Results

| Gate | Command | Result |
|------|---------|--------|
| Compile (all targets) | `cargo check --all-targets` | ✅ |
| Debug build | `cargo build --bin qmkonnect` | ✅ |
| Minimal build | `cargo build --no-default-features` | ✅ |
| Release build | `cargo build --release` | ✅ |
| Clippy | `cargo clippy --all-targets -- -D warnings` | ✅ 0 warnings |
| Format | `cargo fmt --all -- --check` | ✅ clean |
| Unit tests (default) | `cargo test --bin qmkonnect -- --test-threads=1` | ✅ **390 passed, 0 failed** |
| Unit tests (X11) | `cargo test --no-default-features --bin qmkonnect -- --test-threads=1` | ✅ **381 passed, 0 failed** |

---

## End-to-End Workflow Coverage (real binary)

User journeys drawn from `README.md` / `docs/usage.md`, run against the built
binary in an isolated `HOME` (so the real `~/.config/qmkonnect` is untouched):

| Workflow | CLI | Result |
|----------|-----|--------|
| Discover usage | `--help` | ✅ prints `Usage: qmkonnect [OPTIONS]` |
| List supported platforms | `--list` | ✅ `Linux (Hyprland)` |
| **First-run config creation** (README) | `-c` | ✅ creates `config.toml` + `rules.toml`; auto-discovery message shown |
| **Validate host rules** | `--validate-rules` (default) | ✅ `rules.toml valid: 0 rules` |
| Validate a real rule | `--validate-rules` w/ `[[rule]] match="firefox" layer=3` | ✅ `1 rule (1 with layer)` — correctly counted |
| **Reject malformed rules** | `--validate-rules` w/ `[[rule]] layer=3` (no `match`) | ✅ `missing field 'match'`, **exit 1** |
| Typed-command capability | `--list-callbacks` | ✅ prints callback name→id table |

> **Note on `--show-window-info`:** on this headless host it registers the SNI
> tray then blocks (no D-Bus session) — expected environment limitation, not a
> defect. The Hyprland/X11 window-detection paths require a live desktop session
> and a real QMK keyboard, so they are covered by unit tests + audit rather than
> live E2E here.

---

## Observations (NOT defects — informational, out of PRD scope)

These are minor polish notes surfaced during validation. **None are regressions,
none are PRD bugs, and none block release.**

1. **`rules.toml` unknown-table silent drop.** `RuleSet` declares `rules` with
   `#[serde(rename = "rule")]` and does **not** set `deny_unknown_fields`. A user
   who writes `[[rules]]` (plural) gets it silently ignored → `--validate-rules`
   reports `0 rules` instead of an error. The shipped template documents
   `[[rule]]` correctly, so this is by-design — but adding `deny_unknown_fields`
   to the deserializing structs would catch this class of typo at validation time.

2. **Platform-gated regression tests.** The regression tests for Issues 4 & 5
   live in `cfg(windows)` files (`autostart.rs`, `windows.rs`); Issue 2's test
   only compiles in the `--no-default-features` build. A **Linux-only** CI runner
   therefore executes neither. They *are* verified by audit here, and will run
   on Windows/macOS CI runners — but ensure the CI matrix includes those targets
   so these specific guards stay enforceable.

---

## Artifacts

- `validate.sh` — the executable validation harness (23 gates; `./validate.sh` or `./validate.sh --quick`).
- This report — `validation_report.md`.

*Both files are temporary validation artifacts (per task spec) and are not part
of the shipped application.*
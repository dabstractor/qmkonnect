# Research Notes — P1.M1.T2.S1: fix X11 WM_CLASS parser off-by-one + add unit tests

Repo: **`/home/dustin/projects/qmkonnect`**. Single-file bug fix in
**`src/platforms/x11.rs`** (Bug 2, PRD ID 2). No overlap with the sibling S1
(Bug 1, `src/platforms/hyprland.rs` — a different file).

## 1. The bug (verified against the live source)

`src/platforms/x11.rs`, inside `X11Monitor::get_active_window_info`, WM_CLASS
branch (current lines 67–75):

```rust
if line.starts_with("WM_CLASS") {
    if let Some(rest) = line.split_once('=').map(|(_, r)| r) {
        // WM_CLASS(STRING) = "instance", "Class"
        let quoted: Vec<&str> = rest.split('"').filter(|s| !s.is_empty()).collect();
        // Prefer the class (second element), fall back to instance.
        app_class = quoted
            .get(1)
            .or_else(|| quoted.first())
            .map(|s| s.to_string())
            .unwrap_or_default();
    }
}
```

For `rest = ' "instance", "Class"'` (the leading space comes from `= `):
- `split('"')` → `[" ", "instance", ", ", "Class", ""]`
- `filter(!is_empty)` → `[" ", "instance", ", ", "Class"]` (the leading `" "` and the `", "` separator are NON-empty, so they occupy indices 0 and 2)
- `.get(1)` → `"instance"` ← **the INSTANCE, not the class** (off-by-one caused by the leading space + comma separator surviving the filter)

So `WindowInfo::new(app_class, title)` is fed the instance (`firefox`) instead of
the class (`Firefox`), and that's what gets sent to the firmware + matched against
rules. The `_NET_WM_NAME` branch (L76–82) is UNAFFECTED — do not touch it.

## 2. The fix (extract a pure helper; verified byte-correct in /tmp)

Replace the split-on-quote logic with split-on-comma + trim, extracted into a
pure helper so it's unit-testable (x11.rs currently has **no** `#[cfg(test)]`
module — confirmed by grep):

```rust
/// Parse the **class** out of the `WM_CLASS` property's `= …` remainder.
///
/// `xprop` prints `WM_CLASS(STRING) = "instance", "Class"`. After
/// [`split_once('=')`](str::split_once) the caller passes
/// `rest = ' "instance", "Class"'`. Splitting on `,` (not `"`) means a leading
/// space or the `, ` separator can't shift the field index; then trim + strip
/// the quotes. Prefers the **class** (2nd field) and falls back to the instance
/// (1st field) for degenerate single-field output. `None` if no non-empty field.
fn parse_wm_class(rest: &str) -> Option<String> {
    let parts: Vec<&str> = rest
        .split(',')
        .map(|s| s.trim().trim_matches('"'))
        .filter(|s| !s.is_empty())
        .collect();
    parts.get(1).or_else(|| parts.first()).map(|s| s.to_string())
}
```

For `rest = ' "firefox", "Firefox"'`: split(',') → `[' "firefox"', ' "Firefox"']`
→ trim+trim_matches('"') → `['firefox', 'Firefox']` → `.get(1)` = `'Firefox'` ✓.

Call site collapses to one line:
```rust
app_class = parse_wm_class(rest).unwrap_or_default();
```
(`unwrap_or_default()` preserves the existing "empty string on parse failure"
behavior — `app_class` stays `String::new()` if `parse_wm_class` returns `None`.)

**Verified in `/tmp/wmclass_test`** (rustc 1.x, edition 2021): all assertions pass —
the exact bug input → `Some("Firefox")`; the full-line `split_once('=')`+parse path
→ `Some("Firefox")`; single-field → fallback to first; empty/whitespace/`"",""`
→ `None`; multi-word class `"Google Chrome"` → `Some("Google Chrome")`; AND the OLD
split-on-quote logic confirmed to return `Some("firefox")` (the bug, now fixed).

## 3. The cfg-gating constraint (the KEY validation gotcha)

`src/platforms/x11.rs` line 1: `#![cfg(all(target_os = "linux", not(feature = "hyprland")))]`.
- This is an **inner** attribute (`#!`) → it gates the **entire file**, including any
  `#[cfg(test)] mod tests` block inside it.
- `Cargo.toml`: `default = ["hyprland", "macos", "linux-tray"]` → the **default**
  build has `hyprland` ON → x11.rs is **compiled OUT** (and its tests with it).
- ⇒ `cargo test --bin qmkonnect` (default features) **never compiles x11.rs** — a
  syntax error in the fix would NOT surface, and the new tests would NOT run.
- ⇒ The fix + tests are validated ONLY via **`--no-default-features`** (turns hyprland
  OFF → x11.rs compiles on Linux). **Verified**: `cargo check --no-default-features
  --bin qmkonnect` → `Finished` exit 0 (the trayless+x11 target builds cleanly).
- Platform note: this Linux box can run them. On macOS/Windows the file is always
  cfg'd out (`target_os = "linux"` fails), so the tests run only on Linux regardless
  of features — a Linux env/VM is required to execute them.

So the validation command set is:
- `cargo test --bin qmkonnect --no-default-features parse_wm_class -- --nocapture`
  (the 3 new parser tests — MUST pass).
- `cargo clippy --no-default-features --bin qmkonnect -- -D warnings` (lint the x11 code).
- `rustfmt --edition 2021 --check src/platforms/x11.rs` (direct fmt check — `cargo fmt`
  with default features may not see the cfg'd-out file).
- `cargo build` (default features) — regression check (x11.rs absent; nothing else
  changes, so it must still build).

## 4. The 3 unit tests (verified expectations)

x11.rs has no test mod → add the first one. Private free fn `parse_wm_class` is
visible to the child `mod tests` via `use super::*;` (private items are visible to
descendant modules — no `pub` needed).

- **(a)** `parse_wm_class_returns_class_not_instance`: input `r#" "firefox",
  "Firefox""#` → `Some("Firefox")`. Also assert the full-line path
  (`split_once('=')` then parse) for `WM_CLASS(STRING) = "firefox", "Firefox"`.
- **(b)** `parse_wm_class_single_field_falls_back_to_first`: input `r#" "Navigator""#`
  → `Some("Navigator")` (degenerate single-field → first).
- **(c)** `parse_wm_class_empty_or_whitespace_is_none`: `""`, `"   "`, `r#" "", """#`
  → `None`.

(All three verified byte-correct in the /tmp repro.)

## 5. Placement & scope

- **File:** `src/platforms/x11.rs` ONLY.
- **Helper placement:** after the `impl WindowMonitor for X11Monitor` block (end of
  the impl section), before the new `#[cfg(test)] mod tests`. A private free fn.
- **Call site:** the WM_CLASS branch body (L67–75) collapses to
  `app_class = parse_wm_class(rest).unwrap_or_default();`.
- **DO NOT touch:** the `_NET_WM_NAME` branch (L76–82), `get_active_window_info`'s
  root-window/active-window logic, the poll loop, `start`/`stop`, or any other file.

## 6. Doc alignment (Mode B = none needed here)

The fix makes the code MATCH the docs, which already say "window class"
(`docs/configuration.md` L281, `docs/troubleshooting.md` L571). No doc edit is part
of THIS subtask (a separate P1.M2.T3 docs-sweep task verifies doc accuracy). The
bug-findings Documentation Impact Summary confirms: "Mode A: none … Mode B: a final
docs task should evaluate troubleshooting/configuration" — i.e. not this task.

## 7. Sibling contract (P1.M1.T1.S1 — being implemented in parallel)

S1 edits `src/platforms/hyprland.rs` (Bug 1, the `class` vs `initial_class` dialog
fix). It is a **different file** from x11.rs → **no file-level collision**. Both
land independently. Reference it only for the established PRP structure.
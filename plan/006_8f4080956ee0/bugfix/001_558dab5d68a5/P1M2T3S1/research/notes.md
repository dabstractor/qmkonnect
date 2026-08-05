# Research Notes — P1.M2.T3.S1 (docs/troubleshooting.md window-class guidance)

## Task
Evaluate whether `docs/troubleshooting.md` checklist item #3 ("Pattern matches
the real window class?", ~L571-577) needs a clarifying note after the Hyprland
(`initial_class`) and X11 (`WM_CLASS` class field) fixes. Surgical precision
only — do NOT rewrite the section.

## What the two prior fixes changed (the source of truth)

### Hyprland — P1.M1.T1.S1 (`src/platforms/hyprland.rs`)
`app_class` is now `initial_class` everywhere the identifier is produced:
- L398 `app_class: active_window.initial_class.clone()` (active-window path)
- L474/479 `active_window.initial_class` / `app_class: active_window.initial_class.clone()`
- L559-571 `list_foreground_windows()` → `.map(|c| (c.initial_class.clone(), c.title.clone()))`
- L577 `let key = (active.initial_class.clone(), active.title.clone())`

⚠️ `hyprctl clients` / `hyprctl activewindow` print the **`class`** field
prominently. For most apps `initial_class == class`, but they **diverge** for an
app that changes its class after launch — exactly the discrepancy PRD Issue 1
(h3.0) is titled: *"Hyprland 'Show Window Information' reports class, but the
keyboard receives initial_class."* A rule author cross-referencing `hyprctl`
would be misled.

### X11 — P1.M1.T2.S1 (`src/platforms/x11.rs`)
`parse_wm_class()` now returns the **class** (2nd field of
`WM_CLASS(STRING) = "instance", "Class"`), not the instance:
- L67 `app_class = parse_wm_class(rest).unwrap_or_default();`
- L80-89 `parse_wm_class` — docstring: "Prefers the **class** (2nd field) and
  falls back to the **instance**".
- Unit tests L205-240 lock it in (`"firefox", "Firefox"` → `Firefox`).

⚠️ `xprop WM_CLASS` prints BOTH values: `"instance", "Class"`. A user eyeballing
xprop must pick the **2nd** value. Before the fix QMKonnect used the 1st
(instance) — the bug.

### Unchanged platforms (for completeness / not a confusion trap)
- **macOS** `src/platforms/macos.rs` L277/325/334/374/413: `app_class` =
  `NSRunningApp.localizedName`. Doc-comment L334 explicitly:
  *"`class` is the app's `localizedName` — exactly the value QMKonnect sends as
  `application_class`"*. Matches intuition ("the app name").
- **Windows** `src/platforms/windows.rs`: `app_class` = Win32 window class
  (`GetClassName`). Stable, matches intuition.

## What the user actually sees (the authoritative value)
- **Verbose log (`qmkonnect -v`)** prints the real `app_class`/`title`:
  - X11 `src/platforms/x11.rs` L148-153: `"Window changed - Class: '{}', Title: '{}'"`.
  - Hyprland logs the same `window_state.app_class`/`title` around L435-442/471.
- **"Show Window Information" tray dialog** lists rows from
  `list_foreground_windows()` (`src/platforms/mod.rs` L85-93 dispatches per-OS):
  `(class, title)` tuples — i.e. `(initial_class, title)` on Hyprland,
  `(WM_CLASS-class, title)` on X11. Linux dialog: `src/linux_tray.rs` L383+;
  macOS/Windows: `src/tray.rs` L2028+/L2130+.

➡️ Both `-v` and the dialog show the **exact** identifier sent to the keyboard /
matched against `rules.toml`. The existing guidance ("check what QMKonnect
actually sees") is correct and self-sufficient.

## Current text (docs/troubleshooting.md L571-577)
```
3. **Pattern matches the real window class?** The matcher is class-only for a
   bare `match` string. Check what QMKonnect actually sees:
   ```bash
   qmkonnect -v | grep -i "window\|sending"     # the class\x1Dtitle string sent
   ```
   (or use the tray's "Show Window Information"). A `*chrome*` rule won't match a
   class reported as `Google Chrome` — adjust the pattern or use a `[class, title]`
   array.
```

## Other "class" mentions in the repo (scope boundaries — DO NOT touch)
- `README.md` L303: `{application_class}{GS}{window_title}` — generic, accurate.
- `docs/configuration.md` L281/L503/L510: "window class only" / `application_class` —
  generic, accurate.
- `docs/qmk-integration.md` L23: `{application_class}{GS}{window_title}` — generic.
- `docs/troubleshooting.md` L188: `{application_class}{GS}{window_title}` format
  example — generic, accurate.
- macOS-specific section L362 "Window titles not updating (app name only)" —
  accurate (Screen Recording permission).
All remain correct post-fix under the generic term. **Only** item #3 (L571-577)
benefits from a clarifying note. P1.M2.T3.S2 owns broader README/docs consistency
for the heuristic + autostart — do not overlap.

## Decision
**Add a concise clarifying note** (one inserted sentence) to item #3, because
the two named confusion traps (Hyprland `initial_class` vs `hyprctl`'s `class`;
X11 `WM_CLASS` class = 2nd field, not instance) are real and were the subject of
the bugs just fixed. The note reinforces the authoritative source (the `-v` /
dialog value is what's matched) and names both platform specifics. No rewrite,
no change to the existing example, no change to any other section/file.

## Doc tooling / style facts
- Jekyll/GitHub Pages site (front matter: `layout: default`, `permalink: /troubleshooting/`).
- NO markdown linter / vale / prettier config → validation is manual prose +
  structure checks (balanced code fences, preserved 3-space list indentation).
- Prose wraps ~75-80 cols (content inside the numbered item indented 3 spaces).
  Match that wrapping for the inserted text.
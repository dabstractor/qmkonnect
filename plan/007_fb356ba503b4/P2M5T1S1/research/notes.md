# Research Notes — P2.M5.T1.S1: X11 unconditional + Wayland gate

## 0. TL;DR — the target state is ALREADY PRESENT in the repo

This task's contract (make X11 unconditional on Linux + gate selection on
`$WAYLAND_DISPLAY` unset, verify 500 ms poll / WM_CLASS / list_foreground_windows
unchanged, Mode-A doc note) is **already satisfied** by the current `main`. The
code landed in two earlier, COMPLETE tasks:

- **P2.M1.T1.S1** (commit `74e7230`, "Replace compile-time Linux backend cfg with
  runtime dispatcher") — created `select_linux_backend` + the priority candidate
  list, with X11 ungated and always last.
- **P2.M1.T2.S2** (commit `3bf3304`, "Declare wayland/gnome/atspi Cargo features
  and document hidapi link nuance") — declared the optional backend features and
  made X11 unconditional (removed any feature gate).

Therefore **P2.M5.T1.S1 is an acceptance-gate / verification task**, NOT a
greenfield implementation. The PRP frames it that way: verify each contract
point with exact commands, confirm the docs note, and leave the (correct) code
in place. If verification finds a gap, the PRP gives the precise fix.

## 1. Contract point (a): X11 compiles unconditionally on Linux — DONE

- `src/platforms/x11.rs:1` → `#![cfg(target_os = "linux")]` (file-level gate is
  OS-only; **no feature flag**). Matches the `hyprland.rs` pattern.
- `src/platforms/mod.rs:13-19` → `mod x11;` declared **ungated** with the
  comment "X11 is unconditional on Linux now". No `#[cfg(feature = ...)]`.
- `src/platforms/linux.rs` `linux_backend_candidates()` → the X11 row
  `("x11", crate::platforms::x11::probe_available as ProbeFn)` has **no
  `#[cfg(feature = "…")]`** and the comment "X11 is unconditional on Linux
  (always last — lowest priority; never under Wayland via its own probe)".
- `construct_backend()` → the `"x11"` arm has **no feature cfg**.
- `Cargo.toml` `[features]` → there is **no `x11` feature** (only `wayland`,
  `gnome`, `atspi`, `hyprland`, `macos`, `linux-tray`). X11 needs no dep, so it
  needs no feature.

**Proof:** `cargo build --no-default-features --release` compiles cleanly with
X11 as the **sole** backend (verified — exit 0, 8.45 s). With every optional
feature dropped, X11 is still in the candidate list and still constructs.

## 2. Contract point (b): Wayland gate in the X11 probe — DONE

`src/platforms/x11.rs` `pub(crate) fn probe_available(_verbose)` enforces all
three gates in order:

1. `$DISPLAY` set AND non-empty → else `Err("$DISPLAY is not set")`.
2. `$WAYLAND_DISPLAY` **unset OR empty** → else `Err("Wayland session
   ($WAYLAND_DISPLAY set) — X11 is never selected under a Wayland compositor
   (XWayland focus is unreliable for native windows; PLATFORMS.md §6/§10)")`.
   (Invariant 11, ARCHITECTURE.md §10.)
3. `xprop` on PATH (via `which xprop`, NOT `xprop -version` which needs a live
   display) → else `Err("`xprop` not found on PATH (install xorg-xprop)")`.

`select_linux_backend` reaches this probe via the candidate list, so the gate is
enforced exactly once, at the single source of truth (`x11::probe_available`).
There is **no separate** Wayland check in `linux.rs` — and none is needed
(duplicating the gate would risk divergence).

**Proof:** 7 tests in `platforms::linux::select_tests` pass under
`--no-default-features`, including the headline
`x11_probe_err_when_wayland_display_set` (Invariant-11 regression sentinel) and
`select_forced_x11_under_wayland_is_loud_err` (forced path lists every probe
result).

## 3. Contract point (c): 500 ms poll + WM_CLASS — UNCHANGED

- Poll interval: `src/platforms/x11.rs` worker thread →
  `let poll_interval = Duration::from_millis(500);` (the documented value).
- WM_CLASS parse: `fn parse_wm_class(rest)` splits the `= …` remainder on `,`,
  prefers the **class** (2nd field), falls back to the **instance** (1st),
  returns `None` on no non-empty field. Hermetic unit tests
  (`parse_wm_class_returns_class_not_instance`,
  `parse_wm_class_single_field_falls_back_to_first`,
  `parse_wm_class_empty_or_whitespace_is_none`) lock the behavior.
- `0x0`/`0` active-window id ⇒ empty workspace ⇒ notifies empty `WindowInfo`
  (clears last). Fails **loudly** on missing `xprop` (`start()` checks
  `xprop -version`), never emits placeholder strings (#14).

**Nothing to change.** The verification is "the poll is still 500 ms and the
parse still prefers class" — both confirmed by reading the current source.

## 4. Contract point (d): list_foreground_windows() still works — DONE

- X11 has **no** `list_foreground_windows()` function (it is a fallback backend;
  the tray window-info feature was never wired for X11). This is the documented
  behavior — NOT a gap.
- `src/platforms/mod.rs::list_foreground_windows()` cfg ladder reaches, on Linux,
  the `wayland` arm (default features) → `hyprland` → `gnome`, then a final
  catch-all `#[cfg(not(any(...)))] return Vec::new();`. There is no `x11` arm
  and there should not be one (the contract says "verify it still works", i.e.
  "don't break it", not "add a new arm").
- Under `--no-default-features` (X11 the sole backend) the catch-all compiles
  and returns `Vec::new()` — confirmed by the green `--no-default-features`
  build. The fn is `#[allow(dead_code)]` when `linux-tray` is off, so the
  trayless minimal build stays warning-clean.

## 5. DOCS: Mode-A note in PLATFORMS.md §10 — ALREADY PRESENT

- `spec/PLATFORMS.md` §10 (lines 500–515): "The lowest-priority backend;
  compiled in on **every** Linux build (no longer `--no-default-features`-only).
  Selected only for genuine X11 sessions." + a dedicated bullet "Never selected
  under Wayland: the selector gates X11 on `$WAYLAND_DISPLAY` being **unset**,
  because under a Wayland compositor `$DISPLAY` is set by XWayland but its
  notion of focus is unreliable for native Wayland windows (§6 priority #5)."
- `spec/PLATFORMS.md` §6 priority table row #5: "`$DISPLAY` set **and
  `$WAYLAND_DISPLAY` unset** and `xprop` present — *never* under a Wayland
  compositor". Plus "Feature gating: ... X11 is unconditionally compiled on
  Linux. `--no-default-features` yields a trayless service build with only the
  X11 backend."
- `spec/ARCHITECTURE.md` §10 (Invariant 11, lines 460–461): "`select_linux_backend`
  gates X11 on `$WAYLAND_DISPLAY` being **unset**; XWayland sets `$DISPLAY` but
  reports focus unreliably for native Wayland windows".

→ **No doc edit is required** for the contract's Mode-A note. The only generated
omnibus doc, `docs/llms_full.txt`, is owned by **P2.M7.T2.S2** ("Regenerate
docs/llms_full.txt") — this task MUST NOT regenerate it.

## 6. CRITICAL: the main build is RED — but NOT from this task

`cargo build --release` currently fails with:

```
error[E0603]: module `gnome` is private
  --> src/runners/linux.rs:194:30
  --> src/platforms/mod.rs:22:1
22 | mod gnome;
```

Cause: the **parallel** task P2.M3.T2.S2 (GNOME first-run notification in
`src/runners/linux.rs`, git status shows it modified) reaches
`crate::platforms::gnome::…`, but `mod gnome;` is crate-private (no `pub`).
That sibling task owns the fix (either `pub(crate) mod gnome;` in mod.rs OR a
re-export, plus its own runners/linux.rs edits). **P2.M5.T1.S1 MUST NOT touch
`src/runners/linux.rs`, `mod gnome;`, or `gnome.rs`** — they are out of scope.

The X11 work is **independently green**: `cargo build --no-default-features`
(excludes gnome/wayland/atspi/hyprland, leaving X11 as the sole backend)
compiles cleanly. That is the correct verification path for THIS task and it
sidesteps the sibling breakage.

## 7. Scope boundary (files this task does NOT touch)

- `Cargo.toml` — no `x11` feature exists or is needed; P2.M1.T2.S2 owns features.
- `src/platforms/gnome.rs`, `mod gnome;` decl, `src/runners/linux.rs` — parallel
  P2.M3.T2.S2 / P2.M3.T2.S1 (COMPLETE gnome.rs consumed as pattern only).
- `src/platforms/atspi.rs` — parallel P2.M4.T1.S1 (the `atspi_probe` stub at
  linux.rs:180 still returns the fixed `Err`; that's P2.M4's to replace).
- `src/platforms/{wayland_ft,hyprland}.rs` — separate backends.
- `docs/llms_full.txt` — owned by P2.M7.T2.S2 (regenerate).
- `.github/workflows/*`, `PRD.md`, `tasks.json`, `.gitignore`.

## 8. Verification command matrix (all confirmed working on the dev box)

| Contract point | Command | Expected |
|---|---|---|
| (a) X11 unconditional | `cargo build --no-default-features --release` | exit 0 (X11 sole backend) |
| (a) no x11 feature | `grep -n '^x11\b' Cargo.toml \|\| echo NONE` | NONE |
| (a) mod ungated | `grep -n 'mod x11' src/platforms/mod.rs` | `mod x11;` (no cfg) |
| (b) Wayland gate code | `grep -n 'WAYLAND_DISPLAY' src/platforms/x11.rs` | the gate block |
| (b) Wayland gate test | `cargo test --no-default-features --bin qmkonnect -- --test-threads=1 select_tests::x11_probe_err_when_wayland_display_set` | ok |
| (b) forced-under-wayland | `cargo test --no-default-features --bin qmkonnect -- --test-threads=1 select_tests::select_forced_x11_under_wayland_is_loud_err` | ok |
| (c) 500 ms poll | `grep -n 'from_millis(500)' src/platforms/x11.rs` | the worker poll |
| (c) WM_CLASS class-first | `cargo test --no-default-features --bin qmkonnect parse_wm_class_returns_class_not_instance` | ok |
| (d) ladder compiles | (covered by the `--no-default-features` build above) | exit 0 |
| DOCS §10 | `sed -n '500,515p' spec/PLATFORMS.md` | the "lowest-priority… never under Wayland" note |
| DOCS §6 #5 | `grep -n 'WAYLAND_DISPLAY' spec/PLATFORMS.md` | priority row + §10 |
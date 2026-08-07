# Research Notes — P2.M7.T1.S1 (Update installation/platforms/troubleshooting docs for F16/F17)

> Docs-only task. The authoritative source is the **internal `spec/` directory**
> (already ahead of the implementation). External research is for citable
> user-facing URLs only. No code changes.

## 1. Contract (from item_description)

Update user-facing docs so the PRD §3/§5/§12 broad-Linux story (F16 cross-DE
monitor + F17 universal autostart) is reflected:
- (a) **README.md** — distribution/Linux section: supported DEs/compositors + GNOME extension note.
- (b) **docs/installation.md** — Linux section: backend auto-selection, GNOME extension install steps, XDG autostart for non-systemd.
- (c) **docs/troubleshooting.md** — "wrong windows on Wayland" (GNOME extension), "no tray icon on GNOME" (AppIndicator, LINUX.md §7.4), "AT-SPI best-effort" caveats, a11y-enable requirement.
- (d) **docs/platforms/** — **DOES NOT EXIST** (confirmed: no `docs/platforms/`, no `docs/platforms.md`). Contract point (d) is **N/A**. The cross-DE detail lives in `spec/PLATFORMS.md` and is surfaced into installation.md / troubleshooting.md.

OUTPUT: README.md, docs/installation.md, docs/troubleshooting.md.

## 2. Stale "Hyprland only" strings to fix (grep sweep, exact lines)

```
README.md:21:            - Linux: Arch/Hyprland only
docs/installation.md:27:| **Linux** (Hyprland) | binary / Arch PKGBUILD | AUR · Nix |
docs/installation.md:99:### Linux (Hyprland Only)
docs/installation.md:101:**Note**: QMKonnect currently only supports Hyprland on Linux...
docs/troubleshooting.md:203:#### Linux (Hyprland Only)
docs/troubleshooting.md:214:**Note**: Only Hyprland is supported on Linux...
docs/troubleshooting.md:402:**Note**: Only Hyprland is supported on Linux...
docs/configuration.md:74:# (Hyprland only) periodic active-window poll interval (ms).
docs/configuration.md:221:| `poll_interval_ms` | `0` | (Hyprland only) ... |
```

→ 3 in-scope files (README, installation, troubleshooting) are MANDATORY.
`configuration.md` is **adjacent/unowned** (no P2.M7 task touches it) → optional
consistency edit; its `(Hyprland only)` note is technically still correct (only
the Hyprland IPC backend consumes `poll_interval_ms`) but reads as "Linux =
Hyprland only" which contradicts the new story. Low-risk optional clarifying edit.

## 3. Authoritative backend matrix (from spec/PLATFORMS.md §6 + §7.2 + §9 + §10)

`select_linux_backend(verbose)` probes in **priority order**, first-available-wins,
all compiled in by default (`default = ["wayland","gnome","atspi","hyprland",
"linux-tray"]`; X11 unconditional on Linux):

| # | Backend | Feature | Availability probe | Coverage |
|---|---|---|---|---|
| 1 | foreign-toplevel (Wayland) | `wayland` | `$WAYLAND_DISPLAY` + `zwlr_foreign_toplevel_manager_v1` global | Hyprland, Sway, Niri, River, Labwc, Wayfire, **KDE Plasma 6 (KWin)**, **COSMIC** |
| 2 | GNOME (Shell extension) | `gnome` | D-Bus name `io.mulletware.QMKonnect` owned | **GNOME** (Mutter implements neither foreign-toplevel protocol) |
| 3 | Hyprland (IPC) | `hyprland` | `$HYPRLAND_INSTANCE_SIGNATURE` + socket | Hyprland legacy fallback (#1 supersedes it) |
| 4 | AT-SPI (a11y bus) | `atspi` | `org.a11y.Bus` owned / `$ATSPI_BUS_ADDRESS` | **best-effort** (see §9 limitations) |
| 5 | X11 | *(always on Linux)* | `$DISPLAY` set **and `$WAYLAND_DISPLAY` unset** and `xprop` present | XFCE, MATE, Cinnamon, Budgie, LXQt (the X11 DE tail) |

- **Config override**: `[linux] backend = "foreign-toplevel"|"gnome"|"hyprland"|"atspi"|"x11"|"auto"` (default `auto`). `LinuxConfig { backend, gnome_poll_interval_ms }` in `src/core/mod.rs` (confirmed exists).
- **Verbose logging**: `qmkonnect -v` prints each candidate, its probe result, and the chosen backend → "why did it pick X?" is always answerable. (Confirmed: `src/platforms/linux.rs:103 select_linux_backend` with `println!("select_linux_backend: probing '{}'…", ...)`.)
- **No-backend fallback**: if every probe fails (e.g. GNOME on Wayland, extension uninstalled, a11y off) → `select_linux_backend` returns `Err`; runner still starts tray + device-status poll + HID pipeline; emits no window events. On GNOME a one-shot `notify-send` fires pointing to the extension (§8.4).

### AT-SPI limitations (spec/PLATFORMS.md §9) — to surface in troubleshooting
- `app_class` = app's readable **Name** (not `WM_CLASS`) → usually fine (`"Firefox"`) but **inconsistent for Electron/sandboxed apps** (`"python3"`, `"chrome"`, or empty).
- Titles vary (focused *accessible*, not toplevel).
- Apps that don't expose a11y (some games, some Qt apps without the bridge) are invisible.
- **Use the GNOME Shell extension for reliable GNOME support.**
- Most distros ship a11y **OFF** until the user enables "Assistive Technology / Screen Reader" — document that enabling a11y is REQUIRED for this backend.

## 4. GNOME-specific facts (two SEPARATE problems, both already solved in code)

These are the headline troubleshooting additions. From spec/LINUX.md §7.4 + spec/PLATFORMS.md §8:

### 4a. No tray icon on GNOME (LINUX.md §7.4)
Stock GNOME dropped SNI/AppIndicator support → the ksni tray item is **invisible**
on a default GNOME session (daemon still runs headless). Two honest options:
1. **Install the *AppIndicator and KStatusNotifierItem Support* GNOME extension** → ksni's item renders in the top bar. (External: https://extensions.gnome.org/extension/615/appindicator-support/ ; upstream https://github.com/ubuntu/gnome-shell-extension-appindicator ; distro pkg `gnome-shell-extension-appindicator` on Arch/Debian/Fedora.)
2. **Run trayless** — daemon, device-status, rules/settings fully functional without the icon; only the click menu is unavailable (use CLI flags `--list-devices`, `--validate-rules`, etc.).
> Do NOT build a GNOME-native tray. Window detection (Shell extension) and tray (AppIndicator) are solved separately.

### 4b. Wrong/no windows on GNOME (PLATFORMS.md §8) — the GNOME Shell extension
GNOME (Mutter) implements neither foreign-toplevel protocol and exposes no client
API → active window is read **inside gnome-shell** by the `qmkonnect@mulletware`
extension and republished over D-Bus. The app **links to it but cannot load it**
— it's a **user-installed dependency**. D-Bus: name `io.mulletware.QMKonnect`,
path `/io/mulletware/QMKonnect`, iface `io.mulletware.QMKonnect.WindowMonitor`
(method `GetActiveWindow`→(ss), signal `ActiveWindowChanged`(ss)). First-run: if
GNOME + name unowned → one-shot `notify-send` → "install the extension". Without
it, the AT-SPI backend may run best-effort (inconsistent app names) or nothing.

## 5. Autostart / F17 (spec/LINUX.md §6.1/§6.3) — coordination with P2.M6.T1.S1

**P2.M6.T1.S1 (parallel, in progress) ALREADY edits docs/installation.md** — it
adds a `### Autostart at login` subsection (systemd service vs XDG `.desktop`,
the trade-off, disable via `Hidden=true`, wlroots/dex caveat). **This task must
NOT recreate or move that subsection** — treat it as a fixed existing anchor.
This task's installation.md contribution re: autostart is limited to: ensuring
the broad cross-DE narrative references both start paths and that the "Other
Linux Distributions" manual-install block is consistent. The detailed autostart
prose is OWNED by P2.M6.T1.S1.

Key F17 facts (for cross-references, not to duplicate): `/etc/xdg/autostart/
qmkonnect.desktop` (NoDisplay=true; `Hidden=true` to disable per-user);
load-bearing on non-systemd distros (MX/Artix/Void/Gentoo); belt-and-suspenders
on systemd (single-instance dedupe owned by tray/runner, not launcher); pure
wlroots compositors (Sway/Hyprland w/o session manager) need `dex` or systemd
`xdg-autostart-generator`.

## 6. External citations (for user-facing markdown)

| What | URL | Exact string to cite |
|---|---|---|
| AppIndicator GNOME extension | https://extensions.gnome.org/extension/615/appindicator-support/ | "AppIndicator and KStatusNotifierItem Support" |
| AppIndicator upstream | https://github.com/ubuntu/gnome-shell-extension-appindicator | — |
| Enable AT-SPI (GNOME) | https://www.freedesktop.org/wiki/Accessibility/AT-SPI2/ | `gsettings set org.gnome.desktop.interface toolkit-accessibility true` |
| XDG autostart spec | https://specifications.freedesktop.org/autostart-spec/autostart-spec-latest.html | NoDisplay vs Hidden; per-user overrides system |
| Desktop Entry spec | https://specifications.freedesktop.org/desktop-entry-spec/latest/ | key semantics |
| wlr-foreign-toplevel | https://wayland.app/protocols/wlr-foreign-toplevel-management-unstable-v1 | (which compositors implement it) |

## 7. Scope boundaries (do NOT touch)

- `spec/*.md` — human-owned specs (read-only; the IMPLEMENTATION references them, doesn't edit).
- `PRD.md`, `tasks.json`, `prd_snapshot.md`, `.gitignore` — orchestrator-owned.
- `docs/llms_full.txt` — **owned by P2.M7.T2.S2** (the omnibus regen). DO NOT regenerate.
- The `### Autostart at login` subsection in docs/installation.md — **owned by P2.M6.T1.S1**. Preserve it.
- Any Rust source — this is a docs task.
- `packaging/*` — no packaging changes (P2.M6.T1.S1 + P1.M7 own those).
- `.github/workflows/*` — no CI changes (P2.M7.T2.S1 owns the GNOME-extension-zip + Nix CI jobs).

## 8. Validation approach (docs-only → no cargo)

No Rust build. Gates = grep checks (stale strings gone, new sections present),
markdown structure sanity (heading hierarchy), and internal-link integrity
(spec cross-refs + installation↔troubleshooting links). Optionally
`markdownlint` if available. The parallel `cargo build` redness (private `mod
gnome;` reach, per P2.M6.T1.S1 GOTCHA-1) is irrelevant — no code touched.
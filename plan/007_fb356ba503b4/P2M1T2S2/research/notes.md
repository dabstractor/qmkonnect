# Research Notes — P2.M1.T2.S2: Add wayland/gnome/atspi Cargo features, X11 unconditional, hidapi nuance

**Source of truth:** `spec/PACKAGING.md` §2 (the authoritative feature table — quoted
verbatim in §1 below), `spec/ARCHITECTURE.md` invariant 12 (the hidapi link nuance),
and the production code (`Cargo.toml`, `.cargo/config.toml`, `src/platforms/{mod,x11,linux}.rs`).
This file records the verified CURRENT state, the locked target, and the gotchas.

---

## 1. The authoritative target feature table (spec/PACKAGING.md §2, verbatim)

```toml
[features]
default   = ["wayland", "gnome", "atspi", "hyprland", "macos", "linux-tray"]
# Linux window-monitor backends (runtime-selected by select_linux_backend,
# PLATFORMS.md §6). All default-on so a single binary works everywhere;
# turn a backend off to shrink the binary / drop a dep.
wayland   = ["dep:smithay-client-toolkit", "dep:wayland-client"]   # foreign-toplevel (covers Hyprland/Sway/Niri/wlroots/KDE/COSMIC)
gnome     = ["dep:zbus"]                                            # GNOME Shell-extension D-Bus client
atspi     = ["dep:atspi"]                                           # a11y-bus fallback
hyprland  = ["dep:hyprland"]                                        # legacy Hyprland-IPC backend (superseded by wayland)
linux-tray = ["dep:ksni", "dep:gtk"]                                # StatusNotifierItem tray
macos     = ["dep:objc", "dep:core-foundation", "dep:core-graphics", "dep:dispatch"]
```

Companion bullets from the same section:
- Plain `cargo build --release` produces the full app with every Linux backend + a tray.
- `--no-default-features` yields the minimal trayless service build (X11-only monitor).
- **Linux hidapi link nuance (must-preserve):** Arch ships `libhidapi-hidraw` separate
  from `libhidapi-libusb`, so the **Arch PKGBUILD links `-lhidapi-hidraw`** explicitly
  (usage/usage_page matching requires the hidraw backend). Debian/Ubuntu and Fedora ship
  a *unified* hidapi (≥0.14) that folds both backends into one `libhidapi.so` and
  auto-selects hidraw at runtime, so the **.deb and .rpm builds must NOT pass
  `-lhidapi-hidraw`**. (Same note as the Nix flake's `hidrawFlag` caveat.)

---

## 2. Verified CURRENT state (Cargo.toml)

### 2a. Current `[features]` block (the BEFORE)

```toml
[features]
default = ["hyprland", "macos", "linux-tray"]
hyprland = ["dep:hyprland"]
macos = ["dep:objc", "dep:core-foundation", "dep:core-graphics", "dep:dispatch"]
linux-tray = ["dep:ksni", "dep:gtk"]
```

NO `wayland` / `gnome` / `atspi` features exist yet.

### 2b. Current Linux target deps

```toml
[target.'cfg(target_os = "linux")'.dependencies]
hyprland = { version = "0.4.0-beta.2", optional = true }
libxdo = "0.6"
tempfile = "3.0"
libc = "0.2"
ksni = { version = "0.3", optional = true, features = ["blocking"] }
gtk = { version = "0.18", optional = true }
```

NO `smithay-client-toolkit` / `wayland-client` / `zbus` / `atspi` declared yet.

### 2c. Current `[dependencies]` (the `hidapi` line — documentation anchor)

```toml
hidapi = "2.6"
```

No comment near it about the link nuance today.

### 2d. Current `.cargo/config.toml` (the WHOLE file)

```toml
# Statically link the C runtime (UCRT + vcruntime) into the Windows MSVC binary ...
[target.'cfg(all(target_os = "windows", target_env = "msvc"))']
rustflags = ["-C", "target-feature=+crt-static"]
```

There is NO global `-lhidapi-hidraw` rustflag — intentionally. There is also no
comment explaining WHY there is none. (§4 below documents this is the gap to fill.)

---

## 3. X11 is ALREADY unconditional in the working tree (part c = VERIFICATION)

The task says "X11 is compiled unconditionally on Linux (no feature gate) — update
#[cfg] attributes." Inspecting the tree, this is **already done** (by the parallel
P2.M1.T1.S1 item / a prior commit). Confirmed at three sites:

1. **`src/platforms/x11.rs:1`** — the inner attribute is a TARGET gate only, NOT a
   feature gate:
   ```rust
   #![cfg(target_os = "linux")]
   ```
   There is no `feature = "x11"` / `feature = "wayland"` gate on the file. (The only
   other `#[cfg]` in the file is `#[cfg(test)]` at line 242 for its test module.)

2. **`src/platforms/mod.rs`** — the module declaration is target-gated, with an
   explanatory comment already present:
   ```rust
   // X11 is unconditional on Linux now: the runtime selector `select_linux_backend`
   // probes it (last in priority order, never under Wayland). PLATFORMS.md §6/§10.
   #[cfg(target_os = "linux")]
   mod x11;
   ```

3. **`src/platforms/linux.rs` (`select_linux_backend`)** — the X11 candidate is added
   to the probe list UNCONDITIONALLY (no `#[cfg(feature)]`), always last:
   ```rust
   // X11 is unconditional on Linux (always last — lowest priority; never under
   // Wayland via its own probe).
   { name: "x11", probe: crate::platforms::x11::probe_available, ... }
   ```
   And the constructor arm `"x11" => Ok(Box::new(crate::platforms::x11::X11Monitor::new(verbose)))`
   is likewise ungated.

**grep confirmation:** `grep -rn 'feature.*x11\|x11.*feature' src/` returns nothing —
there is no feature gate on X11 anywhere. **Part (c) is a NO-OP verification step:**
the implementing agent must CONFIRM (re-run the grep + eyeball the three sites) that
X11 remains target-gated only, and MUST NOT introduce a feature gate on it. ARCHITECTURE
invariant 11 ("Never select X11 under Wayland") is enforced at RUNTIME by `select_linux_backend`
gating X11's probe on `$WAYLAND_DISPLAY` being unset — NOT by a compile-time feature.
So making X11 a feature would be wrong.

---

## 4. The hidapi link nuance — exact file:line evidence (part d)

### 4a. The invariant (spec/ARCHITECTURE.md:464-469, invariant 12)

> **12. Hidapi link differs per distro.** Arch links `-lhidapi-hidraw` (separate
> lib); Debian/Ubuntu/Fedora (unified hidapi ≥0.14) must **not** pass that flag (the
> unified lib auto-selects the hidraw backend at runtime). Getting this wrong breaks
> usage/usage_page matching on the .deb/.rpm. (`PACKAGING.md` §2.)

### 4b. Where the flag IS passed (Arch + Nix)

- `packaging/linux/arch/PKGBUILD:25` — `RUSTFLAGS="-C link-arg=-lhidapi-hidraw" cargo build --release`
  (source build; comment at :23 "against hidapi-hidraw (not hidapi-libusb)").
- `packaging/linux/aur/PKGBUILD:8` — same flag (binary/source AUR, comment).
- `packaging/linux/aur/README.md:17` — documents the flag.
- `flake.nix:27-55,97` — `hidrawFlag = "-C link-arg=-lhidapi-hidraw";` then
  `RUSTFLAGS = hidrawFlag;` in the dev + default shells. **BUT** `packaging/nix/README.md:115-119`
  + `flake.nix:32-37` document the fallback: if Nixpkgs ships the unified hidapi
  (>=0.14) and `nix build` fails with `cannot find -lhidapi-hidraw`, DROP the
  `hidrawFlag` line (the unified lib auto-selects hidraw).

### 4c. Where the flag must NOT be passed (.deb / .rpm — P1.M7, not yet built)

`spec/PACKAGING.md` (the canonical spec) is explicit:
- §4.3 (`.deb` via cargo-deb): "Build: build the binary **without** `-lhidapi-hidraw`
  (Debian's unified hidapi…)" (line ~178).
- §4.4 (`.rpm` via cargo-generate-rpm): "Build: build the binary **without**
  `-lhidapi-hidraw` (Fedora's unified…)" (line ~231); RPM metadata comment at :216
  "Unified hidapi on Fedora/RHEL ⇒ do NOT add an -lhidapi-hidraw link flag (§2)."
- §4.8 summary table (lines 315-317):
  | distro | runtime deps | link note |
  | Arch | `hidapi libusb zenity …` | link `-lhidapi-hidraw` |
  | Debian/Ubuntu/Mint | `libhidapi-hidraw0 …` | unified hidapi; **no** `-lhidapi-hidraw` |
  | Fedora/RHEL/Rocky | `hidapi …` | unified hidapi; **no** `-lhidapi-hidraw` |

**The .deb/.rpm packaging does NOT exist yet** (`packaging/debian/` and
`packaging/rpm/` are absent; P1.M7 is "Researching"). So THIS task cannot edit those
build scripts; instead it must DOCUMENT the nuance in-repo (Cargo.toml comment +
.cargo/config.toml comment) so the P1.M7 agent (and anyone tempted to add a global
link flag) sees it. The documentation is the deliverable for part (d).

### 4d. Why a global `.cargo/config.toml` rustflag would be WRONG

A global `[target.'cfg(target_os = "linux")'] rustflags = [... "-C" "link-arg=-lhidapi-hidraw"]`
would be inherited by EVERY Linux build — including the future .deb/.rpm builds and
the Nix flake. On unified-hidapi distros (Debian/Ubuntu/Fedora, modern Nixpkgs) the
separate `libhidapi-hidraw.so` does not exist, so the link would fail with
`cannot find -lhidapi-hidraw` — or worse, on a system with a stale separate lib it
would link the wrong backend and silently break usage/usage_page matching. **The
flag MUST stay per-channel (env `RUSTFLAGS` in the Arch PKGBUILD / Nix `hidrawFlag`),
never global.** The `.cargo/config.toml` documentation comment exists to preempt an
agent "helpfully" adding it globally.

---

## 5. The four new optional deps — versions + MSRV (part a/b)

The deps are declared `optional = true` under `[target.'cfg(target_os = "linux")']`
and wired to their feature via `dep:`. MSRV floor is **Rust 1.88** (`Cargo.toml`
`rust-version = "1.88"`; image 0.25.x is the floor). All picks are ≤ 1.88 MSRV:

| crate | version | role | MSRV | notes |
|---|---|---|---|---|
| `smithay-client-toolkit` | `"0.20"` | `wayland` feature | **1.86.0** (crates.io) | toolkit atop wayland-client; < 1.88 ✓ |
| `wayland-client` | `"0.31"` | `wayland` feature | ~1.74 | matches SCTK 0.20's wayland-client major |
| `zbus` | `"5"` | `gnome` feature | ~1.77 | pure-Rust D-Bus; `atspi` 0.30 also uses zbus 5 ⇒ unified |
| `atspi` | `"0.30"` | `atspi` feature | ~1.77 | pure-Rust, zbus-based AT-SPI2 (odilia-dev) |

**Compatibility:** `gnome` (`zbus` 5) and `atspi` (which depends on `zbus` 5) share
the same zbus major, so cargo unifies them — no two-major conflict. If a future `atspi`
bump moves to zbus 6, the `gnome` `zbus` line must move with it (cargo will error
clearly; align the majors). **System build deps:** zbus + atspi are pure Rust (no
system build dep). The wayland pair (`wayland-client` + `smithay-client-toolkit`)
links `libwayland` via `wayland-sys`; on a typical Linux desktop dev box it is present,
but a headless box may need `libwayland-dev` (Debian) / `wayland-devel` (Fedora) /
`wayland` (Arch) — see GOTCHA-3.

**`dep:` syntax is already used** (`hyprland = ["dep:hyprland"]`), edition 2021 +
Rust 1.88 fully support it, and `dep:` means enabling `wayland` does NOT also create
an implicit same-named feature beyond the two deps. Good.

### 5a. CRITICAL build-time implication (GOTCHA-2): enabling in `default` COMPILES them

`default = […, "wayland", "gnome", "atspi", …]` ⇒ a plain `cargo build` (default
features) resolves + compiles all four crates — even though **no source code uses
them yet** (the backend modules `wayland_ft.rs` / `gnome.rs` / `atspi.rs` are P2.M2 /
P2.M3 / P2.M4 and don't exist). Cargo does NOT skip compilation of enabled optional
deps just because nothing `use`s them; an enabled optional dep is in the resolved
graph and gets built. Consequences:
1. The dev/CI build gains the compile cost of zbus + atspi + the wayland pair
   (transient; only paid once per clean build).
2. If the build host lacks `libwayland` (for `wayland-sys`), `cargo build` fails on
   the wayland crates BEFORE any backend code exists. Fix: install the system dev
   package (GOTCHA-3), or verify with `--no-default-features --features macos,linux-tray`
   as an escape hatch.
3. This is the intended transitional state (PACKAGING.md §2 mandates all-three-in-
   default so "a single binary works everywhere" once the backends land). Do NOT
   "optimize" by leaving them out of `default` — that violates the spec.

### 5b. Runtime implication (informational, NOT a bug): graceful probe failures

`select_linux_backend` (P2.M1.T1.S1, in the tree) already has feature-gated probe
stubs:
```rust
#[cfg(feature = "wayland")] fn wayland_probe(_) -> Result<(),String> { Err("…not yet implemented (P2.M2)".into()) }
#[cfg(feature = "gnome")]   fn gnome_probe(_)   -> Result<(),String> { Err("…not yet implemented (P2.M3)".into()) }
#[cfg(feature = "atspi")]   fn atspi_probe(_)   -> Result<(),String> { Err("…not yet implemented (P2.M4)".into()) }
```
Today (features absent) these stubs are compiled OUT. After THIS task (features in
`default`), the stubs ARE compiled, so `select_linux_backend` probes wayland → gnome
→ atspi first, each returns Err, the verbose log prints the "not yet implemented"
messages, and selection falls through to **hyprland** (real) then **x11** (real
fallback). **No regression** — the app still works; it just logs three graceful
probe failures en route to hyprland/x11. This is the designed transitional state
until P2.M2–P2.M4 land. (Do not "fix" the log noise by removing the stubs — they
are the contract surface the backend tasks replace.)

---

## 6. Gotchas summary

- **GOTCHA-1 (X11 already unconditional — DO NOT re-gate):** part (c) is a
  VERIFICATION step. `x11.rs:1` is `#![cfg(target_os="linux")]`, `mod.rs` gates by
  target_os, `select_linux_backend` adds the x11 candidate unconditionally. Confirm
  with `grep -rn 'feature.*x11\|x11.*feature' src/` (must be empty) and DO NOT add
  a feature gate. X11-under-Wayland is prevented at RUNTIME (invariant 11:
  `$WAYLAND_DISPLAY` unset), not compile-time.
- **GOTCHA-2 (default-build now compiles 4 heavy crates):** adding wayland/gnome/
  atspi to `default` means `cargo build` compiles them even with no `use` sites yet.
  Verify `cargo build --bin qmkonnect` succeeds on the dev box; this is the spec'd
  state, not a bug.
- **GOTCHA-3 (wayland needs libwayland):** the `wayland-client`/`smithay-client-toolkit`
  pair links `libwayland` via `wayland-sys`. If `cargo build` fails on a wayland-sys
  link/bindgen step, install `libwayland-dev` (Debian) / `wayland-devel` (Fedora) /
  `wayland` (Arch). zbus + atspi are pure Rust (no system build dep).
- **GOTCHA-4 (NO global hidapi rustflag):** never add
  `[target.'cfg(target_os="linux")'] rustflags=[…"-lhidapi-hidraw"]` to
  `.cargo/config.toml`. It would break .deb/.rpm/Nix unified-hidapi builds. The flag
  stays per-channel (Arch PKGBUILD env RUSTFLAGS, Nix `hidrawFlag`). Document the
  intentional absence with a comment, not a rustflag.
- **GOTCHA-5 (zbus major must match atspi):** `gnome` uses `zbus` directly; `atspi`
  uses zbus too. Both are on zbus 5 today ⇒ unified. If you bump one, bump the other
  to the same major or cargo will fail with a two-major conflict.
- **GOTCHA-6 (MSRV 1.88 vs SCTK 0.20 MSRV 1.86):** SCTK 0.20's MSRV (1.86) is below
  the project floor (1.88) ⇒ OK. But a future SCTK bump above 1.88 forces a project
  MSRV bump. Note in a comment.
- **GOTCHA-7 (keep `dep:` syntax):** use `wayland = ["dep:smithay-client-toolkit",
  "dep:wayland-client"]` (NOT `wayland = ["smithay-client-toolkit", "wayland-client"]`).
  The bare-name form creates an implicit same-named feature AND enables the dep; the
  `dep:` form enables ONLY the dep. The existing `hyprland`/`linux-tray`/`macos`
  features all use `dep:` — match them. (spec/PACKAGING.md §2 uses `dep:`.)
- **GOTCHA-8 (no source-code changes beyond verification):** this task edits
  `Cargo.toml` (+ optionally `.cargo/config.toml` comments) ONLY. It does NOT create
  `wayland_ft.rs`/`gnome.rs`/`atspi.rs` (P2.M2–P2.M4), does NOT touch
  `select_linux_backend` (P2.M1.T1.S1 owns it), does NOT touch `LinuxConfig`
  (P2.M1.T2.S1 owns it). The `#[cfg(feature="…")]` probe stubs in `linux.rs` already
  exist and become live once the features are declared — no edit needed there.
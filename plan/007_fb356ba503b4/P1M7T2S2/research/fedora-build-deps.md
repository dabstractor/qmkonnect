# Research — Fedora build-dependency translation for the `.rpm` CI job

## The core finding (drives the whole PRP)

`spec/PACKAGING.md §9` and the work-item contract both summarize the `.rpm` CI
job's dnf step as **3 packages**: `hidapi-devel libxdo-devel pkg-config`.

That list is **insufficient to `cargo build --release` the QMKonnect binary.**
It is only the *packaging-level* dependency intent (what `cargo generate-rpm`
records as Requires). The binary itself links a much larger set.

### Proof (the `.deb` sibling, already landed — P1.M7.T1.S2)

`.github/workflows/release.yml`'s `deb` job installs the **full** apt build set,
NOT the 3 the spec §9 summary lists:

```yaml
sudo apt-get install -y --no-install-recommends \
  build-essential pkg-config \
  libgtk-3-dev libglib2.0-dev libayatana-appindicator3-dev \
  libx11-dev libxcb1-dev libxdo-dev libhidapi-dev libudev-dev
```

The spec §9 *also* lists only 3 for `.deb` (`libhidapi-dev libxdo-dev
pkg-config`) — but the REAL job installs the full set. The same is true for
`.rpm`: the spec §9 / contract 3-pack is the summary; the real job must install
the full Fedora `-devel` set or `cargo build --release` fails (missing
gtk3.h, X11, appindicator, libudev.h, …).

### Why the full set is needed — Cargo.toml `[features]`

```toml
default = ["wayland", "gnome", "atspi", "hyprland", "macos", "linux-tray"]
linux-tray = ["dep:ksni", "dep:gtk"]      # gtk-rs 0.18 → needs gtk3-devel
wayland    = ["dep:smithay-client-toolkit", "dep:wayland-client"]
gnome      = ["dep:zbus"]
atspi      = ["dep:atspi"]
```

`cargo build --release` (default features) compiles **gtk-rs, ksni, smithay
client toolkit, zbus, atspi** + the always-on **tao / tray-icon / libxdo /
hidapi**. Concretely the linker needs:

| Rust dep            | System lib needed (Fedora `-devel`)            |
|---------------------|------------------------------------------------|
| gtk (gtk-rs 0.18)   | `gtk3-devel` (+ pulls `glib2-devel`, pango, …) |
| glib (transitive)   | `glib2-devel`                                  |
| tray-icon / appind. | `libappindicator-gtk3-devel` (see gotcha)      |
| tao (X11)           | `libX11-devel`                                 |
| tao (xcb)           | `libxcb-devel`                                 |
| libxdo              | `libxdo-devel`                                 |
| hidapi (shared)     | `hidapi-devel` (pkg-config `hidapi`)           |
| udev                | `systemd-devel` (provides `libudev.h`)         |
| C linker (cc-rs)    | `gcc`                                          |
| version parse       | `jq`                                           |
| checkout needs git  | `git` (container ships none)                   |
| TLS for crates fetch| `ca-certificates` (belt+suspenders)            |

### Fedora `-devel` translation of the deb apt set

```
build-essential                      → gcc  (+glibc-devel pulled transitively)
pkg-config                           → pkgconf-pkg-config  (provides `pkg-config`)
libgtk-3-dev                         → gtk3-devel
libglib2.0-dev                       → glib2-devel
libayatana-appindicator3-dev         → libappindicator-gtk3-devel   ★ gotcha
libx11-dev                           → libX11-devel      (capital X)
libxcb1-dev                          → libxcb-devel
libxdo-dev                           → libxdo-devel
libhidapi-dev                        → hidapi-devel
libudev-dev                          → systemd-devel
```

## GOTCHA ★ — `libayatana-appindicator` is NOT in Fedora main repos

- Debian ships `libayatana-appindicator3-dev` (the modern Ayatana fork).
- **Fedora does NOT** ship libayatana-appindicator in the main repo — it is still
  a pending Review Request (Red Hat Bugzilla #2253582). The Ayatana `-devel`
  package is not `dnf install`-able from the default repos.
- Fedora **does** ship the legacy **`libappindicator-gtk3-devel`** (non-Ayatana),
  which provides the `appindicator3-0.1` pkg-config module that the GTK/appindicator
  linkage resolves against.
- **Therefore:** use `libappindicator-gtk3-devel`, NEVER `libayatana-appindicator*`
  on the Fedora container. (Source: web search 2026-08-07 — Reddit
  r/Fedora, rcloneview forum confirm `dnf install libappindicator-gtk3` /
  `libayatana-appindicator-gtk3` only via COPR.)
- Defensive note: even if tray-icon 0.20 does not *strictly* link appindicator,
  including `libappindicator-gtk3-devel` is harmless (extra available pkg, never
  breaks a build) and covers any transitive appindicator linkage the .deb build
  relies on.

## Verdict — the dnf install line the job MUST use

```bash
dnf install -y \
  gcc pkgconf-pkg-config \
  gtk3-devel glib2-devel libappindicator-gtk3-devel \
  libX11-devel libxcb-devel libxdo-devel \
  hidapi-devel systemd-devel \
  git jq gh ca-certificates
```

(`gh` is the GitHub CLI — needed for the `gh release upload` upload step; it IS
in Fedora's main repo, `dnf install gh` works per GitHub's install docs.)

The spec/contract 3-pack (`hidapi-devel libxdo-devel pkg-config`) is a TRUE
SUBSET of this list — installing the superset satisfies it. The superset is what
makes `cargo build --release` succeed.
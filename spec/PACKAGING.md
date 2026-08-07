# SPEC — Build, Packaging & Release

> Companion to `PRD.md`. Cargo build profile, the per-platform installers
> (Inno / PKGBUILD / DMG), the **community package-manager channels**
> (AUR / `.deb` / `.rpm` / Nix flake / Homebrew / Scoop / Winget / mise+asdf),
> the GNOME Shell extension artifact, the CI release workflow, code signing, and
> the committed dev test loop. Covers `Cargo.toml`, `.cargo/config.toml`,
> `release.toml`, `.github/workflows/release.yml`, and `packaging/`.

---

## 1. Cargo Build Profile

`Cargo.toml` `[profile.release]` (optimize for size):
```toml
opt-level   = "z"     # size
lto         = true
codegen-units = 1
panic       = "abort"   # no unwind; systemd Restart=always recovers crashes
strip       = true
```

`.cargo/config.toml` (Windows MSVC only):
```toml
[target.'cfg(all(target_os = "windows", target_env = "msvc"))']
rustflags = ["-C", "target-feature=+crt-static"]
```
⇒ statically links UCRT + vcruntime → **no Visual C++ Redistributable**
dependency on Windows (cost: ~135 KB larger exe).

`src/main.rs` top: `#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]`
⇒ no console window on Windows.

**MSRV Rust 1.88** (`rust-version` in `Cargo.toml`; image 0.25.x is the floor).

---

## 2. Features & Binaries

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

[[bin]] name = "qmkonnect"        path = "src/main.rs"
[[bin]] name = "qmkonnect-hid-id" path = "src/bin/hid_id.rs"   # pure std; udev helper
```

- Plain `cargo build --release` produces the **full app with every Linux backend
  + a tray** on every OS (off-platform features are inert no-ops).
- `--no-default-features` yields the minimal trayless service build (X11-only
  monitor).
- **Linux hidapi link nuance (must-preserve):** Arch ships `libhidapi-hidraw`
  *separate* from `libhidapi-libusb`, so the **Arch PKGBUILD links
  `-lhidapi-hidraw`** explicitly (usage/usage_page matching requires the hidraw
  backend). **Debian/Ubuntu and Fedora ship a *unified* hidapi (≥0.14)** that
  folds both backends into one `libhidapi.so` and auto-selects hidraw at runtime,
  so the **`.deb` and `.rpm` builds must NOT pass `-lhidapi-hidraw`** — linking
  the unified lib keeps usage/usage_page matching working. (Same note as the Nix
  flake's `hidrawFlag` caveat.)

---

## 3. Windows Packaging

### 3.1 The shipped installer — Inno Setup (per-user, no admin)
`packaging/windows/inno/QMKonnect.iss` → `QMKonnect-Setup.exe` (built by
`packaging/windows/inno/build.ps1`; needs `winget install JRSoftware.InnoSetup`).

- **Per-user:** `PrivilegesRequired=lowest`, `DefaultDirName={localappdata}\Programs\QMKonnect`,
  `DisableDirPage=yes` (fixed location). `AppId={{FAAE1F7A-...}}` is the stable
  upgrade identity (constant across versions).
- **Files:** `qmkonnect.exe`, `Icon.ico`, `IconTray-dark.png` → `{app}`.
- **`[Registry]`** writes the **HKCU `Run` value `"QMKonnect"`** (default-on
  autostart; `uninsdeletevalue`). Single source of truth shared with
  `src/autostart.rs` and `install.ps1`.
- **`[Icons]`** Start Menu shortcut (manual launch).
- **`[Code] KillRunningInstance`** in `InitializeSetup`/`InitializeUninstall`:
  `taskkill /IM qmkonnect.exe /F /T` so the single-instance mutex releases.
- **`[Run]`** launches the app after an *interactive* install (`skipifsilent`).
- Version injected from `Cargo.toml` (`#define MyAppVersion`).

### 3.2 `install.ps1` / `uninstall.ps1` (PowerShell equivalent)
`install.ps1`: stops running instance, copies exe + icon assets, Start Menu
`.lnk`, writes the HKCU `Run` value, registers an Add/Remove-Programs uninstall
entry, launches the app. `uninstall.ps1`: clears Run value, removes dir +
shortcuts.

### 3.3 The legacy WiX MSI (Session-0 service) — NOT shipped
`packaging/windows/installer.wxs` + `build-installer.ps1` build a Session-0
service MSI that **cannot show a tray icon** in the interactive session. Retained
as a legacy build path only; **the tray app + Inno installer is what ships**. CI
runs the Inno path, not WiX.

### 3.4 Runtime dependencies
**None.** Static CRT link (`+crt-static`) → runs on any clean Windows 10/11 x64.
Toolchain prereq: **Visual Studio Build Tools** with *Desktop development with
C++* (MSVC + Windows SDK); default `stable-x86_64-pc-windows-msvc`, not `gnu`.

---

## 4. Linux Packaging

The Linux artifact set is shared by every channel — a single binary
(`qmkonnect`), the udev helper (`qmkonnect-hid-id`), the static udev rule, the
systemd user service template, and (new) an XDG autostart `.desktop`. Every
package installs the same files to the same FHS paths:

| File | Path |
|---|---|
| app binary | `/usr/bin/qmkonnect` |
| udev helper | `/usr/lib/udev/qmkonnect-hid-id` |
| static udev rule | `/usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules` |
| systemd user service (template) | `/usr/lib/systemd/user/qmkonnect.service.template` |
| systemd user service (instantiated) | `/usr/lib/systemd/user/qmkonnect.service` (written by `postinst`) |
| **XDG autostart (new)** | `/etc/xdg/autostart/qmkonnect.desktop` |
| docs | `/usr/share/doc/qmkonnect/` |

### 4.1 Arch PKGBUILD — source (`packaging/linux/arch/`)
- `build()`: `RUSTFLAGS="-C link-arg=-lhidapi-hidraw" cargo build --release`
  (Arch's separate hidraw lib — §2).
- `package()` installs binary, helper, static rule, service template.
- `depends=('systemd' 'hidapi' 'libusb' 'zenity' 'libnotify')`;
  `makedepends=('cargo' 'rust' 'libx11' 'libxcb' 'systemd-libs' 'pkg-config')`.
- `backup=("usr/lib/systemd/user/qmkonnect.service.template")` — only the
  (user-instantiated) template is preserved across upgrades.
- `options=(!strip)`.

### 4.2 AUR (`qmkonnect-bin`) (`packaging/linux/aur/`)
- `-bin` PKGBUILD: downloads the pre-built GitHub release tarball
  (`qmkonnect-<ver>-linux-x86_64.tar.gz`, staged by the CI `linux-binary` job —
  §4.6) — no Rust toolchain or build deps. Installs the same four files as the
  source PKGBUILD.
- `qmkonnect.install` (pacman hooks): `post_install` instantiates the service
  template, reloads udev, `systemctl --global enable`, prints zero-config
  next-steps; `post_upgrade` re-instantiates + reloads; `post_remove` disables
  globally, stops/disables per-user services, removes the on-demand
  `/etc/udev/rules.d/99-qmkonnect.rules` + instantiated service, reloads udev.
- `publish.sh` + `.SRCINFO` + README drive AUR publication (CI/§9 can automate).
- Sibling `qmkonnect-git` (source, VCS) is a community option — point contributors
  at it.

### 4.3 `.deb` via cargo-deb (`packaging/debian/`) — NEW

Target: **Ubuntu / Debian / Linux Mint** (and derivatives). Built with
[`cargo-deb`](https://github.com/kornelski/cargo-deb) from a
`[package.metadata.deb]` block in `Cargo.toml`.

```toml
[package.metadata.deb]
name = "qmkonnect"
maintainer = "Mulletware <noreply@mulletware>"
copyright = "2025, Mulletware"
license-file = ["LICENSE", "0"]
extended-description-file = "packaging/debian/long-description.txt"
depends = "libhidapi-hidraw0, libxdo3, zenity, libnotify-bin, systemd"
section = "utils"
priority = "optional"
assets = [
  ["target/release/qmkonnect",                   "usr/bin/",                         "755"],
  ["target/release/qmkonnect-hid-id",            "usr/lib/udev/",                    "755"],
  ["packaging/linux/udev/69-qmkonnect-rawhid.rules", "usr/lib/udev/rules.d/",        "644"],
  ["packaging/linux/systemd/qmkonnect.service.template", "usr/lib/systemd/user/",   "644"],
  ["packaging/linux/xdg/qmkonnect.desktop",      "etc/xdg/autostart/",               "644"],
  ["README.md",                                  "usr/share/doc/qmkonnect/",         "644"],
]
maintainer-scripts = "packaging/debian/"
```

- **Build:** build the binary **without** `-lhidapi-hidraw` (Debian's unified
  hidapi auto-selects hidraw — §2), then `cargo deb`. Build on the **oldest
  supported LTS** (`ubuntu-22.04`) so the glibc (2.35) runtime works on 22.04,
  24.04, Debian 12, Mint 21/22+.
- **Maintainer scripts** (`packaging/debian/{postinst,prerm,postrm}`) mirror the
  Arch `qmkonnect.install` logic:
  - `postinst`: instantiate the service template → `qmkonnect.service`;
    `udevadm control --reload-rules && udevadm trigger`;
    `systemctl --global enable qmkonnect.service`; ensure the `input` group
    exists (`addgroup --system input` if missing); print zero-config next-steps.
  - `prerm`: no-op (let the running service continue until reboot/stop).
  - `postrm`: `systemctl --global disable`; stop+disable per-user services;
    remove the instantiated service + any `/etc/udev/rules.d/99-qmkonnect.rules`;
    reload udev.
- **`depends`:** `libhidapi-hidraw0` (the hidraw backend the unified lib dlopens),
  `libxdo3`, `zenity`, `libnotify-bin`, `systemd`. Build-deps (CI apt step):
  `libhidapi-dev libxdo-dev pkg-config`.
- **Output:** `target/debian/qmkonnect_<ver>_amd64.deb`, renamed by CI to
  `qmkonnect-<ver>-linux-amd64.deb` for the GitHub Release.
- **Distribution:** attach the binary `.deb` to the Release (works on
  Ubuntu/Debian/Mint via `sudo dpkg -i` / `sudo apt install ./…`). An optional
  **Launchpad PPA** (source build) is documented for `apt update`-style upgrades
  but is not required for v1 — the release `.deb` is the primary artifact.

### 4.4 `.rpm` via cargo-generate-rpm (`packaging/rpm/`) — NEW

Target: **Fedora / RHEL / Rocky / Alma / openSUSE**. Built with
[`cargo-generate-rpm`](https://github.com/cat-in-136/cargo-generate-rpm) from a
`[package.metadata.generate-rpm]` block in `Cargo.toml`.

```toml
[package.metadata.generate-rpm]
name = "qmkonnect"
license = "MIT"
summary = "Cross-platform window activity notifier for QMK keyboards"
release = "1"
vendor = "Mulletware"
url = "https://github.com/dabstractor/qmkonnect"
# Unified hidapi on Fedora/RHEL ⇒ do NOT add an -lhidapi-hidraw link flag (§2).
require-local = { "hidapi" >= "0.10", "libxdo", "zenity", "libnotify", "systemd" }
assets = [
  { source = "target/release/qmkonnect",                 dest = "/usr/bin/qmkonnect",            mode = "755" },
  { source = "target/release/qmkonnect-hid-id",          dest = "/usr/lib/udev/qmkonnect-hid-id",mode = "755" },
  { source = "packaging/linux/udev/69-qmkonnect-rawhid.rules", dest = "/usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules", mode = "644" },
  { source = "packaging/linux/systemd/qmkonnect.service.template", dest = "/usr/lib/systemd/user/qmkonnect.service.template", mode = "644" },
  { source = "packaging/linux/xdg/qmkonnect.desktop",    dest = "/etc/xdg/autostart/qmkonnect.desktop", mode = "644" },
]
# maintainer scripts (mirror the Debian/Arch hooks — instantiate service,
# reload udev, systemctl --global enable on install; reverse on erase).
post_install_script = "packaging/rpm/postin"     # file ref
post_uninstall_script = "packaging/rpm/postun"   # file ref
```

- **Build:** build the binary **without** `-lhidapi-hidraw` (Fedora's unified
  hidapi — §2), then `cargo generate-rpm`. Build on **Fedora** (covers Fedora +
  RHEL 9/Rocky 9/Alma 9 — glibc 2.34+). A second RHEL-8/older-glibc build on
  AlmaLinux 8 is optional; document it only if RHEL 8 demand appears.
- **`require` (Fedora unified):** `hidapi` (provides both backends), `libxdo`,
  `zenity`, `libnotify`, `systemd`. Build-deps (CI dnf step): `hidapi-devel
  libxdo-devel pkgconfig`.
- **openSUSE** largely shares this spec (`HIDAPI`, `libxdo-devel`,
  `libnotify-tools`, `zenity`); an OBS submit is a community follow-on.
- **Output:** `target/generate-rpm/qmkonnect-<ver>-1.x86_64.rpm`, renamed
  `qmkonnect-<ver>-linux-x86_64.rpm` for the Release.

### 4.5 Nix flake (`flake.nix`, repo root)
QMKonnect ships a [Nix flake](https://nixos.wiki/wiki/Flakes) that builds **from
source** against pinned Nixpkgs for `x86_64-linux` and `aarch64-linux`, producing
both binaries plus the static udev rule + systemd user service (rewritten to the
Nix store path). The flake also exposes a `nixosModules.default` NixOS module
(`services.qmkonnect.enable`) that registers the udev rule
(`services.udev.packages`), the user service (`systemd.packages`), and `PATH`.

```sh
nix profile install github:dabstractor/qmkonnect   # non-NixOS
nix run github:dabstractor/qmkonnect               # ad-hoc
nix build github:dabstractor/qmkonnect             # → ./result/bin/qmkonnect
nix develop github:dabstractor/qmkonnect           # dev shell (all system libs)
```

The flake links the hidraw backend via `hidrawFlag`, with the documented
unified-hidapi escape hatch (if the Nixpkgs revision ships unified hidapi ≥0.14
and the build fails on `-lhidapi-hidraw`, drop the `hidrawFlag` line — usage
matching still works, §2). Non-NixOS users install the udev rule + helper
manually (the README walks through it). Full detail: `packaging/nix/README.md`.

### 4.6 Generic tarball + `install.sh`
The CI `linux-binary` job stages a portable tarball
`qmkonnect-<ver>-linux-x86_64.tar.gz` (top-level dir holding the two binaries +
the static rule + the service template). This is the artifact the AUR `-bin`
PKGBUILD fetches **and** the input to a documented from-source install for any
distro without a native package:
```bash
cargo build --release
sudo install -m755 target/release/qmkonnect        /usr/local/bin/qmkonnect
sudo install -m755 target/release/qmkonnect-hid-id /usr/lib/udev/qmkonnect-hid-id
sudo install -m644 packaging/linux/udev/69-qmkonnect-rawhid.rules /usr/lib/udev/rules.d/
sudo install -m644 packaging/linux/xdg/qmkonnect.desktop /etc/xdg/autostart/
sudo udevadm control --reload && sudo udevadm trigger
```

### 4.7 XDG autostart `.desktop` (`packaging/linux/xdg/qmkonnect.desktop`) — NEW
The **universal autostart fallback** alongside systemd (`LINUX.md` §6). Every DE
session manager honors `~/.config/autostart/` (and `/etc/xdg/autostart/`), so a
single shipped file starts the daemon at login on every desktop — systemd or not
(MX, Artix, Void, Gentoo). It loses the systemd plug/unplug lifecycle (login-only
start) but gains universal coverage.

```ini
[Desktop Entry]
Type=Application
Name=QMKonnect
Comment=Send the foreground window to your QMK keyboard
Exec=qmkonnect
Icon=input-keyboard
Terminal=false
X-GNOME-Autostart-enabled=true
Categories=Utility;
# Not shown in application menus (autostart-only):
NoDisplay=true
```

- Ship it at `/etc/xdg/autostart/qmkonnect.desktop` in **every** Linux package
  (.deb/.rpm/PKGBUILD/AUR/tarball) so login-autostart works out of the box even
  where the systemd `SYSTEMD_USER_WANTS` path is disabled.
- `Exec=qmkonnect` relies on `/usr/bin` (or the Nix store path) being on `PATH`
  in the session; packages install to `/usr/bin` so this is satisfied. The Nix
  module and the `.desktop` are independent (NixOS uses systemd; the `.desktop`
  is for non-systemd or as a belt-and-suspenders).
- The user disables it by copying to `~/.config/autostart/` with
  `Hidden=true`, or by deleting the system file — same convention as every other
  autostart app.

### 4.8 Runtime dependencies per distro (summary)

| Distro family | Runtime pkgs | Notes |
|---|---|---|
| Arch | `hidapi libusb zenity libnotify systemd libxdo` | link `-lhidapi-hidraw` |
| Debian/Ubuntu/Mint | `libhidapi-hidraw0 libxdo3 zenity libnotify-bin systemd` | unified hidapi; **no** `-lhidapi-hidraw` |
| Fedora/RHEL/Rocky | `hidapi libxdo zenity libnotify systemd` | unified hidapi; **no** `-lhidapi-hidraw` |
| openSUSE | `HIDAPI libxdo libnotify-tools zenity systemd` | shared with Fedora spec |
| NixOS | (provided by Nixpkgs via the flake) | |

---

## 5. macOS Packaging

### 5.1 `packaging/macos/build.sh`
- `cargo build --release`. Assembles `QMKonnect.app/Contents/{MacOS,Resources}`.
- Generates `Info.plist` (`CFBundleIdentifier=io.mulletware.qmkonnect`,
  `LSUIElement=true` — menu-bar-only).
- **Codesign:** `--sign "$CODESIGN_IDENTITY"` where `$CODESIGN_IDENTITY`
  defaults to `-` (ad-hoc); a stable Developer ID stops the TCC re-prompt loop.
- Builds `QMKonnect.dmg` (UDZO) with an `/Applications` symlink.

### 5.2 `clean.sh` (run BEFORE every reinstall)
Stop app → eject stale DMGs → `lsregister -u` stale copies → `rm -rf` old bundles
→ `tccutil reset ScreenCapture io.mulletware.qmkonnect`.

### 5.3 `install.sh` / `uninstall.sh`
Mount DMG → copy to `/Applications`. Uninstall removes the app, the SMAppService
login entry, and per-user config.

### 5.4 Test via `open /Applications/QMKonnect.app`
Never test the menu bar via the raw `target/release/qmkonnect` — the bundle
context is required for the menu-bar icon + template path.

---

## 6. Cross-Platform Package-Manager Channels (F15)

Beyond the native installers, every release is published to the package managers
users already trust. Each channel's manifest is templated on `version` + a hash
placeholder that CI fills from the real release artifact (`PACKAGING.md` §9).

### 6.1 Homebrew Cask (`packaging/homebrew/Casks/qmkonnect.rb`)
- Distributes the macOS `.dmg` via a **custom tap**
  (`brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect`,
  `brew install --cask qmkonnect`) until notarization qualifies it for the
  official `homebrew-cask` repo (PRD §12).
- CI patches `version` + `sha256` (the `:no_check` placeholder) on each tagged
  release and pushes to the tap (`update-cask.sh`). `livecheck` follows GitHub
  releases. Cask `caveats` document Screen Recording + the ad-hoc-signature
  Gatekeeper workaround (`xattr -dr com.apple.quarantine` / `--no-quarantine`).
- Validate locally: `brew audit --cask --new-cask ./qmkonnect.rb`, `ruby -c`.

### 6.2 Scoop (`packaging/scoop/qmkonnect.json`)
- Windows manifest, per-user (no admin). `"innosetup": true` ⇒ Scoop extracts via
  `innounp` (**the Inno installer logic does NOT run**: no HKCU Run autostart, no
  ARP entry). Document this trade-off; autostart must be enabled from the tray's
  "Open at Login" toggle after install.
- CI fills the 64-zero `hash` placeholder (`update-manifest.ps1`). `checkver` +
  `autoupdate` follow GitHub releases. Published to a Scoop bucket (README +
  `bucket-README.md`).

### 6.3 Winget (`packaging/winget/*.yaml`)
- Three manifests (`dabstractor.QMKonnect.yaml` package, `.installer.yaml`,
  `.locale.en-US.yaml`) wrapping the **Inno installer** (`InstallerType: inno`,
  `Scope: user`, per-user — no UAC). `InstallerSha256` is a 64-zero placeholder
  CI fills (`submit.ps1`).
- Published to `microsoft/winget-pkgs` via PR (the `submit.ps1` helper). Unsigned
  installer ⇒ Windows shows an "unverified publisher" prompt (PRD §12).
- `UpgradeBehavior: install`; `Silent: /VERYSILENT`, `SilentWithProgress: /SILENT`.

### 6.4 mise + asdf (`packaging/asdf/`)
- One plugin (`github.com/dabstractor/asdf-qmkonnect`) serves **both** managers:
  mise runs an asdf plugin's `bin/*` unchanged (`packaging/asdf/mise.toml` is a
  documentation example, not consumed at runtime). `bin/download` fetches the
  GitHub release asset per OS/arch; `bin/install` places it in the manager's
  prefix; `bin/list-all` lists releases via the GitHub API.
- Cross-platform (Linux/macOS/Windows-WSL). Installs the **binary only**; the
  platform autostart + (on Linux) the udev rule are set up separately by the user
  (documented). `publish.sh` cuts plugin releases.

> **Why these and not Flatpak/AppImage as primary?** Flatpak's sandbox blocks
> `/dev/hidraw` and portals don't cover HID — wrong model for a HID daemon.
> AppImage still needs the system udev rule + autostart wiring, so it offers no
> real win over the generic tarball. Both are explicitly out of scope as primary
> channels (the generic tarball + native packages cover every distro).

---

## 7. The GNOME Shell Extension Artifact (`packaging/gnome-shell-extension/`)

GNOME (Mutter) cannot report the active window to clients, so a tiny GNOME Shell
extension (`qmkonnect@mulletware`) reads `global.display.focus_window` inside
`gnome-shell` and republishes it over the session D-Bus, where the desktop app's
GNOME backend (`src/platforms/gnome.rs`) subscribes (`PLATFORMS.md` §8). This is
a **separate deliverable** from the app binary — the app cannot load it.

- **Contents:** `metadata.json` (`uuid`, `shell-version`, `version`), `extension.js`
  (`enable`/`disable`/`_onFocus` — §8.2 of PLATFORMS.md), optional `prefs.js`,
  `stylesheet.css`. D-Bus interface introspection XML under the package for
  reference.
- **D-Bus contract:** name `io.mulletware.QMKonnect`, path
  `/io/mulletware/QMKonnect`, interface `io.mulletware.QMKonnect.WindowMonitor`
  (method `GetActiveWindow`→(ss), signal `ActiveWindowChanged`(ss), properties
  `AppClass`/`Title`). `app_class` = `MetaWindow.get_wm_class()`.
- **Build:** zip the directory as `qmkonnect@mulletware.shell-extension.zip`
  (the extensions.gnome.org upload format). CI (§9) builds the zip and attaches
  it to the Release.
- **Distribution:** published on **extensions.gnome.org** (EGO) + the Release
  asset. The app's first-run notification (§8.4) links users to it. The
  `shell-version` array must be bumped per supported GNOME line on each release
  (EGO gates compatibility by it).

---

## 8. The Dev Test Loop (`AGENTS.md`)

### 8.1 macOS
```bash
cargo test --bin qmkonnect -- --test-threads=1   # shared debouncer state
cd packaging/macos && ./clean.sh && ./build.sh && ./install.sh
open /Applications/QMKonnect.app                  # grant the one Screen-Recording prompt
```
If the icon looks dimmed/unclickable, the main thread is wedged →
`sample <pid> 2 | grep -i mutex` (healthy = `nextEventMatchingMask`). Always
rebuild before sampling.

### 8.2 Windows (PowerShell)
```powershell
cargo test --bin qmkonnect -- --test-threads=1
cargo build --release
taskkill /IM qmkonnect.exe /F     # mandatory — single-instance mutex
.\target\release\qmkonnect.exe     # run in YOUR session, never via sc/services.msc
```
Exclude `target\`, `~/.cargo`, and the project dir from Defender. Inno installer:
`powershell -NoProfile -ExecutionPolicy Bypass -File packaging\windows\inno\build.ps1`
→ `packaging\windows\inno\Output\QMKonnect-Setup.exe`.

### 8.3 Linux
```bash
cargo test --bin qmkonnect -- --test-threads=1
cargo build --release                       # builds qmkonnect + qmkonnect-hid-id
cargo clippy --all-targets -- -D warnings
# Arch:   cd packaging/linux/arch && makepkg -f && sudo pacman -U qmkonnect-*.pkg.tar.zst
# .deb:   cargo install cargo-deb && cargo deb   →  target/debian/qmkonnect_*.deb
# .rpm:   cargo install cargo-generate-rpm && cargo generate-rpm → target/generate-rpm/*.rpm
```
Backend smoke test: `qmkonnect -v` prints which backend `select_linux_backend`
chose and why each other candidate was skipped (PLATFORMS.md §6).

---

## 9. CI Release (`.github/workflows/release.yml`)

**Triggers:** push of a `v*` tag (builds **and** publishes) or `workflow_dispatch`
(builds **without** publishing — dry-run the whole pipeline and download real
artifacts before cutting a tag).

- `qmk-notifier` is a pinned git dep (`tag = "vX.Y.Z"`), so a plain
  `actions/checkout` suffices. Version is read from `cargo metadata` (single
  source of truth in `Cargo.toml`); all manifest versions/hashes are injected.
- **macOS job:** `cargo build`, `packaging/macos/build.sh`. If repo var
  `ENABLE_MACOS_NOTARIZE=true` + `APPLE_*` secrets: import Developer ID cert, set
  `CODESIGN_IDENTITY`, `notarytool submit … --wait` + `stapler staple`. Renames
  `QMKonnet-<ver>-macos.dmg`, uploads artifact. **Publish:** the Homebrew job
  patches the cask (`version`+`sha256`) and pushes to the tap.
- **Windows job:** `cargo build`, install Inno Setup,
  `packaging/windows/inno/build.ps1` → `QMKonnect-<ver>-windows-x64.exe`
  (primary Windows artifact). **Publish:** Scoop job fills the manifest `hash`
  and commits to the bucket; Winget job opens/updates the `winget-pkgs` PR with
  the real `InstallerSha256`.
- **Linux binary job:** plain release build, stage
  `qmkonnet-<ver>-linux-x86_64.tar.gz` (binaries + static rule + service
  template). Uploaded as an artifact and fetched by the AUR `-bin` PKGBUILD.
- **Arch job:** `makepkg`/docker → `qmkonnect-<ver>-x86_64.pkg.tar.zst`.
- **`.deb` job (NEW):** on `ubuntu-22.04` — `apt` install `libhidapi-dev
  libxdo-dev pkg-config`, `cargo install cargo-deb`, `cargo deb` (**no**
  `-lhidapi-hidraw`), rename to `qmkonnect-<ver>-linux-amd64.deb`, upload.
- **`.rpm` job (NEW):** on Fedora — `dnf` install `hidapi-devel libxdo-devel
  pkg-config`, `cargo install cargo-generate-rpm`, `cargo generate-rpm` (**no**
  `-lhidapi-hidraw`), rename to `qmkonnect-<ver>-linux-x86_64.rpm`, upload.
- **Nix job:** `nix build .#` (x86_64 + aarch64) to verify the flake; the flake
  is consumed in-place from the repo (no artifact to publish).
- **GNOME extension job (NEW):** zip `packaging/gnome-shell-extension/` →
  `qmkonnect@mulletware.shell-extension.zip`, attach to the Release. (EGO upload
  is a manual maintainer step; CI just builds the zip.)
- **asdf/mise job:** on tag, `packaging/asdf/publish.sh` cuts a plugin release
  tagging the new version.

> The legacy WiX MSI path (`build-installer.ps1`) is **not** invoked by CI.

---

## 10. Release Chore (`release.toml` + `cargo-release`)

QMKonnect is a **binary app**, never published to crates.io (`publish = false`).
`cargo release <level>` (e.g. `cargo release 0.3.0`):
1. bumps `version` in `Cargo.toml` (+ `Cargo.lock`),
2. commits the bump,
3. creates an annotated `v<version>` tag,
4. pushes commit + tag to `origin`.

Pushing the tag triggers `release.yml`. **Nothing tags or publishes on its own**
— the maintainer controls *when* a release happens. Releases cut from `main`
(`allow-branch = ["main"]`).

---

## 11. Build Outputs (gitignored, never commit)
`target/`, `QMKonnect.app/`, `*.dmg`, `*.msi`, `*.exe` installers, `*.deb`,
`*.rpm`, `*.pkg.tar.zst`, `arch/pkg/`, `*.shell-extension.zip`, `docs/_site/`.
Regenerated by the build scripts/CI.

---

*Continue with `SPEC_FIRMWARE.md`.*
# SPEC — Build, Packaging & Release

> Companion to `PRD.md`. Cargo build profile, the per-platform installers (Inno
> Setup / Arch PKGBUILD / macOS DMG), the CI release workflow, code signing, and
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
default    = ["hyprland", "macos", "linux-tray"]
hyprland   = ["dep:hyprland"]
macos      = ["dep:objc", "dep:core-foundation", "dep:core-graphics", "dep:dispatch"]
linux-tray = ["dep:ksni", "dep:gtk"]

[[bin]] name = "qmkonnect"        path = "src/main.rs"
[[bin]] name = "qmkonnect-hid-id" path = "src/bin/hid_id.rs"   # pure std; udev helper
```

- Plain `cargo build --release` produces the **full app with a tray** on every OS
  (off-platform features are inert no-ops).
- `--no-default-features` yields the minimal trayless service build.
- **Linux Arch build** links `-lhidapi-hidraw` (not `-lhidapi-libusb`) so
  usage/usage_page matching works: `RUSTFLAGS="-C link-arg=-lhidapi-hidraw" cargo build --release`.

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
  autostart; `uninsdeletevalue`). This is the **single source of truth** shared
  with `src/autostart.rs` and `install.ps1` — keep the value name identical.
- **`[Icons]`** Start Menu shortcut (manual launch).
- **`[Code] KillRunningInstance`** in `InitializeSetup`/`InitializeUninstall`:
  `taskkill /IM qmkonnect.exe /F /T` so the single-instance mutex releases and
  the exe can be overwritten.
- **`[Run]`** launches the app after an *interactive* install (`skipifsilent`
  avoids a tray-less background process on `/VERYSILENT`).
- Version injected from `Cargo.toml` (`#define MyAppVersion`).

### 3.2 `install.ps1` / `uninstall.ps1` (the PowerShell equivalent)
`install.ps1`: stops any running instance, copies exe + icon assets to
`%LOCALAPPDATA%\Programs\QMKonnect`, Start Menu `.lnk`, writes the HKCU `Run`
value, registers an Add/Remove-Programs uninstall entry (DisplayName/Version/
Publisher/InstallLocation/UninstallString), launches the app.

`uninstall.ps1`: `Remove-ItemProperty … Run -Name QMKonnect`, removes the
install dir + shortcuts.

### 3.3 The legacy WiX MSI (Session-0 service) — NOT shipped
`packaging/windows/installer.wxs` + `build-installer.ps1` (needs WiX v3) build
an MSI that installs a **Session-0 service**. A service **cannot** show a tray
icon in the interactive session, so this is the wrong vehicle for the tray app.
It remains as a legacy build path only; the **tray app + Inno installer is what
ships**. (CI's `windows` job currently uses the WiX path — flag for the dev
agent to reconcile with the Inno path.)

### 3.4 Runtime dependencies
**None.** The release binary statically links the C runtime (`+crt-static`), so
`QMKonnect-Setup.exe` runs on any clean Windows 10/11 x64 machine. Toolchain
prereq: **Visual Studio Build Tools** with *Desktop development with C++*
(MSVC + Windows SDK); use the default `stable-x86_64-pc-windows-msvc` host, not
`gnu`, or the `windows`-crate link step fails.

---

## 4. Linux Packaging

### 4.1 Arch PKGBUILD (`packaging/linux/arch/`)
- `build()`: `RUSTFLAGS="-C link-arg=-lhidapi-hidraw" cargo build --release`
  (builds both `qmkonnect` and `qmkonnect-hid-id`).
- `package()` installs:
  - `qmkonnect` → `/usr/bin/qmkonnect`
  - `qmkonnect-hid-id` → `/usr/lib/udev/qmkonnect-hid-id`
  - `69-qmkonnect-rawhid.rules` → `/usr/lib/udev/rules.d/`
  - `qmkonnect.service.template` → `/usr/lib/systemd/user/` (instantiated by `post_install`)
- `depends=('systemd' 'hidapi' 'libusb' 'zenity' 'libnotify')`.
- `backup=("usr/lib/systemd/user/qmkonnect.service.template")` — only the
  (user-instantiated) template is preserved across upgrades; the static rule
  and helper are package-owned; the on-demand `99-qmkonnect.rules` is user-generated.
- `options=(!strip)`.

### 4.2 `qmkonnect.install` (pacman hooks)
- `post_install`: instantiate the service template; `udevadm control --reload-rules && udevadm trigger`;
  `systemctl --global enable qmkonnect.service`; print zero-config next-steps.
- `post_upgrade`: re-instantiate the template + reload udev. **Does not** call
  `qmkonnect --reload` (needs root + a config that may not exist yet).
- `post_remove`: `systemctl --global disable`; stop+disable per-user services;
  `rm -f /etc/udev/rules.d/99-qmkonnect.rules` + the instantiated service; reload udev.

### 4.3 Other distros (binary install)
Install the binary + the static rule + helper + (optional) service template by
hand — documented in `docs/installation.md`:
```bash
cargo build --release
sudo install -m755 target/release/qmkonnect        /usr/local/bin/qmkonnect
sudo install -m755 target/release/qmkonnect-hid-id /usr/lib/udev/qmkonnect-hid-id
sudo install -m644 packaging/linux/udev/69-qmkonnect-rawhid.rules /usr/lib/udev/rules.d/
sudo udevadm control --reload && sudo udevadm trigger
```

---

## 5. macOS Packaging

### 5.1 `packaging/macos/build.sh`
- `cargo build --release`.
- Assembles `QMKonnect.app/Contents/{MacOS/qmkonnect, Resources/{Icon.icns, IconTemplate.png}}`.
- Generates `Info.plist`:
  ```xml
  CFBundleExecutable    = qmkonnect
  CFBundleIdentifier    = io.mulletware.qmkonnect
  CFBundleName          = QMKonnect
  CFBundleIconFile      = Icon.icns
  LSUIElement           = true        # menu-bar-only: no Dock, no CMD-Tab
  ```
- **Codesign:** `codesign --deep --force --sign "$CODESIGN_IDENTITY"` where
  `$CODESIGN_IDENTITY` defaults to `-` (ad-hoc). For distribution, set
  `CODESIGN_IDENTITY="Developer ID Application: … (TEAMID)"` for a stable,
  TCC-persistent signature.
- Builds `QMKonnect.dmg` (UDZO) with an `/Applications` symlink.

### 5.2 `clean.sh` — run BEFORE every reinstall
The #1 cause of "I rebuilt but nothing changed":
1. `pkill -f QMKonnect.app`.
2. Eject any mounted `QMKonnect` DMGs.
3. `lsregister -u` stale copies (`/Applications`, `~/.Trash`).
4. `rm -rf` old bundles.
5. `tccutil reset ScreenCapture io.mulletware.qmkonnect` (ad-hoc `cdhash`
   changes every build → TCC re-prompts even though Settings shows it granted).

### 5.3 `install.sh` / `uninstall.sh`
- `install.sh`: mount the DMG, copy `QMKonnect.app` to `/Applications`.
- `uninstall.sh`: remove the app, the **Launch at Login** `SMAppService` entry,
  and per-user config.

### 5.4 Test via `open /Applications/QMKonnect.app`
**Never** test the menu bar by running `target/release/qmkonnect` directly —
outside a real bundle the menu-bar icon and template path don't work. The raw
binary is fine for CLI subcommands.

---

## 6. The Dev Test Loop (`AGENTS.md`)

### 6.1 macOS
```bash
cargo test --bin qmkonnect -- --test-threads=1   # shared debouncer state
cd packaging/macos && ./clean.sh && ./build.sh && ./install.sh
open /Applications/QMKonnect.app                  # grant the one Screen-Recording prompt
```
If the icon looks dimmed/unclickable, the main thread is wedged →
`sample <pid> 2 | grep -i mutex` (healthy = `nextEventMatchingMask`). Always
rebuild before sampling (a stale binary misleads).

### 6.2 Windows (PowerShell)
```powershell
cargo test --bin qmkonnect -- --test-threads=1
cargo build --release
taskkill /IM qmkonnect.exe /F     # mandatory — single-instance mutex
.\target\release\qmkonnect.exe     # run in YOUR session, never via sc/services.msc
```
Exclude `target\`, `~/.cargo`, and the project dir from Windows Defender
(real-time scanning makes builds crawl). The Inno installer:
`powershell -NoProfile -ExecutionPolicy Bypass -File packaging\windows\inno\build.ps1`
→ `packaging\windows\inno\Output\QMKonnect-Setup.exe`.

### 6.3 Linux
```bash
cargo test --bin qmkonnect -- --test-threads=1
cargo build --release                       # builds qmkonnect + qmkonnect-hid-id
cargo clippy --all-targets -- -D warnings
# package: cd packaging/linux/arch && makepkg -f && sudo pacman -U qmkonnect-*.pkg.tar.zst
```

---

## 7. CI Release (`.github/workflows/release.yml`)

**Triggers:** push of a `v*` tag (builds **and** publishes) or `workflow_dispatch`
(builds **without** publishing — dry-run the whole pipeline and download real
artifacts before cutting a tag).

- `qmk_notifier` is a pinned git dep (`tag = "vX.Y.Z"`), so a plain
  `actions/checkout` of this repo suffices.
- **macOS job:** `cargo build`, `packaging/macos/build.sh`. If repo var
  `ENABLE_MACOS_NOTARIZE=true` + `APPLE_*` secrets: import Developer ID cert,
  set `CODESIGN_IDENTITY`, then `notarytool submit … --wait` + `stapler staple`.
  Renames `QMKonnect-<ver>-macos.dmg`, uploads artifact.
- **Windows job:** `cargo build`, install WiX v3, `build-installer.ps1` → MSI.
  *(Note: the shipped end-user artifact is the Inno `QMKonnect-Setup.exe`; the
  CI Windows job currently builds the WiX MSI. A dev agent should reconcile
  this — run `packaging/windows/inno/build.ps1` to produce the setup exe and
  upload that as the primary artifact.)*
- **Linux job:** Arch build via `makepkg`/docker → `.pkg.tar.zst` + standalone
  binary.
- Version is read from `cargo metadata` (single source of truth in `Cargo.toml`);
  installer/PKGBUILD versions are injected from it (no `pre-release-replacements`).

---

## 8. Release Chore (`release.toml` + `cargo-release`)

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

## 9. Build Outputs (gitignored, never commit)
`target/`, `QMKonnect.app/`, `*.dmg`, `*.msi`, `*.exe` installers,
`arch/pkg/`, `docs/_site/`. Regenerated by the build scripts.

---

*Continue with `SPEC_FIRMWARE.md`.*

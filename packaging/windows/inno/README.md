# QMKonnect — tray-app installer (Inno Setup)

Builds **`QMKonnect-Setup.exe`**: the per-user, **no-admin** installer for the
*interactive tray app* (menu-bar icon + "Open at Login" toggle). **This is the
installer to ship to end users** — they double-click it and get the standard
Next → Next → Finish wizard.

> **Why a separate installer?** The parent directory's WiX `installer.wxs` /
> `build-installer.ps1` builds an MSI that installs a **Session-0 Windows
> Service**. A service runs in Session 0, which *cannot show a tray icon* in the
> user's interactive session (see [`../../AGENTS.md`](../../AGENTS.md)), so it is
> the wrong vehicle for the tray app. This Inno installer is the per-user,
> interactive-app path. (`install.ps1` is kept as the headless/silent variant of
> the same thing.)

## What it does

Replicates [`../install.ps1`](../install.ps1):

- copies `qmkonnect.exe` + `Icon.ico` + `IconTray-dark.png` to
  `%LOCALAPPDATA%\Programs\QMKonnect`
- Start Menu shortcut (manual launch)
- `HKCU\…\Run\QMKonnect` autostart value — **default on**, and the same value the
  in-app **"Open at Login"** toggle manages (`src/autostart.rs`), so they never
  desync (single source of truth)
- launches the app after an interactive install
- registers an Add/Remove Programs uninstall entry

## Supported platforms & requirements

- **OS:** Windows **10 or 11**. Older versions — 8.1, 8, 7, Vista, XP — are
  **not supported**: the app links against APIs that don't exist there. Unlike
  many Windows programs it is *not* backwards-compatible to old versions, so
  don't expect it to run on a legacy box.
- **Architecture:** **64-bit (x64) only.** The installer refuses to run on
  32-bit Windows. On Windows 11 on ARM it installs and runs via x64 emulation
  (the installer permits this via its `x64compatible` setting).
- **No Administrator / UAC prompt:** per-user install to `%LOCALAPPDATA%`.
- **No extra runtimes to install:** the executable statically links the C
  runtime, so there is **no Visual C++ Redistributable** dependency.
- **First-run prompts to expect:**
  - **SmartScreen "Windows protected your PC / Unknown publisher"** — the app is
    not code-signed. Click *More info → Run anyway*.
  - **Screen-recording permission** — required to read window titles. Grant it;
    the tray and menu work without it, but app names won't.
- **Autostart:** enabled by default via the HKCU `Run` key (toggle it in the tray
  with **Open at Login**).

## Prerequisites

1. Rust toolchain + a release build (see the procedure below).
2. Inno Setup 6: `winget install JRSoftware.InnoSetup`
   (`build.ps1` finds `iscc` on `PATH` or in the default user/machine install dir).

## Release build & installation

Run from the **repo root** in git-bash (PowerShell equivalents inline). The
release exe lands in `%CARGO_TARGET_DIR%\release\` (i.e.
`C:\cargo-target\release\` here; `.\target\release\` if `CARGO_TARGET_DIR` is
unset).

```bash
cd /z   # repo root

# 1. Tests — single-threaded (shared debouncer state; see AGENTS.md)
cargo test --bin qmkonnect -- --test-threads=1

# 2. Final release build  ->  release\qmkonnect.exe
cargo build --release

# 3. Package into the installer (reads version from Cargo.toml)
powershell -NoProfile -ExecutionPolicy Bypass -File packaging/windows/inno/build.ps1
#    -> packaging/windows/inno/Output/QMKonnect-Setup.exe
```

**Install** — must run in your interactive session, because the tray can't render
from Session 0 (see [`../../AGENTS.md`](../../AGENTS.md)):

```bash
# interactive wizard — launches the tray app at the end
start "" "Z:\packaging\windows\inno\Output\QMKonnect-Setup.exe"
#   (or just double-click it in Explorer)
```

Silent alternative (no wizard; does NOT auto-launch the app):

```bash
MSYS_NO_PATHCONV=1 "Z:/packaging/windows/inno/Output/QMKonnect-Setup.exe" \
    /VERYSILENT /SUPPRESSMSGBOXES /NORESTART
```

(`MSYS_NO_PATHCONV=1` stops git-bash mangling `/VERYSILENT` into a path. In plain
PowerShell/cmd, omit it.)

## Verifying the install

```bash
# files + icon assets present
ls -l "$LOCALAPPDATA/Programs/QMKonnect/"

# SHA256: the installed exe MUST equal the built one
sha256sum /c/cargo-target/release/qmkonnect.exe "$LOCALAPPDATA/Programs/QMKonnect/QMKonnect.exe"

# default-on autostart value present
powershell -NoProfile -Command "(Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name QMKonnect).QMKonnect"
```

Then right-click the tray icon → **"Open at Login"** should be checked.

## Notes

- **Version**: `build.ps1` pulls it from `Cargo.toml` into the installer's version
  metadata and the Add/Remove-Programs entry. Bump `Cargo.toml` *before* step 3
  for a versioned release — it's the single source of truth.
  Override: `iscc /DMyAppVersion=9.9.9 QMKonnect.iss`.
- **Upgrade over an existing install**: just run the installer again — its `[Code]`
  force-closes the running app (single-instance named mutex) and replaces files
  in place.
- **Autostart-on-reboot**: the Run key is written by the install; to confirm the
  launch, sign out and back in. Toggle "Open at Login" off in the tray and repeat
  to confirm it stays off.
- **Quick dev iteration (no installer)**: `taskkill //IM qmkonnect.exe //F` then
  run `C:\cargo-target\release\qmkonnect.exe` directly in your session — same tray,
  just not "installed".
- **Uninstall**: Add/Remove Programs, or
  `& "$env:LOCALAPPDATA\Programs\QMKonnect\unins000.exe" /VERYSILENT /SUPPRESSMSGBOXES`.

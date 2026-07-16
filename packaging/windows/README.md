# QMKonnect — Windows packaging

The Windows installer lives in **[`./inno/`](inno/)**:

- **[`inno/build.ps1`](inno/build.ps1)** builds **`QMKonnect-Setup.exe`** — the
  per-user, **no-admin** Inno Setup installer for the *interactive tray app*
  (menu-bar icon + "Open at Login" toggle). **This is the installer to ship.**
- See **[`inno/README.md`](inno/README.md)** for the full release-build &
  installation procedure, supported platforms, and verification steps.

## Headless / silent install (no GUI wizard)

[`install.ps1`](install.ps1) is the script equivalent of the installer — same
per-user layout, same HKCU `Run` autostart value, same ARP entry — for automation
or when you can't (or don't want to) run the wizard:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File install.ps1
```

Its companion [`uninstall.ps1`](uninstall.ps1) removes everything.

> **Historical note:** this directory previously also held a WiX Toolset MSI
> (`installer.wxs` / `build-installer.ps1`) that installed a Session-0 *service*.
> A service can't show a tray icon in your interactive session, so it was the
> wrong vehicle for the tray app and has been removed. The Inno `.exe` is now the
> only Windows installer. (If you need the headless service back, recover it from
> git history at the commit before this change.)

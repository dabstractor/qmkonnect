# Changelog

All notable changes to the `asdf-qmkonnect` plugin are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This plugin wraps [QMKonnect](https://github.com/dabstractor/qmkonnect/releases) GitHub
releases, so its versions track the QMKonnect release each entry ships support for. The
same plugin serves [mise](https://mise.jdx.dev/) unchanged (mise runs an asdf plugin's
`bin/*` scripts verbatim).

## [Unreleased]

## [0.2.8] - 2026-07-16

### Added
- Initial release of the `asdf-qmkonnect` plugin.
- `bin/list-all` — lists QMKonnect versions from the GitHub Releases API, ascending
  (newest last) so `asdf install qmkonnect latest` resolves correctly without a
  `bin/latest-stable` callback.
- `bin/download` — maps `uname -s` / `uname -m` to the release asset and fetches it into
  `$ASDF_DOWNLOAD_PATH` (verifies SHA256 only if a `<asset>.sha256` sidecar exists; none
  is published today).
- `bin/install` — installs into `$ASDF_INSTALL_PATH/bin/`:
  - Linux x86_64 (primary): `qmkonnect` + `qmkonnect-hid-id`, with the udev rule + systemd
    template staged under `share/qmkonnect/` for a one-time manual setup.
  - macOS: the raw Mach-O binary from the DMG (CLI flags only — the menu-bar tray needs
    the full `.app` bundle; use the Homebrew cask or direct DMG for that).
  - Windows: redirects to Scoop / Winget / the Inno installer (the `.exe` is an installer,
    not a portable binary).
- `mise` compatibility — mise runs this plugin's `bin/*` scripts unchanged; see `mise.toml`
  for the `[tools]` pin example.

[Unreleased]: https://github.com/dabstractor/asdf-qmkonnect/compare/v0.2.8...HEAD
[0.2.8]: https://github.com/dabstractor/asdf-qmkonnect/releases/tag/v0.2.8
# qmkonnect — Homebrew Cask (macOS)

This directory holds the **Homebrew Cask** for QMKonnect, distributed via a
custom tap ([`dabstractor/homebrew-qmkonnect`](https://github.com/dabstractor/homebrew-qmkonnect)).
It is the **macOS community channel** alongside the primary direct-DMG download
(see [spec/PRD.md](../../spec/PRD.md) §5 — "macOS: `.dmg` (primary) · Homebrew Cask").

## What this is

`qmkonnect` is a Homebrew **Cask** (a GUI `.app` distributed from a `.dmg`), not
a formula. It downloads the pre-built GitHub release DMG (the `macos` job in
[`.github/workflows/release.yml`](../../.github/workflows/release.yml), a
**universal** build via `MACOS_UNIVERSAL=1`) and installs `QMKonnect.app` into
`/Applications`. **No Rust toolchain, no `cargo`, no build dependencies** — the
DMG ships the ready-to-run universal app (Apple Silicon + Intel, one cask for
both arches).

Homebrew is **per-user**, so the install needs no `sudo`. Per PRD §12 /
[architecture/external_deps.md](../../plan/007_fb356ba503b4/architecture/external_deps.md)
§2, the cask lives in a **custom tap** until the DMG is Developer-ID-signed +
notarized and graduates to the official [`Homebrew/homebrew-cask`](https://github.com/Homebrew/homebrew-cask)
repo.

## Install

```bash
# Add the tap, then install:
brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect
brew install --cask qmkonnect

# If the build is unnotarized and Gatekeeper blocks the first launch:
brew install --cask --no-quarantine qmkonnect
```

Updates are native — `brew upgrade --cask qmkonnect` pulls future releases. The
cask's `livecheck` (`strategy :header` against `/releases/latest`) auto-detects
new versions, so `brew livecheck` / `brew bump-cask-pr` can surface them without
manual bumps.

### Why `--no-quarantine`?

The released DMG is **ad-hoc signed** (not yet Developer-ID-signed + notarized).
Homebrew quarantines unnotarized downloads, and macOS Gatekeeper will block the
first launch with "'QMKonnect' is damaged / can't be opened." The
`--no-quarantine` flag (or the post-install `xattr -dr com.apple.quarantine`
workaround documented in the cask caveats) bypasses that for the ad-hoc case.
Once notarized, this flag is unnecessary and can be dropped.

## Post-install

1. **Grant Screen Recording** (only needed for window *titles*; the app runs
   without it and sends app *names* only). On first launch, go to
   **System Settings → Privacy & Security → Screen Recording** and enable
   QMKonnect.
2. **Discover rule strings** for your `rules.toml` with the built-in discovery
   CLI: `qmkonnect --show-window-info` (focus a window to print its
   class/title).
3. **Auto-start at login** is on by default (registered via `SMAppService`).
   Toggle it in the app's menu-bar icon if you prefer manual launch.

## What it installs

| Path | What |
|---|---|
| `/Applications/QMKonnect.app` | The menu-bar/tray daemon (universal binary) |
| `~/Library/Application Support/QMKonnect/config.toml` | Per-user device filter (VID/PID) + timing config |
| `~/Library/Application Support/QMKonnect/rules.toml` | Per-user window-activity → HID-command rules |

The `/Applications` install + the per-user config dir match exactly what the
source build scripts in [`packaging/macos/`](../macos/) produce
([`build.sh`](../macos/build.sh), [`install.sh`](../macos/install.sh)) and
what [`uninstall.sh`](../macos/uninstall.sh) removes — the cask is just the
package-manager delivery for the same artifact.

## Uninstall

```bash
brew uninstall --cask qmkonnect
# To ALSO remove the per-user config (config.toml + rules.toml):
brew uninstall --cask --zap qmkonnect
```

`--zap` removes `~/Library/Application Support/QMKonnect/` (the cask `zap trash:`
target). QMKonnect is an `LSUIElement` menu-bar app with no UserDefaults/plist
state, so that directory is the complete per-user footprint. The cask uninstall
also clears the `SMAppService` login item.

## Version & checksum maintenance

The `version` and `sha256` fields in
[`Casks/qmkonnect.rb`](Casks/qmkonnect.rb) are the **CI-replaceable template
fields**. The source-of-truth cask in this directory carries:

- `version "0.2.8"` — the current release.
- `sha256 :no_check` — the Cask-Cookbook-documented placeholder. CI overwrites
  it on each tagged release with the real `shasum -a 256` of
  `QMKonnect-<version>-macos.dmg` and pushes the patched file to the tap repo.

This mirrors the AUR channel's checksum-refresh flow
([`packaging/linux/aur/README.md`](../linux/aur/README.md) §"Version & checksum
maintenance"): the external channel's package file always reflects the latest
release, kept in sync by CI.

## Path to the official cask

Per [spec/PRD.md](../../spec/PRD.md) §12 and
[external_deps.md](../../plan/007_fb356ba503b4/architecture/external_deps.md)
§2, once the DMG is **Developer-ID-signed + notarized** (the `notarytool` +
`stapler` step in [`release.yml`](../../.github/workflows/release.yml) is
already wired, gated on secrets), this cask graduates:

- The custom tap stays available but becomes optional.
- The same `sha256` / `url` / `livecheck` / `app` / `zap` stanzas carry over
  **unchanged** — only the distribution channel changes (a PR to
  [`Homebrew/homebrew-cask`](https://github.com/Homebrew/homebrew-cask) instead
  of the custom tap).
- The `--no-quarantine` flag / Gatekeeper caveats are no longer needed (a
  notarized + stapled DMG passes Gatekeeper by default).

See [`docs/installation.md`](../../docs/installation.md) for the consolidated
macOS install matrix once that lands.

## Notes

- **Cask token `qmkonnect`** is lowercase, no special chars — matches the repo
  and app name and the Homebrew token rules (see
  [`Adding-Software-to-Homebrew`](https://docs.brew.sh/Adding-Software-to-Homebrew)).
- **`Casks/qmkonnect.rb` is the source of truth.** The tap repo
  `dabstractor/homebrew-qmkonnect` holds a CI-published copy (sibling
  P1.M2.T1.S2 / P1.M5.T1.S2); edit the file here and let CI propagate it.
- **No `depends_on arch:`** — the DMG is universal (`aarch64` + `x86_64`), so a
  single cask serves both Apple Silicon and Intel.
- **Validate locally**: `ruby -c Casks/qmkonnect.rb` (syntax, any host with
  Ruby); `brew audit --cask --new-cask ./Casks/qmkonnect.rb` (DSL/token/order,
  macOS or Linuxbrew). `brew audit --cask --strict` + the real checksum are
  provable only in the tap repo after CI substitutes the `sha256`.
# Research Notes — P1.M6.T1.S1 (community channel sections in docs/installation.md)

Scope: update **docs/installation.md ONLY** to document the F15 community
package-manager channels. Read-only audit of the inputs (channel READMEs, PRD §5,
external_deps.md). This agent writes the PRP, not the docs.

## Scope boundary (siblings — do NOT touch these files)

| Sibling | File(s) | Status | Boundary |
|---------|---------|--------|----------|
| P1.M6.T1.S1 (**THIS**) | `docs/installation.md` | Researching | the ONLY file this task edits |
| P1.M6.T2.S1 | `README.md` | Planned | downstream; separate file |
| P1.M6.T2.S2 | `docs/llms_full.txt` + `PACKAGING.md` refs | Planned | regenerates llms_full FROM the source docs → it will pick up this task's edits as an INPUT (no conflict; my edits feed it) |
| P1.M5.T2.S2 (parallel) | `.github/workflows/{ci,release}.yml` | Implementing | CI jobs; zero overlap with docs |

⇒ This task edits ONLY `docs/installation.md`. It does NOT touch README.md,
llms_full.txt, PACKAGING.md, or any .yml/.rs file.

## The file being edited: docs/installation.md (current state)

- **Jekyll front-matter (lines 1-4)** — MUST PRESERVE verbatim:
  ```
  ---
  layout: default
  title: Installation
  permalink: /installation/
  ---
  ```
- **Heading structure** (verified `grep -n '^#' docs/installation.md`):
  - `# Installation Guide` (7)
  - `## Windows` (16) → `### Installer (Recommended)` (18), `### Build from Source` (34)
  - `## Linux` (49) → `### Linux (Hyprland Only)` (51) → `#### Arch Linux` (55), `#### Other Linux Distributions` (66)
  - `## macOS` (117) → `### Install from a release` (122), `### Launch at login` (130), `### Build from source (for developers)` (140)
  - `## Build from Source (Linux Only)` (185)
  - `## Verification` (211), `## Next Steps` (241)
- **The stale line to remove** — line 57:
  `Build the package from the local \`PKGBUILD\` (there is no AUR package):`
  (the parenthetical "(there is no AUR package)" is now FALSE — `qmkonnect-bin` exists.)
- **Internal-link convention**: `{{ site.baseurl }}/foo` (Liquid). External links: raw GitHub
  `https://raw.githubusercontent.com/...` and release/AUR URLs are already used.
- **Version**: `0.2.8` (Cargo.toml) — examples in the doc reference `v0.1.0` in one spot
  (line ~60: `releases/download/v0.1.0/qmkonnect`) which is ALREADY stale/unrelated; do not
  "fix" it (out of scope — that's the generic binary-download URL, a separate pre-existing issue).

## The install matrix (authoritative commands — verbatim from each channel README)

| Channel | Platform | Exact install command(s) | Source README |
|---------|----------|--------------------------|---------------|
| **AUR** (`qmkonnect-bin`) | Linux | `yay -S qmkonnect-bin` (or `paru -S qmkonnect-bin`) | packaging/linux/aur/README.md |
| **Nix flake** | Linux (NixOS + non-NixOS) | `nix profile install github:dabstractor/qmkonnect` (or `nix run …`) | packaging/nix/README.md |
| **Homebrew Cask** | macOS | `brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect` then `brew install --cask qmkonnect` | packaging/homebrew/README.md |
| **Scoop** | Windows | `scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnect` then `scoop install qmkonnect` | packaging/scoop/README.md |
| **Winget** | Windows | `winget install dabstractor.QMKonnect` (or `winget install qmkonnect` moniker) | packaging/winget/README.md |
| **asdf / mise** | Linux (full), macOS (CLI only) | asdf: `asdf plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect` + `asdf install qmkonnect latest`; mise: `mise plugin add qmkonnect …` + `mise install qmkonnect@latest` | packaging/asdf/README.md |

## Per-channel caveats (PRD §12 / external_deps.md §2-§4) — MUST appear in the doc

| Channel | Caveat | Source |
|---------|--------|--------|
| **Winget** | "Unverified publisher" prompt + SmartScreen (installer is UNSIGNED). Choose *More info → Run anyway*. Identical to running the unsigned direct installer. | external_deps.md §4; winget/README.md |
| **Homebrew** | Ships via a **custom tap** (`dabstractor/homebrew-qmkonnect`) until the DMG is Developer-ID-signed + notarized (then graduates to official homebrew-cask). The DMG is ad-hoc signed / unnotarized → use `--no-quarantine` (or `xattr -dr com.apple.quarantine`) or Gatekeeper blocks first launch ("damaged / can't be opened"). Screen Recording prompt still required. | external_deps.md §2; homebrew/README.md |
| **Scoop** | Extracts the Inno installer via `innounp` (does NOT run it) → autostart is **off** by default (enable "Open at Login" in the tray), no Add/Remove-Programs entry, toast AUMID not set (toasts render generically). No code-signing check (unaffected). | scoop/README.md; external_deps.md §3 |
| **asdf / mise (macOS)** | **CLI only** — installs the raw binary; the menu-bar tray does NOT work (needs the `.app` bundle). Use the Homebrew cask or direct DMG for the full app. | asdf/README.md |
| **asdf / mise (Windows)** | **NOT supported** — the asset is an Inno installer, not a portable binary. Use Scoop/Winget/Inno. | asdf/README.md |
| **Nix (non-NixOS)** | Cannot install udev rules system-wide → one-time manual udev setup (install the static rule + symlink the helper + reload udev). On NixOS the `nixosModules.default` handles it. | nix/README.md |

## Cross-cutting facts

- **mise + asdf share ONE plugin** (`asdf-qmkonnect`). mise runs asdf plugin scripts
  unchanged → no separate mise backend. Both resolve versions at runtime via the GitHub
  Releases API (no hard-coded version).
- **AUR `qmkonnect-bin`** vs the source PKGBUILD at `packaging/linux/arch/`: the `-bin`
  downloads the prebuilt release tarball (no Rust toolchain/build deps); the source PKGBUILD
  builds from source. Both install to the same 4 paths + reuse the same pacman hooks
  (`qmkonnect.install` → udev reload, systemd template instantiation, global enable).
- **Autostart parity**: Inno (direct + Winget) writes the HKCU `Run` value (default ON).
  Scoop extraction skips it (OFF — user toggles). AUR/Homebrew/Linux-binary rely on the
  shipped systemd/SMAppService mechanism.
- **PRD §5 compatibility matrix** (the cross-reference target): Windows 10/11 x64 (Inno ·
  Scoop · Winget), macOS 13+ (.dmg · Homebrew Cask), Linux/Hyprland (AUR · Nix · PKGBUILD/binary).
  mise+asdf cross-cut Linux + macOS (not Windows). MSRV Rust 1.88.

## Net deliverable set (all in docs/installation.md)

1. **REMOVE** the "(there is no AUR package)" parenthetical (line 57); reword the Arch
   section to point at the new AUR channel.
2. **ADD** a top "## Installation Methods" summary table (cross-refs PRD §5) right after
   the intro, before `## Windows`.
3. **ADD** a "### Package Managers" subsection under EACH platform:
   - Windows → Scoop + Winget (with the unverified-publisher caveat).
   - Linux → AUR + Nix (+ mise/asdf; note non-NixOS udev one-timer).
   - macOS → Homebrew (custom tap + --no-quarantine) + mise/asdf (CLI-only caveat).
4. **PRESERVE** Jekyll front-matter, the existing primary-install + build-from-source
   sections, the `{{ site.baseurl }}` link convention, and the Verification/Next-Steps sections.
5. **DO NOT** edit README.md, llms_full.txt, PACKAGING.md, any .yml/.rs/Cargo.toml, or
   the pre-existing `v0.1.0` binary-download URL (separate, out-of-scope staleness).
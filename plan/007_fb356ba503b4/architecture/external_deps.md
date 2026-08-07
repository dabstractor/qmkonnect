# External Dependencies & Community Channel Requirements

## Community Package Manager Channel Specs

### 1. AUR (Arch User Repository)
- **Package type**: `-bin` (binary) or `-git` (VCS) package
- **Key files**: `PKGBUILD`, `.SRCINFO`, optional `.install`
- **Publication**: `git push` to `aur.archlinux.org/qmkonnect-bin.git`
- **Per-user**: Inherent (pacman is system-level but user-installable via AUR helpers)
- **Current state**: `packaging/linux/arch/PKGBUILD` exists but configured for local makepkg, not AUR
- **Requirements**:
  - `source=()` must point to GitHub release artifacts (URL with `$pkgver`)
  - `sha256sums` must be populated (or `SKIP`)
  - `.SRCINFO` must be generated via `makepkg --printsrcinfo > .SRCINFO`
  - AUR git repo needs SSH key or token for CI publishing
- **CI approach**: Separate workflow job that pushes PKGBUILD + .SRCINFO to AUR on tag

### 2. Homebrew Cask (macOS)
- **Package type**: Cask (GUI app distributed as `.dmg`)
- **Key files**: `Casks/qmkonnect.rb`
- **Publication**: Custom tap (`brew tap mulletware/qmkonnect`) until notarized for official cask
- **Per-user**: Inherent (Homebrew is per-user)
- **Requirements**:
  - Cask DSL: `version`, `sha256`, `url`, `appcast`/`livecheck`, `pkg`/`app`
  - `caveats` for Screen Recording permission prompt
  - Auto-update mechanism: `livecheck do url "https://github.com/dabstractor/qmkonnect/releases/latest" strategy :header end`
- **CI approach**: Push cask file to homebrew tap repo on tag
- **Notarization note**: PRD §12 says "Homebrew ships via a custom tap until notarization qualifies it for the official cask"

### 3. Scoop (Windows)
- **Package type**: App manifest (JSON)
- **Key files**: `qmkonnect.json`
- **Publication**: Custom bucket (`scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnect`)
- **Per-user**: Inherent (Scoop is per-user by design)
- **Requirements**:
  - `version`, `url`, `hash` (SHA256), `extract_dir` or `installer`
  - `shortcuts` for Start Menu
  - `checkver` + `autoupdate` for version detection
  - `innosetup: true` if using Inno installer, or `extract_dir` for portable
- **CI approach**: Push manifest to bucket repo on tag
- **Signing note**: PRD §12 says Scoop is "unaffected (they don't enforce code-signing)"

### 4. Winget (Windows)
- **Package type**: Manifest (YAML, split into multiple files per spec version)
- **Key files**: `dabstractor.QMKonnect.yaml` (or `.installer.yaml`, `.locale.yaml`, etc.)
- **Publication**: PR to `microsoft/winget-pkgs` or custom source
- **Per-user**: Inno installer is per-user; Winget respects installer scope
- **Requirements**:
  - `PackageIdentifier`, `PackageVersion`, `Installers` array
  - `InstallerType: inno` (matching our Inno installer)
  - `InstallerSwitches` for silent install
  - SHA256 hash of installer
  - Publisher, homepage, license metadata
- **CI approach**: Automated PR to winget-pkgs via GitHub Action (e.g., `vedantmgoyal9/winget-pkgs-automation`)
- **Signing note**: PRD §12 says Winget prompts "unverified publisher"

### 5. Nix Flake (Linux)
- **Package type**: `flake.nix` with a `packages` output
- **Key files**: `flake.nix`, `flake.lock`
- **Publication**: Include in repo root; users run `nix run github:dabstractor/qmkonnect`
- **Per-user**: Nix is inherently per-user with user-level profiles
- **Requirements**:
  - `flake.nix` with `inputs.nixpkgs`, `outputs` producing `packages.x86_64-linux.qmkonnect`
  - Uses `rustPlatform.buildRustPackage` to build from source (PRD: "Nix builds from source")
  - Must handle Linux build deps: gtk3, hidapi, libxdo, etc.
  - Wrap udev rule installation in the package's setup hook or document manual steps
- **CI approach**: Flake lives in the repo; validation in CI (`nix flake check`)
- **Note**: The Arch PKGBUILD links `-lhidapi-hidraw`; the Nix build must do the same

### 6. mise (Cross-Platform Version Manager)
- **Package type**: mise plugin (TOML-based or backend-agnostic)
- **Key files**: Plugin repo `mise-qmkonnect` or `asdf-qmkonnect` (mise supports asdf plugins)
- **Per-user**: Inherent (mise installs into user prefix)
- **Requirements**:
  - mise can use asdf-compatible plugins directly
  - A `mise.toml` backend or an `asdf` plugin backend
  - Downloads GitHub release binary, extracts to `$MISE_INSTALL_PATH`
  - `bin/install` script that fetches the right OS/arch binary
- **Note**: Simplest path is to make the asdf plugin work with mise (mise is asdf-compatible)

### 7. asdf (Cross-Platform Version Manager)
- **Package type**: asdf plugin (shell scripts)
- **Key files**: Plugin repo `asdf-qmkonnect` with `bin/install`, `bin/list-all`, `bin/download`
- **Per-user**: Inherent (asdf installs into user prefix)
- **Requirements**:
  - `bin/list-all`: scrape GitHub releases API for versions
  - `bin/download`: fetch release tarball for the right OS/arch
  - `bin/install`: extract to `$ASDF_INSTALL_PATH`
  - Platform detection: map `uname -s`/`uname -m` to release artifact names
- **Note**: This plugin also serves mise users (mise is asdf-compatible)

## Cross-Cutting Concerns

### Version Source of Truth
All channels must derive version from `Cargo.toml` (via `cargo metadata`):
- CI extracts version with `jq` (Linux/macOS) or `ConvertFrom-Json` (Windows)
- AUR PKGBUILD: patch `pkgver` in CI
- Homebrew cask: patch `version` in CI
- Scoop manifest: patch `version` in CI
- Winget manifest: patch `PackageVersion` in CI
- mise/asdf: derive from GitHub releases API at runtime

### Hashing
- AUR: `sha256sums` in PKGBUILD
- Homebrew: `sha256` in cask
- Scoop: `hash` (SHA256) in manifest
- Winget: `InstallerSha256` in manifest
- Nix: handled by `fetchFromGitHub` or `fetchurl` in the flake
- mise/asdf: computed by the plugin at download time (or `sha256sum -c`)

### CI Publishing Strategy
For channels requiring repo pushes (AUR, Homebrew tap, Scoop bucket):
1. Store deploy keys / tokens as GitHub Actions secrets
2. On tag push (after GitHub Release publish), push updated manifests
3. Use `git clone` → update file → commit → push pattern

For Winget:
- Automated PR to `microsoft/winget-pkgs` using `wingetcreate` or the official bot

For Nix:
- Flake lives in-repo; CI runs `nix flake check`
- No external publishing needed
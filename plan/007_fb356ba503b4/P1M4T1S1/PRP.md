# PRP — P1.M4.T1.S1: Create asdf plugin scripts (list-all / download / install + utils.bash)

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). **Pure packaging — no Rust/source/CI change.**
> **Five new files, all under `packaging/asdf/`:** `lib/utils.bash`, `bin/list-all`, `bin/download`,
> `bin/install`, `README.md`. These are the **asdf plugin scripts** that download + install QMKonnect
> from its GitHub Releases. The **SAME scripts serve mise** (mise is asdf-compatible — it runs the
> plugin's `bin/*` unchanged; `architecture/external_deps.md` §6-7). This is the cross-platform-
> version-manager channel of **F15** (PRD §4; §5 platform row: "Linux/macOS/Windows · … · mise/asdf";
> §12 "mise/asdf are unaffected [by signing]").
> **Scope:** the 4 core scripts + the plugin README ONLY. The asdf plugin repo metadata + the mise
> native backend stub are sibling **P1.M4.T1.S2**. The CI job (`asdf plugin test` + push to the
> `asdf-qmkonnect` repo) is **P1.M5.T2.S2**.
> **Pattern:** this task is the asdf analogue of the COMPLETED AUR S1
> (`packaging/linux/aur/PKGBUILD` — the closest in-repo twin: it downloads the SAME Linux tarball,
> extracts the SAME binaries + udev + systemd, and installs them) and follows the same
> "download-from-GitHub-releases + compute/validate SHA256" flow as Scoop/Homebrew/Winget. Read the
> AUR PKGBUILD in full before writing — it is the layout/asset ground truth.

---

## ⚠️ CRITICAL DESIGN REALITY (read first — it shapes the whole task)

**asdf/mise are POSIX/bash version managers that run on Linux + macOS.** Classic asdf does NOT run on
Windows. QMKonnect's three release artifacts have very different natures, so the scripts handle each
honestly rather than pretending all three are "portable binaries":

| OS | Asset | asdf/mise reality |
|---|---|---|
| **Linux x86_64** | `qmkonnect-{ver}-linux-x86_64.tar.gz` (portable `qmkonnect` + `qmkonnect-hid-id` + udev rule + systemd template) | **PRIMARY — fully supported.** tar-extract → copy both binaries into `$ASDF_INSTALL_PATH/bin/`. The udev rule + systemd template need **root** (`udevadm`, `systemctl --global`) and can't be auto-installed under asdf's **per-user** model → stage them in `$ASDF_INSTALL_PATH/share/qmkonnect/` and document the one-time manual setup (mirror how Nix "documents manual steps"). |
| **macOS** (universal2) | `QMKonnect-{ver}-macos.dmg` (a full `.app` bundle) | **CLI-only.** `hdiutil attach` → copy the raw Mach-O `QMKonnect.app/Contents/MacOS/qmkonnect` → `hdiutil detach`. The raw binary runs CLI flags but **NOT the menu-bar tray** (needs the full `.app` bundle context: Info.plist/resources/icon/template-paths). **Document the caveat** (contract requirement) and point to the Homebrew cask / direct DMG for the full app. |
| Windows | `QMKonnect-{ver}-windows-x64.exe` (an Inno **installer**, not a portable binary) | **Not a real asdf target.** The `.exe` is a setup installer unsuitable for `$ASDF_INSTALL_PATH/bin`. The install script **errors with a redirect** to Scoop / Winget / the Inno installer. (download still maps `*_NT-*`/`MSYS*`/`MINGW*` → the .exe for completeness, but install refuses.) |

This matches the contract (Linux tar extract + macOS DMG mount/copy + the macOS CLI-only caveat) and
is truthful about Windows (PRD §12 itself scopes mise/asdf to "unaffected by signing" Linux/macOS-style
channels; Windows users have Scoop/Winget/Inno).

---

## Goal

**Feature Goal**: Stand up the **asdf plugin scripts** (and shared `utils.bash`) that let a user run
`asdf install qmkonnect <version>` (and the mise equivalent `mise install qmkonnect@<version>`) to
fetch and install QMKonnect from its GitHub Releases — primarily on Linux x86_64 (full), with macOS
support (CLI-only, caveated) and a clear Windows redirect. The scripts are the **same artifact** mise
consumes (mise runs asdf plugins unchanged), so one implementation serves both managers.

**Deliverable** (5 new files under `packaging/asdf/`):
1. `lib/utils.bash` — shared bash helpers (release identity, `uname`-based platform detection, version→asset
   name map, `curl` download, optional SHA256 sidecar verification, GitHub-API version listing, logging).
2. `bin/list-all` — prints all published bare versions (ascending, space-separated, newest last).
3. `bin/download` — maps `uname -s`/`uname -m` to the release artifact, downloads it into
   `$ASDF_DOWNLOAD_PATH` via `curl`, and validates SHA256 **if** a sidecar exists (none does today).
4. `bin/install` — Linux: tar-extract + copy binaries to `$ASDF_INSTALL_PATH/bin/` (+ stage udev/systemd);
   macOS: mount DMG + copy the raw binary + detach (+ CLI-only caveat); Windows: error + redirect.
5. `README.md` — asdf setup, mise setup, platform support matrix, macOS CLI-only caveat, Linux one-time
   udev/systemd setup, Windows redirect, version-source-of-truth note.

**Success Definition**:
- All 5 files exist under `packaging/asdf/`; the 3 `bin/` scripts are executable (`chmod +x`) with
  `#!/usr/bin/env bash` shebangs; `bash -n` passes on all 4 scripts; `shellcheck` (if installed) is clean.
- **End-to-end live test on the Linux dev box against the real 0.2.8 release:**
  - `bash packaging/asdf/bin/list-all` (with `ASDF_PLUGIN_PATH` faked) prints `… 0.2.8` with `0.2.8`
    LAST (newest) — validating the curl + grep + sed + portable-numeric-sort pipeline.
  - With `ASDF_INSTALL_VERSION=0.2.8 ASDF_DOWNLOAD_PATH=<tmp>`, `bin/download` fetches
    `qmkonnect-0.2.8-linux-x86_64.tar.gz` into `<tmp>`; `tar tzf` lists the 4 expected files; its sha256
    = `86dcaa57254f4b7dd23595ef8e473432c3997df3b6c12621913d215b9ab1b216`.
  - With `ASDF_INSTALL_*` + `ASDF_DOWNLOAD_PATH` set, `bin/install` produces
    `$ASDF_INSTALL_PATH/bin/qmkonnect` + `$ASDF_INSTALL_PATH/bin/qmkonnect-hid-id` (both executable,
    `--help` exits 0) and stages `69-qmkonnect-rawhid.rules` + `qmkonnect.service.template` under
    `$ASDF_INSTALL_PATH/share/qmkonnect/`.
- `git diff --stat` shows ONLY the 5 new files under `packaging/asdf/` (no Cargo/source/.github/other-packaging changes).
- (Deferred) The macOS DMG-binary path + the mise native run are validated by code review + (for the
  full menu-bar app) on a macOS host; the `asdf plugin test` CI run is P1.M5.T2.S2.

## User Persona (if applicable)

**Target User**: a **QMKonnect end user** (Linux primarily; macOS CLI users) who manages tools with
asdf or mise and wants `asdf install qmkonnect latest` (or `mise install qmkonnect@latest`) instead of a
manual download/extract. (The *maintainer* who publishes the plugin repo + the CI is served by S2/P1.M5.T2.S2.)

**Use Case**: `asdf plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect && asdf install
qmkonnect 0.2.8 && asdf global qmkonnect 0.2.8` puts `qmkonnect` (+ `qmkonnect-hid-id`) on PATH. On
Linux the user then runs the one-time udev/systemd setup (copied from the README) so the static udev
rule grants hidraw permissions and the per-user service starts at login.

**User Journey**: (1) `asdf plugin add` / `mise plugin add` the `asdf-qmkonnect` repo; (2) `asdf list
all qmkonnect` shows versions; (3) `asdf install qmkonnect <ver>` downloads + installs; (4) (Linux only)
follow the README's one-time root commands to wire udev + systemd; (5) (macOS) the README's caveat tells
them the tray needs the Homebrew cask / DMG, not the asdf raw binary.

**Pain Points Addressed**: gives Linux/macOS users who live in asdf/mise a native install path alongside
the direct installers (F8) and the other community channels (F15: AUR/Homebrew/Scoop/Winget/Nix). Fills
the "cross-platform version manager" row of F15.

## Why

- **F15 (PRD §4) requires a mise/asdf channel.** This task ships the plugin scripts that make it work;
  S2 ships the plugin-repo metadata + the mise native backend stub; P1.M5.T2.S2 wires `asdf plugin test`
  + the publish (copy `packaging/asdf/` → the `asdf-qmkonnect` repo on tag). Per `external_deps.md` §6-7,
  the asdf plugin is reused by mise unchanged — **one implementation, two managers**.
- **Mirrors the proven AUR S1 tarball-install pattern.** `packaging/linux/aur/PKGBUILD` (COMPLETE)
  downloads the EXACT same `qmkonnect-{ver}-linux-x86_64.tar.gz`, extracts the SAME 4 files, and installs
  them to system paths. The asdf `install` script does the same extraction but installs into asdf's
  per-user `$ASDF_INSTALL_PATH/bin/` (and stages the root-needing udev/systemd files for manual setup).
- **No new toolchain.** The scripts are portable bash (`#!/usr/bin/env bash`) depending only on `curl`,
  `tar`, `grep`, `sed`, `sort`, `install`, and (macOS) `hdiutil` — all standard on Linux + macOS. No jq
  dependency (the tag_name JSON line is parsed with grep/sed for portability).

## What

### Naming Truth (GROUND TRUTH — verified this session)

- `git remote get-url origin` → `git@github.com:dabstractor/qmkonnect.git` ⇒ **GitHub org = `dabstractor`**.
  (Local Linux user is `dustin` — UNRELATED.)
- Source repo = **`dabstractor/qmkonnect`**. **Tool/plugin name = `qmkonnect`** (the contract's mise
  example `mise plugin add qmkonnet …` is a TYPO — missing `c`; use **`qmkonnect`** everywhere).
- **Plugin repo = `asdf-qmkonnect`** (asdf convention `asdf-{toolname}`), URL
  `https://github.com/dabstractor/asdf-qmkonnect`. (S2 owns the repo's creation/metadata.)
- **Assets** (release.yml + AUR PKGBUILD + Scoop + Homebrew, all consistent):
  - macOS: `QMKonnect-<version>-macos.dmg` (universal2: arm64 + x86_64)
  - Windows: `QMKonnect-<version>-windows-x64.exe` (Inno installer)
  - Linux: `qmkonnect-<version>-linux-x86_64.tar.gz`
- **URL pattern**: `https://github.com/dabstractor/qmkonnect/releases/download/v<version>/<asset>`
  (tag is `v<version>`; version/asset-name are bare — re-add the `v` for the URL path only).
- **Releases API**: `https://api.github.com/repos/dabstractor/qmkonnect/releases` → JSON, lines
  `"tag_name": "v0.2.8",`. list-all greps these, strips the leading `v`.
- **NO `.sha256` sidecar** is published (Scoop hash is all-zeros placeholder; AUR hardcodes; Homebrew
  `:no_check`; Winget 64-zero) ⇒ download validates SHA256 **only if** a sidecar probes 200 (today: never);
  it must NOT invent a sidecar URL.
- **Two binaries** ship in the Linux tarball: `qmkonnect` (main) + `qmkonnect-hid-id` (udev helper,
  `src/bin/hid_id.rs`, `Cargo.toml:132`). Both go into `$ASDF_INSTALL_PATH/bin/`.

### asdf contract this task implements (verified: asdf-vm.com/plugins/create.html + asdf source)

- asdf runs the **`download`→`install` chain**: `bin/download` first (into `$ASDF_DOWNLOAD_PATH`), then
  `bin/install` (reads from `$ASDF_DOWNLOAD_PATH`, writes to `$ASDF_INSTALL_PATH`).
- Env vars asdf sets for the scripts: `ASDF_INSTALL_TYPE=version`, `ASDF_INSTALL_VERSION`,
  `ASDF_INSTALL_PATH`, `ASDF_DOWNLOAD_PATH`, `ASDF_CONCURRENCY`, `ASDF_PLUGIN_PATH`.
- **`bin/list-all`** prints available versions (classic: space-separated one line; asdf tokenizes on
  whitespace). **`bin/download`** exits 0 on success / non-zero on failure (failure aborts install).
  **`bin/install`** installs into `$ASDF_INSTALL_PATH`; the default `list-bin-paths` returns `bin` ⇒
  executables land in **`$ASDF_INSTALL_PATH/bin/`** (what asdf shims).
- Platform detection: asdf passes NO OS/arch env vars; the scripts call `uname -s` / `uname -m`.
- **`latest` resolution without a `bin/latest-stable` callback** (that callback is S2's territory): asdf
  resolves `latest` from list-all output. To make `asdf install qmkonnect latest` correct, **list-all
  outputs versions ASCENDING (oldest→newest)** via a **portable numeric sort** — `sort -t. -k1,1n -k2,2n
  -k3,3n` — which works on BOTH GNU coreutils and BSD sort (do NOT use `sort -V`: BSD sort on macOS may
  lack it). Newest is last → correct `latest`.
- **mise** auto-detects asdf plugins and runs the same `bin/*` scripts → **zero plugin changes** for mise.

### Success Criteria
- [ ] `packaging/asdf/{lib/utils.bash,bin/list-all,bin/download,bin/install,README.md}` all exist; the 3
      `bin/` scripts are `chmod +x` with `#!/usr/bin/env bash`.
- [ ] `bash -n` passes on all 4 scripts; `shellcheck` (if available) is clean.
- [ ] Live Linux test (Validation Level 2): list-all prints `… 0.2.8` (0.2.8 last); download fetches the
      0.2.8 tarball (sha256 `86dcaa57…b216`); install produces `$ASDF_INSTALL_PATH/bin/{qmkonnect,qmkonnect-hid-id}`.
- [ ] The macOS DMG-binary extract path (hdiutil) + Windows redirect are present, correct by review.
- [ ] `git diff --stat` = exactly the 5 new files under `packaging/asdf/`.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior asdf knowledge can create the 4 scripts verbatim from "What → Files 1-4"
(full reference implementations are given) and the README verbatim from "What → File 5", then validate
on the Linux dev box via `bash -n` + `shellcheck` + the live list-all/download/install run against the
real 0.2.8 release (Validation Level 2) + grep invariants (Level 1). The asdf/mise contract, the exact
asset names, the Linux tarball layout, and the no-sidecar reality are all verified and documented.

### Documentation & References

```yaml
# MUST READ — the authoritative asdf plugin contract (the env vars + download→install chain)
- url: https://asdf-vm.com/plugins/create.html
  why: confirms ASDF_INSTALL_TYPE/ASDF_INSTALL_VERSION/ASDF_INSTALL_PATH/ASDF_DOWNLOAD_PATH; that asdf
       runs bin/download BEFORE bin/install; that install reads from ASDF_DOWNLOAD_PATH; that list-all
       prints available versions; the default list-bin-paths=bin (so executables go in ASDF_INSTALL_PATH/bin/).
  critical: "bin/download exit-0 => install runs; non-zero => install skipped. install MUST place
       executables in ASDF_INSTALL_PATH/bin/ (what asdf shims). asdf passes NO OS/arch env vars —
       the plugin calls uname -s / uname -m itself."

# MUST READ — the CLOSEST in-repo twin (downloads + extracts the SAME Linux tarball + binaries)
- file: packaging/linux/aur/PKGBUILD
  why: the ground truth for the Linux tarball LAYOUT (top-level qmkonnect-<ver>-linux-x86_64/ with
       qmkonnect, qmkonnect-hid-id, udev/69-qmkonnect-rawhid.rules, systemd/qmkonnect.service.template),
       the tarball URL, and the real 0.2.8 sha256 (86dcaa57254f4b7dd23595ef8e473432c3997df3b6c12621913d215b9ab1b216).
  pattern: "source=(URL); extract; install -Dm755 the binaries; install -Dm644 the udev rule + service template."
  gotcha: "the AUR installs to SYSTEM paths (/usr/bin, /usr/lib/udev, /usr/lib/systemd) which need root;
       asdf is PER-USER so it installs binaries to ASDF_INSTALL_PATH/bin and STAGES the udev/systemd
       files under ASDF_INSTALL_PATH/share/qmkonnect for manual one-time setup (see README)."

# MUST READ — the one-time Linux root setup steps the README documents (adapted from these pacman hooks)
- file: packaging/linux/aur/qmkonnect.install
  why: post_install shows the EXACT udev/systemd commands (udevadm control --reload-rules; udevadm trigger;
       install the service template to /usr/lib/systemd/user/; systemctl --global enable). The README
       adapts these for asdf's per-user model (the user runs the sudo udev steps + a per-user systemd copy).

# MUST READ — asset naming + version-from-cargo + no-sha256-sidecar (the INPUT facts the scripts encode)
- file: .github/workflows/release.yml
  why: the linux-binary job (lines 168-178) stages the tarball; the macos job (line 84) renames
       QMKonnect.dmg -> QMKonnect-<ver>-macos.dmg; the windows job (line 130) renames to
       QMKonnect-<ver>-windows-x64.exe; version has NO leading v. grep confirms NO .sha256 sidecar upload.
  gotcha: "version is bare; the URL path adds the v (.../v0.2.8/...); the asset filename uses the bare version."

# MUST READ — the architecture decision + mise/asdf contract this implements
- docfile: plan/007_fb356ba503b4/architecture/external_deps.md
  why: §6 (mise — 'downloads GitHub release binary, extracts to $MISE_INSTALL_PATH; mise is asdf-compatible;
       simplest path is to make the asdf plugin work with mise') + §7 (asdf — 'bin/list-all: scrape GitHub
       releases API; bin/download: fetch release tarball for the right OS/arch; bin/install: extract to
       ASDF_INSTALL_PATH; platform detection: map uname -s/uname -m'). §'Version Source of Truth' (mise/asdf
       derive version from GitHub releases API at runtime) + §'Hashing' (mise/asdf: computed by the plugin
       at download time, or sha256sum -c).
  section: "6. mise" + "7. asdf" + "Version Source of Truth" + "Hashing"

# REFERENCE — the COMPLETED in-source download+hash precedents (tone + curl/SHA256 flow)
- file: packaging/scoop/qmkonnect.json
  why: confirms the Windows asset URL pattern + the bare-version convention + that the hash is a
       placeholder the channel fills (asdf computes/validates at runtime instead).
- file: packaging/homebrew/Casks/qmkonnect.rb
  why: confirms the macOS DMG asset URL pattern + the 'app QMKonnect.app' bundle layout (the raw binary
       the asdf install script extracts from Contents/MacOS/qmkonnect).

# REFERENCE — sibling/contract references
- docfile: plan/007_fb356ba503b4/P1M3T2S2/PRP.md   # the parallel sibling (Winget S2) — confirms repo
  why: the 'Naming Truth' (org dabstractor; version bare; URL v-prefixed path), the 'no .sha256 sidecar'
       reality, the download→hash flow, and that it touches ONLY packaging/winget/* (ZERO overlap with this task).
- docfile: plan/007_fb356ba503b4/P1M4T1S1/research/findings.md   # this task's research (full findings + script design)

# REFERENCE — PRD context
- url: spec/PRD.md
  why: §4 F15 (community package-manager distribution); §5 platform row (mise/asdf); §12 signing note
       ('mise/asdf are unaffected [by signing]').
  section: "h2.3 (4. F15)" + "h2.4 (5. Supported Platforms)" + "h2.11 (12. Beta Status)"
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
packaging/
  linux/
    aur/PKGBUILD               # <<< CLOSEST twin: same tarball, same 4 files, system-path install >>>
    aur/qmkonnect.install      # <<< the one-time udev/systemd steps the README documents >>>
    udev/69-qmkonnect-rawhid.rules          # (inside the tarball)
    systemd/qmkonnect.service.template      # (inside the tarball)
  homebrew/Casks/qmkonnect.rb  # macOS DMG asset URL + .app bundle layout
  scoop/qmkonnect.json         # Windows asset URL + bare-version + hash-placeholder pattern
  windows/inno/QMKonnect.iss   # Windows installer (Publisher/Name)
  (NO asdf/ dir yet — this task creates it)
.github/workflows/release.yml  # asset naming + version-from-cargo + NO sha256 sidecar + linux-binary staging
Cargo.toml                     # version=0.2.8; two bins: qmkonnect + qmkonnect-hid-id (src/bin/hid_id.rs)
# NEW (this task): packaging/asdf/{lib/utils.bash, bin/list-all, bin/download, bin/install, README.md}
```

### Desired Codebase tree (files this task ADDS)

```bash
packaging/asdf/
├── lib/
│   └── utils.bash             # NEW — shared helpers (release id, platform detect, asset map, curl, SHA256-if-sidecar, version list)
├── bin/
│   ├── list-all               # NEW — prints bare versions ascending (newest last) from the GitHub releases API
│   ├── download               # NEW — uname→asset map, curl into $ASDF_DOWNLOAD_PATH, verify SHA256 if sidecar
│   └── install                # NEW — Linux tar-extract / macOS DMG-binary / Windows redirect → $ASDF_INSTALL_PATH/bin
└── README.md                  # NEW — asdf + mise setup, platform matrix, macOS CLI-only caveat, Linux one-time setup, Windows redirect
```
(No other files. The asdf plugin repo metadata + mise native backend = P1.M4.T1.S2; the CI `asdf plugin test`
+ publish = P1.M5.T2.S2; docs/installation.md mise/asdf row = P1.M6.T1.S1.)

### Known Gotchas of our codebase & Library Quirks

```bash
# CRITICAL (asdf is PER-USER; udev/systemd need ROOT): the Linux tarball ships a static udev rule + a
#   systemd template that install to /usr/lib/... under the AUR package. asdf CANNOT run those root
#   commands. So bin/install puts the binaries in ASDF_INSTALL_PATH/bin (asdf shims them) and STAGES the
#   udev rule + service template under ASDF_INSTALL_PATH/share/qmkonnect for a one-time manual setup the
#   README documents. Do NOT auto-run udevadm/systemctl from the install script.

# CRITICAL (macOS raw binary ≠ the menu-bar app): the DMG contains a full QMKonnect.app bundle. Extracting
#   only Contents/MacOS/qmkonnect gives a CLI-capable Mach-O but the menu-bar tray/icon WILL NOT work
#   (needs Info.plist/resources/template-paths from the bundle). Document this caveat in the README and
#   point to the Homebrew cask / direct DMG for the full app. (Contract requirement.)

# CRITICAL (Windows asset is an INSTALLER, not portable): QMKonnect-<ver>-windows-x64.exe is an Inno Setup
#   installer. It is NOT a portable binary for ASDF_INSTALL_PATH/bin. bin/install REFUSES on Windows
#   (*_NT-*/MSYS*/MINGW*) with a redirect to Scoop/Winget/Inno. (Classic asdf doesn't run on Windows anyway.)

# CRITICAL (NO .sha256 sidecar exists): the download script must NOT invent a sidecar URL. 'validate SHA256
#   if sidecar exists' => probe ${url}.sha256 with curl --head; on HTTP 200, sha256sum -c it; on anything
#   else (404 today), proceed WITHOUT verification (log a note). The hash is carried by the AUR/Homebrew/
#   Scoop/Winget manifests, not a release sidecar.

# CRITICAL (org != local user): GitHub org = dabstractor (git remote). Local Linux user dustin is UNRELATED.
#   Tool name = qmkonnect (the contract's 'mise plugin add qmkonnet' is a TYPO — use qmkonnect). Plugin
#   repo = asdf-qmkonnect. Do NOT write dustin./Dustin./Mulletware. as the tool/repo name.

# CRITICAL (version v-prefix): tags are v0.2.8; list-all strips the leading v (prints bare 0.2.8); download
#   RE-ADDS the v for the URL path (.../v0.2.8/...); the asset filename uses the bare version. Users type
#   'asdf install qmkonnect 0.2.8' (bare).

# CRITICAL (portable version sort, NOT sort -V): list-all sorts ascending so the NEWEST is LAST (asdf
#   resolves 'latest' from list-all output when no bin/latest-stable callback exists). Use
#   'sort -t. -k1,1n -k2,2n -k3,3n' (works on GNU coreutils AND BSD sort). Do NOT use 'sort -V' — BSD sort
#   on macOS may lack it. Do NOT add bin/latest-stable (that's P1.M4.T1.S2's plugin-repo-metadata territory).

# GOTCHA (download→install chain): asdf runs bin/download FIRST into ASDF_DOWNLOAD_PATH, THEN bin/install
#   (which reads from ASDF_DOWNLOAD_PATH). install must NOT re-download; it reads the cached artifact.

# GOTCHA (bin/ must be executable): chmod +x the 3 bin/ scripts; shebang #!/usr/bin/env bash. lib/utils.bash
#   is sourced (NOT executable); it defines functions only (no set -e / no top-level side effects).

# GOTCHA (no jq dependency): parse the GitHub releases JSON tag_name line with grep + sed (portable). Do
#   NOT require jq (not guaranteed installed). Required commands: curl, tar, grep, sed, sort, install
#   (all standard on Linux+macOS) + hdiutil (macOS only).

# GOTCHA (set -euo pipefail): the 3 bin/ scripts use it. Guard the optional SHA256 probe with '|| true'
#   so a 404 (no sidecar) does NOT abort under set -e. utils.bash must NOT set -e (it's sourced; the
#   caller owns the shell options).

# GOTCHA (scope): do NOT add bin/latest-stable, a mise.toml backend, or plugin-repo metadata (P1.M4.T1.S2);
#   do NOT edit .github/workflows/* (CI = P1.M5.T2.S2) or docs/installation.md (P1.M6.T1.S1).
```

## Implementation Blueprint

### Data models and structure
No code data models. The scripts encode static facts (repo, asset patterns, version) as bash variables in
`utils.bash` and pass asdf's env vars (`ASDF_INSTALL_VERSION/PATH/DOWNLOAD_PATH`) through. No types.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CREATE packaging/asdf/lib/utils.bash (sourced helpers — author the reference from "What → File 1")
  - IMPLEMENT: the verbatim utils.bash from "What → File 1" (QMKONNECT_GITHUB_* identity consts;
    asdf_qmkonnect_info/fail logging; asdf_qmkonnect_require_cmds; asdf_qmkonnect_detect_platform
    [sets ASDF_QMKONNECT_OS/ARCH/PLATFORM_OK]; asdf_qmkonnect_asset_for <version> [prints asset name];
    asdf_qmkonnect_download_url <version> <asset>; asdf_qmkonnect_curl <url> <dest> [--fail --location];
    asdf_qmkonnect_verify_sha256_if_sidecar <url> <file> [probe ${url}.sha256; 200=>sha256sum -c; else skip];
    asdf_qmkonnect_list_versions [curl API/releases; grep '"tag_name"'; sed strip v; grep numeric; sort -t. ascending; tr to space]).
  - NAMING: function prefix asdf_qmkonnect_* (snake_case); globals ASDF_QMKONNECT_*.
  - PLACEMENT: packaging/asdf/lib/utils.bash (sourced, NOT executable; functions only, no set -e).

Task 2: CREATE packaging/asdf/bin/list-all
  - IMPLEMENT: the verbatim list-all from "What → File 2" (set -euo pipefail; source ../lib/utils.bash;
    call asdf_qmkonnect_list_versions; prints bare versions ascending, space-separated, newest LAST).
  - DEPENDS: Task 1 (utils.bash).
  - PLACEMENT: packaging/asdf/bin/list-all; chmod +x.

Task 3: CREATE packaging/asdf/bin/download
  - IMPLEMENT: the verbatim download from "What → File 3" (set -euo pipefail; source utils; assert
    ASDF_INSTALL_VERSION/ASDF_DOWNLOAD_PATH; mkdir -p ASDF_DOWNLOAD_PATH; asdf_qmkonnect_asset_for;
    asdf_qmkonnect_download_url; asdf_qmkonnect_curl into ASDF_DOWNLOAD_PATH/<asset>;
    asdf_qmkonnect_verify_sha256_if_sidecar).
  - DEPENDS: Task 1. NAMING: bin/download; chmod +x.

Task 4: CREATE packaging/asdf/bin/install
  - IMPLEMENT: the verbatim install from "What → File 4" (set -euo pipefail; source utils; assert env
    vars; detect platform; case OS: Linux [tar -xzf; install -m755 binaries to ASDF_INSTALL_PATH/bin;
    stage udev+systemd to ASDF_INSTALL_PATH/share/qmkonnect], macOS [hdiutil attach -nobrowse -mountpoint;
    cp Contents/MacOS/qmkonnect; hdiutil detach; CLI-only caveat], Windows [*_NT-*/MSYS*/MINGW* => fail
    with Scoop/Winget/Inno redirect]).
  - DEPENDS: Task 1. NAMING: bin/install; chmod +x.

Task 5: CREATE packaging/asdf/README.md (author verbatim from "What → File 5")
  - IMPLEMENT: the verbatim README from "What → File 5" (What this is; asdf setup [plugin add
    https://github.com/dabstractor/asdf-qmkonnect; install; global]; mise setup [mise plugin add;
    mise install qmkonnect@latest]; Platform Support table [Linux full / macOS CLI-only / Windows
    redirect]; macOS CLI-only caveat + Homebrew/direct-DMG pointer; Linux one-time udev/systemd setup
    [the sudo commands + per-user systemd copy, adapted from qmkonnect.install]; Windows redirect to
    Scoop/Winget/Inno; Version source-of-truth note).
  - PLACEMENT: packaging/asdf/README.md.

Task 6: VALIDATE (no edits)
  - chmod +x the 3 bin/ scripts; bash -n all 4; shellcheck (if present).
  - Live Linux test: list-all (API), download (0.2.8 tarball, sha256 86dcaa57…b216), install (binaries + staged files).
  - grep invariants (Validation Level 1); git diff --stat (exactly 5 new files).

Task 7: NEVER do these (out of scope / forbidden)
  - DO NOT add bin/latest-stable, a mise native backend (mise.toml), or plugin-repo metadata files (P1.M4.T1.S2).
  - DO NOT auto-run udevadm/systemctl/sudo from bin/install (asdf is per-user; the README documents the one-time setup).
  - DO NOT invent a .sha256 sidecar URL (none exists; verify-if-200-else-skip).
  - DO NOT use 'sort -V' (BSD sort portability) — use 'sort -t. -k1,1n -k2,2n -k3,3n'.
  - DO NOT require jq (parse tag_name with grep/sed).
  - DO NOT re-download in bin/install (read the cached artifact from ASDF_DOWNLOAD_PATH).
  - DO NOT use the typo 'qmkonnet' — the tool name is 'qmkonnect'.
  - DO NOT put set -e in lib/utils.bash (it's sourced; the caller owns shell options).
  - DO NOT edit .github/workflows/* (CI = P1.M5.T2.S2) or docs/installation.md (P1.M6.T1.S1).
  - DO NOT change any Rust source / Cargo.toml / other packaging dir.
  - DO NOT edit PRD.md, any tasks.json, or prd_snapshot.md.
```

### File 1 — `packaging/asdf/lib/utils.bash` (reference — author verbatim)

```bash
#!/usr/bin/env bash
# Shared helpers for the asdf-qmkonnect plugin scripts (bin/list-all, bin/download,
# bin/install). SOURCED by those scripts — NOT a standalone executable. Defines
# functions only; does NOT `set -e` (the caller owns shell options).
#
# These same scripts serve mise unchanged (mise is asdf-compatible: it runs the
# plugin's bin/* directly). See packaging/asdf/README.md.

# ── QMKonnect release identity (single source of truth) ──────────────────────
readonly QMKONNECT_GITHUB_REPO="dabstractor/qmkonnect"
readonly QMKONNECT_GITHUB_API="https://api.github.com/repos/dabstractor/qmkonnect"
readonly QMKONNECT_RELEASE_BASE="https://github.com/dabstractor/qmkonnect/releases/download"

# ── Logging ──────────────────────────────────────────────────────────────────
asdf_qmkonnect_info() { printf '%s\n' "$*" >&2; }
asdf_qmkonnect_fail() { printf 'asdf-qmkonnect: %s\n' "$*" >&2; exit 1; }

# Fail unless every named command is on PATH.
#   asdf_qmkonnect_require_cmds curl tar grep
asdf_qmkonnect_require_cmds() {
    local missing=() c
    for c in "$@"; do
        command -v "$c" >/dev/null 2>&1 || missing+=("$c")
    done
    [ "${#missing[@]}" -eq 0 ] || asdf_qmkonnect_fail "missing required command(s): ${missing[*]}"
}

# Detect the host (OS, arch). Sets globals:
#   ASDF_QMKONNECT_OS   = `uname -s` verbatim
#   ASDF_QMKONNECT_ARCH = `uname -m` verbatim
#   ASDF_QMKONNECT_PLATFORM_OK = 1 (Linux x86_64 / macOS) | 0 (unsupported) | 2 (Windows/Git-Bash)
asdf_qmkonnect_detect_platform() {
    ASDF_QMKONNECT_OS="$(uname -s)"
    ASDF_QMKONNECT_ARCH="$(uname -m)"
    case "$ASDF_QMKONNECT_OS" in
        Darwin)              ASDF_QMKONNECT_PLATFORM_OK=1 ;;   # universal2 DMG (arm64 + x86_64)
        Linux)
            case "$ASDF_QMKONNECT_ARCH" in
                x86_64|amd64) ASDF_QMKONNECT_PLATFORM_OK=1 ;;
                *)            ASDF_QMKONNECT_PLATFORM_OK=0 ;;
            esac ;;
        *_NT-*|MSYS*|MINGW*) ASDF_QMKONNECT_PLATFORM_OK=2 ;;   # Windows under Git Bash (installer, not portable)
        *)                   ASDF_QMKONNECT_PLATFORM_OK=0 ;;
    esac
}

# Print the release asset filename for <version> on the detected platform.
#   asdf_qmkonnect_asset_for 0.2.8   ->  qmkonnect-0.2.8-linux-x86_64.tar.gz
# Exits 1 (via asdf_qmkonnect_fail) on an unsupported platform.
asdf_qmkonnect_asset_for() {
    local version="$1"
    asdf_qmkonnect_detect_platform
    case "$ASDF_QMKONNECT_OS" in
        Darwin) printf 'QMKonnect-%s-macos.dmg\n' "$version" ;;
        Linux)
            case "$ASDF_QMKONNECT_ARCH" in
                x86_64|amd64) printf 'qmkonnect-%s-linux-x86_64.tar.gz\n' "$version" ;;
                *) asdf_qmkonnect_fail "unsupported Linux arch '$ASDF_QMKONNECT_ARCH' (only x86_64 is released)" ;;
            esac ;;
        *_NT-*|MSYS*|MINGW*) printf 'QMKonnect-%s-windows-x64.exe\n' "$version" ;;
        *) asdf_qmkonnect_fail "unsupported OS '$ASDF_QMKONNECT_OS' (asdf-qmkonnect supports Linux x86_64 + macOS)" ;;
    esac
}

# Build the GitHub release download URL.
#   asdf_qmkonnect_download_url <version> <asset>
asdf_qmkonnect_download_url() {
    # The URL path is v-prefixed (tag v<version>); the version/asset name are bare.
    printf '%s/v%s/%s\n' "$QMKONNECT_RELEASE_BASE" "$1" "$2"
}

# Download <url> to <dest> with curl, failing hard on HTTP/network errors.
#   asdf_qmkonnect_curl <url> <dest>
asdf_qmkonnect_curl() {
    asdf_qmkonnect_require_cmds curl
    curl --fail --location --silent --show-error "$1" --output "$2" \
        || asdf_qmkonnect_fail "download failed: $1"
}

# Best-effort SHA256 verification. If a sidecar `<url>.sha256` exists (HTTP 200),
# verify <file> against it; otherwise proceed WITHOUT verification. QMKonnect
# releases do NOT currently publish SHA256 sidecars (the hash is carried by the
# AUR/Homebrew/Scoop/Winget manifests instead), so this is future-proofing: it
# never hard-fails when there is no sidecar. Safe under `set -e` (guarded).
#   asdf_qmkonnect_verify_sha256_if_sidecar <asset_url> <downloaded_file>
asdf_qmkonnect_verify_sha256_if_sidecar() {
    local url="$1" file="$2" sidecar code sum expected
    sidecar="${url}.sha256"
    code="$(curl --silent --location --head --output /dev/null --write-out '%{http_code}' "$sidecar" 2>/dev/null || true)"
    if [ "$code" != "200" ]; then
        asdf_qmkonnect_info "    (no SHA256 sidecar at $(basename "$sidecar"); skipping verification)"
        return 0
    fi
    asdf_qmkonnect_require_cmds sha256sum
    sum="$(sha256sum "$file" | awk '{print $1}')"
    expected="$(curl --fail --location --silent --show-error "$sidecar")"
    expected="${expected%% *}"   # tolerate "<hash>  <name>" or bare "<hash>"
    [ "$sum" = "$expected" ] \
        || asdf_qmkonnect_fail "SHA256 mismatch for $(basename "$file") (got $sum, sidecar says $expected)"
    asdf_qmkonnect_info "    SHA256 verified ($sum)"
}

# Print all published versions, ASCENDING (oldest→newest, space-separated, one
# line). Newest is LAST so `asdf install qmkonnect latest` resolves correctly
# even without a bin/latest-stable callback. jq-free (portable grep/sed parse).
asdf_qmkonnect_list_versions() {
    asdf_qmkonnect_require_cmds curl grep sed sort
    # GitHub /releases returns newest-first JSON; each release has "tag_name":"v<ver>".
    curl --fail --silent --show-error "$QMKONNECT_GITHUB_API/releases" \
        | grep '"tag_name"' \
        | sed -E 's/.*"tag_name"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/' \
        | sed -E 's/^v//' \
        | grep -E '^[0-9]' \
        | sort -t. -k1,1n -k2,2n -k3,3n -k4,4n \
        | tr '\n' ' '
}
```

### File 2 — `packaging/asdf/bin/list-all` (reference — author verbatim)

```bash
#!/usr/bin/env bash
# asdf-qmkonnect: bin/list-all
# Prints every published QMKonnect version (bare, no leading 'v'), ASCENDING
# (oldest first; newest last), space-separated on one line. asdf resolves
# `asdf install qmkonnect latest` from this output (newest = last entry).
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/../lib/utils.bash"

asdf_qmkonnect_list_versions
echo   # asdf tolerates the trailing newline; some tooling expects a final newline
```

### File 3 — `packaging/asdf/bin/download` (reference — author verbatim)

```bash
#!/usr/bin/env bash
# asdf-qmkonnect: bin/download
# Downloads the QMKonnect release artifact for $ASDF_INSTALL_VERSION + the
# detected OS/arch into $ASDF_DOWNLOAD_PATH. asdf runs this BEFORE bin/install
# (install reads the cached artifact from $ASDF_DOWNLOAD_PATH). Exit 0 = success
# (install then runs); non-zero = failure (install is skipped).
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/../lib/utils.bash"

[ -n "${ASDF_INSTALL_VERSION:-}" ] || asdf_qmkonnect_fail "ASDF_INSTALL_VERSION is not set (run via 'asdf install qmkonnect <version>')"
[ -n "${ASDF_DOWNLOAD_PATH:-}" ]   || asdf_qmkonnect_fail "ASDF_DOWNLOAD_PATH is not set"
mkdir -p "$ASDF_DOWNLOAD_PATH"

version="$ASDF_INSTALL_VERSION"
asset="$(asdf_qmkonnect_asset_for "$version")"
url="$(asdf_qmkonnect_download_url "$version" "$asset")"

asdf_qmkonnect_info "==> Downloading QMKonnect $version"
asdf_qmkonnect_info "    asset: $asset"
asdf_qmkonnect_info "    url:   $url"
asdf_qmkonnect_curl "$url" "$ASDF_DOWNLOAD_PATH/$asset"
asdf_qmkonnect_verify_sha256_if_sidecar "$url" "$ASDF_DOWNLOAD_PATH/$asset"
asdf_qmkonnect_info "==> Cached to $ASDF_DOWNLOAD_PATH/$asset"
```

### File 4 — `packaging/asdf/bin/install` (reference — author verbatim)

```bash
#!/usr/bin/env bash
# asdf-qmkonnect: bin/install
# Installs QMKonnect into $ASDF_INSTALL_PATH/bin/ from the artifact cached by
# bin/download in $ASDF_DOWNLOAD_PATH (asdf runs download FIRST, then this).
#
# Platform handling (see the README for the rationale):
#   Linux x86_64 — PRIMARY: tar-extract the tarball; copy qmkonnect +
#                  qmkonnect-hid-id into ASDF_INSTALL_PATH/bin; stage the udev
#                  rule + systemd template under ASDF_INSTALL_PATH/share/qmkonnect
#                  (they need ROOT — the README documents the one-time manual setup).
#   macOS        — CLI-only: mount the DMG, copy the raw Mach-O binary out of
#                  QMKonnect.app/Contents/MacOS/qmkonnect, detach. The menu-bar
#                  tray needs the full .app bundle — use the Homebrew cask / DMG.
#   Windows      — REFUSE: the asset is an Inno SETUP INSTALLER, not a portable
#                  binary; redirect to Scoop/Winget/the Inno installer.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/../lib/utils.bash"

[ -n "${ASDF_INSTALL_VERSION:-}" ] || asdf_qmkonnect_fail "ASDF_INSTALL_VERSION is not set"
[ -n "${ASDF_INSTALL_PATH:-}" ]    || asdf_qmkonnect_fail "ASDF_INSTALL_PATH is not set"
[ -n "${ASDF_DOWNLOAD_PATH:-}" ]   || asdf_qmkonnect_fail "ASDF_DOWNLOAD_PATH is not set"

version="$ASDF_INSTALL_VERSION"
mkdir -p "$ASDF_INSTALL_PATH/bin"

asdf_qmkonnect_detect_platform
case "$ASDF_QMKONNECT_OS" in
    Linux)
        asset="qmkonnect-${version}-linux-x86_64.tar.gz"
        tarball="$ASDF_DOWNLOAD_PATH/$asset"
        [ -f "$tarball" ] || asdf_qmkonnect_fail "cached artifact not found: $tarball (did bin/download run?)"
        stage="qmkonnect-${version}-linux-x86_64"
        asdf_qmkonnect_info "==> Extracting $asset"
        tar -xzf "$tarball" -C "$ASDF_DOWNLOAD_PATH"
        # Two binaries -> asdf shims them (default list-bin-paths = bin).
        install -m 0755 "$ASDF_DOWNLOAD_PATH/$stage/qmkonnect"        "$ASDF_INSTALL_PATH/bin/qmkonnect"
        install -m 0755 "$ASDF_DOWNLOAD_PATH/$stage/qmkonnect-hid-id" "$ASDF_INSTALL_PATH/bin/qmkonnect-hid-id"
        # udev rule + systemd template need ROOT to install to /usr/lib/... — asdf
        # is per-user, so STAGE them for the one-time manual setup (see README).
        mkdir -p "$ASDF_INSTALL_PATH/share/qmkonnect"
        install -m 0644 "$ASDF_DOWNLOAD_PATH/$stage/udev/69-qmkonnect-rawhid.rules" \
                        "$ASDF_INSTALL_PATH/share/qmkonnect/69-qmkonnect-rawhid.rules"
        install -m 0644 "$ASDF_DOWNLOAD_PATH/$stage/systemd/qmkonnect.service.template" \
                        "$ASDF_INSTALL_PATH/share/qmkonnect/qmkonnect.service.template"
        asdf_qmkonnect_info "==> Installed qmkonnect + qmkonnect-hid-id to $ASDF_INSTALL_PATH/bin"
        asdf_qmkonnect_info "==> System-integration files staged in $ASDF_INSTALL_PATH/share/qmkonnect"
        asdf_qmkonnect_info "    (udev rule + systemd template — run the README's one-time setup)"
        ;;
    Darwin)
        asset="QMKonnect-${version}-macos.dmg"
        dmg="$ASDF_DOWNLOAD_PATH/$asset"
        [ -f "$dmg" ] || asdf_qmkonnect_fail "cached artifact not found: $dmg (did bin/download run?)"
        asdf_qmkonnect_info "==> Mounting $asset"
        mountpoint="$(mktemp -d)"
        # -nobrowse keeps it out of Finder; -mountpoint forces a known path.
        hdiutil attach -nobrowse -mountpoint "$mountpoint" "$dmg" >/dev/null
        app_bin="$mountpoint/QMKonnect.app/Contents/MacOS/qmkonnect"
        if [ ! -f "$app_bin" ]; then
            hdiutil detach "$mountpoint" >/dev/null || true
            asdf_qmkonnect_fail "QMKonnect.app binary not found in the DMG"
        fi
        install -m 0755 "$app_bin" "$ASDF_INSTALL_PATH/bin/qmkonnect"
        hdiutil detach "$mountpoint" >/dev/null
        rmdir "$mountpoint" 2>/dev/null || true
        asdf_qmkonnect_info "==> Installed qmkonnect (raw binary) to $ASDF_INSTALL_PATH/bin"
        asdf_qmkonnect_info "    macOS CAVEAT: this is the raw Mach-O, NOT the full .app bundle."
        asdf_qmkonnect_info "    CLI flags work; the menu-bar tray does NOT (needs the bundle context)."
        asdf_qmkonnect_info "    For the full menu-bar app use the Homebrew cask or the direct DMG."
        ;;
    *_NT-*|MSYS*|MINGW*)
        asdf_qmkonnect_fail "Windows is not supported via the asdf channel: the Windows asset is an Inno SETUP INSTALLER (QMKonnect-${version}-windows-x64.exe), not a portable binary. Use Scoop, Winget, or the Inno installer — see packaging/windows/README.md and packaging/scoop/README.md."
        ;;
    *)
        asdf_qmkonnect_fail "unsupported OS '$ASDF_QMKONNECT_OS'"
        ;;
esac
```

### File 5 — `packaging/asdf/README.md` (reference — author verbatim)

````markdown
# asdf-qmkonnect

[asdf](https://asdf-vm.com/) (and [mise](https://mise.jdx.dev/), which is asdf-compatible) plugin for
[**QMKonnect**](https://github.com/dabstractor/qmkonnect) — the cross-platform window-activity notifier
for QMK keyboards. Installs the pre-built release binaries from
[GitHub Releases](https://github.com/dabstractor/qmkonnect/releases).

> **The same plugin serves both managers.** mise runs an asdf plugin's `bin/*` scripts unchanged, so you
> do not need a separate mise backend.

## Setup

### asdf

```bash
asdf plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect
asdf install qmkonnect latest        # or a specific version, e.g. 0.2.8
asdf global qmkonnect latest         # set the default; or `asdf local` per-project
qmkonnect --help
```

### mise

```bash
mise plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect
mise install qmkonnect@latest
mise use -g qmkonnect@latest
qmkonnect --help
```

## Platform support

| OS / arch | Asset | Status |
|---|---|---|
| **Linux x86_64** | `qmkonnect-<ver>-linux-x86_64.tar.gz` | ✅ **Fully supported** — installs `qmkonnect` + `qmkonnect-hid-id` to the version dir's `bin/`. (Run the one-time udev/systemd setup below.) |
| **macOS** (arm64 / x86_64) | `QMKonnect-<ver>-macos.dmg` (universal2) | ⚠️ **CLI only** — installs the raw binary; the menu-bar tray needs the full `.app` bundle (see the caveat below). |
| Windows | `QMKonnect-<ver>-windows-x64.exe` | ❌ **Not supported via asdf/mise** — the asset is an Inno installer, not a portable binary. Use [Scoop](../scoop/README.md), [Winget](../winget/README.md), or the [Inno installer](../windows/README.md). |

## Linux — one-time system setup (udev + systemd)

asdf installs into a per-user version directory, so it cannot (and should not) run the root commands the
static udev rule and the systemd user service need. After `asdf install qmkonnect <ver>`, run this once
(the plugin stages these files under the version dir's `share/qmkonnect/`):

```bash
# Resolve the installed version dir (adjust if you pin a specific version).
SHARE="$(asdf where qmkonnect)/share/qmkonnect"

# 1. Static usage-page udev rule — grants hidraw perms for the QMK Raw HID
#    signature (usage page 0xFF60 / usage 0x61). Identical for every keyboard.
sudo install -m 644 "$SHARE/69-qmkonnect-rawhid.rules" /usr/lib/udev/rules.d/
sudo install -m 755 "$(asdf where qmkonnect)/bin/qmkonnect-hid-id" /usr/lib/udev/qmkonnect-hid-id
sudo udevadm control --reload-rules
sudo udevadm trigger

# 2. Per-user systemd service (starts at login once a matching device is present).
mkdir -p ~/.config/systemd/user
install -m 644 "$SHARE/qmkonnect.service.template" ~/.config/systemd/user/qmkonnect.service
systemctl --user daemon-reload
systemctl --user enable --now qmkonnect.service
```

Default QMK keyboards then need **no further configuration** — QMKonnect auto-discovers them by the
standard Raw HID usage page. Only disambiguate among multiple boards (or target one that overrode
`RAW_USAGE_PAGE`/`RAW_USAGE_ID` in firmware) with `qmkonnect -c` + `sudo qmkonnect -r`.

## macOS caveat — CLI only, not the menu-bar app

The macOS release is a full `QMKonnect.app` bundle inside a `.dmg`. This plugin mounts the DMG and
copies only the raw Mach-O binary (`QMKonnect.app/Contents/MacOS/qmkonnect`) into the version dir's
`bin/`. That binary runs **CLI flags** (`--help`, `--list-callbacks`, `--reload`, …) but the
**menu-bar tray/icon does not work** — that needs the bundle context (`Info.plist`, resources, template
icon paths).

For the full menu-bar app on macOS, use the [Homebrew cask](../homebrew/README.md) or the
[direct DMG installer](../../packaging/macos/install.sh) instead.

## Versioning

Versions come straight from the [GitHub Releases](https://github.com/dabstractor/qmkonnect/releases) API
at install time (`bin/list-all` scrapes the release tags and strips the leading `v`). There is no
hard-coded version in this plugin — `asdf list all qmkonnect` always reflects the published releases.

## How it works

- `bin/list-all` — queries `https://api.github.com/repos/dabstractor/qmkonnect/releases` and prints the
  versions (ascending; `latest` resolves to the newest).
- `bin/download` — maps `uname -s` / `uname -m` to the release asset, downloads it with `curl`, and
  verifies the SHA256 **if** a `<asset>.sha256` sidecar exists (none is published today; the hash is
  carried by the AUR/Homebrew/Scoop/Winget manifests instead).
- `bin/install` — Linux: `tar`-extracts the tarball; macOS: `hdiutil`-mounts the DMG; Windows: redirects
  to Scoop/Winget/Inno. Installs into `$ASDF_INSTALL_PATH/bin/`.

## License

MIT (QMKonnect is MIT-licensed). This plugin repo is MIT.
````

### Implementation Patterns & Key Details

```bash
# PATTERN (source the shared lib — robust path resolution via BASH_SOURCE):
source "$(dirname "${BASH_SOURCE[0]}")/../lib/utils.bash"
# lib/utils.bash defines functions ONLY; no `set -e` there (sourced — caller owns shell options).

# PATTERN (env-var assertions first, then mkdir, then work):
[ -n "${ASDF_INSTALL_VERSION:-}" ] || asdf_qmkonnect_fail "ASDF_INSTALL_VERSION is not set …"
[ -n "${ASDF_DOWNLOAD_PATH:-}" ]   || asdf_qmkonnect_fail "ASDF_DOWNLOAD_PATH is not set"
mkdir -p "$ASDF_DOWNLOAD_PATH"

# PATTERN (download → install chain): asdf runs bin/download into $ASDF_DOWNLOAD_PATH FIRST, then
#   bin/install reads from there. install must NOT re-download:
tarball="$ASDF_DOWNLOAD_PATH/$asset"
[ -f "$tarball" ] || asdf_qmkonnect_fail "cached artifact not found … (did bin/download run?)"

# PATTERN (optional SHA256 safe under set -e): the curl --head probe is guarded with `|| true`, and the
#   whole verify returns 0 when no sidecar exists (HTTP != 200), so it never aborts the download.

# PATTERN (portable ascending version sort — NOT sort -V): newest is LAST so `latest` resolves without
#   a bin/latest-stable callback (which is P1.M4.T1.S2's job):
… | sed -E 's/^v//' | grep -E '^[0-9]' | sort -t. -k1,1n -k2,2n -k3,3n -k4,4n | tr '\n' ' '

# PATTERN (macOS DMG → raw binary, CLI-only): -nobrowse keeps it out of Finder; -mountpoint pins the path.
mountpoint="$(mktemp -d)"
hdiutil attach -nobrowse -mountpoint "$mountpoint" "$dmg" >/dev/null
install -m 0755 "$mountpoint/QMKonnect.app/Contents/MacOS/qmkonnect" "$ASDF_INSTALL_PATH/bin/qmkonnect"
hdiutil detach "$mountpoint" >/dev/null
```

### Integration Points

```yaml
ASDF CONTRACT:
  - env vars consumed: ASDF_INSTALL_TYPE (unused-but-present), ASDF_INSTALL_VERSION,
    ASDF_INSTALL_PATH, ASDF_DOWNLOAD_PATH
  - chain: bin/download (into ASDF_DOWNLOAD_PATH) -> bin/install (reads ASDF_DOWNLOAD_PATH, writes ASDF_INSTALL_PATH/bin)
  - executables land in ASDF_INSTALL_PATH/bin (default list-bin-paths => bin) so asdf shims them
GITHUB:
  - releases API: https://api.github.com/repos/dabstractor/qmkonnect/releases (unauth, 60 req/hr/IP — fine for a version manager)
  - download URL: https://github.com/dabstractor/qmkonnect/releases/download/v<version>/<asset>
EXECUTABLES PRODUCED (Linux): $ASDF_INSTALL_PATH/bin/qmkonnect + $ASDF_INSTALL_PATH/bin/qmkonnect-hid-id
STAGED-INTEGRATION-FILES (Linux): $ASDF_INSTALL_PATH/share/qmkonnect/{69-qmkonnect-rawhid.rules,qmkonnect.service.template}
DEPENDENCIES (runtime): curl, tar, grep, sed, sort, install (Linux+macOS) + hdiutil (macOS only). NO jq. NO brew.
PARALLEL / SIBLING (no conflicts):
  - P1.M3.T2S2 (parallel, in-flight): edits packaging/winget/* ONLY -> ZERO overlap with packaging/asdf/*.
  - P1.M4.T1.S2 (downstream): adds plugin-repo metadata + mise native backend stub (owns bin/latest-stable etc.).
  - P1.M5.T2.S2 (downstream): `asdf plugin test` + push packaging/asdf/ to the asdf-qmkonnect repo on tag.
  - P1.M6.T1.S1 (downstream): docs/installation.md mise/asdf row.
PLATFORM VALIDATION:
  - Linux dev box: bash -n + shellcheck + LIVE list-all/download/install against the real 0.2.8 release.
  - macOS DMG path + full mise run: validated by review + on a macOS host (deferred).
```

## Validation Loop

> The implementing agent runs on a **Linux dev box** — the PRIMARY asdf target. list-all, download, and
> install (Linux arm) are exercised LIVE against the real `v0.2.8` release. The macOS DMG path + Windows
> redirect are validated by code review (the DMG `hdiutil` logic is standard).

### Level 1: Syntax & Style (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
chmod +x packaging/asdf/bin/list-all packaging/asdf/bin/download packaging/asdf/bin/install
bash -n packaging/asdf/lib/utils.bash packaging/asdf/bin/list-all packaging/asdf/bin/download packaging/asdf/bin/install
# Expected: no output (all parse). If a syntax error → fix before proceeding.
shellcheck packaging/asdf/lib/utils.bash packaging/asdf/bin/list-all packaging/asdf/bin/download packaging/asdf/bin/install 2>/dev/null \
  || echo "(shellcheck not installed — skipped; the bash -n parse above is the gate)"
# Expected: clean (or "not installed"). Address any SC warnings.
git diff --stat    # Expected: exactly 5 NEW files under packaging/asdf/.
```

### Level 2: Live Linux end-to-end (runs on Linux — the headline gate)
```bash
cd /home/dustin/projects/qmkonnect
# (a) list-all against the REAL GitHub API (needs network):
ASDF_PLUGIN_PATH="$PWD/packaging/asdf" bash packaging/asdf/bin/list-all
# Expected: a space-separated list ending in "0.2.8" (0.2.8 is the newest published release) e.g.
#   "0.2.7 0.2.8" or just "0.2.8". If "0.2.8" is NOT last → the sort is wrong (must be ascending).

# (b) download the REAL 0.2.8 Linux tarball:
DL=$(mktemp -d)
ASDF_INSTALL_VERSION=0.2.8 ASDF_DOWNLOAD_PATH="$DL" bash packaging/asdf/bin/download
ls -la "$DL"                       # Expected: qmkonnect-0.2.8-linux-x86_64.tar.gz present
sha256sum "$DL/qmkonnect-0.2.8-linux-x86_64.tar.gz"
# Expected: 86dcaa57254f4b7dd23595ef8e473432c3997df3b6c12621913d215b9ab1b216
tar tzf "$DL/qmkonnect-0.2.8-linux-x86_64.tar.gz"
# Expected (the 4 staged files, under the top-level qmkonnect-0.2.8-linux-x86_64/ dir):
#   qmkonnect-0.2.8-linux-x86_64/qmkonnect
#   qmkonnect-0.2.8-linux-x86_64/qmkonnect-hid-id
#   qmkonnect-0.2.8-linux-x86_64/udev/69-qmkonnect-rawhid.rules
#   qmkonnect-0.2.8-linux-x86_64/systemd/qmkonnect.service.template

# (c) install the REAL tarball into a fake ASDF_INSTALL_PATH:
INS=$(mktemp -d)
ASDF_INSTALL_VERSION=0.2.8 ASDF_INSTALL_PATH="$INS" ASDF_DOWNLOAD_PATH="$DL" bash packaging/asdf/bin/install
ls -la "$INS/bin"                  # Expected: qmkonnect + qmkonnect-hid-id, both -rwxr-xr-x
"$INS/bin/qmkonnect" --help >/dev/null; echo "exit=$?"
# Expected: prints usage and exit=0 (the raw binary runs on Linux).
ls -la "$INS/share/qmkonnect"      # Expected: 69-qmkonnect-rawhid.rules + qmkonnect.service.template
rm -rf "$DL" "$INS"
# If any step fails → read the stderr (asdf-qmkonnect: …) and fix; the most likely failures are a wrong
#   asset name, a wrong tarball-internal path, or a set -e trip in the SHA-probe guard.
```

### Level 3: grep invariants (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
grep -c 'asdf_qmkonnect_' packaging/asdf/lib/utils.bash        # Expected: >= 8 (the helper functions)
grep -n 'ASDF_INSTALL_PATH/bin' packaging/asdf/bin/install      # Expected: >=2 (qmkonnect + qmkonnect-hid-id)
grep -n 'ASDF_DOWNLOAD_PATH' packaging/asdf/bin/install         # Expected: present (reads the cached artifact — does NOT re-download)
grep -n 'hdiutil' packaging/asdf/bin/install                    # Expected: present (macOS arm)
grep -niE 'qmkonnet[^c]|plugin add qmkonnet[^c]' packaging/asdf/  # Expected: ZERO matches (no "qmkonnet" typo)
grep -n 'sort -V' packaging/asdf/lib/utils.bash                 # Expected: ZERO (portable sort -t. only)
grep -n 'jq' packaging/asdf/lib/utils.bash                      # Expected: ZERO (no jq dependency)
grep -ni 'sudo\|udevadm\|systemctl --global' packaging/asdf/bin/install   # Expected: ZERO (install never runs root cmds)
```

### Level 4: macOS DMG path + Windows redirect (code-review only on Linux)
```bash
# These arms are NOT runnable on Linux. Validate by reading bin/install:
#   - Darwin arm: hdiutil attach -nobrowse -mountpoint <tmp>; cp QMKonnect.app/Contents/MacOS/qmkonnect;
#     hdiutil detach. CLI-only caveat printed. (Standard macOS idiom; verified against Homebrew cask's
#     'app QMKonnect.app' bundle layout.)
#   - *_NT-*/MSYS*/MINGW* arm: asdf_qmkonnect_fail with a Scoop/Winget/Inno redirect message.
# On a macOS host (deferred): ASDF_INSTALL_VERSION=0.2.8 + a downloaded DMG -> bin/install copies the
#   raw binary; `qmkonnect --help` runs; the menu-bar tray does NOT (caveat).
```

## Final Validation Checklist

### Technical Validation
- [ ] `bash -n` passes on all 4 scripts; `shellcheck` (if installed) clean.
- [ ] Live Linux: list-all prints `… 0.2.8` (0.2.8 last); download fetches the 0.2.8 tarball (sha256 `86dcaa57…b216`); install produces `$ASDF_INSTALL_PATH/bin/{qmkonnect,qmkonnect-hid-id}` + stages the udev/systemd files.
- [ ] `git diff --stat` = exactly the 5 new files under `packaging/asdf/`.

### Feature Validation
- [ ] `bin/list-all` prints bare ascending versions (newest last) from the GitHub API (jq-free).
- [ ] `bin/download` maps `uname -s`/`uname -m` → the right asset, curls into `$ASDF_DOWNLOAD_PATH`, verifies SHA256 only if a sidecar probes 200 (guarded under `set -e`).
- [ ] `bin/install`: Linux tar-extract + both binaries to `$ASDF_INSTALL_PATH/bin` + udev/systemd staged under `$ASDF_INSTALL_PATH/share/qmkonnect`; macOS DMG→raw-binary + CLI-only caveat; Windows refuse+redirect.
- [ ] README has asdf setup, mise setup, platform matrix, macOS caveat, Linux one-time setup, Windows redirect.

### Code Quality Validation
- [ ] Portable bash (`#!/usr/bin/env bash`, `set -euo pipefail` in the 3 bin/ scripts; no `set -e` in sourced utils.bash).
- [ ] No jq dependency; no `sort -V` (portable `sort -t.`); no "qmkonnet" typo; no root commands in install.
- [ ] Comments explain the WHY (per-user vs root; CLI-only macOS; installer-not-portable Windows; download→install chain).

### Documentation & Deployment
- [ ] README is self-contained (setup + caveats + how-it-works); points to Homebrew/DMG (macOS tray) and Scoop/Winget/Inno (Windows).
- [ ] No Rust/Cargo/.github/other-packaging changes; no PRD/tasks.json/prd_snapshot edits.

---

## Anti-Patterns to Avoid
- ❌ Don't auto-run `udevadm`/`systemctl --global`/`sudo` from `bin/install` — asdf is per-user; the README documents the one-time root setup.
- ❌ Don't invent a `.sha256` sidecar URL — none is published; verify-if-200-else-skip is the correct guard.
- ❌ Don't use `sort -V` (BSD-sort portability) or require `jq` — use `sort -t. -k1,1n …` and grep/sed.
- ❌ Don't re-download in `bin/install` — read the cached artifact from `$ASDF_DOWNLOAD_PATH` (asdf ran `bin/download` first).
- ❌ Don't add `bin/latest-stable`, a `mise.toml` backend, or plugin-repo metadata — that's P1.M4.T1.S2.
- ❌ Don't use the "qmkonnet" typo — the tool name is **`qmkonnect`**.
- ❌ Don't `set -e` in `lib/utils.bash` (it's sourced; the caller owns shell options).
- ❌ Don't pretend Windows is a portable-binary target — the `.exe` is an Inno installer; refuse + redirect.
- ❌ Don't omit the macOS CLI-only caveat (contract requirement) — the raw binary lacks the `.app` bundle context for the tray.
- ❌ Don't edit `.github/workflows/*` (CI = P1.M5.T2.S2) or `docs/installation.md` (P1.M6.T1.S1).
- ❌ Don't edit PRD.md, any tasks.json, or prd_snapshot.md.

---

## Confidence Score: 8/10

The task is well-bounded (5 new files, no Rust/CI changes), the asdf contract is verified against the
official plugin docs + asdf source (env vars, download→install chain, `bin` default), and every ground-
truth fact (org `dabstractor`, asset names, the Linux tarball's 4-file layout, the real 0.2.8 sha256,
no-sha256-sidecar, two binaries) is confirmed from in-repo source (release.yml, AUR PKGBUILD, Scoop
manifest, Homebrew cask, qmkonnect.install). The 4 scripts + README are given as verbatim reference
implementations, so one-pass authoring risk is low. The headline gate — a LIVE Linux list-all/download/
install against the real `v0.2.8` release (incl. the sha256 `86dcaa57…b216` and the 4-file tarball
listing) — runs on the implementing agent's Linux box, catching asset-name/path/sort/SHA-probe errors
deterministically. The 2-point reservation is the **macOS DMG path** (not runnable on Linux — validated
by review; the `hdiutil` mount/copy/detach idiom is standard and the bundle layout is confirmed by the
Homebrew cask's `app "QMKonnect.app"`) and the **mise native run** (mise auto-runs asdf plugins, but a
real `mise install qmkonnect@latest` is deferred to a host with mise installed — the scripts are
identical, so the risk is mise-version-detection quirks, not the script logic). The Windows arm is
trivially correct (refuse + redirect). The `latest`-resolution choice (ascending sort, no `latest-stable`
callback) is deliberately deferred-correct: if P1.M4.T1.S2 adds `bin/latest-stable`, it supersedes
list-all ordering cleanly with no conflict.
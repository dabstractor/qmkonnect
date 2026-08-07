# PRP — P1.M1.T1.S1: Create AUR-adapted PKGBUILD for binary releases

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). New files under
> `packaging/linux/aur/`. **No Rust/source change** — this is a packaging task.
> **Scope:** the AUR `-bin` PKGBUILD (downloads the pre-built GitHub release
> tarball instead of building from source), a copied `qmkonnect.install`, a
> hygiene `.gitignore`, and a `README.md`. `.SRCINFO` + AUR-repo publication are
> the **next sibling** (P1.M1.T1.S2) — out of scope here (but the PKGBUILD must
> be `.SRCINFO`-generatable, which is a verified gate below).

---

## Goal

**Feature Goal**: Create `packaging/linux/aur/PKGBUILD` — an AUR **binary** package
(`pkgname=qmkonnect-bin`) that downloads the CI-staged release tarball
`qmkonnect-${pkgver}-linux-x86_64.tar.gz` and installs its pre-built contents to the
**same four paths** the source PKGBUILD (`packaging/linux/arch/PKGBUILD`) uses — so
Arch users can `yay -S qmkonnect-bin` without a Rust toolchain, while getting the
identical on-disk layout (binary, udev helper, static udev rule, systemd template)
and the identical pacman hooks (`qmkonnect.install`).

**Deliverable**:
- `packaging/linux/aur/PKGBUILD` — the `-bin` PKGBUILD (no `build()`; `source=()` =
  GitHub release URL; real `sha256sums`; mirrors depends/install/backup/options).
- `packaging/linux/aur/qmkonnect.install` — **copy** of `packaging/linux/arch/qmkonnect.install`
  (AUR repos are flat; a symlink would not survive publication).
- `packaging/linux/aur/.gitignore` — ignore makepkg artifacts (`pkg/`, `src/`, `*.tar.gz`, `*.pkg.tar.*`).
- `packaging/linux/aur/README.md` — Mode-A doc: what `qmkonnect-bin` is, the `-bin`
  convention, install instructions, relationship to the source PKGBUILD.

**Success Definition**: `cd packaging/linux/aur && makepkg --printsrcinfo` produces a
valid `.SRCINFO`-shaped dump (proves makepkg parses the PKGBUILD); `makepkg -g` prints
`sha256sums=('86dcaa57254f4b7dd23595ef8e473432c3997df3b6c12621913d215b9ab1b216')`
matching the real v0.2.8 tarball; the four install paths in `package()` are identical
to the source PKGBUILD's; `bash -n PKGBUILD` is clean; the copied `qmkonnect.install`
is byte-identical to the arch/ original.

## Verified facts (run during research — the basis for the exact values below)

| Fact | Value | Source |
|---|---|---|
| Current version | `0.2.8` | `Cargo.toml` `version`; latest git tag `v0.2.8` |
| Source PKGBUILD `depends` | `('systemd' 'hidapi' 'libusb' 'zenity' 'libnotify')` | `packaging/linux/arch/PKGBUILD` |
| Source PKGBUILD `makedepends` | `('cargo' 'rust' 'libx11' 'libxcb' 'systemd-libs' 'pkg-config')` — **DROP** for `-bin` | same |
| `backup` / `options` / `install` | `("usr/lib/systemd/user/qmkonnect.service.template")` / `(!strip)` / `qmkonnect.install` | same |
| Tarball URL pattern | `https://github.com/dabstractor/qmkonnect/releases/download/v${pkgver}/qmkonnect-${pkgver}-linux-x86_64.tar.gz` | contract + release.yml |
| **Tarball sha256 (v0.2.8)** | **`86dcaa57254f4b7dd23595ef8e473432c3997df3b6c12621913d215b9ab1b216`** | **downloaded + `sha256sum` this session** |
| Tarball internal layout | top dir `qmkonnect-${ver}-linux-x86_64/` containing `qmkonnect`, `qmkonnect-hid-id`, `udev/69-qmkonnect-rawhid.rules`, `systemd/qmkonnect.service.template` | `.github/workflows/release.yml` `linux-binary` job |
| makepkg available? | YES (`/usr/bin/makepkg`) → `makepkg --printsrcinfo` / `makepkg -g` are real gates | `command -v makepkg` |
| namcap available? | NO → document as optional | `command -v namcap` |

## User Persona (if applicable)

**Target User**: An Arch / Arch-derivative user who wants QMKonnect without
installing a Rust toolchain + build deps just to run it.

**Use Case**: `yay -S qmkonnect-bin` (or `paru -S qmkonnect-bin`, or a manual
`makepkg -si` in a clone of the AUR repo) → gets the binary + udev rule + helper +
systemd template + pacman hooks, identical to the source-built package.

**User Journey**: user installs → pacman runs `post_install` (instantiates the
service template, reloads udev, enables the user service globally) → default QMK
keyboard works with zero config.

**Pain Points Addressed**: Removes the "I have to compile Rust on Arch to use this"
friction. The source PKGBUILD is retained (F8/F15) for users who prefer building
from source / need `-git`; the `-bin` package is the low-friction path.

## Why

- **F15 (community package-manager distribution).** The PRD feature table (F15) calls
  for publishing every release to AUR (among others) so users install via their native
  package manager. `external_deps.md` §1 specifies the AUR channel as a **`-bin`**
  package whose `source=()` points at GitHub release artifacts with `$pkgver` and
  whose `sha256sums` is populated (or SKIP). This subtask delivers exactly that.
- **It complements, not replaces, the source PKGBUILD.** `packaging/linux/arch/PKGBUILD`
  (build-from-source) stays for users who want it; `packaging/linux/aur/PKGBUILD`
  (`-bin`) is the published AUR artifact. Same install paths, same `qmkonnect.install`
  hooks → identical post-install behavior.
- **The CI already stages the exact tarball this package consumes** (`release.yml`
  `linux-binary` job), so no new release artifact is needed — the PKGBUILD just
  points at it.

## What

### (a) `packaging/linux/aur/PKGBUILD` (full target content)

```bash
# Maintainer: Mulletware
#
# AUR BINARY package for QMKonnect. Downloads the pre-built GitHub release
# tarball (staged by the CI `linux-binary` job in .github/workflows/release.yml)
# — no Rust toolchain or build deps required.
#
# This is the -bin sibling of packaging/linux/arch/PKGBUILD (which builds from
# source via `cargo build --release` + `-lhidapi-hidraw`). Both install to the
# SAME four paths and reuse the SAME pacman hooks (qmkonnect.install); pick -bin
# for speed, the source PKGBUILD for a from-source / -git workflow.
#
# Published to the AUR as qmkonnect-bin. Generate .SRCINFO before publishing:
#   makepkg --printsrcinfo > .SRCINFO        (P1.M1.T1.S2 — publication infra)

pkgname=qmkonnect-bin
pkgver=0.2.8
pkgrel=1
pkgdesc="A notification daemon for QMK keyboards (pre-built binary release)"
arch=('x86_64')
url="https://github.com/dabstractor/qmkonnect"
license=('MIT')

# Runtime deps identical to the source PKGBUILD. NO makedepends: the release
# tarball ships the pre-built qmkonnect + qmkonnect-hid-id.
depends=('systemd' 'hidapi' 'libusb' 'zenity' 'libnotify')

# Same pacman hooks as the source package (copied into this dir — see README).
install=qmkonnect.install
# Only the (optional, user-instantiated) service template is preserved across
# upgrades; the static udev rule + helper are package-owned.
backup=("usr/lib/systemd/user/qmkonnect.service.template")
options=(!strip)

# The CI linux-binary job stages this tarball with a top-level
# qmkonnect-${pkgver}-linux-x86_64/ dir holding the two binaries, the static
# udev rule, and the systemd template. makepkg extracts it into $srcdir.
source=("https://github.com/dabstractor/qmkonnect/releases/download/v${pkgver}/qmkonnect-${pkgver}-linux-x86_64.tar.gz")
sha256sums=('86dcaa57254f4b7dd23595ef8e473432c3997df3b6c12621913d215b9ab1b216')
# Refresh after a version bump: `updpkgsums` (from pacman-contrib), or
# `makepkg -g` then paste the printed line over the sha256sums above.

package() {
  cd "$srcdir"

  local stage="qmkonnect-${pkgver}-linux-x86_64"

  # Main binary.
  install -Dm755 "${stage}/qmkonnect" "${pkgdir}/usr/bin/qmkonnect"

  # udev helper: tags hidraw interfaces exposing the QMK Raw HID signature
  # (usage page 0xFF60 / usage 0x61) so the static rule can grant permissions
  # uniformly — no per-VID/PID config, no --reload, no sudo for default users.
  install -Dm755 "${stage}/qmkonnect-hid-id" "${pkgdir}/usr/lib/udev/qmkonnect-hid-id"

  # Static usage-page udev rule: IDENTICAL for every keyboard, never regenerated
  # from config. Numbered 69 so it runs before any on-demand 99-qmkonnect.rules
  # the user may generate with `qmkonnect -r`.
  install -Dm644 "${stage}/udev/69-qmkonnect-rawhid.rules" "${pkgdir}/usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules"

  # systemd user service template (post_install instantiates it). Binds to the
  # qmkonnect_device symlink the static rule creates for any matched interface.
  install -Dm644 "${stage}/systemd/qmkonnect.service.template" "${pkgdir}/usr/lib/systemd/user/qmkonnect.service.template"
}
```

> The four `install` destination paths are **byte-identical** to the source
> PKGBUILD's (`/usr/bin/qmkonnect`, `/usr/lib/udev/qmkonnect-hid-id`,
> `/usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules`,
> `/usr/lib/systemd/user/qmkonnect.service.template`). Only the SOURCE differs
> (extracted tarball vs `target/release/`).

### (b) `packaging/linux/aur/qmkonnect.install` — COPY of `packaging/linux/arch/qmkonnect.install`

```bash
cp packaging/linux/arch/qmkonnect.install packaging/linux/aur/qmkonnect.install
```

Byte-identical copy. (See Gotchas for why copy, not symlink.)

### (c) `packaging/linux/aur/.gitignore` (hygiene — makepkg validation creates artifacts)

```gitignore
pkg/
src/
*.pkg.tar.*
qmkonnect-*-linux-x86_64.tar.gz
.SRCINFO
```

> `.SRCINFO` is gitignored HERE only if you treat it as a generated artifact; AUR
> convention is to **commit** `.SRCINFO` (it is the package index the AUR site
> parses). Since S2 (publication infra) owns `.SRCINFO`, this `.gitignore` lists it
> so a local `makepkg --printsrcinfo > .SRCINFO` during S1 validation isn't
> accidentally committed by S1. **S2 will remove the `.SRCINFO` line / commit the
> real one.** (If you prefer, omit the `.SRCINFO` line and just don't generate it
> in S1 — `makepkg --printsrcinfo | head` is enough for the S1 gate.)

### (d) `packaging/linux/aur/README.md` (Mode A — content sketch)

A short doc covering:
- **What this is**: the AUR `qmkonnect-bin` binary package; sibling of the source
  `packaging/linux/arch/PKGBUILD`.
- **The `-bin` convention**: AUR suffix for pre-built-binary packages (vs `-git` for
  VCS, no suffix / `-git` for source). `qmkonnect-bin` = binary release;
  `qmkonnect` (source) = build-from-source.
- **Install**:
  ```bash
  yay -S qmkonnect-bin          # or: paru -S qmkonnect-bin
  # manual: git clone https://aur.archlinux.org/qmkonnect-bin.git && cd qmkonnect-bin && makepkg -si
  ```
- **What it installs**: the four paths + the pacman hooks (instantiates the service
  template, reloads udev, enables the user service globally) — same as the source
  package.
- **Version/checksum maintenance**: after a release, bump `pkgver` and refresh the
  sha256 with `updpkgsums` (or `makepkg -g`), regenerate `.SRCINFO` (`makepkg
  --printsrcinfo > .SRCINFO`), then push to the AUR (CI does this — P1.M5.T1.S1).
- **Relationship to source PKGBUILD**: identical on-disk result; `-bin` skips the
  Rust toolchain + `-lhidapi-hidraw` link step.

### Success Criteria

- [ ] `packaging/linux/aur/PKGBUILD` exists with `pkgname=qmkonnect-bin`,
      `pkgver=0.2.8`, `source=()` = the GitHub release URL, `sha256sums` = the real
      v0.2.8 hash, NO `build()`, NO `makedepends`.
- [ ] `depends`/`install`/`backup`/`options`/`arch`/`url`/`license` mirror the source PKGBUILD.
- [ ] `package()` installs the four files to the same paths as the source PKGBUILD,
      sourcing from `$srcdir/qmkonnect-${pkgver}-linux-x86_64/`.
- [ ] `packaging/linux/aur/qmkonnect.install` is a byte-identical copy of arch/'s.
- [ ] `packaging/linux/aur/.gitignore` + `README.md` exist.
- [ ] `makepkg --printsrcinfo` (in aur/) produces a valid dump (pkgbase=qmkonnect-bin …).
- [ ] `makepkg -g` prints the verified sha256 (`86dcaa…`).
- [ ] No file under `src/`, `packaging/linux/arch/`, or the Rust source is modified.

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed
> to implement this successfully?"_ — **Yes.** The full target PKGBUILD content, the
> exact tarball internal layout, the verified sha256, the copy-not-symlink decision,
> the README sketch, and the verified makepkg validation commands are all below.

### Documentation & References

```yaml
# MUST READ — the source PKGBUILD to mirror (depends/install/backup/options/install paths)
- file: /home/dustin/projects/qmkonnect/packaging/linux/arch/PKGBUILD
  why: "The -bin package must install to the SAME four paths with the SAME depends/install/backup/options.
        Copy depends/backup/options/install verbatim; DROP makedepends and build(); replace the source
        (target/release) with the extracted release tarball."
  pattern: "package() uses `install -Dm755`/`-Dm644` to the four /usr paths. Mirror the destinations exactly."
  gotcha: "The source PKGBUILD builds in-place (cd $srcdir/.. + ../../../target/release) because it has no
           source array. The -bin package MUST NOT copy that cd/relative-path pattern — it extracts the
           tarball into $srcdir/qmkonnect-\${pkgver}-linux-x86_64/ and installs from there."

# MUST READ — the pacman hooks file to copy verbatim
- file: /home/dustin/projects/qmkonnect/packaging/linux/arch/qmkonnect.install
  why: "post_install/post_upgrade/post_remove hooks (instantiate service template, reload udev, enable
        globally, cleanup). The -bin package references it via install=qmkonnect.install and MUST ship a
        copy in its own dir (AUR repos are flat — a symlink would not survive publication)."
  pattern: "COPY the file byte-for-byte into packaging/linux/aur/qmkonnect.install. Do not edit it."

# MUST READ — the CI job that stages the tarball (defines the exact internal layout this package extracts)
- file: /home/dustin/projects/qmkonnect/.github/workflows/release.yml
  why: "The linux-binary job mkdirs qmkonnect-\${VER}-linux-x86_64/{udev,systemd}, copies the two binaries
        + the rule + the template in, and tars it. package() paths MUST match this layout
        (\${stage}/qmkonnect, \${stage}/qmkonnect-hid-id, \${stage}/udev/69-qmkonnect-rawhid.rules,
        \${stage}/systemd/qmkonnect.service.template)."
  section: "jobs.linux-binary (Stage binary tarball step)"
  critical: "The top-level dir name embeds the version: qmkonnect-\${pkgver}-linux-x86_64. Use \$pkgver
             (not a hardcoded 0.2.8) in package()'s `stage` variable so version bumps work."

# MUST READ — the AUR channel spec (package type, key files, source/checksum requirements)
- file: /home/dustin/projects/qmkonnect/plan/007_fb356ba503b4/architecture/external_deps.md
  why: "§1 AUR: -bin package; source=() must point to GitHub release artifacts with \$pkgver; sha256sums
        populated or SKIP; .SRCINFO via makepkg --printsrcinfo; publication via git push to
        aur.archlinux.org/qmkonnect-bin.git (S2/CI, not S1)."
  section: "1. AUR (Arch User Repository)"
  critical: "source=() MUST use the URL with \$pkgver (not a literal) so CI can bump pkgver and the URL
             follows. sha256sums may be SKIP, but the real hash is known (86dcaa…) — use it."

# REFERENCE — the PACKAGING spec section that documents the source PKGBUILD (Mode-A doc should align)
- file: /home/dustin/projects/qmkonnect/spec/PACKAGING.md
  why: "§4.1 documents the source Arch PKGBUILD; §4.2 documents qmkonnect.install. The aur/README.md should
        cross-reference these so the -bin vs source distinction is clear. (PACKAGING.md itself is updated
        in the P1.M6 doc-sync milestone, NOT this subtask.)"
  section: "4.1 Arch PKGBUILD", "4.2 qmkonnect.install"

# REFERENCE — research notes for this subtask (verified sha256 + makepkg-gate notes)
- docfile: plan/007_fb356ba503b4/P1M1T1S1/research/notes.md
  why: "Records the verified v0.2.8 tarball sha256, the exact CI tarball layout, the copy-vs-symlink
        decision, and the makepkg --printsrcinfo / makepkg -g gate semantics."
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/
├── Cargo.toml                                  # version = "0.2.8" (the pkgver source)
├── .github/workflows/release.yml               # linux-binary job stages qmkonnect-<ver>-linux-x86_64.tar.gz
└── packaging/linux/
    ├── arch/
    │   ├── PKGBUILD                            # SOURCE PKGBUILD to mirror (depends/install/backup/options/paths)
    │   └── qmkonnect.install                   # pacman hooks to COPY into aur/
    ├── udev/69-qmkonnect-rawhid.rules          # (already inside the release tarball — not referenced directly by aur/)
    ├── systemd/qmkonnect.service.template      # (already inside the release tarball)
    └── aur/                                    # <-- CREATE (does not exist yet)
        ├── PKGBUILD                            # NEW — the -bin package (What a)
        ├── qmkonnect.install                   # NEW — copy of arch/qmkonnect.install (What b)
        ├── .gitignore                          # NEW — makepkg-artifact hygiene (What c)
        └── README.md                           # NEW — Mode-A doc (What d)
```

### Desired Codebase tree with files to be added

```bash
packaging/linux/aur/   # NEW DIR — all four files below are new; no other file touched.
├── PKGBUILD
├── qmkonnect.install   # byte-identical copy of ../arch/qmkonnect.install
├── .gitignore
└── README.md
```

> No change to `packaging/linux/arch/`, `release.yml`, `Cargo.toml`, or any Rust source.
> `.SRCINFO` is NOT created here (P1.M1.T1.S2 owns it).

### Known Gotchas of our codebase & Library Quirks

```bash
# CRITICAL: do NOT copy the source PKGBUILD's cd/relative-path pattern.
#   arch/PKGBUILD builds in-place: `cd "$srcdir/.."` + `../../../target/release/...` (it has no source array;
#   makepkg is run from inside packaging/linux/arch/). The -bin package has a REAL source array, so makepkg
#   extracts the tarball into $srcdir/qmkonnect-$pkgver-linux-x86_64/. package() must `cd "$srcdir"` and
#   install from "${stage}/...". Copying the source PKGBUILD's path logic verbatim = broken install.

# CRITICAL: COPY qmkonnect.install, do NOT symlink it.
#   AUR repos are flat (PKGBUILD + .SRCINFO + referenced files at the repo root). A symlink
#   (aur/qmkonnect.install -> ../arch/qmkonnect.install) would (a) not resolve in the published AUR repo,
#   and (b) break `makepkg --printsrcinfo`/`makepkg -si` for anyone cloning the AUR repo. Copy it. The
#   trade-off (drift if arch/qmkonnect.install changes) is documented in the README; re-sync on hooks edits.

# CRITICAL: source=() URL MUST use $pkgver, not a literal version.
#   `https://github.com/dabstractor/qmkonnect/releases/download/v${pkgver}/qmkonnect-${pkgver}-linux-x86_64.tar.gz`.
#   So CI (P1.M5.T1.S1) can bump pkgver and the URL follows. A literal v0.2.8 would freeze the package.

# CRITICAL: install paths must be byte-identical to the source PKGBUILD.
#   /usr/bin/qmkonnect, /usr/lib/udev/qmkonnect-hid-id, /usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules,
#   /usr/lib/systemd/user/qmkonnect.service.template. The qmkonnect.install hooks hardcode these paths
#   (e.g. /usr/lib/systemd/user/qmkonnect.service.template); diverging breaks post_install.

# NOTE: makepkg --printsrcinfo REQUIRES qmkonnect.install to be present (install= references it).
#   Copy qmkonnect.install into aur/ BEFORE running --printsrcinfo, else makepkg errors
#   "install file ... not found".

# NOTE: the verified sha256 (86dcaa…) is for v0.2.8 ONLY. After a version bump it is INVALID — refresh
#   with `updpkgsums` (pacman-contrib) or `makepkg -g` (paste the printed line). Document this in README.

# NOTE: makepkg refuses to run as root. Run validation as the normal user (dustin). It also creates
#   pkg/, src/, and downloads qmkonnect-<ver>-linux-x86_64.tar.gz into the aur/ dir — hence the .gitignore.

# NOTE: namcap is NOT installed on this host. The makepkg gates (--printsrcinfo, -g) are the validation.
#   If namcap were available, `namcap PKGBUILD` would lint it — document as optional in README, not a gate.
```

## Implementation Blueprint

### Data models and structure

No data models. The "structure" is the PKGBUILD shell format: metadata fields
(pkgname/pkgver/pkgrel/...) + a `source`/`sha256sums` pair + a `package()` function.
`.SRCINFO` (S2) is the machine-readable derivative.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CREATE packaging/linux/aur/ and the PKGBUILD
  - MKDIR: packaging/linux/aur/
  - WRITE: packaging/linux/aur/PKGBUILD with the exact content in What (a).
  - CONFIRM: pkgname=qmkonnect-bin; pkgver=0.2.8; source URL uses ${pkgver};
          sha256sums=('86dcaa57254f4b7dd23595ef8e473432c3997df3b6c12621913d215b9ab1b216');
          NO build(), NO makedepends; depends/install/backup/options mirror arch/PKGBUILD.
  - CONFIRM: package() cds to $srcdir and installs the four files from
          $srcdir/qmkonnect-${pkgver}-linux-x86_64/{.,udev,systemd}/ to the four /usr paths.

Task 2: COPY qmkonnect.install (must precede makepkg validation)
  - RUN: cp packaging/linux/arch/qmkonnect.install packaging/linux/aur/qmkonnect.install
  - VERIFY: diff packaging/linux/arch/qmkonnect.install packaging/linux/aur/qmkonnect.install  (empty = identical)

Task 3: CREATE the .gitignore + README.md
  - WRITE: packaging/linux/aur/.gitignore (What c).
  - WRITE: packaging/linux/aur/README.md (What d sketch).

Task 4: VALIDATE (makepkg gates — makepkg is installed)
  - RUN: bash -n packaging/linux/aur/PKGBUILD                       (syntax)
  - RUN: cd packaging/linux/aur && makepkg --printsrcinfo | head -5 (parses; pkgbase = qmkonnect-bin)
  - RUN: cd packaging/linux/aur && makepkg -g                       (downloads tarball; prints the sha256 — must be 86dcaa…)
  - RUN: cd packaging/linux/aur && git status --short -- .          (only the 4 new files; pkg/src/*.tar.gz ignored)

Task 5: PATH-PARITY CHECK vs the source PKGBUILD
  - RUN: diff <(grep -oE '"\$pkgdir/[^"]+"' packaging/linux/arch/PKGBUILD) \
               <(grep -oE '"\$\{?pkgdir\}?/[^"]+"' packaging/linux/aur/PKGBUILD | sed 's/\${pkgdir}/$pkgdir/g; s/${pkgdir}/$pkgdir/g')
          (the four destination paths must match)
```

### Implementation Patterns & Key Details

```bash
# === THE SOURCE-vs-BIN DELTA (the whole point) ===
# arch/PKGBUILD (source):  makedepends=(cargo rust ...); build() { cargo build --release }; installs from target/release
# aur/PKGBUILD   (-bin):   NO makedepends; NO build(); source=(<github release URL>); installs from extracted $srcdir/<stage>/
# Everything else (depends, install, backup, options, arch, url, license, the four /usr destinations) is IDENTICAL.

# === package() extraction anchor (from the CI tarball layout) ===
# Tarball top dir = qmkonnect-${pkgver}-linux-x86_64/
#   ./qmkonnect
#   ./qmkonnect-hid-id
#   ./udev/69-qmkonnect-rawhid.rules
#   ./systemd/qmkonnect.service.template
# makepkg extracts into $srcdir, so: $srcdir/qmkonnect-${pkgver}-linux-x86_64/qmkonnect  etc.

# === THE VERIFIED sha256 (refresh on version bump) ===
# 86dcaa57254f4b7dd23595ef8e473432c3997df3b6c12621913d215b9ab1b216  (v0.2.8, downloaded + sha256sum'd in research)
# Refresh: updpkgsums  OR  makepkg -g  (then paste over the sha256sums line).
```

### Integration Points

```yaml
SOURCE FILES:
  - create: "packaging/linux/aur/PKGBUILD, packaging/linux/aur/qmkonnect.install (copy),
            packaging/linux/aur/.gitignore, packaging/linux/aur/README.md"
  - modify: "NONE (no Rust source, no Cargo.toml, no release.yml, no arch/PKGBUILD)"

RELEASE ARTIFACT CONSUMED (already produced by CI — no new artifact needed):
  - tarball: "qmkonnect-${pkgver}-linux-x86_64.tar.gz from the GitHub release (release.yml linux-binary job)"
  - layout:  "top dir qmkonnect-\${pkgver}-linux-x86_64/{qmkonnect, qmkonnect-hid-id, udev/69-..., systemd/...}"

DOWNSTREAM CONSUMERS (do NOT implement now — sibling subtasks):
  - P1.M1.T1.S2: "Generate .SRCINFO (makepkg --printsrcinfo > .SRCINFO) + AUR repo publication infra (git push to
                  aur.archlinux.org/qmkonnect-bin.git). S1's .gitignore lists .SRCINFO so S1 validation doesn't
                  commit a throwaway one; S2 owns the committed .SRCINFO."
  - P1.M5.T1.S1: "CI job: on tag, bump pkgver (from cargo metadata), refresh sha256 (updpkgsums/makepkg -g),
                  regenerate .SRCINFO, push PKGBUILD + .SRCINFO + qmkonnect.install to the AUR repo."
  - P1.M6.T2.S2: "Regenerate docs/llms_full.txt + update PACKAGING.md §4 to mention the -bin AUR package."

RELATED (NOT this subtask):
  - P1.M1.T2: "Nix flake (builds from source) — different mechanism, different files."
  - P1.M2/P1.M3: "Homebrew / Scoop / Winget — other F15 channels."
```

## Validation Loop

> All commands run from the repo root: `/home/dustin/projects/qmkonnect`. `makepkg`
> IS installed on this host (`/usr/bin/makepkg`); `namcap` is NOT.

### Level 1: Syntax & makepkg-parse (offline, fast)

```bash
cd /home/dustin/projects/qmkonnect

# (a) Shell syntax.
bash -n packaging/linux/aur/PKGBUILD && echo "syntax ok"
# Expected: "syntax ok". (PKGBUILD is valid bash; makepkg-specific semantics checked next.)

# (b) makepkg parses it (also confirms install=qmkonnect.install resolves). Requires Task 2 done first.
(cd packaging/linux/aur && makepkg --printsrcinfo | head -6)
# Expected:
#   pkgbase = qmkonnect-bin
#   pkgdesc = A notification daemon for QMK keyboards (pre-built binary release)
#   pkgver = 0.2.8
#   pkgrel = 1
#   url = https://github.com/dabstractor/qmkonnect
#   install = qmkonnect.install
# (No "install file not found" error => the qmkonnect.install copy is in place. If it errors, run Task 2.)
```

### Level 2: Checksum verification (downloads the real tarball)

```bash
cd /home/dustin/projects/qmkonnect/packaging/linux/aur

# makepkg -g downloads the source and prints the sha256sums line — it MUST match the value in the PKGBUILD.
makepkg -g
# Expected output (one line):
#   sha256sums=('86dcaa57254f4b7dd23595ef8e473432c3997df3b6c12621913d215b9ab1b216')
# If the printed hash DIFFERS from the PKGBUILD's, the tarball changed — update the PKGBUILD (and re-verify
# against the release notes). If makepkg -g FAILS to download, the release URL/pkgver is wrong.
```

### Level 3: Install-path parity vs the source PKGBUILD

```bash
cd /home/dustin/projects/qmkonnect

# The four destination paths in aur/PKGBUILD must equal arch/PKGBUILD's (the qmkonnect.install hooks depend on them).
for p in \
  'usr/bin/qmkonnect' \
  'usr/lib/udev/qmkonnect-hid-id' \
  'usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules' \
  'usr/lib/systemd/user/qmkonnect.service.template'; do
  grep -q "$p" packaging/linux/arch/PKGBUILD || { echo "MISSING in arch: $p"; exit 1; }
  grep -q "$p" packaging/linux/aur/PKGBUILD   || { echo "MISSING in aur: $p";  exit 1; }
done && echo "all four install paths present in BOTH pkgbuilds"
```

### Level 4: Repo hygiene (no stray artifacts committed; no source files touched)

```bash
cd /home/dustin/projects/qmkonnect

# (a) The aur/ git status shows ONLY the four new files (pkg/src/tarball ignored).
git status --short -- packaging/linux/aur/
# Expected: 4 new files (PKGBUILD, qmkonnect.install, .gitignore, README.md). NO pkg/, src/, *.tar.gz, *.pkg.tar.*.

# (b) No source/release/arch file was modified.
git status --short -- Cargo.toml .github/ src/ packaging/linux/arch/ packaging/linux/udev/ packaging/linux/systemd/
# Expected: empty.

# (c) The copied .install is byte-identical.
diff packaging/linux/arch/qmkonnect.install packaging/linux/aur/qmkonnect.install && echo "install: identical"
# Expected: "install: identical" (empty diff).

# OPTIONAL (if namcap is installed — it is NOT on this host, so expect "command not found"):
#   namcap packaging/linux/aur/PKGBUILD        # lint; document result if available, else skip.
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1 (a): `bash -n packaging/linux/aur/PKGBUILD` → syntax ok.
- [ ] Level 1 (b): `makepkg --printsrcinfo | head` → pkgbase = qmkonnect-bin, install = qmkonnect.install.
- [ ] Level 2: `makepkg -g` → `sha256sums=('86dcaa…b216')` matches the PKGBUILD.
- [ ] Level 3: all four install paths present in both arch/ and aur/ PKGBUILDs.
- [ ] Level 4 (a): `git status -- packaging/linux/aur/` → only the 4 new files.
- [ ] Level 4 (b): no Cargo.toml/release.yml/src/arch change.
- [ ] Level 4 (c): `diff arch/qmkonnect.install aur/qmkonnect.install` → identical.

### Feature Validation

- [ ] `pkgname=qmkonnect-bin`, `pkgver=0.2.8`, `source=()` uses `${pkgver}`, real sha256.
- [ ] NO `build()`, NO `makedepends` (binary package).
- [ ] `depends`/`install`/`backup`/`options`/`arch`/`url`/`license` mirror the source PKGBUILD.
- [ ] `package()` extracts from `$srcdir/qmkonnect-${pkgver}-linux-x86_64/` (NOT the source PKGBUILD's cd/relative path).
- [ ] README documents `-bin` convention + `yay -S qmkonnect-bin` + checksum-refresh workflow.

### Code Quality Validation

- [ ] PKGBUILD follows the existing arch/PKGBUILD comment/style conventions.
- [ ] qmkonnect.install is a verbatim copy (single source of truth documented in README).
- [ ] `.gitignore` prevents committing makepkg artifacts.
- [ ] The four install destinations are byte-identical to the source package (hook compatibility).

### Documentation & Deployment

- [ ] Mode A: `packaging/linux/aur/README.md` rides with the work (what it is, `-bin` convention, install, maintenance).
- [ ] PACKAGING.md §4 itself is NOT edited here (the doc-sync milestone P1.M6.T2.S2 adds the -bin mention).
- [ ] No user-facing Rust/CLI/config change.

---

## Anti-Patterns to Avoid

- ❌ Don't copy the source PKGBUILD's `cd "$srcdir/.."` + `../../../target/release/...` path logic — it builds in-place
  with no source array. The `-bin` package has a real `source=()`; makepkg extracts into `$srcdir/<stage>/`.
- ❌ Don't symlink `qmkonnect.install` from `../arch/` — AUR repos are flat and a symlink won't survive publication
  (nor resolve for anyone cloning the AUR repo). Copy it byte-for-byte.
- ❌ Don't keep `makedepends` or a `build()` — this is a binary package; the whole point is no toolchain/build.
- ❌ Don't hardcode the version in the `source` URL — use `${pkgver}` so CI version bumps flow through.
- ❌ Don't change the four install destination paths — `qmkonnect.install`'s hooks hardcode them; diverging breaks
  `post_install`. Mirror the source PKGBUILD's destinations exactly.
- ❌ Don't freeze the sha256 as "magic" — it is v0.2.8-specific; document the `updpkgsums`/`makepkg -g` refresh
  workflow in the README (and CI will refresh it on each release).
- ❌ Don't generate or commit `.SRCINFO` here — that is P1.M1.T1.S2 (publication infra). The `.gitignore` lists it so
  a local `--printsrcinfo > .SRCINFO` during S1 validation isn't accidentally committed.
- ❌ Don't run `makepkg --printsrcinfo` before copying `qmkonnect.install` — it errors "install file not found".
- ❌ Don't run makepkg as root (it refuses); run as the normal user.
- ❌ Don't edit `release.yml`, `Cargo.toml`, `packaging/linux/arch/PKGBUILD`, or any Rust source — S1 is purely additive
  under `packaging/linux/aur/`.

---

**Confidence Score: 9/10** for one-pass implementation success. The full target
PKGBUILD is given verbatim; the tarball internal layout is quoted from the CI job;
the sha256 is **verified by download** (not a placeholder); `makepkg` is installed so
`--printsrcinfo` and `-g` are real, executable gates; the four install paths are
fixed by the `qmkonnect.install` hooks; and the copy-not-symlink + URL-uses-`$pkgver`
decisions are explained. The one residual variable is the version-bump maintenance
workflow (a future release invalidates the sha256) — mitigated by the documented
`updpkgsums`/`makepkg -g` refresh and the CI publication job (P1.M5.T1.S1).
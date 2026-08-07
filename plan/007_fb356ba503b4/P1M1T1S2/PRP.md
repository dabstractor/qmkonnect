# PRP — P1.M1.T1.S2: Create `.SRCINFO` + AUR repo publication infrastructure

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). New files under
> `packaging/linux/aur/`. **No Rust/source change** — a packaging/publication task.
> **Scope:** generate the AUR `.SRCINFO` from S1's `-bin` PKGBUILD, create the
> `publish.sh` publication script, remove `.SRCINFO` from S1's `.gitignore` (so the
> real one is committed), and add a Manual-AUR-Publication section to the README.
> Consumes S1's `packaging/linux/aur/{PKGBUILD,qmkonnect.install,.gitignore,README.md}`.

---

## Goal

**Feature Goal**: Deliver the AUR publication infrastructure for `qmkonnect-bin`: the
committed `.SRCINFO` (AUR package index, generated from the PKGBUILD) and a `publish.sh`
that, given a version, bumps `pkgver`, refreshes the sha256, regenerates `.SRCINFO`, and
pushes `PKGBUILD + .SRCINFO + qmkonnect.install` to the AUR git remote
(`aur@aur.archlinux.org:qmkonnect-bin.git`). Plus Mode-A README instructions for manual
publication. This makes the package ready for CI integration in P1.M5.T1.S1.

**Deliverable** (4 changes, all under `packaging/linux/aur/`):
- `.SRCINFO` — generated via `makepkg --printsrcinfo > .SRCINFO` (verified output below).
- `publish.sh` — the publication script (`--dry-run` + `<version>`; pkgver patch → sha256
  refresh → `.SRCINFO` regen → temp-clone AUR push).
- `.gitignore` — **remove** the `.SRCINFO` line S1 added (the real `.SRCINFO` must be committed).
- `README.md` — **append** a "Manual AUR publication" section (publish.sh usage + SSH-key prereq).

**Success Definition**: `diff <(makepkg --printsrcinfo) .SRCINFO` is empty (the committed
`.SRCINFO` matches the PKGBUILD); `bash -n publish.sh` exits 0; `publish.sh` is executable;
`./publish.sh --dry-run 0.2.8` regenerates `.SRCINFO` and exits 0 (skipping the SSH push);
`.gitignore` no longer lists `.SRCINFO`; `git status` shows only the 4 intended changes
(no makepkg artifacts); no Rust/source/CI/arch file is modified.

## User Persona (if applicable)

**Target User**: The maintainer publishing a new `qmkonnect-bin` release to the AUR (manually
today; via CI in P1.M5.T1.S1), and Arch end users who consume the AUR package index.

**Use Case**: A release ships → maintainer runs `./publish.sh 0.2.9` → the script bumps
pkgver, refreshes the sha256 against the new tarball, regenerates `.SRCINFO`, and pushes the
flat trio to the AUR → `yay -S qmkonnect-bin` users see the new version within minutes.

**User Journey**: maintainer tags+publishes the GitHub release → runs `publish.sh <ver>` →
the AUR repo updates → end users' AUR helpers pick up the new `.SRCINFO` → `yay` upgrades.

**Pain Points Addressed**: Removes the error-prone manual sequence (edit pkgver, recompute
sha256, regenerate .SRCINFO, assemble a flat repo, commit, push) into one idempotent script;
makes `.SRCINFO` a committed, PKGBUILD-synced artifact (the AUR site depends on it).

## Why

- **F15 (community package-manager distribution).** external_deps.md §1 specifies the AUR
  channel requires a committed `.SRCINFO` (generated via `makepkg --printsrcinfo`) and
  publication via `git push` to `aur.archlinux.org/qmkonnect-bin.git`. S1 built the PKGBUILD;
  S2 builds the publication path. Together they complete the AUR channel.
- **`.SRCINFO` is the AUR package index.** The AUR web interface parses it for search results
  and metadata; AUR helpers (`yay`/`paru`) read it to resolve versions + dependencies. It MUST
  be committed (not gitignored) and kept in sync with the PKGBUILD. S1 deliberately gitignored
  a throwaway one during its validation; S2 commits the real one and removes the ignore line.
- **`publish.sh` makes releases reproducible.** A version bump without a sha256 refresh leaves
  a stale hash → AUR users hit a checksum mismatch. The script refreshes the sha256 from the
  actual release tarball every run, so a release can never ship a stale checksum.
- **It unblocks CI.** P1.M5.T1.S1 wraps `publish.sh` in a GitHub Actions job (extract version
  via `cargo metadata | jq`, load the AUR SSH key from a secret, run publish.sh on tag). S2
  delivers the reusable script; S1.M5.T1.S1 wires the trigger + secrets.

## What

### (a) `packaging/linux/aur/.SRCINFO` — generate from the S1 PKGBUILD

Run (from `packaging/linux/aur/`): `makepkg --printsrcinfo > .SRCINFO`.
**Verified output** (run this research session — 19 lines; this IS the target content):

```
pkgbase = qmkonnect-bin
	pkgdesc = A notification daemon for QMK keyboards (pre-built binary release)
	pkgver = 0.2.8
	pkgrel = 1
	url = https://github.com/dabstractor/qmkonnect
	install = qmkonnect.install
	arch = x86_64
	license = MIT
	depends = systemd
	depends = hidapi
	depends = libusb
	depends = zenity
	depends = libnotify
	options = !strip
	backup = usr/lib/systemd/user/qmkonnect.service.template
	source = https://github.com/dabstractor/qmkonnect/releases/download/v0.2.8/qmkonnect-0.2.8-linux-x86_64.tar.gz
	sha256sums = 86dcaa57254f4b7dd23595ef8e473432c3997df3b6c12621913d215b9ab1b216

pkgname = qmkonnect-bin
```

> Line 1 (`pkgbase = …`) and the last line (`pkgname = …`) are at column 0; all metadata
> fields are TAB-indented (makepkg convention). The implementer may either run the generator
> (authoritative — can't drift from the PKGBUILD) or paste these 19 lines and verify with
> `diff <(makepkg --printsrcinfo) .SRCINFO` (must be empty). Running it is preferred.

### (b) `packaging/linux/aur/publish.sh` — the publication script

```bash
#!/usr/bin/env bash
# Publish qmkonnect-bin to the AUR.
#
# Usage:
#   ./publish.sh <version>            # patch pkgver, refresh sha256, regen .SRCINFO, push to AUR
#   ./publish.sh --dry-run <version>  # local steps only; skip the AUR git push
#   ./publish.sh 0.2.8
#
# Steps:
#   1. Patch pkgver=<version> in PKGBUILD.
#   2. Refresh sha256sums (downloads the release tarball via `makepkg -g`).
#   3. Generate .SRCINFO (`makepkg --printsrcinfo > .SRCINFO`).
#   4. Clone the AUR repo (aur:qmkonnect-bin.git) to a temp dir, copy in
#      PKGBUILD + .SRCINFO + qmkonnect.install, commit, and push.
#      (Step 4 is skipped under --dry-run.)
#
# Prerequisites:
#   * makepkg (Arch only; part of `pacman`).
#   * An SSH key whose PUBLIC half is registered with the AUR account
#     (https://aur.archlinux.org -> My Account -> SSH Public Key). The AUR supports
#     SSH-key auth ONLY (no token/password). For CI, store the PRIVATE key as a
#     GitHub Actions secret and load it into ssh-agent before running this script.
#   * The GitHub release for <version> must already be published -- step 2 downloads
#     the tarball to compute its sha256.
#   * Do NOT run as root (makepkg refuses root). Run as a normal user.
#
# This script updates packaging/linux/aur/{PKGBUILD,.SRCINFO} in place. Commit those
# to the qmkonnect SOURCE repo too so it stays in sync with the AUR. CI integration
# (cargo-metadata version extraction + secret loading) is wired in P1.M5.T1.S1.
set -euo pipefail

DRY_RUN=0
VERSION=""
for a in "$@"; do
    case "$a" in
        --dry-run|-n) DRY_RUN=1 ;;
        -h|--help)    sed -n '2,31p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *)            VERSION="$a" ;;
    esac
done

if [ -z "$VERSION" ]; then
    echo "Usage: $0 [--dry-run] <version>   (e.g. 0.2.8)" >&2
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

command -v makepkg >/dev/null || { echo "ERROR: makepkg not found (Arch only)." >&2; exit 1; }

AUR_REMOTE="aur@aur.archlinux.org:qmkonnect-bin.git"

echo "==> Publishing qmkonnect-bin v${VERSION}${DRY_RUN:+ (dry-run)}"

# 1. Patch pkgver.
sed -i "s/^pkgver=.*/pkgver=${VERSION}/" PKGBUILD
echo "    pkgver  -> ${VERSION}"

# 2. Refresh sha256sums (downloads the release tarball; the release must exist).
newsums="$(makepkg -g 2>/dev/null)" || {
    echo "ERROR: makepkg -g failed -- is the v${VERSION} GitHub release published?" >&2
    exit 1
}
sed -i "s|^sha256sums=.*|${newsums}|" PKGBUILD
echo "    sha256  -> ${newsums}"

# 3. Generate .SRCINFO (always in sync with the just-patched PKGBUILD).
makepkg --printsrcinfo > .SRCINFO
echo "    .SRCINFO regenerated"

if [ "$DRY_RUN" -eq 1 ]; then
    echo "==> Dry-run: skipping AUR push."
    exit 0
fi

# 4. Publish to the AUR via a temp clone (flat repo: PKGBUILD + .SRCINFO + .install).
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

git clone "$AUR_REMOTE" "$WORK/aur"
cp PKGBUILD .SRCINFO qmkonnect.install "$WORK/aur/"
git -C "$WORK/aur" add PKGBUILD .SRCINFO qmkonnect.install
if git -C "$WORK/aur" diff --cached --quiet; then
    echo "==> AUR already at v${VERSION}; nothing to push."
else
    git -C "$WORK/aur" commit -m "qmkonnect-bin v${VERSION}"
    git -C "$WORK/aur" push
    echo "==> Published qmkonnect-bin v${VERSION} to the AUR."
fi
```

> `chmod +x packaging/linux/aur/publish.sh`. Design notes: `set -euo pipefail` (arg + git
> ops); `--dry-run` enables a no-SSH testable gate; sha256 refresh is MANDATORY on a version
> bump (a stale hash breaks AUR installs); the `git diff --cached --quiet` check makes a
> re-run of the same version a clean no-op rather than a failed commit; the temp-clone +
> `trap` cleanup avoids a stale local AUR checkout.

### (c) `packaging/linux/aur/.gitignore` — REMOVE the `.SRCINFO` line

S1's `.gitignore` (verified present) is:
```gitignore
pkg/
src/
*.pkg.tar.*
qmkonnect-*-linux-x86_64.tar.gz
.SRCINFO
```
Delete the `.SRCINFO` line (the real `.SRCINFO` must be committed). Keep the other four
(they exclude makepkg artifacts: `pkg/`, `src/`, built packages, the downloaded tarball).

### (d) `packaging/linux/aur/README.md` — APPEND a "Manual AUR publication" section

Add a section (after S1's install/maintenance content) covering:
- **One-shot publish**: `./publish.sh <version>` (e.g. `./publish.sh 0.2.8`) — bumps pkgver,
  refreshes the sha256 from the release tarball, regenerates `.SRCINFO`, pushes to the AUR.
- **Dry run**: `./publish.sh --dry-run <version>` — local steps only (no SSH push); use to
  sanity-check the pkgver/sha256/.SRCINFO regeneration.
- **SSH deploy key (prerequisite)**: register the PUBLIC key at
  https://aur.archlinux.org → My Account → SSH Public Key. The AUR supports SSH-key auth
  ONLY. For CI, store the PRIVATE key as a GitHub Actions secret (P1.M5.T1.S1 loads it).
- **Ordering**: publish the GitHub release FIRST (the script downloads the tarball to compute
  its sha256); THEN run `publish.sh`.
- **Source-repo sync**: `publish.sh` updates `packaging/linux/aur/{PKGBUILD,.SRCINFO}` in
  place — commit those to the qmkonnect source repo too so it stays in sync with the AUR.
- **AUR repo URL**: `https://aur.archlinux.org/packages/qmkonnect-bin` (the published package).

### Success Criteria

- [ ] `packaging/linux/aur/.SRCINFO` exists; `diff <(makepkg --printsrcinfo) .SRCINFO` (in aur/) is empty.
- [ ] `.SRCINFO` carries `pkgbase = qmkonnect-bin`, `pkgver = 0.2.8`, `install = qmkonnect.install`,
      the source URL, and the `sha256sums` line.
- [ ] `packaging/linux/aur/publish.sh` exists, `bash -n` exits 0, and is executable (`+x`).
- [ ] `publish.sh` supports `--dry-run`/`-n` + a positional `<version>`; `set -euo pipefail`.
- [ ] `./publish.sh --dry-run 0.2.8` regenerates `.SRCINFO` and exits 0 (no SSH push).
- [ ] `.gitignore` no longer lists `.SRCINFO` (`grep -cE '^\.SRCINFO$' .gitignore` → 0).
- [ ] `README.md` has a Manual-AUR-Publication section (publish.sh usage + SSH-key prereq + ordering).
- [ ] `git status -- packaging/linux/aur/` shows only `.SRCINFO`, `publish.sh`, modified `.gitignore`, modified `README.md` — NO makepkg artifacts.
- [ ] No file outside `packaging/linux/aur/` is modified (no Rust/CI/arch/PKGBUILD change).

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed to
> implement this successfully?"_ — **Yes.** The verified `.SRCINFO` content (run this
> session), the full `publish.sh` script, the exact `.gitignore` edit, the README section
> sketch, the AUR SSH-auth model, and the verified `makepkg`-based gates are all below.

### Documentation & References

```yaml
# MUST READ — the sibling PRP whose output S2 consumes (the -bin PKGBUILD CONTRACT)
- file: /home/dustin/projects/qmkonnect/plan/007_fb356ba503b4/P1M1T1S1/PRP.md
  why: "Defines the exact aur/PKGBUILD (pkgname=qmkonnect-bin, pkgver=0.2.8, source=() with
        ${pkgver}, sha256sums=('86dcaa…'), install=qmkonnect.install, no build()/makedepends)
        that S2's .SRCINFO is generated from and that publish.sh patches. Also documents that
        S1's .gitignore lists .SRCINFO 'so S1 validation doesn't accidentally commit a
        throwaway one' and that 'S2 will remove the .SRCINFO line / commit the real one.'"
  section: "What (a) PKGBUILD", "What (c) .gitignore", "Integration Points (S2 owns .SRCINFO)"
  critical: "S2 MUST remove the .SRCINFO line from S1's .gitignore or the committed .SRCINFO
             will be ignored. publish.sh patches the PKGBUILD at runtime (pkgver+sha256) but
             S2 does not hand-edit the PKGBUILD."

# MUST READ — the AUR channel spec (publication model + key files + auth)
- file: /home/dustin/projects/qmkonnect/plan/007_fb356ba503b4/architecture/external_deps.md
  why: "§1 AUR: -bin package; key files PKGBUILD + .SRCINFO + optional .install; publication
        via git push to aur.archlinux.org/qmkonnect-bin.git; .SRCINFO via makepkg --printsrcinfo;
        'AUR git repo needs SSH key or token for CI publishing' (in practice SSH-key ONLY).
        Cross-Cutting: 'CI approach: git clone -> update file -> commit -> push'."
  section: "1. AUR (Arch User Repository)", "Cross-Cutting Concerns / CI Publishing Strategy"
  critical: "The AUR repo is a SEPARATE flat git remote (PKGBUILD + .SRCINFO + .install at the
             root), NOT the qmkonnet source repo. publish.sh clones it to a temp dir per run."

# MUST READ — the file being built alongside (the PKGBUILD .SRCINFO is generated from)
- file: /home/dustin/projects/qmkonnect/packaging/linux/aur/PKGBUILD
  why: "S1's -bin PKGBUILD. `makepkg --printsrcinfo` (run from aur/) reads it and emits .SRCINFO.
        publish.sh sed-patches its `^pkgver=` and `^sha256sums=` lines. Do NOT hand-edit it in S2."
  pattern: "The PKGBUILD has `pkgver=0.2.8` and `sha256sums=('86dcaa…')` at column 0 — the sed
            anchors `^pkgver=.*` and `^sha256sums=.*` match them."
  gotcha: "makepkg --printsrcinfo requires qmkonnect.install present (install= references it).
           S1 already copied it into aur/; confirm it's there before generating .SRCINFO."

# MUST READ — the .gitignore S2 must edit (remove the .SRCINFO line)
- file: /home/dustin/projects/qmkonnect/packaging/linux/aur/.gitignore
  why: "S1 listed `.SRCINFO` to avoid committing a throwaway validation file. S2 commits the
        real .SRCINFO, so it MUST delete that line. Keep pkg/, src/, *.pkg.tar.*, *.tar.gz."
  pattern: "One-line removal: delete the line reading exactly `.SRCINFO`."
  gotcha: "If you forget this, `git add .SRCINFO` silently no-ops (the file is ignored) and the
           committed tree lacks the AUR package index."

# REFERENCE — the CI version-extraction pattern (for the README/CI doc; NOT wired by S2)
- file: /home/dustin/projects/qmkonnect/.github/workflows/release.yml
  why: "Lines 41-46 show the canonical version extraction: `cargo metadata --no-deps
        --format-version 1 | jq -r '.packages[] | select(.name==\"qmkonnect\") | .version'`.
        publish.sh takes <version> as an arg; CI (P1.M5.T1.S1) derives it this way and passes it."
  section: "jobs.*.steps: 'Determine version'"
  critical: "S2 does NOT edit release.yml. The AUR CI job is P1.M5.T1.S1. publish.sh is the
             reusable unit S1.M5.T1.S1 wraps."

# REFERENCE — AUR publishing conventions (authoritative external doc)
- url: https://wiki.archlinux.org/title/Arch_User_Repository#Rules
  why: "Confirms AUR repos are plain git remotes at ssh://aur@aur.archlinux.org/<pkg>.git
        containing PKGBUILD + .SRCINFO (+ referenced files); .SRCINFO MUST be present+committed;
        SSH-key auth only; submit = git push."
  section: "Rules ( submitting packages, .SRCINFO, SSH )"
  critical: ".SRCINFO is MANDATORY and must be regenerated whenever the PKGBUILD changes. The
             AUR does not generate it for you."

# REFERENCE — makepkg --printsrcinfo (the .SRCINFO generator)
- url: https://wiki.archlinux.org/title/makepkg#Generating_a_new_.SRCINFO
  why: "Documents `makepkg --printsrcinfo > .SRCINFO` as the canonical generator and the .SRCINFO
        format (the package metadata the AUR parses)."
  section: "Generating a new .SRCINFO"

# REFERENCE — research notes for this subtask (verified .SRCINFO + publish.sh design + gates)
- docfile: /home/dustin/projects/qmkonnect/plan/007_fb356ba503b4/P1M1T1S2/research/notes.md
  why: "§2 = the verified 19-line .SRCINFO. §3 = the AUR publication model. §4 = the publish.sh
        design rationale (why refresh sha256; --dry-run; temp-clone). §6 = the verified gates."
  section: "§2 .SRCINFO", "§4 publish.sh design", "§6 validation gates"
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/
├── .github/workflows/release.yml          # version-extraction pattern (lines 41-46); AUR CI job = P1.M5.T1.S1
└── packaging/linux/
    ├── arch/PKGBUILD                      # source PKGBUILD (untouched)
    └── aur/                               # S1 created PKGBUILD + qmkonnect.install + README.md + .gitignore
        ├── PKGBUILD                       # S1 — .SRCINFO is generated FROM this; publish.sh patches it at runtime
        ├── qmkonnect.install              # S1 — copy of arch/; referenced by install= (must be present for makepkg)
        ├── README.md                      # S1 — S2 APPENDS the Manual-AUR-Publication section
        ├── .gitignore                     # S1 — S2 REMOVES the .SRCINFO line
        ├── .SRCINFO                       # <-- S2 CREATES (makepkg --printsrcinfo)
        └── publish.sh                     # <-- S2 CREATES (the publication script)
```

### Desired Codebase tree with files to be added/modified

```bash
packaging/linux/aur/
├── .SRCINFO        # NEW (S2) — generated from PKGBUILD
├── publish.sh      # NEW (S2) — executable; --dry-run + <version>
├── .gitignore      # MODIFIED (S2) — remove the `.SRCINFO` line
└── README.md       # MODIFIED (S2) — append "Manual AUR publication" section
```

> No change to `PKGBUILD`, `qmkonnect.install`, `packaging/linux/arch/`, `release.yml`,
> `Cargo.toml`, or any Rust source.

### Known Gotchas of our codebase & Library Quirks

```bash
# CRITICAL: REMOVE the `.SRCINFO` line from S1's .gitignore, else the commit silently omits it.
#   S1 listed `.SRCINFO` deliberately (throwaway-validation hygiene). S2 commits the real one,
#   so the ignore line MUST go. Gate: `grep -cE '^\.SRCINFO$' .gitignore` -> 0.

# CRITICAL: generate .SRCINFO AFTER confirming qmkonnect.install is present.
#   makepkg --printsrcinfo/errors with "install file not found" if install=qmkonnect.install
#   can't be resolved. S1 already copied it into aur/; just confirm before generating.

# CRITICAL: publish.sh MUST refresh sha256 on a version bump (not just patch pkgver).
#   A bumped pkgver with the old sha256 => stale hash => AUR users hit a checksum mismatch.
#   `makepkg -g` downloads the release tarball and prints the correct sha256sums=(...) line.

# CRITICAL: the AUR repo is a SEPARATE flat remote, NOT the qmkonnet source repo.
#   publish.sh clones aur@aur.archlinux.org:qmkonnect-bin.git to a temp dir and copies in
#   PKGBUILD + .SRCINFO + qmkonnect.install (the AUR root is flat — no subdir). Do NOT push
#   the whole packaging/linux/aur/ dir (it has README.md + .gitignore the AUR doesn't want).

# CRITICAL: AUR auth is SSH-key ONLY (no token/password).
#   Register the public key at https://aur.archlinux.org -> My Account -> SSH Public Key.
#   publish.sh assumes ssh-agent/auth is configured; a missing key fails at `git clone` (set -e).

# CRITICAL: publish.sh must run AFTER the GitHub release is published.
#   `makepkg -g` downloads the tarball to compute the sha256. Run release.yml first, then
#   publish.sh. (Documented in the script header + README.)

# NOTE: makepkg refuses root. Run publish.sh as a normal user; CI must use a non-root step.
# NOTE: makepkg 7.1.0 is installed on this host (/usr/bin/makepkg) -> the gates are real/executable.
# NOTE: namcap is NOT installed; it is not a gate (optional lint only, documented in S1's README).
# NOTE: publish.sh edits the SOURCE repo's aur/PKGBUILD + aur/.SRCINFO in place. The maintainer/CI
#   must ALSO commit those to the qmkonnet source repo (documented); publish.sh does not auto-commit
#   the source repo (no surprise commits). The source-repo CI wiring is P1.M5.T1.S1.
# NOTE: the .SRCINFO format TAB-indents metadata fields under a column-0 `pkgbase = ...` line,
#   with a column-0 `pkgname = ...` closer after a blank line. Do NOT "reformat" it — makepkg's
#   output is the canonical shape the AUR parses.
```

## Implementation Blueprint

### Data models and structure

No data models. The "structure" is: a generated `.SRCINFO` (key=value text, makepkg format) +
a bash publication script + two doc/config edits.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CONFIRM S1's aur/ is in place (the prerequisite)
  - RUN: ls packaging/linux/aur/            → expect PKGBUILD, qmkonnect.install, README.md, .gitignore
  - RUN: cat packaging/linux/aur/.gitignore → expect the 5-line file incl. `.SRCINFO`
  - RUN: command -v makepkg                 → expect /usr/bin/makepkg (Arch host)
  - IF PKGBUILD/qmkonnect.install absent (S1 not landed): STOP — S2 depends on S1.

Task 2: CREATE .SRCINFO (generated, not hand-written)
  - RUN: cd packaging/linux/aur && makepkg --printsrcinfo > .SRCINFO
  - VERIFY: diff <(makepkg --printsrcinfo) .SRCINFO   → empty (in sync with PKGBUILD)
  - VERIFY: grep -E 'pkgbase = qmkonnect-bin|pkgver = 0.2.8|install = qmkonnect.install|sha256sums =' .SRCINFO
          → 4 hits. (Compare to the verified 19-line content in What (a).)

Task 3: CREATE publish.sh (What (b))
  - WRITE: packaging/linux/aur/publish.sh with the exact content in What (b).
  - RUN: chmod +x packaging/linux/aur/publish.sh
  - RUN: bash -n packaging/linux/aur/publish.sh     → exit 0 (syntax)
  - CONFIRM: set -euo pipefail; --dry-run/-n; <version> positional; AUR_REMOTE is the ssh URL.

Task 4: EDIT .gitignore — remove the `.SRCINFO` line (What (c))
  - EDIT: packaging/linux/aur/.gitignore — delete the single line `.SRCINFO`.
  - KEEP: pkg/, src/, *.pkg.tar.*, qmkonnect-*-linux-x86_64.tar.gz.
  - VERIFY: grep -cE '^\.SRCINFO$' packaging/linux/aur/.gitignore → 0.

Task 5: APPEND the Manual-AUR-Publication section to README.md (What (d))
  - EDIT: packaging/linux/aur/README.md — append the section (publish.sh usage, --dry-run,
          SSH-key prereq, ordering, source-repo sync, AUR URL).
  - DO NOT alter S1's existing install/maintenance content.

Task 6: VALIDATE (the gates)
  - RUN: cd packaging/linux/aur && diff <(makepkg --printsrcinfo) .SRCINFO   → empty
  - RUN: bash -n packaging/linux/aur/publish.sh                              → exit 0
  - RUN: test -x packaging/linux/aur/publish.sh && echo executable
  - RUN: cd packaging/linux/aur && ./publish.sh --dry-run 0.2.8              → exit 0 (network: downloads the v0.2.8 tarball for sha256)
  - RUN: git status --short -- packaging/linux/aur/   → only .SRCINFO, publish.sh, .gitignore, README.md (NO pkg/src/*.tar.gz)
  - RUN: git status --short -- Cargo.toml src/ .github/ packaging/linux/arch/   → empty
```

### Implementation Patterns & Key Details

```bash
# === .SRCINFO GENERATION (canonical, offline) ===
cd packaging/linux/aur && makepkg --printsrcinfo > .SRCINFO
#   Verified output: 19 lines, pkgbase=qmkonnect-bin, the v0.2.8 source URL + sha256.
#   The TAB-indented format is makepkg's canonical shape — do not reformat.

# === publish.sh ARG + sha256 REFRESH (the two non-obvious requirements) ===
#   1. `sed -i "s/^pkgver=.*/pkgver=${VERSION}/" PKGBUILD`           # version bump flows into source URL too (${pkgver})
#   2. `newsums="$(makepkg -g 2>/dev/null)"`                         # downloads tarball; prints sha256sums=('...')
#      `sed -i "s|^sha256sums=.*|${newsums}|" PKGBUILD`              # replace the (now-stale) checksum line
#   A bumped pkgver WITHOUT step 2 leaves a stale hash -> AUR install checksum mismatch. Mandatory.

# === AUR PUSH (flat temp clone; idempotent) ===
#   git clone aur@aur.archlinux.org:qmkonnect-bin.git "$WORK/aur"
#   cp PKGBUILD .SRCINFO qmkonnect.install "$WORK/aur/"              # AUR root is FLAT (no subdir)
#   git -C "$WORK/aur" add PKGBUILD .SRCINFO qmkonnect.install
#   git -C "$WORK/aur" diff --cached --quiet && echo "already published" || { git commit; git push; }
#   The diff --cached --quiet check makes a re-run of the same version a clean no-op.

# === --dry-run GATE (testable without an SSH key) ===
#   ./publish.sh --dry-run 0.2.8  → does steps 1-3 (patch+refresh+regen), skips step 4 (push).
#   Requires network (makepkg -g downloads the tarball) + the v0.2.8 release to be published.
```

### Integration Points

```yaml
SOURCE FILES:
  - create: "packaging/linux/aur/.SRCINFO, packaging/linux/aur/publish.sh"
  - modify: "packaging/linux/aur/.gitignore (remove .SRCINFO line), packaging/linux/aur/README.md (append section)"
  - do NOT modify: "PKGBUILD, qmkonnect.install, release.yml, Cargo.toml, any Rust source, packaging/linux/arch/"

AUR REMOTE (the publication target — a SEPARATE git repo):
  - url: "aur@aur.archlinux.org:qmkonnect-bin.git (SSH only; scp-style URL)"
  - flat contents: "PKGBUILD + .SRCINFO + qmkonnect.install at the repo root"
  - auth: "SSH key (public half registered in the AUR account); no token/password"

VERSION SOURCE (for the <version> arg; CI wires this in P1.M5.T1.S1):
  - pattern: "cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name==\"qmkonnect\") | .version'"

DOWNSTREAM CONSUMERS (do NOT implement now):
  - P1.M5.T1.S1: "CI job: on tag, extract version (cargo metadata | jq), load AUR SSH key from a
                  GitHub Actions secret into ssh-agent, run publish.sh <version>, then commit the
                  updated aur/PKGBUILD + aur/.SRCINFO back to the qmkonnet source repo."
  - P1.M6.T2.S2: "Regenerate docs/llms_full.txt + note the -bin AUR package in PACKAGING.md §4."

RELATED (NOT this subtask):
  - P1.M1.T1.S1: "the -bin PKGBUILD (S2's input)."
  - P1.M1.T2: "Nix flake (builds from source) — different channel, different files."
```

## Validation Loop

> All commands run from the repo root: `/home/dustin/projects/qmkonnect`. `makepkg 7.1.0` is
> installed; `namcap` is not (not a gate). The `--dry-run` gate (Level 2) needs network +
> the published v0.2.8 GitHub release.

### Level 1: `.SRCINFO` correctness (offline — the primary gate)

```bash
cd /home/dustin/projects/qmkonnect/packaging/linux/aur

# The committed .SRCINFO must exactly equal what makepkg generates from the PKGBUILD.
diff <(makepkg --printsrcinfo) .SRCINFO && echo ".SRCINFO in sync with PKGBUILD"
# Expected: "in sync" (empty diff). If non-empty, regenerate: `makepkg --printsrcinfo > .SRCINFO`.

# Key fields present (pkgbase/pkgver/install/source/sha256).
grep -E 'pkgbase = qmkonnect-bin|pkgver = 0.2.8|install = qmkonnect.install|source = https://|sha256sums = 86dcaa' .SRCINFO
# Expected: 5 matching lines.
```

### Level 2: `publish.sh` (syntax + dry-run)

```bash
cd /home/dustin/projects/qmkonnect

# Syntax.
bash -n packaging/linux/aur/publish.sh && echo "publish.sh syntax ok"
# Expected: "syntax ok".

# Executable bit.
test -x packaging/linux/aur/publish.sh && echo "executable"
# Expected: "executable".

# Dry run — regenerates .SRCINFO from the (re-)patched PKGBUILD; no SSH push.
# Requires network (makepkg -g downloads the v0.2.8 tarball) + the published v0.2.8 release.
cd packaging/linux/aur && ./publish.sh --dry-run 0.2.8
# Expected: prints the pkgver/sha256/.SRCINFO steps + "Dry-run: skipping AUR push."; exit 0.
# After it, re-confirm .SRCINFO is still in sync (dry-run regenerates it):
diff <(makepkg --printsrcinfo) .SRCINFO && echo ".SRCINFO still in sync"
```

### Level 3: `.gitignore` + repo hygiene

```bash
cd /home/dustin/projects/qmkonnect

# .SRCINFO is no longer ignored (the real one must be committable).
grep -cE '^\.SRCINFO$' packaging/linux/aur/.gitignore
# Expected: 0. (If 1, you forgot to remove the line.)

# The makepkg-artifact ignores are still present.
grep -cE '^(pkg/|src/|\*\.pkg\.tar\.\*|qmkonnect-\*-linux-x86_64\.tar\.gz)$' packaging/linux/aur/.gitignore
# Expected: 4 (the four artifact entries S1 added — all retained).

# git status shows ONLY the 4 intended changes; no makepkg artifacts leaked in.
git status --short -- packaging/linux/aur/
# Expected: ?? .SRCINFO   ?? publish.sh   M .gitignore   M README.md   (and NO PKGBUILD/qmkonnect.install
# change, NO pkg/ src/ *.tar.gz *.pkg.tar.* — those are gitignored).

# No file outside packaging/linux/aur/ was touched.
git status --short -- Cargo.toml Cargo.lock src/ .github/ packaging/linux/arch/ packaging/linux/udev/ packaging/linux/systemd/
# Expected: empty.
```

### Level 4: README section present

```bash
cd /home/dustin/projects/qmkonnect

# The Manual-AUR-Publication section exists and documents the SSH-key prereq + publish.sh usage.
grep -nE 'Manual AUR publication|publish\.sh|SSH|aur\.archlinux\.org' packaging/linux/aur/README.md
# Expected: several matching lines (the new section). Cross-check it mentions --dry-run + the
# ordering (publish the GitHub release first).
```

## Final Validation Checklist

### Technical Validation
- [ ] Level 1: `diff <(makepkg --printsrcinfo) .SRCINFO` (in aur/) → empty.
- [ ] Level 1: `.SRCINFO` has the 5 key fields (pkgbase/pkgver/install/source/sha256).
- [ ] Level 2: `bash -n publish.sh` → exit 0; `publish.sh` is executable.
- [ ] Level 2: `./publish.sh --dry-run 0.2.8` → exit 0 (network gate).
- [ ] Level 3: `grep -cE '^\.SRCINFO$' .gitignore` → 0; the 4 artifact entries retained.
- [ ] Level 3: `git status -- packaging/linux/aur/` → only `.SRCINFO`, `publish.sh`, `.gitignore`, `README.md`.
- [ ] Level 3: no change outside `packaging/linux/aur/`.

### Feature Validation
- [ ] `.SRCINFO` generated from the S1 PKGBUILD (the verified 19-line content).
- [ ] `publish.sh`: `<version>` arg, `--dry-run`, pkgver patch, sha256 refresh, `.SRCINFO` regen, temp-clone AUR push.
- [ ] `.gitignore`: `.SRCINFO` line removed.
- [ ] `README.md`: Manual-AUR-Publication section (publish.sh usage + SSH-key prereq + ordering + source-repo sync).

### Code Quality Validation
- [ ] `publish.sh` uses `set -euo pipefail`; `--help` prints the header; `command -v makepkg` guard.
- [ ] `publish.sh` is idempotent (`git diff --cached --quiet` no-op on re-run of the same version).
- [ ] `.SRCINFO` format untouched (TAB-indented makepkg shape; not reformatted).
- [ ] No hand-edit to S1's `PKGBUILD`/`qmkonnect.install`.

### Documentation & Deployment
- [ ] Mode A: README section rides with the work (manual publication instructions).
- [ ] PACKAGING.md / docs/*.md NOT edited here (the doc-sync milestone P1.M6 handles those).
- [ ] No Rust/CLI/config change.

---

## Anti-Patterns to Avoid

- ❌ Don't hand-write `.SRCINFO` — generate it with `makepkg --printsrcinfo > .SRCINFO` (it can't
  drift from the PKGBUILD). Pasting the verified 19 lines is acceptable ONLY with a `diff` re-check.
- ❌ Don't forget to remove `.SRCINFO` from S1's `.gitignore` — otherwise `git add .SRCINFO` silently
  no-ops and the AUR package index is missing from the commit. Gate: `grep -cE '^\.SRCINFO$' .gitignore` → 0.
- ❌ Don't write a publish.sh that bumps `pkgver` without refreshing `sha256sums` — a stale checksum
  breaks AUR installs. `makepkg -g` + sed is mandatory on every run.
- ❌ Don't push the whole `packaging/linux/aur/` dir to the AUR — the AUR repo is FLAT
  (PKGBUILD + .SRCINFO + qmkonnect.install only). publish.sh copies exactly those three into a temp clone.
- ❌ Don't add token/password auth to publish.sh — the AUR supports SSH-key auth ONLY. The script
  assumes ssh-agent is configured; a missing key fails cleanly at `git clone` (set -e).
- ❌ Don't run publish.sh before the GitHub release is published — `makepkg -g` downloads the tarball
  to compute the sha256. Document the ordering (release first, then publish.sh).
- ❌ Don't run makepkg/publish.sh as root — makepkg refuses root. CI must use a non-root step.
- ❌ Don't edit the PKGBUILD, `qmkonnect.install`, `release.yml`, `Cargo.toml`, or any Rust source —
  S2 is purely additive under `packaging/linux/aur/` (`.SRCINFO`, `publish.sh`) + two in-place edits
  (`.gitignore`, `README.md`).
- ❌ Don't wire the CI job here — that is P1.M5.T1.S1. publish.sh is the reusable unit; S2 delivers it,
  S1.M5.T1.S1 wraps it (version extraction + secret loading + on-tag trigger).
- ❌ Don't reformat the `.SRCINFO` (e.g. align fields, drop tabs) — makepkg's TAB-indented shape is what
  the AUR parses. `diff <(makepkg --printsrcinfo) .SRCINFO` being empty IS the format check.
- ❌ Don't auto-commit the qmkonnet SOURCE repo from publish.sh — it edits `aur/PKGBUILD` + `aur/.SRCINFO`
  in place; the maintainer/CI commits those separately (documented). No surprise commits.

---

**Confidence Score: 9/10** for one-pass implementation success. The `.SRCINFO` content is
**verified by running `makepkg --printsrcinfo` this session** (not a placeholder); the full
`publish.sh` is given verbatim with the mandatory sha256-refresh + temp-clone-push design;
the `.gitignore` edit is precisely specified (remove one line S1 documented adding); the AUR
SSH-auth model + flat-repo publication pattern are confirmed against `external_deps.md` §1 and
the Arch wiki; and the gates (`diff <(makepkg --printsrcinfo) .SRCINFO`, `bash -n`, `--dry-run`)
are real and executable on this host (makepkg 7.1.0 installed). The one residual variable is
the network-dependent `--dry-run` gate (it downloads the v0.2.8 tarball) — mitigated by the
offline `.SRCINFO`-sync `diff` as the primary gate and the documented release-first ordering.
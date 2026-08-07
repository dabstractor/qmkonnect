# PRP — P1.M1.T1.S3: Update AUR PKGBUILD to install XDG autostart .desktop (F17)

## Goal

**Feature Goal**: Make the AUR binary package (`qmkonnect-bin`) install the XDG
autostart `.desktop` at `/etc/xdg/autostart/qmkonnect.desktop`, so login-autostart
works out of the box on **systemd AND non-systemd** distros (F17), matching the
behavior the source PKGBUILD (`packaging/linux/arch/`) already ships.

**Deliverable**:
1. Updated `packaging/linux/aur/PKGBUILD` — one new `install -Dm644` line in
   `package()` (plus a comment), pulling the `.desktop` out of the release
   tarball's `xdg/` subdir.
2. Regenerated `packaging/linux/aur/.SRCINFO` (via `makepkg --printsrcinfo`).
3. Updated `packaging/linux/aur/README.md` — note that login-autostart ships
   out-of-the-box via the `.desktop` (Mode A doc note).

**Success Definition**: On `makepkg -f` against a tarball containing the `xdg/`
subdir, the built `.pkg.tar.zst` contains
`etc/xdg/autostart/qmkonnect.desktop`, the `.SRCINFO` is in sync with the
`PKGBUILD`, and the README's install manifest lists the new FHS path.

## Why

- **F17 (PRD §4) requires universal Linux autostart**: "XDG autostart `.desktop`
  alongside the systemd user service, so login-autostart works on systemd **and**
  non-systemd distros (MX/Artix/Void/Gentoo)."
- **PACKAGING.md §4** states every Linux package must install the **same files to
  the same FHS paths**, and §4.7 adds: "Ship it at
  `/etc/xdg/autostart/qmkonnect.desktop` in **every** Linux package
  (.deb/.rpm/PKGBUILD/**AUR**/tarball)."
- The AUR `-bin` package (P1.M1.T1.S1, Complete) predates F17 — it installs only
  the binary, hid-id helper, udev rule, and systemd template (four paths). The
  sibling source PKGBUILD (P2.M6.T1.S1) was already updated to ship the fifth
  path; this task brings the `-bin` package to parity so the two AUR siblings
  stay identical on disk.
- **Scope cohesion**: This is a pure packaging-parity edit. The `.desktop` file
  itself already exists (`packaging/linux/xdg/qmkonnect.desktop`, created by
  P2.M6.T1.S1) and the CI `linux-binary` job already stages it into the release
  tarball (P2.M7.T2.S1, Complete). This task only consumes that tarball entry.

## What

Add a fifth `install -Dm644` line to `packaging/linux/aur/PKGBUILD`'s `package()`
that installs the `.desktop` from the tarball's `xdg/` subdir to the FHS autostart
path. Regenerate `.SRCINFO`. Document the out-of-the-box autostart in the README.

### Success Criteria

- [ ] `packaging/linux/aur/PKGBUILD` `package()` installs
      `${stage}/xdg/qmkonnect.desktop` → `$pkgdir/etc/xdg/autostart/qmkonnect.desktop`.
- [ ] The install source path uses the **tarball** layout (`${stage}/xdg/...`),
      NOT the source-repo layout (`../xdg/...`) the arch PKGBUILD uses.
- [ ] The `.desktop` is **NOT** added to the `backup` array (it is package-owned,
      like the static udev rule + helper — not preserved across upgrades).
- [ ] `.SRCINFO` regenerated via `makepkg --printsrcinfo > .SRCINFO` and in sync
      with the patched `PKGBUILD` (expected: byte-identical — see Gotcha #2).
- [ ] `qmkonnect.install` pacman hooks are **unchanged** (the `.desktop` needs no
      instantiation; it is honored passively by the DE session manager).
- [ ] `README.md` notes login-autostart works out of the box via the shipped
      `.desktop`, and the "What it installs" manifest lists
      `/etc/xdg/autostart/qmkonnect.desktop`.
- [ ] No CI change is required (publish.sh + the AUR publish job are generic).

## All Needed Context

### Context Completeness Check

_Pass_: An implementer with no prior knowledge of this repo needs only the four
files below plus this PRP. The edit is a one-line addition to a `package()`
function whose exact source/dest paths, dependencies, and validation are all
specified here with verbatim reference patterns.

### Documentation & References

```yaml
# MUST READ - the spec that mandates this feature
- url: spec/PACKAGING.md#47-xdg-autostart-desktop-packaginglinuxxdgqmkonnettdesktop--new
  why: "§4.7 mandates shipping the .desktop at /etc/xdg/autostart in EVERY Linux
        package, including AUR. §4 ('same files to the same FHS paths') table lists
        the exact target path."
  critical: "Target path MUST be /etc/xdg/autostart/qmkonnect.desktop (FHS, system-wide
        autostart). Do NOT install to ~/.config/autostart (that is the per-user
        override location per §4.7)."

- url: spec/PACKAGING.md#42-aur-qmkonnect-bin-packaginglinuxaur
  why: "§4.2 describes the -bin PKGBUILD: 'Installs the same four files as the source
        PKGBUILD' — this task adds the FIFTH file (the .desktop) to reach parity."

# THE PROVEN PATTERN TO COPY (source PKGBUILD already ships the .desktop)
- file: packaging/linux/arch/PKGBUILD
  why: "The arch SOURCE PKGBUILD's package() ALREADY installs the .desktop (added by
        P2.M6.T1.S1). This is the exact pattern to mirror in the AUR -bin PKGBUILD."
  pattern: |
    # XDG autostart entry — universal login-autostart fallback (F17; LINUX.md §6.3).
    # Static file (no template instantiation); NoDisplay=true hides it from app menus.
    install -Dm644 "../xdg/qmkonnect.desktop" "$pkgdir/etc/xdg/autostart/qmkonnect.desktop"
  gotcha: "DIFFERENT SOURCE PATH between the two PKGBUILDs. The arch PKGBUILD builds
        from the SOURCE REPO (relative path ../xdg/qmkonnect.desktop, resolved against
        $srcdir/..). The AUR -bin PKGBUILD pulls from the RELEASE TARBALL, so the
        source path MUST be ${stage}/xdg/qmkonnect.desktop where
        stage='qmkonnect-${pkgver}-linux-x86_64'. Copying the arch ../xdg/ path
        verbatim WILL FAIL at package() time (file not found in extracted tarball)."

# THE FILE BEING EDITED
- file: packaging/linux/aur/PKGBUILD
  why: "The -bin PKGBUILD to edit. Its package() currently installs 4 files from the
        tarball; add the 5th. Note the existing local `stage` variable and the
        `backup` array (do NOT touch backup)."
  pattern: "package() uses: local stage=\"qmkonnect-${pkgver}-linux-x86_64\"; then
        install -Dm644 \"${stage}/<subdir>/<file>\" \"${pkgdir}/<fhs-path>\". Mirror
        that exact quoting/style for the new line."

# CONFIRMS THE TARBALL ACTUALLY CONTAINS xdg/ (dependency already satisfied)
- file: .github/workflows/release.yml
  why: "The CI `linux-binary` job stages the release tarball. Lines ~175/180 already
        create $STAGE/xdg and copy packaging/linux/xdg/qmkonnect.desktop into it.
        So NEW release tarballs WILL contain qmkonnect-<ver>-linux-x86_64/xdg/qmkonnect.desktop."
  critical: "The locally-committed tarball packaging/linux/aur/qmkonnect-0.2.8-linux-x86_64.tar.gz
        is STALE (predates the CI xdg staging) and does NOT contain xdg/. It is also
        gitignored. Do NOT rely on it for package() validation without first adding
        the xdg/ subdir (see Validation Level 3)."

# THE REGENERATION TOOL
- file: packaging/linux/aur/publish.sh
  why: "publish.sh is the canonical regenerator: it runs `makepkg --printsrcinfo > .SRCINFO`.
        For this task, run that one command directly (publish.sh also bumps pkgver +
        pushes to AUR, which is OUT OF SCOPE here — this task only edits + regenerates
        locally; the next tagged release runs publish.sh in CI)."
  gotcha: "publish.sh refuses to run as root and requires the GitHub release tarball to
        exist (makepkg -g downloads it). For a pure local .SRCINFO regen, call
        makepkg --printsrcinfo directly — it only parses PKGBUILD, no network needed."

# THE DOC FILE TO UPDATE (Mode A note)
- file: packaging/linux/aur/README.md
  why: "README's 'What it installs' manifest (a table) and the 'same four paths'
        intro line both need updating to reflect the fifth path. Add an autostart note."
```

### Current Codebase tree (relevant slice)

```bash
packaging/linux/
├── aur/
│   ├── PKGBUILD                      # EDIT — add .desktop install line in package()
│   ├── .SRCINFO                      # REGENERATE — makepkg --printsrcinfo > .SRCINFO
│   ├── publish.sh                    # READ ONLY — canonical regenerator (do not change)
│   ├── README.md                     # EDIT (Mode A) — add autostart note + 5th manifest row
│   ├── qmkonnect.install             # UNCHANGED — pacman hooks need no .desktop handling
│   └── qmkonnect-0.2.8-linux-x86_64.tar.gz  # STALE/gitignored — predates CI xdg staging
├── arch/
│   └── PKGBUILD                      # READ ONLY — PROVEN PATTERN (already ships .desktop)
├── systemd/qmkonnect.service.template
├── udev/69-qmkonnect-rawhid.rules
└── xdg/
    └── qmkonnect.desktop             # INPUT — the file to install (created P2.M6.T1.S1)
```

### Known Gotchas of our codebase & Library Quirks

```bash
# CRITICAL #1 — TARBALL vs SOURCE-REPO path divergence.
# The two PKGBUILDs pull the .desktop from DIFFERENT places:
#   arch (source) : ../xdg/qmkonnect.desktop        ← source repo, resolved vs $srcdir/..
#   aur  (-bin)   : ${stage}/xdg/qmkonnect.desktop  ← inside the extracted release tarball
# The AUR PKGBUILD defines:  local stage="qmkonnect-${pkgver}-linux-x86_64"
# and cd's into $srcdir. So the source path MUST be "${stage}/xdg/qmkonnect.desktop".
# Copying the arch PKGBUILD's ../xdg/ path verbatim => "file not found" in package().

# CRITICAL #2 — .SRCINFO is expected to be BYTE-IDENTICAL after this edit.
# `makepkg --printsrcinfo` emits ONLY top-level + per-split-package metadata
# (pkgver/pkgrel/depends/source/sha256sums/backup/options/install/arch/url/...).
# It does NOT parse the package() body. Adding an install line changes none of
# those metadata fields, so .SRCINFO will not diff. STILL regenerate it (the
# contract requires it; running the command is cheap and proves in-sync). If
# `git diff` shows no .SRCINFO change, that is the CORRECT expected outcome —
# do not "force" a change.

# CRITICAL #3 — do NOT add the .desktop to the `backup` array.
# backup=("usr/lib/systemd/user/qmkonnect.service.template") is intentional:
# ONLY the user-instantiated service template is preserved across upgrades.
# The .desktop is a STATIC package-owned file (regenerated on upgrade, removed
# on uninstall) — exactly like the static udev rule and the hid-id helper. The
# arch source PKGBUILD does NOT back it up either; match that.

# CRITICAL #4 — the local 0.2.8 tarball is STALE (no xdg/ subdir).
# It was committed Aug 7 before the CI `linux-binary` job learned to stage xdg/
# (release.yml ~line 175/180). It also matches .gitignore
# (qmkonnect-*-linux-x86_64.tar.gz). For local package() validation you MUST
# build a synthetic tarball WITH the xdg/ subdir (see Validation Level 3).
# Do not assume the real release tarball exists yet for v0.2.8.

# QUIRK #5 — makepkg is Arch-only but IS installed on this dev box (/usr/bin/makepkg).
# So makepkg-based validation gates are runnable here. On a non-Arch host, fall
# back to the static gates (bash -n + grep) and rely on CI for the full build.

# QUIRK #6 — qmkonnect.install pacman hooks need NO change.
# The systemd service template is INSTANTIATED by post_install (install -m644
# template -> service). The .desktop needs NO instantiation — DE session
# managers honor /etc/xdg/autostart/*.desktop passively at login. Do not touch
# the hooks.
```

## Implementation Blueprint

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: EDIT packaging/linux/aur/PKGBUILD — add the .desktop install line
  - LOCATE: the package() function; find the systemd template install block:
        install -Dm644 "${stage}/systemd/qmkonnect.service.template" \
          "${pkgdir}/usr/lib/systemd/user/qmkonnect.service.template"
  - ADD immediately after it (new fifth block):
        # XDG autostart entry — universal login-autostart fallback (F17; LINUX.md §6.3,
        # PACKAGING.md §4.7). Static file (no template instantiation); NoDisplay=true
        # hides it from app menus. Source path is INSIDE the release tarball
        # (${stage}/xdg/...), NOT the source-repo ../xdg/ path the arch PKGBUILD uses.
        install -Dm644 "${stage}/xdg/qmkonnect.desktop" "${pkgdir}/etc/xdg/autostart/qmkonnect.desktop"
  - DO NOT MODIFY: pkgname/pkgver/pkgrel/depends/source/sha256sums/backup/options/install.
  - DO NOT add the .desktop to the `backup` array.
  - VERIFY (Level 1): bash -n packaging/linux/aur/PKGBUILD   # syntax OK

Task 2: REGENERATE packaging/linux/aur/.SRCINFO
  - RUN (from packaging/linux/aur/):  makepkg --printsrcinfo > .SRCINFO
  - EXPECT: byte-identical to the pre-edit copy (Gotcha #2). Confirm with:
        git diff --stat packaging/linux/aur/.SRCINFO    # likely: no output
  - WHY still run it: the contract (item OUTPUT) requires .SRCINFO to be
    regenerated and provably in sync; the command is the authoritative check.
    If makepkg is unavailable, document that CI publish.sh regenerates it on
    the next tagged release and leave the file as-is.

Task 3: EDIT packaging/linux/aur/README.md — Mode A autostart note + manifest
  - UPDATE intro line ~18 from "same four paths" to "same five paths":
      "Both packages install to the **same five paths** ..."
    (reflects the new /etc/xdg/autostart/qmkonnect.desktop entry.)
  - ADD a row to the "What it installs" table (after the systemd template row ~61):
      | `/etc/xdg/autostart/qmkonnect.desktop` | XDG autostart entry — starts the daemon at login on systemd **and** non-systemd distros (F17) |
  - ADD a short note (e.g., under "What it installs" or a new bullet in Notes)
    that login-autostart works OUT OF THE BOX via the shipped .desktop — no
    `systemctl --user enable` needed for the systemd-agnostic path; the service
    template + `systemctl --global enable` (from qmkonnect.install) still
    provides the richer plug/unplug lifecycle on systemd hosts.
  - KEEP tone/voice consistent with the existing README (terse, factual,
    cross-linked to spec/PACKAGING.md).

Task 4: VERIFY no collateral changes needed
  - CONFIRM qmkonnect.install is unchanged (grep for xdg/desktop => expect none).
  - CONFIRM no CI edit required: the AUR publish job (release.yml ~line 475) and
    publish.sh are generic — they regenerate .SRCINFO from whatever PKGBUILD
    exists. The new install line flows through automatically on the next release.
    State this explicitly in the PRP validation notes; make NO release.yml edit.
```

### Implementation Pattern & Key Detail (the exact edit)

```bash
# In packaging/linux/aur/PKGBUILD, INSIDE package(), AFTER the systemd block:

  # systemd user service template (post_install instantiates it). Binds to the
  # qmkonnect_device symlink the static rule creates for any matched interface.
  install -Dm644 "${stage}/systemd/qmkonnect.service.template" "${pkgdir}/usr/lib/systemd/user/qmkonnect.service.template"

  # XDG autostart entry — universal login-autostart fallback (F17; LINUX.md §6.3,
  # PACKAGING.md §4.7). Static file (no template instantiation); NoDisplay=true
  # hides it from app menus. Source path is INSIDE the release tarball
  # (${stage}/xdg/...), NOT the source-repo ../xdg/ path the arch PKGBUILD uses.
  install -Dm644 "${stage}/xdg/qmkonnect.desktop" "${pkgdir}/etc/xdg/autostart/qmkonnect.desktop"
```

### Integration Points

```yaml
CI:
  - change: NONE
  - reason: >
      The AUR publish job (.github/workflows/release.yml ~line 475) runs
      publish.sh, which is generic: it patches pkgver, refreshes sha256sums
      (makepkg -g), regenerates .SRCINFO, and pushes. The new package() install
      line requires NO CI awareness — the next tagged release stages a tarball
      (linux-binary job, already with xdg/) and publish.sh regenerates .SRCINFO
      from the updated PKGBUILD automatically. Do NOT edit release.yml.

CONFIG / DATABASE:
  - change: NONE

PACKAGE METADATA (PKGBUILD top-level):
  - depends:    UNCHANGED (no new runtime dep — Exec=qmkonnect -> /usr/bin/qmkonnect already a dep)
  - backup:     UNCHANGED (do NOT add the .desktop; it is package-owned)
  - install:    UNCHANGED (qmkonnect.install needs no .desktop handling)
  - source/sha256sums: UNCHANGED (the tarball URL/hash are unaffected; only how
                its contents are installed changes)

DOWNSTREAM (the real release tarball):
  - dependency: P2.M7.T2.S1 (Complete) — CI linux-binary job already stages
                $STAGE/xdg/qmkonnect.desktop into the tarball. So a tarball
                produced AFTER that CI change contains the xdg/ subdir this
                PKGBUILD now consumes. No new cross-task work needed.
```

## Validation Loop

> `makepkg` IS installed on this dev box (`/usr/bin/makepkg`), so the makepkg
> gates below are runnable here. Levels 1–2 are always runnable on any host.

### Level 1: Syntax & Static Checks (always runnable)

```bash
cd packaging/linux/aur

# 1a. PKGBUILD is valid bash.
bash -n PKGBUILD && echo "PKGBUILD syntax OK"

# 1b. The new install line is present with the CORRECT (tarball) source path
#     and the CORRECT (FHS) dest path.
grep -F '${stage}/xdg/qmkonnect.desktop' PKGBUILD           # source path (tarball layout)
grep -F '${pkgdir}/etc/xdg/autostart/qmkonnect.desktop' PKGBUILD  # dest path (FHS)

# 1c. The .desktop was NOT added to backup (it must stay package-owned).
grep -c 'etc/xdg/autostart' PKGBUILD          # expect: 1 (only the install line)
! grep -q 'xdg/autostart' <(sed -n '/^backup=(/,/)/p' PKGBUILD) && echo "backup array correctly excludes .desktop"

# 1d. .SRCINFO parses cleanly and is in sync with PKGBUILD.
makepkg --printsrcinfo > /tmp/new.SRCINFO && diff -u .SRCINFO /tmp/new.SRCINFO && echo ".SRCINFO in sync"

# 1e. The input .desktop file exists in the source tree.
test -f ../xdg/qmkonnect.desktop && echo "xdg/qmkonnect.desktop present"

# Expected: all echo lines print; diff (1d) is empty (byte-identical .SRCINFO is CORRECT).
```

### Level 2: .SRCINFO Regeneration (contract output)

```bash
cd packaging/linux/aur
# The canonical regenerator used by publish.sh:
makepkg --printsrcinfo > .SRCINFO

# Confirm the metadata is unchanged vs the pre-edit copy (expected: no diff).
git diff --stat .SRCINFO          # expect: no output (byte-identical)
git diff .SRCINFO | head          # if empty, that is the CORRECT outcome — see Gotcha #2

# Sanity: .SRCINFO still lists the source tarball + checksum (unaffected fields).
grep -E '^(source|sha256sums) =' .SRCINFO
```

### Level 3: Build Validation — file lands at the FHS path (needs makepkg)

> The local `qmkonnect-0.2.8-linux-x86_64.tar.gz` is STALE (no `xdg/` subdir).
> Build a synthetic tarball WITH the `xdg/` subdir so package() can run, then
> verify the `.desktop` lands inside the built `.pkg.tar.zst`.

```bash
cd packaging/linux/aur
cp -a qmkonnect-0.2.8-linux-x86_64.tar.gz /tmp/aur-test.tar.gz   # work on a copy
mkdir -p /tmp/aur-syn && tar xzf /tmp/aur-test.tar.gz -C /tmp/aur-syn
# Add the xdg/ subdir the real CI tarball now ships:
mkdir -p /tmp/aur-syn/qmkonnect-0.2.8-linux-x86_64/xdg
cp ../xdg/qmkonnect.desktop /tmp/aur-syn/qmkonnect-0.2.8-linux-x86_64/xdg/
# Repack into the exact filename PKGBUILD's source= expects, then rebuild:
( cd /tmp/aur-syn && tar czf /tmp/aur-test.tar.gz qmkonnect-0.2.8-linux-x86_64 )
cp /tmp/aur-test.tar.gz ./qmkonnect-0.2.8-linux-x86_64.tar.gz   # local-only test copy

makepkg -f                                   # builds qmkonnect-bin-0.2.8-1-x86_64.pkg.tar.zst

# Verify the .desktop IS in the built package at the FHS path:
tar tf qmkonnect-bin-0.2.8-1-x86_64.pkg.tar.zst | grep 'etc/xdg/autostart/qmkonnect.desktop'
# Expected: prints the path (exit 0). Empty => the install line is wrong — fix Task 1.

# Verify perms are 0644 inside the package (should match the install -Dm644):
tar tvf qmkonnect-bin-0.2.8-1-x86_64.pkg.tar.zst | grep 'etc/xdg/autostart/qmkonnect.desktop'
# Expected: leading '-rw-r--r--'.

# Cleanup the local-only synthetic tarball + build artifacts (they are gitignored,
# but remove them so the dir is clean):
rm -f qmkonnect-bin-0.2.8-1-x86_64.pkg.tar.zst qmkonnect-0.2.8-linux-x86_64.tar.gz
rm -rf src/ pkg/
git status --short                # confirm only PKGBUILD / .SRCINFO / README.md changed
```

### Level 4: Optional — namcap lint (if available)

```bash
# namcap is the Arch PKGBUILD linter; optional, not installed on all hosts.
command -v namcap >/dev/null && namcap PKGBUILD || echo "namcap not installed (optional, skip)"
# Expected: no errors about the new line. (pre-existing hidapi/libusb split-package
# notes, if any, are unrelated and out of scope.)
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1: `bash -n PKGBUILD` passes; both grep assertions (1b) match;
      backup array excludes the .desktop (1c); `.SRCINFO` in sync (1d); input
      `.desktop` present (1e).
- [ ] Level 2: `.SRCINFO` regenerated via `makepkg --printsrcinfo > .SRCINFO`;
      `git diff` shows expected (likely empty) result.
- [ ] Level 3: `makepkg -f` against a synthetic tarball WITH `xdg/` succeeds and
      the built `.pkg.tar.zst` contains `etc/xdg/autostart/qmkonnect.desktop`
      at mode `0644`.
- [ ] No edit to `.github/workflows/release.yml`, `publish.sh`, or
      `qmkonnect.install`.

### Feature Validation

- [ ] AUR `-bin` package now installs the same FIVE paths as the arch source
      package (parity with `packaging/linux/arch/PKGBUILD`).
- [ ] The `.desktop` lands at `/etc/xdg/autostart/qmkonnect.desktop` (the exact
      FHS path from PACKAGING.md §4 table).
- [ ] The source path is the **tarball** layout (`${stage}/xdg/...`), confirmed
      distinct from the arch PKGBUILD's source-repo layout (`../xdg/...`).

### Documentation (Mode A)

- [ ] `README.md` intro updated "four paths" → "five paths".
- [ ] `README.md` "What it installs" table has a row for
      `/etc/xdg/autostart/qmkonnect.desktop`.
- [ ] `README.md` notes login-autostart works out of the box via the shipped
      `.desktop` (F17).

### Code Quality / Convention

- [ ] New install line mirrors the existing four install lines' quoting/style
      (`install -Dm644 "${stage}/..." "${pkgdir}/..."`).
- [ ] Comment explains F17 / §4.7 provenance and the tarball-vs-source path nuance.
- [ ] No stray build artifacts left in `packaging/linux/aur/`
      (`git status --short` shows only the intended doc/code edits).

---

## Anti-Patterns to Avoid

- ❌ Don't copy the arch PKGBUILD's `../xdg/qmkonnect.desktop` source path verbatim
      — the AUR `-bin` package reads from the **tarball** (`${stage}/xdg/...`), not
      the source repo. (Gotcha #1.)
- ❌ Don't add the `.desktop` to the `backup` array — it is package-owned, like the
      static udev rule and helper, not a user-instantiated template. (Gotcha #3.)
- ❌ Don't touch `qmkonnect.install` — the `.desktop` needs no pacman-hook
      instantiation; DE session managers honor it passively. (Gotcha #6.)
- ❌ Don't edit `.github/workflows/release.yml` or `publish.sh` — they are generic
      and pick up the new `package()` body automatically on the next release.
- ❌ Don't "force" a `.SRCINFO` change — it is expected to be byte-identical because
      `--printsrcinfo` does not parse `package()` bodies. An empty diff is success.
      (Gotcha #2.)
- ❌ Don't validate against the stale local 0.2.8 tarball as-is — it lacks `xdg/`.
      Build the synthetic tarball in Level 3 first.
- ❌ Don't install to `~/.config/autostart/` — that is the per-user *override*
      location (PACKAGING.md §4.7). The package ships the system-wide
      `/etc/xdg/autostart/` entry.

---

**Confidence Score: 9/10** — The change is a single well-scoped `install` line
with a proven sibling pattern (arch PKGBUILD), the input file and tarball-staging
dependency are both already Complete, `makepkg` is available for full build
validation, and every gotcha (path divergence, .SRCINFO no-op, stale local
tarball, backup exclusion) is explicitly documented. The -1 reserves for the
non-Arch-host fallback path (Levels 1–2 only) where the build gate (Level 3)
cannot run locally.
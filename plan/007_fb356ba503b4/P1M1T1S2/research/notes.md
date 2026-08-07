# Research Notes — P1.M1.T1.S2 (Create .SRCINFO + AUR repo publication infrastructure)

## 0. Task shape & contract

S2 creates the AUR **publication infrastructure** that consumes S1's `-bin` PKGBUILD:
1. `packaging/linux/aur/.SRCINFO` — generated from S1's PKGBUILD via `makepkg --printsrcinfo`.
2. `packaging/linux/aur/publish.sh` — bumps pkgver, refreshes sha256, regenerates
   `.SRCINFO`, and pushes `PKGBUILD + .SRCINFO + qmkonnect.install` to the AUR git remote.
3. README.md addition — Mode-A manual-publication instructions (publish.sh usage).

No Rust/source change. S2 consumes S1's `packaging/linux/aur/{PKGBUILD,qmkonnect.install,
.gitignore,README.md}` (S1 is "Implementing"; its `aur/` dir already exists — verified).

## 1. The S1 contract (what S2 builds on — verified present in the tree)

`ls packaging/linux/aur/` (this session) shows S1 already created: `PKGBUILD`,
`qmkonnect.install`, `README.md`, `.gitignore`, plus makepkg-validation artifacts
(`qmkonnect-0.2.8-linux-x86_64.tar.gz`, `src/`) that the `.gitignore` excludes.

S1's `aur/PKGBUILD`: `pkgname=qmkonnect-bin`, `pkgver=0.2.8`, `source=()` = the GitHub
release URL with `${pkgver}`, `sha256sums=('86dcaa…b216')`, no `build()`/`makedepends`,
`install=qmkonnect.install`, the four `/usr` install paths. (See S1 PRP "What (a)".)

S1's `aur/.gitignore` (VERIFIED this session — S2 must edit it):
```gitignore
pkg/
src/
*.pkg.tar.*
qmkonnect-*-linux-x86_64.tar.gz
.SRCINFO            # <-- S2 MUST REMOVE THIS LINE (the real .SRCINFO must be committed)
```
S1 intentionally listed `.SRCINFO` so its validation (`makepkg --printsrcinfo > .SRCINFO`)
wouldn't accidentally commit a throwaway file. S2 COMMITs the real `.SRCINFO`, so it must
delete that line. The other four entries (makepkg artifacts) stay.

## 2. .SRCINFO — VERIFIED generation + exact content (run this session)

`makepkg --printsrcinfo` IS installed (makepkg 7.1.0 / pacman 7.1.0) and parses the S1
PKGBUILD cleanly. The exact output (19 lines) — this IS the target `.SRCINFO` content:

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

Format notes (AUR/makepkg convention):
- Line 1 is `pkgbase = <name>` (column 0). All metadata fields are **TAB-indented**.
- A blank line separates `pkgbase`-level fields from the `pkgname = <name>` closer (column 0).
- The AUR web interface parses this file for search results + metadata. **It MUST be committed**
  (not gitignored) and kept in sync with the PKGBUILD (`diff <(makepkg --printsrcinfo) .SRCINFO`
  must be empty).

Canonical generation (S2 uses this verbatim): `makepkg --printsrcinfo > .SRCINFO` (run from
`packaging/linux/aur/`). The implementer can either run that or paste the 19 lines above and
verify with the diff — running it is authoritative (it can't drift from the PKGBUILD).

## 3. AUR publication model (external_deps.md §1, verified)

- **AUR repo**: a SEPARATE git remote at `aur@aur.archlinux.org:qmkonnect-bin.git` (SSH,
  scp-style URL). It is NOT the qmkonnect source repo. It contains ONLY the flat package
  files: `PKGBUILD`, `.SRCINFO`, `qmkonnect.install` at the repo root.
- **Auth**: SSH key ONLY — the AUR has no token/password auth. Register the PUBLIC key in the
  AUR account (https://aur.archlinux.org → My Account → SSH Public Key). For CI, store the
  PRIVATE key as a GitHub Actions secret and load it into ssh-agent before publishing
  (P1.M5.T1.S1 wires that; S2's publish.sh assumes SSH is configured).
- **Publish pattern** (external_deps.md "CI approach"): `git clone` the AUR repo → copy in
  the updated PKGBUILD/.SRCINFO/.install → commit → push. publish.sh clones to a TEMP dir
  each run (idempotent, no stale local AUR checkout to manage).
- **`.SRCINFO` is committed** to the AUR repo (it's the package index). publish.sh `git add`s
  all three files.

## 4. publish.sh — design (the script S2 creates)

Core contract: `./publish.sh <version>` patches `pkgver` in PKGBUILD, refreshes sha256,
regenerates `.SRCINFO`, and pushes the three files to the AUR remote. Add `--dry-run` for a
testable gate that does the local steps without the SSH push.

Why refresh sha256 (not in the literal contract, but mandatory): bumping `pkgver` without
refreshing `sha256sums` leaves a STALE hash → AUR users get a checksum mismatch on install.
`makepkg -g` downloads the release tarball and prints the correct `sha256sums=(…)` line; sed
it into the PKGBUILD. (Idempotent for the same version — refreshes to the identical value.)

Key script properties:
- `#!/usr/bin/env bash` + `set -euo pipefail` (robust for arg + git ops; macos scripts use
  `set -e`, but `set -euo pipefail` is the right defensive choice for a publish script).
- Arg parse: `--dry-run`/`-n` flag + a positional `<version>` (e.g. `0.2.8`).
- `command -v makepkg` guard (makepkg is Arch-only).
- pkgver patch: `sed -i "s/^pkgver=.*/pkgver=${VERSION}/" PKGBUILD`.
- sha256 refresh: `newsums="$(makepkg -g 2>/dev/null)"; sed -i "s|^sha256sums=.*|${newsums}|" PKGBUILD`.
- `.SRCINFO` regen: `makepkg --printsrcinfo > .SRCINFO`.
- AUR push (skipped under --dry-run): temp-dir `git clone aur@aur.archlinux.org:qmkonnect-bin.git`,
  copy in PKGBUILD + .SRCINFO + qmkonnect.install, `git add`, `git diff --cached --quiet`
  idempotency check, `git commit -m "qmkonnect-bin v${VERSION}"`, `git push`. `trap` cleanup.

Ordering constraint (documented): the GitHub release for `<version>` must be published FIRST
— `makepkg -g` downloads the tarball to compute its sha256. publish.sh is run AFTER
release.yml publishes the release.

makepkg refuses root: run publish.sh as a normal user (documented; CI must use a non-root step).

publish.sh modifies the SOURCE repo's `aur/PKGBUILD` + `aur/.SRCINFO` in place (the canonical
template). The maintainer/CI should ALSO commit those to the qmkonnect source repo so it stays
in sync with the AUR. publish.sh does NOT auto-commit the source repo (no surprise commits);
it only pushes to the AUR. CI integration (cargo-metadata version extraction + secret loading
+ source-repo commit) is P1.M5.T1.S1's scope.

Version source for the `<version>` arg (release.yml lines 41-46 canonical pattern, for the
README/CI doc): `cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name=="qmkonnect") | .version'`.

## 5. README.md addition (Mode A — rides with the work)

S1's README covers install + per-release checksum maintenance. S2 ADDS a "Manual AUR
publication" section: the publish.sh usage, the SSH-deploy-key prerequisite, the version-arg,
the ordering (publish GitHub release first), and the source-repo commit note. Cross-reference
S1's checksum-refresh note + the AUR repo URL.

## 6. Validation gates (all verified executable this session)

1. `.SRCINFO` exists + is in sync with the PKGBUILD (offline, the primary gate):
   `cd packaging/linux/aur && diff <(makepkg --printsrcinfo) .SRCINFO` → empty.
2. `.SRCINFO` carries the expected fields: `grep -E 'pkgbase = qmkonnect-bin|pkgver = 0.2.8|install = qmkonnect.install|sha256sums =' .SRCINFO` → 4 hits.
3. publish.sh syntax: `bash -n packaging/linux/aur/publish.sh` → exit 0.
4. publish.sh is executable: `test -x packaging/linux/aur/publish.sh`.
5. publish.sh --dry-run (network-dependent — downloads the v0.2.8 tarball for sha256):
   `cd packaging/linux/aur && ./publish.sh --dry-run 0.2.8` → regenerates .SRCINFO, prints
   the dry-run skip, exit 0. (Requires the v0.2.8 GitHub release to be published + network.)
6. `.gitignore` no longer lists `.SRCINFO`: `grep -cE '^\.SRCINFO$' packaging/linux/aur/.gitignore` → 0.
7. Repo hygiene: `git status --short -- packaging/linux/aur/` shows `.SRCINFO`, `publish.sh`,
   modified `.gitignore`, modified `README.md` — and NO makepkg artifacts (pkg/src/*.tar.gz ignored).

## 7. Files NOT to touch (boundary discipline)

- `packaging/linux/aur/PKGBUILD` — S1's deliverable. publish.sh EDITS it in place at runtime
  (pkgver/sha256), but S2 does not hand-edit it.
- `packaging/linux/aur/qmkonnect.install` — S1's byte-identical copy. S2 only references it.
- `packaging/linux/arch/` — the source PKGBUILD; untouched.
- `.github/workflows/release.yml` — the CI job (P1.M5.T1.S1 adds the AUR publish job).
- Any Rust source / Cargo.toml.
- `spec/PACKAGING.md`, `docs/*.md` — the doc-sync milestone (P1.M6) handles those.

## 8. Risk inventory (all low; all gated)

1. **Forgetting to remove `.SRCINFO` from S1's `.gitignore`** → the committed .SRCINFO would
   be ignored / `git add` skips it. Mitigated: Task explicitly edits the .gitignore; gate #6.
2. **Stale sha256 on a version bump** → publish.sh refreshes via `makepkg -g` (mandatory).
3. **publish.sh run as root** → makepkg refuses root; documented; CI uses non-root.
4. **Running publish.sh before the GitHub release exists** → `makepkg -g` fails to download;
   the script errors with a clear message; ordering documented.
5. **Missing SSH deploy key** → the AUR `git clone`/`push` fails (set -e); documented prereq.
6. **Drift between the committed .SRCINFO and the PKGBUILD** → gate #1 (`diff`) catches it;
   publish.sh regenerates .SRCINFO from the current PKGBUILD every run, so they can't drift
   post-publish.
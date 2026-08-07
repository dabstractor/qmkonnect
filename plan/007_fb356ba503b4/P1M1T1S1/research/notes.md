# Research Notes — P1.M1.T1.S1: AUR `-bin` PKGBUILD

> Create `packaging/linux/aur/PKGBUILD` (binary package) + copied `qmkonnect.install`
> + `.gitignore` + `README.md`. `.SRCINFO`/publication = S2. No Rust/source change.

## 1. Verified current version + tags

- `Cargo.toml`: `version = "0.2.8"`.
- `git tag --list 'v*'` latest: `v0.2.8`.
- ⇒ PKGBUILD `pkgver=0.2.8`, `pkgrel=1`.

## 2. Source PKGBUILD to mirror (`packaging/linux/arch/PKGBUILD`)

```
pkgname=qmkonnect
pkgver=0.2.8 / pkgrel=1 / arch=('x86_64')
url="https://github.com/dabstractor/qmkonnect" / license=('MIT')
depends=('systemd' 'hidapi' 'libusb' 'zenity' 'libnotify')
makedepends=('cargo' 'rust' 'libx11' 'libxcb' 'systemd-libs' 'pkg-config')   ← DROP for -bin
backup=("usr/lib/systemd/user/qmkonnect.service.template")
install=qmkonnect.install
options=(!strip)
```
Install destinations (the four paths `qmkonnect.install` hooks depend on):
- `/usr/bin/qmkonnect`
- `/usr/lib/udev/qmkonnect-hid-id`
- `/usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules`
- `/usr/lib/systemd/user/qmkonnect.service.template`

NOTE: source PKGBUILD builds in-place (`cd "$srcdir/.."` + `../../../target/release/...`,
no `source=` array). The `-bin` package must NOT copy this — it has a real `source=()`
and extracts into `$srcdir/qmkonnect-$pkgver-linux-x86_64/`.

## 3. CI tarball layout (`.github/workflows/release.yml` `linux-binary` job)

```
VER = cargo metadata version (0.2.8)
STAGE = qmkonnect-${VER}-linux-x86_64
mkdir -p $STAGE/udev $STAGE/systemd
cp target/release/qmkonnect                     $STAGE/
cp target/release/qmkonnect-hid-id              $STAGE/
cp packaging/linux/udev/69-qmkonnect-rawhid.rules       $STAGE/udev/
cp packaging/linux/systemd/qmkonnect.service.template   $STAGE/systemd/
tar czf ${STAGE}.tar.gz $STAGE
```
⇒ Tarball top dir = `qmkonnect-${pkgver}-linux-x86_64/`, containing:
- `qmkonnect`
- `qmkonnect-hid-id`
- `udev/69-qmkonnect-rawhid.rules`
- `systemd/qmkonnect.service.template`

URL: `https://github.com/dabstractor/qmkonnect/releases/download/v${pkgver}/qmkonnect-${pkgver}-linux-x86_64.tar.gz`

## 4. ⭐ Verified sha256 of the v0.2.8 tarball (downloaded + sha256sum'd this session)

```
86dcaa57254f4b7dd23595ef8e473432c3997df3b6c12621913d215b9ab1b216
```
Command used: `curl -sL '<url>' | sha256sum`. The v0.2.8 release asset EXISTS.
⇒ The PKGBUILD uses the REAL hash (not SKIP). Refresh after a version bump via
`updpkgsums` (pacman-contrib) or `makepkg -g` (paste printed line).

## 5. AUR channel requirements (`external_deps.md` §1)

- Package type: `-bin`.
- Key files: PKGBUILD, `.SRCINFO` (S2), optional `.install` (copied).
- `source=()` must point to GitHub release artifacts with `$pkgver`. ✓
- `sha256sums` populated (or SKIP) — real value known. ✓
- `.SRCINFO` via `makepkg --printsrcinfo > .SRCINFO` — S2's scope.
- Publication: `git push aur.archlinux.org:qmkonnect-bin.git` — S2/CI (P1.M5.T1.S1).

## 6. Validation tooling on this host

- `makepkg`: **INSTALLED** (`/usr/bin/makepkg`). ⇒ Real gates:
  - `makepkg --printsrcinfo` (parses PKGBUILD → .SRCINFO-shaped dump; offline; requires qmkonnect.install present).
  - `makepkg -g` (downloads the source, prints the sha256sums line — must match `86dcaa…`).
- `namcap`: **NOT installed**. Document as optional; the makepkg gates are the validation.
- makepkg refuses to run as root; run as `dustin`. Creates `pkg/`, `src/`, downloads the tarball → hence the `.gitignore`.

## 7. Reference `.SRCINFO` shape (from the SOURCE pkgbuild, proves makepkg works)

`cd packaging/linux/arch && makepkg --printsrcinfo | head`:
```
pkgbase = qmkonnect
	pkgdesc = A notification daemon for QMK keyboards
	pkgver = 0.2.8
	pkgrel = 1
	url = https://github.com/dabstractor/qmkonnect
	install = qmkonnect.install
	arch = x86_64
	...
```
The `-bin` package's `--printsrcinfo` should be analogous with `pkgbase = qmkonnect-bin`.

## 8. copy-vs-symlink decision for qmkonnect.install

- AUR repos are flat: `PKGBUILD` + `.SRCINFO` + any `install=`-referenced file at the repo root.
- A symlink (`aur/qmkonnect.install -> ../arch/qmkonnect.install`) would NOT resolve in the published
  AUR repo, nor for a user who clones it. ⇒ **COPY** byte-for-byte.
- Trade-off: drift if `arch/qmkonnect.install` changes. Mitigation: README documents that hooks edits
  must be re-synced to `aur/`; CI (P1.M5.T1.S1) can re-copy on publish.

## 9. Files to create (S1 deliverables)

| File | Content |
|---|---|
| `packaging/linux/aur/PKGBUILD` | What (a) — `-bin` PKGBUILD (verbatim in PRP). |
| `packaging/linux/aur/qmkonnect.install` | byte-identical copy of `packaging/linux/arch/qmkonnect.install`. |
| `packaging/linux/aur/.gitignore` | `pkg/`, `src/`, `*.pkg.tar.*`, `qmkonnect-*-linux-x86_64.tar.gz`, `.SRCINFO` (S2 owns the committed .SRCINFO). |
| `packaging/linux/aur/README.md` | Mode-A: `-bin` convention, `yay -S qmkonnect-bin`, install paths, checksum-refresh workflow, relationship to source PKGBUILD. |

## 10. Scope boundaries (NOT S1)

- `.SRCINFO` generation + AUR repo publication → P1.M1.T1.S2.
- CI publication job (bump pkgver, refresh sha256, regen .SRCINFO, push) → P1.M5.T1.S1.
- PACKAGING.md §4 update (mention `-bin`) → P1.M6.T2.S2.
- Nix flake (P1.M1.T2), Homebrew/Scoop/Winget (P1.M2/P1.M3) — other F15 channels.
- No change to: `release.yml`, `Cargo.toml`, `packaging/linux/arch/`, `src/`.
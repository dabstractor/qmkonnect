# qmkonnect-bin — AUR binary package

This directory holds the **Arch User Repository (AUR) binary package** for
QMKonnect, published as [`qmkonnect-bin`](https://aur.archlinux.org/packages/qmkonnect-bin).

## What this is

`qmkonnect-bin` is the **`-bin`** AUR package: it downloads the pre-built GitHub
release tarball (staged by the CI `linux-binary` job in
[`.github/workflows/release.yml`](../../../.github/workflows/release.yml)) and
installs its contents. **No Rust toolchain, no `cargo`, no build dependencies**
are required — the release tarball ships the ready-to-run `qmkonnect` binary and
the `qmkonnect-hid-id` udev helper.

It is the **`-bin` sibling** of the source PKGBUILD at
[`packaging/linux/arch/PKGBUILD`](../arch/PKGBUILD), which builds from source via
`cargo build --release` (with `-lhidapi-hidraw`). Both packages install to the
**same four paths** and reuse the **same pacman hooks** (`qmkonnect.install`), so
the on-disk result and post-install behavior are identical — pick `-bin` for speed
and zero build deps, or the source PKGBUILD for a from-source / `-git` workflow.

### The AUR `-bin` convention

AUR package names encode the source of the bits:

| Suffix / name | Meaning |
|---|---|
| `qmkonnect-bin` | **Pre-built binary release** (this package) — downloads GitHub release artifacts |
| `qmkonnect` (no suffix) | Build **from source** via the PKGBUILD in `packaging/linux/arch/` |
| `qmkonnect-git` | Build **from the latest git `master`** (not provided here) |

## Install

```bash
# With an AUR helper:
yay -S qmkonnect-bin          # or: paru -S qmkonnect-bin

# Manually (clone the published AUR repo):
git clone https://aur.archlinux.org/qmkonnect-bin.git
cd qmkonnect-bin
makepkg -si
```

The pacman hooks (`qmkonnect.install`) run automatically on install/upgrade:

1. Instantiate the systemd user service from the shipped template.
2. Reload + trigger udev (loads the static `69-qmkonnect-rawhid.rules`).
3. Enable the user service globally (starts at login once a matching device appears).

Default QMK keyboards then need **no configuration**: QMKonnect auto-discovers them
by the standard Raw HID usage page (`0xFF60` / `0x61`), and the shipped static udev
rule already grants permissions.

## What it installs

| Path | What |
|---|---|
| `/usr/bin/qmkonnect` | The daemon binary |
| `/usr/lib/udev/qmkonnect-hid-id` | udev helper tagging QMK Raw HID interfaces |
| `/usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules` | Static usage-page udev rule |
| `/usr/lib/systemd/user/qmkonnect.service.template` | systemd user service template (instantiated by `post_install`) |

These are identical to the source package's install paths (see
[`spec/PACKAGING.md`](../../../spec/PACKAGING.md) §4).

## Version & checksum maintenance

The `sha256sums` in `PKGBUILD` is the verified hash of the `v0.2.8` release
tarball. **It is invalidated by every version bump.** After a new release:

```bash
# In this directory, after bumping pkgver in PKGBUILD:
updpkgsums                    # from pacman-contrib: refreshes sha256sums in-place
# — or manually:
makepkg -g >> PKGBUILD        # prints the new sha256sums line; paste it over the old one

# Regenerate the package index (committed to the AUR repo):
makepkg --printsrcinfo > .SRCINFO
```

CI automates this on each tagged release (the publication job bumps `pkgver` from
cargo metadata, refreshes the checksum, regenerates `.SRCINFO`, and pushes
`PKGBUILD` + `.SRCINFO` + `qmkonnect.install` to the AUR repo).

## Notes

- **`qmkonnect.install` is a verbatim copy** of `packaging/linux/arch/qmkonnect.install`.
  AUR repos are flat, so a symlink would not survive publication or resolve for anyone
  cloning the AUR repo. If the source hooks file changes, re-sync this copy.
- **`.SRCINFO` is generated**, not hand-edited. It is listed in `.gitignore` here
  because publication infra (a later sibling task) owns the committed copy.
- `namcap` (the Arch PKGBUILD linter) is optional and not installed on all hosts;
  run `namcap PKGBUILD` if available.

## Manual AUR publication

`publish.sh` is the one-shot publication script. It bumps `pkgver`, refreshes the
sha256 from the actual release tarball, regenerates `.SRCINFO`, and pushes the
flat trio (`PKGBUILD` + `.SRCINFO` + `qmkonnect.install`) to the AUR git remote.

### Prerequisite: SSH deploy key

The AUR supports **SSH-key auth ONLY** (no token/password). Register the PUBLIC
half of your key at <https://aur.archlinux.org> → *My Account* → *SSH Public Key*.
For CI, store the PRIVATE key as a GitHub Actions secret; the CI job
(P1.M5.T1.S1) loads it into `ssh-agent` before running `publish.sh`.

### One-shot publish

```bash
./publish.sh 0.2.8      # publish qmkonnect-bin v0.2.8
```

Patches `pkgver=0.2.8` in `PKGBUILD`, downloads the release tarball via
`makepkg -g` to refresh `sha256sums`, regenerates `.SRCINFO`, then clones
`aur@aur.archlinux.org:qmkonnect-bin.git` to a temp dir, copies in the flat
trio, commits, and pushes. Re-running the same version is a clean no-op (the
script detects no staged change and reports "already at v…").

### Dry run

```bash
./publish.sh --dry-run 0.2.8   # local steps only; skip the SSH push
```

Runs the `pkgver`/sha256/`.SRCINFO` regeneration locally without an SSH key —
use it to sanity-check the regeneration before a real publish.

### Ordering

**Publish the GitHub release FIRST.** Step 2 (`makepkg -g`) downloads the
release tarball to compute its sha256 — the release must already exist:

1. Tag + publish the GitHub release (the `linux-binary` job stages the tarball).
2. `./publish.sh <version>` (it downloads the tarball, refreshes the checksum,
   regenerates `.SRCINFO`, pushes to the AUR).

### Source-repo sync

`publish.sh` edits `packaging/linux/aur/{PKGBUILD,.SRCINFO}` **in place** in the
qmkonnect SOURCE repo. Commit those two files back here too so the source repo
stays in sync with the AUR — the script does NOT auto-commit the source repo
(no surprise commits).

### Published package

- AUR package: <https://aur.archlinux.org/packages/qmkonnect-bin>
- AUR git remote: `aur@aur.archlinux.org:qmkonnect-bin.git` (flat repo:
  `PKGBUILD` + `.SRCINFO` + `qmkonnect.install` at the root)

> CI automation (P1.M5.T1.S1) wraps this script: on a tag, it extracts the
> version via `cargo metadata | jq`, loads the AUR SSH key from a secret into
> `ssh-agent`, runs `publish.sh <version>`, then commits the updated
> `aur/PKGBUILD` + `aur/.SRCINFO` back to the source repo. `publish.sh` is the
> reusable unit; CI is the trigger.
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
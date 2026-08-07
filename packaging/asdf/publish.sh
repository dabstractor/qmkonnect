#!/usr/bin/env bash
# Sync packaging/asdf/ (the asdf-qmkonnect plugin) into the dabstractor/asdf-qmkonnect
# GitHub repo so `asdf plugin add qmkonnect …` and `mise plugin add qmkonnect …` work.
#
# Usage:
#   ./publish.sh <version>            # sync into asdf-qmkonnect, commit, push
#   ./publish.sh --dry-run <version>  # clone + stage + (local) commit, but SKIP the push
#   ./publish.sh 0.2.8
#   ASDF_QMKONNECT_REMOTE=file:///tmp/fake.git ./publish.sh --dry-run 0.2.8   # local mock test
#
# What it syncs (the plugin repo = packaging/asdf MINUS this publish.sh):
#   bin/list-all, bin/download, bin/install   (P1.M4.T1.S1 plugin scripts)
#   lib/utils.bash                            (P1.M4.T1.S1 shared helpers)
#   README.md                                 (P1.M4.T1.S1)
#   .tool-versions, mise.toml, CHANGELOG.md   (P1.M4.T1.S2 — this task's metadata)
#
# The <version> arg (a) goes in the commit message and (b) is STAMPED into the cloned
# .tool-versions / mise.toml examples so the published examples reflect this release. asdf
# itself resolves versions from the GitHub Releases API at runtime (bin/list-all), so there
# is no version/hash to patch in the scripts — this is a pure file-sync.
#
# DEPLOY KEY (CI publishing — wired in P1.M5.T2.S2):
#   The plugin repo dabstractor/asdf-qmkonnect is pushed via a GitHub deploy key (SSH, write
#   access). Generate a key pair, add the PUBLIC half to the plugin repo (Settings → Deploy
#   keys), and store the PRIVATE half as the ASDF_PLUGIN_DEPLOY_KEY Actions secret in
#   dabstractor/qmkonnect. CI loads it into ssh-agent, then runs this script on each release
#   tag. (Mirrors the AUR SSH-key model — packaging/linux/aur/publish.sh — and the Homebrew
#   tap model — packaging/homebrew/update-cask.sh.)
#
# Prerequisites:
#   * The dabstractor/asdf-qmkonnect repo must PRE-EXIST on GitHub (empty is fine — create it
#     once at github.com/new; do NOT add a README so the first push is clean). git push to a
#     non-existent repo fails with "Repository not found".
#   * `git`, `cp`, `chmod`, `sed` on PATH (all standard). An SSH key in ssh-agent for the
#     real remote (the local mock uses a file:// remote, so no key needed).
#   * P1.M4.T1.S1's plugin files (bin/, lib/, README.md) must be present under packaging/asdf/.
#
# This script does NOT modify any source file; it only writes inside a temp clone. CI
# integration (secret loading + the release-tag trigger + the 'asdf plugin test' job) is
# P1.M5.T2.S2.
set -euo pipefail

DRY_RUN=0
VERSION=""
for a in "$@"; do
    case "$a" in
        --dry-run|-n) DRY_RUN=1 ;;
        -h|--help)    sed -n '2,40p' "${BASH_SOURCE[0]}"; exit 0 ;;
        -*)           echo "Unknown option: $a" >&2; exit 1 ;;
        *)            VERSION="$a" ;;
    esac
done

if [ -z "$VERSION" ]; then
    echo "Usage: $0 [--dry-run] <version>   (e.g. 0.2.8)" >&2
    exit 1
fi
# Versions are bare (no leading 'v'); only the git TAG is v-prefixed.
case "$VERSION" in
    v*) echo "ERROR: version must not have a leading 'v' (got '$VERSION'). Use '${VERSION#v}'." >&2; exit 1 ;;
esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC="$SCRIPT_DIR"   # packaging/asdf (this script lives here too, but is never copied)
REMOTE="${ASDF_QMKONNECT_REMOTE:-git@github.com:dabstractor/asdf-qmkonnect.git}"

# The plugin scripts + README must exist (P1.M4.T1.S1). Fail fast with a clear message.
for f in bin/list-all bin/download bin/install lib/utils.bash README.md; do
    [ -f "$SRC/$f" ] || { echo "ERROR: required plugin file missing: $SRC/$f (is P1.M4.T1.S1 landed?)" >&2; exit 1; }
done
for f in .tool-versions mise.toml CHANGELOG.md; do
    [ -f "$SRC/$f" ] || { echo "ERROR: required metadata file missing: $SRC/$f (P1.M4.T1.S2)" >&2; exit 1; }
done

# NOTE: use a numeric test (not `${DRY_RUN:+…}`) — that expansion treats `0` as
# set-and-non-empty and would falsely print "(dry-run)" on a real push.
DRY_TAG=""
[ "$DRY_RUN" -eq 1 ] && DRY_TAG=" (dry-run)"
echo "==> Publishing asdf-qmkonnect v${VERSION}${DRY_TAG}"
echo "    remote: $REMOTE"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# 1. Clone the plugin repo (it must pre-exist; an empty repo clones to an empty working tree).
git clone "$REMOTE" "$WORK/plugin"

# 2. Sync the plugin files + metadata INTO the clone (never copy publish.sh itself).
mkdir -p "$WORK/plugin/bin" "$WORK/plugin/lib"
cp "$SRC/bin/list-all" "$SRC/bin/download" "$SRC/bin/install" "$WORK/plugin/bin/"
cp "$SRC/lib/utils.bash" "$WORK/plugin/lib/"
cp "$SRC/README.md" "$SRC/.tool-versions" "$SRC/mise.toml" "$SRC/CHANGELOG.md" "$WORK/plugin/"

# 3. CRITICAL: guarantee the executable bit. cp to an EXISTING file preserves the old (possibly
#    non-executable) mode, so a re-publish could leave bin/* as 100644 → 'asdf install' fails.
#    .gitattributes cannot set the exec bit; this chmod is mandatory.
chmod +x "$WORK/plugin/bin/list-all" "$WORK/plugin/bin/download" "$WORK/plugin/bin/install"

# 4. Stamp the version into the published example files (the source copies keep their default).
#    Full-line patterns; comment lines start with '#'/whitespace so they never match.
sed -i.bak -E "s/^qmkonnect .*/qmkonnect ${VERSION}/" "$WORK/plugin/.tool-versions" && rm -f "$WORK/plugin/.tool-versions.bak"
sed -i.bak -E "s/^qmkonnect = \".*\"/qmkonnect = \"${VERSION}\"/" "$WORK/plugin/mise.toml" && rm -f "$WORK/plugin/mise.toml.bak"

# 5. Ensure a git identity exists in the clone (CI runners often lack one → 'git commit' fails).
if ! git -C "$WORK/plugin" config user.email >/dev/null 2>&1; then
    git -C "$WORK/plugin" config user.email "qmkonnect-bot@users.noreply.github.com"
    git -C "$WORK/plugin" config user.name  "QMKonnect release automation"
fi

# 6. Stage + verify the executable bit landed in the index (100755, NOT 100644).
git -C "$WORK/plugin" add -A
for b in bin/list-all bin/download bin/install; do
    mode="$(git -C "$WORK/plugin" ls-files -s "$b" | awk '{print $1}')"
    [ "$mode" = "100755" ] || { echo "ERROR: $b is $mode in the index (expected 100755); chmod +x did not take." >&2; exit 1; }
done

# 7. Idempotent commit + push (skip push under --dry-run).
if git -C "$WORK/plugin" diff --cached --quiet; then
    echo "==> asdf-qmkonnect already at v${VERSION}; nothing to commit."
else
    git -C "$WORK/plugin" commit -m "asdf-qmkonnect v${VERSION}" -m "Sync plugin scripts + metadata from dabstractor/qmkonnect@v${VERSION}."
fi

if [ "$DRY_RUN" -eq 1 ]; then
    echo "==> Dry-run: skipping push. Staged tree:"
    git -C "$WORK/plugin" ls-files -s | sed 's/^/    /'
    exit 0
fi

# Push to the repo's default branch (origin HEAD = main for dabstractor/asdf-qmkonnect).
# HEAD:main works for both the first publish (empty repo) and subsequent updates.
DEFAULT_BRANCH="$(git -C "$WORK/plugin" symbolic-ref --short HEAD 2>/dev/null || echo main)"
git -C "$WORK/plugin" push origin "HEAD:${DEFAULT_BRANCH}"
echo "==> Published asdf-qmkonnect v${VERSION} to ${DEFAULT_BRANCH}."
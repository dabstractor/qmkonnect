#!/usr/bin/env bash
# Update the QMKonnect Homebrew cask (Casks/qmkonnect.rb) for a new release.
#
# Usage:
#   ./update-cask.sh <version>            # download the release DMG, compute SHA256, patch + audit
#   ./update-cask.sh <version> <sha256>   # use a pre-computed SHA256 (skip the download)
#   ./update-cask.sh --help
#
# Example:
#   ./update-cask.sh 0.2.8
#   ./update-cask.sh 0.2.8 86dcaa57254f4b7dd23595ef8e473432c3997df3b6c12621913d215b9ab1b216
#
# Steps:
#   1. (unless a sha256 arg is given) download QMKonnect-<version>-macos.dmg from the GitHub
#      release and compute its SHA256 (`shasum -a 256`, falling back to `sha256sum`).
#   2. Patch the `version` and `sha256` lines in Casks/qmkonnect.rb (2-space-indented stanzas).
#   3. Validate with `brew audit --cask --new-cask` if `brew` is available (best-effort).
#
# This script is a PURE local file update — it does NOT push to the tap repo. The actual tap
# publication is the CI job P1.M5.T1.S2, which runs this script against the source checkout,
# copies the patched cask into a clone of dabstractor/homebrew-qmkonnect, and pushes.
#
# DEPLOY KEY (CI publishing — wired in P1.M5.T1.S2):
#   The tap repo dabstractor/homebrew-qmkonnect is pushed via a GitHub deploy key (SSH, write
#   access). Generate a key pair, add the PUBLIC half to the tap repo (Settings → Deploy keys),
#   and store the PRIVATE half as the HOMEBREW_TAP_DEPLOY_KEY Actions secret in dabstractor/qmkonnect.
#   CI loads it into ssh-agent, then: git clone git@github.com:dabstractor/homebrew-qmkonnect.git,
#   run this script, cp Casks/qmkonnect.rb into the clone, commit, push. (Mirrors the AUR SSH-key
#   model — see packaging/linux/aur/publish.sh + architecture/external_deps.md §"CI Publishing Strategy".)
#
# Prerequisites:
#   * The GitHub release for <version> must already be published (step 1 downloads the DMG).
#   * `curl`, `awk`, and (`shasum` | `sha256sum`) must be on PATH. `brew` is optional (audit is
#     best-effort). `ruby` is needed only if you additionally run `ruby -c` yourself.
#   * Portable across macOS (BSD sed; ships shasum, NOT sha256sum) and Linux (GNU sed; sha256sum).
set -euo pipefail

CASK_REPO="dabstractor/qmkonnect"
TAP_REPO="dabstractor/homebrew-qmkonnect"

usage() { sed -n '2,40p' "${BASH_SOURCE[0]:-$0}"; }

VERSION=""
SHA256=""
for a in "$@"; do
    case "$a" in
        -h|--help) usage; exit 0 ;;
        -*) echo "Unknown option: $a" >&2; exit 1 ;;
        *)
            if [ -z "$VERSION" ]; then VERSION="$a"
            elif [ -z "$SHA256" ]; then SHA256="$a"
            else echo "Unexpected extra argument: $a" >&2; exit 1
            fi
            ;;
    esac
done

if [ -z "$VERSION" ]; then
    echo "Usage: $0 <version> [sha256]   (e.g. 0.2.8)" >&2
    exit 1
fi

# Reject version strings with a leading 'v' (release TAGS are v-prefixed; cask versions are not).
case "$VERSION" in
    v*) echo "ERROR: version must not have a leading 'v' (got '$VERSION'). Use '${VERSION#v}'." >&2; exit 1 ;;
esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CASK="$SCRIPT_DIR/Casks/qmkonnect.rb"
[ -f "$CASK" ] || { echo "ERROR: cask not found at $CASK" >&2; exit 1; }

DMG_URL="https://github.com/${CASK_REPO}/releases/download/v${VERSION}/QMKonnect-${VERSION}-macos.dmg"

# --- Step 1: obtain the SHA256 (download+hash, or use the provided arg) ---
if [ -n "$SHA256" ]; then
    echo "==> Using provided SHA256 for v${VERSION} (skipping download)"
else
    echo "==> Downloading ${DMG_URL}"
    WORK="$(mktemp -d)"
    trap 'rm -rf "$WORK"' EXIT
    DMG="$WORK/QMKonnect-${VERSION}-macos.dmg"
    curl -fL "$DMG_URL" -o "$DMG"
    if command -v shasum >/dev/null 2>&1; then
        SHA256="$(shasum -a 256 "$DMG" | awk '{print $1}')"
    elif command -v sha256sum >/dev/null 2>&1; then
        SHA256="$(sha256sum "$DMG" | awk '{print $1}')"
    else
        echo "ERROR: need 'shasum' or 'sha256sum' on PATH" >&2; exit 1
    fi
fi

# Sanity: a SHA256 is exactly 64 lowercase hex chars. (case globs don't support {n}
# quantifiers, so we validate length explicitly + the char class.)
if [ "${#SHA256}" -ne 64 ] || ! printf '%s' "$SHA256" | grep -qE '^[0-9a-f]{64}$'; then
    echo "ERROR: sha256 looks malformed: '$SHA256'" >&2; exit 1
fi
echo "    version -> ${VERSION}"
echo "    sha256  -> ${SHA256}"

# --- Step 2: patch version + sha256 in the cask (BSD+GNU sed portable) ---
# S1's cask stanzas are 2-space-indented inside `cask "qmkonnect" do … end`:
#     version "0.2.8"
#     sha256 :no_check
patch_line() {  # <regex> <replacement> <file>
    sed -i.bak -E -e "s|${1}|${2}|" "$3" && rm -f "$3.bak"
}
patch_line '^  version .*$'  "  version \"${VERSION}\""  "$CASK"
patch_line '^  sha256 .*$'   "  sha256 \"${SHA256}\""    "$CASK"

# Confirm both stanzas landed (guards against a future stanza reorder silently no-oping the sed).
grep -q "^  version \"${VERSION}\"" "$CASK" || { echo "ERROR: version stanza not patched" >&2; exit 1; }
grep -q "^  sha256 \"${SHA256}\""   "$CASK" || { echo "ERROR: sha256 stanza not patched" >&2; exit 1; }
echo "    patched $CASK"

# --- Step 3: best-effort `brew audit` (macOS/Linuxbrew only) ---
if command -v brew >/dev/null 2>&1; then
    echo "==> brew audit --cask --new-cask $CASK"
    brew audit --cask --new-cask "$CASK"
else
    echo "==> (brew not found; skipping audit — run on a macOS/Linuxbrew host)"
fi

echo "==> Done. Next (CI, P1.M5.T1.S2): copy $CASK into a clone of ${TAP_REPO} and push."
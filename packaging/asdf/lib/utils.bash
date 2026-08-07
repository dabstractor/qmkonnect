#!/usr/bin/env bash
# Shared helpers for the asdf-qmkonnect plugin scripts (bin/list-all, bin/download,
# bin/install). SOURCED by those scripts — NOT a standalone executable. Defines
# functions only; does NOT `set -e` (the caller owns shell options).
#
# These same scripts serve mise unchanged (mise is asdf-compatible: it runs the
# plugin's bin/* directly). See packaging/asdf/README.md.
# shellcheck disable=SC2034 # the ASDF_QMKONNECT_* / QMKONNECT_* globals are documented identity/diagnostic constants (single source of truth; callers may read them)

# ── QMKonnect release identity (single source of truth) ──────────────────────
# Documented identity const; the API + release-base consts below derive from this org/repo.
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
        # PLATFORM_OK is a documented diagnostic global callers may read.
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
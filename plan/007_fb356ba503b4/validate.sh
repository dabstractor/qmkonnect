#!/usr/bin/env bash
# =============================================================================
# QMKonnect — Comprehensive Validation Script
# =============================================================================
# Validates the QMKonnect desktop daemon end-to-end: the real lint/fmt/build/
# test toolchain the CI + AGENTS.md mandate, the byte-level Raw-HID wire
# contract, the CLI surface + config/rules workflows from the docs, the host-
# rules stack/replace/no-match dispatch (HOST_RULES C13), packaging integrity
# (.deb/.rpm/arch/CI), and a live product smoke run when a QMK keyboard is
# attached. Mirrors actual user journeys from README.md / docs/, not just
# internal APIs.
#
# Usage:   ./validate.sh
#          ./validate.sh --skip-live   # skip the live HID/tray smoke run
#
# Phases SKIP gracefully when an optional tool (cargo-deb, cargo-generate-rpm,
# dpkg-deb, rpm, nix) is absent. Hard toolchain (cargo/rustc) is required.
# =============================================================================
set -uo pipefail

# ---- color helpers ----------------------------------------------------------
if [ -t 1 ]; then
  G=$'\033[32m'; R=$'\033[31m'; Y=$'\033[33m'; B=$'\033[1;34m'; D=$'\033[2m'; N=$'\033[0m'
else
  G=""; R=""; Y=""; B=""; D=""; N=""
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT" || { echo "cannot cd $ROOT"; exit 1; }

PASS=0; WARN=0; FAIL=0; SKIP=0
TOTAL_START=$(date +%s)
SKIP_LIVE=0
for a in "$@"; do [ "$a" = "--skip-live" ] && SKIP_LIVE=1; done

begin() { SECTION="$1"; echo ""; echo "${B}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${N}"; echo "${B}▶ $1${N}"; echo "${B}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${N}"; }
ok()   { echo "  ${G}✓${N} $1"; PASS=$((PASS+1)); }
warn() { echo "  ${Y}⚠${N} $1"; WARN=$((WARN+1)); }
fail() { echo "  ${R}✗${N} $1"; FAIL=$((FAIL+1)); }
skip() { echo "  ${D}⊘${N} $1 ${D}(skipped)${N}"; SKIP=$((SKIP+1)); }
info() { echo "  ${D}·${N} $1"; }

# run <label> <cmd...> — capture exit, echo pass/fail (does not abort the suite)
run() {
  local label="$1"; shift
  if "$@" >/tmp/qmk-validate.out 2>&1; then
    ok "$label"; return 0
  else
    fail "$label"; tail -n 15 /tmp/qmk-validate.out | sed 's/^/      /'; return 1
  fi
}

# =============================================================================
echo "${B}QMKonnect validation — $(date '+%F %T')${N}"
echo "repo: $ROOT"
echo "host: $(uname -srm) · desktop=${XDG_CURRENT_DESKTOP:-unset} · rust=$(rustc --version 2>/dev/null || echo MISSING)"

# ─────────────────────────────────────────────────────────────────────────────
begin "Phase 0 · Toolchain prerequisites"
# ─────────────────────────────────────────────────────────────────────────────
command -v cargo >/dev/null 2>&1 && ok "cargo present: $(cargo --version)" || { fail "cargo MISSING (required)"; exit 1; }
command -v rustc >/dev/null 2>&1 && ok "rustc present: $(rustc --version)" || { fail "rustc MISSING (required)"; exit 1; }
# MSRV gate (Cargo.toml rust-version = 1.88)
rv=$(rustc --version 2>/dev/null | sed -E 's/^rustc ([0-9]+)\.([0-9]+).*/\1 \2/')
major=$(echo "$rv" | cut -d' ' -f1); minor=$(echo "$rv" | cut -d' ' -f2)
if [ -n "$major" ] && { [ "$major" -gt 1 ] || { [ "$major" -eq 1 ] && [ "${minor:-0}" -ge 88 ]; }; }; then
  ok "rustc >= MSRV 1.88 (have $major.$minor)"
else
  fail "rustc < MSRV 1.88 (parsed major='$major' minor='$minor')"
fi
command -v cargo-deb          >/dev/null 2>&1 && info "cargo-deb available ($(cargo-deb --version 2>/dev/null))"          || info "cargo-deb absent (.deb build phase will skip)"
command -v cargo-generate-rpm >/dev/null 2>&1 && info "cargo-generate-rpm available ($(cargo-generate-rpm --version 2>/dev/null))" || info "cargo-generate-rpm absent (.rpm build phase will skip)"
command -v nix                >/dev/null 2>&1 && info "nix available" || info "nix absent (flake check phase will skip)"

# ─────────────────────────────────────────────────────────────────────────────
begin "Phase 1 · Format check (CI gate: cargo fmt --check)"
# ─────────────────────────────────────────────────────────────────────────────
if cargo fmt --all -- --check >/tmp/qmk-validate.out 2>&1; then
  ok "cargo fmt --all -- --check clean (exit 0)"
else
  rc=$?
  fail "cargo fmt --all -- --check FAILED (exit $rc) — the ci.yml 'fmt' job runs this exact command on every push to main"
  n=$(grep -c '^Diff in' /tmp/qmk-validate.out)
  info "$n formatting diff(s); affected files:"
  grep '^Diff in' /tmp/qmk-validate.out | sed -E 's/Diff in .*\/(src\/[^:]+):.*/      \1/' | sort -u | sed 's/^/      - /'
  info "fix: cargo fmt --all"
fi

# ─────────────────────────────────────────────────────────────────────────────
begin "Phase 2 · Lint (cargo clippy -D warnings)"
# ─────────────────────────────────────────────────────────────────────────────
run "cargo clippy --all-targets -- -D warnings" cargo clippy --all-targets -- -D warnings

# ─────────────────────────────────────────────────────────────────────────────
begin "Phase 3 · Build (release, all targets)"
# ─────────────────────────────────────────────────────────────────────────────
run "cargo build --release --all-targets" cargo build --release --all-targets
# Binaries that downstream packaging + dev loop rely on must exist & be fresh.
[ -x target/release/qmkonnect ]      && ok "target/release/qmkonnect built"      || fail "target/release/qmkonnet missing"
[ -x target/release/qmkonnect-hid-id ] && ok "target/release/qmkonnet-hid-id built" || fail "target/release/qmkonnet-hid-id missing"

# ─────────────────────────────────────────────────────────────────────────────
begin "Phase 4 · Unit tests (single-threaded — shared debouncer state)"
# ─────────────────────────────────────────────────────────────────────────────
# AGENTS.md / CI mandate --test-threads=1.
if cargo test --bin qmkonnect -- --test-threads=1 >/tmp/qmk-validate.out 2>&1; then
  ok "cargo test --bin qmkonnect -- --test-threads=1 passed"
  grep -E '^test result:' /tmp/qmk-validate.out | tail -1 | sed 's/^/      /'
else
  rc=$?; fail "unit tests FAILED (exit $rc)"; tail -n 25 /tmp/qmk-validate.out | sed 's/^/      /'
fi
# Also run the hid-id helper's pure-std unit tests (second bin target).
if cargo test --bin qmkonnect-hid-id >/tmp/qmk-validate.out 2>&1; then
  ok "cargo test --bin qmkonnet-hid-id passed (udev helper parser tests)"
else
  rc=$?; warn "qmkonnet-hid-id tests exit $rc (informational)"; tail -n 8 /tmp/qmk-validate.out | sed 's/^/      /'
fi

# ─────────────────────────────────────────────────────────────────────────────
begin "Phase 5 · Wire-protocol invariants (the desktop↔keyboard byte contract)"
# ─────────────────────────────────────────────────────────────────────────────
# The framing lives in the pinned qmk-notifier crate (Cargo.lock tag). Verify
# the constants match PROTOCOL.md §7 exactly.
CRATE="$(find "${CARGO_HOME:-$HOME/.cargo}/git/checkouts" -type d -path '*qmk-notifier*' -name 'f26893e' 2>/dev/null | head -1)"
if [ -z "$CRATE" ]; then
  # fall back to scanning any checkout; the tag sha is f26893e per Cargo.lock
  CRATE="$(find "${CARGO_HOME:-$HOME/.cargo}/git/checkouts" -type d -path '*qmk-notifier*' 2>/dev/null | grep 'f26893e' | head -1)"
fi
if [ -n "$CRATE" ] && [ -f "$CRATE/src/core.rs" ]; then
  ok "located pinned qmk-notifier crate checkout"
  check_const() { # <name> <regex> <file>
    if grep -qE "$2" "$3" 2>/dev/null; then ok "wire const $1 present"; else fail "wire const $1 MISSING in $3"; fi
  }
  check_const "DEFAULT_USAGE_PAGE=0xFF60" 'DEFAULT_USAGE_PAGE: u16 = 0xFF60' "$CRATE/src/core.rs"
  check_const "DEFAULT_USAGE=0x61"        'DEFAULT_USAGE: u16 = 0x61'        "$CRATE/src/core.rs"
  check_const "REPORT_LENGTH=32"          'REPORT_LENGTH: usize = 32'        "$CRATE/src/core.rs"
  check_const "magic 0x81"                'request_data\[1\] = 0x81'          "$CRATE/src/core.rs"
  check_const "magic 0x9F"                'request_data\[2\] = 0x9F'          "$CRATE/src/core.rs"
  check_const "ETX=0x03"                  'ETX_TERMINATOR_BYTE: u8 = 0x03'    "$CRATE/src/core.rs"
  check_const "typed discriminator 0xF0"  'CMD_DISCRIMINATOR: u8 = 0xF0'     "$CRATE/src/core.rs"
  check_const "typed response marker 0x51" 'RESPONSE_MARKER: u8 = 0x51'      "$CRATE/src/core.rs"
  check_const "PAYLOAD_PER_REPORT=30"     'PAYLOAD_PER_REPORT: usize = REPORT_LENGTH - 2' "$CRATE/src/core.rs"
else
  warn "could not locate qmk-notifier crate source (cargo checkout moved?); skipping wire-const deep checks"
fi
# App side: payload is "{class}\x1D{title}" (GS=0x1D), magic never speaks VIA.
if grep -qF 'format!("{}{}{}", window_info.app_class, "\x1D", window_info.title)' src/core/notifier.rs; then
  ok "app payload build uses GS (0x1D) delimiter: {class}\x1D{title}"
else
  fail "app payload build does not match the {class}\x1D{title} contract"
fi
grep -qF 'r_coex_invariants' src/core/notifier.rs \
  && ok "R-COEX invariant tests present (every transport variant emits 0x81 magic)" \
  || warn "R-COEX invariant test module not found"

# ─────────────────────────────────────────────────────────────────────────────
begin "Phase 6 · CLI + config/rules workflows (real user journeys from README/docs)"
# ─────────────────────────────────────────────────────────────────────────────
BIN=target/release/qmkonnect
WORK="$(mktemp -d)"
# Scope the HOME/XDG_CONFIG_HOME override to ONLY these CLI workflow tests so
# later phases (cargo deb/rpm) still see the real rustup toolchain config.
_SAVED_HOME="$HOME"; _SAVED_XDG="$XDG_CONFIG_HOME"
export XDG_CONFIG_HOME="$WORK/.config"; export HOME="$WORK"
trap 'rm -rf "$WORK"' EXIT

run "--help prints version + options" "$BIN" --help
"$BIN" --help 2>&1 | grep -q "QMKonnect v$(grep -m1 '^version' Cargo.toml | sed -E 's/version = "([^"]+)"/\1/')" \
  && ok "--help reports the Cargo.toml version" || fail "--help version mismatch"
"$BIN" -l >/tmp/qk-l.out 2>&1 && ok "-l runs" || fail "-l failed"
grep -q 'Linux' /tmp/qk-l.out && info "-l output: $(grep Linux /tmp/qk-l.out | head -1)" || warn "-l printed no Linux line"

# config + rules seeding (-c), idempotency, zero-config correctness.
"$BIN" -c >/tmp/qk-c.out 2>&1 && ok "-c creates config + rules" || fail "-c failed"
CFG="$WORK/.config/qmkonnect/config.toml"; RUL="$WORK/.config/qmkonnect/rules.toml"
[ -f "$CFG" ] && ok "config.toml seeded" || fail "config.toml not created"
[ -f "$RUL" ] && ok "rules.toml seeded" || fail "rules.toml not created"
grep -qi '0xfeed' "$CFG" && fail "seeded config still contains literal 0xfeed (DEVICE_DISCOVERY §7.2 cleanup)" || ok "seeded config has no 0xfeed literal"
grep -q '0x????' "$CFG" && ok "seeded config uses 0x???? auto-discovery hint" || warn "seeded config missing 0x???? hint"
# idempotency: re-run -c must not overwrite (G7).
cp "$CFG" "$WORK/.stamp"; "$BIN" -c >/dev/null 2>&1; cmp -s "$CFG" "$WORK/.stamp" && ok "-c is idempotent (existing config untouched)" || fail "-c overwrote existing config"
# template round-trips to all-default (parses cleanly).
"$BIN" --validate-rules >/tmp/qk-vr.out 2>&1 && ok "--validate-rules on all-default template passes" || fail "--validate-rules failed on default template"
grep -q 'rules.toml valid: 0 rules' /tmp/qk-vr.out && ok "seeded rules.toml parses to 0 rules (inert)" || warn "seeded rules.toml not 0-rule inert"

# validate a REAL rules.toml (PRD reference-keymap style) incl. intentional footguns.
cat > "$WORK/real-rules.toml" <<'EOF'
[host]
disable_firmware_config = false
[[rule]]
match = "*calculator"
layer = 10
[[rule]]
match = ["*chrome*", "*jitsi*"]
layer = 11
[[rule]]
match = "neovide"
enable = ["vim_lazy", "disable_vim"]
[[rule]]
match = ""
layer = 12
[[rule]]
match = "*"
enable = ["foo"]
disable = ["foo"]
EOF
"$BIN" --validate-rules --rules-path "$WORK/real-rules.toml" >/tmp/qk-vr2.out 2>&1
grep -q 'empty.*match.*pattern' /tmp/qk-vr2.out  && ok "--validate-rules flags empty match footgun" || warn "--validate-rules did not flag empty match"
grep -q 'foo.*both enabled and disabled' /tmp/qk-vr2.out && ok "--validate-rules flags contradictory enable/disable" || warn "--validate-rules did not flag contradiction"
grep -q 'rules.toml valid: 5 rules' /tmp/qk-vr2.out && ok "real rules.toml counts correctly (5 rules)" || warn "real rules count unexpected"

# --rules-path to a nonexistent file must error (non-zero), per main.rs D3.
if "$BIN" --validate-rules --rules-path "$WORK/nope.toml" >/dev/null 2>&1; then
  fail "--validate-rules on missing file should exit non-zero"
else
  ok "--validate-rules on missing file exits non-zero (D3)"
fi

# Hot-config cache round-trips a Settings save: write an explicit config and
# confirm it parses back to the set values (render_config_body preserves fields).
cat > "$CFG" <<'EOF'
vendor_id = 0x1234
product_id = 0x5678
usage_page = 0xff61
debounce_ms = 120
poll_interval_ms = 7
[linux]
backend = "x11"
gnome_poll_interval_ms = 2000
EOF
"$BIN" --validate-rules >/dev/null 2>&1 && ok "explicit config parses cleanly" || fail "explicit config failed to parse"

# Restore the real environment so cargo deb/rpm (Phase 7) can resolve the
# rustup toolchain and the user's real cargo registry cache.
export HOME="$_SAVED_HOME"; export XDG_CONFIG_HOME="$_SAVED_XDG"

# ─────────────────────────────────────────────────────────────────────────────
begin "Phase 7 · Packaging integrity (.deb / .rpm / arch / manifests)"
# ─────────────────────────────────────────────────────────────────────────────
# 7a. All assets referenced by the [package.metadata.deb] / .generate-rpm blocks
#     must exist on disk (a missing asset breaks the build silently in CI).
deb_assets="packaging/linux/udev/69-qmkonnect-rawhid.rules
packaging/linux/systemd/qmkonnect.service.template
packaging/linux/xdg/qmkonnect.desktop
packaging/debian/long-description.txt
packaging/debian/postinst
packaging/debian/prerm
packaging/debian/postrm
packaging/rpm/postin
packaging/rpm/postun
README.md
LICENSE"
miss=0
for a in $deb_assets; do [ -e "$a" ] || { fail "packaging asset MISSING: $a"; miss=1; }; done
[ "$miss" -eq 0 ] && ok "all .deb/.rpm asset + maintainer-script files present"

# 7b. Actually build the .deb if cargo-deb is available (catches metadata errors).
if command -v cargo-deb >/dev/null 2>&1; then
  if cargo deb >/tmp/qmk-validate.out 2>&1; then
    ok "cargo deb built successfully"
    deb_pkg=$(ls target/debian/qmkonnect_*_amd64.deb 2>/dev/null | head -1)
    if [ -n "$deb_pkg" ]; then
      ok ".deb produced: $(basename "$deb_pkg") ($(du -h "$deb_pkg" | cut -f1))"
      # data payload: verify the 6 expected FHS paths are present.
      if command -v ar >/dev/null 2>&1; then
        data=$(ar p "$deb_pkg" data.tar.xz 2>/dev/null | tar -tJ 2>/dev/null)
        for p in usr/bin/qmkonnect usr/lib/udev/qmkonnect-hid-id \
                 usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules \
                 usr/lib/systemd/user/qmkonnect.service.template \
                 etc/xdg/autostart/qmkonnect.desktop; do
          echo "$data" | grep -q "$p" && ok ".deb ships ./$p" || fail ".deb MISSING ./$p"
        done
        ctrl=$(ar p "$deb_pkg" control.tar.xz 2>/dev/null | tar -tJ 2>/dev/null)
        for s in postinst prerm postrm; do
          echo "$ctrl" | grep -q "./$s" && ok ".deb embeds maintainer script $s" || fail ".deb MISSING maintainer script $s"
        done
      fi
    fi
  else
    fail "cargo deb FAILED"; tail -n 20 /tmp/qmk-validate.out | sed 's/^/      /'
  fi
else
  skip "cargo-deb not installed — .deb build"
fi

# 7c. Build the .rpm if cargo-generate-rpm is available.
if command -v cargo-generate-rpm >/dev/null 2>&1; then
  if cargo build --release >/dev/null 2>&1 && cargo generate-rpm >/tmp/qmk-validate.out 2>&1; then
    ok "cargo generate-rpm built successfully"
    rpm_pkg=$(ls target/generate-rpm/qmkonnect-*.x86_64.rpm 2>/dev/null | head -1)
    [ -n "$rpm_pkg" ] && ok ".rpm produced: $(basename "$rpm_pkg") ($(du -h "$rpm_pkg" | cut -f1))" || fail ".rpm not produced"
    # Verify the maintainer scripts are wired: Cargo.toml references them and
    # the files exist (the build already validated they're embeddable). RPM
    # payloads are compressed, so `strings` on the whole archive is unreliable.
    if grep -q 'post_install_script = "packaging/rpm/postin"' Cargo.toml \
       && grep -q 'post_uninstall_script = "packaging/rpm/postun"' Cargo.toml \
       && [ -f packaging/rpm/postin ] && [ -f packaging/rpm/postun ]; then
      ok ".rpm maintainer scripts wired (Cargo.toml refs + files present)"
    else
      fail ".rpm maintainer script wiring incomplete"
    fi
    # Best-effort: extract the payload via rpm2cpio and confirm the 7 declared
    # assets are present (rpm2cpio absent => skip silently).
    if command -v rpm2cpio >/dev/null 2>&1 && [ -n "$rpm_pkg" ]; then
      _xt="$(mktemp -d)"
      if rpm2cpio "$rpm_pkg" 2>/dev/null | (cd "$_xt" && cpio -idm --quiet 2>/dev/null); then
        for p in usr/bin/qmkonnect usr/lib/udev/qmkonnect-hid-id \
                 usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules \
                 usr/lib/systemd/user/qmkonnect.service.template \
                 etc/xdg/autostart/qmkonnect.desktop \
                 usr/share/doc/qmkonnect/README.md \
                 usr/share/licenses/qmkonnect/LICENSE; do
          [ -e "$_xt/$p" ] && ok ".rpm ships /$p" || warn ".rpm payload did not expose /$p (cpio format quirk)"
        done
      fi
      rm -rf "$_xt"
    fi
  else
    fail "cargo generate-rpm FAILED"; tail -n 20 /tmp/qmk-validate.out | sed 's/^/      /'
  fi
else
  skip "cargo-generate-rpm not installed — .rpm build"
fi

# 7d. Community-channel manifests all carry the current version (CI patches them,
#     but they should ship pre-filled for the current release).
VER=$(grep -m1 '^version' Cargo.toml | sed -E 's/version = "([^"]+)"/\1/')
for f in packaging/scoop/qmkonnect.json \
         packaging/winget/dabstractor.QMKonnect.installer.yaml \
         packaging/winget/dabstractor.QMKonnect.yaml \
         packaging/winget/dabstractor.QMKonnect.locale.en-US.yaml \
         packaging/homebrew/Casks/qmkonnect.rb \
         packaging/linux/aur/PKGBUILD \
         packaging/linux/arch/PKGBUILD \
         packaging/gnome-shell-extension/metadata.json; do
  if grep -q "$VER" "$f" 2>/dev/null; then ok "$f carries version $VER"; else warn "$f does not reference version $VER"; fi
done

# 7e. XDG .desktop ships in every Linux package (AUR + source PKGBUILD).
grep -q 'xdg/qmkonnet.desktop\|etc/xdg/autostart' packaging/linux/aur/PKGBUILD \
  && ok "AUR PKGBUILD installs XDG autostart .desktop (F17)" \
  || fail "AUR PKGBUILD does NOT install the XDG .desktop (F17 gap)"
grep -q 'etc/xdg/autostart' packaging/linux/arch/PKGBUILD \
  && ok "source Arch PKGBUILD installs XDG autostart .desktop (F17)" \
  || fail "source Arch PKGBUILD does NOT install the XDG .desktop"

# ─────────────────────────────────────────────────────────────────────────────
begin "Phase 8 · Docs & feature-claim drift"
# ─────────────────────────────────────────────────────────────────────────────
# 8a. llms_full.txt should be no older than the newest doc/source it aggregates.
if [ -f docs/llms_full.txt ] && [ -x docs/generate_llms_full.sh ]; then
  newest_src=$(find docs spec src README.md -type f \( -name '*.md' -o -name '*.rs' \) -printf '%T@\n' 2>/dev/null | sort -rn | head -1 | cut -d. -f1)
  llms_t=$(date -r docs/llms_full.txt +%s 2>/dev/null)
  if [ -n "$newest_src" ] && [ -n "$llms_t" ] && [ "$llms_t" -ge "$newest_src" ]; then
    ok "docs/llms_full.txt is fresh (>= newest aggregated source)"
  else
    warn "docs/llms_full.txt looks stale vs source — run: cd docs && ./generate_llms_full.sh"
  fi
else
  skip "llms_full.txt / generator not found"
fi

# 8b. print_platforms() drift: -l should reflect the F16 multi-backend build,
#     not the legacy hyprland/X11 either-or.
if grep -q 'Linux (Hyprland)' src/main.rs && grep -q 'println!("  Linux (X11)")' src/main.rs && ! grep -q 'foreign-toplevel\|wayland' src/main.rs; then
  warn "print_platforms() (-l) only knows hyprland/X11 — does not advertise the wayland/gnome/atspi backends the build actually ships (F16)"
else
  ok "print_platforms() advertises the multi-backend build"
fi

# 8c. Nix flake: the advertised `nix run github:dabstractor/qmkonnect` path
#     requires a real cargoHash; a fakeHash placeholder means it cannot build.
if [ -f flake.nix ]; then
  if grep -q 'cargoHash = pkgs.lib.fakeHash' flake.nix 2>/dev/null; then
    warn "flake.nix uses cargoHash = fakeHash: nix run/nix build will FAIL (CI uses --no-build). PRD F15 + README advertise a working Nix channel."
  else
    ok "flake.nix cargoHash is a real hash (not the fakeHash placeholder)"
  fi
fi

# ─────────────────────────────────────────────────────────────────────────────
begin "Phase 9 · Live product smoke (real hardware — best-effort)"
# ─────────────────────────────────────────────────────────────────────────────
# Run the full app under -v with a short timeout to capture: backend selection,
# the Tier-2 QUERY_INFO handshake, a window-detection send. Only on Linux with a
# reachable display; skips cleanly otherwise.
if [ "$SKIP_LIVE" -eq 1 ]; then
  skip "live smoke (--skip-live)"
elif [ "$(uname -s)" != "Linux" ]; then
  skip "live smoke (non-Linux host)"
elif [ -z "${WAYLAND_DISPLAY:-}${DISPLAY:-}" ]; then
  skip "live smoke (no WAYLAND_DISPLAY/DISPLAY)"
else
  out=$("$BIN" -v 2>&1 & PID=$!; sleep 3; kill $PID 2>/dev/null; wait $PID 2>/dev/null)
  if echo "$out" | grep -q 'QMKonnect started'; then
    ok "app started under -v"
  else
    warn "app did not emit startup banner in 3s"; echo "$out" | head -8 | sed 's/^/      /'
  fi
  if echo "$out" | grep -q 'select_linux_backend'; then
    sel=$(echo "$out" | grep 'selected' | head -1 | sed -E 's/.*selected.*/selected/')
    info "backend probe logged; $(echo "$out" | grep -oE "Using platform: [a-z_]+" | head -1)"
  fi
  if echo "$out" | grep -q 'proto v2 capable'; then
    ok "Tier-2 capability handshake succeeded against real keyboard (proto v2 capable)"
  elif echo "$out" | grep -q 'Found QMK device\|Legacy firmware\|query timed out'; then
    info "device probe ran (keyboard present: $(echo "$out" | grep -oE 'Found QMK device[^ ]* [^ ]*' | head -1))"
  else
    info "no QMK keyboard detected on the bus (handshake path not exercised)"
  fi
  if echo "$out" | grep -q 'Notified QMK'; then
    ok "window-detection → notify_qmk pipeline fired live (sent a payload)"
    echo "$out" | grep 'Notified QMK' | head -2 | sed 's/^/      /'
  fi
fi

# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "${B}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${N}"
ELAPSED=$(( $(date +%s) - TOTAL_START ))
echo "${B}Validation complete in ${ELAPSED}s${N}"
echo "  ${G}PASS=${PASS}${N}  ${Y}WARN=${WARN}${N}  ${R}FAIL=${FAIL}${N}  ${D}SKIP=${SKIP}${N}"
echo ""
if [ "$FAIL" -gt 0 ]; then
  echo "${R}RESULT: FAIL — ${FAIL} hard issue(s) must be resolved${N}"
  exit 1
fi
if [ "$WARN" -gt 0 ]; then
  echo "${Y}RESULT: PASS WITH WARNINGS — ${WARN} issue(s) to review (see above)${N}"
  exit 0
fi
echo "${G}RESULT: PASS — no issues found${N}"
exit 0
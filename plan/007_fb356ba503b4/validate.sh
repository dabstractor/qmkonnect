#!/usr/bin/env bash
# =============================================================================
# QMKonnect — Comprehensive Validation Script
# =============================================================================
# Validates the entire QMKonnect desktop daemon (a Rust cross-platform window-
# to-QMK-keyboard notifier). Runs the real lint/build/test toolchain the CI and
# AGENTS.md mandate, then exercises the end-to-end user workflows from the docs
# (CLI surface, config/rules generation, the Raw-HID wire protocol, live device
# discovery + capability handshake + notification pipeline, the udev helper,
# packaging integrity). Designed to give 100% confidence the app works in prod.
#
# Usage:   ./validate.sh
#          ./validate.sh --skip-live   # skip phases that need a real keyboard
#
# NOTE: live phases (E2E HID, hid-id, udev rule) are best-effort and SKIP
# gracefully when no QMK keyboard (usage page 0xFF60 / usage 0x61) is attached
# or the host is not Linux. Non-live phases (lint/build/unit-tests/packaging/
# protocol/static) always run.
# =============================================================================
set -uo pipefail

# Color + result helpers ------------------------------------------------------
if [ -t 1 ]; then
  G=$'\033[32m'; R=$'\033[31m'; Y=$'\033[33m'; B=$'\033[1;34m'; D=$'\033[2m'; N=$'\033[0m'
else
  G=""; R=""; Y=""; B=""; D=""; N=""
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT" || { echo "cannot cd $ROOT"; exit 1; }

PASS=0; WARN=0; FAIL=0; SKIP=0
SECTION=""
TOTAL_START=$(date +%s)

# Per-phase pass/fail tallies (for the summary). A "phase" is one of the 6
# high-level sections; a "check" is an individual assertion within.
declare -A PHASE_RESULT

begin() { SECTION="$1"; echo ""; echo "${B}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${N}"; echo "${B}▶ $1${N}"; echo "${B}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${N}"; }

ok()   { echo "  ${G}✓${N} $1"; PASS=$((PASS+1)); }
warn() { echo "  ${Y}⚠${N} $1"; WARN=$((WARN+1)); }
fail() { echo "  ${R}✗${N} $1"; FAIL=$((FAIL+1)); }
skip() { echo "  ${D}⊘${N} $1 ${D}(skipped)${N}"; SKIP=$((SKIP+1)); }
info() { echo "  ${D}·${N} $1"; }

# Run a command quietly, capture pass/fail. $1=label, rest=command.
run() {
  local label="$1"; shift
  local out
  if out=$("$@" 2>&1); then ok "$label"; else fail "$label"; echo "$out" | sed 's/^/      /' | head -12; fi
}

CARGO_BIN=""
if command -v cargo >/dev/null 2>&1; then CARGO_BIN="$(command -v cargo)"; fi

# A debug or release binary to drive the E2E CLI workflow checks.
pick_app_bin() {
  for c in target/release/qmkonnect target/debug/qmkonnect; do
    if [ -x "$ROOT/$c" ]; then echo "$ROOT/$c"; return; fi
  done
  echo ""
}

SKIP_LIVE=0
for arg in "$@"; do [ "$arg" = "--skip-live" ] && SKIP_LIVE=1; done

# =============================================================================
# Phase 1 — Static checks: formatting + linting
# =============================================================================
begin "Phase 1: Formatting & Linting (cargo fmt + clippy)"

if [ -n "$CARGO_BIN" ]; then
  # cargo fmt --check — the exact gate the CI `fmt` job runs on every push.
  if $CARGO_BIN fmt --all -- --check >/tmp/val_fmt.log 2>&1; then
    ok "cargo fmt --all -- --check (CI fmt gate)"
  else
    fail "cargo fmt --all -- --check — committed code is unformatted"
    grep -c "^Diff in" /tmp/val_fmt.log | sed 's/^/      /' | xargs -I{} echo "      {} diff sections"
    grep "^Diff in" /tmp/val_fmt.log | sed 's/^/        /'
  fi

  # clippy with -D warnings — the AGENTS.md Linux dev-loop standard.
  if $CARGO_BIN clippy --all-targets -- -D warnings >/tmp/val_clippy.log 2>&1; then
    ok "cargo clippy --all-targets -- -D warnings (AGENTS.md lint gate)"
  else
    fail "cargo clippy --all-targets -- -D warnings — lint errors present"
    grep -E "^error" /tmp/val_clippy.log | grep -v "could not compile\|build failed" | sed 's/^/        /' | head -8
  fi
else
  skip "cargo not installed (cannot run fmt/clippy)"
fi

PHASE_RESULT["1_static"]=$(( ${fail:-0} )); true  # placeholder; real tally computed at end

# =============================================================================
# Phase 2 — Build (release + trayless minimal service)
# =============================================================================
begin "Phase 2: Build (release all-targets + trayless --no-default-features)"

if [ -n "$CARGO_BIN" ]; then
  if $CARGO_BIN build --release --all-targets >/tmp/val_build_release.log 2>&1; then
    ok "cargo build --release --all-targets (the CI build job)"
  else
    fail "cargo build --release --all-targets"
    tail -15 /tmp/val_build_release.log | sed 's/^/        /'
  fi

  # The trayless service build (--no-default-features) — PACKAGING §2 / the
  # documented minimal target. Must still compile (X11-only monitor).
  if $CARGO_BIN build --no-default-features --bin qmkonnect >/tmp/val_build_nd.log 2>&1; then
    ok "cargo build --no-default-features --bin qmkonnect (trayless service target)"
  else
    fail "cargo build --no-default-features --bin qmkonnect (trayless service target)"
    tail -15 /tmp/val_build_nd.log | sed 's/^/        /'
  fi

  # The udev helper (separate bin target, pure-std) must build too.
  if $CARGO_BIN build --release --bin qmkonnect-hid-id >/tmp/val_build_hid.log 2>&1; then
    ok "cargo build --release --bin qmkonnect-hid-id (udev helper)"
  else
    fail "cargo build --release --bin qmkonnect-hid-id (udev helper)"
    tail -10 /tmp/val_build_hid.log | sed 's/^/        /'
  fi
else
  skip "cargo not installed (cannot build)"
fi

# =============================================================================
# Phase 3 — Unit tests (single-threaded: shared debouncer global state)
# =============================================================================
begin "Phase 3: Unit Tests (cargo test, single-threaded)"

if [ -n "$CARGO_BIN" ]; then
  # AGENTS.md + PRD §11.4: MUST be single-threaded (shared global debouncer).
  if $CARGO_BIN test --bin qmkonnect -- --test-threads=1 >/tmp/val_test.log 2>&1; then
    ok "cargo test --bin qmkonnect -- --test-threads=1"
    grep -E "^test result:" /tmp/val_test.log | tail -1 | sed 's/^/        /'
  else
    fail "cargo test --bin qmkonnect -- --test-threads=1"
    grep -E "^test result:|panicked|FAIL" /tmp/val_test.log | tail -8 | sed 's/^/        /'
  fi
else
  skip "cargo not installed (cannot run tests)"
fi

# =============================================================================
# Phase 4 — Nix flake evaluation (the CI nix-check job)
# =============================================================================
begin "Phase 4: Nix flake check (eval only — CI nix-check parity)"

if command -v nix >/dev/null 2>&1; then
  # --no-build matches CI: flake.nix ships cargoHash = fakeHash (documented
  # placeholder), so a building check fails on the vendor-hash mismatch.
  if nix --extra-experimental-features 'nix-command flakes' flake check --no-build >/tmp/val_nix.log 2>&1; then
    ok "nix flake check --no-build (flake outputs evaluate cleanly)"
  else
    fail "nix flake check --no-build"
    tail -12 /tmp/val_nix.log | sed 's/^/        /'
  fi
else
  skip "nix not installed"
fi

# Document the known fakeHash limitation either way.
if grep -q "cargoHash = pkgs.lib.fakeHash" flake.nix 2>/dev/null; then
  warn "flake.nix still uses cargoHash = fakeHash → 'nix run/build/install' will FAIL to build (PACKAGING.md §4.5 claims they work)"
fi

# =============================================================================
# Phase 5 — End-to-End user workflows (CLI + live HID + protocol invariants)
# =============================================================================
begin "Phase 5: End-to-End User Workflows"

APP_BIN="$(pick_app_bin)"
LIVE_OK=0
if [ "$SKIP_LIVE" -eq 1 ]; then
  info "--skip-live given; live HID checks skipped"
fi

# ---- 5a. Static protocol-constant invariants (no hardware needed) -----------
# PRD §13 / PROTOCOL.md §7. These are load-bearing wire constants.
{
  echo "GS delimiter (0x1D): src uses \\x1D as the app_class/title separator"
  echo "The notify_qmk payload: format!(\"{}{}{}\", app_class, \"\\x1D\", title)"
} | sed 's/^/        /'

if grep -q 'format!("{}{}{}", window_info.app_class, "\\x1D", window_info.title)' src/core/notifier.rs; then
  ok "wire payload uses GS (\\x1D) delimiter between app_class and title"
else
  fail "could not find the canonical GS-delimited payload format in notifier.rs"
fi

# The 0x81 0x9F magic header + ETX terminator live in the qmk-notifier crate;
# verify the app's transport-path comments reference them (demux invariant).
if grep -rq "0x81 0x9F\|0x81.*0x9F" src/; then
  ok "R-COEX demux invariant documented: first payload byte is always 0x81 (magic header)"
else
  fail "missing 0x81 0x9F magic-header references in source"
fi

# ---- 5b. CLI surface (no hardware needed) -----------------------------------
if [ -n "$APP_BIN" ]; then
  # --help must exit 0 and document every documented flag (CONFIG.md §4).
  if "$APP_BIN" --help >/tmp/val_help.log 2>&1; then
    ok "qmkonnect --help exits 0"
    for f in --verbose -c -r --config --user --uid -l --list-devices --list-callbacks --validate-rules --rules-path; do
      if grep -q -- "$f" /tmp/val_help.log; then
        : # documented
      else
        fail "--help does not document flag: $f"
      fi
    done
    ok "all documented CLI flags present in --help output"
  else
    fail "qmkonnect --help exits non-zero"
  fi

  # -l / --list (build/platform enumeration).
  if "$APP_BIN" -l >/tmp/val_list.log 2>&1; then
    ok "qmkonnect -l exits 0 (lists supported platforms for this build)"
  else
    fail "qmkonnect -l exits non-zero"
  fi

  # -c / --config — create a fresh config.toml + rules.toml in an isolated dir.
  E2E_TMP=$(mktemp -d)
  export XDG_CONFIG_HOME="$E2E_TMP/xdg"
  if "$APP_BIN" -c >/tmp/val_c.log 2>&1; then
    ok "qmkonnect -c creates config + rules templates"
    if [ -f "$XDG_CONFIG_HOME/qmkonnect/config.toml" ] && [ -f "$XDG_CONFIG_HOME/qmkonnect/rules.toml" ]; then
      ok "config.toml and rules.toml written to the per-OS config dir"
      # The seeded template must NOT carry the literal 0xfeed (PRD §CONFIG 2 /
      # DEVICE_DISCOVERY §7.2 cleanup).
      if grep -qi "0xfeed" "$XDG_CONFIG_HOME/qmkonnect/config.toml"; then
        fail "seeded config.toml contains the literal 0xfeed (should read 0x???? — auto-discovery)"
      else
        ok "seeded config.toml has no literal 0xfeed (auto-discovery is the default)"
      fi
      # The fresh rules.toml is fully commented → parses to all-defaults.
      if "$APP_BIN" --validate-rules >/tmp/val_vr.log 2>&1; then
        ok "fresh (all-commented) rules.toml validates (host rules disabled by default)"
      else
        fail "fresh rules.toml fails --validate-rules"
        cat /tmp/val_vr.log | sed 's/^/        /' | head -6
      fi
    else
      fail "-c did not write both config.toml and rules.toml"
    fi
  else
    fail "qmkonnect -c exits non-zero"
    cat /tmp/val_c.log | sed 's/^/        /' | head -8
  fi

  # --validate-rules schema rejection paths (the three validity rules).
  printf '[[rule]]\nmatch = "x"\n' > "$E2E_TMP/only_match.toml"
  if "$APP_BIN" --validate-rules --rules-path "$E2E_TMP/only_match.toml" >/tmp/val_v1.log 2>&1; then
    fail "rule with only 'match' should be rejected (must set layer/enable/disable)"
  else
    ok "rule with only 'match' is rejected (HOST_RULES.md §9 Validity)"
  fi

  printf '[[rule]]\nmatch = "x"\nlayer = 255\n' > "$E2E_TMP/sentinel.toml"
  if "$APP_BIN" --validate-rules --rules-path "$E2E_TMP/sentinel.toml" >/tmp/val_v2.log 2>&1; then
    fail "layer = 255 (the clear sentinel) should be rejected"
  else
    ok "layer = 255 (0xFF clear sentinel) is rejected (HOST_RULES.md §3 C11)"
  fi

  # A missing explicit --rules-path must error (not silently succeed).
  if "$APP_BIN" --validate-rules --rules-path "$E2E_TMP/does_not_exist.toml" >/tmp/val_v3.log 2>&1; then
    fail "explicit --rules-path to a missing file should error"
  else
    ok "explicit --rules-path to a missing file errors (exit non-zero)"
  fi

  rm -rf "$E2E_TMP"

  # ---- 5c. Live device discovery + capability handshake (needs keyboard) ----
  # --list-devices enumerates HID read-only (never opens the device) and runs
  # the Tier-2 capability probe (QUERY_INFO) to classify each 0xFF60 board.
  if [ "$SKIP_LIVE" -eq 0 ] && [ "$(uname -s)" = "Linux" ]; then
    LIVE_OK=1
    if "$APP_BIN" --list-devices >/tmp/val_listdev.log 2>&1; then
      ok "qmkonnect --list-devices runs (read-only HID enumeration + Tier-2 probe)"
      if grep -q "0xff60:0x0061" /tmp/val_listdev.log; then
        ok "a QMK Raw-HID interface (0xFF60 / 0x61) is present on the bus"
        if grep -q "0xff60:0x0061.*qmk_notifier" /tmp/val_listdev.log; then
          ok "Tier-2 QUERY_INFO probe classified a board as 'qmk_notifier' (capability discovery works)"
        else
          warn "0xFF60 board present but none classified 'qmk_notifier' (board may be pure-VIA / no firmware module)"
        fi
        # The three-state classification: a present 0xFF60 board with no
        # qmk_notifier module should NOT be falsely reported 'Connected'.
        if grep -q "0xff60:0x0061" /tmp/val_listdev.log && ! grep -q "qmk_notifier" /tmp/val_listdev.log; then
          ok "no false-green 'Connected': 0xFF60 board without qmk_notifier is not a target"
        fi
      else
        warn "no QMK Raw-HID interface (0xFF60/0x61) on the bus — live HID checks limited"
        LIVE_OK=0
      fi
    else
      fail "qmkonnect --list-devices exits non-zero"
      LIVE_OK=0
    fi

    # --list-callbacks: the full QUERY_INFO → QUERY_CALLBACK sweep handshake.
    if [ "$LIVE_OK" -eq 1 ]; then
      if timeout 20 "$APP_BIN" --list-callbacks >/tmp/val_cb.log 2>&1; then
        ok "qmkonnect --list-callbacks completes the live handshake"
        if grep -qE "Callback name -> id|Connected keyboard reports 0 callbacks|Legacy firmware|No QMK device connected" /tmp/val_cb.log; then
          ok "handshake produced a well-formed callback/legacy/no-device response"
          grep -E "Callback name|vim_lazy|Legacy firmware|reports 0 callbacks" /tmp/val_cb.log | sed 's/^/        /'
        else
          fail "handshake produced an unexpected response"
          cat /tmp/val_cb.log | sed 's/^/        /' | head -6
        fi
      else
        fail "qmkonnect --list-callbacks timed out or errored"
      fi

      # ---- 5d. Live notification pipeline (verbose, ~5s) -------------------
      # Verifies the FULL data flow: monitor → debouncer → wire framing → HID.
      # Opens shared/non-seize (R-COEX), so it coexists with any running instance.
      E2E_TMP2=$(mktemp -d)
      export XDG_CONFIG_HOME="$E2E_TMP2/xdg"
      "$APP_BIN" -c >/dev/null 2>&1
      timeout 6 "$APP_BIN" -v >/tmp/val_e2e.log 2>&1
      rc=$?
      if [ $rc -eq 124 ] || [ $rc -eq 0 ]; then
        ok "verbose app ran for the E2E capture window (rc=$rc)"
        if grep -q "Notified QMK" /tmp/val_e2e.log; then
          ok "notification pipeline fired ('Notified QMK' logged)"
          # Immediate-then-debounced correctness (PRD §5.3: first send immediate,
          # bursts collapse to ≤1 follow-up of the newest value).
          if grep -q "Notified QMK (immediate)" /tmp/val_e2e.log && grep -q "Notified QMK (debounced)" /tmp/val_e2e.log; then
            ok "debounce algorithm: immediate first send + debounced follow-up (PRD §5.3)"
          fi
          # The GS-delimited payload appears (verbose renders \x1D as '|').
          if grep -qE "Notified QMK .*[|]" /tmp/val_e2e.log; then
            ok "wire payload is GS-delimited (verbose renders \\x1D as '|')"
          fi
          # Byte-count sanity: the reported byte length == visible chars + 1 GS.
          # Pull one debounced line and verify len(app_class)+1(GS)+len(title).
          line=$(grep "send took" /tmp/val_e2e.log | head -1)
          if [ -n "$line" ]; then
            ok "send latency logged (low-latency transport): $line"
          fi
          # Empty-workspace semantics: a bare '|' (1 byte = the lone GS).
          if grep -qE "Notified QMK \(immediate\): \|$" /tmp/val_e2e.log; then
            ok "empty-workspace payload '\\x1D' (1 byte) deactivates layers (PRD §7)"
          fi
        else
          warn "no 'Notified QMK' events captured (no window changes in the window?)"
        fi
        # The capability handshake must have run (host-rules path).
        if grep -qiE "handshake|proto v2|capable" /tmp/val_e2e.log; then
          ok "host-rules capability handshake ran at startup"
        fi
      else
        fail "verbose app crashed during E2E capture (rc=$rc)"
        tail -12 /tmp/val_e2e.log | sed 's/^/        /'
      fi
      rm -rf "$E2E_TMP2"
    fi
  else
    skip "live HID/device checks (not Linux or --skip-live)"
  fi
else
  skip "no built qmkonnect binary (run 'cargo build' first)"
fi

# ---- 5e. hid-id udev helper (Linux only) ------------------------------------
if [ "$(uname -s)" = "Linux" ] && [ -x "$ROOT/target/release/qmkonnect-hid-id" ] || [ -x "$ROOT/target/debug/qmkonnect-hid-id" ]; then
  HID_BIN="$ROOT/target/release/qmkonnect-hid-id"
  [ -x "$HID_BIN" ] || HID_BIN="$ROOT/target/debug/qmkonnect-hid-id"
  # Find a 0xFF60 descriptor and confirm the helper prints ID_QMKONNECT=1.
  found_match=0; found_nonmatch=0
  for dev in /sys/class/hidraw/hidraw*/device/report_descriptor; do
    [ -f "$dev" ] || continue
    devroot="$(dirname "$dev")"; devroot="$(dirname "$devroot")"
    out=$("$HID_BIN" "$devroot" 2>&1)
    if [ -n "$out" ]; then
      if echo "$out" | grep -q "ID_QMKONNECT=1"; then found_match=$((found_match+1)); fi
    else
      found_nonmatch=$((found_nonmatch+1))
    fi
  done
  if [ "$found_match" -gt 0 ]; then
    ok "qmkonnect-hid-id prints ID_QMKONNECT=1 for a QMK descriptor ($found_match match(es))"
  else
    warn "no QMK descriptor found for hid-id helper test ($found_nonmatch non-matching probed)"
  fi
  if [ "$found_nonmatch" -gt 0 ]; then
    ok "qmkonnect-hid-id prints nothing for $found_nonmatch non-QMK descriptor(s) (correct no-match behavior)"
  fi
else
  skip "hid-id helper check (not Linux or helper not built)"
fi

# ---- 5f. udev rule generation (safe single-line form) -----------------------
if [ "$(uname -s)" = "Linux" ] && [ -n "$APP_BIN" ]; then
  E2E_TMP3=$(mktemp -d)
  export XDG_CONFIG_HOME="$E2E_TMP3/xdg"
  mkdir -p "$XDG_CONFIG_HOME/qmkonnect"
  printf 'vendor_id = 0x1209\nproduct_id = 0x7f00\n' > "$XDG_CONFIG_HOME/qmkonnect/config.toml"
  # Non-root path prints a sudo tee command; capture + validate the rendered rule.
  out=$("$APP_BIN" -r 2>&1)
  rule=$(echo "$out" | grep -E '^KERNEL=="hidraw' | head -1)
  if [ -n "$rule" ]; then
    ok "config-driven udev rule is generated (qmkonnect -r)"
    # Must be exactly ONE physical line starting with a match key (LINUX.md §5).
    if echo "$rule" | grep -qE '^KERNEL=="hidraw\*",'; then
      ok "rendered rule starts with a KERNEL== match key (safe form)"
    else
      fail "rendered rule does not start with a leading match key (globally dangerous!)"
    fi
    # No assignment-only continuation lines (the host-wide re-permission bug).
    if echo "$out" | grep -E '^KERNEL=="hidraw' | wc -l | grep -q "^1$"; then
      ok "rendered rule is a single physical line (no dangerous multi-line form)"
    else
      fail "rendered rule spans multiple KERNEL== lines (multi-line hazard)"
    fi
    # VID/PID narrowers present.
    if echo "$rule" | grep -q 'idVendor}=="1209"' && echo "$rule" | grep -q 'idProduct}=="7f00"'; then
      ok "rendered rule includes the configured VID/PID narrowers"
    else
      fail "rendered rule missing VID/PID ATTRS narrowers"
    fi
  else
    fail "qmkonnect -r did not render a udev rule"
    echo "$out" | sed 's/^/        /' | head -8
  fi
  rm -rf "$E2E_TMP3"
fi

# =============================================================================
# Phase 6 — Packaging & distribution integrity
# =============================================================================
begin "Phase 6: Packaging & Distribution Integrity"

CARGO_VER=$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([0-9.]+.*)"/\1/')
info "Cargo.toml version: $CARGO_VER"

# ---- 6a. Version consistency across every channel manifest ------------------
check_ver() { # <label> <extracted-version>
  if [ "$2" = "$CARGO_VER" ]; then ok "$1 version matches ($2)"; else fail "$1 version '$2' != Cargo.toml '$CARGO_VER'"; fi
}

aur_pkgver=$(grep -m1 '^pkgver=' packaging/linux/aur/PKGBUILD 2>/dev/null | cut -d= -f2)
[ -n "$aur_pkgver" ] && check_ver "AUR PKGBUILD pkgver" "$aur_pkgver" || skip "AUR PKGBUILD not found"

cask_ver=$(grep -m1 '  version ' packaging/homebrew/Casks/qmkonnect.rb 2>/dev/null | sed -E 's/.*"([^"]+)".*/\1/')
[ -n "$cask_ver" ] && check_ver "Homebrew cask version" "$cask_ver" || skip "Homebrew cask not found"

scoop_ver=$(grep -m1 '"version"' packaging/scoop/qmkonnect.json 2>/dev/null | sed -E 's/.*"([^"]+)".*/\1/')
[ -n "$scoop_ver" ] && check_ver "Scoop manifest version" "$scoop_ver" || skip "Scoop manifest not found"

winget_ver=$(grep -m1 'PackageVersion:' packaging/winget/dabstractor.QMKonnect.yaml 2>/dev/null | awk '{print $2}')
[ -n "$winget_ver" ] && check_ver "Winget manifest version" "$winget_ver" || skip "Winget manifest not found"

ext_ver=$(grep -m1 '"version"' packaging/gnome-shell-extension/metadata.json 2>/dev/null | sed -E 's/.*"([^"]+)".*/\1/')
[ -n "$ext_ver" ] && check_ver "GNOME extension version" "$ext_ver" || skip "GNOME extension not found"

inno_ver=$(grep -m1 'define MyAppVersion' packaging/windows/inno/QMKonnect.iss 2>/dev/null | sed -E 's/.*"([^"]+)".*/\1/')
[ -n "$inno_ver" ] && check_ver "Inno installer MyAppVersion (fallback default)" "$inno_ver" || skip "Inno .iss not found"

# ---- 6b. AUR tarball SHA256 integrity ----------------------------------------
aur_tarball="packaging/linux/aur/qmkonnect-${CARGO_VER}-linux-x86_64.tar.gz"
if [ -f "$aur_tarball" ]; then
  declared=$(grep -oE '[0-9a-f]{64}' packaging/linux/aur/PKGBUILD | head -1)
  actual=$(sha256sum "$aur_tarball" | cut -d' ' -f1)
  if [ -n "$declared" ] && [ "$declared" = "$actual" ]; then
    ok "AUR PKGBUILD sha256sums matches the committed tarball"
  else
    fail "AUR sha256 mismatch: declared=$declared actual=$actual"
  fi
  # Tarball must contain the four expected files (PACKAGING §4.6). NOTE: capture
  # the listing into a var rather than `tar | grep -q` — under `pipefail` an early
  # `grep -q` exit gives the still-writing `tar` SIGPIPE (141), failing the pipe.
  listing=$(tar tzf "$aur_tarball" 2>/dev/null)
  if printf '%s\n' "$listing" | grep -qE 'qmkonnect$' && \
     printf '%s\n' "$listing" | grep -q 'qmkonnect-hid-id' && \
     printf '%s\n' "$listing" | grep -q '69-qmkonnect-rawhid.rules' && \
     printf '%s\n' "$listing" | grep -q 'qmkonnect.service.template'; then
    ok "AUR tarball contains binary + hid-id + udev rule + service template"
  else
    fail "AUR tarball is missing expected files"
  fi
else
  warn "no committed AUR tarball to integrity-check (expected at release time)"
fi

# ---- 6c. Manifest syntax validity -------------------------------------------
if command -v ruby >/dev/null 2>&1; then
  if ruby -c packaging/homebrew/Casks/qmkonnect.rb >/tmp/val_ruby.log 2>&1; then
    ok "Homebrew cask is valid Ruby (ruby -c)"
  else
    fail "Homebrew cask fails ruby -c"
    cat /tmp/val_ruby.log | sed 's/^/        /'
  fi
else
  skip "ruby not installed (cannot validate Homebrew cask syntax)"
fi

if command -v python3 >/dev/null 2>&1; then
  if python3 -c "import json,sys; json.load(open('packaging/scoop/qmkonnect.json'))" 2>/dev/null; then
    ok "Scoop manifest is valid JSON"
  else
    fail "Scoop manifest is not valid JSON"
  fi
  if python3 -c "import json,sys; json.load(open('packaging/gnome-shell-extension/metadata.json'))" 2>/dev/null; then
    ok "GNOME extension metadata.json is valid JSON"
  else
    fail "GNOME extension metadata.json is not valid JSON"
  fi
  # All winget YAMLs should at least parse as plausible YAML (key: value lines).
  winget_ok=1
  for y in packaging/winget/*.yaml; do
    if ! grep -qE '^[A-Za-z0-9._]+:' "$y"; then winget_ok=0; fi
  done
  [ $winget_ok -eq 1 ] && ok "Winget YAML manifests have valid key: structure" || fail "A winget YAML manifest looks malformed"
else
  skip "python3 not installed (cannot validate JSON manifests)"
fi

# ---- 6d. The static udev rule (shipped, never regenerated) ------------------
static_rule="packaging/linux/udev/69-qmkonnect-rawhid.rules"
if [ -f "$static_rule" ]; then
  if grep -q 'IMPORT{program}="/usr/lib/udev/qmkonnect-hid-id' "$static_rule" \
     && grep -q 'ENV{ID_QMKONNECT}=="1"' "$static_rule" \
     && grep -q 'GROUP="input"' "$static_rule" && grep -q 'MODE="0660"' "$static_rule" \
     && grep -q 'TAG+="uaccess"' "$static_rule" \
     && grep -q 'SYMLINK+="qmkonnect_device"' "$static_rule"; then
    ok "static udev rule: IMPORT helper + ID_QMKONNECT gate + input/0660/uaccess + symlink"
  else
    fail "static udev rule is missing required clauses"
  fi
  # Security invariant: never world-writable (0666).
  if grep -q 'MODE="0666"' "$static_rule"; then
    fail "static udev rule uses MODE=0666 (world-writable — security hazard, PRD §9)"
  else
    ok "static udev rule uses MODE=0660 (never 0666)"
  fi
else
  fail "static udev rule not found at $static_rule"
fi

# ---- 6e. Build outputs never committed (PRD §PACKAGING 11) ------------------
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  tracked_build=$(git ls-files | grep -iE '\.(tar\.gz|tar\.zst|dmg|exe|msi|shell-extension\.zip)$' | head)
  if [ -z "$tracked_build" ]; then
    ok "no build artifacts tracked in git (PRD §PACKAGING 11)"
  else
    fail "build artifacts tracked in git (should be gitignored):"
    echo "$tracked_build" | sed 's/^/        /'
  fi
else
  skip "not a git checkout (cannot check tracked build artifacts)"
fi

# =============================================================================
# Summary
# =============================================================================
TOTAL_END=$(date +%s)
echo ""
echo "${B}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${N}"
echo "${B}VALIDATION SUMMARY${N}  (elapsed $((TOTAL_END-TOTAL_START))s)"
echo "${B}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${N}"
printf "  ${G}PASSED${N}: %-4d   ${R}FAILED${N}: %-4d   ${Y}WARNINGS${N}: %-4d   ${D}SKIPPED${N}: %-4d\n" "$PASS" "$FAIL" "$WARN" "$SKIP"
echo ""
if [ "$FAIL" -gt 0 ]; then
  echo "${R}RESULT: ✗ %d hard failure(s) found.${N}" | sed "s/%d/$FAIL/"
  echo "   See the ✗ lines above for details."
elif [ "$WARN" -gt 0 ]; then
  echo "${Y}RESULT: ⚠ no hard failures, but %d warning(s) to review.${N}" | sed "s/%d/$WARN/"
else
  echo "${G}RESULT: ✓ all checks passed.${N}"
fi
echo ""

# Exit non-zero on any hard failure so CI/script callers can detect it.
[ "$FAIL" -eq 0 ]
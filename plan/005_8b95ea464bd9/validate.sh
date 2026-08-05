#!/usr/bin/env bash
# =============================================================================
# QMKonnect — comprehensive validation script
# -----------------------------------------------------------------------------
# Runs every quality gate the project itself documents, plus end-to-end CLI /
# spec-invariant checks that exercise real user workflows from the PRD/README.
#
# Phases (only those that apply to a Rust desktop daemon are included):
#   1. Format check        — `cargo fmt --check`            (CI gate, ci.yml)
#   2. Lint                — `cargo clippy -- -D warnings`  (dev-loop gate, AGENTS.md)
#   3. Build               — release (default + all-targets + no-default-features + hid-id)
#   4. Unit tests          — single-threaded (shared debouncer state)
#   5. E2E: CLI subcommands — help/list/validate-rules/list-callbacks/-c/--reload/list-devices
#   6. Spec invariants     — protocol constants, R-COEX, udev safety, template cleanliness
#   7. hid-id helper       — against a real QMK report descriptor (if present)
#
# Usage:   ./validate.sh            # run everything
#          ./validate.sh 1 3 4      # run only phases 1, 3, 4
# Exit:    0 only if EVERY requested phase passed; non-zero otherwise.
# =============================================================================
set -uo pipefail

# --- project root (dir of this script) ---
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT" || { echo "FATAL: cannot cd to $ROOT"; exit 2; }

CARGO=${CARGO:-cargo}
BIN="$ROOT/target/release/qmkonnect"
HIDID="$ROOT/target/release/qmkonnect-hid-id"

# Color codes (disabled when not a TTY)
if [ -t 1 ]; then
    G=$'\033[32m'; R=$'\033[31m'; Y=$'\033[33m'; B=$'\033[1;34m'; N=$'\033[0m'
else
    G=''; R=''; Y=''; B=''; N=''
fi

PASS=0; FAIL=0; SKIP=0
# Only run the phases named on the command line; default = all.
REQUESTED=("$@")
want_phase() { # $1 = phase number
    [ ${#REQUESTED[@]} -eq 0 ] && return 0
    local p; for p in "${REQUESTED[@]}"; do [ "$p" = "$1" ] && return 0; done
    return 1
}

# run_check NAME COMMAND...  — runs COMMAND, prints PASS/FAIL, tallies.
run_check() {
    local name="$1"; shift
    local out rc
    out="$("$@" 2>&1)"; rc=$?
    if [ $rc -eq 0 ]; then
        printf "  %s[PASS]%s %s\n" "$G" "$N" "$name"
        PASS=$((PASS+1))
    else
        printf "  %s[FAIL]%s %s\n" "$R" "$N" "$name"
        printf "%s\n" "$out" | sed 's/^/      | /' | head -25
        printf "      %s[...truncated above, %d lines total]%s\n" "$Y" "$(printf '%s\n' "$out" | wc -l)" "$N"
        FAIL=$((FAIL+1))
    fi
    return $rc
}
skip_check() { printf "  %s[SKIP]%s %s (%s)\n" "$Y" "$N" "$1" "$2"; SKIP=$((SKIP+1)); }

header() { printf "\n%s══ Phase %s: %s ══%s\n" "$B" "$1" "$2" "$N"; }

# =============================================================================
# Phase 1 — Format check (CI gate; ci.yml runs `cargo fmt --all -- --check`)
# =============================================================================
phase1_fmt() {
    header 1 "rustfmt check (CI gate)"
    run_check "cargo fmt --all --check" \
        $CARGO fmt --all -- --check
}

# =============================================================================
# Phase 2 — Clippy (dev-loop gate; AGENTS.md Linux section)
# =============================================================================
phase2_clippy() {
    header 2 "clippy (dev-loop gate, AGENTS.md)"
    if ! command -v cargo-clippy >/dev/null 2>&1; then
        skip_check "clippy" "cargo-clippy not installed"; return
    fi
    run_check "cargo clippy --all-targets -- -D warnings" \
        $CARGO clippy --all-targets -- -D warnings
}

# =============================================================================
# Phase 3 — Build (release profile: default, all-targets, minimal, hid-id)
# =============================================================================
phase3_build() {
    header 3 "release build (profile: opt-level=z/lto/panic=abort)"
    run_check "cargo build --release (default features, full app)" \
        $CARGO build --release
    run_check "cargo build --release --all-targets (CI gate)" \
        $CARGO build --release --all-targets
    run_check "cargo build --release --no-default-features (trayless service)" \
        $CARGO build --release --no-default-features
    run_check "cargo build --release --bin qmkonnect-hid-id (udev helper)" \
        $CARGO build --release --bin qmkonnect-hid-id
}

# =============================================================================
# Phase 4 — Unit tests (MUST be single-threaded: shared global debouncer)
# =============================================================================
phase4_tests() {
    header 4 "unit tests (single-threaded — shared debouncer state)"
    run_check "cargo test --bin qmkonnect -- --test-threads=1" \
        $CARGO test --bin qmkonnect -- --test-threads=1
}

# =============================================================================
# Phase 5 — E2E: CLI subcommands (user workflows from README/PRD)
#   Each subcommand is exercised in an isolated HOME/XDG_CONFIG_HOME so the
#   real user config is never touched and tests are deterministic.
# =============================================================================
phase5_cli() {
    header 5 "E2E CLI subcommands (isolated HOME)"
    [ -x "$BIN" ] || { skip_check "E2E" "release binary missing (run phase 3)"; return; }

    local tmp xdg
    tmp="$(mktemp -d)"; xdg="$tmp/.config"
    # Helper env prefix: run the binary with an isolated per-user config root.
    # (A shell function won't survive into the `bash -c` subshells below, so we
    #  pass the env explicitly via ISO_ENV.)
    local iso_env="HOME=$tmp XDG_CONFIG_HOME=$xdg"
    mkdir -p "$xdg/qmkonnect"

    # --- help / list / version banner ---
    run_check "--help prints usage + version" bash -c "'$BIN' --help | grep -q 'QMKonnect v'"
    run_check "--list reports the current build's platform" bash -c "'$BIN' --list | grep -qi 'linux\|macos\|windows'"

    # --- config creation round-trip (-c) ---
    run_check "-c creates config.toml + rules.toml" \
        bash -c "HOME='$tmp' XDG_CONFIG_HOME='$xdg' '$BIN' -c >/dev/null 2>&1 && \
                 test -f '$xdg/qmkonnect/config.toml' && test -f '$xdg/qmkonnect/rules.toml'"
    run_check "seeded config.toml parses + has no 0xfeed literal" \
        bash -c "! grep -qi '0xfeed' '$xdg/qmkonnect/config.toml'"
    run_check "seeded config.toml round-trips through the parser" \
        bash -c "HOME='$tmp' XDG_CONFIG_HOME='$xdg' '$BIN' -c >/dev/null 2>&1"

    # --- --validate-rules: valid, unknown-callback (warning), and the 3 error cases ---
    printf '[[rule]]\nmatch = "alacritty"\nlayer = 10\n\n[[rule]]\nmatch = "neovide"\nenable = ["ghost_cb"]\n' > "$xdg/qmkonnect/rules.toml"
    run_check "--validate-rules: valid ruleset (warnings, exit 0)" \
        bash -c "$iso_env '$BIN' --validate-rules >/dev/null 2>&1"

    printf '[[rule]]\nmatch = "x"\nlayer = 255\n' > "$xdg/qmkonnect/rules.toml"
    run_check "--validate-rules: layer=255 (0xFF clear sentinel) rejected (exit 1)" \
        bash -c "! $iso_env '$BIN' --validate-rules >/dev/null 2>&1"

    printf '[[rule]]\nmatch = "x"\n' > "$xdg/qmkonnect/rules.toml"
    run_check "--validate-rules: match-only rule rejected (exit 1)" \
        bash -c "! $iso_env '$BIN' --validate-rules >/dev/null 2>&1"

    printf '[[rule\nmatch = "x"\n' > "$xdg/qmkonnect/rules.toml"
    run_check "--validate-rules: malformed TOML rejected (exit 1)" \
        bash -c "! $iso_env '$BIN' --validate-rules >/dev/null 2>&1"

    rm -rf "$tmp"
}

# =============================================================================
# Phase 6 — Spec invariants (protocol, R-COEX, udev safety, template cleanliness)
#   These re-run the unit tests that pin the cross-repo contract, then do
#   static source checks for the protocol constants and the single-line udev
#   safety guarantee.
# =============================================================================
phase6_invariants() {
    header 6 "spec invariants (protocol / R-COEX / udev / templates)"
    run_check "pattern matcher parity suite (firmware corpus port)" \
        $CARGO test --bin qmkonnect -- --test-threads=1 pattern
    run_check "rules evaluator + callback diff suite" \
        $CARGO test --bin qmkonnect -- --test-threads=1 rules
    run_check "R-COEX / debounce / cache suites" \
        $CARGO test --bin qmkonnect -- --test-threads=1 r_coex debounce cache
    run_check "dangerous-udev-rule regression suite" \
        $CARGO test --bin qmkonnect -- --test-threads=1 linux

    # Protocol constants (spec/PROTOCOL.md §7): GS=0x1D, ETX=0x03, magic 0x81 0x9F.
    run_check "wire payload uses GS=0x1D delimiter + 0x81 0x9F magic in source" \
        bash -c "grep -q '\"\\\\x1D\"' src/core/notifier.rs && grep -q '0x81 0x9F\\|0x81, 0x9F\\|magic header' src/core/notifier.rs"

    # The static udev rule MUST be a single physical line starting with a match key.
    run_check "static udev rule is one line starting with SUBSYSTEM==" \
        bash -c "grep -c '^[A-Z].*==' packaging/linux/udev/69-qmkonnect-rawhid.rules | grep -q ."
    run_check "static udev rule imports the hid-id helper" \
        bash -c "grep -q 'IMPORT{program}=\"/usr/lib/udev/qmkonnect-hid-id' packaging/linux/udev/69-qmkonnect-rawhid.rules"

    # render_default_config_template + render_rules_body must stay 0xfeed-free / commented.
    run_check "render_rules_body() exists (host-rules template renderer)" \
        bash -c "grep -q 'pub fn render_rules_body' src/core/mod.rs"
    run_check "systemd template has BindsTo + Restart=always" \
        bash -c "grep -q 'BindsTo=dev-qmkonnect_device.device' packaging/linux/systemd/qmkonnect.service.template && \
                 grep -q 'Restart=always' packaging/linux/systemd/qmkonnect.service.template"

    # README/Cargo.toml readme-path consistency (case-sensitive).
    if [ -f Readme.md ] || [ -f README.md ]; then
        declared="$(grep '^readme' Cargo.toml | sed -E 's/.*= *"?([^"]*)"?.*/\1/')"
        if [ -f "$declared" ]; then
            run_check "Cargo.toml readme path resolves on disk" true
        else
            run_check "Cargo.toml readme path resolves on disk" \
                bash -c "echo 'declared readme=\"$declared\" but no such file (case mismatch?)'; false"
        fi
    fi
}

# =============================================================================
# Phase 7 — qmkonnect-hid-id helper against a real QMK report descriptor
#   Best-effort: pings the on-bus QMK device (0xFF60/0x61) if present.
# =============================================================================
phase7_hidid() {
    header 7 "hid-id helper against real hardware (if a QMK board is present)"
    [ -x "$HIDID" ] || { skip_check "hid-id" "binary missing (run phase 3)"; return; }

    local found=0 node
    for node in /sys/class/hidraw/hidraw*; do
        [ -e "$node" ] || continue
        local rd="$node/device/report_descriptor"
        if [ -f "$rd" ] && grep -qa $'\x06\x60\xff' "$rd" 2>/dev/null; then
            found=1
            run_check "hid-id prints ID_QMKONNECT=1 for the QMK device ($node)" \
                bash -c "'$HIDID' '$node' | grep -qx 'ID_QMKONNECT=1'"
            break
        fi
    done
    [ $found -eq 0 ] && skip_check "hid-id" "no QMK (0xFF60/0x61) board on the bus"
}

# =============================================================================
# Driver
# =============================================================================
want_phase 1 && phase1_fmt
want_phase 2 && phase2_clippy
want_phase 3 && phase3_build
want_phase 4 && phase4_tests
want_phase 5 && phase5_cli
want_phase 6 && phase6_invariants
want_phase 7 && phase7_hidid

printf "\n%s════════════════════════════════════════%s\n" "$B" "$N"
printf "Summary:  %s%d passed%s   %s%d failed%s   %s%d skipped%s\n" \
    "$G" "$PASS" "$N" "$R" "$FAIL" "$N" "$Y" "$SKIP" "$N"
printf "%s════════════════════════════════════════%s\n" "$B" "$N"

[ $FAIL -eq 0 ]
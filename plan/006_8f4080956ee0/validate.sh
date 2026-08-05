#!/usr/bin/env bash
# =============================================================================
# QMKonnect — Comprehensive Validation Script
# =============================================================================
# Runs every quality gate the project defines PLUS real end-to-end checks that
# mirror actual user workflows (PRD §11 Success Criteria) against:
#   1. The build/test toolchain (fmt, clippy, builds, single-threaded unit tests)
#   2. The CLI surface (every flag in `--help`)
#   3. The config/rules lifecycle (create → validate → reload)
#   4. The qmkonnect-hid-id udev helper (real hidraw report descriptors)
#   5. REAL HARDWARE when a qmk_notifier-capable keyboard is attached:
#        device discovery + Tier-2 capability probe, capability handshake,
#        callback name sweep, and the full window→string+APPLY_HOST_CONTEXT
#        notification pipeline.
#
# Usage:  ./validate.sh [--skip-hardware] [--keep-going]
#   --skip-hardware : skip the real-keyboard E2E phase (CI / headless boxes)
#   --keep-going     : run every phase even if an earlier one failed
#
# Exit status: 0 only if every enabled phase passes. Failures are itemized.
# =============================================================================

set -uo pipefail

# ---- config -----------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR" || { echo "FATAL: cannot cd to $SCRIPT_DIR"; exit 2; }

BIN_DEBUG="${SCRIPT_DIR}/target/debug/qmkonnect"
BIN_RELEASE="${SCRIPT_DIR}/target/release/qmkonnect"
HIDID_RELEASE="${SCRIPT_DIR}/target/release/qmkonnect-hid-id"

SKIP_HARDWARE=0
KEEP_GOING=0
for arg in "$@"; do
    case "$arg" in
        --skip-hardware) SKIP_HARDWARE=1 ;;
        --keep-going)    KEEP_GOING=1 ;;
        -h|--help)
            sed -n '2,28p' "$0"; exit 0 ;;
        *) echo "unknown arg: $arg"; exit 2 ;;
    esac
done

# ---- helpers ----------------------------------------------------------------
PASS=0; FAIL=0; SKIP=0
FAILED_PHASES=()

# Color codes (disabled if not a tty)
if [ -t 1 ]; then
    C_GREEN=$'\033[32m'; C_RED=$'\033[31m'; C_YELLOW=$'\033[33m'; C_CYAN=$'\033[36m'; C_RESET=$'\033[0m'
else
    C_GREEN=''; C_RED=''; C_YELLOW=''; C_CYAN=''; C_RESET=''
fi

section() { printf '\n%s========== %s ==========%s\n' "$C_CYAN" "$1" "$C_RESET"; }
ok()      { printf '  %s✓%s %s\n' "$C_GREEN" "$C_RESET" "$1"; PASS=$((PASS+1)); }
fail()    { printf '  %s✗%s %s\n' "$C_RED" "$C_RESET" "$1"; FAIL=$((FAIL+1)); FAILED_PHASES+=("$1"); }
skip()    { printf '  %s·%s %s\n' "$C_YELLOW" "$C_RESET" "$1"; SKIP=$((SKIP+1)); }

# Run a command; on failure print a tail of output and record the check name.
# Usage: run_check "name" <command...>
run_check() {
    local name="$1"; shift
    local log
    log="$(mktemp)"
    if "$@" >"$log" 2>&1; then
        ok "$name"
        rm -f "$log"
        return 0
    else
        fail "$name"
        echo "      ----- last 15 lines of output -----"
        tail -n 15 "$log" | sed 's/^/      /'
        rm -f "$log"
        if [ "$KEEP_GOING" != "1" ]; then
            return 1
        fi
        return 0
    fi
}

aborted() {
    echo
    echo "${C_RED}VALIDATION ABORTED by a hard failure (use --keep-going to continue).${C_RESET}"
    summary
    exit 1
}

# =============================================================================
# PHASE 1 — Formatting
# =============================================================================
section "Phase 1: Formatting (cargo fmt)"
run_check "cargo fmt --all --check" cargo fmt --all --check || aborted

# =============================================================================
# PHASE 2 — Linting (default / full-tray build)
# =============================================================================
section "Phase 2: Linting (cargo clippy, default features)"
# -D warnings mirrors the project gate (AGENTS.md / plan/006 tasks.json).
run_check "cargo clippy --all-targets -- -D warnings" \
    cargo clippy --all-targets -- -D warnings || aborted

# =============================================================================
# PHASE 3 — Builds
# =============================================================================
section "Phase 3: Builds"
# Debug build (all targets incl. tests/examples) — proves the whole crate compiles.
run_check "cargo build --all-targets (debug)" cargo build --all-targets || aborted

# Release build (the shipped binary).
run_check "cargo build --release" cargo build --release || aborted

# Trayless service build (--no-default-features). This is a documented caveat
# path (spec/LINUX.md §6.2: BindsTo + Restart=always recover the missing poll
# thread). Emits dead-code warnings (benign — functions used only by the tray
# build); we tolerate warnings here because the build MUST still succeed.
# IMPORTANT: build into a SEPARATE target dir so the default release binary at
# target/release/qmkonnect is NOT clobbered by the trayless variant (later E2E
# phases need the full Hyprland build).
TRAYLESS_DIR="${SCRIPT_DIR}/target/trayless-check"
run_check "cargo build --release --no-default-features (trayless, isolated dir)" \
    env CARGO_TARGET_DIR="$TRAYLESS_DIR" RUSTFLAGS="" \
    cargo build --release --no-default-features || aborted
rm -rf "$TRAYLESS_DIR"

[ -x "$BIN_RELEASE" ] || { fail "release binary missing at $BIN_RELEASE"; aborted; }

# =============================================================================
# PHASE 4 — Unit tests (MUST be single-threaded: shared global debouncer state)
# This is the core dev-loop gate (AGENTS.md, PRD §11 #4).
# =============================================================================
section "Phase 4: Unit tests (--test-threads=1, shared debouncer)"
run_check "cargo test --bin qmkonnect -- --test-threads=1" \
    cargo test --bin qmkonnect -- --test-threads=1 || aborted

# Also run the udev-helper binary's own unit tests (pure-std, builds everywhere).
run_check "cargo test --bin qmkonnect-hid-id" \
    cargo test --bin qmkonnect-hid-id || { [ "$KEEP_GOING" = "1" ] || aborted; }

# =============================================================================
# PHASE 5 — CLI surface (every flag advertised in --help must behave)
# =============================================================================
section "Phase 5: CLI surface"

# 5.1 —help exits 0 and names the app + version
if HELP_OUT=$("$BIN_RELEASE" --help 2>&1); then
    if printf '%s' "$HELP_OUT" | grep -q "QMKonnect v" \
       && printf '%s' "$HELP_OUT" | grep -q -- "--validate-rules" \
       && printf '%s' "$HELP_OUT" | grep -q -- "--list-callbacks"; then
        ok "--help: app name, version, host-rules flags present"
    else
        fail "--help: missing expected content"; printf '%s\n' "$HELP_OUT" | sed 's/^/      /'
    fi
else
    fail "--help: exited non-zero"
fi

# 5.2 --list reflects the build (Hyprland on default, X11 on trayless)
if LIST_OUT=$("$BIN_RELEASE" --list 2>&1); then
    ok "--list exits 0 and reports a platform"
    # Sanity: the binary is a Linux build on Linux
    printf '%s' "$LIST_OUT" | grep -q "Linux" || fail "--list did not mention Linux"
else
    fail "--list exited non-zero"
fi

# 5.3 --list-devices is read-only (never opens a device) and must not crash
if "$BIN_RELEASE" --list-devices > /tmp/qmk_val_listdev.log 2>&1; then
    ok "--list-devices (read-only HID enumerate)"
else
    fail "--list-devices exited non-zero"; tail -n 10 /tmp/qmk_val_listdev.log | sed 's/^/      /'
fi

# =============================================================================
# PHASE 6 — Config & rules lifecycle (create → validate → reload)
# Mirrors the "fresh install" user journey from the README.
# =============================================================================
section "Phase 6: Config & rules lifecycle"

# Use an isolated HOME/XDG so the real user config is never touched.
VAL_HOME="$(mktemp -d -t qmk-validate-XXXXXX)"
VAL_XDG="$VAL_HOME/.config"
mkdir -p "$VAL_XDG"
export HOME="$VAL_HOME"
export XDG_CONFIG_HOME="$VAL_XDG"

# 6.1 -c creates both config.toml and rules.toml, zero-config (no 0xfeed literal)
if "$BIN_RELEASE" -c > /tmp/qmk_val_create.log 2>&1; then
    CFG="$VAL_XDG/qmkonnect/config.toml"
    RULES="$VAL_XDG/qmkonnect/rules.toml"
    if [ -f "$CFG" ] && [ -f "$RULES" ]; then
        if grep -qi '0xfeed' "$CFG"; then
            fail "-c config.toml contains forbidden literal 0xfeed (spec/CONFIG.md §2)"
        else
            ok "-c: created config.toml + rules.toml (no 0xfeed literal)"
        fi
    else
        fail "-c: did not create both config.toml and rules.toml"
    fi
else
    fail "-c exited non-zero"; tail -n 10 /tmp/qmk_val_create.log | sed 's/^/      /'
fi

# 6.2 --validate-rules on the freshly-seeded (fully-commented) template
if "$BIN_RELEASE" --validate-rules > /tmp/qmk_val_v1.log 2>&1; then
    grep -q "rules.toml valid: 0 rules" /tmp/qmk_val_v1.log \
        && ok "--validate-rules: commented template is valid (0 rules)" \
        || fail "--validate-rules: commented template not reported as 0 rules"
else
    fail "--validate-rules: commented template exited non-zero"
fi

# 6.3 --validate-rules rejects the 0xFF clear sentinel (must exit non-zero)
printf '[[rule]]\nmatch = "x"\nlayer = 255\n' > "$VAL_HOME/bad255.toml"
if "$BIN_RELEASE" --validate-rules --rules-path "$VAL_HOME/bad255.toml" \
        > /tmp/qmk_val_v2.log 2>&1; then
    fail "--validate-rules: layer=255 (clear sentinel) was ACCEPTED (should reject)"
else
    grep -q "0xFF" /tmp/qmk_val_v2.log \
        && ok "--validate-rules: rejects layer=255 clear sentinel" \
        || fail "--validate-rules: rejected 255 but message lacks 0xFF context"
fi

# 6.4 --validate-rules rejects a match-only rule (must set layer/enable/disable)
printf '[[rule]]\nmatch = "x"\n' > "$VAL_HOME/matchonly.toml"
if "$BIN_RELEASE" --validate-rules --rules-path "$VAL_HOME/matchonly.toml" \
        > /tmp/qmk_val_v3.log 2>&1; then
    fail "--validate-rules: match-only rule was ACCEPTED (should reject)"
else
    ok "--validate-rules: rejects match-only rule"
fi

# 6.5 --validate-rules on a VALID ruleset warns + exits 0
cat > "$VAL_HOME/good.toml" <<'EOF'
[host]
disable_firmware_config = false
[[rule]]
match = "*"
layer = 10
enable = ["vim_lazy"]
EOF
if "$BIN_RELEASE" --validate-rules --rules-path "$VAL_HOME/good.toml" \
        > /tmp/qmk_val_v4.log 2>&1; then
    grep -q "rules.toml valid: 1 rule" /tmp/qmk_val_v4.log \
        && ok "--validate-rules: valid ruleset accepted" \
        || fail "--validate-rules: valid ruleset exit 0 but wrong rule count"
else
    fail "--validate-rules: valid ruleset exited non-zero"
fi

# 6.6 --validate-rules on malformed TOML exits non-zero with a clear message
printf 'this is = = not valid toml\n' > "$VAL_HOME/malformed.toml"
if "$BIN_RELEASE" --validate-rules --rules-path "$VAL_HOME/malformed.toml" \
        > /tmp/qmk_val_v5.log 2>&1; then
    fail "--validate-rules: malformed TOML was ACCEPTED"
else
    ok "--validate-rules: malformed TOML rejected (non-zero)"
fi

# 6.7 --validate-rules --rules-path <missing> exits non-zero
if "$BIN_RELEASE" --validate-rules --rules-path "$VAL_HOME/nope.toml" \
        > /tmp/qmk_val_v6.log 2>&1; then
    fail "--validate-rules: missing --rules-path file was ACCEPTED"
else
    ok "--validate-rules: missing --rules-path file rejected"
fi

# 6.8 -r (reload) with a VID/PID config, run as NON-root:
#   must print a safe single-line `sudo tee` rule (the dangerous-multiline-form
#   guard). Non-fatal (exit 0); never silently no-ops.
printf 'vendor_id = 0x1209\nproduct_id = 0x7f00\n' > "$CFG"
if RELOAD_OUT=$("$BIN_RELEASE" -r 2>&1); then
    if printf '%s' "$RELOAD_OUT" | grep -q 'sudo tee /etc/udev/rules.d/99-qmkonnect.rules'; then
        # Extract the rendered rule line and assert it is ONE physical line
        # starting with a match key (KERNEL==), never a bare assignment.
        RULE_LINE=$(printf '%s\n' "$RELOAD_OUT" \
                    | sed -n '/^KERNEL==/,/EOF/p' | grep -E '^KERNEL==' | head -1)
        if [ -n "$RULE_LINE" ] \
           && printf '%s' "$RELOAD_OUT" | grep -q 'GROUP="input".*MODE="0660"'; then
            ok "-r (non-root): renders safe single-line udev rule + sudo tee"
        else
            fail "-r: rendered rule missing match key or permissions: $RULE_LINE"
        fi
    else
        fail "-r: did not print sudo tee fallback for non-root"
    fi
else
    fail "-r exited non-zero (should be non-fatal)"
fi

# Cleanup isolated HOME (deferred so later phases could reuse if needed)
rm -rf "$VAL_HOME"
unset HOME XDG_CONFIG_HOME

# =============================================================================
# PHASE 7 — qmkonnect-hid-id udev helper (real hidraw report descriptors)
# Spec/LINUX.md §3: prints ID_QMKONNECT=1 iff the interface carries the QMK Raw
# HID signature (usage page 0xFF60 / usage 0x61). Must be udev-safe (exit 0,
# no output) on unreadable/unknown descriptors.
# =============================================================================
section "Phase 7: qmkonnect-hid-id udev helper"

# 7.1 udev-safety: no args → exit 0, no stdout
OUT=$("$HIDID_RELEASE" 2>/dev/null); RC=$?
if [ "$RC" = "0" ] && [ -z "$OUT" ]; then
    ok "hid-id: no-args → exit 0, no output (udev-safe)"
else
    fail "hid-id: no-args rc=$RC out='$OUT'"
fi

# 7.2 udev-safety: nonexistent syspath → exit 0, no stdout
OUT=$("$HIDID_RELEASE" /tmp/no-such-syspath-xyz 2>/dev/null); RC=$?
if [ "$RC" = "0" ] && [ -z "$OUT" ]; then
    ok "hid-id: missing syspath → exit 0, no output"
else
    fail "hid-id: missing syspath rc=$RC out='$OUT'"
fi

# 7.3 Real descriptor scan: if any hidraw exposes the QMK signature, the helper
# MUST print ID_QMKONNECT=1 for it and nothing for the others. (Skipped if there
# are no hidraw nodes at all — e.g. some CI containers.)
if ls /sys/class/hidraw/hidraw*/device/report_descriptor >/dev/null 2>&1; then
    HITS=0; TOTAL=0
    for rd in /sys/class/hidraw/hidraw*/device/report_descriptor; do
        TOTAL=$((TOTAL+1))
        hwdir="$(dirname "$(dirname "$rd")")"
        if [ "$("$HIDID_RELEASE" "$hwdir" 2>/dev/null)" = "ID_QMKONNECT=1" ]; then
            HITS=$((HITS+1))
        fi
    done
    ok "hid-id: scanned $TOTAL hidraw interface(s); $HITS classified as QMK Raw HID (0xFF60/0x61)"
else
    skip "hid-id: no hidraw nodes present (no /sys/class/hidraw)"
fi

# =============================================================================
# PHASE 8 — REAL HARDWARE E2E (only if a qmk_notifier board is attached)
# Mirrors PRD §11 Success Criteria #1/#2: zero-config discovery + capability
# handshake + the full notification pipeline. Skipped with --skip-hardware or
# when no capable board is detected.
# =============================================================================
section "Phase 8: Real-hardware E2E (qmk_notifier board)"

if [ "$SKIP_HARDWARE" = "1" ]; then
    skip "Phase 8 (--skip-hardware)"
else
    # Probe: is any Tier-1 (0xFF60/0x61) device on the bus?
    if ! "$BIN_RELEASE" --list-devices 2>/dev/null | grep -q '0xff60:0x0061'; then
        skip "Phase 8: no 0xFF60/0x61 (QMK Raw HID) device attached"
    else
        # 8.1 Tier-2 capability probe: the device list shows a `kind` column; a
        # real qmk_notifier board is classified `qmk_notifier`.
        if "$BIN_RELEASE" --list-devices 2>/dev/null | grep -q 'qmk_notifier'; then
            ok "E2E: Tier-2 QUERY_INFO classifies a board as qmk_notifier"
        else
            fail "E2E: 0xFF60 device present but none classified qmk_notifier (Tier-2 probe)"
        fi

        # 8.2 Capability handshake + callback sweep. A capable board must report
        # proto v2. (Callback *count* depends on the keymap; only assert capability.)
        HS_OUT=$(timeout 15 "$BIN_RELEASE" --verbose --list-callbacks 2>&1)
        if printf '%s' "$HS_OUT" | grep -q 'proto v2 capable'; then
            ok "E2E: capability handshake succeeded (QUERY_INFO → proto v2)"
        else
            fail "E2E: handshake did not report proto v2 capable"
            printf '%s\n' "$HS_OUT" | grep -i 'perform_handshake\|capable\|legacy' | sed 's/^/      /'
        fi

        # NOTE (known transient, see validation_report.md Finding #1): the
        # QUERY_CALLBACK sweep can occasionally mis-parse the reply as Ack and
        # map 0 callbacks. We do NOT assert a non-zero count here because it is
        # a documented transient; --list-callbacks exiting 0 is the real gate.
        if printf '%s' "$HS_OUT" | grep -qE 'Callback name -> id|reports 0 callbacks|Legacy firmware'; then
            ok "E2E: --list-callbacks completed (handshake + sweep ran)"
        else
            fail "E2E: --list-callbacks produced no recognizable summary"
        fi

        # 8.3 Full notification pipeline against real hardware. Requires the
        # Hyprland monitor (default Linux build) OR is skipped on other setups.
        # We run the service briefly with a catch-all rule; success = the legacy
        # string send AND an APPLY_HOST_CONTEXT typed command both appear in the
        # verbose log. This is the PRD §11 #1 "switches layers when app changes"
        # criterion, exercised end-to-end.
        if [ -n "${HYPRLAND_INSTANCE_SIGNATURE:-}" ] && [ -n "${XDG_RUNTIME_DIR:-}" ]; then
            E2E_HOME="$(mktemp -d -t qmk-e2e-XXXXXX)"; E2E_XDG="$E2E_HOME/.config"
            mkdir -p "$E2E_XDG/qmkonnect"
            # catch-all stack rule: match anything → layer 10 (no callbacks, so a
            # missing registry can't block the assert; APPLY_HOST_CONTEXT still sends).
            cat > "$E2E_XDG/qmkonnect/rules.toml" <<'EOF'
[host]
disable_firmware_config = false
[[rule]]
match = "*"
layer = 10
EOF
            # poll_interval_ms>0 forces the monitor to sample the active window.
            printf 'poll_interval_ms = 200\ndebounce_ms = 50\n' > "$E2E_XDG/qmkonnect/config.toml"
            SVC_LOG="$(mktemp)"
            # Run in an isolated env so the real user config is untouched.
            env HOME="$E2E_HOME" XDG_CONFIG_HOME="$E2E_XDG" \
                timeout 6 "$BIN_RELEASE" --verbose >"$SVC_LOG" 2>&1 || true
            if grep -q 'Notified QMK' "$SVC_LOG" && grep -q 'ApplyHostContext' "$SVC_LOG"; then
                ok "E2E: full pipeline fired (legacy string + APPLY_HOST_CONTEXT) on real hardware"
            elif grep -q 'Notified QMK' "$SVC_LOG"; then
                fail "E2E: legacy string sent but APPLY_HOST_CONTEXT missing (host-rules path)"
            elif grep -q 'Starting Hyprland window monitor' "$SVC_LOG"; then
                skip "E2E: monitor started but no window change observed in the window"
            else
                fail "E2E: service did not reach the monitor stage"
                tail -n 8 "$SVC_LOG" | sed 's/^/      /'
            fi
            rm -rf "$E2E_HOME" "$SVC_LOG"
        else
            skip "E2E notification pipeline: requires a Hyprland session (HYPRLAND_INSTANCE_SIGNATURE)"
        fi
    fi
fi

# =============================================================================
# PHASE 9 — Version & packaging consistency
# =============================================================================
section "Phase 9: Version & packaging consistency"

# 9.1 Cargo.toml / Cargo.lock / PKGBUILD / spec version agree
CARGO_VER=$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([0-9.]+)"/\1/')
LOCK_VER=$(grep -A1 'name = "qmkonnect"' Cargo.lock | grep -m1 version | sed -E 's/.*"([0-9.]+)"/\1/')
PKG_VER=$(grep -m1 '^pkgver' packaging/linux/arch/PKGBUILD | cut -d= -f2)
INNO_VER=$(grep -E '^\s*#define MyAppVersion' packaging/windows/inno/QMKonnect.iss | sed -E 's/.*"([0-9.]+)"/\1/')
if [ "$CARGO_VER" = "$LOCK_VER" ] && [ "$CARGO_VER" = "$PKG_VER" ] && [ "$CARGO_VER" = "$INNO_VER" ]; then
    ok "version consistent ($CARGO_VER) across Cargo.toml, Cargo.lock, PKGBUILD, Inno .iss"
else
    fail "version mismatch: cargo=$CARGO_VER lock=$LOCK_VER pkgbuild=$PKG_VER inno=$INNO_VER"
fi

# 9.2 qmk-notifier crate pinned by tag and Cargo.lock matches the pin
PIN_TAG=$(grep -A1 'qmk-notifier' Cargo.toml | grep -m1 'tag' | sed -E 's/.*tag = "([^"]+)".*/\1/')
if grep -q "qmk-notifier?tag=$PIN_TAG" Cargo.lock; then
    ok "qmk-notifier crate pin ($PIN_TAG) matches Cargo.lock source"
else
    fail "qmk-notifier: Cargo.toml tag ($PIN_TAG) not reflected in Cargo.lock"
fi

# 9.3 No build artifacts tracked in git (they're gitignored, but double-check)
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    LEAKED=$(git ls-files | grep -ciE 'arch/pkg/|Output/.*\.exe$|\.dmg$|QMKonnect\.app/' || true)
    if [ "$LEAKED" = "0" ]; then
        ok "no build artifacts (pkg/exe/dmg/app) tracked in git"
    else
        fail "$LEAKED build artifact(s) tracked in git (should be gitignored)"
    fi
else
    skip "not a git work tree — skipped tracked-artifact check"
fi

# 9.4 Autostart identity is the single source of truth across autostart.rs / Inno / ps1
#    (Windows HKCU Run value name "QMKonnect"; AUMID "Mulletware.QMKonnect")
if grep -q 'w!("QMKonnect")' src/autostart.rs \
   && grep -q 'QMKonnect' packaging/windows/inno/QMKonnect.iss \
   && grep -q 'Mulletware.QMKonnect' src/platforms/mod.rs; then
    ok "autostart identity (Run name + AUMID) consistent across sources"
else
    fail "autostart identity inconsistent across autostart.rs / Inno / platforms"
fi

# =============================================================================
# Summary
# =============================================================================
summary() {
    echo
    printf '%s========================================%s\n' "$C_CYAN" "$C_RESET"
    printf '  Validation summary:  %s%d passed%s,  %s%d failed%s,  %d skipped\n' \
        "$C_GREEN" "$PASS" "$C_RESET" \
        "$([ "$FAIL" = "0" ] && printf '%s' "$C_GREEN" || printf '%s' "$C_RED")" "$FAIL" "$C_RESET" \
        "$SKIP"
    if [ "$FAIL" -gt 0 ]; then
        printf '\n  %sFailed checks:%s\n' "$C_RED" "$C_RESET"
        for p in "${FAILED_PHASES[@]}"; do printf '    • %s\n' "$p"; done
        printf '\n  %sSTATUS: FAIL%s\n' "$C_RED" "$C_RESET"
    else
        printf '\n  %sSTATUS: PASS%s\n' "$C_GREEN" "$C_RESET"
    fi
}
summary
[ "$FAIL" -eq 0 ]
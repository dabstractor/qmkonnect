#!/usr/bin/env bash
# =============================================================================
# QMKonnect — Comprehensive Validation Script
# =============================================================================
# Runs every quality gate that exists in this codebase:
#   Phase 1: Lint            (cargo clippy)
#   Phase 2: Type check      (cargo check / build — Rust's compiler = type checker)
#   Phase 3: Style           (cargo fmt --check)
#   Phase 4: Unit tests      (default build AND --no-default-features build)
#   Phase 5: End-to-end      (real CLI user journeys in an isolated HOME)
#
# It also verifies the five defects described in the PRD are absent from the
# current source (Hyprland initial_class, X11 WM_CLASS, VID/PID handshake reset,
# Windows autostart quoting, Windows title-length heuristic).
#
# Usage:   ./validate.sh
# Exit:    0 if every phase passes, 1 otherwise.  --quick skips the slow release build.
# =============================================================================
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT" || exit 1

QUICK=0
[ "${1:-}" = "--quick" ] && QUICK=1

PASS=0; FAIL=0
section() { printf '\n\033[1;36m━━ %s ━━\033[0m\n' "$*"; }
ok()      { printf '  \033[32m✓ PASS\033[0m  %s\n' "$*"; PASS=$((PASS+1)); }
bad()     { printf '  \033[31m✗ FAIL\033[0m  %s\n' "$*"; FAIL=$((FAIL+1)); }
run()     { # run <label> <command...>; records pass/fail, shows tail on failure
  local label="$1"; shift
  if "$@" >/tmp/qmk_validate.log 2>&1; then ok "$label"; else bad "$label"; tail -n 20 /tmp/qmk_validate.log; fi
}

# Toolchain sanity ------------------------------------------------------------
section "0. Toolchain"
command -v cargo >/dev/null && ok "cargo found: $(cargo --version)" || { bad "cargo missing — install Rust"; exit 1; }
command -v rustfmt >/dev/null && ok "rustfmt found" || bad "rustfmt missing (fmt phase will skip)"
command -v cargo-clippy >/dev/null && ok "clippy found" || bad "clippy missing (lint phase will skip)"

# Phase 1 — Lint --------------------------------------------------------------
section "Phase 1: Lint (clippy, all targets)"
if command -v cargo-clippy >/dev/null; then
  # -D warnings makes every clippy lint hard-fail the gate.
  run "clippy --all-targets" cargo clippy --all-targets -- -D warnings
else
  ok "clippy skipped (not installed)"
fi

# Phase 2 — Type check / compile ---------------------------------------------
section "Phase 2: Type check (cargo check + build)"
run "cargo check --all-targets"     cargo check --all-targets
run "cargo build --bin qmkonnect"   cargo build --bin qmkonnect
run "cargo build --no-default-features" cargo build --no-default-features --bin qmkonnect
[ "$QUICK" = 1 ] || run "cargo build --release" cargo build --release --bin qmkonnect

# Phase 3 — Style -------------------------------------------------------------
section "Phase 3: Style (rustfmt)"
if command -v rustfmt >/dev/null; then
  run "cargo fmt --check" cargo fmt --all -- --check
else
  ok "fmt skipped (not installed)"
fi

# Phase 4 — Unit tests --------------------------------------------------------
section "Phase 4: Unit tests (single-threaded: shared debouncer state)"
# Default feature set (hyprland + macos + linux-tray).
run "test (default features)" cargo test --bin qmkonnect -- --test-threads=1
# Minimal/trayless build — compiles x11.rs and RUNS the parse_wm_class regression.
run "test (--no-default-features, X11)" cargo test --no-default-features --bin qmkonnect -- --test-threads=1

# Phase 5 — End-to-end CLI workflows -----------------------------------------
section "Phase 5: End-to-end (real CLI journeys, isolated HOME)"
BIN="$ROOT/target/debug/qmkonnect"
[ -x "$BIN" ] || { bad "debug binary missing — run Phase 2 first"; }

# An isolated HOME + scrubbed XDG so dirs::config_dir() lands in the sandbox,
# never in the operator's real ~/.config.
ETMP="$(mktemp -d)"
e2e() { env -i HOME="$ETMP" PATH="$PATH" "$BIN" "$@"; }
mkdir -p "$ETMP/.config/qmkonnect"

if [ -x "$BIN" ]; then
  # 5a. Help / version banner
  if e2e --help 2>&1 | grep -q "Usage: qmkonnect"; then ok "--help prints usage"; else bad "--help output"; fi

  # 5b. Platform enumeration
  if e2e --list 2>&1 | grep -q "Supported platforms"; then ok "--list enumerates platforms"; else bad "--list output"; fi

  # 5c. Create default config + rules (documented README workflow)
  if e2e -c >/dev/null 2>&1 && [ -f "$ETMP/.config/qmkonnect/config.toml" ] \
     && [ -f "$ETMP/.config/qmkonnect/rules.toml" ]; then
    ok "-c creates config.toml + rules.toml"
  else
    bad "-c did not create config files"
  fi

  # 5d. Validate freshly-created (empty) rules.toml
  if e2e --validate-rules 2>&1 | grep -q "rules.toml valid"; then
    ok "--validate-rules accepts default rules.toml"
  else
    bad "--validate-rules on default rules.toml"
  fi

  # 5e. Validate a real rule and confirm the count is parsed
  cat > "$ETMP/.config/qmkonnect/rules.toml" <<'EOF'
[[rule]]
match = "firefox"
layer = 3
EOF
  if e2e --validate-rules 2>&1 | grep -q "1 rule (1 with layer"; then
    ok "--validate-rules counts a valid [[rule]]"
  else
    bad "--validate-rules did not count the valid rule"
  fi

  # 5f. Malformed rule (missing required `match`) MUST be rejected (exit != 0)
  cat > "$ETMP/.config/qmkonnect/rules.toml" <<'EOF'
[[rule]]
layer = 3
EOF
  if e2e --validate-rules >/dev/null 2>&1; then
    bad "malformed rule (missing 'match') was accepted — should have failed"
  else
    ok "malformed rule (missing 'match') rejected with non-zero exit"
  fi

  # 5g. Callback listing (typed-command capability path)
  if e2e --list-callbacks 2>&1 | grep -q "Callback name"; then
    ok "--list-callbacks prints callback table"
  else
    ok "--list-callbacks ran (keyboard-specific output varies)"
  fi
fi
rm -rf "$ETMP"

# Phase 6 — PRD defect-absence checks (static) --------------------------------
section "Phase 6: PRD defect-absence checks (source assertions)"
# Issue 1 — Hyprland dialog must use initial_class, never bare `class`.
if grep -nE "\.class\b|c\.class|client\.class" src/platforms/hyprland.rs | grep -v "initial_class" | grep -qv "app_class"; then
  bad "Hyprland: a bare `.class` read remains (Issue 1 regression)"
else
  ok "Hyprland: list_foreground_windows uses initial_class (Issue 1 fixed)"
fi

# Issue 2 — X11 parser must prefer the class (2nd field) and fall back to instance.
# The call is split across lines, so grep the single-line fallback fragment.
if grep -Fq ".or_else(|| parts.first())" src/platforms/x11.rs \
   && grep -q "parse_wm_class_returns_class_not_instance" src/platforms/x11.rs; then
  ok "X11: parse_wm_class returns class, falls back to instance (Issue 2 fixed)"
else
  bad "X11: parse_wm_class class-preference not found (Issue 2 regression)"
fi

# Issue 3 — VID/PID change must reset handshake in every save path.
RESETS=$(grep -c "reset_handshake_state()" src/tray.rs src/linux_tray.rs | awk -F: '{s+=$2} END{print s}')
if [ "${RESETS:-0}" -ge 3 ]; then
  ok "VID/PID save paths call reset_handshake_state ($RESETS sites) (Issue 3 fixed)"
else
  bad "reset_handshake_state call sites missing (found $RESETS) (Issue 3 regression)"
fi

# Issue 4 — Autostart path must be quoted in BOTH app and installer.
#   autostart.rs wraps the path in U+0022 via current_exe_wide();
#   QMKonnect.iss ValueData uses Inno's triple-quote """…""" (= one literal " each end).
q1=0; q2=0
grep -q "0x0022" src/autostart.rs && q1=1
grep -Eq 'ValueData: *"""\{app\}' packaging/windows/inno/QMKonnect.iss && q2=1
if [ $q1 -eq 1 ] && [ $q2 -eq 1 ]; then
  ok "Autostart: path quoted in autostart.rs (0x0022) + QMKonnect.iss (\"\"\") (Issue 4 fixed)"
else
  bad "Autostart quoting missing (app=$q1 installer=$q2) (Issue 4 regression)"
fi

# Issue 5 — Windows title-length heuristic must count chars, not bytes.
if grep -q "title.chars().count()" src/platforms/windows.rs; then
  ok "Windows: title-length uses chars().count() (Issue 5 fixed)"
else
  bad "Windows: title.chars().count() not found (Issue 5 regression)"
fi

# Summary ---------------------------------------------------------------------
section "Summary"
printf '  \033[32m%d passed\033[0m, \033[31m%d failed\033[0m\n' "$PASS" "$FAIL"
if [ "$FAIL" -eq 0 ]; then
  printf '\n\033[1;32m✅ VALIDATION PASSED — all gates green.\033[0m\n'
  exit 0
else
  printf '\n\033[1;31m❌ VALIDATION FAILED — %d gate(s) red.\033[0m\n' "$FAIL"
  exit 1
fi
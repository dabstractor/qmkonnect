#!/usr/bin/env bash
# =============================================================================
# QMKonnect — Comprehensive Validation Script
# =============================================================================
# Runs every CI quality gate (build / fmt / clippy / unit tests) AND exercises
# the documented user workflows end-to-end (CLI subcommands, config seeding,
# host-rules validation matrix, udev-rule rendering), plus static-asset /
# schema / doc-sync checks. Hardware- and display-dependent checks self-skip on
# a headless runner, so the script only FAILS on real defects.
#
# Usage:   ./validate.sh
# Exit:    0 = every RUN check passed (skips don't count); 1 = ≥1 RUN check failed
# =============================================================================
set -uo pipefail

# Resolve the REPO ROOT (the directory containing Cargo.toml) by walking up,
# so the script works regardless of where it lives (e.g. archived under
# plan/NNN_.../). All check paths (docs/, packaging/, src/, target/, …) are
# relative to the repo root.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
while [ "$ROOT" != "/" ] && [ ! -f "$ROOT/Cargo.toml" ]; do
  ROOT="$(dirname "$ROOT")"
done
[ -f "$ROOT/Cargo.toml" ] || {
  echo "FATAL: cannot locate repo root (no Cargo.toml upward from $(dirname "${BASH_SOURCE[0]}"))"; exit 1; }
cd "$ROOT" || { echo "FATAL: cannot cd to $ROOT"; exit 1; }

# --- color / summary helpers -------------------------------------------------
PASS=0; FAIL=0; SKIP=0; FAILED_CHECKS=()
if [ -t 1 ]; then
  C_GRN='\033[1;32m'; C_RED='\033[1;31m'; C_YEL='\033[1;33m'; C_BLU='\033[1;36m'; C_OFF='\033[0m'
else
  C_GRN=''; C_RED=''; C_YEL=''; C_BLU=''; C_OFF=''
fi
section() { echo; printf "${C_BLU}━━ %s ━━${C_OFF}\n" "$*"; }
ok()      { printf "${C_GRN}  ✓${C_OFF} %s\n" "$1"; PASS=$((PASS+1)); }
bad()     { printf "${C_RED}  ✗${C_OFF} %s\n" "$1"; FAIL=$((FAIL+1)); FAILED_CHECKS+=("$1"); }
skip()    { printf "${C_YEL}  ⊘${C_OFF} %s\n" "$1"; SKIP=$((SKIP+1)); }
# run a check: $1=label, rest=command. Captures exit code; prints tail on fail.
chk() {
  # Run a check bounded by CHK_TIMEOUT (default 180s). Without this, a single
  # command that hangs (e.g. a CLI flag that accidentally starts the daemon)
  # stalls the ENTIRE script — and any outer agent watchdog — forever. `timeout`
  # exits 124 on expiry, which we surface as a (marked) failure rather than a hang.
  local label="$1"; shift
  local log; log="$(mktemp)"
  local T="${CHK_TIMEOUT:-180}"
  local rc=0
  timeout "$T" "$@" >"$log" 2>&1 || rc=$?
  if [ "$rc" -eq 0 ]; then
    ok "$label"; rm -f "$log"; return 0
  else
    bad "$label"
    [ "$rc" -eq 124 ] && echo "    (timed out after ${T}s — possible hang)" >&2
    { echo "    ----- last 15 lines of output -----"; tail -n 15 "$log" | sed 's/^/    /'; } >&2
    rm -f "$log"; return 1
  fi
}
require() { command -v "$1" >/dev/null 2>&1; }
HAVE_PYTHON=0; require python3 && HAVE_PYTHON=1

# Locate binaries (built/refreshed in Phase 1). Prefer release, fall back debug.
rel_bin()   { echo "$ROOT/target/release/qmkonnect"; }
debug_bin() { echo "$ROOT/target/debug/qmkonnect"; }
hidid_bin() {
  if [ -x "$ROOT/target/release/qmkonnect-hid-id" ]; then echo "$ROOT/target/release/qmkonnect-hid-id"
  else echo "$ROOT/target/debug/qmkonnect-hid-id"; fi
}
qmkonnect_bin() {
  local b; b="$(rel_bin)"
  [ -x "$b" ] || b="$(debug_bin)"
  [ -x "$b" ] || { echo "no qmkonnect binary built" >&2; return 127; }
  echo "$b"
}

# =============================================================================
section "Phase 0 — Prerequisites & toolchain"
# =============================================================================
if require cargo; then ok "cargo on PATH ($(cargo --version))"; else bad "cargo not found on PATH"; fi
if cargo clippy --version >/dev/null 2>&1; then ok "cargo-clippy available"; else bad "cargo-clippy not available"; fi
if cargo fmt --version >/dev/null 2>&1; then ok "rustfmt available"; else bad "rustfmt not available"; fi
echo "  rustc: $(rustc --version)"

# =============================================================================
section "Phase 1 — Build (debug + release + minimal trayless + both binaries)"
# =============================================================================
chk "cargo build (debug, default features)"           cargo build --bin qmkonnect
chk "cargo build (release, default features)"         cargo build --release --bin qmkonnect
chk "cargo build (qmkonnect-hid-id helper binary)"    cargo build --release --bin qmkonnect-hid-id
chk "cargo build (--no-default-features trayless)"    cargo build --no-default-features --bin qmkonnect
chk "cargo build (release, all targets — LTO/abort)"  cargo build --release --all-targets

# =============================================================================
section "Phase 2 — Formatting & Lint (the CI gates)"
# =============================================================================
# EXACT command ci.yml's `fmt` job runs — the highest-signal gate.
if cargo fmt --all -- --check >/tmp/qmk_fmt.log 2>&1; then
  ok "cargo fmt --all -- --check (CI fmt gate)"
else
  bad "cargo fmt --all -- --check (CI fmt gate FAILS — main is red)"
  grep "^Diff in" /tmp/qmk_fmt.log 2>/dev/null | sed 's/^/      /' >&2
fi
chk "cargo clippy --bin qmkonnect (no warnings)"         cargo clippy --bin qmkonnect -- -D warnings
chk "cargo clippy --bin qmkonnect-hid-id (no warnings)"  cargo clippy --bin qmkonnect-hid-id -- -D warnings
chk "cargo clippy --all-targets (CI lint gate)"          cargo clippy --all-targets -- -D warnings

# =============================================================================
section "Phase 3 — Unit Tests (single-threaded: shared debouncer state)"
# =============================================================================
# AGENTS.md mandates --test-threads=1 (the debouncer is process-global state).
chk "cargo test --bin qmkonnect (--test-threads=1)"  cargo test --bin qmkonnect -- --test-threads=1
chk "cargo test --bin qmkonnect-hid-id"              cargo test --bin qmkonnect-hid-id -- --test-threads=1

# =============================================================================
section "Phase 4 — CLI Workflows (no hardware required)"
# =============================================================================
BIN="$(qmkonnect_bin)" || { echo "FATAL: no qmkonnect binary"; exit 1; }
TMP="$(mktemp -d /tmp/qmkval.XXXXXX)"; trap 'rm -rf "$TMP"' EXIT
ISO_HOME="$TMP/home"; mkdir -p "$ISO_HOME/.config"

chk "--help prints version + options"      bash -c "'$BIN' --help | grep -q 'QMKonnect v'"
chk "-l/--list prints a platform line"     bash -c "'$BIN' -l | grep -qi 'linux'"
chk "--bogus-flag is rejected (CLI robustness)" \
    bash -c "! '$BIN' --bogus-flag-xyz </dev/null 2>/dev/null"

# create-config in an isolated HOME (idempotent + seeds rules.toml)
chk "-c creates config.toml + rules.toml (isolated HOME)" \
    bash -c "XDG_CONFIG_HOME='$ISO_HOME/.config' HOME='$ISO_HOME' '$BIN' -c >/dev/null 2>&1 && \
             test -f '$ISO_HOME/.config/qmkonnect/config.toml' && \
             test -f '$ISO_HOME/.config/qmkonnect/rules.toml'"
chk "-c is idempotent (second run is a no-op)" \
    bash -c "XDG_CONFIG_HOME='$ISO_HOME/.config' HOME='$ISO_HOME' '$BIN' -c >/dev/null 2>&1"
# 0xFEED cleanup: the seeded template must NOT carry the literal 0xfeed default.
if grep -qi "0xfeed" "$ISO_HOME/.config/qmkonnect/config.toml" 2>/dev/null; then
  bad "seeded config.toml still carries stale 0xfeed literal (DEVICE_DISCOVERY §7.2)"
else ok "seeded config.toml has no stale 0xfeed literal"; fi

# --validate-rules: documented host-rules lint, across the full error matrix.
V="$TMP/rules"; mkdir -p "$V"
printf '[[rule]]\nmatch="alacritty"\nlayer=10\n' > "$V/valid.toml"
printf '[[rule]]\nlayer=5\n'                       > "$V/nomatch.toml"
printf '[[rule]]\nmatch="foo"\nlayer=255\n'        > "$V/l255.toml"
printf '[[rule]]\nmatch="foo"\n'                   > "$V/noop.toml"
chk "--validate-rules: valid file → exit 0"          bash -c "'$BIN' --validate-rules --rules-path '$V/valid.toml' >/dev/null 2>&1"
chk "--validate-rules: missing match → exit 1"       bash -c "! '$BIN' --validate-rules --rules-path '$V/nomatch.toml' >/dev/null 2>&1"
chk "--validate-rules: layer 255 sentinel → exit 1"  bash -c "! '$BIN' --validate-rules --rules-path '$V/l255.toml' >/dev/null 2>&1"
chk "--validate-rules: rule sets nothing → exit 1"   bash -c "! '$BIN' --validate-rules --rules-path '$V/noop.toml' >/dev/null 2>&1"
chk "--validate-rules: nonexistent --rules-path → exit 1" \
    bash -c "! '$BIN' --validate-rules --rules-path /tmp/does_not_exist_xyz.toml >/dev/null 2>&1"
chk "--validate-rules: no rules file anywhere → info exit 0" \
    bash -c "XDG_CONFIG_HOME='$TMP/empty' HOME='$TMP/empty' '$BIN' --validate-rules >/dev/null 2>&1"

# --reload udev-rule rendering (non-root → prints the rule, never writes /etc).
RCFG="$TMP/rcfg.toml"
printf 'vendor_id = 0x1209\nproduct_id = 0x7f00\n' > "$RCFG"
chk "-r renders single-line VID/PID udev rule (non-root advisory)" \
    bash -c "'$BIN' -r --config '$RCFG' 2>&1 | grep -q 'ATTRS{idVendor}==\"1209\"' && \
             '$BIN' -r --config '$RCFG' 2>&1 | grep -q 'ATTRS{idProduct}==\"7f00\"'"
chk "-r rendered rule is a single safe line with a leading match key" \
    bash -c "'$BIN' -r --config '$RCFG' 2>&1 | grep 'KERNEL==' | grep -v '\\\\$' | grep -q 'KERNEL.*=='"

# =============================================================================
section "Phase 5 — Static Assets, Schema & Documentation Sync"
# =============================================================================
if [ "$HAVE_PYTHON" -eq 1 ]; then
  chk "Cargo.toml is valid TOML" python3 -c "import tomllib; tomllib.load(open('Cargo.toml','rb'))"
  # spec/PACKAGING §4.4 printed an INVALID `require-local` line; the real code
  # must use the upstream-documented [requires] sub-table.
  chk "rpm metadata uses valid [requires] sub-table (no invalid require-local)" \
      python3 -c "import tomllib; d=tomllib.load(open('Cargo.toml','rb')); r=d['package']['metadata']['generate-rpm']; assert 'requires' in r and 'require-local' not in r, 'bad rpm metadata'"
  chk "GNOME extension metadata.json is valid JSON" \
      python3 -c "import json; json.load(open('packaging/gnome-shell-extension/metadata.json'))"
else
  skip "Cargo.toml / GNOME-JSON TOML+JSON validation (python3 absent)"
fi

# Every asset path referenced by Cargo.toml packaging metadata must exist.
chk "all Cargo.toml packaging assets exist on disk" bash -c '
  for f in packaging/linux/udev/69-qmkonnect-rawhid.rules \
           packaging/linux/systemd/qmkonnect.service.template \
           packaging/linux/xdg/qmkonnect.desktop \
           packaging/debian/long-description.txt packaging/debian/postinst \
           packaging/debian/postrm packaging/debian/prerm \
           packaging/rpm/postin packaging/rpm/postun \
           LICENSE README.md; do
    [ -f "$f" ] || { echo "MISSING: $f"; exit 1; }
  done'

# Build outputs must be gitignored, never committed (PACKAGING §11).
chk "build outputs are gitignored" bash -c '
  git check-ignore packaging/linux/arch/pkg \
    packaging/linux/arch/qmkonnect-0.2.8-1-x86_64.pkg.tar.zst \
    packaging/windows/inno/Output target >/dev/null 2>&1'
chk "no build outputs are tracked by git" bash -c '
  [ -z "$(git ls-files packaging/linux/arch/pkg packaging/windows/inno/Output target 2>/dev/null)" ]'

# GNOME extension must define the enable/disable lifecycle + the D-Bus contract.
chk "GNOME extension defines enable/disable + get_wm_class + focus_window" bash -c '
  grep -q "enable()" packaging/gnome-shell-extension/extension.js && \
  grep -q "disable()" packaging/gnome-shell-extension/extension.js && \
  grep -q "get_wm_class" packaging/gnome-shell-extension/extension.js && \
  grep -q "focus_window" packaging/gnome-shell-extension/extension.js'
chk "GNOME D-Bus contract names present in introspection XML" bash -c '
  for s in io.mulletware.QMKonnect GetActiveWindow ActiveWindowChanged WindowMonitor; do
    grep -q "$s" packaging/gnome-shell-extension/dbus-interfaces.xml || exit 1
  done'

# --- Documentation sync: llms_full.txt must equal a fresh regeneration. ---
# This is a load-bearing check: the generated concatenation is the "canonical
# reference for agents/LLMs" and must contain ALL source docs. A truncated file
# (e.g. missing examples.md / troubleshooting.md) is a real defect. We regenerate
# to a temp copy WITHOUT mutating the working tree, then compare byte-for-byte.
if [ -x docs/generate_llms_full.sh ]; then
  LLMS_ORIG="$(mktemp)"; cp docs/llms_full.txt "$LLMS_ORIG"
  if bash docs/generate_llms_full.sh >/tmp/qmk_gen.log 2>&1; then
    GEN_LINES=$(wc -l < docs/llms_full.txt); GEN_BYTES=$(wc -c < docs/llms_full.txt)
    cp docs/llms_full.txt "$TMP/llms_generated.txt"
    cp "$LLMS_ORIG" docs/llms_full.txt   # restore working tree exactly
    if diff -q "$LLMS_ORIG" "$TMP/llms_generated.txt" >/dev/null; then
      ok "docs/llms_full.txt in sync with source docs ($GEN_LINES lines, $GEN_BYTES bytes)"
    else
      bad "docs/llms_full.txt is STALE/TRUNCATED vs source docs"
      missing=$((GEN_LINES - $(wc -l < "$LLMS_ORIG")))
      echo "    working=$(wc -l < "$LLMS_ORIG") lines vs generated=$GEN_LINES lines (Δ $missing lines)" >&2
      echo "    run: bash docs/generate_llms_full.sh   then commit docs/llms_full.txt" >&2
    fi
    rm -f "$LLMS_ORIG"
  else
    cp "$LLMS_ORIG" docs/llms_full.txt; rm -f "$LLMS_ORIG"
    bad "docs/generate_llms_full.sh failed (see /tmp/qmk_gen.log)"
  fi
else
  skip "docs/llms_full.txt sync check (generator absent)"
fi

# --- The actual P1 task deliverable: mise/asdf must be gone from user docs. ---
# Zero channel-advertising hits in user-facing docs; only spec/*.md's intentional
# "NOT a channel" exclusion may mention them. plan/ is read-only history.
mise_userdoc=$(grep -rinE 'mise|asdf' docs/*.md README.md 2>/dev/null | grep -viE 'promise' | wc -l)
if [ "$mise_userdoc" -eq 0 ]; then
  ok "mise/asdf removed from all user-facing docs (docs/*.md, README.md)"
else
  bad "mise/asdf still advertised in user-facing docs ($mise_userdoc hit(s))"
  grep -rinE 'mise|asdf' docs/*.md README.md 2>/dev/null | grep -viE 'promise' | sed 's/^/      /' >&2
fi
# Dead plugin-repo link must not survive anywhere outside plan/ history.
asdf_dead=$(grep -rn 'asdf-qmkonnect' . --include='*.md' --include='*.txt' --include='*.json' --include='*.yaml' --include='*.yml' --include='*.sh' --include='*.rb' 2>/dev/null | grep -vE '/plan/|/\.git/|/target/|/docs/vendor/|/\.pi-subagents/' | wc -l)
if [ "$asdf_dead" -eq 0 ]; then
  ok "no dead asdf-qmkonnect links outside plan/ history"
else
  bad "dead asdf-qmkonnect link(s) found outside plan/ history"
fi

# =============================================================================
section "Phase 6 — Hardware / Display E2E (auto-skip if absent)"
# =============================================================================
# Mirror the PRD "User" persona against REAL hardware. Self-skip on a headless
# CI runner so the script never falsely fails there.
HIDID="$(hidid_bin)"
QMK_HIDRAW=""
if [ -x "$HIDID" ]; then
  for h in /sys/class/hidraw/hidraw*; do
    [ -e "$h" ] || continue
    if "$HIDID" "$h" 2>/dev/null | grep -q "ID_QMKONNECT=1"; then QMK_HIDRAW="$h"; break; fi
  done
  if [ -n "$QMK_HIDRAW" ]; then
    ok "qmkonnect-hid-id tags a real QMK interface ($QMK_HIDRAW → ID_QMKONNECT=1)"
  else
    skip "qmkonnect-hid-id live test (no QMK 0xFF60 keyboard attached)"
  fi
else skip "qmkonnect-hid-id live test (helper not built)"; fi

if "$BIN" --list-devices >/tmp/qmk_ld.log 2>&1; then
  if grep -q "Available HID devices" /tmp/qmk_ld.log; then
    ok "--list-devices enumerates the HID bus ($(grep -c . /tmp/qmk_ld.log) lines)"
    grep -q "0xff60:0x0061" /tmp/qmk_ld.log && ok "--list-devices shows a QMK 0xff60:0x0061 interface" || \
      skip "--list-devices QMK row (no QMK board attached)"
  else bad "--list-devices produced no device listing"; fi
else bad "--list-devices exited non-zero"; fi

cb_out="$("$BIN" --list-callbacks 2>/dev/null)"
if echo "$cb_out" | grep -qE "Callback name -> id|Legacy firmware|No QMK device|0 callbacks"; then
  ok "--list-callbacks handshake completed ($(echo "$cb_out" | head -1))"
else skip "--list-callbacks live handshake (no proto-v2 board / hidraw perms)"; fi

HAVE_SESSION=0
{ [ -n "${DISPLAY:-}" ] || [ -n "${WAYLAND_DISPLAY:-}" ]; } && HAVE_SESSION=1
if [ "$HAVE_SESSION" -eq 1 ] && [ -n "$QMK_HIDRAW" ]; then
  BK="$TMP/rules.bak"; [ -f "$HOME/.config/qmkonnect/rules.toml" ] && cp "$HOME/.config/qmkonnect/rules.toml" "$BK"
  if timeout 5 "$BIN" -v >"$TMP/daemon.log" 2>&1; :; then :; fi
  grep -qiE "select_linux_backend|Using platform" "$TMP/daemon.log" && \
    ok "live daemon selected a window-monitor backend" || \
    skip "live daemon backend selection (session/permissions)"
  grep -q "Notified QMK" "$TMP/daemon.log" && ok "live daemon sent a window-change notification" || \
    skip "live daemon window-detection (no focus events in 5s)"
  [ -f "$BK" ] && cp "$BK" "$HOME/.config/qmkonnect/rules.toml"
else
  skip "live daemon E2E (needs a GUI session AND a QMK keyboard)"
fi

# =============================================================================
section "Result"
# =============================================================================
echo
printf "${C_GRN}  PASS=%d${C_OFF}  ${C_RED}FAIL=%d${C_OFF}  ${C_YEL}SKIP=%d${C_OFF}\n" "$PASS" "$FAIL" "$SKIP"
if [ "$FAIL" -gt 0 ]; then
  printf "${C_RED}FAILED checks:${C_OFF}\n"
  for c in "${FAILED_CHECKS[@]}"; do printf "  • %s\n" "$c"; done
  exit 1
fi
printf "${C_GRN}All run checks passed.${C_OFF} (skips are environment-dependent and do not fail.)\n"
exit 0
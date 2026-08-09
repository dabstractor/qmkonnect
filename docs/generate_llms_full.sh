#!/usr/bin/env bash
# Regenerate docs/llms_full.txt — a single-file concatenation of QMKonnect's
# documentation, for agents/LLMs. Run after editing README.md or any docs/*.md:
#   bash docs/generate_llms_full.sh && git diff --stat docs/llms_full.txt
set -euo pipefail
DOCS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$DOCS_DIR/.." && pwd)"
OUT="$DOCS_DIR/llms_full.txt"
DIV="$(printf '%0.s=' $(seq 1 80))"
# Strip a LEADING Jekyll front-matter block (--- ... ---) if line 1 is '---'.
# Files without front matter (e.g. repo README.md) are passed through whole.
strip_fm() {
  awk 'NR==1 && /^---[[:space:]]*$/ {fm=1; next} fm && /^---[[:space:]]*$/ {fm=0; next} fm {next} {print}' "$1"
}
emit() {  # <number-and-path>  [<label>]
  printf '\n%s\n %s%s\n%s\n\n' "$DIV" "$1" "${2:+ ($2)}" "$DIV"
}
{
  cat <<'HDR'
# QMKonnect - Complete Documentation (for Agents / LLMs)

This is a single-file concatenation of QMKonnect's documentation, generated from
the source Markdown in this repository. It is the canonical reference for agents
and LLMs.

IMPORTANT REALITY CHECK
-----------------------
QMKonnect is only ONE HALF of a two-part system. It detects the active window
and SENDS that information to your keyboard over Raw HID. Your keyboard cannot
react to it unless the companion **qmk_notifier** module is built into your QMK
firmware. That firmware setup is REQUIRED, not optional.

When the docs say "no configuration" or "zero-config", they refer ONLY to the
desktop app's vendor/product-ID selection (a single standard QMK keyboard is
auto-discovered). The firmware configuration is mandatory.
HDR
  emit "1. README.md";            strip_fm "$ROOT/README.md"
  emit "2. docs/index.md"            "Home"
  strip_fm "$DOCS_DIR/index.md"
  emit "3. docs/installation.md"     "Installation"
  strip_fm "$DOCS_DIR/installation.md"
  emit "4. docs/qmk-integration.md"  "QMK Integration - REQUIRED firmware setup"
  strip_fm "$DOCS_DIR/qmk-integration.md"
  emit "5. docs/configuration.md"    "Desktop-side Configuration"
  strip_fm "$DOCS_DIR/configuration.md"
  emit "6. docs/usage.md"            "Usage"
  strip_fm "$DOCS_DIR/usage.md"
  # NOTE: docs/examples.md (firmware examples) and docs/troubleshooting.md are
  # DELIBERATELY excluded from this agent/LLM reference artifact (commit f7617cc,
  # "Strip firmware examples from llms_full.txt artifact"). The committed
  # llms_full.txt ends at this section; emitting them here would make the
  # validate.sh sync check (byte-for-byte vs regeneration) fail. To re-include
  # either source doc, add its emit/strip_fm pair back HERE and regenerate.
} > "$OUT"
echo "wrote $OUT ($(wc -l < "$OUT") lines, $(wc -c < "$OUT") bytes)"
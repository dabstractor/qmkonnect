# Research Notes — P2.M1.T1.S1: Port `process_escapes()` to Rust

## 1. Canonical source (the firmware C — the single source of truth)

**File:** `/home/dustin/projects/qmk-notifier/pattern_match.c` lines 30-75
(`static char *process_escapes(const char *pattern)`).

The function is a **byte transform**: it walks the input pattern left-to-right,
emitting a processed byte stream consumed downstream by the Thompson NFA
compiler. It is `static` (file-local) in the firmware; in the Rust port it
becomes an internal helper of `src/core/pattern.rs`, consumed by
`parse_pattern()` (P2.M1.T1.S2).

### Exact C logic (transcribed verbatim from pattern_match.c:30-75)

State: `bool last_consumable = false;` (did the previous emitted element
consume a char?)

For each byte (C iterates `char *src` with `while (*src)`, NUL-terminated):

| Input (C)                      | Output byte(s)        | last_consumable after | Branch |
|--------------------------------|-----------------------|-----------------------|--------|
| `\\` then `^`                  | `0x01`                | `true`                | escape `\^` |
| `\\` then `$`                  | `0x02`                | `true`                | escape `\$` |
| `\\` then `*`                  | `0x03`                | `true`                | escape `\*` |
| `\\` then `\\`                 | `0x04`                | `true`                | escape `\\` |
| `\\` then `.`                  | `0x2E` (`.`)          | `true`                | escape `\.` → **literal dot, NOT 0x0D** |
| `\\` then `+`                  | `0x2B` (`+`)          | `true`                | escape `\+` → **literal plus, NOT 0x0E** |
| `\\` then `d`                  | `0x05`                | `true`                | class `\d` |
| `\\` then `D`                  | `0x06`                | `true`                | class `\D` |
| `\\` then `w`                  | `0x07`                | `true`                | class `\w` |
| `\\` then `W`                  | `0x08`                | `true`                | class `\W` |
| `\\` then `s`                  | `0x09`                | `true`                | class `\s` |
| `\\` then `S`                  | `0x0A`                | `true`                | class `\S` |
| `\\` then `b`                  | `0x0B`                | **`false`**           | assertion `\b` (zero-width) |
| `\\` then `B`                  | `0x0C`                | **`false`**           | assertion `\B` (zero-width) |
| `\\` then <other>              | `0x5C` + <other byte> | `true`                | **unrecognized escape → keep BOTH bytes literally (2 output bytes)** |
| `\\` at end-of-string (no next)| `0x5C`                | `true`                | trailing lone backslash → literal `\` |
| bare `*`                       | `0x2A`                | **`false`**           | glob wildcard |
| bare `+`, last_consumable=true | `0x0E`                | **`false`**           | quantifier marker (`X+`) |
| bare `+`, last_consumable=false| `0x2B` (`+`)          | `true`                | literal plus (not after consumable) |
| bare `.`                       | `0x0D`                | `true`                | dot metacharacter |
| anything else                  | the byte itself       | `true`                | ordinary literal |

Output length is ALWAYS ≤ input length (escapes shrink by ≥1; `.`/`+`/literal
passthrough is 1:1; unrecognized escape is the only ≥1:1 case at 2:2).

The C result is NUL-terminated (`*dst = '\0'`) because downstream C treats it
as a C string. **In Rust we use `Vec<u8>` whose length is the authoritative
end marker — do NOT append a trailing 0x00.**

## 2. Byte-contract corroboration (3 independent sources agree)

1. C source `pattern_match.c:30-75` (above).
2. Firmware arch doc `qmk-notifier/plan/001_e329fbe4ae4d/architecture/
   pattern_match_architecture.md` lines 66-95 — table with `0x0E`, `0x01-0x04`,
   `0x05-0x0A`, `0x0B/0x0C`, `0x0D`, `0x2A`, plus the `\. \+ → literal` note
   (line 87) and the "Unrecognized → emit `\\` + char" note (line 90).
3. QMKonnect `plan/002_637d65b6e9b8/architecture/external_deps.md` §3.

All three agree. The C source is authoritative where they differ in wording.

## 3. Three non-obvious fidelity gotchas (MUST get right)

### GOTCHA-A — `\. ` and `\+` emit LITERAL bytes, not placeholders
The work-item description's "Literal . and + emitted as ordinary ASCII
0x2E/0x2B" refers to BOTH the escape forms (`\. `, `\+`) AND bare `+`-after-
non-consumable. The placeholder `0x0D` is only for BARE `.`; the placeholder
`0x0E` is only for BARE `+` after a consumable element. An implementer who
maps `\. ` → `0x0D` or `\+` → `0x0E` will silently break parity.

### GOTCHA-B — Unrecognized escapes keep BOTH bytes (2 output bytes)
`\x`, `\z`, `\q`, etc. → emit `0x5C` (`\\`) followed by the char byte. This is
the ONLY case where output can equal input length for an escape. It is NOT an
error; the NFA then treats `\\` as a literal backslash char to match.
(Confirmed by firmware arch doc line 90 + C `default:` branch.)

### GOTCHA-C — Iterate BYTES, and stop at the first NUL (C-parity)
- Iterate `pattern.as_bytes()`, NOT `pattern.chars()`. UTF-8 continuation
  bytes are all ≥ 0x80, so they never collide with ASCII metacharacters
  (`\ ^ $ * . + d D w W s S b B`, all < 0x80). Byte iteration reproduces the
  C `char*` walk exactly; char iteration would break the peek-ahead and the
  multi-byte passthrough.
- The C loop is `while (*src)` — it STOPS at the first NUL (0x00) byte. To
  mirror this exactly, the Rust port must `break` when it encounters a 0x00
  byte (do NOT emit it as a literal). Valid TOML patterns never contain NUL,
  so this is defensive, but it guarantees byte-for-byte firmware parity for
  ALL inputs and prevents an implementer's own NUL test from diverging.

## 4. `last_consumable` transition table (for quantifier disambiguation)

Sets `last_consumable = true`: every escape except `\b`/`\B`; bare `.`; bare
literal `+` (the non-quantifier case); ordinary literals; trailing backslash;
unrecognized escapes.

Sets `last_consumable = false`: `\b`, `\B`, bare `*` (glob), and the `+`
quantifier marker (0x0E) itself.

Consequence — a bare `+` is a LITERAL (`0x2B`) whenever the immediately
preceding emitted element was `*`, `\b`, `\B`, a prior `+`-quantifier, OR at
the start of the pattern (initial `last_consumable = false`). It is the
quantifier (`0x0E`) only after a char/class/dot/escaped-literal.

## 5. Concrete parity test vectors (computed from the C source)

These are the exact expected `Vec<u8>` outputs. Rust string-literal escapes
shown for clarity (`\\` in a Rust literal = one backslash byte 0x5C).

| Rust input literal | C equivalent | Expected `Vec<u8>` (hex) |
|--------------------|--------------|--------------------------|
| `"hello"`          | `"hello"`    | `[68 65 6C 6C 6F]` |
| `""`               | `""`         | `[]` (empty) |
| `"\\^"`            | `"\^"`       | `[01]` |
| `"\\$"`            | `"\$"`       | `[02]` |
| `"\\*"`            | `"\*"`       | `[03]` |
| `"\\\\"`           | `"\\"`       | `[04]` |
| `"\\."`            | `"\."`       | `[2E]` (literal dot — GOTCHA-A) |
| `"\\+"`            | `"\+"`       | `[2B]` (literal plus — GOTCHA-A) |
| `"\\d"`            | `"\d"`       | `[05]` |
| `"\\D"`            | `"\D"`       | `[06]` |
| `"\\w"`            | `"\w"`       | `[07]` |
| `"\\W"`            | `"\W"`       | `[08]` |
| `"\\s"`            | `"\s"`       | `[09]` |
| `"\\S"`            | `"\S"`       | `[0A]` |
| `"\\b"`            | `"\b"`       | `[0B]` |
| `"\\B"`            | `"\B"`       | `[0C]` |
| `"."`              | `"."`        | `[0D]` |
| `"*"`              | `"*"`        | `[2A]` |
| `"+"`              | `"+"`        | `[2B]` (start: non-consumable → literal) |
| `"a+"`             | `"a+"`       | `[61 0E]` |
| `"a+b"`            | `"a+b"`      | `[61 0E 62]` |
| `"a*"`             | `"a*"`       | `[61 2A]` |
| `".*"`             | `".*"`       | `[0D 2A]` |
| `".+"`             | `".+"`       | `[0D 0E]` |
| `"\\d+"`           | `"\d+"`      | `[05 0E]` |
| `"a*+"`            | `"a*+"`      | `[61 2A 2B]` (+ after * → literal) |
| `"\\b+"`           | `"\b+"`      | `[0B 2B]` (+ after \b → literal) |
| `"\\B+"`           | `"\B+"`      | `[0C 2B]` (+ after \B → literal) |
| `"a++"`            | `"a++"`      | `[61 0E 2B]` (2nd + after quantifier → literal) |
| `"\\x"`            | `"\x"`       | `[5C 78]` (unrecognized — GOTCHA-B, 2 bytes) |
| `"\\z"`            | `"\z"`       | `[5C 7A]` |
| `"abc\\"`          | `"abc\"`     | `[61 62 63 5C]` (trailing lone backslash → literal) |
| `"\\^\\$"`         | `"\^\$"`     | `[01 02]` |

These vectors cover every branch of the C switch + the three gotchas. They are
the parity contract for this task's unit tests.

## 6. Rust conventions observed in this crate

- Module registration: `src/core/mod.rs` currently declares `pub mod notifier;`
  `pub mod types;`. New module → add `pub mod pattern;` (work-item spec).
- Tests: inline `#[cfg(test)] mod tests { use super::*; ... }` per file
  (see `src/core/types.rs`, `src/core/notifier.rs:374`). Assert with
  `assert_eq!`. `#[derive(Debug, PartialEq)]` on structs (types.rs).
- Single-threaded test run is MANDATORY for the crate (shared debouncer state
  in notifier.rs): `cargo test --bin qmkonnect -- --test-threads=1` (AGENTS.md).
  A pure-function module like `pattern` has no shared state, so its own tests
  are thread-safe, but the crate-wide flag must still be used to avoid racing
  the notifier tests.
- Edition 2021, MSRV 1.88, no new deps needed (byte iteration is stdlib).
- `proptest` is a dev-dependency — available for property-based fuzzing if
  desired (optional; the deterministic vector table above is sufficient).

## 7. Downstream contract (what P2.M1.T1.S2 / parse_pattern expects)

- `process_escapes(&str) -> Vec<u8>` — the processed byte stream, NO trailing
  NUL (length is authoritative). `parse_pattern` will detect `^`/`$` anchors on
  the ORIGINAL pattern (anchors are stripped BEFORE calling process_escapes in
  the firmware — see arch doc line 24, "core between anchors fed to
  process_escapes"), so this function does NOT need to handle anchors
  specially; `^`/`$` as bare bytes are NOT special here (there is no `case '^'`
  or `case '$'` for bare bytes in process_escapes — only the escaped forms).
  Bare `^` and `$` pass through as ordinary literals `0x5E`/`0x24`.
- IMPORTANT SCOPE BOUNDARY: process_escapes does NOT know about anchors. Anchor
  detection is parse_pattern's job (next subtask). Do not pre-empt it here.

## 8. No external research needed

This is a faithful 1:1 port of an existing, tested C function. There is no novel
algorithm design (the Thompson NFA comes in P2.M1.T2). No third-party crate is
required — byte iteration + a match on the post-backslash byte is pure stdlib.
The "library docs" here ARE the firmware C source + its arch doc, both captured
above.
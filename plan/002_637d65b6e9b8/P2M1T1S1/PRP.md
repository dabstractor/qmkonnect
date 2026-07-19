# PRP — P2.M1.T1.S1: Port `process_escapes()` to Rust — escape/class/quantifier byte transform

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This is a **greenfield module**
> (`src/core/pattern.rs`) — no existing Rust pattern code. It is the **first**
> subtask of the P2.M1 "Pattern Matcher Port" milestone (full firmware-parity
> matcher, PRD §4 + §14). It produces the byte-transform primitive that
> **P2.M1.T1.S2** (`parse_pattern`, anchor detection) consumes.

---

## Goal

**Feature Goal**: Port the firmware `process_escapes(const char *pattern)`
(`qmk-notifier/pattern_match.c:30-75`) to Rust as
`fn process_escapes(pattern: &str) -> Vec<u8>` with **byte-for-byte identical
output** for every input, including the three non-obvious fidelity cases
(literal-dot/literal-plus escapes; unrecognized-escape two-byte passthrough;
NUL-stop parity). This is a pure byte transform: it does NOT compile an NFA,
does NOT detect anchors, and does NOT match anything.

**Deliverable**: `src/core/pattern.rs` (new file) containing:
1. `pub(crate) fn process_escapes(pattern: &str) -> Vec<u8>` mirroring the C
   function exactly;
2. a rustdoc comment on it documenting the placeholder-byte contract;
3. an inline `#[cfg(test)] mod tests` block asserting the exact `Vec<u8>`
   output for all branches (the parity vector table from research).
Plus one line added to `src/core/mod.rs`: `pub mod pattern;`.

**Success Definition**:
- For every input pattern, the Rust output equals the firmware C output (the
  vector table in research/notes.md §5 is the parity contract and must all pass).
- `cargo build --bin qmkonnect` compiles clean (no warnings on the new file).
- `cargo test --bin qmkonnect -- --test-threads=1` passes, including the new
  `pattern::tests` module.
- `git diff` touches ONLY `src/core/pattern.rs` (new) and `src/core/mod.rs`
  (one-line registration). No anchors, no NFA, no `pattern_match()` — those are
  later subtasks.

## User Persona (if applicable)

**Target User**: Downstream callers in the `pattern` module itself — primarily
`parse_pattern()` (P2.M1.T1.S2), which strips `^`/`$` anchors from the original
pattern string and feeds the core substring to `process_escapes()`. Not a
user-facing API.

**Use Case**: Transform a human-authored pattern string (from `rules.toml`,
e.g. `"^Firefox$"` or `"\\d+-\\d+"`) into the placeholder-byte stream the
Thompson NFA compiler (P2.M1.T2.S1) consumes. Example:
`process_escapes("\\d+")` → `vec![0x05, 0x0E]` (`\d` class then `+` quantifier).

**Pain Points Addressed**: Establishes the canonical byte encoding shared
between the escape processor, the NFA compiler, and the matcher, so all three
agree on what each placeholder byte means (the firmware's "processed-pattern
byte contract", PRD §4 + arch `external_deps.md` §3).

## Why

- **Foundation of the full-parity matcher.** PRD §14 mandates the Rust matcher
  be a *"full-parity port of the firmware `pattern_match.c`, not a subset"*
  with *"the firmware matcher + its test corpus the single source of truth for
  match semantics."* `process_escapes` is the entry point of that pipeline;
  a byte-level divergence here would silently corrupt every downstream
  compilation/match. Faithfulness at this layer is non-negotiable.
- **Encodes the shared byte vocabulary.** The placeholder bytes (0x01–0x0E,
  0x2A) are the contract between escape-processing and NFA-compilation. Getting
  them exactly right now lets P2.M1.T2 (NFA) and P2.M1.T4 (parity tests)
  proceed against a stable encoding.
- **No reflash, host-side rules.** Per the P2/P3/P4 milestones, rules move to a
  host-side `rules.toml`; the matcher must run on the desktop. This task begins
  that port.

## What

Create `src/core/pattern.rs` with a single public-to-crate function
`process_escapes(&str) -> Vec<u8>` that reproduces the firmware C function
verbatim in semantics. It walks the input **bytes** left-to-right, tracking a
`last_consumable: bool` (initial `false`), and emits processed bytes per the
table below. It **stops at the first NUL (0x00) byte** (mirrors C `while(*src)`)
and **does not append a trailing NUL** (Rust `Vec` length is authoritative).

### Processed-pattern byte contract (authoritative: C source + arch doc §3)

| Output byte(s) | Source form            | Meaning / downstream NFA op                 | last_consumable after |
|----------------|------------------------|---------------------------------------------|-----------------------|
| `0x01`         | `\^`                   | escaped literal `^`                         | `true`  |
| `0x02`         | `\$`                   | escaped literal `$`                         | `true`  |
| `0x03`         | `\*`                   | escaped literal `*`                         | `true`  |
| `0x04`         | `\\`                   | escaped literal `\`                         | `true`  |
| `0x2E` (`.`)   | `\.`                   | **literal dot** (NOT the 0x0D metachar)     | `true`  |
| `0x2B` (`+`)   | `\+`                   | **literal plus** (NOT the 0x0E quantifier)  | `true`  |
| `0x05`–`0x0A`  | `\d \D \w \W \s \S`    | char classes                                | `true`  |
| `0x0B`         | `\b`                   | word-boundary assertion (zero-width)        | `false` |
| `0x0C`         | `\B`                   | non-boundary assertion (zero-width)         | `false` |
| `0x2A` (`*`)   | bare `*`               | glob wildcard                               | `false` |
| `0x0E`         | bare `+` after consumable | `X+` quantifier marker                    | `false` |
| `0x2B` (`+`)   | bare `+` not after consumable | literal plus                          | `true`  |
| `0x0D`         | bare `.`               | dot metachar (any char except `\n`/`\r`)    | `true`  |
| `0x5C` + byte  | `\<unrecognized>`      | keep backslash + char (2 bytes)             | `true`  |
| `0x5C`         | trailing lone `\`      | literal backslash                           | `true`  |
| the byte       | anything else          | ordinary literal (incl. bare `^` `=` `0x5E`, bare `$` = `0x24`) | `true` |

### Success Criteria
- [ ] `src/core/pattern.rs` exists with `pub(crate) fn process_escapes(pattern: &str) -> Vec<u8>`.
- [ ] Output is **byte-identical** to the firmware C for every input in the
      parity vector table (`research/notes.md` §5) — all of those `assert_eq!`
      pass.
- [ ] The three fidelity gotchas are correct:
      (A) `\.` → `0x2E` and `\+` → `0x2B` (literal, NOT `0x0D`/`0x0E`);
      (B) unrecognized escapes emit **two** bytes (`0x5C` + char);
      (C) iteration is **by byte** and **stops at the first 0x00**.
- [ ] A trailing NUL is NOT appended (length-based `Vec<u8>`).
- [ ] `last_consumable` starts `false` and transitions exactly as the C
      (false after `\b`, `\B`, bare `*`, and the `0x0E` quantifier; true
      everywhere else).
- [ ] Rustdoc on the function explains the placeholder-byte contract.
- [ ] `pub mod pattern;` added to `src/core/mod.rs` (single line).
- [ ] `cargo build --bin qmkonnect` clean; `cargo test --bin qmkonnect -- --test-threads=1` green.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge of this codebase can implement this
using only this PRP + the firmware repo, because (a) the entire C function is
reproduced here as a byte-action table, (b) all three non-obvious fidelity
gotchas are called out with the wrong-vs-right behavior, (c) the exact expected
`Vec<u8>` for 32 representative inputs is given (research/notes.md §5), (d) the
two-file change surface and the crate's test/build commands are specified, and
(e) the scope boundary (no anchors, no NFA) is explicit. See also
`research/notes.md`.

### Documentation & References

```yaml
# MUST READ — the canonical C source (the single source of truth, PRD §14)
- file: /home/dustin/projects/qmk-notifier/pattern_match.c
  why: "lines 30-75 are the function being ported, verbatim. The switch on the
        post-backslash byte + the last_consumable transitions are the spec."
  pattern: "walk src with while(*src); backslash-peek *(src+1); switch on next
            byte; bare '*'/'+'/'.' branches; default passthrough."
  gotcha: "it is `static` (file-local) in C. case '.': emits '.' (0x2E) NOT a
           placeholder; case '+': emits '+' (0x2B) NOT a placeholder. The
           default (unrecognized escape) emits BOTH '\\' and the char."

# MUST READ — the firmware architecture doc's byte-contract table (corroborates C)
- file: /home/dustin/projects/qmk-notifier/plan/001_e329fbe4ae4d/architecture/pattern_match_architecture.md
  why: "lines 66-95 give the processed-pattern byte table (0x0E, 0x01-0x04,
        0x05-0x0A, 0x0B/0x0C, 0x0D, 0x2A, 0x00=NUL terminator) and the
        process_escapes logic summary. Confirms GOTCHA-A (line 87: '\\. \\+ →
        literal ./+') and GOTCHA-B (line 90: unrecognized → emit \\\\ + char)."
  section: "### process_escapes Logic" and the byte table above it

# MUST READ — QMKonnect-side architecture contract
- file: plan/002_637d65b6e9b8/architecture/external_deps.md
  why: "§3 'Pattern Matcher (pattern_match.c → src/core/pattern.rs)' is the
        cross-repo contract: target path, the byte placeholders, and the
        downstream pipeline (parse_pattern → nfa_compile → nfa_match →
        match_with_anchors → match_pattern). Confirms anchors/delimiter logic
        live in LATER subtasks, NOT here."
  section: "## 3. Pattern Matcher"

# MUST READ — the parity vector table (the test contract for THIS task)
- file: plan/002_637d65b6e9b8/P2M1T1S1/research/notes.md
  why: "§5 lists the exact expected Vec<u8> for 32 inputs covering every branch
        + all three gotchas. Copy these directly into the unit tests."
  section: "## 5. Concrete parity test vectors"

# Reference — existing Rust module conventions in THIS repo (follow exactly)
- file: src/core/types.rs
  why: "shows the inline #[cfg(test)] mod tests pattern, #[derive(Debug, PartialEq)],
        assert_eq! usage. Mirror this style in pattern.rs."
  pattern: "pub struct ... ; impl ... ; #[cfg(test)] mod tests { use super::*; #[test] fn ... }"
- file: src/core/mod.rs
  why: "module registration site. Currently `pub mod notifier; pub mod types;`.
        Add `pub mod pattern;` in the same block."
  gotcha: "mod.rs ALSO holds the Config struct + helpers — DO NOT touch those.
           This task adds exactly ONE line (the module declaration)."

# Reference — PRD selectors that scoped this work
- url: spec/PRD.md (heading h2.74 "Pattern-Matching Syntax (pattern_match.c)")
  why: "the user-facing construct table (* wildcard, ^/$ anchors, escapes,
        \\d \\w \\s \\b, .). process_escapes is the byte encoder for the
        escape/class/dot/quantifier rows."
- url: spec/PRD.md (heading h2.92 "Appendix — File Layout & Pattern Subset")
  why: "mandates src/core/pattern.rs as 'full-parity matcher (ported from
        firmware)', Thompson NFA, case_sensitive per rule. Confirms the
        firmware + its test corpus is the single source of truth."
```

### Current Codebase tree (qmkonnect, relevant subset)

```bash
src/
  main.rs                 # CLI entry (unchanged by this task)
  core/
    mod.rs                # Config + helpers; module declarations (ADD one line)
    notifier.rs           # Notifier trait, debouncer, tests (unchanged)
    types.rs              # WindowInfo (unchanged) — style reference
  platforms/              # per-OS window monitors (unchanged)
  tray.rs / linux_tray.rs # tray UI (unchanged)
```

### Desired Codebase tree (files this task adds/changes)

```bash
src/
  core/
    pattern.rs            # NEW — process_escapes() + rustdoc + #[cfg(test)] tests
    mod.rs                # MODIFIED — add `pub mod pattern;` (one line)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (GOTCHA-A): escaped '\.' and '\+' emit LITERAL bytes, NOT placeholders.
//   process_escapes("\\.") == vec![0x2E]   // literal dot  — NOT 0x0D
//   process_escapes("\\+") == vec![0x2B]   // literal plus — NOT 0x0E
//   process_escapes(".")   == vec![0x0D]   // bare dot → placeholder 0x0D
//   process_escapes("a+")  == vec![0x61, 0x0E]  // bare + after consumable → 0x0E
// Mapping '\.' → 0x0D or '\+' → 0x0E is the #1 parity bug.

// CRITICAL (GOTCHA-B): an UNRECOGNIZED escape keeps BOTH bytes (2 output bytes).
//   process_escapes("\\x") == vec![0x5C, 0x78]   // backslash + 'x'
//   process_escapes("\\z") == vec![0x5C, 0x7A]
// It is NOT an error and NOT collapsed to one byte. This is the only ≥1:1 case.

// CRITICAL (GOTCHA-C): iterate pattern.as_bytes() and STOP at the first NUL.
//   - Do NOT use pattern.chars() — it breaks the byte-level peek-ahead and the
//     UTF-8 multi-byte passthrough (ASCII metachars are all < 0x80; UTF-8
//     continuation bytes are all >= 0x80, so byte iteration is safe + exact).
//   - The firmware C loop is `while (*src)` — it halts at the first 0x00. Mirror
//     that: when bytes[i] == 0x00, break (do NOT emit it). Valid TOML patterns
//     never contain NUL, so this is defensive — but it guarantees byte-for-byte
//     firmware parity for ALL inputs and prevents a divergence in edge tests.
//   - Do NOT append a trailing 0x00. Rust Vec<u8> length is the authoritative
//     end marker (unlike C, which NUL-terminates the char*).

// GOTCHA: `last_consumable` governs whether a bare '+' is the 0x0E quantifier or
// the 0x2B literal. It starts FALSE. It is set FALSE by: '\b', '\B', bare '*',
// and the 0x0E quantifier itself. Set TRUE by everything else. So:
//   process_escapes("+")   == vec![0x2B]        // start: non-consumable → literal
//   process_escapes("a*+") == vec![0x61, 0x2A, 0x2B]  // + after * → literal
//   process_escapes("\\b+") == vec![0x0B, 0x2B]       // + after \b → literal
//   process_escapes("a++") == vec![0x61, 0x0E, 0x2B]  // 2nd + after quantifier → literal

// GOTCHA: a trailing lone backslash (pattern ends in a single '\') → literal 0x5C.
//   process_escapes("abc\\") == vec![0x61, 0x62, 0x63, 0x5C]  // (Rust literal "abc\\")
// Implement via the index-bounds check: if bytes[i]==0x5C && i+1 >= len (or
// bytes[i+1]==0x00), emit 0x5C and stop.

// GOTCHA: bare '^' and '$' are NOT special here. There is no case for them as
// bare bytes (only the escaped '\^'/'\$'). They pass through as 0x5E / 0x24.
// Anchor DETECTION is parse_pattern's job (P2.M1.T1.S2) — do NOT pre-empt it.

// CRATE QUIRK: the crate-wide test command MUST be single-threaded because
// src/core/notifier.rs uses shared global debouncer state:
//   cargo test --bin qmkonnect -- --test-threads=1
// (AGENTS.md macOS loop; same flag applies on Linux/Windows.) pattern::tests
// itself is stateless and thread-safe, but run the whole bin single-threaded.
```

## Implementation Blueprint

### Data models and structure

No structs/enums are required for this subtask. The deliverable is a single
free function returning `Vec<u8>`. (`parsed_pattern_t`, the NFA state pool,
`Pattern::Single/Parts`, etc. belong to LATER subtasks — do not add them.)

The only "model" is the placeholder-byte vocabulary, which should be captured
as **named constants** at the top of `pattern.rs` so the NFA compiler
(P2.M1.T2.S1) and the reverse-map (`get_escaped_char`, already in firmware)
can reference the same definitions:

```rust
// Processed-pattern placeholder bytes — the contract between process_escapes
// and the Thompson NFA compiler (P2.M1.T2.S1). Mirrors the firmware
// pattern_match.c byte contract (see arch external_deps.md §3).
pub(crate) const ESC_CARET: u8   = 0x01; // \^  escaped literal ^
pub(crate) const ESC_DOLLAR: u8  = 0x02; // \$  escaped literal $
pub(crate) const ESC_STAR: u8    = 0x03; // \*  escaped literal *
pub(crate) const ESC_BSLASH: u8  = 0x04; // \\  escaped literal backslash
pub(crate) const CLASS_DIGIT: u8 = 0x05; // \d
pub(crate) const CLASS_NDIGIT: u8= 0x06; // \D
pub(crate) const CLASS_WORD: u8  = 0x07; // \w
pub(crate) const CLASS_NWORD: u8 = 0x08; // \W
pub(crate) const CLASS_SPACE: u8 = 0x09; // \s
pub(crate) const CLASS_NSPACE: u8= 0x0A; // \S
pub(crate) const ASSERT_BOUND: u8= 0x0B; // \b  zero-width
pub(crate) const ASSERT_NBOUND:u8= 0x0C; // \B  zero-width
pub(crate) const DOT_META: u8    = 0x0D; // bare .  (any char except \n/\r)
pub(crate) const PLUS_QUANT: u8  = 0x0E; // bare + after a consuming element
pub(crate) const GLOB_STAR: u8   = 0x2A; // bare *  glob wildcard
// Literal '.' (0x2E), '+' (0x2B), '\' (0x5C) are emitted as their ASCII bytes.
```
(Use these constants inside the match arms instead of bare hex literals; it
makes the parity with the firmware self-documenting and gives the NFA compiler
stable names to match on. If the implementer prefers local `const`s without
`pub(crate)`, that is acceptable as long as the names are reused — but
`pub(crate)` is recommended since P2.M1.T2 will need them.)

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CREATE src/core/pattern.rs — placeholder constants + process_escapes
  - DEFINE the `pub(crate) const` placeholder-byte vocabulary above (ESC_*, CLASS_*,
    ASSERT_*, DOT_META, PLUS_QUANT, GLOB_STAR).
  - IMPLEMENT: `pub(crate) fn process_escapes(pattern: &str) -> Vec<u8>`
      * let bytes = pattern.as_bytes();          // GOTCHA-C: BYTES, not chars
      * let mut out: Vec<u8> = Vec::with_capacity(bytes.len());  // output <= input
      * let mut last_consumable = false;
      * let mut i = 0;
      * while i < bytes.len() {
            let b = bytes[i];
            if b == 0x00 { break; }              // GOTCHA-C: stop at NUL (C while(*src))
            if b == b'\\' {                      // 0x5C
                if i + 1 < bytes.len() && bytes[i+1] != 0x00 {
                    let n = bytes[i+1];          // the char after the backslash
                    // match n: b'^' => push ESC_CARET; last_consumable=true
                    //          b'$' | b'*' | b'\\' => ESC_DOLLAR/ESC_STAR/ESC_BSLASH; true
                    //          b'.' => push b'.' (0x2E) [GOTCHA-A]; true
                    //          b'+' => push b'+' (0x2B) [GOTCHA-A]; true
                    //          b'd'=>CLASS_DIGIT ... b'S'=>CLASS_NSPACE; true
                    //          b'b'=>ASSERT_BOUND; false   b'B'=>ASSERT_NBOUND; false
                    //          _   => { push b'\\'; push n; } [GOTCHA-B, 2 bytes]; true
                    i += 2;
                } else {
                    out.push(b'\\');             // trailing lone backslash → literal
                    last_consumable = true;
                    i += 1;                      // (then loop ends: i==len)
                }
            } else if b == b'*' {
                out.push(GLOB_STAR); last_consumable = false; i += 1;
            } else if b == b'+' {
                if last_consumable { out.push(PLUS_QUANT); last_consumable = false; }
                else               { out.push(b'+');      last_consumable = true;  }
                i += 1;
            } else if b == b'.' {
                out.push(DOT_META); last_consumable = true; i += 1;
            } else {
                out.push(b); last_consumable = true; i += 1;   // ordinary literal
            }
        }
      * out   // NO trailing NUL
  - FOLLOW pattern: src/core/types.rs (free functions + inline tests; assert_eq!)
  - NAMING: snake_case fn; SCREAMING_SNAKE_CASE consts; module is `pattern`.
  - GOTCHA: the `b'\\'` arm must check `bytes[i+1] != 0x00` BEFORE treating it as
    an escape char — a backslash immediately before a NUL is the trailing-
    lone-backslash case (C: *(src+1)=='\0' is falsy → second branch).
  - PLACEMENT: src/core/pattern.rs (new file).

Task 2: ADD rustdoc to process_escapes
  - WRITE a `///` doc comment above the fn explaining: input is a pattern slice;
    output is the processed byte stream (no trailing NUL); the placeholder-byte
    table (0x01-0x0E, 0x2A); that literal ./+/\ are ordinary ASCII; that it stops
    at the first NUL for firmware parity; and that anchor detection is NOT done
    here (parse_pattern's job). Mode A (code-level docs only; no external docs).
  - REFERENCE arch external_deps.md §3 and PRD §4 as the contract sources.

Task 3: REGISTER the module in src/core/mod.rs
  - ADD: `pub mod pattern;` alongside the existing `pub mod notifier;` /
    `pub mod types;` declarations (top of mod.rs).
  - PRESERVE: every other line in mod.rs (Config struct, helpers, now_ms, etc.).
  - This is the ONLY edit to an existing file.

Task 4: CREATE the parity unit tests in src/core/pattern.rs
  - ADD: `#[cfg(test)] mod tests { use super::*; ... }` (inline, mirror types.rs).
  - IMPLEMENT one #[test] per row of research/notes.md §5 (the 32-row parity
    vector table), each asserting the exact Vec<u8>. Group with clear names:
      test_plain_passthrough, test_empty, test_escaped_literals_caret_dollar_star_bslash,
      test_escaped_dot_and_plus_are_literal (GOTCHA-A),
      test_classes_ddwwss, test_assertions_bb,
      test_bare_dot_meta, test_bare_glob_star, test_bare_plus_at_start_is_literal,
      test_quantifier_after_consumable, test_quantifier_after_class, test_dot_star_dot_plus,
      test_plus_after_glob_is_literal, test_plus_after_boundary_is_literal,
      test_chained_plus_second_is_literal, test_unrecognized_escape_keeps_both_bytes (GOTCHA-B),
      test_trailing_lone_backslash_is_literal, test_two_escapes, test_bare_caret_dollar_passthrough.
  - ALSO ADD: a NUL-stop test — process_escapes on a &str containing an interior
    NUL byte (construct via std::str::from_utf8 or a byte literal) truncates at
    the NUL (GOTCHA-C). Example: the bytes b"ab\0cd" as a &str → vec![0x61,0x62].
    (Use `std::str::from_utf8(b"ab\0cd").unwrap()` to build the input safely.)
  - COVERAGE: every match arm + every last_consumable transition + all three
    gotchas. The vector table is exhaustive for branches.
  - NAMING: test_<behavior>; #[test] per case (NOT one giant test).

Task 5: VERIFY build + tests (single-threaded, per AGENTS.md)
  - RUN: cargo build --bin qmkonnect        (expect: clean, no warnings on new file)
  - RUN: cargo test --bin qmkonnect pattern -- --test-threads=1
         (expect: all pattern::tests pass)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1
         (expect: full crate green — no regression in notifier/types)
  - IF a parity assertion fails: re-read the C source (pattern_match.c:30-75)
    arm-by-arm; the table here is a faithful transcription, so a failure means
    the Rust arm diverged from the table. Do NOT "fix" the test to match the
    Rust code — fix the Rust code to match the firmware.
```

### Implementation Patterns & Key Details

```rust
// The canonical arm-by-arm structure (this IS the spec — match it exactly):
//
// pub(crate) fn process_escapes(pattern: &str) -> Vec<u8> {
//     let bytes = pattern.as_bytes();
//     let mut out = Vec::with_capacity(bytes.len());
//     let mut last_consumable = false;
//     let mut i = 0;
//     while i < bytes.len() {
//         let b = bytes[i];
//         if b == 0x00 { break; }                          // GOTCHA-C
//         match b {
//             b'\\' => {                                   // 0x5C
//                 if i + 1 < bytes.len() && bytes[i + 1] != 0x00 {
//                     match bytes[i + 1] {
//                         b'^'  => { out.push(ESC_CARET);   last_consumable = true; }
//                         b'$'  => { out.push(ESC_DOLLAR);  last_consumable = true; }
//                         b'*'  => { out.push(ESC_STAR);    last_consumable = true; }
//                         b'\\' => { out.push(ESC_BSLASH);  last_consumable = true; }
//                         b'.'  => { out.push(b'.');        last_consumable = true; } // GOTCHA-A
//                         b'+'  => { out.push(b'+');        last_consumable = true; } // GOTCHA-A
//                         b'd'  => { out.push(CLASS_DIGIT);  last_consumable = true; }
//                         b'D'  => { out.push(CLASS_NDIGIT); last_consumable = true; }
//                         b'w'  => { out.push(CLASS_WORD);   last_consumable = true; }
//                         b'W'  => { out.push(CLASS_NWORD);  last_consumable = true; }
//                         b's'  => { out.push(CLASS_SPACE);  last_consumable = true; }
//                         b'S'  => { out.push(CLASS_NSPACE); last_consumable = true; }
//                         b'b'  => { out.push(ASSERT_BOUND);  last_consumable = false; }
//                         b'B'  => { out.push(ASSERT_NBOUND); last_consumable = false; }
//                         _     => { out.push(b'\\'); out.push(bytes[i + 1]); last_consumable = true; } // GOTCHA-B
//                     }
//                     i += 2;
//                 } else {
//                     out.push(b'\\');                     // trailing lone backslash
//                     last_consumable = true;
//                     i += 1;
//                 }
//             }
//             b'*' => { out.push(GLOB_STAR); last_consumable = false; i += 1; }
//             b'+' => {
//                 if last_consumable { out.push(PLUS_QUANT); last_consumable = false; }
//                 else               { out.push(b'+');      last_consumable = true;  }
//                 i += 1;
//             }
//             b'.' => { out.push(DOT_META); last_consumable = true; i += 1; }
//             _    => { out.push(b);        last_consumable = true; i += 1; }
//         }
//     }
//     out
// }
//
// NOTE: a single outer `match b` with nested arms is cleaner than the C
// if/else-if chain and produces identical behavior. The order of the bare-byte
// arms (\\, *, +, ., _) does not matter because they are mutually exclusive on
// the leading byte; only the backslash arm's nested match is order-sensitive
// (and it is exhaustive via the `_` default = GOTCHA-B).
```

### Integration Points

```yaml
MODULE REGISTRATION:
  - add to: src/core/mod.rs (top, next to `pub mod notifier;`)
  - line:   "pub mod pattern;"

DEPENDENCIES (this task): NONE. Pure stdlib byte processing. No new Cargo deps.
                           No dependency on the qmk_notifier crate or firmware.

DOWNSTREAM CONSUMERS (later subtasks — do NOT implement them here):
  - P2.M1.T1.S2 parse_pattern(): strips ^/$ anchors from the ORIGINAL pattern,
    feeds the core substring to process_escapes(), stores the Vec<u8> as the
    "core_pattern" the NFA compiles. It will import process_escapes + the
    placeholder consts from this module.
  - P2.M1.T2.S1 nfa_compile(): matches on the placeholder bytes (ESC_*,
    CLASS_*, ASSERT_*, DOT_META, PLUS_QUANT, GLOB_STAR) to emit NFA ops. The
    const names defined here ARE that contract — keep them stable.
  - P2.M1.T4.S1 parity tests: ports the firmware test corpus; process_escapes
    output feeds those end-to-end match tests. The byte encoding must not shift.

CONFIG: none.
ROUTES: none (no CLI surface in this subtask).
DATABASE: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles clean. If rustc warns on src/core/pattern.rs (unused,
# non-snake-case, etc.), READ the warning and fix it before proceeding. There
# are no unsafe blocks and no new deps, so the build is deterministic.

# Confirm the module is wired and the function exists:
grep -n 'pub mod pattern' src/core/mod.rs                 # expect one new line
grep -n 'fn process_escapes' src/core/pattern.rs          # expect one definition
grep -n 'pub(crate) const ESC_CARET' src/core/pattern.rs  # expect the vocab
```

### Level 2: Unit Tests — the parity contract (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# Single-threaded is MANDATORY crate-wide (shared debouncer state in notifier.rs).
cargo test --bin qmkonnect pattern -- --test-threads=1
# Expected: every test in pattern::tests passes. These assert the exact Vec<u8>
# from research/notes.md §5 (32-row parity table) + the NUL-stop test (GOTCHA-C).
# A failure means a Rust arm diverged from the firmware C — fix the Rust, not the test.
```

### Level 3: Full-crate regression (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL bin tests green — pattern::tests (new) + notifier + types.
# This proves the one-line mod.rs edit did not break module resolution and that
# the new module compiles in the full crate context (not just in isolation).

# Confirm the change surface is exactly two files:
git status --short
# Expected:
#   new file:  src/core/pattern.rs
#   modified:  src/core/mod.rs        (only the `pub mod pattern;` line)
git diff src/core/mod.rs
# Expected: a single +/- hunk adding `pub mod pattern;`.
```

### Level 4: Fidelity cross-check (optional, high-confidence)

```bash
# Cross-validate against the firmware C for a few inputs by running the firmware's
# own test harness (unchanged by this task — it lives in the OTHER repo):
cd /home/dustin/projects/qmk-notifier && ./run_all_tests.sh
# Expected: the 9-suite pattern_match corpus still passes (it always does — this
# task does not touch the firmware). This is a sanity check that the byte contract
# the Rust port encodes matches the firmware the corpus exercises. The Rust
# parity vectors in research/notes.md §5 were derived FROM this C source.
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean (no warnings on `src/core/pattern.rs`).
- [ ] `cargo test --bin qmkonnect pattern -- --test-threads=1` — all parity tests pass.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` — full crate green.
- [ ] `git status` shows exactly: new `src/core/pattern.rs` + one-line edit to `src/core/mod.rs`.

### Feature Validation (parity)
- [ ] Every row of `research/notes.md` §5 (32 inputs) asserted and passing.
- [ ] GOTCHA-A: `\.` → `0x2E`, `\+` → `0x2B` (literal, not `0x0D`/`0x0E`).
- [ ] GOTCHA-B: unrecognized escape (`\x`, `\z`) → two bytes `0x5C` + char.
- [ ] GOTCHA-C: iteration by byte; stops at first `0x00`; no trailing NUL appended.
- [ ] `last_consumable` starts `false`; false after `\b`/`\B`/bare `*`/`0x0E`; true otherwise.
- [ ] Bare `+` is `0x0E` only after a consuming element; `0x2B` literal otherwise.
- [ ] Bare `.` → `0x0D`; bare `*` → `0x2A`; trailing lone `\` → `0x5C`.
- [ ] Bare `^`/`$` pass through as `0x5E`/`0x24` (anchors NOT handled here).

### Code Quality Validation
- [ ] Placeholder bytes are named `pub(crate) const`s (not magic hex in arms).
- [ ] Rustdoc on `process_escapes` explains the byte contract + scope (no anchors).
- [ ] Inline `#[cfg(test)] mod tests` mirrors `src/core/types.rs` style.
- [ ] No new Cargo dependencies; no `unsafe`; no external docs changed (Mode A).
- [ ] Scope respected: NO anchor detection, NO NFA, NO `pattern_match()` (later subtasks).

### Documentation & Deployment
- [ ] Code-level rustdoc present (Mode A — no `docs/*.md` or README changes this task).
- [ ] The byte-vocabulary consts are commented with their firmware meaning.

---

## Anti-Patterns to Avoid

- ❌ Do NOT map `\.` → `0x0D` or `\+` → `0x0E`. Those escapes yield LITERAL `0x2E`/`0x2B`.
      Only BARE `.` → `0x0D` and BARE `+`-after-consumable → `0x0E`. (GOTCHA-A)
- ❌ Do NOT collapse an unrecognized escape (`\x`) to one byte. Keep BOTH `0x5C` + char.
      (GOTCHA-B)
- ❌ Do NOT iterate `pattern.chars()`. Iterate `pattern.as_bytes()` — byte parity with
      the firmware C `char*` walk, and correct for UTF-8. (GOTCHA-C)
- ❌ Do NOT emit a trailing `0x00`. The C NUL-terminates because it uses C strings;
      Rust uses `Vec<u8>` whose length is the end marker.
- ❌ Do NOT keep scanning past a NUL byte. `break` on `0x00` to mirror C `while(*src)`.
- ❌ Do NOT implement anchor (`^`/`$`) detection, the NFA, or `pattern_match()` here —
      those are P2.M1.T1.S2 / P2.M1.T2 / P2.M1.T3. This task is the byte transform ONLY.
- ❌ Do NOT change the test to match divergent Rust output. The firmware C
      (`pattern_match.c:30-75`) is the source of truth (PRD §14); fix the Rust.
- ❌ Do NOT add new Cargo dependencies or `unsafe` — pure stdlib suffices.
- ❌ Do NOT edit `PRD.md`, any `tasks.json`, `prd_snapshot.md`, or any `plan/` file.
- ❌ Do NOT touch anything in `src/core/mod.rs` except adding the one module line.
- ❌ Do NOT run tests multi-threaded — the crate shares debouncer state
      (`cargo test --bin qmkonnect -- --test-threads=1`).

---

## Confidence Score: 9/10

This is a well-bounded, greenfield, single-function port of an existing, tested
C function whose entire body is transcribed in this PRP as a byte-action table
and reproduced as a reference Rust skeleton. The three non-obvious fidelity
gotchas (literal-dot/literal-plus escapes; unrecognized-escape two-byte
passthrough; byte-iteration + NUL-stop) are all explicitly called out with
wrong-vs-right examples, and 32 exact `Vec<u8>` parity vectors (derived directly
from the C source) are provided as the test contract. No external dependencies,
no `unsafe`, no architectural decisions remain open. The 1-point reservation is
for the (unlikely) event an implementer misreads GOTCHA-A (the `\.`/`\+` literal
case) despite the explicit callouts; the parity tests catch that immediately.
Scope is cleanly bounded from the next three subtasks (parse_pattern, NFA,
public matcher), so there is no risk of over-building.
# PRP — P2.M1.T1.S2: Port `parse_pattern()` — anchor detection & core extraction

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This is an **additive edit to an existing
> greenfield module** (`src/core/pattern.rs`) created by **P2.M1.T1.S1**
> (`process_escapes`). S1 is already implemented in-tree. This task adds the
> `ParsedPattern` struct + `parse_pattern()` function that **consumes**
> `process_escapes()` and is itself consumed by P2.M1.T2.S1 (NFA compiler) and
> P2.M1.T3.S1 (`match_with_anchors`). It is the **second** subtask of the P2.M1
> "Pattern Matcher Port" milestone (full firmware-parity matcher, PRD §4 + §14).

---

## Goal

**Feature Goal**: Port the firmware `parse_pattern(const char *pattern)`
(`qmk-notifier/pattern_match.c`, ~lines 100–175) to Rust as
`fn parse_pattern(pattern: &str) -> ParsedPattern`, with **identical anchor
detection and core extraction** for every input — most importantly the
**even-backslash-count rule** for the trailing `$` (the single trickiest parity
case). It detects a leading `^` (start anchor), a trailing `$` preceded by an
EVEN number of backslashes (end anchor), carves the core substring between
them, and runs it through `process_escapes()` (S1) to produce the placeholder
byte stream the NFA consumes.

**Deliverable**: additions to `src/core/pattern.rs` (the file S1 created):
1. `pub(crate) struct ParsedPattern { core: Vec<u8>, start_anchored: bool, end_anchored: bool }`
   (with `#[derive(Debug, Clone, PartialEq)]`);
2. `pub(crate) fn parse_pattern(pattern: &str) -> ParsedPattern`;
3. a rustdoc on `parse_pattern` explaining the even-backslash-count rule;
4. new `#[test]` fns appended to the existing `#[cfg(test)] mod tests` block,
   asserting the exact `ParsedPattern` for ~27 inputs (the parity table in
   `research/notes.md` §3).

**Success Definition**:
- For every input pattern, the Rust `ParsedPattern` equals the firmware C result
  — anchor flags match AND `core` equals `process_escapes()` of the firmware-carved
  core. The 27-row parity table (`research/notes.md` §3) is the contract and must
  all pass.
- `cargo build --bin qmkonnect` compiles clean (no new warnings).
- `cargo test --bin qmkonnect -- --test-threads=1` passes (including the new
  `parse_pattern` tests AND the existing S1 tests).
- `git diff` touches ONLY `src/core/pattern.rs`. No NFA, no `pattern_match()`,
  no `match_with_anchors`, no `mod.rs` change — those are later subtasks or S1.

## User Persona (if applicable)

**Target User**: Downstream callers in the `pattern` module itself — primarily
`match_with_anchors()` (P2.M1.T3.S1), which reads `start_anchored`/`end_anchored`
to pick the NFA match strategy (exact / prefix / suffix / substring) and feeds
`core` to `nfa_compile()` (P2.M1.T2.S1). Not a user-facing API.

**Use Case**: Turn a human-authored pattern string (from `rules.toml`, e.g.
`"^Firefox$"`, `"\\d+-\\d+"`, or `"abc\\\\$"`) into a `ParsedPattern` whose
`core: Vec<u8>` is the placeholder-byte stream the Thompson NFA compiles, and
whose anchor flags tell the matcher WHERE to anchor. Example:
`parse_pattern("^\\d+$")` → `ParsedPattern { core: vec![0x05, 0x0E],
start_anchored: true, end_anchored: true }` (anchored, `\d+` core).

**Pain Points Addressed**: Establishes the anchor-decision + core-extraction
stage so the NFA compiler and matcher receive a correctly-carved,
escape-processed byte stream with accurate anchor flags. A wrong anchor flag
(e.g. mis-classifying `abc\\$` as anchored) would silently flip a substring
match into a suffix match.

## Why

- **Stage 2 of the full-parity pipeline.** PRD §14 mandates the Rust matcher be
  a *"full-parity port of the firmware `pattern_match.c`, not a subset"* with
  *"the firmware matcher + its test corpus the single source of truth for match
  semantics."* `parse_pattern` is the anchor/core stage between escape-processing
  (S1) and NFA compilation (T2). A divergence here corrupts the core bytes AND
  the anchor strategy downstream.
- **The even-backslash-count rule is the failure magnet.** It is the one
  non-obvious piece of `parse_pattern` and is exercised by 8 derived parity
  vectors + the firmware corpus (`test_error_handling.c`, `test_invalid_patterns.c`).
  Getting it right now (with direct unit tests — impossible in C where the fn is
  `static`) locks the anchor contract.
- **Host-side rules, no reflash.** Per P2/P3/P4, rules move to a host-side
  `rules.toml`; the matcher runs on the desktop. This task ports the
  anchor-detection that those rules depend on.

## What

Add to `src/core/pattern.rs` (the file S1 created — do NOT recreate it):

1. **`ParsedPattern` struct** — 3 fields, derived for testability:
   ```rust
   #[derive(Debug, Clone, PartialEq)]
   pub(crate) struct ParsedPattern {
       pub(crate) core: Vec<u8>,        // process_escapes() output; the NFA input
       pub(crate) start_anchored: bool, // original pattern began with '^'
       pub(crate) end_anchored: bool,   // original pattern ended with unescaped '$'
   }
   ```
2. **`parse_pattern(pattern: &str) -> ParsedPattern`** implementing the algorithm
   below (verbatim-faithful to the C). It calls the S1 `process_escapes()`.
3. **Rustdoc** explaining the even-backslash-count rule (Mode A — code-level docs).
4. **Unit tests** appended to the existing `mod tests` block.

### The algorithm (authoritative: C source + `research/notes.md` §1, §4)

```text
1. bytes = pattern.as_bytes()
2. len = index of first 0x00 in bytes, or bytes.len()   # mirror firmware strlen (GOTCHA-2)
3. start = 0; end = len; start_anchored = false; end_anchored = false
4. START ANCHOR:  if end > start && bytes[start] == b'^':
                      start_anchored = true; start += 1
5. END ANCHOR:    if end > start && bytes[end-1] == b'$':
                      bs = count of CONSECUTIVE b'\\' walking left from end-2 down to start
                      if bs % 2 == 0:           # EVEN => unescaped '$'
                          end_anchored = true; end -= 1   # drop the '$'
                      # else (ODD): '$' is escaped -> leave it in the core
6. core_str = &pattern[start..end]    # safe: ^, $, \, NUL are all ASCII char boundaries
7. core = process_escapes(core_str)   # S1 function; produces placeholder bytes
8. return ParsedPattern { core, start_anchored, end_anchored }
```

**The even-backslash-count rule** (step 5) is the critical parity case:
- `"abc$"`        → 0 backslashes (even) → **anchor**, core `"abc"`
- `"abc\\$"`      → 1 backslash  (odd)  → escaped, core `"abc\$"` → `[.., 0x02]`
- `"abc\\\\$"`    → 2 backslashes (even) → **anchor**, core `"abc\\"` → `[.., 0x04]`
- `"abc\\\\\\$"`  → 3 backslashes (odd)  → escaped, core `"abc\\\$"` → `[.., 0x04, 0x02]`

### Success Criteria
- [ ] `pub(crate) struct ParsedPattern { core, start_anchored, end_anchored }` exists with `#[derive(Debug, Clone, PartialEq)]`.
- [ ] `pub(crate) fn parse_pattern(pattern: &str) -> ParsedPattern` exists.
- [ ] Every row of `research/notes.md` §3 (27 inputs) passes as a direct `ParsedPattern` assertion.
- [ ] GOTCHA-1 (even-backslash): rows 13–20 pass (the 4 canonical `abc$`/`abc\\$`/`abc\\\\$`/`abc\\\\\\$` cases + the escaped-anchor cases).
- [ ] GOTCHA-2 (NUL-stop): row 27 passes (interior `0x00` truncates BEFORE anchor detection).
- [ ] Empty core handled: `"^$"` → `{ core: [], start: true, end: true }`.
- [ ] Leading `^` only ever detected at index 0 (`"^^"` → start=true, core `[0x5E]`).
- [ ] Rustdoc explains the even-backslash-count rule + the strlen/NUL parity.
- [ ] Calls S1's `process_escapes()` (does NOT reimplement escapes).
- [ ] `cargo build --bin qmkonnect` clean; `cargo test --bin qmkonnect -- --test-threads=1` green.
- [ ] `git diff` touches only `src/core/pattern.rs`.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge of this codebase can implement this
using only this PRP + the firmware repo, because (a) the entire C function is
reproduced in `research/notes.md` §1, (b) the algorithm is given as an
8-step pseudocode + a verified Rust skeleton (§4 of notes, mirrored in the
Implementation Blueprint), (c) the even-backslash-count rule is spelled out
with the 4 canonical cases and pinned by 8 parity vectors, (d) the S1 contract
being consumed (`process_escapes` signature, const names, test-module location)
is confirmed from the actual in-tree file, (e) 27 exact `ParsedPattern`
vectors are given as the test contract, and (f) the scope boundary (no NFA, no
matcher entry) is explicit. See `research/notes.md`.

### Documentation & References

```yaml
# MUST READ — the canonical C source (single source of truth, PRD §14)
- file: /home/dustin/projects/qmk-notifier/pattern_match.c
  why: "parse_pattern() at ~lines 100-175 is the function being ported, verbatim.
        The parsed_pattern_t typedef (just above it), the even-backslash-count
        while-loop, the `end > start` guard, and the carve+process_escapes call
        are the spec. Reproduced in research/notes.md §1."
  pattern: "start/end char* pointers; *(end-1)=='$' check; walk check=end-2 left
            counting consecutive '\\'; even => end_anchored, end--; strncpy core;
            process_escapes(core)."
  gotcha: "it is `static` (file-local) in C — the firmware tests reach it only via
           pattern_match(). The Rust port makes it pub(crate) and unit-tests it
           DIRECTLY (stronger coverage). Drop the malloc-failure fallback
           (core_pattern=pattern): Rust Vec can't fail-soft; irrelevant on host."

# MUST READ — the file THIS task edits (S1's deliverable, already in-tree)
- file: src/core/pattern.rs
  why: "the module S1 created. It already has: pub(crate) const ESC_CARET..GLOB_STAR
        (0x01-0x0E, 0x2A), pub(crate) fn process_escapes(&str)->Vec<u8> (~line 85),
        and #[cfg(test)] mod tests { use super::*; ... } (~line 206). APPEND the
        ParsedPattern struct + parse_pattern fn + new tests HERE. Do NOT recreate."
  pattern: "tests are grouped with `// --- header ---` comments and assert with the
            named consts, e.g. assert_eq!(process_escapes(\"\\\\^\"), vec![ESC_CARET]).
            Mirror that style for the parse_pattern tests."
  gotcha: "the file carries #![allow(dead_code)] (S1 ships the API ahead of
           consumers). parse_pattern IS process_escapes' first non-test consumer;
           the per-item #[allow(dead_code)] on process_escapes becomes redundant
           once parse_pattern exists — leaving it is harmless, removing it is
           optional cleanup."

# MUST READ — the parity vector table (the test contract for THIS task)
- file: plan/002_637d65b6e9b8/P2M1T1S2/research/notes.md
  why: "§3 lists the exact expected ParsedPattern for 27 inputs across 4 groups
        (anchor detection, even-backslash rule, escape/class interaction, NUL-stop).
        Copy these directly into the unit tests."
  section: "## 3. Rust-native parity table for parse_pattern"

# MUST READ — the S1 PRP (the upstream contract this task consumes)
- file: plan/002_637d65b6e9b8/P2M1T1S1/PRP.md
  why: "defines process_escapes() and the placeholder-byte vocabulary this task
        calls. Confirms the byte contract (0x01-0x0E, 0x2A, literal ./+/\ as ASCII)
        and that anchor detection is explicitly DEFERRED to THIS task (S2)."
  section: "## What" (the placeholder-byte table) and "### Integration Points"
           (DOWNSTREAM CONSUMERS lists parse_pattern as P2.M1.T1.S2)

# MUST READ — QMKonnect-side architecture contract
- file: plan/002_637d65b6e9b8/architecture/external_deps.md
  why: "§3 'Pattern Matcher' is the cross-repo contract: target path
        src/core/pattern.rs and the pipeline
        parse_pattern -> nfa_compile -> nfa_match -> match_with_anchors.
        Confirms anchors/NFA/matcher live in later subtasks, not here."
  section: "## 3. Pattern Matcher" (point 2: parse_pattern anchor detection)

# Reference — firmware architecture doc (corroborates the C)
- file: /home/dustin/projects/qmk-notifier/plan/001_e329fbe4ae4d/architecture/pattern_match_architecture.md
  why: "### parse_pattern Anchor Detection (lines ~97-101) restates the rule:
        leading ^ -> start_anchored; trailing $ not escaped (even backslash count)
        -> end_anchored; core fed to process_escapes. parsed_pattern_t typedef at
        lines ~32-39. Cross-checks the C source."
  section: "### parse_pattern Anchor Detection"

# Reference — existing Rust struct/test conventions in THIS repo
- file: src/core/types.rs
  why: "shows #[derive(Debug, PartialEq)] on a struct + inline #[cfg(test)] mod
        tests with assert_eq!/assert_ne!. Mirror this derive + test style for
        ParsedPattern."
  pattern: "#[derive(Debug, PartialEq)] pub struct ... ; #[cfg(test)] mod tests { use super::*; #[test] fn ... }"

# Reference — PRD selectors that scoped this work
- url: spec/PRD.md (heading h2.74 "Pattern-Matching Syntax (pattern_match.c)")
  why: "the ^/$ anchor + ^…$ exact-match rows of the construct table are what
        parse_pattern's start_anchored/end_anchored implement."
- url: spec/PRD.md (heading h2.92 "Appendix — File Layout & Pattern Subset")
  why: "mandates src/core/pattern.rs as 'full-parity matcher (ported from
        firmware)', Thompson NFA, case_sensitive per rule. Confirms the firmware
        + its test corpus is the single source of truth."
```

### Current Codebase tree (qmkonnect, relevant subset)

```bash
src/
  main.rs                 # CLI entry (unchanged)
  core/
    mod.rs                # Config + helpers; ALREADY has `pub mod pattern;` (S1) — DO NOT TOUCH
    pattern.rs            # S1: process_escapes() + consts + mod tests  ← EDIT THIS FILE (additive)
    notifier.rs           # Notifier trait, debouncer, tests (unchanged)
    types.rs              # WindowInfo (unchanged) — struct/test style reference
  platforms/              # per-OS window monitors (unchanged)
  tray.rs / linux_tray.rs # tray UI (unchanged)
```

### Desired Codebase tree (files this task changes)

```bash
src/
  core/
    pattern.rs            # MODIFIED (additive) — + ParsedPattern struct, + parse_pattern(), + tests
    # mod.rs UNCHANGED (module already registered by S1)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (GOTCHA-1): the even-backslash-count rule for the trailing '$'.
//   A trailing '$' is a REAL end anchor ONLY when an EVEN number of backslashes
//   (0,2,4,...) immediately precede it. ODD => the '$' is escaped and stays in
//   the core (process_escapes turns '\$' into the 0x02 literal).
//     parse_pattern("abc$")       .end_anchored == true   // 0 backslashes
//     parse_pattern("abc\\$")     .end_anchored == false  // 1 backslash (escaped)
//     parse_pattern("abc\\\\$")   .end_anchored == true   // 2 backslashes
//     parse_pattern("abc\\\\\\$") .end_anchored == false  // 3 backslashes (escaped)
//   The walk counts RAW CONSECUTIVE backslashes — do NOT pair them as escapes.
//   (Rust source: "\\" = 1 byte, "\\\\" = 2, "\\\\\\$" = 3 backslashes + $.)

// CRITICAL (GOTCHA-2): mirror the firmware strlen — stop at the first 0x00.
//   The C computes `end = pattern + strlen(pattern)`. A Rust &str CAN contain a
//   0x00 (valid UTF-8). For byte-for-byte parity, compute the effective length
//   the SAME way BEFORE anchor detection, else the ANCHOR FLAGS diverge on a
//   NUL-containing input (a trailing '$' past a NUL would be wrongly detected):
//     let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
//   Real rules.toml patterns never have NUL; this is defensive but keeps the
//   port honest and matches S1's NUL-stop in process_escapes.

// GOTCHA: the `end > start` guard is mandatory before BOTH anchor checks.
//   It rejects degenerate inputs: a lone "^" leaves start==end after skipping
//   '^', so *(end-1) would read before `start` in C. In Rust it prevents
//   underflow on `bytes[end-1]` / the backslash walk. "^$" still detects BOTH
//   anchors with an empty core: after '^', start=1,end=2; bytes[1]=='$';
//   backslash walk from index 0 (=='^') stops immediately, count=0 (even) =>
//   end_anchored, end=1, core=bytes[1..1]=empty.

// GOTCHA: a leading '^' is detected ONLY at the very front (index 0).
//   "^^" -> start_anchored=true (first ^), core="\^"-minus-the-^ ... = the 2nd ^
//   alone: process_escapes("^") = bare ^ passthrough = [0x5E]. So core=[0x5E],
//   NOT an escaped caret. Bare '^' is NOT special to process_escapes (only the
//   escaped '\^' is); it passes through as 0x5E. Likewise a bare interior/leading
//   '$' that is NOT a trailing anchor passes through as 0x24.

// GOTCHA: the &str slice &pattern[start..end] is ALWAYS safe.
//   '^' (0x5E), '$' (0x24), '\\' (0x5C), and NUL (0x00) are all ASCII (< 0x80).
//   UTF-8 char boundaries coincide with ASCII byte positions, so trimming at an
//   ASCII index never splits a multi-byte sequence. Do NOT index into bytes and
//   rebuild a String — slice the &str directly and hand it to process_escapes.

// GOTCHA: the C malloc-failure fallback (core_pattern = raw pattern) does NOT
//   exist in Rust. Vec<u8> allocation failure aborts (OOM); on a host with GBs
//   of RAM this is never hit for a rules.toml pattern. process_escapes ALWAYS
//   returns a Vec. So ParsedPattern.core is ALWAYS the processed bytes — there
//   is no "raw fallback" field. (This is why the C struct has 4 fields and the
//   Rust struct has 3.)

// GOTCHA: there is NO free_parsed_pattern analog. Rust owns the Vec<u8>; the
//   caller drops ParsedPattern normally. Do not add a Drop impl or a free fn.

// CRATE QUIRK: the crate-wide test command MUST be single-threaded because
//   src/core/notifier.rs uses shared global debouncer state:
//     cargo test --bin qmkonnect -- --test-threads=1
//   (AGENTS.md.) pattern::tests itself is stateless, but run the whole bin
//   single-threaded so notifier's globals don't race.
```

## Implementation Blueprint

### Data models and structure

```rust
/// The result of parsing a user pattern: anchor flags + the `process_escapes()`-
/// processed core the Thompson NFA compiler consumes. Rust analog of the
/// firmware `parsed_pattern_t` (`pattern_match.c`), minus the C malloc/fallback
/// fields (Rust `Vec<u8>` owns its buffer; no manual free, no fail-soft path).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParsedPattern {
    /// `process_escapes()` output for the substring between the anchors — the
    /// placeholder-byte stream the NFA compiles (P2.M1.T2.S1).
    pub(crate) core: Vec<u8>,
    /// `true` iff the original pattern began with `^` (start anchor).
    pub(crate) start_anchored: bool,
    /// `true` iff the original pattern ended with an UNESCAPED `$` (end anchor).
    pub(crate) end_anchored: bool,
}
```
No enums needed. (The `Pattern::Single|Parts` delimiter-aware enum is P2.M1.T3,
NOT this task.)

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: ADD the ParsedPattern struct to src/core/pattern.rs
  - PLACE: after the placeholder-const block (after GLOB_STAR, ~line 36) and
           before the process_escapes rustdoc, OR immediately above parse_pattern.
           Either is fine; group it with parse_pattern for locality.
  - DERIVE: #[derive(Debug, Clone, PartialEq)]  (PartialEq for assert_eq! in tests;
           Clone/Debug for downstream ergonomics + diagnostics.)
  - VISIBILITY: pub(crate) struct; pub(crate) fields (downstream T2/T3 read them).
  - DOC: a `///` comment mapping it to the firmware parsed_pattern_t and noting
         the dropped C fields (processed_pattern, malloc fallback).
  - FOLLOW: src/core/types.rs derive+struct style.

Task 2: IMPLEMENT parse_pattern() in src/core/pattern.rs
  - SIGNATURE: pub(crate) fn parse_pattern(pattern: &str) -> ParsedPattern
  - BODY (verified Rust skeleton — see research/notes.md §4):
      let bytes = pattern.as_bytes();
      let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len()); // GOTCHA-2
      let (mut start, mut end) = (0usize, len);
      let (mut start_anchored, mut end_anchored) = (false, false);
      // START anchor
      if end > start && bytes[start] == b'^' {
          start_anchored = true;
          start += 1;
      }
      // END anchor (even-backslash-count rule, GOTCHA-1)
      if end > start && bytes[end - 1] == b'$' {
          let mut bs = 0usize;
          let mut k = end - 1;                 // index of '$'
          while k > start {
              k -= 1;                           // byte left of '$'
              if bytes[k] == b'\\' { bs += 1; } else { break; }
          }
          if bs % 2 == 0 { end_anchored = true; end -= 1; }
      }
      let core = process_escapes(&pattern[start..end]);  // safe slice (ASCII boundary)
      ParsedPattern { core, start_anchored, end_anchored }
  - CALLS: the S1 `process_escapes()` (already pub(crate) in this module). Do NOT
           reimplement escapes or redeclare the placeholder consts.
  - GOTCHA: the backslash walk uses `while k > start { k -= 1; ... }` (decrement-
            then-check) so it inspects positions end-2 .. start inclusive — matching
            the C `while (check >= start ...)`. Verified in research/notes.md §4.
  - GOTCHA: no NULL guard (Rust &str is never null), no malloc-fallback branch,
            no free fn.
  - PLACEMENT: src/core/pattern.rs, after process_escapes (logical pipeline order).

Task 3: ADD rustdoc to parse_pattern (Mode A — code-level docs only)
  - WRITE a `///` block above the fn explaining:
      * input is a pattern slice; output is ParsedPattern (core = processed bytes,
        + the two anchor flags);
      * the START anchor rule (leading '^');
      * the END anchor EVEN-BACKSLASH-COUNT rule with the 4 canonical examples
        ("abc$" anchor / "abc\\$" escaped / "abc\\\\$" anchor / "abc\\\\\\$" escaped);
      * the empty-core case ("^$" -> both anchored, empty core, matches empty string);
      * that core is then run through process_escapes (cross-ref S1);
      * the strlen/NUL parity note (GOTCHA-2);
      * that this is stage 2 of the pipeline (parse_pattern -> process_escapes ->
        nfa_compile -> nfa_match -> match_with_anchors).
  - REFERENCE arch external_deps.md §3 and PRD §4 as the contract sources.

Task 4: APPEND parse_pattern unit tests to the existing mod tests in pattern.rs
  - DO: add new #[test] fns INTO the existing `mod tests { use super::*; ... }`
        block (line ~206). Group with the same `// --- header ---` comment style.
  - IMPLEMENT one #[test] per row of research/notes.md §3 (27 inputs), asserting
        the full ParsedPattern (core + both flags). Suggested groupings:
      // --- Anchor detection (no escape interaction) ---        (rows 1-12)
      // --- Even-backslash-count rule (GOTCHA-1) ---            (rows 13-20)
      // --- Anchor + escape/class interaction ---               (rows 21-26)
      // --- NUL-stop parity (GOTCHA-2) ---                      (row 27)
  - USE the named consts in core assertions (ESC_DOLLAR, ESC_BSLASH, ESC_CARET,
        GLOB_STAR, CLASS_DIGIT, DOT_META, PLUS_QUANT, ...) — same style as S1's
        existing tests. e.g.:
        assert_eq!(parse_pattern("abc\\\\$"),
                   ParsedPattern { core: vec![0x61,0x62,0x63, ESC_BSLASH],
                                   start_anchored: false, end_anchored: true });
  - NUL test (row 27): build the input via std::str::from_utf8 to avoid a literal
        NUL in source:
        let s = std::str::from_utf8(b"ab\0cd$").unwrap();
        assert_eq!(parse_pattern(s),
                   ParsedPattern { core: vec![0x61,0x62], start_anchored: false, end_anchored: false });
  - COVERAGE: every algorithm branch + both gotchas + the empty-core + lone-anchor
        + double-caret/double-dollar + interior-$ cases.
  - NAMING: test_parse_<behavior> (e.g. test_parse_start_anchor, test_parse_end_anchor_even_backslash,
        test_parse_escaped_dollar_odd_backslash, test_parse_both_anchors_empty_core,
        test_parse_nul_truncates_before_anchor_check).

Task 5: VERIFY build + tests (single-threaded, per AGENTS.md)
  - RUN: cargo build --bin qmkonnect        (expect: clean; no NEW warnings)
  - RUN: cargo test --bin qmkonnect pattern -- --test-threads=1
         (expect: ALL pattern::tests pass — the new parse_pattern tests AND S1's
          process_escapes tests)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1
         (expect: full crate green — no regression)
  - IF a parity assertion fails: re-read the C source (pattern_match.c parse_pattern)
    line-by-line; the table here is a faithful transcription, so a failure means
    the Rust diverged from the algorithm. Do NOT "fix" the test to match the Rust
    code — fix the Rust to match the firmware.
  - CONFIRM git status shows ONLY src/core/pattern.rs modified.
```

### Implementation Patterns & Key Details

```rust
// The canonical parse_pattern body (this IS the spec — match it exactly):
//
// pub(crate) fn parse_pattern(pattern: &str) -> ParsedPattern {
//     let bytes = pattern.as_bytes();
//     // GOTCHA-2: mirror firmware strlen — stop at the first NUL byte.
//     let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
//
//     let mut start = 0usize;
//     let mut end = len;
//     let mut start_anchored = false;
//     let mut end_anchored = false;
//
//     // START anchor: a leading '^' (only at the very front; '\^' would be
//     // processed to 0x01 later by process_escapes, never seen as an anchor here).
//     if end > start && bytes[start] == b'^' {
//         start_anchored = true;
//         start += 1;
//     }
//
//     // END anchor: a trailing '$' preceded by an EVEN number of backslashes.
//     if end > start && bytes[end - 1] == b'$' {
//         let mut bs = 0usize;
//         let mut k = end - 1;          // index of the '$'
//         while k > start {
//             k -= 1;                    // step left onto the byte before '$'
//             if bytes[k] == b'\\' { bs += 1; } else { break; }
//         }
//         if bs % 2 == 0 {               // even (0,2,4,...) => unescaped '$'
//             end_anchored = true;
//             end -= 1;                   // drop the '$'
//         }
//         // odd => '$' is escaped: leave it in the core; process_escapes turns
//         // the trailing '\$' into ESC_DOLLAR (0x02).
//     }
//
//     // Carve the core and process its escapes. The slice is safe: '^','$','\\',
//     // and NUL are all ASCII (<0x80) => char boundaries; we only trim at those.
//     let core = process_escapes(&pattern[start..end]);
//
//     ParsedPattern { core, start_anchored, end_anchored }
// }
//
// NOTE on the backslash walk: decrement-then-check (k -= 1 inside the while body)
// makes the loop inspect indices end-2, end-3, ..., start (inclusive) — exactly
// the C `for (check = end-2; check >= start && *check=='\\'; check--)`. When the
// '$' is the first core byte (e.g. "^$" after the '^' is stripped: start==end-1),
// `while k > start` is false immediately => bs==0 (even) => anchored, empty core.
```

### Integration Points

```yaml
MODULE REGISTRATION:
  - NONE. `pub mod pattern;` is already in src/core/mod.rs (added by S1). Do NOT
    edit mod.rs.

DEPENDENCIES (this task): NONE new. Pure stdlib byte/slice work + a call to the
                           in-module S1 `process_escapes()`. No Cargo deps, no
                           qmk_notifier crate, no firmware link.

UPSTREAM (already present — S1):
  - pub(crate) fn process_escapes(pattern: &str) -> Vec<u8>   (~line 85 of pattern.rs)
  - pub(crate) const ESC_CARET..GLOB_STAR (0x01-0x0E, 0x2A)  (top of pattern.rs)

DOWNSTREAM CONSUMERS (later subtasks — do NOT implement them here):
  - P2.M1.T2.S1 nfa_compile(): compiles `parsed.core` (the placeholder bytes)
    into the Thompson NFA state pool. It dispatches on the same byte values S1
    emits and this task passes through unchanged.
  - P2.M1.T3.S1 match_with_anchors(): reads parsed.start_anchored /
    parsed.end_anchored to choose exact (^…$) / prefix (^) / suffix ($) /
    substring (none) strategy, then loops offsets calling nfa_match on parsed.core.
    A wrong anchor flag here flips the strategy — hence the heavy parity testing.

CONFIG: none.
ROUTES: none (no CLI surface in this subtask).
DATABASE: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles clean. If rustc warns on src/core/pattern.rs (unused, etc.),
# READ it and fix before proceeding. The file already carries #![allow(dead_code)]
# from S1, so the new pub(crate) items won't spuriously warn even though their
# only non-test consumer is a later subtask.

# Confirm the additions are present:
grep -n 'struct ParsedPattern' src/core/pattern.rs   # expect one definition
grep -n 'fn parse_pattern' src/core/pattern.rs       # expect one definition
grep -n 'process_escapes(&pattern' src/core/pattern.rs  # expect the call site
```

### Level 2: Unit Tests — the parity contract (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# Single-threaded is MANDATORY crate-wide (shared debouncer state in notifier.rs).
cargo test --bin qmkonnect pattern -- --test-threads=1
# Expected: every test in pattern::tests passes — BOTH the new parse_pattern
# tests (research/notes.md §3, 27 rows) AND S1's process_escapes tests.
# A failure means the Rust diverged from the firmware C — fix the Rust, not the test.
# Filter to just the new tests to see them individually:
cargo test --bin qmkonnect pattern::tests::test_parse -- --test-threads=1
```

### Level 3: Full-crate regression (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL bin tests green — pattern::tests (new parse_pattern + S1
# process_escapes) + notifier + types. Proves the additive edit didn't break
# module resolution and compiles in the full crate context.

# Confirm the change surface is exactly one file:
git status --short
# Expected:
#   modified:   src/core/pattern.rs        (ONLY this)
git diff --stat
# Expected: only src/core/pattern.rs changed; mod.rs and everything else untouched.
```

### Level 4: Fidelity cross-check (optional, high-confidence)

```bash
# Cross-validate the anchor logic against the firmware's own corpus (unchanged by
# this task — it lives in the OTHER repo and is end-to-end via pattern_match):
cd /home/dustin/projects/qmk-notifier && ./run_all_tests.sh
# Expected: the 9-suite pattern_match corpus still passes (it always does — this
# task does not touch the firmware). The Rust parse_pattern parity vectors in
# research/notes.md §3 were derived FROM this C source + corpus, so a green
# firmware run corroborates the contract the Rust port encodes. The Rust tests
# are STRICTLY STRONGER (they assert ParsedPattern directly; the C only asserts
# end-to-end match results through the static fn).
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean (no NEW warnings on `src/core/pattern.rs`).
- [ ] `cargo test --bin qmkonnect pattern -- --test-threads=1` — all pattern tests pass (new + S1).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` — full crate green.
- [ ] `git status` shows exactly ONE modified file: `src/core/pattern.rs`.

### Feature Validation (parity)
- [ ] Every row of `research/notes.md` §3 (27 inputs) asserted and passing.
- [ ] GOTCHA-1 (even-backslash): rows 13–20 pass — the 4 canonical `abc$`/`abc\\$`/`abc\\\\$`/`abc\\\\\\$` cases + `\^$`/`^\$`/`\^*\$`/`mid$dle$`.
- [ ] GOTCHA-2 (NUL-stop): row 27 passes — interior `0x00` truncates BEFORE anchor detection (anchor flags, not just core, match firmware).
- [ ] Empty core: `"^$"` → `{ core: [], start: true, end: true }`.
- [ ] Lone anchors: `"^"` → start-only empty core; `"$"` → end-only empty core.
- [ ] Leading `^` only at index 0: `"^^"` → start=true, core `[0x5E]` (2nd ^ is literal).
- [ ] Trailing `$` only when even-backslash: `"$$"` → end=true, core `[0x24]` (1st $ literal).
- [ ] Core is always `process_escapes()` of the carved substring (escapes/classes/dot/quantifier all flow through correctly — rows 21–26).

### Code Quality Validation
- [ ] `ParsedPattern` has `#[derive(Debug, Clone, PartialEq)]`; `pub(crate)` struct + fields.
- [ ] `parse_pattern` is `pub(crate)`, takes `&str`, returns `ParsedPattern`.
- [ ] Calls S1's `process_escapes()`; does NOT reimplement escapes or redeclare consts.
- [ ] Rustdoc explains the even-backslash-count rule + strlen/NUL parity + pipeline stage.
- [ ] New tests appended to the existing `mod tests` (S1's tests untouched), grouped with `// --- header ---` comments, using named consts.
- [ ] No new Cargo dependencies; no `unsafe`; no external docs changed (Mode A).
- [ ] Scope respected: NO NFA, NO `pattern_match()`, NO `match_with_anchors`, NO `mod.rs` edit (later subtasks / S1).

### Documentation & Deployment
- [ ] Code-level rustdoc present (Mode A — no `docs/*.md` or README changes this task).
- [ ] `ParsedPattern` fields are commented with their firmware meaning + the dropped C fields.

---

## Anti-Patterns to Avoid

- ❌ Do NOT pair up backslashes as escapes during the end-anchor walk. Count RAW
      CONSECUTIVE backslashes; EVEN ⇒ anchor. `abc\\\\$` (2) IS an anchor;
      `abc\\\\\\$` (3) is NOT. (GOTCHA-1)
- ❌ Do NOT use `pattern.len()` for the end index. Mirror firmware `strlen`: stop
      at the first `0x00`, else anchor FLAGS diverge on NUL input. (GOTCHA-2)
- ❌ Do NOT drop the `end > start` guard before the end-anchor check — it prevents
      underflow (and matches the C degenerate-input rejection, e.g. lone `"^"`).
- ❌ Do NOT detect `^` anywhere but index 0. `"^^"` anchors on the FIRST `^` only;
      the second is a literal `0x5E` in the core.
- ❌ Do NOT index into `bytes[]` and rebuild a `String` for the core. Slice the
      `&str` directly (`&pattern[start..end]`) — safe at ASCII boundaries — and
      pass it to `process_escapes`.
- ❌ Do NOT port the C malloc-failure fallback (`core_pattern = pattern`). Rust
      `Vec` can't fail-soft; the host never OOMs on a rules.toml pattern. There
      is no "raw core" field — `core` is ALWAYS the processed bytes.
- ❌ Do NOT add a `free_parsed_pattern` / `Drop` impl. Rust owns the `Vec<u8>`.
- ❌ Do NOT reimplement `process_escapes` or redeclare the placeholder consts —
      they exist (S1). Just CALL `process_escapes(core_str)`.
- ❌ Do NOT implement the NFA (`nfa_compile`/`nfa_match`), `match_with_anchors`,
      the public `pattern_match()`, the delimiter-aware `match_pattern`, or
      `Pattern::Single|Parts` — those are P2.M1.T2 / P2.M1.T3.
- ❌ Do NOT edit `src/core/mod.rs` — `pub mod pattern;` is already there (S1).
- ❌ Do NOT change the test to match divergent Rust output. The firmware C
      (`pattern_match.c` `parse_pattern`) is the source of truth (PRD §14); fix
      the Rust.
- ❌ Do NOT run tests multi-threaded — the crate shares debouncer state
      (`cargo test --bin qmkonnect -- --test-threads=1`).
- ❌ Do NOT edit `PRD.md`, any `tasks.json`, `prd_snapshot.md`, or any `plan/` file.

---

## Confidence Score: 9/10

This is a well-bounded, single-function port of a ~30-line, heavily-commented C
function whose entire body is reproduced verbatim in `research/notes.md` §1 and
transcribed as a verified Rust skeleton (§4, mirrored in the Implementation
Blueprint). The one non-obvious piece — the even-backslash-count rule for the
trailing `$` — is spelled out with the 4 canonical cases and pinned by 8 derived
parity vectors (rows 13–20) plus the firmware's own corpus. 27 exact
`ParsedPattern` vectors are provided as the test contract, and because the Rust
port makes `parse_pattern` `pub(crate)` (vs. `static` in C), the unit tests are
STRICTLY STRONGER than the firmware's end-to-end-only coverage — they assert the
anchor flags + core directly. The S1 upstream contract (`process_escapes`
signature, const names, test-module location) is confirmed from the actual
in-tree file, so the additions slot in cleanly with no interface guesswork. No
new deps, no `unsafe`, no architectural decisions remain open. The 1-point
reservation is for the (unlikely) event an implementer mishandles the
decrement-then-check backslash walk or skips the NUL-stop parity despite the
explicit callouts; both are caught immediately by the parity tests. Scope is
cleanly bounded from S1 (upstream), T2 (NFA), and T3 (matcher entry), so there
is no risk of over- or under-building.
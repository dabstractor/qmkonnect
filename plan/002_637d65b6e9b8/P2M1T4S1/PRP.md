# PRP — P2.M1.T4.S1: Port Firmware Test Corpus as Rust Parity Tests

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This is an **additive edit** to the
> existing `#[cfg(test)] mod tests` block at the tail of `src/core/pattern.rs`.
> It APPENDS a comprehensive **firmware-parity test corpus** — a mechanical,
> faithful port of the 8 firmware `qmk-notifier/test_*.c` files
> (`/home/dustin/projects/qmk-notifier/`) into Rust `#[test]` functions. Every
> firmware case `{pattern, input, case_sensitive, expected}` becomes a Rust
> vector asserting `pattern_match(pattern, input, cs) == expected`. The leaf
> matcher `pub fn pattern_match` (P2.M1.T3.S1, in-tree ~line 1061) is the
> function under test. This task writes TEST CODE ONLY — no production logic.
> Consumes the leaf matcher from T3.S1 + (optionally) `Pattern`/`match_pattern`
> from T3.S2 (parallel) for one composition test. PRD §12 mandates this:
> *"unit-test (`src/core/pattern.rs`) full matcher parity by porting the firmware
> `pattern_match` corpus (wildcards, `^`/`$`, WT, `+`, classes, case sensitivity)
> and asserting identical results."*

---

## Goal

**Feature Goal**: Port the **entire** firmware `pattern_match` test corpus
(8 `test_*.c` files, **1225 assertion cases**) into Rust `#[test]` functions
inside `src/core/pattern.rs`'s `#[cfg(test)] mod tests` block, asserting that
the Rust leaf `pattern_match` produces the **identical bool** as the firmware C
matcher for every case. This is the authoritative cross-check that the P2.M1
port is a faithful full-parity port (PRD §14: *"full-parity port of the firmware
`pattern_match.c`, not a subset"*).

**Deliverable**: a single clearly-delimited `// ===== FIRMWARE PARITY CORPUS =====`
section appended to the END of the existing `mod tests` block in
`src/core/pattern.rs`, containing:
1. a shared `struct Case { pattern, input, cs, exp }` + `fn assert_parity(&[Case])` helper;
2. ~50 `#[test]` functions (one per firmware test grouping), each a `Case` table;
3. one `no-panic` property test representing the firmware's crash-safety loops;
4. one `match_pattern(&Single(..))` composition test (proves the delimiter-aware
   API delegates to the leaf, satisfying the contract's "delimiter-aware
   match_pattern tests" line without duplicating T3.S2's serde/Parts tests).

**Success Definition**:
- Every one of the ~1225 firmware assertion cases is represented as a Rust
  `Case` and passes (`pattern_match == expected`), modulo the documented
  type-system skip lists (4 NULL cases, ~8 invalid-UTF-8 cases — see Gotchas).
- `cargo test --bin qmkonnect pattern -- --test-threads=1` is green: all new
  `test_parity_*` tests PASS, plus the 160 existing pattern tests AND T3.S2's
  `test_mp_*`/`test_pattern_serde_*` tests.
- `cargo test --bin qmkonnect -- --test-threads=1` (full crate) green — no regression.
- `git status` shows **only** `src/core/pattern.rs` modified.
- A parity failure is treated as a **Rust divergence from the firmware** (PRD §14:
  firmware is source of truth) → fix the Rust, NOT the test. The ONLY exceptions
  are the documented NULL / invalid-UTF-8 skips (impossible in the Rust type system).

## User Persona (if applicable)

**Target User**: the **qmkonnect maintainers** (and the P2.M1 implementer). The
parity suite is the regression net that proves the Thompson-NFA port
(T1.S1→T3.S1) behaves byte-for-byte like the firmware matcher it replaces.

**Use Case**: during P2.M1 development and any future matcher tweak, run
`cargo test --bin qmkonnect pattern -- --test-threads=1`; a green run means the
Rust matcher agrees with the firmware corpus on wildcards, anchors, escapes,
classes, `\b`/`\B`, `.`, `+`, and case sensitivity.

**Pain Points Addressed**: without this corpus, the leaf matcher has only
~160 hand-written unit tests (T1–T3) that cover the *implementation's* view of
correctness. The firmware corpus is the *independent, externally-authored*
source of truth — it catches divergences the implementer's own tests would not.

## Why

- **PRD §12 mandates it.** The testing plan explicitly calls for "full matcher
  parity by porting the firmware `pattern_match` corpus … and asserting
  identical results." This task IS that port.
- **The firmware is the single source of truth for match semantics** (PRD §14).
  The Rust port's correctness is defined *relative to* the firmware; the only
  way to prove parity at scale is to run the firmware's own vectors against the
  Rust code.
- **It future-proofs the matcher.** Any later optimization (e.g. a faster NFA
  simulator) must keep these 1225 vectors green — a strong, executable contract.
- **Scope boundary**: this task writes ONLY tests. It does not touch the leaf
  `pattern_match`, `match_with_anchors`, the NFA, or `rules.rs` (P3). It is the
  capstone validation of milestone P2.M1.

## What

Append a **firmware-parity test section** to `src/core/pattern.rs`'s existing
`#[cfg(test)] mod tests` block. The section ports the 8 firmware files:

| firmware file | style | cases | Rust coverage |
|---------------|-------|------:|---------------|
| test_pattern_match.c | struct_array | 380 | 16 `test_parity_pm_*` (skip no-op #5) |
| test_char_classification.c | custom_helper | 179 | 8 `test_parity_charclass_*` (loop over `&[u8]`) |
| test_metachar_verification.c | printf_helper | 24 | 1 `test_parity_metachar_verification` |
| test_word_boundary_basic.c | struct_array | 74 | 4 `test_parity_wbb_*` |
| test_word_boundary_integration.c | struct_array | 189 | 5 `test_parity_wbi_*` |
| test_comprehensive_integration.c | custom_helper | 130 | 8 `test_parity_ci_*` (normalize param order) |
| test_invalid_patterns.c | custom_helper | 88 | 3 `test_parity_invalid_*` + 1 `no_panic` |
| test_error_handling.c | struct_array+inline | 161 | 8 `test_parity_eh_*` (skip 3 NULL) |
| (composition) | — | — | 1 `test_parity_match_pattern_single_dispatch` |

**Scope decisions (documented in the code):**
- Port **all 1225 assertion cases** (those with a checked bool).
- The **4420 non-assertion executions** (perf timings + crash-safety loops that
  discard the result) are NOT bool-parity tests — they are collapsed into ONE
  `test_parity_invalid_no_panic` property test (feeds the firmware's
  `problematic_patterns[]` to `pattern_match` and asserts no panic).
- **SKIP 4 NULL cases** (firmware `pattern_match(NULL,…)→false`; Rust `&str` is
  never null — no analog). Documented inline.
- **SKIP ~8 invalid-UTF-8 cases** (firmware inputs containing `0xFF`/`0xFE`;
  Rust `&str` requires valid UTF-8 and lone `0xFE`/`0xFF` are never valid).
  The `\x01` control-char cases ARE valid UTF-8 and port fine. Documented inline.

### Success Criteria
- [ ] Shared `struct Case { pattern: &'static str, input: &'static str, cs: bool, exp: bool }`
      + `fn assert_parity(cases: &[Case])` present (with indexed failure message).
- [ ] ~50 `#[test]` fns, all prefixed `test_parity_`, grouped under a single
      `// ===== FIRMWARE PARITY CORPUS (P2.M1.T4.S1) =====` banner at the END
      of `mod tests`.
- [ ] Every firmware grouping from `research/firmware_corpus_enumeration.md`
      has a corresponding `test_parity_*` fn with its full case count.
- [ ] The 4 NULL cases and ~8 invalid-UTF-8 cases are skipped with an inline
      comment citing the reason (G3/G2).
- [ ] C escapes ported correctly: `"\f"`→`"\x0C"`, `"\v"`→`"\x0B"`, `"\t"/"\n"/"\r"` unchanged.
- [ ] `test_comprehensive_integration.c` cases normalize the REVERSED
      `(pattern, str, expected, case_sensitive)` helper order to `(pattern, input, cs, exp)`.
- [ ] Dot-vs-`\r` cases assert expected `false` (firmware locks dot to exclude `\r`).
- [ ] `test_parity_match_pattern_single_dispatch` proves `Pattern::Single(p)`
      delegates to `pattern_match(p, app_class, cs)` on a cross-section.
- [ ] `cargo test --bin qmkonnect pattern -- --test-threads=1` green (new + existing).
- [ ] `git status` → only `src/core/pattern.rs`.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge of this codebase can implement this
using only this PRP + the firmware repo + the existing `pattern.rs`, because
(a) the ENTIRE per-file, per-grouping case counts + skip lists are in
`research/firmware_corpus_enumeration.md`; (b) the porting strategy (table-driven,
one-`#[test]`-per-grouping) + all 6 type-system gotchas are in
`research/notes.md`; (c) the firmware `test_*.c` files are on disk and are the
literal source to transcribe; (d) the leaf matcher contract
(`pub fn pattern_match(pattern: &str, s: &str, case_sensitive: bool) -> bool`)
is confirmed in-tree and the existing `mod tests` import idiom (`use super::*;`)
is shown; (e) the naming prefix `test_parity_*` is proven disjoint from the 160
existing tests and T3.S2's planned `test_mp_*`/`test_pattern_serde_*`.

### Documentation & References

```yaml
# MUST READ — the 8 firmware test files (the literal source to transcribe)
- file: /home/dustin/projects/qmk-notifier/test_pattern_match.c
  why: "380-case struct_array corpus; test_case_t {pattern,input,case_sensitive,
        expected_result,description}; run_test() loop. 17 groupings (skip #5
        test_character_classification — printf doc stub, 0 cases). THE big one."
  pattern: "static void test_X() { test_case_t tests[] = { {\"pat\",\"in\",cs,exp,\"desc\"}, ... };
            for(...) run_test(tests[i]); }"
- file: /home/dustin/projects/qmk-notifier/test_char_classification.c
  why: "179 cases via custom_helper test_class(p,in,cs,exp,meta,desc); cases
        generated by for-loops over char arrays. Port arrays as &[u8] and loop.
        1 NULL-input case (\\bword\\b vs NULL) -> SKIP."
- file: /home/dustin/projects/qmk-notifier/test_metachar_verification.c
  why: "24 cases, printf_helper (inline pattern_match(...) ? \"PASS\":\"FAIL\").
        Read the EXPECTED bool, not the PASS/FAIL label. 1 \\t special byte."
- file: /home/dustin/projects/qmk-notifier/test_word_boundary_basic.c
  why: "74 struct_array cases, 4 funcs (basic/anchors/wildcards/multiple)."
- file: /home/dustin/projects/qmk-notifier/test_word_boundary_integration.c
  why: "189 struct_array cases, 5 funcs. No special bytes, no NULL."
- file: /home/dustin/projects/qmk-notifier/test_comprehensive_integration.c
  why: "130 assertion cases via test_pattern_with_memory(pattern,str,EXPECTED,
        case_sensitive) — REVERSED param order (G5). 8 assertion funcs; skip the
        3500 perf/memory calls (non-assertion). Compiled with -DNOTIFIER_STUB
        (irrelevant to the Rust port — we read the cases, not run the C)."
  gotcha: "param order is (pattern, str, EXPECTED, case_sensitive); normalize."
- file: /home/dustin/projects/qmk-notifier/test_invalid_patterns.c
  why: "88 assertion cases via test_pattern(p,in,cs,exp,desc), 3 funcs. The 920-
        call test_comprehensive_error_handling crash-safety loop -> ONE no-panic
        property test. \"\\\\0\"/\"\\\\n\" are 2-CHAR LITERALS (backslash+letter),
        NOT control bytes (G4)."
- file: /home/dustin/projects/qmk-notifier/test_error_handling.c
  why: "161 cases, struct_array + inline, 9 funcs. SKIP test_null_pointer_handling
        (3 NULL cases, G3). SKIP the \\xFF/\\xFE cases in test_special_character_edge_cases
        (invalid UTF-8, G2). Dot-vs-\\r asserts expected=false despite misleading
        desc (G6)."

# MUST READ — the authoritative per-grouping counts + skip checklist (THIS task's contract)
- file: plan/002_637d65b6e9b8/P2M1T4S1/research/firmware_corpus_enumeration.md
  why: "per-file, per-function case COUNTS (read from the actual arrays) + the
        consolidated SKIP lists (NULL, invalid-UTF-8, special-byte escape mapping,
        \"\\\\0\"-vs-\"\\0\", dot-\\r). This is the completeness checklist — every
        grouping here must have a matching test_parity_* fn."
- file: plan/002_637d65b6e9b8/P2M1T4S1/research/notes.md
  why: "the porting STRATEGY (table-driven, one-test-per-grouping) + the 6
        type-system gotchas (G1\\f/\\v, G2 invalid-UTF-8, G3 NULL, G4 \\\\0 literal,
        G5 reversed params, G6 dot-\\r) + the match_pattern composition-test
        design + T3.S2 coordination. The design rationale."

# MUST READ — the file THIS task edits
- file: src/core/pattern.rs
  why: "the #[cfg(test)] mod tests block starts ~line 1182 (`mod tests { use super::*; }`).
        APPEND the parity corpus at its END, before the closing }. The leaf
        `pub fn pattern_match(pattern: &str, s: &str, case_sensitive: bool) -> bool`
        (~line 1061) is the function under test — call it directly (it's in scope
        via `use super::*`). 160 existing tests use prefixes test_plain_/test_parse_/
        test_compile_/test_match_/test_pm_/test_mwa_; do NOT rename or modify them."
  pattern: "tests grouped with `// --- header ---` comments; bool asserted via
            `assert!(...)` or `assert_eq!(...)`. The new section uses its own
            `// ===== FIRMWARE PARITY CORPUS =====` banner."
  gotcha: "do NOT touch production code (consts, process_escapes, parse_pattern,
           NfaOp, nfa_*, match_with_anchors, pattern_match). Tests only. The file
           carries #![allow(dead_code)] (API shipped ahead of consumers) — irrelevant
           to test code."

# MUST READ — the leaf matcher contract (the function under test)
- file: plan/002_637d65b6e9b8/P2M1T3S1/PRP.md
  why: "fixes the leaf signature pub fn pattern_match(pattern: &str, s: &str,
        case_sensitive: bool) -> bool (parse_pattern -> match_with_anchors). The
        parity corpus calls THIS. Confirms the empty-core special case
        (pattern_match(\"\",\"non-empty\")->false; pattern_match(\"\",\"\")->true)
        and the dot-excludes-{\\n,\\r} + classes-are-ASCII-only semantics the
        corpus exercises."

# MUST READ — the parallel T3.S2 contract (coordination + the composition test's target)
- file: plan/002_637d65b6e9b8/P2M1T3S2/PRP.md
  why: "T3.S2 (implementing IN PARALLEL) appends `pub enum Pattern {Single,Parts}`,
        `pub fn match_pattern(&Pattern, app_class, title, cs)->bool`, and ~30
        match_pattern/serde tests to the SAME mod tests block. This task's
        composition test calls match_pattern(&Pattern::Single(p),..) — so it can
        ONLY be written once T3.S2 lands Pattern/match_pattern. If T3.S2 is not
        yet in-tree, write the 1225-case leaf corpus first and add the composition
        test last (or gate it). Naming prefix test_parity_* is disjoint from
        T3.S2's test_mp_*/test_pattern_serde_*."
  section: "## Goal (the Pattern enum + match_pattern signature)"

# MUST READ — PRD testing mandate
- url: spec/PRD.md (heading h2.90 "12. Testing Plan")
  why: "mandates: 'unit-test (src/core/pattern.rs) full matcher parity by porting
        the firmware pattern_match corpus (wildcards, ^/$, WT, +, classes, case
        sensitivity) and asserting identical results.' This task IS that mandate."
- url: spec/PRD.md (heading h2.74 "Pattern-Matching Syntax" + h2.92 "Appendix")
  why: "the construct table (*, ^/$, \\^\\$\\*\\\\, \\d\\D\\w\\W\\s\\S, \\b\\B, ., +)
        and the 'full-parity port, not a subset' requirement."
```

### Current Codebase tree (relevant subset)

```bash
src/
  core/
    pattern.rs   # T1-T3 leaf matcher + 160-test #[cfg(test)] mod tests (ends ~line 2450)
                   # ← EDIT THIS FILE (append parity corpus at end of mod tests)
    notifier.rs  # debouncer (unchanged)
    types.rs     # WindowInfo (unchanged)
Cargo.toml      # no new deps (test code uses only std + the in-tree matcher)
# Firmware source of truth (READ-ONLY, external repo):
/home/dustin/projects/qmk-notifier/test_*.c   # the 8 files to transcribe
```

### Desired Codebase tree (files this task changes)

```bash
src/
  core/
    pattern.rs   # MODIFIED (additive) — + // ===== FIRMWARE PARITY CORPUS ===== section
                   #                     + struct Case + assert_parity helper
                   #                     + ~50 test_parity_* fns
                   #                     + 1 no_panic property test
                   #                     + 1 match_pattern composition test
    # NOTHING else changes. No Cargo.toml, no mod.rs, no production code.
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (G1 — Rust has NO \f or \v string escapes; VERIFIED via `rustc`):
//   Rust &str escapes are \n \r \t \\ \" \' \0 \x7F \u{...} ONLY. The firmware
//   test data uses C "\f" (form-feed 0x0C) and "\v" (vertical-tab 0x0B) heavily
//   (\s matches them; dot matches them). Map: C "\f" -> Rust "\x0C"; C "\v" ->
//   Rust "\x0B". \t/\n/\r are identical. The Rust matcher already classifies
//   these right (is_ascii_whitespace == {space,\t,\n,\r,\x0c,\x0b}; dot excludes
//   only {\n,\r}), so parity HOLDS once literals use the right escapes.
//
// CRITICAL (G2 — invalid-UTF-8 bytes 0xFE/0xFF CANNOT be a Rust &str):
//   The firmware feeds pattern_match(".", "\xFF", ...) ("dot matches high ASCII").
//   A lone 0xFF/0xFE byte is NEVER valid UTF-8, so it cannot live in a Rust &str.
//   SKIP these cases (a handful in test_error_handling.c::test_special_character_edge_cases:
//   \s/\S/\w/\W/. vs \xFF; literal test\xFF/test\xFE). Document inline. NOTE:
//   0x01 (control char) IS valid UTF-8 — port those as "\x01".
//
// CRITICAL (G3 — NULL-pointer cases have NO Rust &str analog; 4 total):
//   Firmware pattern_match(NULL, ...) returns false; Rust &str is never null.
//   SKIP: test_error_handling.c::test_null_pointer_handling (3 cases) +
//   test_char_classification.c (\bword\b vs NULL, 1 case). Do NOT change the
//   public API to Option<&str> for these — they're unrepresentable in safe Rust.
//
// GOTCHA (G4 — "\\0" is a 2-CHAR LITERAL, NOT a NUL byte):
//   In test_invalid_patterns.c, C "\\0"/"\\n"/"\\t" are backslash+letter (2 chars),
//   NOT control bytes. Port VERBATIM as Rust "\\0"/"\\n" (Rust "\\0" == \ + '0').
//   The firmware corpus contains NO actual NUL bytes in test data.
//
// GOTCHA (G5 — test_comprehensive_integration.c helper has REVERSED params):
//   test_pattern_with_memory(pattern, str, EXPECTED, case_sensitive) — expected
//   comes BEFORE cs, the reverse of every other file. When building the Case
//   table, normalize to (pattern, input, cs, exp). Read carefully.
//
// GOTCHA (G6 — dot vs \r: port the EXPECTED, ignore the misleading description):
//   test_error_handling.c has {".", "\r", true, false, "Dot should match carriage
//   return"} — desc says "match" but expected_result=false. Firmware LOCKS dot to
//   exclude BOTH \n AND \r. The Rust matcher already does this
//   (DOT_META => sc != b'\n' && sc != b'\r'). Assert expected=false; ignore the desc.
//
// GOTCHA (G7 — a parity failure means the RUST diverged; fix the Rust, not the test):
//   PRD §14: firmware is source of truth. If pattern_match returns a different
//   bool than the firmware expected, the Rust port has a bug — fix it. The ONLY
//   non-failures are the G2/G3 skip lists (impossible-in-Rust cases).
//
// GOTCHA (G8 — crate-wide test threading): cargo test --bin qmkonnect --
//   --test-threads=1 (shared debouncer state in notifier.rs, AGENTS.md). NEVER
//   run multi-threaded.
//
// GOTCHA (G9 — T3.S2 runs in parallel on the SAME mod tests block):
//   Both this task and T3.S2 append to the tail of mod tests. Use a single
//   clearly-delimited `// ===== FIRMWARE PARITY CORPUS (P2.M1.T4.S1) =====`
//   banner; place at the very END (after T3.S2's tests if present, before the
//   closing }). Prefix test_parity_* is disjoint from T3.S2's test_mp_*/test_pattern_serde_*.
//   If a textual conflict arises on rebasing, the sections are disjoint — resolve
//   by keeping both.
//
// GOTCHA (G10 — the match_pattern composition test needs T3.S2's Pattern/match_pattern):
//   test_parity_match_pattern_single_dispatch calls match_pattern(&Pattern::Single(p),..).
//   If T3.S2 has not landed when you start, write the 1225-case leaf corpus
//   FIRST, then add the composition test last (it's a thin cross-check, ~1 fn).
```

## Implementation Blueprint

### Data models and structure

This task adds ONE test-only helper struct + ONE assertion helper. No production types.

```rust
// Inside #[cfg(test)] mod tests, at the head of the new parity section:
struct Case {
    pattern: &'static str,
    input: &'static str,
    cs: bool,
    exp: bool,
}

/// Assert that pattern_match(pattern, input, cs) == exp for EVERY case, printing
/// a precise, indexed failure message on the first divergence. Mirrors the
/// firmware run_test() loop (qmk-notifier/test_pattern_match.c).
fn assert_parity(cases: &[Case]) {
    for (i, c) in cases.iter().enumerate() {
        let got = pattern_match(c.pattern, c.input, c.cs);
        assert!(
            got == c.exp,
            "parity FAIL [#{i}] pattern_match({:?}, {:?}, cs={}) = {}, expected {}",
            c.pattern, c.input, c.cs, got, c.exp
        );
    }
}
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: ADD the parity-section header + Case struct + assert_parity helper
  - DO: at the END of `mod tests` (after all existing test_pm_*/test_mwa_* AND
        any T3.S2 tests, before the closing }), insert:
        // ===== FIRMWARE PARITY CORPUS (P2.M1.T4.S1) =====
        // Ports qmk-notifier/test_*.c as Rust parity tests. Source of truth: the
        // C files. A failure = Rust diverged from firmware -> fix the Rust.
        // Skip lists (impossible in the Rust type system) are documented inline.
        struct Case { ... }   // as above
        fn assert_parity(cases: &[Case]) { ... }   // as above
  - GOTCHA: `pattern_match` is already in scope via `use super::*;` (line 1183).
            Do NOT re-import it.
  - PLACEMENT: src/core/pattern.rs, inside mod tests, at the very end.

Task 2: PORT test_pattern_match.c (380 cases, 16 fns; skip no-op #5)
  - DO: one #[test] per firmware function (test_start_anchor, test_end_anchor,
        test_full_anchor, test_anchors_with_wildcards, test_basic_metacharacter_escapes,
        test_basic_metacharacter_matching, test_word_boundary_escape_processing,
        test_escape_sequences, test_backward_compatibility, test_case_sensitivity,
        test_pattern_parsing, test_edge_cases, test_metacharacters_with_anchors,
        test_metacharacters_with_wildcards, test_metacharacter_case_sensitivity,
        test_metacharacter_backward_compatibility). SKIP #5
        test_character_classification (printf doc stub, 0 cases).
  - NAMING: test_parity_pm_<firmware_fn> (e.g. test_parity_pm_start_anchor).
  - BODY: assert_parity(&[ Case{...}, ... ]) transcribed verbatim from the C array.
  - GOTCHA G1: \s/\S sections use C "\f"/"\v" -> Rust "\x0C"/"\x0B".
  - GOTCHA: read the C array's expected_result field (4th), not the description.

Task 3: PORT test_char_classification.c (179 cases, 8 sections)
  - DO: the cases are generated by for-loops over char arrays. Port each array as
        a `const CHARS: &[u8] = &[b'a', b'z', ...]` and LOOP, calling a local
        helper `fn chk(pattern, byte, cs, exp)` that builds the input via
        std::str::from_utf8(&[byte]).unwrap() (or a 1-byte std::str). Eight
        #[test] fns: test_parity_charclass_digit/nondigit/word/nonword/space/
        nonspace/wordboundary/anchored.
  - GOTCHA G1: the \s array {b' ', b'\t', b'\n', b'\r', b'\f(0x0C)', b'\v(0x0B)'}.
  - GOTCHA G3: SKIP the single `\bword\b` vs NULL case (documented inline).

Task 4: PORT test_metachar_verification.c (24 cases, 1 fn)
  - DO: one #[test] test_parity_metachar_verification with a 24-entry Case table.
        Read the EXPECTED bool (the printf ternary's PASS = expected-true, FAIL =
        expected-false; invert the label accordingly — easier: just read each call's
        intent: e.g. `pattern_match("\\d","5",true)?PASS:FAIL` => expected true).
  - GOTCHA G1: one \t case -> "\t" (Rust \t works).

Task 5: PORT test_word_boundary_basic.c (74) + test_word_boundary_integration.c (189)
  - DO: 4 + 5 = 9 #[test] fns (test_parity_wbb_* / test_parity_wbi_*), each an
        assert_parity(&[...]) table. No special bytes, no NULL — straight transcription.

Task 6: PORT test_comprehensive_integration.c (130 assertion cases, 8 fns)
  - DO: 8 #[test] fns test_parity_ci_* (skip test_performance_complex +
        test_memory_management_complex — 3500 NON-assertion calls, out of scope).
  - GOTCHA G5: the helper order is (pattern, str, EXPECTED, case_sensitive).
        Normalize each case to Case{pattern, input, cs, exp} — swap the 3rd/4th.
  - GOTCHA G1: \s combos and dot+newline cases use \t/\n (Rust-native) — fine.

Task 7: PORT test_invalid_patterns.c (88 assertion cases, 3 fns) + the no-panic property test
  - DO: 3 #[test] fns test_parity_invalid_* (test_invalid_regex_patterns,
        test_boundary_conditions, test_error_conditions). SKIP
        test_comprehensive_error_handling (920 NON-assertion crash-safety calls).
  - DO: add ONE #[test] test_parity_invalid_no_panic that iterates the firmware's
        problematic_patterns[] (the ~46 patterns) over a few inputs and asserts
        pattern_match does NOT panic (mirrors the firmware's crash-safety intent;
        asserts no bool). Use std::str inputs only.
  - GOTCHA G4: "\\0"/"\\n"/"\\t" in the C are 2-char literals -> Rust "\\0" etc.

Task 8: PORT test_error_handling.c (161 cases, 8 fns; skip the 3 NULL cases)
  - DO: 8 #[test] fns test_parity_eh_* (test_invalid_escape_sequences,
        test_malformed_patterns, test_long_patterns_and_strings,
        test_special_character_edge_cases, test_memory_allocation_edge_cases,
        test_word_boundary_edge_cases, test_dot_metacharacter_edge_cases,
        test_complex_error_scenarios). SKIP test_null_pointer_handling entirely
        (3 NULL cases, G3).
  - GOTCHA G2: in test_special_character_edge_cases, SKIP the \xFF/\xFE cases
        (invalid UTF-8). Port the \x01 cases ("\x01" is valid UTF-8).
  - GOTCHA G1/G6: dot-vs-\t/\r/\f/\v cases — \t->"\t", \r->"\r", \f->"\x0C",
        \v->"\x0B"; assert expected false for \r (G6).
  - GOTCHA: test_long_patterns_and_strings uses dynamically-built long strings
        (400+/1000+ chars). Port via `let p = "a".repeat(400);` etc. and a local
        helper that takes String (not &'static str) — OR build the Case table with
        owned strings and a parallel assert helper. Simplest: a small
        `fn assert_parity_owned(cases: &[(String,String,bool,bool)])` for this one fn.

Task 9: ADD the match_pattern composition test (satisfies the contract's
        "delimiter-aware match_pattern tests" line)
  - DO: one #[test] test_parity_match_pattern_single_dispatch. Pick a CROSS-SECTION
        (~20-30 cases) spanning anchors, classes, \b, +, escapes, case on/off.
        For each, assert match_pattern(&Pattern::Single(p.into()), input,
        "IGNORED-title", cs) == pattern_match(p, input, cs) == exp. This proves
        Pattern::Single delegates to the leaf on app_class (title ignored) — the
        firmware-parity property of the delimiter-aware API.
  - DEPENDENCY: requires T3.S2's pub enum Pattern + pub fn match_pattern in-tree.
        If T3.S2 hasn't landed, write this fn LAST (or in a follow-up); the 1225-
        case leaf corpus is the primary deliverable and stands alone.
  - GOTCHA: do NOT re-list all 1225 cases here — the delegation is uniform by
        construction; a representative cross-section suffices.

Task 10: VERIFY build + tests (single-threaded, per AGENTS.md)
  - RUN: cargo build --bin qmkonnect       (expect: clean; no NEW warnings)
  - RUN: cargo test --bin qmkonnect pattern -- --test-threads=1
         (expect: ALL pattern::tests pass — the new test_parity_* corpus AND the
          160 existing + T3.S2's tests)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1
         (expect: full crate green — no regression)
  - IF a parity assertion fails: the helper prints pattern/input/cs/expected/got.
        Re-read the C source for that case + research/notes.md §5. The leaf
        matcher already implements firmware semantics, so a failure = either a
        transcription slip (wrong expected, wrong escape) or a genuine Rust
        divergence (fix the Rust per G7). Do NOT "fix" the test to match divergent
        Rust output — UNLESS the case is on the G2/G3 skip list (then it shouldn't
        be in the table at all).
  - CONFIRM git status shows ONLY src/core/pattern.rs modified.
```

### Implementation Patterns & Key Details

```rust
// The canonical parity-section skeleton (this IS the spec — match it).
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//     ... (160 existing tests + any T3.S2 tests) ...
//
//     // ===== FIRMWARE PARITY CORPUS (P2.M1.T4.S1) =====
//     // Ports qmk-notifier/test_*.c. Source of truth: the C files.
//     // A failure = Rust diverged from firmware -> fix the Rust (PRD §14).
//     // Skip lists (NULL, invalid-UTF-8) documented inline.
//
//     struct Case { pattern: &'static str, input: &'static str, cs: bool, exp: bool }
//
//     fn assert_parity(cases: &[Case]) {
//         for (i, c) in cases.iter().enumerate() {
//             let got = pattern_match(c.pattern, c.input, c.cs);
//             assert!(got == c.exp,
//                 "parity FAIL [#{i}] pattern_match({:?}, {:?}, cs={}) = {}, expected {}",
//                 c.pattern, c.input, c.cs, got, c.exp);
//         }
//     }
//
//     #[test] fn test_parity_pm_start_anchor() {
//         assert_parity(&[
//             Case { pattern: "^searchterm", input: "searchterm", cs: true, exp: true },
//             Case { pattern: "^searchterm", input: "presearchterm", cs: true, exp: false },
//             Case { pattern: "^searchterm", input: "searchtermpost", cs: true, exp: true },
//             // ... all 8 from firmware test_start_anchor() ...
//         ]);
//     }
//
//     // Special-byte escape mapping (G1): C "\f" -> "\x0C", C "\v" -> "\x0B".
//     #[test] fn test_parity_pm_basic_metacharacter_matching() {
//         assert_parity(&[
//             // \s class (firmware uses C "\t","\n","\r","\f","\v"):
//             Case { pattern: "\\s", input: " ",  cs: true, exp: true },
//             Case { pattern: "\\s", input: "\t", cs: true, exp: true },   // \t same
//             Case { pattern: "\\s", input: "\n", cs: true, exp: true },   // \n same
//             Case { pattern: "\\s", input: "\r", cs: true, exp: true },   // \r same
//             Case { pattern: "\\s", input: "\x0C", cs: true, exp: true }, // C "\f" -> \x0C
//             Case { pattern: "\\s", input: "\x0B", cs: true, exp: true }, // C "\v" -> \x0B
//             // ...
//         ]);
//     }
//
//     // G2/G3 skips are documented, not asserted:
//     #[test] fn test_parity_eh_null_pointer_handling() {
//         // SKIPPED (G3): firmware pattern_match(NULL,...) returns false, but Rust
//         // &str is never null — there is no analog. The 3 firmware cases
//         // (NULL pattern, NULL input, both NULL) are unrepresentable in safe Rust.
//         // (Keeping the fn as a placeholder so the skip is auditable.)
//     }
//
//     #[test] fn test_parity_eh_special_character_edge_cases() {
//         assert_parity(&[
//             // ... portable cases ...
//             // SKIPPED (G2): {".", "\xFF", true, true} and the \xFF/\xFE literal
//             // cases — 0xFF/0xFE are invalid UTF-8 and cannot be a Rust &str.
//             // The \x01 control-char cases below ARE valid UTF-8:
//             Case { pattern: ".", input: "\x01", cs: true, exp: true },
//         ]);
//     }
// }
//
// NOTE: pattern_match on the RHS is the LEAF matcher (T3.S1, pub fn). The
// parity corpus calls ONLY the leaf. match_pattern (delimiter-aware, T3.S2) is
// exercised solely by the one composition test (Task 9).
```

### Integration Points

```yaml
MODULE REGISTRATION: NONE. `pub mod pattern;` is already in src/core/mod.rs.
  Do NOT edit mod.rs.

DEPENDENCIES (this task): NONE new. Test code uses only std + the in-tree leaf
                           matcher (in scope via `use super::*;`). The composition
                           test additionally uses Pattern/match_pattern from T3.S2
                           (already in-tree or landing in parallel — no Cargo edit).

UPSTREAM (the function under test — already present):
  - pub fn pattern_match(pattern: &str, s: &str, case_sensitive: bool) -> bool
    (T3.S1, ~line 1061). Called by every parity Case. Its semantics (ASCII-only
    classes, dot excludes {\n,\r}, empty-core special case, + quantifier) are what
    the corpus validates. Do NOT modify it.

DOWNSTREAM: NONE. This is the capstone validation of P2.M1. P3 (rules.rs) consumes
            the matcher, not these tests.

CONFIG: none. ROUTES: none. DATABASE: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles clean (test code is #[cfg(test)] — not in the release build,
# but `cargo build` still type-checks the test module under cargo test). If rustc
# errors on src/core/pattern.rs (e.g. unknown escape `\f`/`\v` — G1, or a stray
# syntax slip), READ it and fix.

# Confirm the parity section is present and well-formed:
grep -n 'FIRMWARE PARITY CORPUS' src/core/pattern.rs   # the banner (expect 1+)
grep -n 'struct Case' src/core/pattern.rs              # the helper struct (expect 1)
grep -n 'fn assert_parity' src/core/pattern.rs         # the helper fn (expect 1)
grep -cE 'fn test_parity_' src/core/pattern.rs         # ~50 parity test fns
# Spot-check no \f / \v escapes leaked through:
! grep -nE '"\\\\[fv]"' src/core/pattern.rs || echo "WARN: raw \\f/\\v escape found (should be \\x0C/\\x0B)"
```

### Level 2: Unit Tests — the parity contract (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# Single-threaded is MANDATORY crate-wide (shared debouncer state in notifier.rs).
cargo test --bin qmkonnect pattern -- --test-threads=1
# Expected: every test in pattern::tests passes — the new test_parity_* corpus
# (1225 vectors across ~50 fns) AND the 160 existing + T3.S2's tests.
# A failure prints the offending pattern/input/cs/expected/got via assert_parity.
# Filter to a single grouping to see it in isolation:
cargo test --bin qmkonnect pattern::tests::test_parity_pm_start_anchor -- --test-threads=1
cargo test --bin qmkonnect pattern::tests::test_parity_wbi_ -- --test-threads=1
cargo test --bin qmkonnect pattern::tests::test_parity_eh_dot -- --test-threads=1
cargo test --bin qmkonnect pattern::tests::test_parity_match_pattern_single_dispatch -- --test-threads=1
```

### Level 3: Full-crate regression (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL bin tests green — pattern::tests (new parity + existing + T3.S2)
# + notifier + types. Proves the additive test section compiles in the full crate
# context and didn't disturb anything.

# Confirm the change surface is exactly one file:
git status --short
# Expected: modified:   src/core/pattern.rs   (ONLY this)
git diff --stat
# Expected: only src/core/pattern.rs changed.
```

### Level 4: Fidelity cross-check (optional, high-confidence)

```bash
# Cross-validate against the firmware's own corpus. The Rust parity vectors were
# transcribed from these C files, so a green firmware run corroborates the bools
# the Rust port encodes. (The Rust tests are STRICTLY STRONGER on the host side:
# they also assert the type-system skips are correctly excluded.)
cd /home/dustin/projects/qmk-notifier && ./run_all_tests.sh
# Expected: the full firmware corpus passes (it always does — this task does not
# touch the firmware). Particularly the \s/\S (with \f/\v), dot (excludes \r),
# + quantifier, and \b/\B cases.
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean (no NEW warnings on `src/core/pattern.rs`).
- [ ] `cargo test --bin qmkonnect pattern -- --test-threads=1` — all pattern tests pass (new parity + 160 existing + T3.S2).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` — full crate green.
- [ ] `git status` shows exactly ONE modified file: `src/core/pattern.rs`.

### Feature Validation (parity coverage)
- [ ] **All 8 firmware files** have a corresponding set of `test_parity_*` fns.
- [ ] **test_pattern_match.c**: 16 fns ported (380 cases; skip no-op #5).
- [ ] **test_char_classification.c**: 8 sections ported as loops (179 cases; skip 1 NULL).
- [ ] **test_metachar_verification.c**: 1 fn, 24 cases.
- [ ] **test_word_boundary_basic.c / _integration.c**: 4 + 5 fns (74 + 189 cases).
- [ ] **test_comprehensive_integration.c**: 8 fns (130 cases; reversed params normalized).
- [ ] **test_invalid_patterns.c**: 3 fns (88 cases) + 1 `no_panic` property test.
- [ ] **test_error_handling.c**: 8 fns (161 cases; skip 3 NULL; skip ~8 invalid-UTF-8).
- [ ] **Composition**: `test_parity_match_pattern_single_dispatch` proves `Pattern::Single` delegates to the leaf.
- [ ] Every skip (NULL, invalid-UTF-8) has an inline comment citing G2/G3.

### Code Quality Validation
- [ ] Parity corpus under a single `// ===== FIRMWARE PARITY CORPUS =====` banner at the END of `mod tests`.
- [ ] Naming prefix `test_parity_*` (disjoint from existing + T3.S2's `test_mp_*`/`test_pattern_serde_*`).
- [ ] `assert_parity` helper prints an indexed, precise failure message (pattern/input/cs/got/exp).
- [ ] C escapes correctly mapped (`\f`→`\x0C`, `\v`→`\x0B`; no raw `\f`/`\v` in the file).
- [ ] `"\\0"`-style 2-char literals ported verbatim (G4); reversed params normalized (G5); dot-`\r` asserts `false` (G6).
- [ ] No production code touched (consts/escapes/parse/NFA/match_with_anchors/leaf pattern_match all unchanged).
- [ ] No new Cargo dependencies; no `unsafe`; no `static`.
- [ ] Tests appended additively; prior tests + T3.S2 tests untouched.

### Documentation & Deployment
- [ ] The parity-section banner documents: source-of-truth = firmware C; a failure = fix the Rust (PRD §14); skip lists = type-system limits.
- [ ] No `docs/*.md` or README changes this task (test code only — Mode A).

---

## Anti-Patterns to Avoid

- ❌ Do NOT write 1225 individual `#[test]` fns (one per case). Use the table-driven
      `assert_parity(&[Case{...}])` pattern — one `#[test]` per firmware grouping.
      The firmware itself groups this way; cargo reports the failing index.
- ❌ Do NOT change the test's `exp` to match divergent Rust output. The firmware C
      is the source of truth (PRD §14); fix the Rust (G7). The ONLY exceptions are
      the G2/G3 skip lists (and those shouldn't be in the table at all).
- ❌ Do NOT use C `"\f"` / `"\v"` in Rust string literals — they don't compile (G1).
      Use `"\x0C"` / `"\x0B"`.
- ❌ Do NOT port the `\xFF`/`\xFE` cases as-is — they're invalid UTF-8 and cannot
      be a Rust `&str` (G2). Skip them with a comment. (The `\x01` cases are fine.)
- ❌ Do NOT port the NULL cases by switching the API to `Option<&str>`. Rust `&str`
      is never null; the cases are unrepresentable (G3). Skip with a comment.
- ❌ Do NOT confuse `"\\0"` (2-char literal: backslash + '0') with `"\0"` (NUL byte)
      (G4). The firmware corpus has NO actual NUL bytes — port the 2-char literal verbatim.
- ❌ Do NOT forget to SWAP the 3rd/4th args when porting test_comprehensive_integration.c
      (its helper order is `(pattern, str, EXPECTED, case_sensitive)` — G5).
- ❌ Do NOT assert dot matches `\r` because the firmware description says "should
      match carriage return" — the `expected_result` is `false` (G6). Port the expected bool.
- ❌ Do NOT port the 3500 perf / 920 crash-safety loops as bool-assertion tables —
      they assert NOTHING (results discarded). Represent them with ONE `no_panic`
      property test; do not fabricate expected bools.
- ❌ Do NOT duplicate T3.S2's `match_pattern`/`Pattern`/serde tests. The contract's
      "delimiter-aware match_pattern tests" is satisfied by ONE composition test
      (`test_parity_match_pattern_single_dispatch`) that proves `Single` delegates
      to the leaf on a cross-section.
- ❌ Do NOT touch production code. This task writes ONLY the `#[cfg(test)] mod tests`
      parity section. No `process_escapes`, no `parse_pattern`, no NFA, no leaf
      `pattern_match`, no `match_pattern`, no `rules.rs`, no `Cargo.toml`, no `mod.rs`.
- ❌ Do NOT run tests multi-threaded — `cargo test --bin qmkonnect -- --test-threads=1` (G8).
- ❌ Do NOT edit `PRD.md`, any `tasks.json`, `prd_snapshot.md`, `spec/HOST_RULES.md`,
      or any `plan/` file other than this item's own `PRP.md` + `research/`.

---

## Confidence Score: 9/10

This is a well-bounded **test-only** port: transcribe 1225 firmware vectors into
Rust `Case` tables behind a shared `assert_parity` helper. The per-file, per-grouping
counts + skip lists are authoritative (`research/firmware_corpus_enumeration.md`);
the porting strategy + all 6 type-system gotchas are fully derived
(`research/notes.md`, each VERIFIED — `\f`/`\v` via `rustc`, invalid-UTF-8 via the
UTF-8 spec, NULL via the `&str` type). The leaf matcher under test ALREADY
implements firmware semantics (classes are ASCII-only, dot excludes both `\n` and
`\r`, `+` is the linear Thompson quantifier, `\b`/`\B` thread the original string) —
confirmed by reading `src/core/pattern.rs` — so parity should hold on the first
pass; a failure is either a transcription slip (caught by the helper's indexed
message) or a genuine divergence (fix the Rust per G7). The naming prefix
`test_parity_*` is proven disjoint from the 160 existing tests and T3.S2's planned
`test_mp_*`/`test_pattern_serde_*`, so the parallel T3.S2 work coexists cleanly.
The 1-point reservation is for: (a) the `test_comprehensive_integration.c` reversed-
param order (G5) being mis-read (caught by a parity failure that reveals a
systematic cs/exp swap), (b) the long-pattern cases needing an owned-String helper
variant (a minor ergonomic wrinkle, Task 8), and (c) the composition test's
soft-dependency on T3.S2 landing `Pattern`/`match_pattern` (mitigated by writing it
last). All three are low-risk and recoverable. Scope is cleanly bounded: tests
only, one file, no production touch, no dependency change.
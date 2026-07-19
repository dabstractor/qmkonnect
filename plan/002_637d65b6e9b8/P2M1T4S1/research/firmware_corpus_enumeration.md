# Firmware Corpus Enumeration — Completeness Checklist

> Source: `/home/dustin/projects/qmk-notifier/test_*.c` (8 files in scope).
> ALL call the LEAF `pattern_match(pattern, input, case_sensitive)`.
> Produced by codebase recon; counts read from the actual arrays (not estimated).
> Strategy/gotchas in `notes.md`; this file is the per-grouping COUNT + SKIP checklist.

## Grand total
- **1225 assertion cases** (have a checked bool) — ALL to be ported as `Case` tables.
- **4420 non-assertion execs** (perf + crash-safety, result discarded) → represented
  by ONE `test_parity_invalid_no_panic` property test; not bool-parity.

## 1. test_pattern_match.c — struct_array (`test_case_t`), 380 cases, 17 funcs
| # | function | cases | covers |
|---|----------|------:|--------|
| 1 | test_start_anchor | 8 | `^` prefix |
| 2 | test_end_anchor | 8 | `$` suffix |
| 3 | test_full_anchor | 8 | `^…$` exact |
| 4 | test_anchors_with_wildcards | 9 | `^`/`$` + `*` |
| 5 | test_character_classification | **0** | **NO-OP doc stub** (printf only; skip) |
| 6 | test_basic_metacharacter_escapes | 14 | `\d\D\w\W\s\S` escape-processing (not literal) |
| 7 | test_basic_metacharacter_matching | 68 | `\d\D\w\W\s\S` class match |
| 8 | test_word_boundary_escape_processing | 9 | `\b\B` escape-processing |
| 9 | test_escape_sequences | 25 | `\^\$\*\\` + trailing `\` |
| 10 | test_backward_compatibility | 13 | unanchored substring, `*`, empty |
| 11 | test_case_sensitivity | 10 | case on/off |
| 12 | test_pattern_parsing | 29 | anchor+escape behavior |
| 13 | test_edge_cases | 13 | `^^`/`$$`/`^$`/trailing `\`/`^**$` |
| 14 | test_metacharacters_with_anchors | 55 | classes + `^`/`$` (incl `\w+@\w+`) |
| 15 | test_metacharacters_with_wildcards | 50 | classes + `*` |
| 16 | test_metacharacter_case_sensitivity | 34 | classes case on/off |
| 17 | test_metacharacter_backward_compatibility | 27 | pre-feature regression |

## 2. test_char_classification.c — custom_helper `test_class(p,in,cs,exp,meta,desc)`, 179
| section | cases | covers |
|---------|------:|--------|
| `\d` (digits 0-9 + 14 non) | 24 | `\d` pos+neg |
| `\D` | 4 | samples |
| `\w` (a-z/A-Z/0-9/_ + 33 non) | 96 | `\w` pos+neg |
| `\W` | 5 | samples |
| `\s` (6 ws + 20 non) | 26 | `\s` pos+neg |
| `\S` | 4 | samples |
| `\b`/`\B` | 14 | boundary assertions |
| anchored single-char classes | 6 | `^\d$ ^\w$ ^\s$ ^\S$` |
Port the char arrays as `&[u8]` and LOOP (mirrors firmware `for` loops). 1 NULL
input case (`\bword\b` vs NULL) → SKIP (G3).

## 3. test_metachar_verification.c — printf_helper, 24 (1 function, all inline)
All 24 cases verbatim in `notes.md` enumeration; portable as one `#[test]` with a
24-entry `Case` table. 1 special byte (`\t` → `"\t"`).

## 4. test_word_boundary_basic.c — struct_array, 74, 4 funcs
| function | cases | covers |
|----------|------:|--------|
| test_word_boundary_basic | 37 | `\b`/`\B` start/end/mid, case |
| test_word_boundary_with_anchors | 13 | `\b`/`\B` + `^`/`$`/`^…$` |
| test_word_boundary_with_wildcards | 12 | `\b`/`\B` + `*` |
| test_multiple_word_boundaries | 12 | `\b\b`/`\B\B`/`\b\B`/`\b\d\b` |
No special bytes, no NULL.

## 5. test_word_boundary_integration.c — struct_array, 189, 5 funcs
| function | cases | covers |
|----------|------:|--------|
| test_word_boundaries_with_anchors | 34 | `^\b\w\w\w\w\b$` etc. |
| test_word_boundaries_with_wildcards | 34 | `\b*word*\b` etc. |
| test_word_boundaries_with_metacharacters | 57 | `\b\d`/`\d\b`/`\b\w`/`\B\w` |
| test_word_boundary_edge_cases | 52 | single-char, `@`, position-0 |
| test_word_boundary_case_sensitivity | 12 | case on/off |
No special bytes, no NULL.

## 6. test_comprehensive_integration.c — custom_helper `test_pattern_with_memory`, 130
**⚠️ helper order is (pattern, str, EXPECTED, case_sensitive) — REVERSED (G5).**
| function | cases | covers |
|----------|------:|--------|
| test_complex_metacharacter_combinations | 16 | `\d\w\s`, `\D\W\S` mixed |
| test_metacharacters_with_anchors_complex | 14 | `^\d\w\s$` |
| test_metacharacters_with_wildcards_complex | 18 | `*\d`, `*\w`, `test*\d` |
| test_word_boundaries_complex | 14 | `\b\d\d\b`, `\B\w\B` |
| test_dot_metacharacter_complex | 22 | `.` + classes/anchors/wildcards/newline |
| test_all_features_combined | 18 | email/phone `^.\d.\w.$` |
| test_case_sensitivity_complex | 10 | `^Hello\s\w*$` |
| test_edge_cases_complex | 18 | empty, long, dot+newline/tab |
(test_performance_complex + test_memory_management_complex = 3500 NON-assertion → skip.)

## 7. test_invalid_patterns.c — custom_helper `test_pattern(p,in,cs,exp,desc)`, 88
| function | cases | covers |
|----------|------:|--------|
| test_invalid_regex_patterns | 40 | unmatched `[]()`, `+?`, invalid classes/escapes |
| test_boundary_conditions | 27 | empty, single-char, whitespace, short `.`/`*`/`^`/`$` |
| test_error_conditions | 21 | `a**`, `***`, `********` stress, mixed |
(test_comprehensive_error_handling = 920 NON-assertion crash-safety → port as
`test_parity_invalid_no_panic` property test only.)

## 8. test_error_handling.c — struct_array + inline, 161, 9 funcs
| function | cases | covers |
|----------|------:|--------|
| test_null_pointer_handling | 3 | **NULL — SKIP all 3 (G3)** |
| test_invalid_escape_sequences | 30 | `\x\z\1\@\~` treated as literal |
| test_malformed_patterns | 22 | trailing `\`, mid `^`/`$`, empty anchors |
| test_long_patterns_and_strings | 5 | 400+/1000+ char (inline) |
| test_special_character_edge_cases | 15 | UTF-8 café, **0xFF/0xFE SKIP (G2)**, dot vs ctrl |
| test_memory_allocation_edge_cases | 11 | multi-escape, metachar, minimal |
| test_word_boundary_edge_cases | 26 | `\b\w+`, `\B\w\B`, `@`, `\b\w+\b\s+\b\w+\b` |
| test_dot_metacharacter_edge_cases | 32 | dot vs all types, `\r` excluded (G6) |
| test_complex_error_scenarios | 17 | `+`, backtracking stress, `\b\B` contradiction |

## Consolidated SKIP lists (document each in a comment at its table)

### NULL-pointer cases — SKIP (no Rust &str analog; G3) — 4 total
- test_error_handling.c::test_null_pointer_handling: `(NULL,"test")`, `("test",NULL)`, `(NULL,NULL)` → all expect false.
- test_char_classification.c: `\bword\b` vs NULL → false.

### Invalid-UTF-8 bytes — SKIP (0xFE/0xFF not valid in &str; G2) — ≈8 cases
- test_error_handling.c::test_special_character_edge_cases: `\s`/`\S`/`\w`/`\W`/`.` vs `\xFF`;
  literal `test\xFF`/`test\xFE` matches. (Port the `\x01` control-char cases — those ARE valid UTF-8.)

### Special-byte ESCAPES — PORT with corrected Rust escapes (G1)
C `"\f"` → Rust `"\x0C"`; C `"\v"` → Rust `"\x0B"`. `\t`/`\n`/`\r` are the same.
Affected files: test_pattern_match.c (\s/\S sections), test_char_classification.c
(\s/\d/\w arrays), test_metachar_verification.c (\t), test_comprehensive_integration.c
(\s combos, dot+newline), test_invalid_patterns.c (\t + crash-loop input[7]),
test_error_handling.c (dot vs \t/\r/\f/\v).

### `"\0"` vs `"\\0"` (G4)
`"\\0"` / `"\\n"` / `"\\t"` in test_invalid_patterns.c are 2-CHAR LITERALS
(backslash + letter), NOT control bytes. Port VERBATIM as Rust `"\\0"` etc.

### Dot `\r` (G6)
`{".", "\r", true, false, "Dot should match carriage return"}` — port expected
**false** (description is misleading). Rust dot already excludes `\r`.
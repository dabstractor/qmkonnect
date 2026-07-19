//! Pattern escape processing — byte-for-byte port of the firmware
//! `process_escapes()` (qmk-notifier `pattern_match.c:30-75`).
//!
//! This is the first stage of the P2.M1 "Pattern Matcher Port" pipeline:
//! `parse_pattern` → `process_escapes` → `nfa_compile` → `nfa_match`. It is a
//! pure byte transform and does NOT detect anchors, compile an NFA, or match
//! anything — those are later subtasks (see architecture `external_deps.md` §3).

// The placeholder-byte constants and `process_escapes` below are the
// crate-internal contract for the P2.M1 matcher pipeline. They are delivered
// ahead of their first non-test consumer (`parse_pattern`, P2.M1.T1.S2; the
// NFA compiler, P2.M1.T2; parity tests, P2.M1.T4), so silence the inherent
// "never used" warnings here — mirroring the existing crate idiom
// (`src/platforms/mod.rs:18`, `src/platforms/hyprland.rs:523`).
#![allow(dead_code)]

// Processed-pattern placeholder bytes — the contract between `process_escapes`
// and the Thompson NFA compiler (P2.M1.T2.S1). Mirrors the firmware
// `pattern_match.c` byte contract (see architecture `external_deps.md` §3 and
// firmware `pattern_match_architecture.md` lines 66-95).
#[allow(dead_code)] // pub(crate) API consumed by parse_pattern (P2.M1.T1.S2) + NFA (P2.M1.T2)
pub(crate) const ESC_CARET: u8 = 0x01; // \^  escaped literal ^
pub(crate) const ESC_DOLLAR: u8 = 0x02; // \$  escaped literal $
pub(crate) const ESC_STAR: u8 = 0x03; // \*  escaped literal *
pub(crate) const ESC_BSLASH: u8 = 0x04; // \\  escaped literal backslash
pub(crate) const CLASS_DIGIT: u8 = 0x05; // \d
pub(crate) const CLASS_NDIGIT: u8 = 0x06; // \D
pub(crate) const CLASS_WORD: u8 = 0x07; // \w
pub(crate) const CLASS_NWORD: u8 = 0x08; // \W
pub(crate) const CLASS_SPACE: u8 = 0x09; // \s
pub(crate) const CLASS_NSPACE: u8 = 0x0A; // \S
pub(crate) const ASSERT_BOUND: u8 = 0x0B; // \b  zero-width word-boundary assertion
pub(crate) const ASSERT_NBOUND: u8 = 0x0C; // \B  zero-width non-boundary assertion
pub(crate) const DOT_META: u8 = 0x0D; // bare .  (any char except \n/\r)
pub(crate) const PLUS_QUANT: u8 = 0x0E; // bare + after a consuming element
pub(crate) const GLOB_STAR: u8 = 0x2A; // bare *  glob wildcard
// Literal '.' (0x2E), '+' (0x2B), '\' (0x5C) are emitted as their ASCII bytes.
// Literal '.' (0x2E), '+' (0x2B), '\' (0x5C) are emitted as their ASCII bytes.

/// Transform a human-authored pattern string into the processed byte stream the
/// Thompson NFA compiler consumes — a faithful, byte-for-byte port of the
/// firmware `process_escapes(const char *pattern)` (`pattern_match.c:30-75`).
///
/// # Placeholder-byte contract
///
/// The returned `Vec<u8>` carries placeholder bytes that the NFA compiler
/// (P2.M1.T2.S1) dispatches on. They are the contract between escape
/// processing and matching:
///
/// | Byte(s)        | Source             | Meaning                              |
/// |----------------|--------------------|--------------------------------------|
/// | `0x01`–`0x04`  | `\^ \$ \* \\`      | escaped literal `^ $ * \`            |
/// | `0x2E`/`0x2B`  | `\.` / `\+`        | **literal** dot / plus (NOT meta)    |
/// | `0x05`–`0x0A`  | `\d \D \w \W \s \S`| character classes                    |
/// | `0x0B`/`0x0C`  | `\b` / `\B`        | zero-width boundary assertions        |
/// | `0x0D`         | bare `.`           | dot metacharacter                    |
/// | `0x2A`         | bare `*`           | glob wildcard                        |
/// | `0x0E`         | bare `+` after a consumable element | one-or-more quantifier |
/// | `0x2B`         | bare `+` otherwise | literal plus                         |
/// | `0x5C` + byte  | `\<unrecognized>`  | literal backslash + char (2 bytes)   |
/// | the byte       | anything else      | ordinary literal                     |
///
/// # Semantics
///
/// - Iterates the input **by byte** (not `char`), mirroring the firmware C
///   `char*` walk; ASCII metachars are all `< 0x80`, so UTF-8 continuation
///   bytes pass through untouched.
/// - **Stops at the first NUL byte** (`0x00`) to mirror C `while (*src)`. The
///   NUL is NOT emitted.
/// - Does **not** append a trailing NUL — the `Vec<u8>` length is the
///   authoritative end marker (unlike the firmware, which NUL-terminates the C
///   string for downstream C consumers).
/// - `last_consumable` (whether the previous emitted element consumed an input
///   char) governs whether a bare `+` is the `0x0E` quantifier or the `0x2B`
///   literal. It starts `false` and is reset to `false` by `\b`, `\B`, bare
///   `*`, and the `0x0E` quantifier itself.
///
/// # Scope
///
/// Anchor (`^`/`$`) **detection is not performed here** — bare `^`/`$` pass
/// through as ordinary literals (`0x5E`/`0x24`). Anchor handling is the job of
/// `parse_pattern` (P2.M1.T1.S2), which strips them from the original pattern
/// before feeding the core substring here. See PRD §4 + architecture
/// `external_deps.md` §3 for the contract sources.
pub(crate) fn process_escapes(pattern: &str) -> Vec<u8> {
    let bytes = pattern.as_bytes(); // GOTCHA-C: iterate BYTES, not chars
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len()); // output <= input length
    let mut last_consumable = false;
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        if b == 0x00 {
            break; // GOTCHA-C: stop at NUL (mirrors C `while (*src)`)
        }
        match b {
            b'\\' => {
                // A backslash is an escape only if a non-NUL byte follows it.
                // A backslash at end-of-string (or immediately before a NUL) is
                // a trailing literal backslash.
                if i + 1 < bytes.len() && bytes[i + 1] != 0x00 {
                    match bytes[i + 1] {
                        b'^' => {
                            out.push(ESC_CARET);
                            last_consumable = true;
                        }
                        b'$' => {
                            out.push(ESC_DOLLAR);
                            last_consumable = true;
                        }
                        b'*' => {
                            out.push(ESC_STAR);
                            last_consumable = true;
                        }
                        b'\\' => {
                            out.push(ESC_BSLASH);
                            last_consumable = true;
                        }
                        b'.' => {
                            out.push(b'.'); // GOTCHA-A: literal dot (0x2E), NOT 0x0D
                            last_consumable = true;
                        }
                        b'+' => {
                            out.push(b'+'); // GOTCHA-A: literal plus (0x2B), NOT 0x0E
                            last_consumable = true;
                        }
                        b'd' => {
                            out.push(CLASS_DIGIT);
                            last_consumable = true;
                        }
                        b'D' => {
                            out.push(CLASS_NDIGIT);
                            last_consumable = true;
                        }
                        b'w' => {
                            out.push(CLASS_WORD);
                            last_consumable = true;
                        }
                        b'W' => {
                            out.push(CLASS_NWORD);
                            last_consumable = true;
                        }
                        b's' => {
                            out.push(CLASS_SPACE);
                            last_consumable = true;
                        }
                        b'S' => {
                            out.push(CLASS_NSPACE);
                            last_consumable = true;
                        }
                        b'b' => {
                            out.push(ASSERT_BOUND);
                            last_consumable = false;
                        }
                        b'B' => {
                            out.push(ASSERT_NBOUND);
                            last_consumable = false;
                        }
                        _ => {
                            // GOTCHA-B: unrecognized escape — keep BOTH bytes.
                            out.push(b'\\');
                            out.push(bytes[i + 1]);
                            last_consumable = true;
                        }
                    }
                    i += 2;
                } else {
                    // Trailing lone backslash → literal 0x5C.
                    out.push(b'\\');
                    last_consumable = true;
                    i += 1;
                }
            }
            b'*' => {
                out.push(GLOB_STAR);
                last_consumable = false;
                i += 1;
            }
            b'+' => {
                if last_consumable {
                    out.push(PLUS_QUANT);
                    last_consumable = false;
                } else {
                    out.push(b'+'); // literal plus (not after a consumable element)
                    last_consumable = true;
                }
                i += 1;
            }
            b'.' => {
                out.push(DOT_META);
                last_consumable = true;
                i += 1;
            }
            _ => {
                out.push(b); // ordinary literal (incl. bare '^' = 0x5E, '$' = 0x24)
                last_consumable = true;
                i += 1;
            }
        }
    }

    out // NO trailing NUL — Vec<u8> length is authoritative
}

/// The result of parsing a user pattern: anchor flags + the `process_escapes()`-
/// processed core the Thompson NFA compiler consumes. Rust analog of the firmware
/// `parsed_pattern_t` (`pattern_match.c`), minus the C malloc/fallback fields:
/// Rust `Vec<u8>` owns its heap buffer, so there is no `processed_pattern` to
/// free and no `core_pattern` raw-fallback pointer. The NFA compiler (P2.M1.T2)
/// reads `core`; the matcher entry `match_with_anchors` (P2.M1.T3) reads the
/// anchor flags to pick exact / prefix / suffix / substring strategy.
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

/// Detect the anchors in a human-authored pattern and carve out the core the
/// Thompson NFA consumes — a faithful port of the firmware `parse_pattern`
/// (`pattern_match.c:parse_pattern`).
///
/// This is stage 2 of the P2.M1 matcher pipeline:
/// `parse_pattern` → `process_escapes` → `nfa_compile` → `nfa_match` →
/// `match_with_anchors`. See architecture `external_deps.md` §3 and PRD §4 for
/// the contract sources.
///
/// # Anchor rules
///
/// - **Start anchor**: if the pattern begins with `^`, `start_anchored` is set
///   and the `^` is skipped. Only the very first byte is considered — `\^`
///   would already be an escape sequence processed to `0x01` later by
///   [`process_escapes`], never seen as an anchor here.
/// - **End anchor**: a trailing `$` is a real end anchor ONLY when an EVEN
///   number of backslashes (0, 2, 4, …) immediately precede it. An ODD count
///   means the `$` is escaped (`\$`) and stays in the core — `process_escapes`
///   turns `\$` into the `0x02` literal. This is the standard "is the final
///   metacharacter quoted?" test: walk left from the `$` counting RAW
///   CONSECUTIVE backslashes (do **not** pair them as escapes); even ⇒ unquoted.
///
///   The four canonical cases (Rust source literals):
///
///   | pattern literal   | backslashes | anchored? | core fed to `process_escapes` |
///   |-------------------|-------------|-----------|--------------------------------|
///   | `"abc$"`          | 0 (even)    | ✅ anchor | `"abc"`            → `[61 62 63]`             |
///   | `"abc\\$"`        | 1 (odd)     | ❌ escaped | `"abc\$"` → `[61 62 63 02]` (ESC_DOLLAR) |
///   | `"abc\\\\$"`       | 2 (even)    | ✅ anchor | `"abc\\"` → `[61 62 63 04]` (ESC_BSLASH) |
///   | `"abc\\\\\\$"`      | 3 (odd)     | ❌ escaped | `"abc\\\$"` → `[61 62 63 04 02]`       |
///
/// # Degenerate / edge cases
///
/// - `"^$"` → both anchors set, empty core (matches the empty string only).
/// - A lone `"^"` sets `start_anchored` only (empty core); a lone `"$"` sets
///   `end_anchored` only (empty core). The `end > start` guard before each
///   anchor check prevents underflow and matches the firmware's degenerate-input
///   rejection.
/// - A leading `^` is detected only at index 0: `"^^"` anchors on the first,
///   and the second `^` is a bare literal that passes through as `0x5E`.
/// - A non-trailing `$` (e.g. interior, or the first `$` of `"$$"`) is a bare
///   literal that passes through as `0x24`.
///
/// # NUL / `strlen` parity
///
/// The firmware computes `end = pattern + strlen(pattern)`, stopping at the
/// first `0x00`. A Rust `&str` *can* hold a NUL byte (valid UTF-8), so for
/// byte-for-byte parity this function computes the effective length the same
/// way — *before* anchor detection — otherwise the anchor **flags** would
/// diverge on a NUL-containing input (a `$` past a NUL would be wrongly seen
/// as a trailing anchor). Real `rules.toml` patterns never contain NUL; this
/// is defensive but keeps the port honest and mirrors `process_escapes`' own
/// NUL-stop.
///
/// The carved `core` substring is then handed to [`process_escapes`], which
/// produces the placeholder bytes the NFA compiler dispatches on.
pub(crate) fn parse_pattern(pattern: &str) -> ParsedPattern {
    let bytes = pattern.as_bytes();
    // GOTCHA-2: mirror firmware strlen — stop at the first NUL byte, BEFORE
    // anchor detection (else anchor flags diverge on NUL-containing input).
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());

    let mut start = 0usize;
    let mut end = len;
    let mut start_anchored = false;
    let mut end_anchored = false;

    // START anchor: a leading '^' (only at the very front; '\^' would be
    // processed to 0x01 later by process_escapes, never seen as an anchor here).
    if end > start && bytes[start] == b'^' {
        start_anchored = true;
        start += 1;
    }

    // END anchor: a trailing '$' preceded by an EVEN number of backslashes
    // (GOTCHA-1). The walk counts RAW CONSECUTIVE backslashes — do NOT pair
    // them as escapes. Decrement-then-check so indices end-2 .. start (incl.)
    // are inspected, mirroring the C `for (check = end-2; check >= start; --)`.
    if end > start && bytes[end - 1] == b'$' {
        let mut bs = 0usize;
        let mut k = end - 1; // index of the '$'
        while k > start {
            k -= 1; // step left onto the byte before '$'
            if bytes[k] == b'\\' {
                bs += 1;
            } else {
                break;
            }
        }
        if bs % 2 == 0 {
            // even (0,2,4,...) => unescaped '$'
            end_anchored = true;
            end -= 1; // drop the '$'
        }
        // odd => '$' is escaped: leave it in the core; process_escapes turns
        // the trailing '\$' into ESC_DOLLAR (0x02).
    }

    // Carve the core and process its escapes. The slice is safe: '^','$','\\',
    // and NUL are all ASCII (< 0x80) => UTF-8 char boundaries, so trimming at
    // these indices never splits a multi-byte sequence. Do NOT rebuild a
    // String — slice the &str directly and hand it to process_escapes.
    let core = process_escapes(&pattern[start..end]);

    ParsedPattern {
        core,
        start_anchored,
        end_anchored,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Plain passthrough / empty ---

    #[test]
    fn test_plain_passthrough() {
        // "hello" -> [68 65 6C 6C 6F]
        assert_eq!(process_escapes("hello"), vec![0x68, 0x65, 0x6C, 0x6C, 0x6F]);
    }

    #[test]
    fn test_empty() {
        // "" -> [] (empty)
        assert_eq!(process_escapes(""), Vec::<u8>::new());
    }

    // --- Escaped literals: \^ \$ \* \\ ---

    #[test]
    fn test_escaped_literals_caret_dollar_star_bslash() {
        assert_eq!(process_escapes("\\^"), vec![ESC_CARET]); // [01]
        assert_eq!(process_escapes("\\$"), vec![ESC_DOLLAR]); // [02]
        assert_eq!(process_escapes("\\*"), vec![ESC_STAR]); // [03]
        assert_eq!(process_escapes("\\\\"), vec![ESC_BSLASH]); // [04]
    }

    // --- GOTCHA-A: \. and \+ are LITERAL, not placeholders ---

    #[test]
    fn test_escaped_dot_and_plus_are_literal() {
        assert_eq!(process_escapes("\\."), vec![0x2E]); // literal dot, NOT 0x0D
        assert_eq!(process_escapes("\\+"), vec![0x2B]); // literal plus, NOT 0x0E
    }

    // --- Character classes: \d \D \w \W \s \S ---

    #[test]
    fn test_classes_ddwwss() {
        assert_eq!(process_escapes("\\d"), vec![CLASS_DIGIT]); // [05]
        assert_eq!(process_escapes("\\D"), vec![CLASS_NDIGIT]); // [06]
        assert_eq!(process_escapes("\\w"), vec![CLASS_WORD]); // [07]
        assert_eq!(process_escapes("\\W"), vec![CLASS_NWORD]); // [08]
        assert_eq!(process_escapes("\\s"), vec![CLASS_SPACE]); // [09]
        assert_eq!(process_escapes("\\S"), vec![CLASS_NSPACE]); // [0A]
    }

    // --- Zero-width assertions: \b \B ---

    #[test]
    fn test_assertions_bb() {
        assert_eq!(process_escapes("\\b"), vec![ASSERT_BOUND]); // [0B]
        assert_eq!(process_escapes("\\B"), vec![ASSERT_NBOUND]); // [0C]
    }

    // --- Bare ., *, and + at start ---

    #[test]
    fn test_bare_dot_meta() {
        assert_eq!(process_escapes("."), vec![DOT_META]); // [0D]
    }

    #[test]
    fn test_bare_glob_star() {
        assert_eq!(process_escapes("*"), vec![GLOB_STAR]); // [2A]
    }

    #[test]
    fn test_bare_plus_at_start_is_literal() {
        // Start: last_consumable=false -> literal plus.
        assert_eq!(process_escapes("+"), vec![0x2B]); // [2B]
    }

    // --- Quantifier (+) after a consumable element ---

    #[test]
    fn test_quantifier_after_consumable() {
        assert_eq!(process_escapes("a+"), vec![0x61, PLUS_QUANT]); // [61 0E]
        assert_eq!(process_escapes("a+b"), vec![0x61, PLUS_QUANT, 0x62]); // [61 0E 62]
    }

    #[test]
    fn test_quantifier_after_class() {
        assert_eq!(process_escapes("\\d+"), vec![CLASS_DIGIT, PLUS_QUANT]); // [05 0E]
    }

    #[test]
    fn test_dot_star_dot_plus() {
        assert_eq!(process_escapes("a*"), vec![0x61, GLOB_STAR]); // [61 2A]
        assert_eq!(process_escapes(".*"), vec![DOT_META, GLOB_STAR]); // [0D 2A]
        assert_eq!(process_escapes(".+"), vec![DOT_META, PLUS_QUANT]); // [0D 0E]
    }

    // --- + is literal after non-consumable elements (* \b \B prior quantifier) ---

    #[test]
    fn test_plus_after_glob_is_literal() {
        // a*+ -> [61 2A 2B] (+ after * is literal)
        assert_eq!(process_escapes("a*+"), vec![0x61, GLOB_STAR, 0x2B]);
    }

    #[test]
    fn test_plus_after_boundary_is_literal() {
        // \b+ -> [0B 2B] (+ after \b is literal)
        assert_eq!(process_escapes("\\b+"), vec![ASSERT_BOUND, 0x2B]);
        // \B+ -> [0C 2B]
        assert_eq!(process_escapes("\\B+"), vec![ASSERT_NBOUND, 0x2B]);
    }

    #[test]
    fn test_chained_plus_second_is_literal() {
        // a++ -> [61 0E 2B] (2nd + after quantifier is literal)
        assert_eq!(process_escapes("a++"), vec![0x61, PLUS_QUANT, 0x2B]);
    }

    // --- GOTCHA-B: unrecognized escape keeps BOTH bytes ---

    #[test]
    fn test_unrecognized_escape_keeps_both_bytes() {
        assert_eq!(process_escapes("\\x"), vec![0x5C, 0x78]); // backslash + 'x'
        assert_eq!(process_escapes("\\z"), vec![0x5C, 0x7A]); // backslash + 'z'
    }

    // --- Trailing lone backslash ---

    #[test]
    fn test_trailing_lone_backslash_is_literal() {
        // "abc\" -> [61 62 63 5C]
        assert_eq!(process_escapes("abc\\"), vec![0x61, 0x62, 0x63, 0x5C]);
    }

    // --- Multiple escapes in one pattern ---

    #[test]
    fn test_two_escapes() {
        // \^\$ -> [01 02]
        assert_eq!(process_escapes("\\^\\$"), vec![ESC_CARET, ESC_DOLLAR]);
    }

    // --- Bare ^ and $ pass through (anchor detection is NOT done here) ---

    #[test]
    fn test_bare_caret_dollar_passthrough() {
        // Bare ^ and $ are ordinary literals here (no case for them as bare
        // bytes in process_escapes — only the escaped forms).
        assert_eq!(process_escapes("^"), vec![0x5E]);
        assert_eq!(process_escapes("$"), vec![0x24]);
        assert_eq!(process_escapes("^a$"), vec![0x5E, 0x61, 0x24]);
    }

    // --- GOTCHA-C: iterate by byte; stop at first NUL; no trailing NUL ---

    #[test]
    fn test_nul_stop_truncates() {
        // A pattern containing an interior NUL byte truncates at the NUL,
        // mirroring the firmware C `while (*src)`. The NUL itself is NOT emitted.
        let input = std::str::from_utf8(b"ab\0cd").unwrap();
        assert_eq!(process_escapes(input), vec![0x61, 0x62]); // [61 62]
    }

    #[test]
    fn test_backslash_before_nul_is_trailing_literal() {
        // A backslash immediately before a NUL is the trailing-lone-backslash
        // case (C: *(src+1)=='\0' is falsy -> second branch emits literal '\').
        let input = std::str::from_utf8(b"ab\\\0cd").unwrap();
        assert_eq!(process_escapes(input), vec![0x61, 0x62, 0x5C]); // [61 62 5C]
    }

    #[test]
    fn test_no_trailing_nul_appended() {
        // The Rust Vec<u8> length is authoritative; no 0x00 is appended.
        let out = process_escapes("a");
        assert_eq!(out, vec![0x61]);
        assert_eq!(out.len(), 1); // would be 2 if a trailing NUL were appended
    }

    // ========================================================================
    // parse_pattern — anchor detection + core extraction (P2.M1.T1.S2)
    //
    // Parity table: research/notes.md §3 (27 inputs). `core` is the
    // process_escapes() output of the carved substring; the anchor flags are
    // detected per the even-backslash-count rule. These assert ParsedPattern
    // directly — STRICTLY STRONGER than the firmware's end-to-end-only tests
    // (the C parse_pattern is `static` and only reachable via pattern_match).
    // ========================================================================

    // --- Anchor detection (no escape interaction) — rows 1-12 ---

    #[test]
    fn test_parse_empty() {
        // row 1: "" -> no anchors, empty core.
        assert_eq!(
            parse_pattern(""),
            ParsedPattern {
                core: vec![],
                start_anchored: false,
                end_anchored: false,
            }
        );
    }

    #[test]
    fn test_parse_plain_no_anchors() {
        // row 2: "hello" -> no anchors.
        assert_eq!(
            parse_pattern("hello"),
            ParsedPattern {
                core: vec![0x68, 0x65, 0x6C, 0x6C, 0x6F],
                start_anchored: false,
                end_anchored: false,
            }
        );
    }

    #[test]
    fn test_parse_start_anchor_only() {
        // row 3: "^hello" -> start anchored.
        assert_eq!(
            parse_pattern("^hello"),
            ParsedPattern {
                core: vec![0x68, 0x65, 0x6C, 0x6C, 0x6F],
                start_anchored: true,
                end_anchored: false,
            }
        );
    }

    #[test]
    fn test_parse_end_anchor_only() {
        // row 4: "hello$" -> end anchored.
        assert_eq!(
            parse_pattern("hello$"),
            ParsedPattern {
                core: vec![0x68, 0x65, 0x6C, 0x6C, 0x6F],
                start_anchored: false,
                end_anchored: true,
            }
        );
    }

    #[test]
    fn test_parse_both_anchors() {
        // row 5: "^hello$" -> both anchored.
        assert_eq!(
            parse_pattern("^hello$"),
            ParsedPattern {
                core: vec![0x68, 0x65, 0x6C, 0x6C, 0x6F],
                start_anchored: true,
                end_anchored: true,
            }
        );
    }

    #[test]
    fn test_parse_lone_start_anchor() {
        // row 6: "^" -> start-only, empty core.
        assert_eq!(
            parse_pattern("^"),
            ParsedPattern {
                core: vec![],
                start_anchored: true,
                end_anchored: false,
            }
        );
    }

    #[test]
    fn test_parse_lone_end_anchor() {
        // row 7: "$" -> end-only, empty core.
        assert_eq!(
            parse_pattern("$"),
            ParsedPattern {
                core: vec![],
                start_anchored: false,
                end_anchored: true,
            }
        );
    }

    #[test]
    fn test_parse_both_anchors_empty_core() {
        // row 8: "^$" -> both anchored, empty core (matches the empty string).
        assert_eq!(
            parse_pattern("^$"),
            ParsedPattern {
                core: vec![],
                start_anchored: true,
                end_anchored: true,
            }
        );
    }

    #[test]
    fn test_parse_double_caret_second_is_literal() {
        // row 9: "^^" -> start anchored on FIRST '^'; the second is a bare
        // literal that passes through as 0x5E.
        assert_eq!(
            parse_pattern("^^"),
            ParsedPattern {
                core: vec![0x5E],
                start_anchored: true,
                end_anchored: false,
            }
        );
    }

    #[test]
    fn test_parse_double_dollar_first_is_literal() {
        // row 10: "$$" -> the FIRST '$' is an interior/non-trailing literal
        // (passes through as 0x24); the SECOND '$' is the trailing anchor.
        assert_eq!(
            parse_pattern("$$"),
            ParsedPattern {
                core: vec![0x24],
                start_anchored: false,
                end_anchored: true,
            }
        );
    }

    #[test]
    fn test_parse_anchored_single_char() {
        // row 11: "^a$" -> both anchored, core [0x61].
        assert_eq!(
            parse_pattern("^a$"),
            ParsedPattern {
                core: vec![0x61],
                start_anchored: true,
                end_anchored: true,
            }
        );
    }

    #[test]
    fn test_parse_anchored_glob_star() {
        // row 12: "^*$" -> both anchored, core [GLOB_STAR].
        assert_eq!(
            parse_pattern("^*$"),
            ParsedPattern {
                core: vec![GLOB_STAR],
                start_anchored: true,
                end_anchored: true,
            }
        );
    }

    // --- Even-backslash-count rule (GOTCHA-1) — rows 13-20 ---

    #[test]
    fn test_parse_end_anchor_even_backslash_zero() {
        // row 13: "abc$" -> 0 backslashes (even) => anchor; core "abc".
        assert_eq!(
            parse_pattern("abc$"),
            ParsedPattern {
                core: vec![0x61, 0x62, 0x63],
                start_anchored: false,
                end_anchored: true,
            }
        );
    }

    #[test]
    fn test_parse_escaped_dollar_odd_backslash_one() {
        // row 14: "abc\$" -> 1 backslash (odd) => escaped; core "abc\$"
        // -> process_escapes turns "\$" into ESC_DOLLAR.
        assert_eq!(
            parse_pattern("abc\\$"),
            ParsedPattern {
                core: vec![0x61, 0x62, 0x63, ESC_DOLLAR],
                start_anchored: false,
                end_anchored: false,
            }
        );
    }

    #[test]
    fn test_parse_end_anchor_even_backslash_two() {
        // row 15: "abc\\$" -> 2 backslashes (even) => anchor; the "\\" pair
        // is a literal backslash in the core -> ESC_BSLASH.
        assert_eq!(
            parse_pattern("abc\\\\$"),
            ParsedPattern {
                core: vec![0x61, 0x62, 0x63, ESC_BSLASH],
                start_anchored: false,
                end_anchored: true,
            }
        );
    }

    #[test]
    fn test_parse_escaped_dollar_odd_backslash_three() {
        // row 16: "abc\\\$" -> 3 backslashes (odd) => escaped; "\\" pair +
        // "\$" -> core [.., ESC_BSLASH, ESC_DOLLAR].
        assert_eq!(
            parse_pattern("abc\\\\\\$"),
            ParsedPattern {
                core: vec![0x61, 0x62, 0x63, ESC_BSLASH, ESC_DOLLAR],
                start_anchored: false,
                end_anchored: false,
            }
        );
    }

    #[test]
    fn test_parse_escaped_caret_then_end_anchor() {
        // row 17: "\^$" -> 0 backslashes immediately before '$' (the "\^"
        // escape precedes it) => anchor; core "\^" -> ESC_CARET.
        assert_eq!(
            parse_pattern("\\^$"),
            ParsedPattern {
                core: vec![ESC_CARET],
                start_anchored: false,
                end_anchored: true,
            }
        );
    }

    #[test]
    fn test_parse_start_anchor_escaped_end() {
        // row 18: "^\$" -> start '^' anchored; 1 backslash before '$' (odd)
        // => escaped; core "\$" -> ESC_DOLLAR.
        assert_eq!(
            parse_pattern("^\\$"),
            ParsedPattern {
                core: vec![ESC_DOLLAR],
                start_anchored: true,
                end_anchored: false,
            }
        );
    }

    #[test]
    fn test_parse_escaped_caret_glob_escaped_dollar_no_anchors() {
        // row 19: "\^*\$" -> 1 backslash before final '$' (odd) => escaped,
        // no anchors; core "\^*\$" -> [ESC_CARET, GLOB_STAR, ESC_DOLLAR].
        assert_eq!(
            parse_pattern("\\^*\\$"),
            ParsedPattern {
                core: vec![ESC_CARET, GLOB_STAR, ESC_DOLLAR],
                start_anchored: false,
                end_anchored: false,
            }
        );
    }

    #[test]
    fn test_parse_interior_dollar_then_end_anchor() {
        // row 20: "mid$dle$" -> 0 backslashes before the final '$' (preceded
        // by 'e') => anchor; the interior '$' is a bare 0x24 literal.
        assert_eq!(
            parse_pattern("mid$dle$"),
            ParsedPattern {
                core: vec![0x6D, 0x69, 0x64, 0x24, 0x64, 0x6C, 0x65],
                start_anchored: false,
                end_anchored: true,
            }
        );
    }

    // --- Anchor + escape/class interaction — rows 21-26 ---

    #[test]
    fn test_parse_anchored_digit_class() {
        // row 21: "^\d$" -> both anchored, core [CLASS_DIGIT].
        assert_eq!(
            parse_pattern("^\\d$"),
            ParsedPattern {
                core: vec![CLASS_DIGIT],
                start_anchored: true,
                end_anchored: true,
            }
        );
    }

    #[test]
    fn test_parse_anchored_word_class() {
        // row 22: "^\w$" -> both anchored, core [CLASS_WORD].
        assert_eq!(
            parse_pattern("^\\w$"),
            ParsedPattern {
                core: vec![CLASS_WORD],
                start_anchored: true,
                end_anchored: true,
            }
        );
    }

    #[test]
    fn test_parse_anchored_dot_meta() {
        // row 23: "^.$" -> both anchored, core [DOT_META].
        assert_eq!(
            parse_pattern("^.$"),
            ParsedPattern {
                core: vec![DOT_META],
                start_anchored: true,
                end_anchored: true,
            }
        );
    }

    #[test]
    fn test_parse_anchored_escaped_dot_literal() {
        // row 24: "^\.$" -> both anchored; escaped dot is LITERAL 0x2E
        // (GOTCHA-A from S1), NOT DOT_META.
        assert_eq!(
            parse_pattern("^\\.$"),
            ParsedPattern {
                core: vec![0x2E],
                start_anchored: true,
                end_anchored: true,
            }
        );
    }

    #[test]
    fn test_parse_anchored_plus_quantifier() {
        // row 25: "^a+$" -> both anchored; 'a' consumable so '+' is PLUS_QUANT.
        assert_eq!(
            parse_pattern("^a+$"),
            ParsedPattern {
                core: vec![0x61, PLUS_QUANT],
                start_anchored: true,
                end_anchored: true,
            }
        );
    }

    #[test]
    fn test_parse_leading_glob_no_anchors() {
        // row 26: "*^test" -> no anchors (neither at the very front/end as an
        // anchor); bare '^' passes through as 0x5E.
        assert_eq!(
            parse_pattern("*^test"),
            ParsedPattern {
                core: vec![GLOB_STAR, 0x5E, 0x74, 0x65, 0x73, 0x74],
                start_anchored: false,
                end_anchored: false,
            }
        );
    }

    // --- NUL-stop parity (GOTCHA-2) — row 27 ---

    #[test]
    fn test_parse_nul_truncates_before_anchor_check() {
        // row 27: "ab\0cd$" -> effective_len=2 (NUL at index 2). Trailing-$
        // check sees bytes[1]=='b', NOT '$' => no end anchor (anchor FLAG,
        // not just core, must match firmware strlen). Core = bytes[0..2] = "ab".
        let s = std::str::from_utf8(b"ab\0cd$").unwrap();
        assert_eq!(
            parse_pattern(s),
            ParsedPattern {
                core: vec![0x61, 0x62],
                start_anchored: false,
                end_anchored: false,
            }
        );
    }
}
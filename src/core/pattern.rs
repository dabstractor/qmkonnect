//! Pattern escape processing — byte-for-byte port of the firmware
//! `process_escapes()` (qmk_notifier `pattern_match.c:30-75`).
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

use serde::Deserialize;

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
        if bs.is_multiple_of(2) {
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

// =========================================================================
// Thompson NFA compiler (P2.M1.T2.S1)
//
// Stage 3 of the P2.M1 matcher pipeline:
//   parse_pattern -> process_escapes -> nfa_compile -> nfa_addstate/nfa_match
//                                                              -> match_with_anchors
// Compiles the processed-pattern byte stream (`parse_pattern().core` /
// `process_escapes()` output) into a `Vec<NfaOp>` the NFA simulator
// (P2.M1.T2.S2) runs against a candidate string. Faithful port of the firmware
// `nfa_compile` + `State` struct + `OP_*` opcodes (`qmk_notifier/pattern_match.c`
// ~lines 300-430). See architecture `external_deps.md` §3 point 3 and PRD §4.
// =========================================================================

/// A single compiled NFA node — index-based Thompson construction (no raw
/// pointers). Rust analog of the firmware `State` (`pattern_match.c`), minus
/// the `lastlist` field: the compiled `Vec<NfaOp>` is pure immutable program
/// data (compile once, simulate many times), and the simulator
/// ([nfa_match]/[nfa_addstate], P2.M1.T2.S2) owns its own generation-tag dedup
/// list parallel to `states`. The firmware coupled `lastlist` onto `State` only
/// because C has no cheap alternative.
///
/// Edges are `usize` indices into the `Vec<NfaOp>` returned by [nfa_compile].
/// The start state is ALWAYS index 0 (see [nfa_compile] + GOTCHA-1: the glob
/// allocates SPLIT-before-ANY so a leading `*` still starts at 0). Opcodes
/// mirror the firmware `OP_*` enum 1:1:
///
/// | variant  | firmware opcode | meaning                                           |
/// |----------|-----------------|---------------------------------------------------|
/// | `Char`   | `OP_CHAR`       | consume one byte matching `arg`; go to `out`       |
/// | `Any`    | `OP_ANY`        | consume ANY byte incl `\n`/`\r` (glob `*`); `out` |
/// | `Split`  | `OP_SPLIT`      | epsilon fork: follow BOTH `out` and `out1`         |
/// | `Assert` | `OP_ASSERT`     | zero-width `\b`(0x0B)/`\B`(0x0C); go to `out`      |
/// | `Match`  | `OP_MATCH`      | accepting state (terminal)                         |
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum NfaOp {
    /// Consume one input byte matching `arg`, then go to `out`. `arg` is a
    /// `process_escapes` placeholder byte (escaped literal 0x01-0x04, class
    /// 0x05-0x0A, dot 0x0D, literal `.`/`+`, or any ordinary ASCII byte).
    Char { arg: u8, out: usize },
    /// Consume ANY one byte INCLUDING `\n`/`\r` (the glob `*` compiled as
    /// `.*`). Distinct from `Char { arg: DOT_META }` (the dot excludes
    /// newline). `out` points BACK to the glob's `Split` (a backward edge —
    /// GOTCHA-2), so it cannot be made implicit; it is a stored field.
    Any { out: usize },
    /// Epsilon fork: the simulator follows BOTH `out` and `out1` without
    /// consuming input. Implements `*` (glob) and `+` (one-or-more). The
    /// simulator's `lastlist` guard prevents infinite recursion on convergence.
    Split { out: usize, out1: usize },
    /// Zero-width assertion: `arg` is `ASSERT_BOUND` (`\b`, want a boundary) or
    /// `ASSERT_NBOUND` (`\B`, want a non-boundary). The simulator recurses into
    /// `out` only if the word-boundary test matches `arg`.
    Assert { arg: u8, out: usize },
    /// Accepting state (terminal). A match is reported when a `Match` node is
    /// on the simulator's current state list.
    Match,
}

impl NfaOp {
    /// Write the next-state index into this state's PRIMARY `out` edge.
    /// This is valid on every edge-bearing variant including `Split` (its
    /// primary edge is patched in AFTER allocation — e.g. the glob's `Split`
    /// is allocated before its `Any`, so `Split.out` cannot be filled at push
    /// time and is set here). No-op on `Match` (terminal).
    fn set_out(&mut self, idx: usize) {
        match self {
            NfaOp::Char { out, .. } => *out = idx,
            NfaOp::Any { out } => *out = idx,
            NfaOp::Assert { out, .. } => *out = idx,
            NfaOp::Split { out, .. } => *out = idx,
            NfaOp::Match => {}
        }
    }

    /// Write the next-state index into a `Split`'s SECONDARY `out1` edge.
    fn set_out1(&mut self, idx: usize) {
        if let NfaOp::Split { out1, .. } = self {
            *out1 = idx;
        }
    }
}

/// The dangling exit slot the compiler threads — index-based analog of the
/// firmware `State **tail`. Describes WHERE the next unit's entry index must be
/// written when that unit is compiled. Threaded by value (no borrow issues).
enum DanglingExit {
    /// Write into the function's `start` slot (no unit compiled yet).
    Start,
    /// Write into `states[idx].out` (a Char/Any/Assert primary exit).
    Out(usize),
    /// Write into `states[idx].out1` (a Split secondary exit — the glob/`X+` exit).
    SplitSecond(usize),
}

/// `*tail = entry_idx` — resolve the dangling exit. The first call
/// (`tail == Start`) sets the function's `start` to the first unit's entry
/// (always index 0 by the glob SPLIT-first reorder). Mirrors the firmware
/// `*tail = entry` pointer write.
fn resolve(states: &mut [NfaOp], start: &mut Option<usize>, tail: &DanglingExit, entry_idx: usize) {
    match tail {
        DanglingExit::Start => *start = Some(entry_idx),
        DanglingExit::Out(idx) => states[*idx].set_out(entry_idx),
        DanglingExit::SplitSecond(idx) => states[*idx].set_out1(entry_idx),
    }
}

/// Compile a processed-pattern byte slice into an index-based Thompson NFA —
/// a faithful port of the firmware `nfa_compile` (`pattern_match.c` ~lines
/// 365-430). The input is the output of [process_escapes] / the `core` field of
/// [ParsedPattern] (e.g. `[0x61, PLUS_QUANT]` for `a+`); the output is the
/// compiled `Vec<NfaOp>` whose **start state is always index 0** (see below).
///
/// # Constructs compiled
///
/// | input bytes             | compiled unit                         |
/// |-------------------------|---------------------------------------|
/// | `GLOB_STAR` (`*`)       | `Split` + `Any` loop (== regex `.*`)  |
/// | `ASSERT_BOUND`/`NBOUND` | zero-width `Assert`                    |
/// | `X, PLUS_QUANT` (`X+`)  | `Char` + `Split` loop-back (2 states) |
/// | lone `PLUS_QUANT`       | skipped (stray; never emitted by S1)  |
/// | any other byte          | plain `Char`                           |
/// | (end)                   | `Match` (appended into the final exit) |
///
/// # The linear-time guarantee
///
/// `X+` compiles to **exactly 2 states** (`Char` + `Split` loop-back), so a
/// pattern like `a+a+a+…b` scales as `2k+1` states — **never** `2^k`. NFA
/// simulation is then `O(states × input_len)` with no backtracking. This is
/// the whole reason Thompson construction exists and the fix for the old
/// exponential backtracker (PRD §7.8). See Russ Cox, *“Regular Expression
/// Matching Can Be Simple And Fast”* (<https://swtch.com/~rsc/regexp/regexp1.html>,
/// cited by the firmware rustdoc at PRD §7.5/§7.9), in particular the
/// `a(n+1)b` pathological example.
///
/// # Start-index invariant
///
/// The start state is **always index 0**. The firmware allocates `any`-then-
/// `sp` for the glob, so a *leading* `*` would land its entry `Split` at index
/// one. This port REORDERS to `sp`-then-`any` (allocate `Split` first) so the
/// unit entry is the lowest index — keeping `start == 0` for EVERY pattern.
/// The edge graph is byte-for-byte identical to the firmware (same opcodes,
/// same `out`/`out1` targets modulo the swapped glob indices); only the two
/// glob states' index numbers differ. This is the single intentional
/// divergence from the C, and it preserves match semantics exactly (the
/// simulator follows edges, not indices). The downstream simulator (T2.S2)
/// seeds its epsilon-closure from `states[0]`.
///
/// # Pipeline
///
/// Stage 3 of: `parse_pattern` → `process_escapes` → **`nfa_compile`** →
/// `nfa_addstate`/`nfa_match` → `match_with_anchors`. The simulator + matcher
/// entry are P2.M1.T2.S2 / P2.M1.T3 and are NOT implemented here.
///
/// # Edge cases
///
/// - Empty input (`b""`) → `vec![NfaOp::Match]` (the loop never runs; the
///   end-`Match` is pushed unconditionally, GOTCHA-6).
/// - No `NFA_MAX_STATES` clamp (the firmware corrupts on overflow by design to
///   avoid AVR stack overflow; a Rust `Vec` grows and a host pattern never
///   approaches any cap — GOTCHA-7).
/// - No `lastlist`/generation state: the compiled program is immutable; the
///   simulator owns dedup separately (GOTCHA-5).
pub(crate) fn nfa_compile(pat: &[u8]) -> Vec<NfaOp> {
    let mut states: Vec<NfaOp> = Vec::with_capacity(pat.len() * 2 + 2);
    let mut start: Option<usize> = None; // start slot; resolved to 0 for the first unit
    let mut tail = DanglingExit::Start; // dangling exit slot (firmware `State **tail`)

    let mut i = 0;
    while i < pat.len() {
        let b = pat[i];
        // GOTCHA-8: dispatch in firmware branch order — check glob/assert/
        // stray-quant BEFORE the generic consuming `else` (else those bytes
        // would compile as plain Char).
        if b == GLOB_STAR {
            // (a) glob '*' == regex '.*'. GOTCHA-1: allocate SPLIT FIRST so the
            // unit entry is the lowest index (keeps start==0 for a leading
            // glob). Edge graph is identical to the firmware; only the two
            // states' index numbers swap.
            let sp_idx = states.len();
            states.push(NfaOp::Split { out: 0, out1: 0 }); // both edges filled below
            let any_idx = states.len();
            states.push(NfaOp::Any { out: 0 }); // filled below
                                                // sp.out = ANY ; any.out = sp (GOTCHA-2: BACKWARD loop-back edge)
            states[sp_idx].set_out(any_idx);
            states[any_idx].set_out(sp_idx);
            resolve(&mut states, &mut start, &tail, sp_idx); // *tail = sp (entry)
            tail = DanglingExit::SplitSecond(sp_idx); // exit via sp.out1
            i += 1;
        } else if b == ASSERT_BOUND || b == ASSERT_NBOUND {
            // (b) \b / \B : zero-width assertion.
            let a_idx = states.len();
            states.push(NfaOp::Assert { arg: b, out: 0 }); // out filled by next resolve
            resolve(&mut states, &mut start, &tail, a_idx);
            tail = DanglingExit::Out(a_idx); // exit via a.out
            i += 1;
        } else if b == PLUS_QUANT {
            // (c) stray 0x0E — process_escapes never emits it standalone; skip
            // defensively (firmware `continue`).
            i += 1;
        } else {
            // (d) consuming element X.
            let c_idx = states.len();
            states.push(NfaOp::Char { arg: b, out: 0 });
            if i + 1 < pat.len() && pat[i + 1] == PLUS_QUANT {
                // X+ : LINEAR (exactly 2 states — Char + Split loop-back).
                let sp_idx = states.len();
                states.push(NfaOp::Split {
                    out: c_idx,
                    out1: 0,
                }); // out1 filled by next resolve
                states[c_idx].set_out(sp_idx); // after one X -> split
                resolve(&mut states, &mut start, &tail, c_idx);
                tail = DanglingExit::SplitSecond(sp_idx); // exit via split.out1
                i += 2; // GOTCHA-3: consume X AND the 0x0E marker
            } else {
                resolve(&mut states, &mut start, &tail, c_idx);
                tail = DanglingExit::Out(c_idx); // exit via c.out
                i += 1;
            }
        }
    }

    // (e) End: append the single accepting state into the final dangling slot.
    let m_idx = states.len();
    states.push(NfaOp::Match);
    resolve(&mut states, &mut start, &tail, m_idx);
    debug_assert_eq!(start, Some(0)); // invariant: first unit's entry is always index 0

    states // start is implicitly states[0]; the simulator (T2.S2) seeds from here
}

// =========================================================================
// Thompson NFA simulator (P2.M1.T2.S2)
//
// Stage 4 of the P2.M1 matcher pipeline:
//   parse_pattern -> process_escapes -> nfa_compile -> nfa_addstate/nfa_match
//                                                              -> match_with_anchors
// Runs a compiled `Vec<NfaOp>` (S1 `nfa_compile` output) against a candidate
// string: the epsilon-closure (`nfa_addstate`) + the two-list Thompson
// simulation (`nfa_match`), plus the supporting character predicates. Faithful
// port of the firmware `pattern_match.c` ~lines 454-625 (is_digit_char /
// is_word_char / is_whitespace_char / is_word_boundary / nfa_addstate /
// pattern_char_matches / nfa_has_match / nfa_match) + `get_escaped_char`
// (~116-140, literal cases only). See architecture `external_deps.md` §3
// points 4-5 and PRD §4 for the contract sources.
// =========================================================================

// ---- Character classifiers (byte-identical to the firmware C ranges) ----
//
// GOTCHA-8: these use the `u8::is_ascii_*` methods, NOT the `char::` forms. The
// firmware uses C range checks / `tolower` which are ASCII-only in the C locale;
// `char::is_whitespace()` would add Unicode whitespace and diverge on
// non-ASCII (e.g. UTF-8 continuation) bytes.

#[inline]
fn is_digit_char(c: u8) -> bool {
    c.is_ascii_digit() // == c >= b'0' && c <= b'9'
}

#[inline]
fn is_word_char(c: u8) -> bool {
    // [a-zA-Z0-9_]. is_ascii_alphanumeric == [a-zA-Z0-9]; add '_'.
    c.is_ascii_alphanumeric() || c == b'_'
}

#[inline]
fn is_whitespace_char(c: u8) -> bool {
    // Firmware parity (`pattern_match.c::is_whitespace_char`): the whitespace
    // class is EXACTLY { ' ', '\t', '\n', '\r', '\f'(0x0C), '\v'(0x0B) }.
    // NOTE: `u8::is_ascii_whitespace` is NOT a correct substitute — it omits
    // '\v' (0x0B) and was the cause of a firmware-parity divergence caught by
    // the P2.M1.T4.S1 corpus (`\s` vs "\x0B" must match). Spell the set out.
    c == b' ' || c == b'\t' || c == b'\n' || c == b'\r' || c == 0x0C || c == 0x0B
}

/// Word-boundary test against the ORIGINAL string at an absolute byte position.
/// A boundary exists at `pos` iff exactly one of the neighboring bytes is a
/// word char. Off-string edges use an implicit non-word char.
///
/// Mirrors the firmware `is_word_boundary` (`pattern_match.c`:~470) exactly.
/// The empty-original-string special case for `\b`/`\B` is handled by
/// [`nfa_addstate`] BEFORE calling this (it checks `string_start.is_empty()`
/// first — GOTCHA-6); the guards here are defensive (`pos > len` ⇒ false, and
/// the `len > 0` clauses keep the edge positions sane on an empty string).
fn is_word_boundary(string: &[u8], pos: usize) -> bool {
    let len = string.len();
    if pos == 0 {
        return len > 0 && is_word_char(string[0]);
    }
    if pos == len {
        return len > 0 && is_word_char(string[len - 1]);
    }
    if pos > len {
        return false;
    }
    is_word_char(string[pos - 1]) != is_word_char(string[pos])
}

/// Decode an escaped-literal placeholder (`ESC_CARET`..=`ESC_BSLASH`, i.e.
/// 0x01-0x04) back to its literal byte. Mirrors firmware `get_escaped_char`
/// (`pattern_match.c`:~116-140) for the four literal cases only — the
/// class/assertion bytes (0x05-0x0D) are handled directly in
/// [`pattern_char_matches`]. Any other byte passes through unchanged.
fn decoded_literal(pc: u8) -> u8 {
    match pc {
        ESC_CARET => b'^',   // 0x01 -> \^
        ESC_DOLLAR => b'$',  // 0x02 -> \$
        ESC_STAR => b'*',    // 0x03 -> \*
        ESC_BSLASH => b'\\', // 0x04 -> \\
        _ => pc,             // not an escaped literal; ordinary byte
    }
}

/// Test whether a processed-pattern byte `pc` matches an input byte `sc`.
///
/// Faithful port of the firmware `pattern_char_matches` (`pattern_match.c`
/// ~lines 542-575). Dispatches on the S1 named placeholder consts (NOT raw
/// hex):
/// - `ESC_CARET`..=`ESC_BSLASH` (0x01-0x04): escaped literals are **DECODED
///   first** then ASCII-folded — never fold the placeholder byte itself
///   (GOTCHA-9).
/// - `CLASS_DIGIT`/`NDIGIT`/`WORD`/`NWORD`/`SPACE`/`NSPACE`: the six character
///   classes, via the ASCII classifiers above.
/// - `DOT_META` (0x0D): the dot — matches any byte EXCEPT `\n` AND `\r`
///   (GOTCHA-7). Distinct from the glob `*` (compiled as `Any`, which matches
///   any byte including newline).
/// - anything else: an ordinary literal, ASCII-folded for case-insensitive.
///
/// ASCII folding via [`u8::to_ascii_lowercase`] is exactly C
/// `tolower((unsigned char))` in the C locale — it folds only `A-Z` and is the
/// identity for all other bytes (including UTF-8 continuation bytes).
fn pattern_char_matches(pc: u8, sc: u8, case_sensitive: bool) -> bool {
    // Escaped literal: decode THEN compare (GOTCHA-9: never fold the placeholder).
    if (ESC_CARET..=ESC_BSLASH).contains(&pc) {
        // 0x01..=0x04
        let lit = decoded_literal(pc);
        return if case_sensitive {
            lit == sc
        } else {
            lit.eq_ignore_ascii_case(&sc)
        };
    }
    match pc {
        CLASS_DIGIT => is_digit_char(sc),        // \d
        CLASS_NDIGIT => !is_digit_char(sc),      // \D
        CLASS_WORD => is_word_char(sc),          // \w
        CLASS_NWORD => !is_word_char(sc),        // \W
        CLASS_SPACE => is_whitespace_char(sc),   // \s
        CLASS_NSPACE => !is_whitespace_char(sc), // \S
        DOT_META => sc != b'\n' && sc != b'\r',  // .  (dot excludes newline/CR)
        _ => {
            // ordinary literal
            if case_sensitive {
                pc == sc
            } else {
                pc.eq_ignore_ascii_case(&sc)
            }
        }
    }
}

/// Epsilon-closure add: follow `Split`/`Assert` edges (consuming no input),
/// collect `Char`/`Any`/`Match` states onto the live `list`.
///
/// Faithful port of the firmware `nfa_addstate` (`pattern_match.c`:~480-540).
/// Index-based: `idx` selects `states[idx]`; `list`/`seen` are owned by the
/// caller (the simulator [`nfa_match`]).
///
/// # Dedup (the reason `*` / `\b\b` terminate)
///
/// `seen[idx] == generation` means `idx` was already followed in THIS closure,
/// so it is skipped. The caller bumps `generation` once per phase (seed + each
/// consumed char) and NEVER clears `seen` — the stale tags simply become
/// invisible against the new generation (PRD §13 #11, the O(1)-dedup property).
/// The tag is set BEFORE dispatching (GOTCHA-4) so a state reached via one
/// `Split` branch is not re-added when the other converges on it.
///
/// # Epsilon edges keep `abspos` unchanged
///
/// `string_start` (the FULL original input — firmware `string_start`) and
/// `abspos` (an absolute offset into it) are forwarded UNCHANGED across
/// epsilon edges so `\b`/`\B` evaluate against the ORIGINAL string at an
/// absolute offset (PRD §13 #10; GOTCHA-2/3). Only the simulator's per-char
/// step advances `abspos` (by 1, when feeding a consumed `Char`/`Any`'s `out`).
///
/// # What gets collected
///
/// `Split` and `Assert` are NEVER collected (GOTCHA-5): `Split` follows both
/// edges; `Assert` conditionally follows `out` iff the word-boundary test
/// matches `arg`. Only `Char`/`Any`/`Match` are pushed onto `list`. The
/// empty-original-string special case (GOTCHA-6): if `string_start` is empty,
/// NEITHER `\b` NOR `\B` passes — do not recurse (independent of the
/// [`is_word_boundary`] implementation).
///
/// Recursive on epsilon edges; depth is bounded by the longest epsilon chain
/// (≪ `states.len()`), so no overflow risk for realistic host patterns.
fn nfa_addstate(
    states: &[NfaOp],
    idx: usize,
    list: &mut Vec<usize>,
    seen: &mut Vec<u32>,
    generation: u32,
    string_start: &[u8],
    abspos: usize,
) {
    // Dedup: skip if already followed in THIS closure (firmware `lastlist == nfa_gen`).
    if seen[idx] == generation {
        return;
    }
    // Mark seen BEFORE dispatching (firmware sets `lastlist = nfa_gen` first), so
    // a state reached via one Split branch isn't re-added when the other
    // converges (GOTCHA-4).
    seen[idx] = generation;

    match states[idx] {
        NfaOp::Match => {
            // Accepting state: collect it; nfa_has_match reports the match.
            list.push(idx);
        }
        NfaOp::Split { out, out1 } => {
            // Epsilon fork (glob '*', 'X+'): follow BOTH edges, abspos UNCHANGED
            // (GOTCHA-3).
            nfa_addstate(states, out, list, seen, generation, string_start, abspos);
            nfa_addstate(states, out1, list, seen, generation, string_start, abspos);
        }
        NfaOp::Assert { arg, out } => {
            // Zero-width \b (ASSERT_BOUND, want a boundary) / \B
            // (ASSERT_NBOUND, want a NON-boundary). Recurse into `out` ONLY if
            // the boundary condition holds.
            //
            // EMPTY-STRING SPECIAL CASE (GOTCHA-6): if the original string is
            // empty, NEITHER a boundary nor a non-boundary passes — do NOT
            // recurse. (Firmware checks `*string_start != '\0'` BEFORE
            // is_word_boundary; this short-circuit is independent of the
            // is_word_boundary implementation.)
            let want_boundary = arg == ASSERT_BOUND;
            if !string_start.is_empty() && is_word_boundary(string_start, abspos) == want_boundary {
                nfa_addstate(states, out, list, seen, generation, string_start, abspos);
            }
            // Never collect an Assert itself (firmware `return` with no list add).
        }
        NfaOp::Char { .. } | NfaOp::Any { .. } => {
            // Consuming state: it is "live", waiting for the next input char.
            list.push(idx);
        }
    }
}

/// True iff an accepting [`NfaOp::Match`] state is on the current live `list`.
/// Faithful port of the firmware `nfa_has_match` (`pattern_match.c`:~575-577).
fn nfa_has_match(states: &[NfaOp], list: &[usize]) -> bool {
    list.iter().any(|&idx| states[idx] == NfaOp::Match)
}

/// Two-list Thompson NFA simulation — run a compiled NFA (S1 [`nfa_compile`]
/// output; start state is always `states[0]`) against `string` beginning at
/// byte `start`.
///
/// # Parameters
///
/// - `states`: the compiled NFA (S1 [`nfa_compile`] output). The start state
///   is always index 0 (S1's start-`==`-0 invariant).
/// - `string`: the **FULL ORIGINAL** input (firmware `string_start`).
///   `\b`/`\B` evaluate against this at an absolute offset (PRD §13 #10), so a
///   substring/prefix search that starts mid-string must still pass the whole
///   string here.
/// - `start`: the byte offset into `string` to begin consuming (firmware
///   `str - string_start`). For substring/suffix matching, `match_with_anchors`
///   (P2.M1.T3.S1) loops offsets `i` and calls this at each `start = i`.
/// - `full_match`: `false` ⇒ a `Match` reachable at ANY point (prefix /
///   substring) returns `true`; `true` ⇒ `Match` must be reachable only after
///   consuming the WHOLE remaining string (exact / suffix).
///
/// # The linear-time guarantee
///
/// This is the canonical two-list Thompson simulation (`clist` = current live
/// states, `nlist` = next live states, swapped per step) with generation-tag
/// O(1) dedup (`seen[idx] == generation` ⇒ already followed this phase;
/// `generation` is bumped once per phase and never cleared). Each input byte is
/// consumed in a single pass over `clist`, each state is followed at most once
/// per phase, and there is no backtracking — so the whole match is
/// **O(states × consumed_len)** with **no allocation in the hot loop** (the two
/// lists are pre-sized). This is the fix for the old exponential matcher
/// (PRD §7.8); `a+a+a+…` (which compiles to `2k+1` states, S1) simulates in
/// linear time. See Russ Cox, *“Regular Expression Matching Can Be Simple And
/// Fast”* (<https://swtch.com/~rsc/regexp/regexp1.html>, cited by the firmware
/// rustdoc at PRD §7.5/§7.9), in particular §“NFA-based Regular Expression
/// Algorithms”.
///
/// # Pipeline
///
/// Stage 4 of: `parse_pattern` → `process_escapes` → `nfa_compile` →
/// `nfa_addstate`/`nfa_match` → `match_with_anchors`. The anchor strategy
/// (`match_with_anchors`, which compiles once and calls this at each offset) is
/// P2.M1.T3.S1 — a later subtask.
pub(crate) fn nfa_match(
    states: &[NfaOp],
    string: &[u8],
    start: usize,
    case_sensitive: bool,
    full_match: bool,
) -> bool {
    debug_assert!(!states.is_empty(), "nfa_compile always yields >= [Match]");
    // The firmware defensive guard `if (!start) return full_match ? (*str=='\0')
    // : true;` is dead in Rust: nfa_compile (S1) never returns an empty Vec, so
    // states[0] always exists (GOTCHA-1). Do NOT add a guard returning spurious
    // true.

    let mut clist: Vec<usize> = Vec::with_capacity(states.len());
    let mut nlist: Vec<usize> = Vec::with_capacity(states.len());
    let mut seen: Vec<u32> = vec![0u32; states.len()]; // generation-tag dedup (0 = unseen)
    let mut generation: u32 = 0; // first `+= 1` makes it 1; 0 means unseen (GOTCHA-10)

    let mut pos = start; // abspos: absolute offset into `string` (for \b/\B)

    // Seed the closure from states[0] (the start — always index 0, S1 invariant).
    generation += 1; // fresh phase (GOTCHA-11: bump once, NEVER clear seen)
    nfa_addstate(states, 0, &mut clist, &mut seen, generation, string, pos);
    if !full_match && nfa_has_match(states, &clist) {
        return true; // empty prefix matched (substring/prefix semantics)
    }

    // Consume one input byte per step.
    while pos < string.len() {
        let c = string[pos];
        generation += 1; // fresh phase (firmware `nfa_gen++; nn = 0;`) — O(1) dedup, no clear
        nlist.clear();
        for &s in &clist {
            match states[s] {
                NfaOp::Any { out } => {
                    // glob '*': ANY byte incl '\n'/'\r' (PRD §13 #8). Unconditional add.
                    nfa_addstate(
                        states,
                        out,
                        &mut nlist,
                        &mut seen,
                        generation,
                        string,
                        pos + 1,
                    );
                }
                NfaOp::Char { arg, out } => {
                    if pattern_char_matches(arg, c, case_sensitive) {
                        nfa_addstate(
                            states,
                            out,
                            &mut nlist,
                            &mut seen,
                            generation,
                            string,
                            pos + 1,
                        );
                    }
                }
                // Match/Assert/Split are never on the LIVE list (nfa_addstate
                // resolves them via epsilon edges); skip silently (GOTCHA-5).
                _ => {}
            }
        }
        std::mem::swap(&mut clist, &mut nlist); // swap lists (firmware pointer swap; GOTCHA-12)
        pos += 1;
        if clist.is_empty() {
            break; // dead — no live states can recover (GOTCHA-14)
        }
        if !full_match && nfa_has_match(states, &clist) {
            return true; // prefix matched mid-stream (GOTCHA-13: NOT for full_match)
        }
    }

    nfa_has_match(states, &clist) // full_match: accept only at end; substring: already returned
}

// =========================================================================
// Anchor strategy + public entry (P2.M1.T3.S1)
//
// Stage 5 of the P2.M1 matcher pipeline:
//   parse_pattern -> process_escapes -> nfa_compile -> nfa_addstate/nfa_match
//                                                            -> match_with_anchors
//                                                                -> pattern_match
// Picks the NFA mode (`full_match`) + offset strategy from the parsed anchor
// flags and exposes the public [`pattern_match`] entry. Faithful port of the
// firmware `match_with_anchors` + `pattern_match` (`pattern_match.c` ~lines
// 233-272). See architecture `external_deps.md` §3 point 6 and PRD §4 for the
// contract sources.
// =========================================================================

/// The offset loop shared by the suffix (`$`) and substring (no-anchor) modes.
/// Probes every UTF-8 char boundary PLUS the terminal byte-length offset, so
/// the firmware's inclusive `0..=str_len` range is reproduced.
///
/// (REFINEMENT F per the item spec: `char_indices` keeps the matcher UTF-8-
/// correct — for the entire realistic ASCII domain it is byte-identical to the
/// firmware. `chain(once(bytes.len()))` is MANDATORY: `char_indices` alone stops
/// at the last char START, not one-past, and would miss suffix / tail-empty
/// cases such as pure `*` reaching `Match` at the very end — GOTCHA-B.)
fn suffix_or_substring_loop(
    nfa: &[NfaOp],
    bytes: &[u8],
    s: &str,
    case_sensitive: bool,
    full_match: bool,
) -> bool {
    for i in s
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(bytes.len()))
    {
        if nfa_match(nfa, bytes, i, case_sensitive, full_match) {
            return true;
        }
    }
    false
}

/// Pick the NFA mode (`full_match`) and offset strategy from the parsed anchor
/// flags, then run the compiled core against `s`. Faithful port of the firmware
/// `match_with_anchors` (`pattern_match.c:233-256`).
///
/// # The four anchor modes → `nfa_match` mapping
///
/// The two `start_anchored` / `end_anchored` flags (set by [`parse_pattern`],
/// T1.S2) select one of four strategies. Each maps to a single [`nfa_match`]
/// call shape — the firmware's two C wrapper forwarders
/// (`match_string_with_start` / `match_reaches_end_with_start`) collapse into
/// direct `nfa_match(.., full_match)` calls (REFINEMENT D):
///
/// | flags                          | mode        | `nfa_match` call                              | `full_match` |
/// |--------------------------------|-------------|-----------------------------------------------|--------------|
/// | start && end (`^…$`)           | exact       | one call from offset 0                        | **true**     |
/// | start only (`^…`)              | prefix      | one call from offset 0                        | **false**    |
/// | end only (`…$`)                | suffix      | loop offsets, one call from each              | **true**     |
/// | neither (`…`)                  | substring   | empty-core guard, then loop offsets           | **false**    |
///
/// `full_match` semantics (T2.S2): `false` ⇒ `Match` reachable at ANY point
/// (prefix/substring) returns true; `true` ⇒ `Match` reachable only after
/// consuming the WHOLE remaining string (exact/suffix).
///
/// # Refinements over the firmware (each forced by idiomatic Rust)
///
/// - **D — fold the wrappers**: the firmware needs `match_string_with_start`
///   / `match_reaches_end_with_start` only because its `nfa_match` is `static`
///   and takes a raw pattern; in Rust `nfa_match` takes the compiled `&[NfaOp]`
///   + a `full_match` bool, so the wrappers are zero-information aliases.
/// - **E — compile ONCE**: the firmware `nfa_match` recompiles the pattern
///   internally (stack-local pool) on EVERY call; the suffix/substring loops
///   therefore recompile `len+1` times. Rust's `nfa_compile` heap-allocates,
///   so this function calls it ONCE per invocation and reuses `&nfa` across
///   the loop (semantics identical — `nfa_compile` is pure).
/// - **F — char boundaries**: the firmware loops raw byte offsets; this port
///   iterates `s.char_indices()` (UTF-8-correct; byte-identical for ASCII) and
///   appends the terminal `bytes.len()` to preserve the inclusive end.
///
/// # Empty-core substring special case (GOTCHA-A — the one parity trap)
///
/// With no anchors and an empty core, the default substring loop would match
/// at offset 0 of ANY string (an empty NFA `[Match]` reaches `Match`
/// immediately with `full_match=false`), turning an empty unanchored rule into
/// "match everything". So the substring branch short-circuits
/// `parsed.core.is_empty() ⇒ s.is_empty()`. The OTHER three modes need NO
/// special case (traced in `research/notes.md` §4): the exact/prefix/suffix
/// empty-core behaviors fall out of `nfa_match` naturally.
///
/// # Dead-in-Rust firmware code dropped
///
/// The firmware `if (!parsed || !str) return false` (GOTCHA-D) is dropped —
/// `&str` / `&ParsedPattern` are never null. No `free`/Drop analog is needed
/// (the `ParsedPattern` owns its `Vec`).
pub(crate) fn match_with_anchors(parsed: &ParsedPattern, s: &str, case_sensitive: bool) -> bool {
    let bytes = s.as_bytes();

    if parsed.start_anchored && parsed.end_anchored {
        // ^...$ exact: one FULL match (consume whole string) from offset 0.
        let nfa = nfa_compile(&parsed.core); // GOTCHA-C/REFINEMENT E: compile ONCE
        nfa_match(&nfa, bytes, 0, case_sensitive, true) // GOTCHA-F: full_match=true
    } else if parsed.start_anchored {
        // ^ prefix: one reach-any match from offset 0.
        let nfa = nfa_compile(&parsed.core);
        nfa_match(&nfa, bytes, 0, case_sensitive, false) // GOTCHA-F: full_match=false
    } else if parsed.end_anchored {
        // $ suffix: loop offsets, FULL match from each.
        let nfa = nfa_compile(&parsed.core);
        suffix_or_substring_loop(&nfa, bytes, s, case_sensitive, true)
    } else {
        // substring (default): empty core -> only the empty string; else loop
        // offsets, reach-any from each.
        if parsed.core.is_empty() {
            return s.is_empty(); // GOTCHA-A: empty pattern (no anchors) matches only ""
        }
        let nfa = nfa_compile(&parsed.core);
        suffix_or_substring_loop(&nfa, bytes, s, case_sensitive, false)
    }
}

/// Public pattern-matching entry — the full-parity port of the firmware
/// `pattern_match` (`pattern_match.c:259-272`), the single source of truth for
/// match semantics (PRD §4, §14). It is `parse → match_with_anchors → drop`:
/// the [`ParsedPattern`] owns its `Vec`, so there is no `free` analog and the
/// firmware NULL guard is dead (a Rust `&str` is never null).
///
/// # Supported constructs
///
/// This is a full-parity port of the firmware matcher, NOT a subset. The
/// pattern language (see PRD §4 "Pattern-Matching Syntax"):
///
/// | construct            | meaning                                                  |
/// |----------------------|----------------------------------------------------------|
/// | `*`                  | glob wildcard — any sequence incl empty; == regex `.*`   |
/// | `^`                  | start anchor (match must begin at offset 0)              |
/// | `$`                  | end anchor (match must reach the end of the string)      |
/// | `^…$`                | exact full-string match                                  |
/// | `\^` `\$` `\*` `\\` | literal `^` `$` `*` `\`                                  |
/// | `\d` `\D`            | digit / non-digit                                        |
/// | `\w` `\W`            | word char / non-word char (`[A-Za-z0-9_]`)               |
/// | `\s` `\S`            | whitespace / non-whitespace                              |
/// | `\b` `\B`            | word-boundary / non-boundary (zero-width)                |
/// | `.`                  | any char EXCEPT `\n` and `\r`                            |
/// | `+`                  | one-or-more quantifier (linear-time; no backtracking)    |
///
/// # Anchor / substring semantics
///
/// - **No anchors** ⇒ substring match (backward-compatible): the pattern may
///   appear anywhere in `s`. An empty unanchored pattern matches ONLY the
///   empty string — NOT everything (the empty-core special case).
/// - **`^`** ⇒ prefix; **`$`** ⇒ suffix; **`^…$`** ⇒ exact.
/// - **Case sensitivity** is per the `case_sensitive` arg (the rules layer
///   defaults it to `false`); matching ASCII-folds `A-Z` only.
///
/// # Pipeline
///
/// Stage 5 / the public entry of: `parse_pattern` → `process_escapes` →
/// `nfa_compile` → `nfa_addstate`/`nfa_match` → `match_with_anchors` →
/// **`pattern_match`**. The anchor strategy ([`match_with_anchors`]) compiles
/// the core once and drives [`nfa_match`] at the right offset(s). See Russ Cox,
/// *“Regular Expression Matching Can Be Simple And Fast”*
/// (<https://swtch.com/~rsc/regexp/regexp1.html>, cited by the firmware at
/// PRD §7.5/§7.9) for the linear-time Thompson NFA this ports.
///
/// # Consumers
///
/// - **P2.M1.T3.S2** (delimiter-aware `match_pattern`): splits a pattern/message
///   on the GS delimiter (`0x1D`) and calls this on each half.
/// - **P3.M1 `rules.rs`**: evaluates `rules.toml` layer/callback rules via
///   `pattern_match(rule.pattern, window_class_or_title, rule.case_sensitive)`.
pub fn pattern_match(pattern: &str, s: &str, case_sensitive: bool) -> bool {
    let parsed = parse_pattern(pattern); // T1.S2 (GOTCHA-J: reuse, do NOT reimplement)
    match_with_anchors(&parsed, s, case_sensitive)
    // `parsed` drops here automatically — NO free_parsed_pattern analog (GOTCHA-D).
}

// ============================================================================
// P2.M1.T3.S2 — delimiter-aware `match_pattern` + `Pattern` enum
// (full-parity port of firmware `notifier.c::match_pattern`, lines 425–530)
// ============================================================================

/// A host-side rule pattern — the typed form of the `match` field in
/// `rules.toml`'s `[layer_rules]` / `[callback_rules]`.
///
/// The firmware `match_pattern` receives its pattern as a raw C string that may
/// embed a Group Separator byte (`GS`, `0x1D`, ASCII 29) — the
/// `WT(class, title)` / `WINDOW_TITLE` macro (`notifier.h:36-39`) expands
/// `WT("Firefox", "*youtube*")` to the C literal `"Firefox\x1D*youtube*"`.
/// The matcher must then scan the pattern for the GS at runtime to decide
/// whether to treat it as class-only or class+title.
///
/// On the Rust host this is **structural, not textual**: `serde(untagged)`
/// resolves the variant at *deserialization* time, so the enum variant IS the
/// answer to "does the pattern have a GS?":
///
/// | TOML form                              | `Pattern` variant          | Meaning                       |
/// | -------------------------------------- | -------------------------- | ----------------------------- |
/// | `match = "alacritty"`                  | [`Pattern::Single(String)`] | class only (no GS)            |
/// | `match = ["*chrome*", "*youtube*"]`    | [`Pattern::Parts(String,String)`] | class + title (has GS, == `WT`) |
///
/// `WT("Firefox", "*youtube*")` thus corresponds to `match = ["Firefox", "*youtube*"]`.
///
/// serde `untagged` tries variants in declaration order: a scalar string →
/// `Single`; a 2-element array → `Parts`; a 1/3-element array, an integer, or a
/// table matches neither and **errors** (desired strictness for
/// `--validate-rules`). No custom visitor is needed.
///
/// See `spec/HOST_RULES.md` §8(2) + §9 and `notifier.h` for the contract.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Pattern {
    /// Class-only pattern. Deserialized from a bare TOML string:
    /// `match = "Firefox"`. Matches `app_class` only — firmware parity for a
    /// delimiter-less pattern (the window title is never consulted).
    Single(String),
    /// Class + title pattern. Deserialized from a 2-element TOML array:
    /// `match = ["*chrome*", "*youtube*"]` (== firmware `WT(class, title)`).
    /// **Both** halves must match.
    Parts(String, String),
}

/// Delimiter-aware top-level rule matcher — a full-parity port of firmware
/// `notifier.c::match_pattern` (lines 425–530), the GS-delimiter-aware wrapper
/// around the leaf [`pattern_match`] NFA matcher.
///
/// # Firmware-parity mapping
///
/// The firmware function is a 2×2 dispatch on (pattern has GS?) × (message has
/// GS?). Because the qmkonnect host **always** joins `app_class` + GS + `title`
/// when emitting a window notification (`src/core/notifier.rs:309`), the
/// message column is effectively constant = "has GS" for real traffic, so the
/// 2×2 matrix collapses onto the two `Pattern` variants:
///
/// | firmware case | pattern GS? | msg GS? | `Pattern`     | Rust action                                              |
/// | ------------- | ----------- | ------- | ------------- | -------------------------------------------------------- |
/// | A1 / A2       | no          | any     | [`Single`]    | `pattern_match(p, app_class, cs)` (title NOT consulted)  |
/// | B1 / B2       | yes         | any     | [`Parts`]     | `pattern_match(c, app_class, cs) && pattern_match(t, title, cs)` |
///
/// The firmware `B1` branch ("pattern has GS, message has no GS → match only
/// the left half") is **withdrawn** by the item spec: on the host we always know
/// both halves, so [`Parts`] always evaluates `t` against `title`.
///
/// # Design note (REFINEMENT G)
///
/// The firmware's `find_first_delimiter` / `split_by_delimiter` helpers and the
/// 256-byte stack-buffer overflow guards are not ported: on the host the GS
/// split is already resolved structurally (the `Pattern` variant encodes
/// "does the pattern have a GS?" and `app_class` / `title` are the message's two
/// halves). The whole if/else cascade reduces to the `match pattern` below.
///
/// # Examples
///
/// ```
/// use qmkonnect::core::pattern::{Pattern, match_pattern};
/// // class-only rule (TOML `match = "alacritty"`):
/// assert!(match_pattern(&Pattern::Single("alacritty".into()), "Alacritty", "vim", false));
/// // class+title rule (TOML `match = ["*chrome*","*youtube*"]`, == WT):
/// assert!(match_pattern(&Pattern::Parts("*chrome*".into(), "*youtube*".into()),
///                       "Google Chrome", "cat - YouTube", false));
/// ```
///
/// See firmware `notifier.c:425-530` + PRD §4.1 + §14 for the contract.
pub fn match_pattern(
    pattern: &Pattern,
    app_class: &str,
    title: &str,
    case_sensitive: bool,
) -> bool {
    match pattern {
        // Firmware cases A1 + A2: pattern has no GS. The message's left half
        // (app_class) is matched; `title` is deliberately NOT consulted — a
        // class-only rule never matches on the window title (firmware parity).
        // (When `title` is empty the message is class-only, so "whole message"
        // and "msg_left" both reduce to `app_class`.)
        Pattern::Single(p) => pattern_match(p, app_class, case_sensitive),

        // Firmware case B2: pattern has GS, message has GS (always, on the
        // host) → split both, BOTH halves must match. The item spec mandates
        // "both halves must match" (it does NOT reproduce firmware B1's
        // "message has no GS → match only the left half" branch, because the
        // host always knows both halves).
        Pattern::Parts(c, t) => {
            pattern_match(c, app_class, case_sensitive) && pattern_match(t, title, case_sensitive)
        }
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

    // ========================================================================
    // nfa_compile — Thompson NFA compiler (P2.M1.T2.S1)
    //
    // Parity table: research/notes.md §6 (18 inputs). Inputs are
    // processed-pattern byte slices (parse_pattern().core / process_escapes()
    // output), built from the S1 named consts. These assert the full Vec<NfaOp>
    // directly — STRICTLY STRONGER than the firmware's end-to-end-only tests
    // (the C nfa_compile is `static` and only reachable via pattern_match).
    // ========================================================================

    // --- Simple chains (empty, a, abc) — rows 1-3 ---

    #[test]
    fn test_compile_empty_is_single_match() {
        // row 1: empty input -> [Match] (GOTCHA-6). The loop never runs; the
        // end-Match is pushed unconditionally into the Start slot.
        assert_eq!(nfa_compile(b""), vec![NfaOp::Match]);
    }

    #[test]
    fn test_compile_single_char() {
        // row 2: [0x61] ("a") -> [Char{61,out:1}, Match]
        assert_eq!(
            nfa_compile(&[0x61]),
            vec![NfaOp::Char { arg: 0x61, out: 1 }, NfaOp::Match]
        );
    }

    #[test]
    fn test_compile_plain_chain_abc() {
        // row 3: [61,62,63] ("abc") -> three Chars + Match, each out = next idx.
        assert_eq!(
            nfa_compile(&[0x61, 0x62, 0x63]),
            vec![
                NfaOp::Char { arg: 0x61, out: 1 },
                NfaOp::Char { arg: 0x62, out: 2 },
                NfaOp::Char { arg: 0x63, out: 3 },
                NfaOp::Match,
            ]
        );
    }

    // --- Glob * (SPLIT+ANY loop; start==0 reorder) — rows 4-7 ---

    #[test]
    fn test_compile_glob_split_first_start_zero() {
        // row 4: [GLOB_STAR] ("*") -> [Split{out:1,out1:2}, Any{out:0}, Match].
        // GOTCHA-1: Split is at index 0 (the SPLIT-first reorder keeps
        // start==0 for a leading glob; firmware would put Any@0/Split@1).
        // GOTCHA-2: Any.out points BACK to the Split (index 0), not index+1.
        assert_eq!(
            nfa_compile(&[GLOB_STAR]),
            vec![
                NfaOp::Split { out: 1, out1: 2 },
                NfaOp::Any { out: 0 },
                NfaOp::Match,
            ]
        );
    }

    #[test]
    fn test_compile_char_then_glob() {
        // row 5: [61, GLOB_STAR] ("a*") -> Char then the glob unit.
        assert_eq!(
            nfa_compile(&[0x61, GLOB_STAR]),
            vec![
                NfaOp::Char { arg: 0x61, out: 1 },
                NfaOp::Split { out: 2, out1: 3 },
                NfaOp::Any { out: 1 },
                NfaOp::Match,
            ]
        );
    }

    #[test]
    fn test_compile_glob_then_char() {
        // row 6: [GLOB_STAR, 0x61] ("*a") -> glob unit then Char. Split still
        // at index 0 (GOTCHA-1).
        assert_eq!(
            nfa_compile(&[GLOB_STAR, 0x61]),
            vec![
                NfaOp::Split { out: 1, out1: 2 },
                NfaOp::Any { out: 0 },
                NfaOp::Char { arg: 0x61, out: 3 },
                NfaOp::Match,
            ]
        );
    }

    #[test]
    fn test_compile_dot_then_glob() {
        // row 7: [DOT_META, GLOB_STAR] (".*") -> dot Char then glob unit.
        assert_eq!(
            nfa_compile(&[DOT_META, GLOB_STAR]),
            vec![
                NfaOp::Char {
                    arg: DOT_META,
                    out: 1
                },
                NfaOp::Split { out: 2, out1: 3 },
                NfaOp::Any { out: 1 },
                NfaOp::Match,
            ]
        );
    }

    // --- X+ quantifier (CHAR+SPLIT loop; exactly 2 states) — rows 8-10 ---

    #[test]
    fn test_compile_plus_two_states() {
        // row 8: [0x61, PLUS_QUANT] ("a+") -> [Char{61,out:1}, Split{out:0,out1:2}, Match].
        // GOTCHA-3: i+=2 consumes BOTH bytes; the 0x0E is NOT re-emitted as Char.
        assert_eq!(
            nfa_compile(&[0x61, PLUS_QUANT]),
            vec![
                NfaOp::Char { arg: 0x61, out: 1 },
                NfaOp::Split { out: 0, out1: 2 },
                NfaOp::Match,
            ]
        );
    }

    #[test]
    fn test_compile_digit_class_plus() {
        // row 9: [CLASS_DIGIT, PLUS_QUANT] ("\d+") -> Char(class)+Split loop.
        assert_eq!(
            nfa_compile(&[CLASS_DIGIT, PLUS_QUANT]),
            vec![
                NfaOp::Char {
                    arg: CLASS_DIGIT,
                    out: 1
                },
                NfaOp::Split { out: 0, out1: 2 },
                NfaOp::Match,
            ]
        );
    }

    #[test]
    fn test_compile_dot_plus() {
        // row 10: [DOT_META, PLUS_QUANT] (".+") -> Char(dot)+Split loop.
        assert_eq!(
            nfa_compile(&[DOT_META, PLUS_QUANT]),
            vec![
                NfaOp::Char {
                    arg: DOT_META,
                    out: 1
                },
                NfaOp::Split { out: 0, out1: 2 },
                NfaOp::Match,
            ]
        );
    }

    // --- a+a+a+ : the exponential-backtracker killer (2k+1 states) — row 11 ---

    #[test]
    fn test_compile_a_plus_a_plus_a_plus_linear() {
        // row 11: [61,0E,61,0E,61,0E] ("a+a+a+") -> exactly 7 states (2*3+1).
        // THIS is the linear-time guarantee: each `a+` is 2 states, so the NFA
        // scales as 2k+1 — NEVER 2^k. This is the pathological input that made
        // the old backtracker exponential (PRD §7.8); Thompson construction
        // compiles it in linear space and simulates in O(states × strlen).
        let got = nfa_compile(&[0x61, PLUS_QUANT, 0x61, PLUS_QUANT, 0x61, PLUS_QUANT]);
        assert_eq!(
            got.len(),
            7,
            "a+a+a+ must compile to exactly 7 states (2k+1)"
        );
        assert_eq!(
            got,
            vec![
                NfaOp::Char { arg: 0x61, out: 1 },
                NfaOp::Split { out: 0, out1: 2 },
                NfaOp::Char { arg: 0x61, out: 3 },
                NfaOp::Split { out: 2, out1: 4 },
                NfaOp::Char { arg: 0x61, out: 5 },
                NfaOp::Split { out: 4, out1: 6 },
                NfaOp::Match,
            ]
        );
    }

    // --- Boundary assertions \b \B (zero-width) — rows 12-14 ---

    #[test]
    fn test_compile_assert_bound() {
        // row 12: [ASSERT_BOUND] ("\b") -> [Assert{0x0B,out:1}, Match]
        assert_eq!(
            nfa_compile(&[ASSERT_BOUND]),
            vec![
                NfaOp::Assert {
                    arg: ASSERT_BOUND,
                    out: 1
                },
                NfaOp::Match,
            ]
        );
    }

    #[test]
    fn test_compile_assert_nbound() {
        // row 13: [ASSERT_NBOUND] ("\B") -> [Assert{0x0C,out:1}, Match]
        assert_eq!(
            nfa_compile(&[ASSERT_NBOUND]),
            vec![
                NfaOp::Assert {
                    arg: ASSERT_NBOUND,
                    out: 1
                },
                NfaOp::Match,
            ]
        );
    }

    #[test]
    fn test_compile_assert_bound_then_word() {
        // row 14: [0x0B, 0x77, 0x6F, 0x72, 0x64] ("\bword") -> Assert + 4 Chars.
        assert_eq!(
            nfa_compile(&[ASSERT_BOUND, 0x77, 0x6F, 0x72, 0x64]),
            vec![
                NfaOp::Assert {
                    arg: ASSERT_BOUND,
                    out: 1
                },
                NfaOp::Char { arg: 0x77, out: 2 },
                NfaOp::Char { arg: 0x6F, out: 3 },
                NfaOp::Char { arg: 0x72, out: 4 },
                NfaOp::Char { arg: 0x64, out: 5 },
                NfaOp::Match,
            ]
        );
    }

    // --- Mixed / stray-0x0E defensive — rows 15-16 ---

    #[test]
    fn test_compile_char_then_plus_unit() {
        // row 15: [0x61, 0x62, PLUS_QUANT] ("ab+") -> plain a then the b+ unit.
        assert_eq!(
            nfa_compile(&[0x61, 0x62, PLUS_QUANT]),
            vec![
                NfaOp::Char { arg: 0x61, out: 1 },
                NfaOp::Char { arg: 0x62, out: 2 },
                NfaOp::Split { out: 1, out1: 3 },
                NfaOp::Match,
            ]
        );
    }

    #[test]
    fn test_compile_stray_quant_skipped() {
        // row 16: [PLUS_QUANT] (stray 0x0E) -> branch (c) skips it; only the
        // end-MATCH remains. process_escapes never emits 0x0E standalone, but
        // the firmware keeps the defensive skip; mirror it.
        assert_eq!(nfa_compile(&[PLUS_QUANT]), vec![NfaOp::Match]);
    }

    // --- Literal dot/plus as Char — rows 17-18 ---

    #[test]
    fn test_compile_dot_meta_as_char() {
        // row 17: [0x61, DOT_META] ("a.") -> dot meta compiles as a Char{arg:0x0D}.
        assert_eq!(
            nfa_compile(&[0x61, DOT_META]),
            vec![
                NfaOp::Char { arg: 0x61, out: 1 },
                NfaOp::Char {
                    arg: DOT_META,
                    out: 2
                },
                NfaOp::Match,
            ]
        );
    }

    #[test]
    fn test_compile_literal_dot_as_char() {
        // row 18: [0x61, 0x2E] ("a\.") -> the literal dot (0x2E, an ordinary
        // byte emitted by process_escapes for `\.`) compiles as a plain Char.
        assert_eq!(
            nfa_compile(&[0x61, 0x2E]),
            vec![
                NfaOp::Char { arg: 0x61, out: 1 },
                NfaOp::Char { arg: 0x2E, out: 2 },
                NfaOp::Match,
            ]
        );
    }

    // ========================================================================
    // nfa_match — Thompson NFA simulation (P2.M1.T2.S2)
    //
    // Parity table: research/notes.md §4 (~30 vectors). Patterns are compiled
    // via `nfa_compile(&process_escapes(pat))` (realistic integration of S1 +
    // S2), then `nfa_match` is called at a specific (start, full_match) offset.
    // These are STRICTLY STRONGER than the firmware's end-to-end-only tests:
    // they exercise the simulator in isolation, pinning simulator bugs
    // independent of anchor-strategy bugs (the full end-to-end corpus is ported
    // in P2.M1.T4.S1). The linchpin is `\bword`/`aword`@1 → false (GOTCHA-2 /
    // REFINEMENT A): it proves `\b` sees the ORIGINAL string at an absolute
    // offset, not a per-offset slice.
    // ========================================================================

    // --- Exact full-string (full_match=true, start=0) — §4.1 (7 rows) ---

    #[test]
    fn test_match_exact_whole_string() {
        // "test" vs b"test" full -> consume whole string, reach Match.
        let nfa = nfa_compile(&process_escapes("test"));
        assert!(nfa_match(&nfa, b"test", 0, true, true));
    }

    #[test]
    fn test_match_exact_rejects_trailing_input() {
        // "test" vs b"testing" full -> Match reached at pos 3, but the string
        // is not exhausted; after the Match there are no live consuming states
        // -> dead list before end -> false.
        let nfa = nfa_compile(&process_escapes("test"));
        assert!(!nfa_match(&nfa, b"testing", 0, true, true));
    }

    #[test]
    fn test_match_exact_rejects_short_input() {
        // "test" vs b"tes" full -> ran out of input before reaching Match.
        let nfa = nfa_compile(&process_escapes("test"));
        assert!(!nfa_match(&nfa, b"tes", 0, true, true));
    }

    #[test]
    fn test_match_exact_empty_pattern_empty_string() {
        // "" vs b"" full -> [Match]; seed closure has Match; loop is a no-op;
        // final nfa_has_match -> true.
        let nfa = nfa_compile(&process_escapes(""));
        assert!(nfa_match(&nfa, b"", 0, true, true));
    }

    #[test]
    fn test_match_exact_empty_pattern_nonempty_string() {
        // "" vs b"a" full -> Match is in the seed, but 'a' is unconsumed ->
        // dead list after the first step -> final has_match on empty -> false.
        let nfa = nfa_compile(&process_escapes(""));
        assert!(!nfa_match(&nfa, b"a", 0, true, true));
    }

    #[test]
    fn test_match_exact_case_insensitive() {
        // "abc" vs b"ABC" full, cs=false -> case-insensitive full match.
        let nfa = nfa_compile(&process_escapes("abc"));
        assert!(nfa_match(&nfa, b"ABC", 0, false, true));
    }

    #[test]
    fn test_match_exact_case_sensitive_mismatch() {
        // "abc" vs b"ABC" full, cs=true -> case-sensitive mismatch -> false.
        let nfa = nfa_compile(&process_escapes("abc"));
        assert!(!nfa_match(&nfa, b"ABC", 0, true, true));
    }

    // --- Prefix / substring (full_match=false) — §4.2 (7 rows) ---

    #[test]
    fn test_match_prefix_at_start() {
        // "test" vs b"testing" prefix (full_match=false) -> Match at pos 3 -> true.
        let nfa = nfa_compile(&process_escapes("test"));
        assert!(nfa_match(&nfa, b"testing", 0, true, false));
    }

    #[test]
    fn test_match_prefix_mismatch_at_offset_zero() {
        // "test" vs b"pretest" at start=0, full_match=false -> 't' != 'p' at
        // offset 0 -> dead list immediately -> false. (T3.S1 would retry at
        // offset 3.)
        let nfa = nfa_compile(&process_escapes("test"));
        assert!(!nfa_match(&nfa, b"pretest", 0, true, false));
    }

    #[test]
    fn test_match_prefix_at_offset() {
        // "test" vs b"pretest" at start=3, full_match=false -> prefix at offset 3.
        let nfa = nfa_compile(&process_escapes("test"));
        assert!(nfa_match(&nfa, b"pretest", 3, true, false));
    }

    #[test]
    fn test_match_glob_matches_anything() {
        // "*" vs b"anything" prefix -> glob matches anything.
        let nfa = nfa_compile(&process_escapes("*"));
        assert!(nfa_match(&nfa, b"anything", 0, true, false));
    }

    #[test]
    fn test_match_glob_matches_empty_prefix() {
        // "*" vs b"" prefix -> glob matches the empty prefix (Split->out1->Match
        // is in the seed closure).
        let nfa = nfa_compile(&process_escapes("*"));
        assert!(nfa_match(&nfa, b"", 0, true, false));
    }

    #[test]
    fn test_match_a_star_prefix() {
        // "a*" vs b"aaa" prefix -> glob after 'a'.
        let nfa = nfa_compile(&process_escapes("a*"));
        assert!(nfa_match(&nfa, b"aaa", 0, true, false));
    }

    #[test]
    fn test_match_a_star_b_full() {
        // "a*b" vs b"aaab" full -> full match with a mid glob.
        let nfa = nfa_compile(&process_escapes("a*b"));
        assert!(nfa_match(&nfa, b"aaab", 0, true, true));
    }

    // --- Quantifier + (Char+SPLIT loop) — §4.3 (4 rows) ---

    #[test]
    fn test_match_plus_needs_one() {
        // "a+" vs b"aaa" full -> one-or-more -> true.
        let nfa = nfa_compile(&process_escapes("a+"));
        assert!(nfa_match(&nfa, b"aaa", 0, true, true));
    }

    #[test]
    fn test_match_plus_rejects_empty() {
        // "a+" vs b"" full -> needs >=1 'a' -> false.
        let nfa = nfa_compile(&process_escapes("a+"));
        assert!(!nfa_match(&nfa, b"", 0, true, true));
    }

    #[test]
    fn test_match_plus_rejects_wrong_first_char() {
        // "a+" vs b"b" full -> no leading 'a' -> false.
        let nfa = nfa_compile(&process_escapes("a+"));
        assert!(!nfa_match(&nfa, b"b", 0, true, true));
    }

    #[test]
    fn test_match_digit_plus_full() {
        // "\d+" vs b"123" full -> class + quantifier -> true.
        let nfa = nfa_compile(&process_escapes("\\d+"));
        assert!(nfa_match(&nfa, b"123", 0, true, true));
    }

    // --- Character classes + dot — §4.4 (10 rows) ---

    #[test]
    fn test_match_class_digit_matches_digit() {
        // "\d" vs b"5" -> true.
        let nfa = nfa_compile(&process_escapes("\\d"));
        assert!(nfa_match(&nfa, b"5", 0, true, false));
    }

    #[test]
    fn test_match_class_digit_rejects_letter() {
        // "\d" vs b"a" -> false.
        let nfa = nfa_compile(&process_escapes("\\d"));
        assert!(!nfa_match(&nfa, b"a", 0, true, false));
    }

    #[test]
    fn test_match_class_word_includes_underscore() {
        // "\w" vs b"_" -> true (\w includes underscore).
        let nfa = nfa_compile(&process_escapes("\\w"));
        assert!(nfa_match(&nfa, b"_", 0, true, false));
    }

    #[test]
    fn test_match_class_word_rejects_space() {
        // "\w" vs b" " -> false.
        let nfa = nfa_compile(&process_escapes("\\w"));
        assert!(!nfa_match(&nfa, b" ", 0, true, false));
    }

    #[test]
    fn test_match_class_nword_matches_punctuation() {
        // "\W" vs b"!" -> true (\W matches punctuation).
        let nfa = nfa_compile(&process_escapes("\\W"));
        assert!(nfa_match(&nfa, b"!", 0, true, false));
    }

    #[test]
    fn test_match_class_space_matches_tab() {
        // "\s" vs b"\t" -> true (\s matches tab).
        let nfa = nfa_compile(&process_escapes("\\s"));
        assert!(nfa_match(&nfa, b"\t", 0, true, false));
    }

    #[test]
    fn test_match_class_nspace_rejects_whitespace() {
        // "\S" vs b"\t" -> false (\S does not match whitespace).
        let nfa = nfa_compile(&process_escapes("\\S"));
        assert!(!nfa_match(&nfa, b"\t", 0, true, false));
    }

    #[test]
    fn test_match_dot_matches_letter() {
        // "." vs b"a" -> true (dot matches any non-newline).
        let nfa = nfa_compile(&process_escapes("."));
        assert!(nfa_match(&nfa, b"a", 0, true, false));
    }

    #[test]
    fn test_match_dot_excludes_newline() {
        // "." vs b"\n" -> false (GOTCHA-7: dot EXCLUDES newline).
        let nfa = nfa_compile(&process_escapes("."));
        assert!(!nfa_match(&nfa, b"\n", 0, true, false));
    }

    #[test]
    fn test_match_dot_excludes_cr() {
        // "." vs b"\r" -> false (GOTCHA-7: dot EXCLUDES CR too).
        let nfa = nfa_compile(&process_escapes("."));
        assert!(!nfa_match(&nfa, b"\r", 0, true, false));
    }

    // --- Glob * includes newline (GOTCHA-7 counterpart) ---

    #[test]
    fn test_match_glob_includes_newline() {
        // "*" (glob, compiled as Any) must consume a newline: "*" vs b"a\nb"
        // full_match=true -> true. Any matches ANY byte incl '\n'/'\r'
        // (PRD §13 #8). Contrast with the dot tests above.
        let nfa = nfa_compile(&process_escapes("*"));
        assert!(nfa_match(&nfa, b"a\nb", 0, true, true));
    }

    // --- Word boundary \b (THE abspos/linchpin tests) — §4.5 (7 rows) ---
    //
    // These prove \b evaluates against the ORIGINAL string at an ABSOLUTE
    // offset (GOTCHA-2 / REFINEMENT A). The LINCHPIN is \bword/aword@1 -> false:
    // if \b saw only the per-offset slice "word" (treating it as string_start
    // at abspos 0), it would see 'w' -> boundary -> pass -> true (WRONG). The
    // firmware returns false because \b sees the ORIGINAL "aword" at abspos 1
    // ('a','w' both word -> NO boundary -> \b fails).

    #[test]
    fn test_match_bword_sees_original_string_at_offset() {
        // LINCHPIN (GOTCHA-2 / REFINEMENT A): "\bword" vs b"aword" at start=1
        // -> FALSE. At abspos 1, 'a' and 'w' are both word chars -> NO
        // boundary -> \b fails. A slice-based impl (string_start = "word",
        // abspos = 0) would wrongly see 'w' -> boundary -> pass -> true.
        let nfa = nfa_compile(&process_escapes("\\bword"));
        assert!(!nfa_match(&nfa, b"aword", 1, true, false));
    }

    #[test]
    fn test_match_bword_at_real_boundary_in_string() {
        // "\bword" vs b" word" at start=1 -> true. At abspos 1, ' ' is
        // non-word and 'w' is word -> boundary -> \b passes; then "word"
        // prefix-matches the slice b"word".
        let nfa = nfa_compile(&process_escapes("\\bword"));
        assert!(nfa_match(&nfa, b" word", 1, true, false));
    }

    #[test]
    fn test_match_bword_at_string_start() {
        // "\bword" vs b"word" at start=0 -> true. abspos 0: is_word_char('w')
        // -> boundary -> \b passes; "word" prefix-matches.
        let nfa = nfa_compile(&process_escapes("\\bword"));
        assert!(nfa_match(&nfa, b"word", 0, true, false));
    }

    #[test]
    fn test_match_bword_fails_after_digit() {
        // "\bword" vs b"123word" at start=3 -> false. abspos 3: '3' and 'w'
        // both word -> NO boundary -> \b fails.
        let nfa = nfa_compile(&process_escapes("\\bword"));
        assert!(!nfa_match(&nfa, b"123word", 3, true, false));
    }

    #[test]
    fn test_match_bword_fails_after_underscore() {
        // "\bword" vs b"_word" at start=1 -> false. abspos 1: '_' and 'w'
        // both word -> NO boundary -> \b fails.
        let nfa = nfa_compile(&process_escapes("\\bword"));
        assert!(!nfa_match(&nfa, b"_word", 1, true, false));
    }

    #[allow(non_snake_case)] // capital B mirrors the \B (non-boundary) assertion
    #[test]
    fn test_match_Bord_non_boundary_inside_word() {
        // "\Bord" vs b"word" at start=1 -> true. abspos 1: 'w' and 'o' both
        // word -> NOT a boundary -> \B (non-boundary) passes.
        let nfa = nfa_compile(&process_escapes("\\Bord"));
        assert!(nfa_match(&nfa, b"word", 1, true, false));
    }

    #[allow(non_snake_case)] // capital B mirrors the \B (non-boundary) assertion
    #[test]
    fn test_match_Bord_fails_at_string_start() {
        // "\Bord" vs b"ord" at start=0 -> false. abspos 0: is_word_char('o')
        // -> boundary -> \B fails.
        let nfa = nfa_compile(&process_escapes("\\Bord"));
        assert!(!nfa_match(&nfa, b"ord", 0, true, false));
    }

    // --- Empty-string \b/\B special case — §4.6 (3 rows) ---

    #[test]
    fn test_match_b_empty_false() {
        // "\b" vs b"" -> false (GOTCHA-6: empty original string -> \b
        // short-circuits, no recurse -> no Match). This is independent of
        // is_word_boundary (which would otherwise return false at pos 0 on
        // empty, making \B WRONGLY pass).
        let nfa = nfa_compile(&process_escapes("\\b"));
        assert!(!nfa_match(&nfa, b"", 0, true, false));
    }

    #[allow(non_snake_case)] // capital B mirrors the \B (non-boundary) assertion
    #[test]
    fn test_match_B_empty_false() {
        // "\B" vs b"" -> false (GOTCHA-6: empty original string -> \B ALSO
        // fails — legacy semantics the test corpus encodes).
        let nfa = nfa_compile(&process_escapes("\\B"));
        assert!(!nfa_match(&nfa, b"", 0, true, false));
    }

    #[test]
    fn test_match_bb_generation_dedup_terminates() {
        // "\b\b" vs b"a" -> true (GOTCHA-4/11). Two zero-width asserts then
        // Match: at abspos 0 both \b pass (boundary at 'a'), so the seed
        // closure reaches Match. The generation-tag dedup is what makes the
        // converging epsilon edges terminate (no infinite recursion).
        let nfa = nfa_compile(&process_escapes("\\b\\b"));
        assert!(nfa_match(&nfa, b"a", 0, true, false));
    }

    // ============================================================
    // match_with_anchors + pattern_match (P2.M1.T3.S1)
    //
    // End-to-end parity vectors curated from the firmware's 380-row corpus
    // (research/notes.md §7), grouped by anchor mode + the empty-core special
    // case + escapes/classes/\b. The firmware `pattern_match.c` is the single
    // source of truth for match semantics (PRD §14); every row asserts the
    // firmware-expected bool via the public `pattern_match` (the public
    // contract) plus a few direct `match_with_anchors` calls to isolate the
    // four-mode dispatch.
    // ============================================================

    // --- Start anchor ^ (prefix, full_match=false, offset 0) --- §7.1

    #[test]
    fn test_pm_start_anchor_prefix() {
        assert!(pattern_match("^searchterm", "searchterm", true));
        assert!(!pattern_match("^searchterm", "presearchterm", true));
        assert!(pattern_match("^searchterm", "searchtermpost", true)); // reach-any
        assert!(pattern_match("^test", "test123", true));
        assert!(!pattern_match("^test", "pretest", true));
    }

    #[test]
    fn test_pm_start_anchor_empty_core() {
        // Empty core prefix (^ matches everything — traced in notes §4).
        assert!(pattern_match("^", "", true));
    }

    #[test]
    fn test_pm_start_anchor_case_insensitive() {
        assert!(pattern_match("^abc", "ABC", false));
        assert!(!pattern_match("^abc", "ABC", true));
    }

    #[test]
    fn test_pm_start_anchor_with_glob() {
        assert!(pattern_match("^*test", "anytest", true));
        assert!(pattern_match("^*", "anything", true));
    }

    // --- End anchor $ (suffix, full_match=true, loop offsets) --- §7.2

    #[test]
    fn test_pm_end_anchor_suffix() {
        assert!(pattern_match("searchterm$", "searchterm", true));
        assert!(!pattern_match("searchterm$", "searchtermpost", true));
        assert!(pattern_match("searchterm$", "presearchterm", true)); // loops offsets
        assert!(pattern_match("test$", "pretest", true));
        assert!(!pattern_match("test$", "test123", true));
    }

    #[test]
    fn test_pm_end_anchor_empty_core() {
        // Empty core suffix ($ matches everything — traced in notes §4).
        assert!(pattern_match("$", "", true));
    }

    #[test]
    fn test_pm_end_anchor_case_insensitive() {
        assert!(pattern_match("abc$", "ABC", false));
        assert!(!pattern_match("abc$", "ABC", true));
    }

    #[test]
    fn test_pm_end_anchor_with_glob() {
        assert!(pattern_match("test*$", "testany", true));
        assert!(pattern_match("*$", "anything", true));
    }

    // --- Full anchor ^…$ (exact, full_match=true, offset 0) --- §7.3

    #[test]
    fn test_pm_full_anchor_exact() {
        assert!(pattern_match("^searchterm$", "searchterm", true));
        assert!(!pattern_match("^searchterm$", "presearchterm", true));
        assert!(!pattern_match("^searchterm$", "searchtermpost", true));
        assert!(!pattern_match("^searchterm$", "presearchtermpost", true));
        assert!(pattern_match("^test$", "test", true));
    }

    #[test]
    fn test_pm_full_anchor_empty_core() {
        // ^$ matches empty only (GOTCHA-A natural case for exact mode).
        assert!(pattern_match("^$", "", true));
        assert!(!pattern_match("^$", "a", true));
    }

    #[test]
    fn test_pm_full_anchor_case_insensitive() {
        assert!(pattern_match("^abc$", "ABC", false)); // ci exact
        assert!(!pattern_match("^abc$", "ABC", true)); // cs exact
    }

    #[test]
    fn test_pm_full_anchor_with_glob() {
        assert!(pattern_match("^sear*term$", "searchterm", true));
        assert!(pattern_match("^sear*term$", "searedsalmonterm", true)); // glob expansion
        assert!(pattern_match("^a*b*c$", "aabbcc", true)); // multiple globs
        assert!(pattern_match("^*$", "anything", true)); // full glob exact (matches all)
    }

    // --- Substring (no anchors, full_match=false, loop offsets) + empty-core
    //     special case (GOTCHA-A) --- §7.4

    #[test]
    fn test_pm_substring_basic() {
        assert!(pattern_match("searchterm", "presearchtermpost", true));
        assert!(pattern_match("sear*term", "presearchtermpost", true)); // glob
        assert!(pattern_match("*term", "searchterm", true)); // leading glob (suffix-like)
        assert!(pattern_match("search*", "searchterm", true)); // trailing glob (prefix-like)
        assert!(pattern_match("test", "test", true));
        assert!(pattern_match("test", "testing", true));
    }

    #[test]
    fn test_pm_substring_full_glob_matches_anything() {
        // GOTCHA-B linchpin: a trailing/terminal Match at the very end is only
        // reached because the loop probes i == bytes.len().
        assert!(pattern_match("*", "anything", true));
    }

    #[test]
    fn test_pm_substring_empty_core_only_matches_empty() {
        // GOTCHA-A — THE parity trap. An empty unanchored pattern matches ONLY
        // the empty string, NOT everything. Without the guard, the empty NFA
        // [Match] would reach Match at offset 0 in reach-any mode (full_match=false)
        // and wrongly return true for "test".
        assert!(pattern_match("", "", true)); // empty/empty -> true
        assert!(!pattern_match("", "test", true)); // empty/non-empty -> FALSE (the special case)
    }

    #[test]
    fn test_pm_substring_nonempty_pattern_empty_input() {
        assert!(!pattern_match("test", "", true)); // non-empty pattern, empty str
    }

    #[test]
    fn test_pm_substring_case_insensitive() {
        assert!(pattern_match("abc", "ABC", false));
        assert!(!pattern_match("abc", "ABC", true));
    }

    #[test]
    fn test_pm_substring_globs_both_sides_and_min() {
        assert!(pattern_match("*test*", "pretestpost", true));
        assert!(pattern_match("a*", "a", true)); // glob min match
    }

    // --- Edge cases / escapes / classes / \b (cross-mode) --- §7.5

    #[test]
    fn test_pm_double_caret_double_dollar_literals() {
        // 1st ^ anchors; 2nd ^ is a literal core byte (0x5E). Trailing $ anchors;
        // the $ before it is a literal core byte (0x24).
        assert!(pattern_match("^^test", "^test", true));
        assert!(pattern_match("test$$", "test$", true));
    }

    #[test]
    fn test_pm_complex_escape_literal() {
        // User-authored pattern is the 4 chars `\\\^` (three backslashes then
        // caret): process_escapes turns `\\`->ESC_BSLASH(0x04) then `\^`->
        // ESC_CARET(0x01), so the core decodes to the literal two bytes `\^`.
        // (Rust source: "\\\\\\^" == the string \\\^; "\\^" == \^.)
        assert!(pattern_match("\\\\\\^", "\\^", true));
        assert!(pattern_match("\\\\", "\\", true)); // single backslash
    }

    #[test]
    fn test_pm_digit_class_prefix_suffix() {
        assert!(pattern_match("^\\d", "5", true));
        assert!(!pattern_match("^\\d", "a5", true)); // non-digit at start
        assert!(pattern_match("\\d$", "5", true));
    }

    #[test]
    fn test_pm_plus_quantifier_end_to_end() {
        assert!(pattern_match("^\\d+$", "12345", true)); // exact + \d+
        assert!(pattern_match("^\\w+$", "hello_1", true)); // exact + \w+
    }

    #[test]
    fn test_pm_bword_linchpin_false() {
        // THE linchpin end-to-end: substring loop + original-string \b threading
        // must compose (REFINEMENT F + the `start` offset + \b all together).
        // "\bword" vs "aword" -> false: there is NO boundary between 'a' and
        // 'word' (both word chars), so \b fails and the substring match rejects.
        assert!(!pattern_match("\\bword", "aword", true));
        // Boundary before 'word' (space separates) -> matches.
        assert!(pattern_match("\\bword", "a word", true));
    }

    // --- Direct match_with_anchors isolation (per-mode dispatch) ---

    #[test]
    fn test_mwa_suffix_loops_offsets() {
        // Directly confirms the suffix branch loops offsets (not a single
        // offset-0 call): "test$" must match "pretest" (suffix found mid-string).
        let parsed = parse_pattern("test$");
        assert!(parsed.end_anchored && !parsed.start_anchored);
        assert!(match_with_anchors(&parsed, "pretest", true));
    }

    #[test]
    fn test_mwa_substring_empty_core_guard() {
        // Directly confirms the GOTCHA-A guard in the substring branch.
        let parsed = parse_pattern("");
        assert!(!parsed.start_anchored && !parsed.end_anchored);
        assert!(match_with_anchors(&parsed, "", true));
        assert!(!match_with_anchors(&parsed, "test", true));
    }

    #[test]
    fn test_mwa_full_anchor_single_call() {
        // Exact mode: a single offset-0 full match. Both anchors set.
        let parsed = parse_pattern("^abc$");
        assert!(parsed.start_anchored && parsed.end_anchored);
        assert!(match_with_anchors(&parsed, "abc", true));
        assert!(!match_with_anchors(&parsed, "xabc", true));
    }

    // --- Pattern::Single: always matches app_class (title ignored) ---

    #[test]
    fn test_mp_single_matches_app_class_no_title() {
        // class exact, no title → whole-msg match
        assert!(match_pattern(
            &Pattern::Single("Firefox".into()),
            "Firefox",
            "",
            false
        ));
    }

    #[test]
    fn test_mp_single_matches_app_class_title_present_ignored() {
        // class exact, title PRESENT but ignored (firmware parity)
        assert!(match_pattern(
            &Pattern::Single("Firefox".into()),
            "Firefox",
            "Google",
            false
        ));
    }

    #[test]
    fn test_mp_single_case_insensitive_default() {
        assert!(match_pattern(
            &Pattern::Single("firefox".into()),
            "Firefox",
            "",
            false
        ));
    }

    #[test]
    fn test_mp_single_case_sensitive() {
        assert!(!match_pattern(
            &Pattern::Single("firefox".into()),
            "Firefox",
            "",
            true
        ));
    }

    #[test]
    fn test_mp_single_class_mismatch() {
        assert!(!match_pattern(
            &Pattern::Single("Firefox".into()),
            "Chrome",
            "",
            false
        ));
    }

    #[test]
    fn test_mp_single_ignores_title_linchpin() {
        // THE linchpin (G1): title matches the PATTERN but Single ignores title.
        // An implementer who joined "class\x1Dtitle" and matched Single against
        // it would wrongly return true here.
        assert!(!match_pattern(
            &Pattern::Single("Firefox".into()),
            "Chrome",
            "Firefox",
            false
        ));
    }

    #[test]
    fn test_mp_single_glob_any_class() {
        assert!(match_pattern(
            &Pattern::Single("*".into()),
            "anything",
            "",
            false
        ));
    }

    #[test]
    fn test_mp_single_glob_substring() {
        assert!(match_pattern(
            &Pattern::Single("*ire*".into()),
            "Firefox",
            "",
            false
        ));
    }

    #[test]
    fn test_mp_single_case_sensitive_mismatch() {
        // cs mismatch (only ASCII-folds A–Z)
        assert!(!match_pattern(
            &Pattern::Single("Firefox".into()),
            "firefox",
            "",
            true
        ));
    }

    #[test]
    fn test_mp_single_empty_pattern_non_empty_core() {
        // empty pattern, non-empty class → empty-core special case (T3.S1)
        assert!(!match_pattern(
            &Pattern::Single("".into()),
            "Firefox",
            "",
            false
        ));
    }

    #[test]
    fn test_mp_single_empty_pattern_empty_core() {
        // empty pattern, empty class → empty-core matches
        assert!(match_pattern(&Pattern::Single("".into()), "", "", false));
    }

    #[test]
    fn test_mp_single_anchors_end_to_end() {
        assert!(match_pattern(
            &Pattern::Single("^Firefox$".into()),
            "Firefox",
            "",
            false
        ));
    }

    // --- Pattern::Parts: both halves must match ---

    #[test]
    fn test_mp_parts_both_halves_match() {
        assert!(match_pattern(
            &Pattern::Parts("Firefox".into(), "*youtube*".into()),
            "Firefox",
            "Youtube - X",
            false
        ));
    }

    #[test]
    fn test_mp_parts_title_half_fails() {
        // title half fails (substring "youtube" ∉ "Google")
        assert!(!match_pattern(
            &Pattern::Parts("Firefox".into(), "youtube".into()),
            "Firefox",
            "Google",
            false
        ));
    }

    #[test]
    fn test_mp_parts_title_glob_matches_anything() {
        assert!(match_pattern(
            &Pattern::Parts("Chrome".into(), "*".into()),
            "Chrome",
            "anything",
            false
        ));
    }

    #[test]
    fn test_mp_parts_class_half_fails() {
        assert!(!match_pattern(
            &Pattern::Parts("Chrome".into(), "*".into()),
            "Firefox",
            "anything",
            false
        ));
    }

    #[test]
    fn test_mp_parts_empty_title_composes() {
        // G8: empty title-pattern matches empty title (T3.S1 empty-core:
        // pattern_match("", "") == true)
        assert!(match_pattern(
            &Pattern::Parts("Firefox".into(), "".into()),
            "Firefox",
            "",
            false
        ));
    }

    #[test]
    fn test_mp_parts_empty_title_vs_non_empty_core() {
        // G8: empty title-pattern vs non-empty title → empty-core:
        // pattern_match("", "Google") == false
        assert!(!match_pattern(
            &Pattern::Parts("Firefox".into(), "".into()),
            "Firefox",
            "Google",
            false
        ));
    }

    #[test]
    fn test_mp_parts_glob_title_matches_empty() {
        // glob `*` matches empty title
        assert!(match_pattern(
            &Pattern::Parts("Firefox".into(), "*".into()),
            "Firefox",
            "",
            false
        ));
    }

    #[test]
    fn test_mp_parts_case_insensitive_both_halves() {
        assert!(match_pattern(
            &Pattern::Parts("firefox".into(), "*youtube*".into()),
            "Firefox",
            "MYoutube",
            false
        ));
    }

    #[test]
    fn test_mp_parts_case_sensitive_both_halves() {
        assert!(!match_pattern(
            &Pattern::Parts("firefox".into(), "*youtube*".into()),
            "Firefox",
            "MYoutube",
            true
        ));
    }

    #[test]
    fn test_mp_parts_anchors_both_halves_end_to_end() {
        assert!(match_pattern(
            &Pattern::Parts("^Firefox$".into(), "^*youtube*$".into()),
            "Firefox",
            "youtube",
            false
        ));
    }

    // --- serde untagged deserialization (rules.toml -> Pattern) ---

    #[derive(serde::Deserialize)]
    struct Wrap {
        #[serde(rename = "match")]
        pattern: Pattern,
    }

    #[test]
    fn test_pattern_serde_string_to_single() {
        assert_eq!(
            toml::from_str::<Wrap>(r#"match = "alacritty""#)
                .unwrap()
                .pattern,
            Pattern::Single("alacritty".into())
        );
    }

    #[test]
    fn test_pattern_serde_glob_string_to_single() {
        assert_eq!(
            toml::from_str::<Wrap>(r#"match = "*chrome*""#)
                .unwrap()
                .pattern,
            Pattern::Single("*chrome*".into())
        );
    }

    #[test]
    fn test_pattern_serde_two_array_to_parts() {
        assert_eq!(
            toml::from_str::<Wrap>(r#"match = ["*chrome*", "*youtube*"]"#)
                .unwrap()
                .pattern,
            Pattern::Parts("*chrome*".into(), "*youtube*".into())
        );
    }

    #[test]
    fn test_pattern_serde_three_array_errors() {
        // G5: 3-array → Parts needs exactly 2 → error
        assert!(toml::from_str::<Wrap>(r#"match = ["a", "b", "c"]"#).is_err());
    }

    #[test]
    fn test_pattern_serde_one_array_errors() {
        // G5: 1-array → Parts needs exactly 2 → error
        assert!(toml::from_str::<Wrap>(r#"match = ["solo"]"#).is_err());
    }

    #[test]
    fn test_pattern_serde_int_errors() {
        // G5: int matches no variant → error
        assert!(toml::from_str::<Wrap>(r#"match = 42"#).is_err());
    }
    // ===== FIRMWARE PARITY CORPUS (P2.M1.T4.S1) =====
    // Ports the firmware qmk_notifier/test_*.c pattern_match corpus (8 files,
    // 1225 assertion cases) as Rust parity tests. Source of truth: the C files
    // (PRD §14). A failure = the Rust leaf `pattern_match` diverged from the
    // firmware → fix the Rust, NOT the test.
    //
    // Skip lists (impossible in the Rust type system), each cited inline:
    //   G2 — invalid-UTF-8 bytes (0xFE/0xFF) cannot be a Rust &str.
    //   G3 — NULL-pointer cases have no &str analog (Rust &str is never null).
    // Special-byte escape map (G1): C "\f"→"\x0C", C "\v"→"\x0B" (Rust has no
    // \f/\v escapes); \t/\n/\r are identical.
    // The 4420 non-assertion executions (perf + crash-safety loops) are
    // represented by ONE `test_parity_invalid_no_panic` property test.

    /// A single firmware parity vector. Mirrors the C `test_case_t` (minus the
    /// human description, which the Rust port drops — the indexed failure
    /// message carries pattern/input/cs/got/exp instead).
    struct Case {
        pattern: &'static str,
        input: &'static str,
        cs: bool,
        exp: bool,
    }

    /// Assert `pattern_match(pattern, input, cs) == exp` for EVERY case,
    /// printing a precise, indexed failure message on the first divergence.
    /// Mirrors the firmware `run_test()` loop (qmk_notifier/test_pattern_match.c).
    fn assert_parity(cases: &[Case]) {
        for (i, c) in cases.iter().enumerate() {
            let got = pattern_match(c.pattern, c.input, c.cs);
            assert!(
                got == c.exp,
                "parity FAIL [#{i}] pattern_match({:?}, {:?}, cs={}) = {}, expected {}",
                c.pattern,
                c.input,
                c.cs,
                got,
                c.exp
            );
        }
    }

    // ----------------------------------------------------------------------
    // test_pattern_match.c (380 cases, 16 fns; no-op #5 skipped)
    // ----------------------------------------------------------------------

    #[test]
    fn test_parity_pm_start_anchor() {
        assert_parity(&[
            Case {
                pattern: "^searchterm",
                input: "searchterm",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^searchterm",
                input: "presearchterm",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^searchterm",
                input: "searchtermpost",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^test",
                input: "test123",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^test",
                input: "pretest",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^abc",
                input: "ABC",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "^abc",
                input: "ABC",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_pm_end_anchor() {
        assert_parity(&[
            Case {
                pattern: "searchterm$",
                input: "searchterm",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "searchterm$",
                input: "searchtermpost",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "searchterm$",
                input: "presearchterm",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test$",
                input: "pretest",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test$",
                input: "test123",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "$",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "abc$",
                input: "ABC",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "abc$",
                input: "ABC",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_pm_full_anchor() {
        assert_parity(&[
            Case {
                pattern: "^searchterm$",
                input: "searchterm",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^searchterm$",
                input: "presearchterm",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^searchterm$",
                input: "searchtermpost",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^searchterm$",
                input: "presearchtermpost",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^test$",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^$",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^abc$",
                input: "ABC",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "^abc$",
                input: "ABC",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_pm_anchors_with_wildcards() {
        assert_parity(&[
            Case {
                pattern: "^sear*term$",
                input: "searchterm",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^sear*term$",
                input: "searedsalmonterm",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^sear*term$",
                input: "somesearchterm",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^sear*term$",
                input: "searchtermhere",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^*test",
                input: "anytest",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test*$",
                input: "testany",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^*$",
                input: "anything",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^a*b*c$",
                input: "abc",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^a*b*c$",
                input: "aabbcc",
                cs: true,
                exp: true,
            },
        ]);
    }

    // Firmware #5 `test_character_classification` is a printf doc stub (0 cases) — skipped.

    #[test]
    fn test_parity_pm_basic_metacharacter_escapes() {
        assert_parity(&[
            Case {
                pattern: "\\d",
                input: "\\d",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\d",
                input: "d",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\D",
                input: "5",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\D",
                input: "D",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w",
                input: " ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\w",
                input: "w",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\W",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\W",
                input: "W",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\s",
                input: "\\s",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\s",
                input: "s",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\S",
                input: " ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\S",
                input: "S",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\x",
                input: "\\x",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\z",
                input: "\\z",
                cs: true,
                exp: true,
            },
        ]);
    }

    #[test]
    fn test_parity_pm_basic_metacharacter_matching() {
        assert_parity(&[
            // \d
            Case {
                pattern: "\\d",
                input: "0",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d",
                input: "1",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d",
                input: "9",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\d",
                input: "A",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\d",
                input: " ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\d",
                input: "_",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\d",
                input: "!",
                cs: true,
                exp: false,
            },
            // \D
            Case {
                pattern: "\\D",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\D",
                input: "A",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\D",
                input: " ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\D",
                input: "_",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\D",
                input: "!",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\D",
                input: "0",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\D",
                input: "5",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\D",
                input: "9",
                cs: true,
                exp: false,
            },
            // \w
            Case {
                pattern: "\\w",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w",
                input: "z",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w",
                input: "A",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w",
                input: "Z",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w",
                input: "0",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w",
                input: "9",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w",
                input: "_",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w",
                input: " ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\w",
                input: "!",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\w",
                input: "-",
                cs: true,
                exp: false,
            },
            // \W
            Case {
                pattern: "\\W",
                input: " ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\W",
                input: "!",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\W",
                input: "-",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\W",
                input: ".",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\W",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\W",
                input: "Z",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\W",
                input: "5",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\W",
                input: "_",
                cs: true,
                exp: false,
            },
            // \s — G1: C "\f"→"\x0C", C "\v"→"\x0B"
            Case {
                pattern: "\\s",
                input: " ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\s",
                input: "\t",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\s",
                input: "\n",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\s",
                input: "\r",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\s",
                input: "\x0C",
                cs: true,
                exp: true,
            }, // C "\f"
            Case {
                pattern: "\\s",
                input: "\x0B",
                cs: true,
                exp: true,
            }, // C "\v"
            Case {
                pattern: "\\s",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\s",
                input: "0",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\s",
                input: "_",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\s",
                input: "!",
                cs: true,
                exp: false,
            },
            // \S — G1: C "\f"→"\x0C", C "\v"→"\x0B"
            Case {
                pattern: "\\S",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\S",
                input: "0",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\S",
                input: "_",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\S",
                input: "!",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\S",
                input: " ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\S",
                input: "\t",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\S",
                input: "\n",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\S",
                input: "\r",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\S",
                input: "\x0C",
                cs: true,
                exp: false,
            }, // C "\f"
            Case {
                pattern: "\\S",
                input: "\x0B",
                cs: true,
                exp: false,
            }, // C "\v"
            // case sensitivity for \w/\W
            Case {
                pattern: "\\w",
                input: "A",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\W",
                input: "A",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\W",
                input: "a",
                cs: true,
                exp: false,
            },
            // multiple chars (first only)
            Case {
                pattern: "\\d",
                input: "123",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w",
                input: "abc",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\s",
                input: "   ",
                cs: true,
                exp: true,
            },
            // special chars
            Case {
                pattern: "\\d",
                input: "@",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\w",
                input: "@",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\s",
                input: "@",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\D",
                input: "@",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\W",
                input: "@",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\S",
                input: "@",
                cs: true,
                exp: true,
            },
        ]);
    }

    #[test]
    fn test_parity_pm_word_boundary_escape_processing() {
        assert_parity(&[
            Case {
                pattern: "\\B",
                input: "B",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\x",
                input: "\\x",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\z",
                input: "\\z",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\btest",
                input: "\\btest",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "test\\B",
                input: "test\\B",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b\\B",
                input: "\\b\\B",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\B\\b",
                input: "\\B\\b",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\^\\b",
                input: "^\\b",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\*\\b",
                input: "*\\b",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_pm_escape_sequences() {
        assert_parity(&[
            Case {
                pattern: "\\^searchterm",
                input: "^searchterm",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\^searchterm",
                input: "searchterm",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "searchterm\\$",
                input: "searchterm$",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "searchterm\\$",
                input: "searchterm",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "search\\*term",
                input: "search*term",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "search\\*term",
                input: "searchanyterm",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\\\^searchterm",
                input: "\\^searchterm",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\^test\\$",
                input: "^test$",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test\\*\\*test",
                input: "test**test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\\\test",
                input: "\\test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test\\",
                input: "test\\",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\^Test",
                input: "^test",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "\\^Test",
                input: "^test",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\a",
                input: "\\a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\\\\\^",
                input: "\\^",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\\\\\\\",
                input: "\\\\",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test\\\\",
                input: "test\\",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\",
                input: "\\",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\\\",
                input: "\\",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\^\\$\\*\\\\",
                input: "^$*\\",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "pre\\^mid\\$post",
                input: "pre^mid$post",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\^start",
                input: "^start",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "end\\$",
                input: "end$",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "normal",
                input: "normal",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "nor\\*mal",
                input: "nor*mal",
                cs: true,
                exp: true,
            },
        ]);
    }

    #[test]
    fn test_parity_pm_backward_compatibility() {
        assert_parity(&[
            Case {
                pattern: "searchterm",
                input: "presearchtermpost",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "sear*term",
                input: "presearchtermpost",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*term",
                input: "searchterm",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "search*",
                input: "searchterm",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test",
                input: "testing",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*",
                input: "anything",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "abc",
                input: "ABC",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "abc",
                input: "ABC",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "a*b*c",
                input: "aabbcc",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*test*",
                input: "pretestpost",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "a*",
                input: "a",
                cs: true,
                exp: true,
            },
        ]);
    }

    #[test]
    fn test_parity_pm_case_sensitivity() {
        assert_parity(&[
            Case {
                pattern: "^SearchTerm$",
                input: "searchterm",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "^SearchTerm$",
                input: "searchterm",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\^SearchTerm",
                input: "^searchterm",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "\\^SearchTerm",
                input: "^searchterm",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^Test*",
                input: "testany",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "^Test*",
                input: "testany",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "*Test$",
                input: "anytest",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "*Test$",
                input: "anytest",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "Test\\*",
                input: "test*",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "Test\\*",
                input: "test*",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_pm_pattern_parsing() {
        assert_parity(&[
            Case {
                pattern: "^test",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^test",
                input: "pretest",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "test$",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test$",
                input: "testpost",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^test$",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^test$",
                input: "pretest",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^test$",
                input: "testpost",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\^test",
                input: "^test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\^test",
                input: "test",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "test\\$",
                input: "test$",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test\\$",
                input: "test",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "test\\*test",
                input: "test*test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test\\*test",
                input: "testanytest",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\\\test",
                input: "\\test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\^test\\$",
                input: "^test$",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\^test",
                input: "^test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test\\$$",
                input: "test$",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^test\\*$",
                input: "test*",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "$",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^$",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\",
                input: "\\",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\^",
                input: "^",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\$",
                input: "$",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\*",
                input: "*",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^Test",
                input: "test",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "^Test",
                input: "test",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\^Test",
                input: "^test",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "\\^Test",
                input: "^test",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_pm_edge_cases() {
        assert_parity(&[
            Case {
                pattern: "",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test",
                input: "",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "",
                input: "test",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^^test",
                input: "^test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test$$",
                input: "test$",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^$",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^$",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\\\\\^",
                input: "\\^",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test\\",
                input: "test\\",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\",
                input: "\\",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^*",
                input: "anything",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*$",
                input: "anything",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^**$",
                input: "test",
                cs: true,
                exp: true,
            },
        ]);
    }

    #[test]
    fn test_parity_pm_metacharacters_with_anchors() {
        assert_parity(&[
            // \d anchors
            Case {
                pattern: "^\\d",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\d",
                input: "a5",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\d$",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d$",
                input: "5a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\d$",
                input: "7",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\d$",
                input: "77",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\d$",
                input: "a",
                cs: true,
                exp: false,
            },
            // \D anchors
            Case {
                pattern: "^\\D",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\D",
                input: "5a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\D$",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\D$",
                input: "a5",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\D$",
                input: "x",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\D$",
                input: "5",
                cs: true,
                exp: false,
            },
            // \w anchors
            Case {
                pattern: "^\\w",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\w",
                input: " a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\w$",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w$",
                input: "a ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\w$",
                input: "z",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\w$",
                input: "_",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\w$",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\w$",
                input: " ",
                cs: true,
                exp: false,
            },
            // \W anchors
            Case {
                pattern: "^\\W",
                input: " ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\W",
                input: "a ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\W$",
                input: " ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\W$",
                input: " a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\W$",
                input: "!",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\W$",
                input: "a",
                cs: true,
                exp: false,
            },
            // \s anchors
            Case {
                pattern: "^\\s",
                input: " ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\s",
                input: "a ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\s$",
                input: " ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\s$",
                input: " a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\s$",
                input: "\t",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\s$",
                input: "\n",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\s$",
                input: "a",
                cs: true,
                exp: false,
            },
            // \S anchors
            Case {
                pattern: "^\\S",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\S",
                input: " a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\S$",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\S$",
                input: "a ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\S$",
                input: "x",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\S$",
                input: " ",
                cs: true,
                exp: false,
            },
            // multiple metachars + anchors
            Case {
                pattern: "^\\d\\w",
                input: "5a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\d\\w",
                input: "a5",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\w\\d$",
                input: "a5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w\\d$",
                input: "5a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\s\\S$",
                input: " a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\S\\s$",
                input: "a ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^a\\d",
                input: "a5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^a\\d",
                input: "5a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\db$",
                input: "5b",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\db$",
                input: "b5",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^x\\sy$",
                input: "x y",
                cs: true,
                exp: true,
            },
            // @-literal regression guard
            Case {
                pattern: "^\\w+@\\w+$",
                input: "user@host",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\w+@\\w+$",
                input: "user_host",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\w+_\\w+$",
                input: "user_host",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w+@\\w+",
                input: "user@host",
                cs: true,
                exp: true,
            },
        ]);
    }

    #[test]
    fn test_parity_pm_metacharacters_with_wildcards() {
        assert_parity(&[
            // \d wildcards
            Case {
                pattern: "\\d*",
                input: "123",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d*",
                input: "1abc",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d*",
                input: "abc",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "*\\d",
                input: "abc5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\d",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\d",
                input: "abc",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "a*\\d",
                input: "a5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "a*\\d",
                input: "abc5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "a*\\d",
                input: "b5",
                cs: true,
                exp: false,
            },
            // \D wildcards
            Case {
                pattern: "\\D*",
                input: "abc",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\D*",
                input: "a123",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\D*",
                input: "123",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "*\\D",
                input: "123a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\D",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\D",
                input: "123",
                cs: true,
                exp: false,
            },
            // \w wildcards
            Case {
                pattern: "\\w*",
                input: "abc",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w*",
                input: "a!@#",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w*",
                input: "!@#",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "*\\w",
                input: "!@#a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\w",
                input: "_",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\w",
                input: "!@#",
                cs: true,
                exp: false,
            },
            // \W wildcards
            Case {
                pattern: "\\W*",
                input: "!@#",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\W*",
                input: "!abc",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\W*",
                input: "abc",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "*\\W",
                input: "abc!",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\W",
                input: "!",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\W",
                input: "abc",
                cs: true,
                exp: false,
            },
            // \s wildcards
            Case {
                pattern: "\\s*",
                input: " \t\n",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\s*",
                input: " abc",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\s*",
                input: "abc",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "*\\s",
                input: "abc ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\s",
                input: "\t",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\s",
                input: "abc",
                cs: true,
                exp: false,
            },
            // \S wildcards
            Case {
                pattern: "\\S*",
                input: "abc",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\S*",
                input: "a \t",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\S*",
                input: " \t",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "*\\S",
                input: " \ta",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\S",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\S",
                input: " \t",
                cs: true,
                exp: false,
            },
            // multiple wildcards
            Case {
                pattern: "*\\d*",
                input: "abc5xyz",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\d*",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\d*",
                input: "abc",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "*\\w*\\s*",
                input: "!a ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\w*\\s*",
                input: "a",
                cs: true,
                exp: false,
            },
            // complex
            Case {
                pattern: "^\\d*test",
                input: "123test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\d*test",
                input: "test",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "test\\s*$",
                input: "test   ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test\\s*$",
                input: "test",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\w*\\d*$",
                input: "abc123",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\w*\\d*$",
                input: "123",
                cs: true,
                exp: true,
            },
        ]);
    }

    #[test]
    fn test_parity_pm_metacharacter_case_sensitivity() {
        assert_parity(&[
            // \w
            Case {
                pattern: "\\w",
                input: "A",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w",
                input: "A",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "\\w",
                input: "a",
                cs: false,
                exp: true,
            },
            // \W
            Case {
                pattern: "\\W",
                input: "A",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\W",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\W",
                input: "A",
                cs: false,
                exp: false,
            },
            Case {
                pattern: "\\W",
                input: "a",
                cs: false,
                exp: false,
            },
            Case {
                pattern: "\\W",
                input: "!",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\W",
                input: "!",
                cs: false,
                exp: true,
            },
            // \d / \D
            Case {
                pattern: "\\d",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d",
                input: "5",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "\\D",
                input: "5",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\D",
                input: "5",
                cs: false,
                exp: false,
            },
            Case {
                pattern: "\\D",
                input: "A",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\D",
                input: "A",
                cs: false,
                exp: true,
            },
            // \s / \S
            Case {
                pattern: "\\s",
                input: " ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\s",
                input: " ",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "\\S",
                input: " ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\S",
                input: " ",
                cs: false,
                exp: false,
            },
            Case {
                pattern: "\\S",
                input: "A",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\S",
                input: "A",
                cs: false,
                exp: true,
            },
            // mixed literal + metachar
            Case {
                pattern: "Test\\w",
                input: "TestA",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "Test\\w",
                input: "testa",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "Test\\w",
                input: "TestA",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "Test\\w",
                input: "testa",
                cs: false,
                exp: true,
            },
            // anchored
            Case {
                pattern: "^Test\\d$",
                input: "Test5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^Test\\d$",
                input: "test5",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^Test\\d$",
                input: "Test5",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "^Test\\d$",
                input: "test5",
                cs: false,
                exp: true,
            },
            // wildcard
            Case {
                pattern: "Test*\\w",
                input: "TestAnyA",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "Test*\\w",
                input: "testanya",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "Test*\\w",
                input: "TestAnyA",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "Test*\\w",
                input: "testanya",
                cs: false,
                exp: true,
            },
        ]);
    }

    #[test]
    fn test_parity_pm_metacharacter_backward_compatibility() {
        assert_parity(&[
            Case {
                pattern: "test",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test",
                input: "testing",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test*",
                input: "testing",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*test",
                input: "pretest",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^test",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test$",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^test$",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\^test",
                input: "^test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test\\$",
                input: "test$",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test\\*test",
                input: "test*test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\\\test",
                input: "\\test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "Test",
                input: "test",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "Test",
                input: "test",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^Test$",
                input: "test",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "^Test$",
                input: "test",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^test*end$",
                input: "testmiddleend",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*middle*",
                input: "startmiddleend",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\^start*end\\$",
                input: "^startmiddleend$",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*",
                input: "anything",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^$",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\x",
                input: "\\x",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\z",
                input: "\\z",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test\\",
                input: "test\\",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "simple",
                input: "simple",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "sim*",
                input: "simple",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^simple$",
                input: "simple",
                cs: true,
                exp: true,
            },
        ]);
    }

    // ----------------------------------------------------------------------
    // test_char_classification.c (179 cases, 8 sections; skip 1 NULL)
    // The firmware generates cases by looping over char arrays; we mirror that
    // by looping over &[u8] arrays and building a 1-byte &str input.
    // ----------------------------------------------------------------------

    /// Assert parity for a single-byte input: `pattern_match(pat, byte, cs) == exp`.
    fn assert_byte_parity(pat: &str, bytes: &[u8], cs: bool, exp: bool) {
        for &b in bytes {
            let buf = [b];
            let input = std::str::from_utf8(&buf).unwrap();
            let got = pattern_match(pat, input, cs);
            assert!(
                got == exp,
                "parity FAIL byte 0x{:02X} pattern_match({:?}, {:?}, cs={}) = {}, expected {}",
                b,
                pat,
                input,
                cs,
                got,
                exp
            );
        }
    }

    #[test]
    fn test_parity_charclass_digit() {
        // \d: digits 0-9 match (10), 14 non-digits don't.
        assert_byte_parity("\\d", b"0123456789", true, true);
        // non-digits: letters, space, punct, whitespace
        assert_byte_parity("\\d", b"azAZ !_\t\n\r\x0C\x0B/:", true, false);
    }

    #[test]
    fn test_parity_charclass_nondigit() {
        // \D: representative samples (inverse of \d).
        assert_parity(&[
            Case {
                pattern: "\\D",
                input: "5",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\D",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\D",
                input: "_",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\D",
                input: " ",
                cs: true,
                exp: true,
            },
        ]);
    }

    #[test]
    fn test_parity_charclass_word() {
        // \w: a-z, A-Z, 0-9 (62) + underscore match; 33 non-word don't.
        let mut word: Vec<u8> = Vec::new();
        word.extend(b"abcdefghijklmnopqrstuvwxyz");
        word.extend(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ");
        word.extend(b"0123456789");
        word.push(b'_');
        assert_byte_parity("\\w", &word, true, true);
        // non-word chars
        assert_byte_parity(
            "\\w",
            b" !@#$%^&*()-+=\t\n\r\x0C\x0B/:;<>?[]{}|\\`~",
            true,
            false,
        );
    }

    #[test]
    fn test_parity_charclass_nonword() {
        // \W: representative samples (inverse of \w).
        assert_parity(&[
            Case {
                pattern: "\\W",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\W",
                input: "_",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\W",
                input: "7",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\W",
                input: " ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\W",
                input: "!",
                cs: true,
                exp: true,
            },
        ]);
    }

    #[test]
    fn test_parity_charclass_space() {
        // \s: space, \t, \n, \r, \f(0x0C), \v(0x0B) match; 20 non-space don't.
        assert_byte_parity("\\s", b" \t\n\r\x0C\x0B", true, true);
        assert_byte_parity("\\s", b"azAZ09_!@#$%^&*()-+=", true, false);
    }

    #[test]
    fn test_parity_charclass_nonspace() {
        // \S: representative samples (inverse of \s).
        assert_parity(&[
            Case {
                pattern: "\\S",
                input: " ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\S",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\S",
                input: "_",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\S",
                input: "!",
                cs: true,
                exp: true,
            },
        ]);
    }

    #[test]
    fn test_parity_charclass_wordboundary() {
        assert_parity(&[
            // SKIPPED (G3): firmware `\bword\b` vs NULL — Rust &str is never null.
            Case {
                pattern: "\\bword\\b",
                input: "",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\bword\\b",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\bword\\b",
                input: "a word here",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "\\bword\\b",
                input: "awordhere",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\bword",
                input: "word here",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "word\\b",
                input: "a word here",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "\\bhello",
                input: "hello world",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "world\\b",
                input: "hello world",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\Bword",
                input: "aword",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\Bword",
                input: "word",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "word\\B",
                input: "wordhere",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "a\\Bb",
                input: "ab",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "!\\B-",
                input: "!-",
                cs: true,
                exp: true,
            },
        ]);
    }

    #[test]
    fn test_parity_charclass_anchored() {
        assert_parity(&[
            Case {
                pattern: "^\\d$",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\d$",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\w$",
                input: "_",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\w$",
                input: " ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\s$",
                input: " ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\S$",
                input: "a",
                cs: true,
                exp: true,
            },
        ]);
    }

    // ----------------------------------------------------------------------
    // test_metachar_verification.c (24 cases, 1 fn — printf_helper)
    // Read the EXPECTED bool (PASS=expected-true, FAIL=expected-false).
    // ----------------------------------------------------------------------

    #[test]
    fn test_parity_metachar_verification() {
        assert_parity(&[
            // \d / \D
            Case {
                pattern: "\\d",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\D",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\D",
                input: "5",
                cs: true,
                exp: false,
            },
            // multiple \d and with operators
            Case {
                pattern: "\\d\\d",
                input: "42",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\d$",
                input: "7",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d*",
                input: "123",
                cs: true,
                exp: true,
            },
            // \w / \W
            Case {
                pattern: "\\w",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w",
                input: "Z",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w",
                input: "_",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w",
                input: " ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\W",
                input: " ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\W",
                input: "a",
                cs: true,
                exp: false,
            },
            // \s / \S
            Case {
                pattern: "\\s",
                input: " ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\s",
                input: "\t",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\s",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\S",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\S",
                input: " ",
                cs: true,
                exp: false,
            },
            // case sensitivity
            Case {
                pattern: "\\w",
                input: "A",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w",
                input: "a",
                cs: true,
                exp: true,
            },
            // with operators
            Case {
                pattern: "^\\d\\w$",
                input: "5a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\s*",
                input: "   ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w*\\d",
                input: "abc123",
                cs: true,
                exp: true,
            },
        ]);
    }

    // ----------------------------------------------------------------------
    // test_word_boundary_basic.c (74 cases, 4 fns)
    // ----------------------------------------------------------------------

    #[test]
    fn test_parity_wbb_basic() {
        assert_parity(&[
            // \b at start
            Case {
                pattern: "\\bword",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\bword",
                input: "aword",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\bword",
                input: " word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\bword",
                input: ".word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\bword",
                input: "123word",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\bword",
                input: "_word",
                cs: true,
                exp: false,
            },
            // \b at end
            Case {
                pattern: "word\\b",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "word\\b",
                input: "worda",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "word\\b",
                input: "word ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "word\\b",
                input: "word.",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "word\\b",
                input: "word123",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "word\\b",
                input: "word_",
                cs: true,
                exp: false,
            },
            // \b in middle
            Case {
                pattern: "\\btest\\b",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\btest\\b",
                input: "testing",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\btest\\b",
                input: "pretest",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\btest\\b",
                input: "pretesting",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\btest\\b",
                input: " test ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\btest\\b",
                input: ".test.",
                cs: true,
                exp: true,
            },
            // \B
            Case {
                pattern: "\\Bord",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\Bord",
                input: "ord",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\Bord",
                input: " ord",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "wor\\B",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "wor\\B",
                input: "wor",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "wor\\B",
                input: "wor ",
                cs: true,
                exp: false,
            },
            // edge at string boundaries
            Case {
                pattern: "\\b",
                input: "",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\B",
                input: "",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\ba",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "a\\b",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\Ba",
                input: "ba",
                cs: true,
                exp: true,
            },
            // mixed
            Case {
                pattern: "\\btest\\b",
                input: "test123",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\btest\\b",
                input: "test_var",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\btest\\b",
                input: "test-var",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\btest\\b",
                input: "test.method",
                cs: true,
                exp: true,
            },
            // case sensitivity
            Case {
                pattern: "\\bTest\\b",
                input: "test",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "\\bTest\\b",
                input: "test",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\bTEST\\b",
                input: "test",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "\\bTEST\\b",
                input: "test",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_wbb_with_anchors() {
        assert_parity(&[
            Case {
                pattern: "^\\bword",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\bword",
                input: " word",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\Bord",
                input: "word",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "word\\b$",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "word\\b$",
                input: "word ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "wor\\B$",
                input: "word",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\btest\\b$",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\btest\\b$",
                input: " test",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\btest\\b$",
                input: "test ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\btest\\b$",
                input: "testing",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\b\\w+\\b$",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\b\\w+\\b$",
                input: "word123",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\b\\w+\\b$",
                input: "word-test",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_wbb_with_wildcards() {
        assert_parity(&[
            Case {
                pattern: "\\b*test",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*test",
                input: "pretest",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test*\\b",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test*\\b",
                input: "testing",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*word*\\b",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*word*\\b",
                input: "myword",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*word*\\b",
                input: "wordy",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*word*\\b",
                input: "mywordy",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\B*ord",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\B*ord",
                input: "ord",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "wor*\\B",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "wor*\\B",
                input: "wor",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_wbb_multiple() {
        assert_parity(&[
            Case {
                pattern: "\\b\\b",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\b",
                input: " a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\B\\B",
                input: "ab",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\B\\B",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b\\Bord",
                input: "word",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b\\Bord",
                input: "ord",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "wor\\B\\b",
                input: "word",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "wor\\B\\b",
                input: "wor",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b\\w\\b",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\w\\b",
                input: "ab",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b\\d\\b",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\s\\b",
                input: " ",
                cs: true,
                exp: false,
            },
        ]);
    }

    // ----------------------------------------------------------------------
    // test_word_boundary_integration.c (189 cases, 5 fns)
    // ----------------------------------------------------------------------

    #[test]
    fn test_parity_wbi_anchors() {
        assert_parity(&[
            // ^\b
            Case {
                pattern: "^\\bword",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\bword",
                input: "word123",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\bword",
                input: " word",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\bword",
                input: "aword",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\btest",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\btest",
                input: "testing",
                cs: true,
                exp: true,
            },
            // \b$
            Case {
                pattern: "word\\b$",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "word\\b$",
                input: "123word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "word\\b$",
                input: "word ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "word\\b$",
                input: "worda",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "test\\b$",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test\\b$",
                input: "pretest",
                cs: true,
                exp: true,
            },
            // ^\b...\b$
            Case {
                pattern: "^\\btest\\b$",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\btest\\b$",
                input: " test",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\btest\\b$",
                input: "test ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\btest\\b$",
                input: "testing",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\btest\\b$",
                input: "pretest",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\btest\\b$",
                input: "pretesting",
                cs: true,
                exp: false,
            },
            // \B anchors
            Case {
                pattern: "^\\Bord",
                input: "word",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\Bord",
                input: "sword",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "wor\\B$",
                input: "word",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "wor\\B$",
                input: "words",
                cs: true,
                exp: false,
            },
            // simple anchored word patterns
            Case {
                pattern: "^\\b\\w\\b$",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\b\\w\\w\\b$",
                input: "ab",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\b\\w\\w\\w\\b$",
                input: "abc",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\b\\w\\w\\w\\w\\b$",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\b\\w\\w\\w\\w\\b$",
                input: " word",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\b\\w\\w\\w\\w\\b$",
                input: "word ",
                cs: true,
                exp: false,
            },
            // edge cases with anchors
            Case {
                pattern: "^\\b",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\b",
                input: " word",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\b",
                input: "",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b$",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b$",
                input: "word ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b$",
                input: "",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_wbi_wildcards() {
        assert_parity(&[
            Case {
                pattern: "\\b*test",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*test",
                input: "pretest",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*test",
                input: " test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*test",
                input: "atest",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test*\\b",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test*\\b",
                input: "testing",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test*\\b",
                input: "test ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test*\\b",
                input: "testa",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*word*\\b",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*word*\\b",
                input: "myword",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*word*\\b",
                input: "wordy",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*word*\\b",
                input: "mywordy",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*word*\\b",
                input: " word ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*word*\\b",
                input: "password",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\B*ord",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\B*ord",
                input: "ord",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\B*ord",
                input: " ord",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "wor*\\B",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "wor*\\B",
                input: "wor",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "wor*\\B",
                input: "wor ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*\\w*\\b",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*\\w*\\b",
                input: "word123",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*\\w*\\b",
                input: " word ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*\\w*\\b",
                input: "word-test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*test*case*\\b",
                input: "testcase",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*test*case*\\b",
                input: "mytestcase",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*test*case*\\b",
                input: "testcasemy",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*test*case*\\b",
                input: "mytestcasemy",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*",
                input: " word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b*",
                input: "",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "*\\b",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\b",
                input: "word ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\b",
                input: "",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_wbi_metacharacters() {
        assert_parity(&[
            // \b\d
            Case {
                pattern: "\\b\\d",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\d",
                input: "a5",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b\\d",
                input: " 5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\d",
                input: ".5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d\\b",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d\\b",
                input: "5a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\d\\b",
                input: "5 ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d\\b",
                input: "5.",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\d\\b",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\d\\b",
                input: "a5",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b\\d\\b",
                input: "5a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b\\d\\b",
                input: " 5 ",
                cs: true,
                exp: true,
            },
            // \b\D
            Case {
                pattern: "\\b\\D",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\D",
                input: "5a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b\\D",
                input: " a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\D\\b",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\D\\b",
                input: "a5",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\D\\b",
                input: "a ",
                cs: true,
                exp: true,
            },
            // \b\w
            Case {
                pattern: "\\b\\w",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\w",
                input: " a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\w",
                input: "ba",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w\\b",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w\\b",
                input: "a ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w\\b",
                input: "ab",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\w\\b",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\w\\b",
                input: "_",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\w\\b",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\w\\b",
                input: "ab",
                cs: true,
                exp: false,
            },
            // \b\W
            Case {
                pattern: "\\b\\W",
                input: " ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b\\W",
                input: "a ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\W\\b",
                input: " ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\W\\b",
                input: " a",
                cs: true,
                exp: true,
            },
            // \b\s
            Case {
                pattern: "\\b\\s",
                input: " ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\s\\b",
                input: " ",
                cs: true,
                exp: false,
            },
            // \b\S
            Case {
                pattern: "\\b\\S",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\S",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\S",
                input: " a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\S\\b",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\S\\b",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\S\\b",
                input: "a ",
                cs: true,
                exp: true,
            },
            // complex
            Case {
                pattern: "\\b\\w\\d",
                input: "a5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\w\\d",
                input: " a5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\w\\d",
                input: "ba5",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\d\\w\\b",
                input: "5a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d\\w\\b",
                input: "5a ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d\\w\\b",
                input: "5ab",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b\\d\\d\\d\\b",
                input: "123",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\w\\w\\w\\w\\b",
                input: "word",
                cs: true,
                exp: true,
            },
            // \B\w
            Case {
                pattern: "\\B\\w",
                input: "ab",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\B\\w",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\B\\w",
                input: " a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\w\\B",
                input: "ab",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w\\B",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\w\\B",
                input: "a ",
                cs: true,
                exp: false,
            },
            // edge cases
            Case {
                pattern: "\\b\\w*\\b",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\d*\\b",
                input: "123",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\S*\\b",
                input: "word",
                cs: true,
                exp: true,
            },
        ]);
    }

    #[test]
    fn test_parity_wbi_edge_cases() {
        assert_parity(&[
            // empty
            Case {
                pattern: "\\b",
                input: "",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\B",
                input: "",
                cs: true,
                exp: false,
            },
            // single char
            Case {
                pattern: "\\ba",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "a\\b",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\ba\\b",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b5",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "5\\b",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b5\\b",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b_",
                input: "_",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "_\\b",
                input: "_",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b_\\b",
                input: "_",
                cs: true,
                exp: true,
            },
            // \B single
            Case {
                pattern: "\\Ba",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "a\\B",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\B5",
                input: "5",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "5\\B",
                input: "5",
                cs: true,
                exp: false,
            },
            // two char word boundaries
            Case {
                pattern: "\\bab",
                input: "ab",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "ab\\b",
                input: "ab",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "a\\bb",
                input: "ab",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b12",
                input: "12",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "12\\b",
                input: "12",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "1\\b2",
                input: "12",
                cs: true,
                exp: false,
            },
            // two char non-word boundaries
            Case {
                pattern: "\\Bab",
                input: "ab",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "ab\\B",
                input: "ab",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "a\\Bb",
                input: "ab",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "1\\B2",
                input: "12",
                cs: true,
                exp: true,
            },
            // mixed word/non-word at boundaries
            Case {
                pattern: "\\b ",
                input: " ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b.",
                input: ".",
                cs: true,
                exp: false,
            },
            Case {
                pattern: " \\b",
                input: " ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: ".\\b",
                input: ".",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\B ",
                input: " ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\B.",
                input: ".",
                cs: true,
                exp: true,
            },
            Case {
                pattern: " \\B",
                input: " ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: ".\\B",
                input: ".",
                cs: true,
                exp: true,
            },
            // transitions
            Case {
                pattern: "\\ba ",
                input: "a ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: " a\\b",
                input: " a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b5.",
                input: "5.",
                cs: true,
                exp: true,
            },
            Case {
                pattern: ".5\\b",
                input: ".5",
                cs: true,
                exp: true,
            },
            // complex edge
            Case {
                pattern: "\\b\\b",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\b",
                input: " ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\B\\B",
                input: "ab",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\B\\B",
                input: "a",
                cs: true,
                exp: false,
            },
            // special chars
            Case {
                pattern: "\\b@",
                input: "@",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "@\\b",
                input: "@",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\B@",
                input: "@",
                cs: true,
                exp: true,
            },
            // very short
            Case {
                pattern: "\\b",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b",
                input: " ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\B",
                input: "ab",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\B",
                input: "a",
                cs: true,
                exp: false,
            },
            // boundaries at position 0 and end
            Case {
                pattern: "\\ba*",
                input: "abc",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*a\\b",
                input: "cba",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\Ba*",
                input: "abc",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "*a\\B",
                input: "cba",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_wbi_case_sensitivity() {
        assert_parity(&[
            Case {
                pattern: "^\\bTest\\b$",
                input: "test",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "^\\bTest\\b$",
                input: "test",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\bTEST\\b$",
                input: "test",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "^\\bTEST\\b$",
                input: "test",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b*Test*\\b",
                input: "mytest",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "\\b*Test*\\b",
                input: "mytest",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b*TEST*\\b",
                input: "mytest",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "\\b*TEST*\\b",
                input: "mytest",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\bTest\\w",
                input: "testa",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "\\bTest\\w",
                input: "testa",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\w\\bTest",
                input: "atest",
                cs: false,
                exp: false,
            },
            Case {
                pattern: "\\w\\bTest",
                input: "atest",
                cs: true,
                exp: false,
            },
        ]);
    }

    // ----------------------------------------------------------------------
    // test_comprehensive_integration.c (130 assertion cases, 8 fns)
    // G5: helper order is (pattern, str, EXPECTED, case_sensitive) — normalized.
    // (test_performance_complex + test_memory_management_complex = 3500
    //  NON-assertion calls — out of scope for bool parity.)
    // ----------------------------------------------------------------------

    #[test]
    fn test_parity_ci_complex_metacharacter_combinations() {
        assert_parity(&[
            Case {
                pattern: "\\d\\w\\s",
                input: "5a ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d\\w\\s",
                input: "9_ \t",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d\\w\\s",
                input: "0Z\n",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d\\w\\s",
                input: "a5 ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\d\\w\\s",
                input: "5  ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\d\\w\\s",
                input: "5a5",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\D\\W\\S",
                input: "a!x",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\D\\W\\S",
                input: "x@y",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\D\\W\\S",
                input: "!#$",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\D\\W\\S",
                input: "5!x",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\D\\W\\S",
                input: "a5x",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\D\\W\\S",
                input: "a! ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\d\\W\\s",
                input: "5! ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\D\\w\\S",
                input: "a5x",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w\\D\\s",
                input: "a!\t",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\W\\d\\S",
                input: "!5x",
                cs: true,
                exp: true,
            },
        ]);
    }

    #[test]
    fn test_parity_ci_metacharacters_with_anchors_complex() {
        assert_parity(&[
            Case {
                pattern: "^\\d\\w\\s",
                input: "5a ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\d\\w\\s",
                input: "x5a ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\d\\w\\s",
                input: "5a extra",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d\\w\\s$",
                input: "5a ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d\\w\\s$",
                input: "pre5a ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d\\w\\s$",
                input: "5a extra",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\d\\w\\s$",
                input: "5a ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\d\\w\\s$",
                input: "x5a ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\d\\w\\s$",
                input: "5a x",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\d\\w\\s$",
                input: "pre5a ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\d\\d\\w\\w\\s\\s$",
                input: "55aa  ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\d\\d\\w\\w\\s\\s$",
                input: "55aa \t",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\d\\d\\w\\w\\s\\s$",
                input: "55aa ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^\\d\\d\\w\\w\\s\\s$",
                input: "55aa   ",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_ci_metacharacters_with_wildcards_complex() {
        assert_parity(&[
            Case {
                pattern: "*\\d",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\d",
                input: "a5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\d",
                input: "hello5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\d",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "*\\w",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\w",
                input: "!a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\w",
                input: "hello",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\w",
                input: "!",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "*\\s",
                input: " ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\s",
                input: "a ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\s",
                input: "hello ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\s",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "test*\\d",
                input: "test5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test*\\d",
                input: "testxyz5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test*\\d",
                input: "test",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "*test\\w",
                input: "testa",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*test\\w",
                input: "xyztest5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*test\\w",
                input: "test",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_ci_word_boundaries_complex() {
        assert_parity(&[
            Case {
                pattern: "\\b\\d\\d\\b",
                input: "55",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\d\\d\\b",
                input: " 55 ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\d\\d\\b",
                input: "a55b",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b\\d\\d\\b",
                input: "hello 55 world",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\w",
                input: "hello",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\w",
                input: " hello",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w\\b",
                input: "hello",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w\\b",
                input: "hello ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\B\\w\\B",
                input: "abc",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\B\\w\\B",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\B\\w\\B",
                input: " a ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b\\w\\w\\w\\b",
                input: "cat",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\w\\w\\w\\b",
                input: " cat ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\w\\w\\w\\b",
                input: "catch",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_ci_dot_metacharacter_complex() {
        assert_parity(&[
            Case {
                pattern: ".\\d",
                input: "a5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: ".\\d",
                input: "x9",
                cs: true,
                exp: true,
            },
            Case {
                pattern: ".\\d",
                input: " 3",
                cs: true,
                exp: true,
            },
            Case {
                pattern: ".\\d",
                input: "\n5",
                cs: true,
                exp: false,
            },
            Case {
                pattern: ".\\d",
                input: "5",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\w.",
                input: "a5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w.",
                input: "z!",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w.",
                input: "5 ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w.",
                input: "5\n",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "...",
                input: "abc",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "...",
                input: "a5!",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "...",
                input: "ab",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "...",
                input: "ab\n",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^.\\d$",
                input: "a5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^.\\d$",
                input: "x9",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^.\\d$",
                input: "\n5",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^.\\d$",
                input: "a55",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "*.",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*.",
                input: "hello",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*.",
                input: "",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "a.",
                input: "ab",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "a.",
                input: "a\n",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_ci_all_features_combined() {
        assert_parity(&[
            Case {
                pattern: "^\\b\\d\\w\\s",
                input: "5a ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\b\\d\\w\\s",
                input: " 5a ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\d\\w\\s$",
                input: "5a ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d\\w\\s$",
                input: "5a x",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "*@*..*",
                input: "user@domain.com",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*@*..*",
                input: "test@example.org",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*@*..*",
                input: "a@b.c",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*@*..*",
                input: "invalid",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\d\\d\\d-\\d\\d\\d-\\d\\d\\d\\d",
                input: "123-456-7890",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d\\d\\d-\\d\\d\\d-\\d\\d\\d\\d",
                input: "555-123-4567",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d\\d\\d-\\d\\d\\d-\\d\\d\\d\\d",
                input: "abc-def-ghij",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\d\\d\\d-\\d\\d\\d-\\d\\d\\d\\d",
                input: "123-456-789",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\d\\w\\s\\D\\W\\S",
                input: "5a !@#",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d\\w\\s\\D\\W\\S",
                input: "9_ x@y",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d\\w\\s\\D\\W\\S",
                input: "5a 5@#",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^.\\d.\\w.$",
                input: "a5b_c",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^.\\d.\\w.$",
                input: "x9y2z",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^.\\d.\\w.$",
                input: "\n5b_c",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_ci_case_sensitivity_complex() {
        assert_parity(&[
            // G5: firmware helper is (pattern, str, EXPECTED, cs). The
            // "Case insensitive complex patterns" block uses cs=true, exp=false
            // (case-sensitive match fails on case mismatch) — ported exactly.
            Case {
                pattern: "^Hello\\s\\w*$",
                input: "hello world",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^Hello\\s\\w*$",
                input: "HELLO WORLD",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^Hello\\s\\w*$",
                input: "HeLLo WoRLd",
                cs: true,
                exp: false,
            },
            // "Case sensitive complex patterns" block: (pattern, str, expected, cs).
            Case {
                pattern: "^Hello\\s\\w*$",
                input: "hello world",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "^Hello\\s\\w*$",
                input: "Hello world",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^Hello\\s\\w*$",
                input: "HELLO WORLD",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "\\w\\d\\s",
                input: "A5 ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w\\d\\s",
                input: "a5 ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\W\\D\\S",
                input: "!a5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\W\\D\\S",
                input: "!A5",
                cs: true,
                exp: true,
            },
        ]);
    }

    #[test]
    fn test_parity_ci_edge_cases_complex() {
        assert_parity(&[
            Case {
                pattern: "",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\d",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\w",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*\\s",
                input: " ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\\\\\d",
                input: "\\5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\^\\$\\*",
                input: "^$*",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b",
                input: "",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\B",
                input: "",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b\\w\\b",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\B\\w\\B",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: ".",
                input: "\n",
                cs: true,
                exp: false,
            },
            Case {
                pattern: ".",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: ".",
                input: "\t",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d\\D",
                input: "5a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w\\W",
                input: "a!",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\s\\S",
                input: " a",
                cs: true,
                exp: true,
            },
        ]);
        // G5: these two cases come from test_comprehensive_integration.c
        // whose helper order is (pattern, str, EXPECTED, cs) — the empty-string
        // \b/\B cases expect FALSE with cs=true (firmware source of truth).
        // Long-pattern case (100 \d then $) ported via owned strings below.
        let mut long_pattern = String::from("^");
        let mut long_input = String::new();
        for _ in 0..100 {
            long_pattern.push_str("\\d");
            long_input.push('5');
        }
        long_pattern.push('$');
        assert!(
            pattern_match(&long_pattern, &long_input, true),
            "long \\d*100 pattern should match"
        );
    }

    // ----------------------------------------------------------------------
    // test_invalid_patterns.c (88 assertion cases, 3 fns) + no-panic property test
    // G4: "\\0"/"\\n"/"\\t" are 2-char literals (backslash+letter), NOT control bytes.
    // (test_comprehensive_error_handling = 920 NON-assertion crash-safety calls →
    //  represented by test_parity_invalid_no_panic below.)
    // ----------------------------------------------------------------------

    #[test]
    fn test_parity_invalid_regex_patterns() {
        assert_parity(&[
            // unmatched brackets -> literal
            Case {
                pattern: "[abc",
                input: "[abc",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "abc]",
                input: "abc]",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "[",
                input: "[",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "]",
                input: "]",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "[]",
                input: "[]",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "[^]",
                input: "[^]",
                cs: true,
                exp: true,
            },
            // unmatched parens -> literal
            Case {
                pattern: "(abc",
                input: "(abc",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "abc)",
                input: "abc)",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "(",
                input: "(",
                cs: true,
                exp: true,
            },
            Case {
                pattern: ")",
                input: ")",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "()",
                input: "()",
                cs: true,
                exp: true,
            },
            // quantifiers
            Case {
                pattern: "a+",
                input: "aaa",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "a?",
                input: "a?",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "a{3}",
                input: "a{3}",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "a{3,5}",
                input: "a{3,5}",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "+",
                input: "+",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "?",
                input: "?",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "{}",
                input: "{}",
                cs: true,
                exp: true,
            },
            // invalid char classes
            Case {
                pattern: "[a-",
                input: "[a-",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "[z-a]",
                input: "[z-a]",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "[-a]",
                input: "[-a]",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "[a-]",
                input: "[a-]",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "[^",
                input: "[^",
                cs: true,
                exp: true,
            },
            // invalid escapes — G4: "\\0" etc. are 2-char literals (backslash+letter)
            Case {
                pattern: "\\",
                input: "\\",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\q",
                input: "\\q",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\0",
                input: "\\0",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\9",
                input: "\\9",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\n",
                input: "\\n",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\t",
                input: "\\t",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\r",
                input: "\\r",
                cs: true,
                exp: true,
            },
            // mixed invalid constructs
            Case {
                pattern: "[abc)+",
                input: "[abc)+",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\[abc\\]",
                input: "[abc]",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\(abc\\)",
                input: "(abc)",
                cs: true,
                exp: false,
            },
            // parsing confusion
            Case {
                pattern: "^^",
                input: "^",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "$$",
                input: "$",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "**",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "***",
                input: "anything",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "|",
                input: "|",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "a|b",
                input: "a|b",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\|",
                input: "|",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_invalid_boundary_conditions() {
        assert_parity(&[
            // empty strings
            Case {
                pattern: "",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "a",
                input: "",
                cs: true,
                exp: false,
            },
            // single chars
            Case {
                pattern: "a",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "a",
                input: "b",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "a",
                input: "aa",
                cs: true,
                exp: true,
            },
            // whitespace
            Case {
                pattern: " ",
                input: " ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\t",
                input: "\t",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "   ",
                input: "   ",
                cs: true,
                exp: true,
            },
            // special chars
            Case {
                pattern: "@",
                input: "@",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "#",
                input: "#",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "%",
                input: "%",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "&",
                input: "&",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "!",
                input: "!",
                cs: true,
                exp: true,
            },
            // case sensitivity boundaries
            Case {
                pattern: "A",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "A",
                input: "a",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "aB",
                input: "Ab",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "aB",
                input: "Ab",
                cs: false,
                exp: true,
            },
            // very short patterns
            Case {
                pattern: ".",
                input: "x",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "$",
                input: "",
                cs: true,
                exp: true,
            },
            // anchor edge cases
            Case {
                pattern: "^a",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "a$",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^a$",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^a$",
                input: "aa",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^a$",
                input: "",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_invalid_error_conditions() {
        assert_parity(&[
            // multiple wildcards
            Case {
                pattern: "a**",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "**a",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*a*",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "***",
                input: "",
                cs: true,
                exp: true,
            },
            // complex escape sequences
            Case {
                pattern: "\\\\\\\\",
                input: "\\\\",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\\\\\^",
                input: "\\^",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\\\\\$",
                input: "\\$",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\\\\\*",
                input: "\\*",
                cs: true,
                exp: true,
            },
            // combined features
            Case {
                pattern: "^\\d*\\w+$",
                input: "123abc",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\w*\\s+\\d+",
                input: "hello 123",
                cs: true,
                exp: true,
            },
            // parsing ambiguities
            Case {
                pattern: "^*",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "*$",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^*$",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^*$",
                input: "anything",
                cs: true,
                exp: true,
            },
            // stress
            Case {
                pattern: "\\\\\\\\\\\\\\",
                input: "\\\\\\\\",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^^^^^^^^",
                input: "^^^^^^^",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "$$$$$$$$",
                input: "$$$$$$$",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "********",
                input: "",
                cs: true,
                exp: true,
            },
            // mixed valid/invalid
            Case {
                pattern: "valid\\xinvalid",
                input: "valid\\xinvalid",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\dvalid\\qinvalid",
                input: "5valid\\qinvalid",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^valid[invalid$",
                input: "valid[invalid",
                cs: true,
                exp: true,
            },
        ]);
    }

    /// Property test: the firmware's `test_comprehensive_error_handling` crash-safety
    /// loop feeds ~46 "problematic" patterns against 10 inputs and asserts only
    /// that `pattern_match` does NOT crash (the result is discarded). This Rust
    /// analog mirrors that intent: feed the same problematic patterns and assert
    /// no panic. (NULL pattern is skipped — G3, no &str analog.)
    #[test]
    fn test_parity_invalid_no_panic() {
        let problematic_patterns: &[&str] = &[
            "",
            "\\",
            "\\",
            "\\\\\\",
            "^",
            "$",
            "*",
            ".",
            "[",
            "]",
            "(",
            ")",
            "{",
            "}",
            "+",
            "?",
            "|",
            "\\q",
            "\\0",
            "\\9",
            "[abc",
            "abc]",
            "(abc",
            "abc)",
            "a+",
            "a?",
            "a{3}",
            "^^",
            "$$",
            "**",
            "***",
            "\\\\\\^",
            "\\\\\\$",
            "\\\\\\*",
            "^*$",
            "*^*",
            "$*^",
            "test\\",
            "\\test",
            "te\\st",
            "test[invalid",
            "test(invalid",
            "test{invalid",
            "test+invalid",
            "test?invalid",
            "test|invalid",
        ];
        let test_inputs: &[&str] = &[
            "",
            "a",
            "test",
            "^$*\\",
            "[]()+?{}|",
            "normal text",
            "123456789",
            "   \t\n\r   ",
            "MiXeD cAsE",
            "special@#$%^&*()chars",
        ];
        for &pat in problematic_patterns {
            for &inp in test_inputs {
                // case sensitive
                let _ = pattern_match(pat, inp, true);
                // case insensitive
                let _ = pattern_match(pat, inp, false);
            }
        }
        // If we reached here, no panic occurred — the crash-safety property holds.
    }

    // ----------------------------------------------------------------------
    // test_error_handling.c (161 cases, 8 fns; skip 3 NULL + ~8 invalid-UTF-8)
    // ----------------------------------------------------------------------

    #[test]
    fn test_parity_eh_null_pointer_handling() {
        // SKIPPED (G3): firmware `pattern_match(NULL, ...)` returns false, but
        // Rust `&str` is never null — there is no analog. The 3 firmware cases
        // (NULL pattern, NULL input, both NULL) are unrepresentable in safe Rust.
        // (Keeping the fn as a placeholder so the skip is auditable.)
    }

    #[test]
    fn test_parity_eh_invalid_escape_sequences() {
        assert_parity(&[
            Case {
                pattern: "\\x",
                input: "\\x",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\z",
                input: "\\z",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\1",
                input: "\\1",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\@",
                input: "\\@",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\#",
                input: "\\#",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\%",
                input: "\\%",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\&",
                input: "\\&",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\(",
                input: "\\(",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\)",
                input: "\\)",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\+",
                input: "\\+",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\=",
                input: "\\=",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\[",
                input: "\\[",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\]",
                input: "\\]",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\{",
                input: "\\{",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\}",
                input: "\\}",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\|",
                input: "\\|",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\?",
                input: "\\?",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\.",
                input: "\\.",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\,",
                input: "\\,",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\;",
                input: "\\;",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\:",
                input: "\\:",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\\"",
                input: "\\\"",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\'",
                input: "\\'",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\`",
                input: "\\`",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\~",
                input: "\\~",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "pre\\xinvalid",
                input: "pre\\xinvalid",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\ystart",
                input: "\\ystart",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "end\\z",
                input: "end\\z",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\X",
                input: "\\x",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "\\X",
                input: "\\x",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_eh_malformed_patterns() {
        assert_parity(&[
            Case {
                pattern: "test\\",
                input: "test\\",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\",
                input: "\\",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\\\\\",
                input: "\\\\\\",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\\\\\\\",
                input: "\\\\",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\\\\\\\\\",
                input: "\\\\\\\\\\",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\\\\\^",
                input: "\\^",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\\\\\$",
                input: "\\$",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\\\\\*",
                input: "\\*",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "mid^dle",
                input: "mid^dle",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "mid$dle",
                input: "mid$dle",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "mid$dle$",
                input: "mid$dle",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "$",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^$",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^$",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "*",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "**",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "***",
                input: "anything",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\*$",
                input: "*",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\^*\\$",
                input: "^anything$",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\\\*test",
                input: "\\anything",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\\\*test",
                input: "\\test",
                cs: true,
                exp: true,
            },
        ]);
    }

    #[test]
    fn test_parity_eh_long_patterns_and_strings() {
        // Firmware builds these dynamically (400+/1000+ chars). Port via owned
        // Strings + a local owned-variant assertion loop.
        let cases_owned: Vec<(String, String, bool, bool)> = vec![
            // (1) long ^ + 100×"test" + $  vs  100×"test"  -> true
            {
                let mut p = String::from("^");
                let mut s = String::new();
                for _ in 0..100 {
                    p.push_str("test");
                    s.push_str("test");
                }
                p.push('$');
                (p, s, true, true)
            },
            // (2) same pattern vs string + "extra" appended -> false
            {
                let mut p = String::from("^");
                let mut s = String::new();
                for _ in 0..100 {
                    p.push_str("test");
                    s.push_str("test");
                }
                p.push('$');
                s.push_str("extra");
                (p, s, true, false)
            },
            // (3) 50×"a*" + "end"  vs  50×"aaa" + "end"  -> true
            {
                let mut p = String::new();
                let mut s = String::new();
                for _ in 0..50 {
                    p.push_str("a*");
                    s.push_str("aaa");
                }
                p.push_str("end");
                s.push_str("end");
                (p, s, true, true)
            },
            // (4) 500×"\\*"  vs  500×"*"  -> true
            {
                let mut p = String::new();
                let mut s = String::new();
                for _ in 0..500 {
                    p.push_str("\\*");
                    s.push('*');
                }
                (p, s, true, true)
            },
            // (5) ^ + 200×"\\d\\w\\s" + $  vs  200×"5a "  -> true
            {
                let mut p = String::from("^");
                let mut s = String::new();
                for _ in 0..200 {
                    p.push_str("\\d\\w\\s");
                    s.push_str("5a ");
                }
                p.push('$');
                (p, s, true, true)
            },
        ];
        for (i, (pat, inp, cs, exp)) in cases_owned.iter().enumerate() {
            let got = pattern_match(pat, inp, *cs);
            assert!(
                got == *exp,
                "parity FAIL [long #{i}] pattern_match(len_pat={}, len_inp={}, cs={}) = {}, expected {}",
                pat.len(),
                inp.len(),
                cs,
                got,
                exp
            );
        }
    }

    #[test]
    fn test_parity_eh_special_character_edge_cases() {
        assert_parity(&[
            Case {
                pattern: "test",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "café",
                input: "café",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "café",
                input: "CAFÉ",
                cs: true,
                exp: false,
            },
            // SKIPPED (G2): {"test\xFF","test\xFF"} and {"test\xFE","test\xFE"}
            //   — 0xFF/0xFE are invalid UTF-8, cannot be a Rust &str.
            Case {
                pattern: "test\tmore",
                input: "test\tmore",
                cs: true,
                exp: true,
            },
            // SKIPPED (G2): \s/\S/\w/\W/. vs \xFF — invalid UTF-8.
            // \x01 control char IS valid UTF-8:
            Case {
                pattern: ".",
                input: "\x01",
                cs: true,
                exp: true,
            },
            Case {
                pattern: ".",
                input: "\n",
                cs: true,
                exp: false,
            },
            // G6: dot vs \r — description says "match" but expected=false.
            Case {
                pattern: ".",
                input: "\r",
                cs: true,
                exp: false,
            },
            Case {
                pattern: ".",
                input: "\t",
                cs: true,
                exp: true,
            },
        ]);
    }

    #[test]
    fn test_parity_eh_memory_allocation_edge_cases() {
        assert_parity(&[
            Case {
                pattern: "\\^\\$\\*\\\\",
                input: "^$*\\",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d\\w\\s\\D\\W\\S",
                input: "5a 5a ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\^\\^\\^\\^\\^",
                input: "^^^^^",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\*\\*\\*\\*\\*",
                input: "*****",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "pre\\^mid\\*post",
                input: "pre^mid*post",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d*\\w+\\s?",
                input: "5a ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "a",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\a",
                input: "\\a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "simple",
                input: "simple",
                cs: true,
                exp: true,
            },
        ]);
    }

    #[test]
    fn test_parity_eh_word_boundary_edge_cases() {
        assert_parity(&[
            Case {
                pattern: "\\b",
                input: "",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\B",
                input: "",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b",
                input: " ",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\Ba",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\B ",
                input: " ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\bword",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "word\\b",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\bword\\b",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\bword\\b",
                input: " word ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\bword\\b",
                input: "sword",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\bword\\b",
                input: "words",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\Bord",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "w\\Bord",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\Bword",
                input: "word",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "word\\B",
                input: "word",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b\\w+\\b",
                input: "hello",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\w+\\b",
                input: "hello world",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\B\\w\\B",
                input: "hello",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\B\\w\\B",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b_test",
                input: "_test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b123",
                input: "123",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b@test",
                input: "@test",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "test\\b@",
                input: "test@",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\w+\\b\\s+\\b\\w+\\b",
                input: "hello world",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\w\\b\\s\\b\\w\\b",
                input: "a b",
                cs: true,
                exp: true,
            },
        ]);
    }

    #[test]
    fn test_parity_eh_dot_metacharacter_edge_cases() {
        assert_parity(&[
            // basic dot
            Case {
                pattern: ".",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: ".",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: ".",
                input: " ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: ".",
                input: "\t",
                cs: true,
                exp: true,
            },
            // G6: dot vs \r — expected false (firmware locks dot to exclude \r)
            Case {
                pattern: ".",
                input: "\r",
                cs: true,
                exp: false,
            },
            // G1: C "\f"→"\x0C", C "\v"→"\x0B"
            Case {
                pattern: ".",
                input: "\x0C",
                cs: true,
                exp: true,
            },
            Case {
                pattern: ".",
                input: "\x0B",
                cs: true,
                exp: true,
            },
            Case {
                pattern: ".",
                input: "\n",
                cs: true,
                exp: false,
            },
            // special chars
            Case {
                pattern: ".",
                input: "@",
                cs: true,
                exp: true,
            },
            Case {
                pattern: ".",
                input: "#",
                cs: true,
                exp: true,
            },
            Case {
                pattern: ".",
                input: "!",
                cs: true,
                exp: true,
            },
            // SKIPPED (G2): {".","\xFF"} — 0xFF invalid UTF-8.
            Case {
                pattern: ".",
                input: "\x01",
                cs: true,
                exp: true,
            },
            // multiple dots
            Case {
                pattern: "..",
                input: "ab",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "...",
                input: "abc",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "..",
                input: "a\n",
                cs: true,
                exp: false,
            },
            Case {
                pattern: ".",
                input: "",
                cs: true,
                exp: false,
            },
            // dot with anchors
            Case {
                pattern: "^.",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: ".$",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^.$",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^.$",
                input: "ab",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^.$",
                input: "",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "^.$",
                input: "\n",
                cs: true,
                exp: false,
            },
            // dot with wildcards
            Case {
                pattern: ".*",
                input: "anything",
                cs: true,
                exp: true,
            },
            Case {
                pattern: ".*",
                input: "",
                cs: true,
                exp: false,
            },
            Case {
                pattern: ".*",
                input: "with\nnewline",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "a.*b",
                input: "a\nb",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "a.*b",
                input: "axyzb",
                cs: true,
                exp: true,
            },
            // escaped dot
            Case {
                pattern: "\\.",
                input: ".",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\.",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "test\\.txt",
                input: "test.txt",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test\\.txt",
                input: "testxtxt",
                cs: true,
                exp: false,
            },
        ]);
    }

    #[test]
    fn test_parity_eh_complex_error_scenarios() {
        assert_parity(&[
            Case {
                pattern: "\\\\\\^\\$\\*",
                input: "\\^$*",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\d*\\w+\\s?$",
                input: "123abc",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\b\\w*\\B\\s*\\d+",
                input: "hello 123",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "*^test",
                input: "^test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\\\\\\\\\^\\\\\\$",
                input: "\\\\^\\$",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\\\\\d\\\\\\w",
                input: "\\5\\a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "a*a*a*a*",
                input: "aaaa",
                cs: true,
                exp: true,
            },
            Case {
                pattern: ".*.*.*",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\w*\\w*\\w*",
                input: "abc",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\^*\\$",
                input: "^^^$",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\^*$",
                input: "^^^",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\^$",
                input: "^",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\$",
                input: "$",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\b\\b",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\B\\B\\B",
                input: "test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\b\\B",
                input: "test",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\B\\b",
                input: "test",
                cs: true,
                exp: false,
            },
        ]);
    }

    // ----------------------------------------------------------------------
    // Composition test: Pattern::Single delegates to the leaf pattern_match.
    // (Satisfies the contract's "delimiter-aware match_pattern tests" line
    // without duplicating T3.S2's serde/Parts tests. A representative cross-
    // section — the delegation is uniform by construction.)
    // ----------------------------------------------------------------------

    #[test]
    fn test_parity_match_pattern_single_dispatch() {
        // Pattern::Single(p) must delegate to pattern_match(p, app_class, cs)
        // (title is ignored for the Single variant — firmware case A1/A2).
        let cross_section: &[Case] = &[
            Case {
                pattern: "^searchterm",
                input: "searchterm",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^searchterm",
                input: "presearchterm",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "searchterm$",
                input: "searchterm",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^searchterm$",
                input: "searchterm",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^searchterm$",
                input: "searchtermpost",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\d",
                input: "5",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\d",
                input: "a",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\w",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\W",
                input: "!",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\s",
                input: " ",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\S",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\bword\\b",
                input: "word",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "\\bword\\b",
                input: "aword",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "a+",
                input: "aaa",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "a+",
                input: "b",
                cs: true,
                exp: false,
            },
            Case {
                pattern: ".",
                input: "a",
                cs: true,
                exp: true,
            },
            Case {
                pattern: ".",
                input: "\n",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "\\^test",
                input: "^test",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^abc",
                input: "ABC",
                cs: false,
                exp: true,
            },
            Case {
                pattern: "^abc",
                input: "ABC",
                cs: true,
                exp: false,
            },
            Case {
                pattern: "*test",
                input: "pretest",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "test*",
                input: "testing",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "^\\w+@\\w+$",
                input: "user@host",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "",
                input: "",
                cs: true,
                exp: true,
            },
            Case {
                pattern: "",
                input: "x",
                cs: true,
                exp: false,
            },
        ];
        for c in cross_section {
            let via_single = match_pattern(
                &Pattern::Single(c.pattern.into()),
                c.input,
                "IGNORED-title",
                c.cs,
            );
            let via_leaf = pattern_match(c.pattern, c.input, c.cs);
            assert_eq!(
                via_single, via_leaf,
                "Single dispatch diverged from leaf for pattern={:?}",
                c.pattern
            );
            assert_eq!(
                via_single, c.exp,
                "Single dispatch wrong result for pattern={:?}",
                c.pattern
            );
        }
    }

    // ===== END FIRMWARE PARITY CORPUS (P2.M1.T4.S1) =====
}

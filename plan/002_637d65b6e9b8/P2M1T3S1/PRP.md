# PRP — P2.M1.T3.S1: Port `match_with_anchors()` + public `pattern_match()` entry

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This is an **additive edit** to the existing
> module `src/core/pattern.rs` (which already contains S1's `process_escapes` +
> consts, S2's `ParsedPattern` + `parse_pattern`, and the S1-parallel `NfaOp` +
> `nfa_compile`; plus `nfa_addstate`/`nfa_match`/predicates added by **P2.M1.T2.S2**,
> which is being implemented in parallel and treated as a delivered contract). This
> task adds the **anchor-strategy layer** (`match_with_anchors`) and the **public
> entry** (`pattern_match`) — stage 5 of the pipeline. It consumes `nfa_match`
> (T2.S2) + `ParsedPattern`/`parse_pattern` (T1.S2), and is consumed downstream by
> P2.M1.T3.S2 (delimiter-aware matcher) and P3 `rules.rs`. Firmware
> `pattern_match.c` ~lines 225–272 is the single source of truth (PRD §4, §14).

---

## Goal

**Feature Goal**: Port the firmware `match_with_anchors()` (`pattern_match.c:233–256`)
and the public `pattern_match()` (`pattern_match.c:259–272`) to Rust. The anchor
strategy selects the NFA mode (`full_match` true/false) and the offset strategy
(single call vs loop) from the `ParsedPattern` anchor flags, implementing the
four modes exactly: `^…$` exact (one full match from offset 0), `^…` prefix (one
reach-any from offset 0), `…$` suffix (loop offsets, full match from each), and
substring (loop offsets, reach-any from each, with the **empty-core-only-matches-
empty-string** special case). The public entry is `parse → match_with_anchors →
drop` (Rust owns the `Vec`; no `free` analog).

**Deliverable**: additions to `src/core/pattern.rs` (do NOT recreate the file):
1. `pub(crate) fn match_with_anchors(parsed: &ParsedPattern, s: &str, case_sensitive: bool) -> bool`;
2. (optional DRY helper) `fn suffix_or_substring_loop(nfa: &[NfaOp], bytes: &[u8], s: &str, case_sensitive: bool, full_match: bool) -> bool` — or inline the loop in both branches;
3. `pub fn pattern_match(pattern: &str, s: &str, case_sensitive: bool) -> bool`;
4. Mode-A rustdoc on `pattern_match` documenting all supported constructs (`*`, `^`, `$`, `^…$`, `\^ \$ \* \\`, `\d \D \w \W \s \S`, `\b \B`, `.`, `+`) + the anchor/substring semantics, and a `///` on `match_with_anchors` mapping the four modes to their `nfa_match` calls;
5. new `#[test]` fns appended to the existing `#[cfg(test)] mod tests` block, covering ~40 end-to-end parity vectors (the four anchor modes + the empty-core special case + escapes/classes/`\b`), curated from the firmware's 380-row corpus.

**Success Definition**:
- For every `(pattern, input, case_sensitive)` in `research/notes.md` §7 (~40
  vectors), the Rust `pattern_match` returns the firmware-expected bool —
  including the **empty-core substring special case** (`""` vs `"test"` → false)
  and the **`\b`-sees-original-string** linchpin (`\bword` vs `aword` → false).
- `cargo build --bin qmkonnect` compiles clean (no new warnings).
- `cargo test --bin qmkonnect -- --test-threads=1` passes (new `pattern_match`/
  `match_with_anchors` tests AND all prior pattern tests: S1 `process_escapes`,
  S2 `parse_pattern`, S1-parallel `nfa_compile`, T2.S2 `nfa_match`).
- `git diff` touches ONLY `src/core/pattern.rs`. No delimiter-aware
  `match_pattern`, no `Pattern::Single|Parts` enum, no `mod.rs` change — those
  are P2.M1.T3.S2.

## User Persona (if applicable)

**Target User**: Downstream Rust callers in the qmkonnect crate:
- **P2.M1.T3.S2** (delimiter-aware `match_pattern` + `Pattern::Single|Parts`):
  splits a pattern/message on the GS delimiter (`0x1D`) and calls `pattern_match`
  on each half (PRD §4.1).
- **P3.M1 `rules.rs`**: evaluates `rules.toml` layer/callback rules by calling
  `pattern_match(rule_pattern, window_class_or_title, rule.case_sensitive)`
  (PRD §14, ARCH module-responsibilities).

**Use Case**: A host-side rule such as `pattern = "^Firefox$"` against the active
window title `"Firefox"` → `pattern_match("^Firefox$", "Firefox", false)` →
`true` (exact match). Or `pattern = "Slack"` against `"Slack — general"` → `true`
(substring, backward-compatible). Or an empty pattern `""` against `"Firefox"` →
`false` (empty-core special case — an empty unanchored rule is a no-op, NOT a
match-everything).

**Pain Points Addressed**: Completes the public matcher API so host-side
`rules.toml` evaluation (P3) and the delimiter-aware matcher (T3.S2) have a
single, firmware-faithful entry point. A wrong anchor-mode `full_match` flag or a
missing empty-core guard would silently flip match semantics (e.g. an empty rule
matching everything, or a `$` suffix behaving like a substring).

## Why

- **Stage 5 — the last pipeline stage before the matcher is usable.** PRD §14
  mandates the Rust matcher be a *"full-parity port of the firmware
  `pattern_match.c`, not a subset"* with *"the firmware matcher + its test corpus
  the single source of truth for match semantics."* `match_with_anchors` + the
  public `pattern_match` are the anchor-strategy + entry stages atop the NFA
  simulator (T2.S2). The firmware C source (`pattern_match.c` ~lines 225–272) +
  its 380-row test corpus are the spec.
- **The empty-core substring special case is the one parity trap.** Without it,
  `pattern_match("", "anything")` returns true (an empty NFA reaches `Match`
  immediately in reach-any mode) — silently turning an empty unanchored rule into
  "match everything". The firmware deliberately returns `strlen(str)==0`. Pinned
  by `{"","test",true,false}`. The other three anchor modes need NO special case
  (traced in `research/notes.md` §4).
- **Enables P3 (rules) + T3.S2 (delimiter matcher).** Both depend on this public
  entry. Shipping it unblocks the rules-system milestone and the GS-delimiter
  aware matcher.

## What

Add to `src/core/pattern.rs` (APPEND after `nfa_match` [T2.S2's deliverable],
before the `#[cfg(test)] mod tests` block — logical pipeline order:
`… → nfa_match → match_with_anchors → pattern_match`). Three justified
refinements to the firmware (each forced by idiomatic Rust, fully derived in
`research/notes.md` §3):

### REFINEMENT D — fold the two C wrappers into direct `nfa_match` calls
The firmware needs `match_string_with_start` / `match_reaches_end_with_start`
because `nfa_match` is `static`/forward-declared and takes a raw pattern. In
Rust, `nfa_match` is `pub(crate)` and takes the compiled `&[NfaOp]` + a
`full_match` bool (T2.S2 REFINEMENT B), so the wrappers are zero-information
forwarders. **Resolution:** call `nfa_match` directly with `full_match=true`
(exact/suffix) or `false` (prefix/substring). Do NOT port the two wrapper fns
(they would be dead one-line aliases). Analogous to T1.S2 dropping the C
`free_parsed_pattern`.

### REFINEMENT E — compile the core ONCE, simulate at many offsets
The firmware `nfa_match` recompiles the pattern internally (stack-local pool)
every call, so `match_with_anchors`' suffix/substring loops recompile `len+1`
times. Rust's `nfa_compile -> Vec<NfaOp>` heap-allocates; recompiling per offset
is wasteful. **Resolution:** `match_with_anchors` calls `nfa_compile(&parsed.core)`
ONCE, binds `nfa: Vec<NfaOp>`, and passes `&nfa` to every `nfa_match` in the
loops. Semantics identical (nfa_compile is pure); only the call count drops.
Honors T2.S2 REFINEMENT B's compile-once-simulate-many contract.

### REFINEMENT F — iterate UTF-8 char boundaries (`char_indices`), not raw byte offsets
The item spec explicitly requests *"loop `str.char_indices()`"*. The firmware
loops raw byte offsets `0..=strlen`. For ASCII (the entire realistic domain —
window titles/class names/patterns) these are identical. For non-ASCII,
`char_indices` skips UTF-8 continuation bytes, which the ASCII-oriented NFA
essentially never matches anyway — a safe, UTF-8-correct refinement.
**Resolution:** iterate `s.char_indices().map(|(i,_)| i).chain(std::iter::once(bytes.len()))`
— the trailing `bytes.len()` preserves the firmware's inclusive `0..=len` end
(so suffix `$` / tail-empty cases are covered). Parity guarantee: for every ASCII
input this == `0..=len` exactly. (Byte-offset `0..=bytes.len()` is an acceptable
alternative — same corpus results — but the PRP specifies `char_indices` per the
item spec.)

### The functions to implement

1. **`match_with_anchors(parsed, s, case_sensitive) -> bool`** — the four-mode
   anchor strategy (§2 of notes; the verified skeleton is in §6 / the Blueprint).
   `pub(crate)` (internal helper; lets the in-module `mod tests` unit-test each
   mode directly via `use super::*`).
2. **(optional) `suffix_or_substring_loop(...)`** — the shared offset loop for
   suffix + substring modes (DRY sugar for the `char_indices + chain(once(len))`
   iteration). An implementer may inline it in both branches instead.
3. **`pattern_match(pattern, s, case_sensitive) -> bool`** — the public entry:
   `let parsed = parse_pattern(pattern); match_with_anchors(&parsed, s, case_sensitive)`
   (the `parsed` drops automatically — no `free` analog). `pub` (the module's
   public API surface).
4. **Rustdoc** (Mode A) on `pattern_match` (the construct table) + `match_with_anchors`
   (the four-mode → `nfa_match` mapping + the refinements).
5. **Unit tests** appended to the existing `mod tests`.

### Success Criteria
- [ ] `pub fn pattern_match(pattern: &str, s: &str, case_sensitive: bool) -> bool` exists.
- [ ] `pub(crate) fn match_with_anchors(parsed: &ParsedPattern, s: &str, case_sensitive: bool) -> bool` exists.
- [ ] Every row of `research/notes.md` §7 (~40 vectors) passes end-to-end via `pattern_match`.
- [ ] **Empty-core special case** (GOTCHA-A): `pattern_match("", "test", true)` == `false` AND `pattern_match("", "", true)` == `true`.
- [ ] **Four anchor modes correct** (GOTCHA-F): `^…$` exact, `^` prefix, `$` suffix, substring — each with the right `full_match` + offset strategy (the §7.1–7.4 tables).
- [ ] **`\b` linchpin end-to-end**: `pattern_match("\\bword", "aword", true)` == `false` (substring loop + original-string `\b` compose).
- [ ] **Case sensitivity**: `abc` vs `ABC` matches iff `!case_sensitive` (across all modes).
- [ ] No NULL guard, no `free` analog, no wrapper fns ported (GOTCHA-D/E).
- [ ] No new deps; no `unsafe`; no `static`; pure stdlib + the in-module T1.S2/T2.S2 API.
- [ ] `cargo build --bin qmkonnect` clean; `cargo test --bin qmkonnect -- --test-threads=1` green.
- [ ] `git diff` touches only `src/core/pattern.rs`.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge of this codebase can implement this
using only this PRP + the firmware repo, because (a) the ENTIRE C source for both
functions being ported (`match_with_anchors`, `pattern_match`) AND the two C
wrapper forwarders they call is reproduced VERBATIM in `research/notes.md` §1,
(b) the three refinements (each forced by idiomatic Rust) are fully derived in
§3 with the four-mode → `nfa_match` mapping table in §2, (c) the empty-core
substring special case is traced through all four modes in §4 (proving only
substring needs the guard), (d) a verified Rust skeleton for `match_with_anchors`
+ `pattern_match` + the optional loop helper is given in §6 (mirrored in the
Implementation Blueprint), (e) ~40 end-to-end parity vectors curated from the
firmware's 380-row corpus are provided as the test contract (§7), grouped by
anchor mode + the empty special case + escapes/classes/`\b`, (f) the 10 gotchas
are enumerated with their pinning test rows, (g) the upstream T2.S2 contract
(`nfa_match` signature + the compile-once REFINEMENT B) and the downstream
T3.S2/P3 contracts (how they call `pattern_match`) are both explicit. See
`research/notes.md`.

### Documentation & References

```yaml
# MUST READ — the canonical C source (single source of truth, PRD §14)
- file: /home/dustin/projects/qmk-notifier/pattern_match.c
  why: "lines ~225-272 are the functions being ported, verbatim: match_with_anchors
        (233-256) with its four-mode if/else + the empty-core substring guard, and
        the public pattern_match (259-272) NULL-guard -> parse -> match -> free.
        The two wrapper forwarders (617-628) it calls are zero-information aliases
        for full_match=false/true. Reproduced in research/notes.md §1."
  pattern: "match_with_anchors: start+end -> reaches_end (full); start-only ->
            string_with_start (reach-any) from offset 0; end-only -> loop offsets
            reaches_end; else -> empty-core guard then loop offsets string_with_start.
            pattern_match: parse_pattern -> match_with_anchors -> free_parsed_pattern."
  gotcha: "the two wrapper fns exist ONLY because nfa_match is static + takes a raw
           pattern (recompiles internally). In Rust they collapse into direct
           nfa_match(.., full_match) calls (REFINEMENT D). The NULL guard + free are
           dead in Rust (GOTCHA-D). The core is compiled ONCE, not per offset
           (REFINEMENT E)."

# MUST READ — the file THIS task edits (S1/S2/S1-parallel/T2.S2 already present)
- file: src/core/pattern.rs
  why: "already has: pub(crate) const ESC_CARET..GLOB_STAR (0x01-0x0E, 0x2A),
        pub(crate) fn process_escapes, pub(crate) struct ParsedPattern { core,
        start_anchored, end_anchored } + parse_pattern, pub(crate) enum NfaOp +
        nfa_compile, AND (from T2.S2, treated as delivered) pub(crate) fn nfa_match(
        states: &[NfaOp], string: &[u8], start: usize, case_sensitive: bool,
        full_match: bool) -> bool + nfa_addstate/nfa_has_match/predicates, and
        #[cfg(test)] mod tests { use super::*; ... }. APPEND match_with_anchors +
        pattern_match (+ optional loop helper) + new tests HERE, AFTER nfa_match
        and BEFORE mod tests. Do NOT recreate."
  pattern: "tests grouped with `// --- header ---` comments; assert bool results
            via pattern_match (end-to-end) and/or match_with_anchors (per-mode).
            Mirror the existing style. The file carries #![allow(dead_code)]
            (API shipped ahead of consumers); the new pub pattern_match is the
            first item whose visibility is `pub` (not pub(crate))."
  gotcha: "PLACEMENT: match_with_anchors/pattern_match go AFTER nfa_match (T2.S2's
           deliverable) and BEFORE mod tests. If T2.S2's additions are not yet in
           the file when you start (parallel work), place yours immediately after
           nfa_compile and the implementer reconciles once T2.S2 lands — the
           pipeline order ... -> nfa_compile -> nfa_match -> match_with_anchors
           -> pattern_match is what matters, not exact line numbers."

# MUST READ — the verified Rust skeleton + the parity table (THIS task's contract)
- file: plan/002_637d65b6e9b8/P2M1T3S1/research/notes.md
  why: "§1 reproduces match_with_anchors + pattern_match + the two wrappers VERBATIM.
        §2 is the four-mode -> nfa_match mapping table. §3 derives REFINEMENTS D/E/F.
        §4 traces the empty-core special case through all four modes (proving only
        substring needs it). §5 details REFINEMENT F (char_indices). §6 is the
        verified Rust skeleton. §7 lists ~40 end-to-end parity vectors (the test
        contract). §8 enumerates 10 gotchas with pinning rows."
  section: "## 6. Verified Rust skeleton" and "## 7. End-to-end parity vectors" and
           "## 8. Gotchas"

# MUST READ — the upstream T2.S2 contract (the nfa_match this task consumes)
- file: plan/002_637d65b6e9b8/P2M1T2S2/PRP.md
  why: "fixes the Rust nfa_match signature — pub(crate) fn nfa_match(states:
        &[NfaOp], string: &[u8], start: usize, case_sensitive: bool, full_match:
        bool) -> bool — via REFINEMENT A (start offset, so \\b/\\B see the original
        string at an absolute offset) + REFINEMENT B (compiled &[NfaOp], so the
        caller compiles ONCE — this task's REFINEMENT E builds on it) + REFINEMENT C
        (Vec<u32> generation tags). The four-mode -> nfa_match mapping in THIS
        task's notes §2 is the consumption pattern T2.S2's PRP anticipated."
  section: "## What" (REFINEMENT A/B + the DOWNSTREAM CONSUMERS note showing how
           match_with_anchors calls nfa_match in its four offset loops)

# MUST READ — the upstream T1.S2 contract (ParsedPattern + parse_pattern)
- file: plan/002_637d65b6e9b8/P2M1T1S2/PRP.md
  why: "defines ParsedPattern { core: Vec<u8>, start_anchored: bool, end_anchored:
        bool } + parse_pattern(&str) -> ParsedPattern (complete in-tree). The two
        anchor flags drive match_with_anchors' mode selection; `core` is the
        placeholder-byte stream compiled by nfa_compile. Confirms parse_pattern is
        REUSED verbatim by pattern_match (GOTCHA-J — do not reimplement parsing)."
  section: "## What" (ParsedPattern struct + parse_pattern) and "### Integration
           Points" (DOWNSTREAM CONSUMERS lists match_with_anchors as P2.M1.T3.S1)

# MUST READ — the firmware end-to-end corpus (the source of the test contract)
- file: /home/dustin/projects/qmk-notifier/test_pattern_match.c
  why: "the 380-row end-to-end corpus exercising pattern_match across all 17 test
        functions (start/end/full anchor, anchors+wildcards, backward-compat
        substring, case sensitivity, edge cases incl. empty-pattern, metacharacters
        with anchors). The ~40 curated vectors in notes §7 are drawn from
        test_start_anchor / test_end_anchor / test_full_anchor / test_edge_cases /
        test_backward_compatibility / test_metacharacters_with_anchors. The FULL
        corpus is ported en masse in P2.M1.T4.S1; this task ports a representative
        subset pinning every match_with_anchors branch."
  section: "test_edge_cases()" (the empty-pattern special case) and
           "test_backward_compatibility()" (substring) and "test_full_anchor()"

# MUST READ — QMKonnect-side architecture contract (cross-repo)
- file: plan/002_637d65b6e9b8/architecture/external_deps.md
  why: "§3 'Pattern Matcher' point 6 is the cross-repo contract for match_with_anchors:
        ^...$ -> one full match from offset 0; ^... -> one reach-any from offset 0;
        ...$ -> loop offsets, full match from each; ... -> loop offsets, reach-any
        from each. Confirms the delimiter-aware match_pattern (T3.S2) is a LATER
        subtask that calls pattern_match, NOT this task. Point on nfa_match's
        full_match flag corroborates the four-mode mapping."
  section: "## 3. Pattern Matcher" (point 6: match_with_anchors anchor strategy)

# MUST READ — PRD selectors that scoped this work
- url: spec/PRD.md (heading h2.74 "Pattern-Matching Syntax (pattern_match.c)")
  why: "the construct table (*, ^, $, ^…$, escapes, \\d \\D \\w \\W \\s \\S, \\b \\B, .)
        is what pattern_match's rustdoc must document (Mode A). 'No anchors =>
        substring match (backward-compatible). Case sensitivity is per-row.'"
- url: spec/PRD.md (heading h2.92 "Appendix — File Layout & Pattern Subset")
  why: "mandates src/core/pattern.rs as 'full-parity matcher (ported from firmware)',
        Thompson NFA, case_sensitive per rule, firmware + its test corpus as the
        single source of truth for match semantics."

# Reference — existing Rust test conventions in THIS repo
- file: src/core/types.rs
  why: "shows #[derive(Debug, PartialEq)] + inline #[cfg(test)] mod tests with
        assert_eq!. The pattern.rs mod tests already follows this; mirror it for
        the new pattern_match tests (use assert! for bool results)."
  pattern: "#[cfg(test)] mod tests { use super::*; ... } with `// --- header ---` groups"
```

### Current Codebase tree (qmkonnect, relevant subset)

```bash
src/
  main.rs                 # CLI entry (unchanged)
  core/
    mod.rs                # Config + helpers; ALREADY has `pub mod pattern;` (S1) — DO NOT TOUCH
    pattern.rs            # S1: process_escapes+consts; S2: ParsedPattern+parse_pattern;
                            #             NfaOp+nfa_compile; T2.S2: nfa_addstate/nfa_match/predicates
                            #             + mod tests
                            #   ← EDIT THIS FILE (additive: + match_with_anchors, + pattern_match,
                            #                     + optional loop helper, + tests)
    notifier.rs           # Notifier trait, debouncer, tests (unchanged)
    types.rs              # WindowInfo (unchanged) — struct/test style reference
  platforms/              # per-OS window monitors (unchanged)
  tray.rs / linux_tray.rs # tray UI (unchanged)
```

### Desired Codebase tree (files this task changes)

```bash
src/
  core/
    pattern.rs            # MODIFIED (additive) — + pub(crate) fn match_with_anchors,
                            #                     + pub fn pattern_match, + optional loop helper,
                            #                     + tests
    # mod.rs UNCHANGED (module already registered by S1)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (GOTCHA-A — empty-core substring special case): the single parity trap.
//   In substring mode (NO anchors) with parsed.core.is_empty(), return s.is_empty()
//   BEFORE looping. Without this, pattern_match("", "test") wrongly returns true
//   (an empty NFA [Match] reaches Match at offset 0 in reach-any/full_match=false).
//   The firmware: `if (strlen(core_pattern)==0) return strlen(str)==0;`. Pinned by
//   {"","test",true,false} (false) and {"","",true,true} (true).
//   The OTHER three modes need NO special case (traced in notes §4):
//     ^...$ exact empty core: [Match] full_match=true matches empty str only (naturally).
//     ^...  prefix empty core: [Match] full_match=false early-returns true for ANY str.
//     ...$  suffix empty core: loops, at i==len [Match] full_match=true -> true for ANY str.
//
// CRITICAL (GOTCHA-B — inclusive end offset): the suffix/substring loops MUST probe
//   i == bytes.len() (the empty tail). char_indices() stops at the last char START,
//   not one-past; without chain(once(bytes.len())) the suffix/tail-empty cases
//   (e.g. pure "*" reaching Match at the very end, or "$" matching at the end)
//   would be missed. For ASCII this reproduces firmware 0..=str_len exactly.
//
// CRITICAL (GOTCHA-C — compile ONCE, not per offset): call nfa_compile(&parsed.core)
//   ONCE per match_with_anchors call and reuse &nfa across the loop. The firmware
//   recompiles inside nfa_match each call (stack-local pool, invisible cost); Rust
//   heap-allocates Vec<NfaOp>, so compile-once-simulate-many is the idiom
//   (REFINEMENT E; builds on T2.S2 REFINEMENT B). Semantics identical (nfa_compile
//   is pure) — only the call count drops.
//
// CRITICAL (GOTCHA-D — no NULL guard, no free): drop the firmware `if (!pattern ||
//   !str) return false` (Rust &str/&ParsedPattern are never null) AND the
//   free_parsed_pattern(&parsed) call (ParsedPattern owns its Vec; drop is
//   automatic). Exactly as T1.S2 dropped the C malloc-fallback + free_parsed_pattern.
//
// CRITICAL (GOTCHA-E — do NOT port the two wrapper fns): match_string_with_start
//   and match_reaches_end_with_start are zero-info forwarders in C (exist only
//   because nfa_match is static + takes raw bytes + recompiles). In Rust they
//   collapse into nfa_match(.., full_match=false) / nfa_match(.., full_match=true)
//   directly (REFINEMENT D). Porting them creates dead one-line aliases — DON'T.
//
// CRITICAL (GOTCHA-F — full_match per mode, don't mix): exact=true, suffix=true,
//   prefix=false, substring=false. Mixing flips semantics (e.g. suffix with
//   full_match=false becomes a substring match). The table in notes §2 is
//   authoritative.
//
// GOTCHA-G (char_indices, not raw byte offsets — per item spec): use
//   s.char_indices() for UTF-8 correctness (item spec requests it). For ASCII (the
//   whole corpus) it's byte-identical to firmware 0..=strlen. Add the terminal
//   offset bytes.len() (GOTCHA-B). See REFINEMENT F. (Acceptable alternative:
//   iterate 0..=bytes.len() over raw byte offsets — same corpus results — but the
//   item spec asks for char_indices, so prefer it.)
//
// GOTCHA-H (bytes, not chars, to nfa_match): pass s.as_bytes() to nfa_match (it's
//   byte-oriented); `start`/loop offsets are BYTE offsets from char_indices (always
//   valid UTF-8 boundaries, so slicing isn't even needed — nfa_match indexes bytes
//   directly). Never slice s and pass a sub-&str.
//
// GOTCHA-I (scope boundary — delimiter matcher is T3.S2, NOT here): port ONLY
//   match_with_anchors + pattern_match. The delimiter-aware match_pattern +
//   Pattern::Single|Parts enum + the GS (0x1D) split logic is P2.M1.T3.S2.
//   match_with_anchors is pub(crate); pattern_match is pub (consumed by T3.S2 + P3).
//
// GOTCHA-J (parse_pattern REUSED verbatim): pattern_match calls the EXISTING
//   in-tree parse_pattern (P2.M1.T1.S2, complete). Do NOT reimplement parsing or
//   anchor detection — that is T1.S2's delivered contract.
//
// BORROW-CHECKER: match_with_anchors borrows parsed (shared), binds a local
//   nfa: Vec<NfaOp>, and passes &nfa + s.as_bytes() to nfa_match — all disjoint,
//   no aliasing. The loop borrows nfa (shared) per call. Compiles clean, no unsafe.
//
// CRATE QUIRK: the crate-wide test command MUST be single-threaded (shared
//   debouncer state in notifier.rs):
//     cargo test --bin qmkonnect -- --test-threads=1   (AGENTS.md)
```

## Implementation Blueprint

### Data models and structure

This task adds NO new types (it consumes T1.S2's `ParsedPattern` + T2.S2's
`NfaOp`/`nfa_match`/`nfa_compile`). The only "structure" is the function
signatures:

```rust
pub(crate) fn match_with_anchors(parsed: &ParsedPattern, s: &str, case_sensitive: bool) -> bool;
// optional DRY helper (may be inlined instead):
fn suffix_or_substring_loop(nfa: &[NfaOp], bytes: &[u8], s: &str, case_sensitive: bool, full_match: bool) -> bool;
pub fn pattern_match(pattern: &str, s: &str, case_sensitive: bool) -> bool;
```

No enums, no structs. (`Pattern::Single|Parts` is P2.M1.T3.S2, NOT this task.)

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: IMPLEMENT match_with_anchors() in src/core/pattern.rs
  - SIGNATURE: pub(crate) fn match_with_anchors(parsed: &ParsedPattern, s: &str,
    case_sensitive: bool) -> bool
  - BODY (verified skeleton — research/notes.md §6):
      let bytes = s.as_bytes();
      if parsed.start_anchored && parsed.end_anchored {       // ^...$ exact
          let nfa = nfa_compile(&parsed.core);                 // GOTCHA-C: compile ONCE
          nfa_match(&nfa, bytes, 0, case_sensitive, true)      // GOTCHA-F: full_match=true
      } else if parsed.start_anchored {                        // ^ prefix
          let nfa = nfa_compile(&parsed.core);
          nfa_match(&nfa, bytes, 0, case_sensitive, false)     // GOTCHA-F: full_match=false
      } else if parsed.end_anchored {                          // $ suffix
          let nfa = nfa_compile(&parsed.core);
          suffix_or_substring_loop(&nfa, bytes, s, case_sensitive, true)  // loop, full=true
      } else {                                                  // substring (default)
          if parsed.core.is_empty() { return s.is_empty(); }   // GOTCHA-A: empty-core special case
          let nfa = nfa_compile(&parsed.core);
          suffix_or_substring_loop(&nfa, bytes, s, case_sensitive, false) // loop, full=false
      }
  - CALLS: the in-tree nfa_compile (S1-parallel, T2.S2 upstream) + nfa_match (T2.S2).
           Do NOT reimplement compile/match.
  - GOTCHA: NO NULL guard (GOTCHA-D). NO wrapper fns (GOTCHA-E). Compile ONCE per
            call (GOTCHA-C). full_match per the §2 table (GOTCHA-F). Empty-core
            guard ONLY in substring branch (GOTCHA-A).
  - VISIBILITY: pub(crate) (lets mod tests unit-test each mode via use super::*).
  - PLACEMENT: src/core/pattern.rs, AFTER nfa_match (T2.S2), BEFORE mod tests.

Task 2: (OPTIONAL) IMPLEMENT suffix_or_substring_loop() helper OR inline the loop
  - SIGNATURE: fn suffix_or_substring_loop(nfa: &[NfaOp], bytes: &[u8], s: &str,
    case_sensitive: bool, full_match: bool) -> bool
  - BODY (research/notes.md §6):
      for i in s.char_indices().map(|(i, _)| i).chain(std::iter::once(bytes.len())) {
          if nfa_match(nfa, bytes, i, case_sensitive, full_match) { return true; }
      }
      false
  - WHY: DRY for the shared char_indices + inclusive-end iteration (GOTCHA-B/G).
         An implementer MAY inline this loop in both the suffix and substring
         branches instead — either is acceptable.
  - GOTCHA: chain(once(bytes.len())) is MANDATORY for the inclusive end (GOTCHA-B).
  - VISIBILITY: private fn (only match_with_anchors calls it).
  - PLACEMENT: immediately above match_with_anchors.

Task 3: IMPLEMENT pattern_match() — the public entry
  - SIGNATURE: pub fn pattern_match(pattern: &str, s: &str, case_sensitive: bool) -> bool
  - BODY (research/notes.md §6):
      let parsed = parse_pattern(pattern);                     // T1.S2 (GOTCHA-J: reuse)
      match_with_anchors(&parsed, s, case_sensitive)
      // `parsed` drops here automatically — NO free_parsed_pattern (GOTCHA-D)
  - CALLS: the in-tree parse_pattern (T1.S2) + match_with_anchors (Task 1).
  - GOTCHA: NO NULL guard (GOTCHA-D). NO explicit free/drop (GOTCHA-D).
  - VISIBILITY: pub (the module's public API surface — consumed by T3.S2 + P3
                rules.rs). This is the FIRST `pub` (not pub(crate)) item in the file.
  - PLACEMENT: immediately after match_with_anchors.

Task 4: ADD rustdoc (Mode A — code-level docs only)
  - ON pattern_match (a /// block): document the full construct table per PRD §4 —
      * `*` wildcard (any sequence incl empty; combinable with anchors);
      * `^` start anchor; `$` end anchor; `^…$` exact full-string match;
      * `\^ \$ \* \\` literal escapes;
      * `\d \D` digit/non-digit; `\w \W` word/non-word; `\s \S` whitespace/non-ws;
      * `\b \B` word-boundary/non-boundary;
      * `.` any char except newline; `+` quantifier (one-or-more, linear-time);
      * SEMANTICS: no anchors => substring match (backward-compatible); case
        sensitivity per the `case_sensitive` arg (default false at the rules layer);
      * pipeline stage: parse_pattern -> process_escapes -> nfa_compile -> nfa_match
        -> match_with_anchors; this fn is the public entry (parse -> match -> drop).
      * cite PRD §4 + §14 (firmware is the single source of truth) + Russ Cox
        https://swtch.com/~rsc/regexp/regexp1.html for the linear-time NFA.
  - ON match_with_anchors (a /// block): map the four modes to their nfa_match calls:
      ^...$ exact  -> nfa_match(core, 0, cs, full=true)   (one call)
      ^...  prefix -> nfa_match(core, 0, cs, full=false)  (one call)
      ...$  suffix -> loop offsets nfa_match(core, i, cs, full=true)
      ...   substring -> empty-core guard, then loop offsets nfa_match(core, i, cs, full=false)
      Note the three refinements (D: wrappers folded; E: compile-once; F: char_indices)
      and cite firmware pattern_match.c:233-256 as the source.
  - REFERENCE arch external_deps.md §3 point 6 and PRD §4 as the contract sources.

Task 5: APPEND pattern_match / match_with_anchors unit tests to the existing mod tests
  - DO: add new #[test] fns INTO the existing `mod tests { use super::*; ... }`
        block. Group with the same `// --- header ---` comment style.
  - IMPLEMENT the ~40 vectors of research/notes.md §7 as end-to-end pattern_match
        assertions (and a few direct match_with_anchors calls to isolate modes).
        Suggested groupings:
      // --- Start anchor ^ (prefix) ---                          (§7.1, 10 rows)
      // --- End anchor $ (suffix) ---                            (§7.2, 10 rows)
      // --- Full anchor ^…$ (exact) ---                          (§7.3, 13 rows)
      // --- Substring (no anchors) + empty-core special case --- (§7.4, 14 rows)
      // --- Edge cases / escapes / classes / \b ---              (§7.5, 10 rows)
  - HIGHLIGHT the empty-core special case with a comment (GOTCHA-A):
        assert!(!pattern_match("", "test", true));   // empty pattern, non-empty -> false
        assert!( pattern_match("", "",     true));   // empty pattern, empty -> true
  - HIGHLIGHT the \b linchpin end-to-end:
        assert!(!pattern_match("\\bword", "aword", true));  // \b sees original str
        assert!( pattern_match("\\bword", "a word", true)); // boundary before 'word'
  - HIGHLIGHT case sensitivity across modes:
        assert!( pattern_match("^abc$", "ABC", false));  // ci exact
        assert!(!pattern_match("^abc$", "ABC", true));   // cs exact
  - TEST BOTH entries: primarily pattern_match (end-to-end, the public contract),
        plus 2-3 direct match_with_anchors calls building ParsedPattern via
        parse_pattern to isolate the four-mode dispatch (e.g. confirm suffix mode
        loops offsets by testing pattern_match("test$","pretest",true)==true).
  - NAMING: test_pm_<behavior> for end-to-end (e.g. test_pm_start_anchor_prefix,
        test_pm_end_anchor_suffix, test_pm_full_anchor_exact,
        test_pm_substring_empty_core_only_matches_empty,
        test_pm_bword_linchpin_false, test_pm_case_sensitive_exact_false) and
        test_mwa_<mode> for direct match_with_anchors isolation.
  - COVERAGE: every anchor mode, the empty-core special case, case on/off,
        glob-with-anchors, escapes, classes (\d \w \s), +, \b end-to-end.

Task 6: VERIFY build + tests (single-threaded, per AGENTS.md)
  - RUN: cargo build --bin qmkonnect        (expect: clean; no NEW warnings)
  - RUN: cargo test --bin qmkonnect pattern -- --test-threads=1
         (expect: ALL pattern::tests pass — new pattern_match/match_with_anchors
          tests AND S1/S2/nfa_compile/T2.S2 nfa_match tests)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1
         (expect: full crate green — no regression)
  - IF a parity assertion fails: re-read the C source (pattern_match.c
    match_with_anchors/pattern_match) line-by-line + research/notes.md §6 skeleton;
    the table is a faithful transcription, so a failure means the Rust diverged.
    Do NOT "fix" the test to match the Rust — fix the Rust to match the firmware
    (PRD §14: firmware is source of truth).
  - CONFIRM git status shows ONLY src/core/pattern.rs modified.
```

### Implementation Patterns & Key Details

```rust
// The canonical match_with_anchors body (this IS the spec — match it exactly).
// Full verified version in research/notes.md §6.
//
// pub(crate) fn match_with_anchors(parsed: &ParsedPattern, s: &str,
//                                  case_sensitive: bool) -> bool {
//     let bytes = s.as_bytes();
//     if parsed.start_anchored && parsed.end_anchored {        // ^...$ exact
//         let nfa = nfa_compile(&parsed.core);                 // GOTCHA-C: compile ONCE
//         nfa_match(&nfa, bytes, 0, case_sensitive, true)      // full_match=true (GOTCHA-F)
//     } else if parsed.start_anchored {                        // ^ prefix
//         let nfa = nfa_compile(&parsed.core);
//         nfa_match(&nfa, bytes, 0, case_sensitive, false)     // full_match=false
//     } else if parsed.end_anchored {                          // $ suffix
//         let nfa = nfa_compile(&parsed.core);
//         suffix_or_substring_loop(&nfa, bytes, s, case_sensitive, true)
//     } else {                                                 // substring (default)
//         if parsed.core.is_empty() { return s.is_empty(); }  // GOTCHA-A: empty-core special case
//         let nfa = nfa_compile(&parsed.core);
//         suffix_or_substring_loop(&nfa, bytes, s, case_sensitive, false)
//     }
// }
//
// fn suffix_or_substring_loop(nfa: &[NfaOp], bytes: &[u8], s: &str,
//                             case_sensitive: bool, full_match: bool) -> bool {
//     // GOTCHA-B/G: char_indices (UTF-8-correct; == byte offsets for ASCII) +
//     // chain(once(len)) to preserve the firmware's inclusive 0..=len end.
//     for i in s.char_indices().map(|(i, _)| i).chain(std::iter::once(bytes.len())) {
//         if nfa_match(nfa, bytes, i, case_sensitive, full_match) {
//             return true;
//         }
//     }
//     false
// }
//
// pub fn pattern_match(pattern: &str, s: &str, case_sensitive: bool) -> bool {
//     let parsed = parse_pattern(pattern);   // T1.S2 (GOTCHA-J: reuse, don't reimplement)
//     match_with_anchors(&parsed, s, case_sensitive)
//     // `parsed` drops here — NO free_parsed_pattern (GOTCHA-D)
// }
```

### Integration Points

```yaml
MODULE REGISTRATION:
  - NONE. `pub mod pattern;` is already in src/core/mod.rs (added by S1). Do NOT
    edit mod.rs.

DEPENDENCIES (this task): NONE new. Pure stdlib (char_indices, std::iter::once) +
                           references to the in-module T1.S2 parse_pattern/ParsedPattern
                           + T2.S2 nfa_compile/NfaOp/nfa_match. No Cargo deps, no
                           qmk_notifier crate, no firmware link, no `unsafe`, no `static`.

UPSTREAM (already present — T1.S2 + T2.S2 contracts):
  - pub(crate) struct ParsedPattern { core: Vec<u8>, start_anchored: bool,
    end_anchored: bool } (T1.S2) — the two flags drive mode selection; core is compiled.
  - pub(crate) fn parse_pattern(pattern: &str) -> ParsedPattern (T1.S2) — called by
    pattern_match. REUSED verbatim (GOTCHA-J).
  - pub(crate) fn nfa_compile(pat: &[u8]) -> Vec<NfaOp> (S1-parallel) — compiles the
    core ONCE per match_with_anchors call (GOTCHA-C/REFINEMENT E).
  - pub(crate) fn nfa_match(states: &[NfaOp], string: &[u8], start: usize,
    case_sensitive: bool, full_match: bool) -> bool (T2.S2) — the simulator; the four
    modes map to its full_match=true/false + start offset (notes §2).

DOWNSTREAM CONSUMERS (later subtasks — do NOT implement them here):
  - P2.M1.T3.S2 delimiter-aware match_pattern + Pattern::Single|Parts: splits a
    pattern/message on the GS delimiter (0x1D) and calls pattern_match on each half
    (PRD §4.1). Consumes THIS task's pub pattern_match.
  - P3.M1 rules.rs: evaluates rules.toml layer/callback rules via pattern_match(
    rule.pattern, window_class_or_title, rule.case_sensitive). Consumes THIS task's
    pub pattern_match.
  - P2.M1.T4.S1: ports the FULL 380-row firmware end-to-end corpus as parity tests
    (exercising pattern_match end-to-end). This task's ~40-vector subset is a
    precursor; T4.S1 expands it.

CONFIG: none.
ROUTES: none (no CLI surface in this subtask — that's P5.M1).
DATABASE: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles clean. If rustc warns on src/core/pattern.rs, READ it and fix.
# The file carries #![allow(dead_code)] from S1; the new pub pattern_match is the
# first `pub` item, and match_with_anchors is consumed by it, so no spurious
# dead-code warnings.

# Confirm the additions are present:
grep -n 'fn match_with_anchors' src/core/pattern.rs   # expect one pub(crate) def
grep -n 'pub fn pattern_match' src/core/pattern.rs    # expect one def
grep -n 'char_indices' src/core/pattern.rs            # expect the loop helper / inline
grep -n 'parsed.core.is_empty' src/core/pattern.rs    # the empty-core guard (GOTCHA-A)
```

### Level 2: Unit Tests — the parity contract (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# Single-threaded is MANDATORY crate-wide (shared debouncer state in notifier.rs).
cargo test --bin qmkonnect pattern -- --test-threads=1
# Expected: every test in pattern::tests passes — the new pattern_match/match_with_anchors
# tests (research/notes.md §7, ~40 vectors) AND S1/S2/nfa_compile/T2.S2 tests. A failure
# means the Rust diverged from the firmware C — fix the Rust, not the test.
# Filter to just the new tests to see them individually:
cargo test --bin qmkonnect pattern::tests::test_pm -- --test-threads=1
cargo test --bin qmkonnect pattern::tests::test_mwa -- --test-threads=1
```

### Level 3: Full-crate regression (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL bin tests green — pattern::tests (new pattern_match/match_with_anchors +
# S1 + S2 + nfa_compile + T2.S2 nfa_match) + notifier + types. Proves the additive edit
# didn't break module resolution and compiles in the full crate context.

# Confirm the change surface is exactly one file:
git status --short
# Expected:
#   modified:   src/core/pattern.rs        (ONLY this)
git diff --stat
# Expected: only src/core/pattern.rs changed; mod.rs and everything else untouched.
```

### Level 4: Fidelity cross-check (optional, high-confidence)

```bash
# Cross-validate against the firmware's own corpus (unchanged by this task — it
# lives in the OTHER repo). The Rust parity vectors in research/notes.md §7 were
# DERIVED from these C tests, so a green firmware run corroborates the contract the
# Rust port encodes. The Rust tests are STRICTLY STRONGER: they also exercise
# match_with_anchors per-mode (the firmware tests only assert end-to-end via the
# public pattern_match).
cd /home/dustin/projects/qmk-notifier && ./run_all_tests.sh
# Expected: the full pattern_match corpus passes (it always does — this task does
# not touch the firmware). Particularly: test_edge_cases's {"","test",true,false}
# (empty-core special case), {"^$","",true,true} (exact empty), and the \b cases in
# test_word_boundary_basic — the exact semantics the Rust tests encode.
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean (no NEW warnings on `src/core/pattern.rs`).
- [ ] `cargo test --bin qmkonnect pattern -- --test-threads=1` — all pattern tests pass (new + S1 + S2 + nfa_compile + T2.S2).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` — full crate green.
- [ ] `git status` shows exactly ONE modified file: `src/core/pattern.rs`.

### Feature Validation (parity)
- [ ] Every row of `research/notes.md` §7 (~40 vectors) asserted and passing.
- [ ] **Empty-core special case** (GOTCHA-A): `pattern_match("","test",true)`==false AND `pattern_match("","",true)`==true.
- [ ] **`^` prefix** (§7.1): `^searchterm`/`searchterm`→true; `/presearchterm`→false; `/searchtermpost`→true (reach-any); `^`/`""`→true.
- [ ] **`$` suffix** (§7.2): `searchterm$`/`searchterm`→true; `/searchtermpost`→false; `/presearchterm`→true (loops offsets); `$`/`""`→true.
- [ ] **`^…$` exact** (§7.3): `/searchterm`→true; `/pre…`→false; `/…post`→false; `^$`/`""`→true; `^$`/`a`→false.
- [ ] **substring** (§7.4): `searchterm`/`pre…post`→true; `*`/`anything`→true.
- [ ] **`\b` linchpin end-to-end**: `pattern_match("\\bword","aword",true)`==false.
- [ ] **Case sensitivity** (all modes): `^abc$`/`ABC` matches iff `!case_sensitive`.
- [ ] **Anchors + glob**: `^sear*term$`/`searedsalmonterm`→true; `*term`/`searchterm`→true.
- [ ] **`+` quantifier end-to-end**: `^\\d+$`/`12345`→true; `^\\w+$`/`hello_1`→true.

### Code Quality Validation
- [ ] `pattern_match` is `pub`; `match_with_anchors` is `pub(crate)`.
- [ ] The three refinements applied: wrappers folded into direct `nfa_match` calls (D); core compiled ONCE (E); `char_indices().chain(once(len))` (F).
- [ ] NO NULL guard; NO `free`/Drop analog; NO ported wrapper fns (GOTCHA-D/E).
- [ ] Empty-core guard present ONLY in the substring branch (GOTCHA-A).
- [ ] `full_match` correct per mode: exact/suffix=true, prefix/substring=false (GOTCHA-F).
- [ ] Inclusive end offset via `chain(once(bytes.len()))` (GOTCHA-B).
- [ ] NO `static`; NO `unsafe`; reuses T1.S2 `parse_pattern` + T2.S2 `nfa_compile`/`nfa_match` verbatim (GOTCHA-J).
- [ ] Rustdoc on `pattern_match` lists the full construct table (Mode A per item spec DOCS).
- [ ] New tests appended to the existing `mod tests` (prior tests untouched), grouped with `// --- header ---` comments.
- [ ] No new Cargo dependencies.
- [ ] Scope respected: NO delimiter-aware `match_pattern`, NO `Pattern::Single|Parts`, NO `mod.rs` edit.

### Documentation & Deployment
- [ ] Code-level rustdoc present (Mode A — no `docs/*.md` or README changes this task).
- [ ] `pattern_match` documents all constructs (`*`, `^`, `$`, `^…$`, `\^ \$ \* \\`, `\d \D \w \W \s \S`, `\b \B`, `.`, `+`) + substring-default semantics.
- [ ] `match_with_anchors` documents the four-mode → `nfa_match` mapping + the three refinements.

---

## Anti-Patterns to Avoid

- ❌ Do NOT omit the empty-core substring guard. `pattern_match("", "test")` MUST
      return false (GOTCHA-A). Without the guard it returns true (empty NFA reaches
      Match at offset 0). Only substring mode needs it — the others are correct
      naturally (traced in notes §4).
- ❌ Do NOT forget the inclusive end offset. The loop must probe `i == bytes.len()`
      (GOTCHA-B). `char_indices()` alone stops at the last char start and misses
      suffix/tail-empty cases. Use `chain(once(bytes.len()))`.
- ❌ Do NOT recompile the core per offset. Compile ONCE via `nfa_compile(&parsed.core)`
      and reuse `&nfa` across the loop (GOTCHA-C/REFINEMENT E). The firmware
      recompiles inside nfa_match; Rust heap-allocates, so compile-once is the idiom.
- ❌ Do NOT port the two C wrapper fns (`match_string_with_start` /
      `match_reaches_end_with_start`). They collapse into direct `nfa_match(..,
      full_match)` calls (GOTCHA-E/REFINEMENT D). Porting them creates dead aliases.
- ❌ Do NOT add a NULL guard or a `free`/Drop call. Rust `&str`/`&ParsedPattern` are
      never null; the `Vec` drops automatically (GOTCHA-D).
- ❌ Do NOT mix up `full_match` per mode. exact=true, suffix=true, prefix=false,
      substring=false (GOTCHA-F). A wrong flag flips semantics (e.g. suffix with
      full_match=false becomes substring).
- ❌ Do NOT reimplement `parse_pattern`, `nfa_compile`, or `nfa_match`. Reuse the
      in-tree T1.S2/T2.S2 functions verbatim (GOTCHA-J).
- ❌ Do NOT iterate raw `chars()` and pass to nfa_match. nfa_match is byte-oriented —
      pass `s.as_bytes()` with a byte offset from `char_indices` (GOTCHA-H).
- ❌ Do NOT implement the delimiter-aware `match_pattern`, `Pattern::Single|Parts`,
      or the GS (0x1D) split — those are P2.M1.T3.S2 (GOTCHA-I).
- ❌ Do NOT edit `src/core/mod.rs` — `pub mod pattern;` is already there (S1).
- ❌ Do NOT change the test to match divergent Rust output. The firmware C
      (`pattern_match.c` `match_with_anchors`/`pattern_match`) is the source of
      truth (PRD §14); fix the Rust.
- ❌ Do NOT run tests multi-threaded — the crate shares debouncer state
      (`cargo test --bin qmkonnect -- --test-threads=1`).
- ❌ Do NOT edit `PRD.md`, any `tasks.json`, `prd_snapshot.md`, or any `plan/` file
      other than this item's own `PRP.md` + `research/`.

---

## Confidence Score: 9/10

This is a well-bounded, ~25-line port of two heavily-commented firmware C
functions whose ENTIRE source (`match_with_anchors`, `pattern_match`, and the two
wrapper forwarders they call) is reproduced VERBATIM in `research/notes.md` §1,
and transcribed as a verified Rust skeleton in §6 (mirrored in the Implementation
Blueprint). The three refinements (D: fold wrappers; E: compile-once; F:
char_indices) are each forced by idiomatic Rust and fully derived in §3, with the
four-mode → `nfa_match` mapping table in §2 pinning the `full_match` flag per mode.
The single parity trap — the empty-core substring special case — is traced through
all four modes in §4 (proving only substring needs the guard) and pinned by
`{"","test",true,false}`. ~40 end-to-end parity vectors curated from the firmware's
380-row corpus are provided as the test contract (§7), grouped by anchor mode +
the empty special case + escapes/classes/`\b`, including the `\bword`/`aword`
linchpin end-to-end. The upstream T2.S2 contract (`nfa_match` signature with
`start`+`full_match`+compiled `&[NfaOp]`) and T1.S2 contract (`ParsedPattern`+
`parse_pattern`) are both confirmed from their PRPs and the in-tree file, and the
downstream T3.S2/P3 consumers (how they call the pub `pattern_match`) are
explicit. No new deps, no `unsafe`, no `static`. The 1-point reservation is for
the (unlikely) event an implementer mishandles the inclusive end offset
(`chain(once(len))`) or skips the empty-core guard despite the explicit callouts
and linchpin tests; both are caught immediately by the parity tests. Scope is
cleanly bounded from T2.S2 (upstream simulator) and T3.S2 (delimiter matcher), so
there is no risk of over- or under-building.
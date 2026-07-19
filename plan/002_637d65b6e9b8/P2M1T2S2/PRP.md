# PRP — P2.M1.T2.S2: Port epsilon-closure (`nfa_addstate`) + NFA simulation (`nfa_match`)

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This is an **additive edit** to the existing
> module `src/core/pattern.rs` (S1 `process_escapes` + consts; S2 `ParsedPattern`
> + `parse_pattern`; S1's parallel sibling `NfaOp` + `nfa_compile`). This task
> adds the **Thompson-NFA simulator** — the epsilon-closure (`nfa_addstate`) and
> the two-list simulation (`nfa_match`), plus the five character predicates
> (`is_digit_char`/`is_word_char`/`is_whitespace_char`/`is_word_boundary`/
> `pattern_char_matches`) and `nfa_has_match` — that runs a compiled `Vec<NfaOp>`
> against a candidate string. It consumes the S1 compiler's output and is consumed
> by the anchor strategy (`match_with_anchors`, P2.M1.T3.S1). Firmware
> `pattern_match.c` (~lines 454–625) is the single source of truth (PRD §4, §14).

---

## Goal

**Feature Goal**: Port the firmware `nfa_addstate()` (`pattern_match.c:~480–540`)
and `nfa_match()` (`pattern_match.c:~578–625`), together with their supporting
predicates `is_digit_char`/`is_word_char`/`is_whitespace_char`/`is_word_boundary`
(`:~454–478`), `pattern_char_matches` (`:~542–575`), `decoded_literal`
(firmware `get_escaped_char:~116–140`, literal cases only), and `nfa_has_match`
(`:~575–577`) to Rust as **index-based** types operating on the `Vec<NfaOp>`
produced by S1's `nfa_compile`. The simulation is the canonical two-list
Thompson NFA (clist/nlist swapped per step) with generation-tag O(1) dedup,
guaranteed O(states × strlen), no backtracking.

**Deliverable**: additions to `src/core/pattern.rs` (do NOT recreate the file):
1. five `#[inline]` private byte predicates: `fn is_digit_char(u8)->bool`,
   `fn is_word_char(u8)->bool`, `fn is_whitespace_char(u8)->bool`,
   `fn is_word_boundary(&[u8], usize)->bool`, `fn decoded_literal(u8)->u8`;
2. `fn pattern_char_matches(pc: u8, sc: u8, case_sensitive: bool) -> bool`;
3. `fn nfa_addstate(states: &[NfaOp], idx: usize, list: &mut Vec<usize>,
   seen: &mut Vec<u32>, generation: u32, string_start: &[u8], abspos: usize)`;
4. `fn nfa_has_match(states: &[NfaOp], list: &[usize]) -> bool`;
5. `pub(crate) fn nfa_match(states: &[NfaOp], string: &[u8], start: usize,
   case_sensitive: bool, full_match: bool) -> bool`;
6. rustdoc on `nfa_match` (Mode A) explaining the two-list simulation, the
   O(states × strlen) guarantee, and the `full_match` semantics — citing Russ Cox;
7. new `#[test]` fns appended to the existing `#[cfg(test)] mod tests` block,
   covering the parity table in `research/notes.md` §4 (~30 vectors).

**Success Definition**:
- For every `(pattern, input, start, case_sensitive, full_match)` in
  `research/notes.md` §4, the Rust `nfa_match` returns the firmware-expected
  bool — including the **linchpin** `\bword`/`aword`@1 → **false** test that
  proves `\b` evaluates against the ORIGINAL string at an absolute offset.
- `cargo build --bin qmkonnect` compiles clean (no new warnings).
- `cargo test --bin qmkonnect -- --test-threads=1` passes (new `nfa_match` tests
  AND S1's `process_escapes`/`parse_pattern`/`nfa_compile` tests).
- `git diff` touches ONLY `src/core/pattern.rs`. No `match_with_anchors`, no
  public `pattern_match` entry, no delimiter-aware `match_pattern`, no `mod.rs`
  change — those are P2.M1.T3.

## User Persona (if applicable)

**Target User**: The downstream anchor-strategy layer `match_with_anchors()`
(P2.M1.T3.S1), which compiles a pattern once via `nfa_compile` then calls
`nfa_match` at each candidate offset to implement prefix / suffix / exact /
substring matching. Not a user-facing API.

**Use Case**: Given a compiled NFA (e.g. `[Char{0x61,out:1}, Split{out:0,out1:2},
Match]` for `a+`) and an input string, decide in guaranteed O(states × strlen)
whether the pattern matches — either as a reach-any (prefix/substring,
`full_match=false`) or a consume-whole-remaining (exact/suffix,
`full_match=true`) run. Example: `nfa_match(&nfa_compile(&[0x61, PLUS_QUANT]),
b"aaa", 0, true, true)` → `true`.

**Pain Points Addressed**: Completes the simulator stage so `match_with_anchors`
(T3.S1) has a correct, linear-time matcher to drive. A wrong `\b` (evaluating
against a per-offset slice instead of the original string) would make
`\bword` wrongly match `aword` as a substring — the linchpin test pins this. A
backtracking simulator would blow up on `a+a+a+…`; the generation-tag two-list
simulation is provably linear (the whole reason the firmware moved off the old
backtracker, PRD §7.8).

## Why

- **Stage 4 of the full-parity pipeline.** PRD §14 mandates the Rust matcher be a
  *"full-parity port of the firmware `pattern_match.c`, not a subset"* with
  *"the firmware matcher + its test corpus the single source of truth for match
  semantics."* `nfa_addstate`/`nfa_match` are the simulate stage between
  `nfa_compile` (S1) and `match_with_anchors` (T3.S1). The firmware C source
  (`pattern_match.c` ~lines 454–625) + its test corpus are the spec.
- **The linear-time guarantee lives here.** The two-list simulation with
  generation-tag dedup is O(states × strlen) with no backtracking — the fix for
  the old exponential matcher (PRD §7.8). The `a+a+a+` compiled NFA (7 states,
  from S1) must simulate in linear time against long inputs.
- **`\b`/`\B` correctness is subtle.** They must evaluate against the ORIGINAL
  string at an ABSOLUTE offset (PRD §13 #10), not a per-offset slice. The
  firmware `nfa_match` carries two pointers (`str` + `string_start`) for exactly
  this; the Rust port must preserve it (REFINEMENT A below). The empty-string
  special case (neither boundary nor non-boundary passes) is legacy semantics
  the test corpus encodes.

## What

Add to `src/core/pattern.rs` (the file S1/S2 created and S1's parallel sibling
extended with `NfaOp`/`nfa_compile`; APPEND, do NOT recreate). Three justified
refinements to the item-spec sketch (each forced by correctness, fully derived in
`research/notes.md` §2):

### REFINEMENT A — `nfa_match` takes the ORIGINAL string + a `start` offset

The item-spec sketch: `fn nfa_match(pattern: &[u8], str_bytes: &[u8],
case_sensitive: bool, full_match: bool)` — a SINGLE string. But `\b`/`\B` MUST
evaluate against the **original** string at an **absolute** offset (PRD §13 #10;
the firmware carries `str` + `string_start` separately). Proven by the firmware
parity vector `{"\\bword", "aword", true, false}`: at substring offset 1, `\b`
must see `'a'` before `'w'` (both word chars → no boundary → fail). A lone slice
cannot carry both the original and an offset. **Resolution:** add `start: usize`;
`string` is the full original input (firmware `string_start`), `start` is the
consume offset (firmware `str - string_start`). T3.S1 maps the firmware substring
loop trivially: `for i in 0..=len { nfa_match(core, full, i, cs, false) }`. This
mirrors S1's `Any { out }` refinement (sketch insufficient for correctness).

### REFINEMENT B — `nfa_match` takes the COMPILED `&[NfaOp]`, not raw bytes

The sketch typed the first param `pattern: &[u8]`, but the item spec also says
*"INPUT: Compiled NFA from P2.M1.T2.S1."* The firmware reconciles by having
`nfa_match` call `nfa_compile` **internally** (C can't pass the pool cheaply
without a `static`). **Resolution:** take `states: &[NfaOp]` (S1's `nfa_compile`
output). Idiomatic Rust, honors the INPUT spec, and lets T3.S1 compile ONCE then
simulate at many offsets (the firmware wastefully recompiles each call).

### REFINEMENT C — dedup via `Vec<u32>` generation tags (NOT `Vec<bool>`)

The sketch: `seen: &mut Vec<bool>` + `generation: usize`. A `bool` **cannot carry
a generation tag**, so a `Vec<bool>` would need an O(states) `fill(false)` per
phase — abandoning the firmware's documented O(1)-dedup (RESEARCH NOTE: *"bump
`nfa_gen` once per phase for O(1) dedup"*; PRD §13 #11). **Resolution:**
`seen: &mut Vec<u32>` carrying the generation tag (`seen[idx] == generation` ⇒
already followed this phase); bumped once per phase, NEVER cleared. `u32` matches
the firmware `int lastlist` (overflow impossible: generation ≈ strlen + 1).
Keeps BOTH sketch params meaningful; the single type change `Vec<bool>`→`Vec<u32>`
is forced (a bool can't hold the tag). Parallels S1's lastlist-separation.

### The functions to implement

1. **Five byte predicates** (`#[inline]`, private) — idiomatic `u8` methods that
   are byte-identical to the C ranges:
   - `is_digit_char(c) = c.is_ascii_digit()`
   - `is_word_char(c) = c.is_ascii_alphanumeric() || c == b'_'`
   - `is_whitespace_char(c) = c.is_ascii_whitespace()` (GOTCHA-8: ASCII, not Unicode)
   - `is_word_boundary(string: &[u8], pos: usize) -> bool` (faithful port, §3b of notes)
   - `decoded_literal(pc: u8) -> u8` (0x01→`^`, 0x02→`$`, 0x03→`*`, 0x04→`\\`)
2. **`pattern_char_matches(pc, sc, case_sensitive)`** — decode-then-ASCII-fold
   (§3c of notes). Uses the S1 named consts (`CLASS_DIGIT`…`DOT_META`,
   `ESC_CARET`…`ESC_BSLASH`).
3. **`nfa_addstate(...)`** — the epsilon closure (§3d of notes).
4. **`nfa_has_match(states, list)`** — scan for a `Match`.
5. **`nfa_match(states, string, start, case_sensitive, full_match)`** — the
   two-list simulation (§3e of notes).
6. **Rustdoc** (Mode A) on `nfa_match` (+ brief `///` on the predicates +
   `nfa_addstate`).
7. **Unit tests** appended to the existing `mod tests`.

### Success Criteria
- [ ] `pub(crate) fn nfa_match(states: &[NfaOp], string: &[u8], start: usize, case_sensitive: bool, full_match: bool) -> bool` exists.
- [ ] `fn nfa_addstate(...)`, `fn nfa_has_match(...)`, `fn pattern_char_matches(...)`, and the five predicates exist (private, `#[inline]`).
- [ ] Every row of `research/notes.md` §4 (~30 vectors) passes.
- [ ] **Linchpin**: `nfa_match(&nfa_compile(&process_escapes("\\bword")), b"aword", 1, true, false)` == `false` (GOTCHA-2 / REFINEMENT A).
- [ ] **Empty-string `\b`/`\B`**: `\b`→false AND `\B`→false against `b""` (GOTCHA-6).
- [ ] **Dot vs glob**: `.` excludes `\n`/`\r`; `*` includes them (GOTCHA-7).
- [ ] **Generation dedup**: `\b\b` against `b"a"` → true (no infinite recursion; GOTCHA-4/11).
- [ ] Rustdoc on `nfa_match` cites Russ Cox + explains two-list simulation + `full_match`.
- [ ] No new deps; no `unsafe`; no `static`; pure stdlib.
- [ ] `cargo build --bin qmkonnect` clean; `cargo test --bin qmkonnect -- --test-threads=1` green.
- [ ] `git diff` touches only `src/core/pattern.rs`.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge of this codebase can implement this
using only this PRP + the firmware repo, because (a) the ENTIRE C source for all
seven functions being ported (`nfa_addstate`, `nfa_match`, `nfa_has_match`,
`pattern_char_matches`, `get_escaped_char`, the three classifiers,
`is_word_boundary`) is reproduced VERBATIM in `research/notes.md` §1, (b) the
three signature refinements (each forced by correctness) are fully derived and
justified in §2 with a concrete failing-parity-vector proof for REFINEMENT A,
(c) a verified Rust skeleton for EVERY function is given in §3 (mirrored in the
Implementation Blueprint), (d) ~30 `nfa_match`-level parity vectors derived from
the firmware test corpus are provided as the test contract (§4), including the
linchpin `\b`/original-string cases and the empty-string special case, (e) the 15
gotchas are enumerated with the specific test rows that pin each, (f) the
upstream S1 contract (`NfaOp` shape, `nfa_compile` output, named consts) and the
downstream T3.S1 contract (how `match_with_anchors` will call `nfa_match`) are
both explicit. See `research/notes.md`.

### Documentation & References

```yaml
# MUST READ — the canonical C source (single source of truth, PRD §14)
- file: /home/dustin/projects/qmk-notifier/pattern_match.c
  why: "lines ~454-625 are the functions being ported, verbatim: is_digit_char /
        is_word_char / is_whitespace_char (~454-468), is_word_boundary (~470-478),
        nfa_addstate (~480-540), pattern_char_matches (~542-575), nfa_has_match
        (~575-577), nfa_match (~578-625). get_escaped_char (~116-140) is the
        escaped-literal decoder. Reproduced in research/notes.md §1."
  pattern: "nfa_addstate: dedup via lastlist==nfa_gen (mark BEFORE dispatch),
            OP_SPLIT follows both edges with abspos UNCHANGED, OP_ASSERT
            conditionally recurses (empty-string short-circuit BEFORE
            is_word_boundary), OP_CHAR/ANY/MATCH collected. nfa_match: two-list
            (clist/nlist) swap-per-step, nfa_gen++ once per phase, feed one char
            per step, full_match gates the early-return."
  gotcha: "nfa_match takes TWO pointers (str + string_start) so \\b/\\B evaluate
           against the ORIGINAL string at an absolute offset (PRD §13 #10). A
           single Rust slice can't carry both — REFINEMENT A adds `start: usize`.
           Also: the firmware calls nfa_compile INTERNALLY; Rust takes the
           compiled &[NfaOp] instead (REFINEMENT B)."

# MUST READ — the file THIS task edits (S1+S2+S1-parallel already present in-tree)
- file: src/core/pattern.rs
  why: "already has: pub(crate) const ESC_CARET..GLOB_STAR (0x01-0x0E, 0x2A),
        pub(crate) fn process_escapes, pub(crate) struct ParsedPattern +
        parse_pattern, #[derive(Debug,Clone,PartialEq)] pub(crate) enum NfaOp
        {Char{arg,out},Any{out},Split{out,out1},Assert{arg,out},Match}, impl
        NfaOp {set_out,set_out1}, nfa_compile, and #[cfg(test)] mod tests { use
        super::*; ... }. APPEND the predicates + pattern_char_matches +
        nfa_addstate + nfa_has_match + nfa_match + new tests HERE. Do NOT
        recreate."
  pattern: "tests grouped with `// --- header ---` comments; build pattern bytes
            via process_escapes then nfa_compile (e.g.
            `nfa_compile(&process_escapes(\"\\\\bword\"))`); assert bool results.
            Mirror that style for the nfa_match tests."
  gotcha: "the file carries #![allow(dead_code)] (API shipped ahead of consumers).
           nfa_match's first non-test consumer is match_with_anchors (T3.S1);
           until then #[allow(dead_code)] keeps it warning-free. The predicates
           are private fns used only by nfa_match — also covered by the allow."

# MUST READ — the verified Rust skeletons + the parity table (THIS task's contract)
- file: plan/002_637d65b6e9b8/P2M1T2S2/research/notes.md
  why: "§1 reproduces ALL seven C functions verbatim. §2 derives the three
        signature refinements (with a concrete failing-vector proof for A). §3
        gives a verified Rust skeleton for every function (predicates,
        is_word_boundary, decoded_literal, pattern_char_matches, nfa_addstate,
        nfa_has_match, nfa_match). §4 lists ~30 nfa_match parity vectors (the
        test contract). §5 enumerates 15 gotchas with their pinning test rows."
  section: "## 3. Verified Rust skeleton" and "## 4. Parity table" and "## 5. Gotchas"

# MUST READ — the upstream S1 contract (the compiled NFA this task consumes)
- file: plan/002_637d65b6e9b8/P2M1T2S1/PRP.md
  why: "defines the NfaOp enum shape (Char{arg,out}, Any{out}, Split{out,out1},
        Assert{arg,out}, Match) + nfa_compile(pat: &[u8]) -> Vec<NfaOp> + the
        start==0 invariant (the simulator seeds from states[0]). Confirms the
        compiled-program contract this task operates on. NfaOp has NO lastlist
        field — the simulator owns dedup (this task: Vec<u32> generation tags)."
  section: "## What" (NfaOp enum + the start==0 invariant)

# MUST READ — the firmware parity vectors (the source of the test contract)
- file: /home/dustin/projects/qmk-notifier/test_word_boundary_basic.c
  why: "the \\b/\\B test corpus — INCLUDING the linchpin `{\"\\\\bword\",
        \"aword\", true, false}` that proves \\b sees the original string at an
        absolute offset, and the empty-string `{\"\\\\b\", \"\", true, false}`
        and `{\"\\\\B\", \"\", true, false}` special cases. Derive the
        nfa_match-level vectors in notes §4.5-4.6 from these."
  section: "test_word_boundary_basic()"
- file: /home/dustin/projects/qmk-notifier/test_pattern_match.c
  why: "the broad end-to-end corpus (anchors, wildcards, classes, dot, +,
        escapes, case sensitivity). Used to DERIVE the nfa_match-level vectors in
        notes §4.1-4.4 (the full end-to-end corpus is ported in P2.M1.T4.S1)."

# MUST READ — QMKonnect-side architecture contract (cross-repo)
- file: plan/002_637d65b6e9b8/architecture/external_deps.md
  why: "§3 'Pattern Matcher' points 4-5 are the cross-repo contract: nfa_addstate
        follows OP_SPLIT both branches + conditionally recurses OP_ASSERT via
        is_word_boundary, lastlist==nfa_gen prevents infinite recursion; nfa_match
        is the two-list simulation with nfa_gen bumped once per phase, full_match
        gates prefix/substring vs consume-whole. Confirms match_with_anchors
        (T3.S1) is a LATER subtask, not here."
  section: "## 3. Pattern Matcher" (points 4-6)

# Reference — firmware architecture doc (corroborates the C)
- file: /home/dustin/projects/qmk-notifier/plan/001_e329fbe4ae4d/architecture/pattern_match_architecture.md
  why: "'NFA Simulation (nfa_match)' section restates the two-list clist/nlist
        swap, nfa_gen++ once per phase, feed one char per step, full_match flag.
        Cross-checks the C source."
  section: "### NFA Simulation (nfa_match)"

# Reference — the linear-time guarantee (cite in rustdoc, Mode A)
- url: https://swtch.com/~rsc/regexp/regexp1.html
  why: "Russ Cox, 'Regular Expression Matching Can Be Simple And Fast'. For
        nfa_match's rustdoc: the two-list simulation is O(states x input_len)
        with no backtracking; the generation-tag dedup makes each phase O(states)
        with no allocation. Cite §'NFA-based Regular Expression Algorithms'."
  section: "'NFA-based Regular Expression Algorithms'"

# Reference — existing Rust test conventions in THIS repo
- file: src/core/types.rs
  why: "shows #[derive(Debug, PartialEq)] + inline #[cfg(test)] mod tests with
        assert_eq!. The pattern.rs mod tests already follows this; mirror it."
  pattern: "#[cfg(test)] mod tests { use super::*; ... } with `// --- header ---` groups"
```

### Current Codebase tree (qmkonnect, relevant subset)

```bash
src/
  main.rs                 # CLI entry (unchanged)
  core/
    mod.rs                # Config + helpers; ALREADY has `pub mod pattern;` (S1) — DO NOT TOUCH
    pattern.rs            # S1: process_escapes+consts, ParsedPattern+parse_pattern,
                            #             NfaOp+nfa_compile, + mod tests
                            #   ← EDIT THIS FILE (additive: + predicates, + pattern_char_matches,
                            #                     + nfa_addstate, + nfa_has_match, + nfa_match, + tests)
    notifier.rs           # Notifier trait, debouncer, tests (unchanged)
    types.rs              # WindowInfo (unchanged) — struct/test style reference
  platforms/              # per-OS window monitors (unchanged)
  tray.rs / linux_tray.rs # tray UI (unchanged)
```

### Desired Codebase tree (files this task changes)

```bash
src/
  core/
    pattern.rs            # MODIFIED (additive) — + 5 byte predicates (private, #[inline]),
                            #                     + decoded_literal, + pattern_char_matches,
                            #                     + nfa_addstate, + nfa_has_match, + nfa_match,
                            #                     + tests
    # mod.rs UNCHANGED (module already registered by S1)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (GOTCHA-2 / REFINEMENT A — \\b/\\B see the ORIGINAL string): the
//   single most common porting bug. nfa_addstate's Assert branch MUST call
//   is_word_boundary(string_start, abspos) where string_start is the FULL
//   original string and abspos is ABSOLUTE from its start — NOT a per-offset
//   slice. The linchpin test nfa_match(compile("\\bword"), b"aword", 1, true,
//   false) == FALSE pins this: at abspos 1, 'a' & 'w' are both word chars → no
//   boundary → \\b fails. If you sliced b"aword"[1..]="word" and treated it as
//   string_start (abspos 0), \\b would see 'w' → boundary → pass → TRUE (WRONG).
//   This is WHY nfa_match takes (string, start) not a single str_bytes slice.
//   (PRD §13 #10; firmware nfa_match carries str + string_start separately.)
//
// CRITICAL (GOTCHA-3 — epsilon edges do NOT advance abspos): Split and Assert
//   forward abspos UNCHANGED to their successors. Only the simulator's per-char
//   step advances it (pos + 1 when feeding a consumed Char/Any's out). Forgetting
//   this makes \\b evaluate at the wrong position.
//
// CRITICAL (GOTCHA-6 — empty-string \\b/\\B special case): if the ORIGINAL
//   string is empty, NEITHER \\b NOR \\B passes (do not recurse). Check
//   !string_start.is_empty() BEFORE is_word_boundary — this is INDEPENDENT of
//   the is_word_boundary impl (which would otherwise return false at pos 0 on
//   empty, making \\B WRONGLY pass). Pinned by \\b/""→false AND \\B/""→false.
//
// CRITICAL (GOTCHA-4 — mark seen BEFORE dispatch): set seen[idx] = generation
//   BEFORE recursing into Split/Assert edges, so a state reached via one Split
//   branch isn't re-added when the other converges. Firmware sets lastlist first.
//
// CRITICAL (GOTCHA-11 — bump generation ONCE per phase, NEVER clear seen):
//   increment generation exactly once before building each closure (seed + each
//   consumed char). Do NOT seen.fill(false) per phase — that abandons the O(1)-
//   dedup property (the whole point of the generation tag, PRD §13 #11).
//
// CRITICAL (GOTCHA-10 — generation starts at 1; seen init to 0): seen is
//   vec![0u32; states.len()]; the first `generation += 1` makes it 1, so 0 means
//   "unseen". Starting generation at 0 would make the initial 0s read as "already
//   seen" and the seed closure would be EMPTY → every match wrongly false.
//
// GOTCHA-5 (Assert/Split are NEVER collected): nfa_addstate only pushes
//   Char/Any/Match onto the list. Assert/Split RETURN without pushing (they only
//   conditionally recurse). The simulator's inner loop therefore only ever sees
//   Any/Char on the live list — the `_ => {}` arm is CORRECT, not a bug.
//
// GOTCHA-7 (dot excludes newline AND CR; glob includes them): DOT_META (0x0D)
//   matches sc != '\\n' && sc != '\\r' — BOTH. OP_ANY (glob '*') matches ANY
//   byte INCLUDING '\\n'/'\\r' (PRD §13 #8). Don't conflate.
//
// GOTCHA-8 (ASCII whitespace/digit/word, NOT Unicode): use
//   u8::is_ascii_whitespace() (exactly ' \\t\\n\\r\\f\\v'), NOT char::is_whitespace
//   (adds Unicode ws → diverges on non-ASCII bytes). Same for is_ascii_digit /
//   is_ascii_alphanumeric (+ '_' for word).
//
// GOTCHA-9 (ASCII case-fold; decode-then-fold): case-insensitive compare uses
//   to_ascii_lowercase() on BOTH bytes (== C tolower((unsigned char)) in the C
//   locale; identity for non-letters). For escaped literals (0x01-0x04), DECODE
//   to the literal byte FIRST (decoded_literal), then fold — never fold the
//   placeholder byte itself.
//
// GOTCHA-1 (no defensive NULL guard): the firmware `if (!start) return ...` is
//   DEAD in Rust — nfa_compile (S1) always returns >= [Match], so states[0]
//   always exists. debug_assert!(!states.is_empty()); do NOT add a guard that
//   returns spurious true.
//
// GOTCHA-12 (swap, don't move): use std::mem::swap(&mut clist, &mut nlist)
//   after each step (3-pointer swap, no realloc), then nlist.clear() at the top
//   of the next step. Mirrors the firmware pointer swap.
//
// GOTCHA-13 (full_match early-return placement): the `!full_match && nfa_has_match`
//   early-return appears (a) once after the SEED closure (empty prefix) and
//   (b) once after each step's swap (mid-stream prefix). The FINAL nfa_has_match
//   return handles full_match (accept only at end) AND substring-at-end. Do NOT
//   early-return for full_match=true.
//
// GOTCHA-14 (dead-list break): if clist.is_empty() after a swap, break (no live
//   states can recover). The final nfa_has_match(&[]) → false.
//
// GOTCHA-15 (recursion depth): nfa_addstate is recursive on epsilon edges.
//   Depth is bounded by the longest epsilon chain (<< states.len()); no overflow
//   risk for realistic host patterns. Mirror the firmware's recursive form; an
//   explicit-stack iterative version is unnecessary.
//
// BORROW-CHECKER: nfa_addstate borrows states (shared) + list/seen (exclusive) —
//   all disjoint. The inner loop borrows clist (shared, iterated) while pushing
//   to nlist (exclusive) — different Vecs, no aliasing. std::mem::swap of two
//   Vec<usize> is cheap. Compiles clean with no unsafe, no static, no Cell.
//
// CRATE QUIRK: the crate-wide test command MUST be single-threaded (shared
//   debouncer state in notifier.rs):
//     cargo test --bin qmkonnect -- --test-threads=1   (AGENTS.md)
```

## Implementation Blueprint

### Data models and structure

This task adds NO new public types (it consumes S1's `NfaOp`). The "data" is the
simulator's per-call scratch state, owned by `nfa_match` as locals:

```rust
// Inside nfa_match (locals — no fields, no statics):
let mut clist: Vec<usize> = Vec::with_capacity(states.len());  // current live state indices
let mut nlist: Vec<usize> = Vec::with_capacity(states.len());  // next live state indices
let mut seen: Vec<u32> = vec![0u32; states.len()];             // generation-tag dedup (0 = unseen)
let mut generation: u32 = 0;                                    // bumped once per phase
let mut pos = start;                                            // abspos (absolute offset into `string`)
```

The predicate/helper signatures (all private except `nfa_match`):

```rust
#[inline] fn is_digit_char(c: u8) -> bool;
#[inline] fn is_word_char(c: u8) -> bool;
#[inline] fn is_whitespace_char(c: u8) -> bool;
fn is_word_boundary(string: &[u8], pos: usize) -> bool;
fn decoded_literal(pc: u8) -> u8;                                   // 0x01-0x04 -> literal byte
fn pattern_char_matches(pc: u8, sc: u8, case_sensitive: bool) -> bool;
fn nfa_addstate(states: &[NfaOp], idx: usize, list: &mut Vec<usize>,
                seen: &mut Vec<u32>, generation: u32,
                string_start: &[u8], abspos: usize);
fn nfa_has_match(states: &[NfaOp], list: &[usize]) -> bool;
pub(crate) fn nfa_match(states: &[NfaOp], string: &[u8], start: usize,
                        case_sensitive: bool, full_match: bool) -> bool;
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: ADD the five byte predicates + decoded_literal to src/core/pattern.rs
  - PLACE: after nfa_compile (logical pipeline order: ... -> nfa_compile ->
           nfa_addstate/nfa_match), before the #[cfg(test)] mod tests.
  - IMPLEMENT (verified skeletons in research/notes.md §3a/§3b/§3c):
      * is_digit_char(c)  = c.is_ascii_digit()
      * is_word_char(c)   = c.is_ascii_alphanumeric() || c == b'_'   (GOTCHA-8)
      * is_whitespace_char(c) = c.is_ascii_whitespace()              (GOTCHA-8)
      * is_word_boundary(string, pos): the faithful 4-branch port (pos==0 /
        pos==len / pos>len / XOR of neighbors). NULL guard dropped (no null slices).
      * decoded_literal(pc): match ESC_CARET->b'^', ESC_DOLLAR->b'$',
        ESC_STAR->b'*', ESC_BSLASH->b'\\', _ => pc.   (firmware get_escaped_char
        literal cases only; classes/asserts handled in pattern_char_matches.)
  - VISIBILITY: private fns; #[inline] on the three classifiers (hot path).
  - USE S1 NAMED CONSTS: decoded_literal dispatches on ESC_CARET/ESC_DOLLAR/
           ESC_STAR/ESC_BSLASH (NOT raw 0x01-0x04).
  - DOC: one-line /// on is_word_boundary noting it tests the ORIGINAL string at
         an absolute pos and that the empty-string short-circuit lives in nfa_addstate.

Task 2: IMPLEMENT pattern_char_matches() in src/core/pattern.rs
  - SIGNATURE: fn pattern_char_matches(pc: u8, sc: u8, case_sensitive: bool) -> bool
  - BODY (research/notes.md §3c):
      * if pc in ESC_CARET..=ESC_BSLASH (0x01..=0x04): lit = decoded_literal(pc);
        return case_sensitive ? lit==sc : lit.to_ascii_lowercase()==sc.to_ascii_lowercase().
        (GOTCHA-9: decode THEN fold; never fold the placeholder.)
      * else match pc:
          CLASS_DIGIT  -> is_digit_char(sc)
          CLASS_NDIGIT -> !is_digit_char(sc)
          CLASS_WORD   -> is_word_char(sc)
          CLASS_NWORD  -> !is_word_char(sc)
          CLASS_SPACE  -> is_whitespace_char(sc)
          CLASS_NSPACE -> !is_whitespace_char(sc)
          DOT_META     -> sc != b'\n' && sc != b'\r'   (GOTCHA-7: excludes BOTH)
          _            -> case_sensitive ? pc==sc :
                          pc.to_ascii_lowercase()==sc.to_ascii_lowercase()
  - USE S1 NAMED CONSTS (CLASS_*, DOT_META, ESC_*) for all byte comparisons — no raw hex.
  - PLACEMENT: immediately after the predicates from Task 1.

Task 3: IMPLEMENT nfa_addstate() + nfa_has_match() in src/core/pattern.rs
  - SIGNATURE (research/notes.md §3d):
      fn nfa_addstate(states: &[NfaOp], idx: usize, list: &mut Vec<usize>,
                      seen: &mut Vec<u32>, generation: u32,
                      string_start: &[u8], abspos: usize)
      fn nfa_has_match(states: &[NfaOp], list: &[usize]) -> bool
  - BODY nfa_addstate (verbatim-faithful to the C, research/notes.md §1c + §3d):
      * if seen[idx] == generation { return; }        // dedup (GOTCHA-4: BEFORE dispatch)
      * seen[idx] = generation;
      * match states[idx]:
          Match         -> list.push(idx)
          Split{out,out1} -> recurse BOTH with abspos UNCHANGED (GOTCHA-3)
          Assert{arg,out} -> want_boundary = (arg == ASSERT_BOUND);
                             if !string_start.is_empty()            // GOTCHA-6 empty special case
                                && is_word_boundary(string_start, abspos) == want_boundary
                             { recurse out with abspos UNCHANGED }   // (GOTCHA-2: ORIGINAL string)
                             // NO list.push for Assert (GOTCHA-5)
          Char{..} | Any{..} -> list.push(idx)        // consuming state: collect
  - BODY nfa_has_match: list.iter().any(|&idx| states[idx] == NfaOp::Match)
  - GOTCHA: Assert/Split NEVER collected (GOTCHA-5). abspos UNCHANGED across
            epsilon edges (GOTCHA-3). Mark seen BEFORE dispatch (GOTCHA-4).
  - PLACEMENT: after pattern_char_matches.

Task 4: IMPLEMENT nfa_match() in src/core/pattern.rs
  - SIGNATURE (REFINEMENT A + B — research/notes.md §3e):
      pub(crate) fn nfa_match(states: &[NfaOp], string: &[u8], start: usize,
                              case_sensitive: bool, full_match: bool) -> bool
  - BODY (verified skeleton, research/notes.md §3e):
      * debug_assert!(!states.is_empty())   // S1 invariant; GOTCHA-1 (no NULL guard)
      * locals: clist/nlist Vec<usize> with_capacity(states.len());
        seen = vec![0u32; states.len()]; generation = 0u32; pos = start.
      * generation += 1; nfa_addstate(states, 0, &mut clist, &mut seen, generation,
        string, pos);                                  // seed from states[0]
      * if !full_match && nfa_has_match(states, &clist) { return true; }  // empty prefix
      * while pos < string.len():
          c = string[pos];
          generation += 1; nlist.clear();               // fresh phase (GOTCHA-11)
          for &s in &clist:
            match states[s]:
              Any{out}     -> nfa_addstate(states, out, &mut nlist, &mut seen,
                                           generation, string, pos + 1)   // ANY byte (GOTCHA-7)
              Char{arg,out}-> if pattern_char_matches(arg, c, case_sensitive) {
                                nfa_addstate(states, out, &mut nlist, &mut seen,
                                             generation, string, pos + 1) }
              _ => {}                                   // Match/Assert/Split not on live list (GOTCHA-5)
          std::mem::swap(&mut clist, &mut nlist);       // GOTCHA-12
          pos += 1;
          if clist.is_empty() { break; }                // GOTCHA-14
          if !full_match && nfa_has_match(states, &clist) { return true; }  // GOTCHA-13
      * return nfa_has_match(states, &clist)            // full_match: accept at end
  - GOTCHA: generation starts at 1 (first += 1); seen init 0 (GOTCHA-10).
            Early-return ONLY when !full_match (GOTCHA-13).
  - PLACEMENT: after nfa_addstate.

Task 5: ADD rustdoc (Mode A — code-level docs only)
  - ON nfa_match (a /// block):
      * it runs a compiled NFA (S1 nfa_compile output; start = states[0]) against
        `string` beginning at byte `start`.
      * `string` is the FULL ORIGINAL input (firmware string_start) so \\b/\\B
        evaluate at an absolute offset (PRD §13 #10); `start` is the consume offset.
      * full_match=false ⇒ Match reachable at any point (prefix/substring) → true;
        full_match=true ⇒ Match reachable only after consuming the whole remaining
        string (exact/suffix).
      * THE LINEAR-TIME GUARANTEE: two-list Thompson simulation (clist/nlist
        swapped per step) with generation-tag O(1) dedup (seen[idx]==generation,
        bumped once per phase, never cleared) ⇒ O(states × consumed_len), no
        backtracking, no allocation in the hot loop. The fix for the old
        exponential matcher (PRD §7.8). Reference Russ Cox,
        https://swtch.com/~rsc/regexp/regexp1.html (PRD §7.5/§7.9).
      * stage 4 of the pipeline (parse_pattern -> process_escapes -> nfa_compile
        -> nfa_addstate/nfa_match -> match_with_anchors); the anchor strategy is T3.S1.
  - ON nfa_addstate: /// noting it follows epsilon edges (Split both branches,
        Assert conditional on is_word_boundary vs the ORIGINAL string), collects
        Char/Any/Match, dedups via the generation tag, and that Assert/Split are
        never themselves collected.
  - ON pattern_char_matches: /// noting decode-then-ASCII-fold for escaped literals,
        the class switch, dot-excludes-newline, and that ASCII folding ==
        C tolower in the C locale.
  - REFERENCE arch external_deps.md §3 points 4-5 and PRD §4 as the contract sources.

Task 6: APPEND nfa_match unit tests to the existing mod tests in pattern.rs
  - DO: add new #[test] fns INTO the existing `mod tests { use super::*; ... }`
        block. Group with the same `// --- header ---` comment style.
  - IMPLEMENT one #[test] per group of research/notes.md §4 (~30 vectors). Suggested:
      // --- Exact full-string (full_match=true) ---               (§4.1, 7 rows)
      // --- Prefix / substring (full_match=false) ---             (§4.2, 7 rows)
      // --- Quantifier + (Char+SPLIT loop) ---                    (§4.3, 4 rows)
      // --- Character classes + dot ---                           (§4.4, 10 rows)
      // --- Word boundary \\b (THE abspos/linchpin tests) ---     (§4.5, 7 rows)
      // --- Empty-string \\b/\\B special case ---                 (§4.6, 2-3 rows)
  - COMPILE patterns via process_escapes + nfa_compile (realistic integration):
        let nfa = nfa_compile(&process_escapes("\\bword"));
        assert!(!nfa_match(&nfa, b"aword", 1, true, false));   // linchpin (GOTCHA-2)
        assert!( nfa_match(&nfa, b" word", 1, true, false));
        assert!( nfa_match(&nfa, b"word",  0, true, false));
  - HIGHLIGHT the linchpin test with a comment explaining WHY (\\b sees the
        ORIGINAL string at abspos; a slice-based impl would wrongly return true).
  - HIGHLIGHT the empty-string \\b AND \\B → false tests (GOTCHA-6).
  - HIGHLIGHT dot-vs-glob: `.` on b"\\n" → false; `*` matches b"\\n" (test via a
        glob pattern that must consume a newline, e.g. "*" vs b"a\\nb" full_match=true).
  - HIGHLIGHT \\b\\b vs b"a" → true (generation dedup terminates; GOTCHA-4/11).
  - NAMING: test_match_<behavior> (e.g. test_match_exact_whole_string,
        test_match_prefix_at_offset, test_match_plus_needs_one,
        test_match_bword_sees_original_string_at_offset, test_match_b_empty_false,
        test_match_B_empty_false, test_match_dot_excludes_newline,
        test_match_glob_includes_newline, test_match_bb_generation_dedup_terminates).
  - COVERAGE: every opcode (Char/Any/Split/Assert/Match), both full_match modes,
        case on/off, all 6 classes, dot, +, glob, \\b/\\B incl. empty special case.

Task 7: VERIFY build + tests (single-threaded, per AGENTS.md)
  - RUN: cargo build --bin qmkonnect        (expect: clean; no NEW warnings)
  - RUN: cargo test --bin qmkonnect pattern -- --test-threads=1
         (expect: ALL pattern::tests pass — new nfa_match tests AND S1
          process_escapes/parse_pattern/nfa_compile tests)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1
         (expect: full crate green — no regression)
  - IF a parity assertion fails: re-read the C source (pattern_match.c
    nfa_addstate/nfa_match/is_word_boundary) line-by-line + research/notes.md §3
    skeleton; the table is a faithful transcription, so a failure means the Rust
    diverged. Do NOT "fix" the test to match the Rust — fix the Rust to match the
    firmware (PRD §14: firmware is source of truth).
  - CONFIRM git status shows ONLY src/core/pattern.rs modified.
```

### Implementation Patterns & Key Details

```rust
// The canonical nfa_addstate body (this IS the spec — match it exactly).
// Full verified version in research/notes.md §3d.
//
// fn nfa_addstate(states: &[NfaOp], idx: usize, list: &mut Vec<usize>,
//                 seen: &mut Vec<u32>, generation: u32,
//                 string_start: &[u8], abspos: usize) {
//     if seen[idx] == generation { return; }          // dedup (GOTCHA-4: before dispatch)
//     seen[idx] = generation;
//     match states[idx] {
//         NfaOp::Match => list.push(idx),
//         NfaOp::Split { out, out1 } => {              // epsilon fork, abspos UNCHANGED (GOTCHA-3)
//             nfa_addstate(states, out,  list, seen, generation, string_start, abspos);
//             nfa_addstate(states, out1, list, seen, generation, string_start, abspos);
//         }
//         NfaOp::Assert { arg, out } => {              // zero-width \b/\B
//             let want_boundary = arg == ASSERT_BOUND;
//             if !string_start.is_empty()             // GOTCHA-6: empty -> neither passes
//                 && is_word_boundary(string_start, abspos) == want_boundary  // GOTCHA-2: ORIGINAL
//             {
//                 nfa_addstate(states, out, list, seen, generation, string_start, abspos);
//             }
//             // Assert is NEVER collected (GOTCHA-5)
//         }
//         NfaOp::Char { .. } | NfaOp::Any { .. } => list.push(idx),  // consuming: collect
//     }
// }
//
// The canonical nfa_match body (research/notes.md §3e):
//
// pub(crate) fn nfa_match(states: &[NfaOp], string: &[u8], start: usize,
//                         case_sensitive: bool, full_match: bool) -> bool {
//     debug_assert!(!states.is_empty());              // GOTCHA-1: no NULL guard
//     let mut clist: Vec<usize> = Vec::with_capacity(states.len());
//     let mut nlist: Vec<usize> = Vec::with_capacity(states.len());
//     let mut seen: Vec<u32> = vec![0u32; states.len()]; // 0 = unseen (GOTCHA-10)
//     let mut generation: u32 = 0;
//     let mut pos = start;                             // abspos
//
//     generation += 1;                                 // seed (GOTCHA-11: once per phase)
//     nfa_addstate(states, 0, &mut clist, &mut seen, generation, string, pos);
//     if !full_match && nfa_has_match(states, &clist) { return true; }  // empty prefix
//
//     while pos < string.len() {
//         let c = string[pos];
//         generation += 1; nlist.clear();              // fresh phase, NO seen clear (GOTCHA-11)
//         for &s in &clist {
//             match states[s] {
//                 NfaOp::Any { out } => nfa_addstate(states, out, &mut nlist, &mut seen,
//                                                    generation, string, pos + 1),  // ANY byte
//                 NfaOp::Char { arg, out } if pattern_char_matches(arg, c, case_sensitive) =>
//                     nfa_addstate(states, out, &mut nlist, &mut seen, generation, string, pos + 1),
//                 _ => {}                              // Match/Assert/Split not live (GOTCHA-5)
//             }
//         }
//         std::mem::swap(&mut clist, &mut nlist);      // GOTCHA-12
//         pos += 1;
//         if clist.is_empty() { break; }               // GOTCHA-14
//         if !full_match && nfa_has_match(states, &clist) { return true; }  // GOTCHA-13
//     }
//     nfa_has_match(states, &clist)                    // full_match: accept at end
// }
```

### Integration Points

```yaml
MODULE REGISTRATION:
  - NONE. `pub mod pattern;` is already in src/core/mod.rs (added by S1). Do NOT
    edit mod.rs.

DEPENDENCIES (this task): NONE new. Pure stdlib Vec/usize/u32 + std::mem::swap +
                           references to the in-module S1 named consts + S1's NfaOp.
                           No Cargo deps, no qmk_notifier crate, no firmware link,
                           no `unsafe`, no `static`.

UPSTREAM (already present — S1 + S1-parallel contract):
  - pub(crate) const ESC_CARET..GLOB_STAR (top of pattern.rs, S1) — use these for
    byte comparisons, NOT raw hex.
  - #[derive(Debug,Clone,PartialEq)] pub(crate) enum NfaOp { Char{arg,out}, Any{out},
    Split{out,out1}, Assert{arg,out}, Match } (S1) — the states this simulator walks.
  - pub(crate) fn nfa_compile(pat: &[u8]) -> Vec<NfaOp> (S1) — produces the compiled
    NFA; start state is ALWAYS states[0] (S1's start==0 invariant). The simulator
    seeds its closure from states[0].

DOWNSTREAM CONSUMERS (later subtasks — do NOT implement them here):
  - P2.M1.T3.S1 match_with_anchors(): compiles a pattern once, then calls nfa_match
    at each offset to implement the four anchor strategies:
      ^...$  -> nfa_match(core, str, 0, cs, true)              (exact, full_match=true)
      ^...   -> nfa_match(core, str, 0, cs, false)             (prefix, full_match=false)
      ...$   -> for i in 0..=len { nfa_match(core, str, i, cs, true) }   (suffix)
      ...    -> for i in 0..=len { nfa_match(core, str, i, cs, false) }  (substring)
    The `start` param (REFINEMENT A) is what makes the offset loops correct for \\b/\\B.
  - P2.M1.T4.S1: ports the full firmware end-to-end test corpus as parity tests
    (exercising match_with_anchors -> nfa_match end-to-end).

CONFIG: none.
ROUTES: none (no CLI surface in this subtask).
DATABASE: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles clean. If rustc warns on src/core/pattern.rs, READ it and fix.
# The file carries #![allow(dead_code)] from S1, so the new pub(crate) nfa_match
# won't spuriously warn even though its only non-test consumer (match_with_anchors)
# is a later subtask.

# Confirm the additions are present:
grep -n 'fn nfa_match' src/core/pattern.rs           # expect one pub(crate) def
grep -n 'fn nfa_addstate' src/core/pattern.rs        # expect one def
grep -n 'fn pattern_char_matches' src/core/pattern.rs # expect one def
grep -n 'fn is_word_boundary' src/core/pattern.rs    # expect one def
grep -n 'fn nfa_has_match' src/core/pattern.rs       # expect one def
grep -n 'seen\[idx\] == generation' src/core/pattern.rs  # the dedup guard
```

### Level 2: Unit Tests — the parity contract (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# Single-threaded is MANDATORY crate-wide (shared debouncer state in notifier.rs).
cargo test --bin qmkonnect pattern -- --test-threads=1
# Expected: every test in pattern::tests passes — the new nfa_match tests
# (research/notes.md §4, ~30 vectors) AND S1's process_escapes/parse_pattern/
# nfa_compile tests. A failure means the Rust diverged from the firmware C —
# fix the Rust, not the test.
# Filter to just the new tests to see them individually:
cargo test --bin qmkonnect pattern::tests::test_match -- --test-threads=1
```

### Level 3: Full-crate regression (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL bin tests green — pattern::tests (new nfa_match + S1 + S2) +
# notifier + types. Proves the additive edit didn't break module resolution and
# compiles in the full crate context.

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
# lives in the OTHER repo). The Rust nfa_match parity vectors in research/notes.md
# §4 were DERIVED from these C tests, so a green firmware run corroborates the
# contract the Rust port encodes. The Rust tests are STRICTLY STRONGER: they
# exercise nfa_match at specific (start, full_match) offsets, isolating simulator
# bugs from anchor-strategy bugs.
cd /home/dustin/projects/qmk-notifier && ./run_all_tests.sh
# Expected: the full pattern_match + word_boundary corpus passes (it always does —
# this task does not touch the firmware). Particularly: test_word_boundary_basic's
# `{"\\bword","aword",true,false}` and `{"\\b","","true,false}` and `{"\\B","","true,false}`
# — the exact semantics the Rust linchpin tests encode.
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean (no NEW warnings on `src/core/pattern.rs`).
- [ ] `cargo test --bin qmkonnect pattern -- --test-threads=1` — all pattern tests pass (new + S1 + S2).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` — full crate green.
- [ ] `git status` shows exactly ONE modified file: `src/core/pattern.rs`.

### Feature Validation (parity)
- [ ] Every row of `research/notes.md` §4 (~30 vectors) asserted and passing.
- [ ] **Linchpin** (GOTCHA-2/REFINEMENT A): `nfa_match(compile("\\bword"), b"aword", 1, true, false)` == `false` (\\b sees original at abspos 1).
- [ ] **\\b passes at a real boundary**: `nfa_match(compile("\\bword"), b" word", 1, true, false)` == `true`.
- [ ] **\\B non-boundary**: `nfa_match(compile("\\Bord"), b"word", 1, true, false)` == `true`; `…(b"ord", 0, …)` == `false`.
- [ ] **Empty-string special case** (GOTCHA-6): `\\b`→false AND `\\B`→false against `b""`.
- [ ] **Dot vs glob** (GOTCHA-7): `.` on `b"\n"`/`b"\r"` → false; glob `*` consumes newline.
- [ ] **Classes**: `\d`/`\D`/`\w`/`\W`/`\s`/`\S` each match/negate correctly.
- [ ] **Quantifier +** (GOTCHA linear): `a+` vs `b"aaa"` full → true; vs `b""` → false.
- [ ] **full_match modes**: exact (`^…$`-style) vs prefix/substring both correct.
- [ ] **Case sensitivity**: `abc` vs `ABC` matches iff `!case_sensitive`.
- [ ] **Generation dedup terminates** (GOTCHA-4/11): `\\b\\b` vs `b"a"` → true.

### Code Quality Validation
- [ ] `nfa_match` is `pub(crate)`, takes `(&[NfaOp], &[u8], usize, bool, bool)`, returns `bool`.
- [ ] The three refinements applied: `start: usize` (A), `states: &[NfaOp]` compiled (B), `Vec<u32>` generation (C).
- [ ] Predicates + `pattern_char_matches` + `nfa_addstate` + `nfa_has_match` are private.
- [ ] NO `static`; NO `unsafe`; NO per-phase `seen.clear()`/`fill` (GOTCHA-11).
- [ ] `seen` init `vec![0u32; states.len()]`; generation starts at 1 (GOTCHA-10).
- [ ] Assert/Split never collected (GOTCHA-5); abspos unchanged across epsilon edges (GOTCHA-3).
- [ ] Uses S1 named consts (`CLASS_*`, `DOT_META`, `ESC_*`, `ASSERT_*`) for byte comparisons — no raw hex in pattern_char_matches/decoded_literal.
- [ ] Rustdoc on `nfa_match` cites Russ Cox + the linear-time guarantee + `full_match` + the original-string `\\b` rationale.
- [ ] New tests appended to the existing `mod tests` (S1/S2/nfa_compile tests untouched), grouped with `// --- header ---` comments.
- [ ] No new Cargo dependencies.
- [ ] Scope respected: NO `match_with_anchors`, NO public `pattern_match`, NO delimiter-aware `match_pattern`, NO `Pattern::Single|Parts`, NO `mod.rs` edit.

### Documentation & Deployment
- [ ] Code-level rustdoc present (Mode A — no `docs/*.md` or README changes this task).
- [ ] `nfa_match` + `nfa_addstate` + `pattern_char_matches` documented with their firmware meaning + the refinements.

---

## Anti-Patterns to Avoid

- ❌ Do NOT make `\b`/`\B` evaluate against a per-offset slice. They MUST see the
      ORIGINAL string at an absolute offset (GOTCHA-2/REFINEMENT A). The linchpin
      test `\bword`/`aword`@1 → false pins it. A single `str_bytes` slice can't
      carry both — that's why `nfa_match` takes `(string, start)`.
- ❌ Do NOT advance `abspos` across epsilon edges. Split/Assert forward it
      UNCHANGED; only the per-char step does `pos + 1` (GOTCHA-3).
- ❌ Do NOT mark `seen[idx] = generation` AFTER dispatching. Mark it BEFORE, so
      Split's two converging branches don't double-add (GOTCHA-4).
- ❌ Do NOT clear `seen` (`fill(false)`/`clear()`) per phase. Bump `generation`
      once per phase; the stale tags become invisible (GOTCHA-11). This is the
      O(1)-dedup property.
- ❌ Do NOT start `generation` at 0 with `seen` zero-initialized. The first
      `generation += 1` makes it 1; 0 means "unseen". Starting at 0 makes every
      state read as already-seen → empty closure → all matches wrongly false
      (GOTCHA-10).
- ❌ Do NOT collect Assert/Split onto the live list. Only Char/Any/Match are
      pushed; Assert/Split only conditionally recurse then return (GOTCHA-5). The
      inner loop's `_ => {}` arm is correct.
- ❌ Do NOT use `char::is_whitespace()` / Unicode folding. Use the `u8::is_ascii_*`
      methods and `to_ascii_lowercase` (GOTCHA-8/9) — the firmware uses C
      `tolower`/range checks which are ASCII-only in the C locale.
- ❌ Do NOT fold the escaped-literal placeholder byte. Decode it to the literal
      first (`decoded_literal`), THEN fold (GOTCHA-9).
- ❌ Do NOT conflate dot and glob. `DOT_META` excludes `\n` AND `\r`; `Any` (glob)
      matches any byte INCLUDING them (GOTCHA-7).
- ❌ Do NOT early-return `true` inside the loop when `full_match == true`. The
      early-return is gated on `!full_match`; full_match accepts only at the end
      (GOTCHA-13).
- ❌ Do NOT add a NULL/empty-start defensive guard that returns spurious `true`.
      `nfa_compile` (S1) always yields `>= [Match]`; `debug_assert!` instead
      (GOTCHA-1).
- ❌ Do NOT take raw processed bytes as `nfa_match`'s first param and recompile
      internally. Take the compiled `&[NfaOp]` (REFINEMENT B) — compile once,
      simulate many.
- ❌ Do NOT hardcode `0x05`/`0x0D`/`0x01` etc. in `pattern_char_matches` /
      `decoded_literal`. Use the S1 named consts (`CLASS_*`, `DOT_META`, `ESC_*`).
- ❌ Do NOT implement `match_with_anchors`, the public `pattern_match()`, the
      delimiter-aware `match_pattern`, or `Pattern::Single|Parts` — those are
      P2.M1.T3.
- ❌ Do NOT edit `src/core/mod.rs` — `pub mod pattern;` is already there (S1).
- ❌ Do NOT change the test to match divergent Rust output. The firmware C
      (`pattern_match.c` `nfa_addstate`/`nfa_match`/`is_word_boundary`) is the
      source of truth (PRD §14); fix the Rust.
- ❌ Do NOT run tests multi-threaded — the crate shares debouncer state
      (`cargo test --bin qmkonnect -- --test-threads=1`).
- ❌ Do NOT edit `PRD.md`, any `tasks.json`, `prd_snapshot.md`, or any `plan/` file
      other than this item's own `PRP.md` + `research/`.

---

## Confidence Score: 9/10

This is a well-bounded port of ~150 lines of heavily-commented firmware C whose
ENTIRE source for all seven functions (`nfa_addstate`, `nfa_match`, `nfa_has_match`,
`pattern_char_matches`, `get_escaped_char`, the three classifiers,
`is_word_boundary`) is reproduced VERBATIM in `research/notes.md` §1, and
transcribed as verified Rust skeletons in §3 (mirrored in the Implementation
Blueprint). The three signature refinements (A: `start` offset; B: compiled
`&[NfaOp]`; C: `Vec<u32>` generation tags) are each FORCED by correctness and
fully derived in §2 — REFINEMENT A even carries a concrete failing-parity-vector
proof (the `\bword`/`aword` linchpin), exactly as S1's `Any { out }` refinement
was justified. ~30 `nfa_match`-level parity vectors derived from the firmware
test corpus (`test_pattern_match.c`, `test_word_boundary_basic.c`) are provided
as the test contract, including the linchpin `\b`-sees-original cases, the
empty-string special case, dot-vs-glob, and the generation-dedup-terminates case.
Because the Rust port makes `nfa_match` `pub(crate)` (vs. `static` in C), the
unit tests are STRICTLY STRONGER than the firmware's end-to-end-only coverage —
they exercise the simulator at specific (start, full_match) offsets, isolating
simulator bugs from anchor-strategy bugs (which T4 ports end-to-end). The
upstream S1 contract (NfaOp shape, nfa_compile output, named consts, start==0
invariant) and downstream T3.S1 contract (how match_with_anchors calls nfa_match
in its four offset loops) are both explicit. No new deps, no `unsafe`, no
`static`. The 1-point reservation is for the (unlikely) event an implementer
mishandles the `\b`-original-string invariant or the generation-tag dedup despite
the explicit callouts and linchpin tests; both are caught immediately by the
parity tests. Scope is cleanly bounded from S1 (upstream compiler), T3.S1
(anchor strategy), and T4 (full corpus), so there is no risk of over- or
under-building.
# PRP — P2.M1.T2.S1: Port NFA compiler (`nfa_compile`) + state types

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This is an **additive edit** to the existing
> greenfield module `src/core/pattern.rs` (created by P2.M1.T1.S1 `process_escapes`,
> extended in parallel by P2.M1.T1.S2 `parse_pattern`). This task adds the
> **Thompson-NFA compiler** — the `NfaOp` state enum + `nfa_compile()` — that turns
> the processed-pattern byte stream (`parse_pattern().core`) into an index-based
> NFA state list consumed by the simulator (P2.M1.T2.S2). It is the **first**
> subtask of the P2.M1.T2 "Thompson NFA Engine" milestone (full firmware-parity
> matcher, PRD §4 + §14; firmware `pattern_match.c` is the single source of truth).

---

## Goal

**Feature Goal**: Port the firmware `nfa_compile(const char *pat, State *pool,
int *nstates_out)` (`qmk-notifier/pattern_match.c`, ~lines 365–430) and its
`State` struct + opcode enum (~lines 300–345) to Rust as **index-based** types —
`enum NfaOp` (no raw pointers, per the item-spec mandate) + `fn nfa_compile(pat:
&[u8]) -> Vec<NfaOp>` — with **identical Thompson-construction edge structure**
for every input. The compiler walks the processed-pattern bytes and emits the
state list: `0x2A` glob → SPLIT+ANY loop; `0x0B/0x0C` → zero-width ASSERT; a
consuming byte followed by `0x0E` → CHAR+SPLIT loop-back (the linear `X+`); any
other consuming byte → plain CHAR; append MATCH at the end.

**Deliverable**: additions to `src/core/pattern.rs` (do NOT recreate the file):
1. `pub(crate) enum NfaOp { Char { arg: u8, out: usize }, Any { out: usize },
   Split { out: usize, out1: usize }, Assert { arg: u8, out: usize }, Match }`
   with `#[derive(Debug, Clone, PartialEq)]`;
2. two private helpers: `enum DanglingExit { Start, Out(usize), SplitSecond(usize) }`
   (the index-based `tail`) and `fn resolve(&mut [NfaOp], &mut Option<usize>,
   &DanglingExit, usize)` (the `*tail = entry` analog), plus `impl NfaOp {
   fn set_out(&mut self, idx); fn set_out1(&mut self, idx) }`;
3. `pub(crate) fn nfa_compile(pat: &[u8]) -> Vec<NfaOp>`;
4. rustdoc on `NfaOp` + `nfa_compile` (Mode A) referencing Russ Cox's paper and
   explaining the linear-time guarantee (`X+` ⇒ exactly 2 states);
5. new `#[test]` fns appended to the existing `#[cfg(test)] mod tests` block,
   asserting the exact `Vec<NfaOp>` for the 18-row parity table in
   `research/notes.md` §6.

**Success Definition**:
- For every processed-pattern byte input, the Rust `Vec<NfaOp>` has the **same
  edge graph** as the firmware `State` pool (same opcodes, same `out`/`out1`
  targets modulo the documented glob-allocation reorder). The 18-row parity
  table (`research/notes.md` §6) is the contract and must all pass.
- `cargo build --bin qmkonnect` compiles clean (no new warnings).
- `cargo test --bin qmkonnect -- --test-threads=1` passes (new `nfa_compile`
  tests AND the existing S1 `process_escapes` / S2 `parse_pattern` tests).
- `git diff` touches ONLY `src/core/pattern.rs`. No simulator (`nfa_match`/
  `nfa_addstate`), no `match_with_anchors`, no `pattern_match` entry, no `mod.rs`
  change — those are later subtasks.

## User Persona (if applicable)

**Target User**: The downstream simulator in the `pattern` module itself —
`nfa_match()` / `nfa_addstate()` (P2.M1.T2.S2), which seeds its epsilon-closure
from `states[0]` (the start state — always index 0, see GOTCHA-1) and walks the
NFA edges. Not a user-facing API.

**Use Case**: Turn the processed-pattern byte slice (`parse_pattern().core`, e.g.
`[0x61, 0x0E]` for `a+`) into a compiled NFA the simulator can run against a
candidate string in guaranteed O(states × strlen) with no backtracking. Example:
`nfa_compile(&[0x61, PLUS_QUANT])` → `vec![Char{arg:0x61,out:1},
Split{out:0,out1:2}, Match]` (the 2-state `a+` loop).

**Pain Points Addressed**: Establishes the compiled-program stage so the
simulator receives a correct, index-based state graph. A wrong loop-back edge
(e.g. `Any.out` pointing forward instead of back to the SPLIT) would make `*`
match greedily-once-and-stop; a missing `X+` 2-state collapse would reintroduce
the exponential backtracking the firmware explicitly fixed (PRD §7.8).

## Why

- **Stage 3 of the full-parity pipeline.** PRD §14 mandates the Rust matcher be a
  *"full-parity port of the firmware `pattern_match.c`, not a subset"* with
  *"the firmware matcher + its test corpus the single source of truth for match
  semantics."* `nfa_compile` is the compile stage between `parse_pattern().core`
  (T1.S2) and the NFA simulator (T2.S2). The firmware architecture doc
  (`external_deps.md` §3 point 3) and the C source are the spec.
- **The linear-time guarantee lives here.** `X+` MUST compile to exactly 2 states
  (CHAR + SPLIT loop-back) so `a+a+a+…b` scales as 2k+1 — never 2^k. This is the
  whole reason the firmware moved off the old backtracker (PRD §7.8). The parity
  table row 11 (`a+a+a+` ⇒ 7 states) pins it.
- **Host-side rules, no reflash.** Per P2/P3/P4, rules move to a host-side
  `rules.toml`; the matcher runs on the desktop. This task ports the compiler
  those rules' patterns are run through.

## What

Add to `src/core/pattern.rs` (the file S1 created, S2 extends in parallel — both
already present or assumed-present by contract; APPEND, do NOT recreate):

1. **`NfaOp` enum** — index-based Thompson node, derived for testability:
   ```rust
   #[derive(Debug, Clone, PartialEq)]
   pub(crate) enum NfaOp {
       Char { arg: u8, out: usize },      // consume one byte matching `arg`; go to `out`
       Any { out: usize },                 // consume ANY byte incl \n/\r (glob `*`); go to `out`
       Split { out: usize, out1: usize },  // epsilon fork: follow both `out` and `out1`
       Assert { arg: u8, out: usize },     // zero-width \b(0x0B)/\B(0x0C); go to `out` if condition holds
       Match,                              // accepting (terminal)
   }
   ```
   `out`/`out1` mirror the firmware `State.out`/`State.out1` field names exactly.
   `lastlist` is deliberately **absent** — the compiled program is immutable;
  the simulator (T2.S2) owns dedup state separately (GOTCHA-5).
2. **`DanglingExit` enum + `resolve()` + `impl NfaOp { set_out, set_out1 }`** —
   the index-based translation of the firmware `State **tail` threading (§3 of
   notes). Private (no `pub`).
3. **`nfa_compile(pat: &[u8]) -> Vec<NfaOp>`** implementing the algorithm below
   (verbatim-faithful to the C, with the single documented glob-allocation
   reorder that keeps `start == 0`).
4. **Rustdoc** on `NfaOp` (opcode semantics) and `nfa_compile` (the linear-time
   guarantee + Russ Cox reference + the start-index-0 invariant).
5. **Unit tests** appended to the existing `mod tests` block.

### The algorithm (authoritative: C source + `research/notes.md` §1, §5)

```text
states = Vec::with_capacity(pat.len()*2 + 2)
start = None
tail = DanglingExit::Start          # index-based analog of firmware `State **tail`
i = 0
while i < pat.len():
    b = pat[i]
    if b == 0x2A (GLOB_STAR):                            # (a) glob '*' == regex '.*'
        sp_idx  = states.len(); push Split{out:_, out1:_}   # allocate SPLIT FIRST (reorder ⇒ entry=0)
        any_idx = states.len(); push Any{out:_}
        states[sp_idx].set_out(any_idx)    # sp.out  = ANY
        states[any_idx].set_out(sp_idx)    # any.out = sp  (loop back)
        resolve(..., tail, sp_idx)         # *tail = sp  (entry)
        tail = SplitSecond(sp_idx)         # exit via sp.out1
        i += 1
    elif b == 0x0B or 0x0C (ASSERT_BOUND/ASSERT_NBOUND): # (b) \b / \B zero-width
        a_idx = states.len(); push Assert{arg:b, out:_}
        resolve(..., tail, a_idx)
        tail = Out(a_idx)                  # exit via a.out
        i += 1
    elif b == 0x0E (PLUS_QUANT):                          # (c) stray 0x0E — skip (never emitted standalone)
        i += 1
    else:                                                 # (d) consuming element X
        c_idx = states.len(); push Char{arg:b, out:_}
        if i+1 < pat.len() and pat[i+1] == 0x0E:          # X+ : LINEAR (2 states)
            sp_idx = states.len(); push Split{out:c_idx, out1:_}
            states[c_idx].set_out(sp_idx)  # after one X -> split
            resolve(..., tail, c_idx)      # *tail = c (entry)
            tail = SplitSecond(sp_idx)     # exit via split.out1
            i += 2                          # consume X AND the 0x0E marker   (GOTCHA-3)
        else:
            resolve(..., tail, c_idx)
            tail = Out(c_idx)              # exit via c.out
            i += 1
# (e) End: append the single accepting state into the final dangling slot.
m_idx = states.len(); push Match
resolve(..., tail, m_idx)
debug_assert_eq!(start, Some(0))           # invariant: first unit's entry is always index 0
return states   # start is implicitly states[0]
```

### Success Criteria
- [ ] `pub(crate) enum NfaOp` exists with the 5 variants above and `#[derive(Debug, Clone, PartialEq)]`.
- [ ] `pub(crate) fn nfa_compile(pat: &[u8]) -> Vec<NfaOp>` exists.
- [ ] Every row of `research/notes.md` §6 (18 inputs) passes as a direct `Vec<NfaOp>` assertion.
- [ ] GOTCHA-1 (start==0): rows 4/6 (`b"*"`, `b"*a"`) compile with `Split` at index 0 (glob SPLIT-first reorder).
- [ ] GOTCHA-2 (glob loop-back): `Any.out` points BACK to the SPLIT (rows 4–7).
- [ ] GOTCHA-3 (X+ two bytes): rows 8–11 — `i += 2`, no stray 0x0E re-emitted.
- [ ] GOTCHA-6 (empty → single Match): `nfa_compile(b"")` == `vec![NfaOp::Match]`.
- [ ] Linear guarantee: row 11 `a+a+a+` == exactly 7 states (2·3+1), pinned.
- [ ] Rustdoc references Russ Cox + explains 2-state `X+` / linear time.
- [ ] Calls NO new deps; pure stdlib `Vec`/index work; no `unsafe`; no `static`.
- [ ] `cargo build --bin qmkonnect` clean; `cargo test --bin qmkonnect -- --test-threads=1` green.
- [ ] `git diff` touches only `src/core/pattern.rs`.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge of this codebase can implement this
using only this PRP + the firmware repo, because (a) the entire C function +
`State` struct + opcodes are reproduced verbatim in `research/notes.md` §1, (b)
the index-based design decision (explicit `out` fields, the `DanglingExit` tail,
the start==0 reorder) is fully derived and justified in §2–§4, (c) a verified
Rust skeleton is given in §5 (mirrored in the Implementation Blueprint), (d) 18
exact `Vec<NfaOp>` vectors are provided as the test contract (§6) including the
exponential-backtracker killer `a+a+a+`, (e) the 8 gotchas are enumerated with
the specific test rows that pin each, (f) the upstream S1/S2 contract being
consumed (`process_escapes` consts, `parse_pattern().core` shape) and the
downstream T2.S2 contract (start=index 0, simulator owns dedup) are both
explicit. See `research/notes.md`.

### Documentation & References

```yaml
# MUST READ — the canonical C source (single source of truth, PRD §14)
- file: /home/dustin/projects/qmk-notifier/pattern_match.c
  why: "nfa_compile() at ~lines 365-430 is the function being ported, verbatim.
        The State struct + OP_* opcode enum + NFA_MAX_STATES sizing at ~lines
        300-345 are the types being ported. The State**tail threading pattern
        (*tail = entry; tail = &sp->out1 / &c->out) is the spec. Reproduced in
        research/notes.md §1."
  pattern: "for (*p=pat; *p; p++) dispatch on byte: 0x2A -> SPLIT+ANY loop;
            0x0B/0x0C -> ASSERT; 0x0E standalone -> continue; else CHAR, and if
            p[1]==0x0E emit CHAR+SPLIT loop-back (p++ to consume marker). End:
            append MATCH into the final dangling slot; zero lastlist on all."
  gotcha: "the firmware allocates ANY-then-SPLIT for the glob, so a LEADING glob
           has start index 1 (not 0). The Rust port REORDERS to SPLIT-then-ANY so
           start is always 0 (semantically equivalent; only node indices swap).
           See research/notes.md §4, GOTCHA-1. Also: drop the NFA_MAX_STATES
           clamp (a Vec grows; the clamp corrupts on overflow by design)."

# MUST READ — the file THIS task edits (S1's deliverable, in-tree; S2 extends it)
- file: src/core/pattern.rs
  why: "the module S1 created + S2 extends. It already has: pub(crate) const
        ESC_CARET..GLOB_STAR (0x01-0x0E, 0x2A), pub(crate) fn process_escapes
        (~line 85), pub(crate) struct ParsedPattern + fn parse_pattern (S2), and
        #[cfg(test)] mod tests { use super::*; ... }. APPEND the NfaOp enum +
        nfa_compile fn + helpers + new tests HERE. Do NOT recreate."
  pattern: "tests are grouped with `// --- header ---` comments and assert with
            the named consts, e.g. assert_eq!(process_escapes(\"\\\\^\"), vec![ESC_CARET]).
            Mirror that style for the nfa_compile tests — build inputs from the
            named consts (GLOB_STAR, PLUS_QUANT, ASSERT_BOUND, CLASS_DIGIT,
            DOT_META) and assert the full Vec<NfaOp>."
  gotcha: "the file carries #![allow(dead_code)] (S1 ships the API ahead of
           consumers). nfa_compile's first non-test consumer is the simulator
           (T2.S2); until then the #[allow(dead_code)] keeps it warning-free."

# MUST READ — the parity vector table (the test contract for THIS task)
- file: plan/002_637d65b6e9b8/P2M1T2S1/research/notes.md
  why: "§6 lists the exact expected Vec<NfaOp> for 18 inputs across 7 groups
        (simple chains, glob, X+ quantifier, the a+a+a+ killer, boundary
        asserts, mixed/stray-0x0E, literal-dot/plus). Copy these directly into
        the unit tests. §1 reproduces the C verbatim; §5 is the verified Rust
        skeleton; §7 enumerates the 8 gotchas with their pinning test rows."
  section: "## 6. Parity table" and "## 5. Verified Rust skeleton"

# MUST READ — the upstream contracts (S1 + S2) this task consumes
- file: plan/002_637d65b6e9b8/P2M1T1S1/PRP.md
  why: "defines process_escapes() and the placeholder-byte vocabulary (0x01-0x0E,
        0x2A) that nfa_compile dispatches on. Confirms the byte contract."
  section: "## What" (the placeholder-byte table)
- file: plan/002_637d65b6e9b8/P2M1T1S2/PRP.md
  why: "defines ParsedPattern { core: Vec<u8>, start_anchored, end_anchored }
        whose `.core` field IS the &[u8] this compiler consumes. Confirms the
        upstream stage produces exactly the processed bytes nfa_compile walks."
  section: "## What" (ParsedPattern struct + parse_pipeline)

# MUST READ — QMKonnect-side architecture contract (cross-repo)
- file: plan/002_637d65b6e9b8/architecture/external_deps.md
  why: "§3 'Pattern Matcher' point 3 is the cross-repo contract for nfa_compile:
        lists the 5 opcodes (OP_CHAR/ANY/SPLIT/ASSERT/MATCH), the X+ ⇒ exactly
        2 states guarantee, and the pipeline parse_pattern -> nfa_compile ->
        nfa_addstate -> nfa_match -> match_with_anchors. Confirms the simulator
        + matcher live in later subtasks, not here."
  section: "## 3. Pattern Matcher" (point 3: nfa_compile opcode list + X+ 2-state)

# Reference — firmware architecture doc (corroborates the C)
- file: /home/dustin/projects/qmk-notifier/plan/001_e329fbe4ae4d/architecture/pattern_match_architecture.md
  why: "'NFA Compilation (nfa_compile)' section restates: 0x2A -> OP_ANY looping
        back through OP_SPLIT; 0x0B/0x0C -> OP_ASSERT; X followed by 0x0E ->
        OP_CHAR then OP_SPLIT loop-back (linear for a+a+a+...); end -> OP_MATCH;
        zero lastlist on every allocated state. Cross-checks the C source."
  section: "### NFA Compilation (nfa_compile)"

# Reference — the linear-time guarantee (cite in rustdoc, Mode A)
- url: https://swtch.com/~rsc/regexp/regexp1.html
  why: "Russ Cox, 'Regular Expression Matching Can Be Simple And Fast'. The
        firmware rustdoc already cites it (PRD §7.5, §7.9). For THIS task's
        rustdoc: cite that Thompson construction compiles X+ to exactly 2 states
        (CHAR + SPLIT loop-back) so a+a+a+...b scales as 2k+1 — never 2^k — and
        simulation is O(states x input_len) with no backtracking (the fix for the
        old exponential matcher, PRD §7.8)."
  section: "'NFA-based Regular Expression Algorithms' + the a(n+1)б pathological example"

# Reference — existing Rust enum/test conventions in THIS repo
- file: src/core/types.rs
  why: "shows #[derive(Debug, PartialEq)] on a struct + inline #[cfg(test)] mod
        tests with assert_eq!. Mirror this derive + test style for NfaOp (add
        Clone — NfaOp is cheaply clonable and PartialEq needs it for nothing, but
        Clone aids downstream ergonomics)."
  pattern: "#[derive(Debug, PartialEq)] pub struct ... ; #[cfg(test)] mod tests"

# Reference — PRD selectors that scoped this work
- url: spec/PRD.md (heading h2.74 "Pattern-Matching Syntax (pattern_match.c)")
  why: "the `*` wildcard + `.` + `+` quantifier + `\\b \\B` rows of the construct
        table are exactly the constructs nfa_compile compiles (glob=SPLIT+ANY,
        dot=Char{0x0D}, X+=Char+SPLIT, \\b=Assert)."
- url: spec/PRD.md (heading h2.92 "Appendix — File Layout & Pattern Subset")
  why: "mandates src/core/pattern.rs as 'full-parity matcher (ported from
        firmware)', 'All linear-time (Thompson NFA)'. Confirms the firmware +
        its test corpus is the single source of truth."
```

### Current Codebase tree (qmkonnect, relevant subset)

```bash
src/
  main.rs                 # CLI entry (unchanged)
  core/
    mod.rs                # Config + helpers; ALREADY has `pub mod pattern;` (S1) — DO NOT TOUCH
    pattern.rs            # S1: process_escapes + consts; S2: ParsedPattern + parse_pattern
                            # + mod tests   ← EDIT THIS FILE (additive: + NfaOp, + nfa_compile, + tests)
    notifier.rs           # Notifier trait, debouncer, tests (unchanged)
    types.rs              # WindowInfo (unchanged) — struct/test style reference
  platforms/              # per-OS window monitors (unchanged)
  tray.rs / linux_tray.rs # tray UI (unchanged)
```

### Desired Codebase tree (files this task changes)

```bash
src/
  core/
    pattern.rs            # MODIFIED (additive) — + NfaOp enum, + DanglingExit/resolve/set_out helpers,
                            #                     + nfa_compile(), + tests
    # mod.rs UNCHANGED (module already registered by S1)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (GOTCHA-1 — start index): the firmware allocates ANY-then-SPLIT for
//   the glob, so a LEADING glob has start index 1 (not 0). The Rust port REORDERS
//   to SPLIT-then-ANY so the unit entry is always the lowest index, keeping
//   `start == 0` for EVERY pattern (including leading `*`). The edge graph is
//   IDENTICAL to the firmware — only the two glob states' index numbers swap
//   (firmware any@k/sp@k+1  ->  Rust sp@k/any@k+1). The simulator (T2.S2) seeds
//   closure from states[0], so this invariant is load-bearing. Pinned by test
//   rows 4 (`b"*"`) and 6 (`b"*a"`): entry must be `Split` at index 0.
//     firmware glob:  any@k(out=sp)   sp@k+1(out=any, out1=exit)
//     Rust glob:      sp@k (out=any, out1=exit)   any@k+1(out=sp)
//
// CRITICAL (GOTCHA-2 — glob loop-back is a BACKWARD edge): `Any.out` points
//   BACK to the SPLIT (index sp_idx < any_idx), NOT to `any_idx+1`. Therefore
//   `out` CANNOT be implicit ("next index"); it MUST be a stored field. The
//   item-spec sketch `Any` (no payload) is insufficient — refined to
//   `Any { out: usize }`. Same for Char/Assert (their `out` is resolved to the
//   next unit's entry, which IS index+1 in practice, but storing it explicitly
//   keeps the NFA self-describing and matches the firmware `State.out` field).
//
// CRITICAL (GOTCHA-3 — X+ consumes TWO source bytes): branch (d) peeks
//   `pat[i+1] == 0x0E` and, on hit, does `i += 2` (skip X AND the 0x0E marker).
//   Using `i += 1` here would re-enter the loop on the 0x0E and emit it as a
//   standalone Char (corruption). The standalone branch (c) `i += 1; continue`
//   exists ONLY for a stray 0x0E that survived process_escapes — which should
//   never happen, but the firmware keeps it defensive; mirror it.
//
// GOTCHA-4 (glob and X+ each allocate exactly TWO states): never one. The tail
//   after a glob/X+ is always a `SplitSecond` (the SPLIT's out1 exit), never an
//   `Out`. Forgetting the second push or mis-threading the tail breaks the loop.
//
// GOTCHA-5 (NO lastlist here): `NfaOp` has NO `lastlist` field and there is NO
//   file-scope `static nfa_gen`. The compiled program is pure immutable data;
//   the simulator (T2.S2) owns a separate generation-tag list parallel to
//   `states`. Do NOT zero lastlist, do NOT add a gen counter. (The firmware
//   couples lastlist onto State only because C has no cheap alternative.)
//
// GOTCHA-6 (empty pattern -> single Match): `nfa_compile(b"")` MUST return
//   `vec![NfaOp::Match]` (NOT an empty Vec). The loop body never runs; the
//   unconditional final `push(Match)` + `resolve(tail=Start, ...)` sets
//   start=Some(0). A downstream exact `^$` match against `""` relies on this.
//
// GOTCHA-7 (NFA_MAX_STATES clamp — DO NOT replicate): the firmware clamps
//   allocation at index NFA_MAX_STATES-1 (reuse last slot) for pathological
//   patterns, which CORRUPTS the NFA by design rather than overflowing the
//   stack pool. A Rust `Vec` grows unboundedly; a host rules.toml pattern never
//   approaches any cap. Just let the Vec grow. Document the firmware behavior
//   in a comment for parity awareness; do NOT port the clamp.
//
// GOTCHA-8 (branch order matters): check `0x2A` (glob) and `0x0B/0x0C` (assert)
//   and `0x0E` (stray quant) BEFORE the generic consuming-element `else` arm —
//   those bytes would otherwise compile as plain Char. Match/if-else in exactly
//   the firmware order: glob -> assert -> stray-0x0E -> else(consuming).
//
// BORROW-CHECKER: `resolve(&mut states, &mut start, &tail, idx)` borrows `states`
//   and `start` mutably and `tail` shared — all disjoint, no aliasing. Read
//   `tail` (to resolve the PREVIOUS exit) BEFORE pushing the new unit. Each push
//   releases its borrow immediately. Compiles clean with no `unsafe`, no
//   `Cell`, no separate edge list.
//
// CRATE QUIRK: the crate-wide test command MUST be single-threaded because
//   src/core/notifier.rs uses shared global debouncer state:
//     cargo test --bin qmkonnect -- --test-threads=1
//   (AGENTS.md.) pattern::tests itself is stateless, but run the whole bin
//   single-threaded so notifier's globals don't race.
```

## Implementation Blueprint

### Data models and structure

```rust
/// A single compiled NFA node — index-based Thompson construction (no raw
/// pointers). Rust analog of the firmware `State` (`pattern_match.c`), minus the
/// `lastlist` field (the simulator, P2.M1.T2.S2, owns dedup state separately).
///
/// Edges are `usize` indices into the `Vec<NfaOp>` returned by `nfa_compile`.
/// The start state is ALWAYS index 0 (see `nfa_compile` rustdoc + the
/// glob-allocation reorder). Opcodes mirror the firmware `OP_*` enum 1:1.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum NfaOp {
    /// Consume one input byte matching `arg`, then go to `out`. `arg` is a
    /// `process_escapes` placeholder byte (escaped literal 0x01-0x04, class
    /// 0x05-0x0A, dot 0x0D, literal `.`/`+`, or any ordinary ASCII byte).
    Char { arg: u8, out: usize },
    /// Consume ANY one byte INCLUDING `\n`/`\r` (the glob `*` compiled as
    /// `.*`). Distinct from `Char{arg:DOT_META}` (the dot excludes newline).
    Any { out: usize },
    /// Epsilon fork: the simulator follows BOTH `out` and `out1` without
    /// consuming input. Implements `*` (glob) and `+` (one-or-more).
    Split { out: usize, out1: usize },
    /// Zero-width assertion: `arg` is 0x0B (`\b`, want a boundary) or 0x0C
    /// (`\B`, want a non-boundary). The simulator recurses into `out` only if
    /// `is_word_boundary(string_start, abspos) == (arg == 0x0B)`.
    Assert { arg: u8, out: usize },
    /// Accepting state (terminal). A match is reported when a `Match` node is
    /// on the simulator's current state list.
    Match,
}

impl NfaOp {
    /// Write the next-state index into this state's PRIMARY `out` edge.
    /// No-op on `Split`/`Match` (Split's primary edge is set at allocation;
    /// Match is terminal) — calling it there would be a logic bug, silently
    /// ignored to keep the compiler's match arms uniform.
    fn set_out(&mut self, idx: usize) {
        match self {
            NfaOp::Char { out, .. } => *out = idx,
            NfaOp::Any { out } => *out = idx,
            NfaOp::Assert { out, .. } => *out = idx,
            NfaOp::Split { .. } | NfaOp::Match => {}
        }
    }
    /// Write the next-state index into a `Split`'s SECONDARY `out1` edge.
    fn set_out1(&mut self, idx: usize) {
        if let NfaOp::Split { out1, .. } = self { *out1 = idx; }
    }
}

/// The dangling exit slot the compiler threads — index-based analog of the
/// firmware `State **tail`. Describes WHERE the next unit's entry index must be
/// written when that unit is compiled.
enum DanglingExit {
    /// Write into the function's `start` slot (no unit compiled yet).
    Start,
    /// Write into `states[idx].out` (a Char/Any/Assert primary exit).
    Out(usize),
    /// Write into `states[idx].out1` (a Split secondary exit — glob/X+ exit).
    SplitSecond(usize),
}

/// `*tail = entry_idx` — resolve the dangling exit. The first call (tail==Start)
/// sets the function's `start` to the first unit's entry (always index 0).
fn resolve(states: &mut [NfaOp], start: &mut Option<usize>, tail: &DanglingExit, entry_idx: usize) {
    match tail {
        DanglingExit::Start => *start = Some(entry_idx),
        DanglingExit::Out(idx) => states[*idx].set_out(entry_idx),
        DanglingExit::SplitSecond(idx) => states[*idx].set_out1(entry_idx),
    }
}
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: ADD the NfaOp enum + impl + DanglingExit + resolve to src/core/pattern.rs
  - PLACE: after the parse_pattern fn (logical pipeline order: process_escapes
           -> parse_pattern -> nfa_compile), before the #[cfg(test)] mod tests.
  - DERIVE: #[derive(Debug, Clone, PartialEq)] on NfaOp (PartialEq for assert_eq!
           in tests; Clone/Debug for downstream ergonomics + diagnostics).
  - VISIBILITY: pub(crate) enum NfaOp; the helpers (DanglingExit, resolve,
           set_out/set_out1) are private (no pub) — they are compile-internal.
  - DOC: a `///` block on NfaOp mapping each variant to the firmware OP_* opcode
         + State field, noting lastlist is absent (simulator-owned, GOTCHA-5).
  - FOLLOW: src/core/types.rs derive+struct style; the existing pattern.rs
           const-block + fn style (S1).

Task 2: IMPLEMENT nfa_compile() in src/core/pattern.rs
  - SIGNATURE: pub(crate) fn nfa_compile(pat: &[u8]) -> Vec<NfaOp>
  - BODY (verified Rust skeleton — see research/notes.md §5, mirrored above in
         "## What" + the Implementation Patterns section):
      * Vec::with_capacity(pat.len()*2 + 2); start=None; tail=DanglingExit::Start.
      * while i < pat.len() dispatch in firmware branch order (GOTCHA-8):
          0x2A (GLOB_STAR)        -> glob: push Split THEN Any (reorder, GOTCHA-1),
                                     set sp.out=any, any.out=sp (loop-back, GOTCHA-2),
                                     resolve(tail, sp_idx), tail=SplitSecond(sp_idx), i+=1.
          0x0B/0x0C (ASSERT_*)    -> push Assert{arg:b}, resolve, tail=Out(a_idx), i+=1.
          0x0E (PLUS_QUANT)       -> i+=1 (stray, skip — GOTCHA-3 defensive).
          else (consuming X)      -> push Char{arg:b}; if pat[i+1]==0x0E: push
                                     Split{out:c_idx}, set Char.out=sp_idx, resolve(c_idx),
                                     tail=SplitSecond(sp_idx), i+=2 (GOTCHA-3);
                                     else resolve(c_idx), tail=Out(c_idx), i+=1.
      * End: push Match, resolve(tail, m_idx).
      * debug_assert_eq!(start, Some(0)); return states.
  - CALLS: the S1 named consts (GLOB_STAR, ASSERT_BOUND, ASSERT_NBOUND,
           PLUS_QUANT) for the byte comparisons — do NOT hardcode 0x2A/0x0B/0x0C/0x0E.
  - GOTCHA: NO lastlist zeroing (GOTCHA-5). NO NFA_MAX_STATES clamp (GOTCHA-7).
  - GOTCHA: read `tail` (via resolve) BEFORE pushing the new unit (borrow order).
  - PLACEMENT: src/core/pattern.rs, immediately after the helpers from Task 1.

Task 3: ADD rustdoc (Mode A — code-level docs only)
  - ON NfaOp: per-variant semantics (what it consumes, which edges the simulator
              follows), the absent-lastlist note, the firmware OP_* mapping.
  - ON nfa_compile: a `///` block explaining:
      * input is the processed-pattern byte slice (parse_pattern().core / the
        process_escapes output); output is the compiled NFA (start = states[0]).
      * the 5 constructs compiled (glob=SPLIT+ANY loop; \b/\B=ASSERT; X+=CHAR+
        SPLIT loop-back; plain byte=CHAR; end=MATCH).
      * THE LINEAR-TIME GUARANTEE: X+ compiles to exactly 2 states (CHAR + SPLIT
        loop-back), so a+a+a+...b scales as 2k+1 — never 2^k — and simulation is
        O(states x strlen) with no backtracking. Reference Russ Cox,
        https://swtch.com/~rsc/regexp/regexp1.html (PRD §7.5, §7.8, §7.9).
      * the start==0 invariant + the glob-allocation reorder rationale.
      * that this is stage 3 of the pipeline (parse_pattern -> nfa_compile ->
        nfa_addstate/nfa_match -> match_with_anchors); the simulator is T2.S2.
  - REFERENCE arch external_deps.md §3 point 3 and PRD §4 as the contract sources.

Task 4: APPEND nfa_compile unit tests to the existing mod tests in pattern.rs
  - DO: add new #[test] fns INTO the existing `mod tests { use super::*; ... }`
        block. Group with the same `// --- header ---` comment style.
  - IMPLEMENT one #[test] per row of research/notes.md §6 (18 inputs), asserting
        the full Vec<NfaOp>. Suggested groupings (mirror notes §6.1–§6.7):
      // --- Simple chains (empty, a, abc) ---                    (rows 1-3)
      // --- Glob * (SPLIT+ANY loop; start==0 reorder) ---        (rows 4-7)
      // --- X+ quantifier (CHAR+SPLIT loop; 2 states) ---        (rows 8-10)
      // --- a+a+a+ : the linear-time killer (2k+1 states) ---    (row 11)
      // --- Boundary assertions \b \B (zero-width) ---           (rows 12-14)
      // --- Mixed / stray-0x0E defensive ---                     (rows 15-16)
      // --- Literal dot/plus as Char ---                         (rows 17-18)
  - BUILD inputs from the named consts (NOT raw hex): e.g.
        assert_eq!(nfa_compile(&[CLASS_DIGIT, PLUS_QUANT]),
                   vec![NfaOp::Char { arg: CLASS_DIGIT, out: 1 },
                        NfaOp::Split { out: 0, out1: 2 },
                        NfaOp::Match]);
  - ROW 11 (a+a+a+): assert the Vec has exactly 7 states — this is the
        exponential-backtracker-killer parity assertion; comment WHY.
  - ROW 16 (stray [0x0E]): assert nfa_compile(&[PLUS_QUANT]) == vec![NfaOp::Match]
        (branch c skips the lone marker; only the end-MATCH remains).
  - NAMING: test_compile_<behavior> (e.g. test_compile_empty_is_single_match,
        test_compile_glob_split_first_start_zero, test_compile_plus_two_states,
        test_compile_a_plus_a_plus_a_plus_linear, test_compile_stray_quant_skipped).
  - COVERAGE: every construct + all 8 gotchas + the empty + stray-0x0E edges.

Task 5: VERIFY build + tests (single-threaded, per AGENTS.md)
  - RUN: cargo build --bin qmkonnect        (expect: clean; no NEW warnings)
  - RUN: cargo test --bin qmkonnect pattern -- --test-threads=1
         (expect: ALL pattern::tests pass — new nfa_compile tests AND S1
          process_escapes AND S2 parse_pattern tests)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1
         (expect: full crate green — no regression)
  - IF a parity assertion fails: re-read the C source (pattern_match.c nfa_compile)
    line-by-line + research/notes.md §5 skeleton; the table is a faithful
    transcription, so a failure means the Rust diverged. Do NOT "fix" the test to
    match the Rust — fix the Rust to match the firmware (PRD §14: firmware is
    source of truth), with the ONE documented exception (glob SPLIT-first reorder,
    which preserves the edge graph).
  - CONFIRM git status shows ONLY src/core/pattern.rs modified.
```

### Implementation Patterns & Key Details

```rust
// The canonical nfa_compile body (this IS the spec — match it exactly, modulo
// the documented glob reorder). Full verified version in research/notes.md §5.
//
// pub(crate) fn nfa_compile(pat: &[u8]) -> Vec<NfaOp> {
//     let mut states: Vec<NfaOp> = Vec::with_capacity(pat.len() * 2 + 2);
//     let mut start: Option<usize> = None;
//     let mut tail = DanglingExit::Start;
//
//     let mut i = 0;
//     while i < pat.len() {
//         let b = pat[i];
//         if b == GLOB_STAR {                                   // (a) glob '*' == '.*'
//             // GOTCHA-1: allocate SPLIT FIRST so the unit entry is the lowest
//             // index (keeps start==0 for a leading glob). Edge graph is identical
//             // to the firmware; only the two states' index numbers swap.
//             let sp_idx = states.len();
//             states.push(NfaOp::Split { out: 0, out1: 0 });
//             let any_idx = states.len();
//             states.push(NfaOp::Any { out: 0 });
//             states[sp_idx].set_out(any_idx);   // sp.out  = ANY
//             states[any_idx].set_out(sp_idx);   // any.out = sp  (GOTCHA-2: BACKWARD loop)
//             resolve(&mut states, &mut start, &tail, sp_idx);
//             tail = DanglingExit::SplitSecond(sp_idx);
//             i += 1;
//         } else if b == ASSERT_BOUND || b == ASSERT_NBOUND {   // (b) \b / \B
//             let a_idx = states.len();
//             states.push(NfaOp::Assert { arg: b, out: 0 });
//             resolve(&mut states, &mut start, &tail, a_idx);
//             tail = DanglingExit::Out(a_idx);
//             i += 1;
//         } else if b == PLUS_QUANT {                           // (c) stray 0x0E
//             i += 1;                                            // skip (never emitted standalone)
//         } else {                                              // (d) consuming element X
//             let c_idx = states.len();
//             states.push(NfaOp::Char { arg: b, out: 0 });
//             if i + 1 < pat.len() && pat[i + 1] == PLUS_QUANT { // X+ : LINEAR (2 states)
//                 let sp_idx = states.len();
//                 states.push(NfaOp::Split { out: c_idx, out1: 0 });
//                 states[c_idx].set_out(sp_idx);  // after one X -> split
//                 resolve(&mut states, &mut start, &tail, c_idx);
//                 tail = DanglingExit::SplitSecond(sp_idx);
//                 i += 2;                                        // GOTCHA-3: consume X AND 0x0E
//             } else {
//                 resolve(&mut states, &mut start, &tail, c_idx);
//                 tail = DanglingExit::Out(c_idx);
//                 i += 1;
//             }
//         }
//     }
//
//     // (e) End: append the single accepting state into the final dangling slot.
//     let m_idx = states.len();
//     states.push(NfaOp::Match);
//     resolve(&mut states, &mut start, &tail, m_idx);
//     debug_assert_eq!(start, Some(0));   // invariant: first unit's entry == index 0
//
//     states   // start is implicitly states[0]; the simulator (T2.S2) seeds from here
// }
//
// NOTE on the glob reorder: the firmware does `any = NEW(); sp = NEW();` (ANY at
// the lower index), making a leading glob's entry `sp` land at index 1. We swap
// to `sp` first so the entry is index 0. The loop structure (sp.out=any,
// any.out=sp, sp.out1=exit) is unchanged — the NFA graph is identical, only node
// indices differ. This is the ONLY intentional divergence from the C, and it
// preserves match semantics exactly (the simulator follows edges, not indices).
```

### Integration Points

```yaml
MODULE REGISTRATION:
  - NONE. `pub mod pattern;` is already in src/core/mod.rs (added by S1). Do NOT
    edit mod.rs.

DEPENDENCIES (this task): NONE new. Pure stdlib Vec/index work + references to the
                           in-module S1 named consts. No Cargo deps, no qmk_notifier
                           crate, no firmware link, no `unsafe`, no `static`.

UPSTREAM (already present — S1 + S2 contract):
  - pub(crate) const GLOB_STAR/ASSERT_BOUND/ASSERT_NBOUND/PLUS_QUANT/CLASS_*/DOT_META
    (top of pattern.rs, S1) — use these for byte comparisons, NOT raw hex.
  - pub(crate) fn process_escapes(&str) -> Vec<u8>  (S1) — produces the input bytes.
  - pub(crate) struct ParsedPattern { core: Vec<u8>, .. } (S2) — `.core` IS the
    &[u8] this compiler consumes (via `.as_slice()` / `&parsed.core`).

DOWNSTREAM CONSUMERS (later subtasks — do NOT implement them here):
  - P2.M1.T2.S2 nfa_addstate() / nfa_match(): the simulator. Seeds epsilon-closure
    from states[0] (the start — always index 0 by this task's invariant). Walks
    NfaOp edges (Char.out on match, Any.out on any byte, Split.out+Split.out1 for
    epsilon forks, Assert.out on boundary condition). Owns its OWN dedup state
    (a Vec<u32> generation-tag list parallel to states + a gen counter) — this
    task's NfaOp deliberately has no lastlist field (GOTCHA-5).
  - P2.M1.T3.S1 match_with_anchors(): reads parse_pattern's anchor flags to pick
    the strategy, then loops offsets calling nfa_match on nfa_compile(parsed.core).

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
# from S1, so the new pub(crate) NfaOp/nfa_compile won't spuriously warn even
# though their only non-test consumer (the simulator) is a later subtask.

# Confirm the additions are present:
grep -n 'enum NfaOp' src/core/pattern.rs            # expect one definition
grep -n 'fn nfa_compile' src/core/pattern.rs        # expect one definition
grep -n 'enum DanglingExit' src/core/pattern.rs     # expect one definition
grep -n 'fn resolve' src/core/pattern.rs            # expect one definition
grep -n 'debug_assert_eq!(start, Some(0))' src/core/pattern.rs  # the start-invariant
```

### Level 2: Unit Tests — the parity contract (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# Single-threaded is MANDATORY crate-wide (shared debouncer state in notifier.rs).
cargo test --bin qmkonnect pattern -- --test-threads=1
# Expected: every test in pattern::tests passes — the new nfa_compile tests
# (research/notes.md §6, 18 rows) AND S1's process_escapes AND S2's parse_pattern.
# A failure means the Rust diverged from the firmware C — fix the Rust, not the test.
# Filter to just the new tests to see them individually:
cargo test --bin qmkonnect pattern::tests::test_compile -- --test-threads=1
```

### Level 3: Full-crate regression (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL bin tests green — pattern::tests (new nfa_compile + S1 + S2) +
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
# Cross-validate the compile structure against the firmware's own corpus (unchanged
# by this task — it lives in the OTHER repo and is exercised end-to-end via
# pattern_match, since nfa_compile is `static` in C):
cd /home/dustin/projects/qmk-notifier && ./run_all_tests.sh
# Expected: the pattern_match corpus still passes (it always does — this task does
# not touch the firmware). The Rust nfa_compile parity vectors in research/notes.md
# §6 were derived FROM this C source, so a green firmware run corroborates the
# contract the Rust port encodes. The Rust tests are STRICTLY STRONGER: they
# assert the compiled Vec<NfaOp> (opcode + every edge target) directly, whereas
# the C tests can only observe end-to-end match results through the static fn.
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean (no NEW warnings on `src/core/pattern.rs`).
- [ ] `cargo test --bin qmkonnect pattern -- --test-threads=1` — all pattern tests pass (new + S1 + S2).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` — full crate green.
- [ ] `git status` shows exactly ONE modified file: `src/core/pattern.rs`.

### Feature Validation (parity)
- [ ] Every row of `research/notes.md` §6 (18 inputs) asserted and passing.
- [ ] GOTCHA-1 (start==0): rows 4 & 6 (`b"*"`, `b"*a"`) — `Split` at index 0 (glob SPLIT-first reorder).
- [ ] GOTCHA-2 (glob loop-back): rows 4–7 — `Any.out` points BACK to the `Split`.
- [ ] GOTCHA-3 (X+ two bytes): rows 8–11 — `i += 2`, no stray 0x0E re-emitted as Char.
- [ ] GOTCHA-6 (empty → single Match): `nfa_compile(b"")` == `vec![NfaOp::Match]`.
- [ ] Linear guarantee: row 11 `a+a+a+` == exactly 7 states (2·3+1).
- [ ] Stray-0x0E (row 16): `nfa_compile(&[PLUS_QUANT])` == `vec![NfaOp::Match]`.
- [ ] Assert variants (rows 12–14): `\b`→`Assert{0x0B}`, `\B`→`Assert{0x0C}`.
- [ ] Literal dot/plus compile as plain `Char` (rows 17–18).

### Code Quality Validation
- [ ] `NfaOp` has `#[derive(Debug, Clone, PartialEq)]`; `pub(crate)` enum.
- [ ] `nfa_compile` is `pub(crate)`, takes `&[u8]`, returns `Vec<NfaOp>`.
- [ ] `NfaOp` has NO `lastlist` field; NO file-scope `static` (GOTCHA-5).
- [ ] Uses S1 named consts (`GLOB_STAR`, `PLUS_QUANT`, `ASSERT_*`, …) for byte comparisons — no raw hex literals in the dispatch.
- [ ] `DanglingExit` + `resolve` + `set_out`/`set_out1` are private helpers.
- [ ] Rustdoc on `NfaOp` + `nfa_compile` references Russ Cox + the linear-time guarantee + start==0 invariant.
- [ ] New tests appended to the existing `mod tests` (S1/S2 tests untouched), grouped with `// --- header ---` comments, using named consts.
- [ ] No new Cargo dependencies; no `unsafe`; no external docs changed (Mode A).
- [ ] Scope respected: NO simulator (`nfa_match`/`nfa_addstate`), NO `match_with_anchors`, NO public `pattern_match`, NO delimiter-aware `match_pattern`, NO `Pattern::Single|Parts`, NO `mod.rs` edit.

### Documentation & Deployment
- [ ] Code-level rustdoc present (Mode A — no `docs/*.md` or README changes this task).
- [ ] `NfaOp` variants + `nfa_compile` constructs documented with their firmware meaning.

---

## Anti-Patterns to Avoid

- ❌ Do NOT copy the firmware's `any`-then-`sp` glob allocation verbatim. A
      *leading* glob would then have `start == 1`, breaking the simulator's
      `states[0]` seed (GOTCHA-1). Allocate SPLIT first. The edge graph is
      identical; only node indices swap.
- ❌ Do NOT make `Any.out` implicit ("next index"). The glob loop-back is a
      BACKWARD edge; `out` MUST be a stored field (GOTCHA-2). The item-spec
      sketch `Any` (no payload) is insufficient — use `Any { out: usize }`.
- ❌ Do NOT use `i += 1` in the `X+` branch. Branch (d) consumes TWO source bytes
      (X and the `0x0E` marker) — use `i += 2` (GOTCHA-3). Using `i += 1`
      re-emits the marker as a standalone Char.
- ❌ Do NOT add a `lastlist` field to `NfaOp` or a file-scope `static nfa_gen`.
      Dedup is the SIMULATOR's concern (T2.S2); the compiled program is immutable
      data (GOTCHA-5).
- ❌ Do NOT replicate the firmware's `NFA_MAX_STATES` clamp. It corrupts the NFA
      on overflow BY DESIGN (to avoid stack overflow on AVR); a Rust `Vec` grows
      and a host pattern never approaches any cap (GOTCHA-7).
- ❌ Do NOT reorder the dispatch arms. Check `0x2A`/`0x0B`/`0x0C`/`0x0E` BEFORE
      the generic consuming `else` — else those bytes compile as plain Char
      (GOTCHA-8).
- ❌ Do NOT push the new unit BEFORE resolving the previous `tail`. Read `tail`
      (to resolve the prior exit) first; the borrow-checker requires this order
      and it matches the firmware's `*tail = entry; tail = &new->exit` sequence.
- ❌ Do NOT return an empty `Vec` for `nfa_compile(b"")`. The end-MATCH is pushed
      unconditionally; empty input yields `vec![NfaOp::Match]` (GOTCHA-6).
- ❌ Do NOT hardcode `0x2A`/`0x0B`/`0x0C`/`0x0E` in the dispatch. Use the S1
      named consts (`GLOB_STAR`, `ASSERT_BOUND`, `ASSERT_NBOUND`, `PLUS_QUANT`).
- ❌ Do NOT implement the simulator (`nfa_addstate`/`nfa_match`),
      `match_with_anchors`, the public `pattern_match()`, the delimiter-aware
      `match_pattern`, or `Pattern::Single|Parts` — those are P2.M1.T2.S2 /
      P2.M1.T3.
- ❌ Do NOT edit `src/core/mod.rs` — `pub mod pattern;` is already there (S1).
- ❌ Do NOT change the test to match divergent Rust output. The firmware C
      (`pattern_match.c` `nfa_compile`) is the source of truth (PRD §14); fix the
      Rust, with the single documented exception of the glob SPLIT-first reorder
      (which preserves the edge graph).
- ❌ Do NOT run tests multi-threaded — the crate shares debouncer state
      (`cargo test --bin qmkonnect -- --test-threads=1`).
- ❌ Do NOT edit `PRD.md`, any `tasks.json`, `prd_snapshot.md`, or any `plan/` file
      other than this item's own `PRP.md` + `research/`.

---

## Confidence Score: 9/10

This is a well-bounded, single-function port of a ~45-line, heavily-commented C
function whose entire body + the `State` struct + opcode enum are reproduced
verbatim in `research/notes.md` §1, and transcribed as a verified Rust skeleton
(§5, mirrored in the Implementation Blueprint). The item spec's two mandates —
index-based edges (no raw pointers) and the `enum NfaOp` shape — are honored,
with ONE justified refinement: `out` is an explicit field on `Char`/`Any`/`Assert`
(because the glob's `Any.out` is a backward loop-back that cannot be implicit) and
ONE justified divergence from the firmware's allocation order: the glob allocates
SPLIT-before-ANY so the start state is always index 0 (honoring the item spec's
`-> Vec<NfaOp>` return signature; the edge graph is byte-for-byte identical, only
node indices swap). Both decisions are fully derived in §2–§4 and pinned by test
rows. The linear-time guarantee — the entire reason Thompson construction exists —
is captured by the `a+a+a+` ⇒ 7-states parity assertion (row 11). 18 exact
`Vec<NfaOp>` vectors are provided as the test contract, and because the Rust port
makes `nfa_compile` `pub(crate)` (vs. `static` in C), the unit tests are STRICTLY
STRONGER than the firmware's end-to-end-only coverage — they assert the compiled
graph (opcode + every edge target) directly. The upstream S1/S2 contract (const
names, `ParsedPattern.core` shape) and downstream T2.S2 contract (start=index 0,
simulator owns dedup) are both explicit, so the additions slot in cleanly with no
interface guesswork. No new deps, no `unsafe`, no `static`. The 1-point reservation
is for the (unlikely) event an implementer mishandles the glob reorder or the
`X+` `i += 2` despite the explicit callouts; both are caught immediately by the
parity tests. Scope is cleanly bounded from S1/S2 (upstream), T2.S2 (simulator),
and T3 (matcher entry), so there is no risk of over- or under-building.
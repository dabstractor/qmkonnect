# Research Notes — P2.M1.T2.S1: Port NFA compiler (`nfa_compile`) + state types

**Source of truth (PRD §14):** `/home/dustin/projects/qmk-notifier/pattern_match.c`
(lines ~350–430 = `nfa_compile` + the `State` struct + opcodes + pool sizing).
This file reproduces the C verbatim, derives the index-based Rust design, and
gives a verified Rust skeleton + an exact-`Vec<NfaOp>` parity table for tests.

---

## 1. The canonical firmware C (verbatim — the spec)

### 1a. Pool sizing + opcodes + State struct (lines ~300–345)

```c
#ifndef NFA_MAX_PATTERN
#define NFA_MAX_PATTERN 2048          /* host/test default; QMK overrides via notifier.c */
#endif
#define NFA_MAX_STATES  (2 * NFA_MAX_PATTERN + 2)   /* 2 per byte + MATCH + slack */

/* NFA node opcodes (Thompson construction). */
enum {
    OP_CHAR,   /* consume one input byte matching `arg` */
    OP_ANY,    /* consume ANY one byte incl newline (glob '*') */
    OP_SPLIT,  /* epsilon fork (for '*' and '+') */
    OP_ASSERT, /* zero-width assertion (\b 0x0B / \B 0x0C) */
    OP_MATCH   /* accepting state */
};

typedef struct State State;
struct State {
    int    op;        /* one of the OP_* opcodes */
    char   arg;       /* OP_CHAR: the processed-pattern byte; OP_ASSERT: 0x0B/0x0C */
    State *out;       /* primary outgoing edge (every opcode except MATCH) */
    State *out1;      /* secondary outgoing edge (OP_SPLIT only; NULL for others) */
    int    lastlist;  /* generation-tag dedup == nfa_gen when already on current list */
};

static int nfa_gen = 0;   /* the ONLY file-scope mutable; bumped once per sim phase */
```

### 1b. `nfa_compile` — the function being ported (lines ~365–430)

```c
/* Compile a processed-pattern byte string into a State pool via Thompson
 * construction. Caller allocates `State pool[NFA_MAX_STATES]` on its stack;
 * we fill it and return the start state. Threads `State **tail`: it points at
 * the slot where the NEXT unit's start node must be written (initially &start).
 * Each construct writes *tail = <its start> then advances tail to its own
 * "dangling exit" slot (out1 for SPLIT, out for CHAR/ASSERT). At the end we
 * write OP_MATCH into the final dangling slot. */
static State *nfa_compile(const char *pat, State *pool, int *nstates_out) {
    int n = 0;
    State *start = NULL;
    State **tail = &start;            /* slot to write the next unit's start into */

    /* Bounds-safe state allocator: return &pool[n] and advance n, clamp at
     * NFA_MAX_STATES so a pathological pattern reuses the last slot (never overflow). */
    #define NEW() (&pool[n < NFA_MAX_STATES ? n++ : (NFA_MAX_STATES - 1)])

    for (const char *p = pat; *p; p++) {
        unsigned char b = (unsigned char)*p;

        if (b == 0x2A) {                         /* (a) glob '*' == regex '.*' */
            State *any = NEW(); any->op = OP_ANY;
            State *sp  = NEW(); sp->op  = OP_SPLIT; sp->out = any; sp->out1 = NULL;
            any->out = sp;                        /* loop back: ANY -> SPLIT */
            *tail = sp; tail = &sp->out1;         /* entry = SPLIT; exit via out1 */

        } else if (b == 0x0B || b == 0x0C) {      /* (b) \b / \B : zero-width assert */
            State *a = NEW(); a->op = OP_ASSERT; a->arg = (char)b; a->out = NULL;
            *tail = a; tail = &a->out;

        } else if (b == 0x0E) {                   /* (c) standalone 0x0E — should NOT occur */
            continue;                              /* skip defensively */

        } else {                                  /* (d) consuming element X */
            State *c = NEW(); c->op = OP_CHAR; c->arg = (char)b; c->out = NULL;
            if ((unsigned char)p[1] == 0x0E) {    /* X+ : one-or-more, LINEAR */
                State *sp = NEW(); sp->op = OP_SPLIT; sp->out = c; sp->out1 = NULL;
                c->out = sp;                      /* after one X, reach the split */
                *tail = c; tail = &sp->out1;      /* entry = c; exit via split.out1 */
                p++;                              /* consume the 0x0E marker */
            } else {
                *tail = c; tail = &c->out;        /* plain single consuming element */
            }
        }
    }

    /* (e) End: append the single accepting state into the final dangling slot. */
    State *m = NEW(); m->op = OP_MATCH;
    *tail = m;

    /* Zero lastlist on every allocated state (pool is fresh each call). */
    for (int i = 0; i < n; i++) pool[i].lastlist = 0;
    *nstates_out = n;
    #undef NEW
    return start;
}
```

**Key observations from the C:**
- The `tail` pointer (`State **`) threads the dangling exit slot. It is initially
  `&start`; each unit writes `*tail = entry` then repoints `tail` at its own exit
  (`&sp->out1` for SPLIT-based units, `&c->out` / `&a->out` for CHAR/ASSERT).
- **Allocation order matters for the START index.** The firmware allocates
  `any` THEN `sp` for the glob, so for a *leading* glob the entry `sp` is at index 1
  (not 0). `start` is therefore NOT always 0 in the firmware. See §4 for the
  Rust-safe resolution.
- The glob's `any->out = sp` is a **backward edge** (loop-back). This is why the
  `out` field CANNOT be made implicit ("next index") for `Any` — it must be stored.
- `lastlist` zeroing is a **simulator** concern, not a compiler-structure concern.
- `NEW()` clamps at `NFA_MAX_STATES-1` (reuse last slot) on overflow — a
  pathological-pattern guard; on a host with a growable `Vec` this is moot.

---

## 2. Index-based Rust design (the item-spec mandate)

The item spec mandates: *"using indices instead of pointers for Rust safety"*
and sketches `enum NfaOp { Char(u8), Any, Split(usize, usize), Assert(u8), Match }`.

**Problem with the literal sketch:** `Char(u8)` / `Any` / `Assert(u8)` carry NO
`out` edge. But the simulator (P2.M1.T2.S2) MUST follow `out` for every consuming
state, and the glob's `Any.out` is a **backward** loop-back edge (not `index+1`).
So `out` cannot be implicit. **Refinement (justified):** carry `out` explicitly
as a named field on `Char` / `Any` / `Assert`; carry both edges on `Split`.
Firmware field names (`out`, `out1`) are preserved for parity cross-checking.

```rust
/// A single compiled NFA node — index-based Thompson construction (no raw
/// pointers). The firmware `State` analog minus `lastlist` (the simulator owns
/// dedup state separately; see `nfa_addstate`, P2.M1.T2.S2).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum NfaOp {
    /// Consume one input byte matching `arg`; then go to `out`.
    /// `arg` is a `process_escapes` placeholder byte: an escaped literal
    /// (0x01-0x04), a class (0x05-0x0A), the dot (0x0D), a literal `.`/`+`,
    /// or any ordinary ASCII byte. Tested via `pattern_char_matches` (T3.S1).
    Char { arg: u8, out: usize },
    /// Consume ANY one byte INCLUDING newline/CR (glob `*` compiled as `.*`).
    /// Distinct from the dot (`Char{arg:0x0D}`), which excludes `\n`/`\r`.
    Any { out: usize },
    /// Epsilon fork: follow BOTH `out` and `out1` without consuming input.
    /// Implements `*` (glob) and `+` (one-or-more). The simulator's lastlist
    /// guard prevents infinite recursion when branches converge.
    Split { out: usize, out1: usize },
    /// Zero-width assertion: `arg` is 0x0B (`\b`, want a boundary) or 0x0C
    /// (`\B`, want a non-boundary). The simulator recurses into `out` only if
    /// `is_word_boundary(string_start, abspos) == (arg == 0x0B)`.
    Assert { arg: u8, out: usize },
    /// Accepting state (terminal). The simulator reports a match when a Match
    /// node is on the current state list.
    Match,
}
```

**Why `lastlist` is NOT a field here (deliberate separation of concerns):**
In the firmware `lastlist` + `nfa_gen` live on `State`/file-scope because the
simulator mutates them. In Rust the compiled `Vec<NfaOp>` is **pure immutable
program data** (compile once, simulate many times against many strings); the
simulator (P2.M1.T2.S2) will own a separate `Vec<u32>` generation-tag list (or
`Vec<usize>` seen-at list) parallel to `states`, reset via a generation counter.
This is cleaner than the firmware and is strictly out of scope for T2.S1.
`nfa_compile` therefore does NOT zero `lastlist` (there is none to zero).

---

## 3. The `tail` pointer — index-based translation

The firmware `State **tail` points at a SLOT to write the next entry index into.
The three possible slots:
1. `&start` — the function-local start variable (first unit).
2. `&state->out` — a consuming state's primary exit (CHAR/ASSERT, plain CHAR).
3. `&state->out1` — a SPLIT state's secondary exit (glob `*`, `X+`).

Index-based analog (a tiny enum the compiler threads by value; no borrow issues):

```rust
/// The dangling exit slot: where the NEXT unit's entry index must be written.
enum DanglingExit {
    /// Write into the function's `start` slot (no unit compiled yet).
    Start,
    /// Write into `states[idx].out` (a Char/Any/Assert primary edge).
    Out(usize),
    /// Write into `states[idx].out1` (a Split secondary edge — the exit of glob/X+).
    SplitSecond(usize),
}
```

Resolving it (writes into a prior state's field via index access — borrow released
per statement, so `&mut states[idx]` never aliases an in-progress push):

```rust
impl NfaOp {
    /// Write the next-state index into this state's PRIMARY `out` edge.
    /// Only valid on Char/Any/Assert (Split's primary edge is set at allocation;
    /// Match is terminal). No-op on Split/Match = defensive (logic bug if hit).
    fn set_out(&mut self, idx: usize) {
        match self {
            NfaOp::Char { out, .. } => *out = idx,
            NfaOp::Any { out } => *out = idx,
            NfaOp::Assert { out, .. } => *out = idx,
            NfaOp::Split { .. } | NfaOp::Match => {}
        }
    }
    /// Write the next-state index into a Split's SECONDARY `out1` edge.
    fn set_out1(&mut self, idx: usize) {
        if let NfaOp::Split { out1, .. } = self { *out1 = idx; }
    }
}
```

---

## 4. The START-index question — resolved (start is ALWAYS index 0)

The firmware's `start` is NOT always 0: a *leading* glob allocates `any`@0 then
`sp`@1, entry=`sp`@1. The item spec's signature `fn nfa_compile(pat: &[u8]) ->
Vec<NfaOp>` returns ONLY the Vec (no start index). Two ways to reconcile:

**(A)** Return `(Vec<NfaOp>, usize)` — most faithful to firmware allocation order,
but deviates from the item-spec signature.
**(B)** Guarantee `start == 0` by allocating the glob's SPLIT **before** its ANY
(a semantically-equivalent reorder; only allocation order within the unit changes,
the edge structure is identical).

**Chosen: (B).** Rationale:
- Honors the item-spec signature `-> Vec<NfaOp>` exactly (start implicit = 0).
- Edge structure is byte-for-byte equivalent to the firmware (only the index
  *numbers* of the two glob states swap: firmware `any`@0/`sp`@1 → Rust
  `sp`@0/`any`@1). Match semantics are identical — the simulator follows edges,
  not indices.
- For ALL units, the entry state is allocated FIRST within the unit, so the very
  first unit's entry is always index 0. Traced for every construct:
  - glob `*`: entry=SPLIT → allocate SPLIT@0 first, ANY@1 second. ✓ start=0
  - `X+`: entry=CHAR → CHAR@0, SPLIT@1 (firmware already allocates CHAR first). ✓
  - plain char: entry=CHAR@0. ✓
  - assert: entry=ASSERT@0. ✓
  - empty pattern: no unit; MATCH allocated@0, `start` slot ← 0. ✓
- Downstream (P2.M1.T2.S2) simulator simply seeds closure from `states[0]`.

**Empty-pattern edge case (parity with firmware):** `nfa_compile(b"")` →
`vec![NfaOp::Match]`, start=0. The firmware likewise allocates only MATCH and
returns it as start; the simulator's `if (!start)` guard is dead code (start is
never NULL / index 0 always exists). The Rust simulator handles the single-Match
case normally (closure immediately contains a Match → empty prefix matched).

**Allocation order for the glob (the ONLY reorder):**
```text
firmware:   any@k   (out=sp)        sp@k+1  (out=any, out1=exit)
Rust (B):   sp@k    (out=any, out1=exit)   any@k+1  (out=sp)
```
Same graph, swapped node indices. Documented in the rustdoc + a test comment.

---

## 5. Verified Rust skeleton (the spec — match exactly)

```rust
pub(crate) fn nfa_compile(pat: &[u8]) -> Vec<NfaOp> {
    let mut states: Vec<NfaOp> = Vec::with_capacity(pat.len() * 2 + 2);
    let mut start: Option<usize> = None;           // start slot; resolved to 0 for first unit
    let mut tail = DanglingExit::Start;            // dangling exit slot

    let mut i = 0;
    while i < pat.len() {
        let b = pat[i];
        if b == GLOB_STAR {                        // (a) glob '*' == regex '.*'
            // REORDER vs firmware: allocate SPLIT first so the unit entry is
            // the lowest index (keeps `start == 0` for a leading glob). The
            // edge graph is identical to the firmware (any<->sp loop).
            let sp_idx = states.len();
            states.push(NfaOp::Split { out: 0, out1: 0 });   // both filled below
            let any_idx = states.len();
            states.push(NfaOp::Any { out: 0 });              // filled below
            // sp.out = ANY ; any.out = sp (loop back)
            states[sp_idx].set_out(any_idx);
            states[any_idx].set_out(sp_idx);
            resolve(&mut states, &mut start, &tail, sp_idx); // *tail = sp (entry)
            tail = DanglingExit::SplitSecond(sp_idx);        // exit via sp.out1
            i += 1;
        } else if b == ASSERT_BOUND || b == ASSERT_NBOUND {  // (b) \b / \B
            let a_idx = states.len();
            states.push(NfaOp::Assert { arg: b, out: 0 });   // out filled by next resolve
            resolve(&mut states, &mut start, &tail, a_idx);
            tail = DanglingExit::Out(a_idx);                 // exit via a.out
            i += 1;
        } else if b == PLUS_QUANT {                          // (c) stray 0x0E — skip
            i += 1;                                           // (process_escapes never emits it standalone)
        } else {                                             // (d) consuming element X
            let c_idx = states.len();
            states.push(NfaOp::Char { arg: b, out: 0 });
            if i + 1 < pat.len() && pat[i + 1] == PLUS_QUANT {  // X+ : LINEAR
                let sp_idx = states.len();
                states.push(NfaOp::Split { out: c_idx, out1: 0 }); // out1 filled by next resolve
                states[c_idx].set_out(sp_idx);               // after one X -> split
                resolve(&mut states, &mut start, &tail, c_idx);
                tail = DanglingExit::SplitSecond(sp_idx);    // exit via split.out1
                i += 2;                                       // consume X AND the 0x0E marker
            } else {
                resolve(&mut states, &mut start, &tail, c_idx);
                tail = DanglingExit::Out(c_idx);             // exit via c.out
                i += 1;
            }
        }
    }

    // (e) End: append the single accepting state into the final dangling slot.
    let m_idx = states.len();
    states.push(NfaOp::Match);
    resolve(&mut states, &mut start, &tail, m_idx);
    debug_assert_eq!(start, Some(0));   // invariant: first unit's entry is always index 0

    states   // start is implicitly states[0]
}

/// Write `entry_idx` into the dangling exit slot `tail` describes (the index-based
/// analog of the firmware `*tail = entry`). `start` receives the FIRST entry.
fn resolve(states: &mut [NfaOp], start: &mut Option<usize>, tail: &DanglingExit, entry_idx: usize) {
    match tail {
        DanglingExit::Start => *start = Some(entry_idx),
        DanglingExit::Out(idx) => states[*idx].set_out(entry_idx),
        DanglingExit::SplitSecond(idx) => states[*idx].set_out1(entry_idx),
    }
}
```

**Borrow-checker note:** `resolve` borrows `states` and `start` by mutable ref,
and `tail` by shared ref — all disjoint, no aliasing. The `&tail` read happens
before any push (we read `tail` to resolve the PREVIOUS exit, THEN push the new
unit). Each push releases its borrow immediately. Compiles clean.

---

## 6. Parity table — exact `Vec<NfaOp>` for representative inputs (test contract)

Inputs are **processed-pattern byte slices** (`parse_pattern().core` /
`process_escapes()` output) — NOT raw pattern strings. Built with the named consts
from S1 (`GLOB_STAR`, `ASSERT_BOUND`, `ASSERT_NBOUND`, `PLUS_QUANT`, `CLASS_DIGIT`,
`DOT_META`, …). `C(x)` = `NfaOp::Char { arg: x, out: .. }`, `Sp(o,o1)` = `Split`,
`A(o)` = `Any { out: o }`, `As(x,o)` = `Assert`, `M` = `Match`.

### 6.1 Simple chains
| # | input bytes        | compiled NFA (states[])                         | start |
|---|--------------------|--------------------------------------------------|-------|
| 1 | `b""` (empty)      | `[M]`                                            | 0     |
| 2 | `b"a"` [0x61]      | `[C(0x61,out:1), M]`                             | 0     |
| 3 | `b"abc"` [61,62,63]| `[C(61,o:1), C(62,o:2), C(63,o:3), M]`          | 0     |

### 6.2 Glob `*` (OP_SPLIT + OP_ANY loop) — the reorder case
| # | input bytes        | compiled NFA                                     | start |
|---|--------------------|--------------------------------------------------|-------|
| 4 | `b"*"` [0x2A]      | `[Sp(o:1,o1:2), A(o:0), M]`                      | 0     |
| 5 | `b"a*"` [61,2A]    | `[C(61,o:1), Sp(o:2,o1:3), A(o:1), M]`           | 0     |
| 6 | `b"*a"` [2A,61]    | `[Sp(o:1,o1:2), A(o:0), C(61,o:3), M]`           | 0     |
| 7 | `b".*"` [0D,2A]    | `[C(0D,o:1), Sp(o:2,o1:3), A(o:1), M]`           | 0     |

Row 4 trace (reorder): sp@0 push, any@1 push, sp.out=1, any.out=0, resolve(Start→0),
tail=SplitSecond(0); end m@2, resolve(SplitSecond(0)→ states[0].out1=2). ✓

### 6.3 Quantifier `X+` (CHAR + SPLIT loop-back — the linear-time guarantee)
| # | input bytes         | compiled NFA                                     | start |
|---|---------------------|--------------------------------------------------|-------|
| 8 | `b"a+"` [61,0E]     | `[C(61,o:1), Sp(o:0,o1:2), M]`                   | 0     |
| 9 | `b"\\d+"` [05,0E]   | `[C(05,o:1), Sp(o:0,o1:2), M]`                   | 0     |
|10 | `b".+"`  [0D,0E]    | `[C(0D,o:1), Sp(o:0,o1:2), M]`                   | 0     |

### 6.4 The exponential-backtracker killer — `a+a+a+` (2k+1 states, NOT 2^k)
| # | input bytes                  | compiled NFA                                          |
|---|------------------------------|-------------------------------------------------------|
|11 | `b"a+a+a+"` [61,0E,61,0E,61,0E] | `[C(61,o:1), Sp(o:0,o1:2), C(61,o:3), Sp(o:2,o1:4), C(61,o:5), Sp(o:4,o1:6), M]` |

7 states = 2·3+1. This is the whole reason Thompson construction exists. Assert it.

### 6.5 Boundary assertions (OP_ASSERT, zero-width)
| # | input bytes         | compiled NFA                                     |
|---|---------------------|--------------------------------------------------|
|12 | `b"\\b"` [0B]       | `[As(0B,o:1), M]`                                |
|13 | `b"\\B"` [0C]       | `[As(0C,o:1), M]`                                |
|14 | `b"\\bword"` [0B,77,6F,72,64] | `[As(0B,o:1), C(77,o:2), C(6F,o:3), C(72,o:4), C(64,o:5), M]` |

### 6.6 Mixed / stray-0x0E defensive
| # | input bytes         | compiled NFA                                     | note |
|---|---------------------|--------------------------------------------------|------|
|15 | `b"ab+"` [61,62,0E] | `[C(61,o:1), C(62,o:2), Sp(o:1,o1:3), M]`        | b+ is a 2-state unit |
|16 | [0E] (stray)        | `[M]`                                            | branch (c) skips the lone 0x0E |

Row 15 trace: a plain char@0(out resolved→1); then b+ : c_b@1, sp@2, c_b.out=2,
resolve(prev Out(0)→states[0].out=1)... wait — let me re-trace carefully.
- i=0 `a`: c@0 push Char{61,out:0}; pat[1]=0x62≠0x0E → plain. resolve(Start→start=0); tail=Out(0). i=1.
- i=1 `b`: c@1 push Char{62,out:0}; pat[2]=0x0E → X+. sp@2 push Split{out:1,out1:0}; states[1].out=2; resolve(Out(0)→states[0].out=1); tail=SplitSecond(2). i=3 (done).
- end: m@3 push Match; resolve(SplitSecond(2)→states[2].out1=3).
Result: `[C(61,o:1), C(62,o:2), Sp(o:1,o1:3), M]`. ✓ (matches the table.)

### 6.7 Literal-dot / literal-plus as Char (escapes already resolved by S1)
| # | input bytes            | compiled NFA                          | note |
|---|------------------------|---------------------------------------|------|
|17 | `b"a."` [61,0D]        | `[C(61,o:1), C(0D,o:2), M]`           | dot meta is a Char{arg:0x0D} |
|18 | `b"a\\."`→[61,2E]      | `[C(61,o:1), C(2E,o:2), M]`           | literal dot 0x2E is a plain Char |

---

## 7. Gotchas summary (the failure magnets)

- **GOTCHA-1 (start index):** A *leading* glob would make firmware `start`=1.
  The Rust port keeps `start == 0` by allocating the glob's SPLIT before its ANY
  (semantically-equivalent reorder). If an implementer copies the firmware's
  `any`-then-`sp` order verbatim, `start` is 1 for `b"*"` and the downstream
  simulator (T2.S2) seeding from `states[0]` would start on the wrong node.
  Pinned by test row 4 (`b"*"` → entry is `Split`, index 0).
- **GOTCHA-2 (glob loop-back):** `Any.out` points BACKWARD to the SPLIT (not
  `index+1`). `out` must be an explicit stored field on `Any` — it cannot be
  implicit. The item-spec sketch `Any` (no payload) is insufficient; refined to
  `Any { out: usize }`.
- **GOTCHA-3 (X+ consumes two source bytes):** branch (d) peeks `pat[i+1]==0x0E`
  and does `i += 2` (skip X AND the marker). Forgetting the `i += 2` and using
  `i += 1` re-emits the 0x0E as a standalone Char — corruption. The standalone
  branch (c) `continue` is only for a stray 0x0E that survived process_escapes
  (should never happen).
- **GOTCHA-4 (glob allocates TWO states; X+ allocates TWO states):** never one.
  `*` is never a single state. The `tail` after a glob/X+ is always a
  `SplitSecond` (the SPLIT's `out1` exit), never an `Out`.
- **GOTCHA-5 (no lastlist here):** `nfa_compile` does NOT touch any generation
  tag — `NfaOp` has no `lastlist` field. Dedup is the simulator's (T2.S2) job.
  Do NOT add a `lastlist`/`gen` field to the enum or a file-scope `static`.
- **GOTCHA-6 (empty pattern → single Match):** `nfa_compile(b"")` must return
  `vec![NfaOp::Match]` (NOT empty Vec). The firmware allocates MATCH unconditionally
  at the end and threads it into the `start` slot. A downstream exact-`^$` match
  against `""` relies on this single-Match closure.
- **GOTCHA-7 (NFA_MAX_STATES clamp):** the firmware clamps allocation at
  `NFA_MAX_STATES-1` (reuse last slot) for pathological patterns. The Rust `Vec`
  grows unboundedly; on a host, a `rules.toml` pattern never approaches any cap.
  Do NOT replicate the clamp (it corrupts the NFA on overflow by design); just
  let the Vec grow. Document the firmware behavior for parity awareness only.
- **GOTCHA-8 (branch order):** the `0x2A` (glob) and `0x0B/0x0C` (assert) checks
  MUST come before the generic consuming-element `else` branch, because those
  bytes would otherwise be compiled as plain `Char`. And `0x0E` (stray quant)
  must be checked-and-skipped, NOT compiled as a Char. Match arms in exactly the
  firmware order: glob → assert → stray-0x0E → else(consuming).

---

## 8. External reference (the linear-time guarantee — cite in rustdoc)

- Russ Cox, *"Regular Expression Matching Can Be Simple And Fast"*
  (https://swtch.com/~rsc/regexp/regexp1.html) — the Thompson-construction
  reference the firmware rustdoc already cites (PRD §7.5, §7.9). The key claim
  for the rustdoc: **X+ compiles to exactly 2 states (CHAR + SPLIT loop-back), so
  `a+a+a+…` scales as 2k+1 states — never 2^k** — and simulation is
  O(states × input_len) with no backtracking. Cite §"NFA-based Regular Expression
  Algorithms" + the `a(n+1)б` pathological example (the same class of input that
  made the old backtracker exponential, per PRD §7.8).
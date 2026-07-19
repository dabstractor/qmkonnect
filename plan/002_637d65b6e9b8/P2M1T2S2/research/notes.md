# Research Notes — P2.M1.T2.S2: Port epsilon-closure (`nfa_addstate`) + NFA simulation (`nfa_match`)

**Source of truth (PRD §14):** `/home/dustin/projects/qmk-notifier/pattern_match.c`
(lines ~454–628 = the char predicates + `nfa_addstate` + `pattern_char_matches` +
`nfa_has_match` + `nfa_match`; lines ~116–140 = `get_escaped_char`). This file
reproduces the C verbatim, derives the index-based Rust design (including TWO
justified signature refinements — see §2), gives a verified Rust skeleton, and a
parity table of `nfa_match`-level test vectors derived from the firmware test
corpus (`test_pattern_match.c`, `test_word_boundary_basic.c`).

---

## 1. The canonical firmware C (verbatim — the spec)

### 1a. Character classifiers (lines ~454–468)

```c
static bool is_digit_char(char c) { return c >= '0' && c <= '9'; }

static bool is_word_char(char c) {
    return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z')
        || (c >= '0' && c <= '9') || (c == '_');
}

static bool is_whitespace_char(char c) {
    return c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\f' || c == '\v';
}
```

### 1b. Word-boundary test (lines ~470–478)

```c
/* Word-boundary test against the ORIGINAL string (PRD §7.6, §13 #10). A boundary
 * exists at `pos` when exactly one of the neighboring characters is a word char.
 * Edge positions use an implicit non-word char on the off-string side. The
 * empty-original-string case is short-circuited inside nfa_addstate's OP_ASSERT
 * branch BEFORE this is called, but we keep the NULL + len guards defensive. */
static bool is_word_boundary(const char *str, size_t pos) {
    if (!str) return false;
    size_t str_len = strlen(str);
    if (pos == 0)        return (str_len > 0 && is_word_char(str[0]));
    if (pos == str_len)  return (str_len > 0 && is_word_char(str[str_len - 1]));
    if (pos > str_len)   return false;
    return is_word_char(str[pos - 1]) != is_word_char(str[pos]);
}
```

### 1c. Epsilon-closure add (lines ~480–540)

```c
/* ---- epsilon-closure add (follow SPLIT/ASSERT, collect CHAR/ANY/MATCH) ---- */
static void nfa_addstate(State **list, int *n, State *s,
                         const char *string_start, size_t abspos) {
    /* De-dup + NULL-safe: skip if `s` is NULL or already in THIS closure.
     * lastlist == nfa_gen means we have already added/followed `s` during the
     * current simulation phase (nfa_gen is bumped once per phase by nfa_match).
     * This single guard is what makes OP_SPLIT and \b\b terminate (PRD §13 #11). */
    if (!s || s->lastlist == nfa_gen) return;     /* already in this closure */

    /* Mark seen for THIS generation BEFORE dispatching, so a state reached via an
     * OP_SPLIT branch is not re-added when the OTHER branch converges on it. */
    s->lastlist = nfa_gen;

    if (s->op == OP_MATCH) {
        list[(*n)++] = s;            /* collect accepting state */
        return;
    }

    if (s->op == OP_SPLIT) {
        /* Epsilon fork (glob '*', 'X+'): follow BOTH out and out1 WITHOUT
         * consuming input. abspos is forwarded UNCHANGED to both branches
         * (PRD §13 #10: abspos is absolute from string_start; epsilon edges do
         * not advance the input position). */
        nfa_addstate(list, n, s->out,  string_start, abspos);
        nfa_addstate(list, n, s->out1, string_start, abspos);
        return;
    }

    if (s->op == OP_ASSERT) {
        /* Zero-width assertion \b (arg==0x0B, want a boundary) / \B (arg==0x0C,
         * want a NON-boundary). Recurse into `out` ONLY if the boundary
         * condition holds. abspos is absolute (PRD §13 #10) so \b/\B evaluate
         * against the ORIGINAL string, not the per-offset pointer.
         *
         * EMPTY-STRING SPECIAL CASE (legacy semantics the test suite encodes):
         * if the original string is empty (*string_start == '\0'), NEITHER a
         * boundary nor a non-boundary passes, so we do NOT recurse. The empty-
         * string check short-circuits BEFORE calling is_word_boundary, so this
         * behavior is independent of the is_word_boundary implementation. */
        int want_boundary = (s->arg == 0x0B);     /* \b wants a boundary; \B wants none */
        if (*string_start != '\0' &&
            is_word_boundary(string_start, abspos) == want_boundary)
            nfa_addstate(list, n, s->out, string_start, abspos);
        return;                                   /* never collect an ASSERT state itself */
    }

    /* OP_CHAR / OP_ANY: a consuming state. Add it to the list; it is "live" and
     * waiting for the simulator to feed it the next input char (nfa_match). */
    list[(*n)++] = s;
}
```

### 1d. `get_escaped_char` — escaped-literal decoder (lines ~116–140)

```c
static char get_escaped_char(char placeholder) {
    switch (placeholder) {
        case '\x01': return '^';   /* \^ */
        case '\x02': return '$';   /* \$ */
        case '\x03': return '*';   /* \* */
        case '\x04': return '\\';  /* \\ */
        /* 0x05-0x0D are classes/assertions handled directly in pattern_char_matches;
         * returned here only for debug/error messages. */
        ...
        default:     return placeholder;  /* ordinary literal byte */
    }
}
```

### 1e. `pattern_char_matches` — single-byte match predicate (lines ~542–575)

```c
/* Test whether a processed-pattern byte `pc` matches an input char `sc`.
 * Escaped-literal placeholders are decoded via get_escaped_char() FIRST and then
 * folded — never fold the placeholder byte itself. tolower() takes an unsigned
 * char value, so args are cast to (unsigned char) to avoid sign-extension UB. */
static bool pattern_char_matches(char pc, char sc, bool case_sensitive) {
    if (pc >= '\x01' && pc <= '\x04') {                 /* escaped literal */
        char literal = get_escaped_char(pc);
        return case_sensitive ? (literal == sc)
              : (tolower((unsigned char)literal) == tolower((unsigned char)sc));
    }
    switch (pc) {
        case '\x05': return is_digit_char(sc);          /* \d */
        case '\x06': return !is_digit_char(sc);         /* \D */
        case '\x07': return is_word_char(sc);           /* \w */
        case '\x08': return !is_word_char(sc);          /* \W */
        case '\x09': return is_whitespace_char(sc);     /* \s */
        case '\x0A': return !is_whitespace_char(sc);    /* \S */
        case '\x0D': return (sc != '\n' && sc != '\r'); /* .  (dot excludes newline) */
        default:                                        /* ordinary literal */
            return case_sensitive ? (pc == sc)
                  : (tolower((unsigned char)pc) == tolower((unsigned char)sc));
    }
}
```

### 1f. `nfa_has_match` + `nfa_match` — the two-list simulation (lines ~577–625)

```c
/* Report whether an accepting OP_MATCH state is on the current state list. */
static int nfa_has_match(State **list, int n) {
    for (int i = 0; i < n; i++) if (list[i]->op == OP_MATCH) return 1;
    return 0;
}

/* full_match=false: MATCH reachable at any point (prefix/substring match).
 * full_match=true:  MATCH reachable only after consuming the WHOLE string. */
static bool nfa_match(const char *pattern, const char *str,
                      const char *string_start, bool case_sensitive,
                      bool full_match) {
    State pool[NFA_MAX_STATES];
    int nstates;
    State *start = nfa_compile(pattern, pool, &nstates);
    if (!start) return full_match ? (*str == '\0') : true;   /* defensive guard */
    (void)nstates;

    State *clist_buf[NFA_MAX_STATES];
    State *nlist_buf[NFA_MAX_STATES];
    State **clist = clist_buf, **nlist = nlist_buf;
    int cn = 0, nn;
    size_t abspos = (size_t)(str - string_start);            /* absolute offset */

    nfa_gen++;                                               /* seed closure */
    nfa_addstate(clist, &cn, start, string_start, abspos);
    if (!full_match && nfa_has_match(clist, cn)) return true;/* empty prefix matched */

    size_t pos = abspos;
    for (const char *p = str; *p; p++, pos++) {
        char c = *p;
        nfa_gen++; nn = 0;                                   /* fresh phase */
        for (int i = 0; i < cn; i++) {
            State *s = clist[i];
            if (s->op == OP_ANY) {                           /* glob '*': ANY byte incl \n/\r (PRD §13 #8) */
                nfa_addstate(nlist, &nn, s->out, string_start, pos + 1);
            } else if (s->op == OP_CHAR &&
                       pattern_char_matches(s->arg, c, case_sensitive)) {
                nfa_addstate(nlist, &nn, s->out, string_start, pos + 1);
            }
        }
        State **tmp = clist; clist = nlist; nlist = tmp; cn = nn;  /* swap lists */
        if (cn == 0) break;                                  /* dead — no live states */
        if (!full_match && nfa_has_match(clist, cn)) return true; /* prefix matched */
    }
    return nfa_has_match(clist, cn) ? true : false;          /* full: accept only at end */
}
```

**Key observations from the C:**
- `nfa_match` takes **TWO string pointers**: `str` (consume-from-here, = `string_start + abspos`) and `string_start` (the ORIGINAL). `abspos = str - string_start`. This pair exists *solely* so `\b`/`\B` evaluate against the **original** string at an **absolute** offset (PRD §13 #10; the C comment: *"abspos is absolute from string_start; epsilon edges do not advance the input position"*). See §2.1 — this forces a signature refinement in Rust.
- `nfa_gen` is bumped **once per phase** (once for the seed closure, once per consumed char). `nfa_addstate`'s `lastlist == nfa_gen` check then de-dups the closure in O(1) per state with **no per-phase clearing**. This is the documented O(1)-dedup property (PRD §13 #11).
- The firmware `nfa_match` **calls `nfa_compile` internally** (because C has no cheap way to pass the compiled pool around without it being `static`). The item spec, however, says *"INPUT: Compiled NFA from P2.M1.T2.S1"* — see §2.2 for the Rust resolution (take the compiled `&[NfaOp]`, decouple compile-once from simulate-many).
- `nfa_addstate` marks `lastlist = nfa_gen` **before** dispatching (so a Split's two branches converging on the same state don't double-add).
- `OP_ASSERT` is **never collected** onto the live list (it returns without `list[(*n)++]`); only the `out` edge is conditionally followed. Same for `OP_SPLIT`. Only `OP_CHAR`/`OP_ANY`/`OP_MATCH` land on the list.
- `is_word_boundary` is called with `string_start` (the ORIGINAL) + `abspos`, never the per-offset `str`.

---

## 2. Index-based Rust design (TWO justified signature refinements)

### 2.1 REFINEMENT A — `nfa_match` needs the ORIGINAL string + an absolute offset

The item-spec sketch: `fn nfa_match(pattern: &[u8], str_bytes: &[u8], case_sensitive: bool, full_match: bool) -> bool` — a SINGLE string param. But the firmware `nfa_match` takes **two** pointers (`str` + `string_start`) precisely so `\b`/`\B` see the **original** string at an **absolute** offset.

**A lone `str_bytes` slice is INSUFFICIENT.** Proven by the firmware parity vector
(`test_word_boundary_basic.c`):

```c
{"\\bword", "aword", true, false, "\\b: does not match when not at word boundary"},
```

For substring matching, the firmware loops offsets `i` and calls
`nfa_match(core, str+i, str, ...)` — so at offset `i=1`, `abspos=1`, and the leading
`\b` evaluates `is_word_boundary("aword", 1)` = `is_word_char('a') != is_word_char('w')`
= `true != true` = **false** → not a boundary → `\b` **fails** → no match. ✓ (correct:
`x|w` inside a word is not a boundary).

If instead the Rust port sliced `&"aword"[1..]` = `"word"` and treated THAT as both
the consume string AND `string_start` (abspos=0), the leading `\b` would evaluate
`is_word_boundary("word", 0)` = `is_word_char('w')` = **true** → boundary → `\b`
**passes** → "word" matches → **TRUE**. ✗ WRONG — diverges from firmware.

**Resolution (mirrors S1's `Any { out }` refinement — the sketch was insufficient
for correctness):** add a `start: usize` offset param. `string` is the **full
original** input (firmware `string_start`); `start` is the byte offset to begin
consuming (firmware `str - string_start`). T3.S1's `match_with_anchors` then maps
the firmware substring loop trivially: `for i in 0..=len { nfa_match(core, full, i, cs, false) }`.

```rust
pub(crate) fn nfa_match(
    states: &[NfaOp],     // compiled NFA (S1 nfa_compile output); start state = states[0]
    string: &[u8],        // the FULL ORIGINAL input string (firmware `string_start`)
    start: usize,         // byte offset into `string` to begin consuming (firmware `abspos` initial)
    case_sensitive: bool,
    full_match: bool,
) -> bool
```

This is faithful (identical edge semantics), explicit (no pointer arithmetic /
suffix-length tricks), and keeps `\b`/`\B` correct against the original string.

### 2.2 REFINEMENT B — `nfa_match` takes the COMPILED `&[NfaOp]`, not raw bytes

The sketch typed the first param `pattern: &[u8]` (the processed-pattern bytes),
but the item spec also says *"INPUT: Compiled NFA from P2.M1.T2.S1."* These
conflict. The firmware reconciles by having `nfa_match` call `nfa_compile`
**internally** (C can't pass the pool around cheaply without a `static`).

**Resolution (decouple compile-once from simulate-many):** `nfa_match` takes
`states: &[NfaOp]` — the **compiled** NFA from S1's `nfa_compile`. This honors
the *"INPUT: Compiled NFA"* spec, is idiomatic Rust (pass the `Vec<NfaOp>` by
ref), and lets T3.S1 compile a pattern ONCE then call `nfa_match` at many offsets
(the firmware wastefully recompiles on every `nfa_match` call — we don't). The
sketch's `pattern: &[u8]` is the imprecise rendering of "the thing being matched";
the real type is the compiled program. Tests compile via
`nfa_compile(&process_escapes(...))`.

### 2.3 REFINEMENT C — dedup via `Vec<u32>` generation tags (NOT `Vec<bool>`)

The sketch: `seen: &mut Vec<bool>` + `generation: usize`. A `bool` **cannot carry
a generation tag**, so a `Vec<bool>` would require an O(states) `fill(false)` per
phase. That works (the clear is dominated by the O(states × strlen) simulation)
but **abandons** the firmware's documented O(1)-dedup property (RESEARCH NOTE:
*"bump `nfa_gen` once per phase for O(1) dedup"*; PRD §13 #11). Carrying the tag
in the Vec restores it with **no per-phase clear**.

```rust
// seen[idx] == generation  ==>  state idx already followed in THIS closure.
// Bumped once per phase by nfa_match; NEVER cleared. Exactly the firmware
// lastlist/nfa_gen pair, relocated from the State struct to a simulator-owned
// Vec (the same separation-of-concerns S1 established by keeping lastlist off NfaOp).
seen: &mut Vec<u32>,   // len == states.len(); initialized to 0 (generation starts at 1)
generation: u32,
```

`u32` matches the firmware `int lastlist`/`static int nfa_gen`. Overflow is
impossible in practice (generation ≈ consumed-char-count + 1; a window title is
never 4 billion chars). `usize` is an acceptable alternative.

This keeps BOTH sketch params (`seen` + `generation`) present and meaningful, with
the single forced type change `Vec<bool>` → `Vec<u32>` (a bool literally cannot
hold the tag). Parallels S1's refinements.

---

## 3. Verified Rust skeleton (the spec — match exactly)

### 3a. Character classifiers (idiomatic `u8` methods == the C ranges exactly)

```rust
#[inline]
fn is_digit_char(c: u8) -> bool { c.is_ascii_digit() }   // == c >= b'0' && c <= b'9'

#[inline]
fn is_word_char(c: u8) -> bool {
    // [a-zA-Z0-9_]. is_ascii_alphanumeric == [a-zA-Z0-9]; add '_'.
    c.is_ascii_alphanumeric() || c == b'_'
}

#[inline]
fn is_whitespace_char(c: u8) -> bool {
    // ' ' '\t' '\n' '\r' '\x0c'(\f) '\x0b'(\v). is_ascii_whitespace is EXACTLY this set.
    // GOTCHA: must be is_ascii_whitespace, NOT char::is_whitespace (Unicode adds non-ASCII ws).
    c.is_ascii_whitespace()
}
```

### 3b. `is_word_boundary` — faithful port (the ORIGINAL string + absolute pos)

```rust
/// Word-boundary test against the ORIGINAL string at an absolute byte position.
/// A boundary exists at `pos` iff exactly one of the neighboring bytes is a word
/// char. Off-string edges use an implicit non-word char. Mirrors the firmware
/// `is_word_boundary` (pattern_match.c:~470) exactly. The empty-string special
/// case for `\b`/`\B` is handled by `nfa_addstate` BEFORE calling this (it checks
/// `string_start.is_empty()` first); the guards here are defensive.
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
```

### 3c. `decoded_literal` + `pattern_char_matches` (decode-then-fold, ASCII fold)

```rust
/// Decode an escaped-literal placeholder (0x01-0x04) back to its literal byte.
/// Mirrors firmware `get_escaped_char` for the four literal cases (the
/// class/assertion bytes 0x05-0x0D are handled directly in pattern_char_matches).
fn decoded_literal(pc: u8) -> u8 {
    match pc {
        ESC_CARET   => b'^',   // 0x01 -> \^
        ESC_DOLLAR  => b'$',   // 0x02 -> \$
        ESC_STAR    => b'*',   // 0x03 -> \*
        ESC_BSLASH  => b'\\',  // 0x04 -> \\
        _           => pc,     // not an escaped literal; ordinary byte
    }
}

/// Test whether a processed-pattern byte `pc` matches an input byte `sc`.
/// Faithful port of firmware `pattern_char_matches`. Escaped literals are
/// DECODED FIRST then ASCII-folded (never fold the placeholder byte). ASCII
/// folding via `to_ascii_lowercase` == C `tolower((unsigned char))` in the C
/// locale (both only fold A-Z; identity for all other bytes incl. UTF-8 cont.).
fn pattern_char_matches(pc: u8, sc: u8, case_sensitive: bool) -> bool {
    // Escaped literal: decode THEN compare (GOTCHA: never fold the placeholder).
    if pc >= ESC_CARET && pc <= ESC_BSLASH {   // 0x01..=0x04
        let lit = decoded_literal(pc);
        return if case_sensitive { lit == sc }
               else { lit.to_ascii_lowercase() == sc.to_ascii_lowercase() };
    }
    match pc {
        CLASS_DIGIT  => is_digit_char(sc),         // \d
        CLASS_NDIGIT => !is_digit_char(sc),        // \D
        CLASS_WORD   => is_word_char(sc),          // \w
        CLASS_NWORD  => !is_word_char(sc),         // \W
        CLASS_SPACE  => is_whitespace_char(sc),    // \s
        CLASS_NSPACE => !is_whitespace_char(sc),   // \S
        DOT_META     => sc != b'\n' && sc != b'\r',// .  (dot excludes newline/CR)
        _ => {                                       // ordinary literal
            if case_sensitive { pc == sc }
            else { pc.to_ascii_lowercase() == sc.to_ascii_lowercase() }
        }
    }
}
```

### 3d. `nfa_addstate` — epsilon closure (the index-based core)

```rust
/// Epsilon-closure add: follow SPLIT/ASSERT edges (consuming no input), collect
/// CHAR/ANY/MATCH states onto the live list. Faithful port of firmware
/// `nfa_addstate`. Dedup via the generation tag: `seen[idx] == generation` means
/// idx was already followed in THIS closure (the guard that makes `*` / `\b\b`
/// terminate — PRD §13 #11). `string_start` + `abspos` are forwarded UNCHANGED
/// across epsilon edges so `\b`/`\B` evaluate against the ORIGINAL string at an
/// absolute offset (PRD §13 #10). RECURSIVE; depth bounded by the longest
/// epsilon chain (<< states.len()).
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
    // Mark seen BEFORE dispatching (firmware sets `lastlist = nfa_gen` first), so a
    // state reached via one Split branch isn't re-added when the other converges.
    seen[idx] = generation;

    match states[idx] {
        NfaOp::Match => {
            // Accepting state: collect it; nfa_has_match reports the match.
            list.push(idx);
        }
        NfaOp::Split { out, out1 } => {
            // Epsilon fork (glob '*', 'X+'): follow BOTH edges, abspos UNCHANGED.
            nfa_addstate(states, out, list, seen, generation, string_start, abspos);
            nfa_addstate(states, out1, list, seen, generation, string_start, abspos);
        }
        NfaOp::Assert { arg, out } => {
            // Zero-width \b (0x0B, want boundary) / \B (0x0C, want NON-boundary).
            // EMPTY-STRING SPECIAL CASE: if the original string is empty, NEITHER
            // a boundary nor a non-boundary passes — do NOT recurse. (Firmware
            // checks `*string_start != '\0'` BEFORE is_word_boundary.)
            let want_boundary = arg == ASSERT_BOUND;
            if !string_start.is_empty()
                && is_word_boundary(string_start, abspos) == want_boundary
            {
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
```

### 3e. `nfa_has_match` + `nfa_match` — the two-list simulation

```rust
/// True iff an accepting `Match` state is on the current live list.
fn nfa_has_match(states: &[NfaOp], list: &[usize]) -> bool {
    list.iter().any(|&idx| states[idx] == NfaOp::Match)
}

/// Two-list Thompson NFA simulation. `full_match=false` ⇒ a `Match` reachable at
/// ANY point (prefix/substring) returns true; `full_match=true` ⇒ `Match` must be
/// reachable only after consuming the WHOLE remaining string (exact/suffix).
/// Guaranteed O(states × consumed_len), no backtracking (the fix for the old
/// exponential matcher; PRD §7.8). See Russ Cox, "Regular Expression Matching
/// Can Be Simple And Fast" — https://swtch.com/~rsc/regexp/regexp1.html .
pub(crate) fn nfa_match(
    states: &[NfaOp],      // compiled NFA (S1 nfa_compile output); start = states[0]
    string: &[u8],         // FULL ORIGINAL input (firmware `string_start`) — for \b/\B
    start: usize,          // byte offset to begin consuming (firmware `abspos` initial)
    case_sensitive: bool,
    full_match: bool,
) -> bool {
    debug_assert!(!states.is_empty(), "nfa_compile always yields >= [Match]");
    // Firmware defensive guard `if (!start) return full_match ? (*str=='\0') : true`
    // is dead in Rust: nfa_compile never returns an empty Vec, so states[0] always
    // exists. (Documented GOTCHA-1.)

    let mut clist: Vec<usize> = Vec::with_capacity(states.len());
    let mut nlist: Vec<usize> = Vec::with_capacity(states.len());
    let mut seen: Vec<u32> = vec![0u32; states.len()]; // generation-tag dedup (0 = unseen)
    let mut generation: u32 = 0;

    let mut pos = start; // abspos: absolute offset into `string` (for \b/\B)

    // Seed the closure from states[0] (the start — always index 0, S1 invariant).
    generation += 1;
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
                    nfa_addstate(states, out, &mut nlist, &mut seen, generation, string, pos + 1);
                }
                NfaOp::Char { arg, out } => {
                    if pattern_char_matches(arg, c, case_sensitive) {
                        nfa_addstate(states, out, &mut nlist, &mut seen, generation, string, pos + 1);
                    }
                }
                // Match/Assert/Split are never on the LIVE list (addstate resolves
                // them via epsilon edges); skip silently.
                _ => {}
            }
        }
        std::mem::swap(&mut clist, &mut nlist); // swap lists (firmware pointer swap)
        pos += 1;
        if clist.is_empty() {
            break; // dead — no live states (firmware `if (cn == 0) break;`)
        }
        if !full_match && nfa_has_match(states, &clist) {
            return true; // prefix matched mid-stream
        }
    }

    nfa_has_match(states, &clist) // full_match: accept only at end; substring: already returned
}
```

**Borrow-checker note:** `nfa_addstate` borrows `states` (shared) + `list`/`seen`
(exclusive) — all disjoint. The inner simulation loop borrows `clist` (shared,
iterated) while pushing to `nlist` (exclusive) — different Vecs, no aliasing. The
`std::mem::swap` of two `Vec<usize>` is a 3-pointer swap (cheap; no allocation).
Compiles clean with no `unsafe`, no `static`, no `Cell`.

---

## 4. Parity table — `nfa_match`-level test vectors (the contract)

These call `nfa_match` DIRECTLY on a compiled NFA (via `nfa_compile(&process_escapes(pat))`)
at a specific `start` offset + `full_match` flag. They are DERIVED from the
firmare end-to-end corpus (`test_pattern_match.c`, `test_word_boundary_basic.c`)
by reasoning about what the simulator must return at one offset. The full
end-to-end corpus (anchor strategies) is ported in P2.M1.T4.S1; these validate
the SIMULATOR in isolation.

Notation: `m(pat_str, input, start, cs, full_match) -> bool`, where `pat_str` is
the HUMAN pattern (run through `process_escapes` + `nfa_compile` first).

### 4.1 Exact full-string (`full_match=true`, `start=0`)
| pattern | input | start | cs | full | expected | why |
|---------|-------|-------|----|------|----------|-----|
| `"test"` | `b"test"` | 0 | T | T | **true**  | consume whole string, reach Match |
| `"test"` | `b"testing"` | 0 | T | T | **false** | Match reached at pos 3, but string not exhausted → dead after |
| `"test"` | `b"tes"` | 0 | T | T | **false** | ran out of input before Match |
| `""` | `b""` | 0 | T | T | **true**  | empty pattern → [Match]; seed closure has Match; loop no-op |
| `""` | `b"a"` | 0 | T | T | **false** | Match in seed, but `a` unconsumed → dead list after step |
| `"abc"` | `b"ABC"` | 0 | F | T | **true**  | case-insensitive full match |
| `"abc"` | `b"ABC"` | 0 | T | T | **false** | case-sensitive mismatch |

### 4.2 Prefix / substring (`full_match=false`)
| pattern | input | start | cs | full | expected | why |
|---------|-------|-------|----|------|----------|-----|
| `"test"` | `b"testing"` | 0 | T | F | **true**  | prefix match (Match at pos 3) |
| `"test"` | `b"pretest"` | 0 | T | F | **false** | 't'≠'p' at offset 0 (T3.S1 retries offset 3) |
| `"test"` | `b"pretest"` | 3 | T | F | **true**  | prefix at offset 3 |
| `"*"` | `b"anything"` | 0 | T | F | **true**  | glob matches anything (incl empty prefix) |
| `"*"` | `b""` | 0 | T | F | **true**  | glob matches empty prefix (Split→out1→Match in seed) |
| `"a*"` | `b"aaa"` | 0 | T | F | **true**  | glob after 'a' |
| `"a*b"` | `b"aaab"` | 0 | T | T | **true**  | full match with mid glob |

### 4.3 Quantifier `+` (Char+SPLIT loop)
| pattern | input | start | cs | full | expected | why |
|---------|-------|-------|----|------|----------|-----|
| `"a+"` | `b"aaa"` | 0 | T | T | **true**  | one-or-more |
| `"a+"` | `b""` | 0 | T | T | **false** | needs ≥1 'a' |
| `"a+"` | `b"b"` | 0 | T | T | **false** | no leading 'a' |
| `"\\d+"` | `b"123"` | 0 | T | T | **true**  | class + quantifier |

### 4.4 Character classes (`\d \D \w \W \s \S`) + dot
| pattern | input | start | cs | full | expected | why |
|---------|-------|-------|----|------|----------|-----|
| `"\\d"` | `b"5"` | 0 | T | F | **true**  | \d matches digit |
| `"\\d"` | `b"a"` | 0 | T | F | **false** | \d no match letter |
| `"\\w"` | `b"_"` | 0 | T | F | **true**  | \w includes underscore |
| `"\\w"` | `b" "` | 0 | T | F | **false** | \w no match space |
| `"\\W"` | `b"!"` | 0 | T | F | **true**  | \W matches punctuation |
| `"\\s"` | `b"\t"` | 0 | T | F | **true**  | \s matches tab |
| `"\\S"` | `b"\t"` | 0 | T | F | **false** | \S no match whitespace |
| `"."` | `b"a"` | 0 | T | F | **true**  | dot matches any non-newline |
| `"."` | `b"\n"` | 0 | T | F | **false** | dot EXCLUDES newline |
| `"."` | `b"\r"` | 0 | T | F | **false** | dot EXCLUDES CR |

### 4.5 Word boundary `\b` — **THE critical abspos tests** (proves \b sees the ORIGINAL string)
| pattern | input | start | cs | full | expected | why |
|---------|-------|-------|----|------|----------|-----|
| `"\\bword"` | `b"word"` | 0 | T | F | **true**  | abspos 0: is_word_char('w')→boundary; "word" prefix-matches |
| `"\\bword"` | `b"aword"` | 1 | T | F | **false** | abspos 1: 'a','w' both word→NO boundary→\b FAILS (sees original!) |
| `"\\bword"` | `b" word"` | 1 | T | F | **true**  | abspos 1: ' ' non-word,'w' word→boundary; matches slice "word" |
| `"\\bword"` | `b"123word"` | 3 | T | F | **false** | abspos 3: '3','w' both word→NO boundary→\b fails |
| `"\\bword"` | `b"_word"` | 1 | T | F | **false** | abspos 1: '_','w' both word→NO boundary→\b fails |
| `"\\Bord"` | `b"word"` | 1 | T | F | **true**  | abspos 1: 'w','o' both word→NOT boundary→\B (non-boundary) passes |
| `"\\Bord"` | `b"ord"` | 0 | T | F | **false** | abspos 0: is_word_char('o')→boundary→\B fails |

**The `\bword`/`aword`@1 row is the linchpin:** if `\b` saw only the slice
`"word"` (treating it as string_start at abspos 0), it would see 'w'→boundary→pass
→ **true** (WRONG). The firmware returns **false** because `\b` sees the ORIGINAL
`"aword"` at abspos 1. This test pins REFINEMENT A (the `start` offset param +
original-string view).

### 4.6 Empty-string `\b`/`\B` special case (neither passes on empty input)
| pattern | input | start | cs | full | expected | why |
|---------|-------|-------|----|------|----------|-----|
| `"\\b"` | `b""` | 0 | T | F | **false** | empty original → \b short-circuits, no recurse → no Match |
| `"\\B"` | `b""` | 0 | T | F | **false** | empty original → \B ALSO fails (legacy semantics) |
| `"\\b\\b"` | `b"a"` | 0 | T | F | **true**  | two boundaries at start-of-word both pass; seed closure reaches Match via empty core... (see note) |

> Note on `"\\b\\b"`: the firmware `test_word_boundary_basic.c` asserts
> `{"\\b\\b", "a", true, true}` end-to-end. At the nfa_match level with
> `full_match=false`: pattern `\b\b` compiles to `[Assert, Assert, Match]` (two
> zero-width asserts then Match). Seed closure at abspos 0: both `\b` pass
> (boundary at 'a'), reach Match → `!full_match && has_match` → **true**. ✓

---

## 5. Gotchas summary (the failure magnets)

- **GOTCHA-1 (no defensive NULL guard):** the firmware `if (!start) return ...` is
  DEAD in Rust — `nfa_compile` (S1) always returns a Vec with ≥1 element
  (`[Match]` for empty input), so `states[0]` always exists. Do NOT add a
  None/empty guard that returns spurious `true`; instead `debug_assert!(!states.is_empty())`.
- **GOTCHA-2 (`\b`/`\B` see the ORIGINAL string at an ABSOLUTE offset):** the
  single most common porting bug. `nfa_addstate`'s Assert branch MUST call
  `is_word_boundary(string_start, abspos)` where `string_start` is the FULL
  original string and `abspos` is absolute from its start — NOT a per-offset
  slice. The `\bword`/`aword`@1 test pins this. REFINEMENT A (add `start`) exists
  for this reason. (PRD §13 #10.)
- **GOTCHA-3 (epsilon edges do NOT advance abspos):** Split and Assert forward
  `abspos` UNCHANGED to their successors. Only the simulator's per-char step
  advances it (`pos + 1` when feeding a consumed Char/Any's `out`). Forgetting
  this makes `\b` evaluate at the wrong position. (PRD §13 #10.)
- **GOTCHA-4 (mark seen BEFORE dispatch):** set `seen[idx] = generation` BEFORE
  recursing into Split/Assert edges, so a state reached via one Split branch is
  not re-added when the other branch converges on it. The firmware sets
  `lastlist = nfa_gen` first for exactly this reason.
- **GOTCHA-5 (Assert/Split are NEVER collected):** `nfa_addstate` only pushes
  Char/Any/Match onto the list. Assert/Split RETURN without pushing (they only
  conditionally recurse). The simulator's inner loop therefore only ever sees
  Any/Char on the live list (the `_ => {}` arm is correct, not a bug).
- **GOTCHA-6 (empty-string `\b`/`\B` special case):** if the ORIGINAL string is
  empty, NEITHER `\b` NOR `\B` passes (do not recurse). The firmware checks
  `*string_start != '\0'` BEFORE `is_word_boundary`; mirror with
  `!string_start.is_empty()`. This is INDEPENDENT of the is_word_boundary impl
  (which would otherwise return false for pos 0 / empty → making `\B` wrongly
  pass). Pinned by `\b`/""→false AND `\B`/""→false.
- **GOTCHA-7 (dot excludes newline AND CR):** `DOT_META` (0x0D) matches
  `sc != '\n' && sc != '\r'` — BOTH newline and carriage return. The glob `*`
  (OP_ANY) matches ANY byte INCLUDING `\n`/`\r`. Don't conflate the two.
  (PRD §13 #8.)
- **GOTCHA-8 (ASCII whitespace, not Unicode):** use `u8::is_ascii_whitespace()`
  (exactly ` \t\n\r\f\v`), NOT `char::is_whitespace()` (which adds Unicode
  whitespace and would diverge on non-ASCII bytes). Same care for digit/word:
  `is_ascii_digit` / `is_ascii_alphanumeric` (+ `_` for word).
- **GOTCHA-9 (ASCII case-fold, decode-then-fold):** case-insensitive compare uses
  `to_ascii_lowercase()` on BOTH bytes (== C `tolower((unsigned char))` in the C
  locale; identity for non-letters / non-ASCII). For escaped literals (0x01-0x04),
  DECODE to the literal byte FIRST, then fold — never fold the placeholder byte.
- **GOTCHA-10 (generation starts at 1; seen init to 0):** `seen` is
  `vec![0u32; states.len()]`; the first `generation += 1` makes it 1, so 0 means
  "unseen". Never start generation at 0 or the initial 0s would read as "already
  seen" and the seed closure would be empty.
- **GOTCHA-11 (bump generation ONCE per phase, never clear seen):** increment
  `generation` exactly once before building each closure (seed + each consumed
  char). Do NOT `seen.fill(false)` per phase (that abandons O(1)-dedup; the whole
  point of the generation tag). The new generation value makes every old tag stale.
- **GOTCHA-12 (swap, don't move):** use `std::mem::swap(&mut clist, &mut nlist)`
  after each step (a 3-pointer swap, no realloc), then `nlist.clear()` at the top
  of the next step. Mirrors the firmware pointer swap. Do NOT drain/clone.
- **GOTCHA-13 (full_match early-return placement):** the `!full_match &&
  nfa_has_match` early-return appears (a) once after the SEED closure (empty
  prefix), and (b) once after each step's swap (mid-stream prefix). The FINAL
  `nfa_has_match` return handles full_match (accept only at end) AND the
  substring case that matches exactly at end-of-string. Do NOT early-return for
  full_match=true.
- **GOTCHA-14 (dead-list break):** if `clist.is_empty()` after a swap, `break`
  (no live states can ever recover). The final `nfa_has_match(&[])` → false.
- **GOTCHA-15 (recursion depth):** `nfa_addstate` is recursive on epsilon edges.
  Depth is bounded by the longest epsilon chain in the compiled NFA, which is
  `<< states.len()` (Split/Assert chains are short). No overflow risk for any
  realistic host pattern. An explicit-stack iterative version is possible but
  unnecessary; mirror the firmware's recursive form.

---

## 6. External reference (cite in rustdoc — Mode A)

- Russ Cox, *"Regular Expression Matching Can Be Simple And Fast"*
  (<https://swtch.com/~rsc/regexp/regexp1.html>) — the Thompson-simulation
  reference the firmware rustdoc cites (PRD §7.5/§7.9). For THIS task's rustdoc:
  the two-list simulation (clist/nlist, swap per step) is O(states × strlen) with
  no backtracking; the generation-tag dedup makes each phase O(states) with no
  allocation. Cite §"NFA-based Regular Expression Algorithms".
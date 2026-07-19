# Research Notes — P2.M1.T1.S2: Port `parse_pattern()` (anchor detection + core extraction)

> Companion to `../PRP.md`. Holds the verbatim firmware C analysis, the
> even-backslash-count derivation, the derived **Rust-native parity table**
> for `parse_pattern` (direct `ParsedPattern` assertions — stronger than the
> firmware's end-to-end tests, because the C `parse_pattern` is `static` and
> only reachable through `pattern_match`), and scope boundaries vs. the sibling
> subtasks S1 / T2 / T3.

---

## 1. The canonical firmware source (single source of truth, PRD §14)

File: `/home/dustin/projects/qmk-notifier/pattern_match.c`, `parse_pattern()` at
~lines 100–175 (immediately after `free_parsed_pattern`). Reproduced verbatim:

```c
typedef struct {
    const char *core_pattern;      /* points into processed_pattern, or raw pattern on malloc failure */
    bool        start_anchored;    /* true if the original pattern began with '^' */
    bool        end_anchored;      /* true if the original pattern ended with an unescaped '$' */
    char       *processed_pattern; /* malloc'd by process_escapes(); freed by free_parsed_pattern() */
} parsed_pattern_t;

static parsed_pattern_t parse_pattern(const char *pattern) {
    parsed_pattern_t parsed = {0};          /* all flags false, all pointers NULL */
    if (!pattern) return parsed;

    const char *start = pattern;
    const char *end   = pattern + strlen(pattern);   /* <-- strlen: stops at first NUL */

    /* Start anchor: a leading '^' is always a start anchor. */
    if (*start == '^') {
        parsed.start_anchored = true;
        start++;                                /* skip the '^' */
    }

    /* End anchor: trailing '$' that is NOT escaped (even backslash count). */
    if (end > start && *(end - 1) == '$') {
        int backslash_count = 0;
        const char *check = end - 2;
        while (check >= start && *check == '\\') {
            backslash_count++;
            check--;
        }
        if (backslash_count % 2 == 0) {        /* even (0,2,4,...) => unescaped '$' */
            parsed.end_anchored = true;
            end--;                               /* drop the '$' */
        }
    }

    /* Carve the core (between anchors) and process its escapes. */
    size_t core_len = (size_t)(end - start);
    char *core_pattern = malloc(core_len + 1);
    if (!core_pattern) {                       /* malloc failure: fall back to raw pattern */
        parsed.core_pattern      = pattern;
        parsed.processed_pattern = NULL;
        return parsed;
    }
    strncpy(core_pattern, start, core_len);
    core_pattern[core_len] = '\0';

    parsed.processed_pattern = process_escapes(core_pattern);
    free(core_pattern);

    if (parsed.processed_pattern)
        parsed.core_pattern = parsed.processed_pattern;   /* normal path */
    else
        parsed.core_pattern = pattern;                     /* process_escapes malloc failure */

    return parsed;
}
```

### 1.1 The C→Rust struct mapping

| C field                  | Rust field            | Notes |
|--------------------------|-----------------------|-------|
| `core_pattern` (const char*, points at the bytes the NFA consumes) | `core: Vec<u8>` | In Rust the owned buffer **is** the content — no separate pointer. Holds the `process_escapes()` output. |
| `start_anchored: bool`   | `start_anchored: bool` | identical |
| `end_anchored: bool`     | `end_anchored: bool`   | identical |
| `processed_pattern` (malloc'd buf to free) | **dropped** | Rust `Vec<u8>` owns its heap; no manual free. |
| `if (!pattern) return {0}` (NULL guard) | N/A | `&str` is never NULL. |
| malloc-failure fallback (`core_pattern = pattern`) | **dropped** | Rust `Vec` alloc failure aborts (OOM), never returns a degraded core. Host-side, GBs of RAM — irrelevant for rules.toml patterns. |

So the Rust struct is exactly **3 fields**, per the item spec:
```rust
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParsedPattern {
    pub(crate) core: Vec<u8>,       // process_escapes() output — the bytes the NFA compiles
    pub(crate) start_anchored: bool,
    pub(crate) end_anchored: bool,
}
```

---

## 2. The even-backslash-count rule (GOTCHA-1, the critical parity case)

A trailing `$` is a **real end anchor** ONLY when an **EVEN** number of
backslashes (0, 2, 4, …) immediately precede it. An **ODD** count means the `$`
is escaped (`\$`) and stays in the core for `process_escapes` to turn into the
`0x02` literal. This is the standard "is the final metacharacter quoted?" test:
walk left from the `$` counting **consecutive** `\`; even ⇒ unquoted.

The C comment in `pattern_match.c` states it with these four canonical cases
(translated here to **Rust source literals** + the resulting `ParsedPattern`):

| Rust pattern literal | Bytes before `$` | backslashes | anchored? | core fed to `process_escapes` | `core` output | `end_anchored` |
|---|---|---|---|---|---|---|
| `"abc$"`        | `a b c`         | 0 (even) | ✅ anchor | `"abc"`            | `[61 62 63]`             | **true**  |
| `"abc\\$"`      | `a b c \`       | 1 (odd)  | ❌ escaped | `"abc\$"`          | `[61 62 63 02]` (ESC_DOLLAR) | false |
| `"abc\\\\$"`    | `a b c \ \`     | 2 (even) | ✅ anchor | `"abc\\"`          | `[61 62 63 04]` (ESC_BSLASH) | **true**  |
| `"abc\\\\\\$"`  | `a b c \ \ \`   | 3 (odd)  | ❌ escaped | `"abc\\\$"`        | `[61 62 63 04 02]`       | false |

> Remember: in Rust source, `"\\"` is ONE backslash byte, `"\\\\"` is TWO,
> `"\\\\\\$"` is THREE backslashes + `$`. (C uses the same `\\`-doubling, so the
> firmware test corpus literals map 1:1 to Rust literals.)

**Key subtlety:** the backslash walk counts RAW consecutive backslashes — it
does **not** pair them up as escapes. Even count ⇒ unescaped `$`. This is what
makes the 2-backslash case an anchor (the `\\` pair is a literal backslash in
the core, and the `$` after it is free) and the 3-backslash case NOT an anchor
(`\\` pair + `\$`, so the `$` is consumed by the last backslash).

### 2.1 Firmware corpus anchor/backslash cases (from the test files)

These end-to-end results (pattern, string, expected match) all flow THROUGH
`parse_pattern` — each pins one row of its behavior. The Rust port will assert
the `ParsedPattern` directly (stronger, see §3).

```
# test_invalid_patterns.c / test_error_handling.c (selected, paraphrased)
test_pattern("^^",  "^",   true)   # 1st ^ anchor, 2nd ^ literal  -> start_anchored=true, core=[5E]
test_pattern("$$",  "$",   true)   # 1st $ literal, 2nd $ anchor  -> end_anchored=true, core=[24]
test_pattern("^",   "",    true)   # lone start anchor             -> start=true, core=[]
test_pattern("$",   "",    true)   # lone end anchor               -> end=true, core=[]
test_pattern("^$",  "",    true)   # both anchors, empty core      -> start=end=true, core=[]
test_pattern("^$",  "a",   false)  # empty core rejects non-empty
test_pattern("^a$", "a",   true)   # exact                        -> core=[61]
test_pattern("^a$", "aa",  false)
test_pattern("\\^$", "^",  true)   # escaped ^ + real end anchor   -> end=true, core=[01]
test_pattern("^\\$", "$",  true)   # real start + escaped end      -> start=true, core=[02]
test_pattern("mid$dle$", "mid$dle", true)  # interior $ literal, trailing $ anchor
test_pattern("\\^*\\$", "^anything$", true)  # no anchors; \^ ... \$
```

The `\\\\\\$`-style cases appear in `test_error_handling.c:141` and
`test_invalid_patterns.c:148`/`:212` — these are the 3-backslash (odd) ⇒ escaped
cases that MUST keep `$` in the core.

---

## 3. Rust-native parity table for `parse_pattern` (THE test contract)

These assert the **`ParsedPattern`** directly — `core` is the `process_escapes`
output of the carved core, `start_anchored`/`end_anchored` are the detected
flags. Copy these into `#[test]` fns. Use the S1 `pub(crate) const` names for
readability (`ESC_DOLLAR=0x02`, `ESC_BSLASH=0x04`, `ESC_CARET=0x01`, …).

### 3.1 Anchor detection (no escape interaction)

| # | input          | start | end | core (Vec<u8>)                                                    |
|---|----------------|-------|-----|-------------------------------------------------------------------|
| 1 | `""`           | false | false | `[]` |
| 2 | `"hello"`      | false | false | `[0x68,0x65,0x6C,0x6C,0x6F]` |
| 3 | `"^hello"`     | true  | false | `[0x68,0x65,0x6C,0x6C,0x6F]` |
| 4 | `"hello$"`     | false | true  | `[0x68,0x65,0x6C,0x6C,0x6F]` |
| 5 | `"^hello$"`    | true  | true  | `[0x68,0x65,0x6C,0x6C,0x6F]` |
| 6 | `"^"`          | true  | false | `[]` |
| 7 | `"$"`          | false | true  | `[]` |
| 8 | `"^$"`         | true  | true  | `[]`   (empty core — matches empty string only) |
| 9 | `"^^"`         | true  | false | `[0x5E]`   (2nd ^ is a bare literal — process_escapes passes ^ as 0x5E) |
| 10| `"$$"`         | false | true  | `[0x24]`   (1st $ stays in core as bare 0x24; 2nd $ is the anchor) |
| 11| `"^a$"`        | true  | true  | `[0x61]` |
| 12| `"^*$"`        | true  | true  | `[GLOB_STAR]` (= `[0x2A]`) |

### 3.2 The even-backslash-count rule (GOTCHA-1 — the critical cases)

| # | input (Rust literal)   | backslashes before `$` | start | end | core (Vec<u8>) |
|---|------------------------|------------------------|-------|-----|----------------|
| 13| `"abc$"`               | 0 (even) → ANCHOR      | false | **true**  | `[0x61,0x62,0x63]` |
| 14| `"abc\\$"`             | 1 (odd) → escaped      | false | false | `[0x61,0x62,0x63, ESC_DOLLAR]` |
| 15| `"abc\\\\$"`           | 2 (even) → ANCHOR      | false | **true**  | `[0x61,0x62,0x63, ESC_BSLASH]` |
| 16| `"abc\\\\\\$"`         | 3 (odd) → escaped      | false | false | `[0x61,0x62,0x63, ESC_BSLASH, ESC_DOLLAR]` |
| 17| `"\\^$"`               | 0 before $ (the `\^` is before it) → ANCHOR | false | **true** | `[ESC_CARET]` |
| 18| `"^\\$"`               | 1 before $ → escaped   | **true** | false | `[ESC_DOLLAR]` |
| 19| `"\\^*\\$"`            | 1 before final $ → escaped (no anchors) | false | false | `[ESC_CARET, GLOB_STAR, ESC_DOLLAR]` |
| 20| `"mid$dle$"`           | 0 before final $ (preceded by `e`) → ANCHOR | false | **true** | `[0x6D,0x69,0x64, 0x24, 0x64,0x6C,0x65]` (interior $ is bare 0x24) |

### 3.3 Anchor + escape/class interaction (core still runs through process_escapes)

| # | input       | start | end | core |
|---|-------------|-------|-----|------|
| 21| `"^\\d$"`   | true  | true  | `[CLASS_DIGIT]` (`[0x05]`) |
| 22| `"^\\w$"`   | true  | true  | `[CLASS_WORD]` (`[0x07]`) |
| 23| `"^.$"`     | true  | true  | `[DOT_META]` (`[0x0D]`) |
| 24| `"^\\.$"`   | true  | true  | `[0x2E]` (escaped dot is literal, GOTCHA-A from S1) |
| 25| `"^a+$"`    | true  | true  | `[0x61, PLUS_QUANT]` (`[0x61,0x0E]`) |
| 26| `"*^test"`  | false | false | `[GLOB_STAR, 0x5E, 0x74,0x65,0x73,0x74]` (no anchors; bare ^ is 0x5E) |

### 3.4 The NUL-stop parity case (GOTCHA-2 — defensive, mirrors firmware `strlen`)

The firmware computes `end = pattern + strlen(pattern)`, so an interior `0x00`
byte truncates the effective length BEFORE anchor detection. A Rust `&str` *can*
hold a `0x00` (valid UTF-8). For byte-for-byte parity, `parse_pattern` must
compute the effective length the same way — otherwise the **anchor flags** (not
just the core) can diverge on a NUL-containing input.

| # | input (built via `std::str::from_utf8`) | start | end | core |
|---|------------------------------------------|-------|-----|------|
| 27| `b"ab\0cd$"` → `from_utf8(..)`           | false | false | `[0x61,0x62]` |

Reasoning: `effective_len = 2` (NUL at index 2). Trailing-`$` check sees
`bytes[1] == 'b'`, not `$` ⇒ no end anchor. Core = `bytes[0..2]` = `"ab"`.
`process_escapes("ab")` = `[0x61,0x62]`. **Without** the NUL handling the code
would see `$` at index 5, set `end_anchored = true` (WRONG), and pass
`"ab\0cd"` to `process_escapes` (which would itself stop at the NUL and return
`[0x61,0x62]` — same core, but a wrong anchor flag).

> Real `rules.toml` patterns never contain NUL; this case exists purely to
> guarantee firmware parity for ALL inputs and to keep the port honest. It is a
> 1-line computation (`bytes.iter().position(|&b| b == 0).unwrap_or(len)`).

---

## 4. The byte-walk in Rust (reference skeleton, verified against C)

```rust
pub(crate) fn parse_pattern(pattern: &str) -> ParsedPattern {
    let bytes = pattern.as_bytes();
    // GOTCHA-2: mirror firmware strlen — stop at first NUL.
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());

    let mut start = 0usize;
    let mut end   = len;
    let mut start_anchored = false;
    let mut end_anchored   = false;

    // Start anchor: a leading '^' (only at the very front).
    if end > start && bytes[start] == b'^' {
        start_anchored = true;
        start += 1;
    }

    // End anchor: trailing '$' preceded by an EVEN number of backslashes.
    if end > start && bytes[end - 1] == b'$' {
        let mut bs = 0usize;
        let mut k = end - 1;                 // index of the '$'
        while k > start {
            k -= 1;                           // move to the byte left of '$'
            if bytes[k] == b'\\' { bs += 1; } else { break; }
        }
        if bs % 2 == 0 {                       // even => unescaped '$'
            end_anchored = true;
            end -= 1;                          // drop the '$'
        }
    }

    // Carve the core (indices are ASCII boundaries: ^ $ \ are all < 0x80, and
    // we only ever trim/advance at those positions, so the &str slice is valid).
    let core_str = &pattern[start..end];
    let core = process_escapes(core_str);

    ParsedPattern { core, start_anchored, end_anchored }
}
```

Why the `&str` slice `[start..end]` is safe: `b'^'` (0x5E), `b'$'` (0x24), and
`b'\\'` (0x5C) are all ASCII (< 0x80). UTF-8 char boundaries coincide with ASCII
byte positions (every continuation/lead byte of a multi-byte sequence is
≥ 0x80), so trimming at an ASCII index never splits a UTF-8 sequence. The NUL
byte (0x00) is likewise a char boundary. ⇒ `&pattern[start..end]` cannot panic.

`process_escapes` then runs over the carved core and produces the placeholder
bytes; the NUL-stop inside `process_escapes` (S1) is now redundant for the core
(the core has no NUL — `len` already excluded it) but harmless.

---

## 5. S1 contract that this task CONSUMES (already in `src/core/pattern.rs`)

Confirmed by reading the current file (S1 is implemented):

- `pub(crate) const ESC_CARET/DOLLAR/STAR/BSLASH` (0x01–0x04), `CLASS_*`
  (0x05–0x0A), `ASSERT_BOUND/NBOUND` (0x0B/0x0C), `DOT_META` (0x0D),
  `PLUS_QUANT` (0x0E), `GLOB_STAR` (0x2A) — all `#[allow(dead_code)] pub(crate)`.
- `pub(crate) fn process_escapes(pattern: &str) -> Vec<u8>` (line ~85).
- `mod tests { use super::*; ... }` (line ~206) — grouped with `// --- header ---`
  comments, asserts use the named consts (e.g. `vec![ESC_CARET]`). **Append the
  new `parse_pattern` tests into this same module**, following the same style.
- The file carries `#![allow(dead_code)]` at top because the API ships ahead of
  its first non-test consumer. `parse_pattern` IS that first consumer of
  `process_escapes` — once it exists, the `#[allow(dead_code)]` on
  `process_escapes` becomes unnecessary (optional cleanup; leaving it is fine).

This task changes **only** `src/core/pattern.rs` (adds the struct + fn + tests).
`src/core/mod.rs` already has `pub mod pattern;` (S1) — **do not touch it.**

---

## 6. Scope boundaries (what NOT to do)

- **Do NOT** implement the NFA (`nfa_compile`, `nfa_addstate`, `nfa_match`) —
  that is P2.M1.T2.
- **Do NOT** implement `match_with_anchors` / the public `pattern_match` entry
  / the delimiter-aware `match_pattern` / `Pattern::Single|Parts` — that is
  P2.M1.T3.
- **Do NOT** re-implement `process_escapes` or redeclare the placeholder
  constants — they exist (S1). Just CALL `process_escapes(core_str)`.
- **Do NOT** port the C malloc-failure fallback (`core_pattern = pattern`) —
  Rust `Vec` cannot fail-soft; the host never OOMs on a rules.toml pattern.
- **Do NOT** add a `free_parsed_pattern` analog — Rust owns the `Vec<u8>`; no
  manual cleanup. Downstream (T2/T3) takes `&ParsedPattern` or moves it.
- **Do NOT** change `src/core/mod.rs` — module already registered.
- **Do NOT** edit `PRD.md`, any `tasks.json`, `prd_snapshot.md`, or other
  `plan/` files.

## 7. Confidence notes

- The algorithm is a faithful 1:1 transcription of a ~30-line, heavily-commented
  C function whose entire body is reproduced in §1.
- The even-backslash-count rule (the only non-obvious part) is pinned by 8
  derived parity vectors (rows 13–20) plus the firmware's own corpus.
- The struct is exactly 3 fields with a clear C→Rust mapping (§1.1).
- No new deps, no `unsafe`, no architectural decisions left open.
- The only subtlety beyond the item spec is the NUL-stop parity (GOTCHA-2),
  included for firmware-exactness; it is optional-but-recommended and 1 line.
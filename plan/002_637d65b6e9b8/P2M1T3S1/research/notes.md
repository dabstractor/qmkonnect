# Research Notes — P2.M1.T3.S1: Port `match_with_anchors()` + public `pattern_match()`

> The **anchor-strategy layer** + the **public entry** of the Rust pattern matcher.
> Stage 5 of the P2.M1 pipeline (`parse_pattern` → `process_escapes` →
> `nfa_compile` → `nfa_match` → **`match_with_anchors`** → **`pattern_match`**).
> Consumes `nfa_match` (P2.M1.T2.S2, implementing in parallel — treated as a
> contract) + `ParsedPattern`/`parse_pattern` (P2.M1.T1.S2, complete in-tree).
> Firmware `pattern_match.c` lines ~225–272 are the single source of truth
> (PRD §4, §14).

---

## 1. The firmware C source (verbatim — the spec)

### 1a. `match_with_anchors` (pattern_match.c:225–256)

```c
/* ===== P1.M3.T2.S2: match_with_anchors — the anchor strategy (PRD §7.4) =====
 * ... ^...$ exact -> one full match from offset 0
 *     ^...       prefix -> one reach-any match from offset 0
 *     ...$       suffix -> loop offsets, full match from each
 *     ...        substring -> loop offsets, reach-any from each (empty core only
 *                 matches the empty string). */
static bool match_with_anchors(const parsed_pattern_t *parsed, const char *str, bool case_sensitive) {
    if (!parsed || !str) return false;
    const char *core_pattern = parsed->core_pattern;

    if (parsed->start_anchored && parsed->end_anchored) {        /* ^...$ exact */
        return match_reaches_end_with_start(core_pattern, str, str, case_sensitive);
    } else if (parsed->start_anchored) {                         /* ^ prefix */
        return match_string_with_start(core_pattern, str, str, case_sensitive);
    } else if (parsed->end_anchored) {                           /* $ suffix */
        size_t str_len = strlen(str);
        for (size_t i = 0; i <= str_len; i++)
            if (match_reaches_end_with_start(core_pattern, str + i, str, case_sensitive))
                return true;
        return false;
    } else {                                                     /* substring (default) */
        if (strlen(core_pattern) == 0) return strlen(str) == 0;  /* empty core -> only empty string */
        size_t str_len = strlen(str);
        for (size_t i = 0; i <= str_len; i++)
            if (match_string_with_start(core_pattern, str + i, str, case_sensitive))
                return true;
        return false;
    }
}
```

### 1b. The public entry `pattern_match` (pattern_match.c:259–272)

```c
/* ===== PUBLIC API (declared in pattern_match.h) =====
 * NULL-guard -> parse -> match -> free. Caller frees nothing (PRD §6). */
bool pattern_match(const char *pattern, const char *str, bool case_sensitive) {
    if (!pattern || !str) {
        return false;
    }

    parsed_pattern_t parsed = parse_pattern(pattern);

    bool result = match_with_anchors(&parsed, str, case_sensitive);

    free_parsed_pattern(&parsed);

    return result;
}
```

### 1c. The two wrapper forwarders `match_with_anchors` calls (pattern_match.c:617–628)

```c
/* ===== P1.M3.T2.S2: anchor-strategy wrappers (thin forwarders to nfa_match) =====
 * match_string_with_start     -> reach-any (substring/prefix; full_match=false).
 * match_reaches_end_with_start -> consume-whole-remaining (suffix/exact; full_match=true).
 * Both forward the ORIGINAL string_start so \b/\B compute absolute positions. */
static bool match_string_with_start(const char *pattern, const char *str,
        const char *string_start, bool case_sensitive) {
    return nfa_match(pattern, str, string_start, case_sensitive, false);
}
static bool match_reaches_end_with_start(const char *pattern, const char *str,
        const char *string_start, bool case_sensitive) {
    return nfa_match(pattern, str, string_start, case_sensitive, true);
}
```

**Observation:** the wrappers exist ONLY because the firmware `nfa_match` is
`static` + forward-declared and takes a RAW `pattern` (recompiling internally
each call). They are pure forwarders: `match_string_with_start` ⇔ `full_match=false`,
`match_reaches_end_with_start` ⇔ `full_match=true`. **In Rust they COLLAPSE into
direct `nfa_match` calls** (REFINEMENT D below).

---

## 2. The four anchor modes → `nfa_match` call mapping (the core of this task)

The T2.S2 PRP (REFINEMENT A+B) fixes the Rust `nfa_match` signature as:

```rust
pub(crate) fn nfa_match(states: &[NfaOp], string: &[u8], start: usize,
                        case_sensitive: bool, full_match: bool) -> bool;
```

So each firmware branch maps to ONE idiomatic Rust `nfa_match` call, after
**compiling the core ONCE** (REFINEMENT E — the firmware recompiles inside
`nfa_match` per call/offset; Rust compiles once via `nfa_compile(&parsed.core)`
then loops offsets, mirroring S1's compile-once-simulate-many design):

| Firmware branch                          | Rust `nfa_match` call                                  | full_match |
| ---------------------------------------- | ------------------------------------------------------ | ---------- |
| `^...$` exact (start+end)                | `nfa_match(&nfa, bytes, 0, cs, true)`                  | **true**   |
| `^...` prefix (start only)               | `nfa_match(&nfa, bytes, 0, cs, false)`                 | **false**  |
| `...$` suffix (end only)                 | loop offsets: `nfa_match(&nfa, bytes, i, cs, true)`    | **true**   |
| `...` substring (neither)                | loop offsets: `nfa_match(&nfa, bytes, i, cs, false)`   | **false**  |

Where `bytes = s.as_bytes()`, `cs = case_sensitive`, and `nfa = nfa_compile(&parsed.core)`.

**full_match semantics (recap from T2.S2):**
- `false` ⇒ `Match` reachable at ANY point (prefix/substring) → early-return true.
- `true` ⇒ `Match` reachable only after consuming the WHOLE remaining string
  (exact/suffix) → accept only at end.

---

## 3. The two REFINEMENTS this task makes (both forced, fully derived)

### REFINEMENT D — fold the two C wrappers into direct `nfa_match` calls

The firmware needs `match_string_with_start` / `match_reaches_end_with_start`
because (a) `nfa_match` is `static`/forward-declared and (b) it takes a raw
pattern + recompiles internally, so the wrapper exists to pin `full_match`.
In Rust, `nfa_match` is `pub(crate)` and takes the compiled `&[NfaOp]` + a
`full_match` bool (T2.S2 REFINEMENT B) — so the wrapper is a zero-information
forwarder. **Resolution:** `match_with_anchors` calls `nfa_match` directly with
`full_match=true` (exact/suffix) or `false` (prefix/substring). Do NOT port the
two wrapper fns — they would be dead one-line aliases. (Analogous to T1.S2
dropping the C `free_parsed_pattern` because Rust owns the `Vec`.)

### REFINEMENT E — compile the core ONCE, simulate at many offsets

The firmware `nfa_match` declares `State pool[NFA_MAX_STATES]` on its stack and
calls `nfa_compile(pattern, pool, &nstates)` **every invocation** — so
`match_with_anchors`' suffix/substring loops recompile the SAME pattern `len+1`
times (wasteful but invisible in C since the pool is stack-local). In Rust,
`nfa_compile(&[u8]) -> Vec<NfaOp>` heap-allocates; recompiling per offset is
both wasteful and non-idiomatic. **Resolution:** `match_with_anchors` calls
`nfa_compile(&parsed.core)` ONCE per `match_with_anchors` call, binds it to a
local `nfa: Vec<NfaOp>`, and passes `&nfa` to every `nfa_match` in the loops.
This honors T2.S2 REFINEMENT B's contract (compile-once-simulate-many) and the
item spec ("calls nfa_match with full_match=true/false... For substring mode:
loop ... and call nfa_match from each offset"). The semantics are IDENTICAL
(nfa_compile is a pure function of the core bytes) — only the call count drops.

---

## 4. The empty-core substring special case (the one parity subtlety)

Firmware `match_with_anchors`, substring branch:
```c
if (strlen(core_pattern) == 0) return strlen(str) == 0;  /* empty core -> only empty string */
```

**Why it exists:** with NO anchors and an empty core, the "default" substring
behavior (loop offsets, reach-any) would match at offset 0 of ANY string (an
empty NFA `[Match]` reaches `Match` immediately with `full_match=false`), making
`pattern_match("", "anything")` wrongly return **true**. The firmware
deliberately short-circuits: empty pattern (no anchors) matches ONLY the empty
string. Pinned by corpus vectors:
- `{"", "", true, true}` — empty/empty → true
- `{"", "test", true, false}` — empty/non-empty → **false** (THE special case)

**Rust port:** `if parsed.core.is_empty() { return s.is_empty(); }` — checked
BEFORE compiling/looping in the substring branch ONLY. (`parsed.core` can never
contain a 0x00: parse_pattern truncates at NUL before processing, and
process_escapes never emits 0x00 — so `.is_empty()` == firmware `strlen==0`.)

**The other three modes need NO empty special case** (verified by tracing the
empty `[Match]` NFA through `nfa_match`):
- `^...$` exact, empty core, `full_match=true`: empty str → loop doesn't run →
  `nfa_has_match` true; non-empty → loop can't consume (only `Match` on list) →
  clist empties → false. So `^$` matches empty only. ✓ (matches `{"^$","","",true}`)
- `^...` prefix, empty core, `full_match=false`: seed closure has `Match` →
  early-return true for ANY str. So `^` matches everything. ✓ (`{"^","","",true}`)
- `...$` suffix, empty core, loops offsets `full_match=true`: at `i==len`,
  nothing left to consume → `Match` reachable → true for ANY str. So `$`
  matches everything. ✓ (`{"$","","",true}`)

So ONLY substring mode gets the empty-core guard — exactly as the firmware does.

---

## 5. REFINEMENT F — iterate UTF-8 char boundaries, not raw byte offsets

**The item spec explicitly says:** *"For substring mode: loop `str.char_indices()`
and call nfa_match from each offset."* The firmware loops `for (i=0; i<=str_len; i++)`
over RAW BYTE offsets (it's a byte-oriented C string). These diverge only for
non-ASCII (UTF-8 multibyte) haystacks:

- **ASCII haystack (the entire realistic domain — window titles/class names/patterns):**
  `char_indices()` yields `0,1,2,…,len-1` — byte-identical to the firmware's
  `0..=str_len` MINUS the terminal offset `len`. So we must ALSO probe `i==len`
  (the empty suffix position) to be firmware-faithful for suffix `$` and
  tail-empty-match patterns (e.g. pure `*` reaching `Match` at the very end).
- **Non-ASCII haystack:** `char_indices()` skips continuation bytes (0x80–0xBF),
  which the byte-oriented ASCII NFA (`\d`/`\w`/letters) essentially never matches
  anyway. So skipping them only drops positions where a match is impossible for
  ASCII-oriented patterns — a safe, UTF-8-correct refinement. (A glob `*` could
  theoretically consume a continuation byte mid-codepoint, but the corpus + the
  ASCII domain never exercise this; char-boundary iteration is the idiomatic,
  correct-by-default Rust choice and is what the item spec requests.)

**Resolution:** iterate char-boundary offsets PLUS the terminal byte-length
offset, so the inclusive-end semantics (firmware `0..=str_len`) are preserved:

```rust
let bytes = s.as_bytes();
// firmware loops 0..=len inclusive; iterate UTF-8 char boundaries (== byte
// offsets for ASCII) and append the terminal offset == bytes.len() so suffix/
// tail-empty cases are covered. (REFINEMENT F; item spec requests char_indices.)
for i in s.char_indices().map(|(i, _)| i).chain(std::iter::once(bytes.len())) {
    if nfa_match(&nfa, bytes, i, case_sensitive, full_match) {
        return true;
    }
}
false
```

**Parity guarantee:** for every ASCII input (all 380 corpus vectors),
`char_indices().map(..).chain(once(len))` == `0..=len` exactly. No parity break.
The non-ASCII divergence is a deliberate, documented UTF-8 refinement.

> **Acceptable alternative** (also 100% firmware-faithful for ASCII, simpler):
> iterate raw byte offsets `0..=bytes.len()` directly. Identical results on the
> corpus; less idiomatic for a Rust `&str` API. The PRP specifies `char_indices`
> per the item spec; byte-offset is a valid fallback if the implementer prefers.

---

## 6. Verified Rust skeleton (mirrors the Implementation Blueprint)

```rust
/// Pick the NFA mode + offset strategy from the parsed anchor flags and run the
/// compiled core against `s`. Faithful port of firmware match_with_anchors
/// (pattern_match.c:233-256); the two C wrappers collapse into direct nfa_match
/// calls (REFINEMENT D) and the core is compiled ONCE (REFINEMENT E).
pub(crate) fn match_with_anchors(parsed: &ParsedPattern, s: &str, case_sensitive: bool) -> bool {
    // (firmware `if (!parsed || !str) return false` is dead in Rust — &str/&ParsedPattern
    //  are never null; dropped like the other NULL guards.)
    let bytes = s.as_bytes();

    if parsed.start_anchored && parsed.end_anchored {
        // ^...$ exact: one FULL match (consume whole string) from offset 0.
        let nfa = nfa_compile(&parsed.core);
        nfa_match(&nfa, bytes, 0, case_sensitive, true)
    } else if parsed.start_anchored {
        // ^ prefix: one reach-any match from offset 0.
        let nfa = nfa_compile(&parsed.core);
        nfa_match(&nfa, bytes, 0, case_sensitive, false)
    } else if parsed.end_anchored {
        // $ suffix: loop offsets, FULL match from each.
        let nfa = nfa_compile(&parsed.core);
        suffix_or_substring_loop(&nfa, bytes, s, case_sensitive, true)
    } else {
        // substring (default): empty core -> only empty string; else loop offsets, reach-any.
        if parsed.core.is_empty() {
            return s.is_empty();   // GOTCHA: empty pattern (no anchors) matches only ""
        }
        let nfa = nfa_compile(&parsed.core);
        suffix_or_substring_loop(&nfa, bytes, s, case_sensitive, false)
    }
}

/// The offset loop shared by suffix ($) and substring (none) modes. Probes every
/// UTF-8 char boundary + the terminal offset (firmware 0..=len inclusive).
/// REFINEMENT F: char_indices keeps the matcher UTF-8-correct; chain(once(len))
/// preserves the inclusive end.
fn suffix_or_substring_loop(
    nfa: &[NfaOp],
    bytes: &[u8],
    s: &str,
    case_sensitive: bool,
    full_match: bool,
) -> bool {
    for i in s.char_indices().map(|(i, _)| i).chain(std::iter::once(bytes.len())) {
        if nfa_match(nfa, bytes, i, case_sensitive, full_match) {
            return true;
        }
    }
    false
}

/// Public pattern-matching entry — the full-parity port of firmware
/// `pattern_match` (pattern_match.c:259-272). Parse → match_with_anchors → drop.
///
/// (firmware NULL-guard + free_parsed_pattern are dead in Rust: &str is never
///  null and ParsedPattern owns its Vec; drop is automatic.)
pub fn pattern_match(pattern: &str, s: &str, case_sensitive: bool) -> bool {
    let parsed = parse_pattern(pattern);
    match_with_anchors(&parsed, s, case_sensitive)
    // `parsed` drops here automatically (no free_parsed_pattern analog).
}
```

> The `suffix_or_substring_loop` helper is OPTIONAL DRY sugar — an implementer
> may inline the loop in both branches. The PRP presents it as a helper to avoid
> duplicating the char_indices+chain pattern; either is acceptable.

---

## 7. End-to-end parity vectors (curated from the 380-row firmware corpus)

These exercise `pattern_match` (the PUBLIC entry this task ports) — the strongest
possible end-to-end parity contract. Grouped by anchor mode + the special cases.
(T3.S1 owns the anchor-strategy behavior; the full 380-row corpus is ported en
masse in P2.M1.T4.S1, but these ~40 vectors pin every `match_with_anchors` branch.)

### 7.1 Start anchor `^` (prefix, full_match=false, offset 0)
| pattern        | input           | cs    | expected | why                                   |
| -------------- | --------------- | ----- | -------- | ------------------------------------- |
| `^searchterm`  | `searchterm`    | true  | true     | exact-from-start                      |
| `^searchterm`  | `presearchterm` | true  | false    | not at beginning                      |
| `^searchterm`  | `searchtermpost`| true  | true     | prefix match (reach-any)              |
| `^test`        | `test123`       | true  | true     | trailing content OK                   |
| `^test`        | `pretest`       | true  | false    | leading content rejects               |
| `^`            | ``              | true  | true     | empty core prefix, empty str          |
| `^abc`         | `ABC`           | false | true     | case-insensitive                      |
| `^abc`         | `ABC`           | true  | false    | case-sensitive                        |
| `^*test`       | `anytest`       | true  | true     | prefix + leading glob                 |
| `^*`           | `anything`      | true  | true     | prefix + glob (matches all)           |

### 7.2 End anchor `$` (suffix, full_match=true, loop offsets)
| pattern        | input           | cs    | expected | why                                   |
| -------------- | --------------- | ----- | -------- | ------------------------------------- |
| `searchterm$`  | `searchterm`    | true  | true     | exact-to-end                          |
| `searchterm$`  | `searchtermpost`| true  | false    | not at end                            |
| `searchterm$`  | `presearchterm` | true  | true     | suffix match (leading content OK)     |
| `test$`        | `pretest`       | true  | true     | leading content OK                    |
| `test$`        | `test123`       | true  | false    | trailing content rejects              |
| `$`            | ``              | true  | true     | empty core suffix, empty str          |
| `abc$`         | `ABC`           | false | true     | case-insensitive                      |
| `abc$`         | `ABC`           | true  | false    | case-sensitive                        |
| `test*$`       | `testany`       | true  | true     | suffix + trailing glob                |
| `*$`           | `anything`      | true  | true     | suffix + glob (matches all)           |

### 7.3 Full anchor `^…$` (exact, full_match=true, offset 0)
| pattern          | input           | cs    | expected | why                                 |
| ---------------- | --------------- | ----- | -------- | ----------------------------------- |
| `^searchterm$`   | `searchterm`    | true  | true     | exact match                         |
| `^searchterm$`   | `presearchterm` | true  | false    | leading content rejects             |
| `^searchterm$`   | `searchtermpost`| true  | false    | trailing content rejects            |
| `^searchterm$`   | `presearchtermpost`|true| false    | both reject                         |
| `^test$`         | `test`          | true  | true     | simple exact                        |
| `^$`             | ``              | true  | true     | empty core exact, empty str         |
| `^$`             | `a`             | true  | false    | empty core exact vs non-empty       |
| `^abc$`          | `ABC`           | false | true     | case-insensitive exact              |
| `^abc$`          | `ABC`           | true  | false    | case-sensitive                      |
| `^sear*term$`    | `searchterm`    | true  | true     | exact + glob                        |
| `^sear*term$`    | `searedsalmonterm`|true| true     | glob expansion                     |
| `^a*b*c$`        | `aabbcc`        | true  | true     | multiple globs                      |
| `^*$`            | `anything`      | true  | true     | full glob exact (matches all)       |

### 7.4 Substring (no anchors, full_match=false, loop offsets) — + empty special case
| pattern        | input             | cs    | expected | why                                   |
| -------------- | ----------------- | ----- | -------- | ------------------------------------- |
| `searchterm`   | `presearchtermpost`| true | true     | substring match                       |
| `sear*term`    | `presearchtermpost`| true | true     | substring + glob                      |
| `*term`        | `searchterm`      | true  | true     | leading glob (suffix-like)            |
| `search*`      | `searchterm`      | true  | true     | trailing glob (prefix-like)           |
| `test`         | `test`            | true  | true     | simple match                          |
| `test`         | `testing`         | true  | true     | substring                             |
| `*`            | `anything`        | true  | true     | full glob matches anything            |
| `""` (empty)   | ``                | true  | true     | **EMPTY special case**: empty/empty   |
| `""` (empty)   | `test`            | true  | false    | **EMPTY special case**: empty/non-empty |
| `test`         | ``                | true  | false    | non-empty pattern, empty str          |
| `abc`          | `ABC`             | false | true     | case-insensitive                      |
| `abc`          | `ABC`             | true  | false    | case-sensitive                        |
| `*test*`       | `pretestpost`     | true  | true     | globs both sides                      |
| `a*`           | `a`               | true  | true     | glob min match                        |

### 7.5 Edge cases + escapes + classes (cross-mode)
| pattern        | input      | cs   | expected | why                                   |
| -------------- | ---------- | ---- | -------- | ------------------------------------- |
| `^^test`       | `^test`    | true | true     | 1st `^` anchors, 2nd `^` is literal core (any mode) |
| `test$$`       | `test$`    | true | true     | trailing `$` anchors, the `$` before it is literal   |
| `\\\\\\^`      | `\^`       | true | true     | complex escape → literal `\^` core    |
| `\\`           | `\`        | true | true     | single backslash                      |
| `^\\d`         | `5`        | true | true     | prefix + `\d`                         |
| `^\\d`         | `a5`       | true | false    | prefix + `\d` non-digit at start      |
| `\\d$`         | `5`        | true | true     | suffix + `\d`                         |
| `^\\d+$`       | `12345`    | true | true     | exact + `\d+` quantifier              |
| `^\\w+$`       | `hello_1`  | true | true     | exact + `\w+`                         |
| `\\bword`      | `a word`   | true | true     | substring `\b` (boundary before word) |
| `\\bword`      | `aword`    | true | false    | **linchpin**: `\b` sees original str (no boundary mid-word) |

(The `\b` linchpin is exercised at the `nfa_match` level in T2.S2; at the
`pattern_match` level it confirms the substring loop + original-string threading
compose correctly. `{"\\bword","aword",true,false}` is the single most important
end-to-end vector proving REFINEMENT F + the `start` offset + `\b` all compose.)

---

## 8. Gotchas (with the test rows that pin each)

- **GOTCHA-A (empty-core substring special case):** substring mode (no anchors)
  with `parsed.core.is_empty()` must return `s.is_empty()` — NOT loop offsets.
  Without this, `pattern_match("", "test")` wrongly returns true (empty NFA
  reaches Match at offset 0). Pinned by `{"","test",true,false}` and
  `{"","",true,true}`. The other three modes need NO special case (traced in §4).
- **GOTCHA-B (inclusive end offset):** the suffix/substring loops must probe
  `i == bytes.len()` (the empty tail), because char_indices stops at the last
  char START, not one-past. Without it, `$`-with-empty-ish-core and pure-`*`-at-end
  cases would miss. `chain(std::iter::once(bytes.len()))` covers it. For ASCII
  this reproduces firmware `0..=str_len` exactly.
- **GOTCHA-C (compile ONCE, not per offset):** call `nfa_compile(&parsed.core)`
  ONCE per `match_with_anchors` invocation and reuse `&nfa` across the loop.
  Recompiling per offset is a faithful-but-wasteful C pattern (stack pool); Rust
  heap-allocates, so compile-once-simulate-many is the idiom (REFINEMENT E).
- **GOTCHA-D (no NULL guard, no free):** the firmware `if (!pattern || !str)
  return false` and `free_parsed_pattern(&parsed)` are DEAD in Rust — `&str` /
  `&ParsedPattern` are never null and the `Vec` drops automatically. Drop both,
  exactly as T1.S2 dropped the C malloc-fallback + free_parsed_pattern.
- **GOTCHA-E (do NOT port the two wrapper fns):** `match_string_with_start` and
  `match_reaches_end_with_start` are zero-information forwarders in C (exist only
  because nfa_match is static + takes raw bytes). In Rust they collapse into
  `nfa_match(.., full_match=false)` / `nfa_match(.., full_match=true)` directly
  (REFINEMENT D). Porting them creates dead one-line aliases.
- **GOTCHA-F (full_match per mode):** exact=`true`, suffix=`true`, prefix=`false`,
  substring=`false`. Mixing these flips semantics (e.g. suffix with
  full_match=false would be a substring match). The table in §2 is authoritative.
- **GOTCHA-G (char_indices, not byte offsets — per item spec):** use
  `s.char_indices()` for UTF-8 correctness (item spec requests it); for ASCII
  (the whole corpus) it's byte-identical to the firmware. Add the terminal offset
  (GOTCHA-B). See REFINEMENT F.
- **GOTCHA-H (bytes, not chars, to nfa_match):** pass `s.as_bytes()` to nfa_match
  (it's byte-oriented); `start` is a BYTE offset from char_indices (always a
  valid UTF-8 boundary). Never slice `s` and pass a sub-&str.
- **GOTCHA-I (scope boundary):** this task ports ONLY `match_with_anchors` +
  `pattern_match`. The delimiter-aware `match_pattern` + `Pattern::Single|Parts`
  enum is P2.M1.T3.S2 — do NOT implement it here. `match_with_anchors` is
  `pub(crate)` (internal helper + direct unit testing); `pattern_match` is `pub`
  (the module's public API surface, consumed by T3.S2's delimiter matcher + P3
  rules.rs).
- **GOTCHA-J (parse_pattern reuses T1.S2 verbatim):** `pattern_match` calls the
  EXISTING in-tree `parse_pattern` (P2.M1.T1.S2, complete). Do NOT reimplement
  parsing or anchor detection — that is T1.S2's delivered contract.
- **CRATE QUIRK:** `cargo test --bin qmkonnect -- --test-threads=1` (shared
  debouncer state in notifier.rs; AGENTS.md).
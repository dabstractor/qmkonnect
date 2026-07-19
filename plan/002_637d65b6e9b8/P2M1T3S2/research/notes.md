# Research Notes — P2.M1.T3.S2: Delimiter-aware `match_pattern()` + `Pattern` enum

> Single source of truth: firmware `notifier.c` `match_pattern` (lines 425–530) +
> helpers `find_first_delimiter` (388) / `split_by_delimiter` (398); and
> `spec/HOST_RULES.md` §8(2) + §9 (the Rust `Pattern` enum + `rules.toml`
> schema). PRD §4.1 ("The delimiter-aware matcher") + §14 ("Appendix — File
> Layout & Pattern Subset") are the scoping selectors.

---

## 0. What this task is (and is NOT)

**IS**: the top-level *delimiter-aware* matcher — the public entry a host-side
rule author calls. It takes a **typed** `Pattern` (Single / Parts, deserialized
from `rules.toml`) + the window's **separate** `app_class` and `title` strings,
and dispatches to T3.S1's leaf `pattern_match(pattern, s, cs)` on the right
half/halves.

**IS NOT**: the leaf matcher (that's T3.S1 — already complete in-tree, lines
1061–1066). This task does NOT reimplement NFA/anchor logic; it ONLY does the
GS-delimiter split + variant dispatch, then delegates.

Naming collision to keep straight (both are real, both are needed):
- `pattern_match(pattern: &str, s: &str, cs) -> bool` — **T3.S1's leaf matcher** (NFA). Already `pub` in-tree.
- `match_pattern(pattern: &Pattern, app_class, title, cs) -> bool` — **THIS task's** delimiter-aware wrapper. Takes the enum + two strings.

The names are deliberately kept distinct to mirror the firmware's own split
(firmware has BOTH `pattern_match.c::pattern_match` = leaf NFA, AND
`notifier.c::match_pattern` = delimiter wrapper). Same two-tier naming here.

---

## 1. The C source (verbatim) — `notifier.c`

### 1a. The GS constant — `notifier.h:36`

```c
#define GS_DELIMITER "\x1D"  // ASCII 29 (Group Separator)
#define ETX_TERMINATOR "\x03"  // ASCII 3 (End of Text)
#define WINDOW_TITLE(classname, title) classname GS_DELIMITER title
#define WT(...) WINDOW_TITLE(__VA_ARGS__)
```

So the firmware-side two-part pattern macro `WT("Firefox", "*youtube*")` expands
to the C string literal `"Firefox\x1D*youtube*"`. The GS (0x1D) is the **only**
delimiter. (There is exactly one `GS_DELIMITER` byte per message — the host joins
class+title with it; see §1d.)

### 1b. `find_first_delimiter` — `notifier.c:388`

```c
// Helper function to find the group separator (GS, 0x1D) in a string
const char* find_first_delimiter(const char *str) {
    for (const char *p = str; *p != '\0'; p++) {
        if (*p == GS_DELIMITER[0]) {  // Add any other delimiters here
            return p;
        }
    }
    return NULL;
}
```

### 1c. `split_by_delimiter` — `notifier.c:398`

```c
// Helper function to split string by delimiter
bool split_by_delimiter(const char *source, const char *delimiter_pos,
                         char *left, size_t left_size,
                         char *right, size_t right_size) {
    if (!delimiter_pos) return false;
    size_t left_len = delimiter_pos - source;
    if (left_len >= left_size) return false;
    strncpy(left, source, left_len);
    left[left_len] = '\0';
    if (strlen(delimiter_pos + 1) >= right_size) return false;
    strcpy(right, delimiter_pos + 1);
    return true;
}
```

(The two `*_size >= N` guards are C fixed-buffer-overflow protection — 256-byte
stack buffers. In Rust they vanish: `&str` slices are length-typed, no overflow.
Equivalent to T1.S2/T3.S1 dropping the C `malloc`/`free`/NULL guards.)

### 1d. `match_pattern` — `notifier.c:425` (THE function being ported)

```c
// Generic function for pattern matching with delimiter support
bool match_pattern(const char *pattern, const char *message, bool case_sensitive) {
    // NULL guard FIRST (PRD §8.5 step 2) — find_first_delimiter(pattern) below
    // would dereference a NULL pattern (it loops `for(p=str; *p; p++)`), so this
    // must precede any use of `pattern`. Fixes BUG-1 (former SIGSEGV on NULL).
    if (message == NULL || pattern == NULL) {
        return false;
    }

    const char *pattern_delimiter = find_first_delimiter(pattern);

    if (pattern_delimiter == NULL) {
        // No delimiter in pattern
        // But check if message has a delimiter
        const char *msg_delimiter_pos = find_first_delimiter(message);

        if (msg_delimiter_pos != NULL) {
            // Message has a delimiter but pattern doesn't
            // Match only against first part of message
            char msg_left[256] = {0};
            size_t left_len = msg_delimiter_pos - message;
            if (left_len >= sizeof(msg_left)) return false;
            strncpy(msg_left, message, left_len);
            msg_left[left_len] = '\0';
            // Match pattern against only first part of message
            bool result = pattern_match(pattern, msg_left, case_sensitive);
            return result;
        }

        // No delimiter in either string, use direct pattern matching
        bool result = pattern_match(pattern, message, case_sensitive);
        return result;
    }

    // Pattern contains a delimiter, check if message has the same delimiter
    char delimiter = *pattern_delimiter;
    char *msg_delimiter_pos = NULL;
    for (char *p = (char*)message; *p != '\0'; p++) {
        if (*p == delimiter) { msg_delimiter_pos = p; break; }
    }

    if (msg_delimiter_pos == NULL) {
        // Message doesn't have the delimiter
        // But we should still try to match the part before the delimiter
        char pattern_left[256] = {0};
        size_t left_len = pattern_delimiter - pattern;
        if (left_len >= sizeof(pattern_left)) return false;
        strncpy(pattern_left, pattern, left_len);
        pattern_left[left_len] = '\0';
        // Only match the first part of the pattern against the entire message
        return pattern_match(pattern_left, message, case_sensitive);
    }

    // Split both pattern and message
    char pattern_left[256] = {0}; char pattern_right[256] = {0};
    char msg_left[256] = {0};     char msg_right[256] = {0};
    if (!split_by_delimiter(pattern, pattern_delimiter,
                           pattern_left, sizeof(pattern_left),
                           pattern_right, sizeof(pattern_right))) return false;
    if (!split_by_delimiter(message, msg_delimiter_pos,
                          msg_left, sizeof(msg_left),
                          msg_right, sizeof(msg_right))) return false;
    // Match both sides
    return pattern_match(pattern_left, msg_left, case_sensitive) &&
           pattern_match(pattern_right, msg_right, case_sensitive);
}
```

### 1e. THE host-side fact that collapses the matrix — `notifier.rs:309`

The qmkonnect host **always** emits the GS, joining class + title unconditionally:

```rust
let message = format!("{}{}{}", window_info.app_class, "\x1D", window_info.title);
```

So the message that reaches the firmware is **always** `"app_class\x1Dtitle"`
(even when `title` is empty → `"app_class\x1D"`, a trailing GS with empty right
half). The firmware's two *"message has no delimiter"* branches
(`msg_delimiter_pos == NULL`) therefore only fire for **legacy/test** messages
hand-built without a GS — never for a real qmkonnect window notification.

This is what lets the Rust host model `app_class` and `title` as **two separate
`&str` arguments** instead of re-scanning a joined string for 0x1D.

---

## 2. The four firmware cases → two Rust enum variants (the mapping table)

The firmware `match_pattern` is a 2×2 dispatch on (pattern has GS?) × (message
has GS?). Because the host always emits the GS (§1e), the message column is
effectively constant = "has GS" for real traffic. The **pattern** column is what
`rules.toml` controls, and it maps 1:1 onto the `Pattern` enum variant:

| firmware case                       | pattern has GS? | msg has GS? | firmware action                         | Rust `Pattern` variant | Rust action                                          |
| ----------------------------------- | --------------- | ----------- | --------------------------------------- | ---------------------- | ---------------------------------------------------- |
| A1 — neither has GS                 | no              | no          | `pattern_match(p, whole msg)`           | `Single(p)`            | `pattern_match(p, app_class, cs)` (whole msg=class)  |
| A2 — only msg has GS                | no              | yes         | `pattern_match(p, msg_left)`            | `Single(p)`            | `pattern_match(p, app_class, cs)` (msg_left=class)   |
| B1 — only pattern has GS            | yes             | no          | `pattern_match(p_left, whole msg)`      | `Parts(c,t)`           | `pattern_match(c, app_class, cs) && pattern_match(t, title, cs)` ← spec mandates BOTH halves |
| B2 — both have GS                   | yes             | yes         | `pattern_match(p_left,msg_left) && pattern_match(p_right,msg_right)` | `Parts(c,t)` | `pattern_match(c, app_class, cs) && pattern_match(t, title, cs)` |

**Key observations:**

1. **`Single(p)` → A1 and A2 collapse to the same Rust action**: match `p`
   against `app_class`. (A1 "whole message" and A2 "msg_left" are both
   `app_class` because the host's `app_class` IS the left half / the whole thing
   when there is no title.) → This is the spec's *"if title is non-empty, match p
   against app_class only; else against whole string"* — both branches reduce to
   `app_class`, so the `title` arg is NOT consulted for `Single` (firmware
   parity: a class-only rule never matches on title).

2. **`Parts(c,t)` → B1 and B2 collapse to the same Rust action** per the item
   spec: *"both halves must match."* The spec deliberately does NOT reproduce
   firmware B1's "match only the left half when the message has no GS" behaviour,
   because on the host we ALWAYS know both halves (app_class AND title). So a
   `Parts` rule with an empty title and a non-empty `t` pattern will NOT match
   (`pattern_match(t, "", cs)` → the empty-core special case from T3.S1 makes
   `pattern_match("non-empty-pat", "")` return false). This is the intended,
   predictable semantics for `rules.toml` authors.

3. **`msg_left` ≡ `app_class`, `msg_right` ≡ `title`** — the firmware splits the
   message at the first GS into (left, right); the Rust host already has them
   split as two arguments. No GS scanning needed.

---

## 3. REFINEMENT G — enum dispatch replaces GS scanning

The firmware needs `find_first_delimiter`/`split_by_delimiter` because both the
pattern and the message arrive as **raw C strings with an embedded 0x1D**, and
the matcher must discover the delimiter at runtime.

On the Rust host:

- The **pattern's** delimiter-ness is already known at **deserialization time**:
  `rules.toml` either has `match = "foo"` (a TOML string → `Single`) or
  `match = ["cls", "ttl"]` (a TOML array of 2 → `Parts`). The `Pattern` enum
  variant IS the answer to *"does the pattern have a GS?"*. No byte scan.

- The **message's** split is already known at **window-capture time**:
  `WindowInfo { app_class, title }` (`src/core/types.rs`) keeps them as two
  separate `String`s. The host joins them with GS only at the wire-send boundary
  (`notifier.rs:309`), which is DOWNSTREAM of and irrelevant to host-side rule
  evaluation.

**Resolution:** the entire `find_first_delimiter` + `split_by_delimiter` + the
2×2 if/else cascade collapses into a single `match pattern { Single(p) => …,
Parts(c,t) => … }`. The two C helpers are NOT ported (they would be dead code —
the equivalent of T1.S2 dropping C `free_parsed_pattern` and T3.S1 dropping the
two C wrapper forwarders). The 256-byte buffer-overflow guards vanish too
(`&str` is length-typed).

This mirrors the established refinement pattern in this milestone: every prior
subtask (T1.S2, T2.S2, T3.S1) dropped the C memory-management/forwarding
machinery that Rust makes unnecessary.

---

## 4. The `Pattern` enum (HOST_RULES.md §9, verbatim)

```rust
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Pattern {
    Single(String),                 // "foo"  -> class only
    Parts(String, String),          // ["cls","ttl"]
}
```

### serde `untagged` dispatch (why this works)

`#[serde(untagged)]` makes serde try each variant in declaration order against
the buffered input:

- TOML `match = "alacritty"` → a **scalar string** → `Single(String)` matches
  (a string deserializes into `String`) → `Single("alacritty")`. `Parts(String,
  String)` is never tried.
- TOML `match = ["*chrome*", "*youtube*"]` → a **sequence** → `Single(String)`
  fails (a seq is not a string) → `Parts(String, String)` matches (a 2-element
  seq of strings deserializes into the tuple) → `Parts("*chrome*", "*youtube*")`.
- TOML `match = ["only-one"]` (1-element array) or `match = ["a","b","c"]`
  (3-element) → `Parts(String,String)` requires **exactly** 2 elements → fails →
  the whole `Pattern` deserialization **errors**. This is the desired strict
  behaviour (a malformed `rules.toml` should fail `--validate-rules`, not
  silently coerce). No extra validation code needed.
- TOML `match = 42` (integer) or `match = { ... }` (table) → neither variant
  matches → error. Also desired.

`serde` (with the `derive` feature) and `toml` are already workspace deps
(`Cargo.toml` lines 12 and 21), so importing `serde::Deserialize` into
`pattern.rs` adds **no new dependency**. The file currently has zero serde use;
this task introduces the first `use serde::Deserialize;`.

**Gotcha — `#[derive(Clone, PartialEq)]` too:** HOST_RULES §9 shows only
`Debug, Deserialize`. Add `Clone, PartialEq` (both trivial, zero-cost) so tests
can construct `Pattern` literals with `==` and so downstream code can `.clone()`
it. The in-repo convention (`types.rs`, `pattern.rs`'s `ParsedPattern`/`NfaOp`)
is `#[derive(Debug, Clone, PartialEq)]` on value types. `Deserialize` is added
on top. Do NOT add `Eq` (String already impls it, but it's unnecessary and not
the repo convention).

### Placement: `Pattern` lives in `pattern.rs`, NOT `rules.rs`

HOST_RULES §9's code block shows `Pattern` visually nested under the `rules.rs`
model, but the **functional** home is `pattern.rs` because:

1. `match_pattern` (this task) takes `&Pattern` and is the enum's only
   semantic consumer — they are co-designed (variant ↔ matcher arm).
2. PRD §14 / §8(2) classify `Pattern` as part of the *"host matcher … port the
   firmware pattern_match.c to Rust at src/core/pattern.rs"*.
3. Putting it in `pattern.rs` avoids an awkward import cycle:
   `rules.rs` already needs `crate::core::pattern::pattern_match`-family; if
   `Pattern` lived in `rules.rs`, then `pattern.rs`'s `match_pattern` would have
   to `use crate::core::rules::Pattern`, pulling the whole rules model into the
   matcher. Keeping `Pattern` in `pattern.rs` makes `rules.rs` a one-way
   consumer (`use crate::core::pattern::{Pattern, match_pattern}`).
4. The item title literally co-names them: *"delimiter-aware match_pattern()
   and Pattern enum"* — one deliverable, one file.

**So:** `pub enum Pattern` + `pub fn match_pattern` both land in
`src/core/pattern.rs`. `rules.rs` (P3.M1.T1.S1) will later write
`use crate::core::pattern::{Pattern, match_pattern};` and embed `pattern:
Pattern` in `LayerRule`/`CallbackRule`. (Cross-check: the HOST_RULES §9
`LayerRule { #[serde(rename="match")] pub pattern: Pattern, .. }` is unaffected —
`Pattern` is `pub`, so `rules.rs` can name it from another module.)

---

## 5. The verified Rust skeleton

```rust
use serde::Deserialize;

/// ... (Mode-A rustdoc — see PRP "Implementation Blueprint") ...
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Pattern {
    /// Class-only pattern. Deserialized from a bare TOML string:
    /// `match = "Firefox"`. Matches `app_class` only — firmware parity for a
    /// delimiter-less pattern (it never consults the window title).
    Single(String),
    /// Class + title pattern. Deserialized from a 2-element TOML array:
    /// `match = ["*chrome*", "*youtube*"]` (== firmware `WT(class, title)`).
    /// **Both** halves must match.
    Parts(String, String),
}

/// ... (Mode-A rustdoc — see PRP) ...
///
/// Mirrors firmware `notifier.c::match_pattern` (the GS-delimiter-aware
/// matcher, lines 425–530) using the Group Separator (0x1D) as the conceptual
/// delimiter. On the host, the GS split is structural rather than textual: the
/// `Pattern` variant encodes "does the pattern have a GS?" and `app_class` /
/// `title` are the message's two halves (the host always joins them with 0x1D
/// at the wire boundary — see `src/core/notifier.rs` line 309). Each half is
/// delegated to [`pattern_match`] (T3.S1's leaf NFA matcher).
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
            pattern_match(c, app_class, case_sensitive)
                && pattern_match(t, title, case_sensitive)
        }
    }
}
```

**Compile-check notes:**
- `match pattern { Single(p) => …, Parts(c, t) => … }` is exhaustive (no `_ =>`);
  `clippy::match_like_matches_macro` does not fire (we return `bool` from the
  arms, not constructing the enum).
- `Pattern::Single(p)` binds `p: &String`; passing `p` (auto-deref `&str`) to
  `pattern_match(p, ..)` where `pattern_match: fn(&str, &str, bool)` — Rust
  auto-derefs `&String` → `&str` at the call site. (Alternatively write
  `pattern_match(p, app_class, case_sensitive)`; `p` is `&String`, coerces.)
  `Pattern::Parts(c, t)` binds `c, t: &String` similarly.
- No `unsafe`, no `static`, no new deps beyond the already-present `serde`.

---

## 6. Parity test vectors (the test contract)

All delegate to the in-tree `pattern_match` (T3.S1) for the leaf semantics, so
the leaf-level expectations are inherited. These vectors pin the **delimiter
dispatch + variant mapping** specifically.

### 6.1 `Pattern::Single` — always matches `app_class` (title ignored)

| pattern                                  | app_class   | title          | cs    | expected | why                                                |
| --------------------------------------- | ----------- | -------------- | ----- | -------- | -------------------------------------------------- |
| `Single("Firefox")`                     | `"Firefox"` | `""`           | false | true     | class exact, no title → whole-msg match            |
| `Single("Firefox")`                     | `"Firefox"` | `"Google"`     | false | true     | class exact, title PRESENT but ignored (firmware)  |
| `Single("firefox")`                     | `"Firefox"` | `""`           | false | true     | case-insensitive default                           |
| `Single("firefox")`                     | `"Firefox"` | `""`           | true  | false    | case-sensitive                                     |
| `Single("Firefox")`                     | `"Chrome"`  | `""`           | false | false    | class mismatch                                     |
| `Single("Firefox")`                     | `"Chrome"`  | `"Firefox"`    | false | false    | title matches the PATTERn but Single ignores title |
| `Single("*")`                           | `"anything"`| `""`           | false | true     | glob wildcard matches any class                    |
| `Single("*ire*")`                       | `"Firefox"` | `""`           | false | true     | glob substring                                     |
| `Single("Firefox")`                     | `"firefox"` | `""`           | true  | false    | cs mismatch (only ASCII-folds A–Z)                 |
| `Single("")`                            | `"Firefox"` | `""`           | false | false    | empty pattern → empty-core special case (T3.S1)    |
| `Single("")`                            | `""`        | `""`           | false | true     | empty pattern, empty class → empty-core matches    |
| `Single("^Firefox$")`                   | `"Firefox"` | `""`           | false | true     | anchors work end-to-end through the leaf matcher   |

**Linchpin row**: `Single("Firefox")` / app_class=`"Chrome"` / title=`"Firefox"`
→ **false**. This is the single most important parity assertion: it proves
`Single` consults `app_class` only and never `title`. If an implementer
accidentally matched `Single` against `title` (or against a joined
`"class\x1Dtitle"`), this row catches it.

### 6.2 `Pattern::Parts` — both halves must match

| pattern                                | app_class   | title          | cs    | expected | why                                            |
| -------------------------------------- | ----------- | -------------- | ----- | -------- | ---------------------------------------------- |
| `Parts("Firefox","*youtube*")`         | `"Firefox"` | `"Youtube - X"`| false | true     | both halves match                              |
| `Parts("Firefox","youtube")`           | `"Firefox"` | `"Google"`     | false | false    | title half fails (substring "youtube"∉"Google")|
| `Parts("Chrome","*")`                  | `"Chrome"`  | `"anything"`   | false | true     | title glob matches anything                    |
| `Parts("Chrome","*")`                  | `"Firefox"` | `"anything"`   | false | false    | class half fails                               |
| `Parts("Firefox","")`                  | `"Firefox"` | `""`           | false | true     | empty title-pattern matches empty title (T3.S1 empty-core: `pattern_match("","")`=true) |
| `Parts("Firefox","")`                  | `"Firefox"` | `"Google"`     | false | false    | empty title-pattern vs non-empty title → empty-core: `pattern_match("","Google")`=false |
| `Parts("Firefox","*")`                 | `"Firefox"` | `""`           | false | true     | glob `*` matches empty title                   |
| `Parts("firefox","*youtube*")`         | `"Firefox"` | `"MYoutube"`   | false | true     | ci on both halves                              |
| `Parts("firefox","*youtube*")`         | `"Firefox"` | `"MYoutube"`   | true  | false     | cs fails on both halves                        |
| `Parts("^Firefox$","^*youtube*$")`     | `"Firefox"` | `"youtube"`    | false | true     | anchors on both halves end-to-end              |

**Linchpin row**: `Parts("Firefox","")` / title=`""` → true AND
`Parts("Firefox","")` / title=`"Google"` → false. Together they prove the
"both halves must match" rule interacts correctly with T3.S1's empty-core
special case (the title half is genuinely evaluated, not skipped).

### 6.3 serde `untagged` deserialization (rules.toml → Pattern)

| TOML                          | parsed Pattern                       | note                            |
| ----------------------------- | ------------------------------------ | ------------------------------- |
| `match = "alacritty"`         | `Single("alacritty".into())`         | scalar string → Single          |
| `match = "*chrome*"`          | `Single("*chrome*".into())`          | glob string → Single            |
| `match = ["*chrome*","*yt*"]` | `Parts("*chrome*".into(),"*yt*".into())` | 2-array → Parts             |
| `match = ["a","b","c"]`       | **error**                            | 3-array: Parts needs exactly 2  |
| `match = ["solo"]`            | **error**                            | 1-array: Parts needs exactly 2  |
| `match = 42`                  | **error**                            | int matches no variant          |

Use `toml::from_str` (the `toml = "0.9"` crate, already a dep) on a tiny
`#[derive(Deserialize)] struct Wrap { #[serde(rename="match")] pattern: Pattern }`
to assert these — see PRP validation §Level 2. (This does NOT require `rules.rs`;
it's a pure deserialization unit test living in `pattern::tests`.)

---

## 7. Gotchas (enumerated, each pinned by a §6 row)

- **G1 — `Single` NEVER consults `title`.** Pinned by §6.1 linchpin
  (`Single("Firefox")`/class=`Chrome`/title=`Firefox` → false). An implementer
  who builds the joined `"class\x1Dtitle"` string and matches `Single` against
  it (instead of `app_class`) would wrongly return true here.

- **G2 — `Parts` requires BOTH halves to match (no B1 fallback).** The firmware
  has a B1 branch (pattern has GS, message has no GS → match left half only).
  The item spec withdraws it: on the host we always know both halves, so `Parts`
  always checks `t` against `title`. Pinned by §6.2 `Parts("Firefox","")` rows.
  Do NOT port the `msg_delimiter_pos == NULL` arm.

- **G3 — the GS (0x1D) is NOT scanned in Rust.** The enum variant IS the
  delimiter answer. Do NOT call `find`/`contains('\u{001D}')` on `app_class` or
  `title` — they are clean OS strings (the GS only appears at the wire-join in
  `notifier.rs:309`, downstream of rule evaluation). Porting
  `find_first_delimiter`/`split_by_delimiter` would be dead code (REFINEMENT G).

- **G4 — `serde(untagged)` tries variants IN ORDER.** `Single(String)` MUST be
  declared before `Parts(String,String)`. (Order doesn't change correctness for
  string-vs-seq, but it's the documented serde convention and avoids surprising
  coercions if a future variant is added.) The HOST_RULES §9 declaration order
  is authoritative.

- **G5 — `untagged` is strict on array length.** A 1- or 3-element array errors
  (does NOT coerce to `Single` or truncate). This is desired (malformed
  `rules.toml` must fail `--validate-rules`). No extra validation code needed;
  serde does it. Pinned by §6.3.

- **G6 — `Pattern` derives `Deserialize` but the leaf `pattern_match` does NOT.**
  The enum is the deserialization boundary; the leaf matcher stays pure-stdlib
  (no serde). This task adds `use serde::Deserialize;` as the FIRST serde import
  in `pattern.rs`. Do not sprinkle serde elsewhere in the file.

- **G7 — `Pattern` goes in `pattern.rs`, not `rules.rs`** (§4 placement). If you
  put it in `rules.rs`, `pattern.rs::match_pattern` would need to import from
  `rules.rs`, creating a cycle with P3's `rules.rs` importing `pattern_match`.
  Keep `Pattern` with `match_pattern`.

- **G8 — empty-core special case composes through both halves.** T3.S1's
  GOTCHA-A (`pattern_match("", "non-empty")` → false; `pattern_match("","")` →
  true) propagates into `Parts`: `Parts("Firefox","")` matches only when `title`
  is empty. Don't add a special "empty pattern half" shortcut in `match_pattern`
  — let the leaf matcher handle it (G2).

- **G9 — `&String` → `&str` coercion at the match arms.** `match pattern {
  Single(p) => … }` binds `p: &String`. Passing `p` to `pattern_match(p, ..)`
  (which wants `&str`) auto-derefs. No `.as_str()` needed, but if a clippy lint
  complains, `p.as_str()` or `&**p` are the fixes. (Likely unnecessary.)

- **G10 — borrow checker.** `match_pattern` borrows `pattern` (shared), reads
  `app_class`/`title` (shared `&str`), and delegates to `pattern_match` (which
  borrows its args). All disjoint, no mutation. Compiles clean.

- **G11 — crate-wide test threading.** `cargo test --bin qmkonnect -- --test-threads=1`
  (shared debouncer state in `notifier.rs`, AGENTS.md).

- **G12 — naming collision.** `pattern_match` (T3.S1, leaf NFA, `&str`) vs
  `match_pattern` (THIS task, delimiter-aware, `&Pattern`). Both `pub`, both in
  `pattern.rs`, both needed. Do not rename either — the firmware uses the same
  two names for the same two tiers.

---

## 8. Scope boundary vs siblings

- **Consumes** T3.S1's `pub fn pattern_match(pattern: &str, s: &str,
  case_sensitive: bool) -> bool` (the leaf NFA matcher, in-tree lines
  1061–1066). Calls it on the right half/halves. Does NOT reimplement it.
- **Does NOT touch** T1.S1/T1.S2/T2.S2 internals (`process_escapes`,
  `parse_pattern`, `nfa_compile`, `nfa_match`, `match_with_anchors`).
- **Consumed downstream by** P3.M1.T2.S1 `rules.rs::evaluate`, which will call
  `match_pattern(&rule.pattern, &window.app_class, &window.title,
  rule.case_sensitive)` for each `layer_rule` / `callback_rule`. (P3.M1.T1.S1
  defines `LayerRule`/`CallbackRule` embedding `pattern: Pattern`.)
- **Does NOT build** the joined `"class\x1Dtitle"` message — that stays in
  `notifier.rs:309` (the wire-send path, P4). This matcher receives the two
  halves separately.
- **File touched**: ONLY `src/core/pattern.rs` (additive: `use serde`,
  `pub enum Pattern`, `pub fn match_pattern`, + tests). `mod.rs` already has
  `pub mod pattern;` (no edit). `Cargo.toml` already has `serde` + `toml` (no
  edit). `rules.rs` does not exist yet (P3) — do not create it.
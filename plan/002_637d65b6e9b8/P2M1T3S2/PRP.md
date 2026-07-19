# PRP — P2.M1.T3.S2: Port delimiter-aware `match_pattern()` + `Pattern` enum (Single/Parts)

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This is an **additive edit** to the existing
> module `src/core/pattern.rs`, which already contains the full leaf-matcher
> pipeline delivered by P2.M1.T1/T2/T3.S1: `process_escapes`+consts (T1.S1),
> `ParsedPattern`+`parse_pattern` (T1.S2), `NfaOp`+`nfa_compile` (T1.S1-parallel),
> `nfa_addstate`/`nfa_match`/predicates (T2.S2), and
> `match_with_anchors`+`pub fn pattern_match` (T3.S1 — the leaf NFA entry, in-tree
> lines 985 & 1061). This task adds the **top-level delimiter-aware matcher** and
> the **`Pattern` enum** that deserializes `rules.toml`'s `match` field. Firmware
> `notifier.c::match_pattern` (lines 425–530) + `spec/HOST_RULES.md` §8(2)+§9 are
> the single sources of truth (PRD §4.1, §14). Consumes T3.S1's `pattern_match`;
> consumed downstream by P3.M1.T2 (`rules.rs::evaluate`).

---

## Goal

**Feature Goal**: Port the firmware's **delimiter-aware** `match_pattern()`
(`notifier.c:425–530`) to Rust as the public host-side rule matcher, and define
the `Pattern` enum (`Single(String)` / `Parts(String, String)`) that deserializes
`rules.toml`'s `match` field via `serde(untagged)` (string → Single, 2-array →
Parts). The matcher takes the window's **separate** `app_class` and `title`
strings and dispatches to T3.S1's leaf `pattern_match` on the correct
half/halves, mirroring the firmware's GS (0x1D) split logic with the enum variant
replacing runtime GS-scanning.

**Deliverable**: additions to `src/core/pattern.rs` (do NOT recreate the file):
1. `use serde::Deserialize;` (the file's first serde import — serde is already a Cargo dep);
2. `#[derive(Debug, Clone, PartialEq, Deserialize)] #[serde(untagged)] pub enum Pattern { Single(String), Parts(String, String) }`;
3. `pub fn match_pattern(pattern: &Pattern, app_class: &str, title: &str, case_sensitive: bool) -> bool` — dispatches by variant: `Single(p)` → `pattern_match(p, app_class, case_sensitive)` (title ignored — firmware parity); `Parts(c, t)` → both halves must match;
4. Mode-A rustdoc on `Pattern` and `match_pattern` (construct table, WT() examples, the GS/0x1D split semantics, and the firmware-parity reasoning);
5. new `#[test]` fns appended to the existing `#[cfg(test)] mod tests` block covering ~30 parity vectors: Single-class-only (incl. the title-ignored linchpin), Parts-both-halves (incl. empty-core composition), and serde `untagged` deserialization (string/2-array/wrong-length-error).

**Success Definition**:
- `Pattern` deserializes a bare TOML string to `Single`, a 2-element TOML array to `Parts`, and errors on 1/3-element arrays / non-strings (serde does this for free — no custom visitor).
- `match_pattern(&Single("Firefox"), "Chrome", "Firefox", false)` returns **false** (proves `Single` consults `app_class` only, never `title`).
- `match_pattern(&Parts("Firefox", ""), "Firefox", "", false)` returns **true** AND `match_pattern(&Parts("Firefox", ""), "Firefox", "Google", false)` returns **false** (proves both halves are evaluated, composing with T3.S1's empty-core special case).
- Every vector in `research/notes.md` §6 passes.
- `cargo build --bin qmkonnect` compiles clean (no new warnings).
- `cargo test --bin qmkonnect -- --test-threads=1` passes (new `Pattern`/`match_pattern` tests AND all prior pattern tests: T1.S1/T1.S2/T2.S2/T3.S1).
- `git diff` touches ONLY `src/core/pattern.rs`. No `rules.rs` created (that's P3.M1.T1.S1), no `mod.rs` edit, no `Cargo.toml` edit.

## User Persona (if applicable)

**Target User**: Two consumers, both in the qmkonnect crate:
- **P3.M1.T2.S1** (`rules.rs::evaluate`): for each `layer_rule`/`callback_rule`,
  calls `match_pattern(&rule.pattern, &window.app_class, &window.title, rule.case_sensitive)`.
- **rules.toml authors** (humans writing `[layer_rules]` / `[callback_rules]`):
  they write `match = "alacritty"` (class-only) or `match = ["*chrome*", "*youtube*"]`
  (class+title, == firmware `WT(class, title)`). The `Pattern` enum + serde make
  both forms valid TOML.

**Use Case**: A rule `[[layer_rules]] match = ["*chrome*", "*youtube*"]` against
a window `{ app_class: "Google Chrome", title: "funny cat - YouTube" }` →
`match_pattern(&Parts("*chrome*","*youtube*"), "Google Chrome", "funny cat - YouTube", false)`
→ both halves substring-match → `true`. A rule `match = "alacritty"` against
`{ app_class: "Alacritty", title: "vim" }` → `Single("alacritty")` matches the
class case-insensitively; the title `"vim"` is irrelevant.

**Pain Points Addressed**: Gives host-side `rules.toml` evaluation (P3) a single,
firmware-faithful matcher that understands the two-part `WT(class,title)` /
`Pattern::Parts` form WITHOUT requiring the host to re-join class+title into a
0x1D-laden string and re-scan it. The enum variant IS the delimiter answer.

## Why

- **Completes the matcher API surface.** PRD §14 mandates the Rust matcher be a
  *"full-parity port of the firmware `pattern_match.c`, not a subset"* — and the
  firmware has a **two-tier** matcher: the leaf NFA `pattern_match`
  (`pattern_match.c`, already ported in T3.S1) AND the delimiter-aware wrapper
  `match_pattern` (`notifier.c:425`). This task ports the wrapper. Without it, a
  `rules.toml` author cannot express a two-part `WT(class, title)` rule.
- **The host models class/title separately.** `WindowInfo { app_class, title }`
  (`src/core/types.rs`) keeps them as two `String`s; the host only joins them
  with 0x1D at the wire-send boundary (`notifier.rs:309`). So the host-side rule
  matcher should take them separately and never re-scan for GS — the `Pattern`
  enum (Single vs Parts) structurally captures *"does the pattern have a GS?"*.
- **Enables P3 (rules) + the full host-rules milestone.** P3.M1.T2's `evaluate`
  needs this exact signature. Shipping it unblocks the rules-system milestone,
  `--validate-rules`, and the tray "Reload rules" item.

## What

Add to `src/core/pattern.rs` (APPEND after T3.S1's `pub fn pattern_match`, before
the `#[cfg(test)] mod tests` block — logical order: `… → pattern_match (leaf) →
Pattern enum → match_pattern (delimiter-aware)`). One justified refinement to the
firmware (forced by the Rust host's separate-fields design, fully derived in
`research/notes.md` §3):

### REFINEMENT G — enum dispatch replaces GS-byte scanning
The firmware `match_pattern` is a 2×2 dispatch on (pattern has GS?) × (message
has GS?), using `find_first_delimiter`/`split_by_delimiter` at runtime. On the
Rust host: (a) the **pattern's** GS-ness is known at **deserialization** time
(string→Single, 2-array→Parts), and (b) the **message's** split is known at
**window-capture** time (`WindowInfo` keeps `app_class`/`title` separate; the host
always emits the GS at the wire join per `notifier.rs:309`). So the 2×2 cascade +
both C helpers collapse into a single `match pattern { Single(p) => …, Parts(c,t)
=> … }`. The helpers are NOT ported (dead code — same logic by which T1.S2 dropped
`free_parsed_pattern` and T3.S1 dropped the two C wrapper forwarders). The 256-byte
buffer-overflow guards vanish (`&str` is length-typed).

### The full mapping (research/notes.md §2)

| firmware case      | pattern GS? | msg GS?     | Rust variant | Rust action                                              |
| ------------------ | ----------- | ----------- | ------------ | -------------------------------------------------------- |
| A1 neither         | no          | no          | `Single(p)`  | `pattern_match(p, app_class, cs)`                        |
| A2 msg only        | no          | yes         | `Single(p)`  | `pattern_match(p, app_class, cs)` (msg_left = class)     |
| B1 pattern only    | yes         | no          | `Parts(c,t)` | BOTH halves (spec withdraws firmware's left-only branch) |
| B2 both            | yes         | yes         | `Parts(c,t)` | `pattern_match(c, app_class, cs) && pattern_match(t, title, cs)` |

Because the host **always** emits the GS (`notifier.rs:309`), the message column
is effectively constant = "has GS" for real traffic. `Single` reduces to "match
`app_class`" (title never consulted); `Parts` reduces to "both halves must match".

### Success Criteria
- [ ] `pub enum Pattern { Single(String), Parts(String, String) }` with `#[serde(untagged)]` + `#[derive(Debug, Clone, PartialEq, Deserialize)]`.
- [ ] `pub fn match_pattern(pattern: &Pattern, app_class: &str, title: &str, case_sensitive: bool) -> bool`.
- [ ] `Single(p)` delegates to `pattern_match(p, app_class, case_sensitive)` — title NOT consulted.
- [ ] `Parts(c, t)` delegates to `pattern_match(c, app_class, cs) && pattern_match(t, title, cs)`.
- [ ] serde: string → `Single`; 2-array → `Parts`; 1/3-array → **error**; non-string → **error**.
- [ ] All `research/notes.md` §6 vectors pass (Single, Parts, serde deserialization).
- [ ] No `find_first_delimiter`/`split_by_delimiter` ported (REFINEMENT G); no GS-byte scanning in Rust.
- [ ] No NULL guard, no buffer-overflow guard, no C helper ported.
- [ ] No new deps (serde+toml already present); no `unsafe`; no `static`.
- [ ] `cargo build --bin qmkonnect` clean; `cargo test --bin qmkonnect -- --test-threads=1` green.
- [ ] `git diff` touches only `src/core/pattern.rs`.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge of this codebase can implement this
using only this PRP + the firmware repo, because (a) the ENTIRE C source for the
function being ported (`match_pattern`) + its two helpers
(`find_first_delimiter`, `split_by_delimiter`) + the GS constant + the host-side
message-construction fact are reproduced VERBATIM in `research/notes.md` §1, (b)
the 2×2 firmware case → 2-variant enum mapping table is in §2 (the core design),
(c) REFINEMENT G (enum dispatch replaces GS scanning) is fully derived in §3, (d)
the `Pattern` enum + serde `untagged` dispatch semantics are in §4 (why string →
Single, why 2-array → Parts, why wrong-length errors), (e) a verified Rust
skeleton for `Pattern` + `match_pattern` is in §5 (mirrored in the Implementation
Blueprint), (f) ~30 parity vectors covering Single-class-only (incl. the
title-ignored linchpin), Parts-both-halves (incl. empty-core composition), and
serde deserialization are in §6, (g) 12 gotchas are enumerated with pinning rows
in §7, (h) the upstream T3.S1 contract (`pattern_match` leaf signature) and the
downstream P3 contract (`rules.rs::evaluate` call site) are both explicit. See
`research/notes.md`.

### Documentation & References

```yaml
# MUST READ — the canonical C source (single source of truth, PRD §4.1/§14)
- file: /home/dustin/projects/qmk-notifier/notifier.c
  why: "lines 425-530 are match_pattern (the delimiter-aware wrapper being
        ported), verbatim, with its 2x2 (pattern-GS x msg-GS) dispatch: A1/A2
        (pattern no GS -> match msg_left or whole), B1 (pattern GS, no msg GS ->
        match pattern_left vs whole msg), B2 (both GS -> split both, both halves
        must match). Plus find_first_delimiter (388) + split_by_delimiter (398),
        the two helpers REFINEMENT G retires. Reproduced in research/notes.md §1."
  pattern: "match_pattern: NULL guard -> find_first_delimiter(pattern) -> if NULL:
            check msg for GS (match msg_left if present, else whole) -> else: find
            GS in msg -> if absent match pattern_left vs whole msg -> else split
            both + match both halves (AND)."
  gotcha: "the C NULL guard + 256-byte buffer-overflow guards + the two helpers are
           ALL dead in Rust (GOTCHA-D / REFINEMENT G). The B1 branch (pattern GS,
           no msg GS) is WITHDRAWN by the item spec (Parts always checks both
           halves) — do NOT port it. The message ALWAYS has GS on the host
           (notifier.rs:309), so A1/B1 only fire for legacy/test messages."

# MUST READ — the GS constant + the WT() macro (the two-part pattern form)
- file: /home/dustin/projects/qmk-notifier/notifier.h
  why: "GS_DELIMITER = \"\\x1D\" (ASCII 29, Group Separator) at line 36; WT(...) /
        WINDOW_TITLE(classname, title) = classname GS_DELIMITER title at line
        38-39. WT(\"Firefox\",\"*youtube*\") -> the C literal \"Firefox\\x1D*youtube*\".
        This is what Pattern::Parts deserializes FROM (the 2-array form) and what
        match_pattern models (variant == 'has GS')."
  gotcha: "the GS byte (0x1D) is never scanned in the Rust matcher — the enum
           variant captures it (G3). Cite 0x1D in the rustdoc but don't search for it."

# MUST READ — the host ALWAYS emits the GS (the fact that collapses the matrix)
- file: src/core/notifier.rs
  why: "line 309: `let message = format!(\"{}{}{}\", window_info.app_class, \"\\x1D\",
        window_info.title);` — the host unconditionally joins class+title with GS,
        even when title is empty (-> \"class\\x1D\"). So the firmware's
        'message has no GS' branches (A1-variant, B1) only fire for legacy/test
        messages. This is WHY the Rust host can model app_class/title as two
        separate &str args and dispatch purely by the Pattern variant."
  section: "the notify_qmk message-build (search '\\x1D' / 'format!')"

# MUST READ — HOST_RULES.md §8(2) + §9 (the Pattern enum contract)
- file: spec/HOST_RULES.md
  why: "§8(2): 'Pattern::Single(p): if the window has a title, match p against
        app_class only (firmware parity); else against the whole string.
        Pattern::Parts(c, t): both halves must match.' §9 gives the verbatim Rust
        enum: #[derive(Debug, Deserialize)] #[serde(untagged)] pub enum Pattern {
        Single(String), Parts(String, String) } — string -> Single, [cls,ttl] ->
        Parts. The LayerRule/CallbackRule embed `#[serde(rename=\"match\")] pub
        pattern: Pattern` (P3 will reuse THIS task's Pattern via `use
        crate::core::pattern::Pattern`)."
  section: "## 8. QMKonnect Spec (this repo)" point (2) and "## 9. rules.toml Schema Reference"

# MUST READ — the file THIS task edits (T1/T2/T3.S1 already present)
- file: src/core/pattern.rs
  why: "already has (lines): consts ESC_CARET..GLOB_STAR (22-36), process_escapes
        (85), ParsedPattern (212) + parse_pattern (279), NfaOp (368) + nfa_compile
        (500) + classifiers (582-663), nfa_addstate (728)/nfa_has_match (784)/
        nfa_match (829), suffix_or_substring_loop (913), match_with_anchors (985),
        AND — from T3.S1 — pub fn pattern_match(pattern: &str, s: &str,
        case_sensitive: bool) -> bool (1061) + a 132-test #[cfg(test)] mod tests
        (1068). APPEND `use serde::Deserialize;`, `pub enum Pattern`, `pub fn
        match_pattern`, + new tests HERE, AFTER pattern_match and BEFORE mod tests."
  pattern: "tests grouped with `// --- header ---` comments; assert bool via
            match_pattern and assert Pattern equality via serde round-trip. The
            file currently has #![allow(dead_code)] (API shipped ahead of
            consumers); Pattern/match_pattern are `pub` so no dead-code warning."
  gotcha: "PLACEMENT: Pattern + match_pattern go AFTER the leaf pattern_match
           (T3.S1) and BEFORE mod tests. Add `use serde::Deserialize;` near the
           top imports (it's the file's first serde import). DO NOT recreate the
           file, DO NOT touch the leaf pattern_match, DO NOT touch mod.rs."

# MUST READ — the upstream T3.S1 contract (the leaf matcher this task consumes)
- file: plan/002_637d65b6e9b8/P2M1T3S1/PRP.md
  why: "fixes the leaf signature: pub fn pattern_match(pattern: &str, s: &str,
        case_sensitive: bool) -> bool — parse_pattern -> match_with_anchors ->
        drop. THIS task's match_pattern calls it on app_class (Single) and on
        both app_class+title (Parts). Confirms the empty-core special case
        (pattern_match(\"\", \"non-empty\") -> false; pattern_match(\"\", \"\") ->
        true) which composes into Parts (G8). The leaf is REUSED verbatim — do not
        reimplement (GOTCHA: this task adds a SECOND pub fn, never modifies the first)."
  section: "## What" (the pattern_match signature + GOTCHA-A empty-core special case)
           and "## Goal" (the naming: leaf = pattern_match, wrapper = match_pattern)

# MUST READ — the verified Rust skeleton + the parity table (THIS task's contract)
- file: plan/002_637d65b6e9b8/P2M1T3S2/research/notes.md
  why: "§1 reproduces match_pattern + find_first_delimiter + split_by_delimiter +
        the GS constant + the host message-build fact VERBATIM. §2 is the 2x2 ->
        2-variant mapping table. §3 derives REFINEMENT G (enum dispatch). §4 is
        the Pattern enum + serde untagged dispatch (why string->Single, why
        2-array->Parts, why wrong-length errors). §5 is the verified Rust
        skeleton. §6 lists ~30 parity vectors (the test contract). §7 enumerates
        12 gotchas with pinning rows. §8 is the scope boundary."
  section: "## 5. Verified Rust skeleton" and "## 6. Parity test vectors" and
           "## 7. Gotchas"

# MUST READ — WindowInfo (the source of app_class/title)
- file: src/core/types.rs
  why: "WindowInfo { app_class: String, title: String } + WindowInfo::new. The
        two fields THIS task's match_pattern receives as &str. Downstream P3
        evaluate() will call match_pattern(&rule.pattern, &wi.app_class,
        &wi.title, rule.case_sensitive). Shows the #[derive(Debug, PartialEq)] +
        inline #[cfg(test)] mod tests convention to mirror."
  pattern: "#[derive(Debug, PartialEq)] on value types; this task's Pattern adds
            Clone + Deserialize on top (repo convention)."

# MUST READ — QMKonnect-side architecture contract (cross-repo)
- file: plan/002_637d65b6e9b8/architecture/external_deps.md
  why: "corroborates the matcher contract: pattern.rs is the 'full-parity port of
        firmware pattern_match.c' incl. the two-part WT()/Pattern::Parts form
        (delimiter 0x1D, GS). Confirms rules.rs is a LATER milestone (P3) that
        consumes pattern.rs — so Pattern's home is pattern.rs."

# Reference — serde untagged docs (the deserialization mechanism)
- url: https://serde.rs/enum-representations.html#untagged
  why: "documents #[serde(untagged)]: serde tries each variant in declaration
        order. string -> Single(String) matches first; array -> fails Single,
        matches Parts(String,String) iff exactly 2 elements. This is EXACTLY the
        rules.toml `match = \"foo\"` vs `match = [\"a\",\"b\"]` dispatch."
  critical: "variant declaration ORDER matters (Single before Parts). Wrong-length
             arrays error (no coercion) — desired for --validate-rules strictness."

# MUST READ — PRD selectors that scoped this work
- url: spec/PRD.md (heading h2.74 "Pattern-Matching Syntax" + h3.110 "4.1 The delimiter-aware matcher")
  why: "§4.1: 'If the pattern has a GS delimiter but the message doesn't (or vice
        versa), matching is done on the appropriate side only. If both have it,
        both halves must match.' This is the 2x2 matrix this task ports."
- url: spec/PRD.md (heading h2.92 "Appendix — File Layout & Pattern Subset")
  why: "mandates src/core/pattern.rs as 'full-parity matcher (ported from firmware)',
        including 'two-part WT(class,title) / Pattern::Parts (delimiter 0x1D, GS)'.
        Confirms Pattern lives in pattern.rs (the matcher module), not rules.rs."
```

### Current Codebase tree (qmkonnect, relevant subset)

```bash
src/
  main.rs                 # CLI entry (unchanged)
  core/
    mod.rs                # Config + helpers; ALREADY has `pub mod pattern;` (T1.S1) — DO NOT TOUCH
    pattern.rs            # T1.S1: process_escapes+consts; T1.S2: ParsedPattern+parse_pattern;
                            #             NfaOp+nfa_compile; T2.S2: nfa_* ; T3.S1: match_with_anchors
                            #             + pub fn pattern_match (leaf) + 132-test mod tests
                            #   ← EDIT THIS FILE (additive: + use serde, + pub enum Pattern,
                            #                     + pub fn match_pattern, + tests)
    notifier.rs           # Notifier trait, debouncer, line-309 GS message-build (unchanged)
    types.rs              # WindowInfo { app_class, title } (unchanged) — source of the two halves
  platforms/              # per-OS window monitors (unchanged)
  tray.rs / linux_tray.rs # tray UI (unchanged)
Cargo.toml                # serde 1.0 (+derive), toml 0.9 ALREADY present (unchanged)
```

### Desired Codebase tree (files this task changes)

```bash
src/
  core/
    pattern.rs            # MODIFIED (additive) — + use serde::Deserialize,
                            #                     + pub enum Pattern (Single/Parts, untagged),
                            #                     + pub fn match_pattern (delimiter-aware),
                            #                     + tests
    # mod.rs UNCHANGED (module already registered by T1.S1)
    # Cargo.toml UNCHANGED (serde + toml already deps)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (G1 — Single NEVER consults title): Pattern::Single(p) delegates to
//   pattern_match(p, app_class, cs). The `title` argument is NOT used. This is
//   firmware parity: a class-only rule (delimiter-less pattern) matches the
//   message's LEFT half (app_class) regardless of whether a title exists. Pinned
//   by the linchpin: match_pattern(&Single("Firefox"), "Chrome", "Firefox", false)
//   == false. An implementer who joins "class\x1Dtitle" and matches Single
//   against THAT would wrongly return true (the title half "Firefox" matches).
//
// CRITICAL (G2 — Parts requires BOTH halves; no B1 fallback): Pattern::Parts(c,t)
//   delegates to pattern_match(c, app_class, cs) && pattern_match(t, title, cs).
//   The firmware has a B1 branch (pattern has GS, message has no GS -> match the
//   left half only, ignore the right). The item spec WITHDRAWS it: on the host we
//   always know both halves, so Parts always evaluates t against title. Pinned by
//   Parts("Firefox","") rows (matches iff title is empty). DO NOT port B1.
//
// CRITICAL (G3 — the GS byte is NOT scanned in Rust): the enum variant IS the
//   delimiter answer (Single = no GS, Parts = has GS). Do NOT call
//   .find('\u{001D}') / .contains('\x1d') on app_class or title — they are clean
//   OS strings (the GS only appears at the wire-join in notifier.rs:309,
//   downstream of rule evaluation). find_first_delimiter/split_by_delimiter are
//   NOT ported (REFINEMENT G) — porting them creates dead code.
//
// CRITICAL (G4 — serde untagged variant ORDER): declare Single(String) BEFORE
//   Parts(String, String). serde tries variants in declaration order. (For
//   string-vs-seq the order doesn't change the result, but it's the documented
//   convention and guards against future-variant surprises.) HOST_RULES §9's
//   declaration order is authoritative.
//
// CRITICAL (G5 — untagged is strict on array length): a 1- or 3-element TOML
//   array does NOT coerce to Single and does NOT truncate to Parts — it ERRORS.
//   This is desired (malformed rules.toml must fail --validate-rules). serde does
//   it for free; no custom visitor. Pinned by the deserialization test rows.
//
// GOTCHA (G6 — Pattern derives Deserialize; the leaf pattern_match does NOT): the
//   enum is the deserialization boundary; the leaf matcher stays pure-stdlib. This
//   task adds `use serde::Deserialize;` as the FIRST serde import in pattern.rs
//   (serde + derive feature already a Cargo dep). Do not add serde elsewhere.
//
// GOTCHA (G7 — Pattern goes in pattern.rs, NOT rules.rs): match_pattern takes
//   &Pattern and is the enum's only semantic consumer — co-design them in one
//   file. rules.rs (P3.M1.T1.S1) will later `use crate::core::pattern::Pattern`.
//   Putting Pattern in rules.rs would make pattern.rs import from rules.rs,
//   creating a cycle. Keep Pattern with match_pattern.
//
// GOTCHA (G8 — empty-core composes through both halves): T3.S1's GOTCHA-A
//   (pattern_match("", "non-empty") -> false; pattern_match("","") -> true)
//   propagates into Parts. Parts("Firefox","") matches iff title is empty. Do NOT
//   add a special "empty pattern half" shortcut in match_pattern — let the leaf
//   matcher handle it (mirrors G2).
//
// GOTCHA (G9 — &String -> &str coercion): `match pattern { Single(p) => ... }`
//   binds p: &String. pattern_match(p, app_class, cs) auto-derefs &String -> &str.
//   No .as_str() needed; if clippy complains, p.as_str() is the fix.
//
// GOTCHA (G10 — borrow checker): match_pattern borrows pattern (shared), reads
//   app_class/title (shared &str), delegates to pattern_match. All disjoint, no
//   mutation. Compiles clean, no unsafe.
//
// GOTCHA (G11 — crate-wide test threading): cargo test --bin qmkonnect --
//   --test-threads=1 (shared debouncer state in notifier.rs, AGENTS.md).
//
// GOTCHA (G12 — naming collision, DO NOT rename): pattern_match (T3.S1, leaf NFA,
//   &str) vs match_pattern (THIS task, delimiter-aware, &Pattern). Both pub, both
//   in pattern.rs, both needed. The firmware uses the same two names for the same
//   two tiers (pattern_match.c::pattern_match = leaf; notifier.c::match_pattern =
//   wrapper). Keep them distinct.
//
// CRATE QUIRK: the crate-wide test command MUST be single-threaded:
//   cargo test --bin qmkonnect -- --test-threads=1   (AGENTS.md)
```

## Implementation Blueprint

### Data models and structure

This task adds ONE type — the `Pattern` enum — plus the `match_pattern` function.
No other types. It consumes T3.S1's leaf `pattern_match` verbatim.

```rust
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Pattern {
    Single(String),
    Parts(String, String),
}

pub fn match_pattern(pattern: &Pattern, app_class: &str, title: &str, case_sensitive: bool) -> bool;
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: ADD `use serde::Deserialize;` to src/core/pattern.rs
  - DO: add the import near the existing top-of-file imports (this is the file's
        FIRST serde import — serde + derive feature are already a Cargo dep, so
        no Cargo.toml change).
  - GOTCHA: ONLY serde::Deserialize. Do not pull in serdeSerialize or other items.
  - PLACEMENT: src/core/pattern.rs, top imports block.

Task 2: IMPLEMENT `pub enum Pattern` in src/core/pattern.rs
  - DEFINITION (research/notes.md §4/§5):
      /// ... (Mode-A rustdoc — see Task 4) ...
      #[derive(Debug, Clone, PartialEq, Deserialize)]
      #[serde(untagged)]
      pub enum Pattern {
          /// Class-only. Deserialized from a bare TOML string: `match = "Firefox"`.
          /// Matches `app_class` only — firmware parity for a delimiter-less
          /// pattern (the window title is never consulted).
          Single(String),
          /// Class + title. Deserialized from a 2-element TOML array:
          /// `match = ["*chrome*", "*youtube*"]` (== firmware `WT(class, title)`).
          /// BOTH halves must match.
          Parts(String, String),
      }
  - DERIVES: Debug, Clone, PartialEq (repo convention from types.rs/ParsedPattern),
    PLUS Deserialize (this task's new serde dep). Do NOT add Eq/Serialize/Copy.
  - ATTRIBUTE: #[serde(untagged)] — string -> Single, 2-array -> Parts, wrong-
    length/non-string -> error (G4/G5). Variant ORDER: Single before Parts (G4).
  - VISIBILITY: pub (consumed by P3 rules.rs via `use crate::core::pattern::Pattern`).
  - GOTCHA: Pattern goes in pattern.rs, NOT rules.rs (G7). Do not embed it in any
            rules struct (that's P3.M1.T1.S1).
  - PLACEMENT: src/core/pattern.rs, AFTER the leaf `pub fn pattern_match` (T3.S1)
    and BEFORE `#[cfg(test)] mod tests`.

Task 3: IMPLEMENT `pub fn match_pattern` in src/core/pattern.rs
  - SIGNATURE: pub fn match_pattern(pattern: &Pattern, app_class: &str, title: &str,
    case_sensitive: bool) -> bool
  - BODY (research/notes.md §5 verified skeleton):
      match pattern {
          // Firmware cases A1+A2: pattern has no GS. Match app_class only; title
          // is deliberately NOT consulted (firmware parity — a class-only rule
          // never matches on the window title).
          Pattern::Single(p) => pattern_match(p, app_class, case_sensitive),
          // Firmware case B2: pattern has GS, message has GS (always on host ->
          // notifier.rs:309). Split both, BOTH halves must match. (Spec withdraws
          // firmware B1's "message has no GS -> match left half only" branch: on
          // the host we always know both halves.)
          Pattern::Parts(c, t) => {
              pattern_match(c, app_class, case_sensitive)
                  && pattern_match(t, title, case_sensitive)
          }
      }
  - CALLS: the in-tree leaf `pattern_match` (T3.S1, line ~1061) — REUSED verbatim.
    Do NOT reimplement the NFA/anchor logic (GOTCHA: this task adds a SECOND pub
    fn; it never modifies the leaf).
  - GOTCHA: NO GS-byte scanning (G3). NO C helper ported (REFINEMENT G). NO B1
            branch (G2). Single ignores title (G1). &String -> &str auto-deref (G9).
  - VISIBILITY: pub (consumed by P3 rules.rs::evaluate).
  - PLACEMENT: immediately after the `pub enum Pattern`.

Task 4: ADD rustdoc (Mode A — code-level docs only)
  - ON Pattern (a /// block above the enum): document both variants with their
    TOML deserialization form (string -> Single = class-only; 2-array -> Parts =
    class+title == firmware WT(class,title)), the GS (0x1D) as the conceptual
    delimiter that the enum variant captures structurally, and cite HOST_RULES §8(2)
    + §9. Include WT() examples: `WT("Firefox","*youtube*")` == `match = ["Firefox","*youtube*"]`.
  - ON match_pattern (a /// block above the fn): document the firmware-parity
    mapping (the 2x2 table -> 2 variants from research/notes.md §2), the
    "Single matches app_class only; Parts requires both halves" rule, that it
    delegates each half to [`pattern_match`] (T3.S1 leaf NFA), the GS/0x1D split
    semantics (host always emits GS at notifier.rs:309), and cite firmware
    notifier.c:425-530 + PRD §4.1 + §14. Include a `# Examples` section with:
    ```
    use crate::core::pattern::{Pattern, match_pattern};
    // class-only rule (TOML `match = "alacritty"`):
    assert!(match_pattern(&Pattern::Single("alacritty".into()), "Alacritty", "vim", false));
    // class+title rule (TOML `match = ["*chrome*","*youtube*"]`, == WT):
    assert!(match_pattern(&Pattern::Parts("*chrome*".into(),"*youtube*".into()),
                          "Google Chrome", "cat - YouTube", false));
    ```
  - REFERENCE arch external_deps.md, HOST_RULES.md §8(2)+§9, and the firmware
    notifier.h WT macro as the contract sources.

Task 5: APPEND Pattern / match_pattern unit tests to the existing mod tests
  - DO: add new #[test] fns INTO the existing `mod tests { use super::*; ... }`
        block (which currently ends at test_mwa_full_anchor_single_call). Group
        with the same `// --- header ---` comment style.
  - IMPLEMENT the ~30 vectors of research/notes.md §6 as assertions. Suggested
    groupings:
      // --- Pattern::Single: always matches app_class (title ignored) --- (§6.1, 12 rows)
      // --- Pattern::Parts: both halves must match ---                   (§6.2, 10 rows)
      // --- serde untagged deserialization (rules.toml -> Pattern) ---    (§6.3, 6 rows)
  - HIGHLIGHT the Single title-ignored linchpin (G1):
        assert!(!match_pattern(&Pattern::Single("Firefox".into()), "Chrome", "Firefox", false));
  - HIGHLIGHT the Parts empty-core composition (G2/G8):
        assert!( match_pattern(&Pattern::Parts("Firefox".into(), "".into()), "Firefox", "", false));
        assert!(!match_pattern(&Pattern::Parts("Firefox".into(), "".into()), "Firefox", "Google", false));
  - HIGHLIGHT case sensitivity through both halves:
        assert!( match_pattern(&Pattern::Parts("firefox".into(),"*youtube*".into()), "Firefox", "MYoutube", false));
        assert!(!match_pattern(&Pattern::Parts("firefox".into(),"*youtube*".into()), "Firefox", "MYoutube", true));
  - SERDE TESTS: use a tiny helper struct + toml::from_str (toml 0.9 is a dep):
        #[derive(serde::Deserialize)] struct W { #[serde(rename="match")] pattern: Pattern }
        // scalar string -> Single
        assert_eq!(toml::from_str::<W>(r#"match = "alacritty""#).unwrap().pattern,
                   Pattern::Single("alacritty".into()));
        // 2-array -> Parts
        assert_eq!(toml::from_str::<W>(r#"match = ["*chrome*", "*youtube*"]"#).unwrap().pattern,
                   Pattern::Parts("*chrome*".into(), "*youtube*".into()));
        // 3-array / 1-array / int -> ERROR (serde strictness, G5)
        assert!(toml::from_str::<W>(r#"match = ["a","b","c"]"#).is_err());
        assert!(toml::from_str::<W>(r#"match = ["solo"]"#).is_err());
        assert!(toml::from_str::<W>(r#"match = 42"#).is_err());
    (NOTE: derive serde::Deserialize on the local test helper W inline — do NOT
    add it to Pattern, which is already Deserialize. Using `#[derive(serde::Deserialize)]`
    with the full path avoids a second `use` in the test module.)
  - NAMING: test_mp_single_<behavior> (e.g. test_mp_single_matches_app_class_only,
    test_mp_single_ignores_title_linchpin, test_mp_single_case_sensitive,
    test_mp_single_empty_pattern_empty_core) and test_mp_parts_<behavior> (e.g.
    test_mp_parts_both_halves_must_match, test_mp_parts_empty_title_composes,
    test_mp_parts_case_sensitive_both_halves) and test_pattern_serde_<behavior>
    (e.g. test_pattern_serde_string_to_single, test_pattern_serde_two_array_to_parts,
    test_pattern_serde_wrong_length_errors).
  - COVERAGE: every §6 row; the Single linchpin; the Parts empty-core pair; serde
    string/2-array/wrong-length; case on/off through both halves; glob + anchors
    end-to-end through the leaf matcher.

Task 6: VERIFY build + tests (single-threaded, per AGENTS.md)
  - RUN: cargo build --bin qmkonnect        (expect: clean; no NEW warnings)
  - RUN: cargo test --bin qmkonnect pattern -- --test-threads=1
         (expect: ALL pattern::tests pass — new Pattern/match_pattern tests AND
          T1.S1/T1.S2/T2.S2/T3.S1 tests)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1
         (expect: full crate green — no regression)
  - IF a parity assertion fails: re-read the C source (notifier.c match_pattern)
    + research/notes.md §2 mapping + §5 skeleton. The mapping is faithful, so a
    failure means the Rust diverged. Do NOT "fix" the test to match the Rust — fix
    the Rust to match the firmware (PRD §14: firmware is source of truth). The
    ONE exception: the B1 branch (Parts + empty title) — the item spec mandates
    "both halves must match", so if your Rust returns the firmware-B1 answer
    (match left half only) on an empty-title Parts rule, that is the BUG (G2); the
    test is correct.
  - CONFIRM git status shows ONLY src/core/pattern.rs modified.
```

### Implementation Patterns & Key Details

```rust
// The canonical Pattern enum + match_pattern body (this IS the spec — match it).
// Full verified version in research/notes.md §5.
//
// use serde::Deserialize;
//
// #[derive(Debug, Clone, PartialEq, Deserialize)]
// #[serde(untagged)]
// pub enum Pattern {
//     Single(String),
//     Parts(String, String),
// }
//
// pub fn match_pattern(
//     pattern: &Pattern,
//     app_class: &str,
//     title: &str,
//     case_sensitive: bool,
// ) -> bool {
//     match pattern {
//         Pattern::Single(p) => pattern_match(p, app_class, case_sensitive),
//         Pattern::Parts(c, t) => {
//             pattern_match(c, app_class, case_sensitive)
//                 && pattern_match(t, title, case_sensitive)
//         }
//     }
// }
//
// NOTE: pattern_match on the RHS is T3.S1's LEAF matcher (pub fn pattern_match(
// pattern: &str, s: &str, case_sensitive: bool) -> bool), NOT this fn. The two
// names differ by word order (pattern_match vs match_pattern) — same as the
// firmware's two-tier split. Do not confuse them.
```

### Integration Points

```yaml
MODULE REGISTRATION:
  - NONE. `pub mod pattern;` is already in src/core/mod.rs (added by T1.S1). Do NOT
    edit mod.rs.

DEPENDENCIES (this task): NONE new. serde (1.0 + derive) and toml (0.9) are ALREADY
                           Cargo deps (Cargo.toml lines 12, 21). This task adds the
                           first `use serde::Deserialize;` inside pattern.rs. No
                           Cargo.toml edit, no qmk_notifier crate, no firmware link,
                           no `unsafe`, no `static`.

UPSTREAM (already present — T3.S1 leaf matcher contract):
  - pub fn pattern_match(pattern: &str, s: &str, case_sensitive: bool) -> bool
    (T3.S1, in-tree ~line 1061). REUSED verbatim — called on app_class (Single) and
    on app_class+title (Parts). Do NOT reimplement. Inherit its empty-core special
    case (pattern_match("", "non-empty") -> false; pattern_match("","") -> true),
    which composes into Parts (G8).

DOWNSTREAM CONSUMERS (later subtasks — do NOT implement them here):
  - P3.M1.T1.S1 rules.rs: will define LayerRule/CallbackRule embedding
    `#[serde(rename="match")] pub pattern: Pattern` and will
    `use crate::core::pattern::{Pattern, match_pattern};`. THIS task ships both.
  - P3.M1.T2.S1 rules.rs::evaluate: will call
    match_pattern(&rule.pattern, &window.app_class, &window.title, rule.case_sensitive)
    for each layer_rule (first-match-wins) / callback_rule (all-match). THIS task's
    signature is its contract.

CONFIG: none.
ROUTES: none (no CLI surface in this subtask — --validate-rules is P5.M1.T1.S1).
DATABASE: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles clean. If rustc/clippy warns on src/core/pattern.rs, READ it
# and fix. The file carries #![allow(dead_code)] from T1.S1 (API shipped ahead of
# consumers); Pattern + match_pattern are `pub`, so no spurious dead-code warning.
# Possible clippy nits: &String -> &str at the match arms (G9) — p.as_str() if flagged.

# Confirm the additions are present:
grep -n 'use serde::Deserialize' src/core/pattern.rs     # expect one import
grep -n 'pub enum Pattern' src/core/pattern.rs           # expect one enum def
grep -n '#\[serde(untagged)\]' src/core/pattern.rs       # expect one attr on Pattern
grep -n 'pub fn match_pattern' src/core/pattern.rs       # expect one fn def
grep -n 'pattern_match(p, app_class' src/core/pattern.rs # the Single arm
grep -n 'pattern_match(t, title' src/core/pattern.rs     # the Parts arm (title half)
```

### Level 2: Unit Tests — the parity contract (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# Single-threaded is MANDATORY crate-wide (shared debouncer state in notifier.rs).
cargo test --bin qmkonnect pattern -- --test-threads=1
# Expected: every test in pattern::tests passes — the new Pattern/match_pattern
# tests (research/notes.md §6, ~30 vectors) AND T1.S1/T1.S2/T2.S2/T3.S1 tests.
# A failure means the Rust diverged from the firmware C OR the item spec — fix the
# Rust, not the test. Filter to just the new tests to see them individually:
cargo test --bin qmkonnect pattern::tests::test_mp_single -- --test-threads=1
cargo test --bin qmkonnect pattern::tests::test_mp_parts  -- --test-threads=1
cargo test --bin qmkonnect pattern::tests::test_pattern_serde -- --test-threads=1
```

### Level 3: Full-crate regression (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL bin tests green — pattern::tests (new Pattern/match_pattern +
# T1.S1 + T1.S2 + nfa_compile + T2.S2 nfa_match + T3.S1 match_with_anchors/leaf
# pattern_match) + notifier + types. Proves the additive edit + the new serde
# import didn't break module resolution and compiles in the full crate context.

# Confirm the change surface is exactly one file:
git status --short
# Expected:
#   modified:   src/core/pattern.rs        (ONLY this)
git diff --stat
# Expected: only src/core/pattern.rs changed; mod.rs, Cargo.toml, everything else untouched.
```

### Level 4: Fidelity cross-check (optional, high-confidence)

```bash
# Cross-validate against the firmware's own corpus. The Rust parity vectors in
# research/notes.md §6 were DERIVED from the firmware match_pattern semantics, so
# a green firmware run corroborates the contract the Rust port encodes. The Rust
# tests are STRICTLY STRONGER on the host side: they assert the Single-ignores-
# title + Parts-both-halves semantics that the firmware only exhibits indirectly
# (via the always-emitted-GS message at notifier.rs:309).
cd /home/dustin/projects/qmk-notifier && ./run_all_tests.sh
# Expected: the full firmware corpus passes (it always does — this task does not
# touch the firmware). Particularly the WT(class,title) two-part rule cases.
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean (no NEW warnings on `src/core/pattern.rs`).
- [ ] `cargo test --bin qmkonnect pattern -- --test-threads=1` — all pattern tests pass (new + T1.S1 + T1.S2 + T2.S2 + T3.S1).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` — full crate green.
- [ ] `git status` shows exactly ONE modified file: `src/core/pattern.rs`.

### Feature Validation (parity)
- [ ] Every row of `research/notes.md` §6 (~30 vectors) asserted and passing.
- [ ] **`Single` ignores title** (G1 linchpin): `match_pattern(&Single("Firefox"), "Chrome", "Firefox", false)` == false.
- [ ] **`Single` matches app_class** (§6.1): exact, glob, substring, case on/off, anchors end-to-end.
- [ ] **`Single` empty-pattern empty-core**: `match_pattern(&Single(""), "Firefox", "", false)` == false; `(&Single(""), "", "", false)` == true.
- [ ] **`Parts` both halves must match** (§6.2): both-match true; either-half-fail false.
- [ ] **`Parts` empty-core composition** (G8): `Parts("Firefox","")` / title="" → true; / title="Google" → false.
- [ ] **`Parts` case sensitivity both halves**: ci true, cs false.
- [ ] **serde string → Single**: `match = "alacritty"` deserializes to `Single("alacritty")`.
- [ ] **serde 2-array → Parts**: `match = ["*chrome*","*youtube*"]` deserializes to `Parts(...)`.
- [ ] **serde wrong-length errors** (G5): 1-array, 3-array, int all → `is_err()`.

### Code Quality Validation
- [ ] `Pattern` is `pub` with `#[derive(Debug, Clone, PartialEq, Deserialize)]` + `#[serde(untagged)]`; `match_pattern` is `pub`.
- [ ] REFINEMENT G applied: enum dispatch replaces GS-byte scanning; NO `find_first_delimiter`/`split_by_delimiter` ported.
- [ ] NO NULL guard; NO buffer-overflow guard; NO C helper ported; NO B1 branch (G2).
- [ ] `Single` arm delegates to `pattern_match(p, app_class, cs)` — title NOT consulted (G1).
- [ ] `Parts` arm delegates to `pattern_match(c, app_class, cs) && pattern_match(t, title, cs)` (G2).
- [ ] serde variant ORDER: `Single` before `Parts` (G4).
- [ ] NO `static`; NO `unsafe`; reuses T3.S1 leaf `pattern_match` verbatim (never modifies it).
- [ ] Rustdoc (Mode A) on `Pattern` (variants + TOML forms + WT() examples) and `match_pattern` (2×2 mapping + GS/0x1D + firmware cite).
- [ ] New tests appended to the existing `mod tests` (prior tests untouched), grouped with `// --- header ---` comments.
- [ ] No new Cargo dependencies (serde + toml already present).
- [ ] Scope respected: NO `rules.rs` created (P3.M1.T1.S1), NO `mod.rs`/`Cargo.toml` edit, NO leaf `pattern_match` change.

### Documentation & Deployment
- [ ] Code-level rustdoc present (Mode A — no `docs/*.md` or README changes this task).
- [ ] `Pattern` documents both variants' TOML forms + the GS/0x1D conceptual delimiter + WT() examples.
- [ ] `match_pattern` documents the firmware-parity mapping (2×2 → 2 variants) + the Single/Parts rules + delegation to the leaf matcher.

---

## Anti-Patterns to Avoid

- ❌ Do NOT match `Single` against a joined `"app_class\x1Dtitle"` string. `Single`
      matches `app_class` ONLY (G1). Joining would make `Single("Firefox")` wrongly
      match a window whose TITLE is "Firefox" but class isn't (the linchpin row).
- ❌ Do NOT port the firmware B1 branch (Parts + message-without-GS → match left half
      only). The item spec withdraws it: `Parts` ALWAYS checks both halves (G2).
      `Parts("Firefox","")` must NOT match when title is non-empty.
- ❌ Do NOT scan `app_class`/`title` for the GS byte (0x1D). The enum variant IS the
      delimiter answer (G3). `find_first_delimiter`/`split_by_delimiter` are dead
      code in Rust — don't port them (REFINEMENT G).
- ❌ Do NOT add a NULL guard or a buffer-overflow guard. Rust `&str`/`&Pattern` are
      never null and length-typed (the C guards were for `NULL` deref + 256-byte
      stack buffers).
- ❌ Do NOT rename `pattern_match` (T3.S1 leaf) or `match_pattern` (this task's
      wrapper). The two names mirror the firmware's two-tier split and are both
      needed (G12). Confusing them is the #1 footgun.
- ❌ Do NOT reimplement the leaf NFA/anchor logic. Reuse T3.S1's `pattern_match`
      verbatim — this task only adds the delimiter dispatch on top.
- ❌ Do NOT add `Eq`/`Serialize`/`Copy` to `Pattern`. The repo convention is
      `Debug, Clone, PartialEq`; this task adds `Deserialize` (G6).
- ❌ Do NOT declare `Parts` before `Single`. serde `untagged` tries variants in
      order (G4) — HOST_RULES §9's order (Single, Parts) is authoritative.
- ❌ Do NOT put `Pattern` in `rules.rs`. It lives in `pattern.rs` with its only
      semantic consumer `match_pattern` (G7). Putting it elsewhere creates an
      import cycle with P3.
- ❌ Do NOT add a custom serde visitor / `TryFrom` for array-length validation.
      `untagged` + the tuple variant already error on wrong-length arrays (G5) —
      serde does it for free.
- ❌ Do NOT change the test to match divergent Rust output. The firmware C
      (`notifier.c::match_pattern`) + the item spec ("both halves must match") are
      the source of truth (PRD §14); fix the Rust.
- ❌ Do NOT run tests multi-threaded — the crate shares debouncer state
      (`cargo test --bin qmkonnect -- --test-threads=1`).
- ❌ Do NOT edit `PRD.md`, any `tasks.json`, `prd_snapshot.md`, `spec/HOST_RULES.md`,
      or any `plan/` file other than this item's own `PRP.md` + `research/`.

---

## Confidence Score: 9/10

This is a well-bounded port of one firmware C function (`notifier.c::match_pattern`,
425–530) whose ENTIRE source + its two helpers + the GS constant + the
host-message-build fact are reproduced VERBATIM in `research/notes.md` §1, and
transcribed as a verified Rust skeleton in §5 (mirrored in the Implementation
Blueprint). The core design — a 2×2 firmware case matrix collapsing onto a 2-variant
enum (§2 mapping table) — is fully derived, and the single refinement (G: enum
dispatch replaces GS-byte scanning) follows the established pattern of every prior
subtask in this milestone dropping C memory/scanning machinery. The `Pattern` enum
+ serde `untagged` dispatch is given verbatim by HOST_RULES.md §9 and is the most
idiomatic Rust deserialization for the string-vs-2-array `rules.toml` form (§4
derives why wrong-length arrays error for free). ~30 parity vectors cover the
Single-class-only semantics (incl. the title-ignored linchpin that catches a
join-string bug), the Parts-both-halves semantics (incl. the empty-core composition
pair), and serde deserialization (string/2-array/wrong-length). The upstream T3.S1
contract (leaf `pattern_match` signature + empty-core special case) is confirmed
from its PRP and the in-tree file, and the downstream P3 consumer (`rules.rs::
evaluate` call site + `use crate::core::pattern::Pattern`) is explicit. serde + toml
are already Cargo deps, so no `Cargo.toml` change. No `unsafe`, no `static`. The
1-point reservation is for the (unlikely) event an implementer (a) confuses the two
`pattern_match`/`match_pattern` names (G12 — caught by a compile error if they call
the wrong one with the wrong arg types), (b) accidentally ports the B1 branch (G2 —
caught by the Parts-empty-title test pair), or (c) joins class+title for the Single
arm (G1 — caught by the linchpin row). All three are caught immediately by the
parity tests. Scope is cleanly bounded from T3.S1 (upstream leaf matcher, untouched)
and P3 (downstream rules.rs, not yet created), so there is no risk of over- or
under-building.
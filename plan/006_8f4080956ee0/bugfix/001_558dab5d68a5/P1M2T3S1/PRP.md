# PRP — P1.M2.T3.S1: Verify and update `docs/troubleshooting.md` window-class guidance

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`) — Rust tray/menu-bar daemon.
> **One file edited:** `docs/troubleshooting.md` — **one sentence inserted** into
> checklist item #3 of the "Device shows connected but rules not applying" section
> (~L571-577). **No other file touched. No code change.** This IS the docs-sync task.
> **Scope:** PRD Recommendation h2.5 / docs Mode B. Do NOT rewrite the section, do NOT
> edit README/configuration.md/any `.rs`, do NOT touch other troubleshooting sections.

---

## ⚠️ READ FIRST — this is a DOCS-only task with a surgical contract

The contract (P1.M2.T3.S1, point #3) is explicit and constraining:
> "Read `docs/troubleshooting.md` around L565-580. If the existing text already
> accurately describes 'window class' generically, no change is needed — note this in
> the commit message. If it would benefit from a clarifying note about Hyprland
> `initial_class` vs X11 `WM_CLASS` class, add a one-line note. **Do NOT rewrite the
> section — only add precision where a user would be confused.**"

➡️ **Decision taken in this PRP: ADD a clarifying note.** Rationale (full evidence in
`research/notes.md`): the two prior fixes (P1.M1.T1.S1 Hyprland `initial_class`,
P1.M1.T2.S1 X11 class-field) corrected exactly the discrepancy a rule author hits
when cross-referencing a **native tool**:
- On **Hyprland**, `hyprctl clients`/`activewindow` prints the **`class`** field, but
  QMKonnect now sends **`initial_class`**. These diverge for apps that change class
  after launch — this is literally what PRD Issue 1 (h3.0) is titled: *"Hyprland 'Show
  Window Information' reports class, but the keyboard receives initial_class."*
- On **X11**, `xprop WM_CLASS` prints `"instance", "Class"`; QMKonnect matches the
  **2nd** field (class), not the 1st (instance) — which was the shipped bug.

A user who eyeballs `hyprctl`/`xprop` and pastes that value into `rules.toml` can get a
non-matching pattern. The inserted sentence names both platform specifics and
reinforces that the `-v` / "Show Window Information" value is authoritative.

➡️ **The edit is ONE sentence** inserted mid-item. The existing prose, the `qmkonnect -v`
code block, and the `*chrome*` example are **unchanged**.

---

## Goal

**Feature Goal**: Make `docs/troubleshooting.md` checklist item #3 ("Pattern matches
the real window class?") precise about *which* stable identifier QMKonnect matches on
each Linux backend, so a `rules.toml` author who cross-references `hyprctl` or `xprop`
is not misled into using the wrong field — closing the documentation gap left by the
P1.M1.T1.S1 (Hyprland `initial_class`) and P1.M1.T2.S1 (X11 class-field) fixes.

**Deliverable**: `docs/troubleshooting.md` with a single new sentence inserted into
item #3 (after "Show Window Information")." and before "A `*chrome*` rule …") stating:
the value shown is authoritative (matched as-is); on Hyprland it is `initial_class`
(which can differ from `hyprctl`'s `class`); on X11 it is the **class** — the 2nd field
of `xprop WM_CLASS`, not the instance.

**Success Definition**:
- Item #3 contains the clarifying sentence (Hyprland `initial_class` + X11 WM_CLASS
  class-field both named) and still reads naturally.
- All pre-existing text in item #3 (the `qmkonnect -v | grep …` block, the "Show
  Window Information" reference, and the `*chrome*` / `Google Chrome` example) is
  **byte-for-byte unchanged**.
- No other item in the section, no other section in the file, and no other file is
  modified.
- Commit message states the decision ("add clarifying note") and why (prior fixes made
  the matched identifier `initial_class` on Hyprland / WM_CLASS class on X11; native
  tools show a different field).
- `git diff --stat` shows **only** `docs/troubleshooting.md`.

## User Persona (if applicable)

**Target User**: a Linux user writing/maintaining `rules.toml` host rules who, when a
pattern fails to match, opens a terminal and runs `hyprctl activewindow` (Hyprland) or
`xprop WM_CLASS` (X11) to find the "window class" — and copies that value into their rule.

**Use Case**: User's `match = "…"` rule doesn't fire. They run `hyprctl activewindow`,
see `"class": foot`, and use `foot` — but QMKonnect actually matched `initial_class`
(= `foot` here, but for an app that re-classes itself it differs). Or on X11 they read
`xprop`'s first quoted value (`firefox`, the instance) instead of the second (`Firefox`,
the class). Either way they paste the wrong string.

**User Journey**: rule doesn't match → open Troubleshooting → "Device shows connected
but rules not applying" → item #3 → now reads that the matched value is what `-v`/the
dialog show, and that on Hyprland it's `initial_class` (not `hyprctl`'s `class`) and on
X11 it's the WM_CLASS **class** (2nd field) → user trusts the in-app value and the rule
matches.

**Pain Points Addressed**: eliminates the "I copied the class from hyprctl/xprop and it
still doesn't match" confusion that the identifier bugs themselves created.

## Why

- **Closes the docs loop on the two identifier fixes.** P1.M1.T1.S1 and P1.M1.T2.S1
  corrected the *behavior* (now a stable, consistent identifier is sent). This task
  corrects the *documentation* so authors know which stable identifier that is and why
  a native tool may disagree.
- **Native tools are the natural reflex** for a Linux power user debugging rules, and
  they show a different field than QMKonnect now uses — the single highest-friction
  confusion point post-fix. One sentence resolves it.
- **Generic text is otherwise accurate and stays.** The existing guidance ("check what
  QMKonnect actually sees") is correct; macOS (`localizedName`) and Windows (Win32
  window class) match intuition and are NOT called out (no confusion trap there).
- **Scoped & surgical.** One sentence, one file, one section. No rewrite, no code, no
  other docs (README/configuration.md use generic `{application_class}` which remains
  accurate — owned by P1.M2.T3.S2).

## What

### The edit — insert ONE sentence into checklist item #3 (`docs/troubleshooting.md`)

Exact replacement (the `oldText` is the unique anchor spanning L576-577; `newText`
inserts one sentence and keeps the trailing example line verbatim):

```text
oldText:
   (or use the tray's "Show Window Information"). A `*chrome*` rule won't match a

newText:
   (or use the tray's "Show Window Information"). That value is exactly what your
   pattern is matched against, so trust it over a native tool — on Hyprland
   QMKonnect uses the window's `initial_class` (which can differ from the `class`
   field `hyprctl` prints), and on X11 it uses the **class** (the second field of
   `xprop WM_CLASS`, not the instance). A `*chrome*` rule won't match a
```

Resulting item #3 in full (for review — the inserted sentence is the only change):
```markdown
3. **Pattern matches the real window class?** The matcher is class-only for a
   bare `match` string. Check what QMKonnect actually sees:
   ```bash
   qmkonnect -v | grep -i "window\|sending"     # the class\x1Dtitle string sent
   ```
   (or use the tray's "Show Window Information"). That value is exactly what your
   pattern is matched against, so trust it over a native tool — on Hyprland
   QMKonnect uses the window's `initial_class` (which can differ from the `class`
   field `hyprctl` prints), and on X11 it uses the **class** (the second field of
   `xprop WM_CLASS`, not the instance). A `*chrome*` rule won't match a
   class reported as `Google Chrome` — adjust the pattern or use a `[class, title]`
   array.
```

### Success Criteria
- [ ] The sentence "That value is exactly what your pattern is matched against … not
      the instance)." is present in item #3, naming BOTH `initial_class` (Hyprland) and
      the WM_CLASS **class** / 2nd field (X11).
- [ ] The `qmkonnect -v | grep …` fenced block, the "Show Window Information" reference,
      and the `*chrome*`/`Google Chrome` example are unchanged.
- [ ] Markdown structure intact: the ` ```bash ` fence is balanced, list-item
      indentation (3 spaces) is preserved, no stray blank lines added inside the item.
- [ ] Inserted prose wraps at ~75-80 cols to match surrounding lines (the file uses
      Jekyll; no linter, but match house style).
- [ ] Only `docs/troubleshooting.md` changed; only item #3 of this one section changed.
- [ ] Commit message documents the decision (see "Commit message" below).

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can make the single-sentence insertion using
only the exact `oldText`/`newText` above, the resulting-item review block, the
markdown-integrity checks, and the scope guard below — all present in this PRP. No
knowledge of the Rust codebase is required to *make* the edit (the code references
below justify *why* the note is correct).

### Documentation & References

```yaml
# MUST READ — the file & exact section being edited
- file: docs/troubleshooting.md
  why: "the only file to edit; checklist item #3 of 'Device shows connected but rules
        not applying' (~L571-577) is the edit target"
  pattern: "front matter (layout: default, permalink: /troubleshooting/) then body;
            numbered-list items are indented 3 spaces; prose wraps ~75-80 cols;
            the ```bash fence inside item #3 must stay balanced"
  gotcha: "it is a Jekyll/GitHub-Pages page — keep markdown valid for the site build;
           do NOT add/remove the code fence or change its lang tag"

# MUST READ — why the note is correct (the prior fixes' source of truth)
- file: src/platforms/hyprland.rs
  why: "proves app_class is now initial_class: L398, L474/479, L559-571 (list_foreground_
        windows maps (initial_class, title)), L577. hyprctl clients/activewindow print
        the `class` field, which can differ from initial_class — the exact confusion."
  pattern: "app_class: active_window.initial_class.clone()"
  gotcha: "do NOT edit this file — it is the evidence, not the target."

- file: src/platforms/x11.rs
  why: "proves parse_wm_class now returns the CLASS (2nd field of WM_CLASS), not the
        instance: L67 call site, L80-89 docstring, L205-240 unit tests. xprop prints
        BOTH fields ('instance', 'Class'); the note tells users to use the 2nd."
  pattern: "fn parse_wm_class(rest) -> Option<String> ... 'Prefers the class (2nd field)'"
  gotcha: "do NOT edit this file — it is the evidence, not the target."

# REFERENCE — what the user sees (the authoritative value the note points to)
- file: src/platforms/x11.rs
  why: "L148-153 verbose log prints 'Window changed - Class: {}, Title: {}' — i.e. the
        exact app_class the user should match. Confirms 'trust the -v value'."
- file: src/platforms/mod.rs
  why: "L85-93 list_foreground_windows() dispatches per-OS; the 'Show Window Information'
        dialog rows are exactly (app_class, title). Confirms the dialog value is what is sent."

# REFERENCE — unchanged platforms (NOT called out in the note — they match intuition)
- file: src/platforms/macos.rs
  why: "L277/325/334/374/413: app_class = NSRunningApp.localizedName; doc-comment L334
        says 'class is the app's localizedName — exactly the value QMKonnect sends'. No
        confusion trap → macOS is intentionally NOT mentioned in the note."
- file: src/platforms/windows.rs
  why: "app_class = Win32 window class (GetClassName), stable & intuitive → Windows is
        intentionally NOT mentioned in the note."

# REFERENCE — scope boundaries (DO NOT EDIT these; they remain accurate post-fix)
- file: README.md
  why: "L303 uses generic {application_class}{GS}{window_title} — still accurate."
- file: docs/configuration.md
  why: "L281/L503/L510 use generic 'window class only' / {application_class} — still
        accurate. Broader README/docs consistency for the heuristic is P1.M2.T3.S2."
- file: docs/troubleshooting.md
  why: "L188 ({application_class}{GS}{window_title} format example) and L362 (macOS
        'app name only' Screen-Recording note) are accurate — DO NOT change them."

# REFERENCE — the research that produced this decision
- docfile: plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/P1M2T3S1/research/notes.md
  why: "full evidence: per-platform identifier source, verbose/dialog value, current
        text, decision rationale, and doc-tooling facts."
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
docs/troubleshooting.md        # EDIT: insert ONE sentence into checklist item #3 (~L571-577)
  - L1-6   front matter (layout: default, title, permalink) — DO NOT TOUCH
  - L559-  "### Device shows connected but rules not applying"
  - L571-  "3. **Pattern matches the real window class?** …"  ← EDIT TARGET (one sentence inserted)
  - L188   "{application_class}{GS}{window_title}" format example — DO NOT TOUCH (accurate)
  - L362   macOS "app name only" note — DO NOT TOUCH (accurate)
src/platforms/hyprland.rs      # READ ONLY (evidence: app_class = initial_class)
src/platforms/x11.rs           # READ ONLY (evidence: parse_wm_class returns the class, 2nd field)
src/platforms/macos.rs         # READ ONLY (evidence: localizedName — not in the note)
src/platforms/windows.rs       # READ ONLY (evidence: Win32 class — not in the note)
README.md                      # READ ONLY (out of scope — S2)
docs/configuration.md          # READ ONLY (out of scope — S2)
docs/qmk-integration.md        # READ ONLY (out of scope — S2)
```

### Desired Codebase tree
**Only `docs/troubleshooting.md` changes** — one sentence inserted into item #3. No new
files, no code, no front-matter, no config.

### Known Gotchas of our codebase & Library Quirks
```text
# CRITICAL (scope): edit ONLY docs/troubleshooting.md, and within it ONLY checklist
#   item #3 (~L571-577) of the "Device shows connected but rules not applying" section.
#   Do not touch item #1/#2/#4/#5/#6, other sections, the front matter, or any other file.

# CRITICAL (surgical): the contract forbids rewriting the section. Insert exactly ONE
#   sentence between "Show Window Information")." and "A `*chrome*` rule …". Keep the
#   `qmkonnect -v | grep …` fenced block and the *chrome*/Google Chrome example verbatim.

# CRITICAL (markdown integrity): the ```bash fence inside item #3 must remain balanced
#   (open + close). The list item is indented 3 spaces — the inserted lines must also be
#   indented 3 spaces so they stay inside the numbered item (not a new block). Do not
#   introduce a blank line inside the item (it would break the list continuation).

# GOTCHA (wrapping): this is a Jekyll/GitHub-Pages site with NO markdown linter. Match
#   the surrounding ~75-80 col prose wrapping by hand. The sample newText above is
#   pre-wrapped to fit.

# GOTCHA (no build validation): there is no docs build step in the dev loop
#   (AGENTS.md macOS/Windows loops are cargo + packaging). "Validation" here is manual:
#   read the rendered item, check the fence/indentation, and git diff scope. Do NOT run
#   cargo or packaging for this task.

# GOTCHA (em-dash): the surrounding prose uses "—" (U+2014), e.g. "Google Chrome —
#   adjust". The inserted sentence uses the same "—" for consistency. Keep it.

# GOTCHA (code spans): wrap identifiers in backticks exactly as the file does:
#   `initial_class`, `class`, `hyprctl`, `xprop WM_CLASS`. Bold the word **class** where
#   contrasting with "instance" (matches the file's use of **bold** for emphasis).
```

## Implementation Blueprint

### Data models and structure
None. Documentation-only task.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: READ docs/troubleshooting.md L559-592 (the section) and L170-200 (Window Detection)
  - CONFIRM: checklist item #3 is exactly as quoted in this PRP's "What" block.
  - CONFIRM: no other item/section already explains the platform-specific identifier
    (it does not — L188/L362 are generic/macOS-only).
  - This verifies the oldText anchor is unique and unchanged before editing.

Task 2: EDIT docs/troubleshooting.md checklist item #3 — insert ONE sentence
  - USE the edit tool with the EXACT oldText/newText from the "What" block above.
    oldText (unique, spans the L576-577 line):
      "   (or use the tray's \"Show Window Information\"). A `*chrome*` rule won't match a"
    newText: the 6-line block in "What" (inserts the clarifying sentence, keeps the
      trailing " A `*chrome*` rule won't match a").
  - NAMING/TERMS: "initial_class" (Hyprland), "class" (the hyprctl field), "WM_CLASS",
    "class" vs "instance" (X11). All lowercased identifiers in backticks.
  - PLACEMENT: mid-item #3, after "Show Window Information")." and before the example.
  - PRESERVE: the ```bash fence, the grep line, the "Show Window Information" reference,
    the *chrome*/Google Chrome example, and every other item.

Task 3: VERIFY markdown integrity + scope (no edits in this task — checks only)
  - SEE Validation Loop below (all checks are read-only commands).

Task 4: NEVER do these (out of scope / forbidden)
  - DO NOT rewrite the section, the item, or any sentence other than the single insertion.
  - DO NOT edit the front matter, L188 format example, L362 macOS note, or any other section.
  - DO NOT edit README.md, docs/configuration.md, docs/qmk-integration.md (S2 owns these).
  - DO NOT edit any .rs file, Cargo.toml, PRD.md, tasks.json, prd_snapshot.md, or .gitignore.
  - DO NOT run cargo build/test or the packaging scripts — this is a docs-only task with
    no compile/test gate.
  - DO NOT mention macOS/Windows in the note (they match intuition; no confusion trap).
  - DO NOT add a "window class" definition section or restructure the checklist.
```

### Implementation Patterns & Key Details
```text
# PATTERN: surgical single-sentence insertion anchored on a unique line. The anchor
#   "   (or use the tray's \"Show Window Information\"). A `*chrome*` rule won't match a"
#   appears exactly once in the file (confirmed: the only "Show Window Information" +
#   "*chrome*" co-occurrence). The newText keeps the trailing fragment so the example
#   sentence continues uninterrupted.

# PATTERN: contrast-by-bold. The file uses **bold** to stress load-bearing terms
#   (e.g. "**yes**", "**2-element**"). Bold **class** when contrasting with "instance"
#   to make the X11 correction pop — mirrors house style.

# WHY name both Hyprland AND X11: the contract's clarifying-note option explicitly
#   says "Hyprland initial_class vs X11 WM_CLASS class". Naming only one would leave
#   the other confusion trap undocumented. macOS/Windows are intentionally omitted
#   (intuitive; not a trap).

# WHY "trust it over a native tool": the prior fixes' whole point is that the matched
#   identifier is now stable & consistent; the -v/dialog value IS that identifier.
#   Reinforcing authority prevents the user from second-guessing it with hyprctl/xprop.

# ANTI-PATTERN: do not turn this into a per-platform table or a new subsection. The
#   contract says "add precision where a user would be confused" — one sentence, inline.
```

### Integration Points
```yaml
FILES:       only docs/troubleshooting.md.
FRONTMATTER: unchanged (layout/title/permalink).
LINKS:       no links added/removed (the existing Configuration Guide / Examples links
             at the end of the section are untouched).
CODE FENCE:  the ```bash block inside item #3 is preserved verbatim (open+close balanced).
BUILD:       none — no docs build in the dev loop; Jekyll renders on the GitHub Pages site.
DOWNSTREAM:  P1.M2.T3.S2 (README + docs consistency for heuristic/autostart) is separate;
             this task does NOT touch README.md / configuration.md / qmk-integration.md.
```

## Validation Loop

> This is a **docs-only** task. There is no compiler, test runner, or linter in the
> dev loop for `docs/`. All validation is **manual + read-only shell checks**. Do NOT
> run `cargo` or the packaging scripts.

### Level 1: Markdown structure integrity (immediate, read-only)
```bash
cd /home/dustin/projects/qmkonnect
# (a) The ```bash fence inside item #3 is still balanced (expect an EVEN count overall;
#     the inserted prose adds NO fence):
grep -c '```' docs/troubleshooting.md                # expect EVEN number (unchanged parity)
# (b) The new sentence is present and names both backends:
grep -n 'initial_class' docs/troubleshooting.md      # expect ≥1 hit inside item #3
grep -n 'second field of' docs/troubleshooting.md    # expect 1 hit (the X11 clause)
# (c) The existing example + block are intact:
grep -n 'qmkonnect -v | grep -i "window\\|sending"' docs/troubleshooting.md   # expect 1 hit
grep -n "Google Chrome" docs/troubleshooting.md      # expect 1 hit (unchanged example)
grep -n 'Show Window Information' docs/troubleshooting.md  # expect unchanged hits (≥2)
# Expected: all present; no fence parity change.
```

### Level 2: Content accuracy cross-check (read-only — confirm the note matches the code)
```bash
cd /home/dustin/projects/qmkonnect
# Hyprland sends initial_class (the note's claim):
grep -n 'initial_class.clone()' src/platforms/hyprland.rs   # expect several hits (L398/474/479/571/577)
# X11 parser returns the CLASS (2nd field), not the instance (the note's claim):
grep -n 'Prefers the .*class.* (2nd field)' src/platforms/x11.rs   # expect 1 hit (docstring)
grep -n 'fn parse_wm_class_returns_class_not_instance' src/platforms/x11.rs  # expect 1 hit (test)
# Expected: both claims verified against source — the note is accurate.
```

### Level 3: Scope check (read-only)
```bash
cd /home/dustin/projects/qmkonnect
git diff --stat                       # expect ONLY docs/troubleshooting.md
git diff --name-only                  # expect exactly one file
git diff docs/troubleshooting.md | grep -E '^[+-]' | grep -v '^[+-]{3} ' \
  | grep -vE 'initial_class|second field|matched against|trust it over|hyprctl prints|xprop WM_CLASS|QMKonnect uses the window' \
  | grep -E '^\+'                    # expect EMPTY — every added line is part of the one sentence
# The ONLY removed line should be the original fragment we split (kept verbatim in newText).
# Confirm front matter + other sections untouched:
git diff docs/troubleshooting.md | grep -nE '^@@'                # expect ONE hunk around L571-577
```

### Level 4: Prose / rendered review (manual)
```text
# Read the rendered item #3 end-to-end (the "What" block above shows the target). Check:
#  - Reads as one continuous checklist item (no broken list continuation).
#  - The inserted sentence flows: "...Show Window Information\"). That value is exactly
#    what your pattern is matched against, so trust it over a native tool — on Hyprland
#    ... initial_class ... and on X11 it uses the **class** ... A `*chrome*` rule ...".
#  - Em-dashes (—) and backticked identifiers match house style.
#  - No double spaces, no trailing whitespace on inserted lines.
```

## Final Validation Checklist

### Technical Validation
- [ ] Level 1: ```bash fence parity unchanged; both `initial_class` and `second field of`
      present; the `qmkonnect -v` block + `Google Chrome` example intact.
- [ ] Level 2: `initial_class.clone()` in hyprland.rs and the `parse_wm_class … class …
      (2nd field)` docstring/test in x11.rs confirm the note's claims.
- [ ] Level 3: `git diff --stat` shows ONLY `docs/troubleshooting.md`; exactly one hunk
      around L571-577; no added line outside the single inserted sentence.

### Feature Validation
- [ ] Item #3 names BOTH Hyprland `initial_class` and X11 WM_CLASS **class** (2nd field,
      not instance).
- [ ] The note reinforces that the `-v` / "Show Window Information" value is authoritative.
- [ ] macOS and Windows are intentionally NOT mentioned (no confusion trap).
- [ ] The existing example (`*chrome*` vs `Google Chrome`) and the `qmkonnect -v | grep`
      block are unchanged.

### Code Quality Validation
- [ ] Markdown valid for the Jekyll site (balanced fence, 3-space list indentation preserved,
      no stray blank line inside the item).
- [ ] Inserted prose wrapped ~75-80 cols; em-dash + backticks match house style.
- [ ] No rewrite — exactly one sentence added.

### Documentation & Deployment
- [ ] Commit message states the decision and rationale (see below).
- [ ] No other doc/file/section changed.

### Commit message (guidance)
```text
docs(troubleshooting): clarify which window identifier host rules match

After the Hyprland (initial_class) and X11 (WM_CLASS class-field) fixes,
QMKonnect matches a stable, consistent identifier on every backend. A rule
author who cross-references `hyprctl` (shows `class`) or `xprop WM_CLASS`
(shows "instance", "Class") can paste the wrong field. Add one sentence to
the troubleshooting checklist item #3 naming both platform specifics and
reinforcing that the `qmkonnect -v` / "Show Window Information" value is
authoritative. Section not rewritten; example unchanged.

(Decision: add a clarifying note rather than no-change, because the two
identifier bugs fixed in P1.M1.T1.S1/P1.M1.T2.S1 are exactly the native-tool
vs matched-value discrepancy the note resolves.)
```

---

## Anti-Patterns to Avoid
- ❌ Don't rewrite the section, the item, or any sentence other than the single insertion — the contract forbids it ("only add precision where a user would be confused").
- ❌ Don't add a per-platform table or a new "Window class definitions" subsection — one inline sentence is the contract.
- ❌ Don't mention macOS (`localizedName`) or Windows (Win32 class) in the note — they match intuition and are not confusion traps; adding them dilutes the two real traps.
- ❌ Don't change the `qmkonnect -v | grep …` block, the "Show Window Information" reference, or the `*chrome*`/`Google Chrome` example — they stay verbatim.
- ❌ Don't edit README.md, docs/configuration.md, docs/qmk-integration.md, the L188 format example, the L362 macOS note, or any `.rs` file — out of scope (S2 / code tasks).
- ❌ Don't run `cargo build`/`test` or the packaging scripts — this is a docs-only task with no compile/test gate.
- ❌ Don't add a blank line inside item #3 — it would break the numbered-list continuation.
- ❌ Don't unbalance the ```bash fence or change the 3-space indentation of the inserted lines.

---

## Confidence Score: 9/10

The edit is a single pre-written sentence with an exact unique `oldText` anchor and a
fully-specified `newText`, against a file whose relevant content was read in full this
session. The decision (add vs no-change) is justified by direct evidence in the Rust
sources (`hyprland.rs` `initial_class`, `x11.rs` `parse_wm_class` → class/2nd field) and
by the PRD's own Issue-1 title. The score is 9 rather than 10 only because there is no
automated markdown/docs gate in this repo, so structural integrity (fence balance, list
indentation) rests on the manual Level 1/3 checks — which are explicit and cheap. The
remaining residual (whether a reviewer prefers "instance" vs "1st field" phrasing) is
cosmetic and does not affect correctness.
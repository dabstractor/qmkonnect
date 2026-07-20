# Research Notes — P6.M1.T1.S3: Update `docs/examples.md` + `docs/troubleshooting.md` with host-rules content

> **Repo under change:** the **qmkonnect** desktop app at
> `/home/dustin/projects/qmkonnect`. This is a **documentation-only** task
> (item CONTRACT: "DOCS: This IS the documentation subtask"). It edits **TWO**
> Markdown files — `docs/examples.md` (8.0 KB) and `docs/troubleshooting.md`
> (17.3 KB) — adding the host-side-rules (`rules.toml`) **recipes + FAQ entries**.
>
> **The feature is now SHIPPED.** Per the live `plan_status`, **P1–P5 are all
> Complete**: the typed-command transport + firmware, the host pattern matcher,
> the `rules.toml` data model + evaluator, the handshake + send logic, the CLI
> flags (`--list-callbacks` / `--validate-rules` / `--rules-path`), and the tray
> **"Reload rules"** item on **all three platforms** (verified in `src/main.rs`,
> `src/tray.rs`, `src/linux_tray.rs`). So this task documents a real, landed
> feature — the troubleshooting entries cite the **actual shipped CLI messages +
> exit codes** (read from `src/main.rs`, see §0.4), not a spec prediction. There
> are no code-blocker hedges in this task.
>
> **This task is the how-to / troubleshooting layer of the host-rules docs:**
>   - **S1** (`docs/configuration.md`, COMPLETE) owns the **schema REFERENCE** —
>     field table, the verbatim `HOST_RULES.md` §9 example, the CLI flag table,
>     file location, stack/replace field semantics. → LINK to `/configuration`;
>     do NOT duplicate.
>   - **S2** (`docs/qmk-integration.md`, READY) owns the **MIGRATION guide** —
>     the 4-step procedure, the `DEFINE_HOST_CALLBACKS` registry explanation. →
>     LINK to `/qmk-integration`; do NOT duplicate.
>   - **S3 (THIS)** owns the **RECIPES** (a concrete worked `rules.toml` example
>     + a firmware→host before/after) and **the 4 troubleshooting entries** named
>     in the item CONTRACT.
>   - **S4** (`README.md` + `docs/llms_full.txt`, PLANNED) regenerates the
>     concatenation AFTER all doc edits land. → do NOT touch `llms_full.txt`.

---

## §0 — Contracts consumed (all verified present)

### §0.1 The item CONTRACT (verbatim, the source of truth for scope)

```
1. RESEARCH NOTE: docs/examples.md (8.0 KB) has config and keymap examples.
   docs/troubleshooting.md (17.3 KB) has a FAQ and common issues. Both need
   host-rules content.
2. INPUT: rules.toml examples from HOST_RULES.md §9, CLI flags from P5.M1.T1.S1.
3. LOGIC:
   In examples.md: add a complete rules.toml example showing layer rules,
     callback rules, disable_firmware_config, and the stack/replace model.
     Show the before/after migration from DEFINE_* to rules.toml.
   In troubleshooting.md: add entries for
     - 'Legacy firmware (proto_ver != 2) — host rules disabled'
     - 'Callback name not found in registry'
     - 'rules.toml parse error'
     - 'Device shows connected but rules not applying'
4. OUTPUT: Updated docs/examples.md and docs/troubleshooting.md.
5. DOCS: This IS the documentation subtask.
```

### §0.2 The canonical spec sources (READ-ONLY)

- **`spec/HOST_RULES.md` §9** — the canonical `rules.toml` reference EXAMPLE
  (the `alacritty`→224 replace, `*chrome*/*youtube*`→225, `neovide` callback,
  `*chrome*/*claude*` callback-replace block). This is reproduced VERBATIM by S1
  in `configuration.md`. **S3 must NOT copy it** (that would duplicate S1); S3
  writes a DIFFERENT, self-contained recipe (see §3 D2). The §9 marker is
  `alacritty` — it must NOT appear in `examples.md`.
- **`spec/HOST_RULES.md` §4** — the stack-vs-replace coexistence model: stack =
  string-first then `APPLY_HOST_CONTEXT` `clear_board=0`; replace = context-only
  `clear_board=1`; window is replace iff EVERY matched rule's effective
  `disable_firmware_config` is `true` (or board has no rules); no-match ⇒ clear
  host layer + disable all host callbacks. (Source for Example 4's runtime
  walkthrough.)
- **`spec/HOST_RULES.md` §8(2)** — `Pattern::Single(p)` matches **app_class
  only** when the window has a title (firmware parity); `Pattern::Parts(c,t)`
  requires both halves to match. (The matcher semantics the recipes rely on.)
- **`spec/HOST_RULES.md` §10** — the migration procedure (exposed by S2; S3 shows
  a concrete before/after WORKED EXAMPLE and links to `/qmk-integration` for the
  procedure).
- **`/home/dustin/projects/qmk-notifier/README.md`** §"Host-Side Rules & Typed
  Commands" — the canonical firmware README: the `DEFINE_HOST_CALLBACKS` `mute`
  example (`{ "mute", &mute_on, &mute_off }`), the "two independent state planes"
  model, the "Stack vs replace per window" prose, and the backward-compat
  guarantee. S3's recipes mirror this vocabulary.

### §0.3 Canonical callback/app names to reuse in recipes (for page continuity)

From the firmware README + the existing `examples.md` (so recipes feel native to
the page, not invented):

- `mute` / `mute_on` / `mute_off` (firmware README canonical example).
- `vim_lazy`, `disable_vim` (firmware README + §9 reference example).
- `enable_gaming_mode` / `disable_gaming_mode` (the existing `examples.md`
  Example 2 — Gaming & Productivity; also `steam_app*`, `cs2`, `*word*`).

**S3's Example 4 recipe reuses Example 2's gaming/office context** (steam games,
office apps, `enable_gaming_mode`/`disable_gaming_mode`) for page continuity —
the reader just saw those names in Example 2. This is DISTINCT from the §9
alacritty/neovide reference, so it does not duplicate S1.

### §0.4 The ACTUAL shipped CLI behavior — read from `src/main.rs` (P5.M1.T1.S1 LANDED)

The troubleshooting entries cite these EXACT shipped messages + exit codes. These
are authoritative (read from `src/main.rs` `list_callbacks()` L261 + `validate_rules()`
L305), and they MATCH the spec (`HOST_RULES.md` §8(6)):

**`--list-callbacks`** (`fn list_callbacks`, **always returns `Ok(())` ⇒ exit 0**):
| Situation | Exact stdout | Exit |
| --- | --- | --- |
| No device connected | `No QMK device connected. Connect a keyboard with host-rules firmware and re-run.` | 0 |
| Connected, capable, 0 callbacks | `Connected keyboard reports 0 callbacks.` | 0 |
| Connected, capable, N callbacks | `Callback name -> id (N):` then rows `  {id:>3}  {name}` (sorted by id) | 0 |
| Connected, NOT capable (legacy/timeout) | `Legacy firmware (no callback support) — host rules will run in string-only mode.` | 0 |

**`--validate-rules`** (`fn validate_rules`):
| Situation | Exact output (stdout unless noted) | Exit |
| --- | --- | --- |
| `--rules-path <p>` where `p` missing | (stderr) `rules file not found: {p}` | **non-zero** |
| No `--rules-path`, no candidate exists | `No rules.toml found (host rules disabled). Nothing to validate.` | 0 |
| Found a path | `Validating {path}` (then continues) | — |
| Parse / schema error | (stderr) `rules.toml invalid: {e}` | **non-zero** |
| Parse OK + device connected + capable + unknown names | (stderr) `⚠  unknown callback: {name}` per unknown name | **0 (warnings)** |
| Parse OK + device connected + capable + all names known | `All callback names recognized.` | 0 |
| Parse OK + device connected + NOT capable | `Legacy firmware — callback-name validation skipped (schema-only).` | 0 |
| Parse OK + device NOT connected | `Device not connected — callback-name validation skipped (schema-only).` | 0 |
| (final, on success) | `rules.toml valid: {N} layer rules, {M} callback rules.` | 0 |

**Takeaways for the FAQ entries:**
- The "Legacy firmware" entry quotes the EXACT `--list-callbacks` line: `"Legacy
  firmware (no callback support) — host rules will run in string-only mode."` (exit 0).
- The "Callback name not found" entry quotes `⚠  unknown callback: {name}`
  (a WARNING, exit 0 — NOT fatal; `evaluate` skips unknown names silently).
- The "rules.toml parse error" entry quotes `rules.toml invalid: {e}` (exit
  non-zero) and notes that an explicit missing `--rules-path` ALSO fails non-zero.
- The "Device connected but rules not applying" entry uses `--validate-rules` +
  `--list-callbacks` + `qmkonnect -v` as the diagnostic ladder.

### §0.5 The tray "Reload rules" item is on ALL three platforms (P5.M2 LANDED)

Verified by grep:
- **macOS/Windows** — `src/tray.rs`: `MenuItem::new("Reload rules", true, None)`
  at L318, wired in the prefs group (L389-391), handler `do_reload_rules` (L513,
  L554-560). P5.M2.T1.S1 = Complete.
- **Linux SNI** — `src/linux_tray.rs`: `label: "Reload rules".to_string()` at
  L197, spawns `do_reload_rules` (L199, L468). P5.M2.T1.S2 = Complete.

So the troubleshooting advice says "click the tray's **Reload rules** item" with
NO platform hedge — it ships on all three. (Note: rules.toml is NOT re-read on
every focus change; it is re-read on app start, on device (re)connect, and when
"Reload rules" is clicked.)

---

## §1 — Verbatim CURRENT structure of both files (read in full; unchanged)

### §1.1 `docs/examples.md` (8.0 KB, fences balanced at 8 ``` lines)

```
L1   front matter (layout: default, title: Examples, permalink: /examples/)
L7   # Real-World Examples
L9   > blockquote: "These are firmware examples, not desktop-app configuration …"
L17  ## Example 1: Developer Setup        (DEFINE_SERIAL_LAYERS: code/neovim/jetbrains → _CODE, …)
L85  ## Example 2: Gaming & Productivity  (steam_app*/minecraft/cs2 → _GAMING; word/excel → _OFFICE;
                                           DEFINE_SERIAL_COMMANDS: steam→enable_gaming_mode, …)
L162 ## Example 3: Content Creation Setup (premiere/davinci → _VIDEO; obs → _STREAMING; …)
L229   (Example 3's "**Result**: …" line — END OF EXAMPLE 3)
L231 ## Pattern Matching Tips             (Understanding Window Matching; Common Patterns)
L264 ## Testing Your Configuration
L274 ---
L276 ## Next Steps                        (links /troubleshooting + GitHub)
```

**INSERTION POINT (examples.md):** a new `## Example 4: Host-Side Rules (rules.toml)`
section is INSERTED **after L229** (Example 3's `**Result**:` line) and **before
L231** (`## Pattern Matching Tips`). This keeps the numbered examples together;
`## Pattern Matching Tips` then applies to BOTH firmware and host patterns (the
host matcher is a full-parity port of the firmware one — same `*`, `^`, `$`, `WT`,
`+`, classes).

**OPTIONAL existing-content touch (examples.md):** the L9 intro blockquote claims
"QMKonnect only sends the active-window string" — accurate for Examples 1–3
(firmware rules) but not for host rules (Example 4 sends typed commands too).
Append ONE additive line to the blockquote flagging Example 4 as the host-side
alternative. Do NOT rewrite the blockquote.

### §1.2 `docs/troubleshooting.md` (17.3 KB, fences balanced at 42 ``` lines)

H2 skeleton:
```
L7   # Troubleshooting Guide
L11  ## Debugging Tools       (Linux CLI options; Verbose Logging; Debug Mode; View Logs)
L73  ## General Issues          (Won't Start; Keyboard Not Detected; Window Detection Not Working)
L217 ## Platform-Specific Issues (Windows; Linux [udev, systemd, Hyprland]; macOS)
L404 ## Configuration Issues   (Invalid Configuration File; Wrong Keyboard IDs)
L456 ## Performance Issues     (High CPU; Memory Leaks)
L502 ## Communication Issues   (Data Not Reaching Keyboard; Raw HID Issues)
L568 ## Getting Help           (Collecting Debug Info; Where to Get Help; Bug Reports)
L620 ## Next Steps
```

**INSERTION POINT (troubleshooting.md):** a new `## Host Rules Issues` H2 section
is INSERTED **between the end of `## Configuration Issues` (~L455, after
`### Wrong Keyboard IDs` / `#### Using System Tools`)** and **L456
(`## Performance Issues`)**. Rationale: `rules.toml` is a config file, so its FAQ
sits next to `## Configuration Issues`. The section has 4 `### ` entries (one per
CONTRACT item), each following the existing `**Symptoms**` / numbered
`**Solutions**` (or Cause/Diagnose/Fix) / code-block format used elsewhere.

**Existing cross-link slugs in troubleshooting.md:** `/examples`, `/installation`,
`/qmk-integration`. New entries ADD `/configuration` (S1 owns the schema/CLI
reference there). All internal links use `{{ site.baseurl }}/slug`.

### §1.3 Cross-link convention — Jekyll `{{ site.baseurl }}`

`docs/_config.yml`: `baseurl: "/qmkonnect"`, `markdown: kramdown`,
`highlighter: rouge`. Every internal link is `[text]({{ site.baseurl }}/slug)` —
NO trailing `.md`, NO absolute `https` for internal pages, NO leading `/` before
`{{`. Sibling pages: `/configuration` (S1), `/qmk-integration` (S2), `/examples`
(this), `/troubleshooting` (this). Code fences are LABELED (```c / ```toml /
```bash) so rouge highlights them; never bare ```.

---

## §2 — Boundary: what sibling tasks own (do NOT duplicate)

| Task | File | Owns | This task (S3) relation |
| --- | --- | --- | --- |
| P6.M1.T1.S1 (COMPLETE) | `docs/configuration.md` | `rules.toml` schema REFERENCE (field table), the **verbatim §9 example**, CLI flag REFERENCE (table), file location, stack/replace field semantics | LINK to `/configuration`; do NOT paste the field table, the §9 example, or the CLI table. Cite flags by name + link. |
| P6.M1.T1.S2 (READY) | `docs/qmk-integration.md` | host-rules MIGRATION guide (4-step procedure; `DEFINE_HOST_CALLBACKS` registry explanation) | LINK to `/qmk-integration`; do NOT repeat the 4-step procedure. Show ONE worked before/after recipe + link out for the procedure. |
| **P6.M1.T1.S3 (THIS)** | `docs/examples.md` + `docs/troubleshooting.md` | host-rules RECIPES (a worked `rules.toml` example + firmware→host before/after) + 4 troubleshooting entries | — |
| P6.M1.T1.S4 (PLANNED) | `README.md` + `docs/llms_full.txt` | README mention + regenerate the concatenation | `llms_full.txt` is GENERATED; do NOT touch it (S4 regenerates post-merge). |

**This task edits ONLY `docs/examples.md` and `docs/troubleshooting.md`.** No Rust,
no Cargo, no other docs, no spec, no plan files (except this item's own
PRP/research).

---

## §3 — Design decisions (D1-D8)

- **D1 — examples.md: one new numbered example ("Example 4"), inserted after
  Example 3.** Keeps the numbered-examples block contiguous. Example 4 is a
  self-contained worked recipe (scenario → firmware "before" → `rules.toml`
  "after" → what happens at runtime), NOT a schema dump.
- **D2 — the Example 4 recipe is DIFFERENT from the §9 reference (avoid S1
  duplication).** §9 (in configuration.md via S1) uses alacritty/neovide/chrome.
  Example 4 reuses the page's own Example 2 (Gaming & Productivity) context —
  steam games + office apps + `enable_gaming_mode`/`disable_gaming_mode` — so it
  reads as "Example 2 done as host rules" (great continuity) and is byte-distinct
  from §9. Hard gate: `grep -c alacritty examples.md == 0`.
- **D3 — the before/after is a worked MIGRATION EXAMPLE, not the procedure.**
  Show: (a) firmware "before" = the one-time `DEFINE_HOST_CALLBACKS` registry +
  the `DEFINE_SERIAL_LAYERS`/`DEFINE_SERIAL_COMMANDS` rows being moved; (b)
  `rules.toml` "after" = the equivalent host rules; (c) a one-line note to remove
  the migrated rows from the firmware macros (with a LINK to `/qmk-integration`
  for the full 4-step procedure). This is the "show me" companion to S2's
  "how do I".
- **D4 — Example 4 demonstrates BOTH stack and replace concretely.** Include at
  least one `disable_firmware_config = true` rule (replace) and rely on the
  `[host]` default `false` (stack) for the rest. Then a "What happens at runtime"
  paragraph walks 2-3 windows: a STACK window (board rules run + host layer on
  top), a REPLACE window (board rules skipped), and a callback-only window.
- **D5 — troubleshooting.md: one new H2 "## Host Rules Issues" with 4 H3 entries,
  one per CONTRACT item.** Placed between `## Configuration Issues` and
  `## Performance Issues` (host rules ARE config). Each entry follows the file's
  existing format: `**Symptoms**: …` / numbered `**Solutions**:` (or Cause/Fix)
  with ```bash code blocks for the diagnostic commands.
- **D6 — troubleshooting fixes cite the SHIPPED CLI behavior verbatim (§0.4).**
  Quote the exact stdout/stderr strings: the `--list-callbacks` legacy line; the
  `--validate-rules` `rules.toml invalid: {e}` (non-zero) and
  `⚠  unknown callback: {name}` (warning, exit 0) lines. Do NOT paraphrase the
  shipped messages — users grep for them.
- **D7 — cross-link out, don't duplicate.** Every entry links to `/configuration`
  (schema/CLI reference — S1) and/or `/qmk-integration` (migration/firmware — S2).
  Example 4 links to both. Never re-paste the field table, the §9 example, or the
  CLI table.
- **D8 — "Reload rules" is shipped on all platforms.** The tray item exists on
  macOS/Windows (`src/tray.rs`) and Linux SNI (`src/linux_tray.rs`). Say "click
  the tray's **Reload rules** item" with NO platform hedge.

---

## §4 — Gotchas (G1-G8)

- **G1 (this is NOT a code task, AND the feature is shipped).** P1–P5 are all
  Complete — the CLI flags, handshake, and tray "Reload rules" item are landed.
  This task edits two MARKDOWN files; it does NOT compile or run Rust. There are
  no build preconditions to gate on. (The handshake dedup / `has_been_queried`
  nuance in §8(5) is firmware-internal — irrelevant to user-facing docs.)
- **G2 (do NOT paste the §9 example or the field table).** S1 owns those in
  `configuration.md`. Example 4's `rules.toml` recipe must be a DIFFERENT block
  (the gaming/office scenario — §3 D2). Cite schema facts inline (e.g. "`layer`
  must be ≥ 224") with a link to `/configuration` rather than reproducing the table.
  `grep -c alacritty examples.md == 0`.
- **G3 (do NOT repeat the 4-step migration PROCEDURE).** S2 owns it in
  `qmk-integration.md`. Show ONE before/after recipe + link out.
- **G4 (matcher parity vocabulary).** The host matcher is a FULL-PARITY port of
  the firmware `pattern_match.c` (`*`, `^`, `$`, `WT(class,title)`, `+`,
  `\d \D \w \W \s \S \b \B`, `.`). Say so in Example 4 so readers know their
  existing firmware patterns translate directly. A single-string `match` matches
  **app_class only** (when the window has a title); a 2-element array
  `["class", "title"]` matches both (== `WT()`).
- **G5 (host layers are ≥ 224).** Recipes must use layer numbers ≥ 224 (the host
  range); `255` clears. Don't reuse the firmware examples' low layers (0/1/2/3)
  verbatim in `rules.toml` — translate them UP (e.g. firmware `_GAMING = 1` → host
  `layer = 224`). Call this out so the reader doesn't copy `_GAMING` literally.
- **G6 (the stack/replace decision is per-WINDOW, an AND over matched rules).**
  A single non-disabling matched rule makes the whole window STACK. Don't imply
  each rule independently stacks/replaces. The "runtime" walkthrough in Example 4
  must state this.
- **G7 (DEFINE_HOST_CALLBACKS row shape).** `{ name, on_enable, on_disable }`;
  `on_disable` may be `NULL`; `id` = array index. The pattern is NOT a field of
  the registry (the pattern moves to `rules.toml`). Match the canonical `mute`
  form from the firmware README — don't invent a struct.
- **G8 (Jekyll Markdown).** Labeled fences (```c / ```toml / ```bash — rouge);
  internal links `{{ site.baseurl }}/slug`; the page permalinks are `/examples/`
  and `/troubleshooting/` (in front matter — don't touch them). Keep new H2/H3
  headings free of backticks so kramdown anchors are predictable.

---

## §5 — Validation (documentation-appropriate — NO cargo)

```bash
cd /home/dustin/projects/qmkonnect

# 1. Scope gate: ONLY the two files changed.
git diff --stat          # expect docs/examples.md AND docs/troubleshooting.md ONLY

# 2. examples.md: the recipe + before/after landed.
grep -c 'rules.toml' docs/examples.md                      # >= 5
grep -n 'Example 4\|Host-Side Rules' docs/examples.md      # the new section
grep -n 'disable_firmware_config' docs/examples.md         # the stack/replace flag (both values)
grep -ni 'stack\|replace' docs/examples.md                 # the runtime walkthrough
grep -n 'DEFINE_HOST_CALLBACKS' docs/examples.md           # the firmware "before"

# 3. troubleshooting.md: the 4 entries landed.
grep -n 'Host Rules Issues\|Legacy firmware\|Callback name not found\|parse error\|rules not applying' docs/troubleshooting.md
grep -n '\-\-list-callbacks\|--validate-rules' docs/troubleshooting.md   # diagnostic commands

# 4. Cross-links land and are well-formed.
grep -oE '\{\{ site\.baseurl \}\}/[a-z-]+' docs/examples.md | sort -u        # must include /configuration + /qmk-integration
grep -oE '\{\{ site\.baseurl \}\}/[a-z-]+' docs/troubleshooting.md | sort -u # must include /configuration

# 5. Markdown sanity: balanced code fences in BOTH files (even count of ``` lines).
test $(( $(grep -c '^```' docs/examples.md) % 2 )) -eq 0 && echo "examples fences OK" || echo "examples UNBALANCED"
test $(( $(grep -c '^```' docs/troubleshooting.md) % 2 )) -eq 0 && echo "troubleshooting fences OK" || echo "troubleshooting UNBALANCED"

# 6. No sibling duplication: the §9 alacritty/neovide reference is NOT in examples.md
#    (it lives in configuration.md / S1); the 4-step procedure is NOT pasted (S2).
grep -c 'alacritty' docs/examples.md          # EXPECT 0 (the §9 marker) — recipe uses a different scenario
grep -c 'DEFINE_SERIAL_LAYERS\|DEFINE_SERIAL_COMMANDS' docs/examples.md  # the "before" firmware block is fine

# 7. (Optional, if ruby/bundle present) Jekyll build — catches Liquid/fence errors.
cd docs && bundle exec jekyll build 2>&1 | grep -iE 'error|warning' || echo "jekyll build clean"
```

The authoritative correctness checks are **#2 + #3** (the recipe + 4 FAQ entries
landed and cite the shipped CLI behavior from §0.4) and **#4** (cross-links point
at the real sibling pages). A human read of the rendered sections against §0.2 +
§0.4 is the final gate.
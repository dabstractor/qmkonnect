# Research Notes — P6.M1.T1.S2: Update `docs/qmk-integration.md` with host-rules migration guide

**Task**: DOCUMENTATION-ONLY. Edit **ONE** file — `docs/qmk-integration.md`
(192 lines / 6.6 KB). Add a new H2 section (the host-rules **migration guide**)
documenting: (1) the three-repo architecture, (2) `DEFINE_HOST_CALLBACKS`
(one-time firmware change), (3) migration from `DEFINE_SERIAL_LAYERS`/
`DEFINE_SERIAL_COMMANDS` to `rules.toml` (incremental, optional), (4) the
stack-vs-replace model, (5) a cross-link to `docs/configuration.md` for the
full `rules.toml` schema + CLI flags.

**Status note (re-run)**: P1–P5 are now **Complete** — the code is **landed**
and verified: `src/core/rules.rs` (39 KB, P3.M1) and `src/core/pattern.rs`
(220 KB, P2.M1) exist, and `rules.rs`'s structs match the spec byte-for-byte
(§0.6 below). S1 (`docs/configuration.md`) is still being **implemented** — the
actual `docs/configuration.md` has NOT yet been edited for `rules.toml`, but the
S1 PRP is a hard **contract** (it explicitly delegates the migration guide to
THIS task). Treat the S1 PRP as the source of truth for the schema layer; link
out to it, don't duplicate it.

**Boundary with siblings** (do NOT duplicate):
- **S1** (configuration.md) owns the `rules.toml` **reference** (field table,
  verbatim §9 annotated example, per-OS file-location table, CLI flags,
  `-c` seeding, the deep `disable_firmware_config` field semantics). S1 links
  OUT to `/qmk-integration` for "the firmware side (`DEFINE_HOST_CALLBACKS`
  registry + migrating `DEFINE_*` rules)" — THIS task is the target of that link.
- **S3** (examples.md + troubleshooting.md) owns `rules.toml` **recipes** +
  troubleshooting recipes.
- **S4** (README.md + llms_full.txt) owns the repo README mention +
  **regeneration** of `docs/llms_full.txt`.

---

## §0 — Authoritative sources (single source of truth)

### §0.1 The migration contract — HOST_RULES.md §10 (verbatim)

```text
## 10. Migration from `DEFINE_*`

Board rules keep working, so migration is **incremental and optional**:

1. **Expose callbacks by name** (one-time firmware change): add
   `DEFINE_HOST_CALLBACKS({ … })` listing the functions you already use in
   `DEFINE_SERIAL_COMMANDS`. Reflash once.
2. **Move a layer rule to the host:** add a `[[layer_rules]]` entry to
   `rules.toml`; **remove** it from `DEFINE_SERIAL_LAYERS` to avoid the same
   layer being driven by both trackers (harmless but confusing). No reflash
   needed for future edits.
3. **Move a callback rule to the host:** add a `[[callback_rules]]` entry;
   **remove** it from `DEFINE_SERIAL_COMMANDS` (callbacks are additive — if kept
   in both, the same `on_enable` would fire twice).
4. Iterate by editing `rules.toml` + "Reload rules" — no reflashing.
```

This is the spine of the new "Migration" subsection. Adapt it for a user
audience (not spec-internal), keep the 4 steps and the accurate "why remove"
rationale (layers: same layer driven by both trackers = harmless but confusing;
callbacks: additive → `on_enable` fires twice).

### §0.2 `DEFINE_HOST_CALLBACKS` — the one-time firmware change

**Struct** (HOST_RULES.md §6 + firmware `PRD.md` §14, canonical):

```c
typedef void (*callback_t)(void);
typedef struct {
    const char  *name;
    callback_t   on_enable;
    callback_t   on_disable;   // may be NULL
} host_callback_t;
host_callback_t* get_host_callbacks(void);
size_t           get_host_callbacks_size(void);
```

**Concrete user-facing example** (from the firmware repo's own README at
`/home/dustin/projects/qmk-notifier/README.md` lines 320–338 — the canonical,
tested, real-world form; **verified unchanged** in this re-run):

```c
static void mute_on(void)  { /* unmute / show mute OSD */ }
static void mute_off(void) { /* restore */ }

DEFINE_HOST_CALLBACKS({
    { "mute", &mute_on, &mute_off },
});
```

- Each row is `{ name, on_enable, on_disable }`; `on_disable` may be `NULL`.
- The `id` is the **array index**, stable per build. At connect the host sweeps
  `QUERY_CALLBACK(i)` to build a `name → id` map, so renumbering across flashes
  is harmless (the host re-queries on every reconnect).
- Omit the macro entirely ⇒ the module provides weak `{NULL, 0}` accessors ⇒
  `callback_count == 0` ⇒ the feature bit is clear ⇒ **byte-for-byte identical
  to today** (no `#ifdef`; structural). This is the backward-compat guarantee.
- Host rules need **no `rules.mk` change** — define the macro in `keymap.c`
  (or any file `#include`-d from it), exactly like `DEFINE_SERIAL_COMMANDS`.
- **Needs `proto_ver == 2` firmware** (the typed-command capability,
  advertised via `QUERY_INFO`'s `feature_flags & 0x01`).

**Mapping to the existing `DEFINE_SERIAL_COMMANDS`** (the migration move): the
SAME `on_enable`/`on_disable` C functions already passed to
`DEFINE_SERIAL_COMMANDS` are listed **by name** in `DEFINE_HOST_CALLBACKS`. So
e.g. the existing firmware rule `{ WT("steam_app*", "*"), &disable_vim }` keeps
its `&disable_vim` function — you add `{ "disable_vim", &disable_vim, NULL }`
to the registry, then move the *pattern* (`steam_app*`) to a
`[[callback_rules]]` entry in `rules.toml`.

### §0.3 The three-repo architecture (HOST_RULES.md §2 table + firmware README "Companion Projects")

| Repo | Form | Role | Host-rules change |
| --- | --- | --- | --- |
| **`qmkonnect`** (this repo) | Rust desktop daemon | Detects the foreground window; owns `rules.toml`, the host matcher, the handshake, the send sequencing | `rules.toml` + host matcher + handshake + CLI/tray |
| **`qmk_notifier`** *(underscore)* | Rust transport crate QMKonnect links | Wire framing: `0x81 0x9F` header, 32-byte chunking, `0x03` ETX, response read | Typed-command framing + `CommandResponse` parsing |
| **`qmk-notifier`** *(hyphen)* | C firmware module in your keymap | On-keyboard receiver + matcher + actor | Registry (`DEFINE_HOST_CALLBACKS`), `host_layer` tracker, typed dispatch |

URLs (use these exact external links):
- Firmware: `https://github.com/dabstractor/qmk-notifier`
- Crate: `https://github.com/dabstractor/qmk_notifier`
- App (this repo): `https://github.com/dabstractor/qmkonnect`

**Naming hazard** (firmware README calls this out explicitly): `qmk-notifier`
(hyphen) = the firmware C module; `qmk_notifier` (underscore) = the Rust
transport crate. The two halves talk over the fixed wire protocol. Worth a
one-line note in the new section.

### §0.4 The stack-vs-replace model (firmware README "Stack vs replace per window" + HOST_RULES.md §4)

Two independent state planes on the firmware:
- **Board state** — `activated_layer`, current command, `current_os` (the legacy
  string path; `DEFINE_SERIAL_*` / `DEFINE_*_OS`). Everything the existing doc
  describes.
- **Host state** — `host_layer` (independent, reserved **≥ 224** so it resolves
  above board layers; `0xFF`/255 clears it) + host-callback enable set. Driven
  by typed commands; defined via `DEFINE_HOST_CALLBACKS`.

The two planes touch **only at two seams**: the `clear_board` flag (an explicit
board teardown inside `APPLY_HOST_CONTEXT`) and `SET_OS` (shared `current_os`).
Otherwise orthogonal.

**Per-window decision (host-chosen via `clear_board`):**
- **Stack** (`clear_board = 0`): host sends the legacy **string first** (board
  runs its rules → sets its layer/command → replies), then
  `APPLY_HOST_CONTEXT{layer, callbacks, clear_board=0}`. Board layer/command
  stay active; host layer stacks above; board callbacks fire first, host
  callbacks after.
- **Replace** (`clear_board = 1`): host sends **only**
  `APPLY_HOST_CONTEXT{…, clear_board=1}` (no string). Firmware
  `deactivate_layer()`s its board layer + `disable_command()`s its board
  command, then applies host layer + callbacks. Board rules are inert for that
  window and re-engage on the host's next string send.
- The host computes `clear_board` from its per-rule `disable_firmware_config`
  flag in `rules.toml`: a window is *replace* iff **every** matched rule is
  disabling (or the board has no rules). One non-disabling matched rule ⇒ stack.

**Capability gate.** Host rules require firmware advertising `proto_ver == 2`
**and** the `0x01` (`APPLY_HOST_CONTEXT`) feature flag (from `QUERY_INFO`).
Legacy firmware (`proto_ver == 1`) or a disconnected keyboard ⇒ the host
silently falls back to string-only; the board's existing `DEFINE_*` rules keep
working unchanged. This is the key "nothing breaks if you don't opt in" promise.

### §0.5 The handshake (one-liner for context — NOT to be deeply documented here)

At (re)connect: `QUERY_INFO` → if `response[0]==0x51` & `proto_ver==2` &
`flags & 0x01` → `QUERY_CALLBACK(i)` sweep → `name→id` map → `SET_OS` +
`APPLY_HOST_CONTEXT`. Else (legacy/timeout) ⇒ string-only. Runs at most once
per board boot (`has_been_queried` guard). Mention only as "QMKonnect detects
this automatically at connect"; the deep mechanics belong in
troubleshooting/spec, not the migration guide.

### §0.6 The LANDED rules.toml schema (P3.M1 — verified against src/core/rules.rs)

The code is now landed (`src/core/rules.rs`, 39 KB). The struct shapes (verified
in this re-run via grep) confirm the schema the migration guide's cross-link
points at:

```rust
pub struct RuleSet {                       // #[serde(default)] host + two Vecs
    #[serde(default)] pub host: HostDefaults,
    #[serde(default, rename = "layer_rules")]    pub layer_rules: Vec<LayerRule>,
    #[serde(default, rename = "callback_rules")] pub callback_rules: Vec<CallbackRule>,
}
pub struct HostDefaults {
    #[serde(default)] pub disable_firmware_config: bool,   // default false (stack)
}
pub struct LayerRule {
    #[serde(rename = "match")] pub pattern: Pattern,       // TOML key is `match`
    pub layer: u8,                                          // REQUIRED, host range >= 224
    #[serde(default)] pub case_sensitive: bool,
    #[serde(default)] pub disable_firmware_config: Option<bool>,  // None => inherit [host]
}
pub struct CallbackRule {
    #[serde(rename = "match")] pub pattern: Pattern,
    #[serde(default)] pub enable: Vec<String>,
    #[serde(default)] pub disable: Vec<String>,
    #[serde(default)] pub case_sensitive: bool,
    #[serde(default)] pub disable_firmware_config: Option<bool>,
}
pub enum Pattern { Single(String), Parts(String, String) }  // "foo" | ["cls","ttl"]
```

The migration guide **references** this schema via a cross-link to
`docs/configuration.md` (S1 owns the field table); it does **not** reproduce it.
The one illustrative `[[callback_rules]]` snippet in the migration section uses
the TOML key `match` and a 2-element array form (`["steam_app*", "*"]`), which
matches `Pattern::Parts` / `WT(class, title)`.

---

## §1 — The target file (current state, line-accurate — verified Jul 20)

`docs/qmk-integration.md`, 192 lines. Jekyll page (front matter
`layout: default`, `title: QMK Integration`, `permalink: /qmk-integration/`).
**Every** internal link uses `{{ site.baseurl }}/slug` (see §1.2). Code fences
use ```c, ```make, ```bash. The page is the "how to wire the firmware module
into your keymap" guide.

### §1.1 Current H2/H3 skeleton (verified via grep)

```
L1-5    front matter (--- layout/title/permalink ---)
L7      # QMK Integration Guide
L13     > "This step is required…" blockquote + upstream-README link
L18     ## Overview
          (3-step data flow; "companion projects — you need both")
L34     ## Integration Steps
L36       ### Step 1: Add qmk-notifier as a submodule to your keymap
L45       ### Step 2: Include the module in your `rules.mk`
L68       ### Step 3: Include the module in your `keymap.c`
L83       ### Step 4: Define layer and command rules   ← DEFINE_SERIAL_* (the migration SOURCE)
L111      ### Step 5: Build and flash
L121    ## About `RAW_USAGE_PAGE` / `RAW_USAGE_ID`
L130    ## Testing Your Integration
L132      ### Basic verification
L141      ### Debugging on the keyboard side
L167    ## Common Issues
L188    ## Next Steps
```

### §1.2 Cross-link + formatting conventions (match these exactly)

- Internal links: `[text]({{ site.baseurl }}/slug)` — NO leading slash before
  `{{`, NO `.md`, NO `https`. Existing slugs in use (from prior `grep`):
  `/configuration`, `/examples`, `/installation`, `/qmk-integration`,
  `/troubleshooting`, `/usage`, `/llms`.
- External links: full `https://github.com/…` (the upstream qmk-notifier
  README + repo links already use this form).
- Code fences: ```c for C, ```make for make, ```bash for shell. Always labeled
  (rouge highlighter); never bare ``` for code.
- The page already links to the upstream
  `[qmk-notifier README](https://github.com/dabstractor/qmk-notifier)` and to
  `[pattern matching syntax](https://github.com/dabstractor/qmk-notifier#pattern-matching-syntax)`.
- Existing `DEFINE_SERIAL_LAYERS` / `DEFINE_SERIAL_COMMANDS` example at Step 4
  (≈L83–L110) is the **migration source** — the new section shows how a row
  there becomes a `rules.toml` entry. Reference it, don't duplicate it.
- Tables: the page has no markdown tables today, but kramdown supports them;
  the architecture table (§0.3) and the migration steps can use tables/lists.

### §1.3 Recommended insertion point

**Insert the new H2 section AFTER the `## About RAW_USAGE_PAGE / RAW_USAGE_ID`
section ends and BEFORE `## Testing Your Integration`** (the line
`## Testing Your Integration` at L130 is the exact "before" anchor). Rationale:
keeps all firmware-setup content together (Overview → Steps → RAW_USAGE_PAGE →
**Host-Side Rules**), then Testing, Common Issues, Next Steps. The migration
guide is an *optional evolution* of the basic setup, so sitting adjacent to
setup is the most discoverable spot.

Alternative acceptable placement: immediately before `## Next Steps` (as an
"advanced" capstone). Either is fine; pick one and keep the surrounding
blank-line spacing consistent with the file.

---

## §2 — Boundary with sibling tasks (do NOT duplicate)

| Sibling | Owns | This task (S2) must |
| --- | --- | --- |
| **P6.M1.T1.S1** (configuration.md) | the `rules.toml` **schema reference** (field table from `src/core/rules.rs`, verbatim §9 annotated example, per-OS file-location table, CLI flags `--list-callbacks`/`--validate-rules`/`--rules-path`, `-c` seeding, the deep `disable_firmware_config` field semantics) | LINK to `{{ site.baseurl }}/configuration` for "the full `rules.toml` schema and CLI flags." Show ONE tiny `rules.toml` snippet only to illustrate a migration step — NOT the full field table or the §9 example. |
| **P6.M1.T1.S3** (examples.md + troubleshooting.md) | `rules.toml` **recipes** + troubleshooting recipes (legacy fallback, validate-rules failures, etc.) | LINK to `{{ site.baseurl }}/examples` and `/troubleshooting`. Don't write recipes. |
| **P6.M1.T1.S4** (README.md + llms_full.txt) | repo README mention + **regeneration** of `docs/llms_full.txt` (a generated concatenation of `docs/*.md`) | Do NOT touch `llms_full.txt`. It will pick up this file's edits when S4 regenerates it post-merge. |

**S1's contract to S2** (verified in the S1 PRP, lines 360-361 & 727-728):
configuration.md links OUT to `/qmk-integration` for "the firmware side
(including the `DEFINE_HOST_CALLBACKS` callback registry and migrating
`DEFINE_*` rules)", and S1 explicitly states "the migration GUIDE is S2
(qmk-integration.md)". So the new heading(s) here MUST be present and
discoverable, and the migration content MUST actually exist, for S1's link to
resolve to real content. S1 is being implemented in parallel — assume it lands
as specified.

**The migration guide's job**: explain the *firmware-side* change
(`DEFINE_HOST_CALLBACKS`) and the *workflow* (move rules from the firmware
macro to `rules.toml`, iterate without reflashing), and the *coexistence
model* (stack/replace). It is the "how and why to migrate" layer; the
"what every field means" layer lives in configuration.md (S1).

---

## §3 — Design decisions (D1–D6)

- **D1 — One new H2 section** ("Host-Side Rules" / "Moving Rules to the Host")
  with 3–4 subsections (architecture / `DEFINE_HOST_CALLBACKS` / migration /
  stack-vs-replace). Keeps it a navigable unit, not scattered.
- **D2 — Tone: user-facing, not spec-internal.** Rewrite §10's "trackers" /
  "additive" language into plain user terms. Keep the *why* (remove from the
  firmware macro so the same layer/callback isn't driven twice), drop the
  *internal mechanism* (`activated_layer`, `host_cb_enabled[]`, id diffs).
- **D3 — One firmware code example, one tiny `rules.toml` snippet.** The
  `DEFINE_HOST_CALLBACKS({ { "mute", &mute_on, &mute_off } })` example (from
  the firmware README, canonical). Then a *single* migrated rule shown as a
  `rules.toml` `[[callback_rules]]` entry to make the "move" concrete — and a
  link to configuration.md for the full schema + the §9 annotated example.
  Do NOT paste the full §9 example (S1 owns it).
- **D4 — Capability/fallback note up front.** One blockquote: host rules need
  `proto_ver == 2` firmware; legacy/disconnected ⇒ silent string-only fallback,
  board rules unchanged. Sets expectations before the user reads the steps.
- **D5 — Stack-vs-replace as plain prose**, not the ASCII flowchart from §4.
  Two bullets (stack / replace) + the "one non-disabling rule ⇒ stack" rule of
  thumb. Link to configuration.md for the field-level detail (S1 owns the
  `disable_firmware_config` field table).
- **D6 — Cross-links, not duplication.** Every "for the full schema /
  CLI flags / recipes / troubleshooting" need → `{{ site.baseurl }}` link.

## §4 — Gotchas (G1–G7)

- **G1 — NOT a code task.** P1–P5 are now landed (Complete), so there are no
  code blockers at all. The only thing not-yet-landed is the *documentation*
  (S1 configuration.md is still being implemented). The docs describe the
  locked design regardless; do NOT gate this doc on S1 landing.
- **G2 — Do NOT duplicate the `rules.toml` schema field table or the §9
  annotated example.** Those are S1's (configuration.md). Link out. A single
  illustrative `[[callback_rules]]` snippet is fine; the full table/example is
  not.
- **G3 — Do NOT touch `llms_full.txt`.** It is a GENERATED concatenation; S4
  regenerates it post-merge.
- **G4 — External links must be the real repos.** `qmk-notifier` (hyphen,
  firmware) vs `qmk_notifier` (underscore, crate). The existing page already
  uses the hyphen form for the firmware README link — stay consistent. If you
  link the crate, use the underscore URL.
- **G5 — `DEFINE_HOST_CALLBACKS` example must match the canonical firmware form.**
  Each row `{ name, on_enable, on_disable }`; `on_disable` may be `NULL`; the
  `id` is the array index. Use the `mute` example from the firmware README (it
  is the tested, documented one) — do not invent a different callback shape.
  The pattern is NOT a field of the host-callback registry (it moves to
  rules.toml).
- **G6 — Keep the migration's "remove from the firmware macro" rationale
  accurate.** Layer rules: keeping in both = same layer driven by two trackers
  (harmless but confusing). Callback rules: callbacks are additive = same
  `on_enable` fires twice. This is the *reason* to remove, and it's easy to
  misstate (don't say "the board rule stops working" — it doesn't; it keeps
  working and conflicts).
- **G7 — Heading-anchor discoverability.** S1's configuration.md links to
  `/qmk-integration` (the page). The section heading should be plain enough
  that its kramdown anchor is predictable. Prefer `## Host-Side Rules` /
  `## Moving Window Rules to the Desktop` over heading text full of backticks
  and parens. (Same-page anchors within this page are optional — none required
  by the contract.)

## §5 — Validation commands (Markdown — NO cargo)

```bash
cd /home/dustin/projects/qmkonnect
git diff --stat                 # EXPECT: docs/qmk-integration.md ONLY
grep -c 'DEFINE_HOST_CALLBACKS' docs/qmk-integration.md   # >= 1 (the example)
grep -c 'rules.toml' docs/qmk-integration.md              # >= 3 (mentions)
grep -n 'stack\|replace' docs/qmk-integration.md          # the coexistence subsection
grep -n 'site.baseurl.*/configuration' docs/qmk-integration.md  # cross-link present
test $(( $(grep -c '^```' docs/qmk-integration.md) % 2 )) -eq 0 \
  && echo "fences balanced" || echo "UNBALANCED FENCES"
# (Optional) render: cd docs && bundle exec jekyll build 2>&1 | grep -iE 'error|warning'
```

No `cargo`, no Rust, no tests — this is a single Markdown file edit. The only
"runtime" check is a Jekyll build (optional; GitHub Pages renders on push).
# Research Notes — P6.M1.T1.S1: Update `docs/configuration.md` with `rules.toml` schema + CLI flags

> **Repo under change:** the **qmkonnect** desktop app at
> `/home/dustin/projects/qmkonnect`. This is a **documentation-only** task: it
> edits ONE Markdown file — `docs/configuration.md` (341 lines / 10.8 KB today,
> UNCHANGED since the host-rules feature landed) — to add the host-side-rules
> (`rules.toml`) schema reference, the `disable_firmware_config` stack/replace
> semantics, the file-location note, the three new CLI flags
> (`--list-callbacks`, `--validate-rules`, `--rules-path`), and the note that
> `-c`/`--config` now also seeds a commented `rules.toml`. It includes the
> **complete annotated `HOST_RULES.md` §9 example**.
>
> **STATUS CHANGE since the first research pass:** the two build preconditions
> that previously blocked the CLI task are now **RESOLVED** — P1.M1.T4.S1 (the
> `qmk_notifier` v0.3.0 tag) and P4.M2.T1.S1 (the capability handshake) are both
> **Complete**, and **P5.M1.T1.S1 (the CLI flags) is LANDED in `src/main.rs`**.
> The build passes today (`cargo build --bin qmkonnect` ⇒ EXIT 0). **Every claim
> in this PRP is verified against the LANDED code** (line numbers below), not a
> spec contract. This makes the docs task MORE accurate: the implementer can run
> `cargo run -- --help` / `--validate-rules` to confirm the docs match reality.

---

## §0 — Contracts consumed (all VERIFIED LANDED in HEAD)

### §0.1 `rules.toml` data model — **P3.M1 (LANDED)** — `src/core/rules.rs`

Verified present at these line numbers:
- `pub struct RuleSet` (L68), `pub struct HostDefaults` (L94),
  `pub struct LayerRule` (L130), `pub struct CallbackRule` (L167)
- `pub fn parse_rules(path: &Path) -> Result<RuleSet, Box<dyn Error>>` (L230)
- `pub fn get_rules_paths() -> Vec<PathBuf>` (L248)
- `pub fn evaluate(...)` (L322)

The schema field table in the docs MUST match these structs exactly:

```rust
#[derive(Debug, Deserialize, Default)]
pub struct RuleSet {
    #[serde(default)] pub host: HostDefaults,
    #[serde(default, rename = "layer_rules")]    pub layer_rules: Vec<LayerRule>,
    #[serde(default, rename = "callback_rules")] pub callback_rules: Vec<CallbackRule>,
}
pub struct HostDefaults { #[serde(default)] pub disable_firmware_config: bool }  // default false (manual impl)
pub struct LayerRule {
    #[serde(rename = "match")] pub pattern: Pattern,   // "x" -> Single; ["c","t"] -> Parts
    pub layer: u8,                                     // REQUIRED, >= 224
    #[serde(default)] pub case_sensitive: bool,        // default false
    #[serde(default)] pub disable_firmware_config: Option<bool>,  // None => inherit [host]
}
pub struct CallbackRule {
    #[serde(rename = "match")] pub pattern: Pattern,   // REQUIRED
    #[serde(default)] pub enable: Vec<String>,         // default empty
    #[serde(default)] pub disable: Vec<String>,        // default empty
    #[serde(default)] pub case_sensitive: bool,        // default false
    #[serde(default)] pub disable_firmware_config: Option<bool>,  // None => inherit [host]
}
// Pattern (src/core/pattern.rs): Single(String) | Parts(String,String), #[serde(untagged)]
```

**Required-vs-optional (the strictness `--validate-rules` reports):** `match` and
`layer` have **no** `#[serde(default)]` ⇒ a `[[layer_rules]]` missing either is a
parse error. `enable`/`disable`/`case_sensitive`/`disable_firmware_config` all
default. `disable_firmware_config` is `bool` in `[host]` but `Option<bool>` per
rule (None ⇒ inherits `[host]`). The docs MUST call out which keys are required.

### §0.2 File-location contract — `get_rules_paths()` (LANDED, L248)

`get_rules_paths()` delegates to `platforms::get_config_paths()` and swaps the
filename to `rules.toml` (`PathBuf::with_file_name("rules.toml")`). ⇒ `rules.toml`
lives in the **SAME directory** as `config.toml`. Per OS (HOST_RULES.md §8(1)):

| OS | `config.toml` dir | `rules.toml` path |
| --- | --- | --- |
| Linux | `~/.config/qmk-notifier/` (+ `$XDG_CONFIG_HOME`, `/etc/qmk-notifier/`) | `<same dir>/rules.toml` |
| Windows | `%APPDATA%\QMKonnect\` | `%APPDATA%\QMKonnect\rules.toml` |
| macOS | `~/Library/Application Support/QMKonnect/` | `<same dir>/rules.toml` |

The current `docs/configuration.md` only names the Linux path explicitly. The new
section must state "same directory as `config.toml`, swapped filename" + the
per-OS table, and note that an **absent** `rules.toml` ⇒ host rules disabled
(string-only; not an error).

### §0.3 CLI flags — **P5.M1.T1.S1 (LANDED in `src/main.rs`)** — VERIFIED output strings

All three flags + the `-c` extension are implemented and the build passes. The
docs MUST reproduce the ACTUAL behavior (verified by reading `main.rs`):

**Dispatch (main.rs L125-138):** `--list-callbacks` ⇒ `list_callbacks(verbose)`;
`--validate-rules` ⇒ `validate_rules(rules_path, verbose)` where `rules_path` =
`parse_value_flag(&args, "--rules-path")`. `--rules-path` alone is a no-op
(consumed only inside `--validate-rules`).

**`--list-callbacks`** (`fn list_callbacks`, L257-291) — exact output:
- No device ⇒ `No QMK device connected. Connect a keyboard with host-rules firmware and re-run.` (exit 0)
- `perform_handshake(verbose)`, then:
  - capable + empty map ⇒ `Connected keyboard reports 0 callbacks.`
  - capable + non-empty ⇒ header `Callback name -> id (N):` then rows `  {id:>3}  {name}` sorted by id
  - not capable ⇒ `Legacy firmware (no callback support) — host rules will run in string-only mode.`

**`--validate-rules`** (`fn validate_rules`, L292-364) — exact output + exit codes:
- `--rules-path <p>` where `p` missing ⇒ stderr `rules file not found: {p}` + **exit 1**
- no `--rules-path`, no candidate found ⇒ `No rules.toml found (host rules disabled). Nothing to validate.` + **exit 0**
- `Validating {path}`
- `parse_rules` Err ⇒ stderr `rules.toml invalid: {e}` + **exit 1** (the fatal case)
- device connected + capable ⇒ warns `⚠  unknown callback: {name}` (stderr) per unknown name, else `All callback names recognized.` (exit 0)
- device connected + legacy ⇒ `Legacy firmware — callback-name validation skipped (schema-only).`
- no device ⇒ `Device not connected — callback-name validation skipped (schema-only).`
- success footer ⇒ `rules.toml valid: {N} layer rules, {M} callback rules.` + **exit 0**

**`--rules-path <path>`** — override the `rules.toml` location; meaningful only
with `--validate-rules`. Accepts `--rules-path p` and `--rules-path=p`.

**`-c`/`--config`** (`fn create_config`, L440-455) — **now ALSO seeds `rules.toml`**.
After `create_default_config(config.toml)`, it calls
`core::create_default_rules(&config_dir.join("rules.toml"))`. That function
(mod.rs L246-276) is a **no-op + message** (`rules.toml already exists at: …`) if
the file exists; otherwise it writes `render_rules_body()` and prints
`rules.toml template created at: …` + a two-line hint pointing at `--validate-rules`.

**`print_help()` (L140-170)** — the ACTUAL help text (docs table should mirror):
```
      --list-callbacks   Handshake the keyboard; print its callback name->id table
      --validate-rules   Parse rules.toml; report schema/callback-name errors
          --rules-path <path>  Override the rules.toml location (with --validate-rules)
```
(print_help ALSO gained `--show-window-info [macOS/Windows]` — out of scope for the
rules docs; the existing config.md table omits it by design.)

### §0.4 The seeded template — `render_rules_body()` (LANDED, mod.rs L185-236)

**Critical distinction for the docs:** `qmkonnect -c` seeds a **fully-commented**
template (every active line prefixed `# `), NOT the raw uncommented §9 example.
The template's header is `# QMKonnect Host Rules (rules.toml)` and its body is the
§9 schema with `# ` prefixes. A seeded file therefore parses to an all-default
`RuleSet` (host rules disabled) — verified by mod.rs test
`test_render_rules_body_parses_to_default_ruleset`.

**Docs implication:** the docs should show the **uncommented §9 example** as the
*reference* (what an active `rules.toml` looks like), AND note that `qmkonnect -c`
creates the *commented* version of the same schema (so a fresh install is inert
until the user uncomments). Both are the §9 schema; one is active, one is a
scaffold.

### §0.5 The §9 annotated example — VERBATIM (the reference to embed)

From HOST_RULES.md §9 (= PRD selector `h2.87`). Embed VERBATIM in a ```toml fence
as the "what an active rules.toml looks like" reference:

```toml
# rules.toml — host-side window rules.
# disable_firmware_config chooses, per window, whether the board runs its own
# rules (stack) or is cleared and driven solely by the host (replace). Global
# default under [host]; per-rule override below. Host layers are >= 224.
# Run `qmkonnect --validate-rules` after editing.

[host]
disable_firmware_config = false   # global default: false = stack (board runs), true = replace
# On no match the host layer is always cleared and all host callbacks disabled.

# Layer rules: FIRST match wins. One host layer active at a time (>= 224).
[[layer_rules]]
match = "alacritty"                       # class-only pattern
layer = 224
disable_firmware_config = true           # optional override (default inherits [host])

[[layer_rules]]
match = ["*chrome*", "*youtube*"]         # [class_pattern, title_pattern] (== WT())
layer = 225
case_sensitive = false                    # optional, default false

# Callback rules: ALL matches fire. Names come from the keyboard's registry
# (run `qmkonnect --list-callbacks` to see them). The disable list is an
# explicit-exclusion override; focus-out on_disable fires automatically via the
# desired-set diff.
[[callback_rules]]
match = "neovide"
enable = ["vim_lazy", "disable_vim"]      # run on focus-in
disable = ["vim_lazy"]                    # optional: force-off override

[[callback_rules]]
match = ["*chrome*", "*claude*"]
enable = ["vim_lazy", "disable_vim"]
disable_firmware_config = true           # for this window, skip the string -> board can't match
```

### §0.6 `disable_firmware_config` stack/replace semantics (HOST_RULES.md §4 + §3 C10)

The feature's hardest concept; the docs MUST explain it plainly:
- **`[host] disable_firmware_config`** = global default (default `false`).
- **Per-rule `disable_firmware_config`** (on `[[layer_rules]]`/`[[callback_rules]]`)
  = optional override; absent ⇒ inherits `[host]`. A rule's "effective" flag =
  its override if set, else the `[host]` default.
- **Stack** (effective `false`): the board runs its OWN rules too — the string is
  sent first (board matches it), then the host layer/callbacks apply on top.
- **Replace** (effective `true`): the board is cleared and driven SOLELY by the
  host — no string sent; firmware clears its board layer/command and applies only
  the host layer + host callbacks.
- **Per-window decision:** "replace" iff **every matched rule's** effective flag
  is `true` (or the board has no rules). One non-disabling matched rule ⇒ "stack".
- **No match:** host layer always cleared (`0xFF`/`None`) + all host callbacks
  disabled (`on_no_match = "keep"` withdrawn).
- **Backward compat:** no `rules.toml` ⇒ host rules disabled (string-only, =
  today). Legacy firmware (`proto_ver != 2`/timeout) ⇒ string-only.

---

## §1 — Verbatim CURRENT structure of `docs/configuration.md` (341 lines, UNCHANGED)

```
L1   front matter (layout/title/permalink)
L7   # Configuration Guide
L9   > blockquote: firmware prerequisite; "This guide covers … desktop-side config"
L19  ## Platform-Specific Configuration
L21    ### Windows & macOS - GUI Settings
L40    ### Linux - Configuration File  (path: ~/.config/qmk-notifier/config.toml)
L46      #### Creating the Configuration File   (qmkonnect -c; shows config.toml template)
L78      #### Editing the Configuration
L92      #### Reloading Configuration (Linux Only)  (sudo qmkonnect -r)
L105 ## Finding Your Keyboard IDs
L120 ## Reloading Configuration            (duplicate-ish; sudo qmkonnect -r)
L134   ### Linux Additional Steps           (udevadm / systemctl)
L141 ## Configuration Examples              (Zero / Disambiguate / Tuning)
L173 ## Configuration Reference             (the config.toml keys TABLE)
L213   ### CLI flags                        (the flags TABLE — -c/-r/-l/--list-devices/-v/-h)
L221 ## Validation
L231 ## Troubleshooting
L248 ---
L249 ## QMK Firmware Configuration          (DEFINE_SERIAL_LAYERS / DEFINE_SERIAL_COMMANDS — firmware-side)
L285 ## Framework Elements
L300 ## Pattern Matching Examples
L325 ---
L326 ## Next Steps
```

### §1.1 The CURRENT "### CLI flags" table (verbatim, L215-219) — to be EXTENDED

```markdown
### CLI flags

| Flag | Description |
| --- | --- |
| `-c`, `--config` | Create a default (commented-out) configuration file. |
| `-r`, `--reload` | Re-read the config and write the matching udev rule (Linux; requires root). |
| `-l`, `--list` | List the platforms supported by this build. |
| `--list-devices` | List connected HID devices (VID/PID discovery). |
| `-v`, `--verbose` | Enable verbose logging. |
| `-h`, `--help` | Show help. |
```

### §1.2 Cross-link convention — Jekyll `{{ site.baseurl }}`

Internal links use Liquid `[text]({{ site.baseurl }}/page-slug)`. `baseurl` is
`/qmkonnect` (docs/_config.yml). Sibling pages to cross-link: `/qmk-integration`
(firmware + migration — P6.M1.T1.S2), `/examples` (recipes — P6.M1.T1.S3),
`/troubleshooting` (P6.M1.T1.S3), `/usage` (the tray "Reload rules" UX lives
there-ish). New host-rules content links OUT to these; it does NOT duplicate them.

---

## §2 — Boundary: what sibling tasks own (do NOT duplicate)

| Task | File | Owns | This task's relation |
| --- | --- | --- | --- |
| **P6.M1.T1.S1** (THIS) | `docs/configuration.md` | `rules.toml` schema REFERENCE, CLI flags REFERENCE, file location, `disable_firmware_config` semantics, the §9 annotated example, `-c` seeding note | — |
| P6.M1.T1.S2 | `docs/qmk-integration.md` | host-rules MIGRATION guide (firmware `DEFINE_HOST_CALLBACKS`, migration from `DEFINE_*`) | LINK to it |
| P6.M1.T1.S3 | `docs/examples.md` + `docs/troubleshooting.md` | host-rules RECIPES + troubleshooting | LINK to examples |
| P6.M1.T1.S4 | `README.md` + `docs/llms_full.txt` | README mention + regenerate the concatenation | `llms_full.txt` is a GENERATED concat (its header says so); regenerated by S4 AFTER all doc edits land. NOT touched here. |

**This task edits ONLY `docs/configuration.md`.** It does NOT touch
`qmk-integration.md`, `examples.md`, `troubleshooting.md`, `README.md`, or
`llms_full.txt`.

---

## §3 — Design decisions (D1-D7)

- **D1 — New top-level H2, desktop-side.** Add `## Host Window Rules (rules.toml)`
  AFTER the "### CLI flags" / "## Configuration Reference" block and BEFORE the
  first `---` that precedes "## QMK Firmware Configuration". `rules.toml` is a
  DESKTOP file; grouping it with desktop config keeps the page's desktop/firmware split.
- **D2 — Three subsections:** `### File location` (alongside config.toml + per-OS
  table + absent⇒disabled), `### Schema reference` (field table + verbatim §9
  example + "creating it" note), `### Stack vs. replace (disable_firmware_config)`.
- **D3 — CLI table grows in place.** Extend the existing "### CLI flags" table
  with 3 rows + the `-c` annotation. Don't create a second table. Keep the 6
  existing rows verbatim (only `-c`'s text gains a trailing clause).
- **D4 — `-c` seeding noted twice** (CLI table `-c` row + the "Schema reference →
  creating it" subsection), because users discover it from either direction. State
  that `-c` creates a **commented** template (so a fresh install is inert).
- **D5 — Reference example vs seeded scaffold.** The docs show the UNCOMMENTED §9
  example (§0.5) as "what an active rules.toml looks like". A note explains
  `qmkonnect -c` seeds the COMMENTED version of the same schema (§0.4) — uncomment
  + edit to activate.
- **D6 — Cross-links out, not duplication.** Link to `/qmk-integration` (migration)
  and `/examples` (recipes). Don't reproduce them. Optionally mention the tray
  "Reload rules" item (P5.M2.T1.S1/S2 landed) with a link to `/usage` or
  `/troubleshooting`.
- **D7 — §9 example verbatim.** Copy §0.5 byte-for-byte. Don't "improve" the comments.

---

## §4 — Gotchas (G1-G7)

- **G1 (the build PASSES — use it).** `cargo build --bin qmkonnect` ⇒ EXIT 0 today.
  The implementer can RUN `cargo run -- --help`, `cargo run -- --validate-rules`
  (no rules.toml ⇒ exit 0), `cargo run -- -c` (inspect the seeded commented
  template) to confirm the docs match the real output strings. Do this.
- **G2 (field table must match `src/core/rules.rs` EXACTLY).** Three traps:
  (a) `disable_firmware_config` is `bool` in `[host]` but `Option<bool>` per-rule;
  (b) `match` + `layer` are REQUIRED; (c) TOML key is `match` (Rust field `pattern`).
- **G3 (required layer key is `layer`, u8, >= 224).** Not layer_id. 255 = clear.
- **G4 (`match` accepts string OR 2-element array).** String "foo" ⇒ class-only;
  array ["cls","ttl"] ⇒ class+title (== firmware WT()). 1-/3-element ⇒ parse error.
- **G5 (stack/replace decision is per-WINDOW, an AND over matched rules).** One
  non-disabling matched rule ⇒ whole window is "stack".
- **G6 (Markdown + Jekyll).** ```toml fences (rouge). Internal links
  `{{ site.baseurl }}/slug` (NOT https, NOT .md). Permalink is /configuration/.
- **G7 (scope).** Edit ONLY docs/configuration.md. Don't add the migration guide
  (S2), recipes (S3), or regenerate llms_full.txt (S4). LINK out; don't reproduce.
  Don't edit the firmware DEFINE_* half or the config.toml keys table.

---

## §5 — Validation (documentation-appropriate + runnable)

```bash
cd /home/dustin/projects/qmkonnect

# 1. Scope gate: ONLY docs/configuration.md changed.
git diff --stat                                # expect docs/configuration.md ONLY

# 2. Schema/flag/semantics landed verbatim.
grep -c 'rules.toml' docs/configuration.md                       # >= 6
grep -n 'disable_firmware_config' docs/configuration.md          # field table + semantics
grep -n -- '--list-callbacks\|--validate-rules\|--rules-path' docs/configuration.md  # 3 flags
grep -n 'alacritty' docs/configuration.md                        # the §9 example
grep -ni 'stack\|replace' docs/configuration.md                  # semantics subsection

# 3. Balanced code fences (even count of ``` lines).
test $(( $(grep -c '^```' docs/configuration.md) % 2 )) -eq 0 \
  && echo "fences balanced" || echo "UNBALANCED FENCES"

# 4. Internal-link slugs well-formed.
grep -oE '\{\{ site\.baseurl \}\}/[a-z-]+' docs/configuration.md | sort -u

# 5. RUN the real flags to confirm the docs match the landed behavior:
cargo run --bin qmkonnect -- --help | grep -E 'list-callbacks|validate-rules|rules-path'
cargo run --bin qmkonnect -- --validate-rules            # no rules.toml => "Nothing to validate." exit 0
cargo run --bin qmkonnect -- -c && echo "--- seeded template (commented) ---" \
  && cat "$(cargo run --bin qmkonnect -- -c 2>/dev/null; echo ~/.config/qmk-notifier/rules.toml)" 2>/dev/null | head -5
#   (confirm the seeded file's header is "# QMKonnect Host Rules (rules.toml)" and fully commented)

# 6. (Optional) Jekyll build if ruby/bundle present.
cd docs && bundle exec jekyll build 2>&1 | grep -iE 'error|warning' || echo "jekyll build clean"
```
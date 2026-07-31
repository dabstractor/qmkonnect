# Research Notes — P1.M1.T2.S1: rewrite `[[rule]]` schema in `docs/*.md`

Repo: **`/home/dustin/projects/qmkonnect`**. This is the **docs counterpart** to
S3 (which owns `src/` code callers). Clean separation: S3 explicitly defers
`docs/*.md` to P1.M1.T2 (this task). No `src/` edits here; no `llms_full.txt`
(regenerated in S2 — out of scope).

## 1. Source of truth (copy FROM it, never edit it)

`spec/HOST_RULES.md`:
- **§9 (lines 437–528)** = the canonical `[[rule]]` TOML schema + Rust model +
  Validity paragraph. **ALREADY correct / already singular `[[rule]]`.** The
  annotated TOML block (lines 449–481) is the 4-`[[rule]]` example
  (alacritty/layer10, chrome+youtube/layer11, neovide/enable+disable,
  chrome+claude/enable+disable_firmware_config). The divider prose (449–455) is
  the "one [[rule]] per (app × behavior)… a rule MUST set at least one of
  layer/enable/disable… layer is a RAW QMK layer index (no fixed floor)" text.
- **§10 (lines 530–542)** = migration steps. Step 2 = "add a `[[rule]]` entry
  with a `layer` field"; step 3 = "add a `[[rule]]` entry with
  `enable`/`disable`". Quote these verbatim into qmk-integration.md.

## 2. The 5 semantic deltas every edit MUST capture (from the item contract)

(a) Two table-arrays (`[[layer_rules]]` + `[[callback_rules]]`) collapse to ONE
    `[[rule]]` array.
(b) `layer` is now **OPTIONAL** (was **required** in `[[layer_rules]]`).
(c) A rule MUST set **≥1** of `layer`/`enable`/`disable` (one that sets none is a
    parse error) — this replaces the old "layer_rules requires match+layer".
(d) `layer` is a **raw QMK layer index** (C11): no reserved floor, must be `!= 255`
    and `< layer_state_t` width. **No 224/225 floor** (the withdrawn `_GAMING=224`
    guidance — examples.md already says this is withdrawn).
(e) TOML key is `[[rule]]` **SINGULAR** (serde `rename = "rule"`). Never `[[rules]]`.

## 3. Ground-truth stale-token scan (the gap this task closes)

`grep -rnE '\[\[(layer|callback)_rules\]\]' docs/` (current tree, excl. llms_full.txt):

```
docs/configuration.md:271  | `[[layer_rules]]` table-array |        ┐
docs/configuration.md:272  | `[[layer_rules]] match`      |        │  schema
docs/configuration.md:273  | `[[layer_rules]] layer`      |        │  table
docs/configuration.md:274  | `[[layer_rules]] case_sensitive`      │  (11 rows)
docs/configuration.md:275  | `[[layer_rules]] disable_firmware_config` ┘
docs/configuration.md:276  | `[[callback_rules]]` table-array |    ┐
docs/configuration.md:277  | `[[callback_rules]] match`      |     │
docs/configuration.md:278  | `[[callback_rules]] enable`    |     │
docs/configuration.md:279  | `[[callback_rules]] disable`   |     │
docs/configuration.md:280  | `[[callback_rules]] case_sensitive` │
docs/configuration.md:281  | `[[callback_rules]] disable_firmware_config` ┘
docs/configuration.md:309  [[layer_rules]]   ┐ annotated TOML
docs/configuration.md:314  [[layer_rules]]   │ example
docs/configuration.md:323  [[callback_rules]]│ (4 headers)
docs/configuration.md:328  [[callback_rules]]┘
docs/configuration.md:398  - Each `[[layer_rules]]` / `[[callback_rules]]` ... (stack-vs-replace bullet)
docs/examples.md:296  [[layer_rules]]   ┐ Example-4 TOML
docs/examples.md:300  [[layer_rules]]   │ (3 layer +
docs/examples.md:305  [[layer_rules]]   │  2 callback =
docs/examples.md:311  [[callback_rules]]│  5 headers)
docs/examples.md:315  [[callback_rules]]┘
docs/qmk-integration.md:211  step 2 "...add a `[[layer_rules]]` entry..."
docs/qmk-integration.md:216  step 3 "...add a `[[callback_rules]]` entry..."
docs/qmk-integration.md:230  [[callback_rules]]   (migration example)
docs/troubleshooting.md:520  **Fix**: every `[[layer_rules]]` entry requires...
```

Plus one **prose** hit (not a TOML token, but a structural claim to fix):
`docs/configuration.md:265` — "`rules.toml` has one optional table and two
table-arrays:" → "...one table-array:".

Total: **25 TOML token hits across 4 files + 1 prose hit** — configuration.md
accounts for 16 (11 table rows + 4 example headers + 1 bullet), examples.md 5,
qmk-integration.md 3, troubleshooting.md 1. All covered by the edits below.
After the edits the grep (excl. llms_full.txt) returns ZERO.

## 4. Per-site replacement plan (verbatim before → after)

### docs/configuration.md (4 sites)

**Site A — line 265 intro prose:**
- OLD: `` `rules.toml` has one optional table and two table-arrays: ``
- NEW: `` `rules.toml` has one optional table and one table-array: ``

**Site B — lines 271–281 schema table:** collapse the 11 split rows into ONE
`[[rule]]` row-set (7 rows): `[[rule]]` table-array (with the validity note +
first-match/all-match semantics), `match` (required), `layer` (optional, keeps
the full C11 raw-index/layer_state_t/!=255 wording + "Optional — a rule may set
enable/disable only"), `enable`, `disable`, `case_sensitive`,
`disable_firmware_config`. The layer row reuses the existing wording verbatim
(just drops the `[[layer_rules]]` prefix and flips Required `yes`→`no`).

**Site C — lines 309–332 annotated TOML example:** keep the file-header comment +
`[host]` block + the "On no host match" comment (configuration.md-specific);
replace the TWO dividers ("# Layer rules: FIRST match wins…" + "# Callback rules:
ALL matches fire…") + 4 split headers with ONE unified divider (mirror spec §9
lines 449–455, enriched with the "pick one defined in your keymap / above your
highest board layer" guidance so no info is lost) + 4 `[[rule]]` entries (the
spec §9 contents: alacritty/10, chrome+youtube/11, neovide/enable+disable,
chrome+claude/enable+disable_firmware_config).

**Site D — line 398 stack-vs-replace bullet:**
- OLD: `` - Each `[[layer_rules]]` / `[[callback_rules]]` may set an optional ``
  (continues: `` `disable_firmware_config` to override it. ``)
- NEW: `` - Each `[[rule]]` may set an optional `disable_firmware_config` to override it. ``

### docs/examples.md (1 site, lines 294–318)

Rewrite the Example-4 TOML block. Merge the two `steam_app*` rules (a layer rule
+ a callback rule for the same `match`) into ONE `[[rule]]` (layer=10 +
enable=["enable_gaming"]) to demonstrate the unified schema (spec §9: "it may set
layer only, callbacks only, or both"). Result: 4 `[[rule]]` entries (steam_app*
merged, cs2, chrome+youtube replace, *word*). Unify the two comment dividers into
one ("# Rules — layer is first-match-wins; enable/disable accumulate…"). Keep the
`[host]` block + intro prose unchanged (the "255 clear sentinel" / "layer_state
≤15/≤31" prose is still accurate). The "remove from DEFINE_*" trailing prose is
unchanged.

### docs/qmk-integration.md (2 sites)

**Site A — migration steps 2 & 3 (lines 211–217):** mirror spec §10 verbatim:
- step 2: "add a `[[rule]]` entry with a `layer` field to `rules.toml`"
- step 3: "add a `[[rule]]` entry with `enable`/`disable`"
Keep the surrounding "then remove from DEFINE_…" prose unchanged.

**Site B — migration example (lines 230–232):** header swap
`[[callback_rules]]` → `[[rule]]` (the `match`/`enable` rows unchanged).

### docs/troubleshooting.md (1 site, lines 519–522)

The "Fix" sentence:
- OLD: "every `[[layer_rules]]` entry **requires** `match` and `layer` (an entry
  missing either is an error)"
- NEW: "every `[[rule]]` entry **requires** `match` and at least one of `layer` /
  `enable` / `disable` (an entry setting none of those is an error)"
- And flip the `layer` clause to OPTIONAL: "`layer` is **optional** — a raw QMK
  layer index (no reserved range) when set, it must be `<` your `layer_state`
  width … and `!= 255` …". Keep the trailing "See the Configuration Guide"
  sentence unchanged.

## 5. Validation = grep gate (no compilation; this is docs)

```
grep -rnE '\[\[(layer|callback)_rules\]\]' docs/ | grep -v llms_full.txt   # → ZERO
grep -rnE '\[\[rules\]\]' docs/                                             # → ZERO (no plural)
```
Optional TOML-well-formedness check: pipe each new TOML code-fence through a TOML
parser (`python3 -c 'import tomllib,...'`) — the `[[rule]]` examples are valid
TOML (array of tables). Not strictly required (docs examples aren't executed),
but cheap insurance against a malformed fence.

## 6. Scope boundaries / what NOT to touch

- `llms_full.txt` — regenerated by `docs/generate_llms_full.sh` in S2; do NOT
  hand-edit (it's a concatenation of all docs; it'll be regenerated post-edit).
- `src/` — S3 owns it. No code edits in this task.
- `spec/HOST_RULES.md` — READ-ONLY source of truth. Copy FROM it; never edit.
- Prose mentions of "layer rule" / "callback rule" as CONCEPTS (a rule that sets a
  layer vs callbacks) are still valid in the unified schema and may stay — only
  the TOML `[[layer_rules]]`/`[[callback_rules]]` KEYS and the structural
  "two table-arrays"/"requires layer" claims must change. (The code's
  empty_pattern_warnings keeps "layer rule #N"/"callback rule #N" text per S3, so
  docs staying consistent with that terminology is fine.)
- The README.md + top-level docs audit is **P1.M1.T3.S1** (separate task). This
  task is ONLY the 4 files listed.
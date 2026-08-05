# PRP — P4.M2.T1.S1: Audit README.md + top-level overviews; regenerate docs/llms_full.txt

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. **DOCS-only, Mode-B (the final changeset
> sweep)** across **`README.md` + `docs/usage.md`** + a regenerated
> `docs/llms_full.txt`. Removes the last two `0xfeed` literals in the repo's
> source docs (README L232 + L305), reframes README's discovery/VID-PID section
> to the F13/F14 model (discovered-device picker primary; raw VID/PID → Advanced;
> `--list-devices` `kind` column), updates `docs/usage.md`'s one stale boolean
> "connected" status claim to the three-state model, and **regenerates**
> `docs/llms_full.txt` (currently stale: `0xfeed`×6, `No module`=0).
> **Verify (don't rewrite)** the `spec/` companions — they are already at target
> wording.
>
> **INPUT (all Complete):** P1 (three-state status), P2 (R-COEX), P3
> (classify_devices + picker), P4.M1.T1.S1 (0xfeed renderer cleanup +
> `--list-devices` kind column), **P4.M1.T1.S2** (the parallel Mode-A task that
> cleaned `docs/configuration.md` + `docs/installation.md` + `docs/troubleshooting.md`
> — it has ALREADY landed: those 3 docs are now 0xfeed-clean). This task is the
> last doc sweep and depends on all of them.
>
> **SCOPE WALL (critical):** edit ONLY `README.md`, `docs/usage.md`, and run
> `docs/generate_llms_full.sh` (which rewrites `docs/llms_full.txt`). Do NOT
> touch `src/`, `spec/**` (read-only), the parallel task's 3 already-clean docs
> (`docs/configuration.md`/`installation.md`/`troubleshooting.md`),
> `docs/README.md` (the just-the-docs theme readme — NOT concatenated into
> llms_full), `docs/index.md` (version-agnostic one-liners — leave), `PRD.md`,
> `**/tasks.json`, `**/prd_snapshot.md`, `.gitignore`, `Cargo.toml`. See G3/G7.

---

## Goal

**Feature Goal**: Bring the **top-level overviews** (README, the docs home/usage
pages the `llms_full` generator concatenates) into consistency with the shipped
F13 (truthful three-state device status) + F14 (discovered-device picker /
VIA-coexistence) features, eliminate every remaining `0xfeed` literal from the
repo's user-facing source docs, and regenerate `docs/llms_full.txt` so the
single-file agent/LLM reference reflects the current (F13/F14-clean) docs rather
than the stale pre-feature snapshot.

**Deliverable** (additive markdown edits + one regenerated file; no new files):
1. **`README.md`** — 4 edits: **R1** reframe "Windows & macOS" Settings to the
   discovered-device picker (primary) + Advanced raw-VID/PID disclosure (drops the
   `(e.g., feed)`/`(e.g., 0000)` raw-hex framing); **R2** Linux config `0xfeed`/
   `0x0000` → `0x????` + the §7.2 "auto-discover … (recommended)" comment; **R3**
   "Default Configuration" `0xfeed`/`0x0000` → `0x????` + comment; **R4**
   `--list-devices` block notes the Settings picker + the new `kind` column.
2. **`docs/usage.md`** — 1 edit: **U1** "Verify Keyboard Connection" replaces the
   stale boolean "shows as 'connected'" claim with the three-state model (●/⚠/○,
   spec §3 wording).
3. **`docs/llms_full.txt`** — **REGENERATED** via `bash docs/generate_llms_full.sh`
   (run AFTER R1-R4+U1 so no stale `0xfeed` is baked in — G1).
4. **NO other files change.** `spec/**`, `docs/index.md`, `docs/README.md`,
   `docs/examples.md`, `docs/qmk-integration.md`, `src/` are explicitly left as-is
   (documented with grep evidence — see "No-op verifications" in Tasks).

**Success Definition**:
- `grep -rniE '0xfeed' README.md docs/llms_full.txt` → **zero** hits (was 2 in
  README + 6 in llms_full).
- `grep -rniE '0xfeed' README.md docs/*.md | grep -v vendor` → **zero** (all
  concatenated sources clean).
- README's "Windows & macOS" Settings section describes the **discovered-device
  picker** as primary with raw VID/PID under an **Advanced** disclosure; the
  `(e.g., feed)` / `(e.g., 0000)` literals are gone.
- `docs/usage.md` "Verify Keyboard Connection" shows the **three-state** model
  (● Connected / ⚠ "flash qmk_notifier" / ○ Disconnected).
- `docs/llms_full.txt` is freshly regenerated: `No module` / `no qmk_notifier
  module` count **>0** (was 0) and the file mtime/line-count changed.
- `git diff --stat` = `README.md` + `docs/usage.md` + `docs/llms_full.txt` only.
- `spec/` is **unchanged** (verified at target wording; re-grep confirms).

## User Persona (if applicable)

**Target User**: (a) a new user reading the **GitHub README** to install +
configure QMKonnect, and (b) an **AI agent / LLM** consuming `docs/llms_full.txt`
as the canonical single-file reference. Pre-F13/F14 both audiences saw stale
content: a raw-hex `(e.g., feed)` Settings flow that implied `0xFEED` was the
default VID, and a boolean "connected" tray claim that lied green for a VIA-only
board.

**Use Case**: a user with two QMK boards (one qmk_notifier, one VIA-only) reads
the README, opens Settings, and — per the reframed section — sees the
discovered-device list (✓ Dactyl / ✗ Keychron) instead of being told to type raw
hex. An agent reading the regenerated `llms_full.txt` learns the three-state
status and the picker (not the stale boolean + `0xfeed` defaults).

**Pain Points Addressed**: (1) the `0xFEED`-is-the-default misread (README was
the last source doc still carrying `0xfeed`); (2) the false-green "connected"
tray claim in usage.md; (3) `llms_full.txt` lagging the shipped features (it
predates all of F13/F14 — `No module`=0, `0xfeed`=6).

## Why

- **`spec/DEVICE_DISCOVERY.md` §3** mandates the three-state status; **§5** the
  discovered-device picker; **§7.2** the `0xFEED` comment cleanup. The user-facing
  overviews (README, usage) must echo these so docs ≠ reality.
- **F13/F14 shipped** (P1/P2/P3 + P4.M1.T1.S1 Complete). The parallel Mode-A task
  (P4.M1.T1.S2) cleaned the 3 deep-dive docs but **explicitly deferred README +
  the llms_full regen to this task** (its PRP: "README.md 0xfeed @L232-233/L305-308
  is P4.M2.T1.S1"; "regenerating now would bake README's stale 0xfeed in"). This
  task is that deferred work.
- **`docs/generate_llms_full.sh`** concatenates README + 7 docs into the agent
  reference. It must be re-run whenever README/docs change (the script's own
  header says so). It currently embeds 6 stale `0xfeed` lines + zero F13 content.
- **Contract point 4 (OUTPUT):** "README + overviews consistent with F13/F14 …
  regenerated docs/llms_full.txt."

## What

Mechanical, well-bounded markdown edits (4 in README, 1 in usage.md) + one
`bash docs/generate_llms_full.sh` invocation. Every `0xfeed`/`0x0000` literal in
README becomes `0x????` (+ the §7.2 comment for the config.toml examples). The
README Settings section is reframed picker-first (mirroring the parallel task's
`docs/configuration.md` C1). The one stale boolean status claim in usage.md
becomes the three-state model. No structural rewrites, no new files, no spec/
edits.

### Success Criteria
- [ ] **R1** — README "### Windows & macOS": picker-as-primary + Advanced
      disclosure; `(e.g., feed)`/`(e.g., 0000)` gone.
- [ ] **R2** — README Linux config block: `# vendor_id = 0xfeed`/`# product_id =
      0x0000` → `0x????` + §7.2 comment (matches the parallel task's C2/C4 +
      S1's code output — G4).
- [ ] **R3** — README "## Default Configuration" ```toml block: same `0x????`
      cleanup (both lines — G5).
- [ ] **R4** — README `--list-devices` block: notes the Settings picker + the
      `kind` column (`qmk_notifier` / `qmk-only` / `-`).
- [ ] **U1** — `docs/usage.md` "Verify Keyboard Connection": three-state model
      (spec §3 wording verbatim — G8).
- [ ] **REGEN** — `bash docs/generate_llms_full.sh` run from repo root, AFTER
      R1-R4+U1 (G1); `docs/llms_full.txt` rewritten.
- [ ] **GATE 1** — `grep -rniE '0xfeed' README.md docs/llms_full.txt` → zero.
- [ ] **GATE 3** — `git diff --stat` = README.md + docs/usage.md + docs/llms_full.txt.
- [ ] **spec/ unchanged** — `git diff --stat spec/` empty (G3).

## All Needed Context

### Context Completeness Check

_Pass_: an agent with no prior knowledge can implement this using only this PRP,
because: (a) the EXACT current BEFORE text of every README/usage edit site is
reproduced verbatim with line numbers (R1-R4, U1); (b) the EXACT target AFTER
text (mirroring the spec §3 three-state strings + the §7.2 cleanup comment + the
parallel task's already-landed wording) is given in each Task; (c) the full
`0xfeed` surface is enumerated — **only README L232 + L305** remain in source
files (the parallel task's 3 docs are already clean; verified); (d) the
generator's behavior + the regeneration-ordering constraint (G1: README must be
clean BEFORE regen) is pinned; (e) the contract's over-broad verification grep
(`classify_devices|DeviceStatus|No module`) is resolved into the realistic,
authoritative signals (`0xfeed`=0 + `No module`>0; `classify_devices`/
`DeviceStatus` are code symbols absent from user docs — G2, don't chase); (f)
the no-op files (index.md one-liners, docs/README.md theme readme, examples.md,
qmk-integration.md, spec/) are enumerated with grep evidence; (g) 8 gotchas
(G1-G8) + 5 validation gates are pinned.

### Documentation & References

```yaml
# MUST READ — the verbatim research (edit sites + evidence + the gate resolution)
- file: plan/005_8b95ea464bd9/P4M2T1S1/research/notes.md
  why: "§0 = evidence-backed current state (0xfeed surface = README L232+L305 only;
        llms_full stale 0xfeed=6/No-module=0; spec/ at target wording; index.md =
        one-liners; docs/README.md = theme readme). §1 = the generator verbatim.
        §2 = README edit sites verbatim BEFORE→AFTER. §3 = usage.md U1. §4 = G1-G8.
        §5 = validation gates."

# MUST READ — the spec source of truth (mirror §3 + §7.2 verbatim; read-only)
- file: spec/DEVICE_DISCOVERY.md
  why: "§3 = the EXACT three-state tray text to copy into usage.md U1 (and any
        README mention): '● Device Connected', '⚠ QMK board found — no
        qmk_notifier module (flash it)', '○ No Device Connected'. §5 = the picker
        description (✓/✗ markers, name + VID:PID) to mirror in README R1. §7.2 =
        the EXACT 0x???? cleanup comment ('unset: auto-discover any QMK keyboard
        (recommended)')."
  section: "## 3 Device-Status Semantics  +  ## 5 The Discovered-Device Picker
            +  ## 7.2 0xFEED comment cleanup"
  gotcha: "§5.1's example shows a REAL detected 'Dactyl 0xFEED:0x0000' — fine in
           spec/, but do NOT copy that literal into README/docs (re-introduces a
           0xFEED hit). Use 0x???? or board-name-only (G3)."

# MUST READ — the files THIS task edits
- file: README.md
  why: "4 edit sites: R1 '### Windows & macOS' Settings block (steps 1-5,
        ~L209-225 region — the e.g., feed / e.g., 0000 raw-hex framing); R2 Linux
        config ``` block (L232-233); R3 '## Default Configuration' ```toml block
        (L300-308); R4 the '--list-devices' discovery block (~L227-231). NO
        Jekyll front-matter (repo README). The two 'auto-discovers via 0xFF60/0x61'
        one-liners are LEFT (version-agnostic — contract escape hatch)."
  gotcha: "G7: README's config blocks are ``` and ```toml fenced — keep them
           balanced. G5: change BOTH 0xfeed and the adjacent 0x0000."
- file: docs/usage.md
  why: "1 edit site: U1 '### Verify Keyboard Connection' numbered list (L116).
        Has Jekyll front-matter (--- title: Usage … ---) — LEAVE it (G6). The
        'Status and Monitoring' heading (L105) + systemctl status lines are
        process-running checks, NOT device-status — LEAVE."
  gotcha: "U1's three-state bullets must use spec §3 strings verbatim (G8). Keep
           the remaining 2 numbered steps (verify firmware / test apps)."

# MUST READ — the generator (run it AFTER README is clean — G1)
- file: docs/generate_llms_full.sh
  why: "Concatenates README.md + docs/{index,installation,qmk-integration,
        configuration,usage,examples,troubleshooting}.md (8 files), strips a
        leading Jekyll front-matter block, writes docs/llms_full.txt. Run from
        repo root: `bash docs/generate_llms_full.sh`. It cd's to its own dir
        (cwd-agnostic). Prints 'wrote docs/llms_full.txt (N lines, M bytes)'."
  gotcha: "G1: it concatenates README FIRST — running it before cleaning README
           L232+L305 bakes the stale 0xfeed into llms_full. Order: R1-R4+U1 →
           verify README clean → THEN run it."

# MUST READ — the predecessor PRP (a CONTRACT; the 3 docs are already clean)
- file: plan/005_8b95ea464bd9/P4M1T1S2/PRP.md
  why: "Defines the §7.2 cleanup-comment EXACT text and the picker/Advanced
        wording this task mirrors in README (R1/R2/R3). Confirms its scope was
        ONLY docs/configuration.md + installation.md + troubleshooting.md and
        that it EXPLICITLY deferred 'README.md 0xfeed' + 'llms_full regen' to
        P4.M2.T1.S1 (this task). Confirms it has LANDED (the 3 docs are
        0xfeed-clean)."

# Reference — the F13/F14 feature PRDs (what the docs must reflect)
- file: spec/DEVICE_DISCOVERY.md   # §1 two-tier model, §3 three-state, §5 picker, §7 config
- file: PRD.md   # §4 F13/F14 feature table; §2.1 Goal 1 (two-tier discovery)
  why: "F13 = two-tier discovery + truthful three-state status + picker + broadcast;
        F14 = VIA coexistence. The README/usage edits echo these at overview level."
```

### Current Codebase tree (relevant subset)

```bash
README.md                # R1 (Settings reframe), R2 (Linux config 0xfeed), R3 (Default
                           #   Config 0xfeed), R4 (--list-devices kind). NO front-matter.
docs/
  usage.md               # U1 (Verify Keyboard Connection → three-state). Jekyll front-matter.
  llms_full.txt          # GENERATED (concat of README + 7 docs). REGENERATED by this task.
  generate_llms_full.sh  # the generator. RUN by this task (not edited).
  index.md               # version-agnostic one-liners — LEAVE (G3).
  README.md              # the just-the-docs THEME readme — LEAVE (G3; NOT in llms_full).
  configuration.md, installation.md, troubleshooting.md   # ALREADY clean (parallel task) — LEAVE.
  examples.md, qmk-integration.md                         # 0xfeed-clean, not flagged — LEAVE.
spec/                    # DEVICE_DISCOVERY/UI/PROTOCOL/ARCHITECTURE/HOST_RULES — at target
                           # wording; VERIFY, don't rewrite (G3).
src/                     # 0xfeed renderer cleanup = P4.M1.T1.S1 (Complete) — NOT this task.
```

### Desired Codebase tree with files to be changed

```bash
README.md                # MODIFIED — R1 picker reframe; R2/R3 0xfeed→0x???? (§7.2 comment);
                           #   R4 --list-devices kind column.
docs/usage.md            # MODIFIED — U1 three-state Verify Keyboard Connection.
docs/llms_full.txt       # REGENERATED — `bash docs/generate_llms_full.sh` (after R1-R4+U1).
# EVERYTHING else unchanged (spec/, src/, the parallel task's 3 docs, index.md,
# docs/README.md, examples.md, qmk-integration.md, Cargo.toml, PRD, tasks.json).
```

### Known Gotchas of our codebase & Library Quirks

```markdown
<!-- CRITICAL (G1 — regenerate ONLY after README is clean): docs/generate_llms_full.sh
     concatenates README FIRST. If you run it before cleaning README L232+L305,
     the stale 0xfeed is baked into llms_full. Order: R1-R4 + U1 → grep README for
     0xfeed (expect zero) → THEN `bash docs/generate_llms_full.sh`. -->

<!-- CRITICAL (G2 — classify_devices/DeviceStatus will NOT appear in llms_full):
     the contract's verification grep is `classify_devices|DeviceStatus|No module`.
     After regen ONLY `No module` matches (>0). classify_devices/DeviceStatus are
     RUST SYMBOLS in src/ + spec/, ABSENT from all 8 concatenated user-doc files —
     so they stay 0 in llms_full. That is EXPECTED, not a failure. Do NOT add
     internal symbols to user docs to satisfy the grep. Authoritative regen signals:
     `0xfeed`=0 + `No module`>0 + a changed mtime/line-count. -->

<!-- CRITICAL (G3 — don't rewrite spec/ or the deferred docs): spec/* ARE the spec
     this delta implements (verified at target wording — see research/notes §0.4).
     Re-grep to confirm; if somehow stale, FLAG it (don't edit — spec/ is human-
     owned). Also LEAVE: docs/README.md (the just-the-docs theme readme — NOT
     concatenated into llms_full; 0xfeed-clean), docs/index.md (version-agnostic
     one-liners; contract escape hatch), docs/examples.md, docs/qmk-integration.md,
     and the parallel task's 3 already-clean docs. -->

<!-- CRITICAL (G4 — match the §7.2 comment byte-for-byte): README's cleaned
     config lines MUST equal the parallel task's docs/configuration.md C2/C4 AND
     S1's code output:
       # vendor_id  = 0x????   # unset: auto-discover any QMK keyboard (recommended)
       # product_id = 0x????   # unset: auto-discover any QMK keyboard (recommended)
     Note TWO spaces around '=' on the vendor line, ONE on the product line
     (match the existing alignment). Inconsistent docs-vs-code = user confusion. -->

<!-- CRITICAL (G5 — clean BOTH 0xfeed and the adjacent 0x0000): leaving
     `# product_id = 0x0000` next to `# vendor_id = 0x????` keeps half the
     'feed/0000 = default' misread alive. Change both lines in R2 and R3. -->

<!-- GOTCHA (G6 — preserve ``` fences + Jekyll front-matter): README's config
     blocks are ``` and ```toml fenced — keep them balanced (open+close). R1 is
     prose (no fence); R4 keeps the ```bash fence. docs/usage.md has Jekyll
     front-matter (--- title: Usage … ---) — LEAVE it intact. -->

<!-- CRITICAL (G7 — scope wall): edit ONLY README.md, docs/usage.md, and run
     docs/generate_llms_full.sh (rewrites docs/llms_full.txt). Do NOT touch src/,
     spec/**, PRD.md, **/tasks.json, **/prd_snapshot.md, .gitignore, Cargo.toml,
     or any other docs/*.md. -->

<!-- GOTCHA (G8 — three-state wording mirrors spec §3 verbatim): use the EXACT
     strings in U1 (and any README mention): '● Device Connected', '⚠ QMK board
     found — no qmk_notifier module (flash it)', '○ No Device Connected'. Do not
     paraphrase; do not describe platform-specific widgets. -->
```

## Implementation Blueprint

### Data models and structure

_N/A — documentation task. No data models, schemas, or code artifacts. Each edit
is a precise text replacement given verbatim in the Tasks below (use the `edit`
tool with the EXACT `oldText` → `newText`; the `oldText` strings are unique
within their file). After R1-R4 + U1, run the generator; then verify.

### Implementation Tasks (ordered; README first, then usage, then regen, then verify)

```yaml
# ════════════════════════════════════════════════════════════════════
# FILE 1 of 2 (edits): README.md  (R1, R2, R3, R4)
# ════════════════════════════════════════════════════════════════════

Task R1: REFRAME "### Windows & macOS" Settings (picker = primary; VID/PID → Advanced)
  - WHY: this section currently presents raw-hex VID/PID entry as the primary
         Settings flow (steps 3-4: "Enter Vendor ID (e.g., feed)"), which is the
         F13 misread source + pre-F14 framing. spec §5 makes the discovered-device
         list the primary surface; raw hex is an Advanced disclosure.
  - OLD (the numbered list under "### Windows & macOS", in the ## Configuration section):
        1. Right-click the QMKonnect system tray icon
        2. Select "Settings"
        3. Enter your keyboard's Vendor ID (hex format, e.g., feed)
        4. Enter your keyboard's Product ID (hex format, e.g., 0000)
        5. Click OK to save
  - NEW:
        In the common case you never open Settings — QMKonnect auto-discovers a
        single qmk_notifier-capable board. Right-click the tray/menu-bar icon →
        **Settings** only to disambiguate among several boards: the dialog lists
        every connected QMK board by name with its VID:PID and a ✓
        (qmk_notifier-capable) or ✗ (QMK board, no module) marker — pick one and
        QMKonnect writes its VID/PID for you. **Advanced ▸** (rarely needed): the
        raw `vendor_id` / `product_id` hex fields live under a disclosure for
        manually targeting a board that isn't currently on the bus. Changes take
        effect immediately — no restart needed.
  - VERIFY: `grep -n 'e.g., feed\|e.g., 0000' README.md` → zero; the section
            mentions the discovered-device picker + Advanced.

Task R2: Linux config ``` block (L232-233): 0xfeed/0x0000 → 0x???? + §7.2 comment
  - OLD:
        # vendor_id = 0xfeed
        # product_id = 0x0000
  - NEW:
        # vendor_id  = 0x????   # unset: auto-discover any QMK keyboard (recommended)
        # product_id = 0x????   # unset: auto-discover any QMK keyboard (recommended)
  - GOTCHA G4: byte-for-byte the §7.2 comment (TWO spaces around '=' on vendor,
    ONE on product). G5: both lines changed. This block is a bare ``` fence (no `toml`);
    keep it.
  - VERIFY: `grep -n '0xfeed' README.md` no longer hits L232.

Task R3: "## Default Configuration" ```toml block (L300-308): same cleanup
  - OLD:
        # Your QMK keyboard's vendor ID (in hex)
        # vendor_id = 0xfeed

        # Your QMK keyboard's product ID (in hex)
        # product_id = 0x0000
  - NEW:
        # Your QMK keyboard's vendor/product IDs (in hex) — unset: auto-discover
        # any QMK keyboard (recommended); set only to pin a specific board.
        # vendor_id  = 0x????
        # product_id = 0x????
  - GOTCHA G5: both IDs → 0x????. Keep the ```toml fence balanced (G6/G7).
  - VERIFY: `grep -n '0xfeed' README.md` → ZERO hits anywhere in README.

Task R4: --list-devices discovery block — note the picker + the kind column
  - OLD:
        Don't know your keyboard's IDs? Discover them with read-only enumeration:

        ```bash
        qmkonnect --list-devices
        ```
  - NEW:
        Don't know your keyboard's IDs? The **Settings → discovered-device picker**
        lists connected boards and writes the IDs for you; or enumerate read-only:

        ```bash
        qmkonnect --list-devices     # each row shows a `kind` column:
                                     #   qmk_notifier / qmk-only / -
        ```
  - WHY: F13 (spec §8) added the kind column (P4.M1.T1.S1); the README should
         mention it + point to the picker as the primary ID-discovery surface.
  - VERIFY: the block mentions both the picker and the kind column.

# Mid-README checkpoint:
CHECKPOINT R: `grep -rniE '0xfeed' README.md` → ZERO. `grep -c '```' README.md` even.

# ════════════════════════════════════════════════════════════════════
# FILE 2 of 2 (edits): docs/usage.md  (U1)
# ════════════════════════════════════════════════════════════════════

Task U1: "### Verify Keyboard Connection" → three-state model (L114-118)
  - WHY: L116 "Check that QMKonnect shows as 'connected'" is the stale BOOLEAN
         status claim; F13 makes the tray three-state (spec §3). This is the only
         genuine status mention in index/usage (contract grep target).
  - OLD:
        ### Verify Keyboard Connection

        If your layers aren't switching as expected:
        1. Check that QMKonnect shows as "connected" in the system tray/menu bar
        2. Verify your QMK firmware is properly configured with the qmk_notifier module
        3. Test by switching between different applications
  - NEW:
        ### Verify Keyboard Connection

        If your layers aren't switching as expected, read the tray/menu-bar icon —
        it's three-state:

        - **● Device Connected** — a qmk_notifier-capable board is present (you're set).
        - **⚠ QMK board found — no qmk_notifier module (flash it)** — a QMK board is
          attached but isn't running qmk_notifier; flash it (see the
          [QMK Integration Guide]({{ site.baseurl }}/qmk-integration)). This is the
          most common cause of "running but nothing happens."
        - **○ No Device Connected** — no QMK Raw-HID board detected.

        Then:

        1. Verify your QMK firmware is properly configured with the qmk_notifier module
        2. Test by switching between different applications
  - GOTCHA G8: spec §3 strings verbatim (the ⚠ "flash it" parenthetical). G6:
    keep the Jekyll front-matter (--- title: Usage ---) intact; the {{ site.baseurl }}
    Liquid link form matches the rest of usage.md.
  - VERIFY: `grep -F 'no qmk_notifier module (flash it)' docs/usage.md` → 1 hit.

# ════════════════════════════════════════════════════════════════════
# REGENERATE  (AFTER R1-R4 + U1 — G1 ordering)
# ════════════════════════════════════════════════════════════════════

Task REGEN: regenerate docs/llms_full.txt
  - PRECONDITION: README is 0xfeed-clean (CHECKPOINT R passed) + U1 landed.
  - RUN (from the repo root):
        bash docs/generate_llms_full.sh
  - EXPECT stdout: "wrote docs/llms_full.txt (N lines, M bytes)" (N ≈ 2800+).
  - WHY: the current llms_full.txt is stale (0xfeed=6, No-module=0). Regeneration
         concatenates the now-clean README + the parallel task's clean 3 docs +
         this task's clean usage.md → a 0xfeed-free, F13-current agent reference.
  - GOTCHA G1: do NOT run it before R1-R4 (bakes stale 0xfeed). GOTCHA G2: do not
    hand-edit llms_full.txt — it is GENERATED; the script is the single source.

# ════════════════════════════════════════════════════════════════════
# NO-OP VERIFICATIONS  (document grep evidence; LEAVE these files unchanged)
# ════════════════════════════════════════════════════════════════════

Task NOOP-spec: VERIFY spec/ companions are at target wording (do NOT edit)
  - RUN: for f in spec/DEVICE_DISCOVERY.md spec/UI.md spec/PROTOCOL.md spec/ARCHITECTURE.md spec/HOST_RULES.md; do
           printf "%-28s nomodule=%s classify=%s DeviceStatus=%s\n" "$f" \
             "$(grep -c 'no qmk_notifier module' $f)" "$(grep -c 'classify_devices' $f)" "$(grep -c 'DeviceStatus' $f)";
         done
  - EXPECT: DEVICE_DISCOVERY nomodule=2 classify=10 DeviceStatus=1; UI 1/3/1;
    PROTOCOL 0/1/0; ARCHITECTURE 0/4/0; HOST_RULES 0/1/0. (These ARE the spec;
    they pre-exist at target wording.) `git diff --stat spec/` → empty.
  - IF a spec is somehow stale: FLAG it in the handoff (do NOT edit — spec/ is
    human-owned source of truth; the contract says "verify, don't rewrite").

Task NOOP-overviews: document the index.md / docs/README.md leave-decisions
  - docs/index.md: `grep -niE 'vendor/product|auto-discovery|signature' docs/index.md`
    → L25 "no vendor/product IDs needed for a single keyboard" + L36 "Auto-Discovery:
    Finds your keyboard by the QMK Raw HID signature — no IDs needed for a single
    board". Both are accurate version-agnostic one-liners → LEAVE (contract escape
    hatch). Document the grep hits as evidence.
  - docs/README.md: `grep -rniE '0xfeed' docs/README.md` → zero (it's the
    just-the-docs Jekyll THEME readme, a template leftover, NOT concatenated into
    llms_full). LEAVE.
  - docs/examples.md, docs/qmk-integration.md: `grep -rniE '0xfeed'` → zero; not
    flagged by the discovery/status grep. LEAVE.

Task FINAL: VERIFY all gates (see Validation Loop)
```

### Implementation Patterns & Key Details

```markdown
<!-- All edits are exact text replacements via the `edit` tool. Four rules:
     1. Match the BEFORE text EXACTLY (whitespace, backticks, em-dashes —,
        {{ site.baseurl }} Liquid tags). Copy from this PRP's OLD blocks.
     2. README's two config blocks (R2 bare ```, R3 ```toml) have SIMILAR
        two-line bodies — disambiguate by including a UNIQUE surrounding line in
        each oldText (R2's is preceded by "Only set these to pin a specific
        keyboard…"; R3's is preceded by "QMKonnect auto-discovers standard QMK
        keyboards, so the default config leaves these commented out…").
     3. After R1-R4, grep README for 0xfeed (expect zero) BEFORE regenerating
        llms_full (G1).
     4. llms_full.txt is GENERATED — never hand-edit; the script is authoritative. -->

<!-- Spec-wording fidelity (G8): the three-state strings in U1 are copied from
     spec/DEVICE_DISCOVERY.md §3 VERBATIM, including the leading glyph (● / ⚠ / ○)
     and the exact "flash it" parenthetical. The §7.2 cleanup comment (R2) is
     copied VERBATIM incl. "(recommended)" and the two-spaces-around-'=' alignment. -->

<!-- The regeneration is the LAST source-changing step. It is deterministic (the
     script is pure concatenation + front-matter stripping), so its output is
     fully determined by the 8 input files. Verifying 0xfeed=0 + No-module>0 in
     the OUTPUT is sufficient (no manual content review of the 2800-line file). -->
```

### Integration Points

```yaml
DOCS (this task):
  - file: README.md
    change: "R1 picker reframe (Windows & macOS Settings); R2/R3 0xfeed→0x???? (§7.2
             comment); R4 --list-devices picker + kind column."
  - file: docs/usage.md
    change: "U1 Verify Keyboard Connection → three-state model (spec §3 wording)."
  - file: docs/llms_full.txt
    change: "REGENERATED by `bash docs/generate_llms_full.sh` (after R1-R4+U1)."

DEPENDENCIES (this task): NONE runtime. A POSIX shell + bash for the generator
  (the repo's existing dev environment). No cargo, no build.

UPSTREAM (consumed read-only):
  - spec/DEVICE_DISCOVERY.md §3/§5/§7.2 (the wording to mirror — three-state,
    picker, cleanup comment).
  - P4.M1.T1.S1 (code): the --list-devices kind column + the 0x???? seeded
    template the docs describe (Complete).
  - P4.M1.T1.S2 (parallel, LANDED): the 3 clean docs whose §7.2 wording this
    task's README edits mirror (R2/R3 == its C2/C4).

DOWNSTREAM / SIBLINGS (do NOT implement them here):
  - None. This is the final doc sweep (P4.M2). Nothing depends on it except the
    human-readable consistency of the repo's overviews + the agent reference.

CONFIG / DATABASE / ROUTES: none (documentation only).
```

## Validation Loop

### Level 1: The headline gate (0xfeed elimination + fresh regen)

```bash
cd /home/dustin/projects/qmkonnect

# GATE 1 — zero 0xfeed in README + the regenerated llms_full (the contract gate):
grep -rniE '0xfeed' README.md docs/llms_full.txt
# Expected: ZERO output. (Was: README L232+L305; llms_full 6 lines.)

# All concatenated SOURCES are 0xfeed-clean (so the regen couldn't bake one in):
grep -rniE '0xfeed' README.md docs/*.md | grep -v 'docs/vendor/'
# Expected: ZERO output.

# GATE 2 — the regeneration is fresh AND F13 content landed:
grep -c 'No module\|no qmk_notifier module' docs/llms_full.txt   # expect >0 (was 0)
grep -c '0xfeed' docs/llms_full.txt                              # expect 0 (was 6)
# NOTE (G2): `grep -c 'classify_devices\|DeviceStatus' docs/llms_full.txt` is
# EXPECTED to be 0 — those are Rust symbols in src/+spec/, absent from user docs.
# Do NOT treat 0 as a failure. The authoritative regen signals are the two lines
# above (0xfeed=0 + No-module>0) + `git log -1 --format=%ci docs/llms_full.txt`
# showing today's date.

# Confirm the README edits landed at the right anchors:
grep -n 'discovered-device picker' README.md          # expect ≥1 (R1/R4)
grep -n '0x????' README.md                            # expect ≥4 (R2 x2 + R3 x2)
grep -n 'e.g., feed\|e.g., 0000' README.md            # expect ZERO (R1 removed them)
grep -n 'kind' README.md                              # expect ≥1 (R4)
grep -F 'no qmk_notifier module (flash it)' docs/usage.md  # expect 1 (U1)
```

### Level 2: Markdown / front-matter integrity (G6/G7)

```bash
cd /home/dustin/projects/qmkonnect

# Code fences balanced in the two edited source files:
for f in README.md docs/usage.md; do
  n=$(grep -c '```' "$f")
  echo "$f: $n backtick-fence lines ($(([ $n % 2 ])) unbalanced)"
done
# Expected: 0 unbalanced (even count) for each.

# Jekyll front-matter intact in usage.md (README has none):
head -1 docs/usage.md                       # expect: ---
awk 'NR==1&&/^---$/{f=1;next} f&&/^---$/{print "usage.md front-matter OK";exit}' docs/usage.md

# README still has its top matter intact (it's a plain GitHub README, no Jekyll FM):
head -1 README.md                           # expect: # QMKonnect
```

### Level 3: Scope + no-op verifications (G3/G7)

```bash
cd /home/dustin/projects/qmkonnect

# GATE 3 — ONLY the expected files changed:
git status --short
# Expected EXACTLY:
#    M README.md
#    M docs/usage.md
#    M docs/llms_full.txt
# NOTHING in src/, spec/, the parallel task's 3 docs, index.md, docs/README.md,
# examples.md, qmk-integration.md, Cargo.toml, PRD.md, tasks.json, .gitignore.
git diff --stat
# Expected: 3 files.

# GATE 4 — spec/ companions unchanged + still at target wording (NOOP-spec):
git diff --stat spec/                       # expect: empty
grep -c 'no qmk_notifier module' spec/DEVICE_DISCOVERY.md   # expect ≥1 (unchanged)

# NOOP-overviews — document the leave-decisions (these print evidence; NOT failures):
grep -niE 'vendor/product|auto-discovery|signature' docs/index.md   # L25 + L36 one-liners (LEAVE)
grep -rniE '0xfeed' docs/README.md                                  # zero (theme readme; LEAVE)
grep -rniE '0xfeed' docs/examples.md docs/qmk-integration.md        # zero (LEAVE)
```

### Level 4: Regeneration sanity (the agent-reference freshness check)

```bash
cd /home/dustin/projects/qmkonnect

# The regenerated llms_full.txt reflects the cleaned docs (spot-check a region):
sed -n '/^=*\n 1\. README\.md/,/^=*\n 2\./p' docs/llms_full.txt | grep -iE 'picker|0x????|kind' | head
# Expected: at least one line showing the README's new picker/0x????/kind content
# (proves the regen picked up R1/R2/R4).

# The three-state text is now in the concatenated agent reference:
grep -F 'no qmk_notifier module (flash it)' docs/llms_full.txt
# Expected: ≥2 hits (from docs/usage.md U1 + the parallel task's installation.md/
# troubleshooting.md three-state text).

# (Optional) the generator is idempotent — re-running yields a stable diff:
bash docs/generate_llms_full.sh && git diff --stat docs/llms_full.txt
# Expected: "wrote …" + (ideally) no further diff if re-run immediately.
```

## Final Validation Checklist

### Technical Validation
- [ ] **GATE 1**: `grep -rniE '0xfeed' README.md docs/llms_full.txt` → **zero**.
- [ ] All concatenated sources clean: `grep -rniE '0xfeed' README.md docs/*.md | grep -v vendor` → zero.
- [ ] **GATE 2**: `grep -c 'No module\|no qmk_notifier module' docs/llms_full.txt` > 0 (was 0); `0xfeed`=0.
- [ ] **GATE 3**: `git diff --stat` = README.md + docs/usage.md + docs/llms_full.txt only.
- [ ] **GATE 4**: `git diff --stat spec/` empty (spec/ verified, not rewritten).
- [ ] Code fences balanced + Jekyll front-matter intact in docs/usage.md (L2).

### Feature Validation (contract fidelity)
- [ ] **R1** — README "Windows & macOS" Settings: picker primary + Advanced disclosure; `(e.g., feed)`/`(e.g., 0000)` gone.
- [ ] **R2/R3** — README config blocks: `0xfeed`/`0x0000` → `0x????` + §7.2 comment (matches the parallel task's docs + S1's code, G4); both IDs changed (G5).
- [ ] **R4** — README `--list-devices`: notes the picker + the `kind` column.
- [ ] **U1** — docs/usage.md "Verify Keyboard Connection": three-state model (spec §3 verbatim, G8).
- [ ] **REGEN** — `bash docs/generate_llms_full.sh` run AFTER R1-R4+U1 (G1); llms_full rewritten.

### Code Quality Validation (doc quality)
- [ ] Wording mirrors spec §3 (three-state) + §7.2 (cleanup comment) verbatim — no invented behavior (G4/G8).
- [ ] README edits placed exactly where discovery/VID-PID is discussed; no unrelated rewrites.
- [ ] The two `0xFF60 / 0x61 auto-discovers` one-liners in README LEFT (version-agnostic; documented).
- [ ] `docs/index.md`, `docs/README.md`, `docs/examples.md`, `docs/qmk-integration.md` LEFT (documented no-ops, G3).

### Documentation & Deployment
- [ ] The regenerated `llms_full.txt` is the single-file agent/LLM reference and now reflects F13/F14 (no stale `0xfeed`, three-state present).
- [ ] Commit message notes this is the Mode-B final doc sweep; it depends on P1-P3 + P4.M1 (the parallel Mode-A task landed first).

---

## Anti-Patterns to Avoid

- ❌ Do NOT regenerate `docs/llms_full.txt` before cleaning README L232+L305 — the
      script concatenates README first and would bake the stale `0xfeed` in (G1).
      Order: R1-R4 + U1 → verify README 0xfeed-clean → THEN run the script.
- ❌ Do NOT hand-edit `docs/llms_full.txt`. It is GENERATED. The script is the
      single source; edit the 8 input docs (here: README + usage.md) and re-run.
- ❌ Do NOT chase `classify_devices`/`DeviceStatus` in llms_full (G2). They are
      Rust symbols in `src/`+`spec/`, absent from the 8 user-doc files. After
      regen the contract grep shows only `No module` (>0); that + `0xfeed`=0 are
      the authoritative signals. Do NOT add internal symbols to user docs.
- ❌ Do NOT rewrite `spec/` (G3). They ARE the spec this delta implements (verified
      at target wording). Re-grep to confirm; if somehow stale, FLAG — don't edit.
- ❌ Do NOT leave `0x0000` next to a cleaned `0x????` (G5). Change BOTH
      `vendor_id` and `product_id` lines in R2 and R3.
- ❌ Do NOT paraphrase the three-state status strings (G8). Copy spec §3 verbatim
      (`⚠ QMK board found — no qmk_notifier module (flash it)`, etc.).
- ❌ Do NOT touch `src/`, the parallel task's 3 already-clean docs, `Cargo.toml`,
      `PRD.md`, `**/tasks.json`, `**/prd_snapshot.md`, `.gitignore`, `docs/index.md`
      (one-liners), `docs/README.md` (theme readme), or `docs/examples.md`/
      `qmk-integration.md` (G7 scope wall).
- ❌ Do NOT copy `spec/DEVICE_DISCOVERY.md` §5.1's literal `0xFEED:0x0000` device
      example into README/docs — that re-introduces a `0xFEED` hit. Use `0x????`
      or board-name-only (G3).
- ❌ Do NOT edit `PRD.md`, any `tasks.json`, `prd_snapshot.md`, any `spec/*.md`, or
      any `plan/` file other than this item's own `PRP.md` + `research/`.

---

## Confidence Score: 9/10

Pure documentation edits + one deterministic regeneration. The EXACT BEFORE text
of every edit site is captured from full-file reads (README L232+L305 are the
only source `0xfeed`; usage.md L116 is the only stale status claim); the EXACT
target wording (spec §3 three-state, §7.2 cleanup comment, parallel task's
already-landed picker/Advanced phrasing) is pinned verbatim; and the full
`0xfeed` surface is enumerated so none is missed. The two judgment calls —
resolving the contract's over-broad verification grep into the realistic
`0xfeed`=0 + `No module`>0 signals (G2), and leaving the version-agnostic
one-liners in README/index.md (documented with grep evidence) — are each
justified. Residual risks: (a) exact-whitespace matching in `edit` oldText for
the two similar README config blocks (mitigated by unique surrounding-context
lines); (b) the regeneration depends on the parallel task's 3 docs being clean
(verified: they are). No code, no build, no deps — the work is transcription of
pinned text + one `bash` invocation.
# Research Notes — P4.M2.T1.S1: Audit README.md + overviews; regenerate docs/llms_full.txt

> Scope: the **final Mode-B changeset sweep** for the F13/F14 plan
> (`plan/005_8b95ea464bd9`). DOCS-only across **`README.md` + `docs/usage.md` +
> the regenerated `docs/llms_full.txt`**. Verify (don't rewrite) the `spec/`
> companions. Runs LAST — depends on P1/P2/P3 + P4.M1 (all Complete; the parallel
> Mode-A task P4.M1.T1.S2 has ALREADY landed: its 3 docs are 0xfeed-clean).

---

## §0 — State of the world (evidence-backed)

### §0.1 The `0xfeed` surface (the regeneration can't bake a stray one in)
`grep -rniE '0xfeed' README.md docs/*.md` (excluding `docs/vendor/`):
```
README.md:232:# vendor_id = 0xfeed       (the "### Linux" config block)
README.md:305:# vendor_id = 0xfeed       (the "## Default Configuration" block)
```
→ **ONLY README.md L232 + L305** carry `0xfeed` in any SOURCE file. The parallel
task's 3 docs (`docs/configuration.md`, `docs/installation.md`,
`docs/troubleshooting.md`) are **already 0xfeed-clean** (verified: `grep … ; echo
$?` → 1). `docs/examples.md`, `docs/qmk-integration.md`, `docs/index.md`,
`docs/README.md` are 0xfeed-clean too. So once README L232+L305 are cleaned, the
8-file concatenation has ZERO `0xfeed` sources.

### §0.2 `docs/llms_full.txt` is STALE (the BEFORE state)
`docs/llms_full.txt` is a generated concat (README + 7 docs, Jekyll front-matter
stripped). Current state:
```
0xfeed hits: 6   (L253, L326 from README; L1061, L1172 from configuration.md
                  seeded/zero-config; L1102, L2480 the 0xFEED config.h examples
                  in configuration.md + troubleshooting.md)
No module / no qmk_notifier module: 0
classify_devices: 0     DeviceStatus: 0     three-state: 0
```
The 4 non-README 0xfeed lines (L1061/1102/1172/2480) are LEFTOVERS from the
parallel task's now-cleaned docs — they vanish the moment we **regenerate**
(because the source docs are clean). The 2 README lines (L253/326) vanish once I
clean README. → regeneration is the fix; do NOT hand-edit llms_full.txt.

### §0.3 The contract's verification (c) is partly aspirational — read this
The contract says: *"`grep -rn 'classify_devices|DeviceStatus|No module'
docs/llms_full.txt` shows the regenerated content and the 0xfeed literal is gone."*
After regeneration the REALISTIC result is:
- `0xfeed` → **0** (was 6) ✓ — the authoritative "regen happened + README clean" signal.
- `No module` / `no qmk_notifier module` → **>0** (was 0) ✓ — comes from the
  parallel task's three-state text in installation.md/troubleshooting.md/configuration.md
  + my README/usage edits. This is the F13 user-facing signal.
- `classify_devices` / `DeviceStatus` → **still 0**. These are **Rust symbols**
  that live in `src/` + `spec/`, NOT in any of the 8 concatenated user-doc files.
  Their absence is EXPECTED and is NOT a failure. **Do not contort user docs to
  include internal symbols, and do not treat 0 as a bug.** The authoritative
  regen signals are `0xfeed`=0 + `No module`>0 + a changed mtime/line-count.

### §0.4 `spec/` companions — ALREADY at target wording (verify, don't rewrite)
The contract says spec/* "ARE the spec this delta implements — verify, don't
rewrite." Confirmed via grep (all >0 where relevant; none have 0xfeed):
```
spec/DEVICE_DISCOVERY.md   no qmk_notifier module=2  classify_devices=10  DeviceStatus=1
spec/UI.md                                            classify_devices=3   DeviceStatus=1  (nomodule=1)
spec/PROTOCOL.md                                      classify_devices=1                   (byte-level doc; nomodule=0 is fine)
spec/ARCHITECTURE.md                                  classify_devices=4                   (status-probe section; nomodule=0 fine)
spec/HOST_RULES.md                                     classify_devices=1                   (host-rules doc; nomodule=0 fine)
```
→ **no spec/ edits.** The implementer re-greps to confirm; if a spec is somehow
stale, FLAG it (don't rewrite — spec/ is human-owned source of truth).

### §0.5 Out-of-scope files (documented no-ops)
- **`docs/index.md`** — only version-agnostic one-liners: L25 "no vendor/product
  IDs needed for a single keyboard", L36 "Auto-Discovery: Finds your keyboard by
  the QMK Raw HID signature — no IDs needed for a single board". The contract's
  escape hatch ("if it's a version-agnostic one-liner, leave it") applies. **LEAVE.**
- **`docs/README.md`** — the **just-the-docs Jekyll theme readme** (a template
  leftover), NOT concatenated into llms_full (`generate_llms_full.sh` uses
  `$ROOT/README.md`, the repo root). 0xfeed-clean. **LEAVE.**
- **`docs/examples.md`, `docs/qmk-integration.md`** — in the concatenation but
  0xfeed-clean and not flagged by the contract's discovery/status grep as needing
  F13/F14 reframing. **LEAVE.**

---

## §1 — `docs/generate_llms_full.sh` (the generator — verbatim contract)

Concatenates (in order), stripping a LEADING `--- … ---` Jekyll front-matter block:
1. `README.md` (repo root — NOT `docs/README.md`)
2. `docs/index.md` 3. `docs/installation.md` 4. `docs/qmk-integration.md`
5. `docs/configuration.md` 6. `docs/usage.md` 7. `docs/examples.md`
8. `docs/troubleshooting.md`
→ writes `docs/llms_full.txt`. Run from the repo root:
`bash docs/generate_llms_full.sh` (it `cd`s to its own dir; cwd-agnostic).
**MUST run AFTER README L232+L305 are cleaned** (else it bakes the stale literal
in — G1). It prints `wrote docs/llms_full.txt (N lines, M bytes)`.

---

## §2 — README.md exact edit sites (verbatim BEFORE → AFTER)

### R1 — "### Windows & macOS" Settings block (reframe picker→primary, VID/PID→Advanced)
Removes the `(e.g., feed)` / `(e.g., 0000)` raw-hex framing (the F13 misread
source). Mirrors the parallel task's `docs/configuration.md` C1.
- BEFORE (the block under `## Configuration` → `### Windows & macOS`):
  ```
  1. Right-click the QMKonnect system tray icon
  2. Select "Settings"
  3. Enter your keyboard's Vendor ID (hex format, e.g., feed)
  4. Enter your keyboard's Product ID (hex format, e.g., 0000)
  5. Click OK to save
  ```
- AFTER: a paragraph stating auto-discovery is the common case; Settings opens
  only to disambiguate; the dialog lists boards by name + VID:PID + ✓/✗ marker;
  pick one → QMKonnect writes its VID/PID; **Advanced ▸** = raw hex fields.
  (Full text in PRP Task R1.)

### R2 — Linux config ``` block (L232-233): `0xfeed`/`0x0000` → `0x????` + §7.2 comment
- BEFORE: `# vendor_id = 0xfeed` / `# product_id = 0x0000`
- AFTER:  `# vendor_id  = 0x????   # unset: auto-discover any QMK keyboard (recommended)`
          `# product_id = 0x????   # unset: auto-discover any QMK keyboard (recommended)`
  (matches the parallel task's `docs/configuration.md` C2/C4 + S1's code output — G3.)

### R3 — "## Default Configuration" ```toml block (L304-308): same cleanup
- BEFORE:
  ```
  # Your QMK keyboard's vendor ID (in hex)
  # vendor_id = 0xfeed

  # Your QMK keyboard's product ID (in hex)
  # product_id = 0x0000
  ```
- AFTER: a 2-line `#` header + `# vendor_id  = 0x????` / `# product_id = 0x????`
  (drop the `0xfeed`; keep the block a valid ```toml fence — G7).

### R4 — `--list-devices` discovery block (add the `kind` column note, F13)
- BEFORE:
  ```
  Don't know your keyboard's IDs? Discover them with read-only enumeration:

  ```bash
  qmkonnect --list-devices
  ```
  ```
- AFTER: lead with "the Settings picker lists boards + writes IDs for you"; note
  `--list-devices` now prints a `kind` column (`qmk_notifier` / `qmk-only` / `-`).
  (Full text in PRP Task R4.)

### README items explicitly LEFT (documented, grep evidence)
- The two **"auto-discovers it via the Raw HID usage page (0xFF60 / 0x61)"**
  one-liners (Linux setup + Configuration intro) — accurate Tier-1 statements;
  the contract's one-liner escape hatch applies; the two-tier detail lives in
  DEVICE_DISCOVERY.md + the installation/troubleshooting docs the parallel task
  already updated. **LEAVE.**
- No boolean "status"/"connected" claim exists in README (the contract: "No
  'status' (menu/indicator) mentions yet") → no stale status text to reframe.

---

## §3 — docs/usage.md edit site (U1) — the one stale status claim
- **U1 — "### Verify Keyboard Connection" list, L116** (the ONLY genuine status
  mention in index/usage): `1. Check that QMKonnect shows as "connected" in the
  system tray/menu bar` → boolean framing; F13 makes it three-state. Replace the
  numbered list with the three-state bullets (●/⚠/○, spec §3 wording verbatim),
  then keep the remaining 2 numbered steps. (Full text in PRP Task U1.)
- The "Status and Monitoring" heading (L105) + `systemctl --user status qmkonnect`
  (L26/L110) are **process-running** checks, NOT device-status → **LEAVE.**

---

## §4 — Gotchas (G1-G8)

- **G1 — regenerate ONLY after README is clean.** `generate_llms_full.sh`
  concatenates README first. If you run it before cleaning README L232+L305, the
  stale `0xfeed` is baked into llms_full. Order: R1-R4 + U1 → verify README clean
  → THEN `bash docs/generate_llms_full.sh`.
- **G2 — `classify_devices`/`DeviceStatus` won't appear in llms_full.** They're
  Rust symbols in `src/`+`spec/`, absent from all 8 concatenated user-doc files.
  After regen the contract grep shows only `No module` (>0). Do NOT treat the 0
  on classify/DeviceStatus as a failure; do NOT add internal symbols to user docs.
  Authoritative regen signals: `0xfeed`=0 + `No module`>0 + changed mtime/lines.
- **G3 — don't rewrite `spec/`.** They ARE the spec this delta implements
  (verified at target wording, §0.4). Re-grep to confirm; if somehow stale, FLAG
  (don't edit — spec/ is human-owned). Also don't touch `docs/README.md` (the
  theme readme), `docs/index.md` (one-liners), `docs/examples.md`,
  `docs/qmk-integration.md`.
- **G4 — match the §7.2 comment byte-for-byte.** README's cleaned config lines
  must equal the parallel task's `docs/configuration.md` C2/C4 AND S1's code
  output: `# vendor_id  = 0x????   # unset: auto-discover any QMK keyboard
  (recommended)` (note TWO spaces around `=` on vendor, ONE on product — match
  existing alignment). Inconsistent docs-vs-code = user confusion.
- **G5 — clean BOTH `0xfeed` and the adjacent `0x0000`.** Leaving `# product_id
  = 0x0000` next to `# vendor_id = 0x????` keeps half the "feed/0000 = default"
  misread alive. Change both lines (R2, R3).
- **G6 — preserve ``` fences + Jekyll front-matter.** README's config blocks are
  ``` and ```toml fenced; keep them balanced. usage.md has Jekyll front-matter
  (--- title … ---); leave it. R1's reframe is prose (no fence); R4 keeps the
  ```bash fence.
- **G7 — scope wall.** Edit ONLY `README.md`, `docs/usage.md`, and regenerate
  `docs/llms_full.txt`. Do NOT touch `src/`, `spec/`, the parallel task's 3 docs,
  `PRD.md`, `**/tasks.json`, `**/prd_snapshot.md`, `.gitignore`, `Cargo.toml`,
  or any other file.
- **G8 — three-state wording mirrors spec §3 verbatim.** Use the EXACT strings:
  `● Device Connected`, `⚠ QMK board found — no qmk_notifier module (flash it)`,
  `○ No Device Connected` (U1 + any README mention). Do not paraphrase.

---

## §5 — Validation (the deterministic gates)

```bash
cd /home/dustin/projects/qmkonnect

# GATE 1 — zero 0xfeed in README + regenerated llms_full (the headline gate):
grep -rniE '0xfeed' README.md docs/llms_full.txt            # expect: ZERO output
grep -rniE '0xfeed' README.md docs/*.md | grep -v vendor    # expect: ZERO (all sources clean)

# GATE 2 — regeneration is fresh + F13 content landed:
grep -c 'No module\|no qmk_notifier module' docs/llms_full.txt   # expect: >0 (was 0)
test "$(grep -c '0xfeed' docs/llms_full.txt)" = "0" && echo "llms_full 0xfeed-clean ✓"

# GATE 3 — scope (only README + usage + llms_full changed):
git diff --stat   # expect EXACTLY: README.md, docs/usage.md, docs/llms_full.txt

# GATE 4 — spec/ companions still at target wording (verify, unchanged):
git diff --stat spec/    # expect: empty
grep -c 'no qmk_notifier module' spec/DEVICE_DISCOVERY.md   # expect ≥1 (unchanged)

# GATE 5 — fences balanced in edited files:
for f in README.md docs/usage.md; do n=$(grep -c '```' "$f"); echo "$f: $n fences ($(([ $n % 2 ])) unbalanced)"; done
```
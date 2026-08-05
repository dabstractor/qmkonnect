# Research Notes — P4.M1.T1.S2 (Mode-A user-facing doc sync)

Scope: sync `docs/configuration.md`, `docs/installation.md`,
`docs/troubleshooting.md` with F13 (three-state status) + F14 (discovered-device
picker), per `spec/DEVICE_DISCOVERY.md` §3/§5/§7, AND remove every
`0xfeed`/`0xFEED` literal from those three docs. **Docs-only task — no src/.**

---

## §0 — The sibling contract (P4.M1.T1.S1, parallel, code-side)

S1 (read its PRP) produces, in `src/`:
- `render_default_config_template` + `render_config_body` `None` arms now emit
  `# vendor_id  = 0x????   # unset: auto-discover any QMK keyboard (recommended)`
  (verbatim `spec/DEVICE_DISCOVERY.md` §7.2) instead of the `0xfeed` literal.
- `list_devices(verbose)` → `--list-devices` gains a `kind` column
  (`qmk_notifier` / `qmk-only` / `-`) from a one-shot `classify_devices`.

⇒ The SEEDED `config.toml` (`qmkonnect -c`) that docs describe must match the new
code output (`0x????`). So docs' seeded-template example MUST become `0x????`.
The `--list-devices` CLI row MAY note the `kind` column (F13 consistency).

---

## §1 — Gate resolution (THE critical correctness decision)

Contract says: "Verify `grep -rn '0xfeed|0xFEED' docs/ src/` returns zero hits
(excluding `docs/vendor/`)." Taken literally this is **impossible for S2 alone**:

- **`src/` leg is NOT this task.** S1 owns the src/ renderer literals; AND `src/`
  legitimately keeps `0xfeed` as a config **VALUE** in ~16 test fixtures / parse
  tests (`vendor_id = 0xfeed` parsing, `render_vidpid_rule(Some(0xfeed)…)`,
  picker UI tests with `0xFEED`, the `template_has_no_0xfeed_literal` test S1
  ADDS). `spec/CONFIG.md` §1 explicitly: "Legacy files that set `vendor_id =
  0xfeed` keep working (become `Some(0xfeed)`)." S1's PRP **G2** preserves these.
  So `grep … src/` is **never fully zero** and that is intended. S2 does not
  touch `src/`.

- **`docs/llms_full.txt` is NOT this task.** It is GENERATED
  (`docs/generate_llms_full.sh` = concat of README.md + 7 docs). Task tree gives
  its regen to **P4.M2.T1.S1**. Regen must run AFTER **README.md** is also
  cleaned (README L232-233/L305-308 still have `0xfeed`, owned by P4.M2.T1.S1);
  regenerating now would bake README's stale `0xfeed` into llms_full.txt. So S2
  must NOT regenerate it.

⇒ **S2's verifiable gate (correctly scoped):**
```bash
grep -rn -iE '0xfeed' docs/configuration.md docs/installation.md docs/troubleshooting.md
# → ZERO hits
```
The contract's `docs/ src/` grep is the **aggregate P4 gate** (S1 cleans src/
renderer literals + P4.M2.T1.S1 cleans README + regenerates llms_full ⇒ combined
zero for the *user-facing/template* sense; src/ value-fixture hits remain by
design). Documented as **G1** in the PRP so the implementer does NOT (a) delete
src/ test fixtures or (b) panic over llms_full/src hits.

---

## §2 — Enumeration of 0xfeed/0xFEED in the 3 target docs (grep, current)

| File | Line | Hit | Edit |
|---|---|---|---|
| `docs/configuration.md` | 71 | `# vendor_id  = 0xfeed   # unset: auto-discovery` | C2 → `0x????` + §7.2 |
| `docs/configuration.md` | 72 | `# product_id = 0x0000   # unset: auto-discovery` | C2 (0x0000→0x???? consistency) |
| `docs/configuration.md` | 112 | `#define VENDOR_ID    0xFEED` | C3 → `0x????` placeholder |
| `docs/configuration.md` | 113 | `#define PRODUCT_ID   0x0000` | C3 (→0x???? consistency) |
| `docs/configuration.md` | 182 | `# vendor_id  = 0xfeed   # unset: auto-discovery` | C4 → `0x????` + §7.2 |
| `docs/configuration.md` | 183 | `# product_id = 0x0000   # unset: auto-discovery` | C4 (→0x???? consistency) |
| `docs/troubleshooting.md` | 439 | `#define VENDOR_ID    0xFEED` | T2 → `0x????` placeholder |
| `docs/troubleshooting.md` | 440 | `#define PRODUCT_ID   0x0000` | T2 (→0x???? consistency) |
| `docs/installation.md` | — | **no `0xfeed` hit** (only VID/PID text @103, `--list-devices` @170) | no cleanup; F13/F14 add only |

Note: `0x0000` is **not** matched by the gate (`0xfeed|0xFEED`), but changing
both VENDOR/PRODUCT to `0x????` in the config.h + zero-config examples keeps the
example pairs consistent and fully kills the "0xFEED/0x0000 = the default"
misreading (judgment call, flagged as **G2**).

---

## §3 — Where F13/F14 mentions go (per contract: "where VID/PID discovery is discussed")

**configuration.md** — reframe `vendor_id`/`product_id` as Advanced override:
- Windows & macOS GUI Settings block (C1): picker = primary surface; VID/PID hex
  fields move under an **Advanced** disclosure (mirror `spec` §5).
- Field reference table rows (C5): "Advanced override — the discovered-device
  picker writes this for you" (mirror `spec/CONFIG.md` §1.1 wording verbatim).
- CLI `--list-devices` row (C6): note the `kind` column (F13).

**installation.md**:
- Linux disambiguation block @~103 (I1): "rarely set by hand — the Settings →
  discovered-device picker writes the IDs for you."
- Verification §1 (I2): the tray/menu-bar icon is now **three-state** (●
  Connected / ⚠ No module → flash qmk_notifier / ○ Disconnected) — mirror
  `spec/DEVICE_DISCOVERY.md` §3 table text.

**troubleshooting.md**:
- "Keyboard Not Detected" (T1): lead with the three-state tray status (the "No
  module" → flash qmk_notifier guidance is the most common "detected but nothing
  happens" cause); Solutions bullet → use the picker to confirm which board is
  seen.
- "Wrong Keyboard IDs" (T2): intro → picker / `--list-devices` is the primary way
  to find IDs now; config.h `0xFEED` → `0x????`.

---

## §4 — Spec wording to mirror verbatim

**Three-state status** (`spec/DEVICE_DISCOVERY.md` §3 table):
- Connected: `●  Device Connected` (or `●  N Devices Connected`)
- No module: `⚠  QMK board found — no qmk_notifier module (flash it)`
- Disconnected: `○  No Device Connected`

**Picker** (`spec` §5.1):
- "Detected keyboard(s):" list, ✓ (qmk_notifier-capable) / ✗ (QMK board, no
  module), `[ Choose… ] [ Rescan ]`.
- Single capable board, no VID/PID set → header only, no picker (zero-config).
- Multiple Tier-1 boards → picker; selecting writes that board's VID/PID.
- **Advanced / manual override** (§5.2) disclosure = the two raw hex fields.

**Advanced-override framing** (`spec/CONFIG.md` §1.1):
- vendor_id: "Advanced override — the discovered-device picker writes this for
  you (`DEVICE_DISCOVERY.md` §5). Unset ⇒ auto-discover any qmk_notifier-capable
  board."
- product_id: "Advanced override — set only to disambiguate among multiple
  boards."

**0xFEED cleanup target** (`spec` §7.2):
```
# vendor_id  = 0x????   # unset: auto-discover any QMK keyboard (recommended)
# product_id = 0x????   # unset: auto-discover any QMK keyboard (recommended)
```

---

## §5 — Scope walls (files NOT touched by S2)

- `src/**` → S1 (renderer literals) + permanent 0xfeed VALUE fixtures (spec §1).
- `README.md` → P4.M2.T1.S1 (L232-233/L305-308 0xfeed).
- `docs/llms_full.txt` → P4.M2.T1.S1 (regenerate, AFTER README cleaned).
- `spec/**` → read-only source of truth.
- `docs/qmk-integration.md`, `docs/index.md`, `docs/usage.md`, `docs/examples.md`
  → not in S2's 3-file OUTPUT list; don't edit.
- `PRD.md`, `**/tasks.json`, `**/prd_snapshot.md`, `.gitignore` → never.

---

## §6 — Confidence: 9/10

Pure doc edits; exact BEFORE text captured (full-file reads), exact §7.2/§3/§5
target text pinned from spec, every 0xfeed site enumerated, the gate-over-breadth
contradiction resolved with a precisely-scoped verifiable gate. Only residual:
no markdown-lint gate in repo (validation = grep + front-matter/markdown
structure preserved + visual); Jekyll front-matter `---` blocks must be left
intact.
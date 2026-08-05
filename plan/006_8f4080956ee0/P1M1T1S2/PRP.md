# PRP — P1.M1.T1.S2: Confirm zero spec drift — the 6 v6 diff hunks have corresponding spec passages

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`).
> **This is a READ-ONLY SPEC-DRIFT AUDIT.** No code or spec edits. The deliverable is a
> 6-row PASS/FAIL table proving each of the 6 v6 diff hunks (delta PRD §1) has a
> corresponding passage in v6 wording in its spec file. `git status` for `spec/` (and
> `src/`) must stay clean.
> **Verified baseline (research run, this session): ALL 6 HUNKS PASS — ZERO SPEC DRIFT.**
> The runbook below reproduces that result; the table in "What" is the pre-verified answer.

---

## Goal

**Feature Goal**: Independently re-confirm that the 6 diff hunks introduced by the
v6 capability-keyed-lifecycle delta (commit `d240b27`, already shipped) each have a
corresponding passage in v6 wording in the relevant spec file — i.e. **zero spec drift**
between the delta PRD §1 description and the canonical `spec/*.md` files. This is the
read-only audit counterpart to S1's gate run.

**Deliverable**: A 6-row table — `Hunk # | Spec file | Key phrase searched | Found Y/N |
Line number` — plus an overall verdict (ZERO DRIFT / DRIFT FOUND). Flag any hunk whose
passage is missing or whose wording diverges from v6. (Research result: none diverge.)

**Success Definition**: All 6 hunks report `Found = Y` with the actual line number(s);
the table's overall verdict is **ZERO SPEC DRIFT**; no spec or source file is modified
(`git status` clean for `spec/` and `src/`); the incidental `device_connected`
pseudo-code observation at LINUX.md:~230 is noted but NOT counted as a hunk failure
(see Gotchas).

## User Persona (if applicable)

**Target User**: The release/maintainer who needs a documented, independent confirmation
that the spec files (the source of truth consumed by `docs/` regeneration and by future
implementers) actually carry the v6 delta's wording — not just that the code ships it.

**Use Case**: Before declaring the capability-keyed-lifecycle delta fully verified
(S1 = gates green; S2 = spec drift zero; S3 = caveats backed by code), produce the
spec-side evidence that the 6 hunks landed in `spec/*.md` in v6 wording.

**Pain Points Addressed**: Removes the risk that the delta PRD describes wording the
spec files don't actually contain (silent spec/code drift), which would mislead future
doc regeneration and implementers.

## Why

- **The delta is already shipped to code AND spec** (per
  `plan/006_8f4080956ee0/architecture/verification_findings.md` §0/§3). S1 verifies the
  CODE passes its gates; THIS subtask independently verifies the SPEC carries the v6
  wording. The two are complementary halves of "the delta is complete."
- **`verification_findings.md` §3 already asserts all 6 present**, but the contract
  requires an *independent re-confirmation* — direct grep against the spec files, not a
  transcription of the prior research. The research run for this PRP performed exactly
  that independent re-confirmation; the runbook reproduces it.
- **It gates the doc-sync sibling.** P1.M1.T2.S1 (README/docs overview staleness audit)
  assumes the spec is the authoritative v6 source; a green S2 is the prerequisite signal
  that the spec is trustworthy to sync from.

## What

For each of the 6 hunks, grep its spec file for the contracted key phrase(s) and record
Found Y/N + line number(s). The pre-verified result (run directly against the files in
this research session):

| Hunk # | Spec file (§) | Key phrase searched | Found | Line number(s) | Verdict |
|---|---|---|---|---|---|
| **#1** | `spec/DEVICE_DISCOVERY.md` (§2.2) | `No proto-v2 or pure-VIA` ; `process_full_message("")` (deactivation) | **Y** | 81 ; 90 (also 85) | ✅ PASS |
| **#2** | `spec/DEVICE_DISCOVERY.md` (§2.4) | `Handshake → cache warm-feed scope` ; `correct only when a single Tier-1 board` (guard) | **Y** | 155 ; 159 | ✅ PASS |
| **#3** | `spec/DEVICE_DISCOVERY.md` (§3) | `capable-keyed` ; `PresenceTracker` ; `only when the path set changes` (plug/unplug) | **Y** | 189 ; 97 & 189 ; 191 | ✅ PASS |
| **#4** | `spec/LINUX.md` (§6.2) | `Trayless (--no-default-features) build caveat` ; `BindsTo=dev-qmkonnect_device.device` ; `Restart=always` | **Y** | 211 ; 216 (also 179, 199) ; 217 (also 186, 201) | ✅ PASS |
| **#5** | `spec/LINUX.md` (§7.1) | `PresenceTracker tick` (regex `PresenceTracker.+tick`) ; `t.device_status` (renamed field) | **Y** | 236 ; 240 | ✅ PASS |
| **#6** | `spec/HOST_RULES.md` (§13 R6) | `R6 … Legacy handshake side effect` ; `proto-v1 firmware it can briefly reset the layer` ; `see PresenceTracker` | **Y** | 616 ; 626 ; 627 | ✅ PASS |

**Overall verdict: ZERO SPEC DRIFT.** All 6 v6 diff hunks have corresponding passages in
v6 wording in the spec files.

### Success Criteria

- [ ] The 6-row table is produced with Found Y/N + actual line number(s) per hunk.
- [ ] All 6 hunks report `Found = Y` (research baseline: all Y).
- [ ] Overall verdict stated explicitly: **ZERO SPEC DRIFT** (or DRIFT FOUND + which hunk).
- [ ] Any divergence from v6 wording is flagged (research baseline: none).
- [ ] The hunk #5 grep uses the regex-tolerant `PresenceTracker.+tick` form (NOT the
      literal `PresenceTracker tick` — see Gotchas), so it correctly matches :236.
- [ ] The incidental `device_connected` pseudo-code at LINUX.md:~230 is noted as an
      observation, NOT counted as a hunk #5 failure.
- [ ] No spec or source file modified (`git status --short spec/ src/` clean).

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed to
> run this audit successfully?"_ — **Yes.** The 6 exact greps (with the regex-tolerant
> hunk #5 form), the pre-verified expected line numbers, the hunk #5 backtick gotcha,
> the incidental-observation framing, and the table format are all below.

### Documentation & References

```yaml
# MUST READ — the prior research doc whose §3 assertion S2 re-confirms independently
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/architecture/verification_findings.md
  why: "§3 'Verified Spec Drift (v6 Wording Present)' lists all 6 hunks with line numbers
        from the prior session. S2 re-runs the greps directly against the files (independent
        re-confirmation) and must agree. §0/§1 establish the delta is already shipped
        (commit d240b27) so S2 audits a static target."
  section: "3. Verified Spec Drift (v6 Wording Present)", "0. Executive Summary"
  critical: "S2 must NOT transcribe §3 — it must re-grep the files. Agreement is the point.
             The one place §3's wording needs care: hunk #5's 'PresenceTracker tick' is
             backtick-wrapped in the file (`PresenceTracker` tick) — use a regex grep."

# MUST READ — the three spec files under audit (the grep targets)
- file: /home/dustin/projects/qmkonnect/spec/DEVICE_DISCOVERY.md
  why: "Hunks #1 (§2.2 'What the probe sends'), #2 (§2.4 'Relationship to the host-rules
        handshake'), #3 (§3 'Device-Status Semantics'). grep target for the capable-keyed
        lifecycle + warm-feed scope + proto-v1 caveat wording."
  section: "2.2 What the probe sends", "2.4 Relationship to the host-rules handshake", "3. Device-Status Semantics"
  gotcha: "Hunk #1's process_full_message deactivation is at :90 (the proto-v1 caveat block),
           NOT in the §2.2 lead paragraph. grep the whole file for the phrase."

- file: /home/dustin/projects/qmkonnect/spec/LINUX.md
  why: "Hunks #4 (§6.2 'Why the service is optional' — trayless caveat) and #5 (§7.1
        'spawn()' — the SNI tray poll thread)."
  section: "6.2 Why the service is optional", "7.1 spawn() -> Option<Handle>"
  gotcha: "Hunk #5's phrase is 'drive a `PresenceTracker` tick' at :236 — PresenceTracker is
           BACKTICK-WRAPPED, so a literal grep 'PresenceTracker tick' MISSES it. Use the
           regex 'PresenceTracker.+tick'. The device_status field ref is at :240."

- file: /home/dustin/projects/qmkonnect/spec/HOST_RULES.md
  why: "Hunk #6 (§13 Risks R6 — the proto-v1 exception detail referencing PresenceTracker)."
  section: "13. Risks & Open Questions (R6)"
  gotcha: "R6 is labelled 'RESOLVED' at :616; the proto-v1 per-probe reset detail is :626;
           the PresenceTracker cross-reference is :627. All three sub-phrases must be present."

# REFERENCE — the sibling PRP whose green gate run is S2's prerequisite (context only)
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/P1M1T1S1/PRP.md
  why: "S1 runs the quality gates + confirms the 5 named tests pass. S2 is the spec-side
        counterpart (read-only). S2 does NOT depend on S1's commands, but a green S1 is the
        signal that the code half is verified; S2 verifies the spec half."
  section: "Verified baseline"

# REFERENCE — research notes for this subtask (the independent grep results + hunk #5 nuance)
- docfile: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/P1M1T1S2/research/notes.md
  why: "§2 = the independently-verified 6-row table with verbatim fragments. §3 = the hunk #5
        backtick-grep nuance (the single most likely one-pass error). §4 = the incidental
        device_connected observation (NOT a hunk failure). §5 = the exact greps to run."
  section: "§2 the table", "§3 hunk #5 nuance", "§5 exact greps"
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/                       # THIS repo (HEAD = f4315a6; delta shipped at d240b27)
├── spec/
│   ├── DEVICE_DISCOVERY.md      # AUDIT SUBJECT — hunks #1, #2, #3
│   ├── LINUX.md                 # AUDIT SUBJECT — hunks #4, #5
│   └── HOST_RULES.md            # AUDIT SUBJECT — hunk #6
└── plan/006_8f4080956ee0/architecture/verification_findings.md   # §3 = prior assertion (re-confirm independently)
```

### Desired Codebase tree with files to be modified

```bash
( NONE — this is a read-only audit. No spec or source edits. `git status --short spec/ src/` clean. )
```

> The only artifact is the 6-row PASS/FAIL report you produce (and any scratch grep
> output). Do not commit report files into the repo.

### Known Gotchas of our codebase & Library Quirks

```text
# CRITICAL: hunk #5's "PresenceTracker tick" is BACKTICK-WRAPPED in the spec file.
#   spec/LINUX.md:236 reads "drive a `PresenceTracker` tick" (inline code span). A literal
#   grep -nF 'PresenceTracker tick' MISSES it (the "` " between the words breaks the match)
#   and would wrongly report hunk #5 as FAIL. USE THE REGEX FORM:
#     grep -nE 'PresenceTracker.+tick' spec/LINUX.md      # matches :236
#   (the .+ eats the backtick + space). This is the single most likely one-pass error.

# CRITICAL: do NOT count the incidental device_connected pseudo-code as a hunk failure.
#   LINUX.md §7.1's spawn() bullet (~line 230) shows `QmkTray { device_connected, dark_mode }`
#   at the construction site, while the field was renamed `device_status` in code
#   (src/linux_tray.rs:85) and the spec's update site uses `t.device_status` (:240). That
#   construction-site pseudo-code MAY be a stale field name — BUT it is NOT one of the 6
#   contracted hunks (hunk #5's contract is only "PresenceTracker tick" + "device_status",
#   both PRESENT). Hunk #5 verdict = PASS. Note the observation in the report's "incidental
#   observations"; do NOT edit the spec (S2 is read-only; a fix is a separate doc task).

# CRITICAL: S2 is READ-ONLY. Do not edit any spec file or source file.
#   If a hunk were genuinely missing (research says none are), the action is to FLAG it for
#   a separate doc task (e.g. P1.M1.T2.S1), NOT to patch the spec here. `git status` for
#   spec/ and src/ must stay clean.

# NOTE: line numbers are anchors, not contracts.
#   The table's line numbers are the verified results from this research session. A later
#   commit could shift them by ±a few. The gate is "the key phrase is present somewhere in
#   the file", not "at exact line N". Record the actual line(s) your grep returns.

# NOTE: hunk #1's process_full_message deactivation is in the proto-v1 caveat block (:90),
#   not the §2.2 lead. grep the whole file for 'process_full_message' — expect :85 (the
#   "no side effect" line) AND :90 (the 'process_full_message("")' deactivation line).

# NOTE: hunks #4's BindsTo/Restart appear 3x each in LINUX.md.
#   The systemd template block (:179, :186), the bullet explanation (:199, :201), and the
#   trayless-caveat block (:216, :217). Hunk #4 specifically targets the trayless caveat
#   (§6.2, :211-219); any of the hits confirms the wording is present. Record the §6.2 hit.

# NOTE: agreement with verification_findings.md §3 IS the success signal.
#   S2's purpose is independent re-confirmation. If your greps agree with §3 (all 6 present),
#   verdict = ZERO DRIFT. If a grep DISAGREES (a phrase missing that §3 claims present), that
#   is a real finding — investigate (re-read the spec section; check for a wording variant)
#   before declaring DRIFT.
```

## Implementation Blueprint

### Data models and structure

Not applicable — no data models, no code. This is a grep-and-report audit.

### Implementation Tasks (ordered: one grep per hunk, then assemble the table)

```yaml
Task 1: CONFIRM the working tree is the shipped delta (static audit target)
  - RUN: git -C /home/dustin/projects/qmkonnect status --short spec/ src/
  - EXPECT: empty (clean). The spec files are at HEAD (delta shipped at d240b27). If spec/ is
          dirty, STOP — the audit must run against committed spec files.

Task 2: HUNK #1 — DEVICE_DISCOVERY.md §2.2
  - RUN: grep -nE 'No proto-v2 or pure-VIA' spec/DEVICE_DISCOVERY.md        → expect :81
  - RUN: grep -nF 'process_full_message("")' spec/DEVICE_DISCOVERY.md       → expect :90
  - RECORD: Found=Y, lines 81 + 90 (also 85 for the "no side effect" line).

Task 3: HUNK #2 — DEVICE_DISCOVERY.md §2.4
  - RUN: grep -nE 'Handshake . cache warm-feed scope' spec/DEVICE_DISCOVERY.md   → expect :155
  - RUN: grep -nE 'correct only when a single Tier-1 board' spec/DEVICE_DISCOVERY.md → expect :159
  - RECORD: Found=Y, lines 155 + 159.

Task 4: HUNK #3 — DEVICE_DISCOVERY.md §3
  - RUN: grep -nE 'capable-keyed' spec/DEVICE_DISCOVERY.md                  → expect :189
  - RUN: grep -nE 'PresenceTracker' spec/DEVICE_DISCOVERY.md                → expect :97 and :189
  - RUN: grep -nE 'only when the path set changes' spec/DEVICE_DISCOVERY.md → expect :191
  - RECORD: Found=Y, lines 189 (+ 97) + 191.

Task 5: HUNK #4 — LINUX.md §6.2
  - RUN: grep -nE 'Trayless \(.--no-default-features.\) build caveat' spec/LINUX.md → expect :211
  - RUN: grep -nE 'BindsTo=dev-qmkonnect_device.device' spec/LINUX.md       → expect :216 (also 179,199)
  - RUN: grep -nE 'Restart=always' spec/LINUX.md                            → expect :217 (also 186,201)
  - RECORD: Found=Y, lines 211 + 216 + 217 (the §6.2 trayless-caveat hits).

Task 6: HUNK #5 — LINUX.md §7.1  (⚠ USE THE REGEX-TOLERANT FORM — Gotchas)
  - RUN: grep -nE 'PresenceTracker.+tick' spec/LINUX.md   → expect :236   (NOT the literal 'PresenceTracker tick')
  - RUN: grep -nE 't\.device_status' spec/LINUX.md        → expect :240
  - RECORD: Found=Y, lines 236 + 240.
  - NOTE the incidental `device_connected` pseudo-code at ~:230 in the report's observations (NOT a failure).

Task 7: HUNK #6 — HOST_RULES.md §13 R6
  - RUN: grep -nE 'R6 .+ Legacy handshake side effect' spec/HOST_RULES.md             → expect :616
  - RUN: grep -nE 'proto-v1 firmware it can briefly reset the layer' spec/HOST_RULES.md → expect :626
  - RUN: grep -nE 'see .PresenceTracker.' spec/HOST_RULES.md                          → expect :627
  - RECORD: Found=Y, lines 616 + 626 + 627.

Task 8: ASSEMBLE the 6-row table + verdict
  - PRODUCE: the table (Hunk # | Spec file | Key phrase searched | Found Y/N | Line number) — see "What".
  - STATE: overall verdict = ZERO SPEC DRIFT (all 6 Found=Y; research baseline).
  - INCLUDE: an "Incidental observations" note re: LINUX.md:~230 device_connected pseudo-code
          (not a hunk failure; candidate for a future doc task).
  - DO NOT: edit any spec or source file. Re-run `git status --short spec/ src/` → must be clean.
```

### Implementation Patterns & Key Details

```bash
# === THE HUNK #5 REGEX (the one non-obvious grep) ===
#   The spec wraps PresenceTracker in backticks: "drive a `PresenceTracker` tick".
#   Literal grep MISSES; regex with .+ eats the "` " between the words:
grep -nE 'PresenceTracker.+tick' spec/LINUX.md        # ✅ :236
#   Compare (do NOT use — fails):
grep -nF 'PresenceTracker tick' spec/LINUX.md         # ❌ no match (backtick breaks literal)

# === HUNK #2's ARROW ===
#   The spec heading is "Handshake → cache warm-feed scope." (→ is U+2192). grep with . for the arrow:
grep -nE 'Handshake . cache warm-feed scope' spec/DEVICE_DISCOVERY.md   # ✅ :155

# === WHAT "INDEPENDENT RE-CONFIRMATION" MEANS ===
#   verification_findings.md §3 already lists all 6. S2's job is to RE-GREP the files directly
#   (not transcribe §3) and confirm agreement. If a grep disagrees with §3, investigate before
#   declaring drift (likely a wording variant or the hunk #5 backtick issue, not real drift).

# === THE REPORT IS THE DELIVERABLE ===
#   A 6-row table + verdict. No file edits. No commit of report artifacts. `git status` clean.
```

### Integration Points

```yaml
SOURCE FILES:
  - modify: "NONE. Read-only audit. `git status --short spec/ src/` must stay clean."

AUDIT SUBJECTS (grep targets):
  - spec/DEVICE_DISCOVERY.md   # hunks #1, #2, #3
  - spec/LINUX.md              # hunks #4, #5
  - spec/HOST_RULES.md         # hunk #6

DEPENDENCIES / BUILD:
  - none. Pure grep. No cargo, no build, no tests.

UPSTREAM CONTEXT:
  - verification_findings.md §3: "the prior assertion S2 re-confirms independently (must agree)."
  - commit d240b27: "the shipped delta (code + spec); the static audit target."

DOWNSTREAM CONSUMERS:
  - P1.M1.T1.S3: "Caveat-backing code audit (read-only) — assumes spec is v6-correct (green S2)."
  - P1.M1.T2.S1: "README/docs overview staleness audit — syncs FROM the spec; trusts green S2."

OUT OF SCOPE:
  - Editing any spec file (even the incidental device_connected pseudo-code) — S2 is read-only.
  - Running quality gates or tests (that is S1).
  - Code-side caveat-backing audit (that is S3).
```

## Validation Loop

> The Validation Loop for THIS subtask IS the 6 greps (Tasks 2–7) + the no-edit invariant.
> The levels below are the report-production checks.

### Level 1: Each hunk's grep returns the expected hit (the audit itself)

```bash
cd /home/dustin/projects/qmkonnect

# Run the 6-hunk grep set (see Implementation Tasks for the exact commands). Expected (research baseline):
#   #1 → :81 + :90    #2 → :155 + :159    #3 → :189 (+:97) + :191
#   #4 → :211 + :216 + :217    #5 → :236 + :240    #6 → :616 + :626 + :627
# Every grep MUST print at least one line. A grep that prints nothing = that hunk FAILS
# (investigate: wording variant? hunk #5 backtick issue? real drift?).
```

### Level 2: Agreement with verification_findings.md §3 (the independence check)

```bash
cd /home/dustin/projects/qmkonnect
# §3's asserted line numbers (for cross-reference):
#   #1 :81/:87-97   #2 :155-164   #3 :189   #4 :211-219   #5 :236/:240   #6 :616-627
# Your greps should land on the same passages (exact line may differ by ±a few on a later commit;
# the gate is "same passage present", not "exact line N"). Agreement ⇒ ZERO DRIFT verdict.
```

### Level 3: No-edit invariant (read-only audit)

```bash
cd /home/dustin/projects/qmkonnect
git status --short spec/ src/
# Expected: empty (clean). An audit must not modify the spec or source. If anything appears here,
# revert it — S2 is read-only.
```

### Level 4: Failure triage (only if a hunk reported FAIL)

```text
If a grep printed nothing (hunk appears missing), classify before declaring DRIFT:
1. Re-read the spec section the hunk targets (§2.2/§2.4/§3/§6.2/§7.1/§13 R6) — the passage may use
   a wording variant. Try a broader grep (e.g. for hunk #5, `grep -niE 'tick|presence' spec/LINUX.md`).
2. For hunk #5 specifically: confirm you used the regex `PresenceTracker.+tick`, NOT the literal.
3. Confirm you're grepping the right file (hunk→file map: #1/#2/#3=DEVICE_DISCOVERY, #4/#5=LINUX,
   #6=HOST_RULES).
4. Only if the passage is genuinely absent after (1)-(3): declare DRIFT FOUND for that hunk, capture
   the spec section's actual text, and flag it for a separate doc task (P1.M1.T2.S1). Do NOT edit.
(Research baseline: no hunk reaches this step — all 6 PASS.)
```

## Final Validation Checklist

### Technical Validation
- [ ] Task 1: working tree clean for `spec/` and `src/` (audit against committed files).
- [ ] Tasks 2–7: all 6 hunks' greps return ≥1 hit (research baseline: all Y).
- [ ] Hunk #5 grep used the regex `PresenceTracker.+tick` (NOT the literal) → :236.
- [ ] Level 2: grep results agree with `verification_findings.md` §3 (independence confirmed).

### Feature Validation
- [ ] The 6-row table is produced (Hunk # | Spec file | Key phrase | Found Y/N | Line number).
- [ ] All 6 hunks report `Found = Y`.
- [ ] Overall verdict stated: **ZERO SPEC DRIFT**.
- [ ] Incidental `device_connected` observation (LINUX.md:~230) noted, NOT counted as a failure.

### Code Quality Validation
- [ ] `git status --short spec/ src/` → clean (no spec/source modified).
- [ ] No report/log artifacts committed to the repo.

### Documentation & Deployment
- [ ] DOCS = none per contract (read-only audit; no documentation edit).
- [ ] The 6-row PASS/FAIL table + verdict IS the deliverable (capture it in the report).

---

## Anti-Patterns to Avoid

- ❌ Don't use the literal grep `'PresenceTracker tick'` for hunk #5 — the spec wraps
  `PresenceTracker` in backticks ("`PresenceTracker` tick"), so the literal MISSES. Use the
  regex `PresenceTracker.+tick`. (The single most likely one-pass error.)
- ❌ Don't transcribe `verification_findings.md` §3 — S2's contract is *independent*
  re-confirmation via direct grep. Agreement is the success signal; transcribing defeats the
  purpose.
- ❌ Don't count the incidental `device_connected` pseudo-code at LINUX.md:~230 as a hunk #5
  failure — it is NOT one of the 6 contracted hunks; hunk #5 passes on its two key phrases.
  Note it as an observation; a fix (if warranted) is a separate doc task.
- ❌ Don't edit any spec or source file — S2 is a read-only audit. If a hunk were genuinely
  missing (it isn't), the action is to FLAG it, not patch it. `git status` must stay clean.
- ❌ Don't treat exact line numbers as contracts — they're anchors. A later commit can shift
  them by ±a few. The gate is "the key phrase is present in the file", verified by ≥1 grep hit.
- ❌ Don't grep only the §-lead paragraph for hunk #1's `process_full_message("")` — it lives in
  the proto-v1 caveat block (:90), not the §2.2 lead. grep the whole file.
- ❌ Don't run quality gates or tests here — that is S1's scope. S2 is grep-only.
- ❌ Don't conflate "the spec carries the wording" (S2, this audit) with "the code implements
  it" (S3, the caveat-backing code audit). They are separate read-only audits.
- ❌ Don't declare DRIFT without triage — a missing grep hit is usually a wording variant or the
  hunk #5 backtick issue, not real drift. Re-read the section + try a broader grep first.
- ❌ Don't commit the report/log artifacts into the repo.

---

**Confidence Score: 10/10** for one-pass execution success. This is a read-only grep audit
whose 6 commands are quoted with verified-expected line numbers, whose one non-obvious grep
(hunk #5's backtick-wrapped `PresenceTracker`) is called out with the exact regex fix, and
whose result is a **verified baseline run during research** (all 6 hunks PASS — ZERO SPEC
DRIFT — by direct grep against the spec files, agreeing with `verification_findings.md` §3).
The deliverable is a 6-row table + verdict; no edits, no build, no tests. The only residual
risk (a literal-grep false-FAIL on hunk #5) is pre-empted in the Gotchas and Anti-Patterns.
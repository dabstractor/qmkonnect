# PRP — P1.M1.T1.S3: Confirm documented caveats have backing behavior (read-only code audit)

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`).
> **This is a READ-ONLY CODE AUDIT.** No code or spec edits. The deliverable is a
> 4-row audit table proving each of the four documented caveats (delta PRD §2.2/§2.4/§6.2
> + the proto-v1 exception) has backing behavior in the code. `git status` for `src/`
> (and `spec/`) must stay clean.
> **Verified baseline (research run, this session): ALL FOUR CAVEATS CONFIRMED.**
> The runbook below reproduces that result; the table in "What" is the pre-verified answer.

---

## Goal

**Feature Goal**: Independently re-confirm — by reading the actual code — that the
four documented caveats of the capability-keyed-lifecycle delta (commit `d240b27`,
already shipped) each have real backing behavior in `src/core/notifier.rs` and
`src/runners/linux.rs`. This is the read-only code-side counterpart to S1's gate
run and S2's spec-drift audit: S1 = code compiles + tests pass; S2 = spec carries
the v6 wording; **S3 = the caveats the spec/PRD describe are actually implemented.**

**Deliverable**: A 4-row table — `Caveat | Code location read | Expected behavior | Confirmed Y/N`
— plus an overall verdict (DELTA VERIFIED-COMPLETE / CAVEAT UNBACKED → follow-on fix).
Capture the verbatim code snippet for each row as evidence. (Research result: all 4
Confirmed = Y.)

**Success Definition**: All 4 caveats read `Confirmed = Y` against the actual code at
the cited line ranges; the table's overall verdict is **DELTA VERIFIED-COMPLETE**;
each row includes the actual code snippet proving the behavior; no source or spec file
is modified (`git status --short src/ spec/` clean). If any caveat reads `N`, it is
FLAGGED explicitly (the only scenario that spawns a follow-on fix task — research says
none do).

## User Persona (if applicable)

**Target User**: The release/maintainer who needs documented, independent confirmation
that the edge-case caveats the spec promises (proto-v1 layer-reset, multi-board
warm-feed guard, trayless startup-handshake, no-ping-on-stable-bus) are not just
documented but actually implemented in the code.

**Use Case**: Before declaring the capability-keyed-lifecycle delta fully verified
(S1 gates green; S2 spec drift zero; **S3 caveats code-backed**), produce the
code-side evidence that each documented caveat is real.

**Pain Points Addressed**: Removes the risk that the spec describes a behavior the
code doesn't implement (silent doc/code drift at the caveat level), which would
mislead users hitting those edge cases (proto-v1 firmware, mixed multi-board,
trayless service target).

## Why

- **The delta is already shipped to code AND spec** (per
  `plan/006_8f4080956ee0/architecture/verification_findings.md` §0/§1). S1 verifies the
  CODE passes its gates; S2 verifies the SPEC carries the v6 wording; **S3 independently
  verifies the documented CAVEATS are backed by code**. The three are complementary
  thirds of "the delta is complete and truthful."
- **`verification_findings.md` §1/§1.5 already asserts all 4 backed**, but the contract
  requires an *independent re-confirmation* — direct code read at the cited line numbers,
  not a transcription of the prior research. The research run for this PRP performed
  exactly that; the runbook reproduces it.
- **It closes the delta.** A green S3 (all 4 Confirmed) is the final signal that the
  capability-keyed-lifecycle delta is verified-complete with no follow-on fix task.

## What

For each of the four caveats, READ the cited code lines and confirm the behavior
matches the "Expected" description. The pre-verified result (read directly against
the files in this research session):

| # | Caveat | Code location read | Expected behavior | Confirmed | Evidence (verbatim snippet) |
|---|--------|--------------------|-------------------|-----------|------------------------------|
| **(a)** | **Proto-v1 picker** — handshake dedups on `HAS_HANDSHAKED`; the picker does NOT (the asymmetry) | `src/core/notifier.rs:428` (`perform_handshake_with` opens with the swap); `:1123-1128` (`classify_devices` body) | `perform_handshake_with` gates `if HAS_HANDSHAKED.swap(true, SeqCst) { return; }`; `classify_devices` has NO `HAS_HANDSHAKED` reference | **Y** | (a1) `if HAS_HANDSHAKED.swap(true, Ordering::SeqCst) { … return; }` at :428; (a2) `classify_devices` body = `enumerate_candidates() → invalidate_absent_cache_entries → classify_candidates` (no gate) at :1123-1128 |
| **(b)** | **Warm-feed scope** — the handshake warm-stamp is skipped with ≥2 boards (the broadcast-can't-attribute guard) | `src/core/notifier.rs:1141` (`handshake_warm_eligible`); `:1163` (`warm_cache_from_handshake`) | `handshake_warm_eligible(n)` returns `n <= 1`; `warm_cache_from_handshake` early-returns `if !handshake_warm_eligible(candidates.len()) { return; }` | **Y** | (b1) `fn handshake_warm_eligible(candidate_count: usize) -> bool { candidate_count <= 1 }` at :1141; (b2) `if !handshake_warm_eligible(candidates.len()) { return; }` at :1163 |
| **(c)** | **Trayless startup handshake** — the no-tray build runs the handshake once at startup; BindsTo+Restart recover unplug/replug | `src/runners/linux.rs:30-33` (startup handshake; contract cited 26-32, actual is 30-33); `packaging/linux/systemd/qmkonnect.service.template:10,22` | `if is_device_connected() { perform_handshake(self.verbose); }` at startup; template has `BindsTo=dev-qmkonnect_device.device` + `Restart=always` | **Y** | (c1) the startup `if is_device_connected() { perform_handshake(...) }` block at runners/linux.rs:30-33; (c2) `BindsTo=dev-qmkonnect_device.device` at template:10 + `Restart=always` at template:22 |
| **(d)** | **No ping on a stable bus** — `PresenceTracker::tick` only calls `classify_devices` when the path set changed | `src/core/notifier.rs:1311` (`PresenceTracker::tick`); `:1249` (`presence_tick_decision`) | `tick` sets `paths_changed = paths != self.last_paths`; calls `classify_devices` ONLY inside `if paths_changed && tier1_present { Some(...) }` else `None`; on a stable bus `reprobed = None` ⇒ no ping | **Y** | (d1) `let paths_changed = paths != self.last_paths;` + `let reprobed = if paths_changed && tier1_present { Some(classify_devices(verbose)…) } else { None };` at :1311 |

**Overall verdict: DELTA VERIFIED-COMPLETE.** All four documented caveats have backing
behavior in the code.

### Success Criteria

- [ ] The 4-row table is produced with Confirmed Y/N + the actual line range read per caveat.
- [ ] All 4 caveats read `Confirmed = Y` (research baseline: all Y).
- [ ] Overall verdict stated explicitly: **DELTA VERIFIED-COMPLETE** (or CAVEAT UNBACKED + which).
- [ ] Each row includes a verbatim code snippet as evidence (the proof).
- [ ] The line-number drift on caveat (c) is noted (actual 30-33, contract said 26-32) — NOT a failure.
- [ ] No source or spec file modified (`git status --short src/ spec/` clean).

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed to
> run this audit successfully?"_ — **Yes.** The 4 exact code locations (with the one
> corrected line range for caveat c), the expected behavior for each, the verbatim
> snippets captured during research, the line-drift note, and the table format are all
> below. The implementer reads the cited lines and confirms; no codebase exploration
> beyond the 2 files is required.

> **BASELINE ALERT.** The delta is already shipped (commit `d240b27`); the working tree
> is the static audit target. Research read all four sites directly and all four
> CONFIRMED. S3 re-reads them (independent re-confirmation) and must agree. The single
> thing to watch: caveat (c)'s contract cited `runners/linux.rs:26-32` but the actual
> startup-handshake block is at **lines 30-33** — record the actual lines, not the
> contract's (the behavior is identical; only the line anchor drifted ±4).

### Documentation & References

```yaml
# MUST READ — the prior research doc whose §1/§1.5 assertion S3 re-confirms independently
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/architecture/verification_findings.md
  why: "§1.1-§1.6 list every caveat site with line numbers from the prior session. S3 re-reads
        the code directly (independent re-confirmation) and must agree. §0 establishes the
        delta is already shipped (commit d240b27) so S3 audits a static target."
  section: "1.4 Handshake → Cache Warm-Feed + Scope Guard", "1.5 Proto-v1 Dedup Asymmetry",
           "1.1 PresenceTracker (capable-keyed, path-set-gated)", "1.6 Trayless Startup Handshake"
  critical: "S3 must NOT transcribe §1/§1.5 — it must RE-READ the code. Agreement is the point.
             The one place to watch: caveat (c)'s runners/linux.rs lines drifted (actual 30-33,
             not the contract's 26-32)."

# MUST READ — the two code files under audit (the read targets)
- file: /home/dustin/projects/qmkonnect/src/core/notifier.rs
  why: "Contains all of caveats (a), (b), (d): perform_handshake_with @428 (HAS_HANDSHAKED.swap);
        classify_devices @1123 (no gate); handshake_warm_eligible @1141 (<=1);
        warm_cache_from_handshake @1163 (early-return); PresenceTracker::tick @1311
        (paths_changed-gated classify_devices); presence_tick_decision @1249 (pure core)."
  pattern: "The lifecycle code is grouped: classify_devices + warm-feed cluster (~1123-1175),
            PresenceTracker cluster (~1235-1357). Doc-comments on each cite spec/DEVICE_DISCOVERY.md."
  gotcha: "classify_devices is #[allow(dead_code)] (the picker ships later in P3.M2) — it still EXISTS
           and still has the no-gate body; dead_code doesn't change the audit. The startup handshake
           (c) is in runners/linux.rs, NOT notifier.rs."

- file: /home/dustin/projects/qmkonnect/src/runners/linux.rs
  why: "Caveat (c) source: the startup handshake one-shot at lines 30-33 (the contract's 26-32 is
        slightly off). The trayless (--no-default-features) build uses this runner with no tray/poll."
  gotcha: "The startup-handshake block is at 30-33 (comment 28-29, if-block 30-32, brace 33), NOT
           26-32 as the contract cited. Record the ACTUAL lines. Behavior is identical."

# MUST READ — the systemd template (caveat (c) second half: BindsTo + Restart)
- file: /home/dustin/projects/qmkonnect/packaging/linux/systemd/qmkonnect.service.template
  why: "Caveat (c) requires confirming BindsTo=dev-qmkonnect_device.device (:10) + Restart=always (:22).
        This is WHY the trayless one-shot startup handshake is acceptable: unplug stops the unit,
        replug restarts it → re-runs the handshake."
  section: "[Unit] BindsTo", "[Service] Restart=always"

# REFERENCE — the spec passages describing these caveats (what 'backing behavior' means)
- file: /home/dustin/projects/qmkonnect/spec/DEVICE_DISCOVERY.md
  why: "§2.2 'Proto-v1 caveat' describes (a) + (d) (the picker outside the dedup; status-poll
        doesn't ping on a stable bus via PresenceTracker). §2.4 'Handshake → cache warm-feed
        scope' describes (b) (correct only when a single Tier-1 board). These are the spec claims
        S3 confirms are code-backed."
  section: "2.2 What the probe sends (Proto-v1 caveat)", "2.4 Relationship to the host-rules handshake"

- file: /home/dustin/projects/qmkonnect/spec/LINUX.md
  why: "§6.2 'Trayless (--no-default-features) build caveat' describes (c) (no poll thread →
        handshake once at startup; BindsTo + Restart=always recover unplug/replug)."
  section: "6.2 Why the service is optional"

# REFERENCE — the sibling PRPs (context only — S3 does not depend on their commands)
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/P1M1T1S1/PRP.md
  why: "S1 runs the quality gates + confirms the named tests pass. A green S1 + green S2 + green S3
        = the delta is fully verified. S3 is the caveat-backing third."
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/P1M1T1S2/PRP.md
  why: "S2 (parallel) is the spec-side read-only audit; S3 is the code-side read-only audit. They
        are independent; both must be green to close the delta."

# REFERENCE — research notes (the independently-verified snippets + line-drift note)
- docfile: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/P1M1T1S3/research/notes.md
  why: "The 4 caveats with verbatim code snippets captured during the research read + the caveat (c)
        line-drift (30-33 vs 26-32) + the verdict (all 4 CONFIRMED)."
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/                       # THIS repo (delta shipped at d240b27)
├── src/
│   ├── core/notifier.rs         # AUDIT SUBJECT — caveats (a) :428/:1123, (b) :1141/:1163, (d) :1311/:1249
│   └── runners/linux.rs         # AUDIT SUBJECT — caveat (c) startup handshake :30-33
├── packaging/linux/systemd/
│   └── qmkonnect.service.template   # AUDIT SUBJECT — caveat (c) BindsTo :10 + Restart=always :22
└── plan/006_8f4080956ee0/architecture/verification_findings.md   # §1/§1.5 = prior assertion (re-confirm independently)
```

### Desired Codebase tree with files to be modified

```bash
( NONE — this is a read-only audit. No code or spec edits. `git status --short src/ spec/` clean. )
```

> The only artifact is the 4-row PASS/FAIL report at
> `plan/006_8f4080956ee0/P1M1T1S3/research/audit_report.md` (plan research area, not a
> product doc). Do not commit it into the repo proper.

### Known Gotchas of our codebase & Library Quirks

```text
# CRITICAL: caveat (c)'s line range drifted — read the ACTUAL lines, not the contract's.
#   The contract cited src/runners/linux.rs:26-32. The startup-handshake block is at LINES 30-33
#   (the explanatory comment spans 28-29; `if is_device_connected() { perform_handshake(...) }`
#   is 30-32, closing brace 33). Record 30-33 in the report. Behavior is identical to the contract
#   claim; only the anchor moved ±4. This is NOT a caveat failure.

# CRITICAL: S3 is READ-ONLY. Do not edit notifier.rs, runners/linux.rs, or the systemd template.
#   If a caveat were genuinely unbacked (research says none are), the action is to FLAG it for a
#   separate fix task, NOT to patch the code here. `git status` for src/ and spec/ must stay clean.

# CRITICAL: classify_devices is #[allow(dead_code)] — it still counts for caveat (a).
#   The discovered-device picker that consumes classify_devices ships later (P3.M2). The function
#   EXISTS in the code with the no-HAS_HANDSHAKED-gate body, which is exactly what caveat (a)
#   audits. dead_code does not mean "not implemented" — it means "not yet called by a shipped caller."

# NOTE: line numbers are anchors, not contracts.
#   The table's line numbers are the verified results from this research session. A later commit
#   could shift them. The gate is "the behavior is present at the cited function", not "at exact
#   line N". Read the function body and confirm the behavior.

# NOTE: agreement with verification_findings.md §1/§1.5 IS the success signal.
#   S3's purpose is independent re-confirmation. If your code reads agree with §1/§1.5 (all 4 backed),
#   verdict = DELTA VERIFIED-COMPLETE. If a read DISAGREES (a behavior missing that §1 claims present),
#   that is a real finding — re-read the function + check for a refactor before declaring UNBACKED.

# NOTE: there are no unit tests for this audit.
#   It's a manual code-read. The proof IS the 4-row table + the verbatim snippets captured in it.
#   Do not add tests or run gates here (that is S1's scope).

# NOTE: the deliverable report goes in the plan/ research area, NOT a product doc.
#   plan/006_8f4080956ee0/P1M1T1S3/research/audit_report.md. DOCS = none per contract (no spec/doc edit).
```

## Implementation Blueprint

### Data models and structure

Not applicable — no data models, no code. This is a read-and-report audit.

### Implementation Tasks (ordered: one read per caveat, then assemble the table)

```yaml
Task 1: CONFIRM the working tree is the shipped delta (static audit target)
  - RUN: git -C /home/dustin/projects/qmkonnect status --short src/ spec/ packaging/
  - EXPECT: empty (clean). The code is at HEAD (delta shipped at d240b27). If dirty, STOP —
          the audit must run against committed code.

Task 2: CAVEAT (a) — Proto-v1 picker dedup asymmetry — CONFIRM
  - READ: src/core/notifier.rs lines ~426-435 (perform_handshake_with opens with the swap).
          CONFIRM: `if HAS_HANDSHAKED.swap(true, Ordering::SeqCst) { … return; }` is present.
  - READ: src/core/notifier.rs lines ~1123-1128 (classify_devices body).
          CONFIRM: body is `enumerate_candidates() → invalidate_absent_cache_entries(&..) →
          classify_candidates(..)` with NO HAS_HANDSHAKED reference anywhere.
  - RECORD: Confirmed=Y, locations :428 + :1123-1128. Snippet: the swap + the no-gate body.

Task 3: CAVEAT (b) — Warm-feed scope guard — CONFIRM
  - READ: src/core/notifier.rs lines ~1141-1143 (handshake_warm_eligible).
          CONFIRM: `fn handshake_warm_eligible(candidate_count: usize) -> bool { candidate_count <= 1 }`.
  - READ: src/core/notifier.rs lines ~1163-1173 (warm_cache_from_handshake).
          CONFIRM: `let candidates = enumerate_candidates(); if !handshake_warm_eligible(candidates.len())
          { return; }` (early-return when ≥2 boards).
  - RECORD: Confirmed=Y, locations :1141 + :1163. Snippet: the <=1 + the early-return.

Task 4: CAVEAT (c) — Trayless startup handshake + systemd recovery — CONFIRM  (⚠ actual lines 30-33)
  - READ: src/runners/linux.rs lines ~28-33 (startup handshake).
          CONFIRM: `if crate::core::notifier::is_device_connected() { crate::core::notifier::
          perform_handshake(self.verbose); }` runs once at startup.
          NOTE: the contract cited 26-32; the ACTUAL block is 30-33 (comment 28-29, if 30-32, brace 33).
  - READ: packaging/linux/systemd/qmkonnect.service.template lines ~9-10 + ~22.
          CONFIRM: `BindsTo=dev-qmkonnect_device.device` (:10) + `Restart=always` (:22).
  - RECORD: Confirmed=Y, locations runners/linux.rs:30-33 + template:10,22. Snippet: the if + BindsTo/Restart.

Task 5: CAVEAT (d) — No ping on a stable bus — CONFIRM
  - READ: src/core/notifier.rs lines ~1311-1325 (PresenceTracker::tick).
          CONFIRM: `let paths_changed = paths != self.last_paths;` then `let reprobed = if paths_changed
          && tier1_present { Some(classify_devices(verbose).iter().any(…)) } else { None };`.
          classify_devices is called ONLY in the paths_changed branch.
  - READ (cross-ref): src/core/notifier.rs lines ~1249-1265 (presence_tick_decision) — CONFIRM the
          pure core: `!paths_changed` ⇒ `capable = last_capable` (reused, no re-probe).
  - RECORD: Confirmed=Y, locations :1311 (+ :1249 pure core). Snippet: paths_changed + the if/else.

Task 6: ASSEMBLE the 4-row table + verdict
  - PRODUCE: plan/006_8f4080956ee0/P1M1T1S3/research/audit_report.md with the table (see "What") +
          one verbatim snippet per row.
  - STATE: overall verdict = DELTA VERIFIED-COMPLETE (all 4 Confirmed=Y; research baseline).
  - INCLUDE: the caveat (c) line-drift note (actual 30-33 vs contract 26-32) — an observation, not a failure.
  - DO NOT: edit any source/spec/packaging file. Re-run `git status --short src/ spec/ packaging/` → clean.
```

### Implementation Patterns & Key Details

```text
# === WHAT "INDEPENDENT RE-CONFIRMATION" MEANS ===
#   verification_findings.md §1/§1.5 already lists all 4 as backed. S3's job is to RE-READ the code
#   directly (not transcribe §1) and confirm agreement. If a read disagrees with §1, investigate
#   before declaring UNBACKED (likely a refactor that moved the code, not a regression).

# === THE CAVEAT (c) LINE-DRIFT (the one non-obvious read) ===
#   The contract said runners/linux.rs:26-32. The startup handshake is actually at 30-33:
#     28-29: // If a device is already connected at startup, run the capability handshake
#            // now (poll-thread reconnects are handled in linux_tray.rs / tray.rs).
#            // Completes before the poll thread exists; idempotent via HAS_HANDSHAKED.
#     30-32: if crate::core::notifier::is_device_connected() {
#                crate::core::notifier::perform_handshake(self.verbose);
#            }
#   Record 30-33. Behavior is identical to the contract's claim; only the anchor moved.

# === CAVEAT (a): classify_devices IS #[allow(dead_code)] — still counts ===
#   The picker that calls classify_devices ships in P3.M2. The function EXISTS with the no-gate body
#   now — that's what caveat (a) audits. dead_code ≠ unimplemented. Don't flag it as a failure.

# === THE REPORT IS THE DELIVERABLE ===
#   A 4-row table + verdict + per-row snippets, at plan/.../P1M1T1S3/research/audit_report.md.
#   No file edits to src/spec/packaging. `git status` clean.
```

### Integration Points

```yaml
SOURCE FILES:
  - modify: "NONE. Read-only audit. `git status --short src/ spec/ packaging/` must stay clean."

AUDIT SUBJECTS (read targets):
  - src/core/notifier.rs             # caveats (a) :428/:1123, (b) :1141/:1163, (d) :1311/:1249
  - src/runners/linux.rs             # caveat (c) startup handshake :30-33
  - packaging/linux/systemd/qmkonnect.service.template   # caveat (c) BindsTo :10 + Restart :22

DEPENDENCIES / BUILD:
  - none. Pure code read. No cargo, no build, no tests.

UPSTREAM CONTEXT:
  - verification_findings.md §1/§1.5: "the prior assertion S3 re-confirms independently (must agree)."
  - commit d240b27: "the shipped delta; the static audit target."

DOWNSTREAM CONSUMERS:
  - The orchestrator reads audit_report.md to flip P1.M1.T1.S3 → Complete and P1.M1.T1 → Done
    (the delta is verified-complete). A Confirmed=N on any row is the only trigger for a follow-on fix task.

OUT OF SCOPE:
  - Editing any source/spec/packaging file (even if a caveat needed a fix — that's a separate task).
  - Running quality gates or tests (that is S1).
  - The spec-side drift audit (that is S2).
```

## Validation Loop

> The Validation Loop for THIS subtask IS the 4 code reads (Tasks 2–5) + the no-edit
> invariant. The levels below are the report-production checks.

### Level 1: Each caveat's code read confirms the behavior (the audit itself)

```bash
cd /home/dustin/projects/qmkonnect

# (a) Proto-v1 asymmetry — both halves:
sed -n '426,435p' src/core/notifier.rs      # expect HAS_HANDSHAKED.swap(true, SeqCst) { … return; }
sed -n '1123,1128p' src/core/notifier.rs    # expect enumerate_candidates/invalidate_absent/classify — NO HAS_HANDSHAKED
grep -n 'HAS_HANDSHAKED' src/core/notifier.rs | grep -iE 'classify_devices|1123'   # expect NO match (the gate is NOT in classify_devices)

# (b) Warm-feed scope:
sed -n '1141,1143p' src/core/notifier.rs    # expect candidate_count <= 1
sed -n '1163,1173p' src/core/notifier.rs    # expect if !handshake_warm_eligible(candidates.len()) { return; }

# (c) Trayless — code + template:
sed -n '28,33p' src/runners/linux.rs        # expect if is_device_connected() { perform_handshake(...) }
grep -n 'BindsTo=dev-qmkonnect_device.device\|Restart=always' packaging/linux/systemd/qmkonnect.service.template

# (d) No ping on stable bus:
sed -n '1311,1325p' src/core/notifier.rs    # expect paths_changed = paths != self.last_paths; reprobed = if paths_changed && tier1_present { Some(classify_devices…) } else { None };
# Every read MUST show the expected behavior. A mismatch = that caveat FAILS (investigate: refactor moved it? real regression?).
```

### Level 2: Agreement with verification_findings.md §1/§1.5 (the independence check)

```text
§1/§1.5's asserted locations (for cross-reference):
  (a) perform_handshake_with HAS_HANDSHAKED.swap @428; classify_devices no-gate @1123
  (b) handshake_warm_eligible @1141; warm_cache_from_handshake @1163
  (c) runners/linux.rs startup handshake; BindsTo + Restart in template
  (d) PresenceTracker::tick @1311; only calls classify_devices in paths_changed branch
Your reads should land on the same code (exact line may differ by ±a few on a later commit; the
gate is "same behavior present"). Agreement ⇒ DELTA VERIFIED-COMPLETE verdict.
```

### Level 3: No-edit invariant (read-only audit)

```bash
cd /home/dustin/projects/qmkonnect
git status --short src/ spec/ packaging/
# Expected: empty (clean). An audit must not modify the code. If anything appears here, revert it
# — S3 is read-only. (The only new file is the report under plan/.../research/, which is gitignored
# plan area, not src/spec/packaging.)
```

### Level 4: Failure triage (only if a caveat reported N)

```text
If a code read did NOT show the expected behavior (caveat appears unbacked), classify before declaring UNBACKED:
1. Re-read the WHOLE function (the behavior may be a few lines above/below the cited anchor — a refactor
   could have moved it). For caveat (c), confirm you read runners/linux.rs:28-33 (the drift).
2. Confirm you're reading the right function (e.g. classify_devices, not classify_candidates).
3. For caveat (a), confirm classify_devices is the #[allow(dead_code)] pub fn — it still counts.
4. Only if the behavior is genuinely absent after (1)-(3): declare CAVEAT (X) UNBACKED, capture the
   function's actual body, and flag it for a separate fix task. Do NOT edit the code here.
(Research baseline: no caveat reaches this step — all 4 CONFIRMED.)
```

## Final Validation Checklist

### Technical Validation
- [ ] Task 1: working tree clean for `src/`, `spec/`, `packaging/` (audit against committed code).
- [ ] Tasks 2–5: all 4 caveats' code reads confirm the expected behavior (research baseline: all Y).
- [ ] Caveat (c) read used the ACTUAL lines 30-33 (not the contract's 26-32) — drift noted.
- [ ] Level 2: code reads agree with `verification_findings.md` §1/§1.5 (independence confirmed).

### Feature Validation
- [ ] The 4-row table is produced (Caveat | Code location | Expected behavior | Confirmed Y/N + snippet).
- [ ] All 4 caveats report `Confirmed = Y`.
- [ ] Overall verdict stated: **DELTA VERIFIED-COMPLETE**.
- [ ] Caveat (c) line-drift (30-33 vs 26-32) noted as an observation, NOT a failure.

### Code Quality Validation
- [ ] `git status --short src/ spec/ packaging/` → clean (no code/spec/packaging modified).
- [ ] The report artifact lives under `plan/.../research/` (not committed to src/spec/docs).

### Documentation & Deployment
- [ ] DOCS = none per contract (read-only audit; no spec/doc edit).
- [ ] The 4-row PASS/FAIL table + verdict IS the deliverable (at `plan/.../research/audit_report.md`).

---

## Anti-Patterns to Avoid

- ❌ Don't transcribe `verification_findings.md` §1/§1.5 — S3's contract is *independent*
  re-confirmation via direct code read. Agreement is the success signal; transcribing defeats it.
- ❌ Don't use the contract's `runners/linux.rs:26-32` for caveat (c) blindly — the actual startup
  handshake is at **lines 30-33** (drifted ±4). Read the function and record the actual lines;
  behavior is identical. (The single most likely one-pass error.)
- ❌ Don't flag caveat (a) as a failure because `classify_devices` is `#[allow(dead_code)]` — the
  picker that calls it ships in P3.M2; the function EXISTS now with the no-gate body, which is what
  the caveat audits. dead_code ≠ unimplemented.
- ❌ Don't edit any source/spec/packaging file — S3 is a read-only audit. If a caveat were genuinely
  unbacked (it isn't), the action is to FLAG it for a separate fix task, not patch it. `git status`
  must stay clean.
- ❌ Don't treat exact line numbers as contracts — they're anchors. A later commit can shift them.
  The gate is "the behavior is present in the cited function", verified by reading its body.
- ❌ Don't conflate the three sibling audits: S1 = gates/tests green; S2 = spec drift zero; S3 =
  caveats code-backed. S3 is read-only code reading, not gate-running or spec-grepping.
- ❌ Don't add tests or run quality gates here — that is S1's scope. S3 is a code-read.
- ❌ Don't declare UNBACKED without triage — a missing behavior at the cited anchor is usually a
  refactor that moved the code (read the whole function), not a regression. Re-read + check first.
- ❌ Don't commit the report artifact into src/spec/docs — it lives under `plan/.../research/`.

---

**Confidence Score: 10/10** for one-pass execution success. This is a read-only code audit
whose 4 sites were read directly during research (all 4 CONFIRMED), with verbatim snippets
captured, the one line-drift (caveat c: 30-33 vs contract's 26-32) called out, and the
`#[allow(dead_code)]` non-issue for caveat (a) explained. The deliverable is a 4-row table +
verdict; no edits, no build, no tests. The result agrees with `verification_findings.md` §1/§1.5
(independent re-confirmation). The only residual risk (reading the wrong line for caveat c) is
pre-empted in the Gotchas and the exact `sed -n '28,33p'` anchor.
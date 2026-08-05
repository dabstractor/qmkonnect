# PRP — P1.M1.T2.S1: Verify README.md + docs/ overview files are not stale vs the capability-keyed lifecycle

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`).
> **This is a DOCS-SIDE VERIFICATION (Mode B sweep).** The delta (the v6
> capability-keyed lifecycle: `PresenceTracker`, warm-feed scope, capable-keyed
> three-state status, `classify_devices`, trayless startup handshake) was **already
> shipped to code + spec** in commit `d240b27`; the user docs were **regenerated**
> in commit `293f565`. `verification_findings.md` §5 asserts "Mode B: NONE expected."
> **This subtask verifies that claim** by scanning the user docs for any prose that
> contradicts the v6 lifecycle semantics.
> **Verified baseline (research run, this session): VERIFIED — NO DRIFT.** The
> runbook below reproduces that result. If (and only if) a contradiction reappears
> on re-check, fix the affected overview doc **in place** (Mode B); otherwise the
> task closes as a **no-op** with a one-line verdict.
> **No `src/` or `spec/` edits under any branch** (S1/S2/S3 own code/spec). This is
> docs-only; `git status` for `src/ spec/` must stay clean.

---

## Goal

**Feature Goal**: Independently re-confirm — by scanning the actual user-facing
docs — that `README.md`, the `docs/*.md` overview files, and `docs/llms_full.txt`
contain **no prose that contradicts** the v6 capability-keyed-lifecycle semantics
(shipped in commit `d240b27`). Confirm the docs are either **accurate** (on the
user-visible three-state status + capable/no-module markers they DO describe) or
**silent** (on the internal mechanisms: PresenceTracker, classify_devices,
warm-feed, Tier-1/capable-keying, is_device_connected, single-ping-per-appearance,
broadcast internals). This is the docs third of "the delta is verified-complete"
(S1 = code gates green; S2 = spec drift zero; S3 = caveats code-backed;
**this = user docs not stale**).

**Deliverable**: A one-line verdict — **`VERIFIED: no documentation drift`**
(expected; the docs are silent on the internals and accurate on the user-visible
status) — OR, if a contradiction is found on re-check, a list of specific
file/line sites edited **in place** (Mode B) to remove the contradiction. Plus a
short evidence summary (the grep-scan counts + the spot-read confirmation) captured
under `plan/006_8f4080956ee0/P1M1T2S1/research/`.

**Success Definition**: The verdict is stated explicitly; the §2 grep scan
(internal-mechanism terms = 0 hits in user docs; 5 targeted drift signatures =
none) is reproduced and agrees with the research baseline; the three-state status
sections spot-read as `Connected`/`No module`/`Disconnected`; and either (no-drift
branch) **no doc file is modified** (`git status --short README.md docs/` clean) or
(drift-found branch) only the specific contradicted overview doc(s) are edited. No
`src/` or `spec/` file is ever touched.

## User Persona (if applicable)

**Target User**: The release/maintainer who needs documented, independent
confirmation that the published user docs (README + docs site) do not mislead a
user about how QMKonnect detects/classifies boards — specifically that they don't
promise behaviors the v6 lifecycle intentionally changed (Tier-1-keyed status,
unqualified single-ping, broadcast to every 0xFF60 interface, etc.).

**Use Case**: Before declaring the capability-keyed-lifecycle delta fully verified
(S1 gates; S2 spec; S3 caveats; **this = user docs**), confirm the docs a user
actually reads don't contradict the implementation.

**Pain Points Addressed**: Removes the risk of doc/code drift at the user-visible
layer — e.g. a doc that still says status is a boolean, or that the app pings every
poll, would mislead a user debugging a multi-board or no-module case.

## Why

- **The delta's user-visible surface is the three-state status** (Connected / No
  module / Disconnected), which F13/F14 introduced and the docs (commit `293f565`)
  already describe. The d240b27 **refinement** is internal (how that status is
  computed: capable-keyed, path-set-gated, transition-driven, warm-feed-scoped) —
  the kind of detail user docs are expected to be **silent** on. This task confirms
  that expectation rather than assuming it.
- **It closes the verification.** Green S1 + S2 + S3 + **this** = the delta is
  verified-complete across code, spec, caveats, AND user docs. `verification_findings.md`
  §5 already asserts "Mode B: NONE expected"; this task is the independent
  re-confirmation (same pattern as S3's independent code re-read).
- **It's cheap and self-verifying.** A grep scan + two spot-reads reproduce the
  verdict; no build, no tests, no code. The likely outcome (research-confirmed) is a
  no-op "VERIFIED."

## What

A two-step verification — (1) a grep scan for the internal-mechanism terms and the
five targeted drift signatures, (2) a spot-read of the three-state status sections —
then a verdict. The pre-verified result (scanned directly in this research session):

### Success Criteria

- [ ] The §2 grep scan is reproduced: internal-mechanism terms
      (`PresenceTracker`/`classify_devices`/`warm`/`is_device_connected`/`Tier-1`/`Tier-2`/
      `broadcast`) = **0 hits** in `README.md` + `docs/*.md` (excl. `llms_full.txt`).
- [ ] The 5 targeted drift signatures (§2b) return **none** (no Tier-1-keyed status
      claim, no two-state claim, no "every 0xFF60 interface" broadcast claim, no
      unqualified single-ping promise, no per-poll status-refresh claim).
- [ ] The three-state status sections (`docs/usage.md` ~112-122,
      `docs/troubleshooting.md` ~105-110, `docs/installation.md` ~218) spot-read as
      `● Device Connected` / `⚠ … no qmk_notifier module` / `○ No Device Connected`.
- [ ] Verdict stated: **`VERIFIED: no documentation drift`** (expected) OR specific
      file/line sites edited in place (drift-found branch).
- [ ] (No-drift branch) `git status --short README.md docs/` clean — no doc modified.
- [ ] (Either branch) `git status --short src/ spec/` clean — no code/spec touched.

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed
> to run this verification successfully?"_ — **Yes.** The six v6 semantics + the
> prose drift signature each would create, the exact grep commands (with verified
> expected outputs), the three-state-status sections to spot-read, the
> "docs/llms_full.txt is derived" note, the verdict criteria, and the no-edit-unless-drift
> contract are all below.

> **BASELINE ALERT.** The delta is already shipped (code+spec at `d240b27`; docs
> regenerated at `293f565`). Research scanned the live tree and found **no drift**
> (internal terms = 0 hits; 5 drift signatures = none; three-state status accurate).
> This task re-runs the scan independently and must agree. If a scan disagrees
> (a contradiction that research didn't find), investigate before editing — most
> likely the doc was edited by a later commit, in which case a Mode-B in-place fix
> is correct.

### Documentation & References

```yaml
# MUST READ — the prior research doc whose §5 asserts "Mode B: NONE expected"
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/architecture/verification_findings.md
  why: "§5 (Documentation Impact Assessment) states no Mode A or Mode B doc changes
        are expected and why (docs regenerated in 293f565; d240b27 is internal).
        §1/§3 list the verified code/spec sites so you know the v6 semantics are real
        (not vaporware the docs would be ahead of). This task independently confirms
        §5's 'NONE expected' claim against the actual docs."
  section: "5. Documentation Impact Assessment" (+ 1.1-1.6 for the v6 semantics)
  critical: "The docs are expected to be SILENT on the d240b27 internals (they're
             internal correctness mechanisms) and ACCURATE on the three-state status.
             'No drift' = silent-or-accurate, NOT 'mentions every internal'."

# MUST READ — the sibling PRPs (the other three verification thirds)
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/P1M1T1S1/PRP.md
  why: "S1 runs the quality gates + confirms the named PresenceTracker/warm-feed/
        classify tests pass. Establishes the code actually implements v6 (so the docs
        are checking against a real implementation, not a spec-only claim)."
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/P1M1T1S2/PRP.md
  why: "S2 confirms the SPEC carries the v6 wording (6 diff hunks → spec passages).
        The spec is the v6 source of truth; the user docs should not contradict it."
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/P1M1T1S3/PRP.md
  why: "S3 is the read-only code-side audit (caveats backed). This task is the
        docs-side analog: read-only verification (docs not stale). Same shape:
        independent re-confirmation of a research-verified 'clean' result."

# MUST READ — the v6 source-of-truth spec passages (what 'accurate' means for the user docs)
- file: /home/dustin/projects/qmkonnect/spec/DEVICE_DISCOVERY.md
  why: "§3 'Device-Status Semantics (three states)' is the table the user docs'
        three-state status must match: Connected (≥1 capable) / No module (≥1 Tier-1,
        0 capable) / Disconnected (0 Tier-1). If a user doc's status text disagrees
        with THIS table, that's drift."
  section: "3. Device-Status Semantics (three states)"
- file: /home/dustin/projects/qmkonnect/spec/UI.md
  why: "§4 'Device-Connection Status Indicator' restates the same three-state table
        the user docs cite. Cross-check the user docs' status wording against it."
  section: "4. Device-Connection Status Indicator"

# MUST READ — the files under verification (read/grep before concluding)
- file: /home/dustin/projects/qmkonnect/README.md
  why: "Top-level overview. The only lifecycle-adjacent mentions (research): the
        discovery signature 0xFF60/0x61 (:83, :198) and the capable/no-module picker
        markers (:217-220). Spot-read 205-225 to confirm the picker/status text is
        v6-accurate and makes no Tier-keyed/two-state/per-poll claim."
  gotcha: "README is the highest-traffic doc — but it is intentionally high-level and
           silent on PresenceTracker/warm-feed internals. 'Silent' is correct here."
- file: /home/dustin/projects/qmkonnect/docs/usage.md
  why: "Contains the user-facing three-state status section (~108-122). The canonical
        user-facing statement of the Connected/No-module/Disconnected semantics."
  section: "### Verify Keyboard Connection (~112)"
- file: /home/dustin/projects/qmkonnect/docs/troubleshooting.md
  why: "Three-state status callout (~105-110) under 'Keyboard Not Detected'. Must
        match the spec table."
- file: /home/dustin/projects/qmkonnect/docs/installation.md
  why: "Linux permission section status text (~218). Mentions the three-state icon
        in the permission-grant context."
- file: /home/dustin/projects/qmkonnect/docs/configuration.md
  why: "Two handshake/capable mentions to eyeball: the picker markers (:30,35-36) and
        the --list-callbacks row (:231, 'Handshake the connected keyboard…'). Both
        were accurate/neutral in research — confirm they still make no capable-keyed/
        warm-feed contradiction."

# REFERENCE — the derived concatenation (NOT hand-edited)
- file: /home/dustin/projects/qmkonnect/docs/llms_full.txt
  why: "A concatenation of docs/*.md produced by docs/generate_llms_full.sh;
        regenerated in 293f565. It CANNOT drift independently of docs/*.md — a clean
        docs/*.md ⇒ clean llms_full.txt. Include it in the grep scope as belt-and-
        suspenders (the item lists it as an input), but do NOT hand-edit it; if it
        were stale the fix is regeneration, not a patch."
  gotcha: "Do NOT edit llms_full.txt directly. The grep-against-it is a confirmation
           that the regeneration is current; it is not an edit target."

# REFERENCE — research notes (the independently-verified scan results + verdict)
- docfile: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/P1M1T2S1/research/notes.md
  why: "The full grep-scan table (internal terms = 0; 5 drift signatures = none),
        the false-positive ping hits, the v6-semantics→drift-signature map, and the
        VERIFIED verdict. The PRP runbook reproduces this."
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/                              # delta shipped: code+spec @ d240b27, docs @ 293f565
├── README.md                           # VERIFY (overview; picker/status markers)
├── docs/
│   ├── usage.md                        # VERIFY (three-state status section)
│   ├── troubleshooting.md              # VERIFY (three-state status callout)
│   ├── installation.md                 # VERIFY (status text in Linux perms)
│   ├── configuration.md                # VERIFY (picker markers + --list-callbacks handshake)
│   ├── qmk-integration.md              # VERIFY ("the capability handshake" mention)
│   ├── examples.md, index.md           # VERIFY (grep only — no lifecycle surface expected)
│   └── llms_full.txt                   # VERIFY-by-grep only (DERIVED; never hand-edit)
├── spec/                               # SOURCE OF TRUTH (v6 wording; S2 scope — DO NOT EDIT here)
└── src/                                # S1/S3 scope — DO NOT EDIT here
```

### Desired Codebase tree with files to be modified

```bash
# NO-DRIFT BRANCH (expected): NOTHING modified. Pure verification.
#   git status --short README.md docs/   → clean
#   git status --short src/ spec/        → clean

# DRIFT-FOUND BRANCH (only if a re-check finds a contradiction):
#   README.md            # and/or
#   docs/<overview>.md   # edited IN PLACE (Mode B) to remove the contradictory prose
#   (src/ and spec/ are NEVER touched by this task)
```

### Known Gotchas of our codebase & Library Quirks

```text
# CRITICAL: "no drift" means silent-OR-accurate, NOT "mentions every internal."
#   The d240b27 lifecycle is an INTERNAL correctness mechanism. User docs are EXPECTED
#   to be silent on PresenceTracker/classify_devices/warm-feed/Tier-keying (0 hits is
#   the correct result, not a gap). The user-visible surface they DO describe — the
#   three-state status — must be accurate. Verifying "0 internal-term hits + accurate
#   three-state status" IS the pass.

# CRITICAL: docs/llms_full.txt is DERIVED — never hand-edit it.
#   It's docs/generate_llms_full.sh concatenating docs/*.md. If it drifts, the fix is
#   regeneration (a separate step), not a patch. Grep it as a confirmation the
#   regeneration is current; treat a hit there as "docs/*.md has the same hit" (find
#   and fix the source doc, not llms_full.txt).

# CRITICAL: the 6 "ping|probe" hits are FALSE POSITIVES (substring matches).
#   "ping" appears inside Typing / keeping / skipping / Stopping. A naive grep -i ping
#   flags them; they are NOT device-probe mentions. The drift-signature grep (§2b #4)
#   uses the specific phrase "single ping / one ping / ping…once", which returns none.
#   Don't be fooled by the bare "ping" substring count.

# CRITICAL: ERE alternation uses '|', NOT '\|'.
#   In `grep -E`, '\|' is a literal pipe, not alternation — a multi-alternation pattern
#   with '\|' silently matches nothing. Use '|' (e.g. `grep -niE 'three-state|two-state'`),
#   or split into separate greps. (The research summary loop hit this; the detailed
#   greps used correct '|' and are the ground truth.)

# CRITICAL: this task NEVER edits src/ or spec/.
#   The code/spec verification is S1/S2/S3. Even in the drift-found branch, the fix is
#   to the user doc (README.md / docs/*.md) only. `git status --short src/ spec/` clean.

# GOTCHA: the three-state status is CORRECT — don't "fix" it.
#   usage.md:112-122 / troubleshooting.md:105-110 describe ● Connected / ⚠ No module /
#   ○ Disconnected — this is EXACTLY the v6 spec table. It is not drift. Only one of
#   the 5 specific drift signatures (Tier-1-keyed / two-state / every-0xFF60-broadcast /
#   unqualified-single-ping / per-poll-refresh) is a fix-worthy contradiction.

# NOTE: line numbers are anchors, not contracts.
#   A later commit can shift them. The gate is "the prose claim is absent/present",
#   verified by grep + spot-read, not "at exact line N".

# NOTE: agreement with verification_findings.md §5 IS the success signal.
#   §5 asserts "Mode B: NONE expected." This task independently re-confirms it. If your
#   scan agrees (0 internal terms + 0 drift signatures + accurate three-state), verdict
#   = VERIFIED. If it DISAGREES (a real contradiction), that's a finding — investigate
#   (was the doc edited post-293f565?) before editing; a Mode-B in-place fix is then correct.

# NOTE: there is no build/test for this task.
#   It's a grep + spot-read verification. The deliverable is the verdict + evidence.
```

## Implementation Blueprint

### Data models and structure

Not applicable — no data models, no code. This is a read-and-verify sweep that
produces a one-line verdict (+ optional in-place doc fix only in the drift-found
branch).

### Implementation Tasks (ordered: scan → spot-read → verdict → optional fix)

```yaml
Task 1: CONFIRM the working tree state (static verification target)
  - RUN: git -C /home/dustin/projects/qmkonnect status --short
  - NOTE: the delta is shipped (HEAD includes d240b27 + 293f565). A dirty tree is OK
          ONLY for plan/ research artifacts; src/ spec/ docs/ README.md should be clean
          at the start (you're verifying committed docs). If a doc is already mid-edit,
          coordinate — don't verify a half-written file.

Task 2: SCAN for internal-mechanism terms (the "silent" property) — expect ALL ZERO
  - RUN (each must print 0):
      for t in 'Tier-?1' 'Tier-?2' 'PresenceTracker|presence' 'classify' 'warm' \
               'is_device_connected' 'broadcast|Broadcast'; do
        printf '%-30s %s\n' "$t" "$(grep -rniE "$t" README.md docs/*.md | grep -v llms_full.txt | wc -l)"
      done
  - EXPECT: 0 for every term. (The docs are silent on the d240b27 internals.)
  - IF a non-zero hit appears: read it — is it a genuine internal-mechanism mention
          that contradicts v6, or a benign reuse (e.g. "warm" in an unrelated sentence)?
          Genuine contradiction ⇒ drift (Task 5); benign ⇒ note + continue.

Task 3: SCAN for the 5 targeted DRIFT SIGNATURES — expect ALL NONE
  - RUN:
      # (1) Tier-1-keyed status claim
      grep -rniE 'tier.?1.?key|keyed on tier|tier.?keyed' README.md docs/*.md | grep -v llms_full.txt
      # (2) two-state / boolean status claim
      grep -rniE 'two.?state|boolean status' README.md docs/*.md | grep -v llms_full.txt
      # (3) broadcast/send/ping to every 0xFF60 interface
      grep -rniE '(broadcast|send|ping|burst).{0,40}(every|all).{0,20}(0xFF60|interface|matching)' README.md docs/*.md | grep -v llms_full.txt
      # (4) unqualified single-ping-per-appearance promise
      grep -rniE 'single ping|one ping|ping.{0,15}(once|per appearance|per change)' README.md docs/*.md | grep -v llms_full.txt
      # (5) status refreshes every poll
      grep -rniE '(every|each) poll|polls every|refresh.{0,15}every' README.md docs/*.md | grep -v llms_full.txt
  - EXPECT: every grep prints nothing (exit 1). Any hit = a contradiction ⇒ Task 5.

Task 4: SPOT-READ the three-state status sections (the "accurate" property)
  - READ: docs/usage.md ~108-122 (### Verify Keyboard Connection).
  - READ: docs/troubleshooting.md ~100-115 (> Read the tray/menu-bar status first —
          it's three-state).
  - READ: docs/installation.md ~215-222 (Linux permission status text).
  - READ: README.md ~205-225 (picker / status markers).
  - CONFIRM each describes: ● Device Connected (qmk_notifier-capable present) /
          ⚠ QMK board found — no qmk_notifier module (flash it) / ○ No Device
          Connected — matching spec/DEVICE_DISCOVERY.md §3 + spec/UI.md §4.
  - ALSO eyeball the 2 handshake mentions (configuration.md:231 --list-callbacks;
          qmk-integration.md:154 "the capability handshake") — confirm neither makes a
          capable-keyed/warm-feed claim (both are accurate/neutral per research).

Task 5: (DRIFT-FOUND BRANCH ONLY) fix the contradiction IN PLACE
  - ONLY if Task 2/3 found a genuine contradiction (one of the 5 signatures, or an
          internal-mechanism mention that mis-states v6): edit the affected README.md /
          docs/*.md file in place (Mode B) to remove/repair the contradiction. Mirror
          the spec/DEVICE_DISCOVERY.md §3 + spec/UI.md §4 wording.
  - DO NOT edit docs/llms_full.txt (derived — regeneration is the fix mechanism).
  - DO NOT edit src/ or spec/ (out of scope).
  - Research baseline: this branch is NOT reached — no contradictions found.

Task 6: STATE THE VERDICT + capture evidence
  - NO-DRIFT (expected): write the one-line verdict "VERIFIED: no documentation drift"
          + a short evidence summary (the Task 2 counts = 0; Task 3 = none; Task 4
          spot-read confirms three-state) into
          plan/006_8f4080956ee0/P1M1T2S1/research/verdict.md.
  - DRIFT-FOUND: list the file/line(s) edited + the before→after in verdict.md.
  - RE-RUN: git status --short src/ spec/  → MUST be clean (no code/spec touched).
          git status --short README.md docs/  → clean (no-drift) OR only the fixed
          overview doc(s) (drift-found).
```

### Implementation Patterns & Key Details

```text
# === THE DRIFT-SIGNATURE MAP (what each v6 semantic forbids in prose) ===
#   v6: status is capable-keyed (not Tier-1)        → forbid: "Tier-1-keyed" / "keyed on Tier-1"
#   v6: three-state, transition-driven              → forbid: "two-state"/"boolean"; "(every|each) poll"
#   v6: warm-feed single-board-only                 → forbid: "single ping per appearance" unqualified
#   v6: broadcast to capable boards                 → forbid: "(broadcast|send)…every 0xFF60 interface"
#   v6: is_device_connected is Tier-1-only          → (no user-doc mention expected; 0 hits = correct)
#   v6: trayless handshake once + BindsTo/Restart   → (no user-doc mention expected; 0 hits = correct)

# === WHY "0 internal-term hits" IS THE PASS, NOT A GAP ===
#   d240b27 is an internal correctness refinement. User docs SHOULD be silent on
#   PresenceTracker/classify_devices/warm-feed — those are implementation details a
#   user never reads about. The user-visible surface (three-state status) is what the
#   docs describe, and it is accurate. "Silent + accurate" = no drift.

# === THE FALSE-POSITIVE ping HITS (don't be fooled) ===
#   Bare `grep -i ping` flags "Typing"/"keeping"/"skipping"/"Stopping". The drift
#   signature (§2b #4) is the SPECIFIC phrase "single ping / one ping / ping…once",
#   which correctly returns none. Use the specific phrase, not the bare substring.

# === llms_full.txt IS DERIVED ===
#   It's a concatenation (docs/generate_llms_full.sh). Grep it to confirm the
#   regeneration is current (it should mirror docs/*.md's clean result). NEVER
#   hand-edit it; if it were stale, regeneration (a separate step) is the fix.

# === THE VERDICT IS THE DELIVERABLE ===
#   "VERIFIED: no documentation drift" (expected) + evidence summary, at
#   plan/.../P1M1T2S1/research/verdict.md. No src/spec edit. No doc edit (no-drift).
```

### Integration Points

```yaml
SOURCE FILES:
  - verify (grep + read): "README.md, docs/*.md (index/usage/installation/configuration/
                           troubleshooting/qmk-integration/examples)"
  - verify-by-grep only (DERIVED, never hand-edit): "docs/llms_full.txt"
  - NEVER modify: "src/ (S1/S3), spec/ (S2 — v6 source of truth)"

DEPENDENCIES / BUILD:
  - none. Pure grep + read verification. No cargo, no build, no tests.

UPSTREAM CONTEXT:
  - verification_findings.md §5: "the 'NONE expected' assertion this task re-confirms."
  - commit d240b27: "code+spec shipped (the v6 semantics the docs must not contradict)."
  - commit 293f565: "docs regenerated (the docs under verification)."

PARALLEL SIBLING (S3 — being implemented):
  - S3 is the read-only CODE-side audit (caveats backed). This task is the read-only
    DOCS-side verification (docs not stale). NO file overlap: S3 reads src/+packaging/;
    this reads README.md + docs/. Both must be green to close the delta.

DOWNSTREAM CONSUMERS:
  - The orchestrator reads verdict.md to flip P1.M1.T2.S1 → Complete. A VERIFIED
    verdict + clean git status closes the whole P1.M1 verification milestone (the
    delta is verified-complete across code/spec/caveats/user-docs).

OUT OF SCOPE:
  - Editing src/ or spec/ (S1/S2/S3 scope; the delta is already shipped there).
  - Hand-editing docs/llms_full.txt (derived; regeneration is the mechanism).
  - Editing spec/*.md (they are the v6 source of truth — already carry the wording).
  - Adding/changing user-visible behavior (this is verification only).
```

## Validation Loop

> The Validation Loop for THIS subtask IS the grep scan (Tasks 2–3) + the spot-read
> (Task 4) + the no-src/spec-edit invariant. The levels below are the
> verdict-production checks.

### Level 1: The grep scan reproduces "silent + no contradictions"

```bash
cd /home/dustin/projects/qmkonnect

# (a) Internal-mechanism terms — ALL must be 0 in user docs (the "silent" property).
for t in 'Tier-?1' 'Tier-?2' 'PresenceTracker|presence' 'classify' 'warm' \
         'is_device_connected' 'broadcast|Broadcast'; do
  printf '%-30s %s\n' "$t" "$(grep -rniE "$t" README.md docs/*.md | grep -v llms_full.txt | wc -l)"
done
# Expected: 0 for every line. (Non-zero => read it: genuine contradiction or benign reuse?)

# (b) The 5 drift signatures — ALL must print nothing (exit 1).
grep -rniE 'tier.?1.?key|keyed on tier|tier.?keyed' README.md docs/*.md | grep -v llms_full.txt
grep -rniE 'two.?state|boolean status' README.md docs/*.md | grep -v llms_full.txt
grep -rniE '(broadcast|send|ping|burst).{0,40}(every|all).{0,20}(0xFF60|interface|matching)' README.md docs/*.md | grep -v llms_full.txt
grep -rniE 'single ping|one ping|ping.{0,15}(once|per appearance|per change)' README.md docs/*.md | grep -v llms_full.txt
grep -rniE '(every|each) poll|polls every|refresh.{0,15}every' README.md docs/*.md | grep -v llms_full.txt
# Expected: no output from any. Any output => a contradiction => Level 4 triage.

# (c) llms_full.txt mirrors docs/*.md (derived-artifact confirmation — expect same clean result).
for t in 'Tier-?1' 'PresenceTracker' 'classify_devices' 'warm.feed|warm_feed'; do
  printf 'llms_full %-22s %s\n' "$t" "$(grep -niE "$t" docs/llms_full.txt | wc -l)"
done
# Expected: 0 (the concatenation mirrors the clean source docs).
```

### Level 2: The three-state status spot-read confirms "accurate"

```bash
cd /home/dustin/projects/qmkonnect

# The user-facing three-state status must read Connected / No module / Disconnected.
grep -nE 'three-state|Device Connected|no qmk_notifier module|No Device Connected' \
  docs/usage.md docs/troubleshooting.md docs/installation.md README.md
# Expected: each file shows the three states with the capable/no-module semantics.
#   usage.md ~112: "it's three-state:" + the 3 bullets.
#   troubleshooting.md ~105: "it's three-state" + the 3 states.
#   installation.md ~218: "● Device Connected — a qmk_notifier-capable board is present".
#   README.md ~217-220: the capable/no-module picker markers.

# Cross-check against the spec table (the v6 source of truth):
grep -nE 'Connected|No module|Disconnected' spec/DEVICE_DISCOVERY.md | head -8
# The user docs' state NAMES + semantics must agree with this table.
```

### Level 3: No-src/spec-edit invariant (docs-only task)

```bash
cd /home/dustin/projects/qmkonnect
git status --short src/ spec/
# Expected: empty (clean). This task never touches code or spec. If anything appears,
# revert it — verification must not modify the implementation.

# No-drift branch: docs also untouched.
git status --short README.md docs/
# Expected: empty (pure verification — nothing to commit).
# Drift-found branch: ONLY the fixed overview doc(s) appear here (Mode B in-place fix).
```

### Level 4: Drift-found triage (ONLY if Level 1 found a contradiction)

```text
If a drift signature hit appeared in Level 1(b) or an internal term in 1(a) was a
genuine contradiction, classify before editing:
1. Re-read the surrounding paragraph — is the phrase used in a sense that does NOT
   contradict v6? (e.g. "broadcast" in an unrelated networking sentence; "warm" in
   "warm boot"). Benign reuse => not drift; note + leave it.
2. If it IS a genuine contradiction (a real Tier-1-keyed / two-state / every-0xFF60 /
   unqualified-single-ping / per-poll claim): edit the affected README.md / docs/*.md
   IN PLACE (Mode B) to match spec/DEVICE_DISCOVERY.md §3 + spec/UI.md §4 wording.
   NEVER edit docs/llms_full.txt (derived), src/, or spec/.
3. Re-run Level 1 to confirm the fix removed the contradiction and introduced none.
4. Record the before→after in verdict.md.
(Research baseline: this level is NOT reached — no contradictions found.)
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1(a): internal-mechanism terms = 0 hits in README.md + docs/*.md (excl. llms_full.txt).
- [ ] Level 1(b): all 5 drift signatures return no output.
- [ ] Level 1(c): llms_full.txt mirrors the clean docs/*.md (derived confirmation).
- [ ] Level 2: the three-state status sections spot-read as Connected/No-module/Disconnected
      and agree with spec/DEVICE_DISCOVERY.md §3.
- [ ] Level 3: `git status --short src/ spec/` clean (no code/spec touched).

### Feature Validation

- [ ] Verdict stated explicitly: **`VERIFIED: no documentation drift`** (expected) OR
      the specific file/line(s) edited (drift-found).
- [ ] (No-drift) `git status --short README.md docs/` clean — nothing modified.
- [ ] (Drift-found) only the contradicted overview doc(s) edited in place; re-scan clean.

### Code Quality Validation

- [ ] No `src/` or `spec/` file modified (docs-only task).
- [ ] No `docs/llms_full.txt` hand-edit (derived artifact).
- [ ] The verdict + evidence captured under `plan/.../P1M1T2S1/research/verdict.md`.

### Documentation & Deployment

- [ ] DOCS = Mode B. No-drift ⇒ no doc changes (the expected result). Drift-found ⇒
      in-place fix to the affected overview doc(s) only.
- [ ] The user docs remain silent on internal mechanisms and accurate on the
      three-state status (the v6 contract).

---

## Anti-Patterns to Avoid

- ❌ Don't treat "0 internal-term hits" as a gap to fill — the d240b27 lifecycle is an
  INTERNAL correctness mechanism; user docs are EXPECTED to be silent on
  PresenceTracker/classify_devices/warm-feed. "Silent + accurate" is the pass, not
  "mentions every internal."
- ❌ Don't be fooled by the bare `grep -i ping` substring hits — "ping" appears inside
  *Typing*/*keeping*/*skipping*/*Stopping*. Use the specific drift-signature phrase
  ("single ping / one ping / ping…once"), which correctly returns none.
- ❌ Don't use `\|` for alternation in `grep -E` — it's a literal pipe in ERE; use `|`.
  A multi-alternation pattern with `\|` silently matches nothing (the research summary
  loop hit this). The detailed greps used correct `|` and are the ground truth.
- ❌ Don't hand-edit `docs/llms_full.txt` — it's a concatenation
  (`generate_llms_full.sh`); if it drifts the fix is regeneration, not a patch. Grep it
  only to confirm the regeneration is current.
- ❌ Don't edit `src/` or `spec/` — the code/spec verification is S1/S2/S3 (the delta is
  already shipped there). Even in the drift-found branch, the fix is to a user doc only.
  `git status` for src/ + spec/ must stay clean.
- ❌ Don't "fix" the three-state status — `● Connected / ⚠ No module / ○ Disconnected` is
  EXACTLY the v6 spec table; it is accurate, not drift. Only one of the 5 specific
  drift signatures is a fix-worthy contradiction.
- ❌ Don't conflate this with the sibling audits: S1 = gates/tests; S2 = spec drift; S3 =
  caveats code-backed; **this = user docs not stale**. This task is grep + spot-read,
  not gate-running, spec-grepping, or code-reading.
- ❌ Don't declare drift without triage — a hit on a signature is usually benign reuse
  ("broadcast" in networking prose, "warm" in "warm boot"). Read the surrounding
  paragraph; only a genuine contradiction gets a Mode-B in-place fix.
- ❌ Don't assume line numbers are stable — they're anchors. A later commit can shift
  them. The gate is "the prose claim is present/absent", verified by grep + spot-read.
- ❌ Don't commit the verdict artifact into the repo proper — it lives under
  `plan/.../research/verdict.md` (plan research area).

---

**Confidence Score: 10/10** for one-pass execution success. This is a docs-side
verification whose scan was run directly during research (internal-mechanism terms =
0 hits; all 5 drift signatures = none; three-state status accurate; the 6 "ping" hits
confirmed false-positive substrings), reproducing `verification_findings.md` §5's
"Mode B: NONE expected" assertion. The deliverable is a one-line verdict
("VERIFIED: no documentation drift") + evidence; no build, no tests, no code/spec
edits. The two residual risks — (a) mistaking the bare-"ping" substring hits for real
device-probe mentions, and (b) using `\|` instead of `|` in ERE alternation — are both
pre-empted in the Gotchas with the exact correct grep commands. The drift-found branch
is documented for completeness but research confirms it is not reached: the docs are
silent on the d240b27 internals and accurate on the user-visible three-state status.
# PRP — P2.M1.T1.S2: Confirm the write-narrowing DEFER decision (review the architecture decision record)

> **Repo under change:** the **qmkonnect** desktop app at `/home/dustin/projects/qmkonnect`.
> **This is a DECISION-CONFIRMATION task, NOT a code task.** The single artifact
> is a *Confirmation* section appended to the pre-existing architecture decision
> record `plan/005_8b95ea464bd9/architecture/write_narrowing_decision.md`.
> **ZERO source-code changes. Mode A — no `docs/*.md`.** It is the second subtask
> of P2.M1 "Shared-open invariant: comments + tests" (the F14 / R-COEX milestone).
> The sibling **P2.M1.T1.S1** edits `src/core/notifier.rs` ONLY (R-COEX comments
> + `r_coex_invariants` tests); this task (S2) edits the architecture record ONLY.
> **Zero file overlap.**

---

## ⚠️ READ FIRST — this task produces NO code

The work-item contract says: *"Review the decision record … Confirm the DEFER
decision is correct … If DEFER (expected), no code change and P2 closes with S1
only."* **Research confirms DEFER is correct** (every claim in the record was
re-verified against the Cargo.lock-pinned crate rev `f26893e`, and the search for
a clean app-only write-narrowing mechanism came up empty — see
`research/notes.md` §2–§3). Therefore the deliverable is a **single Markdown
append** to the decision record documenting the confirmation. Do NOT:

- touch any `.rs` file,
- add a `classify_devices()` write-narrowing path (that is P3.M1, and it is for
  the **picker/status**, not the write path — see the §8 "prefer that" trap below),
- edit `PRD.md` / `tasks.json` / `prd_snapshot.md` / `.gitignore` / any `docs/*.md`.

## Goal

**Feature Goal**: Formally **confirm** the DEFER decision in
`architecture/write_narrowing_decision.md` by (1) re-verifying its three factual
claims against the pinned crate source (rev `f26893e`), (2) cross-checking the
target design in `spec/DEVICE_DISCOVERY.md` §4/§8 + `spec/PROTOCOL.md` §3.5, and
(3) exhaustively re-considering whether a *clean* app-only write-narrowing
mechanism exists that the architecture-research phase missed. The expected and
research-supported outcome is **DEFER confirmed** — narrowing needs a coordinated
`qmk-notifier` crate API addition, it is out of scope for this one-repo delta,
and it is harmless today (VIA firmware ignores `0x81 0x9F`-prefixed input).

**Deliverable**: ONE Markdown edit — append a `## Confirmation (Session 005, P2.M1.T1.S2)`
section to `plan/005_8b95ea464bd9/architecture/write_narrowing_decision.md`
recording: the re-verification (crate rev + exact line numbers + grep evidence),
the exhaustive app-only-option analysis (4 options, all rejected), the resolution
of the §8 "prefer that" misread-trap, and the verdict (DEFER confirmed; no code
change; P2 closes with S1+S2; no follow-on subtask).

**Success Definition**:
- The decision record's Status remains **DEFER** and now carries a dated,
  evidence-backed Confirmation section.
- Every claim in the record is re-verified against crate rev `f26893e` with the
  exact grep commands in `research/notes.md` §2 (all return the expected hits).
- The §8 "prefer that" note is explicitly reconciled (classification ≠ write path).
- `git status` shows **exactly one** modified file:
  `plan/005_8b95ea464bd9/architecture/write_narrowing_decision.md`.
- **No `.rs` file is touched** — `cargo build` / `cargo test` are unchanged
  (optionally re-run as a no-op proof of "no behavior change").

## User Persona (if applicable)

**Target User**: Future dev agents (and humans) revisiting the multi-board
question. The Confirmation section is the load-bearing artifact: it closes the
question with pinned evidence so nobody re-opens it without new information
(i.e. a crate API addition).

**Use Case**: A future dev sees DEVICE_DISCOVERY.md §4.2 ("write filter becomes
`configured_filter() && kind==Capable`") and §8's "prefer that" note and wonders
"can I land this app-side now?" The Confirmation section points them at the
verified crate facts (no per-path send, no seize, filter-keyed private MatchKey)
and the rejected option analysis, so they reach "no — needs a crate change" in
one read instead of re-doing the research.

**Pain Points Addressed**: The current decision record is correct but
**unverified-in-this-session** and does not yet reconcile the §8 "prefer that"
trap (which a careless reader could misread as "do it app-side"). This task
closes both gaps and stamps the verdict with the exact crate rev.

## Why

- **Closes the multi-board question for this delta.** P2 (R-COEX) lands the
  in-repo invariant (S1) regardless of write-narrowing; P3's `classify_devices()`
  feeds the picker/status only. Whether the *write* path can also be narrowed
  app-side is the open question this task answers (with a pinned-rev "no").
- **Prevents a wasted implementation pass.** Without a confirmation on record, a
  future agent may attempt option (c)/(d) (classify-and-still-broadcast, or
  re-implement the transport in-app) and burn a cycle before discovering the
  crate boundary. The Confirmation section makes the crate boundary grep-verifiable.
- **Pins the evidence to a rev.** The crate is git-tagged `v0.3.0` (Cargo.lock →
  rev `f26893e`). The claims (private `MatchKey` @641, broadcast
  `open_matching_devices` @723, filter-only `send_raw_report` @172, no seize) are
  true *at this rev*; recording the rev makes a future drift detectable.

## What

Append a section to `plan/005_8b95ea464bd9/architecture/write_narrowing_decision.md`
(sibling to its existing `## Findings` / `## Decision` / `## Follow-up` / `##
Impact on This Delta` blocks). The new section records the confirmation. Its
content is specified verbatim in the Implementation Blueprint (Task 2) below.

### The verification the Confirmation must cite (already done in `research/notes.md` §2)

| Claim | Crate rev `f26893e` location | Evidence |
|---|---|---|
| `MatchKey` is **private** + filter-keyed `{vid?,pid?,usage_page,usage}` | `src/core.rs:641` | `struct MatchKey { vendor_id: Option<u16>, product_id: Option<u16>, usage_page: u16, usage: u16 }` — no `pub`; no path field |
| `open_matching_devices` opens **ALL** matches, no per-path scope | `src/core.rs:723` | `device_list().filter(device_matches…).filter_map(\|info\| info.open_device(api).ok())` |
| The only public send is filter-based | `src/core.rs:172` + `src/lib.rs:404` | `pub fn send_raw_report(data, vid, pid, page, usage, verbose)` builds `MatchKey{…}` internally; `pub fn run(params)` |
| Payload builder is `pub(crate)` (not app-callable) | `src/core.rs:495` | `pub(crate) fn build_command_data` |
| Public surface is filter-only; **no seize** anywhere | `src/lib.rs:1-7` + whole crate | re-export list has no path/device send; `grep -rni 'seize\|exclusive'` ⇒ only CLI "mutually exclusive" arg-group comments |

### Success Criteria
- [ ] A `## Confirmation (Session 005, P2.M1.T1.S2)` section is appended to `write_narrowing_decision.md`.
- [ ] The section cites crate rev `f26893e` (v0.3.0) and the 5 evidence rows above.
- [ ] The section records the exhaustive app-only-option analysis: (a) VID/PID narrowing — non-general; (b) crate API addition — out of scope; (c) app capability overlay — still broadcasts; (d) app re-implements transport — not-clean DRY violation. **All rejected ⇒ DEFER.**
- [ ] The section explicitly reconciles DEVICE_DISCOVERY.md §8's "prefer that" note: it refers to **classification** (picker/status, P1/P3), **NOT** the write path.
- [ ] The decision record's Status remains `DEFER`; no follow-on subtask is added (the existing "Follow-up" note already tracks the crate-side remedy).
- [ ] `git status` shows exactly ONE modified file: the decision record.
- [ ] No `.rs` file changed; `cargo build` / `cargo test --bin qmkonnect -- --test-threads=1` unchanged (no-op).

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can complete this using only this PRP +
the repo, because (a) the decision under review is reproduced in
`research/notes.md` §1, (b) every claim is re-verified against the pinned crate
rev with exact line numbers + grep commands (§2), (c) the exhaustive
app-only-option analysis is given (§3) including the resolution of the §8
"prefer that" trap, (d) the verbatim text to append is specified in the
Implementation Blueprint (Task 2), and (e) the validation is a single
`git status` check. See `research/notes.md`.

### Documentation & References

```yaml
# MUST READ — the decision record under review (the file this task EDITS)
- file: plan/005_8b95ea464bd9/architecture/write_narrowing_decision.md
  why: "the existing DEFER record. APPEND a Confirmation section; do NOT rewrite
        the Findings/Decision. Its 3 claims + 3 options are the thing to verify."
  pattern: "Markdown sections: ## Date / ## Status / ## Question / ## Context /
            ## Findings (### 1/2/3) / ## Decision / ## Follow-up / ## Impact on
            This Delta. Add ## Confirmation as the new final section."

# MUST READ — the pinned crate source (verify the claims; read-only)
- file: ~/.cargo/git/checkouts/qmk-notifier-1f15950b695d9922/f26893e/src/core.rs
  why: "core.rs:641 (private filter-keyed MatchKey), core.rs:723
        (open_matching_devices opens ALL matches), core.rs:172 (send_raw_report,
        the only public send, builds MatchKey internally), core.rs:495
        (pub(crate) build_command_data). These ARE the 4 evidence rows."
  gotcha: "this path is the Cargo.lock-pinned rev f26893e (v0.3.0). If the
           checkout path differs, find it via:
           `grep -A3 'name = \"qmk-notifier\"' Cargo.lock` (gives the rev) then
           `ls ~/.cargo/git/checkouts/qmk-notifier-*/<rev>/src/core.rs`.
           Do NOT read a different rev — the claims are pinned to f26893e."

# MUST READ — the crate's public surface (proves no path/device send, no seize)
- file: ~/.cargo/git/checkouts/qmk-notifier-1f15950b695d9922/f26893e/src/lib.rs
  why: "lib.rs:1-7 (the `pub use core::{…}` re-export list — no path/device send)
        and lib.rs:404 (`pub fn run(params) -> Result<CommandResponse, QmkError>`
        — the app's only device egress). lib.rs:231/247/348/920 are the ONLY
        'exclusive' hits (CLI arg-group comments, not HID seize)."

# MUST READ — the target design (what DEFER is deferring TOWARD)
- url: spec/DEVICE_DISCOVERY.md
  why: "§4 'Multi-Board Policy' (the v1 broadcast decision + §4.2 the target
        'write filter = configured_filter() && kind==Capable, MatchKey enriched'
        + §4.3 the documented limitation). §8 'Implementation Map' row
        'Multi-board broadcast' lists the file as 'src/core/notifier.rs (+ crate
        MatchKey)' — i.e. it needs a crate touch. §8's closing note
        ('classification can live entirely in qmkonnect ... prefer that') is the
        trap: it is about CLASSIFICATION (picker/status), NOT write-narrowing."
  critical: "§8 'prefer that' MUST be reconciled in the Confirmation: it does
        NOT contradict DEFER. Classification-in-app (P1/P3) ≠ write-narrowing."

# MUST READ — the capability tier / broadcast rationale
- url: spec/PROTOCOL.md
  why: "§3.5 'Capability tier & multi-board broadcast' — states the write match
        set is 'Tier-1 AND kind==Capable' and that the crate's existing
        burst-to-every-matching-device behavior broadcasts to all capable boards.
        Confirms the TARGET requires the capability axis in the match set (a
        crate concern), and that today's broadcast is the accepted v1 behavior."
  section: "### 3.5 Capability tier & multi-board broadcast"

# MUST READ — the harmlessness leg (why DEFER is SAFE, not just necessary)
- url: spec/DEVICE_DISCOVERY.md
  why: "§6.4 'Protocol demultiplexing' — VIA firmware ignores 0x81 0x9F-prefixed
        input; QMKonnect ignores VIA-shaped bytes. So a magic burst to a
        pure-VIA board is silently dropped. Narrowing is a politeness/traffic
        optimization, NOT a correctness requirement. This is why DEFER is safe."
  section: "### 6.4 Protocol demultiplexing (why overlapping traffic is harmless)"

# Reference — the parallel sibling PRP (confirms zero file overlap)
- file: plan/005_8b95ea464bd9/P2M1T1S1/PRP.md
  why: "S1 edits src/core/notifier.rs ONLY (R-COEX comments + r_coex_invariants
        tests). It explicitly says 'do NOT do S2's work here.' This task (S2)
        edits the architecture decision record ONLY. Zero overlap with S1."
  critical: "do NOT touch src/core/notifier.rs here (S1's file, in parallel)."

# Reference — the cross-repo contract summary
- file: plan/005_8b95ea464bd9/architecture/external_deps.md
  why: "the crate boundary summary (public API surface, device cache, burst model).
        Corroborates that the app's only egress is run()/send_raw_report()."
```

### Current Codebase tree (relevant subset — NOTHING here is edited by this task)

```bash
plan/005_8b95ea464bd9/architecture/
  write_narrowing_decision.md   # <-- THE ONLY FILE EDITED (append a Confirmation section)
  external_deps.md              # read-only reference
  notifier_mechanisms.md        # read-only reference
  ... (other arch docs)         # read-only
src/core/notifier.rs            # NOT touched (S1's file, in parallel)
spec/DEVICE_DISCOVERY.md        # read-only reference (§4, §6.4, §8)
spec/PROTOCOL.md                # read-only reference (§3.5)
```

### Desired Codebase tree (files this task changes)

```bash
plan/005_8b95ea464bd9/architecture/
  write_narrowing_decision.md   # MODIFIED ONLY — + ## Confirmation (...) section appended
# NO .rs files. NO docs/*.md. NO Cargo.toml.
```

### Known Gotchas of our codebase & Library Quirks

```text
CRITICAL: this task writes NO code. The deliverable is a Markdown append to ONE
architecture decision record. If git status shows any .rs / docs/*.md / Cargo.toml
change, you have over-reached — revert it.

CRITICAL: the §8 "prefer that" note is a TRAP. DEVICE_DISCOVERY.md §8 ends with
"classification can live entirely in qmkonnect and the crate need not change;
prefer that." This is about CLASSIFICATION (the picker + three-state status,
planned in P1/P3 via classify_devices()), NOT about write-narrowing. The write
path still bursts to every filter-matching device via run()/send_raw_report().
The Confirmation MUST state this distinction explicitly, or a future reader will
revive the question.

GOTCHA: pin the verification to crate rev f26893e (v0.3.0). Confirm via
`grep -A3 'name = "qmk-notifier"' Cargo.lock` (the `#<rev>` suffix). The crate
checkout dir is ~/.cargo/git/checkouts/qmk-notifier-1f15950b695d9922/f26893e/ .
If a future bump changes the rev, the line numbers may shift — re-verify against
the THEN-current rev and update the Confirmation's rev citation.

GOTCHA: "no seize API" is proven by ABSENCE. `grep -rni 'seize\|exclusive'` over
the crate src/ returns ONLY CLI arg-group comments ("mutually exclusive" selectors
at lib.rs:231/247/348/920). There is no hidapi seize/exclusive open in the crate
or in hidapi 2.x. Record this as "absence-of-evidence = evidence-of-absence (the
crate exposes no seize surface for the app to call even if it wanted to)."

GOTCHA: the decision record's option (c) ("app-side capability overlay") is what
P3.M1.T1's classify_devices() implements — for the PICKER/STATUS. Do not confuse
"classify app-side" (fine, planned) with "narrow writes app-side" (impossible
without a crate change). The Confirmation must keep these distinct.

GOTCHA: do NOT add a follow-on subtask. The work-item contract says "If DEFER
(expected), no code change and P2 closes with S1 only." The decision record's
existing "## Follow-up" note already tracks the crate-side remedy ("When the
crate adds per-path send or a capability match axis, revisit §4.2"). Adding a
duplicate subtask would be noise; the DEFER's remedy is a crate change owned by
the crate repo, not this plan.
```

## Implementation Blueprint

### Data models and structure
None. This task produces prose (a Markdown confirmation), not code or types.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: RE-VERIFY the decision record's claims against the pinned crate (read-only)
  - LOCATE the pinned crate source:
      grep -A3 'name = "qmk-notifier"' Cargo.lock   # expect ...#f26893e...
      # the checkout: ~/.cargo/git/checkouts/qmk-notifier-1f15950b695d9922/f26893e/
  - RUN these 5 greps and CONFIRM each returns the expected hit (evidence in
    research/notes.md §2). These ARE the verification — record their output:
      CRATE=~/.cargo/git/checkouts/qmk-notifier-1f15950b695d9922/f26893e
      # 1. private filter-keyed MatchKey (no `pub`, no path field):
      sed -n '639,647p' "$CRATE/src/core.rs"
      #    expect: `struct MatchKey { vendor_id: Option<u16>, product_id: Option<u16>, usage_page: u16, usage: u16 }`
      # 2. open_matching_devices opens ALL matches (filter_map open_device):
      sed -n '720,760p' "$CRATE/src/core.rs"
      #    expect: device_list().filter(device_matches…).filter_map(|info| info.open_device(api).ok())
      # 3. the only public send is filter-based (builds MatchKey internally):
      sed -n '170,185p' "$CRATE/src/core.rs"
      #    expect: `pub fn send_raw_report(data, vendor_id, product_id, usage_page, usage, verbose)`
      #            body: `let key = MatchKey { vendor_id, product_id, usage_page, usage };`
      # 4. payload builder is pub(crate) (NOT app-callable):
      sed -n '495p' "$CRATE/src/core.rs"
      #    expect: `pub(crate) fn build_command_data(command: &crate::RunCommand) -> Vec<u8> {`
      # 5. no seize/exclusive API anywhere:
      grep -rni 'seize\|exclusive\|O_EXCL' "$CRATE/src/"
      #    expect: ONLY lib.rs CLI "mutually exclusive" arg-group comments (4 hits)
      # 6. public surface (lib.rs re-export — no path/device send):
      sed -n '1,7p' "$CRATE/src/lib.rs"
  - IF any grep diverges from the expected output: the crate has drifted from
    rev f26893e. Re-pin the rev citation in the Confirmation to the ACTUAL
    checked-out rev and re-run. Do NOT fabricate evidence.
  - OUTCOME: all 5 claims hold. (research/notes.md §2 confirmed this.)

Task 2: APPEND the ## Confirmation section to write_narrowing_decision.md
  - EDIT: plan/005_8b95ea464bd9/architecture/write_narrowing_decision.md
  - APPEND (as the new final section, after "## Impact on This Delta"):
      ## Confirmation (Session 005, P2.M1.T1.S2)

      Re-verified against the pinned crate `qmk-notifier` v0.3.0, rev `f26893e`
      (Cargo.toml git-tag pin; Cargo.lock `#f26893e…`). All claims hold:

      | Claim | Evidence (rev f26893e) |
      |---|---|
      | `MatchKey` private + filter-keyed `{vid?,pid?,usage_page,usage}` | `src/core.rs:641` — `struct MatchKey {…}` (no `pub`; no path field) |
      | `open_matching_devices` opens ALL matches | `src/core.rs:723` — `device_list().filter(device_matches…).filter_map(\|info\| info.open_device(api).ok())` |
      | Only public send is filter-based | `src/core.rs:172` (`pub fn send_raw_report(data,vid,pid,page,usage,verbose)` builds `MatchKey` internally) + `src/lib.rs:404` (`pub fn run`) |
      | Payload builder not app-callable | `src/core.rs:495` — `pub(crate) fn build_command_data` |
      | No seize/exclusive API | `grep -rni 'seize\|exclusive'` ⇒ only CLI "mutually exclusive" arg-group comments (`lib.rs:231/247/348/920`); hidapi 2.x has no seize |

      Exhaustive app-only-mechanism re-check (4 options):
      (a) VID/PID filter narrowing — non-general (many QMK boards share VID/PID);
      (b) crate API addition (MatchKey capability field / path-scoped send) —
          coordinated two-repo change, out of scope for this delta;
      (c) app-side capability overlay (`classify_devices`, P3.M1) — classifies for
          the picker/status but STILL broadcasts via run()/send_raw_report();
      (d) app re-implements the transport (opens hidapi + writes raw) — NOT clean:
          duplicates burst_to_one framing/cache/read-drain, DRY violation, splits
          the R-COEX surface. Rejected.
      No clean app-only write-narrowing mechanism exists.

      §8 "prefer that" reconciled: DEVICE_DISCOVERY.md §8's closing note
      ("classification can live entirely in qmkonnect … prefer that") refers to
      CLASSIFICATION (the picker + three-state status, P1/P3), NOT the write path.
      The write path still bursts to every filter-matching device. It does NOT
      contradict DEFER.

      Verdict: **DEFER confirmed.** Narrowing needs a coordinated crate change
      (tracked by the Follow-up note above); it is harmless today (VIA firmware
      ignores `0x81 0x9F`-prefixed input — DEVICE_DISCOVERY.md §6.4). No code
      change in this delta; no follow-on subtask added; P2 closes with S1 (code)
      + S2 (this confirmation).
  - PRESERVE: every existing section of the record (Date/Status/Question/Context/
    Findings/Decision/Follow-up/Impact). This is an APPEND, not a rewrite.
  - DO NOT change the Status line (it stays `DEFER`).

Task 3: VERIFY scope (no code touched)
  - RUN: git status --short
      # EXPECT exactly one line:
      #   M  plan/005_8b95ea464bd9/architecture/write_narrowing_decision.md
      # (or " M" / untracked-staged depending on state — but ONLY that one path)
  - RUN: git diff --stat
      # EXPECT: only write_narrowing_decision.md changed; ZERO .rs / docs/*.md / Cargo.* changes.
  - OPTIONAL no-op proof (NOT a real validation step — no code changed):
      cargo build          # EXPECT: unchanged from before this task (no recompile needed)
      cargo test --bin qmkonnect -- --test-threads=1   # EXPECT: same green as before
    If these recompile anything, a .rs file was accidentally touched — revert it.
  - IF git status shows ANY other file: you over-reached. Revert with
    `git checkout -- <file>` (never revert the decision record itself).
```

### Implementation Patterns & Key Details

```text
The decision record is Markdown prose. The Confirmation is a single appended
section. Keep the tone factual and evidence-first (claim → pinned-rev location →
one-line evidence), mirroring the record's existing "### Findings" subsections.

The single most important sentence in the Confirmation is the §8 reconciliation:
"§8 'prefer that' refers to CLASSIFICATION, not the write path." Without it, the
record leaves a trap for the next reader. Make it a distinct paragraph.

Do NOT add a "Next steps" / "Action items" block that proposes an in-app
implementation — the whole point of DEFER is that there IS no in-app path. The
only forward action is the crate change, already named in the record's existing
"## Follow-up". Re-stating it as "tracked by Follow-up above" is sufficient.
```

### Integration Points

```yaml
CODE: NONE. Zero .rs files. Zero Cargo changes. (This is what makes the task low-risk.)
DOCS: NONE user-facing (Mode A). The Confirmation lives in plan/005.../architecture/,
      which is the research/decision record area — not docs/*.md.
MILESTONE: on DEFER-confirmation, P2 "Shared-open invariant: comments + tests"
           closes with S1 (src/core/notifier.rs R-COEX comments + tests) + S2
           (this confirmation). No additional subtask.
PARALLEL-TASK BOUNDARY: P2.M1.T1.S1 edits src/core/notifier.rs ONLY. This task
           edits plan/005.../architecture/write_narrowing_decision.md ONLY. Zero overlap.
DOWNSTREAM: none. A future crate release that adds per-path send / a capability
            MatchKey field would reopen this; the record's Follow-up note covers it.
CONFIG: none. ROUTES: none. DATABASE: none.
```

## Validation Loop

### Level 1: Verification re-runs (the actual "test" — all must match)

```bash
cd /home/dustin/projects/qmkonnect
# Confirm the pinned rev first:
grep -A3 'name = "qmk-notifier"' Cargo.lock   # expect: source "...#f26893e..."

CRATE=~/.cargo/git/checkouts/qmk-notifier-1f15950b695d9922/f26893e
# 1. private filter-keyed MatchKey (no `pub`):
sed -n '641p' "$CRATE/src/core.rs"   # expect the struct definition
# 2. open_matching_devices opens all matches:
grep -n 'filter_map.*open_device' "$CRATE/src/core.rs"   # expect a hit near :723
# 3. only public send is filter-based:
grep -n 'pub fn send_raw_report\|pub fn run' "$CRATE/src/core.rs" "$CRATE/src/lib.rs"
# 4. build_command_data is pub(crate):
grep -n 'pub(crate) fn build_command_data' "$CRATE/src/core.rs"   # expect :495
# 5. no seize API (only CLI arg-group comments):
grep -rni 'seize' "$CRATE/src/"   # expect ZERO hits
grep -rni 'exclusive' "$CRATE/src/"   # expect ONLY the 4 lib.rs CLI comments
# Expected: every grep returns the cited hit; none contradict the record.
# IF any diverges, the crate drifted from f26893e — re-pin the rev citation.
```

### Level 2: Artifact check (the deliverable exists and is well-formed)

```bash
cd /home/dustin/projects/qmkonnect
# The Confirmation section exists:
grep -n '## Confirmation (Session 005, P2.M1.T1.S2)' \
    plan/005_8b95ea464bd9/architecture/write_narrowing_decision.md   # expect 1 hit
# It cites the rev + reconciles §8:
grep -n 'f26893e\|prefer that' \
    plan/005_8b95ea464bd9/architecture/write_narrowing_decision.md   # expect both
# The Status is still DEFER:
grep -n '^## Status' -A1 \
    plan/005_8b95ea464bd9/architecture/write_narrowing_decision.md   # expect: DEFER
```

### Level 3: Scope check (no code touched)

```bash
cd /home/dustin/projects/qmkonnect
git status --short
# Expected: EXACTLY one entry — the decision record.
git diff --stat
# Expected: only plan/005_8b95ea464bd9/architecture/write_narrowing_decision.md;
# ZERO changes under src/, docs/, spec/, Cargo.toml, Cargo.lock.
# (Optional no-op proof — should NOT recompile anything since no code changed:)
cargo build 2>&1 | tail -1   # expect "Finished" with no recompile of qmkonnect
```

### Level 4: N/A
This task has no runtime/creative validation — it produces prose, not behavior.

## Final Validation Checklist

### Technical Validation
- [ ] The 5 crate-claim greps (Level 1) all return the cited evidence at rev `f26893e`.
- [ ] A `## Confirmation (Session 005, P2.M1.T1.S2)` section is appended to the decision record.
- [ ] The section cites rev `f26893e`, the 5 evidence rows, the 4 rejected options, and the §8 reconciliation.
- [ ] The decision record's Status remains `DEFER`; existing sections are untouched (append-only).
- [ ] `git status` shows exactly ONE modified file: `plan/005_8b95ea464bd9/architecture/write_narrowing_decision.md`.

### Feature Validation (the decision)
- [ ] DEFER confirmed: write-narrowing needs a coordinated crate API addition; out of scope for this one-repo delta.
- [ ] Harmless-today leg recorded: VIA firmware ignores `0x81 0x9F`-prefixed input (DEVICE_DISCOVERY.md §6.4).
- [ ] §8 "prefer that" trap reconciled: classification (P1/P3) ≠ write path.
- [ ] No follow-on subtask added (existing Follow-up note tracks the crate remedy); P2 closes with S1+S2.

### Code Quality Validation
- [ ] ZERO `.rs` files touched.
- [ ] ZERO `docs/*.md` / `spec/*.md` / `Cargo.*` / `.gitignore` changes (Mode A).
- [ ] No overlap with P2.M1.T1.S1 (`src/core/notifier.rs`).
- [ ] The Confirmation prose is evidence-first (claim → pinned-rev location → evidence).

### Documentation & Deployment
- [ ] Mode A: the Confirmation is the only doc artifact; it lives in the architecture/ record area.
- [ ] No user-facing docs changed (that is P4's job).

---

## Anti-Patterns to Avoid

- ❌ Do NOT write any code. This task's deliverable is a Markdown append to ONE
      architecture decision record. Any `.rs` edit is scope creep — revert it.
- ❌ Do NOT rewrite the decision record's existing Findings/Decision/Follow-up
      sections. APPEND a `## Confirmation` section only; preserve everything above it.
- ❌ Do NOT change the Status from `DEFER`. The whole task is to CONFIRM DEFER.
- ❌ Do NOT misread DEVICE_DISCOVERY.md §8's "prefer that" as "do write-narrowing
      app-side." It refers to CLASSIFICATION (picker/status, P1/P3), NOT the write
      path. State this distinction in the Confirmation or leave a trap for the
      next reader.
- ❌ Do NOT add a `classify_devices()` write-narrowing path or an app-side
      hidapi write loop. Option (c)/(d) is rejected (research/notes.md §3); the
      write path must keep going through `qmk_notifier::run()`.
- ❌ Do NOT add a follow-on subtask. The contract says "P2 closes with S1 only"
      on DEFER; the record's existing Follow-up note already tracks the crate remedy.
- ❌ Do NOT cite crate line numbers without re-running the greps at the pinned
      rev `f26893e`. If the checkout rev differs, re-pin the citation to the
      actual rev — never fabricate evidence.
- ❌ Do NOT touch `src/core/notifier.rs` (P2.M1.T1.S1's file, in parallel) or any
      `docs/*.md` (P4's job) or `PRD.md` / `tasks.json` / `prd_snapshot.md`.
- ❌ Do NOT treat `cargo build`/`cargo test` as a real validation step — no code
      changed, so they are a no-op scope proof, not a behavior check.
- ❌ Do NOT claim a "clean app-only mechanism" was found. Research
      (`research/notes.md` §3) exhaustively shows none exists; the verdict is DEFER.

---

## Confidence Score: 9/10

This is a low-risk, evidence-bounded decision-confirmation task. The decisive
work — re-verifying the decision record's three claims against the Cargo.lock-
pinned crate rev `f26893e` — is already done in `research/notes.md` §2 (all five
greps return the cited evidence), and the exhaustive app-only-mechanism search
(§3, four options, all rejected) plus the §8 "prefer that" reconciliation (§3
note) close the only plausible misread. The deliverable is a single Markdown
append whose verbatim text is specified in the Implementation Blueprint (Task 2);
there is no code to get wrong and no behavior to validate. The 1-point
reservation is for the (unlikely) event the implementing agent (a) edits a `.rs`
file by mistake (caught immediately by the `git status` scope check) or (b)
forgets the §8 reconciliation paragraph (caught by the Level 2 artifact grep).
The verdict — DEFER confirmed, no code, P2 closes with S1+S2 — is the expected
and research-supported outcome.
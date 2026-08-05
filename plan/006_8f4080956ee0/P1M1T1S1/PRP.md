# PRP — P1.M1.T1.S1: Run project quality gates and confirm the PresenceTracker/warm-feed/classify-mixed tests pass

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`).
> **This is a VERIFICATION subtask — no code changes.** Run the project's quality
> gates and the single-threaded test suite against the already-shipped code
> (`d240b27` *"Key handshake lifecycle on capable-board presence, not Tier-1"*;
> HEAD is one plan-only commit ahead, code unchanged), and produce a pass/fail
> report that explicitly confirms the 5 named tests pass.
> **Verified baseline (research run, this session): ALL 5 GATES GREEN, 389 tests
> pass, all 5 named tests `ok`.** The runbook below reproduces that result.

---

## Goal

**Feature Goal**: Execute the 5-gate quality sequence (fmt → clippy → release build →
trayless build → single-threaded tests) against the shipped code and confirm the 5
capability-keyed-lifecycle tests pass, producing an evidence pass/fail report. **No
code is written or modified** — this is a verification gate run.

**Deliverable**: A pass/fail report covering (a)–(e) below, the total test count from
(e), and the explicit per-test confirmation (`... ok`) for the 5 named tests. Any
failing gate is captured verbatim and flagged as a regression to investigate.

**Success Definition**: All 5 gates exit 0; `cargo test --bin qmkonnect -- --test-threads=1`
reports `389 passed; 0 failed`; each of the 5 named tests appears in the output with
`... ok`. (If a gate fails or a named test is missing/FAILS, that is the report's
headline — do not "fix" it here; flag it for P1.M1.T1.S2/S3 or a code investigation.)

## Verified baseline (run during research — reproduce this)

| Gate | Command | Result (this session) |
|---|---|---|
| (a) | `cargo fmt --all --check` | ✅ exit 0 (clean) |
| (b) | `cargo clippy --all-targets -- -D warnings` | ✅ exit 0 (no warnings) |
| (c) | `cargo build --release` | ✅ exit 0 |
| (d) | `cargo build --release --no-default-features` | ✅ exit 0 (**10 expected dead_code warnings** — see Gotchas; not a regression) |
| (e) | `cargo test --bin qmkonnect -- --test-threads=1` | ✅ **389 passed; 0 failed; 0 ignored** (13.58s) |

Named-test confirmations (each printed `... ok`):
- `test core::notifier::tests::test_presence_tick_capable_unplug_with_non_capable_remaining_is_loss ... ok`
- `test core::notifier::tests::test_presence_tick_capable_replug_different_board_is_gain ... ok`
- `test core::notifier::tests::test_presence_tick_stable_bus_no_reprobe_no_action ... ok`
- `test core::notifier::tests::test_handshake_warm_eligible_single_board_only ... ok`
- `test core::notifier::tests::test_classify_candidates_mixed ... ok`

## User Persona (if applicable)

**Target User**: The release/maintainer who needs a green quality-gate signal before
declaring the capability-keyed lifecycle delta (commit `d240b27`) verified and ready,
and the S2/S3 sibling tasks (spec-drift audit, caveat-backing audit) that depend on a
known-green baseline.

**Use Case**: Confirm the already-shipped code passes the documented dev-loop gates
(AGENTS.md §6) and that the 5 tests backing the headline lifecycle properties
(PresenceTracker capable-unplug→Loss, capable-replug→Gain, stable-bus→no-reprobe,
warm-feed single-board-only scope guard, classify mixed desk) are green.

**Pain Points Addressed**: Removes uncertainty about whether the shipped delta is
gate-clean before the read-only spec/caveat audits (S2/S3) build on it.

## Why

- **The delta is already shipped.** Per
  `plan/006_8f4080956ee0/architecture/verification_findings.md`, commit `d240b27`
  shipped the capability-keyed lifecycle (PresenceTracker, warm-feed scope guard,
  proto-v1 dedup asymmetry) to BOTH code and spec. This subtask is the **independent
  machine-check** that the shipped code actually passes its own gates.
- **The 5 named tests are the load-bearing evidence** for the PRD's headline
  multi-board claims (`spec/DEVICE_DISCOVERY.md` §3/§2.4): the truthful
  capable-unplug-while-VIA-remains `Loss`, the replug-different-board `Gain`, the
  stable-bus no-reprobe efficiency, the warm-feed single-board-only scope guard, and
  the mixed-desk classification. Confirming they pass IS the verification.
- **It is the gating baseline for the rest of P1.M1.T1.** S2 (spec-drift audit) and
  S3 (caveat-backing audit) are read-only and assume the code is correct; a green
  gate run here is the prerequisite signal.

## What

Run, in order, the 5 gates below (each must pass for the report to be "green"; capture
verbatim output on any failure). Then grep the gate-(e) output for the 5 named tests.

### The 5 gates (exact commands, exact order)

```bash
cd /home/dustin/projects/qmkonnect

# (a) Format check (CI gate; ci.yml runs `cargo fmt --all -- --check`)
cargo fmt --all --check                                                 # expect exit 0, no output

# (b) Clippy, all targets, warnings = errors (dev-loop gate; AGENTS.md §6.3)
cargo clippy --all-targets -- -D warnings                               # expect exit 0, no warnings

# (c) Release build, default features (full app incl. tray)
cargo build --release                                                   # expect exit 0, "Finished"

# (d) Trayless release build (the documented --no-default-features caveat path)
cargo build --release --no-default-features                             # expect exit 0
#   NOTE: this prints ~10 dead_code/`never used` warnings — EXPECTED, not a regression
#   (see Known Gotchas). The gate is BUILD SUCCESS (exit 0), not warning-free.

# (e) Unit tests — SINGLE-THREADED (shared global debouncer state; AGENTS.md §6)
cargo test --bin qmkonnect -- --test-threads=1                          # expect 389 passed; 0 failed
```

### The 5 named tests to confirm (grep gate-(e) output)

All live in `src/core/notifier.rs` (confirmed line anchors):

| Test | Anchor | What it backs |
|---|---|---|
| `test_presence_tick_capable_unplug_with_non_capable_remaining_is_loss` | notifier.rs:3012 | §3 truthful `Loss` when a capable board is unplugged while a non-capable (VIA) Tier-1 board remains |
| `test_presence_tick_capable_replug_different_board_is_gain` | notifier.rs:3026 | §3 replugging a *different* capable board is a real `Gain` (re-handshake, no restart) |
| `test_presence_tick_stable_bus_no_reprobe_no_action` | notifier.rs:3037 | §3 `PresenceTracker` does NOT re-probe on a stable bus ⇒ `HandshakeAction::None` |
| `test_handshake_warm_eligible_single_board_only` | notifier.rs:4097 | §2.4 warm-feed scope guard (`candidate_count <= 1`) — no false `✓ qmk_notifier` stamp on a mixed desk |
| `test_classify_candidates_mixed` | notifier.rs:3894 | §2.3 mixed-desk per-candidate classification (capable + NotQmkNotifier) |

```bash
# Confirm each named test ran and passed (each line must end in '... ok').
grep -E 'test_presence_tick_capable_unplug_with_non_capable_remaining_is_loss|test_presence_tick_capable_replug_different_board_is_gain|test_presence_tick_stable_bus_no_reprobe_no_action|test_handshake_warm_eligible_single_board_only|test_classify_candidates_mixed' /tmp/gate_e_test.log
# Expect: 5 lines, each ending "... ok". No "... FAILED", no missing line.

# Total count.
grep -E 'test result:' /tmp/gate_e_test.log
# Expect: "test result: ok. 389 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; ..."
```

### Success Criteria

- [ ] Gate (a) `cargo fmt --all --check` → exit 0.
- [ ] Gate (b) `cargo clippy --all-targets -- -D warnings` → exit 0, no warnings.
- [ ] Gate (c) `cargo build --release` → exit 0.
- [ ] Gate (d) `cargo build --release --no-default-features` → exit 0 (warnings allowed — see Gotchas).
- [ ] Gate (e) `cargo test --bin qmkonnect -- --test-threads=1` → `389 passed; 0 failed`.
- [ ] All 5 named tests present in gate-(e) output, each ending `... ok`.
- [ ] Pass/fail report produced (per-gate + named-test confirmation + total count).
- [ ] No source file modified (`git status` clean for `src/`, `Cargo.toml`, `Cargo.lock`).

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed
> to run this verification successfully?"_ — **Yes.** The 5 exact commands (in order),
> the expected result for each (with the verified baseline numbers), the 5 named tests
> with line anchors + the grep to confirm them, the trayless-warnings characterization
> (so they aren't mistaken for a regression), and the failure-capture/reporting
> procedure are all below.

### Documentation & References

```yaml
# MUST READ — the verification findings (what the delta is + which tests back which claim)
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/architecture/verification_findings.md
  why: "§1 maps every PRD claim to its code location (all verified TRUE). §2 lists the test coverage
        (17 PresenceTracker/warm-feed/classify tests; the 5 named here are the headline subset).
        §4 documents the 7-phase validate.sh + the single-threaded test requirement."
  section: "1. Verified Implementation Claims", "2. Verified Test Coverage", "4. Quality Gates"
  critical: "The delta is ALREADY SHIPPED (commit d240b27). This subtask only RUNS gates; it writes
             no code. The 5 named tests are the load-bearing evidence for §3 (PresenceTracker) + §2.4
             (warm-feed scope)."

# MUST READ — the canonical 7-phase gate script (defines the authoritative sequence)
- file: /home/dustin/projects/qmkonnect/plan/005_8b95ea464bd9/validate.sh
  why: "Defines the project's 7 quality phases. This subtask runs phases 1-4 (fmt, clippy, build×3,
        tests). Phases 5-7 (E2E CLI, spec invariants, hid-id-on-hardware) are out of scope for S1
        (S1 is the core dev-loop gate subset the contract enumerates)."
  section: "Phase 1 (fmt), Phase 2 (clippy), Phase 3 (build: default/all-targets/no-default-features/hid-id), Phase 4 (tests)"
  critical: "Phase 4 MUST be single-threaded (--test-threads=1) — shared global debouncer state. The
             contract's gate (e) is exactly Phase 4's command."

# MUST READ — the dev-loop gate definition (the canonical test command + why single-threaded)
- file: /home/dustin/projects/qmkonnect/AGENTS.md
  why: "Defines the core dev-loop gate: `cargo test --bin qmkonnect -- --test-threads=1` (single-threaded
        because of shared global debouncer state). The Linux section adds `cargo clippy --all-targets
        -- -D warnings`."
  section: "macOS / Windows / Linux dev test loop"
  critical: "NEVER run the test suite without --test-threads=1 — shared global state (DebounceState +
             mock globals) makes parallel tests flaky/false. The contract fixes this exact flag."

# REFERENCE — the spec the named tests back (the capability-keyed lifecycle + three-state status)
- file: /home/dustin/projects/qmkonnect/spec/DEVICE_DISCOVERY.md
  why: "§2.4 (warm-feed scope) + §3 (PresenceTracker capable-keyed lifecycle) are the spec passages
        the 5 named tests back. Useful if a test fails and you need to reason about expected behavior."
  section: "2.4 Relationship to the host-rules handshake", "3. Device-Status Semantics"

# REFERENCE — research notes for this subtask (verified gate results + warning characterization)
- docfile: plan/006_8f4080956ee0/P1M1T1S1/research/notes.md
  why: "Captures the verified baseline run (all 5 gates green, 389 tests, 5 named tests ok) and the
        full list of the 10 trayless-build dead_code warnings (so a future run can diff them)."
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/                       # THIS repo (HEAD = f4315a6; code at d240b27 unchanged)
├── AGENTS.md                    # dev-loop gate definition (--test-threads=1)
├── Cargo.toml                   # [features] default = ["hyprland","macos","linux-tray"]; --no-default-features = trayless
├── src/core/notifier.rs         # the 5 named tests live here (lines 3012/3026/3037/3894/4097)
├── plan/005_8b95ea464bd9/validate.sh              # the 7-phase canonical gate script
└── plan/006_8f4080956ee0/architecture/verification_findings.md  # what the delta is + test map
```

### Desired Codebase tree with files to be added/modified

```bash
( NONE — this is a verification subtask. No source changes. `git status` for src/ must stay clean. )
```

> The only artifacts are the captured log files (`/tmp/gate_*.log`) and the pass/fail
> report you produce. Do not commit log files.

### Known Gotchas of our codebase & Library Quirks

```text
# CRITICAL: tests MUST run single-threaded.
#   `cargo test --bin qmkonnect -- --test-threads=1`. Shared global state (DebounceState + mock
#   globals + HOST_CAPABLE/HAS_HANDSHAKED AtomicBools) makes parallel runs flaky/false. The contract
#   and AGENTS.md fix this exact flag. Do NOT drop it "to go faster".

# CRITICAL: gate (d) trayless build prints ~10 warnings — EXPECTED, not a regression.
#   They are all dead_code / `never used` / one `unused variable`:
#     - struct PresenceTracker is never constructed
#     - associated items `new` and `tick` are never used      (PresenceTracker)
#     - function presence_tick_decision is never used
#     - function tier1_paths is never used
#     - function handshake_action is never used
#     - enum HandshakeAction is never used
#     - function reset_handshake_state is never used
#     - function list_foreground_windows is never used
#     - function render_config_body is never used
#     - unused variable: `verbose`  (src/tray.rs:297)
#   Cause: with --no-default-features the tray poll threads (the ONLY callers of these pure fns) are
#   feature-gated out, so the fns become unreferenced. The gate is BUILD SUCCESS (exit 0), NOT
#   warning-free. (Clippy -D warnings (gate b) runs on the DEFAULT feature set, where these ARE used
#   by the tray, so it is clean.) If you see a warning that is NOT in this list, OR the build exits
#   non-zero, THAT is a regression — capture it.

# CRITICAL: do NOT "fix" anything. This is verification only.
#   If a gate fails or a named test fails, capture the verbatim output and report it as a regression
#   for investigation (P1.M1.T1.S2/S3 or a code task). Do not edit src/, Cargo.toml, or Cargo.lock.
#   `git status` for source files must remain clean after the run.

# NOTE: warm target cache. target/ is ~20G with cached debug/release artifacts, so gates (b)-(e) are
#   incremental (~0.1-14s each in the research run). A cold run (fresh clone) will take minutes for
#   the release build + test compile; allow time accordingly.

# NOTE: gate (e) count is 389 (research baseline). If the count differs on a later run, that is not
#   necessarily a failure — tests may have been added/removed by a later commit. The gate is
#   "0 failed" + the 5 named tests present+ok, not an exact count match. Report the actual count.

# NOTE: clippy `--all-targets` (gate b) covers default-feature tests/examples/benches, NOT the
#   trayless build. The 10 trayless warnings are from plain `cargo build` (gate d), which does not
#   apply -D warnings. Do not conflate the two.
```

## Implementation Blueprint

### Data models and structure

Not applicable — no data models, no code. This is a gate-run runbook.

### Implementation Tasks (ordered: the gate sequence itself)

```yaml
Task 1: CONFIRM the working tree is the shipped delta (no uncommitted code changes)
  - RUN: git -C /home/dustin/projects/qmkonnect status --short src/ Cargo.toml Cargo.lock
  - EXPECT: empty (clean). HEAD should be f4315a6 (or d240b27). If src/ is dirty, STOP and surface it
          — verification must run against committed code, not a dirty tree.

Task 2: GATE (a) — format check
  - RUN: cargo fmt --all --check 2>&1 | tee /tmp/gate_a_fmt.log
  - EXPECT: exit 0, no diff output. On non-zero: capture the diff (it lists unformatted files) — flag
          as a regression (a committed file isn't rustfmt-clean).

Task 3: GATE (b) — clippy, warnings = errors
  - RUN: cargo clippy --all-targets -- -D warnings 2>&1 | tee /tmp/gate_b_clippy.log
  - EXPECT: exit 0, "Finished", no warning: lines. On any warning/error: capture verbatim — flag as
          a regression. (This runs on the DEFAULT feature set; it does NOT cover the trayless build.)

Task 4: GATE (c) — release build (default features, full app)
  - RUN: cargo build --release 2>&1 | tee /tmp/gate_c_release.log
  - EXPECT: exit 0, "Finished `release` profile [optimized]". On error: capture verbatim — flag.

Task 5: GATE (d) — trayless release build (the documented caveat path)
  - RUN: cargo build --release --no-default-features 2>&1 | tee /tmp/gate_d_nodefault.log
  - EXPECT: exit 0. ~10 dead_code/`never used` warnings ARE EXPECTED (see Gotchas for the exact list).
          On non-zero exit OR a warning NOT in the expected list: capture verbatim — flag.

Task 6: GATE (e) — single-threaded unit tests (THE headline gate)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1 2>&1 | tee /tmp/gate_e_test.log
  - EXPECT: "test result: ok. 389 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out".
          On any failed test: capture the `... FAILED` line + the panic — flag as a regression.

Task 7: CONFIRM the 5 named tests
  - RUN: grep -E '<the 5 test names>' /tmp/gate_e_test.log   (see the What section for the exact regex)
  - EXPECT: exactly 5 lines, each ending "... ok". If any is missing or ends "... FAILED": flag.
  - RUN: grep -E 'test result:' /tmp/gate_e_test.log  → record the total count in the report.

Task 8: PRODUCE the report (the deliverable)
  - REPORT: per-gate pass/fail (a-e) with exit codes; total test count; the 5 named-test `ok` lines
          (verbatim); any captured failure output. State the overall verdict (GREEN / REGRESSION).
  - DO NOT: modify any source file. `git status` for src/ must remain clean.
```

### Implementation Patterns & Key Details

```bash
# === THE CANONICAL TEST COMMAND (never modify) ===
cargo test --bin qmkonnect -- --test-threads=1
#   ^--bin qmkonnect        only the app's unit tests (the named tests live in src/core/notifier.rs)
#   ^--test-threads=1       MANDATORY: shared global debouncer + mock state (AGENTS.md)

# === NAMED-TEST GREP (one call confirms all 5) ===
grep -E 'test_presence_tick_capable_unplug_with_non_capable_remaining_is_loss|test_presence_tick_capable_replug_different_board_is_gain|test_presence_tick_stable_bus_no_reprobe_no_action|test_handshake_warm_eligible_single_board_only|test_classify_candidates_mixed' /tmp/gate_e_test.log

# === TRAYLESS BUILD = --no-default-features ===
# Cargo.toml: default = ["hyprland","macos","linux-tray"]. Disabling all three yields the minimal
# trayless service build (spec/LINUX.md trayless caveat). Its dead_code warnings are expected.
```

### Integration Points

```yaml
SOURCE FILES:
  - modify: "NONE. `git status` for src/, Cargo.toml, Cargo.lock must stay clean."

GATES (the 5-phase subset of validate.sh phases 1-4):
  - (a) fmt:        "cargo fmt --all --check"
  - (b) clippy:     "cargo clippy --all-targets -- -D warnings"
  - (c) release:    "cargo build --release"
  - (d) trayless:   "cargo build --release --no-default-features"
  - (e) tests:      "cargo test --bin qmkonnect -- --test-threads=1"

OUT OF SCOPE (validate.sh phases 5-7; NOT this subtask):
  - phase 5: "E2E CLI subcommands (user workflows)"
  - phase 6: "Spec invariants (protocol, R-COEX, udev safety, template cleanliness)"
  - phase 7: "qmkonnect-hid-id helper against real hardware (needs a QMK board)"

DOWNSTREAM CONSUMERS of this verification:
  - P1.M1.T1.S2: "Spec-drift audit (the 6 v6 diff hunks) — read-only; assumes code is correct."
  - P1.M1.T1.S3: "Caveat-backing audit (read-only code audit) — assumes green gates."
  - A failing gate here is the signal to STOP those audits and investigate the regression first.
```

## Validation Loop

> The Validation Loop for THIS subtask IS the gate sequence above (Tasks 2–7). The
> "validation" is that the run reproduces the verified baseline. The levels below are
> the report-production checks.

### Level 1: Reproducibility (the run itself)

```bash
cd /home/dustin/projects/qmkonnect

# Re-run is deterministic for (a)-(e) on an unchanged tree. Expected results (research baseline):
#   (a) exit 0 | (b) exit 0, no warnings | (c) exit 0 | (d) exit 0 (+~10 expected warnings)
#   (e) 389 passed; 0 failed; 5 named tests ok.
# If a re-run diverges from the baseline on an UNCHANGED tree, that itself is a finding — report it.
```

### Level 2: Named-test confirmation (the headline)

```bash
# Exactly 5 lines, each "... ok". No FAILED, no missing.
grep -cE 'test_presence_tick_capable_unplug_with_non_capable_remaining_is_loss|test_presence_tick_capable_replug_different_board_is_gain|test_presence_tick_stable_bus_no_reprobe_no_action|test_handshake_warm_eligible_single_board_only|test_classify_candidates_mixed' /tmp/gate_e_test.log
# Expected: 5
grep -cE 'FAILED' /tmp/gate_e_test.log
# Expected: 0
```

### Level 3: No-source-change invariant

```bash
cd /home/dustin/projects/qmkonnect
git status --short src/ Cargo.toml Cargo.lock Cargo.lock
# Expected: empty (clean). A verification subtask must not modify source.
```

### Level 4: Failure triage (only if a gate failed)

```text
If a gate failed, classify it before reporting:
- fmt (a):    a committed file isn't rustfmt-clean → regression (run `cargo fmt --all` is the FIX,
              but DO NOT apply it here — flag it; a fix is a separate code task).
- clippy (b): a new lint warning on the default build → regression (capture the warning + location).
- build (c):  release compile error → regression (capture the error; this blocks release).
- trayless (d): ONLY flag if exit != 0 OR a warning is NOT in the expected 10-item list (Gotchas).
- test (e):   a test FAILED or a named test is MISSING → regression (capture the panic; for a missing
              named test, confirm via `grep -n '<name>' src/core/notifier.rs` that it still exists —
              if it was renamed/removed, that is a larger finding).
Report each failure with the verbatim output + your classification.
```

## Final Validation Checklist

### Technical Validation

- [ ] Task 1: working tree clean for `src/`, `Cargo.toml`, `Cargo.lock` (run against committed code).
- [ ] Gate (a) `cargo fmt --all --check` → exit 0.
- [ ] Gate (b) `cargo clippy --all-targets -- -D warnings` → exit 0, no warnings.
- [ ] Gate (c) `cargo build --release` → exit 0.
- [ ] Gate (d) `cargo build --release --no-default-features` → exit 0 (warnings expected).
- [ ] Gate (e) `cargo test --bin qmkonnect -- --test-threads=1` → 0 failed (record total count).

### Feature Validation

- [ ] All 5 named tests present in gate-(e) output, each ending `... ok`.
- [ ] `grep -cE 'FAILED' /tmp/gate_e_test.log` → 0.
- [ ] Total test count recorded in the report (baseline 389; report actual).

### Code Quality Validation

- [ ] `git status --short src/ Cargo.toml Cargo.lock` → clean (no source modified).
- [ ] No log files committed to the repo.
- [ ] Report states an explicit overall verdict (GREEN / REGRESSION).

### Documentation & Deployment

- [ ] No user-facing/config/API surface change (verification-only; DOCS = none per contract).
- [ ] Pass/fail report captured (per-gate + named-test confirmation + total count + any failure output).

---

## Anti-Patterns to Avoid

- ❌ Don't run tests without `--test-threads=1` — shared global debouncer + mock state make parallel
  runs flaky (AGENTS.md). The contract fixes this exact flag.
- ❌ Don't flag the 10 trayless-build `dead_code`/`never used` warnings as a regression — they are the
  EXPECTED consequence of feature-gating out the tray poll threads (the only callers). Gate (d) is
  build-success (exit 0), not warning-free. Only flag if exit != 0 OR a warning is outside the listed set.
- ❌ Don't conflate clippy `--all-targets` (gate b, default features, `-D warnings`) with the trayless
  `cargo build` (gate d, no `-D warnings`). They cover different feature sets.
- ❌ Don't "fix" a failing gate here — this is verification only. Capture the verbatim failure and
  report it as a regression for a code task. Editing source to make a gate green defeats the purpose.
- ❌ Don't require the test count to be exactly 389 on a later run — later commits may add/remove tests.
  The gate is "0 failed + the 5 named tests present+ok", not an exact count. Report the actual count.
- ❌ Don't run validate.sh phases 5–7 (E2E CLI, spec invariants, hid-id-on-hardware) — they are out of
  scope for S1 (S1 is the contract's 5-gate subset = validate.sh phases 1–4).
- ❌ Don't skip the working-tree-clean check (Task 1) — verification must run against committed code,
  not a dirty tree; a dirty `src/` invalidates the result.
- ❌ Don't commit the `/tmp/gate_*.log` artifacts or any report file into the repo.
- ❌ Don't drop `--bin qmkonnect` from the test command — the named tests are in the app's unit tests
  (`src/core/notifier.rs`); `cargo test` without `--bin` would also run integration/doc tests.

---

**Confidence Score: 10/10** for one-pass execution success. This is a verification
runbook whose commands are quoted verbatim from the contract + AGENTS.md + validate.sh,
whose 5 named tests are confirmed to exist at the cited line anchors, and whose
expected results are not aspirational but a **verified baseline run during research**
(all 5 gates green; 389 passed; 0 failed; all 5 named tests `ok`; the 10 trayless
warnings characterized as expected). The only residual variable is build time on a
cold `target/` (the research run was warm); the commands and expectations are fixed.
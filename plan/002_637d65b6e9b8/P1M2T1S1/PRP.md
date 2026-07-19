# PRP — P1.M2.T1.S1: Add 0xF0 discriminator check and `handle_typed_command()` skeleton in `hid_notify()`

> **Repo under change:** the **qmk-notifier FIRMWARE** (C) at
> `/home/dustin/projects/qmk-notifier` — remote `git@github.com:dabstractor/qmk-notifier`,
> branch `main`. This is **NOT** the `qmk_notifier` Rust crate (P1.M1) and does **not**
> consume the v0.3.0 crate tag being cut in parallel (P1.M1.T4.S1). Firmware ↔ crate are
> independent layers; this task needs no wait on the crate release.

---

## ⚠️ READ FIRST — this is a VERIFY & ALIGN task, NOT greenfield

**HEADLINE FINDING (research-confirmed this session):** The firmware repo **already
contains a complete, committed, tested implementation** of this exact deliverable. It was
implemented by the firmware repo's **own `plan/003_16d737de7a3e`** (the authoritative
firmware-side plan), where **P1.M2.T1.S1 is marked Complete**, along with the entire
P1.M2 milestone and the P1.M3.T1 host test suite. The QMKonnect `plan/002` P1.M2
milestone is a desktop-side **mirror/coordination view** of the same firmware feature.

Evidence:
- Commits on `main`: `c5ad578 Implement typed command dispatch and response builder`,
  `ab7055f Implement host-authoritative SET_OS command`,
  `779152a Implement APPLY_HOST_CONTEXT typed command handler`,
  `11a698f Implement host test suite for typed command queries`.
- `grep -n "typed_mode\|handle_typed_command\|NOTIFY_CMD" notifier.c` → the fork, the
  dispatcher, all four handlers, `set_host_layer`, `apply_host_callbacks`, and the host
  state globals are all present.
- The contract's line numbers are **STALE** (it cites `hid_notify` 543-577 and host state
  137-139; actual host state is at `notifier.c:143-145` and `hid_notify` spans ~748-825).
  This confirms the work-item text was authored against a **pre-implementation** snapshot.

➡️ **Therefore this PRP's deliverable is VERIFICATION + ALIGNMENT, not new code.** An
implementation agent that "creates the skeleton" by ripping out what exists would
**regress** the firmware (see the regression warning below).

---

## 🚨 CRITICAL REGRESSION WARNING — do NOT implement the literal contract text

The work-item **contract text** describes a **simplified, single-report** sketch:

> *"after `data += 2; length -= 2;`, add `if (length >= 1 && data[0] == NOTIFY_CMD_DISCRIMINATOR) { handle_typed_command(data + 1, length - 1); return; }`" and *"Implement `static void handle_typed_command(uint8_t *data, uint8_t length)` that switches on data[0]"*

The **actual** implementation (already on `main`) uses the **multi-report** approach that
PRD §5 (`h2.83`) **mandates**: *"`[0x81][0x9F][0xF0][cmd_id][args…][0x03]`, ETX-framed and
**multi-report** like strings (chunked at 30 payload bytes/report) … **`APPLY_HOST_CONTEXT`
may span reports**."*

| Literal contract (DO NOT implement) | Actual on `main` (CORRECT — keep) |
|---|---|
| check `data[0]==0xF0` **after** `data += 2`, per report | `if (msg_index==0 && length>=3 && data[2]==NOTIFY_CMD_DISCRIMINATOR) typed_mode=true;` **before** strip, **first report only** (`notifier.c:760`) |
| dispatch immediately, single report | reassemble into `msg_buffer` across reports; at ETX with `typed_mode` → `handle_typed_command(msg_buffer)` (`notifier.c:785`) |
| `handle_typed_command(uint8_t*, uint8_t)`, switch on `data[0]` | `handle_typed_command(char*)`, switch on `data[1]` (cmd_id); `data[0]` is the 0xF0 discriminator; returns `bool` (`notifier.c:651`) |

**Why the literal approach is wrong:** checking the discriminator *after* stripping, *per
report*, cannot classify a multi-report typed command (continuation reports carry payload
at `data[2]`, which may coincidentally be `0xF0`). Only a **first-report** classification
into a persistent `typed_mode` flag (reset at ETX/overflow) is correct — and that is what
exists. **Align your UNDERSTANDING to the code; do NOT align the code to the contract text.**

---

## Goal

**Feature Goal**: Confirm the firmware's `hid_notify()` typed-command routing fork and the
`handle_typed_command()` dispatch skeleton satisfy every point of the P1.M2.T1.S1
contract (and the authoritative PRD §4.6 / §14 / §5), that the committed stub-compile gate
is green, and that legacy string behaviour is byte-identical. **No source change is
expected** unless a genuine skeleton-level defect is found (none was found in research).

**Deliverable**: A verification report (inline in the implementation session) that maps each
contract point to its existing `notifier.c`/`notifier.h` location, shows the passing test
gate, and records any delta. If (and only if) a real defect is found, a minimal surgical
fix in `notifier.c`/`notifier.h` that keeps the multi-report architecture intact.

**Success Definition**:
- Every contract point in the table below is satisfied by existing code (verified by read + grep).
- `./run_notifier_stub_tests.sh` prints `✓ notifier stub-compile gate PASSED` with
  `test_notifier_dispatch` **14/14** and `test_notifier_os` **31/31**, 0 FAIL.
- `test_notifier_host.c` (typed suite, built manually) shows **0 failures in skeleton
  scope** (the 7 known SET_OS-handler failures are out of scope — see Validation Level 3).
- `git diff` is **empty** at the end of the task (or, in the defect-fix case, a minimal,
  justified diff with all gates still green).

## User Persona (if applicable)

**Target User**: (1) The QMKonnect desktop host (P4 handshake P4.M2.T1) that sends
`QUERY_INFO` (`0x81 0x9F 0xF0 0x01 … 0x03`) on connect and needs typed commands to reach
the dispatcher **without** clearing board state. (2) The P1.M2.T2 implementers (handlers)
whose work already consumes this skeleton. (3) Every legacy keymap user — string messages
must behave exactly as before.

**Use Case**: Host sends `0x81 0x9F 0xF0 0x01 0x03` (QUERY_INFO). `hid_notify` classifies
the first report (`data[2]==0xF0` → `typed_mode=true`), strips the 2-byte magic, appends
`0xF0 0x01` to `msg_buffer`, and at ETX calls `handle_typed_command(msg_buffer)` which
replies `[0x51][0x01][proto_ver][flags][cb_count][board_rules]`. Board state is untouched
and no legacy 0/1 ack is sent.

## Why

- **Closes the QMKonnect-side tracking view** of a firmware feature already shipped in the
  firmware repo. The value this PRP adds is *preventing a regression*: an agent that takes
  the literal (single-report) contract at face value would destroy multi-report typed
  framing (a PRD §5 requirement) and break the committed dispatch/os/host test suites.
- **Enforces board/host orthogonality (PRD invariant 21):** typed commands touch ONLY host
  state; `process_full_message` (legacy) touches ONLY board state. The fork is the boundary.
- **Backward-compatible by construction (PRD §5):** legacy strings have a printable
  `data[2]` (0x20-0x7E), never `0xF0`, so the typed branch never fires for them — no
  `#ifdef`, both regression suites pass unchanged.

## What

Verify (do not rewrite) the following, all of which already exist in `notifier.c` / `notifier.h`:

### Success Criteria
- [ ] `notifier.h` defines `NOTIFY_CMD_DISCRIMINATOR 0xF0`, `NOTIFY_RESPONSE_MARKER 0x51`,
      `NOTIFY_CMD_QUERY_INFO 0x01`, `NOTIFY_CMD_QUERY_CALLBACK 0x02`, `NOTIFY_CMD_SET_OS 0x03`,
      `NOTIFY_CMD_APPLY_HOST_CONTEXT 0x05`, `NOTIFY_PROTO_VER 2`, `HOST_CALLBACK_MAX 32`,
      `HOST_LAYER_BASE 224`, `host_callback_t`, `DEFINE_HOST_CALLBACKS`, and the
      `get_host_callbacks`/`get_host_callbacks_size` accessors.
- [ ] `notifier.c` declares host state globals `host_layer` (L143), `host_cb_enabled[]` (L144),
      `has_been_queried` (L145) and the `typed_mode` flag (L96).
- [ ] `hid_notify()` sets `typed_mode=true` ONLY when `msg_index==0 && length>=3 &&
      data[2]==NOTIFY_CMD_DISCRIMINATOR` (first report), and resets it at the ETX boundary
      AND the overflow branch.
- [ ] On ETX with `typed_mode`: `handle_typed_command(msg_buffer)` runs; `process_full_message`
      and `sanitize_string` are NOT called; the post-loop legacy 0/1 ack is suppressed
      (`typed_dispatched` guard).
- [ ] On ETX with `!typed_mode`: the path is byte-identical to the pre-feature legacy path.
- [ ] `handle_typed_command(char*)` switches on `data[1]` (cmd_id), sets
      `has_been_queried=true` on the first QUERY_INFO, and each case builds a response via
      `send_typed_response` (the skeleton for P1.M2.T2.S1-S4 handlers, which already exist).
- [ ] `./run_notifier_stub_tests.sh` → `✓ notifier stub-compile gate PASSED`.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge of this codebase can, using only this PRP + the
firmware repo, (a) confirm the feature is present and correct, (b) run the gate, and
(c) avoid regressing it — because the divergence table, code map, and exact commands are all here.

### Documentation & References

```yaml
# MUST READ — PRD sections (QMKonnect plan/002 selected selectors; authoritative firmware
# detail is in the firmware repo's own PRD.md §4.6/§4.7/§14).
- url: spec/PRD.md (heading h2.83 "Wire Protocol (typed commands)")
  why: discriminator data[2]==0xF0; multi-report framing; command table; has_been_queried handshake
  critical: "APPLY_HOST_CONTEXT may span reports" — single-report contract would violate this
- url: spec/PRD.md (heading h2.84 "Firmware Spec (qmk-notifier)")
  why: typed dispatch at top of hid_notify; QUERY_INFO sets has_been_queried; board/host orthogonality
- url: spec/PRD.md (heading h2.75 "Firmware Reception Flow")
  why: the guard, strip, ETX reassembly, process_full_message ordering that typed commands fork off of

# MUST READ — existing firmware source (the thing being verified)
- file: /home/dustin/projects/qmk-notifier/notifier.h
  why: all NOTIFY_CMD_* constants, host_callback_t, DEFINE_HOST_CALLBACKS, accessors
  pattern: "#define NOTIFY_CMD_* …" block + "host_callback_t" struct + accessor decls
  gotcha: constants ALREADY exist — do not re-add or renumber (0x04 is intentionally reserved for VIA)
- file: /home/dustin/projects/qmk-notifier/notifier.c
  why: the full implementation under verification
  pattern: "L96 typed_mode; L143-145 host state; L204 board_rules_present; L252 set_host_layer;
           L283 apply_host_callbacks; L628 send_typed_response; L651 handle_typed_command;
           L748 hid_notify (L760 discriminator; L785 ETX dispatch; L818 legacy-ack suppression)"
  gotcha: the discriminator is checked on data[2] BEFORE strip, FIRST report only — NOT after data+=2

# MUST READ — the firmware's own (authoritative) PRP for this exact task
- file: /home/dustin/projects/qmk-notifier/plan/003_16d737de7a3e/P1M2T1S1/PRP.md
  why: the original 7-edit implementation spec that produced the committed code; use to confirm fidelity
  section: "What" (the 7 edits) and "Success Definition"

# Reference — existing tests
- file: /home/dustin/projects/qmk-notifier/test_notifier_dispatch.c  (14 dispatch tests)
- file: /home/dustin/projects/qmk-notifier/test_notifier_os.c        (31 multi-OS tests)
- file: /home/dustin/projects/qmk-notifier/test_notifier_host.c      (64 typed-command tests)
- file: /home/dustin/projects/qmk-notifier/run_notifier_stub_tests.sh (the committed gate)
```

### Current Codebase tree (firmware repo, verification-relevant only)

```bash
# run from /home/dustin/projects/qmk-notifier
notifier.h            # public API: NOTIFY_CMD_*, host_callback_t, DEFINE_HOST_CALLBACKS
notifier.c            # hid_notify(), handle_typed_command(), host state, helpers
qmk_stubs/            # host-compile stubs (QMK_KEYBOARD_H, raw_hid_send capture, os_detection)
test_notifier_dispatch.c   # 14-test legacy/dispatch regression suite
test_notifier_os.c         # 31-test multi-OS regression suite
test_notifier_host.c       # 64-test typed-command suite (not yet in the runner)
run_notifier_stub_tests.sh # committed gate: stub-compile + dispatch + os
run_all_tests.sh           # 9-suite pattern_match corpus (legacy path unaffected)
```

### Desired Codebase tree
**No files added or removed.** If a genuine defect is found (not expected), the fix is a
surgical edit inside `notifier.c` / `notifier.h` only. Build artifacts (`test_*` binaries
already gitignored) are regenerated by the runners.

### Known Gotchas of our codebase & Library Quirks
```c
// CRITICAL: notifier.c #includes a -D-expanded header name:
//   #include QMK_KEYBOARD_H        // QMK_KEYBOARD_H is a macro expanded by -DQMK_KEYBOARD_H='"...h"'
// It CANNOT compile standalone. ALWAYS use the stub harness:
//   gcc -DQMK_KEYBOARD_H='"qmk_keyboard_stub.h"' -Iqmk_stubs -I.  (see run_notifier_stub_tests.sh)
//
// CRITICAL: the discriminator MUST be classified on the FIRST report only (msg_index==0),
// reading data[2] (the byte after the 0x81 0x9F magic). Continuation reports carry payload
// there. typed_mode is the persistent classification flag; it is reset at EVERY ETX boundary
// AND the overflow branch (RISK-1 from the firmware's findings_and_risks.md).
//
// GOTCHA: handle_typed_command receives the WHOLE reassembled msg_buffer, so data[0]==0xF0
// (discriminator), data[1]==cmd_id, data[2..]==args. Switch on data[1], NOT data[0].
//
// GOTCHA: the 4 -Wunused warnings on stub-compile (host_layer, host_cb_enabled,
// has_been_queried, board_rules_present) are PRE-EXISTING and expected — these symbols are
// consumed by the handlers; a stub build that excludes handler-driving tests under-uses them.
// Do NOT silence them with (void) casts; they are not new and not yours to "fix".
```

## Implementation Blueprint

### Verification map — contract point → existing code (NO new data models; C firmware)

| # | Contract point | Existing location | Verified? |
|---|---|---|---|
| 1 | `NOTIFY_CMD_DISCRIMINATOR=0xF0`, `QUERY_INFO=0x01`, `QUERY_CALLBACK=0x02`, `SET_OS=0x03`, `APPLY_HOST_CONTEXT=0x05` in `notifier.h` | `notifier.h` `NOTIFY_CMD_*` block | ☐ |
| 2 | host state `host_layer`, `host_cb_enabled`, `has_been_queried` declared | `notifier.c:143-145` | ☐ |
| 3 | 0xF0 discriminator check routes to typed path AFTER magic-strip conceptually (first-report classification) | `notifier.c:760` (`data[2]==NOTIFY_CMD_DISCRIMINATOR`) | ☐ |
| 4 | typed command bypasses `process_full_message` (no board side effect) | `notifier.c:780-786` (`if (typed_mode) handle_typed_command …`) | ☐ |
| 5 | legacy ack suppressed for typed path | `notifier.c:818` (`if (!typed_dispatched)`) | ☐ |
| 6 | `handle_typed_command()` switches on cmd_id; builds 32-byte response via `raw_hid_send` | `notifier.c:651` + `send_typed_response` L628 (`RAW_REPORT_SIZE`=32) | ☐ |
| 7 | `has_been_queried=true` on first QUERY_INFO | `notifier.c:659` | ☐ |
| 8 | skeleton ready for consumption by P1.M2.T2.S1-S4 handlers (already present) | `notifier.c:658/672/693/709` | ☐ |

### Implementation Tasks (verification-ordered)

```yaml
Task 1: ESTABLISH baseline (no edits)
  - RUN: cd /home/dustin/projects/qmk-notifier && git status -s && git log --oneline -3
  - EXPECT: clean tree (or only pre-existing plan/test uncommitted files), HEAD at or past c5ad578
  - READ: notifier.h NOTIFY_CMD_* block + notifier.c L96, L143-145, L628-745, L748-825
  - CONFIRM every row of the Verification map above (check the ☐ boxes in your report)

Task 2: RUN the committed stub-compile gate
  - RUN: ./run_notifier_stub_tests.sh
  - EXPECT: stub-compile exit 0; test_notifier_dispatch 14/14; test_notifier_os 31/31;
           final line "✓ notifier stub-compile gate PASSED"
  - IF FAIL: read the failure; it indicates a REAL defect worth fixing (not a contract-literal
    mismatch). Diagnose root cause before any edit. The 4 -Wunused warnings are EXPECTED.

Task 3: BUILD+RUN the typed-command host suite (manual; not yet in the runner)
  - RUN:
      gcc -Wall -Wextra -std=c99 -DQMK_KEYBOARD_H='"qmk_keyboard_stub.h"' -Iqmk_stubs -I. \
          -c notifier.c -o /tmp/nh.o
      gcc -Wall -std=c99 -Iqmk_stubs -I. /tmp/nh.o qmk_stubs/qmk_stubs.c test_notifier_host.c \
          -o /tmp/test_notifier_host
      /tmp/test_notifier_host
  - EXPECT (current baseline): 64 run / 57 pass / 7 fail. The 7 failures are ALL SET_OS
    handler / §4.7 OS-change mechanics — OUT OF SCOPE for P1.M2.T1.S1 (the skeleton).
    Skeleton-relevant tests (discriminator routing, QUERY_INFO, QUERY_CALLBACK,
    APPLY_HOST_CONTEXT incl. two-report multi-report reassembly, legacy coexistence) MUST pass.
  - SCOPE RULE: do NOT attempt to fix the 7 SET_OS failures here. They are tracked by the
    firmware's plan/003 P1.M3.T2.S1 (Researching) and belong to P1.M2.T2.S2/S3.

Task 4: (DEFAULT) NO-OP — write the verification report, leave source untouched
  - IF Tasks 1-3 are green and the verification map is fully satisfied (expected path):
    the deliverable is the inline report. `git diff` stays empty. Done.
  - IF a genuine skeleton-level defect is found (unexpected):
    make the MINIMAL surgical fix in notifier.c/notifier.h that preserves the multi-report
    typed_mode architecture, then re-run Tasks 2 & 3 to confirm still-green + no new failures.
    Document the defect, the fix, and the before/after test counts in the report.

Task 5: NEVER do these
  - DO NOT rewrite hid_notify to check the discriminator "after data += 2" per the literal
    contract — that regresses multi-report framing (PRD §5) and breaks the gate.
  - DO NOT change handle_typed_command's signature to (uint8_t*, uint8_t) or switch on data[0].
  - DO NOT renumber the NOTIFY_CMD_* constants (0x04 is reserved for VIA).
  - DO NOT silence the 4 -Wunused stub-compile warnings.
  - DO NOT edit PRD.md, any tasks.json, or any plan/ files (read-only).
```

### Implementation Patterns & Key Details
```c
// The existing (correct) discriminator classification — first report only, pre-strip:
//   void hid_notify(uint8_t *data, uint8_t length) {
//       if (length < 2 || data[0] != 0x81 || data[1] != 0x9F) return;   // coexistence guard
//       if (msg_index == 0 && length >= 3 && data[2] == NOTIFY_CMD_DISCRIMINATOR) typed_mode = true;
//       data += 2; length -= 2;
//       for (...) { ...on ETX: if (!dropping) { if (typed_mode) { match = handle_typed_command(msg_buffer);
//                              typed_dispatched = true; } else { sanitize_string(...); match = process_full_message(...); } }
//                              msg_index = 0; dropping = false; typed_mode = false; break; ... }
//       if (!typed_dispatched) { response[0] = match; raw_hid_send(response, RAW_REPORT_SIZE); }   // legacy ack only

// The existing (correct) dispatcher signature — whole buffer, switch on data[1]:
//   static bool handle_typed_command(char *data) {   // data[0]==0xF0, data[1]==cmd_id, data[2..]==args
//       uint8_t cmd_id = (uint8_t)data[1];
//       switch (cmd_id) { case NOTIFY_CMD_QUERY_INFO: { has_been_queried = true; ... send_typed_response(...); break; } ... }
//       return true;   // typed path always replied — suppresses legacy ack
//   }
```

### Integration Points
```yaml
DEPENDENCIES: none — this is self-contained firmware C; it does NOT depend on the
              qmk_notifier Rust crate or its v0.3.0 tag (P1.M1.T4.S1, parallel).
DOWNSTREAM (this skeleton is consumed by — ALL ALREADY PRESENT):
  - P1.M2.T2.S1 QUERY_INFO/QUERY_CALLBACK handlers      (notifier.c:658, 672)
  - P1.M2.T2.S2 SET_OS handler                          (notifier.c:693)
  - P1.M2.T2.S3 APPLY_HOST_CONTEXT handler              (notifier.c:709)
  - P1.M2.T1.S2 set_host_layer, P1.M2.T1.S3 apply_host_callbacks (notifier.c:252, 283)
HOST (desktop) handshake that drives this — P4.M2.T1 (planned): sends QUERY_INFO on connect.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)
```bash
cd /home/dustin/projects/qmk-notifier
# The gate's step [1/4] does the canonical stub-compile. Do NOT compile notifier.c standalone.
gcc -Wall -Wextra -std=c99 -DQMK_KEYBOARD_H='"qmk_keyboard_stub.h"' -Iqmk_stubs -I. \
    -c notifier.c -o /tmp/notifier_stub.o
# Expected: exit 0 with ONLY the 4 pre-existing -Wunused warnings
# (host_layer, host_cb_enabled, has_been_queried, board_rules_present). No new warnings.
```

### Level 2: The committed regression suites (Component Validation)
```bash
cd /home/dustin/projects/qmk-notifier
./run_notifier_stub_tests.sh
# Expected: test_notifier_dispatch = 14/14; test_notifier_os = 31/31; 0 FAIL each;
#           final line: "✓ notifier stub-compile gate PASSED"
```

### Level 3: Typed-command host suite (skeleton-scope validation)
```bash
cd /home/dustin/projects/qmk-notifier
gcc -Wall -Wextra -std=c99 -DQMK_KEYBOARD_H='"qmk_keyboard_stub.h"' -Iqmk_stubs -I. \
    -c notifier.c -o /tmp/nh.o
gcc -Wall -std=c99 -Iqmk_stubs -I. /tmp/nh.o qmk_stubs/qmk_stubs.c test_notifier_host.c \
    -o /tmp/test_notifier_host
/tmp/test_notifier_host
# Expected baseline: 64 run / 57 pass / 7 fail. The 7 failures are ALL SET_OS (§4.7) —
# OUT OF SCOPE for the skeleton (P1.M2.T2.S2/S3 + firmware plan/003 P1.M3.T2.S1).
# Skeleton-relevant groups that MUST be 100% pass:
#   (i)   QUERY_INFO response layout + has_been_queried consequence
#   (iii) QUERY_CALLBACK valid index + out-of-range
#   AHC   APPLY_HOST_CONTEXT clear_board (stack/replace) + host layer + callback diff
#   multi two-report APPLY_HOST_CONTEXT reassembly (r[0]=0x51, r[1]=0x05, r[2]=ack=1)
#   coex  legacy 'firefox'/'neovide' NOT routed to typed (r[0]!=0x51)
```

### Level 4: Legacy-path regression (no typed command touches it)
```bash
cd /home/dustin/projects/qmk-notifier
./run_all_tests.sh
# Expected: the 9-suite pattern_match corpus is unaffected (legacy string path is
# byte-identical for !typed_mode). All suites pass.
```

## Final Validation Checklist

### Technical Validation
- [ ] Verification map (8 rows) fully satisfied by existing code.
- [ ] `./run_notifier_stub_tests.sh` → `✓ notifier stub-compile gate PASSED` (dispatch 14/14, os 31/31).
- [ ] `test_notifier_host.c` skeleton-scope groups 100% pass (7 SET_OS failures explicitly out of scope).
- [ ] `./run_all_tests.sh` — 9-suite pattern corpus unaffected.
- [ ] Stub-compile shows ONLY the 4 pre-existing -Wunused warnings (no new warnings).

### Feature Validation
- [ ] Discriminator classifies first-report `data[2]==0xF0` into `typed_mode`; resets at ETX + overflow.
- [ ] Typed path bypasses `process_full_message`/`sanitize_string`; legacy ack suppressed.
- [ ] `has_been_queried` set on first QUERY_INFO.
- [ ] Multi-report typed command (two-report APPLY_HOST_CONTEXT) reassembles and dispatches.
- [ ] Legacy strings (`firefox`, `neovide`) never route to typed.

### Code Quality Validation
- [ ] `git diff` is EMPTY (expected default) OR a minimal, justified defect-fix with all gates green.
- [ ] No re-implementation of the (regressive) literal single-report contract.
- [ ] No renumbering of NOTIFY_CMD_* constants (0x04 stays reserved for VIA).

### Documentation & Deployment
- [ ] No user-facing docs required (firmware C code — per the work-item DOCS: none).
- [ ] Verification report recorded inline (contract map + test counts + any delta).

---

## Anti-Patterns to Avoid
- ❌ Do NOT "implement the literal contract" (after-`data+=2`, single-report, switch on `data[0]`) — it regresses PRD §5 multi-report framing and breaks the gate.
- ❌ Do NOT rip out `typed_mode` / `handle_typed_command` and re-create them — they exist and are tested.
- ❌ Do NOT try to fix the 7 SET_OS handler failures here — out of scope (P1.M2.T2.S2/S3; firmware plan/003 P1.M3.T2.S1).
- ❌ Do NOT silence the 4 expected -Wunused stub-compile warnings.
- ❌ Do NOT edit PRD.md, tasks.json, prd_snapshot.md, or any plan/ file.
- ❌ Do NOT assume this task must produce a diff — the expected, correct outcome is a verification report with an empty diff.

---

## Confidence Score: 9/10

The deliverable is already present, correct, and green at its scope (verified this session:
stub-compile gate PASSED, dispatch 14/14, os 31/31, host suite skeleton-groups pass). The
1-point reservation is for the (unlikely) discovery of a genuine skeleton-level defect during
the implementation agent's own verification pass; if found, the minimal-fix path is specified.
The dominant risk this PRP neutralizes is a regression from naively implementing the literal
single-report contract text.
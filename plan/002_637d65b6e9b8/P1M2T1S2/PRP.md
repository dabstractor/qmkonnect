# PRP — P1.M2.T1.S2: Implement `set_host_layer()` and `apply_host_callbacks()` helper functions

> **Repo under change:** the **qmk-notifier FIRMWARE** (C) at
> `/home/dustin/projects/qmk-notifier` — remote `git@github.com:dabstractor/qmk-notifier`,
> branch `main`. This is **NOT** the `qmk_notifier` Rust crate (P1.M1) and does **not**
> consume the v0.3.0 crate tag (P1.M1.T4.S1, parallel). Firmware ↔ crate are independent
> layers. It is the **same repo** as P1.M2.T1.S1; these two helpers live alongside the
> dispatch skeleton that task verifies.

---

## ⚠️ READ FIRST — this is a VERIFY & ALIGN task, NOT greenfield

**HEADLINE FINDING (research-confirmed this session):** The firmware repo **already
contains a complete, committed, tested implementation** of *both* helper functions in
this work item. It was implemented by the firmware repo's **own `plan/003_16d737de7a3e`**
(the authoritative firmware-side plan), where `set_host_layer` is **P1M2T1S2 (Complete)**
and `apply_host_callbacks` is **P1M2T1S3 (Complete)**. The QMKonnect `plan/002` P1.M2
milestone is a desktop-side **mirror/coordination view** of the same firmware feature —
it folds those two firmware items into this single item.

Evidence (verified by read + grep + test runs this session):
- Both functions are present in `notifier.c`: `set_host_layer` at **L252-265**,
  `apply_host_callbacks` at **L283-317**.
- Their sole caller — the `APPLY_HOST_CONTEXT` handler — exists and is tested
  (`notifier.c:709-734`).
- The host state globals they mutate exist (`host_layer` L143, `host_cb_enabled[]` L144).
- Host suite `test_notifier_host.c` exercises set/clear/diff-ordering/idempotence and
  ALL helper-scope groups pass (see Validation Level 3).
- The contract's line numbers are **STALE** (it cites `notifier.c:137-138` for the host
  state and `~225-240` for `activate_layer`/`deactivate_layer`; actual host state is
  L143-145 and the board layer siblings are L218-235). This confirms the work-item text
  was authored against a **pre-implementation** snapshot.

➡️ **Therefore this PRP's deliverable is VERIFICATION + ALIGNMENT, not new code.** An
implementation agent that "implements the literal contract" by rewriting these helpers
would **regress** the firmware (see the regression warnings below) — most dangerously by
stripping the defensive bounds checks that prevent SIGSEGV on malformed host data, or by
calling `layer_off(255)` unconditionally in the clear branch.

---

## 🚨 CRITICAL REGRESSION WARNINGS — do NOT implement the literal contract text verbatim

The work-item **contract text** is *imprecise* in two places where the committed code is
*correct*. **Align your UNDERSTANDING to the code; do NOT align the code to the contract text.**

| # | Literal contract (DO NOT implement verbatim) | Actual on `main` (CORRECT — keep) | Why the code is right |
|---|---|---|---|
| 1 | `set_host_layer`: "if layer==0xFF, **layer_off(host_layer)** and set host_layer=LAYER_UNSET" (sounds unconditional) | `if (host_layer != LAYER_UNSET) layer_off(host_layer);` then `host_layer = LAYER_UNSET;` (L253-257) | Without the guard, when `host_layer==255` (unset), `layer_off(255)` would erroneously turn off **QMK layer 255**. The guard is a **correctness requirement**, not optional. |
| 2 | `apply_host_callbacks`: "Guard against NULL on_enable/on_disable and ids >= HOST_CALLBACK_MAX" (only two guards) | Both guards present AND additionally `id < cb_size` (registry bounds via `get_host_callbacks_size()`) checked before `cbs[id]` deref in **both** phases (L298, L310) | When `cb_size==0` (no `DEFINE_HOST_CALLBACKS`), `get_host_callbacks()` returns NULL; dereferencing `cbs[id]` would **SIGSEGV**. The registry-bounds check is the RISK-3 defense against malformed host data. Stripping it regresses safety. |

**Two more things you must NOT do:**
- Do NOT change either function's signature. They are `static void set_host_layer(uint8_t layer)`
  and `static void apply_host_callbacks(const uint8_t *ids, uint8_t count)`, file-local.
- Do NOT make them touch the board `activated_layer`. The whole point (architecture
  invariant 21) is that the host tracker is orthogonal to the board tracker. The board
  clear in replace mode is the **AHC handler's** job (`notifier.c:726-728`), NOT these helpers'.

---

## Goal

**Feature Goal**: Confirm the firmware's `set_host_layer()` and `apply_host_callbacks()`
helper functions satisfy every point of the P1.M2.T1.S2 contract (and the authoritative
PRD §14 / §4 architecture / §5 wire), that the committed stub-compile gate is green, and
that every helper-scope host test (set / change / clear / orthogonality / disable-before-
enable ordering / idempotence / multi-report) passes. **No source change is expected**
unless a genuine helper-level defect is found (none was found in research).

**Deliverable**: A verification report (inline in the implementation session) that maps each
contract point to its existing `notifier.c` location, shows the passing test gates, and
records the contract-vs-code deltas (the two documented above are *expected and correct*,
not defects). If (and only if) a real defect is found, a minimal surgical fix in
`notifier.c` that keeps the disable-before-enable + RISK-3-bounds architecture intact.

**Success Definition**:
- Every row of the verification map (below) is satisfied by existing code (verified by read + grep).
- `./run_notifier_stub_tests.sh` prints `✓ notifier stub-compile gate PASSED` with
  `test_notifier_dispatch` **14/14** and `test_notifier_os` **31/31**, 0 FAIL.
- `test_notifier_host.c` (built manually) shows **0 failures in helper scope** — the 7 known
  SET_OS-handler failures are out of scope (see Validation Level 3).
- `git diff` is **empty** at the end of the task (or, in the defect-fix case, a minimal,
  justified diff with all gates still green).

## User Persona (if applicable)

**Target User**: (1) The `APPLY_HOST_CONTEXT` typed-command handler (`notifier.c:709`,
the **sole caller** of both helpers), which in turn serves the QMKonnect desktop host
(P4.M2.T1 handshake) that pushes a host layer (≥ 224) and a desired callback id set per
window change. (2) Every keymap author who uses `DEFINE_HOST_CALLBACKS` and relies on the
disable-before-enable diff to fire `on_disable` on focus-out for free.

**Use Case**: Host sends `0x81 0x9F 0xF0 0x05 [layer=224] [flags] [count=1] [id=0] 0x03`
(APPLY_HOST_CONTEXT, stack mode). The handler calls `set_host_layer(224)` →
`layer_on(224)` (QMK highest-layer-wins makes 224 active), then `apply_host_callbacks({0},1)`
→ Phase 1 finds nothing to disable, Phase 2 fires `cbs[0].on_enable()`, sets `host_cb_enabled[0]=true`.
On the next window the host sends `layer=0xFF, count=0` → `set_host_layer(0xFF)` clears
the host layer, and `apply_host_callbacks({},0)` Phase 1 fires `cbs[0].on_disable()`,
clears `host_cb_enabled[0]`. Board `activated_layer` is untouched throughout (stack mode).

## Why

- **Closes the QMKonnect-side tracking view** of a firmware feature already shipped in the
  firmware repo. The value this PRP adds is *preventing a regression*: an agent that takes
  the literal (unguarded) contract at face value would (a) call `layer_off(255)` erroneously
  in the clear branch, and (b) strip the RISK-3 registry-bounds checks, reintroducing a
  SIGSEGV on malformed host data when no `DEFINE_HOST_CALLBACKS` is present.
- **Enforces board/host orthogonality at the function level (architecture invariant 21):**
  the board functions (`activate_layer`/`deactivate_layer`) mutate `activated_layer`;
  `set_host_layer` mutates `host_layer` only. Neither reads the other's variable. This is
  the code-level guarantee that "board and host state are orthogonal."
- **Codifies disable-before-enable (PRD §13 invariant 4):** `apply_host_callbacks` disables
  newly-out-of-set ids (firing `on_disable`) BEFORE enabling newly-in-set ids (`on_enable`),
  so a callback is never briefly in both states during a transition. This is proven — not
  just asserted — by a monotonic-sequence-stamp test (Level 3, case vii).

## What

Verify (do not rewrite) the following, all of which already exist in `notifier.c`:

### Success Criteria
- [ ] Host state globals present: `static uint8_t host_layer = LAYER_UNSET;` (L143) and
      `static bool host_cb_enabled[HOST_CALLBACK_MAX] = {false};` (L144). `LAYER_UNSET`
      defined as `255` (L126); `HOST_CALLBACK_MAX 32` and `HOST_LAYER_BASE 224` in `notifier.h`.
- [ ] `set_host_layer(uint8_t layer)` (L252): (a) `layer==LAYER_UNSET` ⇒ guarded
      `layer_off(host_layer)` then `host_layer=LAYER_UNSET`; (b) else guarded
      `layer_off(host_layer)` (if set) then `layer_on(layer)` + `host_layer=layer`.
      Touches ONLY `host_layer`.
- [ ] `apply_host_callbacks(const uint8_t *ids, uint8_t count)` (L283): Phase 1 (disable)
      iterates `id` in `0..HOST_CALLBACK_MAX`, for each `host_cb_enabled[id]` NOT in the new
      `ids` set, guards `id < cb_size` then NULL-guards `cbs[id].on_disable`, fires it, and
      clears the flag. Phase 2 (enable) iterates the new set, skips `id >= HOST_CALLBACK_MAX`
      and `id >= cb_size` and already-enabled, NULL-guards `on_enable`, fires it, sets the
      flag. Phase 1 strictly before Phase 2.
- [ ] Neither function reads or writes the board `activated_layer`; neither calls
      `activate_layer`/`deactivate_layer`/`disable_command`.
- [ ] `./run_notifier_stub_tests.sh` → `✓ notifier stub-compile gate PASSED`.
- [ ] `test_notifier_host.c` helper-scope groups (v/vi/vii/viii/multi-rep) 100% pass.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge of this codebase can, using only this PRP + the
firmware repo, (a) confirm both helpers are present and correct, (b) run the gates, and
(c) avoid regressing them — because the contract-vs-code delta table, code map, and exact
commands are all here.

### Documentation & References

```yaml
# MUST READ — PRD sections (QMKonnect plan/002 selected selectors; authoritative firmware
# detail is in the firmware repo's own PRD.md §13/§14/§4).
- url: spec/PRD.md (heading h2.84 "Firmware Spec (qmk-notifier)")
  why: "Second layer tracker host_layer ... set_host_layer(layer): layer_on/off the host
        tracker only; 0xFF ⇒ clear. apply_host_callbacks(ids, count): disable-before-enable diff"
  critical: "host_cb_enabled[]; disable-before-enable diff (fire on_disable for ids leaving,
             on_enable for ids entering); clear_board is the handler's job, NOT these helpers'"
- url: spec/PRD.md (heading h2.82 "Architecture & Coexistence Model")
  why: "two independent layer trackers: activated_layer (board) and host_layer (host) are
        orthogonal; host layers sit ≥ 224 so they resolve above board layers"
  critical: "In replace mode the board tracker is cleared for that window (the host's
             clear_board flag) — that clear is done by the AHC handler, not set_host_layer"

# MUST READ — existing firmware source (the thing being verified)
- file: /home/dustin/projects/qmk-notifier/notifier.c
  why: both helpers + their host state globals + their sole caller
  pattern: "L126 LAYER_UNSET; L143 host_layer; L144 host_cb_enabled; L218 activate_layer;
            L226 deactivate_layer; L252 set_host_layer; L283 apply_host_callbacks;
            L709 APPLY_HOST_CONTEXT handler (caller, L729-730)"
  gotcha: "the clear branch GUARDS layer_off (host_layer != LAYER_UNSET) — required to avoid
           erroneously turning off QMK layer 255; apply_host_callbacks checks BOTH
           HOST_CALLBACK_MAX and the registry cb_size bounds (RISK-3) — do NOT strip either"
- file: /home/dustin/projects/qmk-notifier/notifier.h
  why: "HOST_CALLBACK_MAX 32 (L60), HOST_LAYER_BASE 224 (L63), host_callback_t (L17-21),
        callback_t (L5), DEFINE_HOST_CALLBACKS macro (L69-73), accessor decls (L33-34)"
  pattern: "#define block + struct + weak-default accessor pattern"
  gotcha: constants ALREADY exist — do not re-add or renumber

# MUST READ — the firmware's own (authoritative) PRPs for these exact functions
- file: /home/dustin/projects/qmk-notifier/plan/003_16d737de7a3e/P1M2T1S2/PRP.md
  why: original implementation spec that produced set_host_layer; use to confirm fidelity
  section: "What" (the function body) and "Success Definition"
- file: /home/dustin/projects/qmk-notifier/plan/003_16d737de7a3e/P1M2T1S3/PRP.md
  why: original implementation spec that produced apply_host_callbacks
  section: "What" (Phase 1/Phase 2 + RISK-3 guards) and "Success Definition"

# Reference — existing tests
- file: /home/dustin/projects/qmk-notifier/test_notifier_host.c   # 64-test typed suite; cases v/vi/vii/viii/multi-rep cover these helpers
- file: /home/dustin/projects/qmk-notifier/test_notifier_dispatch.c # 14-test legacy/dispatch regression
- file: /home/dustin/projects/qmk-notifier/test_notifier_os.c       # 31-test multi-OS regression
- file: /home/dustin/projects/qmk-notifier/run_notifier_stub_tests.sh # committed gate
- file: /home/dustin/projects/qmk-notifier/qmk_stubs/              # host-compile stubs (QMK_KEYBOARD_H, raw_hid_send capture, layer state)
```

### Current Codebase tree (firmware repo, verification-relevant only)

```bash
# run from /home/dustin/projects/qmk-notifier
notifier.h            # public API: HOST_CALLBACK_MAX, HOST_LAYER_BASE, host_callback_t, DEFINE_HOST_CALLBACKS
notifier.c            # set_host_layer (L252), apply_host_callbacks (L283), host state (L143-144), AHC caller (L709)
qmk_stubs/            # host-compile stubs (QMK_KEYBOARD_H, raw_hid_send capture, layer_on/off, os_detection)
test_notifier_dispatch.c   # 14-test legacy/dispatch regression suite
test_notifier_os.c         # 31-test multi-OS regression suite
test_notifier_host.c       # 64-test typed-command suite (helper scope = cases v/vi/vii/viii/multi-rep)
run_notifier_stub_tests.sh # committed gate: stub-compile + dispatch + os
run_all_tests.sh           # 9-suite pattern_match corpus (legacy path unaffected)
```

### Desired Codebase tree
**No files added or removed.** If a genuine defect is found (not expected), the fix is a
surgical edit inside `notifier.c` only. Build artifacts (`test_*` binaries already
gitignored) are regenerated by the runners.

### Known Gotchas of our codebase & Library Quirks
```c
// CRITICAL: notifier.c #includes a -D-expanded header name:
//   #include QMK_KEYBOARD_H        // QMK_KEYBOARD_H is a macro expanded by -DQMK_KEYBOARD_H='"...h"'
// It CANNOT compile standalone. ALWAYS use the stub harness:
//   gcc -DQMK_KEYBOARD_H='"qmk_keyboard_stub.h"' -Iqmk_stubs -I.  (see run_notifier_stub_tests.sh)
//
// CRITICAL: the set_host_layer clear branch MUST guard layer_off — calling layer_off(255)
// when host_layer==LAYER_UNSET would erroneously turn off QMK layer 255. The committed code
// guards it: `if (host_layer != LAYER_UNSET) layer_off(host_layer);`. Do NOT "simplify" this.
//
// CRITICAL: apply_host_callbacks MUST keep BOTH bounds checks — id < HOST_CALLBACK_MAX
// (array bounds) AND id < cb_size (registry bounds). When no DEFINE_HOST_CALLBACKS is
// present, get_host_callbacks() returns NULL and cb_size==0; without the cb_size guard,
// dereferencing cbs[id] SIGSEGVs. This is RISK-3 (findings_and_risks.md). Do NOT strip it.
//
// GOTCHA: these helpers are the disable-before-enable mirror of the board
// enable_command/disable_command pair. Phase 1 (disable) runs BEFORE Phase 2 (enable) so a
// callback id moving out of set A and into set B fires on_disable before any on_enable.
//
// GOTCHA: the 4 -Wunused warnings on stub-compile (host_layer, host_cb_enabled,
// has_been_queried, board_rules_present) are PRE-EXISTING and expected — a stub build that
// excludes handler-driving tests under-uses these globals. Do NOT silence them with (void)
// casts; they are not new and not yours to "fix".
```

## Implementation Blueprint

### Verification map — contract point → existing code (NO new data models; C firmware)

| # | Contract point | Existing location | Verified? |
|---|---|---|---|
| 1 | `host_layer` declared `static uint8_t = LAYER_UNSET`; `host_cb_enabled[HOST_CALLBACK_MAX] = {false}` | `notifier.c:143-144`; `LAYER_UNSET 255` L126; `HOST_CALLBACK_MAX 32` notifier.h:60 | ☐ |
| 2 | `LAYER_UNSET=255` and `HOST_LAYER_BASE=224` defined; `layer_on/layer_off` are QMK fns; `activate_layer/deactivate_layer` are board equivalents | `notifier.c:126`; `notifier.h:63`; board pair `notifier.c:218/226` | ☐ |
| 3 | `set_host_layer`: 0xFF ⇒ `layer_off(host_layer)` (guarded) + `host_layer=LAYER_UNSET`; else `layer_off(old)` (if set) then `layer_on(layer)` + `host_layer=layer` | `notifier.c:252-265` | ☐ |
| 4 | `set_host_layer` operates ONLY on host tracker, never touches board `activated_layer` | L252-265 (no `activated_layer` ref) | ☐ |
| 5 | `apply_host_callbacks` Phase 1: for each id in 0..HOST_CALLBACK_MAX enabled-but-not-in-new-set ⇒ guarded `on_disable` + clear flag | `notifier.c:291-302` | ☐ |
| 6 | `apply_host_callbacks` Phase 2: for each new id not already enabled ⇒ guarded `on_enable` + set flag | `notifier.c:307-315` | ☐ |
| 7 | Disable-before-enable ordering (Phase 1 strictly before Phase 2) | L290-306 before L307-315 | ☐ |
| 8 | NULL `on_enable`/`on_disable` guarded | L299, L313 | ☐ |
| 9 | `id >= HOST_CALLBACK_MAX` guarded (Phase 2 skip) | L309 | ☐ |
| 10 | (BEYOND contract, correct) registry `cb_size` bounds guard in BOTH phases (RISK-3) | L298, L310 | ☐ |
| 11 | Sole caller = AHC handler; helpers are `static` file-local | `notifier.c:729-730` (caller); `static` at L252, L283 | ☐ |

### Implementation Tasks (verification-ordered)

```yaml
Task 1: ESTABLISH baseline (no edits)
  - RUN: cd /home/dustin/projects/qmk-notifier && git status -s && git log --oneline -3
  - EXPECT: clean tree (or only the untracked plan/003 P1M3T2S1 dir), HEAD at or past 779152a
  - READ: notifier.c L120-145 (weak accessors + host state), L218-317 (board pair + both helpers),
          L695-734 (AHC handler caller); notifier.h L1-80 (constants + struct + macro)
  - CONFIRM every row of the Verification map above (check the ☐ boxes in your report)
  - NOTE the two documented contract-vs-code deltas (guarded layer_off; cb_size bounds) are
    CORRECT, not defects — record them as "verified correct, kept as-is"

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
    handler / §4.7 OS-change mechanics — OUT OF SCOPE for these helpers.
    Helper-relevant groups that MUST be 100% pass:
      (v)   STACK clear_board=0: board command NOT torn down
      (vi)  REPLACE clear_board=1: board torn down + host layer 224 active
      (vii) callback diff ordering: AHC{[0]} on_enable(id0); AHC{[1]} on_disable(id0) BEFORE
            on_enable(id1) [disable-before-enable ORDER proven via g_seq stamps]
      (viii) AHC{layer=0xFF}: host layer cleared (LAYER_UNSET)
      (multi-rep) two-report AHC: r[0]=0x51, r[1]=0x05, r[2]=ack=1, host layer 224, diff ran
  - SCOPE RULE: do NOT attempt to fix the 7 SET_OS failures here. They are tracked by the
    firmware's plan/003 P1.M3.T2.S1 (Researching) and belong to QMKonnect P1.M2.T2.S2/S3.

Task 4: (DEFAULT) NO-OP — write the verification report, leave source untouched
  - IF Tasks 1-3 are green and the verification map is fully satisfied (expected path):
    the deliverable is the inline report. `git diff` stays empty. Done.
  - IF a genuine helper-level defect is found (unexpected):
    make the MINIMAL surgical fix in notifier.c that preserves the guarded layer_off AND
    both RISK-3 bounds checks AND the disable-before-enable ordering, then re-run Tasks 2 & 3
    to confirm still-green + no new failures. Document the defect, the fix, and the
    before/after test counts in the report.

Task 5: NEVER do these
  - DO NOT rewrite set_host_layer to call layer_off unconditionally in the clear branch —
    that turns off QMK layer 255 when host_layer is unset (regression).
  - DO NOT strip the `id < cb_size` registry-bounds check from apply_host_callbacks — that
    reintroduces a SIGSEGV when no DEFINE_HOST_CALLBACKS is present (RISK-3 regression).
  - DO NOT make either helper touch the board activated_layer / call deactivate_layer /
    disable_command — board clear is the AHC handler's job (replace mode, notifier.c:726-728).
  - DO NOT change either function's signature or remove its `static` qualifier.
  - DO NOT silence the 4 expected -Wunused stub-compile warnings.
  - DO NOT edit PRD.md, any tasks.json, or any plan/ files (read-only).
```

### Implementation Patterns & Key Details
```c
// The existing (correct) set_host_layer — guarded clear, two-branch, host-only:
//   static void set_host_layer(uint8_t layer) {
//       if (layer == LAYER_UNSET) {                 /* (a) clear the host layer */
//           if (host_layer != LAYER_UNSET) {         /* GUARD — avoids layer_off(255) */
//               layer_off(host_layer);
//           }
//           host_layer = LAYER_UNSET;
//       } else {                                    /* (b) real host layer (>= 224) */
//           if (host_layer != LAYER_UNSET) {
//               layer_off(host_layer);              /* turn off the old host layer first */
//           }
//           layer_on(layer);
//           host_layer = layer;
//       }
//   }

// The existing (correct) apply_host_callbacks — disable-before-enable, dual bounds + NULL guards:
//   static void apply_host_callbacks(const uint8_t *ids, uint8_t count) {
//       host_callback_t *cbs     = get_host_callbacks();      /* NULL when no registry (weak) */
//       size_t           cb_size = get_host_callbacks_size(); /* 0 when no registry */
//       /* PHASE 1 — DISABLE: enabled id NOT in new set => on_disable + clear */
//       for (uint8_t id = 0; id < HOST_CALLBACK_MAX; id++) {
//           if (!host_cb_enabled[id]) continue;
//           bool still_desired = false;
//           for (uint8_t i = 0; i < count; i++) if (ids[i] == id) { still_desired = true; break; }
//           if (still_desired) continue;
//           if (id < cb_size && cbs[id].on_disable != NULL) cbs[id].on_disable();  /* RISK-3 + NULL */
//           host_cb_enabled[id] = false;
//       }
//       /* PHASE 2 — ENABLE: new id not already enabled => on_enable + set */
//       for (uint8_t i = 0; i < count; i++) {
//           uint8_t id = ids[i];
//           if (id >= HOST_CALLBACK_MAX) continue;            /* RISK-3: array bounds */
//           if (id >= cb_size) continue;                      /* RISK-3: registry bounds */
//           if (host_cb_enabled[id]) continue;                /* diff: already enabled */
//           if (cbs[id].on_enable != NULL) cbs[id].on_enable();
//           host_cb_enabled[id] = true;
//       }
//   }

// The sole caller — APPLY_HOST_CONTEXT handler (notifier.c:709): the clear_board flag drives
// the BOARD teardown (deactivate_layer + disable_command), THEN these helpers drive the HOST:
//   case NOTIFY_CMD_APPLY_HOST_CONTEXT: {
//       uint8_t layer = data[2], flags = data[3], count = data[4];
//       /* ... clamp count to MSG_BUFFER_SIZE-5 ... */
//       if (flags & 0x01) { deactivate_layer(); disable_command(); }  /* board (replace) */
//       set_host_layer(layer);              /* host: 0xFF clears host_layer */
//       apply_host_callbacks(ids, count);   /* host: disable-before-enable diff */
//       send_typed_response(NOTIFY_CMD_APPLY_HOST_CONTEXT, (uint8_t[]){0x01}, 1);
//   }
```

### Integration Points
```yaml
DEPENDENCIES: none — this is self-contained firmware C; it does NOT depend on the
              qmk_notifier Rust crate or its v0.3.0 tag (P1.M1.T4.S1, parallel).
              It depends on P1.M2.T1.S1 (the typed-dispatch skeleton) only in that the
              skeleton routes the AHC command to the handler that calls these helpers;
              that skeleton ALREADY exists and is verified.
DOWNSTREAM (consumers — ALL ALREADY PRESENT):
  - P1.M2.T2.S4 APPLY_HOST_CONTEXT handler (QMKonnect) = P1.M2.T2.S3 (firmware)
    at notifier.c:709 — the SOLE caller of both helpers.
HOST (desktop) handshake that drives this — P4.M2.T1 (planned): sends APPLY_HOST_CONTEXT
  per window change with layer + callback id set + clear_board flag.
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

### Level 3: Typed-command host suite (helper-scope validation)
```bash
cd /home/dustin/projects/qmk-notifier
gcc -Wall -Wextra -std=c99 -DQMK_KEYBOARD_H='"qmk_keyboard_stub.h"' -Iqmk_stubs -I. \
    -c notifier.c -o /tmp/nh.o
gcc -Wall -std=c99 -Iqmk_stubs -I. /tmp/nh.o qmk_stubs/qmk_stubs.c test_notifier_host.c \
    -o /tmp/test_notifier_host
/tmp/test_notifier_host
# Expected baseline: 64 run / 57 pass / 7 fail. The 7 failures are ALL SET_OS (§4.7) —
# OUT OF SCOPE for these helpers (QMKonnect P1.M2.T2.S2/S3; firmware plan/003 P1.M3.T2.S1).
# Helper-relevant groups that MUST be 100% pass:
#   (v)   STACK clear_board=0: board command NOT torn down
#   (vi)  REPLACE clear_board=1: board torn down + host layer 224 active
#   (vii) callback diff: AHC{[1]} on_disable(id0) seq < on_enable(id1) seq  [disable-before-enable]
#   (viii) AHC{layer=0xFF}: host layer cleared (LAYER_UNSET)
#   (multi-rep) two-report AHC: r[0]=0x51, r[1]=0x05, r[2]=ack=1, host layer 224, diff ran
```

### Level 4: Legacy-path regression (no host helper touches it)
```bash
cd /home/dustin/projects/qmk-notifier
./run_all_tests.sh
# Expected: the 9-suite pattern_match corpus is unaffected (legacy string path is
# byte-identical; these helpers are only reachable via the typed AHC command). All pass.
```

## Final Validation Checklist

### Technical Validation
- [ ] Verification map (11 rows) fully satisfied by existing code.
- [ ] `./run_notifier_stub_tests.sh` → `✓ notifier stub-compile gate PASSED` (dispatch 14/14, os 31/31).
- [ ] `test_notifier_host.c` helper-scope groups (v/vi/vii/viii/multi-rep) 100% pass (7 SET_OS failures explicitly out of scope).
- [ ] `./run_all_tests.sh` — 9-suite pattern corpus unaffected.
- [ ] Stub-compile shows ONLY the 4 pre-existing -Wunused warnings (no new warnings).

### Feature Validation
- [ ] `set_host_layer`: 0xFF guarded-clear; real layer ⇒ `layer_off(old)`→`layer_on(new)`; host-only.
- [ ] `set_host_layer` orthogonality: leaves board `activated_layer` unchanged (test vi shows board torn-down is the handler's clear_board, not the helper).
- [ ] `apply_host_callbacks`: disable-before-enable ORDER proven by `g_seq` stamps (test vii).
- [ ] `apply_host_callbacks` idempotence: re-sending the same id set fires no callbacks (Phase 1/2 diffs both no-op).
- [ ] `apply_host_callbacks` NULL/`count==0`: `{}` disables all currently-enabled ids via Phase 1.
- [ ] Multi-report AHC reassembles and both helpers run (test multi-rep: host layer 224 + diff ran).

### Code Quality Validation
- [ ] `git diff` is EMPTY (expected default) OR a minimal, justified defect-fix with all gates green.
- [ ] No re-implementation of the (regressive) literal contract (unguarded layer_off / stripped cb_size bounds).
- [ ] Neither helper touches board `activated_layer` or calls `deactivate_layer`/`disable_command`.
- [ ] No renumbering of NOTIFY_CMD_* / HOST_CALLBACK_MAX / HOST_LAYER_BASE constants.

### Documentation & Deployment
- [ ] No user-facing docs required (firmware C code — per the work-item DOCS: none).
- [ ] Verification report recorded inline (contract map + test counts + the two documented deltas).

---

## Anti-Patterns to Avoid
- ❌ Do NOT "implement the literal contract" — it omits the `layer_off` clear-guard (would turn off QMK layer 255) and the `cb_size` registry-bounds check (would SIGSEGV with no `DEFINE_HOST_CALLBACKS`). The committed code is correct; align to it.
- ❌ Do NOT rip out `set_host_layer` / `apply_host_callbacks` and re-create them — they exist and are tested.
- ❌ Do NOT make either helper touch board state — board clear is the AHC handler's job (clear_board flag, notifier.c:726-728).
- ❌ Do NOT try to fix the 7 SET_OS handler failures here — out of scope (P1.M2.T2.S2/S3; firmware plan/003 P1.M3.T2.S1).
- ❌ Do NOT silence the 4 expected -Wunused stub-compile warnings.
- ❌ Do NOT edit PRD.md, tasks.json, prd_snapshot.md, or any plan/ file.
- ❌ Do NOT assume this task must produce a diff — the expected, correct outcome is a verification report with an empty diff.

---

## Confidence Score: 9/10

The deliverable is already present, correct, and green at its scope (verified this session:
stub-compile gate PASSED, dispatch 14/14, os 31/31, host suite helper-groups (v/vi/vii/
viii/multi-rep) all pass). The 1-point reservation is for the (unlikely) discovery of a
genuine helper-level defect during the implementation agent's own verification pass; if
found, the minimal-fix path is specified. The dominant risk this PRP neutralizes is a
regression from naively implementing the literal contract text (unguarded `layer_off(255)`
and stripped RISK-3 bounds checks).
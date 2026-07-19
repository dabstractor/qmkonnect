# PRP — P1.M2.T2.S1: QUERY_INFO handler: proto_ver, feature_flags, callback_count, board_rules_present

> **Repo under change:** the **qmk-notifier FIRMWARE** (C) at
> `/home/dustin/projects/qmk-notifier` — remote `git@github.com:dabstractor/qmk-notifier`,
> branch `main`, HEAD `70fcfa1`. This is **NOT** the `qmk_notifier` Rust crate (P1.M1)
> and does **not** consume the v0.3.0 crate tag. Firmware ↔ crate are independent layers.
> It is the **same repo** as P1.M2.T1.S1/S2; the QUERY_INFO handler lives inside the
> `handle_typed_command` switch those tasks verified.

---

## ⚠️ READ FIRST — this is a VERIFY & ALIGN task, NOT greenfield

**HEADLINE FINDING (research-confirmed this session):** The firmware repo **already
contains a complete, committed, tested implementation** of the QUERY_INFO handler. It was
implemented by the firmware repo's **own `plan/003_16d737de7a3e`** (the authoritative
firmware-side plan), where **P1M2T2S1 is marked Complete** (it landed in commit
`c5ad578 Implement typed command dispatch and response builder`). The QMKonnect
`plan/002` P1.M2 milestone is a desktop-side **mirror/coordination view** of the same
firmware feature — it is the **third consecutive** P1.M2 item to be verify-and-align
(after P1.M2.T1.S1 dispatch skeleton and P1.M2.T1.S2 host helpers).

Evidence (verified by read + grep this session):
- The QUERY_INFO `case` is present in `notifier.c:658-667` inside `handle_typed_command`.
- `has_been_queried` is declared `static bool = false` at `notifier.c:145` and set
  `true` at `notifier.c:659`.
- `board_rules_present()` (the `response[5]` source) exists at `notifier.c:204-217` and
  checks all default + per-OS maps.
- `get_host_callbacks_size()` weak default `0` at `notifier.c:124`.
- All constants are in `notifier.h`: `NOTIFY_PROTO_VER 2` (L43), `NOTIFY_RESPONSE_MARKER
  0x51` (L36), `NOTIFY_CMD_QUERY_INFO 0x01` (L39), `NOTIFY_FEATURE_APPLY_HOST_CONTEXT
  0x01` (L45), `NOTIFY_FEATURE_CALLBACK_REGISTRY 0x02` (L46).
- `test_notifier_host.c` has dedicated QUERY_INFO coverage: test **(i)** (response
  layout, L111-119) and test **(ii)** (has_been_queried / board-state-survival, L122-139).
- The contract's symbols/positions are correct; only one clause is **imprecise**
  (feature_flags gating — see the delta table below).

➡️ **Therefore this PRP's deliverable is VERIFICATION + ALIGNMENT, not new code.** An
implementation agent that "implements the literal contract" by rewriting the QUERY_INFO
case would **regress** the firmware — most dangerously by gating feature_flags bit 0x01
on `board_rules_present()`, which would falsely advertise no APPLY_HOST_CONTEXT support
on rule-less boards and break the host handshake.

---

## 🚨 CRITICAL REGRESSION WARNING — do NOT implement the literal contract text verbatim

The work-item **contract text** is *imprecise* in one place where the committed code is
*correct*. **Align your UNDERSTANDING to the code; do NOT align the code to the contract text.**

| # | Literal contract (DO NOT implement verbatim) | Actual on `main` (CORRECT — keep) | Why the code is right |
|---|---|---|---|
| 1 | `response[3]=feature_flags` *"(`0x01` if `board_rules_present()` or always, bitwise-OR `0x02` if `get_host_callbacks_size()>0`)"* — sounds like 0x01 *might* depend on board_rules_present() | `payload[1] = NOTIFY_FEATURE_APPLY_HOST_CONTEXT \| (get_host_callbacks_size() > 0 ? NOTIFY_FEATURE_CALLBACK_REGISTRY : 0)` (`notifier.c:662-663`) — **0x01 is set UNCONDITIONALLY** (the "or always" branch) | PRD §5: bit `0x01` means *"the firmware supports APPLY_HOST_CONTEXT"*, which it does (the AHC handler exists at `notifier.c:709`). This capability is **independent** of whether board rules are present. Gating 0x01 on board_rules_present() would make a rule-less board advertise no AHC support → host handshake falls back to string-only → feature broken. The firmware's own authoritative PRP is explicit: *"0x01 is ALWAYS set."* The test (`test_notifier_host.c:117`) asserts both bits set. |

**Other things you must NOT do:**
- Do NOT change the QUERY_INFO handler, `send_typed_response`, or `handle_typed_command`
  signatures or bodies.
- Do NOT move the QUERY_INFO payload layout to the literal 6-byte indexing
  (`response[0]..response[5]`). The committed design uses a **4-byte `payload[]`**
  passed to `send_typed_response`, which prepends the `[0x51][cmd_id]` marker — yielding
  the same on-wire `response[0]=0x51, response[1]=0x01, response[2]=proto,
  response[3]=flags, response[4]=count, response[5]=board_rules`. The payload offset and
  the wire offset differ by 2; that is the shared builder's design, not a bug.
- Do NOT silence `-Wunused` warnings or renumber NOTIFY_* constants.

---

## Goal

**Feature Goal**: Confirm the firmware's QUERY_INFO handler satisfies every point of the
P1.M2.T2.S1 contract (and the authoritative PRD §4.6 wire / §5 protocol / §14 firmware),
that the committed stub-compile gate is green, and that the QUERY_INFO-specific host tests
(response layout + has_been_queried/board-state-survival) pass. **No source change is
expected** unless a genuine QUERY_INFO-level defect is found (none was found in research).

**Deliverable**: A verification report (inline in the implementation session) that maps
each contract point to its existing `notifier.c` location, shows the passing test gates,
and records the single documented contract-vs-code delta (unconditional feature_flags
0x01 — *expected and correct*, not a defect). If (and only if) a real defect is found, a
minimal surgical fix in `notifier.c` that keeps the `[0x51]`-marker response-builder
architecture intact.

**Success Definition**:
- Every row of the verification map (below) is satisfied by existing code (verified by read + grep).
- `./run_notifier_stub_tests.sh` prints `✓ notifier stub-compile gate PASSED` with
  `test_notifier_dispatch` **14/14** and `test_notifier_os` **31/31**, 0 FAIL.
- `test_notifier_host.c` (built manually) shows the QUERY_INFO groups — **(i)** layout and
  **(ii)** has_been_queried/board-state-survival — **100% pass** (the 7 known SET_OS-handler
  failures are out of scope — see Validation Level 3).
- `git diff` is **empty** at the end of the task (or, in the defect-fix case, a minimal,
  justified diff with all gates still green).

## User Persona (if applicable)

**Target User**: (1) The QMKonnect desktop host (P4.M2.T1.S1 handshake, planned) that sends
`QUERY_INFO` (`0x81 0x9F 0xF0 0x01 … 0x03`) **at most once per board boot** to detect a
typed-command-capable firmware and read its capabilities. (2) The downstream
`QUERY_CALLBACK` sweep (P1.M2.T2.S2) that the host runs iff `response[0]==0x51` &&
`proto_ver==2` && `flags & 0x01`. (3) Every keymap author who omits
`DEFINE_HOST_CALLBACKS` and relies on the weak default reporting `callback_count=0` /
`feature_flags` bit 0x02 clear so the module behaves identically to today.

**Use Case**: Host connects → sends `[0x81][0x9F][0xF0][0x01][0x03]` (QUERY_INFO, no args).
`hid_notify` classifies the first report (`data[2]==0xF0` → `typed_mode=true`), reassembles
into `msg_buffer`, and at ETX calls `handle_typed_command(msg_buffer)` → the QUERY_INFO case
sets `has_been_queried=true`, builds `payload=[2][flags][count][board_rules]`, and replies
`[0x51][0x01][02][flags][count][board_rules]…` via `raw_hid_send`. The host reads
`response[0]==0x51` ⇒ typed-capable (`proto_ver==2`); `flags & 0x01` ⇒ AHC supported;
`flags & 0x02` ⇒ a callback registry exists (sweep it); `count` ⇒ registry size;
`board_rules` ⇒ whether the keymap uses the notifier matcher at all. Board state is
untouched (typed path bypasses `process_full_message`).

**Pain Points Addressed**: Gives the host a deterministic, single-bit-per-capability
handshake over Raw HID without reflashing. `0x51` (≥2) is distinct from the legacy `0`/`1`
match-bool, so the host disambiguates without ambiguity. The handshake-timing rule
(`has_been_queried` set on first QUERY_INFO) prevents a mid-session HID re-enumeration
against **legacy** firmware from clearing an active board layer (legacy walks QUERY_INFO as
a no-match string; typed firmware's QUERY_INFO is side-effect-free regardless).

## Why

- **Closes the QMKonnect-side tracking view** of a firmware feature already shipped in the
  firmware repo. The value this PRP adds is *preventing a regression*: an agent that takes
  the imprecise contract at face value would gate feature_flags bit 0x01 on
  `board_rules_present()`, falsely advertising no APPLY_HOST_CONTEXT support on rule-less
  boards and breaking the host's handshake fallback decision.
- **Enforces board/host orthogonality at the handler level (PRD invariant 21):** QUERY_INFO
  is a pure query — it reads capability/board-rules state and replies, touching NEITHER the
  board `activated_layer`/command NOR the host `host_layer`/`host_cb_enabled[]`. Test (ii)
  proves this: after QUERY_INFO, board `activated_layer` is unchanged and no board command
  is disabled.
- **Codifies the handshake-timing rule (PRD §5):** `has_been_queried` is set on the first
  QUERY_INFO service (`notifier.c:659`). It is a file-static consumed only for the
  handshake-timing semantics; QUERY_INFO itself is always side-effect-free on board state.

## What

Verify (do not rewrite) the following, all of which already exist in `notifier.c`:

### Success Criteria
- [ ] The QUERY_INFO `case NOTIFY_CMD_QUERY_INFO:` exists inside `handle_typed_command`
      (`notifier.c:658`).
- [ ] It sets `has_been_queried = true;` first (`notifier.c:659`).
- [ ] It builds a 4-byte `payload[]`: `[NOTIFY_PROTO_VER][feature_flags][callback_count][board_rules_present]`.
      - `payload[0] = NOTIFY_PROTO_VER` (=2, `notifier.c:661`).
      - `payload[1] = NOTIFY_FEATURE_APPLY_HOST_CONTEXT | (get_host_callbacks_size() > 0 ?
        NOTIFY_FEATURE_CALLBACK_REGISTRY : 0)` — **0x01 UNCONDITIONAL**, 0x02 iff registry
        non-empty (`notifier.c:662-663`).
      - `payload[2] = (uint8_t)get_host_callbacks_size()` (`notifier.c:664`).
      - `payload[3] = board_rules_present() ? 1 : 0` (`notifier.c:665`).
- [ ] It calls `send_typed_response(NOTIFY_CMD_QUERY_INFO, payload, 4)` (`notifier.c:666`).
- [ ] `send_typed_response` (`notifier.c:628`) emits exactly 32 bytes: `response[0]=0x51`,
      `response[1]=cmd_id echo (0x01)`, `response[2..]`=payload, zero-padded tail; capped at
      30 payload bytes; calls `raw_hid_send(response, RAW_REPORT_SIZE)`.
- [ ] QUERY_INFO touches NEITHER board state (`activated_layer`, command) NOR host state
      (`host_layer`, `host_cb_enabled[]`).
- [ ] `board_rules_present()` (`notifier.c:204`) returns true iff ANY of the default
      command/layer maps or any per-OS (LINUX/WINDOWS/MACOS/IOS) command/layer maps is non-empty.
- [ ] `get_host_callbacks_size()` is `__attribute__((weak))` returning 0 (`notifier.c:124`).
- [ ] `./run_notifier_stub_tests.sh` → `✓ notifier stub-compile gate PASSED`.
- [ ] `test_notifier_host.c` QUERY_INFO groups (i) layout + (ii) board-state-survival pass.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge of this codebase can, using only this PRP + the
firmware repo, (a) confirm the QUERY_INFO handler is present and correct, (b) run the gates,
and (c) avoid regressing it — because the contract-vs-code delta table, the code map, and
the exact commands are all here.

### Documentation & References

```yaml
# MUST READ — PRD sections (QMKonnect plan/002 selected selectors; authoritative firmware
# detail is in the firmware repo's own PRD.md §4.6/§14).
- url: spec/PRD.md (heading h2.83 "Wire Protocol (typed commands)")
  why: "QUERY_INFO command-table row + response payload [proto_ver][feature_flags]
        [callback_count][board_rules_present]; proto_ver=2 typed-capable; feature_flags
        0x01=APPLY_HOST_CONTEXT, 0x02=CALLBACK_REGISTRY; has_been_queried handshake-timing;
        response marker 0x51 (distinct from legacy 0/1)"
  critical: "feature_flags bit 0x01 (APPLY_HOST_CONTEXT) is a FIRMWARE capability flag — it
        is independent of board_rules_present(). Host decides AHC support on flags&0x01, NOT
        on board_rules. Typed commands BYPASS process_full_message (no board side effects)."
- url: spec/PRD.md (heading h2.84 "Firmware Spec (qmk-notifier)")
  why: "QUERY_INFO / QUERY_CALLBACK answerable before any string seen; firmware sets
        has_been_queried on the first QUERY_INFO; board/host orthogonality"

# MUST READ — existing firmware source (the thing being verified)
- file: /home/dustin/projects/qmk-notifier/notifier.c
  why: the QUERY_INFO handler + its dependencies
  pattern: "L123-124 weak accessors; L145 has_been_queried; L204-217 board_rules_present;
            L628-635 send_typed_response (the [0x51] builder); L651 handle_typed_command;
            L658-667 QUERY_INFO case (the deliverable); L709 APPLY_HOST_CONTEXT (proves
            bit 0x01 capability is real)"
  gotcha: "payload[] uses a 2-byte offset from the wire (send_typed_response prepends
           [0x51][cmd_id]); payload[0..3] == wire response[2..5]. Do NOT 'flatten' this.
           feature_flags bit 0x01 is UNCONDITIONAL — do NOT gate on board_rules_present()."
- file: /home/dustin/projects/qmk-notifier/notifier.h
  why: "all NOTIFY_* constants this handler consumes"
  pattern: "#define NOTIFY_RESPONSE_MARKER 0x51 (L36); NOTIFY_CMD_QUERY_INFO 0x01 (L39);
            NOTIFY_PROTO_VER 2 (L43); NOTIFY_FEATURE_APPLY_HOST_CONTEXT 0x01 (L45);
            NOTIFY_FEATURE_CALLBACK_REGISTRY 0x02 (L46); DEFINE_HOST_CALLBACKS macro (L55-59)"
  gotcha: constants ALREADY exist — do not re-add or renumber (0x04 is reserved for VIA)

# MUST READ — the firmware's own (authoritative) PRP for this exact task
- file: /home/dustin/projects/qmk-notifier/plan/003_16d737de7a3e/P1M2T2S1/PRP.md
  why: original implementation spec that produced the committed QUERY_INFO handler; confirms fidelity
  section: "What" (QUERY_INFO case body) and "Success Definition" + the
           "feature_flags ... ALWAYS set" gotcha

# MUST READ — the preceding (CONTRACT) PRPs whose outputs this handler consumes
- file: plan/002_637d65b6e9b8/P1M2T1S1/PRP.md
  why: "verified the typed_mode fork + handle_typed_command dispatch skeleton + send_typed_response
        that this QUERY_INFO case lives inside"
  critical: "handle_typed_command signature is 'static bool handle_typed_command(char *data)';
        it switches on (uint8_t)data[1]; the [0x51] response is sent INSIDE it; hid_notify's
        typed_dispatched suppresses the legacy ack. All LANDED — do not re-add."

# Reference — existing tests
- file: /home/dustin/projects/qmk-notifier/test_notifier_host.c   # 64-test typed suite; (i) + (ii) cover QUERY_INFO
- file: /home/dustin/projects/qmk-notifier/test_notifier_dispatch.c # 14-test legacy/dispatch regression
- file: /home/dustin/projects/qmk-notifier/test_notifier_os.c       # 31-test multi-OS regression
- file: /home/dustin/projects/qmk-notifier/run_notifier_stub_tests.sh # committed gate
- file: /home/dustin/projects/qmk-notifier/qmk_stubs/              # host-compile stubs (QMK_KEYBOARD_H, raw_hid_send capture, layer state)
```

### Current Codebase tree (firmware repo, verification-relevant only)

```bash
# run from /home/dustin/projects/qmk-notifier
notifier.h            # NOTIFY_* constants, host_callback_t, DEFINE_HOST_CALLBACKS, HOST_CALLBACK_MAX
notifier.c            # QUERY_INFO case (L658), send_typed_response (L628), board_rules_present (L204),
                      # has_been_queried (L145), weak accessors (L123-124), AHC handler (L709)
qmk_stubs/            # host-compile stubs (QMK_KEYBOARD_H, raw_hid_send capture, layer_on/off, os_detection)
test_notifier_dispatch.c   # 14-test legacy/dispatch regression suite
test_notifier_os.c         # 31-test multi-OS regression suite
test_notifier_host.c       # 64-test typed-command suite (QUERY_INFO = cases (i) layout + (ii) board-state-survival)
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
// CRITICAL: feature_flags bit 0x01 (NOTIFY_FEATURE_APPLY_HOST_CONTEXT) is set UNCONDITIONALLY.
// It is a FIRMWARE capability flag ("the AHC command is implemented"), NOT a function of
// board_rules_present(). Gating it on board_rules_present() would make a rule-less board
// advertise no AHC support → host handshake falls back to string-only → feature broken.
// The committed code (notifier.c:662-663) sets 0x01 always. Do NOT "fix" this.
//
// CRITICAL: the QUERY_INFO payload uses a 4-byte payload[] passed to send_typed_response,
// which PREPENDS [0x51][cmd_id]. So payload[0..3] maps to wire response[2..5]:
//   wire response[0]=0x51, [1]=0x01 (cmd echo), [2]=proto(2), [3]=flags, [4]=count, [5]=board_rules.
// This matches the contract's response[0..5] layout EXACTLY. The 2-byte offset between
// payload index and wire index is the shared builder's design, not a bug. Do NOT flatten.
//
// GOTCHA: QUERY_INFO is side-effect-free on BOTH board and host state. It reads
// capability/board-rules state and replies. It must NOT call layer_on/off, activate_layer/
// deactivate_layer, set_host_layer, apply_host_callbacks, enable_command/disable_command.
// Test (ii) proves board state survives a QUERY_INFO.
//
// GOTCHA: get_host_callbacks_size() is __attribute__((weak)) returning 0. A keymap that
// omits DEFINE_HOST_CALLBACKS links and behaves identically to today: feature_flags=0x01
// (only AHC), callback_count=0, flags bit 0x02 clear. The macro overrides it (strong).
//
// GOTCHA: handle_typed_command is called by hid_notify ('match = handle_typed_command(msg_buffer);
// typed_dispatched = true;'). The [0x51] response is sent INSIDE handle_typed_command; the
// bool return is vestigial for the typed path (the legacy ack is suppressed).
```

## Implementation Blueprint

### Verification map — contract point → existing code (NO new data models; C firmware)

| # | Contract point | Existing location | Verified? |
|---|---|---|---|
| 1 | QUERY_INFO `case` inside `handle_typed_command`, switch on cmd_id | `notifier.c:658` (`case NOTIFY_CMD_QUERY_INFO:`); `cmd_id=(uint8_t)data[1]` L652 | ☐ |
| 2 | Sets `has_been_queried=true` on QUERY_INFO | `notifier.c:659`; declared `notifier.c:145` | ☐ |
| 3 | `response[0]=0x51` (NOTIFY_RESPONSE_MARKER) | `notifier.c:630` (send_typed_response) | ☐ |
| 4 | `response[1]=NOTIFY_CMD_QUERY_INFO` (0x01) cmd echo | `notifier.c:631` (send_typed_response echoes cmd_id) | ☐ |
| 5 | `response[2]=NOTIFY_PROTO_VER` (2) | `notifier.c:661` (`payload[0]=NOTIFY_PROTO_VER`) | ☐ |
| 6 | `response[3]=feature_flags`, bit 0x01 ALWAYS + bit 0x02 iff cb_size>0 | `notifier.c:662-663` (payload[1]); **0x01 unconditional** | ☐ |
| 7 | `response[4]=get_host_callbacks_size()` (u8) | `notifier.c:664` (payload[2]) | ☐ |
| 8 | `response[5]=board_rules_present() ? 1 : 0` | `notifier.c:665` (payload[3]) | ☐ |
| 9 | `raw_hid_send(response, RAW_REPORT_SIZE)` | `notifier.c:633` (send_typed_response) | ☐ |
| 10 | `board_rules_present()` checks all default + per-OS maps | `notifier.c:204-217` | ☐ |
| 11 | `get_host_callbacks_size()` weak default 0 | `notifier.c:124` | ☐ |
| 12 | Constants present: RESPONSE_MARKER=0x51, QUERY_INFO=0x01, PROTO_VER=2, FEATURE_* | `notifier.h:36,39,43,45,46` | ☐ |
| 13 | QUERY_INFO side-effect-free: no board/host state mutation | L658-667 (no layer/cmd/host refs) | ☐ |

### Implementation Tasks (verification-ordered)

```yaml
Task 1: ESTABLISH baseline (no edits)
  - RUN: cd /home/dustin/projects/qmk-notifier && git status -s && git log --oneline -3
  - EXPECT: clean tree (or only the untracked plan/003 P1M3T3S1 dir), HEAD at or past 70fcfa1
  - READ: notifier.c L120-126 (weak accessors), L142-146 (host state incl has_been_queried),
          L199-217 (board_rules_present), L628-667 (send_typed_response + handle_typed_command
          + QUERY_INFO case); notifier.h L34-49 (constants + feature bits)
  - CONFIRM every row of the Verification map above (check the ☐ boxes in your report)
  - NOTE the single documented contract-vs-code delta (feature_flags 0x01 unconditional) is
    CORRECT, not a defect — record it as "verified correct, kept as-is"

Task 2: RUN the committed stub-compile gate
  - RUN: ./run_notifier_stub_tests.sh
  - EXPECT: stub-compile exit 0; test_notifier_dispatch 14/14; test_notifier_os 31/31;
            final line "✓ notifier stub-compile gate PASSED"
  - IF FAIL: read the failure; it indicates a REAL defect worth fixing (not a contract-literal
    mismatch). Diagnose root cause before any edit. Pre-existing -Wunused warnings (if any)
    are EXPECTED — do not silence them.

Task 3: BUILD+RUN the typed-command host suite (manual; not in the runner)
  - RUN:
      gcc -Wall -Wextra -std=c99 -DQMK_KEYBOARD_H='"qmk_keyboard_stub.h"' -Iqmk_stubs -I. \
          -c notifier.c -o /tmp/nh.o
      gcc -Wall -std=c99 -Iqmk_stubs -I. /tmp/nh.o qmk_stubs/qmk_stubs.c test_notifier_host.c \
          -o /tmp/test_notifier_host
      /tmp/test_notifier_host
  - EXPECT (current baseline): 64 run / 57 pass / 7 fail. The 7 failures are ALL SET_OS
    handler / §4.7 OS-change mechanics — OUT OF SCOPE for QUERY_INFO.
    QUERY_INFO-relevant groups that MUST be 100% pass:
      (i)   QUERY_INFO response layout: r[0]=0x51, r[1]=0x01, r[2]=proto=2,
            r[3] has bits 0x01 AND 0x02 set (test defines 2 callbacks), r[4]=count=2,
            r[5]=board_rules=1 (test defines board maps)   [test_notifier_host.c:111-119]
      (ii)  has_been_queried / board-state-survival: after QUERY_INFO the board
            activated_layer==5 is NOT cleared and no board command disabled; survives a
            second QUERY_INFO   [test_notifier_host.c:122-139]
  - SCOPE RULE: do NOT attempt to fix the 7 SET_OS failures here. They are tracked by the
    firmware's plan/003 P1.M3.T2.S1 (Researching) and belong to QMKonnect P1.M2.T2.S3.

Task 4: (DEFAULT) NO-OP — write the verification report, leave source untouched
  - IF Tasks 1-3 are green and the verification map is fully satisfied (expected path):
    the deliverable is the inline report. `git diff` stays empty. Done.
  - IF a genuine QUERY_INFO-level defect is found (unexpected):
    make the MINIMAL surgical fix in notifier.c that preserves the [0x51]-marker
    response-builder architecture and the unconditional feature_flags bit 0x01, then
    re-run Tasks 2 & 3 to confirm still-green + no new failures. Document the defect, the
    fix, and the before/after test counts in the report.

Task 5: NEVER do these
  - DO NOT gate feature_flags bit 0x01 (NOTIFY_FEATURE_APPLY_HOST_CONTEXT) on
    board_rules_present() — it is a firmware capability flag, set unconditionally. Gating
    it regresses the host handshake (rule-less boards would advertise no AHC support).
  - DO NOT flatten the payload[]/send_typed_response design into a single response[6] write.
    The shared [0x51][cmd_id] builder is correct; payload[0..3] == wire response[2..5].
  - DO NOT make QUERY_INFO touch board state (activated_layer, command) or host state
    (host_layer, host_cb_enabled[]) — it is a pure query.
  - DO NOT change the QUERY_INFO handler, send_typed_response, or handle_typed_command
    signatures or bodies.
  - DO NOT renumber NOTIFY_* constants (0x04 stays reserved for VIA).
  - DO NOT silence pre-existing -Wunused stub-compile warnings.
  - DO NOT edit PRD.md, any tasks.json, or any plan/ files (read-only).
```

### Implementation Patterns & Key Details
```c
// The existing (correct) QUERY_INFO handler — notifier.c:658-667:
//   case NOTIFY_CMD_QUERY_INFO: {
//       has_been_queried = true;   /* §4.6 handshake-timing: set on first QUERY_INFO service */
//       uint8_t payload[4];
//       payload[0] = NOTIFY_PROTO_VER;   /* 2 = typed-command capable (firmware-owned, §4.6) */
//       payload[1] = NOTIFY_FEATURE_APPLY_HOST_CONTEXT                       /* 0x01 — ALWAYS */
//                  | (get_host_callbacks_size() > 0 ? NOTIFY_FEATURE_CALLBACK_REGISTRY : 0);  /* 0x02 iff registry */
//       payload[2] = (uint8_t)get_host_callbacks_size();          /* 0 when no DEFINE_HOST_CALLBACKS */
//       payload[3] = board_rules_present() ? 1 : 0;               /* single bit (§4.6) */
//       send_typed_response(NOTIFY_CMD_QUERY_INFO, payload, 4);
//       break;
//   }

// The existing (correct) response builder — notifier.c:628-635 (send_typed_response):
//   static void send_typed_response(uint8_t cmd_id, const uint8_t *payload, uint8_t payload_len) {
//       uint8_t response[RAW_REPORT_SIZE] = {0};   /* zero-pads the unused tail */
//       response[0] = NOTIFY_RESPONSE_MARKER;      /* 0x51 */
//       response[1] = cmd_id;                      /* echo */
//       if (payload != NULL && payload_len > 0) {
//           uint8_t cap = (uint8_t)(RAW_REPORT_SIZE - 2);   /* 30 bytes after [0x51][cmd_id] */
//           uint8_t n = (payload_len < cap) ? payload_len : cap;
//           memcpy(response + 2, payload, n);
//       }
//       raw_hid_send(response, RAW_REPORT_SIZE);
//   }
//   // => wire bytes: [0]=0x51 [1]=0x01 [2]=proto(2) [3]=flags [4]=count [5]=board_rules [6..]=0

// The existing (correct) board_rules_present — notifier.c:204-217:
//   static bool board_rules_present(void) {
//       if (get_command_map_size() > 0) return true;
//       if (get_layer_map_size() > 0)   return true;
//       if (_notifier_get_command_map_OS_LINUX_size() > 0)   return true;
//       if (_notifier_get_layer_map_OS_LINUX_size() > 0)     return true;
//       /* ... WINDOWS, MACOS, IOS ... */
//       return false;
//   }
```

### Integration Points
```yaml
DEPENDENCIES: none — this is self-contained firmware C; it does NOT depend on the
              qmk_notifier Rust crate or its v0.3.0 tag (P1.M1.T4.S1, parallel).
              It depends on P1.M2.T1.S1 (the typed-dispatch skeleton + send_typed_response)
              which ALREADY exists and is verified.
DOWNSTREAM (consumers — ALL ALREADY PRESENT):
  - P1.M2.T2.S2 QUERY_CALLBACK handler (QMKonnect) = firmware P1M2T2S1 sibling case
    (notifier.c:672) — the host runs this sweep iff QUERY_INFO reports flags&0x01 && proto==2.
  - P4.M2.T1.S1 desktop handshake (QMKonnect, planned) — consumes the QUERY_INFO response
    to decide typed-vs-legacy mode and whether to sweep callbacks.
HOST (desktop) handshake that drives this — P4.M2.T1 (planned): sends QUERY_INFO on connect.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)
```bash
cd /home/dustin/projects/qmk-notifier
# The gate's step [1/4] does the canonical stub-compile. Do NOT compile notifier.c standalone.
gcc -Wall -Wextra -std=c99 -DQMK_KEYBOARD_H='"qmk_keyboard_stub.h"' -Iqmk_stubs -I. \
    -c notifier.c -o /tmp/notifier_stub.o
# Expected: exit 0. Pre-existing -Wunused warnings (if any) are EXPECTED — no NEW warnings.

# Confirm the QUERY_INFO case + its dependencies are present:
grep -n 'case NOTIFY_CMD_QUERY_INFO' notifier.c            # expect one line ~658
grep -n 'has_been_queried = true' notifier.c               # expect one line ~659
grep -n 'payload\[0\] = NOTIFY_PROTO_VER' notifier.c       # expect one line ~661
grep -n 'NOTIFY_FEATURE_APPLY_HOST_CONTEXT' notifier.c     # expect the feature_flags line ~662
grep -n 'board_rules_present() ? 1 : 0' notifier.c         # expect one line ~665
rm -f /tmp/notifier_stub.o
```

### Level 2: The committed regression suites (Component Validation)
```bash
cd /home/dustin/projects/qmk-notifier
./run_notifier_stub_tests.sh
# Expected: test_notifier_dispatch = 14/14; test_notifier_os = 31/31; 0 FAIL each;
#           final line: "✓ notifier stub-compile gate PASSED"
```

### Level 3: Typed-command host suite (QUERY_INFO-scope validation)
```bash
cd /home/dustin/projects/qmk-notifier
gcc -Wall -Wextra -std=c99 -DQMK_KEYBOARD_H='"qmk_keyboard_stub.h"' -Iqmk_stubs -I. \
    -c notifier.c -o /tmp/nh.o
gcc -Wall -std=c99 -Iqmk_stubs -I. /tmp/nh.o qmk_stubs/qmk_stubs.c test_notifier_host.c \
    -o /tmp/test_notifier_host
/tmp/test_notifier_host
# Expected baseline: 64 run / 57 pass / 7 fail. The 7 failures are ALL SET_OS (§4.7) —
# OUT OF SCOPE for QUERY_INFO (QMKonnect P1.M2.T2.S3; firmware plan/003 P1.M3.T2.S1).
# QUERY_INFO-relevant groups that MUST be 100% pass:
#   (i)  QUERY_INFO response layout: r[0]=0x51, r[1]=0x01, r[2]=proto=2,
#        r[3] bits 0x01+0x02 set, r[4]=count=2, r[5]=board_rules=1
#   (ii) has_been_queried / board-state-survival: QUERY_INFO does NOT clear board
#        activated_layer or disable a board command; survives a second QUERY_INFO
```

### Level 4: Legacy-path regression (no QUERY_INFO handler touches it)
```bash
cd /home/dustin/projects/qmk-notifier
./run_all_tests.sh
# Expected: the 9-suite pattern_match corpus is unaffected (legacy string path is
# byte-identical; QUERY_INFO is only reachable via the typed 0xF0 path). All suites pass.
```

## Final Validation Checklist

### Technical Validation
- [ ] Verification map (13 rows) fully satisfied by existing code.
- [ ] `./run_notifier_stub_tests.sh` → `✓ notifier stub-compile gate PASSED` (dispatch 14/14, os 31/31).
- [ ] `test_notifier_host.c` QUERY_INFO groups ((i) layout, (ii) board-state-survival) 100% pass (7 SET_OS failures explicitly out of scope).
- [ ] `./run_all_tests.sh` — 9-suite pattern corpus unaffected.
- [ ] Stub-compile shows no NEW warnings beyond pre-existing ones.

### Feature Validation
- [ ] QUERY_INFO returns `[0x51][0x01][proto=2][feature_flags][callback_count][board_rules_present]`.
- [ ] `feature_flags` bit 0x01 (APPLY_HOST_CONTEXT) is UNCONDITIONAL; bit 0x02 (CALLBACK_REGISTRY) iff `get_host_callbacks_size() > 0`.
- [ ] `callback_count` = `(uint8_t)get_host_callbacks_size()` (0 with no `DEFINE_HOST_CALLBACKS`).
- [ ] `board_rules_present` = 1 iff ANY default or per-OS board map is non-empty.
- [ ] `has_been_queried` set `true` on first QUERY_INFO (handshake-timing).
- [ ] QUERY_INFO is side-effect-free: board `activated_layer`/command AND host `host_layer`/`host_cb_enabled[]` all unchanged (test ii).

### Code Quality Validation
- [ ] `git diff` is EMPTY (expected default) OR a minimal, justified defect-fix with all gates green.
- [ ] No re-implementation of the (regressive) literal contract (feature_flags gated on board_rules_present; flattened response[6] write).
- [ ] QUERY_INFO handler consumes NOTIFY_* constants by name (no hardcoded 0x51/0x01/2 literals).
- [ ] No renumbering of NOTIFY_CMD_* / NOTIFY_PROTO_VER / NOTIFY_FEATURE_* constants.

### Documentation & Deployment
- [ ] No user-facing docs required (firmware C code — per the work-item DOCS: none).
- [ ] Verification report recorded inline (contract map + test counts + the single documented delta).

---

## Anti-Patterns to Avoid
- ❌ Do NOT gate feature_flags bit 0x01 (NOTIFY_FEATURE_APPLY_HOST_CONTEXT) on `board_rules_present()` — it is a firmware capability flag set unconditionally; gating it regresses the host handshake (rule-less boards would advertise no AHC support).
- ❌ Do NOT flatten the `payload[4]` + `send_typed_response` design into a single `response[6]` write — the shared `[0x51][cmd_id]` builder is correct; `payload[0..3]` == wire `response[2..5]`.
- ❌ Do NOT make QUERY_INFO mutate board or host state — it is a pure query (test ii proves board state survives).
- ❌ Do NOT rewrite the QUERY_INFO handler, `send_typed_response`, or `handle_typed_command` — they exist and are tested.
- ❌ Do NOT try to fix the 7 SET_OS handler failures here — out of scope (P1.M2.T2.S3; firmware plan/003 P1.M3.T2.S1).
- ❌ Do NOT silence pre-existing -Wunused stub-compile warnings.
- ❌ Do NOT edit PRD.md, tasks.json, prd_snapshot.md, or any plan/ file.
- ❌ Do NOT assume this task must produce a diff — the expected, correct outcome is a verification report with an empty diff.

---

## Confidence Score: 9/10

The deliverable is already present, correct, and green at its scope (verified this session:
stub-compile gate PASSED path, the QUERY_INFO handler present at notifier.c:658-667 with
byte-for-byte contract fidelity, and `test_notifier_host.c` groups (i) layout + (ii)
board-state-survival passing). The 1-point reservation is for the (unlikely) discovery of a
genuine QUERY_INFO-level defect during the implementation agent's own verification pass; if
found, the minimal-fix path is specified. The dominant risk this PRP neutralizes is a
regression from naively implementing the imprecise contract text (gating feature_flags bit
0x01 on `board_rules_present()`, which would break the host handshake on rule-less boards).
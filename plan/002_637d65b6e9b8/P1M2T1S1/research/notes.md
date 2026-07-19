# Research Notes — P1.M2.T1.S1

**Item**: Add 0xF0 discriminator check and `handle_typed_command()` skeleton in `hid_notify()`
**Firmware repo target**: `/home/dustin/projects/qmk-notifier` (remote `git@github.com:dabstractor/qmk-notifier`, branch `main`)
**This is C firmware**, unrelated to the `qmk_notifier` Rust crate (P1.M1.T4.S1) being tagged in parallel — P1.M2 does NOT consume the crate tag.

---

## 🔑 HEADLINE FINDING — the work is ALREADY DONE in the firmware repo

The firmware repo has its **OWN plan/003** (`plan/003_16d737de7a3e/`) that has **already
implemented and COMPLETED** this exact feature, plus the entire P1.M2 milestone and the
P1.M3.T1 host test suite. It is committed to `main`. Relevant commits:

```
7a36675 Add SET_OS and APPLY_HOST_CONTEXT test cases
11a698f Implement host test suite for typed command queries
991477b Add stub accessor for HID response capture
779152a Implement APPLY_HOST_CONTEXT typed command handler
ab7055f Implement host-authoritative SET_OS command
c5ad578 Implement typed command dispatch and response builder
6f736df Ignore .pi agent state directory
```

Firmware plan/003 status (authoritative for the firmware):
```
[Complete] P1.M2.T1.S1: Add typed_mode flag + 0xF0 discriminator routing fork in hid_notify
[Complete] P1.M2.T1.S2: Implement set_host_layer (host layer tracker)
[Complete] P1.M2.T1.S3: Implement apply_host_callbacks (disable-before-enable diff)
[Complete] P1.M2.T2.S1: typed response builder + handle_typed_command dispatch + QUERY_INFO/CALLBACK
[Complete] P1.M2.T2.S2: SET_OS handler
[Complete] P1.M2.T2.S3: APPLY_HOST_CONTEXT handler
[Complete] P1.M3.T1.S1-S4: host test suite (test_notifier_host.c)
[Researching] P1.M3.T2.S1: wire test_notifier_host into the runner + verify §11.2 gate
```

The QMKonnect plan/002 P1.M2 milestone is a **desktop-side mirror/coordination view** of the
SAME firmware feature. Its line-number references are STALE (contract says hid_notify
543-577, host state 137-139; actual host state is at notifier.c:143-145 and hid_notify is
~748-825). This confirms the work-item text was authored against a PRE-implementation snapshot.

➡️ **The correct PRP is a VERIFY & ALIGN PRP, NOT a greenfield "create the skeleton" PRP.**

---

## Contract vs actual implementation — structural divergence (CRITICAL)

The literal contract text describes a SIMPLIFIED, **single-report** approach. The ACTUAL
implementation uses the **multi-report** approach mandated by PRD §5 (h2.83:
"`[0x81][0x9F][0xF0][cmd_id][args…][0x03]`, ETX-framed and multi-report … APPLY_HOST_CONTEXT
may span reports"). Implementing the literal contract would **REGRESS** the firmware.

| Contract text | Actual implementation (notifier.c) | Why actual is correct |
|---|---|---|
| after `data += 2; length -= 2;`, check `data[0]==0xF0` | `if (msg_index==0 && length>=3 && data[2]==NOTIFY_CMD_DISCRIMINATOR) typed_mode=true;` BEFORE strip (L760) | Must check `data[2]` on the FIRST report only — continuation reports carry payload at that offset (could coincidentally be 0xF0). Checking "after strip, per-report" cannot classify a multi-report command. |
| `handle_typed_command(data+1, length-1)` immediately, single report | reassemble bytes into `msg_buffer` over reports; at ETX with `typed_mode`: `match = handle_typed_command(msg_buffer)` (L785) | PRD §5 mandates multi-report framing for typed commands. |
| `handle_typed_command(uint8_t *data, uint8_t length)`, switch on `data[0]` (cmd_id) | `handle_typed_command(char *data)`, switch on `data[1]` (cmd_id); `data[0]` is the 0xF0 discriminator (L651) | Whole reassembled buffer is passed: `[0]=0xF0 [1]=cmd_id [2..]=args`. Returns `bool` to suppress the legacy 0/1 ack. |
| `has_been_queried=true` on first QUERY_INFO | ✅ present, L659 | satisfied |
| `return;` after typed dispatch | `typed_dispatched=true`; post-loop `if(!typed_dispatched) raw_hid_send(legacy ack)` (L785, L818) | suppresses legacy ack correctly |

---

## Existing code locations (notifier.c) — for the verification map

- `RAW_REPORT_SIZE 32` (L42), `MSG_BUFFER_SIZE 256` (L79), `msg_buffer[]` (L81), `dropping` (L90)
- `typed_mode` static bool (L96), reset at ETX (L790) and overflow (L799) — RISK-1 fix
- `LAYER_UNSET 255` (L126); host state globals: `host_layer` (L143), `host_cb_enabled[]` (L144), `has_been_queried` (L145)
- `board_rules_present()` (L204), `set_host_layer()` (L252), `apply_host_callbacks()` (L283)
- `send_typed_response()` (L628), `handle_typed_command()` (L651) with cases QUERY_INFO(658)/QUERY_CALLBACK(672)/SET_OS(693)/APPLY_HOST_CONTEXT(709)/default(741)
- `hid_notify()` (L748): coexistence guard → discriminator check (L760) → strip → byte loop → ETX dispatch (L780) → legacy ack suppression (L818)

notifier.h constants (all present, already defined):
`NOTIFY_CMD_DISCRIMINATOR 0xF0`, `NOTIFY_RESPONSE_MARKER 0x51`, `NOTIFY_CMD_QUERY_INFO 0x01`,
`NOTIFY_CMD_QUERY_CALLBACK 0x02`, `NOTIFY_CMD_SET_OS 0x03`, `NOTIFY_CMD_APPLY_HOST_CONTEXT 0x05`,
`NOTIFY_PROTO_VER 2`, `NOTIFY_FEATURE_*`, `HOST_CALLBACK_MAX 32`, `HOST_LAYER_BASE 224`,
`host_callback_t`, `DEFINE_HOST_CALLBACKS`, `get_host_callbacks`/`_size`.

---

## Test gates — current status (verified this session)

**`./run_notifier_stub_tests.sh`** — GREEN (this is the committed P2 stub-compile gate):
- stub-compile `notifier.c` `-Wall -Wextra -std=c99` → exit 0 (only 4 pre-existing `-Wunused` warnings: host_layer, host_cb_enabled, has_been_queried, board_rules_present — these are consumed by the handlers, benign until used)
- `test_notifier_dispatch`: **14/14 pass** (includes hid_notify reassembly+dispatch, ordering, embedded-NUL)
- `test_notifier_os`: **31/31 pass** (multi-OS selection, OS-change-clear F9)
- prints `✓ notifier stub-compile gate PASSED`

**`test_notifier_host.c`** (typed-command suite; built manually same as stub harness — NOT yet
wired into the runner per firmware plan/003 P1.M3.T2.S1 = Researching):
- 64 tests, **57 pass / 7 fail**
- The 7 failures are ALL in the **SET_OS handler / OS-change mechanics (§4.7)**:
  `(i) SET_OS r[1]/r[2]`, `(ii) post-SET_OS OS_MACOS cmd/layer`, `(iii) SET_OS change on_disable/deactivate`, `(iv) SET_OS idempotent`
- ➡️ **OUT OF SCOPE for P1.M2.T1.S1** (the skeleton). SET_OS handler internals are P1.M2.T2.S2/S3;
  the failing gate is tracked by firmware plan/003 P1.M3.T2.S1 (Researching).
- All **skeleton-relevant** tests PASS: discriminator routing, QUERY_INFO response layout,
  has_been_queried consequence, QUERY_CALLBACK valid+out-of-range, APPLY_HOST_CONTEXT clear_board
  (stack/replace), **multi-report two-report AHC reassembly**, legacy coexistence (firefox/neovide).

**`./run_all_tests.sh`** — 9-suite pattern_match corpus (unaffected by typed commands; legacy path byte-identical).

Build command for the host suite (manual, until runner is extended):
```bash
gcc -Wall -Wextra -std=c99 -DQMK_KEYBOARD_H='"qmk_keyboard_stub.h"' -Iqmk_stubs -I. -c notifier.c -o /tmp/nh.o
gcc -Wall -std=c99 -Iqmk_stubs -I. /tmp/nh.o qmk_stubs/qmk_stubs.c test_notifier_host.c -o /tmp/test_notifier_host
/tmp/test_notifier_host
```

---

## Verdict

P1.M2.T1.S1's deliverable (0xF0 discriminator check + `handle_typed_command()` dispatch
skeleton) is **already present, correct, and verified green** at its scope. The PRP must:
1. Confirm the implementation satisfies each contract point (it does).
2. Confirm the stub-compile gate is green (it is).
3. Explicitly WARN against "aligning to the literal contract" (regression risk).
4. Note the 7 SET_OS handler failures are out of scope (tracked elsewhere).
5. Make NO source change unless a genuine skeleton-level defect is found (none found).
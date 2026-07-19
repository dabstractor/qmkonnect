# State Assessment — P1.M2.T2.S4 (APPLY_HOST_CONTEXT handler)

**Date:** research session for QMKonnect plan/002 P1.M2.T2.S4
**Firmware repo:** `/home/dustin/projects/qmk-notifier`, branch `main`, HEAD **`8441af2`** ("Implement length-aware typed command reassembly"). Working tree **clean**.

## Headline

This is the **sixth consecutive** P1.M2 item (after T1.S1 dispatch, T1.S2 host helpers,
T2.S1 QUERY_INFO, T2.S2 QUERY_CALLBACK, T2.S3 SET_OS) to be **verify-and-align**: the
firmware repo **already contains a complete, committed, tested implementation** of the
APPLY_HOST_CONTEXT (`0x05`) handler AND the multi-report framing reassembly (the contract's
REVISE point) that makes it reachable. It landed across commits `c5ad578` (AHC handler) and
`8441af2` (length-aware typed reassembly, the BUG-1/BUG-2/BUG-3 fix).

The QMKonnect `plan/002` P1.M2 milestone is a desktop-side **mirror/coordination view** of
the firmware repo's own `plan/003_16d737de7a3e`. The firmware's authoritative PRP for this
exact handler is at **`plan/003_16d737de7a3e/P1M2T2S3/PRP.md`** (the firmware repo uses a
slightly finer split: AHC handler = P1.M2.T2.S3 there; host helpers = P1.M2.T1.S2 +
P1.M2.T1.S3). The QMKonnect P1.M2.T2.S4 item maps to the firmware P1.M2.T2.S3.

## Code map (HEAD `8441af2`, verified by read + grep this session)

### The AHC handler — `notifier.c:767-802`
```c
case NOTIFY_CMD_APPLY_HOST_CONTEXT: {
    if (len < 5) {                                   /* BUG-3: disc + cmd + layer + flags + count */
        send_typed_response(cmd_id, NULL, 0);
        break;
    }
    uint8_t layer = (uint8_t)data[2];
    uint8_t flags = (uint8_t)data[3];
    uint8_t count = (uint8_t)data[4];
    uint8_t max_ids = (uint8_t)(MSG_BUFFER_SIZE - 5);
    if (max_ids > (uint8_t)(len - 5)) max_ids = (uint8_t)(len - 5);   /* clamp to reassembled len */
    if (count > max_ids) count = max_ids;
    uint8_t *ids = (uint8_t *)&data[5];

    if (flags & 0x01) {                 /* bit 0 = clear_board (§4.6): replace */
        deactivate_layer();             /* board: turn off activated_layer   */
        disable_command();              /* board: turn off current_command    */
    }
    set_host_layer(layer);              /* host: 0xFF (LAYER_UNSET) clears host_layer */
    apply_host_callbacks(ids, count);   /* host: disable-before-enable diff            */

    uint8_t payload[1] = { 0x01 };      /* ack = 1 (applied) */
    send_typed_response(NOTIFY_CMD_APPLY_HOST_CONTEXT, payload, 1);
    break;
}
```

### Dependencies (all present, all verified)
| Symbol | Location | Notes |
|---|---|---|
| `set_host_layer(uint8_t)` | `notifier.c:293-306` | host-only tracker; guarded clear; P1.M2.T1.S2 |
| `apply_host_callbacks(const uint8_t*, uint8_t)` | `notifier.c:324-358` | disable-before-enable; RISK-3 bounds; P1.M2.T1.S2 |
| `deactivate_layer(void)` | `notifier.c:267-276` | board; guarded (`activated_layer==LAYER_UNSET` no-op) |
| `disable_command(void)` | `notifier.c:373-377` | board; NULL-guards `on_disable`; sets `current_command=NULL` |
| `send_typed_response(...)` | `notifier.c:669-680` | `[0x51][cmd_id][payload]` zero-padded to 32; caps payload at 30 |
| `handle_typed_command(char*, uint16_t)` | `notifier.c:693` | head; `cmd_id=(uint8_t)data[1]` L694 |
| `host_layer` / `host_cb_enabled[]` | `notifier.c:184-185` | host state planes |
| `LAYER_UNSET 255` | `notifier.c:167` | used by 0xFF-clear in `set_host_layer` |
| `typed_mode` | `notifier.c:96` | first-report 0xF0 classification flag |
| `typed_literal_remaining` | `notifier.c:115` | length-aware reassembly counter (BUG-1/2/3 fix) |
| `typed_fixed_arg_bytes(cmd_id)` | `notifier.c:129-137` | **AHC→3** (layer/flags/count fixed) |
| `MSG_BUFFER_SIZE 256` | `notifier.c:79` | ids clamp bound (max 251 ids) |
| `RAW_REPORT_SIZE 32` | `notifier.c:42` | response size |
| `NOTIFY_CMD_APPLY_HOST_CONTEXT 0x05` | `notifier.h:51` | |
| `NOTIFY_RESPONSE_MARKER 0x51` | `notifier.h:46` | |
| `HOST_CALLBACK_MAX 32` / `HOST_LAYER_BASE 224` | `notifier.h:60,63` | |

### The framing/REVISE-point mechanism — `hid_notify()` (~`notifier.c:816-947`)
The contract's REVISE ("0xF0 check should happen AFTER ETX reassembly, not per-report") is
**already satisfied semantically** by the architecture (see `revise_point_analysis.md` for
why the *literal* "move the check after ETX" prescription is wrong and the *implemented*
`typed_mode`-first-report + length-aware-reassembly design is correct):
- `notifier.c:835-837`: first report, `data[2]==0xF0` → `typed_mode=true`, seed
  `typed_literal_remaining=2` (consume disc + cmd_id literally).
- `notifier.c:840`: magic strip `data += 2; length -= 2`.
- `notifier.c:858`: `typed_literal = (typed_mode && typed_literal_remaining > 0)`.
- `notifier.c:862`: gated ETX `if (c == ETX_TERMINATOR[0] && !typed_literal)` — only
  terminates when args are fully consumed (this is what makes AHC args of value 0x03, and
  the cmd_id 0x03 of SET_OS, dispatch — BUG-1/BUG-2).
- `notifier.c:868`: at ETX with `typed_mode` → `handle_typed_command(msg_buffer, msg_index)`
  runs on the **FULL reassembled buffer** (this is the "AFTER ETX reassembly" semantics).
- `notifier.c:918-926`: the AHC **variable-length ids tail** accounting — at `msg_index==5`
  (fixed header complete), read `count = msg_buffer[4]` and add `count` more literal bytes
  (clamped to `MSG_BUFFER_SIZE-1` room). This is what **removes the ≤26-callbacks-per-report
  cap** and makes AHC span reports.
- `notifier.c:889-890` / `934-935`: reset `typed_mode` + `typed_literal_remaining` at every
  ETX boundary and on overflow.

## Test evidence (run this session, HEAD `8441af2`, clean tree)

### Committed gate — `./run_notifier_stub_tests.sh`
```
notifier dispatch fails=0  (exit=0)
notifier os fails=0        (exit=0)
notifier host fails=0      (exit=0)
✓ notifier stub-compile gate PASSED
```

### Stub-compile (Level 1)
`gcc -Wall -Wextra -std=c99 -DQMK_KEYBOARD_H='"qmk_keyboard_stub.h"' -Iqmk_stubs -I. -c notifier.c -o /tmp/nh.o`
→ **exit 0, ZERO warnings** (the prior carried `-Wunused-*` set is gone at this HEAD).

### Host suite — `test_notifier_host.c` (64 run / 64 pass / 0 fail)
AHC-relevant families — ALL PASS:
- **(v) STACK** (`clear_board=0`): host layer 224 active; board command NOT torn down.
- **(vi) REPLACE** (`clear_board=1`): board command torn down; host layer 224 active.
- **(vii) DIFF ORDERING**: `AHC{[0]}` enables id 0; `AHC{[1]}` fires `on_disable(id0)`
  (seq 1) BEFORE `on_enable(id1)` (seq 2) — disable-before-enable proven via `g_seq` stamps.
- **(viii) CLEAR**: `AHC{layer=0xFF}` clears host layer → `LAYER_UNSET` (255).
- **(multi-rep)**: count=28 ids across two reports; reassembles; `r[0]=0x51, r[1]=0x05,
  r[2]=ack=1`; host layer 224 active; callback diff ran (id 0 enabled once). This is the
  load-bearing **multi-report reassembly + ≤26-cap-removal** proof.
- **(coexist-i/ii)**: legacy strings (`firefox`/`neovide`) NOT routed to typed; non-magic
  report discarded. (Confirms AHC is only reachable via the typed 0xF0 path.)

## Conclusion

The deliverable is **VERIFICATION + ALIGNMENT, not new code.** An implementation agent that
"implements the literal contract" by (a) moving the 0xF0 check to after-ETX (breaking the
length-aware reassembly), (b) stripping the count clamp, or (c) rewriting the handler would
**regress** the firmware. The PRP's job is to prevent those regressions. Expected outcome:
empty `git diff`, all gates green, an inline verification report.
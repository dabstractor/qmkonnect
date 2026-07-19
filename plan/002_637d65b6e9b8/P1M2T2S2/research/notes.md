# Research Notes — P1.M2.T2.S2 (QUERY_CALLBACK handler)

## TL;DR
**This is a VERIFY-AND-ALIGN task, not greenfield.** The QUERY_CALLBACK handler is
**already implemented, committed, and tested** in the firmware repo
`/home/dustin/projects/qmk-notifier` at `notifier.c:672-687`. It landed in the
firmware's **own** `plan/003_16d737de7a3e` as part of **P1M2T2S1** there (the firmware
plan bundles "dispatch skeleton + QUERY_INFO + QUERY_CALLBACK + default" into ONE task;
the QMKonnect `plan/002` splits them finer). This is the **fourth consecutive**
P1.M2 item to be verify-and-align (after T1.S1 dispatch, T1.S2 host helpers, T2.S1
QUERY_INFO).

## Repo under change
`/home/dustin/projects/qmk-notifier` (firmware C). Remote
`git@github.com:dabstractor/qmk-notifier`, branch `main`, HEAD `70fcfa1`
"Integrate host tests into acceptance runner". `git status` clean. **NOT** the
`qmk_notifier` Rust crate (P1.M1).

## The handler (verbatim, notifier.c:672-687)
```c
/* QUERY_CALLBACK (0x02) — name discovery (§4.6). args[0]=index. The host
 * sweeps i in 0..count to build name->id. Reply: [index][name bytes, NUL-
 * padded] for a valid index; [index][0x00] (name absent) for out-of-range. */
case NOTIFY_CMD_QUERY_CALLBACK: {
    uint8_t index = (uint8_t)data[2];
    size_t cb_size = get_host_callbacks_size();
    host_callback_t *cbs = get_host_callbacks();
    if (cbs != NULL && index < cb_size && cbs[index].name != NULL) {
        uint8_t payload[30];               /* [index] + up to 29 name bytes */
        payload[0] = index;
        const char *name = cbs[index].name;
        uint8_t n = 0;
        while (n < 29 && name[n] != '\0') { payload[1 + n] = (uint8_t)name[n]; n++; }
        send_typed_response(NOTIFY_CMD_QUERY_CALLBACK, payload, (uint8_t)(1 + n));
    } else {
        uint8_t payload[2] = { index, 0x00 };   /* name absent (§4.6) */
        send_typed_response(NOTIFY_CMD_QUERY_CALLBACK, payload, 2);
    }
    break;
}
```

## Contract-vs-code DELTA (code is CORRECT — do not "fix")
The work-item contract says *"read the index from data[1]"*. The code reads
`index = (uint8_t)data[2]`. **The code is right; the contract text is imprecise.**

Why: `hid_notify` does `data += 2; length -= 2;` to strip the `[0x81][0x9F]` magic
header, THEN appends remaining bytes to `msg_buffer`. So the buffer layout inside
`handle_typed_command` (documented at notifier.c:640-642) is:
- `data[0] = 0xF0` (discriminator, originally report byte 2)
- `data[1] = cmd_id` (originally report byte 3)
- `data[2] = first arg` (the index, originally report byte 4)

So cmd_id is read from `data[1]` (consistent with QUERY_INFO) and the index arg is at
`data[2]`. Switching the code to read `data[1]` for the index would **break** it
(cmd_id byte would be read as the index). Align understanding to the code, not the
contract text. (Same pattern as P1M2T2S1's feature_flags delta.)

## Other notes (no defects)
- The code is **MORE defensive** than the literal contract: it also checks `cbs != NULL`
  (weak default) and `cbs[index].name != NULL`. Strictly safer; no behavior change for
  any contract case.
- Name is capped at **29 bytes** (`payload[30]` = 1 index + 29 name;
  `send_typed_response` caps payload at `RAW_REPORT_SIZE-2 = 30`). The contract's
  "NUL-padded to fill 32 bytes" refers to the TOTAL 32-byte response; the name occupies
  `response[3..31]` (29 bytes). Correct — no delta.
- `response[0]=0x51, response[1]=0x02, response[2]=index, response[3..]=name` is
  produced by `send_typed_response(NOTIFY_CMD_QUERY_CALLBACK, payload, len)` which
  prepends `[0x51][cmd_id]` and zero-pads to 32. Confirmed by tests.

## Symbols (all pre-existing, verified by read + grep)
- `host_callback_t { const char *name; callback_t on_enable; callback_t on_disable; }`
  — notifier.h:16-21.
- `callback_t = void (*)(void)` — notifier.h:5.
- `get_host_callbacks()` weak → NULL — notifier.c:123; strong via DEFINE_HOST_CALLBACKS
  — notifier.h:70-72.
- `get_host_callbacks_size()` weak → 0 — notifier.c:124; strong — notifier.h:73.
- `NOTIFY_CMD_QUERY_CALLBACK 0x02` — notifier.h:49.
- `NOTIFY_RESPONSE_MARKER 0x51` — notifier.h:46.
- `RAW_REPORT_SIZE 32` — notifier.c:42.
- `send_typed_response(cmd_id, payload, len)` — notifier.c:628-637.
- `handle_typed_command(char *data)` switches on `(uint8_t)data[1]` — notifier.c:651.

## Test evidence (run this session)
Built + ran `test_notifier_host.c` (the typed-command suite):
- Group **(iii)** QUERY_CALLBACK valid index 0 and 1: `r[0]=0x51`, `r[1]=0x02`,
  `r[2]=index`, `r[3..]='mute'`/`'layout'`, NUL-padded → **ALL PASS**
  (test_notifier_host.c:142-158).
- Group **(iv)** QUERY_CALLBACK out-of-range index 2: `r[0]=0x51`, `r[1]=0x02`,
  `r[2]=2`, `r[3]=0x00` → **ALL PASS** (test_notifier_host.c:161-169).
- Side-effect-free assertion "QUERY_INFO/QUERY_CALLBACK fired no host callback
  (read-only queries)" → **PASS** (test_notifier_host.c:175).
- Suite baseline: **64 run / 57 pass / 7 fail**. The 7 failures are ALL SET_OS
  handler (the documented `0x03==ETX` blocker) — **OUT OF SCOPE** for QUERY_CALLBACK.

## Gate status (ACCURATE — differs from P1M2T2S1 PRP's assumption)
`run_notifier_stub_tests.sh` was updated by HEAD `70fcfa1` to integrate the host suite.
It now runs dispatch + os + **host**, and PASSES only if all three have 0 fails.
Current reality:
- dispatch: **0 fails** (exit 0) ✓
- os: **0 fails** (exit 0) ✓
- host: **7 fails** (exit 1) ✗ — ALL SET_OS, out of scope.
- Final line: **`✗ notifier stub-compile gate FAILED`** (exit 1) — because of the host
  suite's SET_OS failures, NOT because of QUERY_CALLBACK.

The P1M2T2S1 PRP claimed the gate "PASSED" — that was written assuming the host suite
wasn't in the runner. It now is. QUERY_CALLBACK contributes **0** failures; the gate's
overall failure is entirely the SET_OS blocker (P1.M2.T2.S3's scope).

## Known shared limitation (OUT OF SCOPE — note, do not fix here)
The `0x03 == ETX` framing collision: `hid_notify`'s byte loop treats ANY payload byte
equal to `0x03` as the message terminator. This affects:
- **SET_OS** cmd_id `0x03` (documented blocker → P1.M2.T2.S3 / firmware plan/003
  P1.M3.T2.S1).
- **QUERY_CALLBACK index == 3**: the index byte `0x03` would terminate early, so
  msg_buffer = `[0xF0][0x02]` and `handle_typed_command` reads `index = data[2] = 0`
  (misread). Reachable only if a board defines ≥4 callbacks.

This is a **protocol/framing** issue, NOT a QUERY_CALLBACK handler defect (the handler
is correct given correct msg_buffer contents). Fixing it requires a framing-level
escape change, tracked elsewhere. **Not in scope for P1.M2.T2.S2.** The handler
verification must NOT attempt to "fix" this by special-casing index==3 in the handler
(it would be the wrong layer and would diverge from the sibling cases).

## Conclusion
The deliverable is VERIFICATION + a verification report (empty `git diff` expected),
exactly like P1M2T2S1. The dominant risk this PRP neutralizes: an agent taking the
contract's `data[1]` literally would change the index read to `data[1]`, **breaking**
QUERY_CALLBACK (it would read the cmd_id byte as the index).
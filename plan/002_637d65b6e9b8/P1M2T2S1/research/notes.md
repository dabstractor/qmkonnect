# Research Notes — P1.M2.T2.S1: QUERY_INFO handler

## HEADLINE FINDING — this is a VERIFY & ALIGN task, NOT greenfield

**The QUERY_INFO handler already exists, is committed, is correct, and matches the
work-item contract byte-for-byte.** This is the **third** consecutive P1.M2 task that
turns out to be a desktop-side mirror/coordination view of a firmware feature already
shipped in the firmware repo's own `plan/003_16d737de7a3e` (the authoritative firmware-
side plan). P1.M2.T1.S1 (dispatch skeleton) and P1.M2.T1.S2 (set_host_layer +
apply_host_callbacks) were both VERIFY & ALIGN; P1.M2.T2.S1 (QUERY_INFO) is too.

**Pattern confirmation:** The QMKonnect `plan/002` P1.M2 milestone folds the firmware
repo's work into coordination-view items. The firmware repo's own
`plan/003_16d737de7a3e/P1M2T2S1` is the **authoritative** spec and is **Complete**
(its implementation landed in commit `c5ad578 Implement typed command dispatch and
response builder`).

## Repo under change

- **FIRMWARE (C):** `/home/dustin/projects/qmk-notifier` — remote
  `git@github.com:dabstractor/qmk-notifier`, branch `main`. HEAD `70fcfa1 Integrate
  host tests into acceptance runner`.
- **NOT** the `qmk_notifier` Rust crate (P1.M1). Firmware ↔ crate are independent
  layers; no crate dependency.
- git status: clean except untracked `plan/003_16d737de7a3e/P1M3T3S1/` dir.

## Contract → existing code map (verified by read + grep this session)

The work-item contract specifies the 32-byte QUERY_INFO response layout and the
behavior. Every clause maps to existing, committed code:

| Contract clause | Existing location | Match? |
|---|---|---|
| `has_been_queried=true` on QUERY_INFO | `notifier.c:145` (declared), `notifier.c:659` (set) | ✓ exact |
| `response[0]=0x51` (NOTIFY_RESPONSE_MARKER) | `notifier.c:630` (in `send_typed_response`) | ✓ exact |
| `response[1]=NOTIFY_CMD_QUERY_INFO` (0x01) cmd echo | `notifier.c:631` (send_typed_response echoes cmd_id) | ✓ exact |
| `response[2]=NOTIFY_PROTO_VER` (2) | `notifier.c:661` (`payload[0] = NOTIFY_PROTO_VER`) | ✓ exact |
| `response[3]=feature_flags` | `notifier.c:662-663` (`payload[1]`) | ✓ see delta note |
| `response[4]=get_host_callbacks_size()` (u8) | `notifier.c:664` (`payload[2]`) | ✓ exact |
| `response[5]=board_rules_present() ? 1 : 0` | `notifier.c:665` (`payload[3]`) | ✓ exact |
| `raw_hid_send(response, RAW_REPORT_SIZE)` | `notifier.c:633` (send_typed_response) | ✓ exact |
| `board_rules_present()` checks all default + per-OS maps | `notifier.c:204-217` | ✓ exact |
| `get_host_callbacks_size()` weak default 0 | `notifier.c:124` | ✓ exact |
| Constants: NOTIFY_PROTO_VER=2, FEATURE_APPLY_HOST_CONTEXT=0x01, FEATURE_CALLBACK_REGISTRY=0x02, RESPONSE_MARKER=0x51 | `notifier.h` lines 40-49 | ✓ exact |

## The ONE contract-vs-code delta (CORRECT — keep, do not "fix")

The contract says: *"response[3]=feature_flags (0x01 if board_rules_present() or
always, bitwise-OR 0x02 if get_host_callbacks_size()>0)"*.

The code (`notifier.c:662-663`):
```c
payload[1] = NOTIFY_FEATURE_APPLY_HOST_CONTEXT
           | (get_host_callbacks_size() > 0 ? NOTIFY_FEATURE_CALLBACK_REGISTRY : 0);
```

**The code ALWAYS sets 0x01 (NOTIFY_FEATURE_APPLY_HOST_CONTEXT), unconditionally** — it
does NOT gate it on `board_rules_present()`. This is the **"or always"** branch of the
contract, and it is CORRECT:

- PRD §5 (`h2.83`): *"feature_flags: `0x01` `APPLY_HOST_CONTEXT`"* — bit 0x01 means
  "the firmware supports the APPLY_HOST_CONTEXT command," which it does (the AHC
  handler exists at `notifier.c:709`). This capability is **independent** of whether
  board rules are present.
- The firmware's own authoritative PRP (`plan/003/P1M2T2S1/PRP.md`) is explicit:
  *"feature_flags: NOTIFY_FEATURE_APPLY_HOST_CONTEXT (0x01) is ALWAYS set (this
  firmware implements the namespace)."*
- The test (`test_notifier_host.c:117`) asserts `(r[3] & 0x01) && (r[3] & 0x02)` — both
  bits set, proving 0x01 is unconditional.

➡️ **An implementation agent must NOT change this to gate 0x01 on
board_rules_present().** That would be a regression — it would falsely advertise no
AHC support on a board that has no rules, breaking the host handshake.

## Test coverage (QUERY_INFO scope) — ALL EXIST, ALL PASS

`test_notifier_host.c` (the official host suite, multi-TU, drives the PUBLIC
`hid_notify`):

- **Test (i)** `test_notifier_host.c:111-119` — QUERY_INFO response layout:
  - `r[0]==0x51`, `r[1]==0x01`, `r[2]==2` (proto_ver),
  - `r[3]` has bits 0 and 1 set (feature_flags),
  - `r[4]==2` (callback_count; test defines 2 named callbacks),
  - `r[5]==1` (board_rules_present; test defines board maps).
- **Test (ii)** `test_notifier_host.c:122-139` — has_been_queried / board-state-survival:
  - 1st QUERY_INFO: board `activated_layer==5` NOT cleared (typed path side-effect-free),
    board command NOT disabled.
  - 2nd QUERY_INFO: board layer still NOT cleared (has_been_queried does not clear
    board state).

These are part of the established baseline: **64 run / 57 pass / 7 fail** — the 7
failures are ALL SET_OS-handler / §4.7 OS-change mechanics (out of scope for
QUERY_INFO; tracked by firmware plan/003 P1.M3.T2.S1 = QMKonnect P1.M2.T2.S3).

## The supporting infrastructure (ALL LANDED)

- `send_typed_response(uint8_t cmd_id, const uint8_t *payload, uint8_t payload_len)`
  — `notifier.c:628-635`: `{0}`-inits `response[RAW_REPORT_SIZE=32]`, sets
  `[0x51][cmd_id]`, memcpy's payload (capped at 30), zero-pads the tail, calls
  `raw_hid_send`. This is the shared builder the QUERY_INFO handler calls.
- `handle_typed_command(char *data)` — `notifier.c:651`: switches on
  `(uint8_t)data[1]` (cmd_id). QUERY_INFO is the first case.
- `board_rules_present()` — `notifier.c:204-217`: returns true iff ANY board map
  (default command/layer + all four per-OS command/layer maps) is non-empty.
- `get_host_callbacks()` / `get_host_callbacks_size()` — `notifier.c:123-124`:
  `__attribute__((weak))`, return `{NULL, 0}` when no `DEFINE_HOST_CALLBACKS`;
  overridden (strong) by the macro in a keymap/test.

## Stub-compile warning note

The two prior PRPs noted a baseline of "4 -Wunused warnings." After the QUERY_INFO
handler landed, `board_rules_present` and `has_been_queried` are now USED (the
QUERY_INFO case references both), so they no longer warn. The remaining warnings are
`apply_host_callbacks` + `set_host_layer` ONLY if those are still unused in the stub
build — but they ARE called by the AHC handler, so in the CURRENT state the stub
compile likely has zero -Wunused warnings. (The prior PRPs' "2 warnings" / "4
warnings" baselines predate the full handler set landing.) The implementer should just
confirm NO new warnings appear; the exact count is not load-bearing for a verify task.

## Conclusion

P1.M2.T2.S1 is satisfied by existing, committed, tested code. The implementation
agent's deliverable is a **verification report** (contract map + passing gates + the
one documented delta), with an **empty `git diff`** as the expected outcome. The
dominant risk this PRP neutralizes is a regression from naively "implementing" the
contract — most dangerously by gating feature_flags bit 0x01 on board_rules_present()
(which would falsely advertise no AHC support on rule-less boards and break the host
handshake).
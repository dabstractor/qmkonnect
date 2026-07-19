# PRP — P1.M2.T2.S3: SET_OS handler: update current_os

> **Repo under change:** the **qmk-notifier FIRMWARE** (C) at
> `/home/dustin/projects/qmk-notifier` — remote `git@github.com:dabstractor/qmk-notifier`,
> branch `main`, HEAD **`8441af2`** ("Implement length-aware typed command reassembly").
> This is **NOT** the `qmk_notifier` Rust crate (P1.M1) and does **not** consume the
> v0.3.0 crate tag. Firmware ↔ crate are independent layers. It is the **same repo** as
> P1.M2.T1.S1/S2 and P1.M2.T2.S1/S2; the SET_OS handler is a sibling `case` inside the
> same `handle_typed_command` switch those tasks verified.

---

## ⚠️ READ FIRST — this is a VERIFY & ALIGN task, NOT greenfield

**HEADLINE FINDING (research-confirmed this session):** The firmware repo **already
contains a complete, committed, tested implementation** of the SET_OS handler **AND the
framing fix that makes it reachable**. The fix landed as commit **`8441af2`** during this
research session (the working tree is clean). The QMKonnect `plan/002` P1.M2 milestone is a
desktop-side **mirror/coordination view** with a finer subtask split; the firmware repo's
own `plan/003` had previously documented the SET_OS handler as BLOCKED and deferred the
framing fix to "the dispatcher layer (P1.M2)" — which is exactly this task. This is the
**fifth consecutive** P1.M2 item to be verify-and-align (after T1.S1 dispatch, T1.S2 host
helpers, T2.S1 QUERY_INFO, T2.S2 QUERY_CALLBACK).

Evidence (verified by read + grep + test run against clean HEAD `8441af2` this session):
- The SET_OS `case` is present at `notifier.c:747` inside `handle_typed_command`.
- It reads `os_byte = (uint8_t)data[2]` (L751) — **`data[2]`, NOT `data[1]`** (same
  magic-strip delta as QUERY_CALLBACK), calls `apply_os_change((os_variant_t)os_byte)`
  (L753), and replies `[0x51][0x03][0x01]` via `send_typed_response(NOTIFY_CMD_SET_OS,
  payload, 1)` (L755).
- **The 0x03==ETX framing blocker (BUG-1) is RESOLVED** by length-aware typed reassembly:
  `typed_literal_remaining` (L115) + `typed_fixed_arg_bytes()` (L129) + the gated ETX check
  `if (c == ETX_TERMINATOR[0] && !typed_literal)` (L862). This is the commit `8441af2` delta.
- `test_notifier_host.c` SET_OS coverage is **100% PASS**: (ii-pre) OS_UNSURE baseline,
  (i) response layout, (ii) current_os changed, (iii) F9 clear, (iv) idempotent.
- `./run_notifier_stub_tests.sh` → **dispatch 14/14, os 31/31, host 64/64, gate PASSED**
  (was host 57/64 with 7 SET_OS failures at the prior HEAD `c07e84f`).

➡️ **Therefore this PRP's deliverable is VERIFICATION + ALIGNMENT, not new code.** An
implementation agent that "implements the literal contract" (read `data[1]`, call
`notifier_set_os` directly, "just send the ack") would (a) **regress** the index read to
the `cmd_id` byte, AND (b) **fail to recognize that without the framing fix the handler is
UNREACHABLE** — re-introducing BUG-1 by reverting or weakening `8441af2`.

---

## 🚨 CRITICAL REGRESSION WARNING — do NOT implement the literal contract text verbatim

The work-item **contract text** is *naive* in THREE places where the committed code is
*correct*. **Align your UNDERSTANDING to the code; do NOT align the code to the contract text.**

| # | Literal contract (DO NOT implement verbatim) | Actual on `main` HEAD `8441af2` (CORRECT — keep) | Why the code is right |
|---|---|---|---|
| 1 | *"On SET_OS (0x03): read os_byte, call notifier_set_os, send ack"* — describes the handler in isolation, **completely silent on the framing blocker** | The handler at `notifier.c:747` is correct, BUT it is only reachable because of the **length-aware typed reassembly** (`typed_literal_remaining` L115, `typed_fixed_arg_bytes` L129, gated ETX L862) added in commit `8441af2`. **Without that fix, SET_OS's cmd_id `0x03` == ETX `0x03`, so `hid_notify`'s byte loop terminates on the cmd_id byte BEFORE the handler runs** — `handle_typed_command` sees `cmd_id=0` (default case) and SET_OS never dispatches. | SET_OS is the ONE typed command whose cmd_id collides with ETX. The handler code is necessary but NOT sufficient; the framing fix is the load-bearing change. Do NOT revert `8441af2` and do NOT "simplify" the byte loop back to `if (c == ETX_TERMINATOR[0])`. |
| 2 | *"read os_byte from `data[1]"* | `uint8_t os_byte = (uint8_t)data[2];` (`notifier.c:751`) — the os_byte is at **`data[2]`**, NOT `data[1]` | `hid_notify` strips the `[0x81][0x9F]` magic header (`data += 2`, L840) before reassembling into `msg_buffer`, so the layout passed to `handle_typed_command` is `data[0]=0xF0` (discriminator), `data[1]=cmd_id`, `data[2]=first arg` (the os_byte). This is documented at `notifier.c:683-685` and is **identical to the QUERY_CALLBACK handler** (S2, which reads the index from `data[2]`). Changing to `data[1]` would read the `cmd_id` byte (0x03) as the os_byte for EVERY SET_OS — `OS_MACOS`-only for `0x03`, garbage otherwise. The contract's `data[1]` is indexing imprecision. |
| 3 | *"Call `notifier_set_os((os_variant_t)os_byte)`"* | `apply_os_change((os_variant_t)os_byte);` (`notifier.c:753`) — calls the **shared seam** directly, not the public `notifier_set_os` | `notifier_set_os` (L659-661) is a **one-line forwarder**: `void notifier_set_os(os_variant_t os) { apply_os_change(os); }`. The two are **functionally identical**; the code calls `apply_os_change` directly so the F9 clear-on-change logic (idempotent guard + `disable_command` + `deactivate_layer`) is **NOT duplicated** in the handler. Calling `notifier_set_os` would also work (one extra call frame) but the code's choice is intentional and documented (L742-745). Either is acceptable; do NOT rewrite one into the other. |

**Other things you must NOT do:**
- Do NOT revert commit `8441af2` or weaken the `typed_literal_remaining` mechanism. It is
  the ONLY thing that makes SET_OS (and AHC-with-0x03-args) dispatch. Removing it
  re-introduces the 7 documented SET_OS host-test failures.
- Do NOT change the SET_OS handler, `send_typed_response`, `handle_typed_command`, or
  `apply_os_change`/`notifier_set_os` signatures or bodies.
- Do NOT "fix" the os_byte value for OS_MACOS. `OS_MACOS == 3 == 0x03` is a deliberate
  `os_variant_t` enum value (from QMK's `os_detection.h`); the framing fix already handles
  a `0x03` os_byte literally. Do not special-case it.
- Do NOT change the ack byte from `0x01`. The contract response `[0x51][0x03][0x01]` means
  `ack=1` (applied); `0x00` is reserved for a future NACK and must NOT be sent here.

---

## Goal

**Feature Goal**: Confirm the firmware's SET_OS handler satisfies every point of the
P1.M2.T2.S3 contract (and the authoritative PRD §4.6 wire / §4.7 OS), that the framing
fix (`8441af2`) correctly resolves the `0x03==ETX` blocker (BUG-1) that previously made the
handler unreachable, that the SET_OS-specific host tests (response layout, current_os
change, F9 clear, idempotence, OS_UNSURE baseline) all pass, and that `notifier.c`
stub-compiles clean. **No source change is expected** unless a genuine SET_OS-level defect
is found (none was found in research).

**Deliverable**: A verification report (inline in the implementation session) that maps
each contract point to its existing `notifier.c` location, confirms the framing-fix
mechanism, shows the passing test evidence (64/64, was 7 SET_OS failures), and records the
three documented contract-vs-code deltas (the hidden framing blocker; `data[2]` not
`data[1]`; `apply_os_change` not `notifier_set_os` — all *expected and correct*, not
defects). If (and only if) a real defect is found, a minimal surgical fix that preserves
the length-aware reassembly architecture and the shared-seam design.

**Success Definition**:
- Every row of the verification map (below) is satisfied by existing code (verified by read + grep).
- `notifier.c` stub-compiles with **exit 0, no new warnings** beyond the carried
  `-Wunused-function` set for static helpers.
- `test_notifier_host.c` shows the SET_OS family — (ii-pre) OS_UNSURE baseline, (i)
  response layout, (ii) current_os changed, (iii) F9 clear, (iv) idempotent — **100% pass**,
  bringing the host suite to **64/64** and the overall stub-compile gate to **PASSED**.
- `git diff` is **empty** at the end of the task (or, in the defect-fix case, a minimal,
  justified diff with all gates still green).

## User Persona (if applicable)

**Target User**: The QMKonnect desktop host (P4.M2.T1.S1 handshake, planned) that sends
`SET_OS` **once at connect** to push the authoritative host OS to the keyboard
(`[0x81][0x9F][0xF0][0x03][os_byte][0x03]`), then later on an OS change. While connected,
the host OS is **authoritative** for `current_os`; firmware `OS_DETECTION` is the offline
fallback (PRD §4.7). (2) Every keymap author who uses `DEFINE_SERIAL_*_OS(OS_MACOS,…)` and
relies on `current_os` flipping to select the per-OS command/layer maps.

**Use Case**: Host connects → `SET_OS(OS_MACOS=3)` → `hid_notify` classifies the report
(`data[2]==0xF0` → `typed_mode=true`, seeds `typed_literal_remaining=2`), the byte loop
consumes `[0xF0, 0x03(cmd_id), 0x03(os_byte)]` LITERALLY (the two `0x03` bytes are NOT
treated as ETX because `typed_literal_remaining > 0`), then the terminating `0x03` fires
`handle_typed_command(msg_buffer, 4)` → the SET_OS case reads `os_byte=data[2]=3`, calls
`apply_os_change(OS_MACOS)`, and replies `[0x51][0x03][0x01]`. `current_os` becomes
`OS_MACOS` (idempotent-skipped if already MACOS; F9-clears if changed). The next focus-change
legacy string selects the `OS_MACOS` maps. Board state clear happens only on a CHANGED OS
(F9.1), never on a repeat (F9.3).

**Pain Points Addressed**: Gives the host a deterministic, single-shot OS push over Raw HID
without reflashing, decoupling `current_os` from the firmware's heuristic `OS_DETECTION`
(which is unreliable, e.g. macOS-on-ARM's delayed stability). The `0x51` response marker
(≥2) is distinct from the legacy `0`/`1` match-bool, so the host disambiguates typed from
legacy. Critically: the framing fix means the host need not avoid `0x03` in any arg —
`SET_OS(OS_MACOS)` works even though BOTH its cmd_id and os_byte are `0x03`.

## Why

- **Closes the QMKonnect-side tracking view** of a firmware feature already shipped in the
  firmware repo (commit `8441af2`). The value this PRP adds is *preventing two regressions*:
  (a) an agent taking the naive contract at face value would read the os_byte from `data[1]`
  (the cmd_id byte), and (b) an agent "simplifying" the byte loop would re-introduce BUG-1.
- **Lands the framing fix ownership**: the firmware's `plan/003` P1.M3.T1.S3 (the SET_OS
  *test* task) explicitly documented the `0x03==ETX` blocker as "out of scope for this
  test-only task, owned by P1.M2 (dispatcher layer)". This QMKonnect task (P1.M2.T2.S3) IS
  that dispatcher-layer owner; commit `8441af2` is its implementation. This PRP confirms it.
- **Enforces the F9 OS-change contract (PRD §4.7 / §2 F9):** SET_OS routes through
  `apply_os_change` — the SAME seam as `notifier_set_os` — so the idempotent guard (F9.3),
  state clear on change (F9.1: `disable_command` + `deactivate_layer`), and no-re-dispatch
  (F9.2) are shared, not duplicated.

## What

Verify (do not rewrite) the following, all of which already exist in `notifier.c` at HEAD
`8441af2`:

### Success Criteria
- [ ] The SET_OS `case NOTIFY_CMD_SET_OS:` exists inside `handle_typed_command` (`notifier.c:747`).
- [ ] It has a length guard `if (len < 3) { send_typed_response(cmd_id, NULL, 0); break; }` (`notifier.c:748-751`) — BUG-3 hardening.
- [ ] It reads `uint8_t os_byte = (uint8_t)data[2];` (`notifier.c:751`) — **`data[2]`, not `data[1]`**.
- [ ] It calls `apply_os_change((os_variant_t)os_byte);` (`notifier.c:753`) — the shared F9 seam (NOT `notifier_set_os` directly; equivalent).
- [ ] It builds `uint8_t payload[1] = { 0x01 };` (ack=applied) and calls `send_typed_response(NOTIFY_CMD_SET_OS, payload, 1)` (`notifier.c:754-755`).
- [ ] `send_typed_response` (`notifier.c:669`) emits exactly 32 bytes: `response[0]=0x51`, `response[1]=0x03` (cmd echo), `response[2]=0x01` (ack), zero-padded tail; calls `raw_hid_send(response, RAW_REPORT_SIZE)` (L678).
- [ ] `apply_os_change` (`notifier.c:636`) is idempotent (`if (os == current_os) return;`), clears on change (`disable_command()` + `deactivate_layer()`), does NOT re-dispatch. `notifier_set_os` (L659-661) is a one-line forwarder to it.
- [ ] `current_os` is `os_variant_t current_os = OS_UNSURE;` (`notifier.c:176`).
- [ ] `os_variant_t` maps `OS_UNSURE=0, OS_LINUX=1, OS_WINDOWS=2, OS_MACOS=3, OS_IOS=4` (`qmk_stubs/os_detection.h`).
- [ ] **THE FRAMING FIX (the load-bearing change):** `typed_literal_remaining` (L115) + `typed_fixed_arg_bytes()` (L129, SET_OS→1) + the gated ETX check `if (c == ETX_TERMINATOR[0] && !typed_literal)` (L862) + the seed `typed_literal_remaining = 2` on typed entry (L837) + reset at every ETX (L890) and overflow (L935) boundary.
- [ ] `notifier.c` stub-compiles with exit 0, no new warnings.
- [ ] `test_notifier_host.c` SET_OS family (ii-pre, i, ii, iii, iv) passes; host suite 64/64.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge of this codebase can, using only this PRP + the
firmware repo, (a) confirm the SET_OS handler + framing fix are present and correct, (b)
build and run the host test to see the SET_OS family pass (64/64), (c) understand WHY the
naive contract is unreachable without the framing fix, and (d) avoid regressing either —
because the three contract-vs-code deltas, the BUG-1 analysis, the code map, and the exact
commands are all here (see also `research/framing_blocker.md` and `research/state_assessment.md`).

### Documentation & References

```yaml
# MUST READ — PRD sections (QMKonnect plan/002 selected selectors; authoritative firmware
# detail is in the firmware repo's own PRD.md §4.6/§4.7/§14).
- url: spec/PRD.md (heading h2.83 "Wire Protocol (typed commands)")
  why: "SET_OS command-table row: Request args [os_byte], Response payload [ack];
        os_byte 0 UNSURE · 1 LINUX · 2 WINDOWS · 3 MACOS · 4 IOS; the host sends SET_OS
        once at connect; while connected the host OS is AUTHORITATIVE for current_os
        (firmware OS_DETECTION is the offline fallback)."
  critical: "Typed commands are ETX-framed and may span reports. Responses use the 0x51
        marker (distinct from legacy 0/1). Typed commands BYPASS process_full_message
        (no board side effects). NOTE: the PRD text reuses string ETX-framing for binary
        typed payloads — this is the root of BUG-1 (any payload byte == 0x03 collides
        with ETX); the firmware resolves it with length-aware reassembly, NOT a wire change."

- url: spec/PRD.md (heading h2.84 "Firmware Spec (qmk-notifier)")
  why: "SET_OS (0x03) — update current_os (host-authoritative while a host is connected;
        firmware OS_DETECTION resumes as the offline fallback)."

# MUST READ — existing firmware source (the thing being verified)
- file: /home/dustin/projects/qmk-notifier/notifier.c
  why: the SET_OS handler + the framing fix + all dependencies
  pattern: "L42 RAW_REPORT_SIZE=32; L79 MSG_BUFFER_SIZE=256; L115 typed_literal_remaining;
           L129-136 typed_fixed_arg_bytes (SET_OS->1); L176 current_os=OS_UNSURE;
           L636-647 apply_os_change (F9 seam); L659-661 notifier_set_os forwarder;
           L669-679 send_typed_response (the [0x51] builder, caps payload at 30 bytes);
           L683-685 msg_buffer layout doc (data[0]=0xF0, data[1]=cmd_id, data[2..]=args);
           L700 handle_typed_command(data, len); L747-756 SET_OS case (the deliverable);
           L836-837 typed_mode seed + typed_literal_remaining=2; L840 magic strip data+=2;
           L858 typed_literal flag; L862 gated ETX check; L875 handle_typed_command(msg_buffer,msg_index);
           L890/L935 reset typed_literal_remaining at ETX/overflow; L906-925 literal accounting"
  gotcha: "the os_byte is at data[2], NOT data[1] (magic header stripped before reassembly).
           AND the handler is UNREACHABLE without the typed_literal_remaining framing fix
           (BUG-1): SET_OS cmd_id 0x03 == ETX 0x03, and OS_MACOS==3 makes the os_byte also
           0x03 — both would terminate reassembly early without the length-aware fix."

- file: /home/dustin/projects/qmk-notifier/notifier.h
  why: "all NOTIFY_* constants + os_detection.h include"
  pattern: "L3 #include os_detection.h (os_variant_t TYPE only); L30 notifier_set_os decl;
           L44 NOTIFY_CMD_DISCRIMINATOR 0xF0; L46 NOTIFY_RESPONSE_MARKER 0x51;
           L50 NOTIFY_CMD_SET_OS 0x03; L51 NOTIFY_CMD_APPLY_HOST_CONTEXT 0x05"
  gotcha: "constants ALREADY exist — do not re-add or renumber (0x04 is reserved for VIA).
           notifier.h does NOT redeclare os_variant_t; it comes from os_detection.h."

- file: /home/dustin/projects/qmk-notifier/qmk_stubs/os_detection.h
  why: "the os_variant_t enum values (the os_byte mapping)"
  pattern: "OS_UNSURE=0, OS_LINUX=1, OS_WINDOWS=2, OS_MACOS=3, OS_IOS=4"
  gotcha: "OS_MACOS==3==0x03==ETX. This is why SET_OS(OS_MACOS) is DOUBLY broken without
           the framing fix (cmd_id AND os_byte both 0x03). The enum values are fixed by
           QMK upstream; do not change them."

# MUST READ — the preceding (CONTRACT) PRPs whose outputs this handler consumes
- file: plan/002_637d65b6e9b8/P1M2T1S1/PRP.md
  why: "verified the typed_mode fork + handle_typed_command dispatch skeleton + send_typed_response
        that the SET_OS case lives inside"
  critical: "handle_typed_command switches on (uint8_t)data[1]; the [0x51] response is sent
        INSIDE it; hid_notify's typed_dispatched suppresses the legacy ack. All LANDED —
        do not re-add. NOTE: the signature is now handle_typed_command(char *data, uint16_t len)
        (the len param was added by 8441af2 for BUG-3 hardening)."

- file: plan/002_637d65b6e9b8/P1M2T2S2/PRP.md   # QUERY_CALLBACK — the structural sibling
  why: "the immediately-preceding sibling. Same verify-and-align nature; confirms the shared
        data[2]-not-data[1] delta and the send_typed_response [0x51] builder contract."
  critical: "SET_OS (notifier.c:747) is the structural sibling of QUERY_CALLBACK (notifier.c:717)
        and QUERY_INFO. All live in the same switch; all use send_typed_response. The
        msg_buffer layout (data[0]=0xF0, data[1]=cmd_id, data[2..]=args) is authoritative here."

# Reference — existing tests
- file: /home/dustin/projects/qmk-notifier/test_notifier_host.c   # 64-test typed suite; (ii-pre)+(i-iv) cover SET_OS
- file: /home/dustin/projects/qmk-notifier/test_notifier_dispatch.c # 14-test legacy/dispatch regression
- file: /home/dustin/projects/qmk-notifier/test_notifier_os.c       # 31-test multi-OS regression (F9 pattern template)
- file: /home/dustin/projects/qmk-notifier/run_notifier_stub_tests.sh # committed gate (dispatch + os + host)
- file: /home/dustin/projects/qmk-notifier/qmk_stubs/              # host-compile stubs (QMK_KEYBOARD_H, raw_hid_send capture, layer state)

# Reference — this task's research notes (deep-dive on the blocker + fix)
- file: plan/002_637d65b6e9b8/P1M2T2S3/research/framing_blocker.md  # BUG-1 root cause + the length-aware fix design + alternatives rejected
- file: plan/002_637d65b6e9b8/P1M2T2S3/research/state_assessment.md # HEAD evolution + definitive 64/64 test evidence
```

### Current Codebase tree (firmware repo, verification-relevant only)

```bash
# run from /home/dustin/projects/qmk-notifier
notifier.h            # NOTIFY_* constants, os_detection.h include, notifier_set_os decl
notifier.c            # SET_OS case (L747), apply_os_change (L636), notifier_set_os (L659),
                      # send_typed_response (L669), handle_typed_command(data,len) (L700),
                      # typed_literal_remaining (L115), typed_fixed_arg_bytes (L129),
                      # current_os=OS_UNSURE (L176), RAW_REPORT_SIZE=32 (L42), hid_notify (~L815)
qmk_stubs/            # host-compile stubs (QMK_KEYBOARD_H, raw_hid_send capture, layer_on/off, os_detection.h)
test_notifier_dispatch.c   # 14-test legacy/dispatch regression suite
test_notifier_os.c         # 31-test multi-OS regression suite (F9 clear-on-change template)
test_notifier_host.c       # 64-test typed-command suite (SET_OS = (ii-pre)+(i)+(ii)+(iii)+(iv))
run_notifier_stub_tests.sh # committed gate: stub-compile + dispatch + os + host
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
// CRITICAL — THE BLOCKER (BUG-1): SET_OS cmd_id 0x03 == ETX 0x03. hid_notify's byte loop
// treats EVERY 0x03 as the message terminator. Without the length-aware typed reassembly
// (typed_literal_remaining, L115), the loop terminates on the cmd_id byte BEFORE the
// handler runs -> handle_typed_command sees cmd_id=0 (default case) -> SET_OS never
// dispatches. OS_MACOS==3 makes the os_byte ALSO 0x03, so even a cmd_id-only fix fails.
// The committed fix (8441af2) consumes the command's KNOWN arity bytes literally
// (0x03 included) and only honors 0x03 as ETX AFTER the full arg set. This is load-bearing;
// do NOT revert or "simplify" it.
//
// CRITICAL: the os_byte is read from data[2], NOT data[1]. hid_notify does data += 2 to
// strip the [0x81][0x9F] magic header BEFORE reassembling into msg_buffer. So inside
// handle_typed_command the layout is: data[0]==0xF0 (discriminator), data[1]==cmd_id,
// data[2..]==args. The contract text's "read os_byte from data[1]" is IMPRECISE. The
// committed code reads data[2] (notifier.c:751) and is CORRECT — do NOT change it to
// data[1] (that would read the cmd_id byte 0x03 as the os_byte).
//
// GOTCHA: apply_os_change() (L636) is the shared F9 seam; notifier_set_os() (L659) is a
// one-line forwarder to it. The SET_OS handler calls apply_os_change DIRECTLY (not
// notifier_set_os) so the F9 clear-on-change logic is NOT duplicated. The two are
// functionally identical. Do NOT rewrite one into the other.
//
// GOTCHA: OS_MACOS==3 is a fixed QMK upstream enum value (qmk_stubs/os_detection.h). It
// collides with ETX, but the framing fix already handles a 0x03 os_byte literally. Do NOT
// special-case OS_MACOS or remap the enum.
//
// GOTCHA: the ack byte is 0x01 (applied). The contract response [0x51][0x03][0x01] means
// ack=1. 0x00 is reserved for a future NACK. Do NOT change the ack to 0x00.
//
// GOTCHA: SET_OS is side-effect-free on HOST state (it does not touch host_layer /
// host_cb_enabled[] / has_been_queried). It DOES mutate BOARD state via apply_os_change on
// a CHANGED os (F9.1: disable_command + deactivate_layer) — that is intended (§4.7/F9),
// not a side-effect leak. It is idempotent on an unchanged os (F9.3: no-op).
//
// GOTCHA: the typed_literal_remaining state is `static` (L115) so it survives across
// hid_notify calls — this is what makes MULTI-REPORT typed messages reassemble (the byte
// count survives across reports, exactly like msg_index). It is reset at every ETX
// boundary (L890) and on overflow (L935). If you "fix" a reset, you break multi-report.
//
// GOTCHA: handle_typed_command now takes `len` (the reassembled msg_index) — BUG-3
// hardening. Each case validates len >= its minimum footprint before indexing args; a
// truncated frame falls through to the default no-payload ack. Called as
// handle_typed_command(msg_buffer, msg_index) at L875.
```

## Implementation Blueprint

### Verification map — contract point → existing code (NO new data models; C firmware)

| # | Contract point | Existing location | Verified? |
|---|---|---|---|
| 1 | `current_os` is `os_variant_t`, boot `OS_UNSURE` | `notifier.c:176` `os_variant_t current_os = OS_UNSURE;` | ☐ |
| 2 | OS-change seam: idempotent guard + state clear (F9) | `notifier.c:636-647` `apply_os_change` (`if (os==current_os) return;` then `disable_command()` + `deactivate_layer()`) | ☐ |
| 3 | `notifier_set_os` is the keymap entry point (forwarder) | `notifier.c:659-661` `void notifier_set_os(os_variant_t os) { apply_os_change(os); }` | ☐ |
| 4 | `SET_OS` `case` inside `handle_typed_command`, switch on cmd_id | `notifier.c:747` (`case NOTIFY_CMD_SET_OS:`); `cmd_id=(uint8_t)data[1]` L701 | ☐ |
| 5 | read the os_byte from the command args | `notifier.c:751` `os_byte=(uint8_t)data[2]` — **DELTA: data[2] not data[1]** (CORRECT per msg_buffer layout) | ☐ |
| 6 | map os_byte to os_variant_t and apply | `notifier.c:753` `apply_os_change((os_variant_t)os_byte)` — **DELTA: apply_os_change not notifier_set_os** (equivalent; shared seam) | ☐ |
| 7 | length guard (BUG-3): require disc+cmd+os_byte | `notifier.c:748-751` `if (len < 3) { send_typed_response(cmd_id, NULL, 0); break; }` | ☐ |
| 8 | build ack payload `[0x01]` | `notifier.c:754` `uint8_t payload[1] = { 0x01 };` (ack=applied) | ☐ |
| 9 | `response[0]=0x51` (NOTIFY_RESPONSE_MARKER) | `notifier.c:670` (send_typed_response) | ☐ |
| 10 | `response[1]=NOTIFY_CMD_SET_OS` (0x03) cmd echo | `notifier.c:671` (send_typed_response echoes cmd_id) | ☐ |
| 11 | `response[2]=ack=0x01`, zero-padded to 32 | payload[0]=0x01→response[2]; `raw_hid_send(response, RAW_REPORT_SIZE)` L678 (32 bytes) | ☐ |
| 12 | os_variant_t mapping 0-4 | `qmk_stubs/os_detection.h` (OS_UNSURE=0…OS_IOS=4) | ☐ |
| 13 | Constants: SET_OS=0x03, RESPONSE_MARKER=0x51, RAW_REPORT_SIZE=32 | `notifier.h:50,46`; `notifier.c:42` | ☐ |
| 14 | **THE FRAMING FIX (BUG-1):** cmd_id 0x03 no longer terminates early | `typed_literal_remaining` L115; `typed_fixed_arg_bytes` L129 (SET_OS→1); seed L837; gated ETX `&& !typed_literal` L862; reset L890/L935 | ☐ |
| 15 | OS_MACOS==3 os_byte handled literally (not terminated) | same mechanism (os_byte consumed while typed_literal_remaining>0); test (i)+(ii) PASS with OS_MACOS | ☐ |
| 16 | SET_OS idempotent on unchanged os (F9.3) | `apply_os_change` early-return; test (iv) PASS | ☐ |
| 17 | SET_OS change fires F9 clear (F9.1), no re-dispatch (F9.2) | `apply_os_change` disable_command+deactivate_layer; test (iii) PASS | ☐ |

### Implementation Tasks (verification-ordered)

```yaml
Task 1: ESTABLISH baseline (no edits)
  - RUN: cd /home/dustin/projects/qmk-notifier && git status -s && git log --oneline -3
  - EXPECT: clean tree, HEAD at or past 8441af2 ("Implement length-aware typed command reassembly")
  - READ: notifier.c L42 (RAW_REPORT_SIZE), L115 (typed_literal_remaining), L129-136
          (typed_fixed_arg_bytes), L176 (current_os), L636-647 (apply_os_change), L659-661
          (notifier_set_os forwarder), L669-679 (send_typed_response), L683-685 (msg_buffer
          layout doc), L700-701 (handle_typed_command head), L747-756 (SET_OS case — the
          deliverable), L836-837 (typed seed), L858-862 (gated ETX), L875 (dispatch call),
          L890/L935 (resets); notifier.h L3,30,44,46,50 (constants + decls);
          qmk_stubs/os_detection.h (enum values)
  - CONFIRM every row of the Verification map above (check the ☐ boxes in your report)
  - NOTE the three documented contract-vs-code deltas (framing blocker hidden by contract;
    data[2] not data[1]; apply_os_change not notifier_set_os) are CORRECT, not defects —
    record them as "verified correct, kept as-is"

Task 2: RUN the committed stub-compile gate (expect GREEN now)
  - RUN: ./run_notifier_stub_tests.sh
  - EXPECT (current baseline, accurate as of HEAD 8441af2):
      * dispatch fails=0  (exit=0)   ✓
      * os fails=0        (exit=0)   ✓
      * host fails=0      (exit=0)   ✓   <- was 7 (all SET_OS) before 8441af2
      * final line: "✓ notifier stub-compile gate PASSED" (exit 0)
  - IF host shows ANY failure: that indicates a REAL regression worth investigating
    (likely a partial revert of 8441af2). Diagnose root cause before any edit. The 7 SET_OS
    failures returning means the typed_literal_remaining mechanism was lost — re-apply
    commit 8441af2 (git cherry-pick / git revert) rather than hand-patching. Pre-existing
    -Wunused-function warnings are EXPECTED — do not silence them.

Task 3: BUILD+RUN the typed-command host suite; isolate the SET_OS family
  - RUN:
      gcc -Wall -Wextra -std=c99 -DQMK_KEYBOARD_H='"qmk_keyboard_stub.h"' -Iqmk_stubs -I. \
          -c notifier.c -o /tmp/nh.o
      gcc -Wall -std=c99 -Iqmk_stubs -I. /tmp/nh.o qmk_stubs/qmk_stubs.c test_notifier_host.c \
          -o /tmp/test_notifier_host
      /tmp/test_notifier_host 2>&1 | grep -iE 'SET_OS|current_os|Total tests'
  - EXPECT: 64 run / 64 pass / 0 fail. SET_OS-relevant groups that MUST be 100% pass:
      (ii-pre) OS_UNSURE baseline: 'iTerm' does NOT match at OS_UNSURE   [test_notifier_host.c:187-197]
      (i)  SET_OS r[0]=0x51, r[1]=0x03, r[2]=ack=1                       [test_notifier_host.c:199-206]
      (ii) post-SET_OS(OS_MACOS): mac_cmd fired + layer 44 selected      [test_notifier_host.c:208-220]
      (iii) SET_OS change: prev on_disable + layer deactivated, no re-d  [test_notifier_host.c:222-238]
      (iv)  SET_OS idempotent: no spurious disable/layer on same-OS      [test_notifier_host.c:240-248]
  - PROBE (optional, confirms BUG-1 is truly fixed): if any SET_OS line begins with "FAIL",
    the framing fix is absent/broken — see Task 2's recovery note.

Task 4: (DEFAULT) NO-OP — write the verification report, leave source untouched
  - IF Tasks 1-3 are green (expected path): the deliverable is the inline report. git diff
    stays empty. Done.
  - IF a genuine SET_OS-level defect is found (unexpected): make the MINIMAL surgical fix
    in notifier.c that preserves the length-aware reassembly architecture and the shared-seam
    design, then re-run Tasks 2 & 3 to confirm 64/64. Document the defect, the fix, and the
    before/after test counts in the report.

Task 5: NEVER do these
  - DO NOT revert or weaken commit 8441af2 (the typed_literal_remaining framing fix). It is
    the ONLY thing that makes SET_OS dispatch (BUG-1). Removing it re-introduces the 7
    documented SET_OS host-test failures.
  - DO NOT change the os_byte read from data[2] to data[1] — data[2] is correct (the
    [0x81][0x9F] magic header is stripped before reassembly; data[0]=0xF0, data[1]=cmd_id,
    data[2]=first arg). data[1] is the cmd_id byte (0x03); reading it as the os_byte is
    wrong for every OS except coincidentally MACOS.
  - DO NOT rewrite apply_os_change into notifier_set_os (or vice versa) in the handler.
    They are functionally identical (notifier_set_os forwards to apply_os_change). The
    code's direct call to apply_os_change avoids duplicating the F9 logic — correct.
  - DO NOT change the ack byte from 0x01. ack=1 means applied; 0x00 is reserved for NACK.
  - DO NOT special-case OS_MACOS or remap the os_variant_t enum (OS_MACOS==3 is fixed by
    QMK upstream; the framing fix already handles a 0x03 os_byte literally).
  - DO NOT make SET_OS touch HOST state (host_layer, host_cb_enabled[], has_been_queried).
    It mutates BOARD state only (via apply_os_change, F9 clear on a changed os — intended).
  - DO NOT change the SET_OS handler, send_typed_response, handle_typed_command,
    apply_os_change, or notifier_set_os signatures or bodies.
  - DO NOT renumber NOTIFY_* constants (0x04 stays reserved for VIA).
  - DO NOT silence pre-existing -Wunused-function stub-compile warnings.
  - DO NOT edit PRD.md, any tasks.json, prd_snapshot.md, or any plan/ files (read-only).
```

### Implementation Patterns & Key Details
```c
// The existing (correct) SET_OS handler — notifier.c:747-756:
//   case NOTIFY_CMD_SET_OS: {
//       if (len < 3) {                                   /* BUG-3: disc + cmd + os_byte */
//           send_typed_response(cmd_id, NULL, 0);
//           break;
//       }
//       uint8_t os_byte = (uint8_t)data[2];              /* ARG[0] — data[2], NOT data[1] */
//       apply_os_change((os_variant_t)os_byte);          /* shared F9 seam (notifier_set_os delegates here) */
//       uint8_t payload[1] = { 0x01 };                   /* ack = 1 (applied) */
//       send_typed_response(NOTIFY_CMD_SET_OS, payload, 1);
//       break;
//   }

// The existing (correct) F9 seam — notifier.c:636-647 (apply_os_change):
//   static void apply_os_change(os_variant_t os) {
//       if (os == current_os) return;            /* idempotent: no flap on repeat (F9.3) */
//       current_os = os;
//       disable_command();                       /* fires prev on_disable if active (F9.1) */
//       deactivate_layer();                      /* turns off the active notifier layer (F9.1) */
//       /* Intentionally do NOT re-dispatch the last message (F9.2). */
//   }
//   void notifier_set_os(os_variant_t os) { apply_os_change(os); }   /* L659-661 forwarder */

// THE LOAD-BEARING FRAMING FIX (BUG-1) — notifier.c:858-862 (the gated ETX check):
//   bool typed_literal = (typed_mode && typed_literal_remaining > 0);
//   // End of text (ASCII 3) indicates the end of the message — but ONLY on
//   // the legacy path or once the typed command's args are fully consumed.
//   if (c == ETX_TERMINATOR[0] && !typed_literal) {   /* dispatch ... */
// Without the `&& !typed_literal`, SET_OS's cmd_id 0x03 terminates reassembly at
// msg_index==1 -> handle_typed_command sees cmd_id=0 -> default case -> SET_OS never runs.
// The seed `typed_literal_remaining = 2` (L837, on typed entry) consumes disc+cmd_id
// literally; at msg_index==2 the fixed-arg count is added (SET_OS -> +1 for os_byte).
// See research/framing_blocker.md for the full trace.

// The existing (correct) response builder — notifier.c:669-679 (send_typed_response):
//   static void send_typed_response(uint8_t cmd_id, const uint8_t *payload, uint8_t payload_len) {
//       uint8_t response[RAW_REPORT_SIZE] = {0};   /* zero-pads the unused tail */
//       response[0] = NOTIFY_RESPONSE_MARKER;      /* 0x51 */
//       response[1] = cmd_id;                      /* echo (0x03) */
//       if (payload != NULL && payload_len > 0) {
//           uint8_t cap = (uint8_t)(RAW_REPORT_SIZE - 2);   /* 30 bytes after [0x51][cmd_id] */
//           uint8_t n = (payload_len < cap) ? payload_len : cap;
//           memcpy(response + 2, payload, n);
//       }
//       raw_hid_send(response, RAW_REPORT_SIZE);
//   }
//   // => wire bytes: [0]=0x51 [1]=0x03 [2]=0x01 (ack) [3..31]=0 (zero-padded) = 32 bytes total
```

### Integration Points
```yaml
DEPENDENCIES: none — this is self-contained firmware C; it does NOT depend on the
              qmk_notifier Rust crate or its v0.3.0 tag (P1.M1.T4.S1).
              It depends on P1.M2.T1.S1 (the typed-dispatch skeleton + send_typed_response)
              and P1.M1.T2.S2 (the apply_os_change seam refactor) — BOTH already exist and
              are verified. The framing fix (8441af2) is committed.
DOWNSTREAM (consumers — ALL PLANNED):
  - P4.M2.T1.S1 desktop handshake (QMKonnect, planned) — sends SET_OS once at connect to
    push the authoritative host OS; consumes [0x51][0x03][0x01] as the ack.
  - The host sends SET_OS(OS_MACOS=3) — BOTH the cmd_id and os_byte are 0x03; only the
    length-aware reassembly makes this work. The host need not avoid 0x03 in any arg.
HOST (desktop) handshake that drives this — P4.M2.T1 (planned): sends SET_OS at connect.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)
```bash
cd /home/dustin/projects/qmk-notifier
# The gate's step [1/5] does the canonical stub-compile. Do NOT compile notifier.c standalone.
gcc -Wall -Wextra -std=c99 -DQMK_KEYBOARD_H='"qmk_keyboard_stub.h"' -Iqmk_stubs -I. \
    -c notifier.c -o /tmp/notifier_stub.o
# Expected: exit 0. Pre-existing -Wunused-function warnings for static helpers are EXPECTED
# — no NEW warnings.

# Confirm the SET_OS case + its dependencies are present and correct:
grep -n 'case NOTIFY_CMD_SET_OS' notifier.c                # expect one line ~747
grep -n 'uint8_t os_byte = (uint8_t)data\[2\]' notifier.c  # expect one line ~751 (NOT data[1])
grep -n 'apply_os_change((os_variant_t)os_byte)' notifier.c # expect one line ~753
grep -n 'payload\[1\] = { 0x01 }' notifier.c               # expect the ack payload ~754
grep -n 'typed_literal_remaining = 2' notifier.c           # expect the seed ~837 (BUG-1 fix)
grep -n '&& !typed_literal' notifier.c                     # expect the gated ETX ~862 (BUG-1 fix)
rm -f /tmp/notifier_stub.o
```

### Level 2: The committed regression suites (Component Validation)
```bash
cd /home/dustin/projects/qmk-notifier
./run_notifier_stub_tests.sh
# Expected (ACCURATE as of HEAD 8441af2):
#   notifier dispatch fails=0  (exit=0)   ✓
#   notifier os fails=0        (exit=0)   ✓
#   notifier host fails=0      (exit=0)   ✓   <- was 7 (all SET_OS) before 8441af2
#   final line: "✓ notifier stub-compile gate PASSED" (exit 0)
# CRITICAL: the gate is now GREEN. If host shows 7 failures, the typed_literal_remaining
# framing fix (commit 8441af2) was lost — re-apply it rather than hand-patching. dispatch
# 14/14 and os 31/31 MUST stay green (they prove no regression in the reassembler,
# matcher, F4/F5/F8/F9 logic, or the hid_notify routing fork).
```

### Level 3: Typed-command host suite (SET_OS-scope validation)
```bash
cd /home/dustin/projects/qmk-notifier
gcc -Wall -Wextra -std=c99 -DQMK_KEYBOARD_H='"qmk_keyboard_stub.h"' -Iqmk_stubs -I. \
    -c notifier.c -o /tmp/nh.o
gcc -Wall -std=c99 -Iqmk_stubs -I. /tmp/nh.o qmk_stubs/qmk_stubs.c test_notifier_host.c \
    -o /tmp/test_notifier_host
/tmp/test_notifier_host 2>&1 | grep -iE 'SET_OS|current_os|Total tests'
# Expected: every SET_OS line begins with "PASS". Specifically:
#   (ii-pre) 'iTerm' does NOT match at OS_UNSURE (current_os still OS_UNSURE)
#   (i)  SET_OS r[0]=0x51, r[1]=0x03, r[2]=ack=1
#   (ii) post-SET_OS(OS_MACOS): mac_cmd fired + layer 44 selected (current_os changed)
#   (iii) SET_OS change: prev on_disable + layer deactivated, no re-dispatch (F9.1/F9.2)
#   (iv)  SET_OS idempotent: no spurious disable/layer on same-OS (F9.3)
# Suite total: 64 run / 64 pass / 0 fail.
rm -f /tmp/nh.o /tmp/test_notifier_host
```

### Level 4: Legacy-path regression (no SET_OS handler touches it)
```bash
cd /home/dustin/projects/qmk-notifier
./run_all_tests.sh
# Expected: the 9-suite pattern_match corpus is unaffected (legacy string path is
# byte-identical for non-typed reports; SET_OS is only reachable via the typed 0xF0 path,
# and the typed_literal mechanism never activates for legacy strings because typed_mode
# stays false). All suites pass.
```

## Final Validation Checklist

### Technical Validation
- [ ] Verification map (17 rows) fully satisfied by existing code.
- [ ] `notifier.c` stub-compile → exit 0, no new warnings beyond the carried `-Wunused-function` set.
- [ ] `./run_notifier_stub_tests.sh` → dispatch fails=0, os fails=0, **host fails=0** (gate PASSED).
- [ ] `test_notifier_host.c` SET_OS family ((ii-pre), (i), (ii), (iii), (iv)) 100% pass; host 64/64.
- [ ] `./run_all_tests.sh` — 9-suite pattern corpus unaffected.

### Feature Validation
- [ ] SET_OS(OS_MACOS=3) returns `[0x51][0x03][0x01]` AND changes `current_os` to OS_MACOS
      (proving BOTH the cmd_id 0x03 and os_byte 0x03 are consumed literally — BUG-1 fixed).
- [ ] os_byte is read from `data[2]` (NOT `data[1]`) — the magic header is stripped before reassembly.
- [ ] OS-change routes through `apply_os_change` (the shared F9 seam; `notifier_set_os` delegates to it).
- [ ] SET_OS is idempotent on an unchanged os (F9.3); F9-clears on a changed os (F9.1); no re-dispatch (F9.2).
- [ ] Response is exactly 32 bytes (zero-padded by send_typed_response); ack byte is 0x01.

### Code Quality Validation
- [ ] `git diff` is EMPTY (expected default) OR a minimal, justified defect-fix with 64/64 still green.
- [ ] No re-implementation of the (regressive) literal contract (os_byte from data[1]; direct
      notifier_set_os call; ack 0x00; special-casing OS_MACOS).
- [ ] SET_OS handler consumes NOTIFY_* constants by name (no hardcoded 0x51/0x03/0x01 literals).
- [ ] Commit `8441af2` (the framing fix) is intact — the gated ETX check `&& !typed_literal` is present.
- [ ] No renumbering of NOTIFY_CMD_* constants.

### Documentation & Deployment
- [ ] No user-facing docs required (firmware C code — per the work-item DOCS: none).
- [ ] Verification report recorded inline (contract map + test counts + the three documented
      deltas: framing blocker hidden by contract; data[2] not data[1]; apply_os_change not
      notifier_set_os).

---

## Anti-Patterns to Avoid
- ❌ Do NOT revert or weaken commit `8441af2` (the `typed_literal_remaining` framing fix). It is
  the ONLY thing that makes SET_OS dispatch — SET_OS's cmd_id `0x03` == ETX `0x03`, so without
  length-aware reassembly the byte loop terminates on the cmd_id byte before the handler runs.
  `OS_MACOS==3` makes the os_byte collide too. Reverting re-introduces the 7 SET_OS failures.
- ❌ Do NOT change the os_byte read from `data[2]` to `data[1]` — the magic header `[0x81][0x9F]`
  is stripped before reassembly, so inside `handle_typed_command` the layout is
  `data[0]=0xF0, data[1]=cmd_id, data[2]=first arg`. `data[1]` is the `cmd_id` byte (0x03);
  reading it as the os_byte is wrong for every OS except coincidentally MACOS. The contract
  text's `data[1]` is imprecise.
- ❌ Do NOT rewrite `apply_os_change` into `notifier_set_os` (or vice versa) in the handler — they
  are functionally identical (notifier_set_os forwards to apply_os_change). The direct call to
  apply_os_change avoids duplicating the F9 logic.
- ❌ Do NOT change the ack byte from `0x01` — ack=1 means applied; `0x00` is reserved for NACK.
- ❌ Do NOT special-case `OS_MACOS` or remap the `os_variant_t` enum — `OS_MACOS==3` is fixed by
  QMK upstream; the framing fix already handles a `0x03` os_byte literally.
- ❌ Do NOT make SET_OS mutate HOST state (`host_layer`, `host_cb_enabled[]`, `has_been_queried`).
  It mutates BOARD state only (via apply_os_change, F9 clear on a changed os — intended).
- ❌ Do NOT rewrite the SET_OS handler, `send_typed_response`, `handle_typed_command`,
  `apply_os_change`, or `notifier_set_os` — they exist and are tested.
- ❌ Do NOT silence pre-existing `-Wunused-function` stub-compile warnings.
- ❌ Do NOT mistake a re-appearance of the 7 SET_OS failures for a "handler bug" — it means the
  framing fix (`8441af2`) was lost; re-apply the commit, do not hand-patch the handler.
- ❌ Do NOT edit PRD.md, tasks.json, prd_snapshot.md, or any plan/ file.
- ❌ Do NOT assume this task must produce a diff — the expected, correct outcome is a verification
  report with an empty diff.

---

## Confidence Score: 9/10

The deliverable is already present, correct, committed (`8441af2`), and green at its scope
(verified this session: `notifier.c` stub-compiles clean, the SET_OS handler is present at
notifier.c:747-756 with the os_byte correctly read from `data[2]`, routed through the shared
`apply_os_change` F9 seam, and replying `[0x51][0x03][0x01]`; the `0x03==ETX` framing blocker
is resolved by the length-aware `typed_literal_remaining` reassembly; `test_notifier_host.c`
SET_OS family (ii-pre)+(i)+(ii)+(iii)+(iv) all PASS; host suite 64/64; gate PASSED). The
1-point reservation is for the (unlikely) discovery of a genuine SET_OS-level defect during the
implementation agent's own verification pass; if found, the minimal-fix path is specified. The
dominant risk this PRP neutralizes is TWofold: (1) a regression from naively implementing the
imprecise contract text (reading the os_byte from `data[1]`, the cmd_id byte), and (2) a
regression from reverting/"simplifying" the load-bearing framing fix (`8441af2`), which would
silently re-block SET_OS and re-introduce the 7 host failures. The framing blocker is the
single most important non-obvious fact an implementer must understand: **the naive contract is
UNREACHABLE without the length-aware typed reassembly**, because SET_OS's cmd_id `0x03` collides
with the ETX terminator `0x03` (and `OS_MACOS==3` makes the os_byte collide too).
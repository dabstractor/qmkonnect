# PRP — P1.M2.T2.S2: QUERY_CALLBACK handler: return callback name by index

> **Repo under change:** the **qmk-notifier FIRMWARE** (C) at
> `/home/dustin/projects/qmk-notifier` — remote `git@github.com:dabstractor/qmk-notifier`,
> branch `main`, HEAD `70fcfa1`. This is **NOT** the `qmk_notifier` Rust crate (P1.M1)
> and does **not** consume the v0.3.0 crate tag. Firmware ↔ crate are independent layers.
> It is the **same repo** as P1.M2.T1.S1/S2 and P1.M2.T2.S1; the QUERY_CALLBACK handler
> is a sibling `case` inside the same `handle_typed_command` switch those tasks verified.

---

## ⚠️ READ FIRST — this is a VERIFY & ALIGN task, NOT greenfield

**HEADLINE FINDING (research-confirmed this session):** The firmware repo **already
contains a complete, committed, tested implementation** of the QUERY_CALLBACK handler.
It landed in the firmware repo's **own `plan/003_16d737de7a3e` P1M2T2S1** (the firmware
plan bundles "typed-dispatch skeleton + QUERY_INFO + QUERY_CALLBACK + default" into ONE
task, marked Complete in commit `c5ad578`). The QMKonnect `plan/002` P1.M2 milestone is
a desktop-side **mirror/coordination view** with a finer subtask split. This is the
**fourth consecutive** P1.M2 item to be verify-and-align (after T1.S1 dispatch, T1.S2
host helpers, T2.S1 QUERY_INFO).

Evidence (verified by read + grep + test run this session):
- The QUERY_CALLBACK `case` is present at `notifier.c:672` inside `handle_typed_command`.
- It reads `index = (uint8_t)data[2]` (L673), bounds-checks against `get_host_callbacks_size()`
  and `cbs[index].name != NULL` (L674-686), copies the name NUL-padded into the payload
  (L676-681), and replies via `send_typed_response` (L682/L685).
- `test_notifier_host.c` has dedicated QUERY_CALLBACK coverage: group **(iii)** valid
  index (L142-158) and group **(iv)** out-of-range (L161-169) — **both 100% PASS**.
- The side-effect-free assertion "QUERY_INFO/QUERY_CALLBACK fired no host callback
  (read-only queries)" **PASSES** (test_notifier_host.c:175).
- All constants exist in `notifier.h`: `NOTIFY_CMD_QUERY_CALLBACK 0x02` (L49),
  `NOTIFY_RESPONSE_MARKER 0x51` (L46), `RAW_REPORT_SIZE 32` (notifier.c:42).

➡️ **Therefore this PRP's deliverable is VERIFICATION + ALIGNMENT, not new code.** An
implementation agent that "implements the literal contract" by changing the index read
from `data[2]` to `data[1]` would **regress** QUERY_CALLBACK — it would read the `cmd_id`
byte as the index, breaking the entire callback-name sweep.

---

## 🚨 CRITICAL REGRESSION WARNING — do NOT implement the literal contract text verbatim

The work-item **contract text** is *imprecise* in one place where the committed code is
*correct*. **Align your UNDERSTANDING to the code; do NOT align the code to the contract text.**

| # | Literal contract (DO NOT implement verbatim) | Actual on `main` (CORRECT — keep) | Why the code is right |
|---|---|---|---|
| 1 | *"On QUERY_CALLBACK (0x02): read the index from `data[1]`"* — sounds like the index byte is at `data[1]` of the handler's `data` argument | `uint8_t index = (uint8_t)data[2];` (`notifier.c:673`) — the index is at **`data[2]`**, NOT `data[1]` | `hid_notify` strips the `[0x81][0x9F]` magic header (`data += 2`) before reassembling into `msg_buffer`, so the layout passed to `handle_typed_command` is `data[0]=0xF0` (discriminator), `data[1]=cmd_id`, `data[2]=first arg` (the index). This is documented at `notifier.c:640-642` and is **consistent with the QUERY_INFO handler** (which reads `cmd_id` from `data[1]`). Changing the code to read `data[1]` would read the `cmd_id` byte (0x02) as the index for EVERY query, breaking the name sweep. The contract's `data[1]` is indexing imprecision. |

**Other things you must NOT do:**
- Do NOT change the QUERY_CALLBACK handler, `send_typed_response`, or `handle_typed_command`
  signatures or bodies.
- Do NOT "simplify" the triple guard `cbs != NULL && index < cb_size && cbs[index].name != NULL`
  to just `index < cb_size`. The extra two clauses correctly handle the weak-default
  (`get_host_callbacks()` returns `NULL` when no `DEFINE_HOST_CALLBACKS`) and a
  defensively-NULL name. They are strictly safer and match the test expectations.
- Do NOT change the 29-byte name cap (`while (n < 29 ...)`). The payload is `[index][name]`
  and `send_typed_response` caps the payload at `RAW_REPORT_SIZE - 2 = 30` bytes, so the
  name gets at most 29 bytes. The 32-byte total response (`raw_hid_send(response, 32)`) is
  already guaranteed by `send_typed_response`'s zero-padding. This is correct.
- Do NOT attempt to "fix" the `0x03 == ETX` framing collision by special-casing
  `index == 3` in the handler. That is a **protocol/framing** issue (see Known Gotchas),
  out of scope, and the wrong layer.

---

## Goal

**Feature Goal**: Confirm the firmware's QUERY_CALLBACK handler satisfies every point of
the P1.M2.T2.S2 contract (and the authoritative PRD §4.6 wire / §14 firmware), that the
stub-compile of `notifier.c` is clean (no new warnings), and that the QUERY_CALLBACK-specific
host tests (valid-index name echo + out-of-range name-absent + side-effect-free) pass.
**No source change is expected** unless a genuine QUERY_CALLBACK-level defect is found
(none was found in research).

**Deliverable**: A verification report (inline in the implementation session) that maps
each contract point to its existing `notifier.c` location, shows the passing test evidence,
and records the single documented contract-vs-code delta (index read at `data[2]`, not
`data[1]` — *expected and correct*, not a defect). If (and only if) a real defect is found,
a minimal surgical fix in `notifier.c` that keeps the `send_typed_response` response-builder
architecture and the triple-guard intact.

**Success Definition**:
- Every row of the verification map (below) is satisfied by existing code (verified by read + grep).
- `notifier.c` stub-compiles with **exit 0, no new warnings** beyond the carried
  `-Wunused-function` set for static helpers.
- `test_notifier_host.c` (built from the stub-compiled `notifier.o`) shows the
  QUERY_CALLBACK groups — **(iii)** valid index (0 and 1) and **(iv)** out-of-range (2) —
  **100% pass** (the 7 known SET_OS-handler failures are out of scope — see Validation Level 3).
- `git diff` is **empty** at the end of the task (or, in the defect-fix case, a minimal,
  justified diff with all relevant gates still green).

## User Persona (if applicable)

**Target User**: The QMKonnect desktop host (P4.M2.T1.S1 handshake, planned) that runs a
`QUERY_CALLBACK` sweep over `i = 0 .. callback_count-1` to build a `name → id` map — but
**only iff** the preceding `QUERY_INFO` reported `response[0]==0x51 && proto_ver==2 &&
flags & 0x02` (CALLBACK_REGISTRY). (2) Every keymap author who omits
`DEFINE_HOST_CALLBACKS` and relies on the weak default reporting `callback_count=0` /
`feature_flags` bit 0x02 clear so the module behaves identically to today (no sweep).

**Use Case**: Host connects → `QUERY_INFO` (`0x81 0x9F 0xF0 0x01 … 0x03`) reports
`callback_count = N > 0` and `flags & 0x02`. Host then sweeps
`[0x81][0x9F][0xF0][0x02][i][0x03]` for `i = 0 .. N-1`. For each `i`, `hid_notify`
classifies the report (`data[2]==0xF0` → `typed_mode=true`), reassembles into `msg_buffer`,
and at ETX calls `handle_typed_command(msg_buffer)` → the QUERY_CALLBACK case reads
`index = data[2] = i`, looks up `get_host_callbacks()[i].name`, and replies
`[0x51][0x02][i][name bytes…][NUL padding]`. The host accumulates `name → i`. An out-of-range
`i` (host over-sweeps) yields `[0x51][0x02][i][0x00]` (name absent), which the host treats
as end-of-registry. Board state is untouched (typed path bypasses `process_full_message`).

**Pain Points Addressed**: Gives the host a deterministic name→id discovery over Raw HID
without reflashing, so cross-flash callback renumbering is harmless (the host re-queries names
on every reconnect). Names are stable per build (array index = id), decoupling the wire id
from the user-visible name.

## Why

- **Closes the QMKonnect-side tracking view** of a firmware feature already shipped in the
  firmware repo. The value this PRP adds is *preventing a regression*: an agent that takes
  the imprecise contract at face value would move the index read from `data[2]` to `data[1]`,
  reading the `cmd_id` byte (0x02) as the index on every query and breaking the name sweep.
- **Enforces board/host orthogonality at the handler level (PRD invariant 21):** QUERY_CALLBACK
  is a pure query — it reads the callback registry and replies, touching NEITHER the board
  `activated_layer`/command NOR the host `host_layer`/`host_cb_enabled[]`. The
  side-effect-free test assertion proves this.
- **Codifies the name-discovery contract (PRD §4.6):** `QUERY_CALLBACK [index] → [index][name,
  NUL-padded]`. The `0x51` response marker (≥2) is distinct from the legacy `0`/`1` match-bool,
  so the host disambiguates without ambiguity.

## What

Verify (do not rewrite) the following, all of which already exist in `notifier.c`:

### Success Criteria
- [ ] The QUERY_CALLBACK `case NOTIFY_CMD_QUERY_CALLBACK:` exists inside
      `handle_typed_command` (`notifier.c:672`).
- [ ] It reads `uint8_t index = (uint8_t)data[2];` (`notifier.c:673`) — **`data[2]`, not `data[1]`**.
- [ ] It reads `cb_size = get_host_callbacks_size();` and `cbs = get_host_callbacks();`
      (`notifier.c:674-675`).
- [ ] The valid-index branch guards `cbs != NULL && index < cb_size && cbs[index].name != NULL`
      (`notifier.c:676`).
- [ ] In the valid branch: `payload[0] = index;` then copies up to 29 name bytes into
      `payload[1 + n]` (`notifier.c:677-680`), and calls
      `send_typed_response(NOTIFY_CMD_QUERY_CALLBACK, payload, (uint8_t)(1 + n))` (`notifier.c:682`).
- [ ] The else branch builds `uint8_t payload[2] = { index, 0x00 };` and calls
      `send_typed_response(NOTIFY_CMD_QUERY_CALLBACK, payload, 2)` (`notifier.c:684-685`).
- [ ] `send_typed_response` (`notifier.c:628`) emits exactly 32 bytes: `response[0]=0x51`,
      `response[1]=cmd_id echo (0x02)`, `response[2]=payload[0]=index`, `response[3..]=name`
      (or `0x00` for OOB), zero-padded tail; calls `raw_hid_send(response, RAW_REPORT_SIZE)`.
- [ ] QUERY_CALLBACK touches NEITHER board state (`activated_layer`, command) NOR host state
      (`host_layer`, `host_cb_enabled[]`, `has_been_queried`).
- [ ] `get_host_callbacks()` is `__attribute__((weak))` returning `NULL` (`notifier.c:123`).
- [ ] `get_host_callbacks_size()` is `__attribute__((weak))` returning 0 (`notifier.c:124`).
- [ ] `notifier.c` stub-compiles with exit 0, no new warnings.
- [ ] `test_notifier_host.c` QUERY_CALLBACK groups (iii) valid + (iv) out-of-range pass.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge of this codebase can, using only this PRP + the
firmware repo, (a) confirm the QUERY_CALLBACK handler is present and correct, (b) build and
run the host test to see groups (iii)+(iv) pass, and (c) avoid regressing it — because the
contract-vs-code delta table, the code map, and the exact commands are all here.

### Documentation & References

```yaml
# MUST READ — PRD sections (QMKonnect plan/002 selected selectors; authoritative firmware
# detail is in the firmware repo's own PRD.md §4.6/§14).
- url: spec/PRD.md (heading h2.83 "Wire Protocol (typed commands)")
  why: "QUERY_CALLBACK command-table row: Request args [index], Response payload
        [index][name, NUL-padded]; response marker 0x51 (distinct from legacy 0/1);
        the host runs the QUERY_CALLBACK sweep iff QUERY_INFO reports
        response[0]==0x51 && proto_ver==2 && flags & 0x01 (then validates rules.toml)"
  critical: "Typed commands BYPASS process_full_message (no board side effects). Names are
        NUL-padded C strings. index == array position; the host sweeps 0..callback_count-1."

- url: spec/PRD.md (heading h2.84 "Firmware Spec (qmk-notifier)")
  why: "Named callback registry — DEFINE_HOST_CALLBACKS({ … }) + weak-default accessors
        (get_host_callbacks / _size). ID = array index, stable per build; re-queried by name
        on every reconnect. Bounded by HOST_CALLBACK_MAX (static array). host_callback_t
        typedef; QUERY_CALLBACK/QUERY_INFO answerable before any string seen."

# MUST READ — existing firmware source (the thing being verified)
- file: /home/dustin/projects/qmk-notifier/notifier.c
  why: the QUERY_CALLBACK handler + its dependencies
  pattern: "L42 RAW_REPORT_SIZE=32; L123-124 weak accessors; L628-637 send_typed_response
           (the [0x51] builder, caps payload at 30 bytes); L640-642 msg_buffer layout doc
           (data[0]=0xF0, data[1]=cmd_id, data[2..]=args); L651 handle_typed_command;
           L672-687 QUERY_CALLBACK case (the deliverable); L748-825 hid_notify (magic-strip +
           ETX byte loop + typed routing fork)"
  gotcha: "the index is at data[2], NOT data[1]. hid_notify does data+=2 to strip the
           [0x81][0x9F] magic header BEFORE reassembling into msg_buffer, so within
           handle_typed_command: data[0]=0xF0, data[1]=cmd_id, data[2]=first arg. The
           contract text's 'data[1]' is imprecise; the code's data[2] is correct."

- file: /home/dustin/projects/qmk-notifier/notifier.h
  why: "all NOTIFY_* constants + host_callback_t typedef + DEFINE_HOST_CALLBACKS macro"
  pattern: "L5 callback_t = void(*)(void); L16-21 host_callback_t {name,on_enable,on_disable};
           L33 get_host_callbacks / _size decls; L46 NOTIFY_RESPONSE_MARKER 0x51;
           L49 NOTIFY_CMD_QUERY_CALLBACK 0x02; L60 HOST_CALLBACK_MAX 32;
           L69-73 DEFINE_HOST_CALLBACKS macro (overrides the weak accessors)"
  gotcha: "constants ALREADY exist — do not re-add or renumber (0x04 is reserved for VIA)."

# MUST READ — the preceding (CONTRACT) PRPs whose outputs this handler consumes
- file: plan/002_637d65b6e9b8/P1M2T1S1/PRP.md
  why: "verified the typed_mode fork + handle_typed_command dispatch skeleton + send_typed_response
        that this QUERY_CALLBACK case lives inside"
  critical: "handle_typed_command signature is 'static bool handle_typed_command(char *data)';
        it switches on (uint8_t)data[1]; the [0x51] response is sent INSIDE it; hid_notify's
        typed_dispatched suppresses the legacy ack. All LANDED — do not re-add."

- file: plan/002_637d65b6e9b8/P1M2T2S1/PRP.md
  why: "the immediately-preceding sibling (QUERY_INFO). Same verify-and-align nature. Confirms
        send_typed_response prepends [0x51][cmd_id] so payload[0] == wire response[2]; documents
        the shared contract-vs-code delta pattern (contract text imprecise, code correct)."
  critical: "QUERY_CALLBACK (notifier.c:672) is the structural sibling of QUERY_INFO
        (notifier.c:658). Both live in the same switch; both use send_typed_response; both are
        side-effect-free pure queries. The msg_buffer layout (data[0]=0xF0, data[1]=cmd_id,
        data[2..]=args) is authoritative here too."

# Reference — existing tests
- file: /home/dustin/projects/qmk-notifier/test_notifier_host.c   # 64-test typed suite; (iii) + (iv) cover QUERY_CALLBACK
- file: /home/dustin/projects/qmk-notifier/test_notifier_dispatch.c # 14-test legacy/dispatch regression
- file: /home/dustin/projects/qmk-notifier/test_notifier_os.c       # 31-test multi-OS regression
- file: /home/dustin/projects/qmk-notifier/run_notifier_stub_tests.sh # committed gate (dispatch + os + host)
- file: /home/dustin/projects/qmk-notifier/qmk_stubs/              # host-compile stubs (QMK_KEYBOARD_H, raw_hid_send capture, layer state)
```

### Current Codebase tree (firmware repo, verification-relevant only)

```bash
# run from /home/dustin/projects/qmk-notifier
notifier.h            # NOTIFY_* constants, callback_t, host_callback_t, DEFINE_HOST_CALLBACKS, HOST_CALLBACK_MAX
notifier.c            # QUERY_CALLBACK case (L672), send_typed_response (L628), weak accessors (L123-124),
                      # RAW_REPORT_SIZE=32 (L42), handle_typed_command (L651), hid_notify (L748)
qmk_stubs/            # host-compile stubs (QMK_KEYBOARD_H, raw_hid_send capture, layer_on/off, os_detection.h)
test_notifier_dispatch.c   # 14-test legacy/dispatch regression suite
test_notifier_os.c         # 31-test multi-OS regression suite
test_notifier_host.c       # 64-test typed-command suite (QUERY_CALLBACK = cases (iii) valid + (iv) out-of-range)
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
// CRITICAL: the index is read from data[2], NOT data[1]. hid_notify does data += 2 to strip
// the [0x81][0x9F] magic header BEFORE reassembling into msg_buffer. So inside
// handle_typed_command the layout is: data[0]==0xF0 (discriminator), data[1]==cmd_id,
// data[2..]==args. The contract text's "read the index from data[1]" is IMPRECISE. The
// committed code reads data[2] (notifier.c:673) and is CORRECT — do NOT change it to data[1]
// (that would read the cmd_id byte 0x02 as the index, breaking every query).
//
// CRITICAL: the 0x03 == ETX framing collision is a PRE-EXISTING protocol issue, NOT a
// QUERY_CALLBACK handler defect. hid_notify's byte loop treats ANY payload byte == 0x03 as
// the message terminator. It blocks SET_OS (cmd_id 0x03 — P1.M2.T2.S3's scope) and would
// misread QUERY_CALLBACK index == 3 (the index byte 0x03 terminates early → index read as 0).
// Fixing this requires a framing-level escape change — OUT OF SCOPE for this handler task.
// Do NOT special-case index == 3 in the handler (wrong layer; diverges from sibling cases).
//
// GOTCHA: the triple guard 'cbs != NULL && index < cb_size && cbs[index].name != NULL' is
// intentionally more defensive than the bare 'index < cb_size' the contract implies. The
// cbs != NULL clause handles the weak default (get_host_callbacks() returns NULL when no
// DEFINE_HOST_CALLBACKS); the name != NULL clause handles a defensively-NULL name. Both are
// strictly safer and match the test expectations. Do NOT "simplify" to a single bounds check.
//
// GOTCHA: the name is capped at 29 bytes (payload[30] = 1 index byte + 29 name bytes;
// send_typed_response caps the payload at RAW_REPORT_SIZE-2 = 30). The 32-byte total response
// (raw_hid_send(response, 32)) is guaranteed by send_typed_response's zero-padding. The
// contract's "NUL-padded to fill 32 bytes" refers to the TOTAL response; the name occupies
// response[3..31] (29 bytes). Correct — do NOT raise the cap.
//
// GOTCHA: QUERY_CALLBACK is side-effect-free on BOTH board and host state. It reads the
// callback registry and replies. It must NOT call layer_on/off, activate_layer/
// deactivate_layer, set_host_layer, apply_host_callbacks, enable_command/disable_command,
// or set has_been_queried. The side-effect-free test assertion proves this.
//
// GOTCHA: get_host_callbacks() is __attribute__((weak)) returning NULL; get_host_callbacks_size()
// is weak returning 0. A keymap that omits DEFINE_HOST_CALLBACKS links and behaves identically
// to today: QUERY_INFO reports callback_count=0 and feature_flags bit 0x02 clear, so the host
// never runs the QUERY_CALLBACK sweep. The DEFINE_HOST_CALLBACKS macro overrides both (strong).
//
// GOTCHA: handle_typed_command is called by hid_notify ('match = handle_typed_command(msg_buffer);
// typed_dispatched = true;'). The [0x51] response is sent INSIDE handle_typed_command; the
// bool return is vestigial for the typed path (the legacy ack is suppressed).
```

## Implementation Blueprint

### Verification map — contract point → existing code (NO new data models; C firmware)

| # | Contract point | Existing location | Verified? |
|---|---|---|---|
| 1 | `host_callback_t { name, on_enable, on_disable }` | `notifier.h:16-21` (`callback_t` at L5) | ☐ |
| 2 | `get_host_callbacks()` returns the array (weak → NULL) | `notifier.c:123`; strong via macro `notifier.h:70-72` | ☐ |
| 3 | `get_host_callbacks_size()` returns count (weak → 0) | `notifier.c:124`; strong via macro `notifier.h:73` | ☐ |
| 4 | `QUERY_CALLBACK` `case` inside `handle_typed_command`, switch on cmd_id | `notifier.c:672` (`case NOTIFY_CMD_QUERY_CALLBACK:`); `cmd_id=(uint8_t)data[1]` L652 | ☐ |
| 5 | read the index from the command args | `notifier.c:673` `index=(uint8_t)data[2]` — **DELTA: data[2] not data[1]** (CORRECT per msg_buffer layout) | ☐ |
| 6 | if index >= size → `[0x51][0x02][index][0x00]` (name absent) | `notifier.c:676` guard `cbs!=NULL && index<cb_size && name!=NULL`; else branch `notifier.c:684-685` `payload[2]={index,0x00}` | ☐ |
| 7 | else → copy name string NUL-padded | `notifier.c:677-681` (`payload[0]=index`; copy up to 29 name bytes into `payload[1+n]`) | ☐ |
| 8 | `response[0]=0x51` (NOTIFY_RESPONSE_MARKER) | `notifier.c:630` (send_typed_response) | ☐ |
| 9 | `response[1]=NOTIFY_CMD_QUERY_CALLBACK` (0x02) cmd echo | `notifier.c:631` (send_typed_response echoes cmd_id) | ☐ |
| 10 | `response[2]=index`, `response[3..]=name` | payload[0]=index→response[2]; payload[1..]=name→response[3..] (`notifier.c:633-635` memcpy) | ☐ |
| 11 | `raw_hid_send(response, RAW_REPORT_SIZE)` (32 bytes, zero-padded) | `notifier.c:637` (send_typed_response) | ☐ |
| 12 | Names are C strings (NUL-terminated) | `notifier.c:680` `while (n<29 && name[n]!='\0')` | ☐ |
| 13 | Constants: RESPONSE_MARKER=0x51, QUERY_CALLBACK=0x02, RAW_REPORT_SIZE=32 | `notifier.h:46,49`; `notifier.c:42` | ☐ |
| 14 | QUERY_CALLBACK side-effect-free: no board/host state mutation | L672-687 (no layer/cmd/host refs); test assertion cb_*_en==0 | ☐ |

### Implementation Tasks (verification-ordered)

```yaml
Task 1: ESTABLISH baseline (no edits)
  - RUN: cd /home/dustin/projects/qmk-notifier && git status -s && git log --oneline -3
  - EXPECT: clean tree, HEAD at or past 70fcfa1
  - READ: notifier.c L42 (RAW_REPORT_SIZE), L123-124 (weak accessors), L628-637
          (send_typed_response), L640-652 (msg_buffer layout doc + handle_typed_command
          head), L672-687 (QUERY_CALLBACK case — the deliverable); notifier.h L5,16-21
          (callback_t + host_callback_t), L46,49 (constants), L69-73 (DEFINE_HOST_CALLBACKS)
  - CONFIRM every row of the Verification map above (check the ☐ boxes in your report)
  - NOTE the single documented contract-vs-code delta (index read at data[2], not data[1])
    is CORRECT, not a defect — record it as "verified correct, kept as-is"

Task 2: RUN the committed stub-compile gate (NOTE its current overall-fail status)
  - RUN: ./run_notifier_stub_tests.sh
  - EXPECT (current baseline, accurate as of HEAD 70fcfa1):
      * dispatch fails=0  (exit=0)   ✓
      * os fails=0        (exit=0)   ✓
      * host fails=7      (exit=1)   ✗  — ALL 7 are SET_OS handler (0x03==ETX blocker),
                                         OUT OF SCOPE for QUERY_CALLBACK
      * final line: "✗ notifier stub-compile gate FAILED" (exit 1)
  - CRITICAL FRAMING: the gate FAILS overall, but NOT because of QUERY_CALLBACK.
    QUERY_CALLBACK contributes ZERO failures. The 7 host failures are the documented
    SET_OS blocker (P1.M2.T2.S3's scope). The gate will not go green until SET_OS is fixed.
  - IF dispatch or os show ANY failure: that indicates a REAL regression worth investigating
    (not a contract-literal mismatch). Diagnose root cause before any edit. Pre-existing
    -Wunused-function warnings are EXPECTED — do not silence them.

Task 3: BUILD+RUN the typed-command host suite; isolate QUERY_CALLBACK groups
  - RUN:
      gcc -Wall -Wextra -std=c99 -DQMK_KEYBOARD_H='"qmk_keyboard_stub.h"' -Iqmk_stubs -I. \
          -c notifier.c -o /tmp/nh.o
      gcc -Wall -std=c99 -Iqmk_stubs -I. /tmp/nh.o qmk_stubs/qmk_stubs.c test_notifier_host.c \
          -o /tmp/test_notifier_host
      /tmp/test_notifier_host 2>&1 | grep -iE 'QUERY_CALLBACK|callback|read-only'
  - EXPECT: 64 run / 57 pass / 7 fail. The 7 failures are ALL SET_OS — OUT OF SCOPE.
    QUERY_CALLBACK-relevant groups that MUST be 100% pass:
      (iii)  QUERY_CALLBACK valid index 0 and 1:
             r[0]=0x51, r[1]=0x02, r[2]=index(0/1), r[3..]='mute'/'layout',
             name NUL-padded after the name   [test_notifier_host.c:142-158]
      (iv)   QUERY_CALLBACK out-of-range index 2:
             r[0]=0x51, r[1]=0x02, r[2]=2, r[3]=0x00 (name absent)   [test_notifier_host.c:161-169]
      side-effect-free: "QUERY_INFO/QUERY_CALLBACK fired no host callback (read-only
             queries)" PASS   [test_notifier_host.c:175]
  - SCOPE RULE: do NOT attempt to fix the 7 SET_OS failures here. They are tracked by the
    firmware's plan/003 P1.M3.T2.S1 (Researching) and belong to QMKonnect P1.M2.T2.S3.

Task 4: (DEFAULT) NO-OP — write the verification report, leave source untouched
  - IF Tasks 1-3 are green for QUERY_CALLBACK (expected path): the deliverable is the
    inline report. git diff stays empty. Done. The overall gate failure is the SET_OS
    blocker (out of scope) — document this clearly so it is not mistaken for a
    QUERY_CALLBACK regression.
  - IF a genuine QUERY_CALLBACK-level defect is found (unexpected): make the MINIMAL
    surgical fix in notifier.c that preserves the send_typed_response response-builder
    architecture and the triple guard, then re-run Tasks 2 & 3 to confirm no NEW failures
    beyond the known 7 SET_OS. Document the defect, the fix, and the before/after test
    counts in the report.

Task 5: NEVER do these
  - DO NOT change the index read from data[2] to data[1] — data[2] is correct (the
    [0x81][0x9F] magic header is stripped before reassembly; data[0]=0xF0, data[1]=cmd_id,
    data[2]=first arg). data[1] is the cmd_id byte; reading it as the index breaks every query.
  - DO NOT "simplify" the triple guard (cbs!=NULL && index<cb_size && name!=NULL) to a
    single bounds check — the extra clauses handle the weak-default NULL registry and
    defensively-NULL names; they are correct and tested.
  - DO NOT raise the 29-byte name cap. send_typed_response caps the payload at 30 bytes
    (RAW_REPORT_SIZE-2); [index][name] leaves 29 bytes for the name. The 32-byte total
    response is guaranteed by zero-padding. Correct.
  - DO NOT special-case index == 3 in the handler. The 0x03==ETX framing collision is a
    protocol-level issue (same family as the SET_OS blocker); the fix belongs in the
    framing layer, not this handler. Out of scope.
  - DO NOT make QUERY_CALLBACK touch board state (activated_layer, command) or host state
    (host_layer, host_cb_enabled[], has_been_queried) — it is a pure query.
  - DO NOT change the QUERY_CALLBACK handler, send_typed_response, or handle_typed_command
    signatures or bodies.
  - DO NOT renumber NOTIFY_* constants (0x04 stays reserved for VIA).
  - DO NOT silence pre-existing -Wunused-function stub-compile warnings.
  - DO NOT edit PRD.md, any tasks.json, or any plan/ files (read-only).
```

### Implementation Patterns & Key Details
```c
// The existing (correct) QUERY_CALLBACK handler — notifier.c:672-687:
//   case NOTIFY_CMD_QUERY_CALLBACK: {
//       uint8_t index = (uint8_t)data[2];          /* ARG[0] — data[2], NOT data[1] */
//       size_t cb_size = get_host_callbacks_size();
//       host_callback_t *cbs = get_host_callbacks();
//       if (cbs != NULL && index < cb_size && cbs[index].name != NULL) {
//           uint8_t payload[30];                   /* [index] + up to 29 name bytes */
//           payload[0] = index;
//           const char *name = cbs[index].name;
//           uint8_t n = 0;
//           while (n < 29 && name[n] != '\0') { payload[1 + n] = (uint8_t)name[n]; n++; }
//           send_typed_response(NOTIFY_CMD_QUERY_CALLBACK, payload, (uint8_t)(1 + n));
//       } else {
//           uint8_t payload[2] = { index, 0x00 };  /* name absent (§4.6) */
//           send_typed_response(NOTIFY_CMD_QUERY_CALLBACK, payload, 2);
//       }
//       break;
//   }

// The existing (correct) response builder — notifier.c:628-637 (send_typed_response):
//   static void send_typed_response(uint8_t cmd_id, const uint8_t *payload, uint8_t payload_len) {
//       uint8_t response[RAW_REPORT_SIZE] = {0};   /* zero-pads the unused tail */
//       response[0] = NOTIFY_RESPONSE_MARKER;      /* 0x51 */
//       response[1] = cmd_id;                      /* echo (0x02) */
//       if (payload != NULL && payload_len > 0) {
//           uint8_t cap = (uint8_t)(RAW_REPORT_SIZE - 2);   /* 30 bytes after [0x51][cmd_id] */
//           uint8_t n = (payload_len < cap) ? payload_len : cap;
//           memcpy(response + 2, payload, n);
//       }
//       raw_hid_send(response, RAW_REPORT_SIZE);
//   }
//   // => wire bytes for valid index: [0]=0x51 [1]=0x02 [2]=index [3..]=name [tail]=0
//   // => wire bytes for OOB index:  [0]=0x51 [1]=0x02 [2]=index [3]=0x00 [tail]=0

// The magic-strip that FIXES the data[2] layout — notifier.c:762-763 (hid_notify):
//   // Strip off those 2 identifying characters [0x81][0x9F]
//   data += 2;
//   length -= 2;
//   // ... then the byte loop appends remaining bytes (0xF0, cmd_id, args...) to msg_buffer
//   // so msg_buffer[0]=0xF0, msg_buffer[1]=cmd_id, msg_buffer[2]=args[0]=index.
```

### Integration Points
```yaml
DEPENDENCIES: none — this is self-contained firmware C; it does NOT depend on the
              qmk_notifier Rust crate or its v0.3.0 tag (P1.M1.T4.S1).
              It depends on P1.M2.T1.S1 (the typed-dispatch skeleton + send_typed_response)
              which ALREADY exists and is verified.
DOWNSTREAM (consumers — ALL ALREADY PRESENT or PLANNED):
  - P4.M2.T1.S1 desktop handshake (QMKonnect, planned) — runs the QUERY_CALLBACK sweep
    iff QUERY_INFO reports flags&0x02 && proto==2; consumes [0x51][0x02][index][name]
    to build name→id and validate rules.toml.
  - The weak default (no DEFINE_HOST_CALLBACKS) => QUERY_INFO reports callback_count=0
    and feature_flags bit 0x02 clear, so the host NEVER runs the sweep — module behaves
    identically to today.
HOST (desktop) handshake that drives this — P4.M2.T1 (planned): sends the sweep after QUERY_INFO.
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

# Confirm the QUERY_CALLBACK case + its dependencies are present and correct:
grep -n 'case NOTIFY_CMD_QUERY_CALLBACK' notifier.c          # expect one line ~672
grep -n 'uint8_t index = (uint8_t)data\[2\]' notifier.c      # expect one line ~673 (NOT data[1])
grep -n 'get_host_callbacks_size()' notifier.c               # expect weak decl + QUERY_CALLBACK use
grep -n 'payload\[2\] = { index, 0x00 }' notifier.c          # expect the OOB branch ~685
grep -n 'while (n < 29 && name\[n\]' notifier.c              # expect the name-copy loop ~680
rm -f /tmp/notifier_stub.o
```

### Level 2: The committed regression suites (Component Validation)
```bash
cd /home/dustin/projects/qmk-notifier
./run_notifier_stub_tests.sh
# Expected (ACCURATE as of HEAD 70fcfa1 — the host suite is now integrated into the runner):
#   notifier dispatch fails=0  (exit=0)   ✓
#   notifier os fails=0        (exit=0)   ✓
#   notifier host fails=7      (exit=1)   ✗  — ALL 7 are SET_OS (0x03==ETX blocker), OUT OF SCOPE
#   final line: "✗ notifier stub-compile gate FAILED" (exit 1)
# CRITICAL: the gate FAILS overall because of the 7 SET_OS failures — NOT because of QUERY_CALLBACK.
# dispatch 14/14 and os 31/31 MUST be green (they prove no regression in the reassembler,
# matcher, F4/F5/F8/F9 logic, or the hid_notify routing fork). If either gains a failure,
# STOP — that is a real regression, not a contract-literal mismatch.
```

### Level 3: Typed-command host suite (QUERY_CALLBACK-scope validation)
```bash
cd /home/dustin/projects/qmk-notifier
gcc -Wall -Wextra -std=c99 -DQMK_KEYBOARD_H='"qmk_keyboard_stub.h"' -Iqmk_stubs -I. \
    -c notifier.c -o /tmp/nh.o
gcc -Wall -std=c99 -Iqmk_stubs -I. /tmp/nh.o qmk_stubs/qmk_stubs.c test_notifier_host.c \
    -o /tmp/test_notifier_host
/tmp/test_notifier_host 2>&1 | grep -iE 'QUERY_CALLBACK|read-only'
# Expected: every QUERY_CALLBACK line begins with "PASS". Specifically:
#   (iii) QUERY_CALLBACK(0) r[0]=0x51, r[1]=0x02, r[2]=0, r[3..]='mute', NUL-padded
#   (iii) QUERY_CALLBACK(1) r[0]=0x51, r[1]=0x02, r[2]=1, r[3..]='layout', NUL-padded
#   (iv)  QUERY_CALLBACK(OOB) r[0]=0x51, r[1]=0x02, r[2]=2, r[3]=0x00 (name absent)
#   "QUERY_INFO/QUERY_CALLBACK fired no host callback (read-only queries)" PASS
# Suite total: 64 run / 57 pass / 7 fail. The 7 are ALL SET_OS — OUT OF SCOPE (P1.M2.T2.S3).
rm -f /tmp/nh.o /tmp/test_notifier_host
```

### Level 4: Legacy-path regression (no QUERY_CALLBACK handler touches it)
```bash
cd /home/dustin/projects/qmk-notifier
./run_all_tests.sh
# Expected: the 9-suite pattern_match corpus is unaffected (legacy string path is
# byte-identical; QUERY_CALLBACK is only reachable via the typed 0xF0 path). All suites pass.
```

## Final Validation Checklist

### Technical Validation
- [ ] Verification map (14 rows) fully satisfied by existing code.
- [ ] `notifier.c` stub-compile → exit 0, no new warnings beyond the carried `-Wunused-function` set.
- [ ] `./run_notifier_stub_tests.sh` → dispatch fails=0, os fails=0 (the 7 host SET_OS failures
      are explicitly out of scope; the overall-gate FAIL is NOT a QUERY_CALLBACK regression).
- [ ] `test_notifier_host.c` QUERY_CALLBACK groups ((iii) valid, (iv) out-of-range,
      side-effect-free) 100% pass.
- [ ] `./run_all_tests.sh` — 9-suite pattern corpus unaffected.

### Feature Validation
- [ ] QUERY_CALLBACK(valid i) returns `[0x51][0x02][i][name bytes…][NUL padding]`.
- [ ] QUERY_CALLBACK(out-of-range i) returns `[0x51][0x02][i][0x00]` (name absent).
- [ ] Index is read from `data[2]` (NOT `data[1]`) — the magic header is stripped before reassembly.
- [ ] Name capped at 29 bytes; total response is 32 bytes (zero-padded by send_typed_response).
- [ ] Triple guard intact: `cbs != NULL && index < cb_size && cbs[index].name != NULL`.
- [ ] QUERY_CALLBACK is side-effect-free: board `activated_layer`/command AND host
      `host_layer`/`host_cb_enabled[]`/`has_been_queried` all unchanged.

### Code Quality Validation
- [ ] `git diff` is EMPTY (expected default) OR a minimal, justified defect-fix with no new
      failures beyond the known 7 SET_OS.
- [ ] No re-implementation of the (regressive) literal contract (index read from data[1];
      single bounds check; raised name cap).
- [ ] QUERY_CALLBACK handler consumes NOTIFY_* constants by name (no hardcoded 0x51/0x02 literals).
- [ ] No renumbering of NOTIFY_CMD_* constants.

### Documentation & Deployment
- [ ] No user-facing docs required (firmware C code — per the work-item DOCS: none).
- [ ] Verification report recorded inline (contract map + test counts + the single documented
      delta: index at data[2] not data[1]; overall-gate-fail attributed to the SET_OS blocker).

---

## Anti-Patterns to Avoid
- ❌ Do NOT change the index read from `data[2]` to `data[1]` — the magic header `[0x81][0x9F]`
  is stripped before reassembly, so inside `handle_typed_command` the layout is
  `data[0]=0xF0, data[1]=cmd_id, data[2]=first arg`. `data[1]` is the `cmd_id` byte (0x02);
  reading it as the index breaks every query. The contract text's `data[1]` is imprecise.
- ❌ Do NOT "simplify" the triple guard (`cbs != NULL && index < cb_size && name != NULL`) to a
  single bounds check — the extra clauses handle the weak-default NULL registry and NULL names.
- ❌ Do NOT raise the 29-byte name cap — `send_typed_response` caps the payload at 30 bytes; the
  32-byte total response is guaranteed by zero-padding.
- ❌ Do NOT special-case `index == 3` in the handler — the `0x03 == ETX` framing collision is a
  protocol-level issue (same family as the SET_OS blocker); the fix belongs in the framing layer.
- ❌ Do NOT make QUERY_CALLBACK mutate board or host state — it is a pure query.
- ❌ Do NOT rewrite the QUERY_CALLBACK handler, `send_typed_response`, or `handle_typed_command` —
  they exist and are tested.
- ❌ Do NOT try to fix the 7 SET_OS handler failures here — out of scope (P1.M2.T2.S3; firmware
  plan/003 P1.M3.T2.S1).
- ❌ Do NOT silence pre-existing `-Wunused-function` stub-compile warnings.
- ❌ Do NOT mistake the overall-gate FAIL (caused by the 7 SET_OS failures) for a QUERY_CALLBACK
  regression — QUERY_CALLBACK contributes zero failures.
- ❌ Do NOT edit PRD.md, tasks.json, prd_snapshot.md, or any plan/ file.
- ❌ Do NOT assume this task must produce a diff — the expected, correct outcome is a verification
  report with an empty diff.

---

## Confidence Score: 9/10

The deliverable is already present, correct, and green at its scope (verified this session:
`notifier.c` stub-compiles clean, the QUERY_CALLBACK handler is present at notifier.c:672-687
with the index correctly read from `data[2]`, the triple guard intact, and `test_notifier_host.c`
groups (iii) valid + (iv) out-of-range + side-effect-free all PASS). The 1-point reservation is
for the (unlikely) discovery of a genuine QUERY_CALLBACK-level defect during the implementation
agent's own verification pass; if found, the minimal-fix path is specified. The dominant risk this
PRP neutralizes is a regression from naively implementing the imprecise contract text (reading the
index from `data[1]` instead of `data[2]`, which would read the `cmd_id` byte as the index and
break the entire callback-name sweep). The secondary risk — mistaking the overall-gate FAIL (the
7 out-of-scope SET_OS failures) for a QUERY_CALLBACK defect — is explicitly called out in
Validation Level 2.
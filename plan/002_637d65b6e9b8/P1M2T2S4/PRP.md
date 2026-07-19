# PRP — P1.M2.T2.S4: APPLY_HOST_CONTEXT handler: clear_board, set_host_layer, apply_host_callbacks

> **Repo under change:** the **qmk-notifier FIRMWARE** (C) at
> `/home/dustin/projects/qmk-notifier` — remote `git@github.com:dabstractor/qmk-notifier`,
> branch `main`, HEAD **`8441af2`** ("Implement length-aware typed command reassembly").
> This is **NOT** the `qmk_notifier` Rust crate (P1.M1) and does **not** consume the
> v0.3.0 crate tag. Firmware ↔ crate are independent layers. It is the **same repo** as
> P1.M2.T1.S1/S2 and P1.M2.T2.S1/S2/S3; the APPLY_HOST_CONTEXT handler is the fourth and
> final `case` inside the same `handle_typed_command` switch those tasks verified. It is the
> **sole caller** of the `set_host_layer` / `apply_host_callbacks` helpers verified in
> P1.M2.T1.S2.

---

## ⚠️ READ FIRST — this is a VERIFY & ALIGN task, NOT greenfield

**HEADLINE FINDING (research-confirmed this session):** The firmware repo **already contains
a complete, committed, tested implementation** of the APPLY_HOST_CONTEXT (`0x05`) handler
**AND the multi-report framing reassembly (the contract's REVISE point)** that makes it
reachable and lets it span reports. The handler landed in commit `c5ad578` ("Implement typed
command dispatch and response builder"); the framing fix landed in commit `8441af2`
("Implement length-aware typed command reassembly") — the SAME commit that made the sibling
SET_OS handler (P1.M2.T2.S3) dispatch. The QMKonnect `plan/002` P1.M2 milestone is a
desktop-side **mirror/coordination view** with a finer subtask split; the firmware repo's own
`plan/003_16d737de7a3e` documented this handler as **P1M2T2S3 (Complete)**. This is the
**sixth consecutive** P1.M2 item to be verify-and-align.

Evidence (verified by read + grep + test run against clean HEAD `8441af2` this session):
- The APPLY_HOST_CONTEXT `case` is present at `notifier.c:767` inside `handle_typed_command`.
- It reads `layer=data[2], flags=data[3], count=data[4]` (L785-787), honors `clear_board`
  (`flags & 0x01` ⇒ `deactivate_layer()` + `disable_command()` first, L794-796), then calls
  `set_host_layer(layer)` (L798) + `apply_host_callbacks(ids, count)` (L799), and replies
  `[0x51][0x05][0x01]` via `send_typed_response(NOTIFY_CMD_APPLY_HOST_CONTEXT, payload, 1)`
  (L801-802).
- It has a length guard `if (len < 5)` (BUG-3 hardening) and a defense-in-depth `count`
  clamp to `min(MSG_BUFFER_SIZE-5, len-5)` (L789-792).
- The **contract's REVISE point** ("typed commands MUST be ETX-framed and multi-report
  reassembled; 0xF0 check should happen AFTER ETX reassembly") is **already satisfied
  semantically** by the `typed_mode` (first-report classification) + `typed_literal_remaining`
  (length-aware reassembly) architecture — see the Critical Regression Warning. The variable-
  length ids-tail accounting (`msg_index==5` → read `count` → add `count` literal bytes,
  L921-926) is what **removes the old ≤26-callbacks-per-report cap** and lets AHC span reports.
- `test_notifier_host.c` AHC coverage is **100% PASS**: (v) stack, (vi) replace, (vii)
  disable-before-enable diff ordering (proven via `g_seq` stamps), (viii) layer=0xFF clear,
  and (multi-rep) two-report 28-id reassembly.
- `./run_notifier_stub_tests.sh` → **dispatch 14/14, os 31/31, host 64/64, gate PASSED**.
- Stub-compile is **exit 0 with ZERO warnings** at this HEAD.

➡️ **Therefore this PRP's deliverable is VERIFICATION + ALIGNMENT, not new code.** An
implementation agent that "implements the literal contract" would **regress** the firmware
in three distinct ways (see the table below): most dangerously by moving the `0xF0`
discriminator check to after-ETX (which would silently break the length-aware reassembly
that makes AHC — and the sibling SET_OS — dispatch at all).

---

## 🚨 CRITICAL REGRESSION WARNING — do NOT implement the literal contract text verbatim

The work-item **contract text** is *naive* in THREE places where the committed code is
*correct*. **Align your UNDERSTANDING to the code; do NOT align the code to the contract text.**

| # | Literal contract (DO NOT implement verbatim) | Actual on `main` HEAD `8441af2` (CORRECT — keep) | Why the code is right |
|---|---|---|---|
| 1 | *"In hid_notify(), after ETX triggers msg_buffer dispatch, check if msg_buffer[0]==0xF0 before calling process_full_message. If 0xF0: route to handle_typed_command(msg_buffer, msg_index)."* | The `0xF0` discriminator is classified on the **first report** into a persistent `typed_mode` flag (`notifier.c:835`: `if (msg_index==0 && length>=3 && data[2]==NOTIFY_CMD_DISCRIMINATOR) { typed_mode=true; typed_literal_remaining=2; }`), and at ETX `if (typed_mode) { handle_typed_command(msg_buffer, msg_index); }` (L868). | The length-aware reassembly (`typed_literal_remaining`) can only consume binary args **literally** if the byte loop already *knows* it is inside a typed command. That knowledge must exist **during** the byte loop, not deferred to the ETX boundary — by ETX the loop has already wrongly terminated early on the first `0x03` inside the payload (BUG-1 SET_OS cmd_id `0x03==ETX`; BUG-2 any AHC arg of value `0x03`). The `typed_mode` flag is the durable form of "msg_buffer[0]==0xF0" carried from the first report. **Moving the check after ETX re-introduces BUG-1/BUG-2 AND breaks multi-report classification** (continuation reports carry payload at `data[2]`, which may be `0xF0`). |
| 2 | *"Multi-report: the full message is reassembled in hid_notify's msg_buffer before dispatch — but wait, typed commands are NOT reassembled through msg_buffer (they bypass the ETX reassembly)."* (the contract's own self-correction) — implies AHC might still be single-report | Typed commands ARE reassembled through `msg_buffer` across reports, ETX-framed, exactly like strings (PRD §5). The variable-length ids-tail accounting (`notifier.c:921-926`: at `msg_index==5` read `count=msg_buffer[4]`, add `count` more literal bytes, clamped to `MSG_BUFFER_SIZE-1` room) consumes the full ids tail across reports. The handler reads `ids=&data[5]` and clamps `count` to `min(MSG_BUFFER_SIZE-5, len-5)` (max **251 ids**). | This **removes the old ≤26-callbacks-per-report cap** (PRD §5: *"APPLY_HOST_CONTEXT may span reports"*). The (multi-rep) host test proves 28 ids across two reports reassemble and dispatch. A single-report handler would silently truncate any `count>25`. |
| 3 | *"clear_board flag (data bit 0)"* + *"deactivate_layer() + disable_command() (notifier.c:~225-260)"* — contract line numbers are STALE and the order is unspecified | `clear_board` is `flags & 0x01` where `flags=data[3]` (L786, L794). The board teardown is `deactivate_layer()` (L267) THEN `disable_command()` (L373) — actual line numbers, not the contract's `~225-260`. Order: turn off the active board layer, then disable the board command. | The contract's `notifier.c:~225-260` is a pre-implementation snapshot (the real `deactivate_layer` is L267, `disable_command` is L373). The committed order (layer then command) is harmless and matches the board's own teardown sequence used elsewhere. |

**Other things you must NOT do:**
- Do NOT change the AHC handler, `send_typed_response`, `handle_typed_command`,
  `set_host_layer`, `apply_host_callbacks`, `deactivate_layer`, or `disable_command`
  signatures or bodies.
- Do NOT strip the `count` clamp (`min(MSG_BUFFER_SIZE-5, len-5)`) — it is BUG-3 hardening +
  defense-in-depth against a malformed `0xFF` count.
- Do NOT change the ack byte from `0x01`. `ack=1` (applied); `0x00` is reserved for a future
  NACK and must NOT be sent here.
- Do NOT make the AHC handler touch `has_been_queried` or `current_os` — those belong to
  QUERY_INFO and SET_OS respectively. AHC touches board state (via `deactivate_layer`/
  `disable_command` when `clear_board`) and host state (via `set_host_layer`/
  `apply_host_callbacks`) only.
- Do NOT revert or weaken commit `8441af2` (the `typed_literal_remaining` framing fix). It
  is the ONLY thing that makes AHC (and SET_OS) dispatch when an arg byte is `0x03`.

---

## Goal

**Feature Goal**: Confirm the firmware's APPLY_HOST_CONTEXT handler satisfies every point of
the P1.M2.T2.S4 contract (and the authoritative PRD §4.6 wire / §14 firmware / §4
architecture / §5 protocol), that the multi-report framing reassembly (the contract's REVISE
point) correctly makes the handler reachable across report boundaries with no ≤26-callback
cap, that the AHC-specific host tests (stack / replace / disable-before-enable ordering /
layer-clear / multi-report reassembly) all pass, and that `notifier.c` stub-compiles clean.
**No source change is expected** unless a genuine AHC-level defect is found (none was found
in research).

**Deliverable**: A verification report (inline in the implementation session) that maps each
contract point to its existing `notifier.c` location, confirms the framing/reassembly
mechanism, shows the passing test evidence (64/64), and records the three documented
contract-vs-code deltas (the after-ETX-check prescription; the multi-report reassembly; the
stale line numbers + order — all *expected and correct*, not defects). If (and only if) a
real defect is found, a minimal surgical fix that preserves the length-aware-reassembly
architecture, the disable-before-enable diff, and the RISK-3 bounds checks.

**Success Definition**:
- Every row of the verification map (below) is satisfied by existing code (verified by read + grep).
- `notifier.c` stub-compiles with **exit 0, ZERO warnings** (the carried `-Wunused-*` set is
  resolved at HEAD `8441af2`).
- `test_notifier_host.c` shows the AHC family — (v) stack, (vi) replace, (vii) diff
  ordering, (viii) layer-clear, (multi-rep) two-report reassembly — **100% pass**, bringing
  the host suite to **64/64** and the stub-compile gate to **PASSED**.
- `git diff` is **empty** at the end of the task (or, in the defect-fix case, a minimal,
  justified diff with all gates still green).

## User Persona (if applicable)

**Target User**: The QMKonnect desktop host (P4.M3.T1.S1, planned) that, on every window
focus change, evaluates `rules.toml` and sends `APPLY_HOST_CONTEXT` to push the host layer
(≥ 224) and the desired callback id set, with `clear_board` selecting stack vs replace mode.

**Use Case**: Window focus changes → debounce → host evaluates rules → sends
`[0x81][0x9F][0xF0][0x05][layer][flags][count][id…][0x03]` (possibly across reports). The
first report seeds `typed_mode` + `typed_literal_remaining=2`; the byte loop consumes the
discriminator, cmd_id, fixed header (layer/flags/count), and the `count` ids literally
(any `0x03` among them is NOT treated as ETX); at the terminating `0x03`, `handle_typed_command`
parses `cmd_id=data[1]=0x05`, reads `layer/flags/count`, and:
- if `flags & 0x01` (replace): `deactivate_layer()` + `disable_command()` (board teardown);
- `set_host_layer(layer)` (host layer on/off; `0xFF` clears);
- `apply_host_callbacks(ids, count)` (disable-before-enable diff: `on_disable` for ids leaving
  the set, `on_enable` for ids entering);
- replies `[0x51][0x05][0x01]` (ack). Board and host state planes are otherwise orthogonal.

**Pain Points Addressed**: Gives the host a per-window "replace or stack" control over the
keyboard without reflashing. `clear_board=1` (replace) makes the board's own rules inert for
that window (the board tracker is cleared); `clear_board=0` (stack) lets the board rules and
the host layer coexist (host layer ≥ 224 resolves above board layers under QMK's
highest-layer-wins). The `0x51` response (≥2) is distinct from the legacy `0`/`1` match-bool,
so the host disambiguates typed from legacy. Multi-report framing removes the old ≤26-callback
cap, so a large callback set spans reports transparently.

## Why

- **Closes the QMKonnect-side tracking view** of a firmware feature already shipped (commits
  `c5ad578` + `8441af2`). The value this PRP adds is *preventing three regressions*: (a) an
  agent taking the contract's "move the 0xF0 check after ETX" literally would break the
  length-aware reassembly (BUG-1/BUG-2 return; AHC with a `0x03` arg truncates, and the
  sibling SET_OS stops dispatching); (b) an agent assuming AHC is single-report would
  re-introduce the ≤26-callback cap; (c) an agent "simplifying" the count clamp would weaken
  the BUG-3 defense.
- **Enforces board/host orthogonality (PRD invariant 21)** at the handler level: `clear_board`
  is the ONLY bridge between the planes, and it tears down the board BEFORE applying the host
  context (replace). In stack mode the board is untouched.
- **Codifies disable-before-enable (PRD §13 invariant 4)**: `apply_host_callbacks` disables
  newly-out-of-set ids before enabling newly-in-set ids, so a callback is never briefly in
  both states. Proven — not asserted — by the `g_seq` monotonic-stamp test (vii).

## What

Verify (do not rewrite) the following, all of which already exist in `notifier.c` at HEAD
`8441af2`:

### Success Criteria
- [ ] The APPLY_HOST_CONTEXT `case NOTIFY_CMD_APPLY_HOST_CONTEXT:` exists inside
      `handle_typed_command` (`notifier.c:767`).
- [ ] It has a length guard `if (len < 5) { send_typed_response(cmd_id, NULL, 0); break; }`
      (`notifier.c:780-783`) — BUG-3 hardening (disc + cmd + layer + flags + count).
- [ ] It reads `layer=(uint8_t)data[2]`, `flags=(uint8_t)data[3]`, `count=(uint8_t)data[4]`
      (`notifier.c:785-787`) — **args start at `data[2]`**, NOT `data[1]` (magic header
      stripped before reassembly; `data[0]=0xF0`, `data[1]=cmd_id`).
- [ ] It clamps `count` to `min(MSG_BUFFER_SIZE-5, len-5)` (`notifier.c:789-792`) — defense
      in depth + BUG-3; `ids = &data[5]` (L793).
- [ ] `clear_board` is `flags & 0x01` (L794): if set, `deactivate_layer()` (L795) then
      `disable_command()` (L796) — board teardown FIRST (replace).
- [ ] Then `set_host_layer(layer)` (L798) — `0xFF`/`LAYER_UNSET` clears `host_layer`.
- [ ] Then `apply_host_callbacks(ids, count)` (L799) — disable-before-enable diff.
- [ ] It builds `payload[1] = { 0x01 }` (ack=applied) and calls
      `send_typed_response(NOTIFY_CMD_APPLY_HOST_CONTEXT, payload, 1)` (L801-802).
- [ ] `send_typed_response` (`notifier.c:669-680`) emits exactly 32 bytes:
      `response[0]=0x51`, `response[1]=0x05` (cmd echo), `response[2]=0x01` (ack),
      zero-padded tail; calls `raw_hid_send(response, RAW_REPORT_SIZE)`.
- [ ] **THE FRAMING/REVISE POINT (load-bearing):** `typed_mode` (L96) seeded on the first
      report `data[2]==0xF0` (L835) + `typed_literal_remaining=2` (L837); `typed_fixed_arg_bytes`
      (L129-137, **AHC→3**); the gated ETX `if (c == ETX_TERMINATOR[0] && !typed_literal)`
      (L862); the variable ids-tail accounting at `msg_index==5` (L921-926, adds `count`
      literal bytes clamped to buffer room); dispatch at ETX
      `handle_typed_command(msg_buffer, msg_index)` (L868); resets at L889-890 and L934-935.
- [ ] `notifier.c` stub-compiles with exit 0, ZERO warnings.
- [ ] `test_notifier_host.c` AHC family (v/vi/vii/viii/multi-rep) passes; host suite 64/64.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge of this codebase can, using only this PRP + the
firmware repo, (a) confirm the AHC handler + framing are present and correct, (b) build and
run the host test to see the AHC family pass (64/64), (c) understand WHY the contract's
"move the 0xF0 check after ETX" prescription would REGRESS the firmware, and (d) avoid
regressing any of it — because the three contract-vs-code deltas, the BUG-1/BUG-2 analysis,
the code map, and the exact commands are all here (see also `research/revise_point_analysis.md`
and `research/state_assessment.md`).

### Documentation & References

```yaml
# MUST READ — PRD sections (QMKonnect plan/002 selected selectors; authoritative firmware
# detail is in the firmware repo's own PRD.md §4.6/§4.7/§14).
- url: spec/PRD.md (heading h2.83 "Wire Protocol (typed commands)")
  why: "APPLY_HOST_CONTEXT command-table row: Request args [layer][flags][count][id…],
        Response payload [ack]; layer ≥224 or 0xFF (clear); flags bit 0 = clear_board ⇒
        firmware clears its board activated_layer + current command BEFORE applying the host
        context (the per-window 'replace'); id… = the full desired enabled set; firmware
        diffs (disable-before-enable). Framing: ETX-framed and multi-report like strings
        (chunked at 30 payload bytes/report) — APPLY_HOST_CONTEXT may span reports (the old
        v1 single-report/≤26 limit is withdrawn)."
  critical: "Typed commands bypass process_full_message (no board side effects on the typed
        path EXCEPT the explicit clear_board teardown the AHC handler does). Responses use
        the 0x51 marker. The PRD reuses string ETX-framing for binary typed payloads — any
        payload byte == 0x03 collides with ETX; the firmware resolves this with length-aware
        reassembly, NOT a wire change."

- url: spec/PRD.md (heading h2.84 "Firmware Spec (qmk-notifier)")
  why: "APPLY_HOST_CONTEXT — honor clear_board (flags bit 0): if set, deactivate_layer() the
        board activated_layer + disable_command() the board command FIRST, then
        set_host_layer() + apply_host_callbacks(). set_host_layer: layer_on/off the host
        tracker only; 0xFF ⇒ clear. apply_host_callbacks: disable-before-enable diff."

- url: spec/PRD.md (heading h2.82 "Architecture & Coexistence Model")
  why: "two independent layer trackers (activated_layer board, host_layer host) are
        orthogonal; host layers sit ≥ 224 so they resolve above board layers. In replace
        mode the board tracker is cleared for that window (the host's clear_board flag)."

# MUST READ — existing firmware source (the thing being verified)
- file: /home/dustin/projects/qmk-notifier/notifier.c
  why: the AHC handler + the framing + all dependencies
  pattern: "L42 RAW_REPORT_SIZE=32; L79 MSG_BUFFER_SIZE=256; L96 typed_mode; L115
           typed_literal_remaining; L129-137 typed_fixed_arg_bytes (AHC->3); L167
           LAYER_UNSET=255; L184 host_layer; L185 host_cb_enabled; L259 activate_layer;
           L267 deactivate_layer; L293 set_host_layer; L324 apply_host_callbacks; L373
           disable_command; L669-680 send_typed_response (the [0x51] builder); L693
           handle_typed_command head (data[0]=0xF0, data[1]=cmd_id, data[2..]=args);
           L767-803 APPLY_HOST_CONTEXT case (the deliverable); L816 hid_notify; L835-837
           typed seed (first report); L858-862 gated ETX; L868 dispatch; L889-890/L934-935
           resets; L921-926 AHC variable ids-tail accounting"
  gotcha: "args start at data[2] (magic header stripped before reassembly). AND the handler
           is UNREACHABLE / truncates on a 0x03 arg without the length-aware reassembly
           (BUG-2): any of layer/flags/count/id == 0x03 would terminate reassembly early.
           The variable ids-tail accounting (msg_index==5) is what removes the ≤26-callback
           cap and makes AHC span reports."

- file: /home/dustin/projects/qmk-notifier/notifier.h
  why: "all NOTIFY_* constants + host_callback_t + DEFINE_HOST_CALLBACKS"
  pattern: "L46 NOTIFY_RESPONSE_MARKER 0x51; L51 NOTIFY_CMD_APPLY_HOST_CONTEXT 0x05;
           L60 HOST_CALLBACK_MAX 32; L63 HOST_LAYER_BASE 224; L17-21 host_callback_t;
           L69-73 DEFINE_HOST_CALLBACKS"
  gotcha: "constants ALREADY exist — do not re-add or renumber (0x04 is reserved for VIA)."

# MUST READ — the firmware's own (authoritative) PRP for this exact handler
- file: /home/dustin/projects/qmk-notifier/plan/003_16d737de7a3e/P1M2T2S3/PRP.md
  why: the original implementation spec that produced the committed AHC handler; confirms fidelity
  section: "What" (the handler body) and "Success Definition"

# MUST READ — the preceding (CONTRACT) PRPs whose outputs this handler consumes
- file: plan/002_637d65b6e9b8/P1M2T1S2/PRP.md
  why: "verified set_host_layer + apply_host_callbacks (the two helpers this handler is the
        SOLE caller of). Confirms the guarded layer_off clear, the disable-before-enable
        ordering, and the RISK-3 dual-bounds checks (id < HOST_CALLBACK_MAX AND id < cb_size)."
  critical: "do NOT strip the cb_size registry-bounds check (SIGSEGV when no
        DEFINE_HOST_CALLBACKS) or the layer_off clear-guard (would turn off QMK layer 255)."

- file: plan/002_637d65b6e9b8/P1M2T1S1/PRP.md
  why: "verified the typed_mode fork + handle_typed_command dispatch skeleton +
        send_typed_response that the AHC case lives inside."
  critical: "handle_typed_command signature is 'static bool handle_typed_command(char *data,
        uint16_t len)'; it switches on (uint8_t)data[1]; the [0x51] response is sent INSIDE
        it; hid_notify's typed_dispatched suppresses the legacy ack. The discriminator is
        classified on the FIRST report into typed_mode (NOT after-ETX) — this is load-bearing
        for multi-report + length-aware reassembly. All LANDED — do not re-add."

- file: plan/002_637d65b6e9b8/P1M2T2S3/PRP.md   # SET_OS — the framing-fix sibling
  why: "the framing fix (commit 8441af2) was verified for SET_OS (BUG-1: cmd_id 0x03==ETX).
        The SAME typed_literal_remaining mechanism protects AHC (BUG-2: any arg == 0x03)."
  critical: "do NOT revert 8441af2. The typed_fixed_arg_bytes(AHC)=3 + the msg_index==5
        variable-tail accounting are AHC's share of the same fix."

# Reference — existing tests
- file: /home/dustin/projects/qmk-notifier/test_notifier_host.c   # 64-test typed suite; (v)/(vi)/(vii)/(viii)/(multi-rep) cover AHC
- file: /home/dustin/projects/qmk-notifier/test_notifier_dispatch.c # 14-test legacy/dispatch regression
- file: /home/dustin/projects/qmk-notifier/test_notifier_os.c       # 31-test multi-OS regression
- file: /home/dustin/projects/qmk-notifier/run_notifier_stub_tests.sh # committed gate (dispatch + os + host)
- file: /home/dustin/projects/qmk-notifier/qmk_stubs/              # host-compile stubs (QMK_KEYBOARD_H, raw_hid_send capture, layer state)

# Reference — this task's research notes (the REVISE-point deep-dive + state evidence)
- file: plan/002_637d65b6e9b8/P1M2T2S4/research/revise_point_analysis.md  # why "check after ETX" is wrong; why first-report typed_mode is required
- file: plan/002_637d65b6e9b8/P1M2T2S4/research/state_assessment.md       # HEAD evidence + code map + 64/64 test proof
```

### Current Codebase tree (firmware repo, verification-relevant only)

```bash
# run from /home/dustin/projects/qmk-notifier
notifier.h            # NOTIFY_CMD_APPLY_HOST_CONTEXT 0x05, HOST_CALLBACK_MAX 32, HOST_LAYER_BASE 224, host_callback_t
notifier.c            # AHC case (L767), set_host_layer (L293), apply_host_callbacks (L324),
                      # deactivate_layer (L267), disable_command (L373), send_typed_response (L669),
                      # handle_typed_command head (L693), typed_mode/typed_literal_remaining (L96/115),
                      # typed_fixed_arg_bytes AHC->3 (L134), hid_notify (~L816), variable ids-tail acct (L921)
qmk_stubs/            # host-compile stubs (QMK_KEYBOARD_H, raw_hid_send capture, layer_on/off, os_detection)
test_notifier_dispatch.c   # 14-test legacy/dispatch regression suite
test_notifier_os.c         # 31-test multi-OS regression suite
test_notifier_host.c       # 64-test typed-command suite (AHC = cases v/vi/vii/viii/multi-rep)
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
// CRITICAL — THE REVISE POINT: the contract says "check msg_buffer[0]==0xF0 AFTER ETX
// reassembly". The committed design classifies on the FIRST report into typed_mode
// (notifier.c:835) and dispatches at ETX via `if (typed_mode)`. These are SEMANTICALLY
// equivalent in the happy path BUT the typed_mode flag is REQUIRED for the length-aware
// reassembly (typed_literal_remaining) to know, byte-by-byte during the loop, that it must
// consume binary args literally. Deferring the check to after ETX re-introduces BUG-1
// (SET_OS cmd_id 0x03==ETX truncates) and BUG-2 (any AHC arg == 0x03 truncates). Do NOT
// "move the 0xF0 check after ETX" — the typed_mode-first-report design is correct.
//
// CRITICAL: the args start at data[2], NOT data[1]. hid_notify does data += 2 to strip the
// [0x81][0x9F] magic header BEFORE reassembling into msg_buffer. So inside
// handle_typed_command the layout is: data[0]==0xF0 (discriminator), data[1]==cmd_id (0x05),
// data[2]==layer, data[3]==flags, data[4]==count, data[5..]==ids.
//
// CRITICAL — THE ≤26-CAP REMOVAL: AHC's variable-length ids tail is handled by
// typed_fixed_arg_bytes(AHC)==3 (the fixed header: layer/flags/count) PLUS the
// msg_index==5 accounting (read count, add `count` literal bytes, clamped to MSG_BUFFER_SIZE-1
// room). This lets AHC span reports (max 251 ids). Do NOT remove either piece.
//
// GOTCHA: the count clamp `min(MSG_BUFFER_SIZE-5, len-5)` is BUG-3 hardening + defense in
// depth: a malformed/garbled count (e.g. 0xFF) must never read past msg_buffer or into
// leftover bytes. apply_host_callbacks ALSO range-checks ids (RISK-3). Do NOT strip the clamp.
//
// GOTCHA: clear_board ordering is board-teardown-FIRST (deactivate_layer then
// disable_command), THEN host (set_host_layer then apply_host_callbacks). This is the
// "replace" semantics: the board is inert for this window before the host context applies.
//
// GOTCHA: the ack byte is 0x01 (applied). The contract response [0x51][0x05][0x01] means
// ack=1. 0x00 is reserved for a future NACK. Do NOT change the ack to 0x00.
//
// GOTCHA: AHC must NOT touch has_been_queried (QUERY_INFO's) or current_os (SET_OS's). It
// touches board state (via clear_board) and host state (via the two helpers) only.
//
// GOTCHA: the typed_literal_remaining / typed_mode state is `static` (L96, L115) so it
// survives across hid_notify calls — this is what makes MULTI-REPORT typed messages
// reassemble (the counts survive across reports, exactly like msg_index). They are reset at
// every ETX boundary (L889-890) and on overflow (L934-935). If you "fix" a reset, you break
// multi-report.
//
// GOTCHA: handle_typed_command takes `len` (the reassembled msg_index) — BUG-3 hardening.
// Each case validates len >= its minimum footprint before indexing args; a truncated frame
// falls through to the default no-payload ack. Called as handle_typed_command(msg_buffer,
// msg_index) at L868.
```

## Implementation Blueprint

### Verification map — contract point → existing code (NO new data models; C firmware)

| # | Contract point | Existing location | Verified? |
|---|---|---|---|
| 1 | `APPLY_HOST_CONTEXT` `case` inside `handle_typed_command`, switch on cmd_id | `notifier.c:767` (`case NOTIFY_CMD_APPLY_HOST_CONTEXT:`); `cmd_id=(uint8_t)data[1]` L694 | ☐ |
| 2 | length guard (BUG-3): require disc+cmd+layer+flags+count | `notifier.c:780-783` `if (len < 5) { send_typed_response(cmd_id, NULL, 0); break; }` | ☐ |
| 3 | read `layer=data[2]`, `flags=data[3]`, `count=data[4]` | `notifier.c:785-787` — **args at data[2]** (magic stripped) | ☐ |
| 4 | clamp `count` to buffer + reassembled-len bound | `notifier.c:789-792` `max_ids = min(MSG_BUFFER_SIZE-5, len-5)` | ☐ |
| 5 | `ids = &data[5]` (variable tail) | `notifier.c:793` | ☐ |
| 6 | `clear_board` = `flags & 0x01` | `notifier.c:794` | ☐ |
| 7 | if clear_board: board teardown FIRST | `notifier.c:795-796` `deactivate_layer(); disable_command();` | ☐ |
| 8 | then `set_host_layer(layer)` (0xFF clears host_layer) | `notifier.c:798` (helper L293) | ☐ |
| 9 | then `apply_host_callbacks(ids, count)` (disable-before-enable) | `notifier.c:799` (helper L324) | ☐ |
| 10 | build ack payload `[0x01]` | `notifier.c:801` `uint8_t payload[1] = { 0x01 };` | ☐ |
| 11 | `response[0]=0x51` (NOTIFY_RESPONSE_MARKER) | `notifier.c:671` (send_typed_response) | ☐ |
| 12 | `response[1]=NOTIFY_CMD_APPLY_HOST_CONTEXT` (0x05) cmd echo | `notifier.c:672` (send_typed_response echoes cmd_id) | ☐ |
| 13 | `response[2]=ack=0x01`, zero-padded to 32 | payload[0]=0x01→response[2]; `raw_hid_send(response, RAW_REPORT_SIZE)` L679 (32 bytes) | ☐ |
| 14 | **REVISE POINT:** typed command reassembled across reports, dispatched AFTER ETX | `typed_mode` seed L835-837 (first report); dispatch `handle_typed_command(msg_buffer, msg_index)` L868 at ETX; **NOT** a post-hoc `msg_buffer[0]` check | ☐ |
| 15 | **≤26-CAP REMOVAL:** variable ids tail spans reports | `typed_fixed_arg_bytes(AHC)=3` L134; `msg_index==5` accounting L921-926 (read count, add count literal bytes, clamped to room) | ☐ |
| 16 | length-aware reassembly protects a `0x03` arg (BUG-2) | `typed_literal_remaining` L115; gated ETX `&& !typed_literal` L862 | ☐ |
| 17 | bypasses `process_full_message` (no board side effect except clear_board) | `if (typed_mode) handle_typed_command … else process_full_message` L868 | ☐ |
| 18 | board/host orthogonality: helpers touch ONLY their plane | `set_host_layer` host-only (L293); `apply_host_callbacks` host-only (L324); board teardown is the handler's clear_board | ☐ |
| 19 | disable-before-enable ORDER (Phase 1 before Phase 2) | `apply_host_callbacks` L336-343 (Phase 1) before L346-356 (Phase 2); test (vii) PASS | ☐ |
| 20 | Constants: AHC=0x05, RESPONSE_MARKER=0x51, HOST_CALLBACK_MAX=32, HOST_LAYER_BASE=224 | `notifier.h:51,46,60,63`; `notifier.c:42` (RAW_REPORT_SIZE) | ☐ |
| 21 | AHC does NOT touch `has_been_queried` or `current_os` | L767-803 (no ref to either) | ☐ |

### Implementation Tasks (verification-ordered)

```yaml
Task 1: ESTABLISH baseline (no edits)
  - RUN: cd /home/dustin/projects/qmk-notifier && git status -s && git log --oneline -3
  - EXPECT: clean tree, HEAD at or past 8441af2 ("Implement length-aware typed command reassembly")
  - READ: notifier.c L42 (RAW_REPORT_SIZE), L79 (MSG_BUFFER_SIZE), L96 (typed_mode), L115
          (typed_literal_remaining), L129-137 (typed_fixed_arg_bytes, AHC->3), L167 (LAYER_UNSET),
          L184-186 (host state), L259-276 (activate_layer/deactivate_layer), L293-306
          (set_host_layer), L324-358 (apply_host_callbacks), L373-377 (disable_command),
          L669-680 (send_typed_response), L693-694 (handle_typed_command head), L767-803
          (APPLY_HOST_CONTEXT case — the deliverable), L816-947 (hid_notify: L835-837 seed,
          L858-862 gated ETX, L868 dispatch, L889-890/L934-935 resets, L921-926 ids-tail acct);
          notifier.h L46,51,60,63 (constants)
  - CONFIRM every row of the Verification map above (check the ☐ boxes in your report)
  - NOTE the three documented contract-vs-code deltas (after-ETX-check prescription; multi-
    report reassembly via typed_mode not a post-hoc check; stale line numbers + order) are
    CORRECT, not defects — record them as "verified correct, kept as-is"

Task 2: RUN the committed stub-compile gate (expect GREEN)
  - RUN: ./run_notifier_stub_tests.sh
  - EXPECT (current baseline, accurate as of HEAD 8441af2):
      * dispatch fails=0  (exit=0)   ✓
      * os fails=0        (exit=0)   ✓
      * host fails=0      (exit=0)   ✓
      * final line: "✓ notifier stub-compile gate PASSED" (exit 0)
  - IF host shows ANY failure: that indicates a REAL regression worth investigating
    (likely a partial revert of 8441af2 or c5ad578). Diagnose root cause before any edit.
    The AHC or SET_OS failures returning means the typed_literal_remaining mechanism or the
    variable ids-tail accounting was lost — re-apply the commit (git cherry-pick / git
    revert) rather than hand-patching.

Task 3: BUILD+RUN the typed-command host suite; isolate the AHC family
  - RUN:
      gcc -Wall -Wextra -std=c99 -DQMK_KEYBOARD_H='"qmk_keyboard_stub.h"' -Iqmk_stubs -I. \
          -c notifier.c -o /tmp/nh.o
      gcc -Wall -std=c99 -Iqmk_stubs -I. /tmp/nh.o qmk_stubs/qmk_stubs.c test_notifier_host.c \
          -o /tmp/test_notifier_host
      /tmp/test_notifier_host 2>&1 | grep -iE 'APPLY_HOST_CONTEXT|clear_board|host layer|callback diff|multi-rep|Total tests'
  - EXPECT: 64 run / 64 pass / 0 fail. AHC-relevant groups that MUST be 100% pass:
      (v)   STACK clear_board=0: host layer 224 active; board command NOT torn down
            [test_notifier_host.c:258-269]
      (vi)  REPLACE clear_board=1: board command torn down; host layer 224 active
            [test_notifier_host.c:271-280]
      (vii) callback diff: AHC{[1]} on_disable(id0) seq < on_enable(id1) seq
            [disable-before-enable proven via g_seq stamps]   [test_notifier_host.c:282-301]
      (viii) AHC{layer=0xFF}: host layer cleared (LAYER_UNSET=255)
            [test_notifier_host.c:304-311]
      (multi-rep) two-report AHC count=28: r[0]=0x51, r[1]=0x05, r[2]=ack=1, host layer 224,
            callback diff ran (id 0 enabled once)   [test_notifier_host.c:378-404]
  - PROBE (optional, confirms BUG-2 is fixed): if any AHC line begins with "FAIL", the
    framing fix is absent/broken — see Task 2's recovery note.

Task 4: (DEFAULT) NO-OP — write the verification report, leave source untouched
  - IF Tasks 1-3 are green (expected path): the deliverable is the inline report. git diff
    stays empty. Done.
  - IF a genuine AHC-level defect is found (unexpected): make the MINIMAL surgical fix in
    notifier.c that preserves the length-aware-reassembly architecture, the disable-before-
    enable diff, the RISK-3 bounds checks, and the count clamp, then re-run Tasks 2 & 3 to
    confirm 64/64. Document the defect, the fix, and the before/after test counts.

Task 5: NEVER do these
  - DO NOT move the 0xF0 discriminator check to after-ETX (the contract's literal REVISE
    prescription). The typed_mode first-report classification (notifier.c:835) is REQUIRED
    for the length-aware reassembly to know, byte-by-byte, to consume binary args literally.
    Deferring it re-introduces BUG-1 (SET_OS cmd_id 0x03 truncates) and BUG-2 (any AHC arg
    == 0x03 truncates) and breaks multi-report classification.
  - DO NOT revert or weaken commit 8441af2 (typed_literal_remaining) or the variable ids-tail
    accounting (L921-926). They are what make AHC dispatch with a 0x03 arg AND span reports.
  - DO NOT strip the count clamp `min(MSG_BUFFER_SIZE-5, len-5)` — BUG-3 hardening.
  - DO NOT change the ack byte from 0x01 (ack=applied; 0x00 reserved for NACK).
  - DO NOT change the args indexing (layer=data[2], flags=data[3], count=data[4], ids=data[5]).
    data[1] is the cmd_id byte (0x05).
  - DO NOT make AHC touch has_been_queried or current_os.
  - DO NOT change the AHC handler, send_typed_response, handle_typed_command, set_host_layer,
    apply_host_callbacks, deactivate_layer, or disable_command signatures or bodies.
  - DO NOT renumber NOTIFY_* constants (0x04 stays reserved for VIA).
  - DO NOT edit PRD.md, any tasks.json, prd_snapshot.md, or any plan/ files (read-only).
```

### Implementation Patterns & Key Details
```c
// The existing (correct) APPLY_HOST_CONTEXT handler — notifier.c:767-802:
//   case NOTIFY_CMD_APPLY_HOST_CONTEXT: {
//       if (len < 5) {                                   /* BUG-3: disc+cmd+layer+flags+count */
//           send_typed_response(cmd_id, NULL, 0);
//           break;
//       }
//       uint8_t layer = (uint8_t)data[2];                /* ARG[0] — data[2], NOT data[1] */
//       uint8_t flags = (uint8_t)data[3];                /* ARG[1] — bit 0 = clear_board   */
//       uint8_t count = (uint8_t)data[4];                /* ARG[2] — ids tail length       */
//       uint8_t max_ids = (uint8_t)(MSG_BUFFER_SIZE - 5);
//       if (max_ids > (uint8_t)(len - 5)) max_ids = (uint8_t)(len - 5);  /* clamp to reassembled len */
//       if (count > max_ids) count = max_ids;            /* defense in depth (BUG-3)       */
//       uint8_t *ids = (uint8_t *)&data[5];              /* variable tail                  */
//       if (flags & 0x01) {                              /* clear_board (replace): board FIRST */
//           deactivate_layer();
//           disable_command();
//       }
//       set_host_layer(layer);                           /* host: 0xFF (LAYER_UNSET) clears host_layer */
//       apply_host_callbacks(ids, count);                /* host: disable-before-enable diff          */
//       uint8_t payload[1] = { 0x01 };                   /* ack = 1 (applied) */
//       send_typed_response(NOTIFY_CMD_APPLY_HOST_CONTEXT, payload, 1);
//       break;
//   }

// The load-bearing REVISE-point mechanism — notifier.c:835-837 + 858-868 (first-report
// classification + length-aware reassembly + dispatch at ETX):
//   if (msg_index == 0 && length >= 3 && data[2] == NOTIFY_CMD_DISCRIMINATOR) {
//       typed_mode = true;
//       typed_literal_remaining = 2;   /* consume discriminator + cmd_id literally */
//   }
//   data += 2; length -= 2;            /* strip the [0x81][0x9F] magic header */
//   for (...) {
//       bool typed_literal = (typed_mode && typed_literal_remaining > 0);
//       if (c == ETX_TERMINATOR[0] && !typed_literal) {           /* gated ETX */
//           if (!dropping) {
//               if (typed_mode) {                                 /* AFTER reassembly */
//                   match = handle_typed_command(msg_buffer, msg_index);
//                   typed_dispatched = true;
//               } else { sanitize_string(...); match = process_full_message(msg_buffer); }
//           }
//           msg_index = 0; dropping = false; typed_mode = false; typed_literal_remaining = 0;
//           break;
//       }
//       ...
//       if (msg_index == 5 && (uint8_t)msg_buffer[1] == NOTIFY_CMD_APPLY_HOST_CONTEXT) {
//           uint8_t ahc_count = (uint8_t)msg_buffer[4];           /* the ≤26-cap removal */
//           uint16_t room = (uint16_t)((MSG_BUFFER_SIZE - 1) - msg_index);
//           typed_literal_remaining += (ahc_count > room) ? room : ahc_count;
//       }
//   }

// The existing (correct) response builder — notifier.c:669-680 (send_typed_response):
//   static void send_typed_response(uint8_t cmd_id, const uint8_t *payload, uint8_t payload_len) {
//       uint8_t response[RAW_REPORT_SIZE] = {0};   /* zero-pads the unused tail */
//       response[0] = NOTIFY_RESPONSE_MARKER;      /* 0x51 */
//       response[1] = cmd_id;                      /* echo (0x05) */
//       if (payload != NULL && payload_len > 0) {
//           uint8_t cap = (uint8_t)(RAW_REPORT_SIZE - 2);   /* 30 bytes after [0x51][cmd_id] */
//           uint8_t n = (payload_len < cap) ? payload_len : cap;
//           memcpy(response + 2, payload, n);
//       }
//       raw_hid_send(response, RAW_REPORT_SIZE);
//   }
//   // => wire bytes: [0]=0x51 [1]=0x05 [2]=0x01 (ack) [3..31]=0 (zero-padded) = 32 bytes total
```

### Integration Points
```yaml
DEPENDENCIES: none — this is self-contained firmware C; it does NOT depend on the
              qmk_notifier Rust crate or its v0.3.0 tag (P1.M1.T4.S1).
              It depends on P1.M2.T1.S1 (the typed-dispatch skeleton + send_typed_response)
              and P1.M2.T1.S2 (set_host_layer + apply_host_callbacks) — BOTH already exist and
              are verified. The framing fix (8441af2) is committed and verified by the SET_OS
              sibling (P1.M2.T2.S3).
DOWNSTREAM (consumers):
  - P4.M3.T1.S1 desktop host-context send logic (QMKonnect, planned) — the host evaluates
    rules.toml per window and sends APPLY_HOST_CONTEXT with layer + callback id set +
    clear_board; consumes [0x51][0x05][0x01] as the ack.
  - The host may send AHC with count > 25 (multi-report) and/or an arg of value 0x03; only
    the length-aware reassembly makes that work. The host need not avoid 0x03 in any arg.
HOST (desktop) pipeline that drives this — P4.M3 (planned): sends AHC per window change.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)
```bash
cd /home/dustin/projects/qmk-notifier
# The gate's step [1/5] does the canonical stub-compile. Do NOT compile notifier.c standalone.
gcc -Wall -Wextra -std=c99 -DQMK_KEYBOARD_H='"qmk_keyboard_stub.h"' -Iqmk_stubs -I. \
    -c notifier.c -o /tmp/notifier_stub.o
# Expected: exit 0, ZERO warnings (the prior -Wunused-* set is resolved at HEAD 8441af2).

# Confirm the AHC case + its dependencies are present and correct:
grep -n 'case NOTIFY_CMD_APPLY_HOST_CONTEXT' notifier.c          # expect one line ~767
grep -n 'uint8_t layer = (uint8_t)data\[2\]' notifier.c          # expect one line ~785
grep -n 'if (flags & 0x01)' notifier.c                           # expect clear_board ~794
grep -n 'set_host_layer(layer)' notifier.c                       # expect one line ~798
grep -n 'apply_host_callbacks(ids, count)' notifier.c            # expect one line ~799
grep -n 'payload\[1\] = { 0x01 }' notifier.c                     # expect the ack ~801
grep -n 'case NOTIFY_CMD_APPLY_HOST_CONTEXT: return 3' notifier.c # expect typed_fixed_arg_bytes ~134
grep -n '&& !typed_literal' notifier.c                           # expect the gated ETX ~862
rm -f /tmp/notifier_stub.o
```

### Level 2: The committed regression suites (Component Validation)
```bash
cd /home/dustin/projects/qmk-notifier
./run_notifier_stub_tests.sh
# Expected (ACCURATE as of HEAD 8441af2):
#   notifier dispatch fails=0  (exit=0)   ✓
#   notifier os fails=0        (exit=0)   ✓
#   notifier host fails=0      (exit=0)   ✓
#   final line: "✓ notifier stub-compile gate PASSED" (exit 0)
# CRITICAL: the gate is GREEN. If host shows failures, the typed_literal_remaining framing
# fix (8441af2) or the AHC handler (c5ad578) was lost — re-apply the commit rather than
# hand-patching. dispatch 14/14 and os 31/31 MUST stay green (they prove no regression in
# the reassembler, matcher, F4/F5/F8/F9 logic, or the hid_notify routing fork).
```

### Level 3: Typed-command host suite (AHC-scope validation)
```bash
cd /home/dustin/projects/qmk-notifier
gcc -Wall -Wextra -std=c99 -DQMK_KEYBOARD_H='"qmk_keyboard_stub.h"' -Iqmk_stubs -I. \
    -c notifier.c -o /tmp/nh.o
gcc -Wall -std=c99 -Iqmk_stubs -I. /tmp/nh.o qmk_stubs/qmk_stubs.c test_notifier_host.c \
    -o /tmp/test_notifier_host
/tmp/test_notifier_host 2>&1 | grep -iE 'APPLY_HOST_CONTEXT|clear_board|host layer|callback diff|multi-rep|Total tests'
# Expected: every AHC line begins with "PASS". Specifically:
#   (v)   stack: host layer 224 active; board command NOT torn down (clear_board=0)
#   (vi)  replace: board command torn down; host layer 224 active (clear_board=1)
#   (vii) AHC{[1]}: on_disable(id0) BEFORE on_enable(id1) [disable-before-enable]
#   (viii) AHC{layer=0xFF}: host layer cleared (LAYER_UNSET)
#   (multi-rep) two-report AHC count=28: r[0]=0x51, r[1]=0x05, r[2]=ack=1, host layer 224, diff ran
# Suite total: 64 run / 64 pass / 0 fail.
rm -f /tmp/nh.o /tmp/test_notifier_host
```

### Level 4: Legacy-path regression (no AHC handler touches it)
```bash
cd /home/dustin/projects/qmk-notifier
./run_all_tests.sh
# Expected: the 9-suite pattern_match corpus is unaffected (legacy string path is
# byte-identical for non-typed reports; AHC is only reachable via the typed 0xF0 path, and
# the typed_literal mechanism never activates for legacy strings because typed_mode stays
# false). All suites pass.
```

## Final Validation Checklist

### Technical Validation
- [ ] Verification map (21 rows) fully satisfied by existing code.
- [ ] `notifier.c` stub-compile → exit 0, ZERO warnings.
- [ ] `./run_notifier_stub_tests.sh` → dispatch fails=0, os fails=0, **host fails=0** (gate PASSED).
- [ ] `test_notifier_host.c` AHC family ((v), (vi), (vii), (viii), (multi-rep)) 100% pass; host 64/64.
- [ ] `./run_all_tests.sh` — 9-suite pattern corpus unaffected.

### Feature Validation
- [ ] AHC{layer=224, clear_board=0, [0]} returns `[0x51][0x05][0x01]` AND activates host layer 224
      WITHOUT tearing down the board (stack).
- [ ] AHC{layer=224, clear_board=1, [0]} tears down the board (deactivate_layer + disable_command)
      THEN activates host layer 224 (replace).
- [ ] AHC{layer=0xFF, …} clears the host layer to LAYER_UNSET (255).
- [ ] apply_host_callbacks fires on_disable BEFORE on_enable across a set transition (test vii).
- [ ] AHC with count=28 spanning two reports reassembles and dispatches (test multi-rep) — the
      ≤26-callback cap is removed.
- [ ] args are read from data[2..] (NOT data[1]); count is clamped to min(MSG_BUFFER_SIZE-5, len-5).
- [ ] AHC does NOT touch has_been_queried or current_os.
- [ ] Response is exactly 32 bytes (zero-padded by send_typed_response); ack byte is 0x01.

### Code Quality Validation
- [ ] `git diff` is EMPTY (expected default) OR a minimal, justified defect-fix with 64/64 still green.
- [ ] No re-implementation of the (regressive) literal contract (after-ETX 0xF0 check; single-report;
      stripped count clamp; ack 0x00).
- [ ] AHC handler consumes NOTIFY_* constants by name (no hardcoded 0x51/0x05/0x01 literals).
- [ ] Commit `8441af2` (the framing fix) + the variable ids-tail accounting are intact.
- [ ] No renumbering of NOTIFY_CMD_* constants.

### Documentation & Deployment
- [ ] No user-facing docs required (firmware C code — per the work-item DOCS: none).
- [ ] Verification report recorded inline (contract map + test counts + the three documented
      deltas: the after-ETX-check prescription; multi-report reassembly via typed_mode not a
      post-hoc check; stale line numbers + board-teardown order).

---

## Anti-Patterns to Avoid
- ❌ Do NOT move the `0xF0` discriminator check to after-ETX (the contract's literal REVISE
  prescription). The `typed_mode` first-report classification (`notifier.c:835`) is REQUIRED
  for the length-aware reassembly (`typed_literal_remaining`) to know, byte-by-byte, that it
  must consume binary args literally. Deferring the check re-introduces BUG-1 (SET_OS cmd_id
  `0x03==ETX` truncates) and BUG-2 (any AHC arg of value `0x03` truncates) and breaks
  multi-report classification (continuation reports carry payload at `data[2]`, which may be
  `0xF0`). The `typed_mode` flag IS the durable form of "msg_buffer[0]==0xF0".
- ❌ Do NOT revert or weaken commit `8441af2` or the variable ids-tail accounting (`L921-926`).
  They are what make AHC dispatch with a `0x03` arg AND span reports (≤26-cap removal).
- ❌ Do NOT strip the count clamp `min(MSG_BUFFER_SIZE-5, len-5)` — BUG-3 hardening + defense
  in depth against a malformed `0xFF` count.
- ❌ Do NOT change the ack byte from `0x01` — ack=1 (applied); `0x00` is reserved for NACK.
- ❌ Do NOT change the args indexing — `layer=data[2]`, `flags=data[3]`, `count=data[4]`,
  `ids=data[5]`. `data[1]` is the cmd_id byte (0x05).
- ❌ Do NOT make AHC touch `has_been_queried` (QUERY_INFO's) or `current_os` (SET_OS's).
- ❌ Do NOT change the AHC handler, `send_typed_response`, `handle_typed_command`,
  `set_host_layer`, `apply_host_callbacks`, `deactivate_layer`, or `disable_command`.
- ❌ Do NOT mistake an AHC/SET_OS failure re-appearance for a "handler bug" — it means the
  framing fix (`8441af2`) was lost; re-apply the commit, do not hand-patch the handler.
- ❌ Do NOT renumber NOTIFY_* constants (0x04 stays reserved for VIA).
- ❌ Do NOT edit PRD.md, tasks.json, prd_snapshot.md, or any plan/ file.
- ❌ Do NOT assume this task must produce a diff — the expected, correct outcome is a
  verification report with an empty diff.

---

## Confidence Score: 9/10

The deliverable is already present, correct, committed (`c5ad578` + `8441af2`), and green at
its scope (verified this session: `notifier.c` stub-compiles clean with ZERO warnings; the
APPLY_HOST_CONTEXT handler is present at `notifier.c:767-802` reading `layer/flags/count`
from `data[2..4]`, honoring `clear_board` (`flags & 0x01` ⇒ `deactivate_layer()` +
`disable_command()` first), then calling `set_host_layer(layer)` + `apply_host_callbacks(ids,
count)`, and replying `[0x51][0x05][0x01]`; the contract's REVISE point — multi-report
ETX-framed reassembly — is satisfied by the first-report `typed_mode` classification +
length-aware `typed_literal_remaining` reassembly + the variable ids-tail accounting that
removes the ≤26-callback cap; `test_notifier_host.c` AHC family (v/vi/vii/viii/multi-rep) all
PASS; host suite 64/64; gate PASSED). The 1-point reservation is for the (unlikely) discovery
of a genuine AHC-level defect during the implementation agent's own verification pass; if
found, the minimal-fix path is specified. The dominant risk this PRP neutralizes is
**threefold**: (1) an agent taking the contract's literal "move the 0xF0 check after ETX"
prescription would silently break the length-aware reassembly — re-introducing BUG-1 (SET_OS
undeliverable) and BUG-2 (AHC truncation on a `0x03` arg) and breaking multi-report
classification; (2) an agent assuming AHC is single-report would re-introduce the
≤26-callback cap; (3) an agent "simplifying" the count clamp would weaken BUG-3 defense. The
REVISE-point framing is the single most important non-obvious fact an implementer must
understand: **the contract's prescribed after-ETX check is semantically equivalent in the
happy path but mechanically WRONG** — the length-aware reassembly can only consume binary
args literally if `typed_mode` is known during the byte loop, which requires first-report
classification, not a post-hoc buffer check.
# Research Findings — P1.M2.T1.S2: `set_host_layer()` + `apply_host_callbacks()`

## HEADLINE: work is ALREADY IMPLEMENTED in the firmware repo

Repo under change: **qmk-notifier FIRMWARE (C)** at `/home/dustin/projects/qmk-notifier`,
branch `main`. This is the same repo as P1.M2.T1.S1. Both helper functions exist,
are correct, and are covered by the committed host test suite. This is a desktop-side
**coordination mirror** of firmware work already shipped by the firmware repo's own
`plan/003_16d737de7a3e` (which splits this into P1M2T1S2=`set_host_layer` and
P1M2T1S3=`apply_host_callbacks`; QMKonnect plan/002 folds both into one item).

### Git evidence (HEAD = dc52967)
- `c5ad578 Implement typed command dispatch and response builder`
- `ab7055f Implement host-authoritative SET_OS command`
- `779152a Implement APPLY_HOST_CONTEXT typed command handler`
- `11a698f Implement host test suite for typed command queries`
- `7a36675 Add SET_OS and APPLY_HOST_CONTEXT test cases`
- `dc52967 Add coexistence and multi-report framing tests`

`git status -s` → only `?? plan/003_16d737de7a3e/P1M3T2S1/` (untracked plan dir); source tree clean.

## Exact locations (verified by read + grep)

| Symbol | Location | Notes |
|---|---|---|
| `LAYER_UNSET 255` | `notifier.c:126` | macro, used not literal `0xFF` |
| `activated_layer = LAYER_UNSET` | `notifier.c:127` | board tracker |
| `host_layer = LAYER_UNSET` | `notifier.c:143` | host tracker (contract said ~137) |
| `host_cb_enabled[HOST_CALLBACK_MAX]` | `notifier.c:144` | diff target (contract said ~138) |
| `activate_layer()` | `notifier.c:218` | board sibling (contract said ~225) |
| `deactivate_layer()` | `notifier.c:226` | board sibling |
| **`set_host_layer()`** | **`notifier.c:252-265`** | host layer tracker |
| **`apply_host_callbacks()`** | **`notifier.c:283-317`** | host callback diff (disable-before-enable) |
| `get_host_callbacks()` weak | `notifier.c:123` | `{NULL,0}` default |
| `get_host_callbacks_size()` weak | `notifier.c:124` | `0` default |
| AHC caller | `notifier.c:709-734` | `APPLY_HOST_CONTEXT` handler |

Header constants (`notifier.h`): `HOST_CALLBACK_MAX 32` (L60), `HOST_LAYER_BASE 224` (L63),
`host_callback_t {name, on_enable, on_disable}` (L17-21), `callback_t = void(*)(void)` (L5),
`DEFINE_HOST_CALLBACKS` macro (L69-73), accessor decls (L33-34).

## Contract vs implementation — deltas (align to CODE)

1. **set_host_layer clear-branch guard.** Contract text: "if layer==0xFF, layer_off(host_layer)
   and set host_layer=LAYER_UNSET" (sounds unconditional). Implementation:
   `if (host_layer != LAYER_UNSET) layer_off(host_layer);` then `host_layer = LAYER_UNSET`.
   The **guard is a CORRECTNESS requirement**: without it, `layer_off(255)` would erroneously
   turn off QMK layer 255. Implementation is right; literal contract text is imprecise.

2. **apply_host_callbacks RISK-3 bounds.** Contract: "Guard against NULL on_enable/on_disable
   and ids >= HOST_CALLBACK_MAX." Implementation does both AND additionally checks
   `id < cb_size` (registry bounds via `get_host_callbacks_size()`) before dereferencing
   `cbs[id]` in **both** phases. This is defensive: malformed host data must not crash when
   `cb_size==0` (no `DEFINE_HOST_CALLBACKS`). Stripping it would be a regression (SIGSEGV risk).
   Phase 1 still clears `host_cb_enabled[id]` for out-of-set ids regardless (clears the array,
   not the deref) — matches "clear the flag" in the contract.

3. **Both are `static`, file-local.** Sole caller is the AHC handler (`notifier.c:729-730`).

## Verification: both helpers EXACTLY match the intended semantics

- **set_host_layer**: LAYER_UNSET(0xFF) ⇒ guarded `layer_off` + clear; else `layer_off(old)`
  (if set) then `layer_on(new)` + track. Touches ONLY `host_layer`, never `activated_layer`.
  No range validation (RISK-2: host trusted, ≥224 is a host-side convention). ✔
- **apply_host_callbacks**: Phase 1 disable (id in `host_cb_enabled` but not in `ids` ⇒
  guarded `on_disable` + clear) BEFORE Phase 2 enable (id in `ids`, not already enabled,
  in-range ⇒ guarded `on_enable` + set). NULL on_enable/on_disable guarded. id ≥
  HOST_CALLBACK_MAX skipped. Unchanged ids fire neither. Disable-before-enable ✔ idempotent ✔

## Test baseline (run this session)

### Stub gate `./run_notifier_stub_tests.sh`
- `test_notifier_dispatch` **14/14**, `test_notifier_os` **31/31**, 0 FAIL.
- Final line: `✓ notifier stub-compile gate PASSED`.

### Host suite `test_notifier_host.c` (built manually — not yet in the runner)
Build:
```
gcc -Wall -Wextra -std=c99 -DQMK_KEYBOARD_H='"qmk_keyboard_stub.h"' -Iqmk_stubs -I. \
    -c notifier.c -o /tmp/nh.o
gcc -Wall -std=c99 -Iqmk_stubs -I. /tmp/nh.o qmk_stubs/qmk_stubs.c test_notifier_host.c \
    -o /tmp/test_notifier_host
/tmp/test_notifier_host
```
Result: **64 run / 57 pass / 7 fail.**

**The 7 failures are ALL SET_OS handler (§4.7) — OUT OF SCOPE** for the helpers:
```
FAIL: (i) SET_OS r[1]=0x03 cmd echo [§4.6]
FAIL: (i) SET_OS r[2]=ack=1 [§4.6]
FAIL: (ii) post-SET_OS(OS_MACOS): OS_MACOS command fired (current_os changed) [§4.7]
FAIL: (ii) post-SET_OS(OS_MACOS): OS_MACOS layer 44 selected [§4.7]
FAIL: (iii) SET_OS change: prev command on_disable fired [§4.7/F9.1]
FAIL: (iii) SET_OS change: board layer deactivated (cleared) [§4.7/F9.1]
FAIL: (iv) SET_OS idempotent: no layer change on same-OS [§4.7/F9.3]
```
None reference `set_host_layer` or `apply_host_callbacks`. Tracked by firmware plan/003
P1.M3.T2.S1 (Researching) / QMKonnect P1.M2.T2.S2-S3.

**Helper-scope groups that MUST be 100% pass (and ARE):**
- (v)   STACK `clear_board=0`: board command NOT torn down ✔
- (vi)  REPLACE `clear_board=1`: board torn down + host layer 224 active ✔
- (vii) callback diff ordering: `AHC{[0]}` on_enable(id0); then `AHC{[1]}`:
        on_disable(id0) BEFORE on_enable(id1) [disable-before-enable ORDER proven via g_seq] ✔
- (viii) `AHC{layer=0xFF}`: host layer cleared (LAYER_UNSET) ✔
- (multi-rep) two-report AHC: r[0]=0x51, r[1]=0x05, r[2]=ack=1, host layer 224, cb diff ran ✔

## Test mechanism for ORDER (subtle correctness property)

Plain counters can't prove disable-before-enable ORDER. The harness uses monotonic
sequence stamps (`g_seq`, `cb_*_on_seq`/`cb_*_off_seq`) — each callback records `++g_seq`
at call time. Assertion (vii): `cb_mute_off_seq < cb_layout_on_seq` ⇒ on_disable(id0)
strictly before on_enable(id1). The `_en`/`_dis` counters still increment, so earlier
`cb_*_en==0` QUERY_* assertions remain valid (backward-compatible). `DEFINE_HOST_CALLBACKS`
registers two entries (`mute`, `layout`) so `callback_count=2`.

## Stub-compile warnings (4, all pre-existing & expected)
`board_rules_present`, `has_been_queried`, `host_cb_enabled`, `host_layer` — wait:
`set_host_layer` and `apply_host_callbacks` are `static` and ARE called by the AHC
handler, which IS compiled into `notifier.c`, so they do NOT warn under the full
stub build (they're only unused in a build that excludes the AHC handler — not this one).
The 4 warnings listed by P1.M2.T1.S1's PRP are the file-level under-used globals.
**Do NOT silence them with `(void)` casts.**

## Dependencies / boundaries
- This item does NOT depend on the Rust crate (P1.M1) or its v0.3.0 tag. Firmware C is
  independent.
- It is consumed by the AHC handler (P1.M2.T2.S4 = QMKonnect / P1.M2.T2.S3 firmware),
  which ALREADY exists and is tested (it's the sole caller).
- Host handshake that drives this (desktop) is P4.M2.T1 (planned).

## Authoritative firmware-side PRP references (for fidelity cross-check)
- `/home/dustin/projects/qmk-notifier/plan/003_16d737de7a3e/P1M2T1S2/PRP.md` — set_host_layer
- `/home/dustin/projects/qmk-notifier/plan/003_16d737de7a3e/P1M2T1S3/PRP.md` — apply_host_callbacks
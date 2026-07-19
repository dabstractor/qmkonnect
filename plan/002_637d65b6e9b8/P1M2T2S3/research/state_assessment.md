# State Assessment — P1.M2.T2.S3 (SET_OS handler)

## Repo under change
- **qmk-notifier FIRMWARE** (C) at `/home/dustin/projects/qmk-notifier`
- remote `git@github.com:dabstractor/qmk-notifier`, branch `main`
- Same repo as P1.M2.T1.S1/S2 and P1.M2.T2.S1/S2. NOT the `qmk_notifier` Rust crate.

## HEAD evolution during this research session
| When | HEAD | notifier.c SET_OS state | host suite | gate |
|------|------|------------------------|-----------|------|
| S2 researched | `70fcfa1` | handler present but **BLOCKED** (BUG-1) | 64 run / 57 pass / **7 fail** (all SET_OS) | FAILED |
| start of S3 research | `c07e84f` (docs-only over 70fcfa1) | same — BLOCKED | same 7 fails | FAILED |
| mid-S3 (working tree) | `c07e84f` + **uncommitted** `typed_literal` fix | **FIXED** in working tree | 64/64 pass | PASSED |
| **end of S3 research (NOW)** | **`8441af2` "Implement length-aware typed command reassembly"** | **FIXED + COMMITTED** | **64/64 pass** | **PASSED** |

The framing fix landed as commit `8441af2` **during** this research session (working tree
is now clean). HEAD `8441af2` contains the complete, tested fix.

## Definitive test evidence (clean HEAD `8441af2`, this session)
```
./run_notifier_stub_tests.sh
  notifier dispatch fails=0  (exit=0)
  notifier os fails=0        (exit=0)
  notifier host fails=0      (exit=0)     <- was 7 (all SET_OS)
  ✓ notifier stub-compile gate PASSED     (exit 0)

host suite: Total tests run: 64 / passed: 64 / failed: 0
SET_OS-relevant, ALL PASS:
  (ii-pre) OS_UNSURE baseline: 'iTerm' does NOT match at OS_UNSURE      [§4.7]
  (i)  SET_OS r[0]=0x51, r[1]=0x03, r[2]=ack=1                          [§4.6]
  (ii) post-SET_OS(OS_MACOS): mac_cmd fired + layer 44 selected         [§4.7]
  (iii) SET_OS change: prev on_disable + layer deactivated, no re-disp  [§4.7/F9]
  (iv)  SET_OS idempotent: no spurious disable/layer on same-OS         [§4.7/F9.3]
```

## Conclusion
**S3 is a VERIFY-AND-ALIGN task** (structurally identical to S2/QUERY_CALLBACK). The
SET_OS handler + the framing fix that unblocks it are COMMITTED and green. The expected
deliverable is a verification report with an **empty `git diff`**. The dominant risk this
PRP neutralizes is a regression from implementing the NAIVE contract text verbatim (which
is UNREACHABLE — see framing_blocker.md — and would re-introduce BUG-1).

## Why the naive contract is dangerous here (3 deltas)
1. **THE BLOCKER (hidden by contract):** SET_OS `cmd_id` is `0x03` == ETX terminator
   `0x03`. A handler that "just reads os_byte and calls notifier_set_os" NEVER RUNS —
   `hid_notify`'s byte loop terminates on the cmd_id byte before the handler is reached.
   `OS_MACOS==3` makes the os_byte collide too. Requires the length-aware typed
   reassembly fix (committed in 8441af2). See framing_blocker.md.
2. **`data[2]` not `data[1]`:** the `[0x81][0x9F]` magic header is stripped (`data += 2`)
   before reassembly, so inside `handle_typed_command`: `data[0]=0xF0, data[1]=cmd_id,
   data[2]=os_byte`. Same delta as QUERY_CALLBACK (S2).
3. **`apply_os_change()` not `notifier_set_os()`:** functionally IDENTICAL
   (`notifier_set_os` is a one-line forwarder to `apply_os_change`). The code calls the
   shared seam directly — documented as correct ("F9 clear-on-change is NOT duplicated").
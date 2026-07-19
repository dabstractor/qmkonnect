# REVISE-Point Analysis — the framing change the contract demands is ALREADY DONE

The work-item contract for P1.M2.T2.S4 contains a prominent **REVISE** note:

> *"REVISE: typed commands MUST be ETX-framed and multi-report reassembled, same as strings.
> The handle_typed_command must reassemble the full typed payload across reports before
> parsing. This means the 0xF0 check should happen AFTER ETX reassembly, not per-report.
> See HOST_RULES.md §5: 'Framing: ETX-framed and multi-report like strings.'"*

It then prescribes a mechanism:

> *"In hid_notify(), after ETX triggers msg_buffer dispatch, check if msg_buffer[0]==0xF0
> before calling process_full_message. If 0xF0: route to handle_typed_command(msg_buffer,
> msg_index) which parses the full reassembled payload."*

## The SEMANTICS the contract wants are CORRECT and ALREADY SHIPPED

What the contract is really asking for — "the handler must run on the FULL reassembled
payload, not a single report, and must bypass `process_full_message`" — is **exactly what
exists** at HEAD `8441af2`:

1. **Multi-report reassembly** — the byte loop appends every payload byte to `msg_buffer`
   across reports (the `static` `msg_index` survives `hid_notify` calls).
2. **Length-aware literal consumption** — `typed_literal_remaining` consumes the command's
   KNOWN arity (disc + cmd_id + fixed args + variable ids tail for AHC) literally, so a `0x03`
   byte *inside* the binary payload never terminates reassembly early.
3. **Dispatch AFTER reassembly** — at the ETX boundary (only reached once args are fully
   consumed), `handle_typed_command(msg_buffer, msg_index)` runs on the complete buffer.
4. **Bypass of process_full_message** — `if (typed_mode) { handle_typed_command(...); } else
   { process_full_message(...); }` (notifier.c:868) — typed never touches the board
   disable/deactivate path.

This is proven by the **(multi-rep)** host test: a 28-id AHC split across two reports
reassembles and dispatches with `r[1]==0x05` (the cmd_id persisted across the report
boundary), host layer 224 active, and the callback diff run. That test PASSES.

## The literal MECHANISM the contract prescribes ("check msg_buffer[0]==0xF0 after ETX")
## is WRONG — and would REGRESS the firmware if implemented verbatim

The implemented design classifies on the **first report** into a persistent `typed_mode`
flag (`if (msg_index==0 && length>=3 && data[2]==NOTIFY_CMD_DISCRIMINATOR) typed_mode=true`,
notifier.c:835), NOT by checking `msg_buffer[0]` at the ETX boundary. Why this matters:

### Why first-report classification + length-aware reassembly is REQUIRED

The length-aware reassembly (`typed_literal_remaining`) can only consume binary args
literally **if the byte loop already knows it is inside a typed command**. That knowledge
must exist *during* the byte loop, byte-by-byte — it cannot be deferred to the ETX boundary,
because by ETX the loop has already (wrongly) terminated early on the first `0x03` it saw
inside the payload. Concrete failures the literal "check after ETX" approach would cause:

- **BUG-1 (SET_OS, sibling task T2.S3):** SET_OS cmd_id `0x03 == ETX 0x03`. Without
  first-report `typed_mode` seeding the literal counter, the byte loop terminates at
  `msg_index==1` (the cmd_id byte). At ETX you'd "check msg_buffer[0]" — but msg_buffer[0]
  is the `0xF0` discriminator, so you'd route to `handle_typed_command` with a **truncated
  buffer** (cmd_id never even accumulated). SET_OS never dispatches. (The committed
  `typed_mode` seed consumes disc+cmd_id literally first.)
- **BUG-2 (AHC with a `0x03` arg):** if `layer`, `flags`, `count`, or any id byte is `0x03`,
  the loop terminates early. Same truncation. AHC with e.g. `count==3` or an `id==3` would
  silently truncate. (The committed `typed_fixed_arg_bytes(AHC)=3` + the variable ids-tail
  accounting at `msg_index==5` consume the full header + `count` ids literally.)
- **Multi-report classification break:** the contract's own earlier (naive) sketch checked
  the discriminator *per report after strip*; continuation reports carry payload at `data[2]`,
  which may coincidentally be `0xF0`, mis-classifying a continuation as a new typed command.
  Only a first-report classification into a persistent flag is correct. (See the T1.S1 PRP's
  regression warning for the full argument.)

### Why `msg_buffer[0]==0xF0` after ETX happens to "look equivalent" but isn't the mechanism

It is *true* that for a typed command `msg_buffer[0] == 0xF0` at ETX (the discriminator is
the first byte reassembled after magic strip), so `if (msg_buffer[0]==0xF0)` would route
correctly *in the happy path*. But it is **strictly weaker** than the `typed_mode` flag
because it provides no signal during the byte loop to drive length-aware reassembly. The
`typed_mode` flag is the durable form of "msg_buffer[0]==0xF0" carried from the first report
through every subsequent byte. **Do not "simplify" `if (typed_mode)` into a post-hoc
`msg_buffer[0]` check, and do not move the discriminator classification off the first
report.** Both re-introduce BUG-1/BUG-2 and break multi-report classification.

## The other REVISE concern — "removes the ≤26 callbacks per report cap" — is ALSO DONE

The old (withdrawn) v1 single-report sketch limited AHC to ≤26 callbacks (the 30-byte report
tail minus header). The shipped design removes that cap via:
- `typed_fixed_arg_bytes(APPLY_HOST_CONTEXT) == 3` (notifier.c:134) — the fixed header.
- The variable-tail accounting at `msg_index==5` (notifier.c:921-926): once the fixed header
  is reassembled, read `count = msg_buffer[4]` and add `count` more literal bytes for the ids
  tail (clamped to `MSG_BUFFER_SIZE-1` room, so a malicious `0xFF` count can't overflow).
- The handler's count clamp: `max_ids = min(MSG_BUFFER_SIZE-5, len-5)` — max **251 ids**,
  bounded by the 256-byte reassembly buffer, not by a single 30-byte report.

So AHC may span **many** reports (30 payload bytes/report). The (multi-rep) test with
`count=28` proves the two-report case.

## Bottom line for the implementer

The contract's REVISE describes the **correct end-state semantics** (handler on reassembled
buffer, bypasses `process_full_message`, multi-report, no ≤26 cap) and prescribes a
**mechanism that is partially wrong** (defer the 0xF0 check to after ETX). The shipped code
achieves the correct semantics via the **right** mechanism (first-report `typed_mode` +
length-aware `typed_literal_remaining`). **Align your understanding to the code; do NOT align
the code to the contract's literal "move the check after ETX" prescription.** Doing so would
re-introduce BUG-1 (SET_OS undeliverable), BUG-2 (AHC truncation on a `0x03` arg), and break
multi-report classification — the (multi-rep) and all SET_OS host tests would fail.
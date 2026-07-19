# The 0x03 == ETX Framing Blocker (BUG-1) + The Fix — P1.M2.T2.S3

## The flaw (why the naive SET_OS contract is UNREACHABLE)

`hid_notify`'s reassembly byte loop treats EVERY `0x03` byte as the ETX message terminator:

```c
for (uint8_t i = 0; i < length; i++) {
    char c = (char)data[i];        // data points PAST the 2-byte [0x81][0x9F] magic header
    if (c == ETX_TERMINATOR[0]) {  // 0x03 -> dispatch immediately
        ... handle_typed_command(msg_buffer) ...
        break;
    } else { msg_buffer[msg_index++] = c; }
}
```

SET_OS's post-magic byte stream is `[0xF0, 0x03(cmd_id), os_byte, 0x03(ETX)]`. The loop
appends `0xF0` (msg_index 0->1), then sees `0x03` (the **cmd_id**) and **dispatches
prematurely** with `msg_buffer=[0xF0, 0, ...]`. `handle_typed_command` reads
`cmd_id = data[1] = 0` -> **default case** -> `send_typed_response(0, NULL, 0)` ->
response `[0x51][0x00][0x00...]`. **SET_OS never runs; current_os never changes.**

### It is WORSE than just the cmd_id
`OS_MACOS == 3` (`os_variant_t` enum, `qmk_stubs/os_detection.h`). So the **os_byte
argument** for macOS is ALSO `0x03`. Even a partial fix that only exempts the cmd_id
byte (e.g. `!(typed_mode && msg_index < 2)`) leaves SET_OS(OS_MACOS) broken: the os_byte
`0x03` would terminate the message before being appended -> `current_os` never becomes MACOS.

**Root cause:** ETX-termination is fundamentally incompatible with BINARY typed payloads.
Legacy strings are text (`0x20-0x7E`), so `0x03` never appears in them. Typed commands
carry arbitrary binary (cmd_id, os_byte, layer, flags, count, ids) which CAN be `0x03`.
The PRD §4.6 framing reuses string ETX-framing for binary -> self-contradictory whenever
any payload byte == 0x03.

### Scope of impact (probe-verified against pre-fix notifier.c)
| Test | contains 0x03? | pre-fix verdict |
|------|----------------|-----------------|
| SET_OS (i) response       | cmd_id=0x03 | BLOCKED [0x51][0x00] |
| SET_OS (ii) current_os    | cmd_id=0x03 | BLOCKED (never changes) |
| SET_OS (iii) F9 clear     | cmd_id=0x03 | BLOCKED |
| SET_OS (iv) idempotent    | cmd_id=0x03 | BLOCKED |
| AHC (v-viii)              | no          | PASS |

The 7 host failures (pre-fix) were exactly these 4 SET_OS blocks + the (ii) split = the
SET_OS family. AHC passed because its args (224/0/1/0xFF, ids 0/1) contain no 0x03.

This blocker was documented by:
- Firmware plan/003 P1.M3.T1.S3 `findings.md` -> "out of scope for this test-only task,
  owned by P1.M2 (dispatcher layer)".
- QMKonnect plan/002 S2 PRP -> "7 host failures, ALL SET_OS (0x03==ETX blocker)".

## The fix (COMMITTED in `8441af2`) — length-aware typed reassembly

The fix is **length-aware reassembly**: once `typed_mode` is set and `cmd_id` is known,
the byte loop consumes the command's argument bytes LITERALLY (`0x03` included) and only
honors `0x03` as the terminator AFTER the full argument set is accumulated.

### Pieces (all in `notifier.c`, HEAD `8441af2`)
1. **`static uint16_t typed_literal_remaining = 0;`** (L115) — count of literal arg
   bytes still expected. Persists across `hid_notify` calls (like `msg_index`/`typed_mode`)
   so multi-report typed messages reassemble correctly; reset at every ETX/overflow boundary.

2. **`static uint16_t typed_fixed_arg_bytes(uint8_t cmd_id)`** (L129) — fixed arg byte
   count per cmd_id (EXCLUDING discriminator + cmd_id + variable tail):
   - QUERY_INFO (0x01): 0
   - QUERY_CALLBACK (0x02): 1 (index)
   - SET_OS (0x03): 1 (os_byte)
   - APPLY_HOST_CONTEXT (0x05): 3 (layer, flags, count) — the variable ids tail is added
     after `count` is read.
   - default: 0xFFFF (treated as 0 so unknown commands still terminate at ETX -> default ack).

3. **Seed on typed entry** (L836-837): when `msg_index==0 && data[2]==0xF0`:
   `typed_mode = true; typed_literal_remaining = 2;` (consume discriminator + cmd_id literally).

4. **ETX gate** (L862): `if (c == ETX_TERMINATOR[0] && !typed_literal)` where
   `bool typed_literal = (typed_mode && typed_literal_remaining > 0);` (L858). Only honor
   ETX when NOT mid-literal.

5. **Literal accounting on append** (~L906-925): if `typed_literal`, decrement; at
   `msg_index==2` add `typed_fixed_arg_bytes(cmd_id)`; at `msg_index==5` for AHC read
   `count=msg_buffer[4]` and add `count` (clamped to `(MSG_BUFFER_SIZE-1)-msg_index` so a
   malicious 0xFF count cannot overrun).

6. **Reset** at every ETX boundary (L890) and overflow (L935): `typed_literal_remaining = 0;`
   (alongside `typed_mode = false`).

7. **`handle_typed_command(char *data, uint16_t len)`** (L700) — now takes the reassembled
   length (BUG-3 hardening): each case validates `len` >= its minimum footprint before
   indexing args; a truncated frame falls through to the safe default no-payload ack.
   Called as `handle_typed_command(msg_buffer, msg_index)` (L875).

### SET_OS handler (L747-756) — the deliverable
```c
case NOTIFY_CMD_SET_OS: {
    if (len < 3) {                                    /* BUG-3: disc + cmd + os_byte */
        send_typed_response(cmd_id, NULL, 0);
        break;
    }
    uint8_t os_byte = (uint8_t)data[2];               /* ARG[0] -- data[2], NOT data[1] */
    apply_os_change((os_variant_t)os_byte);           /* shared F9 seam (notifier_set_os delegates here) */
    uint8_t payload[1] = { 0x01 };                    /* ack = 1 (applied) */
    send_typed_response(NOTIFY_CMD_SET_OS, payload, 1);
    break;
}
```
Wire response: `[0x51][0x03][0x01]` (+ 29 zero-pad bytes = 32 total via `send_typed_response`).

### Test restructuring (test_notifier_host.c)
The SET_OS blocks (i-iv) were previously marked "VERIFIED BLOCKER" (written to FAIL,
documenting the flaw). Now updated to "RESOLVED". Block (ii) was split: the OS_UNSURE
baseline check moved to a new `(ii-pre)` block BEFORE the SET_OS(OS_MACOS) call, because
once BUG-1 is fixed, SET_OS(OS_MACOS) legitimately changes current_os — so you can no
longer assert the OS_UNSURE baseline AFTER calling SET_OS.

## Why this is the correct fix design (not the alternatives)
The firmware plan/003 P1.M3.T1.S3 findings listed two alternatives: (1) length-prefixed
framing (changes the wire format), (2) escape 0x03 (0x1B 0x03). The committed fix is a
third, superior option: **length-aware reassembly using the command's known arity**. It:
- requires NO wire-format change (ETX framing preserved, host unchanged),
- requires NO escaping (no byte-stuffing overhead),
- is fully backward-compatible (legacy strings never set `typed_mode`, so the literal path
  never activates for them — their ETX behavior is byte-identical),
- generalizes to all typed commands via the `typed_fixed_arg_bytes` arity table + AHC's
  variable-length `count` extension.

The only residual constraint: a typed command's args are consumed literally up to its KNOWN
arity, then the next 0x03 is ETX. This is correct because every typed command has a
well-defined arity (the table is the contract). Unknown cmd_ids (0xFFFF) get 0 literal bytes
-> terminate at the very next 0x03 -> default-case ack (safe placeholder).
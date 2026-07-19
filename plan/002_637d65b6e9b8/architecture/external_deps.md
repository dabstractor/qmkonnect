# External Dependencies — State & Required Changes

## 1. qmk_notifier Crate (Rust Transport)

**Repo:** `/home/dustin/projects/qmk_notifier`
**Current tag:** `v0.2.1` (pinned in qmkonnect/Cargo.toml:16)
**File:** `src/lib.rs` (~230 lines), `src/core.rs` (~350 lines), `src/error.rs` (~70 lines)

### Current API (v0.2.1)

```rust
pub enum RunCommand {
    SendMessage(String),
    ListDevices,
}

pub struct RunParameters {
    pub command: RunCommand,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub usage_page: u16,
    pub usage: u16,
    pub verbose: bool,
}

pub fn run(params: RunParameters) -> Result<(), QmkError>;
pub fn send_raw_report(data, vid, pid, page, usage, verbose) -> Result<(), QmkError>;
```

### QmkError Variants (11 total, src/error.rs)
- `HidApiInitError(String)`
- `DeviceNotFound { vendor_id, product_id, usage_page, usage }`
- `DeviceOpenError(String)`
- `InvalidHexValue(String)`, `InvalidDecimalValue(String)`
- `SendReportError(HidError)`
- `HidReadError(String)` — **forward-looking placeholder, unused in v0.2.1**
- `NoResponseReceived(String)` — **forward-looking placeholder, unused in v0.2.1**
- `MissingRequiredParameter(String)`, `RemovedFeature(String)`
- `PartialSendError { succeeded, failed }`

### Device Cache Infrastructure (src/core.rs)
- `static DEVICE_CACHE: LazyLock<Mutex<Option<DeviceCache>>>` — process-global
- `MatchKey { vendor_id, product_id, usage_page, usage }` — cache rebuild trigger
- `ensure_cache()` — rebuild only on key change or write failure
- `burst_to_one()` — writes all reports to one device, drains IN acks (bounded `IN_DRAIN_MAX=32`)
- **Currently drains/discards IN replies; v0.3.0 must READ and PARSE them**
- `SEND_RETRIES = 1` — one retry on TotalFailure after cache rebuild

### Framing Constants (src/core.rs)
```rust
const PAYLOAD_PER_REPORT: usize = REPORT_LENGTH - 2;  // 30
const IN_DRAIN_MAX: usize = 32;
const SEND_RETRIES: usize = 1;
// Buffer layout: [0x00, 0x81, 0x9F, <30 payload bytes>]
```

### Required Changes for v0.3.0

1. **New RunCommand variants:** `QueryInfo`, `QueryCallback(u8)`, `SetOs(HostOs)`, `ApplyHostContext { layer: Option<u8>, callbacks: Vec<u8>, clear_board: bool }`
2. **New HostOs enum:** `Unsure=0, Linux=1, Windows=2, Macos=3, Ios=4`
3. **New CommandResponse enum:** `Legacy{matched}`, `Info{proto_ver, feature_flags, callback_count, board_rules_present}`, `CallbackName{index, name}`, `Ack{ok}`, `Timeout`
4. **run() return type:** `Result<(), QmkError>` → `Result<CommandResponse, QmkError>`
5. **Typed-command framing:** `[0x81, 0x9F, 0xF0, cmd_id, args…]` + ETX, multi-report chunked (same as strings, 30 payload bytes/report)
6. **Response parsing:** read one 32-byte IN report; `response[0]==0x51` → typed; `{0,1}` → legacy; no reply → Timeout

### Breaking Change Alert
When qmkonnect bumps to v0.3.0, `run()`'s return type changes from `()` to `CommandResponse`. qmkonnect's current `Ok(_)` match arm still compiles, but typed-command callers need to pattern-match the new return.

---

## 2. qmk-notifier Firmware (C Module)

**Repo:** `/home/dustin/projects/qmk-notifier`
**Files:** `notifier.c` (577 lines), `notifier.h` (~200 lines), `pattern_match.c` (628 lines)

### Already Implemented (Scaffolded)

| Feature | Location | Status |
|---------|----------|--------|
| `host_callback_t` struct | notifier.h | ✅ `{ name, on_enable, on_disable }` |
| `DEFINE_HOST_CALLBACKS` macro | notifier.h | ✅ Declares `user_host_callbacks[]` + accessors |
| `get_host_callbacks()` / `_size()` | notifier.c:140-141 | ✅ Weak defaults `{NULL, 0}` |
| `host_layer` state | notifier.c:137 | ✅ `static uint8_t = LAYER_UNSET (255)` |
| `host_cb_enabled[]` state | notifier.c:138 | ✅ `static bool[HOST_CALLBACK_MAX=32] = {false}` |
| `has_been_queried` flag | notifier.c:139 | ✅ `static bool = false` |
| Typed-command constants | notifier.h | ✅ `NOTIFY_CMD_*`, `NOTIFY_RESPONSE_MARKER=0x51`, `NOTIFY_PROTO_VER=2` |
| Feature flags | notifier.h | ✅ `NOTIFY_FEATURE_APPLY_HOST_CONTEXT=0x01`, `_CALLBACK_REGISTRY=0x02`, `_VIA_COEXIST=0x04` |
| `HOST_LAYER_BASE=224` | notifier.h | ✅ |
| `HOST_CALLBACK_MAX=32` | notifier.h | ✅ |
| `board_rules_present()` | notifier.c:193 | ✅ Checks all default + per-OS maps |
| `notifier_set_os()` | notifier.c | ✅ Idempotent OS selector with state clear |
| Multi-OS overlay | notifier.c/h | ✅ `DEFINE_SERIAL_COMMANDS_OS`, `select_*_map_os()` |
| Per-OS weak accessors | notifier.c | ✅ All 4 OSes × 2 map types |

### NOT Yet Implemented (Missing)

| Feature | Required |
|---------|----------|
| `0xF0` discriminator check in `hid_notify()` | Check `data[2]==0xF0` before legacy processing |
| `handle_typed_command()` dispatcher | Route to QUERY_INFO / QUERY_CALLBACK / SET_OS / APPLY_HOST_CONTEXT |
| `set_host_layer(layer)` | on/off/clear the host tracker only (independent of board) |
| `apply_host_callbacks(ids, count)` | disable-before-enable diff against `host_cb_enabled[]` |
| `QUERY_INFO` response builder | `[0x51][0x01][proto_ver][feature_flags][callback_count][board_rules_present]` |
| `QUERY_CALLBACK` response builder | `[0x51][0x02][index][name, NUL-padded]` |
| `SET_OS` handler | Update `current_os` + return ack |
| `APPLY_HOST_CONTEXT` handler | Honor `clear_board` flag, then `set_host_layer()` + `apply_host_callbacks()` |
| Typed-command response via `raw_hid_send` | 32-byte response with `[0x51][cmd_id_echo][payload…]` |

### Key: hid_notify() Currently (lines 543-577)
```c
void hid_notify(uint8_t *data, uint8_t length) {
    if (length < 2 || data[0] != 0x81 || data[1] != 0x9F) return;
    data += 2; length -= 2;
    // ... legacy string processing only ...
    // NO 0xF0 discriminator check
    // NO handle_typed_command() call
}
```

---

## 3. Pattern Matcher (pattern_match.c → src/core/pattern.rs)

**Source:** `/home/dustin/projects/qmk-notifier/pattern_match.c` (628 lines)
**Target:** `/home/dustin/projects/qmkonnect/src/core/pattern.rs` (NEW)

### Architecture: Thompson NFA (Russ Cox)

The matcher uses Thompson construction for guaranteed O(n×m) matching (no exponential backtracking):

1. **process_escapes()** — Transforms pattern string into processed-pattern bytes:
   - `\x01-\x04`: escaped literals `^ $ * \`
   - `\x05-\x0A`: classes `\d \D \w \W \s \S`
   - `\x0B-\x0C`: zero-width assertions `\b \B`
   - `\x0D`: dot metacharacter (excludes `\n`/`\r`)
   - `\x0E`: `+` quantifier marker (follows consuming element)
   - `0x2A`: glob `*` (wildcard)
   - Literal `.` and `+` emitted as ordinary ASCII

2. **parse_pattern()** — Detects anchors:
   - Leading `^` → start_anchored
   - Trailing `$` with even backslash count → end_anchored
   - Extracts core between anchors

3. **nfa_compile()** — Thompson construction into state pool:
   - `OP_CHAR`: consume one matching byte
   - `OP_ANY`: consume any byte (glob `*` = `.*`)
   - `OP_SPLIT`: epsilon fork (for `*` and `+`)
   - `OP_ASSERT`: zero-width `\b`/`\B`
   - `OP_MATCH`: accepting state
   - `X+` compiles to exactly 2 states (CHAR + SPLIT loop-back)

4. **nfa_addstate()** — Epsilon-closure with `lastlist` generation guard:
   - Follows OP_SPLIT both branches without consuming input
   - OP_ASSERT conditionally recurses based on `is_word_boundary()`
   - `lastlist == nfa_gen` prevents infinite recursion

5. **nfa_match()** — Two-list simulation:
   - Maintains `clist` (current) and `nlist` (next) state lists
   - `nfa_gen` bumped once per phase for O(1) dedup
   - `full_match` flag: prefix/substring vs consume-whole-string

6. **match_with_anchors()** — Anchor strategy:
   - `^...$`: one full match from offset 0
   - `^...`: one reach-any match from offset 0
   - `...$`: loop offsets, full match from each
   - `...`: loop offsets, reach-any from each

### Character Classifiers
- `is_digit_char(c)`: `c >= '0' && c <= '9'`
- `is_word_char(c)`: `[a-zA-Z0-9_]`
- `is_whitespace_char(c)`: ` \t\n\r\f\v`
- `is_word_boundary(str, pos)`: XOR of neighboring word-char-ness

### Delimiter-Aware Matcher (in notifier.c, not pattern_match.c)
`match_pattern(pattern, message, case_sensitive)` in notifier.c:
- If pattern has GS (`\x1D`) but message doesn't → match pattern_left against whole message
- If message has GS but pattern doesn't → match pattern against msg_left (class only)
- If both have GS → split both, match both halves independently
- Neither has GS → direct pattern_match

### Firmware Test Corpus (for parity validation)
Test files in `/home/dustin/projects/qmk-notifier/`:
- `test_pattern_match.c` — basic patterns
- `test_char_classification.c` — class predicates
- `test_metachar_verification.c` — escape sequences
- `test_word_boundary_basic.c`, `test_word_boundary_integration.c`
- `test_comprehensive_integration.c` — end-to-end
- `test_invalid_patterns.c`, `test_error_handling.c`
- `test_memory_stress.c` — NFA sizing
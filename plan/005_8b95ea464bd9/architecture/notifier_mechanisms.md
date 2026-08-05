# Notifier Mechanisms — Existing State Reused by F13/F14

## Tier-1 Presence: `is_device_connected()` — notifier.rs:216-232

Pure enumeration via `hidapi::HidApi::new()`. **Never opens, never sends.** Matches
`usage_page == configured usage_page && usage == configured usage` with optional
VID/PID narrowing. Returns `bool`.

```rust
pub fn is_device_connected() -> bool {
    let f = configured_filter();
    match hidapi::HidApi::new() {
        Ok(api) => api.device_list().any(|d| {
            d.usage_page() == f.usage_page
                && d.usage() == f.usage
                && f.vendor_id.is_none_or(|v| d.vendor_id() == v)
                && f.product_id.is_none_or(|p| d.product_id() == p)
        }),
        Err(_) => false,
    }
}
```

**Retained as-is.** Still used by: the poll threads, the broadcast write decision, and
the future `classify_devices()` Tier-1 pass.

---

## Tier-2 Capability: `host_capable()` + `HOST_CAPABLE` — notifier.rs:270, 689

### The Global
```rust
static HOST_CAPABLE: AtomicBool = AtomicBool::new(false);  // line 270
```

### Reader
```rust
pub fn host_capable() -> bool {                              // line 689
    HOST_CAPABLE.load(Ordering::SeqCst)
}
```

### Set on Gain (capable reply)
In `perform_handshake_with` (line 421), after matching `CommandResponse::Info { proto_ver: 2, feature_flags, .. }`:
```rust
BOARD_HAS_RULES.store(board_rules_present, Ordering::SeqCst);  // line 557 (BEFORE)
HOST_CAPABLE.store(true, Ordering::SeqCst);                     // line 558 (AFTER)
```
**Ordering invariant:** BOARD_HAS_RULES set before HOST_CAPABLE so no window exists
where `host_capable()==true` but `board_has_rules()` reads stale `false`.

### Reset on Loss / Failure
- Timeout arm: line 576
- Non-capable reply arm: line 588
- Device-error arm: line 600
- `reset_handshake_state()`: line 706

---

## `perform_handshake_with(verbose, opts)` — notifier.rs:421-616

The Tier-2 mechanism. The existing handshake **IS** the capability probe — it sends
`QUERY_INFO` and classifies the reply. **No change needed to the handshake itself.**

Key flow:
1. Dedup gate: `HAS_HANDSHAKED.swap(true, SeqCst)` (lines 423-430)
2. Build `configured_filter()`, lock NOTIFIER
3. Send: `n.send_command(RunCommand::QueryInfo, &filter)` (line 436)
4. Capable arm (Info{proto_ver:2, flags&0x01}): SetOs → callback sweep → publish CALLBACK_NAMES → set BOARD_HAS_RULES + HOST_CAPABLE
5. Failure arms: clear HOST_CAPABLE + CALLBACK_NAMES, manage HAS_HANDSHAKED

---

## `handshake_action(prev, now)` — notifier.rs:745-751

```rust
pub fn handshake_action(prev: Option<bool>, now: bool) -> HandshakeAction {
    match (prev, now) {
        (Some(true), false) => HandshakeAction::Loss,
        (p, true) if p != Some(true) => HandshakeAction::Gain,
        _ => HandshakeAction::None,
    }
}
```

Consumed by both poll threads (`tray.rs:380-406`, `linux_tray.rs:259-301`). **Unchanged.**

---

## `reset_handshake_state()` — notifier.rs:705-710

```rust
pub fn reset_handshake_state() {
    HOST_CAPABLE.store(false, Ordering::SeqCst);
    BOARD_HAS_RULES.store(false, Ordering::SeqCst);
    CALLBACK_NAMES.lock().unwrap().clear();
    HAS_HANDSHAKED.store(false, Ordering::SeqCst);
}
```

Called on `HandshakeAction::Loss` by both poll threads.

---

## Three-State Derivation (§2.1 — the headline value)

| Status | Derivation from existing state |
|--------|-------------------------------|
| **Disconnected** | `!is_device_connected()` (0 Tier-1 boards) |
| **NoModule** | `is_device_connected() && !host_capable()` (≥1 Tier-1, 0 capable) |
| **Connected** | `is_device_connected() && host_capable()` (≥1 capable) |

**No new pinging function required for the status line.** The two booleans are
already maintained by the existing poll thread lifecycle.

**Transient caveat:** right after a Gain, `host_capable()` is false until
`perform_handshake` completes (sub-second). The line may briefly read "No module"
before flipping to "Connected". Acceptable per spec.

---

## Once-Guard Pattern — notifier.rs:299, 1085-1101

Canonical "fire at most once per broken state" idiom:

```rust
static RULES_INVALID_NOTIFIED: AtomicBool = AtomicBool::new(false);

// On entry to broken state:
if !FLAG.swap(true, Ordering::SeqCst) { /* fire once */ }

// On recovery:
FLAG.store(false, Ordering::SeqCst);
```

This is the reference pattern for the Linux Disconnected→NoModule one-shot notification.

---

## Mock Test Infrastructure — notifier.rs:1278-1370

```rust
#[cfg(test)]
pub fn set_notifier(notifier: Box<dyn Notifier>)  // line 903 — inject mock

struct MockNotifier;  // line 1278 — records calls, returns queued responses
```

Test setup pattern:
```rust
reset_test_state();
reset_handshake_state();
set_notifier(Box::new(MockNotifier::new()));
MockNotifier::set_mock_responses(vec![...]);
perform_handshake(false);
assert!(host_capable());
```

**All tests must run `--test-threads=1`** (shared mock globals + DebounceState).
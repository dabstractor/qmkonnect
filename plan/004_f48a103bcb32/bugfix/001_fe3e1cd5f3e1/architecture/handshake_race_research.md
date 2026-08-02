# Handshake Mutex-Hold (#4) & First-Tick Device-Absent Race (#5) — Research

Scope: trace the exact code paths behind two bug-hunt findings about QMKonnect's
host-rules capability handshake, identify the mutex held during the handshake,
and map the runner/poll-thread reconnect lifecycle and `HAS_HANDSHAKED` state.

---

## TL;DR verdict

| # | Finding | Verdict | Severity |
|---|---------|---------|----------|
| 4 | `QUERY_CALLBACK` sweep runs under the notifier lock → window notifications block | **CONFIRMED.** It is the **`NOTIFIER` mutex** (`Arc<Mutex<Box<dyn Notifier>>>`), and both window-send paths serialize on it. The 5 s `CALLBACK_SWEEP_DEADLINE` (plus `QueryInfo` + `SetOs`, each up to ~1 s) bounds the stall. | **Low–Medium** — *known, intentionally mitigated* design tradeoff (code comments acknowledge it). Real residual: ~5–8 s worst-case notification stall only on a misbehaving/buggy firmware. |
| 5 | First-tick transient `is_device_connected()==false` seeds poll state without recording a `Loss`; `HAS_HANDSHAKED` stays true; later reconnect short-circuits (no `SET_OS`) | **CONFIRMED.** `handshake_action(None, false) == None`, so `last` becomes `Some(false)` without a reset; the only reset path is `(Some(true), false) == Loss`. | **Medium** — narrow trigger window (real power-cycle in the ~3 s between startup handshake and the poll thread recovering `last` to `Some(true)`), but a genuine lifecycle inconsistency; `SET_OS` is skipped on one reconnect. |

---

## Finding #4 — Handshake holds the NOTIFIER mutex during the sweep

### The mutex in question

It is the **`NOTIFIER`** static, **not** the `STATE` (DebounceState) mutex and
**not** the `CALLBACK_NAMES` mutex:

`src/core/notifier.rs:764`
```rust
static NOTIFIER: Lazy<Arc<Mutex<Box<dyn Notifier>>>> =
    Lazy::new(|| Arc::new(Mutex::new(Box::new(QmkNotifier) as Box<dyn Notifier>)));
```

`src/core/notifier.rs:829`
```rust
fn get_notifier() -> Arc<Mutex<Box<dyn Notifier>>> {
    Arc::clone(&NOTIFIER)
}
```

### Where it is acquired and held

`perform_handshake_with` — `src/core/notifier.rs:388`. Key lines:

- `src/core/notifier.rs:390` — dedup guard (swap `HAS_HANDSHAKED` to true, short-circuit if already set).
- `src/core/notifier.rs:401` — **lock acquired**: `let n = notifier.lock().unwrap();` where `notifier = get_notifier()`.
- `src/core/notifier.rs:407` — `n.send_command(QueryInfo, &filter)` (under lock; up to ~1 s transport timeout).
- `src/core/notifier.rs:420-424` — `n.send_command(SetOs(host_os()), &filter)` (under lock; up to ~1 s).
- `src/core/notifier.rs:430-481` — **the `QUERY_CALLBACK` sweep loop, under lock**:
  ```rust
  let sweep_start = Instant::now();
  let sweep_cap = callback_count.min(MAX_HOST_CALLBACKS);          // 64 cap, line 431
  ...
  for i in 0..sweep_cap {
      if sweep_start.elapsed() > CALLBACK_SWEEP_DEADLINE { ... break; }  // 5 s, line 441
      match n.send_command(qmk_notifier::RunCommand::QueryCallback(i), &filter) { ... }
  }
  ```
- `src/core/notifier.rs:484` — **`drop(n);`** releases the lock *after* the sweep, before publishing `CALLBACK_NAMES` (lines 486-490) and the read-only `validate_rules_callback_names` (line 494). The comment "publish after dropping the notifier lock: D2" confirms the intent was to defer only the *publish*, not the sweep I/O.

The non-capable / `Timeout` / `Err` arms each `drop(n)` early (lines ~517/531/543) before clearing state — they do not sweep, so they are short.

Constants — `src/core/notifier.rs:370,379`:
```rust
const MAX_HOST_CALLBACKS: u8 = 64;
const CALLBACK_SWEEP_DEADLINE: Duration = Duration::from_secs(5);
```

### Why window notifications block

Both window-notification **send** paths acquire the **same `NOTIFIER` mutex** to
do the actual HID write:

- **Immediate send** — `notify_qmk` `src/core/notifier.rs:919`, lock at `956-957`:
  ```rust
  let notifier = get_notifier();
  let notifier = notifier.lock().unwrap();
  let _res = dispatch_window_send(&**notifier, &filter, &message, ctx, "immediate", verbose);
  ```
- **Debounced send** — `debounce_worker` `src/core/notifier.rs:838`, lock at `888-889`:
  ```rust
  let notifier = get_notifier();
  let notifier = notifier.lock().unwrap();
  let _res = dispatch_window_send(&**notifier, &filter, &message, ctx, "debounced", verbose);
  ```

`dispatch_window_send` (the typed `ApplyHostContext` / legacy `SendMessage` path)
calls `notifier.notify(...)` / `notifier.send_command(...)`, which must run
under the `NOTIFIER` guard because the inner `qmk_notifier::run` opens the HID
device each call (no separate inner lock). So while the handshake sweep holds
`NOTIFIER`, neither `notify_qmk`'s immediate arm nor the debounce worker's flush
can proceed → **every queued/new window notification stalls**.

(Note: the `STATE` DebounceState mutex is released *before* the `NOTIFIER` lock
is taken in both paths — `notify_qmk` dequeues at lines 927-949 then locks
`NOTIFIER` at 956; the worker dequeues inside the `STATE` scope at 843-881 then
locks `NOTIFIER` at 888. So the contention is purely on `NOTIFIER`, not `STATE`.)

### Worst-case stall budget

`CALLBACK_SWEEP_DEADLINE` (5 s) bounds only the *sweep*. Each timed-out
`QueryCallback` send blocks up to ~`REPLY_READ_TIMEOUT_MS` (≈ 1 s, per the
comment at `notifier.rs:373`). The deadline check is at the **top** of each
iteration, so one in-flight ~1 s send can overshoot the 5 s mark. Including the
pre-sweep `QueryInfo` + `SetOs` (each up to ~1 s under the same lock), the
absolute worst-case notification stall for a capable-but-slow/buggy board is
roughly **5 s (sweep) + up to ~1 s (one overdue QueryCallback) + ~1 s (QueryInfo)
+ ~1 s (SetOs) ≈ 6–8 s**. A healthy board (handful of callbacks, each replying
in well under a second) completes the sweep in tens of milliseconds.

### Assessment

This is a **known, intentionally-mitigated** tradeoff: the comments at
`notifier.rs:372-378` and `427-429` explicitly state the deadline+cap exist "so a
misbehaving firmware cannot wedge the global notifier mutex (and every
notification behind it)." The residual risk is the bounded stall above, only on a
buggy board. A cleaner (non-blocking) design would release/re-acquire `NOTIFIER`
per sweep iteration, or run the sweep without holding the lock (e.g. a short-lived
owned handle), but the 5 s deadline makes the current design tolerable.

---

## Finding #5 — First-tick device-absent race skips the `Loss` that resets `HAS_HANDSHAKED`

### The lifecycle primitives

- `HAS_HANDSHAKED: AtomicBool` — `src/core/notifier.rs:260`. Dedup token: "at most one handshake per board boot" (firmware sets `has_been_queried` on the first `QUERY_INFO`).
- `reset_handshake_state()` — `src/core/notifier.rs:649-654`: clears `HOST_CAPABLE`, `BOARD_HAS_RULES`, `CALLBACK_NAMES`, and `HAS_HANDSHAKED`.
- `handshake_action(prev, now)` — `src/core/notifier.rs:689-693`:
  ```rust
  match (prev, now) {
      (Some(true), false) => HandshakeAction::Loss,              // the ONLY reset trigger
      (p, true) if p != Some(true) => HandshakeAction::Gain,     // None→true OR false→true
      _ => HandshakeAction::None,                                // no change OR None→false
  }
  ```
  Key consequence: **`(None, false) → None`** — a first-tick "absent" neither
  runs a handshake (Gain) nor resets (Loss).

### Startup handshake (runs BEFORE the poll thread exists)

All three runners do the same thing on the main thread:

- `src/runners/linux.rs:31-33`, `src/runners/macos.rs:31-33`, `src/runners/windows.rs:52-54` (console) and `105-107` (tray app):
  ```rust
  if crate::core::notifier::is_device_connected() {
      crate::core::notifier::perform_handshake(self.verbose);
  }
  ```
  → If the device is present at startup, `perform_handshake` runs: `HAS_HANDSHAKED`
  becomes `true`, and on a capable board `HOST_CAPABLE` becomes `true` and `SET_OS`
  is sent **once**. The comments explicitly note this "completes before the poll
  thread exists; idempotent via `HAS_HANDSHAKED`."

### The poll-thread loop (where the race lives)

**macOS/Windows** — `src/tray.rs:384-409` (spawned inside `setup_tray`):
```rust
let mut last: Option<bool> = None;
loop {
    let connected = crate::core::notifier::is_device_connected();
    if last != Some(connected) {
        match crate::core::notifier::handshake_action(last, connected) {
            HandshakeAction::Gain  => { perform_handshake(verbose); }
            HandshakeAction::Loss  => { reset_handshake_state(); }
            HandshakeAction::None  => {}
        }
        last = Some(connected);
        let _ = status_proxy.send_event(UserEvent::DeviceStatus(connected));
    }
    std::thread::sleep(std::time::Duration::from_secs(3));
}
```

**Linux** — `src/linux_tray.rs:261-294`: identical `last_device: Option<bool> = None`
seed, same `handshake_action` dispatch, polls every `DEVICE_POLL_INTERVAL`.

### The race, step by step

1. **Startup (main thread):** device present → `perform_handshake()` →
   `HAS_HANDSHAKED=true`, `HOST_CAPABLE=true`, `SET_OS` sent. Firmware
   `has_been_queried=true`.
2. **Poll thread first tick:** `is_device_connected()` returns **`false`** due to
   a transient hidapi enumeration hiccup (the device is still physically present).
   `last = None`, so `last != Some(false)` → enter branch.
   `handshake_action(None, false) == None` → **no Gain, no Loss**. `last` becomes
   `Some(false)`.
   State now **inconsistent**: `last=Some(false)` (poll thinks absent) while
   `HAS_HANDSHAKED=true`, `HOST_CAPABLE=true` (startup thinks present). No reset
   was recorded.
3. **Device genuinely power-cycles** (unplug/replug, firmware resets
   `has_been_queried` and `current_os`) **before** the poll thread recovers
   `last` to `Some(true)`. The poll thread still reads `connected=false` →
   `handshake_action(Some(false), false) == None` → still no reset.
4. **Device returns:** `connected=true` → `handshake_action(Some(false), true) == Gain`
   → `perform_handshake()`. But `HAS_HANDSHAKED` is still `true` → **short-circuits**
   (`notifier.rs:390`). `SET_OS` is **not re-sent**. The freshly-rebooted firmware
   is missing `current_os`; board-side OS-conditional rules misbehave until a *real*
   `Some(true)→false` `Loss` is eventually observed.

`HOST_CAPABLE` stays `true` from step 1, and the callback registry is unchanged
(same firmware), so the *host-side* rules pipeline keeps working — the concrete
regression is the missing `SET_OS` on that one reconnect.

### Recovery / when it does NOT bite

If the transient clears on the **next** tick (the common case): tick 2 sees
`connected=true` → `handshake_action(Some(false), true) == Gain` →
`perform_handshake` short-circuits via `HAS_HANDSHAKED` (correct — the device
never left, firmware already has the OS), and `last` recovers to `Some(true)`.
From that point, a real power cycle yields `Some(true)→false == Loss` →
`reset_handshake_state()` → reconnect re-handshakes correctly. So the bug window
is **only the interval between the first bad tick and recovery** (≈ one
`DEVICE_POLL_INTERVAL`, ~3 s), during which a genuine power-cycle is missed.

A more degenerate (low-probability) variant: if `is_device_connected()` returns
`false` *persistently* (e.g. a stuck hidapi state) while the device is actually
present, `last` stays `Some(false)` and *every* subsequent power-cycle is missed —
but that requires a consistent enumeration failure after a successful startup
probe on the same function, which is unlikely.

### Root cause

The poll-thread `last` tracker and the `HAS_HANDSHAKED` dedup token are seeded
independently and can desync: startup seeds `HAS_HANDSHAKED=true` but the poll
thread's `last` starts at `None` and can jump to `Some(false)` without ever
passing through the `Some(true)` state that a `Loss` requires. There is no
mechanism to reconcile "poll thinks absent, but handshake already happened" —
specifically, `handshake_action(None, false)` does not reset `HAS_HANDSHAKED`,
and the startup handshake is never reported to the poll thread's `last` (the poll
thread does not know the device was present at startup).

### Severity

**Medium.** Low probability (transient first-tick hiccup *and* a real power-cycle
within one poll interval), Medium impact (missing `SET_OS` degrades board-side
OS-conditional rules for one reconnect; host rules keep working). The fix surface
is the `handshake_action(None, false)` mapping or seeding the poll thread's
`last` from the same `is_device_connected()` result used for the startup
handshake (so the poll thread starts from a consistent `Some(connected_at_startup)`).

---

## The `HAS_HANDSHAKED` lifecycle (full map)

| Event | Where | `HAS_HANDSHAKED` effect |
|-------|-------|-------------------------|
| Startup handshake (device present) | runners `linux.rs:31-33` / `macos.rs:31-33` / `windows.rs:52-54,105-107` → `perform_handshake` | `swap(true)` → `true` |
| Poll `Gain` (`None→true` or `false→true`) | `tray.rs:395` / `linux_tray.rs:280` → `perform_handshake` | `swap(true)`; if already true → **short-circuit** (no-op) |
| Poll `Loss` (`Some(true)→false`) | `tray.rs:398` / `linux_tray.rs:283` → `reset_handshake_state` | set `false` (`notifier.rs:653`) |
| Poll `None` (`None→false`, no change) | `tray.rs:400` / `linux_tray.rs:285` | **unchanged** ← this is the #5 gap |
| Handshake `Timeout` (transient) | `notifier.rs:520-522` | set `false` (release token → retry) |
| Handshake device `Err` (transient) | `notifier.rs:547-549` | set `false` (release token → retry) |
| Handshake legacy/non-capable `Info` | `notifier.rs:528-539` | **stays true** (firmware set `has_been_queried`; re-query risks R6 side effect) |
| Capable `Info` | `notifier.rs:502` | stays true; sets `HOST_CAPABLE`/`BOARD_HAS_RULES` |

`reset_handshake_state` (`notifier.rs:649-654`) is the **only** production path
that sets `HAS_HANDSHAKED=false` on a disconnect; the transient `Timeout`/`Err`
arms release the token so the next `perform_handshake` retries without needing a
`Loss`.

---

## Start here

Open **`src/core/notifier.rs:388` (`perform_handshake_with`)** — it is the
critical function for **both** findings. For #4, the lock acquisition is line 401
and the swept-under-lock loop is 430-481 (drop at 484). For #5, the dedup swap is
line 390 and the lifecycle decision function `handshake_action` is at 689-693
(consumed by the poll loops at `src/tray.rs:393` and `src/linux_tray.rs:278`).

## Open questions / residual risks

- **#4** is mitigated by design (5 s deadline + 64 cap). Residual: ~5–8 s
  notification stall worst-case on a buggy board; the deadline does not cover the
  pre-sweep `QueryInfo`/`SetOs` sends (each up to ~1 s under the same lock). A
  non-blocking sweep (release lock per iteration) would eliminate it but adds
  complexity. Decision needed: accept the bounded stall, or restructure to release
  `NOTIFIER` during the sweep.
- **#5** is a real lifecycle inconsistency. Candidate fixes (choose one):
  (a) change `handshake_action(None, false)` semantics — but that risks spurious
  resets; (b) seed the poll thread's `last` from the startup `is_device_connected()`
  result so it never starts `None`-but-already-handshooked; (c) have the startup
  handshake report into a shared state the poll thread reads. Recommendation: (b)
  is the least invasive and directly closes the desync.
- `REPLY_READ_TIMEOUT_MS` (≈1 s) lives in the external `qmk_notifier` crate (not
  in this repo); the ~1 s per-send bounds are per the comment at
  `notifier.rs:373`, not verified against vendored source here.
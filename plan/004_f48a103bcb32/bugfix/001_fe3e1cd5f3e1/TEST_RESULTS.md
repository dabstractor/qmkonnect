Everything is green. Here is my comprehensive bug-hunt report.

---

# Creative Bug-Hunt Report — QMKonnect End-to-End PRD Validation

I validated the implementation against the PRD (especially the newest **F11/F12 host-side window rules** feature in `HOST_RULES.md`), reading the core pipeline (`rules.rs`, `pattern.rs`, `notifier.rs`), platform monitors, runners, tray UIs, CLI, and the udev helper. The codebase is unusually well-tested (343 passing tests, clean clippy) and most `REMAINING_ISSUES.md` items are already fixed. I found **one critical bug** (fixed + regression test) and several lower-severity findings.

## 🔴 CRITICAL — Debounce worker panic crashes the service (race condition)

**Location:** `src/core/notifier.rs`, `debounce_worker()` inner wait loop (~line 855).

**Root cause:** The worker's inner wait loop does `state.pending.take().unwrap()` once `now >= target`, but **never re-checks that `pending` is still `Some`**. Meanwhile `notify_qmk`'s immediate-send ("due") branch unconditionally executes `state.pending = None` *and* bumps `last_sent_time`. If the worker is parked in its inner `wait_timeout` with a queued message and a new window change lands at the debounce boundary, `notify_qmk` clears `pending` and shifts the target; the worker's next iteration hits `now >= target` with `pending == None` and **panics on `.unwrap()`**.

**Impact:** The worker panics while holding the `STATE` mutex, poisoning it — so *every subsequent* `notify_qmk` call panics too (cascade). Worse, `Cargo.toml` sets `panic = "abort"` for release, so a single occurrence **aborts the entire QMKonnect service**. This is triggered by rapid window switching (Alt-Tab spam) — precisely the workload debouncing exists to smooth out. Over a long-running daemon it is essentially guaranteed to fire.

**Reproduction:** I wrote a deterministic test (`test_debounce_worker_survives_pending_cleared_mid_wait`) that primes the worker into the inner wait, then simulates the `due`-branch race. On the original code it produced:
```
thread '<unnamed>' panicked at src/core/notifier.rs:855:51:
called `Option::unwrap()` on a `None` value
```
confirming `STATE` was poisoned.

**Fix applied:** Replaced `.take().unwrap()` with a match that falls back to the outer wait loop if `pending` raced to `None`:
```rust
let pm = match state.pending.take() {
    Some(pm) => pm,
    None => break,  // pending raced to None; outer loop re-waits for the next message
};
```
`break` exits the inner loop with `to_send = None` (skips the send), and the outer loop re-waits on the condvar — no busy-loop, no panic. Data correctness is preserved (the newer message was already sent immediately; the superseded pending is correctly dropped, matching "newest wins" debounce semantics). Regression test added; **344 tests pass**.

## 🟡 Lower-severity findings (not fixed — design tradeoffs / polish)

1. **Non-atomic config/rules file writes.** Every save path (`render_config_body` → `std::fs::write`) truncates-then-writes with no temp-file+rename. A crash or a concurrent `configured_filter`/`parse_rules` read mid-write can observe a partial file. Readers *do* degrade gracefully (parse error → defaults / one-time desktop notification), but a partial write that happens to be *valid TOML with wrong values* could persist briefly. Best practice: write to `config.toml.tmp` then `rename`.

2. **Config + `rules.toml` re-read from disk on every window change.** `host_context_for_window` parses `rules.toml` and `configured_filter` parses `config.toml` on *every* focus change (hot-config is intentional), so under rapid switching that's many disk reads/sec. Latency only; a short TTL cache would help on slow/networked filesystems.

3. **Windows desktop-notification deviates from spec (§7).** `platforms::notify` on Windows uses a modal `MessageBoxW(HWND(0), …, MB_OK)` on a spawned thread, not a toast. It's desktop-modal (steals focus, needs a click) and leaks a thread until dismissed. Dedup limits it to one-per-broken-`rules.toml`-state, so impact is small, but it contradicts the spec's "toast on Windows".

4. **Handshake holds the global notifier mutex up to 5 s.** `perform_handshake_with` runs the `QUERY_CALLBACK` sweep while holding the notifier lock; a buggy board is bounded by `CALLBACK_SWEEP_DEADLINE` (5 s), but during that window *all* window notifications block. Only once per board boot (deduped), and only at (re)connect — acceptable, but worth noting.

5. **Transient first-tick device-absent race after a startup handshake.** If `is_device_connected()` returns `false` on the poll thread's *first* tick (transient hidapi hiccup) after the runner already handshook at startup, no `Loss` is ever recorded, so `HAS_HANDSHAKED` stays `true` and a later reconnect short-circuits the handshake (`SET_OS` not re-sent). Benign for the same board (callback names are positional/stable per R4; firmware remembers its state), so severity is very low.

## ✅ Validated as correct / already fixed

- **Pattern matcher** (Thompson NFA, full firmware parity) — empty-core short-circuit, anchors, glob, classes, `+`, case-folding all correct; exhaustive corpus tests.
- **Rules evaluator** — first-match layer / all-match callbacks, order-independent `disable` exclusion, `clear_board` truth table, C13 no-match-never-clears-board all match spec §8(3)/(4).
- **Send orchestration** — stack (string→context), replace (context-only), no-match (string→clear-context) ordering verified via mock; lock ordering (CALLBACK_NAMES vs notifier) is deadlock-free.
- **`static mut` data races** (#5), **Hyprland reconnect backoff reset** (#7), **udev `/tmp`+`sudo` race + `MODE=0666`** (#4), **X11 stub→real `xprop` impl** (#14), **macOS graceful screen-recording degradation** (#13), **config field preservation across VID/PID saves** (Windows/macOS/Linux) — all fixed.
- **`hid_id` udev helper** — correct HID report-descriptor walk (usage-page 0xFF60 + usage 0x61), long-item skip, truncation safety.

**Net:** one critical reliability fix landed (debounce panic), full suite green (344 tests, +1 regression). The remaining items are hardening/polish, not correctness blockers.

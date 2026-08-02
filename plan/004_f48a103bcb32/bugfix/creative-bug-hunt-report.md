# Creative Bug Hunt — End-to-End PRD Validation (independent pass)

**Baseline:** HEAD `9f2783c` ("Fix settings dialog data loss and validation") — contains
the three fixes from the prior `bug-hunt-report.md` (settings data-loss, `--validate-rules`
count, Hyprland `handle_window_state_change` dedup). `cargo test --bin qmkonnect --
--test-threads=1` → **340 passed, 0 failed**. Build clean.

**Scope of this pass:** independently re-validate the F11/F12 host-rules feature and the
PRD's cross-cutting claims, looking *past* the C8 schema unification the prior hunt
focused on. Each finding has file:line, repro, the spec/PRD clause it touches, and a fix
direction. Findings the prior hunt already filed are *not* repeated.

> Note on environment: this repo is being edited by concurrent agents (the three fixes
> were committed as `9f2783c` while this pass ran; pre-existing stashes `stash@{0}
> "test hardening"` / `stash@{1} "host-side rules updates"` were left untouched). This
> report was written to a **new** file to avoid clobbering the committed report.

---

## 🟠 MEDIUM — Title-based host rules are unreliable on Windows & macOS (in-app title changes are never detected)

**Where:**
- Windows hook: `src/platforms/windows.rs:57-64` —
  `SetWinEventHook(EVENT_OBJECT_FOCUS, EVENT_OBJECT_FOCUS, …)`. Event range is a single
  value (`eventMin == eventMax == EVENT_OBJECT_FOCUS`); **`EVENT_OBJECT_NAMECHANGE` is
  not hooked.**
- Windows poller: `src/platforms/windows.rs:91-104` — gates on
  `current_hwnd.0 != last_hwnd.0` (HWND equality). A title change with the **same** HWND
  never enters `handle_focus_change`.
- macOS: `src/platforms/macos.rs:181-184` — registers **only**
  `NSWorkspaceDidActivateApplicationNotification` (app *activation*). `start()`
  (`macos.rs:209-221`) then captures the initial window and blocks on `CFRunLoopRun()`
  — **no `NSTimer`/poller, no title-change observer.**

**Bug:** A headline capability of F11 is matching on the window **title**
(`HOST_RULES.md` §9 examples lean on it: `match = ["*chrome*","*youtube*"]`,
`["*chrome*","*claude*"]`; §8(2): "`Pattern::Parts(c,t)`: both halves must match"). For a
title rule to react, the host must be *told* the title changed. It is, on Hyprland: the
`activewindow` IPC event fires on a title change of the focused window
(`hyprland.rs:127` → `handle_window_state_change` → `Client::get_active()`). It is **not**,
on Windows/macOS: both monitors react only to foreground/**app-activation** transitions.

Concretely, with the keyboard already focused on an app:
- **Browser tab switch** (Ctrl+Tab, or clicking a tab) — same top-level window, title
  changes ("YouTube – Chrome" → "Gmail – Chrome"). On Windows the poller skips it
  (HWND unchanged) and `EVENT_OBJECT_FOCUS` is unreliable for in-window tab focus
  (varies by app; no `NAMECHANGE` hook). On macOS nothing fires at all.
- **Document/sheet switching** in an already-focused Office/IDE/editor — same story.

Result: the keyboard stays in the layer for the *old* title. A user on Windows/macOS who
writes `match = ["*chrome*", "*youtube*"]` sees it work when they alt-tab *into* Chrome
on a YouTube tab, then **silently stops reacting** as they tab around inside Chrome.
Class-only rules (`match = "alacritty"`) are unaffected — the gap is title-pattern-only,
which is half the documented feature surface.

**Spec/PRD violated:**
- `HOST_RULES.md` §9 markets title patterns as a first-class, cross-platform capability
  with no per-platform caveat. `PRD.md` F11 lists the feature without an "Hyprland-only"
  asterisk (Non-Goals §2.2 restricts *compositors*, not *title reactivity*).
- `PLATFORMS.md` §2.1/§3.1 documents the *mechanism* (focus hook / activation observer)
  but never states the consequence: title changes within an active app are not surfaced.
  It is an undocumented limitation, not a documented one.

**Fix direction (pick per platform):**
- **Windows:** add `EVENT_OBJECT_NAMECHANGE` to the hook range (`eventMin =
  EVENT_SYSTEM_FOREGROUND`/`EVENT_OBJECT_NAMECHANGE`, or a second `SetWinEventHook` for
  `EVENT_OBJECT_NAMECHANGE`), and have `event_proc` re-derive the *foreground* window's
  title via `GetForegroundWindow()`→`GetWindowTextW` (ignore the namechange object's own
  hwnd, which may be a child). `handle_focus_change`'s existing `(class,title)` dedup
  (`LAST_WINDOW_INFO`) already suppresses noise. The poller should compare on
  `(class,title)`, not HWND, if it is to serve as a true fallback.
- **macOS:** add an observer for title changes. Options: an `NSTimer` (e.g. 250–500 ms)
  calling `get_active_window_info` (cheap; dedup at `notify_qmk`/debounce), or an
  AXUIElement `kAXTitleChangedNotification` on the focused app (heavier; needs
  Accessibility). A timer mirrors the existing Hyprland `poll_interval_ms` design.

**Severity rationale:** MEDIUM. It silently degrades a headline, documented, cross-platform
feature on 2 of 3 supported platforms; class-only rules still work, and title rules *do*
fire on app-switch-in, so it is "flaky" rather than "dead". But a user will reasonably
expect `*youtube*` to track tab navigation, and it will not.

---

## 🟡 LOW–MED — `poll_interval_ms` is NOT hot-reloaded (Hyprland); self-contradicting code comment

**Where:** `src/platforms/hyprland.rs:75-94` —
```rust
let (_, poll_interval_ms) = crate::core::configured_timing();   // read ONCE
if poll_interval_ms > 0 {
    thread::spawn(move || {
        let interval = Duration::from_millis(poll_interval_ms); // baked in forever
        loop { thread::sleep(interval); poll_window_state(&lws, verbose); }
    });
}
```
The poll thread captures a fixed `Duration` and loops for the process lifetime. The only
consumer of `poll_interval_ms` outside the struct/serializer/tests is this one site
(grep-verified).

**Bug:** Changing `poll_interval_ms` in `config.toml` (or via the Settings dialog, which
now preserves it) has **no effect** without restarting QMKonnect:
- `0 → 200`: the poll thread was never spawned; editing the file cannot start it.
- `200 → 500`: the running thread keeps the 200 ms cadence.
- `200 → 0` (disable): the thread keeps polling.

This is asymmetric with `debounce_ms`, which *is* genuinely hot — `DebounceState::interval()`
(`src/core/notifier.rs:755-757`) re-reads `configured_debounce_ms()` on every notification.

**Spec/PRD violated:** PRD §7 — *"Configuration is hot. VID/PID/**timing** are re-read
from `config.toml` on every notification and every status poll, so editing the file …
takes effect within ~3 s with no restart."* `poll_interval_ms` is a timing field (CONFIG.md
table; `Config.poll_interval_ms`). The in-source comment at `src/core/notifier.rs:731-738`
even *claims* both debounce and poll are hot ("they are re-read … on every notification
and every status poll") — which is false for `poll_interval_ms`. So the comment
documents intent the code doesn't meet.

**Repro:** start on Hyprland with `poll_interval_ms = 0`; while running, set it to `200`
and save. Observe no periodic polls (verbose log shows no `[Nms] poll detected …` lines
and no `periodic active-window poll every 200ms` line). Restart → the line appears.

**Fix direction:** have the poll thread re-read `configured_timing().1` each iteration
(`let interval = Duration::from_millis(configured_timing().1); if interval.is_zero() {
break; }`) so a live edit to `0` stops the poll and a change to the cadence takes effect
on the next tick.

**Severity rationale:** LOW–MED. Narrow audience (Hyprland-only; the field defaults to
`0`/disabled, so most users never set it), but it is a straight PRD-hot-reload violation
with a comment that asserts the opposite.

---

## 🟡 LOW–MED — Malformed `config.toml` is silently swallowed; no startup diagnostic (PRD §2.1 Goal 4)

**Where:**
- `src/core/notifier.rs:83` `configured_filter()`:
  `…find(|p| p.exists()).and_then(|p| crate::core::parse_config(&p).ok())`
- `src/core/mod.rs:99-103` `configured_timing()`:
  `…and_then(|p| parse_config(&p).ok()).map(|cfg| (cfg.debounce_ms, cfg.poll_interval_ms))…`
- `src/core/notifier.rs:122` `startup_device_probe()` builds its filter from
  `configured_filter()` (which already swallowed any parse error).

**Bug:** Both hot-config readers use `.ok()`, discarding parse errors. If `config.toml` is
malformed — a typo'd hex (`vendor_id = 0xfeed` already fine, but `vendor_id = "feed"` or a
duplicate key, or `debounce_ms = "fifty"`), a stray bracket, a wrong type — the readers
fall back to all-default (`usage_page=0xff60`, `usage=0x61`, `debounce_ms=50`, …) with no
log line, no warning, no notification. There is no `--validate-config` (grep-verified: none
exists) to counter-balance, in contrast to `rules.toml` which has `--validate-rules` *and*
a desktop notification on parse failure (`HOST_RULES.md` §7; `notifier.rs:996-1009`).

Worse, `startup_device_probe` then runs the device probe against the **defaulted** filter:
with a standard QMK board attached it prints *"Found QMK device …"* and exits happily,
actively **masking** the user's typo. The user's overridden `usage_page`/`debounce_ms`/
`poll_interval_ms` are all silently ignored.

**Spec/PRD violated:** PRD §2.1 Goal 4 — *"Graceful degradation, never crashes. … Typo'd
config? Probe once at startup and say so clearly."* The "probe once and say so clearly"
half is unmet for `config.toml`. (The prior hunt logged this as an "observation …
graceful-degradation by design"; this pass elevates it: graceful degradation is fine, but
the spec explicitly requires a *clear startup signal* that is entirely absent, and the
device-probe-uses-defaults behavior makes the silence misleading, not just lenient.)

**Fix direction:** have `configured_filter`/`configured_timing` surface the parse error to
`startup_device_probe` (e.g. return `Result` or set a `Lazy<Option<String>>` last-error),
and have `startup_device_probe` print it once ("Warning: config.toml parse failed: {e} —
using defaults"). Optionally add `--validate-config` mirroring `--validate-rules`.

**Severity rationale:** LOW–MED. No crash, real graceful degradation, but a documented
user-facing guarantee is unmet and the misleading "Found QMK device" on a typo'd config is
a genuine footgun.

---

## 🟢 LOW — TOCTOU double-notify race in the freshly-applied Hyprland dedup fix

**Where:** `src/platforms/hyprland.rs:498-524` (the just-landed
`handle_window_state_change` dedup), specifically the two lock acquisitions at
**`:500`** (compare) and **`:517`** (update).

**Bug:** The dedup performs its compare and its update in **two separate** lock
acquisitions:
```rust
let window_changed = { let last = lock; compare; };   // critical section 1 (:500)
if window_changed {
    { let mut last = lock; *last = Some(current); }    // critical section 2 (:517)
    notify_qmk(…);
}
```
Between section 1 and section 2 another thread can read the *same* old `last_window_state`,
also conclude "changed", also update, and also notify. That other thread exists:
`spawn_poll_burst` (`hyprland.rs:369-376`) spawns a poll thread that runs
`poll_window_state` 5×/100 ms, and it is fired **from the same event handlers** that call
`handle_window_state_change` (`hyprland.rs:158,166`). So on a layer/scratchpad event the
listener thread and a poll-burst thread race on `last_window_state`.

Outcome: the same `(class,title)` can be notified twice — *exactly* the duplicate the
dedup was added to prevent. (Compare `poll_window_state` at `hyprland.rs:407`, which holds
the lock across compare+update+notify, so it is atomic.) Impact is bounded: the firmware's
desired-set diff is idempotent for an identical `ApplyHostContext`, so the only cost is a
wasted USB round-trip (and on boards that don't dedup, a re-fire of `on_enable`). This
matches the prior hunt's "mostly harmless" framing of the original dup — but the fix
re-introduces the dup through a race it could have closed.

**Fix direction:** make compare+update atomic in one critical section (mirror
`poll_window_state`): hold the lock, compare, and if changed set `*last` and capture a
clone to notify; drop the lock before `notify_qmk` (notify must not run under
`last_window_state` — it takes the debounce `STATE`/`NOTIFIER` locks).

```rust
let window_info = {
    let mut last = last_window_state.lock().unwrap();
    let changed = match &*last {
        None => true,
        Some(l) => l.app_class != current.app_class || l.title != current.title,
    };
    if changed { *last = Some(current.clone()); Some(WindowInfo::new(current.app_class, current.title)) }
    else { None }
};
if let Some(wi) = window_info { let _ = notifier::notify_qmk(&wi, verbose); }
```

**Severity rationale:** LOW. Narrow window, idempotent effect, and it is a refinement of
the just-landed fix rather than a regression from C8.

---

## ⚪ Observations / refinements (not new defects)

1. **Prior "Windows `notify()` is blocking" note is imprecise.** `platforms::notify`
   (`src/platforms/mod.rs:160-178`) on Windows **spawns a thread** for the `MessageBoxW`,
   so the *caller* (debounce worker / monitor thread) is not blocked — only the spawned
   dialog thread awaits the click. So it cannot wedge the notifier; it remains a UX nit
   (modal toast requires a click; repeated break/fix cycles of `rules.toml` can stack
   modals). No code change needed for correctness.
2. **macOS event channel is bounded + lossy.** `macos.rs:118-124` uses a
   `sync_channel(64)` with `try_send` (drop-on-full). During rapid app switching with a
   slow HID worker, activations are dropped. The debouncer coalesces, so this is benign in
   steady state, but combined with the MEDIUM finding (no title polling) it means macOS
   leans entirely on app-activation edges.
3. **Verified-correct (this pass):** the three staged fixes are sound —
   `render_config_body(&Config)` round-trips all six fields (test
   `render_config_body_preserves_non_vidpid_fields`); both dialog paths parse
   `current_config` at open and overlay only VID/PID on save; the `Config::default()` impl
   matches the serde defaults (so a missing-file save writes `debounce_ms=50`, not `0`);
   the `--validate-rules` count now reports with-layer / with-callbacks / combined
   independently; `evaluate()`'s no-match short-circuit correctly avoids the vacuous-`all()`
   trap. 340 tests green.

---

## Suggested priority

1. **MEDIUM (title reactivity on Win/macOS)** — largest user-visible feature gap; fix is
   localized (one more event hook on Windows; one timer/observer on macOS).
2. **LOW–MED (`poll_interval_ms` hot-reload)** — one-line-ish fix; also fix the misleading
   comment.
3. **LOW–MED (config.toml diagnostic)** — surface the parse error in
   `startup_device_probe`; optionally `--validate-config`.
4. **LOW (Hyprland dedup TOCTOU)** — fold compare+update into one critical section.
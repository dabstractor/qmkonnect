# Creative Bug Hunt — End-to-End PRD Validation (2nd independent pass)

**Baseline:** HEAD `332a31d` ("Fix CI/dev-loop quality gates and readme path metadata").
**State at pass start:** `cargo build --release` clean; **383 unit tests pass**
(`--test-threads=1`); `cargo fmt --all -- --check` **passes**; `cargo clippy
--all-targets -- -D warnings` **passes**; `Cargo.toml readme = "README.md"` correct.

**Scope of this pass:** re-validate F11–F14 (host rules + two-tier discovery + VIA
coexistence) and the PRD cross-cutting claims *past* the prior
`creative-bug-hunt-report.md` / `validation_report.md`. Each finding has
file:line, the spec/PRD clause it touches, a concrete repro, and a fix
direction. Issues the prior reports already filed are **not** repeated; a short
"verified-fixed" list closes the loop on them.

> **Headline:** the prior pass's three behavioural findings are all **resolved**
> in this tree (Windows `EVENT_OBJECT_NAMECHANGE` hook + HWND-agnostic poller;
> macOS 500 ms title poller; `config_parse_error()` surfaced in
> `startup_device_probe`). What remains is a cluster of **multi-board /
> capability-state** edge cases in F13 that the single‑boolean status model
> can't express.

---

## 🟠 MEDIUM — Mixed multi-board partial unplug is undetected: status falsely shows "Connected", and a replug of a *different* capable board never re-handshakes

**Where:**
- Status truth table: `src/core/notifier.rs:814` `device_status()` →
  `classify_device_status(is_device_connected(), host_capable())` (L825).
- Handshake lifecycle keyed on **Tier-1 presence**, not capable-board presence:
  `src/tray.rs:443-456` and `src/linux_tray.rs:296-316` both compute
  `handshake_action(last, connected)` from `is_device_connected()` (any
  `0xFF60` interface).
- `host_capable()` only resets to `false` via `reset_handshake_state()` (L758),
  which the poll threads call **only on a `Loss`** = `is_device_connected()`
  `Some(true) → false` (`handshake_action` L1215).

**Bug:** `is_device_connected()` is a Tier-1 predicate ("is *any* `0xFF60`
board present?"), but `host_capable()` is meant to mean "is a
qmk_notifier-capable board present?". These diverge exactly when a **capable
board is unplugged while a non-capable (VIA/Vial/legacy) board remains on the
bus** — the headline F13 scenario (PRD §2.1 Goal 1: *"a desk with a VIA board
and a qmk_notifier board plugged in at once: Tier 1 sees both; Tier 2 selects
the one that speaks back"*).

Concrete trace (1 capable board A + 1 pure-VIA board B):
1. Both plugged. `is_device_connected()=true`; handshake (broadcast `QUERY_INFO`)
   → A replies proto-v2 → `HOST_CAPABLE=true`. Status = **Connected**. ✓
2. **Unplug A.** B remains. Next poll: `is_device_connected()`=true (B is
   `0xFF60`). `handshake_action(Some(true), true)` = `None` → **no `Loss`, no
   `reset_handshake_state()`**. `HOST_CAPABLE` stays `true` (stale).
3. Status = **Connected** — but the only board left is B (VIA, no module). It
   should be **NoModule** ("⚠ QMK board found — no qmk_notifier module").
4. Every subsequent window change sends `SendMessage` + `ApplyHostContext` to the
   broadcast filter, which now reaches **only B**, which ignores both. Host rules
   and board rules silently no-op while the tray insists the device is connected.
5. **Replug a *different* capable board A′** (e.g. a travel keyboard with a
   different callback registry): `is_device_connected()` stays `true` the whole
   time → **no `Gain`** → `perform_handshake` never re-runs (also blocked by
   `HAS_HANDSHAKED`). `CALLBACK_NAMES` stays mapped to A's registry, so host
   rules reference the wrong ids on A′. The only recovery is unplugging B too
   (full presence loss → `Loss` → reset) or restarting QMKonnect.

The existing test `test_device_status_is_disconnected_in_ci_without_hardware`
(L3393) proves `present=false` dominates a stale `HOST_CAPABLE`, but **no test
exercises `present=true` + stale-capable**, because the bool-based design can't
represent "the capable board left but a Tier-1 board remains".

**Spec/PRD violated:**
- `DEVICE_DISCOVERY.md` §3 — the three-state table defines **Connected** as
  *"≥1 **capable** board present"*, not "≥1 Tier-1 board + a sticky flag". The
  doc-comment on `device_status()` (L786–806) documents the divergence from the
  spec's prescription ("the status probe thread … now calls `classify_devices`
  (cache-backed)") as a simplification — but that simplification is precisely
  what drops the capable-board-lost signal.
- PRD §11 Success Criterion 2 — *"Unplugging the keyboard shows '○ No Device
  Connected' within a few seconds; replugging restores … notifications resume —
  no restart, no crash."* In the mixed setup, unplugging the *capable* keyboard
  shows neither Disconnected nor NoModule, and replugging a different board does
  not resume correctly without a restart.

**Fix direction:** make the status/handshake lifecycle reflect **capable-board**
presence, not Tier-1 presence. Two options:
- (Preferred, matches spec §3) drive `device_status()` from
  `classify_devices()` (cache-backed, so it doesn't re-ping every tick): a board
  leaving the capable set is a real transition that resets `HOST_CAPABLE` and
  re-arms the handshake.
- (Lighter) keep the bool, but on every poll fold `classify_devices()`'s
  `any(Capable)` into the `Loss`/`Gain` decision: `capable_present =
  classify_devices(false).iter().any(|d| matches!(d.kind, Capable))` and key
  `handshake_action` on *that* bool (it's cache-backed → ~free per tick). Either
  way `host_capable()` must be allowed to go `true→false` while Tier-1 presence
  stays `true`.

**Severity rationale:** MEDIUM. It silently misreports the headline three-state
status and breaks host-rule delivery in the exact mixed multi-board case F13
exists to serve; but it needs a ≥2-board mixed setup *and* a partial unplug, and
the user typically notices the keyboard is physically gone. Self-recovers only
when the last Tier-1 board also unplugs.

---

## 🟡 LOW–MED — Picker mislabels a non-capable board as "qmk_notifier ✓" for up to 5 s after the handshake (mixed multi-board)

**Where:** `src/core/notifier.rs:1174` `warm_cache_from_handshake(kind)`:
```rust
fn warm_cache_from_handshake(kind: DeviceKind) {
    for c in enumerate_candidates() {            // EVERY Tier-1 path
        classification_cache_insert(&c.path, kind.clone());  // stamped with ONE kind
    }
}
```
called from the capable arm of `perform_handshake_with` (L596 region). Read back
as a cache hit by `classify_candidates` (L1097, `classification_cache_get`).

**Bug:** the handshake's `QUERY_INFO` is **broadcast** (filter-keyed, no vid/pid
in the zero-config common case), so its single reply can only ever yield *one*
classification — yet `warm_cache_from_handshake` stamps **every** enumerated
Tier-1 candidate with that one result. In a mixed setup (capable board A +
pure-VIA board B), the handshake succeeds on A's reply and warm-stamps **both**
A and B as `Capable`. The discovered-device picker (`classify_devices` → cache
hit) then renders B as **"✓ qmk_notifier"** — the exact falsehood Tier-2 exists
to prevent — until the `CLASSIFICATION_TTL` (5 s, L946) expires *and* the picker
is reopened / `[Rescan]` is clicked (which is what re-probes each candidate with
a vid/pid-narrowed filter and correctly flips B to `✗`).

The in-source comment (L1166–1172) acknowledges a "single-vid/pid-on-bus
assumption" but **mis-frames the scope**: it says the stamp is fine because
same-vid/pid boards "are the same board model in the common case". The actual
problem is independent of vid/pid — *any* two boards with *different* vid/pid
(e.g. a Dactyl `0x1209:0x7f00` capable + a Keychron `0x3434:0x0123` VIA) both get
stamped `Capable`, because the broadcast handshake cannot attribute its single
reply to a path.

**Repro:** with A (capable) + B (VIA) both present, (re)plug either board so the
handshake fires, then open Settings within ~5 s. Both rows show `✓`. Wait 5 s,
click `[Rescan]` → B flips to `✗`. (`--list-devices` shows the same transient
mislabel in its `kind` column.)

**Spec/PRD violated:** `DEVICE_DISCOVERY.md` §2.2 / §5.1 — the picker's ✓/✗ is
the user-facing Tier-2 verdict; a `✓` on a VIA board is a false verdict. §2.4's
"single-ping-per-appearance" goal is what motivates the warm feed, but the warm
feed trades correctness for that ping-savings in the mixed case.

**Fix direction:** don't warm-stamp candidates the handshake can't individually
identify. Either (a) skip `warm_cache_from_handshake` entirely and accept the
one extra per-candidate ping on first `classify_devices` (the cache then holds
*per-path* truths), or (b) only warm-stamp when `enumerate_candidates()` yields
≤1 path (the common single-board case where broadcast == unicast). Option (a) is
simplest and the 5 s TTL already bounds re-pings.

**Severity rationale:** LOW–MED. Self-heals in ≤5 s or on Rescan, display-only
(no wrong bytes hit the wire — writes are still gated on the handshake's real
capable verdict), but it directly undermines the picker's reason for existing in
the mixed case the feature advertises.

---

## 🟡 LOW–MED — `classify_devices` probe sends a typed command to legacy (proto-v1) firmware, deactivating its board layer — contradicts "no board is ever harmed by the probe" / R6

**Where:** `src/core/notifier.rs:1110-1128` `classify_candidates` — for each
Tier-1 candidate it unconditionally calls
`send_command(RunCommand::QueryInfo, &narrowed)` with **no capability/proto
guard** (the guard lives only in `perform_handshake_with`).

**Bug:** `DEVICE_DISCOVERY.md` §2.2 states *"No board is ever harmed by the
probe: the magic header is what makes qmk_notifier coexist with other Raw HID
modules, so VIA/Vial firmware silently ignores the probe."* and `HOST_RULES.md`
R6/§5 states *"legacy firmware never receives typed commands."* Both are true for
**proto-v2** firmware (the `data[2]==0xF0` discriminator routes `QUERY_INFO` to
`handle_typed_command` with no `process_full_message` side effect) and for
**pure-VIA** firmware (ignores `0x81 0x9F` entirely). They are **not** true for
**proto-v1 qmk_notifier firmware** — an older flash that has the `0x81 0x9F`
string path but *not* the typed-command dispatch:

- The probe emits `[0x81][0x9F][0xF0][0x01][…][0x03]`.
- Proto-v1 `hid_notify()` validates + strips the `0x81 0x9F` header (present
  since pre-typed-command qmk_notifier), reassembles `0xF0 0x01 …`, then
  sanitizes to ASCII `0x20–0x7E`: `0xF0` and `0x01` are dropped, `0x03` is ETX
  (terminates) → the reassembled message is **empty**.
- `process_full_message("")` matches nothing → the firmware's no-match path
  **deactivates the active board layer / disables the active command**
  (PRD §7: *"an empty `app_class`+`title` … deactivates any active notifier
  layer"*). The user's currently-active board layer is cleared.

So opening Settings, clicking `[Rescan]`, or running `--list-devices` on a
proto-v1 board briefly resets its active layer — recovering only on the next
real window focus. The 5 s `CLASSIFICATION_TTL` bounds it to "once per 5 s per
user action", but each manual Rescan re-triggers it. This is the exact side
effect R6's `has_been_queried`/host-side `HAS_HANDSHAKED` dedup was designed to
prevent — but `classify_devices` sits **outside** that dedup and pings on its
own schedule.

**Spec/PRD violated:** `DEVICE_DISCOVERY.md` §2.2 ("no board is ever harmed"),
`HOST_RULES.md` §5/R6 ("legacy firmware never receives typed commands"). The
probe *is* a typed command, and it *does* reach legacy firmware with a side
effect.

**Fix direction:** have `classify_candidates` consult the handshake-warmed cache
first and, on a cache miss, prefer to **defer** classification to the handshake
(which is deduped and runs once per boot) rather than independently pinging —
i.e. treat a cold cache for a not-yet-handshaken board as `NotQmkNotifier`
provisionally and let the handshake refine it. Alternatively, document proto-v1
as excluded from the "harmless" claim and accept the transient deactivation (it's
recoverable). The cleanest fix is the cache-deferral above: it also removes the
Finding-#2 warm-stamp problem at the same time.

**Severity rationale:** LOW–MED. Narrow audience (users running an older
proto-v1 qmk_notifier flash), recoverable on the next focus change, and bounded
by the TTL — but it is a live contradiction of two explicit spec invariants and a
genuine user-visible hiccup (active layer drops when poking Settings).

---

## ⚪ Observations / context (not new defects)

1. **Trayless / `--no-default-features` build has no reconnect-driven
   handshake.** The handshake lifecycle (Gain/Loss) lives entirely in the tray
   poll threads (`tray.rs`, `linux_tray.rs`). The minimal trayless service build
   (`runners/linux.rs`) handshakes **once at startup** and never again — an
   unplug/replug after startup is not re-handshaked (and `HOST_CAPABLE` goes
   stale, compounding Finding #1). Acceptable for the documented "trayless
   service" target (systemd `Restart=always` + `BindsTo` own the lifecycle), but
   worth a one-line caveat in `LINUX.md` so a user of the minimal build isn't
   surprised that host rules don't resume after a replug without a restart.
2. **`NOTIFIER` mutex uses `.lock().unwrap()`** (handshake L473/538, worker
   L1445, `notify_qmk` L1513) — a panic while holding it poisons the lock and, under
   `panic = "abort"`, ends the process. The CONFIG/RULES caches use the
   `unwrap_or_else(|e| e.into_inner())` poison-recovery idiom; the comment at
   L1078 calls a poisoned `NOTIFIER` a deliberate hard-failure. Consistent and
   intentional; flagged only because it's the one lock whose poisoning is fatal.
3. **Status-poll HID enumeration cost.** `is_device_connected()` constructs a
   fresh `HidApi::new()` + full `device_list()` scan on **every** poll tick
   (1–3 s) and on every status-derived call, forever. Read-only (never opens a
   device, so R-COEX is safe), but on a bus with many HID devices it is non-trivial
   perpetual CPU. Existing behaviour, not a regression; a small enumeration cache
   (keyed by a short TTL) would lower idle CPU if that ever matters.

## ✅ Verified-fixed since the prior reports (closeout)

- `cargo fmt --all -- --check` → **exit 0** (was the prior "headline" CI-blocker).
- `cargo clippy --all-targets -- -D warnings` → **clean** (prior `type_complexity`).
- `Cargo.toml` `readme = "README.md"` matches the on-disk `README.md`.
- Prior MEDIUM "title reactivity on Win/macOS": Windows now hooks
  `EVENT_OBJECT_NAMECHANGE` (`windows.rs:39-67`) + an HWND-agnostic poller; macOS
  adds a 500 ms title poller (`macos.rs:221-254`). Both closed.
- Prior LOW–MED "config.toml silent swallow": `config_parse_error()`
  (`notifier.rs:96`) is surfaced in `startup_device_probe` (`notifier.rs:191-200`).
- Settings data-loss: all three platforms' save paths overlay VID/PID onto the
  open-time `Config` and serialize via `render_config_body` (`tray.rs:967-978`,
  `linux_tray.rs:1032-1041`), preserving `usage_page`/`usage`/`debounce_ms`/
  `poll_interval_ms`.

## Suggested priority

1. **MEDIUM (mixed multi-board partial unplug)** — largest correctness gap in the
   headline F13 scenario; fix = key the handshake lifecycle on *capable-board*
   presence (use the cache-backed `classify_devices` the spec already names).
2. **LOW–MED (picker warm-stamp mislabel)** — fix = don't warm-stamp candidates
   the broadcast handshake can't individually identify (or skip the warm feed).
3. **LOW–MED (proto-v1 probe side effect)** — fix = cache-defer classification to
   the deduped handshake; also resolves #2.
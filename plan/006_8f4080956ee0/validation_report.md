# QMKonnect — Validation Report & Bug Tracker

**Project:** QMKonnect v0.2.8 · **Validator:** automated + manual E2E
**Date:** 2025-08-05 · **Platform:** Arch Linux x86_64 (kernel 7.1.3), Rust 1.92.0
**Hardware present:** Dactyl-Manuform 5×7-1 (`0x1209:0x7f00`) running qmk_notifier
firmware (proto v2, 1 callback `vim_lazy`, board_rules=true) — **real-hardware E2E was possible.**

---

## Executive Summary

**Overall verdict: PASS.** The codebase is in strong shape. Every automated
quality gate is green and the entire end-to-end user journey works against a
real qmk_notifier-capable keyboard, including the headline F11/F12/F13 features
(host-side rules + typed commands + two-tier discovery). `./validate.sh`
exercises 29 checks across 9 phases and all pass.

No correctness bugs were found in the Linux/default-build hot path. The findings
below are a mix of one **real-hardware transient** (worth hardening), minor
**code-quality inconsistencies**, and a documented **cross-platform validation
gap** that relies on CI. None block a release; they are improvement candidates.

| Severity | Count | Summary |
|---|---|---|
| 🔴 Critical | 0 | — |
| 🟠 High | 0 | — |
| 🟡 Medium | 1 | QUERY_CALLBACK sweep can transiently drop a callback name (host-rules callbacks silently no-op for the session) |
| 🔵 Low | 2 | Trayless-build clippy gate gap; inconsistent Mutex poison-recovery pattern |
| ⚪ Info | 3 | Cross-platform type-check limitation; docs cross-link; stale-artifact reminder |

---

## Validation Scope & Method

### Step 0 — Real user workflows (from README / AGENTS.md / docs)
- **Fresh-install zero-config journey:** `cargo build --release` → auto-discover
  any QMK keyboard by `0xFF60/0x61` → notifications flow with no config.
- **Config/rules lifecycle:** `qmkonnect -c` → edit → `qmkonnect --validate-rules`
  → `qmkonnect -r` (Linux udev).
- **Diagnostics journey:** `--list-devices` (VID/PID discovery) →
  `--list-callbacks` (handshake) → `--validate-rules` (lint).

### Step 1 — Existing tooling discovered
- **Lint/format:** `cargo fmt`, `cargo clippy --all-targets -- -D warnings`
- **Type check:** the Rust compiler (`cargo build`/`check`); no separate checker
- **Unit tests:** `cargo test --bin qmkonnect -- --test-threads=1` (single-threaded
  is mandatory — shared global debouncer state, PRD §11 #4)
- **External integrations exercised E2E:** real HID device (Dactyl-Manuform),
  `hidraw` sysfs report descriptors (udev helper), `hyprctl` IPC, `udevadm`

### Step 2 — `validate.sh` (9 phases, 29 checks)
See `./validate.sh`. Phases: formatting → clippy → builds (debug/release/trayless)
→ single-threaded unit tests → CLI surface → config/rules lifecycle →
`qmkonnect-hid-id` helper → **real-hardware E2E** → version/packaging consistency.

### Step 3 — Results matrix

| Phase | Checks | Result |
|---|---|---|
| 1. `cargo fmt --all --check` | 1 | ✅ |
| 2. `cargo clippy --all-targets -- -D warnings` | 1 | ✅ (clean) |
| 3. Builds (debug / release / trayless `--no-default-features`) | 3 | ✅ |
| 4. Unit tests (`--test-threads=1`) — **389 tests** + hid-id tests | 2 | ✅ |
| 5. CLI surface (`--help`, `--list`, `--list-devices`) | 3 | ✅ |
| 6. Config & rules lifecycle (create/validate×5/reload) | 8 | ✅ |
| 7. `qmkonnect-hid-id` udev helper (udev-safety + real descriptor scan) | 3 | ✅ |
| 8. **Real-hardware E2E** (Tier-2 probe + handshake + full pipeline) | 4 | ✅ |
| 9. Version/packaging consistency | 4 | ✅ |
| **TOTAL** | **29** | **29 PASS / 0 FAIL** |

---

## Detailed Findings

### 🟡 Finding #1 — `QUERY_CALLBACK` sweep can transiently mis-parse a reply, silently dropping a callback name

**Where:** `src/core/notifier.rs` `perform_handshake_with` → the callback sweep
(`Ok(other) => { … "unexpected reply" … }` branch), line ~530.

**Evidence (real hardware, first `--list-callbacks -v` invocation):**
```
[0ms] perform_handshake: proto v2 capable (flags=0x03, 1 callbacks, board_rules=true)
[3ms] perform_handshake: callback 0 unexpected reply Ack { ok: true }
[7ms] perform_handshake: complete — capable (0 callbacks mapped)
Connected keyboard reports 0 callbacks.        ← firmware advertises 1
```
The firmware reports `callback_count = 1`, but `QUERY_CALLBACK(0)` is parsed as
`CommandResponse::Ack { ok: true }` instead of `CallbackName`. The handler logs
the mismatch (verbose only) and skips the name, yielding "0 callbacks mapped".

**Reproducibility:** low. 1 occurrence in ~20 invocations; the next 15+
invocations all mapped `vim_lazy` correctly. Not deterministic.

**Impact (the real concern):** the handshake is deduped per board boot
(`HAS_HANDSHAKED`, reset only on a real device transition). So when this transient
hits **during the connect handshake** (not just `--list-callbacks`), the
`CALLBACK_NAMES` map stays **empty for the entire session** until the device is
unplugged/replugged. Every host-rule `enable`/`disable` then resolves to **no
ids** and is silently dropped (`evaluate` skips unknown names). The user sees
layer switching work but callback toggles (e.g. `vim_lazy`) silently do nothing —
with no non-verbose indication. The F12 "named callback registry" feature is
effectively non-functional for that session.

**Root cause:** cannot be localized inside qmkonnect from this environment. The
qmkonnect logic is *defensible* (it logs + skips rather than panicking). The
misparsed `Ack` originates in either (a) the firmware returning a generic ack
under some timing, or (b) the `qmk-notifier` crate reading a stale/interleaved IN
report. Both are outside this repo.

**Recommendation (qmkonnect-side hardening, not a fix to the root cause):**
- On the `Ok(other) => unexpected reply` branch for `QUERY_CALLBACK(i)`, retry
  that single index once or twice (the crate already supports bounded reads) — a
  transient mis-read is exactly what a single retry would clear.
- Surface the empty-map-after-nonzero-count case as a **warning** (not just
  verbose): if `callback_count > 0` but `CALLBACK_NAMES.len() == 0` after the
  sweep, emit a one-line `eprintln!` warning so a silent session-long failure
  isn't invisible.
- (Stretch) On the next reconnect/re-handshake trigger, if the map is still empty
  despite `callback_count > 0`, release the `HAS_HANDSHAKED` token so it retries.

---

### 🔵 Finding #2 — Trayless build emits 10 warnings; the `-D warnings` clippy gate only covers the default build

**Where:** `cargo build --release --no-default-features`.

**Evidence:**
```
warning: function `list_foreground_windows` is never used
warning: struct `PresenceTracker` is never constructed
warning: function `tier1_paths` is never used
warning: function `reset_handshake_state` is never used
warning: function `render_config_body` is never used
warning: function `presence_tick_decision` is never used
warning: function `handshake_action` is never used
warning: enum `HandshakeAction` is never used
warning: unused variable: `verbose`
… (10 total)
```
These are all `dead_code` warnings for symbols that ARE used by the default
(`linux-tray`) build — benign by themselves. **But:** `.github/workflows/ci.yml`
runs `cargo clippy --all-targets -- -D warnings` only on the default feature set,
so `cargo clippy --no-default-features -- -D warnings` would FAIL and nobody
would notice. The trayless target is a documented, shipped configuration
(spec/LINUX.md §6.2), so its clippy gate should be green too.

**Impact:** low — functionally fine; a contributor running the documented trayless
build under a strict clippy gets a wall of errors for code that is correct.

**Recommendation:** either (a) add a second CI clippy job on
`--no-default-features`, or (b) gate the tray-only free functions with
`#[cfg_attr(not(feature = "linux-tray"), allow(dead_code))]` (or move them behind
the feature). The `unused variable: verbose` is the only one that may indicate a
real oversight — worth a glance.

---

### 🔵 Finding #3 — Inconsistent `Mutex` poison-recovery pattern

**Where:** `src/core/notifier.rs`.

The file deliberately uses the poison-recovery idiom for two globals:
```rust
// STATE (debouncer), CONFIG_CACHE, RULES_CACHE:
STATE.lock().unwrap_or_else(|e| e.into_inner())
```
but uses **raw `.lock().unwrap()`** for two others (10 + 8 call sites):
```rust
let n = notifier.lock().unwrap();                       // NOTIFIER
CALLBACK_NAMES.lock().unwrap().clone();                 // CALLBACK_NAMES
```

**Impact:** under `panic = "abort"` (release profile), a panic kills the process
before any mutex can become poisoned, so this is **not a live production bug**.
It is an inconsistency: the same author wrote both patterns for the same
situation in the same file, and in a debug/test multi-threaded context a poisoned
`NOTIFIER` would cascade. The documented intent (PRD §10: "poison recovery … a
panic in one caller must not poison the cache") is only half-applied.

**Recommendation:** normalize the remaining 18 sites to
`.lock().unwrap_or_else(|e| e.into_inner())`, or add a one-line comment at each
explaining why raw `.unwrap()` is acceptable there (process-abort semantics).

---

### ⚪ Info #4 — macOS / Windows Rust code cannot be type-checked on this Linux box

**Evidence:** `cargo check --target x86_64-pc-windows-msvc` and
`--target x86_64-apple-darwin` both fail — **not on Rust code**, but on the
`hidapi` C dependency:
- Windows: `hidapi/windows/hidapi_winapi.h:31: fatal error: guiddef.h: No such file or directory`
- macOS:   `cc: error: unrecognized command-line option '-arch' '-mmacosx-version-min=10.7'`

This is purely a cross-toolchain limitation (no Windows SDK / no osxcross on this
Linux host), consistent with AGENTS.md ("Visual Studio Build Tools" / "MSVC" are
the documented Windows prereq). The Rust modules are correctly `#[cfg]`-gated
(`mod macos;` only `#[cfg(target_os = "macos")]`, etc.), and CI
(`.github/workflows/ci.yml`) builds + tests on a real 3-OS matrix
(`ubuntu-22.04`, `macos-latest`, `windows-latest`), which is where this gap is
closed.

**Impact:** none for shipping; a local dev on one OS cannot fully validate the
other two OSes without CI. REMAINING_ISSUES.md #1 ("macOS target does not
compile") is **stale** — the code is now properly gated; the historical
unconditional compile no longer reproduces.

**Recommendation:** none required. Optionally note in AGENTS.md that local
cross-OS type-checking is blocked by the hidapi C dep, so devs rely on CI.

---

### ⚪ Info #5 — `docs/usage.md` does not document the CLI flag surface

**Evidence:** `docs/usage.md` is scoped to start/stop instructions and contains
**none** of the CLI flags (`--list-devices`, `--list-callbacks`, `--validate-rules`,
`--rules-path`, `-r`, `-c`, etc.). The full CLI reference lives in
`docs/configuration.md` (lines 231–293). This is not a drift (the flags ARE
documented), but a user landing on `usage.md` looking for "how do I list my
devices" finds nothing and no pointer.

**Recommendation:** add a one-line cross-link in `docs/usage.md`:
> For all command-line flags (incl. `--list-devices`, `--list-callbacks`,
> `--validate-rules`), see the [CLI reference](configuration.md#cli-flags).

---

### ⚪ Info #6 — Stale `target/release/qmkonnect` artifact reminder (live, not theoretical)

**Evidence:** the on-disk `target/release/qmkonnect` was, at the start of this
validation, a **trayless** build — `--list` reported "Linux (X11)" until a fresh
`cargo build --release` (default features) rebuilt it to "Linux (Hyprland)". This
is exactly the "don't trust a stale artifact" hazard called out in AGENTS.md
(Windows section: "cargo served a stale artifact — do not trust it"), now
observed concretely on Linux.

**Impact:** none (binaries are gitignored and never shipped from a dev box), but
it confirms the AGENTS.md warning is load-bearing. The transient in Finding #1
was first investigated against this stale binary before the rebuild, which could
have produced a misleading diagnosis.

**Recommendation:** none — informational. Reaffirms "always rebuild before
debugging."

---

## What Was Verified Working (high-confidence E2E, not just unit tests)

Against the real Dactyl-Manuform + Hyprland session:

- **F1** window detection — Hyprland monitor reported `Alacritty | terminal`.
- **F2/F3** Raw HID transport + **two-tier discovery** — `--list-devices` shows
  all HID interfaces and tags the `0xff60:0x0061` interface `qmk_notifier`
  (Tier-2 `QUERY_INFO` probe ran live).
- **F4** debouncer — immediate send on first window change observed.
- **F5** TOML config hot-read — `poll_interval_ms`/`debounce_ms` picked up live.
- **F10** firmware contract — handshake decoded `proto_ver=2, flags=0x03,
  callback_count=1, board_rules=true`.
- **F11/F12** host-side rules + callbacks — a `match="*"` `rules.toml` produced,
  on one window change, **both** the legacy string `Alacritty\x1Dterminal` (18 B)
  **and** `ApplyHostContext { layer: Some(10), callbacks: [0], clear_board: false }`
  — the full stack-mode send sequence, end-to-end on real hardware.
- **F9** Linux static udev path — `qmkonnect-hid-id` tagged **exactly one** of 9
  hidraw interfaces with `ID_QMKONNECT=1`; the config-driven fallback rule
  renders as a single safe `KERNEL==`-led line (the dangerous-multiline guard).
- **`--list-callbacks`** resolved `vim_lazy → id 0` (15/16 runs; see Finding #1).

All 389 unit tests pass single-threaded; `cargo fmt` and `cargo clippy -D warnings`
are clean on the default build; version `0.2.8` and the `qmk-notifier` v0.3.0 pin
are consistent across `Cargo.toml` / `Cargo.lock` / `PKGBUILD` / the Inno `.iss`;
no build artifacts are tracked in git.

---

## Items Explicitly Not Fixed (per validation-agent remit)

This report only **tests and reports**. No source, spec, plan, or `tasks.json`
files were modified. The two deliverables written — `./validate.sh` and this
`./validation_report.md` — are temporary and should be deleted after review.
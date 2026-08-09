# QMKonnect — Validation Report

**Date:** 2026-08-09 · **Version validated:** 0.2.8 (`Cargo.toml`) · `qmk-notifier` v0.3.0 pinned
**Validation script:** `./validate.sh` — **run live**, then the remaining gates completed by hand (see Methodology)
**Host:** Arch Linux x86_64 (kernel 7.1.6-arch1-1), Rust/cargo 1.92.0, Hyprland (Wayland, `DISPLAY=:1`)
**Hardware present:** Dactyl-Manuform 5×7-1 (`0x1209:0x7f00`, QMK Raw HID `0xff60:0x0061`, `qmk_notifier`) — **real-hardware E2E was possible.**

---

## TL;DR — and the answer to "what is taking so long?"

The validation agent is **not** dying of slow builds (the obvious suspect). It is being killed because **`./validate.sh` enters an infinite hang** in Phase 4 and never returns. The script's `chk()` helper wraps every command with **no timeout**, and one Phase 4 gate runs `qmkonnect --bogus-flag-xyz`, expecting it to be *rejected*. It is not: the app's hand-rolled CLI parser has **no unknown-argument handling**, so an unrecognized flag silently falls through to `runner.run()` and **starts the daemon**, which blocks forever. With no in-script timeout, the whole run stalls until the agent's watchdog fires. That is the kill.

So validation surfaced **one real product defect** (the silent-unknown-flag bug, which *is* the hang) plus **two defects in the harness itself** (no `timeout` in `chk()`; a `--bogus-flag` assertion that assumes clap-style rejection the parser doesn't provide). Everything else is green, including a full live end-to-end run against the attached keyboard.

| Severity | Count | Items |
|---|---|---|
| 🔴 Hard (product bug — also the hang source) | 1 | Unknown CLI flag silently starts the daemon instead of erroring |
| 🟠 Harness defect (causes the watchdog kill) | 1 | `chk()` has no `timeout`; `--bogus-flag` check can never exit |
| ⚪ Environment false positive (not a defect) | 2 | `asdf-qmkonnect` link sweep hits `.pi-subagents/`; `llms_full.txt` is dirty **working-tree** drift (HEAD is correct) |

> **Verdict on the product:** healthy. 441 + 10 unit tests pass single-threaded, `cargo fmt` and `cargo clippy --all-targets -- -D warnings` are both **clean**, all five build variants build, packaging metadata + assets are intact, and the live daemon handshake + window→keyboard pipeline fires end-to-end on real hardware. The single product finding is a CLI-robustness gap, not a runtime correctness bug in the hot path.

---

## Methodology

`./validate.sh` was run live. It progressed cleanly through **Phase 0 → mid-Phase 4**, then **hung indefinitely** on the `--bogus-flag is rejected (CLI robustness)` gate (the process was observed live as a running daemon, 3+ minutes, state `Sl`). The hung run was terminated and the **remaining Phase 4–6 gates were executed individually**, each explicitly bounded with `timeout`, to complete the results matrix. No source, spec, plan, or `tasks.json` files were modified during validation.

| Phase | Gates | Result |
|---|---|---|
| 0. Prerequisites / toolchain | 4 | ✅ all pass |
| 1. Build (debug / release / hid-id / trayless / `--all-targets` LTO) | 5 | ✅ all pass |
| 2. `cargo fmt --check` + clippy (×3) | 4 | ✅ all pass (fmt is clean here) |
| 3. Unit tests, `--test-threads=1` (441 + 10) | 2 | ✅ all pass |
| 4. CLI workflows | 12 | **11 ✅ / 1 ⛔ hang** (the bogus-flag gate) |
| 5. Assets / schema / doc-sync | 11 | 9 ✅ / **2 false positives** |
| 6. Hardware/Display E2E (live board) | 4 | ✅ all pass |
| **TOTAL run gates** | **42** | **40 pass · 1 hang (bug) · 1 harness-broken · 2 env false-positive** |

---

## 🔴 Hard Issue

### H1. An unrecognized CLI flag is silently accepted — the app starts the daemon instead of erroring

**Where:** `src/main.rs::run()` (the manual argument parser) and `src/runners/linux.rs::run()`.

The CLI is parsed **by hand**, not with clap. `run()` scans `env::args()` with a chain of `args.iter().any(|a| a == <known flag>)` checks (`-h/--help`, `-v/--verbose`, `-r/--reload`, `-c/--config`, `-l/--list`, `--list-devices`, `--list-callbacks`, `--validate-rules`). **None of the branches rejects an unknown argument.** If no flag matches, execution falls straight through to:

```rust
// end of run(), src/main.rs
let mut runner = runners::create_runner(verbose)?;
runner.run(&args)
```

…which on Linux is `fn run(&mut self, _args: &[String])` — the `_args` (underscore = unused) are **ignored entirely**, and the daemon starts.

**Evidence (bounded repros, each expected to exit non-zero but instead launching the daemon):**
```
$ timeout 3 ./target/release/qmkonnect --bogus-flag-xyz   # the validate.sh gate
QMKonnect started
StatusNotifierItem tray registered
$ # exit 124 (timeout) — it ran as a daemon instead of rejecting the flag

$ for f in --validat-rules --verbos --list-device --hepl -x; do timeout 3 ./qmkonnect "$f" | head -1; done
QMKonnect started   (×5 — every typo silently launches the app)

$ # controls: real flags exit cleanly
$ ./qmkonnect --help >/dev/null; echo $?   # 0
$ ./qmkonnect -l      >/dev/null; echo $?   # 0
```

**Impact (why it matters):**
- **CLI robustness / UX.** A user who fat-fingers any flag — `--verbos`, `--validat-rules`, `--list-device`, `--hepl`, a stray `-x` — gets **zero feedback** and the app silently starts in the background. The documented `--help` surface promises a rigid CLI; the implementation accepts anything.
- **It is the direct cause of the validation watchdog kills** (see H2 below): the harness's `--bogus-flag is rejected` gate runs this command unbounded, so it hangs forever.
- **No hot-path correctness risk** — once running, the daemon behaves correctly (verified live). The defect is purely in argument acceptance.

**Fix:** after all known flags are scanned, before the `runner.run(&args)` fallthrough, reject any remaining `argv` element that starts with `-`:
```rust
// reject unknown options before starting the service
if let Some(bad) = args.iter().skip(1).find(|a| a.starts_with('-') && !is_known(a)) {
    eprintln!("error: unrecognized option '{bad}'\n  Try 'qmkonnect --help'.");
    process::exit(2);
}
```
(Or migrate the parser to `clap`-derive for free unknown-flag rejection, suggestion-validation, and the `--help` text staying in sync with the code.) Either way, add a unit/regression test that asserts `qmkonnect --bogus` exits non-zero without starting the service.

---

## 🟠 Harness Defect (this is why the agent gets killed)

### H2. `./validate.sh`'s `chk()` has no `timeout`, and the `--bogus-flag` gate can therefore never terminate

**Where:** `validate.sh`, the `chk()` helper and Phase 4.

```bash
chk() {
  local label="$1"; shift
  local log; log="$(mktemp)"
  if "$@" >"$log" 2>&1; then ok "$label"; …   # ← no timeout; "$@" runs unbounded
  …
}
```
```bash
chk "--bogus-flag is rejected (CLI robustness)" \
    bash -c "! '$BIN' --bogus-flag-xyz </dev/null 2>/dev/null"
```

Because of H1, `$BIN --bogus-flag-xyz` starts the daemon and never exits. `chk()` (and thus `set -uo pipefail` + the whole script) blocks on it indefinitely. There is **no in-script watchdog**, so nothing ever interrupts it — the agent's outer watchdog timer is the only thing that fires, and it kills the run. This is the literal mechanism behind "keeps getting killed after a very long time."

**Note on the earlier hypothesis:** the build matrix *is* slow (the `[profile.release]` `lto = true` + `codegen-units = 1` + `opt-level = "z"`, compiled across debug/release/trayless/`--all-targets`, is the slowest possible Rust release configuration and is repeated across dev/release/clippy profiles that share no artifacts). That contributes long run times, but it is **not** what kills the agent — the build gates all *complete*. The definitive killer is the Phase 4 hang.

**Fix (two independent hardenings):**
1. Bound every `chk` invocation: add a `local MAX=120` (or per-check override) and run `timeout "$MAX" "$@"`. A hang then becomes a clean failure (`✗`) instead of an infinite stall.
2. Fix the `--bogus-flag` assertion's premise: once H1 is fixed the app will exit non-zero on unknown flags and the existing `! …` form works as written. Until then, bound it (`! timeout 5 "$BIN" --bogus-flag …`) so the gate reports a real failure rather than hanging.

---

## ⚪ Environment False Positives (NOT product defects)

These are the two Phase 5 gates that report failure. Both are artifacts of this validation environment, not of the qmkonnect repository.

### F1. "dead `asdf-qmkonnect` links" — 104 hits, all outside the project

The Phase 5 grep sweeps `.` for `asdf-qmkonnect` excluding only `plan/`, `.git/`, `target/`, `docs/vendor/`. On this host the matches are:
- **102** inside `.pi-subagents/` (the coding-agent's own mission/artifact scratch — not part of the repo, not shipped), and
- **2** inside `validate.sh` itself (the check's own source comments naming the symbol).

Filtering those out, the **real project has zero** `asdf-qmkonnect` references:
```
$ grep -rn 'asdf-qmkonnect' . … | grep -vE '/plan/|/\.git/|/target/|/docs/vendor/|/\.pi-subagents/|validate.sh'
(empty)
```
**Fix (harness):** add `.pi-subagents/` to the exclusion list. No product change.

### F2. `docs/llms_full.txt` "STALE/TRUNCATED" — working-tree drift only; HEAD is correct

The check flags the working-tree `llms_full.txt` as truncated (1990 lines) vs a fresh regeneration (3304 lines; missing the entire "Real-World Examples" section fed by `docs/examples.md`). However:
```
HEAD (committed)  docs/llms_full.txt  → 3304 lines, contains "Real-World Examples"   ✅ in sync
working-tree      docs/llms_full.txt  → 1990 lines, truncated                        dirty
freshly generated                      → 3304 lines  == HEAD
```
So **HEAD is correct and in sync**; only the local working-tree copy is truncated (it was already `M` in `git status` at the start of the session). This is dirty working-tree state, not a committed defect. `git checkout docs/llms_full.txt` (or `bash docs/generate_llms_full.sh`) restores it. (Provenance of the truncation is not determinable from here — likely residue from an earlier interrupted run; it is exactly the "stale artifact" hazard the check exists to catch, just in the working tree rather than a commit.)

---

## What Was Verified Healthy (the other 40 gates)

### Toolchain & quality gates
- ✅ `cargo` 1.92.0, `cargo-clippy`, `rustfmt` all on PATH; `rustc 1.92.0`.
- ✅ **All five build variants** clean: debug (default), release (default), release `qmkonnect-hid-id`, `--no-default-features` (trayless service), and `--release --all-targets` (the LTO/`panic=abort` gate).
- ✅ **`cargo fmt --all -- --check` passes** — the `main`-branch `fmt` CI gate is green. *(Historical reports in `plan/` flagged this as the headline failure; it is resolved in the current tree.)*
- ✅ **`cargo clippy --all-targets -- -D warnings` exits 0** (and per-binary clippy clean).
- ✅ **441 unit tests pass, 0 failed** (`cargo test --bin qmkonnect -- --test-threads=1`, single-threaded as mandated by AGENTS.md for the shared debouncer state); **10/10** for `qmkonnect-hid-id`.

### CLI + config/rules user journeys (isolated `HOME`/`XDG_CONFIG_HOME`)
- ✅ `--help` reports `QMKonnect v0.2.8`; `-l`/`--list` prints the platform line.
- ✅ `-c` seeds `config.toml` **and** `rules.toml`; idempotent on re-run; seeded config has **no `0xfeed` literal** (DEVICE_DISCOVERY §7.2 cleanup).
- ✅ `--validate-rules` matrix is fully correct: valid → 0; missing `match` → 1; `layer=255` sentinel → 1; no-op rule → 1; nonexistent `--rules-path` → 1; no-rules-anywhere → 0.
- ✅ `-r` renders a **single safe `KERNEL==`-led udev line** with `ATTRS{idVendor}=="1209"` / `ATTRS{idProduct}=="7f00"`, non-root advisory only.

### Live product smoke — real Dactyl-Manuform attached
- ✅ `qmkonnect-hid-id` tags **exactly** `hidraw4` with `ID_QMKONNECT=1`.
- ✅ `--list-devices` enumerates the bus (15 lines) and classifies `0x1209:0x7f00 / 0xff60:0x0061` as `qmk_notifier`.
- ✅ **`--list-callbacks` live handshake succeeds**: `Callback name -> id (1):  0  vim_lazy` (proto-v2 board).
- ✅ **Full window→keyboard pipeline fires live**: `select_linux_backend` probes and selects `foreign-toplevel`; the focused `Alacritty` window is detected and the daemon sends `Notified QMK (debounced): Alacritty|terminal` (18 B) followed by `ApplyHostContext { layer: None, callbacks: [], clear_board: false }`.

### Packaging / asset / schema integrity
- ✅ `Cargo.toml` valid TOML; rpm `[package.metadata.generate-rpm.requires]` sub-table present (no invalid `require-local`); GNOME `metadata.json` valid JSON.
- ✅ Every packaging asset referenced by `Cargo.toml` metadata exists on disk; build outputs (`target/`, `inno/Output`, `arch/pkg`) are gitignored and **not tracked**.
- ✅ GNOME extension defines `enable()`/`disable()` + the D-Bus contract (`get_wm_class`, `focus_window`, `io.mulletware.QMKonnect`, `ActiveWindowChanged`, `WindowMonitor`).
- ✅ `mise`/`asdf` removed from all user-facing docs (`docs/*.md`, `README.md`).

---

## Residual Risks (informational, not actionable as bugs)

1. **Binaries are ad-hoc/unsigned** (PRD §12, by design for beta) — macOS Screen-Recording re-prompts per rebuild; Winget shows "unverified publisher". Not a defect.
2. **Cross-OS Rust modules (macOS/Windows) type-check only via CI** on this Linux host; the hidapi C dep blocks local cross-compilation. They are correctly `#[cfg]`-gated and are exercised by the `.github/workflows` 3-OS matrix.
3. **The build is slow by construction** (max-LTO release profile recompiled across 3+ profiles). Not a defect, but it amplifies the cost of every CI/dev run — and is the reason the *slow* portion of the run looks suspicious before the real cause (the hang) is found.

---

## Recommended Action Order

1. **🔴 H1 (product)** — reject unknown CLI flags in `run()` before the `runner.run()` fallthrough; add a regression test. **This single fix also makes the `--bogus-flag` gate pass**, which unblocks the next item.
2. **🟠 H2 (harness)** — wrap `chk()` in `timeout` so no future hanging command can stall the whole script (and the agent's watchdog) indefinitely. Defense-in-depth regardless of H1.
3. **⚪ F1/F2 (harness/env)** — exclude `.pi-subagents/` from the `asdf-qmkonnect` sweep; `git checkout docs/llms_full.txt` to clear the dirty working-tree truncation (HEAD is already correct).

*Generated by running `./validate.sh` live (Phases 0–4a) and completing Phases 4b–6 by hand with explicit `timeout` bounds, after the live run hung on the `--bogus-flag` gate (see H1/H2).*
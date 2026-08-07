# Research Notes — P2.M3.T2.S2: First-run GNOME notification when extension missing

## 1. The gap (what exists vs. what §8.4 requires)

**Exists today** — `src/runners/linux.rs::maybe_gnome_first_run_notify(verbose)`
(lines 133-155):
- Called from ONE place: the no-backend `Err` branch of the `match
  platforms::create_monitor(...)` block (line 105).
- Checks `$XDG_CURRENT_DESKTOP` contains "GNOME" (case-insensitive) ONLY.
- Fires `crate::platforms::notify(...)`. Doc-comment explicitly relies on the
  fact that the `Err` branch is entered at most once per process, so there is
  **no AtomicBool guard** and **no name-ownership probe** today.

**§8.4 requires** (spec/PLATFORMS.md L453-461): fire when the session is GNOME
**AND** the extension name `io.mulletware.QMKonnect` is **not owned at startup**
— **even if another backend (AT-SPI §9) was selected.** "Fires at most once per
launch."

**The concrete gap (load-bearing):** on a GNOME box with the extension missing
BUT AT-SPI available, `create_monitor` returns `Ok(atspi monitor)` → the `Err`
branch is NEVER entered → the notification NEVER fires. S1's PRP confirms this
is S2's job: *"The §8.4 enhancement (fire even when another backend is
selected) is P2.M3.T2.S2."*

## 2. S1's `gnome.rs` contract (S2 consumes it — does NOT edit it)

S1 (P2.M3.T2.S1) creates `src/platforms/gnome.rs` with, verbatim from its PRP:

```rust
pub(crate) fn probe_available(verbose: bool) -> Result<(), String>
//   Ok  <=> name `io.mulletware.QMKonnect` is owned on the session bus
//   Err <=> name NOT owned  (or, degenerate, no session bus reachable)
```

S2 reuses this: `crate::platforms::gnome::probe_available(false).is_ok()` ⇒
extension present (skip); `Err` ⇒ missing (fire). `Err` also covers the rare
no-session-bus case where `notify-send` fails silently anyway, so firing is
harmless. **S2 does NOT add functions to `gnome.rs`** (parallel task; avoid
edit conflicts). The `gnome` feature is in `default` (Cargo.toml:137).

**Sequencing dependency (CRITICAL):** S2's code references
`crate::platforms::gnome::probe_available`, which only exists after S1 lands.
S2 compiles only once S1's `gnome.rs` is present. The implementer must land S1
+ S2 together (same module). Documented in the PRP.

## 3. The AtomicBool one-shot idiom (mirror this exactly)

`src/linux_tray.rs:67-70`:
```rust
static NO_MODULE_NOTIFIED: AtomicBool = AtomicBool::new(false);
//   swap(true) → fires the first time; the caller re-arms with store(false).
```
Import: `use std::sync::atomic::{AtomicBool, Ordering};` (linux_tray.rs).
S2 mirrors it as `static GNOME_FIRST_RUN_FIRED: AtomicBool`. Unlike the tray's
re-arming flag, this one is **fire-once-and-stay** (never reset — §8.4 "at most
once per launch").

## 4. The `notify` helper (cross-platform, swallows failure)

`src/platforms/mod.rs:241-249` — `pub fn notify(title: &str, body: &str)`:
```rust
#[cfg(target_os = "linux")]
let _ = std::process::Command::new("notify-send")
    .args(["--app-name=QMKonnect", "--icon=input-keyboard", title, body])
    .status();
```
Already used by `host_context_for_window` (notifier.rs:1814). S2 reuses it
unchanged.

## 5. Exact edit sites in `src/runners/linux.rs` (verified line numbers)

- L1-4 imports: `use crate::platforms; use crate::runners::PlatformRunner;
  use std::error::Error; use std::process;` → **ADD**
  `use std::sync::atomic::{AtomicBool, Ordering};`.
- L56-60: tray spawn block, then `match platforms::create_monitor(self.verbose)
  {` at **L60** → **INSERT** the one call to `maybe_gnome_first_run_notify`
  immediately before L60 (fires regardless of Ok/Err branch).
- L100-105 (Err branch): the `eprintln!("No Linux window backend…")` + the call
  `maybe_gnome_first_run_notify(self.verbose);` at **L105** → **REMOVE** the
  call (early call subsumes it) + trim the comment ("Fire the GNOME one-shot
  extension hint, then keep main alive" → "keep main alive").
- L133-155: the `fn maybe_gnome_first_run_notify` body → **REWRITE** (add the
  AtomicBool guard + the `gnome::probe_available` ownership check, cfg-gated;
  link body to docs).
- No `#[cfg(test)] mod tests` exists in this file today → **ADD** one with
  hermetic env-parsing + one-shot tests.

## 6. `docs/qmk-integration.md` (S2's Mode-A doc; NOT touched by S1)

- Today has **ZERO** GNOME/extension coverage (grep: no matches). It is the QMK
  firmware integration guide.
- S1 edits `docs/installation.md` + `docs/troubleshooting.md`; S2 edits
  `qmk-integration.md` → **no file conflict** (parallel-safe).
- Contract: "ensure docs/qmk-integration.md covers the extension." Add a concise
  "### GNOME: window detection needs the Shell extension" subsection (Mode A —
  link to spec/PLATFORMS.md §8 + extensions.gnome.org; do NOT duplicate spec
  detail). Placement: under existing `## Common Issues` (L307) fits.

## 7. Test strategy (mirrors S1's hermetic posture)

- **Hermetic (automated):** `gnome_session()` env-parsing (case-insensitive;
  real-world `"ubuntu:GNOME"`; unset; empty) + `consume_gnome_hint_shot()`
  one-shot (local `AtomicBool`, first call `true`, second `false`). env
  set/restore snapshot pattern (S1's probe test does the same). Run
  `--test-threads=1` (ARCHITECTURE invariant 8).
- **Manual (Level 4, GNOME VM):** the live `probe_available` D-Bus round-trip +
  `notify-send` shell-out. Same posture as S1's live zbus plumbing (this dev
  box is Hyprland, not GNOME).

## 8. Factoring for testability

Split the decision from the side effects so the one-shot is unit-testable
without shelling out or hitting D-Bus:
```rust
fn gnome_session() -> bool { /* pure: XDG_CURRENT_DESKTOP contains GNOME */ }
fn consume_gnome_hint_shot(flag: &AtomicBool) -> bool { /* swap + gnome_session */ }
fn maybe_gnome_first_run_notify(verbose: bool) { /* guard → probe(cfg) → notify(cfg) */ }
```
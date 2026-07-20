# Research Notes — P4.M2.T1.S1: Implement handshake function and global capability state

> Scope: **`src/core/notifier.rs` ONLY** (the host-side orchestration home —
> HOST_RULES.md §11 file layout lists it for "handshake, SET_OS, state"). This
> task adds the capability-handshake **function + global state + accessors +
> MockNotifier scriptability + tests**. It does NOT wire the handshake into the
> runners or the device-status poll — that is **P4.M2.T1.S2** ("Integrate
> handshake into startup and device-status poll"). S2 consumes this task's
> `pub fn perform_handshake` + `pub fn reset_handshake_state` on a real device
> transition (false→true).

---

## §0 — Verbatim current anchors in `src/core/notifier.rs` (861 lines)

> ⚠ P4.M1.T2.S2 is now COMPLETE — it added `PendingMessage` (L257) and widened
> `DebounceState.pending` to `Option<PendingMessage>` (L268). All anchors below
> are CURRENT. My handshake block inserts in the **L182–184 band** (above all of
> P4.M1.T2.S2's edits) so there is NO overlap.

Module-top imports (lines 1-6) — **EDIT**: add `HashMap`/`BTreeSet` + `AtomicBool`/`Ordering`:
```rust
use crate::core::types::WindowInfo;
use once_cell::sync::Lazy;
use std::error::Error;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};
```

Key item anchors (CURRENT line numbers):
```
L13  fn notify(...)                              # trait method 1 (unchanged)
L51  fn send_command(&self, command, filter)     # trait method 2 (P4.M1.T1.S1, unchanged)
L65  pub struct DeviceFilter { vid, pid, usage_page, usage }   # unchanged
L77  fn configured_filter() -> DeviceFilter      # USED BY perform_handshake (unchanged)
L119 pub fn startup_device_probe(verbose: bool)  # S2 calls handshake near here; fn unchanged
L169 pub fn is_device_connected() -> bool  ... } # ENDS L182 (blank L183)  ← INSERT HANDSHAKE BLOCK HERE
L184 impl Notifier for QmkNotifier {             # QmkNotifier::send_command @L228 (unchanged, CALLED by handshake)
L249 static NOTIFIER: Lazy<Arc<Mutex<Box<dyn Notifier>>>>   # the global trait-object seam
L257 struct PendingMessage {...}                 # ❌ P4.M1.T2.S2 — DO NOT TOUCH
L264 struct DebounceState {... pending: Option<PendingMessage> ...}  # ❌ P4.M1.T2.S2 — DO NOT TOUCH
L285 fn get_notifier() -> Arc<Mutex<Box<dyn Notifier>>>    # USED BY perform_handshake (unchanged)
L362 static WORKER / L381 notify_qmk             # ❌ P4.M1.T2.S2 region — DO NOT TOUCH
L453 #[cfg(test)] mod tests {
L455   use super::*;
L456   use crate::core::types::WindowInfo;
L457   use std::sync::atomic::{AtomicUsize, Ordering};     # G3: Ordering also imported at module top (precedence-safe)
L458   use std::sync::Mutex as StdMutex;
L461   static MOCK_CALL_COUNT: Lazy<AtomicUsize>
L462   static MOCK_LAST_MESSAGE: Lazy<StdMutex<Option<String>>>
L463-464 static MOCK_SEND_COMMAND_CALLS: Lazy<StdMutex<Vec<RunCommand>>>   # append MOCK_RESPONSES after
L466   fn reset_global_mock() {  # +1 line: MOCK_RESPONSES.lock().unwrap().clear();
L472   struct MockNotifier;
L480   impl MockNotifier { ... get_send_command_calls @~488 }  # add set_mock_responses accessor here
L493   impl Notifier for MockNotifier { send_command @~L510 }  # EDIT: pop MOCK_RESPONSES front, fallback Ack
L517   fn reset_test_state()  # unchanged (calls reset_global_mock → transitively clears the queue)
tests: test_send_command_* (5, P4.M1.T1.S1) + debounce tests + test_debounced_pending_carries_window_info (P4.M1.T2.S2) — DO NOT modify
```

### Exact text of the two mock-extension sites (verbatim, for precise edits)

`reset_global_mock` (L466-471):
```rust
    fn reset_global_mock() {
        MOCK_CALL_COUNT.store(0, Ordering::SeqCst);
        *MOCK_LAST_MESSAGE.lock().unwrap() = None;
        MOCK_SEND_COMMAND_CALLS.lock().unwrap().clear();
    }
```
→ add one line before the closing brace: `        MOCK_RESPONSES.lock().unwrap().clear();`

`MockNotifier::send_command` (~L510):
```rust
        fn send_command(
            &self,
            command: qmk_notifier::RunCommand,
            _filter: &DeviceFilter,
        ) -> Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>> {
            MOCK_SEND_COMMAND_CALLS
                .lock()
                .unwrap()
                .push(command.clone());
            Ok(qmk_notifier::CommandResponse::Ack { ok: true })
        }
```
→ KEEP the push (G4); after it insert:
```rust
            let resp = MOCK_RESPONSES.lock().unwrap().pop_front();
            Ok(resp.unwrap_or(qmk_notifier::CommandResponse::Ack { ok: true }))
```
…and DELETE the original `Ok(qmk_notifier::CommandResponse::Ack { ok: true })` line (so there's exactly one return). G5: empty queue → that same `Ack{ok:true}` default (the 5 existing tests are unchanged).

### Insertion seam (P4.M1.T2.S2-safe)
- **Handshake block** (statics + fn + helpers + accessors): insert between
  `is_device_connected()`'s closing brace (L182) and `impl Notifier for QmkNotifier` (L184)
  — i.e. at the blank line L183. This band is topically cohesive (device capability,
  alongside `startup_device_probe`/`is_device_connected`) and **above** ALL of
  P4.M1.T2.S2's edits (PendingMessage L257 / DebounceState L264 / worker / notify_qmk).
- **Mock extension + tests**: inside `mod tests` (L453+). Add `MOCK_RESPONSES` after the
  `MOCK_SEND_COMMAND_CALLS` static (L464); add `set_mock_responses` accessor inside
  `impl MockNotifier`; modify `MockNotifier::send_command`; +1 clear-line in
  `reset_global_mock`; append the handshake tests at the end of `mod tests`.

---

## §1 — The crate API this task codes to (qmk_notifier v0.3.0 — **PINNED + TAGGED**)

`Cargo.toml:19` reads `qmk_notifier = { ..., tag = "v0.3.0" }` (P4.M1.T2.S1 = Complete)
AND `git tag -l 'v0.3.0'` (in the adjacent qmk_notifier repo) returns `v0.3.0`
(P1.M1.T4.S1 = Complete). → **NO compile-gate / path-override workaround needed**
(unlike P4.M1.T1.S1, which had to temp-override against v0.2.1). The typed types
resolve directly from the git tag.

Verbatim enum shapes (from `../qmk_notifier/src/lib.rs`):
```rust
pub enum RunCommand {
    SendMessage(String), ListDevices,
    QueryInfo, QueryCallback(u8), SetOs(HostOs),
    ApplyHostContext { layer: Option<u8>, callbacks: Vec<u8>, clear_board: bool },
}
#[repr(u8)] pub enum HostOs { Unsure=0, Linux=1, Windows=2, Macos=3, Ios=4 }   // ⚠ Macos lowercase
pub enum CommandResponse {
    Legacy { matched: bool },
    Info { proto_ver: u8, feature_flags: u8, callback_count: u8, board_rules_present: bool },
    CallbackName { index: u8, name: Option<String> },
    Ack { ok: bool },
    Timeout,
}
```
- Both enums `#[derive(Debug, Clone, PartialEq, Eq)]` → mock can clone into the log;
  `CommandResponse` moves out of the response queue (VecDeque::pop_front).
- `host_os()` mapping (G7 gotcha): `cfg!(target_os="linux")→Linux`,
  `"windows"→Windows`, `"macos"→Macos` (lowercase os in BOTH the cfg and the enum),
  else `Unsure`.

---

## §2 — The handshake contract (HOST_RULES.md §8(5), canonical pseudocode)

```text
resp = run(QueryInfo)
match resp {
  Info { proto_ver: 2, feature_flags, callback_count, .. } if flags & 0x01 => {
      run(SetOs(host_os))                                   // host OS-authoritative at connect
      for i in 0..callback_count { name_to_id.insert(run(QueryCallback(i)).name, i) }
      validate rules.toml names against name_to_id          // warn, don't fail
      capable = true
  }
  _ => capable = false   // legacy/offline → string-only
}
```
- **Order**: QueryInfo → (if capable) SetOs → QueryCallback sweep. SetOs BEFORE the
  sweep (item description: "send SetOs ... sweep callbacks"). The mock test asserts
  this exact 4-call sequence.
- **Capable predicate**: `proto_ver == 2 AND feature_flags & 0x01 != 0`. Anything
  else (proto_ver==1, flags without 0x01, Legacy, Timeout, Err) → string-only.
- **At most once per board boot**: firmware sets `has_been_queried` on first
  QUERY_INFO; host dedups via `HAS_HANDSHAKED`. Re-trigger only on a real device
  transition (S2's job — calls `reset_handshake_state()` then `perform_handshake()`).

---

## §3 — Design decisions (D1-D7)

- **D1 — Idempotent `perform_handshake` + separate `reset_handshake_state()`.**
  `perform_handshake` swaps `HAS_HANDSHAKED`→true on entry and short-circuits if
  already true (the "once per boot" guard). `reset_handshake_state()` clears all
  three statics (HOST_CAPABLE=false, CALLBACK_NAMES={}, HAS_HANDSHAKED=false) so S2
  can re-trigger on device gain. Matches the item: "Deduplicate by tracking
  has_handshaked state."
- **D2 — Collect names locally, publish after the sweep (no nested locks).** The
  sweep holds the NOTIFIER lock (each `send_command` needs `&self`); collecting
  into a local `HashMap` avoids holding CALLBACK_NAMES across slow HID round-trips
  and avoids any lock-ordering hazard. Publish `CALLBACK_NAMES` after `drop(n)`,
  then validate (validate locks CALLBACK_NAMES to read — safe, NOTIFIER dropped).
- **D3 — `validate_rules_callback_names` is best-effort, never fatal.** A missing
  rules.toml ⇒ skip (host rules disabled). A malformed rules.toml ⇒ warn + skip
  (`--validate-rules` in P5.M1 surfaces hard errors). Unknown names ⇒ eprintln
  warning each. HOST_CAPABLE stays true (the device IS capable; broken rules don't
  downgrade capability).
- **D4 — `unknown_callback_names(rules, known) -> Vec<String>` is the pure testable
  core.** Deduped + sorted via `BTreeSet` (deterministic test output). Returns names
  in `callback_rules[].enable`/`.disable` not present in `known`. perform_handshake
  eprintln!s each returned name.
- **D5 — MockNotifier gains a configurable response QUEUE (VecDeque), not a fixed
  map.** `MOCK_RESPONSES: Lazy<StdMutex<VecDeque<CommandResponse>>>`. `send_command`
  pops the front; when empty, returns the legacy default `Ok(Ack{ok:true})`
  (preserves the 5 existing `test_send_command_*` tests verbatim). P4.M1.T1.S1's PRP
  explicitly anticipated this: "P4.M2/P4.M3 will extend the mock with configurable
  responses later." `set_mock_responses(Vec)` extends; cleared in `reset_global_mock`
  → transitively in `reset_test_state`.
- **D6 — `host_os()` is private** (only perform_handshake uses it; the test reaches
  it via `super::*` so the SetOs assertion can be exact, not a `matches!`).
- **D7 — Public surface is minimal + S2/P4.M3/P5.M1-ready.** `pub fn
  perform_handshake(verbose)`, `pub fn host_capable() -> bool`, `pub fn
  callback_names() -> HashMap<String,u8>`, `pub fn reset_handshake_state()`. P4.M3
  gates APPLY_HOST_CONTEXT on `host_capable()` + passes `callback_names()` to
  `evaluate()`; P5.M1's `--list-callbacks` prints `callback_names()`.

---

## §4 — Gotchas (G1-G9)

- **G1 — NO compile-gate workaround.** Crate is pinned to v0.3.0 (Cargo.toml:19,
  P4.M1.T2.S1 done) AND the v0.3.0 tag is pushed (P1.M1.T4.S1 done; verified via
  `git tag -l`). The typed types resolve from the git tag. (P4.M1.T1.S1's temp
  path override is OBSOLETE — do not re-apply it.)
- **G2 — P4.M1.T2.S2 is COMPLETE (PendingMessage L257 / DebounceState L264 /
  worker / notify_qmk).** DO NOT touch that region. My handshake block inserts at
  the L182-184 band (above it); my mock/test edits are additive/replace inside
  `mod tests`. Disjoint — no conflict (P4.M1.T2.S2 is already landed).
- **G3 — Module-top `use std::sync::atomic::{AtomicBool, Ordering};` is precedence-
  safe** alongside the test module's own `use std::sync::atomic::{AtomicUsize,
  Ordering};` (L457). Explicit imports shadow glob imports (`use super::*`); AND
  both resolve to the identical path `std::sync::atomic::Ordering`. No E0252.
- **G4 — `MockNotifier::send_command` MUST keep recording into
  `MOCK_SEND_COMMAND_CALLS`** (the call-sequence log). The queue pop is ADDITIONAL,
  not a replacement — P4.M3's ordering assertions depend on the log. Keep the
  `push(command.clone())` block; THEN pop the response.
- **G5 — Backward-compat: empty queue ⇒ `Ok(Ack{ok:true})`.** The 5 existing
  `test_send_command_*` tests never call `set_mock_responses`, so they get the
  legacy default. The edited send_command has exactly ONE return statement
  (`Ok(resp.unwrap_or(Ack{ok:true}))`).
- **G6 — `--test-threads=1` MANDATORY** (shared STATE/COND/WORKER/NOTIFIER/HANDSHAKE
  globals; AGENTS.md). Every handshake test starts with `reset_test_state()` +
  `reset_handshake_state()` + `set_notifier(MockNotifier::new())`.
- **G7 — `HostOs::Macos` (lowercase os).** `cfg!(target_os="macos")` maps to
  `HostOs::Macos`. The helper: linux→Linux, windows→Windows, macos→Macos, else→Unsure.
- **G8 — `validate_rules_callback_names` reads the REAL platform rules.toml**
  (`get_rules_paths().find(exists)`). In the test env a dev rules.toml may or may not
  exist (and may reference names the mock didn't advertise). This is HARMLESS:
  warnings print (not asserted); HOST_CAPABLE/state are unaffected. Tests assert
  STATE + call sequence, never stderr. No flakiness.
- **G9 — Binary-only crate; Mode-A rustdoc uses ` ```rust,ignore `** (no `cargo test
  --doc` on a bin; `use qmkonnect::...` won't resolve). Match rules.rs/pattern.rs.

---

## §5 — Test plan (8 tests, all in `mod tests`, single-threaded)

1. **test_handshake_capable_populates_state** — scripts
   [Info{2,0x01,2,true}, Ack{true}, CallbackName{0,"vim_lazy"},
   CallbackName{1,"disable_vim"}]; asserts host_capable()==true,
   callback_names()=={vim_lazy:0, disable_vim:1}, call sequence == [QueryInfo,
   SetOs(_), QueryCallback(0), QueryCallback(1)] (len 4).
2. **test_handshake_legacy_proto_v1_string_only** — [Info{1,0x00,0,true}]; asserts
   !host_capable(), empty map, calls.len()==1 (QueryInfo only — NO SetOs/sweep).
3. **test_handshake_no_feature_flag_string_only** — [Info{2,0x00,3,true}] (proto v2
   but flags&0x01==0); asserts !host_capable(), calls.len()==1.
4. **test_handshake_timeout_string_only** — [Timeout]; asserts !host_capable(),
   calls.len()==1.
5. **test_handshake_dedup_idempotent** — handshake once (capable); call again;
   assert calls count UNCHANGED (HAS_HANDSHAKED short-circuit), host_capable()
   still true.
6. **test_handshake_reset_allows_rerun** — handshake (capable); reset_handshake_state
   → host_capable()==false, empty map; re-arm responses + re-handshake → host_capable
   ()==true, new name mapped (the S2 device-transition path).
7. **test_handshake_skips_anonymous_callback** — [Info{2,0x01,2,true}, Ack{true},
   CallbackName{0,None}, CallbackName{1,Some("named")}]; asserts map has only "named"
   (name:None skipped silently, no panic).
8. **test_unknown_callback_names_helper** — pure: parse a RuleSet with
   enable=["known_a","ghost"], disable=["known_b","phantom"]; known={known_a:0,
   known_b:1}; assert unknown==["ghost","phantom"] (sorted).

---

## §6 — Validation (project dev loop, AGENTS.md)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect                                  # clean (crate v0.3.0 resolves)
cargo test --bin qmkonnect -- --test-threads=1               # MANDATORY single-threaded
git diff --stat                                              # expect src/core/notifier.rs ONLY
```
Optional: `cargo clippy --bin qmkonnect --no-deps` (no NEW warnings).
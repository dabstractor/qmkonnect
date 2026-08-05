# Research Notes — P1.M1.T1.S3: Confirm documented caveats have backing behavior (read-only code audit)

## Task nature

READ-ONLY code audit. The deliverable is a 4-row audit report (Caveat | Code
location read | Expected behavior | Confirmed Y/N). NO code changes. If all four
CONFIRMED (baseline), the delta closes as verified-complete. A FAIL is the only
scenario that would spawn a follow-on fix task.

This re-confirms independently (direct code read) what
`plan/006_8f4080956ee0/architecture/verification_findings.md` §1/§1.5 already
asserted. Per the contract, S3 reads the ACTUAL code lines, not a transcription.

## Baseline: ALL FOUR CAVEATS CONFIRMED (verified by direct code read this session)

### (a) Proto-v1 picker caveat — CONFIRMED ✅

The asymmetry: the handshake path dedups on `HAS_HANDSHAKED`; the picker probe
does NOT.

- **`perform_handshake_with` (src/core/notifier.rs:428) — DOES gate:**
  ```rust
  // line 428 (inside perform_handshake_with, ~line 426):
  if HAS_HANDSHAKED.swap(true, Ordering::SeqCst) {
      if verbose { eprintln!("...already handshaked this session — skipping"); }
      return;   // dedup: at most once per board boot
  }
  ```
- **`classify_devices` (src/core/notifier.rs:1123-1128) — does NOT gate:**
  ```rust
  #[allow(dead_code)]
  pub fn classify_devices(verbose: bool) -> Vec<ClassifiedDevice> {
      let candidates = enumerate_candidates();
      invalidate_absent_cache_entries(&candidates);
      classify_candidates(candidates, verbose)
  }
  ```
  No `HAS_HANDSHAKED` reference anywhere in the body. This is the documented
  proto-v1 exception: the picker (Settings/[Rescan]/--list-devices) sits outside
  the dedup, so on a proto-v1 board it can briefly reset the active layer per probe.

### (b) Warm-feed scope — CONFIRMED ✅

- **`handshake_warm_eligible` (src/core/notifier.rs:1141) — rejects ≥2 boards:**
  ```rust
  fn handshake_warm_eligible(candidate_count: usize) -> bool {
      candidate_count <= 1
  }
  ```
- **`warm_cache_from_handshake` (src/core/notifier.rs:1163) — early-returns when
  not eligible:**
  ```rust
  fn warm_cache_from_handshake(kind: DeviceKind) {
      let candidates = enumerate_candidates();
      if !handshake_warm_eligible(candidates.len()) {
          return;   // ≥2 boards: skip the warm-stamp, leave to classify_devices
      }
      for c in candidates {
          classification_cache_insert(&c.path, kind.clone());
      }
  }
  ```
  Without this guard, a mixed desk (capable + pure-VIA) would have BOTH paths
  warm-stamped from the single broadcast reply → the VIA board mislabeled
  `✓ qmk_notifier` until TTL expiry. Confirmed.

### (c) Trayless caveat — CONFIRMED ✅ (line drift: actual block 30-33, not 26-32)

- **`src/runners/linux.rs` startup handshake — runs once at startup if connected:**
  ```rust
  // lines 28-33 (contract cited "26-32"; actual: comment 28-29, if-block 30-32/33):
  // If a device is already connected at startup, run the capability handshake
  // now (poll-thread reconnects are handled in linux_tray.rs / tray.rs).
  // Completes before the poll thread exists; idempotent via HAS_HANDSHAKED.
  if crate::core::notifier::is_device_connected() {
      crate::core::notifier::perform_handshake(self.verbose);
  }
  ```
  The trayless (`--no-default-features`) build has no SNI tray → no poll thread →
  this one-shot startup handshake is the ONLY handshake. An unplug/replug after
  startup is not re-handshaked (host rules won't resume without a restart).

- **systemd template (`packaging/linux/systemd/qmkonnect.service.template`) —
  BindsTo + Restart=always:**
  - Line 10: `BindsTo=dev-qmkonnect_device.device` (stops the unit on unplug;
    waits for it at boot).
  - Line 22: `Restart=always` (+ `RestartSec=5`, line 23).
  This is WHY the trayless one-shot is acceptable: unplug stops the unit, replug
  (re)starts it → re-runs the startup handshake. Confirmed.

### (d) Status-poll never pings on a stable bus — CONFIRMED ✅

- **`PresenceTracker::tick` (src/core/notifier.rs:1311) — path-set-gated re-probe:**
  ```rust
  pub fn tick(&mut self, verbose: bool) -> HandshakeAction {
      let paths = tier1_paths();
      let tier1_present = !paths.is_empty();
      let paths_changed = paths != self.last_paths;
      let reprobed = if paths_changed && tier1_present {
          Some(
              classify_devices(verbose)   // <-- ONLY in the paths_changed branch
                  .iter()
                  .any(|d| matches!(d.kind, DeviceKind::Capable { .. })),
          )
      } else {
          None    // <-- stable bus: no classify_devices call, no ping
      };
      let (action, capable) =
          presence_tick_decision(self.last_capable, paths_changed, tier1_present, reprobed);
      self.last_capable = capable;
      self.last_paths = paths;
      action
  }
  ```
  On a stable bus, `paths == self.last_paths` ⇒ `paths_changed = false` ⇒
  `reprobed = None` ⇒ `classify_devices` is NEVER called ⇒ no ping. The pure
  `presence_tick_decision` (line 1249) confirms: `!paths_changed` ⇒ `capable =
  last_capable` (reused). Confirmed.

## Line-number drift to note in the report

The contract cited `src/runners/linux.rs:26-32` for caveat (c). The ACTUAL
startup-handshake block is at **lines 30-33** (the explanatory comment spans
28-29; the `if is_device_connected() { perform_handshake(...) }` is 30-32, with
the closing brace on 33). This is a ±4 line drift, not a behavioral change. The
audit report should record the ACTUAL lines read (30-33), not the contract's
26-32, so the evidence is accurate. All other cited line numbers (428, 1123,
1141, 1163, 1311) match exactly.

## Where the deliverable report goes

DOCS = none per contract (read-only audit). The 4-row report goes in the plan
research area: `plan/006_8f4080956ee0/P1M1T1S3/research/audit_report.md` (not a
product doc / spec / src change).

## Verification approach (read-only)

1. For each caveat, READ the cited code lines (anchors above).
2. Confirm the behavior matches the "expected" description.
3. Record Confirmed Y/N + the actual line range read.
4. `git status --short src/` must stay clean (no edits).
5. All 4 CONFIRMED ⇒ overall verdict "delta verified-complete".

There are no unit tests for this audit (it's a manual code-read). The proof IS
the 4-row table + the verbatim code snippets captured in it.

## Sources verified (direct code read)
- src/core/notifier.rs:428 (perform_handshake_with HAS_HANDSHAKED.swap).
- src/core/notifier.rs:1123-1128 (classify_devices body — no gate).
- src/core/notifier.rs:1141 (handshake_warm_eligible: candidate_count <= 1).
- src/core/notifier.rs:1163-1173 (warm_cache_from_handshake early-return).
- src/core/notifier.rs:1235-1357 (tier1_paths, presence_tick_decision, PresenceTracker::tick).
- src/runners/linux.rs:28-33 (startup handshake one-shot).
- packaging/linux/systemd/qmkonnect.service.template:10,22 (BindsTo + Restart=always).
- plan/006_8f4080956ee0/architecture/verification_findings.md §1/§1.5 (prior assertion re-confirmed).
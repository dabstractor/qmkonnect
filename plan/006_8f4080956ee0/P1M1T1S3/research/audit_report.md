# Audit Report — P1.M1.T1.S3: Confirm documented caveats have backing behavior

> **Task ID:** P1.M1.T1.S3
> **Task type:** Mode C read-only code-audit gate (closes P1.M1.T1)
> **Repo:** `/home/dustin/projects/qmkonnect`
> **Audit target:** the capability-keyed-lifecycle delta shipped at commit `d240b27`
> **Method:** direct code read at the cited line ranges (independent re-confirmation of
> `verification_findings.md` §1/§1.5 — not a transcription).
> **No source/spec/packaging file modified** — `git status --short src/ spec/ packaging/`
> clean before and after (read-only invariant honored).

---

## Overall Verdict: **DELTA VERIFIED-COMPLETE** ✅

All four documented caveats of the capability-keyed-lifecycle delta are **confirmed
backed by real code** at the cited locations. Each row below records the actual code
read, the expected behavior, the verbatim snippet captured as evidence, and the
Confirmed Y/N verdict.

| # | Caveat | Code location read | Expected behavior | Confirmed |
|---|--------|--------------------|-------------------|-----------|
| (a) | Proto-v1 picker — handshake dedups on `HAS_HANDSHAKED`; the picker does NOT (the asymmetry) | `src/core/notifier.rs:428` (`perform_handshake_with` swap) + `:1122-1127` (`classify_devices` body, no gate) | handshake gates `if HAS_HANDSHAKED.swap(true, SeqCst) { … return; }`; `classify_devices` has NO `HAS_HANDSHAKED` reference | **Y** |
| (b) | Warm-feed scope — handshake warm-stamp is skipped with ≥2 boards (broadcast-can't-attribute guard) | `src/core/notifier.rs:1141-1143` (`handshake_warm_eligible`) + `:1164-1167` (`warm_cache_from_handshake` early-return) | `handshake_warm_eligible(n)` returns `n <= 1`; `warm_cache_from_handshake` early-returns `if !handshake_warm_eligible(candidates.len()) { return; }` | **Y** |
| (c) | Trayless startup handshake — no-tray build runs the handshake once at startup; BindsTo+Restart recover unplug/replug | `src/runners/linux.rs:30-33` (startup handshake) + `packaging/linux/systemd/qmkonnect.service.template:8,20` (BindsTo + Restart) | `if is_device_connected() { perform_handshake(self.verbose); }` at startup; template has `BindsTo=dev-qmkonnect_device.device` + `Restart=always` | **Y** |
| (d) | No ping on a stable bus — `PresenceTracker::tick` only calls `classify_devices` when the path set changed | `src/core/notifier.rs:1311-1323` (`PresenceTracker::tick`) + `:1249-1264` (`presence_tick_decision` pure core) | `tick` sets `paths_changed = paths != self.last_paths`; calls `classify_devices` ONLY inside `if paths_changed && tier1_present { Some(...) } else { None }`; stable bus ⇒ `reprobed = None` ⇒ no ping | **Y** |

---

## Per-caveat evidence (verbatim snippets)

### (a) Proto-v1 picker dedup asymmetry — Confirmed = Y

**(a1) `perform_handshake_with` opens with the `HAS_HANDSHAKED` swap — `src/core/notifier.rs:427-435`:**
```rust
pub fn perform_handshake_with(verbose: bool, opts: HandshakeOptions) {
    // Dedup: at most once per board boot (firmware has_been_queried). S2 resets.
    if HAS_HANDSHAKED.swap(true, Ordering::SeqCst) {
        if verbose {
            eprintln!(
                "[{}ms] perform_handshake: already handshaked this session — skipping",
                crate::core::now_ms()
            );
        }
        return;
```

**(a2) `classify_devices` body has NO `HAS_HANDSHAKED` reference — `src/core/notifier.rs:1121-1127`:**
```rust
#[allow(dead_code)]
pub fn classify_devices(verbose: bool) -> Vec<ClassifiedDevice> {
    let candidates = enumerate_candidates();
    invalidate_absent_cache_entries(&candidates);
    classify_candidates(candidates, verbose)
}
```
**Asymmetry confirmed:** the handshake dedups (`HAS_HANDSHAKED.swap`); the picker (`classify_devices`) does not — its body is `enumerate_candidates() → invalidate_absent_cache_entries(&candidates) → classify_candidates(candidates, verbose)` with no dedup gate.

> **`#[allow(dead_code)]` non-issue:** `classify_devices` is dead_code because the
> discovered-device picker that calls it ships later (P3.M2). The function EXISTS now
> with the no-gate body — which is exactly what caveat (a) audits. `dead_code ≠
> unimplemented`. (Confirmed via `awk '/fn classify_devices/,/^}/' src/core/notifier.rs |
> grep 'HAS_HANDSHAKED'` → no match.)

### (b) Warm-feed scope guard — Confirmed = Y

**(b1) `handshake_warm_eligible` returns true only for ≤1 board — `src/core/notifier.rs:1141-1143`:**
```rust
fn handshake_warm_eligible(candidate_count: usize) -> bool {
    candidate_count <= 1
}
```

**(b2) `warm_cache_from_handshake` early-returns when ineligible — `src/core/notifier.rs:1164-1167`:**
```rust
fn warm_cache_from_handshake(kind: DeviceKind) {
    let candidates = enumerate_candidates();
    if !handshake_warm_eligible(candidates.len()) {
        return;
    }
```
**Scope guard confirmed:** with ≥2 Tier-1 boards the broadcast handshake result cannot be attributed to a single path, so the warm-stamp is skipped (`candidate_count <= 1` gate).

### (c) Trayless startup handshake + systemd recovery — Confirmed = Y

> ⚠ **Line-drift note (an observation, NOT a failure):** the contract cited
> `src/runners/linux.rs:26-32` for the startup handshake. The **actual** block is at
> **lines 30-33** (explanatory comment spans 28-29; the `if is_device_connected() {
> perform_handshake(...) }` is lines 30-32; closing brace 33). Behavior is identical
> to the contract's claim; only the line anchor drifted ±4. Recorded here per the
> PRP Gotchas. The systemd template lines also drifted slightly ahead (`BindsTo` at
> line 8 vs cited :10; `Restart=always` at line 20 vs cited :22).

**(c1) Startup handshake one-shot — `src/runners/linux.rs:28-33`:**
```rust
        // If a device is already connected at startup, run the capability handshake
        // now (poll-thread reconnects are handled in linux_tray.rs / tray.rs).
        // Completes before the poll thread exists; idempotent via HAS_HANDSHAKED.
        if crate::core::notifier::is_device_connected() {
            crate::core::notifier::perform_handshake(self.verbose);
        }
```

**(c2) systemd template — BindsTo + Restart=always (`packaging/linux/systemd/qmkonnect.service.template:8,20`):**
```
8:BindsTo=dev-qmkonnect_device.device
...
20:Restart=always
```
**Recovery confirmed:** this is WHY the trayless one-shot startup handshake is acceptable — unplug stops the unit (BindsTo), replug restarts it (Restart=always) → re-runs the handshake on the next boot.

### (d) No ping on a stable bus — Confirmed = Y

**(d1) `PresenceTracker::tick` gates `classify_devices` on `paths_changed` — `src/core/notifier.rs:1311-1323`:**
```rust
    pub fn tick(&mut self, verbose: bool) -> HandshakeAction {
        let paths = tier1_paths();
        let tier1_present = !paths.is_empty();
        let paths_changed = paths != self.last_paths;
        let reprobed = if paths_changed && tier1_present {
            Some(
                classify_devices(verbose)
                    .iter()
                    .any(|d| matches!(d.kind, DeviceKind::Capable { .. })),
            )
        } else {
            None
        };
```
**Stable-bus guard confirmed:** `classify_devices` is called ONLY inside the
`paths_changed && tier1_present` branch. On a stable bus (`!paths_changed`),
`reprobed = None` ⇒ no `QUERY_INFO` ping.

**(d2) `presence_tick_decision` pure core reuses `last_capable` when `!paths_changed` — `src/core/notifier.rs:1249-1264`:**
```rust
fn presence_tick_decision(
    last_capable: bool,
    paths_changed: bool,
    tier1_present: bool,
    reprobed_capable: Option<bool>,
) -> (HandshakeAction, bool) {
    let capable = if !tier1_present {
        false
    } else if paths_changed {
        reprobed_capable.unwrap_or(false)
    } else {
        last_capable
    };
    (handshake_action(Some(last_capable), capable), capable)
}
```
**Rationale (from the function's own doc-comment):** "`!paths_changed` ⇒ the bus is
stable; `reprobed_capable` is `None` and the last known flag is reused (a board
cannot change firmware without a replug, which changes the path set)."

---

## Independence check (agreement with `verification_findings.md` §1/§1.5)

These four code reads were performed directly against the files in this session (not
transcribed from the prior research). They agree with `verification_findings.md`
§1/§1.5 at every site:

| Caveat | §1/§1.5 assertion | S3 independent read | Agreement |
|--------|-------------------|---------------------|-----------|
| (a) | `perform_handshake_with` `HAS_HANDSHAKED.swap` @428; `classify_devices` no-gate @1123 | swap @428; no-gate body @1122-1127 (no `HAS_HANDSHAKED`) | ✅ same behavior |
| (b) | `handshake_warm_eligible` @1141; `warm_cache_from_handshake` early-return @1163 | `<= 1` @1141-1143; early-return @1164-1167 | ✅ same behavior |
| (c) | `runners/linux.rs` startup handshake; BindsTo + Restart in template | startup handshake @30-33 (drift from cited 26-32); BindsTo @8 + Restart @20 (drift ±2) | ✅ same behavior (line anchors drifted) |
| (d) | `PresenceTracker::tick` @1311; only calls `classify_devices` in `paths_changed` branch | `paths_changed`-gated `classify_devices` @1311-1323; pure core `presence_tick_decision` @1249-1264 | ✅ same behavior |

---

## No-edit invariant (read-only audit)

```
$ git status --short src/ spec/ packaging/
(empty)
```
The audit modified no source, spec, or packaging file. The only artifact produced is
this report, which lives under the `plan/.../research/` area (not committed into
`src/`, `spec/`, or `docs/`).

---

## Conclusion

**P1.M1.T1.S3 PASSES.** The capability-keyed-lifecycle delta (commit `d240b27`) is
**verified-complete**: all four documented caveats — (a) proto-v1 picker dedup
asymmetry, (b) warm-feed ≥2-board scope guard, (c) trayless startup handshake +
systemd recovery, (d) no-ping-on-stable-bus — are confirmed backed by real code at
the cited locations, with verbatim snippets captured as evidence. The reads agree
with `verification_findings.md` §1/§1.5 (independent re-confirmation). The one
line-drift (caveat (c): actual `runners/linux.rs:30-33` vs the contract's 26-32;
template BindsTo/Restart at :8/:20 vs cited :10/:22) is an observation, not a
failure — the behavior is identical. Together with a green S1 (gates/tests) and a
green S2 (spec drift zero), this closes P1.M1.T1 ("Confirm documented caveats have
backing behavior") and the milestone M1 verification gate.
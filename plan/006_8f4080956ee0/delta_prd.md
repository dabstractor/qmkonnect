# Delta PRD — Capability-keyed discovery lifecycle (spec clarification, already realized)

**Delta from:** session 5 (`plan/005_8b95ea464bd9`)
**Scope:** Device discovery + capability status lifecycle (F13 refinement)
**Net new code:** ~0 — this delta is a **specification clarification that was already
shipped to code + spec during the prior session**.

---

## 1. What actually changed (diff analysis)

A line-level diff of `plan/005_8b95ea464bd9/prd_snapshot.md` vs
`plan/006_8f4080956ee0/prd_snapshot.md` is **94 diff lines**, all of them in
three spec files, all describing **one coherent refinement** of the
discovery/status lifecycle. No source file is touched by the spec delta itself.

| # | Spec location | Change |
|---|---|---|
| 1 | `DEVICE_DISCOVERY.md §2.2` | "No board is ever harmed" → "No **proto-v2 or pure-VIA** board is harmed"; adds the **proto-v1 caveat** (the one legacy exception: proto-v1 firmware walks `QUERY_INFO` through the string path → `process_full_message("")` briefly deactivates the layer). |
| 2 | `DEVICE_DISCOVERY.md §2.4` | **NEW:** handshake → cache warm-feed scope. The handshake warm-feeds its `QUERY_INFO` into `CLASSIFICATION_CACHE` so the first `classify_devices` reads a TTL hit — but **only when a single Tier-1 board is present** (broadcast can't attribute per-path). |
| 3 | `DEVICE_DISCOVERY.md §3` | Status probe is **capable-keyed** (not Tier-1-keyed): a `PresenceTracker` remembers the Tier-1 **path set** and re-probes capability only on a plug/unplug. This makes the headline mixed-multi-board case truthful (capable board unplugged while a VIA board remains ⇒ real `Loss` ⇒ "No module"). |
| 4 | `LINUX.md §6.2` | **NEW:** trayless (`--no-default-features`) build caveat — no poll thread ⇒ handshake runs **once at startup**; `BindsTo=…device` + `Restart=always` covers replug. |
| 5 | `LINUX.md §7.1` | Poll thread now "drives a `PresenceTracker` tick" (was: "re-probe `is_device_connected()`"); tray field renamed `device_status`. |
| 6 | `HOST_RULES.md §13 R6` | R6 expanded with the proto-v1 exception detail (mirrors #1). |

**Proportional sizing check:** 6 small spec-text edits, one behavioral mechanism
(`PresenceTracker` + warm-feed scope + documented caveats). This is a
**medium-small** delta — *not* a multi-phase feature build.

---

## 2. Implementation status — ALREADY COMPLETE

The behavior this delta describes was **implemented during the prior session**
after its bug-hunt (`plan/005_8b95ea464bd9/bugfix/creative-bug-hunt-report-2.md`,
finding "🟠 MEDIUM — Mixed multi-board partial unplug is undetected") identified
the Tier-1-keyed lifecycle bug. It shipped in commit `d240b27` *"Key handshake
lifecycle on capable-board presence, not Tier-1"*.

Verified present in the current tree (`src/`, `spec/`):

| Delta requirement | Realized at | Evidence |
|---|---|---|
| `PresenceTracker` (capable-keyed, path-set-gated) | `src/core/notifier.rs:1292` | `struct PresenceTracker` + `tick()` + `presence_tick_decision()` (pure core, `:1251`) + `tier1_paths()` (`:1233`) |
| Wired into both tray poll threads | `src/tray.rs:439,452`; `src/linux_tray.rs:288,302` | `PresenceTracker::new()` + `presence.tick(verbose)` on the 3s (macOS/Win) and 1s (Linux) loops |
| Three-state status reads capable state | `src/core/notifier.rs:783` `device_status()` | both trays call `device_status()` on transition; `linux_tray.rs` field renamed `device_status` (`:85`) |
| Handshake → cache warm-feed + single-board scope guard | `src/core/notifier.rs:1163` `warm_cache_from_handshake`, `:1141` `handshake_warm_eligible` | called from `perform_handshake_with` at `:567,594,607,625`; unit-tested at `:4097` |
| Proto-v1-safe dedup on the handshake path | `src/core/notifier.rs:298` `HAS_HANDSHAKED` (swapped `:428`) | `classify_devices` (`:1123`) deliberately does **not** gate on it → picker probe sits outside the dedup (the documented caveat) |
| Trayless startup handshake | `src/runners/linux.rs:31-32` | runs `perform_handshake` once if `is_device_connected()` at startup; `BindsTo`/`Restart` cover replug (service template unchanged) |
| Spec files at v6 wording | `spec/DEVICE_DISCOVERY.md`, `spec/LINUX.md`, `spec/HOST_RULES.md` | all contain `PresenceTracker`/`proto-v1`/`warm-feed`/`capable-keyed`/`device_status` |
| Test coverage for the new mechanisms | `src/core/notifier.rs` | 8 targeted tests incl. `test_presence_tick_capable_unplug_with_non_capable_remaining_is_loss`, `…_capable_replug_different_board_is_gain`, `…_stable_bus_no_reprobe_no_action`, `test_handshake_warm_eligible_single_board_only`, `test_classify_candidates_mixed` |

**Conclusion:** there is **no new feature work, no new code, and no doc edits**
required to satisfy the v6 PRD. The remaining work is **verification only**.

---

## 3. Scope of the delta (the only work)

A single **verification milestone**: confirm the already-shipped implementation
satisfies the v6 spec, the quality gates are green, and there is zero spec/code
drift. If verification surfaces a gap (unexpected — flag it explicitly), fix it
in place.

### Phase V1 — Verify the capability-keyed lifecycle delta is complete

**Milestone V1.M1 — Verify code/spec/tests in sync and quality gates green**

**Task V1.M1.T1 — Verify the already-shipped delta: gates + drift + caveat-backing**
- **V1.M1.T1.S1** — Run the project quality gates per `AGENTS.md` /
  `plan/005_8b95ea464bd9/validate.sh` and confirm green:
  `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`;
  `cargo build --release`; `cargo test --bin qmkonnect -- --test-threads=1`
  (single-threaded — shared global debouncer state, AGENTS.md). Report the test
  count and confirm the 8 PresenceTracker / warm-feed / classify-mixed tests pass.
- **V1.M1.T1.S2** — Confirm **zero spec drift**: the four spec files touched by
  the delta (`DEVICE_DISCOVERY.md`, `LINUX.md`, `HOST_RULES.md`) match the v6
  snapshot wording (PresenceTracker / proto-v1 caveat / warm-feed scope /
  trayless caveat). Assert each of the 6 diff hunks in §1 has a corresponding
  passage in `spec/`.
- **V1.M1.T1.S3** — Confirm the **documented caveats have backing behavior**
  (read-only audit, no code change expected):
  - proto-v1 picker caveat — `classify_devices` (`src/core/notifier.rs:1123`) does
    **not** gate on `HAS_HANDSHAKED`, while `perform_handshake_with` (`:428`) does;
  - warm-feed scope — `warm_cache_from_handshake` early-returns on
    `!handshake_warm_eligible(candidates.len())` (`:1165`), and the guard rejects
    ≥2 boards (`:4097`);
  - trayless caveat — `runners/linux.rs:31-32` runs the startup handshake;
  - status-poll never pings on a stable bus — `PresenceTracker::tick` only calls
    `classify_devices` when `paths != self.last_paths`.
  Report findings. Only if an audit item fails does this delta gain a follow-on
  fix task; otherwise the delta closes as **verified-complete**.

---

## 4. Documentation impact

- **Mode A (doc-with-work):** **none.** The spec files (`spec/*.md`) are already
  at the v6 wording (shipped in `d240b27`), and the code already carries the
  explanatory doc-comments (`PresenceTracker`, `warm_cache_from_handshake`,
  `presence_tick_decision`, the proto-v1 picker-caveat comment on
  `classify_devices`). No additional inline or file doc is required.
- **Mode B (changeset-level docs):** **none.** `README.md`, `docs/*.md`, and
  `docs/llms_full.txt` were regenerated in the prior session (commit `293f565`)
  and are not affected by this lifecycle refinement — it is an internal
  correctness/edge-case fix, not a user-facing capability change. No
  changeset-level doc sweep is warranted.

---

## 5. Risks / open questions

- **Risk:** a future reader assumes the v6 PRD describes *new* work and rewrites
  the (already-correct) `PresenceTracker` / warm-feed. **Mitigation:** the
  verification milestone's drift + caveat-backing subtasks (V1.M1.T1.S2/S3)
  make the "already done" status explicit; any implementation task here would be
  a regression, not an addition.
- **Open:** none. The proto-v1 picker-caveat and trayless-handshake caveat are
  documented *limitations* (deliberate, with recovery paths), not defects to fix.
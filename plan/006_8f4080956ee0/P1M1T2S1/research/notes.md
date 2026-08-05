# Research Notes — P1.M1.T2.S1: verify README.md + docs/ overview not stale vs the capability-keyed lifecycle

Repo: **`/home/dustin/projects/qmkonnect`**. This is the **docs-side verification**
subtask — the Mode-B counterpart to S3's read-only code audit. Expected outcome:
**VERIFIED: no documentation drift** (a no-op). The research below reproduces that
verdict with exact grep commands; the PRP is the runbook the implementer re-runs.

## 0. What "the v6 capability-keyed lifecycle" IS (the thing docs must NOT contradict)

Shipped to code + spec in commit **`d240b27`** ("Key handshake lifecycle on
capable-board presence, not Tier-1"). It is an **internal correctness refinement**
on top of F13/F14 (the user-visible three-state status + capability probe). The
six v6 semantics, and the prose contradiction each would create if a doc made the
wrong claim:

| # | v6 semantic (internal) | DRIFT SIGNATURE (a doc claim that contradicts it) |
|---|------------------------|---------------------------------------------------|
| 1 | Status is **capable-keyed**, not Tier-1-keyed (`PresenceTracker` re-probes capable presence only when the Tier-1 *path set* changes) | doc claims status is "Tier-1-keyed" / "keyed on Tier-1", or that the status poll pings/probes **every cycle** |
| 2 | **Three-state** device status (Connected / No module / Disconnected), refreshed **only on a transition** | doc claims a boolean/two-state status, or that status refreshes "every poll" |
| 3 | Warm-feed is **single-board-only** (`handshake_warm_eligible`: `candidate_count <= 1`) | doc promises "single ping per appearance" **without the single-board qualifier** |
| 4 | Broadcast goes to **capable** boards (not "every `0xFF60` interface") | doc claims burst/broadcast/send to "every `0xFF60` interface" / "all matching interfaces" |
| 5 | `is_device_connected()` is a **Tier-1-only** predicate used by the write path | doc claims `is_device_connected` reflects capable status |
| 6 | Trayless build handshakes **once at startup**; `BindsTo`+`Restart=always` recover unplug/replug | doc claims the trayless build re-handshakes on replug directly (without BindsTo/Restart) |

§5 of `verification_findings.md` asserts "Mode B: NONE expected" — i.e. user docs
are expected to be **silent on these internals** (they're internal correctness
mechanisms) and **accurate** on the user-visible parts (the three-state status).
This research confirms exactly that.

## 1. Scope (the files under verification)

Per the item contract:
- **`README.md`** (top-level overview) — primary.
- **`docs/*.md` overview files** — `index.md`, `usage.md`, `installation.md`,
  `configuration.md`, `troubleshooting.md`, `qmk-integration.md`, `examples.md`,
  `README.md` (docs/README.md is the GitHub-pages index).
- **`docs/llms_full.txt`** — a **derived** artifact (`docs/generate_llms_full.sh`
  concatenates the docs/*.md; regenerated in commit `293f565`). It cannot drift
  independently of docs/*.md — a clean docs/*.md ⇒ clean llms_full.txt. The
  canonical check is against README.md + docs/*.md; llms_full.txt is included in
  the grep scope as a belt-and-suspenders confirmation (and to satisfy the item's
  "docs/llms_full.txt" input), but it is NOT hand-edited (regeneration is the
  mechanism, owned by a different step).

## 2. Verified grep scan (the ground truth — reproduces "no drift")

All run from repo root, `grep -v llms_full.txt` to check the source docs first.

### 2a. Internal-mechanism terms — ALL ZERO in user docs (the "silent" property)

```
Term (ERE)            README.md + docs/*.md (excl llms_full.txt)
Tier-?1               0
Tier-?2               0
capab                 20   ← see §2c (all are user-visible "capable" markers, accurate)
[Pp]resence           0
classify              0
[Bb]roadcast          0
warm                  0
is_device_connected   0
handshake             2    ← see §2c (both accurate/neutral)
```

→ PresenceTracker / classify_devices / warm-feed / Tier-keying / is_device_connected
appear **nowhere** in user docs. The docs are silent on the d240b27 internals, as §5
predicted.

### 2b. Targeted drift signatures — ALL NONE (no contradictions)

```
(1) tier.?1.?key / keyed on tier          → none ✓   (no Tier-1-keyed status claim)
(2) two.?state / boolean status            → none ✓   (docs say "three-state", not two)
(3) (broadcast|send|ping|burst).{0,40}
    (every|all).{0,20}(0xFF60|interface)   → none ✓   (no "every 0xFF60 interface" claim)
(4) single ping / one ping / ping…once     → none ✓   (no unqualified single-ping promise)
(5) (every|each) poll / refresh…every      → none ✓   (no per-poll status-refresh claim)
```

### 2c. The mentions that DO exist are all v6-ACCURATE

- **Three-state status** (the v6 user-visible surface): `docs/usage.md:112-122`,
  `docs/troubleshooting.md:105-110`, `docs/installation.md:218`, `README.md:217-220`
  all describe `● Device Connected` (qmk_notifier-capable present) /
  `⚠ QMK board found — no qmk_notifier module (flash it)` /
  `○ No Device Connected` — this is **exactly** DEVICE_DISCOVERY.md §3 / UI.md §4's
  Connected/No-module/Disconnected table. v6-accurate.
- **Capable / no-module picker markers** (`✓ qmk_notifier-capable` /
  `✗ QMK board, no module`): `README.md:217-220`, `configuration.md:19,30,35-36`,
  `installation.md:105`, `troubleshooting.md:139-140` — the discovered-device picker
  UX (F13/F14). v6-accurate.
- **2 handshake mentions**: `configuration.md:231` (`--list-callbacks` "Handshake the
  connected keyboard and print its callback name→id table" — accurate CLI action) and
  `qmk-integration.md:154` ("the capability handshake" as a component QMKonnect owns
  — accurate, neutral). Neither makes a capable-keyed/warm-feed contradiction.

### 2d. The 6 "ping|probe" hits are FALSE POSITIVES (substring matches)

```
examples.md:226      "Typing-focused layout"        (ping ⊂ Typing)
examples.md:319,320  "…keeping a callback rule…"    (ping ⊂ keeping)
installation.md:170  "…Skipping it is…"             (ping ⊂ Skipping)
qmk-integration.md:213 "Keeping it in both…"        (ping ⊂ Keeping)
usage.md:32          "## Stopping QMKonnect"        (ping ⊂ Stopping)
```
→ **zero** actual device-ping/probe mentions in user docs. ✓

## 3. Verdict (the pre-verified result the runbook reproduces)

**VERIFIED: no documentation drift.** The user docs are:
- **Silent** on every d240b27 internal mechanism (PresenceTracker, classify_devices,
  warm-feed, Tier-1/capable-keying, is_device_connected, single-ping-per-appearance,
  broadcast internals) — 0 hits each.
- **Accurate** on the user-visible surface they DO describe (the three-state status,
  the capable/no-module picker markers) — all match the v6 spec tables.
- **Free of contradictory claims** — all 5 targeted drift signatures return none.

This agrees with `verification_findings.md` §5 ("Mode B: NONE expected") exactly.

## 4. What the implementer does (no-op unless drift reappears)

1. Re-run the §2 grep scan + the §2b targeted signatures against the live tree
   (independent re-confirmation — same pattern as S3). Expect the same "all zero /
   all none" result.
2. Spot-read the three-state status sections (usage.md ~112-122, troubleshooting.md
   ~105-110) to confirm they still read `Connected`/`No module`/`Disconnected`.
3. If (and only if) a contradiction is found, fix the affected overview doc **in
   place** (Mode B) — otherwise the task closes as a **no-op** with the one-line
   verdict. No `src/` or `spec/` edits under any branch (those are S3/S2 scope).
4. `git status` for `src/` and `spec/` MUST stay clean (this is a docs-only task).
   If no drift: `git status` for `docs/` + `README.md` also clean (pure verification).

## 5. Scope boundaries (what NOT to do)

- **No `src/` or `spec/` edits** — the delta is already shipped there (S1/S2/S3 own
  the code/spec verification). This task is docs-only.
- **No `docs/llms_full.txt` hand-edit** — it's a derived concatenation; if it were
  stale it would be regenerated (a separate mechanical step), not hand-patched.
- **No spec docs** (`spec/*.md`) — they are the v6 source of truth (already carry
  the wording, verified in S2). Copy FROM them conceptually; never edit.
- **Don't "fix" the v6-accurate three-state status or capable markers** — those are
  CORRECT. Only a genuine contradiction (one of the 5 drift signatures) is a fix.
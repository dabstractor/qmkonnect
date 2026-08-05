# Research Notes — P1.M1.T1.S2 (Confirm zero spec drift: 6 v6 diff hunks)

## 0. Task shape

This is a **read-only spec-drift audit**. No code/spec edits. The deliverable is a
6-row PASS/FAIL table proving each of the 6 v6 diff hunks described in the delta PRD §1
has a corresponding passage (in v6 wording) in its spec file. The "implementation" IS
running the greps and producing the report.

S2 assumes the spec files are at HEAD (commit `f4315a6`; the capability-keyed lifecycle
delta shipped in `d240b27`). S1 (parallel sibling) runs the quality gates; S3 is the
caveat-backing code audit. S2 must NOT edit any spec file — `git status` for `spec/`
must stay clean.

## 1. The three spec files and which hunks touch them

- `spec/DEVICE_DISCOVERY.md` — hunks #1 (§2.2), #2 (§2.4), #3 (§3)
- `spec/LINUX.md` — hunks #4 (§6.2), #5 (§7.1)
- `spec/HOST_RULES.md` — hunk #6 (§13 R6)

## 2. Independent re-confirmation — ALL 6 HUNKS PASS (greps run directly against the files)

The item says "All 6 diff hunks have been verified present during this research phase.
This subtask re-confirms that assertion independently." Below is that independent
re-confirmation: every grep was run directly against the spec file in this session
(not transcribed from `verification_findings.md` §3, though it agrees).

| Hunk | Spec file §  | Contracted key phrase(s) | Found | Line(s) | Verbatim fragment |
|---|---|---|---|---|---|
| #1 | DEVICE_DISCOVERY.md §2.2 | "No proto-v2 or pure-VIA board" + `process_full_message("")` deactivation | ✅ Y | 81, 85, 90 | "**No proto-v2 or pure-VIA board is** harmed by the probe" (81); "no `process_full_message` side effect" (85); "`process_full_message("")` deactivates the active board layer" (90) |
| #2 | DEVICE_DISCOVERY.md §2.4 | "Handshake → cache warm-feed scope" + single-Tier-1-board guard | ✅ Y | 155, 159 | "**Handshake → cache warm-feed scope.**" (155); "this warm-feed is **correct only when a single Tier-1 board is present**" (159) |
| #3 | DEVICE_DISCOVERY.md §3 | "capable-keyed" + "PresenceTracker" + re-probe only on plug/unplug | ✅ Y | 189, 191 (also 97) | "**capable-keyed** (not Tier-1-keyed): a `PresenceTracker` remembers the Tier-1" (189); "**only when the path set changes** — a physical plug/unplug" (191) |
| #4 | LINUX.md §6.2 | trayless (`--no-default-features`) caveat + `BindsTo` + `Restart=always` | ✅ Y | 211, 216, 217 | "**Trayless (`--no-default-features`) build caveat.**" (211); "`BindsTo=dev-qmkonnect_device.device` stops the unit on unplug" (216); "`Restart=always` (re)starts it on replug" (217) |
| #5 | LINUX.md §7.1 | "PresenceTracker tick" + `device_status` field | ✅ Y | 236, 240 | "drive a `PresenceTracker` tick" (236); `handle.update(|t| { t.device_status = …; …})` (240) |
| #6 | HOST_RULES.md §13 R6 | R6 expanded + proto-v1 exception detail + `PresenceTracker` reference | ✅ Y | 616, 626, 627 | "**R6 — Legacy handshake side effect — RESOLVED.**" (616); "on proto-v1 firmware it can briefly reset the layer per probe" (626); "see `PresenceTracker`" (627) |

**Overall verdict: ZERO SPEC DRIFT.** All 6 v6 diff hunks have corresponding passages
in v6 wording in the spec files. (Agrees with `verification_findings.md` §3.)

## 3. The hunk #5 nuance (why a naive grep misses it — important for the runbook)

The item phrases hunk #5's key phrase as "'PresenceTracker tick'". In the actual spec
file (`spec/LINUX.md:236`) the text is:

> "Poll thread: every **1 s** drive a `PresenceTracker` tick (re-probes capable
> presence via the cache-backed `classify_devices` only when the Tier-1 path *set*
> changes — a plug/unplug — so the hot loop never pings on a stable bus)"

Note `PresenceTracker` is **backtick-wrapped** (it's an inline code span), and there is
a backtick + space between "PresenceTracker" and "tick". Consequences for the grep:
- ❌ `grep -nF 'PresenceTracker tick' spec/LINUX.md` → **NO match** (the literal string
  "PresenceTracker tick" is broken by "` ").
- ✅ `grep -nE 'PresenceTracker.+tick' spec/LINUX.md` → matches :236 (the `.+` eats "` ").
- ✅ or `grep -nE 'drive a .*PresenceTracker' spec/LINUX.md` → matches :236.

The PRP runbook MUST use the regex-tolerant form (`PresenceTracker.+tick`), not the
literal phrase, or the audit will wrongly report hunk #5 as FAIL. This is the single
most likely one-pass error and is called out in the PRP gotchas.

## 4. Incidental observation (NOT a hunk failure — out of S2 scope, just noted)

While reading LINUX.md §7.1, the `spawn()` bullet at line ~230 shows the constructor
pseudo-code as `QmkTray { device_connected, dark_mode }`, whereas the CODE field was
renamed `device_status` (confirmed at `src/linux_tray.rs:85` per verification_findings
§1.2; and the spec itself uses `t.device_status` at :240). So the §7.1 spawn() pseudo-
code may carry a stale field name (`device_connected`) at the construction site even
though the update site (`:240`) uses the new `device_status`.

This is **NOT** one of the 6 contracted hunks (hunk #5's contract is only the two key
phrases "PresenceTracker tick" + "device_status", both PRESENT). S2's verdict for hunk
#5 is therefore PASS. The incidental `device_connected`→`device_status` rename at the
spawn() pseudo-code line is a *separate* minor doc-staleness item outside this audit's
6-hunk scope; flag it in the report's "incidental observations" but do NOT count it as
a hunk failure, and do NOT edit the spec to fix it (S2 is read-only; a fix would be a
separate doc task, e.g. P1.M1.T2.S1).

## 5. The exact greps to run (one per hunk, all verified to hit)

```bash
cd /home/dustin/projects/qmkonnect

# Hunk #1 — DEVICE_DISCOVERY.md §2.2
grep -nE 'No proto-v2 or pure-VIA' spec/DEVICE_DISCOVERY.md            # expect :81
grep -nF 'process_full_message("")' spec/DEVICE_DISCOVERY.md           # expect :90

# Hunk #2 — DEVICE_DISCOVERY.md §2.4
grep -nE 'Handshake . cache warm-feed scope' spec/DEVICE_DISCOVERY.md  # expect :155  (. eats the → arrow)
grep -nE 'correct only when a single Tier-1 board' spec/DEVICE_DISCOVERY.md  # expect :159

# Hunk #3 — DEVICE_DISCOVERY.md §3
grep -nE 'capable-keyed' spec/DEVICE_DISCOVERY.md                     # expect :189
grep -nE 'PresenceTracker' spec/DEVICE_DISCOVERY.md                   # expect :97 and :189
grep -nE 'only when the path set changes' spec/DEVICE_DISCOVERY.md    # expect :191

# Hunk #4 — LINUX.md §6.2
grep -nE 'Trayless \(.--no-default-features.\) build caveat' spec/LINUX.md  # expect :211
grep -nE 'BindsTo=dev-qmkonnect_device.device' spec/LINUX.md          # expect :179, :199, :216
grep -nE 'Restart=always' spec/LINUX.md                               # expect :186, :201, :217

# Hunk #5 — LINUX.md §7.1  (USE THE REGEX-TOLERANT FORM — see §3)
grep -nE 'PresenceTracker.+tick' spec/LINUX.md                        # expect :236
grep -nE 't\.device_status' spec/LINUX.md                             # expect :240

# Hunk #6 — HOST_RULES.md §13 R6
grep -nE 'R6 .+ Legacy handshake side effect' spec/HOST_RULES.md      # expect :616
grep -nE 'proto-v1 firmware it can briefly reset the layer' spec/HOST_RULES.md  # expect :626
grep -nE 'see .PresenceTracker.' spec/HOST_RULES.md                   # expect :627
```

(All `expect` line numbers are the verified results from this research session. They
are anchors for the report; if a line shifted by ±a few on a later commit, the gate is
"the phrase is present somewhere in the file", not an exact line match.)

## 6. Files NOT to touch (boundary discipline)

- `spec/DEVICE_DISCOVERY.md`, `spec/LINUX.md`, `spec/HOST_RULES.md` — READ-ONLY (the audit
  subjects). S2 must not edit them.
- Any source file (`src/`) — S2 is a spec audit, not a code change.
- `PRD.md`, `tasks.json`, `prd_snapshot.md` — owned by humans/orchestrator.
- `plan/006_8f4080956ee0/architecture/verification_findings.md` — the prior research doc;
  S2 re-confirms §3 independently but does not edit it.

## 7. Risk inventory (all low)

1. **Naive literal grep for "PresenceTracker tick"** → false FAIL on hunk #5 (the
   backtick-wrapped code span breaks the literal). Mitigated by the regex-tolerant
   `PresenceTracker.+tick` (§3) — called out in the PRP gotchas.
2. **Treating the incidental `device_connected` pseudo-code at LINUX.md:~230 as a hunk
   failure** — it is NOT part of any contracted hunk; hunk #5 passes on its two key
   phrases. Mitigated by the explicit "incidental observation" framing in §4.
3. **Exact-line-number brittleness** — a later commit could shift line numbers. The gate
   is "phrase present in file", not "phrase at exact line N". The report records the
   actual line(s) found.
4. **Editing a spec file to "fix" a perceived gap** — S2 is read-only; any fix is a
   separate doc task. The 6 hunks all PASS, so no fix is wanted anyway.
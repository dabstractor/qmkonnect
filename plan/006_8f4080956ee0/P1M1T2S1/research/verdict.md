# Verdict — P1.M1.T2.S1 (user-docs Mode B sweep)

## VERIFIED: no documentation drift

The v6 capability-keyed-lifecycle delta (shipped to code+spec in `d240b27`; docs
regenerated in `293f565`) is **not contradicted** by any user-facing doc. The docs
are silent on the internal mechanisms (correct — they are implementation details) and
accurate on the user-visible three-state status. No file was modified.

## Evidence

### Level 1(a) — internal-mechanism terms in `README.md` + `docs/*.md` (excl. `llms_full.txt`)

All **0** (the "silent" property — d240b27 is an internal correctness mechanism):

| term | hits |
| --- | ---: |
| `Tier-?1` | 0 |
| `Tier-?2` | 0 |
| `PresenceTracker\|presence` | 0 |
| `classify` | 0 |
| `warm` | 0 |
| `is_device_connected` | 0 |
| `broadcast\|Broadcast` | 0 |

### Level 1(b) — the 5 targeted drift signatures

All **none** (no output):

1. Tier-1-keyed status claim (`tier.?1.?key\|keyed on tier\|tier.?keyed`) — none.
2. two-state / boolean status claim (`two.?state\|boolean status`) — none.
3. broadcast/send/ping to every 0xFF60 interface — none.
4. unqualified single-ping-per-appearance promise — none.
5. status refreshes every poll (`(every\|each) poll\|…`) — none.

### Level 1(c) — `docs/llms_full.txt` (derived artifact confirmation)

Mirrors the clean source docs (all 0): `Tier-?1`, `PresenceTracker`,
`classify_devices`, `warm.feed|warm_feed` → 0 hits each. Regeneration is current.

### Level 2 — three-state status spot-read (the "accurate" property)

The user-facing status matches `spec/DEVICE_DISCOVERY.md` §3 + `spec/UI.md` §4
exactly:

| state | user doc wording | spec semantics | agree? |
| --- | --- | --- | :---: |
| ● Connected | "a qmk_notifier-capable board is present" (`usage.md:118`) | ≥1 capable present | ✅ |
| ⚠ No module | "QMK board found — no qmk_notifier module (flash it)" (`usage.md:119`, `troubleshooting.md:107`) | ≥1 Tier-1, 0 capable | ✅ |
| ○ Disconnected | "No Device Connected — no QMK Raw-HID board detected" (`usage.md:123`) | 0 Tier-1 boards | ✅ |

Confirmed across `docs/usage.md:116-123`, `docs/troubleshooting.md:105-107`,
`docs/installation.md:218-222`, and the README picker markers (`README.md:217-220`:
`qmk_notifier-capable` / `QMK board, no module`). The two handshake mentions
(`configuration.md:230-231` `--list-devices`/`--list-callbacks`,
`qmk-integration.md:154` "capability handshake") are accurate/neutral — no
capable-keyed or warm-feed claim.

### Level 3 — scope invariants

- `git status --short src/ spec/` → **clean** (0). No code/spec touched.
- `git status --short README.md docs/` → **clean** (0). No doc modified (no-drift branch).
- `docs/llms_full.txt` → **not hand-edited** (derived; regeneration is the mechanism).

## Branch taken

**No-drift branch** (the expected, research-confirmed result). Agreement with
`verification_findings.md` §5 ("Mode B: NONE expected") is reproduced independently.
No Mode-B in-place fix was required.

## Conclusion

The delta is verified-complete at the user-docs layer: the published README + docs
site are silent on the d240b27 internals and accurate on the v6 three-state status.
Combined with green S1 (code gates), S2 (spec drift), and S3 (caveats code-backed),
this closes the P1.M1 verification milestone across code / spec / caveats / user-docs.
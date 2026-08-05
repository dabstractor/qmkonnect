# Research Notes — P3.M1.T1.S2: classify_devices() logic + unit tests with MockNotifier

> **Scope:** ADD the Tier-2 per-candidate capability classifier to
> `src/core/notifier.rs` — `pub fn classify_devices(verbose) -> Vec<ClassifiedDevice>`
> + its factored-out mock-testable core + cache invalidation + a best-effort
> cache warm from the handshake path + a small MockNotifier extension to verify
> the per-candidate vid/pid filter narrowing. Unit tests over the MockNotifier
> assert capable/legacy/timeout→DeviceKind, cache hit/miss/TTL, and per-candidate
> ordering. **Consumes** `DeviceKind`/`ClassifiedDevice`/`CLASSIFICATION_CACHE` +
> the 3 cache helpers from P3.M1.T1.S1 (assumed delivered per the parallel-exec
> contract). **Does NOT** build the picker UI (P3.M2), change the write path
> (P2 DEFER), or add CLI flags (P4).

---

## 1. The exact upstream contract (P3.M1.T1.S1 — assumed delivered verbatim)

`src/core/notifier.rs` will contain (S1's additions) a `// ===== Device
classification (P3.M1) =====` banner section with:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceKind {
    Capable { proto_ver: u8, feature_flags: u8, callback_count: u8, board_rules_present: bool },
    NotQmkNotifier,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ClassifiedDevice {
    pub path: String, pub vendor_id: u16, pub product_id: u16,
    pub product_name: Option<String>, pub usage_page: u16, pub usage: u16,
    pub kind: DeviceKind,
}
const CLASSIFICATION_TTL: Duration = Duration::from_secs(5);
static CLASSIFICATION_CACHE: Lazy<Mutex<HashMap<String, (DeviceKind, Instant)>>> = ...; // PRIVATE
pub fn classification_cache_get(path: &str) -> Option<DeviceKind>   // TTL-checked, clones out
pub fn classification_cache_insert(path: &str, kind: DeviceKind)    // stamps Instant::now()
pub fn classification_cache_clear()                                 // drains
```

THIS task ADDS to that same banner section (keeps it contiguous) and to the
`perform_handshake_with` arms + the MockNotifier. It does NOT touch the structs,
the const, the static, or the 3 helpers (S1 owns those; this task only CALLS them).

**CRITICAL — parallel execution (G0):** S1 may not have landed when this task
starts. If S1 is absent, this task's `classify_devices` references
`DeviceKind`/`ClassifiedDevice`/`CLASSIFICATION_CACHE`/the 3 helpers and WILL NOT
compile until S1 merges. That is EXPECTED — the orchestrator lands both before
the build gate. Do NOT redefine S1's items (conflict on merge); add only THIS
task's functions, marked so they sit in S1's banner section once merged.

---

## 2. The crate contract (the probe + the constraint that shapes the design)

**The probe** (DEVICE_DISCOVERY.md §2.2): send `RunCommand::QueryInfo`, match the
`CommandResponse`. A board is **capable** iff the reply is
`CommandResponse::Info { proto_ver: 2, .. }`. Everything else (Legacy, Timeout,
Ack, CallbackName, Info{proto_ver!=2}, Err) ⇒ `NotQmkNotifier`. **No board is
harmed** — the 0x81 0x9F magic makes qmk_notifier coexist with VIA/Vial (VIA's
`raw_hid_receive` silently ignores magic-prefixed input).

**The crate's CommandResponse variants** (rev f26893e, lib.rs:86-115):
```rust
pub enum CommandResponse {
    Legacy { matched: bool },
    Info { proto_ver: u8, feature_flags: u8, callback_count: u8, board_rules_present: bool },
    CallbackName { index: u8, name: Option<String> },
    Ack { ok: bool },
    Timeout,
}
```

**THE CRITICAL CONSTRAINT (external_deps.md):** the crate has **NO per-path send
and NO per-device send.** `MatchKey` (vid/pid/page/usage) is PRIVATE
(core.rs:641); `open_matching_devices` opens ALL filter-matching devices;
`run()`/`send_raw_report()` broadcast to all matches. So `send_command(QueryInfo,
&filter)` reaches EVERY device matching `filter`.

**This shapes the per-candidate mechanism (item RESEARCH NOTE point 1):** to ping
ONE candidate at a time, the app **narrows the DeviceFilter to the candidate's
vid/pid** (`Some(d.vendor_id()), Some(d.product_id())`) so the crate opens only
devices with that exact vid/pid. **LIMITATION:** this is only a true single-device
ping when vid/pid is unique on the bus. If two boards share vid/pid (e.g. a split
pair, or two 0xFEED:0x0000 boards), the narrowed filter STILL matches both, so both
get pinged and both reply — the app cannot attribute the reply to a specific path.
This is the documented v1 limitation (DEVICE_DISCOVERY.md §4.3). The implementer
MUST record it in a comment at the narrowing site. (The handshake has the same
limitation; both are bounded by the same assumption.)

**Spec §2.2 match arm does NOT gate on `feature_flags & 0x01`** (unlike the
handshake at notifier.rs:444, which additionally requires the APPLY_HOST_CONTEXT
bit for the host-rules send). classify_devices classifies `Info{proto_ver:2}` →
`Capable` REGARDLESS of the feature bit, and RECORDS feature_flags in the struct so
the consumer can check it. The item description confirms: "Info{proto_ver:2} ->
Capable{...}". Do NOT add the feature-bit gate to the classifier (it would diverge
from §2.2 and hide capable-but-no-host-rules boards from the picker).

---

## 3. hidapi enumeration — the exact API surface (mirrors is_device_connected @216)

**DeviceInfo methods** (hidapi 2.6.3, confirmed `src/lib.rs:358`):
- `d.path() -> &CStr` — the **stable per-interface identity + cache key**. Convert
  to String via `d.path().to_string_lossy().to_string()`. (CStr, NOT OsStr —
  `to_string_lossy()` gives `Cow<str>`; `.to_string()` is equivalent to
  `.into_owned()`.)
- `d.vendor_id() -> u16`, `d.product_id() -> u16`.
- `d.usage_page() -> u16`, `d.usage() -> u16`.
- `d.product_string() -> Option<&str>` — the product name (already used by
  `list_devices` @139). → `.map(|s| s.to_string())` for `ClassifiedDevice.product_name`.

**The Tier-1 enumerate + filter** — VERBATIM mirror of `is_device_connected` @216-232,
but `.filter()` (collect) instead of `.any()` (exists):
```rust
let f = configured_filter();
let api = hidapi::HidApi::new()?;          // or match Ok/Err like is_device_connected
let candidates: Vec<Candidate> = api.device_list()
    .filter(|d| {
        d.usage_page() == f.usage_page
            && d.usage() == f.usage
            && f.vendor_id.is_none_or(|v| d.vendor_id() == v)
            && f.product_id.is_none_or(|p| d.product_id() == p)
    })
    .map(|d| Candidate {
        path: d.path().to_string_lossy().to_string(),
        vendor_id: d.vendor_id(),
        product_id: d.product_id(),
        product_name: d.product_string().map(|s| s.to_string()),
        usage_page: d.usage_page(),
        usage: d.usage(),
    })
    .collect();
```
`configured_filter()` (@83) returns `DeviceFilter { vendor_id: Option<u16>,
product_id: Option<u16>, usage_page: u16 (default 0xFF60), usage: u16 (default
0x61) }` — the optional vid/pid narrowers come from the user's config. THIS
filter is the Tier-1 set; the per-candidate probe FURTHER narrows to each
candidate's specific vid/pid (§4 below).

**HidApi::new() is read-only enumeration** — never opens, never sends (R-COEX
safe; same as is_device_connected which the poll threads call every tick). Safe to
call inside classify_devices AND inside the handshake cross-feed.

---

## 4. The narrowed-filter per-candidate ping (the chosen mechanism)

For each `Candidate`, build a **narrowed** DeviceFilter pinned to the candidate's
vid/pid, then `send_command(QueryInfo, &narrowed)` via the global notifier:

```rust
let narrowed = DeviceFilter {
    vendor_id: Some(c.vendor_id),
    product_id: Some(c.product_id),
    usage_page: c.usage_page,   // == configured (0xFF60) — kept for parity
    usage: c.usage,             // == configured (0x61)
};
let notifier = get_notifier();
let n = notifier.lock().unwrap();   // poison: .unwrap() matches perform_handshake @435
let resp = n.send_command(qmk_notifier::RunCommand::QueryInfo, &narrowed);
let kind = classify_reply(resp);
classification_cache_insert(&c.path, kind.clone());
```

**Why narrow to vid/pid (not path):** the crate has no per-path send (§2). vid/pid
is the only app-side knob. Documented limitation (§2): if two interfaces share
vid/pid, the narrowed filter still matches both. Comment REQUIRED at this site.

**Lock discipline:** acquire `get_notifier().lock().unwrap()` per-candidate (short
scope), mirroring how the callback sweep re-acquires NOTIFIER per iteration
(notifier.rs:446-455 comment: "re-acquires NOTIFIER per iteration so a
window-notification send can acquire it between any two QueryCallback iterations").
Per-candidate acquisition lets a concurrent `notify_qmk`/debounce-flush interleave
between candidates. Do NOT hold one lock across all candidates.

---

## 5. The reply→DeviceKind classifier (pure, the core test target)

```rust
/// Classify a QUERY_INFO reply into a [`DeviceKind`] (DEVICE_DISCOVERY.md §2.2).
/// `Info { proto_ver: 2, .. }` ⇒ Capable (records all 4 fields); everything else
/// (Legacy / Timeout / Ack / CallbackName / Info{proto_ver!=2} / Err) ⇒ NotQmkNotifier.
fn classify_reply(
    resp: Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>>,
) -> DeviceKind {
    match resp {
        Ok(qmk_notifier::CommandResponse::Info {
            proto_ver: 2, feature_flags, callback_count, board_rules_present,
        }) => DeviceKind::Capable { proto_ver: 2, feature_flags, callback_count, board_rules_present },
        _ => DeviceKind::NotQmkNotifier,
    }
}
```
**Note:** the match arm binds `proto_ver: 2` as a literal pattern — an
`Info{proto_ver: 1, ..}` reply does NOT match this arm and falls to `_` ⇒
NotQmkNotifier (a proto-v1 board is "replied but not the typed-command protocol we
need"). This is EXACTLY §2.2's match. The `feature_flags & 0x01` gate is the
handshake's concern (for the host-rules SEND), NOT the classifier's — the picker
shows the board as Capable and the consumer can read `feature_flags`.

---

## 6. The factored core (REQUIRED for the item's MockNotifier test mandate)

`classify_devices` enumerates real HID (`HidApi::new()`) — **uncontrollable in a
unit test** (CI/dev box may have 0 QMK boards, or N unpredictable ones). The item
OUTPUT mandates "unit tests over the MockNotifier (capable/legacy/timeout →
DeviceKind; cache hit/miss/TTL)". The ONLY way to make those tests deterministic
is to **factor the per-candidate core out of the hidapi shell**:

```rust
/// Internal Tier-1 descriptor (what enumerate produces per interface).
/// Factored out so [`classify_candidates`] is testable without a real HID bus.
struct Candidate {
    path: String,
    vendor_id: u16,
    product_id: u16,
    product_name: Option<String>,
    usage_page: u16,
    usage: u16,
}

/// The pure, MockNotifier-testable core: classify N Tier-1 candidates by pinging
/// each (cache-aware). Used by [`classify_devices`]; tested directly with
/// hand-built `Vec<Candidate>` + queued mock responses.
fn classify_candidates(candidates: Vec<Candidate>, verbose: bool) -> Vec<ClassifiedDevice> {
    let notifier = get_notifier();
    candidates.into_iter().map(|c| {
        let kind = match classification_cache_get(&c.path) {
            Some(k) => { if verbose { eprintln!("[{}ms] classify: cache hit {}", now_ms(), c.path); } k }
            None => {
                let narrowed = DeviceFilter { /* Some(c.vendor_id), Some(c.product_id), c.usage_page, c.usage */ };
                let resp = notifier.lock().unwrap().send_command(qmk_notifier::RunCommand::QueryInfo, &narrowed);
                let kind = classify_reply(resp);
                classification_cache_insert(&c.path, kind.clone());
                kind
            }
        };
        ClassifiedDevice { path: c.path, vendor_id: c.vendor_id, product_id: c.product_id,
                           product_name: c.product_name, usage_page: c.usage_page, usage: c.usage, kind }
    }).collect()
}

pub fn classify_devices(verbose: bool) -> Vec<ClassifiedDevice> {
    let candidates = enumerate_candidates();                 // hidapi shell (§3)
    invalidate_absent_cache_entries(&candidates);            // §7 — drop disappeared paths
    classify_candidates(candidates, verbose)
}
```

**Why this factoring is correct (not a workaround):** it is the standard "pure core
+ effectful shell" split. The SHELL (`enumerate_candidates` + `classify_devices`)
touches hidapi and is exercised by the Level-3 full-crate build + a light smoke
test (returns a Vec without panic; may be empty on a box with no QMK board). The
CORE (`classify_candidates` + `classify_reply`) is pure w.r.t. the candidate list
and is fully driven by the MockNotifier — exactly the test surface the item names.
The cache is a real global static (cleared per-test via
`classification_cache_clear()`), so cache hit/miss/TTL tests are faithful.

**Enumerate shell signature:**
```rust
fn enumerate_candidates() -> Vec<Candidate>   // HidApi::new + configured_filter .filter/.map/.collect; Err ⇒ vec![]
```

---

## 7. Cache invalidation on disappearance + warm-from-handshake (item point 3)

**Invalidation (path no longer present):** after enumerate, drop cache entries
whose path is not in the candidate set. Factored as a pure, testable helper:
```rust
/// Drop cache entries whose path is not in `present` (device disappearance).
fn invalidate_absent_cache_entries(candidates: &[Candidate]) {
    let present: Vec<&str> = candidates.iter().map(|c| c.path.as_str()).collect();
    if let Ok(mut map) = CLASSIFICATION_CACHE.lock() {
        map.retain(|path, _| present.contains(&path.as_str()));
    }
}
```
(`HashMap::retain` is in std; `Vec::contains` is O(n) but n = board count ≈ 1-2.)
Testable directly: insert 2 fake entries, call with a candidate list containing
only path1, assert path2 evicted. (Reach into `CLASSIFICATION_CACHE` from the
same-module test, like S1's TTL test.)

**Warm from handshake (item: "Also feed the cache from the existing
perform_handshake path so the status path stays single-ping-per-appearance"):**
`perform_handshake_with` (@421) ALREADY pings (QueryInfo) once per boot. If
`classify_devices` ALSO pinged, that's 2 pings per appearance. To keep it to 1,
the handshake stamps its result into the per-path cache so `classify_devices`
reads a warm cache (TTL hit ⇒ no re-ping). Since the handshake is filter-keyed
(no single path), it enumerates the present Tier-1 paths and stamps them all —
correct under the single-vid/pid-on-bus assumption (the same limitation as §2).
```rust
/// Warm the per-path cache from the handshake result (best-effort). Enumerates
/// present Tier-1 paths and stamps each with `kind`. Bounded by the single-
/// vid/pid-on-bus assumption (DEVICE_DISCOVERY.md §4.3): if multiple interfaces
/// share vid/pid they all get the handshake's (single) result — acceptable
/// because they are the same board model in the common case.
fn warm_cache_from_handshake(kind: DeviceKind) {
    for c in enumerate_candidates() {
        classification_cache_insert(&c.path, kind.clone());
    }
}
```
**Called from `perform_handshake_with`'s 4 arms** (notifier.rs:444/576/588/600):
- Capable arm (@444, after `BOARD_HAS_RULES`/`HOST_CAPABLE` set): `warm_cache_from_handshake(DeviceKind::Capable { proto_ver: 2, feature_flags, callback_count, board_rules_present });`
- Timeout arm (@576): `warm_cache_from_handshake(DeviceKind::NotQmkNotifier);`
- `Ok(other)` arm (@588): `warm_cache_from_handshake(DeviceKind::NotQmkNotifier);`
- `Err(e)` arm (@600): `warm_cache_from_handshake(DeviceKind::NotQmkNotifier);`

**Safe in tests:** the handshake tests use MockNotifier (no real HID), so
`warm_cache_from_handshake`'s `enumerate_candidates()` finds 0 Tier-1 devices ⇒
stamps nothing ⇒ existing handshake assertions (on HOST_CAPABLE, callback_names,
send_command calls) are UNAFFECTED (the cache is a separate static). The cross-feed
is a no-op in the test env and a real warm in production. R-COEX-safe (pure
enumeration, no open/send).

---

## 8. MockNotifier extension (small, additive — item point 4 explicitly permits it)

The item: "The mock infra may need a small extension to return per-device
responses — extend MOCK_RESPONSES to be FIFO-queued per send_command call (already
is)." The FIFO queue (`MOCK_RESPONSES` @1418, popped front-first @1503) ALREADY
gives per-candidate ordering: candidate 0 consumes response[0], candidate 1
consumes response[1], etc. So **no change needed for per-device responses** — the
existing FIFO suffices.

**The one extension NEEDED:** verify the per-candidate **filter narrowing** (the
chosen mechanism, §4). The current MockNotifier.send_command (@1497) takes
`_filter: &DeviceFilter` (IGNORED) — it cannot assert the narrowed filter. Add a
static that records each call's filter so a test can assert
"candidate i's send_command was called with vid/pid == candidate i's vid/pid":

```rust
// NEW static (alongside MOCK_SEND_COMMAND_CALLS @1416):
static MOCK_SEND_COMMAND_FILTERS: Lazy<StdMutex<Vec<(Option<u16>, Option<u16>, u16, u16)>>> =
    Lazy::new(|| StdMutex::new(Vec::new()));
```
- In `reset_global_mock()` (@1428): add `MOCK_SEND_COMMAND_FILTERS.lock().unwrap().clear();`
- In `MockNotifier::send_command` (@1497): change `_filter` → `filter` and push
  `MOCK_SEND_COMMAND_FILTERS.lock().unwrap().push((filter.vendor_id, filter.product_id, filter.usage_page, filter.usage));`
- Add `fn get_send_command_filters() -> Vec<(Option<u16>, Option<u16>, u16, u16)>`
  { MOCK_SEND_COMMAND_FILTERS.lock().unwrap().clone() } to `impl MockNotifier`.

**Why record tuples (not DeviceFilter):** `DeviceFilter` (@67) has NO derives.
Recording `(vid, pid, page, usage)` tuples avoids adding `Clone`/`PartialEq` to a
production struct purely for test convenience — zero production-code change. The
tuple mirrors DeviceFilter's 4 fields exactly, so the assertion
`(Some(0x1234), Some(0x5678), 0xFF60, 0x61)` proves the narrowing.

**No other mock change.** `MOCK_RESPONSES` (FIFO), `MOCK_SEND_COMMAND_ERRORS`,
`MOCK_SEND_DELAY`, `MOCK_SEND_COMMAND_CALLS` are all reused as-is.

---

## 9. Test plan (~10 tests, all over the MockNotifier, single-threaded)

All in the existing `#[cfg(test)] mod tests` (or a new `mod classify_tests`).
Prefix `test_classify_` (disjoint from S1's `test_classification_cache_*` /
`test_devicekind_*`, the `r_coex_*`, and the 49 existing `test_*`). Each
cache-touching test STARTS with `classification_cache_clear()` (the static outlives
tests — S1's G4). **Single-threaded:** `--test-threads=1` (AGENTS.md).

**Setup pattern (mirror test_send_command_reset_clears_log @1940):**
```rust
reset_test_state();
reset_handshake_state();
set_notifier(Box::new(MockNotifier::new()));
classification_cache_clear();
```

### A. classify_reply (pure, 6 tests — no mock needed)
1. `test_classify_reply_info_proto2_capable` — Ok(Info{proto_ver:2, flags:0x01, cb:3, rules:true}) ⇒ Capable{proto_ver:2, flags:0x01, cb:3, rules:true}.
2. `test_classify_reply_info_proto1_notqmk` — Ok(Info{proto_ver:1, ..}) ⇒ NotQmkNotifier (literal arm doesn't match).
3. `test_classify_reply_legacy_notqmk` — Ok(Legacy{matched:true}) ⇒ NotQmkNotifier.
4. `test_classify_reply_timeout_notqmk` — Ok(Timeout) ⇒ NotQmkNotifier.
5. `test_classify_reply_ack_notqmk` — Ok(Ack{ok:true}) ⇒ NotQmkNotifier (the empty-queue default).
6. `test_classify_reply_err_notqmk` — Err("device error".into()) ⇒ NotQmkNotifier.

### B. classify_candidates (MockNotifier, 4 tests — the core)
7. `test_classify_candidates_capable` — 1 candidate, queue [Info{proto_ver:2,...}] ⇒
   result[0].kind == Capable{...}; assert send_command called once; assert filter
   narrowed to candidate's vid/pid via get_send_command_filters().
8. `test_classify_candidates_mixed` — 3 candidates, queue [Info{p:2}, Legacy, Timeout] ⇒
   [Capable, NotQmk, NotQmk]; assert 3 send_command calls, filters narrowed per-candidate.
9. `test_classify_candidates_cache_hit_skips_ping` — pre-insert candidate's path into
   cache (classification_cache_insert), then classify_candidates([candidate]) with an
   EMPTY response queue ⇒ result kind == cached; assert send_command NOT called
   (get_send_command_calls().is_empty()).
10. `test_classify_candidates_cache_miss_pings_and_caches` — empty cache, queue [Info{p:2}] ⇒
    Capable; THEN second classify_candidates([candidate]) with empty queue ⇒ STILL Capable
    (cache hit, no second ping — assert call count unchanged).
11. `test_classify_candidates_ttl_re_ping` — insert candidate with a PAST Instant (rewrite
    via CLASSIFICATION_CACHE.lock()...insert(path, (kind, now-TTL-1ms))), then
    classify_candidates([candidate]) queue [Timeout] ⇒ NotQmkNotifier (re-pinged, new result
    cached). Assert send_command called once (TTL expired ⇒ miss).

### C. invalidate_absent_cache_entries (pure, 1 test)
12. `test_invalidate_drops_absent_paths` — insert 2 fake entries (p1, p2); call
    invalidate_absent_cache_entries(&[Candidate{path:"p1",..}]); assert p1 retained, p2 gone
    (reach into CLASSIFICATION_CACHE from the same-module test).

### D. classify_devices smoke (light — the untestable shell, 1 test)
13. `test_classify_devices_smoke_returns_vec` — just `assert!(classify_devices(false).len() <=
    usize::MAX)` i.e. it returns without panic. On a box with no QMK board it's `vec![]`; on
    a box with one it's 1 element. Do NOT assert a specific count (env-dependent). This
    proves the enumerate+delegate wiring compiles+runs.

**Why no filter-assertion test for warm_cache_from_handshake:** the handshake tests
use MockNotifier (no HID), so warm_cache_from_handshake stamps nothing (0 Tier-1
paths). The warm logic (`for c in enumerate: insert`) is trivially correct by
inspection + the existing handshake tests prove no regression (the cache is a
separate static). The invalidate + classify_candidates tests cover the cache
mechanics.

---

## 10. Gotchas (pinned to concrete failure modes)

- **G0 — runs in PARALLEL with S1:** DeviceKind/ClassifiedDevice/CLASSIFICATION_CACHE/
  the 3 helpers may not exist yet. This task references them; it won't compile until S1
  merges (expected). Do NOT redefine them (merge conflict). Add only THIS task's items,
  placed to sit in S1's banner section once merged.
- **G1 — factor the core out (REQUIRED, not optional):** `classify_devices` enumerates real
  HID → uncontrollable in tests. The item's MockNotifier test mandate is ONLY satisfiable
  by factoring `classify_candidates` (+ `classify_reply`) out of the hidapi shell. Do NOT
  inline all logic into classify_devices and then hand-wave testing — the tests MUST drive
  a factored, candidate-list-taking core.
- **G2 — classify on proto_ver==2, NOT the feature bit.** §2.2: capable iff Info{proto_ver:2}.
  The handshake (@444) ADDS `feature_flags & 0x01` for the host-rules SEND; the classifier
  does NOT (it records feature_flags for the consumer). Adding the gate would hide
  capable-but-no-host-rules boards from the picker — diverges from §2.2.
- **G3 — `d.path()` returns `&CStr`, not `&OsStr`/`&str`.** Convert via
  `d.path().to_string_lossy().to_string()`. Using `.to_str().unwrap()` panics on non-UTF8
  paths (rare but possible on Windows); to_string_lossy is the safe choice.
- **G4 — narrow filter to vid/pid (the ONLY per-candidate mechanism).** The crate has no
  per-path send (external_deps.md). vid/pid narrowing is the sole app-side knob. Document
  the multi-same-vid/pid limitation (§4.3) in a comment AT the narrowing site.
- **G5 — per-candidate lock acquisition (short scope).** Acquire `get_notifier().lock().unwrap()`
  INSIDE the per-candidate loop (one lock per candidate), NOT once across all. Mirrors the
  callback sweep's per-iteration re-acquire (notifier.rs:446-455) so a concurrent
  notify_qmk/debounce-flush can interleave. Do NOT hold one lock across all candidates.
- **G6 — `.lock().unwrap()` for poison (match the codebase).** The item writes
  `get_notifier().lock().send_command(...)` but `lock()` returns `LockResult` (won't compile
  without unwrap). Use `.lock().unwrap()` like perform_handshake @435. (The cache helpers from
  S1 use the non-panicking `.lock().ok()?`/`if let Ok` form for get/insert/clear — that's S1's
  choice; THIS task's notifier-lock follows the handshake's `.unwrap()` since a poisoned
  NOTIFIER is a hard failure, not a cache miss.)
- **G7 — single-threaded tests crate-wide.** `cargo test --bin qmkonnect -- --test-threads=1`
  (shared MockNotifier globals + DebounceState, AGENTS.md). Each cache-touching test STARTS
  with `classification_cache_clear()`.
- **G8 — MockNotifier extension is additive + minimal.** Add ONE static
  (MOCK_SEND_COMMAND_FILTERS) + clear it in reset_global_mock + change `_filter`→`filter` in
  send_command + add ONE getter. Do NOT touch MOCK_RESPONSES (FIFO already per-device-ordered),
  MOCK_SEND_COMMAND_ERRORS, MOCK_SEND_DELAY, or the default `Ack{ok:true}` fallback. Record
  tuples `(vid,pid,page,usage)` NOT DeviceFilter (avoids adding derives to a production struct).
- **G9 — the handshake cross-feed is a no-op in tests (safe).** warm_cache_from_handshake's
  enumerate_candidates() finds 0 Tier-1 devices under MockNotifier ⇒ stamps nothing ⇒ the 30+
  existing handshake tests (asserting HOST_CAPABLE/callback_names/send_command calls) are
  UNAFFECTED. Do NOT add assertions about the cache in the existing handshake tests.
- **G10 — binary-only crate; doctests don't run under `--bin`.** Mode-A doc-comments are prose
  citing DEVICE_DISCOVERY.md §2 (+ record the filter-narrow mechanism + §4.3 limitation).
  Use ` ```rust,ignore ``` ` for any code sketch; do NOT add bare ` ``` ` runnable doctests.
- **G11 — verbose logging convention.** `eprintln!("[{}ms] classify: ...", crate::core::now_ms(), ...)`
  when verbose (matches perform_handshake/notify_qmk). Keep it terse. The transport stays
  quiet (send_command verbose=false, as everywhere).
- **G12 — imports.** `HashSet` is NOT currently imported (notifier.rs:3 is `use std::collections::{BTreeSet, HashMap};`).
  Use `Vec::contains` in invalidate (no new import) OR add `HashSet` to that use line (one-token
  edit). `hidapi`, `qmk_notifier`, `Duration`/`Instant`/`Lazy`/`Mutex`/`HashMap` are all in scope.
  `now_ms` is `crate::core::now_ms()`.
- **G13 — do NOT touch the write path / device_status / picker.** Write-narrowing is DEFER
  (P2.M1.T1.S2 decision record). device_status() (P1) is Complete. The picker is P3.M2. THIS
  task ships ONLY classify_devices + cache mechanics + handshake cross-feed + mock extension.
- **G14 — do NOT change the handshake's HOST_CAPABLE/callback/HAS_HANDSHAKED logic.** The
  cross-feed ADDS one warm_cache_from_handshake(kind) call per arm (after the existing
  set/clear). It does NOT alter the dedup, the SET_OS, the callback sweep, or the ordering
  invariant (BOARD_HAS_RULES before HOST_CAPABLE).

---

## 11. Downstream consumer contracts (do NOT implement — just satisfy)

- **P3.M2.T1 (the picker, Win32/macOS/Linux):** calls `classify_devices(verbose)` → renders
  `Vec<ClassifiedDevice>` (the `kind` column: Capable ⇒ "qmk_notifier ✓", NotQmkNotifier ⇒
  "QMK board, no module"). "Rescan" in the picker calls `classification_cache_clear()` then
  `classify_devices()` again.
- **P1 device_status() (Complete):** conceptually a fold over per-device kinds; THIS task's
  Vec<ClassifiedDevice> is the per-device view. No change to device_status this task.
- **P4.M1.T1.S1 (--list-devices kind column):** calls a one-shot `classify_devices()` to
  annotate the `--list-devices` output with the `kind` column.
- **The poll threads (tray.rs/linux_tray.rs):** TODAY call is_device_connected +
  perform_handshake on a false→true transition. With the cross-feed (§7), the handshake warms
  the cache, so a subsequent classify_devices (if the poll thread or picker calls it) reads the
  cache ⇒ single-ping-per-appearance. (Wiring classify_devices into the poll thread's
  transition is a LATER task — this task ships the function + the cache, not the call site.)

---

## 12. Scope boundary (do NOT do)

- ❌ The picker UI (P3.M2 — three platforms). This task ships the DATA source only.
- ❌ Write-narrowing (P2.M1.T1.S2 DEFER — needs a crate change). classify_devices feeds the
      picker/status, not the write path.
- ❌ CLI flags (--list-devices kind column is P4.M1.T1.S1).
- ❌ Redefine S1's structs/cache/helpers (G0 — merge conflict). Only CALL them.
- ❌ Change the handshake's core logic (HOST_CAPABLE/callback/HAS_HANDSHAKED/dedup/SET_OS/
      sweep). Only ADD the warm_cache_from_handshake call per arm (G14).
- ❌ Add the feature_flags & 0x01 gate to the classifier (G2 — diverges from §2.2).
- ❌ Hold one notifier lock across all candidates (G5 — starves notify_qmk).
- ❌ Touch Cargo.toml (hidapi/qmk-notifier/once_cell all deps), the crate, docs/*.md (P4),
      PRD.md / tasks.json / prd_snapshot.md.
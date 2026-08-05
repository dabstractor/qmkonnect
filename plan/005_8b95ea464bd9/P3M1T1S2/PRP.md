# PRP — P3.M1.T1.S2: classify_devices() logic + unit tests with MockNotifier

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This task adds the **Tier-2 per-candidate
> capability classifier** to `src/core/notifier.rs`: `pub fn classify_devices(verbose)
> -> Vec<ClassifiedDevice>` + its **factored-out MockNotifier-testable core**
> (`classify_candidates`, `classify_reply`) + cache invalidation on disappearance +
> a best-effort cache warm from the `perform_handshake` path (single-ping-per-
> appearance) + a small additive MockNotifier extension to verify per-candidate
> vid/pid filter narrowing. Source of truth: **`spec/DEVICE_DISCOVERY.md` §2** (the
> Capability Probe). **Consumes** `DeviceKind`/`ClassifiedDevice`/
> `CLASSIFICATION_CACHE` + the 3 cache helpers from P3.M1.T1.S1 (parallel; assumed
> delivered). **Consumed downstream by** P3.M2.T1 (the picker), P1's
> `device_status()` (per-device complement), P4.M1.T1.S1 (`--list-devices` kind
> column). It does **not** touch the write path (P2 DEFER), the picker UI (P3.M2),
> or CLI flags (P4).

> **PARALLEL-EXECUTION NOTE:** this item runs in parallel with P3.M1.T1.S1 (which
> ships `DeviceKind`/`ClassifiedDevice`/`CLASSIFICATION_CACHE`/the 3 cache helpers).
> This task references those items; it will not compile until S1 merges (expected —
> the orchestrator lands both before the build gate). Do NOT redefine S1's items
> (merge conflict). See Task 1 / G0.

---

## Goal

**Feature Goal**: Add `pub fn classify_devices(verbose: bool) -> Vec<ClassifiedDevice>`
that enumerates Tier-1 HID candidates (`HidApi::new()` + `configured_filter()`
narrowers: `usage_page==0xFF60 && usage==0x61` + optional vid/pid), and for each
candidate: consults `CLASSIFICATION_CACHE` (TTL hit ⇒ reuse), else narrows a
`DeviceFilter` to the candidate's vid/pid and pings `RunCommand::QueryInfo` via
`get_notifier().lock().send_command(...)`, classifies the reply into `DeviceKind`
(`Info{proto_ver:2}` ⇒ `Capable{..}`; everything else ⇒ `NotQmkNotifier`), caches
it, and returns the `Vec<ClassifiedDevice>`. Stale entries (disappeared paths) are
evicted each pass. The handshake path (`perform_handshake_with`) warm-feeds the
same cache so the status path stays single-ping-per-appearance. The per-candidate
classification core is **factored out** (`classify_candidates`) so it is
MockNotifier-testable without real HID hardware.

**Deliverable** (additions to `src/core/notifier.rs`):
1. **`fn classify_reply(resp: Result<CommandResponse, Box<dyn Error + Send + Sync>>) -> DeviceKind`** — pure §2.2 matcher (`Info{proto_ver:2,..}`⇒Capable carrying all 4 fields; `_`⇒NotQmkNotifier).
2. **`struct Candidate`** (private) — the Tier-1 descriptor (`path, vendor_id, product_id, product_name, usage_page, usage`), factored out for testability.
3. **`fn classify_candidates(candidates: Vec<Candidate>, verbose: bool) -> Vec<ClassifiedDevice>`** — the pure, MockNotifier-testable core (cache-aware; per-candidate narrowed-filter ping).
4. **`fn enumerate_candidates() -> Vec<Candidate>`** — the hidapi shell (`HidApi::new()` + `configured_filter()` `.filter`/`.map`/`.collect`; `Err`⇒`vec![]`).
5. **`pub fn classify_devices(verbose: bool) -> Vec<ClassifiedDevice>`** — `enumerate_candidates()` → `invalidate_absent_cache_entries()` → `classify_candidates()`. Mode-A rustdoc citing `spec/DEVICE_DISCOVERY.md` §2 + recording the filter-narrow-by-vid/pid mechanism + its §4.3 limitation.
6. **`fn invalidate_absent_cache_entries(candidates: &[Candidate])`** — `CLASSIFICATION_CACHE.retain` keeping only present paths (pure, testable).
7. **`fn warm_cache_from_handshake(kind: DeviceKind)`** — enumerate present Tier-1 paths + `classification_cache_insert` each (best-effort; single-vid/pid assumption documented).
8. **4 surgical calls** in `perform_handshake_with`'s arms (Capable@~444 / Timeout@~576 / Ok(other)@~588 / Err@~600): `warm_cache_from_handshake(kind)` after the existing HOST_CAPABLE set/clear.
9. **MockNotifier extension**: `static MOCK_SEND_COMMAND_FILTERS` + clear in `reset_global_mock` + record `(vid,pid,page,usage)` tuples in `send_command` (`_filter`→`filter`) + `MockNotifier::get_send_command_filters()` getter.
10. **~13 unit tests** over the MockNotifier (6 classify_reply + 5 classify_candidates incl. cache hit/miss/TTL + 1 invalidate + 1 classify_devices smoke).

**Success Definition**:
- `classify_reply(Ok(Info{proto_ver:2,..}))` ⇒ `Capable{..}` (all 4 fields); `Ok(Legacy|Timeout|Ack|CallbackName|Info{proto_ver:1})` and `Err(_)` ⇒ `NotQmkNotifier`.
- `classify_candidates([c], false)` with a queued `Info{proto_ver:2}` ⇒ `result[0].kind == Capable{..}`; asserts exactly 1 `send_command` call AND the filter was narrowed to `(Some(c.vendor_id), Some(c.product_id), c.usage_page, c.usage)`.
- Cache hit: pre-inserting `c.path` ⇒ `classify_candidates` makes **0** `send_command` calls (queue stays untouched) and returns the cached kind.
- Cache miss then re-call: second `classify_candidates([c])` with an empty queue returns the cached kind with **no additional** `send_command` call.
- TTL expiry: a cache entry stamped `now - TTL - 1ms` ⇒ re-ping (1 `send_command` call), new result cached.
- `invalidate_absent_cache_entries(&[Candidate{path:"p1",..}])` evicts a pre-inserted `"p2"` and keeps `"p1"`.
- `classify_devices(false)` returns a `Vec` (possibly empty) without panic.
- `cargo build --bin qmkonnect` clean; `cargo test --bin qmkonnect -- --test-threads=1` green (~13 new + all existing incl. the 30+ handshake tests — UNAFFECTED, the cross-feed is a no-op under MockNotifier); `git status` = `src/core/notifier.rs` only.

## User Persona (if applicable)

**Target User**: three downstream consumers (none fully wired this task):
- **P3.M2.T1** (the discovered-device picker, Win32/macOS/Linux): renders `Vec<ClassifiedDevice>` as the Settings rows + a "kind" column.
- **P1's `device_status()` resolver** (Complete): the aggregate three-state status is conceptually a fold over the per-device `DeviceKind`s; this task ships the per-device vector.
- **P4.M1.T1.S1** (`--list-devices`): annotates the device list with a `kind` column via a one-shot `classify_devices()`.

**Use Case**: a user opens Settings → the picker calls `classify_devices(verbose)` → gets `Vec<ClassifiedDevice>` → renders each row with VID/PID/product + "qmk_notifier ✓" (Capable) or "QMK board, no module" (NotQmkNotifier). "Rescan" clears the cache + re-classifies. Because the handshake warm-feeds the cache, opening Settings right after connect does NOT re-ping (TTL hit).

**Pain Points Addressed**: today the app knows only *aggregate* presence (`is_device_connected`) + *aggregate* capability (`host_capable`). It cannot tell the user *which* board is capable, nor distinguish a pure-VIA board from a qmk_notifier board in a list. `classify_devices` is the per-device capability resolver the picker needs (DEVICE_DISCOVERY.md §2).

## Why

- **DEVICE_DISCOVERY.md §2.3 mandates `classify_devices(verbose) -> Vec<ClassifiedDevice>`** with this exact algorithm (enumerate Tier-1 → per-candidate QUERY_INFO → cache keyed by path, 5s TTL). §2.2 defines the `Info{proto_ver:2}`⇒Capable match. This task ships that function.
- **§2.4 + the item require single-ping-per-appearance.** The handshake and the probe are the SAME QUERY_INFO tx; to avoid double-pinging on connect, the handshake warm-feeds the cache so the probe reads a TTL hit. This task wires that cross-feed.
- **Unblocks P3.M2 (picker), P4.M1.T1.S1 (--list-devices kind column).** Both call `classify_devices()`.
- **The crate has no per-path send (external_deps.md), so the per-candidate mechanism is filter-narrow-by-vid/pid** — the only app-side knob. This task implements + documents that mechanism (with its §4.3 limitation) and tests it via the filter-recording mock extension.

## What

Additive edits to `src/core/notifier.rs` (a new contiguous sub-section inside S1's `// ===== Device classification (P3.M1) =====` banner, plus 4 one-line calls in `perform_handshake_with`'s arms, plus the MockNotifier filter-recording extension). No new Cargo deps; `hidapi`/`qmk_notifier`/`once_cell` already present. No CLI/tray/write-path changes.

### Success Criteria
- [ ] **`fn classify_reply(resp) -> DeviceKind`**: `Ok(Info{proto_ver:2,feature_flags,callback_count,board_rules_present})` ⇒ `Capable{proto_ver:2,..}` (all 4 fields); every other `Ok(_)` and every `Err(_)` ⇒ `NotQmkNotifier`. Does NOT gate on `feature_flags & 0x01` (G2).
- [ ] **`struct Candidate { path, vendor_id, product_id, product_name, usage_page, usage }`** (private; fields per `ClassifiedDevice` minus `kind`).
- [ ] **`fn classify_candidates(candidates: Vec<Candidate>, verbose: bool) -> Vec<ClassifiedDevice>`**: per-candidate — cache hit ⇒ reuse (no ping); miss ⇒ narrow `DeviceFilter{vendor_id:Some(c.vendor_id), product_id:Some(c.product_id), usage_page:c.usage_page, usage:c.usage}`, `get_notifier().lock().unwrap().send_command(QueryInfo, &narrowed)`, `classify_reply`, `classification_cache_insert`, build `ClassifiedDevice`. Per-candidate lock scope (G5).
- [ ] **`fn enumerate_candidates() -> Vec<Candidate>`**: `HidApi::new()` + `configured_filter()` `.filter(usage_page/usage/vid/pid)` `.map(path/vid/pid/product_name/usage_page/usage)`; `Err`⇒`vec![]`. `d.path().to_string_lossy().to_string()` (G3, CStr).
- [ ] **`pub fn classify_devices(verbose: bool) -> Vec<ClassifiedDevice>`**: `enumerate_candidates()` → `invalidate_absent_cache_entries(&cands)` → `classify_candidates(cands, verbose)`. Mode-A rustdoc cites `spec/DEVICE_DISCOVERY.md` §2 + records the filter-narrow mechanism + §4.3 limitation (G4, G10).
- [ ] **`fn invalidate_absent_cache_entries(candidates: &[Candidate])`**: `CLASSIFICATION_CACHE.lock()` `.retain(|path, _| present_paths.contains(path))`.
- [ ] **`fn warm_cache_from_handshake(kind: DeviceKind)`**: `for c in enumerate_candidates() { classification_cache_insert(&c.path, kind.clone()); }` + comment documenting the single-vid/pid assumption.
- [ ] **4 calls in `perform_handshake_with`**: Capable arm ⇒ `warm_cache_from_handshake(DeviceKind::Capable{..})`; Timeout/Ok(other)/Err arms ⇒ `warm_cache_from_handshake(DeviceKind::NotQmkNotifier)`. Each AFTER the existing HOST_CAPABLE set/clear (G14).
- [ ] **MockNotifier extension**: `static MOCK_SEND_COMMAND_FILTERS: Lazy<StdMutex<Vec<(Option<u16>,Option<u16>,u16,u16)>>>`; cleared in `reset_global_mock`; recorded in `send_command` (`_filter`→`filter`); `MockNotifier::get_send_command_filters()` getter (G8).
- [ ] **~13 tests** pass (6 classify_reply + 5 classify_candidates incl. hit/miss/TTL + 1 invalidate + 1 smoke), prefix `test_classify_` (disjoint from S1's `test_classification_cache_*`/`test_devicekind_*`, the `r_coex_*`, the 49 `test_*`).
- [ ] Existing handshake tests (30+) still pass UNCHANGED (G9 — cross-feed is a no-op under MockNotifier).
- [ ] `cargo build --bin qmkonnect` clean; `cargo test --bin qmkonnect -- --test-threads=1` green; `git status` = `src/core/notifier.rs` only.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can implement this using only this PRP, because: (a) the exact §2 algorithm + the crate's CommandResponse variants + the no-per-path-send constraint are reproduced in `research/notes.md` §2; (b) the hidapi DeviceInfo API (`path()->&CStr`, `vendor_id()`, `product_string()->Option<&str>`) + the verbatim enumerate-and-filter mirror of `is_device_connected` @216 are in §3; (c) the narrowed-filter per-candidate ping body + the per-candidate lock discipline (mirroring the callback sweep @446) are in §4; (d) the pure `classify_reply` body (the §2.2 match, NO feature-bit gate) is in §5; (e) the factored-core design (REQUIRED for the MockNotifier test mandate — `classify_candidates` takes a `Vec<Candidate>`) is justified in §6; (f) the invalidation + warm-from-handshake bodies + the 4 arm edit sites are in §7; (g) the minimal mock extension (record filter tuples, NOT DeviceFilter) is in §8; (h) the ~13-test plan with the verbatim setup pattern is in §9; (i) 14 gotchas are pinned (G0 parallel, G1 must-factor, G2 no-feature-gate, G3 CStr, G4 vid/pid-narrow+limitation, G5 per-candidate-lock, G6 unwrap, G7 single-thread, G8 mock-additive, G9 no-op-in-tests, G10 doctest, G11 verbose, G12 HashSet, G13 scope, G14 handshake-untouched); (j) consumer contracts + scope wall are in §11/§12.

### Documentation & References

```yaml
# MUST READ — the spec source of truth (the §2 algorithm + the §2.2 classifier)
- url: spec/DEVICE_DISCOVERY.md
  why: "§2.1 (why Tier-2), §2.2 (the QUERY_INFO match: capable iff Info{proto_ver:2}; NO feature-bit
        gate; everything else incl. clean Timeout = NotQmkNotifier; no board harmed), §2.3 (the
        classify_devices algorithm: HidApi::new + Tier-1 filter, per-candidate open+send+classify+close,
        cache keyed by path, 5s TTL), §2.4 (same QUERY_INFO tx as perform_handshake; per-candidate vs
        once-per-boot), §4.3 (the multi-same-vid/pid v1 limitation to document)."
  section: "## 2. The Capability Probe (§2.1-§2.4)" and "### 4.3 v1 limitation"

# MUST READ — the verbatim research (THIS task's full contract)
- file: plan/005_8b95ea464bd9/P3M1T1S2/research/notes.md
  why: "§1 the S1 upstream contract (the types/cache/helpers this task calls). §2 the crate's
        CommandResponse variants + the no-per-path-send constraint + the proto_ver==2 (no feature-gate)
        decision. §3 the hidapi API (path()->&CStr) + the verbatim enumerate mirror of is_device_connected.
        §4 the narrowed-filter ping + per-candidate lock discipline. §5 the classify_reply body. §6 the
        REQUIRED factored-core design (classify_candidates) + why inlining breaks the test mandate.
        §7 invalidation + warm-from-handshake + the 4 arm edit sites. §8 the minimal mock extension.
        §9 the ~13-test plan. §10 14 gotchas. §11/§12 consumer contracts + scope wall."

# MUST READ — the file THIS task edits (every referenced line confirmed by reading)
- file: src/core/notifier.rs
  why: "DeviceFilter @67-72 (the struct S2 narrows per-candidate). configured_filter @83 (the Tier-1
        vid/pid/page/usage resolver). list_devices @129 (hidapi device_list + product_string precedent).
        is_device_connected @216-232 (the ENUMERATE+FILTER pattern to mirror with .filter/.map/.collect).
        perform_handshake_with @421 (the 4 arms: Capable @444, Timeout @576, Ok(other) @588, Err @600 —
        the cross-feed edit sites; note the lock is held in the capable arm until `drop(n)` then re-acquired
        per sweep iteration @446). get_notifier @982. NOTIFIER @912. host_os @305. MockNotifier @1440 +
        MOCK_* statics @1414-1426 + reset_global_mock @1428 + send_command @1497 (the `_filter`→`filter`
        edit + the filter-record insert) + reset_test_state @1510. now_ms via crate::core::now_ms()."
  pattern: "verbose logging is eprintln!('[{}ms] ...', crate::core::now_ms(), ...). Lock = .lock().unwrap()
            (poison panic, matches handshake). Tests use reset_test_state()+set_notifier(MockNotifier::new())."
  gotcha: "G6: the item writes get_notifier().lock().send_command(...) but lock() returns LockResult —
           use .lock().unwrap() like perform_handshake @435. G5: acquire per-candidate (short scope)."

# MUST READ — the S1 contract (the types/cache/helpers this task consumes)
- file: plan/005_8b95ea464bd9/P3M1T1S1/PRP.md
  why: "defines DeviceKind/ClassifiedDevice/CLASSIFICATION_CACHE/CLASSIFICATION_TTL + the 3 helpers
        (classification_cache_get/insert/clear) this task CALLS. Field shapes: DeviceKind::Capable{
        proto_ver:u8, feature_flags:u8, callback_count:u8, board_rules_present:bool}; ClassifiedDevice{
        path:String, vendor_id:u16, product_id:u16, product_name:Option<String>, usage_page:u16,
        usage:u16, kind:DeviceKind}. classification_cache_get returns Option<DeviceKind> (TTL-checked)."
  section: "## What (Data models block) + Implementation Tasks Task 2/3"

# MUST READ — the crate boundary (proves the no-per-path-send constraint; pins CommandResponse)
- file: plan/005_8b95ea464bd9/architecture/external_deps.md
  why: "CommandResponse::Info { proto_ver, feature_flags, callback_count, board_rules_present } (the
        capable classifier's source). THE CRITICAL CONSTRAINT: 'There is NO per-path send and NO
        per-device send in the crate. The only send primitives take a MatchKey (vid/pid/page/usage) and
        broadcast to ALL matching devices.' ⇒ S2 narrows the FILTER by vid/pid (the sole app-side knob).
        Also: MatchKey/open_matching_devices/DEVICE_CACHE are PRIVATE — confirmed no per-path API exists."

# MUST READ — the existing mechanism map (confirms the handshake is the same QUERY_INFO tx)
- file: plan/005_8b95ea464bd9/architecture/notifier_mechanisms.md
  why: "is_device_connected() @216 (Tier-1 enumerate — the pattern classify_devices mirrors).
        perform_handshake_with @421 (the existing QUERY_INFO tx — S2's per-candidate probe is the same
        tx, which is why the cross-feed is valid). HOST_CAPABLE/host_capable @270/689 (the GLOBAL
        capability; S2's per-path cache is the per-device complement). reset_test_state + MockNotifier
        setup pattern (the test idiom)."

# Reference — the crate's public enums (read-only, pinned rev f26893e)
- file: ~/.cargo/git/checkouts/qmk-notifier-1f15950b695d9922/f26893e/src/lib.rs
  why: "lines 86-115 are CommandResponse (Legacy/Info/CallbackName/Ack/Timeout) — the variants
        classify_reply matches. line 19+ is RunCommand (QueryInfo is the probe). Confirm rev via
        `grep -A3 'name = \"qmk-notifier\"' Cargo.lock` (expect f26893e)."

# Reference — hidapi DeviceInfo (the path()->&CStr gotcha)
- url: https://docs.rs/hidapi/2.6.3/hidapi/struct.DeviceInfo.html
  why: "documents path() -> &CStr (NOT &OsStr — use to_string_lossy().to_string()), vendor_id()/product_id()
        -> u16, usage_page()/usage() -> u16, product_string() -> Option<&str>. These are the Tier-1
        enumerate fields classify_devices reads."
  critical: "path() returns &CStr. Do NOT call .to_str().unwrap() (panics on non-UTF8 Windows paths);
             use .to_string_lossy().to_string() (Cow<str>→String)."
```

### Current Codebase tree (relevant subset)

```bash
src/
  main.rs              # `mod core;` (binary-only crate — NO lib.rs; see G10)
  core/
    notifier.rs        # Notifier trait @16; DeviceFilter @67; configured_filter @83; list_devices @129;
                         #   is_device_connected @216 (the ENUMERATE pattern); HOST_CAPABLE @270;
                         #   perform_handshake_with @421 (4 arms = cross-feed sites); get_notifier @982;
                         #   MockNotifier @1440 + MOCK_* @1414 + reset_global_mock @1428 + send_command @1497;
                         #   reset_test_state @1510; #[cfg(test)] (49+ tests, --test-threads=1)
                         #   <-- S1 adds the Device-classification banner section (DeviceKind/ClassifiedDevice/cache)
                         #   <-- THIS TASK adds the classify_* functions + Candidate to that section,
                         #       + 4 warm_cache_from_handshake calls in perform_handshake arms,
                         #       + the MOCK_SEND_COMMAND_FILTERS extension.
    types.rs / mod.rs / pattern.rs / rules.rs   # unchanged
Cargo.toml             # hidapi 2.6, qmk-notifier v0.3.0 (rev f26893e), once_cell 1.21 — UNCHANGED
spec/DEVICE_DISCOVERY.md   # §2 = the algorithm source of truth (READ-ONLY)
```

### Desired Codebase tree (files this task changes)

```bash
src/
  core/
    notifier.rs        # MODIFIED (additive) — + classify_reply/Candidate/classify_candidates/
                         #     enumerate_candidates/classify_devices/invalidate_absent_cache_entries/
                         #     warm_cache_from_handshake (in S1's banner section)
                         #   + 4 warm_cache_from_handshake(kind) calls in perform_handshake_with arms
                         #   + MOCK_SEND_COMMAND_FILTERS static + reset_global_mock clear + send_command
                         #     filter-record + get_send_command_filters() getter
                         #   + ~13 test_classify_* tests
    # EVERYTHING else unchanged (Cargo.toml, types.rs, mod.rs, pattern.rs, rules.rs, tray*.rs, ...)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (G0 — runs in PARALLEL with S1): DeviceKind/ClassifiedDevice/CLASSIFICATION_CACHE/CLASSIFICATION_TTL
//   + the 3 classification_cache_* helpers are P3.M1.T1.S1's. They may not exist when this task starts.
//   This task references them and won't compile until S1 merges (EXPECTED — orchestrator lands both before
//   the gate). Do NOT redefine them (merge conflict). Place THIS task's functions so they sit inside S1's
//   `// ===== Device classification (P3.M1) =====` banner once merged.
//
// CRITICAL (G1 — FACTOR THE CORE OUT; this is REQUIRED, not optional): classify_devices enumerates REAL
//   HID (HidApi::new) — uncontrollable in a unit test (CI box may have 0 QMK boards). The item's OUTPUT
//   mandates "unit tests over the MockNotifier (capable/legacy/timeout → DeviceKind; cache hit/miss/TTL)".
//   The ONLY way to make those tests deterministic is to factor the per-candidate core into
//   `classify_candidates(candidates: Vec<Candidate>, verbose)` and drive IT with hand-built candidates +
//   queued mock responses. Do NOT inline all logic into classify_devices and hand-wave the tests.
//
// CRITICAL (G2 — classify on proto_ver==2, NOT the feature bit): spec §2.2 — capable iff Info{proto_ver:2}.
//   The handshake (@444) ADDS `feature_flags & 0x01` for the host-rules SEND; the classifier does NOT — it
//   records feature_flags in Capable for the consumer. Adding the gate hides capable-but-no-host-rules
//   boards from the picker (diverges from §2.2). The item confirms: "Info{proto_ver:2} -> Capable{...}".
//
// CRITICAL (G3 — d.path() returns &CStr, not &OsStr/&str): hidapi 2.6.3 DeviceInfo::path() -> &CStr
//   (confirmed src/lib.rs:358). Convert via `d.path().to_string_lossy().to_string()`. Do NOT
//   `.to_str().unwrap()` (panics on non-UTF8 paths). product_string() -> Option<&str> (.map(to_string)).
//
// CRITICAL (G4 — narrow filter to vid/pid; document the limitation): the crate has NO per-path send
//   (external_deps.md: MatchKey private + filter-keyed). vid/pid is the ONLY app-side per-candidate knob.
//   Build DeviceFilter{vendor_id:Some(c.vendor_id), product_id:Some(c.product_id), usage_page:c.usage_page,
//   usage:c.usage}. DOCUMENT in a comment: this is a true single-device ping ONLY when vid/pid is unique
//   on the bus; two boards sharing vid/pid both get pinged (DEVICE_DISCOVERY.md §4.3 v1 limitation).
//
// CRITICAL (G5 — per-candidate lock scope): acquire get_notifier().lock().unwrap() INSIDE the per-candidate
//   loop (one lock per candidate), NOT once across all. Mirrors the callback sweep's per-iteration
//   re-acquire (notifier.rs:446-455) so a concurrent notify_qmk/debounce-flush can interleave between
//   candidates. Holding one lock across all candidates starves the notification path.
//
// GOTCHA (G6 — .lock().unwrap() for the notifier lock): the item writes get_notifier().lock().send_command(...)
//   but Mutex::lock returns LockResult (won't compile without unwrap). Use .lock().unwrap() like
//   perform_handshake @435. (S1's cache helpers use the non-panicking .lock().ok()?/if-let form — that's
//   their choice for a cache; the NOTIFIER lock follows the handshake's .unwrap() since a poisoned
//   NOTIFIER is a hard failure.)
//
// GOTCHA (G7 — single-threaded tests crate-wide): cargo test --bin qmkonnect -- --test-threads=1
//   (shared MockNotifier globals + DebounceState, AGENTS.md). Each cache-touching test STARTS with
//   classification_cache_clear() (the static outlives tests — S1's G4).
//
// CRITICAL (G8 — MockNotifier extension is ADDITIVE + minimal): add ONE static MOCK_SEND_COMMAND_FILTERS
//   (Vec<(Option<u16>,Option<u16>,u16,u16)>); clear it in reset_global_mock; change `_filter`→`filter` in
//   send_command + push the tuple; add get_send_command_filters() getter. Do NOT touch MOCK_RESPONSES
//   (FIFO already per-device-ordered — candidate i pops response[i]), MOCK_SEND_COMMAND_ERRORS,
//   MOCK_SEND_DELAY, or the default Ack{ok:true} fallback. Record TUPLES, not DeviceFilter (avoids adding
//   Clone/PartialEq derives to a production struct).
//
// CRITICAL (G9 — the handshake cross-feed is a NO-OP in tests, so existing tests are unaffected):
//   warm_cache_from_handshake calls enumerate_candidates() (HidApi::new). Under MockNotifier (no real HID)
//   it finds 0 Tier-1 devices ⇒ stamps nothing ⇒ the 30+ existing handshake tests (asserting HOST_CAPABLE/
//   callback_names/send_command calls) see NO change (the cache is a separate static). Do NOT add cache
//   assertions to the existing handshake tests.
//
// GOTCHA (G10 — binary-only crate; doctests don't run under `--bin`): Mode-A doc-comments are PROSE citing
//   spec/DEVICE_DISCOVERY.md §2 + recording the filter-narrow mechanism + §4.3 limitation. Use
//   `// ` for doc-comments; for any code sketch use ```rust,ignore```. Do NOT add bare ``` runnable doctests.
//
// GOTCHA (G11 — verbose logging): eprintln!("[{}ms] classify: ...", crate::core::now_ms(), ...) when
//   verbose (matches perform_handshake/notify_qmk). Terse. send_command verbose=false (transport quiet).
//
// GOTCHA (G12 — HashSet not imported): notifier.rs:3 is `use std::collections::{BTreeSet, HashMap};`.
//   Use Vec::contains in invalidate_absent_cache_entries (no new import) OR add HashSet to that use line
//   (one-token edit). hidapi/qmk_notifier/Duration/Instant/Lazy/Mutex/HashMap all in scope. now_ms =
//   crate::core::now_ms().
//
// CRITICAL (G13 — scope): do NOT touch the write path (P2 DEFER), device_status() (P1 Complete), the picker
//   (P3.M2), or CLI flags (P4). THIS task = classify_devices + cache mechanics + handshake cross-feed +
//   mock extension + tests. Nothing else.
//
// CRITICAL (G14 — do NOT change the handshake's core logic): the cross-feed ADDS one
//   warm_cache_from_handshake(kind) call per arm AFTER the existing HOST_CAPABLE set/clear. It does NOT
//   alter the dedup (HAS_HANDSHAKED), SET_OS, the callback sweep, or the BOARD_HAS_RULES-before-HOST_CAPABLE
//   ordering invariant. The 4 edit sites: Capable arm @~444 (after HOST_CAPABLE.store(true)), Timeout @~576
//   (after HAS_HANDSHAKED.store(false)), Ok(other) @~588, Err @~600 (after HAS_HANDSHAKED.store(false)).
//
// CRATE QUIRK: the crate-wide test command MUST be single-threaded:
//   cargo test --bin qmkonnect -- --test-threads=1   (AGENTS.md)
```

## Implementation Blueprint

### Data models and structure

No new PUBLIC data models (S1 owns `DeviceKind`/`ClassifiedDevice`). This task adds
one PRIVATE internal helper struct + functions that consume S1's types + the
existing `configured_filter()`/`get_notifier()`/`CLASSIFICATION_CACHE`.

```rust
// ── (1) the pure §2.2 reply classifier ──
/// Classify a QUERY_INFO reply into a [`DeviceKind`] (DEVICE_DISCOVERY.md §2.2).
/// `Info { proto_ver: 2, .. }` ⇒ [`DeviceKind::Capable`] (records all 4 fields);
/// every other reply (Legacy / Timeout / Ack / CallbackName / `Info{proto_ver:1}`)
/// and every error ⇒ [`DeviceKind::NotQmkNotifier`]. Does NOT gate on the
/// `feature_flags & 0x01` APPLY_HOST_CONTEXT bit — that is the handshake's concern
/// for the host-rules SEND; the classifier records `feature_flags` so the picker
/// can show it. No board is harmed: the 0x81 0x9F magic is ignored by VIA/Vial.
fn classify_reply(
    resp: Result<qmk_notifier::CommandResponse, Box<dyn Error + Send + Sync>>,
) -> DeviceKind {
    match resp {
        Ok(qmk_notifier::CommandResponse::Info {
            proto_ver: 2,
            feature_flags,
            callback_count,
            board_rules_present,
        }) => DeviceKind::Capable {
            proto_ver: 2,
            feature_flags,
            callback_count,
            board_rules_present,
        },
        _ => DeviceKind::NotQmkNotifier,
    }
}

// ── (2) the internal Tier-1 descriptor (factored for testability — G1) ──
/// One enumerated Tier-1 HID interface, pre-classification. Factored out of
/// [`classify_devices`] so [`classify_candidates`] is testable without a real
/// HID bus (the enumerate step talks to `hidapi::HidApi::new()`).
struct Candidate {
    path: String,
    vendor_id: u16,
    product_id: u16,
    product_name: Option<String>,
    usage_page: u16,
    usage: u16,
}
```

(The full bodies of `classify_candidates`/`enumerate_candidates`/`classify_devices`/
`invalidate_absent_cache_entries`/`warm_cache_from_handshake` are in
`research/notes.md` §4/§6/§7 — reproduced in Implementation Patterns below.)

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: RECONCILE the S1 banner section (parallel with S1 — see G0)
  - DO: `grep -n 'pub enum DeviceKind\|pub struct ClassifiedDevice\|static CLASSIFICATION_CACHE\|pub fn classification_cache_' src/core/notifier.rs`.
    (a) S1 LANDED (all present): add THIS task's items INSIDE S1's `// ===== Device classification (P3.M1) =====`
        banner section, AFTER the 3 cache helpers (keep contiguous).
    (b) S1 NOT YET LANDED (absent): add THIS task's items in a clearly-marked region with a comment
        `// ===== Device classification (P3.M1) — classify_devices (S2); structs/cache/helpers are S1's =====`
        so it merges into S1's section. The build will fail on the missing DeviceKind/ClassifiedDevice/cache
        until S1 lands — EXPECTED. Do NOT define S1's items.
  - PRESERVE: whatever S1 has written. Additive only.

Task 2: ADD classify_reply + struct Candidate (the pure pieces)
  - DO: add `fn classify_reply(resp) -> DeviceKind` and `struct Candidate { path, vendor_id,
        product_id, product_name, usage_page, usage }` EXACTLY as in the Data-models block.
  - GOTCHA G2: the match arm binds `proto_ver: 2` literally; Info{proto_ver:1} falls to `_`. NO
        feature-bit gate. GOTCHA G1: Candidate is PRIVATE (test-only via `use super::*`).

Task 3: ADD enumerate_candidates (the hidapi shell — mirrors is_device_connected @216)
  - DO: add `fn enumerate_candidates() -> Vec<Candidate>`:
        let f = configured_filter();
        match hidapi::HidApi::new() {
            Ok(api) => api.device_list().filter(|d| {
                d.usage_page() == f.usage_page && d.usage() == f.usage
                    && f.vendor_id.is_none_or(|v| d.vendor_id() == v)
                    && f.product_id.is_none_or(|p| d.product_id() == p)
            }).map(|d| Candidate {
                path: d.path().to_string_lossy().to_string(),       // G3: &CStr -> String
                vendor_id: d.vendor_id(),
                product_id: d.product_id(),
                product_name: d.product_string().map(|s| s.to_string()),
                usage_page: d.usage_page(),
                usage: d.usage(),
            }).collect(),
            Err(_) => Vec::new(),
        }
  - FOLLOW: is_device_connected @216-232 (the enumerate+filter pattern; .filter/.map/.collect vs .any).
  - GOTCHA G3: d.path() is &CStr — to_string_lossy().to_string(). GOTCHA: configured_filter() is the
        Tier-1 resolver (vid/pid/page/usage from config). This fn does NOT ping — pure enumeration (R-COEX safe).

Task 4: ADD classify_candidates (the pure, MockNotifier-testable core — G1)
  - DO: add `fn classify_candidates(candidates: Vec<Candidate>, verbose: bool) -> Vec<ClassifiedDevice>`:
        let notifier = get_notifier();
        candidates.into_iter().map(|c| {
            let kind = match classification_cache_get(&c.path) {
                Some(k) => {
                    if verbose { eprintln!("[{}ms] classify: cache hit {}", crate::core::now_ms(), c.path); }
                    k
                }
                None => {
                    // G4: narrow the filter to this candidate's vid/pid (the crate has no per-path send).
                    // LIMITATION (DEVICE_DISCOVERY.md §4.3): a true single-device ping only when vid/pid is
                    // unique on the bus; two boards sharing vid/pid both get pinged.
                    let narrowed = DeviceFilter {
                        vendor_id: Some(c.vendor_id),
                        product_id: Some(c.product_id),
                        usage_page: c.usage_page,
                        usage: c.usage,
                    };
                    let resp = notifier.lock().unwrap()  // G5: per-candidate scope; G6: .unwrap()
                        .send_command(qmk_notifier::RunCommand::QueryInfo, &narrowed);
                    let kind = classify_reply(resp);
                    classification_cache_insert(&c.path, kind.clone());
                    kind
                }
            };
            ClassifiedDevice {
                path: c.path, vendor_id: c.vendor_id, product_id: c.product_id,
                product_name: c.product_name, usage_page: c.usage_page, usage: c.usage, kind,
            }
        }).collect()
  - GOTCHA G1: this is the test target — takes Vec<Candidate> so tests don't need real HID. G5: lock
        per-candidate (short scope). G6: .lock().unwrap(). G11: verbose log terse. G4: narrowed filter +
        documented limitation.

Task 5: ADD invalidate_absent_cache_entries + classify_devices + warm_cache_from_handshake
  - DO: add:
        fn invalidate_absent_cache_entries(candidates: &[Candidate]) {
            let present: Vec<&str> = candidates.iter().map(|c| c.path.as_str()).collect();
            if let Ok(mut map) = CLASSIFICATION_CACHE.lock() {
                map.retain(|path, _| present.contains(&path.as_str()));   // G12: Vec::contains, no HashSet import
            }
        }
        pub fn classify_devices(verbose: bool) -> Vec<ClassifiedDevice> {
            let candidates = enumerate_candidates();
            invalidate_absent_cache_entries(&candidates);
            classify_candidates(candidates, verbose)
        }
        fn warm_cache_from_handshake(kind: DeviceKind) {
            // Best-effort: enumerate present Tier-1 paths + stamp each. Bounded by the single-vid/pid-on-bus
            // assumption (§4.3): multiple interfaces sharing vid/pid all get the handshake's single result.
            for c in enumerate_candidates() {
                classification_cache_insert(&c.path, kind.clone());
            }
        }
  - DOC-COMMENT classify_devices (Mode A): cite spec/DEVICE_DISCOVERY.md §2.3 (the algorithm) + §2.2
        (the Info{proto_ver:2}⇒Capable match) + record "per-candidate mechanism = filter-narrow-by-vid/pid
        (crate has no per-path send) + §4.3 limitation". G10: prose, no runnable doctest.

Task 6: WIRE the handshake cross-feed (4 surgical calls in perform_handshake_with — G14)
  - DO: in src/core/notifier.rs perform_handshake_with (@421), add ONE call in each of the 4 arms,
        AFTER the existing HOST_CAPABLE set/clear (and after BOARD_HAS_RULES in the capable arm):
        - Capable arm (@~444, the Ok(Info{proto_ver:2, ..}) if feature_flags & 0x01 arm — AFTER
          HOST_CAPABLE.store(true,..) @~558): `warm_cache_from_handshake(DeviceKind::Capable {
          proto_ver: 2, feature_flags, callback_count, board_rules_present });`
        - Timeout arm (@~576, after HAS_HANDSHAKED.store(false,..)): `warm_cache_from_handshake(DeviceKind::NotQmkNotifier);`
        - Ok(other) arm (@~588, after HOST_CAPABLE.store(false,..)): `warm_cache_from_handshake(DeviceKind::NotQmkNotifier);`
        - Err(e) arm (@~600, after HAS_HANDSHAKED.store(false,..)): `warm_cache_from_handshake(DeviceKind::NotQmkNotifier);`
  - GOTCHA G14: ONLY add the call; do NOT change the dedup/SET_OS/sweep/ordering. G9: under MockNotifier
        (no real HID) warm_cache_from_handshake stamps nothing ⇒ existing handshake tests UNAFFECTED.
  - NOTE: the Capable arm currently holds the notifier lock `n` until `drop(n)` before the sweep. The
        warm_cache_from_handshake call does NOT need the lock (it only enumerates + writes the cache) —
        place it AFTER `drop(n)` / after the sweep completes HOST_CAPABLE set, wherever the arm is in a
        lock-free region (the end of the capable arm is fine). The 3 failure arms already `drop(n)` early.

Task 7: EXTEND the MockNotifier (additive — G8)
  - DO: in the #[cfg(test)] region:
        - add `static MOCK_SEND_COMMAND_FILTERS: Lazy<StdMutex<Vec<(Option<u16>, Option<u16>, u16, u16)>>> =
          Lazy::new(|| StdMutex::new(Vec::new()));` (next to MOCK_SEND_COMMAND_CALLS @1416).
        - in reset_global_mock (@1428): add `MOCK_SEND_COMMAND_FILTERS.lock().unwrap().clear();`.
        - in MockNotifier::send_command (@1497): change the param `_filter: &DeviceFilter` → `filter: &DeviceFilter`,
          and after the MOCK_SEND_COMMAND_CALLS push, add:
          `MOCK_SEND_COMMAND_FILTERS.lock().unwrap().push((filter.vendor_id, filter.product_id, filter.usage_page, filter.usage));`
        - in impl MockNotifier: add
          `fn get_send_command_filters() -> Vec<(Option<u16>, Option<u16>, u16, u16)> { MOCK_SEND_COMMAND_FILTERS.lock().unwrap().clone() }`.
  - GOTCHA G8: do NOT touch MOCK_RESPONSES (FIFO already per-device-ordered), MOCK_SEND_COMMAND_ERRORS,
        MOCK_SEND_DELAY, or the Ack{ok:true} default. Record tuples, NOT DeviceFilter.

Task 8: ADD ~13 tests (prefix test_classify_, single-threaded — G7)
  - DO: append to the existing #[cfg(test)] mod tests (use super::*). Each cache-touching test STARTS with
        reset_test_state(); reset_handshake_state(); set_notifier(Box::new(MockNotifier::new()));
        classification_cache_clear();  (mirror test_send_command_reset_clears_log @1940). See §9 of the
        research notes for the verbatim ~13 tests:
        A. classify_reply (6, pure — no mock): info_proto2_capable; info_proto1_notqmk; legacy_notqmk;
           timeout_notqmk; ack_notqmk; err_notqmk.
        B. classify_candidates (5, MockNotifier): capable (1 cand + [Info{p:2}] ⇒ Capable; assert 1 call +
           filter narrowed via get_send_command_filters()); mixed (3 cands + [Info{p:2},Legacy,Timeout] ⇒
           [Capable,NotQmk,NotQmk]; 3 calls, per-candidate filters); cache_hit_skips_ping (pre-insert path,
           empty queue ⇒ 0 calls); cache_miss_pings_and_caches (empty cache + [Info{p:2}] ⇒ Capable, then
           2nd call empty-queue ⇒ still Capable, call count unchanged); ttl_re_ping (rewrite cached Instant
           to now-TTL-1ms via CLASSIFICATION_CACHE.lock().unwrap().insert(...), then [Timeout] ⇒ NotQmk, 1 call).
        C. invalidate (1): invalidate_drops_absent_paths (insert p1+p2, invalidate with [Candidate{path:"p1"}],
           assert p1 kept p2 gone — reach into CLASSIFICATION_CACHE from same-module test).
        D. smoke (1): classify_devices_smoke_returns_vec (classify_devices(false) returns without panic;
           len is env-dependent — do NOT assert a count).
  - GOTCHA G7: --test-threads=1. G8: use MockNotifier::get_send_command_filters() for the narrowing assertion.
        For the TTL test, reach into CLASSIFICATION_CACHE directly (same module) to rewrite the Instant —
        same idiom as S1's test_classification_cache_ttl_expiry.

Task 9: VERIFY build + tests (single-threaded, per AGENTS.md)
  - RUN: cargo build --bin qmkonnect   (expect clean once S1 has landed; no NEW warnings)
  - RUN: cargo test --bin qmkonnect classify -- --test-threads=1   (the ~13 new test_classify_* pass)
  - RUN: cargo test --bin qmkonnect handshake -- --test-threads=1  (the 30+ existing handshake tests STILL
         pass — G9: cross-feed is a no-op under MockNotifier)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1   (full crate green — new tests + S1's cache tests
         + 49 existing + r_coex + pattern/rules/types; no regression)
  - CONFIRM git status shows EXACTLY one file: src/core/notifier.rs.
  - IF a classify_candidates test fails: re-read research/notes.md §4 (narrowed filter) + §5 (classify_reply)
        + §8 (mock extension). The mechanism is the spec; a failure = a transcription slip (wrong filter
        tuple, missing cache clear, holding the lock across candidates). Do NOT "fix" the test to match
        divergent behavior.
```

### Implementation Patterns & Key Details

```rust
// The canonical bodies (this IS the contract — match it; research/notes.md §4-§8).

// (Task 4) classify_candidates — the MockNotifier-testable core (G1 factoring):
// fn classify_candidates(candidates: Vec<Candidate>, verbose: bool) -> Vec<ClassifiedDevice> {
//     let notifier = get_notifier();
//     candidates.into_iter().map(|c| {
//         let kind = match classification_cache_get(&c.path) {
//             Some(k) => k,                                  // cache hit — no ping
//             None => {
//                 let narrowed = DeviceFilter {              // G4: narrow by vid/pid
//                     vendor_id: Some(c.vendor_id), product_id: Some(c.product_id),
//                     usage_page: c.usage_page, usage: c.usage,
//                 };
//                 let resp = notifier.lock().unwrap()        // G5 per-candidate; G6 unwrap
//                     .send_command(qmk_notifier::RunCommand::QueryInfo, &narrowed);
//                 let kind = classify_reply(resp);
//                 classification_cache_insert(&c.path, kind.clone());
//                 kind
//             }
//         };
//         ClassifiedDevice { path: c.path, vendor_id: c.vendor_id, product_id: c.product_id,
//                            product_name: c.product_name, usage_page: c.usage_page, usage: c.usage, kind }
//     }).collect()
// }

// (Task 7) MockNotifier extension — the filter record (G8):
// static MOCK_SEND_COMMAND_FILTERS: Lazy<StdMutex<Vec<(Option<u16>, Option<u16>, u16, u16)>>> =
//     Lazy::new(|| StdMutex::new(Vec::new()));
// fn send_command(&self, command: RunCommand, filter: &DeviceFilter)   // was _filter
//     -> Result<CommandResponse, Box<dyn Error + Send + Sync>> {
//     MOCK_SEND_COMMAND_CALLS.lock().unwrap().push(command.clone());
//     MOCK_SEND_COMMAND_FILTERS.lock().unwrap().push(
//         (filter.vendor_id, filter.product_id, filter.usage_page, filter.usage));   // NEW
//     /* ... existing delay/error/response logic unchanged ... */
// }

// (Task 8) test idiom — the cache-hit assertion:
// #[test] fn test_classify_candidates_cache_hit_skips_ping() {
//     reset_test_state(); reset_handshake_state();
//     set_notifier(Box::new(MockNotifier::new()));
//     classification_cache_clear();
//     let c = Candidate { path: "p".into(), vendor_id: 0x1234, product_id: 0x5678,
//                         product_name: None, usage_page: 0xFF60, usage: 0x61 };
//     classification_cache_insert(&c.path, DeviceKind::Capable { proto_ver: 2,
//         feature_flags: 0x01, callback_count: 0, board_rules_present: false });
//     let result = classify_candidates(vec![c], false);   // EMPTY response queue
//     assert_eq!(result[0].kind, DeviceKind::Capable { proto_ver: 2, feature_flags: 0x01,
//         callback_count: 0, board_rules_present: false });
//     assert!(MockNotifier::get_send_command_calls().is_empty());   // cache hit ⇒ NO ping
// }

// NOTE: `get_notifier()` returns Arc<Mutex<Box<dyn Notifier>>>; .lock().unwrap() derefs to dyn Notifier.
// classification_cache_get/insert/clear are S1's pub helpers. DeviceKind::Capable derives PartialEq (S1)
// so assert_eq! works directly.
```

### Integration Points

```yaml
CODE (this task):
  - file: src/core/notifier.rs
    change: "additive — classify_* functions + Candidate in S1's banner section; 4 warm_cache_from_handshake
             calls in perform_handshake_with arms; MOCK_SEND_COMMAND_FILTERS extension; ~13 tests"
    pattern: "enumerate mirrors is_device_connected @216; per-candidate lock mirrors the callback sweep @446;
              mock-tuple-record avoids touching DeviceFilter derives; tests mirror test_send_command_reset_clears_log @1940"

DEPENDENCIES (this task): NONE new. hidapi (2.6), qmk-notifier (v0.3.0 rev f26893e), once_cell (1.21),
                           std — all already Cargo deps. NO new `use` lines (HashSet avoided via Vec::contains — G12).

UPSTREAM (consumed unchanged):
  - DeviceKind/ClassifiedDevice/CLASSIFICATION_CACHE/CLASSIFICATION_TTL + classification_cache_get/insert/clear
    (P3.M1.T1.S1). DeviceKind::Capable derives Clone+PartialEq (S1) ⇒ cache clone + assert_eq! work.
  - configured_filter() @83 (Tier-1 resolver). get_notifier() @982 / NOTIFIER @912. hidapi::HidApi::new +
    device_list (the enumerate API; list_devices @129 + is_device_connected @216 precedent).
  - qmk_notifier::{RunCommand::QueryInfo, CommandResponse::Info{..}} (crate rev f26893e).

DOWNSTREAM CONSUMERS (later subtasks — do NOT implement them here):
  - P3.M2.T1 (picker, 3 platforms): calls classify_devices(verbose) → renders Vec<ClassifiedDevice>;
    "Rescan" calls classification_cache_clear() + classify_devices().
  - P1 device_status() (Complete): per-device complement; no change this task.
  - P4.M1.T1.S1 (--list-devices kind column): one-shot classify_devices().
  - poll threads (tray.rs/linux_tray.rs): TODAY call perform_handshake on false→true. The cross-feed (Task 6)
    warms the cache from that existing ping. Wiring classify_devices into the poll transition is a LATER task.

NO OVERLAP:
  - P2.M1.T1.S1 (Complete): R-COEX comments + tests — separate section, untouched.
  - P2.M1.T1.S2 (Complete): write_narrowing_decision.md — DEFER, zero .rs overlap. classify_devices feeds
    the picker/status, NOT the write path.

CONFIG: none. ROUTES: none (no CLI surface — --list-devices kind column is P4.M1.T1.S1). DATABASE: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles clean once P3.M1.T1.S1 has landed (no NEW warnings). If it fails on missing
# DeviceKind/ClassifiedDevice/CLASSIFICATION_CACHE — S1 hasn't landed yet (G0, expected); the orchestrator
# lands both before the gate. If it fails AFTER both landed (typo, a held-too-long lock, a missing
# `filter` rename in the mock), READ it and fix.

# Confirm the deliverables are present:
grep -n 'pub fn classify_devices' src/core/notifier.rs          # expect 1
grep -n 'fn classify_candidates' src/core/notifier.rs           # expect 1
grep -n 'fn classify_reply' src/core/notifier.rs                # expect 1
grep -n 'fn enumerate_candidates' src/core/notifier.rs          # expect 1
grep -n 'fn invalidate_absent_cache_entries' src/core/notifier.rs  # expect 1
grep -n 'fn warm_cache_from_handshake' src/core/notifier.rs     # expect 1 (+ 4 call sites in perform_handshake)
grep -n 'MOCK_SEND_COMMAND_FILTERS' src/core/notifier.rs        # expect >=3 (static + clear + push + maybe getter)
grep -c 'warm_cache_from_handshake' src/core/notifier.rs        # expect 5 (1 def + 4 calls)
# Confirm NO feature-bit gate leaked into the classifier (G2):
! grep -A12 'fn classify_reply' src/core/notifier.rs | grep 'feature_flags & 0x01' || echo "FAIL: feature-bit gate in classifier (G2)"
# Confirm the mock `_filter` was renamed (G8):
grep -n 'fn send_command' src/core/notifier.rs | head   # the MockNotifier impl line should say `filter: &DeviceFilter`
```

### Level 2: Unit Tests — the classify core (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# Single-threaded is MANDATORY crate-wide (shared MockNotifier globals + DebounceState, AGENTS.md).
cargo test --bin qmkonnect classify -- --test-threads=1
# Expected: all ~13 test_classify_* pass (6 classify_reply + 5 classify_candidates incl. cache hit/miss/TTL
# + 1 invalidate + 1 smoke). Filter to families:
cargo test --bin qmkonnect test_classify_reply -- --test-threads=1          # 6 pure classifier tests
cargo test --bin qmkonnect test_classify_candidates -- --test-threads=1     # 5 MockNotifier core tests
cargo test --bin qmkonnect test_invalidate_drops_absent_paths -- --test-threads=1
cargo test --bin qmkonnect test_classify_devices_smoke -- --test-threads=1
```

### Level 3: Full-crate regression (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
# CRITICAL: the handshake cross-feed (Task 6) must NOT break the 30+ existing handshake tests.
cargo test --bin qmkonnect handshake -- --test-threads=1
# Expected: all handshake tests pass UNCHANGED (G9: warm_cache_from_handshake is a no-op under MockNotifier
# — 0 Tier-1 devices ⇒ stamps nothing ⇒ HOST_CAPABLE/callback_names/send_command assertions unaffected).

# Full crate:
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL bin tests green — ~13 new test_classify_* + S1's test_classification_cache_* /
# test_devicekind_* + the 49 existing + r_coex_invariants + pattern + rules + types + mod. Proves the
# additive section + the 4 handshake-arm calls + the mock extension compile in the full crate and didn't
# disturb the statics cluster, the handshake, or the R-COEX invariants.

# Confirm the change surface is exactly one file:
git status --short
# Expected: only src/core/notifier.rs modified. NOTHING in Cargo.toml, types.rs, mod.rs, tray*.rs,
# architecture/, docs/, spec/.
git diff --stat
# Expected: 1 file: src/core/notifier.rs.
```

### Level 4: N/A for the unit-testable core (the hidapi shell is env-dependent)

The classify_candidates core + classify_reply + cache mechanics are fully covered by Level-2 unit tests
over the MockNotifier (the item's mandated test surface). `classify_devices`/`enumerate_candidates`/
`warm_cache_from_handshake` touch real HID and are env-dependent — covered by the smoke test
(test_classify_devices_smoke_returns_vec: returns a Vec without panic) + the Level-3 full-crate green run.
On a box with a real qmk_notifier board flashed, a manual `classify_devices(true)` (once the P4 CLI / P3.2
picker calls it) would show the Capable classification; that end-to-end check belongs to P3.M2/P4, not here.

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean (once S1 landed; no NEW warnings).
- [ ] `cargo test --bin qmkonnect classify -- --test-threads=1` — all ~13 `test_classify_*` pass.
- [ ] `cargo test --bin qmkonnect handshake -- --test-threads=1` — 30+ handshake tests pass UNCHANGED (G9).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` — full crate green (no regression).
- [ ] `git status` shows exactly ONE modified file: `src/core/notifier.rs`.

### Feature Validation (contract fidelity)
- [ ] **classify_reply**: `Ok(Info{proto_ver:2,..})`⇒`Capable{..}` (4 fields); `Ok(Legacy|Timeout|Ack|CallbackName|Info{proto_ver:1})` + `Err(_)`⇒`NotQmkNotifier`; NO feature-bit gate (G2).
- [ ] **classify_candidates**: cache hit ⇒ 0 `send_command` calls; cache miss ⇒ 1 ping per candidate, result cached; TTL-expired ⇒ re-ping.
- [ ] **filter narrowing**: each candidate's `send_command` is called with `(Some(c.vendor_id), Some(c.product_id), c.usage_page, c.usage)` (asserted via `get_send_command_filters()`).
- [ ] **per-candidate ordering**: N candidates consume N queued responses in FIFO order (candidate i ⇔ response i).
- [ ] **enumerate_candidates**: `HidApi::new()` + `configured_filter()` `.filter`/`.map`; `Err`⇒`vec![]`; `d.path().to_string_lossy().to_string()` (G3).
- [ ] **classify_devices**: enumerate → invalidate-absent → classify_candidates; Mode-A rustdoc cites `spec/DEVICE_DISCOVERY.md` §2 + records filter-narrow mechanism + §4.3 limitation.
- [ ] **invalidate_absent_cache_entries**: `retain` keeps only present paths (test evicts a disappeared path).
- [ ] **warm_cache_from_handshake**: 4 calls in `perform_handshake_with` arms (Capable⇒Capable{..}; 3 failure arms⇒NotQmkNotifier), each after the existing HOST_CAPABLE set/clear.
- [ ] **MockNotifier extension**: `MOCK_SEND_COMMAND_FILTERS` recorded + cleared + getter; `_filter`→`filter`; MOCK_RESPONSES/errors/delay/default-Ack UNCHANGED.

### Code Quality Validation
- [ ] classify core FACTORED out (`classify_candidates` takes `Vec<Candidate>`) — G1 (REQUIRED for the test mandate).
- [ ] Per-candidate lock scope (G5); `.lock().unwrap()` (G6).
- [ ] Mode-A doc-comment on `classify_devices` cites `spec/DEVICE_DISCOVERY.md` §2 + the mechanism + limitation (G10).
- [ ] Mock extension is additive (G8); records tuples not DeviceFilter.
- [ ] Handshake arms ONLY gain one call each (G14); dedup/SET_OS/sweep/ordering untouched.
- [ ] New tests prefixed `test_classify_` (disjoint from S1's `test_classification_cache_*`/`test_devicekind_*`, `r_coex_*`, the 49 `test_*`); each cache-touching test starts with `classification_cache_clear()` (G7).
- [ ] No new Cargo deps; `HashSet` avoided via `Vec::contains` (G12) or a one-token import edit.
- [ ] No `unsafe`; no new module-scope `mut`; no runnable Rust doctests (G10).

### Documentation & Deployment
- [ ] Mode-A doc-comment on `classify_devices` records the per-candidate mechanism (filter-narrow-by-vid/pid) + its §4.3 limitation + cites `spec/DEVICE_DISCOVERY.md` §2.
- [ ] No `docs/*.md` or README changes this task (Mode A — P4.M1/P4.M2 own user-facing docs).

---

## Anti-Patterns to Avoid

- ❌ Do NOT inline all classification logic into `classify_devices` and then hand-wave the MockNotifier
      tests. The core MUST be factored into `classify_candidates(candidates: Vec<Candidate>, verbose)` so
      the tests are deterministic (G1 — REQUIRED, the item's test mandate is unsatisfiable otherwise).
- ❌ Do NOT add `feature_flags & 0x01` to `classify_reply`. §2.2 classifies on `proto_ver==2` alone; the
      feature bit is the handshake's gate for the host-rules SEND, not the classifier's. The classifier
      RECORDS feature_flags for the consumer (G2).
- ❌ Do NOT use `d.path().to_str().unwrap()`. `path()` returns `&CStr`; use
      `d.path().to_string_lossy().to_string()` (G3 — `.to_str().unwrap()` panics on non-UTF8 paths).
- ❌ Do NOT hold one `get_notifier().lock()` across all candidates. Acquire per-candidate (short scope),
      mirroring the callback sweep's per-iteration re-acquire — a single held lock starves `notify_qmk`/the
      debounce flush (G5).
- ❌ Do NOT write `get_notifier().lock().send_command(...)` literally — `lock()` returns `LockResult`; use
      `.lock().unwrap()` (G6, matches perform_handshake @435).
- ❌ Do NOT key/narrow the per-candidate ping by path. The crate has NO per-path send (external_deps.md);
      narrow the FILTER by vid/pid (the sole app-side knob) and DOCUMENT the multi-same-vid/pid limitation
      (G4, DEVICE_DISCOVERY.md §4.3).
- ❌ Do NOT touch `MOCK_RESPONSES` / `MOCK_SEND_COMMAND_ERRORS` / `MOCK_SEND_DELAY` / the default
      `Ack{ok:true}` fallback. The FIFO queue ALREADY gives per-candidate ordering (candidate i ⇔ response i).
      The ONLY mock change is adding `MOCK_SEND_COMMAND_FILTERS` (+ clear + push + getter) and renaming
      `_filter`→`filter` (G8).
- ❌ Do NOT record `DeviceFilter` in the mock (would require adding `Clone`/`PartialEq` to a production
      struct). Record the 4-tuple `(Option<u16>, Option<u16>, u16, u16)` (G8).
- ❌ Do NOT alter the handshake's dedup (`HAS_HANDSHAKED`), `SET_OS`, the callback sweep, or the
      `BOARD_HAS_RULES`-before-`HOST_CAPABLE` ordering. The cross-feed ADDS one `warm_cache_from_handshake`
      call per arm, AFTER the existing set/clear (G14).
- ❌ Do NOT add cache assertions to the existing handshake tests. Under MockNotifier there is no real HID,
      so `warm_cache_from_handshake` stamps nothing — the existing tests must pass UNCHANGED (G9).
- ❌ Do NOT write runnable Rust doctests (` ``` `) with `qmkonnect::` paths. Binary-only crate (no lib.rs);
      doctests don't run under `--bin` (G10). Mode-A doc-comments are prose citing `spec/DEVICE_DISCOVERY.md` §2.
- ❌ Do NOT run tests multi-threaded — `cargo test --bin qmkonnect -- --test-threads=1` (shared MockNotifier
      globals + DebounceState, G7/AGENTS.md).
- ❌ Do NOT touch the write path (P2 DEFER), `device_status()` (P1 Complete), the picker (P3.M2), CLI flags
      (P4), Cargo.toml, the crate, or any `docs/*.md` (P4). This task = classify_devices + cache mechanics
      + handshake cross-feed + mock extension + tests (G13).
- ❌ Do NOT redefine S1's `DeviceKind`/`ClassifiedDevice`/`CLASSIFICATION_CACHE`/helpers (G0 — merge conflict).
- ❌ Do NOT edit `PRD.md`, any `tasks.json`, `prd_snapshot.md`, `spec/DEVICE_DISCOVERY.md`, `Cargo.toml`,
      or any `plan/` file other than this item's own `PRP.md` + `research/`.

---

## Confidence Score: 9/10

This is a well-bounded **logic + testable-core** task whose design is forced by two
hard constraints I verified by reading the code: (1) the crate has **no per-path
send** (`external_deps.md` — MatchKey private + filter-keyed), so the per-candidate
mechanism is **filter-narrow-by-vid/pid** with the documented §4.3 limitation; and
(2) `classify_devices` enumerates **real HID** (`HidApi::new()`), so the per-
candidate core MUST be **factored out** (`classify_candidates` takes `Vec<Candidate>`)
to satisfy the item's MockNotifier test mandate — this factoring is not optional
(G1). Every referenced line is confirmed: the enumerate mirror of
`is_device_connected` @216-232, the 4 handshake arms @444/576/588/600, the
MockNotifier @1440 + `MOCK_*` @1414-1426 + `send_command` @1497 + `reset_global_mock`
@1428 + `reset_test_state` @1510, the `DeviceInfo::path()->&CStr` gotcha (hidapi
2.6.3 lib.rs:358), and the crate's `CommandResponse` variants (rev f26893e lib.rs:86-115).
The §2.2 classifier (`Info{proto_ver:2}`⇒Capable, no feature-bit gate — G2) is given
verbatim. The minimal mock extension (record filter tuples, G8) lets the tests
assert the ACTUAL narrowing mechanism, not just abstract logic. The handshake
cross-feed is a no-op under MockNotifier (G9), so the 30+ existing handshake tests
pass unchanged — the highest-risk integration is provably safe. The 1-point
reservation is for: (a) the **parallel-execution file state** (G0 — S1 may not have
landed; the build fails on the missing types until it does, expected); (b) the
precise placement of the `warm_cache_from_handshake` call in the Capable arm
(the arm holds the notifier lock `n` until `drop(n)` before the sweep — the call
must go in a lock-free region, which the arm's tail satisfies; an implementer who
places it before `drop(n)` would deadlock under the nested `get_notifier().lock()`
in `classify_candidates` — but `warm_cache_from_handshake` does NOT take the
notifier lock, only the cache lock, so this is actually safe anywhere; flagged for
clarity); and (c) the `HashSet` import decision (G12 — `Vec::contains` avoids it,
one-token edit either way). All three are low-risk and caught by the build/tests.
Scope is cleanly bounded from the write path (P2 DEFER), the picker (P3.M2),
device_status (P1), CLI (P4), and S1's structs (untouched, only called).
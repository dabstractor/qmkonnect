# PRP — P3.M1.T1.S1: Add DeviceKind / ClassifiedDevice types + CLASSIFICATION_CACHE infrastructure

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This task is a **types-and-infrastructure
> addition** to `src/core/notifier.rs` — the data model + a TTL cache + 3 cache
> helpers that P3.M1.T1.S2's `classify_devices()` will populate/consume. **No
> `classify_devices()` logic, no hidapi enumeration, no `send_command` calls**
> (all S2). It is the first subtask of P3.M1 "classify_devices() + cache"
> (the F13 discovered-device-picker milestone). Source of truth for the types:
> **`spec/DEVICE_DISCOVERY.md` §2** (the Capability Probe). It is consumed
> downstream by **S2** (`classify_devices`) → **P3.M2** (the three-platform
> picker) and is the per-device complement of the already-complete
> `DeviceStatus`/`device_status()` resolver (P1.M1.T1.S1). It **does not touch**
> the in-flight P2.M1.T1.S2 (which edits `architecture/write_narrowing_decision.md`
> only — zero `.rs` overlap).

---

## Goal

**Feature Goal**: Add to `src/core/notifier.rs` the **per-device capability
classification data model** (`pub enum DeviceKind`, `pub struct ClassifiedDevice`)
and its **TTL cache infrastructure** (`const CLASSIFICATION_TTL`, `static
CLASSIFICATION_CACHE`, and `classification_cache_get` / `classification_cache_insert`
/ `classification_cache_clear` helpers). These are the exact types/cache S2's
`classify_devices()` populates and the picker (P3.M2) + status (P1) read. The task
ships **types + cache + helpers + Mode-A rustdoc + tests**; it compiles cleanly and
introduces **no behavior change** (nothing calls the cache yet — S2 will).

**Deliverable**: additions to `src/core/notifier.rs` (a single new, clearly-delimited
section), specifically:
1. `pub enum DeviceKind { Capable { proto_ver: u8, feature_flags: u8, callback_count: u8, board_rules_present: bool }, NotQmkNotifier }` (derives `Debug, Clone, PartialEq`).
2. `pub struct ClassifiedDevice { path: String, vendor_id: u16, product_id: u16, product_name: Option<String>, usage_page: u16, usage: u16, kind: DeviceKind }` (derives `Debug, Clone, PartialEq`).
3. `const CLASSIFICATION_TTL: Duration = Duration::from_secs(5);`.
4. `static CLASSIFICATION_CACHE: Lazy<Mutex<HashMap<String, (DeviceKind, Instant)>>> = Lazy::new(|| Mutex::new(HashMap::new()));` (private; keyed by hidapi `path`).
5. `pub fn classification_cache_get(path: &str) -> Option<DeviceKind>` (TTL-checked, lazy, non-evicting).
6. `pub fn classification_cache_insert(path: &str, kind: DeviceKind)`.
7. `pub fn classification_cache_clear()`.
8. Mode-A doc-comments on each item citing `spec/DEVICE_DISCOVERY.md` §2.
9. A `#[cfg(test)]` block of ~7 cache tests (no HID mock needed — the helpers are pure).

**Success Definition**:
- `cargo build --bin qmkonnect` compiles clean (no new warnings).
- `classification_cache_insert("p", DeviceKind::Capable{..})` then
  `classification_cache_get("p")` returns `Some(DeviceKind::Capable{..})` with equal fields.
- `classification_cache_get` returns `None` for an unseen path, for an expired entry
  (Instant stamped past-TTL), and after `classification_cache_clear()`.
- `DeviceKind`/`ClassifiedDevice` derive `PartialEq` + `Clone` (asserted in tests).
- `cargo test --bin qmkonnect -- --test-threads=1` is green (new
  `test_classification_cache_*` tests + all 49 existing tests; no regression).
- `git status` shows exactly ONE modified file: `src/core/notifier.rs`.
- No `classify_devices()` / `send_command` / `HidApi::new()` added to the production
  path (S2's job).

## User Persona (if applicable)

**Target User**: three downstream consumers (none implemented this task):
- **P3.M1.T1.S2** (`classify_devices()`): the per-candidate Tier-2 probe — for each
  Tier-1 HID interface, narrows `configured_filter()` to the candidate's vid/pid,
  sends `QUERY_INFO` via `send_command`, matches the reply into `DeviceKind`, calls
  `classification_cache_insert`, collects `ClassifiedDevice`.
- **P3.M2.T1** (the discovered-device picker on Win32/macOS/Linux): renders a
  `Vec<ClassifiedDevice>` as the settings UI (VID/PID/product + a "kind" column).
- **P1's `device_status()` resolver** (already Complete): the aggregate three-state
  status is *conceptually* a fold over the per-device `DeviceKind`s; this task's
  types are the per-device complement of that global resolver.

**Use Case**: a user opens Settings → the picker calls `classify_devices()` (S2) →
gets `Vec<ClassifiedDevice>` → renders each row with its `kind` (`Capable` ⇒
"qmk_notifier ✓", `NotQmkNotifier` ⇒ "QMK board, no module"). The cache means the
hot status-poll thread (3s/1s) does NOT re-ping on every tick — only on device
appearance or TTL expiry.

**Pain Points Addressed**: today the app only knows *aggregate* presence
(`is_device_connected`) + *aggregate* capability (`host_capable`). It cannot tell
the user *which* board is capable, nor distinguish a pure-VIA board from a
qmk_notifier board in the picker. These types + cache are the foundation for that.

## Why

- **DEVICE_DISCOVERY.md §2 mandates a per-candidate Tier-2 probe.** §2.3 names
  `ClassifiedDevice` (with these exact fields) + a `CLASSIFICATION_CACHE` keyed by
  path with a 5s TTL; §2.2 defines the `QUERY_INFO` → `CommandResponse::Info`
  match that produces `DeviceKind`. This task ships the data model §2 specifies.
- **Unblocks S2 + the picker (P3.M2) + the three-state status's per-device view.**
  S2 cannot be written without `DeviceKind`/`ClassifiedDevice` + the cache helpers.
- **Separates "data model + cache" from "probe logic" for review clarity.** S1 is a
  small, pure, easily-tested delta; S2 adds the hidapi + `send_command` surface
  (which needs the `MockNotifier` test infra). Keeping them split makes each
  reviewable and lets the picker's type contract stabilize first.
- **Consistent with the locked multi-board decision (§4).** The cache is keyed by
  `path` (per-device identity), not by filter — so broadcast-to-all-capable (§4.1)
  and per-device display (§5) both read the same per-path cache.

## What

An additive edit to `src/core/notifier.rs` — one new, contiguous, banner-delimited
section. The 9 items above. Mirrors existing in-file conventions exactly:
`CALLBACK_NAMES` (Lazy<Mutex<HashMap>>) for the static, `CALLBACK_SWEEP_DEADLINE`
(Duration const) for the TTL, the private-static + pub-reader/writer convention
(`HOST_CAPABLE`/`host_capable()`). **No new `use` lines** (Lazy, Mutex, HashMap,
Duration, Instant are all already imported at lines 2-9). **No serde, no Cargo change.**

### Success Criteria
- [ ] `pub enum DeviceKind { Capable { proto_ver: u8, feature_flags: u8, callback_count: u8, board_rules_present: bool }, NotQmkNotifier }` with `#[derive(Debug, Clone, PartialEq)]`.
- [ ] `pub struct ClassifiedDevice { path: String, vendor_id: u16, product_id: u16, product_name: Option<String>, usage_page: u16, usage: u16, kind: DeviceKind }` with `#[derive(Debug, Clone, PartialEq)]` and all fields `pub`.
- [ ] `const CLASSIFICATION_TTL: Duration = Duration::from_secs(5);`.
- [ ] `static CLASSIFICATION_CACHE: Lazy<Mutex<HashMap<String, (DeviceKind, Instant)>>>` (private), keyed by path.
- [ ] `pub fn classification_cache_get(path: &str) -> Option<DeviceKind>` — returns `None` if absent OR older than `CLASSIFICATION_TTL`; clones out the `DeviceKind` on a fresh hit; does NOT eagerly evict.
- [ ] `pub fn classification_cache_insert(path: &str, kind: DeviceKind)` — stamps `Instant::now()`, inserts/overwrites.
- [ ] `pub fn classification_cache_clear()` — drains the map.
- [ ] Mode-A doc-comments cite `spec/DEVICE_DISCOVERY.md` §2 (esp. §2.2, §2.3, §2.4); no runnable Rust doctests.
- [ ] ~7 `test_classification_cache_*` tests pass (insert/get, miss, clear, overwrite, TTL expiry, NotQmk variant, derive sanity).
- [ ] No `classify_devices()` / `send_command` / `HidApi::new()` added to production code.
- [ ] `cargo build --bin qmkonnect` clean; `cargo test --bin qmkonnect -- --test-threads=1` green; `git status` = 1 file.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can implement this using only this PRP,
because (a) the exact types are given verbatim from DEVICE_DISCOVERY.md §2.3
(reproduced in `research/notes.md` §1); (b) the capable-classifier contract
(`CommandResponse::Info` field-for-field) is pinned to crate rev `f26893e`
(`research/notes.md` §2); (c) every in-file pattern to mirror is named with its
file:line and exact form (`research/notes.md` §3); (d) the 3 helper signatures +
the TTL-check body are given verbatim (`research/notes.md` §4); (e) placement is
decided (a new banner section after the `device_status()` resolver region, §5);
(f) the relationship to already-complete `DeviceStatus` (P1) + in-flight P2.M1.T1.S2
(zero overlap) is documented (§6) so the implementer avoids collisions; (g) 10
gotchas are enumerated (§7 — incl. the binary-only-crate doctest constraint G1, the
crate-has-no-per-path-send constraint G3, and the must-derive-Clone G5); (h) the
~7-test set is specified (§8); (i) the scope wall is explicit (§9).

### Documentation & References

```yaml
# MUST READ — the schema source of truth (verbatim types + cache contract)
- url: spec/DEVICE_DISCOVERY.md
  why: "§2.2 (the QUERY_INFO -> CommandResponse::Info match that defines Capable),
        §2.3 (the ClassifiedDevice struct + the CLASSIFICATION_CACHE keyed by path,
        5s TTL, 'so the hot path does not re-ping on every status poll'), §2.4 (the
        relationship to perform_handshake — same QUERY_INFO tx, per-candidate vs once-per-boot).
        §4.1/§4.2 (multi-board broadcast reads the capable set). §5 (the picker renders
        Vec<ClassifiedDevice>). §8 (file-level implementation map names notifier.rs)."
  section: "## 2. The Capability Probe" (§2.1-§2.4) and "## 4. Multi-Board Policy"

# MUST READ — the verbatim types + mirror patterns + helper bodies (THIS task's contract)
- file: plan/005_8b95ea464bd9/P3M1T1S1/research/notes.md
  why: "§1 the exact DeviceKind/ClassifiedDevice (verbatim from §2.3 + derives).
        §2 the CommandResponse::Info contract (crate rev f26893e, lib.rs:95-99) — proves
        Capable mirrors the reply field-for-field. §3 the in-file patterns to mirror
        (CALLBACK_NAMES @276, CALLBACK_SWEEP_DEADLINE @~412, HOST_CAPABLE/host_capable).
        §4 the 3 helper signatures + the TTL-check body (lazy, non-evicting). §5 placement.
        §7 the 10 gotchas. §8 the ~7-test set. §9 the scope wall."

# MUST READ — the file THIS task edits
- file: src/core/notifier.rs
  why: "imports already present (Lazy@2, HashMap@3, Mutex@7, Duration/Instant@9 — NO new
        `use` lines). DeviceFilter @67-72 (the vid/pid/page/usage struct S2 narrows). The
        statics cluster @270-299 (HOST_CAPABLE, CALLBACK_NAMES, HAS_HANDSHAKED,
        STARTUP_DEVICE_CONNECTED, RULES_INVALID_NOTIFIED) — MIRROR CALLBACK_NAMES for the
        cache. The Duration-const pattern CALLBACK_SWEEP_DEADLINE @~412 — MIRROR for TTL.
        host_capable() @689 + board_has_rules()/BOARD_HAS_RULES @1146-1151 — MIRROR the
        private-static + pub-reader convention. DeviceStatus @719 + device_status() @761 +
        classify_device_status() test helper @2960 (P1, COMPLETE) — DO NOT TOUCH, cite as the
        aggregate complement. The MockNotifier test infra @1278-1370 + reset_test_state()
        (S2 will use it; THIS task's cache tests are pure, no mock needed)."
  pattern: "bannered section comments (`// ===== ... =====`), private statics + pub fns,
            doc-comments cite the spec section. tests grouped with `// --- header ---`."
  gotcha: "PLACEMENT: a new `// ===== Device classification (P3.M1) =====` section AFTER the
           device_status() resolver region (~line 778). Keep the 9 items contiguous. Do NOT
           scatter into the existing statics cluster."

# MUST READ — the crate contract (proves Capable mirrors the reply; pins the no-per-path-send constraint)
- file: plan/005_8b95ea464bd9/architecture/external_deps.md
  why: "CommandResponse::Info { proto_ver: u8, feature_flags: u8, callback_count: u8,
        board_rules_present: bool } (the capable classifier). The CRITICAL CONSTRAINT: there
        is NO per-path send in the crate (MatchKey is private + filter-keyed @core.rs:641) —
        so the cache is keyed by PATH (stable per-interface identity) while S2 narrows the
        FILTER by vid/pid. THIS task adds no send; it just establishes the path-keyed cache."

# MUST READ — the existing mechanism map (cite in rustdoc; confirms no new pinging for status)
- file: plan/005_8b95ea464bd9/architecture/notifier_mechanisms.md
  why: "is_device_connected() @216 (Tier-1, the enumeration S2 will mirror with .filter()),
        HOST_CAPABLE/host_capable() @270/689 (the GLOBAL capability, set by perform_handshake),
        perform_handshake_with @421 (the existing QUERY_INFO tx — S2's per-candidate probe is
        the same tx). The three-state derivation table (Disconnected/NoModule/Connected) — my
        DeviceKind is the per-device view that AGGREGATES into that global status."

# MUST READ — the in-flight parallel task (ZERO overlap; cite its DEFER verdict)
- file: plan/005_8b95ea464bd9/P2M1T1S2/PRP.md
  why: "confirms P2.M1.T1.S2 edits architecture/write_narrowing_decision.md ONLY — ZERO .rs
        overlap with this task. Its DEFER verdict (write-narrowing needs a crate change) is
        CONSISTENT with this task: my classification types feed the PICKER/STATUS, not the
        write path. Do NOT add any write-narrowing in this task (or S2)."

# Reference — the crate's public enum (read-only, pinned rev)
- file: ~/.cargo/git/checkouts/qmk-notifier-1f15950b695d9922/f26893e/src/lib.rs
  why: "lines 95-99 are the Info variant (proto_ver/feature_flags/callback_count/
        board_rules_present) — the exact field-for-field source of DeviceKind::Capable.
        Confirm the rev via `grep -A3 'name = \"qmk-notifier\"' Cargo.lock` (expect #f26893e)."

# Reference — serde field/container attrs are NOT needed here (these are runtime types)
- url: https://serde.rs/field-attrs.html
  why: "(only if a reviewer asks) these types intentionally carry NO serde derives — they are
        HID runtime values, never serialized to config.toml. serde is a config-only dep here."
```

### Current Codebase tree (relevant subset)

```bash
src/
  main.rs              # `mod core;` (binary-only crate — NO lib.rs; see G1)
  core/
    notifier.rs        # Notifier trait + QmkNotifier; DeviceFilter @67; configured_filter @83;
                         #   list_devices @129; is_device_connected @216; HOST_CAPABLE @270;
                         #   CALLBACK_NAMES @276 (Lazy<Mutex<HashMap>> pattern); CALLBACK_SWEEP_DEADLINE @~412;
                         #   perform_handshake_with @421; host_capable @689; DeviceStatus @719;
                         #   device_status @761; BOARD_HAS_RULES @1146; MockNotifier @1278;
                         #   #[cfg(test)] mod tests (49 fns, --test-threads=1)
                         #   <-- EDIT THIS FILE: add ONE new banner section
    types.rs / mod.rs / pattern.rs / rules.rs   # unchanged
Cargo.toml             # once_cell 1.21, hidapi 2.6, qmk-notifier v0.3.0 (rev f26893e) — UNCHANGED
spec/DEVICE_DISCOVERY.md   # §2 = the schema source of truth (READ-ONLY)
```

### Desired Codebase tree (files this task changes)

```bash
src/
  core/
    notifier.rs        # MODIFIED (additive) — + // ===== Device classification (P3.M1) ===== section
                         #                     + pub enum DeviceKind + pub struct ClassifiedDevice
                         #                     + const CLASSIFICATION_TTL + static CLASSIFICATION_CACHE
                         #                     + 3 pub cache helpers + Mode-A doc-comments
                         #                     + ~7 test_classification_cache_* tests in #[cfg(test)]
    # EVERYTHING else unchanged (Cargo.toml, types.rs, mod.rs, pattern.rs, rules.rs, tray*.rs, ...)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (G1 — binary-only crate; doctests don't run under `--bin`): there is NO
//   lib.rs (`src/main.rs:3` is `mod core;`). The AGENTS.md command
//   `cargo test --bin qmkonnect` runs UNIT TESTS ONLY, not doctests. A runnable Rust
//   doctest (` ``` ` block) doing `use qmkonnect::core::notifier::DeviceKind;` would
//   NOT compile under `cargo test --doc` (no library target). MITIGATION: Mode-A
//   doc-comments here are PROSE citing spec/DEVICE_DISCOVERY.md §2 — NOT runnable
//   ` ``` ` Rust doctests. If a code sketch is wanted, use ` ```text ` / ` ```ignore `.
//
// CRITICAL (G2 — NO logic in this task): do NOT implement classify_devices(), do NOT
//   call send_command, do NOT HidApi::new() in production code, do NOT change
//   perform_handshake. Only the 9 items (2 types + 1 const + 1 static + 3 helpers +
//   doc-comments + tests). S2 owns the probe logic.
//
// CRITICAL (G3 — crate has NO per-path send; cache is keyed by PATH): external_deps.md
//   records that MatchKey is private + filter-keyed (core.rs:641) and the only sends
//   (run/send_raw_report) broadcast to ALL filter-matching devices. So S2 narrows the
//   FILTER to a candidate's vid/pid (the only app-side mechanism). THIS task's cache is
//   keyed by hidapi `path` (DeviceInfo::path() — the stable per-interface identity)
//   because the picker/status care WHICH physical device is capable. Do NOT key by vid/pid.
//
// GOTCHA (G4 — single-threaded tests crate-wide): `cargo test --bin qmkonnect --
//   --test-threads=1` (shared MockNotifier globals + DebounceState, AGENTS.md). The cache
//   tests should each START with `classification_cache_clear()` to avoid cross-test bleed
//   (the static outlives tests).
//
// CRITICAL (G5 — DeviceKind MUST derive Clone): classification_cache_get returns an OWNED
//   `Option<DeviceKind>`, so it clones the cached value out. DeviceKind::Capable holds only
//   Copy fields (u8/u8/u8/bool) so Clone is trivial; ClassifiedDevice also Clone (picker
//   clones rows). Do NOT make the cache value a reference or return `&DeviceKind`.
//
// GOTCHA (G6 — no name collision with DeviceStatus): P1.M1.T1.S1 (Complete) ships
//   `enum DeviceStatus { Connected, NoModule, Disconnected }` (the aggregate 3-state) +
//   `device_status()` + the `classify_device_status(present, capable)` test helper (@2960).
//   My `enum DeviceKind { Capable{..}, NotQmkNotifier }` is the PER-DEVICE classification.
//   Distinct names, distinct semantics. Do NOT rename either; do NOT touch DeviceStatus.
//
// GOTCHA (G7 — do NOT touch the GLOBAL capability path): HOST_CAPABLE @270, host_capable()
//   @689, BOARD_HAS_RULES @1146, board_has_rules() @1150, CALLBACK_NAMES @276,
//   perform_handshake_with @421 are the GLOBAL capability (set once per boot by the
//   handshake). My CLASSIFICATION_CACHE is the PER-DEVICE path (populated by S2 per
//   candidate). They coexist; do NOT wire them together or modify the handshake.
//
// GOTCHA (G8 — Mode A docs only): doc-comments cite spec/DEVICE_DISCOVERY.md §2 (§2.2 the
//   QUERY_INFO->Info match, §2.3 the struct+cache, §2.4 the handshake relationship). NO
//   docs/*.md or README edits (that is P4.M1/P4.M2).
//
// GOTCHA (G9 — imports already present; NO new `use`): notifier.rs:1-9 already imports
//   `once_cell::sync::Lazy`, `std::collections::{BTreeSet, HashMap}`,
//   `std::sync::{Arc, Condvar, Mutex, OnceLock}`, `std::time::{Duration, Instant}`. Adding a
//   duplicate `use` is an unused-import warning. Use them as-is (fully-qualified paths are
//   already in scope: `Lazy`, `Mutex`, `HashMap`, `Duration`, `Instant`).
//
// GOTCHA (G10 — no serde, no Cargo change): these are runtime HID types. serde/toml are
//   config-only deps. Do NOT add serde derives; do NOT edit Cargo.toml. once_cell (Lazy) is
//   already a dep (1.21).
//
// CRATE QUIRK: the crate-wide test command MUST be single-threaded:
//   cargo test --bin qmkonnect -- --test-threads=1   (AGENTS.md)
```

## Implementation Blueprint

### Data models and structure

Two types (verbatim from `spec/DEVICE_DISCOVERY.md` §2.3) + the cache. See
`research/notes.md` §1/§3/§4 for the mirror patterns.

```rust
/// Per-device capability classification, the result of the Tier-2 probe
/// (`spec/DEVICE_DISCOVERY.md` §2.2). `Capable` mirrors the crate's
/// `CommandResponse::Info { proto_ver: 2, .. }` reply field-for-field; every
/// other reply (Legacy / Timeout / error) classifies as `NotQmkNotifier`.
///
/// See [`DeviceStatus`] for the AGGREGATE three-state tray status that folds
/// over a set of these per-device kinds (via S2's `classify_devices`).
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceKind {
    /// The board advertised `proto_ver == 2` + the host-rules feature bit.
    /// Fields mirror `qmk_notifier::CommandResponse::Info` (crate rev f26893e).
    Capable {
        proto_ver: u8,
        feature_flags: u8,
        callback_count: u8,
        board_rules_present: bool,
    },
    /// Tier-1-present but not qmk_notifier-capable (pure VIA, legacy, silent).
    NotQmkNotifier,
}

/// One enumerated Tier-1 HID interface (`usage_page == 0xFF60 && usage == 0x61`)
/// plus its Tier-2 classification. `path` is the stable hidapi path and the
/// `CLASSIFICATION_CACHE` key. Returned by `classify_devices()` (P3.M1.T1.S2)
/// and rendered by the discovered-device picker (P3.M2).
///
/// `spec/DEVICE_DISCOVERY.md` §2.3 / §5.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassifiedDevice {
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub product_name: Option<String>,
    pub usage_page: u16,
    pub usage: u16,
    pub kind: DeviceKind,
}

/// TTL for [`CLASSIFICATION_CACHE`] entries. Default 5s so the hot status-poll
/// thread (macOS/Windows 3s, Linux 1s) does not re-ping on every tick
/// (`spec/DEVICE_DISCOVERY.md` §2.3). Mirrors the `CALLBACK_SWEEP_DEADLINE`
/// Duration-const idiom.
const CLASSIFICATION_TTL: Duration = Duration::from_secs(5);

/// Per-device classification cache, keyed by stable hidapi `path`. Value is
/// `(DeviceKind, Instant)` where the `Instant` stamps when it was classified for
/// the TTL check. Populated by `classify_devices()` (S2); read by the picker
/// (P3.M2) + the status resolver. Mirrors the `CALLBACK_NAMES`
/// `Lazy<Mutex<HashMap>>` idiom. PRIVATE — access via the 3 helpers below.
static CLASSIFICATION_CACHE: Lazy<Mutex<HashMap<String, (DeviceKind, Instant)>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: ADD the new banner section header + DeviceKind + ClassifiedDevice types
  - DO: insert a new `// ===== Device classification (P3.M1) — per-candidate capability tier =====`
        banner section AFTER the `device_status()` resolver region (~line 778; acceptable
        alternative: after the statics cluster @~299 — pick ONE, keep contiguous).
  - ADD: the two types EXACTLY as in the Data-models block above (derives Debug, Clone,
    PartialEq; all ClassifiedDevice fields pub; DeviceKind::Capable carries the 4 Info fields).
  - GOTCHA G1/G8: Mode-A doc-comments are PROSE citing spec/DEVICE_DISCOVERY.md §2 — NO
    runnable ``` Rust doctests. GOTCHA G5: derives include Clone. GOTCHA G6: do NOT rename
    DeviceStatus. GOTCHA G9: no new `use` lines.
  - PLACEMENT: src/core/notifier.rs, the new banner section.

Task 2: ADD the CLASSIFICATION_TTL const + CLASSIFICATION_CACHE static (same section)
  - DO: add `const CLASSIFICATION_TTL: Duration = Duration::from_secs(5);` (mirror
        CALLBACK_SWEEP_DEADLINE @~412) and
        `static CLASSIFICATION_CACHE: Lazy<Mutex<HashMap<String, (DeviceKind, Instant)>>> =
         Lazy::new(|| Mutex::new(HashMap::new()));` (mirror CALLBACK_NAMES @276).
  - DOC-COMMENT both (cite §2.3: "cache keyed by path with a 5s TTL so the hot path does
    not re-ping on every status poll").
  - GOTCHA G3: keyed by PATH (String), value (DeviceKind, Instant). GOTCHA G7: the static
    is PRIVATE (no `pub`) — like HOST_CAPABLE/CALLBACK_NAMES. GOTCHA G10: no serde, no Cargo.

Task 3: ADD the 3 cache helpers (same section, after the static)
  - DO: add `classification_cache_get`, `classification_cache_insert`,
        `classification_cache_clear` — all `pub`. Bodies (research/notes.md §4):
        // get: lock ok?; get path?; if stamped.elapsed() < TTL { Some(kind.clone()) } else { None }
        // insert: map.insert(path.to_string(), (kind, Instant::now()))
        // clear:  map.clear()
  - SIGNATURES:
        pub fn classification_cache_get(path: &str) -> Option<DeviceKind>
        pub fn classification_cache_insert(path: &str, kind: DeviceKind)
        pub fn classification_cache_clear()
  - GOTCHA G5: get CLONES the DeviceKind out (returns owned). GOTCHA: get is LAZY +
        NON-EVICTING (returns None for expired but leaves the stale entry; S2's insert
        overwrites). Do NOT call Instant::now() at module load. Handle the poisoned-lock
        case: get returns None on `lock().ok()?`; insert/clear use `.lock().unwrap()`
        (panic on poison — matches the existing CALLBACK_NAMES/HOST_CAPABLE access style;
        the crate has no poison-recovery policy).
  - DOC-COMMENT each (cite §2.3 cadence + §2.4 handshake relationship).

Task 4: ADD ~7 cache tests to the existing #[cfg(test)] mod tests block
  - DO: append tests (prefix `test_classification_cache_`) to the existing test module
        (which uses `use super::*;`). Each test STARTS with `classification_cache_clear()`
        (G4 — avoid cross-test bleed). Cover (research/notes.md §8):
        1. test_classification_cache_insert_then_get — Capable round-trips (assert field equality).
        2. test_classification_cache_miss — unseen path -> None.
        3. test_classification_cache_clear — insert, clear, get -> None.
        4. test_classification_cache_overwrite — Capable then NotQmk same path -> get NotQmk.
        5. test_classification_cache_ttl_expiry — insert, then REWRITE the stored Instant to
           `Instant::now() - CLASSIFICATION_TTL - Duration::from_millis(1)` via
           `CLASSIFICATION_CACHE.lock().unwrap().insert(path, (kind, past))`, then get -> None.
           (Reaching into the static from the same-module test is fine.)
        6. test_classification_cache_notqmk_variant — round-trip the unit variant (Clone sanity).
        7. test_devicekind_classifieddevice_derives — assert PartialEq + Clone produce equal
           values for both DeviceKind variants and a ClassifiedDevice.
  - NO HID MOCK needed — the helpers are pure (lock a static HashMap). Do NOT use
    MockNotifier/set_notifier for these (that's S2's classify_devices tests).
  - NAMING: `test_classification_cache_*` + `test_devicekind_*` (disjoint from the 49
    existing `test_*` / `r_coex_*`).

Task 5: VERIFY build + tests (single-threaded, per AGENTS.md)
  - RUN: cargo build --bin qmkonnect          (expect clean; no NEW warnings — esp. no
        unused-import from a stray duplicate `use`)
  - RUN: cargo test --bin qmkonnect classification_cache -- --test-threads=1
        (expect: all ~7 new tests pass)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1
        (expect: full crate green — new tests + 49 existing + pattern/rules/types; no regression)
  - CONFIRM git status shows EXACTLY one file: src/core/notifier.rs.
  - IF a test fails: re-read research/notes.md §4 (the TTL-check body) + §7 gotchas. The
        helpers are the spec; a failure = a transcription slip (wrong TTL comparison, missing
        Clone, poisoned-lock mishandling). Do NOT "fix" the test to match divergent behavior.
```

### Implementation Patterns & Key Details

```rust
// The canonical helper bodies (this IS the spec — match it; research/notes.md §4).
//
// /// Look up a cached classification. `None` if absent OR older than the TTL.
// pub fn classification_cache_get(path: &str) -> Option<DeviceKind> {
//     let map = CLASSIFICATION_CACHE.lock().ok()?;          // poisoned lock -> None
//     let (kind, stamped) = map.get(path)?;
//     if stamped.elapsed() < CLASSIFICATION_TTL {
//         Some(kind.clone())                                // fresh -> clone out
//     } else {
//         None                                              // stale -> miss (lazy, no evict)
//     }
// }
//
// /// Record/refresh a classification (stamps now).
// pub fn classification_cache_insert(path: &str, kind: DeviceKind) {
//     if let Ok(mut map) = CLASSIFICATION_CACHE.lock() {
//         map.insert(path.to_string(), (kind, Instant::now()));
//     }
// }
//
// /// Drop all entries (device-loss / reload-rules).
// pub fn classification_cache_clear() {
//     if let Ok(mut map) = CLASSIFICATION_CACHE.lock() {
//         map.clear();
//     }
// }
//
// NOTE: insert/clear swallow a poisoned lock silently (no-op). get returns None on poison.
// This is the simplest correct behavior; the crate has no poison-recovery policy, and a
// panic here would be disproportionate for a cache. (If a reviewer prefers .unwrap(), that
// matches HOST_CAPABLE/CALLBACK_NAMES access style too — either is defensible; the PRP
// permits both. Pick the non-panicking form to keep the daemon alive under a panic-in-test.)
//
// The TTL-expiry TEST reaches into the static directly (same module):
//   CLASSIFICATION_CACHE.lock().unwrap().insert(
//       "p".to_string(),
//       (DeviceKind::Capable { proto_ver:2, feature_flags:1, callback_count:0, board_rules_present:false },
//        Instant::now() - CLASSIFICATION_TTL - Duration::from_millis(1)),
//   );
//   assert_eq!(classification_cache_get("p"), None);
```

### Integration Points

```yaml
CODE (this task):
  - file: src/core/notifier.rs
    change: "additive — one new banner section: 2 types + 1 const + 1 static + 3 pub helpers + tests"
    pattern: "mirrors CALLBACK_NAMES (Lazy<Mutex<HashMap>>) + CALLBACK_SWEEP_DEADLINE (Duration const) +
              HOST_CAPABLE/host_capable (private static + pub reader)"

DEPENDENCIES (this task): NONE new. once_cell (Lazy), hidapi, qmk-notifier, std are ALL
                           already Cargo deps. NO serde (runtime types). NO new `use` lines.

UPSTREAM (the contract these types mirror — already present):
  - qmk_notifier::CommandResponse::Info { proto_ver: u8, feature_flags: u8, callback_count: u8,
    board_rules_present: bool } (crate rev f26893e, lib.rs:95-99). DeviceKind::Capable mirrors
    it field-for-field. qmk_notifier::RunCommand::QueryInfo is the probe S2 will send.

DOWNSTREAM CONSUMERS (later subtasks — do NOT implement them here):
  - P3.M1.T1.S2 (classify_devices): HidApi::new() (mirror is_device_connected @216 with
    .filter), per-candidate vid/pid-narrowed filter, send_command(QueryInfo, &narrowed),
    match reply -> DeviceKind, classification_cache_insert, collect Vec<ClassifiedDevice>.
  - P3.M2.T1 (picker on Win32/macOS/Linux): renders Vec<ClassifiedDevice> (the `kind` column).
  - P1 device_status() (already Complete): conceptually a fold over per-device kinds; THIS
    task's types are the per-device complement. No change to device_status.

NO OVERLAP:
  - P2.M1.T1.S1 (Complete): R-COEX comments + tests in notifier.rs — separate section, untouched.
  - P2.M1.T1.S2 (in-flight): edits architecture/write_narrowing_decision.md ONLY — ZERO .rs overlap.

CONFIG: none. ROUTES: none (no CLI surface this subtask — --list-devices kind column is P4.M1.T1.S1).
DATABASE: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles clean, no NEW warnings (esp. no unused-import from a duplicate `use` — G9).
# If rustc errors (missing Clone derive G5, a `pub` slip, a static made `pub`), READ it and fix.

# Confirm the deliverables are present:
grep -n 'pub enum DeviceKind' src/core/notifier.rs          # expect 1
grep -n 'pub struct ClassifiedDevice' src/core/notifier.rs  # expect 1
grep -n 'const CLASSIFICATION_TTL' src/core/notifier.rs     # expect 1
grep -n 'static CLASSIFICATION_CACHE' src/core/notifier.rs  # expect 1 (NOT pub)
grep -nE 'pub fn classification_cache_(get|insert|clear)' src/core/notifier.rs  # expect 3
# Confirm NO classify_devices logic leaked in (G2):
! grep -n 'pub fn classify_devices' src/core/notifier.rs || echo "FAIL: classify_devices added (G2 violation)"
# Confirm NO duplicate imports (G9):
grep -cE '^use (once_cell|std::time|std::sync|std::collections)' src/core/notifier.rs  # expect the existing count, unchanged
```

### Level 2: Unit Tests — cache contract (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# Single-threaded is MANDATORY crate-wide (shared MockNotifier globals + DebounceState, AGENTS.md).
cargo test --bin qmkonnect classification_cache -- --test-threads=1
# Expected: all ~7 test_classification_cache_* / test_devicekind_* pass (insert/get, miss,
# clear, overwrite, TTL expiry, NotQmk variant, derive sanity). Filter to one:
cargo test --bin qmkonnect classification_cache::tests::test_classification_cache_ttl_expiry -- --test-threads=1
cargo test --bin qmkonnect classification_cache::tests::test_classification_cache_overwrite -- --test-threads=1
```

### Level 3: Full-crate regression (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL bin tests green — new classification tests + the 49 existing notifier tests
# (incl. r_coex_invariants from P2.M1.T1.S1) + pattern + rules + types + mod. Proves the
# additive section compiles in the full crate context and didn't disturb the statics cluster
# or the handshake/device_status paths.

# Confirm the change surface is exactly one file:
git status --short
# Expected:
#   modified:   src/core/notifier.rs        (ONLY this)
git diff --stat
# Expected: only src/core/notifier.rs; nothing in Cargo.toml, types.rs, mod.rs, tray*.rs,
# architecture/, docs/, spec/.
```

### Level 4: N/A (infrastructure task — no runtime/HID behavior)

This task adds types + a pure cache + tests. There is no runtime behavior to validate
beyond the unit tests (S2 will add the hidapi/send_command probe path with its own
MockNotifier-backed tests). The Level-2 + Level-3 green run IS the validation gate.

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean (no NEW warnings; no duplicate `use` — G9).
- [ ] `cargo test --bin qmkonnect classification_cache -- --test-threads=1` — all ~7 new tests pass.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` — full crate green (no regression).
- [ ] `git status` shows exactly ONE modified file: `src/core/notifier.rs`.

### Feature Validation (the data model + cache contract)
- [ ] **DeviceKind** is `pub enum { Capable{proto_ver,feature_flags,callback_count,board_rules_present}, NotQmkNotifier }`, derives `Debug, Clone, PartialEq`.
- [ ] **ClassifiedDevice** is `pub struct { path, vendor_id, product_id, product_name, usage_page, usage, kind }`, all fields `pub`, derives `Debug, Clone, PartialEq`.
- [ ] **Capable fields mirror `CommandResponse::Info`** (crate rev f26893e) field-for-field.
- [ ] **CLASSIFICATION_TTL** = `Duration::from_secs(5)` (const).
- [ ] **CLASSIFICATION_CACHE** is a private `Lazy<Mutex<HashMap<String, (DeviceKind, Instant)>>>` keyed by path.
- [ ] **get** returns `None` for absent / expired / poisoned-lock; `Some(cloned kind)` for fresh.
- [ ] **insert** stamps `Instant::now()` and overwrites.
- [ ] **clear** drains the map.
- [ ] **TTL expiry test** passes (rewriting the stored Instant to the past yields `None`).
- [ ] **No classify_devices / send_command / HidApi::new** in production code (G2).

### Code Quality Validation
- [ ] New items in ONE bannered section (`// ===== Device classification (P3.M1) =====`), contiguous.
- [ ] Mirrors `CALLBACK_NAMES` (static), `CALLBACK_SWEEP_DEADLINE` (const), `HOST_CAPABLE`/`host_capable` (private static + pub reader) conventions.
- [ ] NO new `use` lines (G9); NO serde (G10); NO Cargo.toml change.
- [ ] Mode-A doc-comments cite `spec/DEVICE_DISCOVERY.md` §2 (§2.2/§2.3/§2.4); NO runnable Rust doctests (G1).
- [ ] No name collision with `DeviceStatus` (G6); no touch of `perform_handshake`/`HOST_CAPABLE`/`BOARD_HAS_RULES`/`CALLBACK_NAMES` (G7).
- [ ] Cache keyed by **path** (G3), not vid/pid.
- [ ] Tests start with `classification_cache_clear()` (G4); prefix `test_classification_cache_` / `test_devicekind_` (disjoint from 49 existing).

### Documentation & Deployment
- [ ] Mode-A doc-comments present on DeviceKind, ClassifiedDevice, CLASSIFICATION_TTL, CLASSIFICATION_CACHE, and the 3 helpers.
- [ ] Doc-comments cite `spec/DEVICE_DISCOVERY.md` §2 + the crate `CommandResponse::Info` contract + the `device_status()` aggregate relationship.
- [ ] No `docs/*.md` or README changes this task (Mode A — P4.M1/P4.M2 own user-facing docs).

---

## Anti-Patterns to Avoid

- ❌ Do NOT implement `classify_devices()`, call `send_command`, or `HidApi::new()` in
      production code. That is S2's entire job (G2). This task is types + cache + helpers + tests.
- ❌ Do NOT key the cache by vid/pid. It is keyed by hidapi **path** (the stable per-interface
      identity) because the picker/status care WHICH physical device is capable, and the crate
      has no per-path send (G3 — S2 narrows the *filter* by vid/pid instead).
- ❌ Do NOT make `CLASSIFICATION_CACHE` `pub`. Mirror `HOST_CAPABLE`/`CALLBACK_NAMES` — private
      static, `pub` reader/writer helpers (G7).
- ❌ Do NOT omit `Clone` from `DeviceKind`/`ClassifiedDevice`. `classification_cache_get`
      returns an owned `Option<DeviceKind>` (it clones out); the picker clones rows (G5).
- ❌ Do NOT add serde derives. These are runtime HID types, never serialized to config (G10).
- ❌ Do NOT add new `use` lines. `Lazy`/`Mutex`/`HashMap`/`Duration`/`Instant` are already
      imported (notifier.rs:1-9) — a duplicate `use` is an unused-import warning (G9).
- ❌ Do NOT use runnable Rust doctests (` ``` `) with `qmkonnect::` paths. This is a binary-only
      crate (no lib.rs); doctests don't run under `--bin` and won't compile under `--doc` (G1).
      Mode-A doc-comments are prose citing `spec/DEVICE_DISCOVERY.md` §2.
- ❌ Do NOT touch `DeviceStatus`/`device_status()`/`classify_device_status()` (P1, Complete) —
      they are the aggregate complement, distinct names (G6). Do NOT touch `perform_handshake`,
      `HOST_CAPABLE`, `BOARD_HAS_RULES`, `CALLBACK_NAMES` (the global capability path, G7).
- ❌ Do NOT eagerly evict expired entries in `get`. Lazy, non-evicting get is simplest and
      side-effect-free; S2's insert overwrites stale entries (research/notes.md §4).
- ❌ Do NOT run tests multi-threaded — `cargo test --bin qmkonnect -- --test-threads=1`
      (shared MockNotifier globals + DebounceState, G4/AGENTS.md).
- ❌ Do NOT edit the in-flight P2.M1.T1.S2 file (`architecture/write_narrowing_decision.md`)
      or any `docs/*.md` (P4) or `PRD.md` / `tasks.json` / `prd_snapshot.md` / `Cargo.toml`.
- ❌ Do NOT add write-narrowing logic. P2.M1.T1.S2 confirms write-narrowing is DEFER (needs a
      crate change); this task's classification types feed the PICKER/STATUS, not the write path.

---

## Confidence Score: 9/10

This is a well-bounded **types-and-infrastructure** task: the two types are given
**verbatim** from `spec/DEVICE_DISCOVERY.md` §2.3 (reproduced in `research/notes.md`
§1), `DeviceKind::Capable` is confirmed field-for-field against the Cargo.lock-pinned
crate `CommandResponse::Info` (rev `f26893e`, lib.rs:95-99 — §2), and every in-file
pattern to mirror is named with its file:line + exact form (`CALLBACK_NAMES` @276,
`CALLBACK_SWEEP_DEADLINE` @~412, `HOST_CAPABLE`/`host_capable` @270/689 — §3). The 3
helper signatures + the TTL-check body are given verbatim (§4), the imports are all
already present (no new `use`, G9), and no Cargo/serde change is needed (G10). The
relationship to the already-complete `DeviceStatus` (P1) and the in-flight P2.M1.T1.S2
(zero `.rs` overlap) is documented (§6) to prevent collisions. The ~7 cache tests are
pure (no HID mock) and pin the TTL/overwrite/clear contract. The 1-point reservation
is for: (a) the poisoned-lock handling choice in insert/clear (swallow vs unwrap —
both defensible; the PRP permits the non-panicking form, caught by a build warning at
worst); (b) the TTL-expiry test reaching into the static to rewrite the `Instant`
(an idiomatic same-module test, but an implementer might over-engineer a clock-injection
trait — the PRP explicitly gives the direct-rewrite approach); and (c) the precise
placement line (after `device_status()` ~778 vs after the statics cluster ~299 — either
is correct, the PRP names both). All three are low-risk and caught by the build/tests.
Scope is cleanly bounded from S2 (classify_devices logic — not implemented), the global
capability path (untouched, G7), and the write path (P2.M1.T1.S2 DEFER, untouched).
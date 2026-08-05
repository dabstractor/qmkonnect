# Research Notes — P3.M1.T1.S1: DeviceKind / ClassifiedDevice types + CLASSIFICATION_CACHE infra

> **Scope of THIS subtask:** add the **types + cache infrastructure only** to
> `src/core/notifier.rs`. NO `classify_devices()` logic (that is S2), NO
> `send_command` calls, NO hidapi opens beyond what's already there. It ships
> the data model + TTL cache + 3 cache helpers that S2's `classify_devices()`
> will populate/consume. Compiles; no behavior change yet.

---

## 1. The exact types to add (verbatim from `spec/DEVICE_DISCOVERY.md` §2.3)

```rust
/// Per-device capability classification. `Capable` mirrors the crate's
/// `CommandResponse::Info` reply (see §2.2); everything else → `NotQmkNotifier`.
pub enum DeviceKind {
    Capable {
        proto_ver: u8,
        feature_flags: u8,
        callback_count: u8,
        board_rules_present: bool,
    },
    NotQmkNotifier,
}

/// One enumerated Tier-1 HID interface + its classification. `path` is the
/// stable hidapi path (the cache key). Returned by `classify_devices()` (S2).
pub struct ClassifiedDevice {
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub product_name: Option<String>,
    pub usage_page: u16,
    pub usage: u16,
    pub kind: DeviceKind,
}
```

**Derives:** `DeviceKind` → `Debug, Clone, PartialEq`. Clone is REQUIRED (the
cache `get` returns an owned `DeviceKind`; the picker S2/P3.M2 may clone).
`ClassifiedDevice` → `Debug, Clone, PartialEq` (mirrors; the picker clones rows).
`Copy` is *available* on `DeviceKind` (all fields are `Copy`) but is NOT mandated
by the spec — leave it at `Clone` to match the minimal derive set (avoid
over-deriving; `ClassifiedDevice` contains `String` so it is NOT `Copy`).
**No serde** — these are runtime HID types, not config.

## 2. The capable classifier — crate contract (PINNED rev `f26893e`)

The crate's `CommandResponse::Info` variant (verified at the Cargo.lock-pinned
checkout `~/.cargo/git/checkouts/qmk-notifier-1f15950b695d9922/f26893e/src/lib.rs:95-99`):

```rust
Info {
    proto_ver: u8,          // :96
    feature_flags: u8,      // :97
    callback_count: u8,     // :98
    board_rules_present: bool,  // :99
}
```

`DeviceKind::Capable { proto_ver, feature_flags, callback_count, board_rules_present }`
mirrors this **field-for-field** (same names, same types). S2's `classify_devices()`
will `match` the `send_command(RunCommand::QueryInfo, &filter)` reply: `Info { proto_ver: 2, .. }`
→ `Capable { .. }`; `Legacy`/`Timeout`/other/`Err` → `NotQmkNotifier` (DEVICE_DISCOVERY §2.2).
The existing `perform_handshake_with` (notifier.rs:421) already does this match for the
GLOBAL capability (`HOST_CAPABLE`); S2 does it PER CANDIDATE. **THIS task ships only the
type that carries the result.**

## 3. The exact in-file patterns to mirror (confirmed by reading notifier.rs)

| thing to add | pattern to mirror (file:line) | exact form |
|---|---|---|
| `CLASSIFICATION_CACHE` static | `CALLBACK_NAMES` @ notifier.rs:276 | `static CALLBACK_NAMES: Lazy<Mutex<HashMap<String, u8>>> = Lazy::new(\|\| Mutex::new(HashMap::new()));` |
| `CLASSIFICATION_TTL` const | `CALLBACK_SWEEP_DEADLINE` @ notifier.rs:~412 | `const CALLBACK_SWEEP_DEADLINE: Duration = Duration::from_secs(5);` |
| private static + pub reader/writer | `HOST_CAPABLE` @270 / `host_capable()` @689 | static is NOT `pub`; the reader fn IS `pub`. `BOARD_HAS_RULES` @1146 / `board_has_rules()` @1150 same. |
| imports already present | notifier.rs:1-9 | `use once_cell::sync::Lazy;`, `use std::collections::{BTreeSet, HashMap};`, `use std::sync::{Arc, Condvar, Mutex, OnceLock};`, `use std::time::{Duration, Instant};` — **ALL already imported; NO new `use` lines.** |

So `CLASSIFICATION_CACHE` is written as:
```rust
static CLASSIFICATION_CACHE: Lazy<Mutex<HashMap<String, (DeviceKind, Instant)>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
```
keyed by **path** (`String`), value `(DeviceKind, Instant)` (the Instant = when it
was classified, for the TTL check). This matches the spec §2.3 verbatim.

## 4. The 3 cache helpers — signatures + TTL semantics

```rust
/// Look up a device's cached [`DeviceKind`] by its stable hidapi `path`.
/// Returns `None` when absent OR when the entry is older than
/// [`CLASSIFICATION_TTL`] (lazy expiry — the stale entry is left in place; a
/// later `classification_cache_insert` overwrites it). `Some(kind)` only when
/// fresh. S2's `classify_devices()` calls this before pinging.
pub fn classification_cache_get(path: &str) -> Option<DeviceKind> { ... }

/// Record (or refresh) a device's classification. The `Instant` is stamped now.
/// Called by S2's `classify_devices()` after each successful per-candidate probe.
pub fn classification_cache_insert(path: &str, kind: DeviceKind) { ... }

/// Drop every cached entry. Called on a real device transition (device-loss)
/// and by the tray "Reload rules" path so stale classifications don't survive a
/// board swap.
pub fn classification_cache_clear() { ... }
```

**TTL check in `get`** (the ONLY non-trivial logic in this task):
```rust
pub fn classification_cache_get(path: &str) -> Option<DeviceKind> {
    let map = CLASSIFICATION_CACHE.lock().ok()?;
    let (kind, stamped) = map.get(path)?;
    if stamped.elapsed() < CLASSIFICATION_TTL {
        Some(kind.clone())          // fresh → clone out (DeviceKind: Clone)
    } else {
        None                        // stale → treat as miss (lazy; do NOT mutate under lock)
    }
}
```
**Design choice — lazy, non-evicting get:** `get` returns `None` for expired
entries but does NOT remove them (removing under the read lock + the S2 insert
overwrite is enough). This keeps `get` side-effect-free and trivially testable.
S2 is the authority on eviction cadence. **No `Instant::now()` is called at module
load** — only inside `get`/`insert`, so the cache is cheap until used.

## 5. Placement in notifier.rs

The statics cluster lives at lines 270-299 (`HOST_CAPABLE`, `CALLBACK_NAMES`,
`HAS_HANDSHAKED`, `STARTUP_DEVICE_CONNECTED`, `RULES_INVALID_NOTIFIED`); the
three-state resolver cluster lives at 689-778 (`host_capable`, `board_has_rules`
@1146-1151, `DeviceStatus` enum @719, `device_status()` @761).

**Recommended placement:** a NEW clearly-delimited section
`// ===== Device classification (P3.M1) — per-candidate capability tier =====`
containing: `DeviceKind`, `ClassifiedDevice`, `CLASSIFICATION_TTL` const,
`CLASSIFICATION_CACHE` static, and the 3 helpers. Put it **after the
`DeviceStatus`/`device_status()` resolver region (after ~line 778)** — that is the
status/classification neighborhood, and S2's `classify_devices()` (a sibling of
`is_device_connected` @216 and `device_status` @761) will land adjacent. An
acceptable alternative is right after the existing statics cluster (after
`RULES_INVALID_NOTIFIED`, ~line 299). Either is fine; pick one and keep the
section contiguous. Do NOT scatter the five items.

## 6. Relationship to already-shipped / in-flight work (avoid collisions)

- **P1.M1.T1.S1 (Complete): `DeviceStatus` enum + `device_status()` resolver**
  (notifier.rs:719-778). `DeviceStatus { Connected, NoModule, Disconnected }` is
  the **aggregate three-state tray status** derived from `is_device_connected() &&
  host_capable()`. My `DeviceKind` is the **per-device** classification. **No name
  collision** (DeviceStatus vs DeviceKind vs ClassifiedDevice are all distinct).
  Do NOT touch `DeviceStatus`/`device_status()`/`classify_device_status()` (the
  latter is a pure test helper @2960). Cite the relationship in rustdoc: the
  global `device_status()` is an aggregation over the per-device `DeviceKind`s
  that S2's `classify_devices()` produces.
- **P2.M1.T1.S1 (Complete): R-COEX comments + `r_coex_invariants` tests** in
  notifier.rs. Separate section; do NOT touch it.
- **P2.M1.T1.S2 (in-flight, parallel): write-narrowing DEFER confirmation** —
  edits `architecture/write_narrowing_decision.md` ONLY. **ZERO `.rs` overlap**
  with this task. Its conclusion (DEFER) is *consistent* with this task: per-device
  classification (my types) feeds the **picker/status**, NOT the write path. The
  crate has no per-path send (external_deps.md: `MatchKey` is private +
  filter-keyed @core.rs:641), so S2's per-candidate probe narrows the **filter** to
  the candidate's vid/pid (the only app-side mechanism). **THIS task adds no send
  logic at all** — only the types + cache. Cite external_deps.md §"CRITICAL CONSTRAINT".
- **Downstream S2 (`classify_devices`)** will: `HidApi::new()` (mirror
  `is_device_connected` @216 / `list_devices` @129), `.filter()` Tier-1 candidates,
  for each narrow `configured_filter()` to the candidate's vid/pid, call
  `send_command(QueryInfo, &narrowed)`, match the reply into `DeviceKind`,
  `classification_cache_insert(path, kind)`, collect `ClassifiedDevice`. THIS task
  ships everything S2 calls. **Do NOT implement any of it here.**

## 7. Gotchas (with pinning)

- **G1 — binary-only crate; doctests don't run under `--bin`.** No `lib.rs`
  (`src/main.rs:3` `mod core;`). The AGENTS.md command `cargo test --bin qmkonnect`
  runs UNIT tests only, not doctests. Mode-A doc-comments here are **prose citing
  DEVICE_DISCOVERY.md §2** — NOT runnable ` ``` ` Rust doctests (a runnable
  `use qmkonnect::core::notifier::DeviceKind` doctest won't compile under
  `--doc`). Use plain prose + ` ```text `/` ```ignore ` if a code sketch is wanted.
- **G2 — NO logic in this task.** No `classify_devices()`, no `send_command`, no
  `HidApi::new()` in the production path, no `perform_handshake` change. Only the 5
  items (2 types + 1 const + 1 static + 3 helpers) + tests. S2 owns the logic.
- **G3 — the crate has NO per-path send** (external_deps.md). This constrains S2
  (narrow filter by vid/pid), NOT this task. But the cache is keyed by **path**
  (not filter) because the picker/status cares about *which physical device* is
  capable, and hidapi `path` is the stable per-interface identity
  (`DeviceInfo::path()`). Do NOT key the cache by vid/pid.
- **G4 — single-threaded tests, crate-wide.** `cargo test --bin qmkonnect --
  --test-threads=1` (shared `MockNotifier` globals + `DebounceState`, AGENTS.md).
  Never multi-threaded.
- **G5 — `DeviceKind` must derive `Clone`** so `classification_cache_get` can
  return an owned `Option<DeviceKind>`. (The cache stores `(DeviceKind, Instant)`;
  `get` clones the `DeviceKind` out. `ClassifiedDevice` also Clone — picker clones
  rows.) Do NOT make the static's value type a reference.
- **G6 — no name collision with `DeviceStatus`.** The aggregate three-state enum
  (P1.M1.T1.S1) is `DeviceStatus { Connected, NoModule, Disconnected }`. My
  per-device enum is `DeviceKind { Capable{..}, NotQmkNotifier }`. Distinct names,
  distinct semantics. Do NOT rename either.
- **G7 — do NOT touch `perform_handshake` / `HOST_CAPABLE` / `BOARD_HAS_RULES`
  / `CALLBACK_NAMES`.** Those are the GLOBAL capability path (set once per boot).
  My cache is the PER-DEVICE path (populated by S2 per candidate). They coexist.
- **G8 — Mode A docs only.** Doc-comments cite `spec/DEVICE_DISCOVERY.md` §2
  (esp. §2.2 the QUERY_INFO → `CommandResponse::Info` match, §2.3 the
  `ClassifiedDevice` struct + cache, §2.4 the handshake relationship). No
  `docs/*.md` / README edits (that's P4).
- **G9 — `Instant` is already imported** (`use std::time::{Duration, Instant};`
  @notifier.rs:9). `Lazy`, `Mutex`, `HashMap` also already imported. **No new
  `use` lines** — adding a duplicate `use` is a compile warning.
- **G10 — no serde, no Cargo change.** These are runtime types. serde/toml are
  config deps; irrelevant here. once_cell (Lazy) + std are already deps.

## 8. Recommended test set (the validation gate)

Mirror the existing notifier.rs test idiom (`reset_test_state()`,
`--test-threads=1`). The cache helpers are pure (lock a static HashMap), so tests
need no HID mock — just call insert/get/clear directly. Naming prefix
`test_classification_cache_` (disjoint from the 49 existing `test_*`/`r_coex_*`):
1. `test_classification_cache_insert_then_get` — insert a `Capable`, get returns `Some(equal)`.
2. `test_classification_cache_miss` — get on an unseen path → `None`.
3. `test_classification_cache_clear` — insert, clear, get → `None`.
4. `test_classification_cache_overwrite` — insert Capable, insert NotQmkNotifier same path, get → Some(NotQmkNotifier).
5. `test_classification_cache_ttl_expiry` — insert, then **simulate expiry** by
   rewriting the stored `Instant` to the past (`stamp = Instant::now() -
   CLASSIFICATION_TTL - Duration::from_millis(1)`) — get → `None`. (Requires the
   test to reach into the map via the same `CLASSIFICATION_CACHE.lock()`; that's
   fine in a `#[cfg(test)]` block in the same module.)
6. `test_classification_cache_notqmk_variant` — round-trip a `NotQmkNotifier`
   entry (proves the unit variant Clone works).
7. `test_devicekind_classifieddevice_derives` — assert `DeviceKind::Capable{..} ==
   DeviceKind::Capable{..}` (PartialEq), and `Clone` produces an equal value (sanity).

~7 tests. `reset`-friendly: each test should start with `classification_cache_clear()`
to avoid cross-test interference (the static outlives tests).

## 9. Scope boundary (do NOT do)

- ❌ `classify_devices()` / any hidapi enumeration / any `send_command` (S2).
- ❌ Narrowing the filter to a candidate's vid/pid (S2 — and DEFER'd for the write path per P2.M1.T1.S2).
- ❌ Touching `perform_handshake`, `HOST_CAPABLE`, `BOARD_HAS_RULES`, `CALLBACK_NAMES`,
  `DeviceStatus`, `device_status()`, `is_device_connected()`, `list_devices()`.
- ❌ Wiring the cache into any tray/poll/handshake path (S2 + P3.M2 + P4).
- ❌ serde / Cargo.toml / new `use` imports (all already present).
- ❌ `docs/*.md` / README (Mode A — P4).
- ❌ Runnable Rust doctests with `qmkonnect::` paths (G1).
- ❌ Editing the in-flight P2.M1.T1.S2 file (`architecture/write_narrowing_decision.md`).
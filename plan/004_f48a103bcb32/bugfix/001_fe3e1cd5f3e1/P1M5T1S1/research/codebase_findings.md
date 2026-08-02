# Codebase Findings — P1.M5.T1.S1 (mtime-keyed Config cache)

> Direct source reading of the QMKonnect Rust crate. All line numbers verified
> against current `src/` (rustc 1.92, MSRV 1.88, edition 2021).

## 1. The two HOT read paths (only these get cached)

Per the `architecture/config_reread_research.md` "exact read path per send":

```
notify_qmk / debounce_worker flush
 ├─ state.interval() → configured_debounce_ms() → configured_timing()   [config.toml READ #1]
 ├─ configured_filter()                                                  [config.toml READ #2]
 └─ host_context_for_window() → parse_rules()                            [rules.toml READ #1]
```

The cache eliminates the **double config.toml read** per send and caches the
rules read. Only these THREE call sites change:

| Call site | File:line | Current | After |
|---|---|---|---|
| `configured_timing` | `src/core/mod.rs:97-102` | `parse_config(&p)` inline | `cached_config()` |
| `configured_filter` | `src/core/notifier.rs:80-99` | `parse_config(&p)` inline | `cached_config()` |
| `host_context_for_window` | `src/core/notifier.rs:1072` | `rules::parse_rules(&path)` | `cached_rules_at(&path)` |

## 2. `parse_config` / `parse_rules` callers that STAY uncached (verified by grep)

**parse_config** (must remain uncached — fresh read desired):
- `notifier.rs:120` `config_parse_error_at` — startup diagnostic (one-time, wants fresh error).
- `linux_tray.rs:789,818` + `tray.rs:764,1192` — Settings-dialog reads (user-initiated UI, infrequent; the user just edited the file — MUST see fresh content, not a stale cache entry from the notifier thread).
- `main.rs:512` — `--validate` / startup config load.
- tests in `mod.rs` (use `toml::from_str` directly, not parse_config — unaffected).

**parse_rules** (must remain uncached):
- `notifier.rs:633` — `perform_handshake` callback-name validation (once per board boot; fresh desired).
- `main.rs:414` — `--validate-rules` CLI lint.
- tests in `rules.rs:663,735,752,762,784,1291,1310,1326` (hermetic per-test temp files).

## 3. CONTRACT CORRECTION — `test_debounce_ms_is_hot_config` does NOT validate the cache

The item contract claims: *"test_debounce_ms_is_hot_config (notifier.rs:1379)
asserts ... it writes a new config.toml and verifies the new interval is picked
up."* **This is inaccurate.** Reading `notifier.rs:1451-1491`, the test:

```rust
STATE.lock().unwrap().interval_override = Some(Duration::from_millis(200));
...
STATE.lock().unwrap().interval_override.replace(Duration::from_secs(30));
```

It sets `DebounceState::interval_override` (a `#[cfg(test)] Option<Duration>`),
which **bypasses** `configured_debounce_ms()` → `configured_timing()` →
`parse_config()` entirely (see the `#[cfg(test)] impl DebounceState::interval`
at `notifier.rs:860-866`: `self.interval_override.unwrap_or_else(|| …)`). The
test never touches `config.toml` on disk.

**Implication:** this existing test passes trivially regardless of caching
(because it never enters the config-read path). It CANNOT validate the cache.
→ **New dedicated cache tests are MANDATORY** (see PRP Validation + Tasks).

## 4. Clone derivation chain (verified)

The cache stores owned values and returns clones; all cached types need `Clone`:

| Type | File:line | Current derives | Action |
|---|---|---|---|
| `Config` | `mod.rs:18` | `serde::Deserialize, serde::Serialize` | **add `Clone`** (fields: Option<u16>×4, u64×2 — all Clone) |
| `RuleSet` | `rules.rs:73` | `Debug, Deserialize, Default` | **add `Clone`** (fields: HostDefaults, Vec<Rule>) |
| `HostDefaults` | `rules.rs:97` | `Debug, Default, Deserialize` | **add `Clone`** (field: bool) |
| `Rule` | `rules.rs:138` | `Debug, Deserialize` | **add `Clone`** (fields: Pattern, Option<u8>, Vec<String>×2, bool, Option<bool>) |
| `Pattern` | `pattern.rs:1116` | `Debug, Clone, PartialEq, Deserialize` | **already Clone** ✓ |

All fields are `Clone`, so a bare `#[derive(Clone)]` on each struct suffices.

## 5. Cache key design (robustness over the contract's literal `(SystemTime, Config)`)

Contract specifies `Option<(SystemTime, Config)>`. **Hardened to
`Option<(PathBuf, SystemTime, u64)>` + value**, for three reasons:

1. **mtime** = primary hot-config invalidation signal (`meta.modified()?`).
2. **size** (`meta.len()`) = secondary key for **coarse-mtime filesystems**
   (contract NOTE: 1s resolution; a same-second rewrite that changes size still
   invalidates). A same-second, same-size rewrite is an accepted, vanishingly-rare edge.
3. **path** = identity safety. `get_config_paths()` returns up to 3 candidates;
   if the user deletes a higher-priority candidate mid-run, the resolved path
   changes. Keying on path prevents serving a cached entry from a *different*
   file that happens to share mtime+size.

## 6. Testability design — the `_at` hermetic core + miss-counter

**Problem:** `cached_config()` resolves the path via `platforms::get_config_paths()`
(platform-specific, reads `$HOME`/XDG — not redirectable to a temp dir in a test).

**Solution (mirrors the existing `parse_config` / `config_parse_error_at` split):**
- `cached_config_at(path)` — hermetic core: stat + cache-check + parse. **Tests call this** with a `tempfile` path.
- `cached_config()` — resolves path, delegates to `_at` (or returns `Config::default()` uncached when no path exists).

Same split for rules: `cached_rules_at(path)` (tests) vs `host_context_for_window` resolving the path then calling `cached_rules_at`.

**Proving a cache HIT (the hard part):** on Linux ext4, mtime has **nanosecond**
resolution, so two rapid writes ALWAYS differ in mtime — you cannot force a
cache hit by controlling mtime/size. The only rigorous, platform-independent
observable is a **miss counter**:

```rust
#[cfg(test)]
static CONFIG_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);  // incremented ONLY on cache-miss (parse_config invocation)
```

Tests snapshot `before = CONFIG_CACHE_MISSES.load()`, call `cached_config_at`
twice, assert the **delta** is `0` (HIT) or `1` (MISS/re-parse). Single-threaded
suite (`--test-threads=1`, AGENTS.md) ⇒ no races on the global counter/cache.
Unique `tempfile::TempDir` paths per test ⇒ no cross-test cache collisions.

**Deterministic mtime advance (no sleep):** MSRV 1.88 ≥ 1.75, so use
`std::fs::FileTimes` + `File::set_times` to set mtime to `SystemTime::now() +
2s` — fast and exact (vs the flaky ≥1s `thread::sleep` alternative).

## 7. Mutex poisoning — follow P1.M1.T1.S1 idiom

The codebase already recovers `std::sync::Mutex` poison defensively
(`STATE.lock().unwrap_or_else(|e| e.into_inner())` — `notifier.rs:890+`). Use the
**same** `unwrap_or_else(|e| e.into_inner())` for both new caches — never let a
panic in one call site poison the cache for the whole process.

## 8. Don't-cache-failures rule

On a parse error, `cached_config_at`/`cached_rules_at` return `Err` WITHOUT
storing. Rationale:
- A later valid edit must re-read (re-arm). Caching an error would freeze the broken state.
- `host_context_for_window`'s `RULES_INVALID_NOTIFIED` re-arm logic
  (`notifier.rs:1071`: good parse → store false) depends on re-reading after the
  user fixes the file.

## 9. Imports to add to `src/core/mod.rs`

```rust
use std::path::PathBuf;            // already has std::path::Path
use std::sync::Mutex;              // already has std::sync::OnceLock
use std::time::SystemTime;
use once_cell::sync::Lazy;         // dep present (notifier.rs uses it; Cargo.toml once_cell=1.21)
#[cfg(test)]
use std::sync::atomic::AtomicU64;
```

## 10. Comment updates required (so docs stop lying)

- `notifier.rs:84` comment "Read per-call so config changes take effect..." →
  note the mtime-keyed cache (still hot-config; just avoids redundant re-reads).
- `notifier.rs:830-835` comment "intentionally NOT cached here" → update: the
  *value* is now mtime-cached in `cached_config()`, but `interval()` still
  re-resolves the effective window each call (hot-config preserved — an mtime
  change invalidates on the next call, ~instant, not a TTL delay).

## 11. No external research needed

The approach (stat → `Metadata::modified()` → key a `Mutex<Option<…>>`) is plain
`std`. `once_cell::sync::Lazy` is the established repo convention. No new crate,
no novel API. The authoritative internal analysis is
`architecture/config_reread_research.md` (recommends exactly this: "Prefer
mtime-keyed caching over fixed TTL"; "Coalesce the config.toml double-read first").
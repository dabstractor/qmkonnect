# PRP — P1.M5.T1.S1: mtime-keyed Config/Rules cache wrapping parse_config / parse_rules

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`) — Rust menu-bar/tray daemon.
> **Files (2 Rust source + 0 packaging):** `src/core/mod.rs` (EDIT), `src/core/notifier.rs`
> (EDIT), `src/core/rules.rs` (EDIT — derive Clone only).
>
> **Scope:** internal performance optimization. Eliminates the redundant `config.toml`
> double-read per emitted send (`notify_qmk` reads it once for the debounce interval,
> once for the device filter) and caches the per-window `rules.toml` read — all while
> **preserving the documented hot-config contract** (editing either file takes effect on
> the very next call, via an mtime+size re-stat; NO TTL delay). No user-facing, config-
> schema, or API surface change. All 345+ tests must still pass, plus new cache tests.
>
> **Source of truth for this design:** `architecture/config_reread_research.md`
> (recommends exactly this: "Prefer mtime-keyed caching over fixed TTL"; "Coalesce the
> config.toml double-read first") + `research/codebase_findings.md` (verbatim caller
> inventory, Clone chain, testability design, contract corrections).

---

## Goal

**Feature Goal**: Add a thread-safe, **mtime+size-keyed** cache in `src/core/mod.rs`
that wraps `parse_config` (and `parse_rules`) so that the three HOT per-send read paths
— `configured_timing` (`mod.rs:97`), `configured_filter` (`notifier.rs:80`), and
`host_context_for_window` (`notifier.rs:1072`) — re-stat the file on every call but
only re-read+re-parse it when the file actually changed. This halves the `config.toml`
disk reads per send (2× → 1×) and caches the `rules.toml` read, while keeping hot-config
**instant** (mtime change invalidates on the next call — not delayed by a TTL window).

**Deliverable** (3 Rust files edited, no new files, no new deps):
1. `src/core/mod.rs` — add `CONFIG_CACHE` + `RULES_CACHE` (both `Lazy<Mutex<…>>`),
   the hermetic cores `cached_config_at(path)` / `cached_rules_at(path)`, the path-
   resolving wrappers `cached_config()`, a `#[cfg(test)] CONFIG_CACHE_MISSES` +
   `RULES_CACHE_MISSES` observable counter, and switch `configured_timing()` to
   `cached_config()`. Derive `Clone` on `Config`.
2. `src/core/notifier.rs` — switch `configured_filter()` to `cached_config()` and
   `host_context_for_window`'s `parse_rules(&path)` to `cached_rules_at(&path)`. Update
   the now-stale "Read per-call" / "intentionally NOT cached" comments (notifier.rs:84,
   :830) to describe mtime-keyed caching.
3. `src/core/rules.rs` — derive `Clone` on `RuleSet`, `HostDefaults`, `Rule` (`Pattern`
   is already `Clone`). `parse_rules` stays uncached for its other callers.

**Success Definition**:
- `cargo test --bin qmkonnect -- --test-threads=1` is green with **3 new cache tests**
  added (2 config, 1 rules) and **zero existing tests regressing**.
- A cache HIT provably skips re-parse: the miss-counter delta is `0` across two
  `cached_config_at` calls with an unchanged file (rigorous, platform-independent proof).
- Hot-config is preserved: writing a new value to the file invalidates the cache on the
  next call (verified by the miss-counter delta `1` AND the returned value updating) —
  for BOTH a size change AND a mtime-only change (same byte length).
- The per-send `config.toml` double-read is coalesced: `configured_timing` and
  `configured_filter` share a single cache entry (verified by reading the code path:
  both call `cached_config()` → one `Mutex`, one `(mtime, size)` key).

## User Persona (if applicable)

N/A — internal optimization. The end user never sees this. The beneficiary is a user
whose `config.toml`/`rules.toml` lives on a **slow/networked filesystem** (NFS, iCloud,
Syncthing-backed home dir) where each `read_to_string` is tens of ms and the 2×
amplification adds perceptible latency to the foreground focus thread on the immediate-
send path. On local SSD the win is negligible but free.

## Why

- **The double-read is real and intentional-but-cheap-to-fix.** `architecture/config_reread_research.md`
  confirms: every emitted send reads `config.toml` **2×** (interval + filter) and, when
  host-capable + rules.toml exists, `rules.toml` **1×**. No cache of any kind exists
  today (verified by `grep` for `OnceLock`/`cache`/`mtime` across `src/core` + `src/platforms`).
- **Hot-config must be preserved — and mtime-keying does that better than a TTL.** The
  comments (`notifier.rs:84`, `:830`) and the PRD (§8 hot-config SLO ~3s) state the
  re-read is deliberate. A fixed-TTL cache would re-introduce a delay; an **mtime-keyed**
  cache re-stats every call (cheap `stat()` syscall) and re-reads only on change — so an
  edit takes effect on the **next** notification call (~instant), never delayed. This is
  exactly the recommendation in `config_reread_research.md` ("Prefer mtime-keyed caching
  over fixed TTL").
- **Coalescing the double-read is the highest-leverage, zero-behavior-change win.**
  Sharing one cache entry between `configured_timing` + `configured_filter` halves reads
  with no semantic change (they already read the same file the same way).

## What

### Approach: per-file `Lazy<Mutex<Option<(PathBuf, SystemTime, u64)>>>` + value, in `mod.rs`

Both caches live in `src/core/mod.rs` (single responsibility: the core module owns all
config/rules caching). The **hermetic, testable core** takes an explicit path
(`cached_config_at` / `cached_rules_at`, mirroring the existing `parse_config` /
`config_parse_error_at` split); the path-resolving wrapper (`cached_config`) resolves
candidates via `get_config_paths()` then delegates. `host_context_for_window` resolves
its own path (it needs it for the malformed-file error message) then calls
`cached_rules_at`.

**Cache key = `(PathBuf, SystemTime, u64)`** = (resolved path, mtime, byte-size):
- **mtime** — primary hot-config signal (`path.metadata()?.modified()?`).
- **size** — secondary key for coarse-mtime filesystems (the contract NOTE: 1s resolution;
  a same-second rewrite that changes size still invalidates).
- **path** — identity safety (`get_config_paths()` returns up to 3 candidates; keying on
  path prevents serving a cached entry from a *different* file that coincidentally shares
  mtime+size if the resolved candidate changes mid-run).

This **hardens** the contract's literal `Option<(SystemTime, Config)>` — see
`research/codebase_findings.md` §5 for the rationale.

### Success Criteria

- [ ] `Config` (`mod.rs:18`), `RuleSet`/`HostDefaults`/`Rule` (`rules.rs:73/97/138`)
      derive `Clone` (`Pattern` already does).
- [ ] `src/core/mod.rs` has `CONFIG_CACHE` + `RULES_CACHE`
      (`Lazy<Mutex<Option<(PathBuf, SystemTime, u64, T)>>>`) and `cached_config_at` /
      `cached_rules_at` (hermetic) + `cached_config` (path-resolving).
- [ ] `configured_timing` (`mod.rs:97`) calls `cached_config()`; `configured_filter`
      (`notifier.rs:80`) calls `cached_config()`; `host_context_for_window`
      (`notifier.rs:1072`) calls `cached_rules_at(&path)`.
- [ ] `parse_config` (`mod.rs:106`) and `parse_rules` (`rules.rs:210`) are **unchanged**
      and remain the uncached path for: `config_parse_error_at`, tray/UI reads
      (`tray.rs`, `linux_tray.rs`), `main.rs` CLI, the handshake validation
      (`notifier.rs:633`), and all existing tests.
- [ ] Parse failures are **never cached** (return `Err` without storing — preserves
      `host_context_for_window`'s `RULES_INVALID_NOTIFIED` re-arm + the graceful fallback).
- [ ] Mutex access uses the `unwrap_or_else(|e| e.into_inner())` poison-recovery idiom
      (P1.M1.T1.S1 precedent) — never propagates poison.
- [ ] The stale "Read per-call" / "intentionally NOT cached" comments at `notifier.rs:84`
      and `:830` are updated to describe the mtime-keyed cache (hot-config preserved).
- [ ] 3 new tests added in `mod.rs`'s test module (config hit, config mtime-invalidation,
      rules hit+invalidate); all pass single-threaded.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` green; existing count unchanged
      except for the +3 new tests; `test_debounce_ms_is_hot_config` STILL passes
      (it bypasses the config path via `interval_override` — see Gotchas).

## All Needed Context

### Context Completeness Check

_Pass._ An agent with no prior knowledge can implement this from: the exact current
bodies of `configured_timing`, `configured_filter`, `host_context_for_window`, and
`parse_config`/`parse_rules` (quoted in Tasks); the verbatim cache implementation
(given in Patterns); the exact derives to add; the verbatim 3 test functions; the
caller inventory (`research/codebase_findings.md` §1-2) proving which sites change vs
stay uncached; and the single critical contract correction (the existing hot-config
test does NOT validate the cache). The two non-obvious pieces (proving a cache HIT on
ns-mtime Linux; deterministic mtime advance) are spelled out with the exact primitives
(`CONFIG_CACHE_MISSES` counter; `std::fs::FileTimes`).

### Documentation & References

```yaml
# MUST READ — the authoritative internal analysis (recommends exactly this approach)
- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/architecture/config_reread_research.md
  why: "establishes the exact per-send read path (config 2x + rules 1x), confirms NO cache
        exists today, confirms hot-config is intentional (PRD §8 ~3s SLO), and RECOMMENDS
        'Prefer mtime-keyed caching over fixed TTL' + 'Coalesce the config.toml double-read
        first'. This PRP implements that recommendation verbatim."
  section: "The Exact Read Path Per Send", "Recommendation", "Start Here"
  critical: "the three HOT call sites to change are configured_timing (mod.rs:97),
        configured_filter (notifier.rs:80), host_context_for_window (notifier.rs:1072).
        Keep parse_config/parse_rules uncached for the OTHER callers (research §1-2)."

# MUST READ — verbatim caller inventory, Clone chain, testability design, contract corrections
- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/P1M5T1S1/research/codebase_findings.md
  why: "the grep-verified list of EVERY parse_config/parse_rules caller (which CHANGE vs which
        STAY uncached), the exact derives to add (Config + RuleSet/HostDefaults/Rule; Pattern
        already Clone), the cache-key hardening rationale ((PathBuf,SystemTime,u64)), the
        _at hermetic-core testability split, the miss-counter design, and the CRITICAL
        contract correction: test_debounce_ms_is_hot_config does NOT exercise the cache
        (it uses STATE.interval_override, bypassing the config-read path)."
  section: "all"
  critical: "do NOT rely on test_debounce_ms_is_hot_config to validate the cache — it can't.
        New dedicated cache tests are MANDATORY (Tasks 7-9)."

# MUST READ — the file being edited: configured_timing + parse_config + the Config struct
- file: src/core/mod.rs
  why: "lines 18 (Config derive — add Clone), 89-102 (configured_debounce_ms/configured_timing
        — switch the latter to cached_config), 106-112 (parse_config — KEEP unchanged as the
        uncached core). The imports at top (add Mutex, SystemTime, PathBuf, Lazy, AtomicU64).
        The test module (#[cfg(test)] mod tests) is where the 3 new tests go."
  pattern: "configured_timing currently: get_config_paths().find(exists).and_then(parse_config.ok()).map(timing).unwrap_or(default).
        After: cached_config().ok().map(timing).unwrap_or(default). parse_config stays for
        config_parse_error (notifier.rs:120) + tray/main callers."
  gotcha: "Config has a MANUAL Default impl (mod.rs:32-48) — do NOT replace it with derive.
        Adding #[derive(Clone)] alongside serde is safe (all fields Clone). The DEFAULT_*
        consts and render_config_body/create_default_config are unrelated — don't touch them."

# MUST READ — the file being edited: configured_filter + host_context_for_window
- file: src/core/notifier.rs
  why: "lines 80-99 (configured_filter — switch to cached_config().ok()), 1062-1097
        (host_context_for_window — switch parse_rules(&path) to cached_rules_at(&path),
        KEEP the Ok/Err match + RULES_INVALID_NOTIFIED re-arm logic intact), 84 + 830-835
        (stale comments to update). Imports at line 2-7 already have Lazy/Mutex."
  pattern: "configured_filter builds a DeviceFilter from the Option<Config>; keep that shape —
        only swap the read source. host_context_for_window resolves get_rules_paths().find(exists)?,
        then parses; keep the path resolution (needed for the error message at :1085) and just
        swap parse_rules -> cached_rules_at."
  gotcha: "do NOT change notifier.rs:633 (perform_handshake callback validation) — it stays on
        uncached parse_rules (once-per-boot, fresh desired). Do NOT change the re-arm logic
        (RULES_INVALID_NOTIFIED) — a good parse must still store(false); the cache just feeds
        the same Result<RuleSet> into the same match."

# MUST READ — derive Clone targets (RuleSet/HostDefaults/Rule)
- file: src/core/rules.rs
  why: "lines 73 (RuleSet), 97 (HostDefaults), 138 (Rule) — add Clone to each derive list.
        parse_rules (210-215) stays UNCHANGED (the hermetic cached_rules_at in mod.rs calls it).
        All fields are Clone (Pattern is already Clone — pattern.rs:1116)."
  pattern: "just extend the existing #[derive(...)] line: Debug,Deserialize,Default -> Debug,Deserialize,Default,Clone
        (and for Rule: Debug,Deserialize -> Debug,Deserialize,Clone). No field changes."
  gotcha: "do NOT touch validate_rules / get_rules_paths / evaluate / HostContext — only the 3 derives.
        HostContext (rules.rs:367) already derives Clone; don't duplicate."

# REFERENCE — the existing poison-recovery idiom to copy for the new Mutexes
- file: src/core/notifier.rs
  why: "P1.M1.T1.S1 established `STATE.lock().unwrap_or_else(|e| e.into_inner())` everywhere a
        std Mutex is locked in the hot path (notifier.rs:890+). The new CONFIG_CACHE/RULES_CACHE
        Mutexes MUST use the identical idiom — a panic in one caller must not poison the cache
        for the whole process."
  pattern: "CONFIG_CACHE.lock().unwrap_or_else(|e| e.into_inner())  // recovers poison, never propagates"

# EXTERNAL — std primitives used (no new crate; all stable, MSRV 1.88 satisfies)
- url: https://doc.rust-lang.org/std/fs/struct.Metadata.html#method.modified
  why: "Metadata::modified() -> Result<SystemTime> — the mtime read. Returns Err on filesystems
        that don't support mtime (rare); cached_*_at propagates that Err exactly like parse_config
        propagates read errors (caller swallows via .ok())."
- url: https://doc.rust-lang.org/std/time/struct.FileTimes.html
  why: "std::fs::FileTimes + File::set_times (stable 1.75) — used ONLY in the mtime-invalidation
        TEST to deterministically advance mtime (set_modified(now+2s)) without a flaky thread::sleep.
        MSRV 1.88 >> 1.75 so this is available."
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
src/core/
  mod.rs            # EDIT: +CONFIG_CACHE, +RULES_CACHE, +cached_config[_at], +cached_rules_at,
                    #       configured_timing -> cached_config, +Clone on Config, +3 tests
      :18    #[derive(serde::Deserialize, serde::Serialize)] pub struct Config    # + Clone
      :89-91 configured_debounce_ms -> configured_timing
      :97-102 configured_timing()  [parse_config inline]   # -> cached_config()
      :106-112 parse_config()       # KEEP unchanged (uncached core)
  notifier.rs       # EDIT: configured_filter -> cached_config; host_context_for_window -> cached_rules_at; +comment updates
      :80-99  configured_filter()   [parse_config inline]  # -> cached_config()
      :633    perform_handshake parse_rules                # KEEP uncached (once-per-boot)
      :830-835 "intentionally NOT cached" comment          # UPDATE
      :1062-1097 host_context_for_window [parse_rules]     # -> cached_rules_at(&path) (keep Ok/Err match)
  rules.rs          # EDIT: +Clone on RuleSet/HostDefaults/Rule (3 derive lines). parse_rules unchanged.
      :73   #[derive(Debug, Deserialize, Default)] RuleSet        # + Clone
      :97   #[derive(Debug, Default, Deserialize)] HostDefaults    # + Clone
      :138  #[derive(Debug, Deserialize)] Rule                     # + Clone
      :210-215 parse_rules()                # KEEP unchanged (uncached core)
  pattern.rs        # NO CHANGE (Pattern already Clone at :1116)
Cargo.toml         # NO CHANGE (once_cell=1.21 + tempfile=3.0 already deps)
```

### Desired Codebase tree with files added/changed

```bash
src/core/mod.rs     # +5 statics (2 caches, 2 cfg(test) counters, imports), +3 fns
                     #   (cached_config, cached_config_at, cached_rules_at),
                     #   configured_timing body swap, +Clone(Config), +3 tests. NO new file.
src/core/notifier.rs # 2 call-site swaps + 2 comment updates. NO new file.
src/core/rules.rs    # 3 derive-line edits (+Clone). NO new file.
# (no new files; no Cargo.toml; no packaging; no docs)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (the existing hot-config test does NOT validate the cache): test_debounce_ms_is_hot_config
//   (notifier.rs:1451) sets STATE.interval_override — a #[cfg(test)] field that BYPASSES
//   configured_debounce_ms() -> configured_timing() -> parse_config ENTIRELY (see the
//   #[cfg(test)] impl DebounceState::interval at :860-866). It never touches config.toml on disk.
//   -> It will still pass trivially after caching. It CANNOT prove the cache works. NEW tests are
//   mandatory (Tasks 7-9). Do not be lulled by it passing.

// CRITICAL (never cache a parse failure): cached_config_at / cached_rules_at must return Err WITHOUT
//   storing. Rationale: (1) a later valid edit must re-read (hot-config / re-arm); (2) host_context_for_window's
//   RULES_INVALID_NOTIFIED re-arm logic (notifier.rs:1071 "good parse -> store(false)") depends on a fresh
//   re-read after the user fixes rules.toml. Caching an Err would freeze the broken state forever.

// CRITICAL (proving a cache HIT on Linux needs a counter, not mtime control): ext4 mtime has NANOSECOND
//   resolution, so two rapid writes ALWAYS differ in mtime — you CANNOT force a cache hit by controlling
//   mtime/size. The only rigorous, platform-independent observable is a #[cfg(test)] miss counter
//   (CONFIG_CACHE_MISSES) incremented ONLY when cached_*_at falls through to parse_config/parse_rules.
//   Tests assert the DELTA (snapshot before/after) because the counter is global + cumulative across tests.

// CRITICAL (single-threaded tests — AGENTS.md): cargo test --bin qmkonnect -- --test-threads=1 is MANDATORY.
//   The global caches + counters are process-wide; parallel tests would race. The suite already requires
//   single-threaded (shared STATE/COND notifier state — P1.M1.T1.S1).

// CRITICAL (Mutex poison — recover, never propagate): use CONFIG_CACHE.lock().unwrap_or_else(|e| e.into_inner())
//   and RULES_CACHE.lock().unwrap_or_else(|e| e.into_inner()) — the P1.M1.T1.S1 idiom (notifier.rs:890+).
//   A panic in one caller must not poison the cache for the process.

// CRITICAL (keep parse_config / parse_rules UNCACHED for these callers — verified by grep):
//   parse_config: config_parse_error_at (notifier.rs:120 startup), tray.rs:764/1192 + linux_tray.rs:789/818
//     (Settings-dialog UI reads — the user JUST edited the file, MUST see fresh content), main.rs:512 (--validate).
//   parse_rules: notifier.rs:633 (perform_handshake — once per board boot), main.rs:414 (--validate-rules),
//     rules.rs tests (hermetic per-test temp files).
//   Rationale: caching the UI read would show stale content right after a save (the notifier thread's cached
//   mtime entry wouldn't have updated yet if the save raced). The tray reads directly off parse_config for freshness.

// GOTCHA (Config has a MANUAL Default impl — do not replace it): mod.rs:32-48 impl Default for Config by hand
//   (so debounce_ms defaults to 50, not 0). Adding #[derive(Clone)] is independent of Default — safe. Do NOT
//   switch to #[derive(Default)].

// GOTCHA (include the PATH in the cache key, not just mtime): get_config_paths() returns up to 3 candidates
//   (XDG, $HOME, /etc on Linux). If a higher-priority candidate is deleted mid-run, the resolved path changes.
//   Keying on (PathBuf, SystemTime, u64) prevents serving a cached entry from a DIFFERENT file that coincidentally
//   shares mtime+size. This hardens the contract's literal Option<(SystemTime, Config)>.

// GOTCHA (mtime-only change, same byte size — the coarse-mtime edge): a same-second, same-size rewrite (e.g.
//   "debounce_ms = 100" -> "debounce_ms = 999", both same length) on a 1s-resolution fs could be missed. This
//   is the accepted vanishingly-rare edge (contract NOTE). Size as a secondary key catches the common case
//   (different value length). Documented, not worth further complexity.

// GOTCHA (host_context_for_window needs the resolved PATH for its error message): don't hide path resolution
//   behind cached_rules() (no path). Keep get_rules_paths().find(exists)? in host_context_for_window, then call
//   cached_rules_at(&path). The path feeds the eprintln!("...{}...", path.display()) at :1085.

// GOTCHA (no new dependency): once_cell::sync::Lazy is already the repo convention (notifier.rs:2, Cargo.toml
//   once_cell=1.21). Do NOT introduce std::sync::LazyLock (works, but breaks convention) or a cache crate.
//   tempfile=3.0 is already a dev-dep (used by existing tests).
```

## Implementation Blueprint

### Data models and structure

No new data models. The only type-level change is **deriving `Clone`** on the cached
types so the cache can return owned copies:

```rust
// src/core/mod.rs:18  — add Clone
#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub struct Config { /* unchanged fields */ }

// src/core/rules.rs:73 — add Clone
#[derive(Debug, Deserialize, Default, Clone)]
pub struct RuleSet { /* unchanged */ }
// src/core/rules.rs:97 — add Clone
#[derive(Debug, Default, Deserialize, Clone)]
pub struct HostDefaults { /* unchanged */ }
// src/core/rules.rs:138 — add Clone
#[derive(Debug, Deserialize, Clone)]
pub struct Rule { /* unchanged */ }
// Pattern (pattern.rs:1116) ALREADY derives Clone — no change.
```

The cache state types (private statics in `mod.rs`):

```rust
// Key = (resolved path, mtime, byte-size). Value = the parsed struct.
// PathBuf in the key = identity safety (see Gotchas). Poison recovered (P1.M1.T1.S1 idiom).
static CONFIG_CACHE: Lazy<Mutex<Option<(PathBuf, SystemTime, u64, Config)>>> =
    Lazy::new(|| Mutex::new(None));
static RULES_CACHE: Lazy<Mutex<Option<(PathBuf, SystemTime, u64, rules::RuleSet)>>> =
    Lazy::new(|| Mutex::new(None));

// Test-only observables: incremented ONLY on a cache MISS (the fall-through to parse_*).
// Tests assert the delta to prove HIT/MISS precisely (ns-mtime Linux can't fake a hit — see Gotchas).
#[cfg(test)]
static CONFIG_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static RULES_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: EDIT src/core/rules.rs — derive Clone on 3 structs (zero behavior change)
  - RULESET:   line 73  `#[derive(Debug, Deserialize, Default)]`  -> add `, Clone`
  - HOSTDEFAULTS: line 97 `#[derive(Debug, Default, Deserialize)]` -> add `, Clone`
  - RULE:      line 138 `#[derive(Debug, Deserialize)]`           -> add `, Clone`
  - VERIFY: all fields are Clone (Pattern:1116 already Clone; Option<u8>/Vec<String>/bool/Option<bool> trivially Clone).
  - DO NOT touch parse_rules / validate_rules / get_rules_paths / evaluate / HostContext / any field.
  - WHY FIRST: the cache (Task 3) stores owned RuleSet and returns .clone() — won't compile without this.

Task 2: EDIT src/core/mod.rs — derive Clone on Config
  - line 18 `#[derive(serde::Deserialize, serde::Serialize)]` -> add `, Clone`
  - DO NOT touch the manual `impl Default for Config` (mod.rs:32-48) — Clone and Default are independent.
  - WHY EARLY: the cache (Task 3) returns cfg.clone() — won't compile without this.

Task 3: EDIT src/core/mod.rs — add the caches + the cached_* accessors (the core of this task)
  STEP 3a — imports (top of mod.rs, alongside existing `use std::sync::OnceLock;` etc.):
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::SystemTime;
    use once_cell::sync::Lazy;
    #[cfg(test)]
    use std::sync::atomic::{AtomicU64, Ordering};
    (Note: `use std::path::Path;` and `use std::sync::OnceLock;` and `use std::fs;` already exist — keep them.)

  STEP 3b — place the caches + accessors IMMEDIATELY ABOVE `pub fn configured_timing()`
  (mod.rs:96), so they read top-down. Verbatim implementation (see "Implementation Patterns" for
  the full body; summary here):
    - `static CONFIG_CACHE: Lazy<Mutex<Option<(PathBuf, SystemTime, u64, Config)>>>` = Lazy::new(|| Mutex::new(None));
    - `static RULES_CACHE: Lazy<Mutex<Option<(PathBuf, SystemTime, u64, rules::RuleSet)>>>` = Lazy::new(|| Mutex::new(None));
    - `#[cfg(test)] static CONFIG_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);`
    - `#[cfg(test)] static RULES_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);`
    - `pub fn cached_config_at(path: &Path) -> Result<Config, Box<dyn Error>>` — stat (metadata()?.modified()? + .len()),
      lock CONFIG_CACHE (poison-recover), if key (path,mtime,size) matches -> return cfg.clone(); else miss-counter++,
      parse_config(path)?, store (path,mtime,size,cfg.clone()), return cfg. (Errors NOT cached.)
    - `pub fn cached_rules_at(path: &Path) -> Result<rules::RuleSet, Box<dyn Error>>` — identical shape, wrapping
      `rules::parse_rules(path)`, using RULES_CACHE / RULES_CACHE_MISSES.
    - `pub fn cached_config() -> Result<Config, Box<dyn Error>>` —
        match crate::platforms::get_config_paths().into_iter().find(|p| p.exists()) {
            Some(p) => cached_config_at(&p),
            None => Ok(Config::default()),   // no config -> defaults, UNCACHED (cheap; avoids stale-after-create)
        }
  - NAMING: `cached_config` / `cached_config_at` / `cached_rules_at` (mirror parse_config / config_parse_error_at
    `_at` hermetic-core convention). snake_case fns.
  - GOTCHA: never store an Err. The miss-counter increments BEFORE parse (it counts attempted re-parses), and is
    #[cfg(test)]-gated so prod has zero overhead.

Task 4: EDIT src/core/mod.rs — switch configured_timing to cached_config
  - REPLACE the body of `configured_timing()` (mod.rs:97-102):
      // BEFORE:
      crate::platforms::get_config_paths().into_iter().find(|p| p.exists())
          .and_then(|p| parse_config(&p).ok())
          .map(|cfg| (cfg.debounce_ms, cfg.poll_interval_ms))
          .unwrap_or((DEFAULT_DEBOUNCE_MS, DEFAULT_POLL_INTERVAL_MS))
      // AFTER:
      cached_config().ok()
          .map(|cfg| (cfg.debounce_ms, cfg.poll_interval_ms))
          .unwrap_or((DEFAULT_DEBOUNCE_MS, DEFAULT_POLL_INTERVAL_MS))
  - Add a 1-line doc comment noting the mtime-keyed cache (hot-config preserved: an edit invalidates on the next call).
  - WHY this preserves behavior: cached_config() returns Ok(Config::default()) when no path exists -> (50, 0) default;
    on parse error returns Err -> .ok()=None -> same default fallback as before. On success returns the same Config.

Task 5: EDIT src/core/notifier.rs — switch configured_filter to cached_config
  - REPLACE configured_filter()'s body (notifier.rs:80-99):
      // BEFORE:
      let cfg = crate::platforms::get_config_paths().into_iter().find(|p| p.exists())
          .and_then(|p| crate::core::parse_config(&p).ok());
      // AFTER:
      let cfg = crate::core::cached_config().ok();
  - KEEP the rest of configured_filter (the DeviceFilter construction with .and_then(...) / unwrap_or(defaults)) UNCHANGED.
  - UPDATE the comment at notifier.rs:84 ("Read per-call so config changes take effect...") to:
    "Read via cached_config() (mtime-keyed cache in core): re-stats every call, re-reads+re-parses only when the
     file's mtime/size change — so config edits still take effect on the next call (hot-config), without the
     redundant per-call disk read."
  - PRESERVE: the DeviceFilter field defaults (usage_page/usage unwrap_or DEFAULT_*).

Task 6: EDIT src/core/notifier.rs — switch host_context_for_window to cached_rules_at
  - At notifier.rs:1072, REPLACE:
      let rules = match crate::core::rules::parse_rules(&path) {
  - WITH:
      let rules = match crate::core::cached_rules_at(&path) {
  - KEEP the entire Ok/Err match arms UNCHANGED: the Ok arm's `RULES_INVALID_NOTIFIED.store(false, ...)` re-arm,
    the Err arm's one-time `crate::platforms::notify(...)` + verbose eprintln + `return None`. The cache feeds the
    same Result<RuleSet,_> into the same match — semantics identical, just faster on a cache hit.
  - DO NOT change notifier.rs:633 (perform_handshake parse_rules) — stays uncached (once-per-boot, fresh desired).
  - The path variable (`let path = get_rules_paths()...find(exists)?`) is UNCHANGED — cached_rules_at takes it.
  - UPDATE comment block notifier.rs:830-835 ("it is intentionally NOT cached here"):
    note the value is now mtime-cached in cached_config(), but interval() still re-resolves the effective window each
    call (hot-config preserved — an mtime change invalidates ~instantly, not via a TTL delay).

Task 7: CREATE test (in mod.rs `#[cfg(test)] mod tests`) — config cache HIT skips re-parse
  - NAME: `test_config_cache_hit_avoids_reparse` (follow the test_{scenario} convention in the module).
  - BODY (verbatim in "Implementation Patterns"):
    - let dir = tempfile::TempDir::new()?; let path = dir.path().join("config.toml");
    - std::fs::write(&path, "debounce_ms = 100\npoll_interval_ms = 7\n")?;
    - let before = CONFIG_CACHE_MISSES.load(Ordering::SeqCst);
    - let c1 = cached_config_at(&path)?; assert_eq!(c1.debounce_ms, 100); assert_eq!(c1.poll_interval_ms, 7);
    - let after_first = CONFIG_CACHE_MISSES.load(Ordering::SeqCst); assert_eq!(after_first - before, 1, "first call = MISS (parse)");
    - let c2 = cached_config_at(&path)?; assert_eq!(c2.debounce_ms, 100);  // same value
    - let after_second = CONFIG_CACHE_MISSES.load(Ordering::SeqCst);
    - assert_eq!(after_second - after_first, 0, "second call = HIT (no re-parse; mtime+size unchanged)");
  - WHY this is rigorous: the miss-counter directly observes whether parse_config ran. Delta 0 = HIT, provably.

Task 8: CREATE test — config cache INVALIDATES on change (mtime OR size)
  - NAME: `test_config_cache_invalidates_on_change`.
  - BODY (verbatim in Patterns):
    - write "debounce_ms = 100\n"; cached_config_at -> 100; snapshot miss-counter.
    - (a) SIZE change: overwrite "debounce_ms = 2000\n" (longer). cached_config_at -> 2000;
          assert miss-counter delta == 1 (re-parsed) AND value updated.
    - (b) MTIME-only change (same byte size): overwrite "debounce_ms = 999\n" (same length as 100's line);
          use std::fs::FileTimes to set_modified(now + 2s) so mtime advances even on ns-resolution fs;
          cached_config_at -> 999; assert miss-counter delta == 1 AND value updated.
  - GOTCHA: use FileTimes (stable 1.75, MSRV 1.88 OK) for deterministic mtime — NOT thread::sleep (flaky + slow):
        let f = std::fs::File::options().write(true).open(&path)?;
        let times = std::fs::FileTimes::new().set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(2));
        f.set_times(times)?;

Task 9: CREATE test — rules cache HIT + invalidate (parallel to 7+8, for cached_rules_at)
  - NAME: `test_rules_cache_hit_and_invalidation`.
  - BODY: write a minimal valid rules.toml (`[[rule]]\nmatch = "x"\nlayer = 1\n`), cached_rules_at -> Ok; snapshot
    RULES_CACHE_MISSES; call again -> delta 0 (HIT). Overwrite with a different valid ruleset (different size) ->
    cached_rules_at -> delta 1 (MISS) + new layer. Also assert a MALFORMED rules.toml returns Err WITHOUT being cached
    (write garbage -> Err; fix it -> Ok re-read; the cache did NOT freeze the error — verified by the re-read succeeding).

Task 10: VALIDATE (no edits)
  - cargo build                                          # green (derives + caches compile)
  - cargo test --bin qmkonnect -- --test-threads=1       # ALL green; +3 new tests; no regressions
  - git diff --stat                                      # EXACTLY 3 files: mod.rs, notifier.rs, rules.rs. ZERO packaging/Cargo/docs.
  - grep sanity (see Validation Level 3).

Task 11: NEVER do these (out of scope / forbidden)
  - DO NOT cache parse_config for the UI/startup callers (tray.rs, linux_tray.rs, main.rs, config_parse_error_at).
    Those MUST read fresh — the user just saved the file; a stale cache entry would show pre-edit content.
  - DO NOT cache notifier.rs:633 (perform_handshake) or main.rs:414 (--validate-rules) parse_rules.
  - DO NOT cache parse failures (return Err without storing — see Gotchas).
  - DO NOT replace Config's manual Default impl with derive(Default).
  - DO NOT add a new dependency (once_cell::sync::Lazy + tempfile are already deps; std::fs::FileTimes is std).
  - DO NOT use std::sync::LazyLock (breaks the once_cell::sync::Lazy repo convention in notifier.rs).
  - DO NOT change the RULES_INVALID_NOTIFIED re-arm logic or the Ok/Err match arms in host_context_for_window.
  - DO NOT touch packaging/, Cargo.toml, docs/, or any *.iss/*.ps1 (P1.M4.T2.S1 owns those; zero overlap).
  - DO NOT edit PRD.md, tasks.json, prd_snapshot.md, or .gitignore.
```

### Implementation Patterns & Key Details

```rust
// ===== src/core/mod.rs: the cache accessors (Task 3 verbatim) =====
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;
use once_cell::sync::Lazy;
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};

/// mtime+size-keyed cache for the hot-config `config.toml` read. Re-stats every call
/// (cheap `stat()`); re-reads+re-parses only when the resolved file's (path, mtime, size)
/// change. Preserves hot-config: editing config.toml invalidates on the NEXT call
/// (~instant — no TTL delay). Keyed on path too, so a relocated candidate never serves a
/// stale entry from a different file. Shared by configured_timing + configured_filter so
/// the per-send double-read is coalesced to one parse per mtime.
static CONFIG_CACHE: Lazy<Mutex<Option<(PathBuf, SystemTime, u64, Config)>>> =
    Lazy::new(|| Mutex::new(None));

/// mtime+size-keyed cache for the hot-config `rules.toml` read (host_context_for_window).
/// Same contract as CONFIG_CACHE. parse_rules stays uncached for its other callers
/// (perform_handshake, --validate-rules, tests).
static RULES_CACHE: Lazy<Mutex<Option<(PathBuf, SystemTime, u64, crate::core::rules::RuleSet)>>> =
    Lazy::new(|| Mutex::new(None));

// Test-only observables: incremented ONLY on a cache miss (the fall-through to parse_*).
// Tests snapshot the delta to prove HIT/MISS — ns-mtime Linux can't fake a hit via mtime control.
#[cfg(test)]
static CONFIG_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static RULES_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);

/// Hermetic, testable core: cache `config.toml` at `path` by (path, mtime, size).
/// On a cache HIT returns the stored Config clone (no disk read, no parse). On a MISS
/// calls parse_config and stores the result. Parse ERRORS ARE NOT CACHED (a later valid
/// edit must re-read). Mirror of parse_config / config_parse_error_at's `_at` convention.
pub fn cached_config_at(path: &Path) -> Result<Config, Box<dyn Error>> {
    let meta = path.metadata()?;
    let mtime = meta.modified()?;
    let size = meta.len();
    {
        // Poison-recover (P1.M1.T1.S1 idiom) — never propagate a panic from one caller.
        let cache = CONFIG_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((cp, cm, cs, cfg)) = cache.as_ref() {
            if cp == path && *cm == mtime && *cs == size {
                return Ok(cfg.clone());
            }
        }
    }
    #[cfg(test)]
    CONFIG_CACHE_MISSES.fetch_add(1, Ordering::SeqCst);
    let cfg = parse_config(path)?;
    *CONFIG_CACHE.lock().unwrap_or_else(|e| e.into_inner()) =
        Some((path.to_path_buf(), mtime, size, cfg.clone()));
    Ok(cfg)
}

/// Hermetic, testable core for `rules.toml` — identical shape to cached_config_at.
pub fn cached_rules_at(path: &Path) -> Result<crate::core::rules::RuleSet, Box<dyn Error>> {
    let meta = path.metadata()?;
    let mtime = meta.modified()?;
    let size = meta.len();
    {
        let cache = RULES_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((cp, cm, cs, rs)) = cache.as_ref() {
            if cp == path && *cm == mtime && *cs == size {
                return Ok(rs.clone());
            }
        }
    }
    #[cfg(test)]
    RULES_CACHE_MISSES.fetch_add(1, Ordering::SeqCst);
    let rs = crate::core::rules::parse_rules(path)?;
    *RULES_CACHE.lock().unwrap_or_else(|e| e.into_inner()) =
        Some((path.to_path_buf(), mtime, size, rs.clone()));
    Ok(rs)
}

/// Resolve the first existing config.toml candidate and return it via the mtime cache.
/// When NO candidate exists, returns Config::default() WITHOUT caching (cheap; and avoids
/// serving a stale default after the user later creates the file). Errors from
/// cached_config_at propagate (callers swallow via .ok() — same as today's parse_config).
pub fn cached_config() -> Result<Config, Box<dyn Error>> {
    match crate::platforms::get_config_paths().into_iter().find(|p| p.exists()) {
        Some(p) => cached_config_at(&p),
        None => Ok(Config::default()),
    }
}

// ===== src/core/mod.rs: configured_timing (Task 4) =====
pub fn configured_timing() -> (u64, u64) {
    cached_config()
        .ok()
        .map(|cfg| (cfg.debounce_ms, cfg.poll_interval_ms))
        .unwrap_or((DEFAULT_DEBOUNCE_MS, DEFAULT_POLL_INTERVAL_MS))
}

// ===== src/core/notifier.rs: configured_filter (Task 5) =====
fn configured_filter() -> DeviceFilter {
    let cfg = crate::core::cached_config().ok();   // mtime-keyed cache; hot-config preserved
    DeviceFilter {
        vendor_id: cfg.as_ref().and_then(|c| c.vendor_id),
        product_id: cfg.as_ref().and_then(|c| c.product_id),
        usage_page: cfg.as_ref().and_then(|c| c.usage_page).unwrap_or(qmk_notifier::DEFAULT_USAGE_PAGE),
        usage: cfg.as_ref().and_then(|c| c.usage).unwrap_or(qmk_notifier::DEFAULT_USAGE),
    }
}

// ===== src/core/notifier.rs: host_context_for_window (Task 6 — only the match scrutinee changes) =====
// line 1072:
let rules = match crate::core::cached_rules_at(&path) {   // was: crate::core::rules::parse_rules(&path)
    Ok(r) => { RULES_INVALID_NOTIFIED.store(false, Ordering::SeqCst); r }   // KEEP — good parse re-arms
    Err(e) => { /* KEEP the one-time notify + eprintln + return None */ }
};
```

```rust
// ===== TESTS (Tasks 7-9, verbatim — go in mod.rs `#[cfg(test)] mod tests`) =====
#[test]
fn test_config_cache_hit_avoids_reparse() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "debounce_ms = 100\npoll_interval_ms = 7\n").unwrap();

    let before = CONFIG_CACHE_MISSES.load(Ordering::SeqCst);
    let c1 = cached_config_at(&path).unwrap();
    assert_eq!(c1.debounce_ms, 100);
    assert_eq!(c1.poll_interval_ms, 7);
    let after_first = CONFIG_CACHE_MISSES.load(Ordering::SeqCst);
    assert_eq!(after_first - before, 1, "first call is a MISS -> parse_config runs once");

    // Second call, file unchanged -> cache HIT -> parse_config must NOT run.
    let c2 = cached_config_at(&path).unwrap();
    assert_eq!(c2.debounce_ms, 100, "HIT returns the same parsed value");
    let after_second = CONFIG_CACHE_MISSES.load(Ordering::SeqCst);
    assert_eq!(
        after_second - after_first,
        0,
        "second call is a HIT -> no re-parse (mtime+size unchanged)"
    );
}

#[test]
fn test_config_cache_invalidates_on_change() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "debounce_ms = 100\n").unwrap();
    let _ = cached_config_at(&path).unwrap();          // prime the cache
    let before = CONFIG_CACHE_MISSES.load(Ordering::SeqCst);

    // (a) SIZE change: longer value -> different cache key -> re-parse.
    std::fs::write(&path, "debounce_ms = 2000\n").unwrap();
    let c = cached_config_at(&path).unwrap();
    assert_eq!(c.debounce_ms, 2000, "size change must invalidate");
    let after_size = CONFIG_CACHE_MISSES.load(Ordering::SeqCst);
    assert_eq!(after_size - before, 1, "size change -> MISS (re-parse)");

    // (b) MTIME-only change: same byte length ("100" -> "999"), advance mtime deterministically.
    std::fs::write(&path, "debounce_ms = 999\n").unwrap();
    let f = std::fs::File::options().write(true).open(&path).unwrap();
    let times = std::fs::FileTimes::new()
        .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(2));
    f.set_times(times).unwrap();
    drop(f);
    let c = cached_config_at(&path).unwrap();
    assert_eq!(c.debounce_ms, 999, "mtime change (same size) must invalidate");
    let after_mtime = CONFIG_CACHE_MISSES.load(Ordering::SeqCst);
    assert_eq!(after_mtime - after_size, 1, "mtime change -> MISS (re-parse) — hot-config preserved");
}

#[test]
fn test_rules_cache_hit_and_invalidation_and_no_error_caching() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("rules.toml");
    std::fs::write(&path, "[[rule]]\nmatch = \"x\"\nlayer = 1\n").unwrap();

    // HIT: second call with unchanged file skips re-parse.
    let before = RULES_CACHE_MISSES.load(Ordering::SeqCst);
    let _ = cached_rules_at(&path).unwrap();
    let _ = cached_rules_at(&path).unwrap();
    let after_two = RULES_CACHE_MISSES.load(Ordering::SeqCst);
    assert_eq!(after_two - before, 1, "first=MISS, second=HIT -> exactly one parse");

    // INVALIDATE: different size -> re-parse.
    std::fs::write(&path, "[[rule]]\nmatch = \"y\"\nlayer = 22\n").unwrap();
    let rs = cached_rules_at(&path).unwrap();
    let after_three = RULES_CACHE_MISSES.load(Ordering::SeqCst);
    assert_eq!(after_three - after_two, 1, "size change -> MISS");
    assert_eq!(rs.rules[0].layer, Some(22), "invalidation picked up the new value");

    // NO ERROR CACHING: a malformed file returns Err and is NOT stored, so fixing it re-reads.
    std::fs::write(&path, "this is = = not valid toml\n").unwrap();
    assert!(cached_rules_at(&path).is_err(), "malformed -> Err");
    std::fs::write(&path, "[[rule]]\nmatch = \"z\"\nlayer = 3\n").unwrap();
    let rs = cached_rules_at(&path).expect("error was NOT cached -> fixed file re-reads cleanly");
    assert_eq!(rs.rules[0].layer, Some(3));
}
```

### Integration Points

```yaml
CONFIG_CACHE / RULES_CACHE (NEW statics in src/core/mod.rs):
  - placement: immediately above `pub fn configured_timing()` (mod.rs:96)
  - key type: `(PathBuf, SystemTime, u64)` + value (Config / rules::RuleSet)
  - lock idiom: `.lock().unwrap_or_else(|e| e.into_inner())` (P1.M1.T1.S1 poison recovery)
CONFIGURED_TIMING (mod.rs:97): body swap parse_config -> cached_config. Behavior identical (defaults on no-file/parse-err).
CONFIGURED_FILTER (notifier.rs:80): body swap parse_config -> cached_config. DeviceFilter shape unchanged.
HOST_CONTEXT_FOR_WINDOW (notifier.rs:1072): scrutinee swap parse_rules -> cached_rules_at. Match arms unchanged.
DERIVES: Config (mod.rs:18), RuleSet/HostDefaults/Rule (rules.rs:73/97/138) += Clone.
CONSUMES (already present): once_cell::sync::Lazy (Cargo.toml once_cell=1.21), tempfile (dev-dep, existing tests).
SIBLING / PARALLEL (zero conflict): P1.M4.T2.S1 edits packaging/windows/* only (zero Rust) — no overlap.
  P1.M3.T2.S1 (parallel) edits src/core/notifier.rs (perform_handshake) — DIFFERENT function than the 2 call sites
  this PRP touches (configured_filter :80, host_context_for_window :1072); merge cleanly, but BOTH touch notifier.rs
  comments — coordinate so the two edits land in distinct regions (they do: :80/:1072 here vs handshake ~:550-680).
```

## Validation Loop

### Level 1: Build (compiles + no new deps)

```bash
cd /home/dustin/projects/qmkonnect
cargo build
# Expected: green. The new statics, cached_* accessors, and Clone derives compile. If a Clone
# derive fails ("X does not implement Clone"), check every field of that struct (Pattern is
# already Clone; all Rule/Config fields are trivially Clone). If once_cell::Lazy import errors,
# confirm `use once_cell::sync::Lazy;` was added to mod.rs (it's already used in notifier.rs).
git diff --stat
# Expected: EXACTLY 3 files — src/core/mod.rs, src/core/notifier.rs, src/core/rules.rs.
#   ZERO Cargo.toml, ZERO packaging/, ZERO docs/. If anything else appears -> overstepped scope.
git diff -- Cargo.toml packaging/ docs/   # Expected: EMPTY
```

### Level 2: Unit Tests (single-threaded — AGENTS.md MANDATORY)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL green. The 3 NEW cache tests (test_config_cache_hit_avoids_reparse,
#   test_config_cache_invalidates_on_change, test_rules_cache_hit_and_invalidation_and_no_error_caching)
#   pass. ZERO existing tests regress. In particular test_debounce_ms_is_hot_config STILL passes
#   (it bypasses the config path via STATE.interval_override — unaffected, but also uninformative).
# If a NEW test fails on "delta == 0" -> the cache key didn't match (check mtime/size comparison).
# If a NEW test fails on "delta == 1" after an unchanged re-read -> the file's mtime changed between
#   writes (ns-resolution) — that's the point of the miss-counter; re-check the test logic.

# Run ONLY the new tests to iterate fast:
cargo test --bin qmkonnect test_config_cache_ -- --test-threads=1
cargo test --bin qmkonnect test_rules_cache_  -- --test-threads=1
```

### Level 3: Static sanity (the cache is wired correctly)

```bash
cd /home/dustin/projects/qmkonnect
# (a) Only the 2 HOT readers + 1 host_context site use the cache; parse_config/parse_rules stay uncached elsewhere.
grep -n "cached_config\b" src/core/mod.rs src/core/notifier.rs
# Expected: definition in mod.rs + call in configured_timing + call in configured_filter.
grep -n "cached_rules_at" src/core/notifier.rs
# Expected: exactly ONE call (host_context_for_window :1072). NOT notifier.rs:633 (handshake stays uncached).
grep -n "parse_config\b" src/core/notifier.rs src/tray.rs src/linux_tray.rs src/main.rs
# Expected: config_parse_error_at, tray/linux_tray UI reads, main.rs --validate ALL still call parse_config (uncached).
grep -n "parse_rules\b" src/core/notifier.rs src/main.rs
# Expected: notifier.rs:633 (handshake) + main.rs:414 (--validate-rules) still call parse_rules directly (uncached).

# (b) Clone derives present.
grep -n "derive.*Clone" src/core/mod.rs src/core/rules.rs | grep -E "Config|RuleSet|HostDefaults|struct Rule"

# (c) Poison-recovery idiom on both new Mutexes.
grep -n "unwrap_or_else(|e| e.into_inner())" src/core/mod.rs   # both CONFIG_CACHE + RULES_CACHE lock sites

# (d) No new dependency.
git diff Cargo.toml Cargo.lock | head   # Cargo.lock may show nothing (no new dep); Cargo.toml must be UNCHANGED.
```

### Level 4: Hot-config behavior (manual smoke — optional, deferred to a real run)

```bash
# The automated test_config_cache_invalidates_on_change already proves hot-config is preserved
# (mtime change -> re-parse). This manual check is a belt-and-suspenders on a running app:
# 1. cargo run -- -v   (or the platform dev loop in AGENTS.md)
# 2. Edit config.toml's debounce_ms; save (atomic_write from P1.M2 -> mtime advances).
# 3. Switch window focus a few times -> the next send uses the NEW debounce window (~instant, no restart).
# 4. Edit rules.toml; save -> the next host_context_for_window reflects the new rules.
# Expected: edits take effect on the next notification (hot-config preserved), same as before caching.
```

## Final Validation Checklist

### Technical Validation
- [ ] Level 1: `cargo build` green; `git diff --stat` = exactly 3 Rust files; no Cargo/packaging/docs change.
- [ ] Level 2: `cargo test --bin qmkonnect -- --test-threads=1` ALL green; +3 new tests; 0 regressions.
- [ ] Level 3 grep gates pass (cache on the 2 hot readers + host_context; parse_* uncached elsewhere).

### Feature Validation
- [ ] Cache HIT provably skips re-parse: `test_config_cache_hit_avoids_reparse` asserts miss-counter delta 0.
- [ ] Hot-config preserved on size change AND mtime-only change: `test_config_cache_invalidates_on_change`.
- [ ] Rules cache works + errors are never cached: `test_rules_cache_hit_and_invalidation_and_no_error_caching`.
- [ ] `configured_timing` + `configured_filter` share ONE cache entry (coalesced double-read) — verified by code path.
- [ ] `host_context_for_window` Ok/Err match + `RULES_INVALID_NOTIFIED` re-arm logic unchanged (only the scrutinee swapped).
- [ ] `test_debounce_ms_is_hot_config` still passes (it bypasses the config path — unaffected).

### Code Quality Validation
- [ ] Mutex poison recovered via `unwrap_or_else(|e| e.into_inner())` (P1.M1.T1.S1 idiom) on both caches.
- [ ] No parse failure is ever cached (errors return without storing).
- [ ] No new dependency (once_cell + tempfile already present; FileTimes is std).
- [ ] Stale comments at notifier.rs:84 and :830 updated to describe mtime-keyed caching.
- [ ] Follows existing conventions: `once_cell::sync::Lazy`, `_at` hermetic-core split, single-threaded tests.

### Documentation
- [ ] No user-facing docs change (internal optimization — PRD §h2.1 finding #2 is latency-only).
- [ ] Code comments at the cache + the 2 call sites explain the mtime-keyed hot-config contract.

---

## Anti-Patterns to Avoid

- ❌ Don't cache `parse_config`/`parse_rules` for the UI/startup/handshake callers — the user just saved the file; a stale cache entry would show pre-edit content (tray MUST read fresh).
- ❌ Don't cache parse failures — a later valid edit must re-read (re-arm logic depends on it).
- ❌ Don't rely on `test_debounce_ms_is_hot_config` to validate the cache — it uses `STATE.interval_override` and bypasses the config-read path entirely; it CANNOT prove the cache works.
- ❌ Don't try to force a cache HIT by controlling mtime on Linux (ext4 has ns resolution) — use the `#[cfg(test)]` miss-counter instead.
- ❌ Don't propagate Mutex poison — use `unwrap_or_else(|e| e.into_inner())`.
- ❌ Don't add a TTL cache (it re-introduces a hot-config delay); mtime+size keying invalidates on the next call (~instant).
- ❌ Don't replace Config's manual `Default` impl with `derive(Default)` (it would zero `debounce_ms`).
- ❌ Don't introduce `std::sync::LazyLock` or a cache crate — `once_cell::sync::Lazy` is the repo convention.
- ❌ Don't run tests multi-threaded — the global caches + shared STATE require `--test-threads=1` (AGENTS.md).

---

## Confidence Score: 9/10

One-pass success is highly likely: the contract is precise, the design is the literal
recommendation of the authoritative internal research, the caller inventory is grep-verified,
the Clone chain is confirmed, and the testability approach (hermetic `_at` core + miss-counter +
`FileTimes` deterministic mtime) sidesteps the only genuinely tricky part (proving a cache HIT
on ns-mtime Linux). The −1 reserves for: (a) the parallel P1.M3.T2.S1 also editing
notifier.rs — both touch distinct functions, but a merge-order care is needed; (b) the
single-threaded-test suite is timing-sensitive (existing 150ms+50ms sleeps in
`reset_test_state`), so the new tests must not depend on wall-clock mtime (they don't — they
use the counter + FileTimes).
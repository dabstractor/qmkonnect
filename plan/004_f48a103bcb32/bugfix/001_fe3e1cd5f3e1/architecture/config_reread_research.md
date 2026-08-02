# Config / Rules Re-read-on-Every-Window-Change — Research (#2)

## Verdict (TL;DR)

**Confirmed: latency-only, not a correctness bug. Hot-config is intentional and
documented.** On every window focus that survives debouncing, `config.toml` is
re-read **2×** (once for the debounce interval, once for the device filter) and,
when the board is host-capable and `rules.toml` exists, `rules.toml` is re-read
**1×**. No caching of any kind exists for these reads (no `OnceLock`, no
mtime/stat invalidation — verified by `grep` across `src/core` and `src/platforms`).
A short-TTL or mtime-keyed cache would reduce this to effectively one read per
debounce window and would help only on slow/networked filesystems; on a local
SSD each `read_to_string`+`toml::from_str` is sub-millisecond and is dominated
by the HID round-trip that follows.

The reported "many disk reads/sec" is **bounded by the 50 ms debounce window**
(default `DEFAULT_DEBOUNCE_MS = 50`), which collapses rapid-switching bursts.
Worst case ≈ 20 sends/sec (one immediate + one flushed per 50 ms), i.e. up to
~40 `config.toml` reads + ~20 `rules.toml` reads/sec under pathological
switching — still not a correctness or latency problem on local disk.

---

## Files Retrieved

1. `src/core/notifier.rs:80-99` — `configured_filter()`: re-reads `config.toml`
   per call. Comment line 84: *"Read per-call so config changes take effect
   without restarting the service."* This is the explicit hot-config intent.
2. `src/core/notifier.rs:1013-1050` — `host_context_for_window()`: gates on
   `host_capable()` (cheap static `OnceLock`/`AtomicBool`), then resolves a
   `rules.toml` candidate via `.find(|p| p.exists())` and calls `parse_rules()`
   → `fs::read_to_string` + `toml::from_str`.
3. `src/core/notifier.rs:770-813` — `DebounceState` + `interval()`: the worker's
   `interval()` calls `configured_debounce_ms()` (→ `configured_timing()` → a
   full `config.toml` read) **every** flush cycle. Comment line 781-784:
   *"it is re-read from `configured_debounce_ms()` on every notification, so it
   is intentionally NOT cached here — editing `config.toml` takes effect within
   ~3 s with no restart."*
4. `src/core/notifier.rs:876-889` — debounce **worker flush**: calls
   `configured_filter()` + `host_context_for_window()` (the per-send reads).
5. `src/core/notifier.rs:919-967` — `notify_qmk()` immediate-send path: calls
   `state.interval()` (→ `config.toml` read) **and**, when `due`,
   `configured_filter()` (→ `config.toml` read again) + `host_context_for_window()`.
6. `src/core/mod.rs:43-44` — `DEFAULT_DEBOUNCE_MS = 50`, `DEFAULT_POLL_INTERVAL_MS = 0`.
7. `src/core/mod.rs:89-111` — `configured_debounce_ms()` / `configured_timing()` /
   `parse_config()`: `parse_config` is `fs::read_to_string` + `toml::from_str`,
   no cache. `configured_timing` does `get_config_paths().find(exists)` + parse.
8. `src/core/rules.rs:210-215` — `parse_rules()`: `let text = fs::read_to_string(path)?; let rules = toml::from_str(&text)?; validate_rules(&rules)?;` — pure disk read, no cache.
9. `src/core/rules.rs:281-286` — `get_rules_paths()`: derives candidates from
   `get_config_paths()` by swapping the filename to `rules.toml` (in-memory only,
   no IO — but each candidate is then `.exists()`-statted by the caller).
10. `src/platforms/linux.rs:116-139` — `get_config_paths()` builds up to 3
    candidate `config.toml` paths in memory (XDG, `$HOME`, `/etc`); the caller
    stats each with `.exists()` until one is found.
11. `src/platforms/{macos.rs, windows.rs, x11.rs, hyprland.rs}` — each platform's
    focus-change callback calls `notifier::notify_qmk(...)` per event (event-driven,
    not polled — except Hyprland which polls at `poll_interval_ms`, default `0` = off).

---

## The Exact Read Path Per Send

For a focus change that actually emits to the keyboard (immediate-send when
`due`, or the worker flush after coalescing):

```
notify_qmk() / debounce_worker flush
 ├─ state.interval()  ──► configured_debounce_ms()
 │     └─► configured_timing()                       [config.toml READ #1]
 │           └─► get_config_paths()  (in-mem; N stat() syscalls)
 │           └─► parse_config()  → fs::read_to_string + toml::from_str
 ├─ configured_filter()                              [config.toml READ #2]
 │     └─► get_config_paths()  (in-mem; N stat() syscalls)
 │     └─► parse_config()  → fs::read_to_string + toml::from_str
 └─ host_context_for_window()
       ├─ host_capable()  (OnceLock bool, in-mem; no IO)  ── early-exit if false
       ├─ get_rules_paths()  (in-mem) + .find(|p| p.exists())  (N stat() syscalls)
       └─ parse_rules()                                  [rules.toml READ #1]
             └─► fs::read_to_string + toml::from_str + validate_rules
```

So **per emitted send**: 2× `config.toml` + (0 or 1)× `rules.toml` full file
reads, plus up to ~3-6 `stat()` syscalls for path probing (`get_config_paths`
returns up to 3 candidates on Linux/macOS; the first `.exists()` hit short-circuits).

**Gates that limit `rules.toml` reads:** `host_context_for_window` returns `None`
*without touching the disk* when `!host_capable()` (legacy/offline board, set at
handshake) and also returns `None` without `parse_rules` when no `rules.toml`
candidate `.exists()`. So a user with no `rules.toml`, or a non-host-capable
board, pays **zero** `rules.toml` reads. `config.toml` reads have no such gate.

## Does Any Caching Exist?

**No.** Verified by:
- `grep -rn 'OnceLock\|OnceCell\|cached\|cache\|mtime\|modified' src/core/ src/platforms/`
  → the only `OnceLock` is `START: OnceLock<Instant>` (`mod.rs:10`) for the
  uptime clock; no `metadata()`/`.modified()`/filetime invalidation anywhere.
- `HOST_CAPABLE` / `BOARD_HAS_RULES` (`AtomicBool`, `notifier.rs:989`) are
  handshake-state, not config caches — and they are the gate that *avoids*
  rules.toml reads on legacy boards.
- The source comments state the no-cache choice is deliberate
  (`notifier.rs:781` "intentionally NOT cached", `notifier.rs:84` "Read per-call").

## Call Frequency Analysis

**Per focus change → one `notify_qmk` call** (event-driven from the platform's
focus callback: `macos.rs:124/215`, `windows.rs:244`, `x11.rs:139/149`,
`hyprland.rs:451/538`). Hyprland additionally polls at `poll_interval_ms`
(default `0` = disabled).

Every `notify_qmk` call pays **at minimum 1× `config.toml` read** via
`state.interval()` (the debounce-window lookup), regardless of whether the
message is sent immediately or queued.

If the message is **sent immediately** (`due`: ≥ 50 ms since last send, or first
ever), it pays the full per-send cost: 2× `config.toml` + up to 1× `rules.toml`.

If the message is **queued** (within the 50 ms window), no filter/rules read
this call — the reads happen later, once, on the worker flush.

**Worst-case sustained switching:** the 50 ms debounce window caps true sends at
≈ 20/sec. So the absolute upper bound on local-disk IO is ≈ 40 `config.toml` +
20 `rules.toml` full reads/sec, each a small file (typically < 1 KB) plus a TOML
deserialize. On local SSD this is a few hundred microseconds/sec total —
negligible versus the multi-millisecond HID send that follows each. The report's
"many disk reads/sec" is real but **bounded and cheap**.

On a **slow/networked filesystem** (e.g. `config.toml` on an NFS/iCloud/Syncthing
home dir), each `read_to_string` can be tens of ms and the 2× amplification
becomes noticeable latency on the foreground focus thread (the immediate-send
path in `notify_qmk` runs on the platform event thread). This is the only
scenario where the finding is a real problem.

## Hot-Config Design Intent (why they re-read)

Explicit and consistent across the code:
- `notifier.rs:781-785` — `debounce_ms`/`poll_interval_ms` are "hot config ...
  intentionally NOT cached ... editing `config.toml` takes effect within ~3 s
  with no restart." Cross-referenced to PRD §8, ARCHITECTURE.md §10 #4, CONFIG.md §1.2.
- `notifier.rs:84` — `configured_filter`: *"Read per-call so config changes take
  effect without restarting the service."*
- `rules.rs:186-189` doc — `parse_rules` is described as the "host-side-rules
  counterpart to `parse_config`", i.e. same hot-reload contract.
- Test `test_debounce_ms_is_hot_config` (`notifier.rs:1379-1411`) **asserts** the
  no-cache behavior is correct: editing the interval mid-flight must extend the
  coalescing window. Any cache would have to preserve this property.

The validation-on-flush (re-arm malformed-notification, `notifier.rs:1025-1046`)
also depends on fresh reads: a re-read of a *now-valid* `rules.toml` re-arms the
"once per breakage" desktop notification.

## Latency vs Correctness

- **Correctness:** unaffected. The re-read is required for the documented hot-config
  contract; reads are idempotent (pure parse + evaluate). A `config.toml`/`rules.toml`
  parse error degrades gracefully (`configured_filter`/`configured_timing` swallow via
  `.ok()` and fall back to defaults; `host_context_for_window` falls back to string-only).
- **Latency:** the only real cost. Local disk: negligible (sub-ms, hidden behind the
  HID send). Networked/slow home dir: can add perceptible latency to the focus thread
  on the immediate-send path. Mitigation = a short-TTL or **mtime-keyed** cache that
  still honors hot-config (re-stat, re-read only on mtime change). Note the
  `~3 s` hot-config SLO cited in the comments gives a TTL cache plenty of headroom.

## Recommendation (for whoever fixes this)

1. **Prefer mtime-keyed caching over fixed TTL** — `Path::metadata()?.modified()?`
  changes exactly when the user edits the file, so hot-config stays instant rather
  than being delayed by a TTL window. A single `Mutex<Option<(mtime, Config)>>` per
  of `config.toml` / `rules.toml` covers all three readers
  (`configured_timing`, `configured_filter`, `host_context_for_window`).
2. **Coalesce the config.toml double-read first** — `notify_qmk` currently reads
  `config.toml` twice per immediate send (once for the interval, once for the
  filter). A single cached `Config` snapshot shared by `configured_timing` +
  `configured_filter` halves the read count with zero behavior change.
3. Keep the parse-error → graceful-fallback semantics and the
  `test_debounce_ms_is_hot_config` invariant intact (any cache must re-evaluate
  on file change; the existing test would catch a broken cache).
4. Priority: **low**. This is an optimization for slow-fs users, not a bug fix.
  The hot-config SLO (`~3 s`) is the real constraint, and even a 100-500 ms TTL
  satisfies it comfortably.

## Start Here

Open `src/core/notifier.rs:876-889` (worker flush) and `:919-967` (`notify_qmk`
immediate path) — these are the two call sites that pay the per-send read cost,
and the natural place to thread a shared cache. Then `src/core/mod.rs:89-111`
(`configured_timing`/`parse_config`) and `src/core/rules.rs:210-215`
(`parse_rules`) are the two functions a cache would wrap.
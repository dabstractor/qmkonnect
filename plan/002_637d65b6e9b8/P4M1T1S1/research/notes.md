# Research Notes — P4.M1.T1.S1: Add `send_command()` to `Notifier` trait + `QmkNotifier` impl

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This task ADDS typed-command transport to
> `src/core/notifier.rs`: (1) a new `fn send_command(...)` on the `Notifier`
> trait, (2) its `QmkNotifier` impl (the real one that calls `qmk_notifier::run`),
> and (3) a `MockNotifier` impl with a call-sequence recorder + `reset_test_state`
> clearing. **Consumes:** the `qmk_notifier` v0.3.0 public API (`RunCommand`,
> `CommandResponse`, `HostOs`, `RunParameters`, `run`) — whose exact types are
> reproduced verbatim in §1. **Consumed downstream by:** P4.M2.T1.S1 (handshake:
> `QueryInfo`/`QueryCallback`/`SetOs`) and P4.M3.T1.S1 (host-context send:
> `ApplyHostContext`).
>
> **PARALLEL-EXECUTION NOTE:** P3.M1.T2.S1 (the rules `evaluate()` engine) is
> being implemented in parallel and touches `src/core/rules.rs` ONLY. This task
> touches `src/core/notifier.rs` ONLY. The two files are disjoint; no merge
> conflict. P3.M1.T2.S1's `HostContext`/`evaluate` are NOT consumed here (they
> are consumed by P4.M3.T1.S1, which calls both `evaluate()` and
> `send_command(ApplyHostContext{…})`).

---

## 0. Current `src/core/notifier.rs` state (verified, line numbers)

File: `/home/dustin/projects/qmkonnect/src/core/notifier.rs` (588 lines, 21 KB).

### 0.1 The `Notifier` trait (lines 12–14) — THE thing this task extends

```rust
// Trait to abstract the notification functionality
pub trait Notifier: Send + Sync {
    fn notify(&self, message: String) -> Result<(), Box<dyn Error + Send + Sync>>;
}
```

- `Send + Sync` supertrait bound (required because the trait object lives behind
  `Arc<Mutex<Box<dyn Notifier>>>` — see §0.4).
- Return type is `Result<(), Box<dyn Error + Send + Sync>>` — the `Box<dyn Error +
  Send + Sync>` is the **crate-wide error idiom** (also used by `notify_qmk`,
  `list_devices`, `startup_device_probe` is infallible).
- **THIS TASK ADDS** a second method (signature in §3.1). It is a REQUIRED method
  (no default body) — see design decision D1.

### 0.2 `DeviceFilter` (lines 24–30) — the `filter` parameter's type (already exists)

```rust
pub struct DeviceFilter {
    pub vendor_id: Option<u16>,   // None = match any (auto-discovery)
    pub product_id: Option<u16>,  // None = match any
    pub usage_page: u16,          // default 0xFF60 (qmk_notifier::DEFAULT_USAGE_PAGE)
    pub usage: u16,               // default 0x61   (qmk_notifier::DEFAULT_USAGE)
}
```

- All four fields `pub`. `configured_filter()` (line 35) re-reads `config.toml`
  per call to build one. `send_command` takes `&DeviceFilter` (borrowed) so the
  caller decides how to resolve it (`configured_filter()` or a hand-built test
  fixture). **No change to `DeviceFilter` itself this task.**

### 0.3 `QmkNotifier::notify` (lines 142–186) — the impl pattern to mirror

```rust
impl Notifier for QmkNotifier {
    fn notify(&self, message: String) -> Result<(), Box<dyn Error + Send + Sync>> {
        let f = configured_filter();
        for attempt in 1..=3 {
            let params = qmk_notifier::RunParameters::new(
                qmk_notifier::RunCommand::SendMessage(message.clone()),
                f.vendor_id, f.product_id, f.usage_page, f.usage,
                false, // verbose
            );
            match qmk_notifier::run(params) {
                Ok(_) => return Ok(()),
                Err(e) => {
                    let error_str = e.to_string().to_lowercase();
                    if error_str.contains("no device found")
                        || error_str.contains("permission denied")
                        || error_str.contains("failed to open")
                    {
                        if attempt < 3 { thread::sleep(Duration::from_millis(100*attempt as u64)); continue; }
                        eprintln!("QMK device unavailable after {} attempts: {}", attempt, e);
                        return Ok(()); // Don't fail the service for device issues
                    }
                    return Err(Box::new(e));   // ← THE error-mapping idiom: Box::new(qmk_notifier::QmkError)
                }
            }
        }
        Ok(())
    }
}
```

**Two idioms to reuse in `send_command`:**
1. **RunParameters construction** — `qmk_notifier::RunParameters::new(command,
   vid, pid, usage_page, usage, verbose)`. Identical 6-arg shape; `send_command`
   passes the `command` it received instead of `SendMessage(message.clone())`.
2. **Error mapping** — `return Err(Box::new(e));` where `e: qmk_notifier::QmkError`.
   Works because `QmkError: std::error::Error + Debug + Send + Sync` (verified in
   §1.5), so `Box::new(e)` coerces to `Box<dyn Error + Send + Sync>`. NO
   `.to_string()`/`From`/custom wrapper needed.

**`notify()` is UNCHANGED this task** (contract: "Keep notify() as-is"). It
already builds `RunParameters` and (under v0.3.0) `run()` returns
`CommandResponse`; `Ok(_)` discards it — that's fine, `notify()`'s contract is
unit (`()`). The discard does NOT change behavior (SendMessage's response was
always ignored by the app). See D2 for why `send_command` does NOT bake in retry.

### 0.4 The global `NOTIFIER` static (lines 188–189) + accessors

```rust
static NOTIFIER: Lazy<Arc<Mutex<Box<dyn Notifier>>>> =
    Lazy::new(|| Arc::new(Mutex::new(Box::new(QmkNotifier) as Box<dyn Notifier>)));

fn get_notifier() -> Arc<Mutex<Box<dyn Notifier>>> { Arc::clone(&NOTIFIER) }

#[cfg(test)]
pub fn set_notifier(notifier: Box<dyn Notifier>) { /* swap the boxed trait object */ }
```

- The trait object (`Box<dyn Notifier>`) is what makes adding a REQUIRED trait
  method safe: both implementors (`QmkNotifier`, `MockNotifier`) are in THIS file,
  so the compiler catches any missing impl at `cargo build`/`cargo test`. **No
  external implementors exist** (rg confirmed: only 2 `impl Notifier for` sites,
  both in notifier.rs). Adding the method is NOT a breaking change for this repo.

### 0.5 `MockNotifier` (lines ~399–415, inside `#[cfg(test)] mod tests`) — extend this

```rust
static MOCK_CALL_COUNT: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));
static MOCK_LAST_MESSAGE: Lazy<StdMutex<Option<String>>> = Lazy::new(|| StdMutex::new(None));

fn reset_global_mock() {
    MOCK_CALL_COUNT.store(0, Ordering::SeqCst);
    *MOCK_LAST_MESSAGE.lock().unwrap() = None;
}

struct MockNotifier;
impl MockNotifier {
    fn new() -> Self { reset_global_mock(); Self }
    fn get_call_count() -> usize { MOCK_CALL_COUNT.load(Ordering::SeqCst) }
    fn get_last_message() -> Option<String> { MOCK_LAST_MESSAGE.lock().unwrap().clone() }
}
impl Notifier for MockNotifier {
    fn notify(&self, message: String) -> Result<(), Box<dyn Error + Send + Sync>> {
        MOCK_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
        *MOCK_LAST_MESSAGE.lock().unwrap() = Some(message);
        Ok(())
    }
}
```

- `StdMutex` = the `use std::sync::Mutex as StdMutex;` alias (line 378) — used for
  the mock's `Option<String>` because the `Mutex` name is shadowed by the
  `std::sync::{Arc, Condvar, Mutex}` import at module top for the debounce state.
  **Reuse `StdMutex`** for the new call-log static.
- The existing mock records only `notify` calls (count + last message). **THIS
  TASK ADDS** a parallel recorder for `send_command`: a `Vec<RunCommand>`
  sequence (the contract: "store sequence of calls for ordering assertions"). See
  §3.3 for the exact static + accessor.

### 0.6 `reset_test_state()` (lines ~424–443) — extend to clear the new log

```rust
fn reset_test_state() {
    thread::sleep(Duration::from_millis(150));      // let in-flight worker flush
    {
        let mut state = STATE.lock().unwrap();
        state.last_sent_time = None;
        state.pending = None;
        state.verbose = false;
        state.interval = Duration::from_millis(50);
        COND.notify_all();
    }
    reset_global_mock();                              // ← calls the mock reset (MOCK_CALL_COUNT, MOCK_LAST_MESSAGE)
    thread::sleep(Duration::from_millis(50));         // let the woken worker settle
}
```

- **THIS TASK EXTENDS** `reset_global_mock()` to ALSO clear the new
  `send_command` call log (so `reset_test_state()` → `reset_global_mock()` clears
  it transitively). One added line: `MOCK_SEND_COMMAND_CALLS.lock().unwrap().clear();`.
- Single-threaded test suite is MANDATORY (shared `STATE`/`COND`/`WORKER`):
  `cargo test --bin qmkonnect -- --test-threads=1` (AGENTS.md).

### 0.7 Other `qmk_notifier::` usages (rg-confirmed, the ONLY sites in `src/`)

```
src/core/notifier.rs:46   .unwrap_or(qmk_notifier::DEFAULT_USAGE_PAGE)   // configured_filter
src/core/notifier.rs:50   .unwrap_or(qmk_notifier::DEFAULT_USAGE)        // configured_filter
src/core/notifier.rs:148  qmk_notifier::RunParameters::new(...)          // QmkNotifier::notify
src/core/notifier.rs:149  qmk_notifier::RunCommand::SendMessage(...)     // QmkNotifier::notify
src/core/notifier.rs:156  qmk_notifier::run(params)                      // QmkNotifier::notify
```

`DEFAULT_USAGE_PAGE`/`DEFAULT_USAGE` exist in BOTH v0.2.1 and v0.3.0 (so
`configured_filter` compiles either way). `RunCommand`/`RunParameters`/`run` also
exist in v0.2.1 — but the **TYPED VARIANTS** (`QueryInfo`, `QueryCallback`,
`SetOs`, `ApplyHostContext`) and `CommandResponse`/`HostOs` exist ONLY in v0.3.0.
See §2 (the compile gate).

---

## 1. The `qmk_notifier` v0.3.0 public API (verbatim, the contract this task codes to)

Source: `/home/dustin/projects/qmk_notifier/src/lib.rs` + `src/error.rs` (local
working tree; `Cargo.toml version = "0.3.0"`). All types are `pub` and re-exported
at the crate root (so `qmk_notifier::RunCommand` etc. resolve).

### 1.1 `RunCommand` (lib.rs:19) — `#[derive(Debug, Clone, PartialEq, Eq)]`

```rust
pub enum RunCommand {
    SendMessage(String),          // legacy: "{class}\x1D{title}" window string
    ListDevices,                  // enumerate HID devices (no I/O)
    QueryInfo,                    // typed 0x01 — proto_ver/feature_flags/callback_count/board_rules_present
    QueryCallback(u8),            // typed 0x02 — read callback name by index
    SetOs(HostOs),                // typed 0x03 — declare host OS
    ApplyHostContext {            // typed 0x05 — push layer + callbacks + clear_board
        layer: Option<u8>,        //   None => 0xFF (clear host layer)
        callbacks: Vec<u8>,       //   full desired enabled callback-id set
        clear_board: bool,        //   true => firmware clears board layer/cmd first
    },
}
```

- **`Clone`** is derived ⇒ the `MockNotifier` can `command.clone()` into its call
  log (the recorder stores owned `RunCommand` values, not references).
- `send_command` takes `command: qmk_notifier::RunCommand` **by value** (move) —
  matches `RunParameters::new`'s first param (also by value). The mock clones
  before recording (it still owns the moved-in value; recording needs a clone
  because the function signature returns the `RunParameters`-bound value... no —
  the mock does NOT build RunParameters; it just records + returns. It receives
  `command` by value, clones it into the log, and the original is dropped. Clean.)

### 1.2 `HostOs` (lib.rs:65) — `#[repr(u8)] #[derive(Debug, Clone, Copy, PartialEq, Eq)]`

```rust
pub enum HostOs { Unsure=0, Linux=1, Windows=2, Macos=3, Ios=4 }
```
- `Copy` ⇒ trivially passable. Mirrors firmware `os_variant_t`. Used by
  `SetOs(HostOs)`. P4.M2.T1.S1 constructs the right variant per platform.

### 1.3 `CommandResponse` (lib.rs:86) — `#[derive(Debug, Clone, PartialEq, Eq)]`

```rust
pub enum CommandResponse {
    Legacy { matched: bool },                                   // string reply: response[0] ∈ {0,1}
    Info {                                                      // QUERY_INFO reply
        proto_ver: u8, feature_flags: u8,
        callback_count: u8, board_rules_present: bool,
    },
    CallbackName { index: u8, name: Option<String> },           // QUERY_CALLBACK reply
    Ack { ok: bool },                                           // SET_OS / APPLY_HOST_CONTEXT reply
    Timeout,                                                    // no reply within read_timeout
}
```
- **The return type of `send_command`** (wrapped in `Result<_, Box<dyn Error +
  Send + Sync>>`). P4.M2 matches on `Info{ proto_ver: 2, .. }`; P4.M3 matches on
  `Ack{ ok: true }`. `Clone + PartialEq` ⇒ the mock's default return and test
  assertions are ergonomic.

### 1.4 `RunParameters` (lib.rs:120) + `run` (lib.rs:418)

```rust
#[derive(Debug, Clone)]
pub struct RunParameters {
    pub command: RunCommand,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub usage_page: u16,
    pub usage: u16,
    pub verbose: bool,
}
impl RunParameters {
    pub fn new(command: RunCommand, vendor_id: Option<u16>, product_id: Option<u16>,
                usage_page: u16, usage: u16, verbose: bool) -> Self { /* field init */ }
}

pub fn run(params: RunParameters) -> Result<CommandResponse, QmkError> { /* dispatch all cmds */ }
```
- `run()` dispatches EVERY `RunCommand` variant (SendMessage, ListDevices, and all
  four typed variants) through ONE send path: build payload → burst-send → parse
  the first captured reply (or `Timeout`). So `send_command` does NOT need to
  special-case any variant — it hands `command` to `RunParameters::new` and `run`
  does the rest. (Verified at lib.rs:418–466: the `command @ (SendMessage | … |
  ApplyHostContext)` arm handles them uniformly.)

### 1.5 `QmkError` (error.rs) — `#[derive(Debug)]`, impls `Display` + `std::error::Error`

```rust
pub enum QmkError {
    HidApiInitError(String),
    DeviceNotFound { vendor_id: Option<u16>, product_id: Option<u16>, usage_page: u16, usage: u16 },
    DeviceOpenError(String),
    InvalidHexValue(String), InvalidDecimalValue(String),
    SendReportError(HidError), HidReadError(String), NoResponseReceived(String),
    MissingRequiredParameter(String), RemovedFeature(String),
    PartialSendError { succeeded: usize, failed: usize },
}
// impl fmt::Display for QmkError { ... }      // human-readable, lowercase substrings used by notify()'s retry classifier
// impl std::error::Error for QmkError {}      // ← THE line that makes Box::new(e) work
```
- **`QmkError: std::error::Error + Debug + Send + Sync`** ⇒ `Box::new(e)` coerces
  to `Box<dyn Error + Send + Sync>` with NO adapter. (`HidError` is `Send + Sync`;
  `String`/primitives are too; enums of Send+Sync fields are Send+Sync.) This is
  the EXACT idiom `notify()` already uses at line 173 (`return Err(Box::new(e));`).

---

## 2. ⚠️ THE COMPILE GATE — qmk_notifier v0.2.1 (pinned) vs v0.3.0 (needed)

### 2.1 The problem (verified)

`/home/dustin/projects/qmkonnect/Cargo.toml:16`:
```toml
qmk_notifier = { package = "qmk_notifier", git = "https://github.com/dabstractor/qmk_notifier", tag = "v0.2.1" }
```
- The pinned **v0.2.1** has `RunCommand` with ONLY `{ SendMessage, ListDevices }`
  and NO `CommandResponse`/`HostOs`/typed variants. (The current `notify()` only
  uses `SendMessage`, so the app compiles against v0.2.1 today.)
- **v0.3.0 is NOT yet tagged/pushed** — `git -C ../qmk_notifier tag` ⇒
  `v0.1.0, v0.2.0, v0.2.1` only; `git ls-remote --tags origin v0.3.0` ⇒ empty.
  P1.M1.T4.S1 (tag v0.3.0) is "Ready" (not done); P4.M1.T2.S1 (pin Cargo.toml →
  v0.3.0) is "Planned". See `plan/002_*/P1M1T4S1/research/notes.md` §6/§9.
- This task's code references `qmk_notifier::RunCommand::QueryInfo`,
  `qmk_notifier::CommandResponse`, etc. — **NONE of which exist in v0.2.1.** So
  `cargo build` / `cargo test` will FAIL with "no variant `QueryInfo`" / "cannot
  find type `CommandResponse`" UNLESS qmk_notifier resolves to v0.3.0.

### 2.2 The workaround (for THIS task's validation to pass)

The local crate `/home/dustin/projects/qmk_notifier` IS the v0.3.0 source
(`Cargo.toml version = "0.3.0"`, all typed types present). Temporarily point
qmkonnect at it via a **path dependency** so cargo resolves v0.3.0 locally:

```toml
# Cargo.toml line 16 — TEMPORARY dev override (revert before commit, or let
# P4.M1.T2.S1 convert to the v0.3.0 git tag once P1.M1.T4.S1 pushes it):
qmk_notifier = { path = "../qmk_notifier" }
```

- `path` deps resolve the LOCAL working tree (v0.3.0) ⇒ the typed types resolve ⇒
  `cargo build`/`cargo test` pass.
- This is a **dev-only override**: the permanent resolution is P4.M1.T2.S1
  (`tag = "v0.3.0"` git dep, which needs P1.M1.T4.S1 to have pushed the tag).
- **DO NOT commit the `path` override** unless coordinated — it's a local-machine
  path. Either (a) implement this task + run validation with the override, then
  revert Cargo.toml so the diff is notifier.rs-only, OR (b) if implementing
  P4.M1.T2.S1 in the same session, leave the git-tag pin and ensure the tag is
  pushed first. The PRP's validation gates assume the override is in place during
  `cargo build`/`cargo test`.

> **The PRP treats this as a documented Dependency Gate (G1), not a silent
> failure.** An implementer who skips it will see compile errors on the typed
> variants and must apply §2.2 before proceeding.

---

## 3. Design decisions (RESOLVED — the contract + rationale)

### D1 — `send_command` is a REQUIRED trait method (no default body)

The contract gives the exact signature with no default. There are exactly TWO
implementors (`QmkNotifier`, `MockNotifier`), both in notifier.rs — a required
method makes the compiler enforce that every `Notifier` supports typed commands,
which is the whole point (P4.M2/P4.M3 rely on it). A default body returning
`Err("not supported")` would let a future impl silently lack the capability and
break P4.M2/P4.M3 at runtime. **Required method.** (Adding a required method is
NOT a breaking change for THIS repo — no external implementors exist.)

### D2 — `send_command` does NOT bake in retry (unlike `notify()`)

`notify()` has 3-attempt retry + Ok-after-exhaustion baked in (legacy, the
contract says "Keep notify() as-is"). The new `send_command` is a **thin transport
wrapper**: build `RunParameters` → `run()` → map `QmkError` → return
`CommandResponse`. NO retry loop. Rationale:
- The contract's LOGIC clause is explicit and minimal ("build RunParameters from
  the command + filter, call qmk_notifier::run(params), map QmkError to boxed
  error, return the CommandResponse") — retry is absent.
- Retry/cache parity ("Retry/cache for the typed command match the string path",
  PRD §5.7/§8(4)) is the **caller's** job — P4.M3.T1.S1 (notify_qmk host-context
  send logic) applies uniform retry to BOTH the string send and the typed send at
  the orchestration layer. Keeping `send_command` retry-free makes it a clean,
  single-responsibility, unit-testable primitive (one `run()` call ⇒ one result).
- The caller (`P4.M3`) controls the graceful-failure policy (log-and-continue vs
  propagate), which differs between the handshake (P4.M2: capability probe,
  failure ⇒ string-only fallback) and the context send (P4.M3: per-window). Baking
  one policy in would force the wrong behavior on the other.

### D3 — `QmkNotifier::send_command` is retry-free AND verbose-free (verbose=false)

Mirror `notify()`'s `false` verbose arg (line 152). Verbose logging belongs to the
orchestration layer (`notify_qmk` already logs `[ts] Notified QMK`); the transport
primitive stays quiet. (If P4.M3 later wants verbose transport logs it can add a
`verbose` param, but the contract pins the signature without one — so `false`.)

### D4 — Mock call recorder: a `Vec<RunCommand>` sequence + optimistic default return

The contract: "implement `send_command()` with a call recorder (store sequence of
calls for ordering assertions). Update `reset_test_state()` to clear the call log."
Design:
```rust
static MOCK_SEND_COMMAND_CALLS: Lazy<StdMutex<Vec<qmk_notifier::RunCommand>>> =
    Lazy::new(|| StdMutex::new(Vec::new()));
// accessor:
fn get_send_command_calls() -> Vec<qmk_notifier::RunCommand> {
    MOCK_SEND_COMMAND_CALLS.lock().unwrap().clone()
}
```
- Stores the **sequence** (push in call order) so P4.M3 can assert "string before
  context" ordering (`calls[0] == SendMessage(..) && matches!(calls[1], ApplyHostContext{..})`).
- `RunCommand: Clone` ⇒ clone-into-log is cheap and correct.
- **Default return: `Ok(qmk_notifier::CommandResponse::Ack { ok: true })`** —
  optimistic success, mirroring `notify()`'s `Ok(())`. For this task's
  ordering-assertion tests the return is irrelevant; P4.M2 (handshake) and P4.M3
  will EXTEND the mock with configurable responses (e.g. a response-queue static
  keyed on command type) for their own happy-path/sad-path tests — that is THEIR
  scope, NOT this task's. Returning a fixed `Ack` does not block them (they add,
  not rewrite) and does not accidentally pass a downstream test for the wrong
  reason (they configure before asserting on response content).

### D5 — `send_command` rustdoc cites the typed-command protocol (Mode A)

Mode-A `///` doc on the trait method explaining: typed-command support; that
`notify()` is the legacy string path and `send_command` is the typed path; that
the caller owns retry/cache (cross-ref PRD §5.7, §8(4)). ` ```rust,ignore `
examples (binary-only crate — no lib.rs; doctests don't compile under `--bin`;
matches the convention in rules.rs/pattern.rs).

---

## 4. Gotchas (pinned for the PRP's "Known Gotchas" section)

- **G1 (THE compile gate):** v0.2.1 (pinned) lacks the typed types. Apply the
  §2.2 path override BEFORE running any validation, or the build fails on
  `QueryInfo`/`CommandResponse`. Documented as the first validation step.
- **G2 (required method, 2 impls only):** adding `send_command` as required is
  safe — both implementors are in notifier.rs. NO external impls (rg-confirmed).
- **G3 (error mapping is `Box::new(e)`, NOT a wrapper):** `QmkError: Error + Send
  + Sync`, so `Err(Box::new(e))` coerces directly. Do NOT build a custom error
  type or `.to_string()` it (loses type info). Reuse notify()'s exact idiom.
- **G4 (keep `notify()` byte-for-byte):** the contract says "Keep notify() as-is."
  Do NOT refactor its retry, do NOT make it call `send_command(SendMessage(..))`
  (that would change the discard-the-result semantics + retry ownership). `notify`
  stays the legacy string path.
- **G5 (mock recorder uses `StdMutex`, not `Mutex`):** the test module aliases
  `use std::sync::Mutex as StdMutex;` (line 378) because `Mutex` is shadowed by
  the module-top `std::sync::{Arc, Condvar, Mutex}` import. Reuse `StdMutex` for
  the new `Vec<RunCommand>` static (consistent with `MOCK_LAST_MESSAGE`).
- **G6 (clone into the log):** `RunCommand` arrives by value (moved). Clone it
  into `MOCK_SEND_COMMAND_CALLS` (`calls.push(command.clone())`) because the
  signature returns the `RunParameters`-bound value... actually the mock does NOT
  build RunParameters, so it could move — but `clone()` is safest (keeps the
  param usable if the mock ever does more). Either works; `clone()` is the
  defensive choice matching `MOCK_LAST_MESSAGE = Some(message)` (which moves the
  String — but RunCommand is Clone-derivable and cheap to clone). **Use `.clone()`.**
- **G7 (reset_test_state clears the log via reset_global_mock):** add
  `MOCK_SEND_COMMAND_CALLS.lock().unwrap().clear();` to `reset_global_mock()` so
  `reset_test_state()` (which calls `reset_global_mock()`) clears it transitively.
  Do NOT duplicate the clear inside `reset_test_state` directly (single source of
  truth = `reset_global_mock`).
- **G8 (single-threaded tests):** `cargo test --bin qmkonnect -- --test-threads=1`
  (shared `STATE`/`COND`/`WORKER`/`NOTIFIER` globals; AGENTS.md). NEVER multi-threaded.
- **G9 (binary-only crate; doctests):** NO lib.rs (`src/main.rs` declares `mod
  core;`). Use ` ```rust,ignore ` for rustdoc examples (don't add runnable
  `use qmkonnect::...` doctests — they won't compile under `--bin`).
- **G10 (no new deps):** `qmk_notifier` is already a dep; `once_cell::sync::Lazy`
  + `std::sync::{Mutex as StdMutex, atomic::AtomicUsize}` are already imported in
  the test module. NO Cargo.toml dep additions (other than the §2.2 temporary
  pin flip, which is not a new crate).

---

## 5. Test plan (appended to `#[cfg(test)] mod tests`)

The existing 6 tests (`test_immediate_send_first_message`,
`test_debounce_subsequent_messages`, `test_send_after_debounce_timeout`,
`test_multiple_rapid_updates`, `test_verbose_mode`, `test_threads_dont_interfere`)
are UNCHANGED — they exercise `notify()` only and remain green (the mock still
implements `notify`). NEW tests for `send_command` (prefix `test_send_command_*`):

1. **`test_send_command_records_call_sequence`** — build a `MockNotifier`, call
   `send_command(QueryInfo, &filter)`, `send_command(ApplyHostContext{..},
   &filter)`, assert `get_send_command_calls() == vec![QueryInfo,
   ApplyHostContext{..}]` (ordering preserved). THIS is the contract's
   "store sequence of calls for ordering assertions."
2. **`test_send_command_reset_clears_log`** — record ≥1 call, call
   `reset_test_state()`, assert `get_send_command_calls().is_empty()` (G7).
3. **`test_send_command_returns_ok_ack_default`** — assert the default return is
   `Ok(CommandResponse::Ack { ok: true })` (D4).
4. **`test_qmk_notifier_send_command_builds_runparameters`** — a UNIT test of the
   real `QmkNotifier::send_command` that does NOT need hardware: construct a
   `DeviceFilter` with a bogus VID/PID, call
   `send_command(QueryInfo, &filter)`, and assert the result is an `Err` whose
   `to_string()` contains "no device found" (proves it built `RunParameters`,
   called `run()`, and mapped `QmkError::DeviceNotFound` → boxed error via G3).
   This validates the real impl's wiring without a keyboard. (Use
   `QmkNotifier` directly — it's a unit struct; no global swap needed. NB: this
   test exercises the REAL `qmk_notifier::run` ⇒ it will attempt HID enumeration;
   a bogus VID/PID yields `DeviceNotFound`, which is exactly what we assert. Safe,
   no device opened, no report sent — `DeviceNotFound` is raised at enumeration,
   before any open/write.)
5. **`test_send_command_notify_unaffected`** — assert `notify()` still works on
   the mock and `get_call_count()`/`get_last_message()` are independent of the
   `send_command` log (the two recorders don't cross-contaminate). Guards G4.

All 5 run under `--test-threads=1`. Tests 1–3 + 5 use `MockNotifier` (no HID);
test 4 uses `QmkNotifier` against a nonexistent filter (enumeration-only, safe on
any machine).

---

## 6. Confidence

**9/10.** The change is small, additive, and every consumed contract is verified
present and reproduced verbatim (trait shape, both impls, the exact v0.3.0 crate
API, the `Box::new(e)` error-mapping idiom). The ONE residual risk is the compile
gate (G1: v0.2.1 vs v0.3.0) — fully mitigated by the §2.2 path-override, called
out as the first validation step. No design ambiguity remains (D1–D5 resolved
with rationale). Downstream consumers (P4.M2/P4.M3) are unblocked and the mock's
fixed-`Ack` default does not paint them into a corner (they extend, not rewrite).
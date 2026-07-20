# Research Notes — P4.M1.T2.S1: Pin qmk_notifier crate to v0.3.0 tag in Cargo.toml

## Task
Change `Cargo.toml` git dependency pin from `tag = "v0.2.1"` → `tag = "v0.3.0"`,
run `cargo update -p qmk_notifier`, verify the build compiles, and update the
Cargo.toml comment (Mode A) noting the bump reason.

## Key finding 1 — the build is CURRENTLY BROKEN (and this task is what fixes it)
`src/core/notifier.rs` already consumes the v0.3.0 crate API (the
`send_command()` trait method + `QmkNotifier` impl were added in **P4.M1.T1.S1**,
commit `ced887a "Implement send_command transport and verification"`), but
`Cargo.toml` was NOT yet bumped. So the tree does not compile against the
still-pinned v0.2.1:

```
$ cargo build
error[E0412]: cannot find type `CommandResponse` in crate `qmk_notifier`
  --> src/core/notifier.rs:55:31
error[E0412]: cannot find type `CommandResponse` in crate `qmk_notifier`
  --> src/core/notifier.rs:232:31
error: could not compile `qmkonnect` (bin "qmkonnect") due to 2 previous errors  (EXIT 101)
```

Only **2** errors, both the same missing type. No other compile errors exist —
so the consumer code is otherwise v0.3.0-compatible (validated below).

## Key finding 2 — CRITICAL: the `v0.3.0` git tag does NOT exist on the remote
Although plan status marks **P1.M1.T4.S1** ("Bump version, update Cargo.toml,
tag v0.3.0") as Complete, the tag has **not** been pushed to GitHub:

```
$ git ls-remote --tags https://github.com/dabstractor/qmk_notifier
cbcd39d... refs/tags/v0.1.0
660c88f... refs/tags/v0.2.0
1b3b7e8... refs/tags/v0.2.1
32986053a674e13220a64f75e395be44257c40ed  refs/tags/v0.2.1^{}   (annotated)
# NO v0.3.0 !
```

Default branch HEAD = `a606f645e4dbca79ab33adc18e2159bd5277123d`, and its
`Cargo.toml` reads `version = "0.3.0"` with the full typed-command API present.
The git log references "v0.3.1 docs/fixes", i.e. work has progressed PAST 0.3.0
on the default branch but nothing has been tagged since v0.2.1.

**Consequence:** pinning `tag = "v0.3.0"` will make `cargo update`/`cargo build`
FAIL with a "revspec ... not found" / tag-not-found error. The prerequisite
(cutting the tag on the qmk_notifier repo) must be resolved first — see PRP
"Critical Gotcha" + Resolution Paths A/B.

## Key finding 3 — the v0.3.0 crate API on default HEAD exactly matches consumer usage
Confirmed by reading `/tmp/qmk_notifier_full/src/lib.rs` (clone of default HEAD):

| Symbol consumer uses | Present at HEAD? |
|---|---|
| `pub enum CommandResponse { Legacy{matched}, Info{proto_ver,feature_flags,callback_count,board_rules_present}, CallbackName{index,name}, Ack{ok}, Timeout }` | ✅ lib.rs:86-112 |
| `pub enum HostOs { Unsure=0, Linux=1, Windows=2, Macos=3 }` | ✅ lib.rs:65 |
| `pub enum RunCommand { SendMessage(String), ListDevices, QueryInfo, QueryCallback(u8), SetOs(HostOs), ApplyHostContext{layer,callbacks,clear_board} }` | ✅ lib.rs:19-44 |
| `pub fn run(RunParameters) -> Result<CommandResponse, QmkError>` (was `Result<(),QmkError>` in v0.2.1) | ✅ lib.rs:404 |
| `RunParameters::new(command, vid, pid, usage_page, usage, verbose)` — **same 6-arg signature** | ✅ lib.rs:135 |
| `DEFAULT_USAGE_PAGE`, `DEFAULT_USAGE` re-exports | ✅ lib.rs (from inner module) |
| `QmkError: std::error::Error` (so it coerces to `Box<dyn Error+Send+Sync>`) | ✅ error.rs:69; `From<HidError>` at :71 |

## Key finding 4 — the `Ok(_) => return Ok(())` arm in notify() is UNAFFECTED
`notify()` (notifier.rs:198) does `match qmk_notifier::run(params) { Ok(_) => return Ok(()), ... }`.
Under v0.2.1 the `Ok(_)` bound `()`; under v0.3.0 it binds a `CommandResponse`.
The arm discards the value, so it still compiles unchanged. Contract point 3
("the Ok(_) match arm still works since CommandResponse is a success value") is
**confirmed** — **no edit to `notify()` is required** for the return-type change.
The only other `qmk_notifier::run` call is `send_command` (notifier.rs:241),
already `Ok(resp) => Ok(resp)` — also fine.

## Key finding 5 — decisive non-destructive API proof (consumer code needs ZERO source changes)
Built a throwaway crate `/tmp/api_proof` that depends on
`qmk_notifier = { git=..., rev = "a606f645..." }` and reproduces the **exact**
API surface `src/core/notifier.rs` uses (send_command body + every
`CommandResponse`/`RunCommand`/`HostOs` variant the mock tests + handshake
reference). Result: **`cargo build` EXIT 0**. This proves that once the dep is
pinned to v0.3.0 (tag or that rev), the qmkonnect build turns green with no
source edits. (The only current failures are the two missing-type E0412s.)

## Scope / consumer inventory
`grep -rn "qmk_notifier" --include=*.rs --include=*.toml .` (excl. target/Cargo.lock)
shows qmk_notifier is referenced in exactly **two** files:
- `Cargo.toml:16` — the pin to bump
- `src/core/notifier.rs` — already v0.3.0-ready (send_command) + legacy notify()

No other module, bench, example, or build script touches the crate. So this task
is genuinely a 1-line pin + lockfile refresh + build verify, modulo the tag
prerequisite.

## Validation commands (verified against AGENTS.md dev loop)
- Build: `cargo build` (expect green; was failing with 2× E0412)
- Tests: `cargo test --bin qmkonnect -- --test-threads=1`
  (AGENTS.md mandates single-threaded because notifier tests share global
  debouncer `STATE`; `reset_test_state()` relies on it.)
- Lockfile refresh after tag change: `cargo update -p qmk_notifier`
- Tag existence check: `git ls-remote --tags https://github.com/dabstractor/qmk_notifier 'refs/tags/v0.3.0'`
  (empty output ⇒ tag missing ⇒ use Resolution Path A or B)

## Files
- `Cargo.toml:13-16` — the dependency line + comment (both edited by this task)
- `src/core/notifier.rs` — NO CHANGES expected; reference only
- `Cargo.lock` — regenerated by `cargo update -p qmk_notifier` (version 0.2.1 → 0.3.0, source `?tag=v0.3.0`)
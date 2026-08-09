// Regression test for the silent-unknown-flag bug (validation report H1).
//
// `qmkonnect`'s CLI is parsed by hand in `src/main.rs::run()`; before the fix,
// an unrecognized flag (e.g. `--bogus-flag-xyz`, or a typo like `--verbos`) was
// silently ignored and — when no other flag matched — fell through to
// `runner.run()` and STARTED THE DAEMON, which never exits. That both broke the
// CLI contract ("unrecognized option should error") and hung the validate.sh
// `--bogus-flag` gate indefinitely.
//
// This spawns the real binary with a bogus flag and asserts it exits non-zero
// quickly. It is bounded: if the fix regresses and the daemon starts, the test
// kills the child and fails after a few seconds instead of hanging the suite.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The fixed parser exits within milliseconds. Give a generous-but-bounded
/// window; if the child is still alive the bug regressed (it started the
/// daemon) — kill it and fail.
const REJECT_DEADLINE: Duration = Duration::from_secs(3);

#[test]
fn unknown_flag_exits_nonzero_and_does_not_hang() {
    rejects_quickly("--bogus-flag-xyz");
}

#[test]
fn typoed_flag_exits_nonzero_and_does_not_hang() {
    // A realistic fat-finger of `--verbose` / `--validate-rules`.
    rejects_quickly("--verbos");
}

fn rejects_quickly(flag: &str) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_qmkonnect"))
        .arg(flag)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn qmkonnect {flag}: {e}"));

    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                assert_ne!(
                    status.code(),
                    Some(0),
                    "qmkonnect {flag} exited 0 — it must be REJECTED (non-zero)"
                );
                return; // rejected as expected
            }
            Ok(None) if start.elapsed() < REJECT_DEADLINE => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Ok(None) => {
                // Still alive after the deadline: the daemon started (regression).
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "qmkonnect {flag} was still alive after {REJECT_DEADLINE:?} — \
                     it started the daemon instead of rejecting the flag \
                     (regression of the unknown-flag fix)"
                );
            }
            Err(e) => panic!("wait failed for qmkonnect {flag}: {e}"),
        }
    }
}

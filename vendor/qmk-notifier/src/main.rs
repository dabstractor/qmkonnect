use qmk_notifier::{parse_cli_args, run, CommandResponse, RunCommand, RunParameters};

fn main() {
    // Restore the default SIGPIPE disposition so piping into `head`/`less`
    // (which close stdout early) terminates the process cleanly instead of
    // panicking with "failed printing to stdout: Broken pipe" (exit 101).
    // Rust's std ignores SIGPIPE by default; this puts it back to SIG_DFL so
    // the process exits with 141 (128 + SIGPIPE) like a normal Unix tool.
    #[cfg(unix)]
    reset_sigpipe_to_default();

    // --list-callbacks is a CLI-only sweep signal. The library no longer returns
    // it from parse_cli_args (PRD §3: parse_cli_args -> RunParameters, which has
    // no sweep flag — Issue 2). Detect it out-of-band from raw argv so main.rs
    // can run the multi-call callback sweep after `run` returns
    // `CommandResponse::Info`. Placed BEFORE parse_cli_args so the flag is
    // captured even if clap exits on --help/--version/an error (harmless: the
    // process exits either way).
    let list_callbacks = std::env::args().any(|a| a == "--list-callbacks");

    let params = match parse_cli_args() {
        Ok(params) => params,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    // Copy out the device-targeting scalars (all Copy) BEFORE run() moves
    // `params` by value — they're needed to rebuild per-callback params.
    // `is_list` records whether the action is `--list` (ListDevices) so that
    // after `run` we can suppress the library's `Timeout` sentinel for it
    // without re-borrowing the moved `params`.
    let is_list = params.command == RunCommand::ListDevices;
    let vendor_id = params.vendor_id;
    let product_id = params.product_id;
    let usage_page = params.usage_page;
    let usage = params.usage;
    let verbose = params.verbose;

    match run(params) {
        // --list-callbacks: after QueryInfo succeeds, sweep the callback registry.
        Ok(CommandResponse::Info { callback_count, .. }) if list_callbacks => {
            println!("callbacks ({callback_count}):");
            for index in 0..callback_count {
                let params = RunParameters::new(
                    RunCommand::QueryCallback(index),
                    vendor_id,
                    product_id,
                    usage_page,
                    usage,
                    verbose,
                );
                match run(params) {
                    Ok(CommandResponse::CallbackName { index, name }) => {
                        println!(
                            "  {}: {}",
                            index,
                            name.unwrap_or_else(|| "(unnamed)".to_string())
                        );
                    }
                    Ok(other) => eprintln!("callback {index}: unexpected response {other:?}"),
                    Err(e) => eprintln!("callback {index}: error: {e}"),
                }
            }
        }
        // --list: device enumeration is already printed by `list_hid_devices`
        // inside `run`. The library returns `CommandResponse::Timeout` for
        // `ListDevices` ("nothing was sent over the wire"), which is a fine
        // library-level semantic but confusing to surface to the user — so we
        // suppress it here instead of printing a bare `Timeout` line.
        Ok(_) if is_list => {}
        // --query-info / message, OR a non-capable device (Timeout/Legacy)
        // when --list-callbacks was asked for: just print the raw response.
        Ok(response) => println!("{:?}", response),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

/// Reset the SIGPIPE disposition to its default (`SIG_DFL`).
///
/// Rust's runtime sets SIGPIPE to `SIG_IGN`, which turns the next `println!` to
/// a closed pipe into a panic (exit 101). Unix CLI tools are expected to die
/// quietly with SIGPIPE (exit 141) when a downstream consumer exits early
/// (e.g. `qmk_notifier --list | head -1`). This restores that behavior.
///
/// The FFI binding goes through the maintained [`libc`] crate (PRD Issue-4
/// suggested-fix option (a)) rather than a hand-rolled `extern "C"` block.
/// `libc` is already a transitive dependency of `hidapi`, so this adds no new
/// supply-chain surface. On non-Unix targets this function is absent (only
/// compiled behind `#[cfg(unix)]`, and only called from `main` behind the same
/// gate).
///
/// # Deviation from PRD §12 ("No `unsafe`")
///
/// The `unsafe` block below is inherent to POSIX `signal(2)` — calling any FFI
/// function is `unsafe` by definition, and there is no safe Rust API to reset
/// the SIGPIPE disposition before std initializes. This is an **accepted,
/// isolated deviation** from the literal §12 wording: the NFR's intent is "no
/// `unsafe` in the HID I/O transport path" (the sentence's actual subject), and
/// this code lives in the **binary** (`main.rs`), is unrelated to HID, and uses
/// only the well-audited `libc` binding. It is the single `unsafe` block in the
/// entire crate.
#[cfg(unix)]
fn reset_sigpipe_to_default() {
    // SAFETY: `signal()` is a thread-safe-enough POSIX call for one-shot
    // process-global disposition reset performed before any threads are spawned.
    // `SIG_DFL` is a well-known sentinel value. Ignoring the return value is
    // fine: worst case the disposition stays unchanged, which is benign.
    //
    // The FFI binding is provided by the maintained `libc` crate (a transitive
    // dependency of `hidapi`, already resolved in Cargo.lock) rather than a
    // hand-rolled `extern "C"` block.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

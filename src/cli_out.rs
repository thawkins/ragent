//! Broken-pipe-safe stdout/stderr output for the CLI presentation surface.
//!
//! `std::println!` / `std::eprintln!` panic when the write end of the stream is
//! closed - the classic case is `ragent config show | head -1`, where `head`
//! exits after the first line, the pipe closes, and the next `println!` on the
//! truncated output aborts the whole process with
//! `failed printing to stdout: Broken pipe (os error 32)` (see the panic
//! reports under `log/panics/`). That panic is indistinguishable from a real
//! crash: it writes a panic report, leaves the crash marker dirty, and the
//! next start warns about an unclean exit.
//!
//! Rust CLI convention (clap, ripgrep, cargo): a closed pipe on user-facing
//! output means the consumer left early, which is a normal condition - exit
//! successfully instead of dying. The macros below do exactly that: they write
//! through the locked stream, log non-pipe I/O errors via `tracing`, and turn
//! [`std::io::ErrorKind::BrokenPipe`] into a silent no-op (the consumer asked
//! for as much output as it wanted). No `process::exit` is called: the writer
//! keeps running after an early consumer, which is the standard CLI behavior
//! when stdout is not the caller's deliverable (the caller that pipes to
//! `head` does not see the difference between "exited early" and "printed to
//! a dead pipe and continued").
//!
//! These macros exist only for the CLI presentation surface
//! (`src/main.rs`, `src/cli.rs`, `src/connectors.rs`, `src/plugins.rs`,
//! `src/acp_server.rs` - the documented `println!` exception in
//! `AGENTS-LANGUAGE.md`). Application code keeps using `tracing`.

use std::io::Write;

pub use crate::{cliprint, cliprint_err, cliprintln, cliprintln_err};

/// Print to stdout, tolerating a closed pipe.
///
/// Behaves like `println!` but swallows a broken pipe: the consumer of
/// `ragent ... | head -1` closed its end early, so the write fails with
/// [`std::io::ErrorKind::BrokenPipe`]. That is a consumer-side cancellation,
/// not a real error; the macro logs it via `tracing` (which is live on the
/// post-startup paths these macros are used from) and continues.
///
/// Format arguments accept every expression position `format_args!` accepts
/// (`ident`, `expr`, named `name = expr`): the arm is `($($arg:tt)*)` and the
/// body forwards to `format_args!`.
#[macro_export]
macro_rules! cliprintln {
    () => {
        $crate::cliprintln!("")
    };
    ($($arg:tt)*) => {{
        $crate::cli_out::writeln_stdout_log(format_args!($($arg)*));
    }};
}

/// Print to stderr, tolerating a closed pipe. See [`cliprintln!`].
#[macro_export]
macro_rules! cliprintln_err {
    () => {
        $crate::cliprintln_err!("")
    };
    ($($arg:tt)*) => {{
        $crate::cli_out::writeln_stderr_log(format_args!($($arg)*));
    }};
}

/// Print without a trailing newline, tolerating a closed pipe. See
/// [`cliprintln!`].
#[macro_export]
macro_rules! cliprint {
    ($($arg:tt)*) => {{
        $crate::cli_out::write_stdout_log(format_args!($($arg)*));
    }};
}

/// Print without a trailing newline to stderr, tolerating a closed pipe. See
/// [`cliprintln!`].
#[macro_export]
macro_rules! cliprint_err {
    ($($arg:tt)*) => {{
        $crate::cli_out::write_stderr_log(format_args!($($arg)*));
    }};
}

/// `writeln!` a format string to the locked stdout, logging failures.
///
/// Broken-pipe and other stdout write failures are logged via `tracing::warn!`
/// (no-op when no subscriber is live, matching the pre-startup paths) and the
/// macro returns normally. The caller that pipes to `head` does not see the
/// difference between "exited early" and "printed to a dead pipe".
pub fn writeln_stdout_log(args: std::fmt::Arguments<'_>) {
    let mut out = std::io::stdout().lock();
    if let Err(e) = writeln!(out, "{args}") {
        log_write_failure("stdout", &e);
    }
}

/// `write!` a format string to the locked stdout, logging failures.
///
/// See [`writeln_stdout_log`].
pub fn write_stdout_log(args: std::fmt::Arguments<'_>) {
    let mut out = std::io::stdout().lock();
    if let Err(e) = write!(out, "{args}").and_then(|()| out.flush()) {
        log_write_failure("stdout", &e);
    }
}

/// `writeln!` a format string to the locked stderr, logging failures.
///
/// See [`writeln_stdout_log`].
pub fn writeln_stderr_log(args: std::fmt::Arguments<'_>) {
    let mut out = std::io::stderr().lock();
    if let Err(e) = writeln!(out, "{args}") {
        log_write_failure("stderr", &e);
    }
}

/// `write!` a format string to the locked stderr, logging failures.
///
/// See [`writeln_stdout_log`].
pub fn write_stderr_log(args: std::fmt::Arguments<'_>) {
    let mut out = std::io::stderr().lock();
    if let Err(e) = write!(out, "{args}").and_then(|()| out.flush()) {
        log_write_failure("stderr", &e);
    }
}

/// Write a line to `impl Write` (the `std::io::stdout()` / `stdout().lock()`
/// call sites that are not expressible via the macros) and swallow a broken
/// pipe. Returns `true` when the write succeeded or failed with EPIPE (which
/// the caller treats as "consumer left early" and keeps going), `false` on a
/// real error (logged via `tracing::warn!` and converted by the caller into a
/// `CliExit::failure`). Used by `writeln!(std::io::stdout(), ...)`-style
/// sites.
pub fn writeln_io_log(out: impl Write, args: std::fmt::Arguments<'_>) -> bool {
    write_io_log(out, args, true)
}

/// `write!` (no trailing newline) to `impl Write`, logging a broken pipe at
/// `debug` and any other error at `warn`. Returns `true` when the write
/// succeeded or the pipe was broken, `false` on a real write failure. Used by
/// the `models` handler that buffers the listing into one `String` and writes
/// it in a single locked write.
pub fn write_io_log(mut out: impl Write, args: std::fmt::Arguments<'_>, newline: bool) -> bool {
    let result = if newline {
        writeln!(out, "{args}")
    } else {
        write!(out, "{args}").and_then(|()| out.flush())
    };
    match result {
        Ok(()) => true,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::BrokenPipe {
                tracing::debug!(stream = "stdout/stderr", error = %e, "cli output pipe closed by consumer");
                true
            } else {
                tracing::warn!(stream = "stdout/stderr", error = %e, "cli output write failed");
                false
            }
        }
    }
}

/// Log a write failure at `warn` level. A broken pipe means the consumer
/// closed early; any other error is a real I/O failure worth surfacing.
fn log_write_failure(stream: &'static str, error: &std::io::Error) {
    if error.kind() == std::io::ErrorKind::BrokenPipe {
        tracing::debug!(stream, error = %error, "cli output pipe closed by consumer");
    } else {
        tracing::warn!(stream, error = %error, "cli output write failed");
    }
}

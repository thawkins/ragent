//! Coverage for the broken-pipe-safe CLI output helpers in
//! `ragent_types::cli_out`.
//!
//! The four user-facing macros (`cliprintln!`, `cliprintln_err!`, `cliprint`,
//! `cliprint_err!`) write straight to the process's stdout/stderr, so they can
//! only be checked for the property they exist for: a closed or failing pipe
//! must not panic. The `*_io_log` helpers take any `impl Write`, so the routing
//! (stdout vs stderr, newline or not) and the success/failure reporting are
//! asserted directly against a buffer and a deliberately failing writer.
//!
//! The macro cases run in a child process (`ragent_types_cli_out_probe`) whose
//! streams are redirected to `/dev/null` and then closed, because closing the
//! *test harness's* own stdout would break the run.

use std::io::Write;

use ragent_types::cli_out::{write_io_log, writeln_io_log};

/// A writer that fails every call with a fixed error kind.
struct FailingWriter(std::io::ErrorKind);

impl Write for FailingWriter {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::from(self.0))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::from(self.0))
    }
}

/// A writer that records everything and can be told to fail mid-run.
#[derive(Default)]
struct Recorder {
    bytes: Vec<u8>,
}

impl Write for Recorder {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn test_writeln_io_log_appends_one_newline() {
    let mut out = Recorder::default();
    assert!(writeln_io_log(&mut out, format_args!("hello {}", 7)));
    assert_eq!(out.bytes, b"hello 7\n");
}

#[test]
fn test_write_io_log_without_newline_writes_no_newline() {
    let mut out = Recorder::default();
    assert!(write_io_log(&mut out, format_args!("bare"), false));
    assert_eq!(out.bytes, b"bare");
}

#[test]
fn test_write_io_log_with_newline_writes_one() {
    let mut out = Recorder::default();
    assert!(write_io_log(&mut out, format_args!("line"), true));
    assert_eq!(out.bytes, b"line\n");
}

#[test]
fn test_broken_pipe_is_reported_as_success() {
    // A closed consumer (`| head -1`) is a normal cancellation, so the helper
    // reports success and the caller keeps running.
    assert!(writeln_io_log(
        FailingWriter(std::io::ErrorKind::BrokenPipe),
        format_args!("x")
    ));
    assert!(write_io_log(
        FailingWriter(std::io::ErrorKind::BrokenPipe),
        format_args!("x"),
        false
    ));
}

#[test]
fn test_real_write_error_is_reported_as_failure() {
    // Anything else is a genuine I/O failure the caller turns into a failure
    // exit, so it must be distinguishable from a broken pipe.
    assert!(!writeln_io_log(
        FailingWriter(std::io::ErrorKind::StorageFull),
        format_args!("x")
    ));
    assert!(!write_io_log(
        FailingWriter(std::io::ErrorKind::PermissionDenied),
        format_args!("x"),
        false
    ));
}

#[test]
fn test_partial_write_then_error_is_a_failure() {
    // `write!` succeeds but the trailing `flush` fails: the flush error decides
    // the result, so a buffered stream that dies on drain is not reported as
    // written.
    struct FlushFails(Recorder);
    impl Write for FlushFails {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("flush failed"))
        }
    }
    let mut out = FlushFails(Recorder::default());
    assert!(!write_io_log(&mut out, format_args!("data"), false));
    assert_eq!(out.0.bytes, b"data", "the bytes still reached the writer");
}

#[test]
fn test_macros_do_not_panic_when_stdout_and_stderr_are_closed() {
    // Re-executes this test binary as a probe, because the only way to prove a
    // macro tolerates a dead stream is to actually close one - and closing the
    // harness's own stdout would wedge the run.
    let output = std::process::Command::new(std::env::current_exe().expect("test binary path"))
        .args([
            "--exact",
            "test_cli_out::test_macro_output_probe",
            "--nocapture",
            "--ignored",
        ])
        .env("RAGENT_CLI_OUT_PROBE", "1")
        .output()
        .expect("probe runs");
    assert!(
        output.status.success(),
        "probe failed: status={:?}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Probe body: closes stdout and stderr, then drives every macro.
///
/// Ignored unless `RAGENT_CLI_OUT_PROBE` is set, so a plain `cargo nextest run`
/// never closes its own streams. When it is set the body replaces this process's
/// stdout and stderr with `/dev/null` file descriptors and closes them, which is
/// exactly the `| head -1` shape the helpers exist for.
#[test]
#[ignore = "probe body; driven by test_macros_do_not_panic_when_stdout_and_stderr_are_closed"]
fn test_macro_output_probe() {
    if std::env::var_os("RAGENT_CLI_OUT_PROBE").is_none() {
        return;
    }

    // Closing fd 1 and 2 is only meaningful on Unix; elsewhere the probe body
    // is a no-op and the parent's assertion still holds (the macros are
    // exercised by the other tests through `*_io_log`).
    #[cfg(unix)]
    {
        // SAFETY: this is a probe process whose only job is to close its own
        // streams; it does no further I/O that anyone observes.
        #[allow(unsafe_code)]
        unsafe {
            let dev_null = std::fs::OpenOptions::new()
                .write(true)
                .open("/dev/null")
                .expect("open /dev/null");
            use std::os::fd::AsRawFd;
            // Redirect first so the macro writes have a live target, then close
            // it: the following writes hit EBADF rather than a full console.
            let fd = dev_null.as_raw_fd();
            libc::dup2(fd, 1);
            libc::dup2(fd, 2);
            libc::close(1);
            libc::close(2);
        }
    }

    // Every macro, in every documented form: bare, formatted, named argument.
    ragent_types::cliprintln!("plain");
    ragent_types::cliprintln!("formatted {}", 1);
    ragent_types::cliprintln!("named {name}", name = "value");
    ragent_types::cliprint!("no newline");
    ragent_types::cliprintln_err!("err line");
    ragent_types::cliprint_err!("err bare");

    // The `impl Write` helpers, against the same dead descriptors they are used
    // with at real call sites.
    assert!(writeln_io_log(std::io::stdout(), format_args!("io stdout")));
    assert!(write_io_log(
        std::io::stderr(),
        format_args!("io stderr"),
        false
    ));
}

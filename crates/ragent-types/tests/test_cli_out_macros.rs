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
    let marker = probe_marker();
    let _ = std::fs::remove_file(&marker); // INTENTIONAL: clears any stale marker from an earlier run
    let output = std::process::Command::new(std::env::current_exe().expect("test binary path"))
        .args([
            "--exact",
            // `tests/all.rs` registers this file as the `test_cli_out_macros`
            // module, so the name the harness knows the probe by carries that
            // prefix. A filter that matches nothing exits successfully, so the
            // "one probe ran" assertion below is what makes a stale name fail.
            "test_cli_out_macros::test_macro_output_probe",
            "--nocapture",
        ])
        // The marker path is passed in rather than recomputed in the child: the
        // helper derives it from the process id, and the child's id is not the
        // parent's, so a recomputed path could never be observed here. The child
        // closes its own streams only when it finds this variable, so the probe
        // running inside this binary's own test pass cannot touch our stdio.
        .env("RAGENT_CLI_OUT_PROBE_MARKER", &marker)
        .output()
        .expect("probe runs");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "probe failed: status={:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status
    );
    // The probe closes stdout and stderr as its first act, so the harness's own
    // `running 1 test` / `1 passed` lines land on /dev/null. The probe therefore
    // reports that it ran by creating a marker file, and a filter that selected
    // no test leaves no marker - which is exactly the silent false positive this
    // guards against.
    assert!(
        marker.exists(),
        "the probe did not run: the --exact filter selected no test, so this \
         case would pass without exercising a single macro\nstdout: {stdout}\nstderr: {stderr}"
    );
    // The marker proves the body started; this proves the filter named exactly
    // it. A prefix or substring that matched a sibling as well would start more
    // than one test, and `--exact` plus this count is what pins the name. The
    // count comes from the harness's `running N tests` line: the probe closes
    // the streams before the `N passed` summary is written, so that line is the
    // only count that survives.
    let started = counted(&stdout, "test").max(counted(&stderr, "test"));
    assert_eq!(
        started, 1,
        "the --exact filter must select exactly one test (the probe)\n\
         stdout: {stdout}\nstderr: {stderr}"
    );
    // The marker proves the body *started*; this proves it *finished*. The probe
    // rewrites the marker once every macro has run, so a body that was filtered
    // out, or that panicked on a closed stream, leaves the initial content and
    // fails here - which is what catches a filter that names a different
    // registered module, since such a filter exits successfully having run
    // nothing.
    assert_eq!(
        std::fs::read(&marker).expect("probe marker readable"),
        PROBE_FINISHED.as_bytes(),
        "the probe must drive every macro to completion before finishing"
    );
}

#[test]
fn test_a_filter_that_names_no_test_exits_zero_and_reports_no_probe() {
    // The false positive the parent above guards against, shown directly: a
    // filter naming no test is a green run ("0 passed", exit 0), so `status`
    // and `--exact` alone can never prove the probe ran.
    let output = std::process::Command::new(std::env::current_exe().expect("test binary path"))
        .args([
            "--exact",
            "test_cli_out_macros::no_such_probe",
            "--nocapture",
        ])
        .output()
        .expect("binary runs");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "a filter that matches nothing is a green run: status={:?}",
        output.status
    );
    assert_eq!(
        counted(&stdout, "test").max(counted(&stderr, "test")),
        0,
        "a filter that matches nothing runs no test\nstdout: {stdout}\nstderr: {stderr}"
    );
}

/// The marker content the probe writes once it has driven every macro.
const PROBE_FINISHED: &str = "ran: macros drove the closed streams";

/// The number preceding `label` in the harness's output, if it wrote one.
///
/// `counted(stdout, "test")` reads the `running 1 test` / `running 3 tests`
/// line (the harness emits `test` or `tests` to agree with the count); a stream
/// with no such line counts as zero.
fn counted(stream: &str, label: &str) -> u64 {
    stream
        .lines()
        .find_map(|line| {
            let words: Vec<&str> = line.split_whitespace().collect();
            let idx = words.iter().position(|w| *w == label)?;
            words.get(idx.checked_sub(1)?)?.parse().ok()
        })
        .unwrap_or(0)
}

/// Path of the marker the probe body creates once it has started.
///
/// Under `target/temp` (NFR-004: tests never write outside the workspace) and
/// keyed by pid, so two concurrent runs of this binary cannot see each other's
/// marker. The child does not call this: the parent computes it once, removes any
/// stale file, and passes the path through `RAGENT_CLI_OUT_PROBE_MARKER`, because
/// a path derived from the child's own pid could never match what the parent
/// checks.
fn probe_marker() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("temp")
        .join("cli-out-probe")
        .join(format!("ran-{}.marker", std::process::id()))
}

/// The marker path the parent handed this probe, if any.
fn inherited_probe_marker() -> Option<std::path::PathBuf> {
    std::env::var_os("RAGENT_CLI_OUT_PROBE_MARKER").map(std::path::PathBuf::from)
}

/// Probe body: closes stdout and stderr, then drives every macro.
///
/// Always runs (no `#[ignore]` and no env gate): a filter that names this test
/// wrongly then matches nothing and the harness exits `0` having run zero
/// tests, which is invisible. Running the body unconditionally turns that into
/// a panic, because the body closes this process's stdout and stderr. The parent
/// therefore always passes `RAGENT_CLI_OUT_PROBE_MARKER`; selected as a child
/// probe it closes the streams, and selected inside the parent's own run - with
/// no marker, and with the parent's own streams left alone - it is a no-op.
#[test]
fn test_macro_output_probe() {
    if !cfg!(unix) {
        return;
    }

    // Announce that the probe body ran, before the streams are closed: the
    // parent's only other evidence would be the harness's own output, which the
    // next lines send to /dev/null.
    let Some(marker) = inherited_probe_marker() else {
        // Not driven as a probe (this binary's own run of the test, or a filter
        // that named it by accident): leave the harness's streams alone.
        return;
    };
    if let Some(parent) = marker.parent() {
        let _ = std::fs::create_dir_all(parent); // INTENTIONAL: best-effort, the create below is the assertion
    }
    std::fs::write(&marker, b"started").expect("probe marker");

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

    // Every macro returned, so record completion: the marker is the parent's
    // only evidence that the body ran through, because its streams are closed by
    // now.
    std::fs::write(&marker, PROBE_FINISHED.as_bytes()).expect("probe marker");
}

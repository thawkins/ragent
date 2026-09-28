//! Crash-dump and stderr-spool integration tests.
//!
//! Exercises the two failure-capture paths directly. `crash_dump` lives in the
//! binary crate, so this test re-imports its source with `#[path]` and supplies
//! the `chrono`/`serde`/`serde_json`/`libc` it depends on — none of which are
//! re-declared here because they come from the workspace-level build graph.
//!
//! A stack overflow aborts the process by design, so it cannot be asserted
//! in-process; exercise that path with `cargo run` and inspect the marker plus
//! `coredumpctl list` afterwards.

// The crash marker left behind by an abort must be detectable, and a clean
// exit must not be. `crash_dump` lives in the binary crate, so its source is
// pulled in here; the surrounding test target already depends on `chrono`,
// `serde`, `serde_json`, and `libc` through the binary.
#[path = "../src/crash_dump.rs"]
mod crash_dump;
use crash_dump::CrashRecord;

#[test]
fn crash_marker_records_and_reports_unclean_exit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = crash_dump::write_record(dir.path(), "running").expect("write");
    assert!(path.ends_with("log/panics/last-crash.json"));

    // A pid that cannot exist on this kernel (max pid + 1) simulates the
    // process having died without unwinding.
    let mut record: CrashRecord =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("parse");
    record.pid = 4_194_304;
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&record).expect("serialise"),
    )
    .expect("rewrite");

    let found = crash_dump::previous_unclean_exit(dir.path())
        .expect("scan")
        .expect("unclean exit expected");
    assert!(found.is_unclean());
    assert!(found.core_dump_hint.contains("core"));
}

/// The truncating spool is shared with the TUI and must hold the newest lines.
mod stderr_spool {
    use ragent_types::stderr_spool::{SPOOL_MAX_LINES, Spool};

    #[test]
    fn spool_holds_only_the_newest_lines() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("stderr.log");
        let spool = Spool::new(path.clone());

        for i in 0..(SPOOL_MAX_LINES + 250) {
            spool.write(format!("line-{i}\n").as_bytes());
        }

        let content = std::fs::read_to_string(&path).expect("read");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), SPOOL_MAX_LINES);
        assert_eq!(
            lines[lines.len() - 1],
            format!("line-{}", SPOOL_MAX_LINES + 249)
        );
    }
}

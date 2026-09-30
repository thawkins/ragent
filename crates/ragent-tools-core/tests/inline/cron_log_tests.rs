//! Inline tests for `cron_log.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_log_dir_resolves_under_working_dir() {
    assert_eq!(
        log_dir(Path::new("/project")),
        PathBuf::from("/project/log")
    );
}

#[test]
fn test_cron_outcome_as_str() {
    assert_eq!(CronOutcome::Success.as_str(), "success");
    assert_eq!(CronOutcome::Error.as_str(), "error");
    assert_eq!(CronOutcome::Skipped.as_str(), "skipped");
}

#[test]
fn test_cron_outcome_serde_roundtrip() {
    for o in [
        CronOutcome::Success,
        CronOutcome::Error,
        CronOutcome::Skipped,
    ] {
        let json = serde_json::to_string(&o).unwrap();
        let back: CronOutcome = serde_json::from_str(&json).unwrap();
        assert_eq!(o, back);
    }
}

#[test]
fn test_log_cron_execution_writes_jsonl() {
    let tmp = tempfile::tempdir().unwrap();
    let wd = tmp.path();

    log_cron_execution(
        wd,
        "cron-test-1",
        "general",
        "Run tests",
        "every 30m",
        CronOutcome::Success,
        None,
        Some("session-123"),
    );

    let entries = read_cron_log(wd, None);
    assert_eq!(entries.len(), 1);
    let e = &entries[0];
    assert_eq!(e.event_id, "cron-test-1");
    assert_eq!(e.agent_type, "general");
    assert_eq!(e.prompt, "Run tests");
    assert_eq!(e.schedule, "every 30m");
    assert_eq!(e.outcome, "success");
    assert!(e.error.is_none());
    assert_eq!(e.run_id.as_deref(), Some("session-123"));
    assert_ne!(e.timestamp, "", "timestamp should not be empty");

    clear_cron_logs(wd);
}

#[test]
fn test_log_cron_execution_error_with_message() {
    let tmp = tempfile::tempdir().unwrap();
    let wd = tmp.path();

    log_cron_execution(
        wd,
        "cron-test-2",
        "unknown-agent",
        "Do thing",
        "at 2025-01-01T00:00:00Z",
        CronOutcome::Error,
        Some("Unknown agent type: unknown-agent"),
        None,
    );

    let entries = read_cron_log(wd, None);
    assert_eq!(entries.len(), 1);
    let e = &entries[0];
    assert_eq!(e.outcome, "error");
    assert_eq!(
        e.error.as_deref(),
        Some("Unknown agent type: unknown-agent")
    );
    assert!(e.run_id.is_none());

    clear_cron_logs(wd);
}

#[test]
fn test_log_cron_execution_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    let wd = tmp.path();

    log_cron_execution(
        wd,
        "cron-test-3",
        "general",
        "Run tests",
        "every 1h",
        CronOutcome::Skipped,
        None,
        None,
    );

    let entries = read_cron_log(wd, None);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, "skipped");

    clear_cron_logs(wd);
}

#[test]
fn test_read_cron_log_filters_by_event_id() {
    let tmp = tempfile::tempdir().unwrap();
    let wd = tmp.path();

    log_cron_execution(
        wd,
        "cron-a",
        "general",
        "A",
        "every 30m",
        CronOutcome::Success,
        None,
        None,
    );
    log_cron_execution(
        wd,
        "cron-b",
        "build",
        "B",
        "every 1h",
        CronOutcome::Error,
        Some("oops"),
        None,
    );
    log_cron_execution(
        wd,
        "cron-a",
        "general",
        "A2",
        "every 30m",
        CronOutcome::Success,
        None,
        None,
    );

    let all = read_cron_log(wd, None);
    assert_eq!(all.len(), 3);

    let filtered = read_cron_log(wd, Some("cron-a"));
    assert_eq!(filtered.len(), 2);
    assert!(filtered.iter().all(|e| e.event_id == "cron-a"));

    let filtered_b = read_cron_log(wd, Some("cron-b"));
    assert_eq!(filtered_b.len(), 1);
    assert_eq!(filtered_b[0].event_id, "cron-b");

    clear_cron_logs(wd);
}

#[test]
fn test_read_cron_log_multiple_entries_chronological() {
    let tmp = tempfile::tempdir().unwrap();
    let wd = tmp.path();

    for i in 0..5 {
        log_cron_execution(
            wd,
            &format!("cron-{i}"),
            "general",
            &format!("Prompt {i}"),
            "every 30m",
            CronOutcome::Success,
            None,
            Some(&format!("run-{i}")),
        );
    }

    let entries = read_cron_log(wd, None);
    assert_eq!(entries.len(), 5);
    // All entries should be present and in file order.
    for (i, e) in entries.iter().enumerate() {
        assert_eq!(e.event_id, format!("cron-{i}"));
        assert_eq!(e.run_id.as_deref(), Some(format!("run-{i}").as_str()));
    }

    clear_cron_logs(wd);
}

#[test]
fn test_read_cron_log_empty_when_no_files() {
    let tmp = tempfile::tempdir().unwrap();
    let wd = tmp.path();
    let entries = read_cron_log(wd, None);
    assert!(entries.is_empty());
}

#[test]
fn test_read_cron_log_skips_non_cron_files() {
    let tmp = tempfile::tempdir().unwrap();
    let wd = tmp.path();
    let log_dir = wd.join("log");
    std::fs::create_dir_all(&log_dir).unwrap();

    // Write an edits-*.jsonl file (should be ignored).
    std::fs::write(log_dir.join("edits-20250101-120000.jsonl"), "{}\n").unwrap();
    // Write a valid cron file.
    std::fs::write(
        log_dir.join("cron-20250101-120000.jsonl"),
        r#"{"timestamp":"2025-01-01T12:00:00Z","event_id":"cron-x","agent_type":"general","prompt":"p","schedule":"every 1m","outcome":"success","error":null,"run_id":null}"#,
    )
    .unwrap();

    let entries = read_cron_log(wd, None);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].event_id, "cron-x");

    clear_cron_logs(wd);
}

#[test]
fn test_clear_cron_logs_removes_files() {
    let tmp = tempfile::tempdir().unwrap();
    let wd = tmp.path();

    log_cron_execution(
        wd,
        "cron-1",
        "general",
        "p",
        "every 1m",
        CronOutcome::Success,
        None,
        None,
    );
    log_cron_execution(
        wd,
        "cron-2",
        "general",
        "p",
        "every 1m",
        CronOutcome::Success,
        None,
        None,
    );

    assert!(!read_cron_log(wd, None).is_empty());
    let removed = clear_cron_logs(wd);
    assert!(removed >= 1);
    assert!(read_cron_log(wd, None).is_empty());
}

//! Inline tests for `gather_log.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn gather_log_writes_considered_and_rejected_records() {
    let dir = tempfile::tempdir().unwrap();
    let log = GatherLog::new(dir.path(), "my research").unwrap();
    log.log_event(&serde_json::json!({"event": "gather_start", "topic": "t"}))
        .unwrap();
    log.log_url(
        "https://a.example",
        "q",
        "considered",
        "A",
        "mf_search",
        "openalex",
        None,
        None,
    )
    .unwrap();
    log.log_url(
        "https://b.example",
        "q",
        "rejected",
        "B",
        "mf_search",
        "openalex",
        Some("relevance too low (Low)"),
        Some(&serde_json::json!({"relevance": "Low"})),
    )
    .unwrap();
    log.flush().unwrap();

    let contents = fs::read_to_string(log.path()).unwrap();
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 3);
    let start: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(start["event"], "gather_start");
    let considered: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(considered["status"], "considered");
    assert_eq!(considered["url"], "https://a.example");
    assert!(considered["reason"].is_null());
    assert!(considered["timestamp"].is_string());
    let rejected: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
    assert_eq!(rejected["status"], "rejected");
    assert_eq!(rejected["reason"], "relevance too low (Low)");
    assert_eq!(rejected["relevance"], "Low");
    let file_name = log
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    assert!(file_name.starts_with("research-my-research-"));
    assert!(file_name.ends_with("-web.jsonl"));
}

#[test]
fn new_does_not_create_the_file_until_the_first_append() {
    let dir = tempfile::tempdir().unwrap();
    let log = GatherLog::new(dir.path(), "lazy").unwrap();
    assert!(!log.path().exists());
    log.flush().unwrap();
    assert!(!log.path().exists());

    log.log_url("u", "q", "considered", "t", "tool", "engine", None, None)
        .unwrap();
    assert!(log.path().exists());
}

#[test]
fn gather_summary_flushes_without_explicit_flush() {
    let dir = tempfile::tempdir().unwrap();
    let log = GatherLog::new(dir.path(), "flush-test").unwrap();
    log.log_url("u", "q", "considered", "t", "tool", "engine", None, None)
        .unwrap();
    log.log_event(&serde_json::json!({"event": "gather_summary", "captured": 1}))
        .unwrap();

    // No explicit flush / drop: the summary marker must have flushed both
    // the preceding record and itself.
    let contents = fs::read_to_string(log.path()).unwrap();
    assert_eq!(contents.lines().count(), 2);
}

#[test]
fn sanitize_replaces_unsafe_characters() {
    assert_eq!(sanitize("v1 rocket/german?"), "v1-rocket-german-");
    assert_eq!(sanitize("plain_name-1"), "plain_name-1");
}

//! Coverage for the hook-failure ledger (spec `teamssetup` FR-019 / NFR-004,
//! task T-026).
//!
//! Before T-026 a hook that could not run returned `HookOutcome::Allow` and
//! left no trace, so an inert hook was indistinguishable from a hook that had
//! run and approved (finding F-14). Every test here drives one of the three
//! failure kinds through the real runtime path (`run_hook` / `run_team_hook` /
//! a task mutation) and asserts on the ledger that results.

use std::path::{Path, PathBuf};

use ragent_agent::team::manager::HOOK_TIMEOUT;
use ragent_agent::team::{
    HOOK_FAILURES_FILE, HOOK_FAILURES_ROTATED_FILE, HookEvent, HookFailure, HookFailureReason,
    HookOutcome, MAX_LEDGER_BYTES, Task, TaskStore, TeamStore, hook_failure_count, ledger_path,
    read_hook_failures, record_hook_failure, render_hook_failures, run_hook, run_team_hook,
    try_record_hook_failure,
};
use tempfile::TempDir;

/// Create a real project-local team and return `(tempdir, team_dir)`.
fn team(name: &str) -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".ragent").join("teams")).expect("teams dir");
    let store = TeamStore::create(name, "lead-sess", dir.path(), true).expect("create team");
    let team_dir = store.dir.clone();
    (dir, team_dir)
}

/// Install one hook for `event` on a team's `config.json`.
fn install_hook(team_dir: &Path, event: HookEvent, command: &str) {
    let mut store = TeamStore::load(team_dir).expect("load team");
    store.config.settings.hooks = vec![ragent_agent::team::HookEntry {
        event,
        command: command.to_string(),
    }];
    store.save().expect("save team");
}

// -- TC-012: a refused command is recorded -----------------------------------

#[tokio::test]
async fn test_refused_hook_command_is_recorded_in_the_ledger() {
    let (_tmp, team_dir) = team("ledger-invalid");

    let outcome = run_hook(
        &team_dir,
        HookEvent::TaskCompleted,
        "echo > out.txt",
        &[],
        None,
    )
    .await;

    // The verdict is unchanged (a gate that cannot run does not block work),
    // but it is no longer invisible.
    assert_eq!(outcome, HookOutcome::Allow);

    let failures = read_hook_failures(&team_dir);
    assert_eq!(failures.len(), 1, "one refused hook writes one record");
    let record = &failures[0];
    assert_eq!(record.hook, "echo > out.txt");
    assert_eq!(record.event, HookEvent::TaskCompleted);
    assert_eq!(record.reason, HookFailureReason::Invalid);
    assert_eq!(record.detail, "hook command contains a shell metacharacter");
}

#[tokio::test]
async fn test_ledger_line_is_one_json_object_per_record() {
    let (_tmp, team_dir) = team("ledger-json");

    run_hook(&team_dir, HookEvent::TaskCreated, "echo > a", &[], None).await;
    run_hook(&team_dir, HookEvent::TaskClaimed, "echo > b", &[], None).await;

    let text =
        std::fs::read_to_string(ledger_path(&team_dir)).expect("read the ledger file itself");
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), 2, "one line per failure: {text}");

    for line in lines {
        // Each line must stand alone: a reader that streams the file and
        // parses line-by-line must not need the whole array.
        let value: serde_json::Value = serde_json::from_str(line).expect("one JSON object");
        assert!(value.get("timestamp").is_some(), "timestamp: {line}");
        assert!(value.get("hook").is_some(), "hook: {line}");
        assert!(value.get("reason").is_some(), "reason: {line}");
        assert!(value.get("event").is_some(), "event: {line}");
    }
    assert_eq!(hook_failure_count(&team_dir), 2);
}

// -- the other two failure kinds --------------------------------------------

#[tokio::test]
async fn test_spawn_failure_is_recorded_in_the_ledger() {
    let (_tmp, team_dir) = team("ledger-spawn");

    let outcome = run_hook(
        &team_dir,
        HookEvent::TaskCompleted,
        "this-is-not-a-binary-anywhere-ragent",
        &[],
        None,
    )
    .await;
    assert_eq!(outcome, HookOutcome::Allow);

    let failures = read_hook_failures(&team_dir);
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].reason, HookFailureReason::SpawnFailed);
    assert_eq!(failures[0].event, HookEvent::TaskCompleted);
    assert!(
        !failures[0].detail.is_empty(),
        "the spawn error must be named: {:?}",
        failures[0].detail
    );
}

#[tokio::test]
async fn test_timeout_is_recorded_in_the_ledger() {
    let (_tmp, team_dir) = team("ledger-timeout");

    // `sleep 300` is a bare, metacharacter-free command, so it reaches the
    // spawn and only the hard timeout stops it.
    let started = std::time::Instant::now();
    let outcome = run_hook(
        &team_dir,
        HookEvent::TaskClaimed,
        "sleep",
        &["300".to_string()],
        None,
    )
    .await;
    assert_eq!(outcome, HookOutcome::Allow);
    assert!(
        started.elapsed() < HOOK_TIMEOUT + std::time::Duration::from_secs(10),
        "the hook must respect its timeout"
    );

    let failures = read_hook_failures(&team_dir);
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].reason, HookFailureReason::Timeout);
    assert_eq!(failures[0].event, HookEvent::TaskClaimed);
    assert!(
        failures[0].detail.contains("timeout"),
        "the reason must name the timeout: {:?}",
        failures[0].detail
    );
}

#[tokio::test]
async fn test_a_hook_that_runs_is_not_recorded() {
    let (_tmp, team_dir) = team("ledger-healthy");

    let outcome = run_hook(&team_dir, HookEvent::TaskCompleted, "true", &[], None).await;
    assert_eq!(outcome, HookOutcome::Allow);

    assert!(
        read_hook_failures(&team_dir).is_empty(),
        "a hook that ran wrote nothing: {:?}",
        read_hook_failures(&team_dir)
    );
    assert!(
        !ledger_path(&team_dir).exists(),
        "no ledger file is created"
    );
}

// -- the runtime path: a task mutation writes the record ---------------------

#[tokio::test]
async fn test_task_completion_records_an_inert_hook() {
    let (_tmp, team_dir) = team("ledger-runtime");
    install_hook(&team_dir, HookEvent::TaskCompleted, "echo > out.txt");

    let store = TaskStore::open(&team_dir).expect("open task store");
    store
        .add_task(Task::new("task-001", "ledger probe"))
        .await
        .expect("add task");
    store
        .claim_specific("task-001", "lead")
        .await
        .expect("claim task");
    let (task, outcome) = store
        .complete("task-001", "lead")
        .await
        .expect("complete task");
    assert_eq!(task.status, ragent_agent::team::TaskStatus::Completed);
    assert_eq!(outcome, HookOutcome::Allow);

    let failures = read_hook_failures(&team_dir);
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].event, HookEvent::TaskCompleted);
    assert_eq!(failures[0].hook, "echo > out.txt");
}

#[tokio::test]
async fn test_run_team_hook_records_a_refused_command() {
    let (_tmp, team_dir) = team("ledger-team-hook");
    install_hook(&team_dir, HookEvent::TaskCreated, "true; rm");

    let outcome = run_team_hook(&team_dir, HookEvent::TaskCreated, None).await;
    assert_eq!(outcome, HookOutcome::Allow);

    let failures = read_hook_failures(&team_dir);
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].event, HookEvent::TaskCreated);
    assert_eq!(failures[0].hook, "true; rm");
}

// -- NFR-004: append-only and bounded ---------------------------------------

#[test]
fn test_ledger_rotates_at_one_mebibyte_and_keeps_one_generation() {
    let (_tmp, team_dir) = team("ledger-rotate");

    // Fill the live file past the cap without needing 1 MiB of records: a
    // single oversized record is impossible (records are small), so write the
    // filler straight into the file the ledger reads.
    let filler = format!("{}\n", "x".repeat(MAX_LEDGER_BYTES as usize));
    std::fs::write(ledger_path(&team_dir), &filler).expect("write filler");
    assert!(std::fs::metadata(ledger_path(&team_dir)).unwrap().len() >= MAX_LEDGER_BYTES);

    let record = HookFailure::new(
        HookEvent::TaskCompleted,
        "gate.sh",
        HookFailureReason::Timeout,
        "hook exceeded the 30s timeout",
    );
    try_record_hook_failure(&team_dir, &record).expect("append after rotation");

    // The old content moved aside; the live file holds exactly the new record.
    let rotated =
        std::fs::read_to_string(team_dir.join(HOOK_FAILURES_ROTATED_FILE)).expect("rotated file");
    assert_eq!(
        rotated.len(),
        filler.len(),
        "the previous generation is kept whole"
    );
    let live = read_hook_failures(&team_dir);
    assert_eq!(live.len(), 1, "the live file restarted with one record");
    assert_eq!(live[0].hook, "gate.sh");
    assert!(
        std::fs::metadata(ledger_path(&team_dir)).unwrap().len() < MAX_LEDGER_BYTES,
        "the live file is back under the cap"
    );
}

#[test]
fn test_second_rotation_replaces_the_previous_generation() {
    let (_tmp, team_dir) = team("ledger-rotate-twice");

    let record = |hook: &str| {
        HookFailure::new(
            HookEvent::TaskCreated,
            hook,
            HookFailureReason::Invalid,
            "hook command is empty",
        )
    };

    let filler = format!("{}\n", "y".repeat(MAX_LEDGER_BYTES as usize));
    std::fs::write(ledger_path(&team_dir), &filler).expect("write filler");
    try_record_hook_failure(&team_dir, &record("first.sh")).expect("first rotation");

    std::fs::write(ledger_path(&team_dir), &filler).expect("refill");
    try_record_hook_failure(&team_dir, &record("second.sh")).expect("second rotation");

    let rotated =
        std::fs::read_to_string(team_dir.join(HOOK_FAILURES_ROTATED_FILE)).expect("rotated file");
    assert_eq!(rotated.len(), filler.len());
    let live = read_hook_failures(&team_dir);
    assert_eq!(live.len(), 1);
    assert_eq!(
        live[0].hook, "second.sh",
        "only one previous generation is kept"
    );
}

#[test]
fn test_ledger_appends_without_rewriting_existing_records() {
    let (_tmp, team_dir) = team("ledger-append");

    let first = HookFailure::new(
        HookEvent::TaskCompleted,
        "one.sh",
        HookFailureReason::Invalid,
        "hook command is empty",
    );
    record_hook_failure(&team_dir, &first);
    let after_first = std::fs::read(ledger_path(&team_dir)).expect("read");

    let second = HookFailure::new(
        HookEvent::TaskCompleted,
        "two.sh",
        HookFailureReason::SpawnFailed,
        "No such file or directory",
    );
    record_hook_failure(&team_dir, &second);
    let after_second = std::fs::read(ledger_path(&team_dir)).expect("read");

    assert!(
        after_second.starts_with(&after_first),
        "an append must leave the existing bytes untouched"
    );
    assert_eq!(read_hook_failures(&team_dir).len(), 2);
}

// -- reporting surfaces ------------------------------------------------------

#[test]
fn test_render_is_empty_when_nothing_failed() {
    assert_eq!(render_hook_failures("t", &[], 2), "");
}

#[test]
fn test_render_names_the_count_each_failure_and_the_ledger() {
    let failures = vec![
        HookFailure::new(
            HookEvent::TaskCompleted,
            "echo > out.txt",
            HookFailureReason::Invalid,
            "hook command contains a shell metacharacter",
        ),
        HookFailure::new(
            HookEvent::TaskCreated,
            "gate.sh",
            HookFailureReason::Timeout,
            "hook exceeded the 30s timeout",
        ),
    ];
    let rendered = render_hook_failures("thook", &failures, 2);

    assert!(rendered.starts_with("## Hook failures (2)"), "{rendered}");
    assert!(rendered.contains("echo > out.txt"), "{rendered}");
    assert!(
        rendered.contains("hook command contains a shell metacharacter"),
        "{rendered}"
    );
    assert!(rendered.contains("task_created"), "{rendered}");
    assert!(rendered.contains(HOOK_FAILURES_FILE), "{rendered}");
    assert!(rendered.contains(HOOK_FAILURES_ROTATED_FILE), "{rendered}");
    assert!(rendered.contains("team 'thook'"), "{rendered}");
}

#[test]
fn test_record_carries_a_timestamp() {
    let before = chrono::Utc::now();
    let failure = HookFailure::new(
        HookEvent::TaskCompleted,
        "gate.sh",
        HookFailureReason::Invalid,
        "hook command is empty",
    );
    let after = chrono::Utc::now();
    assert!(failure.timestamp >= before && failure.timestamp <= after);
}

// -- a malformed ledger never hides a readable one --------------------------

#[test]
fn test_truncated_final_line_does_not_hide_earlier_records() {
    let (_tmp, team_dir) = team("ledger-truncated");

    record_hook_failure(
        &team_dir,
        &HookFailure::new(
            HookEvent::TaskCompleted,
            "good.sh",
            HookFailureReason::Invalid,
            "hook command is empty",
        ),
    );
    // Simulate a crash mid-append: a half-written trailing line.
    let mut text = std::fs::read_to_string(ledger_path(&team_dir)).expect("read");
    text.push_str("{\"timestamp\":\"2026-08-01T0");
    std::fs::write(ledger_path(&team_dir), text).expect("write");

    let failures = read_hook_failures(&team_dir);
    assert_eq!(failures.len(), 1, "the readable record survives");
    assert_eq!(failures[0].hook, "good.sh");
}

#[test]
fn test_missing_ledger_reads_as_empty() {
    let (_tmp, team_dir) = team("ledger-missing");
    let failures = read_hook_failures(&team_dir);
    assert!(failures.is_empty(), "a missing ledger reads as empty");
    assert_eq!(hook_failure_count(&team_dir), 0);
    assert_eq!(render_hook_failures("ledger-missing", &failures, 1), "");
}

// -- the reason tokens are the documented wire form -------------------------

#[test]
fn test_reason_tokens_are_stable() {
    assert_eq!(HookFailureReason::Invalid.as_str(), "invalid");
    assert_eq!(HookFailureReason::SpawnFailed.as_str(), "spawn_failed");
    assert_eq!(HookFailureReason::Timeout.as_str(), "timeout");
}

// -- TC-012 step 4/5: both surfaces show a non-zero count -------------------

#[tokio::test]
async fn test_team_status_reports_the_hook_failure_count() {
    use ragent_agent::tool::{Tool, ToolContext};

    let (_tmp, team_dir) = team("ledger-surface");
    install_hook(&team_dir, HookEvent::TaskCompleted, "echo > out.txt");

    // Drive one completion so the hook fires and cannot run.
    let store = TaskStore::open(&team_dir).expect("open task store");
    store
        .add_task(Task::new("task-001", "ledger probe"))
        .await
        .expect("add task");
    store
        .claim_specific("task-001", "lead")
        .await
        .expect("claim task");
    store.complete("task-001", "lead").await.expect("complete");

    let workspace = team_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf();
    let ctx = ToolContext {
        session_id: "ledger-surface".to_string(),
        working_dir: workspace,
        event_bus: std::sync::Arc::new(ragent_agent::event::EventBus::new(16)),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        allowed_roots: Vec::new(),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        cached_team_dir: std::sync::Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        canonical_cache: std::sync::Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        tool_registry: ToolContext::default_tool_registry(),
    };

    let tool = ragent_agent::tool::team_status::TeamStatusTool;
    let out = tool
        .execute(serde_json::json!({"team_name": "ledger-surface"}), &ctx)
        .await
        .expect("team_status");

    assert!(
        out.content.contains("Hook failures (1)"),
        "team_status must surface the count: {}",
        out.content
    );
    assert!(
        out.content.contains("echo > out.txt"),
        "team_status must name the inert hook: {}",
        out.content
    );
    let metadata = out.metadata.expect("metadata");
    assert_eq!(metadata["hook_failure_count"], serde_json::json!(1));
    assert_eq!(
        metadata["hook_failures"][0]["reason"],
        serde_json::json!("invalid")
    );
    assert_eq!(
        metadata["hook_failures"][0]["event"],
        serde_json::json!("task_completed")
    );
}

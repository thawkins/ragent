//! Inline tests for `cron.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use ragent_storage::Storage;
use ragent_types::{CronEvent, CronSchedule};

/// Verify that `cron_tick` does not panic when there are no events.
#[tokio::test]
async fn test_cron_tick_no_events() {
    let storage = Storage::open_in_memory().expect("storage");
    let working_dir = unique_temp_dir();
    let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
    // SessionProcessor is not available in unit tests, but cron_tick
    // should handle the no-events case without needing it.
    // We test the no-events path by calling cron_tick with a mock
    // session processor reference. Since there are no due events,
    // the processor is never accessed.
    let processor = create_test_processor();
    cron_tick(&storage, &processor, &working_dir, &running_events).await;
}

/// Verify that `cron_tick` picks up a due event and processes it
/// (spawning fails gracefully because no AgentManager is set).
#[tokio::test]
async fn test_cron_tick_with_due_event() {
    let storage = Storage::open_in_memory().expect("storage");
    let working_dir = unique_temp_dir();
    let past = chrono::Utc::now() - chrono::Duration::hours(1);
    let event = CronEvent::new(
        "test-due".to_string(),
        "general".to_string(),
        "test prompt".to_string(),
        CronSchedule::one_shot(past),
        "at past".to_string(),
        past,
    );
    storage.insert_cron_event(&event).expect("insert");
    let processor = create_test_processor();
    let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
    cron_tick(&storage, &processor, &working_dir, &running_events).await;
    // The one-shot event should be disabled after firing (FR-005),
    // even though the spawn failed (no AgentManager set).
    let row = storage
        .get_cron_event("test-due")
        .expect("get")
        .expect("found");
    assert!(
        !row.enabled,
        "one-shot event should be disabled after firing"
    );
}

/// Verify that a repeating event's next_due is advanced after firing.
#[tokio::test]
async fn test_cron_tick_repeating_event_advances_next_due() {
    let storage = Storage::open_in_memory().expect("storage");
    let working_dir = unique_temp_dir();
    let now = chrono::Utc::now();
    let past = now - chrono::Duration::hours(2);
    let event = CronEvent::new(
        "test-repeat".to_string(),
        "general".to_string(),
        "test prompt".to_string(),
        CronSchedule::repeat_from(past, 3600), // 1h interval
        "from past every 1h".to_string(),
        past,
    );
    storage.insert_cron_event(&event).expect("insert");
    let processor = create_test_processor();
    let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
    cron_tick(&storage, &processor, &working_dir, &running_events).await;
    // The repeating event should still be enabled with an advanced next_due.
    let row = storage
        .get_cron_event("test-repeat")
        .expect("get")
        .expect("found");
    assert!(row.enabled, "repeating event should still be enabled");
    let new_next_due = chrono::DateTime::parse_from_rfc3339(&row.next_due).expect("parse next_due");
    assert!(
        new_next_due.with_timezone(&Utc) > now,
        "next_due should be advanced to the future"
    );
}

/// Verify that a disabled due event is skipped and logged as "skipped"
/// (FR-007, FR-011). The event should not be fired (no agent spawn
/// attempted) and its next_due should be advanced.
#[tokio::test]
async fn test_cron_tick_disabled_event_skipped() {
    let storage = Storage::open_in_memory().expect("storage");
    let working_dir = unique_temp_dir();
    let now = chrono::Utc::now();
    let past = now - chrono::Duration::hours(1);
    let event = CronEvent::new(
        "test-disabled".to_string(),
        "general".to_string(),
        "test prompt".to_string(),
        CronSchedule::repeat_from(past, 3600), // 1h interval
        "from past every 1h".to_string(),
        past,
    );
    storage.insert_cron_event(&event).expect("insert");
    // Disable the event.
    storage
        .set_cron_event_enabled("test-disabled", false)
        .expect("disable");

    let processor = create_test_processor();
    let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
    cron_tick(&storage, &processor, &working_dir, &running_events).await;

    // The disabled event should NOT be fired - it remains disabled.
    let row = storage
        .get_cron_event("test-disabled")
        .expect("get")
        .expect("found");
    assert!(
        !row.enabled,
        "disabled event should still be disabled after tick"
    );
    // next_due should be advanced to the future (skip logged + advanced).
    let new_next_due = chrono::DateTime::parse_from_rfc3339(&row.next_due).expect("parse next_due");
    assert!(
        new_next_due.with_timezone(&Utc) > now,
        "disabled repeating event next_due should be advanced after skip"
    );

    // Verify a "skipped" log entry was written.
    let logs = ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("test-disabled"));
    assert!(
        logs.iter().any(|e| e.outcome == "skipped"),
        "expected a 'skipped' log entry for disabled event"
    );
}

/// Verify that an event with an unknown agent type is logged as "error"
/// and not spawned (FR-016). The event should still be advanced/disabled
/// so it doesn't retry on every tick.
#[tokio::test]
async fn test_cron_tick_unknown_agent_type_error() {
    let storage = Storage::open_in_memory().expect("storage");
    let working_dir = unique_temp_dir();
    let past = chrono::Utc::now() - chrono::Duration::hours(1);
    let event = CronEvent::new(
        "test-unknown-agent".to_string(),
        "nonexistent-agent-xyz".to_string(),
        "test prompt".to_string(),
        CronSchedule::one_shot(past),
        "at past".to_string(),
        past,
    );
    storage.insert_cron_event(&event).expect("insert");
    let processor = create_test_processor();
    let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
    cron_tick(&storage, &processor, &working_dir, &running_events).await;

    // The one-shot event should be disabled after the error (not retried).
    let row = storage
        .get_cron_event("test-unknown-agent")
        .expect("get")
        .expect("found");
    assert!(
        !row.enabled,
        "one-shot event with unknown agent should be disabled after error"
    );

    // Verify an "error" log entry was written with the unknown agent message.
    let logs = ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("test-unknown-agent"));
    let error_entry = logs
        .iter()
        .find(|e| e.outcome == "error")
        .expect("expected an 'error' log entry");
    assert!(
        error_entry
            .error
            .as_ref()
            .is_some_and(|msg| msg.contains("Unknown agent type")),
        "error message should mention 'Unknown agent type', got: {:?}",
        error_entry.error
    );
}

/// Verify that a repeating event whose previous execution is still
/// running is skipped and logged as `"skipped"` (FR-012).
///
/// We simulate a running event by manually inserting its ID into the
/// `running_events` set before calling `cron_tick`. The scheduler should
/// skip the cycle, log `"skipped"`, and advance `next_due` - without
/// attempting to spawn an agent run.
#[tokio::test]
async fn test_cron_tick_repeating_event_double_fire_guard() {
    let storage = Storage::open_in_memory().expect("storage");
    let working_dir = unique_temp_dir();
    let now = chrono::Utc::now();
    let past = now - chrono::Duration::hours(2);
    let event = CronEvent::new(
        "test-double-fire".to_string(),
        "general".to_string(),
        "test prompt".to_string(),
        CronSchedule::repeat_from(past, 3600), // 1h interval
        "from past every 1h".to_string(),
        past,
    );
    storage.insert_cron_event(&event).expect("insert");

    // Simulate a still-running previous execution by pre-inserting the
    // event ID into the running_events set.
    let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
    {
        let mut set = running_events.lock().expect("lock");
        set.insert("test-double-fire".to_string());
    }

    let processor = create_test_processor();
    cron_tick(&storage, &processor, &working_dir, &running_events).await;

    // The repeating event should still be enabled (it was skipped, not fired).
    let row = storage
        .get_cron_event("test-double-fire")
        .expect("get")
        .expect("found");
    assert!(
        row.enabled,
        "skipped repeating event should still be enabled"
    );

    // next_due should be advanced to the future (skip logged + advanced).
    let new_next_due = chrono::DateTime::parse_from_rfc3339(&row.next_due).expect("parse next_due");
    assert!(
        new_next_due.with_timezone(&Utc) > now,
        "skipped repeating event next_due should be advanced to the future"
    );

    // Verify a "skipped" log entry was written with the double-fire reason.
    let logs = ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("test-double-fire"));
    let skipped_entry = logs
        .iter()
        .find(|e| e.outcome == "skipped")
        .expect("expected a 'skipped' log entry for double-fire guard");
    assert!(
        skipped_entry
            .error
            .as_ref()
            .is_some_and(|msg| msg.contains("Previous execution still running")),
        "skipped log entry should mention 'Previous execution still running', got: {:?}",
        skipped_entry.error
    );

    // The event ID should still be in the running_events set (the guard
    // does not remove it - only the completion monitor does).
    {
        let set = running_events.lock().expect("lock");
        assert!(
            set.contains("test-double-fire"),
            "event ID should remain in running_events set after double-fire skip"
        );
    }
}

/// Integration test: a repeating event fires, logs, and advances
/// `next_due` by exactly one duration interval (FR-004, FR-006).
///
/// Because the test processor has no `AgentManager`, the spawn fails and
/// the outcome is `"error"`. We still verify the full pipeline:
/// the log entry is written with the correct fields, the event remains
/// enabled, and `next_due` advances by exactly one interval.
#[tokio::test]
async fn test_integration_repeating_event_fires_logs_advances() {
    let storage = Storage::open_in_memory().expect("storage");
    let working_dir = unique_temp_dir();
    let now = chrono::Utc::now();
    let past = now - chrono::Duration::hours(2);
    let duration_secs: i64 = 3600; // 1h interval
    let event = CronEvent::new(
        "integration-repeat".to_string(),
        "general".to_string(),
        "run integration test".to_string(),
        CronSchedule::repeat_from(past, duration_secs),
        "from past every 1h".to_string(),
        past,
    );
    storage.insert_cron_event(&event).expect("insert");

    let processor = create_test_processor();
    let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
    cron_tick(&storage, &processor, &working_dir, &running_events).await;

    // FR-004: repeating event should still be enabled after firing.
    let row = storage
        .get_cron_event("integration-repeat")
        .expect("get")
        .expect("found");
    assert!(
        row.enabled,
        "repeating event should still be enabled after firing"
    );

    // FR-004: next_due should be advanced by exactly one duration interval
    // from the original next_due (which was `past`).
    let original_next_due = event.next_due;
    let new_next_due =
        chrono::DateTime::parse_from_rfc3339(&row.next_due).expect("parse new next_due");
    let expected_advance = original_next_due + chrono::Duration::seconds(duration_secs);
    // The advance may skip ahead multiple intervals if behind, so the new
    // next_due must be >= original + one interval and in the future.
    assert!(
        new_next_due.with_timezone(&Utc) >= expected_advance,
        "next_due should be advanced by at least one interval: \
         original={original_next_due}, new={new_next_due}, expected>={expected_advance}"
    );
    assert!(
        new_next_due.with_timezone(&Utc) > now,
        "next_due should be in the future after advancement"
    );

    // FR-006: a log entry should have been written with the correct fields.
    let logs = ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("integration-repeat"));
    assert!(
        !logs.is_empty(),
        "expected at least one log entry for the fired repeating event"
    );
    let entry = &logs[0];
    assert_eq!(entry.event_id, "integration-repeat");
    assert_eq!(entry.agent_type, "general");
    assert_eq!(entry.prompt, "run integration test");
    assert_eq!(entry.schedule, "from past every 1h");
    // Outcome is "error" because spawn fails without a AgentManager.
    assert_eq!(
        entry.outcome, "error",
        "outcome should be 'error' (spawn fails without AgentManager)"
    );
    assert!(
        entry.error.is_some(),
        "error field should be populated when outcome is 'error'"
    );
    // Timestamp should be a valid RFC 3339 string.
    chrono::DateTime::parse_from_rfc3339(&entry.timestamp)
        .expect("timestamp should be valid RFC 3339");
}

/// Integration test: a one-shot event fires, logs, and is disabled
/// (FR-005, FR-006).
///
/// Because the test processor has no `AgentManager`, the spawn fails and
/// the outcome is `"error"`. We still verify the full pipeline:
/// the log entry is written with the correct fields, and the event is
/// disabled so it does not fire again.
#[tokio::test]
async fn test_integration_one_shot_event_fires_logs_disabled() {
    let storage = Storage::open_in_memory().expect("storage");
    let working_dir = unique_temp_dir();
    let past = chrono::Utc::now() - chrono::Duration::hours(1);
    let event = CronEvent::new(
        "integration-oneshot".to_string(),
        "coder".to_string(),
        "do a one-shot thing".to_string(),
        CronSchedule::one_shot(past),
        "at past".to_string(),
        past,
    );
    storage.insert_cron_event(&event).expect("insert");

    let processor = create_test_processor();
    let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
    cron_tick(&storage, &processor, &working_dir, &running_events).await;

    // FR-005: one-shot event should be disabled after firing.
    let row = storage
        .get_cron_event("integration-oneshot")
        .expect("get")
        .expect("found");
    assert!(
        !row.enabled,
        "one-shot event should be disabled after firing"
    );

    // FR-006: a log entry should have been written with the correct fields.
    let logs =
        ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("integration-oneshot"));
    assert!(
        !logs.is_empty(),
        "expected at least one log entry for the fired one-shot event"
    );
    let entry = &logs[0];
    assert_eq!(entry.event_id, "integration-oneshot");
    assert_eq!(entry.agent_type, "coder");
    assert_eq!(entry.prompt, "do a one-shot thing");
    assert_eq!(entry.schedule, "at past");
    assert_eq!(
        entry.outcome, "error",
        "outcome should be 'error' (spawn fails without AgentManager)"
    );
    assert!(
        entry.error.is_some(),
        "error field should be populated when outcome is 'error'"
    );
    // Timestamp should be a valid RFC 3339 string.
    chrono::DateTime::parse_from_rfc3339(&entry.timestamp)
        .expect("timestamp should be valid RFC 3339");
}

/// Integration test: the JSONL log entry contains all fields required by
/// FR-006 (event id, agent type, prompt, outcome, error, timestamp).
///
/// Fires both a repeating and a one-shot event in separate working dirs,
/// then reads back the logs and verifies every FR-006 field is present
/// and correctly typed.
#[tokio::test]
async fn test_integration_log_entry_contains_all_fr006_fields() {
    // --- Repeating event ---
    let storage = Storage::open_in_memory().expect("storage");
    let working_dir = unique_temp_dir();
    let past = chrono::Utc::now() - chrono::Duration::hours(1);
    let event = CronEvent::new(
        "fr006-repeat".to_string(),
        "general".to_string(),
        "fr006 repeating prompt".to_string(),
        CronSchedule::repeat_from(past, 3600),
        "from past every 1h".to_string(),
        past,
    );
    storage.insert_cron_event(&event).expect("insert");
    let processor = create_test_processor();
    let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
    cron_tick(&storage, &processor, &working_dir, &running_events).await;

    let logs = ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("fr006-repeat"));
    assert_eq!(
        logs.len(),
        1,
        "expected exactly one log entry for the repeating event"
    );
    let entry = &logs[0];
    // FR-006: event id
    assert_eq!(entry.event_id, "fr006-repeat");
    // FR-006: agent type
    assert_eq!(entry.agent_type, "general");
    // FR-006: prompt
    assert_eq!(entry.prompt, "fr006 repeating prompt");
    // FR-006: outcome
    assert!(
        matches!(entry.outcome.as_str(), "success" | "error" | "skipped"),
        "outcome should be one of success/error/skipped, got: {}",
        entry.outcome
    );
    // FR-006: error message (present when outcome is "error")
    if entry.outcome == "error" {
        assert!(
            entry.error.is_some(),
            "error field must be populated when outcome is 'error'"
        );
    }
    // FR-006: timestamp (valid RFC 3339)
    let ts =
        chrono::DateTime::parse_from_rfc3339(&entry.timestamp).expect("valid RFC 3339 timestamp");
    // Timestamp should be recent (within the last few seconds).
    let now = chrono::Utc::now();
    let age = now.signed_duration_since(ts.with_timezone(&Utc));
    assert!(
        age.num_seconds() < 10,
        "log timestamp should be recent, got age={age}"
    );
    // FR-006: schedule field
    assert_eq!(entry.schedule, "from past every 1h");

    // --- One-shot event ---
    let storage2 = Storage::open_in_memory().expect("storage");
    let working_dir2 = unique_temp_dir();
    let event2 = CronEvent::new(
        "fr006-oneshot".to_string(),
        "ask".to_string(),
        "fr006 one-shot prompt".to_string(),
        CronSchedule::one_shot(past),
        "at past".to_string(),
        past,
    );
    storage2.insert_cron_event(&event2).expect("insert");
    let processor2 = create_test_processor();
    let running_events2: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
    cron_tick(&storage2, &processor2, &working_dir2, &running_events2).await;

    let logs2 = ragent_tools_core::cron_log::read_cron_log(&working_dir2, Some("fr006-oneshot"));
    // The one-shot event may produce two log entries in the same tick:
    // 1. "error" from fire_cron_event (spawn fails -> disabled)
    // 2. "skipped" from skip_disabled_event (disabled event found again
    //    in the same tick's disabled-due query before next_due is pushed
    //    to the far future). We verify the "error" entry has all fields.
    let error_entry2 = logs2
        .iter()
        .find(|e| e.outcome == "error")
        .expect("expected an 'error' log entry for the one-shot event");
    assert_eq!(error_entry2.event_id, "fr006-oneshot");
    assert_eq!(error_entry2.agent_type, "ask");
    assert_eq!(error_entry2.prompt, "fr006 one-shot prompt");
    assert_eq!(error_entry2.schedule, "at past");
    assert!(
        error_entry2.error.is_some(),
        "error field must be populated when outcome is 'error'"
    );
    let ts2 = chrono::DateTime::parse_from_rfc3339(&error_entry2.timestamp)
        .expect("valid RFC 3339 timestamp");
    let age2 = now.signed_duration_since(ts2.with_timezone(&Utc));
    assert!(
        age2.num_seconds() < 10,
        "log timestamp should be recent, got age={age2}"
    );
}

/// Integration test: a disabled one-shot event is skipped (not fired),
/// logged with all FR-006 fields, and not re-logged on a second tick
/// (FR-007, FR-011).
///
/// The existing unit test covers disabled *repeating* events. This test
/// covers the disabled *one-shot* path: `skip_disabled_event` pushes
/// `next_due` to a far-future timestamp (9999-12-31) so that a second
/// tick does not re-encounter the event.
#[tokio::test]
async fn test_integration_disabled_one_shot_skipped_not_refired() {
    let storage = Storage::open_in_memory().expect("storage");
    let working_dir = unique_temp_dir();
    let past = chrono::Utc::now() - chrono::Duration::hours(1);
    let event = CronEvent::new(
        "int-disabled-oneshot".to_string(),
        "general".to_string(),
        "disabled one-shot prompt".to_string(),
        CronSchedule::one_shot(past),
        "at past".to_string(),
        past,
    );
    storage.insert_cron_event(&event).expect("insert");
    // Disable the event before the tick.
    storage
        .set_cron_event_enabled("int-disabled-oneshot", false)
        .expect("disable");

    let processor = create_test_processor();
    let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));

    // First tick: should skip the disabled event.
    cron_tick(&storage, &processor, &working_dir, &running_events).await;

    // FR-011: event remains disabled (never fired).
    let row = storage
        .get_cron_event("int-disabled-oneshot")
        .expect("get")
        .expect("found");
    assert!(
        !row.enabled,
        "disabled one-shot should still be disabled after tick"
    );

    // FR-007: a "skipped" log entry should have been written.
    let logs =
        ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("int-disabled-oneshot"));
    let skipped = logs
        .iter()
        .find(|e| e.outcome == "skipped")
        .expect("expected a 'skipped' log entry for disabled one-shot");

    // FR-006: verify all log fields.
    assert_eq!(skipped.event_id, "int-disabled-oneshot");
    assert_eq!(skipped.agent_type, "general");
    assert_eq!(skipped.prompt, "disabled one-shot prompt");
    assert_eq!(skipped.schedule, "at past");
    assert!(
        skipped
            .error
            .as_ref()
            .is_some_and(|m| m.contains("disabled")),
        "skipped log should mention 'disabled', got: {:?}",
        skipped.error
    );
    chrono::DateTime::parse_from_rfc3339(&skipped.timestamp).expect("valid RFC 3339 timestamp");

    // The one-shot's next_due should be pushed to far future (9999-12-31).
    let new_next_due = chrono::DateTime::parse_from_rfc3339(&row.next_due).expect("parse next_due");
    assert!(
        new_next_due.with_timezone(&Utc) > chrono::Utc::now() + chrono::Duration::days(365),
        "disabled one-shot next_due should be pushed to far future, got: {new_next_due}"
    );

    // Second tick: the event should NOT be re-logged (far-future next_due
    // means it's no longer "due").
    cron_tick(&storage, &processor, &working_dir, &running_events).await;
    let logs2 =
        ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("int-disabled-oneshot"));
    let skipped_count = logs2.iter().filter(|e| e.outcome == "skipped").count();
    assert_eq!(
        skipped_count, 1,
        "disabled one-shot should not be re-skipped on second tick, got {skipped_count} skip entries"
    );
}

/// Integration test: a disabled event with an unknown agent type is
/// skipped (FR-007/FR-011 take priority over FR-016), not errored.
///
/// The scheduler processes disabled events in a separate query
/// (`list_disabled_due_cron_events`) that never reaches the agent-type
/// validation. So the log should show `"skipped"`, not `"error"`, even
/// though the agent type is invalid.
#[tokio::test]
async fn test_integration_disabled_unknown_agent_skipped_not_errored() {
    let storage = Storage::open_in_memory().expect("storage");
    let working_dir = unique_temp_dir();
    let past = chrono::Utc::now() - chrono::Duration::hours(1);
    let event = CronEvent::new(
        "int-disabled-unknown".to_string(),
        "nonexistent-agent-xyz".to_string(),
        "disabled unknown agent prompt".to_string(),
        CronSchedule::repeat_from(past, 3600),
        "from past every 1h".to_string(),
        past,
    );
    storage.insert_cron_event(&event).expect("insert");
    storage
        .set_cron_event_enabled("int-disabled-unknown", false)
        .expect("disable");

    let processor = create_test_processor();
    let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
    cron_tick(&storage, &processor, &working_dir, &running_events).await;

    let logs =
        ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("int-disabled-unknown"));
    // Should be "skipped", NOT "error" - disabled check takes priority.
    assert!(
        logs.iter().any(|e| e.outcome == "skipped"),
        "disabled event with unknown agent should be 'skipped', not 'error'"
    );
    assert!(
        !logs.iter().any(|e| e.outcome == "error"),
        "disabled event should not produce an 'error' entry (FR-011 takes priority over FR-016)"
    );
}

/// Integration test: an unknown agent type on a *repeating* event logs
/// `"error"` and advances `next_due` (FR-016). Unlike the one-shot case
/// (which is disabled after error), a repeating event should remain
/// enabled with `next_due` moved to the future.
#[tokio::test]
async fn test_integration_unknown_agent_repeating_advances_not_disabled() {
    let storage = Storage::open_in_memory().expect("storage");
    let working_dir = unique_temp_dir();
    let now = chrono::Utc::now();
    let past = now - chrono::Duration::hours(2);
    let event = CronEvent::new(
        "int-unknown-repeat".to_string(),
        "nonexistent-agent-xyz".to_string(),
        "unknown agent repeating prompt".to_string(),
        CronSchedule::repeat_from(past, 3600),
        "from past every 1h".to_string(),
        past,
    );
    storage.insert_cron_event(&event).expect("insert");

    let processor = create_test_processor();
    let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
    cron_tick(&storage, &processor, &working_dir, &running_events).await;

    // FR-016: repeating event should remain enabled (not disabled).
    let row = storage
        .get_cron_event("int-unknown-repeat")
        .expect("get")
        .expect("found");
    assert!(
        row.enabled,
        "repeating event with unknown agent should remain enabled (advanced, not disabled)"
    );

    // next_due should be advanced to the future.
    let new_next_due = chrono::DateTime::parse_from_rfc3339(&row.next_due).expect("parse next_due");
    assert!(
        new_next_due.with_timezone(&Utc) > now,
        "repeating event next_due should be advanced after unknown-agent error"
    );

    // FR-006: verify the error log entry has all required fields.
    let logs = ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("int-unknown-repeat"));
    let error_entry = logs
        .iter()
        .find(|e| e.outcome == "error")
        .expect("expected an 'error' log entry for unknown agent type");
    assert_eq!(error_entry.event_id, "int-unknown-repeat");
    assert_eq!(error_entry.agent_type, "nonexistent-agent-xyz");
    assert_eq!(error_entry.prompt, "unknown agent repeating prompt");
    assert_eq!(error_entry.schedule, "from past every 1h");
    assert!(
        error_entry
            .error
            .as_ref()
            .is_some_and(|m| m.contains("Unknown agent type")),
        "error message should mention 'Unknown agent type', got: {:?}",
        error_entry.error
    );
    chrono::DateTime::parse_from_rfc3339(&error_entry.timestamp).expect("valid RFC 3339 timestamp");
}

/// Integration test: the double-fire skip (FR-012) produces a log entry
/// with all FR-006 fields, the event remains enabled, `next_due` is
/// advanced, and the event ID stays in the `running_events` set.
///
/// After the running set is manually cleared (simulating completion
/// monitor removal), a second tick fires the event normally (producing
/// an `"error"` log since spawn still fails without a AgentManager).
#[tokio::test]
async fn test_integration_double_fire_skip_then_fire_after_clear() {
    let storage = Storage::open_in_memory().expect("storage");
    let working_dir = unique_temp_dir();
    let now = chrono::Utc::now();
    let past = now - chrono::Duration::hours(2);
    let event = CronEvent::new(
        "int-double-fire".to_string(),
        "general".to_string(),
        "double-fire integration prompt".to_string(),
        CronSchedule::repeat_from(past, 3600),
        "from past every 1h".to_string(),
        past,
    );
    storage.insert_cron_event(&event).expect("insert");

    let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));

    // Simulate a still-running previous execution.
    {
        let mut set = running_events.lock().expect("lock");
        set.insert("int-double-fire".to_string());
    }

    let processor = create_test_processor();

    // First tick: should skip due to double-fire guard (FR-012).
    cron_tick(&storage, &processor, &working_dir, &running_events).await;

    // FR-012: event remains enabled (skipped, not fired).
    let row = storage
        .get_cron_event("int-double-fire")
        .expect("get")
        .expect("found");
    assert!(
        row.enabled,
        "double-fire skipped event should remain enabled"
    );

    // next_due should be advanced to the future.
    let next_due_1 = chrono::DateTime::parse_from_rfc3339(&row.next_due).expect("parse next_due");
    assert!(
        next_due_1.with_timezone(&Utc) > now,
        "next_due should be advanced after double-fire skip"
    );

    // FR-006: verify the skipped log entry has all required fields.
    let logs = ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("int-double-fire"));
    let skipped = logs
        .iter()
        .find(|e| e.outcome == "skipped")
        .expect("expected a 'skipped' log entry for double-fire");
    assert_eq!(skipped.event_id, "int-double-fire");
    assert_eq!(skipped.agent_type, "general");
    assert_eq!(skipped.prompt, "double-fire integration prompt");
    assert_eq!(skipped.schedule, "from past every 1h");
    assert!(
        skipped
            .error
            .as_ref()
            .is_some_and(|m| m.contains("Previous execution still running")),
        "skipped log should mention 'Previous execution still running', got: {:?}",
        skipped.error
    );
    chrono::DateTime::parse_from_rfc3339(&skipped.timestamp).expect("valid RFC 3339 timestamp");

    // Event ID should still be in running_events (guard doesn't remove it).
    {
        let set = running_events.lock().expect("lock");
        assert!(
            set.contains("int-double-fire"),
            "event ID should remain in running_events after double-fire skip"
        );
    }

    // Simulate completion: clear the running_events set.
    {
        let mut set = running_events.lock().expect("lock");
        set.clear();
    }

    // Manually set next_due back to the past so the event is due again
    // (the first tick already advanced it to the future).
    let past_again = chrono::Utc::now() - chrono::Duration::hours(1);
    storage
        .update_cron_event_next_due("int-double-fire", &past_again, None)
        .expect("reset next_due");

    // Second tick: should now fire normally (no double-fire guard).
    // The spawn fails (no AgentManager) so outcome is "error".
    cron_tick(&storage, &processor, &working_dir, &running_events).await;

    let logs2 = ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("int-double-fire"));
    // Should now have both a "skipped" (first tick) and an "error" (second
    // tick, spawn failed) entry.
    assert!(
        logs2.iter().any(|e| e.outcome == "skipped"),
        "should still have the 'skipped' entry from the first tick"
    );
    assert!(
        logs2.iter().any(|e| e.outcome == "error"),
        "should have an 'error' entry from the second tick (spawn failed after guard cleared)"
    );

    // The event should still be enabled (repeating event after error is
    // advanced, not disabled).
    let row2 = storage
        .get_cron_event("int-double-fire")
        .expect("get")
        .expect("found");
    assert!(
        row2.enabled,
        "repeating event should remain enabled after post-guard fire attempt"
    );
}

/// Verify that the scheduler handle can be created and stopped.
#[test]
fn test_scheduler_handle_stop() {
    let handle = CronSchedulerHandle {
        cancel: Arc::new(AtomicBool::new(false)),
    };
    assert!(!handle.cancel.load(Ordering::Relaxed));
    handle.stop();
    assert!(handle.cancel.load(Ordering::Relaxed));
}

/// Verify that dropping the handle signals stop.
#[test]
fn test_scheduler_handle_drop_stops() {
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_clone = Arc::clone(&cancel);
    {
        let _handle = CronSchedulerHandle {
            cancel: cancel_clone,
        };
    } // dropped here
    assert!(cancel.load(Ordering::Relaxed));
}

/// Create a minimal `SessionProcessor` for unit tests.
///
/// The `AgentManager` is not set, so `spawn_background` will fail with
/// "AgentManager not initialized". This is intentional - we want to
/// verify that the tick handles spawn failures gracefully.
fn create_test_processor() -> ragent_agent::session::processor::SessionProcessor {
    use std::sync::Arc;

    let storage = Arc::new(ragent_storage::Storage::open_in_memory().expect("storage"));
    let event_bus = Arc::new(ragent_agent::EventBus::new(1024));
    let session_manager = Arc::new(ragent_agent::session::SessionManager::new(
        storage,
        event_bus.clone(),
    ));
    let provider_registry = Arc::new(ragent_llm::ProviderRegistry::new());
    let tool_registry = Arc::new(ragent_agent::tool::ToolRegistry::new());
    let permission_checker = Arc::new(parking_lot::RwLock::new(
        ragent_agent::permission::PermissionChecker::new(Vec::new()),
    ));

    ragent_agent::session::processor::SessionProcessor {
        session_manager,
        provider_registry,
        tool_registry,
        permission_checker,
        event_bus,
        agent_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
            std::collections::HashMap::new(),
        )),
        tool_repeat_guard: std::sync::Arc::new(parking_lot::Mutex::new(
            std::collections::HashMap::new(),
        )),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        bg_service: std::sync::OnceLock::new(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
        stream_config: ragent_agent::StreamConfig::default(),
        extraction_engine: std::sync::OnceLock::new(),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        cached_config: parking_lot::Mutex::new(None),
        telemetry: std::sync::Arc::new(ragent_telemetry::TelemetrySubsystem::disabled()),
        skill_body_cache: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        activity_log: std::sync::OnceLock::new(),
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(std::collections::HashMap::new()),
    }
}

/// Create a unique temporary directory for cron log isolation.
///
/// Each test gets its own directory so that log entries from one test
/// do not interfere with another test's `read_cron_log` assertions.
fn unique_temp_dir() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("ragent-cron-test-{pid}-{id}"));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

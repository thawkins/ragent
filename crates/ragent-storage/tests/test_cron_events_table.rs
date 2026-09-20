//! Tests for the `cron_events` CRUD storage methods (spec agentchron T-006).
//!
//! These tests verify that all fields of a [`CronEvent`] round-trip through
//! the SQLite `cron_events` table: insert, get, list, list-due, delete,
//! update-next-due, and set-enabled.

use chrono::{DateTime, Utc};
use ragent_storage::storage::Storage;
use ragent_types::{CronEvent, CronSchedule};

/// Parse an RFC-3339 timestamp for test setup.
fn ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
}

/// Build a one-shot event for testing.
fn one_shot(id: &str, at: &str, agent: &str, prompt: &str) -> CronEvent {
    let schedule = CronSchedule::one_shot(ts(at));
    let next_due = ts(at);
    CronEvent::new(
        id.to_string(),
        agent.to_string(),
        prompt.to_string(),
        schedule,
        format!("at {at}"),
        next_due,
    )
}

/// Build a repeat-now event for testing.
fn repeat_now(id: &str, duration_secs: i64, agent: &str, prompt: &str) -> CronEvent {
    let now = Utc::now();
    let schedule = CronSchedule::repeat_now(duration_secs);
    let next_due = schedule.initial_next_due(now);
    CronEvent::new(
        id.to_string(),
        agent.to_string(),
        prompt.to_string(),
        schedule,
        format!("every {duration_secs}s"),
        next_due,
    )
}

/// Build a repeat-from event for testing.
fn repeat_from(id: &str, start: &str, duration_secs: i64, agent: &str, prompt: &str) -> CronEvent {
    let start_ts = ts(start);
    let now = Utc::now();
    let schedule = CronSchedule::repeat_from(start_ts, duration_secs);
    let next_due = schedule.initial_next_due(now);
    CronEvent::new(
        id.to_string(),
        agent.to_string(),
        prompt.to_string(),
        schedule,
        format!("from {start} every {duration_secs}s"),
        next_due,
    )
}

/// Verify that a one-shot event round-trips through the database.
#[test]
fn test_insert_get_one_shot_round_trip() {
    let storage = Storage::open_in_memory().expect("storage");
    let event = one_shot("cron-1", "2025-07-01T09:00:00Z", "general", "Run tests");
    storage.insert_cron_event(&event).expect("insert");

    let row = storage
        .get_cron_event("cron-1")
        .expect("get")
        .expect("row exists");
    assert_eq!(row.id, "cron-1");
    assert_eq!(row.agent_type, "general");
    assert_eq!(row.prompt, "Run tests");
    assert_eq!(row.schedule_form, "one_shot");
    assert_eq!(row.start_at, Some("2025-07-01T09:00:00+00:00".to_string()));
    assert_eq!(row.duration_secs, None);
    assert_eq!(row.schedule_raw, "at 2025-07-01T09:00:00Z");
    assert!(row.enabled);
    assert_eq!(row.next_due, "2025-07-01T09:00:00+00:00");
    assert!(!row.created_at.is_empty());
    assert_eq!(row.last_fired, None);
}

/// Verify that a repeat-now event round-trips with duration_secs set.
#[test]
fn test_insert_get_repeat_now_round_trip() {
    let storage = Storage::open_in_memory().expect("storage");
    let event = repeat_now("cron-2", 1800, "coder", "Build");
    storage.insert_cron_event(&event).expect("insert");

    let row = storage
        .get_cron_event("cron-2")
        .expect("get")
        .expect("row exists");
    assert_eq!(row.schedule_form, "repeat_now");
    assert_eq!(row.start_at, None);
    assert_eq!(row.duration_secs, Some(1800));
    assert!(row.enabled);
}

/// Verify that a repeat-from event round-trips with both start and duration.
#[test]
fn test_insert_get_repeat_from_round_trip() {
    let storage = Storage::open_in_memory().expect("storage");
    let event = repeat_from(
        "cron-3",
        "2025-07-01T09:00:00Z",
        3600,
        "general",
        "Hourly check",
    );
    storage.insert_cron_event(&event).expect("insert");

    let row = storage
        .get_cron_event("cron-3")
        .expect("get")
        .expect("row exists");
    assert_eq!(row.schedule_form, "repeat_from");
    assert_eq!(row.start_at, Some("2025-07-01T09:00:00+00:00".to_string()));
    assert_eq!(row.duration_secs, Some(3600));
}

/// Verify that get_cron_event returns None for a non-existent id.
#[test]
fn test_get_cron_event_nonexistent() {
    let storage = Storage::open_in_memory().expect("storage");
    let row = storage.get_cron_event("does-not-exist").expect("query");
    assert!(row.is_none());
}

/// Verify that list_cron_events returns all events ordered by next_due.
#[test]
fn test_list_cron_events_ordered_by_next_due() {
    let storage = Storage::open_in_memory().expect("storage");
    // Insert in reverse order of next_due.
    let e1 = one_shot("cron-late", "2025-12-01T09:00:00Z", "general", "late");
    let e2 = one_shot("cron-early", "2025-01-01T09:00:00Z", "general", "early");
    let e3 = one_shot("cron-mid", "2025-06-01T09:00:00Z", "general", "mid");
    storage.insert_cron_event(&e1).expect("insert 1");
    storage.insert_cron_event(&e2).expect("insert 2");
    storage.insert_cron_event(&e3).expect("insert 3");

    let rows = storage.list_cron_events().expect("list");
    assert_eq!(rows.len(), 3);
    // Should be ordered by next_due ascending.
    assert_eq!(rows[0].id, "cron-early");
    assert_eq!(rows[1].id, "cron-mid");
    assert_eq!(rows[2].id, "cron-late");
}

/// Verify that delete_cron_event removes an event and returns true.
#[test]
fn test_delete_cron_event() {
    let storage = Storage::open_in_memory().expect("storage");
    let event = one_shot("cron-del", "2025-07-01T09:00:00Z", "general", "temp");
    storage.insert_cron_event(&event).expect("insert");

    let deleted = storage.delete_cron_event("cron-del").expect("delete");
    assert!(deleted);

    let row = storage.get_cron_event("cron-del").expect("get");
    assert!(row.is_none());

    // Deleting again returns false.
    let deleted_again = storage.delete_cron_event("cron-del").expect("delete again");
    assert!(!deleted_again);
}

/// Verify that list_due_cron_events returns only enabled events whose
/// next_due has passed.
#[test]
fn test_list_due_cron_events() {
    let storage = Storage::open_in_memory().expect("storage");
    let now = Utc::now();
    let past = now - chrono::Duration::hours(1);
    let future = now + chrono::Duration::hours(1);

    // Due: next_due is in the past.
    let due = CronEvent::new(
        "cron-due".to_string(),
        "general".to_string(),
        "p".to_string(),
        CronSchedule::one_shot(past),
        "at past".to_string(),
        past,
    );
    // Not due: next_due is in the future.
    let not_due = CronEvent::new(
        "cron-future".to_string(),
        "general".to_string(),
        "p".to_string(),
        CronSchedule::one_shot(future),
        "at future".to_string(),
        future,
    );
    // Due but disabled.
    let disabled_due = {
        let mut e = CronEvent::new(
            "cron-disabled".to_string(),
            "general".to_string(),
            "p".to_string(),
            CronSchedule::one_shot(past),
            "at past".to_string(),
            past,
        );
        e.enabled = false;
        e
    };

    storage.insert_cron_event(&due).expect("insert due");
    storage.insert_cron_event(&not_due).expect("insert not_due");
    storage
        .insert_cron_event(&disabled_due)
        .expect("insert disabled");

    let due_rows = storage.list_due_cron_events(&now).expect("list due");
    // Only the enabled, past-due event should appear.
    assert_eq!(due_rows.len(), 1);
    assert_eq!(due_rows[0].id, "cron-due");
}

/// Verify that update_cron_event_next_due advances next_due and sets
/// last_fired.
#[test]
fn test_update_cron_event_next_due() {
    let storage = Storage::open_in_memory().expect("storage");
    let event = repeat_now("cron-adv", 3600, "general", "p");
    storage.insert_cron_event(&event).expect("insert");

    let new_next = event.next_due + chrono::Duration::seconds(3600);
    let fired_at = Utc::now();

    let updated = storage
        .update_cron_event_next_due("cron-adv", &new_next, Some(&fired_at))
        .expect("update");
    assert!(updated);

    let row = storage
        .get_cron_event("cron-adv")
        .expect("get")
        .expect("row");
    assert_eq!(row.next_due, new_next.to_rfc3339());
    assert_eq!(row.last_fired, Some(fired_at.to_rfc3339()));
}

/// Verify that set_cron_event_enabled toggles the enabled flag.
#[test]
fn test_set_cron_event_enabled() {
    let storage = Storage::open_in_memory().expect("storage");
    let event = one_shot("cron-toggle", "2025-07-01T09:00:00Z", "general", "p");
    storage.insert_cron_event(&event).expect("insert");
    // Should start enabled.
    let row = storage
        .get_cron_event("cron-toggle")
        .expect("get")
        .expect("row");
    assert!(row.enabled);

    // Disable.
    storage
        .set_cron_event_enabled("cron-toggle", false)
        .expect("disable");
    let row = storage
        .get_cron_event("cron-toggle")
        .expect("get")
        .expect("row");
    assert!(!row.enabled);

    // Re-enable.
    storage
        .set_cron_event_enabled("cron-toggle", true)
        .expect("enable");
    let row = storage
        .get_cron_event("cron-toggle")
        .expect("get")
        .expect("row");
    assert!(row.enabled);
}

/// Verify that inserting a duplicate id fails.
#[test]
fn test_insert_cron_event_duplicate_fails() {
    let storage = Storage::open_in_memory().expect("storage");
    let event = one_shot("cron-dup", "2025-07-01T09:00:00Z", "general", "p");
    storage.insert_cron_event(&event).expect("first insert");

    // Second insert with same id should fail.
    let result = storage.insert_cron_event(&event);
    assert!(result.is_err());
}

/// Verify that update_cron_event_next_due returns false for non-existent id.
#[test]
fn test_update_next_due_nonexistent() {
    let storage = Storage::open_in_memory().expect("storage");
    let now = Utc::now();
    let updated = storage
        .update_cron_event_next_due("no-such-id", &now, Some(&now))
        .expect("update");
    assert!(!updated);
}

/// Verify that set_cron_event_enabled returns false for non-existent id.
#[test]
fn test_set_enabled_nonexistent() {
    let storage = Storage::open_in_memory().expect("storage");
    let updated = storage
        .set_cron_event_enabled("no-such-id", false)
        .expect("set enabled");
    assert!(!updated);
}

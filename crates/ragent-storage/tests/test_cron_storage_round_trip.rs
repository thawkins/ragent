//! Comprehensive storage round-trip tests for the `cron_events` table
//! (spec agentchron T-018).
//!
//! These tests verify that **every field** of a [`CronEvent`] survives the
//! insert → get cycle with exact fidelity, and that subsequent update and
//! toggle operations also preserve field integrity.  This is a stricter
//! superset of the basic CRUD tests in `test_cron_events_table.rs`.

use chrono::{DateTime, Utc};
use ragent_storage::storage::Storage;
use ragent_types::{CronEvent, CronForm, CronSchedule};

// ── helpers ───────────────────────────────────────────────────────────

/// Parse an RFC-3339 timestamp for test setup.
fn ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
}

/// Assert every column of a `CronEventRow` matches the original `CronEvent`.
///
/// This is the core "all fields persist" assertion used by every round-trip
/// test in this file.
fn assert_all_fields(event: &CronEvent, row: &ragent_storage::CronEventRow) {
    assert_eq!(row.id, event.id, "id mismatch");
    assert_eq!(row.agent_type, event.agent_type, "agent_type mismatch");
    assert_eq!(row.prompt, event.prompt, "prompt mismatch");

    // schedule_form
    let expected_form = match event.schedule.form {
        CronForm::OneShot => "one_shot",
        CronForm::RepeatFrom => "repeat_from",
        CronForm::RepeatNow => "repeat_now",
    };
    assert_eq!(row.schedule_form, expected_form, "schedule_form mismatch");

    // start_at
    match (&event.schedule.start_at, &row.start_at) {
        (Some(orig), Some(db)) => {
            let db_parsed = DateTime::parse_from_rfc3339(db)
                .unwrap()
                .with_timezone(&Utc);
            assert_eq!(db_parsed, *orig, "start_at value mismatch");
        }
        (None, None) => {}
        (Some(_), None) => panic!("start_at was Some but persisted as None"),
        (None, Some(_)) => panic!("start_at was None but persisted as Some"),
    }

    // duration_secs
    assert_eq!(
        row.duration_secs, event.schedule.duration_secs,
        "duration_secs mismatch"
    );

    // schedule_raw
    assert_eq!(
        row.schedule_raw, event.schedule_raw,
        "schedule_raw mismatch"
    );

    // enabled
    assert_eq!(row.enabled, event.enabled, "enabled mismatch");

    // next_due
    let db_next = DateTime::parse_from_rfc3339(&row.next_due)
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(db_next, event.next_due, "next_due mismatch");

    // created_at
    let db_created = DateTime::parse_from_rfc3339(&row.created_at)
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(db_created, event.created_at, "created_at mismatch");

    // last_fired
    match (&event.last_fired, &row.last_fired) {
        (Some(orig), Some(db)) => {
            let db_parsed = DateTime::parse_from_rfc3339(db)
                .unwrap()
                .with_timezone(&Utc);
            assert_eq!(db_parsed, *orig, "last_fired value mismatch");
        }
        (None, None) => {}
        (Some(_), None) => panic!("last_fired was Some but persisted as None"),
        (None, Some(_)) => panic!("last_fired was None but persisted as Some"),
    }
}

// ── one-shot full round-trip ──────────────────────────────────────────

/// A one-shot event with all fields populated should survive insert → get
/// with every column intact.
#[test]
fn test_round_trip_one_shot_all_fields() {
    let storage = Storage::open_in_memory().expect("storage");
    let event = CronEvent::new(
        "evt-one-shot".to_string(),
        "general".to_string(),
        "Run the nightly test suite".to_string(),
        CronSchedule::one_shot(ts("2025-07-15T09:00:00Z")),
        "at 2025-07-15T09:00:00Z".to_string(),
        ts("2025-07-15T09:00:00Z"),
    );

    storage.insert_cron_event(&event).expect("insert");
    let row = storage
        .get_cron_event("evt-one-shot")
        .expect("get")
        .expect("row exists");
    assert_all_fields(&event, &row);
}

// ── repeat-from full round-trip ───────────────────────────────────────

/// A repeat-from event with both start_at and duration_secs should persist
/// every field.
#[test]
fn test_round_trip_repeat_from_all_fields() {
    let storage = Storage::open_in_memory().expect("storage");
    let start = ts("2025-03-01T08:00:00Z");
    let schedule = CronSchedule::repeat_from(start, 3_600);
    let next_due = schedule.initial_next_due(Utc::now());
    let event = CronEvent::new(
        "evt-repeat-from".to_string(),
        "coder".to_string(),
        "Build and deploy".to_string(),
        schedule,
        "from 2025-03-01T08:00:00Z every 1h".to_string(),
        next_due,
    );

    storage.insert_cron_event(&event).expect("insert");
    let row = storage
        .get_cron_event("evt-repeat-from")
        .expect("get")
        .expect("row exists");
    assert_all_fields(&event, &row);
}

// ── repeat-now full round-trip ────────────────────────────────────────

/// A repeat-now event (no explicit start_at) should persist with start_at
/// as None and duration_secs populated.
#[test]
fn test_round_trip_repeat_now_all_fields() {
    let storage = Storage::open_in_memory().expect("storage");
    let schedule = CronSchedule::repeat_now(1_800);
    let next_due = schedule.initial_next_due(Utc::now());
    let event = CronEvent::new(
        "evt-repeat-now".to_string(),
        "architect".to_string(),
        "Check system health".to_string(),
        schedule,
        "every 30m".to_string(),
        next_due,
    );

    storage.insert_cron_event(&event).expect("insert");
    let row = storage
        .get_cron_event("evt-repeat-now")
        .expect("get")
        .expect("row exists");
    assert_all_fields(&event, &row);
}

// ── special characters in prompt ───────────���──────────────────────────

/// A prompt containing quotes, newlines, and unicode should persist
/// byte-for-byte.
#[test]
fn test_round_trip_special_prompt_chars() {
    let storage = Storage::open_in_memory().expect("storage");
    let prompt = "Run \"tests\" with\nnewlines\tand tabs — café naïve";
    let event = CronEvent::new(
        "evt-special".to_string(),
        "debug".to_string(),
        prompt.to_string(),
        CronSchedule::one_shot(ts("2025-07-15T09:00:00Z")),
        "at 2025-07-15T09:00:00Z".to_string(),
        ts("2025-07-15T09:00:00Z"),
    );

    storage.insert_cron_event(&event).expect("insert");
    let row = storage
        .get_cron_event("evt-special")
        .expect("get")
        .expect("row exists");
    assert_all_fields(&event, &row);
}

// ── very long prompt ──────────────────────────────────────────────────

/// A very long prompt (10 KiB) should persist without truncation.
#[test]
fn test_round_trip_long_prompt() {
    let storage = Storage::open_in_memory().expect("storage");
    let prompt = "x".repeat(10_240);
    let event = CronEvent::new(
        "evt-long".to_string(),
        "general".to_string(),
        prompt.clone(),
        CronSchedule::one_shot(ts("2025-07-15T09:00:00Z")),
        "at 2025-07-15T09:00:00Z".to_string(),
        ts("2025-07-15T09:00:00Z"),
    );

    storage.insert_cron_event(&event).expect("insert");
    let row = storage
        .get_cron_event("evt-long")
        .expect("get")
        .expect("row exists");
    assert_all_fields(&event, &row);
    assert_eq!(row.prompt.len(), 10_240);
}

// ─– created_at stability ──────────────────────────────────────────────

/// created_at must not change after an update_cron_event_next_due call.
/// Only next_due and last_fired should be modified.
#[test]
fn test_created_at_stable_after_update() {
    let storage = Storage::open_in_memory().expect("storage");
    let schedule = CronSchedule::repeat_now(3_600);
    let next_due = schedule.initial_next_due(Utc::now());
    let event = CronEvent::new(
        "evt-stable".to_string(),
        "general".to_string(),
        "p".to_string(),
        schedule,
        "every 1h".to_string(),
        next_due,
    );

    storage.insert_cron_event(&event).expect("insert");

    let row_before = storage
        .get_cron_event("evt-stable")
        .expect("get")
        .expect("row");
    let original_created_at = row_before.created_at.clone();

    let new_next = event.next_due + chrono::Duration::seconds(3_600);
    let fired_at = Utc::now();
    storage
        .update_cron_event_next_due("evt-stable", &new_next, Some(&fired_at))
        .expect("update");

    let row_after = storage
        .get_cron_event("evt-stable")
        .expect("get")
        .expect("row");
    // created_at must be unchanged.
    assert_eq!(row_after.created_at, original_created_at);
    // next_due and last_fired should be updated.
    let db_next = DateTime::parse_from_rfc3339(&row_after.next_due)
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(db_next, new_next);
    let db_fired = DateTime::parse_from_rfc3339(&row_after.last_fired.unwrap())
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(db_fired, fired_at);
    // Other fields must also be unchanged.
    assert_eq!(row_after.agent_type, event.agent_type);
    assert_eq!(row_after.prompt, event.prompt);
    assert_eq!(row_after.schedule_raw, event.schedule_raw);
    assert_eq!(row_after.duration_secs, event.schedule.duration_secs);
}

// ── set_enabled preserves other fields ───────────────────��────────────

/// Toggling enabled should not alter any other column.
#[test]
fn test_set_enabled_preserves_all_fields() {
    let storage = Storage::open_in_memory().expect("storage");
    let event = CronEvent::new(
        "evt-toggle".to_string(),
        "coder".to_string(),
        "Build project".to_string(),
        CronSchedule::repeat_from(ts("2025-01-01T00:00:00Z"), 1_800),
        "from 2025-01-01T00:00:00Z every 30m".to_string(),
        ts("2025-01-01T00:00:00Z"),
    );

    storage.insert_cron_event(&event).expect("insert");

    // Disable
    storage
        .set_cron_event_enabled("evt-toggle", false)
        .expect("disable");
    let row = storage
        .get_cron_event("evt-toggle")
        .expect("get")
        .expect("row");
    assert!(!row.enabled);
    assert_eq!(row.agent_type, event.agent_type);
    assert_eq!(row.prompt, event.prompt);
    assert_eq!(row.schedule_raw, event.schedule_raw);
    assert_eq!(row.start_at, Some("2025-01-01T00:00:00+00:00".to_string()));
    assert_eq!(row.duration_secs, Some(1_800));
    let db_next = DateTime::parse_from_rfc3339(&row.next_due)
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(db_next, event.next_due);

    // Re-enable
    storage
        .set_cron_event_enabled("evt-toggle", true)
        .expect("enable");
    let row = storage
        .get_cron_event("evt-toggle")
        .expect("get")
        .expect("row");
    assert!(row.enabled);
    assert_eq!(row.agent_type, event.agent_type);
    assert_eq!(row.prompt, event.prompt);
}

// ── list round-trip for mixed forms ───────────────────────────────────

/// Multiple events of different forms should all round-trip through list.
#[test]
fn test_list_round_trip_mixed_forms_all_fields() {
    let storage = Storage::open_in_memory().expect("storage");

    let one_shot = CronEvent::new(
        "mix-one".to_string(),
        "general".to_string(),
        "One".to_string(),
        CronSchedule::one_shot(ts("2025-06-01T09:00:00Z")),
        "at 2025-06-01T09:00:00Z".to_string(),
        ts("2025-06-01T09:00:00Z"),
    );
    let repeat_from = {
        let s = CronSchedule::repeat_from(ts("2025-01-01T00:00:00Z"), 3_600);
        let nd = s.initial_next_due(Utc::now());
        CronEvent::new(
            "mix-from".to_string(),
            "coder".to_string(),
            "From".to_string(),
            s,
            "from 2025-01-01T00:00:00Z every 1h".to_string(),
            nd,
        )
    };
    let repeat_now = {
        let s = CronSchedule::repeat_now(900);
        let nd = s.initial_next_due(Utc::now());
        CronEvent::new(
            "mix-now".to_string(),
            "debug".to_string(),
            "Now".to_string(),
            s,
            "every 15m".to_string(),
            nd,
        )
    };

    storage.insert_cron_event(&one_shot).expect("insert 1");
    storage.insert_cron_event(&repeat_from).expect("insert 2");
    storage.insert_cron_event(&repeat_now).expect("insert 3");

    let rows = storage.list_cron_events().expect("list");
    assert_eq!(rows.len(), 3);

    // Build a lookup map.
    let by_id: std::collections::HashMap<&str, &ragent_storage::CronEventRow> =
        rows.iter().map(|r| (r.id.as_str(), r)).collect();

    assert_all_fields(&one_shot, by_id["mix-one"]);
    assert_all_fields(&repeat_from, by_id["mix-from"]);
    assert_all_fields(&repeat_now, by_id["mix-now"]);
}

// ── full lifecycle round-trip ─────────────────────────────────────────

/// Insert → get → update → get → disable → get → delete → get (None).
/// Verifies every field at each stage.
#[test]
fn test_full_lifecycle_all_fields() {
    let storage = Storage::open_in_memory().expect("storage");

    // 1. Insert
    let schedule = CronSchedule::repeat_from(ts("2025-01-01T00:00:00Z"), 3_600);
    let next_due = ts("2025-01-01T00:00:00Z");
    let event = CronEvent::new(
        "lifecycle".to_string(),
        "general".to_string(),
        "Lifecycle test".to_string(),
        schedule,
        "from 2025-01-01T00:00:00Z every 1h".to_string(),
        next_due,
    );
    storage.insert_cron_event(&event).expect("insert");

    // 2. Get — all fields
    let row = storage
        .get_cron_event("lifecycle")
        .expect("get")
        .expect("row");
    assert_all_fields(&event, &row);

    // 3. Update next_due + last_fired
    let new_next = next_due + chrono::Duration::seconds(3_600);
    let fired_at = Utc::now();
    storage
        .update_cron_event_next_due("lifecycle", &new_next, Some(&fired_at))
        .expect("update");

    // 4. Get — verify updated fields, all others stable
    let row = storage
        .get_cron_event("lifecycle")
        .expect("get")
        .expect("row");
    assert_eq!(row.next_due, new_next.to_rfc3339());
    assert_eq!(row.last_fired, Some(fired_at.to_rfc3339()));
    assert_eq!(row.id, event.id);
    assert_eq!(row.agent_type, event.agent_type);
    assert_eq!(row.prompt, event.prompt);
    assert_eq!(row.schedule_form, "repeat_from");
    assert_eq!(row.schedule_raw, event.schedule_raw);
    assert_eq!(row.duration_secs, Some(3_600));
    assert!(row.enabled);

    // 5. Disable
    storage
        .set_cron_event_enabled("lifecycle", false)
        .expect("disable");

    // 6. Get — verify disabled, all others stable
    let row = storage
        .get_cron_event("lifecycle")
        .expect("get")
        .expect("row");
    assert!(!row.enabled);
    assert_eq!(row.next_due, new_next.to_rfc3339());
    assert_eq!(row.last_fired, Some(fired_at.to_rfc3339()));
    assert_eq!(row.prompt, event.prompt);

    // 7. Delete
    let deleted = storage.delete_cron_event("lifecycle").expect("delete");
    assert!(deleted);

    // 8. Get — None
    let row = storage.get_cron_event("lifecycle").expect("get");
    assert!(row.is_none());
}

// ── last_fired None → Some transition ─────────────────────────────────

/// An event starts with last_fired = None. After update_cron_event_next_due
/// with Some(fired_at), last_fired should become Some.  After update with
/// None, it should become None again (clearing).
#[test]
fn test_last_fired_none_to_some_to_none() {
    let storage = Storage::open_in_memory().expect("storage");
    let schedule = CronSchedule::repeat_now(3_600);
    let next_due = schedule.initial_next_due(Utc::now());
    let event = CronEvent::new(
        "fired-transition".to_string(),
        "general".to_string(),
        "p".to_string(),
        schedule,
        "every 1h".to_string(),
        next_due,
    );
    storage.insert_cron_event(&event).expect("insert");

    // Initially None
    let row = storage
        .get_cron_event("fired-transition")
        .expect("get")
        .expect("row");
    assert!(row.last_fired.is_none());

    // Fire once — last_fired becomes Some
    let fired = Utc::now();
    let new_next = event.next_due + chrono::Duration::seconds(3_600);
    storage
        .update_cron_event_next_due("fired-transition", &new_next, Some(&fired))
        .expect("update");
    let row = storage
        .get_cron_event("fired-transition")
        .expect("get")
        .expect("row");
    assert_eq!(row.last_fired, Some(fired.to_rfc3339()));

    // Clear last_fired — pass None
    storage
        .update_cron_event_next_due("fired-transition", &new_next, None)
        .expect("clear");
    let row = storage
        .get_cron_event("fired-transition")
        .expect("get")
        .expect("row");
    assert!(row.last_fired.is_none());
    // next_due should be unchanged from the last update
    assert_eq!(row.next_due, new_next.to_rfc3339());
}

// ── timezone-aware timestamp round-trip ───────────────────────────────

/// A timestamp with a non-UTC offset should be normalised to UTC on storage
/// and the round-tripped value should represent the same instant.
#[test]
fn test_round_trip_non_utc_timestamp() {
    let storage = Storage::open_in_memory().expect("storage");
    // 2025-07-15T11:00:00+02:00  ==  2025-07-15T09:00:00Z
    let at = ts("2025-07-15T11:00:00+02:00");
    let event = CronEvent::new(
        "evt-tz".to_string(),
        "general".to_string(),
        "Timezone test".to_string(),
        CronSchedule::one_shot(at),
        "at 2025-07-15T11:00:00+02:00".to_string(),
        at,
    );

    storage.insert_cron_event(&event).expect("insert");
    let row = storage
        .get_cron_event("evt-tz")
        .expect("get")
        .expect("row exists");

    // The stored start_at should represent the same instant.
    let db_start = DateTime::parse_from_rfc3339(&row.start_at.unwrap())
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(db_start, at);
    // And next_due.
    let db_next = DateTime::parse_from_rfc3339(&row.next_due)
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(db_next, at);
}

// ─– month-duration round-trip ─────────────────────────────────────────

/// A month-duration (2_592_000 s) event should persist with the correct
/// duration_secs value.
#[test]
fn test_round_trip_month_duration() {
    let storage = Storage::open_in_memory().expect("storage");
    let schedule = CronSchedule::repeat_now(2_592_000); // 30 days
    let next_due = schedule.initial_next_due(Utc::now());
    let event = CronEvent::new(
        "evt-month".to_string(),
        "general".to_string(),
        "Monthly report".to_string(),
        schedule,
        "every 1mo".to_string(),
        next_due,
    );

    storage.insert_cron_event(&event).expect("insert");
    let row = storage
        .get_cron_event("evt-month")
        .expect("get")
        .expect("row");
    assert_all_fields(&event, &row);
    assert_eq!(row.duration_secs, Some(2_592_000));
}

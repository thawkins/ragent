//! Isolation coverage for the cron-event lifecycle and automation-run history
//! (`set_cron_event_enabled`, `update_cron_event_next_due`,
//! `list_disabled_due_cron_events`, `list_due_cron_events`,
//! `delete_cron_event`, `prune_automation_runs`, `get_automation_run`).
//!
//! The existing `test_cron_events_table.rs` covers the round-trip; this file
//! targets the scheduler-driving branches: the enabled/disabled due split and
//! the next-due touch, plus the durable automation run-history pruning.

use chrono::{DateTime, Duration, Utc};
use ragent_storage::storage::Storage;
use ragent_types::{AutomationRun, AutomationTrigger, CronEvent, CronSchedule, RunOutcome};

fn ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
}

fn one_shot(id: &str, at: &str) -> CronEvent {
    let schedule = CronSchedule::one_shot(ts(at));
    CronEvent::new(
        id.to_string(),
        "general".to_string(),
        format!("run {id}"),
        schedule,
        format!("at {at}"),
        ts(at),
    )
}

#[test]
fn enabled_and_disabled_due_split() {
    let storage = Storage::open_in_memory().expect("storage");
    let past = "2020-01-01T00:00:00Z";
    let future = "2999-01-01T00:00:00Z";
    storage
        .insert_cron_event(&one_shot("due-enabled", past))
        .expect("e1");
    storage
        .insert_cron_event(&one_shot("due-disabled", past))
        .expect("e2");
    storage
        .insert_cron_event(&one_shot("future-enabled", future))
        .expect("e3");

    assert!(
        storage
            .set_cron_event_enabled("due-disabled", false)
            .expect("disable")
    );

    let now = Utc::now();
    let due = storage.list_due_cron_events(&now).expect("due");
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].id, "due-enabled");

    let disabled = storage
        .list_disabled_due_cron_events(&now)
        .expect("disabled due");
    assert_eq!(disabled.len(), 1);
    assert_eq!(disabled[0].id, "due-disabled");
    assert!(!disabled[0].enabled);

    // Re-enabling moves the row back into the enabled set.
    assert!(
        storage
            .set_cron_event_enabled("due-disabled", true)
            .expect("enable")
    );
    assert_eq!(
        storage.list_due_cron_events(&now).expect("due again").len(),
        2
    );
    assert!(
        storage
            .list_disabled_due_cron_events(&now)
            .expect("disabled again")
            .is_empty()
    );

    // Toggling an unknown id is a no-op, not an error.
    assert!(
        !storage
            .set_cron_event_enabled("nope", false)
            .expect("unknown")
    );
}

#[test]
fn update_cron_event_next_due_and_delete() {
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .insert_cron_event(&one_shot("c1", "2020-01-01T00:00:00Z"))
        .expect("insert");

    let fired = ts("2020-01-01T00:00:00Z");
    let next = fired + Duration::seconds(600);
    assert!(
        storage
            .update_cron_event_next_due("c1", &next, Some(&fired))
            .expect("update")
    );
    let row = storage.get_cron_event("c1").expect("get").expect("row");
    assert!(row.next_due.starts_with("2020-01-01T00:10:00"));
    assert!(
        row.last_fired
            .as_deref()
            .unwrap_or("")
            .starts_with("2020-01-01T00:00:00")
    );

    // Updating a missing id reports false.
    assert!(
        !storage
            .update_cron_event_next_due("missing", &next, None)
            .expect("missing")
    );

    assert!(storage.delete_cron_event("c1").expect("delete"));
    assert!(storage.get_cron_event("c1").expect("get").is_none());
    assert!(!storage.delete_cron_event("c1").expect("delete again"));
}

fn run(id: &str, automation: &str) -> AutomationRun {
    AutomationRun::start(
        id.to_string(),
        automation.to_string(),
        AutomationTrigger::Schedule,
        "local".to_string(),
    )
}

#[test]
fn automation_run_finish_and_get() {
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .insert_automation_run(&run("r1", "auto"))
        .expect("insert");

    let fetched = storage
        .get_automation_run("r1")
        .expect("get")
        .expect("present");
    assert_eq!(fetched.automation_id, "auto");
    assert!(fetched.ended_at.is_none());

    assert!(
        storage
            .finish_automation_run("r1", RunOutcome::Cancelled, None)
            .expect("finish")
    );
    let done = storage
        .get_automation_run("r1")
        .expect("get")
        .expect("present");
    assert_eq!(done.outcome, Some(RunOutcome::Cancelled));
    assert!(done.ended_at.is_some());

    // Finishing a missing run reports false.
    assert!(
        !storage
            .finish_automation_run("missing", RunOutcome::Success, None)
            .expect("missing")
    );
    assert!(
        storage
            .get_automation_run("missing")
            .expect("get missing")
            .is_none()
    );
}

#[test]
fn prune_automation_runs_keeps_newest() {
    let storage = Storage::open_in_memory().expect("storage");
    // Insert runs oldest-first; `list_automation_runs` orders newest-first by
    // started_at, so insert with a strictly increasing timestamp by sleeping
    // is too slow - instead bump started_at directly after each insert.
    for i in 0..5 {
        storage
            .insert_automation_run(&run(&format!("r{i}"), "auto"))
            .expect("insert");
        {
            let conn = storage.conn_lock_for_test().expect("lock");
            conn.execute(
                "UPDATE automation_runs SET started_at = ?1 WHERE id = ?2",
                (
                    format!("2024-01-0{}T00:00:00+00:00", i + 1),
                    format!("r{i}"),
                ),
            )
            .expect("bump");
        }
    }

    let removed = storage.prune_automation_runs("auto", 2).expect("prune");
    assert_eq!(removed, 3);
    let remaining = storage.list_automation_runs("auto", 10).expect("list");
    assert_eq!(remaining.len(), 2);
    // Newest two survive.
    assert_eq!(remaining[0].id, "r4");
    assert_eq!(remaining[1].id, "r3");

    // Pruning below the current count removes nothing.
    assert_eq!(
        storage
            .prune_automation_runs("auto", 10)
            .expect("prune noop"),
        0
    );
}

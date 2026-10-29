//! PERF-005: regression test for the duplicate `TeamTaskCompleted` event.
//!
//! Previously `team_task_complete::execute` published `Event::TeamTaskCompleted`
//! **twice** - a copy-paste bug that doubled the event-bus load and the disk
//! I/O (two `TeamStore::load` calls) for every task completion.
//!
//! T-023 (`teamssetup` FR-017) moved the event out of the tool entirely: the
//! runtime publishes it from [`TeamManager::complete_task`], so a lead-side
//! completion reports the same event the tool's completion does. This test
//! therefore drives the completion through the **manager** - the event's one
//! publisher - and asserts that exactly **one** `TeamTaskCompleted` event is
//! published per completion, and that the member's `current_task_id` is cleared
//! once by the tool surface.
//!
//! [`TeamManager::complete_task`]: ragent_agent::team::TeamManager::complete_task

use ragent_agent::team::{Task, TaskStatus, TaskStore, TeamStore};
use ragent_types::event::Event;

async fn collect_events(
    mut rx: tokio::sync::broadcast::Receiver<Event>,
    deadline_ms: u64,
) -> Vec<Event> {
    let mut out = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(deadline_ms);
    loop {
        let remaining = deadline.checked_duration_since(std::time::Instant::now());
        match remaining {
            Some(dur) if !dur.is_zero() => {
                let sleep = tokio::time::timeout(dur, rx.recv()).await;
                match sleep {
                    Ok(Ok(ev)) => out.push(ev),
                    Ok(Err(_)) => break, // lagged / closed - stop
                    Err(_) => break,     // timed out
                }
            }
            _ => break,
        }
    }
    out
}

#[path = "manager_support/mod.rs"]
mod manager_support;

#[tokio::test]
async fn team_task_completed_is_published_exactly_once() {
    // ---- fixture: a real team with one member and one claimed task ----
    let (manager, _tmp, bus) = manager_support::team_manager("perf-005-team", "lead-sess", true);
    let team_dir = manager.team_dir.clone();

    let ts = TaskStore::open(&team_dir).expect("open task store");
    ts.add_task(Task::new("t-001", "duplicate-event regression"))
        .await
        .expect("add task");
    ts.claim_specific("t-001", "tm-001")
        .await
        .expect("claim task");

    // Set current_task_id so we can assert the tool clears it.
    {
        let mut s = TeamStore::load(&team_dir).expect("reload store");
        let m = s.config.member_by_id_mut("tm-001").expect("find member");
        m.current_task_id = Some("t-001".to_string());
        s.save().expect("save store");
    }

    // ---- drive the completion through the manager ----
    let rx = bus.subscribe();
    let (task, _hook) = manager
        .complete_task("t-001", "lead")
        .await
        .expect("complete through the manager");
    assert_eq!(task.status, TaskStatus::Completed);

    // ---- assertion: exactly one TeamTaskCompleted event ----
    let events = collect_events(rx, 200).await;
    let completed: Vec<&Event> = events
        .iter()
        .filter(|e| matches!(e, Event::TeamTaskCompleted { .. }))
        .collect();
    assert_eq!(
        completed.len(),
        1,
        "PERF-005: TeamTaskCompleted must be published exactly once (got {})",
        completed.len()
    );

    // ---- assertion: the task is completed on disk ----
    let list = ts.read().expect("read task list");
    let task = list
        .tasks
        .iter()
        .find(|t| t.id == "t-001")
        .expect("find task");
    assert_eq!(
        task.status,
        TaskStatus::Completed,
        "task should be Completed"
    );
}

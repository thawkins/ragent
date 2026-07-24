//! PERF-005: regression test for the duplicate `TeamTaskCompleted` event.
//!
//! Previously `team_task_complete::execute` published `Event::TeamTaskCompleted`
//! **twice** — a copy-paste bug that doubled the event-bus load and the disk
//! I/O (two `TeamStore::load` calls) for every task completion.
//!
//! This test exercises the tool end-to-end against a real (temp-dir) team and
//! asserts that exactly **one** `TeamTaskCompleted` event is published per
//! completion, and that the member's `current_task_id` is cleared once.

use std::sync::Arc;

use ragent_agent::tool::{Tool, ToolContext};
use ragent_team::team::{Task, TaskStatus, TaskStore, TeamMember, TeamStore};
use ragent_team::tools::team_task_complete::TeamTaskCompleteTool;
use ragent_types::event::{Event, EventBus};
use serde_json::json;
use tempfile::tempdir;

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
                    Ok(Err(_)) => break, // lagged / closed — stop
                    Err(_) => break,     // timed out
                }
            }
            _ => break,
        }
    }
    out
}

#[tokio::test]
async fn team_task_complete_publishes_exactly_one_event() {
    // ---- fixture: temp working dir, team, member, assigned task ----
    let dir = tempdir().expect("create temp dir");
    let workdir = dir.path().to_path_buf();
    let teams_root = workdir.join(".ragent").join("teams");
    std::fs::create_dir_all(&teams_root).expect("create teams root");

    let mut store =
        TeamStore::create("perf-005-team", "lead-sess", &workdir, true).expect("create team store");
    let member = TeamMember::new("alice", "tm-001", "general");
    store.add_member(member).expect("add member");
    store.save().expect("save store");

    let team_dir = store.dir.clone();
    let task = Task::new("t-001", "duplicate-event regression");
    let ts = TaskStore::open(&team_dir).expect("open task store");
    ts.add_task(task).expect("add task");
    let _ = ts.claim_specific("t-001", "tm-001").expect("claim task");

    // Set current_task_id so we can assert it's cleared exactly once.
    {
        let mut s = TeamStore::load(&team_dir).expect("reload store");
        let m = s.config.member_by_id_mut("tm-001").expect("find member");
        m.current_task_id = Some("t-001".to_string());
        s.save().expect("save store");
    }

    // ---- tool execution ----
    let event_bus = Arc::new(EventBus::new(256));
    let rx = event_bus.subscribe();
    let ctx = ToolContext {
        session_id: "lead-sess".to_string(),
        working_dir: workdir.clone(),
        event_bus: event_bus.clone(),
        storage: None,
        task_manager: None,
        active_model: None,
        team_context: Some(Arc::new(ragent_agent::tool::TeamContext {
            team_name: "perf-005-team".to_string(),
            agent_id: "tm-001".to_string(),
            is_lead: false,
        })),
        team_manager: None,
        code_index: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        cached_team_dir: std::sync::Arc::new(std::sync::Mutex::new(None)),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    };

    let tool = TeamTaskCompleteTool;
    let input = json!({
        "team_name": "perf-005-team",
        "task_id": "t-001",
    });
    let output = tool
        .execute(input, &ctx)
        .await
        .expect("execute must succeed");
    assert!(
        output
            .metadata
            .as_ref()
            .and_then(|m| m.get("completed"))
            .and_then(serde_json::Value::as_bool)
            == Some(true),
        "tool should report completed=true"
    );

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

    // ---- assertion: current_task_id cleared once ----
    let s = TeamStore::load(&team_dir).expect("reload store");
    let m = s.config.member_by_id("tm-001").expect("find member");
    assert!(
        m.current_task_id.is_none(),
        "current_task_id should be cleared after completion"
    );

    // ---- assertion: task is completed on disk ----
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

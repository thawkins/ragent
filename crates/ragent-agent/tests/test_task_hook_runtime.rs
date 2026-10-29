//! FR-017: the `TaskCreated` / `TaskClaimed` / `TaskCompleted` hooks fire from
//! the team runtime's own mutation path, not from the `team_task_*` tools.
//!
//! F-13 recorded the defect: only the three tools called `run_team_hook`, so a
//! lead-side claim (`team_assign_task`), an auto-claim (`TeamManager`) or an
//! adoption fired no hook at all. T-023 moves the trigger into the mutation:
//! `TaskStore::add_task` fires `TaskCreated`, and every claim/completion entry
//! point funnels through `fire_task_hook`, which derives the event from the
//! transition it is reporting.
//!
//! Strategy: install a hook that records the event it was invoked for (one
//! script per event, each appending its name to a log) and then drive each entry
//! point. Asserting on the log rather than on the tool's output is what makes
//! this a test of the *runtime*, not of a caller: a tool that still called the
//! hook itself would append the name twice.

#[path = "manager_support/mod.rs"]
mod manager_support;

mod team_support;

use std::path::{Path, PathBuf};

use ragent_agent::team::{
    HookEntry, HookEvent, HookOutcome, Task, TaskStatus, TaskStore, TeamStore,
    fire_task_created_hook, fire_task_hook,
};
use ragent_agent::tool::team_task_complete::TeamTaskCompleteTool;
use ragent_agent::tool::{TeamContext, Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::json;
use tempfile::TempDir;

/// Install a hook for `event` that appends `label` to `log`.
///
/// The command is a single-token executable (the hook runner refuses shell
/// metacharacters - SEC-ragent-team-005), so the log path travels in the
/// script's own text rather than through an argument.
#[cfg(unix)]
fn install_marker_hook(team_dir: &Path, event: HookEvent, label: &str, log: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let script = team_dir.join(format!("hook-{label}.sh"));
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\ncat >> '{}' <<'EOF'\n{label}\nEOF\nexit 0\n",
            log.display()
        ),
    )
    .expect("write hook script");
    let mut perms = std::fs::metadata(&script).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).expect("chmod hook script");

    let mut store = TeamStore::load(team_dir).expect("load team store");
    store.config.settings.hooks.push(HookEntry {
        event,
        command: script.to_str().expect("utf8 script path").to_string(),
    });
    store.save().expect("persist hook");
}

/// Read the marker log as a list of fired event labels, in order.
fn fired(log: &Path) -> Vec<String> {
    match std::fs::read_to_string(log) {
        Ok(raw) => raw.lines().map(str::to_string).collect(),
        // No file means the hook never ran.
        Err(_) => Vec::new(),
    }
}

/// A team with a task store, a lead session, and a marker log for each of the
/// three task hooks.
#[cfg(unix)]
fn harness(name: &str) -> (TempDir, PathBuf, PathBuf) {
    let (tmp, teams_root) = team_support::setup_workspace();
    let workdir = tmp.path().to_path_buf();
    let store = TeamStore::create(name, "lead-sess", &workdir, true).expect("create team");
    let team_dir = store.dir.clone();
    assert!(
        team_dir.starts_with(&teams_root),
        "the team must live under the workspace: {}",
        team_dir.display()
    );

    let log = tmp.path().join("hook-firings.log");
    install_marker_hook(&team_dir, HookEvent::TaskCreated, "created", &log);
    install_marker_hook(&team_dir, HookEvent::TaskClaimed, "claimed", &log);
    install_marker_hook(&team_dir, HookEvent::TaskCompleted, "completed", &log);
    (tmp, team_dir, log)
}

// -- TaskCreated ---------------------------------------------------------------

/// `TaskStore::add_task` fires `TaskCreated` — so the `team_create` seed path,
/// which calls the same store method, fires it too (FR-017).
#[cfg(unix)]
#[tokio::test]
async fn add_task_fires_task_created_from_the_store() {
    let (_tmp, team_dir, log) = harness("hook-created");
    let store = TaskStore::open(&team_dir).expect("task store");

    store
        .add_task(Task::new("task-001", "seed"))
        .await
        .expect("add");

    assert_eq!(
        fired(&log),
        ["created"],
        "the store's own add must fire TaskCreated exactly once"
    );
}

/// A refusal leaves `tasks.json` alone and fires nothing.
#[cfg(unix)]
#[tokio::test]
async fn a_duplicate_add_fires_no_hook() {
    let (_tmp, team_dir, log) = harness("hook-created-dup");
    let store = TaskStore::open(&team_dir).expect("task store");
    store
        .add_task(Task::new("task-001", "seed"))
        .await
        .expect("add");
    let before = std::fs::read_to_string(team_dir.join("tasks.json")).expect("read tasks");

    assert!(
        store
            .add_task(Task::new("task-001", "again"))
            .await
            .is_err(),
        "a duplicate id is refused"
    );
    assert_eq!(fired(&log), ["created"], "the refusal fires no hook");
    assert_eq!(
        std::fs::read_to_string(team_dir.join("tasks.json")).expect("read tasks"),
        before,
        "a refused add must not touch tasks.json"
    );
}

// -- TaskClaimed ---------------------------------------------------------------

/// The uncached claim path fires the claim hook.
#[cfg(unix)]
#[tokio::test]
async fn claim_specific_fires_task_claimed() {
    let (_tmp, team_dir, log) = harness("hook-claimed-store");
    let store = TaskStore::open(&team_dir).expect("task store");
    store
        .add_task(Task::new("task-001", "work"))
        .await
        .expect("add");

    store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("claim");

    assert_eq!(fired(&log), ["created", "claimed"]);
}

/// The lead-side assignment path fires the claim hook: F-13's headline case, a
/// task assigned by `team_assign_task` rather than claimed by the teammate.
#[cfg(unix)]
#[tokio::test]
async fn lead_side_assignment_fires_task_claimed() {
    let (_tmp, team_dir, log) = harness("hook-claimed-assign");
    let store = TaskStore::open(&team_dir).expect("task store");
    store
        .add_task(Task::new("task-001", "work"))
        .await
        .expect("add");

    let assigned = store.assign("task-001", "tm-001").await.expect("assign");

    assert_eq!(assigned.status, TaskStatus::InProgress);
    assert_eq!(assigned.assigned_to.as_deref(), Some("tm-001"));
    assert_eq!(
        fired(&log),
        ["created", "claimed"],
        "a lead-side assignment must fire the claim hook"
    );
}

/// A claim that is already owned is not a transition, so it fires nothing.
#[cfg(unix)]
#[tokio::test]
async fn an_idempotent_claim_fires_no_hook() {
    let (_tmp, team_dir, log) = harness("hook-claimed-idem");
    let store = TaskStore::open(&team_dir).expect("task store");
    store
        .add_task(Task::new("task-001", "work"))
        .await
        .expect("add");
    store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("claim");

    // The second claim reports "already owned" and mutates nothing.
    store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("re-claim");

    assert_eq!(
        fired(&log),
        ["created", "claimed"],
        "re-claiming must not fire a second hook"
    );
}

/// A refused claim fires nothing.
#[cfg(unix)]
#[tokio::test]
async fn a_refused_claim_fires_no_hook() {
    let (_tmp, team_dir, log) = harness("hook-claimed-refused");
    let store = TaskStore::open(&team_dir).expect("task store");
    let mut cancelled = Task::new("task-001", "dropped");
    cancelled.status = TaskStatus::Cancelled;
    store.add_task(cancelled).await.expect("add");

    assert!(
        store.claim_specific("task-001", "tm-001").await.is_err(),
        "a cancelled task is not claimable"
    );
    assert_eq!(fired(&log), ["created"], "the refusal fires no hook");
}

// -- TaskCompleted --------------------------------------------------------------

/// The uncached completion path fires the completion hook and hands back its
/// verdict (FR-018's input).
#[cfg(unix)]
#[tokio::test]
async fn complete_fires_task_completed_and_returns_the_verdict() {
    let (_tmp, team_dir, log) = harness("hook-completed-store");
    let store = TaskStore::open(&team_dir).expect("task store");
    store
        .add_task(Task::new("task-001", "work"))
        .await
        .expect("add");
    store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("claim");

    let (done, outcome) = store
        .complete("task-001", "tm-001")
        .await
        .expect("complete");

    assert_eq!(done.status, TaskStatus::Completed);
    assert_eq!(
        outcome,
        HookOutcome::Allow,
        "the marker hook allows (exit 0)"
    );
    assert_eq!(fired(&log), ["created", "claimed", "completed"]);
}

/// A refused completion fires nothing.
#[cfg(unix)]
#[tokio::test]
async fn a_refused_completion_fires_no_hook() {
    let (_tmp, team_dir, log) = harness("hook-completed-refused");
    let store = TaskStore::open(&team_dir).expect("task store");
    store
        .add_task(Task::new("task-001", "work"))
        .await
        .expect("add");
    store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("claim");

    // Completing is not claiming (FR-001): another agent does not own it.
    assert!(
        store.complete("task-001", "tm-002").await.is_err(),
        "an outsider cannot complete the task"
    );
    assert_eq!(
        fired(&log),
        ["created", "claimed"],
        "a refused completion fires no hook"
    );
}

// -- the tool no longer fires it ------------------------------------------------

/// The tool must not fire the completion hook itself: one completion, one firing.
///
/// Before T-023 both the tool and (once T-023 landed) the store would call the
/// hook, which is exactly the double-fire F-13 warns about.
#[cfg(unix)]
#[tokio::test]
async fn the_completion_tool_fires_the_hook_once() {
    let (tmp, team_dir, log) = harness("hook-completed-tool");
    let workdir = tmp.path().to_path_buf();
    let store = TaskStore::open(&team_dir).expect("task store");
    store
        .add_task(Task::new("task-001", "work"))
        .await
        .expect("add");
    store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("claim");

    let event_bus = std::sync::Arc::new(EventBus::new(64));
    let ctx = ToolContext {
        session_id: "lead-sess".to_string(),
        working_dir: workdir,
        event_bus: event_bus.clone(),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: Some(std::sync::Arc::new(TeamContext {
            team_name: "hook-completed-tool".to_string(),
            agent_id: "tm-001".to_string(),
            is_lead: false,
        })),
        team_manager: None,
        code_index: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        bg_service: None,
        cached_team_dir: std::sync::Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        canonical_cache: std::sync::Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: Vec::new(),
        tool_registry: ToolContext::default_tool_registry(),
    };

    let out = TeamTaskCompleteTool
        .execute(
            json!({"team_name": "hook-completed-tool", "task_id": "task-001"}),
            &ctx,
        )
        .await
        .expect("execute");

    assert_eq!(
        out.metadata
            .as_ref()
            .and_then(|m| m.get("completed"))
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "the tool must report completion: {}",
        out.content
    );
    assert_eq!(
        fired(&log),
        ["created", "claimed", "completed"],
        "the completion hook must fire exactly once, from the store"
    );
}

// -- the event derivation -------------------------------------------------------

/// `fire_task_hook` derives the event from the transition it reports, and fires
/// nothing for a status that has no hook event.
#[cfg(unix)]
#[tokio::test]
async fn fire_task_hook_maps_each_transition_to_its_event() {
    let (_tmp, team_dir, log) = harness("hook-mapping");
    let mut task = Task::new("task-001", "work");
    let claim = ragent_agent::team::TaskTransition {
        task_id: "task-001".to_string(),
        from: TaskStatus::Pending,
        to: TaskStatus::InProgress,
        actor: "tm-001".to_string(),
    };
    fire_task_hook(&team_dir, Some(&claim), &task, "tm-001")
        .await
        .expect("claim hook");
    assert_eq!(fired(&log), ["claimed"]);

    task.status = TaskStatus::Completed;
    task.completed_by = Some("tm-001".to_string());
    task.completed_at = Some(chrono::Utc::now());
    let done = ragent_agent::team::TaskTransition {
        task_id: "task-001".to_string(),
        from: TaskStatus::InProgress,
        to: TaskStatus::Completed,
        actor: "tm-001".to_string(),
    };
    fire_task_hook(&team_dir, Some(&done), &task, "tm-001")
        .await
        .expect("completion hook");
    assert_eq!(fired(&log), ["claimed", "completed"]);

    // A cancellation has no hook event in this schema.
    let cancelled = ragent_agent::team::TaskTransition {
        task_id: "task-001".to_string(),
        from: TaskStatus::Pending,
        to: TaskStatus::Cancelled,
        actor: "lead".to_string(),
    };
    let outcome = fire_task_hook(&team_dir, Some(&cancelled), &task, "lead")
        .await
        .expect("cancellation asks nothing");
    assert_eq!(outcome, HookOutcome::Allow);
    assert_eq!(
        fired(&log),
        ["claimed", "completed"],
        "a cancellation fires no hook"
    );

    // Creation is the transition-less case.
    fire_task_created_hook(&team_dir, &task)
        .await
        .expect("creation hook");
    assert_eq!(fired(&log), ["claimed", "completed", "created"]);
}

/// A hook lookup against a directory with no team config is an error, not a
/// silent "allowed" - the difference matters to the caller that acts on the
/// verdict (T-025).
#[tokio::test]
async fn fire_task_hook_reports_a_missing_team_store() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let missing = tmp.path().join("no-such-team");
    let task = Task::new("task-001", "work");

    let err = fire_task_created_hook(&missing, &task)
        .await
        .expect_err("a team directory with no config cannot be asked");
    assert!(
        err.to_string().contains("load team store"),
        "the error must name the failing step: {err}"
    );
}

// -- the manager path -----------------------------------------------------------

/// A `TeamManager` completion runs the hook too, so a lead-side completion is
/// gated like the tool's (FR-017).
#[cfg(unix)]
#[tokio::test]
async fn manager_completion_fires_the_hook() {
    let (manager, tmp, _bus) =
        manager_support::team_manager("hook-completed-manager", "lead", true);
    let team_dir = manager.team_dir.clone();
    let log = tmp.path().join("hook-firings.log");
    install_marker_hook(&team_dir, HookEvent::TaskCreated, "created", &log);
    install_marker_hook(&team_dir, HookEvent::TaskClaimed, "claimed", &log);
    install_marker_hook(&team_dir, HookEvent::TaskCompleted, "completed", &log);

    let store = TaskStore::open(&team_dir).expect("task store");
    store
        .add_task(Task::new("task-001", "work"))
        .await
        .expect("add");
    store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("claim");

    let (done, outcome) = manager
        .complete_task("task-001", "lead")
        .await
        .expect("lead completes on the owner's behalf");

    assert_eq!(done.status, TaskStatus::Completed);
    assert_eq!(outcome, HookOutcome::Allow);
    assert_eq!(fired(&log), ["created", "claimed", "completed"]);
}

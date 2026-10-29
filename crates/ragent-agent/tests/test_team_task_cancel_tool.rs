//! The `team_task_cancel` tool (spec `teamssetup` T-012, FR-005, FR-042).
//!
//! Before this tool the runtime had **no** cancellation surface at all: F-05
//! recorded that `TaskStatus::Cancelled` existed in the enum, `as_str` and the
//! serde table, but no tool and no `/team` subcommand ever wrote it, so a lead
//! could only abandon work by editing `tasks.json` by hand. These tests pin the
//! contract the tool is responsible for:
//!
//! 1. **FR-005 - it cancels.** A `pending` or `in_progress` task becomes
//!    `cancelled`, stays in the list, and an `in_progress` task's owner is told
//!    through its mailbox.
//! 2. **FR-005 - lead only.** A teammate calling it is refused by name and
//!    mutates nothing.
//! 3. **FR-006 - the cancel is final.** The cancelled task is refused by every
//!    claim path with a `cancelled` message, not a `pending` one.
//! 4. **FR-042 - the description states the rule.** The description must name
//!    the lead-only rule, both cancellable statuses, the terminal statuses, and
//!    that the task is not deleted.
//!
//! `#[cfg(unix)]` is not needed: every fixture lives in its own
//! `tempfile::tempdir()`, so nothing is written into the repository.

use std::sync::Arc;

use ragent_agent::team::{Mailbox, MessageType, Task, TaskStatus, TaskStore, TeamStore};
use ragent_agent::tool::team_task_cancel::TeamTaskCancelTool;
use ragent_agent::tool::{TeamContext, Tool, ToolContext};
use serde_json::{Value, json};

#[path = "manager_support/mod.rs"]
mod manager_support;

/// The lead's agent id, as `TeamContext` spells it.
const LEAD: &str = "lead";

/// A workspace holding a project-local team named `name`, with one task in
/// `status` assigned to `owner`.
///
/// The team is created through [`TeamStore::create`] so its directory really is
/// `<workspace>/.ragent/teams/<name>` - which is what `find_team_dir`, the
/// lookup every team tool performs from its `working_dir`, resolves. Returns the
/// workspace tempdir (keep it alive), the **working dir** the tool context uses,
/// the team directory, and an open task store.
fn team_with_task(
    name: &str,
    id: &str,
    status: TaskStatus,
    owner: Option<&str>,
) -> (
    tempfile::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
    TaskStore,
) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().to_path_buf();
    std::fs::create_dir_all(workspace.join(".ragent")).expect("create .ragent");
    let store = TeamStore::create(name, LEAD, &workspace, true).expect("create team");
    let team_dir = store.dir.clone();
    let tasks = TaskStore::open(&team_dir).expect("open task store");
    let mut task = Task::new(id, format!("work {id}"));
    task.status = status;
    task.assigned_to = owner.map(str::to_owned);
    tasks
        .write_through(|list| list.tasks.push(task))
        .expect("seed task");
    (tmp, workspace, team_dir, tasks)
}

/// A `ToolContext` whose `working_dir` is the team's workspace and whose
/// `team_context` names `team_name`, acting as `agent_id`.
///
/// `manager` is `None` for the store fallback and `Some` when the test wants the
/// runtime path (the one that sends the FR-005 mailbox notice).
fn tool_context(
    workspace: &std::path::Path,
    team_name: &str,
    agent_id: &str,
    manager: Option<Arc<dyn ragent_agent::tool::TeamManagerInterface>>,
) -> ToolContext {
    let bus = Arc::new(ragent_agent::event::EventBus::new(64));
    ToolContext {
        session_id: agent_id.to_string(),
        working_dir: workspace.to_path_buf(),
        event_bus: bus,
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: Some(Arc::new(TeamContext {
            team_name: team_name.to_string(),
            agent_id: agent_id.to_string(),
            is_lead: agent_id == LEAD,
        })),
        team_manager: manager,
        code_index: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        bg_service: None,
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: Vec::new(),
        tool_registry: ToolContext::default_tool_registry(),
    }
}

/// Run the tool and return its output, asserting the schema shape first.
async fn cancel(
    ctx: &ToolContext,
    team_name: &str,
    task_id: &str,
    reason: Option<&str>,
) -> ragent_agent::tool::ToolOutput {
    let mut input = json!({ "team_name": team_name, "task_id": task_id });
    if let Some(reason) = reason {
        input["reason"] = Value::String(reason.to_string());
    }
    TeamTaskCancelTool
        .execute(input, ctx)
        .await
        .expect("cancelling through the tool must not error")
}

/// The task's persisted status, or `None` when it is not in the list.
fn status_of(store: &TaskStore, id: &str) -> Option<TaskStatus> {
    store
        .read()
        .expect("read tasks")
        .tasks
        .iter()
        .find(|t| t.id == id)
        .map(|t| t.status)
}

// -- FR-005: it cancels -------------------------------------------------------

#[tokio::test]
async fn a_pending_task_is_cancelled_and_stays_in_the_list() {
    let (_tmp, workspace, _team_dir, store) =
        team_with_task("tcancel-tool", "task-001", TaskStatus::Pending, None);
    let ctx = tool_context(&workspace, "tcancel-tool", LEAD, None);

    let out = cancel(&ctx, "tcancel-tool", "task-001", Some("not needed")).await;

    assert_eq!(
        status_of(&store, "task-001"),
        Some(TaskStatus::Cancelled),
        "a pending task must become cancelled"
    );
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["cancelled"], json!(true));
    assert_eq!(meta["status"], json!("cancelled"));
    assert_eq!(meta["from_status"], json!("pending"));
    assert_eq!(meta["task_id"], json!("task-001"));
    assert!(
        meta["notified"].is_null(),
        "an unclaimed task has nobody to notify: {meta}"
    );
    assert!(
        out.content.contains("pending -> cancelled"),
        "the reply must say what moved: {}",
        out.content
    );
    assert!(
        out.content.contains("cannot be claimed or completed again"),
        "the reply must say the cancel is final: {}",
        out.content
    );
}

#[tokio::test]
async fn an_in_progress_task_is_cancelled_and_its_owner_is_notified() {
    let (_tmp, workspace, team_dir, store) = team_with_task(
        "tcancel-owner",
        "task-001",
        TaskStatus::InProgress,
        Some("tm-001"),
    );
    // A manager gives the tool the runtime path, which is the one that sends the
    // FR-005 mailbox notice. It is built over *this* workspace's team directory,
    // so the tool's `find_team_dir` and the manager's roster are one team.
    let (manager, _tmp2, _bus) = manager_support::team_manager_for(&team_dir);
    let ctx = tool_context(
        &workspace,
        "tcancel-owner",
        LEAD,
        Some(manager.clone() as Arc<dyn ragent_agent::tool::TeamManagerInterface>),
    );

    let out = cancel(&ctx, "tcancel-owner", "task-001", Some("superseded")).await;

    assert_eq!(
        status_of(&store, "task-001"),
        Some(TaskStatus::Cancelled),
        "an in-progress task must become cancelled"
    );
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["from_status"], json!("in_progress"));
    assert_eq!(meta["owner"], json!("tm-001"), "the owner is retained");
    assert_eq!(
        meta["notified"],
        json!("tm-001"),
        "the owner must be told: {meta}"
    );

    // The notice itself, from the manager's team directory.
    let mailbox = Mailbox::open(&manager.team_dir, "tm-001").expect("open mailbox");
    let messages = mailbox.drain_unread().expect("drain");
    let notice = messages
        .iter()
        .find(|m| m.message_type == MessageType::TaskCancelled)
        .expect("a TaskCancelled message");
    assert_eq!(notice.from, LEAD);
    assert!(
        notice.content.contains("task-001"),
        "the notice names the task: {}",
        notice.content
    );
    assert!(
        notice.content.contains("superseded"),
        "the notice carries the lead's reason: {}",
        notice.content
    );
}

// -- FR-005: lead only --------------------------------------------------------

#[tokio::test]
async fn a_teammate_calling_the_tool_is_refused_and_mutates_nothing() {
    let (_tmp, workspace, team_dir, store) = team_with_task(
        "tcancel-refuse",
        "task-001",
        TaskStatus::InProgress,
        Some("tm-002"),
    );
    let (manager, _tmp2, _bus) = manager_support::team_manager_for(&team_dir);
    let ctx = tool_context(
        &workspace,
        "tcancel-refuse",
        "tm-002",
        Some(manager.clone() as Arc<dyn ragent_agent::tool::TeamManagerInterface>),
    );

    let err = TeamTaskCancelTool
        .execute(
            json!({ "team_name": "tcancel-refuse", "task_id": "task-001" }),
            &ctx,
        )
        .await
        .expect_err("only the lead may cancel");

    assert!(
        err.to_string().contains("only the lead"),
        "the refusal must name the rule: {err}"
    );
    assert_eq!(
        status_of(&store, "task-001"),
        Some(TaskStatus::InProgress),
        "a refused cancel must change nothing"
    );
}

// -- FR-006: the cancel is final ----------------------------------------------

#[tokio::test]
async fn a_cancelled_task_is_refused_by_every_claim_path() {
    let (_tmp, workspace, _team_dir, store) =
        team_with_task("tcancel-final", "task-001", TaskStatus::Pending, None);
    let ctx = tool_context(&workspace, "tcancel-final", LEAD, None);
    cancel(&ctx, "tcancel-final", "task-001", None).await;

    // The specific claim must name the cancelled status, never "pending".
    let err = store
        .claim_specific("task-001", "tm-001")
        .await
        .expect_err("a cancelled task is not claimable");
    let msg = err.to_string();
    assert!(msg.contains("in status cancelled"), "err: {msg}");
    assert!(
        !msg.contains("in status pending"),
        "a cancelled task must not be described as pending: {msg}"
    );

    // The "next task" scan must skip it.
    let (next, already_had) = store.claim_next("tm-001").await.expect("claim next");
    assert!(
        next.is_none() && !already_had,
        "a cancelled task must never be handed out by the next-task scan"
    );

    // Completion is refused too: the task is terminal.
    let err = store
        .complete("task-001", LEAD)
        .await
        .expect_err("a cancelled task cannot be completed");
    assert!(
        err.to_string().contains("cancelled"),
        "the completion refusal names the cancelled status: {err}"
    );
}

#[tokio::test]
async fn cancelling_a_terminal_task_is_refused_naming_its_status() {
    for status in [TaskStatus::Completed, TaskStatus::Cancelled] {
        let (_tmp, workspace, _team_dir, store) =
            team_with_task("tcancel-terminal", "task-001", status, Some("tm-001"));
        let ctx = tool_context(&workspace, "tcancel-terminal", LEAD, None);

        let err = TeamTaskCancelTool
            .execute(
                json!({ "team_name": "tcancel-terminal", "task_id": "task-001" }),
                &ctx,
            )
            .await
            .expect_err("a terminal task cannot be cancelled");

        assert!(
            err.to_string()
                .contains(&format!("in status {}", status.as_str())),
            "{status:?}: the refusal must name the status the task holds: {err}"
        );
        assert_eq!(
            status_of(&store, "task-001"),
            Some(status),
            "{status:?}: a refused cancel must not change the status"
        );
    }
}

#[tokio::test]
async fn cancelling_an_unknown_task_is_refused() {
    let (_tmp, workspace, _team_dir, _store) =
        team_with_task("tcancel-unknown", "task-001", TaskStatus::Pending, None);
    let ctx = tool_context(&workspace, "tcancel-unknown", LEAD, None);

    let err = TeamTaskCancelTool
        .execute(
            json!({ "team_name": "tcancel-unknown", "task_id": "task-404" }),
            &ctx,
        )
        .await
        .expect_err("an unknown task cannot be cancelled");
    assert!(err.to_string().contains("task-404"), "err: {err}");
}

// -- FR-042: the description states the enforced rule --------------------------

/// The `team_task_cancel` description and schema must state the rule the tool
/// enforces (spec `teamssetup` FR-042).
///
/// FR-042's principle is that a description states what the runtime *does*, so a
/// model's expectation matches it. For cancellation the three facts that matter
/// are the lead-only rule, which statuses are cancellable and that the two
/// terminal ones are not, and that a cancel keeps the task (it does not delete
/// it) - a model that expected a delete would look for a missing task.
#[tokio::test]
async fn the_description_states_the_lead_only_and_status_rules() {
    let registry = ragent_agent::tool::create_default_registry();
    let def = registry
        .definitions()
        .iter()
        .find(|d| d.name == "team_task_cancel")
        .cloned()
        .expect("team_task_cancel must be registered");
    let desc = def.description.to_lowercase();

    for (needle, why) in [
        ("lead-only", "must state the lead-only rule"),
        ("refused", "must say a teammate's attempt is refused"),
        ("pending", "must name the first cancellable status"),
        ("in_progress", "must name the second cancellable status"),
        (
            "completed",
            "must name a terminal status that cannot be cancelled",
        ),
        ("cancelled", "must name the terminal status itself"),
        ("terminal", "must say the terminal statuses are final"),
        ("mailbox", "must say the owning teammate is notified"),
        (
            "can never be claimed or completed again",
            "must state the cancel is final",
        ),
        (
            "does not delete the task",
            "must say the task is kept, not deleted",
        ),
    ] {
        assert!(
            desc.contains(needle),
            "team_task_cancel description must contain {needle:?} ({why}), got: {desc}"
        );
    }

    // The schema: the two required fields, `reason` optional, nothing else.
    let schema = def.parameters;
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .expect("team_task_cancel schema must declare a 'required' array")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(
        required,
        vec!["team_name", "task_id"],
        "only the team and the task are required; `reason` is optional"
    );
    let props = schema
        .get("properties")
        .and_then(Value::as_object)
        .expect("properties");
    let mut names: Vec<&str> = props.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        vec!["reason", "task_id", "team_name"],
        "the schema must declare exactly team_name, task_id and reason"
    );
    assert_eq!(
        schema.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "unknown keys must be rejected, not silently ignored"
    );

    // The permission family: cancelling is a management verb, like assignment
    // and cleanup, not a read and not a claim.
    assert_eq!(
        TeamTaskCancelTool.permission_category(),
        "team:manage",
        "cancelling is lead-only management, so it must gate as `team:manage`"
    );
}

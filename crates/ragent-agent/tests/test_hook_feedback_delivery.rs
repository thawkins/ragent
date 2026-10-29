//! FR-018: a `TaskCompleted` hook's `Feedback` verdict reaches the completing
//! agent as a tool result, whichever path completed the task.
//!
//! F-15 recorded the defect: only the three `team_task_*` tools read exit code
//! 2, and after T-023 moved the hook onto the runtime path (the completion
//! itself) the tool's verdict was simply discarded - a lead-side completion ran
//! the hook and threw the answer away. T-025 makes the tool reach the one
//! completion implementation (`TeamManager::complete_task`) and report the
//! verdict it returns.
//!
//! Strategy: install a `TaskCompleted` hook that exits 2 and prints a known
//! string, then complete a task through `team_task_complete` twice - once with a
//! `TeamManager` wired into the `ToolContext` (the runtime path) and once with
//! only a `TaskStore` (the uncached fallback). Both must show the hook's stdout,
//! and both must leave the task completed, because `Completed` is terminal (D3).
//!
//! `#[cfg(unix)]` guards the script fixtures; every fixture lives in its own
//! `tempfile::tempdir()`, so nothing is written into the repository.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ragent_agent::team::{
    HookEntry, HookEvent, MemberStatus, Task, TaskStatus, TaskStore, TeamStore,
};
use ragent_agent::tool::team_task_complete::TeamTaskCompleteTool;
use ragent_agent::tool::{TeamContext, TeamManagerInterface, Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::{Value, json};
use tempfile::TempDir;

#[path = "manager_support/mod.rs"]
mod manager_support;

/// The text the vetoing hook prints, which the tool result must carry.
const FEEDBACK: &str = "static analysis reported 3 issues";

/// A team named `name` under a fresh workspace, with one `in_progress` task
/// assigned to `tm-001`.
///
/// The team is created through [`TeamStore::create`] so its directory really is
/// `<workspace>/.ragent/teams/<name>` - what `find_team_dir`, the lookup the
/// tool performs from its `working_dir`, resolves.
#[cfg(unix)]
fn harness(name: &str) -> (TempDir, PathBuf, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().to_path_buf();
    std::fs::create_dir_all(workspace.join(".ragent")).expect("create .ragent");
    let store = TeamStore::create(name, "lead-sess", &workspace, true).expect("create team");
    let team_dir = store.dir.clone();
    drop(store);

    let tasks = TaskStore::open(&team_dir).expect("open task store");
    let mut task = Task::new("task-001", "work task-001");
    task.status = TaskStatus::InProgress;
    task.assigned_to = Some("tm-001".to_string());
    tasks
        .write_through(|list| list.tasks.push(task))
        .expect("seed task");

    (tmp, workspace, team_dir)
}

/// Install a `TaskCompleted` hook that vetoes the completion with [`FEEDBACK`].
///
/// The command is a single-token executable (the hook runner refuses shell
/// metacharacters - SEC-ragent-team-005), so the text travels in the script
/// itself rather than through an argument.
#[cfg(unix)]
fn install_vetoing_hook(team_dir: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let script = team_dir.join("hook-veto.sh");
    std::fs::write(
        &script,
        format!("#!/bin/sh\nprintf '%s\\n' '{FEEDBACK}'\nexit 2\n"),
    )
    .expect("write hook script");
    let mut perms = std::fs::metadata(&script).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).expect("chmod hook script");

    let mut store = TeamStore::load(team_dir).expect("load team store");
    store.config.settings.hooks.push(HookEntry {
        event: HookEvent::TaskCompleted,
        command: script.to_str().expect("utf8 script path").to_string(),
    });
    store.save().expect("persist hook");
}

/// A `ToolContext` acting as `agent_id` in team `team_name`, optionally with a
/// real `TeamManager` attached (the runtime path).
fn tool_context(
    workspace: &Path,
    team_name: &str,
    agent_id: &str,
    manager: Option<Arc<dyn TeamManagerInterface>>,
) -> ToolContext {
    ToolContext {
        session_id: "lead-sess".to_string(),
        working_dir: workspace.to_path_buf(),
        event_bus: Arc::new(EventBus::new(64)),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: Some(Arc::new(TeamContext {
            team_name: team_name.to_string(),
            agent_id: agent_id.to_string(),
            is_lead: agent_id == "lead",
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

/// The task's persisted status, or `None` when it is not in the list.
fn status_of(team_dir: &Path, id: &str) -> Option<TaskStatus> {
    TaskStore::open(team_dir)
        .expect("open task store")
        .read()
        .expect("read tasks")
        .tasks
        .iter()
        .find(|t| t.id == id)
        .map(|t| t.status)
}

/// The tool's metadata field `key`, if present.
fn meta_string(out: &ragent_agent::tool::ToolOutput, key: &str) -> Option<String> {
    out.metadata
        .as_ref()
        .and_then(|m| m.get(key))
        .and_then(Value::as_str)
        .map(str::to_string)
}

// -- FR-018: the verdict reaches the agent on the runtime path -----------------

/// The headline case: with a `TeamManager` in the context - the runtime path a
/// real session has - the hook's feedback appears in the tool's output.
#[cfg(unix)]
#[tokio::test]
async fn runtime_completion_delivers_hook_feedback() {
    let (_tmp, workspace, team_dir) = harness("fb-runtime");
    install_vetoing_hook(&team_dir);
    let (manager, _placeholder, _bus) = manager_support::team_manager_for(&team_dir);
    let ctx = tool_context(&workspace, "fb-runtime", "tm-001", Some(manager));

    let out = TeamTaskCompleteTool
        .execute(
            json!({"team_name": "fb-runtime", "task_id": "task-001"}),
            &ctx,
        )
        .await
        .expect("execute");

    assert!(
        out.content.contains(FEEDBACK),
        "the hook's feedback must reach the completing agent, got: {}",
        out.content
    );
    assert_eq!(
        out.metadata
            .as_ref()
            .and_then(|m| m.get("hook_rejected"))
            .and_then(Value::as_bool),
        Some(true),
        "the verdict must be machine-readable too: {:?}",
        out.metadata
    );
    assert_eq!(
        meta_string(&out, "feedback").as_deref().map(str::trim),
        Some(FEEDBACK),
        "the structured feedback must carry the hook's stdout verbatim"
    );
}

/// The completion is not undone: `Completed` is terminal (D3), and a revert
/// would be a second writer of the task status (FR-008, FR-026).
#[cfg(unix)]
#[tokio::test]
async fn a_vetoed_completion_stays_completed() {
    let (_tmp, workspace, team_dir) = harness("fb-stays");
    install_vetoing_hook(&team_dir);
    let (manager, _placeholder, _bus) = manager_support::team_manager_for(&team_dir);
    let ctx = tool_context(&workspace, "fb-stays", "tm-001", Some(manager));

    TeamTaskCompleteTool
        .execute(
            json!({"team_name": "fb-stays", "task_id": "task-001"}),
            &ctx,
        )
        .await
        .expect("execute");

    assert_eq!(
        status_of(&team_dir, "task-001"),
        Some(TaskStatus::Completed),
        "a veto is reported, not acted on: the completion stands"
    );
}

/// The delivery is not the store fallback's: an *allowing* hook reports no
/// feedback and the agent gets an ordinary success.
#[cfg(unix)]
#[tokio::test]
async fn an_allowing_hook_reports_no_feedback() {
    use std::os::unix::fs::PermissionsExt;

    let (_tmp, workspace, team_dir) = harness("fb-allow");
    let script = team_dir.join("hook-allow.sh");
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").expect("write hook script");
    let mut perms = std::fs::metadata(&script).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).expect("chmod hook script");
    let mut store = TeamStore::load(&team_dir).expect("load team store");
    store.config.settings.hooks.push(HookEntry {
        event: HookEvent::TaskCompleted,
        command: script.to_str().expect("utf8 script path").to_string(),
    });
    store.save().expect("persist hook");

    let (manager, _placeholder, _bus) = manager_support::team_manager_for(&team_dir);
    let ctx = tool_context(&workspace, "fb-allow", "tm-001", Some(manager));

    let out = TeamTaskCompleteTool
        .execute(
            json!({"team_name": "fb-allow", "task_id": "task-001"}),
            &ctx,
        )
        .await
        .expect("execute");

    assert!(
        !out.content.contains("did not accept the work"),
        "an allowing hook must not produce a veto sentence: {}",
        out.content
    );
    assert!(
        out.metadata
            .as_ref()
            .and_then(|m| m.get("hook_rejected"))
            .is_none(),
        "no veto key for an allowed completion: {:?}",
        out.metadata
    );
    assert_eq!(
        status_of(&team_dir, "task-001"),
        Some(TaskStatus::Completed)
    );
}

/// A session with no manager still delivers the verdict: the uncached fallback
/// is the same in-list implementation, so FR-018 holds on both paths.
#[cfg(unix)]
#[tokio::test]
async fn store_fallback_also_delivers_hook_feedback() {
    let (_tmp, workspace, team_dir) = harness("fb-store");
    install_vetoing_hook(&team_dir);
    let ctx = tool_context(&workspace, "fb-store", "tm-001", None);

    let out = TeamTaskCompleteTool
        .execute(
            json!({"team_name": "fb-store", "task_id": "task-001"}),
            &ctx,
        )
        .await
        .expect("execute");

    assert!(
        out.content.contains(FEEDBACK),
        "the fallback path must deliver the verdict too, got: {}",
        out.content
    );
    assert_eq!(
        status_of(&team_dir, "task-001"),
        Some(TaskStatus::Completed)
    );
}

/// The lead completing on a teammate's behalf gets the same verdict: FR-018
/// says "regardless of which path", and the lead's own session is one of them.
#[cfg(unix)]
#[tokio::test]
async fn lead_side_completion_delivers_hook_feedback() {
    let (_tmp, workspace, team_dir) = harness("fb-lead");
    install_vetoing_hook(&team_dir);
    let (manager, _placeholder, _bus) = manager_support::team_manager_for(&team_dir);
    let ctx = tool_context(&workspace, "fb-lead", "lead", Some(manager));

    let out = TeamTaskCompleteTool
        .execute(json!({"team_name": "fb-lead", "task_id": "task-001"}), &ctx)
        .await
        .expect("execute");

    assert!(
        out.content.contains(FEEDBACK),
        "a lead-side completion must deliver the verdict, got: {}",
        out.content
    );
    assert_eq!(meta_string(&out, "completed_by").as_deref(), Some("lead"));
}

// -- FR-026: one completion path ----------------------------------------------

/// The tool must reach the manager's one completion implementation, not a task
/// store of its own: a manager bound to a *different* team refuses the call.
///
/// This is the observable difference FR-026 buys. A tool that completed through
/// its own `TaskStore` would succeed here and silently skip the runtime's
/// ownership rule, event and cache refresh.
#[cfg(unix)]
#[tokio::test]
async fn a_foreign_manager_refuses_the_completion() {
    let (_tmp, workspace, team_dir) = harness("fb-foreign");
    let (_tmp2, workspace2, team_dir2) = harness("fb-other");
    let (foreign, _placeholder, _bus) = manager_support::team_manager_for(&team_dir2);
    let ctx = tool_context(&workspace, "fb-foreign", "tm-001", Some(foreign));

    let out = TeamTaskCompleteTool
        .execute(
            json!({"team_name": "fb-foreign", "task_id": "task-001"}),
            &ctx,
        )
        .await
        .expect("a refusal is reported as a tool result, not an error");

    let err = meta_string(&out, "error").unwrap_or_default();
    assert!(
        err.contains("bound to team"),
        "the refusal must name the binding, got: {err}"
    );
    assert_eq!(
        status_of(&team_dir, "task-001"),
        Some(TaskStatus::InProgress),
        "a refused completion leaves this team's task list untouched"
    );
    // Keep the second fixture's workspace alive until the assertion above.
    drop(workspace2);
}

// -- the description states the verdict contract ------------------------------

/// FR-042: the tool description must tell the agent that a veto arrives in the
/// result and that the completion still stands, or an agent will retry a
/// terminal transition forever.
#[test]
fn the_description_states_the_hook_verdict_contract() {
    let description = TeamTaskCompleteTool.description();
    for needle in [
        "HOOK VERDICT",
        "hook_rejected",
        "feedback",
        "terminal",
        "completion stands",
    ] {
        assert!(
            description.contains(needle),
            "the description must mention {needle:?}: {description}"
        );
    }
}

// -- the trait's default arm ---------------------------------------------------

/// A manager that does not implement completion says so rather than silently
/// completing nothing.
#[tokio::test]
async fn the_trait_default_refuses_rather_than_completing() {
    struct NoCompletion;
    #[async_trait::async_trait]
    impl TeamManagerInterface for NoCompletion {
        async fn spawn_teammate(
            &self,
            _team_name: &str,
            _teammate_name: &str,
            _agent_type: &str,
            _prompt: &str,
            _teammate_model: Option<&ragent_agent::agent::ModelRef>,
            _lead_model: Option<&ragent_agent::agent::ModelRef>,
            _working_dir: &Path,
        ) -> anyhow::Result<String> {
            anyhow::bail!("not implemented")
        }

        async fn shutdown_teammate(&self, _agent_id: &str, _graceful: bool) -> anyhow::Result<()> {
            anyhow::bail!("not implemented")
        }

        fn lead_session_id(&self) -> Option<&str> {
            None
        }

        fn is_plan_pending(&self, _agent_id: &str) -> bool {
            false
        }
    }

    let err = NoCompletion
        .complete_task_in(Path::new("/nonexistent"), "task-001", "tm-001")
        .await
        .expect_err("the default arm must refuse");
    assert!(
        err.to_string().contains("no completion path"),
        "the refusal must name the missing path, got: {err}"
    );
}

// -- the member's current task is cleared on the runtime path ------------------

/// Completing through the manager still clears the member's `current_task_id`:
/// the bookkeeping stayed in the tool, and routing the completion through the
/// runtime must not drop it.
#[cfg(unix)]
#[tokio::test]
async fn completion_clears_the_members_current_task() {
    let (_tmp, workspace, team_dir) = harness("fb-clear");
    let mut store = TeamStore::load(&team_dir).expect("load team store");
    let mut member = ragent_agent::team::TeamMember::new("alice", "tm-001", "general");
    member.status = MemberStatus::Working;
    member.current_task_id = Some("task-001".to_string());
    store.add_member(member).expect("add member");
    store.save().expect("save team");

    let (manager, _placeholder, _bus) = manager_support::team_manager_for(&team_dir);
    let ctx = tool_context(&workspace, "fb-clear", "tm-001", Some(manager));

    TeamTaskCompleteTool
        .execute(
            json!({"team_name": "fb-clear", "task_id": "task-001"}),
            &ctx,
        )
        .await
        .expect("execute");

    let store = TeamStore::load(&team_dir).expect("reload team store");
    assert_eq!(
        store
            .config
            .member_by_id("tm-001")
            .and_then(|m| m.current_task_id.clone()),
        None,
        "the completing member must no longer hold the task"
    );
}

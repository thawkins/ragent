//! Deep coverage for `src/app/event_handler.rs`, `src/app/swarm.rs` and
//! `src/app/research.rs` branches the existing suites do not reach.
//!
//! The event-handler arms are driven through the PUBLIC
//! `App::handle_event` / `App::handle_key_event` / `App::handle_mouse_event`
//! entry points; the swarm and research dispatch arms through the PUBLIC
//! `App::execute_slash_command`. Nothing in `src/` is modified.
//!
//! Filesystem-touching tests run inside an isolated tempdir project (cwd +
//! `XDG_CONFIG_HOME` redirected by [`slash_support::enter_project`]) so a run
//! leaves the repository byte-identical. Env mutation is `unsafe` in edition
//! 2024 and the workspace denies `unsafe_code`, so this target opts back in
//! explicitly; the isolation harness holds a process-global mutex across the
//! async body, so `clippy::await_holding_lock` is also allowed here.
//!
//! Tests only - no production logic is touched.
#![allow(unsafe_code)]
#![cfg(test)]
#![allow(clippy::await_holding_lock)]

use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use ragent_agent::event::{Event, FinishReason};
use ragent_agent::mcp::{McpServer, McpStatus};
use ragent_agent::message::{Message, MessagePart, Role};
use ragent_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use ragent_agent::team::{Mailbox, MemberStatus, SwarmState, TeamConfig, TeamMember, TeamStore};
use ragent_tui::App;
use ragent_tui::app::{BgTaskView, ProviderSetupStep, SpecImplState};

#[path = "slash_support/mod.rs"]
mod slash_support;
#[path = "support/mod.rs"]
mod support;

use slash_support::{app_with_session, enter_project, last_message};

/// An app with a live session, no filesystem isolation.
fn new_app(session: &str) -> App {
    let mut app = support::make_app();
    app.session_id = Some(session.to_string());
    app.needs_redraw = false;
    app
}

/// An app inside a fresh isolated tempdir project with a live session.
#[allow(clippy::type_complexity)]
fn isolated_app() -> (
    std::sync::MutexGuard<'static, ()>,
    slash_support::EnvGuard,
    tempfile::TempDir,
    App,
) {
    let (guard, env, temp) = enter_project(r#"{ "defaultAgent": "general" }"#);
    let app = app_with_session();
    (guard, env, temp, app)
}

/// Concatenated text of every text part in the message window.
fn joined_text(app: &App) -> String {
    app.messages
        .iter()
        .flat_map(|m| m.parts.iter())
        .filter_map(|p| match p {
            MessagePart::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// True when any log-panel entry contains `needle`.
fn logged(app: &App, needle: &str) -> bool {
    app.log_entries.iter().any(|e| e.message.contains(needle))
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn alt_key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::ALT)
}

fn shift_key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::SHIFT)
}

/// Render one frame so the renderer publishes the cursor row maps the window
/// key arms consult.
fn render_window(app: &mut App) {
    let backend = ratatui::backend::TestBackend::new(170, 44);
    let mut terminal = ratatui::Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");
}

fn task_entry(id: &str, status: TaskStatus) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: "s1".to_string(),
        child_session_id: format!("child-{id}"),
        agent_name: "explore".to_string(),
        task_prompt: "do work".to_string(),
        background: true,
        detached: false,
        status,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ReportStatus::default(),
    }
}

fn member(name: &str, agent_id: &str) -> TeamMember {
    TeamMember::new(name, agent_id, "general")
}

fn mcp_server(id: &str, status: McpStatus) -> McpServer {
    McpServer {
        id: id.to_string(),
        config: ragent_agent::McpServerConfig::default(),
        status,
        tools: Vec::new(),
    }
}

// ===========================================================================
// MessageEnd - reverse-create chaining, plan delegation, autopilot
// ===========================================================================

#[tokio::test]
async fn message_end_cancelled_consumes_pending_reverse_create_folder() {
    let mut app = new_app("s1");
    app.pending_reverse_create = Some("newspec".to_string());
    app.pending_reverse_create_folder = Some("/tmp/scaffold".to_string());

    app.handle_event(Event::MessageEnd {
        session_id: "s1".to_string(),
        message_id: "m-cancel".to_string(),
        reason: FinishReason::Cancelled,
    })
    .await;

    assert_eq!(app.pending_reverse_create, None);
    assert_eq!(app.pending_reverse_create_folder, None);
    assert!(app.agent_halted, "a cancelled turn halts the agent");
    assert!(
        !joined_text(&app).contains("Generating specification"),
        "a cancelled turn must not chain into /spec create"
    );
}

#[tokio::test]
async fn message_end_reverse_create_with_folder_chains_into_spec_create() {
    let mut app = new_app("s1");
    app.messages.push(Message::new(
        "s1",
        Role::Assistant,
        vec![MessagePart::Text {
            text: "Build a widget service.".to_string(),
        }],
    ));
    app.pending_reverse_create = Some("newspec".to_string());
    app.pending_reverse_create_folder = Some("/tmp/scaffold".to_string());

    app.handle_event(Event::MessageEnd {
        session_id: "s1".to_string(),
        message_id: "m-stop".to_string(),
        reason: FinishReason::Stop,
    })
    .await;

    assert_eq!(app.pending_reverse_create, None);
    assert_eq!(app.pending_reverse_create_folder, None);
    let text = joined_text(&app);
    assert!(
        text.contains("Generating specification"),
        "the chained /spec create should render its create message: {text}"
    );
}

#[tokio::test]
async fn message_end_pending_plan_task_delegates_to_the_plan_agent() {
    let mut app = new_app("s1");
    app.pending_plan_task = Some(("Plan the migration".to_string(), "src/db.rs".to_string()));

    app.handle_event(Event::MessageEnd {
        session_id: "s1".to_string(),
        message_id: "m-plan".to_string(),
        reason: FinishReason::Stop,
    })
    .await;

    assert_eq!(app.agent_name, "plan");
    assert_eq!(app.pending_plan_task, None);
    assert!(
        app.status.contains("agent: plan (delegated from"),
        "got: {}",
        app.status
    );
    let text = joined_text(&app);
    assert!(text.contains("Context:"), "the context is appended: {text}");
    assert_eq!(app.agent_stack.len(), 1, "the previous agent is stacked");
}

#[tokio::test]
async fn message_end_plan_delegation_without_plan_agent_logs_an_error() {
    let mut app = new_app("s1");
    app.cycleable_agents.retain(|a| a.name != "plan");
    app.pending_plan_task = Some(("Plan it".to_string(), String::new()));

    app.handle_event(Event::MessageEnd {
        session_id: "s1".to_string(),
        message_id: "m-plan-missing".to_string(),
        reason: FinishReason::Stop,
    })
    .await;

    assert!(logged(&app, "plan agent not found"), "logs the miss");
    assert_eq!(app.agent_stack.len(), 0, "the pushed agent is popped");
    assert_eq!(app.pending_plan_task, None);
}

#[tokio::test]
async fn message_end_autopilot_schedules_a_continuation() {
    let mut app = new_app("s1");
    app.autopilot_enabled = true;
    app.last_task_completed_at = None;
    app.autopilot_time_limit_secs = None;

    app.handle_event(Event::MessageEnd {
        session_id: "s1".to_string(),
        message_id: "m-auto".to_string(),
        reason: FinishReason::Stop,
    })
    .await;

    assert!(app.autopilot_pending_continue.is_some());
    assert_eq!(app.status, " autopilot");
}

#[tokio::test]
async fn message_end_autopilot_time_limit_stops_the_run() {
    let mut app = new_app("s1");
    app.autopilot_enabled = true;
    app.last_task_completed_at = None;
    app.autopilot_time_limit_secs = Some(0);
    app.autopilot_started_at = Some(
        std::time::Instant::now()
            .checked_sub(Duration::from_secs(5))
            .unwrap_or_else(std::time::Instant::now),
    );

    app.handle_event(Event::MessageEnd {
        session_id: "s1".to_string(),
        message_id: "m-auto-limit".to_string(),
        reason: FinishReason::Stop,
    })
    .await;

    assert!(!app.autopilot_enabled);
    assert_eq!(app.autopilot_started_at, None);
    assert_eq!(app.autopilot_pending_continue, None);
    assert_eq!(app.status, "autopilot: time limit reached");
    assert!(joined_text(&app).contains("Autopilot stopped"));
}

// ===========================================================================
// Event arms the other suites leave untouched
// ===========================================================================

#[tokio::test]
async fn hook_warning_truncates_a_long_stderr_and_arms_expiry() {
    let mut app = new_app("s1");
    let long = "e".repeat(120);
    app.handle_event(Event::HookWarning {
        session_id: "s1".to_string(),
        hook_command: "lint.sh".to_string(),
        tool: "Write".to_string(),
        stderr: long.clone(),
    })
    .await;
    assert_eq!(
        app.status,
        format!("hook warning: Write - {}...", &long[..80]),
        "stderr over 80 chars is truncated"
    );
    assert!(app.status_set_at.is_some(), "the toast arms status expiry");

    let mut short = new_app("s1");
    short
        .handle_event(Event::HookWarning {
            session_id: "s1".to_string(),
            hook_command: "lint.sh".to_string(),
            tool: "read".to_string(),
            stderr: "short".to_string(),
        })
        .await;
    assert_eq!(short.status, "hook warning: read - short");
}

#[tokio::test]
async fn tool_result_flagged_sets_a_warn_status_and_error_log() {
    let mut app = new_app("s1");
    app.handle_event(Event::ToolResultFlagged {
        session_id: "s1".to_string(),
        tool: "Bash".to_string(),
        hook_command: "guard.sh".to_string(),
        reason: "secret detected".to_string(),
    })
    .await;
    assert_eq!(app.status, "[warn] Bash flagged");
    assert!(logged(
        &app,
        "[flag] tool Bash flagged by guard.sh: secret detected"
    ));
}

#[tokio::test]
async fn quota_update_records_the_percentage() {
    let mut app = new_app("s1");
    app.handle_event(Event::QuotaUpdate {
        session_id: "s1".to_string(),
        percent: 55.5,
    })
    .await;
    assert_eq!(app.quota_percent, Some(55.5));
    assert!(logged(&app, "quota: 55.5% used"));
}

#[tokio::test]
async fn router_classification_short_prompt_notes_the_requested_tier() {
    let mut app = new_app("s1");
    app.handle_event(Event::RouterClassification {
        session_id: "s1".to_string(),
        tier: "standard".to_string(),
        requested_tier: Some("simple".to_string()),
        model: "anthropic/claude".to_string(),
        composite_score: 0.25,
        prompt: "short prompt".to_string(),
        dimensions: vec![("length".to_string(), 0.5), ("depth".to_string(), 0.75)],
    })
    .await;
    assert_eq!(app.router_current_tier.as_deref(), Some("standard"));
    assert_eq!(
        app.router_current_model.as_deref(),
        Some("anthropic/claude")
    );
    let entry = app
        .log_entries
        .iter()
        .find(|e| e.message.starts_with("Router: bucket=standard"))
        .map(|e| e.message.clone())
        .unwrap_or_default();
    assert!(entry.contains("length=0.50, depth=0.75"), "dims: {entry}");
    assert!(entry.contains("(requested simple)"), "fallback: {entry}");
    assert!(entry.contains("prompt=\"short prompt\""), "prompt: {entry}");
}

#[tokio::test]
async fn provider_loading_finished_advances_the_setup_dialog() {
    let mut app = new_app("s1");
    app.provider_setup = Some(ProviderSetupStep::LoadingModels {
        provider_id: "ollama".to_string(),
        provider_name: "Ollama".to_string(),
    });

    app.handle_event(Event::ProviderLoadingFinished {
        provider_id: "ollama".to_string(),
        provider_name: "Ollama".to_string(),
        models: Vec::new(),
        error: None,
    })
    .await;

    assert!(app.model_loading_state.is_none());
    match app.provider_setup.as_ref() {
        Some(ProviderSetupStep::SelectModel { provider_id, .. }) => {
            assert_eq!(provider_id, "ollama");
        }
        other => panic!("expected SelectModel, got {other:?}"),
    }
    assert!(logged(&app, "Ollama model discovery finished (0 models)"));
}

#[tokio::test]
async fn model_download_finished_without_error_logs_the_success() {
    let mut app = new_app("s1");
    app.handle_event(Event::ModelDownloadStarted {
        provider_id: "ollama".to_string(),
        model_id: "llama3".to_string(),
        session_id: "s1".to_string(),
    })
    .await;
    assert!(app.model_download_state.is_some());

    app.handle_event(Event::ModelDownloadFinished {
        provider_id: "ollama".to_string(),
        model_id: "llama3".to_string(),
        session_id: "s1".to_string(),
        error: None,
    })
    .await;
    assert!(app.model_download_state.is_none());
    assert!(logged(&app, "Download finished for ollama/llama3"));
}

#[tokio::test]
async fn service_start_error_for_another_session_skips_the_chat_panel() {
    let mut app = new_app("s1");
    app.handle_event(Event::ServiceStartError {
        session_id: "other-session".to_string(),
        service: "ollama serve".to_string(),
        command_path: "/usr/bin/ollama".to_string(),
        stdout: "out".to_string(),
        stderr: "err".to_string(),
        error: "timed out".to_string(),
    })
    .await;
    assert!(
        !joined_text(&app).contains("failed to start"),
        "another session's failure stays out of this chat panel"
    );
    assert!(logged(&app, "ollama serve failed to start"));
    assert!(
        !app.status.contains("failed to start"),
        "status: {}",
        app.status
    );
}

#[tokio::test]
async fn mcp_status_changed_updates_a_registered_server() {
    let mut app = new_app("s1");
    app.mcp_servers = vec![mcp_server("filesystem", McpStatus::Disabled)];

    app.handle_event(Event::McpStatusChanged {
        server_id: "filesystem".to_string(),
        status: "connected".to_string(),
    })
    .await;

    assert_eq!(app.mcp_servers[0].status, McpStatus::Connected);
    assert_eq!(
        app.mcp_status_map.get("filesystem"),
        Some(&McpStatus::Connected)
    );
}

#[tokio::test]
async fn mcp_server_enabled_changed_toggles_the_registered_server() {
    let mut app = new_app("s1");
    app.mcp_servers = vec![mcp_server("filesystem", McpStatus::Disabled)];

    app.handle_event(Event::McpServerEnabledChanged {
        server_id: "filesystem".to_string(),
        enabled: true,
    })
    .await;
    assert_eq!(app.mcp_enabled_map.get("filesystem"), Some(&true));
    assert_eq!(app.mcp_servers[0].status, McpStatus::Connected);

    app.handle_event(Event::McpServerEnabledChanged {
        server_id: "filesystem".to_string(),
        enabled: false,
    })
    .await;
    assert_eq!(app.mcp_enabled_map.get("filesystem"), Some(&false));
    assert_eq!(app.mcp_servers[0].status, McpStatus::Disabled);
}

#[tokio::test]
async fn permission_requested_duplicate_and_other_session_are_ignored() {
    let mut app = new_app("s1");
    for _ in 0..2 {
        app.handle_event(Event::PermissionRequested {
            session_id: "s1".to_string(),
            request_id: "p1".to_string(),
            permission: "file:write".to_string(),
            description: "write a file".to_string(),
            options: vec!["once".to_string()],
        })
        .await;
    }
    assert_eq!(app.permission_queue.len(), 1, "the duplicate is dropped");

    app.handle_event(Event::PermissionRequested {
        session_id: "other".to_string(),
        request_id: "p2".to_string(),
        permission: "file:write".to_string(),
        description: "other session".to_string(),
        options: Vec::new(),
    })
    .await;
    assert_eq!(app.permission_queue.len(), 1, "another session is ignored");
}

#[tokio::test]
async fn question_requested_clamps_an_out_of_range_default_index() {
    let mut app = new_app("s1");
    app.handle_event(Event::QuestionRequested {
        secret: false,
        session_id: "s1".to_string(),
        request_id: "q1".to_string(),
        question: "Pick one".to_string(),
        options: vec!["a".to_string(), "b".to_string()],
        default_index: Some(99),
    })
    .await;
    assert_eq!(app.question_queue.len(), 1);
    assert_eq!(app.question_selected_index, 1, "clamped to the last option");

    // A duplicate request id is dropped.
    app.handle_event(Event::QuestionRequested {
        secret: false,
        session_id: "s1".to_string(),
        request_id: "q1".to_string(),
        question: "Pick one".to_string(),
        options: vec!["a".to_string()],
        default_index: None,
    })
    .await;
    assert_eq!(app.question_queue.len(), 1);
}

#[tokio::test]
async fn shell_cwd_changed_and_user_input_update_state() {
    let mut app = new_app("s1");
    app.handle_event(Event::ShellCwdChanged {
        session_id: "s1".to_string(),
        cwd: "/tmp/elsewhere".to_string(),
    })
    .await;
    assert_eq!(app.shell_cwd.as_deref(), Some("/tmp/elsewhere"));

    app.status = "idle".to_string();
    app.handle_event(Event::UserInput {
        session_id: "s1".to_string(),
        request_id: "u1".to_string(),
        response: "typed".to_string(),
    })
    .await;
    assert!(app.status.contains("processing"), "got: {}", app.status);
}

#[tokio::test]
async fn memory_recalled_forgotten_and_searched_mark_the_cache_dirty() {
    let mut app = new_app("s1");
    app.memory_cache_dirty = false;
    app.handle_event(Event::MemoryRecalled {
        session_id: "s1".to_string(),
        query: "rust".to_string(),
        result_count: 1,
    })
    .await;
    assert!(app.memory_cache_dirty);

    app.memory_cache_dirty = false;
    app.handle_event(Event::MemoryForgotten {
        session_id: "s1".to_string(),
        count: 2,
    })
    .await;
    assert!(app.memory_cache_dirty);

    app.memory_cache_dirty = false;
    app.handle_event(Event::MemorySearched {
        session_id: "s1".to_string(),
        query: "rust".to_string(),
        result_count: 0,
        mode: "fts".to_string(),
    })
    .await;
    assert!(app.memory_cache_dirty);
}

#[tokio::test]
async fn open_research_view_populates_the_viewer() {
    let mut app = new_app("s1");
    app.handle_event(Event::OpenResearchView {
        name: "viewable".to_string(),
        path: std::path::PathBuf::from("/tmp/research/viewable/RESEARCH.md"),
        markdown: "# Body".to_string(),
    })
    .await;
    let view = app.research_view.as_ref().expect("viewer is open");
    assert_eq!(view.name, "viewable");
    assert_eq!(view.markdown, "# Body");
    assert_eq!(app.status, "research: viewing viewable");
}

#[tokio::test]
async fn reasoning_delta_appends_to_an_existing_reasoning_part() {
    let mut app = new_app("s1");
    app.messages.push(Message::new(
        "s1",
        Role::Assistant,
        vec![MessagePart::Reasoning {
            text: "first".to_string(),
            signature: None,
        }],
    ));

    app.handle_event(Event::ReasoningDelta {
        session_id: "s1".to_string(),
        text: " second".to_string(),
    })
    .await;

    assert_eq!(app.messages.len(), 1, "the existing message is reused");
    match app.messages[0].parts.last() {
        Some(MessagePart::Reasoning { text, .. }) => assert_eq!(text, "first second"),
        other => panic!("expected a reasoning part, got {other:?}"),
    }
    assert!(app.stream_in_bytes > 0, "the delta is accounted for");
}

#[tokio::test]
async fn reasoning_delta_starts_a_new_message_after_a_user_turn() {
    let mut app = new_app("s1");
    app.messages
        .push(Message::user_text("s1", "explain the design"));

    app.handle_event(Event::ReasoningDelta {
        session_id: "s1".to_string(),
        text: "thinking".to_string(),
    })
    .await;

    assert_eq!(app.messages.len(), 2, "a fresh assistant message is pushed");
    assert_eq!(app.messages[1].role, Role::Assistant);
    match app.messages[1].parts.first() {
        Some(MessagePart::Reasoning { text, .. }) => assert_eq!(text, "thinking"),
        other => panic!("expected a leading reasoning part, got {other:?}"),
    }
}

// ===========================================================================
// /spec impl sequential driver - advance_spec_impl + dispatch_spec_impl_task
// ===========================================================================

/// A `SPEC.md` in the given lifecycle state.
fn spec_md(status: &str) -> String {
    format!(
        "---\nstatus: {status}\n---\n# Sample Spec\n\n## Requirements\n\n\
         FR-001: The system shall always expose the sample behaviour.\n"
    )
}

/// A six-column `PLAN.md` (no Status column, so every task starts `pending`).
const PLAN_MD: &str = "# Plan\n\n## Tasks\n\n\
| ID | Title | Requirement | Effort | Priority | Dependencies |\n\
|---|---|---|---|---|---|\n\
| T-001 | Build the thing | FR-001 | S | Critical | - |\n\
| T-002 | Test the thing | FR-001 | S | High | T-001 |\n";

/// A seven-column `PLAN.md` whose first task is blocked.
const PLAN_MD_BLOCKED: &str = "# Plan\n\n## Tasks\n\n\
| ID | Title | Requirement | Effort | Priority | Status | Dependencies |\n\
|---|---|---|---|---|---|---|\n\
| T-001 | Build the thing | FR-001 | S | Critical | blocked | - |\n\
| T-002 | Test the thing | FR-001 | S | High | pending | T-001 |\n";

/// Write a spec pair under `root` and return the specs root.
fn write_spec(root: &std::path::Path, status: &str, plan: &str) -> std::path::PathBuf {
    let specs_root = root.join("specs");
    let dir = specs_root.join("sample");
    std::fs::create_dir_all(&dir).expect("create spec dir");
    std::fs::write(dir.join("SPEC.md"), spec_md(status)).expect("write SPEC.md");
    std::fs::write(dir.join("PLAN.md"), plan).expect("write PLAN.md");
    specs_root
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn advance_spec_impl_auto_completes_and_dispatches_the_next_task() {
    let temp = tempfile::tempdir().expect("tempdir");
    let specs_root = write_spec(temp.path(), "in_progress", PLAN_MD);
    let runner = ragent_specs::SpecImplRunner::new(
        "sample",
        specs_root.clone(),
        ragent_specs::ImplOptions::default(),
    )
    .await
    .expect("build runner");

    let mut app = new_app("s1");
    app.spec_impl_state = Some(SpecImplState {
        spec_id: "sample".to_string(),
        specs_root: specs_root.clone(),
        task_ids: vec!["T-001".to_string(), "T-002".to_string()],
        current_rank: 1,
        total: 2,
        runner,
    });

    app.handle_event(Event::MessageEnd {
        session_id: "s1".to_string(),
        message_id: "m-spec".to_string(),
        reason: FinishReason::Stop,
    })
    .await;

    assert_eq!(app.status, "spec: implementing sample - task 2/2");
    assert!(app.is_processing, "the next task is dispatched");
    assert_eq!(
        app.spec_impl_state.as_ref().map(|s| s.current_rank),
        Some(2),
        "the rank advances"
    );
    let text = joined_text(&app);
    assert!(
        text.contains("auto-marked task T-001 as completed")
            || logged(&app, "auto-marked task T-001 as completed"),
        "the task is auto-completed"
    );
    let plan =
        std::fs::read_to_string(specs_root.join("sample").join("PLAN.md")).expect("read PLAN.md");
    assert!(plan.contains("T-001"), "plan rewritten: {plan}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn advance_spec_impl_finishes_the_run_and_transitions_the_spec() {
    let temp = tempfile::tempdir().expect("tempdir");
    let specs_root = write_spec(temp.path(), "in_progress", PLAN_MD);
    let runner = ragent_specs::SpecImplRunner::new(
        "sample",
        specs_root.clone(),
        ragent_specs::ImplOptions::default(),
    )
    .await
    .expect("build runner");

    let mut app = new_app("s1");
    app.spec_impl_state = Some(SpecImplState {
        spec_id: "sample".to_string(),
        specs_root: specs_root.clone(),
        task_ids: vec!["T-001".to_string(), "T-002".to_string()],
        current_rank: 2,
        total: 2,
        runner,
    });

    app.handle_event(Event::MessageEnd {
        session_id: "s1".to_string(),
        message_id: "m-spec-done".to_string(),
        reason: FinishReason::Stop,
    })
    .await;

    assert!(app.spec_impl_state.is_none(), "the run stops");
    assert_eq!(app.status, "spec: sample implemented");
    assert!(joined_text(&app).contains("All 2 tasks completed"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn advance_spec_impl_with_an_invalid_spec_id_stops_the_run() {
    let temp = tempfile::tempdir().expect("tempdir");
    let specs_root = write_spec(temp.path(), "in_progress", PLAN_MD);
    let runner = ragent_specs::SpecImplRunner::new(
        "sample",
        specs_root.clone(),
        ragent_specs::ImplOptions::default(),
    )
    .await
    .expect("build runner");

    let mut app = new_app("s1");
    app.spec_impl_state = Some(SpecImplState {
        spec_id: "Bad Name".to_string(),
        specs_root,
        task_ids: vec!["T-001".to_string()],
        current_rank: 1,
        total: 1,
        runner,
    });

    app.handle_event(Event::MessageEnd {
        session_id: "s1".to_string(),
        message_id: "m-spec-invalid".to_string(),
        reason: FinishReason::Stop,
    })
    .await;

    assert!(app.spec_impl_state.is_none());
    assert!(joined_text(&app).contains("[warn] Invalid spec ID `Bad Name`"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn advance_spec_impl_with_a_blocked_task_stops_the_run() {
    let temp = tempfile::tempdir().expect("tempdir");
    let specs_root = write_spec(temp.path(), "in_progress", PLAN_MD_BLOCKED);
    let runner = ragent_specs::SpecImplRunner::new(
        "sample",
        specs_root.clone(),
        ragent_specs::ImplOptions::default(),
    )
    .await
    .expect("build runner");

    let mut app = new_app("s1");
    app.spec_impl_state = Some(SpecImplState {
        spec_id: "sample".to_string(),
        specs_root,
        task_ids: vec!["T-001".to_string(), "T-002".to_string()],
        current_rank: 1,
        total: 2,
        runner,
    });

    app.handle_event(Event::MessageEnd {
        session_id: "s1".to_string(),
        message_id: "m-spec-blocked".to_string(),
        reason: FinishReason::Stop,
    })
    .await;

    assert!(app.spec_impl_state.is_none());
    let text = joined_text(&app);
    assert!(text.contains("[stop] Task **T-001**"), "got: {text}");
    assert!(!app.is_processing);
}

// ===========================================================================
// Teammate focus cycling / messaging (keyboard driven)
// ===========================================================================

#[tokio::test]
async fn alt_down_and_alt_up_cycle_teammate_focus() {
    let mut app = new_app("s1");
    app.team_members = vec![member("swarm-s1", "tm-001"), member("swarm-s2", "tm-002")];
    app.active_team = Some(TeamConfig::new("team-a", "s1"));

    app.handle_key_event(alt_key(KeyCode::Down)).await;
    assert_eq!(app.focused_teammate.as_deref(), Some("tm-001"));
    assert_eq!(app.status, "team: focused -> swarm-s1");

    app.handle_key_event(alt_key(KeyCode::Down)).await;
    assert_eq!(app.focused_teammate.as_deref(), Some("tm-002"));

    // Forward from the last member clears the focus.
    app.handle_key_event(alt_key(KeyCode::Down)).await;
    assert_eq!(app.focused_teammate, None);
    assert!(app.output_view.is_none());
    assert_eq!(app.status, "team: focus cleared");

    // Backward from "no focus" lands on the last member.
    app.handle_key_event(alt_key(KeyCode::Up)).await;
    assert_eq!(app.focused_teammate.as_deref(), Some("tm-002"));

    app.handle_key_event(alt_key(KeyCode::Up)).await;
    assert_eq!(app.focused_teammate.as_deref(), Some("tm-001"));

    // Backward from the first member clears the focus again.
    app.handle_key_event(alt_key(KeyCode::Up)).await;
    assert_eq!(app.focused_teammate, None);
}

#[tokio::test]
async fn alt_down_without_members_clears_the_focus() {
    let mut app = new_app("s1");
    app.team_members.clear();
    app.focused_teammate = Some("tm-999".to_string());

    app.handle_key_event(alt_key(KeyCode::Down)).await;
    assert_eq!(app.focused_teammate, None);
}

#[tokio::test]
async fn team_focus_by_name_resolves_ambiguity_and_misses() {
    let mut app = new_app("s1");
    app.active_team = Some(TeamConfig::new("team-a", "s1"));
    app.team_members = vec![member("swarm-s1", "tm-001"), member("swarm-s2", "tm-002")];

    app.execute_slash_command("/team focus swarm-s1").await;
    assert_eq!(app.focused_teammate.as_deref(), Some("tm-001"));
    assert!(joined_text(&app).contains("Focused on **swarm-s1**"));

    app.execute_slash_command("/team focus swarm").await;
    assert!(app.status.contains("Ambiguous"), "got: {}", app.status);

    app.execute_slash_command("/team focus nope").await;
    assert!(
        app.status.contains("No teammate matching"),
        "got: {}",
        app.status
    );
    assert!(joined_text(&app).contains("Available teammates"));
}

#[tokio::test]
async fn enter_with_a_focused_teammate_routes_the_message_to_its_mailbox() {
    let (_g, _e, temp, mut app) = isolated_app();
    TeamStore::create("team-a", "s1", temp.path(), true).expect("create team");

    app.active_team = Some(TeamConfig::new("team-a", "s1"));
    app.team_members = vec![member("swarm-s1", "tm-001")];
    app.focused_teammate = Some("tm-001".to_string());
    app.input = "please review the diff".to_string();
    app.input_cursor = app.input.chars().count();

    app.handle_key_event(key(KeyCode::Enter)).await;

    // FR-058: the focused-input path routes the message to the teammate's
    // mailbox as the authenticated lead session
    // (SEC-ragent-team-003), reports success, and clears the input.
    assert_eq!(app.status, "message sent to swarm-s1");
    assert_eq!(app.input, "", "the input buffer is cleared");
    let store = TeamStore::load_by_name("team-a", temp.path()).expect("load team");
    let mail = Mailbox::open(&store.dir, "tm-001")
        .and_then(|mb| mb.read_all())
        .expect("read mailbox");
    assert_eq!(mail.len(), 1, "exactly one routed message");
    assert_eq!(mail[0].from, "lead");
    assert_eq!(mail[0].to, "tm-001");
    assert_eq!(mail[0].content, "please review the diff");
    assert_eq!(
        mail[0].sender_session_id.as_deref(),
        Some("slash-support"),
        "the lead-origin message must carry the authenticated sender session id"
    );
}

#[tokio::test]
async fn focused_message_without_a_team_or_member_reports_the_reason() {
    let mut app = new_app("s1");
    app.active_team = None;
    app.team_members = vec![member("swarm-s1", "tm-001")];
    app.focused_teammate = Some("tm-001".to_string());
    app.input = "hello".to_string();
    app.input_cursor = 5;
    app.handle_key_event(key(KeyCode::Enter)).await;
    assert_eq!(app.status, "No active team");

    // A focus id with no matching member falls through to the ordinary send
    // path, which refuses without a configured provider/model.
    let mut app = new_app("s1");
    app.active_team = Some(TeamConfig::new("team-a", "s1"));
    app.team_members = vec![member("swarm-s1", "tm-001")];
    app.focused_teammate = Some("tm-404".to_string());
    app.input = "hello".to_string();
    app.input_cursor = 5;
    app.handle_key_event(key(KeyCode::Enter)).await;
    assert!(
        app.status.contains("No provider configured"),
        "got: {}",
        app.status
    );
}

#[tokio::test]
async fn focused_message_for_a_missing_team_store_reports_the_load_failure() {
    let (_g, _e, _temp, mut app) = isolated_app();
    app.active_team = Some(TeamConfig::new("ghost-team", "s1"));
    app.team_members = vec![member("swarm-s1", "tm-001")];
    app.focused_teammate = Some("tm-001".to_string());
    app.input = "hello".to_string();
    app.input_cursor = 5;

    app.handle_key_event(key(KeyCode::Enter)).await;

    assert!(
        app.status.starts_with("Failed to load team"),
        "got: {}",
        app.status
    );
}

// ===========================================================================
// Agents panel keyboard + mouse task control
// ===========================================================================

#[tokio::test]
async fn space_on_a_running_task_suspends_and_on_a_suspended_task_resumes() {
    let mut app = new_app("s1");
    app.show_agents_window = true;
    app.active_tasks = vec![
        task_entry("t-running", TaskStatus::Running),
        task_entry("t-suspended", TaskStatus::Suspended),
    ];

    // Cursor ordinal 0 = t-running.
    app.move_agents_cursor(1);
    app.handle_key_event(key(KeyCode::Char(' '))).await;
    assert_eq!(app.active_tasks.len(), 2, "suspend is fire-and-forget");

    // Cursor ordinal 1 = t-suspended.
    app.move_agents_cursor(1);
    app.handle_key_event(key(KeyCode::Char(' '))).await;
    assert!(app.show_agents_window, "the panel stays open");
    assert_eq!(app.active_tasks.len(), 2);
}

#[tokio::test]
async fn shift_k_on_a_task_row_dispatches_a_kill() {
    let mut app = new_app("s1");
    app.show_agents_window = true;
    app.active_tasks = vec![task_entry("t-kill", TaskStatus::Running)];
    render_window(&mut app);
    app.move_agents_cursor(1);

    app.handle_key_event(shift_key(KeyCode::Char('K'))).await;
    assert_eq!(app.active_tasks.len(), 1, "the local row is untouched");
    app.row_action_confirm_cancel();
    assert!(!app.row_action_confirm_open(), "Esc dismisses the dialog");

    // With the rows gone the cursor is stale: Del/Shift+K dispatch nothing.
    app.active_tasks.clear();
    app.needs_redraw = false;
    app.status = "sentinel".to_string();
    app.handle_key_event(shift_key(KeyCode::Char('K'))).await;
    assert_eq!(app.agents_cursor_subagent, Some(0));
    assert_eq!(app.active_tasks.len(), 0);
    assert!(
        !app.row_action_confirm_open(),
        "a stale range arms no dialog"
    );
    assert_eq!(
        app.status, "agents: selected row is gone",
        "a stale cursor reports the consumed no-op (FR-069)"
    );
}

#[tokio::test]
async fn agents_panel_background_task_survives_the_key_surface() {
    // The row-mouse model is retired: the keyboard cursor never covers the
    // background-shell rows (they are service-terminated, not cursorable), so
    // a background task stays untouched by the Agents-window keys (FR-016's
    // key-surface re-expression).
    let mut app = new_app("s1");
    app.show_agents_window = true;
    app.active_tasks = vec![task_entry("t-work", TaskStatus::Running)];
    app.bg_tasks = vec![BgTaskView {
        id: "bg-1".to_string(),
        session_id: "s1".to_string(),
        command: "sleep 1".to_string(),
        status: "running".to_string(),
        created_at: chrono::Utc::now(),
        completed_at: None,
    }];
    render_window(&mut app);

    // Move the cursor down onto the sub-agent row; the background task is not
    // part of the cursor range.
    app.move_agents_cursor(1);
    assert_eq!(app.agents_cursor_subagent, Some(0));
    assert_eq!(
        app.agents_subagent_task_ids.first().map(String::as_str),
        Some("t-work"),
        "the cursor range names sub-agent rows only (FR-016)"
    );
    assert_eq!(app.bg_tasks.len(), 1, "no cancellation was dispatched");
    assert_eq!(app.bg_tasks[0].status, "running");
}

// ===========================================================================
// /swarm - status + cancel variants with a real task store
// ===========================================================================

#[tokio::test]
async fn swarm_status_with_a_store_reports_cancelled_tasks() {
    use ragent_agent::team::{SwarmDecomposition, SwarmSubtask, TaskStore, task::Task};

    let (_g, _e, temp, mut app) = isolated_app();
    let store = TeamStore::create("swarm-store", "s1", temp.path(), true).expect("create team");
    let ts = TaskStore::open(&store.dir).expect("task store");
    let mut cancelled = Task::new("s1", "Cancelled task");
    cancelled.status = ragent_agent::team::TaskStatus::Cancelled;
    ts.add_task(cancelled).expect("add task");

    app.swarm_state = Some(SwarmState {
        team_name: "swarm-store".to_string(),
        prompt: "go".to_string(),
        decomposition: SwarmDecomposition {
            tasks: vec![SwarmSubtask {
                id: "s1".to_string(),
                title: "Cancelled task".to_string(),
                description: "d".to_string(),
                depends_on: Vec::new(),
                agent_type: None,
                model: None,
            }],
        },
        spawned: true,
        completed: false,
        default_agent_type: None,
    });
    app.team_members = vec![TeamMember {
        status: MemberStatus::Idle,
        ..member("swarm-s1", "tm-001")
    }];

    app.execute_slash_command("/swarm status").await;
    let text = last_message(&app);
    assert!(text.contains("Cancelled task"), "got: {text}");
    assert!(text.contains("[err]"), "got: {text}");
    assert!(text.contains("0/1"), "got: {text}");
    assert!(text.contains("idle"), "member status rendered: {text}");
}

#[tokio::test]
async fn swarm_cancel_after_completion_clears_the_state() {
    let (_g, _e, temp, mut app) = isolated_app();
    TeamStore::create("swarm-done", "s1", temp.path(), true).expect("create team");
    app.swarm_state = Some(SwarmState {
        team_name: "swarm-done".to_string(),
        prompt: "go".to_string(),
        decomposition: ragent_agent::team::SwarmDecomposition { tasks: Vec::new() },
        spawned: true,
        completed: true,
        default_agent_type: Some("general".to_string()),
    });

    app.execute_slash_command("/swarm cancel").await;
    assert!(app.swarm_state.is_none());
    assert_eq!(app.status, "swarm: cancelled");
}

// ===========================================================================
// /research dispatch shapes
// ===========================================================================

#[tokio::test]
async fn research_help_renders_the_usage_block() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/research help").await;
    assert_eq!(app.status, "research: help");
    assert!(last_message(&app).to_lowercase().contains("research"));
}

#[tokio::test]
async fn research_bare_and_unknown_subcommands_report_errors() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/research").await;
    assert!(
        last_message(&app).contains("unknown subcommand"),
        "got: {}",
        last_message(&app)
    );

    app.execute_slash_command("/research status").await;
    assert!(
        last_message(&app).contains("unknown subcommand `status`"),
        "got: {}",
        last_message(&app)
    );
}

#[tokio::test]
async fn research_create_with_every_run_flag_seeds_progress() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command(
        "/research create flagged a concrete 2024 rust async runtime comparison \
         --max-concepts 3 --max-findings 4 --papers --no-oa --url-cloak --depth deep",
    )
    .await;
    assert_eq!(app.status, "[wait] research: flagged...");
    assert!(
        app.research_progress.iter().any(|p| p.name == "flagged"),
        "a progress tracker is seeded"
    );
}

#[tokio::test]
async fn research_create_with_a_bad_integer_flag_is_rejected() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/research create bad topic --max-concepts xyz")
        .await;
    let text = last_message(&app);
    // The diagnostic is rendered inside the message window, where the markdown
    // pass may close the inline code span, so assert the flag/value and the
    // reason separately rather than as one contiguous span.
    assert!(
        text.contains("invalid value for `--max-concepts xyz"),
        "got: {text}"
    );
    assert!(text.contains("expected an integer"), "got: {text}");
}

#[tokio::test]
async fn research_create_with_a_bad_age_span_is_rejected() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/research create fresh topic --age 30w")
        .await;
    let text = last_message(&app);
    assert!(text.contains("invalid value for `--age 30w"), "got: {text}");
    assert!(text.contains("unknown unit `w`"), "got: {text}");
}

#[tokio::test]
async fn research_create_without_a_name_reports_the_unknown_verb() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/research create --depth deep")
        .await;
    assert!(
        last_message(&app).contains("unknown subcommand `create`"),
        "got: {}",
        last_message(&app)
    );
}

#[tokio::test]
async fn research_delete_without_confirmation_is_refused() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/research delete topic-a").await;
    let text = last_message(&app);
    assert!(
        text.contains("Refusing to delete research/topic-a"),
        "got: {text}"
    );
    assert!(text.contains("--yes"), "got: {text}");
}

#[tokio::test]
async fn research_continue_and_show_and_archive_reach_their_arms() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/research continue topic-a --message go-deeper")
        .await;
    assert!(
        last_message(&app).contains("with follow-up: go-deeper"),
        "got: {}",
        last_message(&app)
    );

    app.execute_slash_command("/research show topic-a").await;
    assert!(app.status.contains("showing"), "got: {}", app.status);

    app.execute_slash_command("/research archive topic-a").await;
    assert!(app.status.contains("archiving"), "got: {}", app.status);

    app.execute_slash_command("/research list --all").await;
    assert!(
        app.status.contains("including archived"),
        "got: {}",
        app.status
    );
}

/// Wait for the first matching event on `rx`, giving the spawned task a bounded
/// window to reach it.
async fn wait_for_notice(
    rx: &mut tokio::sync::broadcast::Receiver<Event>,
    needle: &str,
) -> Option<String> {
    for _ in 0..100 {
        match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
            Ok(Ok(Event::AgentNotice { message, .. })) if message.contains(needle) => {
                return Some(message);
            }
            Ok(Ok(_)) | Ok(Err(_)) => {}
            Err(_) => continue,
        }
    }
    None
}

#[tokio::test]
async fn research_clarify_question_dismissal_cancels_the_run() {
    let (_g, _e, _t, mut app) = isolated_app();
    let bus = app.event_bus.clone();
    let mut rx = app.event_bus.clone().subscribe();

    app.execute_slash_command("/research create clarified rust --clarify")
        .await;

    // The clarification prompt arrives as a `QuestionRequested`; dismiss it so
    // the retry never reaches the network.
    let mut asked = false;
    for _ in 0..100 {
        match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
            Ok(Ok(Event::QuestionRequested {
                secret: false,
                session_id,
                request_id,
                ..
            })) => {
                bus.publish(Event::QuestionAnswered {
                    session_id,
                    request_id,
                    response: "[User dismissed question]".to_string(),
                    secret: false,
                });
                asked = true;
                break;
            }
            Ok(Ok(_)) | Ok(Err(_)) => {}
            Err(_) => continue,
        }
    }
    assert!(asked, "the clarification question is asked");

    let notice = wait_for_notice(&mut rx, "clarification cancelled").await;
    assert!(
        notice.is_some(),
        "a dismissed clarification cancels the run"
    );
}

#[tokio::test]
async fn research_cluster_with_a_model_spawns_the_extraction_and_reports_failure() {
    let (_g, _e, temp, mut app) = isolated_app();
    let sources = temp.path().join("research").join("kilo").join("sources");
    std::fs::create_dir_all(&sources).expect("create sources dir");
    std::fs::write(sources.join("web-01.md"), "body text").expect("write source");

    app.selected_model = Some("gemini/gemini-2.0-flash".to_string());
    app.selected_model_ctx_window = Some(1_048_576);
    let mut rx = app.event_bus.clone().subscribe();

    app.execute_slash_command("/research cluster kilo").await;
    assert!(
        app.status.contains("reading sources"),
        "got: {}",
        app.status
    );

    let notice = wait_for_notice(&mut rx, "cluster 'kilo' failed").await;
    assert!(
        notice.is_some(),
        "the extraction reports its failure to the session"
    );
}

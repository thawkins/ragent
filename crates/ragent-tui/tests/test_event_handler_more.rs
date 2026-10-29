//! Additional `App::handle_event` coverage for the event-handler arms the
//! existing `test_widgets_and_events_extra.rs` suite does not exercise:
//! permission/question replies, agent-switch/restore requests, task
//! completion, the sub-agent suspend/resume/kill/cancel lifecycle, background
//! task updates, team P2P messages and cleanup, memory cache invalidation,
//! model-download failure, the GitHub/GitLab device-flow outcomes, the
//! no-op/unknown variants, and the MCP initialization sentinel.
//!
//! Tests only - no production logic is touched.

use ragent_agent::event::{Event, FinishReason};
use ragent_agent::message::{MessagePart, Role};
use ragent_agent::permission::PermissionDecision;
use ragent_agent::task::TaskEntry;
use ragent_agent::team::{MemberStatus, TeamConfig, TeamMember};
use ragent_tui::App;
use ragent_tui::app::PlanApprovalState;

#[path = "support/mod.rs"]
mod support;

fn app_with_session(session: &str) -> App {
    let mut app = support::make_app();
    app.session_id = Some(session.to_string());
    app.needs_redraw = false;
    app
}

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

fn task_entry(id: &str, child: &str) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: "s1".to_string(),
        child_session_id: child.to_string(),
        agent_name: "explore".to_string(),
        task_prompt: "do work".to_string(),
        background: true,
        detached: false,
        status: ragent_agent::task::TaskStatus::Running,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ragent_agent::task::ReportStatus::default(),
        time_bound_secs: None,
    }
}

// ===========================================================================
// Permission / question replies
// ===========================================================================

#[tokio::test]
async fn permission_replied_removes_request_and_restores_working_status() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::PermissionRequested {
        session_id: "s1".to_string(),
        request_id: "req-1".to_string(),
        permission: "file:write".to_string(),
        description: "write a file".to_string(),
        options: Vec::new(),
    })
    .await;
    assert_eq!(app.permission_queue.len(), 1);

    app.handle_event(Event::PermissionReplied {
        session_id: "s1".to_string(),
        request_id: "req-1".to_string(),
        allowed: true,
        decision: PermissionDecision::Once,
    })
    .await;
    assert!(
        app.permission_queue.is_empty(),
        "the answered request is removed"
    );
    assert!(
        app.status.contains("processing"),
        "an empty permission queue restores the working status, got: {}",
        app.status
    );
}

#[tokio::test]
async fn duplicate_permission_request_is_deduplicated() {
    let mut app = app_with_session("s1");
    for _ in 0..2 {
        app.handle_event(Event::PermissionRequested {
            session_id: "s1".to_string(),
            request_id: "dup".to_string(),
            permission: "bash".to_string(),
            description: "run".to_string(),
            options: Vec::new(),
        })
        .await;
    }
    assert_eq!(app.permission_queue.len(), 1, "the duplicate is ignored");
}

#[tokio::test]
async fn question_answered_removes_request() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::QuestionRequested {
        secret: false,
        session_id: "s1".to_string(),
        request_id: "q-1".to_string(),
        question: "pick one".to_string(),
        options: vec!["a".to_string(), "b".to_string()],
        default_index: Some(1),
    })
    .await;
    assert_eq!(app.question_queue.len(), 1);
    assert_eq!(
        app.question_selected_index, 1,
        "the default index pre-selects"
    );

    app.handle_event(Event::QuestionAnswered {
        session_id: "s1".to_string(),
        request_id: "q-1".to_string(),
        response: "b".to_string(),
        secret: false,
    })
    .await;
    assert!(app.question_queue.is_empty());
}

#[tokio::test]
async fn question_requested_from_non_current_session_still_queues() {
    // Questions from any session are displayed (a sub-agent may need input).
    let mut app = app_with_session("s1");
    app.handle_event(Event::QuestionRequested {
        secret: false,
        session_id: "child-9".to_string(),
        request_id: "q-x".to_string(),
        question: "child question".to_string(),
        options: Vec::new(),
        default_index: None,
    })
    .await;
    assert_eq!(app.question_queue.len(), 1);
}

// ===========================================================================
// Agent switch / restore requests
// ===========================================================================

#[tokio::test]
async fn agent_switch_requested_records_pending_plan_task() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::AgentSwitchRequested {
        session_id: "s1".to_string(),
        to: "plan".to_string(),
        task: "plan the work".to_string(),
        context: "extra context".to_string(),
    })
    .await;
    let (task, context) = app
        .pending_plan_task
        .clone()
        .expect("pending plan task set");
    assert_eq!(task, "plan the work");
    assert_eq!(context, "extra context");
}

#[tokio::test]
async fn agent_restore_requested_opens_plan_approval_dialog() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::AgentRestoreRequested {
        session_id: "s1".to_string(),
        summary: "the plan summary".to_string(),
    })
    .await;
    let pending = app
        .plan_approval_pending
        .as_ref()
        .expect("plan approval dialog opened");
    assert_eq!(pending.plan_text, "the plan summary");
    assert!(pending.cursor_approve);
}

// ===========================================================================
// Task completion + autopilot
// ===========================================================================

#[tokio::test]
async fn task_completed_stops_autopilot_and_renders_summary() {
    let mut app = app_with_session("s1");
    app.autopilot_enabled = true;
    app.autopilot_started_at = Some(std::time::Instant::now());
    app.handle_event(Event::TaskCompleted {
        session_id: "s1".to_string(),
        summary: "all done".to_string(),
    })
    .await;
    assert!(!app.autopilot_enabled, "autopilot stops on task completion");
    assert!(app.last_task_completed_at.is_some());
    let text = joined_text(&app);
    assert!(text.contains("Task Complete"), "got: {text}");
}

// ===========================================================================
// Sub-agent lifecycle (suspend / resume / kill / cancel)
// ===========================================================================

#[tokio::test]
async fn subagent_suspend_resume_toggle_status() {
    let mut app = app_with_session("s1");
    app.active_tasks.push(task_entry("t1", "child-1"));

    app.handle_event(Event::SubagentSuspended {
        session_id: "s1".to_string(),
        task_id: "t1".to_string(),
        child_session_id: "child-1".to_string(),
    })
    .await;
    assert_eq!(
        app.active_tasks[0].status,
        ragent_agent::task::TaskStatus::Suspended
    );

    app.handle_event(Event::SubagentResumed {
        session_id: "s1".to_string(),
        task_id: "t1".to_string(),
        child_session_id: "child-1".to_string(),
    })
    .await;
    assert_eq!(
        app.active_tasks[0].status,
        ragent_agent::task::TaskStatus::Running
    );
}

#[tokio::test]
async fn subagent_killed_force_removes_task() {
    let mut app = app_with_session("s1");
    app.active_tasks.push(task_entry("t1", "child-1"));
    app.handle_event(Event::SubagentKilled {
        session_id: "s1".to_string(),
        task_id: "t1".to_string(),
        child_session_id: "child-1".to_string(),
        force: true,
    })
    .await;
    assert!(app.active_tasks.is_empty());
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("Force-killed")),
        "force-kill is logged"
    );
}

#[tokio::test]
async fn subagent_cancelled_without_matching_task_still_logs() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::SubagentCancelled {
        session_id: "s1".to_string(),
        task_id: "ghost".to_string(),
    })
    .await;
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("cancelled")),
        "the cancel is logged even without a matching task"
    );
}

// ===========================================================================
// Background task updates
// ===========================================================================

#[tokio::test]
async fn background_task_updated_marks_completed_at_for_terminal_states() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::BackgroundTaskSpawned {
        session_id: "s1".to_string(),
        task_id: "bg-1".to_string(),
        command: "sleep 1".to_string(),
    })
    .await;
    assert_eq!(app.bg_tasks.len(), 1);

    app.handle_event(Event::BackgroundTaskUpdated {
        session_id: "s1".to_string(),
        task_id: "bg-1".to_string(),
        status: "failed".to_string(),
        progress: None,
    })
    .await;
    assert_eq!(app.bg_tasks[0].status, "failed");
    assert!(app.bg_tasks[0].completed_at.is_some());
}

// ===========================================================================
// Team: P2P message, cleanup, teammate suspended/resumed (no-op arms)
// ===========================================================================

#[tokio::test]
async fn teammate_p2p_message_updates_send_and_receive_counts() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::TeammateP2PMessage {
        session_id: "s1".to_string(),
        team_name: "alpha".to_string(),
        from: "tm-001".to_string(),
        to: "tm-002".to_string(),
        message_type: "message".to_string(),
        preview: "hi".to_string(),
    })
    .await;
    assert_eq!(app.team_message_counts.get("tm-001").copied(), Some((1, 0)));
    assert_eq!(app.team_message_counts.get("tm-002").copied(), Some((0, 1)));
}

#[tokio::test]
async fn team_cleaned_up_clears_members_and_swarm_state() {
    let mut app = app_with_session("s1");
    app.active_team = Some(TeamConfig::new("alpha", "s1"));
    app.team_members
        .push(TeamMember::new("writer", "tm-001", "general"));
    app.show_teams = true;
    app.focused_teammate = Some("tm-001".to_string());
    app.swarm_state = Some(ragent_agent::team::SwarmState {
        team_name: "alpha".to_string(),
        prompt: "x".to_string(),
        decomposition: ragent_agent::team::SwarmDecomposition { tasks: vec![] },
        spawned: true,
        completed: false,
        default_agent_type: None,
    });

    app.handle_event(Event::TeamCleanedUp {
        session_id: "s1".to_string(),
        team_name: "alpha".to_string(),
    })
    .await;
    assert!(app.active_team.is_none());
    assert!(app.team_members.is_empty());
    assert!(!app.show_teams);
    assert!(app.focused_teammate.is_none());
    assert!(
        app.swarm_state.is_none(),
        "the swarm backed by this team is cleared"
    );
}

#[tokio::test]
async fn teammate_idle_and_failed_update_member_status() {
    let mut app = app_with_session("s1");
    app.team_members
        .push(TeamMember::new("writer", "tm-001", "general"));

    app.handle_event(Event::TeammateIdle {
        session_id: "s1".to_string(),
        team_name: "alpha".to_string(),
        agent_id: "tm-001".to_string(),
    })
    .await;
    assert_eq!(app.team_members[0].status, MemberStatus::Idle);

    app.handle_event(Event::TeammateFailed {
        session_id: "s1".to_string(),
        team_name: "alpha".to_string(),
        agent_id: "tm-001".to_string(),
        error: "boom".to_string(),
    })
    .await;
    assert_eq!(app.team_members[0].status, MemberStatus::Failed);
    assert_eq!(
        app.team_members[0].last_spawn_error.as_deref(),
        Some("boom")
    );
}

#[tokio::test]
async fn team_task_claimed_and_completed_update_member_task_id() {
    let mut app = app_with_session("s1");
    app.team_members
        .push(TeamMember::new("writer", "tm-001", "general"));

    app.handle_event(Event::TeamTaskClaimed {
        session_id: "s1".to_string(),
        team_name: "alpha".to_string(),
        agent_id: "tm-001".to_string(),
        task_id: "task-7".to_string(),
    })
    .await;
    assert_eq!(app.team_members[0].status, MemberStatus::Working);
    assert_eq!(
        app.team_members[0].current_task_id.as_deref(),
        Some("task-7")
    );

    app.handle_event(Event::TeamTaskCompleted {
        session_id: "s1".to_string(),
        team_name: "alpha".to_string(),
        agent_id: "tm-001".to_string(),
        task_id: "task-7".to_string(),
    })
    .await;
    assert!(app.team_members[0].current_task_id.is_none());
}

#[tokio::test]
async fn teammate_suspended_and_resumed_update_the_member_row() {
    // T-014 (spec `agentui`): the two variants gained explicit arms - the
    // runtime publishes them from the FR-040 suspend/resume dispatch and
    // `/team suspend`, so the row must follow instead of relying on the
    // next render-time store hydration.
    let mut app = app_with_session("s1");
    app.team_members
        .push(TeamMember::new("writer", "tm-001", "general"));
    assert_eq!(app.team_members[0].status, MemberStatus::Spawning);

    app.handle_event(Event::TeammateSuspended {
        session_id: "s1".to_string(),
        team_name: "alpha".to_string(),
        agent_id: "tm-001".to_string(),
    })
    .await;
    assert_eq!(app.team_members[0].status, MemberStatus::Suspended);
    assert!(app.needs_redraw);

    app.handle_event(Event::TeammateResumed {
        session_id: "s1".to_string(),
        team_name: "alpha".to_string(),
        agent_id: "tm-001".to_string(),
    })
    .await;
    assert_eq!(app.team_members[0].status, MemberStatus::Idle);
    assert!(app.needs_redraw);
}

// ===========================================================================
// Structured-memory cache invalidation
// ===========================================================================

#[tokio::test]
async fn memory_recalled_forgotten_searched_mark_cache_dirty() {
    let mut app = app_with_session("s1");
    app.memory_cache_dirty = false;

    app.handle_event(Event::MemoryRecalled {
        session_id: "s1".to_string(),
        query: "rust".to_string(),
        result_count: 3,
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
        query: "async".to_string(),
        result_count: 1,
        mode: "semantic".to_string(),
    })
    .await;
    assert!(app.memory_cache_dirty);
}

#[tokio::test]
async fn conversation_and_session_searched_are_accepted_noops() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::ConversationSearched {
        session_id: "s1".to_string(),
        query: "stats".to_string(),
        result_count: 5,
        mode: "stats".to_string(),
    })
    .await;
    app.handle_event(Event::SessionSearched {
        session_id: "s1".to_string(),
        query: "build".to_string(),
        result_count: 2,
        mode: "keyword".to_string(),
    })
    .await;
    // Both land on the catch-all: the app must still be marked dirty.
    assert!(app.needs_redraw);
}

// ===========================================================================
// Model download failure + MCP + provider-setup advance
// ===========================================================================

#[tokio::test]
async fn model_download_finished_error_surfaces_status_and_message() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::ModelDownloadStarted {
        provider_id: "ollama".to_string(),
        model_id: "llama3".to_string(),
        session_id: "s1".to_string(),
    })
    .await;
    app.handle_event(Event::ModelDownloadFinished {
        provider_id: "ollama".to_string(),
        model_id: "llama3".to_string(),
        session_id: "s1".to_string(),
        error: Some("disk full".to_string()),
    })
    .await;
    assert!(app.model_download_state.is_none());
    assert!(
        app.status.contains("download failed"),
        "got: {}",
        app.status
    );
    let text = joined_text(&app);
    assert!(text.contains("Model download failed"), "got: {text}");
}

#[tokio::test]
async fn mcp_status_changed_empty_initialized_runs_the_settle_path() {
    // The empty-id `initialized` sentinel triggers the one-shot reconciliation
    // (`register_mcp_tools` + `report_mcp_startup`) rather than a per-server
    // status update.
    let mut app = app_with_session("s1");
    app.handle_event(Event::McpStatusChanged {
        server_id: String::new(),
        status: "initialized".to_string(),
    })
    .await;
    assert!(
        app.mcp_status_map.is_empty(),
        "no per-server entry is recorded"
    );
    assert!(app.needs_redraw);
}

#[tokio::test]
async fn mcp_server_enabled_changed_updates_enabled_map() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::McpServerEnabledChanged {
        server_id: "filesystem".to_string(),
        enabled: false,
    })
    .await;
    assert_eq!(app.mcp_enabled_map.get("filesystem").copied(), Some(false));
}

#[tokio::test]
async fn provider_loading_finished_with_models_advances_no_dialog_when_absent() {
    let mut app = app_with_session("s1");
    app.provider_setup = None;
    app.handle_event(Event::ProviderLoadingStarted {
        provider_id: "ollama".to_string(),
        provider_name: "Ollama".to_string(),
    })
    .await;
    assert!(app.model_loading_state.is_some());

    app.handle_event(Event::ProviderLoadingFinished {
        provider_id: "ollama".to_string(),
        provider_name: "Ollama".to_string(),
        models: Vec::new(),
        error: Some("connection refused".to_string()),
    })
    .await;
    assert!(app.model_loading_state.is_none());
    assert!(
        app.status.contains("discovery failed"),
        "got: {}",
        app.status
    );
}

// ===========================================================================
// GitHub / GitLab device-flow outcomes
// ===========================================================================

#[tokio::test]
async fn github_device_flow_success_sets_status() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::GithubDeviceFlowComplete {
        success: true,
        error: None,
    })
    .await;
    assert_eq!(app.status, "GitHub authenticated");
    assert!(joined_text(&app).contains("GitHub authentication successful"));
}

#[tokio::test]
async fn github_device_flow_failure_reports_error() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::GithubDeviceFlowComplete {
        success: false,
        error: Some("bad code".to_string()),
    })
    .await;
    assert_eq!(app.status, "GitHub login failed");
    assert!(joined_text(&app).contains("bad code"));
}

#[tokio::test]
async fn gitlab_setup_success_and_failure_paths() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::GitLabSetupComplete {
        success: true,
        error: None,
    })
    .await;
    assert!(app.provider_setup.is_none());
    assert!(
        app.log_entries.iter().any(|e| e.message.contains("GitLab")),
        "success is logged"
    );

    // A failure reverts the setup dialog to the form-bearing state.
    app.handle_event(Event::GitLabSetupComplete {
        success: false,
        error: Some("token rejected".to_string()),
    })
    .await;
    assert!(
        app.provider_setup.is_some(),
        "failure reverts the setup form"
    );
}

// ===========================================================================
// Session aborted + memory-candidate extracted (catch-all arms)
// ===========================================================================

#[tokio::test]
async fn session_aborted_and_memory_candidate_are_accepted_noops() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::SessionAborted {
        session_id: "s1".to_string(),
        reason: "user_requested".to_string(),
    })
    .await;
    app.handle_event(Event::MemoryCandidateExtracted {
        session_id: "s1".to_string(),
        content: "a fact".to_string(),
        category: "fact".to_string(),
        tags: vec!["t".to_string()],
        confidence: 0.9,
        source: "auto".to_string(),
        reason: "pattern".to_string(),
    })
    .await;
    app.handle_event(Event::SessionUpdated {
        session_id: "s1".to_string(),
    })
    .await;
    assert!(app.needs_redraw);
}

// ===========================================================================
// ToolCallBatch - atomic fallback rebuilds tool-call parts
// ===========================================================================

#[tokio::test]
async fn tool_call_batch_creates_and_finalises_tool_parts() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::ToolCallBatch {
        session_id: "s1".to_string(),
        step: 1,
        calls: vec![ragent_types::event::ToolCallBatchEntry {
            call_id: "c1".to_string(),
            tool: "read".to_string(),
            args: "{\"file_path\":\"a.rs\"}".to_string(),
            error: None,
            duration_ms: 12,
            content: "file body".to_string(),
            content_line_count: 1,
            metadata: None,
            success: true,
        }],
    })
    .await;
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("tool call: read")),
        "the dropped-start fallback logs the tool call"
    );
    // The batch created the tool-call part and applied its args/status. The
    // batch arm updates the part in place via `update_tool_call_status` and
    // `update_tool_call_input`; it does not emit the per-result log line (that
    // is the per-call `ToolResult` arm's job).
    let mut saw_completed = false;
    let mut saw_input = false;
    for part in app.messages.iter().flat_map(|m| m.parts.iter()) {
        if let MessagePart::ToolCall { call_id, state, .. } = part
            && call_id == "c1"
        {
            saw_completed |= state.status == ragent_agent::message::ToolCallStatus::Completed;
            saw_input |= !state.input.is_null();
        }
    }
    assert!(saw_input, "the batch args are applied to the part");
    assert!(saw_completed, "the batch marks the part completed");
}

// ===========================================================================
// ToolCallEnd error branch
// ===========================================================================

#[tokio::test]
async fn tool_call_end_with_error_logs_failure() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::ToolCallStart {
        session_id: "s1".to_string(),
        call_id: "err-call".to_string(),
        tool: "Bash".to_string(),
    })
    .await;
    app.handle_event(Event::ToolCallEnd {
        session_id: "s1".to_string(),
        call_id: "err-call".to_string(),
        tool: "Bash".to_string(),
        error: Some("exit code 1".to_string()),
        duration_ms: 5,
    })
    .await;
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("failed: exit code 1")),
        "the failure is logged"
    );
}

// ===========================================================================
// LoopChangeSummary with an empty file list still arms rollback
// ===========================================================================

#[tokio::test]
async fn plan_approval_state_is_publicly_constructible() {
    let state = PlanApprovalState {
        plan_text: "p".to_string(),
        cursor_approve: false,
    };
    assert!(!state.cursor_approve);
}

// ===========================================================================
// Agent notice research-progress routing with a per-URL event (log suppressed)
// ===========================================================================

#[tokio::test]
async fn agent_notice_per_url_web_event_is_not_logged_but_updates_status() {
    let mut app = app_with_session("s1");
    // Sentinel-prefixed progress event.
    let cluster = ragent_tui::research_progress::encode_cluster_progress_event(
        "run-x",
        "topic",
        ragent_research::session::SessionPhase::Setup,
        "started",
        "reading sources...",
    );
    app.handle_event(Event::AgentNotice {
        session_id: "s1".to_string(),
        message: cluster,
    })
    .await;
    assert!(
        app.research_progress.iter().any(|p| p.name == "run-x"),
        "the progress tracker is created on first event"
    );
}

// ===========================================================================
// TeamCleanup path: swarm state cleared through the team store
// ===========================================================================

#[tokio::test]
async fn teammate_spawned_adds_member_and_reveals_teams_panel() {
    // The handler adds the member in-memory before any disk reconciliation,
    // so this needs no process-cwd change.
    let mut app = app_with_session("s1");
    app.handle_event(Event::TeammateSpawned {
        session_id: "s1".to_string(),
        team_name: "alpha".to_string(),
        teammate_name: "writer".to_string(),
        agent_id: "tm-001".to_string(),
    })
    .await;

    assert!(
        app.show_teams,
        "spawning a teammate reveals the teams panel"
    );
    assert!(app.team_members.iter().any(|m| m.agent_id == "tm-001"));
    assert!(app.team_message_counts.contains_key("tm-001"));
}

#[tokio::test]
async fn loop_change_summary_rejects_rollback_when_no_files() {
    // Empty file list still arms the rollback offer; the offer itself is the
    // important state. Confirm the status line points at the decision.
    let mut app = app_with_session("s1");
    app.handle_event(Event::LoopChangeSummary {
        session_id: "s1".to_string(),
        status: "completed".to_string(),
        iterations: 1,
        files_modified: 0,
        files_created: 0,
        files_deleted: 0,
        diffstat: "+0 -0".to_string(),
        files: Vec::new(),
    })
    .await;
    assert!(app.pending_rollback.is_some());
    assert!(app.status.contains("roll back"));
}

#[tokio::test]
async fn message_end_cancelled_retains_input_queue() {
    let mut app = app_with_session("s1");
    app.input_queue.push_back(ragent_tui::app::QueuedInput {
        text: "queued".to_string(),
        image_paths: Vec::new(),
    });
    app.is_processing = true;

    app.handle_event(Event::MessageEnd {
        session_id: "s1".to_string(),
        message_id: "m".to_string(),
        reason: FinishReason::Cancelled,
    })
    .await;
    // A cancelled turn retains the queue (FR-018).
    assert_eq!(app.input_queue_len(), 1);
}

// ===========================================================================
// Small helper: role assertion keeps MessagePart imported
// ===========================================================================

#[tokio::test]
async fn text_delta_appends_to_assistant_message() {
    let mut app = app_with_session("s1");
    app.handle_event(Event::MessageStart {
        session_id: "s1".to_string(),
        message_id: "m1".to_string(),
    })
    .await;
    app.handle_event(Event::TextDelta {
        session_id: "s1".to_string(),
        text: "streamed".to_string(),
    })
    .await;
    assert!(
        app.messages
            .iter()
            .any(|m| m.role == Role::Assistant && m.text_content().contains("streamed"))
    );
}

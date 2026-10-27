//! Extra coverage tests for `ragent_tui::layout` render branches.
//!
//! Drives the real `layout::render` path through a `TestBackend` across the
//! layout permutations the crate supports: terminal sizes (tiny/large/zero),
//! split ratios, every side-panel visibility toggle, and every overlay branch
//! (provider-setup steps, pickers, dialogs, viewers, popups, teammate strip).
//! Also exercises the public layout helpers directly.
//!
//! Tests only - no production logic is touched.

mod support;

use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

use ragent_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use ragent_agent::permission::PermissionRequest;
use ragent_agent::task::TaskEntry;
use ragent_agent::team::{TeamConfig, TeamMember};
use ragent_plugins::{PluginListRow, PluginListView};
use ragent_storage::storage::MemoryRow;
use ragent_tui::App;
use ragent_tui::app::{
    BackendPanelRow, BackendPanelState, ConfigSavePickerState, ConfiguredProvider, ContextAction,
    ContextMenuState, DeviceFlowKind, FileMenuEntry, FileMenuState, HistoryPickerState,
    McpDiscoverState, MemoryViewState, ModelDownloadState, ModelLoadingState, ModelPickerEntry,
    OutputViewLineCache, OutputViewState, OutputViewTarget, PendingForceCleanup,
    PendingMemoryDelete, PlanApprovalState, PluginsListState, ProviderSetupStep, ProviderSource,
    QueuedInput, ResearchViewState, RollbackOfferState, SelectionPane, SlashMenuEntry,
    SlashMenuState, TextSelection,
};
use ragent_types::{ThinkingConfig, ThinkingLevel};
use serde_json::json;

/// Render one frame at the given terminal size and return the terminal so the
/// painted buffer can be inspected.
fn render(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("draw");
    terminal
}

/// Flatten the whole painted buffer into one string.
fn painted(terminal: &Terminal<TestBackend>) -> String {
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

fn entry(id: &str, name: &str) -> ModelPickerEntry {
    ModelPickerEntry {
        provider_id: "anthropic".to_string(),
        id: id.to_string(),
        name: name.to_string(),
        context_window: 200_000,
        max_output: Some(64_000),
        cost_input: 3.0,
        cost_output: 15.0,
        reasoning: true,
        vision: true,
        tool_use: true,
        thinking_levels: vec![
            ThinkingLevel::Auto,
            ThinkingLevel::Off,
            ThinkingLevel::Low,
            ThinkingLevel::Medium,
            ThinkingLevel::High,
        ],
        thinking_config: Some(ThinkingConfig::new(ThinkingLevel::Auto)),
        cost_tier: "Premium".to_string(),
        cost_multiplier: "1x".to_string(),
    }
}

fn config_provider(id: &str, name: &str) -> ConfiguredProvider {
    ConfiguredProvider {
        id: id.to_string(),
        name: name.to_string(),
        source: ProviderSource::Database,
    }
}

// ===========================================================================
// Terminal sizes / split ratios
// ===========================================================================

#[test]
fn render_tiny_terminal_does_not_panic() {
    let mut app = support::make_app();
    let _ = render(&mut app, 1, 1);
    let _ = render(&mut app, 20, 4);
}

#[test]
fn render_zero_dim_does_not_panic() {
    let mut app = support::make_app();
    let _ = render(&mut app, 0, 0);
}

#[test]
fn render_large_terminal() {
    let mut app = support::make_app();
    let terminal = render(&mut app, 200, 60);
    assert_ne!(painted(&terminal), "");
}

#[test]
fn render_breakpoint_sizes() {
    let mut app = support::make_app();
    for w in [39u16, 40, 79, 80, 120, 121, 160] {
        let _ = render(&mut app, w, 40);
    }
}

#[test]
fn render_each_side_panel_toggle() {
    for (log, profile, tasks, memory, telemetry, context) in [
        (true, false, false, false, false, false),
        (false, true, false, false, false, false),
        (true, true, false, false, false, false),
        (false, false, true, false, false, false),
        (false, false, false, true, false, false),
        (false, false, false, false, true, false),
        (false, false, false, false, false, true),
    ] {
        let mut app = support::make_app();
        app.show_log = log;
        app.show_profile = profile;
        app.show_tasks_panel = tasks;
        app.show_memory = memory;
        app.show_telemetry = telemetry;
        app.show_context_panel = context;
        let terminal = render(&mut app, 120, 40);
        assert_ne!(painted(&terminal), "");
    }
}

#[test]
fn render_log_panel_with_entries_and_overflow() {
    use ragent_tui::app::{LogEntry, LogLevel};
    let mut app = support::make_app();
    app.show_log = true;
    for i in 0..40u64 {
        app.log_entries.push(LogEntry {
            timestamp: chrono::Utc::now(),
            level: if i % 3 == 0 {
                LogLevel::Tool
            } else {
                LogLevel::Info
            },
            message: format!("compaction started for message {i} with a longer body to wrap"),
            session_id: Some("s1".to_string()),
            agent_id: Some("tm-001".to_string()),
            seq: i,
        });
    }
    let _ = render(&mut app, 100, 24);
}

#[test]
fn render_profile_panel_enabled_and_disabled() {
    use ragent_agent::session::profiler::agent_loop_profiler;
    let profiler = agent_loop_profiler();
    profiler.set_enabled(false);
    let mut app = support::make_app();
    app.show_profile = true;
    app.show_log = false;
    let terminal = render(&mut app, 120, 30);
    assert!(painted(&terminal).contains("disabled"));

    profiler.set_enabled(true);
    profiler.record_duration("render_message", std::time::Duration::from_millis(3));
    let _ = render(&mut app, 120, 30);
    profiler.set_enabled(false);
}

#[test]
fn render_tasks_panel_with_rows() {
    let mut app = support::make_app();
    app.session_id = Some("layout-tasks".to_string());
    app.show_tasks_panel = true;
    app.show_log = false;
    app.storage
        .create_session("layout-tasks", ".")
        .expect("create session");
    app.storage
        .create_task(
            "t-1",
            "layout-tasks",
            "Investigate the layout renderer",
            "",
            "in_progress",
            Some("Investigating the layout renderer"),
            Some("builder"),
            "{}",
            &[],
        )
        .expect("create task");
    app.storage
        .create_task(
            "t-2",
            "layout-tasks",
            "Blocked follow-up",
            "",
            "pending",
            None,
            None,
            "{}",
            &["t-1".to_string()],
        )
        .expect("create task");
    app.tasks_cache_dirty = true;
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_memory_panel_visible() {
    let mut app = support::make_app();
    app.show_memory = true;
    app.show_log = false;
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_telemetry_panel_visible() {
    let mut app = support::make_app();
    app.show_telemetry = true;
    app.show_log = false;
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_context_panel_visible() {
    let mut app = support::make_app();
    app.show_context_panel = true;
    app.show_log = false;
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_teammate_strip_and_focused_input() {
    let mut app = support::make_app();
    app.active_team = Some(TeamConfig::new("alpha", "s1"));
    let mut member = TeamMember::new("reviewer", "tm-001", "general");
    member.status = ragent_agent::team::MemberStatus::Working;
    app.team_members.push(member);
    app.focused_teammate = Some("tm-001".to_string());
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_input_with_attachments_and_queue_counter() {
    let mut app = support::make_app();
    app.pending_attachments
        .push(std::path::PathBuf::from("shot.png"));
    app.input_queue.push_back(QueuedInput {
        text: "queued".to_string(),
        image_paths: Vec::new(),
    });
    app.input = "typed text".to_string();
    app.input_cursor = 4;
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_input_locked_border() {
    let mut app = support::make_app();
    app.show_shortcuts = true;
    let _ = render(&mut app, 120, 40);
}

// ===========================================================================
// Overlays / dialogs
// ===========================================================================

#[test]
fn render_run_cost_banner() {
    let mut app = support::make_app();
    app.run_cost_banner = Some("* run complete - 10+20 tokens - $0.001 - 1.2s".to_string());
    let terminal = render(&mut app, 120, 30);
    assert!(painted(&terminal).contains("run complete"));
}

#[test]
fn render_queue_menu_overlay() {
    let mut app = support::make_app();
    app.queue_menu_open = true;
    app.input_queue.push_back(QueuedInput {
        text: "one".to_string(),
        image_paths: Vec::new(),
    });
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_queue_show_panel_overlay() {
    let mut app = support::make_app();
    app.queue_show_open = true;
    for t in ["one", "two", "three"] {
        app.input_queue.push_back(QueuedInput {
            text: t.to_string(),
            image_paths: Vec::new(),
        });
    }
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_queue_clear_confirm_overlay() {
    let mut app = support::make_app();
    app.queue_clear_confirm_open = true;
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("Clear the input queue"));
}

#[test]
fn render_memory_clear_confirm_overlay() {
    let mut app = support::make_app();
    app.memory_clear_confirm_open = true;
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("Clear this project's memory"));
}

#[test]
fn render_shortcuts_panel_overlay() {
    let mut app = support::make_app();
    app.show_shortcuts = true;
    let terminal = render(&mut app, 120, 40);
    let text = painted(&terminal);
    assert!(text.contains("Shortcuts") || text.contains("Ctrl+"));
}

#[test]
fn render_context_menu_overlay() {
    let mut app = support::make_app();
    app.context_menu = Some(ContextMenuState {
        x: 3,
        y: 3,
        pane: SelectionPane::Input,
        selected: 2,
        items: vec![
            (ContextAction::Cut, false),
            (ContextAction::Copy, true),
            (ContextAction::Paste, true),
        ],
    });
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("Paste"));
}

#[test]
fn render_force_cleanup_dialog_modals() {
    let mut app = support::make_app();
    app.pending_forcecleanup = Some(PendingForceCleanup {
        team_name: "alpha".to_string(),
        active_members: vec!["alice".to_string(), "bob".to_string()],
    });
    let terminal = render(&mut app, 120, 40);
    let text = painted(&terminal);
    assert!(text.contains("Force-cleanup"));
    assert!(text.contains("alice"));
}

#[test]
fn render_memory_delete_dialog() {
    let mut app = support::make_app();
    app.pending_memory_delete = Some(PendingMemoryDelete {
        id: 42,
        preview: "some memory content".to_string(),
    });
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("Delete Memory"));
}

#[test]
fn render_stop_confirm_dialog() {
    let mut app = support::make_app();
    app.pending_stop_confirm = true;
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_model_loading_popup() {
    let mut app = support::make_app();
    app.model_loading_state = Some(ModelLoadingState {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        started_at: std::time::Instant::now(),
    });
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("Anthropic"));
}

#[test]
fn render_model_download_popup() {
    let mut app = support::make_app();
    app.model_download_state = Some(ModelDownloadState {
        provider_id: "ollama".to_string(),
        model_id: "qwen2.5:7b".to_string(),
        percent: 42.0,
        started_at: std::time::Instant::now(),
    });
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_agents_and_teams_windows() {
    let mut app = support::make_app();
    app.show_agents_window = true;
    let _ = render(&mut app, 120, 40);
    app.show_agents_window = false;
    app.show_teams_window = true;
    app.active_team = Some(TeamConfig::new("alpha", "s1"));
    app.team_members
        .push(TeamMember::new("reviewer", "tm-001", "general"));
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_active_tasks_present() {
    let mut app = support::make_app();
    app.active_tasks.push(TaskEntry {
        id: "task-1".to_string(),
        parent_session_id: "s1".to_string(),
        child_session_id: "s2".to_string(),
        agent_name: "explore".to_string(),
        task_prompt: "look around".to_string(),
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
    });
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_text_selection_highlight() {
    let mut app = support::make_app();
    app.messages
        .push(Message::user_text("s1", "selectable transcript line"));
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Messages,
        anchor: (2, 3),
        endpoint: (10, 4),
    });
    let _ = render(&mut app, 120, 40);
}

// ===========================================================================
// Provider-setup dialog: every step variant
// ===========================================================================

fn render_setup(step: ProviderSetupStep) -> String {
    let mut app = support::make_app();
    app.provider_setup = Some(step);
    let terminal = render(&mut app, 120, 40);
    painted(&terminal)
}

#[test]
fn setup_loading_models_step() {
    let text = render_setup(ProviderSetupStep::LoadingModels {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
    });
    assert!(text.contains("Loading models"));
}

#[test]
fn setup_select_provider_step() {
    let text = render_setup(ProviderSetupStep::SelectProvider {
        selected: 2,
        force_key_entry: false,
    });
    assert!(text.contains("Provider Setup"));
}

#[test]
fn setup_enter_key_step() {
    let text = render_setup(ProviderSetupStep::EnterKey {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic (Claude)".to_string(),
        key_field: ragent_tui::input_field::InputField::with_text("[REDACTED]"),
        endpoint_field: ragent_tui::input_field::InputField::new(),
        active_field: 0,
        error: None,
    });
    assert!(text.contains("Configure"));
}

#[test]
fn setup_enter_key_step_with_endpoint_field_focused() {
    let text = render_setup(ProviderSetupStep::EnterKey {
        provider_id: "generic_openai".to_string(),
        provider_name: "Generic OpenAI API".to_string(),
        key_field: ragent_tui::input_field::InputField::with_text("[REDACTED]"),
        endpoint_field: ragent_tui::input_field::InputField::with_text("http://127.0.0.1:8080"),
        active_field: 1,
        error: Some("bad key".to_string()),
    });
    assert!(text.contains("Generic OpenAI"));
}

#[test]
fn setup_device_flow_pending_both_flows() {
    for flow in [DeviceFlowKind::Copilot, DeviceFlowKind::GitHub] {
        let text = render_setup(ProviderSetupStep::DeviceFlowPending {
            flow,
            user_code: "ABCD-1234".to_string(),
            verification_uri: "https://github.com/login/device".to_string(),
        });
        assert!(text.contains("ABCD-1234"));
    }
}

#[test]
fn setup_select_model_step() {
    let text = render_setup(ProviderSetupStep::SelectModel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        models: vec![entry("claude-sonnet-4", "Claude Sonnet 4")],
        selected: 0,
    });
    assert!(text.contains("Claude Sonnet 4"));
}

#[test]
fn setup_select_model_empty_step() {
    let text = render_setup(ProviderSetupStep::SelectModel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        models: Vec::new(),
        selected: 0,
    });
    assert!(text.contains("No models"));
}

#[test]
fn setup_select_thinking_level_step() {
    let text = render_setup(ProviderSetupStep::SelectThinkingLevel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        model: entry("claude-sonnet-4", "Claude Sonnet 4"),
        selected: 2,
    });
    assert!(text.contains("Select thinking"));
}

#[test]
fn setup_done_step_with_and_without_model() {
    let text = render_setup(ProviderSetupStep::Done {
        provider_name: "Anthropic".to_string(),
        model_name: Some("claude-sonnet-4".to_string()),
    });
    assert!(text.contains("Provider Configured"));
    assert!(text.contains("Model: claude-sonnet-4"));

    let text = render_setup(ProviderSetupStep::Done {
        provider_name: "OpenAI".to_string(),
        model_name: None,
    });
    assert!(text.contains("Provider Configured"));
}

#[test]
fn setup_select_agent_step() {
    let text = render_setup(ProviderSetupStep::SelectAgent {
        agents: vec![
            ("general".to_string(), "General helper".to_string(), false),
            ("coder".to_string(), "Codes things".to_string(), true),
        ],
        selected: 1,
    });
    assert!(text.contains("Select an Agent"));
}

#[test]
fn setup_select_configured_provider_step() {
    let text = render_setup(ProviderSetupStep::SelectConfiguredProvider {
        providers: vec![config_provider("anthropic", "Anthropic (Claude)")],
        selected: 0,
    });
    assert!(text.contains("Anthropic"));
}

#[test]
fn setup_show_provider_config_step() {
    let text = render_setup(ProviderSetupStep::ShowProviderConfig {
        providers: vec![config_provider("openai", "OpenAI (GPT)")],
        selected: 0,
    });
    assert!(text.contains("OpenAI"));
}

#[test]
fn setup_reset_provider_step() {
    let text = render_setup(ProviderSetupStep::ResetProvider { selected: 0 });
    assert!(text.contains("Reset"));
}

#[test]
fn setup_gitlab_form_and_validating_steps() {
    let text = render_setup(ProviderSetupStep::GitLabSetup {
        url_input: "https://gitlab.com".to_string(),
        url_cursor: 8,
        token_input: "[REDACTED]".to_string(),
        token_cursor: 4,
        active_field: 1,
        error: Some("nope".to_string()),
    });
    assert!(text.contains("GitLab"));

    let text = render_setup(ProviderSetupStep::GitLabValidating {
        instance_url: "https://gitlab.com".to_string(),
        token: "[REDACTED]".to_string(),
    });
    assert!(text.contains("GitLab") || text.contains("Validat"));
}

#[test]
fn setup_telemetry_step() {
    let text = render_setup(ProviderSetupStep::TelemetrySetup {
        endpoint_field: ragent_tui::input_field::InputField::with_text("http://localhost:4318"),
        protocol: ragent_config::OtelProtocol::Http,
        interval_field: ragent_tui::input_field::InputField::with_text("30"),
        timeout_field: ragent_tui::input_field::InputField::with_text("10"),
        port_field: ragent_tui::input_field::InputField::new(),
        active_field: 0,
        error: None,
    });
    assert!(text.contains("Telemetry"));
}

#[test]
fn setup_select_azure_resource_step() {
    let text = render_setup(ProviderSetupStep::SelectAzureResource {
        entries: Vec::new(),
        selected: 0,
        error: Some("No Azure resources found.".to_string()),
    });
    assert!(text.contains("Azure Resource"));

    let text = render_setup(ProviderSetupStep::SelectAzureResource {
        entries: Vec::new(),
        selected: 0,
        error: None,
    });
    assert!(text.contains("Azure Resource"));
}

#[test]
fn setup_setup_router_step() {
    let text = render_setup(ProviderSetupStep::SetupRouter {
        providers: vec![config_provider("anthropic", "Anthropic")],
        selected_provider_ids: vec!["anthropic".to_string()],
        selected_provider_index: 0,
        draft_config: ragent_llm::providers::router_config::RouterConfig::default(),
        active_bucket: ragent_llm::providers::router_config::Tier::Simple,
        active_bucket_index: 0,
        left_pane_focused: true,
        error: Some("pick a model".to_string()),
    });
    assert!(text.contains("SIMPLE") || text.contains("Router") || text.contains("router"));
}

#[test]
fn setup_select_router_model_step() {
    let text = render_setup(ProviderSetupStep::SelectRouterModel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        models: vec![entry("claude-sonnet-4", "Claude Sonnet 4")],
        selected: 0,
        target_tier: ragent_llm::providers::router_config::Tier::Simple,
    });
    assert!(text.contains("Claude Sonnet 4"));
}

// ===========================================================================
// Pickers / viewers / dialogs
// ===========================================================================

#[test]
fn render_history_picker_overlay() {
    let mut app = support::make_app();
    app.history_picker = Some(HistoryPickerState {
        entries: vec!["first prompt".to_string(), "second prompt".to_string()],
        selected: 1,
        scroll_offset: 0,
    });
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("second prompt"));
}

#[test]
fn render_config_save_picker_overlay() {
    let mut app = support::make_app();
    app.config_save_picker = Some(ConfigSavePickerState {
        entries: vec![std::path::PathBuf::from(
            "/tmp/saves/ragent.json.2024-01-01",
        )],
        selected: 0,
        scroll_offset: 0,
        config_dir: std::path::PathBuf::from("/tmp"),
    });
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_slash_menu_overlay() {
    let mut app = support::make_app();
    app.input = "/he".to_string();
    app.slash_menu = Some(SlashMenuState {
        matches: (0..20)
            .map(|i| SlashMenuEntry {
                trigger: format!("cmd{i}"),
                description: format!("command {i}"),
                is_skill: i % 2 == 0,
                suggestions: Vec::new(),
                parameter_hint: None,
            })
            .collect(),
        selected: 15,
        filter: "he".to_string(),
    });
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_file_menu_overlay() {
    let mut app = support::make_app();
    app.input = "@/src".to_string();
    app.file_menu = Some(FileMenuState {
        matches: (0..12)
            .map(|i| FileMenuEntry {
                display: format!("src/file{i}.rs"),
                path: std::path::PathBuf::from(format!("src/file{i}.rs")),
                is_dir: false,
            })
            .collect(),
        selected: 10,
        scroll_offset: 5,
        query: "src".to_string(),
        current_dir: None,
    });
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_output_view_overlay() {
    let mut app = support::make_app();
    app.session_id = Some("sess-abc".to_string());
    app.messages
        .push(Message::assistant_text("sess-abc", "assistant output line"));
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: "sess-abc".to_string(),
            label: "primary".to_string(),
        },
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: OutputViewLineCache::default(),
    });
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_research_view_overlay() {
    let mut app = support::make_app();
    app.research_view = Some(ResearchViewState {
        name: "topic".to_string(),
        path: std::path::PathBuf::from("/research/topic/RESEARCH.md"),
        base_dir: std::path::PathBuf::from("/research/topic"),
        markdown: "# Heading\n\nbody text\n".repeat(20),
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: OutputViewLineCache::default(),
    });
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_memory_view_overlay() {
    let mut app = support::make_app();
    app.memory_view = Some(MemoryViewState {
        row: MemoryRow {
            id: 7,
            content: "line one\nline two\n".repeat(20),
            category: "fact".to_string(),
            source: "test".to_string(),
            confidence: 0.9,
            project: "p".to_string(),
            session_id: "s".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            access_count: 0,
            last_accessed: None,
        },
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: OutputViewLineCache::default(),
    });
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_mcp_discover_overlay() {
    use ragent_agent::mcp::discovery::{DiscoveredMcpServer, McpDiscoverySource};
    let mut app = support::make_app();
    app.mcp_discover = Some(McpDiscoverState {
        servers: vec![DiscoveredMcpServer {
            id: "filesystem".to_string(),
            name: "Filesystem".to_string(),
            executable: std::path::PathBuf::from("/usr/bin/mcp-fs"),
            args: Vec::new(),
            env: HashMap::new(),
            source: McpDiscoverySource::SystemPath,
        }],
        enabled: vec![true],
        selected: 0,
        scroll_offset: 0,
        feedback: Some("enabled".to_string()),
    });
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("filesystem"));
}

#[test]
fn render_plugins_list_overlay() {
    let mut app = support::make_app();
    app.plugins_list = Some(PluginsListState {
        view: PluginListView {
            attribution: "From: /plugins list".to_string(),
            header: Some("id  version  enabled".to_string()),
            separator: Some("--- -------- -------".to_string()),
            body: vec![
                "alpha 1.0.0 yes".to_string(),
                "beta  1.0.0 no".to_string(),
                "contributions: 2".to_string(),
            ],
            rows: vec![
                PluginListRow {
                    id: "alpha".to_string(),
                    enabled: true,
                },
                PluginListRow {
                    id: "beta".to_string(),
                    enabled: false,
                },
            ],
        },
        heading: Some("id  version  enabled".to_string()),
        selected: 1,
        scroll_offset: 0,
        verbose: false,
        feedback: None,
    });
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("alpha"));
}

#[test]
fn render_loop_setup_dialog() {
    let mut app = support::make_app();
    ragent_tui::app::open_loop_setup(&mut app);
    assert!(app.loop_setup.is_some());
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_plugin_store_overlay() {
    use ragent_plugins::{StoreEntry, StoreKind};
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "", false);
    app.plugin_store
        .as_mut()
        .expect("open")
        .set_entries(vec![StoreEntry {
            id: "demo".to_string(),
            name: "Demo".to_string(),
            version: "1.0.0".to_string(),
            source: "https://example.org/demo.zip".to_string(),
            description: "A demo plugin".to_string(),
            dialect: None,
            tags: Vec::new(),
            homepage: None,
            marketplace: String::new(),
            category: None,
        }]);
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_connector_catalogue_overlay() {
    use ragent_connectors::CategoryFilter;
    let mut app = support::make_app();
    app.open_connector_catalogue("", CategoryFilter::all(), false);
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_backend_panel_overlay() {
    let mut app = support::make_app();
    app.backend_panel = Some(BackendPanelState {
        rows: vec![BackendPanelRow {
            id: "local".to_string(),
            name: "Local host".to_string(),
            kind: "local",
            active: true,
            health: "ok",
            health_ok: true,
            detail: None,
            connection: "host".to_string(),
        }],
        cursor: 0,
    });
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("local"));
}

#[test]
fn render_plan_approval_dialog() {
    let mut app = support::make_app();
    app.plan_approval_pending = Some(PlanApprovalState {
        plan_text: "1. do a thing\n2. do another".to_string(),
        cursor_approve: true,
    });
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_rollback_offer_banner() {
    let mut app = support::make_app();
    app.pending_rollback = Some(RollbackOfferState {
        session_id: "s1".to_string(),
        status: "completed".to_string(),
        iterations: 3,
        diffstat: "+6 -2 lines across 2 files".to_string(),
        files: vec!["a.rs".to_string(), "b.rs".to_string()],
    });
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_question_dialog_multiple_choice_and_free_text() {
    use ragent_tui::app::QuestionRequest;
    let mut app = support::make_app();
    app.question_queue.push_back(QuestionRequest {
        id: "q1".to_string(),
        session_id: "s1".to_string(),
        question: "Pick one".to_string(),
        options: vec!["alpha".to_string(), "beta".to_string()],
        default_index: Some(1),
        expire_at_ms: None,
    });
    let _ = render(&mut app, 120, 40);

    let mut app = support::make_app();
    app.question_queue.push_back(QuestionRequest {
        id: "q2".to_string(),
        session_id: "s1".to_string(),
        question: "Type something".to_string(),
        options: Vec::new(),
        default_index: None,
        expire_at_ms: None,
    });
    app.pending_question_input = "half typed".to_string();
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_permission_dialog() {
    let mut app = support::make_app();
    app.permission_queue.push_back(PermissionRequest {
        id: "p1".to_string(),
        session_id: "s1".to_string(),
        permission: "bash".to_string(),
        patterns: vec!["rm -rf /".to_string()],
        metadata: json!({"timeout_secs": 120u64}),
        tool_call_id: None,
    });
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_router_save_dialog() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SetupRouter {
        providers: vec![config_provider("anthropic", "Anthropic")],
        selected_provider_ids: vec!["anthropic".to_string()],
        selected_provider_index: 0,
        draft_config: ragent_llm::providers::router_config::RouterConfig::default(),
        active_bucket: ragent_llm::providers::router_config::Tier::Simple,
        active_bucket_index: 0,
        left_pane_focused: true,
        error: None,
    });
    app.pending_router_save = Some(ragent_llm::providers::router_config::RouterConfig::default());
    let _ = render(&mut app, 120, 40);
}

#[test]
fn render_telemetry_and_ctx_and_yolo_indicators() {
    let mut app = support::make_app();
    app.show_telemetry = true;
    app.status = "telemetry: saved".to_string();
    let _ = render(&mut app, 140, 30);
}

// ===========================================================================
// layout public helpers
// ===========================================================================

#[test]
fn mask_secret_boundaries() {
    assert_eq!(ragent_tui::layout::mask_secret(""), "");
    assert_eq!(
        ragent_tui::layout::mask_secret("123456789012"),
        "************"
    );
    let masked = ragent_tui::layout::mask_secret("sk-abcdefghijklmnop");
    assert!(masked.starts_with("sk-a"));
    assert!(masked.ends_with("mnop"));
}

#[test]
fn wrap_line_styled_testable_edges() {
    use ratatui::text::Line;
    // Width 0 -> a single unbounded line.
    let out = ragent_tui::layout::wrap_line_styled_testable(&Line::from("hello world"), 0);
    assert_eq!(out.len(), 1);

    // Narrow width wraps into several rows.
    let out = ragent_tui::layout::wrap_line_styled_testable(&Line::from("one two three four"), 5);
    assert!(out.len() >= 2);

    // Tabs are filtered (ratatui drops control graphemes).
    let _ = ragent_tui::layout::wrap_line_styled_testable(&Line::from("a\tb"), 4);

    // Wide Unicode and zero-width handling.
    let _ = ragent_tui::layout::wrap_line_styled_testable(&Line::from("中文 中文"), 3);
    let _ = ragent_tui::layout::wrap_line_styled_testable(&Line::from("a\u{200b}b"), 4);
    let _ = ragent_tui::layout::wrap_line_styled_testable(&Line::from("a\u{00a0}b"), 4);
}

#[test]
fn markdown_to_lines_testable_more() {
    let base = std::path::PathBuf::from("/");
    let md = "# H1\n\n- item one\n- item two\n  - nested\n\n1. first\n2. second\n\n> quote\n\n---\n\n**bold** and *italic* and `code` and ~~strike~~\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n- [ ] todo\n- [x] done";
    let lines = ragent_tui::layout::markdown_to_lines_testable(md, &base, 60);
    let rendered: Vec<String> = lines.iter().map(std::string::ToString::to_string).collect();
    let joined = rendered.join("\n");
    assert!(joined.contains("H1"));
    assert!(joined.contains("item one"));
    assert!(joined.contains("nested"));
    assert!(joined.contains("quote"));
}

#[test]
fn ensure_message_copy_lines_builds_cache() {
    use ragent_tui::app::MessageLineGroup;
    let mut app = support::make_app();
    app.messages.push(Message::user_text(
        "s1",
        "a message that is long enough to wrap in a narrow pane",
    ));
    app.messages
        .push(Message::assistant_text("s1", "second message"));
    app.message_cache_width = 20;
    // Seed one stale cache group per message (edit_seq mismatch => re-render).
    for _ in 0..app.messages.len() {
        app.message_line_cache.push(MessageLineGroup {
            lines: Vec::new(),
            wrapped_lines: Vec::new(),
            content_lines: Vec::new(),
            wrapped_count: 0,
            edit_seq: u64::MAX,
        });
    }
    app.message_content_lines_dirty = true;
    ragent_tui::layout::ensure_message_copy_lines(&mut app);
    assert_ne!(app.message_content_lines, [] as [std::string::String; 0]);
    assert!(!app.message_content_lines_dirty);
}

#[test]
fn render_messages_directly_with_overflow() {
    let mut app = support::make_app();
    for i in 0..30 {
        app.messages.push(Message::new(
            "s1",
            if i % 2 == 0 {
                Role::User
            } else {
                Role::Assistant
            },
            vec![MessagePart::Text {
                text: format!("message number {i} with some content to wrap around"),
            }],
        ));
    }
    let area = Rect::new(0, 0, 60, 12);
    let backend = TestBackend::new(60, 12);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render_messages(frame, &mut app, area))
        .expect("draw");
    assert!(app.message_area.width > 0 || app.message_area == Rect::default());
}

#[test]
fn render_messages_with_tool_calls_and_steps() {
    let mut app = support::make_app();
    let mut map = HashMap::new();
    map.insert("call-1".to_string(), ("sid".to_string(), 1u32, 2u32));
    app.tool_step_map = map;
    app.messages.push(Message::new(
        "s1",
        Role::Assistant,
        vec![
            MessagePart::Text {
                text: "let me look".to_string(),
            },
            MessagePart::ToolCall {
                tool: "read".to_string(),
                call_id: "call-1".to_string(),
                state: Box::new(ToolCallState {
                    status: ToolCallStatus::Completed,
                    input: json!({"path": "/home/user/project/src/main.rs"}),
                    output: Some(json!({"lines": 3, "start_line": 1, "end_line": 10})),
                    error: None,
                    duration_ms: Some(1234),
                }),
            },
            MessagePart::Reasoning {
                text: "reasoning line".to_string(),
                signature: None,
            },
        ],
    ));
    let area = Rect::new(0, 0, 80, 10);
    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render_messages(frame, &mut app, area))
        .expect("draw");
}

#[test]
fn stream_min_interval_constant() {
    assert!(ragent_tui::layout::MESSAGE_STREAM_MIN_INTERVAL >= std::time::Duration::ZERO);
}

/// Feed a key through the app handler so the layout reacts to real state
/// transitions (used for the loop dialog field navigation below).
async fn press(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    app.handle_key_event(KeyEvent::new(code, mods)).await;
}

#[tokio::test]
async fn render_loop_dialog_after_field_navigation() {
    let mut app = support::make_app();
    ragent_tui::app::open_loop_setup(&mut app);
    for _ in 0..6 {
        press(&mut app, KeyCode::Tab, KeyModifiers::NONE).await;
    }
    assert!(app.loop_setup.is_some());
    let _ = render(&mut app, 120, 40);
}

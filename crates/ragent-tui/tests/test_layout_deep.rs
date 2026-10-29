//! Deep branch coverage for `ragent_tui::layout` plus three small reusable
//! widgets and the status-bar / teams / active-agents panel renderers.
//!
//! Every test drives the real public render entry point through a ratatui
//! `TestBackend` (or a plain `Buffer` for the `Widget` impls) and reads the
//! painted output back, so no renderer logic is re-implemented here.
//!
//! Depends on `tests/support/mod.rs` for `support::make_app()` and on the
//! `crate::global_env_lock()` defined by the consolidated `all` test target for
//! the one test that touches process-global profiler state.
//!
//! Tests only - no production logic is touched.

mod support;

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};

use ragent_agent::agent::oasf::OasfAgentRecord;
use ragent_agent::agent::{AgentInfo, CustomAgentDef, ModelRef};
use ragent_agent::message::{ImageData, Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use ragent_agent::permission::PermissionRequest;
use ragent_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use ragent_agent::team::{MemberStatus, TeamConfig, TeamMember};
use ragent_plugins::{PluginListRow, PluginListView};
use ragent_tui::App;
use ragent_tui::app::{
    BgTaskView, ConfigSavePickerState, ConfiguredProvider, DeviceFlowKind, FileMenuEntry,
    FileMenuState, HistoryPickerState, LogEntry, LogLevel, McpDiscoverState, ModelLoadingState,
    ModelPickerEntry, OutputViewLineCache, OutputViewState, OutputViewTarget, PluginsListState,
    ProviderSetupStep, ProviderSource, QueuedInput,
};
use ragent_tui::input_field::InputField;
use ragent_tui::research_progress::ResearchProgress;
use ragent_tui::widgets::dialog::{
    Dialog, DialogAlignment, DialogContent, DialogSize, DialogVariant,
};
use ragent_tui::widgets::permission_dialog::PermissionDialog;
use ragent_tui::widgets::selectable_list::{SelectableList, SelectableListRender};
use ragent_types::{ThinkingConfig, ThinkingLevel};
use serde_json::json;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// Render one frame of the whole TUI at the given terminal size.
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

/// Flatten a standalone `Buffer` into one string.
fn buffer_text(buf: &Buffer) -> String {
    buf.content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

/// A scratch directory under the git-ignored `target/temp` tree.
fn scratch_dir(tag: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("temp")
        .join(tag);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn entry(id: &str, name: &str, ctx_window: usize) -> ModelPickerEntry {
    ModelPickerEntry {
        provider_id: "anthropic".to_string(),
        id: id.to_string(),
        name: name.to_string(),
        context_window: ctx_window,
        max_output: Some(64_000),
        cost_input: 3.0,
        cost_output: 15.0,
        reasoning: true,
        vision: true,
        tool_use: true,
        thinking_levels: vec![ThinkingLevel::Auto, ThinkingLevel::Off, ThinkingLevel::High],
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

fn render_setup(step: ProviderSetupStep, width: u16, height: u16) -> String {
    let mut app = support::make_app();
    app.provider_setup = Some(step);
    let terminal = render(&mut app, width, height);
    painted(&terminal)
}

fn task_entry(
    id: &str,
    parent: &str,
    child: &str,
    agent: &str,
    status: TaskStatus,
    background: bool,
) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: parent.to_string(),
        child_session_id: child.to_string(),
        agent_name: agent.to_string(),
        task_prompt: "do a thing".to_string(),
        background,
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
        time_bound_secs: None,
    }
}

// ===========================================================================
// render_provider_setup_dialog: every variant
// ===========================================================================

#[test]
fn setup_loading_models_uses_elapsed_spinner() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::LoadingModels {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
    });
    app.model_loading_state = Some(ModelLoadingState {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        started_at: std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(3))
            .expect("instant arithmetic"),
    });
    let terminal = render(&mut app, 120, 40);
    let text = painted(&terminal);
    assert!(text.contains("Loading models"));
    assert!(text.contains("Fetching model list"));
}

#[test]
fn setup_select_provider_scrolling_with_high_selection() {
    // Height 20 gives an inner list of 10 rows against 14 providers, so the
    // scrolling footer variant renders.
    let text = render_setup(
        ProviderSetupStep::SelectProvider {
            selected: 13,
            force_key_entry: false,
        },
        120,
        20,
    );
    assert!(text.contains("more above"));
    assert!(text.contains("more below"));
}

#[test]
fn setup_select_provider_without_scrolling_and_ollama_badge() {
    let text = render_setup(
        ProviderSetupStep::SelectProvider {
            selected: 0,
            force_key_entry: true,
        },
        120,
        60,
    );
    assert!(text.contains("Select a Provider"));
    assert!(text.contains("[local]"));
}

#[test]
fn setup_enter_key_generic_openai_both_fields() {
    let text = render_setup(
        ProviderSetupStep::EnterKey {
            provider_id: "generic_openai".to_string(),
            provider_name: "Generic OpenAI API".to_string(),
            key_field: InputField::with_text("sk-abcdefghijklmnop"),
            endpoint_field: InputField::with_text("http://127.0.0.1:8080/v1"),
            active_field: 1,
            error: Some("bad key".to_string()),
        },
        120,
        40,
    );
    assert!(text.contains("Endpoint URL"));
    assert!(text.contains("bad key"));
}

#[test]
fn setup_enter_key_azure_foundry_empty_endpoint_placeholder() {
    let text = render_setup(
        ProviderSetupStep::EnterKey {
            provider_id: "azure_foundry".to_string(),
            provider_name: "Azure AI Foundry".to_string(),
            key_field: InputField::new(),
            endpoint_field: InputField::new(),
            active_field: 0,
            error: None,
        },
        120,
        40,
    );
    assert!(text.contains("(use default/env)"));
}

#[test]
fn setup_device_flow_both_kinds() {
    for (flow, expect) in [
        (DeviceFlowKind::Copilot, "Copilot"),
        (DeviceFlowKind::GitHub, "GitHub Authentication"),
    ] {
        let text = render_setup(
            ProviderSetupStep::DeviceFlowPending {
                flow,
                user_code: "WXYZ-9876".to_string(),
                verification_uri: "https://github.com/login/device".to_string(),
            },
            120,
            40,
        );
        assert!(text.contains("WXYZ-9876"), "code painted for {expect}");
        assert!(text.contains("github.com/login/device"));
    }
}

#[test]
fn setup_select_model_context_window_and_feature_columns() {
    // Covers the 1M / 1K / raw context-window formats and the empty-feature
    // "-" fallback.
    let mut wide = entry("model-wide", "Wide Window", 2_000_000);
    wide.reasoning = false;
    wide.vision = false;
    wide.tool_use = false;
    let text = render_setup(
        ProviderSetupStep::SelectModel {
            provider_id: "anthropic".to_string(),
            provider_name: "Anthropic".to_string(),
            models: vec![wide, entry("model-mid", "Mid Window", 200_000)],
            selected: 1,
        },
        140,
        40,
    );
    assert!(text.contains("Wide Window"));
    assert!(text.contains("2M"));
    assert!(text.contains("200K"));
}

#[test]
fn setup_select_model_scrolled_past_first_page() {
    let models: Vec<ModelPickerEntry> = (0..40)
        .map(|i| entry(&format!("m{i}"), &format!("Model {i}"), 8_192))
        .collect();
    let text = render_setup(
        ProviderSetupStep::SelectModel {
            provider_id: "anthropic".to_string(),
            provider_name: "Anthropic".to_string(),
            models,
            selected: 35,
        },
        120,
        24,
    );
    assert!(text.contains("Showing"));
}

#[test]
fn setup_select_model_empty_and_thinking_level() {
    let text = render_setup(
        ProviderSetupStep::SelectModel {
            provider_id: "anthropic".to_string(),
            provider_name: "Anthropic".to_string(),
            models: Vec::new(),
            selected: 0,
        },
        120,
        40,
    );
    assert!(text.contains("No models are currently available"));

    let text = render_setup(
        ProviderSetupStep::SelectThinkingLevel {
            provider_id: "anthropic".to_string(),
            provider_name: "Anthropic".to_string(),
            model: entry("claude", "Claude", 200_000),
            selected: 2,
        },
        120,
        40,
    );
    assert!(text.contains("Select thinking for Claude"));
    assert!(text.contains("deepest reasoning budget"));
}

#[test]
fn setup_done_with_and_without_model() {
    let text = render_setup(
        ProviderSetupStep::Done {
            provider_name: "Anthropic".to_string(),
            model_name: Some("claude-sonnet-4".to_string()),
        },
        120,
        40,
    );
    assert!(text.contains("Provider Configured"));
    assert!(text.contains("Model: claude-sonnet-4"));

    let text = render_setup(
        ProviderSetupStep::Done {
            provider_name: "OpenAI".to_string(),
            model_name: None,
        },
        120,
        40,
    );
    assert!(text.contains("OpenAI is now ready to use."));
}

#[test]
fn setup_select_agent_custom_and_current_marker() {
    let mut app = support::make_app();
    app.current_agent_index = 1;
    app.provider_setup = Some(ProviderSetupStep::SelectAgent {
        agents: vec![
            ("general".to_string(), "General helper".to_string(), false),
            ("coder".to_string(), "Codes things".to_string(), true),
        ],
        selected: 1,
    });
    let terminal = render(&mut app, 120, 40);
    let text = painted(&terminal);
    assert!(text.contains("[custom]"));
    assert!(text.contains("Codes things"));
}

#[test]
fn setup_select_configured_provider_and_show_config() {
    let text = render_setup(
        ProviderSetupStep::SelectConfiguredProvider {
            providers: vec![
                config_provider("anthropic", "Anthropic (Claude)"),
                config_provider("openai", "OpenAI (GPT)"),
            ],
            selected: 1,
        },
        120,
        40,
    );
    assert!(text.contains("Switch Provider"));
    assert!(text.contains("OpenAI (GPT)"));

    let text = render_setup(
        ProviderSetupStep::ShowProviderConfig {
            providers: vec![config_provider("openai", "OpenAI (GPT)")],
            selected: 0,
        },
        120,
        40,
    );
    assert!(text.contains("Provider Config"));
    assert!(text.contains("Enter show config"));
}

#[test]
fn setup_reset_provider_active_marker_and_ollama_badge() {
    let mut app = support::make_app();
    app.configured_provider = Some(config_provider("ollama", "Ollama (Local)"));
    app.provider_setup = Some(ProviderSetupStep::ResetProvider { selected: 0 });
    let terminal = render(&mut app, 120, 40);
    let text = painted(&terminal);
    assert!(text.contains("Reset Provider Credentials"));
    assert!(text.contains("[local]"));
}

#[test]
fn setup_gitlab_form_empty_url_and_token_and_error() {
    let text = render_setup(
        ProviderSetupStep::GitLabSetup {
            url_input: String::new(),
            url_cursor: 0,
            token_input: "glpat-0123456789abcdef".to_string(),
            token_cursor: 6,
            active_field: 0,
            error: Some("token rejected".to_string()),
        },
        120,
        40,
    );
    assert!(text.contains("https://gitlab.com"));
    assert!(text.contains("token rejected"));

    let text = render_setup(
        ProviderSetupStep::GitLabSetup {
            url_input: "https://gitlab.example.com".to_string(),
            url_cursor: 5,
            token_input: String::new(),
            token_cursor: 0,
            active_field: 1,
            error: None,
        },
        120,
        40,
    );
    assert!(text.contains("Personal Access Token"));
}

#[test]
fn setup_gitlab_validating() {
    let text = render_setup(
        ProviderSetupStep::GitLabValidating {
            instance_url: "https://gitlab.com".to_string(),
            token: "glpat-secret".to_string(),
        },
        120,
        40,
    );
    assert!(text.contains("Validating token"));
}

#[test]
fn setup_router_with_models_both_pane_focus_and_error() {
    let mut draft = ragent_llm::providers::router_config::RouterConfig::default();
    draft.tiers.insert(
        "SIMPLE".to_string(),
        ragent_llm::providers::router_config::TierConfig {
            models: vec![ragent_llm::providers::router_config::TierEntry {
                provider: "anthropic".to_string(),
                model: "claude-sonnet-4-20250514".to_string(),
            }],
            timeout_ms: None,
        },
    );
    let text = render_setup(
        ProviderSetupStep::SetupRouter {
            providers: vec![config_provider("anthropic", "Anthropic")],
            selected_provider_ids: vec!["anthropic".to_string()],
            selected_provider_index: 0,
            draft_config: draft,
            active_bucket: ragent_llm::providers::router_config::Tier::Simple,
            active_bucket_index: 0,
            left_pane_focused: false,
            error: Some("pick a model".to_string()),
        },
        220,
        50,
    );
    assert!(text.contains("Model Router Setup"));
    assert!(text.contains("Anthropic"));
    assert!(text.contains("pick a model"));
}

#[test]
fn setup_router_empty_providers_left_pane_focused() {
    let text = render_setup(
        ProviderSetupStep::SetupRouter {
            providers: Vec::new(),
            selected_provider_ids: Vec::new(),
            selected_provider_index: 0,
            draft_config: ragent_llm::providers::router_config::RouterConfig::default(),
            active_bucket: ragent_llm::providers::router_config::Tier::Reasoning,
            active_bucket_index: 0,
            left_pane_focused: true,
            error: None,
        },
        120,
        40,
    );
    assert!(text.contains("No concrete providers configured"));
    assert!(text.contains("empty"));
}

#[test]
fn setup_select_router_model_with_and_without_models() {
    let text = render_setup(
        ProviderSetupStep::SelectRouterModel {
            provider_id: "anthropic".to_string(),
            provider_name: "Anthropic".to_string(),
            models: vec![entry("claude", "Claude Sonnet", 200_000)],
            selected: 0,
            target_tier: ragent_llm::providers::router_config::Tier::Simple,
        },
        140,
        40,
    );
    assert!(text.contains("Assign model for Anthropic"));

    let text = render_setup(
        ProviderSetupStep::SelectRouterModel {
            provider_id: "anthropic".to_string(),
            provider_name: "Anthropic".to_string(),
            models: Vec::new(),
            selected: 0,
            target_tier: ragent_llm::providers::router_config::Tier::Complex,
        },
        120,
        40,
    );
    assert!(text.contains("No models are currently available"));
}

#[test]
fn setup_telemetry_both_protocols_error_and_empty_port() {
    let text = render_setup(
        ProviderSetupStep::TelemetrySetup {
            endpoint_field: InputField::with_text("http://localhost:4318"),
            protocol: ragent_config::OtelProtocol::Http,
            interval_field: InputField::with_text("30"),
            timeout_field: InputField::with_text("10"),
            port_field: InputField::new(),
            active_field: 1,
            error: Some("endpoint rejected".to_string()),
        },
        120,
        44,
    );
    assert!(text.contains("[http]"));
    assert!(text.contains("disabled"));
    assert!(text.contains("endpoint rejected"));

    let text = render_setup(
        ProviderSetupStep::TelemetrySetup {
            endpoint_field: InputField::new(),
            protocol: ragent_config::OtelProtocol::Grpc,
            interval_field: InputField::new(),
            timeout_field: InputField::new(),
            port_field: InputField::with_text("9100"),
            active_field: 2,
            error: None,
        },
        120,
        44,
    );
    assert!(text.contains("[grpc]"));
    assert!(text.contains("9100"));
}

#[test]
fn setup_select_azure_resource_entries_and_empty_states() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectAzureResource {
        entries: vec![
            ragent_llm::providers::azure_resource::AzureResourceEntry {
                id: "res-1".to_string(),
                name: "Primary deployment".to_string(),
                endpoint: "https://example.openai.azure.com".to_string(),
                api_key: None,
                api_key_env: Some("AZURE_KEY".to_string()),
                context_window: Some(200_000),
                capabilities: None,
                thinking: None,
                api_type: Some("openai".to_string()),
            },
            ragent_llm::providers::azure_resource::AzureResourceEntry {
                id: "res-2".to_string(),
                name: "Secondary deployment".to_string(),
                endpoint: "https://second.openai.azure.com".to_string(),
                api_key: None,
                api_key_env: None,
                context_window: None,
                capabilities: None,
                thinking: None,
                api_type: None,
            },
        ],
        selected: 1,
        error: None,
    });
    let terminal = render(&mut app, 120, 40);
    let text = painted(&terminal);
    assert!(text.contains("Secondary deployment"));

    let text = render_setup(
        ProviderSetupStep::SelectAzureResource {
            entries: Vec::new(),
            selected: 0,
            error: None,
        },
        120,
        40,
    );
    assert!(text.contains("No resources found"));

    let text = render_setup(
        ProviderSetupStep::SelectAzureResource {
            entries: Vec::new(),
            selected: 0,
            error: Some("azureresources.json missing".to_string()),
        },
        120,
        40,
    );
    assert!(text.contains("azureresources.json missing"));
}

// ===========================================================================
// messages_to_lines / render_output_view_overlay
// ===========================================================================

/// Every message-part and tool-status arm of `messages_to_lines`, driven
/// through the public `render_messages`.
#[test]
fn render_messages_covers_all_part_and_status_arms() {
    let mut app = support::make_app();
    app.session_id = Some("sess-abcdefgh-1234".to_string());
    app.messages
        .push(Message::user_text("s1", "first line\nsecond line"));
    app.messages.push(Message::assistant_text(
        "s1",
        "Agent Notice: switched model",
    ));
    app.messages.push(Message::new(
        "s1",
        Role::Compaction,
        vec![MessagePart::Text {
            text: "## Objective\nsummarised".to_string(),
        }],
    ));
    app.messages.push(Message::new(
        "s1",
        Role::Assistant,
        vec![
            MessagePart::ToolCall {
                tool: "Edit".to_string(),
                call_id: "c-edit".to_string(),
                state: Box::new(ToolCallState {
                    status: ToolCallStatus::Completed,
                    input: json!({"path": "/p/src/main.rs"}),
                    output: Some(json!({"old_lines": 2, "new_lines": 5})),
                    error: None,
                    duration_ms: Some(12),
                }),
            },
            MessagePart::ToolCall {
                tool: "read".to_string(),
                call_id: "c-read".to_string(),
                state: Box::new(ToolCallState {
                    status: ToolCallStatus::Completed,
                    input: json!({"path": "/p/src/lib.rs"}),
                    output: Some(json!({"start_line": 4, "end_line": 9})),
                    error: None,
                    duration_ms: Some(2500),
                }),
            },
            MessagePart::ToolCall {
                tool: "think".to_string(),
                call_id: "c-think".to_string(),
                state: Box::new(ToolCallState {
                    status: ToolCallStatus::Completed,
                    input: json!({}),
                    output: Some(json!({"thought": "a private thought"})),
                    error: None,
                    duration_ms: None,
                }),
            },
            MessagePart::ToolCall {
                tool: "multiedit".to_string(),
                call_id: "c-multi".to_string(),
                state: Box::new(ToolCallState {
                    status: ToolCallStatus::Completed,
                    input: json!({}),
                    output: Some(json!({
                        "file_stats": [
                            {"path": "/p/src/a.rs", "added": 3, "removed": 1},
                            {"path": "/p/src/bb.rs", "added": 1, "removed": 0}
                        ]
                    })),
                    error: None,
                    duration_ms: None,
                }),
            },
            MessagePart::ToolCall {
                tool: "agent_complete".to_string(),
                call_id: "c-done".to_string(),
                state: Box::new(ToolCallState {
                    status: ToolCallStatus::Completed,
                    input: json!({}),
                    output: Some(json!({"summary": "line a\nline b"})),
                    error: None,
                    duration_ms: None,
                }),
            },
            MessagePart::ToolCall {
                tool: "agent_complete".to_string(),
                call_id: "c-done-empty".to_string(),
                state: Box::new(ToolCallState {
                    status: ToolCallStatus::Completed,
                    input: json!({}),
                    output: Some(json!({})),
                    error: None,
                    duration_ms: None,
                }),
            },
            MessagePart::ToolCall {
                tool: "Bash".to_string(),
                call_id: "c-bash".to_string(),
                state: Box::new(ToolCallState {
                    status: ToolCallStatus::Running,
                    input: json!({"command": "ls -la"}),
                    output: None,
                    error: None,
                    duration_ms: None,
                }),
            },
            MessagePart::ToolCall {
                tool: "Write".to_string(),
                call_id: "c-write".to_string(),
                state: Box::new(ToolCallState {
                    status: ToolCallStatus::Pending,
                    input: json!({"path": "/p/src/x.rs"}),
                    output: None,
                    error: None,
                    duration_ms: None,
                }),
            },
            MessagePart::ToolCall {
                tool: "Grep".to_string(),
                call_id: "c-grep".to_string(),
                state: Box::new(ToolCallState {
                    status: ToolCallStatus::Error,
                    input: json!({"pattern": "needle"}),
                    output: None,
                    error: Some("no such directory".to_string()),
                    duration_ms: None,
                }),
            },
            MessagePart::ToolCall {
                tool: "list".to_string(),
                call_id: "c-list".to_string(),
                state: Box::new(ToolCallState {
                    status: ToolCallStatus::Completed,
                    input: json!({"path": "/p"}),
                    output: Some(json!({"entries": ["a.rs"]})),
                    error: None,
                    duration_ms: None,
                }),
            },
            MessagePart::Reasoning {
                text: "reasoning line one\nreasoning line two".to_string(),
                signature: None,
            },
            MessagePart::Image(Box::new(ImageData {
                mime_type: "image/png".to_string(),
                path: PathBuf::from("/scratch/shot.png"),
            })),
        ],
    ));
    let terminal = render(&mut app, 140, 60);
    let text = painted(&terminal);
    assert!(text.contains("first line"));
    assert!(text.contains("Agent Notice"));
    assert!(text.contains("[compaction]"));
    assert!(text.contains("reasoning line one"));
    assert!(text.contains("shot.png"));
}

#[test]
fn output_view_overlay_session_target_with_empty_and_filled_transcripts() {
    let mut app = support::make_app();
    app.session_id = Some("sess-live".to_string());
    app.messages
        .push(Message::assistant_text("sess-live", "live transcript line"));
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: "sess-live".to_string(),
            label: "primary".to_string(),
        },
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: OutputViewLineCache::default(),
    });
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("live transcript line"));

    // Storage-backed target with nothing stored: the "No output yet" line.
    let mut app = support::make_app();
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: "sess-absent".to_string(),
            label: "absent".to_string(),
        },
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: OutputViewLineCache::default(),
    });
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("No output yet"));
}

#[test]
fn output_view_overlay_team_member_resolves_session_and_builds_step_map() {
    let mut app = support::make_app();
    let sid = "sess-teammate-0001-abcd";
    app.storage
        .create_session(sid, ".")
        .expect("create session");
    let mut msg = Message::new(
        sid,
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "read".to_string(),
            call_id: "call-9".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({"path": "/p/src/main.rs"}),
                output: Some(json!({"lines": 2})),
                error: None,
                duration_ms: Some(5),
            }),
        }],
    );
    msg.parts.push(MessagePart::Text {
        text: "teammate output".to_string(),
    });
    app.storage.create_message(&msg).expect("create message");
    // The member row supplies the session id the overlay's target lacks.
    let mut member = TeamMember::new("reviewer", "tm-001", "general");
    member.session_id = Some(sid.to_string());
    member.status = MemberStatus::Working;
    app.team_members.push(member);
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::TeamMember {
            team_name: "alpha".to_string(),
            agent_id: "tm-001".to_string(),
            teammate_name: "reviewer".to_string(),
            session_id: None,
        },
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: OutputViewLineCache::default(),
    });
    let terminal = render(&mut app, 140, 40);
    let text = painted(&terminal);
    assert!(text.contains("teammate output"));
    assert!(text.contains("reviewer"));
}

#[test]
fn output_view_overlay_absent_session_title_and_no_output_line() {
    let mut app = support::make_app();
    // A storage-backed target whose session has no messages at all: the title
    // carries the label and the body carries the placeholder line.
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: "sess-missing".to_string(),
            label: "missing".to_string(),
        },
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: OutputViewLineCache::default(),
    });
    let terminal = render(&mut app, 120, 30);
    assert!(painted(&terminal).contains("Output: missing"));
}

// ===========================================================================
// render_mcp_discover_dialog
// ===========================================================================

#[test]
fn mcp_discover_empty_state() {
    let mut app = support::make_app();
    app.mcp_discover = Some(McpDiscoverState {
        servers: Vec::new(),
        enabled: Vec::new(),
        selected: 0,
        scroll_offset: 0,
        feedback: None,
    });
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("No MCP servers detected"));
}

#[test]
fn mcp_discover_rows_all_sources_and_feedback_variants() {
    use ragent_agent::mcp::discovery::{DiscoveredMcpServer, McpDiscoverySource};
    let servers = vec![
        DiscoveredMcpServer {
            id: "filesystem".to_string(),
            name: "Filesystem".to_string(),
            executable: PathBuf::from("/usr/bin/mcp-fs"),
            args: Vec::new(),
            env: HashMap::new(),
            source: McpDiscoverySource::SystemPath,
            definition_file: None,
        },
        DiscoveredMcpServer {
            id: "npm-server".to_string(),
            name: "NPM Server".to_string(),
            executable: PathBuf::from("/usr/lib/node_modules/mcp"),
            args: Vec::new(),
            env: HashMap::new(),
            source: McpDiscoverySource::NpmGlobal {
                prefix_dir: PathBuf::from("/usr/lib"),
            },
            definition_file: None,
        },
        DiscoveredMcpServer {
            id: "registry-server".to_string(),
            name: "Registry Server".to_string(),
            executable: PathBuf::from("/opt/mcp"),
            args: Vec::new(),
            env: HashMap::new(),
            source: McpDiscoverySource::McpRegistry {
                registry_dir: PathBuf::from("/opt/registry"),
            },
            definition_file: None,
        },
    ];
    let mut app = support::make_app();
    app.mcp_discover = Some(McpDiscoverState {
        servers,
        enabled: vec![true, false, true],
        selected: 2,
        scroll_offset: 0,
        feedback: Some("[ok] enabled filesystem".to_string()),
    });
    let terminal = render(&mut app, 120, 40);
    let text = painted(&terminal);
    assert!(text.contains("filesystem"));
    assert!(text.contains("npm global"));
    assert!(text.contains("MCP registry"));
    assert!(text.contains("enabled filesystem"));

    // Error feedback renders in red; a selected row above the window clamps the
    // scroll offset back to the cursor.
    if let Some(state) = app.mcp_discover.as_mut() {
        state.feedback = Some("could not enable server".to_string());
        state.selected = 0;
        state.scroll_offset = 2;
    }
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("could not enable server"));
}

// ===========================================================================
// render_config_save_picker / render_history_picker
// ===========================================================================

#[test]
fn config_save_picker_with_real_files_and_scrollbar() {
    let dir = scratch_dir("layout_deep_saves");
    let mut paths = Vec::new();
    for i in 0..30 {
        let path = dir.join(format!("ragent.json.2024-01-{i:02}"));
        std::fs::write(&path, "{}").expect("write save file");
        paths.push(path);
    }
    // One missing path exercises the metadata-error branch.
    paths.push(dir.join("does-not-exist.json.2024-02-01"));
    let mut app = support::make_app();
    app.config_save_picker = Some(ConfigSavePickerState {
        entries: paths,
        selected: 30,
        scroll_offset: 0,
        config_dir: dir,
    });
    let terminal = render(&mut app, 120, 40);
    let text = painted(&terminal);
    assert!(text.contains("Saved configurations"));
    assert!(text.contains("does-not-exist"));
}

#[test]
fn history_picker_truncates_long_entries_and_scrolls() {
    let mut entries: Vec<String> = (0..40)
        .map(|i| format!("prompt number {i} {}", "x".repeat(120)))
        .collect();
    entries.push("short prompt".to_string());
    let mut app = support::make_app();
    app.history_picker = Some(HistoryPickerState {
        entries,
        selected: 40,
        scroll_offset: 0,
    });
    let terminal = render(&mut app, 120, 40);
    let text = painted(&terminal);
    assert!(text.contains("History (41 entries)"));
    assert!(text.contains("short prompt"));
}

// ===========================================================================
// render_file_menu
// ===========================================================================

#[test]
fn file_menu_empty_matches_and_directory_titles() {
    let mut app = support::make_app();
    app.input = "@nothing".to_string();
    app.file_menu = Some(FileMenuState {
        matches: Vec::new(),
        selected: 0,
        scroll_offset: 0,
        query: "nothing".to_string(),
        current_dir: None,
    });
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("No matches"));

    // A directory listing with hidden files enabled and a selected directory.
    let mut app = support::make_app();
    app.input = "@/src".to_string();
    app.file_menu_show_hidden = true;
    app.file_menu = Some(FileMenuState {
        matches: vec![
            FileMenuEntry {
                display: "src".to_string(),
                path: PathBuf::from("src"),
                is_dir: true,
            },
            FileMenuEntry {
                display: "src/main.rs".to_string(),
                path: PathBuf::from("src/main.rs"),
                is_dir: false,
            },
        ],
        selected: 0,
        scroll_offset: 0,
        query: "src".to_string(),
        current_dir: Some(PathBuf::from("src")),
    });
    let terminal = render(&mut app, 120, 40);
    let text = painted(&terminal);
    assert!(text.contains("hidden:on"));
    assert!(text.contains("[dir]"));
}

#[test]
fn file_menu_many_matches_and_selected_file() {
    let mut app = support::make_app();
    app.input = "@/".to_string();
    app.file_menu = Some(FileMenuState {
        matches: (0..20)
            .map(|i| FileMenuEntry {
                display: format!("src/very/deeply/nested/path/file{i}.rs"),
                path: PathBuf::from(format!("src/file{i}.rs")),
                is_dir: false,
            })
            .collect(),
        selected: 19,
        scroll_offset: 12,
        query: String::new(),
        current_dir: None,
    });
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("Enter/Tab accept"));
}

// ===========================================================================
// render_log_panel / log_entry_to_lines
// ===========================================================================

#[test]
fn log_panel_covers_levels_agent_ids_and_session_prefixes() {
    let mut app = support::make_app();
    app.show_log = true;
    app.show_profile = false;
    app.sid_to_display_name
        .insert("sid-a".to_string(), "ag001".to_string());
    let levels = [
        LogLevel::Info,
        LogLevel::Tool,
        LogLevel::Warn,
        LogLevel::Error,
    ];
    for (i, level) in levels.iter().enumerate() {
        app.log_entries.push(LogEntry {
            timestamp: chrono::Utc::now(),
            level: *level,
            message: format!("[sid-a:{}] plain body {i}", i + 1),
            session_id: Some("sid-a".to_string()),
            agent_id: Some("tm-001".to_string()),
            seq: i as u64,
        });
    }
    // Compaction highlight, a bracketed message with no colon, and one with no
    // closing bracket at all.
    app.log_entries.push(LogEntry {
        timestamp: chrono::Utc::now(),
        level: LogLevel::Info,
        message: "compaction completed for session".to_string(),
        session_id: None,
        agent_id: None,
        seq: 10,
    });
    app.log_entries.push(LogEntry {
        timestamp: chrono::Utc::now(),
        level: LogLevel::Info,
        message: "[not-a-prefix] body".to_string(),
        session_id: None,
        agent_id: None,
        seq: 11,
    });
    app.log_entries.push(LogEntry {
        timestamp: chrono::Utc::now(),
        level: LogLevel::Warn,
        message: "[sid-a:2 unterminated".to_string(),
        session_id: None,
        agent_id: None,
        seq: 12,
    });
    let terminal = render(&mut app, 120, 40);
    let text = painted(&terminal);
    assert!(text.contains("ag001"));
    assert!(text.contains("CMP"));
    assert!(text.contains("[not-a-prefix] body"));
}

#[test]
fn log_panel_empty_state() {
    let mut app = support::make_app();
    app.show_log = true;
    app.show_profile = false;
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("No log entries yet"));
}

// ===========================================================================
// markdown_to_lines
// ===========================================================================

#[test]
fn markdown_to_lines_covers_code_blocks_headings_links_images() {
    let base = PathBuf::from("/");
    let md = "# H1\n\n## H2\n\n### H3\n\n```rust\nfn main() {}\n```\n\n\
              ```mermaid\ngraph TD; A-->B\n```\n\n    indented code\n\n\
              A [link text](https://example.org/page) inline.\n\n\
              An ![alt text](/does/not/exist.png) image.\n\n\
              Inline `code` and <b>raw html</b>.\n\n---\n\n\
              - one\n- two\n  1. nested\n";
    let lines = ragent_tui::layout::markdown_to_lines_testable(md, &base, 40);
    let rendered: Vec<String> = lines.iter().map(std::string::ToString::to_string).collect();
    let joined = rendered.join("\n");
    assert!(joined.contains("H1"));
    assert!(joined.contains("H2"));
    assert!(joined.contains("H3"));
    assert!(joined.contains("fn main()"));
    assert!(joined.contains("Mermaid diagram"));
    assert!(joined.contains("link text"));
    assert!(joined.contains("example.org/page"));
    assert!(joined.contains("image"));
    assert!(joined.contains("Esc to close"));
}

#[test]
fn research_view_overlay_renders_markdown() {
    let mut app = support::make_app();
    app.research_view = Some(ragent_tui::app::ResearchViewState {
        name: "topic".to_string(),
        path: PathBuf::from("/research/topic/RESEARCH.md"),
        base_dir: PathBuf::from("/research/topic"),
        markdown: "# Findings\n\n- source one\n- source two\n".repeat(6),
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: OutputViewLineCache::default(),
    });
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("Findings"));
}

// ===========================================================================
// render_profile_panel
// ===========================================================================

#[test]
fn profile_panel_disabled_branch_and_content_lines() {
    use ragent_agent::session::profiler::agent_loop_profiler;
    // The profiler is a process-global (the other layout suite toggles it too),
    // so take the binary-wide lock and leave it disabled - flipping it to
    // enabled here would race that suite's own "disabled" assertion.
    let _guard = crate::global_env_lock();
    let profiler = agent_loop_profiler();
    profiler.set_enabled(false);
    let mut app = support::make_app();
    app.show_profile = true;
    app.show_log = false;
    // Another suite in this binary toggles the same process-global profiler, so
    // retry the observation rather than depending on a single racy frame.
    let mut text = String::new();
    for _ in 0..8 {
        profiler.set_enabled(false);
        text = painted(&render(&mut app, 120, 30));
        if text.contains("Profiler is disabled") {
            break;
        }
    }
    assert!(text.contains("Profiler is disabled"));
    assert_eq!(
        app.profile_content_lines,
        ["Profiler is disabled".to_string()]
    );
}

// ===========================================================================
// render_plugins_list_panel
// ===========================================================================

fn plugin_view(with_heading: bool) -> PluginListView {
    PluginListView {
        attribution: "From: /plugins list".to_string(),
        header: with_heading.then(|| "id  version  enabled".to_string()),
        separator: with_heading.then(|| "--- -------- -------".to_string()),
        body: (0..30)
            .map(|i| format!("plugin{i} 1.0.0 yes"))
            .chain(std::iter::once("contributions: 1".to_string()))
            .collect(),
        rows: (0..30)
            .map(|i| PluginListRow {
                id: format!("plugin{i}"),
                enabled: i % 2 == 0,
            })
            .collect(),
    }
}

#[test]
fn plugins_list_panel_with_heading_and_ok_feedback() {
    let mut app = support::make_app();
    app.plugins_list = Some(PluginsListState {
        view: plugin_view(true),
        heading: Some("id  version  enabled".to_string()),
        selected: 28,
        scroll_offset: 0,
        verbose: true,
        feedback: Some("[ok] enabled plugin3".to_string()),
    });
    let terminal = render(&mut app, 160, 40);
    let text = painted(&terminal);
    assert!(text.contains("Installed Plugins"));
    assert!(text.contains("enabled plugin3"));
}

#[test]
fn plugins_list_panel_without_heading_and_error_feedback() {
    let mut app = support::make_app();
    app.plugins_list = Some(PluginsListState {
        view: plugin_view(false),
        heading: None,
        selected: 0,
        scroll_offset: 5,
        verbose: false,
        feedback: Some("could not disable plugin1".to_string()),
    });
    let terminal = render(&mut app, 120, 40);
    let text = painted(&terminal);
    assert!(text.contains("could not disable plugin1"));
    // A scroll offset past a selection clamps back to the cursor.
    assert!(text.contains("plugin0"));
}

// ===========================================================================
// render_loop_setup_dialog
// ===========================================================================

#[test]
fn loop_setup_dialog_with_error_and_checkpoints_off() {
    let mut app = support::make_app();
    ragent_tui::app::open_loop_setup(&mut app);
    if let Some(state) = app.loop_setup.as_mut() {
        state.active_field = ragent_tui::app::LoopSetupField::Goal;
        state.goal_field = InputField::with_text("ship the feature");
        state.checkpoints = false;
        state.error = Some("goal is required".to_string());
    }
    let terminal = render(&mut app, 120, 40);
    let text = painted(&terminal);
    assert!(text.contains("Start a goal-driven loop"));
    assert!(text.contains("ship the feature"));
    assert!(text.contains("goal is required"));
    assert!(text.contains("checkpoints"));
}

#[test]
fn loop_setup_dialog_agent_picker_row() {
    let mut app = support::make_app();
    ragent_tui::app::open_loop_setup(&mut app);
    if let Some(state) = app.loop_setup.as_mut() {
        state.active_field = ragent_tui::app::LoopSetupField::Checkpoints;
        state.checkpoints = true;
    }
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("agent"));
}

// ===========================================================================
// render_connector_catalogue_panel
// ===========================================================================

fn connector_descriptor(id: &str, category: &str) -> ragent_connectors::ConnectorDescriptor {
    ragent_connectors::ConnectorDescriptor {
        id: ragent_connectors::ConnectorId::new(id).expect("valid id"),
        name: id.to_string(),
        description: format!("{id} description"),
        category: category.to_string(),
        tags: Vec::new(),
        source: format!("https://example.org/{id}.json"),
        marketplace: String::new(),
        provenance: Default::default(),
        auth: ragent_connectors::ConnectorAuthShape::None,
        auth_scope: Vec::new(),
        credential: None,
        servers: vec![ragent_connectors::ConnectorServer {
            id: "main".to_string(),
            transport: "stdio".to_string(),
            command: Some("echo".to_string()),
            args: Vec::new(),
            env: Default::default(),
            url: None,
            headers: Default::default(),
        }],
        unsupported: Vec::new(),
    }
}

fn open_catalogue(entries: Vec<ragent_connectors::ConnectorDescriptor>) -> App {
    let mut app = support::make_app();
    let mut browser = ragent_connectors::CatalogueBrowser::new("", false);
    browser.set_entries(entries, 0);
    app.connector_store = Some(browser);
    app
}

#[test]
fn connector_catalogue_loading_and_failed_states() {
    let mut app = support::make_app();
    // `CatalogueBrowser::new` starts in the Loading state.
    app.connector_store = Some(ragent_connectors::CatalogueBrowser::new("", false));
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("loading connector catalogue"));

    let mut app = support::make_app();
    let mut browser = ragent_connectors::CatalogueBrowser::new("", false);
    browser.set_failed("network unreachable".to_string());
    app.connector_store = Some(browser);
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("catalogue failed: network unreachable"));
}

#[test]
fn connector_catalogue_empty_state_with_skipped_entries() {
    let mut app = support::make_app();
    let mut browser = ragent_connectors::CatalogueBrowser::new("", false);
    browser.set_entries(Vec::new(), 3);
    app.connector_store = Some(browser);
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("3 malformed entries skipped"));
}

#[test]
fn connector_catalogue_rows_installed_marker_and_install_notice() {
    let mut app = open_catalogue(vec![
        connector_descriptor("google-drive", "productivity"),
        connector_descriptor("github", "developer"),
    ]);
    if let Some(browser) = app.connector_store.as_mut() {
        browser.set_installed(BTreeSet::from(["google-drive".to_string()]));
        browser.last_install = Some("[ok] installed google-drive".to_string());
        browser.enriching = true;
    }
    let terminal = render(&mut app, 120, 40);
    let text = painted(&terminal);
    assert!(text.contains("google-drive"));
    // The installed marker is the `*` painted in the one-column lane between the
    // cursor gutter and the id column, mirroring the plugin-store rows.
    assert!(
        text.lines().any(|line| line.contains('*')),
        "an installed row carries the lane marker: {text:?}"
    );
    assert!(text.contains("installed google-drive"));
    assert!(text.contains("loading details"));
}

#[test]
fn connector_catalogue_no_match_line() {
    let mut app = open_catalogue(vec![connector_descriptor("github", "developer")]);
    if let Some(browser) = app.connector_store.as_mut() {
        browser.set_query("zzz-no-such-entry".to_string());
    }
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("no matching connectors"));
}

// ===========================================================================
// render_tasks_panel
// ===========================================================================

#[test]
fn tasks_panel_no_tasks_placeholder() {
    let mut app = support::make_app();
    app.show_tasks_panel = true;
    app.show_log = false;
    app.session_id = Some("layout-deep-tasks".to_string());
    app.tasks_cache_dirty = true;
    let terminal = render(&mut app, 120, 40);
    assert!(painted(&terminal).contains("No tasks"));
}

#[test]
fn tasks_panel_rows_statuses_owner_blocked_and_overflow() {
    let mut app = support::make_app();
    app.show_tasks_panel = true;
    app.show_log = false;
    app.session_id = Some("layout-deep-tasks-2".to_string());
    app.storage
        .create_session("layout-deep-tasks-2", ".")
        .expect("create session");
    app.storage
        .create_task(
            "t-1",
            "layout-deep-tasks-2",
            "Completed dependency",
            "",
            "completed",
            None,
            Some("builder"),
            "{}",
            &[],
        )
        .expect("create task");
    app.storage
        .create_task(
            "t-2",
            "layout-deep-tasks-2",
            "Blocked follow-up task",
            "",
            "pending",
            None,
            None,
            "{}",
            &["t-3".to_string()],
        )
        .expect("create task");
    app.storage
        .create_task(
            "t-3",
            "layout-deep-tasks-2",
            "Running task with active form",
            "",
            "in_progress",
            Some("Running task"),
            Some("writer"),
            "{}",
            &[],
        )
        .expect("create task");
    for i in 4..30 {
        app.storage
            .create_task(
                &format!("t-{i}"),
                "layout-deep-tasks-2",
                &format!("Extra task {i} to force the panel scrollbar"),
                "",
                "pending",
                None,
                None,
                "{}",
                &[],
            )
            .expect("create task");
    }
    app.tasks_cache_dirty = true;
    app.tasks_scroll_offset = u16::MAX;
    let terminal = render(&mut app, 160, 40);
    let text = painted(&terminal);
    assert!(text.contains("TASKS"));
    assert!(text.contains("Completed dependency"));
    assert!(text.contains("blocked by"));
    assert!(text.contains("Running task"));
}

// ===========================================================================
// input_lines_with_kb_selection / render_input
// ===========================================================================

#[test]
fn input_kb_selection_multiline_and_queue_counter() {
    let mut app = support::make_app();
    app.input = "hello world\nsecond line here".to_string();
    app.input_cursor = 3;
    app.kb_select_anchor = Some(20);
    app.input_queue.push_back(QueuedInput {
        text: "queued prompt".to_string(),
        image_paths: Vec::new(),
    });
    let terminal = render(&mut app, 40, 20);
    let text = painted(&terminal);
    assert!(text.contains("hello"));
    assert!(text.contains("01> "));
}

#[test]
fn input_empty_placeholder_and_selection_across_wrap() {
    let mut app = support::make_app();
    let terminal = render(&mut app, 60, 20);
    assert!(painted(&terminal).contains("Type @ to mention files"));

    let mut app = support::make_app();
    app.input = "0123456789012345678901234567890123456789".to_string();
    app.input_cursor = 38;
    app.kb_select_anchor = Some(1);
    // A narrow pane forces the selection to span several display rows.
    let terminal = render(&mut app, 60, 20);
    assert!(painted(&terminal).contains("0123456789"));
}

// ===========================================================================
// render_teammate_strip
// ===========================================================================

#[test]
fn teammate_strip_renders_every_status_icon() {
    let mut app = support::make_app();
    app.session_id = Some("s-lead".to_string());
    app.active_team = Some(TeamConfig::new("alpha", "s-lead"));
    let statuses = [
        MemberStatus::Working,
        MemberStatus::Idle,
        MemberStatus::Spawning,
        MemberStatus::Blocked,
        MemberStatus::PlanPending,
        MemberStatus::Suspended,
        MemberStatus::ShuttingDown,
        MemberStatus::Stopped,
        MemberStatus::Failed,
    ];
    for (i, status) in statuses.into_iter().enumerate() {
        let mut member = TeamMember::new(
            format!("teammate-with-a-long-name-{i}"),
            format!("tm-{i:03}"),
            "general",
        );
        member.status = status;
        app.team_members.push(member);
    }
    app.focused_teammate = Some("tm-001".to_string());
    let terminal = render(&mut app, 120, 40);
    let text = painted(&terminal);
    assert!(text.contains("[team]"));
    // The strip clips every name to `NAME_COLUMNS` display columns (FR-027), so
    // the assertion is anchored on the clipped label the strip itself paints
    // rather than on the raw member name. (It previously matched an 18-column
    // substring that only ever appeared in the input-border focus title, which
    // FR-058 no longer paints - the strip never emitted it.)
    let clipped: String = "teammate-with-a-long-name-0"
        .chars()
        .take(ragent_tui::teammate_strip::NAME_COLUMNS)
        .collect();
    assert!(
        text.contains(&clipped),
        "the strip must paint the clipped name {clipped:?}"
    );
}

// ===========================================================================
// layout_teams / layout_active_agents
// ===========================================================================

#[test]
fn teams_window_renders_all_member_statuses_and_buttons() {
    let mut app = support::make_app();
    app.session_id = Some("sess-lead-0001".to_string());
    app.agent_name = "general".to_string();
    app.active_team = Some(TeamConfig::new("alpha", "sess-lead-0001"));
    app.show_teams_window = true;
    app.team_message_counts.insert("tm-001".to_string(), (3, 2));
    app.team_message_counts.insert("lead".to_string(), (1, 1));
    let statuses = [
        MemberStatus::Working,
        MemberStatus::Suspended,
        MemberStatus::Stopped,
        MemberStatus::Failed,
        MemberStatus::PlanPending,
    ];
    for (i, status) in statuses.into_iter().enumerate() {
        let mut member = TeamMember::new(format!("reviewer-{i}"), format!("tm-{i:03}"), "general");
        member.status = status;
        member.session_id = Some(format!("sess-child-{i}"));
        if i == 0 {
            member.model_override = Some(ModelRef {
                provider_id: "anthropic".to_string(),
                model_id: "claude-sonnet-4-20250514".to_string(),
            });
        }
        app.team_members.push(member);
    }
    app.focused_teammate = Some("tm-000".to_string());
    // More members than rows, so the subpanel scrollbar renders.
    for i in 0..20 {
        let mut member = TeamMember::new(format!("filler-{i}"), format!("tm-1{i:02}"), "general");
        member.status = MemberStatus::Idle;
        app.team_members.push(member);
    }
    app.teams_scroll_offset = 1;
    let terminal = render(&mut app, 160, 40);
    let text = painted(&terminal);
    assert!(text.contains("Team: alpha"));
    assert!(text.contains("reviewer-0"));
    assert!(text.contains("anthropic/claude-"));
}

#[test]
fn teams_window_with_no_members_shows_hint() {
    let mut app = support::make_app();
    app.active_team = Some(TeamConfig::new("empty-team", "s1"));
    app.show_teams_window = true;
    let terminal = render(&mut app, 140, 40);
    assert!(painted(&terminal).contains("no teammates yet"));
}

#[test]
fn active_agents_window_covers_nesting_statuses_and_bg_tasks() {
    let mut app = support::make_app();
    app.session_id = Some("sess-primary".to_string());
    app.show_agents_window = true;
    // A primary-agent child and a grandchild exercise the recursion.
    app.active_tasks.push(task_entry(
        "task-running-0001",
        "sess-primary",
        "sess-child-1",
        "general",
        TaskStatus::Running,
        true,
    ));
    app.active_tasks.push(task_entry(
        "task-suspended-0002",
        "sess-child-1",
        "sess-child-2",
        "explore",
        TaskStatus::Suspended,
        false,
    ));
    app.active_tasks.push(task_entry(
        "task-terminating-0003",
        "sess-primary",
        "sess-child-3",
        "general",
        TaskStatus::Terminating,
        true,
    ));
    app.active_tasks.push(task_entry(
        "task-completed-0004",
        "sess-primary",
        "sess-child-4",
        "general",
        TaskStatus::Completed,
        false,
    ));
    // Custom + teammate badges.
    app.custom_agent_defs.push(CustomAgentDef {
        record: OasfAgentRecord {
            name: "general".to_string(),
            description: "custom".to_string(),
            version: "1.0.0".to_string(),
            schema_version: "0.7.0".to_string(),
            authors: Vec::new(),
            created_at: None,
            skills: Vec::new(),
            domains: Vec::new(),
            locators: Vec::new(),
            modules: Vec::new(),
        },
        source_path: PathBuf::from("/agents/general.json"),
        agent_info: Arc::new(AgentInfo::new("general", "custom general")),
        is_project_local: true,
    });
    let mut member = TeamMember::new("reviewer", "tm-001", "general");
    member.session_id = Some("sess-child-1".to_string());
    member.status = MemberStatus::Working;
    app.team_members.push(member);
    app.bg_tasks.push(BgTaskView {
        id: "bg-1".to_string(),
        session_id: "sess-primary".to_string(),
        command: "cargo test --workspace --all-targets".to_string(),
        status: "running".to_string(),
        created_at: chrono::Utc::now(),
        completed_at: None,
    });
    app.bg_tasks.push(BgTaskView {
        id: "bg-2".to_string(),
        session_id: "sess-primary".to_string(),
        command: "ls".to_string(),
        status: "completed".to_string(),
        created_at: chrono::Utc::now(),
        completed_at: Some(chrono::Utc::now()),
    });
    app.bg_tasks.push(BgTaskView {
        id: "bg-3".to_string(),
        session_id: "sess-primary".to_string(),
        command: "false".to_string(),
        status: "failed".to_string(),
        created_at: chrono::Utc::now(),
        completed_at: Some(chrono::Utc::now()),
    });
    let terminal = render(&mut app, 160, 40);
    let text = painted(&terminal);
    assert!(text.contains("Background shell tasks"));
    assert!(text.contains("[C]"));
    assert!(text.contains("[T]"));
    assert!(text.contains("[stop]"));
}

#[test]
fn active_agents_window_scrolled_and_empty() {
    let mut app = support::make_app();
    app.session_id = Some("sess-primary".to_string());
    app.show_agents_window = true;
    for i in 0..30 {
        app.active_tasks.push(task_entry(
            &format!("task-{i:04}"),
            "sess-primary",
            &format!("sess-child-{i}"),
            "general",
            TaskStatus::Running,
            true,
        ));
    }
    app.active_agents_scroll_offset = 4;
    let terminal = render(&mut app, 140, 30);
    assert!(painted(&terminal).contains("general"));
}

// ===========================================================================
// layout_statusbar (through App state permutations)
// ===========================================================================

#[test]
fn status_bar_full_width_with_all_indicators() {
    let mut app = support::make_app();
    app.cwd = "/home/user/projects/ragent".to_string();
    app.git_branch = Some("feature/deep-coverage".to_string());
    app.status = "Processing request".to_string();
    app.last_prompt = "a very long prompt that should be truncated by the status bar".repeat(2);
    app.active_backend = "docker".to_string();
    app.compact_in_progress = true;
    app.compress_in_progress = true;
    app.code_index_busy = true;
    app.code_index_graph_busy = true;
    app.code_index_enabled = true;
    app.autopilot_enabled = true;
    app.quota_percent = Some(96.0);
    app.selected_thinking_level = Some(ThinkingLevel::High);
    app.selected_model = Some("anthropic/claude".to_string());
    let terminal = render(&mut app, 200, 30);
    let text = painted(&terminal);
    assert!(text.contains("ragent"));
    assert!(text.contains("feature/deep"));
    assert!(text.contains("Backend:docker"));
    assert!(text.contains("cmp+compress"));
    assert!(text.contains("96%"));
    assert!(text.contains("cdx-idx"));
    assert!(text.contains("cdx-graph"));
}

#[test]
fn status_bar_backend_segment_is_full_name_in_cyan() {
    let mut app = support::make_app();
    app.cwd = "/home/user/projects/ragent".to_string();
    app.active_backend = "podman".to_string();
    let terminal = render(&mut app, 160, 24);
    let buffer = terminal.backend().buffer();

    // Locate the line-2 cell run that paints "Backend:podman".
    let line2: Vec<String> = buffer
        .content
        .iter()
        .skip(buffer.area.width as usize)
        .take(buffer.area.width as usize)
        .map(|cell| cell.symbol().to_string())
        .collect();
    let line2_text: String = line2.concat();
    let start = line2_text
        .find("Backend:podman")
        .expect("line 2 must name the active backend in full");
    let cells = &buffer.content[buffer.area.width as usize..];
    for idx in start..start + "Backend:podman".len() {
        assert_eq!(
            cells[idx].fg,
            ratatui::style::Color::Cyan,
            "backend label cell {idx} ({:?}) must be cyan",
            cells[idx].symbol()
        );
    }
    assert!(
        !line2_text.contains("B:podman"),
        "the abbreviated label must be gone; line was: {line2_text}"
    );
}

#[test]
fn status_bar_ready_state_web_deadline_and_quota_bands() {
    let mut app = support::make_app();
    app.status = "Ready".to_string();
    let mut progress = ResearchProgress::new("topic", "topic");
    progress.web_phase_deadline =
        Some(std::time::Instant::now() + std::time::Duration::from_secs(125));
    app.research_progress.push(progress);
    app.quota_percent = Some(85.0);
    app.compact_in_progress = true;
    app.active_backend = "local".to_string();
    let terminal = render(&mut app, 160, 30);
    let text = painted(&terminal);
    assert!(text.contains("web:2:"));
    assert!(text.contains("85%"));
    assert!(text.contains("compacting"));

    app.quota_percent = Some(5.0);
    app.compact_in_progress = false;
    app.compress_in_progress = true;
    let terminal = render(&mut app, 160, 30);
    let text = painted(&terminal);
    assert!(text.contains("compressing"));
    assert!(text.contains("5%"));
}

#[test]
fn status_bar_compact_and_minimal_modes() {
    let mut app = support::make_app();
    app.cwd = "/very/long/path/to/a/deeply/nested/project/directory".to_string();
    app.git_branch = Some("main".to_string());
    app.compact_in_progress = true;
    app.selected_thinking_level = Some(ThinkingLevel::Medium);
    app.selected_model = Some("openai/gpt-4o".to_string());
    app.quota_percent = Some(42.0);

    let terminal = render(&mut app, 100, 24);
    let text = painted(&terminal);
    assert!(text.contains("42%"));

    let terminal = render(&mut app, 70, 24);
    let text = painted(&terminal);
    // Minimal mode drops the quota bar and the service indicators.
    assert!(text.contains("42%"));
    assert!(text.contains("ragent"));
}

#[test]
fn status_bar_no_prompt_no_branch_no_model() {
    let mut app = support::make_app();
    app.git_branch = None;
    app.last_prompt = String::new();
    app.selected_model = None;
    app.selected_thinking_level = None;
    app.quota_percent = None;
    let terminal = render(&mut app, 120, 24);
    assert!(painted(&terminal).contains("Ready"));
}

// ===========================================================================
// widgets/dialog.rs
// ===========================================================================

const ALL_VARIANTS: [DialogVariant; 4] = [
    DialogVariant::Info,
    DialogVariant::Warning,
    DialogVariant::Danger,
    DialogVariant::Success,
];

const ALL_SIZES: [DialogSize; 4] = [
    DialogSize::Small,
    DialogSize::Medium,
    DialogSize::Large,
    DialogSize::Custom(50, 50),
];

const ALL_ALIGNMENTS: [DialogAlignment; 3] = [
    DialogAlignment::Center,
    DialogAlignment::Top,
    DialogAlignment::Bottom,
];

#[test]
fn dialog_variants_sizes_and_alignments_render_their_title() {
    for variant in ALL_VARIANTS {
        for size in ALL_SIZES {
            for alignment in ALL_ALIGNMENTS {
                let dialog = Dialog::new("Deep Coverage", variant)
                    .with_size(size)
                    .with_alignment(alignment);
                assert_eq!(dialog.variant, variant);
                assert_eq!(dialog.size, size);
                assert_eq!(dialog.alignment, alignment);
                assert_eq!(dialog.border_color(), dialog.block_style().fg.unwrap());
                assert_eq!(dialog.title_style().fg, Some(dialog.border_color()));

                let area = Rect::new(0, 0, 40, 8);
                let mut buf = Buffer::empty(area);
                dialog
                    .render(area, DialogContent::Text("body text".to_string()))
                    .render(area, &mut buf);
                let text = buffer_text(&buf);
                assert!(
                    text.contains("Deep Coverage"),
                    "title painted for {variant:?}"
                );
                assert!(text.contains("body text"));
            }
        }
    }
}

#[test]
fn dialog_border_colors_match_variant() {
    assert_eq!(
        Dialog::new("i", DialogVariant::Info).border_color(),
        ratatui::style::Color::Cyan
    );
    assert_eq!(
        Dialog::new("w", DialogVariant::Warning).border_color(),
        ratatui::style::Color::Yellow
    );
    assert_eq!(
        Dialog::new("d", DialogVariant::Danger).border_color(),
        ratatui::style::Color::Red
    );
    assert_eq!(
        Dialog::new("s", DialogVariant::Success).border_color(),
        ratatui::style::Color::Green
    );
}

#[test]
fn dialog_content_lines_and_paragraph_variants() {
    let dialog = Dialog::new("Lines Dialog", DialogVariant::Info);
    let area = Rect::new(0, 0, 30, 6);

    let mut buf = Buffer::empty(area);
    dialog
        .render(
            area,
            DialogContent::Lines(vec![Line::from("first"), Line::from("second")]),
        )
        .render(area, &mut buf);
    let text = buffer_text(&buf);
    assert!(text.contains("first"));
    assert!(text.contains("second"));

    let mut buf = Buffer::empty(area);
    dialog
        .render(
            area,
            DialogContent::Paragraph(Box::new(Paragraph::new("paragraph body"))),
        )
        .render(area, &mut buf);
    assert!(buffer_text(&buf).contains("paragraph body"));
}

// ===========================================================================
// widgets/selectable_list.rs
// ===========================================================================

fn text_of(s: &&str) -> String {
    (*s).to_string()
}

#[test]
fn selectable_list_constructors_and_prefixes() {
    let list = SelectableList::new(vec!["a", "b"], text_of);
    assert_eq!(list.selected, 0);
    assert_eq!(list.selected_prefix, ragent_tui::theme::focus::SELECTED);
    assert_eq!(list.unselected_prefix, ragent_tui::theme::focus::UNSELECTED);

    let list = SelectableList::new_focus_list(vec!["a"], text_of);
    assert_eq!(list.selected_prefix, ragent_tui::theme::focus::FOCUSED);

    let list = SelectableList::new(vec!["a"], text_of)
        .with_selected_prefix("-> ")
        .with_unselected_prefix(".. ");
    assert_eq!(list.selected_prefix, "-> ");
    assert_eq!(list.unselected_prefix, ".. ");
}

#[test]
fn selectable_list_navigation_wraps_and_selection_tracks_index() {
    let mut list = SelectableList::new(vec!["a", "b", "c"], text_of);
    assert_eq!(list.selection(), Some(&"a"));
    list.next();
    assert_eq!(list.selection(), Some(&"b"));
    list.next();
    list.next();
    assert_eq!(list.selected, 0);
    list.prev();
    assert_eq!(list.selection(), Some(&"c"));

    list.set_selected(1);
    assert_eq!(list.selection(), Some(&"b"));
    // Out-of-range indices are ignored.
    list.set_selected(99);
    assert_eq!(list.selected, 1);
}

#[test]
fn selectable_list_empty_navigation_is_a_no_op() {
    let mut list: SelectableList<'_, &str> = SelectableList::new(Vec::new(), text_of);
    list.next();
    list.prev();
    assert_eq!(list.selected, 0);
    assert_eq!(list.selection(), None);
    list.set_selected(3);
    assert_eq!(list.selected, 0);
}

#[test]
fn selectable_list_render_paints_prefixes() {
    let list = SelectableList::new(vec!["alpha", "beta", "gamma"], text_of)
        .with_selected_prefix("> ")
        .with_unselected_prefix("  ");
    let area = Rect::new(0, 0, 24, 8);
    let mut buf = Buffer::empty(area);
    SelectableListRender::new(&list, area).render(area, &mut buf);
    let text = buffer_text(&buf);
    assert!(text.contains("alpha"));
    assert!(text.contains("beta"));
    assert!(text.contains("gamma"));
}

#[test]
fn selectable_list_render_empty_list_is_safe() {
    let list: SelectableList<'_, &str> = SelectableList::new(Vec::new(), text_of);
    let area = Rect::new(0, 0, 10, 4);
    let mut buf = Buffer::empty(area);
    SelectableListRender::new(&list, area).render(area, &mut buf);
    let text = buffer_text(&buf);
    assert!(!text.contains("alpha"));
}

// ===========================================================================
// widgets/permission_dialog.rs
// ===========================================================================

fn permission_request() -> PermissionRequest {
    PermissionRequest {
        id: "perm-1".to_string(),
        session_id: "s1".to_string(),
        permission: "bash".to_string(),
        patterns: vec!["rm -rf ./build".to_string()],
        metadata: json!({"timeout_secs": 120u64}),
        tool_call_id: None,
    }
}

#[test]
fn permission_dialog_paints_request_text_and_prompts() {
    let request = permission_request();
    for (width, height) in [(80u16, 40u16), (200, 60), (120, 30)] {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        PermissionDialog::new(&request).render(area, &mut buf);
        let text = buffer_text(&buf);
        assert!(
            text.contains("Permission Required"),
            "title missing at {width}x{height}"
        );
        assert!(
            text.contains("bash"),
            "permission missing at {width}x{height}"
        );
        assert!(text.contains("rm -rf ./build"));
        assert!(text.contains("[y]es"));
        assert!(text.contains("[a]lways"));
        assert!(text.contains("[n]o"));
    }
}

#[test]
fn permission_dialog_zero_and_tiny_areas_do_not_panic() {
    let request = permission_request();
    for (width, height) in [(0u16, 0u16), (1, 1), (3, 2), (10, 3)] {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        PermissionDialog::new(&request).render(area, &mut buf);
    }
}

#[test]
fn permission_dialog_without_patterns_renders_empty_details() {
    let mut request = permission_request();
    request.patterns.clear();
    let area = Rect::new(0, 0, 100, 30);
    let mut buf = Buffer::empty(area);
    PermissionDialog::new(&request).render(area, &mut buf);
    let text = buffer_text(&buf);
    assert!(text.contains("Details:"));
}

#[test]
fn permission_dialog_through_app_queue() {
    let mut app = support::make_app();
    app.permission_queue.push_back(permission_request());
    let terminal = render(&mut app, 120, 50);
    let text = painted(&terminal);
    assert!(text.contains("Permission Required"));
    assert!(text.contains("[y] to allow"));
    assert!(text.contains("rm -rf ./build"));
}

//! Tests for test_slash_commands.rs

//! Tests for TUI slash command parsing and dispatch (TASK-006).
//!
//! Verifies each slash command updates app state correctly, handles arguments,
//! and provides user feedback via status bar and log entries.

use std::sync::{Arc, Mutex, OnceLock};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ragent_core::{
    agent,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor, profiler::agent_loop_profiler},
    storage::Storage,
    tool,
};
use ragent_tui::App;
use ragent_tui::app::{
    ConfiguredProvider, FileMenuEntry, FileMenuState, HistoryPickerState, LogEntry, LogLevel,
    OutputViewState, OutputViewTarget, ProviderSetupStep, ProviderSource, ScreenMode,
};

/// Build an [`App`] backed by an in-memory database.
fn make_app() -> App {
    let event_bus = Arc::new(EventBus::default());
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(vec![])));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let session_processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: provider_registry.clone(),
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_core::config::StreamConfig::default(),
        auto_approve: false,
    });
    let agent_info =
        agent::resolve_agent("general", &Default::default()).expect("resolve general agent");

    App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        agent_info,
        false,
    )
}

struct CwdGuard(std::path::PathBuf);

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

fn enter_temp_config_dir() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_current_dir(temp.path()).expect("set cwd");
    std::fs::create_dir_all(temp.path().join(".ragent")).expect("create .ragent");
    temp
}

fn cwd_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

// ── /clear ──────────────────────────────────────────────────────────

#[test]
fn test_slash_clear_empties_messages() {
    let mut app = make_app();
    // Add some dummy messages
    app.messages
        .push(ragent_core::message::Message::user_text("s1", "hello"));
    app.messages
        .push(ragent_core::message::Message::user_text("s1", "world"));
    assert_eq!(app.messages.len(), 2);

    app.execute_slash_command("/clear");

    assert!(app.messages.is_empty(), "messages should be cleared");
    assert_eq!(app.scroll_offset, 0, "scroll should reset");
    assert_eq!(app.status, "messages cleared");
    // Should log the command start, the action, and the completion.
    assert!(
        app.log_entries.len() >= 2,
        "expected at least start+action logs"
    );
    assert!(app.log_entries[0].message.contains("Executing /clear"));
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("cleared"))
    );
    assert!(
        app.log_entries
            .last()
            .unwrap()
            .message
            .contains("Finished /clear")
    );
}

// ── /help ───────────────────────────────────────────────────────────

#[test]
fn test_slash_help_shows_commands() {
    let mut app = make_app();
    // Set a session so append_assistant_text can push messages
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/help");

    assert_eq!(app.status, "help");
    // Should have created an assistant message with command list
    assert!(!app.messages.is_empty(), "help should create a message");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("/clear"), "help should mention /clear");
    assert!(text.contains("/quit"), "help should mention /quit");
    assert!(text.contains("/system"), "help should mention /system");
    assert!(text.contains("/compact"), "help should mention /compact");
    assert!(text.contains("/agent"), "help should mention /agent");
    assert!(text.contains("/model"), "help should mention /model");
    assert!(
        text.contains("/internal-llm"),
        "help should mention /internal-llm"
    );
    assert!(
        text.contains("/inputdiag"),
        "help should mention /inputdiag"
    );
    assert!(text.contains("/help"), "help should mention /help");
}

#[test]
fn test_slash_help_executes_in_chat_screen() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    // App now starts in Chat mode - home screen has been removed
    assert_eq!(app.current_screen, ScreenMode::Chat);

    app.execute_slash_command("/help");
    // Should remain in Chat mode
    assert_eq!(app.current_screen, ScreenMode::Chat);
}

// ── /quit ───────────────────────────────────────────────────────────

#[test]
fn test_slash_quit_stops_app() {
    let mut app = make_app();
    assert!(app.is_running);

    app.execute_slash_command("/quit");
    assert!(!app.is_running, "app should stop after /quit");
}

#[test]
fn test_slash_exit_stops_app() {
    let mut app = make_app();
    assert!(app.is_running);

    app.execute_slash_command("/exit");
    assert!(!app.is_running, "app should stop after /exit");
}

// ── /system ─────────────────────────────────────────────────────────

#[test]
fn test_slash_system_sets_prompt() {
    let mut app = make_app();
    app.execute_slash_command("/system You are a pirate. Respond in pirate speak.");

    assert_eq!(
        app.agent_info.prompt.as_deref(),
        Some("You are a pirate. Respond in pirate speak.")
    );
    assert_eq!(app.status, "system prompt updated");
    // Should have start/action/finish logs
    assert!(app.log_entries.len() >= 2);
    assert!(app.log_entries[0].message.contains("Executing /system"));
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("System prompt set"))
    );
    assert!(
        app.log_entries
            .last()
            .unwrap()
            .message
            .contains("Finished /system")
    );
}

#[test]
fn test_slash_system_no_args_shows_current() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    let original = app.agent_info.prompt.clone();

    app.execute_slash_command("/system");

    // Should display the current prompt, not change it
    assert_eq!(app.agent_info.prompt, original);
    if original.is_some() {
        assert!(!app.messages.is_empty(), "should show current prompt");
        let text = app.messages.last().unwrap().text_content();
        assert!(text.contains("Current system prompt"));
    }
}

#[test]
fn test_slash_system_replaces_existing() {
    let mut app = make_app();
    app.execute_slash_command("/system First prompt");
    assert_eq!(app.agent_info.prompt.as_deref(), Some("First prompt"));

    app.execute_slash_command("/system Second prompt");
    assert_eq!(app.agent_info.prompt.as_deref(), Some("Second prompt"));
}

#[test]
fn test_slash_internal_llm_toggle_and_feature_switch_persist() {
    let _lock = cwd_test_lock().lock().expect("cwd lock");
    let original_cwd = std::env::current_dir().expect("cwd");
    let _guard = CwdGuard(original_cwd);
    let _temp = enter_temp_config_dir();

    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/internal-llm on");
    assert!(app.internal_llm_config.enabled);
    assert_eq!(app.status, "internal-llm: on");

    app.execute_slash_command("/internal-llm sessiontitle on");
    assert!(app.internal_llm_config.session_title_enabled);
    assert_eq!(app.status, "internal-llm: sessiontitle on");

    let cfg = ragent_core::config::Config::load().expect("load saved config");
    assert!(cfg.internal_llm.enabled);
    assert!(cfg.internal_llm.session_title_enabled);
}

#[test]
fn test_slash_internal_llm_show_displays_feature_switches() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.internal_llm_config.enabled = true;
    app.internal_llm_config.session_title_enabled = true;
    app.internal_llm_config.prompt_context_enabled = false;
    app.internal_llm_config.memory_extraction_enabled = true;

    app.execute_slash_command("/internal-llm show");

    let text = app.messages.last().expect("message").text_content();
    assert!(text.contains("enabled"));
    assert!(text.contains("session title"));
    assert!(text.contains("prompt/context compaction"));
    assert!(text.contains("memory extraction prefilter"));
    assert!(text.contains("off"));
    assert!(text.matches("on").count() >= 3);
}

// ── /agent ──────────────────────────────────────────────────────────

#[test]
fn test_slash_agent_with_name_switches() {
    let mut app = make_app();
    assert_eq!(app.agent_name, "general");

    app.execute_slash_command("/agent ask");

    assert_eq!(app.agent_name, "ask");
    assert_eq!(app.agent_info.name, "ask");
    assert!(app.status.contains("ask"));
}

#[test]
fn test_slash_agent_unknown_name_shows_error() {
    let mut app = make_app();
    app.execute_slash_command("/agent nonexistent");

    assert!(
        app.status.contains("Unknown agent"),
        "status should warn about unknown agent: {}",
        app.status
    );
    assert_eq!(app.agent_name, "general", "should not change agent");
}

#[test]
fn test_slash_agent_no_args_opens_dialog() {
    let mut app = make_app();
    app.execute_slash_command("/agent");

    assert!(
        app.provider_setup.is_some(),
        "should open agent selection dialog"
    );
}

// ── /log ────────────────────────────────────────────────────────────

#[test]
fn test_slash_log_toggles_panel() {
    let mut app = make_app();
    assert!(!app.show_log, "log should be hidden initially");

    app.execute_slash_command("/log");
    assert!(app.show_log, "log should be visible after first toggle");
    assert_eq!(app.status, "log panel visible");

    app.execute_slash_command("/log");
    assert!(!app.show_log, "log should be hidden after second toggle");
    assert_eq!(app.status, "log panel hidden");
}

// ── /profile ────────────────────────────────────────────────────────

#[test]
fn test_slash_profile_on_enables_profiler_panel() {
    agent_loop_profiler().set_enabled(false);

    let mut app = make_app();
    assert!(!app.show_profile, "profile should be hidden initially");

    app.execute_slash_command("/profile on");

    assert!(app.show_profile, "profile should be visible after enabling");
    assert_eq!(app.status, "profile panel visible");

    agent_loop_profiler().set_enabled(false);
}

#[test]
fn test_slash_profile_off_disables_profiler_panel() {
    agent_loop_profiler().set_enabled(true);

    let mut app = make_app();
    app.show_profile = true;

    app.execute_slash_command("/profile off");

    assert!(
        !app.show_profile,
        "profile should be hidden after disabling"
    );
    assert_eq!(app.status, "profile panel hidden");
}

#[test]
fn test_alt_p_toggles_profiler_panel() {
    agent_loop_profiler().set_enabled(false);

    let mut app = make_app();
    assert!(!app.show_profile, "profile should be hidden initially");

    app.handle_key_event(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::ALT));
    assert!(app.show_profile, "profile should be visible after Alt+P");
    assert_eq!(app.status, "profile panel visible");

    app.handle_key_event(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::ALT));
    assert!(
        !app.show_profile,
        "profile should be hidden after second Alt+P"
    );
    assert_eq!(app.status, "profile panel hidden");

    agent_loop_profiler().set_enabled(false);
}

// ── /llmstats ───────────────────────────────────────────────────────

#[test]
fn test_slash_llmstats_shows_average_metrics() {
    let mut app = make_app();
    app.selected_model = Some("openai/gpt-4o".to_string());
    app.llm_request_stats = vec![
        ragent_tui::app::LlmRequestStat {
            model_ref: "openai/gpt-4o".to_string(),
            elapsed_ms: 1000,
            input_tokens: 100,
            output_tokens: 50,
        },
        ragent_tui::app::LlmRequestStat {
            model_ref: "openai/gpt-4o".to_string(),
            elapsed_ms: 500,
            input_tokens: 200,
            output_tokens: 100,
        },
    ];

    app.execute_slash_command("/llmstats");

    assert_eq!(app.status, "llm stats");
    assert!(!app.messages.is_empty(), "llmstats should create a message");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("From: /llmstats"));
    assert!(text.contains("Model: openai/gpt-4o"));
    assert!(text.contains("Samples: 2"));
    assert!(text.contains("Average round-trip"));
    assert!(text.contains("Average prompt parsing"));
    assert!(text.contains("Average output"));
}

#[test]
fn test_slash_llmstats_no_samples_shows_message() {
    let mut app = make_app();
    app.selected_model = Some("openai/gpt-4o".to_string());

    app.execute_slash_command("/llmstats");

    assert_eq!(app.status, "llm stats unavailable");
    assert!(!app.messages.is_empty(), "llmstats should create a message");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("No completed LLM responses yet"));
}

// ── /cost ───────────────────────────────────────────────────────────

#[test]
fn test_slash_cost_shows_estimated_cost() {
    let mut app = make_app();
    app.llm_request_stats = vec![
        ragent_tui::app::LlmRequestStat {
            model_ref: "openai/gpt-4o".to_string(),
            elapsed_ms: 1000,
            input_tokens: 1000,
            output_tokens: 500,
        },
        ragent_tui::app::LlmRequestStat {
            model_ref: "ollama/llama3.2".to_string(),
            elapsed_ms: 750,
            input_tokens: 800,
            output_tokens: 400,
        },
    ];

    app.execute_slash_command("/cost");

    assert_eq!(app.status, "cost summary");
    assert!(!app.messages.is_empty(), "cost should create a message");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("From: /cost"));
    assert!(text.contains("Samples: 2"));
    assert!(text.contains("Total tokens"));
    assert!(text.contains("Estimated cost"));
}

#[test]
fn test_slash_cost_no_samples_shows_message() {
    let mut app = make_app();

    app.execute_slash_command("/cost");

    assert_eq!(app.status, "cost unavailable");
    assert!(!app.messages.is_empty(), "cost should create a message");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("No completed LLM responses yet"));
}

// ── /compact ────────────────────────────────────────────────────────

#[test]
fn test_slash_compact_no_session_shows_warning() {
    let mut app = make_app();
    assert!(app.session_id.is_none());

    app.execute_slash_command("/compact");
    assert!(
        app.status.contains("No messages"),
        "should create session then warn about empty messages: {}",
        app.status
    );
    assert!(app.session_id.is_some(), "session should be created");
}

#[test]
fn test_slash_compact_no_messages_shows_warning() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    assert!(app.messages.is_empty());

    app.execute_slash_command("/compact");
    assert!(
        app.status.contains("No messages"),
        "should warn about empty messages: {}",
        app.status
    );
}

// ── /model ──────────────────────────────────────────────────────────

#[test]
fn test_slash_model_no_provider_shows_warning() {
    let mut app = make_app();
    // No provider configured by default (no env vars in test)
    if app.configured_provider.is_none() {
        app.execute_slash_command("/model");
        assert!(
            app.status.contains("No provider"),
            "should warn about missing provider"
        );
    }
}

#[test]
fn test_slash_model_show_without_selected_model_uses_agent_model() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/model show");

    assert_eq!(app.status, "active model metadata");
    let text = app
        .messages
        .last()
        .expect("metadata message")
        .text_content();
    assert!(text.contains("From: /model show"));
    assert!(text.contains("Model Ref"));
}

#[test]
fn test_slash_model_show_displays_metadata_for_active_model() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.configured_provider = Some(ConfiguredProvider {
        id: "openai".to_string(),
        name: "OpenAI (GPT)".to_string(),
        source: ProviderSource::Database,
    });
    app.selected_model = Some("openai/gpt-4o-mini".to_string());
    app.selected_model_ctx_window = Some(128_000);

    app.execute_slash_command("/model show");

    assert_eq!(app.status, "active model metadata");
    let text = app
        .messages
        .last()
        .expect("metadata message")
        .text_content();
    assert!(text.contains("From: /model show"));
    assert!(text.contains("OpenAI (GPT)"));
    assert!(text.contains("gpt-4o-mini"));
    assert!(text.contains("Context window"));
    assert!(text.contains("Tool use"));
}

#[test]
fn test_slash_model_show_invalid_subcommand_shows_usage() {
    let mut app = make_app();

    app.execute_slash_command("/model nope");

    assert_eq!(app.status, "Usage: /model [show]");
}

// ── /provider ───────────────────────────────────────────────────────

#[test]
fn test_slash_provider_opens_setup() {
    let mut app = make_app();
    app.execute_slash_command("/provider");

    assert!(
        app.provider_setup.is_some(),
        "should open provider setup dialog"
    );
}

#[test]
fn test_slash_provider_selection_updates_displayed_provider() {
    let mut app = make_app();

    // Start with a different provider so we can verify the display updates.
    app.configured_provider = Some(ConfiguredProvider {
        id: "openai".to_string(),
        name: "OpenAI (GPT)".to_string(),
        source: ProviderSource::Database,
    });
    app.selected_model = Some("openai/gpt-4".to_string());

    // Simulate selecting a provider/model via the interactive dialog.
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "ollama".to_string(),
        provider_name: "Ollama (Local)".to_string(),
        models: vec![ragent_tui::app::ModelPickerEntry {
            id: "llama3.2".to_string(),
            name: "Llama 3.2".to_string(),
            context_window: 131_072,
            max_output: None,
            cost_input: 0.0,
            cost_output: 0.0,
            reasoning: false,
            vision: false,
            tool_use: true,
            cost_tier: "Free".to_string(),
            cost_multiplier: "0x".to_string(),
        }],
        selected: 0,
    });

    // Press Enter to confirm the model selection.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(
        app.configured_provider.as_ref().map(|p| p.id.as_str()),
        Some("ollama"),
        "provider should update when a new model is selected"
    );
    assert_eq!(
        app.provider_model_label().as_deref(),
        Some("Ollama (Local) / llama3.2"),
        "provider/model label should reflect the new provider"
    );
}

#[test]
fn test_provider_list_includes_generic_openai() {
    assert!(
        ragent_tui::app::PROVIDER_LIST
            .iter()
            .any(|(id, name)| *id == "generic_openai" && *name == "Generic OpenAI API"),
        "provider list should include Generic OpenAI API"
    );
    assert!(
        ragent_tui::app::PROVIDER_LIST
            .iter()
            .any(|(id, name)| *id == "ollama_cloud" && *name == "Ollama Cloud"),
        "provider list should include Ollama Cloud"
    );
}

#[test]
fn test_model_selector_navigation_wraps_top_and_bottom() {
    let mut app = make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "copilot".to_string(),
        provider_name: "GitHub Copilot".to_string(),
        models: vec![
            ragent_tui::app::ModelPickerEntry {
                id: "m1".to_string(),
                name: "Model 1".to_string(),
                context_window: 128_000,
                max_output: Some(16_384),
                cost_input: 0.0,
                cost_output: 0.0,
                reasoning: false,
                vision: true,
                tool_use: true,
                cost_tier: "Free".to_string(),
                cost_multiplier: "0x".to_string(),
            },
            ragent_tui::app::ModelPickerEntry {
                id: "m2".to_string(),
                name: "Model 2".to_string(),
                context_window: 128_000,
                max_output: Some(16_384),
                cost_input: 0.0,
                cost_output: 0.0,
                reasoning: false,
                vision: true,
                tool_use: true,
                cost_tier: "Free".to_string(),
                cost_multiplier: "0x".to_string(),
            },
            ragent_tui::app::ModelPickerEntry {
                id: "m3".to_string(),
                name: "Model 3".to_string(),
                context_window: 128_000,
                max_output: Some(16_384),
                cost_input: 0.0,
                cost_output: 0.0,
                reasoning: false,
                vision: true,
                tool_use: true,
                cost_tier: "Free".to_string(),
                cost_multiplier: "0x".to_string(),
            },
        ],
        selected: 0,
    });

    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    match app.provider_setup.as_ref().expect("provider setup present") {
        ProviderSetupStep::SelectModel { selected, .. } => assert_eq!(*selected, 2),
        _ => panic!("expected SelectModel state"),
    }

    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    match app.provider_setup.as_ref().expect("provider setup present") {
        ProviderSetupStep::SelectModel { selected, .. } => assert_eq!(*selected, 0),
        _ => panic!("expected SelectModel state"),
    }
}

// ── /provider_reset ─────────────────────────────────────────────────

#[test]
fn test_slash_provider_reset_opens_dialog() {
    let mut app = make_app();
    app.execute_slash_command("/provider_reset");

    assert!(
        app.provider_setup.is_some(),
        "should open provider reset dialog"
    );
}

#[test]
fn test_generic_openai_enter_key_supports_endpoint_field_and_tab_toggle() {
    let mut app = make_app();
    app.provider_setup = Some(ProviderSetupStep::EnterKey {
        provider_id: "generic_openai".to_string(),
        provider_name: "Generic OpenAI API".to_string(),
        key_input: String::new(),
        key_cursor: 0,
        endpoint_input: String::new(),
        endpoint_cursor: 0,
        editing_endpoint: false,
        error: None,
    });

    // Toggle to endpoint field and type URL.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    ragent_tui::input::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
    );
    ragent_tui::input::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
    );

    match app.provider_setup.as_ref().expect("provider setup present") {
        ProviderSetupStep::EnterKey {
            endpoint_input,
            editing_endpoint,
            ..
        } => {
            assert!(*editing_endpoint);
            assert_eq!(endpoint_input, "ht");
        }
        _ => panic!("expected EnterKey"),
    }
}

#[test]
fn test_generic_openai_enter_key_persists_endpoint_setting() {
    let mut app = make_app();
    app.provider_setup = Some(ProviderSetupStep::EnterKey {
        provider_id: "generic_openai".to_string(),
        provider_name: "Generic OpenAI API".to_string(),
        key_input: "test-key".to_string(),
        key_cursor: 8,
        endpoint_input: "http://localhost:11434/v1".to_string(),
        endpoint_cursor: 25,
        editing_endpoint: false,
        error: None,
    });

    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(
        app.storage
            .get_setting("generic_openai_api_base")
            .ok()
            .flatten(),
        Some("http://localhost:11434/v1".to_string())
    );
}

#[test]
fn test_provider_setup_paste_text_into_key_field() {
    let mut app = make_app();
    app.provider_setup = Some(ProviderSetupStep::EnterKey {
        provider_id: "ollama_cloud".to_string(),
        provider_name: "Ollama Cloud".to_string(),
        key_input: String::new(),
        key_cursor: 0,
        endpoint_input: String::new(),
        endpoint_cursor: 0,
        editing_endpoint: false,
        error: None,
    });

    app.paste_text_into_provider_setup("cloud-key");

    match app.provider_setup.as_ref().expect("provider setup present") {
        ProviderSetupStep::EnterKey {
            key_input,
            key_cursor,
            ..
        } => {
            assert_eq!(key_input, "cloud-key");
            assert_eq!(*key_cursor, 9);
        }
        _ => panic!("expected EnterKey"),
    }
}

// ── unknown command ─────────────────────────────────────────────────

#[test]
fn test_slash_unknown_command_shows_error() {
    let mut app = make_app();
    app.execute_slash_command("/foobar");

    assert!(
        app.status.contains("Unknown command"),
        "should show error for unknown command: {}",
        app.status
    );
    assert!(app.status.contains("foobar"));
    // Expect at least start and completion logs plus the warning.
    assert!(app.log_entries.len() >= 2);
    assert!(app.log_entries.iter().any(|e| e.level == LogLevel::Warn));
    assert!(app.log_entries[0].message.contains("Executing /foobar"));
    assert!(
        app.log_entries
            .last()
            .unwrap()
            .message
            .contains("Finished /foobar")
    );
}

// ── input clearing ──────────────────────────────────────────────────

#[test]
fn test_slash_command_clears_input() {
    let mut app = make_app();
    app.input = "/help".to_string();

    app.execute_slash_command(&app.input.clone());
    assert!(
        app.input.is_empty(),
        "input should be cleared after command"
    );
    assert!(app.slash_menu.is_none(), "slash menu should be closed");
}

#[test]
fn test_input_cursor_left_right_and_editing() {
    let mut app = make_app();
    app.input = "abc".to_string();
    app.input_cursor = app.input.chars().count();

    // Move cursor left twice (from end to between 'b' and 'c')
    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));

    assert_eq!(app.input_cursor, 1);

    // Insert a character at the cursor position
    app.handle_key_event(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE));
    assert_eq!(app.input, "aXbc");
    assert_eq!(app.input_cursor, 2);

    // Move to end and delete the inserted character
    app.handle_key_event(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    assert_eq!(app.input_cursor, 4);

    // Backspace at end removes the last character.
    app.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    assert_eq!(app.input, "aXb");
    assert_eq!(app.input_cursor, 3);

    // Move left one position and delete the inserted character.
    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    assert_eq!(app.input_cursor, 2);
    app.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    assert_eq!(app.input, "ab");
    assert_eq!(app.input_cursor, 1);
}

#[test]
fn test_input_editing_handles_unicode_backspace_and_delete() {
    let mut app = make_app();
    app.input = "a💡b".to_string();
    app.input_cursor = app.input.chars().count();

    // Move to between 💡 and b, then backspace removes 💡.
    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    assert_eq!(app.input_cursor, 2);
    app.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    assert_eq!(app.input, "ab");
    assert_eq!(app.input_cursor, 1);

    // Delete at cursor should remove the next character.
    app.input = "a💡b".to_string();
    app.input_cursor = 1; // before 💡
    app.handle_key_event(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
    assert_eq!(app.input, "ab");
    assert_eq!(app.input_cursor, 1);
}

#[test]
fn test_file_menu_mode_editing_respects_midline_cursor() {
    let mut app = make_app();
    app.input = "ab@cd".to_string();
    app.input_cursor = 2; // between 'b' and '@'
    app.file_menu = Some(FileMenuState {
        matches: vec![FileMenuEntry {
            display: "src/main.rs".to_string(),
            path: std::path::PathBuf::from("src/main.rs"),
            is_dir: false,
        }],
        selected: 0,
        scroll_offset: 0,
        query: "src".to_string(),
        current_dir: None,
    });

    app.handle_key_event(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE));
    assert_eq!(app.input, "abX@cd");
    assert_eq!(app.input_cursor, 3);

    app.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    assert_eq!(app.input, "ab@cd");
    assert_eq!(app.input_cursor, 2);
}

#[test]
fn test_history_picker_enter_sets_char_cursor_for_unicode() {
    let mut app = make_app();
    app.history_picker = Some(HistoryPickerState {
        entries: vec!["éé".to_string()],
        selected: 0,
        scroll_offset: 0,
    });

    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(app.history_picker.is_none());
    assert_eq!(app.input, "éé");
    assert_eq!(app.input_cursor, 2);
}

#[test]
fn test_chat_keystrokes_produce_expected_edit_result() {
    // Test that input handling works correctly in chat mode
    let mut chat = make_app();
    // App now starts in Chat mode - home screen has been removed
    assert_eq!(chat.current_screen, ScreenMode::Chat);

    let sequence = vec![
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('💡'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('Z'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
    ];
    for key in sequence {
        chat.handle_key_event(key);
    }

    // Verify the final state
    assert_eq!(chat.input, "aZ");
    assert_eq!(chat.input_cursor, 2);
}

#[test]
fn test_ctrl_word_navigation_and_deletes() {
    let mut app = make_app();
    app.input = "hello world again".to_string();
    app.input_cursor = app.input.chars().count();

    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL));
    assert_eq!(app.input_cursor, "hello world ".chars().count());

    app.handle_key_event(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
    assert_eq!(app.input, "hello again");
    assert_eq!(app.input_cursor, "hello ".chars().count());

    app.handle_key_event(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
    assert_eq!(app.input, "hello ");
    assert_eq!(app.input_cursor, "hello ".chars().count());
}

#[test]
fn test_ctrl_terminal_cursor_movement_bindings() {
    let mut app = make_app();
    app.input = "abcdef".to_string();
    app.input_cursor = 3;

    app.handle_key_event(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL));
    assert_eq!(app.input_cursor, 2);
    app.handle_key_event(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
    assert_eq!(app.input_cursor, 3);

    // Ctrl+A now selects all: anchor → 0, cursor → end.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
    assert_eq!(app.kb_select_anchor, Some(0));
    assert_eq!(app.input_cursor, 6);
    // Ctrl+E moves to end (cursor is already there; clears selection).
    app.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL));
    assert_eq!(app.input_cursor, 6);
}

#[test]
fn test_ctrl_home_end_bindings() {
    let mut app = make_app();
    app.input = "abcdef".to_string();
    app.input_cursor = 3;

    app.handle_key_event(KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL));
    assert_eq!(app.input_cursor, 0);
    app.handle_key_event(KeyEvent::new(KeyCode::End, KeyModifiers::CONTROL));
    assert_eq!(app.input_cursor, 6);
}

#[test]
fn test_file_menu_targets_mention_under_cursor_not_last_mention() {
    let mut app = make_app();
    app.input = "compare @first with @second".to_string();
    let first_cursor = app.input.find("@first").expect("first mention exists") + "@fi".len();
    app.input_cursor = app.input[..first_cursor].chars().count();

    app.project_files_cache = Some(vec![
        std::path::PathBuf::from("first_file.rs"),
        std::path::PathBuf::from("second_file.rs"),
    ]);
    app.project_files_cache_cwd = Some(std::env::current_dir().expect("cwd"));

    app.update_file_menu();
    let menu = app.file_menu.as_ref().expect("file menu should open");
    assert_eq!(menu.query, "first");
    assert!(
        menu.matches
            .iter()
            .any(|e| e.display.contains("first_file.rs"))
    );
}

#[test]
fn test_accept_file_menu_replaces_active_mention_span_only() {
    let mut app = make_app();
    app.input = "compare @first with @second".to_string();
    let first_cursor = app.input.find("@first").expect("first mention exists") + "@first".len();
    app.input_cursor = app.input[..first_cursor].chars().count();

    app.file_menu = Some(FileMenuState {
        matches: vec![FileMenuEntry {
            display: "src/first_match.rs".to_string(),
            path: std::path::PathBuf::from("src/first_match.rs"),
            is_dir: false,
        }],
        selected: 0,
        scroll_offset: 0,
        query: "first".to_string(),
        current_dir: None,
    });

    let closed = app.accept_file_menu_selection();
    assert!(closed);
    assert_eq!(app.input, "compare @src/first_match.rs with @second");
    assert_eq!(
        app.input_cursor,
        "compare @src/first_match.rs".chars().count()
    );
}

#[test]
fn test_file_menu_closes_when_cursor_not_inside_mention() {
    let mut app = make_app();
    app.input = "compare @first with @second".to_string();
    app.input_cursor = 0;
    app.project_files_cache = Some(vec![std::path::PathBuf::from("first.rs")]);

    app.update_file_menu();
    assert!(app.file_menu.is_none());
}

#[test]
fn test_file_menu_mode_supports_cursor_movement_and_delete() {
    let mut app = make_app();
    app.input = "ab@cd".to_string();
    app.input_cursor = 3; // between '@' and 'c'
    app.file_menu = Some(FileMenuState {
        matches: vec![FileMenuEntry {
            display: "src/main.rs".to_string(),
            path: std::path::PathBuf::from("src/main.rs"),
            is_dir: false,
        }],
        selected: 0,
        scroll_offset: 0,
        query: "c".to_string(),
        current_dir: None,
    });

    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    assert_eq!(app.input_cursor, 2);

    app.handle_key_event(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
    assert_eq!(app.input, "abcd");
    assert_eq!(app.input_cursor, 2);
}

#[test]
fn test_file_menu_mode_supports_ctrl_word_actions() {
    let mut app = make_app();
    app.input = "@hello world".to_string();
    app.input_cursor = app.input.chars().count();
    app.file_menu = Some(FileMenuState {
        matches: vec![FileMenuEntry {
            display: "src/main.rs".to_string(),
            path: std::path::PathBuf::from("src/main.rs"),
            is_dir: false,
        }],
        selected: 0,
        scroll_offset: 0,
        query: "world".to_string(),
        current_dir: None,
    });

    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL));
    assert_eq!(app.input_cursor, "@hello ".chars().count());

    app.handle_key_event(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
    assert_eq!(app.input, "@hello ");
    assert_eq!(app.input_cursor, "@hello ".chars().count());
}

#[test]
fn test_file_menu_enter_accepts_without_sending() {
    let mut app = make_app();
    app.input = "@first".to_string();
    app.input_cursor = app.input.chars().count();
    app.file_menu = Some(FileMenuState {
        matches: vec![FileMenuEntry {
            display: "src/first.rs".to_string(),
            path: std::path::PathBuf::from("src/first.rs"),
            is_dir: false,
        }],
        selected: 0,
        scroll_offset: 0,
        query: "first".to_string(),
        current_dir: None,
    });

    let action =
        ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(action.is_none(), "enter should accept mention but not send");
    assert_eq!(app.input, "@src/first.rs");
    assert!(
        app.file_menu.is_none(),
        "menu should close after file acceptance"
    );
}

#[test]
fn test_file_menu_no_matches_stays_open_for_feedback() {
    let mut app = make_app();
    app.input = "@nomatch".to_string();
    app.input_cursor = app.input.chars().count();
    app.project_files_cache = Some(vec![std::path::PathBuf::from("src/first.rs")]);

    app.update_file_menu();
    let menu = app.file_menu.as_ref().expect("menu should stay open");
    assert!(menu.matches.is_empty(), "no matches should be represented");
    assert_eq!(menu.query, "nomatch");
}

#[test]
fn test_slash_browse_refresh_updates_cache_metadata() {
    let mut app = make_app();
    app.project_files_cache = None;
    app.project_files_cache_cwd = None;
    app.project_files_cache_refreshed_at = None;
    app.project_files_cache_count = 0;

    app.execute_slash_command("/browse_refresh");

    assert!(
        app.status.starts_with("browse index refreshed"),
        "status should reflect browse refresh"
    );
    assert!(
        app.project_files_cache.is_some(),
        "cache should be populated"
    );
    assert!(
        app.project_files_cache_cwd.is_some(),
        "cache cwd should be set"
    );
    assert!(
        app.project_files_cache_refreshed_at.is_some(),
        "cache timestamp should be set"
    );
    assert_eq!(
        app.project_files_cache_count,
        app.project_files_cache
            .as_ref()
            .map(|v| v.len())
            .unwrap_or(0)
    );
}

#[test]
fn test_update_file_menu_refreshes_cache_on_cwd_mismatch() {
    let mut app = make_app();
    app.input = "@src".to_string();
    app.input_cursor = app.input.chars().count();
    app.project_files_cache = Some(vec![]);
    app.project_files_cache_cwd = Some(std::path::PathBuf::from("/definitely/not/current"));

    app.update_file_menu();

    let cwd = std::env::current_dir().expect("cwd");
    assert_eq!(app.project_files_cache_cwd, Some(cwd));
    assert!(app.project_files_cache.is_some());
    assert_eq!(
        app.project_files_cache_count,
        app.project_files_cache
            .as_ref()
            .map(|v| v.len())
            .unwrap_or(0)
    );
}

#[test]
fn test_directory_menu_has_back_to_fuzzy_entry() {
    let mut app = make_app();
    app.input = "@src".to_string();
    app.input_cursor = app.input.chars().count();
    app.file_menu = Some(FileMenuState {
        matches: vec![FileMenuEntry {
            display: "src/".to_string(),
            path: std::path::PathBuf::from("src"),
            is_dir: true,
        }],
        selected: 0,
        scroll_offset: 0,
        query: "src".to_string(),
        current_dir: None,
    });
    let _ = app.accept_file_menu_selection();
    let menu = app.file_menu.as_ref().expect("directory menu should open");
    assert_eq!(
        menu.matches.first().map(|e| e.display.as_str()),
        Some("<back to fuzzy>")
    );
}

#[test]
fn test_file_menu_ctrl_backslash_toggles_hidden_filter() {
    let mut app = make_app();
    app.input = "@src".to_string();
    app.input_cursor = app.input.chars().count();
    app.file_menu = Some(FileMenuState {
        matches: vec![FileMenuEntry {
            display: "src/main.rs".to_string(),
            path: std::path::PathBuf::from("src/main.rs"),
            is_dir: false,
        }],
        selected: 0,
        scroll_offset: 0,
        query: "src".to_string(),
        current_dir: Some(std::path::PathBuf::from("src")),
    });

    assert!(!app.file_menu_show_hidden);
    let _ = ragent_tui::input::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('\\'), KeyModifiers::CONTROL),
    );
    assert!(app.file_menu_show_hidden);
    assert!(app.file_menu.is_some());
}

#[test]
fn test_file_menu_down_scrolls_selection_window() {
    let mut app = make_app();
    let mut entries = Vec::new();
    for i in 0..12 {
        entries.push(FileMenuEntry {
            display: format!("src/file_{i}.rs"),
            path: std::path::PathBuf::from(format!("src/file_{i}.rs")),
            is_dir: false,
        });
    }
    app.file_menu = Some(FileMenuState {
        matches: entries,
        selected: 0,
        scroll_offset: 0,
        query: "file".to_string(),
        current_dir: None,
    });

    for _ in 0..9 {
        let _ = ragent_tui::input::handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
        );
    }
    let menu = app.file_menu.as_ref().expect("menu");
    assert_eq!(menu.selected, 9);
    assert!(menu.scroll_offset > 0);
}

#[test]
fn test_slash_inputdiag_reports_input_state() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.input = "abc".to_string();
    app.input_cursor = 2;

    app.execute_slash_command("/inputdiag");

    assert_eq!(app.status, "inputdiag");
    assert!(!app.messages.is_empty());
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("Input diagnostics:"));
    assert!(text.contains("input chars: 0"));
    assert!(text.contains("input cursor: 0"));
    assert!(text.contains("browse cache entries:"));
    assert!(text.contains("browse menu state:"));
}

// ── with leading slash and without ──────────────────────────────────

#[test]
fn test_slash_command_works_without_leading_slash() {
    let mut app = make_app();
    app.execute_slash_command("quit");
    assert!(!app.is_running, "/quit should work without leading slash");
}

#[test]
fn test_keyboard_quit_requires_ctrl_c_then_ctrl_d() {
    let mut app = make_app();
    assert!(app.is_running);

    app.handle_key_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert!(app.is_running, "Ctrl+D alone should not quit");
    assert!(app.status.contains("Ctrl+C first"));

    app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(app.is_running, "Ctrl+C should arm, not quit");
    assert!(app.status.contains("Ctrl+D"));

    app.handle_key_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert!(!app.is_running, "Ctrl+C then Ctrl+D should quit");
}

#[test]
fn test_keyboard_quit_ctrl_c_then_ctrl_c_does_not_exit() {
    let mut app = make_app();
    assert!(app.is_running);

    app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(app.is_running);

    app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(app.is_running, "second Ctrl+C should not exit");
    assert!(app.status.contains("Ctrl+D"));
}

#[test]
fn test_output_view_paging_shortcuts() {
    let mut app = make_app();
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: "s1".to_string(),
            label: "primary".to_string(),
        },
        scroll_offset: 10,
        max_scroll: 50,
    });

    app.handle_key_event(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
    assert_eq!(app.output_view.as_ref().unwrap().scroll_offset, 5);

    app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
    assert_eq!(app.output_view.as_ref().unwrap().scroll_offset, 10);

    app.handle_key_event(KeyEvent::new(KeyCode::PageUp, KeyModifiers::CONTROL));
    assert_eq!(app.output_view.as_ref().unwrap().scroll_offset, 0);

    app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::CONTROL));
    assert_eq!(app.output_view.as_ref().unwrap().scroll_offset, 50);
}

#[test]
fn test_output_view_escape_closes_overlay() {
    let mut app = make_app();
    app.selected_agent_session_id = Some("s1".to_string());
    app.selected_agent_index = Some(1);
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: "s1".to_string(),
            label: "primary".to_string(),
        },
        scroll_offset: 0,
        max_scroll: 0,
    });

    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.output_view.is_none());
    assert!(app.selected_agent_session_id.is_none());
    assert!(app.selected_agent_index.is_none());
}

#[test]
fn test_output_view_team_member_without_session_uses_log_filter() {
    let mut app = make_app();
    app.log_entries.push(LogEntry {
        timestamp: chrono::Utc::now(),
        level: LogLevel::Info,
        message: "📨 [alpha] tm-001 → lead: done".to_string(),
        session_id: None,
        agent_id: None,
    });

    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::TeamMember {
            team_name: "alpha".to_string(),
            agent_id: "tm-001".to_string(),
            teammate_name: "writer".to_string(),
            session_id: None,
        },
        scroll_offset: 0,
        max_scroll: 0,
    });

    app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
    assert!(app.output_view.is_some());
}

// ── /system preserves whitespace ────────────────────────────────────

#[test]
fn test_slash_system_preserves_argument_whitespace() {
    let mut app = make_app();
    app.execute_slash_command("/system   You are   a   helpful   bot  ");

    assert_eq!(
        app.agent_info.prompt.as_deref(),
        Some("You are   a   helpful   bot"),
        "leading/trailing whitespace trimmed, internal preserved"
    );
}

// ── /tools ──────────────────────────────────────────────────────────

#[test]
fn test_slash_tools_lists_visibility_switches() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_core::config::ToolVisibilityConfig::default();

    app.execute_slash_command("/tools");

    assert_eq!(app.status, "tools");
    assert!(!app.messages.is_empty());
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Tool Family Visibility"),
        "should show visibility heading"
    );
    assert!(text.contains("office"), "should list office switch");
    assert!(text.contains("journal"), "should list journal switch");
    assert!(text.contains("github"), "should list github switch");
    assert!(text.contains("codeindex"), "should list codeindex switch");
}

#[test]
fn test_slash_tools_shows_single_switch_state() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_core::config::ToolVisibilityConfig::default();

    app.execute_slash_command("/tools office");

    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("`office` is currently **off**"));
}

#[test]
fn test_slash_tools_help_shows_usage() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_core::config::ToolVisibilityConfig::default();

    app.execute_slash_command("/tools help");

    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("`/tools show`"));
    assert!(text.contains("`/tools help`"));
    assert!(text.contains("`/tools <switch> on|off`"));
    assert!(text.contains("`office`, `journal`, `github`, `gitlab`, `codeindex`"));
}

#[test]
fn test_slash_tools_show_alias_lists_visibility_switches() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_core::config::ToolVisibilityConfig::default();

    app.execute_slash_command("/tools show");

    assert_eq!(app.status, "tools");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("Tool Family Visibility"));
    assert!(text.contains("office"));
    assert!(text.contains("codeindex"));
}

#[test]
fn test_slash_tools_office_on_shows_office_tools() {
    let _lock = cwd_test_lock().lock().expect("cwd lock");
    let original_cwd = std::env::current_dir().expect("cwd");
    let _guard = CwdGuard(original_cwd);
    let _temp = enter_temp_config_dir();

    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_core::config::ToolVisibilityConfig::default();

    let hidden = ragent_core::config::tool_family_names("office")
        .expect("office family")
        .iter()
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();
    app.session_processor.tool_registry.set_hidden(&hidden);
    assert!(
        !app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "office_read")
    );

    app.execute_slash_command("/tools office on");

    assert!(app.tool_visibility.office);
    assert_eq!(app.status, "tools: office on");
    assert!(
        app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "office_read")
    );
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("`office` visibility is now **on**"));
}

#[test]
fn test_slash_tools_creates_session_if_none() {
    let mut app = make_app();
    assert!(app.session_id.is_none());

    app.execute_slash_command("/tools");

    assert!(app.session_id.is_some(), "should create session");
    assert_eq!(app.status, "tools");
    assert!(!app.messages.is_empty());
}

// ── /opt ────────────────────────────────────────────────────────────

#[test]
fn test_slash_opt_help_shows_markdown_table() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/opt help");

    assert_eq!(app.status, "opt help");
    assert!(
        !app.messages.is_empty(),
        "/opt help should produce a message"
    );
    let text = app.messages.last().unwrap().text_content();
    // The table must list at least a few well-known methods
    assert!(text.contains("co_star"), "table should include co_star");
    assert!(text.contains("crispe"), "table should include crispe");
    assert!(text.contains("cot"), "table should include cot");
    assert!(text.contains("draw"), "table should include draw");
    assert!(text.contains("rise"), "table should include rise");
    assert!(text.contains("meta"), "table should include meta");
    assert!(
        text.contains("variational"),
        "table should include variational"
    );
    assert!(text.contains("q_star"), "table should include q_star");
    assert!(text.contains("openai"), "table should include openai");
    assert!(text.contains("claude"), "table should include claude");
    assert!(text.contains("microsoft"), "table should include microsoft");
}

#[test]
fn test_slash_opt_help_stays_in_chat() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    // App now starts in Chat mode - home screen has been removed
    assert_eq!(app.current_screen, ScreenMode::Chat);

    app.execute_slash_command("/opt help");

    // Should remain in Chat mode
    assert_eq!(app.current_screen, ScreenMode::Chat);
}

#[test]
fn test_slash_opt_no_args_shows_help() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    // /opt with no args falls through to the help branch
    app.execute_slash_command("/opt");

    assert_eq!(app.status, "opt help");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("co_star"));
}

#[tokio::test]
async fn test_slash_opt_co_star_formats_prompt() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    // Provide a configured model so the command proceeds past the guard.
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt co_star Explain async/await in Rust");

    // With LLM integration the command is async: status shows "optimizing" immediately,
    // and no message is appended until the background task completes.
    assert!(
        app.status.contains("⏳") && app.status.contains("co_star"),
        "status should show optimizing with method name: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_opt_crispe_formats_prompt() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt crispe Write a blog post intro");

    assert!(
        app.status.contains("⏳") && app.status.contains("crispe"),
        "status should show optimizing with method name: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_opt_cot_formats_prompt() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt cot Solve the fizzbuzz problem");

    // "cot" is an alias for ChainOfThought whose canonical name is "cot".
    assert!(
        app.status.contains("⏳") && app.status.contains("cot"),
        "status should show optimizing: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_opt_draw_formats_prompt() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt draw A futuristic cityscape at sunset");

    assert!(
        app.status.contains("⏳") && app.status.contains("draw"),
        "status should show optimizing: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_opt_rise_formats_prompt() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt rise Summarise this article");

    assert!(
        app.status.contains("⏳") && app.status.contains("rise"),
        "status should show optimizing: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_opt_meta_formats_prompt() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt meta Generate a test suite");

    assert!(
        app.status.contains("⏳") && app.status.contains("meta"),
        "status should show optimizing: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_opt_variational_formats_prompt() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt variational Write a product description");

    assert!(
        app.status.contains("⏳") && app.status.contains("variational"),
        "status should show optimizing: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_opt_qstar_alias_works() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt qstar What is Rust ownership?");

    // "qstar" alias resolves to canonical name "q_star".
    assert!(
        app.status.contains("⏳") && app.status.contains("q_star"),
        "qstar alias should resolve to canonical q_star: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_opt_openai_formats_prompt() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt openai Translate text to French");

    assert!(
        app.status.contains("⏳") && app.status.contains("openai"),
        "status should show optimizing: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_opt_claude_formats_prompt() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt claude Summarise this meeting transcript");

    assert!(
        app.status.contains("⏳") && app.status.contains("claude"),
        "status should show optimizing: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_opt_microsoft_alias_azure() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt azure Classify this support ticket");

    // "azure" alias resolves to canonical name "microsoft".
    assert!(
        app.status.contains("⏳") && app.status.contains("microsoft"),
        "azure alias should resolve to canonical microsoft: {}",
        app.status
    );
}

#[test]
fn test_slash_opt_unknown_method_shows_warning() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/opt nonexistent Some prompt text");

    assert!(
        app.status.contains("Unknown optimization method"),
        "status should warn about unknown method: {}",
        app.status
    );
    // No new message should appear for an unknown method
    assert!(
        app.messages.is_empty(),
        "unknown method should not produce a message"
    );
}

#[test]
fn test_slash_opt_missing_prompt_shows_warning() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/opt co_star");

    assert!(
        app.status.contains("Please provide a prompt"),
        "status should ask for prompt: {}",
        app.status
    );
    assert!(
        app.messages.is_empty(),
        "missing prompt should not produce a message"
    );
}

#[test]
fn test_slash_opt_is_listed_in_help() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/help");

    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("/opt"), "/help should mention /opt");
}

#[test]
fn test_slash_opt_adds_to_input_history() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/opt help");

    assert!(
        app.input_history.iter().any(|h| h.starts_with("/opt")),
        "input history should include the /opt command"
    );
}

#[tokio::test]
async fn test_slash_opt_o1_alias_works() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt o1 Write a creative short story");

    // "o1" alias resolves to canonical name "o1_style".
    assert!(
        app.status.contains("⏳") && app.status.contains("o1_style"),
        "o1 alias should resolve to canonical o1_style: {}",
        app.status
    );
}

#[test]
fn test_slash_opt_no_model_shows_warning() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    // No selected_model — should produce a friendly error.

    app.execute_slash_command("/opt co_star Explain async/await in Rust");

    assert!(
        app.status.contains("requires a configured model"),
        "status should warn when no model is configured: {}",
        app.status
    );
    assert!(
        app.messages.is_empty(),
        "no message should be added when no model configured"
    );
}

#[test]
fn test_slash_webapi_help_shows_endpoints() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/webapi help");

    assert!(!app.messages.is_empty(), "help should produce a message");
    let last = app.messages.last().unwrap();
    let text = format!("{:?}", last);
    assert!(
        text.contains("health") || text.contains("sessions"),
        "help output should list API endpoints"
    );
}

#[test]
fn test_slash_webapi_disable_when_not_running() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/webapi disable");

    let last = app.messages.last().unwrap();
    let text = format!("{:?}", last);
    assert!(
        text.contains("not running") || text.contains("Disabled"),
        "should report server not running"
    );
}

#[tokio::test]
async fn test_slash_webapi_enable_sets_token() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    assert!(
        app.webapi_token.is_none(),
        "token should be None before enabling"
    );

    app.execute_slash_command("/webapi enable");

    assert!(
        app.webapi_token.is_some(),
        "token should be set after /webapi enable"
    );
    assert!(app.webapi_server.is_some(), "server handle should be set");

    // Clean up
    if let Some(h) = app.webapi_server.take() {
        h.abort();
    }
}

//! Tests for TUI slash command parsing and dispatch (TASK-006).
//!
//! Verifies each slash command updates app state correctly, handles arguments,
//! and provides user feedback via status bar and log entries.

use std::sync::Arc;

use ragent_core::{
    agent,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ragent_tui::App;
use ragent_tui::app::{ConfiguredProvider, LogLevel, ProviderSetupStep, ProviderSource, ScreenMode};

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
        lsp_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
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
    assert_eq!(app.log_entries.len(), 1);
    assert!(app.log_entries[0].message.contains("cleared"));
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
    assert!(text.contains("/help"), "help should mention /help");
}

#[test]
fn test_slash_help_switches_to_chat_screen() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    assert_eq!(app.current_screen, ScreenMode::Home);

    app.execute_slash_command("/help");
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
    assert_eq!(app.log_entries.len(), 1);
    assert!(app.log_entries[0].message.contains("System prompt set"));
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

// ── /compact ────────────────────────────────────────────────────────

#[test]
fn test_slash_compact_no_session_shows_warning() {
    let mut app = make_app();
    assert!(app.session_id.is_none());

    app.execute_slash_command("/compact");
    assert!(
        app.status.contains("No active session"),
        "should warn about missing session: {}",
        app.status
    );
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
        models: vec![("llama3.2".to_string(), "Llama 3.2".to_string())],
        selected: 0,
    });

    // Press Enter to confirm the model selection.
    ragent_tui::input::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    );

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
fn test_model_selector_navigation_wraps_top_and_bottom() {
    let mut app = make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "copilot".to_string(),
        provider_name: "GitHub Copilot".to_string(),
        models: vec![
            ("m1".to_string(), "Model 1".to_string()),
            ("m2".to_string(), "Model 2".to_string()),
            ("m3".to_string(), "Model 3".to_string()),
        ],
        selected: 0,
    });

    ragent_tui::input::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
    );
    match app.provider_setup.as_ref().expect("provider setup present") {
        ProviderSetupStep::SelectModel { selected, .. } => assert_eq!(*selected, 2),
        _ => panic!("expected SelectModel state"),
    }

    ragent_tui::input::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
    );
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
    assert_eq!(app.log_entries.len(), 1);
    assert_eq!(app.log_entries[0].level, LogLevel::Warn);
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

// ── with leading slash and without ──────────────────────────────────

#[test]
fn test_slash_command_works_without_leading_slash() {
    let mut app = make_app();
    app.execute_slash_command("quit");
    assert!(!app.is_running, "/quit should work without leading slash");
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
fn test_slash_tools_lists_builtin_tools() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/tools");

    assert_eq!(app.status, "tools");
    assert!(!app.messages.is_empty());
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Built-in Tools:"),
        "should have built-in heading"
    );
    assert!(text.contains("read"), "should list 'read' tool");
    assert!(text.contains("bash"), "should list 'bash' tool");
    assert!(text.contains("edit"), "should list 'edit' tool");
    assert!(text.contains("grep"), "should list 'grep' tool");
}

#[test]
fn test_slash_tools_shows_no_mcp_when_empty() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/tools");

    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("MCP Tools:"), "should have MCP heading");
    assert!(
        text.contains("no MCP servers connected"),
        "should indicate no MCP servers"
    );
}

#[test]
fn test_slash_tools_shows_mcp_tools() {
    use ragent_core::mcp::{McpServer, McpStatus, McpToolDef};

    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.mcp_servers = vec![McpServer {
        id: "github".to_string(),
        config: Default::default(),
        status: McpStatus::Connected,
        tools: vec![McpToolDef {
            name: "search_repos".to_string(),
            description: "Search GitHub repositories".to_string(),
            parameters: serde_json::json!({}),
        }],
    }];

    app.execute_slash_command("/tools");

    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("search_repos"), "should list MCP tool name");
    assert!(text.contains("[github]"), "should show MCP server name");
    assert!(
        text.contains("Search GitHub repositories"),
        "should show MCP tool description"
    );
    assert!(
        !text.contains("no MCP servers connected"),
        "should not show empty message"
    );
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

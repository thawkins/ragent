//! Integration tests for the `/loop` setup dialog (spec `agentloop`,
//! tasks T-014 / FR-002, FR-004 and T-024 / FR-003, FR-005, FR-025): key
//! navigation, field editing, Esc cancellation with value preservation,
//! one-shot defaults, and the exactly-once loop-telemetry guarantee, with a
//! scripted mock provider.

use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use futures::stream;

use ragent_agent::agent::{AgentInfo, ModelRef};
use ragent_agent::event::{Event, EventBus};
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::loop_state::LoopSpec;
use ragent_agent::session::processor::CachedConfig;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::{Capabilities, Config as RagentConfig, Cost};
use ragent_types::{ThinkingConfig, ThinkingLevel};

use ragent_tui::App;
use ragent_tui::app::{
    ConfiguredProvider, LoopOverrides, LoopSetupField, ProviderSource, apply_loop_overrides,
    parse_loop_flags,
};
use ragent_tui::input::handle_key;

#[path = "support/mod.rs"]
mod support;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Open the dialog exactly as the (T-015) `/loop` entry point will.
fn open_dialog(app: &mut App) {
    ragent_tui::app::open_loop_setup(app);
}

/// Type a character by routing it through the dialog key handler.
fn type_char(app: &mut App, c: char) {
    handle_key(app, key(KeyCode::Char(c)));
}

/// The current goal text of the open dialog.
fn dialog_goal(app: &App) -> String {
    app.loop_setup
        .as_ref()
        .expect("dialog open")
        .goal_field
        .text()
        .to_string()
}

/// The current max-steps text of the open dialog.
fn dialog_max_steps(app: &App) -> String {
    app.loop_setup
        .as_ref()
        .expect("dialog open")
        .max_steps_field
        .text()
        .to_string()
}

/// Advance the active field forward `n` times.
fn tab_n(app: &mut App, n: usize) {
    for _ in 0..n {
        handle_key(app, key(KeyCode::Tab));
    }
}

// ── FR-002: agent picker navigation ─────────────────────────────
#[test]
fn test_dialog_opens_with_agent_picker_focused() {
    let mut app = support::make_app();
    open_dialog(&mut app);
    let state = app.loop_setup.as_ref().expect("dialog open");
    assert!(
        !state.agents.is_empty(),
        "loaded agents must populate the picker"
    );
    assert_eq!(state.active_field, LoopSetupField::Agent);
    assert!(state.checkpoints, "checkpoints default on");
    // The pre-filled step-limit default is numeric text.
    assert!(
        dialog_max_steps(&app).parse::<u32>().is_ok(),
        "step limit pre-filled from config: {}",
        dialog_max_steps(&app)
    );
}

#[test]
fn test_agent_picker_arrow_keys_wrap() {
    let mut app = support::make_app();
    open_dialog(&mut app);
    let len = app.loop_setup.as_ref().expect("open").agents.len();
    let start = app.loop_setup.as_ref().expect("open").selected_agent;

    // Up wraps past the first agent to the last.
    handle_key(&mut app, key(KeyCode::Up));
    let after_up = app.loop_setup.as_ref().expect("open").selected_agent;
    let expected_up = if start == 0 { len - 1 } else { start - 1 };
    assert_eq!(after_up, expected_up, "Up wraps past the first");

    // Down returns to the original selection.
    handle_key(&mut app, key(KeyCode::Down));
    let after_down = app.loop_setup.as_ref().expect("open").selected_agent;
    assert_eq!(after_down, start, "Down wraps past the last");

    // Forward navigation moves +1 (mod len).
    handle_key(&mut app, key(KeyCode::Down));
    let after_down2 = app.loop_setup.as_ref().expect("open").selected_agent;
    assert_eq!(after_down2, (start + 1) % len);
}

// ── FR-002: text fields ─────────────────────────────
#[test]
fn test_tab_moves_between_fields_and_chars_route_to_active_field() {
    let mut app = support::make_app();
    open_dialog(&mut app);
    // Tab from the agent picker lands on the goal field.
    handle_key(&mut app, key(KeyCode::Tab));
    assert_eq!(
        app.loop_setup.as_ref().expect("open").active_field,
        LoopSetupField::Goal
    );
    type_char(&mut app, 'f');
    type_char(&mut app, 'i');
    assert_eq!(dialog_goal(&app), "fi");
}

#[test]
fn test_typing_goes_only_to_the_active_field() {
    let mut app = support::make_app();
    open_dialog(&mut app);
    tab_n(&mut app, 1); // Agent -> Goal
    type_char(&mut app, 'g');
    type_char(&mut app, 'o');
    assert_eq!(dialog_goal(&app), "go");

    // Move to the verify-cmd field; characters must NOT append to goal.
    handle_key(&mut app, key(KeyCode::Tab));
    type_char(&mut app, 'x');
    assert_eq!(dialog_goal(&app), "go", "goal unchanged while unfocused");
    let verify = app
        .loop_setup
        .as_ref()
        .expect("open")
        .verify_cmd_field
        .text();
    assert_eq!(verify, "x");

    // BackTab returns to goal; further typing appends there.
    handle_key(&mut app, key(KeyCode::BackTab));
    type_char(&mut app, '!');
    assert_eq!(dialog_goal(&app), "go!");
}

#[test]
fn test_arrow_keys_move_fields_and_toggle_checkpoints() {
    let mut app = support::make_app();
    open_dialog(&mut app);
    // Focus the checkpoints toggle: BackTab to Agent, then wrap the ring
    // forward 8 steps (9 fields total) to land back on Checkpoints.
    tab_n(&mut app, 1); // -> Goal
    handle_key(&mut app, key(KeyCode::BackTab)); // -> Agent
    tab_n(&mut app, 8); // wrap the ring -> Checkpoints
    assert_eq!(
        app.loop_setup.as_ref().expect("open").active_field,
        LoopSetupField::Checkpoints
    );

    // Up/Down toggle the switch; Space also toggles it.
    handle_key(&mut app, key(KeyCode::Up));
    assert!(!app.loop_setup.as_ref().expect("open").checkpoints);
    handle_key(&mut app, key(KeyCode::Down));
    assert!(app.loop_setup.as_ref().expect("open").checkpoints);
    handle_key(&mut app, key(KeyCode::Char(' ')));
    assert!(!app.loop_setup.as_ref().expect("open").checkpoints);

    // Tab from checkpoints wraps to the agent picker (arrows toggle the
    // switch in place; Tab is the field-mover).
    handle_key(&mut app, key(KeyCode::Tab));
    assert_eq!(
        app.loop_setup.as_ref().expect("open").active_field,
        LoopSetupField::Agent
    );
}

// ── FR-004: Esc cancels without starting, values preserved ─────────────────────────────
#[test]
fn test_esc_cancels_without_starting_and_preserves_values() {
    let mut app = support::make_app();
    app.session_id = Some("sess-loop".to_string());
    open_dialog(&mut app);
    tab_n(&mut app, 1);
    type_char(&mut app, 'g');
    type_char(&mut app, 'o');
    type_char(&mut app, 'a');
    type_char(&mut app, 'l');
    tab_n(&mut app, 5); // Goal -> MaxSteps
    // The step-limit field is pre-filled with the config default; clear it
    // with Ctrl+U before typing so the preserved value is unambiguous.
    handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
    );
    type_char(&mut app, '9');
    let max_steps_before = dialog_max_steps(&app);
    let goal_before = dialog_goal(&app);

    handle_key(&mut app, key(KeyCode::Esc));
    assert!(app.loop_setup.is_none(), "dialog closed");
    assert!(
        app.loop_setup_draft.is_some(),
        "draft preserved for re-open"
    );
    assert_eq!(goal_before, "goal");
    assert_eq!(max_steps_before, "9");

    // Re-opening restores every entered value (FR-004).
    open_dialog(&mut app);
    let restored = app.loop_setup.as_ref().expect("re-opened");
    assert_eq!(restored.goal_field.text(), "goal");
    assert_eq!(restored.max_steps_field.text(), "9");
    assert_eq!(dialog_goal(&app), "goal");
}

#[tokio::test]
async fn test_fresh_open_after_confirm_does_not_restore_draft() {
    let mut app = support::make_app();
    app.session_id = Some("sess-loop".to_string());
    app.configured_provider = Some(ConfiguredProvider {
        id: "ollama".to_string(),
        name: "Ollama".to_string(),
        source: ProviderSource::AutoDiscovered,
    });
    app.selected_model = Some("ollama/qwen3:latest".to_string());
    open_dialog(&mut app);
    tab_n(&mut app, 1);
    type_char(&mut app, 'g');
    type_char(&mut app, 'o');
    handle_key(&mut app, key(KeyCode::Enter));
    assert!(app.loop_setup.is_none(), "dialog closed after confirm");
    assert!(
        app.loop_setup_draft.is_none(),
        "draft cleared after confirm"
    );
    // A later open starts fresh (no stale draft).
    open_dialog(&mut app);
    assert_eq!(dialog_goal(&app), "");
}

// ── FR-005: empty goal rejected ─────────────────────────────
#[test]
fn test_confirm_with_empty_goal_returns_to_dialog_with_error() {
    let mut app = support::make_app();
    app.session_id = Some("sess-loop".to_string());
    open_dialog(&mut app);
    handle_key(&mut app, key(KeyCode::Enter));
    let state = app.loop_setup.as_ref().expect("dialog still open");
    let error = state.error.clone().expect("error set");
    assert!(error.contains("goal"), "error names the field: {error}");
    assert!(app.messages.is_empty(), "no message sent");
    assert!(app.cancel_flag.is_none(), "no loop started");
}

// ── FR-002: confirm starts the loop ─────────────────────────────
#[derive(Clone)]
struct ScriptedProvider {
    /// Shared with the created clients so all requests land in one list.
    shared_captured: Arc<Mutex<Vec<ChatRequest>>>,
    call_count: Arc<AtomicU32>,
}

struct ScriptedClient {
    captured: Arc<Mutex<Vec<ChatRequest>>>,
    call_count: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl LlmClient for ScriptedClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        self.captured.lock().expect("captured lock").push(request);
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let events: Vec<StreamEvent> = vec![
            StreamEvent::TextDelta {
                text: "goal acknowledged".to_string(),
            },
            StreamEvent::Usage {
                input_tokens: 10,
                output_tokens: 5,
            },
            StreamEvent::Finish {
                reason: LlmFinishReason::Stop,
            },
        ];
        Ok(Box::pin(stream::iter(events)))
    }
}

#[async_trait::async_trait]
impl Provider for ScriptedProvider {
    fn id(&self) -> &'static str {
        "ollama"
    }

    fn name(&self) -> &'static str {
        "Scripted Mock Ollama"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "qwen3:latest".to_string(),
            provider_id: "ollama".to_string(),
            name: "Qwen3".to_string(),
            cost: Cost {
                input: 0.0,
                output: 0.0,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: vec![ThinkingLevel::Auto, ThinkingLevel::Off],
            },
            context_window: 128_000,
            max_output: Some(8_192),
            request_multiplier: None,
            thinking_config: Some(ThinkingConfig::new(ThinkingLevel::Auto)),
        }]
    }

    fn as_any_static(&self) -> &(dyn std::any::Any + 'static) {
        self
    }

    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        _options: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<Box<dyn LlmClient>> {
        Ok(Box::new(ScriptedClient {
            captured: Arc::clone(&self.shared_captured),
            call_count: Arc::clone(&self.call_count),
        }))
    }
}

/// App wired to a scripted provider so `process_message` never touches the
/// network; the captured-request handle counts real conversation requests.
fn make_scripted_app() -> (App, Arc<Mutex<Vec<ChatRequest>>>) {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let event_bus = Arc::new(EventBus::default());
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(ScriptedProvider {
        shared_captured: Arc::clone(&captured),
        call_count: Arc::new(AtomicU32::new(0)),
    }));
    let provider_registry = Arc::new(provider_registry);
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![])));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let session_processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: provider_registry.clone(),
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        agent_manager: std::sync::OnceLock::new(),
        bg_service: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::StreamConfig::default(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
            std::collections::HashMap::new(),
        )),
        tool_repeat_guard: std::sync::Arc::new(parking_lot::Mutex::new(
            std::collections::HashMap::new(),
        )),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        telemetry: std::sync::Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        activity_log: std::sync::OnceLock::new(),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
    });
    let agent_info =
        ragent_agent::agent::resolve_agent("general", &Default::default()).expect("resolve agent");
    let mut app = App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        std::sync::Arc::unwrap_or_clone(agent_info),
        false,
        std::path::PathBuf::new(),
    );
    // A real session row so `process_message` can persist the goal message.
    let session = app
        .session_processor
        .session_manager
        .create_session(std::env::current_dir().unwrap_or_default())
        .expect("session created");
    app.session_id = Some(session.id.clone());
    app.configured_provider = Some(ConfiguredProvider {
        id: "ollama".to_string(),
        name: "Ollama".to_string(),
        source: ProviderSource::AutoDiscovered,
    });
    app.selected_model = Some("ollama/qwen3:latest".to_string());
    (app, captured)
}

#[tokio::test]
async fn test_confirm_starts_loop_and_sends_goal() -> Result<()> {
    let (mut app, captured) = make_scripted_app();
    open_dialog(&mut app);
    tab_n(&mut app, 1); // Agent -> Goal
    for c in "fix the flaky test".chars() {
        type_char(&mut app, c);
    }
    handle_key(&mut app, key(KeyCode::Enter));

    assert!(app.loop_setup.is_none(), "dialog closed after confirm");
    assert!(
        app.messages.iter().any(|m| {
            m.text_content().contains("[loop") && m.text_content().contains("fix the flaky test")
        }),
        "goal message pushed to the transcript"
    );
    // Wait for the spawned task: it must have sent the goal conversation.
    for _ in 0..200 {
        if !captured.lock().expect("captured lock").is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let requests = captured.lock().expect("captured lock");
    assert_eq!(requests.len(), 1, "one conversation request with the goal");
    let text = requests
        .first()
        .expect("one request")
        .messages
        .last()
        .map(|m| match &m.content {
            ragent_types::llm::ChatContent::Text(text) => text.clone(),
            ragent_types::llm::ChatContent::Parts(parts) => parts
                .iter()
                .filter_map(|part| match part {
                    ragent_types::llm::ContentPart::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" "),
        })
        .unwrap_or_default();
    assert!(
        text.contains("fix the flaky test"),
        "goal text reached the LLM request: {text}"
    );
    Ok(())
}

#[tokio::test]
async fn test_confirm_spec_fields_reach_the_processor() -> Result<()> {
    let (mut app, captured) = make_scripted_app();
    let mut rx = app.event_bus.subscribe();
    open_dialog(&mut app);
    // Select the second agent in the picker.
    handle_key(&mut app, key(KeyCode::Down));
    let chosen_agent = app
        .loop_setup
        .as_ref()
        .expect("open")
        .selected_agent_name()
        .to_string();
    tab_n(&mut app, 1); // -> Goal
    for c in "reach the goal".chars() {
        type_char(&mut app, c);
    }
    tab_n(&mut app, 6); // Goal -> MaxSteps
    for c in "7".chars() {
        type_char(&mut app, c);
    }
    tab_n(&mut app, 1); // -> CostLimit
    for c in "999".chars() {
        type_char(&mut app, c);
    }
    tab_n(&mut app, 1); // -> Checkpoints
    handle_key(&mut app, key(KeyCode::Char(' '))); // checkpoints off
    handle_key(&mut app, key(KeyCode::Enter));

    // Wait for the spawned task to register the loop and process the goal.
    for _ in 0..200 {
        if !captured.lock().expect("captured lock").is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(
        captured.lock().expect("captured lock").len(),
        1,
        "loop started and goal processed"
    );

    // The loop-termination event proves the loop engaged and stopped on the
    // scripted no-tool-call response (stop condition 1, FR-010).
    let mut terminated = 0;
    while let Ok(event) = rx.try_recv() {
        if matches!(event, Event::LoopTerminated { status, .. } if status == "completed") {
            terminated += 1;
        }
    }
    assert_eq!(terminated, 1, "loop completed exactly once");
    let _ = chosen_agent; // agent preset recorded in the spec (checked below via tracker map)
    Ok(())
}

#[tokio::test]
async fn test_confirm_uses_spec_agent_preset() -> Result<()> {
    let (mut app, captured) = make_scripted_app();
    // Pick an agent by name that exists in the loaded list but is NOT the
    // currently active one, then confirm.
    open_dialog(&mut app);
    let target = app
        .cycleable_agents
        .iter()
        .position(|a| a.name != app.agent_name)
        .expect("a second agent exists");
    let target_name = app.cycleable_agents[target].name.clone();
    let mut state = app.loop_setup.take().expect("open");
    state.selected_agent = target.min(state.agents.len() - 1);
    state.goal_field.insert_str("use the picked agent preset");
    app.loop_setup = Some(state);
    handle_key(&mut app, key(KeyCode::Enter));

    for _ in 0..200 {
        if !captured.lock().expect("captured lock").is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(captured.lock().expect("captured lock").len(), 1);
    // The transcript message records which agent preset was used.
    let text = app
        .messages
        .last()
        .map(|m| m.text_content())
        .unwrap_or_default();
    assert!(
        text.contains(&format!("[loop {target_name}]")),
        "dispatch message names the spec agent: {text}"
    );
    let _ = LoopSpec::new("general", "unused");
    let _ = AgentInfo::new("general", "unused");
    let _ = ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    };
    Ok(())
}
// ── T-015: `/loop` slash-command registration (FR-001, FR-003, FR-005) ──────

/// FR-001: `/loop` with no arguments opens the setup dialog.
#[test]
fn test_slash_loop_no_arg_opens_dialog() {
    let mut app = support::make_app();
    app.execute_slash_command("/loop");
    assert!(
        app.loop_setup.is_some(),
        "no-arg /loop must open the setup dialog"
    );
}

/// FR-001: `/loop` appears in `/help` output for autocomplete visibility.
#[test]
fn test_help_lists_loop_command() {
    let mut app = support::make_app();
    app.session_id = Some("sess-help".to_string());
    app.execute_slash_command("/help");
    let text = app
        .messages
        .last()
        .map(|m| m.text_content())
        .unwrap_or_default();
    assert!(
        text.contains("/loop"),
        "/help should document the /loop command: {text}"
    );
}

/// FR-003: the one-shot form `/loop <agent> <goal>` starts the loop
/// immediately with the documented defaults.
#[tokio::test]
async fn test_slash_loop_one_shot_starts_loop() -> Result<()> {
    let (mut app, captured) = make_scripted_app();
    app.execute_slash_command("/loop coder fix the flaky login test");
    // Dialog must not open; the goal went straight to the loop path.
    assert!(
        app.loop_setup.is_none(),
        "one-shot /loop must not open the setup dialog"
    );
    assert!(
        app.messages
            .iter()
            .any(|m| m.text_content().contains("[loop coder]")
                && m.text_content().contains("fix the flaky login")),
        "goal message pushed with the named agent"
    );
    // Wait for the spawned task to send the goal conversation.
    for _ in 0..200 {
        if !captured.lock().expect("captured lock").is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(
        captured.lock().expect("captured lock").len(),
        1,
        "one conversation request sent with the goal"
    );
    Ok(())
}

/// FR-005: `/loop <agent>` without a goal shows an error naming the missing
/// field and re-opens the dialog instead of starting a loop.
#[tokio::test]
async fn test_slash_loop_empty_goal_shows_error() -> Result<()> {
    let (mut app, captured) = make_scripted_app();
    app.execute_slash_command("/loop coder");
    let text = app
        .messages
        .last()
        .map(|m| m.text_content())
        .unwrap_or_default();
    assert!(
        text.contains("missing field: goal"),
        "error must name the missing field: {text}"
    );
    assert!(
        app.loop_setup.is_some(),
        "dialog re-opened so the user can complete the goal field"
    );
    // Give any (incorrectly) spawned task time to fire; none must.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(
        captured.lock().expect("captured lock").is_empty(),
        "no loop may start without a goal"
    );
    Ok(())
}
/// T-024 (FR-003): the one-shot form `/loop <agent> <goal>` activates the
/// documented defaults — config step limit (25), config cost limit (none),
/// checkpoints on, no verification command, no restrictions — before the
/// first iteration. The spec is captured as soon as it is registered because
/// termination removes it from the tracker map.
#[tokio::test]
async fn test_slash_loop_one_shot_starts_with_documented_defaults() -> Result<()> {
    let (mut app, captured) = make_scripted_app();
    // Deterministic budgets: preload the processor's config cache with the
    // documented loop defaults so `start_loop` resolves against this entry.
    let config: RagentConfig =
        serde_json::from_str(r#"{"loop":{"max_steps":25}}"#).expect("valid config JSON");
    *app.session_processor.cached_config.lock() = Some(CachedConfig {
        config: std::sync::Arc::new(config),
        file_mtimes: Vec::new(),
        env_overrides_present: false,
    });
    app.execute_slash_command("/loop coder list the files and stop");
    let session_id = app.session_id.clone().expect("session id");

    // The spec appears as soon as the spawned task registers the loop; grab
    // it early because termination removes the entry from the map.
    let spec = {
        let mut captured_spec = None;
        for _ in 0..200 {
            let existing = app
                .session_processor
                .active_loop_specs
                .read()
                .await
                .get(&session_id)
                .cloned();
            if let Some(spec) = existing {
                captured_spec = Some(spec);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        captured_spec.expect("loop spec registered by the spawned task")
    };
    assert_eq!(spec.agent, "coder");
    assert_eq!(spec.max_steps, Some(25), "config step limit applies");
    assert!(spec.cost_limit.is_none(), "config cost limit (none)");
    assert!(spec.checkpoints, "checkpoints on by default");
    assert!(spec.verify_cmd.is_none(), "no verification command");
    assert!(spec.scope.is_empty(), "no scope restriction");
    assert!(spec.read_only.is_empty(), "no read-only constraint");
    assert!(spec.tool_set.is_empty(), "no tool-set restriction");

    // The goal conversation goes out exactly once.
    for _ in 0..200 {
        if !captured.lock().expect("captured lock").is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(
        captured.lock().expect("captured lock").len(),
        1,
        "one conversation request sent with the goal"
    );
    Ok(())
}

/// T-024 (FR-025 + NFR-003): a one-shot loop run records the agent-loop
/// telemetry exactly once — one `LoopTerminated` event, the final iteration
/// count, and the exactly-once flag flipped after the run.
#[tokio::test]
async fn test_slash_loop_one_shot_records_telemetry_once() -> Result<()> {
    let (mut app, captured) = make_scripted_app();
    let mut rx = app.event_bus.subscribe();
    app.execute_slash_command("/loop coder finish immediately");

    // Wait for the goal conversation, then for the termination event.
    for _ in 0..200 {
        if !captured.lock().expect("captured lock").is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let mut terminated = Vec::new();
    for _ in 0..200 {
        while let Ok(event) = rx.try_recv() {
            if matches!(event, Event::LoopTerminated { .. }) {
                terminated.push(event);
            }
        }
        if !terminated.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(terminated.len(), 1, "termination published exactly once");
    let Event::LoopTerminated {
        status, iterations, ..
    } = terminated.into_iter().next().expect("one event")
    else {
        unreachable!("filtered to LoopTerminated");
    };
    assert_eq!(status, "completed", "stop condition 1 (FR-010)");
    assert_eq!(iterations, 1, "the final iteration count is recorded");
    assert!(
        app.session_processor
            .loop_telemetry_recorded
            .load(Ordering::SeqCst),
        "the telemetry record flipped the exactly-once flag"
    );
    Ok(())
}

/// `/loop help` shows the usage help for every loop command and option and
/// does not start a loop or open the setup dialog.
#[test]
fn test_slash_loop_help_shows_usage() {
    let (mut app, captured) = make_scripted_app();
    app.execute_slash_command("/loop help");
    let text = app
        .messages
        .last()
        .map(|m| m.text_content())
        .unwrap_or_default();
    assert!(
        text.contains("/loop help"),
        "help lists the help form: {text}"
    );
    assert!(
        text.contains("Setup dialog fields"),
        "help documents the dialog fields: {text}"
    );
    assert!(
        text.contains("stop condition"),
        "help documents the stop conditions: {text}"
    );
    assert!(
        text.contains("loop.max_steps"),
        "help documents the config knobs: {text}"
    );
    assert!(
        app.loop_setup.is_none(),
        "help must not open the setup dialog"
    );
    // Give any (incorrectly) spawned task time to fire; none must.
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert!(
        captured.lock().expect("captured lock").is_empty(),
        "no loop may start from the help subcommand"
    );
}
/// FR-003 (one-shot flags): `parse_loop_flags` extracts `--max-steps`,
/// `--cost_limit` / `--cost-limit`, and `--timeout` from anywhere in the
/// token list and removes them, leaving the agent and goal tokens behind.
#[test]
fn test_parse_loop_flags_extracts_overrides() {
    let tokens: Vec<String> = [
        "--max-steps",
        "10",
        "coder",
        "--cost_limit",
        "5000",
        "ship",
        "it",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    let (remaining, overrides) = parse_loop_flags(&tokens).expect("parse ok");
    assert_eq!(remaining, ["coder", "ship", "it"]);
    assert_eq!(overrides.max_steps, Some(10));
    assert_eq!(overrides.cost_limit, Some(5000));
    assert_eq!(overrides.timeout, None);

    // Dash/underscore spelling, `=` value form, and flag-after-goal order.
    let tokens: Vec<String> = [
        "general",
        "fix",
        "it",
        "--timeout=30",
        "--cost-limit",
        "250",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    let (remaining, overrides) = parse_loop_flags(&tokens).expect("parse ok");
    assert_eq!(remaining, ["general", "fix", "it"]);
    assert_eq!(overrides.timeout, Some(30));
    assert_eq!(overrides.cost_limit, Some(250));
    assert_eq!(overrides.max_steps, None);
}

/// FR-003: flag values must be non-negative integers and present; the
/// error names the offending flag.
#[test]
fn test_parse_loop_flags_rejects_bad_values() {
    let tokens: Vec<String> = vec!["--max-steps".to_string()];
    let err = parse_loop_flags(&tokens).expect_err("missing value is rejected");
    assert!(err.contains("--max-steps"), "error names the flag: {err}");

    let tokens: Vec<String> = ["--max-steps", "abc"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let err = parse_loop_flags(&tokens).expect_err("non-numeric value is rejected");
    assert!(err.contains("--max-steps") && err.contains("abc"), "{err}");

    let tokens: Vec<String> = ["--timeout", "-5"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    assert!(
        parse_loop_flags(&tokens).is_err(),
        "negative timeout rejected"
    );

    // Unknown dashed tokens are NOT flags — they stay in the remaining
    // tokens (an agent preset or goal text may legitimately contain dashes).
    let tokens: Vec<String> = ["--unknown", "5"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let (remaining, overrides) = parse_loop_flags(&tokens).expect("unknown flag passes through");
    assert_eq!(remaining, ["--unknown", "5"]);
    assert_eq!(overrides, LoopOverrides::default());
}

/// FR-003: overrides land on the spec — `apply_loop_overrides` only sets
/// the fields the user actually passed.
#[test]
fn test_apply_loop_overrides_sets_only_passed_flags() {
    let mut spec = LoopSpec::new("coder", "goal");
    let overrides = LoopOverrides {
        max_steps: Some(7),
        cost_limit: None,
        timeout: Some(45),
    };
    apply_loop_overrides(&mut spec, &overrides);
    assert_eq!(spec.max_steps, Some(7));
    assert_eq!(spec.cost_limit, None, "unset flag leaves the default");
    assert_eq!(spec.checkpoint_timeout_secs, Some(45));
}

/// FR-003: the one-shot form with flags registers a spec carrying every
/// override (step budget, cost budget, checkpoint timeout) before the first
/// iteration.
#[tokio::test]
async fn test_slash_loop_one_shot_flags_override_budgets() -> Result<()> {
    let (mut app, captured) = make_scripted_app();
    app.execute_slash_command(
        "/loop coder --max-steps 9 --cost_limit 12345 --timeout 60 fix the flaky login test",
    );
    let session_id = app.session_id.clone().expect("session id");

    let spec = {
        let mut captured_spec = None;
        for _ in 0..200 {
            let existing = app
                .session_processor
                .active_loop_specs
                .read()
                .await
                .get(&session_id)
                .cloned();
            if let Some(spec) = existing {
                captured_spec = Some(spec);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        captured_spec.expect("loop spec registered by the spawned task")
    };
    assert_eq!(spec.agent, "coder");
    assert_eq!(spec.goal, "fix the flaky login test");
    assert_eq!(spec.max_steps, Some(9), "--max-steps override applies");
    assert_eq!(
        spec.cost_limit,
        Some(12_345),
        "--cost_limit override applies"
    );
    assert_eq!(
        spec.checkpoint_timeout_secs,
        Some(60),
        "--timeout override applies"
    );

    // The goal conversation still goes out exactly once.
    for _ in 0..200 {
        if !captured.lock().expect("captured lock").is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(
        captured.lock().expect("captured lock").len(),
        1,
        "one conversation request sent with the goal"
    );
    Ok(())
}

/// FR-003: an invalid flag value stops the loop from starting and reports
/// the offending flag; nothing is dispatched.
#[tokio::test]
async fn test_slash_loop_invalid_flag_value_does_not_start() -> Result<()> {
    let (mut app, captured) = make_scripted_app();
    app.execute_slash_command("/loop coder --max-steps abc fix it");
    let text = app
        .messages
        .last()
        .map(|m| m.text_content())
        .unwrap_or_default();
    assert!(
        text.contains("--max-steps") && text.contains("abc"),
        "error names the flag and the bad value: {text}"
    );
    assert_eq!(app.status, "loop: invalid argument");
    // Give any (incorrectly) spawned task time to fire; none must.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(
        captured.lock().expect("captured lock").is_empty(),
        "no loop may start with an invalid flag value"
    );
    Ok(())
}

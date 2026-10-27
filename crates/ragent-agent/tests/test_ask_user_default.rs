//! `AskUserQuestion` default/fallback option selection.
//!
//! When an `AskUserQuestion` call carries `options`, the tool resolves a zero-based
//! `default_index` published on `Event::QuestionRequested`. The TUI pre-selects
//! that option and, if the user lets the dialog time out, answers with it. The
//! resolution order is:
//!
//! 1. the option matching an explicit `default` string (case-insensitive),
//! 2. the first option containing the word "recommended",
//! 3. the first option.
//!
//! These tests cover the schema/description surface and the tool's own
//! resolution through the published event.

use serde_json::{Value, json};
use std::sync::Arc;

use ragent_agent::event::{Event, EventBus};
use ragent_agent::tool::{Tool, ToolContext, create_default_registry};

fn test_ctx(bus: Arc<EventBus>) -> ToolContext {
    ToolContext {
        session_id: "ask-user-default-test".to_string(),
        working_dir: std::env::current_dir().expect("cwd"),
        event_bus: bus,
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        allowed_roots: Vec::new(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        tool_registry: ToolContext::default_tool_registry(),
    }
}

fn ask_user_schema() -> Value {
    let registry = create_default_registry();
    registry
        .definitions()
        .iter()
        .find(|d| d.name == "AskUserQuestion")
        .map(|d| d.parameters.clone())
        .expect("AskUserQuestion should be registered")
}

fn ask_user_description() -> String {
    let registry = create_default_registry();
    registry
        .definitions()
        .iter()
        .find(|d| d.name == "AskUserQuestion")
        .map(|d| d.description.clone())
        .expect("AskUserQuestion should be registered")
}

#[test]
fn test_ask_user_schema_advertises_optional_default() {
    let schema = ask_user_schema();
    let props = schema
        .get("properties")
        .and_then(Value::as_object)
        .expect("ask_user schema must define properties");
    assert!(
        props.contains_key("default"),
        "ask_user schema must advertise a `default` property"
    );
    // `default` is optional - only `question` is required.
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    assert_eq!(required, vec!["question"]);
}

#[test]
fn test_ask_user_description_mentions_timeout_default() {
    let desc = ask_user_description();
    assert!(
        desc.contains("default"),
        "ask_user description must mention the `default` parameter, got: {desc}"
    );
    assert!(
        desc.to_lowercase().contains("time out") || desc.to_lowercase().contains("timeout"),
        "ask_user description must explain the timeout fallback, got: {desc}"
    );
}

/// Resolve the published `default_index` for a given `AskUserQuestion` input.
///
/// The tool blocks until a `QuestionAnswered` arrives, so the test pre-publishes
/// a wildcard reply (like the existing subagent tests) and then inspects the
/// `QuestionRequested` event for the resolved index.
async fn resolved_index(options: Vec<&str>, default: Option<&str>) -> Option<usize> {
    let bus = Arc::new(EventBus::new(64));
    let ctx = test_ctx(bus.clone());

    let mut input = json!({
        "question": "Pick one",
        "options": options,
    });
    if let Some(d) = default {
        input["default"] = json!(d);
    }

    // One subscriber captures the resolved index; another answers the question
    // so the tool's blocking wait returns.
    let mut watch = bus.subscribe();
    let watcher = tokio::spawn(async move {
        while let Ok(ev) = watch.recv().await {
            if let Event::QuestionRequested { default_index, .. } = ev {
                return default_index;
            }
        }
        None
    });

    let mut answer = bus.subscribe();
    let responder = {
        let bus = bus.clone();
        tokio::spawn(async move {
            while let Ok(ev) = answer.recv().await {
                if let Event::QuestionRequested {
                    secret: false,
                    session_id,
                    request_id,
                    ..
                } = ev
                {
                    bus.publish(Event::QuestionAnswered {
                        session_id,
                        request_id,
                        response: "ok".to_string(),
                        secret: false,
                    });
                    break;
                }
            }
        })
    };

    let tool = ragent_agent::tool::aliases::AskUserTool;
    let _ = tool.execute(input, &ctx).await;
    let observed = watcher.await.ok().flatten();
    let _ = responder.await;
    observed
}

#[tokio::test]
async fn test_explicit_default_wins() {
    assert_eq!(
        resolved_index(vec!["a", "b", "c"], Some("b")).await,
        Some(1)
    );
}

#[tokio::test]
async fn test_explicit_default_is_case_insensitive_and_trimmed() {
    assert_eq!(
        resolved_index(vec!["Alpha", "Beta"], Some("  beta ")).await,
        Some(1)
    );
}

#[tokio::test]
async fn test_recommended_option_preferred_when_no_explicit_default() {
    assert_eq!(
        resolved_index(vec!["first", "second (recommended)", "third"], None).await,
        Some(1)
    );
}

#[tokio::test]
async fn test_first_option_when_no_default_and_no_recommended() {
    assert_eq!(
        resolved_index(vec!["first", "second", "third"], None).await,
        Some(0)
    );
}

#[tokio::test]
async fn test_unknown_default_falls_back_to_first_option() {
    assert_eq!(
        resolved_index(vec!["first", "second"], Some("nonexistent")).await,
        Some(0)
    );
}

#[tokio::test]
async fn test_unknown_default_falls_back_to_recommended_option() {
    assert_eq!(
        resolved_index(vec!["first", "second (recommended)"], Some("nope")).await,
        Some(1)
    );
}

#[tokio::test]
async fn test_no_options_yields_no_default_index() {
    assert_eq!(resolved_index(vec![], Some("anything")).await, None);
}

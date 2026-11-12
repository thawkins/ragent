//! A project's base model seeds new sessions (spec `projects`, T-007).

use ragent_agent::agent::ModelRef;
use ragent_tui::App;

#[path = "support/mod.rs"]
mod support;

fn model(provider: &str, id: &str) -> ModelRef {
    ModelRef {
        provider_id: provider.to_string(),
        model_id: id.to_string(),
    }
}

/// An app whose launch project has `base` as its base model, before any
/// session exists.
fn app_with_base_model(base: &ModelRef) -> App {
    let app = support::make_app();
    let project = app.focused_project_for_test().expect("launch project");
    app.project_manager_for_test()
        .set_base_model(&project.id, Some(base))
        .expect("set base model");
    app
}

#[tokio::test]
async fn test_project_base_model_seeds_the_first_session() {
    let mut app = app_with_base_model(&model("anthropic", "claude-base"));
    app.session.selected_model = Some("openai/global-default".to_string());
    app.session.selected_model_ctx_window = Some(1234);

    // The slash gate creates the session.
    app.execute_slash_command("/session list").await;

    assert!(app.session.session_id.is_some());
    assert_eq!(
        app.session.selected_model.as_deref(),
        Some("anthropic/claude-base")
    );
    assert_eq!(
        app.session.selected_model_ctx_window, None,
        "the previous model's window is not kept"
    );
}

#[tokio::test]
async fn test_project_base_model_seeds_each_new_session_and_model_stays_per_session() {
    let mut app = app_with_base_model(&model("anthropic", "claude-base"));
    app.execute_slash_command("/session list").await;
    let first = app.session.session_id.clone().expect("first");

    // `/model` in the first session: only that session changes.
    app.session.selected_model = Some("openai/gpt-picked".to_string());
    app.execute_slash_command("/session new").await;

    assert_ne!(app.session.session_id.as_deref(), Some(first.as_str()));
    assert_eq!(
        app.session.selected_model.as_deref(),
        Some("anthropic/claude-base"),
        "a new session starts on the project base, not the focused session's model"
    );
    let parked = app
        .background
        .iter()
        .find(|v| v.session_id.as_deref() == Some(first.as_str()))
        .expect("first session is parked");
    assert_eq!(parked.selected_model.as_deref(), Some("openai/gpt-picked"));
    assert_eq!(
        app.focused_project_for_test()
            .expect("project")
            .base_model_label()
            .as_deref(),
        Some("anthropic/claude-base"),
        "changing a session's model leaves the project base alone"
    );
}

#[tokio::test]
async fn test_project_base_model_absent_inherits_the_focused_model() {
    let mut app = support::make_app();
    app.session.selected_model = Some("openai/gpt-picked".to_string());
    app.execute_slash_command("/session list").await;

    app.execute_slash_command("/session new").await;

    assert_eq!(
        app.session.selected_model.as_deref(),
        Some("openai/gpt-picked"),
        "with no project base a new session inherits the focused model"
    );
}

#[tokio::test]
async fn test_project_base_model_applies_to_sessions_in_another_project() {
    let mut app = support::make_app();
    app.session.selected_model = Some("openai/gpt-picked".to_string());
    app.execute_slash_command("/session list").await;
    let other = app
        .storage
        .ensure_project("/nonexistent/base_model/other")
        .expect("other project");
    app.project_manager_for_test()
        .set_base_model(&other.id, Some(&model("ollama", "local-model")))
        .expect("base model");

    app.execute_slash_command(&format!("/session new --project {}", other.id))
        .await;

    assert_eq!(
        app.session.selected_model.as_deref(),
        Some("ollama/local-model")
    );
}

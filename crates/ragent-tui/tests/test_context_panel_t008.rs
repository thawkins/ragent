//! Tests for the Context side panel (spec `contextpanel`).
//!
//! - **T-008**: Fetch the selected model's context-window limit.

mod support;

/// Mirror of [`ragent_agent::agent::resolve_default_model`] resolution used
/// to determine which (provider, model) pair the panel should look up.
fn resolved_default_model(app: &ragent_tui::App) -> Option<(String, String)> {
    if let Some(model) = app.agent_info.model.as_ref() {
        return Some((model.provider_id.clone(), model.model_id.clone()));
    }
    app.provider_registry
        .list()
        .iter()
        .find_map(|p| p.models.first().map(|m| (p.id.clone(), m.id.clone())))
}

#[test]
fn test_context_window_matches_registry_advertisement() {
    // FR-010: when the active model's provider advertises a context window,
    // the panel must resolve exactly that positive token capacity. When the
    // resolved model advertises nothing, the panel must report unknown
    // instead of guessing (FR-011).
    let app = support::make_app();
    let expected = resolved_default_model(&app).and_then(|(provider, model)| {
        app.provider_registry
            .resolve_model(&provider, &model)
            .map(|m| m.context_window)
            .filter(|w| *w > 0)
    });
    let actual = app.active_context_window_tokens();
    assert_eq!(
        actual, expected,
        "panel window must match the registry advertisement for the resolved model"
    );
}

#[test]
fn test_context_window_is_positive_when_available() {
    // FR-010: whenever a value is returned it must be a usable positive
    // token capacity — zero is never a valid window.
    let app = support::make_app();
    if let Some(window) = app.active_context_window_tokens() {
        assert!(window > 0, "advertised context window must be positive");
    }
}

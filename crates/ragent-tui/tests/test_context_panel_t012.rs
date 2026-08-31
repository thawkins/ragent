//! Tests for the Context side panel (spec `contextpanel`).
//!
//! - **T-012**: Ensure panel content is excluded from LLM context (FR-016).

mod support;

#[test]
fn test_rendering_panel_does_not_mutate_conversation() {
    // FR-016: the context breakdown is a UI-only artifact. Rendering the
    // panel must not append messages, alter the history estimate, or change
    // the message count that feeds the LLM request.
    let mut app = support::make_app();
    app.show_context_panel = true;

    let before_messages = app.conversation_message_count();
    let before_history = app.conversation_history_token_count();

    let backend = ratatui::backend::TestBackend::new(100, 30);
    let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, &mut app))
        .expect("draw");

    assert_eq!(
        app.conversation_message_count(),
        before_messages,
        "panel render must not add messages"
    );
    assert_eq!(
        app.conversation_history_token_count(),
        before_history,
        "panel render must not change the history estimate"
    );
    assert!(
        app.messages.is_empty(),
        "panel content must never enter the session message list"
    );
}

#[test]
fn test_panel_cache_is_isolated_from_message_list() {
    // FR-016: the plain-text copy cache is populated during render, but the
    // session message list (the sole source for LLM requests and storage)
    // is untouched by it.
    let mut app = support::make_app();
    app.show_context_panel = true;
    assert!(app.messages.is_empty());

    let backend = ratatui::backend::TestBackend::new(100, 30);
    let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, &mut app))
        .expect("draw");

    // The panel content cache was populated for copy support...
    assert!(
        !app.context_content_lines.is_empty(),
        "panel should cache its lines for text selection"
    );
    // ...while the conversation stays empty (nothing to send to the LLM).
    assert!(app.messages.is_empty());
    assert_eq!(app.conversation_message_count(), 0);
    assert_eq!(app.conversation_history_token_count(), 0);
}

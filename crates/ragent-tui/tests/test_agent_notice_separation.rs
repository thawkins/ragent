//! TUI event-handler tests for `Event::AgentNotice` chat-bubble separation.
//!
//! Covers the regression where two consecutive `AgentNotice` events ran onto
//! each other in the message window: each notice must land in its own
//! assistant message so the renderer's per-notice blank-line separation
//! applies between bubbles, and streamed text must not merge into a notice
//! bubble.

use ragent_agent::event::Event;
use ragent_agent::message::{MessagePart, Role};

#[path = "support/mod.rs"]
mod support;

/// Two consecutive notices for the current session must produce two separate
/// assistant messages, each starting with the notice sentinel.
#[test]
fn test_consecutive_agent_notices_get_separate_messages() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());

    app.handle_event(Event::AgentNotice {
        session_id: "s1".to_string(),
        message: "First notice item".to_string(),
    });
    app.handle_event(Event::AgentNotice {
        session_id: "s1".to_string(),
        message: "Second notice item".to_string(),
    });

    let notices: Vec<&str> = app
        .messages
        .iter()
        .filter(|m| m.role == Role::Assistant)
        .filter_map(|m| match m.parts.first() {
            Some(MessagePart::Text { text }) => Some(text.as_str()),
            _ => None,
        })
        .collect();

    assert_eq!(
        notices.len(),
        2,
        "two consecutive notices must be two separate messages, got: {notices:?}"
    );
    assert!(
        notices[0].contains("First notice item") && !notices[0].contains("Second notice item"),
        "first notice must not contain the second notice, got: {:?}",
        notices[0]
    );
    assert!(
        notices[1].starts_with("📋 Agent Notice") && notices[1].contains("Second notice item"),
        "second notice must start its own bubble, got: {:?}",
        notices[1]
    );
}

/// Assistant text streamed after a notice must not merge into the notice
/// bubble (a notice is a system annotation, not the agent's own output).
#[test]
fn test_agent_notice_does_not_absorb_subsequent_streamed_text() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());

    app.handle_event(Event::AgentNotice {
        session_id: "s1".to_string(),
        message: "Waiting for model response... (30s)".to_string(),
    });
    app.handle_event(Event::TextDelta {
        session_id: "s1".to_string(),
        text: "Here is the actual answer.".to_string(),
    });

    let assistant_texts: Vec<&str> = app
        .messages
        .iter()
        .filter(|m| m.role == Role::Assistant)
        .filter_map(|m| match m.parts.first() {
            Some(MessagePart::Text { text }) => Some(text.as_str()),
            _ => None,
        })
        .collect();

    let notice = assistant_texts
        .iter()
        .find(|t| t.starts_with("📋 Agent Notice"))
        .expect("notice bubble must exist");
    assert!(
        !notice.contains("Here is the actual answer"),
        "streamed text must not merge into the notice bubble, got: {notice:?}"
    );
}

/// A notice arriving mid-stream must split the streamed assistant message so
/// the notice bubble does not swallow the trailing streamed text.
#[test]
fn test_agent_notice_splits_streamed_message() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());

    app.handle_event(Event::TextDelta {
        session_id: "s1".to_string(),
        text: "partial answer".to_string(),
    });
    app.handle_event(Event::AgentNotice {
        session_id: "s1".to_string(),
        message: "Waiting for model response... (60s)".to_string(),
    });
    app.handle_event(Event::TextDelta {
        session_id: "s1".to_string(),
        text: "rest of answer".to_string(),
    });

    let assistant_texts: Vec<&str> = app
        .messages
        .iter()
        .filter(|m| m.role == Role::Assistant)
        .filter_map(|m| match m.parts.first() {
            Some(MessagePart::Text { text }) => Some(text.as_str()),
            _ => None,
        })
        .collect();

    let notice_idx = assistant_texts
        .iter()
        .position(|t| t.starts_with("📋 Agent Notice"))
        .expect("notice bubble must exist");
    assert!(
        notice_idx > 0,
        "streamed text before the notice must be its own message"
    );
    let notice = assistant_texts[notice_idx];
    assert!(
        !notice.contains("partial answer") && !notice.contains("rest of answer"),
        "notice bubble must not contain streamed text, got: {notice:?}"
    );
}

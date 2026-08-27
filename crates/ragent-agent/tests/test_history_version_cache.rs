//! Integration tests for the history-version cache.
//!
//! Validates that `AgentPerf` T-007 / FR-006 — the agent loop skips
//! `history_to_chat_messages` when the history version has not changed
//! since the previous step.

use ragent_agent::message::{Message, MessagePart, Role};
use ragent_agent::session::cache::SessionState;

fn user_message(id: &str, text: &str) -> Message {
    Message {
        id: id.to_string(),
        session_id: "s".to_string(),
        role: Role::User,
        parts: vec![MessagePart::Text {
            text: text.to_string(),
        }],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        edit_seq: 0,
    }
}

#[test]
fn session_state_starts_empty() {
    let state = SessionState::new("s1");
    assert_eq!(state.session_id(), "s1");
    assert_eq!(state.cached_serialised(), None);
}

#[test]
fn cached_chat_messages_for_version_returns_none_for_first_call() {
    let mut state = SessionState::new("s1");
    let _msgs = [user_message("m1", "hello")];
    let version = 42u64;
    let result = state.cached_chat_messages_for_version(version);
    assert!(result.is_none());
}

#[test]
fn cached_chat_messages_for_version_hits_on_repeat() {
    let mut state = SessionState::new("s1");
    let msgs = [user_message("m1", "hello")];
    let version = 7u64;
    // First call: miss.
    assert!(state.cached_chat_messages_for_version(version).is_none());
    // Populate the cache.
    state.store_chat_messages(
        msgs.iter()
            .map(|m| ragent_llm::llm::ChatMessage {
                role: "user".to_string(),
                content: ragent_llm::llm::ChatContent::Text(m.parts[0].text_clone()),
            })
            .collect(),
        None,
    );
    // Second call: hit.
    let cached = state.cached_chat_messages_for_version(version);
    assert!(cached.is_some());
    assert_eq!(cached.unwrap().len(), 1);
}

#[test]
fn cached_chat_messages_for_version_misses_on_version_change() {
    let mut state = SessionState::new("s1");
    let msgs = [user_message("m1", "hello")];
    state.store_chat_messages(
        msgs.iter()
            .map(|m| ragent_llm::llm::ChatMessage {
                role: "user".to_string(),
                content: ragent_llm::llm::ChatContent::Text(m.parts[0].text_clone()),
            })
            .collect(),
        None,
    );
    // Different version -> miss.
    let cached = state.cached_chat_messages_for_version(99);
    assert!(cached.is_none());
    // Same version -> hit.
    let cached = state.cached_chat_messages_for_version(99);
    assert!(cached.is_some());
}

#[test]
fn clear_resets_caches() {
    let mut state = SessionState::new("s1");
    let msgs = [user_message("m1", "hello")];
    state.store_chat_messages(
        msgs.iter()
            .map(|m| ragent_llm::llm::ChatMessage {
                role: "user".to_string(),
                content: ragent_llm::llm::ChatContent::Text(m.parts[0].text_clone()),
            })
            .collect(),
        Some(b"serialised".to_vec()),
    );
    state.clear();
    assert!(state.cached_chat_messages_for_version(1).is_none());
    assert_eq!(state.cached_serialised(), None);
}

trait TextClone {
    fn text_clone(&self) -> String;
}
impl TextClone for MessagePart {
    fn text_clone(&self) -> String {
        match self {
            Self::Text { text } => text.clone(),
            _ => String::new(),
        }
    }
}

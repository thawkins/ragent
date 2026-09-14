#![allow(clippy::assert_is_empty)]
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
        std::sync::Arc::new(
            msgs.iter()
                .map(|m| ragent_llm::llm::ChatMessage {
                    role: "user".to_string(),
                    content: ragent_llm::llm::ChatContent::Text(m.parts[0].text_clone()),
                })
                .collect(),
        ),
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
        std::sync::Arc::new(
            msgs.iter()
                .map(|m| ragent_llm::llm::ChatMessage {
                    role: "user".to_string(),
                    content: ragent_llm::llm::ChatContent::Text(m.parts[0].text_clone()),
                })
                .collect(),
        ),
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
        std::sync::Arc::new(
            msgs.iter()
                .map(|m| ragent_llm::llm::ChatMessage {
                    role: "user".to_string(),
                    content: ragent_llm::llm::ChatContent::Text(m.parts[0].text_clone()),
                })
                .collect(),
        ),
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

/// PERF-032: the cache-hit path must hand back the *same* allocation it was
/// given, proving the per-turn path is a refcount bump and not a deep clone of
/// the transcript.
#[test]
fn cache_hit_shares_the_stored_allocation() {
    let mut state = SessionState::new("s1");
    let stored = std::sync::Arc::new(vec![ragent_llm::llm::ChatMessage {
        role: "user".to_string(),
        content: ragent_llm::llm::ChatContent::Text("hello".to_string()),
    }]);
    state.store_chat_messages(std::sync::Arc::clone(&stored), None);
    let version = 5u64;
    // First call records the version but returns None (no prior version match).
    assert!(state.cached_chat_messages_for_version(version).is_none());
    let hit = state
        .cached_chat_messages_for_version(version)
        .expect("repeat call hits");
    assert!(
        std::sync::Arc::ptr_eq(&hit, &stored),
        "cache hit must share the stored allocation, not clone it"
    );
}

/// Build a `Message` with an explicit id and text.
fn msg(id: &str, text: &str) -> Message {
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

/// PERF-033: a pure append is detected and only the appended tail needs
/// conversion; the concatenation must equal a full rebuild.
#[tokio::test]
async fn append_fast_path_matches_full_rebuild() {
    use ragent_agent::session::history::history_to_chat_messages;

    let mut state = SessionState::new("s1");
    let base_history = vec![msg("m1", "one"), msg("m2", "two")];
    let built = std::sync::Arc::new(history_to_chat_messages(&base_history).await);
    state.store_chat_messages(std::sync::Arc::clone(&built), None);
    state.record_history_base(&base_history);
    // Drop the test's own handle so the cache holds the sole reference and the
    // fast path can extend the vector in place without cloning it.
    drop(built);

    // Append two more messages.
    let mut grown = base_history.clone();
    grown.push(msg("m3", "three"));
    grown.push(msg("m4", "four"));

    let (base, base_len) = state
        .take_cached_for_append(&grown)
        .expect("pure append is detected");
    assert_eq!(base_len, 2);
    // The cached Arc was moved out (refcount dropped), so extending it in
    // place is allocation-only work, no full-vector clone.
    let mut rebuilt = std::sync::Arc::try_unwrap(base).expect("sole owner");
    rebuilt.extend(history_to_chat_messages(&grown[base_len..]).await);

    let full = history_to_chat_messages(&grown).await;
    let as_json = |v: &[ragent_llm::llm::ChatMessage]| {
        serde_json::to_string(v).expect("chat messages serialise")
    };
    assert_eq!(as_json(&rebuilt), as_json(&full));
}

/// PERF-033: if the last message of the cached prefix changes (the history
/// version folds `id`/`updated_at` of the last message) the fast path must be
/// declined so the transcript is rebuilt.
#[tokio::test]
async fn mutated_prefix_declines_append_fast_path() {
    use ragent_agent::session::history::history_to_chat_messages;

    let mut state = SessionState::new("s1");
    let base_history = vec![msg("m1", "one"), msg("m2", "two")];
    let built = std::sync::Arc::new(history_to_chat_messages(&base_history).await);
    state.store_chat_messages(built, None);
    state.record_history_base(&base_history);

    // Append a message, but also change the last message of the cached prefix
    // so its version no longer matches what was recorded.
    let mut mutated = base_history.clone();
    mutated[1].updated_at += chrono::Duration::seconds(1);
    mutated.push(msg("m3", "three"));

    assert!(
        state.take_cached_for_append(&mutated).is_none(),
        "a changed prefix must force a full rebuild"
    );
}

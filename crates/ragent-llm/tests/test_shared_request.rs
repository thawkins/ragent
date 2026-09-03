#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-llm/src/shared_request.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_llm::shared_request::SharedChatRequest;
use ragent_types::llm::{ChatContent, ChatMessage, ToolDefinition};
use std::sync::Arc;

#[test]
fn shared_chat_request_is_cheaply_cloneable() {
    let body = SharedChatRequest::new(
        vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hello".to_string()),
        }],
        vec![],
    );
    let clone = body.clone();
    // Cheap clone — same pointer, no deep copy.
    assert!(Arc::ptr_eq(&body.messages, &clone.messages));
    assert!(Arc::ptr_eq(&body.tools, &clone.tools));
}

#[test]
fn shared_chat_request_len_and_is_empty() {
    let empty = SharedChatRequest::new(vec![], vec![]);
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);

    let non_empty = SharedChatRequest::new(
        vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("x".to_string()),
        }],
        vec![],
    );
    assert!(!non_empty.is_empty());
    assert_eq!(non_empty.len(), 1);
}

#[test]
fn from_arc_preserves_existing_allocation() {
    let messages: Arc<Vec<ChatMessage>> = Arc::new(vec![ChatMessage {
        role: "user".to_string(),
        content: ChatContent::Text("hi".to_string()),
    }]);
    let tools: Arc<Vec<ToolDefinition>> = Arc::new(vec![]);
    let body = SharedChatRequest::from_arc(Arc::clone(&messages), Arc::clone(&tools));
    // No new allocation; the same `Arc`s are reused.
    assert!(Arc::ptr_eq(&body.messages, &messages));
    assert!(Arc::ptr_eq(&body.tools, &tools));
}

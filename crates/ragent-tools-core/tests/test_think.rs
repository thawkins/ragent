//! Integration tests for the `ragent-tools-core` `ThinkTool`.
//!
//! Relocated from the inline `#[cfg(test)]` module in `src/think.rs`
//! (T-008 of the testconsolidate spec). `ThinkTool` and `ToolContext` are
//! public types, so no `#[path]` re-import is needed.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use ragent_tools_core::think::ThinkTool;
use ragent_tools_core::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::json;

#[tokio::test]
async fn test_think_tool_returns_full_thought_in_metadata() {
    let tool = ThinkTool;
    let ctx = ToolContext {
        session_id: "session-1".to_string(),
        working_dir: PathBuf::from("/tmp"),
        event_bus: Arc::new(EventBus::new(16)),
        read_timestamps: Arc::new(RwLock::new(HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    };
    let thought = "This is the full reasoning content that should remain visible.";

    let output = tool
        .execute(json!({ "thought": thought }), &ctx)
        .await
        .expect("think tool should succeed");

    assert!(output.content.is_empty());
    let metadata = output
        .metadata
        .expect("think output should include metadata");
    assert_eq!(metadata["thinking"], true);
    assert_eq!(metadata["thought"], thought);
}

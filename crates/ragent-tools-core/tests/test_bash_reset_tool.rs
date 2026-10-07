//! Integration tests for the `bash_reset` tool (audit T-702).
//!
//! `BashResetTool` deletes the session's persistent shell state file (if any)
//! and reports success. The state-file path is derived from the session id, so
//! these tests place a sentinel file at that path, reset the shell, and assert
//! the sentinel is gone.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use ragent_tools_core::bash::state_file_path;
use ragent_tools_core::bash_reset::BashResetTool;
use ragent_tools_core::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::json;

fn make_ctx(session_id: &str) -> ToolContext {
    let dir = PathBuf::from("target/temp");
    ToolContext {
        session_id: session_id.to_string(),
        working_dir: dir.clone(),
        event_bus: Arc::new(EventBus::new(16)),
        read_timestamps: Arc::new(RwLock::new(HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: vec![dir],
    }
}

#[tokio::test]
async fn test_bash_reset_removes_existing_state_file() {
    let session_id = "ragent-test-bash-reset-a";
    let state_file = PathBuf::from(state_file_path(session_id));
    std::fs::create_dir_all(state_file.parent().expect("state file has a parent"))
        .expect("create state dir");
    std::fs::write(&state_file, b"PWD=somewhere").expect("write sentinel state");
    assert!(state_file.exists());

    let output = BashResetTool
        .execute(json!({}), &make_ctx(session_id))
        .await
        .expect("bash_reset execute");

    assert!(
        output.content.starts_with("Shell state reset."),
        "unexpected content: {}",
        output.content
    );
    assert!(output.metadata.is_none());
    assert!(
        !state_file.exists(),
        "reset must delete the session state file"
    );
}

#[tokio::test]
async fn test_bash_reset_is_noop_when_no_state_exists() {
    let session_id = "ragent-test-bash-reset-b";
    let state_file = PathBuf::from(state_file_path(session_id));
    let _ = std::fs::remove_file(&state_file);
    assert!(!state_file.exists());

    let output = BashResetTool
        .execute(json!({}), &make_ctx(session_id))
        .await
        .expect("bash_reset execute on a clean session");
    assert!(output.content.contains("Shell state reset."));
}

#[test]
fn test_bash_reset_metadata_surface() {
    let tool = BashResetTool;
    assert_eq!(tool.name(), "bash_reset");
    assert_eq!(tool.permission_category(), "bash:execute");
    assert_eq!(tool.parameters_schema()["type"], "object");
}

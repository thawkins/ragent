//! Integration tests for the `agent_complete` tool (audit T-702).
//!
//! Verifies the terminal-summary contract: a `summary` is required, the
//! success output echoes it, the metadata carries the `agent_complete` flag,
//! and a `TaskCompleted` event is published on the session event bus.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use ragent_tools_core::agent_complete::AgentCompleteTool;
use ragent_tools_core::event::{Event, EventBus};
use ragent_tools_core::{Tool, ToolContext};
use serde_json::json;

fn make_ctx(event_bus: Arc<EventBus>) -> ToolContext {
    let dir = PathBuf::from("target/temp");
    ToolContext {
        session_id: "agent-complete-test".to_string(),
        working_dir: dir.clone(),
        event_bus,
        read_timestamps: Arc::new(RwLock::new(HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: vec![dir],
    }
}

#[tokio::test]
async fn test_agent_complete_returns_summary_and_metadata() {
    let bus = Arc::new(EventBus::new(16));
    let ctx = make_ctx(Arc::clone(&bus));

    let output = AgentCompleteTool
        .execute(json!({ "summary": "shipped the feature" }), &ctx)
        .await
        .expect("agent_complete execute");

    assert!(output.content.contains("shipped the feature"));
    assert!(output.content.starts_with("[ok] Task complete."));
    let metadata = output.metadata.expect("agent_complete metadata");
    assert_eq!(metadata["agent_complete"], true);
    assert_eq!(metadata["summary"], "shipped the feature");
}

#[tokio::test]
async fn test_agent_complete_publishes_task_completed_event() {
    let bus = Arc::new(EventBus::new(16));
    let mut rx = bus.subscribe();
    let ctx = make_ctx(Arc::clone(&bus));

    AgentCompleteTool
        .execute(json!({ "summary": "done" }), &ctx)
        .await
        .expect("agent_complete execute");

    match rx.try_recv().expect("a TaskCompleted event") {
        Event::TaskCompleted {
            session_id,
            summary,
        } => {
            assert_eq!(session_id, "agent-complete-test");
            assert_eq!(summary, "done");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn test_agent_complete_missing_summary_is_error() {
    let bus = Arc::new(EventBus::new(16));
    let err = AgentCompleteTool
        .execute(json!({ "task_id": "task-1" }), &make_ctx(bus))
        .await
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("Missing required 'summary' parameter for agent_complete"),
        "unexpected error message: {err}"
    );
}

#[test]
fn test_agent_complete_metadata_surface() {
    let tool = AgentCompleteTool;
    assert_eq!(tool.name(), "agent_complete");
    assert_eq!(tool.permission_category(), "none");
    assert_eq!(tool.parameters_schema()["required"][0], "summary");
}

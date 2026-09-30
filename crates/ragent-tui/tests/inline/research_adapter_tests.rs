//! Inline tests for `research_adapter.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use ragent_research::ResearchManager;
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn test_parse_websearch_output() {
    let text = "1. Example Site\n   https://example.com\n   A useful example page.\n2. Another Site\n   https://another.example.com\n";
    let hits = parse_websearch_output(text);
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].title, "Example Site");
    assert_eq!(hits[0].url, "https://example.com");
    assert_eq!(hits[0].snippet, "A useful example page.");
    assert_eq!(hits[1].title, "Another Site");
    assert_eq!(hits[1].url, "https://another.example.com");
}

#[test]
fn test_build_research_session_wires_available_tools() {
    use ragent_agent::{event::EventBus, tool::create_default_registry};
    let registry = Arc::new(create_default_registry());
    let manager = ResearchManager::new("research");
    let session = build_research_session(
        &registry,
        manager,
        "test-session".into(),
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        Arc::new(EventBus::new(256)),
        None,
        None,
        None,
        None,
        None,
    );
    let debug = format!("{:?}", session);
    assert!(
        debug.contains("has_web: true"),
        "default registry should provide websearch+webfetch tools: {debug}"
    );
    assert!(
        debug.contains("has_local: true"),
        "default registry should provide glob/grep/read/list tools: {debug}"
    );
}

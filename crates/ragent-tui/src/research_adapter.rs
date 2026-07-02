//! Re-export the shared research adapter from [`ragent_core`].
//!
//! The adapter lives in the agent crate so the TUI, HTTP server, and CLI can all
//! build research sessions with the same web/local gathering wiring.

pub use ragent_core::research_adapter::*;

#[cfg(test)]
mod tests {
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
        use ragent_core::{event::EventBus, tool::create_default_registry};
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
}

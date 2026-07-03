//! Re-export the shared research adapter from [`ragent_agent`].
//!
//! The adapter lives in the agent crate so the TUI, HTTP server, and CLI can all
//! build research sessions with the same web/local gathering wiring.

pub use ragent_agent::research_adapter::*;

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

// ── TUI-specific observer and completer structs ──────────────────────────────
//
// Previously these lived inline in `app.rs` (lines 51–147).  They were moved
// here in REMPLAN.md M5 / T5.8 to reduce the size of `app.rs`.

use std::sync::Arc;

use ragent_agent::event::{Event, EventBus};

/// TUI observer that mirrors research session events to the log panel and
/// status bar using [`Event::AgentNotice`].
///
/// Each event is encoded with [`crate::research_progress::encode_progress_event`]
/// (sentinel-prefixed JSON) so the TUI can route it to the structured
/// [`ResearchProgress`](crate::research_progress::ResearchProgress) log list
/// rendered in the message window.
pub(crate) struct TuiResearchObserver {
    pub app_event_bus: Arc<EventBus>,
    pub session_id: String,
    /// Research item name, captured at spawn time so progress events carry it.
    pub name: String,
    /// Research topic, captured at spawn time so progress events carry it.
    pub topic: String,
}

impl ragent_research::SessionObserver for TuiResearchObserver {
    fn on_event(&self, event: ragent_research::SessionEvent) {
        let message =
            crate::research_progress::encode_progress_event(&self.name, &self.topic, &event);
        self.app_event_bus.publish(Event::AgentNotice {
            session_id: self.session_id.clone(),
            message,
        });
    }
}

/// Connects the `ragent-prompt_opt` crate to the session's active LLM provider.
///
/// `RagentCompleter` implements [`Completer`] by building an [`LlmClient`] from
/// the configured provider, sending the system+user message pair, and collecting
/// the streaming `TextDelta` events into a single `String`.
pub(crate) struct RagentCompleter {
    pub registry: Arc<ragent_agent::provider::ProviderRegistry>,
    pub storage: Arc<ragent_agent::storage::Storage>,
    pub provider_id: String,
    pub model_id: String,
}

#[async_trait::async_trait]
impl ragent_prompt_opt::Completer for RagentCompleter {
    async fn complete(&self, system: &str, user: &str) -> anyhow::Result<String> {
        use anyhow::Context as _;
        use futures::StreamExt as _;
        use ragent_agent::llm::{ChatContent, ChatMessage, ChatRequest, StreamEvent};

        let api_key = self
            .storage
            .get_provider_auth(&self.provider_id)
            .context("reading API key")?
            .unwrap_or_default();

        let provider = self
            .registry
            .get(&self.provider_id)
            .with_context(|| format!("provider '{}' not found", self.provider_id))?;

        let client = provider
            .create_client(&api_key, None, &Default::default())
            .await
            .context("creating LLM client")?;

        let request = ChatRequest {
            model: self.model_id.clone(),
            messages: Arc::new(vec![ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text(user.to_string()),
            }]),
            tools: Arc::new(vec![]),
            temperature: None,
            top_p: None,
            max_tokens: None,
            system: Some(std::sync::Arc::from(system)),
            options: Default::default(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: None,
            thinking: None,
        };

        let mut stream = client.chat(request).await.context("starting LLM stream")?;
        let mut result = String::new();
        while let Some(event) = stream.next().await {
            if let StreamEvent::TextDelta { text } = event {
                result.push_str(&text);
            }
        }
        Ok(result)
    }
}

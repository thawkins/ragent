//! Re-export the shared research adapter from [`ragent_agent`].
//!
//! The adapter lives in the agent crate so the TUI, HTTP server, and CLI can all
//! build research sessions with the same web/local gathering wiring.

pub(crate) use ragent_agent::research_adapter::*;

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
    pub(crate) app_event_bus: Arc<EventBus>,
    pub(crate) session_id: String,
    /// Research item name, captured at spawn time so progress events carry it.
    pub(crate) name: String,
    /// Research topic, captured at spawn time so progress events carry it.
    pub(crate) topic: String,
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

/// One-shot LLM helper for the `/swarm` decomposition call: builds a client
/// from the session's active provider, sends the system+user message pair, and
/// collects the streaming `TextDelta` events into a single `String`.
pub(crate) struct RagentCompleter {
    pub(crate) registry: Arc<ragent_agent::provider::ProviderRegistry>,
    pub(crate) storage: Arc<ragent_agent::storage::Storage>,
    pub(crate) provider_id: String,
    pub(crate) model_id: String,
}

impl RagentCompleter {
    pub(crate) async fn complete(&self, system: &str, user: &str) -> anyhow::Result<String> {
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

#[cfg(test)]
#[path = "../tests/inline/research_adapter_tests.rs"]
mod tests;

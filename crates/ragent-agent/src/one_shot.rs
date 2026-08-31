//! One-shot LLM dispatch helper.
//!
//! Provides a lightweight, provider-agnostic way to send a single user prompt
//! to the active LLM without spinning up a full agent loop. Used by the
//! `/research cluster` concept-extraction flow (FR-005, NFR-001).

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, bail};
use futures::StreamExt;
use ragent_llm::ProviderRegistry;
use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, StreamEvent};
use ragent_storage::Storage;
use uuid::Uuid;

use crate::agent::ModelRef;

/// Resolve an API key for `provider_id` from the environment and (optionally)
/// persistent storage.
///
/// This is a lightweight standalone version of the resolver used by
/// [`SessionProcessor`](crate::session::SessionProcessor); it covers the
/// standard environment-variable providers and falls back to the database.
fn resolve_api_key(provider_id: &str, storage: Option<&Arc<Storage>>) -> Result<String> {
    if provider_id == "router" {
        return Ok(String::new());
    }
    if provider_id == "ollama" {
        return Ok(std::env::var("OLLAMA_API_KEY").unwrap_or_default());
    }

    let env_vars: &[&str] = match provider_id {
        "anthropic" => &["ANTHROPIC_API_KEY"],
        "openai" => &["OPENAI_API_KEY"],
        "gemini" => &["GEMINI_API_KEY"],
        "huggingface" => &["HF_TOKEN", "HUGGING_FACE_HUB_TOKEN"],
        "generic_openai" => &["GENERIC_OPENAI_API_KEY", "OPENAI_API_KEY"],
        "ollama_cloud" => &["OLLAMA_API_KEY"],
        "azure_foundry" => &["AZURE_AI_FOUNDRY_API_KEY"],
        "openrouter" => &["OPENROUTER_API_KEY"],
        "copilot" => &["GITHUB_COPILOT_TOKEN", "GITHUB_TOKEN"],
        _ => &[],
    };

    for var in env_vars {
        if let Ok(key) = std::env::var(var)
            && !key.is_empty()
        {
            return Ok(key);
        }
    }

    if let Some(storage) = storage {
        if let Ok(Some(key)) = storage.get_provider_auth(provider_id)
            && !key.is_empty()
        {
            return Ok(key);
        }
    }

    bail!(
        "No API key found for provider '{provider_id}'. Set the appropriate environment variable \
         or run `ragent auth {provider_id} <key>` to store one."
    )
}

/// Send a single user prompt to the model described by `model_ref` and return
/// the complete response text.
///
/// The call is bounded by a hard 5-minute wall-clock timeout to satisfy
/// NFR-001. Streaming responses are collected into a single string; only
/// [`StreamEvent::TextDelta`] contributes to the output, and provider errors
/// abort the call.
///
/// # Arguments
///
/// * `provider_registry` — registry of available providers.
/// * `storage` — optional storage used to look up stored API keys.
/// * `model_ref` — provider/model binding.
/// * `system` — optional system prompt.
/// * `prompt` — the user prompt (e.g. the output of [`ragent_research::build_concept_extraction_prompt`]).
/// * `max_tokens` — optional cap on the model's output tokens.
///
/// # Errors
///
/// Returns an error if the provider/model is unknown, no API key is found, the
/// client cannot be created, or the stream emits an error / times out.
pub async fn send_one_shot(
    provider_registry: Arc<ProviderRegistry>,
    storage: Option<Arc<Storage>>,
    model_ref: ModelRef,
    system: Option<String>,
    prompt: String,
    max_tokens: Option<u32>,
) -> Result<String> {
    let provider = provider_registry
        .get(&model_ref.provider_id)
        .ok_or_else(|| anyhow::anyhow!("Provider '{}' not found", model_ref.provider_id))?;

    let api_key = resolve_api_key(&model_ref.provider_id, storage.as_ref())?;
    let client = provider
        .create_client(&api_key, None, &HashMap::new())
        .await
        .map_err(|e| anyhow::anyhow!("failed to create LLM client: {e}"))?;

    let request = ChatRequest {
        model: model_ref.model_id.clone(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text(prompt),
        }]),
        tools: Arc::new(Vec::new()),
        temperature: None,
        top_p: None,
        max_tokens,
        system: system.map(|s| Arc::from(s.into_boxed_str())),
        options: HashMap::new(),
        thinking: None,
        session_id: None,
        request_id: Some(Uuid::new_v4().to_string()),
        stream_timeout_secs: None,
    };

    let overall_timeout = std::time::Duration::from_secs(300);
    let response_fut = async {
        let mut stream = client.chat(request).await?;
        let mut chunks = Vec::new();
        while let Some(event) = stream.next().await {
            match event {
                StreamEvent::TextDelta { text } => chunks.push(text),
                StreamEvent::Error { message } => {
                    bail!("LLM stream error: {message}")
                }
                StreamEvent::Finish { .. } => break,
                _ => {}
            }
        }
        Ok(chunks.join(""))
    };

    match tokio::time::timeout(overall_timeout, response_fut).await {
        Ok(result) => result,
        Err(_) => bail!("LLM one-shot request timed out after 300s"),
    }
}

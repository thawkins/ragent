//! Microsoft Azure AI Foundry provider implementation.
//!
//! Azure AI Foundry provides OpenAI-compatible chat completions via a single
//! REST endpoint. Authentication uses the `api-key` header (Azure convention)
//! or `Authorization: Bearer` as a fallback. Model discovery is supported via
//! the `/openai/models?api-version=2024-10-21` endpoint (Azure OpenAI Service
//! compatible).
//!
//! # Configuration
//!
//! Set `AZURE_AI_FOUNDRY_API_KEY` as the API key, and optionally set
//! `AZURE_AI_FOUNDRY_BASE` for a custom endpoint. The endpoint can also be
//! configured via `ragent.json` under `provider.azure_foundry.base_url`.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

use crate::llm::LlmClient;
use crate::provider::openai::{OpenAiClient, openai_default_models};
use crate::{ModelInfo, Provider};

const DEFAULT_AZURE_FOUNDRY_HOST: &str = "https://services.ai.azure.com";

/// Provider implementation for Microsoft Azure AI Foundry.
pub struct AzureFoundryProvider;

impl AzureFoundryProvider {
    const ENDPOINT_OPTION_KEY: &'static str = "endpoint_url";
    const DEFAULT_ENV_ENDPOINT_KEY: &'static str = "AZURE_AI_FOUNDRY_BASE";
}

#[async_trait::async_trait]
impl Provider for AzureFoundryProvider {
    fn id(&self) -> &'static str {
        "azure_foundry"
    }

    fn name(&self) -> &'static str {
        "Azure AI Foundry"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        let mut models = openai_default_models("azure_foundry");
        // Add Azure-specific model variants with reasoning support
        models.push(ModelInfo {
            id: "o1".to_string(),
            provider_id: "azure_foundry".to_string(),
            name: "Azure o1".to_string(),
            cost: ragent_config::Cost {
                input: 15.0,
                output: 60.0,
            },
            capabilities: ragent_config::Capabilities {
                reasoning: true,
                streaming: true,
                vision: true,
                tool_use: true,
                thinking_levels: crate::provider::thinking::openai_thinking_levels_for_model("o1"),
            },
            context_window: 200_000,
            max_output: Some(100_000),
            request_multiplier: None,
            thinking_config: None,
        });
        models.push(ModelInfo {
            id: "o3-mini".to_string(),
            provider_id: "azure_foundry".to_string(),
            name: "Azure o3-mini".to_string(),
            cost: ragent_config::Cost {
                input: 1.10,
                output: 4.40,
            },
            capabilities: ragent_config::Capabilities {
                reasoning: true,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: crate::provider::thinking::openai_thinking_levels_for_model(
                    "o3-mini",
                ),
            },
            context_window: 200_000,
            max_output: Some(100_000),
            request_multiplier: None,
            thinking_config: None,
        });
        models
    }

    async fn create_client(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        let env_endpoint = std::env::var(Self::DEFAULT_ENV_ENDPOINT_KEY)
            .ok()
            .filter(|s| !s.trim().is_empty());
        let configured_endpoint = options
            .get(Self::ENDPOINT_OPTION_KEY)
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty());
        let resolved_base = configured_endpoint
            .or(base_url)
            .or(env_endpoint.as_deref())
            .unwrap_or(DEFAULT_AZURE_FOUNDRY_HOST)
            .trim_end_matches('/')
            .to_string();

        let client = AzureFoundryClient::new(api_key, &resolved_base);
        tracing::info!(chat_endpoint = %format!("{}/openai/v1/chat/completions", resolved_base), models_endpoint = %format!("{}/openai/v1/models", resolved_base), "Azure AI Foundry provider connected");
        Ok(Box::new(client))
    }
}

/// HTTP client for Azure AI Foundry.
///
/// Wraps the OpenAI-compatible client but sends the `api-key` header
/// (Azure convention) instead of the standard `Authorization: Bearer`.
pub(crate) struct AzureFoundryClient {
    api_key: String,
    base_url: String,
    inner: OpenAiClient,
}

impl AzureFoundryClient {
    pub(crate) fn new(api_key: &str, base_url: &str) -> Self {
        let base = base_url.trim_end_matches('/').to_string();
        Self {
            api_key: api_key.to_string(),
            base_url: base.clone(),
            inner: OpenAiClient::new(api_key, &base),
        }
    }

    /// Discover available models from the Azure AI Foundry endpoint.
    #[allow(dead_code)]
    pub(crate) async fn discover_models(&self) -> Result<Vec<ModelInfo>> {
        discover_azure_foundry_models(&self.api_key, &self.base_url).await
    }
}

/// Standalone function to discover Azure AI Foundry models.
///
/// Used by the TUI for dynamic model discovery without needing to construct
/// a full client.
pub async fn discover_azure_foundry_models(
    api_key: &str,
    base_url: &str,
) -> Result<Vec<ModelInfo>> {
    let base = base_url.trim_end_matches('/').to_string();
    // Use the Azure OpenAI Service /models endpoint with api-version
    let url = format!("{}/openai/models?api-version=2024-10-21", base);
    let response = crate::provider::http_client::create_http_client()
        .get(&url)
        .header("api-key", api_key)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .inspect_err(|e| {
            tracing::warn!(url = %url, error = %e, "Azure AI Foundry model discovery failed");
        })
        .with_context(|| format!("Failed to connect to Azure AI Foundry at {url}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        bail!("Azure AI Foundry API returned status {status}: {body}");
    }

    let data: AzureFoundryModelsResponse = response
        .json()
        .await
        .context("Failed to parse Azure AI Foundry models response")?;

    let models: Vec<ModelInfo> = data
        .data
        .into_iter()
        .map(|m| {
            let supports_tools =
                m.id.starts_with("gpt-4") || m.id == "o1" || m.id.starts_with("o3");
            let supports_vision = m.id.contains("o1") || m.id.contains("4o");
            let supports_reasoning = m.id == "o1" || m.id.starts_with("o3");

            ModelInfo {
                id: m.id.clone(),
                provider_id: "azure_foundry".to_string(),
                name: m.id.clone(),
                cost: ragent_config::Cost {
                    input: 0.0,
                    output: 0.0,
                },
                capabilities: ragent_config::Capabilities {
                    reasoning: supports_reasoning,
                    streaming: true,
                    vision: supports_vision,
                    tool_use: supports_tools,
                    thinking_levels: if supports_reasoning {
                        crate::provider::thinking::openai_thinking_levels_for_model(&m.id)
                    } else {
                        Vec::new()
                    },
                },
                context_window: 128_000,
                max_output: None,
                request_multiplier: None,
                thinking_config: None,
            }
        })
        .collect();

    Ok(models)
}

#[async_trait::async_trait]
impl LlmClient for AzureFoundryClient {
    async fn chat(
        &self,
        request: crate::llm::ChatRequest,
    ) -> Result<std::pin::Pin<Box<dyn futures::Stream<Item = crate::llm::StreamEvent> + Send>>>
    {
        // Azure AI Foundry uses the OpenAI-compatible endpoint but with api-key header.
        // We override the request to use api-key instead of Authorization: Bearer.
        let url = if self.base_url.contains(".openai.azure.com") {
            if self.base_url.ends_with("/openai") {
                format!("{}/v1/chat/completions", self.base_url)
            } else {
                format!("{}/openai/v1/chat/completions", self.base_url)
            }
        } else {
            format!("{}/openai/v1/chat/completions", self.base_url)
        };
        let body = self.inner.build_request_body(&request);

        tracing::info!(
            endpoint = %url,
            model = %request.model,
            "[azure_foundry/{}] Sending chat request", request.model
        );

        let client = crate::provider::http_client::create_streaming_http_client();
        let api_key = self.api_key.clone();
        let url_for_error = url.clone();
        let response = crate::provider::http_client::execute_with_retry(
            move || {
                let client = client.clone();
                let api_key = api_key.clone();
                let url = url.clone();
                let body = body.clone();
                async move {
                    client
                        .post(&url)
                        .header("api-key", api_key)
                        .header("content-type", "application/json")
                        .json(&body)
                        .send()
                        .await
                }
            },
            4,
        )
        .await
        .inspect_err(|e| {
            tracing::warn!(url = %url_for_error, error = %e, "Azure AI Foundry chat request failed after retries");
        })
        .with_context(|| format!("Failed to send request to Azure AI Foundry at {url_for_error}"))?;

        // Reuse the OpenAI SSE stream parser since the response format is identical
        self.inner.parse_sse_stream(response).await
    }
}

/// Response from the Azure AI Foundry `/models` endpoint.
#[derive(Debug, Deserialize)]
struct AzureFoundryModelsResponse {
    data: Vec<AzureFoundryModelEntry>,
}

/// A single model entry in the Azure AI Foundry model list.
#[derive(Debug, Deserialize)]
struct AzureFoundryModelEntry {
    id: String,
    #[allow(dead_code)]
    object: String,
}

//! Generic OpenAI-compatible provider implementation.
//!
//! This provider mirrors the OpenAI Chat Completions flow but uses a
//! configurable API base URL, including custom ports.

use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

use crate::llm::LlmClient;
use crate::provider::openai::{OPENAI_API_BASE, OpenAiClient, openai_default_models};
use crate::provider::{ModelInfo, Provider};

/// Provider implementation for arbitrary OpenAI-compatible endpoints.
pub struct GenericOpenAiProvider;

impl GenericOpenAiProvider {
    const ENDPOINT_OPTION_KEY: &'static str = "endpoint_url";
    const DEFAULT_ENV_ENDPOINT_KEY: &'static str = "GENERIC_OPENAI_API_BASE";
}

#[async_trait::async_trait]
impl Provider for GenericOpenAiProvider {
    fn id(&self) -> &str {
        "generic_openai"
    }

    fn name(&self) -> &str {
        "Generic OpenAI API"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        openai_default_models("generic_openai")
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
            .unwrap_or(OPENAI_API_BASE);
        let client = OpenAiClient::new(api_key, resolved_base);
        Ok(Box::new(client))
    }
}

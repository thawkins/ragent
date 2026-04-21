//! LLM provider traits and types
//!
//! This module defines the core abstractions for LLM providers.

use crate::error::RagentError;
use crate::message::Message;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Trait for LLM providers
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Provider name (e.g., "anthropic", "openai")
    fn name(&self) -> &str;

    /// Generate a response from the LLM
    async fn generate(
        &self,
        messages: &[Message],
        temperature: Option<f32>,
        max_tokens: Option<u32>,
        tools: Option<&[ToolDefinition]>,
        stream: bool,
    ) -> Result<LlmResponse, RagentError>;

    /// List available models
    async fn list_models(&self) -> Result<Vec<ModelInfo>, RagentError>;

    /// Validate API credentials
    async fn validate_credentials(&self) -> Result<(), RagentError>;
}

/// LLM response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: String,
    pub tool_calls: Option<Vec<serde_json::Value>>,
    pub finish_reason: Option<String>,
    pub usage: Option<Usage>,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub context_window: u32,
    pub max_output_tokens: Option<u32>,
    pub capabilities: Vec<String>,
}

/// Tool definition for function calling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub api_key: Option<String>,
    pub api_base: Option<String>,
    pub default_model: Option<String>,
    pub extra: HashMap<String, serde_json::Value>,
}

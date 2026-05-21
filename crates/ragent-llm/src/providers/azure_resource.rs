//! Azure Resource Provider — file-based model catalog.
//!
//! Reads model definitions from `azureresources.json` so users can register
//! Azure-hosted endpoints (e.g. Azure OpenAI, Azure AI Foundry, custom
//! endpoints) without rebuilding the application.
//!
//! # `azureresources.json` schema
//!
//! ```json
//! {
//!   "version": "1",
//!   "resources": [
//!     {
//!       "id": "my-gpt-4o",
//!       "name": "My Azure GPT-4o",
//!       "endpoint": "https://my-resource.openai.azure.com",
//!       "api_key_env": "MY_AOAI_KEY",
//!       "context_window": 128000,
//!       "capabilities": ["streaming", "vision", "tool_use"],
//!       "thinking": { "enabled": false }
//!     }
//!   ]
//! }
//! ```
//!
//! The file is searched in this order:
//! 1. `~/.config/ragent/azureresources.json`
//! 2. `.ragent/azureresources.json` (current working directory)

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::llm::LlmClient;
use crate::provider::azure_foundry::AzureFoundryClient;
use crate::{ModelInfo, Provider};
use ragent_config::{Capabilities, Cost};

/// A single Azure resource entry parsed from `azureresources.json`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AzureResourceEntry {
    /// Unique identifier for this resource (used as model ID).
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Base endpoint URL for the Azure resource.
    pub endpoint: String,
    /// Optional inline API key (discouraged — prefer `api_key_env`).
    #[serde(default)]
    pub api_key: Option<String>,
    /// Name of the environment variable that holds the API key.
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// Maximum context-window size in tokens.
    #[serde(default)]
    pub context_window: Option<usize>,
    /// Optional capability tags (e.g. `"streaming"`, `"vision"`, `"tool_use"`).
    #[serde(default)]
    pub capabilities: Option<Vec<String>>,
    /// Optional thinking / reasoning configuration.
    #[serde(default)]
    pub thinking: Option<ragent_types::ThinkingConfig>,
}

/// Top-level structure of `azureresources.json`.
#[derive(Debug, Deserialize)]
struct AzureResourcesFile {
    version: String,
    resources: Vec<AzureResourceEntry>,
}

/// Parse an `azureresources.json` file at `path` and return validated entries.
///
/// Validation rules:
/// * `version` must be `"1"`.
/// * Each entry must have non-empty `id`, `name`, and `endpoint`.
/// * Each entry must have at least one of `api_key` or `api_key_env`.
/// * Entries missing required fields are skipped with a `tracing::warn!` log.
/// * Duplicate IDs are deduplicated (first wins) with a warning.
///
/// Returns an empty `Vec` on any fatal error (missing file, malformed JSON,
/// etc.).
pub fn parse_azure_resources(path: &Path) -> Result<Vec<AzureResourceEntry>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let file: AzureResourcesFile = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse JSON in {}", path.display()))?;

    if file.version != "1" {
        anyhow::bail!(
            "Unsupported azureresources.json version: expected '1', got '{}'",
            file.version
        );
    }

    let mut seen_ids = HashMap::new();
    let mut entries = Vec::new();

    for entry in file.resources {
        // Validate mandatory fields
        if entry.id.trim().is_empty() {
            tracing::warn!("Skipping Azure resource entry: missing 'id'");
            continue;
        }
        if entry.name.trim().is_empty() {
            tracing::warn!("Skipping Azure resource entry: missing 'name'");
            continue;
        }
        if entry.endpoint.trim().is_empty() {
            tracing::warn!("Skipping Azure resource entry: missing 'endpoint'");
            continue;
        }
        if entry.api_key.is_none() && entry.api_key_env.is_none() {
            tracing::warn!(
                resource_id = %entry.id,
                "Skipping Azure resource entry: neither 'api_key' nor 'api_key_env' provided"
            );
            continue;
        }

        // Deduplicate IDs
        if seen_ids.contains_key(&entry.id) {
            tracing::warn!(
                resource_id = %entry.id,
                "Skipping duplicate Azure resource entry"
            );
            continue;
        }

        seen_ids.insert(entry.id.clone(), ());
        entries.push(entry);
    }

    Ok(entries)
}

/// Resolve the path to `azureresources.json`.
///
/// Tries, in order:
/// 1. `~/.config/ragent/azureresources.json`
/// 2. `.ragent/azureresources.json` in the current working directory
fn resolve_config_path() -> Option<PathBuf> {
    // 1. User config directory
    if let Some(home) = dirs::home_dir() {
        let p = home.join(".config").join("ragent").join("azureresources.json");
        if p.exists() {
            return Some(p);
        }
    }

    // 2. Project-local .ragent directory
    let p = PathBuf::from(".ragent").join("azureresources.json");
    if p.exists() {
        return Some(p);
    }

    None
}

/// Provider implementation backed by a user-supplied `azureresources.json` file.
///
/// Models are loaded dynamically from the file; if the file is absent or
/// malformed the provider advertises an empty catalog.
pub struct AzureResourceProvider {
    config_path: PathBuf,
}

impl AzureResourceProvider {
    /// Creates a new provider using the default config-file resolution.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config_path: resolve_config_path().unwrap_or_else(|| {
                PathBuf::from(".ragent").join("azureresources.json")
            }),
        }
    }

    /// Creates a new provider with an explicit config file path.
    #[must_use]
    pub fn with_path(path: PathBuf) -> Self {
        Self { config_path: path }
    }
}

impl Default for AzureResourceProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Provider for AzureResourceProvider {
    fn id(&self) -> &str {
        "azure_resource"
    }

    fn name(&self) -> &str {
        "Azure Resource (File)"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        match parse_azure_resources(&self.config_path) {
            Ok(entries) => entries.into_iter().map(entry_to_model_info).collect(),
            Err(e) => {
                tracing::debug!(
                    path = %self.config_path.display(),
                    error = %e,
                    "AzureResourceProvider: no model catalog loaded"
                );
                Vec::new()
            }
        }
    }

    async fn create_client(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        _options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        let resolved = base_url.unwrap_or_default().trim_end_matches('/').to_string();
        let client = AzureFoundryClient::new(api_key, &resolved);
        tracing::info!(
            chat_endpoint = %format!("{}/openai/v1/chat/completions", resolved),
            "Azure Resource provider connected"
        );
        Ok(Box::new(client))
    }
}

/// Convert an [`AzureResourceEntry`] into a [`ModelInfo`].
fn entry_to_model_info(entry: AzureResourceEntry) -> ModelInfo {
    let caps = entry.capabilities.as_ref();
    ModelInfo {
        id: entry.id.clone(),
        provider_id: "azure_resource".to_string(),
        name: entry.name,
        cost: Cost {
            input: 0.0,
            output: 0.0,
        },
        capabilities: {
            if let Some(c) = caps {
                // When capabilities are explicitly listed, only enable the ones listed
                Capabilities {
                    reasoning: c.contains(&"reasoning".to_string()),
                    streaming: c.contains(&"streaming".to_string()),
                    vision: c.contains(&"vision".to_string()),
                    tool_use: c.contains(&"tool_use".to_string()),
                    thinking_levels: Vec::new(),
                }
            } else {
                // When capabilities are not specified, use sensible defaults
                Capabilities {
                    reasoning: false,
                    streaming: true,
                    vision: false,
                    tool_use: true,
                    thinking_levels: Vec::new(),
                }
            }
        },
        context_window: entry.context_window.unwrap_or(128_000),
        max_output: None,
        request_multiplier: None,
        thinking_config: entry.thinking,
    }
}

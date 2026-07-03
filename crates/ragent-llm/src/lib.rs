//! LLM provider implementations for ragent
//!
//! This crate provides:
//! - LLM provider trait implementations (Anthropic, OpenAI, Gemini, Ollama, etc.)
//! - Provider registry and discovery
//! - HTTP client utilities
//! - Model metadata and capabilities

pub mod llm;
pub mod providers;
pub mod shared_request;

/// Compatibility re-export matching the historic `provider` module path.
pub use providers as provider;

pub use providers::{
    ModelInfo, Provider, ProviderInfo, ProviderRegistry, UsageInfo, anthropic::AnthropicProvider,
    azure_resource::AzureResourceEntry, azure_resource::AzureResourceProvider,
    copilot::CopilotProvider, create_default_registry, gemini::GeminiProvider,
    generic_openai::GenericOpenAiProvider, huggingface::HuggingFaceProvider,
    ollama::OllamaProvider, ollama_cloud::OllamaCloudProvider, openai::OpenAiProvider,
};

pub use shared_request::SharedChatRequest;

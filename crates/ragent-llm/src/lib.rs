//! LLM provider implementations for ragent
//!
//! This crate provides:
//! - LLM provider trait implementations (Anthropic, OpenAI, Gemini, Ollama, etc.)
//! - Provider registry and discovery
//! - HTTP client utilities
//! - Model metadata and capabilities

pub mod embedded;
pub mod llm;
pub mod providers;

/// Compatibility re-export matching the historic `provider` module path.
pub use providers as provider;

/// Compatibility shim for provider modules that still refer to `crate::config`.
pub mod config {
    pub use ragent_config::{Capabilities, Cost};
}

/// Compatibility shim for provider modules that still refer to `crate::event`.
pub mod event {
    pub use ragent_types::event::FinishReason;
}

pub use providers::{
    ModelInfo, Provider, ProviderInfo, ProviderRegistry, UsageInfo, anthropic::AnthropicProvider,
    copilot::CopilotProvider, create_default_registry, gemini::GeminiProvider,
    generic_openai::GenericOpenAiProvider, huggingface::HuggingFaceProvider,
    ollama::OllamaProvider, ollama_cloud::OllamaCloudProvider, openai::OpenAiProvider,
};

pub use embedded::{
    ChatTemplate, EmbeddedBackend, EmbeddedModelArtifact, EmbeddedModelManifest, EmbeddedRuntime,
    EmbeddedRuntimeLifecycle, EmbeddedRuntimeSettings, EmbeddedRuntimeStatus, RuntimeAvailability,
    SUB_1G_MAX_BYTES,
};

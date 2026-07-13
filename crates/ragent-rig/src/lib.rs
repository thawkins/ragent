//! Rig framework adapter for ragent.
//!
//! This crate isolates all Rig-specific integration logic for ragent.
//! It provides optional adapters for:
//!
//! * Provider completion models (`CompletionModel`).
//! * Generalized streaming responses.
//! * Conversation-memory policies (`rig-memory`).
//! * Embedding models (`EmbeddingModel`).
//! * Vector-store indexes (`VectorStoreIndex`).
//! * Mock-model and VCR test utilities.
//! * Rig `Tool` wrappers for ragent's security-audited tool registry (T-013).
//!
//! The crate uses Cargo feature flags so that each Rig provider or backend is
//! compiled only when requested. Native ragent providers remain the default;
//! Rig backends are opt-in via `ragent.json` configuration.

#![warn(missing_docs)]

pub mod config;
pub mod error;

/// Internal adapter traits for the embedding path (T-003 / FR-007).
///
/// Compiled unconditionally: the [`embeddings_trait::RigEmbeddingBackend`]
/// trait and [`embeddings_trait::EmbeddingAdapter`] metadata handle have no
/// `rig-core` dependency, so they are available even when the `embeddings`
/// feature is off. The concrete Rig-backed implementation lives in
/// [`embeddings`] (compiled only when `embeddings` is enabled).
pub mod embeddings_trait;

/// Internal adapter traits for the completion path (FR-004 / FR-005).
///
/// Compiled whenever any provider feature is enabled, since the concrete
/// Rig-backed completion backend (T-004 / T-005) implements
/// [`completion::CompletionBackend`].
#[cfg(any(
    feature = "provider-openai",
    feature = "provider-anthropic",
    feature = "provider-gemini",
    feature = "provider-cohere",
    feature = "provider-deepseek",
    feature = "provider-groq",
    feature = "provider-huggingface",
    feature = "provider-mistral",
    feature = "provider-ollama",
    feature = "provider-perplexity",
    feature = "provider-together",
    feature = "provider-xai"
))]
pub mod completion;

#[cfg(feature = "embeddings")]
pub mod embeddings;

#[cfg(feature = "memory")]
pub mod memory;

#[cfg(feature = "mock")]
pub mod testing;

/// Marker for mock-model support (kept for backwards compatibility; the real
/// harness is in [`testing`]).
pub mod testing_marker;

#[cfg(any(
    feature = "provider-openai",
    feature = "provider-anthropic",
    feature = "provider-gemini",
    feature = "provider-cohere",
    feature = "provider-deepseek",
    feature = "provider-groq",
    feature = "provider-huggingface",
    feature = "provider-mistral",
    feature = "provider-ollama",
    feature = "provider-perplexity",
    feature = "provider-together",
    feature = "provider-xai"
))]
pub mod provider;

#[cfg(any(
    feature = "vector-store-sqlite",
    feature = "vector-store-memory",
    feature = "vector-store-http"
))]
pub mod vector_store;

/// Rig-backed semantic search helpers for `ragent-codeindex` (T-009 / FR-010).
#[cfg(feature = "rig-semantic")]
pub mod codeindex;

/// Rig-backed semantic search helpers for ragent's memory subsystem
/// (T-010 / FR-010). The memory counterpart of [`codeindex`].
#[cfg(feature = "memory-semantic")]
pub mod memory_semantic;

#[cfg(any(
    feature = "provider-openai",
    feature = "provider-anthropic",
    feature = "provider-gemini",
    feature = "provider-cohere",
    feature = "provider-deepseek",
    feature = "provider-groq",
    feature = "provider-huggingface",
    feature = "provider-mistral",
    feature = "provider-ollama",
    feature = "provider-perplexity",
    feature = "provider-together",
    feature = "provider-xai"
))]
pub mod registry;

#[cfg(any(
    feature = "provider-openai",
    feature = "provider-anthropic",
    feature = "provider-gemini",
    feature = "provider-cohere",
    feature = "provider-deepseek",
    feature = "provider-groq",
    feature = "provider-huggingface",
    feature = "provider-mistral",
    feature = "provider-ollama",
    feature = "provider-perplexity",
    feature = "provider-together",
    feature = "provider-xai"
))]
pub use registry::{RigProvider, register_rig_providers};

#[cfg(feature = "vcr")]
pub mod vcr;

#[cfg(feature = "research")]
pub mod research;

/// Rig `Tool` wrappers for ragent core tools (T-013 / FR-031).
///
/// Compiled whenever `rig-core` is available (any provider feature or `mock`),
/// so a Rig-backed agent can invoke ragent tools through the same
/// permission-gated `execute` path the native agent loop uses.
#[cfg(any(
    feature = "provider-openai",
    feature = "provider-anthropic",
    feature = "provider-gemini",
    feature = "provider-cohere",
    feature = "provider-deepseek",
    feature = "provider-groq",
    feature = "provider-huggingface",
    feature = "provider-mistral",
    feature = "provider-ollama",
    feature = "provider-perplexity",
    feature = "provider-together",
    feature = "provider-xai",
    feature = "mock"
))]
pub mod tool;

/// Placeholder function used until provider adapters are implemented.
///
/// Calling this returns the crate version, which is useful for smoke tests.
pub fn crate_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Returns whether the Rig integration was compiled with any provider backend.
pub fn any_provider_enabled() -> bool {
    cfg!(any(
        feature = "provider-openai",
        feature = "provider-anthropic",
        feature = "provider-gemini",
        feature = "provider-cohere",
        feature = "provider-deepseek",
        feature = "provider-groq",
        feature = "provider-huggingface",
        feature = "provider-mistral",
        feature = "provider-ollama",
        feature = "provider-perplexity",
        feature = "provider-together",
        feature = "provider-xai"
    ))
}

/// Returns whether the Rig integration was compiled with embedding support.
pub fn embeddings_enabled() -> bool {
    cfg!(feature = "embeddings")
}

/// Returns whether the Rig integration was compiled with memory support.
pub fn memory_enabled() -> bool {
    cfg!(feature = "memory")
}

/// Returns whether the Rig integration was compiled with vector-store support.
pub fn vector_store_enabled() -> bool {
    cfg!(any(
        feature = "vector-store-memory",
        feature = "vector-store-sqlite",
        feature = "vector-store-http"
    ))
}

/// Returns whether the Rig integration was compiled with mock-model support.
pub fn mock_enabled() -> bool {
    cfg!(feature = "mock")
}

/// Returns whether the Rig integration was compiled with memory-semantic
/// support (T-010 / FR-010): Rig-backed embedding and vector-store search over
/// structured memories.
pub fn memory_semantic_enabled() -> bool {
    cfg!(feature = "memory-semantic")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_version_matches_manifest() {
        // The version reported by `crate_version()` must match the workspace
        // version that was baked into the crate at compile time.
        assert!(!crate_version().is_empty());
        assert!(crate_version().starts_with("0.1.0-alpha"));
    }

    #[test]
    fn default_features_are_enabled() {
        assert!(any_provider_enabled());
    }

    #[test]
    fn vector_store_enabled_matches_feature_flags() {
        let expected = cfg!(any(
            feature = "vector-store-memory",
            feature = "vector-store-sqlite",
            feature = "vector-store-http"
        ));
        assert_eq!(vector_store_enabled(), expected);
    }
}

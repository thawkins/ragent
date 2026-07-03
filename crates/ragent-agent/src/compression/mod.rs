//! Context-aware compression using the Headroom crate ecosystem.
//!
//! This module provides content-aware, reversible compression for conversation
//! history, replacing the legacy truncation fallback with intelligent
//! compression that preserves the most relevant parts of every message.
//!
//! # Architecture
//!
//! The compression pipeline consists of:
//!
//! - **Tokenizer** — Accurate model-aware token counting (tiktoken, HfTokenizer,
//!   or estimation fallback)
//! - **Content detection** — Classifies message parts as JSON, diff, log, search
//!   results, code, or prose
//! - **Compressor dispatch** — Routes to the appropriate Headroom compressor
//!   based on content type
//! - **CCR (Compress-Cache-Retrieve)** — Stores original content and inserts
//!   `<<ccr:HASH>>` markers so the LLM can retrieve originals on demand
//! - **Relevance filtering** — BM25 scoring to keep the most relevant messages
//!
//! # Dependencies
//!
//! Requires the `headroom-core` crate, which is now an unconditional
//! dependency of `ragent-agent`.

pub mod ccr_store;
pub mod pipeline;
pub mod relevance;

/// Re-exports from headroom-core.
pub use headroom_core;

// Re-export primary types for convenience.
pub use pipeline::{
    CompressionMode, CompressionResult, CompressionStats, compress_help, compress_history,
    compress_history_with_mode, count_tokens, count_tokens_text,
};

/// Check whether the compression feature is enabled at compile time.
///
/// Always returns `true` now that compression is an unconditional dependency.
#[must_use]
pub const fn is_available() -> bool {
    true
}
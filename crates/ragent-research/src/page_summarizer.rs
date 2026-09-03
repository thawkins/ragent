//! Per-page webpage summarization for the research pipeline (T-012, T-013).
//!
//! Provides the [`PageSummarizer`] trait and an LLM-backed implementation
//! that condenses fetched page bodies before they enter synthesis or the
//! source vault.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;

use crate::analysis::LlmAnalysisEngine;

/// Default maximum tokens for a page summary.
pub const DEFAULT_PAGE_SUMMARY_MAX_TOKENS: u32 = 1024;

/// A summarized page together with its original URL and the timestamp at
/// which the summary was produced (T-013, FR-003, FR-018).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageSummary {
    /// Original URL of the summarized page.
    pub url: String,
    /// Concise LLM-generated summary text.
    pub summary: String,
    /// UTC timestamp at which the summary was generated.
    pub summarized_at: DateTime<Utc>,
}

/// Summarize a fetched webpage body into a concise form.
#[async_trait]
pub trait PageSummarizer: Send + Sync {
    /// Summarize `body` from `url` into a shorter text.
    async fn summarize_page(&self, url: &str, body: &str) -> anyhow::Result<PageSummary>;
}

/// LLM-backed page summarizer (FR-002, FR-010).
#[derive(Debug, Clone)]
pub struct LlmPageSummarizer {
    engine: Arc<LlmAnalysisEngine>,
    max_tokens: u32,
}

impl LlmPageSummarizer {
    /// Build a summarizer around an existing analysis engine.
    pub fn new(engine: Arc<LlmAnalysisEngine>) -> Self {
        Self {
            engine,
            max_tokens: DEFAULT_PAGE_SUMMARY_MAX_TOKENS,
        }
    }

    /// Override the summary output budget in tokens.
    #[must_use]
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens.max(1);
        self
    }
}

#[async_trait]
impl PageSummarizer for LlmPageSummarizer {
    async fn summarize_page(&self, url: &str, body: &str) -> anyhow::Result<PageSummary> {
        let summary = self
            .engine
            .summarize_page(url, body, self.max_tokens)
            .await?;
        Ok(PageSummary {
            url: url.to_string(),
            summary,
            summarized_at: Utc::now(),
        })
    }
}

//! Router LLM client that classifies prompts and delegates to resolved providers.
//!
//! Implements [`LlmClient`] for the router. When `chat()` is called, the client:
//! 1. Checks for prompt modifiers (slash/bracket/word prefixes).
//! 2. If no modifier, classifies the prompt across 15 dimensions.
//! 3. Selects a tier and resolves the target provider/model.
//! 4. Delegates the request to the resolved provider's client.
//!
//! The fallback executor (T-007) will handle retrying with the next model
//! in the tier's fallback chain when the primary fails.

use std::pin::Pin;

use anyhow::{Result, bail};
use futures::Stream;

use super::router_classifier::{AttachmentInfo, ClassificationResult, PromptClassifier};
use super::router_config::RouterConfig;
use super::router_modifiers;
use crate::llm::{ChatRequest, LlmClient, StreamEvent};

/// LLM client that routes requests based on prompt classification.
///
/// Created by [`RouterProvider::create_client`](super::router::RouterProvider).
/// Holds a snapshot of the router configuration at creation time.
pub struct RouterClient {
    /// Router configuration snapshot.
    config: RouterConfig,
}

impl RouterClient {
    /// Create a new `RouterClient` with the given configuration.
    pub fn new(config: RouterConfig) -> Self {
        Self { config }
    }

    /// Classify a prompt, returning the classification result.
    ///
    /// First checks for prompt modifiers (explicit tier overrides). If a
    /// modifier is found, uses that tier directly. Otherwise, scores the
    /// prompt across all 15 dimensions and selects a tier.
    ///
    /// When `attachments` indicates media is present, the result's
    /// `requires_vision` flag is set and the router should only consider
    /// models with `Capabilities::vision == true`.
    ///
    /// On classification error, falls back to `Tier::Medium` (FR-039).
    pub fn classify_prompt(
        &self,
        prompt: &str,
        history_text: Option<&str>,
        attachments: &AttachmentInfo,
    ) -> ClassificationResult {
        // Check for prompt modifiers first (FR-016, FR-017, FR-018).
        if let Some(mod_result) = router_modifiers::detect_modifier(prompt) {
            let scores = PromptClassifier::score_all_dimensions(
                &mod_result.remaining_prompt,
                history_text,
                attachments,
            );
            let composite = PromptClassifier::compute_composite(&scores, &self.config.weights);
            return ClassificationResult {
                dimension_scores: scores,
                composite_score: composite,
                tier: mod_result.tier,
                requires_vision: attachments.has_media(),
            };
        }

        // Normal classification path with error fallback to MEDIUM (FR-039).
        PromptClassifier::classify_safe(
            prompt,
            history_text,
            &self.config.weights,
            &self.config.boundaries,
            attachments,
        )
    }

    /// Returns the router configuration reference.
    pub fn config(&self) -> &RouterConfig {
        &self.config
    }
}

/// Extract attachment information from a [`ChatRequest`].
///
/// Scans all messages for `ContentPart::ImageUrl` variants and counts
/// image and video attachments. This information feeds into dimension 15
/// (`image_attachment`) of the classifier.
pub fn extract_attachments(request: &ChatRequest) -> AttachmentInfo {
    let mut image_count = 0usize;
    let mut video_count = 0usize;

    for msg in request.messages.iter() {
        if let crate::llm::ChatContent::Parts(parts) = &msg.content {
            for part in parts {
                if let crate::llm::ContentPart::ImageUrl { url } = part {
                    // Distinguish video URLs from image URLs by MIME type
                    // in data URIs or known video extensions.
                    let lower = url.to_lowercase();
                    let is_video = lower.starts_with("data:video/")
                        || lower.ends_with(".mp4")
                        || lower.ends_with(".webm")
                        || lower.ends_with(".mov")
                        || lower.ends_with(".avi")
                        || lower.ends_with(".mkv");
                    if is_video {
                        video_count += 1;
                    } else {
                        image_count += 1;
                    }
                }
            }
        }
    }

    AttachmentInfo {
        image_count,
        video_count,
    }
}

#[async_trait::async_trait]
impl LlmClient for RouterClient {
    /// Route a chat request based on prompt classification.
    ///
    /// Currently a stub that classifies the prompt and returns an error
    /// indicating the router client needs a downstream provider (T-007
    /// will implement the full fallback executor with provider delegation).
    ///
    /// # Errors
    ///
    /// Returns an error because the router does not directly handle LLM
    /// requests — it must delegate to a resolved provider. Full delegation
    /// is implemented as part of T-007 (FallbackExecutor).
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>> {
        // Extract attachment info from the request for dimension 15.
        let attachments = extract_attachments(&request);

        // Build history text from messages for context-aware classification.
        let history_text = request
            .messages
            .iter()
            .map(|m| match &m.content {
                crate::llm::ChatContent::Text(t) => t.as_str(),
                crate::llm::ChatContent::Parts(parts) => parts
                    .iter()
                    .find_map(|p| match p {
                        crate::llm::ContentPart::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .unwrap_or(""),
            })
            .collect::<Vec<_>>()
            .join(" ");

        // Extract the prompt from the last user message.
        let prompt = request
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| match &m.content {
                crate::llm::ChatContent::Text(t) => t.clone(),
                crate::llm::ChatContent::Parts(parts) => parts
                    .iter()
                    .find_map(|p| match p {
                        crate::llm::ContentPart::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .unwrap_or_default(),
            })
            .unwrap_or_default();

        let _result = self.classify_prompt(&prompt, Some(&history_text), &attachments);

        // T-007 will resolve the tier to a provider/model and delegate.
        // When requires_vision is true, only vision-capable models should
        // be considered from the tier's fallback chain.
        // For now, return an error indicating routing is not yet connected.
        bail!(
            "Router classified prompt but provider delegation is not yet implemented \
                   (T-007). Enable a concrete provider to use chat."
        );
    }
}

use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, bail};
use futures::Stream;

use super::router_classifier::{AttachmentInfo, ClassificationResult, PromptClassifier};
use super::router_config::{RouterConfig, TierConfig, TierEntry};
use super::router_modifiers;
use crate::llm::{ChatRequest, LlmClient, StreamEvent};
use crate::provider::ProviderRegistry;

/// LLM client that routes requests based on prompt classification.
///
/// Created by [`RouterProvider::create_client`](super::router::RouterProvider).
/// Holds a snapshot of the router configuration at creation time.
pub struct RouterClient {
    /// Router configuration snapshot.
    config: RouterConfig,
    /// Provider registry used to create downstream clients. `None` only during
    /// tests or before the registry is attached.
    registry: Option<Arc<ProviderRegistry>>,
    /// Optional storage handle used to resolve database-backed provider API keys.
    /// When `Some`, keys stored via `ragent auth` take precedence over
    /// environment variables.
    storage: Option<Arc<ragent_storage::Storage>>,
    /// Optional event bus for publishing router lifecycle events (e.g.
    /// classification results) to the TUI log panel.
    event_bus: Option<Arc<ragent_types::event::EventBus>>,
    /// Cached routing decision from the most recent fresh-prompt classification.
    /// Reused for continuation calls (tool-result messages) so the router does
    /// not re-classify or re-log after every tool execution.
    cached_entry: Mutex<Option<TierEntry>>,
    /// Cache of downstream `(provider, model)` → client so a delegated call
    /// reuses a warm client (and its connection pool) instead of rebuilding
    /// one — and re-establishing TLS/keep-alive — on every loop step (H2).
    downstream_clients: Mutex<HashMap<String, Arc<dyn LlmClient>>>,
}

impl RouterClient {
    /// Create a new `RouterClient` with the given configuration and optional
    /// provider registry and storage.
    pub fn new(
        config: RouterConfig,
        registry: Option<Arc<ProviderRegistry>>,
        storage: Option<Arc<ragent_storage::Storage>>,
    ) -> Self {
        Self {
            config,
            registry,
            storage,
            event_bus: None,
            cached_entry: Mutex::new(None),
            downstream_clients: Mutex::new(HashMap::new()),
        }
    }

    /// Attach an event bus so the router can publish classification events
    /// visible in the TUI log panel.
    pub fn with_event_bus(mut self, event_bus: Option<Arc<ragent_types::event::EventBus>>) -> Self {
        self.event_bus = event_bus;
        self
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
                modifier_tier: Some(mod_result.tier),
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

/// Extract attachment information from the most recent user message in a
/// [`ChatRequest`].
///
/// Scans only the last `user`-role message for `ContentPart::ImageUrl`
/// variants and counts image and video attachments. This information feeds
/// into dimension 15 (`image_attachment`) of the classifier. Historical images
/// from earlier turns should not force vision-model selection for text-only
/// follow-ups, but a current user message that still refers to a previous image
/// should be classified based on its own content.
pub fn extract_attachments(request: &ChatRequest) -> AttachmentInfo {
    let mut image_count = 0usize;
    let mut video_count = 0usize;

    // Only consider attachments attached to the current user prompt. Earlier
    // turns are context, not new media that requires vision routing.
    if let Some(last_user) = request.messages.iter().rev().find(|m| m.role == "user")
        && let crate::llm::ChatContent::Parts(parts) = &last_user.content
    {
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

    AttachmentInfo {
        image_count,
        video_count,
    }
}

#[async_trait::async_trait]
impl LlmClient for RouterClient {
    /// Route a chat request based on prompt classification.
    ///
    /// Classifies the prompt (or uses an explicit tier modifier), selects the
    /// first model in the resolved tier's fallback chain, and delegates the
    /// request to that provider's client. When `requires_vision` is true, only
    /// vision-capable models from the tier are considered.
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>> {
        // Extract attachment info from the request for dimension 15.
        let attachments = extract_attachments(&request);

        // Detect whether this is a continuation call (the last user message
        // contains only tool results) or a fresh user prompt. The agent loop
        // appends tool results as a "user"-role message after every tool
        // execution, so without this guard the router would re-classify tool
        // output and re-log the classification on every loop iteration.
        let is_continuation = last_user_message_is_tool_results(&request.messages);

        // On a continuation, reuse the cached routing decision so the router
        // only classifies + logs when a real user prompt is presented.
        if is_continuation
            && let Some(cached) = self.cached_entry.lock().ok().and_then(|g| g.clone())
        {
            return self.delegate_downstream(cached, request).await;
        }

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

        let result = self.classify_prompt(&prompt, Some(&history_text), &attachments);
        let tier_config = self.config.tier_config(result.tier);

        // For image/video requests, make sure the registry knows about each
        // candidate model's capabilities before selecting. Dynamic providers
        // (Ollama, Ollama Cloud, OpenAI, Anthropic, ...) discover models at
        // runtime and only expose vision support through the discovered
        // metadata.
        //
        // Warm *every* configured tier entry, not just the resolved tier, so
        // that vision-capable fallback models can be found when the resolved
        // tier has no usable entries.
        if result.requires_vision
            && let Some(registry) = self.registry.as_deref()
        {
            let mut seen = HashSet::new();
            for tier in super::router_config::Tier::all() {
                for entry in &self.config.tier_config(*tier).models {
                    if seen.insert((entry.provider.clone(), entry.model.clone())) {
                        let _ = registry
                            .resolve_model_async(&entry.provider, &entry.model)
                            .await;
                    }
                }
            }
        }

        // Pick the first suitable model from the tier. Vision requests require
        // a model whose registry entry advertises `capabilities.vision == true`.
        let (entry, selected_tier) = match select_tier_entry(
            &tier_config,
            result.requires_vision,
            self.registry.as_deref(),
        ) {
            Some(e) => (e.clone(), result.tier),
            None => {
                // If the resolved tier has no usable entries, try higher tiers
                // first, then lower tiers. This keeps partially configured
                // router clusters working.
                select_tier_entry_with_fallback(
                    &self.config,
                    result.tier,
                    result.requires_vision,
                    self.registry.as_deref(),
                )
                .ok_or_else(|| {
                    let active_dimensions = format_active_dimensions(&result.dimension_scores);
                    anyhow::anyhow!(
                        "Router classified prompt into {} tier (composite {:.4}, \
                         requires_vision={}, dimensions=[{}]) — no suitable model is configured.",
                        result.tier,
                        result.composite_score,
                        result.requires_vision,
                        active_dimensions,
                    )
                })?
            }
        };

        let selected_model = format!("{}:{}", entry.provider, entry.model);
        let classification_summary =
            format_classification_summary(&prompt, &result, &selected_model);
        tracing::info!(
            message = %classification_summary,
            tier = %selected_tier,
            requested_tier = %result.tier,
            composite_score = format!("{:.4}", result.composite_score),
            selected_model = %selected_model,
            requires_vision = result.requires_vision,
            prompt_len = prompt.chars().count(),
            "router classified prompt"
        );
        if let Some(ref bus) = self.event_bus
            && let Some(ref session_id) = request.session_id
        {
            // Only publish dimensions above the reporting threshold so the TUI
            // log panel matches the tracing log (which already filters in
            // `format_classification_summary`).  This avoids cluttering the panel
            // with zeroed-out dimensions for every classification.
            const REPORT_THRESHOLD: f64 = 0.05;
            let dimensions: Vec<(String, f64)> = result
                .dimension_scores
                .iter()
                .enumerate()
                .filter(|&(_, score)| *score > REPORT_THRESHOLD)
                .map(|(i, &score)| {
                    (
                        super::router_classifier::dimension_name(i).to_string(),
                        score,
                    )
                })
                .collect();
            let _ = bus.publish(ragent_types::event::Event::RouterClassification {
                session_id: session_id.clone(),
                tier: selected_tier.to_string(),
                requested_tier: Some(result.tier.to_string()),
                model: selected_model.clone(),
                composite_score: result.composite_score,
                prompt: prompt.clone(),
                dimensions,
            });
        }

        // Cache the selected entry so continuation calls (tool-result messages)
        // reuse it instead of re-classifying and re-logging.
        if let Ok(mut guard) = self.cached_entry.lock() {
            *guard = Some(entry.clone());
        }

        self.delegate_downstream(entry, request).await
    }
}

impl RouterClient {
    /// Delegate the chat request to the downstream provider/model identified by
    /// `entry`, rewriting `request.model` and creating a fresh downstream
    /// client for each call.
    async fn delegate_downstream(
        &self,
        entry: TierEntry,
        mut request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>> {
        let selected_model = format!("{}:{}", entry.provider, entry.model);

        let registry = match &self.registry {
            Some(r) => r,
            None => {
                bail!(
                    "Router selected model {} — \
                     but no provider registry is attached. Enable a concrete provider to use chat.",
                    selected_model,
                );
            }
        };

        let provider = match registry.get(&entry.provider) {
            Some(p) => p,
            None => {
                bail!(
                    "Router selected model {} but provider '{}' is not registered.",
                    selected_model,
                    entry.provider,
                );
            }
        };

        let api_key = self.resolve_api_key(&entry.provider).await;
        let base_url = resolve_env_base_url(&entry.provider);
        let mut options: HashMap<String, serde_json::Value> = HashMap::new();
        options.insert(
            "model_id".to_string(),
            serde_json::Value::String(entry.model.clone()),
        );

        // H2: reuse a warm downstream client keyed by (provider, model) so the
        // connection pool (TLS, keep-alive) is amortised across loop steps
        // instead of being torn down and rebuilt on every delegated call.
        let cache_key = format!("{}:{}", entry.provider, entry.model);
        let downstream = match self
            .downstream_clients
            .lock()
            .ok()
            .and_then(|g| g.get(&cache_key).cloned())
        {
            Some(client) => client,
            None => {
                let created = provider
                    .create_client(&api_key, base_url.as_deref(), &options)
                    .await
                    .with_context(|| {
                        format!(
                            "Router failed to create client for selected model {}",
                            selected_model
                        )
                    })?;
                let arc: Arc<dyn LlmClient> = Arc::from(created);
                if let Ok(mut guard) = self.downstream_clients.lock() {
                    guard.insert(cache_key, arc.clone());
                }
                arc
            }
        };

        // Route the request to the downstream model.
        request.model = entry.model.clone();
        let downstream_model = entry.model.clone();
        let downstream_stream = downstream.chat(request).await?;

        // Guarantee terminal-signal delivery: if the downstream provider's
        // stream ends without yielding `StreamEvent::Finish` (provider bug,
        // protocol drift, or malformed terminal frame), synthesise a
        // `Finish { reason: Stop }` so the session loop always observes a
        // terminal event per call.
        let stream = futures::stream::unfold(
            (downstream_stream, false),
            |(mut stream, mut saw_finish)| async move {
                match futures::StreamExt::next(&mut stream).await {
                    Some(event) => {
                        if matches!(event, StreamEvent::Finish { .. }) {
                            saw_finish = true;
                        }
                        Some((event, (stream, saw_finish)))
                    }
                    None if !saw_finish => Some((
                        StreamEvent::Finish {
                            reason: crate::llm::LlmFinishReason::Stop,
                        },
                        (stream, true),
                    )),
                    None => None,
                }
            },
        );
        tracing::debug!(model = %downstream_model, "router delegated stream");
        Ok(Box::pin(stream))
    }
}
/// Returns `true` when the last user-role message consists entirely of tool
/// results (i.e. the agent loop appended tool output, not a fresh user prompt).
///
/// The agent loop sends tool results back to the model as a `"user"`-role
/// message whose content is `ChatContent::Parts` with only `ContentPart::ToolResult`
/// variants. When this is the case, the router should reuse its prior
/// classification rather than re-classifying the tool output as if it were a
/// new prompt.
fn last_user_message_is_tool_results(messages: &[crate::llm::ChatMessage]) -> bool {
    let Some(last_user) = messages.iter().rev().find(|m| m.role == "user") else {
        return false;
    };
    let crate::llm::ChatContent::Parts(parts) = &last_user.content else {
        return false;
    };
    !parts.is_empty()
        && parts
            .iter()
            .all(|p| matches!(p, crate::llm::ContentPart::ToolResult { .. }))
}

/// Build a concise, TUI-log-friendly summary of a router classification.
fn format_classification_summary(
    prompt: &str,
    result: &super::router_classifier::ClassificationResult,
    selected_model: &str,
) -> String {
    const THRESHOLD: f64 = 0.05;
    let active_dimensions: Vec<String> = result
        .dimension_scores
        .iter()
        .enumerate()
        .filter(|(_, s)| **s > THRESHOLD)
        .map(|(i, s)| format!("{}={:.2}", super::router_classifier::dimension_name(i), s))
        .collect();

    let display_prompt = if prompt.chars().count() > 80 {
        let mut end = 80;
        while end > 0 && !prompt.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &prompt[..end])
    } else {
        prompt.to_string()
    };

    let mut parts = vec![
        format!("prompt=\"{}\"", display_prompt),
        format!("bucket={}", result.tier),
        format!("model={}", selected_model),
        format!("composite={:.4}", result.composite_score),
    ];

    if let Some(modifier) = result.modifier_tier {
        parts.push(format!("modifier={}", modifier));
    }

    if result.requires_vision {
        parts.push("requires_vision=true".to_string());
    }

    if !active_dimensions.is_empty() {
        parts.push(format!("dimensions=[{}]", active_dimensions.join(", ")));
    }

    format!("Router classification — {}", parts.join(" | "))
}

/// Format a comma-separated list of active classification dimensions for error
/// messages. Always includes `image_attachment` when it is active so that
/// vision requests are explicit in the log.
fn format_active_dimensions(dimension_scores: &[f64; 15]) -> String {
    const THRESHOLD: f64 = 0.05;
    let parts: Vec<String> = dimension_scores
        .iter()
        .enumerate()
        .filter(|(_, s)| **s > THRESHOLD)
        .map(|(i, s)| format!("{}={:.2}", super::router_classifier::dimension_name(i), s))
        .collect();
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(", ")
    }
}

/// Resolve an API key for a downstream provider.
///
/// Environment variables take precedence. If no env key is set and a storage
/// handle is attached, the stored provider auth (set via `ragent auth`) is
/// used as a fallback. This mirrors the resolution path used when the
/// provider is selected directly.
fn resolve_env_api_key(provider_id: &str) -> String {
    let vars: &[&str] = match provider_id {
        "anthropic" => &["ANTHROPIC_API_KEY"],
        "openai" => &["OPENAI_API_KEY"],
        "gemini" => &["GEMINI_API_KEY"],
        "huggingface" => &["HF_TOKEN", "HUGGING_FACE_HUB_TOKEN"],
        "generic_openai" => &["OPENAI_API_KEY", "GENERIC_OPENAI_API_KEY"],
        "ollama_cloud" => &["OLLAMA_API_KEY"],
        "azure_foundry" => &["AZURE_AI_FOUNDRY_API_KEY"],
        "xai" => &["XAI_API_KEY"],
        "ollama" => &["OLLAMA_API_KEY"],
        _ => &[],
    };
    for var in vars {
        if let Ok(key) = std::env::var(var)
            && !key.is_empty()
        {
            return key;
        }
    }
    String::new()
}

impl RouterClient {
    /// Resolve the API key for a downstream provider, using storage-backed keys
    /// when available.
    async fn resolve_api_key(&self, provider_id: &str) -> String {
        // 1. Prefer environment variables.
        let key = resolve_env_api_key(provider_id);
        if !key.is_empty() {
            return key;
        }

        // 2. Fall back to stored provider auth.
        if let Some(storage) = self.storage.as_ref() {
            let provider_id = provider_id.to_string();
            let storage = Arc::clone(storage);
            if let Ok(Some(key)) = tokio::task::spawn_blocking(move || {
                storage
                    .get_provider_auth(&provider_id)
                    .ok()
                    .flatten()
                    .filter(|k| !k.is_empty())
            })
            .await
            {
                return key;
            }
        }

        String::new()
    }
}

/// Resolve a custom base URL from environment variables for a downstream provider.
fn resolve_env_base_url(provider_id: &str) -> Option<String> {
    match provider_id {
        "generic_openai" => std::env::var("GENERIC_OPENAI_API_BASE")
            .ok()
            .filter(|s| !s.trim().is_empty()),
        "azure_foundry" => std::env::var("AZURE_AI_FOUNDRY_BASE")
            .ok()
            .filter(|s| !s.trim().is_empty()),
        _ => None,
    }
}

/// Select the first usable entry from the resolved tier.
///
/// If `requires_vision` is false, the first entry is returned. Otherwise the
/// first model whose registry [`ModelInfo`] advertises `capabilities.vision`
/// is returned. For dynamically-discovered providers, the caller must warm the
/// registry's model cache (via [`ProviderRegistry::resolve_model_async`]) so
/// that vision capability is known.
fn select_tier_entry<'a>(
    tier_config: &'a TierConfig,
    requires_vision: bool,
    registry: Option<&'a ProviderRegistry>,
) -> Option<&'a super::router_config::TierEntry> {
    if !requires_vision {
        return tier_config.models.first();
    }

    // Vision requests require per-model capability metadata. The caller
    // (`RouterClient::chat`) warms every configured tier entry via the async
    // registry before selection, so the synchronous lookup here already sees
    // the discovered metadata for dynamic providers such as Ollama Cloud.
    let registry = registry?;
    tier_config.models.iter().find(|entry| {
        registry
            .resolve_model(&entry.provider, &entry.model)
            .map(|m| m.capabilities.vision)
            .unwrap_or(false)
    })
}

/// Select the first usable entry from the resolved tier, with fallback to higher
/// and lower tiers when the resolved tier has no suitable model.
///
/// If `requires_vision` is false, the first entry is returned. Otherwise the
/// first model whose registry [`ModelInfo`] advertises `capabilities.vision` is
/// returned. If the resolved tier has no candidates (or none satisfy the vision
/// requirement), the search falls back to higher tiers first, then lower tiers,
/// until a non-empty tier is found.
fn select_tier_entry_with_fallback(
    config: &super::router_config::RouterConfig,
    tier: super::router_config::Tier,
    requires_vision: bool,
    registry: Option<&ProviderRegistry>,
) -> Option<(super::router_config::TierEntry, super::router_config::Tier)> {
    let tiers = super::router_config::Tier::all();
    let requested_idx = tiers.iter().position(|t| *t == tier)?;

    // Build candidate tier order: higher tiers first, then lower tiers,
    // skipping the requested tier itself (already checked by the caller).
    let mut candidates: Vec<super::router_config::Tier> = Vec::new();
    for offset in 1..tiers.len() {
        if requested_idx + offset < tiers.len() {
            candidates.push(tiers[requested_idx + offset]);
        }
        if requested_idx >= offset {
            candidates.push(tiers[requested_idx - offset]);
        }
    }

    candidates
        .into_iter()
        .filter_map(|fallback_tier| {
            let tier_config = config.tier_config(fallback_tier);
            select_tier_entry(&tier_config, requires_vision, registry)
                .cloned()
                .map(|entry| (entry, fallback_tier))
        })
        .next()
}

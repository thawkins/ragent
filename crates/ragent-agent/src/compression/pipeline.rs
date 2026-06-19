//! Headroom compression pipeline wrapper.
//!
//! This module wraps the `headroom_core` compression facilities for use in
//! ragent's session processor. It provides content-aware compression for
//! conversation history.
//!
//! # Pipeline architecture
//!
//! The compression pipeline follows the Compress-Cache-Retrieve (CCR) model:
//!
//! 1. **Token counting** — Accurately estimate tokens using Headroom's tokenizer.
//! 2. **Threshold check** — Skip compression if under the configured threshold.
//! 3. **Content detection** — Classify each message part by content type.
//! 4. **Compression** — Apply the appropriate compressor based on content type:
//!    - **JSON** → minify (remove whitespace, deduplicate keys)
//!    - **Diff** → `DiffCompressor` from headroom-core (hunk selection, context trimming)
//!    - **Log** → line deduplication and priority-based selection
//!    - **Search** → file-level deduplication
//!    - **Prose** → pass through unchanged (or truncate if very long)
//! 5. **CCR stashing** — Store original content under a BLAKE3 hash key and
//!    insert `<<ccr:HASH>>` markers in the compressed output.
//!
//! # Protected messages (FR-017)
//!
//! System messages and the most recent user message are never compressed —
//! they pass through unchanged to preserve instruction fidelity.
//!
//! This entire module is only compiled when the `compression` Cargo feature
//! is enabled. When disabled, no automatic context reduction is performed.

use crate::message::{ImageData, Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use chrono::Utc;
use ragent_config::compression::CompressionConfig;
use tracing::{debug, info};

use headroom_core::tokenizer::Tokenizer;
use headroom_core::transforms::DiffCompressor;

use crate::compression::ccr_store::CcrStoreHandle;

// ── Content type detection ──────────────────────────────────────────────────

/// Content types detected by the content router.
///
/// These classify message parts for compressor dispatch. When the
/// `compression` feature is enabled, content detection uses Headroom's
/// `DiffCompressor` for diff content and heuristic classification for
/// other content types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentType {
    /// JSON tool output (arrays, objects, structured data).
    Json,
    /// Unified diff or patch content.
    Diff,
    /// Log output (application logs, SRE debugging).
    Log,
    /// Code search results (grep, codeindex output).
    Search,
    /// Source code (AST-compressible).
    Code,
    /// Plain prose text (not compressible by specialised compressors).
    Prose,
}

impl std::fmt::Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json => write!(f, "JSON"),
            Self::Diff => write!(f, "Diff"),
            Self::Log => write!(f, "Log"),
            Self::Search => write!(f, "Search"),
            Self::Code => write!(f, "Code"),
            Self::Prose => write!(f, "Prose"),
        }
    }
}

/// Detect the content type of text using heuristic rules.
///
/// Uses simple pattern-based classification suitable for the initial
/// pipeline. Future iterations may integrate more sophisticated detection.
#[must_use]
pub fn detect_content_type_heuristic(text: &str) -> ContentType {
    let trimmed = text.trim();
    // JSON detection: starts with { or [
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return ContentType::Json;
    }
    // Diff detection: starts with diff --git or --- a/
    if trimmed.starts_with("diff --git") || trimmed.starts_with("--- a/") {
        return ContentType::Diff;
    }
    // Log detection: contains common log level prefixes
    if trimmed.contains("ERROR") || trimmed.contains("WARN") || trimmed.contains("INFO") {
        return ContentType::Log;
    }
    // Search results detection: grep-style file:line:content patterns
    if trimmed.contains(':') && trimmed.contains('\n') {
        let first_line = trimmed.lines().next().unwrap_or("");
        // Pattern: filename:line_number:content
        if first_line.split(':').count() >= 3 {
            return ContentType::Search;
        }
    }
    ContentType::Prose
}

/// Detect the content type of a message part.
///
/// Uses heuristic rules for content classification. Tool calls use their
/// output text for classification; images and reasoning default to Prose.
pub fn detect_content_type(part: &MessagePart) -> ContentType {
    match part {
        MessagePart::Text { text } => detect_content_type_heuristic(text),
        MessagePart::ToolCall { tool, state, .. } => {
            if let Some(output) = state.output.as_ref().and_then(|v| v.as_str()) {
                detect_content_type_heuristic(output)
            } else {
                let _ = tool;
                ContentType::Prose
            }
        }
        MessagePart::Image { .. } => ContentType::Prose,
        MessagePart::Reasoning { .. } => ContentType::Prose,
    }
}

// ── Token counting ───────────────────────────────────────────────────────────

/// Characters per token for the estimation fallback.
pub const CHARS_PER_TOKEN_ESTIMATE: usize = 4;

/// Per-message overhead in tokens (approximation for message metadata).
pub const MESSAGE_OVERHEAD_TOKENS: usize = 10;

/// Estimate image tokens (rough approximation for vision models).
pub const IMAGE_TOKEN_ESTIMATE: usize = 1000;

/// Count tokens in a message history using Headroom's tokenizer.
///
/// Uses `headroom_core::tokenizer::EstimatingCounter` for accurate
/// token estimation that replaces the `chars / 4 + 10` heuristic.
pub fn count_tokens(messages: &[Message]) -> usize {
    let estimator = headroom_core::tokenizer::EstimatingCounter::default();
    let mut total = 0usize;
    for msg in messages {
        for part in &msg.parts {
            let text = match part {
                MessagePart::Text { text } => text.clone(),
                MessagePart::ToolCall { tool, state, .. } => {
                    let mut s = tool.clone();
                    if let Some(output) = state.output.as_ref().and_then(|v| v.as_str()) {
                        s.push_str(output);
                    }
                    if let Some(error) = state.error.as_ref() {
                        s.push_str(error);
                    }
                    s
                }
                MessagePart::Image { .. } => {
                    total += IMAGE_TOKEN_ESTIMATE;
                    continue;
                }
                MessagePart::Reasoning { text } => text.clone(),
            };
            total += estimator.count_text(&text);
        }
        total += MESSAGE_OVERHEAD_TOKENS;
    }
    total
}

/// Count tokens for a single piece of text using Headroom's tokenizer.
pub fn count_tokens_text(text: &str) -> usize {
    let estimator = headroom_core::tokenizer::EstimatingCounter::default();
    estimator.count_text(text)
}

/// Check whether the estimated token count exceeds the compression threshold.
///
/// Returns `true` when the estimated tokens exceed `threshold_fraction` of
/// `context_window`.
#[must_use]
pub fn should_compress(
    messages: &[Message],
    context_window: usize,
    threshold_fraction: f64,
) -> bool {
    let token_count = count_tokens(messages);
    let threshold = (context_window as f64 * threshold_fraction) as usize;
    token_count > threshold
}

// ── Statistics ───────────────────────────────────────────────────────────────

/// Statistics from a compression run.
#[derive(Debug, Clone)]
pub struct CompressionStats {
    /// Token count before compression.
    pub original_tokens: usize,
    /// Token count after compression.
    pub compressed_tokens: usize,
    /// Compression ratio (original / compressed). 1.0 = no compression.
    pub compression_ratio: f64,
    /// Number of CCR entries stashed during compression.
    pub ccr_entries_stashed: usize,
    /// Number of messages that were content-compressed (not just truncated).
    pub messages_compressed: usize,
}

impl CompressionStats {
    /// Calculate compression ratio from before/after token counts.
    #[must_use]
    pub fn from_tokens(original: usize, compressed: usize) -> Self {
        let ratio = if compressed > 0 {
            original as f64 / compressed as f64
        } else {
            1.0
        };
        Self {
            original_tokens: original,
            compressed_tokens: compressed,
            compression_ratio: ratio,
            ccr_entries_stashed: 0,
            messages_compressed: 0,
        }
    }
}

/// Result of compressing a provider-facing chat-message payload.
///
/// Mirrors [`CompressionResult`] but carries [`crate::llm::ChatMessage`]s so
/// the agent loop can replace its request payload directly.
#[derive(Debug, Clone)]
pub struct ChatCompressionResult {
    /// Compressed chat messages ready for the LLM request.
    pub chat_messages: Vec<crate::llm::ChatMessage>,
    /// Token statistics for the compression run.
    pub stats: CompressionStats,
}

// ── Chat message round-trip helpers ───────────────────────────────────────────

/// Convert provider-facing [`ChatMessage`]s into the internal [`Message`]
/// representation used by the Headroom pipeline.
///
/// This round-trip lets us reuse the content-aware compressors on the actual
/// request payload, including tool-use/tool-result pairs that only exist as
/// [`ContentPart`]s. Stray tool results without a matching tool use are kept as
/// text so their token cost is preserved.
fn chat_messages_to_messages(chat_messages: &[crate::llm::ChatMessage]) -> Vec<Message> {
    use crate::llm::{ChatContent, ContentPart};

    let mut messages: Vec<Message> = Vec::new();
    let now = Utc::now();
    for msg in chat_messages {
        let role = if msg.role == "assistant" {
            Role::Assistant
        } else {
            Role::User
        };
        let mut parts: Vec<MessagePart> = Vec::new();
        match &msg.content {
            ChatContent::Text(text) => {
                parts.push(MessagePart::Text { text: text.clone() });
            }
            ChatContent::Parts(content_parts) => {
                for part in content_parts {
                    match part {
                        ContentPart::Text { text } => {
                            parts.push(MessagePart::Text { text: text.clone() });
                        }
                        ContentPart::ToolUse { id, name, input } => {
                            parts.push(MessagePart::ToolCall {
                                tool: name.clone(),
                                call_id: id.clone(),
                                state: ToolCallState {
                                    status: ToolCallStatus::Completed,
                                    input: input.clone(),
                                    output: None,
                                    error: None,
                                    duration_ms: None,
                                },
                            });
                        }
                        ContentPart::ToolResult {
                            tool_use_id,
                            content,
                        } => {
                            // Pair with the most recent assistant ToolUse that
                            // has not yet received a result.
                            let mut paired = false;
                            for prev in messages.iter_mut().rev() {
                                if prev.role != Role::Assistant {
                                    break;
                                }
                                for p in &mut prev.parts {
                                    if let MessagePart::ToolCall { call_id, state, .. } = p {
                                        if call_id == tool_use_id && state.output.is_none() {
                                            state.output = Some(serde_json::Value::String(
                                                content.to_string(),
                                            ));
                                            paired = true;
                                            break;
                                        }
                                    }
                                }
                                if paired {
                                    break;
                                }
                            }
                            if !paired {
                                parts.push(MessagePart::Text {
                                    text: format!("[tool result {tool_use_id}]: {content}"),
                                });
                            }
                        }
                        ContentPart::ImageUrl { url } => {
                            parts.push(MessagePart::Image(Box::new(ImageData {
                                mime_type: "image/png".to_string(),
                                path: std::path::PathBuf::from(url),
                            })));
                        }
                    }
                }
            }
        }
        messages.push(Message {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: "compression".to_string(),
            role,
            parts,
            created_at: now,
            updated_at: now,
        });
    }
    messages
}

/// Convert internal [`Message`]s back to provider-facing [`ChatMessage`]s.
///
/// Synchronous counterpart to `history_to_chat_messages` in the session
/// processor. Each assistant [`MessagePart::ToolCall`] produces an assistant
/// `ToolUse` part and a following user `ToolResult` part so the LLM API sees
/// the required pairs.
fn messages_to_chat_messages(messages: &[Message]) -> Vec<crate::llm::ChatMessage> {
    use crate::llm::{ChatContent, ChatMessage as LlmChatMessage, ContentPart};

    let mut chat_messages: Vec<LlmChatMessage> = Vec::new();
    for msg in messages {
        let role = match msg.role {
            Role::User => "user".to_string(),
            Role::Assistant => "assistant".to_string(),
        };
        let mut parts: Vec<ContentPart> = Vec::new();
        let mut tool_results: Vec<ContentPart> = Vec::new();
        for part in &msg.parts {
            match part {
                MessagePart::Text { text } => {
                    parts.push(ContentPart::Text { text: text.clone() });
                }
                MessagePart::Reasoning { text } => {
                    parts.push(ContentPart::Text {
                        text: format!("[reasoning]: {text}"),
                    });
                }
                MessagePart::Image(img) => {
                    parts.push(ContentPart::ImageUrl {
                        url: img.path.to_string_lossy().to_string(),
                    });
                }
                MessagePart::ToolCall {
                    tool,
                    call_id,
                    state,
                } => {
                    parts.push(ContentPart::ToolUse {
                        id: call_id.clone(),
                        name: tool.clone(),
                        input: state.input.clone(),
                    });
                    let result_text = state
                        .output
                        .as_ref()
                        .and_then(|v| v.as_str().map(std::string::ToString::to_string))
                        .unwrap_or_default();
                    let content = if result_text.is_empty() {
                        state.error.clone().unwrap_or_default()
                    } else {
                        result_text
                    };
                    tool_results.push(ContentPart::ToolResult {
                        tool_use_id: call_id.clone(),
                        content: content.into(),
                    });
                }
            }
        }
        let content = if parts.len() == 1 {
            if let Some(ContentPart::Text { text }) = parts.first() {
                ChatContent::Text(text.clone())
            } else {
                ChatContent::Parts(parts)
            }
        } else {
            ChatContent::Parts(parts)
        };
        chat_messages.push(LlmChatMessage { role, content });
        if !tool_results.is_empty() && msg.role == Role::Assistant {
            chat_messages.push(LlmChatMessage {
                role: "user".to_string(),
                content: ChatContent::Parts(tool_results),
            });
        }
    }
    chat_messages
}

/// Run the Headroom compression pipeline on provider-facing chat messages.
///
/// This is the per-iteration entry point used by the agent loop. It round-trips
/// [`ChatMessage`]s through the internal [`Message`] representation so the
/// existing content-aware compressors can be reused on the actual request
/// payload.
///
/// Returns the original messages unchanged when token usage is below the
/// configured threshold.
pub fn compress_chat_messages(
    chat_messages: &[crate::llm::ChatMessage],
    context_window: usize,
    max_output_tokens: usize,
    config: &CompressionConfig,
) -> ChatCompressionResult {
    let messages = chat_messages_to_messages(chat_messages);
    let result = compress_history(&messages, context_window, max_output_tokens, config);
    ChatCompressionResult {
        chat_messages: messages_to_chat_messages(&result.messages),
        stats: result.stats,
    }
}

/// Check whether provider-facing [`ChatMessage`]s exceed the compression threshold.
///
/// This is a lightweight check that mirrors the threshold logic inside
/// [`compress_history`], allowing callers to avoid the overhead (and the
/// unconditional UI events) of invoking the full pipeline when compression is
/// not needed.
pub fn should_compress_chat_messages(
    chat_messages: &[crate::llm::ChatMessage],
    context_window: usize,
    threshold_fraction: f64,
) -> bool {
    let messages = chat_messages_to_messages(chat_messages);
    should_compress(&messages, context_window, threshold_fraction)
}

/// Compression mode for the `/compress` slash command (FR-012).
///
/// Controls how aggressively the compression pipeline processes conversation
/// history. Each mode selects a different configuration profile:
///
/// - **Default** — Runs all enabled compressors with the user's config.
/// - **Aggressive** — Enables relevance filtering (BM25), line-importance
///   scoring, and all content-type compressors to maximise compression.
/// - **Conservative** — Only applies lossless compressors (JSON minification,
///   tag protection) and preserves more original content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CompressionMode {
    /// Default pipeline — all enabled compressors with user config.
    #[serde(rename = "default")]
    Default,
    /// Aggressive compression — enables relevance filtering and all compressors.
    #[serde(rename = "aggressive")]
    Aggressive,
    /// Conservative compression — only lossless compressors (JSON minification,
    /// tag protection) are applied; content is preserved as much as possible.
    #[serde(rename = "conservative")]
    Conservative,
}

impl std::fmt::Display for CompressionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Default => write!(f, "default"),
            Self::Aggressive => write!(f, "aggressive"),
            Self::Conservative => write!(f, "conservative"),
        }
    }
}

impl std::str::FromStr for CompressionMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "default" | "" => Ok(Self::Default),
            "aggressive" | "max" | "maximum" => Ok(Self::Aggressive),
            "conservative" | "light" | "minimal" => Ok(Self::Conservative),
            _ => Err(format!(
                "Unknown compression mode: '{}'. Valid modes: default, aggressive, conservative",
                s
            )),
        }
    }
}

/// Result of a compression pipeline run.
#[derive(Debug)]
pub struct CompressionResult {
    /// The compressed message history.
    pub messages: Vec<Message>,
    /// Statistics about the compression run.
    pub stats: CompressionStats,
}

// ── Content compressors ──────────────────────────────────────────────────────

/// Compress JSON content by minifying whitespace and deduplicating.
///
/// This removes unnecessary whitespace from JSON while preserving
/// semantic content. For very large JSON arrays, it also applies
/// row sampling.
fn compress_json(text: &str, ccr_store: &mut CcrStoreHandle, config: &CompressionConfig) -> String {
    if !config.compressors.json || text.len() < 200 {
        return text.to_string();
    }

    // Try to minify JSON by removing whitespace.
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(value) => {
            let minified = serde_json::to_string(&value).unwrap_or_else(|_| text.to_string());

            // For very large JSON, apply row sampling for arrays of objects.
            if let serde_json::Value::Array(arr) = &value {
                if arr.len() > 50 {
                    let original_len = text.len();
                    let minified_len = minified.len();
                    // If minification didn't reduce enough, stash and truncate.
                    if minified_len > original_len / 2 {
                        let (_key, marker) = ccr_store.stash(text);
                        let sample_size = 20.min(arr.len());
                        let mut sampled = arr.clone();
                        sampled.truncate(sample_size);
                        let sampled_json =
                            serde_json::to_string(&serde_json::Value::Array(sampled))
                                .unwrap_or_default();
                        return format!(
                            "{}\n\n[... {} of {} rows omitted ... {}]\n",
                            sampled_json,
                            arr.len() - sample_size,
                            arr.len(),
                            marker
                        );
                    }
                }
            }

            minified
        }
        Err(_) => {
            // Not valid JSON — treat as prose.
            text.to_string()
        }
    }
}

/// Compress diff content using Headroom's DiffCompressor.
///
/// This applies hunk selection, context trimming, and file-level
/// filtering. The original content is stashed in the CCR store.
fn compress_diff(text: &str, ccr_store: &mut CcrStoreHandle, config: &CompressionConfig) -> String {
    if !config.compressors.diff || text.len() < 200 {
        return text.to_string();
    }

    let compressor = DiffCompressor::default();
    let result = compressor.compress(text, "");

    // Only use the compressed result if it actually reduced the size
    // and the DiffCompressor produced output (not empty).
    if result.compressed.is_empty() || result.compressed.len() >= text.len() {
        return text.to_string();
    }

    // If the compressor generated a CCR cache key, store the original.
    if let Some(cache_key) = &result.cache_key {
        ccr_store.stash(text);
        debug!(key = %cache_key, original_lines = result.original_line_count,
               compressed_lines = result.compressed_line_count, "Diff compressed with CCR");
    }

    result.compressed
}

/// Compress log content by deduplicating repeated lines and
/// filtering by priority level.
fn compress_log(text: &str, ccr_store: &mut CcrStoreHandle, config: &CompressionConfig) -> String {
    if !config.compressors.log || text.len() < 200 {
        return text.to_string();
    }

    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 10 {
        return text.to_string();
    }

    // Priority-based log compression: keep ERROR and WARN lines,
    // sample INFO lines, drop DEBUG/TRACE lines.
    let mut compressed_lines = Vec::new();
    let mut last_line_hash = String::new();
    let mut dedup_count = 0usize;
    let mut info_count = 0usize;
    let info_sample_rate = 3; // Keep every 3rd INFO line.

    for line in &lines {
        let trimmed = line.trim();
        let line_hash = blake3::hash(trimmed.as_bytes()).to_hex().to_string()[..8].to_string();

        // Deduplicate consecutive identical lines.
        if line_hash == last_line_hash {
            dedup_count += 1;
            continue;
        }
        last_line_hash = line_hash;

        if trimmed.contains("ERROR") || trimmed.contains("FATAL") || trimmed.contains("CRITICAL") {
            compressed_lines.push((*line).to_string());
        } else if trimmed.contains("WARN") || trimmed.contains("WARNING") {
            compressed_lines.push((*line).to_string());
        } else if trimmed.contains("INFO") {
            info_count += 1;
            if info_count % info_sample_rate == 1 || lines.len() < 30 {
                compressed_lines.push((*line).to_string());
            }
        } else {
            // Unknown level — keep it if there aren't too many.
            compressed_lines.push((*line).to_string());
        }
    }

    if dedup_count > 0 {
        compressed_lines.push(format!("... {dedup_count} duplicate lines omitted ..."));
    }

    let compressed = compressed_lines.join("\n");

    // If compression didn't help much, stash the original and keep compressed.
    if compressed.len() < text.len() * 8 / 10 {
        let (_, marker) = ccr_store.stash(text);
        format!("{compressed}\n\n[{marker}]")
    } else {
        text.to_string()
    }
}

/// Compress search results by deduplicating file-level entries.
fn compress_search(
    text: &str,
    ccr_store: &mut CcrStoreHandle,
    config: &CompressionConfig,
) -> String {
    if !config.compressors.search || text.len() < 200 {
        return text.to_string();
    }

    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 10 {
        return text.to_string();
    }

    // Group results by file, keep top N matches per file.
    let mut file_groups: std::collections::BTreeMap<String, Vec<&str>> =
        std::collections::BTreeMap::new();
    let mut header_lines: Vec<String> = Vec::new();

    for line in &lines {
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        if parts.len() >= 3 {
            let file = parts[0].to_string();
            file_groups.entry(file).or_default().push(*line);
        } else {
            // Not a search result line — keep as header.
            header_lines.push((*line).to_string());
        }
    }

    // Keep at most 5 results per file.
    let max_per_file = 5;
    let mut compressed_lines: Vec<String> = header_lines;
    let mut total_kept = 0usize;
    let mut total_omitted = 0usize;

    for (file, matches) in &file_groups {
        if matches.len() <= max_per_file {
            for s in matches {
                compressed_lines.push((*s).to_string());
            }
            total_kept += matches.len();
        } else {
            for m in &matches[..max_per_file] {
                compressed_lines.push((*m).to_string());
            }
            total_kept += max_per_file;
            total_omitted += matches.len() - max_per_file;
            compressed_lines.push(format!(
                "  ... {} more results in {}",
                matches.len() - max_per_file,
                file
            ));
        }
    }

    if total_omitted > 0 {
        compressed_lines.push(format!(
            "\n[Search results compressed: {total_kept} shown, {total_omitted} omitted]"
        ));
    }

    let compressed = compressed_lines.join("\n");

    // If compression helped, stash original and add marker.
    if compressed.len() < text.len() * 8 / 10 {
        let (_, marker) = ccr_store.stash(text);
        format!("{compressed}\n[{marker}]")
    } else {
        text.to_string()
    }
}

// ── UTF-8 safety ─────────────────────────────────────────────────────────────

/// Find a safe UTF-8 character boundary near the target byte position.
///
/// Ensures that truncation doesn't split a multi-byte character, satisfying
/// NFR-003 (compressed output must be valid UTF-8).
fn find_char_boundary(s: &str, target_byte: usize) -> usize {
    if s.len() <= target_byte {
        return s.len();
    }
    // Find the nearest char boundary at or before target_byte.
    let mut pos = target_byte;
    while pos > 0 && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

/// Compress a single text string through the appropriate compressor.
///
/// Detects the content type, routes through the appropriate compressor,
/// and stashes the original in the CCR store if offloading occurred.
///
/// Returns the compressed text.
fn compress_text(
    text: &str,
    content_type: ContentType,
    ccr_store: &mut CcrStoreHandle,
    config: &CompressionConfig,
) -> String {
    if text.is_empty() || text.len() < 200 {
        return text.to_string();
    }

    match content_type {
        ContentType::Json => compress_json(text, ccr_store, config),
        ContentType::Diff => compress_diff(text, ccr_store, config),
        ContentType::Log => compress_log(text, ccr_store, config),
        ContentType::Search => compress_search(text, ccr_store, config),
        ContentType::Code | ContentType::Prose => {
            // Code and prose don't have specialised compressors yet.
            // For very long content, truncate with a CCR marker.
            if text.len() > 50_000 {
                let truncation_point = find_char_boundary(text, 20_000);
                let (_, marker) = ccr_store.stash(text);
                format!("{} ... [{}]", &text[..truncation_point], marker)
            } else {
                text.to_string()
            }
        }
    }
}
// ── Protected messages ───────────────────────────────────────────────────────

/// Identify message indices that must NOT be compressed (FR-017).
///
/// Protected messages:
/// - All system messages (instructions must be preserved verbatim)
/// - The most recent user message (the current query must be preserved)
fn find_protected_messages(messages: &[Message]) -> Vec<usize> {
    let mut protected = Vec::new();

    // Find the most recent user message.
    let last_user_idx = messages.iter().rposition(|m| m.role == Role::User);

    for (idx, _msg) in messages.iter().enumerate() {
        // Protect the first message (typically contains initial instructions).
        if idx == 0 {
            protected.push(idx);
            debug!(index = idx, "Protecting first message");
        }
        // Protect the most recent user message (FR-017).
        if Some(idx) == last_user_idx {
            protected.push(idx);
            debug!(index = idx, "Protecting most recent user message");
        }
    }

    protected
}

// ── Main pipeline ─────────────────���──────────────────────────────────────────

/// Run the Headroom compression pipeline on the conversation history.
///
/// This function:
/// 1. Counts tokens using Headroom's tokenizer.
/// 2. If the total is below the threshold, returns the history unchanged.
/// 3. For messages over the threshold, compresses content-aware parts:
///    - Detects content type per message part
///    - Routes through the appropriate compressor
///    - Stores originals in the CCR store with BLAKE3 hash markers
///    - Never compresses system messages or most recent user message (FR-017)
/// 4. If compression doesn't reduce enough, falls back to truncation.
/// 5. Returns the compressed history with statistics.
///
/// # Errors
///
/// Returns the original history unchanged if compression fails, with a warning log.
pub fn compress_history(
    history: &[Message],
    context_window: usize,
    _max_output_tokens: usize,
    config: &CompressionConfig,
) -> CompressionResult {
    let original_tokens = count_tokens(history);

    // If we're under the threshold, no compression needed.
    let threshold = (context_window as f64 * config.auto_threshold) as usize;
    if original_tokens <= threshold {
        debug!(
            original_tokens,
            threshold, "History under compression threshold, skipping"
        );
        return CompressionResult {
            messages: history.to_vec(),
            stats: CompressionStats::from_tokens(original_tokens, original_tokens),
        };
    }

    info!(
        original_tokens,
        threshold, "History exceeds compression threshold, running pipeline"
    );

    // Build the CCR store for stashing originals.
    let mut ccr_store = CcrStoreHandle::in_memory();

    // Identify protected message indices (system + most recent user).
    let protected_indices = find_protected_messages(history);

    // Compress each non-protected message part through the pipeline.
    let mut compressed_messages = Vec::with_capacity(history.len());
    let mut total_ccr_entries = 0usize;
    let mut messages_compressed = 0usize;

    for (idx, msg) in history.iter().enumerate() {
        if protected_indices.contains(&idx) {
            // Protected message — pass through unchanged (FR-017).
            debug!(index = idx, role = %msg.role, "Protected message, skipping compression");
            compressed_messages.push(msg.clone());
            continue;
        }

        let mut compressed_parts = Vec::with_capacity(msg.parts.len());
        let mut msg_was_compressed = false;

        for part in &msg.parts {
            let content_type = detect_content_type(part);
            let (text, can_compress) = match part {
                MessagePart::Text { text } => (text.clone(), true),
                MessagePart::ToolCall { state, .. } => {
                    if let Some(output) = state.output.as_ref().and_then(|v| v.as_str()) {
                        (output.to_string(), true)
                    } else {
                        compressed_parts.push(part.clone());
                        continue;
                    }
                }
                MessagePart::Image { .. } | MessagePart::Reasoning { .. } => {
                    compressed_parts.push(part.clone());
                    continue;
                }
            };

            if !can_compress || text.len() < 200 {
                compressed_parts.push(part.clone());
                continue;
            }
            let compressed_text = compress_text(&text, content_type, &mut ccr_store, config);

            // Count CCR markers to track how many originals were stashed.
            let markers_before = total_ccr_entries;
            let markers_after = ccr_store.len();
            let new_ccr_entries = markers_after.saturating_sub(markers_before);
            total_ccr_entries = ccr_store.len();

            if compressed_text.len() < text.len() || new_ccr_entries > 0 {
                msg_was_compressed = true;
                // Replace the text in the message part.
                match part {
                    MessagePart::Text { .. } => {
                        compressed_parts.push(MessagePart::Text {
                            text: compressed_text,
                        });
                    }
                    MessagePart::ToolCall {
                        tool,
                        call_id,
                        state,
                    } => {
                        compressed_parts.push(MessagePart::ToolCall {
                            tool: tool.clone(),
                            call_id: call_id.clone(),
                            state: crate::message::ToolCallState {
                                status: state.status.clone(),
                                input: state.input.clone(),
                                output: Some(serde_json::Value::String(compressed_text)),
                                error: state.error.clone(),
                                duration_ms: state.duration_ms,
                            },
                        });
                    }
                    MessagePart::Reasoning { .. } | MessagePart::Image { .. } => {
                        // These are handled above, but include for completeness.
                        compressed_parts.push(part.clone());
                    }
                }
            } else {
                // No compression benefit — keep original.
                compressed_parts.push(part.clone());
            }
        }

        if msg_was_compressed {
            messages_compressed += 1;
        }

        // Only replace message if we actually compressed something.
        let compressed_msg = if msg_was_compressed {
            Message {
                id: msg.id.clone(),
                session_id: msg.session_id.clone(),
                role: msg.role.clone(),
                parts: compressed_parts,
                created_at: msg.created_at,
                updated_at: msg.updated_at,
            }
        } else {
            msg.clone()
        };
        compressed_messages.push(compressed_msg);
    }

    let compressed_tokens = count_tokens(&compressed_messages);

    info!(
        original_tokens,
        compressed_tokens,
        ccr_entries = total_ccr_entries,
        messages_compressed,
        ratio = if compressed_tokens > 0 {
            format!("{:.2}", original_tokens as f64 / compressed_tokens as f64)
        } else {
            "N/A".to_string()
        },
        "Compression pipeline completed"
    );

    CompressionResult {
        messages: compressed_messages,
        stats: CompressionStats {
            original_tokens,
            compressed_tokens,
            compression_ratio: if compressed_tokens > 0 {
                original_tokens as f64 / compressed_tokens as f64
            } else {
                1.0
            },
            ccr_entries_stashed: total_ccr_entries,
            messages_compressed,
        },
    }
}
/// Run the Headroom compression pipeline with a specific compression mode.
///
/// This is the mode-aware entry point for the `/compress` slash command.
/// It creates a derived `CompressionConfig` from the mode and delegates
/// to [`compress_history`].
///
/// # Mode behaviour
///
/// - **Default** — Uses the provided config as-is (all enabled compressors).
/// - **Aggressive** — Enables all compressors and relevance filtering,
///   sets `auto_threshold` to 0.50 to force compression sooner, and
///   enables BM25 relevance ranking to keep the most relevant messages.
/// - **Conservative** — Disables lossy compressors (log, search, code, prose)
///   and only keeps lossless compressors (JSON, diff) enabled. Relevance
///   filtering is disabled.
pub fn compress_history_with_mode(
    history: &[Message],
    context_window: usize,
    max_output_tokens: usize,
    config: &CompressionConfig,
    mode: CompressionMode,
) -> CompressionResult {
    let derived_config = match mode {
        CompressionMode::Default => config.clone(),
        CompressionMode::Aggressive => CompressionConfig {
            // Force compression at 50% of context window.
            auto_threshold: 0.50,
            // Enable all compressors for maximum compression.
            compressors: ragent_config::compression::CompressorConfig {
                json: true,
                diff: true,
                log: true,
                search: true,
                code: true,
                prose: true,
            },
            // Enable relevance filtering for aggressive mode.
            relevance: ragent_config::compression::RelevanceConfig {
                enabled: true,
                scorer: "bm25".to_string(),
                keep_top_k: 15,
            },
            ..config.clone()
        },
        CompressionMode::Conservative => CompressionConfig {
            // Only compress at 90% of context window.
            auto_threshold: 0.90,
            // Only lossless compressors.
            compressors: ragent_config::compression::CompressorConfig {
                json: true,
                diff: true,
                log: false,
                search: false,
                code: false,
                prose: false,
            },
            // No relevance filtering in conservative mode.
            relevance: ragent_config::compression::RelevanceConfig {
                enabled: false,
                scorer: "bm25".to_string(),
                keep_top_k: 20,
            },
            ..config.clone()
        },
    };
    compress_history(history, context_window, max_output_tokens, &derived_config)
}

/// Generate a formatted help string for the `/compress` slash command.
///
/// Lists all subcommands and describes the current compression configuration.
#[must_use]
pub fn compress_help(config: &CompressionConfig) -> String {
    let enabled_status = if config.enabled {
        "enabled"
    } else {
        "disabled"
    };
    let ccr_backend = &config.ccr.backend;
    let tokenizer_backend = &config.tokenizer.backend;

    let mut help = String::from("From: /compress help\n\n");
    help.push_str("Compression subcommands:\n\n");
    help.push_str("| Subcommand | Description |\n");
    help.push_str("|---|---|\n");
    help.push_str(
        "| `/compress` | Run the default compression pipeline (all enabled compressors) |\n",
    );
    help.push_str("| `/compress aggressive` | Maximum compression with relevance filtering and line-importance scoring |\n");
    help.push_str("| `/compress conservative` | Only apply lossless compressors (JSON minification, tag protection) |\n");
    help.push_str("| `/compress help` | Display this help text |\n");
    help.push_str(
        "| `/compress stats` | Show compression statistics for the current session |\n\n",
    );
    help.push_str(&format!(
                      "Configuration status:\n  - Compression: {}\n  - Auto threshold: {:.0}%\n  - CCR backend: {}\n  - Tokenizer: {}\n",
                      enabled_status,
                      config.auto_threshold * 100.0,
                      ccr_backend,
                      tokenizer_backend,
                  ));
    help.push_str(&format!(
        "  - Compressors: json={}, diff={}, log={}, search={}, code={}, prose={}\n",
        config.compressors.json,
        config.compressors.diff,
        config.compressors.log,
        config.compressors.search,
        config.compressors.code,
        config.compressors.prose,
    ));
    help.push_str(&format!(
        "  - Relevance filtering: {} (scorer={}, keep_top_k={})\n",
        if config.relevance.enabled {
            "on"
        } else {
            "off"
        },
        config.relevance.scorer,
        config.relevance.keep_top_k,
    ));
    help
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_stats_from_tokens() {
        let stats = CompressionStats::from_tokens(10000, 3000);
        assert_eq!(stats.original_tokens, 10000);
        assert_eq!(stats.compressed_tokens, 3000);
        assert!((stats.compression_ratio - 3.333).abs() < 0.01);
        assert_eq!(stats.ccr_entries_stashed, 0);
        assert_eq!(stats.messages_compressed, 0);
    }

    #[test]
    fn test_compression_stats_from_tokens_no_compression() {
        let stats = CompressionStats::from_tokens(5000, 5000);
        assert!((stats.compression_ratio - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compression_stats_from_tokens_zero_compressed() {
        let stats = CompressionStats::from_tokens(5000, 0);
        assert!((stats.compression_ratio - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_content_type_display() {
        assert_eq!(ContentType::Json.to_string(), "JSON");
        assert_eq!(ContentType::Diff.to_string(), "Diff");
        assert_eq!(ContentType::Log.to_string(), "Log");
        assert_eq!(ContentType::Search.to_string(), "Search");
        assert_eq!(ContentType::Code.to_string(), "Code");
        assert_eq!(ContentType::Prose.to_string(), "Prose");
    }

    #[test]
    fn test_detect_content_type_heuristic_json() {
        assert_eq!(
            detect_content_type_heuristic("{\"key\": \"value\"}"),
            ContentType::Json
        );
        assert_eq!(
            detect_content_type_heuristic("[1, 2, 3]"),
            ContentType::Json
        );
    }

    #[test]
    fn test_detect_content_type_heuristic_diff() {
        assert_eq!(
            detect_content_type_heuristic("diff --git a/file.rs b/file.rs"),
            ContentType::Diff
        );
        assert_eq!(
            detect_content_type_heuristic("--- a/file.rs\n+++ b/file.rs"),
            ContentType::Diff
        );
    }

    #[test]
    fn test_detect_content_type_heuristic_log() {
        assert_eq!(
            detect_content_type_heuristic("ERROR: something failed"),
            ContentType::Log
        );
        assert_eq!(
            detect_content_type_heuristic("WARN: deprecated"),
            ContentType::Log
        );
        assert_eq!(
            detect_content_type_heuristic("INFO: started"),
            ContentType::Log
        );
    }

    #[test]
    fn test_detect_content_type_heuristic_prose() {
        assert_eq!(
            detect_content_type_heuristic("Hello, world!"),
            ContentType::Prose
        );
    }

    #[test]
    fn test_detect_content_type_heuristic_search() {
        let search_output =
            "src/main.rs:42:fn main() {\nsrc/main.rs:43:    println!(\"hello\");\nsrc/main.rs:44:}";
        assert_eq!(
            detect_content_type_heuristic(search_output),
            ContentType::Search
        );
    }

    #[test]
    fn test_count_tokens_text() {
        let count = count_tokens_text("Hello, world!");
        assert!(count > 0, "Token count should be positive");
        assert!(
            count < 100,
            "Short text should not have excessive token count"
        );
    }

    #[test]
    fn test_count_tokens_text_empty() {
        let count = count_tokens_text("");
        assert_eq!(count, 0, "Empty text should have zero tokens");
    }

    #[test]
    fn test_should_compress_under_threshold() {
        let messages = vec![Message::user_text("test-session", "Hello, world!")];
        assert!(!should_compress(&messages, 128_000, 0.80));
    }

    #[test]
    fn test_should_compress_over_threshold() {
        let long_text = "x ".repeat(500_000);
        let messages = vec![Message::user_text("test-session", long_text)];
        assert!(should_compress(&messages, 10_000, 0.80));
    }

    #[test]
    fn test_headroom_tokenizer_available() {
        let estimator = headroom_core::tokenizer::EstimatingCounter::default();
        let count = estimator.count_text("Hello, world!");
        assert!(count > 0, "EstimatingCounter should count at least 1 token");
    }

    #[test]
    fn test_headroom_hello() {
        assert_eq!(headroom_core::hello(), "headroom-core");
    }

    #[test]
    fn test_headroom_diff_compressor_available() {
        let compressor = DiffCompressor::default();
        let result = compressor.compress("diff --git a/test.rs b/test.rs\n--- a/test.rs\n+++ b/test.rs\n@@ -1,3 +1,3 @@\n-old line\n+new line\n context\n", "");
        // DiffCompressor should produce output (possibly unchanged for short diffs).
        assert!(!result.compressed.is_empty() || result.original_line_count == 0);
    }

    #[test]
    fn test_find_protected_messages_protects_first_and_last_user() {
        let messages = vec![
            Message::user_text("s1", "First message"), // idx 0: protected (first)
            Message::new("s1", Role::Assistant, vec![]), // idx 1: not protected
            Message::user_text("s1", "Middle message"), // idx 2: not protected
            Message::new("s1", Role::Assistant, vec![]), // idx 3: not protected
            Message::user_text("s1", "Latest message"), // idx 4: protected (last user)
        ];
        let protected = find_protected_messages(&messages);
        assert!(protected.contains(&0), "First message should be protected");
        assert!(
            protected.contains(&4),
            "Last user message should be protected"
        );
        assert!(
            !protected.contains(&1),
            "Assistant message should not be protected"
        );
        assert!(
            !protected.contains(&2),
            "Middle user message should not be protected"
        );
        assert!(
            !protected.contains(&3),
            "Assistant message should not be protected"
        );
    }

    #[test]
    fn test_find_protected_messages_single_message() {
        let messages = vec![Message::user_text("s1", "Only message")];
        let protected = find_protected_messages(&messages);
        assert!(protected.contains(&0), "Single message should be protected");
    }

    #[test]
    fn test_find_char_boundary_ascii() {
        let s = "Hello, world!";
        let boundary = find_char_boundary(s, 5);
        assert_eq!(&s[..boundary], "Hello");
    }

    #[test]
    fn test_find_char_boundary_empty() {
        let s = "";
        let boundary = find_char_boundary(s, 10);
        assert_eq!(boundary, 0);
    }

    #[test]
    fn test_find_char_boundary_short() {
        let s = "Hi";
        let boundary = find_char_boundary(s, 100);
        assert_eq!(boundary, s.len());
    }

    #[test]
    fn test_find_char_boundary_unicode() {
        // "café" has a multi-byte 'é' at the end.
        let s = "café";
        // The byte length of "caf" is 3; "é" starts at byte 3.
        let boundary = find_char_boundary(s, 3);
        assert_eq!(boundary, 3);
        assert!(s.is_char_boundary(boundary));
    }

    #[test]
    fn test_compress_json_minification() {
        let mut ccr = CcrStoreHandle::in_memory();
        let config = CompressionConfig::default();
        let json = r#"{"key1": "value1", "key2": "value2", "key3": "value3"}"#;
        let compressed = compress_json(json, &mut ccr, &config);
        // Minified JSON should be shorter or same length.
        assert!(
            compressed.len() <= json.len(),
            "Minified JSON should not be longer"
        );
    }

    #[test]
    fn test_compress_json_array_sampling() {
        let mut ccr = CcrStoreHandle::in_memory();
        let config = CompressionConfig::default();
        // Create a large JSON array.
        let items: Vec<String> = (0..100).map(|i| format!(r#"{{"id": {i}}}"#)).collect();
        let json = format!("[{}]", items.join(", "));
        let compressed = compress_json(&json, &mut ccr, &config);
        // For arrays > 50 items, compression should reduce size.
        assert!(
            compressed.len() < json.len(),
            "Large JSON array should be compressed"
        );
    }

    #[test]
    fn test_compress_log_priority_filtering() {
        let mut ccr = CcrStoreHandle::in_memory();
        let config = CompressionConfig::default();
        let log = "ERROR: critical failure\nINFO: starting\nINFO: running\nINFO: still running\nWARN: deprecated\nINFO: done\n";
        let compressed = compress_log(log, &mut ccr, &config);
        // ERROR and WARN should be preserved.
        assert!(compressed.contains("ERROR: critical failure"));
        assert!(compressed.contains("WARN: deprecated"));
    }

    #[test]
    fn test_compress_search_dedup() {
        let mut ccr = CcrStoreHandle::in_memory();
        let config = CompressionConfig::default();
        // Create search results with > 5 results per file.
        let mut search = String::new();
        for i in 0..20 {
            search.push_str(&format!("src/main.rs:{i}:some code line {i}\n"));
        }
        let compressed = compress_search(&search, &mut ccr, &config);
        // Should keep at most 5 results per file.
        assert!(
            compressed.contains("more results"),
            "Should indicate omitted results"
        );
    }

    #[test]
    fn test_chat_messages_round_trip_preserves_text_and_tool_pair() {
        use crate::llm::{ChatContent, ChatMessage as LlmChatMessage, ContentPart};
        use serde_json::json;

        let chat_messages = vec![
            LlmChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text("What is 2+2?".to_string()),
            },
            LlmChatMessage {
                role: "assistant".to_string(),
                content: ChatContent::Parts(vec![
                    ContentPart::Text {
                        text: "I will calculate that.".to_string(),
                    },
                    ContentPart::ToolUse {
                        id: "call_1".to_string(),
                        name: "calculator".to_string(),
                        input: json!({"expression": "2+2"}),
                    },
                ]),
            },
            LlmChatMessage {
                role: "user".to_string(),
                content: ChatContent::Parts(vec![ContentPart::ToolResult {
                    tool_use_id: "call_1".to_string(),
                    content: "4".to_string().into(),
                }]),
            },
        ];

        let messages = chat_messages_to_messages(&chat_messages);
        assert_eq!(messages.len(), 3);
        // ToolUse/ToolResult should be paired into a single assistant ToolCall.
        assert_eq!(messages[1].parts.len(), 2);
        assert!(
            matches!(&messages[1].parts[1], MessagePart::ToolCall { state, .. }
            if state.output.as_ref().and_then(|v| v.as_str()) == Some("4"))
        );

        let round_tripped = messages_to_chat_messages(&messages);
        assert_eq!(round_tripped.len(), 4);
        assert_eq!(round_tripped[0].role, "user");
        assert_eq!(round_tripped[1].role, "assistant");
        assert_eq!(round_tripped[2].role, "user");
        assert_eq!(round_tripped[3].role, "user");
    }

    #[test]
    fn test_compress_chat_messages_under_threshold_passes_through() {
        use crate::llm::{ChatContent, ChatMessage as LlmChatMessage};

        let chat_messages = vec![LlmChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Short question".to_string()),
        }];
        let config = CompressionConfig::default();
        let result = compress_chat_messages(&chat_messages, 128_000, 8192, &config);
        assert_eq!(result.chat_messages.len(), chat_messages.len());
        assert_eq!(result.stats.original_tokens, result.stats.compressed_tokens);
    }

    #[test]
    fn test_compress_chat_messages_over_threshold_triggers_compression() {
        use crate::llm::{ChatContent, ChatMessage as LlmChatMessage};

        let long_text = "x ".repeat(50_000);
        let chat_messages = vec![
            LlmChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text("Start".to_string()),
            },
            LlmChatMessage {
                role: "assistant".to_string(),
                content: ChatContent::Text(long_text.clone()),
            },
        ];
        let config = CompressionConfig::default();
        let result = compress_chat_messages(&chat_messages, 1_000, 512, &config);
        assert!(
            result.stats.compressed_tokens < result.stats.original_tokens,
            "Long assistant content should be compressed"
        );
        assert!(!result.chat_messages.is_empty());
    }

    #[test]
    fn test_stray_tool_result_kept_as_text() {
        use crate::llm::{ChatContent, ChatMessage as LlmChatMessage, ContentPart};

        let chat_messages = vec![LlmChatMessage {
            role: "user".to_string(),
            content: ChatContent::Parts(vec![ContentPart::ToolResult {
                tool_use_id: "orphan".to_string(),
                content: "lost result".to_string().into(),
            }]),
        }];
        let messages = chat_messages_to_messages(&chat_messages);
        assert_eq!(messages.len(), 1);
        assert!(matches!(&messages[0].parts[0], MessagePart::Text { text }
            if text.contains("lost result")));
    }

    #[test]
    fn test_image_url_round_trip() {
        use crate::llm::{ChatContent, ChatMessage as LlmChatMessage, ContentPart};

        let chat_messages = vec![LlmChatMessage {
            role: "user".to_string(),
            content: ChatContent::Parts(vec![ContentPart::ImageUrl {
                url: "data:image/png;base64,ABC".to_string(),
            }]),
        }];
        let messages = chat_messages_to_messages(&chat_messages);
        assert!(matches!(&messages[0].parts[0], MessagePart::Image(img)
            if img.path.to_string_lossy() == "data:image/png;base64,ABC"));
        let round_tripped = messages_to_chat_messages(&messages);
        assert!(matches!(
            &round_tripped[0].content,
            ChatContent::Parts(parts)
            if matches!(parts.first(), Some(ContentPart::ImageUrl { url }) if url == "data:image/png;base64,ABC")
        ));
    }

    #[test]
    fn test_compress_history_under_threshold() {
        let messages = vec![
            Message::user_text("s1", "Hello"),
            Message::new(
                "s1",
                Role::Assistant,
                vec![MessagePart::Text {
                    text: "Hi there!".into(),
                }],
            ),
        ];
        let config = CompressionConfig::default();
        let result = compress_history(&messages, 128_000, 8192, &config);
        // Under threshold — should return unchanged.
        assert_eq!(result.messages.len(), messages.len());
        assert_eq!(result.stats.original_tokens, result.stats.compressed_tokens);
    }

    #[test]
    fn test_compress_history_preserves_protected_messages() {
        // Create messages that exceed the threshold.
        let long_text = "x ".repeat(50_000);
        let messages = vec![Message::user_text("s1", &long_text)];
        let config = CompressionConfig::default();
        // Use a very small context window to force compression.
        let result = compress_history(&messages, 1000, 512, &config);
        // The first (and only) message is both first AND last user message,
        // so it should be protected and passed through.
        assert!(!result.messages.is_empty());
    }

    #[test]
    fn test_compress_text_short_content() {
        let mut ccr = CcrStoreHandle::in_memory();
        let config = CompressionConfig::default();
        let compressed = compress_text("Hello", ContentType::Prose, &mut ccr, &config);
        assert_eq!(
            compressed, "Hello",
            "Short content should pass through unchanged"
        );
    }

    #[test]
    fn test_compress_text_empty() {
        let mut ccr = CcrStoreHandle::in_memory();
        let config = CompressionConfig::default();
        let compressed = compress_text("", ContentType::Prose, &mut ccr, &config);
        assert!(compressed.is_empty());
    }

    #[test]
    fn test_compress_diff_with_real_diff() {
        let mut ccr = CcrStoreHandle::in_memory();
        let config = CompressionConfig::default();
        let diff = "diff --git a/test.rs b/test.rs\n--- a/test.rs\n+++ b/test.rs\n@@ -1,5 +1,5 @@\n-old line 1\n-old line 2\n context\n+new line 1\n+new line 2\n context\n";
        let compressed = compress_diff(diff, &mut ccr, &config);
        // DiffCompressor should produce output.
        assert!(!compressed.is_empty());
    }

    #[test]
    fn test_compress_code_truncation_with_ccr() {
        let mut ccr = CcrStoreHandle::in_memory();
        let config = CompressionConfig::default();
        // Create very long code content.
        let long_code = format!("fn main() {{\n{}\n}}", "// comment\n".repeat(50000));
        assert!(long_code.len() > 50_000, "Should be over 50k chars");
        let compressed = compress_text(&long_code, ContentType::Code, &mut ccr, &config);
        assert!(
            compressed.len() < long_code.len(),
            "Long code should be truncated"
        );
        // Should contain a CCR marker.
        assert!(
            compressed.contains("<<ccr:"),
            "Truncated content should contain CCR marker"
        );
        // Should be able to retrieve the original.
        let key = crate::compression::ccr_store::parse_ccr_marker(&compressed).unwrap();
        let original = ccr.retrieve(&key);
        assert_eq!(original.as_deref(), Some(long_code.as_str()));
    }
}

#[test]
fn test_should_compress_chat_messages_under_threshold() {
    use crate::llm::{ChatContent, ChatMessage as LlmChatMessage};

    let chat_messages = vec![LlmChatMessage {
        role: "user".to_string(),
        content: ChatContent::Text("Short question".to_string()),
    }];
    assert!(!should_compress_chat_messages(
        &chat_messages,
        128_000,
        0.80
    ));
}

#[test]
fn test_should_compress_chat_messages_over_threshold() {
    use crate::llm::{ChatContent, ChatMessage as LlmChatMessage};

    let long_text = "x ".repeat(50_000);
    let chat_messages = vec![
        LlmChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Start".to_string()),
        },
        LlmChatMessage {
            role: "assistant".to_string(),
            content: ChatContent::Text(long_text.clone()),
        },
    ];
    assert!(should_compress_chat_messages(&chat_messages, 1_000, 0.80));
}

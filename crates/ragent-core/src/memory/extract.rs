//! Automatic memory extraction from conversations and tool usage.
//!
//! The [`ExtractionEngine`] observes tool executions and session events,
//! identifies patterns, error resolutions, and key learnings, then proposes
//! them as [`MemoryCandidate`] entries for storage in the structured memory
//! system.
//!
//! # Hook points
//!
//! Extraction hooks are called at well-defined points in the session lifecycle:
//!
//! - **After tool execution** ([`ExtractionEngine::on_tool_result`]):
//!   Inspects file edits for coding patterns, bash failures for error
//!   resolutions, and other tool results for insights.
//!
//! - **At session end** ([`ExtractionEngine::on_session_end`]):
//!   Compiles a summary of key activities and learnings from the
//!   conversation history.
//!
//! # Confirmation flow
//!
//! When `auto_extract.require_confirmation` is `true` (default), extracted
//! candidates are emitted as events but **not** automatically stored. The
//! agent or user must explicitly call `memory_store` to persist them.
//!
//! When `require_confirmation` is `false`, candidates are auto-stored
//! directly into the structured memory database.
//!
//! # Configuration
//!
//! Controlled by the `auto_extract` section of `memory` in `ragent.json`:
//!
//! ```json
//! {
//!   "memory": {
//!     "auto_extract": {
//!       "enabled": true,
//!       "require_confirmation": true
//!     }
//!   }
//! }
//! ```

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::config::AutoExtractConfig;
use crate::event::EventBus;
use crate::storage::Storage;

// ── MemoryCandidate ──────────────────────────────────────────────────────────

/// A proposed memory entry awaiting user or agent confirmation.
///
/// Unlike a stored [`StructuredMemory`](super::store::StructuredMemory),
/// a candidate has not yet been persisted. It represents a learning or
/// pattern that the extraction engine identified from tool usage or
/// conversation context.
///
/// # Confirmation flow
///
/// 1. Extraction engine produces a `MemoryCandidate`.
/// 2. If `require_confirmation` is `true`, an event is emitted and the
///    candidate is **not** stored. The agent may call `memory_store` to
///    persist it.
/// 3. If `require_confirmation` is `false`, the candidate is auto-stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCandidate {
    /// The memory content to store.
    pub content: String,
    /// Category: one of `fact`, `pattern`, `preference`, `insight`, `error`, `workflow`.
    pub category: String,
    /// Tags for filtering and categorisation.
    pub tags: Vec<String>,
    /// Confidence score (0.0–1.0).
    pub confidence: f64,
    /// Source of the extraction (e.g., "auto-extract/bash", "auto-extract/edit").
    pub source: String,
    /// Human-readable explanation of why this was extracted.
    pub reason: String,
}

// ── ExtractionEngine ──────────────────────────────────────────────────────────

/// Engine for automatically extracting memories from tool usage and sessions.
///
/// Maintains per-session state for tracking recent tool failures (for
/// error-resolution detection) and proposed candidates (for deduplication).
///
/// # Thread safety
///
/// The engine is `Send + Sync` — all mutable state is protected by interior
/// mutability (`std::sync::Mutex`).
pub struct ExtractionEngine {
    /// Configuration controlling extraction behaviour.
    config: AutoExtractConfig,
    /// Per-session tracker for recent tool failures.
    /// Key: session_id, Value: list of recent failed tool calls.
    failure_tracker: std::sync::Mutex<Vec<FailedToolCall>>,
    /// Set of already-proposed content hashes to prevent duplicates.
    proposed_hashes: std::sync::Mutex<HashSet<u64>>,
}

/// Record of a recent failed tool call, used for error-resolution detection.
#[derive(Debug, Clone)]
struct FailedToolCall {
    /// Session ID where the failure occurred.
    session_id: String,
    /// Name of the tool that failed.
    tool_name: String,
    /// Input that was passed to the tool.
    input: String,
    /// Error message from the tool.
    error: String,
    /// Timestamp of the failure.
    #[allow(dead_code)]
    timestamp: chrono::DateTime<Utc>,
}

impl ExtractionEngine {
    /// Create a new extraction engine with the given configuration.
    pub fn new(config: AutoExtractConfig) -> Self {
        Self {
            config,
            failure_tracker: std::sync::Mutex::new(Vec::new()),
            proposed_hashes: std::sync::Mutex::new(HashSet::new()),
        }
    }

    /// Returns `true` if automatic extraction is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Hook called after each tool execution.
    ///
    /// Inspects the tool name, input, result, and success status to
    /// identify potential memory extractions:
    ///
    /// - **File edits** (`edit`, `write`, `create`, `multiedit`):
    ///   Extract coding patterns from file paths and edit content.
    /// - **Bash errors**: Track failures for error-resolution detection.
    /// - **Bash successes after failures**: Extract the error + resolution.
    /// - **Other tools**: Look for insights in results.
    ///
    /// # Arguments
    ///
    /// * `tool_name` - Name of the tool that was executed.
    /// * `input` - JSON input that was passed to the tool.
    /// * `result_content` - Text result from the tool.
    /// * `success` - Whether the tool call succeeded.
    /// * `session_id` - Current session ID.
    /// * `storage` - Storage for checking dedup and storing candidates.
    /// * `event_bus` - Event bus for publishing extraction events.
    /// * `working_dir` - Current working directory.
    pub fn on_tool_result(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
        result_content: &str,
        success: bool,
        session_id: &str,
        storage: &Arc<Storage>,
        event_bus: &Arc<EventBus>,
        working_dir: &Path,
    ) {
        if !self.config.enabled {
            return;
        }

        // 1. Track bash failures for error-resolution detection.
        if !success && tool_name == "bash" {
            self.track_failure(session_id, tool_name, input, result_content);
            return;
        }

        // 2. Detect error resolution: bash success after a recent failure.
        if success && tool_name == "bash" {
            if let Some(candidate) = self.detect_error_resolution(
                session_id,
                input,
                result_content,
                storage,
                working_dir,
            ) {
                self.process_candidate(candidate, session_id, storage, event_bus);
            }
            // Clear old failures for this session regardless.
            self.clear_failures(session_id);
            return;
        }

        // 3. Extract patterns from file edits.
        if success
            && matches!(
                tool_name,
                "edit" | "write" | "create" | "multiedit" | "str_replace_editor"
            )
        {
            if let Some(candidate) =
                Self::extract_pattern_from_edit(tool_name, input, result_content, working_dir)
            {
                if !self.is_duplicate(&candidate.content, storage) {
                    self.process_candidate(candidate, session_id, storage, event_bus);
                }
            }
        }
    }

    /// Hook called when a session ends.
    ///
    /// Compiles a summary of key activities from the conversation by
    /// inspecting the messages and tool calls. Produces [`MemoryCandidate`]
    /// entries for significant learnings.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session that is ending.
    /// * `messages` - The conversation messages from the session.
    /// * `storage` - Storage for checking dedup and storing candidates.
    /// * `event_bus` - Event bus for publishing extraction events.
    /// * `working_dir` - Current working directory.
    pub fn on_session_end(
        &self,
        session_id: &str,
        messages: &[SessionMessageSummary],
        storage: &Arc<Storage>,
        event_bus: &Arc<EventBus>,
        working_dir: &Path,
    ) {
        if !self.config.enabled {
            return;
        }

        debug!("Session end extraction for session {session_id}");

        let candidates = self.extract_session_summary(session_id, messages, working_dir);

        for candidate in candidates {
            if !self.is_duplicate(&candidate.content, storage) {
                self.process_candidate(candidate, session_id, storage, event_bus);
            }
        }

        // Clean up failure tracker for this session.
        self.clear_failures(session_id);
    }

    // ── Pattern extraction from file edits ──────────────────────────────

    /// Extract a coding pattern from a file edit tool result.
    ///
    /// Detects conventions from file paths (e.g., test file locations,
    /// module structure) and from the content being written (e.g., error
    /// handling style, import grouping).
    fn extract_pattern_from_edit(
        tool_name: &str,
        input: &serde_json::Value,
        _result_content: &str,
        working_dir: &Path,
    ) -> Option<MemoryCandidate> {
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("");

        if path.is_empty() {
            return None;
        }

        // Make the path relative to working_dir for cleaner storage.
        let rel_path = Path::new(path)
            .strip_prefix(working_dir)
            .unwrap_or(Path::new(path))
            .to_string_lossy()
            .to_string();

        let mut tags: Vec<String> = Vec::new();
        let mut content_parts: Vec<String> = Vec::new();
        let mut confidence = 0.5_f64;

        // Detect test files.
        if rel_path.contains("tests/")
            || rel_path.contains("test_")
            || rel_path.contains("_test.")
            || rel_path.contains(".test.")
        {
            tags.push("testing".to_string());
            content_parts.push(format!("Test files are located at: {}", rel_path));
            confidence = 0.6;
        }

        // Detect Rust source files.
        if std::path::Path::new(&rel_path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("rs")) {
            tags.push("rust".to_string());
            let module_path = rel_path.replace('/', "::").replace(".rs", "");
            if module_path.contains("::") {
                content_parts.push(format!("Rust module structure: {module_path}"));
            }
        }

        // Detect Python source files.
        if std::path::Path::new(&rel_path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("py")) {
            tags.push("python".to_string());
        }

        // Detect TypeScript/JavaScript files.
        if std::path::Path::new(&rel_path)
            .extension()
            .is_some_and(|ext| {
                ext.eq_ignore_ascii_case("ts")
                    || ext.eq_ignore_ascii_case("tsx")
                    || ext.eq_ignore_ascii_case("js")
                    || ext.eq_ignore_ascii_case("jsx")
            }) {
            tags.push("typescript".to_string());
        }

        // Detect configuration files.
        if std::path::Path::new(&rel_path)
            .extension()
            .is_some_and(|ext| {
                ext.eq_ignore_ascii_case("toml")
                    || ext.eq_ignore_ascii_case("json")
                    || ext.eq_ignore_ascii_case("yaml")
                    || ext.eq_ignore_ascii_case("yml")
            }) {
            tags.push("config".to_string());
            content_parts.push(format!("Configuration file: {rel_path}"));
            confidence = 0.4;
        }

        // Detect doc files.
        if std::path::Path::new(&rel_path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md")) {
            tags.push("documentation".to_string());
            confidence = 0.3;
        }

        // Detect edit patterns from the content being written.
        if let Some(new_str) = input.get("new_str").and_then(|v| v.as_str()) {
            Self::detect_content_patterns(new_str, &mut content_parts, &mut tags);
        }
        if let Some(content) = input.get("content").and_then(|v| v.as_str()) {
            Self::detect_content_patterns(content, &mut content_parts, &mut tags);
        }
        if let Some(file_text) = input.get("file_text").and_then(|v| v.as_str()) {
            Self::detect_content_patterns(file_text, &mut content_parts, &mut tags);
        }

        if content_parts.is_empty() {
            // Generic: just record that files are edited in this area.
            let parent = Path::new(&rel_path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".to_string());
            if parent != "." && !parent.is_empty() {
                content_parts.push(format!("Source files are organised under: {parent}/"));
            } else {
                return None;
            }
        }

        tags.sort();
        tags.dedup();

        Some(MemoryCandidate {
            content: content_parts.join("; "),
            category: "pattern".to_string(),
            tags,
            confidence,
            source: format!("auto-extract/{tool_name}"),
            reason: format!("Detected from file edit: {rel_path}"),
        })
    }

    /// Detect coding patterns from file content.
    fn detect_content_patterns(
        content: &str,
        content_parts: &mut Vec<String>,
        tags: &mut Vec<String>,
    ) {
        // Detect error handling patterns.
        if content.contains("anyhow::Result") || content.contains("anyhow!") {
            content_parts.push("Project uses anyhow for error handling".to_string());
            tags.push("error-handling".to_string());
        }
        if content.contains("thiserror") {
            content_parts.push("Project uses thiserror for custom error types".to_string());
            tags.push("error-handling".to_string());
        }

        // Detect tracing/logging patterns.
        if content.contains("tracing::") || content.contains("tracing::info") {
            content_parts.push("Project uses the tracing crate for logging".to_string());
            tags.push("logging".to_string());
        }

        // Detect async patterns.
        if content.contains("async fn") || content.contains("#[tokio::test]") {
            content_parts.push("Project uses async Rust (tokio runtime)".to_string());
            tags.push("async".to_string());
        }

        // Detect serde usage.
        if content.contains("serde::")
            || content.contains("#[derive(") && content.contains("Serialize")
        {
            content_parts.push("Project uses serde for serialisation".to_string());
            tags.push("serialisation".to_string());
        }

        // Detect test patterns.
        if content.contains("#[test]") || content.contains("#[tokio::test]") {
            content_parts.push("Project uses Rust's built-in test framework".to_string());
            tags.push("testing".to_string());
        }
    }

    // ── Error resolution extraction ─────────────────────────────────────

    /// Track a failed tool call for later error-resolution detection.
    fn track_failure(
        &self,
        session_id: &str,
        tool_name: &str,
        input: &serde_json::Value,
        error: &str,
    ) {
        let mut tracker = self
            .failure_tracker
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // Keep only the most recent 20 failures per session.
        tracker.retain(|f| f.session_id == session_id);
        if tracker.len() >= 20 {
            tracker.remove(0);
        }
        let input_str = serde_json::to_string(input).unwrap_or_default();
        tracker.push(FailedToolCall {
            session_id: session_id.to_string(),
            tool_name: tool_name.to_string(),
            input: input_str,
            error: error.to_string(),
            timestamp: Utc::now(),
        });
        debug!("Tracked tool failure: {tool_name} in session {session_id}");
    }

    /// Clear failure records for a session (after resolution or session end).
    fn clear_failures(&self, session_id: &str) {
        let mut tracker = self
            .failure_tracker
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        tracker.retain(|f| f.session_id != session_id);
    }

    /// Detect that a recent bash failure has been resolved by a subsequent success.
    ///
    /// Looks for recent bash failures in the same session and generates
    /// a `MemoryCandidate` with category `"error"` documenting the problem
    /// and solution.
    fn detect_error_resolution(
        &self,
        session_id: &str,
        _input: &serde_json::Value,
        _result_content: &str,
        storage: &Arc<Storage>,
        working_dir: &Path,
    ) -> Option<MemoryCandidate> {
        let tracker = self
            .failure_tracker
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let recent_failures: Vec<&FailedToolCall> = tracker
            .iter()
            .filter(|f| f.session_id == session_id && f.tool_name == "bash")
            .collect();

        if recent_failures.is_empty() {
            return None;
        }

        // Use the most recent failure.
        let failure = recent_failures.last()?;
        let error_msg = failure.error.clone();

        // Truncate the error for readability.
        let truncated_error = if error_msg.len() > 300 {
            format!("{}…", &error_msg[..300])
        } else {
            error_msg
        };

        // Extract the command from the input.
        let command = failure
            .input
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(&failure.input);
        let truncated_cmd = if command.len() > 100 {
            format!("{}…", &command[..100])
        } else {
            command.to_string()
        };

        let project_name = working_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let content = format!(
            "Command `{truncated_cmd}` failed with: {truncated_error}. \
             Subsequent execution succeeded, indicating the issue was resolved."
        );

        // Check for duplicates.
        if self.is_duplicate_content(&content, storage) {
            return None;
        }

        Some(MemoryCandidate {
            content,
            category: "error".to_string(),
            tags: vec![
                "bash".to_string(),
                "error-resolution".to_string(),
                project_name,
            ],
            confidence: 0.7,
            source: "auto-extract/bash-error-resolution".to_string(),
            reason: "Detected bash failure followed by successful retry".to_string(),
        })
    }

    // ── Session summary extraction ──────────────────────────────────────

    /// Extract key learnings from the session's conversation history.
    ///
    /// Produces candidates summarising the work done (files edited,
    /// commands run, errors encountered) as structured memories.
    fn extract_session_summary(
        &self,
        session_id: &str,
        messages: &[SessionMessageSummary],
        working_dir: &Path,
    ) -> Vec<MemoryCandidate> {
        let project_name = working_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let mut candidates = Vec::new();
        let mut edited_files: HashSet<String> = HashSet::new();
        let mut tools_used: HashSet<String> = HashSet::new();
        let mut error_count = 0usize;

        // Scan messages for tool usage patterns.
        for msg in messages {
            if msg.role == "assistant" {
                for tc in &msg.tool_calls {
                    tools_used.insert(tc.tool_name.clone());

                    // Track edited files.
                    if matches!(
                        tc.tool_name.as_str(),
                        "edit" | "write" | "create" | "multiedit" | "str_replace_editor"
                    ) {
                        if let Some(path) = tc.input.get("path").and_then(|v| v.as_str()) {
                            let rel = Path::new(path)
                                .strip_prefix(working_dir)
                                .unwrap_or(Path::new(path))
                                .to_string_lossy()
                                .to_string();
                            edited_files.insert(rel);
                        }
                    }

                    // Count errors.
                    if !tc.success {
                        error_count += 1;
                    }
                }
            }
        }

        // Produce a summary candidate if there was meaningful activity.
        if !edited_files.is_empty() || !tools_used.is_empty() {
            let mut summary_parts: Vec<String> = Vec::new();

            if !edited_files.is_empty() {
                let files: Vec<&str> = edited_files.iter().map(|s| s.as_str()).take(10).collect();
                summary_parts.push(format!("Files edited: {}", files.join(", ")));
            }

            if error_count > 0 {
                summary_parts.push(format!(
                    "Encountered {error_count} tool errors during session"
                ));
            }

            let tools: Vec<&str> = tools_used.iter().map(|s| s.as_str()).take(10).collect();
            summary_parts.push(format!("Tools used: {}", tools.join(", ")));

            candidates.push(MemoryCandidate {
                content: format!("Session {session_id} summary: {}", summary_parts.join(". ")),
                category: "workflow".to_string(),
                tags: vec!["session-summary".to_string(), project_name],
                confidence: 0.4,
                source: "auto-extract/session-summary".to_string(),
                reason: "Automatically generated at session end".to_string(),
            });
        }

        candidates
    }

    // ── Deduplication ───────────────────────────────────────────────────

    /// Check whether a candidate with similar content already exists in
    /// either the proposed set or the stored memories.
    fn is_duplicate(&self, content: &str, storage: &Arc<Storage>) -> bool {
        // Check proposed hash set first (fast, in-memory).
        let hash = Self::content_hash(content);
        {
            let proposed = self
                .proposed_hashes
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if proposed.contains(&hash) {
                return true;
            }
        }

        // Check stored memories via FTS5.
        self.is_duplicate_content(content, storage)
    }

    /// Check if similar content exists in the structured memory store.
    fn is_duplicate_content(&self, content: &str, storage: &Arc<Storage>) -> bool {
        // Extract a few key terms from the content for FTS search.
        let search_terms: Vec<&str> = content
            .split_whitespace()
            .filter(|w| w.len() > 4 && !Self::is_stop_word(w))
            .take(5)
            .collect();

        if search_terms.is_empty() {
            return false;
        }

        let query = search_terms.join(" ");
        match storage.search_memories(&query, None, None, 3, 0.0) {
            Ok(results) => {
                // Simple heuristic: if any existing memory has >70% word overlap, it's a dup.
                for existing in &results {
                    let overlap = Self::word_overlap(content, &existing.content);
                    if overlap > 0.7 {
                        debug!(
                            "Skipping duplicate candidate (overlap {:.0}%): {}",
                            overlap * 100.0,
                            &content[..content.len().min(60)]
                        );
                        return true;
                    }
                }
                false
            }
            Err(_) => false,
        }
    }

    /// Compute a simple hash of the content for dedup.
    fn content_hash(content: &str) -> u64 {
        use std::hash::Hasher;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        // Normalise: lowercase, trim whitespace.
        let normalised = content.to_lowercase().trim().to_string();
        std::hash::Hash::hash_slice(normalised.as_bytes(), &mut hasher);
        hasher.finish()
    }

    /// Compute word overlap ratio between two strings.
    fn word_overlap(a: &str, b: &str) -> f64 {
        let a_lower = a.to_lowercase();
        let b_lower = b.to_lowercase();
        let words_a: HashSet<&str> = a_lower.split_whitespace().collect();
        let words_b: HashSet<&str> = b_lower.split_whitespace().collect();
        if words_a.is_empty() || words_b.is_empty() {
            return 0.0;
        }
        let intersection = words_a.intersection(&words_b).count();
        intersection as f64 / words_a.len().max(words_b.len()) as f64
    }

    /// Common English stop words to skip in dedup queries.
    fn is_stop_word(word: &str) -> bool {
        const STOP_WORDS: &[&str] = &[
            "the", "and", "for", "that", "with", "this", "from", "are", "was", "were", "been",
            "have", "has", "had", "will", "would", "could", "should", "into", "about", "which",
            "their", "other", "than", "then", "also", "when", "what", "each", "does", "just",
            "more", "some", "over", "such", "after", "before", "between", "through", "during",
            "without", "using", "project", "files",
        ];
        STOP_WORDS.contains(&word.to_lowercase().as_str())
    }

    // ── Candidate processing ───────────────────────────────────────────

    /// Process a candidate: auto-store or emit event depending on config.
    fn process_candidate(
        &self,
        candidate: MemoryCandidate,
        session_id: &str,
        storage: &Arc<Storage>,
        event_bus: &Arc<EventBus>,
    ) {
        let hash = Self::content_hash(&candidate.content);
        {
            let mut proposed = self
                .proposed_hashes
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            proposed.insert(hash);
        }

        if self.config.require_confirmation {
            // Emit event but don't store.
            info!(
                category = candidate.category,
                confidence = candidate.confidence,
                source = candidate.source,
                "Memory candidate extracted (awaiting confirmation): {}",
                &candidate.content[..candidate.content.len().min(80)]
            );
            event_bus.publish(Event::MemoryCandidateExtracted {
                session_id: session_id.to_string(),
                content: candidate.content.clone(),
                category: candidate.category.clone(),
                tags: candidate.tags.clone(),
                confidence: candidate.confidence,
                source: candidate.source.clone(),
                reason: candidate.reason.clone(),
            });
        } else {
            // Auto-store the candidate.
            let project = storage
                .get_setting("project_name")
                .ok()
                .flatten()
                .unwrap_or_default();

            match storage.create_memory(
                &candidate.content,
                &candidate.category,
                &candidate.source,
                candidate.confidence,
                &project,
                session_id,
                &candidate.tags,
            ) {
                Ok(id) => {
                    info!(
                        id,
                        category = candidate.category,
                        "Auto-stored memory candidate"
                    );
                    event_bus.publish(Event::MemoryStored {
                        session_id: session_id.to_string(),
                        id,
                        category: candidate.category.clone(),
                    });
                }
                Err(e) => {
                    warn!(error = %e, "Failed to auto-store memory candidate");
                }
            }
        }
    }
}

// ── Session message summary ──────────────────────────────────────────────────

/// Summary of a message in the session, used for session-end extraction.
///
/// This lightweight representation avoids cloning the entire message
/// history. Only the fields needed for extraction are included.
#[derive(Debug, Clone)]
pub struct SessionMessageSummary {
    /// Role of the message author (`user`, `assistant`, `system`).
    pub role: String,
    /// Tool calls made in this message (assistant messages only).
    pub tool_calls: Vec<ToolCallSummary>,
}

/// Summary of a single tool call within a message.
#[derive(Debug, Clone)]
pub struct ToolCallSummary {
    /// Name of the tool that was called.
    pub tool_name: String,
    /// JSON input to the tool call.
    pub input: serde_json::Value,
    /// Whether the tool call succeeded.
    pub success: bool,
}

// ── Confidence decay ──────────────────────────────────────────────────────────

/// Apply time-based confidence decay to all memories.
///
/// Memories that have not been accessed recently have their confidence
/// reduced by a multiplicative decay factor. This ensures that stale,
/// unconfirmed memories gradually fade, while frequently recalled
/// memories maintain high confidence.
///
/// # Arguments
///
/// * `storage` - The SQLite storage backend.
/// * `decay_factor` - Multiplicative factor per day since last access (e.g., 0.95).
/// * `min_confidence` - Don't decay below this threshold (e.g., 0.1).
///
/// # Returns
///
/// The number of memories whose confidence was updated.
pub fn decay_confidence(storage: &Storage, decay_factor: f64, min_confidence: f64) -> usize {
    let memories = match storage.list_memories("", 100_000) {
        Ok(m) => m,
        Err(e) => {
            warn!(error = %e, "Failed to list memories for decay");
            return 0;
        }
    };

    let now = Utc::now();
    let mut updated = 0usize;

    for mem in &memories {
        // Determine the reference time for decay.
        let reference_time = mem
            .last_accessed
            .as_ref()
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .or_else(|| {
                chrono::DateTime::parse_from_rfc3339(&mem.updated_at)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            })
            .or_else(|| {
                chrono::DateTime::parse_from_rfc3339(&mem.created_at)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            });

        let Some(reference_time) = reference_time else {
            continue;
        };

        let days_since_access = (now - reference_time).num_days().max(0) as f64;
        if days_since_access < 1.0 {
            // Accessed today, no decay.
            continue;
        }

        // Apply exponential decay: confidence *= factor^days
        let new_confidence = mem.confidence * decay_factor.powf(days_since_access);

        // Clamp to minimum.
        let new_confidence = new_confidence.max(min_confidence);

        // Only update if there's a meaningful change.
        if (new_confidence - mem.confidence).abs() > 0.001 {
            if storage
                .update_memory_confidence(mem.id, new_confidence)
                .unwrap_or(false)
            {
                updated += 1;
            }
        }
    }

    if updated > 0 {
        info!(updated, "Applied confidence decay to memories");
    }

    updated
}

use crate::event::Event;

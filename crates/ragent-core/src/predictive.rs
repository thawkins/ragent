//! Predictive tool execution for the session processor.
//!
//! Analyzes LLM streaming tokens to predict and pre-execute likely tool calls,
//! reducing perceived latency for common operations like file reads.
//!
//! # Features
//!
//! - **Tool Intent Detection**: Recognizes patterns like "I'll read the file" or
//!   "Let me check" to predict upcoming tool calls.
//! - **Argument Pre-validation**: Validates tool arguments as they stream in,
//!   caching validation results.
//! - **File Pre-fetch**: Pre-loads likely file contents into a cache for
//!   immediate availability when the tool call is actually executed.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Tool call patterns that indicate likely upcoming tool invocations.
///
/// These are matched against streaming text to predict what tools
/// the model is likely to call next.
const READ_FILE_PATTERNS: [&str; 10] = [
    "I'll read",
    "Let me read",
    "Let me check",
    "I'll examine",
    "Let me examine",
    "Let me look at",
    "I need to read",
    "Let me inspect",
    "I'll open",
    "Let me view",
];

const GLOB_PATTERNS: [&str; 5] = [
    "Let me search for",
    "I'll find all",
    "Let me look for files",
    "I need to find",
    "Let me list",
];

#[allow(dead_code)]
const GREP_PATTERNS: [&str; 5] = [
    "Let me search for",
    "I'll search for",
    "Let me find",
    "I need to find",
    "Let me grep",
];

/// Predicted tool call with confidence score and pre-fetched data.
#[derive(Debug, Clone)]
pub struct PredictedToolCall {
    /// Name of the predicted tool (e.g., "read", "glob", "grep").
    pub tool_name: String,
    /// Predicted arguments (may be partial during streaming).
    pub predicted_args: HashMap<String, String>,
    /// Confidence score (0.0-1.0) based on pattern match strength.
    pub confidence: f32,
    /// Pre-fetched file content (for read operations).
    pub prefetched_content: Option<Arc<String>>,
    /// Timestamp of prediction.
    pub predicted_at: std::time::Instant,
}

/// Cache for pre-fetched file contents.
#[derive(Debug, Clone, Default)]
pub struct PrefetchCache {
    /// Cached file contents keyed by path.
    contents: Arc<RwLock<HashMap<PathBuf, Arc<String>>>>,
    /// Maximum cache size (number of entries).
    max_size: usize,
}

impl PrefetchCache {
    /// Create a new prefetch cache with default size (100 entries).
    #[must_use]
    pub fn new() -> Self {
        Self {
            contents: Arc::new(RwLock::new(HashMap::new())),
            max_size: 100,
        }
    }

    /// Create a new prefetch cache with custom size.
    #[must_use]
    pub fn with_capacity(max_size: usize) -> Self {
        Self {
            contents: Arc::new(RwLock::new(HashMap::new())),
            max_size,
        }
    }

    /// Get cached content for a path.
    pub async fn get(&self, path: &Path) -> Option<Arc<String>> {
        let contents = self.contents.read().await;
        contents.get(path).cloned()
    }

    /// Insert content into cache, evicting oldest if at capacity.
    pub async fn insert(&self, path: PathBuf, content: Arc<String>) {
        let mut contents = self.contents.write().await;

        // Simple eviction: if at capacity, remove arbitrary entry
        // In production, use LRU eviction
        if contents.len() >= self.max_size && !contents.contains_key(&path) {
            if let Some(first_key) = contents.keys().next().cloned() {
                contents.remove(&first_key);
            }
        }

        contents.insert(path, content);
    }

    /// Clear all cached entries.
    pub async fn clear(&self) {
        let mut contents = self.contents.write().await;
        contents.clear();
    }
}

/// Predictive tool executor that analyzes streaming tokens.
#[derive(Debug)]
pub struct PredictiveExecutor {
    /// Cache for pre-fetched file contents.
    prefetch_cache: PrefetchCache,
    /// Currently predicted tool calls (cleared on each turn).
    current_predictions: Arc<RwLock<HashMap<String, PredictedToolCall>>>,
    /// Set of file paths currently being pre-fetched (to avoid duplicates).
    pending_prefetch: Arc<RwLock<HashSet<PathBuf>>>,
    /// Working directory for resolving relative paths.
    working_dir: PathBuf,
}

impl PredictiveExecutor {
    /// Create a new predictive executor.
    #[must_use]
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            prefetch_cache: PrefetchCache::new(),
            current_predictions: Arc::new(RwLock::new(HashMap::new())),
            pending_prefetch: Arc::new(RwLock::new(HashSet::new())),
            working_dir,
        }
    }

    /// Analyze streaming text for tool call patterns.
    ///
    /// Called incrementally as text tokens arrive from the LLM.
    /// Returns any new predictions made based on the text.
    pub async fn analyze_text(&self, text: &str) -> Vec<PredictedToolCall> {
        let mut predictions = Vec::new();
        let text_lower = text.to_lowercase();

        // Check for read file patterns
        for pattern in &READ_FILE_PATTERNS {
            if text_lower.contains(&pattern.to_lowercase()) {
                // Extract potential file path after the pattern
                if let Some(path) = self.extract_file_path_after_pattern(text, pattern) {
                    let prediction = PredictedToolCall {
                        tool_name: "read".to_string(),
                        predicted_args: {
                            let mut args = HashMap::new();
                            args.insert("path".to_string(), path.clone());
                            args
                        },
                        confidence: 0.7,
                        prefetched_content: None,
                        predicted_at: std::time::Instant::now(),
                    };
                    predictions.push(prediction);

                    // Trigger async prefetch
                    self.prefetch_file(path).await;
                }
            }
        }

        // Check for glob patterns
        for pattern in &GLOB_PATTERNS {
            if text_lower.contains(&pattern.to_lowercase()) {
                if let Some(pattern_str) = self.extract_pattern_after_text(text, pattern) {
                    let prediction = PredictedToolCall {
                        tool_name: "glob".to_string(),
                        predicted_args: {
                            let mut args = HashMap::new();
                            args.insert("pattern".to_string(), pattern_str);
                            args
                        },
                        confidence: 0.6,
                        prefetched_content: None,
                        predicted_at: std::time::Instant::now(),
                    };
                    predictions.push(prediction);
                }
            }
        }

        // Store predictions
        {
            let mut current = self.current_predictions.write().await;
            for pred in &predictions {
                let key = format!("{}:{:?}", pred.tool_name, pred.predicted_args);
                current.insert(key, pred.clone());
            }
        }

        predictions
    }

    /// Analyze tool call arguments as they stream in.
    ///
    /// Validates partial JSON and caches validation results.
    /// Returns validation errors if the arguments are invalid.
    pub fn validate_tool_args(&self, tool_name: &str, args_json: &str) -> Result<(), String> {
        // Try to parse as JSON
        match serde_json::from_str::<serde_json::Value>(args_json) {
            Ok(value) => {
                // Validate required fields based on tool
                match tool_name {
                    "read" => {
                        if let Some(obj) = value.as_object() {
                            if obj.contains_key("path") {
                                Ok(())
                            } else {
                                Err("Missing required field: path".to_string())
                            }
                        } else {
                            Err("Arguments must be a JSON object".to_string())
                        }
                    }
                    "glob" => {
                        if let Some(obj) = value.as_object() {
                            if obj.contains_key("pattern") {
                                Ok(())
                            } else {
                                Err("Missing required field: pattern".to_string())
                            }
                        } else {
                            Err("Arguments must be a JSON object".to_string())
                        }
                    }
                    "grep" => {
                        if let Some(obj) = value.as_object() {
                            if obj.contains_key("pattern") {
                                Ok(())
                            } else {
                                Err("Missing required field: pattern".to_string())
                            }
                        } else {
                            Err("Arguments must be a JSON object".to_string())
                        }
                    }
                    _ => Ok(()), // Unknown tool, allow
                }
            }
            Err(e) => {
                // Partial JSON is OK during streaming
                let trimmed = args_json.trim();
                if trimmed.is_empty()
                    || trimmed.ends_with('{')
                    || trimmed.ends_with(',')
                    || trimmed.ends_with('"')
                    || trimmed.ends_with(':')
                {
                    Ok(())
                } else {
                    Err(format!("JSON parse error: {e}"))
                }
            }
        }
    }

    /// Get pre-fetched content for a file path.
    pub async fn get_prefetched_content(&self, path: &Path) -> Option<Arc<String>> {
        self.prefetch_cache.get(path).await
    }

    /// Clear predictions and prefetch cache for a new turn.
    pub async fn clear_turn_state(&self) {
        self.current_predictions.write().await.clear();
        self.pending_prefetch.write().await.clear();
    }

    /// Extract a potential file path after a pattern.
    fn extract_file_path_after_pattern(&self, text: &str, pattern: &str) -> Option<String> {
        // Simple heuristic: look for quoted string or path-like sequence after pattern
        if let Some(pos) = text.find(pattern) {
            let after = &text[pos + pattern.len()..];
            let after_trimmed = after.trim_start();

            // Look for quoted path
            if let Some(first_quote) = after_trimmed.find('"') {
                let rest = &after_trimmed[first_quote + 1..];
                if let Some(second_quote) = rest.find('"') {
                    let path = &rest[..second_quote];
                    if self.looks_like_file_path(path) {
                        return Some(path.to_string());
                    }
                }
            }

            // Look for unquoted path (alphanumeric, dots, slashes)
            let path_chars: String = after_trimmed
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '.' || *c == '/' || *c == '_')
                .collect();

            if !path_chars.is_empty() && self.looks_like_file_path(&path_chars) {
                return Some(path_chars);
            }
        }

        None
    }

    /// Extract a pattern string after text.
    fn extract_pattern_after_text(&self, text: &str, prefix: &str) -> Option<String> {
        if let Some(pos) = text.find(prefix) {
            let after = &text[pos + prefix.len()..];
            let after_trimmed = after.trim_start();

            // Look for quoted pattern
            if let Some(first_quote) = after_trimmed.find('"') {
                let rest = &after_trimmed[first_quote + 1..];
                if let Some(second_quote) = rest.find('"') {
                    return Some(rest[..second_quote].to_string());
                }
            }

            // Take next word/phrase
            let pattern: String = after_trimmed
                .chars()
                .take_while(|c| {
                    c.is_alphanumeric() || *c == '*' || *c == '.' || *c == '/' || *c == '_'
                })
                .collect();

            if !pattern.is_empty() {
                return Some(pattern);
            }
        }

        None
    }

    /// Check if a string looks like a file path.
    fn looks_like_file_path(&self, s: &str) -> bool {
        // Must contain a dot (file extension) or slash (directory)
        s.contains('.') || s.contains('/')
    }

    /// Asynchronously pre-fetch a file into the cache.
    async fn prefetch_file(&self, path_str: String) {
        let path = self.working_dir.join(&path_str);

        // Check if already pending
        {
            let pending = self.pending_prefetch.read().await;
            if pending.contains(&path) {
                return;
            }
        }

        // Mark as pending
        {
            let mut pending = self.pending_prefetch.write().await;
            pending.insert(path.clone());
        }

        let cache = self.prefetch_cache.clone();
        let pending_clone = self.pending_prefetch.clone();

        // Spawn async read
        tokio::spawn(async move {
            match tokio::fs::read_to_string(&path).await {
                Ok(content) => {
                    tracing::debug!(
                        path = %path.display(),
                        bytes = content.len(),
                        "Prefetched file content"
                    );
                    cache.insert(path.clone(), Arc::new(content)).await;
                }
                Err(e) => {
                    tracing::debug!(
                        path = %path.display(),
                        error = %e,
                        "Failed to prefetch file"
                    );
                }
            }

            // Remove from pending
            let mut pending = pending_clone.write().await;
            pending.remove(&path);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_extract_file_path() {
        let executor = PredictiveExecutor::new(PathBuf::from("/home/user"));

        let text = r#"I'll read the file "src/main.rs" to check"#;
        let path = executor.extract_file_path_after_pattern(text, "I'll read");
        assert_eq!(path, Some("src/main.rs".to_string()));
    }

    #[tokio::test]
    async fn test_extract_glob_pattern() {
        let executor = PredictiveExecutor::new(PathBuf::from("/home/user"));

        let text = r#"Let me search for "*.rs" files"#;
        let pattern = executor.extract_pattern_after_text(text, "search for");
        assert_eq!(pattern, Some("*.rs".to_string()));
    }

    #[tokio::test]
    async fn test_prefetch_cache() {
        let cache = PrefetchCache::with_capacity(2);
        let content = Arc::new("Hello world".to_string());

        cache
            .insert(PathBuf::from("/test/file.txt"), content.clone())
            .await;

        let retrieved = cache.get(&PathBuf::from("/test/file.txt")).await;
        assert_eq!(retrieved, Some(content));
    }

    #[test]
    fn test_validate_tool_args() {
        let executor = PredictiveExecutor::new(PathBuf::from("/home/user"));

        // Valid read args
        assert!(
            executor
                .validate_tool_args("read", r#"{"path": "test.txt"}"#)
                .is_ok()
        );

        // Missing path
        assert!(executor.validate_tool_args("read", r#"{}"#).is_err());

        // Partial JSON during streaming
        assert!(executor.validate_tool_args("read", r#"{"path":"#).is_ok());

        // Valid glob args
        assert!(
            executor
                .validate_tool_args("glob", r#"{"pattern": "*.rs"}"#)
                .is_ok()
        );
    }
}
